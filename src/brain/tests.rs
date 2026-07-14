//! Brain behavior tests. All run against the fresh-per-process temp sqlite
//! that `store::brain_db_path()` resolves under cfg(test). The path must NOT
//! be re-pointed here via `SCRIPT_KIT_TEST_BRAIN_DB_PATH`: `store::BRAIN_DB` is
//! a process-global `OnceLock`, and brain-adjacent tests elsewhere in the
//! suite (input-history selection signals, launcher grouping, MCP resources)
//! can bind it before this module's setup runs — a per-module env var would
//! then disagree with the already-bound connection. Each brain test holds a
//! module-local mutex guard and clears mutable rows so the default parallel
//! runner cannot leave stale docs or orphan embedding rows for sibling tests.

use chrono::TimeZone as _;

use super::curator;
use super::inbox::{self, InboxKind};
use super::indexer::{
    extract_topics, sync_day_pages_with_substrate, sync_file_sources_for_recall_with_substrate,
    sync_fragments_with_substrate, sync_notes_with_substrate,
};
use super::search::{self, aggregate_signals, cosine_top_ids, fuse_ranks};
use super::store::{self, BrainDoc, BrainSignal, DocSource};
use super::substrate::{BrainFrontmatter, BrainSubstrate, DayEntry};
use super::telegram;
use crate::notes::NoteId;

fn init_test_db() -> std::sync::MutexGuard<'static, ()> {
    static TEST_DB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = TEST_DB_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        store::init_brain_db().expect("init test brain db");
    });
    store::reset_for_test().expect("reset test brain db");
    guard
}

fn doc(id: i64, title: &str, content: &str) -> BrainDoc {
    BrainDoc {
        id,
        source: DocSource::Note,
        source_id: id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
        canonical_path: None,
        updated_at: 0,
    }
}

#[test]
fn upsert_is_idempotent_and_updates_on_change() {
    let _db = init_test_db();
    let id1 = store::upsert_doc(DocSource::Note, "n-upsert", "Title", "body", 100).unwrap();
    let id2 = store::upsert_doc(DocSource::Note, "n-upsert", "Title", "body", 100).unwrap();
    assert_eq!(id1, id2, "same source_id must keep one row");
    let id3 = store::upsert_doc(DocSource::Note, "n-upsert", "Title", "body v2", 200).unwrap();
    assert_eq!(id1, id3);
    let docs = store::get_docs_by_ids(&[id1]).unwrap();
    assert_eq!(docs[0].content, "body v2");
}

#[test]
fn fts_matches_natural_language_questions() {
    let _db = init_test_db();
    let id = store::upsert_doc(
        DocSource::Note,
        "n-nlq",
        "Project Bluefin launch checklist",
        "Project Bluefin deploys from the zephyr-42 branch.",
        100,
    )
    .unwrap();
    // Question contains filler words absent from the doc — OR semantics must
    // still surface it.
    let hits = store::fts_search("what branch does project bluefin deploy from", 10).unwrap();
    assert!(hits.contains(&id), "natural-language question should match");
}

/// Regression: a fresh database used to make the curator "due" immediately,
/// firing a live LLM call (and surfacing surprise inbox items) seconds after
/// first launch. The first `run_if_due` on a fresh DB must only stamp the
/// marker and defer real distillation a full interval.
#[test]
fn curator_first_run_on_fresh_db_only_stamps_marker() {
    let _db = init_test_db();
    assert!(
        store::meta_get("curator_last_run").unwrap().is_none(),
        "fresh db must start without a curator marker"
    );
    curator::run_if_due();
    let stamped = store::meta_get("curator_last_run")
        .unwrap()
        .expect("first run_if_due must stamp the marker");
    let stamped: i64 = stamped.parse().unwrap();
    let now = chrono::Utc::now().timestamp();
    assert!((now - stamped).abs() < 60, "marker must be stamped to now");
}

/// Regression: identical content stored under several sources (clipboard +
/// note + chat turn) used to fill every launcher slot with duplicates.
/// `brain_search` must collapse them to one hit and let distinct memories in.
#[test]
fn brain_search_dedupes_identical_content_across_sources() {
    let _db = init_test_db();
    let dup = "We agreed to upgrade the dedupletron cluster to 1.31 next sprint.";
    store::upsert_doc(
        DocSource::Note,
        "dup-note",
        "dedupletron upgrade plan",
        dup,
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::Clipboard,
        "dup-clip",
        "dedupletron upgrade plan",
        dup,
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::ChatTurn,
        "dup-chat#0",
        "dedupletron upgrade plan",
        dup,
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::Note,
        "dup-other",
        "dedupletron rollback notes",
        "Rollback plan if the dedupletron upgrade fails: drain and restore.",
        100,
    )
    .unwrap();
    let hits = search::brain_search("dedupletron upgrade", None, None, 4).unwrap();
    let dup_hits = hits.iter().filter(|hit| hit.doc.content == dup).count();
    assert_eq!(dup_hits, 1, "identical content must collapse to one hit");
    assert!(
        hits.iter().any(|hit| hit.doc.source_id == "dup-other"),
        "distinct memory should fill the freed slot"
    );
}

#[test]
fn brain_search_proximity_filters_scattered_and_partial_multi_word_hits() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::Note,
        "proximity-partial",
        "A new memory",
        "This contains new but neither of the other requested terms.",
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::Note,
        "proximity-scattered",
        "Create plans",
        "create one two three four five six seven eight nine ten eleven twelve new thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty flow",
        100,
    )
    .unwrap();

    let hits = search::brain_search_proximity("create new flow", None, None, 4).unwrap();
    assert!(
        !hits.iter().any(|hit| matches!(
            hit.doc.source_id.as_str(),
            "proximity-partial" | "proximity-scattered"
        )),
        "partial and widely scattered terms must not enter passive launcher results"
    );

    let or_hits = search::brain_search("create new flow", None, None, 4).unwrap();
    assert!(
        or_hits
            .iter()
            .any(|hit| hit.doc.source_id == "proximity-partial"),
        "agent recall must retain OR semantics"
    );
}

#[test]
fn brain_search_proximity_matches_nearby_and_single_terms() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::Note,
        "proximity-nearby",
        "Deployment inbox",
        "Review the vercel emails before lunch.",
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::Note,
        "proximity-single",
        "Wildlife",
        "A quokka checks the build status.",
        100,
    )
    .unwrap();

    let nearby = search::brain_search_proximity("vercel emails", None, None, 4).unwrap();
    assert!(nearby
        .iter()
        .any(|hit| hit.doc.source_id == "proximity-nearby"));
    let single = search::brain_search_proximity("quokka", None, None, 4).unwrap();
    assert!(single
        .iter()
        .any(|hit| hit.doc.source_id == "proximity-single"));
}

/// The proximity substring fallback (emoji/symbol queries FTS cannot tokenize)
/// must require EVERY term. Each per-term `title LIKE ?n OR content LIKE ?n`
/// group needs its own parentheses — without them SQL's tighter-binding AND
/// turns the multi-term clause into an effective OR and readmits partial
/// matches.
#[test]
fn substring_search_proximity_requires_every_term() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::Note,
        "emoji-both",
        "Launch day",
        "Shipping the 🚀 build after the 🌮 party.",
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::Note,
        "emoji-partial",
        "Snack notes",
        "Only the 🌮 shows up in this memory.",
        100,
    )
    .unwrap();

    let both = store::substring_search_proximity("🚀 🌮", 10).unwrap();
    let partial_doc = store::get_doc(DocSource::Note, "emoji-partial")
        .unwrap()
        .unwrap();
    let both_doc = store::get_doc(DocSource::Note, "emoji-both")
        .unwrap()
        .unwrap();
    assert!(
        both.contains(&both_doc.id),
        "doc with every term must match"
    );
    assert!(
        !both.contains(&partial_doc.id),
        "doc missing a term must not match the AND fallback"
    );
}

/// Recall SELECTs route through a SEPARATE read-only connection so an in-flight
/// indexer write (which holds the single write `Mutex<Connection>`) cannot stall
/// a submit-path recall query. Proves both correctness (read path returns the
/// seeded doc) and the regression the fix prevents (a thread holding the write
/// lock for 500ms must not delay recall — pre-fix it shared that one Mutex and
/// blocked). If the read connection silently fell back to the write connection,
/// the timing assertion below would fail, so this also guards the open path.
#[test]
fn recall_uses_read_connection_and_does_not_queue_behind_writes() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::Note,
        "readconn-doc",
        "readconn recall subject",
        "The quokka telemetry pipeline ships nightly to the readconn warehouse.",
        100,
    )
    .unwrap();

    // brain_search now routes its SELECTs through with_read_conn; it must still
    // find the seeded doc via the read connection.
    let hits = search::brain_search("quokka telemetry", None, None, 4).unwrap();
    assert!(
        hits.iter().any(|hit| hit.doc.source_id == "readconn-doc"),
        "read-path recall must return the seeded doc"
    );

    // Hold the WRITE connection Mutex for 2s in a background thread, then time a
    // recall query on this thread. A separate read connection returns in
    // single-digit ms; the pre-fix shared Mutex would make recall wait out the
    // full 2s writer. The wide gap (recall well under 1s vs a 2s hold) keeps this
    // robust under CI load while still catching a queue-behind-writes regression.
    const WRITE_HOLD: std::time::Duration = std::time::Duration::from_millis(2000);
    const RECALL_BUDGET: std::time::Duration = std::time::Duration::from_millis(1000);
    let (tx, rx) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        store::with_conn(|_conn| {
            let _ = tx.send(());
            std::thread::sleep(WRITE_HOLD);
            Ok(())
        })
        .unwrap();
    });
    rx.recv().expect("writer acquired the write lock");

    let started = std::time::Instant::now();
    let under_contention = search::brain_search("quokka telemetry", None, None, 4).unwrap();
    let elapsed = started.elapsed();
    assert!(
        under_contention
            .iter()
            .any(|hit| hit.doc.source_id == "readconn-doc"),
        "read-path recall must still return the seeded doc under write contention"
    );
    assert!(
        elapsed < RECALL_BUDGET,
        "recall must not queue behind the {WRITE_HOLD:?} write lock: took {elapsed:?}"
    );
    writer.join().expect("writer thread finished");
}

/// Regression: terms of 2-3 bytes ("git", "k8s", "ai") used to be dropped by
/// the FTS sanitizer, producing an empty query and an invisible brain section
/// for exactly the short tool names users recall most.
#[test]
fn fts_matches_short_keywords() {
    let _db = init_test_db();
    let id = store::upsert_doc(
        DocSource::Note,
        "n-short",
        "git rebase workflow",
        "use git rebase -i and force push with lease; ai pairing notes for k8s",
        100,
    )
    .unwrap();
    for query in ["git", "k8s", "ai", "git rebase"] {
        let hits = store::fts_search(query, 10).unwrap();
        assert!(hits.contains(&id), "short keyword {query:?} should match");
    }
}

#[test]
fn emoji_only_query_matches_via_substring_fallback() {
    let _db = init_test_db();
    let id = store::upsert_doc(
        DocSource::Note,
        "n-emoji",
        "🚀 launch checklist",
        "ship the release, post the announcement",
        100,
    )
    .unwrap();
    // The tokenizer drops the emoji, so the FTS leg alone finds nothing…
    assert!(store::fts_search("🚀", 10).unwrap().is_empty());
    // …but full search recalls it via the substring fallback.
    let hits = search::brain_search("🚀", None, None, 10).unwrap();
    assert!(
        hits.iter().any(|h| h.doc.id == id),
        "emoji-only query should recall the doc via substring fallback"
    );
    // LIKE wildcards in the query must stay literal, not match everything.
    let id2 = store::upsert_doc(
        DocSource::Note,
        "n-percent",
        "100% coverage plan",
        "we promised 100% on the parser",
        100,
    )
    .unwrap();
    let hits = store::substring_search("100%", 10).unwrap();
    assert!(hits.contains(&id2), "escaped % should match literally");
    let hits = store::substring_search("100%zzz", 10).unwrap();
    assert!(
        hits.is_empty(),
        "wildcard characters must not act as wildcards"
    );
}

fn inbox_item(id: i64) -> inbox::InboxItem {
    inbox::InboxItem {
        id,
        kind: InboxKind::Commitment,
        title: format!("item {id}"),
        detail: String::new(),
        source: "note".to_string(),
        source_id: format!("n-{id}"),
        created_at: id,
        resolved_at: None,
    }
}

/// F8 regression: a curator insert landing mid-session must not displace the
/// row under the user's cursor — kept items hold position, resolved items
/// drop out, and new items append below the visible ones.
#[test]
fn inbox_stable_merge_keeps_visible_order_and_appends_new_items() {
    let current = vec![inbox_item(1), inbox_item(2), inbox_item(3)];
    // Fresh read: 3 resolved elsewhere, 9 is a brand-new curator insert that
    // a newest-first reload would pin at the very top.
    let fresh = vec![inbox_item(9), inbox_item(2), inbox_item(1)];
    let merged = inbox::stable_merge_open_inbox(&current, fresh);
    let ids: Vec<i64> = merged.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![1, 2, 9]);
}

#[test]
fn inbox_stable_merge_from_empty_takes_fresh_order() {
    let fresh = vec![inbox_item(9), inbox_item(2)];
    let merged = inbox::stable_merge_open_inbox(&[], fresh);
    let ids: Vec<i64> = merged.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![9, 2]);
}

#[test]
fn fts_finds_doc_by_content_and_respects_deletion() {
    let _db = init_test_db();
    let id = store::upsert_doc(
        DocSource::Note,
        "n-fts",
        "Postgres tricks",
        "use EXPLAIN ANALYZE to find slow queries",
        100,
    )
    .unwrap();
    let hits = store::fts_search("explain analyze slow", 10).unwrap();
    assert!(hits.contains(&id), "fts should match content terms");
    store::remove_doc(DocSource::Note, "n-fts").unwrap();
    let hits = store::fts_search("explain analyze slow", 10).unwrap();
    assert!(!hits.contains(&id), "fts must drop deleted docs");
}

#[test]
fn embedding_roundtrip_and_staleness() {
    let _db = init_test_db();
    let v1 = "version one of a note long enough to embed semantically";
    let v2 = "version two of a note long enough to embed semantically";
    let id = store::upsert_doc(DocSource::Note, "n-embed", "T", v1, 100).unwrap();
    let pending = store::docs_needing_embedding("model-a", 50).unwrap();
    assert!(
        pending.iter().any(|d| d.id == id),
        "new doc needs embedding"
    );
    store::store_embedding(id, "model-a", "T", v1, &[0.6, 0.8]).unwrap();
    let pending = store::docs_needing_embedding("model-a", 50).unwrap();
    assert!(
        !pending.iter().any(|d| d.id == id),
        "embedded doc is current"
    );
    // Content change invalidates by hash.
    store::upsert_doc(DocSource::Note, "n-embed", "T", v2, 200).unwrap();
    let pending = store::docs_needing_embedding("model-a", 50).unwrap();
    assert!(
        pending.iter().any(|d| d.id == id),
        "changed doc needs re-embed"
    );
    // Different model invalidates everything.
    store::store_embedding(id, "model-a", "T", v2, &[1.0, 0.0]).unwrap();
    let pending = store::docs_needing_embedding("model-b", 50).unwrap();
    assert!(
        pending.iter().any(|d| d.id == id),
        "model swap needs re-embed"
    );
    let loaded = store::load_embeddings("model-a").unwrap();
    let vec = &loaded.iter().find(|(i, _)| *i == id).unwrap().1;
    assert_eq!(vec, &vec![1.0, 0.0], "blob roundtrip preserves f32s");
}

#[test]
fn cosine_orders_by_similarity() {
    let embeddings = vec![
        (1, vec![1.0, 0.0]),
        (2, vec![0.0, 1.0]),
        (3, vec![0.9, 0.1]),
    ];
    let top = cosine_top_ids(&[1.0, 0.0], &embeddings, 2);
    assert_eq!(top, vec![1, 3]);
}

#[test]
fn cosine_dedupes_chunks_keeping_best_per_doc() {
    // Doc 1 has two chunks: one weak, one strong; doc 2 one medium chunk.
    // The doc's score is its BEST chunk — one strong passage in a long day
    // page must outrank a diffuse single-vector match.
    let embeddings = vec![
        (1, vec![0.1, 0.9]),
        (1, vec![1.0, 0.0]),
        (2, vec![0.7, 0.3]),
    ];
    let top = cosine_top_ids(&[1.0, 0.0], &embeddings, 10);
    assert_eq!(top, vec![1, 2], "best chunk wins and doc ids are unique");
}

#[test]
fn cosine_drops_candidates_below_similarity_floor() {
    let embeddings = vec![(1, vec![0.39, 0.0]), (2, vec![0.41, 0.0])];
    let top = cosine_top_ids(&[1.0, 0.0], &embeddings, 10);
    assert_eq!(top, vec![2], "unused slots must not admit weak matches");
}

#[test]
fn chunked_embeddings_roundtrip_and_staleness() {
    let _db = init_test_db();
    let long_content = "alpha section about rust gpui internals. ".repeat(40);
    let id = store::upsert_doc(DocSource::Note, "n-chunked", "T", &long_content, 100).unwrap();
    let chunk_vecs = vec![(0usize, vec![1.0, 0.0]), (1800usize, vec![0.0, 1.0])];
    store::store_chunk_embeddings(id, "model-a", "T", &long_content, &chunk_vecs).unwrap();

    // Both chunks load for cosine; doc no longer pending.
    let loaded = store::load_embeddings("model-a").unwrap();
    let mine: Vec<_> = loaded.iter().filter(|(i, _)| *i == id).collect();
    assert_eq!(mine.len(), 2, "one row per chunk");
    let pending = store::docs_needing_embedding("model-a", 500).unwrap();
    assert!(
        !pending.iter().any(|d| d.id == id),
        "chunked doc is current"
    );

    // Content change invalidates ALL chunks via the doc-level hash.
    let changed = "changed content long enough to stay in the semantic leg";
    store::upsert_doc(DocSource::Note, "n-chunked", "T", changed, 200).unwrap();
    let pending = store::docs_needing_embedding("model-a", 500).unwrap();
    assert!(pending.iter().any(|d| d.id == id), "stale chunks re-embed");

    // Re-storing replaces the old chunk set atomically.
    store::store_chunk_embeddings(id, "model-a", "T", changed, &[(0, vec![0.5, 0.5])]).unwrap();
    let loaded = store::load_embeddings("model-a").unwrap();
    let mine: Vec<_> = loaded.iter().filter(|(i, _)| *i == id).collect();
    assert_eq!(mine.len(), 1, "stale chunk rows are deleted on re-store");
}

#[test]
fn rrf_prefers_docs_ranked_by_both_systems() {
    let docs = vec![doc(1, "a", ""), doc(2, "b", ""), doc(3, "c", "")];
    // doc 2 is mid-ranked by both; doc 1 only lexical; doc 3 only semantic.
    let ranked = fuse_ranks(&[1, 2], &[3, 2], &[], &docs, 10);
    assert_eq!(ranked[0].0, 2, "agreement between rankers wins");
}

#[test]
fn signals_boost_matching_docs() {
    let docs = vec![
        doc(1, "random note", "nothing relevant"),
        doc(2, "youtube strategy", "thumbnails and hooks"),
    ];
    let no_boost = fuse_ranks(&[1, 2], &[], &[], &docs, 10);
    assert_eq!(no_boost[0].0, 1);
    let boosted = fuse_ranks(&[1, 2], &[], &[("youtube".to_string(), 6)], &docs, 10);
    assert_eq!(boosted[0].0, 2, "attention signal should re-rank");
}

#[test]
fn signals_cannot_hoist_a_bottom_ranked_doc_over_a_clean_rank_one_doc() {
    let ids: Vec<i64> = (1..=100).collect();
    let docs = vec![
        doc(1, "clean rank one", "directly relevant"),
        doc(100, "all signal topics", "alpha beta gamma delta epsilon"),
    ];
    let topics = ["alpha", "beta", "gamma", "delta", "epsilon"]
        .into_iter()
        .map(|topic| (topic.to_string(), 100))
        .collect::<Vec<_>>();
    let ranked = fuse_ranks(&ids, &ids, &topics, &docs, 100);
    assert_eq!(ranked[0].0, 1, "signals must remain a bounded tiebreaker");
}

#[test]
fn load_embeddings_excludes_short_content_docs() {
    let _db = init_test_db();
    let real_content = "substantial semantic content that clears the minimum length gate";
    let empty =
        store::upsert_doc(DocSource::Note, "empty-semantic", "Untitled Note", "", 100).unwrap();
    let tiny = store::upsert_doc(
        DocSource::Note,
        "tiny-semantic",
        "Chat",
        "clipstays too",
        100,
    )
    .unwrap();
    let real = store::upsert_doc(
        DocSource::Note,
        "real-semantic",
        "Real note",
        real_content,
        100,
    )
    .unwrap();
    store::store_embedding(
        empty,
        "model-short-filter",
        "Untitled Note",
        "",
        &[1.0, 0.0],
    )
    .unwrap();
    store::store_embedding(
        tiny,
        "model-short-filter",
        "Chat",
        "clipstays too",
        &[1.0, 0.0],
    )
    .unwrap();
    store::store_embedding(
        real,
        "model-short-filter",
        "Real note",
        real_content,
        &[0.9, 0.1],
    )
    .unwrap();

    let loaded = store::load_embeddings("model-short-filter").unwrap();
    assert!(!loaded.iter().any(|(id, _)| *id == empty));
    assert!(!loaded.iter().any(|(id, _)| *id == tiny));
    assert!(loaded.iter().any(|(id, _)| *id == real));
    let pending = store::docs_needing_embedding("another-model", 50).unwrap();
    assert!(!pending.iter().any(|doc| doc.id == empty));
    assert!(!pending.iter().any(|doc| doc.id == tiny));
    assert!(pending.iter().any(|doc| doc.id == real));
}

#[test]
fn signal_recording_and_aggregation() {
    let _db = init_test_db();
    store::record_signal("script kit", 2, "ask").unwrap();
    store::record_signal("Script Kit", 1, "chat").unwrap();
    store::record_signal("", 5, "ask").unwrap(); // ignored
    let signals = store::recent_signals(50).unwrap();
    let aggregated = aggregate_signals(&signals);
    let entry = aggregated.iter().find(|(t, _)| t == "script kit");
    assert!(
        entry.is_some_and(|(_, w)| *w >= 3),
        "weights accumulate case-insensitively"
    );
}

#[test]
fn aggregate_orders_by_weight() {
    let signals = vec![
        BrainSignal {
            topic: "alpha".into(),
            weight: 1,
            source: "ask".into(),
            created_at: 0,
        },
        BrainSignal {
            topic: "beta".into(),
            weight: 5,
            source: "ask".into(),
            created_at: 0,
        },
        BrainSignal {
            topic: "alpha".into(),
            weight: 1,
            source: "chat".into(),
            created_at: 0,
        },
    ];
    let aggregated = aggregate_signals(&signals);
    assert_eq!(aggregated[0].0, "beta");
}

#[test]
fn topic_extraction_skips_stopwords_and_keeps_pairs() {
    let topics = extract_topics("How do I configure the YouTube thumbnail pipeline?");
    assert!(topics.iter().any(|t| t.contains("youtube")));
    assert!(!topics.iter().any(|t| t == "how" || t == "the"));
}

#[test]
fn topic_extraction_drops_conversational_filler() {
    // The exact failure mode that filled the inbox with "else"/"again":
    // a throwaway ask made of filler words must yield zero topics.
    let topics = extract_topics("can you do that again for something else a second time");
    assert!(
        topics.is_empty(),
        "filler-only ask must not record topics, got {topics:?}"
    );
    // Filler next to a real subject keeps the subject, drops the filler.
    let topics = extract_topics("try the thumbnail pipeline again");
    assert!(topics.iter().any(|t| t == "thumbnail pipeline"));
    assert!(!topics.iter().any(|t| t.contains("again")));
}

#[test]
fn substantive_topic_gate_accepts_subjects_and_rejects_filler() {
    use super::indexer::is_substantive_topic;
    for junk in ["again", "else", "second", "Something Else", "  ", "the"] {
        assert!(!is_substantive_topic(junk), "{junk:?} should be rejected");
    }
    for real in [
        "second brain",
        "build script",
        "YouTube pipeline",
        "egghead",
    ] {
        assert!(is_substantive_topic(real), "{real:?} should be accepted");
    }
}

#[test]
fn chat_turn_ingestion_is_idempotent_and_searchable() {
    let _db = init_test_db();
    super::ingest_chat_turn(
        "thread-x",
        0,
        "how do I deploy the egghead site",
        "Use vercel --prod.",
    )
    .unwrap();
    super::ingest_chat_turn(
        "thread-x",
        0,
        "how do I deploy the egghead site",
        "Use vercel --prod.",
    )
    .unwrap();
    let hits = store::fts_search("egghead deploy", 10).unwrap();
    let docs = store::get_docs_by_ids(&hits).unwrap();
    let matching: Vec<_> = docs
        .iter()
        .filter(|d| d.source == DocSource::ChatTurn && d.source_id == "thread-x#0")
        .collect();
    assert_eq!(matching.len(), 1, "same turn ingested once");
}

#[test]
fn chat_turn_source_id_format_is_stable() {
    assert_eq!(
        super::indexer::chat_turn_source_id("thread-x", 2),
        "thread-x#2"
    );
}

#[test]
fn retain_docs_forgets_deleted_sources() {
    let _db = init_test_db();
    // Uses the Clipboard source to cover deletion sync without coupling this
    // destructive path to the Note-heavy fixtures above.
    store::upsert_doc(
        DocSource::Clipboard,
        "clip-keep",
        "Keeper",
        "clipstays",
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::Clipboard,
        "clip-gone",
        "Goner",
        "clipleaves",
        100,
    )
    .unwrap();
    store::upsert_doc(
        DocSource::ChatTurn,
        "chat-keep-ret",
        "Chat",
        "clipstays too",
        100,
    )
    .unwrap();
    let removed = store::retain_docs(DocSource::Clipboard, &["clip-keep".to_string()]).unwrap();
    assert!(removed >= 1);
    let hits = store::fts_search("clipleaves", 10).unwrap();
    let docs = store::get_docs_by_ids(&hits).unwrap();
    assert!(
        !docs.iter().any(|d| d.source_id == "clip-gone"),
        "deleted source must be forgotten"
    );
    let chat_hits = store::fts_search("clipstays", 20).unwrap();
    let chat_docs = store::get_docs_by_ids(&chat_hits).unwrap();
    assert!(
        chat_docs.iter().any(|d| d.source_id == "chat-keep-ret"),
        "other sources untouched"
    );
}

#[test]
fn prune_ages_out_old_journals_but_keeps_fresh_data() {
    let _db = init_test_db();
    let now = chrono::Utc::now().timestamp();
    // An ancient daily journal (well past the 90-day window)...
    store::upsert_doc(
        DocSource::Activity,
        "activity:2020-01-01",
        "Activity journal 2020-01-01",
        "prunable ancient action",
        now - 200 * 86_400,
    )
    .unwrap();
    // ...and the focus review, which shares the source but must survive
    // regardless of age (it's keyed without the `activity:` day prefix).
    store::upsert_doc(
        DocSource::Activity,
        "focus-review",
        "Focus review",
        "current focus distillation",
        now - 200 * 86_400,
    )
    .unwrap();
    store::record_signal("prune-test-topic", 1, "test").unwrap();

    let (journals, _signals, _inbox) = store::prune_ambient_data_at(now).unwrap();
    assert!(journals >= 1, "ancient journal should be pruned");
    assert!(
        store::get_doc(DocSource::Activity, "activity:2020-01-01")
            .unwrap()
            .is_none(),
        "ancient journal must be gone"
    );
    assert!(
        store::get_doc(DocSource::Activity, "focus-review")
            .unwrap()
            .is_some(),
        "focus review is exempt from journal pruning"
    );
    // Fresh signals survive a prune at the current time (guards against an
    // inverted cutoff wiping everything).
    let signals = store::recent_signals(500).unwrap();
    assert!(
        signals.iter().any(|s| s.topic == "prune-test-topic"),
        "fresh signals must survive pruning"
    );
}

#[test]
fn brain_status_resource_reports_health() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 13, 8, 0, 0).unwrap();

    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: "private status day body".to_string(),
            },
        )
        .expect("append status day page");
    let long = (0..250)
        .map(|index| format!("statusfragment{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    substrate
        .write_fragment(now, "Status Fixture", "scriptkit://status/fragment", &long)
        .expect("write status fragment");
    let note_id = NoteId::new();
    substrate
        .write_document(
            &substrate.paths().note_file("status-note"),
            &BrainFrontmatter::new(note_id, now, now),
            "# Status Note\n\nprivate status note body",
        )
        .expect("write status note");
    sync_notes_with_substrate(&substrate).expect("sync status notes");
    sync_day_pages_with_substrate(&substrate).expect("sync status day pages");
    sync_fragments_with_substrate(&substrate).expect("sync status fragments");
    store::meta_set("last_index_cycle", "1780000000").expect("set index heartbeat");
    super::health::record_health(&super::health::BrainHealth {
        last_cycle_started_unix: Some(1_780_000_000),
        last_cycle_finished_unix: Some(1_780_000_004),
        last_cycle_ok: Some(true),
        last_error: None,
        docs_total: 3,
        docs_pending_embedding: 3,
        recall_mode: "lexical-only".to_string(),
        embedder_alive: false,
    })
    .expect("record status health snapshot");

    let (mime, body) = super::resources::read_brain_resource("kit://brain").unwrap();
    assert_eq!(mime, "application/json");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["docs"], 3, "status fixtures indexed");
    assert_eq!(value["docsBySource"]["day_page"], 1);
    assert_eq!(value["docsBySource"]["fragment"], 1);
    assert_eq!(value["docsBySource"]["note"], 1);
    assert!(value.get("embedHelperFound").is_some(), "helper presence");
    let health = &value["health"];
    assert_eq!(health["last_cycle_ok"], true, "cycle outcome surfaced");
    assert!(health["last_error"].is_null(), "no error on a clean cycle");
    assert_eq!(health["docs_total"], 3, "doc count in health block");
    assert_eq!(health["docs_pending_embedding"], 3);
    assert_eq!(
        health["recall_mode"], "lexical-only",
        "recall mode surfaced in health block"
    );
    assert_eq!(health["embedder_alive"], false);
    assert_eq!(value["lastIndexCycle"], 1780000000, "indexer heartbeat");
    assert_eq!(value["ftsVersion"], "2", "fts migration recorded");
    assert!(value["dbSizeBytes"].as_u64().unwrap_or(0) > 0, "db on disk");
    assert_eq!(value["canonicalRoots"]["brain"], "~/.scriptkit/brain");
    assert_eq!(value["indexStore"], "~/.scriptkit/db/brain.sqlite");
    assert!(
        !body.contains("private status day body")
            && !body.contains("private status note body")
            && !body.contains("statusfragment42"),
        "status resource should report counts/health, not dump memory content"
    );
}

#[test]
fn health_snapshot_round_trips_all_fields() {
    let _db = init_test_db();
    let health = super::health::BrainHealth {
        last_cycle_started_unix: Some(1_780_000_000),
        last_cycle_finished_unix: Some(1_780_000_005),
        last_cycle_ok: Some(false),
        last_error: Some("embed helper missing".to_string()),
        docs_total: 42,
        docs_pending_embedding: 7,
        recall_mode: "lexical-only".to_string(),
        embedder_alive: false,
    };
    super::health::record_health(&health).expect("record health");
    let read = super::health::read_health().expect("health snapshot present");
    assert_eq!(read, health, "every field round-trips through the meta KV");
}

#[test]
fn health_error_snapshot_is_replaced_by_later_ok() {
    let _db = init_test_db();
    let failed = super::health::BrainHealth {
        last_cycle_started_unix: Some(1_780_000_000),
        last_cycle_finished_unix: Some(1_780_000_005),
        last_cycle_ok: Some(false),
        last_error: Some("cycle blew up".to_string()),
        docs_total: 3,
        docs_pending_embedding: 3,
        recall_mode: "lexical-only".to_string(),
        embedder_alive: false,
    };
    super::health::record_health(&failed).expect("record failing snapshot");
    let recovered = super::health::BrainHealth {
        last_cycle_started_unix: Some(1_780_000_100),
        last_cycle_finished_unix: Some(1_780_000_106),
        last_cycle_ok: Some(true),
        last_error: None,
        docs_total: 4,
        docs_pending_embedding: 0,
        recall_mode: "semantic".to_string(),
        embedder_alive: true,
    };
    super::health::record_health(&recovered).expect("record ok snapshot");
    let read = super::health::read_health().expect("health snapshot present");
    assert_eq!(read.last_cycle_ok, Some(true), "latest snapshot wins");
    assert_eq!(read.last_error, None, "prior error cleared on success");
    assert_eq!(read, recovered);
}

#[test]
fn activity_journal_appends_newest_first_and_recalls() {
    let _db = init_test_db();
    store::append_activity("searched files for \"png\" and opened CleanShot.png").unwrap();
    store::append_activity("ran script kill-port").unwrap();
    let hits = store::fts_search("what was the last thing I searched for", 10).unwrap();
    let docs = store::get_docs_by_ids(&hits).unwrap();
    let journal = docs
        .iter()
        .find(|d| d.source == DocSource::Activity)
        .expect("activity journal should match a 'searched' question");
    // Newest first: the script run line must appear before the search line.
    let script_pos = journal.content.find("kill-port").unwrap();
    let search_pos = journal.content.find("CleanShot").unwrap();
    assert!(script_pos < search_pos, "journal must be newest-first");
    // One doc per day, not one per event.
    let all =
        store::get_docs_by_ids(&store::fts_search("kill-port CleanShot", 50).unwrap()).unwrap();
    let journals: Vec<_> = all
        .iter()
        .filter(|d| d.source == DocSource::Activity)
        .collect();
    assert_eq!(journals.len(), 1);
    // And recall renders it. Assert on the journal's rendered header rather
    // than a specific line: concurrent suite tests that emit ambient search
    // signals (e.g. input-history selection learning) append newer lines to
    // the same shared day journal, which can push this test's lines below
    // the 700-char recall excerpt cap.
    let block = super::recall_context_block("what did I search for recently").unwrap();
    assert!(
        block.is_some_and(|b| b.contains("[Activity journal]")),
        "recall must render the activity journal doc"
    );
}

#[test]
fn recall_context_block_formats_and_caps() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::Note,
        "n-recall",
        "Egghead publish checklist",
        "vercel --prod then update the course index",
        100,
    )
    .unwrap();
    let block = super::recall_context_block("egghead publish checklist").unwrap();
    let block = block.expect("expected recall content");
    assert!(block.contains("Brain recall"));
    assert!(block.contains("Egghead publish checklist"));
    assert!(block.len() <= super::BRAIN_CONTEXT_MAX_CHARS + 200);
    // Irrelevant queries return None, not noise.
    let none = super::recall_context_block("zzqx unrelated nonsense").unwrap();
    assert!(none.is_none());
}

#[test]
fn file_derived_day_page_recall_context_block() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 6, 13, 16, 45, 0)
        .unwrap();

    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: "The calico-lighthouse handoff port is 49217.".to_string(),
            },
        )
        .expect("append day page fact");
    sync_day_pages_with_substrate(&substrate).expect("sync day page from file");

    let block = super::recall_context_block("calico-lighthouse handoff port")
        .expect("recall context")
        .expect("day page recall block");
    assert!(block.contains("[Day Page] Day Page 2026-06-13"));
    assert!(block.contains("calico-lighthouse"));
    assert!(block.contains("49217"));
}

#[test]
fn recall_file_source_refresh_indexes_notes_and_day_pages_together() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 6, 13, 16, 45, 0)
        .unwrap();
    let day_token = "day-violet-bridge-49217";
    let note_token = "note-violet-bridge-8841";

    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: format!("The violet bridge day gate is {day_token}."),
            },
        )
        .expect("append day fact");
    let note_id = NoteId::new();
    substrate
        .write_document(
            &substrate.paths().note_file("violet-bridge-note"),
            &BrainFrontmatter::new(note_id, now, now),
            &format!("# Violet Bridge Note\n\nThe violet bridge note gate is {note_token}."),
        )
        .expect("write note fact");

    let receipt = sync_file_sources_for_recall_with_substrate(&substrate);
    assert_eq!(receipt.failed_sources, Vec::<&'static str>::new());
    assert!(receipt.day_pages >= 1);
    assert!(receipt.notes >= 1);
    assert_eq!(
        store::meta_get("last_index_cycle").unwrap(),
        None,
        "recall refresh must not masquerade as a full index cycle"
    );

    let block = super::recall_context_block("violet bridge gate")
        .expect("recall context")
        .expect("recall block");
    assert!(block.contains("[Day Page]"), "{block}");
    assert!(block.contains("[Note]"), "{block}");
    assert!(block.contains(day_token), "{block}");
    assert!(block.contains(note_token), "{block}");
}

/// After corruption recovery the existing file-source sync must repopulate
/// canonical markdown so recall heals without waiting for embeddings or a
/// later cycle. The process-wide test DB can receive unrelated documents from
/// parallel tests, so this proves absence and recovery of one unique day fact
/// instead of assuming the entire shared fixture stays globally empty.
#[test]
fn recovered_empty_index_repopulates_from_file_sources() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc.with_ymd_and_hms(2026, 7, 1, 12, 0, 0).unwrap();
    let marker = "recovered-index heal marker RCV-7788";
    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: marker.to_string(),
            },
        )
        .expect("append day fact");

    let before: i64 = store::with_conn(|conn| {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM brain_docs WHERE instr(content, ?1) > 0",
            [marker],
            |r| r.get(0),
        )?)
    })
    .expect("count marker docs before rebuild");
    assert_eq!(before, 0, "recovered fact is absent before file sync");

    let receipt = sync_file_sources_for_recall_with_substrate(&substrate);
    assert_eq!(receipt.failed_sources, Vec::<&'static str>::new());
    assert!(receipt.day_pages >= 1, "day file synced into a fresh index");

    let day_docs: i64 = store::with_conn(|conn| {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM brain_docs WHERE source = 'day_page' AND instr(content, ?1) > 0",
            [marker],
            |r| r.get(0),
        )?)
    })
    .expect("count day docs after rebuild");
    assert_eq!(day_docs, 1, "fresh index repopulated the unique day fact");
}

#[test]
fn brain_recall_resource_reads_file_derived_day_page() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 14, 9, 15, 0).unwrap();

    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: "The quartz-harbor rollback token is QH-8841.".to_string(),
            },
        )
        .expect("append resource fact");
    sync_day_pages_with_substrate(&substrate).expect("sync day page from file");

    let (mime, body) = super::resources::read_brain_resource(
        "kit://brain/recall?q=quartz-harbor%20rollback%20token",
    )
    .expect("brain recall resource");
    assert_eq!(mime, "text/markdown");
    assert!(body.contains("[Day Page] Day Page 2026-06-14"));
    assert!(body.contains("quartz-harbor"));
    assert!(body.contains("QH-8841"));
}

#[test]
fn brain_recall_json_resource_reports_source_refs() {
    let _db = init_test_db();
    store::upsert_doc_with_canonical_path(
        DocSource::Note,
        "source-ref-note",
        "Source ref note",
        "The source-ref recall token is SR-49217.\nSecond line for range proof.",
        100,
        Some("brain/notes/source-ref-note.md"),
    )
    .unwrap();

    let (mime, body) = super::resources::read_brain_resource(
        "kit://brain/recall?q=source-ref%20SR-49217&format=json",
    )
    .expect("brain recall json resource");
    assert_eq!(mime, "application/json");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    let hit = value["hits"]
        .as_array()
        .expect("hits array")
        .iter()
        .find(|hit| hit["sourceId"] == "source-ref-note")
        .expect("source ref hit");
    assert_eq!(hit["source"], "note");
    assert_eq!(hit["citationUri"], "brain://note/source-ref-note");
    assert_eq!(hit["canonicalPath"], "brain/notes/source-ref-note.md");
    assert!(hit["lineStart"].as_u64().unwrap_or_default() >= 1);
    assert!(hit["lineEnd"].as_u64().unwrap_or_default() >= 1);
    assert!(hit["excerpt"].as_str().unwrap().contains("SR-49217"));
}

#[test]
fn brain_recall_json_resource_reports_file_derived_note_canonical_path() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 17, 12, 0, 0).unwrap();
    let note_id = NoteId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let note_frontmatter = BrainFrontmatter::new(note_id, now, now);
    substrate
        .write_document(
            &substrate.paths().note_file("provenance-note"),
            &note_frontmatter,
            "# Provenance note\n\nThe qmd note path token is NOTE-PATH-8841.\nSecond line.",
        )
        .expect("write canonical note");

    sync_notes_with_substrate(&substrate).expect("sync canonical note");

    let (mime, body) =
        super::resources::read_brain_resource("kit://brain/recall?q=NOTE-PATH-8841&format=json")
            .expect("brain recall json resource");
    assert_eq!(mime, "application/json");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    let hit = value["hits"]
        .as_array()
        .expect("hits array")
        .iter()
        .find(|hit| hit["sourceId"] == note_id.to_string())
        .expect("file-derived note hit");
    assert_eq!(hit["source"], "note");
    assert_eq!(hit["citationUri"], format!("brain://note/{note_id}"));
    assert_eq!(hit["canonicalPath"], "brain/notes/provenance-note.md");
    assert!(hit["lineStart"].as_u64().unwrap_or_default() >= 1);
    assert!(hit["lineEnd"].as_u64().unwrap_or_default() >= 1);
    assert!(hit["excerpt"].as_str().unwrap().contains("NOTE-PATH-8841"));
    let metadata = serde_json::to_string(hit).unwrap();
    assert!(!metadata.contains("/Users/"));
    assert!(!metadata.contains(".scriptkit/db/brain.sqlite"));
}

#[test]
fn brain_doc_resource_gets_by_source_id_and_line_range() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::DayPage,
        "2026-06-15",
        "Day Page 2026-06-15",
        "line one\nline two has DOC-8841\nline three has DOC-8842\nline four",
        200,
    )
    .unwrap();

    let (mime, body) = super::resources::read_brain_resource(
        "kit://brain/doc?source=day_page&sourceId=2026-06-15&lines=2-3&format=json",
    )
    .expect("brain doc json resource");
    assert_eq!(mime, "application/json");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["doc"]["source"], "day_page");
    assert_eq!(value["doc"]["sourceId"], "2026-06-15");
    assert_eq!(value["doc"]["citationUri"], "brain://day_page/2026-06-15");
    assert_eq!(value["doc"]["canonicalPath"], "brain/days/2026-06-15.md");
    assert_eq!(value["doc"]["lineStart"], 2);
    assert_eq!(value["doc"]["lineEnd"], 3);
    let content = value["doc"]["content"].as_str().unwrap();
    assert!(!content.contains("line one"));
    assert!(content.contains("DOC-8841"));
    assert!(content.contains("DOC-8842"));
    assert!(!content.contains("line four"));

    store::upsert_doc_with_canonical_path(
        DocSource::Note,
        "doc-note",
        "Doc Note",
        "note line one\nnote line two has DOC-NOTE-8841\nnote line three",
        201,
        Some("brain/notes/doc-note.md"),
    )
    .unwrap();
    let (note_mime, note_body) = super::resources::read_brain_resource(
        "kit://brain/doc?source=note&sourceId=doc-note&lines=2-2&format=json",
    )
    .expect("brain note doc json resource");
    assert_eq!(note_mime, "application/json");
    let note_value: serde_json::Value = serde_json::from_str(&note_body).unwrap();
    assert_eq!(note_value["doc"]["source"], "note");
    assert_eq!(note_value["doc"]["sourceId"], "doc-note");
    assert_eq!(note_value["doc"]["citationUri"], "brain://note/doc-note");
    assert_eq!(
        note_value["doc"]["canonicalPath"],
        "brain/notes/doc-note.md"
    );
    assert_eq!(note_value["doc"]["lineStart"], 2);
    assert_eq!(note_value["doc"]["lineEnd"], 2);
    let note_content = note_value["doc"]["content"].as_str().unwrap();
    assert!(!note_content.contains("note line one"));
    assert!(note_content.contains("DOC-NOTE-8841"));
    assert!(!note_content.contains("note line three"));
}

#[test]
fn brain_doc_resource_rejects_invalid_line_range_without_body_leak() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::DayPage,
        "2026-06-16",
        "Day Page 2026-06-16",
        "private line one\nprivate line two SECRET-DOC-8843",
        210,
    )
    .unwrap();

    let error = super::resources::read_brain_resource(
        "kit://brain/doc?source=day_page&sourceId=2026-06-16&lines=0-2&format=json",
    )
    .expect_err("invalid line range should fail closed");
    assert!(error.contains("invalid kit://brain/doc lines parameter"));
    assert!(!error.contains("SECRET-DOC-8843"));
}

#[test]
fn brain_doc_resource_keeps_source_id_raw_but_encodes_citation_uri() {
    let _db = init_test_db();
    let source_id = "thread x#2/part";
    store::upsert_doc(
        DocSource::ChatTurn,
        source_id,
        "Thread source ref",
        "chat turn body with encoded citation proof",
        220,
    )
    .unwrap();

    let (mime, body) = super::resources::read_brain_resource(
        "kit://brain/doc?source=chat_turn&sourceId=thread%20x%232%2Fpart&format=json",
    )
    .expect("brain doc json resource");
    assert_eq!(mime, "application/json");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["doc"]["sourceId"], source_id);
    assert_eq!(
        value["doc"]["citationUri"],
        "brain://chat_turn/thread%20x%232%2Fpart"
    );
}

#[test]
fn brain_multi_get_resource_preserves_order_and_missing_receipts() {
    let _db = init_test_db();
    store::upsert_doc_with_canonical_path(
        DocSource::Note,
        "multi-a",
        "Multi A",
        "alpha token",
        100,
        Some("brain/notes/multi-a.md"),
    )
    .unwrap();
    store::upsert_doc(DocSource::Fragment, "multi-b", "Multi B", "beta token", 200).unwrap();

    let (mime, body) = super::resources::read_brain_resource(
        "kit://brain/docs?refs=fragment:multi-b,note:multi-a,note:missing",
    )
    .expect("brain docs resource");
    assert_eq!(mime, "application/json");
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    let docs = value["docs"].as_array().expect("docs array");
    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0]["ref"], "fragment:multi-b");
    assert_eq!(docs[0]["found"], true);
    assert_eq!(docs[0]["doc"]["source"], "fragment");
    assert_eq!(docs[1]["ref"], "note:multi-a");
    assert_eq!(docs[1]["found"], true);
    assert_eq!(docs[1]["doc"]["source"], "note");
    assert_eq!(docs[1]["doc"]["canonicalPath"], "brain/notes/multi-a.md");
    assert_eq!(docs[2]["ref"], "note:missing");
    assert_eq!(docs[2]["found"], false);
    assert_eq!(docs[2]["error"], "not_found");
}

#[test]
fn brain_context_block_includes_source_refs_without_private_status_dump() {
    let _db = init_test_db();
    store::upsert_doc(
        DocSource::Fragment,
        "context-ref-fragment",
        "Context ref fragment",
        "The context-ref proof token is CR-2771.",
        300,
    )
    .unwrap();

    let block = super::recall_context_block("context-ref proof token")
        .expect("recall context")
        .expect("context block");
    assert!(block.contains("Source: brain://fragment/context-ref-fragment"));
    assert!(block.contains("path: brain/fragments/context-ref-fragment.md"));
    assert!(block.contains("lines: 1-1"));
    assert!(!block.contains("/Users/"));
    assert!(!block.contains(".scriptkit/db/brain.sqlite"));
}

#[test]
fn inbox_insert_dedupes_resolves_and_orders() {
    let _db = init_test_db();
    let first = inbox::insert_inbox_item(
        InboxKind::Commitment,
        "Inbox-test ship the gizmo",
        "said in chat yesterday",
        "chat_turn",
        "thread-inbox#1",
    )
    .unwrap();
    assert!(first, "first insert lands");
    // Same kind + title modulo case/whitespace → deduped.
    let dup = inbox::insert_inbox_item(
        InboxKind::Commitment,
        "  inbox-test SHIP   the gizmo ",
        "different detail",
        "chat_turn",
        "thread-inbox#2",
    )
    .unwrap();
    assert!(!dup, "normalized kind|title must dedupe");
    // Same title, different kind → distinct item.
    let other_kind = inbox::insert_inbox_item(
        InboxKind::Question,
        "Inbox-test ship the gizmo",
        "is this even a question",
        "chat_turn",
        "",
    )
    .unwrap();
    assert!(other_kind, "kind participates in the dedupe key");
    // Blank titles are rejected, not stored.
    assert!(!inbox::insert_inbox_item(InboxKind::Drift, "   ", "", "", "").unwrap());

    let open = inbox::open_inbox_items(1000).unwrap();
    let pos_question = open
        .iter()
        .position(|i| i.kind == InboxKind::Question && i.title == "Inbox-test ship the gizmo")
        .expect("question item open");
    let pos_commitment = open
        .iter()
        .position(|i| i.kind == InboxKind::Commitment && i.title == "Inbox-test ship the gizmo")
        .expect("commitment item open");
    assert!(pos_question < pos_commitment, "newest first");
    let commitment = &open[pos_commitment];
    assert_eq!(commitment.source, "chat_turn");
    assert_eq!(commitment.source_id, "thread-inbox#1", "first insert wins");
    assert!(commitment.resolved_at.is_none());
    assert!(inbox::count_open_inbox().unwrap() >= 2);

    assert!(inbox::resolve_inbox_item(commitment.id).unwrap());
    assert!(
        !inbox::resolve_inbox_item(commitment.id).unwrap(),
        "second resolve is a no-op"
    );
    let open = inbox::open_inbox_items(1000).unwrap();
    assert!(
        !open.iter().any(|i| i.id == commitment.id),
        "resolved item leaves the open list"
    );
}

#[test]
fn inbox_kind_roundtrips_and_labels() {
    for kind in [
        InboxKind::Commitment,
        InboxKind::Question,
        InboxKind::Drift,
        InboxKind::StalePin,
    ] {
        assert_eq!(InboxKind::parse(kind.as_str()), Some(kind));
        assert!(!kind.label().is_empty());
    }
    assert_eq!(InboxKind::parse("nonsense"), None);
    assert_eq!(InboxKind::StalePin.label(), "Stale Pin");
    assert_eq!(InboxKind::Question.label(), "Open Question");
    assert_eq!(InboxKind::Drift.label(), "Drifting");
}

#[test]
fn prune_removes_only_old_resolved_inbox_items() {
    let _db = init_test_db();
    let now = chrono::Utc::now().timestamp();
    // Assert end states (rows gone/kept) rather than coupling the test to the
    // exact count returned by one prune call.
    inbox::insert_inbox_item(InboxKind::Drift, "Inbox-prune ancient resolved", "", "", "").unwrap();
    inbox::insert_inbox_item(InboxKind::Drift, "Inbox-prune fresh resolved", "", "", "").unwrap();
    inbox::insert_inbox_item(InboxKind::Drift, "Inbox-prune still open", "", "", "").unwrap();
    let open = inbox::open_inbox_items(1000).unwrap();
    let id_of = |title: &str| {
        open.iter()
            .find(|i| i.title == title)
            .unwrap_or_else(|| panic!("missing {title}"))
            .id
    };
    // Backdate one resolution past the 30-day retention window.
    assert!(
        inbox::resolve_inbox_item_at(id_of("Inbox-prune ancient resolved"), now - 40 * 86_400)
            .unwrap()
    );
    assert!(inbox::resolve_inbox_item_at(id_of("Inbox-prune fresh resolved"), now).unwrap());

    let (_journals, _signals, _inbox_removed) = store::prune_ambient_data_at(now).unwrap();

    // The aged resolved row is gone: its dedupe hash is freed, so the same
    // title inserts again.
    assert!(
        inbox::insert_inbox_item(InboxKind::Drift, "Inbox-prune ancient resolved", "", "", "")
            .unwrap(),
        "aged resolved item must be deleted"
    );
    // The freshly resolved row survives: its hash still blocks re-insertion.
    assert!(
        !inbox::insert_inbox_item(InboxKind::Drift, "Inbox-prune fresh resolved", "", "", "")
            .unwrap(),
        "recently resolved item must be retained"
    );
    // Open items are never pruned regardless of age.
    assert!(inbox::open_inbox_items(1000)
        .unwrap()
        .iter()
        .any(|i| i.title == "Inbox-prune still open"));
}

#[test]
fn inbox_response_prompt_includes_detail_and_source_context() {
    let _db = init_test_db();
    let source_id = "thread-inbox-response-prompt#2";
    store::upsert_doc(
        DocSource::ChatTurn,
        source_id,
        "Original chat turn",
        "User said the second option should be shipped after checking the migration notes.",
        200,
    )
    .unwrap();
    assert!(
        inbox::insert_inbox_item(
            InboxKind::Question,
            "Clarify the second option",
            "The conversation left open whether second means the script or the build script.",
            DocSource::ChatTurn.as_str(),
            source_id,
        )
        .unwrap(),
        "inbox item should insert"
    );

    let item = inbox::open_inbox_items(1000)
        .unwrap()
        .into_iter()
        .find(|item| item.source_id == source_id)
        .expect("inserted inbox item");
    let prompt = inbox::response_prompt_for_inbox_item(&item);

    assert!(prompt.contains("Follow up on this Brain Inbox item."));
    assert!(prompt.contains("- Type: Open Question"));
    assert!(prompt.contains("- Title: Clarify the second option"));
    assert!(prompt.contains(
        "- Details: The conversation left open whether second means the script or the build script."
    ));
    assert!(prompt.contains("- Source: chat_turn"));
    assert!(prompt.contains("- Source ID: thread-inbox-response-prompt#2"));
    assert!(prompt.contains("- Source title: Original chat turn"));
    assert!(prompt.contains("User said the second option should be shipped"));
    assert!(prompt.contains("Use the inbox details and source context above"));
}

#[test]
fn parse_inbox_extraction_accepts_strict_json() {
    let raw = r#"{"commitments":[{"title":"Ship the inbox","detail":"Promised in chat.","sourceId":"thread-9#2"}],"questions":[{"title":"Which DB?","detail":"Never answered.","sourceId":""}],"drift":[{"title":"YouTube pipeline","detail":"No activity in a week."}]}"#;
    let parsed = curator::parse_inbox_extraction(raw).unwrap();
    assert_eq!(parsed.commitments.len(), 1);
    assert_eq!(parsed.commitments[0].title, "Ship the inbox");
    assert_eq!(parsed.commitments[0].source_id, "thread-9#2");
    assert_eq!(parsed.questions.len(), 1);
    assert_eq!(parsed.questions[0].source_id, "");
    assert_eq!(parsed.drift.len(), 1);
    assert_eq!(parsed.drift[0].source_id, "", "drift omits sourceId");
}

#[test]
fn parse_inbox_extraction_tolerates_fences_and_prose() {
    let raw = "Sure! Here is the extraction you asked for:\n```json\n{\"commitments\":[],\"questions\":[{\"title\":\"What port?\",\"detail\":\"\",\"sourceId\":\"t#0\"}],\"drift\":[]}\n```\nLet me know if you need anything else.";
    let parsed = curator::parse_inbox_extraction(raw).unwrap();
    assert_eq!(parsed.questions.len(), 1);
    assert_eq!(parsed.questions[0].title, "What port?");
    assert!(parsed.commitments.is_empty());
}

#[test]
fn parse_inbox_extraction_rejects_garbage_and_caps_items() {
    assert!(
        curator::parse_inbox_extraction("no json anywhere").is_err(),
        "prose without an object is an error"
    );
    assert!(
        curator::parse_inbox_extraction("{this is not json}").is_err(),
        "malformed object is an error"
    );
    assert!(curator::parse_inbox_extraction("} backwards {").is_err());
    // Blank titles dropped; categories capped at 8.
    let many = (0..12)
        .map(|i| format!("{{\"title\":\"q{i}\",\"detail\":\"\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    let raw = format!(
        "{{\"commitments\":[{{\"title\":\"   \",\"detail\":\"blank title\"}}],\"questions\":[{many}],\"drift\":[]}}"
    );
    let parsed = curator::parse_inbox_extraction(&raw).unwrap();
    assert!(parsed.commitments.is_empty(), "blank titles skipped");
    assert_eq!(parsed.questions.len(), 8, "per-category cap enforced");
}

#[test]
fn parse_inbox_extraction_gates_generic_drift_titles() {
    let raw = r#"{"commitments":[],"questions":[{"title":"What's going on today?","detail":"","sourceId":"t#0"}],"drift":[{"title":"else","detail":"attention 8, no activity"},{"title":"again","detail":""},{"title":"second brain","detail":"real subject survives"}]}"#;
    let parsed = curator::parse_inbox_extraction(raw).unwrap();
    let drift_titles: Vec<&str> = parsed.drift.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        drift_titles,
        vec!["second brain"],
        "generic drift titles are filtered, substantive ones kept"
    );
    // The gate applies to drift only — a question phrased entirely in
    // generic words is still a genuine user question.
    assert_eq!(parsed.questions.len(), 1);
}

#[test]
fn stale_pin_detection_flags_old_pinned_notes_only() {
    let now = chrono::Utc::now();
    let old = now - chrono::Duration::days(30);
    let fresh = now - chrono::Duration::days(2);
    let notes = vec![
        ("Old pinned".to_string(), old, true, "note-old".to_string()),
        (
            "Fresh pinned".to_string(),
            fresh,
            true,
            "note-fresh".to_string(),
        ),
        (
            "Old unpinned".to_string(),
            old,
            false,
            "note-unpinned".to_string(),
        ),
        ("   ".to_string(), old, true, "note-untitled".to_string()),
    ];
    let stale = curator::stale_pins_from(&notes, now);
    assert_eq!(stale.len(), 2, "only old pinned notes qualify");
    let old_pin = stale
        .iter()
        .find(|(_, _, id)| id == "note-old")
        .expect("old pinned note flagged");
    assert_eq!(old_pin.0, "Old pinned");
    assert_eq!(
        old_pin.1,
        format!("Pinned but untouched since {}", old.format("%Y-%m-%d"))
    );
    let untitled = stale
        .iter()
        .find(|(_, _, id)| id == "note-untitled")
        .expect("untitled pinned note flagged");
    assert_eq!(untitled.0, "Pinned note", "blank title gets fallback");
    assert!(
        !stale
            .iter()
            .any(|(_, _, id)| id == "note-fresh" || id == "note-unpinned"),
        "fresh or unpinned notes are never flagged"
    );
}

// ============================================================
// Telegram bridge (pure core; no network)
// ============================================================

#[test]
fn telegram_parse_updates_extracts_messages_and_next_offset() {
    // 2 text messages + 1 non-message update (an edit): the messages route,
    // and the offset still advances past the non-message update.
    let body = r#"{
        "ok": true,
        "result": [
            {
                "update_id": 101,
                "message": {
                    "message_id": 7,
                    "from": {"id": 42, "is_bot": false, "first_name": "John"},
                    "chat": {"id": 42, "type": "private"},
                    "date": 1700000000,
                    "text": "what did I work on this week?"
                }
            },
            {
                "update_id": 102,
                "edited_message": {"message_id": 7, "text": "ignored edit"}
            },
            {
                "update_id": 103,
                "message": {
                    "message_id": 8,
                    "from": {"id": 7},
                    "chat": {"id": 7, "type": "private"},
                    "date": 1700000001,
                    "text": "/start"
                }
            }
        ]
    }"#;
    let updates = telegram::parse_updates_json(body).expect("realistic getUpdates parses");
    assert_eq!(updates.len(), 3, "non-message updates still parse");
    let messages = telegram::incoming_messages(&updates);
    assert_eq!(messages.len(), 2, "only text messages become incoming");
    assert_eq!(messages[0].update_id, 101);
    assert_eq!(messages[0].chat_id, 42);
    assert_eq!(messages[0].user_id, 42);
    assert_eq!(messages[0].text, "what did I work on this week?");
    assert_eq!(messages[1].text, "/start");
    assert_eq!(
        telegram::next_offset(&updates),
        Some(103),
        "offset covers the non-message update too"
    );
    assert_eq!(telegram::next_offset(&[]), None, "empty batch keeps offset");
}

#[test]
fn telegram_parse_tolerates_partial_messages_and_rejects_bad_envelopes() {
    assert!(telegram::parse_updates_json("not json").is_err());
    assert!(telegram::parse_updates_json(r#"{"ok": false, "result": []}"#).is_err());
    // A message missing text/from is skipped, but its id still advances.
    let updates = telegram::parse_updates_json(
        r#"{"ok": true, "result": [{"update_id": 5, "message": {"chat": {"id": 1}}}]}"#,
    )
    .expect("partial message parses");
    assert!(telegram::incoming_messages(&updates).is_empty());
    assert_eq!(telegram::next_offset(&updates), Some(5));
}

#[test]
fn telegram_authorization_requires_allowlist_membership() {
    assert!(telegram::is_authorized(42, &[42, 7]));
    assert!(!telegram::is_authorized(9, &[42, 7]));
    assert!(
        !telegram::is_authorized(42, &[]),
        "empty allowlist authorizes nobody"
    );
}

#[test]
fn telegram_answer_prompt_grounds_question_in_context() {
    let prompt =
        telegram::build_answer_prompt("what is project bluefin?", "[memory] bluefin notes");
    assert!(prompt.contains("what is project bluefin?"));
    assert!(prompt.contains("[memory] bluefin notes"));
    assert!(
        prompt.contains("ONLY"),
        "prompt restricts to memory context"
    );
    assert!(
        prompt.contains("no markdown"),
        "telegram replies are plain text"
    );
}

#[test]
fn telegram_replies_trim_and_truncate_at_cap() {
    assert_eq!(telegram::truncate_reply("  short answer  "), "short answer");
    let long = "x".repeat(5_000);
    let capped = telegram::truncate_reply(&long);
    assert_eq!(capped.chars().count(), 4_000, "capped under telegram limit");
    assert!(capped.ends_with('…'), "truncation is marked");
}

#[test]
fn telegram_redaction_strips_token_from_error_text() {
    let redacted = telegram::redact_token(
        "123456:ABC-secret",
        "https://api.telegram.org/bot123456:ABC-secret/getUpdates: timeout",
    );
    assert!(!redacted.contains("123456:ABC-secret"));
    assert!(redacted.contains("<redacted-token>"));
    assert_eq!(telegram::redact_token("", "unchanged"), "unchanged");
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedDocSnapshot {
    source: DocSource,
    source_id: String,
    title: String,
    content: String,
    canonical_path: Option<String>,
}

fn snapshot_doc(source: DocSource, source_id: &str) -> Option<IndexedDocSnapshot> {
    store::get_doc(source, source_id)
        .expect("doc lookup")
        .map(|doc| IndexedDocSnapshot {
            source: doc.source,
            source_id: doc.source_id,
            title: doc.title,
            content: doc.content,
            canonical_path: doc.canonical_path,
        })
}

/// Simulate "delete brain.sqlite" for only this test's docs. Targeted deletes
/// let the rebuild contract exercise the same FK cascade production uses when
/// a file source disappears.
fn clear_brain_docs_for_rebuild_test(docs: &[(DocSource, &str)]) {
    for (source, source_id) in docs {
        store::remove_doc(*source, source_id).expect("clear rebuild test doc");
    }
}

fn chunk_embedding_count_for_doc(doc_id: i64) -> i64 {
    store::with_conn(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM brain_chunk_embeddings WHERE doc_id = ?1",
            [doc_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    })
    .expect("count chunk embeddings for doc")
}

fn orphan_chunk_embedding_count() -> i64 {
    store::with_conn(|conn| {
        conn.query_row(
            "SELECT COUNT(*)
             FROM brain_chunk_embeddings e
             LEFT JOIN brain_docs d ON d.id = e.doc_id
             WHERE d.id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
    })
    .expect("count orphan chunk embeddings")
}

fn test_substrate(base: &std::path::Path) -> BrainSubstrate {
    BrainSubstrate::with_timezone(base, chrono_tz::UTC)
}

#[test]
fn file_sources_sync_day_page_fragment_and_forget_trashed_note() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc.with_ymd_and_hms(2026, 6, 11, 10, 0, 0).unwrap();

    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: "capture for brain".to_string(),
            },
        )
        .expect("append day capture");

    let long = (0..250)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    substrate
        .write_fragment(now, "clipboard", "scriptkit://clipboard/entry-t7", &long)
        .expect("write fragment");

    let note_id = NoteId::new();
    let note_path = substrate.paths().note_file("t7-note");
    substrate
        .write_document(
            &note_path,
            &BrainFrontmatter::new(note_id, now, now),
            "# T7 Note\n\nbrain indexer note body",
        )
        .expect("write note");

    sync_notes_with_substrate(&substrate).expect("sync notes");
    sync_day_pages_with_substrate(&substrate).expect("sync day pages");
    sync_fragments_with_substrate(&substrate).expect("sync fragments");

    let day_doc = store::get_doc(DocSource::DayPage, "2026-06-11")
        .expect("day page lookup")
        .expect("day page doc");
    assert!(day_doc.content.contains("capture for brain"));
    assert_eq!(day_doc.title, "Day Page 2026-06-11");

    // Shared test DB: other parallel tests may also index fragments, so find
    // this test's doc by provenance rather than asserting a global count.
    let fragment_docs =
        store::recent_docs_for_source(DocSource::Fragment, 0, 100).expect("fragments");
    let fragment_doc = fragment_docs
        .iter()
        .find(|doc| doc.content.contains("scriptkit://clipboard/entry-t7"))
        .expect("fragment doc indexed with provenance");
    assert_eq!(
        fragment_doc.canonical_path.as_deref(),
        Some(format!("brain/fragments/{}.md", fragment_doc.source_id).as_str())
    );
    assert!(fragment_doc
        .title
        .contains("scriptkit://clipboard/entry-t7"));

    let note_doc = store::get_doc(DocSource::Note, &note_id.to_string())
        .expect("note lookup")
        .expect("note doc");
    assert!(note_doc.content.contains("brain indexer note body"));
    assert_eq!(
        note_doc.canonical_path.as_deref(),
        Some("brain/notes/t7-note.md")
    );

    substrate.trash(&note_path).expect("trash note");
    sync_notes_with_substrate(&substrate).expect("re-sync notes after trash");
    assert!(
        store::get_doc(DocSource::Note, &note_id.to_string())
            .expect("note lookup after trash")
            .is_none(),
        "trashed note must be forgotten"
    );
}

#[test]
fn index_capture_now_makes_day_capture_recallable_without_a_cycle() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc.with_ymd_and_hms(2026, 7, 1, 16, 45, 0).unwrap();
    // Globally-unique single token so FTS + provenance counting can't collide
    // with day pages other parallel tests write into the shared brain DB.
    let marker = "zebrafishinstantrecall";
    let source_id = now.date_naive().to_string();

    // A capture writes the file only. Under cfg(test) the substrate's
    // auto-index wrapper is a no-op, so nothing is in brain_docs yet — this
    // isolates `index_capture_now` as the thing under test.
    substrate
        .append_to_day(
            now,
            DayEntry::Capture {
                text: format!("captured line about {marker}"),
            },
        )
        .expect("append day capture");
    assert!(
        store::get_doc(DocSource::DayPage, &source_id)
            .unwrap()
            .is_none(),
        "capture write alone must not index under cfg(test)"
    );

    // Synchronous index: FTS must see the marker WITHOUT running any cycle.
    let day_path = substrate.paths().day_page(now.date_naive());
    super::indexer::index_capture_now(&day_path);

    let doc = store::get_doc(DocSource::DayPage, &source_id)
        .unwrap()
        .expect("day page doc row exists immediately after index_capture_now");
    let hits = store::fts_search(marker, 10).unwrap();
    assert!(
        hits.contains(&doc.id),
        "FTS must match the captured marker with no indexer cycle"
    );

    // Identity parity: the periodic sync over the same substrate must upsert
    // the SAME row. Drift would leave a second day_page doc carrying the same
    // marker. Count by marker (not globally) to stay robust under parallel
    // tests that write their own day pages into this shared DB.
    let day_docs_with_marker = || {
        store::recent_docs_for_source(DocSource::DayPage, 0, 1000)
            .expect("day docs")
            .into_iter()
            .filter(|doc| doc.content.contains(marker))
            .count()
    };
    assert_eq!(
        day_docs_with_marker(),
        1,
        "index_capture_now created exactly one day page row"
    );

    let synced = sync_day_pages_with_substrate(&substrate).expect("sync day pages");
    assert_eq!(synced, 1, "sync sees exactly the one day file");
    assert_eq!(
        day_docs_with_marker(),
        1,
        "sync reused the row index_capture_now created — no identity drift / duplicate"
    );
    let after = store::get_doc(DocSource::DayPage, &source_id)
        .unwrap()
        .expect("day page row still present after sync");
    assert_eq!(
        after.id, doc.id,
        "sync upserted the same row id, proving shared identity"
    );
}

#[test]
fn brain_rebuild_from_files_restores_day_fragment_and_note_sources() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let substrate = test_substrate(&tmp.path().join("brain"));
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 6, 12, 14, 30, 0)
        .unwrap();

    substrate
        .append_to_day(
            now,
            DayEntry::Task {
                body: "rebuild parity task".to_string(),
                tags: vec!["t7".to_string()],
                due: None,
            },
        )
        .expect("append task");
    let long = (0..250)
        .map(|index| format!("parity{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    substrate
        .write_fragment(
            now,
            "Slack Paste",
            "scriptkit://clipboard/rebuild-t7",
            &long,
        )
        .expect("write fragment");
    let note_id = NoteId::new();
    let mut note_frontmatter =
        BrainFrontmatter::new(note_id, now, now).with_source("scriptkit://note/rebuild-t7");
    note_frontmatter.tags = vec!["qmd".to_string(), "brain".to_string()];
    note_frontmatter.aliases = vec!["Rebuild Alias".to_string()];
    note_frontmatter.pinned = true;
    substrate
        .write_document(
            &substrate.paths().note_file("rebuild-note"),
            &note_frontmatter,
            "# Rebuild note\n\nparity body with [[Linked Note]]",
        )
        .expect("write note");

    sync_notes_with_substrate(&substrate).expect("sync notes");
    sync_day_pages_with_substrate(&substrate).expect("sync day pages");
    sync_fragments_with_substrate(&substrate).expect("sync fragments");

    let fragment_source_id = store::recent_docs_for_source(DocSource::Fragment, 0, 100)
        .expect("fragment docs")
        .into_iter()
        .find(|doc| doc.content.contains("scriptkit://clipboard/rebuild-t7"))
        .expect("fragment indexed before rebuild")
        .source_id
        .clone();
    let note_source_id = note_id.to_string();
    let day_source_id = "2026-06-12".to_string();

    let before = [
        snapshot_doc(DocSource::DayPage, &day_source_id).expect("day page indexed"),
        snapshot_doc(DocSource::Fragment, &fragment_source_id).expect("fragment indexed"),
        snapshot_doc(DocSource::Note, &note_source_id).expect("note indexed"),
    ];
    assert!(before[1]
        .content
        .contains("Provenance: scriptkit://clipboard/rebuild-t7"));
    assert!(before[2]
        .content
        .contains("parity body with [[Linked Note]]"));
    assert_eq!(
        before[0].canonical_path.as_deref(),
        Some("brain/days/2026-06-12.md")
    );
    assert_eq!(
        before[1].canonical_path.as_deref(),
        Some(format!("brain/fragments/{fragment_source_id}.md").as_str())
    );
    assert_eq!(
        before[2].canonical_path.as_deref(),
        Some("brain/notes/rebuild-note.md")
    );
    let before_doc_ids = [
        store::get_doc(DocSource::DayPage, &day_source_id)
            .unwrap()
            .unwrap()
            .id,
        store::get_doc(DocSource::Fragment, &fragment_source_id)
            .unwrap()
            .unwrap()
            .id,
        store::get_doc(DocSource::Note, &note_source_id)
            .unwrap()
            .unwrap()
            .id,
    ];
    for doc_id in before_doc_ids {
        store::store_embedding(doc_id, "rebuild-model", "T", "body", &[1.0, 0.0])
            .expect("store rebuild-test embedding");
        assert_eq!(
            chunk_embedding_count_for_doc(doc_id),
            1,
            "fixture doc should have a chunk embedding before rebuild delete"
        );
    }

    clear_brain_docs_for_rebuild_test(&[
        (DocSource::DayPage, &day_source_id),
        (DocSource::Fragment, &fragment_source_id),
        (DocSource::Note, &note_source_id),
    ]);
    assert!(
        snapshot_doc(DocSource::DayPage, &day_source_id).is_none()
            && snapshot_doc(DocSource::Fragment, &fragment_source_id).is_none()
            && snapshot_doc(DocSource::Note, &note_source_id).is_none(),
        "rebuild test docs must be gone before re-sync"
    );
    for doc_id in before_doc_ids {
        assert_eq!(
            chunk_embedding_count_for_doc(doc_id),
            0,
            "removing file-source docs must cascade chunk embeddings"
        );
    }
    assert_eq!(
        orphan_chunk_embedding_count(),
        0,
        "targeted file-source delete must not leave orphan chunk embeddings"
    );

    sync_notes_with_substrate(&substrate).expect("re-sync notes");
    sync_day_pages_with_substrate(&substrate).expect("re-sync day pages");
    sync_fragments_with_substrate(&substrate).expect("re-sync fragments");

    let after = [
        snapshot_doc(DocSource::DayPage, &day_source_id).expect("day page rebuilt"),
        snapshot_doc(DocSource::Fragment, &fragment_source_id).expect("fragment rebuilt"),
        snapshot_doc(DocSource::Note, &note_source_id).expect("note rebuilt"),
    ];
    assert_eq!(before, after, "rebuild must restore indexed file sources");

    let rebuilt_fragment_id = store::get_doc(DocSource::Fragment, &fragment_source_id)
        .unwrap()
        .unwrap()
        .id;
    store::store_embedding(
        rebuilt_fragment_id,
        "rebuild-model",
        "Fragment",
        &after[1].content,
        &[0.0, 1.0],
    )
    .expect("store rebuilt fragment embedding");
    assert_eq!(chunk_embedding_count_for_doc(rebuilt_fragment_id), 1);

    std::fs::remove_file(substrate.paths().fragment_file(&fragment_source_id))
        .expect("delete canonical fragment file");
    sync_fragments_with_substrate(&substrate).expect("forget deleted fragment file");
    assert!(
        snapshot_doc(DocSource::Fragment, &fragment_source_id).is_none(),
        "deleting a canonical fragment file must remove the derived brain doc"
    );
    assert_eq!(
        chunk_embedding_count_for_doc(rebuilt_fragment_id),
        0,
        "forgetting a deleted fragment file must cascade chunk embeddings"
    );
    assert_eq!(
        orphan_chunk_embedding_count(),
        0,
        "forgetting a deleted file-source doc must not leave orphan chunks"
    );
}

#[test]
fn capture_stores_sync_into_brain_and_respect_deletion() {
    let _db = init_test_db();
    let tmp = tempfile::TempDir::new().expect("tempdir");

    // Link + snippet through their real stores (same writers the `;` capture
    // path uses). Todos live on day pages (indexed in T7), not capture stores.
    let link = match crate::menu_syntax::capture::parse_capture(
        ";link https://example.com Example description:Docs",
    ) {
        crate::menu_syntax::capture::CaptureParse::Ok(invocation) => invocation,
        _ => panic!("link capture should parse"),
    };
    let draft = crate::menu_syntax::parse_link_scriptlet_capture(&link).expect("link draft");
    crate::scriptlets::link_markdown_store::upsert_link_section(tmp.path(), &draft)
        .expect("write link");
    let snippet =
        match crate::menu_syntax::capture::parse_capture(";snippet Hello there keyword:hi name:Hi")
        {
            crate::menu_syntax::capture::CaptureParse::Ok(invocation) => invocation,
            _ => panic!("snippet capture should parse"),
        };
    let draft =
        crate::menu_syntax::parse_snippet_scriptlet_capture(&snippet).expect("snippet draft");
    crate::scriptlets::snippet_markdown_store::upsert_snippet_section(tmp.path(), &draft)
        .expect("write snippet");

    let synced = super::indexer::sync_capture_stores_in_sk_path(tmp.path()).expect("sync");
    assert_eq!(synced, 2, "link + snippet");

    let docs = store::recent_docs_for_source(DocSource::Capture, 0, 10).unwrap();
    assert!(docs.iter().any(|d| d.title == "Link: Example"));
    assert!(docs.iter().any(|d| d.title.starts_with("Snippet:")));
}

/// End-to-end embed cycle with the embedder injected: a long doc rides one
/// batch call as multiple chunks, vectors split back per doc, and the docs
/// stop reporting as pending. Guards the chunk/batch/split bookkeeping in
/// `indexer::embed_pending_with` without the helper subprocess.
#[test]
fn embed_cycle_chunks_long_docs_and_clears_pending() {
    let _db = init_test_db();
    let model = "model-embed-cycle";
    let long_content = "## Section\n\nlong day page text about embedding cycles. ".repeat(160); // ~8.6 KB
    let long_id = store::upsert_doc(
        DocSource::DayPage,
        "d-embed-cycle-long",
        "Long",
        &long_content,
        100,
    )
    .unwrap();
    let short_id = store::upsert_doc(
        DocSource::Note,
        "n-embed-cycle-short",
        "Short",
        "a short note that still clears the semantic minimum length gate",
        100,
    )
    .unwrap();

    let mut calls: Vec<usize> = Vec::new();
    let embedded = super::indexer::embed_pending_with(model, |texts| {
        calls.push(texts.len());
        // Deterministic fake embedder: unit vector per text.
        Ok(texts.iter().map(|_| vec![1.0f32, 0.0]).collect())
    })
    .unwrap();
    assert!(embedded >= 2, "both docs embed in the cycle: {embedded}");

    let loaded = store::load_embeddings(model).unwrap();
    let long_chunks = loaded.iter().filter(|(id, _)| *id == long_id).count();
    let short_chunks = loaded.iter().filter(|(id, _)| *id == short_id).count();
    assert!(
        long_chunks > 1,
        "long doc must store multiple chunks: {long_chunks}"
    );
    assert_eq!(short_chunks, 1, "short doc stays single-chunk");
    assert_eq!(
        orphan_chunk_embedding_count(),
        0,
        "embedding cycle must not leave rows without matching brain_docs"
    );
    assert!(
        calls.iter().sum::<usize>() >= long_chunks + short_chunks,
        "all chunks rode the embed batches: {calls:?}"
    );

    let pending = store::docs_needing_embedding(model, 500).unwrap();
    assert!(
        !pending.iter().any(|d| d.id == long_id || d.id == short_id),
        "embedded docs no longer pending"
    );

    // An embedder that returns empty vectors must not spin the cycle.
    store::upsert_doc(
        DocSource::Note,
        "n-embed-cycle-short",
        "Short",
        "a short note v2 that still clears the semantic minimum length gate",
        200,
    )
    .unwrap();
    let embedded = super::indexer::embed_pending_with(model, |texts| {
        Ok(texts.iter().map(|_| Vec::new()).collect())
    })
    .unwrap();
    assert_eq!(embedded, 0, "all-empty vectors store nothing and terminate");
}

/// Embed backoff policy: no skipping until 3 consecutive failures, then a
/// linear cool-down window (3 failures skips 1 cycle, 4 skips 2, ...), and any
/// success (cycles_since_failure reset with failures back to 0) resumes embeds.
#[test]
fn embed_backoff_skips_only_after_threshold_and_scales_linearly() {
    use super::indexer::should_skip_embed;

    // Below threshold: never skip, regardless of cycles since failure.
    assert!(!should_skip_embed(0, 0));
    assert!(!should_skip_embed(1, 0));
    assert!(!should_skip_embed(2, 0));
    assert!(!should_skip_embed(2, 5));

    // 3 consecutive failures: skip exactly the next 1 cycle.
    assert!(
        should_skip_embed(3, 0),
        "first cycle after 3 failures skips"
    );
    assert!(!should_skip_embed(3, 1), "second cycle retries");

    // 4 consecutive failures: skip the next 2 cycles, then retry.
    assert!(should_skip_embed(4, 0));
    assert!(should_skip_embed(4, 1));
    assert!(!should_skip_embed(4, 2));

    // A success resets the counter, so we are never in backoff again.
    assert!(!should_skip_embed(0, 0));
}

/// Concurrent `atomic_append_line` on one file must never drop a line. Before
/// the process-wide write lock this read-modify-write raced: two threads read
/// the same contents, each appended one line, and one overwrite clobbered the
/// other. With the lock every append is serialized, so all 200 lines survive.
#[test]
fn concurrent_atomic_append_line_never_drops_a_line() {
    use super::substrate::io::atomic_append_line;
    use std::collections::HashSet;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("append-race.md");

    const THREADS: usize = 8;
    const PER_THREAD: usize = 25;

    std::thread::scope(|scope| {
        for thread_index in 0..THREADS {
            let path = path.clone();
            scope.spawn(move || {
                for line_index in 0..PER_THREAD {
                    atomic_append_line(&path, &format!("t{thread_index}-l{line_index}"))
                        .expect("atomic append under lock");
                }
            });
        }
    });

    let contents = std::fs::read_to_string(&path).expect("read appended file");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines.len(),
        THREADS * PER_THREAD,
        "every append must survive the read-modify-write race"
    );
    let unique: HashSet<&str> = lines.iter().copied().collect();
    assert_eq!(unique.len(), THREADS * PER_THREAD, "no duplicated lines");
    for thread_index in 0..THREADS {
        for line_index in 0..PER_THREAD {
            let expected = format!("t{thread_index}-l{line_index}");
            assert!(
                unique.contains(expected.as_str()),
                "missing appended line {expected}"
            );
        }
    }
}

//! Hybrid brain search: FTS5 BM25 + vector cosine fused with Reciprocal Rank
//! Fusion, then boosted by recent attention signals.
//!
//! This is the qmd retrieval recipe implemented natively: lexical search
//! catches exact terms, semantic search catches meaning, RRF makes them
//! agree, and signals tilt results toward what John currently cares about.

use super::store::{self, BrainDoc, DocSource};
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Write as _;

const RRF_K: f64 = 60.0;
const SIGNAL_WINDOW: usize = 200;
const SIGNAL_BOOST: f64 = 0.05;
/// Total attention boost a doc can receive, capped at half a rank-1 RRF
/// contribution so signals break ties but can never hoist an unranked doc
/// over a genuinely matching one (Brain-Inbox follow-up turns quote the top
/// drifting topics verbatim and previously won every launcher search).
const MAX_SIGNAL_BOOST: f64 = 0.5 / (RRF_K + 1.0);
/// Semantic-leg floor, measured against the real embeddinggemma index: docs
/// of ≥[`store::MIN_SEMANTIC_CONTENT_CHARS`] content score ≤0.36 cosine for
/// unrelated queries and ≥0.43 for genuine matches, so 0.40 splits the bands
/// with margin. Without a floor the top-N by relative rank qualify for ANY
/// query. Embeddings are unit-normalized, so dot product == cosine.
const MIN_COSINE_SIMILARITY: f32 = 0.40;

#[derive(Debug, Clone)]
pub struct BrainHit {
    pub doc: BrainDoc,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainHitSourceRef {
    pub source: DocSource,
    pub source_id: String,
    pub citation_uri: String,
    pub canonical_path: Option<String>,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
}

fn encode_brain_uri_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                let _ = write!(&mut out, "%{byte:02X}");
            }
        }
    }
    out
}

pub fn source_ref_for_doc(doc: &BrainDoc) -> BrainHitSourceRef {
    let canonical_path = doc.canonical_path.clone().or_else(|| match doc.source {
        DocSource::DayPage => Some(format!("brain/days/{}.md", doc.source_id)),
        DocSource::Fragment => Some(format!("brain/fragments/{}.md", doc.source_id)),
        DocSource::Note
        | DocSource::ChatTurn
        | DocSource::Clipboard
        | DocSource::Activity
        | DocSource::Capture => None,
    });
    let (line_start, line_end) = excerpt_line_range(&doc.content, &excerpt_for_doc(&doc.content));
    BrainHitSourceRef {
        source: doc.source,
        source_id: doc.source_id.clone(),
        citation_uri: format!(
            "brain://{}/{}",
            doc.source.as_str(),
            encode_brain_uri_segment(&doc.source_id)
        ),
        canonical_path,
        line_start,
        line_end,
    }
}

pub fn excerpt_for_doc(content: &str) -> String {
    let mut excerpt: String = content.chars().take(700).collect();
    if content.chars().count() > 700 {
        excerpt.push_str(" …");
    }
    excerpt
}

pub fn excerpt_line_range(content: &str, excerpt: &str) -> (Option<usize>, Option<usize>) {
    if content.is_empty() || excerpt.is_empty() {
        return (None, None);
    }
    let excerpt = excerpt.trim_end_matches(" …");
    let Some(start_byte) = content.find(excerpt) else {
        return (None, None);
    };
    let end_byte = start_byte + excerpt.len();
    let line_start = content[..start_byte]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let line_end = content[..end_byte]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    (Some(line_start), Some(line_end.max(line_start)))
}

/// Rank documents for `query`. `query_vec` is the embedded query (None =>
/// lexical-only). Pure function over the supplied candidate lists — separated
/// from IO for testability.
pub(crate) fn fuse_ranks(
    fts_ids: &[i64],
    vec_ids: &[i64],
    signal_topics: &[(String, i64)],
    docs: &[BrainDoc],
    limit: usize,
) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for (rank, id) in fts_ids.iter().enumerate() {
        *scores.entry(*id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    for (rank, id) in vec_ids.iter().enumerate() {
        *scores.entry(*id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    // Attention boost: docs whose title/content mention a recent signal topic
    // get a nudge proportional to the topic's recent weight.
    if !signal_topics.is_empty() {
        let by_id: HashMap<i64, &BrainDoc> = docs.iter().map(|d| (d.id, d)).collect();
        for (id, score) in scores.iter_mut() {
            if let Some(doc) = by_id.get(id) {
                let haystack = format!(
                    "{} {}",
                    doc.title.to_lowercase(),
                    doc.content.to_lowercase()
                );
                let mut signal_boost = 0.0;
                for (topic, weight) in signal_topics {
                    if haystack.contains(topic.as_str()) {
                        signal_boost += SIGNAL_BOOST * (*weight as f64).min(3.0) / (RRF_K / 10.0);
                    }
                }
                *score += signal_boost.min(MAX_SIGNAL_BOOST);
            }
        }
    }
    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(limit);
    ranked
}

/// Rank doc ids by cosine similarity. `embeddings` carries one row per CHUNK
/// (the same doc id can appear many times); a doc's score is its best chunk,
/// so one strong passage in a long day page outranks a diffuse match.
pub(crate) fn cosine_top_ids(
    query_vec: &[f32],
    embeddings: &[(i64, Vec<f32>)],
    limit: usize,
) -> Vec<i64> {
    let mut best: HashMap<i64, f32> = HashMap::new();
    for (id, v) in embeddings {
        if v.len() != query_vec.len() || v.is_empty() {
            continue;
        }
        let dot: f32 = query_vec.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        if dot < MIN_COSINE_SIMILARITY {
            continue;
        }
        let entry = best.entry(*id).or_insert(f32::NEG_INFINITY);
        if dot > *entry {
            *entry = dot;
        }
    }
    let mut scored: Vec<(i64, f32)> = best.into_iter().collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.truncate(limit);
    scored.into_iter().map(|(id, _)| id).collect()
}

/// Aggregate recent signals into (topic, total weight), strongest first.
pub(crate) fn aggregate_signals(signals: &[store::BrainSignal]) -> Vec<(String, i64)> {
    let mut by_topic: HashMap<String, i64> = HashMap::new();
    for signal in signals {
        *by_topic.entry(signal.topic.clone()).or_default() += signal.weight;
    }
    let mut topics: Vec<(String, i64)> = by_topic
        .into_iter()
        .filter(|(topic, _)| topic.len() > 2)
        .collect();
    topics.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    topics.truncate(16);
    topics
}

/// The main entry point: hybrid search over everything the brain knows.
/// `query_vec` should come from [`super::embedder::BrainEmbedder::embed`];
/// pass `None` for lexical-only (no model on disk, or latency-critical paths).
pub fn brain_search(
    query: &str,
    query_vec: Option<&[f32]>,
    model_id: Option<&str>,
    limit: usize,
) -> Result<Vec<BrainHit>> {
    brain_search_with_lexical_mode(query, query_vec, model_id, limit, LexicalMode::RecallOr)
}

/// Hybrid search for passive launcher results. Its lexical leg requires
/// multi-word terms to occur near each other; all semantic ranking and fusion
/// behavior is shared with [`brain_search`].
pub fn brain_search_proximity(
    query: &str,
    query_vec: Option<&[f32]>,
    model_id: Option<&str>,
    limit: usize,
) -> Result<Vec<BrainHit>> {
    brain_search_with_lexical_mode(query, query_vec, model_id, limit, LexicalMode::Proximity)
}

#[derive(Clone, Copy)]
enum LexicalMode {
    RecallOr,
    Proximity,
}

fn brain_search_with_lexical_mode(
    query: &str,
    query_vec: Option<&[f32]>,
    model_id: Option<&str>,
    limit: usize,
    lexical_mode: LexicalMode,
) -> Result<Vec<BrainHit>> {
    let candidate_limit = limit.max(8) * 4;
    let mut fts_ids = match lexical_mode {
        LexicalMode::RecallOr => store::fts_search(query, candidate_limit)?,
        LexicalMode::Proximity => store::fts_search_proximity(query, candidate_limit)?,
    };
    let should_fallback = match lexical_mode {
        LexicalMode::RecallOr => !query.trim().is_empty(),
        LexicalMode::Proximity => {
            !query.trim().is_empty() && !query.chars().any(char::is_alphanumeric)
        }
    };
    if fts_ids.is_empty() && should_fallback {
        // unicode61 drops emoji/symbol tokens, so FTS can come back empty for
        // text that exists verbatim in docs. Fall back to a substring scan as
        // the lexical leg before giving up.
        fts_ids = match lexical_mode {
            LexicalMode::RecallOr => store::substring_search(query, candidate_limit),
            LexicalMode::Proximity => store::substring_search_proximity(query, candidate_limit),
        }
        .unwrap_or_default();
    }
    let vec_ids = match (query_vec, model_id) {
        (Some(qv), Some(mid)) if !qv.is_empty() => {
            let embeddings = store::load_embeddings(mid)?;
            cosine_top_ids(qv, &embeddings, candidate_limit)
        }
        _ => Vec::new(),
    };
    let mut candidate_ids: Vec<i64> = fts_ids.clone();
    for id in &vec_ids {
        if !candidate_ids.contains(id) {
            candidate_ids.push(*id);
        }
    }
    let docs = store::get_docs_by_ids(&candidate_ids)?;
    let signals = store::recent_signals(SIGNAL_WINDOW).unwrap_or_default();
    let signal_topics = aggregate_signals(&signals);
    // Rank the full candidate pool, then dedupe by content before taking the
    // top `limit`: the same text captured via clipboard, a note, and a chat
    // turn must not crowd distinct memories out of the launcher section.
    let ranked = fuse_ranks(&fts_ids, &vec_ids, &signal_topics, &docs, candidate_limit);
    let by_id: HashMap<i64, BrainDoc> = docs.into_iter().map(|d| (d.id, d)).collect();
    let mut seen_content = std::collections::HashSet::new();
    Ok(ranked
        .into_iter()
        .filter_map(|(id, score)| by_id.get(&id).cloned().map(|doc| BrainHit { doc, score }))
        .filter(|hit| seen_content.insert(store::content_hash(&hit.doc.title, &hit.doc.content)))
        .take(limit)
        .collect())
}

/// Render hits as a compact markdown context block for agent prompts.
/// Hard-capped so retrieval can never blow out a prompt.
pub fn render_context_block(hits: &[BrainHit], max_chars: usize) -> String {
    render_context_block_with_provenance(hits, max_chars)
}

pub fn render_context_block_with_provenance(hits: &[BrainHit], max_chars: usize) -> String {
    if hits.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "## Brain recall (auto-retrieved from the user's local knowledge; \
         treat as background memory, cite naturally when relevant)\n\n",
    );
    for hit in hits {
        let title = if hit.doc.title.trim().is_empty() {
            "(untitled)"
        } else {
            hit.doc.title.trim()
        };
        let excerpt = excerpt_for_doc(&hit.doc.content);
        let source_ref = source_ref_for_doc(&hit.doc);
        let line_text = match (source_ref.line_start, source_ref.line_end) {
            (Some(start), Some(end)) => format!(" · lines: {start}-{end}"),
            _ => String::new(),
        };
        let path_text = source_ref
            .canonical_path
            .as_ref()
            .map(|path| format!(" · path: {path}"))
            .unwrap_or_default();
        let entry = format!(
            "### [{}] {}\nSource: {}{}{} · updated: {}\n{}\n\n",
            hit.doc.source.label(),
            title,
            source_ref.citation_uri,
            line_text,
            path_text,
            hit.doc.updated_at,
            excerpt.trim()
        );
        if out.len() + entry.len() > max_chars {
            break;
        }
        out.push_str(&entry);
    }
    out
}

pub fn recall_hits_json(query: &str, hits: &[BrainHit]) -> serde_json::Value {
    json!({
        "schemaVersion": 1,
        "query": query,
        "hits": hits
            .iter()
            .map(|hit| {
                let source_ref = source_ref_for_doc(&hit.doc);
                json!({
                    "source": hit.doc.source.as_str(),
                    "sourceId": &hit.doc.source_id,
                    "title": &hit.doc.title,
                    "score": hit.score,
                    "updatedAt": hit.doc.updated_at,
                    "citationUri": source_ref.citation_uri,
                    "canonicalPath": source_ref.canonical_path,
                    "lineStart": source_ref.line_start,
                    "lineEnd": source_ref.line_end,
                    "excerpt": excerpt_for_doc(&hit.doc.content),
                })
            })
            .collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(topic: &str, weight: i64) -> store::BrainSignal {
        store::BrainSignal {
            topic: topic.to_owned(),
            weight,
            source: "test".to_owned(),
            created_at: 0,
        }
    }

    #[test]
    fn rrf_tied_scores_have_stable_identity_order() {
        for (lexical, semantic) in [([20, 10], [10, 20]), ([10, 20], [20, 10])] {
            let ranked = fuse_ranks(&lexical, &semantic, &[], &[], 1);
            assert_eq!(ranked.len(), 1);
            assert_eq!(
                ranked[0].0, 10,
                "equal reciprocal-rank scores must prefer the stable lower document identity"
            );
        }
    }

    #[test]
    fn cosine_tied_scores_have_stable_identity_order() {
        for embeddings in [
            vec![(20, vec![1.0, 0.0]), (10, vec![1.0, 0.0])],
            vec![(10, vec![1.0, 0.0]), (20, vec![1.0, 0.0])],
        ] {
            assert_eq!(
                cosine_top_ids(&[1.0, 0.0], &embeddings, 1),
                vec![10],
                "equal semantic scores must select the same document before truncation"
            );
        }
    }

    #[test]
    fn signal_ties_have_stable_topic_order() {
        let signals: Vec<_> = (0..17)
            .rev()
            .map(|index| signal(&format!("topic-{index:02}"), 1))
            .collect();
        let aggregated = aggregate_signals(&signals);
        let expected: Vec<_> = (0..16)
            .map(|index| (format!("topic-{index:02}"), 1))
            .collect();

        assert_eq!(
            aggregated, expected,
            "the 16-topic cap must preserve the same alphabetic identities for tied weights"
        );
    }

    #[test]
    fn ranking_tie_breakers_never_override_stronger_primary_scores() {
        let ranked = fuse_ranks(&[20, 10], &[], &[], &[], 1);
        assert_eq!(ranked[0].0, 20);

        let embeddings = vec![(10, vec![0.8, 0.0]), (20, vec![1.0, 0.0])];
        assert_eq!(cosine_top_ids(&[1.0, 0.0], &embeddings, 1), vec![20]);

        let signals = [signal("alpha", 1), signal("zulu", 5)];
        assert_eq!(aggregate_signals(&signals)[0], ("zulu".to_owned(), 5));
    }
}

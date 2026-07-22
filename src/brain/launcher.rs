//! Root launcher passive "From Your Brain" section plumbing.
//!
//! Mirrors the Notes passive-source pattern (`src/notes/storage.rs`): a small
//! options struct sourced from config, an eligibility gate, and a direct
//! search entry point that maps brain documents into lightweight UI hits.
//!
//! IMPORTANT: this path must never record attention signals — passive search
//! feeding the attention log would self-amplify whatever the user happens to
//! be typing.

use super::store::DocSource;

/// Maximum characters carried into a row excerpt.
const ROOT_BRAIN_EXCERPT_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootBrainSectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
}

impl Default for RootBrainSectionOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 4,
            min_query_chars: 3,
        }
    }
}

/// Options for the pinned "Brain Inbox" section shown at the top of the
/// empty root query. Sourced from config via
/// `UnifiedSearchConfig::brain_inbox_section_options()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootBrainInboxSectionOptions {
    pub enabled: bool,
    pub max_results: usize,
}

impl Default for RootBrainInboxSectionOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 3,
        }
    }
}

/// Subtitle for a Brain Inbox launcher row: `"<kind label> · <detail>"` when
/// the item carries a detail line, otherwise `"<kind label> · <relative age>"`
/// so the row still explains itself ("Commitment · 3d ago").
pub fn root_brain_inbox_subtitle(item: &super::inbox::InboxItem, now: i64) -> String {
    let detail = item
        .detail
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let context = if detail.is_empty() {
        relative_age(now.saturating_sub(item.created_at))
    } else {
        excerpt_for_content(detail)
    };
    format!("{} · {}", item.kind.label(), context)
}

/// Coarse human age for inbox rows ("just now", "5m ago", "3h ago", "2d ago").
fn relative_age(age_secs: i64) -> String {
    let age_secs = age_secs.max(0);
    if age_secs < 60 {
        "just now".to_string()
    } else if age_secs < 3_600 {
        format!("{}m ago", age_secs / 60)
    } else if age_secs < 86_400 {
        format!("{}h ago", age_secs / 3_600)
    } else {
        format!("{}d ago", age_secs / 86_400)
    }
}

/// A brain document projected down to exactly what the launcher row needs.
/// Full document content intentionally stays behind in the store.
#[derive(Debug, Clone)]
pub struct RootBrainSearchHit {
    pub title: String,
    pub excerpt: String,
    pub source_label: &'static str,
    pub source: DocSource,
    pub source_id: String,
}

pub fn root_brain_query_is_eligible(query: &str, options: RootBrainSectionOptions) -> bool {
    let query = query.trim();
    options.enabled
        && !query.contains('\n')
        && crate::scripts::search::query_meets_min_query_chars(query, options.min_query_chars)
}

const PASSIVE_ROOT_BRAIN_STOPWORDS: &[&str] = &[
    "a", "an", "and", "any", "anyway", "are", "as", "at", "be", "been", "being", "but", "by",
    "did", "do", "does", "for", "from", "had", "has", "have", "having", "he", "her", "his", "how",
    "i", "in", "is", "it", "me", "my", "of", "on", "or", "our", "she", "that", "the", "their",
    "this", "to", "was", "we", "were", "what", "when", "where", "which", "who", "why", "with",
    "you", "your",
];

fn passive_root_brain_stopword_or_prefix(term: &str) -> bool {
    if PASSIVE_ROOT_BRAIN_STOPWORDS.contains(&term) {
        return true;
    }

    term.chars().all(|ch| ch.is_ascii_alphabetic())
        && PASSIVE_ROOT_BRAIN_STOPWORDS
            .iter()
            .any(|stopword| term.len() < stopword.len() && stopword.starts_with(term))
}

fn passive_root_brain_term_is_noise(term: &str) -> bool {
    term.is_empty()
        || term
            .chars()
            .all(|ch| ch.is_ascii_punctuation() || ch.is_whitespace())
        || passive_root_brain_stopword_or_prefix(term)
}

fn passive_root_brain_meaningful_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter_map(|term| {
            let term = term
                .trim_matches(|ch: char| ch.is_ascii_punctuation())
                .to_lowercase();
            if passive_root_brain_term_is_noise(&term) {
                None
            } else {
                Some(term)
            }
        })
        .collect()
}

/// Passive root Brain rows should not fire for filler-only natural-language
/// phrases like "What is this anyway?". Explicit `brain:` queries intentionally
/// use `root_brain_query_is_eligible` directly and remain permissive.
pub fn root_brain_passive_search_text(
    query: &str,
    options: RootBrainSectionOptions,
) -> Option<String> {
    if !root_brain_query_is_eligible(query, options) {
        return None;
    }
    let terms = passive_root_brain_meaningful_terms(query);
    if terms.is_empty() {
        None
    } else {
        let search_text = terms.join(" ");
        root_brain_query_is_eligible(&search_text, options).then_some(search_text)
    }
}

pub fn root_brain_passive_query_is_eligible(query: &str, options: RootBrainSectionOptions) -> bool {
    root_brain_passive_search_text(query, options).is_some()
}

/// One shared decision for BOTH brain passes — the sync lexical pass in
/// `filtering_cache.rs` and the async semantic pass in `root_brain_search.rs`
/// — so their eligibility rules cannot drift apart.
///
/// The async pass layers view-state gates (`can_collect`, `max_results > 0`)
/// on top; those are ownership concerns, not query concerns, and stay at the
/// call site. `advanced_query_blocks` is likewise caller-supplied because the
/// two passes intentionally gate on different advanced-query granularity
/// (sync: any advanced query; async: predicates only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootBrainQueryPlan {
    /// The brain section renders nothing and semantic state clears.
    Skip,
    /// Bare explicit `brain:`: show the most recent memories instead of a
    /// blank panel (audit F6). No search runs — recents are not semantic
    /// results, so the async pass stays cleared.
    RecentsOnly,
    /// Run the lexical search now (and the semantic pass async) with this
    /// effective query text: trimmed input for explicit `brain:`, the
    /// stopword-stripped passive text otherwise.
    Search(String),
}

pub fn root_brain_query_plan(
    search_text: &str,
    explicit_brain: bool,
    advanced_query_blocks: bool,
    allow_brain: bool,
    options: RootBrainSectionOptions,
) -> RootBrainQueryPlan {
    if advanced_query_blocks || !allow_brain {
        return RootBrainQueryPlan::Skip;
    }
    let trimmed = search_text.trim();
    if explicit_brain {
        if trimmed.is_empty() {
            return RootBrainQueryPlan::RecentsOnly;
        }
        if root_brain_query_is_eligible(trimmed, options) {
            return RootBrainQueryPlan::Search(trimmed.to_string());
        }
        return RootBrainQueryPlan::Skip;
    }
    match root_brain_passive_search_text(trimmed, options) {
        Some(text) => RootBrainQueryPlan::Search(text),
        None => RootBrainQueryPlan::Skip,
    }
}

/// Search the brain for passive root-launcher rows. Returns an empty list when
/// the section is disabled, the query is too short, or the store errors —
/// passive sources must never surface failures into the launcher.
pub fn search_root_brain_direct(
    query: &str,
    options: &RootBrainSectionOptions,
) -> Vec<RootBrainSearchHit> {
    if !root_brain_query_is_eligible(query, *options) || options.max_results == 0 {
        return Vec::new();
    }

    // The sync per-keystroke pass stays lexical-only (query_vec: None); the
    // async pass ([`search_root_brain_semantic`]) upgrades it to hybrid.
    let fetch_limit = options.max_results.saturating_mul(2);
    let mut hits = map_root_brain_hits(
        super::search::brain_search_proximity(query.trim(), None, None, fetch_limit)
            .unwrap_or_default(),
    );
    hits.truncate(options.max_results);
    hits
}

/// Async hybrid pass for the root launcher: embed the query on the warm
/// indexer thread (hard ~200ms budget), then run hybrid FTS+cosine search.
/// Returns `None` when no embedding model is warm — callers keep the lexical
/// hits. Blocking; call from a background thread only, never the UI thread.
pub fn search_root_brain_semantic(
    query: &str,
    options: &RootBrainSectionOptions,
) -> Option<Vec<RootBrainSearchHit>> {
    if !root_brain_query_is_eligible(query, *options) || options.max_results == 0 {
        return None;
    }
    let query = query.trim();
    let (model_id, query_vec) = super::indexer::embed_query_within_budget(query)?;
    let fetch_limit = options.max_results.saturating_mul(2);
    let mut hits = map_root_brain_hits(
        super::search::brain_search_proximity(
            query,
            Some(&query_vec),
            Some(&model_id),
            fetch_limit,
        )
        .unwrap_or_default(),
    );
    hits.truncate(options.max_results);
    Some(hits)
}

/// Most recent brain docs as launcher rows. Backs the armed-but-empty
/// `brain:` source filter so it shows "what your brain holds" instead of a
/// blank panel. Never errors into the launcher (empty on store failure).
pub fn recent_root_brain_hits(max_results: usize) -> Vec<RootBrainSearchHit> {
    if max_results == 0 {
        return Vec::new();
    }
    // Over-fetch then dedupe by content, mirroring `brain_search`: the same
    // text mirrored from several sources must not fill the recents view.
    let mut seen = std::collections::HashSet::new();
    map_root_brain_hits(
        super::store::recent_docs(max_results.saturating_mul(3).max(8))
            .unwrap_or_default()
            .into_iter()
            .filter(|doc| seen.insert(super::store::content_hash(&doc.title, &doc.content)))
            .take(max_results)
            .map(|doc| super::search::BrainHit { doc, score: 0.0 })
            .collect(),
    )
}

/// Prefer async semantic hits over the sync lexical pass when they were
/// computed for exactly the query the launcher is currently showing.
/// `semantic` is `(stored_query, hits)` from app state; `None` (or a stale
/// stored query) means "use lexical". Pure so it's testable without GPUI.
pub fn semantic_root_brain_hits_for_query(
    current_query: &str,
    semantic: Option<&(String, Vec<RootBrainSearchHit>)>,
    options: &RootBrainSectionOptions,
) -> Option<Vec<RootBrainSearchHit>> {
    if !root_brain_query_is_eligible(current_query, *options) || options.max_results == 0 {
        return None;
    }
    let (stored_query, hits) = semantic?;
    if stored_query != current_query.trim() {
        return None;
    }
    let mut hits = hits.clone();
    hits.truncate(options.max_results);
    Some(hits)
}

/// Map full brain hits down to launcher rows. Shared by the sync lexical and
/// async semantic passes so both produce identical row shapes.
pub(crate) fn map_root_brain_hits(hits: Vec<super::search::BrainHit>) -> Vec<RootBrainSearchHit> {
    let mut seen_rendered = std::collections::HashSet::new();
    hits.into_iter()
        .map(|hit| {
            let excerpt = match hit.doc.source {
                DocSource::Activity => excerpt_for_activity_content(&hit.doc.content),
                _ => excerpt_for_content(&hit.doc.content),
            };
            let title = if hit.doc.title.trim().is_empty() {
                hit.doc.source.label().to_string()
            } else {
                hit.doc.title.clone()
            };
            RootBrainSearchHit {
                title,
                excerpt,
                source_label: hit.doc.source.label(),
                source: hit.doc.source,
                source_id: hit.doc.source_id.clone(),
            }
        })
        .filter(|hit| seen_rendered.insert((hit.title.clone(), hit.excerpt.clone())))
        .collect()
}

/// Render a brain memory as DivPrompt HTML for the launcher's read-only
/// preview (Enter on a non-note memory row — audit F5: no path let the user
/// actually READ a memory). Plain text is escaped and split into paragraphs;
/// the metadata line carries the source and last-updated time.
pub fn root_brain_memory_preview_html(
    title: &str,
    source_label: &str,
    updated_at: Option<i64>,
    content: &str,
) -> String {
    fn escape_html(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
    let mut meta = format!("From your brain · {source_label}");
    if let Some(ts) = updated_at {
        if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
            let local = dt.with_timezone(&chrono::Local);
            meta.push_str(&format!(" · {}", local.format("%b %d, %Y %H:%M")));
        }
    }
    let mut html = format!(
        "<h1>{}</h1><p><em>{}</em></p>",
        escape_html(title),
        escape_html(&meta)
    );
    for line in content.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        html.push_str(&format!("<p>{}</p>", escape_html(line)));
    }
    html.push_str("<p><em>Esc to go back</em></p>");
    html
}

/// First non-empty content line, truncated to a row-friendly excerpt.
fn excerpt_for_content(content: &str) -> String {
    truncate_excerpt(first_non_empty_line(content))
}

/// Activity-journal variant: journal lines are stamped "HH:MM — detail". The
/// stamp is useful inside the journal but reads like plumbing in a launcher
/// row (audit F11) — show only the detail.
fn excerpt_for_activity_content(content: &str) -> String {
    truncate_excerpt(strip_activity_stamp(first_non_empty_line(content)))
}

fn first_non_empty_line(content: &str) -> &str {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
}

fn truncate_excerpt(line: &str) -> String {
    if line.chars().count() <= ROOT_BRAIN_EXCERPT_MAX_CHARS {
        return line.to_string();
    }
    let mut excerpt: String = line.chars().take(ROOT_BRAIN_EXCERPT_MAX_CHARS).collect();
    excerpt.push('…');
    excerpt
}

/// Strip a leading "HH:MM — " journal stamp; anything else passes through.
fn strip_activity_stamp(line: &str) -> &str {
    match line.split_once(" — ") {
        Some((stamp, rest))
            if stamp.len() == 5
                && stamp.chars().enumerate().all(|(i, c)| {
                    if i == 2 {
                        c == ':'
                    } else {
                        c.is_ascii_digit()
                    }
                }) =>
        {
            rest
        }
        _ => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_dedupes_rendered_identity_and_keeps_best_ranked_hit() {
        let hit = |id: i64, source_id: &str| super::super::search::BrainHit {
            doc: super::super::store::BrainDoc {
                id,
                source: DocSource::ChatTurn,
                source_id: source_id.to_string(),
                title: "Follow up on this Brain Inbox item.".to_string(),
                content: "User: Follow up on this Brain Inbox item.\nDistinct hidden detail"
                    .to_string(),
                canonical_path: None,
                updated_at: id,
            },
            score: 1.0 / id as f64,
        };
        let mapped = map_root_brain_hits(vec![hit(1, "best"), hit(2, "lower")]);

        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].source_id, "best", "first ranked hit must win");
    }

    #[test]
    fn query_plan_locks_shared_sync_async_decisions() {
        let passive = RootBrainSectionOptions {
            enabled: true,
            max_results: 4,
            min_query_chars: 3,
        };
        // Explicit `brain:` widens options exactly like both call sites do.
        let explicit = RootBrainSectionOptions {
            enabled: true,
            max_results: 12,
            min_query_chars: 0,
        };

        // Bare `brain:` shows recents (sync) and stays semantically cleared
        // (async) — one variant, both behaviors.
        assert_eq!(
            root_brain_query_plan("  ", true, false, true, explicit),
            RootBrainQueryPlan::RecentsOnly
        );
        // Explicit with any query searches the trimmed text.
        assert_eq!(
            root_brain_query_plan(" x ", true, false, true, explicit),
            RootBrainQueryPlan::Search("x".to_string())
        );
        // Passive 2-char ASCII stays below min_query_chars.
        assert_eq!(
            root_brain_query_plan("xy", false, false, true, passive),
            RootBrainQueryPlan::Skip
        );
        // Passive 2-char CJK is 6 bytes — eligible (F12 byte semantics).
        assert_eq!(
            root_brain_query_plan("日本", false, false, true, passive),
            RootBrainQueryPlan::Search("日本".to_string())
        );
        // Advanced-query and negated-source gates always win.
        assert_eq!(
            root_brain_query_plan("memory", false, true, true, passive),
            RootBrainQueryPlan::Skip
        );
        assert_eq!(
            root_brain_query_plan("memory", false, false, false, passive),
            RootBrainQueryPlan::Skip
        );
        // Passive stopword-only queries have no meaningful terms.
        assert_eq!(
            root_brain_query_plan("the and", false, false, true, passive),
            RootBrainQueryPlan::Skip
        );
    }

    #[test]
    fn eligibility_mirrors_notes_gating() {
        let options = RootBrainSectionOptions {
            enabled: true,
            max_results: 4,
            min_query_chars: 3,
        };
        assert!(root_brain_query_is_eligible("fix", options));
        assert!(!root_brain_query_is_eligible("fi", options));
        assert!(!root_brain_query_is_eligible("fix\nbug", options));
        // Multibyte single "characters" carry full recall intent (F12).
        assert!(root_brain_query_is_eligible("🚀", options));
        assert!(root_brain_query_is_eligible("猫", options));
        assert!(!root_brain_query_is_eligible(
            "fix",
            RootBrainSectionOptions {
                enabled: false,
                ..options
            }
        ));
    }

    #[test]
    fn passive_root_brain_search_text_filters_filler_only_questions() {
        let options = RootBrainSectionOptions {
            enabled: true,
            max_results: 4,
            min_query_chars: 3,
        };

        for query in [
            "Why is t",
            "Why is thi",
            "Why is this o",
            "Why is this on any",
            "What is this anyway?",
            "What is this anyw",
            "What is this anywa",
            "how do I",
            "how do I fi",
            "where is the",
            "wher",
            "which",
            "whic",
            "their",
            "thei",
            "having",
            "havin",
            "why is the",
        ] {
            assert!(
                root_brain_query_is_eligible(query, options),
                "raw eligibility should stay permissive for {query:?}"
            );
            assert_eq!(
                root_brain_passive_search_text(query, options),
                None,
                "passive search text should reject filler query {query:?}"
            );
            assert!(
                !root_brain_passive_query_is_eligible(query, options),
                "passive query should reject filler query {query:?}"
            );
        }

        for (query, expected) in [
            ("Why is this script crashing", "script crashing"),
            ("how do I fix rust ownership", "fix rust ownership"),
            ("anywhere plans", "anywhere plans"),
            ("brain works", "brain works"),
            ("rust ownership", "rust ownership"),
        ] {
            assert_eq!(
                root_brain_passive_search_text(query, options).as_deref(),
                Some(expected),
                "passive query should normalize {query:?}"
            );
            assert!(root_brain_passive_query_is_eligible(query, options));
        }

        assert_eq!(
            root_brain_passive_search_text("🚀", options).as_deref(),
            Some("🚀")
        );
        assert_eq!(
            root_brain_passive_search_text("猫", options).as_deref(),
            Some("猫")
        );
        assert!(root_brain_passive_query_is_eligible("🚀", options));
        assert!(root_brain_passive_query_is_eligible("猫", options));

        assert_eq!(
            root_brain_passive_search_text("gpu", options).as_deref(),
            Some("gpu")
        );
        let short_options = RootBrainSectionOptions {
            min_query_chars: 2,
            ..options
        };
        assert_eq!(
            root_brain_passive_search_text("q2", short_options).as_deref(),
            Some("q2")
        );
        assert_eq!(
            root_brain_passive_search_text("ai", short_options).as_deref(),
            Some("ai")
        );
    }

    #[test]
    fn excerpt_uses_first_non_empty_line_and_truncates() {
        assert_eq!(
            excerpt_for_content("\n\n  hello world  \nsecond"),
            "hello world"
        );
        let long = "x".repeat(300);
        let excerpt = excerpt_for_content(&long);
        assert_eq!(excerpt.chars().count(), ROOT_BRAIN_EXCERPT_MAX_CHARS + 1);
        assert!(excerpt.ends_with('…'));
    }

    /// The memory preview must escape HTML in user content (a clipboard
    /// memory can hold arbitrary markup) and keep every content line.
    #[test]
    fn memory_preview_html_escapes_and_keeps_lines() {
        let html = root_brain_memory_preview_html(
            "Deploy <notes>",
            "Clipboard",
            None,
            "first & second\n\n<script>alert(1)</script>",
        );
        assert!(html.contains("<h1>Deploy &lt;notes&gt;</h1>"));
        assert!(html.contains("<p>first &amp; second</p>"));
        assert!(html.contains("<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("From your brain · Clipboard"));
    }

    /// F11: launcher rows for activity-journal hits must not lead with the
    /// "HH:MM — " stamp; non-stamp dashes stay untouched.
    #[test]
    fn activity_excerpt_strips_journal_stamp() {
        assert_eq!(
            excerpt_for_activity_content("23:26 — captured todo \"TPS report\"\n09:00 — older"),
            "captured todo \"TPS report\""
        );
        assert_eq!(
            excerpt_for_activity_content("plan — review the TPS report"),
            "plan — review the TPS report"
        );
        assert_eq!(excerpt_for_activity_content("2x:26 — odd"), "2x:26 — odd");
    }

    fn semantic_hit(title: &str) -> RootBrainSearchHit {
        RootBrainSearchHit {
            title: title.to_string(),
            excerpt: String::new(),
            source_label: "Note",
            source: DocSource::Note,
            source_id: title.to_string(),
        }
    }

    #[test]
    fn root_brain_prefers_semantic_hits_when_stored_query_matches() {
        let options = RootBrainSectionOptions::default();
        let stored = (
            "fix bug".to_string(),
            vec![semantic_hit("a"), semantic_hit("b")],
        );
        let hits = semantic_root_brain_hits_for_query("fix bug", Some(&stored), &options)
            .expect("matching query should prefer semantic hits");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "a");
        // Whitespace around the live query must not break the match — the
        // stored query is the trimmed search text.
        assert!(
            semantic_root_brain_hits_for_query("  fix bug  ", Some(&stored), &options).is_some()
        );
    }

    #[test]
    fn root_brain_falls_back_to_lexical_when_semantic_is_stale_or_missing() {
        let options = RootBrainSectionOptions::default();
        let stored = ("fix bug".to_string(), vec![semantic_hit("a")]);
        // Stale stored query → lexical.
        assert!(semantic_root_brain_hits_for_query("fix bugs", Some(&stored), &options).is_none());
        // No semantic results yet → lexical.
        assert!(semantic_root_brain_hits_for_query("fix bug", None, &options).is_none());
    }

    #[test]
    fn root_brain_semantic_merge_respects_eligibility_and_caps() {
        let stored = (
            "fix".to_string(),
            vec![semantic_hit("a"), semantic_hit("b"), semantic_hit("c")],
        );
        // Disabled section → never serve semantic hits.
        let disabled = RootBrainSectionOptions {
            enabled: false,
            ..Default::default()
        };
        assert!(semantic_root_brain_hits_for_query("fix", Some(&stored), &disabled).is_none());
        // Query below min chars → ineligible.
        let options = RootBrainSectionOptions::default();
        let short_stored = ("fi".to_string(), vec![semantic_hit("a")]);
        assert!(semantic_root_brain_hits_for_query("fi", Some(&short_stored), &options).is_none());
        // max_results caps stored hits (e.g. options shrank since the spawn).
        let capped = RootBrainSectionOptions {
            max_results: 2,
            ..Default::default()
        };
        let hits = semantic_root_brain_hits_for_query("fix", Some(&stored), &capped)
            .expect("eligible matching query");
        assert_eq!(hits.len(), 2);
        // max_results == 0 → section renders nothing; keep lexical path.
        let zero = RootBrainSectionOptions {
            max_results: 0,
            ..Default::default()
        };
        assert!(semantic_root_brain_hits_for_query("fix", Some(&stored), &zero).is_none());
    }

    #[test]
    fn root_brain_semantic_empty_hits_still_count_as_results() {
        // Hybrid search subsumes lexical: an empty semantic batch for the
        // current query means "the brain has nothing", not "fall back".
        let options = RootBrainSectionOptions::default();
        let stored = ("fix bug".to_string(), Vec::new());
        let hits = semantic_root_brain_hits_for_query("fix bug", Some(&stored), &options)
            .expect("empty semantic batch is authoritative");
        assert!(hits.is_empty());
    }

    fn inbox_item(detail: &str, created_at: i64) -> crate::brain::InboxItem {
        crate::brain::InboxItem {
            id: 7,
            kind: crate::brain::inbox::InboxKind::Commitment,
            title: "Ship the launcher inbox".to_string(),
            detail: detail.to_string(),
            source: "chat_turn".to_string(),
            source_id: "thread#3".to_string(),
            created_at,
            resolved_at: None,
        }
    }

    #[test]
    fn inbox_subtitle_prefers_detail_over_age() {
        let now = 1_000_000;
        let item = inbox_item(
            "  \n promised in chat yesterday \nsecond line",
            now - 3 * 86_400,
        );
        assert_eq!(
            root_brain_inbox_subtitle(&item, now),
            "Commitment · promised in chat yesterday"
        );
    }

    #[test]
    fn inbox_subtitle_falls_back_to_relative_age() {
        let now = 1_000_000;
        assert_eq!(
            root_brain_inbox_subtitle(&inbox_item("", now - 30), now),
            "Commitment · just now"
        );
        assert_eq!(
            root_brain_inbox_subtitle(&inbox_item("", now - 5 * 60), now),
            "Commitment · 5m ago"
        );
        assert_eq!(
            root_brain_inbox_subtitle(&inbox_item("", now - 3 * 3_600), now),
            "Commitment · 3h ago"
        );
        assert_eq!(
            root_brain_inbox_subtitle(&inbox_item("", now - 2 * 86_400), now),
            "Commitment · 2d ago"
        );
        // Clock skew (created in the future) clamps to "just now".
        assert_eq!(
            root_brain_inbox_subtitle(&inbox_item("", now + 600), now),
            "Commitment · just now"
        );
    }

    #[test]
    fn inbox_subtitle_truncates_long_detail() {
        let now = 1_000_000;
        let item = inbox_item(&"d".repeat(300), now);
        let subtitle = root_brain_inbox_subtitle(&item, now);
        assert!(subtitle.starts_with("Commitment · "));
        assert!(subtitle.ends_with('…'));
    }

    #[test]
    fn disabled_or_short_queries_return_empty() {
        let options = RootBrainSectionOptions {
            enabled: false,
            ..Default::default()
        };
        assert!(search_root_brain_direct("anything", &options).is_empty());
        let options = RootBrainSectionOptions::default();
        assert!(search_root_brain_direct("ab", &options).is_empty());
    }
}

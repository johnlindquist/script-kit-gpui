//! Shared context selector catalog and row types for Agent Chat and portals.
//!
//! This module is deliberately UI-state-free. It builds selector rows and
//! parses trigger queries, but popup lifecycle and selection state belong to
//! the surface that renders them.

pub mod types;

use crate::ai::context_contract::{context_attachment_specs, ContextAttachmentKind};
use gpui::SharedString;
use types::{
    ContextPortalKind, ContextPortalPrefixPayload, ContextSelectorRow, ContextSelectorRowKind,
    ContextSelectorTrigger, InlinePortalAttachment, InlinePortalResultPayload,
    PROFILE_TRIGGER_CHAR,
};

use std::sync::{Arc, OnceLock, RwLock};

/// Maximum number of file/folder results to include.
const FILE_RESULTS_LIMIT: usize = 10;
const INLINE_PORTAL_RESULTS_LIMIT: usize = 10;

/// Parsed trigger + query extracted from the composer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextSelectorQuery {
    pub trigger: ContextSelectorTrigger,
    pub query: String,
}

/// Cursor-aware trigger extraction result with char range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextSelectorQueryAtCursor {
    pub trigger: ContextSelectorTrigger,
    pub char_range: std::ops::Range<usize>,
    pub query: String,
}

fn char_to_byte_offset(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(ix, _)| ix)
        .unwrap_or(text.len())
}

/// Extract a trigger query from the composer text at a specific cursor position.
///
/// Shared implementation used by Agent Chat and other selector-capable surfaces.
/// Returns `None` when there is no active trigger before the cursor.
pub(crate) fn context_selector_query_before_cursor(
    input: &str,
    cursor: usize,
) -> Option<ContextSelectorQueryAtCursor> {
    if cursor > input.chars().count() {
        return None;
    }

    let cursor_byte = char_to_byte_offset(input, cursor);
    let before_cursor = &input[..cursor_byte];

    let trigger_pos = before_cursor.rfind(['@', '/', PROFILE_TRIGGER_CHAR])?;
    let trigger_byte = before_cursor.as_bytes().get(trigger_pos).copied()?;

    let trigger = match trigger_byte {
        b'@' => ContextSelectorTrigger::Mention,
        b'/' => ContextSelectorTrigger::Slash,
        b'|' => ContextSelectorTrigger::Profile,
        _ => return None,
    };

    // Trigger must be at start of text or preceded by appropriate chars
    if trigger_pos > 0 {
        let prev = before_cursor.as_bytes()[trigger_pos - 1];
        match trigger_byte {
            // `@` requires non-alnum/underscore before it (reject `me@home`)
            b'@' if prev.is_ascii_alphanumeric() || prev == b'_' => return None,
            // `/` requires whitespace before it (reject `foo/bar`)
            b'/' if prev != b' ' && prev != b'\n' && prev != b'\t' => return None,
            // `|` mirrors slash-command behavior and only opens at a token boundary.
            b'|' if prev != b' ' && prev != b'\n' && prev != b'\t' => return None,
            _ => {}
        }
    }

    let query = &before_cursor[trigger_pos + 1..];

    // Reject if whitespace immediately follows trigger
    if query.starts_with(' ') || query.starts_with('\n') || query.starts_with('\t') {
        return None;
    }

    // Reject if query contains another trigger char or any whitespace
    let trigger_char = match trigger {
        ContextSelectorTrigger::Mention => '@',
        ContextSelectorTrigger::Slash => '/',
        ContextSelectorTrigger::Profile => PROFILE_TRIGGER_CHAR,
    };
    if query.contains(trigger_char) || query.chars().any(char::is_whitespace) {
        return None;
    }

    let trigger_char_idx = before_cursor[..trigger_pos].chars().count();

    let safe_query = crate::logging::log_private_user_value(query);
    tracing::debug!(
        target: "ai",
        ?trigger,
        cursor,
        trigger_char_idx,
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        "context_selector_trigger_extracted"
    );

    Some(ContextSelectorQueryAtCursor {
        trigger,
        char_range: trigger_char_idx..cursor,
        query: query.to_string(),
    })
}

/// Extract a trigger query from the composer text (end-of-string cursor).
///
/// Thin wrapper around `context_selector_query_before_cursor`.
pub(crate) fn context_selector_query(input: &str) -> Option<ContextSelectorQuery> {
    let result = context_selector_query_before_cursor(input, input.chars().count())?;
    Some(ContextSelectorQuery {
        trigger: result.trigger,
        query: result.query,
    })
}

/// Fuzzy match query characters in order against a candidate string.
///
/// Returns the indices of matched characters in the candidate, or `None`
/// if the query cannot be matched in order.
pub(crate) fn match_query_chars(query: &str, candidate: &str) -> Option<Vec<usize>> {
    if query.is_empty() {
        return Some(Vec::new());
    }
    let candidate_chars: Vec<char> = candidate.chars().collect();
    let mut hits = Vec::with_capacity(query.len());
    let mut from = 0usize;
    for needle in query.chars().map(|ch| ch.to_ascii_lowercase()) {
        let mut found = None;
        for (ix, ch) in candidate_chars.iter().enumerate().skip(from) {
            if ch.to_ascii_lowercase() == needle {
                found = Some(ix);
                break;
            }
        }
        let ix = found?;
        hits.push(ix);
        from = ix + 1;
    }
    Some(hits)
}

/// Match query characters against rendered meta text, offsetting indices
/// past any leading `@` or `/` prefix so highlights land on the matched
/// characters rather than the trigger symbol.
pub(crate) fn match_query_chars_in_display_meta(
    query: &str,
    display_meta: &str,
) -> Option<Vec<usize>> {
    if query.is_empty() {
        return Some(Vec::new());
    }
    let prefix_len = display_meta
        .chars()
        .take_while(|ch| *ch == '@' || *ch == '/')
        .count();
    let bare = display_meta.trim_start_matches(['@', '/']);
    let hits = match_query_chars(query, bare)?;
    Some(hits.into_iter().map(|ix| ix + prefix_len).collect())
}

/// A hint chip for the empty state when no results match.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ContextSelectorEmptyStateHint {
    /// What is displayed in the hint chip.
    pub display: &'static str,
    /// What is inserted into the composer when clicked.
    pub insertion: &'static str,
}

/// Hint chips for the empty state when no results match.
///
/// These are the canonical entries shared by Agent Chat and inline mention sync.
/// `@file:<path>` uses `insertion: "@file:"` so clicking it keeps file
/// suggestion flows open instead of fabricating a fake path.
pub(crate) fn context_selector_empty_state_hints(
    trigger: ContextSelectorTrigger,
) -> std::borrow::Cow<'static, [ContextSelectorEmptyStateHint]> {
    static MENTION_HINTS: &[ContextSelectorEmptyStateHint] = &[
        ContextSelectorEmptyStateHint {
            display: "@screenshot",
            insertion: "@screenshot",
        },
        ContextSelectorEmptyStateHint {
            display: "@clipboard",
            insertion: "@clipboard",
        },
        ContextSelectorEmptyStateHint {
            display: "@git-diff",
            insertion: "@git-diff",
        },
        ContextSelectorEmptyStateHint {
            display: "@recent-scripts",
            insertion: "@recent-scripts",
        },
        ContextSelectorEmptyStateHint {
            display: "@dictation",
            insertion: "@dictation",
        },
        ContextSelectorEmptyStateHint {
            display: "@calendar",
            insertion: "@calendar",
        },
        ContextSelectorEmptyStateHint {
            display: "@file:<path>",
            insertion: "@file:",
        },
    ];
    static SLASH_HINTS: &[ContextSelectorEmptyStateHint] = &[
        ContextSelectorEmptyStateHint {
            display: "/compact",
            insertion: "/compact ",
        },
        ContextSelectorEmptyStateHint {
            display: "/clear",
            insertion: "/clear ",
        },
        ContextSelectorEmptyStateHint {
            display: "/help",
            insertion: "/help ",
        },
    ];
    static PROFILE_HINTS: &[ContextSelectorEmptyStateHint] = &[
        ContextSelectorEmptyStateHint {
            display: "|general",
            insertion: "|general",
        },
        ContextSelectorEmptyStateHint {
            display: "|script-kit",
            insertion: "|script-kit",
        },
    ];
    let base = match trigger {
        ContextSelectorTrigger::Mention => MENTION_HINTS,
        ContextSelectorTrigger::Slash => SLASH_HINTS,
        ContextSelectorTrigger::Profile => PROFILE_HINTS,
    };

    if trigger != ContextSelectorTrigger::Mention {
        tracing::debug!(
            target: "ai",
            event = "ai_context_selector_context_selector_empty_state_hints_selected",
            trigger = ?trigger,
            hint_count = base.len(),
            filtered_hint_count = base.len(),
        );
        return std::borrow::Cow::Borrowed(base);
    }

    let filtered: Vec<ContextSelectorEmptyStateHint> = base
        .iter()
        .copied()
        .filter(|hint| {
            crate::ai::context_contract::ContextAttachmentKind::from_mention_line(hint.insertion)
                .map(|kind| kind.provider_data_available())
                .unwrap_or(true)
        })
        .collect();

    tracing::debug!(
        target: "ai",
        event = "ai_context_selector_context_selector_empty_state_hints_selected",
        trigger = ?trigger,
        hint_count = base.len(),
        filtered_hint_count = filtered.len(),
    );

    std::borrow::Cow::Owned(filtered)
}

// ── Cached built-in picker seeds ──────────────────────────────────────

#[derive(Debug, Clone)]
struct BuiltinPickerSeed {
    kind: ContextAttachmentKind,
    label: &'static str,
    label_lower: String,
    search_alias_lowers: Vec<String>,
    mention_meta: &'static str,
    mention_meta_lower: String,
    slash_meta: &'static str,
    slash_meta_lower: String,
    has_slash_command: bool,
}

fn builtin_picker_seeds() -> &'static [BuiltinPickerSeed] {
    static CACHE: OnceLock<Vec<BuiltinPickerSeed>> = OnceLock::new();
    CACHE.get_or_init(|| {
        context_attachment_specs()
            .iter()
            .map(|spec| {
                let mention_meta = spec
                    .mention
                    .or(spec.slash_command)
                    .unwrap_or(spec.action_title);
                let slash_meta = spec
                    .slash_command
                    .or(spec.mention)
                    .unwrap_or(spec.action_title);
                let mut search_alias_lowers = Vec::new();
                search_alias_lowers.push(spec.action_title.to_lowercase());
                for alias in spec.mention_aliases {
                    search_alias_lowers.push(alias.trim_start_matches(['@', '/']).to_lowercase());
                }
                for alias in spec.slash_aliases {
                    search_alias_lowers.push(alias.trim_start_matches(['@', '/']).to_lowercase());
                }
                if spec.kind == ContextAttachmentKind::Current {
                    search_alias_lowers.push("current context".to_string());
                    search_alias_lowers.push("context".to_string());
                }
                BuiltinPickerSeed {
                    kind: spec.kind,
                    label: spec.label,
                    label_lower: spec.label.to_lowercase(),
                    search_alias_lowers,
                    mention_meta,
                    mention_meta_lower: mention_meta.to_lowercase(),
                    slash_meta,
                    slash_meta_lower: slash_meta.to_lowercase(),
                    has_slash_command: spec.slash_command.is_some(),
                }
            })
            .collect()
    })
}

fn builtin_seed(kind: ContextAttachmentKind) -> &'static BuiltinPickerSeed {
    builtin_picker_seeds()
        .iter()
        .find(|seed| seed.kind == kind)
        .unwrap_or_else(|| unreachable!("missing BuiltinPickerSeed"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InlinePortalQuery {
    kind: ContextPortalKind,
    prefix: &'static str,
    query: String,
}

fn portal_prefix_for_kind(kind: ContextPortalKind) -> &'static str {
    match kind {
        ContextPortalKind::FileSearch => "file",
        ContextPortalKind::BrowserHistory => "browser-history",
        ContextPortalKind::BrowserTabs => "tabs",
        ContextPortalKind::ClipboardHistory => "clipboard",
        ContextPortalKind::DictationHistory => "dictation",
        ContextPortalKind::ScriptSearch => "script",
        ContextPortalKind::ScriptletSearch => "scriptlet",
        ContextPortalKind::SkillSearch => "skill",
        ContextPortalKind::NotesBrowse => "note",
        ContextPortalKind::AgentChatHistory => "history",
        ContextPortalKind::Terminal => "terminal",
    }
}

fn portal_kind_from_prefix(prefix: &str) -> Option<ContextPortalKind> {
    match prefix {
        "file" => Some(ContextPortalKind::FileSearch),
        "browser-history" => Some(ContextPortalKind::BrowserHistory),
        "tabs" | "browser-tabs" => Some(ContextPortalKind::BrowserTabs),
        "clipboard" => Some(ContextPortalKind::ClipboardHistory),
        "dictation" => Some(ContextPortalKind::DictationHistory),
        "script" => Some(ContextPortalKind::ScriptSearch),
        "scriptlet" => Some(ContextPortalKind::ScriptletSearch),
        "skill" => Some(ContextPortalKind::SkillSearch),
        "note" => Some(ContextPortalKind::NotesBrowse),
        "history" => Some(ContextPortalKind::AgentChatHistory),
        "terminal" => Some(ContextPortalKind::Terminal),
        _ => None,
    }
}

fn inline_portal_query(trigger: ContextSelectorTrigger, query: &str) -> Option<InlinePortalQuery> {
    if trigger != ContextSelectorTrigger::Mention {
        return None;
    }
    let trimmed = query.trim();
    let lower = trimmed.to_lowercase();
    let (prefix, search_query) = match lower.split_once(':') {
        Some((prefix, _)) => {
            let query_offset = prefix.len() + 1;
            (prefix, trimmed.get(query_offset..).unwrap_or_default())
        }
        None => (lower.as_str(), ""),
    };
    let kind = portal_kind_from_prefix(prefix)?;
    if lower.starts_with(&format!("{prefix}:")) {
        return Some(InlinePortalQuery {
            kind,
            prefix: portal_prefix_for_kind(kind),
            query: search_query.to_string(),
        });
    }
    None
}

fn file_search_query(trigger: ContextSelectorTrigger, query: &str) -> Option<String> {
    inline_portal_query(trigger, query)
        .filter(|inline| inline.kind == ContextPortalKind::FileSearch)
        .map(|inline| inline.query)
}

fn split_file_query(base_dir: &std::path::Path, raw_query: &str) -> (std::path::PathBuf, String) {
    if raw_query.is_empty() {
        return (base_dir.to_path_buf(), String::new());
    }
    let query_path = std::path::Path::new(raw_query);
    let ends_with_sep = raw_query.ends_with(std::path::MAIN_SEPARATOR);
    let parent = if ends_with_sep {
        query_path
    } else {
        query_path.parent().unwrap_or(std::path::Path::new(""))
    };
    let name_filter = if ends_with_sep {
        ""
    } else {
        query_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    };
    let search_dir = if parent.as_os_str().is_empty() {
        base_dir.to_path_buf()
    } else if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        base_dir.join(parent)
    };
    (search_dir, name_filter.to_lowercase())
}

fn score_builtin_seed(
    seed: &BuiltinPickerSeed,
    trigger: ContextSelectorTrigger,
    query: &str,
) -> (u32, Vec<usize>, Vec<usize>) {
    if query.is_empty() {
        return (100, Vec::new(), Vec::new());
    }

    let (display_meta, primary, secondary): (&str, &str, &str) = match trigger {
        ContextSelectorTrigger::Mention => (
            seed.mention_meta,
            seed.mention_meta_lower.trim_start_matches(['@', '/']),
            seed.slash_meta_lower.trim_start_matches(['@', '/']),
        ),
        ContextSelectorTrigger::Slash => (
            seed.slash_meta,
            seed.slash_meta_lower.trim_start_matches(['@', '/']),
            seed.mention_meta_lower.trim_start_matches(['@', '/']),
        ),
        ContextSelectorTrigger::Profile => ("", "", ""),
    };

    let mut best_score = 0u32;
    let mut best_label_hits = Vec::new();
    let mut best_meta_hits = Vec::new();

    let compute_hits = |q: &str| -> (Vec<usize>, Vec<usize>) {
        (
            match_query_chars(q, seed.label).unwrap_or_default(),
            match_query_chars_in_display_meta(q, display_meta).unwrap_or_default(),
        )
    };

    if primary == query {
        best_score = 1000;
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }
    if best_score < 500 && !primary.is_empty() && primary.starts_with(query) {
        best_score = 500 + (100 - query.len().min(99) as u32);
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }
    if best_score < 500 && !secondary.is_empty() && secondary.starts_with(query) {
        best_score = 500 + (100 - query.len().min(99) as u32);
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }
    if best_score < 400 && seed.label_lower.starts_with(query) {
        best_score = 400;
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }
    if best_score < 400
        && seed
            .search_alias_lowers
            .iter()
            .any(|alias| alias.starts_with(query))
    {
        best_score = 400;
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }
    if best_score < 200 && seed.label_lower.contains(query) {
        best_score = 200;
        best_label_hits = match_query_chars(query, seed.label).unwrap_or_default();
        best_meta_hits = Vec::new();
    }
    if best_score < 200
        && seed
            .search_alias_lowers
            .iter()
            .any(|alias| alias.contains(query))
    {
        best_score = 200;
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }
    if best_score < 100
        && ((!primary.is_empty() && primary.contains(query))
            || (!secondary.is_empty() && secondary.contains(query)))
    {
        best_score = 100;
        (best_label_hits, best_meta_hits) = compute_hits(query);
    }

    // Fuzzy fallback: admit scattered-character matches when no stronger
    // rule matched (e.g. "gst" → "git-status").
    if best_score == 0 {
        let label_fuzzy = match_query_chars(query, seed.label);
        let meta_fuzzy = match_query_chars_in_display_meta(query, display_meta);
        if label_fuzzy.is_some() || meta_fuzzy.is_some() {
            best_score = 50;
            best_label_hits = label_fuzzy.unwrap_or_default();
            best_meta_hits = meta_fuzzy.unwrap_or_default();
            let safe_query = crate::logging::log_private_user_value(query);
            tracing::debug!(
                target: "ai",
                kind = ?seed.kind,
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                score = best_score,
                label_hits = ?best_label_hits,
                meta_hits = ?best_meta_hits,
                "ai_context_selector_builtin_fuzzy_match"
            );
        }
    }

    (best_score, best_label_hits, best_meta_hits)
}

/// Populate `items` with built-in context attachment entries and optional
/// portal results. Shared by both `context_selector_rows` and
/// `slash_command_rows`.
fn extend_builtin_picker_items(
    trigger: ContextSelectorTrigger,
    query: &str,
    query_lower: &str,
    items: &mut Vec<ContextSelectorRow>,
) {
    if trigger == ContextSelectorTrigger::Profile {
        return;
    }

    for seed in builtin_picker_seeds() {
        if trigger == ContextSelectorTrigger::Slash && !seed.has_slash_command {
            continue;
        }

        // Hide provider-backed items when no real data exists
        if !seed.kind.provider_data_available() {
            tracing::info!(
                target: "ai",
                event = "ai_context_selector_seed_skipped_provider_unavailable",
                kind = ?seed.kind,
                trigger = ?trigger,
            );
            continue;
        }

        let (score, label_hits, meta_hits) = score_builtin_seed(seed, trigger, query_lower);

        if score == 0 && !query_lower.is_empty() {
            continue;
        }

        let meta = match trigger {
            ContextSelectorTrigger::Mention => seed.mention_meta,
            ContextSelectorTrigger::Slash => seed.slash_meta,
            ContextSelectorTrigger::Profile => "",
        };

        items.push(ContextSelectorRow {
            id: SharedString::from(format!("builtin:{:?}", seed.kind).to_lowercase()),
            label: SharedString::from(seed.label),
            description: SharedString::from(seed.kind.spec().action_title),
            meta: SharedString::from(meta),
            kind: ContextSelectorRowKind::BuiltIn(seed.kind),
            score: if query_lower.is_empty() { 100 } else { score },
            label_highlight_indices: label_hits,
            meta_highlight_indices: meta_hits,
        });
    }

    // Portal-prefixed results stay inline after `@clipboard:`,
    // `@browser-history:`, etc. File search is the exception: it must open
    // the full built-in File Search surface so preview and folder browsing
    // stay identical to the direct Search Files command.
    if let Some(inline_query) = inline_portal_query(trigger, query) {
        if inline_query.kind != ContextPortalKind::FileSearch {
            collect_inline_portal_items(&inline_query, items);
        }
        inject_full_portal_fallback(&inline_query, items);
        return;
    } else if file_search_query(trigger, query).is_none() {
        let safe_query = crate::logging::log_private_user_value(query);
        tracing::debug!(
            target: "ai",
            ?trigger,
            query_bytes = safe_query.raw_bytes,
            query_sha256 = %safe_query.sha256,
            "ai_context_selector_file_scan_skipped"
        );
    }

    // Portal items — rich browse surfaces that attach their selection back to Agent Chat.
    // Only in mention mode; slash mode is command-only.
    if trigger == ContextSelectorTrigger::Mention {
        inject_portal_items(query_lower, items);
    }
}

/// Inject portal items for rich browsing. These open a temporary browse
/// surface and attach the selected result back to Agent Chat.
fn inject_portal_items(query_lower: &str, items: &mut Vec<ContextSelectorRow>) {
    struct PortalDef {
        kind: ContextPortalKind,
        id: &'static str,
        label: &'static str,
        description: &'static str,
        meta: &'static str,
        match_terms: &'static [&'static str],
    }

    let portals: &[PortalDef] = &[
        PortalDef {
            kind: ContextPortalKind::FileSearch,
            id: "portal:file_search",
            label: "@file",
            description: "Search files with Spotlight and browse folders",
            meta: "Portal",
            match_terms: &["file", "files", "browse", "search"],
        },
        PortalDef {
            kind: ContextPortalKind::BrowserHistory,
            id: "portal:browser_history",
            label: "@browser-history",
            description: "Browse recent browser history across supported browsers",
            meta: "Portal",
            match_terms: &[
                "browser", "history", "chrome", "safari", "firefox", "arc", "brave", "edge",
                "visit", "url",
            ],
        },
        PortalDef {
            kind: ContextPortalKind::BrowserTabs,
            id: "portal:browser_tabs",
            label: "@tabs",
            description: "Attach an open browser tab with title, URL, and browser metadata",
            meta: "Portal",
            match_terms: &[
                "tabs",
                "tab",
                "browser-tabs",
                "browser",
                "open",
                "url",
                "chrome",
                "safari",
                "firefox",
                "arc",
                "brave",
                "edge",
            ],
        },
        PortalDef {
            kind: ContextPortalKind::ClipboardHistory,
            id: "portal:clipboard_history",
            label: "@clipboard",
            description: "Browse clipboard history with previews",
            meta: "Portal",
            match_terms: &["clipboard", "clip", "paste"],
        },
        PortalDef {
            kind: ContextPortalKind::ScriptSearch,
            id: "portal:script_search",
            label: "@script",
            description: "Browse installed scripts and attach one to Agent Chat",
            meta: "Portal",
            match_terms: &["script", "scripts", "command", "commands", "browse"],
        },
        PortalDef {
            kind: ContextPortalKind::ScriptletSearch,
            id: "portal:scriptlet_search",
            label: "@scriptlet",
            description: "Browse scriptlets and attach one to Agent Chat",
            meta: "Portal",
            match_terms: &["scriptlet", "scriptlets", "snippet", "snippets", "browse"],
        },
        PortalDef {
            kind: ContextPortalKind::SkillSearch,
            id: "portal:skill_search",
            label: "@skill",
            description: "Browse skills and attach one to Agent Chat",
            meta: "Portal",
            match_terms: &["skill", "skills", "agent", "agents", "browse"],
        },
        PortalDef {
            kind: ContextPortalKind::NotesBrowse,
            id: "portal:notes_browse",
            label: "@note",
            description: "Browse notes and attach one to Agent Chat",
            meta: "Portal",
            match_terms: &["note", "notes", "markdown", "browse"],
        },
        PortalDef {
            kind: ContextPortalKind::AgentChatHistory,
            id: "portal:agent_chat_history",
            label: "@history",
            description: "Browse prior Agent Chat conversations",
            meta: "Portal",
            match_terms: &["history", "conversation", "chat", "resume", "reuse"],
        },
        PortalDef {
            kind: ContextPortalKind::Terminal,
            id: "portal:terminal",
            label: "@terminal",
            description: "Run commands and attach the terminal output",
            meta: "Portal",
            match_terms: &["terminal", "term", "shell", "command", "commands", "output"],
        },
    ];

    for def in portals {
        let PortalDef {
            kind,
            id,
            label,
            description,
            meta,
            match_terms,
        } = def;
        let (score, label_hits) = if query_lower.is_empty() {
            // Higher than built-in default (100) to appear at top of the list.
            (200u32, Vec::new())
        } else if match_terms.iter().any(|t| t.starts_with(query_lower)) {
            (
                80,
                match_query_chars(query_lower, &label.to_lowercase()).unwrap_or_default(),
            )
        } else if match_terms.iter().any(|t| t.contains(query_lower)) {
            (
                40,
                match_query_chars(query_lower, &label.to_lowercase()).unwrap_or_default(),
            )
        } else if let Some(hits) = match_query_chars(query_lower, &label.to_lowercase()) {
            (20, hits)
        } else {
            continue;
        };

        let meta_hits = match_query_chars_in_display_meta(query_lower, meta).unwrap_or_default();

        items.push(ContextSelectorRow {
            id: SharedString::from(*id),
            label: SharedString::from(*label),
            description: SharedString::from(*description),
            meta: SharedString::from(*meta),
            kind: ContextSelectorRowKind::Portal(*kind),
            score,
            label_highlight_indices: label_hits,
            meta_highlight_indices: meta_hits,
        });
    }
}

fn portal_kind_detail_label(kind: ContextPortalKind) -> &'static str {
    match kind {
        ContextPortalKind::FileSearch => "file search",
        ContextPortalKind::BrowserHistory => "browser history",
        ContextPortalKind::BrowserTabs => "browser tabs",
        ContextPortalKind::ClipboardHistory => "clipboard history",
        ContextPortalKind::DictationHistory => "dictation history",
        ContextPortalKind::ScriptSearch => "script search",
        ContextPortalKind::ScriptletSearch => "scriptlet search",
        ContextPortalKind::SkillSearch => "skill search",
        ContextPortalKind::NotesBrowse => "notes",
        ContextPortalKind::AgentChatHistory => "Agent Chat history",
        ContextPortalKind::Terminal => "terminal",
    }
}

fn inject_full_portal_fallback(
    inline_query: &InlinePortalQuery,
    items: &mut Vec<ContextSelectorRow>,
) {
    let label = format!("Open full {}", portal_kind_detail_label(inline_query.kind));
    items.push(ContextSelectorRow {
        id: SharedString::from(format!("portal-full:{}", inline_query.prefix)),
        label: SharedString::from(label.clone()),
        description: SharedString::from(format!(
            "Open the full {} browser",
            portal_kind_detail_label(inline_query.kind)
        )),
        meta: SharedString::from("Portal"),
        kind: ContextSelectorRowKind::Portal(inline_query.kind),
        score: 10,
        label_highlight_indices: match_query_chars(&inline_query.query.to_lowercase(), &label)
            .unwrap_or_default(),
        meta_highlight_indices: Vec::new(),
    });
}

fn collect_inline_portal_items(
    inline_query: &InlinePortalQuery,
    items: &mut Vec<ContextSelectorRow>,
) {
    match inline_query.kind {
        ContextPortalKind::FileSearch => {}
        ContextPortalKind::BrowserHistory => {
            collect_browser_history_inline_items(&inline_query.query, items)
        }
        ContextPortalKind::BrowserTabs => {
            collect_browser_tabs_inline_items(&inline_query.query, items)
        }
        ContextPortalKind::ClipboardHistory => {
            collect_clipboard_inline_items(&inline_query.query, items)
        }
        ContextPortalKind::DictationHistory => {
            collect_dictation_inline_items(&inline_query.query, items)
        }
        ContextPortalKind::ScriptSearch
        | ContextPortalKind::ScriptletSearch
        | ContextPortalKind::SkillSearch => collect_script_list_inline_items(inline_query, items),
        ContextPortalKind::NotesBrowse => collect_notes_inline_items(&inline_query.query, items),
        ContextPortalKind::AgentChatHistory => {
            collect_agent_chat_history_inline_items(&inline_query.query, items)
        }
        ContextPortalKind::Terminal => {
            collect_terminal_history_inline_items(&inline_query.query, items)
        }
    }
}

fn collect_terminal_history_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    let query_lower = query.to_lowercase();
    for entry in crate::terminal_history::recent(INLINE_PORTAL_RESULTS_LIMIT) {
        let haystack = format!("{} {}", entry.label, entry.text).to_lowercase();
        if !query_lower.is_empty() && !haystack.contains(&query_lower) {
            continue;
        }
        let preview = entry
            .text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Terminal output")
            .chars()
            .take(96)
            .collect::<String>();
        items.push(ContextSelectorRow {
            id: SharedString::from(format!("terminal-history:{}", entry.source)),
            label: SharedString::from(entry.label.clone()),
            description: SharedString::from(preview),
            meta: SharedString::from("Terminal"),
            kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                portal_kind: ContextPortalKind::Terminal,
                attachment: InlinePortalAttachment::TextBlock {
                    label: entry.label,
                    source: entry.source,
                    text: entry.text,
                    mime_type: Some("text/x-terminal-transcript".to_string()),
                },
            }),
            score: if query_lower.is_empty() { 80 } else { 120 },
            label_highlight_indices: match_query_chars(&query_lower, "terminal")
                .unwrap_or_default(),
            meta_highlight_indices: Vec::new(),
        });
    }
}

#[derive(Default)]
struct LauncherCatalogSnapshot {
    scripts: Vec<Arc<crate::scripts::Script>>,
    scriptlets: Vec<Arc<crate::scripts::Scriptlet>>,
    skills: Vec<Arc<crate::plugins::PluginSkill>>,
}

#[derive(Default)]
struct LauncherCatalogStore {
    snapshot: RwLock<Arc<LauncherCatalogSnapshot>>,
}

impl LauncherCatalogStore {
    fn snapshot(&self) -> Arc<LauncherCatalogSnapshot> {
        self.snapshot
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn publish(
        &self,
        scripts: &[Arc<crate::scripts::Script>],
        scriptlets: &[Arc<crate::scripts::Scriptlet>],
        skills: &[Arc<crate::plugins::PluginSkill>],
    ) {
        let snapshot = Arc::new(LauncherCatalogSnapshot {
            scripts: scripts.to_vec(),
            scriptlets: scriptlets.to_vec(),
            skills: skills.to_vec(),
        });
        *self
            .snapshot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = snapshot;
    }
}

fn launcher_catalog_store() -> &'static LauncherCatalogStore {
    static STORE: OnceLock<LauncherCatalogStore> = OnceLock::new();
    STORE.get_or_init(LauncherCatalogStore::default)
}

/// Publish the launcher's already-loaded catalogs as one coherent immutable snapshot.
///
/// Detached Agent Chat Spine searches never discover plugins, read files, or
/// retain a stale process-lifetime catalog: the host republishes on refresh.
pub(crate) fn publish_launcher_catalog(
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
    skills: &[Arc<crate::plugins::PluginSkill>],
) {
    launcher_catalog_store().publish(scripts, scriptlets, skills);
}

/// Build the actual detached Agent Chat Spine attachment section from the
/// host-owned immutable command snapshot, not from the retired picker.
pub(crate) fn composer_catalog_subsearch_section(
    source: crate::spine::catalog_subsearch::ContextSubsearchSource,
    query: &str,
    segment_index: usize,
    segment_byte_range: std::ops::Range<usize>,
) -> Option<crate::spine::attach::ComposerSubsearchSection> {
    let snapshot = launcher_catalog_store().snapshot();
    composer_catalog_subsearch_section_from_snapshot(
        source,
        query,
        segment_index,
        segment_byte_range,
        &snapshot,
    )
}

fn composer_catalog_subsearch_section_from_snapshot(
    source: crate::spine::catalog_subsearch::ContextSubsearchSource,
    query: &str,
    segment_index: usize,
    segment_byte_range: std::ops::Range<usize>,
    snapshot: &LauncherCatalogSnapshot,
) -> Option<crate::spine::attach::ComposerSubsearchSection> {
    crate::spine::attach::composer_catalog_subsearch_section(
        source,
        query,
        segment_index,
        segment_byte_range,
        &snapshot.scripts,
        &snapshot.scriptlets,
        &snapshot.skills,
    )
}

fn inline_portal_scripts() -> &'static [Arc<crate::scripts::Script>] {
    static CACHE: OnceLock<Vec<Arc<crate::scripts::Script>>> = OnceLock::new();
    CACHE.get_or_init(crate::scripts::read_scripts).as_slice()
}

fn inline_portal_scriptlets() -> &'static [Arc<crate::scripts::Scriptlet>] {
    static CACHE: OnceLock<Vec<Arc<crate::scripts::Scriptlet>>> = OnceLock::new();
    CACHE
        .get_or_init(crate::scripts::load_scriptlets)
        .as_slice()
}

fn inline_portal_skills() -> &'static [Arc<crate::plugins::PluginSkill>] {
    static CACHE: OnceLock<Vec<Arc<crate::plugins::PluginSkill>>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            crate::plugins::discover_plugins()
                .ok()
                .and_then(|index| crate::plugins::discover_plugin_skills(&index).ok())
                .unwrap_or_default()
                .into_iter()
                .map(Arc::new)
                .collect()
        })
        .as_slice()
}

fn collect_script_list_inline_items(
    inline_query: &InlinePortalQuery,
    items: &mut Vec<ContextSelectorRow>,
) {
    let results = crate::scripts::fuzzy_search_unified_all_with_skills(
        inline_portal_scripts(),
        inline_portal_scriptlets(),
        &[],
        &[],
        inline_portal_skills(),
        &inline_query.query,
    );

    for result in results
        .into_iter()
        .filter(|result| script_list_result_matches_portal_kind(inline_query.kind, result))
        .take(INLINE_PORTAL_RESULTS_LIMIT)
    {
        if let Some(item) = inline_portal_item_from_search_result(inline_query.kind, result) {
            items.push(item);
        }
    }
}

fn script_list_result_matches_portal_kind(
    kind: ContextPortalKind,
    result: &crate::scripts::SearchResult,
) -> bool {
    matches!(
        (kind, result),
        (
            ContextPortalKind::ScriptSearch,
            crate::scripts::SearchResult::Script(_)
        ) | (
            ContextPortalKind::ScriptletSearch,
            crate::scripts::SearchResult::Scriptlet(_)
        ) | (
            ContextPortalKind::SkillSearch,
            crate::scripts::SearchResult::Skill(_)
        )
    )
}

fn inline_portal_item_from_search_result(
    portal_kind: ContextPortalKind,
    result: crate::scripts::SearchResult,
) -> Option<ContextSelectorRow> {
    let query_prefix = portal_prefix_for_kind(portal_kind);
    match result {
        crate::scripts::SearchResult::Script(script_match) => {
            let path = script_match.script.path.to_string_lossy().to_string();
            let label = script_match.script.name.clone();
            let description = script_match
                .script
                .description
                .clone()
                .unwrap_or_else(|| path.clone());
            Some(ContextSelectorRow {
                id: SharedString::from(format!("portal-result:script:{path}")),
                label: SharedString::from(label.clone()),
                description: SharedString::from(description),
                meta: SharedString::from(format!("@{query_prefix}:{}", label)),
                kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                    portal_kind,
                    attachment: InlinePortalAttachment::FilePath { path, label },
                }),
                score: (300 + script_match.score.max(0) as u32).max(300),
                label_highlight_indices: script_match.match_indices.name_indices,
                meta_highlight_indices: Vec::new(),
            })
        }
        crate::scripts::SearchResult::Scriptlet(scriptlet_match) => {
            let label = scriptlet_match.scriptlet.name.clone();
            let semantic_id = scriptlet_match.scriptlet.launcher_command_id();
            Some(ContextSelectorRow {
                id: SharedString::from(format!("portal-result:scriptlet:{semantic_id}")),
                label: SharedString::from(label.clone()),
                description: SharedString::from(
                    scriptlet_match
                        .scriptlet
                        .description
                        .clone()
                        .unwrap_or_else(|| "Scriptlet".to_string()),
                ),
                meta: SharedString::from(format!("@{query_prefix}:{}", label)),
                kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                    portal_kind,
                    attachment: InlinePortalAttachment::FocusedTarget {
                        source: "ScriptList".to_string(),
                        kind: "scriptlet".to_string(),
                        semantic_id,
                        label: label.clone(),
                        metadata: Some(serde_json::json!({
                            "name": scriptlet_match.scriptlet.name,
                            "description": scriptlet_match.scriptlet.description,
                            "tool": scriptlet_match.scriptlet.tool,
                            "code": scriptlet_match.scriptlet.code,
                            "filePath": scriptlet_match.scriptlet.file_path,
                            "pluginId": scriptlet_match.scriptlet.plugin_id,
                            "pluginTitle": scriptlet_match.scriptlet.plugin_title,
                        })),
                    },
                }),
                score: (300 + scriptlet_match.score.max(0) as u32).max(300),
                label_highlight_indices: scriptlet_match.match_indices.name_indices,
                meta_highlight_indices: Vec::new(),
            })
        }
        crate::scripts::SearchResult::Skill(skill_match) => {
            let owner = if skill_match.skill.plugin_title.is_empty() {
                skill_match.skill.plugin_id.clone()
            } else {
                skill_match.skill.plugin_title.clone()
            };
            Some(ContextSelectorRow {
                id: SharedString::from(format!(
                    "portal-result:skill:{}:{}",
                    skill_match.skill.plugin_id, skill_match.skill.skill_id
                )),
                label: SharedString::from(skill_match.skill.title.clone()),
                description: SharedString::from(skill_match.skill.description.clone()),
                meta: SharedString::from(format!("@{query_prefix}:{}", skill_match.skill.title)),
                kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                    portal_kind,
                    attachment: InlinePortalAttachment::SkillFile {
                        path: skill_match.skill.path.to_string_lossy().to_string(),
                        label: skill_match.skill.title.clone(),
                        skill_name: skill_match.skill.title.clone(),
                        owner_label: owner,
                        slash_name: skill_match.skill.skill_id.clone(),
                    },
                }),
                score: (300 + skill_match.score.max(0) as u32).max(300),
                label_highlight_indices: skill_match.match_indices.name_indices,
                meta_highlight_indices: Vec::new(),
            })
        }
        _ => None,
    }
}

fn collect_browser_history_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    let Ok(entries) = crate::browser_history::list_recent_history(200) else {
        return;
    };
    collect_browser_history_inline_items_from_entries(query, entries, items);
}

fn collect_browser_history_inline_items_from_entries(
    query: &str,
    entries: Vec<crate::browser_history::BrowserHistoryEntry>,
    items: &mut Vec<ContextSelectorRow>,
) {
    let query_lower = query.trim().to_lowercase();
    let matches = crate::browser_history::fuzzy_search_browser_history(&entries, query);
    for matched in matches.into_iter().take(INLINE_PORTAL_RESULTS_LIMIT) {
        let entry = matched.entry;
        let label = entry.display_title().to_string();
        let meta = format!("@browser-history:{}", entry.host);
        items.push(ContextSelectorRow {
            id: SharedString::from(format!(
                "portal-result:browser-history:{}",
                entry.history_key()
            )),
            label: SharedString::from(label.clone()),
            description: SharedString::from(entry.url.clone()),
            meta: SharedString::from(meta),
            kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                portal_kind: ContextPortalKind::BrowserHistory,
                attachment: InlinePortalAttachment::FocusedTarget {
                    source: "BrowserHistory".to_string(),
                    kind: "browser_history_entry".to_string(),
                    semantic_id: entry.history_key(),
                    label: label.clone(),
                    metadata: Some(serde_json::json!({
                        "browserName": entry.browser_name,
                        "browserBundleId": entry.browser_bundle_id,
                        "title": entry.title,
                        "url": entry.url,
                        "host": entry.host,
                        "profile": entry.profile,
                        "lastVisitedAtMs": entry.last_visited_at_ms,
                        "visitCount": entry.visit_count,
                    })),
                },
            }),
            score: (300 + matched.score.max(0) as u32).max(300),
            label_highlight_indices: match_query_chars(&query_lower, &label).unwrap_or_default(),
            meta_highlight_indices: Vec::new(),
        });
    }
}

fn collect_browser_tabs_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    let Ok(tabs) = crate::browser_tabs::list_open_tabs() else {
        return;
    };
    collect_browser_tabs_inline_items_from_tabs(query, tabs, items);
}

fn collect_browser_tabs_inline_items_from_tabs(
    query: &str,
    tabs: Vec<crate::browser_tabs::BrowserTabInfo>,
    items: &mut Vec<ContextSelectorRow>,
) {
    let query_lower = query.trim().to_lowercase();
    let matches = crate::browser_tabs::fuzzy_search_browser_tabs(&tabs, query);
    for (index, matched) in matches
        .into_iter()
        .take(INLINE_PORTAL_RESULTS_LIMIT)
        .enumerate()
    {
        let tab = matched.tab;
        let label = tab.display_title().to_string();
        let stable_key = crate::browser_tabs::browser_tab_stable_key(&tab);
        let host = crate::browser_tabs::browser_tab_host(&tab);
        items.push(ContextSelectorRow {
            id: SharedString::from(format!("portal-result:browser-tabs:{stable_key}")),
            label: SharedString::from(label.clone()),
            description: SharedString::from(tab.url.to_string()),
            meta: SharedString::from(format!("@tabs:{host}")),
            kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                portal_kind: ContextPortalKind::BrowserTabs,
                attachment: InlinePortalAttachment::FocusedTarget {
                    source: "BrowserTabs".to_string(),
                    kind: "browser_tab".to_string(),
                    semantic_id: crate::protocol::generate_semantic_id(
                        "browser-tab",
                        index,
                        &stable_key,
                    ),
                    label: label.clone(),
                    metadata: Some(serde_json::json!({
                        "browserName": tab.browser_name,
                        "browserBundleId": tab.browser_bundle_id,
                        "windowIndex": tab.window_index,
                        "tabIndex": tab.tab_index,
                        "title": tab.title,
                        "url": tab.url,
                        "host": host,
                        "stableKey": stable_key,
                    })),
                },
            }),
            score: (300 + matched.score.max(0) as u32).max(300),
            label_highlight_indices: match_query_chars(&query_lower, &label).unwrap_or_default(),
            meta_highlight_indices: Vec::new(),
        });
    }
}

fn collect_clipboard_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    let entries = crate::clipboard_history::get_cached_entries(200);
    collect_clipboard_inline_items_from_entries(query, entries, items);
}

fn collect_clipboard_inline_items_from_entries(
    query: &str,
    entries: Vec<crate::clipboard_history::ClipboardEntryMeta>,
    items: &mut Vec<ContextSelectorRow>,
) {
    let query_lower = query.trim().to_lowercase();
    entries
        .into_iter()
        .filter(crate::clipboard_history::root_clipboard_entry_is_eligible)
        .filter(|entry| {
            query_lower.is_empty() || entry.text_preview.to_lowercase().contains(&query_lower)
        })
        .take(INLINE_PORTAL_RESULTS_LIMIT)
        .for_each(|entry| {
            let label = entry.display_preview();
            items.push(ContextSelectorRow {
                id: SharedString::from(format!("portal-result:clipboard:{}", entry.id)),
                label: SharedString::from(label.clone()),
                description: SharedString::from(format!(
                    "{} clipboard entry",
                    entry.content_type.as_str()
                )),
                meta: SharedString::from(format!("@clipboard:{}", entry.id)),
                kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                    portal_kind: ContextPortalKind::ClipboardHistory,
                    attachment: InlinePortalAttachment::ResourceUri {
                        uri: format!("kit://clipboard-history?id={}", entry.id),
                        label: format!("Clipboard: {label}"),
                    },
                }),
                score: if query_lower.is_empty() { 300 } else { 420 },
                label_highlight_indices: match_query_chars(&query_lower, &label)
                    .unwrap_or_default(),
                meta_highlight_indices: Vec::new(),
            });
        });
}

fn collect_dictation_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    let query_lower = query.trim().to_lowercase();
    for hit in crate::dictation::search_history(query, INLINE_PORTAL_RESULTS_LIMIT) {
        let label = hit.entry.preview.clone();
        items.push(ContextSelectorRow {
            id: SharedString::from(format!("portal-result:dictation:{}", hit.entry.id)),
            label: SharedString::from(label.clone()),
            description: SharedString::from(format!(
                "Dictation to {}",
                hit.entry.display_target_label()
            )),
            meta: SharedString::from(format!("@dictation:{}", hit.entry.id)),
            kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                portal_kind: ContextPortalKind::DictationHistory,
                attachment: InlinePortalAttachment::ResourceUri {
                    uri: format!("kit://dictation-history?id={}", hit.entry.id),
                    label: format!("Dictation: {label}"),
                },
            }),
            score: (300 + hit.score).max(300),
            label_highlight_indices: match_query_chars(&query_lower, &label).unwrap_or_default(),
            meta_highlight_indices: Vec::new(),
        });
    }
}

fn collect_notes_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    // Always read fresh from storage so the picker reflects creates, renames,
    // and deletes that happened outside the Agent Chat composer. For blank queries
    // prefer `get_all_notes()` so the picker shows the current note list
    // even if the FTS index is mid-rebuild.
    let trimmed = query.trim();
    let result = if trimmed.is_empty() {
        crate::notes::get_all_notes()
    } else {
        crate::notes::search_notes(query)
    };
    let Ok(notes) = result else {
        return;
    };
    let query_lower = trimmed.to_lowercase();
    for note in notes.into_iter().take(INLINE_PORTAL_RESULTS_LIMIT) {
        let title = if note.title.trim().is_empty() {
            "Untitled Note".to_string()
        } else {
            note.title.clone()
        };
        let semantic_id = note.id;
        let note_title = note.title;
        let content = note.content;
        let updated_at = note.updated_at.to_rfc3339();
        let is_pinned = note.is_pinned;
        items.push(ContextSelectorRow {
            id: SharedString::from(format!("portal-result:note:{semantic_id}")),
            label: SharedString::from(title.clone()),
            description: SharedString::from(format!("{} chars", content.chars().count())),
            meta: SharedString::from(format!("@note:{title}")),
            kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                portal_kind: ContextPortalKind::NotesBrowse,
                attachment: InlinePortalAttachment::FocusedTarget {
                    source: "NotesBrowse".to_string(),
                    kind: "note".to_string(),
                    semantic_id: semantic_id.to_string(),
                    label: title.clone(),
                    metadata: Some(serde_json::json!({
                        "id": semantic_id,
                        "title": note_title,
                        "content": content,
                        "updatedAt": updated_at,
                        "isPinned": is_pinned,
                    })),
                },
            }),
            score: 300,
            label_highlight_indices: match_query_chars(&query_lower, &title).unwrap_or_default(),
            meta_highlight_indices: Vec::new(),
        });
    }
}

fn collect_agent_chat_history_inline_items(query: &str, items: &mut Vec<ContextSelectorRow>) {
    let query_lower = query.trim().to_lowercase();
    for hit in
        crate::ai::agent_chat::ui::history::search_history(query, INLINE_PORTAL_RESULTS_LIMIT)
    {
        let title = hit.entry.title_display().to_string();
        let preview = hit.entry.preview_display().to_string();
        let session_id = hit.entry.session_id.clone();
        let first_message = hit.entry.first_message;
        let message_count = hit.entry.message_count;
        let timestamp = hit.entry.timestamp;
        items.push(ContextSelectorRow {
            id: SharedString::from(format!("portal-result:history:{session_id}")),
            label: SharedString::from(title.clone()),
            description: SharedString::from(preview.clone()),
            meta: SharedString::from(format!("@history:{session_id}")),
            kind: ContextSelectorRowKind::PortalResult(InlinePortalResultPayload {
                portal_kind: ContextPortalKind::AgentChatHistory,
                attachment: InlinePortalAttachment::FocusedTarget {
                    source: "AgentChatHistory".to_string(),
                    kind: "agent_chatHistory".to_string(),
                    semantic_id: session_id.clone(),
                    label: title.clone(),
                    metadata: Some(serde_json::json!({
                        "sessionId": session_id,
                        "title": title.clone(),
                        "preview": preview.clone(),
                        "firstMessage": first_message,
                        "messageCount": message_count,
                        "timestamp": timestamp,
                    })),
                },
            }),
            score: (300 + hit.score).max(300),
            label_highlight_indices: match_query_chars(&query_lower, &title).unwrap_or_default(),
            meta_highlight_indices: match_query_chars(&query_lower, &preview).unwrap_or_default(),
        });
    }
}

/// Populate `items` with agent slash command entries (e.g. `/compact`,
/// `/clear`). Shared by Agent Chat and any future slash-command surface.
///
/// Uses bare `(name, description)` pairs — all entries get `Default` payload.
fn extend_agent_slash_command_items<'a, I>(
    query_lower: &str,
    commands: I,
    items: &mut Vec<ContextSelectorRow>,
) where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    use types::SlashCommandPayload;
    let payloads: Vec<(SlashCommandPayload, String)> = commands
        .into_iter()
        .map(|(name, desc)| {
            (
                SlashCommandPayload::Default {
                    name: name.to_string(),
                },
                desc.to_string(),
            )
        })
        .collect();
    extend_agent_slash_command_items_with_payloads(
        query_lower,
        payloads.iter().map(|(p, d)| (p, d.as_str())),
        items,
    );
}

/// Populate `items` with source-aware slash command entries.
///
/// Each entry carries a `SlashCommandPayload` so duplicate skill slugs
/// from different plugins produce rows with distinct stable IDs.
fn extend_agent_slash_command_items_with_payloads<'a, I>(
    query_lower: &str,
    commands: I,
    items: &mut Vec<ContextSelectorRow>,
) where
    I: IntoIterator<Item = (&'a types::SlashCommandPayload, &'a str)>,
{
    for (payload, description) in commands {
        let name = payload.slash_name();
        let name_lower = name.to_lowercase();
        let score = if query_lower.is_empty() {
            50
        } else if name_lower.starts_with(query_lower) {
            90
        } else if name_lower.contains(query_lower) {
            50
        } else if match_query_chars(query_lower, &name_lower).is_some() {
            10
        } else {
            continue;
        };

        let meta_str = payload.picker_owner_meta();
        let label_hits = if query_lower.is_empty() {
            Vec::new()
        } else {
            match_query_chars(query_lower, name).unwrap_or_default()
        };
        let meta_hits = if query_lower.is_empty() {
            Vec::new()
        } else {
            match_query_chars_in_display_meta(query_lower, &meta_str).unwrap_or_default()
        };

        let safe_item_id = crate::logging::log_private_user_value(&payload.stable_id());
        let safe_name = crate::logging::log_private_user_value(name);
        let safe_owner = crate::logging::log_private_user_value(&payload.owner_label());
        let safe_meta = crate::logging::log_private_user_value(&meta_str);
        tracing::debug!(
            item_id_bytes = safe_item_id.raw_bytes,
            item_id_sha256 = %safe_item_id.sha256,
            slash_name_bytes = safe_name.raw_bytes,
            slash_name_sha256 = %safe_name.sha256,
            owner_bytes = safe_owner.raw_bytes,
            owner_sha256 = %safe_owner.sha256,
            meta_bytes = safe_meta.raw_bytes,
            meta_sha256 = %safe_meta.sha256,
            "agent_chat_slash_picker_entry_built"
        );

        items.push(ContextSelectorRow {
            id: SharedString::from(format!("slash-cmd:{}", payload.stable_id())),
            label: SharedString::from(name.to_string()),
            description: SharedString::from(slash_command_description(name, description)),
            meta: SharedString::from(meta_str),
            kind: ContextSelectorRowKind::SlashCommand(payload.clone()),
            score,
            label_highlight_indices: label_hits,
            meta_highlight_indices: meta_hits,
        });
    }
}

fn slash_command_description(name: &str, discovered_description: &str) -> String {
    let trimmed = discovered_description.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    match name {
        "compact" => "Compact the conversation to reduce context usage.".to_string(),
        "clear" => "Clear the current conversation from the composer.".to_string(),
        "bug" => "Report a problem with the current session.".to_string(),
        "help" => "Show slash command help and usage guidance.".to_string(),
        "init" => "Initialize the current workspace for the agent.".to_string(),
        "login" => "Authenticate the current agent session.".to_string(),
        "logout" => "Sign out of the current agent session.".to_string(),
        "status" => "Show the current session and account status.".to_string(),
        "cost" => "Show current usage and cost details.".to_string(),
        "doctor" => "Run diagnostics for the current agent setup.".to_string(),
        "review" => "Ask the agent to review the current work.".to_string(),
        "memory" => "Inspect or manage the agent memory store.".to_string(),
        _ => format!("Run /{name}."),
    }
}

/// Sort items by section priority then score (descending).
fn sort_picker_items(items: &mut [ContextSelectorRow]) {
    items.sort_by(|a, b| {
        let section_a = section_priority(&a.kind);
        let section_b = section_priority(&b.kind);
        section_a
            .cmp(&section_b)
            .then(b.score.cmp(&a.score))
            .then_with(|| a.id.as_ref().cmp(b.id.as_ref()))
    });
}

/// Log the top ranked items for debugging.
fn log_top_ranked_items(items: &[ContextSelectorRow]) {
    for (rank, item) in items.iter().enumerate().take(5) {
        let safe_item_id = crate::logging::log_private_user_value(item.id.as_ref());
        tracing::debug!(
            target: "ai",
            rank,
            item_id_bytes = safe_item_id.raw_bytes,
            item_id_sha256 = %safe_item_id.sha256,
            score = item.score,
            label_hits = ?item.label_highlight_indices,
            meta_hits = ?item.meta_highlight_indices,
            "ai_context_selector_ranked_item"
        );
    }
}

/// Build the ranked list of picker items for a given trigger and query.
///
/// Uses the cached `BuiltinPickerSeed` catalog to avoid per-query
/// lowercasing and metadata reconstruction. File results are only
/// included when the query resolves to explicit `@file:` intent.
pub fn context_selector_rows(
    trigger: ContextSelectorTrigger,
    query: &str,
) -> Vec<ContextSelectorRow> {
    let query_lower = query.to_lowercase();
    let mut items = Vec::with_capacity(builtin_picker_seeds().len() + FILE_RESULTS_LIMIT);

    extend_builtin_picker_items(trigger, query, &query_lower, &mut items);
    sort_picker_items(&mut items);

    let safe_query = crate::logging::log_private_user_value(query);
    tracing::debug!(
        target: "ai",
        ?trigger,
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        item_count = items.len(),
        "ai_context_selector_items_built"
    );
    log_top_ranked_items(&items);

    items
}

/// Build a ranked list of picker items for slash mode using only agent slash
/// commands.
///
/// Slash mode is command-only. Context attachments belong behind `@`.
pub fn slash_command_rows<'a, I>(query: &str, agent_commands: I) -> Vec<ContextSelectorRow>
where
    I: IntoIterator<Item = &'a str>,
{
    slash_command_rows_with_descriptions(query, agent_commands.into_iter().map(|name| (name, "")))
}

pub fn slash_command_rows_with_descriptions<'a, I>(
    query: &str,
    agent_commands: I,
) -> Vec<ContextSelectorRow>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let query_lower = query.to_lowercase();
    let commands: Vec<(&str, &str)> = agent_commands.into_iter().collect();
    let command_count = commands.len();
    let mut items = Vec::with_capacity(command_count);

    extend_agent_slash_command_items(&query_lower, commands, &mut items);
    sort_picker_items(&mut items);

    let safe_query = crate::logging::log_private_user_value(query);
    tracing::info!(
        target: "ai",
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        command_count,
        item_count = items.len(),
        "ai_context_selector_slash_items_built"
    );
    log_top_ranked_items(&items);

    items
}

/// Build a ranked list of picker items for slash mode using source-aware
/// payloads. Each payload carries plugin/Claude ownership so duplicate
/// skill slugs produce rows with distinct stable IDs.
pub fn slash_command_rows_with_payloads<'a, I>(
    query: &str,
    payload_commands: I,
) -> Vec<ContextSelectorRow>
where
    I: IntoIterator<Item = (&'a types::SlashCommandPayload, &'a str)>,
{
    let query_lower = query.to_lowercase();
    let commands: Vec<(&types::SlashCommandPayload, &str)> = payload_commands.into_iter().collect();
    let command_count = commands.len();
    let mut items = Vec::with_capacity(command_count);

    extend_agent_slash_command_items_with_payloads(&query_lower, commands, &mut items);
    sort_picker_items(&mut items);

    let safe_query = crate::logging::log_private_user_value(query);
    tracing::info!(
        target: "ai",
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        command_count,
        item_count = items.len(),
        "ai_context_selector_slash_items_built"
    );
    log_top_ranked_items(&items);

    items
}

/// Score a built-in spec against the user query (mention mode).
pub fn score_builtin(
    spec: &crate::ai::context_contract::ContextAttachmentSpec,
    query: &str,
) -> u32 {
    score_builtin_seed(
        builtin_seed(spec.kind),
        ContextSelectorTrigger::Mention,
        &query.to_lowercase(),
    )
    .0
}

/// Score a built-in spec against the user query with a specific trigger mode.
pub fn score_builtin_with_trigger(
    spec: &crate::ai::context_contract::ContextAttachmentSpec,
    trigger: ContextSelectorTrigger,
    query: &str,
) -> (u32, Vec<usize>, Vec<usize>) {
    score_builtin_seed(builtin_seed(spec.kind), trigger, &query.to_lowercase())
}

/// Collect file and folder items from the given directory matching the query.
fn collect_file_items(dir: &std::path::Path, raw_query: &str, items: &mut Vec<ContextSelectorRow>) {
    let (search_dir, name_filter) = split_file_query(dir, raw_query);

    let read_dir = match std::fs::read_dir(&search_dir) {
        Ok(rd) => rd,
        Err(error) => {
            let safe_query = crate::logging::log_private_user_value(raw_query);
            let safe_directory =
                crate::logging::log_private_user_value(&search_dir.display().to_string());
            let safe_error = crate::logging::log_private_user_value(&error.to_string());
            tracing::debug!(
                target: "ai",
                query_bytes = safe_query.raw_bytes,
                query_sha256 = %safe_query.sha256,
                directory_bytes = safe_directory.raw_bytes,
                directory_sha256 = %safe_directory.sha256,
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
                "ai_context_selector_file_scan_failed"
            );
            return;
        }
    };

    let mut entries: Vec<_> = read_dir.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());

    let mut file_count = 0usize;
    let mut folder_count = 0usize;

    for entry in entries {
        if file_count + folder_count >= FILE_RESULTS_LIMIT {
            break;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let name_lower = name.to_lowercase();
        if !name_filter.is_empty() && !name_lower.contains(&name_filter) {
            continue;
        }

        let path = entry.path();
        let is_dir = path.is_dir();

        let score = if name_filter.is_empty() || name_lower.starts_with(&name_filter) {
            200
        } else {
            100
        };

        let meta = format!("@file:{}", path.display());

        let item = ContextSelectorRow {
            id: SharedString::from(format!(
                "{}:{}",
                if is_dir { "folder" } else { "file" },
                path.display()
            )),
            label: SharedString::from(name.clone()),
            description: SharedString::from(path.display().to_string()),
            meta: SharedString::from(meta.clone()),
            kind: if is_dir {
                ContextSelectorRowKind::Folder(path.clone())
            } else {
                ContextSelectorRowKind::File(path.clone())
            },
            score,
            label_highlight_indices: if name_filter.is_empty() {
                Vec::new()
            } else {
                match_query_chars(&name_filter, &name).unwrap_or_default()
            },
            meta_highlight_indices: if raw_query.is_empty() {
                Vec::new()
            } else {
                match_query_chars(raw_query, &meta).unwrap_or_default()
            },
        };

        if is_dir {
            if folder_count < FILE_RESULTS_LIMIT / 2 {
                items.push(item);
                folder_count += 1;
            }
        } else if file_count < FILE_RESULTS_LIMIT / 2 {
            items.push(item);
            file_count += 1;
        }
    }

    let safe_query = crate::logging::log_private_user_value(raw_query);
    let safe_directory = crate::logging::log_private_user_value(&search_dir.display().to_string());
    tracing::info!(
        target: "ai",
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        directory_bytes = safe_directory.raw_bytes,
        directory_sha256 = %safe_directory.sha256,
        file_count,
        folder_count,
        "ai_context_selector_file_scan_complete"
    );
}

/// Map item kind to section priority for stable sort grouping.
fn section_priority(kind: &ContextSelectorRowKind) -> u8 {
    match kind {
        ContextSelectorRowKind::BuiltIn(_) => 0,
        ContextSelectorRowKind::Portal(_) => 0,
        ContextSelectorRowKind::PortalPrefix(_) | ContextSelectorRowKind::PortalResult(_) => 1,
        ContextSelectorRowKind::SlashCommand(_) => 2,
        ContextSelectorRowKind::AgentChatProfile { .. } => 2,
        ContextSelectorRowKind::File(_) => 3,
        ContextSelectorRowKind::Folder(_) => 4,
        // Inert rows (loading / empty state) sort last so live results
        // always appear above them.
        ContextSelectorRowKind::Inert => 255,
    }
}

// ── Slash picker loading and empty-state rows ───────────────────────

/// Build a non-actionable "Discovering plugin skills…" placeholder row.
///
/// Shown when the Agent Chat slash picker opens before async discovery completes
/// (i.e. `cached_slash_commands` is still empty).
pub(crate) fn slash_command_loading_row() -> ContextSelectorRow {
    tracing::debug!(
        event = "agent_chat_slash_picker_loading",
        "Building slash picker loading row"
    );
    ContextSelectorRow {
        id: SharedString::from("slash-loading"),
        label: SharedString::from("Discovering plugin skills\u{2026}"),
        description: SharedString::from("Scanning installed plugins and Claude Code skills"),
        meta: SharedString::from(""),
        kind: ContextSelectorRowKind::Inert,
        score: 0,
        label_highlight_indices: Vec::new(),
        meta_highlight_indices: Vec::new(),
    }
}

/// Build a non-actionable "No slash commands or skills found" row.
///
/// Shown when async discovery completed but the catalog is empty
/// (no defaults, no plugins, no Claude skills were found).
pub(crate) fn slash_command_empty_row() -> ContextSelectorRow {
    tracing::debug!(
        event = "agent_chat_slash_picker_empty_state",
        "Building slash picker empty row"
    );
    ContextSelectorRow {
        id: SharedString::from("slash-empty"),
        label: SharedString::from("No slash commands or skills found"),
        description: SharedString::from("Install a plugin skill or try a built-in slash command"),
        meta: SharedString::from(""),
        kind: ContextSelectorRowKind::Inert,
        score: 0,
        label_highlight_indices: Vec::new(),
        meta_highlight_indices: Vec::new(),
    }
}

/// Build a non-actionable "No matching skills or commands" row.
///
/// Shown when the discovered catalog is non-empty but the current query
/// filters every entry to zero. This is distinct from the empty catalog
/// state (`slash_command_empty_row`) and the loading state
/// (`slash_command_loading_row`).
pub(crate) fn slash_command_no_match_row() -> ContextSelectorRow {
    tracing::debug!(
        event = "agent_chat_slash_picker_no_match",
        "Building slash picker no-match row"
    );
    ContextSelectorRow {
        id: SharedString::from("slash-no-match"),
        label: SharedString::from("No matching skills or commands"),
        description: SharedString::from(
            "Try another slash name or choose a different plugin skill",
        ),
        meta: SharedString::from(""),
        kind: ContextSelectorRowKind::Inert,
        score: 0,
        label_highlight_indices: Vec::new(),
        meta_highlight_indices: Vec::new(),
    }
}

#[cfg(test)]
include!("mod_tests.rs");

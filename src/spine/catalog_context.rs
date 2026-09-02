use std::ops::Range;

use crate::ai::context_contract::{
    context_attachment_specs, ContextAttachmentKind, ContextAttachmentSpec,
};

use super::list::{ss, SpineListAction, SpineListRow, SpineListRowKind};

#[derive(Debug, Clone, Copy)]
struct ContextSubsearchSpec {
    prefix: &'static str,
    title: &'static str,
    subtitle: &'static str,
    icon: &'static str,
}

const CONTEXT_SUBSEARCH_SPECS: &[ContextSubsearchSpec] = &[
    ContextSubsearchSpec {
        prefix: "file",
        title: "Files",
        subtitle: "Search files",
        icon: "file-search",
    },
    ContextSubsearchSpec {
        prefix: "project",
        title: "Project Files",
        subtitle: "Search files in the working directory",
        icon: "folder",
    },
    ContextSubsearchSpec {
        prefix: "clipboard",
        title: "Clipboard History",
        subtitle: "Search clipboard history",
        icon: "clipboard",
    },
    ContextSubsearchSpec {
        prefix: "browser-history",
        title: "Browser History",
        subtitle: "Search browser history inline",
        icon: "globe",
    },
    ContextSubsearchSpec {
        prefix: "notes",
        title: "Notes",
        subtitle: "Search notes",
        icon: "notebook-text",
    },
    ContextSubsearchSpec {
        prefix: "history",
        title: "Agent Chat History",
        subtitle: "Search past conversations",
        icon: "message-circle",
    },
    ContextSubsearchSpec {
        prefix: "scripts",
        title: "Scripts",
        subtitle: "Search Script Kit scripts",
        icon: "file-code",
    },
    ContextSubsearchSpec {
        prefix: "dictation",
        title: "Dictation History",
        subtitle: "Search saved dictation",
        icon: "mic",
    },
    ContextSubsearchSpec {
        prefix: "scriptlets",
        title: "Scriptlets",
        subtitle: "Search snippets",
        icon: "scroll-text",
    },
    ContextSubsearchSpec {
        prefix: "skills",
        title: "Skills",
        subtitle: "Search plugin skills",
        icon: "workflow",
    },
    ContextSubsearchSpec {
        prefix: "calendar",
        title: "Calendar Events",
        subtitle: "Search calendar events",
        icon: "calendar",
    },
    ContextSubsearchSpec {
        prefix: "notifications",
        title: "Notifications",
        subtitle: "Search notifications",
        icon: "bell",
    },
];

const PENALTY_EXACT: i32 = 0;
const PENALTY_PREFIX: i32 = 100;
const PENALTY_SUBSTRING: i32 = 1000;
const PENALTY_FUZZY: i32 = 5000;

const CATEGORY_PENALTY_BUILTIN: i32 = 0;
const CATEGORY_PENALTY_SUBSEARCH: i32 = 50;

fn truncate_display(value: &str, max_chars: usize) -> String {
    let compact: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }
    let mut truncated: String = compact.chars().take(max_chars.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

fn normalized_context_query(query: &str) -> String {
    query
        .trim()
        .trim_start_matches(['@', '/', '|', '.', ';'])
        .to_ascii_lowercase()
}

fn context_value_match_penalty(value: &str, normalized_query: &str) -> Option<i32> {
    if normalized_query.is_empty() {
        return Some(PENALTY_EXACT);
    }
    let value_lower = value.to_ascii_lowercase();
    let trimmed = value_lower.trim_start_matches(['@', '/', '|', '.', ';']);
    if trimmed == normalized_query {
        Some(PENALTY_EXACT)
    } else if trimmed.starts_with(normalized_query) {
        Some(PENALTY_PREFIX)
    } else if value_lower.contains(normalized_query) {
        Some(PENALTY_SUBSTRING)
    } else if crate::scripts::search::is_fuzzy_match(&value_lower, normalized_query) {
        Some(PENALTY_FUZZY)
    } else {
        None
    }
}

fn context_row_score(match_penalty: i32, category_penalty: i32, rank: usize) -> i32 {
    i32::MAX
        .saturating_sub(match_penalty)
        .saturating_sub(category_penalty)
        .saturating_sub(rank as i32)
}

pub(super) fn build_context_root_rows(
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
) -> Vec<SpineListRow> {
    build_context_root_rows_with_preview(query, segment_index, segment_byte_range, None)
}

pub(super) fn build_context_root_rows_with_preview(
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
) -> Vec<SpineListRow> {
    let mut rows = Vec::new();

    for (rank, spec) in context_attachment_specs().iter().enumerate() {
        if let Some(row) = build_builtin_context_row(
            spec,
            rank,
            query,
            segment_index,
            segment_byte_range.clone(),
            live_preview,
        ) {
            rows.push(row);
        }
    }

    for (rank, spec) in CONTEXT_SUBSEARCH_SPECS.iter().enumerate() {
        if let Some(row) = build_subsearch_context_row(
            spec,
            rank,
            query,
            segment_index,
            segment_byte_range.clone(),
        ) {
            rows.push(row);
        }
    }

    rows.sort_by_key(|a| std::cmp::Reverse(a.score));
    rows
}

fn build_builtin_context_row(
    spec: &ContextAttachmentSpec,
    rank: usize,
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
) -> Option<SpineListRow> {
    let mention = spec.mention?;
    let normalized_query = normalized_context_query(query);
    let match_penalty = context_spec_match_penalty(spec, &normalized_query)?;

    let slug = mention_slug(mention);

    let title = spec.label.to_string();

    let subtitle = live_preview
        .map(|lp| {
            let s = lp.subtitle_for_context_kind(spec.kind);
            truncate_display(&s, 76)
        })
        .unwrap_or_else(|| truncate_display(spec.action_title, 76));

    Some(SpineListRow {
        id: ss(format!("spine:@:builtin:{slug}")),
        kind: SpineListRowKind::ContextBuiltin {
            context_type: ss(slug),
        },
        title: ss(title),
        subtitle: Some(ss(subtitle)),
        meta: None,
        icon: Some(ss(icon_for_context_kind(spec.kind))),
        badges: vec![],
        score: context_row_score(match_penalty, CATEGORY_PENALTY_BUILTIN, rank),
        is_selectable: true,
        action_label: None,
        action: SpineListAction::ResolveSegment {
            segment_index,
            segment_byte_range,
            replacement: ss(mention),
            resolution_id: ss(spec.action_id),
            resolution_label: ss(spec.label),
            resolution_source: ss("context-builtin"),
            trailing_space: true,
        },
    })
}

fn build_subsearch_context_row(
    spec: &ContextSubsearchSpec,
    rank: usize,
    query: &str,
    segment_index: usize,
    segment_byte_range: Range<usize>,
) -> Option<SpineListRow> {
    let prefix_text = format!("@{}:", spec.prefix);
    let normalized_query = normalized_context_query(query);
    let match_penalty = subsearch_spec_match_penalty(spec, &prefix_text, &normalized_query)?;

    // A3 decision (2026-06-09): every subsearch row — including Files —
    // completes the segment inline to its `@prefix:` colon mode. Building the
    // prompt must never clear/replace the `@` input; the full File Search
    // portal stays reachable via the explicit "Open full File Search" row
    // inside `@file:` colon mode.
    let action = SpineListAction::InsertSegmentText {
        segment_index,
        segment_byte_range,
        text: ss(prefix_text),
        trailing_space: false,
    };

    Some(SpineListRow {
        id: ss(format!("spine:@:subsearch:{}", spec.prefix)),
        kind: SpineListRowKind::ContextSubSearch {
            context_type: ss(spec.prefix),
        },
        title: ss(spec.title),
        subtitle: Some(ss(spec.subtitle)),
        meta: None,
        icon: Some(ss(spec.icon)),
        badges: vec![],
        score: context_row_score(match_penalty, CATEGORY_PENALTY_SUBSEARCH, rank),
        is_selectable: true,
        action_label: None,
        action,
    })
}

fn context_spec_match_penalty(spec: &ContextAttachmentSpec, normalized_query: &str) -> Option<i32> {
    let direct_values = [
        Some(spec.label),
        Some(spec.action_title),
        Some(spec.action_id),
        spec.mention,
        spec.slash_command,
    ];

    let mut best: Option<i32> = direct_values
        .iter()
        .copied()
        .flatten()
        .filter_map(|value| context_value_match_penalty(value, normalized_query))
        .min();

    for alias in spec.mention_aliases.iter().chain(spec.slash_aliases.iter()) {
        if let Some(penalty) = context_value_match_penalty(alias, normalized_query) {
            best = Some(best.map_or(penalty, |b| b.min(penalty)));
        }
    }

    best
}

fn subsearch_spec_match_penalty(
    spec: &ContextSubsearchSpec,
    prefix_text: &str,
    normalized_query: &str,
) -> Option<i32> {
    [spec.prefix, prefix_text, spec.title, spec.subtitle]
        .iter()
        .filter_map(|value| context_value_match_penalty(value, normalized_query))
        .min()
}

fn mention_slug(mention: &str) -> &str {
    mention.trim_start_matches('@')
}

fn icon_for_context_kind(kind: ContextAttachmentKind) -> &'static str {
    match kind {
        ContextAttachmentKind::Current => "scan-text",
        ContextAttachmentKind::Full => "layers",
        ContextAttachmentKind::Selection => "mouse-pointer-2",
        ContextAttachmentKind::Browser => "globe",
        ContextAttachmentKind::Window => "panel-top",
        ContextAttachmentKind::Diagnostics => "bug",
        ContextAttachmentKind::Screenshot => "image",
        ContextAttachmentKind::Clipboard => "clipboard",
        ContextAttachmentKind::FrontmostApp => "app-window",
        ContextAttachmentKind::MenuBar => "menu",
        ContextAttachmentKind::RecentScripts => "file-code",
        ContextAttachmentKind::GitStatus => "git-branch",
        ContextAttachmentKind::GitDiff => "git-compare",
        ContextAttachmentKind::Processes => "activity",
        ContextAttachmentKind::System => "monitor",
        ContextAttachmentKind::Dictation => "mic",
        ContextAttachmentKind::Calendar => "calendar",
        ContextAttachmentKind::Notifications => "bell",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_prefix_match_scores_above_notifications_substring() {
        let rows = build_context_root_rows("@fi", 0, 0..3);
        let file = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:subsearch:file")
            .expect("expected @file: subsearch row");
        let notifications = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:builtin:notifications")
            .expect("expected @notifications builtin row");
        assert!(
            file.score > notifications.score,
            "@file: (prefix, score={}) must rank above @notifications (substring, score={})",
            file.score,
            notifications.score,
        );
    }

    #[test]
    fn file_subsearch_row_completes_inline_not_portal() {
        // A3 decision (2026-06-09): the Files row must never replace the
        // prompt with the File Search portal; it completes inline to @file:
        // colon mode. The portal stays reachable via the explicit
        // "Open full File Search" row inside colon mode.
        let rows = build_context_root_rows("@file", 0, 0..5);
        let file = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:subsearch:file")
            .expect("expected @file subsearch row");
        assert!(
            matches!(
                &file.action,
                SpineListAction::InsertSegmentText { text, .. } if text.as_ref() == "@file:"
            ),
            "top-level Files row must complete inline to @file:, got {:?}",
            file.action,
        );
    }

    #[test]
    fn project_subsearch_row_completes_inline_to_project_colon() {
        let rows = build_context_root_rows("@proj", 0, 0..5);
        let project = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:subsearch:project")
            .expect("expected @project: subsearch row for @proj");
        assert!(
            matches!(
                &project.action,
                SpineListAction::InsertSegmentText { text, trailing_space: false, .. }
                    if text.as_ref() == "@project:"
            ),
            "Enter on the Project Files row must complete inline to @project:, got {:?}",
            project.action,
        );
    }

    #[test]
    fn files_row_for_at_fi_completes_inline_to_file_colon() {
        let rows = build_context_root_rows("@fi", 0, 0..3);
        let file = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:subsearch:file")
            .expect("expected @file: subsearch row for @fi");
        assert!(
            matches!(
                &file.action,
                SpineListAction::InsertSegmentText { text, trailing_space: false, .. }
                    if text.as_ref() == "@file:"
            ),
            "Enter on the Files row for @fi must complete inline to @file:, got {:?}",
            file.action,
        );
    }

    #[test]
    fn non_file_subsearch_rows_keep_inline_insert_action() {
        let rows = build_context_root_rows("@clipboard", 0, 0..10);
        let clipboard = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:subsearch:clipboard")
            .expect("expected @clipboard subsearch row");
        assert!(matches!(
            clipboard.action,
            SpineListAction::InsertSegmentText { .. }
        ));
    }

    #[test]
    fn exact_mention_beats_prefix() {
        let penalty_exact = context_value_match_penalty("@clipboard", "clipboard").unwrap();
        let penalty_prefix = context_value_match_penalty("@clipboard-extra", "clipboard").unwrap();
        assert!(penalty_exact < penalty_prefix);
    }

    #[test]
    fn prefix_beats_substring() {
        let penalty_prefix = context_value_match_penalty("file", "fi").unwrap();
        let penalty_sub = context_value_match_penalty("notifications", "fi").unwrap();
        assert!(penalty_prefix < penalty_sub);
    }

    #[test]
    fn empty_query_matches_everything() {
        let rows = build_context_root_rows("@", 0, 0..1);
        assert!(!rows.is_empty());
    }

    #[test]
    fn sorted_results_for_at_fi_show_file_first() {
        let mut rows = build_context_root_rows("@fi", 0, 0..3);
        rows.sort_by(|a, b| b.score.cmp(&a.score));
        let titles: Vec<&str> = rows.iter().map(|r| r.title.as_ref()).collect();
        let file_pos = titles
            .iter()
            .position(|t| t.to_ascii_lowercase().contains("file"))
            .expect("@file: row missing");
        let notif_pos = titles
            .iter()
            .position(|t| t.contains("Notif"))
            .expect("Notifications row missing");
        assert!(
            file_pos < notif_pos,
            "@file: at position {} must appear before Notifications at position {}; order: {:?}",
            file_pos,
            notif_pos,
            titles,
        );
    }
}

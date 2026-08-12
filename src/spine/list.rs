use std::fmt::Write as _;
use std::ops::Range;

use gpui::SharedString;

use super::{
    SpineCursorProjection, SpineParse, SpineSegment, SpineSegmentKind, SpineSegmentResolution,
};

pub const SPINE_LIST_MODEL_VERSION: u64 = 6;
pub const SPINE_LIST_RESOLUTION_GENERATION: u64 = 0;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SpineListBuildContext<'a> {
    pub current_cwd: Option<&'a std::path::Path>,
    pub cwd_recents: &'a [std::path::PathBuf],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineListSection {
    pub id: SharedString,
    pub title: SharedString,
    pub subtitle: Option<SharedString>,
    pub icon: Option<SharedString>,
    pub rows: Vec<SpineListRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpineListRow {
    pub id: SharedString,
    pub kind: SpineListRowKind,
    pub title: SharedString,
    pub subtitle: Option<SharedString>,
    pub meta: Option<SharedString>,
    pub icon: Option<SharedString>,
    pub badges: Vec<SharedString>,
    pub score: i32,
    pub is_selectable: bool,
    pub action_label: Option<SharedString>,
    pub action: SpineListAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpineListRowKind {
    ContextBuiltin {
        context_type: SharedString,
    },
    ContextSubSearch {
        context_type: SharedString,
    },
    ContextResult {
        context_type: SharedString,
        result_id: SharedString,
    },
    SlashCommand {
        command: SharedString,
    },
    Profile {
        profile_id: SharedString,
    },
    Style {
        style_id: SharedString,
    },
    CaptureTarget {
        target: SharedString,
    },
    Flow {
        flow_id: SharedString,
    },
    Hint,
    Empty,
}

impl SpineListRowKind {
    pub fn type_label(&self) -> &'static str {
        match self {
            Self::ContextBuiltin { .. } => "Context",
            Self::ContextSubSearch { .. } => "Context Search",
            Self::ContextResult { .. } => "Context Result",
            Self::SlashCommand { .. } => "Command",
            Self::Profile { .. } => "Profile",
            Self::Style { .. } => "Style",
            Self::CaptureTarget { .. } => "Capture",
            Self::Flow { .. } => "Flow",
            Self::Hint => "Hint",
            Self::Empty => "Empty",
        }
    }

    pub fn type_accessory_info(&self) -> (&'static str, &'static str) {
        match self {
            Self::ContextBuiltin { .. } => ("Context", "at-sign"),
            Self::ContextSubSearch { .. } => ("Context Search", "search"),
            Self::ContextResult { .. } => ("Context Result", "paperclip"),
            Self::SlashCommand { .. } => ("Command", "slash"),
            Self::Profile { .. } => ("Profile", "user-round"),
            Self::Style { .. } => ("Style", "sparkles"),
            Self::CaptureTarget { .. } => ("Capture", "inbox"),
            Self::Flow { .. } => ("Flow", "flow"),
            Self::Hint => ("Hint", "info"),
            Self::Empty => ("Empty", "circle"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpineListAction {
    InsertSegmentText {
        segment_index: usize,
        segment_byte_range: Range<usize>,
        text: SharedString,
        trailing_space: bool,
    },
    ResolveSegment {
        segment_index: usize,
        segment_byte_range: Range<usize>,
        replacement: SharedString,
        resolution_id: SharedString,
        resolution_label: SharedString,
        resolution_source: SharedString,
        trailing_space: bool,
    },
    OpenModeExit {
        sigil: char,
        rest: SharedString,
    },
    /// Open the full built-in File Search surface (split preview) as a
    /// ScriptList-hosted attachment portal. Accepting a file resolves the
    /// originating `@file` segment into a compact `@file:basename` token.
    OpenFileSearchPortal {
        segment_index: usize,
        segment_byte_range: Range<usize>,
        query: SharedString,
    },
    Noop,
}

impl SpineListRow {
    pub fn default_action_text(&self) -> &str {
        self.action_label
            .as_ref()
            .map(|label| label.as_ref())
            .unwrap_or(match self.action {
                SpineListAction::Noop => "No Action",
                SpineListAction::OpenModeExit { .. } => "Open",
                SpineListAction::OpenFileSearchPortal { .. } => "Browse",
                // One verb per Enter mechanic: rows that insert text and
                // keep you typing say "Refine"; rows that resolve into a
                // prompt segment say "Attach".
                SpineListAction::InsertSegmentText { .. } => "Refine",
                SpineListAction::ResolveSegment { .. } => "Attach",
            })
    }
}

pub fn is_prompt_builder_segment_kind(kind: &SpineSegmentKind) -> bool {
    matches!(
        kind,
        SpineSegmentKind::ContextMention { .. }
            | SpineSegmentKind::SlashCommand { .. }
            | SpineSegmentKind::Profile { .. }
            | SpineSegmentKind::Style { .. }
            | SpineSegmentKind::ProjectCwd { .. }
    )
}

pub fn parse_has_prompt_builder_segments(parse: &SpineParse) -> bool {
    parse
        .segments
        .iter()
        .any(|segment| is_prompt_builder_segment_kind(&segment.kind))
}

pub(crate) fn ss(value: impl Into<SharedString>) -> SharedString {
    value.into()
}

fn active_segment<'a>(
    parse: &'a SpineParse,
    projection: &SpineCursorProjection,
) -> Option<&'a SpineSegment> {
    parse.segments.get(projection.active_segment_index)
}

pub(super) fn active_segment_range(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
) -> Range<usize> {
    active_segment(parse, projection)
        .map(|segment| segment.byte_range.clone())
        .unwrap_or(0..parse.input.len())
}

fn stripped_query(query: &str) -> String {
    query
        .trim()
        .trim_start_matches(['@', '/', '|', '.', ';'])
        .to_ascii_lowercase()
}

pub(super) fn matches_query(value: &str, query: &str) -> bool {
    let query = stripped_query(query);
    if query.is_empty() {
        return true;
    }
    let value_lower = value.to_ascii_lowercase();
    // Substring first (cheap), then the launcher's fuzzy subsequence match
    // so catalog filtering behaves like the main list (`/rw` → /rewrite).
    value_lower.contains(&query) || crate::scripts::search::is_fuzzy_match(&value_lower, &query)
}

pub(super) fn section_with_empty(
    id: &'static str,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
    icon: Option<SharedString>,
    mut rows: Vec<SpineListRow>,
    empty_title: &'static str,
    empty_subtitle: &'static str,
) -> SpineListSection {
    if rows.is_empty() {
        rows.push(SpineListRow {
            id: ss(format!("{id}:empty")),
            kind: SpineListRowKind::Empty,
            title: ss(empty_title),
            subtitle: Some(ss(empty_subtitle)),
            meta: Some(ss("Spine")),
            icon: Some(ss("circle")),
            badges: vec![],
            score: i32::MIN,
            is_selectable: false,
            action_label: None,
            action: SpineListAction::Noop,
        });
    }
    SpineListSection {
        id: ss(id),
        title: title.into(),
        subtitle,
        icon,
        rows,
    }
}

pub fn build_spine_list_sections(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
) -> Vec<SpineListSection> {
    build_spine_list_sections_with_context(parse, projection)
}

pub(crate) fn build_spine_list_sections_with_context(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
) -> Vec<SpineListSection> {
    build_spine_list_sections_full(parse, projection, None)
}

pub(crate) fn build_spine_list_sections_full(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
) -> Vec<SpineListSection> {
    build_spine_list_sections_full_with_resolved_tokens(parse, projection, live_preview, &|_| false)
}

/// Variant that knows which compact mention tokens are alias-registered
/// (`@file:basename`, `@notes:title`, …) so the tail summary can honestly
/// distinguish tokens that will attach from tokens that will not.
pub(crate) fn build_spine_list_sections_full_with_resolved_tokens(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
    _is_resolved_token: &dyn Fn(&str) -> bool,
) -> Vec<SpineListSection> {
    build_spine_list_sections_full_with_resolved_tokens_and_context(
        parse,
        projection,
        live_preview,
        _is_resolved_token,
        SpineListBuildContext::default(),
    )
}

pub(crate) fn build_spine_list_sections_full_with_resolved_tokens_and_context(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
    _is_resolved_token: &dyn Fn(&str) -> bool,
    build_context: SpineListBuildContext<'_>,
) -> Vec<SpineListSection> {
    let segment = active_segment(parse, projection);
    let raw = segment.map(|segment| segment.raw.as_str()).unwrap_or("");

    match &projection.active_segment_kind {
        SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } => {
            if let Some((source, query)) = super::catalog_subsearch::parse_context_subsearch(
                context_type,
                sub_query.as_deref(),
            ) {
                vec![super::catalog_subsearch::build_context_subsearch_section(
                    source, query,
                )]
            } else if sub_query.is_some() || raw.contains(':') {
                vec![build_context_subsearch_placeholder_section(
                    context_type,
                    sub_query.as_deref().unwrap_or(""),
                )]
            } else {
                vec![build_context_root_section(parse, projection, live_preview)]
            }
        }
        SpineSegmentKind::SlashCommand { .. } => {
            vec![build_slash_command_section(parse, projection)]
        }
        SpineSegmentKind::Flow { .. } => {
            let range = active_segment_range(parse, projection);
            vec![super::catalog_flows::build_flow_section(
                &projection.active_query,
                projection.active_segment_index,
                range,
                build_context
                    .current_cwd
                    .and_then(|path| path.to_str())
                    .map(|path| path.to_string()),
            )]
        }
        SpineSegmentKind::Profile { .. } => vec![build_profile_section(parse, projection)],
        SpineSegmentKind::Style { .. } => {
            vec![build_style_section(parse, projection, live_preview)]
        }
        SpineSegmentKind::Capture { args, .. } => {
            let range = active_segment_range(parse, projection);
            let query = projection.active_query.as_str();
            let rows = super::catalog_capture::build_capture_rows(
                query,
                args,
                projection.active_segment_index,
                range,
            );
            vec![section_with_empty(
                "spine-section-capture",
                "Capture",
                Some(ss("Choose a capture target")),
                Some(ss("inbox")),
                rows,
                "No capture target matches",
                "Try ;todo or ;note",
            )]
        }
        SpineSegmentKind::ProjectCwd { .. } => {
            vec![super::catalog_cwd::build_cwd_section(
                parse,
                projection,
                super::catalog_cwd::CwdRootRowsParams {
                    current_cwd: build_context.current_cwd,
                    recents: build_context.cwd_recents,
                },
            )]
        }
        SpineSegmentKind::ModeExit { sigil, rest } => {
            vec![build_mode_exit_section(parse, projection, *sigil, rest)]
        }
        SpineSegmentKind::ListFilter { .. } => {
            let range = active_segment_range(parse, projection);
            vec![super::catalog_filter::build_filter_qualifier_section(
                &projection.active_query,
                projection.active_segment_index,
                range,
            )]
        }
        SpineSegmentKind::FreeText => Vec::new(),
    }
}

fn build_context_root_section(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
) -> SpineListSection {
    let range = active_segment_range(parse, projection);
    let query = projection.active_query.as_str();

    let rows = super::catalog_context::build_context_root_rows_with_preview(
        query,
        projection.active_segment_index,
        range,
        live_preview,
    );

    section_with_empty(
        "spine-section-context",
        "Context",
        Some(ss("Attach context to the prompt")),
        Some(ss("at-sign")),
        rows,
        "No context matches",
        "Try @selection, @clipboard, or @file:",
    )
}

fn build_context_subsearch_placeholder_section(
    context_type: &str,
    sub_query: &str,
) -> SpineListSection {
    let context_label = if context_type.trim().is_empty() {
        "context"
    } else {
        context_type
    };
    SpineListSection {
        id: ss(format!("spine-section-context-subsearch:{context_label}")),
        title: ss(format!("@{context_label}:")),
        subtitle: Some(ss("Unknown context search source")),
        icon: Some(ss("search")),
        rows: vec![SpineListRow {
            id: ss(format!("spine:@:subsearch-placeholder:{context_label}")),
            kind: SpineListRowKind::Hint,
            title: ss(format!(
                "Search {context_label} for \u{201c}{sub_query}\u{201d}"
            )),
            subtitle: Some(ss(
                "Try @file:, @clipboard:, @browser-history:, @notes:, or @history:",
            )),
            meta: Some(ss("Spine")),
            icon: Some(ss("info")),
            badges: vec![ss("@")],
            score: 0,
            is_selectable: false,
            action_label: None,
            action: SpineListAction::Noop,
        }],
    }
}

fn build_slash_command_section(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
) -> SpineListSection {
    let range = active_segment_range(parse, projection);
    let query = projection.active_query.as_str();

    let rows = super::catalog_slash::build_slash_command_rows(
        query,
        projection.active_segment_index,
        range,
    );

    section_with_empty(
        "spine-section-slash",
        "Commands",
        Some(ss("Choose an AI command")),
        Some(ss("slash")),
        rows,
        "No command matches",
        "Try /rewrite or /summarize",
    )
}

fn build_profile_section(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
) -> SpineListSection {
    let range = active_segment_range(parse, projection);
    let query = projection.active_query.as_str();

    let rows =
        super::catalog_profile::build_profile_rows(query, projection.active_segment_index, range);

    section_with_empty(
        "spine-section-profile",
        "Profiles",
        Some(ss("Choose a response profile")),
        Some(ss("user-round")),
        rows,
        "No profile matches",
        "Try |creative or |concise",
    )
}

fn build_style_section(
    parse: &SpineParse,
    projection: &SpineCursorProjection,
    live_preview: Option<&super::live_preview::SpineLivePreview>,
) -> SpineListSection {
    let range = active_segment_range(parse, projection);
    let query = projection.active_query.as_str();
    let rows = super::catalog_style::build_style_rows(
        query,
        projection.active_segment_index,
        range,
        super::prompt_plan::spine_parse_is_style_only(parse),
    );
    let subtitle = live_preview
        .map(|preview| preview.style_target_subtitle())
        .unwrap_or_else(|| "Rewrites the selected or focused text".to_string());
    let title = format!("Styles · {subtitle}");

    section_with_empty(
        "spine-section-style",
        title,
        Some(ss(subtitle)),
        Some(ss("sparkles")),
        rows,
        "No style matches",
        "Try .professional or .concise",
    )
}

fn build_mode_exit_section(
    _parse: &SpineParse,
    _projection: &SpineCursorProjection,
    sigil: char,
    rest: &str,
) -> SpineListSection {
    let (title, subtitle, icon) = match sigil {
        '~' => ("Open File Search", "Browse files", "folder"),
        '!' => ("Open Quick Terminal", "Run a shell command", "terminal"),
        '?' => ("Open Actions Help", "Show available actions", "circle-help"),
        _ => (
            "Open Mode",
            "Leave the prompt-builder flow",
            "external-link",
        ),
    };

    SpineListSection {
        id: ss(format!("spine-section-mode-exit:{sigil}")),
        title: ss("Mode"),
        subtitle: Some(ss("Mode-exit sigils leave the Spine projection")),
        icon: Some(ss(icon)),
        rows: vec![SpineListRow {
            id: ss(format!("spine:{sigil}:mode-exit")),
            kind: SpineListRowKind::Hint,
            title: ss(title),
            subtitle: Some(ss(subtitle)),
            meta: Some(ss("Mode")),
            icon: Some(ss(icon)),
            badges: vec![ss(sigil.to_string())],
            score: i32::MAX,
            is_selectable: true,
            action_label: Some(ss("Open")),
            action: SpineListAction::OpenModeExit {
                sigil,
                rest: ss(rest.to_string()),
            },
        }],
    }
}

pub fn spine_projection_cache_key(
    live_filter_text: &str,
    computed_filter_text: &str,
    parse: &SpineParse,
    projection: &SpineCursorProjection,
) -> String {
    let active = parse.segments.get(projection.active_segment_index);
    let active_range = active
        .map(|segment| segment.byte_range.clone())
        .unwrap_or(0..0);
    let active_resolution = active
        .map(|segment| spine_resolution_cache_key(&segment.resolution))
        .unwrap_or_else(|| "missing".to_string());

    let mut segment_signature = String::new();
    for (index, segment) in parse.segments.iter().enumerate() {
        let _ = write!(
            segment_signature,
            "\x1E{index}:{}..{}:raw-len={}:kind={}:resolution={}",
            segment.byte_range.start,
            segment.byte_range.end,
            segment.raw.len(),
            spine_segment_kind_cache_key(&segment.kind),
            spine_resolution_cache_key(&segment.resolution),
        );
    }

    format!(
        "main-list-spine-projection\
        \x1Fmodel-v={}\
        \x1Fresolution-gen={}\
        \x1Flive-len={}\
        \x1Flive={}\
        \x1Fcomputed-len={}\
        \x1Fcomputed={}\
        \x1Fparse-input-len={}\
        \x1Fparse-input={}\
        \x1Factive-index={}\
        \x1Factive-kind={}\
        \x1Factive-query-len={}\
        \x1Factive-query={}\
        \x1Factive-range={}..{}\
        \x1Factive-resolution={}\
        \x1Fis-tail={}\
        \x1Fhas-prompt-segments={}\
        \x1Fsegments={}",
        SPINE_LIST_MODEL_VERSION,
        SPINE_LIST_RESOLUTION_GENERATION,
        live_filter_text.len(),
        live_filter_text,
        computed_filter_text.len(),
        computed_filter_text,
        parse.input.len(),
        parse.input,
        projection.active_segment_index,
        spine_segment_kind_cache_key(&projection.active_segment_kind),
        projection.active_query.len(),
        projection.active_query,
        active_range.start,
        active_range.end,
        active_resolution,
        projection.is_tail,
        projection.has_prompt_segments,
        segment_signature,
    )
}

fn spine_segment_kind_cache_key(kind: &SpineSegmentKind) -> String {
    match kind {
        SpineSegmentKind::FreeText => "free-text".to_string(),
        SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } => format!("context:{context_type}:sub={sub_query:?}"),
        SpineSegmentKind::SlashCommand { command } => format!("slash:{command}"),
        SpineSegmentKind::Flow { query } => format!("flow:{query}"),
        SpineSegmentKind::Profile { profile_id } => format!("profile:{profile_id}"),
        SpineSegmentKind::Style { style_id } => format!("style:{style_id}"),
        SpineSegmentKind::Capture { target, args } => {
            format!("capture:{target}:args-len={}", args.len())
        }
        SpineSegmentKind::ListFilter { query } => format!("filter:{query}"),
        SpineSegmentKind::ProjectCwd { sub_query } => {
            format!("cwd:sub={sub_query:?}")
        }
        SpineSegmentKind::ModeExit { sigil, rest } => {
            format!("mode-exit:{sigil}:rest-len={}", rest.len())
        }
    }
}

fn spine_resolution_cache_key(resolution: &SpineSegmentResolution) -> String {
    match resolution {
        SpineSegmentResolution::Unresolved => "unresolved".to_string(),
        SpineSegmentResolution::Resolved { id, label, source } => {
            format!("resolved:id={id}:label={label}:source={source}")
        }
        SpineSegmentResolution::Unknown {
            raw,
            preflight_instruction,
        } => format!(
            "unknown:raw-len={}:preflight-len={}",
            raw.len(),
            preflight_instruction.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spine::{parse_spine, project_cursor};

    #[test]
    fn context_mention_produces_builtin_and_subsearch_rows() {
        let parse = parse_spine("@");
        let proj = project_cursor(&parse, 1);
        let sections = build_spine_list_sections(&parse, &proj);
        assert!(!sections.is_empty());
        let rows: Vec<_> = sections.iter().flat_map(|s| &s.rows).collect();
        assert!(rows
            .iter()
            .any(|r| matches!(r.kind, SpineListRowKind::ContextBuiltin { .. })));
        assert!(rows
            .iter()
            .any(|r| matches!(r.kind, SpineListRowKind::ContextSubSearch { .. })));
    }

    #[test]
    fn slash_command_produces_rows() {
        let parse = parse_spine("/");
        let proj = project_cursor(&parse, 1);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|s| &s.rows).collect();
        assert!(rows
            .iter()
            .any(|r| matches!(r.kind, SpineListRowKind::SlashCommand { .. })));
    }

    #[test]
    fn profile_produces_rows() {
        let parse = parse_spine("|");
        let proj = project_cursor(&parse, 1);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|s| &s.rows).collect();
        assert!(rows
            .iter()
            .any(|r| matches!(r.kind, SpineListRowKind::Profile { .. })));
    }

    #[test]
    fn style_section_keeps_catalog_descriptions_and_never_contains_captured_text() {
        const SELECTION_SENTINEL: &str = "P01_SELECTION_BYTES_MUST_NOT_LEAK";
        let parse = parse_spine(".");
        let proj = project_cursor(&parse, 1);
        let preview = crate::spine::live_preview::SpineLivePreview {
            selection_text: Some(SELECTION_SENTINEL.to_string()),
            selection_source_app: Some("Mail".to_string()),
            ..Default::default()
        };
        let sections = build_spine_list_sections_full(&parse, &proj, Some(&preview));

        assert_eq!(sections.len(), 1);
        let section = &sections[0];
        assert_eq!(section.id.as_ref(), "spine-section-style");
        assert_eq!(
            section.title.as_ref(),
            "Styles · Rewrites your selection in Mail"
        );
        assert_eq!(
            section.subtitle.as_ref().map(|subtitle| subtitle.as_ref()),
            Some("Rewrites your selection in Mail")
        );
        assert_eq!(
            section.icon.as_ref().map(|icon| icon.as_ref()),
            Some("sparkles")
        );

        let catalog = crate::spine::catalog_style::resolved_styles();
        let style_rows = section
            .rows
            .iter()
            .filter(|row| matches!(&row.kind, SpineListRowKind::Style { .. }))
            .collect::<Vec<_>>();
        assert_eq!(style_rows.len(), catalog.len());

        for row in style_rows {
            let SpineListRowKind::Style { style_id } = &row.kind else {
                unreachable!("filtered to style rows")
            };
            let style = catalog
                .iter()
                .find(|style| style.id == style_id.as_ref())
                .unwrap_or_else(|| panic!("missing catalog style {}", style_id.as_ref()));
            let expected_id = format!("spine:.:{}", style.id);
            assert_eq!(row.id.as_ref(), expected_id.as_str());
            assert_eq!(
                row.subtitle.as_ref().map(|subtitle| subtitle.as_ref()),
                Some(style.description.as_str()),
                "style row {} must retain its catalog description",
                style.id
            );
            assert_eq!(
                row.icon.as_ref().map(|icon| icon.as_ref()),
                Some(style.icon.as_str())
            );
            assert_eq!(
                row.action_label.as_ref().map(|label| label.as_ref()),
                Some("Rewrite")
            );
        }

        assert!(
            !format!("{section:?}").contains(SELECTION_SENTINEL),
            "captured selection bytes must not enter any style section or row field"
        );
    }

    #[test]
    fn free_text_without_prompt_segments_returns_empty() {
        let parse = parse_spine("hello");
        let proj = project_cursor(&parse, 5);
        let sections = build_spine_list_sections(&parse, &proj);
        assert!(sections.is_empty());
    }

    #[test]
    fn cache_key_differs_for_different_inputs() {
        let parse_a = parse_spine("@sel");
        let proj_a = project_cursor(&parse_a, 4);
        let parse_b = parse_spine("@clip");
        let proj_b = project_cursor(&parse_b, 5);

        let key_a = spine_projection_cache_key("@sel", "@sel", &parse_a, &proj_a);
        let key_b = spine_projection_cache_key("@clip", "@clip", &parse_b, &proj_b);
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn slash_root_contains_rewrite_row_from_catalog() {
        let parse = parse_spine("/");
        let proj = project_cursor(&parse, 1);
        let sections = build_spine_list_sections(&parse, &proj);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].id.as_ref(), "spine-section-slash");
        let rows: Vec<_> = sections.iter().flat_map(|section| &section.rows).collect();
        let row = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:/:rewrite")
            .expect("expected /rewrite row");
        assert_eq!(row.title.as_ref(), "Rewrite");
        assert_eq!(
            row.subtitle.as_ref().map(|s| s.as_ref()),
            Some("Rewrite the prompt or selected context")
        );
        assert!(row.is_selectable);
        match &row.kind {
            SpineListRowKind::SlashCommand { command } => {
                assert_eq!(command.as_ref(), "rewrite");
            }
            other => panic!("expected SlashCommand row, got {other:?}"),
        }
        match &row.action {
            SpineListAction::ResolveSegment {
                replacement,
                resolution_id,
                resolution_source,
                trailing_space,
                ..
            } => {
                assert_eq!(replacement.as_ref(), "/rewrite");
                assert_eq!(resolution_id.as_ref(), "default:rewrite");
                assert_eq!(resolution_source.as_ref(), "slash-command-default");
                assert!(*trailing_space);
            }
            other => panic!("expected ResolveSegment action, got {other:?}"),
        }
    }

    #[test]
    fn slash_rew_filters_to_rewrite() {
        let parse = parse_spine("/rew");
        let proj = project_cursor(&parse, 4);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|section| &section.rows).collect();
        let titles: Vec<_> = rows.iter().map(|row| row.title.as_ref()).collect();
        assert!(
            titles.contains(&"Rewrite"),
            "expected /rewrite in filtered slash rows: {titles:?}"
        );
        assert!(
            !titles.contains(&"Summarize"),
            "did not expect /summarize to match /rew: {titles:?}"
        );
    }

    #[test]
    fn slash_unknown_query_produces_empty_row() {
        let input = "/definitely-no-such-spine-command-zzzz";
        let parse = parse_spine(input);
        let proj = project_cursor(&parse, input.len());
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|section| &section.rows).collect();
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0].kind, SpineListRowKind::Empty));
        assert!(!rows[0].is_selectable);
    }

    #[test]
    fn context_root_contains_all_context_attachment_specs() {
        let parse = parse_spine("@");
        let proj = project_cursor(&parse, 1);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|s| &s.rows).collect();

        for spec in crate::ai::context_contract::context_attachment_specs() {
            let mention = spec
                .mention
                .expect("Step 5 expects every context attachment spec to have a mention");
            let expected_id = format!("spine:@:builtin:{}", mention.trim_start_matches('@'));
            assert!(
                rows.iter().any(|row| row.id.as_ref() == expected_id),
                "missing context row for {mention}"
            );
        }
    }

    #[test]
    fn context_selection_row_uses_context_attachment_contract() {
        let parse = parse_spine("@sel");
        let proj = project_cursor(&parse, 4);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|s| &s.rows).collect();

        let row = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:builtin:selection")
            .expect("expected @selection row");

        assert_eq!(row.title.as_ref(), "Selection");
        assert!(row.meta.is_none());

        match &row.action {
            SpineListAction::ResolveSegment {
                replacement,
                resolution_id,
                resolution_label,
                resolution_source,
                trailing_space,
                ..
            } => {
                assert_eq!(replacement.as_ref(), "@selection");
                assert_eq!(resolution_id.as_ref(), "chat:add_selection_context");
                assert_eq!(resolution_label.as_ref(), "Selection");
                assert_eq!(resolution_source.as_ref(), "context-builtin");
                assert!(*trailing_space);
            }
            other => panic!("expected ResolveSegment action, got {other:?}"),
        }
    }

    #[test]
    fn free_text_tail_after_prompt_builder_segments_returns_empty() {
        let input = "@selection /rewrite make it punchier";
        let parse = parse_spine(input);
        let proj = project_cursor(&parse, parse.input.len());
        let sections = build_spine_list_sections(&parse, &proj);
        assert!(sections.is_empty());
    }

    #[test]
    fn synthetic_tail_projection_returns_empty() {
        let parse = parse_spine("@selection /rewrite ");
        let synthetic_proj = SpineCursorProjection {
            active_segment_index: parse.segments.len(),
            active_segment_kind: SpineSegmentKind::FreeText,
            active_query: String::new(),
            is_tail: true,
            has_prompt_segments: true,
        };
        let sections = build_spine_list_sections(&parse, &synthetic_proj);
        assert!(sections.is_empty());
    }

    #[test]
    fn slash_catalog_matches_fuzzy_subsequence() {
        // Catalog filtering should behave like the main list: a fuzzy
        // subsequence ("rwt" → rewrite) still finds the command.
        let parse = parse_spine("/rwt");
        let proj = project_cursor(&parse, 4);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|section| &section.rows).collect();
        assert!(
            rows.iter().any(|row| row.id.as_ref() == "spine:/:rewrite"),
            "fuzzy subsequence must match /rewrite"
        );
    }

    /// A partial fragment (`@fi`) still offers the completion row; an exact
    /// trigger (`@file`) routes straight into the file search section — the
    /// list IS the search mode, with no "press Enter to refine" picker step.
    #[test]
    fn partial_context_root_includes_file_subsearch_prefix() {
        let parse = parse_spine("@fi");
        let proj = project_cursor(&parse, 3);
        let sections = build_spine_list_sections(&parse, &proj);
        let rows: Vec<_> = sections.iter().flat_map(|s| &s.rows).collect();

        let row = rows
            .iter()
            .find(|row| row.id.as_ref() == "spine:@:subsearch:file")
            .expect("expected @file: subsearch row");

        assert_eq!(row.title.as_ref(), "Files");

        match &row.action {
            SpineListAction::InsertSegmentText {
                text,
                trailing_space,
                ..
            } => {
                assert_eq!(text.as_ref(), "@file:");
                assert!(!*trailing_space);
            }
            other => panic!("expected InsertSegmentText action, got {other:?}"),
        }
    }

    #[test]
    fn exact_context_root_fragment_routes_to_subsearch_section() {
        for input in ["@file", "@files", "@clipboard", "@history"] {
            let parse = parse_spine(input);
            let proj = project_cursor(&parse, input.len());
            let sections = build_spine_list_sections(&parse, &proj);
            assert!(
                sections
                    .iter()
                    .any(|s| s.id.as_ref().starts_with("spine-section-subsearch:")),
                "{input} must route to the subsearch section, got {:?}",
                sections.iter().map(|s| s.id.as_ref()).collect::<Vec<_>>(),
            );
        }
    }
}

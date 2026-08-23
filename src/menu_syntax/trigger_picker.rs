use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::mode::capture_body_boundary_has_started_with_targets;
use super::parse::{parse, parse_with_capture_targets, MenuSyntaxParse};
use super::payload::{
    picker_visible_capture_targets, resolve_capture_target, ArtifactKind, IncompleteKind,
    KNOWN_CAPTURE_TARGETS,
};
use crate::scripts::{Script, Scriptlet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerPickerMode {
    AdvancedQuery,
    Capture,
    Command,
}

impl TriggerPickerMode {
    /// Leading section header (label, icon token) for this picker mode in the
    /// main list. Labels align with the footer tips' vocabulary (": filters
    /// results" → "Filters") so invoking a sigil confirms what the tip
    /// promised. Shared by the render path and the automation collector so
    /// they can never disagree.
    pub fn main_list_section(self) -> (&'static str, &'static str) {
        match self {
            Self::Capture => ("Capture", "inbox"),
            Self::Command => ("Command", "terminal"),
            Self::AdvancedQuery => ("Filters", "search"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerPickerRowKind {
    Qualifier,
    QualifierValue,
    UnknownQualifierFix,
    RecentQuery,
    CaptureTarget,
    CaptureHandler,
    CaptureArtifact,
    Command,
    Shortcut,
    FooterAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerPickerAction {
    InsertToken {
        token: String,
        keep_open: bool,
    },
    ReplaceInput {
        text: String,
    },
    FixQualifier {
        bad: String,
        good: String,
    },
    #[allow(dead_code)]
    ExecuteCaptureHandler {
        command_id: String,
    },
    #[allow(dead_code)]
    OpenCaptures {
        target: Option<String>,
    },
    CreateHandler {
        target: Option<String>,
    },
    OpenHelp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedCaptureHandler {
    pub path: PathBuf,
    pub filename: String,
}

pub trait CaptureHandlerScaffoldEffects {
    fn path_exists(&self, path: &Path) -> bool;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()>;
    fn open_in_editor(&self, path: &Path) -> io::Result<()>;
}

pub struct OsCaptureHandlerScaffoldEffects;

impl CaptureHandlerScaffoldEffects for OsCaptureHandlerScaffoldEffects {
    fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        std::fs::write(path, contents)
    }

    fn open_in_editor(&self, path: &Path) -> io::Result<()> {
        let _child = std::process::Command::new("open").arg(path).spawn()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerPickerRow {
    pub id: String,
    pub mode: TriggerPickerMode,
    pub kind: TriggerPickerRowKind,
    pub title: String,
    pub token: Option<String>,
    pub subtitle: Option<String>,
    pub detail: Option<String>,
    pub example: Option<String>,
    pub badges: Vec<String>,
    pub action: TriggerPickerAction,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerPickerSnapshot {
    pub mode: TriggerPickerMode,
    pub target: Option<String>,
    pub rows: Vec<TriggerPickerRow>,
}

pub fn create_capture_handler_scaffold(
    effects: &impl CaptureHandlerScaffoldEffects,
    scripts_dir: &Path,
    slug: &str,
    open_editor: bool,
) -> io::Result<CreatedCaptureHandler> {
    let normalized_slug = normalize_capture_handler_slug(slug);
    let path = resolve_capture_handler_destination(effects, scripts_dir, &normalized_slug);
    let contents =
        super::templates::render_capture_handler_template(&normalized_slug, &normalized_slug);
    if let Some(parent) = path.parent() {
        effects.create_dir_all(parent)?;
    }
    effects.write_file(&path, &contents)?;
    if open_editor {
        effects.open_in_editor(&path)?;
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(CreatedCaptureHandler { path, filename })
}

pub fn resolve_capture_handler_destination(
    effects: &impl CaptureHandlerScaffoldEffects,
    scripts_dir: &Path,
    slug: &str,
) -> PathBuf {
    let base = format!("capture-{slug}-handler");
    let mut candidate = scripts_dir.join(format!("{base}.ts"));
    let mut suffix = 2;
    while effects.path_exists(&candidate) {
        candidate = scripts_dir.join(format!("{base}-{suffix}.ts"));
        suffix += 1;
    }
    candidate
}

fn normalize_capture_handler_slug(slug: &str) -> String {
    let lower = slug.trim().to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_dash = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "capture".to_string()
    } else {
        trimmed
    }
}

#[derive(Debug, Clone, Default)]
pub struct TriggerPickerContext {
    pub recent_queries: Vec<String>,
    /// Scripts available to the launcher for this render pass. Command mode
    /// uses them for `!` rows; capture handler execution is intentionally not
    /// rendered in the target picker while users compose text.
    pub scripts: Vec<Arc<Script>>,
    /// Scriptlets available to command mode for `!` rows.
    pub scriptlets: Vec<Arc<Scriptlet>>,
}

pub fn build_trigger_picker_snapshot(
    input: &str,
    ctx: &TriggerPickerContext,
) -> Option<TriggerPickerSnapshot> {
    if let Some(snapshot) = bang_command_snapshot(input, ctx) {
        return snapshot;
    }

    let capture_targets = registered_capture_targets(ctx);
    if let Some(snapshot) = build_capture_field_snapshot(input, &capture_targets) {
        return Some(snapshot);
    }
    if capture_body_boundary_has_started_with_targets(input, &capture_targets) {
        return None;
    }
    if plus_capture_body_boundary_has_started(input, &capture_targets) {
        return None;
    }
    if let Some(filter) = canonical_capture_picker_filter(input) {
        return Some(build_capture_picker_snapshot(filter.as_deref(), ctx));
    }

    if is_committed_complete_type_value_query(input) {
        return None;
    }
    if should_show_has_field_completion(input) {
        return non_empty_snapshot(build_advanced_query_snapshot(input, ctx));
    }
    if should_show_type_value_completion(input) {
        return non_empty_snapshot(build_advanced_query_snapshot(input, ctx));
    }

    let parsed = if capture_targets.is_empty() {
        parse(input)
    } else {
        parse_with_capture_targets(input, &capture_targets)
    };

    match parsed {
        MenuSyntaxParse::AdvancedQuery(_) if is_complete_has_field_query(input) => None,
        MenuSyntaxParse::AdvancedQuery(query) if query.is_source_filter_only() => None,
        MenuSyntaxParse::AdvancedQuery(_) if should_show_advanced_query_completion(input) => {
            non_empty_snapshot(build_advanced_query_snapshot(input, ctx))
        }
        MenuSyntaxParse::AdvancedQuery(_) => None,
        MenuSyntaxParse::Capture(inv) => {
            Some(build_capture_snapshot(Some(inv.target.as_str()), ctx))
        }
        MenuSyntaxParse::Argv(inv) => {
            if command_body_boundary_has_started(input) {
                None
            } else {
                non_empty_snapshot(build_command_snapshot(Some(inv.head.as_str()), ctx))
            }
        }
        MenuSyntaxParse::Incomplete(s) => match s.kind {
            IncompleteKind::BareQueryPrefix => {
                non_empty_snapshot(build_advanced_query_snapshot(input, ctx))
            }
            IncompleteKind::BareCapturePrefix => {
                let filter = capture_picker_filter(input);
                if filter.is_none() && unknown_plus_capture_body(input) {
                    return None;
                }
                Some(build_capture_picker_snapshot(filter.as_deref(), ctx))
            }
            IncompleteKind::MissingCaptureBody(t) => {
                Some(build_capture_snapshot(Some(t.as_str()), ctx))
            }
            IncompleteKind::BareArgvPrefix => non_empty_snapshot(build_command_snapshot(None, ctx)),
            _ => None,
        },
        MenuSyntaxParse::None => None,
    }
}

fn bang_command_snapshot(
    input: &str,
    ctx: &TriggerPickerContext,
) -> Option<Option<TriggerPickerSnapshot>> {
    let rest = input.strip_prefix('!')?;
    if rest.find(char::is_whitespace).is_some() {
        return Some(None);
    }
    let head = (!rest.is_empty()).then_some(rest);
    Some(non_empty_snapshot(build_command_snapshot(head, ctx)))
}

pub fn registered_capture_targets(ctx: &TriggerPickerContext) -> Vec<String> {
    crate::menu_syntax::registered_capture_targets_from_scripts(&ctx.scripts)
}

fn plus_capture_body_boundary_has_started(input: &str, registered_targets: &[String]) -> bool {
    let raw = input.trim_start();
    let Some(rest) = raw.strip_prefix('+') else {
        return false;
    };
    let target_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    if target_end == 0 || target_end == rest.len() {
        return false;
    }
    let target = &rest[..target_end];
    crate::menu_syntax::capture::is_capture_target_registered(target, registered_targets)
}

fn non_empty_snapshot(snapshot: TriggerPickerSnapshot) -> Option<TriggerPickerSnapshot> {
    (!snapshot.rows.is_empty()).then_some(snapshot)
}

fn build_advanced_query_snapshot(input: &str, ctx: &TriggerPickerContext) -> TriggerPickerSnapshot {
    let mut rows: Vec<TriggerPickerRow> = Vec::new();

    rows.extend(typo_fix_rows(input));
    rows.extend(filtered_static_qualifier_rows(input));
    if advanced_query_active_token(input).is_empty() && !is_exact_bare_colon(input) {
        rows.extend(recent_query_rows(&ctx.recent_queries));
    }
    // Run 12 — the advanced-query popup no longer pushes a generic
    // help-launcher footer row. The main-hint surface now
    // owns context-aware copy for `has:`, source heads, and `:type:`
    // / `:tag:` / `:shortcut:` so a footer help row is redundant.
    // Capture create-handler footers remain via
    // [[footer_create_handler_row]].

    TriggerPickerSnapshot {
        mode: TriggerPickerMode::AdvancedQuery,
        target: None,
        rows,
    }
}

fn build_capture_field_snapshot(
    input: &str,
    registered_targets: &[String],
) -> Option<TriggerPickerSnapshot> {
    let selector =
        crate::menu_syntax::active_capture_field_selector_for_input(input, registered_targets)
            .or_else(|| crate::menu_syntax::active_link_capture_field_selector_for_input(input))?;
    let rows = selector
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let replacement = format!("{}:", field.key);
            let text = replace_range(input, selector.range, &replacement);
            TriggerPickerRow {
                id: format!("capture-field:{}:{}", selector.target, field.key),
                mode: TriggerPickerMode::Capture,
                kind: TriggerPickerRowKind::Qualifier,
                title: field.label.to_string(),
                token: Some(replacement),
                subtitle: Some(format!(";{} field", selector.target)),
                detail: None,
                example: None,
                badges: if field.required_on_create {
                    vec!["required".to_string()]
                } else {
                    Vec::new()
                },
                action: TriggerPickerAction::ReplaceInput { text },
                enabled: index < 12,
            }
        })
        .take(12)
        .collect::<Vec<_>>();
    (!rows.is_empty()).then_some(TriggerPickerSnapshot {
        mode: TriggerPickerMode::Capture,
        target: Some(selector.target),
        rows,
    })
}

fn replace_range(input: &str, range: (usize, usize), replacement: &str) -> String {
    let mut text = String::new();
    text.push_str(&input[..range.0]);
    text.push_str(replacement);
    if !replacement.ends_with(' ') {
        text.push(' ');
    }
    text.push_str(&input[range.1..]);
    text
}

fn filtered_static_qualifier_rows(input: &str) -> Vec<TriggerPickerRow> {
    if is_exact_bare_colon(input) {
        return bare_colon_filter_head_rows();
    }

    let active = advanced_query_active_token(input);
    if let Some(rows) = has_field_value_rows_for_active_token(&active) {
        return rows;
    }
    if input.trim_start().starts_with(':') && !active.contains(':') && !active.starts_with('#') {
        return filtered_bare_colon_filter_head_rows(&active);
    }
    let rows = static_qualifier_rows();
    if active.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| qualifier_row_matches_active_token(row, &active))
        .collect()
}

fn is_exact_bare_colon(input: &str) -> bool {
    input.trim() == ":"
}

fn advanced_query_active_token(input: &str) -> String {
    let stripped = input.strip_prefix(':').unwrap_or(input);
    let active = stripped.split_whitespace().last().unwrap_or_default();
    let active_lc = active.to_ascii_lowercase();
    if active_lc.starts_with("has:")
        || active_lc.starts_with("type:")
        || active_lc.starts_with("kind:")
    {
        return active_lc;
    }
    if active_lc.contains(':') && !active_lc.ends_with(':') {
        return String::new();
    }
    active_lc
}

fn should_show_has_field_completion(input: &str) -> bool {
    let active = advanced_query_active_token(input);
    if active == "has" {
        return true;
    }
    let Some(value) = active.strip_prefix("has:") else {
        return false;
    };
    if value.is_empty() {
        return true;
    }
    crate::menu_syntax::has_fields::lookup_has_field(value).is_none()
        && crate::menu_syntax::has_fields::HAS_FIELD_SPECS
            .iter()
            .any(|spec| {
                spec.canonical.to_ascii_lowercase().starts_with(value)
                    || spec
                        .aliases
                        .iter()
                        .any(|alias| alias.to_ascii_lowercase().starts_with(value))
            })
}

fn should_show_type_value_completion(input: &str) -> bool {
    let active = advanced_query_active_token(input);
    active.starts_with("type:") || active.starts_with("kind:")
}

fn should_show_advanced_query_completion(input: &str) -> bool {
    if is_exact_bare_colon(input) {
        return true;
    }

    let active = advanced_query_active_token(input);
    if active.is_empty() {
        return false;
    }
    if active.contains(':') {
        return true;
    }

    [
        "type",
        "kind",
        "tag",
        "shortcut",
        "has",
        "source",
        "plugin",
        "name",
        "desc",
        "description",
        "alias",
    ]
    .iter()
    .any(|head| head.starts_with(active.as_str()))
}

fn is_complete_has_field_query(input: &str) -> bool {
    let active = advanced_query_active_token(input);
    let Some(value) = active.strip_prefix("has:") else {
        return false;
    };
    crate::menu_syntax::has_fields::lookup_has_field(value).is_some()
}

fn is_committed_complete_type_value_query(input: &str) -> bool {
    input.chars().last().is_some_and(char::is_whitespace) && active_type_value_is_complete(input)
}

fn active_type_value_is_complete(input: &str) -> bool {
    let active = advanced_query_active_token(input);
    let value = active
        .strip_prefix("type:")
        .or_else(|| active.strip_prefix("kind:"));
    value.is_some_and(|value| !value.is_empty() && ArtifactKind::parse(value).is_some())
}

fn has_field_value_rows_for_active_token(active: &str) -> Option<Vec<TriggerPickerRow>> {
    let partial = if active == "has" {
        ""
    } else {
        active.strip_prefix("has:")?
    };
    Some(
        crate::menu_syntax::has_fields::HAS_FIELD_SPECS
            .iter()
            .filter(|spec| {
                partial.is_empty()
                    || spec.canonical.to_ascii_lowercase().starts_with(partial)
                    || spec
                        .aliases
                        .iter()
                        .any(|alias| alias.to_ascii_lowercase().starts_with(partial))
            })
            .map(|spec| TriggerPickerRow {
                id: format!("qualifier:{}", spec.token),
                mode: TriggerPickerMode::AdvancedQuery,
                kind: TriggerPickerRowKind::QualifierValue,
                title: spec.token.to_string(),
                token: Some(spec.token.to_string()),
                subtitle: spec.subtitle.map(str::to_string),
                detail: spec.detail.map(str::to_string),
                example: Some(spec.token.to_string()),
                badges: Vec::new(),
                action: TriggerPickerAction::InsertToken {
                    token: spec.token.to_string(),
                    keep_open: false,
                },
                enabled: true,
            })
            .collect(),
    )
}

fn qualifier_row_matches_active_token(row: &TriggerPickerRow, active: &str) -> bool {
    let token = row
        .token
        .as_deref()
        .unwrap_or_default()
        .trim_start_matches(':')
        .to_ascii_lowercase();
    let normalized_active = active
        .strip_prefix("kind:")
        .map(|value| format!("type:{value}"));
    let active = normalized_active.as_deref().unwrap_or(active);
    let title = row.title.to_ascii_lowercase();
    token.starts_with(active)
        || title
            .split_whitespace()
            .any(|word| word.starts_with(active))
}

fn build_capture_snapshot(
    target: Option<&str>,
    ctx: &TriggerPickerContext,
) -> TriggerPickerSnapshot {
    if target.is_none() {
        return build_capture_picker_snapshot(None, ctx);
    }

    let mut rows: Vec<TriggerPickerRow> = Vec::new();

    match target {
        None => {}
        Some(t) => {
            let public_target = public_capture_target_slug(t);
            let entry = capture_target_catalog(ctx, true)
                .into_iter()
                .find(|entry| entry.slug.eq_ignore_ascii_case(public_target))
                .unwrap_or_else(|| builtin_capture_target_entry(public_target));
            rows.push(capture_target_row_from_entry(&entry));
        }
    }

    rows.push(footer_create_handler_row(
        target.map(public_capture_target_slug).map(str::to_string),
    ));

    TriggerPickerSnapshot {
        mode: TriggerPickerMode::Capture,
        target: target.map(str::to_string),
        rows,
    }
}

fn capture_picker_filter(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    if let Some(rest) = trimmed.strip_prefix(';') {
        // Canonical capture sigil: use the first whitespace-delimited token as
        // the filter slug so trailing text (`;gcal `, `;gcal Lunch w/ Mindy`)
        // does not collapse back into the bare-`;` "all targets" picker. A
        // registered target would have already short-circuited via
        // capture_body_boundary_has_started_with_targets before this point, so
        // any whitespace here means the typed slug is unknown.
        let head = rest.split_whitespace().next()?;
        if head.is_empty() {
            return None;
        }
        return Some(head.to_ascii_lowercase());
    }
    // Legacy `+` sigil (being phased out): only treat purely-prefix input as
    // capture filter — `+react component` falls back to fuzzy search via the
    // unknown_plus_capture_body branch above.
    let rest = trimmed.strip_prefix('+')?;
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return None;
    }
    Some(rest.to_ascii_lowercase())
}

fn canonical_capture_picker_filter(input: &str) -> Option<Option<String>> {
    let trimmed = input.trim_start();
    let rest = trimmed.strip_prefix(';')?;
    if rest.trim().is_empty() {
        return Some(None);
    }
    let head = rest.split_whitespace().next()?;
    Some(Some(head.to_ascii_lowercase()))
}

fn unknown_plus_capture_body(input: &str) -> bool {
    let trimmed = input.trim_start();
    trimmed.starts_with('+') && trimmed.trim() != "+"
}

fn build_capture_picker_snapshot(
    filter: Option<&str>,
    ctx: &TriggerPickerContext,
) -> TriggerPickerSnapshot {
    let catalog = capture_target_catalog(ctx, filter.is_some());
    if let Some(needle) = filter {
        if let Some(entry) = catalog
            .iter()
            .find(|entry| entry.slug.eq_ignore_ascii_case(needle))
        {
            let rows = vec![capture_target_row_from_entry(entry)];
            return TriggerPickerSnapshot {
                mode: TriggerPickerMode::Capture,
                target: Some(entry.slug.clone()),
                rows,
            };
        }
    }

    let (mut rows, footer_target): (Vec<TriggerPickerRow>, Option<String>) = match filter {
        None => (
            catalog.iter().map(capture_target_row_from_entry).collect(),
            None,
        ),
        Some(needle) => {
            let mut scored: Vec<(i32, usize, &CaptureTargetEntry)> = catalog
                .iter()
                .enumerate()
                .filter_map(|(idx, entry)| {
                    score_capture_target(needle, entry).map(|score| (score, idx, entry))
                })
                .collect();

            if scored.is_empty() {
                // No fuzzy match: leave the list empty so the user isn't shown
                // unrelated targets with one auto-focused. The create-handler
                // footer is the only useful action for an unknown slug.
                (Vec::new(), Some(needle.to_string()))
            } else {
                scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
                (
                    scored
                        .into_iter()
                        .map(|(_, _, entry)| capture_target_row_from_entry(entry))
                        .collect(),
                    None,
                )
            }
        }
    };

    if filter.is_some() {
        rows.push(footer_create_handler_row(footer_target));
    }

    TriggerPickerSnapshot {
        mode: TriggerPickerMode::Capture,
        target: None,
        rows,
    }
}

fn build_command_snapshot(head: Option<&str>, ctx: &TriggerPickerContext) -> TriggerPickerSnapshot {
    let rows = command_rows(head, ctx);

    TriggerPickerSnapshot {
        mode: TriggerPickerMode::Command,
        target: head.map(str::to_string),
        rows,
    }
}

struct StaticQualifierRow {
    token: &'static str,
    title: &'static str,
    subtitle: Option<&'static str>,
    detail: Option<&'static str>,
    display_token: &'static str,
    example: &'static str,
    insert: &'static str,
    keep_open: bool,
}

struct AdvancedQueryHeadRowSpec {
    id: &'static str,
    display_token: &'static str,
    insert_token: &'static str,
    title: &'static str,
    subtitle: &'static str,
}

const ADVANCED_QUERY_HEAD_ROW_SPECS: &[AdvancedQueryHeadRowSpec] = &[
    AdvancedQueryHeadRowSpec {
        id: "has",
        display_token: "has:",
        insert_token: "has:",
        title: "has:",
        subtitle: "Filter by available metadata fields.",
    },
    AdvancedQueryHeadRowSpec {
        id: "type",
        display_token: "type:",
        insert_token: "type:",
        title: "type:",
        subtitle: "Filter by result type.",
    },
    AdvancedQueryHeadRowSpec {
        id: "shortcut",
        display_token: "shortcut:",
        insert_token: "shortcut:",
        title: "shortcut:",
        subtitle: "Filter by keyboard shortcut.",
    },
    AdvancedQueryHeadRowSpec {
        id: "source",
        display_token: "source:",
        insert_token: "source:",
        title: "source:",
        subtitle: "Filter by plugin, kit, or source name.",
    },
    AdvancedQueryHeadRowSpec {
        id: "plugin",
        display_token: "plugin:",
        insert_token: "plugin:",
        title: "plugin:",
        subtitle: "Filter by plugin id.",
    },
    AdvancedQueryHeadRowSpec {
        id: "name",
        display_token: "name:",
        insert_token: "name:",
        title: "name:",
        subtitle: "Filter by result name.",
    },
    AdvancedQueryHeadRowSpec {
        id: "desc",
        display_token: "desc:",
        insert_token: "desc:",
        title: "desc:",
        subtitle: "Filter by description text.",
    },
    AdvancedQueryHeadRowSpec {
        id: "alias",
        display_token: "alias:",
        insert_token: "alias:",
        title: "alias:",
        subtitle: "Filter by alias text.",
    },
    AdvancedQueryHeadRowSpec {
        id: "tag",
        display_token: "tag:",
        insert_token: "tag:",
        title: "tag:",
        subtitle: "Filter by tag name.",
    },
    AdvancedQueryHeadRowSpec {
        id: "meta",
        display_token: "meta.<path>:",
        insert_token: "meta.",
        title: "meta.<path>:",
        subtitle: "Filter by a nested metadata path.",
    },
];

fn bare_colon_filter_head_rows() -> Vec<TriggerPickerRow> {
    let mut rows = source_filter_head_rows(false);
    rows.extend(
        ADVANCED_QUERY_HEAD_ROW_SPECS
            .iter()
            .map(|spec| TriggerPickerRow {
                id: format!("qualifier-head:{}", spec.id),
                mode: TriggerPickerMode::AdvancedQuery,
                kind: TriggerPickerRowKind::Qualifier,
                title: spec.title.to_string(),
                token: Some(spec.display_token.to_string()),
                subtitle: Some(spec.subtitle.to_string()),
                detail: None,
                example: None,
                badges: Vec::new(),
                action: TriggerPickerAction::InsertToken {
                    token: spec.insert_token.to_string(),
                    keep_open: true,
                },
                enabled: true,
            }),
    );
    rows
}

fn filtered_bare_colon_filter_head_rows(active: &str) -> Vec<TriggerPickerRow> {
    bare_colon_filter_head_rows()
        .into_iter()
        .filter(|row| {
            let token = row
                .token
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            token.starts_with(active)
        })
        .collect()
}

fn static_qualifier_rows() -> Vec<TriggerPickerRow> {
    let qualifiers: &[StaticQualifierRow] = &[
        StaticQualifierRow {
            token: "type:script",
            title: "Scripts only",
            subtitle: Some("Limit results to runnable scripts."),
            detail: None,
            display_token: "type:script",
            example: "type:script git",
            insert: "type:script",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:scriptlet",
            title: "Scriptlets only",
            subtitle: Some("Limit results to scriptlets."),
            detail: None,
            display_token: "type:scriptlet",
            example: "type:scriptlet shell",
            insert: "type:scriptlet",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:skill",
            title: "Skills only",
            subtitle: Some("Find agent skills without using / chat syntax."),
            detail: None,
            display_token: "type:skill",
            example: "type:skill review",
            insert: "type:skill",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:builtin",
            title: "Built-ins only",
            subtitle: None,
            detail: None,
            display_token: "type:builtin",
            example: "type:builtin clipboard",
            insert: "type:builtin",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:app",
            title: "Apps only",
            subtitle: None,
            detail: None,
            display_token: "type:app",
            example: "type:app safari",
            insert: "type:app",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:window",
            title: "Windows only",
            subtitle: None,
            detail: None,
            display_token: "type:window",
            example: "type:window chrome",
            insert: "type:window",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:agent",
            title: "Agents only",
            subtitle: None,
            detail: None,
            display_token: "type:agent",
            example: "type:agent",
            insert: "type:agent",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "type:issue",
            title: "Script issues only",
            subtitle: None,
            detail: None,
            display_token: "type:issue",
            example: "type:issue",
            insert: "type:issue",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "shortcut:any",
            title: "Has any keyboard shortcut",
            subtitle: None,
            detail: None,
            display_token: "shortcut:any",
            example: "shortcut:any",
            insert: "shortcut:any",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "shortcut:none",
            title: "Has no keyboard shortcut",
            subtitle: None,
            detail: None,
            display_token: "shortcut:none",
            example: "shortcut:none",
            insert: "shortcut:none",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "shortcut:cmd+k",
            title: "Exact shortcut match",
            subtitle: None,
            detail: None,
            display_token: "shortcut:cmd+k",
            example: "shortcut:cmd+k",
            insert: "shortcut:cmd+k",
            keep_open: false,
        },
        StaticQualifierRow {
            token: "source:",
            title: "Source",
            subtitle: Some("Broad match against plugin or kit name."),
            detail: None,
            display_token: "source:",
            example: "source:main inbox",
            insert: "source:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "plugin:",
            title: "Plugin",
            subtitle: Some("Exact plugin pair match."),
            detail: None,
            display_token: "plugin:",
            example: "plugin:main.todo",
            insert: "plugin:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "name:",
            title: "Name contains",
            subtitle: None,
            detail: None,
            display_token: "name:",
            example: "name:deploy",
            insert: "name:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "desc:",
            title: "Description contains",
            subtitle: None,
            detail: None,
            display_token: "desc:",
            example: "desc:database",
            insert: "desc:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "alias:",
            title: "Alias contains",
            subtitle: None,
            detail: None,
            display_token: "alias:",
            example: "alias:db",
            insert: "alias:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "#",
            title: "Filter by tag",
            subtitle: Some("Use #work or tag:work"),
            detail: Some("Tags label captures and can narrow launcher rows."),
            display_token: "#",
            example: "#work type:script",
            insert: "#",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "tag:",
            title: "Tag name",
            subtitle: Some("Same filter as #tag"),
            detail: Some("Useful for long or namespaced tags."),
            display_token: "tag:",
            example: "tag:client/acme type:issue",
            insert: "tag:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "meta.category:",
            title: "Metadata category",
            subtitle: Some("Read a nested metadata path."),
            detail: None,
            display_token: "meta.category:",
            example: "meta.category:inbox",
            insert: "meta.category:",
            keep_open: true,
        },
        StaticQualifierRow {
            token: "-type:app",
            title: "Exclude apps",
            subtitle: Some("A leading - negates a filter."),
            detail: None,
            display_token: "-type:app",
            example: "-type:app triage",
            insert: "-type:app",
            keep_open: false,
        },
    ];

    let mut rows = source_filter_head_rows(true);

    rows.extend(qualifiers.iter().map(|row| TriggerPickerRow {
        id: format!("qualifier:{}", row.token),
        mode: TriggerPickerMode::AdvancedQuery,
        kind: TriggerPickerRowKind::Qualifier,
        title: row.title.to_string(),
        token: Some(row.display_token.to_string()),
        subtitle: row.subtitle.map(str::to_string),
        detail: row.detail.map(str::to_string),
        example: Some(row.example.to_string()),
        badges: Vec::new(),
        action: TriggerPickerAction::InsertToken {
            token: row.insert.to_string(),
            keep_open: row.keep_open,
        },
        enabled: true,
    }));
    rows
}

fn source_filter_head_rows(include_examples: bool) -> Vec<TriggerPickerRow> {
    crate::menu_syntax::SOURCE_HEAD_SPECS
        .iter()
        .map(|spec| TriggerPickerRow {
            id: format!("source:{}", spec.canonical),
            mode: TriggerPickerMode::AdvancedQuery,
            kind: TriggerPickerRowKind::Qualifier,
            title: spec.label.to_string(),
            token: Some(spec.canonical.to_string()),
            subtitle: Some(spec.description.to_string()),
            detail: spec.short.map(|short| format!("Shortcut: {short}")),
            example: include_examples.then(|| format!("{} project", spec.canonical)),
            badges: Vec::new(),
            action: TriggerPickerAction::InsertToken {
                token: spec.canonical.to_string(),
                keep_open: false,
            },
            enabled: true,
        })
        .collect()
}

fn typo_fix_rows(input: &str) -> Vec<TriggerPickerRow> {
    const KNOWN_HEADS: &[&str] = &[
        "type",
        "kind",
        "shortcut",
        "source",
        "plugin",
        "name",
        "desc",
        "description",
        "alias",
        "tag",
        "has",
        "meta",
    ];

    let stripped = input.strip_prefix(':').unwrap_or(input);
    let mut rows: Vec<TriggerPickerRow> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for token in stripped.split_whitespace() {
        let body = token.strip_prefix('-').unwrap_or(token);
        let Some((head, value)) = body.split_once(':') else {
            continue;
        };
        if head.is_empty() {
            continue;
        }
        let head_lc = head.to_ascii_lowercase();
        if KNOWN_HEADS.iter().any(|k| *k == head_lc) {
            continue;
        }
        if head_lc.starts_with("meta.") {
            continue;
        }

        let Some(correct) = KNOWN_HEADS
            .iter()
            .find(|k| within_one_edit(&head_lc, k))
            .copied()
        else {
            continue;
        };

        let canonical = format!("{correct}:{value}");
        if seen.iter().any(|s| s == &canonical) {
            continue;
        }
        seen.push(canonical.clone());

        rows.push(TriggerPickerRow {
            id: format!("fix:{head_lc}:{correct}"),
            mode: TriggerPickerMode::AdvancedQuery,
            kind: TriggerPickerRowKind::UnknownQualifierFix,
            title: format!("Did you mean {correct}:{value}?"),
            token: Some(format!(":{canonical}")),
            subtitle: Some(format!("'{head}' is not a known qualifier")),
            detail: None,
            example: None,
            badges: vec!["typo".to_string()],
            action: TriggerPickerAction::FixQualifier {
                bad: format!("{head}:{value}"),
                good: canonical,
            },
            enabled: true,
        });
    }

    rows
}

fn command_body_boundary_has_started(input: &str) -> bool {
    let Some(rest) = input.strip_prefix('>') else {
        return false;
    };
    let rest = rest.trim_start();
    if rest.is_empty() {
        return false;
    }
    rest.find(char::is_whitespace)
        .map(|idx| idx > 0)
        .unwrap_or(false)
}

fn command_rows(head: Option<&str>, ctx: &TriggerPickerContext) -> Vec<TriggerPickerRow> {
    let needle = head
        .map(crate::menu_syntax::command_slug)
        .unwrap_or_default();
    let mut slug_counts: HashMap<String, usize> = HashMap::new();
    for script in &ctx.scripts {
        let slug = crate::menu_syntax::script_command_head(script);
        if command_row_matches(&slug, &needle) {
            *slug_counts.entry(slug).or_insert(0) += 1;
        }
    }
    for scriptlet in &ctx.scriptlets {
        let slug = crate::menu_syntax::scriptlet_command_head(scriptlet);
        if command_row_matches(&slug, &needle) {
            *slug_counts.entry(slug).or_insert(0) += 1;
        }
    }

    let mut rows: Vec<TriggerPickerRow> = Vec::new();

    for script in &ctx.scripts {
        let slug = crate::menu_syntax::script_command_head(script);
        if command_row_matches(&slug, &needle) {
            let duplicate = slug_counts.get(&slug).copied().unwrap_or(0) > 1;
            rows.push(mark_command_duplicate(
                command_script_row(script, &slug),
                duplicate,
            ));
        }
    }
    for scriptlet in &ctx.scriptlets {
        let slug = crate::menu_syntax::scriptlet_command_head(scriptlet);
        if command_row_matches(&slug, &needle) {
            let duplicate = slug_counts.get(&slug).copied().unwrap_or(0) > 1;
            rows.push(mark_command_duplicate(
                command_scriptlet_row(scriptlet, &slug),
                duplicate,
            ));
        }
    }

    rows.sort_by(|a, b| a.token.cmp(&b.token).then_with(|| a.title.cmp(&b.title)));
    rows.truncate(24);
    rows
}

fn mark_command_duplicate(mut row: TriggerPickerRow, duplicate: bool) -> TriggerPickerRow {
    if duplicate {
        if let Some(slug) = row
            .token
            .as_deref()
            .and_then(|token| token.strip_prefix('>'))
        {
            let bang_token = format!("!{slug}");
            row.token = Some(bang_token.clone());
            row.example = Some(format!("{bang_token} -- "));
            row.action = TriggerPickerAction::InsertToken {
                token: format!("{bang_token} "),
                keep_open: false,
            };
        }
        row.enabled = false;
        row.badges.push("duplicate".to_string());
        row.detail = Some("Ambiguous command head; give one command a unique alias".to_string());
    }
    row
}

fn command_row_matches(slug: &str, needle: &str) -> bool {
    needle.is_empty() || slug.starts_with(needle)
}

fn command_script_row(script: &Script, slug: &str) -> TriggerPickerRow {
    TriggerPickerRow {
        id: format!("command:script:{slug}:{}", script.path.display()),
        mode: TriggerPickerMode::Command,
        kind: TriggerPickerRowKind::Command,
        title: script.name.clone(),
        token: Some(format!(">{slug}")),
        subtitle: script.description.clone(),
        detail: script.source_detail_for_picker(),
        example: Some(format!(">{slug} -- ")),
        badges: vec!["script".to_string()],
        action: TriggerPickerAction::InsertToken {
            token: format!(">{slug} "),
            keep_open: false,
        },
        enabled: true,
    }
}

fn command_scriptlet_row(scriptlet: &Scriptlet, slug: &str) -> TriggerPickerRow {
    TriggerPickerRow {
        id: format!(
            "command:scriptlet:{slug}:{}",
            scriptlet.file_path.as_deref().unwrap_or(&scriptlet.name)
        ),
        mode: TriggerPickerMode::Command,
        kind: TriggerPickerRowKind::Command,
        title: scriptlet.name.clone(),
        token: Some(format!("!{slug}")),
        subtitle: scriptlet.description.clone(),
        detail: scriptlet.group.clone(),
        example: Some(format!("!{slug} -- ")),
        badges: vec!["scriptlet".to_string()],
        action: TriggerPickerAction::InsertToken {
            token: format!("!{slug} "),
            keep_open: false,
        },
        enabled: true,
    }
}

trait ScriptPickerDetail {
    fn source_detail_for_picker(&self) -> Option<String>;
}

impl ScriptPickerDetail for Script {
    fn source_detail_for_picker(&self) -> Option<String> {
        self.plugin_title
            .clone()
            .or_else(|| {
                if self.plugin_id.is_empty() {
                    self.kit_name.clone()
                } else {
                    Some(self.plugin_id.clone())
                }
            })
            .filter(|s| !s.is_empty())
    }
}

fn recent_query_rows(recent: &[String]) -> Vec<TriggerPickerRow> {
    recent
        .iter()
        .enumerate()
        .filter_map(|(idx, text)| {
            match parse(text) {
                MenuSyntaxParse::AdvancedQuery(_) | MenuSyntaxParse::Incomplete(_) => {}
                _ => return None,
            }
            Some(TriggerPickerRow {
                id: format!("recent:{idx}"),
                mode: TriggerPickerMode::AdvancedQuery,
                kind: TriggerPickerRowKind::RecentQuery,
                title: text.clone(),
                token: None,
                subtitle: Some("Recent query".to_string()),
                detail: None,
                example: None,
                badges: Vec::new(),
                action: TriggerPickerAction::ReplaceInput { text: text.clone() },
                enabled: true,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct CaptureTargetEntry {
    slug: String,
    title: String,
    detail: String,
    example: Option<String>,
}

fn capture_target_catalog(
    ctx: &TriggerPickerContext,
    include_hidden_aliases: bool,
) -> Vec<CaptureTargetEntry> {
    let label_overrides = registered_capture_target_label_overrides(ctx);
    let mut entries: Vec<CaptureTargetEntry> = KNOWN_CAPTURE_TARGETS
        .iter()
        .filter(|target| !suppress_public_capture_alias(target))
        .filter(|target| {
            include_hidden_aliases
                || picker_visible_capture_targets()
                    .iter()
                    .any(|visible| visible.eq_ignore_ascii_case(target))
        })
        .map(|target| builtin_capture_target_entry(target))
        .collect();
    for target in registered_capture_targets(ctx) {
        if !include_hidden_aliases {
            if let Some(resolution) = resolve_capture_target(&target) {
                if !resolution.picker_visible {
                    continue;
                }
            }
        }
        if !entries
            .iter()
            .any(|existing| existing.slug.eq_ignore_ascii_case(&target))
        {
            let title = label_overrides
                .get(&target.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| humanize_target_slug(&target));
            entries.push(CaptureTargetEntry {
                slug: target,
                title,
                detail: "Registered capture target".to_string(),
                example: None,
            });
        }
    }
    entries
}

fn registered_capture_target_label_overrides(
    ctx: &TriggerPickerContext,
) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    for script in &ctx.scripts {
        for spec in crate::menu_syntax::script_menu_syntax_specs(script) {
            if spec.family != "capture.v1" {
                continue;
            }
            let Some(label) = spec
                .label
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            for target in spec.targets {
                if target == "*" || target.trim().is_empty() {
                    continue;
                }
                labels
                    .entry(target.to_ascii_lowercase())
                    .or_insert_with(|| label.to_string());
            }
        }
    }
    labels
}

fn suppress_public_capture_alias(target: &str) -> bool {
    matches!(
        target.to_ascii_lowercase().as_str(),
        "notes" | "reminder" | "snooze" | "defer"
    )
}

fn public_capture_target_slug(target: &str) -> &str {
    match target.to_ascii_lowercase().as_str() {
        "notes" => "note",
        "reminder" | "snooze" | "defer" => "todo",
        _ => target,
    }
}

fn builtin_capture_target_entry(target: &str) -> CaptureTargetEntry {
    let (title, detail, example) = match target {
        "todo" => (
            "Todo inbox",
            "Create or update a Todo task",
            Some(";todo buy milk #errand p1"),
        ),
        "cal" => (
            "Calendar event",
            "Write .ics file under menu-syntax/calendar",
            Some(";cal standup tomorrow 3pm"),
        ),
        "note" => (
            "Note",
            "Create or update a built-in Note or daily note",
            Some(";note decision to ship parser first"),
        ),
        "social" => (
            "Social draft",
            "Write markdown draft (clipboard on macOS)",
            Some(";social shipping menu syntax today"),
        ),
        "link" => (
            "Saved link",
            "Save or update a tagged link",
            Some(";link https://zed.dev #rust"),
        ),
        "snippet" => (
            "Snippet",
            "Create, update, or remove a snippet",
            Some(";snippet add fetch-json trigger:fj lang:ts -- const res = await fetch(url)"),
        ),
        _ => resolve_capture_target(target)
            .map(|resolution| (resolution.title, resolution.detail, None))
            .unwrap_or(("Capture target", "Unknown target", None)),
    };

    CaptureTargetEntry {
        slug: target.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
        example: example.map(str::to_string),
    }
}

fn humanize_target_slug(target: &str) -> String {
    let words: Vec<String> = target
        .split(|c: char| c == '_' || c == '-' || c.is_ascii_punctuation())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect();
    if words.is_empty() {
        return "Capture target".to_string();
    }
    let mut title = words.join(" ");
    if let Some(first) = title.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    title
}

fn capture_target_row_from_entry(entry: &CaptureTargetEntry) -> TriggerPickerRow {
    // A4 decision (2026-06-09): accepting a capture target converts the input
    // to the canonical postfix spelling (`todo; `), which commits the target
    // and hands the input to the capture form.
    TriggerPickerRow {
        id: format!("target:{}", entry.slug),
        mode: TriggerPickerMode::Capture,
        kind: TriggerPickerRowKind::CaptureTarget,
        title: entry.title.clone(),
        token: Some(format!("{};", entry.slug)),
        subtitle: None,
        detail: Some(entry.detail.clone()),
        example: entry.example.clone(),
        badges: Vec::new(),
        action: TriggerPickerAction::InsertToken {
            token: format!("{}; ", entry.slug),
            keep_open: false,
        },
        enabled: true,
    }
}

fn score_capture_target(needle: &str, entry: &CaptureTargetEntry) -> Option<i32> {
    let needle = normalize_match_text(needle);
    if needle.is_empty() {
        return None;
    }
    let slug = normalize_match_text(&entry.slug);
    let title = normalize_match_text(&entry.title);
    [
        score_text_match(&needle, &slug, true),
        score_text_match(&needle, &title, false),
    ]
    .into_iter()
    .flatten()
    .chain(
        capture_target_search_aliases(&entry.slug)
            .into_iter()
            .filter_map(|alias| score_text_match(&needle, alias, false)),
    )
    .max()
}

fn capture_target_search_aliases(slug: &str) -> Vec<&'static str> {
    match slug {
        "note" => vec!["daily", "daily note", "notes"],
        "todo" => vec!["reminder", "snooze", "defer"],
        _ => Vec::new(),
    }
}

fn normalize_match_text(text: &str) -> String {
    let mut out = String::new();
    let mut last_was_space = true;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

fn score_text_match(needle: &str, haystack: &str, is_slug: bool) -> Option<i32> {
    if haystack.is_empty() {
        return None;
    }
    let length_delta = haystack.len().saturating_sub(needle.len()) as i32;
    if needle == haystack {
        return Some(if is_slug { 10000 } else { 9500 });
    }
    if haystack.starts_with(needle) {
        return Some(if is_slug {
            9000 - length_delta
        } else {
            8500 - length_delta
        });
    }
    if !is_slug {
        if let Some(word) = haystack
            .split_whitespace()
            .find(|word| word.starts_with(needle))
        {
            return Some(8000 - word.len().saturating_sub(needle.len()) as i32);
        }
    }
    if let Some(index) = haystack.find(needle) {
        return Some(if is_slug {
            7000 - index as i32
        } else {
            6500 - index as i32
        });
    }
    if needle.len() >= 2 {
        if let Some(gap_penalty) = fuzzy_subsequence_gap_penalty(needle, haystack) {
            return Some(if is_slug {
                5000 - gap_penalty
            } else {
                4500 - gap_penalty
            });
        }
    }
    None
}

fn fuzzy_subsequence_gap_penalty(needle: &str, haystack: &str) -> Option<i32> {
    let mut needle_chars = needle.chars();
    let mut current = needle_chars.next()?;
    let mut first_match: Option<usize> = None;

    for (idx, ch) in haystack.chars().enumerate() {
        if ch == current {
            first_match.get_or_insert(idx);
            match needle_chars.next() {
                Some(next) => current = next,
                None => {
                    let span = idx - first_match.unwrap_or(idx) + 1;
                    return Some(span.saturating_sub(needle.chars().count()) as i32);
                }
            }
        }
    }
    None
}

fn footer_create_handler_row(target: Option<String>) -> TriggerPickerRow {
    let (title, detail) = match target.as_deref() {
        Some(t) => (
            format!("Create capture handler for ;{t}…"),
            format!("Scaffold ~/.scriptkit/plugins/main/scripts/capture-{t}-<slug>.ts"),
        ),
        None => (
            "Create capture handler…".to_string(),
            "Pick a target first, then scaffold a .ts handler".to_string(),
        ),
    };
    TriggerPickerRow {
        id: "footer:create-handler".to_string(),
        mode: TriggerPickerMode::Capture,
        kind: TriggerPickerRowKind::FooterAction,
        title,
        token: None,
        subtitle: None,
        detail: Some(detail),
        example: None,
        badges: Vec::new(),
        action: TriggerPickerAction::CreateHandler { target },
        enabled: true,
    }
}

/// Returns the slug + human title of the closest near-miss capture target for
/// a typed slug, or `None` if no entry is within one edit. Used by the main
/// hint surface to show a muted "Similar → ;cal" recovery line in the
/// no-match/create-handler state without re-introducing rejected fuzzy rows
/// into the popup itself.
pub fn nearest_capture_target_for_slug(
    slug: &str,
    scripts: &[Arc<Script>],
) -> Option<(String, String)> {
    let needle = normalize_match_text(slug);
    if needle.is_empty() {
        return None;
    }
    let ctx = TriggerPickerContext {
        scripts: scripts.to_vec(),
        ..Default::default()
    };
    capture_target_catalog(&ctx, true)
        .into_iter()
        .find(|entry| within_one_edit(&needle, &normalize_match_text(&entry.slug)))
        .map(|entry| (entry.slug, entry.title))
}

fn within_one_edit(a: &str, b: &str) -> bool {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();

    if la == 0 || lb == 0 {
        return false;
    }
    if la.abs_diff(lb) > 1 {
        return false;
    }

    if la == lb {
        let mut diffs: Vec<usize> = Vec::new();
        for i in 0..la {
            if a_chars[i] != b_chars[i] {
                diffs.push(i);
                if diffs.len() > 2 {
                    return false;
                }
            }
        }
        if diffs.len() <= 1 {
            return true;
        }
        if diffs.len() == 2 {
            let (i, j) = (diffs[0], diffs[1]);
            return j == i + 1 && a_chars[i] == b_chars[j] && a_chars[j] == b_chars[i];
        }
        return false;
    }

    let (shorter, longer) = if la < lb {
        (&a_chars, &b_chars)
    } else {
        (&b_chars, &a_chars)
    };
    let mut i = 0usize;
    let mut j = 0usize;
    let mut skipped = false;
    while i < shorter.len() && j < longer.len() {
        if shorter[i] == longer[j] {
            i += 1;
            j += 1;
        } else if !skipped {
            skipped = true;
            j += 1;
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
include!("trigger_picker_tests.rs");

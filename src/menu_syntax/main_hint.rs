use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::capture_schema::{builtin_schema, validate, FieldRequirement, ValidationResult};
use super::filter::script_command_schema_for;
use super::fragments::{MenuSyntaxFragmentRole, MenuSyntaxFragmentStatus};
use super::mode::MenuSyntaxMode;
use super::payload::{
    object_ref_deeplink, resolve_capture_target, AdvancedQuery, ArgvInvocation, ArtifactKind,
    CanonicalCaptureTarget, CaptureAlias, CaptureInvocation, CaptureObjectRef,
    CaptureTargetResolution, CommandArgSpec, CommandFlagSpec, DatePhrase, DateRole, IncompleteKind,
    MenuSyntaxHandlerSpec, Predicate, ShortcutPredicate,
};
use super::trigger_picker::{
    nearest_capture_target_for_slug, TriggerPickerAction, TriggerPickerMode, TriggerPickerRow,
    TriggerPickerRowKind, TriggerPickerSnapshot,
};
use crate::scripts::{Script, Scriptlet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuSyntaxMainHintKind {
    AdvancedQueryGuide,
    CapturePickerCompanion,
    CaptureComposer,
    CommandPickerCompanion,
    CommandComposer,
    AdvancedQueryEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuSyntaxMainHintTone {
    Neutral,
    Accent,
    Info,
    Warning,
    Success,
    Muted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSyntaxMainHintChip {
    pub label: String,
    pub tone: MenuSyntaxMainHintTone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSyntaxMainHintRow {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chips: Vec<MenuSyntaxMainHintChip>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSyntaxFragmentPreviewSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<MenuSyntaxFragmentPreviewRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSyntaxFragmentPreviewRow {
    pub role: crate::menu_syntax::fragments::MenuSyntaxFragmentRole,
    pub label: String,
    pub value: String,
    pub source: String,
    pub source_span: (usize, usize),
    pub status: crate::menu_syntax::fragments::MenuSyntaxFragmentStatus,
    pub tone: MenuSyntaxMainHintTone,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chips: Vec<MenuSyntaxMainHintChip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MenuSyntaxCaptureValidationStatus {
    Ready,
    Incomplete,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSyntaxCaptureValidationSnapshot {
    pub target: String,
    pub status: MenuSyntaxCaptureValidationStatus,
    pub can_submit: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_field_labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub malformed_field_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub malformed_reason: Option<String>,
    /// Run 12 Pass 14 — the same HUD nudge string the gate would show on
    /// Enter, surfaced in the snapshot so automation can verify it via
    /// `getState.menuSyntaxMainHint.captureValidation.hudMessage` without
    /// scraping the transient HUD overlay. Mirrors
    /// `CaptureGateDecision::hud_message()`. None on `Ready`; populated on
    /// `Incomplete` and `Malformed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hud_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSyntaxMainHintSnapshot {
    pub kind: MenuSyntaxMainHintKind,
    pub raw_filter_text: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_chip: Option<MenuSyntaxMainHintChip>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_chip: Option<MenuSyntaxMainHintChip>,
    /// Multi-chip alternative to `status_chip` for capture validation
    /// surfaces (mode chip + per-missing-field chips + ready chip). The
    /// existing `status_chip` is preserved for backward compatibility with
    /// non-capture surfaces. (Run 11 Pass 22.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_chips: Vec<MenuSyntaxMainHintChip>,
    /// Structured capture-validation receipt (Pass 22). Present only for
    /// `CaptureComposer` snapshots where the target has a registered
    /// schema (built-in or dynamic).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_validation: Option<MenuSyntaxCaptureValidationSnapshot>,
    /// Structured form projection for capture composer mode. This is the
    /// clean field surface derived from the same grammar/schema as the power
    /// syntax, so users can tab through fields while the main input remains
    /// the synchronized power-user representation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<crate::menu_syntax::form::MenuSyntaxFormSnapshot>,
    /// Date phrases the parser recognized as date-slot keys but could not
    /// interpret (e.g. `due:asdf`). Run 12 Pass 10 — wires the data layer
    /// shipped in Run 11 Pass 34's [[src/menu_syntax/date.rs#resolve_capture_dates]]
    /// into the snapshot so the renderer + state-receipt can warn the user.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_dates: Vec<crate::menu_syntax::date::UnresolvedDate>,
    /// Inline AI proposal (Run 12 Pass 11). Set when the user pressed
    /// Cmd+Enter while composing power syntax; cleared on filter change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_syntax_ai_proposal: Option<crate::menu_syntax_ai::MenuSyntaxAiProposal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<MenuSyntaxMainHintRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment_preview: Option<MenuSyntaxFragmentPreviewSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// Run 12 — the active grammar head the user is typing, with the
    /// trailing punctuation included (`has:`, `c:`, `:type:`, `:tag:`,
    /// `:shortcut:`, `;`, `!`). Display-only receipt: never mutates
    /// parser predicates; never reclassifies input. Populated by
    /// [[active_head_context_for_filter]] so automation can verify
    /// head-aware copy without screenshots or popup scraping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_head: Option<String>,
    /// The partial value the user has typed after [[Self::active_head]],
    /// e.g. `"sh"` for `has:sh` or `"zzz"` for `c:zzz`. Empty when the
    /// cursor sits immediately after the head (e.g. bare `has:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_head_value_partial: Option<String>,
    pub accessibility_label: String,
}

pub struct MenuSyntaxMainHintContext<'a> {
    pub raw_filter_text: &'a str,
    pub mode: &'a MenuSyntaxMode,
    pub picker_snapshot: Option<&'a TriggerPickerSnapshot>,
    pub picker_selected_row_id: Option<&'a str>,
    pub scripts: &'a [Arc<Script>],
    pub scriptlets: &'a [Arc<Scriptlet>],
    pub advanced_query_results_empty: bool,
    /// Run 12 Pass 11 — pending Cmd+Enter inline AI proposal threaded into
    /// the capture composer snapshot.
    pub menu_syntax_ai_proposal: Option<&'a crate::menu_syntax_ai::MenuSyntaxAiProposal>,
}

pub fn build_menu_syntax_main_hint(
    ctx: MenuSyntaxMainHintContext<'_>,
) -> Option<MenuSyntaxMainHintSnapshot> {
    if ctx.raw_filter_text.is_empty() {
        return None;
    }

    if ctx
        .mode
        .capture_composer_owns_input_for(ctx.raw_filter_text)
    {
        return capture_composer_hint(&ctx);
    }

    if let Some(snapshot) = ctx.picker_snapshot {
        if matches!(snapshot.mode, TriggerPickerMode::AdvancedQuery)
            && !should_show_advanced_query_guide(ctx.raw_filter_text)
        {
            let active = active_head_context_for_filter(ctx.raw_filter_text);
            if let Some(query) = ctx.mode.advanced_query_for(ctx.raw_filter_text) {
                return Some(advanced_query_applied_hint(
                    ctx.raw_filter_text,
                    query,
                    active.as_ref(),
                ));
            }
            if snapshot.rows.iter().any(|row| row.enabled) {
                return None;
            }
            if let Some(active) = active.as_ref() {
                if let Some(hint) = synthetic_active_head_empty_hint(ctx.raw_filter_text, active) {
                    return Some(hint);
                }
            }
        }
    }

    if ctx.advanced_query_results_empty {
        if let Some(query) = ctx.mode.advanced_query_for(ctx.raw_filter_text) {
            // Source-only browse (bare `c:` / `c: ` / `v: `) keeps its
            // existing no-hint behavior. Source-attached queries with
            // free text (`c:zzz`) fall through to the head-aware empty
            // branch, which renders compact source-specific copy.
            if query.is_source_filter_only() && query.free_text.is_empty() {
                return None;
            }
            let active = active_head_context_for_filter(ctx.raw_filter_text);
            return advanced_query_empty_hint(ctx.raw_filter_text, query, active.as_ref());
        }
        // No parsed AdvancedQuery (e.g. bare `has:` is Incomplete; bare
        // `c:` is a source filter), but the user IS typing a known head.
        // Synthesize a head-aware empty hint from the active-head detector
        // alone so completions and zero-result copy work for in-flight input.
        if let Some(active) = active_head_context_for_filter(ctx.raw_filter_text) {
            if let Some(hint) = synthetic_active_head_empty_hint(ctx.raw_filter_text, &active) {
                return Some(hint);
            }
        }
    }

    if let Some(snapshot) = ctx.picker_snapshot {
        match snapshot.mode {
            TriggerPickerMode::AdvancedQuery => {
                if should_show_advanced_query_guide(ctx.raw_filter_text) {
                    return advanced_query_guide_hint(ctx.raw_filter_text, snapshot);
                }
            }
            TriggerPickerMode::Capture => return capture_picker_companion_hint(&ctx, snapshot),
            TriggerPickerMode::Command
                if !command_body_boundary_has_started(ctx.raw_filter_text) =>
            {
                return command_picker_companion_hint(&ctx, snapshot);
            }
            _ => {}
        }
    }

    if ctx.mode.command_owns_input_for(ctx.raw_filter_text) {
        return command_composer_hint(&ctx);
    }

    None
}

fn should_show_advanced_query_guide(raw_filter_text: &str) -> bool {
    let Some(body) = raw_filter_text.strip_prefix(':') else {
        return false;
    };
    if body.is_empty() {
        return true;
    }
    if body.chars().any(char::is_whitespace) {
        return false;
    }

    let lower = body.to_ascii_lowercase();
    if lower == "#" || lower == "tag:" {
        return true;
    }
    if lower.starts_with('#') {
        return false;
    }
    if lower.contains(':') {
        return lower.ends_with(':');
    }
    true
}

fn advanced_query_guide_hint(
    raw_filter_text: &str,
    snapshot: &TriggerPickerSnapshot,
) -> Option<MenuSyntaxMainHintSnapshot> {
    let body = raw_filter_text.strip_prefix(':').unwrap_or(raw_filter_text);
    let active = body.to_ascii_lowercase();

    if active == "#" {
        return Some(finalize_hint(MenuSyntaxMainHintSnapshot {
            kind: MenuSyntaxMainHintKind::AdvancedQueryGuide,
            status_chips: Vec::new(),
            capture_validation: None,
            form: None,
            unresolved_dates: Vec::new(),
            menu_syntax_ai_proposal: None,
            raw_filter_text: raw_filter_text.to_string(),
            title: "Filter by tag".to_string(),
            subtitle: Some(
                "After `:`, `#tag` narrows the launcher catalog to tagged items.".to_string(),
            ),
            mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
            status_chip: Some(chip("tag filter", MenuSyntaxMainHintTone::Neutral)),
            rows: tag_boundary_rows(),
            fragment_preview: None,
            primary_hint: Some("Type the tag name after `:#`.".to_string()),
            secondary_hint: Some("Use `;... #tag` to label saved data.".to_string()),
            example: Some(":#work type:script".to_string()),
            examples: vec![
                ":#work type:script".to_string(),
                ":#client/acme type:issue".to_string(),
                ";todo Send proposal #client/acme".to_string(),
            ],
            warning: None,
            active_head: None,
            active_head_value_partial: None,
            accessibility_label: String::new(),
        }));
    }

    let (title, subtitle, status_label, rows) = if body.is_empty() {
        (
            "Refine launcher search".to_string(),
            "Use `:` to add filters, then type the words you want to match.".to_string(),
            "guide".to_string(),
            vec![
                hint_row(
                    "Filters",
                    "type, shortcut, source, plugin, name, description, alias, tag",
                ),
                hint_row(
                    "Tags",
                    "`:#work` filters tagged items; `#work` alone is normal search",
                ),
                hint_row(
                    "Search words",
                    "Anything after filters still uses launcher search",
                ),
            ],
        )
    } else {
        let matches = snapshot
            .rows
            .iter()
            .filter(|row| row.enabled)
            .filter_map(|row| row.token.as_deref())
            .take(4)
            .collect::<Vec<_>>()
            .join(", ");
        (
            "Choose a filter".to_string(),
            "Filters narrow launcher results before your search words run.".to_string(),
            "filtering".to_string(),
            vec![
                hint_row("Typed", body),
                hint_row(
                    "Matches",
                    if matches.is_empty() {
                        "No filter matches yet"
                    } else {
                        &matches
                    },
                ),
                hint_row("Example", ":type:script deploy"),
            ],
        )
    };

    Some(finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::AdvancedQueryGuide,
        status_chips: Vec::new(),
        capture_validation: None,
        form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: raw_filter_text.to_string(),
        title,
        subtitle: Some(subtitle),
        mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
        status_chip: Some(chip(&status_label, MenuSyntaxMainHintTone::Neutral)),
        rows,
        fragment_preview: None,
        primary_hint: Some("Pick a filter in the popup, or keep typing.".to_string()),
        secondary_hint: Some(
            "Refine is search only. It does not save or capture anything.".to_string(),
        ),
        example: Some(":type:script deploy".to_string()),
        examples: vec![
            ":type:script deploy".to_string(),
            ":#work type:script".to_string(),
            ":-type:app triage".to_string(),
            ":shortcut:any".to_string(),
        ],
        warning: None,
        active_head: None,
        active_head_value_partial: None,
        accessibility_label: String::new(),
    }))
}

fn advanced_query_applied_hint(
    raw_filter_text: &str,
    query: &AdvancedQuery,
    active: Option<&ActiveHeadContext>,
) -> MenuSyntaxMainHintSnapshot {
    let mut rows = Vec::new();
    if !query.predicates.is_empty() {
        rows.push(MenuSyntaxMainHintRow {
            label: "Filters".to_string(),
            value: query
                .predicates
                .iter()
                .map(predicate_user_label)
                .collect::<Vec<_>>()
                .join(" · "),
            chips: Vec::new(),
        });
    }
    if !query.free_text.trim().is_empty() {
        rows.push(hint_row("Search words", query.free_text.trim()));
    }
    rows.push(hint_row(
        "Recovery",
        "Remove a filter to widen the launcher results.",
    ));

    let (title, example) = match active {
        Some(active) if matches!(active.kind, ActiveHeadKind::TypeQualifier) => {
            let value = active.value_partial.trim();
            let kind_label = type_value_label(value);
            (
                format!("Filtering to {kind_label}"),
                Some(if value.is_empty() {
                    ":type:script review".to_string()
                } else if active.head.eq_ignore_ascii_case(":kind:") {
                    format!(":kind:{value} review")
                } else {
                    format!(":type:{value} review")
                }),
            )
        }
        Some(active) if matches!(active.kind, ActiveHeadKind::ShortcutQualifier) => (
            "Filtering by shortcut".to_string(),
            Some(":shortcut:any review".to_string()),
        ),
        Some(active) if matches!(active.kind, ActiveHeadKind::TagQualifier) => (
            "Filtering by tag".to_string(),
            Some(format!(":{} review", active.value_partial.trim())),
        ),
        _ => (
            "Refine launcher search".to_string(),
            Some(":type:script review".to_string()),
        ),
    };

    finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::AdvancedQueryGuide,
        status_chips: Vec::new(),
        capture_validation: None,
        form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: raw_filter_text.to_string(),
        title,
        subtitle: Some("Filters narrow launcher results before search words run.".to_string()),
        mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
        status_chip: Some(chip("filtering", MenuSyntaxMainHintTone::Neutral)),
        rows,
        fragment_preview: None,
        primary_hint: Some(
            "Keep typing to narrow results, or remove a filter to widen.".to_string(),
        ),
        secondary_hint: None,
        example: example.clone(),
        examples: example.into_iter().collect(),
        warning: None,
        active_head: active.map(|a| a.head.clone()),
        active_head_value_partial: active.and_then(|a| a.value_partial_opt()),
        accessibility_label: String::new(),
    })
}

fn capture_picker_companion_hint(
    ctx: &MenuSyntaxMainHintContext<'_>,
    snapshot: &TriggerPickerSnapshot,
) -> Option<MenuSyntaxMainHintSnapshot> {
    let selected = selected_picker_row(snapshot, ctx.picker_selected_row_id);

    // No-match / create-handler-focused branch: when the typed slug has no
    // fuzzy matches, the picker renders only the "Create capture handler for
    // ;<slug>…" footer. The generic capture companion copy ("Choose a capture
    // target in the popup", body-composition hint, cross-target examples)
    // describes a different state, so emit a setup-focused hint instead.
    if let Some(row) = selected {
        if let TriggerPickerAction::CreateHandler { target: Some(slug) } = &row.action {
            let has_target_rows = snapshot
                .rows
                .iter()
                .any(|r| r.kind == TriggerPickerRowKind::CaptureTarget && r.enabled);
            if !has_target_rows {
                return Some(capture_create_handler_hint(ctx, slug));
            }
        }
    }

    let target = selected
        .and_then(|row| row.token.as_deref())
        .and_then(|token| {
            // A4 capture rows spell the token postfix ("todo;"); accept the
            // older prefix form (";todo") too so the companion hint can
            // always name the selected target.
            token.strip_suffix(';').or_else(|| token.strip_prefix(';'))
        })
        .or(snapshot.target.as_deref());

    let mut rows = Vec::new();
    if let Some(row) = selected {
        if let Some(token) = row.token.as_deref() {
            rows.push(hint_row("Selected", token));
        }
        if let Some(detail) = row.detail.as_deref() {
            rows.push(hint_row("Target", detail));
        }
    }

    let title = target
        .map(|target| capture_title(target, selected.map(|row| row.title.as_str())))
        .unwrap_or_else(|| "Start a capture".to_string());

    let primary_hint = if let Some(target) = target {
        Some(format!(
            "Press Enter or Tab to accept ;{target}, then type the body."
        ))
    } else {
        Some("Choose a capture target in the popup.".to_string())
    };

    Some(finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::CapturePickerCompanion,
        status_chips: Vec::new(),
        capture_validation: None,
            form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: ctx.raw_filter_text.to_string(),
        title,
        subtitle: Some("Create local structured data without searching scripts.".to_string()),
        mode_chip: Some(chip("; capture", MenuSyntaxMainHintTone::Accent)),
        status_chip: None,
        rows,
        fragment_preview: None,
        primary_hint,
        secondary_hint: Some(
            "After choosing: type body text, #tags, p1-p4 priority, dates, URLs, or key=value fields."
                .to_string(),
        ),
        example: selected
            .and_then(|row| row.example.clone())
            .or_else(|| {
                target.and_then(|t| target_examples(public_examples_target(t)).into_iter().next())
            })
            .or_else(|| Some(";todo Buy milk #errands p2 due:tomorrow".to_string())),
        examples: target
            .map(public_examples_target)
            .map(target_examples)
            .unwrap_or_else(|| vec![
                ";todo Buy milk #errands p2 due:tomorrow".to_string(),
                ";note Decision to ship parser first #product".to_string(),
                ";link https://zed.dev #rust title:\"GPUI notes\"".to_string(),
                ";snippet add fetch-json trigger:fj lang:ts -- const res = await fetch(url)"
                    .to_string(),
            ]),
        warning: None,
        active_head: None,
        active_head_value_partial: None,
        accessibility_label: String::new(),
    }))
}

fn capture_create_handler_hint(
    ctx: &MenuSyntaxMainHintContext<'_>,
    slug: &str,
) -> MenuSyntaxMainHintSnapshot {
    let typed_token = format!(";{slug}");
    let mut rows = vec![
        hint_row("Action", "Create blank capture handler"),
        hint_row(
            "File",
            &format!("~/.scriptkit/plugins/main/scripts/capture-{slug}-<slug>.ts"),
        ),
        hint_row("Registers", &format!("capture.v1 target \"{slug}\"")),
    ];
    if let Some((nearest_slug, nearest_title)) = nearest_capture_target_for_slug(slug, ctx.scripts)
    {
        rows.push(hint_row(
            "Similar",
            &format!(";{nearest_slug} — {nearest_title}"),
        ));
    }

    finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::CapturePickerCompanion,
        status_chips: Vec::new(),
        capture_validation: None,
        form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: ctx.raw_filter_text.to_string(),
        title: format!("No capture target named {typed_token}"),
        subtitle: None,
        mode_chip: Some(chip("; capture", MenuSyntaxMainHintTone::Accent)),
        status_chip: Some(chip("new target", MenuSyntaxMainHintTone::Neutral)),
        rows,
        fragment_preview: None,
        primary_hint: Some("Press Enter to create the handler scaffold.".to_string()),
        secondary_hint: Some("Press Cmd+Enter to ask AI to draft the handler first.".to_string()),
        example: None,
        examples: Vec::new(),
        warning: None,
        active_head: None,
        active_head_value_partial: None,
        accessibility_label: String::new(),
    })
}

fn capture_composer_hint(
    ctx: &MenuSyntaxMainHintContext<'_>,
) -> Option<MenuSyntaxMainHintSnapshot> {
    let (target, invocation) = match ctx.mode.capture_for(ctx.raw_filter_text) {
        Some(invocation) => (invocation.target.as_str(), Some(invocation)),
        None => match ctx.mode.incomplete_for(ctx.raw_filter_text) {
            Some(incomplete) => match &incomplete.kind {
                IncompleteKind::MissingCaptureBody(target) => (target.as_str(), None),
                _ => return None,
            },
            None => return None,
        },
    };

    let mut rows = Vec::new();
    if let Some(resolution) = resolve_capture_target(target) {
        rows.push(hint_row("Target", resolution.canonical_target_str()));
        rows.push(hint_row(
            "Operation",
            display_capture_operation(&resolution, invocation),
        ));
        if let Some(alias_of) = resolution.target_alias_of_str() {
            let show_alias = !(resolution.canonical_target == CanonicalCaptureTarget::Note
                && target.eq_ignore_ascii_case("notes"));
            if show_alias {
                rows.push(hint_row(
                    "Alias",
                    &format!("Compatibility alias of ;{alias_of}"),
                ));
            }
        }
        if target.eq_ignore_ascii_case("snippet") {
            rows.push(hint_row(
                "Body separator",
                "Use -- before code or long snippet text",
            ));
        }
    }
    if let Some(invocation) = invocation {
        rows.extend(capture_preview_rows(invocation));
    }
    if rows.is_empty() {
        rows.push(hint_row("Body", "Waiting for text"));
    }
    let schema = builtin_schema(target);
    let priority_allowed = schema.as_ref().is_some_and(|schema| {
        schema
            .optional
            .iter()
            .chain(schema.required.iter())
            .any(|field| matches!(field, FieldRequirement::Priority))
    });
    let priority_unset = invocation.is_none_or(|invocation| invocation.priority.is_none());
    if priority_allowed && priority_unset && raw_last_token_is_priority_prefix(ctx.raw_filter_text)
    {
        rows.push(MenuSyntaxMainHintRow {
            label: "Priority choices".to_string(),
            value: FieldRequirement::Priority.enum_values().join(" "),
            chips: vec![chip("schema", MenuSyntaxMainHintTone::Accent)],
        });
    }
    let has_tags = invocation
        .map(|invocation| !invocation.tags.is_empty())
        .unwrap_or(false);
    if !has_tags {
        rows.push(hint_row(
            "Tags",
            "Optional labels, e.g. #errands #client/acme",
        ));
    }
    let mut ranking_warning = None;
    if let Some(invocation) = invocation {
        let ranking = crate::menu_syntax::explain_capture_handler_ranking(ctx.scripts, invocation);
        if let Some(winner) = ranking.winner.as_ref() {
            rows.push(hint_row("Handler", &winner.script_name));
            rows.push(hint_row("Why selected", &winner.reason_parts.join(" · ")));
        } else {
            rows.push(hint_row(
                "Handler",
                &format!("No registered ;{target} handler"),
            ));
        }
        if !ranking.alternatives.is_empty() {
            let alternatives = ranking
                .alternatives
                .iter()
                .take(3)
                .map(|row| row.script_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            rows.push(hint_row("Other matches", &alternatives));
        }
        if let Some(warning) = ranking.warning {
            rows.push(hint_row("Handler conflict", &warning));
            ranking_warning = Some(warning);
        }
    } else {
        rows.push(hint_row(
            "Handler",
            &format!("Best matching ;{target} handler"),
        ));
    }

    let has_body = invocation
        .map(|invocation| {
            !invocation.body.trim().is_empty()
                || invocation.url.is_some()
                || todo_alias_has_selected_todo_ref(invocation)
                || note_update_has_selected_note_and_fields(invocation)
        })
        .unwrap_or(false);

    let clock = crate::menu_syntax::date::MenuSyntaxClock::local_now();
    let accepts = capture_accepts_for_target(ctx.scripts, target);
    let resolved = invocation.map(|inv| {
        crate::menu_syntax::date::resolve_capture_dates_with_accepts(inv, &clock, &accepts)
    });
    let capture_validation =
        capture_validation_snapshot(target, invocation, resolved.as_ref(), ctx.scripts, &accepts);

    // Run 12 Pass 10 — surface unresolved date phrases (e.g. `due:asdf`)
    // by routing the live invocation through Pass-34's resolve_capture_dates.
    let unresolved_dates: Vec<crate::menu_syntax::date::UnresolvedDate> = resolved
        .as_ref()
        .map(|resolved| resolved.unresolved_dates.clone())
        .unwrap_or_default();
    if !unresolved_dates.is_empty() {
        let phrases: Vec<String> = unresolved_dates
            .iter()
            .map(|u| format!("{:?}: {}", u.role, u.source).to_lowercase())
            .collect();
        rows.push(hint_row("Unresolved", &phrases.join(", ")));
        rows.push(MenuSyntaxMainHintRow {
            label: "Date suggestions".to_string(),
            value: "today, tomorrow, friday, mon, 9am, noon, eod".to_string(),
            chips: vec![chip("schema", MenuSyntaxMainHintTone::Accent)],
        });
    }
    if let Some(invocation) = invocation {
        rows.extend(object_ref_hint_rows(invocation));
    }

    Some(finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::CaptureComposer,
        unresolved_dates,
        menu_syntax_ai_proposal: ctx.menu_syntax_ai_proposal.cloned(),
        raw_filter_text: ctx.raw_filter_text.to_string(),
        title: resolve_capture_target(target)
            .map(|resolution| format!("Capture {}", resolution.title))
            .unwrap_or_else(|| format!("Capture {target}")),
        subtitle: Some("Enter saves this as structured local data.".to_string()),
        mode_chip: None,
        status_chip: None,
        status_chips: Vec::new(),
        capture_validation,
        form: None,
        rows,
        fragment_preview: invocation
            .zip(resolved.as_ref())
            .and_then(|(invocation, resolved)| {
                fragment_preview_for_capture(invocation, resolved, &clock, &accepts)
            }),
        primary_hint: Some(if has_body {
            "Press Enter to capture.".to_string()
        } else {
            "Type what you want to save.".to_string()
        }),
        secondary_hint: Some(if has_tags {
            "Tags label this capture for grouping later. They are not launcher filters here."
                .to_string()
        } else if target.eq_ignore_ascii_case("note") || target.eq_ignore_ascii_case("notes") {
            "Tags group the note. due:/at:/start:/end: adds note metadata; url: and key=value attach fields."
                .to_string()
        } else {
            "Tags group the saved item. p1-p4 sets priority; due:/at:/start: adds dates; key=value adds fields."
                .to_string()
        }),
        example: Some(
            target_examples(public_examples_target(target))
                .into_iter()
                .next()
                .unwrap_or_else(|| format!(";{target} Example")),
        ),
        examples: target_examples(public_examples_target(target)),
        warning: ranking_warning,
        active_head: None,
        active_head_value_partial: None,
        accessibility_label: String::new(),
    }))
}

fn object_ref_hint_rows(invocation: &CaptureInvocation) -> Vec<MenuSyntaxMainHintRow> {
    crate::menu_syntax::object_refs_for_raw_capture(&invocation.target, &invocation.raw)
        .into_iter()
        .filter(|object_ref| object_ref.resolved)
        .take(3)
        .map(|object_ref| object_ref_hint_row(&object_ref))
        .collect()
}

fn object_ref_hint_row(object_ref: &CaptureObjectRef) -> MenuSyntaxMainHintRow {
    let token = object_ref
        .token
        .clone()
        .unwrap_or_else(|| format!("@{}:{}", object_ref.kind.as_str(), object_ref.id));
    let deeplink = object_ref
        .deeplink
        .clone()
        .unwrap_or_else(|| object_ref_deeplink(object_ref.kind, &object_ref.id));
    MenuSyntaxMainHintRow {
        label: "Bound object".to_string(),
        value: deeplink,
        chips: vec![
            chip(&token, MenuSyntaxMainHintTone::Success),
            chip(object_ref.kind.as_str(), MenuSyntaxMainHintTone::Accent),
        ],
    }
}

fn display_capture_operation<'a>(
    resolution: &'a CaptureTargetResolution,
    invocation: Option<&CaptureInvocation>,
) -> &'a str {
    let Some(invocation) = invocation else {
        return resolution.operation.as_str();
    };
    match resolution.canonical_target {
        // Snippet: normalize_snippet_capture_invocation rewrites `body` to the
        // post-`--` code, dropping the typed operation word — derive the
        // operation from the same parser the executor uses so the hint and
        // execution always agree.
        CanonicalCaptureTarget::Snippet => {
            match super::snippet_scriptlet::parse_snippet_scriptlet_capture(invocation) {
                Ok(draft) => match draft.operation {
                    super::snippet_scriptlet::SnippetScriptletOperation::Create => "create",
                    super::snippet_scriptlet::SnippetScriptletOperation::Update => "update",
                    super::snippet_scriptlet::SnippetScriptletOperation::Delete => "delete",
                },
                Err(_) => first_app_owned_operation_word(&invocation.body)
                    .unwrap_or_else(|| resolution.operation.as_str()),
            }
        }
        CanonicalCaptureTarget::Link => first_app_owned_operation_word(&invocation.body)
            .unwrap_or_else(|| resolution.operation.as_str()),
        CanonicalCaptureTarget::Note => first_app_owned_operation_word(&invocation.body)
            .unwrap_or_else(|| {
                if note_has_selected_note_ref(invocation) {
                    "update"
                } else {
                    "create"
                }
            }),
        _ => resolution.operation.as_str(),
    }
}

fn note_update_has_selected_note_and_fields(invocation: &CaptureInvocation) -> bool {
    if !matches!(
        invocation.target.to_ascii_lowercase().as_str(),
        "note" | "notes"
    ) || !note_has_selected_note_ref(invocation)
    {
        return false;
    }
    invocation.url.is_some()
        || !invocation.tags.is_empty()
        || !invocation.kv.is_empty()
        || !invocation.date_phrases.is_empty()
        || matches!(
            first_app_owned_operation_word(&invocation.body),
            Some("update" | "delete")
        )
}

fn note_has_selected_note_ref(invocation: &CaptureInvocation) -> bool {
    crate::menu_syntax::object_refs_for_raw_capture(&invocation.target, &invocation.raw)
        .into_iter()
        .any(|object_ref| {
            object_ref.role == "primary"
                && object_ref.kind == crate::menu_syntax::CaptureObjectKind::Note
                && object_ref.resolved
        })
}

fn todo_alias_has_selected_todo_ref(invocation: &CaptureInvocation) -> bool {
    if !matches!(
        invocation.target.to_ascii_lowercase().as_str(),
        "reminder" | "snooze" | "defer"
    ) {
        return false;
    }
    crate::menu_syntax::object_refs_for_raw_capture(&invocation.target, &invocation.raw)
        .into_iter()
        .any(|object_ref| {
            object_ref.role == "primary"
                && object_ref.kind == crate::menu_syntax::CaptureObjectKind::Todo
                && object_ref.resolved
        })
}

fn first_app_owned_operation_word(body: &str) -> Option<&'static str> {
    let first = body.split_whitespace().next()?.to_ascii_lowercase();
    match first.as_str() {
        "add" | "create" | "save" => Some("create"),
        "update" | "edit" => Some("update"),
        "remove" | "rm" | "delete" => Some("delete"),
        _ => None,
    }
}

fn capture_accepts_for_target(scripts: &[Arc<Script>], target: &str) -> Vec<String> {
    let mut accepts = crate::menu_syntax::capture_accepts_for_target_from_scripts(scripts, target);
    if accepts.is_empty() {
        accepts.extend(crate::menu_syntax::date::builtin_capture_accepts_for_target(target));
    }
    accepts
}

fn fragment_preview_for_capture(
    invocation: &CaptureInvocation,
    resolved: &crate::menu_syntax::date::ResolvedCaptureInvocation,
    clock: &crate::menu_syntax::date::MenuSyntaxClock,
    accepts: &[String],
) -> Option<MenuSyntaxFragmentPreviewSnapshot> {
    let mut rows = Vec::new();
    let body = resolved.body.trim();
    if !body.is_empty() {
        let (source, source_span) = source_for_text(invocation, body);
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: MenuSyntaxFragmentRole::Subject,
            label: "Subject".to_string(),
            value: body.to_string(),
            source,
            source_span,
            status: MenuSyntaxFragmentStatus::Resolved,
            tone: MenuSyntaxMainHintTone::Neutral,
            chips: Vec::new(),
        });
    }

    for date in &resolved.dates {
        let has_end = date.end_iso.is_some();
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: if has_end {
                MenuSyntaxFragmentRole::DateRange
            } else {
                MenuSyntaxFragmentRole::Date
            },
            label: if has_end { "Date range" } else { "Date" }.to_string(),
            value: date_preview_value(date, clock),
            source: date.source.clone(),
            source_span: source_span_for_fragment(invocation, &date.source, date.source_span),
            status: MenuSyntaxFragmentStatus::Resolved,
            tone: MenuSyntaxMainHintTone::Info,
            chips: vec![chip("resolved", MenuSyntaxMainHintTone::Info)],
        });
    }

    if let Some(duration) = resolved.duration_resolved.as_ref() {
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: MenuSyntaxFragmentRole::Duration,
            label: "Duration".to_string(),
            value: format!("{} ({} minutes)", duration.source, duration.minutes),
            source: duration.source.clone(),
            source_span: source_span_for_fragment(
                invocation,
                &duration.source,
                duration.source_span,
            ),
            status: MenuSyntaxFragmentStatus::Resolved,
            tone: MenuSyntaxMainHintTone::Warning,
            chips: Vec::new(),
        });
    }

    if let Some(recurrence) = resolved.recurrence.as_ref() {
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: MenuSyntaxFragmentRole::Recurrence,
            label: "Recurrence".to_string(),
            value: format!("{} ({})", recurrence.label, recurrence.rrule),
            source: recurrence.source.clone(),
            source_span: source_span_for_fragment(
                invocation,
                &recurrence.source,
                recurrence.source_span,
            ),
            status: MenuSyntaxFragmentStatus::Resolved,
            tone: MenuSyntaxMainHintTone::Success,
            chips: Vec::new(),
        });
    }

    if !resolved.tags.is_empty() {
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: MenuSyntaxFragmentRole::Tag,
            label: "Tags".to_string(),
            value: resolved
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(", "),
            source: String::new(),
            source_span: (0, 0),
            status: MenuSyntaxFragmentStatus::Resolved,
            tone: MenuSyntaxMainHintTone::Accent,
            chips: Vec::new(),
        });
    }

    for (key, value) in &resolved.kv {
        let source = format!("{key}={value}");
        let (_, source_span) = source_for_text(invocation, &source);
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: MenuSyntaxFragmentRole::Kv,
            label: key.clone(),
            value: value.clone(),
            source,
            source_span,
            status: MenuSyntaxFragmentStatus::Resolved,
            tone: MenuSyntaxMainHintTone::Accent,
            chips: Vec::new(),
        });
    }

    for unresolved in &resolved.unresolved_dates {
        rows.push(MenuSyntaxFragmentPreviewRow {
            role: MenuSyntaxFragmentRole::Unresolved,
            label: "Unresolved date".to_string(),
            value: format!(
                "{}: {}",
                date_role_label(&unresolved.role),
                unresolved.source
            ),
            source: unresolved.source.clone(),
            source_span: unresolved.source_span,
            status: MenuSyntaxFragmentStatus::Unresolved,
            tone: MenuSyntaxMainHintTone::Muted,
            chips: Vec::new(),
        });
    }

    if rows.is_empty()
        && invocation.body.trim().is_empty()
        && accepts.is_empty()
        && invocation.tags.is_empty()
        && invocation.kv.is_empty()
    {
        return None;
    }

    (!rows.is_empty()).then_some(MenuSyntaxFragmentPreviewSnapshot { rows })
}

fn date_preview_value(
    date: &crate::menu_syntax::date::ResolvedDate,
    clock: &crate::menu_syntax::date::MenuSyntaxClock,
) -> String {
    let mut value = date.source.clone();
    let resolved = if let Some(end_iso) = date.end_iso.as_deref() {
        format!(
            "resolved {}-{} {}",
            compact_datetime(&date.iso),
            compact_time(end_iso),
            clock.timezone_label
        )
    } else {
        format!(
            "resolved {} {}",
            compact_datetime(&date.iso),
            clock.timezone_label
        )
    };
    value.push_str(" (");
    value.push_str(&resolved);
    value.push(')');
    value
}

fn compact_datetime(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| crate::formatting::format_absolute_datetime(dt.with_timezone(&chrono::Utc)))
        .unwrap_or_else(|_| iso.to_string())
}

fn compact_time(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn source_for_text(invocation: &CaptureInvocation, text: &str) -> (String, (usize, usize)) {
    let span = source_span_for_text(invocation, text).unwrap_or((0, 0));
    let source = if span.0 < span.1 {
        invocation.raw[span.0..span.1].to_string()
    } else {
        String::new()
    };
    (source, span)
}

fn source_span_for_fragment(
    invocation: &CaptureInvocation,
    source: &str,
    span: (usize, usize),
) -> (usize, usize) {
    if span.1 <= invocation.raw.len()
        && invocation.raw.is_char_boundary(span.0)
        && invocation.raw.is_char_boundary(span.1)
        && invocation.raw.get(span.0..span.1) == Some(source)
    {
        return span;
    }
    source_span_for_text(invocation, source).unwrap_or((0, 0))
}

fn source_span_for_text(invocation: &CaptureInvocation, text: &str) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }
    let start = crate::menu_syntax::prefix_span_for_input(&invocation.raw)
        .map(|range| range.end)
        .unwrap_or(0);
    let relative = invocation.raw[start..].find(text)?;
    let begin = start + relative;
    let end = begin + text.len();
    (invocation.raw.is_char_boundary(begin) && invocation.raw.is_char_boundary(end))
        .then_some((begin, end))
}

fn command_picker_companion_hint(
    ctx: &MenuSyntaxMainHintContext<'_>,
    snapshot: &TriggerPickerSnapshot,
) -> Option<MenuSyntaxMainHintSnapshot> {
    let selected = selected_picker_row(snapshot, ctx.picker_selected_row_id);
    if let Some(row) = selected {
        if !row.enabled || row.badges.iter().any(|badge| badge == "duplicate") {
            return Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                kind: MenuSyntaxMainHintKind::CommandPickerCompanion,
                status_chips: Vec::new(),
                capture_validation: None,
                form: None,
                unresolved_dates: Vec::new(),
                menu_syntax_ai_proposal: None,
                raw_filter_text: ctx.raw_filter_text.to_string(),
                title: "Ambiguous command".to_string(),
                subtitle: Some("Multiple registered commands share this command head.".to_string()),
                mode_chip: Some(chip("> run", MenuSyntaxMainHintTone::Accent)),
                status_chip: Some(chip("blocked", MenuSyntaxMainHintTone::Warning)),
                rows: row
                    .token
                    .as_deref()
                    .map(|token| vec![hint_row("Command", token)])
                    .unwrap_or_default(),
                fragment_preview: None,
                primary_hint: Some("Enter will not run this until aliases are unique.".to_string()),
                secondary_hint: Some(
                    "Give one command a unique alias or choose another command.".to_string(),
                ),
                example: None,
                examples: vec![">deploy env:dev #demo -- --dry-run".to_string()],
                warning: row.detail.clone().or_else(|| {
                    Some("Ambiguous command head; give one command a unique alias.".to_string())
                }),
                active_head: None,
                active_head_value_partial: None,
                accessibility_label: String::new(),
            }));
        }
    }

    let mut rows = Vec::new();
    if let Some(row) = selected {
        if let Some(token) = row.token.as_deref() {
            rows.push(hint_row("Selected", token));
        }
        if let Some(kind) = row.badges.first() {
            rows.push(hint_row("Kind", kind));
        }
    }

    Some(finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::CommandPickerCompanion,
        status_chips: Vec::new(),
        capture_validation: None,
        form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: ctx.raw_filter_text.to_string(),
        title: selected
            .map(|row| format!("Run {}", row.token.as_deref().unwrap_or("a command")))
            .unwrap_or_else(|| "Run a registered command".to_string()),
        subtitle: Some("Choose a registered Script Kit command in the popup.".to_string()),
        mode_chip: Some(chip("> run", MenuSyntaxMainHintTone::Accent)),
        status_chip: None,
        rows,
        fragment_preview: None,
        primary_hint: Some("After choosing: add fields, #tags, or argv after --.".to_string()),
        secondary_hint: None,
        example: selected
            .and_then(|row| row.example.clone())
            .or_else(|| Some(">ps-env env:dev #demo -- --dry-run".to_string())),
        examples: vec![
            ">ps-env env:dev #demo -- --dry-run".to_string(),
            ">test-menu-syntax -- --watch".to_string(),
        ],
        warning: None,
        active_head: None,
        active_head_value_partial: None,
        accessibility_label: String::new(),
    }))
}

fn command_composer_hint(
    ctx: &MenuSyntaxMainHintContext<'_>,
) -> Option<MenuSyntaxMainHintSnapshot> {
    let invocation = ctx.mode.command_for(ctx.raw_filter_text)?;
    let resolution = resolve_command(invocation, ctx.scripts, ctx.scriptlets);

    let mut rows = command_preview_rows(invocation);
    if rows.is_empty() {
        rows.push(hint_row("Command", &format!(">{}", invocation.head)));
    }

    // Author-declared command schema rows: if any loaded script registers a
    // `command.v1` handler with `head` matching this invocation, append rows
    // describing the expected args + flags. These are the rows the
    // `setFilter ">deploy"` getState receipt looks for in
    // `menuSyntaxMainHint.rows`.
    let command_schema = script_command_schema_for(ctx.scripts, &invocation.head);
    if let Some(spec) = command_schema.as_ref() {
        rows.extend(command_schema_rows(spec));
    }

    let (title, status_chip, primary_hint, warning) = match resolution {
        CommandHintResolution::Unique { title, kind } => {
            rows.insert(0, hint_row("Target", &format!("{title} ({kind})")));
            (
                format!("Run {}", invocation.head),
                Some(chip("ready", MenuSyntaxMainHintTone::Success)),
                "Press Enter to run the registered Script Kit command.".to_string(),
                None,
            )
        }
        CommandHintResolution::Ambiguous { count } => (
            "Ambiguous command".to_string(),
            Some(chip("blocked", MenuSyntaxMainHintTone::Warning)),
            "Enter will not run this while multiple commands match.".to_string(),
            Some(format!(
                "{count} registered commands use >{}. Give one command a unique alias.",
                invocation.head
            )),
        ),
        CommandHintResolution::None => (
            format!("No registered command named !{}", invocation.head),
            Some(chip("not found", MenuSyntaxMainHintTone::Warning)),
            "This will not run a shell command.".to_string(),
            Some("Backspace to search normally, or type ! to choose a command.".to_string()),
        ),
    };

    Some(finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::CommandComposer,
        status_chips: Vec::new(),
        capture_validation: None,
        form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: ctx.raw_filter_text.to_string(),
        title,
        subtitle: Some("Command invocation is explicit Script Kit execution.".to_string()),
        mode_chip: Some(chip("> run", MenuSyntaxMainHintTone::Accent)),
        status_chip,
        rows,
        fragment_preview: None,
        primary_hint: Some(primary_hint),
        secondary_hint: Some(
            "Fields use key:value; #tags are command metadata; argv after -- is passed through."
                .to_string(),
        ),
        example: Some(format!(">{} env:dev #demo -- --dry-run", invocation.head)),
        examples: vec![
            format!(">{} env:dev #demo -- --dry-run", invocation.head),
            format!(">{} -- --help", invocation.head),
        ],
        warning,
        active_head: None,
        active_head_value_partial: None,
        accessibility_label: String::new(),
    }))
}

/// Extract the free-text words that trail `head + value_token` in the raw
/// filter. Used by synthetic empty hints to render full
/// "No scriptlets match `zzz`." copy even when the parser declines to
/// promote the input to a full `AdvancedQuery`.
fn free_words_after_qualifier(raw: &str, head: &str, value_token: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let needle = format!(
        "{}{}",
        head.to_ascii_lowercase(),
        value_token.to_ascii_lowercase()
    );
    if let Some(pos) = lower.find(&needle) {
        let after = &raw[pos + needle.len()..];
        return after.trim().to_string();
    }
    String::new()
}

/// Build an empty hint snapshot from the active-head detector alone,
/// for inputs the parser cannot turn into an `AdvancedQuery` yet
/// (bare `has:`, `has:sh` without a leading `:`, source-only `c:`, etc.).
/// Keeps the snapshot kind `AdvancedQueryEmpty` so automation consumers
/// keep working without a new variant.
fn synthetic_active_head_empty_hint(
    raw_filter_text: &str,
    active: &ActiveHeadContext,
) -> Option<MenuSyntaxMainHintSnapshot> {
    let raw_active_head = Some(active.head.clone());
    let raw_active_partial = active.value_partial_opt();

    match active.kind {
        ActiveHeadKind::Has => {
            let matching = has_field_rows_for_partial(&active.value_partial);
            let has_unknown = matching.is_empty() && !active.value_partial.is_empty();

            let (title, primary_hint, examples, rows) = if has_unknown {
                let field = &active.value_partial;
                (
                    format!("No scripts or scriptlets have a `{field}` field."),
                    "Try `has:shortcut`, `has:alias`, or `has:menuSyntax`.".to_string(),
                    has_unknown_example_tokens(),
                    vec![
                        hint_row("Typed field", field),
                        hint_row("Known fields", "has:shortcut, has:alias, has:menuSyntax"),
                    ],
                )
            } else if active.value_partial.is_empty() {
                let all = has_field_rows_for_partial("");
                (
                    "Filter by metadata field".to_string(),
                    "Choose a field from the list or finish typing a metadata key.".to_string(),
                    all.iter().map(|r| r.label.clone()).collect(),
                    all,
                )
            } else {
                (
                    "Filter by metadata field".to_string(),
                    "Choose a field from the list or finish typing a metadata key.".to_string(),
                    matching.iter().map(|r| r.label.clone()).collect(),
                    matching,
                )
            };

            Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
                status_chips: Vec::new(),
                capture_validation: None,
                form: None,
                unresolved_dates: Vec::new(),
                menu_syntax_ai_proposal: None,
                raw_filter_text: raw_filter_text.to_string(),
                title,
                subtitle: None,
                mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
                status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
                rows,
                fragment_preview: None,
                primary_hint: Some(primary_hint),
                secondary_hint: None,
                example: examples.first().cloned(),
                examples,
                warning: None,
                active_head: raw_active_head,
                active_head_value_partial: raw_active_partial,
                accessibility_label: String::new(),
            }))
        }
        ActiveHeadKind::Source => {
            // Only render source-attached zero copy when there IS a typed
            // partial. Bare source heads (`c:`) keep the prior no-hint
            // behavior owned by [[removed-docs Filter Legend]].
            if active.value_partial.is_empty() {
                return None;
            }
            let spec = source_head_spec_for_token(&active.head)?;
            let label = source_zero_copy_label(spec);
            let value = &active.value_partial;
            let example = format!("{} {}", spec.short.unwrap_or(spec.canonical), "order id");
            Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
                status_chips: Vec::new(),
                capture_validation: None,
                form: None,
                unresolved_dates: Vec::new(),
                menu_syntax_ai_proposal: None,
                raw_filter_text: raw_filter_text.to_string(),
                title: format!("No {label} match `{value}`."),
                subtitle: None,
                mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
                status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
                rows: Vec::new(),
                fragment_preview: None,
                primary_hint: Some("Press Esc to clear the filter.".to_string()),
                secondary_hint: None,
                example: Some(example.clone()),
                examples: vec![example],
                warning: None,
                active_head: raw_active_head,
                active_head_value_partial: raw_active_partial,
                accessibility_label: String::new(),
            }))
        }
        ActiveHeadKind::TypeQualifier => {
            // Parser may return Incomplete for `:type:scriptlet zzz` etc.
            // when free text trails the qualifier; synthesize a
            // kind-aware empty hint so the receipt copy still works.
            let head_token = active.value_partial.trim();
            if head_token.is_empty() {
                return None;
            }
            let kind_label = type_value_label(head_token);
            // The active_head detector cuts value_partial at the first
            // whitespace, so it never includes the trailing free-text
            // words (`:type:scriptlet zzz` → value_partial="scriptlet").
            // Pull free words straight from the raw input so the title
            // can render them.
            let free_words = free_words_after_qualifier(raw_filter_text, &active.head, head_token);
            let title = if free_words.is_empty() {
                format!("No {kind_label} match this filter.")
            } else {
                format!("No {kind_label} match `{free_words}`.")
            };
            let primary_hint = format!("Remove `type:{head_token}` to widen.");
            let example_token = format!("type:{head_token} shell");
            let mut rows = vec![
                hint_row("Filter", &format!("type:{head_token}")),
                hint_row("Scope", &format!("{kind_label} only")),
            ];
            if !free_words.is_empty() {
                rows.push(hint_row("Search words", &free_words));
            }
            rows.push(hint_row(
                "Recovery",
                &format!("Remove `type:{head_token}` to widen results"),
            ));
            Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
                status_chips: Vec::new(),
                capture_validation: None,
                form: None,
                unresolved_dates: Vec::new(),
                menu_syntax_ai_proposal: None,
                raw_filter_text: raw_filter_text.to_string(),
                title,
                subtitle: None,
                mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
                status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
                rows,
                fragment_preview: None,
                primary_hint: Some(primary_hint),
                secondary_hint: None,
                example: Some(example_token.clone()),
                examples: vec![example_token],
                warning: None,
                active_head: raw_active_head,
                active_head_value_partial: raw_active_partial,
                accessibility_label: String::new(),
            }))
        }
        ActiveHeadKind::ShortcutQualifier => {
            let value = active.value_partial.trim();
            let title = format!("No shortcut-backed scripts or scriptlets match `{value}`.");
            let primary_hint = "Remove `shortcut:any` to widen.".to_string();
            Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
                status_chips: Vec::new(),
                capture_validation: None,
                form: None,
                unresolved_dates: Vec::new(),
                menu_syntax_ai_proposal: None,
                raw_filter_text: raw_filter_text.to_string(),
                title,
                subtitle: None,
                mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
                status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
                rows: Vec::new(),
                fragment_preview: None,
                primary_hint: Some(primary_hint),
                secondary_hint: None,
                example: Some("shortcut:any".to_string()),
                examples: vec!["shortcut:any".to_string()],
                warning: None,
                active_head: raw_active_head,
                active_head_value_partial: raw_active_partial,
                accessibility_label: String::new(),
            }))
        }
        ActiveHeadKind::TagQualifier | ActiveHeadKind::OtherQualifier => {
            let filter = if active.value_partial.trim().is_empty() {
                active.head.clone()
            } else {
                format!("{}{}", active.head, active.value_partial.trim())
            };
            let example = format!("{filter} shell");
            Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
                status_chips: Vec::new(),
                capture_validation: None,
                form: None,
                unresolved_dates: Vec::new(),
                menu_syntax_ai_proposal: None,
                raw_filter_text: raw_filter_text.to_string(),
                title: "No matching items match this filter.".to_string(),
                subtitle: None,
                mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
                status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
                rows: vec![
                    hint_row("Filter", &filter),
                    hint_row("Scope", "matching items only"),
                    hint_row("Recovery", &format!("Remove `{filter}` to widen results")),
                ],
                fragment_preview: None,
                primary_hint: Some(format!("Remove `{filter}` to widen.")),
                secondary_hint: None,
                example: Some(example.clone()),
                examples: vec![example],
                warning: None,
                active_head: raw_active_head,
                active_head_value_partial: raw_active_partial,
                accessibility_label: String::new(),
            }))
        }
        _ => None,
    }
}

fn advanced_query_empty_hint(
    raw_filter_text: &str,
    query: &AdvancedQuery,
    active: Option<&ActiveHeadContext>,
) -> Option<MenuSyntaxMainHintSnapshot> {
    // Head-aware branches first so the snapshot drops generic
    // tag/type leak from the previous shape. Each branch builds a
    // dedicated `AdvancedQueryEmpty` snapshot; fall back to the
    // generic copy at the bottom for predicates we have no specialized
    // story for yet.
    let mut rows = Vec::new();
    if !query.predicates.is_empty() {
        rows.push(MenuSyntaxMainHintRow {
            label: "Filters".to_string(),
            value: query
                .predicates
                .iter()
                .map(predicate_user_label)
                .collect::<Vec<_>>()
                .join(" · "),
            chips: Vec::new(),
        });
    }
    if !query.free_text.is_empty() {
        rows.push(hint_row("Search words", &query.free_text));
    }

    let raw_active_head = active.map(|a| a.head.clone());
    let raw_active_partial = active.and_then(|a| a.value_partial_opt());

    // `has:<field>` family — the screenshot regression. Examples and
    // rows must come from HAS_FIELD_SPECS only.
    let has_field_value = has_field_predicate(query)
        .map(str::to_string)
        .or_else(|| match active {
            Some(a) if matches!(a.kind, ActiveHeadKind::Has) => Some(a.value_partial.clone()),
            _ => None,
        });
    if let Some(field_value) = has_field_value {
        let matching = has_field_rows_for_partial(&field_value);
        let has_unknown = matching.is_empty() && !field_value.is_empty();

        let (title, primary_hint, examples) = if has_unknown {
            (
                format!("No scripts or scriptlets have a `{field_value}` field."),
                "Try `has:shortcut`, `has:alias`, or `has:menuSyntax`.".to_string(),
                has_unknown_example_tokens(),
            )
        } else if field_value.is_empty() {
            (
                "Filter by metadata field".to_string(),
                "Choose a field from the list or finish typing a metadata key.".to_string(),
                has_field_rows_for_partial("")
                    .into_iter()
                    .map(|r| r.label)
                    .collect(),
            )
        } else {
            (
                "Filter by metadata field".to_string(),
                "Choose a field from the list or finish typing a metadata key.".to_string(),
                matching.iter().map(|r| r.label.clone()).collect(),
            )
        };

        let mut all_rows = rows.clone();
        if has_unknown {
            all_rows.push(hint_row("Typed field", &field_value));
            all_rows.push(hint_row(
                "Known fields",
                "has:shortcut, has:alias, has:menuSyntax",
            ));
        } else {
            all_rows.extend(matching);
        }

        return Some(finalize_hint(MenuSyntaxMainHintSnapshot {
            kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
            status_chips: Vec::new(),
            capture_validation: None,
            form: None,
            unresolved_dates: Vec::new(),
            menu_syntax_ai_proposal: None,
            raw_filter_text: raw_filter_text.to_string(),
            title,
            subtitle: None,
            mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
            status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
            rows: all_rows,
            fragment_preview: None,
            primary_hint: Some(primary_hint),
            secondary_hint: None,
            example: examples.first().cloned(),
            examples,
            warning: None,
            active_head: raw_active_head,
            active_head_value_partial: raw_active_partial,
            accessibility_label: String::new(),
        }));
    }

    // Source-attached query (e.g. `c:zzz`, `v:foo`, `f:bar`).
    if let Some(active_ctx) = active {
        if matches!(active_ctx.kind, ActiveHeadKind::Source) && !active_ctx.value_partial.is_empty()
        {
            if let Some(spec) = source_head_spec_for_token(&active_ctx.head) {
                let label = source_zero_copy_label(spec);
                let value = &active_ctx.value_partial;
                let title = format!("No {label} match `{value}`.");
                let primary_hint = "Press Esc to clear the filter.".to_string();
                let example = format!("{} {}", spec.short.unwrap_or(spec.canonical), "order id");
                return Some(finalize_hint(MenuSyntaxMainHintSnapshot {
                    kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
                    status_chips: Vec::new(),
                    capture_validation: None,
                    form: None,
                    unresolved_dates: Vec::new(),
                    menu_syntax_ai_proposal: None,
                    raw_filter_text: raw_filter_text.to_string(),
                    title,
                    subtitle: None,
                    mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
                    status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
                    rows,
                    fragment_preview: None,
                    primary_hint: Some(primary_hint),
                    secondary_hint: None,
                    example: Some(example.clone()),
                    examples: vec![example],
                    warning: None,
                    active_head: raw_active_head,
                    active_head_value_partial: raw_active_partial,
                    accessibility_label: String::new(),
                }));
            }
        }
    }

    // `:type:<kind>` family — title and copy use the kind label.
    if let Some(kind) = type_predicate_kind(query) {
        let kind_label = artifact_kind_zero_copy_label(kind);
        let raw_kind = match kind {
            ArtifactKind::Script => "script",
            ArtifactKind::Scriptlet => "scriptlet",
            ArtifactKind::Skill => "skill",
            ArtifactKind::Agent => "agent",
            ArtifactKind::Builtin => "builtin",
            ArtifactKind::App => "app",
            ArtifactKind::Window => "window",
            ArtifactKind::File => "file",
            ArtifactKind::Note => "note",
            ArtifactKind::Todo => "todo",
            ArtifactKind::AgentChatHistory => "agent_chat-history",
            ArtifactKind::AiVault => "ai-vault",
            ArtifactKind::ClipboardHistory => "clipboard-history",
            ArtifactKind::DictationHistory => "dictation-history",
            ArtifactKind::BrowserTab => "browser-tab",
            ArtifactKind::BrowserHistory => "browser-history",
            ArtifactKind::Fallback => "fallback",
            ArtifactKind::Issue => "issue",
        };
        let words = query.free_text.trim();
        let title = if words.is_empty() {
            format!("No {kind_label} match this filter.")
        } else {
            format!("No {kind_label} match `{words}`.")
        };
        let primary_hint = format!("Remove `type:{raw_kind}` to widen.");
        let example_token = format!("type:{raw_kind} shell");
        return Some(finalize_hint(MenuSyntaxMainHintSnapshot {
            kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
            status_chips: Vec::new(),
            capture_validation: None,
            form: None,
            unresolved_dates: Vec::new(),
            menu_syntax_ai_proposal: None,
            raw_filter_text: raw_filter_text.to_string(),
            title,
            subtitle: None,
            mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
            status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
            rows,
            fragment_preview: None,
            primary_hint: Some(primary_hint),
            secondary_hint: None,
            example: Some(example_token.clone()),
            examples: vec![example_token],
            warning: None,
            active_head: raw_active_head,
            active_head_value_partial: raw_active_partial,
            accessibility_label: String::new(),
        }));
    }

    // `:shortcut:` family.
    if shortcut_predicate(query).is_some() {
        let words = query.free_text.trim();
        let title = if words.is_empty() {
            "No shortcut-backed scripts or scriptlets match this filter.".to_string()
        } else {
            format!("No shortcut-backed scripts or scriptlets match `{words}`.")
        };
        let primary_hint = "Remove `shortcut:any` to widen.".to_string();
        return Some(finalize_hint(MenuSyntaxMainHintSnapshot {
            kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
            status_chips: Vec::new(),
            capture_validation: None,
            form: None,
            unresolved_dates: Vec::new(),
            menu_syntax_ai_proposal: None,
            raw_filter_text: raw_filter_text.to_string(),
            title,
            subtitle: None,
            mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
            status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
            rows,
            fragment_preview: None,
            primary_hint: Some(primary_hint),
            secondary_hint: None,
            example: Some("shortcut:any".to_string()),
            examples: vec!["shortcut:any".to_string()],
            warning: None,
            active_head: raw_active_head,
            active_head_value_partial: raw_active_partial,
            accessibility_label: String::new(),
        }));
    }

    // `:tag:` / `:#tag` family — keep tag-specific copy and examples
    // since the previous behavior remains the right fix for this case.
    let tag = query.predicates.iter().find_map(predicate_tag);
    let title = tag
        .map(|tag| format!("No launcher items tagged #{tag}"))
        .unwrap_or_else(|| "No matches after these filters".to_string());
    let subtitle = tag
        .map(|tag| format!("After `:`, `#{tag}` is a tag filter on the launcher catalog."))
        .unwrap_or_else(|| "`:` narrows the launcher catalog before search words run.".to_string());
    let primary_hint = tag
        .map(|tag| format!("Try another tag, remove `:#{tag}`, or change the search words."))
        .unwrap_or_else(|| "Remove a filter or change the search words.".to_string());
    let (example, examples) = if let Some(tag) = tag {
        (
            Some(format!(":#{tag}")),
            vec![format!(":#{tag}"), format!(":tag:{tag}")],
        )
    } else {
        (None, Vec::new())
    };

    Some(finalize_hint(MenuSyntaxMainHintSnapshot {
        kind: MenuSyntaxMainHintKind::AdvancedQueryEmpty,
        status_chips: Vec::new(),
        capture_validation: None,
        form: None,
        unresolved_dates: Vec::new(),
        menu_syntax_ai_proposal: None,
        raw_filter_text: raw_filter_text.to_string(),
        title,
        subtitle: Some(subtitle),
        mode_chip: Some(chip(": refine", MenuSyntaxMainHintTone::Accent)),
        status_chip: Some(chip("no matches", MenuSyntaxMainHintTone::Muted)),
        rows,
        fragment_preview: None,
        primary_hint: Some(primary_hint),
        secondary_hint: None,
        example,
        examples,
        warning: None,
        active_head: raw_active_head,
        active_head_value_partial: raw_active_partial,
        accessibility_label: String::new(),
    }))
}

fn tag_boundary_rows() -> Vec<MenuSyntaxMainHintRow> {
    vec![
        hint_row("#work", "Plain launcher search"),
        hint_row(":#work", "Filter launcher rows tagged #work"),
        hint_row(";... #work", "Label the captured item as #work"),
    ]
}

/// Build the structured `CaptureValidation` snapshot for a `;target`
/// capture composer state. Returns None when the target has no registered
/// schema, e.g. `;github`.
///
/// Story: capture-validation-snapshot (Pass 22). Wires
/// [[crate::menu_syntax::capture_schema::validate]] into the snapshot the
/// hint card consumes without re-running the validation rules.
fn capture_validation_snapshot(
    target: &str,
    invocation: Option<&CaptureInvocation>,
    resolved: Option<&crate::menu_syntax::date::ResolvedCaptureInvocation>,
    scripts: &[Arc<Script>],
    accepts: &[String],
) -> Option<MenuSyntaxCaptureValidationSnapshot> {
    // Run 12 Pass 15 — `capture-dynamic-target-schema`. Resolve the schema
    // through the script-aware lookup so script-declared `capture.v1`
    // specs (e.g. `;expense` from `capture-expense-ledger.ts`) flow into
    // the live snapshot's `captureValidation`. Builtin still wins when
    // present; falls back to the first matching dynamic schema.
    let schema =
        crate::menu_syntax::capture_gate::resolve_capture_schema_for_target(target, scripts)?;
    // Use a synthetic empty payload when the user has only typed `;target ` —
    // the schema's `missing_required` still computes the correct Vec from
    // an empty body / no date_phrases / no kv. This matches the receipt
    // contract: `setFilter ";cal"` reports both `needs body` and `needs date`.
    let synthetic;
    let payload = match invocation {
        Some(inv) => inv,
        None => {
            synthetic = CaptureInvocation {
                target: target.to_string(),
                alias_form: CaptureAlias::CapturePrefix,
                body: String::new(),
                tags: vec![],
                priority: None,
                url: None,
                duration: None,
                kv: vec![],
                date_phrases: vec![],
                raw: format!(";{target}"),
            };
            &synthetic
        }
    };
    let validation_payload = payload_for_capture_validation(payload, resolved);
    let result = validate(&validation_payload, &schema);
    // Run 12 Pass 14 — re-run the gate on the same payload+schema so the
    // exact HUD nudge string the runtime would show on Enter is part of the
    // snapshot. Pure read; same inputs as `validate` so no extra branching.
    let gate_decision = crate::menu_syntax::capture_gate::decide_capture_gate_with_accepts(
        &validation_payload,
        Some(&schema),
        accepts,
    );
    let hud_message = gate_decision.hud_message().map(|s| s.to_string());
    let snapshot = match &result {
        ValidationResult::Ready => MenuSyntaxCaptureValidationSnapshot {
            target: schema.target.clone(),
            status: MenuSyntaxCaptureValidationStatus::Ready,
            can_submit: true,
            missing_field_labels: Vec::new(),
            malformed_field_label: None,
            malformed_reason: None,
            hud_message,
        },
        ValidationResult::Incomplete { missing } => {
            let labels: Vec<String> = missing.iter().map(|req| req.label()).collect();
            MenuSyntaxCaptureValidationSnapshot {
                target: schema.target.clone(),
                status: MenuSyntaxCaptureValidationStatus::Incomplete,
                can_submit: false,
                missing_field_labels: labels,
                malformed_field_label: None,
                malformed_reason: None,
                hud_message,
            }
        }
        ValidationResult::Malformed { field, reason } => MenuSyntaxCaptureValidationSnapshot {
            target: schema.target.clone(),
            status: MenuSyntaxCaptureValidationStatus::Malformed,
            can_submit: false,
            missing_field_labels: Vec::new(),
            malformed_field_label: Some(field.label()),
            malformed_reason: Some(reason.clone()),
            hud_message,
        },
    };
    Some(snapshot)
}

fn payload_for_capture_validation(
    payload: &CaptureInvocation,
    resolved: Option<&crate::menu_syntax::date::ResolvedCaptureInvocation>,
) -> CaptureInvocation {
    let Some(resolved) = resolved else {
        return payload.clone();
    };
    let mut validation_payload = payload.clone();
    validation_payload.body = resolved.body.clone();
    validation_payload.duration = resolved.duration.clone();
    validation_payload.date_phrases = resolved
        .dates
        .iter()
        .map(|date| DatePhrase {
            role: date.role.clone(),
            source: date.source.clone(),
            source_span: date.source_span,
        })
        .collect();
    validation_payload
}

fn capture_preview_rows(invocation: &CaptureInvocation) -> Vec<MenuSyntaxMainHintRow> {
    let mut rows = Vec::new();
    let body = invocation.body.trim();
    if !body.is_empty() {
        rows.push(hint_row("Body", body));
    }
    if !invocation.tags.is_empty() {
        rows.push(hint_row(
            "Tags",
            &invocation
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" "),
        ));
    }
    if let Some(priority) = invocation.priority {
        rows.push(hint_row("Priority", &format!("P{priority}")));
    }
    if let Some(url) = invocation.url.as_deref() {
        rows.push(hint_row("URL", url));
    }
    if let Some(duration) = invocation.duration.as_deref() {
        rows.push(hint_row("Duration", duration));
    }
    if !invocation.date_phrases.is_empty() {
        rows.push(hint_row(
            "Dates",
            &invocation
                .date_phrases
                .iter()
                .map(|date| format!("{}:{}", date_role_label(&date.role), date.source))
                .collect::<Vec<_>>()
                .join(" | "),
        ));
    }
    if !invocation.kv.is_empty() {
        rows.push(hint_row(
            "Fields",
            &invocation
                .kv
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(" | "),
        ));
    }
    rows.truncate(5);
    rows
}

fn command_preview_rows(invocation: &ArgvInvocation) -> Vec<MenuSyntaxMainHintRow> {
    let mut rows = Vec::new();
    if !invocation.fields.is_empty() {
        rows.push(hint_row(
            "Fields",
            &invocation
                .fields
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(" | "),
        ));
    }
    if !invocation.tags.is_empty() {
        rows.push(hint_row(
            "Tags",
            &invocation
                .tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" "),
        ));
    }
    if !invocation.argv.is_empty() {
        rows.push(hint_row("Argv", &invocation.argv.join(" ")));
    }
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandHintResolution {
    None,
    Unique { title: String, kind: &'static str },
    Ambiguous { count: usize },
}

fn resolve_command(
    invocation: &ArgvInvocation,
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
) -> CommandHintResolution {
    let mut matches = Vec::new();
    for script in scripts {
        let mut hit =
            super::command_head_matches(&invocation.head, &super::script_command_head(script));
        if !hit {
            // Run 13 Pass 5 — also accept author-declared `command.v1` heads
            // from the script's menuSyntax metadata. Without this, scripts
            // that declare `head: "deploy"` (e.g. via the !head pivot) get
            // a misleading `not found` chip even though the command schema
            // rows below are populated from the same metadata.
            if let Some(declared) = super::first_command_head_for_script(script) {
                if super::command_head_matches(&invocation.head, &declared) {
                    hit = true;
                }
            }
        }
        if hit {
            matches.push((script.name.clone(), "script"));
        }
    }
    for scriptlet in scriptlets {
        if super::command_head_matches(&invocation.head, &super::scriptlet_command_head(scriptlet))
        {
            matches.push((scriptlet.name.clone(), "scriptlet"));
        }
    }

    match matches.len() {
        0 => CommandHintResolution::None,
        1 => {
            let (title, kind) = matches.remove(0);
            CommandHintResolution::Unique { title, kind }
        }
        count => CommandHintResolution::Ambiguous { count },
    }
}

fn selected_picker_row<'a>(
    snapshot: &'a TriggerPickerSnapshot,
    selected_row_id: Option<&str>,
) -> Option<&'a TriggerPickerRow> {
    selected_row_id
        .and_then(|id| snapshot.rows.iter().find(|row| row.id == id))
        .or_else(|| snapshot.rows.iter().find(|row| row.enabled))
        .or_else(|| snapshot.rows.first())
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

/// Per-target example list shown in the hint card. Each entry MUST start with
/// `;<target>` (no cross-target leakage like a `;todo` example sneaking into a
/// `;cal` hint) and SHOULD include the target's required field slots so the
/// example doubles as a fix-it template the user can paste-and-edit. Falls back
/// to a generic `;todo` row for unknown targets so the hint is never empty.
/// Story: `hint-examples-target-relevant` (Run 12 user priority #3). Also
/// used by [[src/menu_syntax/capture_gate.rs#decide_capture_gate]] (Run 12
/// Pass 3) to build the HUD nudge's fix-it suggestion when Enter is blocked.
pub(crate) fn target_examples(target: &str) -> Vec<String> {
    let target = public_examples_target(target);
    match target {
        "cal" => vec![
            ";cal Design review start:\"friday 2pm\" #work".to_string(),
            ";cal Lunch with Anna at:\"tomorrow 12:30pm\"".to_string(),
            ";cal Sprint demo start:\"mon 10am\" end:\"mon 11am\"".to_string(),
        ],
        "todo" => vec![
            ";todo Buy milk #errands p2 due:tomorrow".to_string(),
            ";todo Send proposal #client/acme p1 due:friday".to_string(),
            ";todo Renew passport due:eom".to_string(),
        ],
        "note" => vec![
            ";note Decision to ship parser first #product".to_string(),
            ";note Q2 retrospective takeaways #team".to_string(),
            ";note Coffee chat with Sam — follow up on hiring".to_string(),
        ],
        "link" => vec![
            ";link https://zed.dev #rust title:\"GPUI notes\"".to_string(),
            ";link https://news.ycombinator.com #read-later".to_string(),
            ";link https://docs.rs/chrono #reference title:\"chrono docs\"".to_string(),
        ],
        "social" => vec![
            ";social Shipped the new launcher chrome — feedback welcome #build".to_string(),
            ";social Reading 'Designing Data-Intensive Applications' — solid #books".to_string(),
            ";social TIL: GPUI's flex_col gap saves a lot of margin churn #rust".to_string(),
        ],
        "mcal" => vec![
            ";mcal Lunch with Ryan tomorrow at 12pm til 1pm calendar=Work".to_string(),
            ";mcal Design review start:\"friday 2pm\" for 45m alarm=15".to_string(),
            ";mcal Team sync every mon from 1 til 2 calendar=Work".to_string(),
        ],
        "gcal" => vec![
            ";gcal Design review tomorrow 2pm for 45m calendarId=primary location=\"Zoom\""
                .to_string(),
            ";gcal Project kickoff start:\"friday 10am\" end:\"friday 11am\" guests=ada@example.com".to_string(),
            ";gcal Weekly planning every mon at 9am calendarId=primary location=\"Google Meet\"".to_string(),
        ],
        "github" => vec![
            ";github johnlindquist/kit Fix popup focus #bug p1".to_string(),
            ";github Review OAuth examples repo=johnlindquist/kit #demo".to_string(),
            ";github Track flaky CI url:https://github.com/johnlindquist/kit/actions #ci"
                .to_string(),
        ],
        "expense" => vec![
            ";expense Coffee amount=4.75 vendor=Bluebird #travel".to_string(),
            ";expense Client lunch amount=38.20 currency=USD vendor=\"Cafe Rio\" #client/acme"
                .to_string(),
            ";expense Taxi amount=22.00 reimbursable=true project=offsite #transport".to_string(),
        ],
        "snippet" => vec![
            ";snippet add parse_capture trigger:pc lang:rust -- fn parse_capture() {}".to_string(),
            ";snippet update @promise-timeout lang:ts -- const timeout = Promise.race([])"
                .to_string(),
            ";snippet rm @old-jq-filter".to_string(),
        ],
        "fixture" => vec![
            ";fixture Validate metadata filter env=dev project=launcher #demo".to_string(),
            ";fixture Parser smoke case kind=search tag=power-syntax".to_string(),
            ";fixture Snapshot row state=ready owner=qa #fixture".to_string(),
        ],
        other => vec![
            format!(";{other} Capture useful context #inbox"),
            format!(";{other} Follow up with team owner=me"),
            format!(";{other} Save this for later status=open"),
        ],
    }
}

fn public_examples_target(target: &str) -> &str {
    match target.to_ascii_lowercase().as_str() {
        "notes" => "note",
        "reminder" | "snooze" | "defer" => "todo",
        _ => target,
    }
}

fn capture_title(target: &str, row_title: Option<&str>) -> String {
    match row_title {
        Some(title) if !title.eq_ignore_ascii_case("Capture target") => title.to_string(),
        _ => format!("Capture {target}"),
    }
}

fn date_role_label(role: &DateRole) -> &'static str {
    match role {
        DateRole::Due => "due",
        DateRole::At => "at",
        DateRole::Start => "start",
        DateRole::End => "end",
        DateRole::Inferred => "date",
    }
}

#[cfg(test)]
fn predicate_label(predicate: &Predicate) -> String {
    match predicate {
        Predicate::Type(kind) => format!("type:{}", artifact_kind_label(kind)),
        Predicate::Tag(tag) => format!("tag:{tag}"),
        Predicate::HasShortcut(ShortcutPredicate::Any) => "shortcut:any".to_string(),
        Predicate::HasShortcut(ShortcutPredicate::None) => "shortcut:none".to_string(),
        Predicate::HasShortcut(ShortcutPredicate::Literal(shortcut)) => {
            format!("shortcut:{shortcut}")
        }
        Predicate::Source(source) => format!("source:{source}"),
        Predicate::Plugin(plugin) => format!("plugin:{plugin}"),
        Predicate::Name(name) => format!("name:{name}"),
        Predicate::Desc(desc) => format!("desc:{desc}"),
        Predicate::Alias(alias) => format!("alias:{alias}"),
        Predicate::Has(has) => format!("has:{has}"),
        Predicate::MetaPath { path, value } => format!("meta.{path}:{value}"),
        Predicate::Negate(inner) => format!("-{}", predicate_label(inner)),
    }
}

fn predicate_user_label(predicate: &Predicate) -> String {
    match predicate {
        Predicate::Type(kind) => match kind {
            ArtifactKind::Script => "scripts only".to_string(),
            ArtifactKind::Scriptlet => "scriptlets only".to_string(),
            ArtifactKind::Skill => "skills only".to_string(),
            ArtifactKind::Agent => "agents only".to_string(),
            ArtifactKind::Builtin => "built-ins only".to_string(),
            ArtifactKind::App => "apps only".to_string(),
            ArtifactKind::Window => "windows only".to_string(),
            ArtifactKind::File => "files only".to_string(),
            ArtifactKind::Note => "notes only".to_string(),
            ArtifactKind::Todo => "todos only".to_string(),
            ArtifactKind::AgentChatHistory => "Agent Chat conversations only".to_string(),
            ArtifactKind::AiVault => "AI Vault sessions only".to_string(),
            ArtifactKind::ClipboardHistory => "clipboard history only".to_string(),
            ArtifactKind::DictationHistory => "dictation history only".to_string(),
            ArtifactKind::BrowserTab => "browser tabs only".to_string(),
            ArtifactKind::BrowserHistory => "browser history only".to_string(),
            ArtifactKind::Fallback => "fallbacks only".to_string(),
            ArtifactKind::Issue => "issues only".to_string(),
        },
        Predicate::Tag(tag) => format!("#{tag}"),
        Predicate::HasShortcut(ShortcutPredicate::Any) => "has shortcut".to_string(),
        Predicate::HasShortcut(ShortcutPredicate::None) => "no shortcut".to_string(),
        Predicate::HasShortcut(ShortcutPredicate::Literal(shortcut)) => {
            format!("shortcut {shortcut}")
        }
        Predicate::Source(source) => format!("source {source}"),
        Predicate::Plugin(plugin) => format!("plugin {plugin}"),
        Predicate::Name(name) => format!("name contains {name}"),
        Predicate::Desc(desc) => format!("description contains {desc}"),
        Predicate::Alias(alias) => format!("alias contains {alias}"),
        Predicate::Has(has) => format!("has {has}"),
        Predicate::MetaPath { path, value } => format!("metadata {path} is {value}"),
        Predicate::Negate(inner) => match inner.as_ref() {
            Predicate::Type(ArtifactKind::App) => "exclude apps".to_string(),
            _ => format!("exclude {}", predicate_user_label(inner)),
        },
    }
}

fn predicate_tag(predicate: &Predicate) -> Option<&str> {
    match predicate {
        Predicate::Tag(tag) => Some(tag.as_str()),
        Predicate::Negate(inner) => predicate_tag(inner),
        _ => None,
    }
}

/// Display-only classification of the active grammar head the user is
/// typing. Drives head-aware copy and `examples` in the main hint and
/// surfaces as the camelCase `activeHead` / `activeHeadValuePartial`
/// receipts on [[MenuSyntaxMainHintSnapshot]]. Detection never mutates
/// parser predicates and never reclassifies input.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveHeadContext {
    head: String,
    value_partial: String,
    kind: ActiveHeadKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ActiveHeadKind {
    Has,
    Source,
    Capture,
    Command,
    TypeQualifier,
    TagQualifier,
    ShortcutQualifier,
    OtherQualifier,
}

impl ActiveHeadContext {
    fn value_partial_opt(&self) -> Option<String> {
        if self.value_partial.is_empty() {
            None
        } else {
            Some(self.value_partial.clone())
        }
    }
}

/// Detect the active head and value partial from the raw filter text.
/// Display-only; uses simple prefix matching, not the parser, so it
/// covers in-flight input like `c:zzz` even when no Predicate exists.
/// Public predicate: true when the raw filter text is mid-typing a known
/// grammar head (`has:`, `c:`, `:type:`, etc). Used by the getState path
/// to extend the empty-hint gate when the parser returns Incomplete.
pub fn has_active_head(raw: &str) -> bool {
    active_head_context_for_filter(raw).is_some()
}

/// True only for source-filter heads such as `c:`, `clipboard:`, `f:`,
/// and `files:`. Render uses this to avoid showing source no-match copy
/// before async/passive source results have been allowed to paint.
pub fn active_head_is_source_filter(raw: &str) -> bool {
    active_head_context_for_filter(raw)
        .is_some_and(|active| matches!(active.kind, ActiveHeadKind::Source))
}

pub fn active_filter_head_owns_main_list(raw: &str) -> bool {
    let Some(active) = active_head_context_for_filter(raw) else {
        return false;
    };
    let value = active.value_partial.trim();
    match active.kind {
        ActiveHeadKind::Source | ActiveHeadKind::Capture | ActiveHeadKind::Command => false,
        ActiveHeadKind::TypeQualifier => value.is_empty() || ArtifactKind::parse(value).is_none(),
        ActiveHeadKind::Has => {
            value.is_empty() || crate::menu_syntax::has_fields::lookup_has_field(value).is_none()
        }
        ActiveHeadKind::ShortcutQualifier
        | ActiveHeadKind::TagQualifier
        | ActiveHeadKind::OtherQualifier => value.is_empty(),
    }
}

fn active_head_context_for_filter(raw: &str) -> Option<ActiveHeadContext> {
    let trimmed = raw.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // ; capture head — capture composer takes over once a known target
    // is followed by a body, but `;daily` and `;unknownTarget` still
    // expose `;` as the active head for receipt purposes.
    if let Some(rest) = trimmed.strip_prefix(';') {
        let value_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let value = &rest[..value_end];
        return Some(ActiveHeadContext {
            head: ";".to_string(),
            value_partial: value.to_string(),
            kind: ActiveHeadKind::Capture,
        });
    }

    // ! command head
    if let Some(rest) = trimmed.strip_prefix('!') {
        let value_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let value = &rest[..value_end];
        return Some(ActiveHeadContext {
            head: "!".to_string(),
            value_partial: value.to_string(),
            kind: ActiveHeadKind::Command,
        });
    }

    // `:type:` / `:tag:` / `:shortcut:` qualifier heads. Only fires when
    // the input opens with a single colon prefix and a known qualifier
    // head is the active token. Falls through to bare-source heads
    // (`has:sh`, `c:zzz`) below.
    if let Some(after_colon) = trimmed.strip_prefix(':') {
        let active_token_end = after_colon
            .find(char::is_whitespace)
            .unwrap_or(after_colon.len());
        let token = &after_colon[..active_token_end];
        if let Some(value) = qualifier_value_partial(token, "type") {
            return Some(ActiveHeadContext {
                head: ":type:".to_string(),
                value_partial: value,
                kind: ActiveHeadKind::TypeQualifier,
            });
        }
        if let Some(value) = qualifier_value_partial(token, "kind") {
            return Some(ActiveHeadContext {
                head: ":kind:".to_string(),
                value_partial: value,
                kind: ActiveHeadKind::TypeQualifier,
            });
        }
        if let Some(value) = qualifier_value_partial(token, "tag") {
            return Some(ActiveHeadContext {
                head: ":tag:".to_string(),
                value_partial: value,
                kind: ActiveHeadKind::TagQualifier,
            });
        }
        if let Some(value) = qualifier_value_partial(token, "shortcut") {
            return Some(ActiveHeadContext {
                head: ":shortcut:".to_string(),
                value_partial: value,
                kind: ActiveHeadKind::ShortcutQualifier,
            });
        }
        if let Some(value) = qualifier_value_partial(token, "has") {
            return Some(ActiveHeadContext {
                head: "has:".to_string(),
                value_partial: value,
                kind: ActiveHeadKind::Has,
            });
        }
        for head in ["source", "plugin", "name", "desc", "description", "alias"] {
            if let Some(value) = qualifier_value_partial(token, head) {
                return Some(ActiveHeadContext {
                    head: format!(":{head}:"),
                    value_partial: value,
                    kind: ActiveHeadKind::OtherQualifier,
                });
            }
        }
        if let Some(ctx) = meta_active_head_context(token, true) {
            return Some(ctx);
        }
        // Source heads after a leading colon, e.g. `:c:zzz` — treat as
        // an OtherQualifier for receipts but do not over-specialize copy.
        let lower_token = token.to_ascii_lowercase();
        if let Some(spec) = source_head_spec_for_token(&extract_head_prefix(&lower_token)) {
            let head = spec.short.unwrap_or(spec.canonical);
            let value = token
                .split_once(':')
                .map(|(_, rest)| rest.to_string())
                .unwrap_or_default();
            return Some(ActiveHeadContext {
                head: head.to_string(),
                value_partial: value,
                kind: ActiveHeadKind::Source,
            });
        }
    }

    // Bare qualifier heads inserted by the `:` filter-head picker.
    let first_token_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let first_token = &trimmed[..first_token_end];
    for (head, kind) in [
        ("type", ActiveHeadKind::TypeQualifier),
        ("kind", ActiveHeadKind::TypeQualifier),
        ("tag", ActiveHeadKind::TagQualifier),
        ("shortcut", ActiveHeadKind::ShortcutQualifier),
        ("has", ActiveHeadKind::Has),
        ("source", ActiveHeadKind::OtherQualifier),
        ("plugin", ActiveHeadKind::OtherQualifier),
        ("name", ActiveHeadKind::OtherQualifier),
        ("desc", ActiveHeadKind::OtherQualifier),
        ("description", ActiveHeadKind::OtherQualifier),
        ("alias", ActiveHeadKind::OtherQualifier),
    ] {
        if let Some(value) = qualifier_value_partial(first_token, head) {
            return Some(ActiveHeadContext {
                head: format!("{head}:"),
                value_partial: value,
                kind,
            });
        }
    }
    if let Some(ctx) = meta_active_head_context(first_token, false) {
        return Some(ctx);
    }

    // Source heads (`c:`, `clipboard:`, etc.) used as the very first
    // token of the filter. The active partial is everything up to the
    // first whitespace.
    if let Some(colon_idx) = first_token.find(':') {
        let head_with_colon = &first_token[..=colon_idx];
        if let Some(spec) = source_head_spec_for_token(head_with_colon) {
            let display_head = spec
                .short
                .filter(|short| {
                    first_token
                        .to_ascii_lowercase()
                        .starts_with(&short.to_ascii_lowercase())
                })
                .unwrap_or(spec.canonical);
            let value = &first_token[colon_idx + 1..];
            return Some(ActiveHeadContext {
                head: display_head.to_string(),
                value_partial: value.to_string(),
                kind: ActiveHeadKind::Source,
            });
        }
    }

    None
}

fn meta_active_head_context(token: &str, leading_colon: bool) -> Option<ActiveHeadContext> {
    let lower = token.to_ascii_lowercase();
    if lower == "meta." || lower.starts_with("meta.") {
        let (head, value) = match token.split_once(':') {
            Some((head, value)) => (format!("{head}:"), value.to_string()),
            None => (token.to_string(), String::new()),
        };
        let head = if leading_colon {
            format!(":{head}")
        } else {
            head
        };
        return Some(ActiveHeadContext {
            head,
            value_partial: value,
            kind: ActiveHeadKind::OtherQualifier,
        });
    }
    None
}

/// Case-insensitive lookup against [[crate::menu_syntax::payload::SOURCE_HEAD_SPECS]].
fn source_head_spec_for_token(
    token: &str,
) -> Option<&'static crate::menu_syntax::payload::SourceHeadSpec> {
    let lower = token.to_ascii_lowercase();
    crate::menu_syntax::SOURCE_HEAD_SPECS.iter().find(|spec| {
        spec.canonical.eq_ignore_ascii_case(&lower)
            || spec
                .short
                .is_some_and(|short| short.eq_ignore_ascii_case(&lower))
    })
}

/// If `token` begins with `<head>:` (case-insensitively), return the
/// remainder after the trailing colon. Used by [[active_head_context_for_filter]].
fn qualifier_value_partial(token: &str, head: &str) -> Option<String> {
    let head_with_colon = format!("{head}:");
    // `get` (not direct slicing) so a multibyte token whose char boundary
    // straddles the head length can never panic — e.g. "ミーティング" sliced at
    // the byte length of "todo:" aborted the whole app before this guard.
    let prefix = token.get(..head_with_colon.len())?;
    if !prefix.eq_ignore_ascii_case(&head_with_colon) {
        return None;
    }
    Some(token[head_with_colon.len()..].to_string())
}

/// Truncate `token` at the first colon (inclusive) so we can hand the
/// committed head to `source_head_spec_for_token`.
fn extract_head_prefix(token: &str) -> String {
    if let Some(colon_idx) = token.find(':') {
        token[..=colon_idx].to_string()
    } else {
        token.to_string()
    }
}

/// Build at most `limit` `has:<field>` rows whose canonical or alias
/// matches `partial` (case-insensitively, prefix or substring). When
/// `partial` is empty, return the entire shipped catalog. Used by both
/// the main hint and the (existing) trigger picker completion path.
fn has_field_rows_for_partial(partial: &str) -> Vec<MenuSyntaxMainHintRow> {
    use crate::menu_syntax::has_fields::{HasFieldOwner, HAS_FIELD_SPECS};
    let probe = partial.trim().to_ascii_lowercase();
    HAS_FIELD_SPECS
        .iter()
        .filter(|spec| {
            if probe.is_empty() {
                return true;
            }
            spec.canonical.to_ascii_lowercase().contains(&probe)
                || spec
                    .aliases
                    .iter()
                    .any(|alias: &&str| alias.to_ascii_lowercase().contains(&probe))
        })
        .map(|spec| {
            let mut chips = Vec::new();
            for owner in spec.owners {
                let label = match owner {
                    HasFieldOwner::Script => "script",
                    HasFieldOwner::Scriptlet => "scriptlet",
                };
                chips.push(chip(label, MenuSyntaxMainHintTone::Neutral));
            }
            MenuSyntaxMainHintRow {
                label: spec.token.to_string(),
                value: spec.subtitle.unwrap_or(spec.label).to_string(),
                chips,
            }
        })
        .collect()
}

/// The catalog tokens to fall back on for `examples` when the partial
/// matches nothing in the catalog (e.g. `has:notAField`). Mirrors the
/// goal's "Try `has:shortcut`, `has:alias`, or `has:menuSyntax`." nudge.
fn has_unknown_example_tokens() -> Vec<String> {
    vec![
        "has:shortcut".to_string(),
        "has:alias".to_string(),
        "has:menuSyntax".to_string(),
    ]
}

/// Sentence-natural label for a source's zero-result copy, e.g.
/// "clipboard entries" instead of "Clipboard". Falls back to the
/// descriptor label lowercased when no override is registered.
fn source_zero_copy_label(spec: &crate::menu_syntax::payload::SourceHeadSpec) -> String {
    use crate::menu_syntax::payload::RootUnifiedSourceFilter;
    match spec.source {
        RootUnifiedSourceFilter::ClipboardHistory => "clipboard entries".to_string(),
        RootUnifiedSourceFilter::Conversations => "Agent Chat conversations".to_string(),
        RootUnifiedSourceFilter::Brain => "brain memories".to_string(),
        RootUnifiedSourceFilter::Notes => "notes".to_string(),
        RootUnifiedSourceFilter::Todo => "todos".to_string(),
        RootUnifiedSourceFilter::Files => "files".to_string(),
        RootUnifiedSourceFilter::BrowserTabs => "browser tabs".to_string(),
        RootUnifiedSourceFilter::BrowserHistory => "browser history entries".to_string(),
        RootUnifiedSourceFilter::Apps => "apps".to_string(),
        RootUnifiedSourceFilter::Scripts => "scripts".to_string(),
        RootUnifiedSourceFilter::Commands => "commands".to_string(),
        RootUnifiedSourceFilter::AiVault => "AI Vault sessions".to_string(),
        RootUnifiedSourceFilter::Dictation => "dictation entries".to_string(),
        RootUnifiedSourceFilter::Windows => "windows".to_string(),
        RootUnifiedSourceFilter::Processes => "processes".to_string(),
    }
}

fn artifact_kind_zero_copy_label(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Script => "scripts",
        ArtifactKind::Scriptlet => "scriptlets",
        ArtifactKind::Skill => "skills",
        ArtifactKind::Agent => "agents",
        ArtifactKind::Builtin => "built-ins",
        ArtifactKind::App => "apps",
        ArtifactKind::Window => "windows",
        ArtifactKind::File => "files",
        ArtifactKind::Note => "notes",
        ArtifactKind::Todo => "todos",
        ArtifactKind::AgentChatHistory => "Agent Chat conversations",
        ArtifactKind::AiVault => "AI Vault sessions",
        ArtifactKind::ClipboardHistory => "clipboard entries",
        ArtifactKind::DictationHistory => "dictation entries",
        ArtifactKind::BrowserTab => "browser tabs",
        ArtifactKind::BrowserHistory => "browser history entries",
        ArtifactKind::Fallback => "fallbacks",
        ArtifactKind::Issue => "issues",
    }
}

fn type_value_label(value: &str) -> &'static str {
    match value.to_ascii_lowercase().as_str() {
        "script" | "scripts" => "scripts",
        "scriptlet" | "scriptlets" => "scriptlets",
        "skill" | "skills" => "skills",
        "agent" | "agents" => "agents",
        "builtin" | "builtins" | "built-in" | "built-ins" => "built-ins",
        "app" | "apps" => "apps",
        "window" | "windows" => "windows",
        "file" | "files" => "files",
        "note" | "notes" => "notes",
        "clipboard" | "clipboard-history" | "clipboardhistory" => "clipboard entries",
        "dictation" | "dictation-history" | "dictationhistory" | "transcript" | "transcripts" => {
            "dictation entries"
        }
        "browser-tab" | "browser-tabs" | "browsertab" | "browsertabs" | "tab" | "tabs" => {
            "browser tabs"
        }
        "browser" | "browser-history" | "browserhistory" | "web" => "browser history entries",
        "agent_chathistory" | "agent_chat-history" | "ai-conversation" | "ai-conversations" => {
            "Agent Chat conversations"
        }
        "fallback" | "fallbacks" => "fallbacks",
        "issue" | "issues" | "scriptissue" | "script-issue" => "issues",
        _ => "matching items",
    }
}

fn type_predicate_kind(query: &AdvancedQuery) -> Option<&ArtifactKind> {
    query.predicates.iter().find_map(|p| match p {
        Predicate::Type(k) => Some(k),
        _ => None,
    })
}

fn shortcut_predicate(query: &AdvancedQuery) -> Option<&ShortcutPredicate> {
    query.predicates.iter().find_map(|p| match p {
        Predicate::HasShortcut(s) => Some(s),
        _ => None,
    })
}

fn has_field_predicate(query: &AdvancedQuery) -> Option<&str> {
    query.predicates.iter().find_map(|p| match p {
        Predicate::Has(field) => Some(field.as_str()),
        _ => None,
    })
}

#[cfg(test)]
fn artifact_kind_label(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Script => "script",
        ArtifactKind::Scriptlet => "scriptlet",
        ArtifactKind::Skill => "skill",
        ArtifactKind::Agent => "agent",
        ArtifactKind::Builtin => "builtin",
        ArtifactKind::App => "app",
        ArtifactKind::Window => "window",
        ArtifactKind::File => "file",
        ArtifactKind::Note => "note",
        ArtifactKind::Todo => "todo",
        ArtifactKind::AgentChatHistory => "agent_chat-history",
        ArtifactKind::ClipboardHistory => "clipboard-history",
        ArtifactKind::DictationHistory => "dictation-history",
        ArtifactKind::AiVault => "ai-vault",
        ArtifactKind::BrowserTab => "browser-tab",
        ArtifactKind::BrowserHistory => "browser-history",
        ArtifactKind::Fallback => "fallback",
        ArtifactKind::Issue => "issue",
    }
}

/// Render hint rows from an author-declared `command.v1` schema. One row
/// per arg (`label = arg.name`), one per flag (`label = flag.name`). Each
/// row carries a "required" chip when applicable and either the
/// description, allowed values, or example as the value text. The order
/// matters — the `setFilter ">deploy"` getState receipt looks for the
/// arg names ("env") and flag names ("--dry-run") in
/// `menuSyntaxMainHint.rows[].label`.
fn command_schema_rows(spec: &MenuSyntaxHandlerSpec) -> Vec<MenuSyntaxMainHintRow> {
    let mut rows = Vec::with_capacity(spec.args.len() + spec.flags.len());
    for arg in &spec.args {
        rows.push(arg_to_row(arg));
    }
    for flag in &spec.flags {
        rows.push(flag_to_row(flag));
    }
    rows
}

fn arg_to_row(arg: &CommandArgSpec) -> MenuSyntaxMainHintRow {
    let mut chips = Vec::new();
    if arg.required {
        chips.push(chip("required", MenuSyntaxMainHintTone::Warning));
    }
    let value = describe_arg_value(arg);
    MenuSyntaxMainHintRow {
        label: arg.name.clone(),
        value: truncate_hint_value(&value),
        chips,
    }
}

fn flag_to_row(flag: &CommandFlagSpec) -> MenuSyntaxMainHintRow {
    let mut chips = Vec::new();
    if flag.required {
        chips.push(chip("required", MenuSyntaxMainHintTone::Warning));
    }
    let value = describe_flag_value(flag);
    MenuSyntaxMainHintRow {
        label: flag.name.clone(),
        value: truncate_hint_value(&value),
        chips,
    }
}

fn describe_arg_value(arg: &CommandArgSpec) -> String {
    if !arg.values.is_empty() {
        return arg.values.join(" | ");
    }
    if let Some(desc) = arg.description.as_deref() {
        return desc.to_string();
    }
    if let Some(ex) = arg.example.as_deref() {
        return format!("e.g. {ex}");
    }
    String::new()
}

fn describe_flag_value(flag: &CommandFlagSpec) -> String {
    let mut parts = Vec::new();
    if let Some(alias) = flag.alias.as_deref() {
        parts.push(format!("alias {alias}"));
    }
    if !flag.values.is_empty() {
        parts.push(flag.values.join(" | "));
    }
    if let Some(desc) = flag.description.as_deref() {
        parts.push(desc.to_string());
    }
    if parts.is_empty() {
        if let Some(ex) = flag.example.as_deref() {
            parts.push(format!("e.g. {ex}"));
        }
    }
    parts.join(" — ")
}

fn hint_row(label: &str, value: &str) -> MenuSyntaxMainHintRow {
    MenuSyntaxMainHintRow {
        label: label.to_string(),
        value: truncate_hint_value(value),
        chips: Vec::new(),
    }
}

fn chip(label: &str, tone: MenuSyntaxMainHintTone) -> MenuSyntaxMainHintChip {
    MenuSyntaxMainHintChip {
        label: label.to_string(),
        tone,
    }
}

fn truncate_hint_value(value: &str) -> String {
    const MAX_CHARS: usize = 80;
    if value.chars().count() <= MAX_CHARS {
        return value.to_string();
    }
    let mut out: String = value.chars().take(MAX_CHARS.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn finalize_hint(mut hint: MenuSyntaxMainHintSnapshot) -> MenuSyntaxMainHintSnapshot {
    hint.raw_filter_text = truncate_raw_filter_text(&hint.raw_filter_text);
    hint.accessibility_label = accessibility_label(&hint);
    hint
}

fn truncate_raw_filter_text(value: &str) -> String {
    const MAX_CHARS: usize = 200;
    if value.chars().count() <= MAX_CHARS {
        return value.to_string();
    }
    let mut out: String = value.chars().take(MAX_CHARS.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn raw_last_token_is_priority_prefix(raw: &str) -> bool {
    matches!(raw.split_whitespace().last(), Some(t) if t.eq_ignore_ascii_case("p"))
}

fn accessibility_label(hint: &MenuSyntaxMainHintSnapshot) -> String {
    let mut parts = vec![hint.title.clone()];
    if let Some(subtitle) = hint.subtitle.as_ref() {
        parts.push(subtitle.clone());
    }
    if let Some(primary) = hint.primary_hint.as_ref() {
        parts.push(primary.clone());
    }
    for row in &hint.rows {
        parts.push(format!("{} {}", row.label, row.value));
    }
    if let Some(warning) = hint.warning.as_ref() {
        parts.push(format!("Warning: {warning}"));
    }
    parts.join(". ")
}

#[cfg(test)]
include!("main_hint_tests.rs");

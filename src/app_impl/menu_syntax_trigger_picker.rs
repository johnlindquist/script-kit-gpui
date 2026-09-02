//! Menu-syntax trigger picker — pure state machine and row adapter.
//!
//! This is commit D1 of the Oracle iter 015 picker-pivot plan. It lands the
//! owner-neutral state machine + adapter that `src/app_impl/filter_input_change.rs`
//! will consume in commit D2 along with the GPUI window entity, keyboard
//! dispatch, and removal of the `build_trigger_picker_grouped_results`
//! SectionHeader takeover.
//!
//! Shape of this module:
//!
//! - [`MenuSyntaxTriggerPickerState`] is what the owner (launcher) stores
//!   between filter updates: the current snapshot and the selected row id.
//!   Selection persistence is by **row id**, not index, so snapshot rebuilds
//!   from filtering do not snap the cursor back to the top when the same row
//!   is still visible.
//! - [`plan_trigger_picker_transition`] is the pure state-machine function.
//!   Given the current state, a raw filter string, and a
//!   [`TriggerPickerContext`], it returns a [`TriggerPickerTransition`]
//!   describing what the owner should do:
//!   - [`TriggerPickerTransition::Close`] when the filter routes to a legacy
//!     handoff trigger (`~ / @ > ?`) or `build_trigger_picker_snapshot`
//!     returns `None`.
//!   - [`TriggerPickerTransition::Open`] when a new snapshot appears and the
//!     picker was previously closed.
//!   - [`TriggerPickerTransition::Update`] when a new snapshot appears and
//!     the picker was already open; this variant carries the selected row id
//!     chosen to preserve the user's cursor when possible.
//!   - [`TriggerPickerTransition::NoChange`] when the new snapshot equals the
//!     current one — owners can skip GPUI work in that case.
//! - [`adapt_trigger_picker_row`] converts a `TriggerPickerRow` (menu-syntax
//!   domain type, `String` fields) into the neutral
//!   [`InlinePickerRow`] shape the shared renderer consumes. Owners call this
//!   adapter from their own module so the shared components never import
//!   menu-syntax types.
//! - [`starts_with_legacy_trigger`] classifies a filter as routing to an
//!   existing surface handoff (`~` command, `/` scripts, `@` ai mentions,
//!   `>` shortcuts, `?` help). Legacy triggers MUST close the picker before
//!   menu-syntax claims lifecycle, per Oracle iter 015.
//!
//! This module has no GPUI imports and no side effects. Every behavior is
//! table-tested.

use crate::components::inline_picker::{
    InlinePickerBadge, InlinePickerBadgeTone, InlinePickerHighlights, InlinePickerRow,
    InlinePickerRowKind,
};
use crate::menu_syntax::{
    build_trigger_picker_snapshot, TriggerPickerAction, TriggerPickerContext, TriggerPickerMode,
    TriggerPickerRow, TriggerPickerRowKind, TriggerPickerSnapshot,
};
use gpui::SharedString;

const TRIGGER_PICKER_MAX_VISIBLE_ROWS: usize = 8;

/// Currently-cached state for the menu-syntax trigger picker. Held by the
/// launcher (`ScriptListApp`) between filter updates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MenuSyntaxTriggerPickerState {
    /// None means the picker is closed.
    pub(crate) snapshot: Option<TriggerPickerSnapshot>,
    /// Row id currently highlighted. None when the picker is closed or no
    /// enabled row exists in the current snapshot.
    pub(crate) selected_row_id: Option<String>,
    /// Start index of the visible page. Preserved across selection updates so
    /// shared inline-dropdown range math can keep the page stable.
    pub(crate) visible_start: usize,
}

impl MenuSyntaxTriggerPickerState {
    /// Picker snapshots own the ScriptList main list while the user is choosing
    /// a syntax head or value. Terminal structured queries stop producing a
    /// snapshot, so normal launcher results take over again.
    #[allow(dead_code)] // Lib-crate copy has no consumer; binary crate uses this in filtering/render.
    pub(crate) fn owns_main_list(&self) -> bool {
        matches!(
            self.snapshot.as_ref().map(|snapshot| snapshot.mode),
            Some(
                TriggerPickerMode::AdvancedQuery
                    | TriggerPickerMode::Capture
                    | TriggerPickerMode::Command
            )
        )
    }
}

/// Transition the owner should apply to its GPUI layer after
/// [`plan_trigger_picker_transition`] runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TriggerPickerTransition {
    /// Current snapshot stays valid; no GPUI work needed.
    NoChange,
    /// Close the picker window if it is open; clear `MenuSyntaxTriggerPickerState`.
    Close,
    /// Open a new picker window with this snapshot and selection.
    Open {
        snapshot: TriggerPickerSnapshot,
        selected_row_id: Option<String>,
    },
    /// Reuse the existing picker window but swap in this snapshot. Selection
    /// is carried across by row id when that row still exists and is enabled;
    /// otherwise the first enabled row wins.
    Update {
        snapshot: TriggerPickerSnapshot,
        selected_row_id: Option<String>,
    },
}

/// Character-index highlights for the compact row renderer.
///
/// This mirrors Agent Chat's `/` and `@` picker contract: query matches are
/// expressed as visible character positions, and trigger prefixes like `;`
/// are not highlighted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct TriggerPickerRowHighlightIndices {
    pub(crate) title: Vec<usize>,
    pub(crate) meta: Vec<usize>,
}

/// Classify whether `raw` begins with a legacy launcher handoff trigger.
///
/// Legacy triggers own their own surfaces (`~` command, `/` scripts, `@`
/// mentions, `>` shortcuts, `?` help) and must not be claimed by the
/// menu-syntax picker. Oracle iter 015 pins this check ahead of
/// `build_trigger_picker_snapshot` so the picker closes immediately when the
/// user types one.
pub(crate) fn starts_with_legacy_trigger(raw: &str) -> bool {
    matches!(raw.chars().next(), Some('~' | '/' | '@' | '>' | '?'))
}

/// Compute the next [`TriggerPickerTransition`] given the current picker state,
/// the raw filter text, and the launcher's trigger-picker context.
///
/// Pure function: no I/O, no window updates, no global state. Owners call
/// this from their filter-change handler and then apply the returned
/// transition to their GPUI layer.
//
// `#[allow(dead_code)]` is kept because this module is compiled twice — once
// in the binary (via `include!("app_impl/mod.rs")` in `src/main.rs`) where it
// IS consumed by `src/app_impl/filter_input_change.rs`, and once in the lib
// (via `#[path] pub mod menu_syntax_trigger_picker;` in `src/lib.rs`) solely so
// the state-machine tests can run under `cargo test --lib`. The lib copy has
// no consumer, so without this allow the lib build warns.
#[allow(dead_code)]
pub(crate) fn plan_trigger_picker_transition(
    current: &MenuSyntaxTriggerPickerState,
    raw_filter: &str,
    ctx: &TriggerPickerContext,
) -> TriggerPickerTransition {
    if starts_with_legacy_trigger(raw_filter) {
        return if current.snapshot.is_some() {
            TriggerPickerTransition::Close
        } else {
            TriggerPickerTransition::NoChange
        };
    }

    let next_snapshot = build_trigger_picker_snapshot(raw_filter, ctx)
        .or_else(|| partial_trigger_snapshot(raw_filter, ctx));

    match (&current.snapshot, next_snapshot) {
        (None, None) => TriggerPickerTransition::NoChange,
        (Some(_), None) => TriggerPickerTransition::Close,
        (None, Some(snapshot)) => {
            let selected = preserve_or_pick_first_enabled(&snapshot, None);
            TriggerPickerTransition::Open {
                snapshot,
                selected_row_id: selected,
            }
        }
        (Some(prev), Some(snapshot)) => {
            if prev == &snapshot
                && preserve_or_pick_first_enabled(&snapshot, current.selected_row_id.as_deref())
                    == current.selected_row_id
            {
                return TriggerPickerTransition::NoChange;
            }
            let selected =
                preserve_or_pick_first_enabled(&snapshot, current.selected_row_id.as_deref());
            TriggerPickerTransition::Update {
                snapshot,
                selected_row_id: selected,
            }
        }
    }
}

/// Fallback snapshot when the raw parser rejects `;<partial>` or `:<partial>`.
/// Without this, typing the first character after a trigger
/// would close the picker (because the parser only claims
/// known capture targets and whitespace-qualified queries), which makes
/// the picker unusable for power users who don't remember the full
/// target / qualifier name.
///
/// Policy:
/// - Only kicks in for `;`, legacy `+`, and `:` heads (legacy triggers are handled
///   earlier in `plan_trigger_picker_transition`).
/// - Only kicks in when the text AFTER the trigger is non-empty and
///   contains no whitespace. As soon as the user types a space, it's
///   body text, not a partial target / qualifier — the parser takes
///   over.
/// - Builds the bare snapshot (`;` or `:`) and retains rows whose
///   `title` or `token` substring-matches the partial text
///   case-insensitively. Footer action rows are always kept so the
///   "Create capture handler…" / "Open help" affordances remain
///   reachable.
/// - If every selectable row filters out, returns `None` — nothing to
///   show, so the picker closes (a cleaner UX than showing only the
///   footer).
#[allow(dead_code)] // Lib-crate copy has no consumer; see plan_trigger_picker_transition comment.
fn partial_trigger_snapshot(
    raw_filter: &str,
    ctx: &TriggerPickerContext,
) -> Option<TriggerPickerSnapshot> {
    let trigger_char = raw_filter.chars().next()?;
    let bare = match trigger_char {
        ';' => ";",
        '+' => "+",
        ':' => ":",
        _ => return None,
    };
    let after_trigger = &raw_filter[trigger_char.len_utf8()..];
    if after_trigger.is_empty() || after_trigger.contains(char::is_whitespace) {
        return None;
    }

    let mut snapshot = build_trigger_picker_snapshot(bare, ctx)?;
    let canonical_token_trigger = canonical_capture_trigger(trigger_char);
    let needle = after_trigger.to_lowercase();
    snapshot.rows.retain(|row| {
        if matches!(
            row.kind,
            crate::menu_syntax::TriggerPickerRowKind::FooterAction
        ) {
            return true;
        }
        // Prefix match against the token slug (e.g. ";todo" -> "todo").
        // Loose `contains` was too permissive: typing `+t` matched
        // "Daily note" ('t' inside "note"), "Social draft" ('t' inside
        // "draft"), "Tagged link" ('t' in "tagged"). Users typing the
        // start of a target name expect slug-prefix semantics.
        let token_prefix_matches = row
            .token
            .as_deref()
            .and_then(|t| t.strip_prefix(canonical_token_trigger).or(Some(t)))
            .map(|slug| slug.to_lowercase().starts_with(&needle))
            .unwrap_or(false);
        let colon_title_prefix_matches =
            trigger_char == ':' && row.title.to_lowercase().starts_with(&needle);
        token_prefix_matches || colon_title_prefix_matches
    });

    let all_footers = snapshot.rows.iter().all(|row| {
        matches!(
            row.kind,
            crate::menu_syntax::TriggerPickerRowKind::FooterAction
        )
    });
    if all_footers {
        return None;
    }
    Some(snapshot)
}

/// Whether a trigger-picker row may become the keyboard/default selection.
///
/// Footer rows stay explicit and clickable, but should not be highlighted as
/// the default keyboard target.
#[allow(dead_code)] // Lib-crate copy has no consumer; see plan_trigger_picker_transition comment.
pub(crate) fn trigger_picker_row_is_default_selectable(row: &TriggerPickerRow) -> bool {
    row.enabled
        && (row.kind != TriggerPickerRowKind::FooterAction
            || matches!(row.action, TriggerPickerAction::CreateHandler { .. }))
}

/// Try to keep `previous_id` selected if the row still exists in
/// `snapshot` and is keyboard-selectable. Otherwise fall back to the first
/// selectable row. Returns `None` when the snapshot has no selectable rows.
#[allow(dead_code)] // Lib-crate copy has no consumer; see plan_trigger_picker_transition comment.
fn preserve_or_pick_first_enabled(
    snapshot: &TriggerPickerSnapshot,
    previous_id: Option<&str>,
) -> Option<String> {
    if let Some(prev) = previous_id {
        if let Some(row) = snapshot
            .rows
            .iter()
            .find(|row| row.id == prev && trigger_picker_row_is_default_selectable(row))
        {
            return Some(row.id.clone());
        }
    }
    snapshot
        .rows
        .iter()
        .find(|row| trigger_picker_row_is_default_selectable(row))
        .map(|row| row.id.clone())
}

#[allow(dead_code)]
pub(crate) fn trigger_picker_visible_start_for_selection(
    visible_start: usize,
    selected_index: usize,
    item_count: usize,
) -> usize {
    crate::components::inline_dropdown::inline_dropdown_visible_range_from_start(
        visible_start,
        selected_index,
        item_count,
        TRIGGER_PICKER_MAX_VISIBLE_ROWS,
    )
    .start
}

/// Convert a [`TriggerPickerRow`] into the neutral [`InlinePickerRow`] shape
/// consumed by the shared renderer. Menu-syntax rows all carry the
/// [`InlinePickerRowKind::TextTrigger`] kind except for the "Open Menu
/// Syntax help" / "Create capture handler…" footer rows which map to
/// [`InlinePickerRowKind::Action`].
#[allow(dead_code)]
pub(crate) fn adapt_trigger_picker_row(row: &TriggerPickerRow) -> InlinePickerRow {
    let kind = match row.kind {
        TriggerPickerRowKind::FooterAction => InlinePickerRowKind::Action,
        _ => InlinePickerRowKind::TextTrigger,
    };

    let badges = row
        .badges
        .iter()
        .map(|label| InlinePickerBadge {
            label: SharedString::from(label.clone()),
            tone: InlinePickerBadgeTone::Neutral,
        })
        .collect();

    InlinePickerRow {
        id: SharedString::from(row.id.clone()),
        kind,
        title: SharedString::from(row.title.clone()),
        token: row.token.as_ref().map(|s| SharedString::from(s.clone())),
        subtitle: row.subtitle.as_ref().map(|s| SharedString::from(s.clone())),
        detail: row.detail.as_ref().map(|s| SharedString::from(s.clone())),
        example: row.example.as_ref().map(|s| SharedString::from(s.clone())),
        leading: None,
        badges,
        accessory: None,
        highlights: InlinePickerHighlights::default(),
        enabled: row.enabled,
        disabled_reason: None,
    }
}

#[allow(dead_code)]
pub(crate) fn trigger_picker_row_to_main_list_row(
    row: &TriggerPickerRow,
) -> crate::spine::SpineListRow {
    let subtitle = row
        .subtitle
        .as_ref()
        .or(row.detail.as_ref())
        .map(|text| SharedString::from(text.clone()));
    let meta = row
        .token
        .as_ref()
        .map(|token| SharedString::from(token.clone()));
    let action_label = match row.action {
        TriggerPickerAction::InsertToken { .. } => Some("Insert"),
        TriggerPickerAction::ReplaceInput { .. } => Some("Use"),
        TriggerPickerAction::FixQualifier { .. } => Some("Fix"),
        TriggerPickerAction::ExecuteCaptureHandler { .. } => Some("Run"),
        TriggerPickerAction::OpenCaptures { .. } => Some("Open"),
        TriggerPickerAction::CreateHandler { .. } => Some("Create"),
        TriggerPickerAction::OpenHelp => Some("Help"),
    };

    crate::spine::SpineListRow {
        id: SharedString::from(format!("menu-syntax-trigger:{}", row.id)),
        kind: crate::spine::list::SpineListRowKind::CaptureTarget {
            target: SharedString::from(row.id.clone()),
        },
        title: SharedString::from(row.title.clone()),
        subtitle,
        meta,
        icon: Some(SharedString::from(match row.mode {
            TriggerPickerMode::Capture => "inbox",
            TriggerPickerMode::Command => "terminal",
            TriggerPickerMode::AdvancedQuery => "search",
        })),
        badges: row
            .badges
            .iter()
            .map(|badge| SharedString::from(badge.clone()))
            .collect(),
        score: if row.enabled { 0 } else { i32::MIN },
        is_selectable: trigger_picker_row_is_default_selectable(row),
        action_label: action_label.map(SharedString::from),
        action: crate::spine::SpineListAction::AcceptMenuSyntaxTrigger {
            row_id: row.id.clone().into(),
        },
    }
}

/// Compute compact-row highlights for the menu-syntax picker from the current
/// raw filter text.
///
/// The renderer shows the row title on the left and either `token` or
/// `subtitle` on the right. This helper uses the same ordered-character match
/// behavior as the Agent Chat `/` and `@` pickers, offsetting `;` / `:` token matches
/// past the trigger prefix so the typed trigger itself stays neutral.
#[allow(dead_code)]
pub(crate) fn trigger_picker_row_highlight_indices(
    row: &TriggerPickerRow,
    raw_filter: &str,
) -> TriggerPickerRowHighlightIndices {
    if matches!(row.kind, TriggerPickerRowKind::FooterAction) {
        return TriggerPickerRowHighlightIndices::default();
    }

    let Some((trigger, query)) = trigger_picker_highlight_query(raw_filter) else {
        return TriggerPickerRowHighlightIndices::default();
    };

    let title = row.title.as_str();
    let meta = row
        .token
        .as_deref()
        .or(row.subtitle.as_deref())
        .unwrap_or("");

    let title_hits = best_trigger_picker_hits(&query, title, trigger, false);
    let meta_hits = best_trigger_picker_hits(&query, meta, trigger, true);

    TriggerPickerRowHighlightIndices {
        title: title_hits,
        meta: meta_hits,
    }
}

fn trigger_picker_highlight_query(raw_filter: &str) -> Option<(char, String)> {
    let trimmed = raw_filter.trim_start();
    let trigger = trimmed.chars().next()?;
    if trigger != ';' && trigger != '+' && trigger != ':' {
        return None;
    }

    let tail = &trimmed[trigger.len_utf8()..];
    let query = tail.split_whitespace().next().unwrap_or("").trim();
    if query.is_empty() {
        return None;
    }

    Some((trigger, query.to_ascii_lowercase()))
}

fn best_trigger_picker_hits(
    query: &str,
    candidate: &str,
    trigger: char,
    skip_trigger_prefix: bool,
) -> Vec<usize> {
    let mut query_candidates = vec![query.to_string()];

    if trigger == ':' {
        if let Some(head) = colon_qualifier_head(query) {
            if !query_candidates.iter().any(|q| q == head) {
                query_candidates.push(head.to_string());
            }
        }
    }

    let canonical_token_trigger = canonical_capture_trigger(trigger);
    for query in query_candidates {
        let hits = if skip_trigger_prefix {
            match_query_chars_after_trigger_prefix(&query, candidate, canonical_token_trigger)
        } else {
            match_query_chars(&query, candidate)
        };
        if let Some(mut hits) = hits {
            if skip_trigger_prefix
                && candidate.starts_with(canonical_token_trigger)
                && !hits.is_empty()
            {
                hits.insert(0, 0);
            }
            return hits;
        }
    }

    Vec::new()
}

fn canonical_capture_trigger(trigger: char) -> char {
    if trigger == '+' {
        ';'
    } else {
        trigger
    }
}

fn colon_qualifier_head(query: &str) -> Option<&str> {
    let colon_idx = query.find(':')?;
    Some(&query[..=colon_idx])
}

fn match_query_chars_after_trigger_prefix(
    query: &str,
    candidate: &str,
    trigger: char,
) -> Option<Vec<usize>> {
    let prefix_len = candidate.chars().take_while(|ch| *ch == trigger).count();
    let bare = candidate
        .char_indices()
        .nth(prefix_len)
        .map(|(idx, _)| &candidate[idx..])
        .unwrap_or("");
    let hits = match_query_chars(query, bare)?;
    Some(hits.into_iter().map(|ix| ix + prefix_len).collect())
}

fn match_query_chars(query: &str, candidate: &str) -> Option<Vec<usize>> {
    if query.is_empty() {
        return Some(Vec::new());
    }

    let candidate_chars: Vec<char> = candidate.chars().collect();
    let mut hits = Vec::with_capacity(query.chars().count());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_syntax::{TriggerPickerAction, TriggerPickerMode};

    fn qualifier_row(id: &str, enabled: bool) -> TriggerPickerRow {
        TriggerPickerRow {
            id: id.to_string(),
            mode: TriggerPickerMode::AdvancedQuery,
            kind: TriggerPickerRowKind::Qualifier,
            title: format!("title-{id}"),
            token: Some(format!("{id}:")),
            subtitle: Some("Qualifier".to_string()),
            detail: None,
            example: Some(format!("{id}:value")),
            badges: Vec::new(),
            action: TriggerPickerAction::InsertToken {
                token: format!("{id}:"),
                keep_open: true,
            },
            enabled,
        }
    }

    fn snapshot(rows: Vec<TriggerPickerRow>) -> TriggerPickerSnapshot {
        TriggerPickerSnapshot {
            mode: TriggerPickerMode::AdvancedQuery,
            target: None,
            rows,
        }
    }

    fn ctx() -> TriggerPickerContext {
        TriggerPickerContext::default()
    }

    #[test]
    fn legacy_trigger_closes_open_popup() {
        let state = MenuSyntaxTriggerPickerState {
            snapshot: Some(snapshot(vec![qualifier_row("type", true)])),
            selected_row_id: Some("type".to_string()),
            visible_start: 0,
        };
        for trigger in ["~home", "/scripts", "@file", ">shortcut", "?help"] {
            let transition = plan_trigger_picker_transition(&state, trigger, &ctx());
            assert_eq!(
                transition,
                TriggerPickerTransition::Close,
                "trigger `{}` should close the picker",
                trigger
            );
        }
    }

    #[test]
    fn legacy_trigger_on_closed_popup_is_no_change() {
        let state = MenuSyntaxTriggerPickerState::default();
        assert_eq!(
            plan_trigger_picker_transition(&state, "/scripts", &ctx()),
            TriggerPickerTransition::NoChange,
        );
    }

    #[test]
    fn non_menu_syntax_filter_stays_closed() {
        let state = MenuSyntaxTriggerPickerState::default();
        assert_eq!(
            plan_trigger_picker_transition(&state, "plain search text", &ctx()),
            TriggerPickerTransition::NoChange,
        );
    }

    #[test]
    fn non_menu_syntax_filter_closes_open_popup() {
        let prev = snapshot(vec![qualifier_row("type", true)]);
        let state = MenuSyntaxTriggerPickerState {
            snapshot: Some(prev),
            selected_row_id: Some("type".to_string()),
            visible_start: 0,
        };
        assert_eq!(
            plan_trigger_picker_transition(&state, "plain text", &ctx()),
            TriggerPickerTransition::Close,
        );
    }

    #[test]
    fn colon_prefix_opens_popup_when_closed() {
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, ":", &ctx());
        match transition {
            TriggerPickerTransition::Open {
                snapshot,
                selected_row_id,
            } => {
                assert!(!snapshot.rows.is_empty(), ":` should yield qualifier rows");
                assert!(selected_row_id.is_some(), "should pre-select first enabled");
            }
            other => panic!("expected Open, got {:?}", other),
        }
    }

    #[test]
    fn exact_type_value_keeps_open_trigger_picker() {
        let context = ctx();
        let state = MenuSyntaxTriggerPickerState {
            snapshot: build_trigger_picker_snapshot("type:", &context),
            selected_row_id: Some("qualifier:type:script".to_string()),
            visible_start: 0,
        };

        for input in ["type:script", ":type:script"] {
            match plan_trigger_picker_transition(&state, input, &context) {
                TriggerPickerTransition::Open { snapshot, .. }
                | TriggerPickerTransition::Update { snapshot, .. } => {
                    let tokens: Vec<&str> = snapshot
                        .rows
                        .iter()
                        .filter_map(|row| row.token.as_deref())
                        .collect();
                    assert!(tokens.contains(&"type:script"));
                    assert!(tokens.contains(&"type:scriptlet"));
                }
                other => panic!("expected picker rows for {input:?}, got {other:?}"),
            }
        }

        match plan_trigger_picker_transition(
            &MenuSyntaxTriggerPickerState::default(),
            "type:script",
            &context,
        ) {
            TriggerPickerTransition::Open { snapshot, .. } => {
                let tokens: Vec<&str> = snapshot
                    .rows
                    .iter()
                    .filter_map(|row| row.token.as_deref())
                    .collect();
                assert!(tokens.contains(&"type:script"));
                assert!(tokens.contains(&"type:scriptlet"));
            }
            other => panic!("expected closed picker to open for exact type value, got {other:?}"),
        }
    }

    #[test]
    fn type_value_boundary_closes_open_trigger_picker() {
        let context = ctx();
        let state = MenuSyntaxTriggerPickerState {
            snapshot: build_trigger_picker_snapshot("type:", &context),
            selected_row_id: Some("qualifier:type:script".to_string()),
            visible_start: 0,
        };

        for input in ["type:script ", ":type:script "] {
            assert_eq!(
                plan_trigger_picker_transition(&state, input, &context),
                TriggerPickerTransition::Close,
                "{input:?} should commit the type filter after a boundary"
            );
        }
    }

    #[test]
    fn source_filter_query_does_not_open_power_popup() {
        let state = MenuSyntaxTriggerPickerState::default();
        assert_eq!(
            plan_trigger_picker_transition(&state, "png :f", &ctx()),
            TriggerPickerTransition::NoChange,
            "inline file source filters should refine search without showing the power-user picker"
        );
        assert_eq!(
            plan_trigger_picker_transition(&state, ":n meeting", &ctx()),
            TriggerPickerTransition::NoChange,
            "prefix source filters should still behave as normal search refinements"
        );
    }

    #[test]
    fn plus_prefix_opens_popup_when_closed() {
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, "+", &ctx());
        match transition {
            TriggerPickerTransition::Open { snapshot, .. } => {
                assert!(snapshot
                    .rows
                    .iter()
                    .any(|row| row.kind == TriggerPickerRowKind::CaptureTarget));
            }
            other => panic!("expected Open for `+`, got {:?}", other),
        }
    }

    #[test]
    fn semicolon_prefix_opens_canonical_capture_popup_when_closed() {
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, ";", &ctx());
        match transition {
            TriggerPickerTransition::Open { snapshot, .. } => {
                assert!(snapshot
                    .rows
                    .iter()
                    .any(|row| row.kind == TriggerPickerRowKind::CaptureTarget));
            }
            other => panic!("expected Open for `;`, got {:?}", other),
        }
    }

    #[test]
    fn capture_body_composer_closes_open_popup() {
        let prev = build_trigger_picker_snapshot(";todo", &ctx()).expect("todo target snapshot");
        let state = MenuSyntaxTriggerPickerState {
            snapshot: Some(prev),
            selected_row_id: Some("target:todo".to_string()),
            visible_start: 0,
        };

        assert_eq!(
            plan_trigger_picker_transition(&state, ";todo Take out the garbage", &ctx()),
            TriggerPickerTransition::Close,
            "body composition owns input after the capture target boundary"
        );
        assert_eq!(
            plan_trigger_picker_transition(
                &MenuSyntaxTriggerPickerState::default(),
                ";todo ",
                &ctx()
            ),
            TriggerPickerTransition::NoChange,
            "composer mode starts with a blank main surface, not a picker"
        );
    }

    #[test]
    fn preserve_selection_when_row_still_present() {
        let prev = snapshot(vec![
            qualifier_row("type", true),
            qualifier_row("shortcut", true),
        ]);
        let state = MenuSyntaxTriggerPickerState {
            snapshot: Some(prev),
            selected_row_id: Some("shortcut".to_string()),
            visible_start: 0,
        };
        let next = snapshot(vec![
            qualifier_row("shortcut", true),
            qualifier_row("source", true),
        ]);
        let selected = preserve_or_pick_first_enabled(&next, Some("shortcut"));
        assert_eq!(selected, Some("shortcut".to_string()));
        let _ = state;
    }

    #[test]
    fn preserve_falls_back_to_first_enabled_when_previous_gone() {
        let next = snapshot(vec![
            qualifier_row("type", true),
            qualifier_row("source", true),
        ]);
        let selected = preserve_or_pick_first_enabled(&next, Some("shortcut"));
        assert_eq!(selected, Some("type".to_string()));
    }

    #[test]
    fn preserve_skips_disabled_previous_row() {
        let next = snapshot(vec![
            qualifier_row("type", false),
            qualifier_row("source", true),
        ]);
        let selected = preserve_or_pick_first_enabled(&next, Some("type"));
        assert_eq!(selected, Some("source".to_string()));
    }

    #[test]
    fn preserve_returns_none_when_no_enabled_rows() {
        let next = snapshot(vec![
            qualifier_row("type", false),
            qualifier_row("source", false),
        ]);
        let selected = preserve_or_pick_first_enabled(&next, Some("type"));
        assert_eq!(selected, None);
    }

    #[test]
    fn preserve_skips_footer_actions_for_default_selection() {
        let footer = TriggerPickerRow {
            id: "footer:test".to_string(),
            mode: TriggerPickerMode::AdvancedQuery,
            kind: TriggerPickerRowKind::FooterAction,
            title: "Footer action".to_string(),
            token: None,
            subtitle: None,
            detail: None,
            example: None,
            badges: Vec::new(),
            action: TriggerPickerAction::OpenHelp,
            enabled: true,
        };
        let next = snapshot(vec![footer.clone(), qualifier_row("type", true)]);

        assert_eq!(
            preserve_or_pick_first_enabled(&next, Some("footer:test")),
            Some("type".to_string()),
            "enabled footer rows stay clickable but cannot become the default keyboard selection"
        );

        let footer_only = snapshot(vec![footer]);
        assert_eq!(preserve_or_pick_first_enabled(&footer_only, None), None);
    }

    #[test]
    fn visible_start_preserves_page_until_selection_leaves_it() {
        assert_eq!(trigger_picker_visible_start_for_selection(6, 9, 20), 6);
        assert_eq!(trigger_picker_visible_start_for_selection(6, 13, 20), 6);
        assert_eq!(trigger_picker_visible_start_for_selection(6, 14, 20), 7);
        assert_eq!(trigger_picker_visible_start_for_selection(6, 5, 20), 5);
    }

    #[test]
    fn update_preserves_selection_across_rebuild() {
        let prev = snapshot(vec![
            qualifier_row("type", true),
            qualifier_row("shortcut", true),
        ]);
        let state = MenuSyntaxTriggerPickerState {
            snapshot: Some(prev),
            selected_row_id: Some("type".to_string()),
            visible_start: 0,
        };
        // Same two rows, same state → NoChange.
        let transition = plan_trigger_picker_transition(&state, ":type:", &ctx());
        // `:type:` produces qualifier-value rows; we only check that the
        // transition is not spuriously Close / Open.
        match transition {
            TriggerPickerTransition::NoChange
            | TriggerPickerTransition::Update { .. }
            | TriggerPickerTransition::Open { .. } => {}
            other => panic!("unexpected transition {:?}", other),
        }
    }

    #[test]
    fn starts_with_legacy_trigger_matches_all_five() {
        assert!(starts_with_legacy_trigger("~"));
        assert!(starts_with_legacy_trigger("/"));
        assert!(starts_with_legacy_trigger("@"));
        assert!(starts_with_legacy_trigger(">"));
        assert!(starts_with_legacy_trigger("?"));
        assert!(starts_with_legacy_trigger("/scripts and things"));
    }

    #[test]
    fn starts_with_legacy_trigger_rejects_menu_syntax_prefixes() {
        assert!(!starts_with_legacy_trigger(":"));
        assert!(!starts_with_legacy_trigger("+"));
        assert!(!starts_with_legacy_trigger(";todo"));
        assert!(!starts_with_legacy_trigger(":type:script"));
        assert!(!starts_with_legacy_trigger("plain search"));
        assert!(!starts_with_legacy_trigger(""));
    }

    #[test]
    fn adapt_row_maps_qualifier_fields_to_neutral_shape() {
        let row = qualifier_row("type", true);
        let neutral = adapt_trigger_picker_row(&row);
        assert_eq!(neutral.id.as_ref(), "type");
        assert_eq!(neutral.kind, InlinePickerRowKind::TextTrigger);
        assert_eq!(neutral.title.as_ref(), "title-type");
        assert_eq!(neutral.token.as_deref().map(AsRef::as_ref), Some("type:"));
        assert_eq!(
            neutral.subtitle.as_deref().map(AsRef::as_ref),
            Some("Qualifier")
        );
        assert_eq!(
            neutral.example.as_deref().map(AsRef::as_ref),
            Some("type:value")
        );
        assert!(neutral.enabled);
    }

    #[test]
    fn main_list_action_binds_raw_owner_id_and_preserves_enabled_gate() {
        for enabled in [false, true] {
            let row = qualifier_row("type", enabled);
            let projected = trigger_picker_row_to_main_list_row(&row);
            assert_eq!(projected.id.as_ref(), "menu-syntax-trigger:type");
            assert_eq!(
                projected.action,
                crate::spine::SpineListAction::AcceptMenuSyntaxTrigger {
                    row_id: "type".into()
                }
            );
            assert_eq!(
                crate::scripts::SearchResult::SpineProjection(projected)
                    .command_descriptor()
                    .unwrap()
                    .can_execute(),
                enabled
            );
        }
    }

    #[test]
    fn adapt_row_maps_footer_action_to_action_kind() {
        let row = TriggerPickerRow {
            id: "help".to_string(),
            mode: TriggerPickerMode::AdvancedQuery,
            kind: TriggerPickerRowKind::FooterAction,
            title: "Footer action".to_string(),
            token: None,
            subtitle: None,
            detail: None,
            example: None,
            badges: Vec::new(),
            action: TriggerPickerAction::OpenHelp,
            enabled: true,
        };
        let neutral = adapt_trigger_picker_row(&row);
        assert_eq!(neutral.kind, InlinePickerRowKind::Action);
    }

    #[test]
    fn adapt_row_converts_badges_to_neutral_chips() {
        let mut row = qualifier_row("has", true);
        row.badges = vec!["default".to_string(), "shipped".to_string()];
        let neutral = adapt_trigger_picker_row(&row);
        assert_eq!(neutral.badges.len(), 2);
        assert_eq!(neutral.badges[0].label.as_ref(), "default");
        assert_eq!(neutral.badges[0].tone, InlinePickerBadgeTone::Neutral);
        assert_eq!(neutral.badges[1].label.as_ref(), "shipped");
    }

    #[test]
    fn adapt_row_preserves_disabled_flag() {
        let row = qualifier_row("disabled-row", false);
        let neutral = adapt_trigger_picker_row(&row);
        assert!(!neutral.enabled);
    }

    #[test]
    fn plus_partial_highlights_title_and_token_like_slash_picker() {
        let snap = build_trigger_picker_snapshot("+", &ctx()).expect("plus snapshot");
        let row = snap
            .rows
            .iter()
            .find(|row| row.token.as_deref() == Some("todo;"))
            .expect("todo target row");

        let highlights = trigger_picker_row_highlight_indices(row, "+to");

        assert_eq!(highlights.title, vec![0, 1]);
        assert_eq!(highlights.meta, vec![0, 1]);
    }

    #[test]
    fn plus_target_highlights_target_token() {
        let snap = build_trigger_picker_snapshot(";todo", &ctx()).expect("todo snapshot");
        let row = snap
            .rows
            .iter()
            .find(|row| row.token.as_deref() == Some("todo;"))
            .expect("todo target row");

        let highlights = trigger_picker_row_highlight_indices(row, ";todo");

        assert_eq!(highlights.title, vec![0, 1, 2, 3]);
        assert_eq!(highlights.meta, vec![0, 1, 2, 3]);
    }

    #[test]
    fn colon_query_highlights_qualifier_token() {
        // Bare `:` now lists only open-value qualifier heads (b3f83a991);
        // concrete `type:script` rows appear only after the head is accepted.
        let snap = build_trigger_picker_snapshot(":type", &ctx()).expect("colon type snapshot");
        let row = snap
            .rows
            .iter()
            .find(|row| row.token.as_deref() == Some("type:"))
            .expect("type qualifier head row");

        let highlights = trigger_picker_row_highlight_indices(row, ":type");

        assert_eq!(highlights.meta, vec![0, 1, 2, 3]);
        assert_eq!(highlights.title, vec![0, 1, 2, 3]);
    }

    #[test]
    fn colon_open_value_highlights_qualifier_head_after_value_starts() {
        let snap = build_trigger_picker_snapshot(":", &ctx()).expect("colon snapshot");
        let row = snap
            .rows
            .iter()
            .find(|row| row.token.as_deref() == Some("source:"))
            .expect("source qualifier row");

        let highlights = trigger_picker_row_highlight_indices(row, ":source:main");

        assert_eq!(highlights.meta, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn footer_rows_do_not_highlight_partial_queries() {
        // The bare-sigil picker no longer appends a footer row; the
        // create-handler footer only appears once a slug filter is typed
        // (`;t`), so exercise the footer from that state.
        let snap = build_trigger_picker_snapshot(";t", &ctx()).expect("semicolon slug snapshot");
        let row = snap
            .rows
            .iter()
            .find(|row| row.kind == TriggerPickerRowKind::FooterAction)
            .expect("footer action row");

        let highlights = trigger_picker_row_highlight_indices(row, ";t");

        assert!(highlights.title.is_empty());
        assert!(highlights.meta.is_empty());
    }

    #[test]
    fn partial_plus_trigger_keeps_popup_open_with_filtered_targets() {
        let state = MenuSyntaxTriggerPickerState::default();
        // `+t` should NARROW the picker to targets whose slug starts with
        // 't' — `+todo` matches, `+cal`, `+note`, `+social`, `+link` do
        // not. Loose `contains` matches would spuriously keep "Daily note"
        // (title contains 't' in "note"), "Tagged link" (title starts
        // with 't' but slug doesn't), etc.; we want slug-prefix semantics.
        let transition = plan_trigger_picker_transition(&state, "+t", &ctx());
        match transition {
            TriggerPickerTransition::Open { snapshot, .. } => {
                let target_slugs: Vec<String> = snapshot
                    .rows
                    .iter()
                    .filter_map(|row| {
                        // Postfix capture spelling: `todo;` → slug `todo`.
                        row.token
                            .as_deref()
                            .and_then(|t| t.strip_suffix(';'))
                            .map(str::to_string)
                    })
                    .collect();
                assert!(
                    target_slugs.iter().any(|s| s == "todo"),
                    "expected `+todo` row to survive `+t` filter, got {:?}",
                    target_slugs
                );
                assert!(
                    !target_slugs.iter().any(|s| s == "cal"),
                    "`+cal` should not match `+t` — slug doesn't start with 't'"
                );
                assert!(
                    !target_slugs.iter().any(|s| s == "note"),
                    "`+note` should not match `+t` even though title \"Daily note\" contains 't'"
                );
                assert!(
                    !target_slugs.iter().any(|s| s == "social"),
                    "`+social` should not match `+t` even though \"Social draft\" contains 't' in 'draft'"
                );
                assert!(
                    !target_slugs.iter().any(|s| s == "link"),
                    "`+link` should not match `+t` even though \"Tagged link\" starts with 't'"
                );
            }
            other => panic!("expected Open for `+t`, got {:?}", other),
        }
    }

    #[test]
    fn partial_colon_trigger_keeps_popup_open_with_filtered_qualifiers() {
        let state = MenuSyntaxTriggerPickerState::default();
        // `:typ` — partial match for `type:` qualifier. Parser returns
        // AdvancedQuery without filtering, so build_trigger_picker_snapshot
        // already keeps the picker open. This test pins the behavior.
        let transition = plan_trigger_picker_transition(&state, ":typ", &ctx());
        assert!(
            matches!(transition, TriggerPickerTransition::Open { .. }),
            "`:typ` should open the picker, got {:?}",
            transition
        );
    }

    #[test]
    fn partial_colon_trigger_matches_browser_source_labels() {
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, ":bro", &ctx());
        match transition {
            TriggerPickerTransition::Open { snapshot, .. } => {
                let titles: Vec<&str> =
                    snapshot.rows.iter().map(|row| row.title.as_str()).collect();
                assert!(titles.contains(&"Browser Tabs"), "got {titles:?}");
                assert!(titles.contains(&"Browser History"), "got {titles:?}");
            }
            other => panic!("expected Open for `:bro`, got {:?}", other),
        }
    }

    #[test]
    fn partial_colon_trigger_with_no_match_stays_closed() {
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, ":zzzzzzz", &ctx());
        assert_eq!(
            transition,
            TriggerPickerTransition::NoChange,
            "bogus partial `:zzzzzzz` should not spuriously open a blank picker"
        );
    }

    #[test]
    fn partial_plus_trigger_with_no_match_closes_popup() {
        // `+zzzzzzz` matches no known targets — picker should stay closed.
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, "+zzzzzzz", &ctx());
        assert_eq!(
            transition,
            TriggerPickerTransition::NoChange,
            "bogus partial `+zzzzzzz` should not spuriously open the picker"
        );
    }

    #[test]
    fn partial_plus_trigger_with_whitespace_falls_through_to_parser() {
        // Once whitespace appears, the parser claims the string as body
        // text. Our partial fallback must not kick in.
        let state = MenuSyntaxTriggerPickerState::default();
        let transition = plan_trigger_picker_transition(&state, "+ some text", &ctx());
        // `+ ` is not a valid capture, parser returns None, partial fallback
        // sees whitespace → returns None, transition is NoChange.
        assert_eq!(transition, TriggerPickerTransition::NoChange);
    }

    #[test]
    fn partial_plus_trigger_updates_existing_open_popup() {
        // Open on `+`, then type `+t` — picker should Update to the
        // filtered snapshot, not Close.
        let state = MenuSyntaxTriggerPickerState {
            snapshot: Some(snapshot(vec![qualifier_row("todo", true)])),
            selected_row_id: Some("todo".to_string()),
            visible_start: 0,
        };
        let transition = plan_trigger_picker_transition(&state, "+t", &ctx());
        match transition {
            TriggerPickerTransition::Open { .. } | TriggerPickerTransition::Update { .. } => {}
            other => panic!(
                "typing `+t` over an open picker should keep it open, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn launcher_trigger_updates_render_in_main_area_not_popup_window() {
        let source = std::fs::read_to_string("src/app_impl/filter_input_change.rs")
            .expect("Failed to read src/app_impl/filter_input_change.rs");
        let branch = source
            .split("} else if trigger_state_changed {")
            .nth(1)
            .and_then(|tail| tail.split("} else if matches!(").next())
            .expect("filter_input_change.rs should have a trigger_state_changed branch");

        assert!(
            branch.contains("self.invalidate_grouped_cache();"),
            "trigger snapshots should be documented as feeding the main search area"
        );
        assert!(
            !source.contains("sync_menu_syntax_trigger_picker_main_list_for_filter"),
            "filter input changes must not sync trigger snapshots into the detached picker window"
        );
    }

    #[test]
    fn launcher_trigger_state_machine_does_not_open_popup_window() {
        let source =
            std::fs::read_to_string("src/app_impl/menu_syntax_trigger_picker_main_list.rs")
                .expect("Failed to read src/app_impl/menu_syntax_trigger_picker_main_list.rs");
        let body = source
            .split("pub(crate) fn run_menu_syntax_trigger_picker_state_machine(")
            .nth(1)
            .and_then(|tail| {
                tail.split("pub(crate) fn menu_syntax_trigger_picker_context")
                    .next()
            })
            .expect("run_menu_syntax_trigger_picker_state_machine should exist");

        assert!(
            !body.contains("sync_menu_syntax_trigger_picker_main_list_for_filter"),
            "trigger state machine must preserve snapshots for the main area without opening the picker"
        );
    }
}

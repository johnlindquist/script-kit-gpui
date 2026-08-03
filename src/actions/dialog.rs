#![allow(dead_code)]

// --- merged from part_01.rs ---
// Actions Dialog
//
// The main ActionsDialog struct and its implementation, providing a searchable
// action menu as a compact overlay popup.

use crate::components::scrollbar::{Scrollbar, ScrollbarColors, ScrollbarMetrics};
use crate::designs::{get_tokens, DesignColors, DesignVariant};
use crate::logging;
use crate::menu_syntax_actions::{
    power_syntax_section_to_actions, PowerSyntaxActionSection, SectionMode,
};
use crate::protocol::ProtocolAction;
use crate::theme;
use crate::theme::types::BackgroundOpacity;
use crate::theme::AppChromeColors;
use gpui::{
    div, list, prelude::*, px, rgb, rgba, App, BoxShadow, Context, ElementId, Entity, FocusHandle,
    Focusable, ListAlignment, ListState, Render, SharedString, Subscription, Window,
};
use gpui_component::Sizable as _;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::builders::{
    format_shortcut_hint as format_shortcut_hint_shared, get_clipboard_history_context_actions,
    get_emoji_context_actions, get_file_context_actions, get_global_actions,
    get_path_context_actions, get_script_context_actions,
    get_scriptlet_context_actions_with_custom, ChatPromptInfo, ClipboardEntryInfo, EmojiActionInfo,
};
use super::constants::{ACTIONS_POPUP_RADIUS, ACTIONS_ROW_RADIUS, ACTION_ROW_INSET, POPUP_WIDTH};
#[allow(unused_imports)] // AnchorPosition reserved for future use
use super::types::{
    Action, ActionCallback, ActionCategory, ActionsDialogConfig, AnchorPosition, CloseCallback,
    ScriptInfo, SearchPosition, SectionStyle,
};
use crate::file_search::FileInfo;
use crate::prompts::PathInfo;
use crate::scriptlets::Scriptlet;

// --- Storybook adoption: style struct and defaults ---
// When the storybook feature is enabled, delegate to the storybook module.
// When disabled, use hardcoded defaults (the "Current" variant).

#[cfg(feature = "storybook")]
fn actions_dialog_default_style() -> crate::storybook::actions_dialog_variations::ActionsDialogStyle
{
    crate::storybook::actions_dialog_variations::adopted_actions_dialog_style()
}

#[cfg(not(feature = "storybook"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ActionsDialogStyleFallback {
    pub show_container_border: bool,
    pub show_header: bool,
    pub show_search_divider: bool,
    pub show_icons: bool,
    pub selection_opacity: f32,
    pub hover_opacity: f32,
    pub row_height: f32,
    pub row_radius: f32,
    pub shortcut_visible: bool,
    pub mono_font: bool,
    pub prefix_marker: Option<&'static str>,
}

#[cfg(not(feature = "storybook"))]
fn actions_dialog_default_style() -> ActionsDialogStyleFallback {
    let tokens = crate::designs::current_actions_popup_theme();
    ActionsDialogStyleFallback {
        show_container_border: false,
        show_header: true,
        show_search_divider: false,
        show_icons: false,
        selection_opacity: tokens.row.selection_opacity,
        hover_opacity: tokens.row.hover_opacity,
        row_height: tokens.list.row_height,
        row_radius: tokens.row.radius,
        shortcut_visible: true,
        mono_font: false,
        prefix_marker: None,
    }
}

/// Helper function to combine a hex color with an alpha value
/// Delegates to DesignColors::hex_with_alpha for DRY
#[inline]
fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
    DesignColors::hex_with_alpha(hex, alpha)
}

/// Action subtitle text shown in the popup row, if any.
///
/// Action-menu hosts suppress subtitle/description rendering to keep rows
/// visually focused on title + shortcut + icon. Switcher-style hosts (e.g.
/// the Notes recent-note switcher) opt in via `config.show_subtitles` to
/// surface `Action::description` as a preview/metadata line, matching
/// main-list row anatomy.
pub(crate) fn action_subtitle_for_display(action: &Action, show_subtitles: bool) -> Option<&str> {
    if !show_subtitles {
        return None;
    }
    action.description.as_deref().filter(|d| !d.is_empty())
}

/// Whether an action should render with destructive styling.
///
/// We key off stable action IDs first, then fall back to title prefixes for
/// dynamic or SDK-defined destructive actions.
///
/// Resolve shortcut tokens for render, preferring the pre-cached
/// `shortcut_tokens`. Falls back to on-demand parsing when tokens are missing.
/// This helper runs in the render path, so it must stay side-effect free.
fn action_shortcut_tokens_for_render(action: &Action) -> Option<std::borrow::Cow<'_, [String]>> {
    if !action.is_enabled() {
        return None;
    }
    if let Some(tokens) = action.shortcut_tokens.as_deref() {
        return Some(std::borrow::Cow::Borrowed(tokens));
    }
    let shortcut = action.shortcut.as_deref()?;
    Some(std::borrow::Cow::Owned(
        crate::components::hint_strip::shortcut_tokens_from_hint(shortcut),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VisibleActionShortcutBinding {
    pub action_id: String,
    pub shortcut: String,
    pub canonical: String,
    pub routable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionShortcutParityReport {
    pub displayed_shortcut_count: usize,
    pub routable_shortcut_count: usize,
    pub duplicate_shortcut_count: usize,
    pub unroutable_displayed_shortcuts: Vec<VisibleActionShortcutBinding>,
    pub visible_shortcut_bindings: Vec<VisibleActionShortcutBinding>,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(namespace = script_kit, no_json, no_register)]
pub(crate) struct MainListDisplayedActionShortcut {
    pub shortcut: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisplayedShortcutKeyBindingSpec {
    pub canonical: String,
    pub gpui_keystroke: String,
}

fn action_canonical_shortcut(action: &Action) -> Option<String> {
    if !action.is_enabled() {
        return None;
    }
    let display_shortcut = action.shortcut.as_deref()?;
    let canonical = crate::components::hint_strip::canonical_shortcut_hint(display_shortcut);
    if canonical.is_empty() {
        None
    } else {
        Some(canonical)
    }
}

fn action_matches_keystroke_shortcut(action: &Action, normalized_keystroke_shortcut: &str) -> bool {
    action_canonical_shortcut(action)
        .is_some_and(|canonical| canonical == normalized_keystroke_shortcut)
}

pub(crate) fn visible_action_shortcut_bindings(
    actions: &[Action],
    filtered_actions: &[usize],
) -> Vec<VisibleActionShortcutBinding> {
    let mut canonical_counts: HashMap<String, usize> = HashMap::new();
    let mut bindings = Vec::new();

    for &action_idx in filtered_actions {
        let Some(action) = actions.get(action_idx) else {
            continue;
        };
        if !action.is_enabled() {
            continue;
        }
        let Some(shortcut) = action.shortcut.clone() else {
            continue;
        };
        let canonical = crate::components::hint_strip::canonical_shortcut_hint(&shortcut);
        if canonical.is_empty() {
            bindings.push(VisibleActionShortcutBinding {
                action_id: action.id.clone(),
                shortcut,
                canonical,
                routable: false,
            });
            continue;
        }
        *canonical_counts.entry(canonical.clone()).or_insert(0) += 1;
        bindings.push(VisibleActionShortcutBinding {
            action_id: action.id.clone(),
            shortcut,
            canonical,
            routable: true,
        });
    }

    for binding in &mut bindings {
        if binding.canonical.is_empty()
            || canonical_counts
                .get(&binding.canonical)
                .is_some_and(|count| *count > 1)
        {
            binding.routable = false;
        }
    }

    bindings
}

fn action_has_routable_shortcut(
    actions: &[Action],
    filtered_actions: &[usize],
    action_id: &str,
) -> bool {
    visible_action_shortcut_bindings(actions, filtered_actions)
        .into_iter()
        .any(|binding| binding.action_id == action_id && binding.routable)
}

pub(crate) fn action_shortcut_parity_report(
    actions: &[Action],
    filtered_actions: &[usize],
) -> ActionShortcutParityReport {
    let bindings = visible_action_shortcut_bindings(actions, filtered_actions);
    let routable_shortcut_count = bindings.iter().filter(|binding| binding.routable).count();
    let displayed_shortcut_count = routable_shortcut_count;
    let duplicate_shortcut_count = bindings
        .iter()
        .filter(|binding| {
            !binding.canonical.is_empty()
                && bindings
                    .iter()
                    .filter(|other| other.canonical == binding.canonical)
                    .count()
                    > 1
        })
        .count();
    // Unroutable candidates are intentionally not displayed: a visible keycap
    // without one executable route is a false promise.
    let unroutable_displayed_shortcuts = Vec::new();

    ActionShortcutParityReport {
        displayed_shortcut_count,
        routable_shortcut_count,
        duplicate_shortcut_count,
        unroutable_displayed_shortcuts,
        visible_shortcut_bindings: bindings,
    }
}

pub(crate) fn gpui_keystroke_for_canonical_shortcut(canonical: &str) -> Option<String> {
    let canonical = canonical.trim();
    if canonical.is_empty() {
        None
    } else {
        Some(canonical.replace('+', "-"))
    }
}

pub(crate) fn displayed_action_keybinding_specs(
    actions: &[Action],
    filtered_actions: &[usize],
) -> Vec<DisplayedShortcutKeyBindingSpec> {
    action_shortcut_parity_report(actions, filtered_actions)
        .visible_shortcut_bindings
        .into_iter()
        .filter(|binding| binding.routable)
        .filter_map(|binding| {
            let gpui_keystroke = gpui_keystroke_for_canonical_shortcut(&binding.canonical)?;
            Some(DisplayedShortcutKeyBindingSpec {
                canonical: binding.canonical,
                gpui_keystroke,
            })
        })
        .collect()
}

pub(crate) fn matching_action_id_for_canonical_shortcut(
    actions: &[Action],
    filtered_actions: &[usize],
    canonical_shortcut: &str,
) -> Option<String> {
    let mut matches = filtered_actions.iter().filter_map(|&action_idx| {
        let action = actions.get(action_idx)?;
        let action_canonical = action_canonical_shortcut(action)?;
        (action_canonical == canonical_shortcut).then(|| action.id.clone())
    });

    let first = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(first)
    }
}

pub(crate) fn resolve_visible_action_shortcut(
    actions: &[Action],
    filtered_actions: &[usize],
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<String> {
    let keystroke_shortcut = crate::shortcuts::keystroke_to_shortcut(key, modifiers);
    matching_action_id_for_canonical_shortcut(actions, filtered_actions, &keystroke_shortcut)
}

pub(crate) fn matching_action_id_for_keystroke(
    actions: &[Action],
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<String> {
    let filtered_actions: Vec<usize> = (0..actions.len()).collect();
    resolve_visible_action_shortcut(actions, &filtered_actions, key, modifiers)
}

pub(crate) fn matching_filtered_action_id_for_keystroke(
    actions: &[Action],
    filtered_actions: &[usize],
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<String> {
    resolve_visible_action_shortcut(actions, filtered_actions, key, modifiers)
}

fn clear_action_shortcut(action: &mut Action) {
    action.shortcut = None;
    action.shortcut_tokens = None;
    action.shortcut_lower = None;
}

pub(super) fn clear_duplicate_action_shortcuts(actions: &mut [Action]) {
    let mut seen = HashSet::new();

    for action in actions {
        let Some(shortcut) = action.shortcut.as_deref() else {
            continue;
        };
        let canonical = crate::components::hint_strip::canonical_shortcut_hint(shortcut);
        if canonical.is_empty() {
            continue;
        }
        if !seen.insert(canonical) {
            clear_action_shortcut(action);
        }
    }
}

pub(crate) fn is_destructive_action(action: &Action) -> bool {
    let id = action.id.as_str();

    if id == "move_to_trash"
        || id == "reset_ranking"
        || id == "clear_conversation"
        || id == "force_quit_app"
        || id.starts_with("remove_")
        || id.starts_with("delete_")
        || id.contains("_delete")
        || id.contains("_trash")
    {
        return true;
    }

    action.title_lower.starts_with("remove ")
        || action.title_lower.starts_with("delete ")
        || action.title_lower.starts_with("clear ")
        || action.title_lower.starts_with("force quit ")
        || action.title_lower.starts_with("move to trash")
}

/// Grouped action item for variable-height list rendering
/// Section headers are 22px, action items are 36px
#[derive(Clone, Debug)]
pub enum GroupedActionItem {
    /// A section header (e.g., "Actions", "Navigation")
    SectionHeader(String),
    /// An action item - usize is the index in filtered_actions
    Item(usize),
}

#[inline]
pub(super) fn grouped_action_item_eligibility(
    row: &GroupedActionItem,
) -> crate::list_item::RowEligibility {
    match row {
        GroupedActionItem::SectionHeader(_) => crate::list_item::RowEligibility::inert(),
        GroupedActionItem::Item(_) => crate::list_item::RowEligibility::enabled_action(),
    }
}

fn grouped_action_eligibilities(
    rows: &[GroupedActionItem],
) -> Vec<crate::list_item::RowEligibility> {
    rows.iter().map(grouped_action_item_eligibility).collect()
}

fn action_row_eligibility(action: &Action) -> crate::list_item::RowEligibility {
    if action.is_enabled() {
        crate::list_item::RowEligibility::enabled_action()
    } else {
        crate::list_item::RowEligibility::disabled_explanation()
    }
}

pub(super) fn first_selectable_index(rows: &[GroupedActionItem]) -> Option<usize> {
    crate::list_item::first_selectable_eligibility_index(&grouped_action_eligibilities(rows))
}

pub(super) fn last_selectable_index(rows: &[GroupedActionItem]) -> Option<usize> {
    crate::list_item::last_selectable_eligibility_index(&grouped_action_eligibilities(rows))
}

pub(super) fn selectable_index_at_or_before(
    rows: &[GroupedActionItem],
    start: usize,
) -> Option<usize> {
    crate::list_item::selectable_eligibility_index_at_or_before(
        &grouped_action_eligibilities(rows),
        start,
    )
}

pub(super) fn selectable_index_at_or_after(
    rows: &[GroupedActionItem],
    start: usize,
) -> Option<usize> {
    crate::list_item::selectable_eligibility_index_at_or_after(
        &grouped_action_eligibilities(rows),
        start,
    )
}

pub(super) fn coerce_action_selection(rows: &[GroupedActionItem], ix: usize) -> Option<usize> {
    crate::list_item::coerce_eligibility_selection(&grouped_action_eligibilities(rows), ix)
}

/// Compute the initial selected row for grouped items. `None` is the only
/// representation of a list with no selectable action row.
pub(super) fn initial_selection_index(rows: &[GroupedActionItem]) -> Option<usize> {
    coerce_action_selection(rows, 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionsRefreshSelectionPolicy {
    SnapToFirst,
    RestoreIdentity,
}

pub(super) fn actions_refresh_selection_policy(
    user_moved_selection: bool,
) -> ActionsRefreshSelectionPolicy {
    if user_moved_selection {
        ActionsRefreshSelectionPolicy::RestoreIdentity
    } else {
        ActionsRefreshSelectionPolicy::SnapToFirst
    }
}

/// Whether config changes require rebuilding grouped rows.
///
/// Grouped rows depend on section style because `Headers` injects extra rows.
pub(super) fn should_rebuild_grouped_items_for_config_change(
    previous: &ActionsDialogConfig,
    next: &ActionsDialogConfig,
) -> bool {
    previous.section_style != next.section_style
}

/// Resolve a selected protocol action index from the selected visible action index.
///
/// `sdk_action_indices` maps visible action indices to indices in the original
/// SDK protocol action array.
pub(super) fn resolve_selected_protocol_action_index(
    selected_action_index: Option<usize>,
    sdk_action_indices: &[usize],
) -> Option<usize> {
    selected_action_index.and_then(|action_idx| sdk_action_indices.get(action_idx).copied())
}

/// Build grouped items from actions and filtered_actions
/// This is a static helper used during construction to avoid borrowing issues
pub(super) fn build_grouped_items_static(
    actions: &[Action],
    filtered_actions: &[usize],
    section_style: SectionStyle,
) -> Vec<GroupedActionItem> {
    let mut grouped = Vec::new();

    if filtered_actions.is_empty() {
        return grouped;
    }

    let mut prev_section: Option<String> = None;
    let mut prev_category: Option<ActionCategory> = None;
    for (filter_idx, &action_idx) in filtered_actions.iter().enumerate() {
        if let Some(action) = actions.get(action_idx) {
            match section_style {
                SectionStyle::Headers => {
                    // Add section header when section changes
                    if let Some(ref section) = action.section {
                        if prev_section.as_ref() != Some(section) {
                            grouped.push(GroupedActionItem::SectionHeader(section.clone()));
                            prev_section = Some(section.clone());
                        }
                    }
                }
                SectionStyle::Separators | SectionStyle::None => {
                    // For separators, we track category changes but don't add headers
                    // (separators are rendered inline in the item renderer)
                    prev_category = Some(action.category.clone());
                }
            }
            grouped.push(GroupedActionItem::Item(filter_idx));
        }
    }

    // Suppress unused variable warning
    let _ = prev_category;

    grouped
}

/// Whether a separator line should be shown before a filtered item index.
///
/// Used for `SectionStyle::Separators` so we can visually group sections
/// without injecting explicit header rows.
pub(super) fn should_render_section_separator(
    actions: &[Action],
    filtered_actions: &[usize],
    filter_idx: usize,
) -> bool {
    if filter_idx == 0 {
        return false;
    }

    let current_action = filtered_actions
        .get(filter_idx)
        .and_then(|&idx| actions.get(idx));
    let previous_action = filtered_actions
        .get(filter_idx - 1)
        .and_then(|&idx| actions.get(idx));

    match (previous_action, current_action) {
        (Some(prev), Some(curr)) => prev.section != curr.section,
        _ => false,
    }
}

const ACTIONS_DIALOG_FOOTER_HEIGHT: f32 = 32.0;

#[inline]
fn actions_dialog_list_overdraw_px() -> f32 {
    crate::designs::current_actions_popup_theme()
        .list
        .overdraw_px
}
const ACTIONS_DIALOG_SCROLLBAR_IDLE_DELAY: Duration = Duration::from_millis(900);
const ACTIONS_DIALOG_SCROLLBAR_FADE_TICK: Duration = Duration::from_millis(16);

#[inline]
fn actions_dialog_scrollbar_fade_duration() -> Duration {
    crate::transitions::DURATION_MEDIUM + Duration::from_millis(50)
}

#[inline]
fn actions_dialog_scrollbar_fade_opacity(progress: f32) -> crate::transitions::Opacity {
    use crate::transitions::Lerp;
    let eased = crate::transitions::ease_in_quad(progress.clamp(0.0, 1.0));
    crate::transitions::Opacity::VISIBLE.lerp(&crate::transitions::Opacity::INVISIBLE, eased)
}

/// Calculate the list viewport height used for scrollbar geometry.
///
/// This must mirror popup layout constraints so the scrollbar thumb represents
/// the visible list region (excluding search/header/footer chrome).
pub(super) fn actions_dialog_scrollbar_viewport_height(
    total_content_height: f32,
    show_search: bool,
    has_header: bool,
    show_footer: bool,
    max_height: f32,
) -> f32 {
    let tokens = crate::designs::current_actions_popup_theme();
    let search_height = if show_search {
        tokens.search.height
    } else {
        0.0
    };
    let header_height = if has_header {
        tokens.context_header.height
    } else {
        0.0
    };
    let footer_height = if show_footer {
        ACTIONS_DIALOG_FOOTER_HEIGHT
    } else {
        0.0
    };
    let list_padding_height = tokens.list.padding_top + tokens.list.padding_bottom;
    let available_viewport_height = (max_height.min(tokens.shell.max_height)
        - search_height
        - header_height
        - footer_height
        - list_padding_height)
        .max(0.0);

    total_content_height.min(available_viewport_height)
}

/// List viewport inside an already-sized shell. Unlike the inline/dynamic
/// helper above, this intentionally does not clamp to content height: short
/// and empty results retain the same pointer target and outer popup bounds.
pub(super) fn actions_dialog_fixed_shell_viewport_height(
    shell_height: f32,
    show_search: bool,
    has_header: bool,
    show_footer: bool,
) -> f32 {
    let tokens = crate::designs::current_actions_popup_theme();
    let search_height = if show_search {
        tokens.search.height
    } else {
        0.0
    };
    let header_height = if has_header {
        tokens.context_header.height
    } else {
        0.0
    };
    let footer_height = if show_footer {
        ACTIONS_DIALOG_FOOTER_HEIGHT
    } else {
        0.0
    };
    let list_padding_height = tokens.list.padding_top + tokens.list.padding_bottom;

    (shell_height
        - tokens.shell.border_height
        - search_height
        - header_height
        - footer_height
        - list_padding_height)
        .max(0.0)
}

pub(super) fn actions_dialog_revealed_scroll_top(
    current_top: f32,
    viewport_height: f32,
    content_height: f32,
    selected_top: f32,
    selected_bottom: f32,
) -> f32 {
    let current_bottom = current_top + viewport_height;
    let next_top = if selected_top < current_top {
        selected_top
    } else if selected_bottom > current_bottom {
        selected_bottom - viewport_height
    } else {
        current_top
    };
    next_top.clamp(0.0, (content_height - viewport_height).max(0.0))
}

// ── Route / back-stack contract ──────────────────────────────────────────────
// Reusable drill-down navigation for the shared ActionsDialog.

/// A route represents a named set of actions that can be displayed in the dialog.
/// Routes are pushed onto a stack to support drill-down navigation (e.g.,
/// root actions -> agent picker) with back-stack semantics on Escape.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionsDialogRoute {
    pub id: String,
    pub actions: Vec<Action>,
    pub context_title: Option<String>,
    pub search_placeholder: Option<String>,
    /// Action ID to pre-select when this route is first displayed.
    pub initial_selected_action_id: Option<String>,
}

/// Snapshot of per-route UI state (search text, selected item) so that
/// popping back to a parent route restores the user's position.
#[derive(Clone, Debug)]
pub(super) struct ActionsDialogRouteState {
    pub(super) route: ActionsDialogRoute,
    pub(super) search_text: String,
    pub(super) selected_action_id: Option<String>,
}

impl ActionsDialogRouteState {
    pub(super) fn new(route: ActionsDialogRoute) -> Self {
        let selected_action_id = route.initial_selected_action_id.clone();
        Self {
            route,
            search_text: String::new(),
            selected_action_id,
        }
    }
}

/// Immutable inputs used to size one detached Actions shell at open time.
///
/// The snapshot intentionally excludes live filter text and child-route state.
/// Once attached, all content changes reflow or scroll inside the resulting
/// outer bounds for the lifetime of that popup.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionsDialogShellSizingSnapshot {
    pub route_id: Option<String>,
    pub action_count: usize,
    pub section_header_count: usize,
    pub search_visible: bool,
    pub context_header_visible: bool,
    pub footer_visible: bool,
    pub max_height_px: f32,
    pub row_height_px: f32,
}

/// The result of pressing Enter (or clicking) on the selected action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionsDialogActivation {
    /// A drill-down route was pushed onto the stack.
    DrillDownPushed { action_id: String, route_id: String },
    /// The action was executed via the on_select callback.
    Executed {
        action_id: String,
        should_close: bool,
    },
    /// The row remains selected so its explanation can be read, but execution
    /// is blocked at the one shared activation guard.
    Blocked { action_id: String, reason: String },
    /// Nothing was selected.
    NoSelection,
}

pub(crate) type ActivationCallback =
    Arc<dyn Fn(ActionsDialogActivation, &mut Window, &mut gpui::App) + Send + Sync>;

/// The result of pressing Escape in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionsDialogEscapeOutcome {
    /// A child route was popped; the parent route is now visible.
    PoppedRoute,
    /// The stack is at root; the dialog should be closed.
    CloseDialog,
}

#[derive(Clone, Copy)]
pub struct AgentChatActionsDialogContext<'a> {
    pub(crate) available_models: &'a [crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    pub(crate) selected_model_id: Option<&'a str>,
    pub(crate) focused_text: bool,
    pub(crate) focused_text_expanded: bool,
    /// Count of session "Allow always" grants; >0 adds the review action.
    pub(crate) standing_approval_count: usize,
    /// Retained background threads, surfaced as a "Threads" switcher section.
    pub(crate) thread_summaries: &'a [crate::ai::agent_chat::ui::AgentChatThreadSummary],
    /// Rewindable user messages (Pi fork checkpoints) for "Rewind & Edit".
    pub(crate) fork_points: &'a [crate::ai::agent_chat::ui::AgentChatForkPoint],
}

/// Immutable parent/subject identity captured when an ActionsDialog opens.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionsHostContextSnapshot {
    pub host: String,
    pub parent_automation_id: Option<String>,
    pub parent_kind: Option<String>,
    pub parent_semantic_surface: Option<String>,
    pub parent_subject_id: Option<String>,
    pub parent_subject_text_fingerprint: Option<String>,
    pub selected_semantic_id: Option<String>,
}

/// ActionsDialog - Compact overlay popup for quick actions
/// Implements Raycast-style design with individual keycap shortcuts
///
/// # Configuration
/// Use `ActionsDialogConfig` to customize appearance:
/// - `search_position`: Top for searchable Actions, or Hidden for external search
/// - `section_style`: Headers (text labels) or Separators (subtle lines)
/// - `anchor`: Top (list grows down) or Bottom (list grows up)
/// - `show_icons`: Display icons next to actions
/// - `show_footer`: Legacy flag; action dialogs keep shortcuts inline and omit a footer
pub struct ActionsDialog {
    pub actions: Vec<Action>,
    pub filtered_actions: Vec<usize>, // Indices into actions
    /// Title match char indices for the current search, aligned positionally
    /// with `filtered_actions`. Cleared whenever the filter is reset outside
    /// the scored path (see `reset_filter_to_all`).
    pub(crate) filtered_title_match_indices: Vec<Vec<usize>>,
    /// Description match char indices, aligned with `filtered_actions`.
    /// Only rendered when `config.show_subtitles` is set.
    pub(crate) filtered_description_match_indices: Vec<Vec<usize>>,
    pub selected_index: Option<usize>, // Index within grouped_items; None when no row is selectable
    /// Renderer-neutral search value mirrored from the entity-backed input.
    pub search_text: String,
    /// The sole text-editing owner for Actions search.
    pub(crate) search_input: Option<Entity<gpui_component::input::InputState>>,
    search_input_subscription: Option<Subscription>,
    search_input_syncing: bool,
    pub focus_handle: FocusHandle,
    pub on_select: ActionCallback,
    /// Currently focused script for context-aware actions
    pub focused_script: Option<ScriptInfo>,
    /// Currently focused scriptlet (for H3-defined custom actions)
    pub focused_scriptlet: Option<Scriptlet>,
    pub menu_syntax_section: Option<PowerSyntaxActionSection>,
    /// Host-owned contextual section (e.g. the Day Page "Today" actions).
    /// Rendered above script/global rows; rows are full `Action` values so the
    /// host controls ids, shortcuts, and section titles.
    pub host_section: Option<Vec<Action>>,
    /// List state for variable-height list (section headers 22px, items 36px)
    pub list_state: ListState,
    /// Grouped items for list rendering (includes section headers)
    pub grouped_items: Vec<GroupedActionItem>,
    /// Theme for consistent color styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
    /// Cursor visibility for blinking (controlled externally)
    pub cursor_visible: bool,
    /// When true, hide the search input (used when rendered inline in main.rs header)
    pub hide_search: bool,
    /// SDK-provided actions (when present, replaces built-in actions)
    pub sdk_actions: Option<Vec<ProtocolAction>>,
    /// Visible action index -> original protocol action index mapping.
    /// Keeps SDK action resolution deterministic when names collide.
    sdk_action_indices: Vec<usize>,
    /// Context title shown in the header (e.g., "Activity Monitor", script name)
    pub context_title: Option<String>,
    /// Configuration for appearance and behavior
    pub config: ActionsDialogConfig,
    /// When true, skip track_focus in render (parent handles focus, e.g., ActionsWindow)
    pub skip_track_focus: bool,
    /// When true, the container fills the hosting window's bounds instead of
    /// its token-derived inline shell. Detached Actions uses this so GPUI content
    /// reflows with the calibrated native grow-in presentation.
    pub fill_window_bounds: bool,
    /// Outer shell height frozen from the root, unfiltered route at open.
    /// Content/filter/route mutations never rewrite this value.
    fixed_shell_height_px: Option<f32>,
    /// When true, reuse the main window vibrancy alpha for the dialog container.
    /// This is for detached popup-window actions surfaces that should read like the
    /// same background/material stack as their parent window.
    pub match_main_window_background: bool,
    /// Callback for when the dialog is closed (escape pressed, window dismissed)
    /// Used to notify the main app to restore focus
    pub on_close: Option<CloseCallback>,
    /// Callback for mouse-triggered activations that need parent window handling.
    on_activation: Option<ActivationCallback>,
    /// Parent/subject identity captured before the popup is registered.
    host_context: Option<ActionsHostContextSnapshot>,
    // ── Route / back-stack state ─────────────────────────────────────────────
    /// Stack of route states (empty = no route-based navigation active).
    route_stack: Vec<ActionsDialogRouteState>,
    /// Registered drill-down routes keyed by the action ID that triggers them.
    drill_down_routes: HashMap<String, ActionsDialogRoute>,
    /// Original search placeholder to restore when no route overrides it.
    default_search_placeholder: Option<String>,
    /// Tracks the last row armed by a mouse click so actions require an explicit
    /// second click, while native double-click still submits immediately.
    mouse_armed_row: Option<usize>,
    /// Last row entered by pointer hover. This mirrors GPUI's visual hover so
    /// DevTools can prove hover/selection mismatches without screenshots.
    hovered_row: Option<usize>,
    /// Current animated scrollbar visibility (0.0 hidden .. 1.0 visible).
    scrollbar_visibility: crate::transitions::Opacity,
    /// Generation counter used to cancel stale scrollbar fade tasks.
    scrollbar_fade_gen: u64,
    /// Last scroll activity time used by the idle fade timer.
    last_scroll_time: Option<Instant>,
    /// Pixel scroll position expected from pending keyboard reveal work.
    pending_scrollbar_scroll_top_y: Option<f32>,
    /// True only after explicit keyboard, pointer, or scroll navigation.
    /// Automatic/default selection must not gain identity across refreshes.
    actions_selection_user_moved: bool,
}

#[inline]
fn actions_dialog_footerless_config(mut config: ActionsDialogConfig) -> ActionsDialogConfig {
    config.show_footer = false;
    config
}

pub(super) fn append_disabled_action_test_fixture(actions: &mut Vec<Action>) {
    let test_status = std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() == Some("1");
    let fixture_enabled = std::env::var("SCRIPT_KIT_TEST_DISABLED_ACTION_FIXTURE")
        .ok()
        .as_deref()
        == Some("1");
    if !test_status
        || !fixture_enabled
        || actions
            .iter()
            .any(|action| action.id == "consistency_disabled_action")
    {
        return;
    }

    actions.push(
        Action::new(
            "consistency_disabled_action",
            "Unavailable Test Action",
            Some("Deterministic disabled-action fixture".to_string()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘⌥⇧9")
        .with_section("Test Fixtures")
        .disabled("Requires an unavailable test capability"),
    );
}

// --- merged from part_02.rs ---
// --- merged from part_01.rs ---
impl ActionsDialog {
    fn shows_context_header(&self) -> bool {
        self.config.show_context_header && self.context_title.is_some()
    }

    /// One canonical visibility predicate for rendering, focus, input setup,
    /// geometry, and automation. Hidden/external-search variants never install
    /// or focus a local input owner.
    pub(crate) fn search_is_visible(&self) -> bool {
        matches!(self.config.search_position, SearchPosition::Top) && !self.hide_search
    }

    /// Snapshot the root, unfiltered content that owns this popup lifetime's
    /// outer shell size. A currently visible child route never rebases it.
    pub(crate) fn opening_shell_sizing_snapshot(&self) -> ActionsDialogShellSizingSnapshot {
        let (route_id, root_actions, root_context_title) =
            if let Some(root) = self.route_stack.first() {
                let mut actions = root.route.actions.clone();
                append_disabled_action_test_fixture(&mut actions);
                (
                    Some(root.route.id.clone()),
                    actions,
                    root.route.context_title.as_ref(),
                )
            } else {
                (None, self.actions.clone(), self.context_title.as_ref())
            };
        let root_indices: Vec<usize> = (0..root_actions.len()).collect();
        let section_header_count = if self.config.section_style == SectionStyle::Headers {
            super::window::count_section_headers(&root_actions, &root_indices)
        } else {
            0
        };

        ActionsDialogShellSizingSnapshot {
            route_id,
            action_count: root_actions.len(),
            section_header_count,
            search_visible: self.search_is_visible(),
            context_header_visible: self.config.show_context_header
                && root_context_title.is_some()
                && actions_dialog_default_style().show_header,
            footer_visible: self.config.show_footer,
            max_height_px: self.config.max_height,
            row_height_px: self.effective_row_height(),
        }
    }

    pub(crate) fn attach_to_fixed_shell(&mut self, height_px: f32) {
        self.fill_window_bounds = true;
        self.fixed_shell_height_px = Some(height_px);
    }

    pub(crate) fn release_fixed_shell(&mut self) {
        self.fill_window_bounds = false;
        self.fixed_shell_height_px = None;
    }

    pub(crate) fn fixed_shell_height_px(&self) -> Option<f32> {
        self.fixed_shell_height_px
    }

    /// Row height for this host's action rows.
    ///
    /// Switcher-style hosts (`config.show_subtitles`) render two-line rows
    /// with main-list anatomy, so they use the shared `LIST_ITEM_HEIGHT`
    /// token. Action menus stay on the compact popup row token. Window
    /// sizing (`compute_popup_height`) and the dialog's interior scroll math
    /// both derive from this so they can never disagree.
    pub(crate) fn effective_row_height(&self) -> f32 {
        if self.config.show_subtitles {
            crate::list_item::LIST_ITEM_HEIGHT
        } else {
            actions_dialog_default_style().row_height
        }
    }

    fn row_height_for_scroll(item: &GroupedActionItem, row_height: f32) -> f32 {
        match item {
            GroupedActionItem::SectionHeader(_) => {
                crate::designs::current_actions_popup_theme()
                    .list
                    .section_header_height
            }
            GroupedActionItem::Item(_) => row_height,
        }
    }

    fn scroll_content_height_for_items(items: &[GroupedActionItem], row_height: f32) -> f32 {
        items
            .iter()
            .map(|item| Self::row_height_for_scroll(item, row_height))
            .sum()
    }

    fn min_scroll_content_height(&self, row_height: f32) -> f32 {
        let has_action = self
            .grouped_items
            .iter()
            .any(|item| matches!(item, GroupedActionItem::Item(_)));
        if has_action {
            0.0
        } else {
            row_height
        }
    }

    fn actions_scroll_content_height(&self, row_height: f32) -> f32 {
        Self::scroll_content_height_for_items(&self.grouped_items, row_height)
            .max(self.min_scroll_content_height(row_height))
    }

    fn actions_scroll_viewport_height(&self, row_height: f32) -> f32 {
        let show_search = self.search_is_visible();
        let has_header = self.shows_context_header() && actions_dialog_default_style().show_header;
        if let Some(shell_height) = self.fixed_shell_height_px {
            actions_dialog_fixed_shell_viewport_height(
                shell_height,
                show_search,
                has_header,
                self.config.show_footer,
            )
        } else {
            actions_dialog_scrollbar_viewport_height(
                self.actions_scroll_content_height(row_height),
                show_search,
                has_header,
                self.config.show_footer,
                self.config.max_height,
            )
        }
    }

    fn scroll_top_y_for_item_index(&self, item_index: usize, row_height: f32) -> f32 {
        self.grouped_items
            .iter()
            .take(item_index)
            .map(|item| Self::row_height_for_scroll(item, row_height))
            .sum()
    }

    fn live_scroll_top_y(&self, row_height: f32) -> f32 {
        self.scroll_top_y_for_item_index(self.list_state.logical_scroll_top().item_ix, row_height)
    }

    fn effective_scroll_top_y(&self, row_height: f32) -> f32 {
        let content_height = self.actions_scroll_content_height(row_height);
        let viewport_height = self.actions_scroll_viewport_height(row_height);
        let max_scroll_top = (content_height - viewport_height).max(0.0);
        self.pending_scrollbar_scroll_top_y
            .unwrap_or_else(|| self.live_scroll_top_y(row_height))
            .clamp(0.0, max_scroll_top)
    }

    fn selected_row_content_range(&self, row_height: f32) -> Option<(f32, f32)> {
        let selected_index = self.selected_index?;
        let item = self.grouped_items.get(selected_index)?;
        let top = self.scroll_top_y_for_item_index(selected_index, row_height);
        Some((top, top + Self::row_height_for_scroll(item, row_height)))
    }

    fn scrollbar_metrics(&self, row_height: f32) -> ScrollbarMetrics {
        ScrollbarMetrics::from_pixels(
            self.actions_scroll_content_height(row_height),
            self.actions_scroll_viewport_height(row_height),
            self.effective_scroll_top_y(row_height),
        )
    }

    fn update_pending_scrollbar_reveal_offset(&mut self, row_height: f32) {
        if first_selectable_index(&self.grouped_items) == self.selected_index {
            self.pending_scrollbar_scroll_top_y = Some(0.0);
            return;
        }

        let Some((selected_top, selected_bottom)) = self.selected_row_content_range(row_height)
        else {
            self.pending_scrollbar_scroll_top_y = None;
            return;
        };

        let content_height = self.actions_scroll_content_height(row_height);
        let viewport_height = self.actions_scroll_viewport_height(row_height);
        let current_top = self.effective_scroll_top_y(row_height);
        self.pending_scrollbar_scroll_top_y = Some(actions_dialog_revealed_scroll_top(
            current_top,
            viewport_height,
            content_height,
            selected_top,
            selected_bottom,
        ));
    }

    fn clear_pending_scrollbar_offset(&mut self) {
        self.pending_scrollbar_scroll_top_y = None;
    }

    pub(crate) fn scroll_to_selected(&self) {
        if let Some(selected_index) = self.selected_index {
            self.list_state.scroll_to_reveal_item(selected_index);
        }
    }

    fn search_placeholder_text(&self) -> SharedString {
        self.config
            .search_placeholder
            .as_ref()
            .cloned()
            .or_else(|| {
                self.context_title
                    .clone()
                    .filter(|_| !self.config.show_context_header)
            })
            .unwrap_or_else(|| "Search actions...".to_string())
            .into()
    }

    /// Build a presentation model from the current live dialog state.
    ///
    /// This extracts the same data that the shared presenter uses, enabling
    /// storybook and live app to share a common data contract. The live dialog
    /// still uses its own interactive render path (with list virtualization,
    /// scrollbars, click handlers) but this model serves as the verified
    /// bridge between the two surfaces.
    #[cfg(feature = "storybook")]
    pub fn build_presentation_model(&self) -> crate::storybook::ActionsDialogPresentationModel {
        let search_placeholder = self.search_placeholder_text();
        let show_search = self.search_is_visible();
        let search_at_top = matches!(self.config.search_position, SearchPosition::Top);

        let items = self
            .grouped_items
            .iter()
            .filter_map(|grouped_item| match grouped_item {
                GroupedActionItem::SectionHeader(label) => Some(
                    crate::storybook::ActionsDialogPresentationItem::SectionHeader(
                        SharedString::from(label.clone()),
                    ),
                ),
                GroupedActionItem::Item(filter_idx) => {
                    let action_idx = *self.filtered_actions.get(*filter_idx)?;
                    let action = self.actions.get(action_idx)?;
                    Some(crate::storybook::ActionsDialogPresentationItem::Action(
                        crate::storybook::ActionsDialogPresentationAction {
                            title: SharedString::from(action.title.clone()),
                            subtitle: action_subtitle_for_display(
                                action,
                                self.config.show_subtitles,
                            )
                            .map(|v| SharedString::from(v.to_string())),
                            shortcut: action.shortcut.clone().map(SharedString::from),
                            icon_svg_path: action
                                .icon
                                .map(|icon| SharedString::from(icon.asset_path().to_string())),
                            is_destructive: is_destructive_action(action),
                        },
                    ))
                }
            })
            .collect();

        crate::storybook::ActionsDialogPresentationModel {
            context_title: self.context_title.clone().map(SharedString::from),
            search_text: SharedString::from(self.search_text.clone()),
            search_placeholder,
            cursor_visible: self.cursor_visible,
            show_search,
            search_at_top,
            show_footer: false,
            items,
            selected_index: self.selected_index.unwrap_or_default(),
            hovered_index: None,
            input_mode_mouse: true,
        }
    }

    pub fn new(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_script_and_design(focus_handle, on_select, None, theme, DesignVariant::Default)
    }

    pub fn with_script(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        focused_script: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_script_and_design(
            focus_handle,
            on_select,
            focused_script,
            theme,
            DesignVariant::Default,
        )
    }

    pub fn with_design(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        Self::with_script_and_design(focus_handle, on_select, None, theme, design_variant)
    }

    pub(crate) fn agent_chat_dialog_config() -> ActionsDialogConfig {
        ActionsDialogConfig {
            search_position: SearchPosition::Top,
            section_style: SectionStyle::Headers,
            anchor: AnchorPosition::Top,
            show_icons: true,
            show_context_header: false,
            ..ActionsDialogConfig::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_actions_with_context(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        mut actions: Vec<Action>,
        focused_script: Option<ScriptInfo>,
        focused_scriptlet: Option<Scriptlet>,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
        context_title: Option<String>,
        config: ActionsDialogConfig,
    ) -> Self {
        let config = actions_dialog_footerless_config(config);
        append_disabled_action_test_fixture(&mut actions);
        let filtered_actions: Vec<usize> = (0..actions.len()).collect();
        let grouped_items =
            build_grouped_items_static(&actions, &filtered_actions, config.section_style);
        let list_state = ListState::new(
            grouped_items.len(),
            ListAlignment::Top,
            px(actions_dialog_list_overdraw_px()),
        );
        let selected_index = initial_selection_index(&grouped_items);

        ActionsDialog {
            actions,
            filtered_actions,
            filtered_title_match_indices: Vec::new(),
            filtered_description_match_indices: Vec::new(),
            selected_index,
            search_text: String::new(),
            search_input: None,
            search_input_subscription: None,
            search_input_syncing: false,
            focus_handle,
            on_select,
            focused_script,
            focused_scriptlet,
            menu_syntax_section: None,
            host_section: None,
            list_state,
            grouped_items,
            theme,
            design_variant,
            cursor_visible: true,
            hide_search: matches!(config.search_position, SearchPosition::Hidden),
            sdk_actions: None,
            sdk_action_indices: Vec::new(),
            context_title,
            default_search_placeholder: config.search_placeholder.clone(),
            config,
            skip_track_focus: false,
            fill_window_bounds: false,
            fixed_shell_height_px: None,
            match_main_window_background: true,
            on_close: None,
            on_activation: None,
            host_context: None,
            route_stack: Vec::new(),
            drill_down_routes: HashMap::new(),
            mouse_armed_row: None,
            hovered_row: None,
            scrollbar_visibility: crate::transitions::Opacity::INVISIBLE,
            scrollbar_fade_gen: 0,
            last_scroll_time: None,
            pending_scrollbar_scroll_top_y: None,
            actions_selection_user_moved: false,
        }
    }

    /// Create ActionsDialog for a path (file/folder) with path-specific actions
    pub fn with_path(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        path_info: &PathInfo,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let actions = get_path_context_actions(path_info);
        let config = ActionsDialogConfig::default();

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created for path: {} (is_dir={}) with {} actions",
                path_info.path,
                path_info.is_dir,
                actions.len()
            ),
        );

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            None,
            None,
            theme,
            DesignVariant::Default,
            Some(path_info.path.clone()),
            config,
        )
    }

    /// Create ActionsDialog for a file search result with file-specific actions
    /// Actions: Open, Reveal in Finder, Quick Look, Open With..., Show Info, Copy Path
    pub fn with_file(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        file_info: &FileInfo,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let actions = get_file_context_actions(file_info);
        let config = ActionsDialogConfig::default();

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created for file: {} (is_dir={}) with {} actions",
                file_info.path,
                file_info.is_dir,
                actions.len()
            ),
        );

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            None,
            None,
            theme,
            DesignVariant::Default,
            Some(file_info.name.clone()),
            config,
        )
    }

    /// Create ActionsDialog for the file-search view, combining selected-row
    /// file actions (when a row is selected) and current-directory actions
    /// (when browsing a concrete directory).
    pub fn with_file_search_context(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        file_info: Option<&FileInfo>,
        dir_info: Option<&crate::actions::FileSearchDirectoryInfo>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let mut actions = Vec::new();

        if let Some(file_info) = file_info {
            actions.extend(get_file_context_actions(file_info));
        }
        if let Some(dir_info) = dir_info {
            actions.extend(crate::actions::builders::get_file_search_directory_actions(
                dir_info,
            ));
        }
        // Run 14 Pass 1 — global actions are a fallback so the file-search
        // Cmd+K dialog never opens empty when the user has not yet selected
        // a file or browsed into a directory. With a file/directory focused,
        // the menu stays contextual to it. Story
        // `actions-debounce-builtins-cross-host-live`.
        if actions.is_empty() {
            actions.extend(crate::actions::builders::get_global_actions());
        }
        clear_duplicate_action_shortcuts(&mut actions);

        let context_title = match (file_info, dir_info) {
            (Some(file), Some(dir)) => Some(format!("{} · in {}", file.name, dir.name)),
            (Some(file), None) => Some(file.name.clone()),
            (None, Some(dir)) => Some(dir.name.clone()),
            (None, None) => None,
        };

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created for file search context: file={}, dir={}, {} actions",
                file_info.map(|f| f.name.as_str()).unwrap_or("none"),
                dir_info.map(|d| d.name.as_str()).unwrap_or("none"),
                actions.len()
            ),
        );

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            None,
            None,
            theme,
            DesignVariant::Default,
            context_title,
            ActionsDialogConfig::default(),
        )
    }

    /// Create ActionsDialog for a clipboard history entry with clipboard-specific actions
    /// Actions: Paste, Copy, Paste and Keep Open, Share, Attach to Agent Chat, Pin/Unpin, Delete, etc.
    pub fn with_clipboard_entry(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        entry_info: &ClipboardEntryInfo,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let actions = get_clipboard_history_context_actions(entry_info);
        let config = ActionsDialogConfig::default();

        let context_title = Self::clipboard_context_title(&entry_info.preview);

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created for clipboard entry: {} (type={:?}, pinned={}) with {} actions",
                entry_info.id,
                entry_info.content_type,
                entry_info.pinned,
                actions.len()
            ),
        );

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            None,
            None,
            theme,
            DesignVariant::Default,
            Some(context_title),
            config,
        )
    }

    /// Create ActionsDialog for an emoji picker entry with emoji-specific actions
    /// Actions: Paste, Copy, Paste and Keep Open, Pin/Unpin, Copy Unicode, Copy Section
    pub fn with_emoji(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        emoji_info: &EmojiActionInfo,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let actions = get_emoji_context_actions(emoji_info);
        let config = ActionsDialogConfig::default();

        let context_title = format!("{} {}", emoji_info.value, emoji_info.name);

        tracing::debug!(
            target: "script_kit::actions",
            emoji = %emoji_info.value,
            name = %emoji_info.name,
            action_count = actions.len(),
            "ActionsDialog created for emoji"
        );

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            None,
            None,
            theme,
            DesignVariant::Default,
            Some(context_title),
            config,
        )
    }

    fn clipboard_context_title(preview: &str) -> String {
        const CONTEXT_TITLE_MAX_CHARS: usize = 30;
        const CONTEXT_TITLE_TRUNCATE_CHARS: usize = 27;

        if preview.chars().count() > CONTEXT_TITLE_MAX_CHARS {
            let truncated: String = preview.chars().take(CONTEXT_TITLE_TRUNCATE_CHARS).collect();
            format!("{truncated}...")
        } else {
            preview.to_string()
        }
    }

    /// Create ActionsDialog for a chat prompt with chat-specific actions.
    ///
    /// Initializes a root route whose first-level actions contain
    /// `chat:change_model` (a drill-down trigger) instead of flat model rows.
    /// Selecting Change Model transitions to a second-level model picker.
    /// Escape pops back to the root route, then dismisses the dialog.
    pub fn with_chat(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        chat_info: &ChatPromptInfo,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let root_route = super::builders::get_chat_root_route(chat_info);
        let config = ActionsDialogConfig::default();

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created for chat prompt: model={:?}, root_actions={}",
                chat_info.current_model,
                root_route.actions.len(),
            ),
        );

        let mut dialog = Self::from_actions_with_context(
            focus_handle,
            on_select,
            root_route.actions.clone(),
            None,
            None,
            theme,
            DesignVariant::Default,
            root_route.context_title.clone(),
            config,
        );

        dialog.set_root_route(root_route);
        dialog.register_drill_down_route(
            super::builders::CHAT_CHANGE_MODEL_ACTION_ID,
            super::builders::get_chat_model_picker_route(chat_info),
        );

        dialog
    }

    pub fn with_script_and_design(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        focused_script: Option<ScriptInfo>,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        let actions = Self::build_actions(&focused_script, &None, &None, &None);
        let config = ActionsDialogConfig::default();

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created with {} actions, script: {:?}, design: {:?}",
                actions.len(),
                focused_script.as_ref().map(|s| &s.name),
                design_variant
            ),
        );

        // Log theme color configuration for debugging
        logging::log(
            "ACTIONS_THEME",
            &format!(
                "Theme colors applied: bg_main=#{:06x}, bg_search=#{:06x}, text_primary=#{:06x}, accent_selected=#{:06x}",
                theme.colors.background.main,
                theme.colors.background.search_box,
                theme.colors.text.primary,
                theme.colors.accent.selected
            ),
        );

        // Extract context title from focused script if available
        let context_title = focused_script.as_ref().map(|s| s.name.clone());

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            focused_script,
            None,
            theme,
            design_variant,
            context_title,
            config,
        )
    }

    /// Update cursor visibility (called from parent's blink timer)
    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    /// Hide the search input (for inline mode where header has search)
    pub fn set_hide_search(&mut self, hide: bool) {
        self.hide_search = hide;
    }

    /// Set the context title shown in the header
    pub fn set_context_title(&mut self, title: Option<String>) {
        self.context_title = title;
    }

    /// Set the configuration for appearance and behavior.
    ///
    /// When a route is active, route-owned `context_title` and `search_placeholder`
    /// are preserved so host config updates don't clobber them.
    pub fn set_config(&mut self, config: ActionsDialogConfig) {
        let config = actions_dialog_footerless_config(config);
        let should_rebuild = should_rebuild_grouped_items_for_config_change(&self.config, &config);
        let previously_selected_action_id = self.get_selected_action_id();

        self.config = config;
        self.default_search_placeholder = self.config.search_placeholder.clone();
        // Update hide_search based on config for backwards compatibility
        self.hide_search = matches!(self.config.search_position, SearchPosition::Hidden);

        // Preserve route-owned shell state over host defaults
        if let Some(state) = self.route_stack.last() {
            if state.route.context_title.is_some() {
                self.context_title = state.route.context_title.clone();
            }
            self.config.search_placeholder = state
                .route
                .search_placeholder
                .clone()
                .or_else(|| self.default_search_placeholder.clone());
        }

        if should_rebuild {
            self.rebuild_grouped_items();
            self.apply_refresh_selection(previously_selected_action_id.as_deref());
            if !self.grouped_items.is_empty() {
                self.scroll_to_selected();
            }
        }

        tracing::info!(
            target: "script_kit::actions",
            route_id = ?self.current_route_id(),
            route_depth = self.route_depth(),
            search_placeholder = ?self.current_search_placeholder(),
            "actions_dialog_config_applied"
        );
    }

    /// Set skip_track_focus to let parent handle focus (used by ActionsWindow)
    pub fn set_skip_track_focus(&mut self, skip: bool) {
        self.skip_track_focus = skip;
    }

    /// Align the dialog background alpha with the main window vibrancy tint.
    pub fn set_match_main_window_background(&mut self, match_main_window_background: bool) {
        self.match_main_window_background = match_main_window_background;
    }

    /// Set the callback for row-click activations that need parent window handling.
    pub(crate) fn set_on_activation(&mut self, callback: ActivationCallback) {
        self.on_activation = Some(callback);
    }

    pub(crate) fn on_activation_callback(&self) -> Option<ActivationCallback> {
        self.on_activation.clone()
    }

    pub(crate) fn set_host_context(&mut self, context: ActionsHostContextSnapshot) {
        self.host_context = Some(context);
    }

    pub(crate) fn host_context(&self) -> Option<&ActionsHostContextSnapshot> {
        self.host_context.as_ref()
    }

    /// Set the callback for when the dialog is closed (escape pressed, window dismissed)
    /// Used to notify the main app to restore focus
    pub fn set_on_close(&mut self, callback: CloseCallback) {
        self.on_close = Some(callback);
    }

    /// Call the on_close callback if set
    /// Returns true if a callback was called, false otherwise
    pub fn trigger_on_close(&self, cx: &mut gpui::App) -> bool {
        if let Some(ref callback) = self.on_close {
            callback(cx);
            true
        } else {
            false
        }
    }

    // ── Route / back-stack methods ─────────────────────────────────────────

    /// Replace the route stack with a single root route.
    pub fn set_root_route(&mut self, route: ActionsDialogRoute) {
        self.route_stack.clear();
        self.route_stack
            .push(ActionsDialogRouteState::new(route.clone()));
        self.apply_route_state_from_route(&route);
    }

    /// Prepend host-owned actions to the root route without replacing Agent Chat's
    /// route stack or drill-down registrations.
    pub(crate) fn prepend_root_route_actions(&mut self, mut actions: Vec<Action>) {
        if actions.is_empty() {
            return;
        }
        let root_route = {
            let Some(root) = self.route_stack.first_mut() else {
                return;
            };
            actions.append(&mut root.route.actions);
            root.route.actions = actions;
            root.route.clone()
        };

        if self.route_stack.len() == 1 {
            let route = root_route;
            self.apply_route_state_from_route(&route);
        }
    }

    /// The ID of the topmost route on the stack, if any.
    pub fn current_route_id(&self) -> Option<&str> {
        self.route_stack.last().map(|s| s.route.id.as_str())
    }

    /// The search placeholder currently active (route override or default).
    pub fn current_search_placeholder(&self) -> Option<&str> {
        self.config.search_placeholder.as_deref()
    }

    /// Number of routes on the stack (0 = no route navigation).
    pub fn route_depth(&self) -> usize {
        self.route_stack.len()
    }

    /// Whether a pop would return to a parent route (vs. closing).
    pub fn can_pop_route(&self) -> bool {
        self.route_stack.len() > 1
    }

    /// Register a drill-down route that is pushed when the given action ID
    /// is selected via `activate_selected`.
    pub fn register_drill_down_route(
        &mut self,
        action_id: impl Into<String>,
        route: ActionsDialogRoute,
    ) {
        self.drill_down_routes.insert(action_id.into(), route);
    }

    /// Snapshot current search text and selection into the topmost route state.
    fn snapshot_current_route_state(&mut self) {
        let selected_action_id = self.get_selected_action_id();
        if let Some(state) = self.route_stack.last_mut() {
            state.search_text = self.search_text.clone();
            state.selected_action_id = selected_action_id;
        }
    }

    /// Push a child route onto the stack, preserving the parent's UI state.
    pub fn push_route(&mut self, route: ActionsDialogRoute, cx: &mut Context<Self>) {
        self.snapshot_current_route_state();
        let route_id = route.id.clone();
        self.route_stack
            .push(ActionsDialogRouteState::new(route.clone()));
        self.apply_route_state_from_route(&route);
        tracing::info!(
            target: "script_kit::actions",
            route_id = %route_id,
            depth = self.route_stack.len(),
            "actions_dialog_route_push"
        );
        cx.notify();
    }

    /// Pop the topmost route, restoring the parent route's UI state.
    /// Returns `false` if already at root (nothing to pop).
    pub fn pop_route(&mut self, cx: &mut Context<Self>) -> bool {
        if self.route_stack.len() <= 1 {
            return false;
        }
        self.snapshot_current_route_state();
        self.route_stack.pop();
        let Some(state) = self.route_stack.last().cloned() else {
            return false;
        };
        let route_id = state.route.id.clone();
        self.apply_route_state(&state, cx);
        tracing::info!(
            target: "script_kit::actions",
            route_id = %route_id,
            depth = self.route_stack.len(),
            "actions_dialog_route_pop"
        );
        cx.notify();
        true
    }

    /// Handle Escape with back-stack semantics: pop a child route, or signal close.
    pub fn handle_escape(&mut self, cx: &mut Context<Self>) -> ActionsDialogEscapeOutcome {
        let outcome = if self.pop_route(cx) {
            ActionsDialogEscapeOutcome::PoppedRoute
        } else {
            ActionsDialogEscapeOutcome::CloseDialog
        };
        tracing::info!(
            target: "script_kit::actions",
            ?outcome,
            depth = self.route_stack.len(),
            "actions_dialog_escape"
        );
        outcome
    }

    /// Handle Enter / row-click: drill down if the selected action has a
    /// registered route, otherwise execute it via `on_select`.
    pub fn activate_selected(&mut self, cx: &mut Context<Self>) -> ActionsDialogActivation {
        let Some(action_id) = self.get_selected_action_id() else {
            tracing::info!(
                target: "script_kit::actions",
                outcome = "no_selection",
                depth = self.route_stack.len(),
                "actions_dialog_activation"
            );
            return ActionsDialogActivation::NoSelection;
        };

        self.activate_action_id(action_id, cx)
    }

    pub(crate) fn activate_action_id(
        &mut self,
        action_id: String,
        cx: &mut Context<Self>,
    ) -> ActionsDialogActivation {
        let Some(action) = self.actions.iter().find(|action| action.id == action_id) else {
            tracing::info!(
                target: "script_kit::actions",
                action_id = %action_id,
                outcome = "missing_action",
                "actions_dialog_activation"
            );
            return ActionsDialogActivation::NoSelection;
        };
        if let Some(reason) = action.disabled_reason() {
            let reason = reason.to_string();
            tracing::info!(
                target: "script_kit::actions",
                action_id = %action_id,
                reason_fingerprint = %Self::devtools_text_fingerprint(&reason),
                outcome = "blocked",
                "actions_dialog_activation"
            );
            return ActionsDialogActivation::Blocked { action_id, reason };
        }

        // Check for a registered drill-down route
        if let Some(route) = self.drill_down_routes.get(&action_id).cloned() {
            let route_id = route.id.clone();
            self.push_route(route, cx);
            tracing::info!(
                target: "script_kit::actions",
                action_id = %action_id,
                route_id = %route_id,
                outcome = "drill_down",
                depth = self.route_stack.len(),
                "actions_dialog_activation"
            );
            return ActionsDialogActivation::DrillDownPushed {
                action_id,
                route_id,
            };
        }

        let should_close = self.action_should_close(&action_id);
        (self.on_select)(action_id.clone());
        tracing::info!(
            target: "script_kit::actions",
            action_id = %action_id,
            should_close,
            outcome = "executed",
            depth = self.route_stack.len(),
            "actions_dialog_activation"
        );
        ActionsDialogActivation::Executed {
            action_id,
            should_close,
        }
    }

    /// Try to select an action by ID without requiring `cx`.
    /// Returns `true` if the action was found and selected.
    pub(super) fn restore_selected_action_id(&mut self, action_id: &str) -> bool {
        let Some(action_index) = self
            .filtered_actions
            .iter()
            .position(|&idx| self.actions.get(idx).is_some_and(|a| a.id == action_id))
        else {
            return false;
        };
        let Some(grouped_index) = self
            .grouped_items
            .iter()
            .position(|item| matches!(item, GroupedActionItem::Item(fi) if *fi == action_index))
        else {
            return false;
        };
        self.selected_index = Some(grouped_index);
        self.clear_mouse_submit_arm();
        true
    }

    pub(super) fn apply_refresh_selection(&mut self, previously_selected_action_id: Option<&str>) {
        let policy = actions_refresh_selection_policy(self.actions_selection_user_moved);
        let previous_visual_index = self.selected_index;
        let restored = matches!(policy, ActionsRefreshSelectionPolicy::RestoreIdentity)
            && previously_selected_action_id
                .map(|id| self.restore_selected_action_id(id))
                .unwrap_or(false);
        if !restored {
            self.selected_index = match (policy, previous_visual_index) {
                (ActionsRefreshSelectionPolicy::RestoreIdentity, Some(index)) => {
                    coerce_action_selection(&self.grouped_items, index)
                        .or_else(|| initial_selection_index(&self.grouped_items))
                }
                _ => initial_selection_index(&self.grouped_items),
            };
            self.clear_mouse_submit_arm();
        }

        tracing::info!(
            target: "script_kit::actions",
            ?policy,
            restored,
            selected_action_id = ?self.get_selected_action_id(),
            "actions_dialog_refresh_selection_applied"
        );
    }

    /// Apply a route's actions/title/placeholder to the live dialog (no state restore).
    fn apply_route_state_from_route(&mut self, route: &ActionsDialogRoute) {
        self.actions = route.actions.clone();
        append_disabled_action_test_fixture(&mut self.actions);
        self.reset_filter_to_all();
        self.search_text.clear();
        self.context_title = route.context_title.clone();
        self.config.search_placeholder = route
            .search_placeholder
            .clone()
            .or_else(|| self.default_search_placeholder.clone());
        self.sdk_actions = None;
        self.sdk_action_indices.clear();
        self.rebuild_grouped_items();

        self.actions_selection_user_moved = false;
        self.selected_index = initial_selection_index(&self.grouped_items);
        self.clear_mouse_submit_arm();

        if !self.grouped_items.is_empty() {
            self.scroll_to_selected();
        }
    }

    /// Restore a full route state snapshot (search text + selection).
    fn apply_route_state(&mut self, state: &ActionsDialogRouteState, _cx: &mut Context<Self>) {
        self.actions = state.route.actions.clone();
        append_disabled_action_test_fixture(&mut self.actions);
        self.reset_filter_to_all();
        self.search_text = state.search_text.clone();
        self.context_title = state.route.context_title.clone();
        self.config.search_placeholder = state
            .route
            .search_placeholder
            .clone()
            .or_else(|| self.default_search_placeholder.clone());
        self.sdk_actions = None;
        self.sdk_action_indices.clear();
        self.refilter();

        let restored = state
            .selected_action_id
            .as_deref()
            .map(|id| self.restore_selected_action_id(id))
            .unwrap_or(false);
        if !restored {
            self.selected_index = initial_selection_index(&self.grouped_items);
            self.clear_mouse_submit_arm();
        }

        if !self.grouped_items.is_empty() {
            self.scroll_to_selected();
        }
    }

    /// Hint label for the footer: "Esc Back" when a parent route exists,
    /// otherwise "Esc Close".
    pub fn route_hint_label(&self) -> &'static str {
        if self.can_pop_route() {
            "Esc Back"
        } else {
            "Esc Close"
        }
    }

    // ── Agent Chat chat constructor ─────────────────────────────────────────────

    /// Create an ActionsDialog pre-configured for Agent Chat Chat with route-based
    /// drill-down entries for agent and model changes.
    /// Accepts an explicit host so that detached Agent Chat can filter unsupported actions.
    pub(crate) fn with_agent_chat_for_host(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        context: AgentChatActionsDialogContext<'_>,
        theme: Arc<theme::Theme>,
        host: super::builders::AgentChatActionsDialogHost,
    ) -> Self {
        let root_route = if context.focused_text {
            super::builders::get_focused_text_agent_chat_root_route(context.focused_text_expanded)
        } else {
            super::builders::get_agent_chat_root_route_for_host(
                context.available_models,
                context.selected_model_id,
                context.standing_approval_count,
                context.thread_summaries,
                context.fork_points,
                host,
            )
        };
        let config = Self::agent_chat_dialog_config();

        let mut dialog = Self::from_actions_with_context(
            focus_handle,
            on_select,
            root_route.actions.clone(),
            None,
            None,
            theme,
            DesignVariant::Default,
            root_route.context_title.clone(),
            config,
        );

        dialog.set_root_route(root_route);
        dialog.register_drill_down_route(
            super::builders::AGENT_CHAT_CHANGE_PROFILE_ACTION_ID,
            super::builders::get_agent_chat_profile_picker_route_for_host(host),
        );
        dialog.register_drill_down_route(
            super::builders::AGENT_CHAT_CHANGE_MODEL_ACTION_ID,
            super::builders::get_agent_chat_model_picker_route_for_host(
                context.available_models,
                context.selected_model_id,
                host,
            ),
        );
        dialog.register_drill_down_route(
            super::builders::AGENT_CHAT_SHOW_RECEIPT_HISTORY_ACTION_ID,
            crate::actions::get_agent_chat_receipt_history_route(),
        );
        if !context.fork_points.is_empty() {
            dialog.register_drill_down_route(
                super::builders::AGENT_CHAT_REWIND_ACTION_ID,
                super::builders::get_agent_chat_fork_picker_route_for_host(
                    context.fork_points,
                    host,
                ),
            );
        }
        if matches!(
            host,
            super::builders::AgentChatActionsDialogHost::Detached
                | super::builders::AgentChatActionsDialogHost::Notes
        ) {
            dialog.register_drill_down_route(
                "agent_chat_show_history",
                crate::actions::get_agent_chat_history_route(),
            );
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_actions_menu_routes_registered",
            host = ?host,
            has_models = !context.available_models.is_empty(),
            model_count = context.available_models.len(),
            "Registered Agent Chat Actions Menu drill-down routes"
        );

        dialog
    }

    /// Create an ActionsDialog pre-configured for Agent Chat Chat (shared host).
    pub(crate) fn with_agent_chat(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        context: AgentChatActionsDialogContext<'_>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_agent_chat_for_host(
            focus_handle,
            on_select,
            context,
            theme,
            super::builders::AgentChatActionsDialogHost::Shared,
        )
    }

    /// Create ActionsDialog with custom configuration and actions
    ///
    /// Use this for contexts like AI chat that need different appearance:
    /// - Canonical top search or intentionally hidden search
    /// - Section headers instead of separators
    /// - Icons next to actions
    pub fn with_config(
        focus_handle: FocusHandle,
        on_select: ActionCallback,
        actions: Vec<Action>,
        theme: Arc<theme::Theme>,
        config: ActionsDialogConfig,
    ) -> Self {
        let filtered_actions_preview: Vec<usize> = (0..actions.len()).collect();
        let grouped_items_preview =
            build_grouped_items_static(&actions, &filtered_actions_preview, config.section_style);
        let initial_selection = initial_selection_index(&grouped_items_preview);

        logging::log(
            "ACTIONS",
            &format!(
                "ActionsDialog created with config: {} actions, search={:?}, section_style={:?}, initial_selection={:?}",
                actions.len(),
                config.search_position,
                config.section_style,
                initial_selection
            ),
        );

        Self::from_actions_with_context(
            focus_handle,
            on_select,
            actions,
            None,
            None,
            theme,
            DesignVariant::Default,
            None,
            config,
        )
    }

    /// Resolve a shortcut into canonical keycap tokens.
    ///
    /// Actions owns no parser: multi-character keys and literal punctuation
    /// follow the same token stream used by rows, buttons, recorders, and
    /// native/GPUI footers.
    pub(crate) fn parse_shortcut_keycaps(shortcut: &str) -> Vec<String> {
        crate::components::hint_strip::shortcut_tokens_from_hint(shortcut)
    }
}

#[cfg(test)]
mod unicode_keycap_safety_tests {
    use super::ActionsDialog;

    #[test]
    fn test_clipboard_context_title_does_not_panic_when_preview_contains_multibyte_unicode() {
        let preview = "😀".repeat(31);
        let title = ActionsDialog::clipboard_context_title(&preview);

        assert_eq!(title.chars().count(), 30);
        assert_eq!(title, format!("{}...", "😀".repeat(27)));
    }

    #[test]
    fn test_parse_shortcut_keycaps_does_not_panic_when_shortcut_contains_multibyte_unicode() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘😀");
        let expected = vec!["⌘".to_string(), "😀".to_string()];

        assert_eq!(keycaps, expected);
    }
}

// --- merged from part_02.rs ---
impl ActionsDialog {
    fn devtools_rect(x: f32, y: f32, width: f32, height: f32) -> serde_json::Value {
        serde_json::json!({
            "x": x,
            "y": y,
            "width": width,
            "height": height,
        })
    }

    fn devtools_row_geometry(&self, cx: &gpui::App) -> serde_json::Value {
        let mut style = actions_dialog_default_style();
        style.row_height = self.effective_row_height();
        let attached_popup_generation = crate::actions::actions_popup_automation_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .get("generation")
                    .and_then(serde_json::Value::as_u64)
            });
        let show_search = self.search_is_visible();
        let popup_theme = crate::designs::current_actions_popup_theme();
        let search_height = if show_search {
            popup_theme.search.height
        } else {
            0.0
        };
        let header_height = if self.shows_context_header() && style.show_header {
            popup_theme.context_header.height
        } else {
            0.0
        };
        let list_top = search_height + header_height;

        let mut raw_content_height = 0.0;
        let mut action_count = 0_usize;
        let mut section_count = 0_usize;
        for item in &self.grouped_items {
            match item {
                GroupedActionItem::SectionHeader(_) => {
                    section_count += 1;
                    raw_content_height += popup_theme.list.section_header_height;
                }
                GroupedActionItem::Item(_) => {
                    action_count += 1;
                    raw_content_height += style.row_height;
                }
            }
        }

        let content_height =
            raw_content_height.max(self.min_scroll_content_height(style.row_height));
        let dynamic_shell_height = super::window::actions_window_dynamic_height(
            action_count,
            section_count,
            !show_search,
            header_height > 0.0,
            self.config.show_footer,
            self.config.max_height,
            style.row_height,
        );
        let shell_height = self.fixed_shell_height_px.unwrap_or(dynamic_shell_height);
        let metrics = self.scrollbar_metrics(style.row_height);
        let viewport_height = metrics.viewport_height;
        let scroll_top_item_index = self.list_state.logical_scroll_top().item_ix;
        let live_scroll_top_content_y = self.live_scroll_top_y(style.row_height);
        let scroll_top_content_y = metrics.scroll_top_y;

        let viewport_bottom = list_top + viewport_height;
        let mut content_y = 0.0;
        let mut rows = Vec::new();
        let mut sections = Vec::new();
        let mut shortcut_layout_rows = Vec::new();
        let mut selected_row = None;
        let marker_metrics = resolved_actions_dialog_row_metrics(
            &popup_theme,
            crate::designs::current_main_menu_theme().def(),
        );

        for (visual_index, item) in self.grouped_items.iter().enumerate() {
            let section_presentation = match item {
                GroupedActionItem::SectionHeader(label) => {
                    Some(crate::list_item::resolve_section_header_presentation(
                        label,
                        None,
                        None,
                        crate::list_item::SectionPresentationFamily::Launcher,
                    ))
                }
                GroupedActionItem::Item(_) => None,
            };
            let (kind, height, label, action_id, shortcut, shortcut_tokens) = match item {
                GroupedActionItem::SectionHeader(label) => (
                    "section",
                    popup_theme.list.section_header_height,
                    Some(label.as_str()),
                    None,
                    None,
                    Vec::<String>::new(),
                ),
                GroupedActionItem::Item(filter_idx) => {
                    let action = self
                        .filtered_actions
                        .get(*filter_idx)
                        .and_then(|action_idx| self.actions.get(*action_idx));
                    let display_shortcut = action.is_some_and(|action| {
                        action_has_routable_shortcut(
                            &self.actions,
                            &self.filtered_actions,
                            &action.id,
                        )
                    });
                    (
                        "action",
                        style.row_height,
                        action.map(|action| action.title.as_str()),
                        action.map(|action| action.id.as_str()),
                        action
                            .filter(|_| display_shortcut)
                            .and_then(|action| action.shortcut.as_deref()),
                        action
                            .filter(|_| display_shortcut)
                            .and_then(|action| {
                                action_shortcut_tokens_for_render(action)
                                    .map(|tokens| tokens.iter().cloned().collect::<Vec<_>>())
                            })
                            .unwrap_or_default(),
                    )
                }
            };
            let action_for_row = match item {
                GroupedActionItem::Item(filter_idx) => self
                    .filtered_actions
                    .get(*filter_idx)
                    .and_then(|action_idx| self.actions.get(*action_idx)),
                GroupedActionItem::SectionHeader(_) => None,
            };
            let eligibility = action_for_row
                .map(action_row_eligibility)
                .unwrap_or_else(crate::list_item::RowEligibility::inert);
            let enabled = action_for_row.is_none_or(Action::is_enabled);
            let disabled_reason = action_for_row.and_then(Action::disabled_reason);
            let viewport_y = list_top + content_y - scroll_top_content_y;
            let rect = Self::devtools_rect(0.0, viewport_y, POPUP_WIDTH, height);
            let inner_rect = if kind == "action" {
                Self::devtools_rect(
                    ACTION_ROW_INSET,
                    viewport_y,
                    POPUP_WIDTH - (ACTION_ROW_INSET * 2.0),
                    height,
                )
            } else {
                Self::devtools_rect(0.0, viewport_y, POPUP_WIDTH, height)
            };
            let shortcut_layout = if kind == "action" && !shortcut_tokens.is_empty() {
                let probe =
                    crate::components::footer_chrome::footer_shortcut_keycap_layout_model_measured(
                        shortcut_tokens.iter().map(String::as_str),
                        0.0,
                        0.0,
                        cx,
                    );
                let shortcut_bounds = probe.get("bounds");
                let shortcut_width = shortcut_bounds
                    .and_then(|bounds| bounds.get("width"))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let shortcut_height = shortcut_bounds
                    .and_then(|bounds| bounds.get("height"))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let inner_x = ACTION_ROW_INSET;
                let inner_y = viewport_y;
                let inner_width = POPUP_WIDTH - (ACTION_ROW_INSET * 2.0);
                let inner_height = height;
                let shortcut_x = inner_x + (inner_width - shortcut_width).max(0.0);
                let shortcut_y = inner_y + ((inner_height - shortcut_height).max(0.0) / 2.0);
                let layout =
                    crate::components::footer_chrome::footer_shortcut_keycap_layout_model_measured(
                        shortcut_tokens.iter().map(String::as_str),
                        shortcut_x,
                        shortcut_y,
                        cx,
                    );
                let row_layout = serde_json::json!({
                    "visualIndex": visual_index,
                    "actionId": action_id,
                    "shortcut": shortcut,
                    "shortcutTokens": shortcut_tokens.clone(),
                    "layout": layout,
                    "rightAligned": true,
                    "clipped": shortcut_width > inner_width,
                });
                shortcut_layout_rows.push(row_layout.clone());
                Some(row_layout)
            } else {
                None
            };
            let visible = viewport_y + height > list_top && viewport_y < viewport_bottom;
            let is_selected = self.selected_index == Some(visual_index);
            let selection_marker_bounds = (kind == "action")
                .then(|| {
                    crate::list_item::list_item_selection_marker_geometry(
                        marker_metrics,
                        is_selected,
                    )
                })
                .flatten()
                .map(|marker| {
                    Self::devtools_rect(
                        ACTION_ROW_INSET + marker.left,
                        viewport_y + marker.top,
                        marker.width,
                        marker.height,
                    )
                });
            let row = serde_json::json!({
                "semanticId": if kind == "action" {
                    action_id.map(|id| format!("choice:{visual_index}:{id}"))
                } else {
                    Some(format!("section:{visual_index}"))
                },
                "visualIndex": visual_index,
                "groupedIndex": visual_index,
                "kind": kind,
                "labelLength": label.map(|value| value.chars().count()),
                "labelFingerprint": label.map(Self::devtools_text_fingerprint),
                "semanticLabel": section_presentation
                    .as_ref()
                    .map(|presentation| presentation.semantic_label.as_ref()),
                "displayLabel": section_presentation
                    .as_ref()
                    .map(|presentation| presentation.display_label.as_ref()),
                "labelTier": section_presentation
                    .as_ref()
                    .map(|presentation| presentation.label_tier.as_str()),
                "countTier": section_presentation
                    .as_ref()
                    .map(|presentation| presentation.count_tier.as_str()),
                "iconTier": section_presentation
                    .as_ref()
                    .map(|presentation| presentation.icon_tier.as_str()),
                "actionId": action_id,
                "enabled": enabled,
                "disabledReason": disabled_reason,
                "focusable": eligibility.focusable,
                "selectable": eligibility.selectable,
                "activatable": eligibility.activatable,
                "selected": is_selected,
                "selectionMarkerBounds": selection_marker_bounds,
                "hovered": self.hovered_row == Some(visual_index),
                "mouseArmed": self.mouse_armed_row == Some(visual_index),
                "visible": visible,
                "clipped": !visible,
                "contentY": content_y,
                "bounds": rect.clone(),
                "rect": rect,
                "innerBounds": inner_rect.clone(),
                "innerRect": inner_rect,
                "shortcut": shortcut,
                "shortcutTokens": shortcut_tokens.clone(),
                "shortcutBoundsAvailable": shortcut_layout.is_some(),
                "shortcutBounds": shortcut_layout
                    .as_ref()
                    .and_then(|layout| layout.get("layout"))
                    .and_then(|layout| layout.get("bounds"))
                    .cloned(),
                "shortcutLayout": shortcut_layout,
                "disabledReasonBoundsAvailable": disabled_reason.is_some(),
                "disabledReasonBounds": disabled_reason.map(|_| Self::devtools_rect(
                    ACTION_ROW_INSET + (POPUP_WIDTH - (ACTION_ROW_INSET * 2.0) - 132.0).max(0.0),
                    viewport_y,
                    132.0,
                    height,
                )),
            });

            if kind == "section" {
                sections.push(row.clone());
            }
            if is_selected {
                selected_row = Some(row.clone());
            }
            rows.push(row);
            content_y += height;
        }
        let hovered_row = self.hovered_row.and_then(|hovered_index| {
            rows.iter()
                .find(|row| {
                    row.get("visualIndex").and_then(serde_json::Value::as_u64)
                        == Some(hovered_index as u64)
                })
                .cloned()
        });
        let mouse_armed_row = self.mouse_armed_row.and_then(|armed_index| {
            rows.iter()
                .find(|row| {
                    row.get("visualIndex").and_then(serde_json::Value::as_u64)
                        == Some(armed_index as u64)
                })
                .cloned()
        });

        serde_json::json!({
            "schemaVersion": 1,
            "available": true,
            "source": "runtime.actionsDialog.render",
            "generation": attached_popup_generation.unwrap_or(0),
            "attachedPopupGeneration": attached_popup_generation,
            "stale": false,
            "measurementSource": "runtime.actionsDialog.render",
            "coordinateSpace": "popupLogicalPx",
            "units": "logicalPx",
            "ownerWindowId": "actions-dialog",
            "stopReason": null,
            "quality": "model",
            "popupWidth": POPUP_WIDTH,
            "shellHeight": shell_height,
            "fixedShellHeight": self.fixed_shell_height_px,
            "listViewportRect": Self::devtools_rect(0.0, list_top, POPUP_WIDTH, viewport_height),
            "viewport": {
                "containerBounds": Self::devtools_rect(0.0, 0.0, POPUP_WIDTH, shell_height),
                "searchBounds": if show_search {
                    Some(Self::devtools_rect(
                        0.0,
                        0.0,
                        POPUP_WIDTH,
                        search_height,
                    ))
                } else {
                    None
                },
                "contextHeaderBounds": if header_height > 0.0 {
                    Some(Self::devtools_rect(
                        0.0,
                        search_height,
                        POPUP_WIDTH,
                        header_height,
                    ))
                } else {
                    None
                },
                "listBounds": Self::devtools_rect(0.0, list_top, POPUP_WIDTH, viewport_height),
                "visibleRange": {
                    "firstGroupedIndex": rows.iter().find(|row| row.get("visible").and_then(serde_json::Value::as_bool) == Some(true)).and_then(|row| row.get("groupedIndex")).cloned(),
                    "lastGroupedIndex": rows.iter().rev().find(|row| row.get("visible").and_then(serde_json::Value::as_bool) == Some(true)).and_then(|row| row.get("groupedIndex")).cloned(),
                },
                "scroll": {
                    "topGroupedIndex": scroll_top_item_index,
                    "pixelOffsetAvailable": true,
                    "pixelOffset": scroll_top_content_y,
                    "livePixelOffset": live_scroll_top_content_y,
                    "pendingPixelOffset": self.pending_scrollbar_scroll_top_y,
                },
            },
            "contentHeight": content_height,
            "scrollTopItemIndex": scroll_top_item_index,
            "scrollTopContentY": scroll_top_content_y,
            "scrollbar": {
                "contentHeight": metrics.content_height,
                "viewportHeight": metrics.viewport_height,
                "scrollTopY": metrics.scroll_top_y,
                "maxScrollTopY": metrics.max_scroll_top_y,
                "thumbHeightPx": metrics.thumb_height_px,
                "thumbTopPx": metrics.thumb_top_px,
                "thumbPositionRatio": metrics.thumb_position_ratio,
                "visible": metrics.should_show(),
            },
            "sectionBoundsAvailable": true,
            "rowBoundsAvailable": true,
            "selectedRowBoundsAvailable": selected_row.is_some(),
            "hoverRowAvailable": true,
            "hoveredRow": {
                "available": true,
                "state": if hovered_row.is_some() { "hovered" } else { "none" },
                "row": hovered_row,
            },
            "mouseArmedRowAvailable": true,
            "mouseArmedRow": {
                "available": true,
                "state": if mouse_armed_row.is_some() { "armed" } else { "none" },
                "row": mouse_armed_row,
            },
            "shortcutBoundsAvailable": true,
            "disabledReasonBoundsAvailable": false,
            "shortcutLayout": {
                "boundsAvailable": true,
                "rowCount": shortcut_layout_rows.len(),
                "rows": shortcut_layout_rows,
                "stopReason": null,
                "measurementSource": crate::components::footer_chrome::FOOTER_SHORTCUT_LAYOUT_MEASUREMENT_SOURCE,
            },
            "disabledReasonLayout": {
                "status": "no-visible-disabled-reasons",
                "boundsAvailable": false,
                "rowCount": 0,
                "rows": [],
            },
            "warnings": [],
            "counts": {
                "rows": rows.len(),
                "actions": action_count,
                "sections": section_count,
            },
            "selectedRow": selected_row,
            "sections": sections,
            "rows": rows,
        })
    }

    fn update_hovered_row_from_popup_y(&mut self, popup_y: f32, cx: &mut Context<Self>) {
        let mut style = actions_dialog_default_style();
        style.row_height = self.effective_row_height();
        let show_search = self.search_is_visible();
        let popup_theme = crate::designs::current_actions_popup_theme();
        let search_height = if show_search {
            popup_theme.search.height
        } else {
            0.0
        };
        let header_height = if self.shows_context_header() && style.show_header {
            popup_theme.context_header.height
        } else {
            0.0
        };
        let list_top = search_height + header_height;
        let mut content_y = popup_y - list_top;
        if content_y < 0.0 {
            if self.hovered_row.take().is_some() {
                cx.notify();
            }
            return;
        }

        let scroll_top_content_y = self.effective_scroll_top_y(style.row_height);
        content_y += scroll_top_content_y;

        let mut cursor_y = 0.0;
        let mut next_hovered_row = None;
        for (visual_index, item) in self.grouped_items.iter().enumerate() {
            let height = match item {
                GroupedActionItem::SectionHeader(_) => popup_theme.list.section_header_height,
                GroupedActionItem::Item(_) => style.row_height,
            };
            if content_y >= cursor_y && content_y < cursor_y + height {
                if matches!(item, GroupedActionItem::Item(_)) {
                    next_hovered_row = Some(visual_index);
                }
                break;
            }
            cursor_y += height;
        }

        if self.hovered_row != next_hovered_row {
            self.hovered_row = next_hovered_row;
            cx.notify();
        }
    }

    pub(crate) fn devtools_text_fingerprint(value: &str) -> String {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("fnv1a64:{hash:016x}")
    }

    fn devtools_action_summary(action: &Action, shortcut_routable: bool) -> serde_json::Value {
        let eligibility = action_row_eligibility(action);
        let displayed_shortcut = shortcut_routable
            .then_some(action.shortcut.as_deref())
            .flatten();
        let canonical_shortcut =
            displayed_shortcut.map(crate::components::hint_strip::canonical_shortcut_hint);
        serde_json::json!({
            "id": action.id.as_str(),
            "titleLength": action.title.chars().count(),
            "titleFingerprint": Self::devtools_text_fingerprint(&action.title),
            "descriptionLength": action.description.as_ref().map(|value| value.chars().count()),
            "descriptionFingerprint": action.description.as_ref().map(|value| Self::devtools_text_fingerprint(value)),
            "section": action.section.as_deref(),
            "category": format!("{:?}", action.category),
            "enabled": action.is_enabled(),
            "disabledReason": action.disabled_reason(),
            "focusable": eligibility.focusable,
            "selectable": eligibility.selectable,
            "activatable": eligibility.activatable,
            "hasShortcut": displayed_shortcut.is_some(),
            "shortcut": displayed_shortcut,
            "canonicalShortcut": canonical_shortcut,
            "hasAction": action.has_action,
        })
    }

    fn devtools_rect_field(rect: &serde_json::Value, field: &str) -> f32 {
        rect.get(field)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32
    }

    fn devtools_layout_component_from_rect(
        name: impl Into<String>,
        component_type: crate::protocol::LayoutComponentType,
        rect: &serde_json::Value,
    ) -> crate::protocol::LayoutComponentInfo {
        crate::protocol::LayoutComponentInfo::new(name, component_type).with_bounds(
            Self::devtools_rect_field(rect, "x"),
            Self::devtools_rect_field(rect, "y"),
            Self::devtools_rect_field(rect, "width"),
            Self::devtools_rect_field(rect, "height"),
        )
    }

    pub(crate) fn automation_layout_info(
        &self,
        target: &crate::protocol::AutomationWindowInfo,
        cx: &gpui::App,
    ) -> crate::protocol::LayoutInfo {
        use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
        use crate::ui::chrome as chrome_tokens;

        let row_geometry = self.devtools_row_geometry(cx);
        let viewport = row_geometry
            .get("viewport")
            .and_then(serde_json::Value::as_object);
        let container_bounds = viewport
            .and_then(|viewport| viewport.get("containerBounds"))
            .cloned()
            .unwrap_or_else(|| Self::devtools_rect(0.0, 0.0, POPUP_WIDTH, 0.0));
        let target_bounds = target.bounds.as_ref();
        let window_width = target_bounds
            .map(|bounds| bounds.width as f32)
            .unwrap_or_else(|| Self::devtools_rect_field(&container_bounds, "width"));
        let window_height = target_bounds
            .map(|bounds| bounds.height as f32)
            .unwrap_or_else(|| Self::devtools_rect_field(&container_bounds, "height"));
        let mut components = Vec::new();

        components.push(
            LayoutComponentInfo::new("ActionsDialog", LayoutComponentType::Container)
                .with_bounds(0.0, 0.0, window_width, window_height)
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FLOATING,
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                    Some(ACTIONS_POPUP_RADIUS),
                )
                .with_visual_token("chrome.actionsDialog")
                .with_flex_column()
                .with_depth(0)
                .with_explanation(
                    "Actions dialog popup root measured from the registered automation target bounds.",
                ),
        );

        if let Some(search_bounds) = viewport
            .and_then(|viewport| viewport.get("searchBounds"))
            .filter(|value| !value.is_null())
        {
            components.push(
                Self::devtools_layout_component_from_rect(
                    "ActionsSearchInput",
                    LayoutComponentType::Input,
                    search_bounds,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_CONTROL_RADIUS_PX),
                )
                .with_visual_token("chrome.actionsSearch")
                .with_depth(1)
                .with_parent("ActionsDialog")
                .with_explanation("Search/filter input owned by the ActionsDialog route."),
            );
        }

        if let Some(header_bounds) = viewport
            .and_then(|viewport| viewport.get("contextHeaderBounds"))
            .filter(|value| !value.is_null())
        {
            components.push(
                Self::devtools_layout_component_from_rect(
                    "ActionsContextHeader",
                    LayoutComponentType::Header,
                    header_bounds,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                )
                .with_visual_token("chrome.actionsHeader")
                .with_depth(1)
                .with_parent("ActionsDialog")
                .with_explanation("Optional contextual header shown above the actions list."),
            );
        }

        if let Some(list_bounds) = viewport.and_then(|viewport| viewport.get("listBounds")) {
            components.push(
                Self::devtools_layout_component_from_rect(
                    "ActionsList",
                    LayoutComponentType::List,
                    list_bounds,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_CONTENT,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                )
                .with_visual_token("content.actionsList")
                .with_depth(1)
                .with_parent("ActionsDialog")
                .with_explanation("Scrollable grouped actions list viewport."),
            );
        }

        if let Some(rows) = row_geometry
            .get("rows")
            .and_then(serde_json::Value::as_array)
        {
            for row in rows.iter().filter(|row| {
                row.get("visible")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            }) {
                let visual_index = row
                    .get("visualIndex")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let kind = row
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("row");
                let rect = row.get("bounds").or_else(|| row.get("rect"));
                if let Some(rect) = rect {
                    let component_type = if kind == "section" {
                        LayoutComponentType::Header
                    } else {
                        LayoutComponentType::ListItem
                    };
                    components.push(
                        Self::devtools_layout_component_from_rect(
                            format!("ActionsRow[{visual_index}]"),
                            component_type,
                            rect,
                        )
                        .with_visual_style(
                            if kind == "section" {
                                chrome_tokens::CHROME_LAYER_FUNCTIONAL
                            } else {
                                chrome_tokens::CHROME_LAYER_CONTENT
                            },
                            chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                            Some(ACTIONS_ROW_RADIUS),
                        )
                        .with_visual_token(if kind == "section" {
                            "chrome.actionsSection"
                        } else {
                            "content.actionsRow"
                        })
                        .with_depth(2)
                        .with_parent("ActionsList")
                        .with_explanation(
                            "Visible grouped action row measured from runtime ActionsDialog row geometry.",
                        ),
                    );
                }

                if let Some(marker_bounds) = row
                    .get("selectionMarkerBounds")
                    .filter(|value| !value.is_null())
                {
                    components.push(
                        Self::devtools_layout_component_from_rect(
                            format!("ActionsSelectionMarker[{visual_index}]"),
                            LayoutComponentType::Other,
                            marker_bounds,
                        )
                        .with_visual_style(
                            chrome_tokens::CHROME_LAYER_CONTENT,
                            chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                            Some(
                                crate::designs::current_main_menu_theme()
                                    .def()
                                    .row
                                    .selected_marker_radius,
                            ),
                        )
                        .with_visual_token("content.selectionMarker")
                        .with_depth(3)
                        .with_parent(format!("ActionsRow[{visual_index}]"))
                        .with_explanation(
                            "Absolute launcher-family selected marker; excluded from flex, hit, and scroll geometry.",
                        ),
                    );
                }

                if let Some(shortcut_bounds) =
                    row.get("shortcutBounds").filter(|value| !value.is_null())
                {
                    components.push(
                        Self::devtools_layout_component_from_rect(
                            format!("ActionsShortcut[{visual_index}]"),
                            LayoutComponentType::Other,
                            shortcut_bounds,
                        )
                        .with_visual_style(
                            chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                            chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                            Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                        )
                        .with_visual_token("chrome.shortcutKeycap")
                        .with_visual_exception("denseKeyboardHint")
                        .with_depth(3)
                        .with_parent(format!("ActionsRow[{visual_index}]"))
                        .with_explanation(
                            "Right-aligned shortcut hint bounds measured by the shared hint-strip layout model.",
                        ),
                    );
                }
            }
        }

        LayoutInfo {
            window_width,
            window_height,
            prompt_type: "actionsDialog".to_string(),
            components,
            fidelity: None,
            handler_form: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub(crate) fn automation_state(&self, surface: &str, cx: &gpui::App) -> serde_json::Value {
        let selected_action = self.get_selected_action();
        let shortcut_parity = action_shortcut_parity_report(&self.actions, &self.filtered_actions);
        let shortcut_bindings: Vec<serde_json::Value> = shortcut_parity
            .visible_shortcut_bindings
            .iter()
            .map(|binding| {
                serde_json::json!({
                    "actionId": binding.action_id,
                    "shortcut": binding.shortcut,
                    "canonical": binding.canonical,
                    "routable": binding.routable,
                })
            })
            .collect();
        let unroutable_shortcuts: Vec<serde_json::Value> = shortcut_parity
            .unroutable_displayed_shortcuts
            .iter()
            .map(|binding| {
                serde_json::json!({
                    "actionId": binding.action_id,
                    "shortcut": binding.shortcut,
                    "canonical": binding.canonical,
                    "routable": binding.routable,
                })
            })
            .collect();
        let visible_actions: Vec<serde_json::Value> = self
            .filtered_actions
            .iter()
            .filter_map(|idx| self.actions.get(*idx))
            .take(20)
            .map(|action| {
                Self::devtools_action_summary(
                    action,
                    action_has_routable_shortcut(&self.actions, &self.filtered_actions, &action.id),
                )
            })
            .collect();
        let section_count = self.count_section_headers();
        let mut sections = std::collections::BTreeMap::<String, usize>::new();
        for action in self
            .filtered_actions
            .iter()
            .filter_map(|idx| self.actions.get(*idx))
        {
            let section = action
                .section
                .clone()
                .unwrap_or_else(|| "Unsectioned".to_string());
            *sections.entry(section).or_insert(0) += 1;
        }
        let route_stack: Vec<serde_json::Value> = self
            .route_stack
            .iter()
            .map(|state| {
                serde_json::json!({
                    "id": state.route.id.as_str(),
                    "actionCount": state.route.actions.len(),
                    "contextTitleLength": state.route.context_title.as_ref().map(|value| value.chars().count()),
                    "contextTitleFingerprint": state.route.context_title.as_ref().map(|value| Self::devtools_text_fingerprint(value)),
                    "searchTextLength": state.search_text.chars().count(),
                    "searchTextFingerprint": Self::devtools_text_fingerprint(&state.search_text),
                    "selectedActionId": state.selected_action_id.as_deref(),
                })
            })
            .collect();

        let search_input_state = self
            .search_is_visible()
            .then(|| {
                self.search_input.as_ref().map(|input| {
                    let input = input.read(cx);
                    let selection = input.selection();
                    serde_json::json!({
                        "owner": "gpuiComponentInputState",
                        "valueLength": input.value().chars().count(),
                        "valueFingerprint": Self::devtools_text_fingerprint(input.value().as_ref()),
                        "valueInSync": input.value().as_ref() == self.search_text,
                        "selectionStart": selection.start,
                        "selectionEnd": selection.end,
                    })
                })
            })
            .flatten();

        let mut state = serde_json::json!({
            "schemaVersion": 1,
            "surface": surface,
            "redacted": true,
            "search": {
                "textLength": self.search_text.chars().count(),
                "textFingerprint": Self::devtools_text_fingerprint(&self.search_text),
                "placeholderLength": self.current_search_placeholder().map(|value| value.chars().count()),
                "placeholderFingerprint": self.current_search_placeholder().map(Self::devtools_text_fingerprint),
                "hidden": self.hide_search || matches!(self.config.search_position, SearchPosition::Hidden),
                "position": actions_dialog_search_position_name(&self.config.search_position),
                "inputState": search_input_state,
            },
            "selection": {
                "groupedIndex": self.selected_index,
                "filteredIndex": self.get_selected_filtered_index(),
                "actionId": selected_action.map(|action| action.id.clone()),
                "actionTitleLength": selected_action.map(|action| action.title.chars().count()),
                "actionTitleFingerprint": selected_action.map(|action| Self::devtools_text_fingerprint(&action.title)),
                "shouldClose": self.selected_action_should_close(),
            },
            "actions": {
                "totalCount": self.actions.len(),
                "filteredCount": self.filtered_actions.len(),
                "groupedRowCount": self.grouped_items.len(),
                "sectionHeaderCount": section_count,
                "sections": sections,
                "visibleSampleLimit": visible_actions.len(),
                "visibleSample": visible_actions,
                "shortcutCount": shortcut_parity.displayed_shortcut_count,
                "shortcutParity": {
                    "displayedShortcutCount": shortcut_parity.displayed_shortcut_count,
                    "routableShortcutCount": shortcut_parity.routable_shortcut_count,
                    "duplicateShortcutCount": shortcut_parity.duplicate_shortcut_count,
                    "unroutableDisplayedShortcuts": unroutable_shortcuts,
                    "visibleShortcutBindings": shortcut_bindings,
                },
                "sdkActionsActive": self.has_sdk_actions(),
            },
            "route": {
                "currentRouteId": self.current_route_id(),
                "depth": self.route_depth(),
                "canPop": self.can_pop_route(),
                "hintLabel": self.route_hint_label(),
                "stack": route_stack,
                "registeredDrillDownRouteCount": self.drill_down_routes.len(),
            },
            "config": {
                "searchPosition": actions_dialog_search_position_name(&self.config.search_position),
                "sectionMode": actions_dialog_section_mode_name(&self.config.section_style),
                "anchor": match self.config.anchor {
                    AnchorPosition::Top => "top",
                    AnchorPosition::Bottom => "bottom",
                },
                "showIcons": self.config.show_icons,
                "showFooter": self.config.show_footer,
                "maxHeight": self.config.max_height,
            },
        });

        if let Some(attached_popup) = crate::actions::actions_popup_automation_snapshot() {
            state["attachedPopup"] = attached_popup;
        }
        if let Some(host_context) = self.host_context() {
            state["hostContext"] =
                serde_json::to_value(host_context).unwrap_or(serde_json::Value::Null);
            state["contextStableKey"] = serde_json::json!(host_context.parent_subject_id);
            state["contextSource"] = serde_json::json!(host_context.host);
            state["selectedSemanticId"] = serde_json::json!(host_context.selected_semantic_id);
        }
        state["rowGeometry"] = self.devtools_row_geometry(cx);
        let runtime_audit = ActionsDialogRuntimeAudit::from_parts(
            "actions_dialog",
            &self.config,
            &actions_dialog_default_style(),
        );
        let runtime_violations = runtime_audit.validate();
        state["runtimeAudit"] =
            serde_json::to_value(&runtime_audit).unwrap_or(serde_json::Value::Null);
        state["runtimeAuditViolations"] =
            serde_json::to_value(&runtime_violations).unwrap_or(serde_json::Value::Null);
        state["runtimeAuditStatus"] = serde_json::json!(if runtime_violations.is_empty() {
            "ok"
        } else {
            "violation"
        });

        state
    }

    /// Set actions from SDK (replaces built-in actions)
    ///
    /// Converts `ProtocolAction` items to internal `Action` format and updates
    /// the actions list. Filters out actions with `visible: false`.
    /// The `has_action` field on each action determines routing:
    /// - `has_action=true`: Send ActionTriggered back to SDK
    /// - `has_action=false`: Submit value directly
    pub fn set_sdk_actions(&mut self, actions: Vec<ProtocolAction>) {
        let previously_selected_action_id = self.get_selected_action_id();
        let total_count = actions.len();
        let mut sdk_action_indices = Vec::new();
        let mut seen_names: HashSet<String> = HashSet::new();
        let mut duplicate_names = Vec::new();

        let mut converted: Vec<Action> = actions
            .iter()
            .enumerate()
            .filter_map(|(protocol_index, pa)| {
                if !pa.is_visible() {
                    return None;
                }
                if !seen_names.insert(pa.name.clone()) {
                    duplicate_names.push(pa.name.clone());
                }
                sdk_action_indices.push(protocol_index);
                let shortcut = pa.shortcut.as_ref().map(|s| Self::format_shortcut_hint(s));
                let mut action = Action::new(
                    pa.name.clone(),
                    pa.name.clone(),
                    pa.description.clone(),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut_opt(shortcut);
                action.has_action = pa.has_action;
                action.value = pa.value.clone();
                Some(action)
            })
            .collect();
        append_disabled_action_test_fixture(&mut converted);
        let visible_count = converted.len();

        if !duplicate_names.is_empty() {
            tracing::warn!(
                target: "script_kit::actions",
                duplicate_names = ?duplicate_names,
                "SDK actions contain duplicate names; using selected row index for protocol mapping"
            );
        }

        logging::log(
            "ACTIONS",
            &format!(
                "SDK actions set: {} visible of {} total",
                visible_count, total_count
            ),
        );

        self.actions = converted;
        self.reset_filter_to_all();
        self.search_text.clear();
        self.sdk_actions = Some(actions);
        self.sdk_action_indices = sdk_action_indices;
        // Rebuild grouped items and reset selection
        self.rebuild_grouped_items();
        self.apply_refresh_selection(previously_selected_action_id.as_deref());
    }

    /// Format a keyboard shortcut for display (e.g., "cmd+c" → "⌘C")
    pub(crate) fn format_shortcut_hint(shortcut: &str) -> String {
        format_shortcut_hint_shared(shortcut)
    }

    /// Clear SDK actions and restore built-in actions
    pub fn clear_sdk_actions(&mut self) {
        if self.sdk_actions.is_some() {
            let previously_selected_action_id = self.get_selected_action_id();
            logging::log(
                "ACTIONS",
                "Clearing SDK actions, restoring built-in actions",
            );
            self.sdk_actions = None;
            self.sdk_action_indices.clear();
            self.actions = Self::build_actions(
                &self.focused_script,
                &self.focused_scriptlet,
                &self.menu_syntax_section,
                &self.host_section,
            );
            self.reset_filter_to_all();
            self.search_text.clear();
            // Rebuild grouped items and reset selection
            self.rebuild_grouped_items();
            self.apply_refresh_selection(previously_selected_action_id.as_deref());
        }
    }

    /// Check if SDK actions are currently active
    pub fn has_sdk_actions(&self) -> bool {
        self.sdk_actions.is_some()
    }

    /// Get the currently selected action (for external handling)
    pub fn get_selected_action(&self) -> Option<&Action> {
        self.selected_action_index()
            .and_then(|action_idx| self.actions.get(action_idx))
    }

    /// Count the number of section headers in the filtered action list
    /// A section header appears when an action's section differs from the previous action's section
    pub fn count_section_headers(&self) -> usize {
        if self.filtered_actions.is_empty() {
            return 0;
        }

        let mut count = 0;
        let mut prev_section: Option<&Option<String>> = None;

        for &idx in &self.filtered_actions {
            if let Some(action) = self.actions.get(idx) {
                let current_section = &action.section;
                // Count as header if: first item with a section, or section changed
                if current_section.is_some() {
                    match prev_section {
                        None => count += 1,                                  // First item with a section
                        Some(prev) if prev != current_section => count += 1, // Section changed
                        _ => {}
                    }
                }
                prev_section = Some(current_section);
            }
        }

        count
    }

    /// Build the complete actions list based on focused script and optional scriptlet.
    ///
    /// The actions menu is contextual: Power Syntax composer rows, host-owned
    /// rows (e.g. Day Page "Today"), and focused script/scriptlet actions own
    /// the list. Global rows (`get_global_actions`) are a fallback appended
    /// ONLY when the focused item contributes nothing, so Cmd+K never opens
    /// empty — they must not trail item-specific actions.
    fn build_actions(
        focused_script: &Option<ScriptInfo>,
        focused_scriptlet: &Option<Scriptlet>,
        menu_syntax_section: &Option<PowerSyntaxActionSection>,
        host_section: &Option<Vec<Action>>,
    ) -> Vec<Action> {
        let mut actions = Vec::new();

        if let Some(section) = menu_syntax_section {
            actions.extend(power_syntax_section_to_actions(section));
            if section.mode == SectionMode::Replace {
                append_disabled_action_test_fixture(&mut actions);
                return actions;
            }
        }

        // Host-owned contextual rows (e.g. Day Page "Today" section) lead the
        // list so the surface's own affordances outrank generic global rows.
        if let Some(host_actions) = host_section {
            actions.extend(host_actions.iter().cloned());
        }

        // Add script-specific actions if a script is focused
        if let Some(script) = focused_script {
            // If this is a scriptlet with custom actions, use the enhanced builder
            if script.is_scriptlet && focused_scriptlet.is_some() {
                actions.extend(get_scriptlet_context_actions_with_custom(
                    script,
                    focused_scriptlet.as_ref(),
                ));
            } else {
                // Use standard actions for regular scripts
                actions.extend(get_script_context_actions(script));
            }
        }

        if actions.is_empty() {
            actions.extend(get_global_actions());
        }

        append_disabled_action_test_fixture(&mut actions);
        actions
    }

    /// Update the focused script and rebuild actions
    pub fn set_focused_script(&mut self, script: Option<ScriptInfo>) {
        let previously_selected_action_id = self.get_selected_action_id();
        self.focused_script = script;
        self.focused_scriptlet = None; // Clear scriptlet when only setting script
        self.actions = Self::build_actions(
            &self.focused_script,
            &self.focused_scriptlet,
            &self.menu_syntax_section,
            &self.host_section,
        );
        self.refilter_with_previous_selection(previously_selected_action_id);
    }

    /// Update both the focused script and scriptlet for custom actions
    ///
    /// Use this when the focused item is a scriptlet with H3-defined custom actions.
    /// The scriptlet's actions will appear in the Actions Menu.
    pub fn set_focused_scriptlet(
        &mut self,
        script: Option<ScriptInfo>,
        scriptlet: Option<Scriptlet>,
    ) {
        let previously_selected_action_id = self.get_selected_action_id();
        self.focused_script = script;
        self.focused_scriptlet = scriptlet;
        self.actions = Self::build_actions(
            &self.focused_script,
            &self.focused_scriptlet,
            &self.menu_syntax_section,
            &self.host_section,
        );
        self.refilter_with_previous_selection(previously_selected_action_id);

        logging::log(
            "ACTIONS",
            &format!(
                "Set focused scriptlet with {} custom actions",
                self.focused_scriptlet
                    .as_ref()
                    .map(|s| s.actions.len())
                    .unwrap_or(0)
            ),
        );
    }

    /// Push the live Power Syntax action section. The App computes the
    /// live `MenuSyntaxActionState` from filter parse + active mode and pushes
    /// the owned section here when Cmd+K opens (Run 12 Pass 7 wiring at
    /// `src/app_impl/actions_toggle.rs`'s dialog construction site).
    pub fn set_menu_syntax_section(&mut self, section: Option<PowerSyntaxActionSection>) {
        let previously_selected_action_id = self.get_selected_action_id();
        self.menu_syntax_section = section;
        self.actions = Self::build_actions(
            &self.focused_script,
            &self.focused_scriptlet,
            &self.menu_syntax_section,
            &self.host_section,
        );
        self.refilter_with_previous_selection(previously_selected_action_id);
    }

    /// Push host-owned contextual rows (e.g. the Day Page "Today" section).
    /// Rows lead the rebuilt list; pass `None` to clear.
    pub fn set_host_section(&mut self, section: Option<Vec<Action>>) {
        let previously_selected_action_id = self.get_selected_action_id();
        self.host_section = section;
        self.actions = Self::build_actions(
            &self.focused_script,
            &self.focused_scriptlet,
            &self.menu_syntax_section,
            &self.host_section,
        );
        self.refilter_with_previous_selection(previously_selected_action_id);
    }

    #[cfg(test)]
    pub(crate) fn replace_actions_for_test(&mut self, actions: Vec<Action>) {
        let previously_selected_action_id = self.get_selected_action_id();
        self.actions = actions;
        self.reset_filter_to_all();
        self.rebuild_grouped_items();
        self.apply_refresh_selection(previously_selected_action_id.as_deref());
    }

    /// Update the theme when hot-reloading
    /// Call this from the parent when theme changes to ensure dialog reflects new colors
    pub fn update_theme(&mut self, theme: Arc<theme::Theme>) {
        let is_dark = theme.should_use_dark_vibrancy();
        logging::log(
            "ACTIONS_THEME",
            &format!(
                "Theme updated in ActionsDialog (mode={}, keycap_base=#{:06x})",
                if is_dark { "dark" } else { "light" },
                if is_dark {
                    theme.colors.ui.border
                } else {
                    theme.colors.text.secondary
                }
            ),
        );
        self.theme = theme;
    }

    /// Install the entity-backed Actions search input once a host window exists.
    pub(crate) fn ensure_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() || !self.search_is_visible() {
            return;
        }
        let input = cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .placeholder(self.search_placeholder_text())
                .default_value(self.search_text.clone())
                .tab_navigation(true)
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this, input, event: &gpui_component::input::InputEvent, _window, cx| {
                if !matches!(event, gpui_component::input::InputEvent::Change)
                    || this.search_input_syncing
                {
                    return;
                }
                let value = input.read(cx).value().to_string();
                this.apply_search_input_value(value, cx);
            },
        );
        self.search_input = Some(input);
        self.search_input_subscription = Some(subscription);
    }

    pub(crate) fn focus_search_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.search_is_visible() {
            return false;
        }
        let Some(input) = self.search_input.as_ref() else {
            return false;
        };
        input.update(cx, |state, cx| state.focus(window, cx));
        true
    }

    pub(crate) fn search_input_is_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.search_is_visible()
            && self
                .search_input
                .as_ref()
                .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window))
    }

    /// Reconcile route/automation model state into the real input owner.
    pub(crate) fn sync_search_input_from_model(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = self.search_input.clone() else {
            return;
        };
        let current = input.read(cx).value().to_string();
        let placeholder = self.search_placeholder_text();
        self.search_input_syncing = true;
        input.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
            if current != self.search_text {
                state.set_value(self.search_text.clone(), window, cx);
            }
        });
        self.search_input_syncing = false;
    }

    fn apply_search_input_value(&mut self, value: String, cx: &mut Context<Self>) {
        if self.search_text == value {
            return;
        }
        self.search_text = value;
        self.actions_selection_user_moved = false;
        self.refilter();
        cx.notify();
    }

    fn edit_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(
            &mut gpui_component::input::InputState,
            &mut Window,
            &mut Context<gpui_component::input::InputState>,
        ),
    ) -> bool {
        let Some(input) = self.search_input.clone() else {
            return false;
        };
        self.search_input_syncing = true;
        input.update(cx, |state, cx| edit(state, window, cx));
        let value = input.read(cx).value().to_string();
        self.search_input_syncing = false;
        self.apply_search_input_value(value, cx);
        true
    }

    pub(crate) fn insert_search_text(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let text = text.into();
        self.edit_search_input(window, cx, move |state, window, cx| {
            state.replace(text, window, cx);
        })
    }

    pub(crate) fn backspace_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, |state, window, cx| {
            state.edit_backspace(window, cx);
        })
    }

    pub(crate) fn delete_previous_search_word(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, |state, window, cx| {
            state.edit_delete_previous_word(window, cx);
        })
    }

    pub(crate) fn paste_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, |state, window, cx| {
            state.edit_paste(window, cx);
        })
    }

    pub(crate) fn move_search_cursor_left(
        &mut self,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, move |state, window, cx| {
            if select {
                state.edit_select_left(window, cx);
            } else {
                state.edit_move_left(window, cx);
            }
        })
    }

    pub(crate) fn move_search_cursor_right(
        &mut self,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, move |state, window, cx| {
            if select {
                state.edit_select_right(window, cx);
            } else {
                state.edit_move_right(window, cx);
            }
        })
    }

    pub(crate) fn select_all_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, |state, window, cx| {
            state.edit_select_all(window, cx);
        })
    }

    pub(crate) fn undo_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, |state, window, cx| {
            state.edit_undo(window, cx);
        })
    }

    pub(crate) fn redo_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.edit_search_input(window, cx, |state, window, cx| {
            state.edit_redo(window, cx);
        })
    }

    pub(crate) fn set_search_text_in_window(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search_input.is_none() {
            self.search_text = text;
            self.actions_selection_user_moved = false;
            self.refilter();
            cx.notify();
            return;
        }
        let _ = self.edit_search_input(window, cx, move |state, window, cx| {
            state.set_value(text, window, cx);
        });
    }

    /// Refilter actions based on current search_text using ranked fuzzy matching.
    ///
    /// Matching uses the shared launcher matcher (`SearchHighlightMatchCtx`),
    /// so popup rows match — and highlight — exactly like main-list rows.
    /// Ranking keeps the legacy tiers:
    /// - Title prefix match: +100; title substring: +50; title fuzzy: +25
    /// - Description contains: +15; shortcut contains: +10
    /// - Results are sorted by score (descending, stable on ties)
    fn refilter(&mut self) {
        let previously_selected = self.get_selected_action_id();
        self.refilter_with_previous_selection(previously_selected);
    }

    fn refilter_with_previous_selection(&mut self, previously_selected: Option<String>) {
        self.clear_mouse_submit_arm();
        self.clear_pending_scrollbar_offset();

        if self.search_text.trim().is_empty() {
            self.reset_filter_to_all();
        } else {
            let search_lower = self.search_text.trim().to_lowercase();
            let query_char_count = search_lower.chars().count();
            // Shared with the main launcher search so popups match (and
            // highlight) by exactly the same semantics.
            let mut highlight_ctx =
                crate::scripts::search::SearchHighlightMatchCtx::new(&self.search_text);

            // Score each action; matched actions carry their highlight indices.
            let mut scored: Vec<(usize, i32, Vec<usize>, Vec<usize>)> = self
                .actions
                .iter()
                .enumerate()
                .filter_map(|(idx, action)| {
                    Self::score_action_with_highlights(
                        action,
                        &search_lower,
                        query_char_count,
                        &mut highlight_ctx,
                    )
                    .map(|(score, title_indices, description_indices)| {
                        (idx, score, title_indices, description_indices)
                    })
                })
                .collect();

            // Sort by score descending (stable: ties keep action order)
            scored.sort_by(|a, b| b.1.cmp(&a.1));

            self.filtered_actions = Vec::with_capacity(scored.len());
            self.filtered_title_match_indices = Vec::with_capacity(scored.len());
            self.filtered_description_match_indices = Vec::with_capacity(scored.len());
            for (idx, _score, title_indices, description_indices) in scored {
                self.filtered_actions.push(idx);
                self.filtered_title_match_indices.push(title_indices);
                self.filtered_description_match_indices
                    .push(description_indices);
            }
        }

        // Rebuild grouped items after filter change
        self.rebuild_grouped_items();

        self.apply_refresh_selection(previously_selected.as_deref());

        // Only scroll if we have results
        if !self.grouped_items.is_empty() {
            self.update_pending_scrollbar_reveal_offset(self.effective_row_height());
            self.scroll_to_selected();
        }

        logging::log_debug(
            "ACTIONS_SCROLL",
            &format!(
                "Filter changed: {} results, selected={:?}",
                self.filtered_actions.len(),
                self.selected_index
            ),
        );
    }

    /// Rebuild grouped_items from current filtered_actions
    fn rebuild_grouped_items(&mut self) {
        self.grouped_items = build_grouped_items_static(
            &self.actions,
            &self.filtered_actions,
            self.config.section_style,
        );
        let old_count = self.list_state.item_count();
        let new_count = self.grouped_items.len();

        // Workaround for GPUI ListState stale layout: when transitioning
        // from 0 items back to N items (e.g., type "nice" → 0 results,
        // then delete all chars → all items restored), splice(0..0, N)
        // doesn't fully recalculate the list's internal layout heights.
        // Recreating the ListState forces a clean layout pass.
        if old_count == 0 && new_count > 0 {
            self.list_state = ListState::new(
                new_count,
                ListAlignment::Top,
                px(actions_dialog_list_overdraw_px()),
            );
        } else {
            self.list_state.splice(0..old_count, new_count);
        }
        self.clear_pending_scrollbar_offset();
    }

    fn selected_action_index(&self) -> Option<usize> {
        let filter_idx = self.get_selected_filtered_index()?;
        self.filtered_actions.get(filter_idx).copied()
    }

    fn grouped_index_for_action_index(&self, action_idx: usize) -> Option<usize> {
        let filter_idx = self
            .filtered_actions
            .iter()
            .position(|&idx| idx == action_idx)?;
        self.grouped_items
            .iter()
            .position(|item| matches!(item, GroupedActionItem::Item(i) if *i == filter_idx))
    }

    /// Get the filtered_actions index for the current selection
    /// Returns None if selection is on a section header
    pub fn get_selected_filtered_index(&self) -> Option<usize> {
        match self.grouped_items.get(self.selected_index?) {
            Some(GroupedActionItem::Item(filter_idx)) => Some(*filter_idx),
            _ => None,
        }
    }

    /// Reset the filter to include every action and drop stale match
    /// highlights. This is the single mutation point for `filtered_actions`
    /// outside the scored path in `refilter`, so the highlight vectors can
    /// never desync from the visible rows.
    pub(crate) fn reset_filter_to_all(&mut self) {
        self.filtered_actions = (0..self.actions.len()).collect();
        self.filtered_title_match_indices.clear();
        self.filtered_description_match_indices.clear();
    }

    /// Score an action against a search query.
    /// Returns 0 if no match, higher scores for better matches.
    ///
    /// Matching is delegated to the shared launcher matcher (case-insensitive,
    /// Unicode-aware); ranking keeps the legacy prefix > contains > fuzzy tiers.
    /// Production filtering goes through `score_action_with_highlights` inside
    /// `refilter`; this wrapper is the scoring contract surface for tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn score_action(action: &Action, search: &str) -> i32 {
        let search_lower = search.trim().to_lowercase();
        let mut highlight_ctx = crate::scripts::search::SearchHighlightMatchCtx::new(search);
        Self::score_action_with_highlights(
            action,
            &search_lower,
            search_lower.chars().count(),
            &mut highlight_ctx,
        )
        .map(|(score, _, _)| score)
        .unwrap_or(0)
    }

    /// Score one action and return its title/description highlight indices
    /// (char positions in the rendered strings). `None` when it doesn't match.
    ///
    /// Title matching uses the SAME matcher as the main launcher search
    /// (`SearchHighlightMatchCtx`: ASCII fast path + nucleo fallback), so a
    /// query matches a popup row exactly when it would match a main-list row.
    /// Description/shortcut keep their `contains` bonus semantics using the
    /// pre-computed lowercase fields.
    fn score_action_with_highlights(
        action: &Action,
        search_lower: &str,
        query_char_count: usize,
        highlight_ctx: &mut crate::scripts::search::SearchHighlightMatchCtx,
    ) -> Option<(i32, Vec<usize>, Vec<usize>)> {
        let mut score = 0;

        let mut title_indices = Vec::new();
        if search_lower.is_empty() {
            // Legacy contract: the empty query is a prefix of every title.
            // (`refilter` short-circuits empty searches; this only affects
            // direct `score_action` callers.)
            score += 100;
        } else {
            let (title_matched, indices) = highlight_ctx.indices_for(&action.title);
            if title_matched {
                score += Self::title_match_tier(&indices, query_char_count);
                title_indices = indices;
            }
        }

        let mut description_indices = Vec::new();
        if let Some(ref desc_lower) = action.description_lower {
            if desc_lower.contains(search_lower) {
                score += 15;
                if let Some(desc) = action.description.as_deref() {
                    let (_, indices) = highlight_ctx.indices_for(desc);
                    description_indices = indices;
                }
            }
        }

        if let Some(ref shortcut_lower) = action.shortcut_lower {
            if shortcut_lower.contains(search_lower) {
                score += 10;
            }
        }

        (score > 0).then_some((score, title_indices, description_indices))
    }

    /// Legacy case-sensitive subsequence matcher. Production matching goes
    /// through the shared `SearchHighlightMatchCtx` (see `score_action`); this
    /// is retained test-only because a large generated test corpus pins its
    /// exact subsequence semantics as a reference implementation.
    #[cfg(test)]
    pub(crate) fn fuzzy_match(haystack: &str, needle: &str) -> bool {
        let mut haystack_chars = haystack.chars();
        for needle_char in needle.chars() {
            loop {
                match haystack_chars.next() {
                    Some(h) if h == needle_char => break,
                    Some(_) => continue,
                    None => return false,
                }
            }
        }
        true
    }

    /// Derive the legacy prefix(100) > contains(50) > fuzzy(25) ranking tier
    /// from the shape of the matcher's indices: a contiguous run covering the
    /// whole query is a substring match; starting at 0 makes it a prefix.
    fn title_match_tier(indices: &[usize], query_char_count: usize) -> i32 {
        let contiguous = query_char_count > 0
            && indices.len() == query_char_count
            && indices.windows(2).all(|pair| pair[1] == pair[0] + 1);
        if contiguous && indices.first() == Some(&0) {
            100
        } else if contiguous {
            50
        } else {
            25
        }
    }

    /// Model-only replacement used by headless filtering tests. Production
    /// window routes use `set_search_text_in_window` so InputState stays the
    /// canonical editor and history owner.
    #[cfg(test)]
    pub(crate) fn set_search_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.search_text = text;
        self.actions_selection_user_moved = false;
        self.refilter();
        cx.notify();
    }

    /// Select an action by its `action.id`, optionally triggering it.
    ///
    /// Returns `Some(id)` if found, `None` otherwise.
    pub fn select_action_by_id(
        &mut self,
        action_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        // Find in filtered_actions
        let filter_pos = self
            .filtered_actions
            .iter()
            .position(|&idx| self.actions.get(idx).is_some_and(|a| a.id == action_id))?;

        // Map filter_pos to grouped_items index
        let grouped_idx = self
            .grouped_items
            .iter()
            .position(|item| matches!(item, GroupedActionItem::Item(fi) if *fi == filter_pos))?;

        self.selected_index = Some(grouped_idx);
        self.actions_selection_user_moved = true;
        self.clear_mouse_submit_arm();
        self.update_pending_scrollbar_reveal_offset(self.effective_row_height());
        self.scroll_to_selected();
        cx.notify();
        Some(action_id.to_string())
    }

    /// Select an action by its semantic ID (`choice:<filter_pos>:<action_id>`).
    ///
    /// Returns `Some(semantic_id)` if found, `None` otherwise.
    pub fn select_action_by_semantic_id(
        &mut self,
        semantic_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        // Parse "choice:<pos>:<id>"
        let parts: Vec<&str> = semantic_id.splitn(3, ':').collect();
        if parts.len() < 3 || parts[0] != "choice" {
            return None;
        }
        let action_id = parts[2];
        self.select_action_by_id(action_id, cx)
            .map(|_| semantic_id.to_string())
    }
}

// --- merged from part_03.rs ---
const ACTIONS_DIALOG_COLOR_ALPHA_MAX: f32 = 255.0;
const ACTIONS_DIALOG_SEARCH_BORDER_ALPHA_SCALE: f32 = 2.0;
const ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA: u8 = 0x80;
const ACTIONS_DIALOG_OPAQUE_DIALOG_MIN_OPACITY: f32 = 0.95;
// The actions dialog renders in its own native NSPanel with a real
// NSVisualEffectView blur layer.  A low opacity floor lets the system
// blur show through prominently while still tinting the background
// enough for text contrast.
const ACTIONS_DIALOG_VIBRANT_INLINE_MIN_OPACITY: f32 = 0.25;

fn actions_dialog_alpha_u8(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * ACTIONS_DIALOG_COLOR_ALPHA_MAX) as u8
}

#[inline]
fn should_submit_actions_dialog_row_click(was_mouse_armed: bool, click_count: usize) -> bool {
    was_mouse_armed || click_count >= 2
}

fn actions_dialog_search_border_alpha(border_inactive_opacity: f32) -> u8 {
    let scaled_border_opacity =
        (border_inactive_opacity * ACTIONS_DIALOG_SEARCH_BORDER_ALPHA_SCALE).min(1.0);
    actions_dialog_alpha_u8(scaled_border_opacity)
}

fn actions_dialog_container_border_alpha(border_inactive_opacity: f32) -> u8 {
    actions_dialog_search_border_alpha(border_inactive_opacity)
        .max(ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA)
}

fn actions_dialog_container_background_alpha(dialog_opacity: f32, use_vibrancy: bool) -> u8 {
    // The actions dialog has its own native NSPanel with NSVisualEffectView,
    // so a low opacity floor lets the system blur show through prominently.
    // Opaque (non-vibrancy) mode keeps a near-full readability floor.
    let resolved_opacity = if use_vibrancy {
        dialog_opacity.max(ACTIONS_DIALOG_VIBRANT_INLINE_MIN_OPACITY)
    } else {
        dialog_opacity.max(ACTIONS_DIALOG_OPAQUE_DIALOG_MIN_OPACITY)
    };
    actions_dialog_alpha_u8(resolved_opacity)
}

fn actions_dialog_rgba_with_alpha(hex: u32, alpha: u8) -> gpui::Rgba {
    rgba(hex_with_alpha(hex, alpha))
}

#[inline]
fn semantic_text_rgba(text_primary: u32, opacity: f32) -> gpui::Rgba {
    rgba(hex_with_alpha(
        text_primary,
        actions_dialog_alpha_u8(opacity.clamp(0.0, 1.0)),
    ))
}

#[inline]
fn actions_dialog_search_text_colors(
    text_primary: u32,
    opacity: &BackgroundOpacity,
) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    (
        semantic_text_rgba(text_primary, opacity.text_muted_alpha),
        semantic_text_rgba(text_primary, opacity.text_placeholder),
        semantic_text_rgba(text_primary, opacity.text_strong),
    )
}

#[inline]
fn actions_dialog_container_text_color(
    text_primary: u32,
    opacity: &BackgroundOpacity,
) -> gpui::Rgba {
    semantic_text_rgba(text_primary, opacity.text_muted_alpha)
}

fn actions_dialog_main_window_background_alpha(theme: &theme::Theme) -> u8 {
    let popup_surface = AppChromeColors::from_theme(theme).popup_surface_rgba;
    (popup_surface & 0xff) as u8
}

// ── Design-contract resolvers ───────────────────────────────────────────────
// Shared between `Render for ActionsDialog` and the token exporter
// (src/design_contract). The exporter passes explicit base definitions so
// checked-in artifacts never depend on runtime overrides; the renderer passes
// the current (override-applied) definitions. Both MUST route through these
// functions — duplicating the arithmetic or the alpha packing is how the HTML
// mockups drift from what production paints.

/// The exact `ListItemMetricsOverride` mutation the action-row render closure
/// applies on top of the main-menu metrics: action titles use the popup title
/// font size, the line height never shrinks below it, and the absolute selected
/// marker centers in the host's compact row without changing content geometry.
///
/// Deliberately does NOT apply `popup.row.radius`, `inner_y`,
/// `selection_opacity` or `hover_opacity`: production does not pass those into
/// the shared `ListItem` (recorded as `actionsRow.*ConfiguredVsPainted`
/// contract conflicts).
pub(crate) fn resolved_actions_dialog_row_metrics(
    popup: &crate::designs::ActionsPopupThemeDef,
    main_menu: crate::designs::MainMenuThemeDef,
) -> crate::list_item::ListItemMetricsOverride {
    let mut metrics = crate::list_item::ListItemMetricsOverride::from_main_menu_def(main_menu);
    metrics.name_font_size = popup.row.title_font_size;
    metrics.name_line_height = metrics.name_line_height.max(popup.row.title_font_size);
    metrics.row_selected_marker_center_height = popup.list.row_height;
    metrics
}

/// What one action row actually paints: slot/inset geometry plus the resolved
/// selected/hover fills from the shared main-menu row resolver.
pub(crate) struct ResolvedActionsDialogRowChrome {
    pub slot_height: f32,
    pub wrapper_inset_x: f32,
    pub metrics: crate::list_item::ListItemMetricsOverride,
    /// Selected surface left inset from the popup shell edge.
    pub surface_inset_x: f32,
    /// Action title text origin from the popup shell edge.
    pub text_origin_x: f32,
    pub selected_background_rgba: u32,
    pub hover_background_rgba: u32,
}

pub(crate) fn resolved_actions_dialog_row_chrome(
    popup: &crate::designs::ActionsPopupThemeDef,
    main_menu: crate::designs::MainMenuThemeDef,
    theme: &theme::Theme,
) -> ResolvedActionsDialogRowChrome {
    let metrics = resolved_actions_dialog_row_metrics(popup, main_menu);
    let fill = crate::list_item::resolved_main_menu_row_fill(
        main_menu.row_kind,
        &metrics,
        theme.get_opacity().hover,
    );
    let base_hex = match fill.base {
        crate::list_item::MainMenuRowFillBase::TextPrimary => theme.colors.text.primary,
        crate::list_item::MainMenuRowFillBase::Accent => theme.colors.accent.selected,
    };
    ResolvedActionsDialogRowChrome {
        slot_height: popup.list.row_height,
        wrapper_inset_x: popup.row.inset_x,
        surface_inset_x: popup.row.inset_x + metrics.row_outer_padding_x,
        text_origin_x: popup.row.inset_x
            + metrics.row_outer_padding_x
            + metrics.row_inner_padding_x,
        selected_background_rgba: (base_hex << 8) | fill.selected_alpha as u32,
        hover_background_rgba: (base_hex << 8) | fill.hover_alpha as u32,
        metrics,
    }
}

/// The search row's resolved paint values, using the same truncating
/// `(opacity * 255.0) as u8` packing as `semantic_text_rgba` /
/// `actions_dialog_alpha_u8` above. Default design variant only (the checked-in
/// contract profile).
pub(crate) struct ResolvedActionsDialogSearchChrome {
    pub padding_y: f32,
    pub caret_rgba: u32,
    pub placeholder_rgba: u32,
    pub input_text_rgba: u32,
}

pub(crate) fn resolved_actions_dialog_search_chrome(
    popup: &crate::designs::ActionsPopupThemeDef,
    spacing: &crate::designs::DesignSpacing,
    theme: &theme::Theme,
) -> ResolvedActionsDialogSearchChrome {
    let opacity = theme.get_opacity();
    let text_primary = theme.colors.text.primary;
    ResolvedActionsDialogSearchChrome {
        padding_y: spacing.item_padding_y + popup.search.padding_y_extra,
        caret_rgba: (theme.colors.accent.selected << 8) | 0xFF,
        placeholder_rgba: hex_with_alpha(
            text_primary,
            actions_dialog_alpha_u8(opacity.text_placeholder),
        ),
        input_text_rgba: (text_primary << 8) | 0xFF,
    }
}

/// The section-header row's resolved layout and paint. The actions section
/// renderer centers vertically inside `list.section_header_height` and does
/// not consume the declared `section.padding_top/bottom` (recorded as the
/// `actionsSection.paddingDeclaredVsCenteredRenderer` conflict).
pub(crate) struct ResolvedActionsDialogSectionChrome {
    pub height: f32,
    pub padding_x: f32,
    pub font_size: f32,
    pub font_weight: gpui::FontWeight,
    pub text_rgba: u32,
}

pub(crate) fn resolved_actions_dialog_section_chrome(
    popup: &crate::designs::ActionsPopupThemeDef,
    theme: &theme::Theme,
) -> ResolvedActionsDialogSectionChrome {
    let opacity = theme.get_opacity();
    ResolvedActionsDialogSectionChrome {
        height: popup.list.section_header_height,
        padding_x: popup.section.padding_x,
        font_size: popup.section.font_size,
        font_weight: popup.section.font_weight,
        text_rgba: hex_with_alpha(
            theme.colors.text.primary,
            actions_dialog_alpha_u8(opacity.text_muted_alpha),
        ),
    }
}

impl ActionsDialog {
    fn clear_mouse_submit_arm(&mut self) {
        self.mouse_armed_row = None;
    }

    fn trigger_scrollbar_activity(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.last_scroll_time = Some(now);
        self.scrollbar_visibility = crate::transitions::Opacity::VISIBLE;
        self.scrollbar_fade_gen = self.scrollbar_fade_gen.wrapping_add(1);
        let fade_gen = self.scrollbar_fade_gen;

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ACTIONS_DIALOG_SCROLLBAR_IDLE_DELAY)
                .await;

            let should_start_fade = cx
                .update(|cx| {
                    this.update(cx, |dialog, _cx| {
                        if dialog.scrollbar_fade_gen != fade_gen {
                            return false;
                        }

                        dialog
                            .last_scroll_time
                            .map(|last_time| {
                                last_time.elapsed() >= ACTIONS_DIALOG_SCROLLBAR_IDLE_DELAY
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if !should_start_fade {
                return;
            }

            let fade_duration = actions_dialog_scrollbar_fade_duration();
            let fade_start = Instant::now();

            loop {
                let elapsed = fade_start.elapsed();
                let progress =
                    (elapsed.as_secs_f32() / fade_duration.as_secs_f32()).clamp(0.0, 1.0);
                let opacity = actions_dialog_scrollbar_fade_opacity(progress);

                let continue_fade = cx
                    .update(|cx| {
                        this.update(cx, |dialog, cx| {
                            if dialog.scrollbar_fade_gen != fade_gen {
                                return false;
                            }

                            dialog.scrollbar_visibility = opacity;
                            cx.notify();
                            progress < 1.0
                        })
                    })
                    .unwrap_or(false);

                if !continue_fade {
                    break;
                }

                cx.background_executor()
                    .timer(ACTIONS_DIALOG_SCROLLBAR_FADE_TICK)
                    .await;
            }
        })
        .detach();

        cx.notify();
    }

    pub(crate) fn reveal_selection_after_navigation(&mut self, cx: &mut Context<Self>) {
        self.actions_selection_user_moved = true;
        self.clear_mouse_submit_arm();
        self.update_pending_scrollbar_reveal_offset(self.effective_row_height());
        self.scroll_to_selected();
        self.trigger_scrollbar_activity(cx);
        cx.notify();
    }

    /// Move selection up, skipping section headers
    ///
    /// When moving up and landing on a section header, we must search UPWARD
    /// (not downward) to find the previous selectable item. This ensures
    /// navigation past section headers works correctly.
    pub fn move_up(&mut self, cx: &mut Context<Self>) {
        let Some(selected_index) = self.selected_index else {
            self.selected_index = last_selectable_index(&self.grouped_items);
            if self.selected_index.is_some() {
                self.reveal_selection_after_navigation(cx);
            }
            return;
        };
        if selected_index == 0 {
            return;
        }

        if let Some(index) = selectable_index_at_or_before(&self.grouped_items, selected_index - 1)
        {
            self.selected_index = Some(index);
            self.reveal_selection_after_navigation(cx);
            logging::log_debug(
                "ACTIONS_SCROLL",
                &format!("Up: selected_index={:?}", self.selected_index),
            );
        }
    }

    /// Move selection down, skipping section headers
    pub fn move_down(&mut self, cx: &mut Context<Self>) {
        let Some(selected_index) = self.selected_index else {
            self.selected_index = first_selectable_index(&self.grouped_items);
            if self.selected_index.is_some() {
                self.reveal_selection_after_navigation(cx);
            }
            return;
        };
        if selected_index >= self.grouped_items.len().saturating_sub(1) {
            return;
        }

        let Some(start) = selected_index.checked_add(1) else {
            return;
        };

        if let Some(index) = selectable_index_at_or_after(&self.grouped_items, start) {
            self.selected_index = Some(index);
            self.reveal_selection_after_navigation(cx);
            logging::log_debug(
                "ACTIONS_SCROLL",
                &format!("Down: selected_index={:?}", self.selected_index),
            );
        }
    }

    /// Get the currently selected action ID (for external handling)
    pub fn get_selected_action_id(&self) -> Option<String> {
        self.get_selected_action().map(|action| action.id.clone())
    }

    /// Get the currently selected ProtocolAction (for checking close behavior)
    /// Returns the original ProtocolAction from sdk_actions if this is an SDK action,
    /// or None for built-in actions.
    pub fn get_selected_protocol_action(&self) -> Option<&ProtocolAction> {
        let protocol_action_index = resolve_selected_protocol_action_index(
            self.selected_action_index(),
            &self.sdk_action_indices,
        )?;
        self.sdk_actions.as_ref()?.get(protocol_action_index)
    }

    fn action_should_close(&self, action_id: &str) -> bool {
        let Some(action_index) = self
            .actions
            .iter()
            .position(|action| action.id == action_id)
        else {
            return false;
        };
        if !self.actions[action_index].is_enabled() {
            return false;
        }
        self.sdk_actions
            .as_ref()
            .and_then(|actions| {
                self.sdk_action_indices
                    .get(action_index)
                    .and_then(|protocol_index| actions.get(*protocol_index))
            })
            .map(ProtocolAction::should_close)
            .unwrap_or(true)
    }

    /// Check if the currently selected action should close the dialog
    /// Returns true if the action has close: true (or no close field, which defaults to true)
    /// Returns true for built-in actions (they always close)
    pub fn selected_action_should_close(&self) -> bool {
        if self
            .get_selected_action()
            .is_some_and(|action| !action.is_enabled())
        {
            return false;
        }
        if let Some(protocol_action) = self.get_selected_protocol_action() {
            protocol_action.should_close()
        } else {
            // Built-in actions always close
            true
        }
    }

    /// Cancel - close the dialog
    pub fn submit_cancel(&mut self) {
        logging::log("ACTIONS", "Actions dialog cancelled");
        (self.on_select)("__cancel__".to_string());
    }

    /// Select a grouped item by index without submitting.
    /// Skips the update if the row is already selected.
    fn select_grouped_item(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.selected_index == Some(ix) {
            return;
        }
        self.selected_index = Some(ix);
        self.actions_selection_user_moved = true;
        self.clear_mouse_submit_arm();
        self.update_pending_scrollbar_reveal_offset(self.effective_row_height());
        self.scroll_to_selected();
        let action_id = self
            .get_selected_action()
            .map(|action| action.id.clone())
            .unwrap_or_else(|| "<none>".to_string());
        tracing::info!(
            event = "actions_dialog_row_selected",
            row_index = ix,
            action_id = %action_id,
        );
        cx.notify();
    }

    /// Handle a click on a row: first click selects, a second click on the
    /// same mouse-armed row submits, and native double-clicks also submit.
    /// Section headers are ignored.
    pub fn handle_row_click(
        &mut self,
        ix: usize,
        event: &gpui::ClickEvent,
        cx: &mut Context<Self>,
    ) -> Option<ActionsDialogActivation> {
        // Ignore clicks on section headers
        if !matches!(self.grouped_items.get(ix), Some(GroupedActionItem::Item(_))) {
            return None;
        }

        self.actions_selection_user_moved = true;

        let was_selected = self.selected_index == Some(ix);
        let was_mouse_armed = self.mouse_armed_row == Some(ix);
        if !was_selected {
            self.select_grouped_item(ix, cx);
        }

        let click_count = event.click_count();
        let should_submit = should_submit_actions_dialog_row_click(was_mouse_armed, click_count);
        self.mouse_armed_row = Some(ix);

        let action_id = self
            .grouped_items
            .get(ix)
            .and_then(|item| match item {
                GroupedActionItem::Item(filter_idx) => self.filtered_actions.get(*filter_idx),
                GroupedActionItem::SectionHeader(_) => None,
            })
            .and_then(|&action_idx| self.actions.get(action_idx))
            .map(|action| action.id.clone())
            .unwrap_or_else(|| "<none>".to_string());

        tracing::info!(
            event = "actions_dialog_row_click",
            row_index = ix,
            action_id = %action_id,
            click_count = click_count,
            was_selected = was_selected,
            was_mouse_armed = was_mouse_armed,
            should_submit = should_submit,
        );

        if should_submit {
            self.clear_mouse_submit_arm();
            return Some(self.activate_selected(cx));
        }

        None
    }

    /// Create box shadow for the overlay popup
    /// When rendered in a separate vibrancy window, no shadow is needed
    /// (the window vibrancy provides visual separation)
    pub(super) fn create_popup_shadow() -> Vec<BoxShadow> {
        // No shadow - vibrancy window provides visual separation
        vec![]
    }

    /// Get colors for the search box based on design variant
    /// Returns: (search_box_bg, border_color, muted_text, hint_text, strong_text)
    pub(super) fn get_search_colors(
        &self,
        colors: &crate::designs::DesignColors,
    ) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        // Use theme opacity for input background to support vibrancy
        let opacity = self.theme.get_opacity();
        let input_alpha = actions_dialog_alpha_u8(opacity.input);
        // Keep search and container borders on the same opacity scaling path.
        let border_alpha = actions_dialog_search_border_alpha(opacity.border_inactive);
        let (search_box_background, search_box_border, text_primary) =
            if self.design_variant == DesignVariant::Default {
                (
                    self.theme.colors.background.search_box,
                    self.theme.colors.ui.border,
                    self.theme.colors.text.primary,
                )
            } else {
                (
                    colors.background_secondary,
                    colors.border,
                    colors.text_primary,
                )
            };

        let (muted_text, hint_text, strong_text) =
            actions_dialog_search_text_colors(text_primary, &opacity);

        (
            actions_dialog_rgba_with_alpha(search_box_background, input_alpha),
            actions_dialog_rgba_with_alpha(search_box_border, border_alpha),
            muted_text,
            hint_text,
            strong_text,
        )
    }

    /// Get colors for the main container based on design variant
    /// Returns: (main_bg, container_border, container_text)
    pub(super) fn get_container_colors(
        &self,
        colors: &crate::designs::DesignColors,
    ) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        let opacity = self.theme.get_opacity();
        let use_vibrancy = self.theme.is_vibrancy_enabled();
        let dialog_alpha = if use_vibrancy && self.match_main_window_background {
            actions_dialog_main_window_background_alpha(self.theme.as_ref())
        } else {
            // In vibrancy mode this uses theme dialog opacity directly so blur can show through.
            // In opaque mode it keeps a high readability floor.
            actions_dialog_container_background_alpha(opacity.dialog, use_vibrancy)
        };
        let border_alpha = actions_dialog_container_border_alpha(opacity.border_inactive);
        let (main_background, container_border, text_primary) =
            if self.design_variant == DesignVariant::Default {
                (
                    self.theme.colors.background.main,
                    self.theme.colors.ui.border,
                    self.theme.colors.text.primary,
                )
            } else {
                (colors.background, colors.border, colors.text_primary)
            };

        (
            actions_dialog_rgba_with_alpha(main_background, dialog_alpha),
            actions_dialog_rgba_with_alpha(container_border, border_alpha),
            actions_dialog_container_text_color(text_primary, &opacity),
        )
    }
}

#[cfg(test)]
mod actions_dialog_opacity_consistency_tests {
    use super::{
        actions_dialog_container_background_alpha, actions_dialog_container_border_alpha,
        actions_dialog_container_text_color, actions_dialog_main_window_background_alpha,
        actions_dialog_rgba_with_alpha, actions_dialog_search_border_alpha,
        actions_dialog_search_text_colors, semantic_text_rgba,
        ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA,
    };
    use crate::theme::{AppChromeColors, Theme};
    use gpui::rgba;

    #[test]
    fn test_actions_dialog_search_border_alpha_scales_border_inactive_opacity() {
        assert_eq!(actions_dialog_search_border_alpha(0.20), 102);
    }

    #[test]
    fn test_actions_dialog_container_border_alpha_enforces_minimum_contrast() {
        assert_eq!(
            actions_dialog_container_border_alpha(0.10),
            ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA
        );
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_uses_vibrant_floor() {
        // 0.15 dialog opacity is clamped up to 0.25 vibrant floor → 63
        assert_eq!(actions_dialog_container_background_alpha(0.15, true), 63);
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_keeps_non_vibrancy_floor() {
        assert_eq!(actions_dialog_container_background_alpha(0.15, false), 242);
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_passes_through_above_floor() {
        // 0.80 is above the 0.25 vibrant floor → passes through → 204
        assert_eq!(actions_dialog_container_background_alpha(0.80, true), 204);
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_uses_higher_theme_value_above_floor() {
        // 0.90 is above the 0.25 floor → passes through → 229
        assert_eq!(actions_dialog_container_background_alpha(0.90, true), 229);
    }

    #[test]
    fn test_actions_dialog_main_window_background_alpha_matches_dark_window_default() {
        let theme = Theme::dark_default();
        let expected = (crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND * 255.0) as u8;
        assert_eq!(
            actions_dialog_main_window_background_alpha(&theme),
            expected
        );
    }

    #[test]
    fn test_actions_dialog_main_window_background_alpha_uses_shared_popup_surface_token() {
        let mut theme = Theme::light_default();
        let mut opacity = theme.get_opacity();
        opacity.vibrancy_background = Some(0.40);
        theme.opacity = Some(opacity);

        assert_eq!(
            actions_dialog_main_window_background_alpha(&theme),
            (AppChromeColors::from_theme(&theme).popup_surface_rgba & 0xff) as u8
        );
    }

    #[test]
    fn test_actions_dialog_rgba_with_alpha_combines_hex_and_alpha_channels() {
        let theme = Theme::default();
        let background = theme.colors.background.main;

        assert_eq!(
            actions_dialog_rgba_with_alpha(background, 0x44),
            rgba((background << 8) | 0x44)
        );
    }

    #[test]
    fn test_actions_dialog_search_and_container_text_follow_shared_theme_opacity_ladder() {
        let theme = Theme::dark_default();
        let opacity = theme.get_opacity();
        let (muted_text, hint_text, strong_text) =
            actions_dialog_search_text_colors(theme.colors.text.primary, &opacity);
        let container_text =
            actions_dialog_container_text_color(theme.colors.text.primary, &opacity);

        assert_eq!(
            muted_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_muted_alpha),
            "search muted text must use primary text plus shared muted alpha"
        );
        assert_eq!(
            hint_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_placeholder),
            "search hint text must use primary text plus shared placeholder alpha"
        );
        assert_eq!(
            strong_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_strong),
            "search strong text must use primary text plus shared strong alpha"
        );
        assert_eq!(
            container_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_muted_alpha),
            "container text must use primary text plus shared muted alpha"
        );
    }
}

// --- merged from part_03.rs ---

impl Focusable for ActionsDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// --- merged from dialog_part_04_rewire.rs ---
impl Render for ActionsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut style = actions_dialog_default_style();
        style.row_height = self.effective_row_height();
        let popup_theme = crate::designs::current_actions_popup_theme();
        crate::components::hint_strip::emit_shortcut_chrome_audit(
            "actions_dialog",
            "compact-inline-focused-only",
        );

        // Get design tokens for the current design variant
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();

        // NOTE: Key handling is done by the parent (ScriptListApp in main.rs)
        // which routes all keyboard events to this dialog's methods.
        // We do NOT attach our own on_key_down handler to avoid double-processing.

        // The Actions chrome owns the row surface; InputState owns text editing,
        // selection, cursor, clipboard, IME, and undo history.

        // Use helper method for design/theme color extraction
        let (_search_box_bg, border_color, _muted_text, hint_text, _strong_text) =
            self.get_search_colors(&colors);

        // Get primary text color for cursor (matches main list styling)
        let primary_text = if self.design_variant == DesignVariant::Default {
            rgb(self.theme.colors.text.primary)
        } else {
            rgb(colors.text_primary)
        };

        // Raycast-style footer search input: minimal styling, full-width, top separator line
        // No boxed input field - just text on a clean background with a thin top border
        // Use theme colors for both light and dark mode
        // Light mode derives from the same theme tokens as dark mode
        let separator_color = border_color;
        let hint_text_color = hint_text;
        let input_text_color = primary_text;
        let build_search_content = || {
            if let Some(search_input) = self.search_input.as_ref() {
                gpui_component::input::Input::new(search_input)
                    .w_full()
                    .h(px(popup_theme.search.inner_height))
                    .line_height(px(popup_theme.search.inner_height))
                    .px(px(0.0))
                    .py(px(0.0))
                    .with_size(gpui_component::Size::Size(px(popup_theme.search.font_size)))
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .text_color(input_text_color)
                    .when_some(style.prefix_marker, |input, prefix_marker| {
                        input.prefix(
                            div()
                                .flex_none()
                                .mr(px(popup_theme.search.prefix_gap))
                                .font_family(if style.mono_font {
                                    crate::list_item::FONT_MONO
                                } else {
                                    crate::list_item::FONT_SYSTEM_UI
                                })
                                .text_size(px(popup_theme.search.font_size))
                                .text_color(hint_text_color)
                                .child(prefix_marker),
                        )
                    })
                    .into_any_element()
            } else {
                // Hidden-search and model-only test dialogs never install a window-bound
                // InputState. Keep their render inert rather than inventing another editor.
                div()
                    .w_full()
                    .h(px(popup_theme.search.inner_height))
                    .into_any_element()
            }
        };

        // Render action list using list() for variable-height items
        // Section headers are 22px, action items are 36px
        //
        // IMPORTANT: Always render the list() component, even when empty.
        // Switching between a static empty-state div and the list component
        // causes the GPUI ListState to lose sync with the render tree,
        // resulting in stale layout when items are restored after filtering
        // to zero results (e.g., type "nice" then delete all characters).
        let actions_container = {
            // Clone data needed for the list closure
            let grouped_items_clone = self.grouped_items.clone();
            let design_variant = self.design_variant;
            let is_empty = self.grouped_items.is_empty();

            // Get scrollbar colors from theme for consistent styling
            let scrollbar_colors = ScrollbarColors::from_theme(&self.theme);
            let scrollbar_metrics = self.scrollbar_metrics(style.row_height);

            // Create scrollbar (only visible if content overflows)
            let scrollbar = Scrollbar::from_pixel_metrics(scrollbar_metrics, scrollbar_colors)
                .visibility_opacity(self.scrollbar_visibility.value());

            // Capture entity handle for use in the render closure
            let entity = cx.entity();

            let variable_height_list = list(self.list_state.clone(), move |ix, _window, cx| {
                // Access entity state inside the closure
                entity.update(cx, |this, _cx| {
                    let current_selected = this.selected_index;

                    if let Some(grouped_item) = grouped_items_clone.get(ix) {
                        match grouped_item {
                            GroupedActionItem::SectionHeader(label) => {
                                // Actions uses the shared launcher-family display grammar while
                                // retaining its authored section string in the grouped model.
                                let presentation =
                                    crate::list_item::resolve_section_header_presentation(
                                        label,
                                        None,
                                        None,
                                        crate::list_item::SectionPresentationFamily::Launcher,
                                    );
                                let theme_opacity = this.theme.get_opacity();
                                let header_text =
                                    if this.design_variant == DesignVariant::Default {
                                        semantic_text_rgba(
                                            this.theme.colors.text.primary,
                                            theme_opacity.text_strong,
                                        )
                                    } else {
                                        let tokens = get_tokens(this.design_variant);
                                        semantic_text_rgba(
                                            tokens.colors().text_primary,
                                            theme_opacity.text_strong,
                                        )
                                    };
                                let section_header = div()
                                    .id(ElementId::NamedInteger("section-header".into(), ix as u64))
                                    .h(px(popup_theme.list.section_header_height))
                                    .w_full()
                                    .px(px(popup_theme.section.padding_x))
                                    .flex()
                                    .items_center();

                                section_header
                                    .child(
                                        div()
                                            .text_size(px(popup_theme.section.font_size))
                                            .font_weight(popup_theme.section.font_weight)
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_color(header_text)
                                            .child(presentation.display_label),
                                    )
                                    .into_any_element()
                            }
                            GroupedActionItem::Item(filter_idx) => {
                                // Action rows reuse the shared main-search ListItem chrome.
                                if let Some(&action_idx) = this.filtered_actions.get(*filter_idx) {
                                    if let Some(action) = this.actions.get(action_idx) {
                                        let is_selected = Some(ix) == current_selected;
                                        let is_destructive = is_destructive_action(action);

                                        if is_destructive && style.shortcut_visible && action.shortcut.is_some() {
                                            crate::components::hint_strip::emit_shortcut_chrome_audit(
                                                "actions_dialog_destructive_shortcut",
                                                "neutral-muted",
                                            );
                                        }

                                        let list_colors =
                                            crate::list_item::ListItemColors::from_theme(
                                                this.theme.as_ref(),
                                            );
                                        let main_menu_theme =
                                            crate::designs::current_main_menu_theme();
                                        let actions_row_metrics =
                                            resolved_actions_dialog_row_metrics(
                                                &popup_theme,
                                                main_menu_theme.def(),
                                            );
                                        let shortcut_routable = action_has_routable_shortcut(
                                            &this.actions,
                                            &this.filtered_actions,
                                            &action.id,
                                        );
                                        let shortcut = if style.shortcut_visible && shortcut_routable {
                                            action.shortcut.clone()
                                        } else {
                                            None
                                        };
                                        // Match highlights are stored positionally alongside
                                        // `filtered_actions` (see `refilter`), so `filter_idx`
                                        // addresses both vectors.
                                        let title_highlights = this
                                            .filtered_title_match_indices
                                            .get(*filter_idx)
                                            .cloned();
                                        let description_highlights = this
                                            .filtered_description_match_indices
                                            .get(*filter_idx)
                                            .cloned();
                                        let mut list_item = crate::list_item::ListItem::new(
                                            action.title.clone(),
                                            list_colors,
                                        )
                                        .index(ix)
                                        .selected(is_selected)
                                        .disabled(!action.is_enabled())
                                        .hovered(this.hovered_row == Some(ix))
                                        .main_menu_theme(main_menu_theme)
                                        .metrics_override(actions_row_metrics)
                                        .semantic_id(format!("choice:{ix}:{}", action.id))
                                        .description_opt(
                                            action_subtitle_for_display(
                                                action,
                                                this.config.show_subtitles,
                                            )
                                            .map(str::to_string),
                                        )
                                        .highlight_indices_opt(title_highlights)
                                        .description_highlight_indices_opt(description_highlights)
                                        .shortcut_opt(shortcut)
                                        .shortcut_visibility_policy(
                                            crate::list_item::RowShortcutVisibilityPolicy::AllRows,
                                        );

                                        if let Some(reason) = action.disabled_reason() {
                                            list_item = list_item.trailing_accessory(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "disabled-reason:{}",
                                                        action.id
                                                    )))
                                                    .w(px(132.0))
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .whitespace_nowrap()
                                                    .text_size(px(11.0))
                                                    .text_color(rgba(
                                                        (this.theme.colors.text.dimmed << 8) | 0xFF,
                                                    ))
                                                    .child(reason.to_string()),
                                            );
                                        }

                                        if is_destructive {
                                            let destructive_text =
                                                if design_variant == DesignVariant::Default {
                                                    this.theme.colors.ui.error
                                                } else {
                                                    get_tokens(design_variant).colors().error
                                                };
                                            list_item =
                                                list_item.destructive_text_color(destructive_text);
                                        }

                                        if let Some(prefix_marker) = style.prefix_marker {
                                            let prefix_marker = prefix_marker.to_string();
                                            list_item = list_item.leading_accessory(
                                                div()
                                                    .text_color(rgba(
                                                        (this.theme.colors.text.primary << 8)
                                                            | crate::theme::types::opacity_to_alpha(
                                                                this.theme.get_opacity().text_hint,
                                                            ),
                                                    ))
                                                    .font_family(crate::list_item::FONT_MONO)
                                                    .child(prefix_marker),
                                            );
                                        }

                                        if this.config.show_icons && style.show_icons {
                                            if let Some(icon) = action.icon {
                                                list_item = list_item.icon_kind(
                                                    crate::list_item::IconKind::Svg(
                                                        icon.asset_path().to_string(),
                                                    ),
                                                );
                                            }
                                        }

                                        let action_row = div()
                                            .id(ElementId::NamedInteger(
                                                "action-item".into(),
                                                ix as u64,
                                            ))
                                            .h(px(style.row_height))
                                            .w_full()
                                            .px(px(popup_theme.row.inset_x))
                                            .flex()
                                            .flex_col()
                                            .justify_center()

                                            .on_click({
                                                let entity = entity.clone();
                                                move |event, window, cx| {
                                                    let (activation, callback) = entity.update(
                                                        cx,
                                                        |this, cx| {
                                                            (
                                                                this.handle_row_click(ix, event, cx),
                                                                this.on_activation.clone(),
                                                            )
                                                        },
                                                    );
                                                    if let (Some(activation), Some(callback)) =
                                                        (activation, callback)
                                                    {
                                                        callback(activation, window, cx);
                                                    }
                                                }
                                            })
                                            .on_mouse_move({
                                                let entity = entity.clone();
                                                move |_event: &gpui::MouseMoveEvent, _window, cx| {
                                                    entity.update(cx, |this, cx| {
                                                        if this.hovered_row != Some(ix) {
                                                            this.hovered_row = Some(ix);
                                                            cx.notify();
                                                        }
                                                    });
                                                }
                                            })
                                            .on_hover({
                                                let entity = entity.clone();
                                                move |hovered: &bool, _window, cx| {
                                                    entity.update(cx, |this, cx| {
                                                        if *hovered {
                                                            if this.hovered_row != Some(ix) {
                                                                this.hovered_row = Some(ix);
                                                                cx.notify();
                                                            }
                                                        } else if this.hovered_row == Some(ix) {
                                                            this.hovered_row = None;
                                                            cx.notify();
                                                        }
                                                    });
                                                }
                                            });

                                        action_row.child(list_item).into_any_element()
                                    } else {
                                        // Fallback for missing action
                                        div().h(px(style.row_height)).into_any_element()
                                    }
                                } else {
                                    // Fallback for missing filtered index
                                    div().h(px(style.row_height)).into_any_element()
                                }
                            }
                        }
                    } else {
                        // Fallback for out-of-bounds index
                        div().h(px(style.row_height)).into_any_element()
                    }
                })
            })
            .flex_1()
            .w_full();

            // Wrap list in a relative container with scrollbar overlay
            // Note: Using flex_1() to fill remaining space in flex column.
            // Do NOT use h_full() here as it can conflict with flex layout
            // and cause the search bar to be pushed off-screen.
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .pt(px(popup_theme.list.padding_top))
                .pb(px(popup_theme.list.padding_bottom))
                .on_scroll_wheel(cx.listener(|this, _event, _window, cx| {
                    this.actions_selection_user_moved = true;
                    this.clear_pending_scrollbar_offset();
                    this.trigger_scrollbar_activity(cx);
                    cx.propagate();
                }))
                // Always render the list to keep ListState in the render tree
                .child(variable_height_list)
                .child(scrollbar)
                // Overlay empty state message when no items match
                .when(is_empty, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(popup_theme.list.padding_top))
                            .left_0()
                            .w_full()
                            .h(px(style.row_height))
                            .flex()
                            .items_center()
                            .px(px(spacing.item_padding_x))
                            .child(crate::components::render_shared_empty_state(
                                crate::components::InfoEmptySurface::ActionsSearch,
                                &self.search_text,
                                &self.theme,
                                cx,
                            )),
                    )
                })
                .into_any_element()
        };

        // Use helper method for container colors
        let (main_bg, container_border, container_text) = self.get_container_colors(&colors);
        let use_vibrancy = self.theme.is_vibrancy_enabled();

        let show_search = self.search_is_visible();
        // Count items and section headers separately for accurate height calculation
        let mut section_header_count = 0_usize;
        let mut action_item_count = 0_usize;
        for item in &self.grouped_items {
            match item {
                GroupedActionItem::SectionHeader(_) => section_header_count += 1,
                GroupedActionItem::Item(_) => action_item_count += 1,
            }
        }

        // Shell height comes from the same formula that sizes the popup
        // NSWindow (`compute_popup_height` in window.rs), so the rendered
        // interior can never drift from the window bounds. Action dialogs are
        // footerless by contract (`config.show_footer` is forced off).
        let dynamic_height = super::window::actions_window_dynamic_height(
            action_item_count,
            section_header_count,
            !show_search,
            self.shows_context_header() && style.show_header,
            false,
            self.config.max_height,
            self.effective_row_height(),
        );
        let total_height = self.fixed_shell_height_px.unwrap_or(dynamic_height);

        // Build header row (section header style - non-interactive label)
        // Styled to match render_section_header() from list_item.rs:
        // - Smaller font (text_xs)
        // - Semibold weight
        // - Dimmed color (visually distinct from actionable items)
        let header_container = if self.shows_context_header() && style.show_header {
            self.context_title.as_ref().map(|title| {
                let header_text = if self.design_variant == DesignVariant::Default {
                    semantic_text_rgba(
                        self.theme.colors.text.primary,
                        self.theme.get_opacity().text_muted_alpha,
                    )
                } else {
                    semantic_text_rgba(
                        colors.text_primary,
                        self.theme.get_opacity().text_muted_alpha,
                    )
                };

                let header = div()
                    .w_full()
                    .h(px(popup_theme.context_header.height))
                    .px(px(popup_theme.context_header.padding_x))
                    .pt(px(popup_theme.context_header.padding_top))
                    .pb(px(popup_theme.context_header.padding_bottom))
                    .flex()
                    .flex_col()
                    .justify_center();

                header.child(
                    div()
                        .text_size(px(popup_theme.context_header.font_size))
                        .font_weight(popup_theme.context_header.font_weight)
                        .text_color(header_text)
                        .child(title.clone()),
                )
            })
        } else {
            None
        };

        // Main overlay popup container. In vibrancy mode the native popup
        // window's material owns the steady-state background; GPUI only paints
        // transient row/selection states.

        emit_actions_dialog_runtime_audit(&ActionsDialogRuntimeAudit::from_parts(
            "actions_dialog",
            &self.config,
            &style,
        ));

        // Canonical top-positioned search input shared by every searchable Actions surface.
        // No boxed input field or ⌘K prefix: text sits on clean chrome above the list separator.
        let input_container_top = if show_search {
            Some({
                let mut top_input = div()
                    .w_full() // Fill the container, which owns the shell width
                    .h(px(popup_theme.search.height)) // Fixed height for the input row
                    .min_h(px(popup_theme.search.height))
                    .max_h(px(popup_theme.search.height))
                    .overflow_hidden() // Prevent any content from causing shifts
                    .px(px(popup_theme.search.padding_x))
                    .py(px(
                        spacing.item_padding_y + popup_theme.search.padding_y_extra
                    ))
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(build_search_content());
                // Drill-down depth is otherwise invisible: surface the Escape
                // contract inline at the search row's trailing edge.
                if self.can_pop_route() {
                    top_input = top_input.child(
                        div()
                            .flex_none()
                            .ml(px(popup_theme.search.prefix_gap))
                            .text_size(px(popup_theme.search.font_size))
                            .text_color(hint_text_color)
                            .child(self.route_hint_label()),
                    );
                }
                if style.show_search_divider {
                    top_input = top_input.border_b_1().border_color(separator_color);
                }
                top_input
            })
        } else {
            None
        };

        let mut container = div().flex().flex_col();
        if self.fill_window_bounds {
            // Detached Actions fills the hosting bounds. The native window's
            // outer size is frozen at open; all later content mutations reflow
            // or scroll inside this full-height shell.
            container = container.w_full().h_full();
        } else {
            container = container.w(px(popup_theme.shell.width)).h(px(total_height));
        }
        let mut container = container
            .when(!use_vibrancy, |d| d.bg(main_bg))
            .rounded(px(popup_theme.shell.radius))
            .overflow_hidden()
            .text_color(container_text)
            .text_color(container_text)
            .key_context("actions_dialog")
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                    this.update_hovered_row_from_popup_y(f32::from(event.position.y), cx);
                }),
            );
        if style.show_container_border {
            container = container.border_1().border_color(container_border);
        }
        if !self.skip_track_focus {
            container = container.track_focus(&self.focus_handle);
        }

        let mut container = container;
        if let Some(input) = input_container_top {
            container = container.child(input);
        }
        if let Some(header) = header_container {
            container = container.child(header);
        }
        container = container.child(actions_container);
        container
    }
}

// --- Chrome contract audit ------------------------------------------------

/// Machine-readable audit of the Actions dialog chrome contract.
///
/// Both the live dialog and Storybook presenter can produce an audit,
/// enabling tests to assert visual parity without GPUI tree scraping.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize))]
pub(crate) struct ActionsDialogChromeAudit {
    /// `"sharp"` (no rounded corners), `"rounded"`, or `"rounded_glass"`.
    pub container_mode: &'static str,
    /// `"top"` or `"hidden"`.
    pub search_position: &'static str,
    /// Whether the search row renders a visible border/divider.
    pub shows_search_divider: bool,
    /// Whether the dialog container renders a visible border.
    pub show_container_border: bool,
    /// `"headers"`, `"separators"`, or `"none"`.
    pub section_mode: &'static str,
    /// Corner radius for row selection background (0 = sharp).
    pub row_radius: u16,
    /// Number of items in the footer hint strip (spec: none for actions dialogs).
    pub footer_hint_count: u8,
}

impl ActionsDialogChromeAudit {
    /// Audit the live dialog defaults against the `.impeccable.md` spec.
    pub(crate) fn from_live_defaults() -> Self {
        let style = actions_dialog_default_style();
        Self {
            container_mode: "rounded_glass",
            search_position: super::constants::ACTIONS_DIALOG_EXPECT_SEARCH_POSITION,
            shows_search_divider: style.show_search_divider,
            show_container_border: style.show_container_border,
            section_mode: "headers",
            row_radius: style.row_radius as u16,
            footer_hint_count: super::constants::ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT,
        }
    }

    /// Audit a Storybook presenter style.
    #[cfg(feature = "storybook")]
    pub(crate) fn from_storybook_style(
        style: &crate::storybook::actions_dialog_variations::ActionsDialogStyle,
    ) -> Self {
        Self {
            container_mode: "rounded_glass",
            search_position: super::constants::ACTIONS_DIALOG_EXPECT_SEARCH_POSITION,
            shows_search_divider: style.show_search_divider,
            show_container_border: style.show_container_border,
            section_mode: "headers",
            row_radius: style.row_radius as u16,
            footer_hint_count: super::constants::ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT,
        }
    }
}

// --- Runtime chrome contract audit -----------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub(crate) struct ActionsDialogRuntimeAudit {
    pub surface: &'static str,
    pub search_position: &'static str,
    pub section_mode: &'static str,
    pub shows_search_divider: bool,
    pub show_footer: bool,
    pub show_icons: bool,
    pub show_container_border: bool,
    pub footer_hint_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ActionsDialogRuntimeViolation {
    pub surface: &'static str,
    pub field: &'static str,
    pub expected: &'static str,
    pub actual: &'static str,
}

impl std::fmt::Display for ActionsDialogRuntimeViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "surface={} field={} expected={} actual={}",
            self.surface, self.field, self.expected, self.actual
        )
    }
}

#[inline]
fn actions_dialog_search_position_name(value: &super::types::SearchPosition) -> &'static str {
    match value {
        super::types::SearchPosition::Top => "top",
        super::types::SearchPosition::Hidden => "hidden",
    }
}

#[inline]
fn actions_dialog_section_mode_name(value: &super::types::SectionStyle) -> &'static str {
    match value {
        super::types::SectionStyle::Headers => "headers",
        super::types::SectionStyle::Separators => "separators",
        super::types::SectionStyle::None => "none",
    }
}

/// Surface-scoped expected contract for the Actions dialog.
///
/// `impeccable()` returns the `.impeccable.md` baseline. Future presets
/// (e.g. notes, main-menu) can define their own contracts by constructing
/// this struct with different values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub(crate) struct ActionsDialogExpectedContract {
    pub search_position: &'static str,
    pub shows_search_divider: bool,
    pub show_container_border: bool,
    pub show_footer: bool,
    pub footer_hint_count: u8,
}

impl ActionsDialogExpectedContract {
    pub(crate) const fn impeccable() -> Self {
        Self {
            search_position: super::constants::ACTIONS_DIALOG_EXPECT_SEARCH_POSITION,
            shows_search_divider: super::constants::ACTIONS_DIALOG_EXPECT_SEARCH_DIVIDER,
            show_container_border: super::constants::ACTIONS_DIALOG_EXPECT_CONTAINER_BORDER,
            show_footer: false,
            footer_hint_count: super::constants::ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT,
        }
    }
}

#[inline]
fn actions_dialog_bool_name(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

impl ActionsDialogRuntimeAudit {
    #[cfg(not(feature = "storybook"))]
    pub(crate) fn from_parts(
        surface: &'static str,
        config: &super::types::ActionsDialogConfig,
        style: &ActionsDialogStyleFallback,
    ) -> Self {
        Self {
            surface,
            search_position: actions_dialog_search_position_name(&config.search_position),
            section_mode: actions_dialog_section_mode_name(&config.section_style),
            shows_search_divider: style.show_search_divider,
            show_footer: config.show_footer,
            show_icons: config.show_icons && style.show_icons,
            show_container_border: style.show_container_border,
            footer_hint_count: if config.show_footer { 2 } else { 0 },
        }
    }

    #[cfg(feature = "storybook")]
    pub(crate) fn from_parts(
        surface: &'static str,
        config: &super::types::ActionsDialogConfig,
        style: &crate::storybook::actions_dialog_variations::ActionsDialogStyle,
    ) -> Self {
        Self {
            surface,
            search_position: actions_dialog_search_position_name(&config.search_position),
            section_mode: actions_dialog_section_mode_name(&config.section_style),
            shows_search_divider: style.show_search_divider,
            show_footer: config.show_footer,
            show_icons: config.show_icons && style.show_icons,
            show_container_border: style.show_container_border,
            footer_hint_count: if config.show_footer { 2 } else { 0 },
        }
    }

    pub(crate) fn validate_against(
        &self,
        expected: &ActionsDialogExpectedContract,
    ) -> Vec<ActionsDialogRuntimeViolation> {
        let mut violations = Vec::new();
        if self.search_position != expected.search_position && self.search_position != "hidden" {
            violations.push(ActionsDialogRuntimeViolation {
                surface: self.surface,
                field: "search_position",
                expected: expected.search_position,
                actual: self.search_position,
            });
        }
        if self.shows_search_divider != expected.shows_search_divider {
            violations.push(ActionsDialogRuntimeViolation {
                surface: self.surface,
                field: "shows_search_divider",
                expected: actions_dialog_bool_name(expected.shows_search_divider),
                actual: actions_dialog_bool_name(self.shows_search_divider),
            });
        }
        if self.section_mode == "separators" {
            violations.push(ActionsDialogRuntimeViolation {
                surface: self.surface,
                field: "section_mode",
                expected: "headers_or_none",
                actual: "separators",
            });
        }
        if self.show_container_border != expected.show_container_border {
            violations.push(ActionsDialogRuntimeViolation {
                surface: self.surface,
                field: "show_container_border",
                expected: actions_dialog_bool_name(expected.show_container_border),
                actual: actions_dialog_bool_name(self.show_container_border),
            });
        }
        if self.show_footer != expected.show_footer {
            violations.push(ActionsDialogRuntimeViolation {
                surface: self.surface,
                field: "show_footer",
                expected: actions_dialog_bool_name(expected.show_footer),
                actual: actions_dialog_bool_name(self.show_footer),
            });
        }
        if self.footer_hint_count != expected.footer_hint_count {
            violations.push(ActionsDialogRuntimeViolation {
                surface: self.surface,
                field: "footer_hint_count",
                expected: if expected.footer_hint_count == 0 {
                    "0"
                } else {
                    "not_0"
                },
                actual: if self.footer_hint_count == 0 {
                    "0"
                } else {
                    "not_0"
                },
            });
        }
        violations
    }

    pub(crate) fn validate(&self) -> Vec<ActionsDialogRuntimeViolation> {
        self.validate_against(&ActionsDialogExpectedContract::impeccable())
    }
}

fn seen_actions_dialog_runtime_audits(
) -> &'static std::sync::Mutex<std::collections::HashSet<ActionsDialogRuntimeAudit>> {
    static SEEN: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashSet<ActionsDialogRuntimeAudit>>,
    > = std::sync::OnceLock::new();
    SEEN.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn mark_actions_dialog_runtime_audit_seen(audit: &ActionsDialogRuntimeAudit) -> bool {
    let mut seen = seen_actions_dialog_runtime_audits()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    seen.insert(audit.clone())
}

fn emit_actions_dialog_runtime_audit(audit: &ActionsDialogRuntimeAudit) {
    if !mark_actions_dialog_runtime_audit_seen(audit) {
        return;
    }
    tracing::info!(
        target: "script_kit::actions_chrome",
        event = "actions_dialog_runtime_audit",
        surface = audit.surface,
        search_position = audit.search_position,
        section_mode = audit.section_mode,
        shows_search_divider = audit.shows_search_divider,
        show_footer = audit.show_footer,
        show_icons = audit.show_icons,
        show_container_border = audit.show_container_border,
        footer_hint_count = audit.footer_hint_count,
        "actions dialog runtime audit"
    );
    for violation in audit.validate() {
        tracing::warn!(
            target: "script_kit::actions_chrome",
            event = "actions_dialog_runtime_contract_violation",
            surface = violation.surface,
            field = violation.field,
            expected = violation.expected,
            actual = violation.actual,
            message = %violation,
            "actions dialog runtime contract violation"
        );
    }
}

// --- merged from part_05.rs ---

#[cfg(test)]
mod tests {
    use super::{
        action_has_routable_shortcut, action_shortcut_parity_report, action_subtitle_for_display,
        actions_dialog_fixed_shell_viewport_height, actions_dialog_revealed_scroll_top,
        actions_dialog_scrollbar_fade_duration, actions_dialog_scrollbar_fade_opacity,
        actions_dialog_scrollbar_viewport_height, clear_duplicate_action_shortcuts,
        displayed_action_keybinding_specs, first_selectable_index, is_destructive_action,
        last_selectable_index, matching_action_id_for_keystroke,
        matching_filtered_action_id_for_keystroke, resolve_visible_action_shortcut,
        resolved_actions_dialog_row_metrics, selectable_index_at_or_after,
        selectable_index_at_or_before, should_render_section_separator,
        visible_action_shortcut_bindings, ActionsDialog, ActionsDialogChromeAudit,
        ActionsDialogRuntimeAudit, GroupedActionItem, MainListDisplayedActionShortcut,
    };
    use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
    use crate::menu_syntax::{MenuSyntaxAction, MenuSyntaxActionKind};
    use crate::menu_syntax_actions::{PowerSyntaxActionSection, SectionMode};

    #[test]
    fn actions_marker_centers_in_compact_host_without_changing_list_item_content_geometry() {
        let popup = crate::designs::base_actions_popup_theme();
        let main = crate::designs::MainMenuThemeVariant::InfoBarBase.def();
        let baseline = crate::list_item::ListItemMetricsOverride::from_main_menu_def(main);
        let metrics = resolved_actions_dialog_row_metrics(&popup, main);
        let marker = crate::list_item::list_item_selection_marker_geometry(metrics, true)
            .expect("selected Actions row has a marker");

        assert_eq!(metrics.item_height, baseline.item_height);
        assert_eq!(metrics.row_inner_padding_x, baseline.row_inner_padding_x);
        assert_eq!(metrics.icon_text_gap, baseline.icon_text_gap);
        assert_eq!(metrics.accessory_gap, baseline.accessory_gap);
        assert_eq!(
            metrics.row_selected_marker_center_height,
            popup.list.row_height
        );
        assert_eq!(marker.top, (popup.list.row_height - marker.height) / 2.0);
    }

    #[test]
    fn selectable_index_helpers_skip_section_headers_directionally() {
        let rows = vec![
            GroupedActionItem::SectionHeader("One".to_string()),
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("Two".to_string()),
            GroupedActionItem::Item(1),
        ];

        assert_eq!(first_selectable_index(&rows), Some(1));
        assert_eq!(last_selectable_index(&rows), Some(3));
        assert_eq!(selectable_index_at_or_before(&rows, 2), Some(1));
        assert_eq!(selectable_index_at_or_after(&rows, 2), Some(3));
    }

    #[test]
    fn destructive_detection_matches_known_ids() {
        let remove_action = Action::new(
            "remove_alias",
            "Remove Alias",
            Some("Remove alias".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(is_destructive_action(&remove_action));

        let trash_action = Action::new(
            "move_to_trash",
            "Move to Trash",
            Some("Move item to Trash".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(is_destructive_action(&trash_action));
    }

    #[test]
    fn destructive_detection_matches_title_prefix_fallback() {
        let delete_action = Action::new(
            "custom_action",
            "Delete Export Cache",
            Some("Delete cached export".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(is_destructive_action(&delete_action));

        let safe_action = Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy path".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(!is_destructive_action(&safe_action));
    }

    #[test]
    fn build_actions_applies_power_syntax_replace_and_prepend_modes() {
        fn focused_script() -> ScriptInfo {
            ScriptInfo {
                name: "Demo Script".to_string(),
                path: "/tmp/demo-script.ts".to_string(),
                is_script: true,
                action_verb: "Run".to_string(),
                ..ScriptInfo::default()
            }
        }

        fn power_syntax_section(mode: SectionMode) -> PowerSyntaxActionSection {
            PowerSyntaxActionSection {
                title: "Power Syntax".to_string(),
                mode,
                actions: vec![MenuSyntaxAction {
                    id: "capture.cancel".to_string(),
                    label: "Cancel without saving".to_string(),
                    kind: MenuSyntaxActionKind::Cancel,
                    enabled: true,
                }],
            }
        }

        let focused_script = Some(focused_script());
        let normal_actions = ActionsDialog::build_actions(&focused_script, &None, &None, &None);
        assert!(
            normal_actions
                .iter()
                .any(|action| action.id == "run_script"),
            "fixture must include normal selected-row actions"
        );

        let replace_section = Some(power_syntax_section(SectionMode::Replace));
        let replace_actions =
            ActionsDialog::build_actions(&focused_script, &None, &replace_section, &None);
        assert_eq!(replace_actions.len(), 1);
        assert_eq!(replace_actions[0].id, "menu_syntax:capture.cancel");
        assert!(
            !replace_actions
                .iter()
                .any(|action| action.id == "run_script"),
            "replace mode must wipe normal selected-row actions"
        );

        let prepend_section = Some(power_syntax_section(SectionMode::Prepend));
        let prepend_actions =
            ActionsDialog::build_actions(&focused_script, &None, &prepend_section, &None);
        assert_eq!(prepend_actions[0].id, "menu_syntax:capture.cancel");
        assert_eq!(&prepend_actions[1..], normal_actions.as_slice());
        assert!(
            prepend_actions[1..]
                .iter()
                .any(|action| action.id == "run_script"),
            "prepend mode must keep normal selected-row actions after Power Syntax"
        );
    }

    /// The actions menu is contextual to the focused item. Global/Discover
    /// rows and Agent Chat prompt export/handoff rows must never trail a
    /// focused item's actions; global rows are strictly a fallback for hosts
    /// where the focused row contributes nothing (so Cmd+K never opens empty).
    #[test]
    fn build_actions_keeps_global_rows_out_of_item_focused_menus() {
        let focused_script = Some(ScriptInfo {
            name: "Demo Script".to_string(),
            path: "/tmp/demo-script.ts".to_string(),
            is_script: true,
            action_verb: "Run".to_string(),
            ..ScriptInfo::default()
        });

        let focused_actions = ActionsDialog::build_actions(&focused_script, &None, &None, &None);
        assert!(
            focused_actions.iter().all(|action| {
                action.id != "reload_scripts"
                    && action.id != "sdk_reference"
                    && action.id != "open_settings_menu"
                    && !action.id.starts_with("prompt-action/")
                    && !action.id.starts_with("prompt-target/")
            }),
            "focused-item menu must not include global/Discover/prompt rows: {:?}",
            focused_actions
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>()
        );

        let fallback_actions = ActionsDialog::build_actions(&None, &None, &None, &None);
        assert!(
            fallback_actions
                .iter()
                .any(|action| action.id == "reload_scripts"),
            "empty-context menu must fall back to global rows"
        );
        assert!(
            fallback_actions.iter().all(|action| {
                !action.id.starts_with("prompt-action/") && !action.id.starts_with("prompt-target/")
            }),
            "prompt export/handoff rows are Agent Chat-owned and must not appear in the global fallback"
        );

        let host_section = Some(vec![Action::new(
            "day_page_today",
            "Today",
            Some("Host-owned row".to_string()),
            ActionCategory::ScriptContext,
        )]);
        let host_actions = ActionsDialog::build_actions(&None, &None, &None, &host_section);
        assert!(
            host_actions
                .iter()
                .all(|action| action.id != "reload_scripts"),
            "host-owned sections count as context and must suppress the global fallback"
        );
    }

    #[test]
    fn section_separator_only_shows_on_section_boundary() {
        let actions = vec![
            Action::new(
                "run_script",
                "Run Script",
                Some("Run".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Actions"),
            Action::new(
                "edit_script",
                "Edit Script",
                Some("Edit".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Edit"),
            Action::new(
                "copy_path",
                "Copy Path",
                Some("Copy".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Share"),
            Action::new(
                "copy_deeplink",
                "Copy Deeplink",
                Some("Copy".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Share"),
        ];
        let filtered_actions = vec![0, 1, 2, 3];

        assert!(!should_render_section_separator(
            &actions,
            &filtered_actions,
            0
        ));
        assert!(should_render_section_separator(
            &actions,
            &filtered_actions,
            1
        ));
        assert!(should_render_section_separator(
            &actions,
            &filtered_actions,
            2
        ));
        assert!(!should_render_section_separator(
            &actions,
            &filtered_actions,
            3
        ));
    }

    #[test]
    fn test_scrollbar_viewport_subtracts_header_footer_and_search_height() {
        let total_content_height = 500.0;
        let viewport_height = actions_dialog_scrollbar_viewport_height(
            total_content_height,
            true,
            true,
            true,
            crate::actions::constants::POPUP_MAX_HEIGHT,
        );

        // POPUP_MAX_HEIGHT (400) - search (40) - context header (26) - footer (32)
        // - list padding (top 0 + bottom 6)
        assert_eq!(viewport_height, 296.0);
    }

    #[test]
    fn test_scrollbar_viewport_clamps_to_content_when_content_shorter_than_viewport() {
        let total_content_height = 120.0;
        let viewport_height = actions_dialog_scrollbar_viewport_height(
            total_content_height,
            true,
            true,
            true,
            crate::actions::constants::POPUP_MAX_HEIGHT,
        );

        assert_eq!(viewport_height, 120.0);
    }

    #[test]
    fn ux13_fixed_shell_viewport_does_not_clamp_to_short_or_empty_content() {
        let shell_height = 300.0;
        let viewport = actions_dialog_fixed_shell_viewport_height(shell_height, true, true, false);
        let tokens = crate::designs::current_actions_popup_theme();
        let expected = shell_height
            - tokens.shell.border_height
            - tokens.search.height
            - tokens.context_header.height
            - tokens.list.padding_top
            - tokens.list.padding_bottom;

        assert_eq!(viewport, expected);
        assert!(
            viewport > 120.0,
            "short content must leave safe empty space"
        );
    }

    #[test]
    fn ux13_hidden_search_preserves_the_same_shell_for_external_filtering() {
        let tokens = crate::designs::current_actions_popup_theme();
        let searchable = actions_dialog_fixed_shell_viewport_height(300.0, true, false, false);
        let hidden = actions_dialog_fixed_shell_viewport_height(300.0, false, false, false);

        assert_eq!(hidden - searchable, tokens.search.height);
    }

    #[test]
    fn test_scrollbar_reveal_offset_moves_down_when_selection_leaves_viewport() {
        let offset = actions_dialog_revealed_scroll_top(0.0, 120.0, 400.0, 144.0, 180.0);

        assert_eq!(offset, 60.0);
    }

    #[test]
    fn test_scrollbar_reveal_offset_moves_up_when_selection_is_above_viewport() {
        let offset = actions_dialog_revealed_scroll_top(160.0, 120.0, 400.0, 72.0, 108.0);

        assert_eq!(offset, 72.0);
    }

    #[test]
    fn test_scrollbar_reveal_offset_keeps_current_top_when_selection_is_visible() {
        let offset = actions_dialog_revealed_scroll_top(72.0, 120.0, 400.0, 96.0, 132.0);

        assert_eq!(offset, 72.0);
    }

    #[test]
    fn test_scrollbar_reveal_offset_clamps_to_max_scroll() {
        let offset = actions_dialog_revealed_scroll_top(240.0, 120.0, 300.0, 288.0, 324.0);

        assert_eq!(offset, 180.0);
    }

    #[test]
    fn test_scrollbar_fade_duration_matches_shared_scroll_feel() {
        assert_eq!(
            actions_dialog_scrollbar_fade_duration(),
            crate::transitions::DURATION_MEDIUM + std::time::Duration::from_millis(50)
        );
    }

    #[test]
    fn test_scrollbar_fade_opacity_starts_visible_and_ends_hidden() {
        assert_eq!(
            actions_dialog_scrollbar_fade_opacity(0.0),
            crate::transitions::Opacity::VISIBLE
        );
        assert_eq!(
            actions_dialog_scrollbar_fade_opacity(1.0),
            crate::transitions::Opacity::INVISIBLE
        );
    }

    #[test]
    fn test_action_subtitle_for_display_gates_on_show_subtitles() {
        let action_with_description = Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy the selected path".to_string()),
            ActionCategory::ScriptContext,
        );
        let action_without_description = Action::new(
            "run_script",
            "Run Script",
            None,
            ActionCategory::ScriptContext,
        );

        // Action-menu hosts (show_subtitles = false) stay title-only.
        assert_eq!(
            action_subtitle_for_display(&action_with_description, false),
            None
        );
        // Switcher-style hosts opt in to render the description line.
        assert_eq!(
            action_subtitle_for_display(&action_with_description, true),
            Some("Copy the selected path")
        );
        assert_eq!(
            action_subtitle_for_display(&action_without_description, true),
            None
        );
    }

    #[test]
    fn test_matching_action_id_for_keystroke_uses_canonical_shortcut_normalization() {
        let actions = vec![
            Action::new(
                "history",
                "Agent Chat History",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘P"),
            Action::new(
                "copy_last_response",
                "Copy Last Response",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⇧⌘C"),
        ];

        let mut cmd_only = gpui::Modifiers::default();
        cmd_only.platform = true;
        assert_eq!(
            matching_action_id_for_keystroke(&actions, "p", &cmd_only),
            Some("history".to_string())
        );

        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;
        assert_eq!(
            matching_action_id_for_keystroke(&actions, "c", &shift_cmd),
            Some("copy_last_response".to_string())
        );

        assert_eq!(
            matching_action_id_for_keystroke(&actions, "x", &cmd_only),
            None
        );
    }

    #[test]
    fn test_matching_filtered_action_id_for_keystroke_ignores_hidden_actions() {
        let actions = vec![
            Action::new("rename_path", "Rename", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘R"),
            Action::new(
                "file:refresh_directory",
                "Refresh Directory",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘R"),
        ];

        let mut cmd_only = gpui::Modifiers::default();
        cmd_only.platform = true;

        assert_eq!(
            matching_filtered_action_id_for_keystroke(&actions, &[1], "r", &cmd_only),
            Some("file:refresh_directory".to_string())
        );
    }

    #[test]
    fn cmd_shift_k_matches_add_shortcut_display_shortcut() {
        let actions = vec![Action::new(
            "add_shortcut",
            "Add Keyboard Shortcut",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧K")];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            matching_action_id_for_keystroke(&actions, "k", &shift_cmd),
            Some("add_shortcut".to_string())
        );
    }

    #[test]
    fn cmd_shift_k_matches_builtin_add_shortcut_display_shortcut() {
        let builtin = ScriptInfo::with_all(
            "Theme Designer",
            "builtin:builtin/choose-theme",
            false,
            "Open",
            None,
            None,
        );
        let actions = crate::actions::get_script_context_actions(&builtin);
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        let add_shortcut = actions
            .iter()
            .find(|action| action.id == "add_shortcut")
            .expect("built-ins without an assigned shortcut must expose add_shortcut");
        assert_eq!(add_shortcut.shortcut.as_deref(), Some("⌘⇧K"));
        assert_eq!(
            matching_action_id_for_keystroke(&actions, "K", &shift_cmd),
            Some("add_shortcut".to_string())
        );
    }

    #[test]
    fn cmd_shift_k_matches_update_shortcut_display_shortcut() {
        let actions = vec![Action::new(
            "update_shortcut",
            "Edit Keyboard Shortcut",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧K")];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            matching_action_id_for_keystroke(&actions, "k", &shift_cmd),
            Some("update_shortcut".to_string())
        );
    }

    #[test]
    fn visible_shortcut_router_ignores_filtered_out_add_shortcut() {
        let actions = vec![
            Action::new(
                "add_shortcut",
                "Add Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧K"),
            Action::new(
                "copy_path",
                "Copy Path",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧C"),
        ];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            matching_filtered_action_id_for_keystroke(&actions, &[1], "k", &shift_cmd),
            None
        );
    }

    #[test]
    fn disabled_action_shortcut_is_neither_displayed_nor_routable() {
        let actions = vec![Action::new(
            "disabled",
            "Unavailable",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘D")
        .disabled("Requires a selected file")];
        let mut command = gpui::Modifiers::default();
        command.platform = true;

        assert!(visible_action_shortcut_bindings(&actions, &[0]).is_empty());
        assert_eq!(
            resolve_visible_action_shortcut(&actions, &[0], "d", &command),
            None
        );
        let report = action_shortcut_parity_report(&actions, &[0]);
        assert_eq!(report.displayed_shortcut_count, 0);
        assert_eq!(report.routable_shortcut_count, 0);
    }

    #[test]
    fn duplicate_visible_shortcuts_do_not_create_two_executable_routes() {
        let actions = vec![
            Action::new(
                "add_shortcut",
                "Add Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧K"),
            Action::new(
                "update_shortcut",
                "Edit Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+shift+k"),
        ];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            resolve_visible_action_shortcut(&actions, &[0, 1], "k", &shift_cmd),
            None
        );
        let report = action_shortcut_parity_report(&actions, &[0, 1]);
        assert_eq!(report.displayed_shortcut_count, 0);
        assert_eq!(report.routable_shortcut_count, 0);
        assert_eq!(report.duplicate_shortcut_count, 2);
        assert!(report.unroutable_displayed_shortcuts.is_empty());
        assert!(!action_has_routable_shortcut(
            &actions,
            &[0, 1],
            "add_shortcut"
        ));
        assert!(!action_has_routable_shortcut(
            &actions,
            &[0, 1],
            "update_shortcut"
        ));
    }

    #[test]
    fn displayed_action_keybinding_specs_are_generated_from_routable_metadata() {
        let actions = vec![Action::new(
            "add_shortcut",
            "Add Keyboard Shortcut",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧K")];

        let specs = displayed_action_keybinding_specs(&actions, &[0]);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].canonical, "cmd+shift+k");
        assert_eq!(specs[0].gpui_keystroke, "cmd-shift-k");
    }

    #[test]
    fn duplicate_displayed_action_shortcuts_do_not_generate_keybindings() {
        let actions = vec![
            Action::new(
                "add_shortcut",
                "Add Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧K"),
            Action::new(
                "update_shortcut",
                "Edit Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+shift+k"),
        ];

        let specs = displayed_action_keybinding_specs(&actions, &[0, 1]);

        assert!(specs.is_empty());
    }

    #[test]
    fn generated_displayed_action_binding_is_receivable_in_script_list_context() {
        let binding = gpui::KeyBinding::new(
            "cmd-shift-k",
            MainListDisplayedActionShortcut {
                shortcut: "cmd+shift+k".to_string(),
            },
            Some("script_list"),
        );
        let mut keymap = gpui::Keymap::default();
        keymap.add_bindings([binding]);

        let (matches, pending) = keymap.bindings_for_input(
            &[gpui::Keystroke::parse("cmd-shift-k").unwrap()],
            &[gpui::KeyContext::parse("script_list").unwrap()],
        );

        assert!(!pending);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_clear_duplicate_action_shortcuts_keeps_first_visible_binding() {
        let mut actions = vec![
            Action::new("rename_path", "Rename", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘R"),
            Action::new(
                "file:refresh_directory",
                "Refresh Directory",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+r"),
            Action::new(
                "file:sort_name_asc",
                "Sort by Name",
                None,
                ActionCategory::ScriptContext,
            ),
        ];

        clear_duplicate_action_shortcuts(&mut actions);

        assert_eq!(actions[0].shortcut.as_deref(), Some("⌘R"));
        assert_eq!(actions[1].shortcut, None);
        assert_eq!(actions[1].shortcut_tokens, None);
        assert_eq!(actions[1].shortcut_lower, None);
        assert_eq!(actions[2].shortcut, None);
    }

    #[test]
    fn test_create_popup_shadow_returns_visible_shadow() {
        let shadows = ActionsDialog::create_popup_shadow();

        assert!(shadows.is_empty());
    }

    // ── Chrome contract tests (.impeccable.md) ──────────────────────────

    /// The live dialog omits a footer so shortcuts stay inline with rows.
    #[test]
    fn actions_dialog_omits_footer_hints() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.footer_hint_count, 0,
            "actions dialog must not show footer hints; shortcuts live in rows"
        );
    }

    /// The Storybook presenter must use the same rounded glass container mode
    /// as the live dialog.
    #[test]
    fn actions_dialog_story_presenter_uses_rounded_glass_container() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.container_mode, "rounded_glass",
            "container must expose Tahoe rounded glass chrome"
        );

        // Also verify the Storybook "current" variant agrees
        #[cfg(feature = "storybook")]
        {
            let (style, _) =
                crate::storybook::actions_dialog_variations::resolve_actions_dialog_style(Some(
                    "current",
                ));
            let story_audit = ActionsDialogChromeAudit::from_storybook_style(&style);
            assert_eq!(
                audit, story_audit,
                "live and storybook chrome audits must agree"
            );
        }
    }

    /// The search row must NOT render a divider/border — bare input per
    /// `.impeccable.md`: "Bare, no border, no background box."
    #[test]
    fn actions_dialog_story_presenter_does_not_render_search_divider() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert!(
            !audit.shows_search_divider,
            "search row must not show a divider per .impeccable.md bare input rule"
        );

        #[cfg(feature = "storybook")]
        {
            let (style, _) =
                crate::storybook::actions_dialog_variations::resolve_actions_dialog_style(Some(
                    "current",
                ));
            assert!(
                !style.show_search_divider,
                "storybook current variant must not show search divider"
            );
        }
    }

    /// Section grouping must use `SectionStyle::Headers` (spacing-defined
    /// groups), never `SectionStyle::Separators` (inline separator lines).
    #[test]
    fn actions_dialog_section_headers_require_header_mode_not_separator_mode() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.section_mode, "headers",
            "section style must be headers per .impeccable.md — no separator lines"
        );

        // Verify the default ActionsDialogConfig uses Headers
        let config = crate::actions::types::ActionsDialogConfig::default();
        assert_eq!(
            config.section_style,
            SectionStyle::Headers,
            "ActionsDialogConfig default must be SectionStyle::Headers"
        );
    }

    // ── Runtime audit tests ────────────────────────────────────────────

    #[test]
    fn actions_dialog_runtime_audit_reflects_actual_config() {
        use crate::actions::types::{ActionsDialogConfig, AnchorPosition, SearchPosition};
        let mut style = super::actions_dialog_default_style();
        // Use spec-compliant style for a clean validation pass.
        style.show_container_border = false;
        style.show_icons = true;
        let audit = ActionsDialogRuntimeAudit::from_parts(
            "test_actions_dialog",
            &ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Top,
                show_icons: true,
                show_footer: false,
                ..ActionsDialogConfig::default()
            },
            &style,
        );
        assert_eq!(audit.search_position, "top");
        assert_eq!(audit.section_mode, "headers");
        assert!(audit.show_icons);
        assert!(!audit.show_footer);
        assert!(!audit.shows_search_divider);
        assert!(audit.validate().is_empty());
    }

    #[test]
    fn actions_dialog_footerless_config_normalizes_legacy_footer_flag() {
        use crate::actions::types::ActionsDialogConfig;

        let config = super::actions_dialog_footerless_config(ActionsDialogConfig {
            show_footer: true,
            ..ActionsDialogConfig::default()
        });

        assert!(
            !config.show_footer,
            "actions dialogs should normalize legacy footer state to match the footerless render path"
        );
    }

    #[test]
    fn actions_dialog_runtime_audit_reports_resolved_icon_visibility() {
        use crate::actions::types::ActionsDialogConfig;
        let mut style = super::actions_dialog_default_style();
        style.show_icons = false;

        let audit = ActionsDialogRuntimeAudit::from_parts(
            "test_actions_dialog",
            &ActionsDialogConfig {
                show_icons: true,
                ..ActionsDialogConfig::default()
            },
            &style,
        );

        assert!(
            !audit.show_icons,
            "runtime audit should report rendered icon visibility, not only requested config"
        );
    }

    #[test]
    fn actions_dialog_runtime_audit_flags_separator_and_divider_regressions() {
        use crate::actions::types::{ActionsDialogConfig, AnchorPosition, SearchPosition};
        let mut style = super::actions_dialog_default_style();
        style.show_search_divider = true;
        style.show_container_border = true;
        let audit = ActionsDialogRuntimeAudit::from_parts(
            "test_actions_dialog",
            &ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Separators,
                anchor: AnchorPosition::Top,
                show_icons: true,
                show_footer: false,
                ..ActionsDialogConfig::default()
            },
            &style,
        );
        let violations = audit.validate();
        assert!(violations.iter().any(|v| v.field == "shows_search_divider"));
        assert!(violations.iter().any(|v| v.field == "section_mode"));
        assert!(violations
            .iter()
            .any(|v| v.field == "show_container_border"));
    }
}

// ── Focused spec tests (cargo test actions_dialog_spec_tests --lib) ──────

#[cfg(test)]
mod actions_dialog_spec_tests {
    use super::{
        ActionsDialogChromeAudit, ActionsDialogExpectedContract, ActionsDialogRuntimeAudit,
    };

    #[test]
    fn live_defaults_match_impeccable_contract() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(audit.container_mode, "rounded_glass");
        assert_eq!(audit.search_position, "top");
        assert!(!audit.shows_search_divider);
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.footer_hint_count, 0);
    }

    #[test]
    fn runtime_audit_flags_bottom_search() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "search_position"),
            "bottom search position should fail verification"
        );
    }

    #[test]
    fn ux13_runtime_audit_accepts_intentionally_hidden_search() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "external_search_surface",
            search_position: "hidden",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: false,
            show_container_border: false,
            footer_hint_count: 0,
        };

        assert!(audit.validate().is_empty());
    }

    #[test]
    fn runtime_audit_flags_visible_search_divider() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: true,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "shows_search_divider"),
            "visible search divider should fail verification"
        );
    }

    #[test]
    fn runtime_audit_flags_separator_sections() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "separators",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit.validate().iter().any(|v| v.field == "section_mode"),
            "separator sections should fail verification"
        );
    }

    #[test]
    fn runtime_audit_flags_visible_container_border() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "show_container_border"),
            "visible container border should fail verification"
        );
    }

    #[test]
    fn runtime_audit_flags_any_footer_presence() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: true,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 2,
        };
        assert!(
            audit.validate().iter().any(|v| v.field == "show_footer"),
            "any footer should fail verification"
        );
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "footer_hint_count"),
            "non-zero footer hint count should fail verification"
        );
    }

    #[test]
    fn spec_compliant_audit_passes_clean() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit.validate().is_empty(),
            "fully spec-compliant audit should produce zero violations"
        );
    }

    // ── Contract struct tests ─────────────────────────────────────────

    #[test]
    fn actions_dialog_live_defaults_match_top_search_contract() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.search_position,
            super::super::constants::ACTIONS_DIALOG_EXPECT_SEARCH_POSITION,
            "search position must match .impeccable.md top-search rule"
        );
    }

    #[test]
    fn actions_dialog_live_defaults_hide_container_border() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        let style = super::actions_dialog_default_style();
        assert_eq!(
            audit.show_container_border, style.show_container_border,
            "chrome audit must reflect the actual live style value"
        );
        assert!(
            !audit.show_container_border,
            "live actions dialog defaults should stay footerless and borderless"
        );
    }

    #[test]
    fn actions_dialog_expected_contract_impeccable_matches_constants() {
        let contract = ActionsDialogExpectedContract::impeccable();
        assert_eq!(contract.search_position, "top");
        assert!(!contract.shows_search_divider);
        assert!(!contract.show_container_border);
        assert!(!contract.show_footer);
        assert_eq!(contract.footer_hint_count, 0);
    }

    #[test]
    fn actions_dialog_runtime_audit_reports_search_position_and_border_violations() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "actions_dialog.current",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        let violations = audit.validate_against(&ActionsDialogExpectedContract::impeccable());
        assert!(
            violations.iter().any(|v| v.field == "search_position"
                && v.expected == "top"
                && v.actual == "bottom"),
            "expected a search_position violation"
        );
        assert!(
            violations.iter().any(|v| v.field == "show_container_border"
                && v.expected == "false"
                && v.actual == "true"),
            "expected a show_container_border violation"
        );
    }

    #[test]
    fn actions_dialog_runtime_violations_serialize_as_machine_readable_json() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "actions_dialog.current",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        let violations = audit.validate_against(&ActionsDialogExpectedContract::impeccable());
        let json = serde_json::to_string(&violations).expect("serialize violations");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse violations");
        assert_eq!(value[0]["surface"], "actions_dialog.current");
        assert_eq!(value[0]["field"], "search_position");
        assert_eq!(value[0]["expected"], "top");
        assert_eq!(value[0]["actual"], "bottom");
    }

    #[test]
    fn actions_dialog_validate_delegates_to_validate_against_impeccable() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        let via_validate = audit.validate();
        let via_validate_against =
            audit.validate_against(&ActionsDialogExpectedContract::impeccable());
        assert_eq!(via_validate, via_validate_against);
    }
}

// ── Click contract tests ─────────────────────────────────────────────

#[cfg(test)]
mod actions_dialog_click_contract_tests {
    use super::should_submit_actions_dialog_row_click;

    #[test]
    fn actions_dialog_requires_second_single_click_after_mouse_selection() {
        assert!(!should_submit_actions_dialog_row_click(false, 1));
        assert!(should_submit_actions_dialog_row_click(true, 1));
    }

    #[test]
    fn actions_dialog_still_submits_on_native_double_click() {
        assert!(should_submit_actions_dialog_row_click(false, 2));
        assert!(should_submit_actions_dialog_row_click(false, 3));
    }
}

// --- merged from part_01.rs ---
// CommandBar - Reusable command palette component
//
// A high-level wrapper around ActionsDialog that provides:
// - Consistent window management (open/close/toggle)
// - Focus handling
// - Keyboard routing
// - Configuration presets for different contexts (main menu, AI chat, etc.)
//
// # Usage
//
// ```rust,ignore
// // Create a CommandBar with actions and config
// let command_bar = CommandBar::new(
//     actions,
//     CommandBarConfig::ai_style(),
//     theme,
//     cx,
// );
//
// // Toggle with Cmd+K
// command_bar.toggle(window, cx);
//
// // Handle selected action
// if let Some(action_id) = command_bar.get_selected_action_id(cx) {
//     execute_action(&action_id);
// }
// ```

use super::constants::NOTES_RECENT_POPUP_MAX_HEIGHT;
use super::dialog::{
    first_selectable_index, last_selectable_index, selectable_index_at_or_after,
    selectable_index_at_or_before, GroupedActionItem,
};
use super::types::{Action, ActionsDialogConfig, AnchorPosition, SearchPosition, SectionStyle};
use super::window::{
    close_actions_window, is_actions_window_open, notify_actions_window, open_actions_window,
};
use super::ActionsDialog;
use crate::logging;
use crate::theme;
use crate::ui_foundation::{is_key_backspace, is_key_down, is_key_enter, is_key_escape, is_key_up};
use gpui::{App, AppContext, Context, Entity, FocusHandle, Window};
use itertools::Itertools;
use std::sync::Arc;

const COMMAND_BAR_PAGE_JUMP: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandBarKeyIntent {
    MoveUp,
    MoveDown,
    MoveHome,
    MoveEnd,
    MovePageUp,
    MovePageDown,
    ExecuteSelected,
    Close,
    Backspace,
    /// Option+Backspace: delete the trailing word, like the main search input.
    BackspaceWord,
    MoveCursorLeft {
        select: bool,
    },
    MoveCursorRight {
        select: bool,
    },
    TypeChar(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandBarOpenFeedbackAction {
    OpenWindow,
}

impl CommandBarOpenFeedbackAction {
    fn success_log(self, position: super::window::WindowPosition) -> String {
        match self {
            Self::OpenWindow => format!("Command bar opened at {position:?}"),
        }
    }

    fn failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::OpenWindow => format!("Failed to open command bar: {error}"),
        }
    }
}

#[inline]
fn is_non_text_named_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("tab")
        || key.eq_ignore_ascii_case("left")
        || key.eq_ignore_ascii_case("arrowleft")
        || key.eq_ignore_ascii_case("right")
        || key.eq_ignore_ascii_case("arrowright")
        || key.eq_ignore_ascii_case("shift")
        || key.eq_ignore_ascii_case("control")
        || key.eq_ignore_ascii_case("alt")
        || key.eq_ignore_ascii_case("meta")
        || key.eq_ignore_ascii_case("cmd")
        || key.eq_ignore_ascii_case("command")
        || key.eq_ignore_ascii_case("capslock")
        || key.eq_ignore_ascii_case("numlock")
        || key.eq_ignore_ascii_case("scrolllock")
}

#[inline]
fn command_bar_key_intent(key: &str, modifiers: &gpui::Modifiers) -> Option<CommandBarKeyIntent> {
    if is_key_up(key) {
        return Some(CommandBarKeyIntent::MoveUp);
    }
    if is_key_down(key) {
        return Some(CommandBarKeyIntent::MoveDown);
    }
    if key.eq_ignore_ascii_case("home") {
        return Some(CommandBarKeyIntent::MoveHome);
    }
    if key.eq_ignore_ascii_case("end") {
        return Some(CommandBarKeyIntent::MoveEnd);
    }
    if key.eq_ignore_ascii_case("pageup") {
        return Some(CommandBarKeyIntent::MovePageUp);
    }
    if key.eq_ignore_ascii_case("pagedown") {
        return Some(CommandBarKeyIntent::MovePageDown);
    }
    if is_key_enter(key) {
        return Some(CommandBarKeyIntent::ExecuteSelected);
    }
    if is_key_escape(key) {
        return Some(CommandBarKeyIntent::Close);
    }
    if (key.eq_ignore_ascii_case("left") || key.eq_ignore_ascii_case("arrowleft"))
        && !modifiers.platform
        && !modifiers.control
        && !modifiers.alt
    {
        return Some(CommandBarKeyIntent::MoveCursorLeft {
            select: modifiers.shift,
        });
    }
    if (key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright"))
        && !modifiers.platform
        && !modifiers.control
        && !modifiers.alt
    {
        return Some(CommandBarKeyIntent::MoveCursorRight {
            select: modifiers.shift,
        });
    }
    if is_key_backspace(key) || key.eq_ignore_ascii_case("delete") {
        // Option+Backspace deletes a word like the main search input;
        // Cmd+Backspace falls through so destructive action shortcuts
        // (e.g. Delete Note ⌘⌫) can be handled by the host.
        if modifiers.alt && !modifiers.platform && !modifiers.control {
            return Some(CommandBarKeyIntent::BackspaceWord);
        }
        if !modifiers.platform && !modifiers.control {
            return Some(CommandBarKeyIntent::Backspace);
        }
        return None;
    }
    if key.eq_ignore_ascii_case("space") {
        return Some(CommandBarKeyIntent::TypeChar(' '));
    }
    if is_non_text_named_key(key) {
        return None;
    }

    if !modifiers.platform && !modifiers.control && !modifiers.alt {
        if let Some(ch) = key.chars().next() {
            if ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_' {
                return Some(CommandBarKeyIntent::TypeChar(ch));
            }
        }
    }

    None
}

// --- Chrome contract audit ------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub(crate) struct CommandBarChromeAudit {
    pub surface: &'static str,
    pub search_position: &'static str,
    pub section_mode: &'static str,
    pub anchor: &'static str,
    pub show_icons: bool,
    pub show_footer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct CommandBarChromeViolation {
    pub surface: &'static str,
    pub field: &'static str,
    pub expected: &'static str,
    pub actual: &'static str,
}

impl std::fmt::Display for CommandBarChromeViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "surface={} field={} expected={} actual={}",
            self.surface, self.field, self.expected, self.actual
        )
    }
}

#[inline]
fn command_bar_search_position_name(position: &SearchPosition) -> &'static str {
    match position {
        SearchPosition::Top => "top",
        SearchPosition::Hidden => "hidden",
    }
}

#[inline]
fn command_bar_section_style_name(section_style: &SectionStyle) -> &'static str {
    match section_style {
        SectionStyle::Headers => "headers",
        SectionStyle::Separators => "separators",
        SectionStyle::None => "none",
    }
}

#[inline]
fn command_bar_anchor_name(anchor: &AnchorPosition) -> &'static str {
    match anchor {
        AnchorPosition::Top => "top",
        AnchorPosition::Bottom => "bottom",
    }
}

#[inline]
fn push_command_bar_violation_if_mismatch(
    violations: &mut Vec<CommandBarChromeViolation>,
    surface: &'static str,
    field: &'static str,
    expected: &'static str,
    actual: &'static str,
) {
    if expected != actual {
        violations.push(CommandBarChromeViolation {
            surface,
            field,
            expected,
            actual,
        });
    }
}

impl CommandBarChromeAudit {
    pub(crate) fn from_config(surface: &'static str, config: &CommandBarConfig) -> Self {
        Self {
            surface,
            search_position: command_bar_search_position_name(
                &config.dialog_config.search_position,
            ),
            section_mode: command_bar_section_style_name(&config.dialog_config.section_style),
            anchor: command_bar_anchor_name(&config.dialog_config.anchor),
            show_icons: config.dialog_config.show_icons,
            show_footer: config.dialog_config.show_footer,
        }
    }

    pub(crate) fn validate(&self) -> Vec<CommandBarChromeViolation> {
        let mut violations = Vec::new();
        match self.surface {
            "main_menu" => {
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "search_position",
                    "top",
                    self.search_position,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "section_mode",
                    super::constants::ACTIONS_DIALOG_EXPECT_SECTION_MODE,
                    self.section_mode,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "anchor",
                    "bottom",
                    self.anchor,
                );
            }
            "no_search" => {
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "search_position",
                    "hidden",
                    self.search_position,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "section_mode",
                    super::constants::ACTIONS_DIALOG_EXPECT_SECTION_MODE,
                    self.section_mode,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "anchor",
                    "bottom",
                    self.anchor,
                );
            }
            "notes" => {
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "search_position",
                    "top",
                    self.search_position,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "section_mode",
                    super::constants::ACTIONS_DIALOG_EXPECT_SECTION_MODE,
                    self.section_mode,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "anchor",
                    "top",
                    self.anchor,
                );
            }
            "ai" => {
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "search_position",
                    "top",
                    self.search_position,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "section_mode",
                    super::constants::ACTIONS_DIALOG_EXPECT_SECTION_MODE,
                    self.section_mode,
                );
                push_command_bar_violation_if_mismatch(
                    &mut violations,
                    self.surface,
                    "anchor",
                    "top",
                    self.anchor,
                );
            }
            _ => {}
        }
        violations
    }
}

fn seen_command_bar_chrome_audits(
) -> &'static std::sync::Mutex<std::collections::HashSet<CommandBarChromeAudit>> {
    static SEEN: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashSet<CommandBarChromeAudit>>,
    > = std::sync::OnceLock::new();
    SEEN.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn mark_command_bar_chrome_audit_seen(audit: &CommandBarChromeAudit) -> bool {
    let mut seen = seen_command_bar_chrome_audits()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    seen.insert(audit.clone())
}

fn emit_command_bar_chrome_audit(surface: &'static str, config: &CommandBarConfig) {
    let audit = CommandBarChromeAudit::from_config(surface, config);
    if !mark_command_bar_chrome_audit_seen(&audit) {
        return;
    }
    tracing::info!(
        target: "script_kit::actions_chrome",
        event = "command_bar_chrome_audit",
        surface = audit.surface,
        search_position = audit.search_position,
        section_mode = audit.section_mode,
        anchor = audit.anchor,
        show_icons = audit.show_icons,
        show_footer = audit.show_footer,
        "command bar chrome audit"
    );
    for violation in audit.validate() {
        tracing::warn!(
            target: "script_kit::actions_chrome",
            event = "command_bar_chrome_contract_violation",
            surface = violation.surface,
            field = violation.field,
            expected = violation.expected,
            actual = violation.actual,
            message = %violation,
            "command bar chrome contract violation"
        );
    }
}

/// Configuration presets for common CommandBar use cases
#[derive(Debug, Clone)]
#[allow(dead_code)] // Public API - fields used by consumers
pub struct CommandBarConfig {
    /// ActionsDialog configuration
    pub dialog_config: ActionsDialogConfig,
    /// Whether to close when an action is selected (default: true)
    pub close_on_select: bool,
    /// Whether to close when clicking outside (default: true)
    pub close_on_click_outside: bool,
    /// Whether to close on Escape key (default: true)
    pub close_on_escape: bool,
}

impl Default for CommandBarConfig {
    fn default() -> Self {
        Self {
            dialog_config: ActionsDialogConfig::default(),
            close_on_select: true,
            close_on_click_outside: true,
            close_on_escape: true,
        }
    }
}

#[allow(dead_code)] // Public API - methods for future main menu and other integrations
impl CommandBarConfig {
    /// Create config for main menu style (top search, bottom host anchor, headers).
    pub fn main_menu_style() -> Self {
        let config = Self {
            dialog_config: ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Bottom,
                ..ActionsDialogConfig::default()
            },
            ..Default::default()
        };
        emit_command_bar_chrome_audit("main_menu", &config);
        config
    }

    /// Create config for AI chat style (search at top, headers, icons)
    pub fn ai_style() -> Self {
        let config = Self {
            dialog_config: ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Top,
                show_icons: true,
                ..ActionsDialogConfig::default()
            },
            ..Default::default()
        };
        emit_command_bar_chrome_audit("ai", &config);
        config
    }

    /// Create config with search hidden (external search handling)
    pub fn no_search() -> Self {
        let config = Self {
            dialog_config: ActionsDialogConfig {
                search_position: SearchPosition::Hidden,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Bottom,
                ..ActionsDialogConfig::default()
            },
            ..Default::default()
        };
        emit_command_bar_chrome_audit("no_search", &config);
        config
    }

    /// Create config for Notes window style (search at top, headers, icons)
    pub fn notes_style() -> Self {
        let config = Self {
            dialog_config: ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Top,
                show_icons: true,
                ..ActionsDialogConfig::default()
            },
            ..Default::default()
        };
        emit_command_bar_chrome_audit("notes", &config);
        config
    }

    /// Create config for the Notes recent-note switcher (Cmd+P).
    pub fn notes_recent_style() -> Self {
        let config = Self {
            dialog_config: ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Top,
                show_icons: true,
                search_placeholder: Some("Search Notes".to_string()),
                max_height: NOTES_RECENT_POPUP_MAX_HEIGHT,
                // Switcher rows show note preview/metadata as a subtitle line
                // with main-list row anatomy (see Action::description from
                // get_note_switcher_actions).
                show_subtitles: true,
                ..ActionsDialogConfig::default()
            },
            ..Default::default()
        };
        emit_command_bar_chrome_audit("notes_recent", &config);
        config
    }
}

/// Callback type for action execution
#[allow(dead_code)] // Public API type
pub type CommandBarActionCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// CommandBar - A reusable command palette component
///
/// Provides a high-level API for creating Raycast-style command menus.
/// Wraps ActionsDialog with window management and focus handling.
#[allow(dead_code)] // Public API - many methods for future integrations
pub struct CommandBar {
    /// The underlying dialog entity
    dialog: Option<Entity<ActionsDialog>>,
    /// Actions for the command bar
    actions: Vec<Action>,
    /// Configuration
    pub config: CommandBarConfig,
    /// Theme for styling
    theme: Arc<theme::Theme>,
    /// Whether the command bar is currently visible
    is_open: bool,
    /// Callback when an action is selected
    on_action: Option<CommandBarActionCallback>,
}

#[cfg(test)]
mod command_bar_key_intent_tests {
    use super::*;

    #[test]
    fn test_command_bar_key_intent_routes_cursor_keys_without_typing_their_names() {
        let no_mods = gpui::Modifiers::default();

        assert_eq!(
            command_bar_key_intent("space", &no_mods),
            Some(CommandBarKeyIntent::TypeChar(' '))
        );
        assert_eq!(command_bar_key_intent("tab", &no_mods), None);
        assert_eq!(
            command_bar_key_intent("arrowleft", &no_mods),
            Some(CommandBarKeyIntent::MoveCursorLeft { select: false })
        );
        assert_eq!(
            command_bar_key_intent("backspace", &no_mods),
            Some(CommandBarKeyIntent::Backspace)
        );
    }

    #[test]
    fn test_command_bar_key_intent_maps_required_key_variants_to_same_intents() {
        let no_mods = gpui::Modifiers::default();

        assert_eq!(
            command_bar_key_intent("up", &no_mods),
            Some(CommandBarKeyIntent::MoveUp)
        );
        assert_eq!(
            command_bar_key_intent("arrowup", &no_mods),
            Some(CommandBarKeyIntent::MoveUp)
        );

        assert_eq!(
            command_bar_key_intent("down", &no_mods),
            Some(CommandBarKeyIntent::MoveDown)
        );
        assert_eq!(
            command_bar_key_intent("arrowdown", &no_mods),
            Some(CommandBarKeyIntent::MoveDown)
        );

        assert_eq!(
            command_bar_key_intent("enter", &no_mods),
            Some(CommandBarKeyIntent::ExecuteSelected)
        );
        assert_eq!(
            command_bar_key_intent("Enter", &no_mods),
            Some(CommandBarKeyIntent::ExecuteSelected)
        );

        assert_eq!(
            command_bar_key_intent("escape", &no_mods),
            Some(CommandBarKeyIntent::Close)
        );
        assert_eq!(
            command_bar_key_intent("Escape", &no_mods),
            Some(CommandBarKeyIntent::Close)
        );
    }
}

// --- merged from part_02.rs ---
#[allow(dead_code)] // Public API - many methods for future integrations
impl CommandBar {
    fn devtools_text_fingerprint(value: &str) -> String {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("fnv1a64:{hash:016x}")
    }

    pub(crate) fn automation_state(&self, surface: &str, cx: &App) -> serde_json::Value {
        let mut sections = std::collections::BTreeMap::<String, usize>::new();
        for action in &self.actions {
            let section = action
                .section
                .clone()
                .unwrap_or_else(|| "Unsectioned".to_string());
            *sections.entry(section).or_insert(0) += 1;
        }
        let configured_section_count = sections.len();
        let configured_shortcut_count = super::dialog::action_shortcut_parity_report(
            &self.actions,
            &(0..self.actions.len()).collect::<Vec<_>>(),
        )
        .displayed_shortcut_count;

        let base = serde_json::json!({
            "schemaVersion": 1,
            "surface": surface,
            "redacted": true,
            "open": self.is_open,
            "configuredActionCount": self.actions.len(),
            "configuredSectionCount": configured_section_count,
            "configuredSections": sections,
            "configuredShortcutCount": configured_shortcut_count,
            "closePolicy": {
                "closeOnSelect": self.config.close_on_select,
                "closeOnClickOutside": self.config.close_on_click_outside,
                "closeOnEscape": self.config.close_on_escape,
            },
            "config": {
                "searchPosition": command_bar_search_position_name(&self.config.dialog_config.search_position),
                "sectionMode": command_bar_section_style_name(&self.config.dialog_config.section_style),
                "anchor": command_bar_anchor_name(&self.config.dialog_config.anchor),
                "showIcons": self.config.dialog_config.show_icons,
                "showFooter": self.config.dialog_config.show_footer,
                "maxHeight": self.config.dialog_config.max_height,
            },
        });

        if let Some(dialog) = &self.dialog {
            let dialog_state = dialog.read(cx).automation_state(surface, cx);
            serde_json::json!({
                "schemaVersion": 1,
                "surface": surface,
                "redacted": true,
                "open": self.is_open,
                "configuredActionCount": self.actions.len(),
                "configuredSectionCount": configured_section_count,
                "configuredShortcutCount": configured_shortcut_count,
                "selectedActionId": self.get_selected_action_id(cx),
                "selectedActionTitleFingerprint": self.get_selected_action(cx).map(|action| Self::devtools_text_fingerprint(&action.title)),
                "dialog": dialog_state,
                "base": base,
            })
        } else {
            base
        }
    }

    #[inline]
    fn reset_open_state(&mut self) {
        self.is_open = false;
        self.dialog = None;
    }

    /// Reconcile the in-memory `is_open` flag against the actual detached
    /// actions window. If the window was dismissed externally (focus loss,
    /// click outside, system close) without routing through `close()`, the
    /// boolean would otherwise stick true and swallow every keystroke at the
    /// keyboard-routing layer. Returns true when stale state was cleared.
    #[inline]
    fn reset_open_state_if_window_gone(&mut self) -> bool {
        if self.is_open && !is_actions_window_open() {
            self.reset_open_state();
            logging::log(
                "COMMAND_BAR",
                "Reset stale open state because the actions window is no longer open",
            );
            return true;
        }
        false
    }

    /// Public reconciliation entry used by host windows (e.g. Notes keyboard
    /// handler) before they consume keystrokes based on `is_open()`.
    pub fn reconcile_open_state(&mut self) -> bool {
        self.reset_open_state_if_window_gone()
    }

    /// Mark the command bar closed without re-entering the detached window's
    /// close path. Hosts call this from the dialog's `on_close` callback —
    /// the popup window is already tearing itself down (Escape/Cmd+K while
    /// the popup was the key window, focus loss), so only the in-memory
    /// state needs to drop. Returns true when the bar was open.
    pub fn mark_closed_externally(&mut self) -> bool {
        if !self.is_open {
            return false;
        }
        self.reset_open_state();
        logging::log(
            "COMMAND_BAR",
            "Command bar marked closed (detached popup closed itself)",
        );
        true
    }

    /// Create a new CommandBar with actions and configuration
    pub fn new(actions: Vec<Action>, config: CommandBarConfig, theme: Arc<theme::Theme>) -> Self {
        Self {
            dialog: None,
            actions,
            config,
            theme,
            is_open: false,
            on_action: None,
        }
    }

    /// Set the action callback
    pub fn with_on_action(mut self, callback: CommandBarActionCallback) -> Self {
        self.on_action = Some(callback);
        self
    }

    /// Set the action callback (mutable version)
    pub fn set_on_action(&mut self, callback: CommandBarActionCallback) {
        self.on_action = Some(callback);
    }

    /// Update the actions list
    pub fn set_actions(&mut self, actions: Vec<Action>, cx: &mut App) {
        self.actions = actions.clone();

        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                let previously_selected_action_id = d.get_selected_action_id();
                d.actions = actions;
                super::dialog::append_disabled_action_test_fixture(&mut d.actions);
                d.reset_filter_to_all();
                d.search_text.clear();
                d.grouped_items = rebuild_grouped_items_for_command_bar(
                    &d.actions,
                    &d.filtered_actions,
                    d.config.section_style,
                );

                let old_count = d.list_state.item_count();
                d.list_state.splice(0..old_count, d.grouped_items.len());
                d.apply_refresh_selection(previously_selected_action_id.as_deref());
                d.scroll_to_selected();
                cx.notify();
            });

            if self.is_open {
                crate::actions::set_actions_dialog_search_text(dialog, String::new(), cx);
            }
        }
    }

    /// Update the theme
    pub fn set_theme(&mut self, theme: Arc<theme::Theme>, cx: &mut App) {
        self.theme = theme.clone();

        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                d.update_theme(theme);
                cx.notify();
            });

            if self.is_open {
                notify_actions_window(cx);
            }
        }
    }

    /// Toggle open/close state (for Cmd+K binding)
    pub fn toggle<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        self.reset_open_state_if_window_gone();
        if self.is_open {
            self.close(cx);
        } else {
            self.open(window, cx);
        }
    }

    /// Open the command bar at the default position (bottom-right)
    pub fn open<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        self.open_at_position(window, cx, super::window::WindowPosition::BottomRight);
    }

    /// Open the command bar at top-center (Raycast-style, for Notes window)
    pub fn open_centered<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        self.open_at_position(window, cx, super::window::WindowPosition::TopCenter);
    }

    /// Open the command bar at a specific position
    pub fn open_at_position<V: 'static>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<V>,
        position: super::window::WindowPosition,
    ) {
        self.reset_open_state_if_window_gone();
        if self.is_open {
            return;
        }

        // Create callback for dialog
        let on_select: Arc<dyn Fn(String) + Send + Sync> = Arc::new(|_| {
            // Action handling is done via execute_selected_action()
        });

        // Create the dialog entity
        let theme = self.theme.clone();
        let actions = self.actions.clone();
        let config = self.config.dialog_config.clone();

        // Log what actions we're creating the dialog with
        logging::log(
            "COMMAND_BAR",
            &format!(
                "Creating dialog with {} actions: [{}]",
                actions.len(),
                actions.iter().take(5).map(|a| a.title.as_str()).join(", ")
            ),
        );

        let dialog = cx.new(|cx| {
            let mut d =
                ActionsDialog::with_config(cx.focus_handle(), on_select, actions, theme, config);
            // Tell dialog to skip track_focus - ActionsWindow handles focus instead
            // This ensures keyboard events go to ActionsWindow's on_key_down handler
            d.set_skip_track_focus(true);
            d
        });

        // Get window bounds and display for positioning
        let bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());

        // Store dialog and mark as open
        self.dialog = Some(dialog.clone());
        self.is_open = true;

        // Open the vibrancy window at the specified position
        let parent_automation_id = if crate::notes::is_notes_window(window) {
            Some("notes".to_string())
        } else {
            crate::windows::focused_automation_window_id()
        };
        let open_feedback = CommandBarOpenFeedbackAction::OpenWindow;
        match open_actions_window(
            cx,
            window.window_handle(),
            bounds,
            display_id,
            dialog,
            position,
            parent_automation_id.as_deref(),
        ) {
            Ok(_) => {
                logging::log("COMMAND_BAR", &open_feedback.success_log(position));
            }
            Err(e) => {
                logging::log("COMMAND_BAR", &open_feedback.failure_log(e));
                self.reset_open_state();
            }
        }

        cx.notify();
    }

    /// Close the command bar
    pub fn close<V: 'static>(&mut self, cx: &mut Context<V>) {
        if !self.is_open {
            return;
        }

        close_actions_window(cx);
        self.reset_open_state();
        logging::log("COMMAND_BAR", "Command bar closed");
        cx.notify();
    }

    /// Close the command bar (App context version)
    pub fn close_app(&mut self, cx: &mut App) {
        if !self.is_open {
            return;
        }

        close_actions_window(cx);
        self.reset_open_state();
        logging::log("COMMAND_BAR", "Command bar closed");
    }

    /// Check if the command bar is open.
    ///
    /// Returns true only when both the local flag is set AND the detached
    /// actions window is still open. This guards against external dismissals
    /// (focus loss, click outside) that would otherwise leave `is_open` stuck
    /// true and make the keyboard router consume every keystroke.
    pub fn is_open(&self) -> bool {
        self.is_open && is_actions_window_open()
    }

    /// Get the currently selected action ID
    pub fn get_selected_action_id(&self, cx: &App) -> Option<String> {
        self.dialog
            .as_ref()
            .and_then(|d| d.read(cx).get_selected_action_id())
    }

    /// Get the currently selected action
    pub fn get_selected_action<'a>(&'a self, cx: &'a App) -> Option<&'a Action> {
        self.dialog
            .as_ref()
            .and_then(|d| d.read(cx).get_selected_action())
    }

    /// Execute the selected action and optionally close the command bar
    ///
    /// Returns the action ID if an action was executed, None otherwise.
    pub fn execute_selected_action<V: 'static>(&mut self, cx: &mut Context<V>) -> Option<String> {
        let action_id = match self.get_selected_action_id(cx) {
            Some(action_id) => action_id,
            None => {
                logging::log(
                    "COMMAND_BAR",
                    "No selected action available during execute_selected_action",
                );
                return None;
            }
        };

        logging::log(
            "COMMAND_BAR",
            &format!("Executing selected action '{}'", action_id),
        );

        // Call the callback if set
        if let Some(callback) = &self.on_action {
            callback(&action_id);
        }

        // Close if configured to do so
        if self.config.close_on_select {
            self.close(cx);
        }

        Some(action_id)
    }

    pub fn move_search_cursor<V: 'static>(
        &mut self,
        right: bool,
        select: bool,
        window: &mut Window,
        cx: &mut Context<V>,
    ) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                if right {
                    d.move_search_cursor_right(select, window, cx);
                } else {
                    d.move_search_cursor_left(select, window, cx);
                }
            });
            notify_actions_window(cx);
        }
    }

    /// Forward character insertion to the entity-backed Actions search input.
    pub fn handle_char<V: 'static>(&mut self, ch: char, window: &mut Window, cx: &mut Context<V>) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                d.insert_search_text(ch.to_string(), window, cx);
            });
            notify_actions_window(cx);
        }
    }

    /// Forward Backspace to the entity-backed Actions search input.
    pub fn handle_backspace<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                d.backspace_search_input(window, cx);
            });
            notify_actions_window(cx);
        }
    }

    /// Forward Option+Backspace to the entity-backed Actions search input.
    pub fn handle_backspace_word<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                d.delete_previous_search_word(window, cx);
            });
            notify_actions_window(cx);
        }
    }

    /// Forward Cmd+V to the entity-backed Actions search input.
    pub fn handle_paste<V: 'static>(&mut self, window: &mut Window, cx: &mut Context<V>) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                d.paste_search_input(window, cx);
            });
            notify_actions_window(cx);
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| d.move_up(cx));
            notify_actions_window(cx);
        }
    }

    /// Move selection down
    pub fn select_next(&mut self, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| d.move_down(cx));
            notify_actions_window(cx);
        }
    }

    /// Jump to first action in the list.
    pub fn select_first(&mut self, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                if let Some(first) = first_selectable_index(&d.grouped_items) {
                    d.selected_index = Some(first);
                    d.scroll_to_selected();
                    cx.notify();
                }
            });
            notify_actions_window(cx);
        }
    }

    /// Jump to last action in the list.
    pub fn select_last(&mut self, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                if let Some(last) = last_selectable_index(&d.grouped_items) {
                    d.selected_index = Some(last);
                    d.scroll_to_selected();
                    cx.notify();
                }
            });
            notify_actions_window(cx);
        }
    }

    /// Move one page up in the list.
    pub fn select_page_up(&mut self, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                if d.grouped_items.is_empty() {
                    return;
                }

                let current = d
                    .selected_index
                    .or_else(|| first_selectable_index(&d.grouped_items))
                    .unwrap_or(0);
                let target = current.saturating_sub(COMMAND_BAR_PAGE_JUMP);
                if let Some(next_index) = selectable_index_at_or_before(&d.grouped_items, target)
                    .or_else(|| first_selectable_index(&d.grouped_items))
                {
                    d.selected_index = Some(next_index);
                    d.scroll_to_selected();
                    cx.notify();
                }
            });
            notify_actions_window(cx);
        }
    }

    /// Move one page down in the list.
    pub fn select_page_down(&mut self, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, cx| {
                if d.grouped_items.is_empty() {
                    return;
                }

                let last_index = d.grouped_items.len() - 1;
                let current = d
                    .selected_index
                    .or_else(|| first_selectable_index(&d.grouped_items))
                    .unwrap_or(0);
                let target = (current + COMMAND_BAR_PAGE_JUMP).min(last_index);
                if let Some(next_index) = selectable_index_at_or_after(&d.grouped_items, target)
                    .or_else(|| last_selectable_index(&d.grouped_items))
                {
                    d.selected_index = Some(next_index);
                    d.scroll_to_selected();
                    cx.notify();
                }
            });
            notify_actions_window(cx);
        }
    }

    /// Set cursor visibility (for blink animation)
    pub fn set_cursor_visible(&mut self, visible: bool, cx: &mut App) {
        if let Some(dialog) = &self.dialog {
            dialog.update(cx, |d, _cx| {
                d.set_cursor_visible(visible);
            });
            notify_actions_window(cx);
        }
    }

    /// Get the dialog entity (for advanced use cases)
    pub fn dialog(&self) -> Option<&Entity<ActionsDialog>> {
        self.dialog.as_ref()
    }

    /// Get access to the focus handle of the underlying dialog
    pub fn focus_handle(&self, cx: &App) -> Option<FocusHandle> {
        self.dialog
            .as_ref()
            .map(|d| d.read(cx).focus_handle.clone())
    }
}

fn rebuild_grouped_items_for_command_bar(
    actions: &[Action],
    filtered_actions: &[usize],
    section_style: SectionStyle,
) -> Vec<GroupedActionItem> {
    let mut grouped = Vec::new();
    if filtered_actions.is_empty() {
        return grouped;
    }

    let mut previous_section: Option<String> = None;
    for (filter_idx, &action_idx) in filtered_actions.iter().enumerate() {
        if let Some(action) = actions.get(action_idx) {
            if section_style == SectionStyle::Headers {
                if let Some(section) = &action.section {
                    if previous_section.as_ref() != Some(section) {
                        grouped.push(GroupedActionItem::SectionHeader(section.clone()));
                        previous_section = Some(section.clone());
                    }
                }
            }

            grouped.push(GroupedActionItem::Item(filter_idx));
        }
    }

    grouped
}

#[cfg(test)]
mod command_bar_set_actions_tests {
    use super::*;
    use crate::actions::types::ActionCategory;

    fn test_action(id: &str, section: Option<&str>) -> Action {
        let mut action = Action::new(id, id, None, ActionCategory::ScriptContext);
        action.has_action = true;
        if let Some(section) = section {
            action = action.with_section(section);
        }
        action
    }

    #[test]
    fn test_rebuild_grouped_items_for_command_bar_adds_headers_for_new_sections() {
        let actions = vec![
            test_action("a", Some("Alpha")),
            test_action("b", Some("Alpha")),
            test_action("c", Some("Beta")),
        ];
        let filtered = vec![0, 1, 2];

        let grouped =
            rebuild_grouped_items_for_command_bar(&actions, &filtered, SectionStyle::Headers);

        assert_eq!(grouped.len(), 5);
        assert!(matches!(
            grouped.first(),
            Some(GroupedActionItem::SectionHeader(section)) if section == "Alpha"
        ));
        assert!(matches!(grouped.get(1), Some(GroupedActionItem::Item(0))));
        assert!(matches!(grouped.get(2), Some(GroupedActionItem::Item(1))));
        assert!(matches!(
            grouped.get(3),
            Some(GroupedActionItem::SectionHeader(section)) if section == "Beta"
        ));
        assert!(matches!(grouped.get(4), Some(GroupedActionItem::Item(2))));
    }

    #[test]
    fn test_rebuild_grouped_items_for_command_bar_keeps_compact_rows_for_separators() {
        let actions = vec![
            test_action("a", Some("Alpha")),
            test_action("b", Some("Beta")),
        ];
        let filtered = vec![0, 1];

        let grouped =
            rebuild_grouped_items_for_command_bar(&actions, &filtered, SectionStyle::Separators);

        assert_eq!(grouped.len(), 2);
        assert!(matches!(grouped.first(), Some(GroupedActionItem::Item(0))));
        assert!(matches!(grouped.get(1), Some(GroupedActionItem::Item(1))));
    }
}

// --- merged from part_03.rs ---
/// Trait for views that can host a command bar
///
/// Implement this trait to enable Cmd+K command bar functionality in your view.
#[allow(dead_code)] // Public API - trait for future integrations
pub trait CommandBarHost {
    /// Get a reference to the command bar
    fn command_bar(&self) -> &CommandBar;

    /// Get a mutable reference to the command bar
    fn command_bar_mut(&mut self) -> &mut CommandBar;

    /// Get actions for the current context
    ///
    /// Override this to provide context-aware actions.
    fn get_context_actions(&self) -> Vec<Action> {
        vec![]
    }

    /// Handle action execution
    ///
    /// Called when an action is selected from the command bar.
    /// Override this to implement action handling.
    fn execute_action(&mut self, action_id: &str, window: &mut Window, cx: &mut Context<Self>)
    where
        Self: Sized;

    /// Toggle the command bar (Cmd+K)
    fn toggle_command_bar(&mut self, window: &mut Window, cx: &mut Context<Self>)
    where
        Self: Sized + 'static,
    {
        self.command_bar_mut().toggle(window, cx);
    }

    /// Handle keyboard input when command bar is open
    ///
    /// Returns true if the key was handled, false otherwise.
    fn handle_command_bar_key(
        &mut self,
        key: &str,
        modifiers: &gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool
    where
        Self: Sized + 'static,
    {
        if !self.command_bar().is_open() {
            return false;
        }

        match command_bar_key_intent(key, modifiers) {
            Some(CommandBarKeyIntent::MoveUp) => {
                self.command_bar_mut().select_prev(cx);
                true
            }
            Some(CommandBarKeyIntent::MoveDown) => {
                self.command_bar_mut().select_next(cx);
                true
            }
            Some(CommandBarKeyIntent::MoveHome) => {
                self.command_bar_mut().select_first(cx);
                true
            }
            Some(CommandBarKeyIntent::MoveEnd) => {
                self.command_bar_mut().select_last(cx);
                true
            }
            Some(CommandBarKeyIntent::MovePageUp) => {
                self.command_bar_mut().select_page_up(cx);
                true
            }
            Some(CommandBarKeyIntent::MovePageDown) => {
                self.command_bar_mut().select_page_down(cx);
                true
            }
            Some(CommandBarKeyIntent::ExecuteSelected) => {
                if let Some(action_id) = self.command_bar_mut().execute_selected_action(cx) {
                    self.execute_action(&action_id, window, cx);
                }
                true
            }
            Some(CommandBarKeyIntent::Close) => {
                if self.command_bar().config.close_on_escape {
                    self.command_bar_mut().close(cx);
                }
                true
            }
            Some(CommandBarKeyIntent::Backspace) => {
                self.command_bar_mut().handle_backspace(window, cx);
                true
            }
            Some(CommandBarKeyIntent::BackspaceWord) => {
                self.command_bar_mut().handle_backspace_word(window, cx);
                true
            }
            Some(CommandBarKeyIntent::MoveCursorLeft { select }) => {
                self.command_bar_mut()
                    .move_search_cursor(false, select, window, cx);
                true
            }
            Some(CommandBarKeyIntent::MoveCursorRight { select }) => {
                self.command_bar_mut()
                    .move_search_cursor(true, select, window, cx);
                true
            }
            Some(CommandBarKeyIntent::TypeChar(ch)) => {
                self.command_bar_mut().handle_char(ch, window, cx);
                true
            }
            None => false,
        }
    }
}

/// Check if any command bar window is currently open (global check)
#[allow(dead_code)] // Public API - global check function for future integrations
pub fn is_command_bar_open() -> bool {
    is_actions_window_open()
}

#[cfg(test)]
mod command_bar_config_tests {
    use super::*;

    #[test]
    fn test_command_bar_config_defaults() {
        let config = CommandBarConfig::default();
        assert!(config.close_on_select);
        assert!(config.close_on_escape);
        assert!(config.close_on_click_outside);
    }

    #[test]
    fn test_command_bar_config_ai_style() {
        let config = CommandBarConfig::ai_style();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Top
        ));
        assert!(matches!(
            config.dialog_config.section_style,
            SectionStyle::Headers
        ));
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn test_command_bar_config_main_menu_style() {
        let config = CommandBarConfig::main_menu_style();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Top
        ));
        assert!(matches!(
            config.dialog_config.section_style,
            SectionStyle::Headers
        ));
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn test_command_bar_config_no_search() {
        let config = CommandBarConfig::no_search();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Hidden
        ));
    }

    #[test]
    fn command_bar_presets_keep_action_dialog_footer_disabled() {
        assert!(
            !CommandBarConfig::main_menu_style()
                .dialog_config
                .show_footer,
            "main_menu_style must keep action dialogs footerless"
        );
        assert!(
            !CommandBarConfig::no_search().dialog_config.show_footer,
            "no_search must keep action dialogs footerless"
        );
        assert!(
            !CommandBarConfig::ai_style().dialog_config.show_footer,
            "ai_style must keep action dialogs footerless"
        );
        assert!(
            !CommandBarConfig::notes_style().dialog_config.show_footer,
            "notes_style must keep action dialogs footerless"
        );
    }

    #[test]
    fn main_menu_style_emits_headers_contract() {
        let audit =
            CommandBarChromeAudit::from_config("main_menu", &CommandBarConfig::main_menu_style());
        assert_eq!(audit.search_position, "top");
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.anchor, "bottom");
        assert!(audit.validate().is_empty());
    }

    #[test]
    fn no_search_style_emits_hidden_headers_contract() {
        let audit = CommandBarChromeAudit::from_config("no_search", &CommandBarConfig::no_search());
        assert_eq!(audit.search_position, "hidden");
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.anchor, "bottom");
        assert!(audit.validate().is_empty());
    }

    #[test]
    fn notes_style_emits_top_headers_contract() {
        let audit = CommandBarChromeAudit::from_config("notes", &CommandBarConfig::notes_style());
        assert_eq!(audit.search_position, "top");
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.anchor, "top");
        assert!(audit.validate().is_empty());
    }

    #[test]
    fn notes_recent_style_uses_taller_top_headers_contract() {
        let config = CommandBarConfig::notes_recent_style();
        let audit = CommandBarChromeAudit::from_config("notes_recent", &config);
        assert_eq!(audit.search_position, "top");
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.anchor, "top");
        assert!(audit.validate().is_empty());
        assert_eq!(
            config.dialog_config.max_height,
            crate::actions::constants::NOTES_RECENT_POPUP_MAX_HEIGHT
        );
        assert!(
            config.dialog_config.max_height > crate::actions::constants::POPUP_MAX_HEIGHT,
            "Notes recent switcher needs a taller cap than generic action popups"
        );
    }

    #[test]
    fn ai_style_emits_top_headers_contract() {
        let audit = CommandBarChromeAudit::from_config("ai", &CommandBarConfig::ai_style());
        assert_eq!(audit.search_position, "top");
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.anchor, "top");
        assert!(audit.validate().is_empty());
    }

    #[test]
    fn test_command_bar_key_intent_supports_aliases_and_jump_keys() {
        let no_mods = gpui::Modifiers::default();

        assert_eq!(
            command_bar_key_intent("return", &no_mods),
            Some(CommandBarKeyIntent::ExecuteSelected)
        );
        assert_eq!(
            command_bar_key_intent("esc", &no_mods),
            Some(CommandBarKeyIntent::Close)
        );
        assert_eq!(
            command_bar_key_intent("home", &no_mods),
            Some(CommandBarKeyIntent::MoveHome)
        );
        assert_eq!(
            command_bar_key_intent("end", &no_mods),
            Some(CommandBarKeyIntent::MoveEnd)
        );
        assert_eq!(
            command_bar_key_intent("pageup", &no_mods),
            Some(CommandBarKeyIntent::MovePageUp)
        );
        assert_eq!(
            command_bar_key_intent("pagedown", &no_mods),
            Some(CommandBarKeyIntent::MovePageDown)
        );
    }

    #[test]
    fn test_selectable_index_helpers_skip_section_headers() {
        let rows = vec![
            GroupedActionItem::SectionHeader("System".to_string()),
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("Script".to_string()),
            GroupedActionItem::Item(1),
        ];

        assert_eq!(first_selectable_index(&rows), Some(1));
        assert_eq!(last_selectable_index(&rows), Some(3));
        assert_eq!(selectable_index_at_or_before(&rows, 2), Some(1));
        assert_eq!(selectable_index_at_or_after(&rows, 2), Some(3));
    }
}

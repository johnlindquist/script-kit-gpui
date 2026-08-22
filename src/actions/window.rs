// --- merged from part_01.rs ---
// Actions Window - Separate vibrancy window for actions panel
//
// This creates a floating popup window with its own vibrancy blur effect,
// similar to Raycast's actions panel. The window is:
// - Non-draggable (fixed position relative to main window)
// - Positioned below the header, at the right edge of main window
// - Auto-closes when app loses focus
// - Shares the ActionsDialog entity with the main app for keyboard routing

use crate::platform;
use crate::protocol::AutomationWindowKind;
use crate::theme::get_cached_theme;
use crate::ui_foundation::{is_key_backspace, is_key_down, is_key_enter, is_key_escape, is_key_up};
use crate::window_resize::layout::FOOTER_HEIGHT;
use gpui::{
    div, prelude::*, px, AnyWindowHandle, App, Bounds, Context, DisplayId, Entity, FocusHandle,
    Focusable, Pixels, Point, Render, Size, Subscription, Window, WindowBounds, WindowHandle,
    WindowKind, WindowOptions,
};
// Root intentionally NOT used — its opaque bg blocks NSVisualEffectView vibrancy
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
use super::constants::POPUP_WIDTH;
use super::dialog::{
    first_selectable_index, last_selectable_index, selectable_index_at_or_after,
    selectable_index_at_or_before, ActionsDialog, ActionsDialogShellSizingSnapshot,
};
use super::types::Action;

/// Count the number of section headers in the filtered action list
/// A section header appears when an action's section differs from the previous action's section
pub(super) fn count_section_headers(actions: &[Action], filtered_indices: &[usize]) -> usize {
    if filtered_indices.is_empty() {
        return 0;
    }

    let mut count = 0;
    let mut prev_section: Option<&str> = None;

    for &idx in filtered_indices {
        if let Some(action) = actions.get(idx) {
            // Match header insertion behavior from grouped list rendering:
            // only track non-empty sections so unsectioned rows do not break a section run.
            if let Some(current_section) = action.section.as_deref() {
                if prev_section != Some(current_section) {
                    count += 1;
                    prev_section = Some(current_section);
                }
            }
        }
    }

    count
}

/// Structured lifecycle events for the actions popup.
///
/// Every significant state transition emits one of these via
/// [`emit_actions_popup_event`] under the `ACTIONS_POPUP` tracing target,
/// giving agentic callers a machine-readable contract for open/route/close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Variants used from include!()-ed code in app_impl/
pub(crate) enum ActionsPopupEvent {
    /// Toggle or explicit open was requested.
    OpenRequested,
    /// Window was successfully created and stored.
    OpenSucceeded,
    /// Window creation failed (see tracing error for details).
    OpenFailed,
    /// A keyboard event was routed through the popup.
    RoutedKey,
    /// The popup was closed (via Cmd+K, Escape, blur, etc.).
    Closed,
}

/// Emit a structured receipt for an actions popup lifecycle event.
///
/// All fields are optional so callers only supply what is relevant to their
/// transition.  The receipt is emitted at `info` level under
/// `target: "ACTIONS_POPUP"` so log consumers can filter deterministically.
pub(crate) fn emit_actions_popup_event(
    event: ActionsPopupEvent,
    host: Option<&str>,
    position: Option<WindowPosition>,
    num_actions: Option<usize>,
    section_headers: Option<usize>,
    height_px: Option<f32>,
) {
    tracing::info!(
        target: "ACTIONS_POPUP",
        ?event,
        host,
        position = ?position,
        num_actions,
        section_headers,
        height_px,
        "actions popup receipt"
    );
}

/// Global singleton for the actions window handle
static ACTIONS_WINDOW: OnceLock<Mutex<Option<WindowHandle<ActionsWindow>>>> = OnceLock::new();

/// Parent window kind of the currently open actions window (None when closed).
/// The main app's key interceptors consult this so they only route keys for
/// popups the main launcher actually hosts; popups hosted by secondary windows
/// (Notes, detached Agent Chat) own their keys via ActionsWindow::on_key_down.
static ACTIONS_WINDOW_PARENT_KIND: OnceLock<Mutex<Option<AutomationWindowKind>>> = OnceLock::new();
static ACTIONS_POPUP_AUTOMATION_SNAPSHOT: OnceLock<Mutex<Option<serde_json::Value>>> =
    OnceLock::new();
static ACTIONS_POPUP_AUTOMATION_GENERATION: OnceLock<Mutex<u64>> = OnceLock::new();

const ACTIONS_WINDOW_PAGE_JUMP: usize = 8;
#[cfg(target_os = "macos")]
const NS_WINDOW_ABOVE: i64 = 1;

fn actions_window_reserves_shortcut(canonical: &str) -> bool {
    matches!(canonical, "escape" | "cmd+k")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionsWindowKeyIntent {
    MoveUp,
    MoveDown,
    MoveHome,
    MoveEnd,
    MovePageUp,
    MovePageDown,
    ExecuteSelected,
    /// Cmd+Enter: hand the selected action to Agent Chat as a canonical target.
    SendToAgentChat,
    /// Escape: pop a drill-down route first, close only at the top level.
    Close,
    /// Cmd+K: the open/close toggle — always fully closes, never route-pops,
    /// matching the main-hosted popup's `route_key_to_actions_dialog` path so
    /// the same chord cannot mean two different things across hosts.
    Dismiss,
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

#[inline]
fn actions_window_key_intent(
    key: &str,
    key_char: Option<&str>,
    modifiers: &gpui::Modifiers,
) -> Option<ActionsWindowKeyIntent> {
    if is_key_up(key) {
        return Some(ActionsWindowKeyIntent::MoveUp);
    }
    if is_key_down(key) {
        return Some(ActionsWindowKeyIntent::MoveDown);
    }
    if key.eq_ignore_ascii_case("home") {
        return Some(ActionsWindowKeyIntent::MoveHome);
    }
    if key.eq_ignore_ascii_case("end") {
        return Some(ActionsWindowKeyIntent::MoveEnd);
    }
    if key.eq_ignore_ascii_case("pageup") {
        return Some(ActionsWindowKeyIntent::MovePageUp);
    }
    if key.eq_ignore_ascii_case("pagedown") {
        return Some(ActionsWindowKeyIntent::MovePageDown);
    }
    // Cmd+Enter must precede plain Enter to avoid being swallowed.
    if modifiers.platform
        && !modifiers.shift
        && !modifiers.control
        && !modifiers.alt
        && is_key_enter(key)
    {
        return Some(ActionsWindowKeyIntent::SendToAgentChat);
    }
    if is_key_enter(key) {
        return Some(ActionsWindowKeyIntent::ExecuteSelected);
    }
    if is_key_escape(key) {
        return Some(ActionsWindowKeyIntent::Close);
    }
    if modifiers.platform
        && !modifiers.shift
        && !modifiers.control
        && !modifiers.alt
        && key.eq_ignore_ascii_case("k")
    {
        return Some(ActionsWindowKeyIntent::Dismiss);
    }
    if key.eq_ignore_ascii_case("left")
        && !modifiers.platform
        && !modifiers.control
        && !modifiers.alt
    {
        return Some(ActionsWindowKeyIntent::MoveCursorLeft {
            select: modifiers.shift,
        });
    }
    if key.eq_ignore_ascii_case("right")
        && !modifiers.platform
        && !modifiers.control
        && !modifiers.alt
    {
        return Some(ActionsWindowKeyIntent::MoveCursorRight {
            select: modifiers.shift,
        });
    }
    if is_key_backspace(key) || key.eq_ignore_ascii_case("delete") {
        // Option+Backspace deletes a word like the main search input.
        // Cmd+Backspace intentionally falls through (returns None) so
        // destructive action shortcuts (e.g. Delete Note ⌘⌫) can match.
        if modifiers.alt && !modifiers.platform && !modifiers.control {
            return Some(ActionsWindowKeyIntent::BackspaceWord);
        }
        if !modifiers.platform && !modifiers.control {
            return Some(ActionsWindowKeyIntent::Backspace);
        }
        return None;
    }
    if !modifiers.platform && !modifiers.control && !modifiers.alt {
        // Full printable charset via the produced character (matches the main
        // search input), falling back to single-char `key` names for callers
        // without a key_char (tests, synthetic events).
        if let Some(ch) = crate::ui_foundation::printable_char(key_char) {
            return Some(ActionsWindowKeyIntent::TypeChar(ch));
        }
        if let Some(ch) = key.chars().next() {
            if ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_' {
                return Some(ActionsWindowKeyIntent::TypeChar(ch));
            }
        }
    }
    None
}

#[inline]
fn should_auto_close_actions_window(
    parent_window_focused: bool,
    actions_window_active: bool,
) -> bool {
    if std::env::var("SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN")
        .ok()
        .as_deref()
        == Some("1")
    {
        return false;
    }
    !parent_window_focused && !actions_window_active
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionsParentFocusState {
    AutomationRegistryFocused,
    PlatformFocused,
    Unfocused,
}

impl ActionsParentFocusState {
    fn is_focused(self) -> bool {
        matches!(
            self,
            ActionsParentFocusState::AutomationRegistryFocused
                | ActionsParentFocusState::PlatformFocused
        )
    }
}

fn actions_parent_window_focus_state(parent_automation_id: &str) -> ActionsParentFocusState {
    if crate::windows::focused_automation_window_id()
        .as_deref()
        .is_some_and(|focused_id| focused_id == parent_automation_id)
    {
        return ActionsParentFocusState::AutomationRegistryFocused;
    }

    let platform_focused =
        match crate::windows::automation_window_by_id(parent_automation_id).map(|info| info.kind) {
            Some(AutomationWindowKind::Main) => platform::is_main_window_focused(),
            Some(AutomationWindowKind::Notes) => platform::is_notes_window_focused(),
            Some(_) | None => false,
        };

    if platform_focused {
        ActionsParentFocusState::PlatformFocused
    } else {
        ActionsParentFocusState::Unfocused
    }
}

fn actions_parent_window_focused(parent_automation_id: &str) -> bool {
    actions_parent_window_focus_state(parent_automation_id).is_focused()
}

#[inline]
fn clear_window_slot<T>(slot: &mut Option<T>) -> bool {
    let had_value = slot.is_some();
    *slot = None;
    had_value
}

fn set_actions_window_parent_kind(kind: Option<AutomationWindowKind>) {
    if let Ok(mut guard) = ACTIONS_WINDOW_PARENT_KIND
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *guard = kind;
    }
}

/// Parent window kind of the currently open actions window, if any.
pub fn actions_window_parent_kind() -> Option<AutomationWindowKind> {
    ACTIONS_WINDOW_PARENT_KIND
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| *guard)
}

/// True when the detached actions window is open AND hosted by the main
/// launcher window. Secondary-window popups (Notes, etc.) must return false
/// so the main app's routers leave their keys alone.
pub fn is_actions_window_open_for_main() -> bool {
    is_actions_window_open()
        && matches!(
            actions_window_parent_kind(),
            Some(AutomationWindowKind::Main)
        )
}

fn clear_actions_window_handle(reason: &str) {
    set_actions_window_parent_kind(None);
    let Some(window_storage) = ACTIONS_WINDOW.get() else {
        crate::logging::log(
            "ACTIONS",
            &format!(
                "ACTIONS_WINDOW_LIFECYCLE clear_actions_window_handle skipped: reason={}, state=uninitialized",
                reason
            ),
        );
        return;
    };

    match window_storage.lock() {
        Ok(mut guard) => {
            let had_handle = clear_window_slot(&mut guard);
            crate::logging::log(
                "ACTIONS",
                &format!(
                    "ACTIONS_WINDOW_LIFECYCLE clear_actions_window_handle: reason={}, had_handle={}",
                    reason, had_handle
                ),
            );
        }
        Err(error) => {
            crate::logging::log(
                "ACTIONS",
                &format!(
                    "ACTIONS_WINDOW_LIFECYCLE clear_actions_window_handle failed: reason={}, error={}",
                    reason, error
                ),
            );
        }
    }
}

/// Actions window width (height is calculated dynamically based on content)
#[cfg(test)]
const ACTIONS_WINDOW_WIDTH: f32 = POPUP_WIDTH;
/// Horizontal margin from main window right edge
#[cfg(test)]
const ACTIONS_MARGIN_X: f32 = 8.0;
/// Vertical margin from header/footer
#[cfg(test)]
const ACTIONS_MARGIN_Y: f32 = 8.0;
/// Titlebar height (for top-anchored positioning)
#[cfg(test)]
const TITLEBAR_HEIGHT: f32 = 36.0;

#[inline]
fn current_actions_window_width() -> f32 {
    crate::designs::current_actions_popup_theme().shell.width
}

/// Window position relative to the parent window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // Some variants reserved for future use
pub enum WindowPosition {
    /// Bottom-right, above the footer (default for Cmd+K actions)
    #[default]
    BottomRight,
    /// Top-right, below the titlebar (for new chat dropdown)
    TopRight,
    /// Top-center, below the titlebar, horizontally centered (Raycast-style for Notes)
    TopCenter,
}

/// ActionsWindow wrapper that renders the shared ActionsDialog entity
pub struct ActionsWindow {
    /// The shared dialog entity (created by main app, rendered here)
    pub dialog: Entity<ActionsDialog>,
    /// Focus handle for this window (not actively used - main window keeps focus)
    pub focus_handle: FocusHandle,
    /// Keep activation observer alive so blur-driven auto-close is reliable.
    activation_subscription: Option<Subscription>,
    close_requested: bool,
    parent_automation_id: String,
    parent_kind: AutomationWindowKind,
    /// Authoritative outer size for this popup lifetime.
    fixed_shell_size: Size<Pixels>,
    /// Root/unfiltered inputs that produced `fixed_shell_size`.
    opening_shell_basis: ActionsDialogShellSizingSnapshot,
    registered_displayed_shortcuts: HashSet<String>,
    did_request_focus: bool,
}

impl ActionsWindow {
    fn new(
        dialog: Entity<ActionsDialog>,
        parent_automation_id: String,
        parent_kind: AutomationWindowKind,
        fixed_shell_size: Size<Pixels>,
        opening_shell_basis: ActionsDialogShellSizingSnapshot,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        Self {
            dialog,
            focus_handle,
            activation_subscription: None,
            close_requested: false,
            parent_automation_id,
            parent_kind,
            fixed_shell_size,
            opening_shell_basis,
            registered_displayed_shortcuts: HashSet::new(),
            did_request_focus: false,
        }
    }

    fn sync_displayed_action_shortcut_keybindings(&mut self, cx: &mut Context<Self>) {
        let specs = {
            let dialog = self.dialog.read(cx);
            crate::actions::displayed_action_keybinding_specs(
                &dialog.actions,
                &dialog.filtered_actions,
            )
        };

        let mut bindings = Vec::new();
        let registered_before = self.registered_displayed_shortcuts.len();
        for spec in specs {
            // Popup-owned navigation always wins over row shortcuts. A host may
            // advertise Escape as its conversation Back action, but while the
            // Actions window is open that same key must close exactly one
            // overlay rather than execute the host action underneath it.
            if actions_window_reserves_shortcut(&spec.canonical) {
                continue;
            }
            if !self
                .registered_displayed_shortcuts
                .insert(spec.canonical.clone())
            {
                continue;
            }
            crate::logging::log(
                "KEY_BIND",
                &format!(
                    "ACTIONS_POPUP_SHORTCUT_BIND canonical={} gpui={} context=actions_popup action=MainListDisplayedActionShortcut",
                    spec.canonical, spec.gpui_keystroke
                ),
            );
            bindings.push(gpui::KeyBinding::new(
                &spec.gpui_keystroke,
                crate::actions::MainListDisplayedActionShortcut {
                    shortcut: spec.canonical,
                },
                Some("actions_popup"),
            ));
        }

        if !bindings.is_empty() {
            cx.bind_keys(bindings);
        }
        if self.registered_displayed_shortcuts.len() != registered_before {
            crate::logging::log(
                "KEY_SETUP",
                &format!(
                    "ACTIONS_POPUP_SHORTCUT_SYNC context=actions_popup new_bindings={} registered_total={} env_shortcut_debug={} parent_automation_id={} parent_kind={:?}",
                    self.registered_displayed_shortcuts.len() - registered_before,
                    self.registered_displayed_shortcuts.len(),
                    std::env::var("SCRIPT_KIT_SHORTCUT_DEBUG")
                        .unwrap_or_else(|_| "<unset>".to_string()),
                    self.parent_automation_id,
                    self.parent_kind
                ),
            );
        }
    }

    fn defer_close(
        window: &mut Window,
        cx: &mut Context<Self>,
        reason: &'static str,
        dialog: Entity<ActionsDialog>,
    ) {
        crate::logging::log(
            "ACTIONS",
            &format!("ACTIONS_WINDOW_LIFECYCLE defer_close_scheduled: reason={reason}"),
        );
        let dialog_for_close = dialog;
        window.defer(cx, move |window, cx| {
            crate::logging::log(
                "ACTIONS",
                &format!("ACTIONS_WINDOW_LIFECYCLE defer_close_executing: reason={reason}"),
            );
            clear_actions_popup_automation_snapshot();
            crate::windows::automation_surface_collector::remove_actions_dialog_snapshot(
                "actions-dialog",
            );
            crate::windows::remove_runtime_window_handle("actions-dialog");
            crate::windows::remove_automation_window("actions-dialog");
            clear_actions_window_handle(reason);
            dialog_for_close.update(cx, |dialog, _cx| {
                dialog.release_fixed_shell();
            });
            crate::platform::dematerialize_then_remove_gpui_window_from_app(
                window,
                cx,
                "ACTIONS",
                "Actions popup",
            );
        });
    }

    fn request_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        reason: &'static str,
        activate_main_window: bool,
    ) {
        if self.close_requested {
            crate::logging::log(
                "ACTIONS",
                &format!(
                    "ACTIONS_WINDOW_LIFECYCLE request_close_ignored: reason={reason}, already_requested=true"
                ),
            );
            return;
        }
        self.close_requested = true;

        crate::logging::log(
            "ACTIONS",
            &format!(
                "ACTIONS_WINDOW_LIFECYCLE request_close: reason={reason}, activate_main_window={activate_main_window}"
            ),
        );

        // Activate the parent window BEFORE scheduling focus restoration.
        // macOS window activation is async; starting it early gives the OS
        // more time to make the parent window key before the deferred
        // on_close callback runs and sets pending focus.
        if activate_main_window {
            if self.parent_kind == AutomationWindowKind::Main {
                platform::activate_main_window();
            } else if let Some(parent_handle) =
                crate::windows::get_runtime_window_handle(&self.parent_automation_id)
            {
                // Secondary hosts (Notes, detached Agent Chat) keep their
                // popups when AppKit promotes the popup to key window. On
                // Escape/Cmd+K the key status must hand back to the host
                // window, or keyboard focus lands nowhere after the close.
                cx.defer(move |cx| {
                    let _ = parent_handle.update(cx, |_root, window, _cx| {
                        window.activate_window();
                    });
                });
            }
        }

        if let Some(on_close) = self.dialog.read(cx).on_close.clone() {
            on_close(cx);
        }

        Self::defer_close(window, cx, reason, self.dialog.clone());
    }

    fn ensure_activation_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.activation_subscription.is_some() {
            return;
        }

        crate::logging::log(
            "ACTIONS",
            "ACTIONS_WINDOW_LIFECYCLE activation_subscription_initialized",
        );

        self.activation_subscription = Some(cx.observe_window_activation(window, |this, window, cx| {
            let parent_window_focused = actions_parent_window_focused(&this.parent_automation_id);
            let actions_window_active = window.is_window_active();
            let should_close =
                should_auto_close_actions_window(parent_window_focused, actions_window_active);

            crate::logging::log(
                "ACTIONS",
                &format!(
                    "ACTIONS_WINDOW_LIFECYCLE activation_changed: parent_window_focused={}, actions_window_active={}, should_close={}, parent_automation_id={}",
                    parent_window_focused, actions_window_active, should_close, this.parent_automation_id
                ),
            );

            if !should_close {
                return;
            }

            this.request_close(window, cx, "focus_lost", false);
        }));
    }

    /// Handle one keystroke for the actions popup. Shared by the popup
    /// window's own key listener and by parent-window routers
    /// (`route_key_to_detached_actions_window`) for hosts whose parent window
    /// stays the key window while the detached popup is open.
    ///
    /// Returns `true` when the popup consumed the key.
    fn handle_key_event(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        modifiers: &gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if key.eq_ignore_ascii_case("tab") {
            // Actions has no tab stop traversal; consume the named key before
            // AppKit can offer its control character to the single-line input.
            return true;
        }
        let intent = actions_window_key_intent(key, key_char, modifiers);
        let input_focused = self.dialog.read(cx).search_input_is_focused(window, cx);
        if input_focused
            && matches!(
                intent,
                Some(
                    ActionsWindowKeyIntent::Backspace
                        | ActionsWindowKeyIntent::BackspaceWord
                        | ActionsWindowKeyIntent::MoveCursorLeft { .. }
                        | ActionsWindowKeyIntent::MoveCursorRight { .. }
                        | ActionsWindowKeyIntent::TypeChar(_)
                )
            )
        {
            // The rendered gpui-component Input receives the native action/text
            // event directly. Returning false prevents the popup host from
            // applying the same edit a second time.
            return false;
        }

        match intent {
            Some(ActionsWindowKeyIntent::MoveUp) => {
                crate::logging::log("ACTIONS", "ActionsWindow: handling UP arrow");

                self.dialog.update(cx, |d, cx| d.move_up(cx));
                cx.notify();
                true
            }
            Some(ActionsWindowKeyIntent::MoveDown) => {
                crate::logging::log("ACTIONS", "ActionsWindow: handling DOWN arrow");
                self.dialog.update(cx, |d, cx| d.move_down(cx));
                cx.notify();
                true
            }
            Some(ActionsWindowKeyIntent::MoveHome) => {
                self.dialog.update(cx, |d, cx| {
                    if let Some(first) = first_selectable_index(&d.grouped_items) {
                        d.selected_index = Some(first);
                        d.reveal_selection_after_navigation(cx);
                    }
                });
                true
            }
            Some(ActionsWindowKeyIntent::MoveEnd) => {
                self.dialog.update(cx, |d, cx| {
                    if let Some(last) = last_selectable_index(&d.grouped_items) {
                        d.selected_index = Some(last);
                        d.reveal_selection_after_navigation(cx);
                    }
                });
                true
            }
            Some(ActionsWindowKeyIntent::MovePageUp) => {
                self.dialog.update(cx, |d, cx| {
                    if d.grouped_items.is_empty() {
                        return;
                    }

                    let current = d
                        .selected_index
                        .or_else(|| first_selectable_index(&d.grouped_items))
                        .unwrap_or(0);
                    let target = current.saturating_sub(ACTIONS_WINDOW_PAGE_JUMP);
                    if let Some(next_index) =
                        selectable_index_at_or_before(&d.grouped_items, target)
                            .or_else(|| first_selectable_index(&d.grouped_items))
                    {
                        d.selected_index = Some(next_index);
                        d.reveal_selection_after_navigation(cx);
                    }
                });
                true
            }
            Some(ActionsWindowKeyIntent::MovePageDown) => {
                self.dialog.update(cx, |d, cx| {
                    if d.grouped_items.is_empty() {
                        return;
                    }

                    let last_index = d.grouped_items.len() - 1;
                    let current = d
                        .selected_index
                        .or_else(|| first_selectable_index(&d.grouped_items))
                        .unwrap_or(0);
                    let target = (current + ACTIONS_WINDOW_PAGE_JUMP).min(last_index);
                    if let Some(next_index) = selectable_index_at_or_after(&d.grouped_items, target)
                        .or_else(|| last_selectable_index(&d.grouped_items))
                    {
                        d.selected_index = Some(next_index);
                        d.reveal_selection_after_navigation(cx);
                    }
                });
                true
            }
            Some(ActionsWindowKeyIntent::SendToAgentChat) => {
                if let Some(action) = self.dialog.read(cx).get_selected_action().cloned() {
                    let target =
                        crate::ai::build_action_target_for_ai(&action, "DetachedActionsWindow");
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_actions_window_cmd_enter",
                        action_id = %action.id,
                        semantic_id = %target.semantic_id,
                    );
                    // Use the shared secondary-window handoff helper to enqueue
                    // the target. Pass show_main_window=false because request_close
                    // already handles activate_main_window with the correct timing
                    // (before defer_close, not after).
                    crate::ai::request_explicit_agent_chat_handoff_from_secondary_window(
                        target,
                        "DetachedActionsWindow",
                        false,
                    );
                    self.request_close(window, cx, "send_to_agent_chat", true);
                }
                true
            }
            Some(ActionsWindowKeyIntent::ExecuteSelected) => {
                let activation = self.dialog.update(cx, |d, cx| d.activate_selected(cx));
                self.handle_dialog_activation(activation, window, cx, "execute_selected");
                true
            }
            Some(ActionsWindowKeyIntent::Close) => {
                let outcome = self.dialog.update(cx, |d, cx| d.handle_escape(cx));
                match outcome {
                    super::dialog::ActionsDialogEscapeOutcome::PoppedRoute => {
                        self.dialog.update(cx, |d, cx| {
                            d.sync_search_input_from_model(window, cx);
                        });
                        let (route_id, search_placeholder, route_depth, escape_hint) = {
                            let dialog = self.dialog.read(cx);
                            (
                                dialog.current_route_id().map(str::to_string),
                                dialog.current_search_placeholder().map(str::to_string),
                                dialog.route_depth(),
                                dialog.route_hint_label(),
                            )
                        };
                        tracing::info!(
                            target: "script_kit::actions",
                            host = "detached_actions_window",
                            route_id = ?route_id,
                            route_depth,
                            escape_hint,
                            search_placeholder = ?search_placeholder,
                            "actions_dialog_route_visible"
                        );
                    }
                    super::dialog::ActionsDialogEscapeOutcome::CloseDialog => {
                        self.request_close(window, cx, "escape", true);
                    }
                }
                true
            }
            Some(ActionsWindowKeyIntent::Dismiss) => {
                let route_depth = self.dialog.read(cx).route_depth();
                match crate::window_orchestrator::interaction::plan_overlay_dismiss(
                    route_depth,
                    crate::window_orchestrator::interaction::OverlayDismissTrigger::ActionsToggle,
                ) {
                    crate::window_orchestrator::interaction::OverlayDismissDecision::CloseOverlay => {
                        self.request_close(window, cx, "cmd_k_toggle", true);
                    }
                    crate::window_orchestrator::interaction::OverlayDismissDecision::PopRoute => {
                        tracing::warn!(
                            target: "script_kit::actions",
                            route_depth,
                            "actions toggle unexpectedly requested route navigation"
                        );
                    }
                }
                true
            }
            Some(ActionsWindowKeyIntent::Backspace) => {
                crate::logging::log("ACTIONS", "ActionsWindow: backspace pressed");
                self.dialog.update(cx, |d, cx| {
                    d.backspace_search_input(window, cx);
                });
                cx.notify();
                true
            }
            Some(ActionsWindowKeyIntent::BackspaceWord) => {
                crate::logging::log("ACTIONS", "ActionsWindow: word backspace pressed");
                self.dialog.update(cx, |d, cx| {
                    d.delete_previous_search_word(window, cx);
                });
                cx.notify();
                true
            }
            Some(ActionsWindowKeyIntent::MoveCursorLeft { select }) => {
                self.dialog.update(cx, |d, cx| {
                    d.move_search_cursor_left(select, window, cx);
                });
                true
            }
            Some(ActionsWindowKeyIntent::MoveCursorRight { select }) => {
                self.dialog.update(cx, |d, cx| {
                    d.move_search_cursor_right(select, window, cx);
                });
                true
            }
            Some(ActionsWindowKeyIntent::TypeChar(ch)) => {
                crate::logging::log("ACTIONS", &format!("ActionsWindow: char '{}' pressed", ch));
                self.dialog.update(cx, |d, cx| {
                    d.insert_search_text(ch.to_string(), window, cx);
                });
                cx.notify();
                true
            }
            None => {
                let matched_action_id = {
                    let dialog = self.dialog.read(cx);
                    crate::actions::matching_filtered_action_id_for_keystroke(
                        &dialog.actions,
                        &dialog.filtered_actions,
                        key,
                        modifiers,
                    )
                };
                if let Some(action_id) = matched_action_id {
                    tracing::info!(
                        target: "script_kit::actions",
                        event = "actions_window_shortcut_matched",
                        action_id = %action_id,
                        key = %key,
                        platform = modifiers.platform,
                        shift = modifiers.shift,
                        control = modifiers.control,
                        alt = modifiers.alt,
                    );
                    let activation = self
                        .dialog
                        .update(cx, |d, cx| d.activate_action_id(action_id, cx));
                    self.handle_dialog_activation(activation, window, cx, "shortcut_execute");
                    true
                } else if input_focused
                    && modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt
                    && matches!(key.to_ascii_lowercase().as_str(), "a" | "v" | "z")
                {
                    // The input's key context owns select-all, paste, undo, and
                    // redo once row-shortcut matching has had first refusal.
                    false
                } else if modifiers.platform
                    && !modifiers.shift
                    && !modifiers.control
                    && !modifiers.alt
                    && key.eq_ignore_ascii_case("v")
                {
                    // Cmd+V pastes into the popup search, like the main
                    // search input. Runs after shortcut matching so a host
                    // action that binds ⌘V keeps its row shortcut.
                    self.dialog.update(cx, |d, cx| {
                        d.paste_search_input(window, cx);
                    });
                    cx.notify();
                    true
                } else if modifiers.platform
                    && !modifiers.shift
                    && !modifiers.control
                    && !modifiers.alt
                    && key.eq_ignore_ascii_case("a")
                {
                    self.dialog.update(cx, |d, cx| {
                        d.select_all_search_input(window, cx);
                    });
                    true
                } else if modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt
                    && key.eq_ignore_ascii_case("z")
                {
                    self.dialog.update(cx, |d, cx| {
                        if modifiers.shift {
                            d.redo_search_input(window, cx);
                        } else {
                            d.undo_search_input(window, cx);
                        }
                    });
                    true
                } else {
                    false
                }
            }
        }
    }

    fn handle_dialog_activation(
        &mut self,
        activation: super::dialog::ActionsDialogActivation,
        window: &mut Window,
        cx: &mut Context<Self>,
        close_reason: &'static str,
    ) {
        let callback = self.dialog.read(cx).on_activation_callback();
        if let Some(callback) = callback {
            let activation = activation.clone();
            window.defer(cx, move |window, cx| {
                callback(activation, window, cx);
            });
            return;
        }

        match activation {
            super::dialog::ActionsDialogActivation::DrillDownPushed { .. } => {
                self.dialog.update(cx, |d, cx| {
                    d.sync_search_input_from_model(window, cx);
                });
                let (route_id, search_placeholder, route_depth, escape_hint) = {
                    let dialog = self.dialog.read(cx);
                    (
                        dialog.current_route_id().map(str::to_string),
                        dialog.current_search_placeholder().map(str::to_string),
                        dialog.route_depth(),
                        dialog.route_hint_label(),
                    )
                };
                tracing::info!(
                    target: "script_kit::actions",
                    host = "detached_actions_window",
                    route_id = ?route_id,
                    route_depth,
                    escape_hint,
                    search_placeholder = ?search_placeholder,
                    "actions_dialog_route_visible"
                );
            }
            super::dialog::ActionsDialogActivation::Executed {
                action_id,
                should_close,
            } => {
                tracing::info!(
                    event = "actions_window_execute_selected",
                    action = %action_id,
                    should_close,
                );
                if should_close {
                    self.request_close(window, cx, close_reason, true);
                }
            }
            super::dialog::ActionsDialogActivation::Blocked { action_id, reason } => {
                tracing::info!(
                    event = "actions_window_activation_blocked",
                    action = %action_id,
                    reason_fingerprint = %super::dialog::ActionsDialog::devtools_text_fingerprint(&reason),
                );
            }
            super::dialog::ActionsDialogActivation::NoSelection => {}
        }
    }
}

impl Focusable for ActionsWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ActionsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_activation_subscription(window, cx);

        // Log focus state AND window focus state
        let is_focused = self.focus_handle.is_focused(window);
        let window_is_active = window.is_window_active();
        let fixed_width_px = f32::from(self.fixed_shell_size.width);
        let fixed_height_px = f32::from(self.fixed_shell_size.height);
        crate::logging::log(
            "ACTIONS",
            &format!(
                "ActionsWindow render: focus_handle.is_focused={}, window_is_active={}, fixed_shell={:.0}x{:.0}, opening_actions={}",
                is_focused,
                window_is_active,
                fixed_width_px,
                fixed_height_px,
                self.opening_shell_basis.action_count,
            ),
        );

        let parent_window_focused = actions_parent_window_focused(&self.parent_automation_id);
        if should_auto_close_actions_window(parent_window_focused, window_is_active) {
            crate::logging::log(
                "ACTIONS",
                &format!(
                    "ACTIONS_WINDOW_LIFECYCLE render_auto_close: parent_window_focused={}, actions_window_active={}, parent_automation_id={}",
                    parent_window_focused, window_is_active, self.parent_automation_id
                ),
            );
            self.request_close(window, cx, "render_focus_lost", false);
        }

        // Own the GPUI focus once the popup exists. Parent-window interceptors can
        // still route keys while the parent is focused, but when AppKit makes this
        // popup the key window its local context must be focusable too.
        if !self.did_request_focus {
            self.did_request_focus = true;
            let focus_handle = self.focus_handle.clone();
            let dialog = self.dialog.clone();
            window.defer(cx, move |window, cx| {
                let focused_input = dialog.update(cx, |dialog, cx| {
                    dialog.focus_search_input(window, cx)
                });
                if !focused_input {
                    window.focus(&focus_handle, cx);
                }
                crate::logging::log(
                    "KEY_SETUP",
                    if focused_input {
                        "ACTIONS_POPUP_FOCUS_REQUESTED context=actions_search input_owner=gpui_component"
                    } else {
                        "ACTIONS_POPUP_FOCUS_REQUESTED context=actions_popup focus_handle=requested"
                    },
                );
            });
        }

        // Key handler for the actions window
        // Since this is a separate window, it needs its own key handling
        // (the parent window can't route events to us)
        self.sync_displayed_action_shortcut_keybindings(cx);

        let handle_key = cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            let modifiers = &event.keystroke.modifiers;

            crate::logging::log(
                "KEY_ROUTE",
                &format!(
                    "ACTIONS_POPUP_KEYDOWN key='{}' shortcut={} modifiers={:?} focus_handle_focused={} window_active={}",
                    key,
                    crate::shortcuts::keystroke_to_shortcut(key, modifiers),
                    modifiers,
                    this.focus_handle.is_focused(window),
                    window.is_window_active()
                ),
            );

            if this.handle_key_event(key, event.keystroke.key_char.as_deref(), modifiers, window, cx)
            {
                cx.stop_propagation();
            }
        });

        // Render inside the full hosting window. The native outer size is frozen
        // at open; the dialog owns only interior flex/scroll changes.
        div()
            .size_full()
            .key_context("actions_popup")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(
                |this, action: &crate::actions::MainListDisplayedActionShortcut, window, cx| {
                    crate::logging::log(
                        "KEY_ROUTE",
                        &format!(
                            "ActionsWindow displayed shortcut keybinding received canonical={} context=actions_popup",
                            action.shortcut
                        ),
                    );
                    let matched_action_id = {
                        let dialog = this.dialog.read(cx);
                        crate::actions::matching_action_id_for_canonical_shortcut(
                            &dialog.actions,
                            &dialog.filtered_actions,
                            &action.shortcut,
                        )
                    };
                    if let Some(action_id) = matched_action_id {
                        let activation = this
                            .dialog
                            .update(cx, |d, cx| d.activate_action_id(action_id, cx));
                        this.handle_dialog_activation(
                            activation,
                            window,
                            cx,
                            "displayed_shortcut_keybinding",
                        );
                    }
                },
            ))
            .on_key_down(handle_key)
            .child(self.dialog.clone())
    }
}

#[cfg(test)]
#[path = "tests/window_lifecycle.rs"]
mod window_lifecycle_tests;

// --- merged from part_02.rs ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actions_window_reserves_overlay_dismissal_shortcuts() {
        assert!(actions_window_reserves_shortcut("escape"));
        assert!(actions_window_reserves_shortcut("cmd+k"));
        assert!(!actions_window_reserves_shortcut("cmd+shift+c"));
    }

    #[test]
    fn test_actions_window_key_intent_supports_aliases_and_jump_keys() {
        let no_mods = gpui::Modifiers::default();

        assert_eq!(
            actions_window_key_intent("return", None, &no_mods),
            Some(ActionsWindowKeyIntent::ExecuteSelected)
        );
        assert_eq!(
            actions_window_key_intent("esc", None, &no_mods),
            Some(ActionsWindowKeyIntent::Close)
        );
        assert_eq!(
            actions_window_key_intent("home", None, &no_mods),
            Some(ActionsWindowKeyIntent::MoveHome)
        );
        assert_eq!(
            actions_window_key_intent("end", None, &no_mods),
            Some(ActionsWindowKeyIntent::MoveEnd)
        );
        assert_eq!(
            actions_window_key_intent("pageup", None, &no_mods),
            Some(ActionsWindowKeyIntent::MovePageUp)
        );
        assert_eq!(
            actions_window_key_intent("pagedown", None, &no_mods),
            Some(ActionsWindowKeyIntent::MovePageDown)
        );
    }

    #[test]
    fn test_actions_window_dynamic_height_matches_single_row_when_empty() {
        let row_height = crate::designs::current_actions_popup_theme()
            .list
            .row_height;
        let empty_height = actions_window_dynamic_height(
            0,
            0,
            false,
            false,
            false,
            crate::actions::constants::POPUP_MAX_HEIGHT,
            row_height,
        );
        let single_row_height = actions_window_dynamic_height(
            1,
            0,
            false,
            false,
            false,
            crate::actions::constants::POPUP_MAX_HEIGHT,
            row_height,
        );

        assert!(
            (empty_height - single_row_height).abs() < 0.001,
            "empty_height={empty_height}, single_row_height={single_row_height}"
        );
    }

    #[test]
    fn test_actions_window_dynamic_height_includes_footer_height() {
        let row_height = crate::designs::current_actions_popup_theme()
            .list
            .row_height;
        let without_footer = actions_window_dynamic_height(
            3,
            1,
            false,
            true,
            false,
            crate::actions::constants::POPUP_MAX_HEIGHT,
            row_height,
        );
        let with_footer = actions_window_dynamic_height(
            3,
            1,
            false,
            true,
            true,
            crate::actions::constants::POPUP_MAX_HEIGHT,
            row_height,
        );

        assert!(
            (with_footer - without_footer - 32.0).abs() < 0.001,
            "without_footer={without_footer}, with_footer={with_footer}"
        );
    }

    #[test]
    fn test_actions_window_dynamic_height_clamps_to_live_shell_max_height() {
        let mut tokens = crate::designs::base_actions_popup_theme();
        tokens.shell.max_height = 240.0;
        tokens.list.padding_top = 10.0;
        tokens.list.padding_bottom = 14.0;

        let row_height = tokens.list.row_height;
        let height =
            resolved_actions_popup_height(&tokens, (20, 4), false, true, false, 400.0, row_height);

        assert_eq!(height, 242.0);
    }
}

/// Single source of truth for Actions shell height. Detached windows evaluate
/// it once from the opening root/unfiltered snapshot; inline dialogs use it for
/// their local content-derived shell.
#[inline]
pub(super) fn actions_window_dynamic_height(
    num_actions: usize,
    section_header_count: usize,
    hide_search: bool,
    has_header: bool,
    show_footer: bool,
    max_height: f32,
    row_height: f32,
) -> f32 {
    let tokens = crate::designs::current_actions_popup_theme();
    resolved_actions_popup_height(
        &tokens,
        (num_actions, section_header_count),
        hide_search,
        has_header,
        show_footer,
        max_height,
        row_height,
    )
}

/// Pure popup-height formula over an explicit token definition. Production
/// passes `current_actions_popup_theme()`; the design-contract exporter passes
/// `base_actions_popup_theme()` so checked-in artifacts always match the base
/// token definition.
pub(crate) fn resolved_actions_popup_height(
    tokens: &crate::designs::ActionsPopupThemeDef,
    row_counts: (usize, usize),
    hide_search: bool,
    has_header: bool,
    show_footer: bool,
    max_height: f32,
    row_height: f32,
) -> f32 {
    let (num_actions, section_header_count) = row_counts;
    const POPUP_FOOTER_HEIGHT: f32 = 32.0;
    let search_box_height = if hide_search {
        0.0
    } else {
        tokens.search.height
    };
    let header_height = if has_header {
        tokens.context_header.height
    } else {
        0.0
    };
    let footer_height = if show_footer {
        POPUP_FOOTER_HEIGHT
    } else {
        0.0
    };
    let section_headers_height = section_header_count as f32 * tokens.list.section_header_height;
    let min_items_height = if num_actions == 0 {
        tokens.list.empty_row_height
    } else {
        0.0
    };
    let list_padding_height = tokens.list.padding_top + tokens.list.padding_bottom;
    let max_height = max_height.min(tokens.shell.max_height);
    let items_height = ((num_actions as f32 * row_height + section_headers_height)
        .max(min_items_height)
        + list_padding_height)
        .min(max_height - search_box_height - header_height - footer_height);
    let border_height = tokens.shell.border_height;
    items_height + search_box_height + header_height + footer_height + border_height
}

/// Compute the origin point for the actions popup window.
///
/// Pure helper that encapsulates all position-dependent origin math so it can
/// be tested without standing up a real window.
fn actions_popup_origin(
    main_window_bounds: Bounds<Pixels>,
    window_width: Pixels,
    window_height: Pixels,
    position: WindowPosition,
) -> Point<Pixels> {
    let tokens = crate::designs::current_actions_popup_theme();
    let right_aligned_x = main_window_bounds.origin.x + main_window_bounds.size.width
        - window_width
        - px(tokens.shell.margin_x);

    let y = match position {
        WindowPosition::BottomRight => {
            main_window_bounds.origin.y + main_window_bounds.size.height
                - window_height
                - px(FOOTER_HEIGHT)
                - px(tokens.shell.margin_y)
        }
        WindowPosition::TopRight | WindowPosition::TopCenter => {
            main_window_bounds.origin.y
                + px(tokens.shell.titlebar_offset_y)
                + px(tokens.shell.margin_y)
        }
    };

    let x = match position {
        WindowPosition::TopCenter => {
            main_window_bounds.origin.x + (main_window_bounds.size.width - window_width) / 2.0
        }
        _ => right_aligned_x,
    };

    Point { x, y }
}

/// Full popup bounds (origin + size) for the actions window.
///
/// Wraps [`actions_popup_origin`] so callers get a single `Bounds` value
/// without reconstructing size separately.
fn actions_popup_bounds(
    main_window_bounds: Bounds<Pixels>,
    window_width: Pixels,
    window_height: Pixels,
    position: WindowPosition,
) -> Bounds<Pixels> {
    Bounds {
        origin: actions_popup_origin(main_window_bounds, window_width, window_height, position),
        size: Size {
            width: window_width,
            height: window_height,
        },
    }
}

/// Structured placement receipt for the actions popup.
///
/// Captures all inputs and computed outputs of a placement decision so that
/// agentic callers can verify geometry deterministically.
#[derive(Debug)]
struct ActionsPopupPlacementReceipt {
    position: WindowPosition,
    display_id: Option<DisplayId>,
    main_window_bounds: Bounds<Pixels>,
    popup_bounds: Bounds<Pixels>,
    anchor_x: Pixels,
    anchor_y: Pixels,
    pinned_edge: &'static str,
}

fn actions_popup_placement_receipt(
    main_window_bounds: Bounds<Pixels>,
    window_width: Pixels,
    window_height: Pixels,
    position: WindowPosition,
    display_id: Option<DisplayId>,
) -> ActionsPopupPlacementReceipt {
    let tokens = crate::designs::current_actions_popup_theme();
    let popup_bounds =
        actions_popup_bounds(main_window_bounds, window_width, window_height, position);

    let (anchor_x, anchor_y, pinned_edge) = match position {
        WindowPosition::BottomRight => (
            main_window_bounds.origin.x + main_window_bounds.size.width - px(tokens.shell.margin_x),
            main_window_bounds.origin.y + main_window_bounds.size.height
                - px(FOOTER_HEIGHT)
                - px(tokens.shell.margin_y),
            "bottom",
        ),
        WindowPosition::TopRight => (
            main_window_bounds.origin.x + main_window_bounds.size.width - px(tokens.shell.margin_x),
            main_window_bounds.origin.y
                + px(tokens.shell.titlebar_offset_y)
                + px(tokens.shell.margin_y),
            "top",
        ),
        WindowPosition::TopCenter => (
            main_window_bounds.origin.x + (main_window_bounds.size.width / 2.0),
            main_window_bounds.origin.y
                + px(tokens.shell.titlebar_offset_y)
                + px(tokens.shell.margin_y),
            "top",
        ),
    };

    ActionsPopupPlacementReceipt {
        position,
        display_id,
        main_window_bounds,
        popup_bounds,
        anchor_x,
        anchor_y,
        pinned_edge,
    }
}

fn log_actions_popup_placement(stage: &'static str, receipt: &ActionsPopupPlacementReceipt) {
    let main_origin_x_px: f32 = receipt.main_window_bounds.origin.x.into();
    let main_origin_y_px: f32 = receipt.main_window_bounds.origin.y.into();
    let main_width_px: f32 = receipt.main_window_bounds.size.width.into();
    let main_height_px: f32 = receipt.main_window_bounds.size.height.into();

    let popup_origin_x_px: f32 = receipt.popup_bounds.origin.x.into();
    let popup_origin_y_px: f32 = receipt.popup_bounds.origin.y.into();
    let popup_width_px: f32 = receipt.popup_bounds.size.width.into();
    let popup_height_px: f32 = receipt.popup_bounds.size.height.into();

    let anchor_x_px: f32 = receipt.anchor_x.into();
    let anchor_y_px: f32 = receipt.anchor_y.into();

    tracing::info!(
        target: "ACTIONS_POPUP",
        stage = stage,
        position = ?receipt.position,
        display_id = ?receipt.display_id,
        pinned_edge = receipt.pinned_edge,
        main_origin_x_px,
        main_origin_y_px,
        main_width_px,
        main_height_px,
        popup_origin_x_px,
        popup_origin_y_px,
        popup_width_px,
        popup_height_px,
        anchor_x_px,
        anchor_y_px,
        "actions popup placement receipt"
    );
}

fn protocol_bounds_json(bounds: Bounds<Pixels>) -> serde_json::Value {
    serde_json::json!({
        "x": f32::from(bounds.origin.x) as f64,
        "y": f32::from(bounds.origin.y) as f64,
        "width": f32::from(bounds.size.width) as f64,
        "height": f32::from(bounds.size.height) as f64,
    })
}

fn next_actions_popup_automation_generation() -> u64 {
    let storage = ACTIONS_POPUP_AUTOMATION_GENERATION.get_or_init(|| Mutex::new(0));
    match storage.lock() {
        Ok(mut guard) => {
            *guard += 1;
            *guard
        }
        Err(_) => 0,
    }
}

fn clear_actions_popup_automation_snapshot() {
    let storage = ACTIONS_POPUP_AUTOMATION_SNAPSHOT.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        *guard = None;
    }
}

fn unregister_actions_dialog_automation_surfaces() {
    clear_actions_popup_automation_snapshot();
    crate::windows::automation_surface_collector::remove_actions_dialog_snapshot("actions-dialog");
    crate::windows::remove_runtime_window_handle("actions-dialog");
    crate::windows::remove_automation_window("actions-dialog");
}

fn record_actions_popup_automation_snapshot(
    parent_automation_id: &str,
    parent_kind: AutomationWindowKind,
    receipt: &ActionsPopupPlacementReceipt,
    fixed_shell_size: Size<Pixels>,
    opening_shell_basis: &ActionsDialogShellSizingSnapshot,
) {
    let generation = next_actions_popup_automation_generation();
    let snapshot = serde_json::json!({
        "schemaVersion": 1,
        "generation": generation,
        "updatedStage": "open",
        "stale": false,
        "host": match parent_kind {
            AutomationWindowKind::Notes => "notes.actions",
            AutomationWindowKind::Main => "main.actions",
            AutomationWindowKind::AgentChatDetached => "agentChatDetached.actions",
            AutomationWindowKind::Dictation => "dictation.actions",
            AutomationWindowKind::ActionsDialog => "actionsDialog.actions",
            AutomationWindowKind::PromptPopup => "promptPopup.actions",
            AutomationWindowKind::Hud => "hud.actions",
        },
        "parentAutomationId": parent_automation_id,
        "parentKind": format!("{parent_kind:?}"),
        "position": format!("{:?}", receipt.position),
        "displayId": format!("{:?}", receipt.display_id),
        "fixedShell": {
            "fixedForLifetime": true,
            "policy": "rootUnfilteredAtOpen",
            "widthPx": f32::from(fixed_shell_size.width) as f64,
            "heightPx": f32::from(fixed_shell_size.height) as f64,
            "openingBasis": opening_shell_basis,
        },
        "geometry": {
            "popupRect": protocol_bounds_json(receipt.popup_bounds),
            "parentRect": protocol_bounds_json(receipt.main_window_bounds),
            "anchorRect": {
                "x": f32::from(receipt.anchor_x) as f64,
                "y": f32::from(receipt.anchor_y) as f64,
                "width": 0.0,
                "height": 0.0,
            },
            "position": format!("{:?}", receipt.position),
            "pinnedEdge": receipt.pinned_edge,
            "displayId": format!("{:?}", receipt.display_id),
        },
    });
    let storage = ACTIONS_POPUP_AUTOMATION_SNAPSHOT.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        *guard = Some(snapshot);
    }
}

pub(crate) fn actions_popup_automation_snapshot() -> Option<serde_json::Value> {
    ACTIONS_POPUP_AUTOMATION_SNAPSHOT
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

#[cfg(target_os = "macos")]
fn actions_popup_ns_window(window: &mut Window) -> Option<cocoa::base::id> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use cocoa::base::nil;
            use objc::{msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as cocoa::base::id;
            // SAFETY: `ns_view` comes from the live GPUI window on the AppKit main
            // thread. `-[NSView window]` returns the owning NSWindow or nil.
            unsafe {
                let ns_window: cocoa::base::id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some(ns_window);
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn attach_actions_popup_to_parent_window(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    child_ns_window: cocoa::base::id,
) {
    let attach_result = cx.update_window(parent_window_handle, move |_, parent_window, _cx| {
        let Some(parent_ns_window) = actions_popup_ns_window(parent_window) else {
            return false;
        };

        // SAFETY: both NSWindow pointers come from live GPUI windows on the main
        // thread. We guard against nil/equal pointers before attaching so AppKit
        // only receives distinct parent/child windows.
        unsafe {
            use cocoa::base::nil;
            use objc::{msg_send, sel, sel_impl};

            if parent_ns_window == nil
                || child_ns_window == nil
                || parent_ns_window == child_ns_window
            {
                return false;
            }

            let _: () =
                msg_send![parent_ns_window, addChildWindow:child_ns_window ordered:NS_WINDOW_ABOVE];
            let _: () = msg_send![child_ns_window, orderFrontRegardless];

            tracing::info!(
                target: "script_kit::actions",
                event = "actions_popup_attached_to_parent",
                parent = format!("{:?}", parent_ns_window),
                child = format!("{:?}", child_ns_window),
                "Attached actions popup as native child window"
            );
        }

        true
    });

    match attach_result {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(
                target: "script_kit::actions",
                event = "actions_popup_attach_parent_skipped",
                "Skipped attaching actions popup as native child window"
            );
        }
        Err(error) => {
            tracing::warn!(
                target: "script_kit::actions",
                event = "actions_popup_attach_parent_failed",
                error = ?error,
                "Failed to attach actions popup as native child window"
            );
        }
    }
}

fn resolve_actions_popup_parent_automation_id(
    parent_window_handle: AnyWindowHandle,
    parent_window_bounds: Bounds<Pixels>,
    parent_automation_id: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(id) = parent_automation_id {
        return Ok(id.to_string());
    }

    let Some(main_window_handle) = crate::get_main_window_handle() else {
        tracing::warn!(
            target: "script_kit::actions",
            event = "actions_popup_open_blocked_missing_parent",
            "Actions popup open blocked: no parent automation identity"
        );
        anyhow::bail!("Cannot open actions popup: parent automation identity is required");
    };

    if main_window_handle != parent_window_handle {
        tracing::warn!(
            target: "script_kit::actions",
            event = "actions_popup_open_blocked_missing_parent",
            "Actions popup open blocked: no parent automation identity"
        );
        anyhow::bail!("Cannot open actions popup: parent automation identity is required");
    }

    let synthesized_parent_id = "main".to_string();
    crate::windows::upsert_runtime_window_handle(&synthesized_parent_id, parent_window_handle);

    // Preserve the existing main window's semantic_surface if the registry
    // already has one (e.g. "clipboardHistory" when the clipboard-history
    // builtin is hosted in main, or "fileSearch" for file-search, or
    // "agentChatChat" for embedded Agent Chat). Previously this `upsert_automation_window`
    // call hardcoded `semantic_surface: "scriptList"` and so REWROTE main's
    // surface tag mid-flight every time actions opened, which broke any
    // automation caller that routed on surface. See
    // `[?] actions-cmdk-clipboard-main-surface-flip` filed Run 7 Pass #17
    // and independently reproduced Pass #20.
    let preserved_semantic_surface = crate::windows::list_automation_windows()
        .into_iter()
        .find(|w| w.id == synthesized_parent_id)
        .and_then(|w| w.semantic_surface)
        .unwrap_or_else(|| "scriptList".to_string());

    crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
        id: synthesized_parent_id.clone(),
        kind: crate::protocol::AutomationWindowKind::Main,
        title: Some("Script Kit".to_string()),
        focused: true,
        visible: true,
        semantic_surface: Some(preserved_semantic_surface),
        bounds: Some(crate::protocol::AutomationWindowBounds {
            x: f32::from(parent_window_bounds.origin.x) as f64,
            y: f32::from(parent_window_bounds.origin.y) as f64,
            width: f32::from(parent_window_bounds.size.width) as f64,
            height: f32::from(parent_window_bounds.size.height) as f64,
        }),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
        generation: None,
    });
    tracing::info!(
        target: "script_kit::actions",
        event = "actions_popup_synthesized_main_parent",
        parent_window_id = %synthesized_parent_id,
        "Synthesized main-window automation identity for actions popup"
    );

    Ok(synthesized_parent_id)
}

/// Open the actions window as a separate floating window with vibrancy.
/// It opens without taking key-window status so the parent keeps its active
/// drop shadow; keys arrive via parent routing, or locally after a click
/// promotes the popup (`setBecomesKeyOnlyIfNeeded:`).
///
/// # Arguments
/// * `cx` - The application context
/// * `parent_window_handle` - The window that owns the popup
/// * `main_window_bounds` - The bounds of the parent window in SCREEN-RELATIVE coordinates
///   (as returned by GPUI's window.bounds() - top-left origin relative to the window's screen)
/// * `display_id` - The display where the parent window is located (actions window will be on same display)
/// * `dialog_entity` - The shared ActionsDialog entity (created by main app)
/// * `position` - Where to position the window relative to the parent window
///
/// # Returns
/// The window handle on success
pub fn open_actions_window(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    main_window_bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
    dialog_entity: Entity<ActionsDialog>,
    position: WindowPosition,
    parent_automation_id: Option<&str>,
) -> anyhow::Result<WindowHandle<ActionsWindow>> {
    crate::platform::host_clock::log_entry_timeline_event("actions_open_requested");
    let parent_automation_id = resolve_actions_popup_parent_automation_id(
        parent_window_handle,
        main_window_bounds,
        parent_automation_id,
    )?;
    let parent_kind = crate::windows::automation_window_by_id(&parent_automation_id)
        .map(|info| info.kind)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot open actions popup: parent '{}' is missing from automation registry",
                parent_automation_id
            )
        })?;

    // Close any existing actions window first
    close_actions_window(cx);
    set_actions_window_parent_kind(Some(parent_kind));

    // Load theme for vibrancy settings
    let theme = get_cached_theme();
    let is_dark_vibrancy = theme.should_use_dark_vibrancy();
    let window_background = if theme.is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    // Freeze the detached shell once from the root, unfiltered route. Search,
    // route, and action mutations reflow or scroll inside these outer bounds.
    let opening_shell_basis = dialog_entity.read(cx).opening_shell_sizing_snapshot();
    let fixed_height = actions_window_dynamic_height(
        opening_shell_basis.action_count,
        opening_shell_basis.section_header_count,
        !opening_shell_basis.search_visible,
        opening_shell_basis.context_header_visible,
        opening_shell_basis.footer_visible,
        opening_shell_basis.max_height_px,
        opening_shell_basis.row_height_px,
    );

    // Calculate window position:
    // - X: Right edge of main window, minus actions width, minus margin
    // - Y: Depends on position parameter:
    //   - BottomRight: Above footer, aligned to bottom
    //   - TopRight: Below titlebar, aligned to top
    //
    // CRITICAL: main_window_bounds must be in SCREEN-RELATIVE coordinates from GPUI's
    // window.bounds(). These are top-left origin, relative to the window's current screen.
    // When we pass display_id to WindowOptions, GPUI will position this window on the
    // same screen as the main window, using these screen-relative coordinates.
    let window_width = px(current_actions_window_width());
    let window_height = px(fixed_height);
    let fixed_shell_size = Size {
        width: window_width,
        height: window_height,
    };

    let receipt = actions_popup_placement_receipt(
        main_window_bounds,
        window_width,
        window_height,
        position,
        display_id,
    );
    log_actions_popup_placement("open", &receipt);

    let bounds = receipt.popup_bounds;

    crate::logging::log(
        "ACTIONS",
        &format!(
            "Opening actions window at ({:?}, {:?}), size {:?}x{:?}, display_id={:?}, position={:?}",
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height,
            display_id,
            position,
        ),
    );

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None, // No titlebar = no drag affordance
        window_background,
        // MUST stay `false`: a focused popup makes GPUI call makeKeyAndOrderFront,
        // which steals key status from the parent panel and visibly drops its
        // active shadow (the popup must read as an attached child, like NSMenu).
        // Keys reach the popup via parent routing (`route_key_to_actions_dialog`,
        // `route_key_to_detached_actions_window`) while the parent stays key, and
        // via the popup's own handlers when AppKit click-promotes it
        // (`setBecomesKeyOnlyIfNeeded:` in `configure_actions_popup_window`).
        focus: false,
        show: true,
        kind: WindowKind::PopUp, // Floating popup window
        display_id,              // CRITICAL: Position on same display as main window
        ..Default::default()
    };

    // Create the window with the shared dialog entity
    // The popup requests its GPUI focus handle from render after the window exists,
    // while parent surfaces can still route keys through their interceptors.
    // Detached windows own their size: let the dialog fill the window bounds
    // so the list reflows WITH the glass morph (reset on close).
    dialog_entity.update(cx, |dialog, _cx| {
        dialog.attach_to_fixed_shell(fixed_height);
    });
    let parent_automation_id_for_window = parent_automation_id.clone();
    let fixed_shell_size_for_window = fixed_shell_size;
    let opening_shell_basis_for_window = opening_shell_basis.clone();
    let handle = match cx.open_window(window_options, |window, cx| {
        dialog_entity.update(cx, |dialog, cx| {
            dialog.ensure_search_input(window, cx);
            dialog.sync_search_input_from_model(window, cx);
        });
        cx.new(|cx| {
            ActionsWindow::new(
                dialog_entity.clone(),
                parent_automation_id_for_window.clone(),
                parent_kind,
                fixed_shell_size_for_window,
                opening_shell_basis_for_window.clone(),
                cx,
            )
        })
    }) {
        Ok(handle) => handle,
        Err(error) => {
            dialog_entity.update(cx, |dialog, _cx| dialog.release_fixed_shell());
            set_actions_window_parent_kind(None);
            return Err(error);
        }
    };

    // Configure the window as non-movable on macOS
    // Use window.defer() to avoid RefCell borrow conflicts - GPUI may still have
    // internal state borrowed immediately after open_window returns.
    #[cfg(target_os = "macos")]
    {
        let configure_result = handle.update(cx, move |_this, window, cx| {
            window.defer(cx, move |window, cx| {
                if let Some(ns_window) = actions_popup_ns_window(window) {
                    // Instrumentation only (Oracle `glass-entry-feel-options`
                    // WP0). The configure-then-attach ORDER below is load
                    // bearing product behavior and is deliberately unchanged:
                    // the popup's entry morph is armed while `parentWindow` is
                    // still nil, and the attach lands while that animation is
                    // in flight. The user likes the resulting feel, so these
                    // events exist to MEASURE that sequence, not to correct it.
                    crate::platform::host_clock::log_entry_timeline_event(
                        "actions_native_configure_started",
                    );
                    // SAFETY: `ns_window` comes from the live GPUI popup window via
                    // `actions_popup_ns_window`, so it is a valid AppKit NSWindow
                    // pointer on the main thread when configuration runs.
                    unsafe {
                        platform::configure_actions_popup_window(ns_window, is_dark_vibrancy);
                    }
                    crate::platform::host_clock::log_entry_timeline_event("actions_morph_armed");
                    attach_actions_popup_to_parent_window(cx, parent_window_handle, ns_window);
                    crate::platform::host_clock::log_entry_timeline_event(
                        "actions_parent_attached",
                    );
                } else {
                    tracing::warn!(
                        target: "script_kit::actions",
                        event = "actions_popup_missing_nswindow",
                        "Could not resolve NSWindow for actions popup configuration"
                    );
                }
            });
        });

        if let Err(error) = configure_result {
            crate::logging::log(
                "WARN",
                &format!(
                    "ACTIONS_WINDOW_OP_FAIL configure_popup_window update failed: operation=position_focus error={error:?}"
                ),
            );
            crate::logging::log_debug(
                "ACTIONS",
                &format!(
                    "ACTIONS_WINDOW_OP_FAIL configure_popup_window context: display_id={display_id:?}, position={position:?}"
                ),
            );
        }
    }

    // Store the one fixed-size popup handle globally.
    let window_storage = ACTIONS_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = window_storage.lock() {
        *guard = Some(handle);
    }

    crate::logging::log("ACTIONS", "Actions popup window opened with vibrancy");

    // Register in the automation window registry with parent identity.
    // Fail-closed: if registration fails, close the popup and propagate the error.
    //
    // `bounds` carries the lifetime-fixed placement receipt verbatim so
    // `listAutomationWindows` / `inspectAutomationWindow` surface the same
    // popup frame through every filter, route, and action-data mutation.
    let popup_automation_id = "actions-dialog".to_string();
    let popup_bounds_for_registry = crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    };
    if let Err(e) = crate::windows::register_attached_popup(
        popup_automation_id.clone(),
        crate::protocol::AutomationWindowKind::ActionsDialog,
        Some("Actions".to_string()),
        Some("actionsDialog".to_string()),
        Some(popup_bounds_for_registry),
        Some(parent_automation_id.as_str()),
    ) {
        tracing::warn!(
            target: "script_kit::actions",
            event = "actions_popup_registry_failed",
            error = %e,
            "Failed to register actions popup in automation registry — closing popup"
        );
        // Close the already-opened popup before returning the error
        close_actions_window(cx);
        return Err(e);
    }
    let popup_any_handle: AnyWindowHandle = handle.into();
    let popup_generation = crate::windows::resolve_automation_window(Some(
        &crate::protocol::AutomationWindowTarget::Id {
            id: popup_automation_id.clone(),
        },
    ))
    .ok()
    .and_then(|info| info.generation);
    crate::windows::upsert_runtime_window_handle_instance(
        &popup_automation_id,
        popup_any_handle,
        popup_generation,
    );
    crate::windows::automation_surface_collector::upsert_actions_dialog_snapshot(
        popup_automation_id.as_str(),
        &dialog_entity,
        cx,
    );
    record_actions_popup_automation_snapshot(
        &parent_automation_id,
        parent_kind,
        &receipt,
        fixed_shell_size,
        &opening_shell_basis,
    );

    // Structured receipt reports the same root/unfiltered sizing basis that
    // owns the lifetime-fixed shell.
    emit_actions_popup_event(
        ActionsPopupEvent::OpenSucceeded,
        None,
        Some(position),
        Some(opening_shell_basis.action_count),
        Some(opening_shell_basis.section_header_count),
        Some(fixed_height),
    );

    Ok(handle)
}

/// Close the actions window if it's open
pub fn close_actions_window(cx: &mut App) {
    set_actions_window_parent_kind(None);
    // Unregister from automation registry before destroying the window
    unregister_actions_dialog_automation_surfaces();

    if let Some(window_storage) = ACTIONS_WINDOW.get() {
        if let Ok(mut guard) = window_storage.lock() {
            if let Some(handle) = guard.take() {
                crate::logging::log("ACTIONS", "Closing actions popup window");
                emit_actions_popup_event(ActionsPopupEvent::Closed, None, None, None, None, None);
                // Close the window
                let close_result = handle.update(cx, |this, window, cx| {
                    this.dialog
                        .update(cx, |dialog, _cx| dialog.release_fixed_shell());
                    crate::platform::dematerialize_then_remove_gpui_window(
                        window,
                        cx,
                        "ACTIONS",
                        "Actions popup",
                    );
                });
                if let Err(error) = close_result {
                    crate::logging::log(
                        "WARN",
                        &format!(
                            "ACTIONS_WINDOW_OP_FAIL close_actions_window update failed: operation=focus_cleanup error={error:?}"
                        ),
                    );
                    crate::logging::log_debug(
                        "ACTIONS",
                        "ACTIONS_WINDOW_OP_FAIL close_actions_window context: remove_window requested",
                    );
                }
            }
        }
    }
}

/// Check if the given window handle matches the actions window.
///
/// Used by keystroke interceptors to avoid handling keys meant for the
/// actions popup (which manages its own Escape / Enter / arrows).
pub fn is_actions_window(window: &gpui::Window) -> bool {
    if let Some(window_storage) = ACTIONS_WINDOW.get() {
        if let Ok(guard) = window_storage.lock() {
            if let Some(actions_handle) = guard.as_ref() {
                let actions_any: gpui::AnyWindowHandle = (*actions_handle).into();
                return window.window_handle() == actions_any;
            }
        }
    }
    false
}

/// Check if the actions window is currently open
pub fn is_actions_window_open() -> bool {
    if let Some(window_storage) = ACTIONS_WINDOW.get() {
        if let Ok(guard) = window_storage.lock() {
            return guard.is_some();
        }
    }
    false
}

/// Route a key from a parent window's keyboard router into the detached
/// actions popup.
///
/// Hosts whose parent window can stay the key window while the popup is open
/// (the Notes-hosted Agent Chat Cmd+K actions / Cmd+P history popups, the
/// detached Agent Chat window) call this so navigation, typing, and Enter
/// drive the visible popup instead of leaking into the host surface (e.g.
/// the chat composer, where Enter silently no-ops). Returns `true` when the
/// popup consumed the key.
pub fn route_key_to_detached_actions_window(
    key: &str,
    key_char: Option<&str>,
    modifiers: &gpui::Modifiers,
    cx: &mut gpui::App,
) -> bool {
    let Some(handle) = get_actions_window_handle() else {
        return false;
    };
    handle
        .update(cx, |this, window, cx| {
            let handled = this.handle_key_event(key, key_char, modifiers, window, cx);
            if handled {
                crate::logging::log(
                    "KEY_ROUTE",
                    &format!("ACTIONS_POPUP_PARENT_ROUTED key='{key}'"),
                );
            }
            handled
        })
        .unwrap_or(false)
}

/// Activate an action exposed by the live detached Actions dialog. Direct
/// automation IDs must cross the same availability guard as Enter, click, and
/// displayed shortcuts; bypassing the dialog would execute disabled rows and
/// close the popup before their explanation can be read.
#[allow(
    dead_code,
    reason = "direct Actions automation is dispatched by the separately compiled application binary"
)]
pub(crate) fn activate_detached_actions_window_action(
    action_id: String,
    cx: &mut gpui::App,
) -> Option<super::dialog::ActionsDialogActivation> {
    let handle = get_actions_window_handle()?;
    handle
        .update(cx, |this, window, cx| {
            let activation = this
                .dialog
                .update(cx, |dialog, cx| dialog.activate_action_id(action_id, cx));
            this.handle_dialog_activation(activation.clone(), window, cx, "direct_action_id");
            activation
        })
        .ok()
}

/// Get the actions window handle if it exists
pub fn get_actions_window_handle() -> Option<WindowHandle<ActionsWindow>> {
    if let Some(window_storage) = ACTIONS_WINDOW.get() {
        if let Ok(guard) = window_storage.lock() {
            return *guard;
        }
    }
    None
}

/// Get the actions dialog entity from the actions window, if both exist.
///
/// Used by the automation surface collector to read dialog state without
/// needing `&mut App`.
pub fn get_actions_dialog_entity(cx: &gpui::App) -> Option<Entity<ActionsDialog>> {
    let handle = get_actions_window_handle()?;
    handle
        .read_with(cx, |window, _cx| window.dialog.clone())
        .ok()
}

/// Replace Actions search text through the live entity-backed input owner.
/// Returns false when the target is not hosted by the current Actions window.
pub(crate) fn set_actions_dialog_search_text(
    dialog: &Entity<ActionsDialog>,
    text: String,
    cx: &mut gpui::App,
) -> bool {
    let Some(handle) = get_actions_window_handle() else {
        return false;
    };
    handle
        .update(cx, |actions_window, window, cx| {
            if actions_window.dialog.entity_id() != dialog.entity_id() {
                return false;
            }
            dialog.update(cx, |dialog, cx| {
                dialog.set_search_text_in_window(text, window, cx);
            });
            true
        })
        .unwrap_or(false)
}

/// Notify the actions window to re-render (call after updating dialog entity)
pub fn notify_actions_window(cx: &mut App) {
    if let Some(handle) = get_actions_window_handle() {
        let notify_result = handle.update(cx, |_this, _window, cx| {
            cx.notify();
        });
        if let Err(error) = notify_result {
            crate::logging::log(
                "WARN",
                &format!(
                    "ACTIONS_WINDOW_OP_FAIL notify_actions_window update failed: operation=focus_refresh error={error:?}"
                ),
            );
            crate::logging::log_debug(
                "ACTIONS",
                "ACTIONS_WINDOW_OP_FAIL notify_actions_window context: cx.notify() skipped",
            );
        }
    }
}

// --- merged from part_03.rs ---

#[allow(dead_code)] // Protected calibration sentinel; content resizing is intentionally gone.
const ACTIONS_WINDOW_RESIZE_ANIMATE: bool = false;

#[cfg(test)]
mod resize_instant_tests {
    use super::ACTIONS_WINDOW_RESIZE_ANIMATE;

    #[test]
    fn test_actions_window_resize_animation_flag_is_disabled() {
        let flag = ACTIONS_WINDOW_RESIZE_ANIMATE;
        assert!(
            !flag,
            "Actions window resize must stay instant with animation disabled"
        );
    }
}

#[cfg(test)]
mod request_close_ordering_tests {
    use std::fs;

    #[test]
    fn test_request_close_activates_main_window_before_on_close_callback() {
        let source = fs::read_to_string("src/actions/window.rs")
            .expect("Failed to read src/actions/window.rs");

        let start = source
            .find("fn request_close")
            .expect("Expected request_close function in src/actions/window.rs");
        let end = source[start..]
            .find("Self::defer_close")
            .map(|idx| start + idx)
            .expect("Expected defer_close call in request_close");
        let body = &source[start..end];

        let activate_idx = body
            .find("platform::activate_main_window")
            .expect("Expected activate_main_window call in request_close");
        let on_close_idx = body
            .find("on_close(cx)")
            .expect("Expected on_close(cx) invocation in request_close");

        assert!(
            activate_idx < on_close_idx,
            "request_close must activate the main window BEFORE scheduling focus restoration \
             via on_close callback. macOS window activation is async — starting it earlier \
             gives the OS time to make the main window key before the deferred callback runs."
        );
    }

    #[test]
    fn test_is_actions_window_function_exists() {
        let source = fs::read_to_string("src/actions/window.rs")
            .expect("Failed to read src/actions/window.rs");

        assert!(
            source.contains("pub fn is_actions_window(window: &gpui::Window) -> bool"),
            "window.rs must export is_actions_window(window) for keystroke interceptor guards"
        );
    }
}

#[cfg(test)]
mod actions_popup_origin_tests {
    use super::*;
    use gpui::{px, Bounds, Point, Size};

    #[test]
    fn top_center_centers_inside_main_window() {
        let origin = actions_popup_origin(
            Bounds {
                origin: Point {
                    x: px(100.0),
                    y: px(50.0),
                },
                size: Size {
                    width: px(480.0),
                    height: px(300.0),
                },
            },
            px(ACTIONS_WINDOW_WIDTH),
            px(220.0),
            WindowPosition::TopCenter,
        );

        assert_eq!(
            f32::from(origin.x),
            100.0 + ((480.0 - ACTIONS_WINDOW_WIDTH) / 2.0),
            "TopCenter must center horizontally within the parent window"
        );
        assert_eq!(
            f32::from(origin.y),
            50.0 + TITLEBAR_HEIGHT + ACTIONS_MARGIN_Y,
            "TopCenter must anchor below the titlebar"
        );
    }

    #[test]
    fn bottom_right_stays_above_footer() {
        let origin = actions_popup_origin(
            Bounds {
                origin: Point {
                    x: px(20.0),
                    y: px(40.0),
                },
                size: Size {
                    width: px(750.0),
                    height: px(500.0),
                },
            },
            px(ACTIONS_WINDOW_WIDTH),
            px(180.0),
            WindowPosition::BottomRight,
        );

        assert_eq!(
            f32::from(origin.x),
            20.0 + 750.0 - ACTIONS_WINDOW_WIDTH - ACTIONS_MARGIN_X,
            "BottomRight must right-align with margin"
        );
        assert_eq!(
            f32::from(origin.y),
            40.0 + 500.0 - 180.0 - FOOTER_HEIGHT - ACTIONS_MARGIN_Y,
            "BottomRight must sit above the footer"
        );
    }

    #[test]
    fn top_right_right_aligns_below_titlebar() {
        let origin = actions_popup_origin(
            Bounds {
                origin: Point {
                    x: px(0.0),
                    y: px(0.0),
                },
                size: Size {
                    width: px(600.0),
                    height: px(400.0),
                },
            },
            px(ACTIONS_WINDOW_WIDTH),
            px(200.0),
            WindowPosition::TopRight,
        );

        assert_eq!(
            f32::from(origin.x),
            600.0 - ACTIONS_WINDOW_WIDTH - ACTIONS_MARGIN_X,
            "TopRight must right-align with margin"
        );
        assert_eq!(
            f32::from(origin.y),
            TITLEBAR_HEIGHT + ACTIONS_MARGIN_Y,
            "TopRight must anchor below the titlebar"
        );
    }

    #[test]
    fn open_actions_window_uses_placement_receipt_helper() {
        let source = std::fs::read_to_string("src/actions/window.rs")
            .expect("Failed to read src/actions/window.rs");

        let fn_start = source
            .find("pub fn open_actions_window(")
            .expect("open_actions_window not found");
        let fn_body = &source[fn_start..];

        assert!(
            fn_body.contains("actions_popup_placement_receipt("),
            "open_actions_window must delegate to actions_popup_placement_receipt helper"
        );
    }

    #[test]
    fn open_actions_window_attaches_popup_as_native_child_window() {
        let source = std::fs::read_to_string("src/actions/window.rs")
            .expect("Failed to read src/actions/window.rs");

        let fn_start = source
            .find("pub fn open_actions_window(")
            .expect("open_actions_window not found");
        let fn_body = &source[fn_start..];

        assert!(
            fn_body.contains("parent_window_handle: AnyWindowHandle"),
            "open_actions_window should accept the parent window handle"
        );
        assert!(
            fn_body.contains("attach_actions_popup_to_parent_window("),
            "open_actions_window should attach the popup to its parent window after configuration"
        );
        assert!(
            source.contains("addChildWindow:child_ns_window ordered:NS_WINDOW_ABOVE"),
            "actions popup child attachment should use AppKit addChildWindow ordering"
        );
    }

    #[test]
    fn open_actions_window_registers_protocol_runtime_handle() {
        let source = std::fs::read_to_string("src/actions/window.rs")
            .expect("Failed to read src/actions/window.rs");

        let fn_start = source
            .find("pub fn open_actions_window(")
            .expect("open_actions_window not found");
        let close_start = source
            .find("pub fn close_actions_window(")
            .expect("close_actions_window not found");
        let open_body = &source[fn_start..close_start];
        let close_body = &source[close_start..];

        assert!(
            open_body.contains("let popup_any_handle: AnyWindowHandle = handle.into();"),
            "open_actions_window must convert the popup handle to an AnyWindowHandle"
        );
        assert!(
            open_body.contains(
                "crate::windows::upsert_runtime_window_handle(&popup_automation_id, popup_any_handle)"
            ),
            "open_actions_window must register the actions-dialog runtime handle for simulateGpuiEvent"
        );
        // The runtime-handle removal moved into the shared
        // unregister_actions_dialog_automation_surfaces() helper; follow the
        // indirection so the invariant (closing removes the handle) still holds.
        assert!(
            close_body.contains("unregister_actions_dialog_automation_surfaces()"),
            "close_actions_window must unregister the actions-dialog automation surfaces"
        );
        let helper_start = source
            .find("fn unregister_actions_dialog_automation_surfaces()")
            .expect("unregister_actions_dialog_automation_surfaces not found");
        let helper_body = &source[helper_start..];
        let helper_end = helper_body
            .find("\n}")
            .map(|i| i + 2)
            .unwrap_or(helper_body.len());
        let helper_body = &helper_body[..helper_end];
        assert!(
            helper_body
                .contains("crate::windows::remove_runtime_window_handle(\"actions-dialog\")"),
            "unregister_actions_dialog_automation_surfaces must remove the actions-dialog runtime handle"
        );
    }
}

#[cfg(test)]
mod actions_popup_geometry_tests {
    use super::*;
    use gpui::{px, Bounds, Point, Size};

    fn test_main_window_bounds() -> Bounds<Pixels> {
        Bounds {
            origin: Point {
                x: px(100.0),
                y: px(50.0),
            },
            size: Size {
                width: px(480.0),
                height: px(220.0),
            },
        }
    }

    #[test]
    fn top_center_bounds_are_centered_below_titlebar() {
        let bounds = actions_popup_bounds(
            test_main_window_bounds(),
            px(320.0),
            px(180.0),
            WindowPosition::TopCenter,
        );

        let x: f32 = bounds.origin.x.into();
        let y: f32 = bounds.origin.y.into();

        assert_eq!(x, 180.0);
        assert_eq!(y, 94.0);
    }

    #[test]
    fn bottom_right_bounds_are_right_aligned_above_footer() {
        let bounds = actions_popup_bounds(
            test_main_window_bounds(),
            px(320.0),
            px(180.0),
            WindowPosition::BottomRight,
        );

        let x: f32 = bounds.origin.x.into();
        let y: f32 = bounds.origin.y.into();

        // x = 100 + 480 - 320 - 8 = 252
        assert_eq!(x, 252.0);
        // y = 50 + 220 - 180 - FOOTER_HEIGHT(30) - ACTIONS_MARGIN_Y(8) = 52
        assert_eq!(y, 52.0);
    }

    #[test]
    fn placement_receipt_captures_correct_pinned_edge() {
        let receipt = actions_popup_placement_receipt(
            test_main_window_bounds(),
            px(320.0),
            px(180.0),
            WindowPosition::TopCenter,
            None,
        );
        assert_eq!(receipt.pinned_edge, "top");

        let receipt = actions_popup_placement_receipt(
            test_main_window_bounds(),
            px(320.0),
            px(180.0),
            WindowPosition::BottomRight,
            None,
        );
        assert_eq!(receipt.pinned_edge, "bottom");
    }

    #[test]
    fn actions_popup_bounds_size_matches_inputs() {
        let bounds = actions_popup_bounds(
            test_main_window_bounds(),
            px(320.0),
            px(180.0),
            WindowPosition::TopCenter,
        );

        let w: f32 = bounds.size.width.into();
        let h: f32 = bounds.size.height.into();
        assert_eq!(w, 320.0);
        assert_eq!(h, 180.0);
    }
}

// Confirm popup window — a native GPUI WindowKind::PopUp window with macOS
// vibrancy blur. Replaces the old in-window overlay dialog approach so the
// confirmation surface gets real NSPanel blur instead of plain transparency.

use std::{
    rc::Rc,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use gpui::{
    div, prelude::*, px, AnyElement, AnyWindowHandle, App, Bounds, Context, DisplayId, FocusHandle,
    Focusable, Pixels, Point, Render, SharedString, Size, Task, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
};
use gpui_component::button::ButtonVariant as ConfirmButtonVariant;

use crate::{
    components::confirm_modal_shell::{
        confirm_modal_header, confirm_modal_number_override, confirm_modal_shell, modal_action_row,
        ConfirmModalShellConfig, ModalActionRowButton, CONFIRM_MODAL_RADIUS, MODAL_WIDTH_PX,
    },
    components::footer_chrome::{
        current_main_menu_footer_height, current_main_menu_footer_metrics,
        footer_action_slot_width, footer_button_height, footer_centered_action_button_layout,
        footer_centered_action_edge_padding_x, FooterActionSlot, FooterHintButtonLayoutOverrides,
    },
    components::overlay_modal::MODAL_PADDING,
    dev_style_tool::{
        ConfirmModalKnobId, CONFIRM_MODAL_ACTIONS_BUTTON_HEIGHT_KNOB_ID,
        CONFIRM_MODAL_ACTIONS_BUTTON_RADIUS_KNOB_ID,
        CONFIRM_MODAL_ACTIONS_CANCEL_SLOT_WIDTH_KNOB_ID,
        CONFIRM_MODAL_ACTIONS_CONFIRM_SLOT_WIDTH_KNOB_ID,
        CONFIRM_MODAL_ACTIONS_CONTENT_GAP_KNOB_ID, CONFIRM_MODAL_ACTIONS_EDGE_PADDING_X_KNOB_ID,
        CONFIRM_MODAL_ACTIONS_GAP_KNOB_ID, CONFIRM_MODAL_ACTIONS_PADDING_X_KNOB_ID,
        CONFIRM_MODAL_ACTIONS_PADDING_Y_KNOB_ID, CONFIRM_MODAL_ANATOMY_BODY_ACTIONS_GAP_KNOB_ID,
        CONFIRM_MODAL_ANATOMY_BODY_LINE_HEIGHT_KNOB_ID,
        CONFIRM_MODAL_ANATOMY_HEADER_BODY_GAP_KNOB_ID,
    },
    platform,
    theme::get_cached_theme,
    ui_foundation::{is_key_enter, is_key_escape, is_key_left, is_key_tab},
};

const CONFIRM_PADDING_X: f32 = MODAL_PADDING;
const CONFIRM_PADDING_Y: f32 = 20.0;
const CONFIRM_SECTION_GAP: f32 = 10.0;
const CONFIRM_TITLE_LINE_HEIGHT: f32 = 16.0;
const CONFIRM_MIN_HEIGHT: f32 = 132.0;
const CONFIRM_MAX_HEIGHT: f32 = 240.0;
const CONFIRM_BODY_MAX_LINES: usize = 5;
/// The body renders with `.text_xs()` — 0.75rem at the default 16px rem.
const CONFIRM_BODY_FONT_SIZE: f32 = 12.0;
const CONFIRM_LIFECYCLE_POLL_MS: u64 = 120;
/// NSWindowOrderingMode::NSWindowAbove — place child above parent.
const NS_WINDOW_ABOVE: i64 = 1;

static CONFIRM_WINDOW: OnceLock<Mutex<Option<WindowHandle<ConfirmPopupWindow>>>> = OnceLock::new();
static CONFIRM_PARENT_WINDOW: OnceLock<Mutex<Option<AnyWindowHandle>>> = OnceLock::new();
static CONFIRM_RESULT_TX: OnceLock<Mutex<Option<async_channel::Sender<ParentDialogResult>>>> =
    OnceLock::new();
static CONFIRM_FOCUSED_BUTTON: OnceLock<Mutex<FocusedButton>> = OnceLock::new();
static CONFIRM_HAS_SECONDARY: OnceLock<Mutex<bool>> = OnceLock::new();

const CONFIRM_POPUP_AUTOMATION_ID: &str = "confirm-popup";

fn unregister_confirm_popup_automation_window(reason: &'static str) {
    tracing::info!(
        target: "script_kit::confirm",
        event = "confirm_popup_registry_remove",
        reason
    );
    crate::windows::remove_automation_window(CONFIRM_POPUP_AUTOMATION_ID);
}

#[derive(Clone)]
pub(crate) struct ConfirmWindowOptions {
    pub title: SharedString,
    pub body: SharedString,
    pub confirm_text: SharedString,
    pub cancel_text: SharedString,
    pub secondary_text: Option<SharedString>,
    pub confirm_variant: ConfirmButtonVariant,
    pub width: Pixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParentDialogResult {
    Primary,
    Secondary,
    Dismiss,
    ProgrammaticClose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedButton {
    Cancel,
    Secondary,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmWindowKeyIntent {
    FocusNext,
    FocusPrev,
    ActivateFocused,
    ActivateSecondary,
    Cancel,
}

#[inline]
fn is_key_right(key: &str) -> bool {
    key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright")
}

#[inline]
fn confirm_window_key_intent(
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<ConfirmWindowKeyIntent> {
    if key.eq_ignore_ascii_case("i") && modifiers.platform && !modifiers.control && !modifiers.alt {
        return Some(ConfirmWindowKeyIntent::ActivateSecondary);
    }
    if is_key_escape(key) {
        return Some(ConfirmWindowKeyIntent::Cancel);
    }
    if is_key_enter(key) {
        return Some(ConfirmWindowKeyIntent::ActivateFocused);
    }
    if is_key_tab(key) {
        return Some(if modifiers.shift {
            ConfirmWindowKeyIntent::FocusPrev
        } else {
            ConfirmWindowKeyIntent::FocusNext
        });
    }
    if is_key_left(key) {
        return Some(ConfirmWindowKeyIntent::FocusPrev);
    }
    if is_key_right(key) {
        return Some(ConfirmWindowKeyIntent::FocusNext);
    }
    None
}

/// Wrapped line count of `body` at the modal's real body font and width,
/// using the text system's line wrapper. Replaces the old `width / 7.4`
/// chars-per-line guess, which drifted whenever the body wrapped differently
/// than the estimate (clipped text or excess bottom padding).
fn confirm_body_wrapped_lines(body: &str, content_width: f32, cx: &App) -> usize {
    let mut wrapper = cx.text_system().line_wrapper(
        gpui::font(crate::list_item::FONT_SYSTEM_UI),
        px(CONFIRM_BODY_FONT_SIZE),
    );
    body.lines()
        .map(|line| {
            wrapper
                .wrap_line(&[gpui::LineFragment::text(line)], px(content_width))
                .count()
                + 1
        })
        .sum::<usize>()
        .max(1)
}

/// Pure clamp step shared by the measured path and the unit tests, so the
/// sizing contract is testable without a live text system.
fn confirm_window_height_from_body_lines(has_body: bool, body_lines: usize) -> f32 {
    let body_lines = if has_body {
        body_lines.clamp(1, CONFIRM_BODY_MAX_LINES)
    } else {
        0
    };
    let body_height = body_lines as f32 * confirm_body_line_height();
    let gaps = confirm_modal_stack_gaps(has_body);

    (confirm_shell_padding_y() * 2.0
        + CONFIRM_TITLE_LINE_HEIGHT
        + gaps.after_header_px
        + body_height
        + gaps.after_body_px.unwrap_or(0.0)
        + confirm_action_button_height())
    .clamp(CONFIRM_MIN_HEIGHT, CONFIRM_MAX_HEIGHT)
}

fn confirm_window_dynamic_height(width: Pixels, body: &str, cx: &App) -> f32 {
    let width_px: f32 = width.into();
    let content_width = (width_px - (confirm_shell_padding_x() * 2.0)).max(160.0);

    let has_body = !body.trim().is_empty();
    let body_lines = if has_body {
        confirm_body_wrapped_lines(body, content_width, cx)
    } else {
        0
    };
    confirm_window_height_from_body_lines(has_body, body_lines)
}

fn confirm_modal_number(id: ConfirmModalKnobId, fallback: f32) -> f32 {
    confirm_modal_number_override(id, fallback)
}

fn confirm_shell_padding_x() -> f32 {
    confirm_modal_number(
        crate::dev_style_tool::CONFIRM_MODAL_PADDING_X_KNOB_ID,
        CONFIRM_PADDING_X,
    )
}

fn confirm_shell_padding_y() -> f32 {
    confirm_modal_number(
        crate::dev_style_tool::CONFIRM_MODAL_PADDING_Y_KNOB_ID,
        CONFIRM_PADDING_Y,
    )
}

fn confirm_shell_gap() -> f32 {
    confirm_modal_number(
        crate::dev_style_tool::CONFIRM_MODAL_GAP_KNOB_ID,
        CONFIRM_SECTION_GAP,
    )
}

fn confirm_action_button_height() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ACTIONS_BUTTON_HEIGHT_KNOB_ID,
        footer_button_height(current_main_menu_footer_height()),
    )
}

fn confirm_action_button_gap() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ACTIONS_GAP_KNOB_ID,
        current_main_menu_footer_metrics().item_gap_px,
    )
}

fn confirm_cancel_slot_width() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ACTIONS_CANCEL_SLOT_WIDTH_KNOB_ID,
        footer_action_slot_width(FooterActionSlot::Close),
    )
}

fn confirm_confirm_slot_width() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ACTIONS_CONFIRM_SLOT_WIDTH_KNOB_ID,
        footer_action_slot_width(FooterActionSlot::Run),
    )
}

fn confirm_action_button_radius() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ACTIONS_BUTTON_RADIUS_KNOB_ID,
        current_main_menu_footer_metrics().button_radius,
    )
}

fn confirm_action_button_layout() -> FooterHintButtonLayoutOverrides {
    let footer_layout = footer_centered_action_button_layout();
    let metrics = current_main_menu_footer_metrics();
    FooterHintButtonLayoutOverrides {
        button_padding_x_px: Some(confirm_modal_number(
            CONFIRM_MODAL_ACTIONS_PADDING_X_KNOB_ID,
            footer_layout
                .button_padding_x_px
                .unwrap_or(metrics.button_padding_x),
        )),
        button_padding_y_px: Some(confirm_modal_number(
            CONFIRM_MODAL_ACTIONS_PADDING_Y_KNOB_ID,
            footer_layout
                .button_padding_y_px
                .unwrap_or(metrics.button_padding_y),
        )),
        content_gap_px: Some(confirm_modal_number(
            CONFIRM_MODAL_ACTIONS_CONTENT_GAP_KNOB_ID,
            footer_layout.content_gap_px.unwrap_or(metrics.content_gap),
        )),
        button_radius_px: Some(confirm_action_button_radius()),
        edge_padding_x_px: Some(confirm_modal_number(
            CONFIRM_MODAL_ACTIONS_EDGE_PADDING_X_KNOB_ID,
            footer_layout
                .edge_padding_x_px
                .unwrap_or_else(footer_centered_action_edge_padding_x),
        )),
        // Confirm/cancel labels are caller-supplied ("Move to Trash", custom
        // cancel copy) — a fixed footer slot ellipsizes them. Hug the rendered
        // content like render_universal_footer_action_buttons while keeping
        // the shared footer metrics above.
        shrink_frame_to_content_px: true,
        hug_frame_to_content: true,
    }
}

fn confirm_anatomy_header_body_gap() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ANATOMY_HEADER_BODY_GAP_KNOB_ID,
        confirm_shell_gap(),
    )
}

fn confirm_anatomy_body_actions_gap() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ANATOMY_BODY_ACTIONS_GAP_KNOB_ID,
        confirm_shell_gap(),
    )
}

fn confirm_body_line_height() -> f32 {
    confirm_modal_number(
        CONFIRM_MODAL_ANATOMY_BODY_LINE_HEIGHT_KNOB_ID,
        crate::dev_style_tool::CONFIRM_MODAL_DEFAULT_BODY_LINE_HEIGHT,
    )
}

#[derive(Clone, Copy, Debug)]
struct ConfirmModalStackGaps {
    after_header_px: f32,
    after_body_px: Option<f32>,
}

fn confirm_modal_stack_gaps(has_body: bool) -> ConfirmModalStackGaps {
    if has_body {
        ConfirmModalStackGaps {
            after_header_px: confirm_anatomy_header_body_gap(),
            after_body_px: Some(confirm_anatomy_body_actions_gap()),
        }
    } else {
        ConfirmModalStackGaps {
            after_header_px: confirm_anatomy_header_body_gap(),
            after_body_px: None,
        }
    }
}

fn confirm_modal_spacer(id: &'static str, height_px: f32) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .h(px(height_px.max(0.0)))
        .flex_none()
        .into_any_element()
}

fn confirm_window_bounds(
    parent_bounds: Bounds<Pixels>,
    width: Pixels,
    body: &str,
    cx: &App,
) -> Bounds<Pixels> {
    let requested_width = width.min(px(MODAL_WIDTH_PX));
    let actual_width = requested_width.min(parent_bounds.size.width);
    let dynamic_height = confirm_window_dynamic_height(actual_width, body, cx);
    confirm_window_bounds_from_height(parent_bounds, actual_width, dynamic_height)
}

/// Pure centering step: place a `width` × `height` popup centered over the
/// parent, clamping height to the parent. Split from the measuring wrapper so
/// tests can exercise the placement contract without a live text system.
fn confirm_window_bounds_from_height(
    parent_bounds: Bounds<Pixels>,
    actual_width: Pixels,
    dynamic_height: f32,
) -> Bounds<Pixels> {
    let height = px(dynamic_height).min(parent_bounds.size.height);

    let x = parent_bounds.origin.x + ((parent_bounds.size.width - actual_width) / 2.0);
    let y = parent_bounds.origin.y + ((parent_bounds.size.height - height) / 2.0);

    Bounds {
        origin: Point { x, y },
        size: Size {
            width: actual_width,
            height,
        },
    }
}

fn clear_confirm_window_handle() {
    if let Some(storage) = CONFIRM_WINDOW.get() {
        if let Ok(mut guard) = storage.lock() {
            *guard = None;
        }
    }
    // Also clear shared state
    if let Some(storage) = CONFIRM_RESULT_TX.get() {
        if let Ok(mut guard) = storage.lock() {
            *guard = None;
        }
    }
}

/// Route a key event to the confirm popup window if it's open.
/// Returns true if the key was handled (confirm popup consumed it).
/// Called from the main window's key handler chain.
#[allow(dead_code)]
pub(crate) fn consume_main_window_key_while_confirm_open(
    key: &str,
    modifiers: &gpui::Modifiers,
    cx: &mut App,
) -> bool {
    if !is_confirm_window_open() {
        return false;
    }

    let intent = confirm_window_key_intent(key, modifiers);

    tracing::info!(
        target: "script_kit::confirm",
        event = "route_key_to_confirm_popup",
        key,
        shift = modifiers.shift,
        platform = modifiers.platform,
        alt = modifiers.alt,
        control = modifiers.control,
        intent = ?intent,
        "Main window routing key to confirm popup"
    );

    match intent {
        Some(ConfirmWindowKeyIntent::Cancel) => {
            tracing::info!(
                target: "script_kit::confirm",
                event = "route_key_confirm_cancel",
                "Routing Escape to confirm popup → cancel"
            );
            resolve_confirm_window_from_parent(ParentDialogResult::Dismiss, cx);
            true
        }
        Some(ConfirmWindowKeyIntent::ActivateFocused) => {
            let focused = get_confirm_focused_button();
            let result = match focused {
                FocusedButton::Confirm => ParentDialogResult::Primary,
                FocusedButton::Secondary => ParentDialogResult::Secondary,
                FocusedButton::Cancel => ParentDialogResult::Dismiss,
            };
            tracing::info!(
                target: "script_kit::confirm",
                event = "route_key_confirm_enter",
                result = ?result,
                focused_button = ?focused,
                "Routing Enter to confirm popup → activate focused"
            );
            resolve_confirm_window_from_parent(result, cx);
            true
        }
        Some(ConfirmWindowKeyIntent::ActivateSecondary) => {
            if CONFIRM_HAS_SECONDARY
                .get()
                .and_then(|state| state.lock().ok())
                .is_some_and(|state| *state)
            {
                resolve_confirm_window_from_parent(ParentDialogResult::Secondary, cx);
            }
            true
        }
        Some(ConfirmWindowKeyIntent::FocusNext) => {
            cycle_confirm_focused_button(false);
            // Notify the confirm window to re-render with updated focus
            notify_confirm_window(cx);
            true
        }
        Some(ConfirmWindowKeyIntent::FocusPrev) => {
            cycle_confirm_focused_button(true);
            notify_confirm_window(cx);
            true
        }
        None => {
            tracing::debug!(
                target: "script_kit::confirm",
                event = "route_key_confirm_consume_unhandled",
                key,
                "Confirm popup is open — consuming unhandled key"
            );
            true
        }
    }
}

#[allow(dead_code)]
pub(crate) fn route_key_to_confirm_popup(key: &str, cx: &mut App) -> bool {
    consume_main_window_key_while_confirm_open(key, &gpui::Modifiers::default(), cx)
}

pub(crate) fn send_confirm_result(confirmed: bool) {
    send_parent_dialog_result(if confirmed {
        ParentDialogResult::Primary
    } else {
        ParentDialogResult::Dismiss
    });
}

pub(crate) fn send_parent_dialog_result(result: ParentDialogResult) {
    if let Some(storage) = CONFIRM_RESULT_TX.get() {
        if let Ok(mut guard) = storage.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.try_send(result);
            }
        }
    }
}

/// Close an action dialog after its owner has completed without treating the
/// close as a user dismissal. This resolves the parent task but invokes none
/// of the primary, secondary, or dismiss callbacks.
pub(crate) fn close_parent_action_dialog_programmatically(cx: &mut App) {
    send_parent_dialog_result(ParentDialogResult::ProgrammaticClose);
    close_confirm_window(cx);
}

/// Select and activate a confirm dialog button by value for batch automation.
///
/// Accepts `"confirm"`, `"secondary"`, or `"cancel"` as the value. Sends the
/// result and closes the dialog. Returns `Some(value)` on success, `None` if
/// the value is invalid or no confirm dialog is open.
#[allow(dead_code)]
pub(crate) fn batch_select_confirm_button_by_value(value: &str, cx: &mut App) -> Option<String> {
    let result = match value {
        "confirm" => ParentDialogResult::Primary,
        "secondary" => {
            let has_secondary = CONFIRM_HAS_SECONDARY
                .get()
                .and_then(|state| state.lock().ok())
                .is_some_and(|state| *state);
            if !has_secondary {
                return None;
            }
            ParentDialogResult::Secondary
        }
        "cancel" => ParentDialogResult::Dismiss,
        _ => return None,
    };
    if !is_confirm_window_open() {
        return None;
    }
    send_parent_dialog_result(result);
    close_confirm_window(cx);
    Some(value.to_string())
}

/// Select and activate a confirm dialog button by semantic ID. Three-action
/// dialogs insert `button:1:secondary` and move cancel to `button:2:cancel`;
/// legacy two-action dialogs retain `button:1:cancel`.
#[allow(dead_code)]
pub(crate) fn batch_select_confirm_button_by_semantic_id(
    semantic_id: &str,
    cx: &mut App,
) -> Option<String> {
    let value = match semantic_id {
        "button:0:confirm" => "confirm",
        "button:1:secondary" => "secondary",
        "button:1:cancel" | "button:2:cancel" => "cancel",
        _ => return None,
    };
    batch_select_confirm_button_by_value(value, cx)?;
    Some(semantic_id.to_string())
}

fn get_confirm_focused_button() -> FocusedButton {
    CONFIRM_FOCUSED_BUTTON
        .get()
        .and_then(|s| s.lock().ok())
        .map_or(FocusedButton::Confirm, |g| *g)
}

fn cycle_confirm_focused_button(reverse: bool) {
    let has_secondary = CONFIRM_HAS_SECONDARY
        .get()
        .and_then(|state| state.lock().ok())
        .is_some_and(|state| *state);
    let next = next_confirm_focused_button(get_confirm_focused_button(), reverse, has_secondary);
    set_confirm_focused_button(next);
}

fn next_confirm_focused_button(
    current: FocusedButton,
    reverse: bool,
    has_secondary: bool,
) -> FocusedButton {
    match (current, reverse, has_secondary) {
        (FocusedButton::Confirm, false, true) => FocusedButton::Secondary,
        (FocusedButton::Secondary, false, true) => FocusedButton::Cancel,
        (FocusedButton::Cancel, false, _) => FocusedButton::Confirm,
        (FocusedButton::Confirm, true, _) => FocusedButton::Cancel,
        (FocusedButton::Cancel, true, true) => FocusedButton::Secondary,
        (FocusedButton::Secondary, true, true) => FocusedButton::Confirm,
        (FocusedButton::Confirm, _, false) => FocusedButton::Cancel,
        (FocusedButton::Cancel, _, false) | (FocusedButton::Secondary, _, false) => {
            FocusedButton::Confirm
        }
    }
}

fn set_confirm_focused_button(next: FocusedButton) {
    if let Some(storage) = CONFIRM_FOCUSED_BUTTON.get() {
        if let Ok(mut guard) = storage.lock() {
            *guard = next;
        }
    }
}

/// Defer closing the confirm window to the next frame so that
/// `is_confirm_window_open()` remains true for the rest of the current
/// event processing cycle. This prevents PressEnter and other handlers
/// from also processing the same Enter keystroke.
fn resolve_confirm_window_from_parent(result: ParentDialogResult, cx: &mut App) {
    if let Some(storage) = CONFIRM_WINDOW.get() {
        if let Ok(guard) = storage.lock() {
            if let Some(handle) = guard.as_ref() {
                let _ = handle.update(cx, |confirm, window, cx| {
                    confirm.resolve_and_close(result, window, cx);
                });
            }
        }
    }
}

fn notify_confirm_window(cx: &mut App) {
    if let Some(storage) = CONFIRM_WINDOW.get() {
        if let Ok(guard) = storage.lock() {
            if let Some(handle) = guard.as_ref() {
                let _ = handle.update(cx, |_root, _window, cx| {
                    cx.notify();
                });
            }
        }
    }
}

fn notify_confirm_parent_window(cx: &mut App) {
    if let Some(storage) = CONFIRM_PARENT_WINDOW.get() {
        if let Ok(guard) = storage.lock() {
            if let Some(handle) = guard.as_ref() {
                let _ = handle.update(cx, |root, _window, cx| {
                    cx.notify(root.entity_id());
                });
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn refresh_confirm_popup_for_runtime_style(cx: &mut App) {
    notify_confirm_window(cx);
    notify_confirm_parent_window(cx);
}

#[allow(dead_code)]
/// Snapshot of the confirm popup's semantic state for automation.
#[derive(Debug, Clone)]
pub(crate) struct ConfirmPopupSnapshot {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) confirm_text: String,
    pub(crate) cancel_text: String,
    pub(crate) secondary_text: Option<String>,
    pub(crate) focused_button: &'static str,
}

/// Read the confirm popup snapshot if the popup window is open.
///
/// Used by the automation surface collector to extract semantic elements
/// from the live popup state without needing `&mut App`.
pub(crate) fn get_confirm_popup_snapshot(cx: &gpui::App) -> Option<ConfirmPopupSnapshot> {
    let storage = CONFIRM_WINDOW.get()?;
    let guard = storage.lock().ok()?;
    let handle = (*guard)?;
    handle
        .read_with(cx, |popup, _cx| {
            let focused_button = match popup.focused_button {
                FocusedButton::Confirm => "confirm",
                FocusedButton::Secondary => "secondary",
                FocusedButton::Cancel => "cancel",
            };
            ConfirmPopupSnapshot {
                title: popup.title.to_string(),
                body: popup.body.to_string(),
                confirm_text: popup.confirm_text.to_string(),
                cancel_text: popup.cancel_text.to_string(),
                secondary_text: popup.secondary_text.as_ref().map(ToString::to_string),
                focused_button,
            }
        })
        .ok()
}

pub(crate) fn is_confirm_window_open() -> bool {
    CONFIRM_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .is_some_and(|guard| guard.is_some())
}

pub(crate) fn close_confirm_window(cx: &mut App) {
    // Unregister from automation registry before destroying the window
    unregister_confirm_popup_automation_window("close_confirm_window");

    tracing::info!(
        target: "script_kit::confirm",
        event = "close_confirm_window_called",
        "close_confirm_window called"
    );
    if let Some(storage) = CONFIRM_WINDOW.get() {
        if let Ok(mut guard) = storage.lock() {
            if let Some(handle) = guard.take() {
                tracing::info!(
                    target: "script_kit::confirm",
                    event = "close_confirm_window_removing",
                    "close_confirm_window: removing window"
                );
                // remove_window() destroys the NSWindow, which causes AppKit
                // to automatically detach it from its parent (addChildWindow
                // relationship). No manual removeChildWindow: needed.
                let _ = handle.update(cx, |_root, window, _cx| {
                    window.remove_window();
                });
            } else {
                tracing::debug!(
                    target: "script_kit::confirm",
                    event = "close_confirm_window_no_handle",
                    "close_confirm_window: no handle stored"
                );
            }
        }
    }
    notify_confirm_parent_window(cx);
    if let Some(storage) = CONFIRM_PARENT_WINDOW.get() {
        if let Ok(mut guard) = storage.lock() {
            *guard = None;
        }
    }
}

pub(crate) struct ConfirmPopupParentWindow {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) display_id: Option<DisplayId>,
    pub(crate) automation_id: Option<String>,
}

/// Read an NSWindow's `title` as a Rust String, or `None` if the title is nil
/// or not valid UTF-8. Safe to call on the AppKit main thread inside the
/// confirm-popup defer block where we already hold raw NSWindow pointers.
#[cfg(target_os = "macos")]
unsafe fn nswindow_title_string(window: cocoa::base::id) -> Option<String> {
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;
    if window == nil {
        return None;
    }
    let title: cocoa::base::id = unsafe { msg_send![window, title] };
    if title == nil {
        return None;
    }
    let title_cstr: *const std::os::raw::c_char = unsafe { msg_send![title, UTF8String] };
    if title_cstr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(title_cstr) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn automation_bounds_from_gpui(bounds: Bounds<Pixels>) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

pub(crate) fn open_confirm_popup_window(
    cx: &mut App,
    parent_window: ConfirmPopupParentWindow,
    options: ConfirmWindowOptions,
    keep_open_while: Rc<dyn Fn() -> bool>,
    result_tx: async_channel::Sender<ParentDialogResult>,
) -> anyhow::Result<WindowHandle<ConfirmPopupWindow>> {
    let parent_automation_id = resolve_confirm_popup_parent_automation_id(
        parent_window.handle,
        parent_window.bounds,
        parent_window.automation_id.as_deref(),
        options.title.as_ref(),
    )?;

    tracing::info!(
        target: "script_kit::confirm",
        event = "open_confirm_popup_window",
        title = %options.title,
        parent_x = ?parent_window.bounds.origin.x,
        parent_y = ?parent_window.bounds.origin.y,
        parent_w = ?parent_window.bounds.size.width,
        parent_h = ?parent_window.bounds.size.height,
        display_id = ?parent_window.display_id,
        "open_confirm_popup_window: opening native confirm popup"
    );
    close_confirm_window(cx);
    let has_secondary = options.secondary_text.is_some();
    if let Ok(mut state) = CONFIRM_HAS_SECONDARY
        .get_or_init(|| Mutex::new(false))
        .lock()
    {
        *state = has_secondary;
    }

    let theme = get_cached_theme();
    let is_dark_vibrancy = theme.should_use_dark_vibrancy();
    let vibrancy_enabled = theme.is_vibrancy_enabled();
    let window_background = if vibrancy_enabled {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    };

    let bounds = confirm_window_bounds(
        parent_window.bounds,
        options.width,
        options.body.as_ref(),
        cx,
    );

    tracing::info!(
        target: "script_kit::confirm",
        event = "open_confirm_popup_window_bounds",
        x = ?bounds.origin.x,
        y = ?bounds.origin.y,
        width = ?bounds.size.width,
        height = ?bounds.size.height,
        vibrancy_enabled,
        is_dark_vibrancy,
        "open_confirm_popup_window: calculated bounds"
    );

    let request = options.clone();
    let lifecycle = keep_open_while.clone();
    let sender = result_tx.clone();

    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_background,
            // Keep the popup from becoming key before AppKit attaches it as a child.
            // Main-window confirm routing owns live Enter/Tab/Escape handling while
            // AppKit keeps the attached popup visually above the parent.
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            display_id: parent_window.display_id,
            ..Default::default()
        },
        move |_window, cx| cx.new(|cx| ConfirmPopupWindow::new(request, lifecycle, sender, cx)),
    )?;

    tracing::info!(
        target: "script_kit::confirm",
        event = "open_confirm_popup_window_created",
        "open_confirm_popup_window: window created successfully"
    );

    // Capture expected frame for NSWindow matching in the deferred callback
    let expected_w: f32 = bounds.size.width.into();
    let expected_h: f32 = bounds.size.height.into();
    let expected_x: f32 = bounds.origin.x.into();
    let expected_y: f32 = bounds.origin.y.into();

    // Capture intended parent identity + frame so the AppKit attach step can
    // pick the *intended* parent NSWindow deterministically, instead of
    // defaulting to whichever window happens to be `isKeyWindow` when the
    // defer block runs (which is brittle when Notes / Agent Chat / main coexist).
    let parent_automation_id_for_nswindow = parent_automation_id.clone();
    let parent_expected_w: f32 = parent_window.bounds.size.width.into();
    let parent_expected_h: f32 = parent_window.bounds.size.height.into();
    let parent_expected_title =
        crate::windows::automation_window_by_id(&parent_automation_id).and_then(|info| info.title);

    #[cfg(target_os = "macos")]
    {
        let _ = handle.update(cx, move |_root, window, cx| {
            window.defer(cx, move |_window, _cx| {
                use cocoa::appkit::NSApp;
                use cocoa::base::nil;
                use objc::{msg_send, sel, sel_impl};

                // SAFETY: On the AppKit main thread inside GPUI's deferred
                // window callback. We enumerate all NSWindows to find the
                // confirm popup by matching the expected frame bounds, rather
                // than relying on lastObject (which may return the wrong
                // window when other popups coexist).
                unsafe {
                    let app: cocoa::base::id = NSApp();
                    let windows: cocoa::base::id = msg_send![app, windows];
                    let count: usize = msg_send![windows, count];

                    tracing::info!(
                        target: "script_kit::confirm",
                        event = "configure_confirm_nswindow_search",
                        window_count = count,
                        target_x = expected_x,
                        target_y = expected_y,
                        target_w = expected_w,
                        target_h = expected_h,
                        "Searching {} NSWindows for confirm popup by frame bounds",
                        count
                    );

                    if count == 0 {
                        tracing::warn!(
                            target: "script_kit::confirm",
                            event = "configure_confirm_no_windows",
                            "No NSWindows found"
                        );
                        return;
                    }

                    // Log all windows for diagnosis
                    let mut confirm_ns_window: cocoa::base::id = nil;
                    for i in 0..count {
                        let w: cocoa::base::id = msg_send![windows, objectAtIndex: i];
                        if w == nil {
                            continue;
                        }
                        let frame: cocoa::foundation::NSRect = msg_send![w, frame];
                        let level: i64 = msg_send![w, level];
                        let is_visible: bool = msg_send![w, isVisible];
                        let is_key: bool = msg_send![w, isKeyWindow];

                        tracing::info!(
                            target: "script_kit::confirm",
                            event = "configure_confirm_nswindow_enumerate",
                            index = i,
                            ptr = format!("{:?}", w),
                            x = frame.origin.x,
                            y = frame.origin.y,
                            w = frame.size.width,
                            h = frame.size.height,
                            level,
                            is_visible,
                            is_key,
                            "NSWindow[{}]: {:?} frame=({:.0},{:.0} {:.0}x{:.0}) level={} visible={} key={}",
                            i, w, frame.origin.x, frame.origin.y,
                            frame.size.width, frame.size.height,
                            level, is_visible, is_key
                        );

                        // Match by approximate frame bounds (GPUI may apply
                        // slight adjustments, so use tolerance on position
                        // and size). Confirm is shortcut-sized, so size alone
                        // can collide with another compact popup.
                        if (frame.size.width - expected_w as f64).abs() < 2.0
                            && (frame.size.height - expected_h as f64).abs() < 2.0
                            && (frame.origin.x - expected_x as f64).abs() < 4.0
                            && (frame.origin.y - expected_y as f64).abs() < 4.0
                            && is_visible
                        {
                            tracing::info!(
                                target: "script_kit::confirm",
                                event = "configure_confirm_nswindow_matched",
                                index = i,
                                ptr = format!("{:?}", w),
                                "Matched confirm popup NSWindow by frame bounds"
                            );
                            confirm_ns_window = w;
                        }
                    }

                    // Fallback to lastObject if frame matching didn't find it
                    if confirm_ns_window == nil {
                        confirm_ns_window = msg_send![windows, lastObject];
                        tracing::warn!(
                            target: "script_kit::confirm",
                            event = "configure_confirm_nswindow_fallback",
                            ptr = format!("{:?}", confirm_ns_window),
                            "Frame match failed, falling back to lastObject"
                        );
                    }

                    if confirm_ns_window != nil {
                        tracing::info!(
                            target: "script_kit::confirm",
                            event = "configure_confirm_popup_applying",
                            ptr = format!("{:?}", confirm_ns_window),
                            is_dark_vibrancy,
                            "Applying vibrancy + level to confirm NSWindow"
                        );
                        platform::configure_confirm_popup_window(confirm_ns_window, is_dark_vibrancy);

                        // Attach confirm as child of the parent window so AppKit
                        // keeps it above and moves it with the parent.
                        //
                        // Deterministic match first: prefer the NSWindow whose
                        // frame size matches the parent we computed bounds from,
                        // optionally cross-checked against the registered
                        // automation title. This protects multi-window setups
                        // (Notes / Agent Chat / main coexist) where the key window may
                        // not be the *intended* parent. Fall back to the legacy
                        // isKeyWindow / first-visible heuristic only if the
                        // deterministic match fails.
                        let mut main_ns_window: cocoa::base::id = nil;
                        for idx in 0..count {
                            let w: cocoa::base::id = msg_send![windows, objectAtIndex: idx];
                            if w == nil || w == confirm_ns_window {
                                continue;
                            }
                            let w_visible: bool = msg_send![w, isVisible];
                            if !w_visible {
                                continue;
                            }
                            let frame: cocoa::foundation::NSRect = msg_send![w, frame];
                            let size_matches = (frame.size.width
                                - parent_expected_w as f64)
                                .abs()
                                < 2.0
                                && (frame.size.height - parent_expected_h as f64).abs() < 2.0;
                            let title_opt = nswindow_title_string(w);
                            let expected_title_matches = parent_expected_title
                                .as_deref()
                                .is_some_and(|expected| title_opt.as_deref() == Some(expected));
                            let notes_title_matches = parent_automation_id_for_nswindow == "notes"
                                && title_opt.as_deref() == Some("Notes");
                            if (size_matches && expected_title_matches) || notes_title_matches {
                                main_ns_window = w;
                                tracing::info!(
                                    target: "script_kit::confirm",
                                    event = "confirm_window_parent_matched_by_automation_id",
                                    parent_window_id = %parent_automation_id_for_nswindow,
                                    parent_title = ?title_opt,
                                    parent_w = frame.size.width,
                                    parent_h = frame.size.height,
                                    "Matched confirm popup parent NSWindow deterministically"
                                );
                                break;
                            }
                        }
                        if main_ns_window == nil {
                            tracing::warn!(
                                target: "script_kit::confirm",
                                event = "confirm_window_parent_deterministic_match_failed",
                                parent_window_id = %parent_automation_id_for_nswindow,
                                expected_title = ?parent_expected_title,
                                expected_w = parent_expected_w,
                                expected_h = parent_expected_h,
                                "Falling back to legacy key/visible-window parent search"
                            );
                            let mut fallback_ns_window: cocoa::base::id = nil;
                            for idx in 0..count {
                                let w: cocoa::base::id =
                                    msg_send![windows, objectAtIndex: idx];
                                if w != nil && w != confirm_ns_window {
                                    let w_visible: bool = msg_send![w, isVisible];
                                    if w_visible {
                                        let w_key: bool = msg_send![w, isKeyWindow];
                                        if w_key {
                                            main_ns_window = w;
                                            break;
                                        }
                                        if fallback_ns_window == nil {
                                            fallback_ns_window = w;
                                        }
                                    }
                                }
                            }
                            if main_ns_window == nil {
                                main_ns_window = fallback_ns_window;
                                if main_ns_window == nil {
                                    tracing::warn!(
                                        target: "script_kit::confirm",
                                        event = "confirm_window.no_parent_found",
                                        "No visible parent window found for addChildWindow"
                                    );
                                }
                            }
                        }
                        if main_ns_window != nil {
                            // SAFETY: both pointers verified non-nil and distinct.
                            let _: () = msg_send![main_ns_window, addChildWindow:confirm_ns_window ordered:NS_WINDOW_ABOVE];
                            tracing::info!(
                                target: "script_kit::confirm",
                                event = "confirm_window_attached_to_parent",
                                parent_window_id = %parent_automation_id_for_nswindow,
                                parent = format!("{:?}", main_ns_window),
                                child = format!("{:?}", confirm_ns_window),
                                "Attached confirm popup as native child window"
                            );
                        }

                        // Always order front regardless of parent attachment.
                        // orderFrontRegardless is needed for the no-parent fallback and
                        // also ensures the child is visually ordered even if addChildWindow
                        // doesn't immediately reorder on non-activating panels. Do not make
                        // this popup key: the main window routes confirm keys while AppKit
                        // keeps the child popup above the parent.
                        let _: () = msg_send![confirm_ns_window, orderFrontRegardless];
                    } else {
                        tracing::error!(
                            target: "script_kit::confirm",
                            event = "configure_confirm_popup_no_window",
                            "Cannot configure confirm popup: no NSWindow found"
                        );
                    }
                }
            });
        });
    }

    let storage = CONFIRM_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        *guard = Some(handle);
    }
    let parent_storage = CONFIRM_PARENT_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = parent_storage.lock() {
        *guard = Some(parent_window.handle);
    }
    notify_confirm_parent_window(cx);

    // Store result sender and focused button state for key routing from main window
    let tx_storage = CONFIRM_RESULT_TX.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = tx_storage.lock() {
        *guard = Some(result_tx);
    }
    let btn_storage = CONFIRM_FOCUSED_BUTTON.get_or_init(|| Mutex::new(FocusedButton::Confirm));
    if let Ok(mut guard) = btn_storage.lock() {
        *guard = FocusedButton::Confirm;
    }

    // Register in the automation window registry with parent identity.
    // Fail-closed: if registration fails, close the popup and propagate the error.
    if let Err(e) = crate::windows::register_attached_popup(
        "confirm-popup".to_string(),
        crate::protocol::AutomationWindowKind::PromptPopup,
        Some(options.title.to_string()),
        Some("confirmDialog".to_string()),
        Some(automation_bounds_from_gpui(bounds)),
        Some(parent_automation_id.as_str()),
    ) {
        tracing::warn!(
            target: "script_kit::confirm",
            event = "confirm_popup_registry_failed",
            error = %e,
            "Failed to register confirm popup in automation registry — closing popup"
        );
        close_confirm_window(cx);
        return Err(e);
    }

    tracing::info!(
        target: "script_kit::confirm",
        event = "open_confirm_popup_window_ready",
        "open_confirm_popup_window: handle stored, popup ready"
    );

    Ok(handle)
}

/// Validate an explicit `parent_automation_id` (e.g. `"notes"`) against the
/// automation registry. Returns the id back on success; bails when the id is
/// not registered so the popup fails closed instead of attaching to a
/// surprise parent. Extracted from `resolve_confirm_popup_parent_automation_id`
/// so it can be exercised without fabricating a GPUI `AnyWindowHandle` in
/// unit tests.
fn resolve_registered_parent_automation_id(
    parent_automation_id: &str,
    title: &str,
) -> anyhow::Result<String> {
    let Some(parent_info) = crate::windows::automation_window_by_id(parent_automation_id) else {
        tracing::warn!(
            target: "script_kit::confirm",
            event = "confirm_popup_open_blocked_unknown_parent",
            title,
            parent_window_id = parent_automation_id,
            "Confirm popup open blocked: explicit parent automation id is not registered"
        );
        anyhow::bail!(
            "Cannot open confirm popup: parent automation id '{}' is not registered",
            parent_automation_id
        );
    };
    tracing::info!(
        target: "script_kit::confirm",
        event = "confirm_popup_resolved_explicit_parent",
        parent_window_id = %parent_automation_id,
        parent_kind = ?parent_info.kind,
        "Resolved explicit confirm popup parent automation identity"
    );
    Ok(parent_automation_id.to_string())
}

fn resolve_confirm_popup_parent_automation_id(
    parent_window_handle: AnyWindowHandle,
    parent_window_bounds: Bounds<Pixels>,
    parent_automation_id: Option<&str>,
    title: &str,
) -> anyhow::Result<String> {
    if let Some(id) = parent_automation_id {
        let resolved = resolve_registered_parent_automation_id(id, title)?;
        // Refresh the registered runtime handle + live bounds so downstream
        // automation snapshots (and the AppKit child-window lookup) reflect the
        // exact parent we are about to attach to.
        crate::windows::upsert_runtime_window_handle(&resolved, parent_window_handle);
        crate::windows::set_automation_bounds(
            &resolved,
            Some(automation_bounds_from_gpui(parent_window_bounds)),
        );
        return Ok(resolved);
    }

    let Some(main_window_handle) = crate::get_main_window_handle() else {
        tracing::warn!(
            target: "script_kit::confirm",
            event = "confirm_popup_open_blocked_missing_parent",
            title,
            "Confirm popup open blocked: no parent automation identity"
        );
        anyhow::bail!("Cannot open confirm popup: parent automation identity is required");
    };

    if main_window_handle != parent_window_handle {
        tracing::warn!(
            target: "script_kit::confirm",
            event = "confirm_popup_open_blocked_missing_parent",
            title,
            "Confirm popup open blocked: no parent automation identity"
        );
        anyhow::bail!("Cannot open confirm popup: parent automation identity is required");
    }

    let synthesized_parent_id = "main".to_string();
    crate::windows::upsert_runtime_window_handle(&synthesized_parent_id, parent_window_handle);
    crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
        id: synthesized_parent_id.clone(),
        kind: crate::protocol::AutomationWindowKind::Main,
        title: Some("Script Kit".to_string()),
        focused: true,
        visible: true,
        semantic_surface: Some("scriptList".to_string()),
        bounds: Some(crate::protocol::AutomationWindowBounds {
            x: f32::from(parent_window_bounds.origin.x) as f64,
            y: f32::from(parent_window_bounds.origin.y) as f64,
            width: f32::from(parent_window_bounds.size.width) as f64,
            height: f32::from(parent_window_bounds.size.height) as f64,
        }),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
    });
    tracing::info!(
        target: "script_kit::confirm",
        event = "confirm_popup_synthesized_main_parent",
        parent_window_id = %synthesized_parent_id,
        "Synthesized main-window automation identity for confirm popup"
    );

    Ok(synthesized_parent_id)
}

pub(crate) struct ConfirmPopupWindow {
    title: SharedString,
    body: SharedString,
    confirm_text: SharedString,
    cancel_text: SharedString,
    secondary_text: Option<SharedString>,
    confirm_variant: ConfirmButtonVariant,
    focus_handle: FocusHandle,
    focused_button: FocusedButton,
    keep_open_while: Rc<dyn Fn() -> bool>,
    result_tx: async_channel::Sender<ParentDialogResult>,
    lifecycle_task: Option<Task<()>>,
    did_request_focus: bool,
    resolved: bool,
}

impl ConfirmPopupWindow {
    fn new(
        options: ConfirmWindowOptions,
        keep_open_while: Rc<dyn Fn() -> bool>,
        result_tx: async_channel::Sender<ParentDialogResult>,
        cx: &mut Context<Self>,
    ) -> Self {
        tracing::info!(
            target: "script_kit::confirm",
            event = "confirm_popup_window_new",
            title = %options.title,
            body_len = options.body.len(),
            confirm_text = %options.confirm_text,
            cancel_text = %options.cancel_text,
            "ConfirmPopupWindow::new"
        );
        Self {
            title: options.title,
            body: options.body,
            confirm_text: options.confirm_text,
            cancel_text: options.cancel_text,
            secondary_text: options.secondary_text,
            confirm_variant: options.confirm_variant,
            focus_handle: cx.focus_handle(),
            focused_button: FocusedButton::Confirm,
            keep_open_while,
            result_tx,
            lifecycle_task: None,
            did_request_focus: false,
            resolved: false,
        }
    }

    fn shift_focus(&mut self, reverse: bool, cx: &mut Context<Self>) {
        self.focused_button = match (self.focused_button, reverse, self.secondary_text.is_some()) {
            (FocusedButton::Confirm, false, true) => FocusedButton::Secondary,
            (FocusedButton::Secondary, false, true) => FocusedButton::Cancel,
            (FocusedButton::Cancel, false, _) => FocusedButton::Confirm,
            (FocusedButton::Confirm, true, _) => FocusedButton::Cancel,
            (FocusedButton::Cancel, true, true) => FocusedButton::Secondary,
            (FocusedButton::Secondary, true, true) => FocusedButton::Confirm,
            (_, _, false) => FocusedButton::Confirm,
        };
        set_confirm_focused_button(self.focused_button);
        cx.notify();
    }

    fn ensure_lifecycle_task(&mut self, cx: &mut Context<Self>) {
        if self.lifecycle_task.is_some() {
            return;
        }

        tracing::info!(
            target: "script_kit::confirm",
            event = "lifecycle_task_started",
            poll_interval_ms = CONFIRM_LIFECYCLE_POLL_MS,
            "Starting confirm window lifecycle polling task"
        );

        let keep_open_while = self.keep_open_while.clone();
        let result_tx = self.result_tx.clone();

        self.lifecycle_task = Some(cx.spawn(async move |this, cx| {
            let mut poll_count: u64 = 0;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(CONFIRM_LIFECYCLE_POLL_MS))
                    .await;

                poll_count += 1;
                let predicate_result = (keep_open_while)();

                if predicate_result {
                    if poll_count.is_multiple_of(50) {
                        tracing::debug!(
                            target: "script_kit::confirm",
                            event = "lifecycle_poll_heartbeat",
                            poll_count,
                            "Lifecycle predicate still true"
                        );
                    }
                    continue;
                }

                tracing::info!(
                    target: "script_kit::confirm",
                    event = "lifecycle_predicate_false",
                    poll_count,
                    "Lifecycle predicate returned false — closing confirm window"
                );

                let _ = this.update(cx, |this, _cx| {
                    if !this.resolved {
                        tracing::info!(
                            target: "script_kit::confirm",
                            event = "lifecycle_auto_cancel",
                            "Auto-cancelling confirm (lifecycle predicate false)"
                        );
                        this.resolved = true;
                        let _ = result_tx.try_send(ParentDialogResult::Dismiss);
                    }
                });

                cx.update(|cx| {
                    close_confirm_window(cx);
                });

                break;
            }
        }));
    }

    // NOTE: We intentionally do NOT observe_window_activation here.
    // In Accessory app mode the app is never truly "active" in the macOS
    // sense, so the window would report as inactive immediately and close
    // itself. Instead we rely on the lifecycle polling task and explicit
    // user actions (confirm/cancel/escape) to close the window.

    fn resolve_and_close(
        &mut self,
        result: ParentDialogResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.resolved {
            tracing::debug!(
                target: "script_kit::confirm",
                event = "resolve_and_close_already_resolved",
                result = ?result,
                "resolve_and_close: already resolved, ignoring"
            );
            return;
        }

        tracing::info!(
            target: "script_kit::confirm",
            event = "resolve_and_close",
            result = ?result,
            "resolve_and_close: sending result and closing window"
        );

        self.resolved = true;
        let _ = self.result_tx.try_send(result);

        window.defer(cx, |window, _cx| {
            tracing::info!(
                target: "script_kit::confirm",
                event = "resolve_and_close_deferred",
                "resolve_and_close: deferred removal executing"
            );
            unregister_confirm_popup_automation_window("resolve_and_close");
            clear_confirm_window_handle();
            notify_confirm_parent_window(_cx);
            if let Some(storage) = CONFIRM_PARENT_WINDOW.get() {
                if let Ok(mut guard) = storage.lock() {
                    *guard = None;
                }
            }
            window.remove_window();
        });
    }
}

impl Drop for ConfirmPopupWindow {
    fn drop(&mut self) {
        tracing::warn!(
            target: "script_kit::confirm",
            event = "confirm_popup_window_DROPPED",
            resolved = self.resolved,
            title = %self.title,
            "ConfirmPopupWindow entity DROPPED — if resolved=false, the window was destroyed externally"
        );
        if !self.resolved {
            tracing::error!(
                target: "script_kit::confirm",
                event = "confirm_popup_window_DROPPED_UNRESOLVED",
                "ConfirmPopupWindow dropped WITHOUT resolving — this will send false to the result channel"
            );
        }
    }
}

impl Focusable for ConfirmPopupWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConfirmPopupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);
        let is_active = window.is_window_active();
        tracing::info!(
            target: "script_kit::confirm",
            event = "confirm_popup_render",
            is_focused,
            is_active,
            resolved = self.resolved,
            focused_button = ?self.focused_button,
            confirm_variant_is_danger = matches!(self.confirm_variant, ConfirmButtonVariant::Danger),
            did_request_focus = self.did_request_focus,
            "ConfirmPopupWindow::render"
        );

        self.ensure_lifecycle_task(cx);
        self.focused_button = get_confirm_focused_button();

        if !self.did_request_focus {
            self.did_request_focus = true;
            tracing::info!(
                target: "script_kit::confirm",
                event = "confirm_popup_requesting_focus",
                "Requesting initial focus for confirm popup"
            );
            window.focus(&self.focus_handle, cx);
        }

        let handle_key = cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            let modifiers = &event.keystroke.modifiers;
            let intent = confirm_window_key_intent(key, modifiers);

            tracing::info!(
                target: "script_kit::confirm",
                event = "confirm_popup_key_down",
                key,
                intent = ?intent,
                "Confirm popup received key"
            );

            match intent {
                Some(ConfirmWindowKeyIntent::Cancel) => {
                    tracing::info!(
                        target: "script_kit::confirm",
                        event = "confirm_popup_escape",
                        "User pressed Escape — cancelling"
                    );
                    this.resolve_and_close(ParentDialogResult::Dismiss, window, cx);
                }
                Some(ConfirmWindowKeyIntent::ActivateFocused) => {
                    let result = match this.focused_button {
                        FocusedButton::Confirm => ParentDialogResult::Primary,
                        FocusedButton::Secondary => ParentDialogResult::Secondary,
                        FocusedButton::Cancel => ParentDialogResult::Dismiss,
                    };
                    tracing::info!(
                        target: "script_kit::confirm",
                        event = "confirm_popup_enter",
                        result = ?result,
                        focused_button = ?this.focused_button,
                        "User pressed Enter — activating focused button"
                    );
                    this.resolve_and_close(result, window, cx);
                }
                Some(ConfirmWindowKeyIntent::ActivateSecondary) => {
                    if this.secondary_text.is_some() {
                        this.resolve_and_close(ParentDialogResult::Secondary, window, cx);
                    }
                }
                Some(ConfirmWindowKeyIntent::FocusNext) => {
                    this.shift_focus(false, cx);
                }
                Some(ConfirmWindowKeyIntent::FocusPrev) => {
                    this.shift_focus(true, cx);
                }
                None => {
                    tracing::debug!(
                        target: "script_kit::confirm",
                        event = "confirm_popup_consume_unhandled_key",
                        key,
                        "Confirm popup consumed unhandled key"
                    );
                }
            }
            cx.stop_propagation();
        });

        let theme = get_cached_theme();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let title_color = gpui::rgb(chrome.text_primary_hex);
        let body_color = gpui::rgb(chrome.text_secondary_hex);
        let surface_bg = gpui::transparent_black();
        let panel_bg = gpui::rgba(chrome.popup_surface_rgba);
        let border_color = gpui::rgba(chrome.border_rgba);
        let accent_color = gpui::rgb(chrome.accent_hex);
        let cancel_slot_width = confirm_cancel_slot_width();
        let confirm_slot_width = confirm_confirm_slot_width();
        let action_button_height = confirm_action_button_height();
        let action_button_layout = confirm_action_button_layout();

        let current_focused = self.focused_button;
        let cancel_focused = current_focused == FocusedButton::Cancel;
        let secondary_focused = current_focused == FocusedButton::Secondary;
        let confirm_focused = current_focused == FocusedButton::Confirm;

        let entity = cx.entity();
        let cancel_entity = entity.clone();
        let confirm_entity = entity.clone();
        let secondary_entity = entity.clone();

        let title_row = confirm_modal_header(self.title.clone(), accent_color, title_color);

        // Footer button order, not macOS-alert order: the primary ↵ action
        // leads and the Esc action trails, matching the native footer strips
        // ("Run ↵ … Actions ⌘K", Quick Terminal's trailing Close, and the
        // in-window SDK confirm's Apply ↵ / Close Esc).
        let mut action_buttons = vec![ModalActionRowButton {
            id: "confirm-ok-button",
            label: self.confirm_text.clone(),
            key: "↵".into(),
            slot_width_px: confirm_slot_width,
            height_px: action_button_height,
            selected: confirm_focused,
            enabled: true,
            layout: action_button_layout,
            on_click: Box::new(move |_, window, cx| {
                confirm_entity.update(cx, |this: &mut Self, cx| {
                    this.resolve_and_close(ParentDialogResult::Primary, window, cx);
                });
            }),
        }];
        if let Some(label) = self.secondary_text.clone() {
            action_buttons.push(ModalActionRowButton {
                id: "confirm-secondary-button",
                label,
                key: "".into(),
                slot_width_px: cancel_slot_width,
                height_px: action_button_height,
                selected: secondary_focused,
                enabled: true,
                layout: action_button_layout,
                on_click: Box::new(move |_, window, cx| {
                    secondary_entity.update(cx, |this: &mut Self, cx| {
                        this.resolve_and_close(ParentDialogResult::Secondary, window, cx);
                    });
                }),
            });
        }
        action_buttons.push(ModalActionRowButton {
            id: "confirm-cancel-button",
            label: self.cancel_text.clone(),
            key: "Esc".into(),
            slot_width_px: cancel_slot_width,
            height_px: action_button_height,
            selected: cancel_focused,
            enabled: true,
            layout: action_button_layout,
            on_click: Box::new(move |_, window, cx| {
                cancel_entity.update(cx, |this: &mut Self, cx| {
                    this.resolve_and_close(ParentDialogResult::Dismiss, window, cx);
                });
            }),
        });
        let action_row = modal_action_row(
            "confirm-modal-action-row",
            confirm_action_button_gap(),
            action_buttons,
            &theme,
        );

        let has_body = !self.body.trim().is_empty();
        let gaps = confirm_modal_stack_gaps(has_body);
        let mut stack = div()
            .id("confirm-modal-stack")
            .w_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(title_row)
            .child(confirm_modal_spacer(
                "confirm-modal-gap:after-header",
                gaps.after_header_px,
            ));
        if has_body {
            stack = stack
                .child(
                    div()
                        .w_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .text_xs()
                        .line_height(px(confirm_body_line_height()))
                        .text_color(body_color)
                        .child(self.body.clone()),
                )
                .child(confirm_modal_spacer(
                    "confirm-modal-gap:after-body",
                    gaps.after_body_px.unwrap_or(0.0),
                ));
        }
        stack = stack.child(action_row);

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .bg(surface_bg)
            .overflow_hidden()
            .child(confirm_modal_shell(
                ConfirmModalShellConfig {
                    content_id: "confirm-modal-content",
                    width: None,
                    padding_x: CONFIRM_PADDING_X,
                    padding_y: CONFIRM_PADDING_Y,
                    gap: 0.0,
                    background: Some(panel_bg),
                    border: border_color,
                    radius: CONFIRM_MODAL_RADIUS,
                    offset_y: 0.0,
                    opacity: 1.0,
                },
                vec![stack.into_any_element()],
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_window_key_intent_maps_escape_enter_and_tab_navigation() {
        let no_mods = gpui::Modifiers::default();
        let shift_mods = gpui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let command_mods = gpui::Modifiers {
            platform: true,
            ..Default::default()
        };

        assert_eq!(
            confirm_window_key_intent("escape", &no_mods),
            Some(ConfirmWindowKeyIntent::Cancel)
        );
        assert_eq!(
            confirm_window_key_intent("Enter", &no_mods),
            Some(ConfirmWindowKeyIntent::ActivateFocused)
        );
        assert_eq!(
            confirm_window_key_intent("i", &command_mods),
            Some(ConfirmWindowKeyIntent::ActivateSecondary)
        );
        assert_eq!(
            confirm_window_key_intent("tab", &no_mods),
            Some(ConfirmWindowKeyIntent::FocusNext)
        );
        assert_eq!(
            confirm_window_key_intent("tab", &shift_mods),
            Some(ConfirmWindowKeyIntent::FocusPrev)
        );
        assert_eq!(
            confirm_window_key_intent("arrowleft", &no_mods),
            Some(ConfirmWindowKeyIntent::FocusPrev)
        );
        assert_eq!(
            confirm_window_key_intent("right", &no_mods),
            Some(ConfirmWindowKeyIntent::FocusNext)
        );
    }

    #[test]
    fn confirm_action_buttons_use_hugging_footer_action_layout() {
        let metrics = current_main_menu_footer_metrics();
        let layout = confirm_action_button_layout();

        assert_eq!(layout.button_padding_x_px, Some(metrics.button_padding_x));
        assert_eq!(layout.button_padding_y_px, Some(metrics.button_padding_y));
        assert_eq!(layout.content_gap_px, Some(metrics.content_gap));
        assert_eq!(layout.button_radius_px, Some(metrics.button_radius));
        assert_eq!(
            layout.edge_padding_x_px,
            Some(crate::components::footer_chrome::footer_centered_action_edge_padding_x())
        );
        // Confirm/cancel labels are caller-supplied; fixed footer slots
        // ellipsize longer copy ("Cancel" already clipped at the Close slot
        // width). Hug content like render_universal_footer_action_buttons.
        assert!(
            layout.shrink_frame_to_content_px,
            "confirm actions must hug their content so caller-supplied labels never truncate"
        );
        assert!(layout.hug_frame_to_content);
    }

    #[test]
    fn confirm_window_height_grows_with_body_lines_up_to_cap() {
        let no_body = confirm_window_height_from_body_lines(false, 0);
        let one_line = confirm_window_height_from_body_lines(true, 1);
        let three_lines = confirm_window_height_from_body_lines(true, 3);
        let max_lines = confirm_window_height_from_body_lines(true, CONFIRM_BODY_MAX_LINES);
        let many_lines = confirm_window_height_from_body_lines(true, 40);

        assert!(one_line >= no_body);
        assert!(three_lines > one_line);
        assert!(max_lines >= three_lines);
        assert_eq!(
            many_lines, max_lines,
            "body lines must clamp at CONFIRM_BODY_MAX_LINES"
        );
        assert!(many_lines <= CONFIRM_MAX_HEIGHT);
        assert!(no_body >= CONFIRM_MIN_HEIGHT);
    }

    #[test]
    fn confirm_window_bounds_centered_over_parent_window() {
        let parent_bounds = Bounds {
            origin: Point {
                x: px(100.),
                y: px(200.),
            },
            size: Size {
                width: px(750.),
                height: px(500.),
            },
        };

        let expected_width = MODAL_WIDTH_PX;
        let expected_height = confirm_window_height_from_body_lines(true, 1);
        let bounds =
            confirm_window_bounds_from_height(parent_bounds, px(expected_width), expected_height);
        let actual_x: f32 = bounds.origin.x.into();
        let actual_y: f32 = bounds.origin.y.into();
        let actual_w: f32 = bounds.size.width.into();

        let expected_x = 100.0 + ((750.0 - expected_width) / 2.0);
        let expected_y = 200.0 + ((500.0 - expected_height) / 2.0);

        assert!((actual_x - expected_x).abs() < 0.5);
        assert!((actual_y - expected_y).abs() < 0.5);
        assert!((actual_w - expected_width).abs() < 0.5);
    }

    #[test]
    fn confirm_window_bounds_centered_notes_sized_parent() {
        let parent_bounds = Bounds {
            origin: Point {
                x: px(960.),
                y: px(80.),
            },
            size: Size {
                width: px(350.),
                height: px(280.),
            },
        };

        // Notes trash confirm: short two-line body at the compact width.
        let bounds = confirm_window_bounds_from_height(
            parent_bounds,
            px(326.),
            confirm_window_height_from_body_lines(true, 2),
        );

        let x: f32 = bounds.origin.x.into();
        let y: f32 = bounds.origin.y.into();
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();

        assert!(
            (x - (960.0 + ((350.0 - 326.0) / 2.0))).abs() < 0.5,
            "popup x must center over notes parent"
        );
        assert!(
            (width - 326.0).abs() < 0.5,
            "popup width should use the requested compact width when it fits"
        );
        assert!(
            (y - (80.0 + ((280.0 - height) / 2.0))).abs() < 0.5,
            "popup must center vertically over notes parent"
        );
    }

    #[test]
    fn confirm_focus_global_state_tracks_native_popup_focus_changes() {
        if let Ok(mut state) = CONFIRM_HAS_SECONDARY
            .get_or_init(|| Mutex::new(false))
            .lock()
        {
            *state = false;
        }
        let _ = CONFIRM_FOCUSED_BUTTON.get_or_init(|| Mutex::new(FocusedButton::Confirm));
        set_confirm_focused_button(FocusedButton::Confirm);
        assert_eq!(get_confirm_focused_button(), FocusedButton::Confirm);

        cycle_confirm_focused_button(false);
        assert_eq!(get_confirm_focused_button(), FocusedButton::Cancel);

        set_confirm_focused_button(FocusedButton::Confirm);
        assert_eq!(get_confirm_focused_button(), FocusedButton::Confirm);
    }

    #[test]
    fn action_dialog_focus_cycles_through_optional_secondary_action() {
        assert_eq!(
            next_confirm_focused_button(FocusedButton::Confirm, false, true),
            FocusedButton::Secondary
        );
        assert_eq!(
            next_confirm_focused_button(FocusedButton::Secondary, false, true),
            FocusedButton::Cancel
        );
        assert_eq!(
            next_confirm_focused_button(FocusedButton::Cancel, true, true),
            FocusedButton::Secondary
        );
    }

    #[test]
    fn confirm_nswindow_search_matches_position_and_size() {
        let source = std::fs::read_to_string("src/confirm/window.rs")
            .expect("Failed to read src/confirm/window.rs");
        let search_section = source
            .split("configure_confirm_nswindow_search")
            .nth(1)
            .and_then(|section| section.split("if confirm_ns_window == nil").next())
            .expect("expected confirm NSWindow search section");

        assert!(
            search_section.contains("expected_x")
                && search_section.contains("expected_y")
                && search_section.contains("frame.origin.x")
                && search_section.contains("frame.origin.y")
                && search_section.contains("frame.size.width")
                && search_section.contains("frame.size.height"),
            "confirm popup NSWindow matching should include position and size"
        );
    }

    #[test]
    fn confirm_popup_parent_search_prefers_automation_id_before_key_window() {
        let source = std::fs::read_to_string("src/confirm/window.rs")
            .expect("Failed to read src/confirm/window.rs");

        let attach_section = source
            .split("Deterministic match first")
            .nth(1)
            .expect("expected AppKit deterministic-match attach section");

        assert!(
            attach_section.contains("parent_automation_id_for_nswindow")
                && attach_section.contains("confirm_window_parent_matched_by_automation_id"),
            "confirm popup parent NSWindow search should prefer the resolved automation id"
        );

        let automation_match_idx = attach_section
            .find("confirm_window_parent_matched_by_automation_id")
            .expect("expected deterministic parent match event");
        let key_window_idx = attach_section
            .find("msg_send![w, isKeyWindow]")
            .expect("legacy fallback may still call msg_send![w, isKeyWindow]");

        assert!(
            automation_match_idx < key_window_idx,
            "automation-id parent matching must happen before key-window fallback"
        );
    }

    #[test]
    fn resolve_registered_parent_accepts_notes_and_rejects_unknown_ids() {
        use crate::protocol::{AutomationWindowInfo, AutomationWindowKind};

        crate::windows::upsert_automation_window(AutomationWindowInfo {
            id: "notes".to_string(),
            kind: AutomationWindowKind::Notes,
            title: Some("Notes".to_string()),
            focused: true,
            visible: true,
            semantic_surface: Some("notes".to_string()),
            bounds: None,
            parent_window_id: None,
            parent_kind: None,
            pid: Some(std::process::id()),
        });

        let resolved = resolve_registered_parent_automation_id("notes", "Move note to Trash")
            .expect("explicit Notes parent id must resolve");
        assert_eq!(resolved, "notes");

        let unknown = resolve_registered_parent_automation_id("nope-unknown-window-id", "x");
        assert!(
            unknown.is_err(),
            "resolver must reject unregistered explicit parent ids"
        );

        crate::windows::remove_automation_window("notes");
    }

    #[test]
    fn confirm_popup_can_synthesize_main_parent_identity() {
        let source = std::fs::read_to_string("src/confirm/window.rs")
            .expect("Failed to read src/confirm/window.rs");

        assert!(
            source.contains("fn resolve_confirm_popup_parent_automation_id("),
            "confirm popup should resolve the parent automation identity through a dedicated helper"
        );
        assert!(
            source.contains("event = \"confirm_popup_synthesized_main_parent\""),
            "confirm popup should log when it synthesizes the main-window automation identity"
        );
        assert!(
            source.contains("crate::windows::upsert_runtime_window_handle(&synthesized_parent_id, parent_window_handle);"),
            "confirm popup should register the synthesized main-window runtime handle"
        );
    }
}

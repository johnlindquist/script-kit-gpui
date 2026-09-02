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

use crate::runtime_policy::WindowHostPolicy;
use crate::{
    components::confirm_modal_shell::{
        confirm_modal_header, confirm_modal_shell, modal_action_row, ConfirmModalShellConfig,
        ModalActionRowButton, CONFIRM_MODAL_RADIUS, MODAL_WIDTH_PX,
    },
    components::footer_chrome::{
        current_main_menu_footer_height, current_main_menu_footer_metrics,
        footer_action_slot_width, footer_button_height, footer_centered_action_button_layout,
        footer_centered_action_edge_padding_x, FooterActionSlot, FooterHintButtonLayoutOverrides,
    },
    components::overlay_modal::MODAL_PADDING,
    platform,
    theme::get_cached_theme,
    ui_foundation::{is_key_enter, is_key_escape, is_key_left, is_key_tab},
};

const CONFIRM_PADDING_X: f32 = MODAL_PADDING;
const CONFIRM_PADDING_Y: f32 = 20.0;
const CONFIRM_SECTION_GAP: f32 = 10.0;
const CONFIRM_TITLE_LINE_HEIGHT: f32 = 16.0;
const CONFIRM_MODAL_DEFAULT_BODY_LINE_HEIGHT: f32 = 16.0;
const CONFIRM_MIN_HEIGHT: f32 = 132.0;
const CONFIRM_MAX_HEIGHT: f32 = 240.0;
const CONFIRM_BODY_MAX_LINES: usize = 5;
/// The body renders with `.text_xs()` — 0.75rem at the default 16px rem.
const CONFIRM_BODY_FONT_SIZE: f32 = 12.0;
const CONFIRM_LIFECYCLE_POLL_MS: u64 = 120;
/// NSWindowOrderingMode::NSWindowAbove — place child above parent.
const NS_WINDOW_ABOVE: i64 = 1;

static CONFIRM_WINDOW: OnceLock<Mutex<Option<WindowHandle<ConfirmPopupWindow>>>> = OnceLock::new();
static CONFIRM_PARENT_WINDOW: OnceLock<Mutex<Option<(String, u64, AnyWindowHandle)>>> =
    OnceLock::new();
static CONFIRM_RESULT_TX: OnceLock<Mutex<Option<async_channel::Sender<ParentDialogResult>>>> =
    OnceLock::new();
static CONFIRM_FOCUSED_BUTTON: OnceLock<Mutex<FocusedButton>> = OnceLock::new();
static CONFIRM_HAS_SECONDARY: OnceLock<Mutex<bool>> = OnceLock::new();

fn current_confirm_window_handle() -> Option<WindowHandle<ConfirmPopupWindow>> {
    CONFIRM_WINDOW
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|slot| *slot))
}

fn retire_confirm_lifetime(handle: WindowHandle<ConfirmPopupWindow>, generation: Option<u64>) {
    if let Some(generation) = generation {
        crate::windows::remove_runtime_window_instance(CONFIRM_POPUP_AUTOMATION_ID, generation);
    }
    if current_confirm_window_handle() == Some(handle) {
        clear_confirm_window_handle();
        if let Some(slot) = CONFIRM_PARENT_WINDOW.get() {
            if let Ok(mut slot) = slot.lock() {
                *slot = None;
            }
        }
    }
}
const CONFIRM_POPUP_AUTOMATION_ID: &str = "confirm-popup";

fn unregister_confirm_popup_automation_window(reason: &'static str) {
    tracing::info!(
        target: "script_kit::confirm",
        event = "confirm_popup_registry_remove",
        reason
    );
    crate::windows::remove_automation_window(CONFIRM_POPUP_AUTOMATION_ID);
    crate::windows::remove_runtime_window_handle(CONFIRM_POPUP_AUTOMATION_ID);
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

fn confirm_shell_padding_x() -> f32 {
    CONFIRM_PADDING_X
}

fn confirm_shell_padding_y() -> f32 {
    CONFIRM_PADDING_Y
}

fn confirm_shell_gap() -> f32 {
    CONFIRM_SECTION_GAP
}

fn confirm_action_button_height() -> f32 {
    footer_button_height(current_main_menu_footer_height())
}

fn confirm_action_button_gap() -> f32 {
    current_main_menu_footer_metrics().item_gap_px
}

fn confirm_cancel_slot_width() -> f32 {
    footer_action_slot_width(FooterActionSlot::Close)
}

fn confirm_confirm_slot_width() -> f32 {
    footer_action_slot_width(FooterActionSlot::Run)
}

fn confirm_action_button_radius() -> f32 {
    current_main_menu_footer_metrics().button_radius
}

fn confirm_action_button_layout() -> FooterHintButtonLayoutOverrides {
    let footer_layout = footer_centered_action_button_layout();
    let metrics = current_main_menu_footer_metrics();
    FooterHintButtonLayoutOverrides {
        button_padding_x_px: Some(
            footer_layout
                .button_padding_x_px
                .unwrap_or(metrics.button_padding_x),
        ),
        button_padding_y_px: Some(
            footer_layout
                .button_padding_y_px
                .unwrap_or(metrics.button_padding_y),
        ),
        content_gap_px: Some(footer_layout.content_gap_px.unwrap_or(metrics.content_gap)),
        button_radius_px: Some(confirm_action_button_radius()),
        edge_padding_x_px: Some(
            footer_layout
                .edge_padding_x_px
                .unwrap_or_else(footer_centered_action_edge_padding_x),
        ),
        // Confirm/cancel labels are caller-supplied ("Move to Trash", custom
        // cancel copy) — a fixed footer slot ellipsizes them. Hug the rendered
        // content like render_universal_footer_action_buttons while keeping
        // the shared footer metrics above.
        shrink_frame_to_content_px: true,
        hug_frame_to_content: true,
    }
}

fn confirm_anatomy_header_body_gap() -> f32 {
    confirm_shell_gap()
}

fn confirm_anatomy_body_actions_gap() -> f32 {
    confirm_shell_gap()
}

fn confirm_body_line_height() -> f32 {
    CONFIRM_MODAL_DEFAULT_BODY_LINE_HEIGHT
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

#[allow(
    dead_code,
    reason = "legacy confirm automation retains this compatibility result adapter"
)]
pub(crate) fn send_confirm_result(confirmed: bool) {
    send_parent_dialog_result(if confirmed {
        ParentDialogResult::Primary
    } else {
        ParentDialogResult::Dismiss
    });
}

pub(crate) fn send_parent_dialog_result(result: ParentDialogResult) -> bool {
    let Some(storage) = CONFIRM_RESULT_TX.get() else {
        return false;
    };
    let Ok(mut guard) = storage.lock() else {
        return false;
    };
    let Some(tx) = guard.as_ref() else {
        return false;
    };
    match tx.try_send(result) {
        Ok(()) => {
            *guard = None;
            true
        }
        Err(error) => {
            tracing::error!(%error, "confirm_result_delivery_failed");
            false
        }
    }
}

/// Close an action dialog after its owner has completed without treating the
/// close as a user dismissal. This resolves the parent task but invokes none
/// of the primary, secondary, or dismiss callbacks.
#[allow(
    dead_code,
    reason = "the separately compiled application binary owns Agent Chat parent-dialog cleanup"
)]
pub(crate) fn close_parent_action_dialog_programmatically(cx: &mut App) {
    send_parent_dialog_result(ParentDialogResult::ProgrammaticClose);
    close_confirm_window(cx);
}

/// Activate an exact live Confirm button only when submission is explicit.
/// Confirm has no independent selection state: selection-only is refused.
#[allow(dead_code)]
pub(crate) fn batch_select_confirm_button_by_value(
    generation: u64,
    value: &str,
    submit: bool,
    cx: &mut App,
) -> Result<Option<String>, crate::protocol::TransactionError> {
    if !submit {
        return Err(crate::protocol::TransactionError {
            code: crate::protocol::TransactionErrorCode::UnsupportedCommand,
            message: "selection_only_unsupported".into(),
            suggestion: Some("Use explicit submit:true to activate a Confirm button".into()),
        });
    }
    let Some(handle) = current_confirm_window_handle() else {
        return Ok(None);
    };
    if crate::windows::get_runtime_window_handle_for_generation(
        CONFIRM_POPUP_AUTOMATION_ID,
        generation,
    ) != Some(handle.into())
    {
        return Ok(None);
    }
    let parent_live = crate::windows::automation_window_by_id(CONFIRM_POPUP_AUTOMATION_ID)
        .and_then(|info| info.parent_window_id.zip(info.parent_window_generation))
        .is_some_and(|(id, generation)| {
            crate::windows::get_runtime_window_handle_for_generation(&id, generation).is_some()
        });
    if !parent_live {
        return Ok(None);
    }
    let result = match value {
        "confirm" => ParentDialogResult::Primary,
        "secondary" => {
            let has_secondary = CONFIRM_HAS_SECONDARY
                .get()
                .and_then(|state| state.lock().ok())
                .is_some_and(|state| *state);
            if !has_secondary {
                return Ok(None);
            }
            ParentDialogResult::Secondary
        }
        "cancel" => ParentDialogResult::Dismiss,
        _ => return Ok(None),
    };
    if !is_confirm_window_open() || !send_parent_dialog_result(result) {
        return Ok(None);
    }
    close_confirm_window(cx);
    Ok(Some(value.to_string()))
}

/// Select and activate a confirm dialog button by semantic ID. Three-action
/// dialogs insert `button:1:secondary` and move cancel to `button:2:cancel`;
/// legacy two-action dialogs retain `button:1:cancel`.
#[allow(dead_code)]
pub(crate) fn batch_select_confirm_button_by_semantic_id(
    generation: u64,
    semantic_id: &str,
    submit: bool,
    cx: &mut App,
) -> Result<Option<String>, crate::protocol::TransactionError> {
    let has_secondary = CONFIRM_HAS_SECONDARY
        .get()
        .and_then(|state| state.lock().ok())
        .is_some_and(|state| *state);
    let value = match semantic_id {
        "button:0:confirm" => "confirm",
        "button:1:secondary" if has_secondary => "secondary",
        "button:1:cancel" if !has_secondary => "cancel",
        "button:2:cancel" if has_secondary => "cancel",
        _ => return Ok(None),
    };
    Ok(
        batch_select_confirm_button_by_value(generation, value, submit, cx)?
            .map(|_| semantic_id.to_owned()),
    )
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
    let parent = CONFIRM_PARENT_WINDOW
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|slot| slot.clone()));
    if let Some((id, generation, handle)) = parent {
        if crate::windows::get_runtime_window_handle_for_generation(&id, generation) == Some(handle)
        {
            let _ = handle.update(cx, |root, _, cx| cx.notify(root.entity_id()));
        }
    }
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
    pub(crate) generation: u64,
    pub(crate) completion_error: Option<String>,
    pub(crate) revisions: (u64, u64, u64, u64),
}

/// Read the confirm popup snapshot if the popup window is open.
///
/// Used by the automation surface collector to extract semantic elements
/// from the live popup state without needing `&mut App`.
pub(crate) fn get_confirm_popup_snapshot(
    cx: &gpui::App,
    expected_generation: u64,
    window: Option<&Window>,
) -> Option<ConfirmPopupSnapshot> {
    crate::windows::get_runtime_window_handle_for_generation(
        CONFIRM_POPUP_AUTOMATION_ID,
        expected_generation,
    )?;
    let storage = CONFIRM_WINDOW.get()?;
    let handle = (*storage.lock().ok()?)?;
    crate::windows::automation_surface_collector::read_window_root(
        handle,
        window,
        cx,
        |popup, _| {
            if popup.generation != Some(expected_generation) {
                return None;
            }
            Some(ConfirmPopupSnapshot {
                title: popup.title.to_string(),
                body: popup.body.to_string(),
                confirm_text: popup.confirm_text.to_string(),
                cancel_text: popup.cancel_text.to_string(),
                secondary_text: popup.secondary_text.as_ref().map(ToString::to_string),
                focused_button: match popup.focused_button {
                    FocusedButton::Confirm => "confirm",
                    FocusedButton::Secondary => "secondary",
                    FocusedButton::Cancel => "cancel",
                },
                generation: expected_generation,
                completion_error: popup.completion_error.clone(),
                revisions: popup.revision_facts(),
            })
        },
    )
    .ok()
    .flatten()
}

pub(crate) fn is_confirm_window_open() -> bool {
    CONFIRM_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .is_some_and(|guard| guard.is_some())
}

pub(crate) fn close_owned_confirm_window(generation: u64, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::windows::runtime_window_host_policy(CONFIRM_POPUP_AUTOMATION_ID, generation)?
            .is_hidden(),
        "confirm_owned_host_required"
    );
    let handle = crate::windows::get_runtime_window_handle_for_generation(
        CONFIRM_POPUP_AUTOMATION_ID,
        generation,
    )
    .ok_or_else(|| anyhow::anyhow!("confirm_target_stale"))?;
    let handle = handle
        .downcast::<ConfirmPopupWindow>()
        .ok_or_else(|| anyhow::anyhow!("confirm_root_mismatch"))?;
    handle.update(cx, |view, window, cx| {
        anyhow::ensure!(
            view.generation == Some(generation),
            "confirm_generation_mismatch"
        );
        view.resolve_and_close(ParentDialogResult::ProgrammaticClose, window, cx);
        anyhow::ensure!(
            view.resolved,
            "confirm_completion_failed: {}",
            view.completion_error.as_deref().unwrap_or("unknown")
        );
        Ok(())
    })?
}

fn close_confirm_window_if_generation(generation: u64, cx: &mut App) {
    if crate::windows::get_runtime_window_handle_for_generation(
        CONFIRM_POPUP_AUTOMATION_ID,
        generation,
    )
    .is_some()
    {
        close_confirm_window(cx);
    }
}
pub(crate) fn close_confirm_window(cx: &mut App) {
    let handle = CONFIRM_WINDOW
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|mut slot| slot.take()));
    let Some(handle) = handle else {
        return;
    };
    notify_confirm_parent_window(cx);
    unregister_confirm_popup_automation_window("close_confirm_window");
    if let Some(slot) = CONFIRM_PARENT_WINDOW.get() {
        if let Ok(mut slot) = slot.lock() {
            *slot = None;
        }
    }
    clear_confirm_window_handle();
    let _ = handle.update(cx, |_, window, cx| {
        if window.is_owned_hidden() {
            window.remove_window();
        } else {
            crate::platform::dematerialize_then_remove_gpui_window(
                window,
                cx,
                "CONFIRM",
                "Confirm popup",
            );
        }
    });
}

pub(crate) struct ConfirmPopupParentWindow {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) display_id: Option<DisplayId>,
    pub(crate) automation_id: Option<String>,
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
    host_policy: WindowHostPolicy,
) -> anyhow::Result<WindowHandle<ConfirmPopupWindow>> {
    host_policy.validate()?;
    let parent_id = resolve_confirm_popup_parent_automation_id(
        parent_window.handle,
        parent_window.bounds,
        parent_window.automation_id.as_deref(),
        &options.title,
    )?;
    let parent = crate::windows::automation_window_by_id(&parent_id)
        .ok_or_else(|| anyhow::anyhow!("confirm_parent_missing"))?;
    let parent_generation = parent
        .generation
        .ok_or_else(|| anyhow::anyhow!("confirm_parent_generation_missing"))?;
    anyhow::ensure!(
        crate::windows::runtime_window_host_policy(&parent_id, parent_generation)? == host_policy,
        "confirm_parent_host_policy_mismatch"
    );
    close_confirm_window(cx);
    *CONFIRM_HAS_SECONDARY
        .get_or_init(|| Mutex::new(false))
        .lock()
        .map_err(|_| anyhow::anyhow!("confirm_state_poisoned"))? = options.secondary_text.is_some();
    let theme = get_cached_theme();
    let is_dark_vibrancy = theme.should_use_dark_vibrancy();
    let window_background = if !host_policy.is_hidden() && theme.is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        WindowBackgroundAppearance::Opaque
    };
    let bounds = confirm_window_bounds(parent_window.bounds, options.width, &options.body, cx);
    let request = options.clone();
    let sender = result_tx.clone();
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_background,
            focus: false,
            show: !host_policy.is_hidden(),
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id: parent_window.display_id,
            ..Default::default()
        },
        move |_, cx| cx.new(|cx| ConfirmPopupWindow::new(request, keep_open_while, sender, cx)),
    )?;
    handle.update(cx, |popup, _, _| popup.window_handle = Some(handle))?;
    *CONFIRM_WINDOW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| anyhow::anyhow!("confirm_state_poisoned"))? = Some(handle);
    let exact_parent_id = parent_id.clone();

    let publish = move |cx: &mut App| -> anyhow::Result<()> {
        anyhow::ensure!(
            current_confirm_window_handle() == Some(handle),
            "confirm_lifetime_superseded"
        );
        let info = crate::windows::register_runtime_window_instance(
            crate::protocol::AutomationWindowInfo {
                id: CONFIRM_POPUP_AUTOMATION_ID.to_string(),
                kind: crate::protocol::AutomationWindowKind::PromptPopup,
                title: Some(options.title.to_string()),
                focused: false,
                visible: !host_policy.is_hidden(),
                semantic_surface: Some("confirmDialog".into()),
                bounds: Some(automation_bounds_from_gpui(bounds)),
                parent_window_id: Some(parent_id.clone()),
                parent_kind: Some(parent.kind),
                parent_window_generation: Some(parent_generation),
                pid: Some(std::process::id()),
                generation: None,
            },
            handle.into(),
            cx,
        )?;
        let generation = info
            .generation
            .ok_or_else(|| anyhow::anyhow!("confirm_generation_missing"))?;
        let on_close = cx.on_window_closed(move |cx, window_id| {
            if window_id == handle.window_id() {
                if current_confirm_window_handle() == Some(handle) {
                    notify_confirm_parent_window(cx);
                }
                retire_confirm_lifetime(handle, Some(generation));
            }
        });
        handle.update(cx, |popup, _, _| {
            popup.generation = Some(generation);
            popup.close_subscription = Some(on_close);
        })?;
        *CONFIRM_WINDOW
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|_| anyhow::anyhow!("confirm_state_poisoned"))? = Some(handle);
        *CONFIRM_PARENT_WINDOW
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|_| anyhow::anyhow!("confirm_state_poisoned"))? =
            Some((parent_id, parent_generation, parent_window.handle));
        *CONFIRM_RESULT_TX
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|_| anyhow::anyhow!("confirm_state_poisoned"))? = Some(result_tx);
        *CONFIRM_FOCUSED_BUTTON
            .get_or_init(|| Mutex::new(FocusedButton::Confirm))
            .lock()
            .map_err(|_| anyhow::anyhow!("confirm_state_poisoned"))? = FocusedButton::Confirm;
        notify_confirm_parent_window(cx);
        Ok(())
    };

    #[cfg(target_os = "macos")]
    if !host_policy.is_hidden() {
        let parent_handle = parent_window.handle;
        handle.update(cx, move |_, window, cx| {
            window.defer(cx, move |window, cx| {
                if current_confirm_window_handle() != Some(handle)
                    || crate::windows::get_runtime_window_handle_for_generation(
                        &exact_parent_id,
                        parent_generation,
                    ) != Some(parent_handle)
                {
                    window.remove_window();
                    return;
                }
                if crate::runtime_policy::check(
                    crate::runtime_policy::ExternalEffect::NativeVisibility,
                )
                .is_err()
                {
                    window.remove_window();
                    return;
                }
                let child = crate::components::inline_popup_window::inline_popup_ns_window(window);
                let parent = cx
                    .update_window(parent_handle, |_, parent, _| {
                        crate::components::inline_popup_window::inline_popup_ns_window(parent)
                    })
                    .ok()
                    .flatten();
                let (Some(child), Some(parent)) = (child, parent) else {
                    window.remove_window();
                    return;
                };
                if child == parent {
                    window.remove_window();
                    return;
                }
                crate::platform::configure_child_attached_overlay_window_glass(
                    window,
                    "CONFIRM",
                    "Confirm popup",
                );
                // SAFETY: pointers come from exact live GPUI handles on the foreground thread.
                unsafe {
                    use objc::{msg_send, sel, sel_impl};
                    platform::configure_confirm_popup_window(child, is_dark_vibrancy);
                    let _: () = msg_send![parent, addChildWindow: child ordered: NS_WINDOW_ABOVE];
                    let _: () = msg_send![child, orderFrontRegardless];
                }
                cx.defer(move |cx| {
                    if let Err(error) = publish(cx) {
                        tracing::error!(%error, "confirm_registration_failed");
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                });
            });
        })?;
        return Ok(handle);
    }
    if let Err(error) = publish(cx) {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        return Err(error);
    }
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
    _parent_window_bounds: Bounds<Pixels>,
    parent_automation_id: Option<&str>,
    title: &str,
) -> anyhow::Result<String> {
    let parent = match parent_automation_id {
        Some(id) => crate::windows::automation_window_by_id(
            &resolve_registered_parent_automation_id(id, title)?,
        ),
        None => crate::windows::list_automation_windows()
            .into_iter()
            .find(|info| {
                info.generation.is_some_and(|generation| {
                    crate::windows::get_runtime_window_handle_for_generation(&info.id, generation)
                        == Some(parent_window_handle)
                })
            }),
    }
    .ok_or_else(|| anyhow::anyhow!("confirm_parent_identity_missing"))?;
    let generation = parent
        .generation
        .ok_or_else(|| anyhow::anyhow!("confirm_parent_generation_missing"))?;
    anyhow::ensure!(
        crate::windows::get_runtime_window_handle_for_generation(&parent.id, generation)
            == Some(parent_window_handle),
        "confirm_parent_stale"
    );
    Ok(parent.id)
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
    generation: Option<u64>,
    completion_error: Option<String>,
    window_handle: Option<WindowHandle<ConfirmPopupWindow>>,
    close_subscription: Option<gpui::Subscription>,
    semantic_revision: u64,
    presentation_revision: u64,
    applied_theme_revision: u64,
}

impl ConfirmPopupWindow {
    pub(crate) fn revision_facts(&self) -> (u64, u64, u64, u64) {
        (
            self.semantic_revision,
            self.semantic_revision,
            self.presentation_revision,
            self.applied_theme_revision,
        )
    }

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
            generation: None,
            completion_error: None,
            window_handle: None,
            close_subscription: None,
            semantic_revision: 1,
            presentation_revision: 1,
            applied_theme_revision: 0,
        }
    }

    fn shift_focus(&mut self, reverse: bool, cx: &mut Context<Self>) {
        self.focused_button = next_confirm_focused_button(
            self.focused_button,
            reverse,
            self.secondary_text.is_some(),
        );
        self.semantic_revision = self.semantic_revision.saturating_add(1);
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

                let generation = this
                    .update(cx, |this, cx| {
                        if this.resolved {
                            return this.generation;
                        }
                        match result_tx.try_send(ParentDialogResult::Dismiss) {
                            Ok(()) => {
                                this.resolved = true;
                                this.generation
                            }
                            Err(error) => {
                                this.completion_error = Some(error.to_string());
                                cx.notify();
                                None
                            }
                        }
                    })
                    .ok()
                    .flatten();
                if let Some(generation) = generation {
                    cx.update(|cx| close_confirm_window_if_generation(generation, cx));
                }

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
            return;
        }
        if let Err(error) = self.result_tx.try_send(result) {
            self.semantic_revision = self.semantic_revision.saturating_add(1);
            self.completion_error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.resolved = true;
        self.semantic_revision = self.semantic_revision.saturating_add(1);
        let expected_generation = self.generation;
        let expected_handle = window.window_handle();
        window.defer(cx, move |window, cx| {
            if !expected_generation.is_some_and(|generation| {
                crate::windows::get_runtime_window_handle_for_generation(
                    CONFIRM_POPUP_AUTOMATION_ID,
                    generation,
                ) == Some(expected_handle)
            }) {
                window.remove_window();
                return;
            }
            unregister_confirm_popup_automation_window("resolve_and_close");
            clear_confirm_window_handle();
            notify_confirm_parent_window(cx);
            if let Some(storage) = CONFIRM_PARENT_WINDOW.get() {
                if let Ok(mut guard) = storage.lock() {
                    *guard = None;
                }
            }
            if window.is_owned_hidden() {
                window.remove_window();
                return;
            }
            crate::platform::dematerialize_then_remove_gpui_window_from_app(
                window,
                cx,
                "CONFIRM",
                "Confirm popup",
            );
        });
    }
}

impl Drop for ConfirmPopupWindow {
    fn drop(&mut self) {
        if !self.resolved {
            let _ = self
                .result_tx
                .try_send(ParentDialogResult::ProgrammaticClose);
        }
        if let Some(handle) = self.window_handle {
            retire_confirm_lifetime(handle, self.generation);
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
        let revision = crate::theme::service::theme_revision();
        if self.applied_theme_revision != revision {
            self.applied_theme_revision = revision;
            self.presentation_revision = self.presentation_revision.saturating_add(1);
        }
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
        let focused = get_confirm_focused_button();
        if self.focused_button != focused {
            self.focused_button = focused;
            self.semantic_revision = self.semantic_revision.saturating_add(1);
        }

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

        let title_row = confirm_modal_header(self.title.clone(), accent_color, title_color)
            .debug_selector(|| "confirm-modal-header".to_string());

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
        )
        .debug_selector(|| "confirm-modal-action-row".to_string());

        let has_body = !self.body.trim().is_empty();
        let gaps = confirm_modal_stack_gaps(has_body);
        let mut stack = div()
            .id("confirm-modal-stack")
            .debug_selector(|| "confirm-modal-stack".to_string())
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
                        .debug_selector(|| "confirm-modal-body".to_string())
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
            .debug_selector(|| "confirm-popup-root".to_string())
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
    fn confirm_popup_native_attachment_uses_exact_gpui_handles() {
        let source = std::fs::read_to_string("src/confirm/window.rs")
            .expect("Failed to read src/confirm/window.rs");
        let open = source
            .split("pub(crate) fn open_confirm_popup_window(")
            .nth(1)
            .and_then(|body| {
                body.split("\nfn resolve_registered_parent_automation_id(")
                    .next()
            })
            .expect("confirm popup open implementation must exist");

        assert!(
            open.contains("inline_popup_ns_window(window)")
                && open.contains(".update_window(parent_handle,")
                && open.contains("inline_popup_ns_window(parent)")
                && open.contains("addChildWindow: child ordered: NS_WINDOW_ABOVE"),
            "confirm popup must attach the exact live GPUI child and parent, never infer identity from frame geometry"
        );
        assert!(
            open.contains("let (Some(child), Some(parent)) = (child, parent) else {")
                && open.contains("if child == parent {"),
            "native attachment must reject missing handles and self-parenting"
        );
    }

    #[test]
    fn confirm_popup_attachment_revalidates_parent_generation() {
        let source = std::fs::read_to_string("src/confirm/window.rs")
            .expect("Failed to read src/confirm/window.rs");
        let open = source
            .split("pub(crate) fn open_confirm_popup_window(")
            .nth(1)
            .and_then(|body| {
                body.split("\nfn resolve_registered_parent_automation_id(")
                    .next()
            })
            .expect("confirm popup open implementation must exist");
        let revalidation = open
            .split("window.defer(cx, move |window, cx| {")
            .nth(1)
            .and_then(|body| body.split("let child =").next())
            .expect("deferred confirm attachment must validate its owner before native lookup");
        let compact: String = revalidation.split_whitespace().collect();
        assert!(
            compact.contains("current_confirm_window_handle()!=Some(handle)")
                && compact.contains(
                    "get_runtime_window_handle_for_generation(&exact_parent_id,parent_generation,)!=Some(parent_handle)"
                )
                && compact.contains("window.remove_window();return;"),
            "stale popup or parent generations must fail closed before native child attachment"
        );
        assert!(
            !open.contains("isKeyWindow") && !open.contains("orderedWindows"),
            "confirm attachment must not fall back to an unrelated focused or ordered window"
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
            parent_window_generation: None,
            parent_kind: None,
            pid: Some(std::process::id()),
            generation: None,
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
    fn confirm_popup_parent_identity_requires_live_registered_generation() {
        let source = std::fs::read_to_string("src/confirm/window.rs")
            .expect("Failed to read src/confirm/window.rs");
        let resolver = source
            .split("fn resolve_confirm_popup_parent_automation_id(")
            .nth(1)
            .and_then(|body| body.split("\npub(crate) struct ConfirmPopupWindow").next())
            .expect("confirm popup parent identity resolver must exist");

        assert!(
            resolver.contains("resolve_registered_parent_automation_id(id, title)?")
                && resolver
                    .contains("get_runtime_window_handle_for_generation(&parent.id, generation)")
                && resolver.contains("== Some(parent_window_handle)"),
            "confirm popup parent identity must resolve to the exact registered GPUI generation"
        );
        for failure in [
            "confirm_parent_identity_missing",
            "confirm_parent_generation_missing",
            "confirm_parent_stale",
        ] {
            assert!(
                resolver.contains(failure),
                "resolver must fail closed for {failure}"
            );
        }
        assert!(
            !resolver.contains("upsert_") && !resolver.contains("register_runtime_window_instance"),
            "confirm popup resolution must not synthesize or overwrite a parent identity"
        );
    }
}

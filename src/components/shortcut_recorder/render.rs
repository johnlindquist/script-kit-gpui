use gpui::{
    div, prelude::*, px, rgb, rgba, App, Context, FocusHandle, Focusable, IntoElement, Render,
    Window,
};

use crate::components::confirm_modal_shell::{
    confirm_modal_header, confirm_modal_shell, modal_action_row, ConfirmModalShellConfig,
    ModalActionRowButton, CONFIRM_MODAL_RADIUS, MODAL_ACTION_ROW_TOP_MARGIN_PX, MODAL_WIDTH_PX,
};
use crate::components::footer_chrome::{
    current_main_menu_footer_height, current_main_menu_footer_metrics, footer_action_slot_width,
    footer_button_height, footer_centered_action_button_layout,
    footer_centered_action_edge_padding_x, FooterActionSlot, FooterHintButtonLayoutOverrides,
};
use crate::logging;
use crate::ui_foundation::{is_key_enter, is_key_escape};

use super::types::{ShortcutRecorderFocusedAction, RECORDER_MODAL_PADDING};
use super::ShortcutRecorder;

fn recorder_action_button_height() -> f32 {
    footer_button_height(current_main_menu_footer_height())
}

fn recorder_action_button_gap() -> f32 {
    current_main_menu_footer_metrics().item_gap_px
}

fn recorder_action_button_layout() -> FooterHintButtonLayoutOverrides {
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
        button_radius_px: Some(
            footer_layout
                .button_radius_px
                .unwrap_or(metrics.button_radius),
        ),
        edge_padding_x_px: Some(
            footer_layout
                .edge_padding_x_px
                .unwrap_or_else(footer_centered_action_edge_padding_x),
        ),
        // Save/Clear/Cancel must never ellipsize inside a fixed footer slot:
        // hug the rendered content like render_universal_footer_action_buttons
        // while keeping the shared footer metrics above.
        shrink_frame_to_content_px: true,
        hug_frame_to_content: true,
    }
}

impl Focusable for ShortcutRecorder {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ShortcutRecorder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::components::hint_strip::emit_shortcut_chrome_audit(
            "shortcut_recorder",
            "compact-modal",
        );

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);

        // Determine button states
        let can_save = self.shortcut.is_complete() && self.conflict.is_none();
        let can_clear = !self.shortcut.is_empty();

        let title = self
            .command_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or("Shortcut")
            .to_string();
        let header =
            confirm_modal_header(title, rgb(chrome.accent_hex), rgb(chrome.text_primary_hex));

        // Build button row
        let clear_handler = cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
            this.clear(cx);
        });

        let cancel_handler = cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
            this.cancel();
            cx.notify();
        });

        let save_handler = cx.listener(|this, _: &gpui::ClickEvent, _window, cx| {
            this.save();
            cx.notify();
        });

        let action_button_height = recorder_action_button_height();
        let action_button_layout = recorder_action_button_layout();
        let close_slot_width = footer_action_slot_width(FooterActionSlot::Close);
        let run_slot_width = footer_action_slot_width(FooterActionSlot::Run);

        // Footer button order: the primary ↵ action leads and the Esc action
        // trails, matching the native footer strips and the confirm popup.
        let mut button_specs = Vec::new();

        button_specs.push(ModalActionRowButton {
            id: "shortcut-save-button",
            label: "Save".into(),
            key: "↵".into(),
            slot_width_px: run_slot_width,
            height_px: action_button_height,
            selected: self.focused_action == ShortcutRecorderFocusedAction::Save,
            enabled: can_save,
            layout: action_button_layout,
            on_click: Box::new(move |event, window, cx| {
                if can_save {
                    save_handler(event, window, cx);
                }
            }),
        });

        button_specs.push(ModalActionRowButton {
            id: "shortcut-clear-button",
            label: "Clear".into(),
            key: "".into(),
            slot_width_px: close_slot_width,
            height_px: action_button_height,
            selected: self.focused_action == ShortcutRecorderFocusedAction::Clear,
            enabled: can_clear,
            layout: action_button_layout,
            on_click: Box::new(move |event, window, cx| {
                if can_clear {
                    clear_handler(event, window, cx);
                }
            }),
        });

        button_specs.push(ModalActionRowButton {
            id: "shortcut-cancel-button",
            label: "Cancel".into(),
            key: "Esc".into(),
            slot_width_px: close_slot_width,
            height_px: action_button_height,
            selected: self.focused_action == ShortcutRecorderFocusedAction::Cancel,
            enabled: true,
            layout: action_button_layout,
            on_click: Box::new(move |event, window, cx| {
                cancel_handler(event, window, cx);
            }),
        });

        let buttons = div()
            .mt(px(MODAL_ACTION_ROW_TOP_MARGIN_PX))
            .child(modal_action_row(
                "shortcut-modal-action-row",
                recorder_action_button_gap(),
                button_specs,
                &self.theme,
            ));

        // Key down event handler - captures modifiers and keys
        let handle_key_down = cx.listener(move |this, event: &gpui::KeyDownEvent, _window, cx| {
            let key = event.keystroke.key.as_str();
            let mods = event.keystroke.modifiers;

            logging::log(
                "SHORTCUT",
                &format!(
                    "KeyDown: key='{}' cmd={} ctrl={} alt={} shift={}",
                    key, mods.platform, mods.control, mods.alt, mods.shift
                ),
            );

            // Handle special keys
            let is_plain_tab =
                key.eq_ignore_ascii_case("tab") && !mods.platform && !mods.alt && !mods.control;

            if is_plain_tab {
                if mods.shift {
                    this.focus_previous_action(cx);
                } else {
                    this.focus_next_action(cx);
                }
            } else if (mods.platform && key.eq_ignore_ascii_case("w")) || is_key_escape(key) {
                this.cancel();
                cx.notify();
            } else if is_key_enter(key) {
                this.activate_focused_action(cx);
            } else {
                this.handle_key_down(key, mods, cx);
            }
            cx.stop_propagation();
        });

        // Modifiers changed handler - CRITICAL for live modifier feedback
        // This fires whenever ANY modifier key is pressed or released (e.g., pressing Cmd alone)
        let handle_modifiers_changed = cx.listener(
            move |this, event: &gpui::ModifiersChangedEvent, _window, cx| {
                // Only update if we're still recording (haven't captured a complete shortcut yet)
                if this.is_recording {
                    logging::log(
                        "SHORTCUT",
                        &format!(
                            "ModifiersChanged: cmd={} ctrl={} alt={} shift={}",
                            event.modifiers.platform,
                            event.modifiers.control,
                            event.modifiers.alt,
                            event.modifiers.shift
                        ),
                    );
                    // Update current modifiers for live display
                    this.current_modifiers = event.modifiers;
                    cx.notify(); // Trigger re-render to show keycaps
                }
            },
        );

        // Cancel handler for backdrop clicks
        let detached_surface_cancel = cx.listener(|this, _: &gpui::MouseDownEvent, _window, cx| {
            logging::log(
                "SHORTCUT",
                "Shortcut recorder popup margin clicked - cancelling",
            );
            this.cancel();
            cx.notify();
            cx.stop_propagation();
        });

        let mut modal_children = vec![
            header.into_any_element(),
            self.render_key_display().into_any_element(),
            self.render_conflict_warning().into_any_element(),
        ];
        if !self.capture_only {
            modal_children.push(buttons.into_any_element());
        }

        // Modal content - with stop propagation to prevent backdrop dismiss
        let modal = confirm_modal_shell(
            ConfirmModalShellConfig {
                content_id: "shortcut-modal-content",
                width: Some(MODAL_WIDTH_PX),
                padding_x: RECORDER_MODAL_PADDING,
                padding_y: RECORDER_MODAL_PADDING,
                gap: 10.0,
                background: (!self.detached_window).then_some(rgba(chrome.popup_surface_rgba)),
                border: rgba(chrome.border_rgba),
                radius: CONFIRM_MODAL_RADIUS,
                offset_y: 0.0,
                opacity: 1.0,
            },
            modal_children,
        )
        // Stop propagation - clicks inside modal shouldn't dismiss it
        .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {
            // Empty handler stops propagation to backdrop
        });

        let recorder_surface = div()
            .id("shortcut-recorder-overlay")
            .absolute()
            .inset_0()
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key_down)
            .on_modifiers_changed(handle_modifiers_changed); // CRITICAL: Live modifier feedback

        recorder_surface.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .mt(px(0.0))
                .opacity(1.0)
                .on_mouse_down(gpui::MouseButton::Left, detached_surface_cancel)
                .child(modal),
        )
    }
}

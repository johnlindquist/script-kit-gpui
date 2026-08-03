use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::components::button::{Button, ButtonColors, ButtonVariant};
use crate::list_item::FONT_MONO;
use crate::theme::get_cached_theme;
use crate::theme::opacity::{OPACITY_ICON_MUTED, OPACITY_MUTED, OPACITY_NEAR_FULL};
use crate::ui::chrome::alpha_from_opacity;

use super::types::{
    TOAST_ACTIONS_GAP_PX, TOAST_ACTIONS_MARGIN_TOP_PX, TOAST_BORDER_WIDTH_PX, TOAST_CONTENT_GAP_PX,
    TOAST_CONTENT_PADDING_X_PX, TOAST_CONTENT_PADDING_Y_PX, TOAST_ICON_SIZE_PX, TOAST_MAX_WIDTH_PX,
    TOAST_MESSAGE_COLUMN_GAP_PX, TOAST_RADIUS_PX,
};
use super::{Toast, ToastColors, ToastVariant};

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        let variant = self.variant;
        let toast_id = self.id.clone();
        let on_dismiss_callback = self.on_dismiss;
        let is_error = variant == ToastVariant::Error;

        // Check vibrancy to conditionally apply shadow
        let theme = get_cached_theme();
        let vibrancy_enabled = theme.is_vibrancy_enabled();
        let hover_overlay_alpha = alpha_from_opacity(theme.get_opacity().hover) as u8;

        // Icon opacity: full for error (must stay prominent), muted for others
        let icon_opacity = if is_error {
            OPACITY_NEAR_FULL
        } else {
            OPACITY_ICON_MUTED
        };

        // Main toast container
        let base_toast = div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(TOAST_MAX_WIDTH_PX))
            .when_some(
                crate::ui_foundation::get_vibrancy_background(&theme),
                |d, bg| d.bg(bg),
            )
            .border_l(px(TOAST_BORDER_WIDTH_PX))
            .border_color(rgb(colors.border))
            .rounded(px(TOAST_RADIUS_PX));

        // Only apply shadow when vibrancy is disabled - shadows block blur
        let styled_toast = if vibrancy_enabled {
            base_toast
        } else {
            base_toast.shadow_md()
        };

        let root_control_id = toast_id.control_id("root");
        let root_debug_id = root_control_id.clone();
        let mut toast = styled_toast
            .id(ElementId::Name(root_control_id))
            .debug_selector(move || root_debug_id.to_string())
            .overflow_hidden()
            .group("toast");

        // Content row (icon, message, actions, dismiss)
        let content_row = div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(TOAST_CONTENT_GAP_PX))
            .px(px(TOAST_CONTENT_PADDING_X_PX))
            .py(px(TOAST_CONTENT_PADDING_Y_PX));

        let icon = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(TOAST_ICON_SIZE_PX))
            .h(px(TOAST_ICON_SIZE_PX))
            .text_sm()
            .text_color(rgb(colors.icon))
            .opacity(icon_opacity)
            .font_weight(FontWeight::MEDIUM)
            .child(variant.icon());

        // Message and actions column
        let mut message_col = div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(TOAST_MESSAGE_COLUMN_GAP_PX));

        // Message text
        let message_text = div()
            .text_sm()
            .text_color(rgb(colors.text))
            .font_weight(FontWeight::MEDIUM)
            .child(self.message.clone());

        message_col = message_col.child(message_text);

        // Actions row (if any)
        if !self.actions.is_empty() {
            let mut actions_row = div()
                .flex()
                .flex_row()
                .gap(px(TOAST_ACTIONS_GAP_PX))
                .mt(px(TOAST_ACTIONS_MARGIN_TOP_PX));

            for action in self.actions {
                let callback = action.callback.clone();
                let label = action.label.clone();
                let action_control_id =
                    toast_id.control_id(format_args!("action:{}", action.id.as_str()));
                let button_colors = ButtonColors {
                    text_color: colors.action_text,
                    text_hover: colors.action_text,
                    background: colors.action_background,
                    background_hover: colors.action_background,
                    accent: colors.action_text,
                    border: colors.border,
                    focus_ring: colors.action_text,
                    focus_tint: colors.action_background,
                    hover_overlay: ToastColors::overlay_with_alpha(
                        colors.action_background,
                        hover_overlay_alpha,
                    ),
                };
                let action_btn = Button::new(action_control_id, label.clone(), button_colors)
                    .variant(ButtonVariant::Ghost)
                    .on_click(Box::new(move |event, window, cx| {
                        tracing::debug!(action = %label, "Toast action button clicked");
                        (callback)(event, window, cx);
                    }));

                actions_row = actions_row.child(action_btn);
            }

            message_col = message_col.child(actions_row);
        }

        // Dismiss button — quiet by default, visible on hover
        let dismiss_btn = if self.dismissible {
            let dismiss_callback = on_dismiss_callback.clone();
            let button_colors = ButtonColors {
                text_color: colors.dismiss,
                text_hover: colors.text,
                background: 0x00000000, // transparent
                background_hover: colors.action_background,
                accent: colors.dismiss,
                border: 0x00000000, // no border
                focus_ring: colors.dismiss,
                focus_tint: colors.action_background,
                hover_overlay: ToastColors::overlay_with_alpha(
                    colors.action_background,
                    hover_overlay_alpha,
                ),
            };
            let button = Button::new(toast_id.control_id("dismiss"), "×", button_colors)
                .variant(ButtonVariant::Icon)
                .on_click(Box::new(move |_event, window, cx| {
                    tracing::debug!("Toast dismiss button clicked");
                    if let Some(ref callback) = dismiss_callback {
                        callback(window, cx);
                    }
                }));
            Some(
                div()
                    .opacity(OPACITY_ICON_MUTED)
                    .group_hover("toast", |s| s.opacity(OPACITY_MUTED))
                    .child(button),
            )
        } else {
            None
        };

        // Assemble content row
        let mut assembled_row = content_row.child(icon).child(message_col);

        if let Some(dismiss) = dismiss_btn {
            assembled_row = assembled_row.child(dismiss);
        }

        toast = toast.child(assembled_row);

        // Details section (if present)
        if let Some(details_text) = self.details {
            let details_section = div()
                .w_full()
                .px(px(TOAST_CONTENT_PADDING_X_PX))
                .py(px(TOAST_CONTENT_PADDING_Y_PX))
                .bg(rgba(colors.details_bg)) // Theme-aware details background
                .border_t_1()
                .border_color(rgba((colors.border << 8) | 0x40))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors.text))
                        .font_family(FONT_MONO)
                        .overflow_hidden()
                        .child(details_text),
                );

            toast = toast.child(details_section);
        }

        toast
    }
}

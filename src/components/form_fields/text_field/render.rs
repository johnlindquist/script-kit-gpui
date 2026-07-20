use gpui::*;

use super::super::helpers::{char_len, slice_by_char_range};
use super::super::FormFieldMetrics;
use super::FormTextField;

impl Focusable for FormTextField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormTextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let metrics = FormFieldMetrics::from_colors(colors);
        let is_focused = self.focus_handle.is_focused(window);
        let display_text = self.display_text();
        let placeholder = self.field.placeholder.clone().unwrap_or_default();
        let label = self.field.label.clone();
        let cursor_pos = self.cursor_position;
        let has_value = !self.value.is_empty();

        // Only log in debug builds to avoid performance issues in production
        #[cfg(debug_assertions)]
        if std::env::var("SCRIPT_KIT_FIELD_DEBUG").is_ok() {
            crate::logging::log(
                "FIELD",
                &format!(
                    "TextField[{}] render: is_focused={}, value='{}'",
                    self.field.name, is_focused, self.value
                ),
            );
        }

        // Calculate border and background based on focus using shared whisper surface
        let surface = colors.whisper_surface(is_focused);
        let border_color = surface.border;
        let bg_color = surface.background;

        let field_name = self.field.name.clone();
        let field_name_for_log = field_name.clone();

        // Keyboard handler for text input - use unified handler that properly
        // handles char indexing, modifiers, selection, and clipboard
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                #[cfg(debug_assertions)]
                {
                    let key = event.keystroke.key.as_str();
                    crate::logging::log(
                        "FIELD",
                        &format!(
                            "TextField[{}] key: '{}' (key_char: {:?})",
                            field_name_for_log, key, event.keystroke.key_char
                        ),
                    );
                }

                // Use the unified key event handler which:
                // - Uses char indices (not byte indices) for cursor/selection
                // - Handles Cmd/Ctrl modifiers correctly (won't insert "v" on Cmd+V)
                // - Supports selection with Shift+Arrow
                // - Supports clipboard operations
                this.handle_key_event(event, cx);
            },
        );

        // Build cursor element (2px width is fixed for crisp rendering)
        let cursor_element = div()
            .w(px(metrics.cursor_width_px))
            .h(rems(metrics.cursor_height_rems))
            .bg(colors.cursor);

        // Build text content based on value and focus state
        // IMPORTANT: cursor_pos is a CHAR index, not byte index.
        // For password fields with bullets ("•" = 3 bytes), we must slice by char.
        let display_len = char_len(&display_text);
        let safe_cursor = cursor_pos.min(display_len);
        let text_before = slice_by_char_range(&display_text, 0, safe_cursor);
        let text_after = slice_by_char_range(&display_text, safe_cursor, display_len);

        let text_content: Div = if has_value {
            let mut content = div()
                .flex()
                .flex_row()
                .items_center()
                // Text before cursor
                .child(
                    div()
                        .text_size(px(colors.input_font_size))
                        .text_color(colors.text)
                        .child(text_before.to_string()),
                );

            // Cursor (only when focused)
            if is_focused {
                content = content.child(cursor_element);
            }

            // Text after cursor
            content.child(
                div()
                    .text_size(px(colors.input_font_size))
                    .text_color(colors.text)
                    .child(text_after.to_string()),
            )
        } else {
            let mut content = div().flex().flex_row().items_center();

            if is_focused {
                // Cursor when focused and empty
                content = content.child(cursor_element);
            }
            content.child(
                div()
                    .text_size(px(colors.input_font_size))
                    .text_color(colors.placeholder)
                    .child(placeholder),
            )
        };

        // Handle click to focus this field
        let focus_handle_for_click = self.focus_handle.clone();
        let handle_click = cx.listener(
            move |_this: &mut Self,
                  _event: &ClickEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                focus_handle_for_click.focus(window, cx);
            },
        );

        // Input surface uses the shared exact-height single-line prompt control.
        let input_surface =
            crate::components::prompt_single_line_control_surface(bg_color, border_color)
                .id(ElementId::Name(format!("input-{}", field_name).into()))
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key)
                .on_click(handle_click)
                .flex()
                .flex_row()
                .items_center()
                .cursor_text()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_1()
                        .overflow_hidden()
                        .child(text_content),
                );

        // Build the main container - stacked vertical layout with label above input
        let mut container = div()
            .id(ElementId::Name(format!("form-field-{}", field_name).into()))
            .flex()
            .flex_col()
            .gap(px(metrics.field_gap_px))
            .w_full();

        // Add label above input if present
        if let Some(label_text) = label {
            container = container.child(
                div()
                    .text_size(px(colors.label_font_size))
                    .text_color(colors.label)
                    .font_weight(FontWeight::MEDIUM)
                    .child(label_text),
            );
        }

        container.child(input_surface)
    }
}

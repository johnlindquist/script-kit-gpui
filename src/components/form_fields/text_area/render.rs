use gpui::*;
use gpui_component::scroll::ScrollableElement;

use super::super::helpers::{char_len, slice_by_char_range};
use super::super::{
    render_form_field_shell, resolve_form_field_shell_style, FormFieldMetrics, FormFieldShellSpec,
};
use super::FormTextArea;

impl Focusable for FormTextArea {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormTextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let metrics = FormFieldMetrics::from_colors(colors);
        let is_focused = self.focus_handle.is_focused(window);
        let display_text = self.value.clone();
        let placeholder = self.field.placeholder.clone().unwrap_or_default();
        let label = self.field.label.clone().map(SharedString::from);
        let rows = self.rows;
        let has_value = !self.value.is_empty();
        let cursor_pos = self.cursor_position;
        let field_name = self.field.name.clone();
        let field_height = metrics.text_area_height_px(rows);
        let shell_spec = FormFieldShellSpec::neutral(
            format!("form-textarea-{field_name}"),
            label,
            is_focused,
            true,
            field_height,
            Some(field_height),
        );
        let shell_style = resolve_form_field_shell_style(&shell_spec, colors);

        let field_name_for_log = field_name.clone();
        let focus_handle_for_click = self.focus_handle.clone();
        let handle_click = cx.listener(
            move |_this: &mut Self,
                  _event: &ClickEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                #[cfg(debug_assertions)]
                crate::logging::log(
                    "FIELD",
                    &format!("TextArea[{}] clicked - focusing", field_name_for_log),
                );
                focus_handle_for_click.focus(window, cx);
            },
        );
        let handle_key = cx.listener(
            |this: &mut Self,
             event: &KeyDownEvent,
             _window: &mut Window,
             cx: &mut Context<Self>| {
                this.handle_key_event(event, cx);
            },
        );

        let cursor_element = div()
            .w(px(metrics.cursor_width_px))
            .h(rems(metrics.cursor_height_rems))
            .bg(colors.cursor);
        let text_content: Div = if has_value {
            let display_len = char_len(&display_text);
            let safe_cursor = cursor_pos.min(display_len);
            let text_before = slice_by_char_range(&display_text, 0, safe_cursor);
            let text_after = slice_by_char_range(&display_text, safe_cursor, display_len);
            let mut content = div().flex().flex_row().items_start().child(
                div()
                    .text_size(px(metrics.input_font_size))
                    .text_color(shell_style.text)
                    .child(text_before.to_string()),
            );
            if is_focused {
                content = content.child(cursor_element);
            }
            content.child(
                div()
                    .text_size(px(metrics.input_font_size))
                    .text_color(shell_style.text)
                    .child(text_after.to_string()),
            )
        } else {
            let mut content = div().flex().flex_row().items_center();
            if is_focused {
                content = content.child(cursor_element);
            }
            content.child(
                div()
                    .text_size(px(metrics.input_font_size))
                    .text_color(shell_style.placeholder)
                    .child(placeholder),
            )
        };

        let body = div()
            .id(ElementId::Name(format!("textarea-{field_name}").into()))
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .on_click(handle_click)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .cursor_text()
            .overflow_x_hidden()
            .overflow_y_scrollbar()
            .child(text_content)
            .into_any_element();

        render_form_field_shell(&shell_spec, colors, metrics, body)
    }
}

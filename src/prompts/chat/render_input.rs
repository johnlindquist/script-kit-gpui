use super::*;

impl ChatPrompt {
    pub(super) fn render_input(&self, is_focused: bool) -> impl IntoElement {
        let theme_colors = &self.theme.colors;
        let text = self.input.text();
        let cursor_visible = self.cursor_visible && is_focused;
        let input_text_color = rgb(theme_colors.text.primary);
        let placeholder_text_color = rgb(theme_colors.text.muted);
        let pasted_text_pills = crate::pasted_text::token_ranges(text, &self.pasted_text_tokens);
        let rendered_pasted_text_pills: Vec<crate::components::text_input::TextInlinePillRange> =
            pasted_text_pills
                .iter()
                .map(|pill| crate::components::text_input::TextInlinePillRange {
                    start: pill.range.start,
                    end: pill.range.end,
                    label: pill.label.clone(),
                    text_color: theme_colors.text.primary,
                    background_color: theme_colors.accent.selected_subtle,
                    border_color: theme_colors.ui.border,
                })
                .collect();
        // Mini mode: match mini main window's visual input size.
        // The mini main window uses font_size_xl (20.0) on gpui-component Input, which renders
        // smaller than 20px on a raw div().text_size(). font_size_lg (16.0) is the visual match.
        let input_font_size = if self.mini_mode() { 16.0 } else { 14.0 };

        let mut input_content = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .text_size(px(input_font_size))
            .text_color(input_text_color);
        input_content = input_content.child(
            crate::components::text_input::render_text_input_cursor_selection(
                crate::components::text_input::TextInputRenderConfig {
                    cursor: self.input.cursor(),
                    selection: Some(self.input.selection()),
                    cursor_visible,
                    cursor_color: theme_colors.accent.selected,
                    text_color: theme_colors.text.primary,
                    selection_color: theme_colors.accent.selected,
                    selection_text_color: theme_colors.text.primary,
                    transform: Some(Self::input_display_text),
                    pill_ranges: &rendered_pasted_text_pills,
                    ..crate::components::text_input::TextInputRenderConfig::default_for_prompt(text)
                },
            ),
        );

        if text.is_empty() {
            let placeholder = self
                .placeholder
                .clone()
                .unwrap_or_else(|| "Ask follow-up...".into());
            input_content =
                input_content.child(div().text_color(placeholder_text_color).child(placeholder));
        }

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let field_bg = if self.mini_mode() {
            None
        } else if is_focused {
            Some(rgba(chrome.input_active_rgba))
        } else {
            Some(rgba(chrome.input_surface_rgba))
        };

        div()
            .id("chat-input-field")
            .w_full()
            .min_h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .when_some(field_bg, |d, bg| d.bg(bg))
            .child(input_content)
    }

    /// Render the header with back button and title
    pub(super) fn render_header(&self) -> impl IntoElement {
        let colors = &self.prompt_colors;
        let title = self.title.clone().unwrap_or_else(|| "Chat".into());

        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(CHAT_LAYOUT_PADDING_X))
            .py(px(CHAT_LAYOUT_SECTION_PADDING_Y))
            .border_b_1()
            .border_color(rgba((colors.quote_border << 8) | CHAT_LAYOUT_BORDER_ALPHA))
            .child(
                // Back arrow
                div()
                    .text_sm()
                    .text_color(rgb(colors.text_secondary))
                    .child("←"),
            )
            .child(
                // Title
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(colors.text_primary))
                    .child(title),
            )
    }
}

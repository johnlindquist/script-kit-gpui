//! DivPrompt - HTML content display
//!
//! Features:
//! - Parse and render HTML elements as native GPUI components
//! - Support for headers, paragraphs, bold, italic, code, lists, blockquotes
//! - Theme-aware styling
//! - Simple keyboard: Enter or Escape to submit

use gpui::{
    div, prelude::*, px, rgb, rgba, Context, Div, FocusHandle, Focusable, FontWeight, Render,
    Window,
};
use std::sync::Arc;

use crate::designs::{get_tokens, DesignVariant};
use crate::logging;
use crate::theme;
use crate::utils::{parse_html, HtmlElement, TailwindStyles};

use super::SubmitCallback;

/// DivPrompt - HTML content display
///
/// Features:
/// - Parse and render HTML elements as native GPUI components
/// - Support for headers, paragraphs, bold, italic, code, lists, blockquotes
/// - Theme-aware styling
/// - Simple keyboard: Enter or Escape to submit
pub struct DivPrompt {
    pub id: String,
    pub html: String,
    pub tailwind: Option<String>,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
}

impl DivPrompt {
    pub fn new(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_design(
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            DesignVariant::Default,
        )
    }

    pub fn with_design(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        logging::log(
            "PROMPTS",
            &format!(
                "DivPrompt::new with theme colors: bg={:#x}, text={:#x}, design: {:?}",
                theme.colors.background.main, theme.colors.text.primary, design_variant
            ),
        );
        DivPrompt {
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            design_variant,
        }
    }

    /// Submit - always with None value (just acknowledgment)
    fn submit(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }
}

/// Style context for rendering HTML elements
#[derive(Clone, Copy)]
struct RenderContext {
    /// Primary text color
    text_primary: u32,
    /// Secondary text color (for muted content)
    text_secondary: u32,
    /// Tertiary text color
    text_tertiary: u32,
    /// Accent/link color
    accent_color: u32,
    /// Code background color
    code_bg: u32,
    /// Blockquote border color
    quote_border: u32,
    /// HR color
    hr_color: u32,
}

impl RenderContext {
    fn from_theme(colors: &theme::ColorScheme) -> Self {
        Self {
            text_primary: colors.text.primary,
            text_secondary: colors.text.secondary,
            text_tertiary: colors.text.tertiary,
            accent_color: colors.accent.selected,
            code_bg: colors.background.search_box,
            quote_border: colors.ui.border,
            hr_color: colors.ui.border,
        }
    }
}

/// Render a vector of HtmlElements as a GPUI Div
fn render_elements(elements: &[HtmlElement], ctx: RenderContext) -> Div {
    let mut container = div().flex().flex_col().gap_2().w_full();

    for element in elements {
        container = container.child(render_element(element, ctx));
    }

    container
}

/// Render a single HtmlElement as a GPUI element
fn render_element(element: &HtmlElement, ctx: RenderContext) -> Div {
    match element {
        HtmlElement::Text(text) => {
            // Text is a block with the text content
            div()
                .w_full()
                .text_color(rgb(ctx.text_secondary))
                .child(text.clone())
        }

        HtmlElement::Header { level, children } => {
            let font_size = match level {
                1 => 28.0,
                2 => 24.0,
                3 => 20.0,
                4 => 18.0,
                5 => 16.0,
                _ => 14.0,
            };

            // Collect all text content from children
            let text_content = collect_text(children);

            div()
                .w_full()
                .text_size(px(font_size))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(ctx.text_primary))
                .mb(px(8.0))
                .child(text_content)
        }

        HtmlElement::Paragraph(children) => {
            // Collect all text content from children
            let text_content = collect_text(children);

            div()
                .w_full()
                .text_size(px(14.0))
                .text_color(rgb(ctx.text_secondary))
                .mb(px(8.0))
                .child(text_content)
        }

        HtmlElement::Bold(children) => {
            let text_content = collect_text(children);
            div().font_weight(FontWeight::BOLD).child(text_content)
        }

        HtmlElement::Italic(children) => {
            // GPUI doesn't have native italic support, so we use a slightly different color
            let text_content = collect_text(children);
            div().text_color(rgb(ctx.text_tertiary)).child(text_content)
        }

        HtmlElement::InlineCode(code) => div()
            .px(px(6.0))
            .py(px(2.0))
            .bg(rgba((ctx.code_bg << 8) | 0x80))
            .rounded(px(4.0))
            .font_family("Menlo")
            .text_size(px(13.0))
            .text_color(rgb(ctx.accent_color))
            .child(code.clone()),

        HtmlElement::CodeBlock { code, .. } => div()
            .w_full()
            .p(px(12.0))
            .mb(px(8.0))
            .bg(rgba((ctx.code_bg << 8) | 0xC0))
            .rounded(px(6.0))
            .font_family("Menlo")
            .text_size(px(13.0))
            .text_color(rgb(ctx.text_primary))
            .child(code.clone()),

        HtmlElement::UnorderedList(items) => {
            let mut list = div()
                .flex()
                .flex_col()
                .gap_1()
                .mb(px(8.0))
                .pl(px(16.0))
                .w_full();

            for item in items {
                if let HtmlElement::ListItem(children) = item {
                    let text_content = collect_text(children);
                    list = list.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .w_full()
                            .child(
                                div().text_color(rgb(ctx.text_tertiary)).child("\u{2022}"), // Bullet point
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(rgb(ctx.text_secondary))
                                    .child(text_content),
                            ),
                    );
                }
            }

            list
        }

        HtmlElement::OrderedList(items) => {
            let mut list = div()
                .flex()
                .flex_col()
                .gap_1()
                .mb(px(8.0))
                .pl(px(16.0))
                .w_full();

            for (index, item) in items.iter().enumerate() {
                if let HtmlElement::ListItem(children) = item {
                    let text_content = collect_text(children);
                    list = list.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .w_full()
                            .child(
                                div()
                                    .text_color(rgb(ctx.text_tertiary))
                                    .min_w(px(20.0))
                                    .child(format!("{}.", index + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(rgb(ctx.text_secondary))
                                    .child(text_content),
                            ),
                    );
                }
            }

            list
        }

        HtmlElement::ListItem(children) => {
            // Standalone list item (shouldn't normally happen, but handle gracefully)
            let text_content = collect_text(children);
            div()
                .w_full()
                .text_color(rgb(ctx.text_secondary))
                .child(text_content)
        }

        HtmlElement::Blockquote(children) => {
            let text_content = collect_text(children);
            div()
                .w_full()
                .pl(px(16.0))
                .py(px(8.0))
                .mb(px(8.0))
                .border_l_4()
                .border_color(rgb(ctx.quote_border))
                .text_color(rgb(ctx.text_tertiary))
                .child(text_content)
        }

        HtmlElement::HorizontalRule => div().w_full().h(px(1.0)).my(px(12.0)).bg(rgb(ctx.hr_color)),

        HtmlElement::Link { children, .. } => {
            // Links are styled but not clickable (as per requirements)
            let text_content = collect_text(children);
            div().text_color(rgb(ctx.accent_color)).child(text_content)
        }

        HtmlElement::LineBreak => {
            div().h(px(8.0)) // Line break spacing
        }

        HtmlElement::Div { classes, children } => {
            let base = render_elements(children, ctx);
            if let Some(class_str) = classes {
                apply_tailwind_styles(base, class_str)
            } else {
                base
            }
        }

        HtmlElement::Span { classes, children } => {
            let base = render_elements(children, ctx);
            if let Some(class_str) = classes {
                apply_tailwind_styles(base, class_str)
            } else {
                base
            }
        }
    }
}

/// Apply Tailwind styles to a div based on a class string
fn apply_tailwind_styles(mut element: Div, class_string: &str) -> Div {
    let styles = TailwindStyles::parse(class_string);

    // Layout
    if styles.flex {
        element = element.flex();
    }
    if styles.flex_col {
        element = element.flex_col();
    }
    if styles.flex_row {
        element = element.flex_row();
    }
    if styles.flex_1 {
        element = element.flex_1();
    }
    if styles.items_center {
        element = element.items_center();
    }
    if styles.items_start {
        element = element.items_start();
    }
    if styles.items_end {
        element = element.items_end();
    }
    if styles.justify_center {
        element = element.justify_center();
    }
    if styles.justify_between {
        element = element.justify_between();
    }
    if styles.justify_start {
        element = element.justify_start();
    }
    if styles.justify_end {
        element = element.justify_end();
    }

    // Sizing
    if styles.w_full {
        element = element.w_full();
    }
    if styles.h_full {
        element = element.h_full();
    }
    if styles.min_w_0 {
        element = element.min_w(px(0.));
    }
    if styles.min_h_0 {
        element = element.min_h(px(0.));
    }

    // Spacing - padding
    if let Some(p) = styles.padding {
        element = element.p(px(p));
    }
    if let Some(px_val) = styles.padding_x {
        element = element.px(px(px_val));
    }
    if let Some(py_val) = styles.padding_y {
        element = element.py(px(py_val));
    }
    if let Some(pt) = styles.padding_top {
        element = element.pt(px(pt));
    }
    if let Some(pb) = styles.padding_bottom {
        element = element.pb(px(pb));
    }
    if let Some(pl) = styles.padding_left {
        element = element.pl(px(pl));
    }
    if let Some(pr) = styles.padding_right {
        element = element.pr(px(pr));
    }

    // Spacing - margin
    if let Some(m) = styles.margin {
        element = element.m(px(m));
    }
    if let Some(mx_val) = styles.margin_x {
        element = element.mx(px(mx_val));
    }
    if let Some(my_val) = styles.margin_y {
        element = element.my(px(my_val));
    }
    if let Some(mt) = styles.margin_top {
        element = element.mt(px(mt));
    }
    if let Some(mb) = styles.margin_bottom {
        element = element.mb(px(mb));
    }
    if let Some(ml) = styles.margin_left {
        element = element.ml(px(ml));
    }
    if let Some(mr) = styles.margin_right {
        element = element.mr(px(mr));
    }

    // Gap
    if let Some(gap_val) = styles.gap {
        element = element.gap(px(gap_val));
    }

    // Colors
    if let Some(color) = styles.bg_color {
        element = element.bg(rgb(color));
    }
    if let Some(color) = styles.text_color {
        element = element.text_color(rgb(color));
    }
    if let Some(color) = styles.border_color {
        element = element.border_color(rgb(color));
    }

    // Typography
    if let Some(size) = styles.font_size {
        element = element.text_size(px(size));
    }
    if styles.font_bold {
        element = element.font_weight(FontWeight::BOLD);
    }
    if styles.font_medium {
        element = element.font_weight(FontWeight::MEDIUM);
    }
    if styles.font_normal {
        element = element.font_weight(FontWeight::NORMAL);
    }

    // Border radius
    if let Some(r) = styles.rounded {
        element = element.rounded(px(r));
    }

    // Border
    if styles.border {
        element = element.border_1();
    }
    if let Some(width) = styles.border_width {
        if width == 0.0 {
            // No border
        } else if width == 2.0 {
            element = element.border_2();
        } else if width == 4.0 {
            element = element.border_4();
        } else if width == 8.0 {
            element = element.border_8();
        }
    }

    element
}

/// Collect all text content from HTML elements into a single string
fn collect_text(elements: &[HtmlElement]) -> String {
    let mut result = String::new();

    for element in elements {
        match element {
            HtmlElement::Text(text) => result.push_str(text),
            HtmlElement::Bold(children) => result.push_str(&collect_text(children)),
            HtmlElement::Italic(children) => result.push_str(&collect_text(children)),
            HtmlElement::InlineCode(code) => {
                result.push('`');
                result.push_str(code);
                result.push('`');
            }
            HtmlElement::Link { children, .. } => result.push_str(&collect_text(children)),
            HtmlElement::LineBreak => result.push('\n'),
            HtmlElement::Header { children, .. }
            | HtmlElement::Paragraph(children)
            | HtmlElement::ListItem(children)
            | HtmlElement::Blockquote(children)
            | HtmlElement::Div { children, .. }
            | HtmlElement::Span { children, .. } => {
                result.push_str(&collect_text(children));
            }
            HtmlElement::UnorderedList(items) | HtmlElement::OrderedList(items) => {
                for item in items {
                    result.push_str(&collect_text(std::slice::from_ref(item)));
                    result.push(' ');
                }
            }
            HtmlElement::CodeBlock { code, .. } => {
                result.push_str(code);
            }
            HtmlElement::HorizontalRule => {
                result.push_str("---");
            }
        }
    }

    result
}

/// Render an inline element (text, bold, italic, code, link)
fn render_inline(element: &HtmlElement, ctx: RenderContext) -> Div {
    match element {
        HtmlElement::Text(text) => div().child(text.clone()),

        HtmlElement::Bold(children) => div()
            .flex()
            .flex_row()
            .items_baseline()
            .font_weight(FontWeight::BOLD)
            .children(children.iter().map(|c| render_inline(c, ctx))),

        HtmlElement::Italic(children) => div()
            .flex()
            .flex_row()
            .items_baseline()
            .text_color(rgb(ctx.text_tertiary))
            .children(children.iter().map(|c| render_inline(c, ctx))),

        HtmlElement::InlineCode(code) => div()
            .px(px(4.0))
            .py(px(1.0))
            .bg(rgba((ctx.code_bg << 8) | 0x80))
            .rounded(px(3.0))
            .font_family("Menlo")
            .text_size(px(12.0))
            .text_color(rgb(ctx.accent_color))
            .child(code.clone()),

        HtmlElement::Link { children, .. } => div()
            .flex()
            .flex_row()
            .items_baseline()
            .text_color(rgb(ctx.accent_color))
            .children(children.iter().map(|c| render_inline(c, ctx))),

        HtmlElement::LineBreak => div().h(px(14.0)),

        // Block elements appearing inline - just render their content
        HtmlElement::Header { children, .. }
        | HtmlElement::Paragraph(children)
        | HtmlElement::ListItem(children)
        | HtmlElement::Blockquote(children)
        | HtmlElement::Div { children, .. }
        | HtmlElement::Span { children, .. } => div()
            .flex()
            .flex_row()
            .items_baseline()
            .children(children.iter().map(|c| render_inline(c, ctx))),

        HtmlElement::UnorderedList(items) | HtmlElement::OrderedList(items) => {
            // Flatten list items inline
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .children(items.iter().filter_map(|item| {
                    if let HtmlElement::ListItem(children) = item {
                        Some(
                            div()
                                .flex()
                                .flex_row()
                                .children(children.iter().map(|c| render_inline(c, ctx))),
                        )
                    } else {
                        None
                    }
                }))
        }

        HtmlElement::CodeBlock { code, .. } => div()
            .px(px(4.0))
            .py(px(1.0))
            .bg(rgba((ctx.code_bg << 8) | 0x80))
            .rounded(px(3.0))
            .font_family("Menlo")
            .text_size(px(12.0))
            .child(code.clone()),

        HtmlElement::HorizontalRule => div().w(px(20.0)).h(px(1.0)).bg(rgb(ctx.hr_color)),
    }
}

impl Focusable for DivPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DivPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get design tokens for the current design variant
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  _cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();

                match key_str.as_str() {
                    "enter" | "escape" => this.submit(),
                    _ => {}
                }
            },
        );

        // Parse HTML into elements
        let elements = parse_html(&self.html);

        // Create render context from theme
        let render_ctx = if self.design_variant == DesignVariant::Default {
            RenderContext::from_theme(&self.theme.colors)
        } else {
            RenderContext {
                text_primary: colors.text_primary,
                text_secondary: colors.text_secondary,
                text_tertiary: colors.text_muted, // Use text_muted for tertiary
                accent_color: colors.accent,
                code_bg: colors.background_tertiary, // Use background_tertiary for code bg
                quote_border: colors.border,
                hr_color: colors.border,
            }
        };

        // Use design tokens for colors (with theme fallback for Default variant)
        let main_bg = if self.design_variant == DesignVariant::Default {
            rgb(self.theme.colors.background.main)
        } else {
            rgb(colors.background)
        };

        // Generate semantic IDs for div prompt elements
        let panel_semantic_id = format!("panel:content-{}", self.id);

        // Render the HTML elements with any inline Tailwind classes
        let content = render_elements(&elements, render_ctx);

        // Apply root tailwind classes if provided
        let styled_content = if let Some(tw) = &self.tailwind {
            apply_tailwind_styles(content, tw)
        } else {
            content
        };

        // Main container - fills entire window height with no bottom gap
        // Content area uses flex_1 to fill all remaining space
        div()
            .id(gpui::ElementId::Name("window:div".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full() // Fill container height completely
            .min_h(px(0.)) // Allow proper flex behavior
            .bg(main_bg)
            .p(px(spacing.padding_lg))
            .key_context("div_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                div()
                    .id(gpui::ElementId::Name(panel_semantic_id.into()))
                    .flex_1() // Grow to fill available space to bottom
                    .min_h(px(0.)) // Allow shrinking
                    .w_full()
                    .overflow_y_hidden() // Clip content at container boundary
                    .child(styled_content),
            )
        // Footer removed - content now extends to bottom of container
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_context_from_theme() {
        let colors = theme::ColorScheme::dark_default();
        let ctx = RenderContext::from_theme(&colors);

        assert_eq!(ctx.text_primary, colors.text.primary);
        assert_eq!(ctx.text_secondary, colors.text.secondary);
        assert_eq!(ctx.accent_color, colors.accent.selected);
    }

    #[test]
    fn test_render_simple_text() {
        let elements = parse_html("Hello World");
        let ctx = RenderContext::from_theme(&theme::ColorScheme::dark_default());

        // Should not panic
        let _ = render_elements(&elements, ctx);
    }

    #[test]
    fn test_render_complex_html() {
        let html = r#"
            <h1>Title</h1>
            <p>A paragraph with <strong>bold</strong> and <em>italic</em> text.</p>
            <ul>
                <li>Item 1</li>
                <li>Item 2</li>
            </ul>
            <blockquote>A quote</blockquote>
            <pre><code>let x = 1;</code></pre>
            <hr>
            <a href="https://example.com">Link</a>
        "#;
        let elements = parse_html(html);
        let ctx = RenderContext::from_theme(&theme::ColorScheme::dark_default());

        // Should not panic
        let _ = render_elements(&elements, ctx);
    }

    #[test]
    fn test_render_headers_different_sizes() {
        for level in 1..=6 {
            let html = format!("<h{}>Header {}</h{}>", level, level, level);
            let elements = parse_html(&html);
            let ctx = RenderContext::from_theme(&theme::ColorScheme::dark_default());

            // Should not panic
            let _ = render_elements(&elements, ctx);
        }
    }

    #[test]
    fn test_render_nested_formatting() {
        let html = "<p><strong><em>Bold and italic</em></strong></p>";
        let elements = parse_html(html);
        let ctx = RenderContext::from_theme(&theme::ColorScheme::dark_default());

        // Should not panic
        let _ = render_elements(&elements, ctx);
    }
}

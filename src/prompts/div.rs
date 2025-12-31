//! DivPrompt - HTML content display
//!
//! Features:
//! - Parse and render HTML elements as native GPUI components
//! - Support for headers, paragraphs, bold, italic, code, lists, blockquotes
//! - Theme-aware styling
//! - Simple keyboard: Enter or Escape to submit

use gpui::{
    div, prelude::*, px, rgb, rgba, Context, Div, FocusHandle, Focusable, FontWeight, Hsla, Render,
    Window,
};
use std::sync::Arc;

use crate::designs::{get_tokens, DesignVariant};
use crate::logging;
use crate::theme;
use crate::utils::{parse_color, parse_html, HtmlElement, TailwindStyles};

use super::SubmitCallback;

/// Options for customizing the div container appearance
#[derive(Debug, Clone, Default)]
pub struct ContainerOptions {
    /// Background color: "transparent", "#RRGGBB", "#RRGGBBAA", or Tailwind color name
    pub background: Option<String>,
    /// Padding in pixels, or None to use default
    pub padding: Option<ContainerPadding>,
    /// Opacity (0-100), applies to entire container
    pub opacity: Option<u8>,
    /// Tailwind classes for the content container
    pub container_classes: Option<String>,
}

/// Padding options for the container
#[derive(Debug, Clone)]
pub enum ContainerPadding {
    /// No padding
    None,
    /// Custom padding in pixels
    Pixels(f32),
}

impl ContainerOptions {
    /// Parse container background to GPUI color
    pub fn parse_background(&self) -> Option<Hsla> {
        let bg = self.background.as_ref()?;

        // Handle "transparent"
        if bg == "transparent" {
            return Some(Hsla::transparent_black());
        }

        // Handle hex colors: #RGB, #RRGGBB, #RRGGBBAA
        if bg.starts_with('#') {
            return parse_hex_color(bg);
        }

        // Handle Tailwind color names (e.g., "blue-500", "gray-900")
        if let Some(color) = parse_color(bg) {
            return Some(rgb_to_hsla(color, self.opacity));
        }

        None
    }

    /// Get padding value
    pub fn get_padding(&self, default: f32) -> f32 {
        match &self.padding {
            Some(ContainerPadding::None) => 0.0,
            Some(ContainerPadding::Pixels(px)) => *px,
            None => default,
        }
    }
}

/// Parse hex color string to GPUI Hsla
fn parse_hex_color(hex: &str) -> Option<Hsla> {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        // #RGB -> #RRGGBB
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Hsla::from(gpui::Rgba {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: 1.0,
            }))
        }
        // #RRGGBB
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Hsla::from(gpui::Rgba {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: 1.0,
            }))
        }
        // #RRGGBBAA
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Hsla::from(gpui::Rgba {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: a as f32 / 255.0,
            }))
        }
        _ => None,
    }
}

/// Convert RGB u32 to Hsla with optional opacity
fn rgb_to_hsla(color: u32, opacity: Option<u8>) -> Hsla {
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    let a = opacity.map(|o| o as f32 / 100.0).unwrap_or(1.0);
    Hsla::from(gpui::Rgba { r, g, b, a })
}

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
    /// Container customization options
    pub container_options: ContainerOptions,
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
        Self::with_options(
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            DesignVariant::Default,
            ContainerOptions::default(),
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
        Self::with_options(
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
            theme,
            design_variant,
            ContainerOptions::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
        container_options: ContainerOptions,
    ) -> Self {
        logging::log(
            "PROMPTS",
            &format!(
                "DivPrompt::new with theme colors: bg={:#x}, text={:#x}, design: {:?}, container_opts: {:?}",
                theme.colors.background.main, theme.colors.text.primary, design_variant, container_options
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
            container_options,
        }
    }

    /// Submit - always with None value (just acknowledgment)
    fn submit(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Submit with a specific value (for submit:value links)
    fn submit_with_value(&mut self, value: String) {
        logging::log("DIV", &format!("Submit with value: {}", value));
        (self.on_submit)(self.id.clone(), Some(value));
    }

    /// Handle link click based on href pattern
    pub fn handle_link_click(&mut self, href: &str) {
        logging::log("DIV", &format!("Link clicked: {}", href));

        if let Some(value) = href.strip_prefix("submit:") {
            self.submit_with_value(value.to_string());
        } else if href.starts_with("http://") || href.starts_with("https://") {
            if let Err(e) = open::that(href) {
                logging::log("DIV", &format!("Failed to open URL {}: {}", href, e));
            }
        } else if href.starts_with("file://") {
            if let Err(e) = open::that(href) {
                logging::log("DIV", &format!("Failed to open file {}: {}", href, e));
            }
        } else {
            logging::log("DIV", &format!("Unknown link protocol: {}", href));
        }
    }
}

/// Callback type for link clicks - needs App context to update entity
type LinkClickCallback = Arc<dyn Fn(&str, &mut gpui::App) + Send + Sync>;

/// Style context for rendering HTML elements
#[derive(Clone)]
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
    /// Optional link click callback
    on_link_click: Option<LinkClickCallback>,
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
            on_link_click: None,
        }
    }

    fn with_link_callback(mut self, callback: LinkClickCallback) -> Self {
        self.on_link_click = Some(callback);
        self
    }
}

/// Render a vector of HtmlElements as a GPUI Div
fn render_elements(elements: &[HtmlElement], ctx: RenderContext) -> Div {
    let mut container = div().flex().flex_col().gap_2().w_full();

    for element in elements {
        container = container.child(render_element(element, ctx.clone()));
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

        HtmlElement::Link { href, children } => {
            // Links are styled and clickable
            let text_content = collect_text(children);
            let mut link_div = div()
                .text_color(rgb(ctx.accent_color))
                .cursor_pointer()
                .child(text_content);

            if let Some(ref callback) = ctx.on_link_click {
                let cb = callback.clone();
                let href_for_click = href.clone();
                link_div = link_div.on_mouse_down(
                    gpui::MouseButton::Left,
                    move |_event, _window, cx: &mut gpui::App| {
                        cb(&href_for_click, cx);
                    },
                );
            }

            link_div
        }

        HtmlElement::LineBreak => {
            div().h(px(8.0)) // Line break spacing
        }

        HtmlElement::Div { classes, children } => {
            let base = render_elements(children, ctx.clone());
            if let Some(class_str) = classes {
                apply_tailwind_styles(base, class_str)
            } else {
                base
            }
        }

        HtmlElement::Span { classes, children } => {
            let base = render_elements(children, ctx.clone());
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

        HtmlElement::Bold(children) => {
            let ctx_clone = ctx.clone();
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .font_weight(FontWeight::BOLD)
                .children(
                    children
                        .iter()
                        .map(move |c| render_inline(c, ctx_clone.clone())),
                )
        }

        HtmlElement::Italic(children) => {
            let ctx_clone = ctx.clone();
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .text_color(rgb(ctx.text_tertiary))
                .children(
                    children
                        .iter()
                        .map(move |c| render_inline(c, ctx_clone.clone())),
                )
        }

        HtmlElement::InlineCode(code) => div()
            .px(px(4.0))
            .py(px(1.0))
            .bg(rgba((ctx.code_bg << 8) | 0x80))
            .rounded(px(3.0))
            .font_family("Menlo")
            .text_size(px(12.0))
            .text_color(rgb(ctx.accent_color))
            .child(code.clone()),

        HtmlElement::Link { children, .. } => {
            let ctx_clone = ctx.clone();
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .text_color(rgb(ctx.accent_color))
                .children(
                    children
                        .iter()
                        .map(move |c| render_inline(c, ctx_clone.clone())),
                )
        }

        HtmlElement::LineBreak => div().h(px(14.0)),

        // Block elements appearing inline - just render their content
        HtmlElement::Header { children, .. }
        | HtmlElement::Paragraph(children)
        | HtmlElement::ListItem(children)
        | HtmlElement::Blockquote(children)
        | HtmlElement::Div { children, .. }
        | HtmlElement::Span { children, .. } => {
            let ctx_clone = ctx.clone();
            div().flex().flex_row().items_baseline().children(
                children
                    .iter()
                    .map(move |c| render_inline(c, ctx_clone.clone())),
            )
        }

        HtmlElement::UnorderedList(items) | HtmlElement::OrderedList(items) => {
            // Flatten list items inline
            let ctx_clone = ctx.clone();
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .children(items.iter().filter_map(move |item| {
                    if let HtmlElement::ListItem(children) = item {
                        let ctx_inner = ctx_clone.clone();
                        Some(
                            div().flex().flex_row().children(
                                children
                                    .iter()
                                    .map(move |c| render_inline(c, ctx_inner.clone())),
                            ),
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

        // Create link click callback using a weak entity handle
        // This allows us to call back into the DivPrompt to handle submit:value links
        let weak_handle = cx.entity().downgrade();
        let on_link_click: LinkClickCallback = Arc::new(move |href: &str, cx: &mut gpui::App| {
            logging::log("DIV", &format!("Link clicked: {}", href));

            if let Some(value) = href.strip_prefix("submit:") {
                // submit:value links - need entity context to call on_submit
                let value_owned = value.to_string();
                if let Some(entity) = weak_handle.upgrade() {
                    entity.update(cx, |this, _cx| {
                        this.submit_with_value(value_owned);
                    });
                }
            } else if href.starts_with("http://") || href.starts_with("https://") {
                if let Err(e) = open::that(href) {
                    logging::log("DIV", &format!("Failed to open URL {}: {}", href, e));
                }
            } else if href.starts_with("file://") {
                if let Err(e) = open::that(href) {
                    logging::log("DIV", &format!("Failed to open file {}: {}", href, e));
                }
            } else {
                logging::log("DIV", &format!("Unknown link protocol: {}", href));
            }
        });

        // Create render context from theme with link callback
        let render_ctx = if self.design_variant == DesignVariant::Default {
            RenderContext::from_theme(&self.theme.colors).with_link_callback(on_link_click)
        } else {
            RenderContext {
                text_primary: colors.text_primary,
                text_secondary: colors.text_secondary,
                text_tertiary: colors.text_muted, // Use text_muted for tertiary
                accent_color: colors.accent,
                code_bg: colors.background_tertiary, // Use background_tertiary for code bg
                quote_border: colors.border,
                hr_color: colors.border,
                on_link_click: Some(on_link_click),
            }
        };

        // Determine container background:
        // 1. If container_options.background is set, use that
        // 2. Otherwise fall back to design tokens / theme
        let container_bg = if let Some(custom_bg) = self.container_options.parse_background() {
            custom_bg
        } else if self.design_variant == DesignVariant::Default {
            // Apply opacity if specified
            let base_color = self.theme.colors.background.main;
            if let Some(opacity) = self.container_options.opacity {
                rgb_to_hsla(base_color, Some(opacity))
            } else {
                Hsla::from(rgb(base_color))
            }
        } else if let Some(opacity) = self.container_options.opacity {
            rgb_to_hsla(colors.background, Some(opacity))
        } else {
            Hsla::from(rgb(colors.background))
        };

        // Determine container padding
        let container_padding = self.container_options.get_padding(spacing.padding_lg);

        // Generate semantic IDs for div prompt elements
        let panel_semantic_id = format!("panel:content-{}", self.id);

        // Render the HTML elements with any inline Tailwind classes
        let content = render_elements(&elements, render_ctx);

        // Apply root tailwind classes if provided (legacy support)
        let styled_content = if let Some(tw) = &self.tailwind {
            apply_tailwind_styles(content, tw)
        } else {
            content
        };

        // Build the content container with optional containerClasses
        // Apply containerClasses first (before .id() which makes it Stateful for overflow_y_scroll)
        let content_base = div()
            .flex_1() // Grow to fill available space to bottom
            .min_h(px(0.)) // Allow shrinking
            .w_full()
            .child(styled_content);

        let content_styled = if let Some(ref classes) = self.container_options.container_classes {
            apply_tailwind_styles(content_base, classes)
        } else {
            content_base
        };

        // Add ID to make it Stateful, then enable vertical scrolling
        // overflow_y_scroll requires StatefulInteractiveElement trait (needs .id() first)
        let content_container = content_styled
            .id(gpui::ElementId::Name(panel_semantic_id.into()))
            .overflow_y_scroll();

        // Main container - fills entire window height with no bottom gap
        div()
            .id(gpui::ElementId::Name("window:div".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full() // Fill container height completely
            .min_h(px(0.)) // Allow proper flex behavior
            .bg(container_bg)
            .p(px(container_padding))
            .key_context("div_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(content_container)
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

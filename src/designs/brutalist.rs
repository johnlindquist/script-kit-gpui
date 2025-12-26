#![allow(dead_code)]
//! Brutalist Design Renderer
//!
//! A raw, unpolished, intentionally ugly anti-design aesthetic.
//! Features solid black borders, serif fonts (Georgia), harsh colors,
//! NO rounded corners, asymmetric layouts, and visible grid lines.

use gpui::*;

use super::{DesignRenderer, DesignVariant};
use crate::list_item::LIST_ITEM_HEIGHT;
use crate::scripts::SearchResult;

/// Brutalist design colors - harsh, high contrast
mod colors {
    pub const WHITE: u32 = 0xFFFFFF;
    pub const BLACK: u32 = 0x000000;
    pub const RED: u32 = 0xFF0000;
    pub const YELLOW: u32 = 0xFFFF00;
}

/// Border width for brutalist thick borders
const BORDER_WIDTH: f32 = 3.0;

/// Brutalist design renderer
///
/// Implements the DesignRenderer trait with raw, anti-design aesthetics:
/// - Solid black borders (3px+)
/// - Serif font (Georgia)
/// - Harsh colors: red, yellow, black, white
/// - NO rounded corners (all sharp)
/// - Asymmetric layouts
/// - Visible grid lines between items
/// - ALL CAPS headers
/// - Underlined text for clickable items
pub struct BrutalistRenderer;

impl BrutalistRenderer {
    /// Create a new brutalist renderer
    pub fn new() -> Self {
        Self
    }

    /// Render a single list item in brutalist style
    fn render_item(
        &self,
        result: &SearchResult,
        index: usize,
        is_selected: bool,
        is_hovered: bool,
    ) -> impl IntoElement {
        // Determine background color based on state
        let bg_color = if is_selected {
            rgb(colors::YELLOW) // Selected: harsh yellow
        } else if is_hovered {
            rgb(colors::RED) // Hover: aggressive red
        } else {
            rgb(colors::WHITE) // Default: white
        };

        // Text color inverts for hover state
        let text_color = if is_hovered {
            rgb(colors::WHITE) // White text on red background
        } else {
            rgb(colors::BLACK) // Black text otherwise
        };

        // Calculate asymmetric offset based on index for brutalist feel
        let offset = ((index % 3) as f32) * 2.0;

        // Build name + description content
        let mut name_content = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_color)
                    .text_decoration_1() // Underlined - clickable
                    .child(result.name().to_uppercase())
            );
        
        // Add description if present
        if let Some(desc) = result.description() {
            name_content = name_content.child(
                div()
                    .text_sm()
                    .text_color(text_color)
                    .child(desc.to_string())
            );
        }

        // Build the item container with thick black border
        div()
            .id(ElementId::NamedInteger("brutalist-item".into(), index as u64))
            .w_full()
            .h(px(LIST_ITEM_HEIGHT))
            .bg(bg_color)
            .border_color(rgb(colors::BLACK))
            .border(px(BORDER_WIDTH))
            .ml(px(offset)) // Asymmetric left margin
            .flex()
            .flex_row()
            .items_center()
            .px(px(16.))
            .gap(px(12.))
            .font_family("Georgia")
            .child(name_content)
            .child(
                // Type label badge - stark styling
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .bg(rgb(colors::BLACK))
                    .text_color(rgb(colors::WHITE))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child(result.type_label().to_uppercase())
            )
    }
}

impl Default for BrutalistRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App: 'static> DesignRenderer<App> for BrutalistRenderer {
    fn render_script_list(
        &self,
        _app: &App,
        _cx: &mut Context<App>,
    ) -> AnyElement {
        // This is a placeholder implementation
        // The actual rendering requires access to the app's filtered_results and selected_index
        // which should be provided through the App generic parameter
        
        div()
            .w_full()
            .h_full()
            .bg(rgb(colors::WHITE))
            .border_color(rgb(colors::BLACK))
            .border(px(BORDER_WIDTH))
            .flex()
            .flex_col()
            .font_family("Georgia")
            .child(
                // Header - ALL CAPS, stark
                div()
                    .w_full()
                    .h(px(48.))
                    .bg(rgb(colors::BLACK))
                    .text_color(rgb(colors::WHITE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BOLD)
                    .text_lg()
                    .child("SCRIPTS")
            )
            .child(
                // Content area placeholder
                div()
                    .flex_1()
                    .w_full()
                    .p(px(BORDER_WIDTH))
                    .child(
                        div()
                            .text_color(rgb(colors::BLACK))
                            .font_family("Georgia")
                            .child("Brutalist design active. Use with ScriptListApp.")
                    )
            )
            .into_any_element()
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Brutalist
    }
}

/// Render a list of items in brutalist style
/// 
/// This is a helper function that can be used by ScriptListApp to render
/// the filtered results in brutalist style.
pub fn render_brutalist_list(
    results: &[SearchResult],
    selected_index: usize,
    hovered_index: Option<usize>,
) -> impl IntoElement {
    let renderer = BrutalistRenderer::new();
    
    div()
        .w_full()
        .h_full()
        .bg(rgb(colors::WHITE))
        .border_color(rgb(colors::BLACK))
        .border(px(BORDER_WIDTH))
        .flex()
        .flex_col()
        .font_family("Georgia")
        .child(
            // Header - ALL CAPS, stark black background
            div()
                .w_full()
                .h(px(48.))
                .bg(rgb(colors::BLACK))
                .text_color(rgb(colors::WHITE))
                .flex()
                .items_center()
                .justify_center()
                .font_weight(FontWeight::BOLD)
                .text_lg()
                .border_b(px(BORDER_WIDTH))
                .border_color(rgb(colors::BLACK))
                .child("SCRIPTS")
        )
        .child(
            // List container with grid lines (borders between items)
            div()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .flex()
                .flex_col()
                .children(
                    results.iter().enumerate().map(|(index, result)| {
                        let is_selected = index == selected_index;
                        let is_hovered = hovered_index == Some(index);
                        renderer.render_item(result, index, is_selected, is_hovered)
                    })
                )
        )
}

/// Get brutalist colors for external use
pub struct BrutalistColors;

impl BrutalistColors {
    pub const fn background() -> u32 {
        colors::WHITE
    }

    pub const fn text() -> u32 {
        colors::BLACK
    }

    pub const fn border() -> u32 {
        colors::BLACK
    }

    pub const fn selected() -> u32 {
        colors::YELLOW
    }

    pub const fn hover() -> u32 {
        colors::RED
    }

    pub const fn border_width() -> f32 {
        BORDER_WIDTH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brutalist_renderer_variant() {
        let renderer = BrutalistRenderer::new();
        assert_eq!(renderer.variant(), DesignVariant::Brutalist);
    }

    #[test]
    fn test_brutalist_renderer_name() {
        let renderer = BrutalistRenderer::new();
        assert_eq!(renderer.name(), "Brutalist");
    }

    #[test]
    fn test_brutalist_colors() {
        assert_eq!(BrutalistColors::background(), 0xFFFFFF);
        assert_eq!(BrutalistColors::text(), 0x000000);
        assert_eq!(BrutalistColors::border(), 0x000000);
        assert_eq!(BrutalistColors::selected(), 0xFFFF00);
        assert_eq!(BrutalistColors::hover(), 0xFF0000);
        assert_eq!(BrutalistColors::border_width(), 3.0);
    }

    #[test]
    fn test_brutalist_default() {
        let renderer = BrutalistRenderer::default();
        assert_eq!(renderer.variant(), DesignVariant::Brutalist);
    }
}

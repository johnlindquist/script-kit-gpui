#![allow(dead_code)]
//! Apple Human Interface Guidelines (HIG) Design
//!
//! A clean, system-like design following Apple's Human Interface Guidelines.
//! Features iOS-style grouped list cells, SF Pro font, and familiar system colors.
//!
//! # Design Specifications
//! - Background: iOS grouped table view gray (0xf2f2f7)
//! - Card/Group background: Pure white (0xffffff)
//! - Text: System black (0x000000)
//! - Accent: iOS blue (0x007aff)
//! - Separator: iOS light gray (0xc6c6c8)
//! - Touch target height: 44px (iOS standard)

use gpui::*;

use super::{DesignRenderer, DesignVariant};

/// Apple HIG color palette
pub mod colors {
    /// iOS grouped table view background gray
    pub const BACKGROUND: u32 = 0xf2f2f7;
    /// Card/group background white
    pub const CARD_BG: u32 = 0xffffff;
    /// Primary text black
    pub const TEXT_PRIMARY: u32 = 0x000000;
    /// Secondary text gray
    pub const TEXT_SECONDARY: u32 = 0x3c3c43;
    /// Tertiary text gray (60% opacity approximation)
    pub const TEXT_TERTIARY: u32 = 0x8e8e93;
    /// iOS system blue accent
    pub const ACCENT: u32 = 0x007aff;
    /// Separator line gray
    pub const SEPARATOR: u32 = 0xc6c6c8;
    /// Selection background (light blue)
    pub const SELECTION_BG: u32 = 0xe5f0ff;
    /// Search bar background
    pub const SEARCH_BG: u32 = 0xe5e5ea;
}

/// iOS standard touch target height (44pt)
pub const ITEM_HEIGHT: f32 = 44.0;

/// Corner radius for grouped sections
pub const GROUP_RADIUS: f32 = 10.0;

/// Separator inset from left edge (for iOS-style indented separators)
pub const SEPARATOR_INSET: f32 = 16.0;

/// Apple HIG Design Renderer
///
/// Implements the Apple Human Interface Guidelines design language with:
/// - SF Pro font (via .AppleSystemUIFont)
/// - iOS-style grouped list cells with chevron indicators
/// - Grouped rounded sections
/// - Light gray separators between items
/// - Blue accent color
/// - 44px minimum touch targets
/// - Subtle selection states
pub struct AppleHIGRenderer;

impl AppleHIGRenderer {
    /// Create a new Apple HIG renderer
    pub fn new() -> Self {
        Self
    }

    /// Render a single list item in Apple HIG style
    fn render_list_item(
        &self,
        name: SharedString,
        description: Option<String>,
        shortcut: Option<String>,
        is_selected: bool,
        is_first: bool,
        is_last: bool,
    ) -> Div {
        // Selection background or card background
        let bg_color = if is_selected {
            rgb(colors::SELECTION_BG)
        } else {
            rgb(colors::CARD_BG)
        };

        // Build content area with name + description
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden();

        // Primary text - name
        content = content.child(
            div()
                .text_sm()
                .font_weight(FontWeight::NORMAL)
                .text_color(rgb(colors::TEXT_PRIMARY))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(name),
        );

        // Secondary text - description
        if let Some(desc) = description {
            content = content.child(
                div()
                    .text_xs()
                    .text_color(rgb(colors::TEXT_TERTIARY))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(desc),
            );
        }

        // Right side: shortcut badge + chevron
        let mut right_side = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .flex_shrink_0();

        // Shortcut badge if present
        if let Some(sc) = shortcut {
            right_side = right_side.child(
                div()
                    .text_xs()
                    .text_color(rgb(colors::TEXT_TERTIARY))
                    .child(sc),
            );
        }

        // iOS-style chevron indicator
        right_side = right_side.child(
            div()
                .text_color(rgb(colors::TEXT_TERTIARY))
                .text_sm()
                .child(">"),
        );

        // Build the item container
        let mut item = div()
            .w_full()
            .h(px(ITEM_HEIGHT))
            .px(px(16.))
            .bg(bg_color)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .font_family(".AppleSystemUIFont")
            .cursor_pointer()
            .child(content)
            .child(right_side);

        // Apply rounded corners for first/last items in group
        if is_first && is_last {
            item = item.rounded(px(GROUP_RADIUS));
        } else if is_first {
            item = item
                .rounded_tl(px(GROUP_RADIUS))
                .rounded_tr(px(GROUP_RADIUS));
        } else if is_last {
            item = item
                .rounded_bl(px(GROUP_RADIUS))
                .rounded_br(px(GROUP_RADIUS));
        }

        item
    }

    /// Render a separator between list items
    fn render_separator(&self) -> Div {
        div()
            .w_full()
            .h(px(1.))
            .bg(rgb(colors::CARD_BG))
            .child(
                div()
                    .ml(px(SEPARATOR_INSET))
                    .h(px(1.))
                    .bg(rgb(colors::SEPARATOR)),
            )
    }

    /// Render the iOS-style search bar
    fn render_search_bar(&self, filter_text: &str, placeholder: &str) -> Div {
        let display_text = if filter_text.is_empty() {
            placeholder.to_string()
        } else {
            filter_text.to_string()
        };

        let text_color = if filter_text.is_empty() {
            rgb(colors::TEXT_TERTIARY)
        } else {
            rgb(colors::TEXT_PRIMARY)
        };

        div()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .bg(rgb(colors::BACKGROUND))
            .child(
                div()
                    .w_full()
                    .h(px(36.))
                    .px(px(12.))
                    .bg(rgb(colors::SEARCH_BG))
                    .rounded(px(10.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .font_family(".AppleSystemUIFont")
                    // Search icon (magnifying glass)
                    .child(
                        div()
                            .text_color(rgb(colors::TEXT_TERTIARY))
                            .text_sm()
                            .child("🔍"),
                    )
                    // Search text
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(text_color)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(display_text),
                    ),
            )
    }

    /// Render a section header
    fn render_section_header(&self, title: &str) -> Div {
        div()
            .w_full()
            .px(px(32.))
            .py(px(8.))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors::TEXT_TERTIARY))
                    .font_family(".AppleSystemUIFont")
                    .child(title.to_uppercase()),
            )
    }
}

impl Default for AppleHIGRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App> DesignRenderer<App> for AppleHIGRenderer {
    fn render_script_list(&self, _app: &App, _cx: &mut Context<App>) -> AnyElement {
        // This is a placeholder implementation that demonstrates the design
        // The actual integration with ScriptListApp will happen when the
        // design system is fully wired up
        
        let container = div()
            .w_full()
            .h_full()
            .bg(rgb(colors::BACKGROUND))
            .flex()
            .flex_col()
            // Search bar
            .child(self.render_search_bar("", "Search"))
            // Section header
            .child(self.render_section_header("Scripts"))
            // Grouped list container with padding
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .px(px(16.))
                    .flex()
                    .flex_col()
                    // Example items in a rounded group
                    .child(
                        div()
                            .w_full()
                            .bg(rgb(colors::CARD_BG))
                            .rounded(px(GROUP_RADIUS))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(self.render_list_item(
                                "Example Script".into(),
                                Some("A helpful script".to_string()),
                                Some("⌘K".to_string()),
                                true,
                                true,
                                false,
                            ))
                            .child(self.render_separator())
                            .child(self.render_list_item(
                                "Another Script".into(),
                                Some("Does something else".to_string()),
                                None,
                                false,
                                false,
                                false,
                            ))
                            .child(self.render_separator())
                            .child(self.render_list_item(
                                "Third Script".into(),
                                None,
                                Some("⌘T".to_string()),
                                false,
                                false,
                                true,
                            )),
                    ),
            );

        container.into_any_element()
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::AppleHIG
    }

    fn name(&self) -> &'static str {
        "Apple HIG"
    }

    fn description(&self) -> &'static str {
        "Clean Apple design following Human Interface Guidelines"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_hig_colors() {
        // Verify color constants match iOS specs
        assert_eq!(colors::BACKGROUND, 0xf2f2f7);
        assert_eq!(colors::CARD_BG, 0xffffff);
        assert_eq!(colors::ACCENT, 0x007aff);
        assert_eq!(colors::SEPARATOR, 0xc6c6c8);
    }

    #[test]
    fn test_item_height() {
        // iOS touch target minimum is 44pt
        assert_eq!(ITEM_HEIGHT, 44.0);
    }

    #[test]
    fn test_renderer_variant() {
        let renderer = AppleHIGRenderer::new();
        assert_eq!(renderer.variant(), DesignVariant::AppleHIG);
        assert_eq!(renderer.name(), "Apple HIG");
    }

    #[test]
    fn test_renderer_default() {
        let renderer = AppleHIGRenderer::default();
        assert_eq!(renderer.variant(), DesignVariant::AppleHIG);
    }
}

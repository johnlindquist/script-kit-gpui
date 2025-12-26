#![allow(dead_code)]
//! Paper/Skeuomorphic Design
//!
//! A warm, paper-like design with textured shadows and sepia tones.
//! Evokes the feel of handwritten notes and classic stationery.
//!
//! # Design Philosophy
//! - Warm cream/beige backgrounds reminiscent of aged paper
//! - Realistic drop shadows for depth
//! - Sepia/warm tones for text (comfortable reading)
//! - Serif typography for headings
//! - Bookmark/tab indicators for selection (left border accent)
//! - Cards that look like paper notes

use gpui::*;

use super::{DesignRenderer, DesignVariant};
use crate::list_item::LIST_ITEM_HEIGHT;

/// Paper/Skeuomorphic color palette
pub mod colors {
    /// Warm cream background (main window) - #faf8f0
    pub const BACKGROUND_MAIN: u32 = 0xfaf8f0;
    
    /// Slightly warmer white for cards - #fffef8
    pub const CARD_BACKGROUND: u32 = 0xfffef8;
    
    /// Light cream for search box - #f5f3eb
    pub const SEARCH_BACKGROUND: u32 = 0xf5f3eb;
    
    /// Primary text (dark sepia) - #3d3d3d
    pub const TEXT_PRIMARY: u32 = 0x3d3d3d;
    
    /// Secondary text (lighter sepia) - #5a5a5a
    pub const TEXT_SECONDARY: u32 = 0x5a5a5a;
    
    /// Muted text - #8a8a7a
    pub const TEXT_MUTED: u32 = 0x8a8a7a;
    
    /// Dimmed text (very light) - #b0b0a0
    pub const TEXT_DIMMED: u32 = 0xb0b0a0;
    
    /// Accent color (warm tan/gold) - #d4a574
    pub const ACCENT: u32 = 0xd4a574;
    
    /// Subtle accent for hover states - #e8d4be
    pub const ACCENT_SUBTLE: u32 = 0xe8d4be;
    
    /// Border color (light sepia) - #e0dcd0
    pub const BORDER: u32 = 0xe0dcd0;
    
    /// Selected item bookmark color - same as accent
    pub const BOOKMARK: u32 = ACCENT;
}

/// Paper design renderer
///
/// Implements a skeuomorphic, paper-like appearance with:
/// - Warm cream backgrounds
/// - Realistic drop shadows (offset down-right)
/// - Sepia text tones
/// - Bookmark-style left border for selection
/// - Card-like items that resemble paper notes
pub struct PaperRenderer;

impl PaperRenderer {
    /// Create a new Paper renderer
    pub fn new() -> Self {
        Self
    }
    
    /// Create warm-tinted shadow for paper effect
    /// Uses sepia-tinted shadows offset down-right for realistic paper depth
    fn create_card_shadow() -> Vec<BoxShadow> {
        vec![
            // Main shadow: offset down-right, warm sepia tint
            BoxShadow {
                color: hsla(0.1, 0.2, 0.5, 0.25), // Warm brownish shadow
                offset: point(px(2.), px(3.)),
                blur_radius: px(6.),
                spread_radius: px(0.),
            },
            // Subtle ambient shadow
            BoxShadow {
                color: hsla(0.1, 0.15, 0.5, 0.12), // Lighter warm shadow
                offset: point(px(0.), px(1.)),
                blur_radius: px(3.),
                spread_radius: px(0.),
            },
        ]
    }
    
    /// Create inset shadow for search box (paper indent effect)
    fn create_inset_shadow() -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: hsla(0.1, 0.15, 0.4, 0.1), // Very subtle warm shadow
            offset: point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(-1.),
        }]
    }
    
    /// Render a paper-style card with realistic shadow
    fn render_paper_card(&self, content: impl IntoElement) -> impl IntoElement {
        // Outer container with shadow
        div()
            .bg(rgb(colors::CARD_BACKGROUND))
            .rounded(px(4.))
            .shadow(Self::create_card_shadow())
            .child(content)
    }
    
    /// Render a list item in paper style
    fn render_paper_item(
        &self,
        index: usize,
        name: &str,
        description: Option<&str>,
        shortcut: Option<&str>,
        is_selected: bool,
    ) -> impl IntoElement {
        // Selected state uses bookmark-style left border
        let bookmark_width = if is_selected { px(4.) } else { px(0.) };
        let bookmark_color = rgb(colors::BOOKMARK);
        
        // Background varies based on selection
        let bg_color = if is_selected {
            rgba((colors::ACCENT_SUBTLE << 8) | 0x60)
        } else {
            rgba(0x00000000)
        };
        
        // Text colors
        let name_color = if is_selected {
            rgb(colors::TEXT_PRIMARY)
        } else {
            rgb(colors::TEXT_SECONDARY)
        };
        
        let desc_color = if is_selected {
            rgb(colors::ACCENT)
        } else {
            rgb(colors::TEXT_MUTED)
        };
        
        // Build content
        let mut content_col = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(2.));
        
        // Name with serif styling note (would use Georgia font family)
        content_col = content_col.child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(name_color)
                .overflow_hidden()
                .child(name.to_string())
        );
        
        // Description
        if let Some(desc) = description {
            content_col = content_col.child(
                div()
                    .text_xs()
                    .text_color(desc_color)
                    .overflow_hidden()
                    .max_h(px(16.))
                    .child(desc.to_string())
            );
        }
        
        // Shortcut badge (paper-style tag)
        let shortcut_el = if let Some(sc) = shortcut {
            div()
                .text_xs()
                .text_color(rgb(colors::TEXT_DIMMED))
                .px(px(6.))
                .py(px(2.))
                .bg(rgba((colors::BORDER << 8) | 0x60))
                .rounded(px(2.))
                .child(sc.to_string())
        } else {
            div()
        };
        
        // Main item container
        div()
            .id(ElementId::NamedInteger("paper-item".into(), index as u64))
            .w_full()
            .h(px(LIST_ITEM_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            // Bookmark border on left
            .border_l(bookmark_width)
            .border_color(bookmark_color)
            // Background
            .bg(bg_color)
            .hover(|s| s.bg(rgba((colors::ACCENT_SUBTLE << 8) | 0x40)))
            .cursor_pointer()
            // Padding (account for bookmark)
            .pl(px(if is_selected { 12. } else { 16. }))
            .pr(px(16.))
            // Content
            .child(content_col)
            .child(shortcut_el)
    }
    
    /// Render a search box in paper style
    fn render_search_box(&self, placeholder: &str) -> impl IntoElement {
        div()
            .w_full()
            .h(px(44.))
            .px(px(16.))
            .py(px(10.))
            .bg(rgb(colors::SEARCH_BACKGROUND))
            .border_1()
            .border_color(rgb(colors::BORDER))
            .rounded(px(4.))
            .flex()
            .items_center()
            // Inner shadow for inset effect
            .shadow(Self::create_inset_shadow())
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(colors::TEXT_MUTED))
                    .child(placeholder.to_string())
            )
    }
}

impl Default for PaperRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App: 'static> DesignRenderer<App> for PaperRenderer {
    fn render_script_list(
        &self,
        _app: &App,
        _cx: &mut Context<App>,
    ) -> AnyElement {
        // Create a sample paper-style layout
        let search = self.render_search_box("Search scripts...");
        
        // Sample items demonstrating the paper style
        let items = vec![
            ("Welcome Script", Some("Get started with Script Kit"), Some("⌘1"), false),
            ("Open Project", Some("Quickly open projects in VS Code"), Some("⌘P"), true),
            ("Clipboard History", Some("View and manage clipboard"), None, false),
            ("Quick Notes", Some("Jot down ideas"), Some("⌘N"), false),
        ];
        
        let list_items: Vec<_> = items
            .into_iter()
            .enumerate()
            .map(|(i, (name, desc, shortcut, selected))| {
                self.render_paper_item(i, name, desc, shortcut, selected)
            })
            .collect();
        
        let list_container = div()
            .flex_1()
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(2.))
            .py(px(8.))
            .children(list_items);
        
        // Wrap in paper card
        let card = self.render_paper_card(
            div()
                .flex()
                .flex_col()
                .w_full()
                .p(px(4.))
                .child(list_container)
        );
        
        // Main container with paper background
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(colors::BACKGROUND_MAIN))
            .p(px(16.))
            .gap(px(12.))
            .child(search)
            .child(card)
            .into_any_element()
    }
    
    fn variant(&self) -> DesignVariant {
        DesignVariant::Paper
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_paper_colors() {
        // Verify warm palette
        assert_eq!(colors::BACKGROUND_MAIN, 0xfaf8f0);
        assert_eq!(colors::CARD_BACKGROUND, 0xfffef8);
        assert_eq!(colors::TEXT_PRIMARY, 0x3d3d3d);
        assert_eq!(colors::ACCENT, 0xd4a574);
    }
    
    #[test]
    fn test_paper_renderer_variant() {
        let renderer = PaperRenderer::new();
        assert_eq!(renderer.variant(), DesignVariant::Paper);
        assert_eq!(renderer.name(), "Paper");
    }
    
    #[test]
    fn test_paper_renderer_default() {
        let renderer = PaperRenderer::default();
        assert_eq!(renderer.variant(), DesignVariant::Paper);
    }
}

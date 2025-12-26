#![allow(dead_code)]
//! Playful Design Renderer
//!
//! Animated/Playful design with rounded colorful bouncy feel.
//!
//! Design principles:
//! - Rounded everything (24px+ corners)
//! - Playful color palette: coral (#ff6b6b), mint (#4ecdc4), lavender (#a29bfe)
//! - Emoji integration (add emoji before script names based on first letter)
//! - "Bouncy" feel via oversized rounded corners and soft shadows
//! - Thick friendly borders (2px, rounded)
//! - Celebration indicators (sparkle emoji for selected)
//! - Large, friendly text (16px base)
//! - Pill-shaped badges for shortcuts

use gpui::*;

use super::{DesignRenderer, DesignVariant};

/// Height for playful design items (generous for bouncy feel)
const PLAYFUL_ITEM_HEIGHT: f32 = 64.0;

/// Very rounded corners for card feel
const CARD_RADIUS: f32 = 24.0;

/// Smaller radius for inner elements
const ELEMENT_RADIUS: f32 = 16.0;

/// Pill radius for badges
const PILL_RADIUS: f32 = 12.0;

/// Thick friendly border
const BORDER_WIDTH: f32 = 2.0;

/// Horizontal padding
const HORIZONTAL_PADDING: f32 = 20.0;

/// Vertical padding
const VERTICAL_PADDING: f32 = 12.0;

// Playful color palette
const CORAL: u32 = 0xff6b6b;      // Primary - warm, energetic
const MINT: u32 = 0x4ecdc4;       // Secondary - fresh, friendly
const LAVENDER: u32 = 0xa29bfe;   // Accent - soft, playful
const CREAM: u32 = 0xfff9f0;      // Background - warm white
const DARK_CORAL: u32 = 0xe55555; // Darker coral for borders
const SOFT_GRAY: u32 = 0x6c6c6c;  // Muted text
const NEAR_WHITE: u32 = 0xffffff; // Pure white for contrast

/// Pre-computed colors for playful list item rendering
#[derive(Clone, Copy)]
pub struct PlayfulColors {
    pub coral: u32,
    pub mint: u32,
    pub lavender: u32,
    pub cream: u32,
    pub dark_coral: u32,
    pub soft_gray: u32,
    pub white: u32,
}

impl Default for PlayfulColors {
    fn default() -> Self {
        Self {
            coral: CORAL,
            mint: MINT,
            lavender: LAVENDER,
            cream: CREAM,
            dark_coral: DARK_CORAL,
            soft_gray: SOFT_GRAY,
            white: NEAR_WHITE,
        }
    }
}

/// Get a fun emoji based on the first letter of a name
fn get_emoji_for_name(name: &str) -> &'static str {
    match name.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a') => "🎨",
        Some('b') => "🦋",
        Some('c') => "🎪",
        Some('d') => "🎲",
        Some('e') => "✨",
        Some('f') => "🎸",
        Some('g') => "🌈",
        Some('h') => "🌺",
        Some('i') => "💡",
        Some('j') => "🃏",
        Some('k') => "🪁",
        Some('l') => "🍋",
        Some('m') => "🎵",
        Some('n') => "🌙",
        Some('o') => "🍊",
        Some('p') => "🎉",
        Some('q') => "👑",
        Some('r') => "🚀",
        Some('s') => "⭐",
        Some('t') => "🌷",
        Some('u') => "☂️",
        Some('v') => "🎻",
        Some('w') => "🌊",
        Some('x') => "❌",
        Some('y') => "💛",
        Some('z') => "⚡",
        _ => "🎈",
    }
}

/// Playful design renderer
///
/// Provides a fun, colorful UI with:
/// - Very rounded corners (24px+)
/// - Coral, mint, and lavender color palette
/// - Emoji prefixes for script names
/// - Sparkle indicators for selected items
/// - Pill-shaped shortcut badges
pub struct PlayfulRenderer {
    colors: PlayfulColors,
}

impl PlayfulRenderer {
    pub fn new() -> Self {
        Self {
            colors: PlayfulColors::default(),
        }
    }

    /// Render the playful search bar
    fn render_search_bar(&self, filter_text: &str, is_empty: bool) -> Div {
        let colors = self.colors;
        
        let display_text = if is_empty {
            "🔍 Search for something fun...".to_string()
        } else {
            format!("🔍 {}", filter_text)
        };
        
        let text_color = if is_empty {
            rgb(colors.soft_gray)
        } else {
            rgb(colors.dark_coral)
        };
        
        div()
            .w_full()
            .px(px(HORIZONTAL_PADDING))
            .py(px(VERTICAL_PADDING))
            .child(
                div()
                    .w_full()
                    .px(px(20.))
                    .py(px(16.))
                    .bg(rgb(colors.white))
                    .border(px(BORDER_WIDTH))
                    .border_color(rgb(colors.coral))
                    .rounded(px(CARD_RADIUS))
                    .shadow_md()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_color)
                            .child(display_text)
                    )
            )
    }

    /// Render an empty state with playful messaging
    fn render_empty_state(&self, filter_text: &str) -> Div {
        let colors = self.colors;
        
        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .child(
                div()
                    .text_3xl()
                    .child("🎪")
            )
            .child(
                div()
                    .text_color(rgb(colors.soft_gray))
                    .text_lg()
                    .font_weight(FontWeight::MEDIUM)
                    .child(if filter_text.is_empty() {
                        "No scripts yet - let's create some!".to_string()
                    } else {
                        format!("No matches for '{}' - try something else!", filter_text)
                    })
            )
    }

    /// Render a single playful list item
    #[allow(dead_code)]
    pub fn render_list_item(
        &self,
        name: &str,
        description: Option<&str>,
        shortcut: Option<&str>,
        is_selected: bool,
        index: usize,
    ) -> AnyElement {
        let colors = self.colors;
        
        // Get fun emoji for this script
        let emoji = get_emoji_for_name(name);
        
        // Sparkle prefix for selected items
        let display_name = if is_selected {
            format!("✨ {} {}", emoji, name)
        } else {
            format!("{} {}", emoji, name)
        };
        
        // Card colors: selected uses coral bg, others use white
        let (bg_color, text_color, border_color) = if is_selected {
            (rgb(colors.coral), rgb(colors.white), rgb(colors.dark_coral))
        } else {
            (rgb(colors.white), rgb(colors.dark_coral), rgb(colors.mint))
        };
        
        // Description color
        let desc_color = if is_selected {
            rgba(0xffffffcc)  // White with opacity
        } else {
            rgb(colors.soft_gray)
        };
        
        // Build shortcut pill badge if present
        let shortcut_el = if let Some(s) = shortcut {
            div()
                .px(px(12.))
                .py(px(4.))
                .bg(rgb(colors.lavender))
                .rounded(px(PILL_RADIUS))
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(colors.white))
                .child(s.to_string())
        } else {
            div()
        };
        
        // Build description element if present
        let desc_el = if let Some(desc) = description {
            div()
                .text_sm()
                .text_color(desc_color)
                .child(desc.to_string())
        } else {
            div()
        };
        
        // Build card item
        div()
            .id(ElementId::NamedInteger("playful-item".into(), index as u64))
            .w_full()
            .px(px(HORIZONTAL_PADDING))
            .py(px(6.))
            .child(
                div()
                    .w_full()
                    .h(px(PLAYFUL_ITEM_HEIGHT - 12.))
                    .px(px(16.))
                    .bg(bg_color)
                    .border(px(BORDER_WIDTH))
                    .border_color(border_color)
                    .rounded(px(ELEMENT_RADIUS))
                    .shadow_sm()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .hover(|s| s.shadow_md())
                    .child(
                        // Left side: name and description
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_color)
                                    .child(display_name)
                            )
                            .child(desc_el)
                    )
                    .child(shortcut_el)
            )
            .into_any_element()
    }
}

impl Default for PlayfulRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App: 'static> DesignRenderer<App> for PlayfulRenderer {
    fn render_script_list(
        &self,
        _app: &App,
        _cx: &mut Context<App>,
    ) -> AnyElement {
        // Note: This is a standalone renderer that doesn't have access to app state.
        // In a real integration, we would need to pass the filtered scripts,
        // selected index, and filter text through the trait method.
        //
        // For now, this demonstrates the visual structure.
        // Full integration requires modifying the trait to pass necessary data.
        
        let colors = self.colors;
        
        // Main container - warm cream background with bouncy card feel
        div()
            .w_full()
            .h_full()
            .bg(rgb(colors.cream))
            .flex()
            .flex_col()
            // Header with playful title
            .child(
                div()
                    .w_full()
                    .px(px(HORIZONTAL_PADDING))
                    .pt(px(16.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_2xl()
                                    .child("🎉")
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(colors.coral))
                                    .child("Script Kit")
                            )
                    )
                    .child(
                        // Item count badge
                        div()
                            .px(px(12.))
                            .py(px(4.))
                            .bg(rgb(colors.mint))
                            .rounded(px(PILL_RADIUS))
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(colors.white))
                            .child("Ready to play!")
                    )
            )
            .child(self.render_search_bar("", true))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.render_empty_state(""))
            )
            .into_any_element()
    }
    
    fn variant(&self) -> DesignVariant {
        DesignVariant::Playful
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_playful_renderer_variant() {
        let renderer = PlayfulRenderer::new();
        assert_eq!(renderer.variant(), DesignVariant::Playful);
    }
    
    #[test]
    fn test_playful_renderer_name() {
        let renderer = PlayfulRenderer::new();
        assert_eq!(renderer.name(), "Playful");
    }
    
    #[test]
    fn test_playful_renderer_description() {
        let renderer = PlayfulRenderer::new();
        assert!(!renderer.description().is_empty());
    }
    
    #[test]
    fn test_emoji_for_name() {
        assert_eq!(get_emoji_for_name("apple"), "🎨");
        assert_eq!(get_emoji_for_name("Banana"), "🦋");
        assert_eq!(get_emoji_for_name("script"), "⭐");
        assert_eq!(get_emoji_for_name(""), "🎈");
        assert_eq!(get_emoji_for_name("123"), "🎈");
    }
    
    #[test]
    fn test_card_radius_is_large() {
        // Playful design uses very rounded corners
        assert!(CARD_RADIUS >= 24.0);
    }
    
    #[test]
    fn test_colors_are_correct() {
        let colors = PlayfulColors::default();
        assert_eq!(colors.coral, 0xff6b6b);
        assert_eq!(colors.mint, 0x4ecdc4);
        assert_eq!(colors.lavender, 0xa29bfe);
    }
}

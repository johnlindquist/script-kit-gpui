#![allow(dead_code)]
//! Neon Cyberpunk Design
//!
//! A vibrant, futuristic design with bright neon colors against a deep dark background.
//! Features heavy glow effects, gradient-like borders, and a futuristic monospace font.
//!
//! Color Palette:
//! - Background: Deep purple/black (#0a0015)
//! - Primary (Cyan): #00ffff
//! - Secondary (Magenta): #ff00ff
//! - Accent (Yellow): #ffff00
//!
//! Design Features:
//! - Multiple box-shadows for neon glow effects
//! - Text with neon glow appearance
//! - Selected items: bright magenta glow
//! - Hover: cyan glow
//! - Search input glows cyan
//! - Futuristic monospace font (Menlo)

use gpui::*;

use super::{DesignRenderer, DesignVariant};

/// Fixed height for list items
pub const LIST_ITEM_HEIGHT: f32 = 52.0;

/// Neon Cyberpunk color palette
pub mod colors {
    /// Deep purple/black background
    pub const BACKGROUND: u32 = 0x0a0015;
    /// Slightly lighter panel background
    pub const BACKGROUND_PANEL: u32 = 0x120024;
    /// Background for selected items
    pub const BACKGROUND_SELECTED: u32 = 0x1a003a;
    /// Background on hover
    pub const BACKGROUND_HOVER: u32 = 0x150030;
    
    /// Primary cyan neon color
    pub const CYAN: u32 = 0x00ffff;
    /// Secondary magenta neon color
    pub const MAGENTA: u32 = 0xff00ff;
    /// Accent yellow neon color
    pub const YELLOW: u32 = 0xffff00;
    /// Dimmed cyan for secondary text
    pub const CYAN_DIM: u32 = 0x00aaaa;
    /// Dimmed magenta for muted elements
    pub const MAGENTA_DIM: u32 = 0xaa00aa;
    
    /// Border color (dim cyan)
    pub const BORDER: u32 = 0x0066aa;
    /// Active border (bright cyan)
    pub const BORDER_ACTIVE: u32 = 0x00ccff;
}

/// Pre-computed colors for NeonCyberpunk list item rendering
#[derive(Clone, Copy)]
pub struct NeonListItemColors {
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub accent_selected: u32,
    pub background: u32,
    pub background_selected: u32,
    pub background_hover: u32,
}

impl Default for NeonListItemColors {
    fn default() -> Self {
        Self {
            text_primary: colors::CYAN,
            text_secondary: colors::CYAN_DIM,
            text_muted: colors::MAGENTA_DIM,
            accent_selected: colors::MAGENTA,
            background: colors::BACKGROUND,
            background_selected: colors::BACKGROUND_SELECTED,
            background_hover: colors::BACKGROUND_HOVER,
        }
    }
}

/// Neon Cyberpunk design renderer
///
/// Implements the DesignRenderer trait with a futuristic cyberpunk aesthetic.
/// Features bright neon colors, glow effects, and a dark atmospheric background.
pub struct NeonCyberpunkRenderer;

impl NeonCyberpunkRenderer {
    /// Create a new NeonCyberpunkRenderer
    pub fn new() -> Self {
        Self
    }
    
    /// Get the neon list item colors
    pub fn list_item_colors() -> NeonListItemColors {
        NeonListItemColors::default()
    }
    
    /// Create a glow effect box shadow (cyan glow)
    fn cyan_glow() -> BoxShadow {
        BoxShadow {
            color: hsla(180.0 / 360.0, 1.0, 0.5, 0.6),
            offset: Point { x: px(0.), y: px(0.) },
            blur_radius: px(12.),
            spread_radius: px(2.),
        }
    }
    
    /// Create a glow effect box shadow (magenta glow)
    fn magenta_glow() -> BoxShadow {
        BoxShadow {
            color: hsla(300.0 / 360.0, 1.0, 0.5, 0.7),
            offset: Point { x: px(0.), y: px(0.) },
            blur_radius: px(16.),
            spread_radius: px(4.),
        }
    }
    
    /// Create a subtle inner glow for text areas
    fn inner_glow() -> BoxShadow {
        BoxShadow {
            color: hsla(180.0 / 360.0, 1.0, 0.5, 0.3),
            offset: Point { x: px(0.), y: px(0.) },
            blur_radius: px(8.),
            spread_radius: px(0.),
        }
    }
    
    /// Render the search input with cyan glow
    fn render_search_input(&self, filter_text: &str, filter_is_empty: bool) -> impl IntoElement {
        let display_text = if filter_is_empty {
            "Search scripts..."
        } else {
            filter_text
        };
        
        div()
            .w_full()
            .px(px(16.))
            .py(px(12.))
            .bg(rgb(colors::BACKGROUND_PANEL))
            .border_1()
            .border_color(rgb(colors::BORDER_ACTIVE))
            .rounded(px(4.))
            .shadow(vec![Self::cyan_glow(), Self::inner_glow()])
            .child(
                div()
                    .text_color(if filter_is_empty {
                        rgb(colors::CYAN_DIM)
                    } else {
                        rgb(colors::CYAN)
                    })
                    .font_family("Menlo")
                    .text_sm()
                    .child(display_text.to_string())
            )
    }
    
    /// Render a single list item with neon styling
    fn render_list_item(
        &self,
        name: &str,
        description: Option<&str>,
        shortcut: Option<&str>,
        is_selected: bool,
        index: usize,
    ) -> impl IntoElement {
        let colors = Self::list_item_colors();
        
        // Background color based on selection state
        let bg_color = if is_selected {
            rgb(colors.background_selected)
        } else {
            rgba(0x00000000)
        };
        
        // Text colors based on selection
        let name_color = if is_selected {
            rgb(colors::MAGENTA)
        } else {
            rgb(colors::CYAN)
        };
        
        let desc_color = if is_selected {
            rgb(colors::YELLOW)
        } else {
            rgb(colors::CYAN_DIM)
        };
        
        // Build description element
        let description_element = if let Some(desc) = description {
            div()
                .text_xs()
                .text_color(desc_color)
                .overflow_hidden()
                .max_h(px(16.))
                .child(desc.to_string())
        } else {
            div()
        };
        
        // Build shortcut badge
        let shortcut_element = if let Some(sc) = shortcut {
            div()
                .text_xs()
                .text_color(rgb(colors::YELLOW))
                .px(px(8.))
                .py(px(2.))
                .bg(rgba(0xffff0020))
                .rounded(px(4.))
                .child(sc.to_string())
        } else {
            div()
        };
        
        // Main item container
        let mut container = div()
            .id(ElementId::NamedInteger("neon-item".into(), index as u64))
            .w_full()
            .h(px(LIST_ITEM_HEIGHT))
            .px(px(12.))
            .flex()
            .items_center();
        
        // Inner content with styling
        let mut inner = div()
            .w_full()
            .h_full()
            .px(px(12.))
            .bg(bg_color)
            .rounded(px(4.))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2();
        
        // Apply glow effects based on selection
        if is_selected {
            inner = inner
                .border_1()
                .border_color(rgb(colors::MAGENTA))
                .shadow(vec![Self::magenta_glow()]);
        } else {
            inner = inner
                .border_1()
                .border_color(rgba(0x00ffff20))
                .hover(|s| s
                    .bg(rgb(colors.background_hover))
                    .border_color(rgb(colors::CYAN))
                    .shadow(vec![Self::cyan_glow()])
                );
        }
        
        // Text content
        let text_content = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .font_family("Menlo")
                    .text_color(name_color)
                    .overflow_hidden()
                    .child(name.to_string())
            )
            .child(description_element);
        
        inner = inner
            .child(text_content)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .child(shortcut_element)
            );
        
        container = container.child(inner);
        container
    }
    
    /// Render the header with design name
    fn render_header(&self) -> impl IntoElement {
        div()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .font_family("Menlo")
                    .text_color(rgb(colors::MAGENTA_DIM))
                    .child("// NEON CYBERPUNK")
            )
            .child(
                div()
                    .text_xs()
                    .font_family("Menlo")
                    .text_color(rgb(colors::YELLOW))
                    .child("CMD+6")
            )
    }
    
    /// Render the status bar
    fn render_status_bar(&self, total_items: usize, filtered_items: usize) -> impl IntoElement {
        let status_text = if total_items == filtered_items {
            format!("{} scripts", total_items)
        } else {
            format!("{}/{} scripts", filtered_items, total_items)
        };
        
        div()
            .w_full()
            .px(px(16.))
            .py(px(8.))
            .border_t_1()
            .border_color(rgb(colors::BORDER))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .font_family("Menlo")
                    .text_color(rgb(colors::CYAN_DIM))
                    .child(status_text)
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_family("Menlo")
                            .text_color(rgb(colors::MAGENTA_DIM))
                            .child("↑↓ navigate")
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("Menlo")
                            .text_color(rgb(colors::CYAN_DIM))
                            .child("⏎ select")
                    )
            )
    }
}

impl Default for NeonCyberpunkRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App> DesignRenderer<App> for NeonCyberpunkRenderer {
    fn render_script_list(
        &self,
        _app: &App,
        _cx: &mut Context<App>,
    ) -> AnyElement {
        // This is a stub implementation - the actual integration with ScriptListApp
        // will be done when the design system is fully connected.
        // For now, return a placeholder that shows the design is available.
        
        div()
            .w_full()
            .h_full()
            .bg(rgb(colors::BACKGROUND))
            .flex()
            .flex_col()
            .child(self.render_header())
            .child(
                div()
                    .p(px(16.))
                    .child(self.render_search_input("", true))
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(rgb(colors::CYAN))
                            .font_family("Menlo")
                            .child("Neon Cyberpunk Design Active")
                    )
                    .child(
                        div()
                            .text_color(rgb(colors::MAGENTA_DIM))
                            .font_family("Menlo")
                            .text_xs()
                            .mt_2()
                            .child("Integration pending with ScriptListApp")
                    )
            )
            .child(self.render_status_bar(0, 0))
            .into_any_element()
    }
    
    fn variant(&self) -> DesignVariant {
        DesignVariant::NeonCyberpunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_renderer_creation() {
        let renderer = NeonCyberpunkRenderer::new();
        assert_eq!(renderer.variant(), DesignVariant::NeonCyberpunk);
    }
    
    #[test]
    fn test_default_colors() {
        let colors = NeonListItemColors::default();
        assert_eq!(colors.text_primary, 0x00ffff);
        assert_eq!(colors.accent_selected, 0xff00ff);
    }
    
    #[test]
    fn test_variant_name() {
        let renderer = NeonCyberpunkRenderer::new();
        assert_eq!(renderer.name(), "Neon Cyberpunk");
    }
    
    #[test]
    fn test_variant_description() {
        let renderer = NeonCyberpunkRenderer::new();
        assert_eq!(renderer.description(), "Dark backgrounds with vibrant neon accents");
    }
}

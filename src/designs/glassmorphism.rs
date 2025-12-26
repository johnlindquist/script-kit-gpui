#![allow(dead_code)]
//! Glassmorphism Design Renderer
//!
//! Implements a frosted glass aesthetic with translucent backgrounds,
//! soft white borders, layered panels, and gentle shadows.
//!
//! Design characteristics:
//! - Heavy use of transparency (0.3-0.6 alpha backgrounds)
//! - White/light borders (1px, 0xFFFFFF with 0.2 alpha)
//! - Layered frosted panels effect
//! - Light text on translucent backgrounds
//! - Rounded corners everywhere (16px+)
//! - Soft drop shadows with low opacity
//!
//! Note: This design relies on WindowBackgroundAppearance::Blurred
//! being set at the app level for the true frosted glass effect.

use gpui::*;

use super::{DesignRenderer, DesignVariant};

/// Fixed height for list items
const LIST_ITEM_HEIGHT: f32 = 52.0;

/// Pre-computed colors for glassmorphism rendering
#[derive(Clone, Copy)]
pub struct GlassColors {
    /// Main background with transparency (white at ~25% opacity)
    pub background_main: u32,
    /// Card/panel background (white at ~19% opacity)
    pub card_bg: u32,
    /// Selected item background (white at ~31% opacity)
    pub selected_bg: u32,
    /// Hover state background (white at ~25% opacity)
    pub hover_bg: u32,
    /// Border color (white at ~20% opacity)
    pub border: u32,
    /// Primary text (pure white)
    pub text_primary: u32,
    /// Secondary text (white at ~80% opacity)
    pub text_secondary: u32,
    /// Muted text (white at ~60% opacity)
    pub text_muted: u32,
    /// Accent/highlight color (soft blue-white)
    pub accent: u32,
}

impl Default for GlassColors {
    fn default() -> Self {
        Self {
            // Backgrounds with alpha encoded in lower 8 bits
            // Format: 0xRRGGBBAA
            background_main: 0xffffff40, // white @ 25% (~0.25 * 255 = 64 = 0x40)
            card_bg: 0xffffff30,          // white @ 19% (~0.19 * 255 = 48 = 0x30)
            selected_bg: 0xffffff50,      // white @ 31% (~0.31 * 255 = 80 = 0x50)
            hover_bg: 0xffffff40,         // white @ 25%
            border: 0xffffff33,           // white @ 20%
            // Text colors (solid)
            text_primary: 0xffffff,       // pure white
            text_secondary: 0xffffffcc,   // white @ 80%
            text_muted: 0xffffff99,       // white @ 60%
            accent: 0xa8d8ff,             // soft blue-white
        }
    }
}

/// Glassmorphism design renderer
///
/// Creates a frosted glass aesthetic with translucent panels,
/// soft white borders, and gentle depth through layered transparency.
pub struct GlassmorphismRenderer {
    colors: GlassColors,
}

impl GlassmorphismRenderer {
    /// Create a new glassmorphism renderer with default colors
    pub fn new() -> Self {
        Self {
            colors: GlassColors::default(),
        }
    }

    /// Create with custom colors
    #[allow(dead_code)]
    pub fn with_colors(colors: GlassColors) -> Self {
        Self { colors }
    }

    /// Render a glass card container with rounded corners and subtle border
    fn glass_card(&self) -> Div {
        div()
            .bg(rgba(self.colors.card_bg))
            .border_1()
            .border_color(rgba(self.colors.border))
            .rounded(px(16.))
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.1),
                offset: point(px(0.), px(4.)),
                blur_radius: px(12.),
                spread_radius: px(0.),
            }])
    }

    /// Render a single list item in glassmorphism style
    ///
    /// Returns an AnyElement for use in uniform_list or other containers.
    #[allow(dead_code)]
    fn render_list_item(
        &self,
        name: String,
        description: Option<String>,
        shortcut: Option<String>,
        is_selected: bool,
        index: usize,
    ) -> AnyElement {
        let colors = self.colors;

        // Selected items get brighter glass effect with subtle glow
        let item_bg = if is_selected {
            rgba(colors.selected_bg)
        } else {
            rgba(0x00000000) // transparent, let card show through
        };

        let text_color = if is_selected {
            rgb(colors.text_primary)
        } else {
            rgb(colors.text_secondary)
        };

        let desc_color = if is_selected {
            rgb(colors.accent)
        } else {
            rgba(colors.text_muted)
        };

        // Build content column (name + description)
        let mut content = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(2.));

        // Name
        content = content.child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_color)
                .overflow_hidden()
                .child(name),
        );

        // Description
        if let Some(desc) = description {
            content = content.child(
                div()
                    .text_xs()
                    .text_color(desc_color)
                    .overflow_hidden()
                    .max_h(px(16.))
                    .child(desc),
            );
        }

        // Shortcut badge
        let shortcut_el = if let Some(sc) = shortcut {
            div()
                .text_xs()
                .text_color(rgba(colors.text_muted))
                .bg(rgba(0xffffff15)) // very subtle background
                .px(px(8.))
                .py(px(2.))
                .rounded(px(8.))
                .child(sc)
        } else {
            div()
        };

        // Glass card wrapper for each item - use shadow vec directly in builder chain
        let shadow_vec = if is_selected {
            vec![BoxShadow {
                color: hsla(0., 0., 1., 0.15), // white glow
                offset: point(px(0.), px(0.)),
                blur_radius: px(16.),
                spread_radius: px(2.),
            }]
        } else {
            vec![]
        };

        div()
            .id(ElementId::NamedInteger("glass-item".into(), index as u64))
            .mx(px(8.)) // horizontal margin for card spacing
            .mb(px(4.)) // bottom margin between cards
            .bg(item_bg)
            .rounded(px(12.))
            .border_1()
            .border_color(if is_selected {
                rgba(0xffffff40) // brighter border when selected
            } else {
                rgba(0xffffff15) // subtle border normally
            })
            .shadow(shadow_vec)
            .px(px(16.))
            .py(px(8.))
            .h(px(LIST_ITEM_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .cursor_pointer()
            .font_family(".AppleSystemUIFont")
            .hover(|s| s.bg(rgba(colors.hover_bg)))
            .child(content)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .child(shortcut_el),
            )
            .into_any_element()
    }

    /// Render floating search bar in glass style
    fn render_search_bar(&self, filter_text: &str, is_empty: bool) -> Div {
        let display_text = if is_empty {
            "Type to search...".to_string()
        } else {
            filter_text.to_string()
        };

        let text_color = if is_empty {
            rgba(self.colors.text_muted)
        } else {
            rgb(self.colors.text_primary)
        };

        // Floating glass search card
        self.glass_card()
            .mx(px(16.))
            .mt(px(16.))
            .mb(px(8.))
            .px(px(20.))
            .py(px(14.))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                // Search icon placeholder (magnifying glass)
                div()
                    .text_color(rgba(self.colors.text_muted))
                    .text_sm()
                    .child("⌕"),
            )
            .child(
                div()
                    .flex_1()
                    .text_base()
                    .text_color(text_color)
                    .font_family(".AppleSystemUIFont")
                    .child(display_text),
            )
    }

    /// Render empty state
    fn render_empty_state(&self, filter_text: &str) -> Div {
        let message = if filter_text.is_empty() {
            "No scripts or snippets found".to_string()
        } else {
            format!("No results match '{}'", filter_text)
        };

        div()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgba(self.colors.text_muted))
            .font_family(".AppleSystemUIFont")
            .text_base()
            .child(message)
    }
}

impl Default for GlassmorphismRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App: 'static> DesignRenderer<App> for GlassmorphismRenderer {
    fn render_script_list(&self, _app: &App, _cx: &mut Context<App>) -> AnyElement {
        // Note: This is a standalone renderer that doesn't have access to app state.
        // In a real integration, we would need to pass the filtered scripts,
        // selected index, and filter text through the trait method.
        //
        // For now, this demonstrates the visual structure.
        // Full integration requires modifying the trait to pass necessary data.

        let colors = self.colors;

        // Main container with glass background
        div()
            .w_full()
            .h_full()
            .bg(rgba(colors.background_main))
            .flex()
            .flex_col()
            // Search bar (floating glass card)
            .child(self.render_search_bar("", true))
            // Content area with layered glass panels
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .px(px(8.))
                    .py(px(4.))
                    // Inner glass panel for depth
                    .child(
                        self.glass_card()
                            .w_full()
                            .h_full()
                            .flex()
                            .flex_col()
                            .p(px(4.))
                            .child(self.render_empty_state("")),
                    ),
            )
            .into_any_element()
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Glassmorphism
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glass_colors_default() {
        let colors = GlassColors::default();
        // Verify key color values
        assert_eq!(colors.text_primary, 0xffffff);
        assert_eq!(colors.background_main, 0xffffff40);
        assert_eq!(colors.card_bg, 0xffffff30);
    }

    #[test]
    fn test_renderer_creation() {
        let renderer = GlassmorphismRenderer::new();
        assert_eq!(renderer.variant(), DesignVariant::Glassmorphism);
        assert_eq!(renderer.name(), "Glassmorphism");
    }

    #[test]
    fn test_renderer_with_custom_colors() {
        let custom = GlassColors {
            background_main: 0x000000ff,
            ..Default::default()
        };
        let renderer = GlassmorphismRenderer::with_colors(custom);
        assert_eq!(renderer.colors.background_main, 0x000000ff);
    }
}

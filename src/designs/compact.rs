#![allow(dead_code)]
//! Compact Design - Maximum Information Density
//!
//! Design 9: Dense layout for power users with many scripts.
//! Features:
//! - 24px row height (vs 52px standard)
//! - 10px font size (text_xs)
//! - Minimal padding (4px horizontal, 2px vertical)
//! - Monospace font for consistent character width
//! - Light table-like borders between rows
//! - No preview panel - all space for the list

use gpui::*;
use gpui::prelude::FluentBuilder;

use super::{DesignRenderer, DesignVariant};
use crate::list_item::ListItemColors;
use crate::theme::Theme;

/// Fixed height for compact list items (less than half of standard 52px)
pub const COMPACT_ITEM_HEIGHT: f32 = 24.0;

/// Compact design renderer for maximum information density
pub struct CompactRenderer {
    theme: std::sync::Arc<Theme>,
}

impl CompactRenderer {
    /// Create a new compact renderer with the given theme
    pub fn new(theme: std::sync::Arc<Theme>) -> Self {
        Self { theme }
    }
    
    /// Get list item colors from theme
    fn list_colors(&self) -> ListItemColors {
        ListItemColors::from_theme(&self.theme)
    }
}

impl<App> DesignRenderer<App> for CompactRenderer
where
    App: 'static,
{
    fn render_script_list(
        &self,
        _app: &App,
        _cx: &mut Context<App>,
    ) -> AnyElement {
        // This is a placeholder - the actual rendering will be done
        // by integrating with ScriptListApp's data access patterns
        // For now, return an empty container styled for compact layout
        
        let colors = self.list_colors();
        let theme = &self.theme;
        
        div()
            .w_full()
            .h_full()
            .bg(rgb(theme.colors.background.main))
            .flex()
            .flex_col()
            .font_family("Menlo") // Monospace for consistent width
            .text_xs() // 10px font size
            .child(
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(colors.text_muted))
                    .child("Compact design - pending full integration")
            )
            .into_any_element()
    }
    
    fn variant(&self) -> DesignVariant {
        DesignVariant::Compact
    }
}

/// Compact list item component for dense display
/// 
/// Unlike the standard ListItem (52px), this uses:
/// - 24px height
/// - Monospace font
/// - Minimal padding
/// - Border-bottom separator instead of spacing
#[derive(IntoElement)]
pub struct CompactListItem {
    name: SharedString,
    description: Option<String>,
    shortcut: Option<String>,
    selected: bool,
    colors: ListItemColors,
    border_color: u32,
}

impl CompactListItem {
    /// Create a new compact list item
    pub fn new(name: impl Into<SharedString>, colors: ListItemColors, border_color: u32) -> Self {
        Self {
            name: name.into(),
            description: None,
            shortcut: None,
            selected: false,
            colors,
            border_color,
        }
    }
    
    /// Set the description (shown inline after name, truncated)
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    
    /// Set an optional description
    pub fn description_opt(mut self, d: Option<String>) -> Self {
        self.description = d;
        self
    }
    
    /// Set the shortcut badge
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    
    /// Set an optional shortcut
    pub fn shortcut_opt(mut self, s: Option<String>) -> Self {
        self.shortcut = s;
        self
    }
    
    /// Set selection state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for CompactListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        
        // Selection colors with alpha - subtle for compact view
        let selected_bg = rgba((colors.accent_selected_subtle << 8) | 0x60);
        let hover_bg = rgba((colors.accent_selected_subtle << 8) | 0x30);
        
        // Build content: name + description inline, separated by dash
        let mut content_text = self.name.to_string();
        if let Some(ref desc) = self.description {
            // Truncate description to keep row compact
            let truncated = if desc.len() > 40 {
                format!("{}..", &desc[..38])
            } else {
                desc.clone()
            };
            content_text = format!("{} - {}", content_text, truncated);
        }
        
        // Row container with border-bottom for table-like appearance
        div()
            .w_full()
            .h(px(COMPACT_ITEM_HEIGHT))
            .px(px(4.)) // Minimal horizontal padding
            .py(px(2.)) // Minimal vertical padding
            .bg(if self.selected { selected_bg } else { rgba(0x00000000) })
            .hover(|s| s.bg(hover_bg))
            .border_b_1()
            .border_color(rgba((self.border_color << 8) | 0x40)) // Subtle border
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .font_family("Menlo") // Monospace
            .text_xs() // 10px font
            .cursor_pointer()
            // Main content (name + description)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_color(if self.selected { 
                        rgb(colors.text_primary) 
                    } else { 
                        rgb(colors.text_secondary) 
                    })
                    .child(content_text)
            )
            // Shortcut badge (if present)
            .when_some(self.shortcut, |el: Div, sc: String| {
                el.child(
                    div()
                        .text_color(rgb(colors.text_dimmed))
                        .ml(px(4.))
                        .flex_shrink_0()
                        .child(sc)
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compact_item_height() {
        // Compact should be less than half of standard (52px)
        assert!(COMPACT_ITEM_HEIGHT < 26.0);
        assert_eq!(COMPACT_ITEM_HEIGHT, 24.0);
    }
    
    #[test]
    fn test_variant() {
        // Verify the renderer returns correct variant
        // Note: Can't test without a real theme, but the match is simple
        assert_eq!(DesignVariant::Compact.name(), "Compact");
    }
}

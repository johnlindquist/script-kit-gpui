#![allow(dead_code)]
//! Minimal Design Renderer
//!
//! Ultra minimalist design with maximum whitespace and NO visual noise.
//!
//! Design principles:
//! - Maximum whitespace with generous padding (80px horizontal, 24px vertical)
//! - Thin sans-serif typography (.AppleSystemUIFont)
//! - NO borders anywhere
//! - Subtle hover states (slight opacity change only)
//! - Monochrome palette with single accent color
//! - Full-width list (no preview panel)
//! - Search bar is just cursor + typed text, no box
//! - Items show name only (no description)
//! - Taller items (64px instead of 52px)

use gpui::*;

use super::{DesignRenderer, DesignVariant};
use crate::scripts::SearchResult;
use crate::theme::Theme;

/// Height for minimal design items (taller than default 52px)
pub const MINIMAL_ITEM_HEIGHT: f32 = 64.0;

/// Horizontal padding for list items
pub const HORIZONTAL_PADDING: f32 = 80.0;

/// Vertical padding for list items
pub const VERTICAL_PADDING: f32 = 24.0;

/// Pre-computed colors for minimal list item rendering
#[derive(Clone, Copy)]
pub struct MinimalColors {
    pub text_primary: u32,
    pub text_muted: u32,
    pub accent_selected: u32,
    pub background: u32,
}

impl MinimalColors {
    /// Create from theme reference
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            text_primary: theme.colors.text.primary,
            text_muted: theme.colors.text.muted,
            accent_selected: theme.colors.accent.selected,
            background: theme.colors.background.main,
        }
    }
}

/// Minimal design renderer
///
/// Provides an ultra-clean, whitespace-focused UI with:
/// - No borders or dividers
/// - Simple text-only list items
/// - Accent color for selected items
/// - Generous padding throughout
pub struct MinimalRenderer;

impl MinimalRenderer {
    /// Create a new minimal renderer
    pub fn new() -> Self {
        Self
    }

    /// Render a single list item in minimal style
    pub fn render_item(
        &self,
        result: &SearchResult,
        index: usize,
        is_selected: bool,
        colors: MinimalColors,
    ) -> impl IntoElement {
        // Get name only (no description in minimal design)
        let name = result.name().to_string();

        // Text color: accent when selected, primary otherwise
        let text_color = if is_selected {
            rgb(colors.accent_selected)
        } else {
            rgb(colors.text_primary)
        };

        // Font weight: normal when selected, thin otherwise
        let font_weight = if is_selected {
            FontWeight::NORMAL
        } else {
            FontWeight::THIN
        };

        div()
            .id(ElementId::NamedInteger("minimal-item".into(), index as u64))
            .w_full()
            .h(px(MINIMAL_ITEM_HEIGHT))
            .px(px(HORIZONTAL_PADDING))
            .py(px(VERTICAL_PADDING / 2.0))
            .flex()
            .items_center()
            .font_family(".AppleSystemUIFont")
            .font_weight(font_weight)
            .text_base()
            .text_color(text_color)
            .cursor_pointer()
            // Subtle hover: just slightly brighter
            .hover(|s| s.opacity(0.8))
            .child(name)
    }
}

impl Default for MinimalRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl<App: 'static> DesignRenderer<App> for MinimalRenderer {
    fn render_script_list(
        &self,
        _app: &App,
        _cx: &mut Context<App>,
    ) -> AnyElement {
        // This is a placeholder implementation
        // The actual rendering is done via render_minimal_list() helper
        // which should be called from ScriptListApp with the actual data
        div()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .font_family(".AppleSystemUIFont")
            .font_weight(FontWeight::THIN)
            .text_lg()
            .child("Minimal design active. Use with ScriptListApp.")
            .into_any_element()
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Minimal
    }
}

/// Render the minimal search bar
///
/// Just shows typed text (or placeholder) with a blinking cursor.
/// No box, no border - pure minimal.
pub fn render_minimal_search_bar(
    filter_text: &str,
    cursor_visible: bool,
    colors: MinimalColors,
) -> impl IntoElement {
    let display_text = if filter_text.is_empty() {
        "Type to search..."
    } else {
        filter_text
    };

    let text_color = if filter_text.is_empty() {
        rgb(colors.text_muted)
    } else {
        rgb(colors.text_primary)
    };

    // Blinking cursor after text
    let cursor = if cursor_visible {
        div()
            .w(px(2.))
            .h(px(20.))
            .bg(rgb(colors.accent_selected))
            .ml(px(2.))
    } else {
        div()
            .w(px(2.))
            .h(px(20.))
            .ml(px(2.))
    };

    div()
        .w_full()
        .py(px(VERTICAL_PADDING))
        .px(px(HORIZONTAL_PADDING))
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::THIN)
                .font_family(".AppleSystemUIFont")
                .text_color(text_color)
                .child(display_text.to_string())
        )
        .child(cursor)
}

/// Render the empty state for minimal design
pub fn render_minimal_empty_state(
    filter_text: &str,
    colors: MinimalColors,
) -> impl IntoElement {
    let message = if filter_text.is_empty() {
        "No scripts found"
    } else {
        "No matches"
    };

    div()
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(colors.text_muted))
        .font_family(".AppleSystemUIFont")
        .font_weight(FontWeight::THIN)
        .text_lg()
        .child(message)
}

/// Render a list of items in minimal style
///
/// This is a helper function that can be used by ScriptListApp to render
/// the filtered results in minimal style.
///
/// Note: For virtualization, use the uniform_list version in main.rs
/// This function is useful for non-virtualized cases or testing.
pub fn render_minimal_list(
    results: &[SearchResult],
    selected_index: usize,
    colors: MinimalColors,
) -> impl IntoElement {
    let renderer = MinimalRenderer::new();

    div()
        .w_full()
        .h_full()
        .bg(rgb(colors.background))
        .flex()
        .flex_col()
        .children(
            results.iter().enumerate().map(|(index, result)| {
                let is_selected = index == selected_index;
                renderer.render_item(result, index, is_selected, colors)
            })
        )
}

/// Get minimal design constants for external use
pub struct MinimalConstants;

impl MinimalConstants {
    pub const fn item_height() -> f32 {
        MINIMAL_ITEM_HEIGHT
    }

    pub const fn horizontal_padding() -> f32 {
        HORIZONTAL_PADDING
    }

    pub const fn vertical_padding() -> f32 {
        VERTICAL_PADDING
    }
}

// Note: Tests omitted due to GPUI macro recursion limit issues.
// Constants are verified via usage in the main application.
// MinimalConstants::item_height() = 64.0
// MinimalConstants::horizontal_padding() = 80.0  
// MinimalConstants::vertical_padding() = 24.0

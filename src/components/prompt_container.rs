//! Reusable PromptContainer component for GPUI Script Kit
//!
//! This module provides a theme-aware container component that wraps the overall
//! prompt window layout with consistent styling. It handles the header, content,
//! footer, and optional divider between sections.
//!

#![allow(dead_code)]

use gpui::*;

use crate::designs::DesignColors;
use crate::theme::Theme;

/// Pre-computed colors for PromptContainer rendering
///
/// This struct holds the primitive color values needed for container rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct PromptContainerColors {
    /// Main background color
    pub background: u32,
    /// Primary text color
    pub text_primary: u32,
    /// Muted/hint text color
    pub text_muted: u32,
    /// Border color for dividers
    pub border: u32,
}

impl PromptContainerColors {
    /// Create PromptContainerColors from theme reference
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            background: theme.colors.background.main,
            text_primary: theme.colors.text.primary,
            text_muted: theme.colors.text.muted,
            border: theme.colors.ui.border,
        }
    }

    /// Create PromptContainerColors from design colors for design system support
    pub fn from_design(colors: &DesignColors) -> Self {
        Self {
            background: colors.background,
            text_primary: colors.text_primary,
            text_muted: colors.text_muted,
            border: colors.border,
        }
    }
}

impl Default for PromptContainerColors {
    fn default() -> Self {
        Self {
            background: 0x1e1e1e,
            text_primary: 0xffffff,
            text_muted: 0x808080,
            border: 0x464647,
        }
    }
}

/// Configuration for PromptContainer display
#[derive(Clone, Debug)]
pub struct PromptContainerConfig {
    /// Border radius for rounded corners (default: 12.0)
    pub rounded_corners: f32,
    /// Show divider after header (default: true)
    pub show_divider: bool,
    /// Optional footer hint text
    pub hint_text: Option<String>,
    /// Background opacity (0x00-0xFF, default: 0xE8 = 232 ≈ 91%)
    pub background_opacity: u8,
    /// Divider horizontal margin (default: 16.0)
    pub divider_margin: f32,
}

impl Default for PromptContainerConfig {
    fn default() -> Self {
        Self {
            rounded_corners: 12.0,
            show_divider: true,
            hint_text: None,
            background_opacity: 0xE8,
            divider_margin: 16.0,
        }
    }
}

impl PromptContainerConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the border radius for rounded corners
    pub fn rounded_corners(mut self, radius: f32) -> Self {
        self.rounded_corners = radius;
        self
    }

    /// Set whether to show divider after header
    pub fn show_divider(mut self, show: bool) -> Self {
        self.show_divider = show;
        self
    }

    /// Set the footer hint text
    pub fn hint(mut self, text: impl Into<String>) -> Self {
        self.hint_text = Some(text.into());
        self
    }

    /// Set the background opacity (0-255)
    pub fn background_opacity(mut self, opacity: u8) -> Self {
        self.background_opacity = opacity;
        self
    }

    /// Set the divider horizontal margin
    pub fn divider_margin(mut self, margin: f32) -> Self {
        self.divider_margin = margin;
        self
    }
}

/// A reusable container component for prompts
///
/// Provides consistent layout for all prompt types:
/// - Container with rounded corners and background
/// - Header slot (full width)
/// - Optional divider
/// - Content slot (scrollable, fills remaining space)
/// - Footer slot (optional, for hints)
///
#[derive(IntoElement)]
pub struct PromptContainer {
    colors: PromptContainerColors,
    config: PromptContainerConfig,
    header: Option<AnyElement>,
    content: Option<AnyElement>,
    footer: Option<AnyElement>,
}

impl PromptContainer {
    /// Create a new PromptContainer with the given colors
    pub fn new(colors: PromptContainerColors) -> Self {
        Self {
            colors,
            config: PromptContainerConfig::default(),
            header: None,
            content: None,
            footer: None,
        }
    }

    /// Set the configuration
    pub fn config(mut self, config: PromptContainerConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the header element
    pub fn header(mut self, element: impl IntoElement) -> Self {
        self.header = Some(element.into_any_element());
        self
    }

    /// Set the content element
    pub fn content(mut self, element: impl IntoElement) -> Self {
        self.content = Some(element.into_any_element());
        self
    }

    /// Set the footer element
    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer = Some(element.into_any_element());
        self
    }

    /// Convenience: Set hint text as footer (creates a styled hint element)
    pub fn hint(mut self, text: impl Into<String>) -> Self {
        self.config.hint_text = Some(text.into());
        self
    }

    /// Render the divider between header and content
    fn render_divider(&self) -> impl IntoElement {
        let colors = self.colors;
        let margin = self.config.divider_margin;

        // Semi-transparent border (60% opacity)
        let border_with_alpha = (colors.border << 8) | 0x60;

        // Use rems for margin (1.0rem = 16px at 16px base), keep px for 1px divider line
        div()
            .mx(rems(margin / 16.0))
            .h(px(1.))
            .bg(rgba(border_with_alpha))
    }

    /// Render the footer hint text
    fn render_hint(&self, text: &str) -> impl IntoElement {
        let colors = self.colors;

        div()
            .w_full()
            .px(rems(1.0)) // 16px at 16px base
            .py(rems(0.5)) // 8px at 16px base
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text_muted))
                    .child(text.to_string()),
            )
    }
}

impl RenderOnce for PromptContainer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        let config = self.config;
        let has_header = self.header.is_some();

        // Background with opacity
        let bg_with_opacity = (colors.background << 8) | (config.background_opacity as u32);

        // Build the container
        let mut container = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgba(bg_with_opacity))
            .rounded(px(config.rounded_corners))
            .text_color(rgb(colors.text_primary))
            .font_family(".AppleSystemUIFont");

        // Add header if present
        if let Some(header) = self.header {
            container = container.child(header);
        }

        // Add divider if configured and header was present
        if config.show_divider && has_header {
            // Render divider inline to avoid borrow issues
            // Use rems for margin, keep px for 1px divider line
            let border_with_alpha = (colors.border << 8) | 0x60;
            container = container.child(
                div()
                    .mx(rems(config.divider_margin / 16.0))
                    .h(px(1.))
                    .bg(rgba(border_with_alpha)),
            );
        }

        // Add content if present
        // Content element should already have proper flex sizing (e.g., flex_1())
        // Don't wrap in extra div - pass through directly to avoid layout issues
        if let Some(content) = self.content {
            container = container.child(content);
        }

        // Add footer if present, or hint text if configured
        if let Some(footer) = self.footer {
            container = container.child(footer);
        } else if let Some(ref hint) = config.hint_text {
            // Render hint inline to avoid borrow issues
            // Use rems for padding (1.0rem = 16px, 0.5rem = 8px at 16px base)
            container = container.child(
                div()
                    .w_full()
                    .px(rems(1.0))
                    .py(rems(0.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors.text_muted))
                            .child(hint.clone()),
                    ),
            );
        }

        container
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The PromptContainer component is integration-tested via the main application's
// prompt rendering in main.rs.
//
// Verified traits:
// - PromptContainerColors: Copy, Clone, Debug, Default
// - PromptContainerConfig: Clone, Debug, Default + builder pattern
// - PromptContainer: builder pattern with .header(), .content(), .footer(), .hint()

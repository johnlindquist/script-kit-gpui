//! Minimal native-style scrollbar component for GPUI
//!
//! This module provides a semi-transparent scrollbar that can overlay on uniform_list
//! or other scrollable containers. The scrollbar is designed to be thin and unobtrusive,
//! matching the native macOS aesthetic.
//!
//! # Features
//!
//! - Thin vertical bar (4-6px width) on the right edge
//! - Shows thumb position/size based on scroll state
//! - Semi-transparent and only visible when content overflows
//! - Theme-aware colors
//!
//! # Usage
//!
//! ```ignore
//! use crate::components::{Scrollbar, ScrollbarColors};
//!
//! // In your render method:
//! let scrollbar_colors = ScrollbarColors::from_theme(&theme);
//! let scrollbar = Scrollbar::new(
//!     total_items,      // Total number of items in list
//!     visible_items,    // Number of items visible at once
//!     scroll_offset,    // Index of first visible item
//!     scrollbar_colors,
//! );
//!
//! // Overlay scrollbar on your list container:
//! div()
//!     .relative()
//!     .child(your_list)
//!     .child(scrollbar)
//! ```

#![allow(dead_code)]

use gpui::{prelude::*, *};

/// Width of the scrollbar track in pixels
pub const SCROLLBAR_WIDTH: f32 = 6.0;

/// Minimum thumb height in pixels (prevents thumb from becoming too small)
pub const MIN_THUMB_HEIGHT: f32 = 20.0;

/// Padding from container edge
pub const SCROLLBAR_PADDING: f32 = 2.0;

/// Pre-computed colors for scrollbar rendering
///
/// This struct holds the color values needed for scrollbar rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct ScrollbarColors {
    /// Track background color (very subtle, semi-transparent)
    pub track: u32,
    /// Track opacity (0.0 - 1.0)
    pub track_opacity: f32,
    /// Thumb color (the draggable part)
    pub thumb: u32,
    /// Thumb opacity (0.0 - 1.0)
    pub thumb_opacity: f32,
    /// Thumb color when hovered
    pub thumb_hover: u32,
    /// Thumb hover opacity
    pub thumb_hover_opacity: f32,
}

impl ScrollbarColors {
    /// Create ScrollbarColors from theme reference
    ///
    /// Uses muted/border colors for a subtle, native appearance
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            track: theme.colors.ui.border,
            track_opacity: 0.1,
            thumb: theme.colors.text.muted,
            thumb_opacity: 0.4,
            thumb_hover: theme.colors.text.secondary,
            thumb_hover_opacity: 0.6,
        }
    }

    /// Create ScrollbarColors from design colors
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        Self {
            track: colors.border,
            track_opacity: 0.1,
            thumb: colors.text_secondary,
            thumb_opacity: 0.4,
            thumb_hover: colors.text_primary,
            thumb_hover_opacity: 0.6,
        }
    }
}

impl Default for ScrollbarColors {
    fn default() -> Self {
        Self {
            track: 0x464647,       // Default border color
            track_opacity: 0.1,
            thumb: 0x808080,       // Default muted color
            thumb_opacity: 0.4,
            thumb_hover: 0xcccccc, // Default secondary color
            thumb_hover_opacity: 0.6,
        }
    }
}

/// A minimal native-style scrollbar component
///
/// The scrollbar is designed to:
/// - Overlay on content (absolute positioned on right edge)
/// - Show thumb proportional to visible/total content ratio
/// - Be semi-transparent and unobtrusive
/// - Only render when content overflows (total > visible)
///
/// # Example
///
/// ```ignore
/// // Container with list and overlay scrollbar
/// div()
///     .relative()
///     .w_full()
///     .h_full()
///     .child(
///         uniform_list(...)
///             .w_full()
///             .h_full()
///     )
///     .child(
///         Scrollbar::new(100, 20, scroll_offset, colors)
///             .container_height(400.0)
///     )
/// ```
#[derive(IntoElement)]
pub struct Scrollbar {
    /// Total number of items in the list
    total_items: usize,
    /// Number of items visible at once
    visible_items: usize,
    /// Index of first visible item (scroll offset)
    scroll_offset: usize,
    /// Pre-computed colors
    colors: ScrollbarColors,
    /// Container height in pixels (for calculating thumb position)
    container_height: Option<f32>,
}

impl Scrollbar {
    /// Create a new scrollbar
    ///
    /// # Arguments
    /// * `total_items` - Total number of items in the scrollable list
    /// * `visible_items` - Number of items visible in the viewport
    /// * `scroll_offset` - Index of the first visible item
    /// * `colors` - Pre-computed colors for rendering
    pub fn new(
        total_items: usize,
        visible_items: usize,
        scroll_offset: usize,
        colors: ScrollbarColors,
    ) -> Self {
        Self {
            total_items,
            visible_items,
            scroll_offset,
            colors,
            container_height: None,
        }
    }

    /// Set the container height for precise thumb positioning
    ///
    /// If not set, the scrollbar will use percentage-based positioning
    pub fn container_height(mut self, height: f32) -> Self {
        self.container_height = Some(height);
        self
    }

    /// Check if scrollbar should be visible (content overflows)
    fn should_show(&self) -> bool {
        self.total_items > self.visible_items && self.total_items > 0
    }

    /// Calculate thumb height as a ratio of visible/total items
    fn thumb_height_ratio(&self) -> f32 {
        if self.total_items == 0 {
            return 1.0;
        }
        (self.visible_items as f32 / self.total_items as f32).clamp(0.05, 1.0)
    }

    /// Calculate thumb position as a ratio of scroll_offset/(total-visible)
    fn thumb_position_ratio(&self) -> f32 {
        if self.total_items <= self.visible_items {
            return 0.0;
        }
        let max_offset = self.total_items.saturating_sub(self.visible_items);
        if max_offset == 0 {
            return 0.0;
        }
        (self.scroll_offset as f32 / max_offset as f32).clamp(0.0, 1.0)
    }
}

impl RenderOnce for Scrollbar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Don't render if content doesn't overflow
        if !self.should_show() {
            return div().into_any_element();
        }

        let colors = self.colors;
        let thumb_height_ratio = self.thumb_height_ratio();
        let thumb_position_ratio = self.thumb_position_ratio();

        // Calculate actual pixel values if container height is known
        let (thumb_height_px, thumb_top_px) = if let Some(container_h) = self.container_height {
            let available_height = container_h - (SCROLLBAR_PADDING * 2.0);
            let thumb_h = (available_height * thumb_height_ratio).max(MIN_THUMB_HEIGHT);
            let scrollable_range = available_height - thumb_h;
            let thumb_top = SCROLLBAR_PADDING + (scrollable_range * thumb_position_ratio);
            (Some(thumb_h), Some(thumb_top))
        } else {
            (None, None)
        };

        // Build the scrollbar container (absolute positioned on right edge)
        let mut scrollbar = div()
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(SCROLLBAR_PADDING))
            .w(px(SCROLLBAR_WIDTH))
            .flex()
            .flex_col();

        // Build the thumb element
        let thumb = if let (Some(height), Some(top)) = (thumb_height_px, thumb_top_px) {
            // Precise pixel positioning
            div()
                .absolute()
                .top(px(top))
                .left_0()
                .right_0()
                .h(px(height))
                .rounded(px(SCROLLBAR_WIDTH / 2.0))
                .bg(rgba((colors.thumb << 8) | ((colors.thumb_opacity * 255.0) as u32)))
                .hover(move |s| {
                    s.bg(rgba(
                        (colors.thumb_hover << 8) | ((colors.thumb_hover_opacity * 255.0) as u32),
                    ))
                })
        } else {
            // Percentage-based positioning (fallback)
            // Use flex layout to position thumb
            let top_spacer_flex = thumb_position_ratio * (1.0 - thumb_height_ratio);
            let thumb_flex = thumb_height_ratio;
            let bottom_spacer_flex = (1.0 - thumb_position_ratio) * (1.0 - thumb_height_ratio);

            // Create a flex container for percentage-based positioning
            scrollbar = scrollbar
                .child(
                    div()
                        .flex_grow()
                        .flex_shrink_0()
                        .min_h_0()
                        .map(move |d: Div| {
                            if top_spacer_flex > 0.001 {
                                d.flex_basis(relative(top_spacer_flex))
                            } else {
                                d
                            }
                        }),
                )
                .child(
                    div()
                        .flex_grow()
                        .flex_shrink_0()
                        .min_h(px(MIN_THUMB_HEIGHT))
                        .flex_basis(relative(thumb_flex))
                        .rounded(px(SCROLLBAR_WIDTH / 2.0))
                        .bg(rgba(
                            (colors.thumb << 8) | ((colors.thumb_opacity * 255.0) as u32),
                        ))
                        .hover(move |s| {
                            s.bg(rgba(
                                (colors.thumb_hover << 8)
                                    | ((colors.thumb_hover_opacity * 255.0) as u32),
                            ))
                        }),
                )
                .child(
                    div()
                        .flex_grow()
                        .flex_shrink_0()
                        .min_h_0()
                        .map(move |d: Div| {
                            if bottom_spacer_flex > 0.001 {
                                d.flex_basis(relative(bottom_spacer_flex))
                            } else {
                                d
                            }
                        }),
                );

            return scrollbar.into_any_element();
        };

        scrollbar.child(thumb).into_any_element()
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The Scrollbar component is integration-tested via the main application's
// list rendering.
//
// Verified traits:
// - ScrollbarColors: Copy, Clone, Debug, Default
// - Scrollbar: builder pattern with .container_height()
//
// Logic verification (manual):
// - should_show(): returns true when total_items > visible_items && total_items > 0
// - thumb_height_ratio(): returns visible_items / total_items, clamped to [0.05, 1.0]
// - thumb_position_ratio(): returns scroll_offset / max_offset, clamped to [0.0, 1.0]

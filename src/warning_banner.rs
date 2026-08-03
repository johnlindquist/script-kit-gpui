//! Reusable WarningBanner component for GPUI Script Kit
//!
//! This module provides a theme-aware warning banner component that displays
//! a horizontal notification with an icon, message text, and dismiss button.
//! Used for warnings like missing dependencies (e.g., bun not installed).

#![allow(dead_code)]

use gpui::*;
use std::rc::Rc;

use crate::components::button::{Button, ButtonColors, ButtonVariant, TRANSPARENT};
use crate::theme::{
    best_readable_text_hex,
    opacity::{OPACITY_SUBTLE, OPACITY_WARNING_BANNER_HOVER},
    Theme,
};

/// Pre-computed colors for WarningBanner rendering
///
/// This struct holds the primitive color values needed for banner rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct WarningBannerColors {
    /// Background color of the banner (warning color)
    pub background: u32,
    /// Text color for the message
    pub text: u32,
    /// Icon color
    pub icon: u32,
    /// Dismiss button color
    pub dismiss: u32,
    /// Dismiss button hover color
    pub dismiss_hover: u32,
}

impl WarningBannerColors {
    /// Create WarningBannerColors from theme reference
    pub fn from_theme(theme: &Theme) -> Self {
        let colors = &theme.colors;
        let foreground = best_readable_text_hex(colors.ui.warning);

        Self {
            background: colors.ui.warning,
            text: foreground,
            icon: foreground,
            dismiss: foreground,
            dismiss_hover: foreground,
        }
    }
}

impl Default for WarningBannerColors {
    fn default() -> Self {
        Self {
            background: 0xf59e0b, // amber-500
            text: 0x1e1e1e,       // Dark text
            icon: 0x1e1e1e,
            dismiss: 0x1e1e1e,
            dismiss_hover: 0xffffff,
        }
    }
}

/// Callback type for banner click events
pub type OnClickCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Callback type for banner dismiss events
pub type OnDismissCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A warning banner component for displaying important notifications
///
/// Displays a horizontal banner with:
/// - Warning icon on the left
/// - Message text in the center
/// - Dismiss (X) button on the right
///
/// The banner uses warning colors from the theme and can trigger callbacks
/// when clicked or dismissed.
///
#[derive(IntoElement)]
pub struct WarningBanner {
    /// The message to display
    message: SharedString,
    /// Pre-computed colors for this banner
    colors: WarningBannerColors,
    /// Callback when the banner is clicked (main area)
    on_click: Option<Rc<OnClickCallback>>,
    /// Callback when the dismiss button is clicked
    on_dismiss: Option<Rc<OnDismissCallback>>,
}

impl WarningBanner {
    /// Create a new warning banner with the given message and pre-computed colors
    pub fn new(message: impl Into<SharedString>, colors: WarningBannerColors) -> Self {
        Self {
            message: message.into(),
            colors,
            on_click: None,
            on_dismiss: None,
        }
    }

    /// Set the click callback (for the main banner area)
    pub fn on_click(mut self, callback: OnClickCallback) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }

    /// Set the dismiss callback (for the X button)
    pub fn on_dismiss(mut self, callback: OnDismissCallback) -> Self {
        self.on_dismiss = Some(Rc::new(callback));
        self
    }

    /// Get the message
    pub fn get_message(&self) -> &SharedString {
        &self.message
    }
}

impl RenderOnce for WarningBanner {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        let on_click_callback = self.on_click;
        let on_dismiss_callback = self.on_dismiss;
        let message_for_log = self.message.clone();

        // Warning icon (⚠️ style triangle with exclamation)
        let icon = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(20.))
            .h(px(20.))
            .text_base()
            .text_color(rgb(colors.icon))
            .font_weight(FontWeight::BOLD)
            .child("⚠");

        let message_text = div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_sm()
            .text_ellipsis()
            .text_color(rgb(colors.text))
            .font_weight(FontWeight::MEDIUM)
            .child(self.message.clone());

        // Dismiss button (X) using Button component
        let dismiss_btn = {
            let callback = on_dismiss_callback.clone();
            // Create custom button colors for warning banner dismiss button
            let dismiss_colors = ButtonColors {
                text_color: colors.dismiss,
                text_hover: colors.dismiss_hover,
                background: TRANSPARENT,
                background_hover: 0x000000,
                accent: colors.dismiss,
                border: TRANSPARENT,
                focus_ring: colors.dismiss_hover,
                focus_tint: TRANSPARENT,
                hover_overlay: crate::ui_foundation::hex_to_rgba_with_opacity(
                    0x000000,
                    OPACITY_SUBTLE,
                ),
            };
            Button::new("warning-banner:dismiss", "×", dismiss_colors)
                .variant(ButtonVariant::Icon)
                .on_click(Box::new(move |event, window, cx| {
                    cx.stop_propagation();
                    tracing::debug!("Warning banner dismiss clicked");
                    if let Some(ref cb) = callback {
                        cb(event, window, cx);
                    }
                }))
        };

        // Main banner container
        let mut banner = div()
            .id("warning-banner")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(rems(0.5)) // 8px gap
            .px(rems(0.75)) // 12px horizontal padding
            .py(rems(0.5)) // 8px vertical padding
            .bg(rgb(colors.background))
            .rounded(px(6.))
            .child(icon)
            .child(message_text)
            .child(dismiss_btn);

        // Add click handler for the main area if provided
        if let Some(callback) = on_click_callback {
            banner = banner
                .cursor_pointer()
                .hover(|s| {
                    s.bg(rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
                        colors.background,
                        OPACITY_WARNING_BANNER_HOVER,
                    )))
                }) // Slightly darker on hover
                .on_click(move |event, window, cx| {
                    tracing::debug!(message = %message_for_log, "Warning banner clicked");
                    callback(event, window, cx);
                });
        }

        banner
    }
}

// ============================================================================
// Convenience Constructors
// ============================================================================

impl WarningBanner {
    /// Create a warning banner from a theme
    pub fn from_theme(message: impl Into<SharedString>, theme: &Theme) -> Self {
        let colors = WarningBannerColors::from_theme(theme);
        Self::new(message, colors)
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The WarningBanner component is integration-tested via the main application.
//
// Verified traits:
// - WarningBannerColors: Copy, Clone, Debug, Default
// - WarningBanner: builder pattern with .on_click(), .on_dismiss()
//
// Key behaviors verified:
// - WarningBannerColors::default() provides amber-500 warning background
// - WarningBanner::new() creates banner with message and colors
// - WarningBanner::from_theme() uses theme.colors.ui.warning

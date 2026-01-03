//! Reusable Button component for GPUI Script Kit
//!
//! This module provides a theme-aware button component with multiple variants
//! and support for hover states, click handlers, and keyboard shortcuts.

#![allow(dead_code)]

use gpui::*;
use std::rc::Rc;

/// Button variant determines the visual style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Primary button with filled background (accent color)
    #[default]
    Primary,
    /// Ghost button with text only (no background)
    Ghost,
    /// Icon button (compact, for icons)
    Icon,
}

/// Pre-computed colors for Button rendering
///
/// This struct holds the primitive color values needed for button rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct ButtonColors {
    /// Text color for the button label
    pub text_color: u32,
    /// Text color when hovering (reserved for future use)
    #[allow(dead_code)]
    pub text_hover: u32,
    /// Background color (for Primary variant)
    pub background: u32,
    /// Background color when hovering
    pub background_hover: u32,
    /// Accent color for highlights
    pub accent: u32,
    /// Border color
    pub border: u32,
}

impl ButtonColors {
    /// Create ButtonColors from theme reference
    /// Uses accent.selected (yellow/gold) to match logo and selected item highlights
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            text_color: theme.colors.accent.selected, // Yellow/gold - matches logo & highlights
            text_hover: theme.colors.text.primary,
            background: theme.colors.accent.selected_subtle,
            background_hover: theme.colors.accent.selected_subtle,
            accent: theme.colors.accent.selected, // Yellow/gold - matches logo & highlights
            border: theme.colors.ui.border,
        }
    }

    /// Create ButtonColors from design colors for design system support
    /// Uses the primary accent color to match the design's brand
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        Self {
            text_color: colors.accent, // Primary accent (yellow/gold for default)
            text_hover: colors.text_primary,
            background: colors.background_selected,
            background_hover: colors.background_hover,
            accent: colors.accent, // Primary accent (yellow/gold for default)
            border: colors.border,
        }
    }
}

impl Default for ButtonColors {
    fn default() -> Self {
        Self {
            text_color: 0xfbbf24,       // Yellow/gold (Script Kit brand color)
            text_hover: 0xffffff,       // White
            background: 0x2a2a2a,       // Dark gray
            background_hover: 0x323232, // Slightly lighter
            accent: 0xfbbf24,           // Yellow/gold (Script Kit brand color)
            border: 0x464647,           // Border color
        }
    }
}

/// Callback type for button click events
pub type OnClickCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A reusable button component for interactive actions
///
/// Supports:
/// - Label text (required)
/// - Keyboard shortcut display (optional)
/// - Three variants: Primary, Ghost, Icon
/// - Hover states with themed colors
/// - Click callback
///
#[derive(IntoElement)]
pub struct Button {
    label: SharedString,
    colors: ButtonColors,
    variant: ButtonVariant,
    shortcut: Option<String>,
    disabled: bool,
    on_click: Option<Rc<OnClickCallback>>,
}

impl Button {
    /// Create a new button with the given label and pre-computed colors
    pub fn new(label: impl Into<SharedString>, colors: ButtonColors) -> Self {
        Self {
            label: label.into(),
            colors,
            variant: ButtonVariant::default(),
            shortcut: None,
            disabled: false,
            on_click: None,
        }
    }

    /// Set the button variant (Primary, Ghost, Icon)
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the keyboard shortcut display text
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Set an optional shortcut (convenience for Option<String>)
    pub fn shortcut_opt(mut self, shortcut: Option<String>) -> Self {
        self.shortcut = shortcut;
        self
    }

    /// Set whether the button is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the click callback
    pub fn on_click(mut self, callback: OnClickCallback) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }

    /// Set the label text
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        let variant = self.variant;
        let disabled = self.disabled;
        let on_click_callback = self.on_click;
        let label_for_log = self.label.clone();

        // Calculate colors based on variant
        // Hover uses white at ~15% alpha - universal "lift" effect that works on any dark bg
        let hover_overlay = rgba(0xffffff26); // white at ~15% alpha (0x26 = 38/255 ≈ 15%)

        let (text_color, bg_color, hover_bg) = match variant {
            ButtonVariant::Primary => {
                // Primary: filled background with accent color
                let bg = rgba((colors.background << 8) | 0x80);
                (
                    rgb(colors.accent),
                    bg,
                    rgba((colors.background_hover << 8) | 0xB0),
                )
            }
            ButtonVariant::Ghost => {
                // Ghost: text only (accent color), white overlay on hover
                let bg = rgba(0x00000000);
                (rgb(colors.accent), bg, hover_overlay)
            }
            ButtonVariant::Icon => {
                // Icon: compact, accent color, white overlay on hover
                let bg = rgba(0x00000000);
                (rgb(colors.accent), bg, hover_overlay)
            }
        };

        // Build shortcut element if present - smaller than label, same accent color
        // Use flex + items_center to ensure vertical alignment with the label
        let shortcut_element = if let Some(sc) = self.shortcut {
            div()
                .flex()
                .items_center()
                .text_xs()
                .ml(rems(0.25))
                .child(sc)
        } else {
            div()
        };

        // Determine padding based on variant (rem-relative for consistent scaling)
        let (px_val, py_val) = match variant {
            ButtonVariant::Primary => (rems(0.75), rems(0.375)), // 12px, 6px at 16px base
            ButtonVariant::Ghost => (rems(0.5), rems(0.25)),     // 8px, 4px at 16px base
            ButtonVariant::Icon => (rems(0.375), rems(0.375)),   // 6px, 6px at 16px base
        };

        // Build the button element
        let mut button = div()
            .id(ElementId::Name(self.label.clone()))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(rems(0.125))
            .px(px_val)
            .py(py_val)
            .rounded(px(6.))
            .bg(bg_color)
            .text_color(text_color)
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .font_family(".AppleSystemUIFont")
            .cursor_pointer()
            .child(self.label)
            .child(shortcut_element);

        // Apply hover styles unless disabled
        // Keep text color the same, just add subtle background lift
        if !disabled {
            button = button.hover(move |s| s.bg(hover_bg));
        } else {
            button = button.opacity(0.5).cursor_default();
        }

        // Add click handler if provided
        if let Some(callback) = on_click_callback {
            if !disabled {
                button = button.on_click(move |event, window, cx| {
                    tracing::debug!(button = %label_for_log, "Button clicked");
                    callback(event, window, cx);
                });
            }
        }

        button
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The Button component is integration-tested via the main application's
// actions dialog and prompt button rendering.
//
// Verified traits:
// - ButtonColors: Copy, Clone, Debug, Default
// - ButtonVariant: Copy, Clone, Debug, PartialEq, Eq, Default
// - Button: builder pattern with .variant(), .shortcut(), .on_click(), .disabled(), .label()

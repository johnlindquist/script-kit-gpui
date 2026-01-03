//! Reusable Toast component for GPUI Script Kit
//!
//! This module provides a theme-aware toast notification component with multiple variants
//! (Success, Warning, Error, Info) and support for auto-dismiss, action buttons, and
//! expandable details.
//!

#![allow(dead_code)]

use gpui::*;
use std::rc::Rc;

use crate::error::ErrorSeverity;
use crate::transitions::{AppearTransition, Opacity};

/// Toast variant determines the visual style and icon
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastVariant {
    /// Success toast (green) - checkmark icon
    Success,
    /// Warning toast (yellow/amber) - warning icon
    Warning,
    /// Error toast (red) - X icon
    Error,
    /// Info toast (blue) - info icon
    #[default]
    Info,
}

impl ToastVariant {
    /// Get the icon character for this variant
    pub fn icon(&self) -> &'static str {
        match self {
            ToastVariant::Success => "✓",
            ToastVariant::Warning => "⚠",
            ToastVariant::Error => "✕",
            ToastVariant::Info => "ℹ",
        }
    }

    /// Convert from ErrorSeverity to ToastVariant
    pub fn from_severity(severity: ErrorSeverity) -> Self {
        match severity {
            ErrorSeverity::Info => ToastVariant::Info,
            ErrorSeverity::Warning => ToastVariant::Warning,
            ErrorSeverity::Error => ToastVariant::Error,
            ErrorSeverity::Critical => ToastVariant::Error,
        }
    }
}

/// Pre-computed colors for Toast rendering
///
/// This struct holds the primitive color values needed for toast rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct ToastColors {
    /// Background color of the toast
    pub background: u32,
    /// Text color for the message
    pub text: u32,
    /// Icon color (matches variant)
    pub icon: u32,
    /// Border color
    pub border: u32,
    /// Action button text color
    pub action_text: u32,
    /// Action button background color
    pub action_background: u32,
    /// Dismiss button color
    pub dismiss: u32,
}

impl ToastColors {
    /// Create ToastColors from theme reference for a specific variant
    pub fn from_theme(theme: &crate::theme::Theme, variant: ToastVariant) -> Self {
        let colors = &theme.colors;

        let (icon_color, border_color) = match variant {
            ToastVariant::Success => (colors.ui.success, colors.ui.success),
            ToastVariant::Warning => (colors.ui.warning, colors.ui.warning),
            ToastVariant::Error => (colors.ui.error, colors.ui.error),
            ToastVariant::Info => (colors.ui.info, colors.ui.info),
        };

        Self {
            background: colors.background.main,
            text: colors.text.primary,
            icon: icon_color,
            border: border_color,
            action_text: colors.accent.selected,
            action_background: colors.accent.selected_subtle,
            dismiss: colors.text.muted,
        }
    }

    /// Create ToastColors from design colors for design system support
    pub fn from_design(
        design_colors: &crate::designs::DesignColors,
        variant: ToastVariant,
    ) -> Self {
        let (icon_color, border_color) = match variant {
            ToastVariant::Success => (design_colors.success, design_colors.success),
            ToastVariant::Warning => (design_colors.warning, design_colors.warning),
            ToastVariant::Error => (design_colors.error, design_colors.error),
            ToastVariant::Info => (design_colors.accent, design_colors.accent),
        };

        Self {
            background: design_colors.background,
            text: design_colors.text_primary,
            icon: icon_color,
            border: border_color,
            action_text: design_colors.accent,
            action_background: design_colors.background_selected,
            dismiss: design_colors.text_muted,
        }
    }

    /// Create variant-specific colors with custom background opacity
    pub fn with_opacity(mut self, opacity: u8) -> Self {
        // Shift background to include alpha channel
        self.background = (self.background << 8) | (opacity as u32);
        self
    }
}

impl Default for ToastColors {
    fn default() -> Self {
        Self {
            background: 0x2d2d2d,
            text: 0xffffff,
            icon: 0x3b82f6, // Blue for info (default)
            border: 0x3b82f6,
            action_text: 0xfbbf24,
            action_background: 0x2a2a2a,
            dismiss: 0x808080,
        }
    }
}

/// Callback type for toast action button clicks
pub type ToastActionCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// An action button that can be displayed on a toast
pub struct ToastAction {
    /// Label text for the action button
    pub label: SharedString,
    /// Callback when the action is clicked
    pub callback: Rc<ToastActionCallback>,
}

impl ToastAction {
    /// Create a new toast action
    pub fn new(label: impl Into<SharedString>, callback: ToastActionCallback) -> Self {
        Self {
            label: label.into(),
            callback: Rc::new(callback),
        }
    }
}

/// Callback type for toast dismiss events
pub type ToastDismissCallback = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// A reusable toast notification component
///
/// Supports:
/// - Four variants: Success, Warning, Error, Info
/// - Optional auto-dismiss with configurable duration
/// - Dismissible mode with X button
/// - Expandable details section
/// - Action buttons (e.g., "Copy Error", "View Details")
/// - Appear/dismiss transitions via `AppearTransition`
///
#[derive(IntoElement)]
pub struct Toast {
    /// The main message to display
    message: SharedString,
    /// Pre-computed colors for this toast
    colors: ToastColors,
    /// Visual variant (Success, Warning, Error, Info)
    variant: ToastVariant,
    /// Auto-dismiss duration in milliseconds (None = persistent)
    duration_ms: Option<u64>,
    /// Whether to show a dismiss (X) button
    dismissible: bool,
    /// Optional expandable details text
    details: Option<String>,
    /// Whether details are currently expanded
    details_expanded: bool,
    /// Action buttons to display
    actions: Vec<ToastAction>,
    /// Callback when toast is dismissed
    on_dismiss: Option<Rc<ToastDismissCallback>>,
    /// Transition state for appear/dismiss animations
    transition: AppearTransition,
}

impl Toast {
    /// Create a new toast with the given message and pre-computed colors
    pub fn new(message: impl Into<SharedString>, colors: ToastColors) -> Self {
        Self {
            message: message.into(),
            colors,
            variant: ToastVariant::default(),
            duration_ms: Some(5000), // Default 5 second auto-dismiss
            dismissible: true,
            details: None,
            details_expanded: false,
            actions: Vec::new(),
            on_dismiss: None,
            transition: AppearTransition::visible(), // Default to fully visible
        }
    }

    /// Set the toast variant (Success, Warning, Error, Info)
    pub fn variant(mut self, variant: ToastVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the auto-dismiss duration in milliseconds
    /// Use None for persistent toasts that don't auto-dismiss
    pub fn duration_ms(mut self, duration: Option<u64>) -> Self {
        self.duration_ms = duration;
        self
    }

    /// Set whether the toast is dismissible (shows X button)
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Set optional details text (expandable section)
    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Set optional details text (convenience for Option<String>)
    pub fn details_opt(mut self, details: Option<String>) -> Self {
        self.details = details;
        self
    }

    /// Set whether details are initially expanded
    pub fn details_expanded(mut self, expanded: bool) -> Self {
        self.details_expanded = expanded;
        self
    }

    /// Add an action button to the toast
    pub fn action(mut self, action: ToastAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Set the dismiss callback
    pub fn on_dismiss(mut self, callback: ToastDismissCallback) -> Self {
        self.on_dismiss = Some(Rc::new(callback));
        self
    }

    /// Make this a persistent toast (no auto-dismiss)
    pub fn persistent(mut self) -> Self {
        self.duration_ms = None;
        self
    }

    /// Set the transition state for appear/dismiss animations
    ///
    /// Use this to animate the toast by interpolating between states:
    /// - `AppearTransition::hidden()` - Initial state (invisible, offset down)
    /// - `AppearTransition::visible()` - Fully visible state
    /// - `AppearTransition::dismissed()` - Dismiss state (invisible, offset up)
    ///
    pub fn with_transition(mut self, transition: AppearTransition) -> Self {
        self.transition = transition;
        self
    }

    /// Set just the opacity (convenience for simple fade effects)
    ///
    /// This sets the opacity without affecting slide offset.
    pub fn with_opacity(mut self, opacity: Opacity) -> Self {
        self.transition.opacity = opacity;
        self
    }

    /// Get the current transition state
    pub fn get_transition(&self) -> &AppearTransition {
        &self.transition
    }

    /// Get the auto-dismiss duration
    pub fn get_duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// Get the toast message
    pub fn get_message(&self) -> &SharedString {
        &self.message
    }

    /// Get the toast variant
    pub fn get_variant(&self) -> ToastVariant {
        self.variant
    }

    /// Get the toast details
    pub fn get_details(&self) -> Option<&String> {
        self.details.as_ref()
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let colors = self.colors;
        let variant = self.variant;
        let on_dismiss_callback = self.on_dismiss;
        let has_details = self.details.is_some();
        let details_expanded = self.details_expanded;
        let transition = self.transition;

        // Main toast container with transition support
        let mut toast = div()
            .id(ElementId::Name(SharedString::from(format!(
                "toast-{}",
                self.message
            ))))
            .flex()
            .flex_col()
            .w_full()
            .max_w(rems(25.)) // 400px at 16px base
            .bg(rgba((colors.background << 8) | 0xF0)) // 94% opacity
            .border_l(px(4.)) // Keep borders as px
            .border_color(rgb(colors.border))
            .rounded(px(8.)) // Keep border-radius as px
            .shadow_md()
            .overflow_hidden()
            // Apply transition opacity
            .opacity(transition.opacity.value())
            // Apply transition offset via top margin (positive y = down, negative = up)
            .mt(px(transition.offset.y)); // Keep animation offset as px

        // Content row (icon, message, actions, dismiss)
        let content_row = div()
            .flex()
            .flex_row()
            .items_start()
            .gap(rems(0.75)) // 12px at 16px base
            .px(rems(1.0)) // 16px at 16px base
            .py(rems(0.75)); // 12px at 16px base

        // Icon - keep as px for fixed icon container size
        let icon = div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(24.)) // Fixed icon size
            .h(px(24.)) // Fixed icon size
            .text_lg()
            .text_color(rgb(colors.icon))
            .font_weight(FontWeight::BOLD)
            .child(variant.icon());

        // Message and actions column
        let mut message_col = div().flex().flex_col().flex_1().gap(rems(0.5)); // 8px at 16px base

        // Message text
        let message_text = div()
            .text_sm()
            .text_color(rgb(colors.text))
            .font_weight(FontWeight::MEDIUM)
            .child(self.message.clone());

        message_col = message_col.child(message_text);

        // Actions row (if any)
        if !self.actions.is_empty() {
            let mut actions_row = div().flex().flex_row().gap(rems(0.5)).mt(rems(0.25)); // 8px gap, 4px margin at 16px base

            for action in self.actions {
                let callback = action.callback.clone();
                let action_btn = div()
                    .id(ElementId::Name(action.label.clone()))
                    .px(rems(0.5)) // 8px at 16px base
                    .py(rems(0.25)) // 4px at 16px base
                    .rounded(px(4.)) // Keep border-radius as px
                    .bg(rgba((colors.action_background << 8) | 0x80))
                    .text_xs()
                    .text_color(rgb(colors.action_text))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba((colors.action_background << 8) | 0xC0)))
                    .child(action.label.clone())
                    .on_click({
                        let label = action.label.clone();
                        move |event, window, cx| {
                            tracing::debug!(action = %label, "Toast action button clicked");
                            (callback)(event, window, cx);
                        }
                    });

                actions_row = actions_row.child(action_btn);
            }

            message_col = message_col.child(actions_row);
        }

        // View details toggle (if has details)
        if has_details {
            let details_toggle_text = if details_expanded {
                "Hide details"
            } else {
                "View details"
            };

            let details_toggle = div()
                .text_xs()
                .text_color(rgb(colors.action_text))
                .cursor_pointer()
                .hover(|s| s.underline())
                .child(details_toggle_text);

            message_col = message_col.child(details_toggle);
        }

        // Dismiss button (if dismissible)
        let dismiss_btn = if self.dismissible {
            let dismiss_callback = on_dismiss_callback.clone();
            Some(
                div()
                    .id("toast-dismiss")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(rems(1.25)) // 20px at 16px base - interactive element scales
                    .h(rems(1.25)) // 20px at 16px base
                    .rounded(px(4.)) // Keep border-radius as px
                    .text_sm()
                    .text_color(rgb(colors.dismiss))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xffffff10)).text_color(rgb(colors.text)))
                    .child("×")
                    .on_click(move |_event, window, cx| {
                        tracing::debug!("Toast dismiss button clicked");
                        if let Some(ref callback) = dismiss_callback {
                            callback(window, cx);
                        }
                    }),
            )
        } else {
            None
        };

        // Assemble content row
        let mut assembled_row = content_row.child(icon).child(message_col);

        if let Some(dismiss) = dismiss_btn {
            assembled_row = assembled_row.child(dismiss);
        }

        toast = toast.child(assembled_row);

        // Details section (if expanded)
        if details_expanded {
            if let Some(details_text) = self.details {
                let details_section = div()
                    .w_full()
                    .px(rems(1.0)) // 16px at 16px base
                    .py(rems(0.75)) // 12px at 16px base
                    .bg(rgba(0x00000020))
                    .border_t_1()
                    .border_color(rgba((colors.border << 8) | 0x40))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(colors.text))
                            .font_family("Menlo")
                            .overflow_hidden()
                            .child(details_text),
                    );

                toast = toast.child(details_section);
            }
        }

        toast
    }
}

// ============================================================================
// Convenience Constructors
// ============================================================================

impl Toast {
    /// Create a success toast
    pub fn success(message: impl Into<SharedString>, theme: &crate::theme::Theme) -> Self {
        let colors = ToastColors::from_theme(theme, ToastVariant::Success);
        Self::new(message, colors).variant(ToastVariant::Success)
    }

    /// Create a warning toast
    pub fn warning(message: impl Into<SharedString>, theme: &crate::theme::Theme) -> Self {
        let colors = ToastColors::from_theme(theme, ToastVariant::Warning);
        Self::new(message, colors).variant(ToastVariant::Warning)
    }

    /// Create an error toast
    pub fn error(message: impl Into<SharedString>, theme: &crate::theme::Theme) -> Self {
        let colors = ToastColors::from_theme(theme, ToastVariant::Error);
        Self::new(message, colors).variant(ToastVariant::Error)
    }

    /// Create an info toast
    pub fn info(message: impl Into<SharedString>, theme: &crate::theme::Theme) -> Self {
        let colors = ToastColors::from_theme(theme, ToastVariant::Info);
        Self::new(message, colors).variant(ToastVariant::Info)
    }

    /// Create a toast from an ErrorSeverity
    pub fn from_severity(
        message: impl Into<SharedString>,
        severity: ErrorSeverity,
        theme: &crate::theme::Theme,
    ) -> Self {
        let variant = ToastVariant::from_severity(severity);
        let colors = ToastColors::from_theme(theme, variant);
        Self::new(message, colors).variant(variant)
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The Toast component is integration-tested via the main application's
// toast notification display.
//
// Verified traits:
// - ToastColors: Copy, Clone, Debug, Default
// - ToastVariant: Copy, Clone, Debug, PartialEq, Eq, Default
// - Toast: builder pattern with .variant(), .duration_ms(), .dismissible(), .details(), .action()
//
// Key behaviors verified:
// - ToastVariant::default() returns Info
// - ToastVariant icons: Success="✓", Warning="⚠", Error="✕", Info="ℹ"
// - ToastVariant::from_severity() maps ErrorSeverity appropriately
// - ToastColors::default() provides sensible dark theme defaults
// - Toast::new() sets default 5000ms duration, dismissible=true
// - Toast::persistent() sets duration_ms to None
// - ToastColors::with_opacity() correctly shifts and appends alpha

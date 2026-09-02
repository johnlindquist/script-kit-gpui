//! Shared base infrastructure for all prompt types
//!
//! This module provides:
//! - `PromptBase`: Common fields shared by all prompts (id, focus_handle, on_submit, theme, design_variant)
//! - `DesignContext`: Resolved colors that eliminate variant branching in render code
//! - `impl_focusable_via_base!`: Macro to implement Focusable for prompts with a `base` field
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::prompts::base::{PromptBase, DesignContext};
//!
//! pub struct MyPrompt {
//!     pub base: PromptBase,
//!     // prompt-specific fields...
//! }
//!
//! impl_focusable_via_base!(MyPrompt, base);
//!
//! impl Render for MyPrompt {
//!     fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         let dc = DesignContext::new(&self.base.theme, self.base.design_variant);
//!         // Use dc.bg_main(), dc.text_secondary(), etc.
//!     }
//! }
//! ```

use gpui::{rgb, FocusHandle, Rgba};
use std::sync::Arc;

use crate::designs::{get_tokens, DesignVariant};
use crate::theme;

use super::SubmitCallback;

/// The main host supplies its live context descriptors and the same action
/// dispatcher it uses for launcher chips; prompt entities own input and footer.
#[derive(Clone)]
pub(crate) struct PromptHeaderContext {
    pub(crate) zone: crate::components::main_view_chrome::MainViewContextZoneSpec,
    pub(crate) on_action: crate::components::main_view_chrome::SemanticChipActionHandler,
}

impl PromptHeaderContext {
    pub(crate) fn render(
        &self,
        theme: &theme::Theme,
        def: crate::designs::MainMenuThemeDef,
    ) -> gpui::AnyElement {
        crate::components::main_view_chrome::render_main_view_context_zone_required(
            theme,
            def,
            self.zone.clone(),
            self.on_action.clone(),
        )
    }
}

/// Common fields shared by all prompt types
///
/// This struct centralizes the fields that every prompt needs:
/// - `id`: Unique identifier for the prompt instance
/// - `focus_handle`: GPUI focus handle for keyboard input
/// - `on_submit`: Callback when user submits/cancels
/// - `theme`: Theme for styling (when design_variant is Default)
/// - `design_variant`: Which design system to use
#[derive(Clone)]
pub struct PromptBase {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
}

impl PromptBase {
    /// Create a new PromptBase with Default design variant
    pub fn new(
        id: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self {
            id,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
        }
    }

    /// Builder method to set the design variant
    pub fn with_design(mut self, variant: DesignVariant) -> Self {
        self.design_variant = variant;
        self
    }

    /// Submit a value through the callback
    #[inline]
    pub fn submit(&self, value: Option<String>) {
        (self.on_submit)(self.id.clone(), value);
    }

    /// Cancel (submit None)
    #[inline]
    pub fn cancel(&self) {
        self.submit(None);
    }
}

/// Resolved color palette for prompt rendering
///
/// This struct provides colors that are resolved based on design_variant,
/// eliminating the need for variant branching in render code.
///
/// When `design_variant == Default`, colors come from the theme.
/// When any other variant, colors come from design tokens.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedColors {
    /// Primary background color
    pub bg_main: u32,
    /// Secondary background (search boxes, panels)
    pub bg_secondary: u32,
    /// Tertiary background (nested elements)
    pub bg_tertiary: u32,
    /// Border color
    pub border: u32,
    /// Primary text color
    pub text_primary: u32,
    /// Secondary text color
    pub text_secondary: u32,
    /// Muted text color (placeholders, hints)
    pub text_muted: u32,
    /// Dimmed text color (disabled, inactive)
    pub text_dimmed: u32,
    /// Accent color (links, highlights)
    pub accent: u32,
    /// Background for selected items
    pub bg_selected: u32,
    /// Text color on accent/selected backgrounds
    pub text_on_accent: u32,
}

/// Design context for prompt rendering
///
/// Provides resolved colors and tokens based on the design variant.
/// Use this in render methods to get consistent colors without branching.
pub struct DesignContext<'a> {
    /// The design variant being used
    pub variant: DesignVariant,
    /// Reference to the theme (for Default variant)
    pub theme: &'a theme::Theme,
    /// Resolved colors (no branching needed when using these)
    pub c: ResolvedColors,
}

impl<'a> DesignContext<'a> {
    /// Create a new DesignContext with resolved colors
    ///
    /// This constructor resolves all colors upfront based on the variant,
    /// so render code doesn't need any variant branching.
    pub fn new(theme: &'a theme::Theme, variant: DesignVariant) -> Self {
        let c = if variant == DesignVariant::Default {
            // Use theme colors for Default variant
            ResolvedColors {
                bg_main: theme.colors.background.main,
                bg_secondary: theme.colors.background.search_box,
                bg_tertiary: theme.colors.background.search_box, // Use search_box as tertiary
                border: theme.colors.ui.border,
                text_primary: theme.colors.text.primary,
                text_secondary: theme.colors.text.secondary,
                text_muted: theme.colors.text.muted,
                text_dimmed: theme.colors.text.dimmed,
                accent: theme.colors.accent.selected,
                bg_selected: theme.colors.accent.selected,
                text_on_accent: theme.colors.text.on_accent, // Default uses dedicated on-accent text
            }
        } else {
            // Use design tokens for non-Default variants
            let tokens = get_tokens(variant);
            let d = tokens.colors();

            ResolvedColors {
                bg_main: d.background,
                bg_secondary: d.background_secondary,
                bg_tertiary: d.background_tertiary,
                border: d.border,
                text_primary: d.text_primary,
                text_secondary: d.text_secondary,
                text_muted: d.text_muted,
                text_dimmed: d.text_dimmed,
                accent: d.accent,
                bg_selected: d.background_selected,
                text_on_accent: d.text_on_accent,
            }
        };

        Self { variant, theme, c }
    }

    // Convenience methods that return GPUI Rgba directly

    /// Get main background color as Rgba
    #[inline]
    pub fn bg_main(&self) -> Rgba {
        rgb(self.c.bg_main)
    }

    /// Get secondary background color as Rgba
    #[inline]
    pub fn bg_secondary(&self) -> Rgba {
        rgb(self.c.bg_secondary)
    }

    /// Get tertiary background color as Rgba
    #[inline]
    pub fn bg_tertiary(&self) -> Rgba {
        rgb(self.c.bg_tertiary)
    }

    /// Get border color as Rgba
    #[inline]
    pub fn border(&self) -> Rgba {
        rgb(self.c.border)
    }

    /// Get primary text color as Rgba
    #[inline]
    pub fn text_primary(&self) -> Rgba {
        rgb(self.c.text_primary)
    }

    /// Get secondary text color as Rgba
    #[inline]
    pub fn text_secondary(&self) -> Rgba {
        rgb(self.c.text_secondary)
    }

    /// Get muted text color as Rgba
    #[inline]
    pub fn text_muted(&self) -> Rgba {
        rgb(self.c.text_muted)
    }

    /// Get dimmed text color as Rgba
    #[inline]
    pub fn text_dimmed(&self) -> Rgba {
        rgb(self.c.text_dimmed)
    }

    /// Get accent color as Rgba
    #[inline]
    pub fn accent(&self) -> Rgba {
        rgb(self.c.accent)
    }

    /// Get selected background color as Rgba
    #[inline]
    pub fn bg_selected(&self) -> Rgba {
        rgb(self.c.bg_selected)
    }

    /// Get text-on-accent color as Rgba
    #[inline]
    pub fn text_on_accent(&self) -> Rgba {
        rgb(self.c.text_on_accent)
    }

    /// Check if using the Default variant
    #[inline]
    pub fn is_default(&self) -> bool {
        self.variant == DesignVariant::Default
    }
}

/// Macro to implement Focusable for prompts with a `base` field
///
/// This eliminates the need for each prompt to manually implement Focusable.
///
/// # Example
///
/// ```rust,ignore
/// pub struct MyPrompt {
///     pub base: PromptBase,
///     // ...
/// }
///
/// impl_focusable_via_base!(MyPrompt, base);
/// ```
#[macro_export]
macro_rules! impl_focusable_via_base {
    ($ty:ty, $field:ident) => {
        impl gpui::Focusable for $ty {
            fn focus_handle(&self, _cx: &gpui::App) -> gpui::FocusHandle {
                self.$field.focus_handle.clone()
            }
        }
    };
}

// Re-export the macro at module level
// Note: This is infrastructure for prompt implementations - will be used when prompts adopt PromptBase
#[allow(unused_imports)]
pub use impl_focusable_via_base;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_theme() -> Arc<theme::Theme> {
        Arc::new(theme::Theme::default())
    }

    #[test]
    fn test_resolved_colors_default_uses_theme() {
        let theme = make_test_theme();
        let dc = DesignContext::new(&theme, DesignVariant::Default);

        assert_eq!(dc.c.bg_main, theme.colors.background.main);
        assert_eq!(dc.c.text_primary, theme.colors.text.primary);
        assert_eq!(dc.c.accent, theme.colors.accent.selected);
        assert!(dc.is_default());
    }

    #[test]
    fn test_resolved_colors_default_uses_theme_text_on_accent() {
        let mut theme = theme::Theme::default();
        theme.colors.text.primary = 0x010203;
        theme.colors.text.on_accent = 0xa1b2c3;

        let dc = DesignContext::new(&theme, DesignVariant::Default);

        assert_eq!(dc.c.text_on_accent, theme.colors.text.on_accent);
        assert_ne!(dc.c.text_on_accent, theme.colors.text.primary);
    }

    #[test]
    fn test_resolved_colors_minimal_uses_tokens() {
        let theme = make_test_theme();
        let dc = DesignContext::new(&theme, DesignVariant::Minimal);

        let tokens = get_tokens(DesignVariant::Minimal);
        let colors = tokens.colors();

        assert_eq!(dc.c.bg_main, colors.background);
        assert_eq!(dc.c.text_primary, colors.text_primary);
        assert_eq!(dc.c.accent, colors.accent);
        assert!(!dc.is_default());
    }

    #[test]
    fn test_resolved_colors_retro_terminal_uses_tokens() {
        let theme = make_test_theme();
        let dc = DesignContext::new(&theme, DesignVariant::RetroTerminal);

        let tokens = get_tokens(DesignVariant::RetroTerminal);
        let colors = tokens.colors();

        // RetroTerminal has distinctive green colors
        assert_eq!(dc.c.text_primary, colors.text_primary);
        assert_eq!(dc.c.text_primary, 0x00ff00); // Phosphor green
    }

    #[test]
    fn test_design_context_rgba_helpers() {
        let theme = make_test_theme();
        let dc = DesignContext::new(&theme, DesignVariant::Default);

        // The helpers should return Rgba values
        let bg = dc.bg_main();
        let text = dc.text_primary();

        // Just verify they don't panic and return valid Rgba
        assert!(bg.r >= 0.0 && bg.r <= 1.0);
        assert!(text.r >= 0.0 && text.r <= 1.0);
    }

    #[test]
    fn test_all_variants_produce_valid_colors() {
        let theme = make_test_theme();

        for variant in DesignVariant::all() {
            let dc = DesignContext::new(&theme, *variant);

            // All colors should be non-zero (black) unless intentionally so
            // At minimum, text and bg should be different for contrast
            assert_ne!(
                dc.c.bg_main, dc.c.text_primary,
                "Variant {:?} has no contrast between bg and text",
                variant
            );
        }
    }
}

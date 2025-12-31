// macOS Panel Configuration Module
// This module configures GPUI windows as macOS floating panels
// that appear above other applications with native vibrancy effects
//
// Also provides placeholder configuration for input fields

#![allow(dead_code)]

/// Vibrancy configuration for GPUI window background appearance
///
/// GPUI supports three WindowBackgroundAppearance values:
/// - Opaque: Solid, no transparency
/// - Transparent: Fully transparent
/// - Blurred: macOS vibrancy effect (recommended for Spotlight/Raycast-like feel)
///
/// The actual vibrancy effect is achieved through:
/// 1. Setting `WindowBackgroundAppearance::Blurred` in WindowOptions (done in main.rs)
/// 2. Using semi-transparent background colors (controlled by theme opacity settings)
///
/// The blur shows through the transparent portions of the window background,
/// creating the native macOS vibrancy effect similar to Spotlight and Raycast.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum WindowVibrancy {
    /// Solid, opaque background - no vibrancy effect
    Opaque,
    /// Transparent background without blur
    Transparent,
    /// macOS vibrancy/blur effect - the native feel
    /// This is the recommended setting for Spotlight/Raycast-like appearance
    #[default]
    Blurred,
}

impl WindowVibrancy {
    /// Check if this vibrancy setting enables the blur effect
    pub fn is_blurred(&self) -> bool {
        matches!(self, WindowVibrancy::Blurred)
    }

    /// Check if this vibrancy setting is fully opaque
    pub fn is_opaque(&self) -> bool {
        matches!(self, WindowVibrancy::Opaque)
    }
}

#[cfg(target_os = "macos")]
/// Configure the current key window as a floating panel window that appears above other apps.
///
/// This function:
/// - Sets the window level to NSFloatingWindowLevel (3) so it floats above normal windows
/// - Sets collection behavior to MoveToActiveSpace (2) so the window moves to the user's
///   current space when activated, rather than appearing on all spaces simultaneously
///
/// Note: Vibrancy/blur effect is configured via WindowBackgroundAppearance::Blurred
/// in WindowOptions when the window is created (see main.rs).
///
/// Should be called immediately after the window is created and visible.
pub fn configure_as_floating_panel() {
    // This will be called from main.rs where objc macros are available
    // The actual implementation is in main.rs to avoid macro issues in lib code
    crate::logging::log("PANEL", "Panel configuration (implemented in main.rs)");
}

#[cfg(not(target_os = "macos"))]
/// No-op on non-macOS platforms
pub fn configure_as_floating_panel() {}

// ============================================================================
// Input Placeholder Configuration
// ============================================================================

/// Default placeholder text for the main search input
pub const DEFAULT_PLACEHOLDER: &str = "Script Kit";

/// Configuration for input field placeholder behavior
///
/// When using this configuration:
/// - Cursor should be positioned at FAR LEFT (index 0) when input is empty
/// - Placeholder text appears dimmed/muted when no user input
/// - Placeholder disappears immediately when user starts typing
#[derive(Debug, Clone)]
pub struct PlaceholderConfig {
    /// The placeholder text to display when input is empty
    pub text: String,
    /// Whether cursor should appear at left (true) or right (false) of placeholder
    pub cursor_at_left: bool,
}

impl Default for PlaceholderConfig {
    fn default() -> Self {
        Self {
            text: DEFAULT_PLACEHOLDER.to_string(),
            cursor_at_left: true,
        }
    }
}

impl PlaceholderConfig {
    /// Create a new placeholder configuration with custom text
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cursor_at_left: true,
        }
    }

    /// Log when placeholder state changes (for observability)
    pub fn log_state_change(&self, is_showing_placeholder: bool) {
        crate::logging::log(
            "PLACEHOLDER",
            &format!(
                "Placeholder state changed: showing={}, text='{}', cursor_at_left={}",
                is_showing_placeholder, self.text, self.cursor_at_left
            ),
        );
    }

    /// Log cursor position on input focus (for observability)
    pub fn log_cursor_position(&self, position: usize, is_empty: bool) {
        crate::logging::log(
            "PLACEHOLDER",
            &format!(
                "Cursor position on focus: pos={}, input_empty={}, expected_left={}",
                position,
                is_empty,
                is_empty && self.cursor_at_left
            ),
        );
    }
}

// ============================================================================
// Cursor Styling Constants
// ============================================================================

/// Standard cursor width in pixels for text input fields
///
/// This matches the standard cursor width used in editor.rs and provides
/// visual consistency across all input fields.
pub const CURSOR_WIDTH: f32 = 2.0;

/// Horizontal gap between the cursor and adjacent text/placeholder, in pixels.
///
/// Keep this identical in empty and non-empty states to avoid horizontal shifting
/// when switching between placeholder and typed text.
pub const CURSOR_GAP_X: f32 = 2.0;

/// Cursor height for large text (.text_lg() / 18px font)
///
/// This value is calculated to align properly with GPUI's .text_lg() text rendering:
/// - GPUI's text_lg() uses ~18px font size
/// - With natural line height (~1.55), this gives ~28px line height
/// - Cursor should be 18px with 5px top/bottom spacing for vertical centering
///
/// NOTE: This value differs from `font_size_lg * line_height_normal` in design tokens
/// because GPUI's .text_lg() has different line-height than our token calculations.
/// Using this constant ensures proper cursor-text alignment.
pub const CURSOR_HEIGHT_LG: f32 = 18.0;

/// Cursor height for small text (.text_sm() / 12px font)
pub const CURSOR_HEIGHT_SM: f32 = 14.0;

/// Cursor height for medium text (.text_md() / 14px font)
pub const CURSOR_HEIGHT_MD: f32 = 16.0;

/// Vertical margin for cursor centering within text line
///
/// Apply this as `.my(px(CURSOR_MARGIN_Y))` to vertically center the cursor
/// within its text line. This follows the editor.rs pattern.
pub const CURSOR_MARGIN_Y: f32 = 2.0;

/// Configuration for input cursor styling
///
/// Use this struct to ensure consistent cursor appearance across all input fields.
/// The cursor should:
/// 1. Use a fixed height matching the text size (not calculated from design tokens)
/// 2. Use vertical margin for centering within the line
/// 3. Be rendered as an always-present div to prevent layout shift, with bg toggled
#[derive(Debug, Clone, Copy)]
pub struct CursorStyle {
    /// Cursor width in pixels
    pub width: f32,
    /// Cursor height in pixels (should match text size, not line height)
    pub height: f32,
    /// Vertical margin for centering
    pub margin_y: f32,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::large()
    }
}

impl CursorStyle {
    /// Cursor style for large text (.text_lg())
    pub const fn large() -> Self {
        Self {
            width: CURSOR_WIDTH,
            height: CURSOR_HEIGHT_LG,
            margin_y: CURSOR_MARGIN_Y,
        }
    }

    /// Cursor style for medium text (.text_md())
    pub const fn medium() -> Self {
        Self {
            width: CURSOR_WIDTH,
            height: CURSOR_HEIGHT_MD,
            margin_y: CURSOR_MARGIN_Y,
        }
    }

    /// Cursor style for small text (.text_sm())
    pub const fn small() -> Self {
        Self {
            width: CURSOR_WIDTH,
            height: CURSOR_HEIGHT_SM,
            margin_y: CURSOR_MARGIN_Y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_vibrancy() {
        assert_eq!(WindowVibrancy::default(), WindowVibrancy::Blurred);
    }

    #[test]
    fn test_vibrancy_is_blurred() {
        assert!(WindowVibrancy::Blurred.is_blurred());
        assert!(!WindowVibrancy::Opaque.is_blurred());
        assert!(!WindowVibrancy::Transparent.is_blurred());
    }

    #[test]
    fn test_vibrancy_is_opaque() {
        assert!(WindowVibrancy::Opaque.is_opaque());
        assert!(!WindowVibrancy::Blurred.is_opaque());
        assert!(!WindowVibrancy::Transparent.is_opaque());
    }

    // Placeholder configuration tests

    #[test]
    fn test_default_placeholder_text() {
        assert_eq!(DEFAULT_PLACEHOLDER, "Script Kit");
    }

    #[test]
    fn test_placeholder_config_default() {
        let config = PlaceholderConfig::default();
        assert_eq!(config.text, "Script Kit");
        assert!(config.cursor_at_left);
    }

    #[test]
    fn test_placeholder_config_new() {
        let config = PlaceholderConfig::new("Custom Placeholder");
        assert_eq!(config.text, "Custom Placeholder");
        assert!(config.cursor_at_left);
    }

    #[test]
    fn test_placeholder_cursor_at_left_by_default() {
        // Verify that cursor_at_left is true by default
        // This ensures cursor appears at FAR LEFT when input is empty
        let config = PlaceholderConfig::default();
        assert!(
            config.cursor_at_left,
            "Cursor should be at left by default for proper placeholder behavior"
        );
    }

    // Cursor styling tests

    #[test]
    fn test_cursor_width_constant() {
        assert_eq!(CURSOR_WIDTH, 2.0);
    }

    #[test]
    fn test_cursor_height_lg_matches_text_lg() {
        // CURSOR_HEIGHT_LG should be 18px to match GPUI's .text_lg() font size
        // This ensures proper vertical alignment of cursor with text
        assert_eq!(CURSOR_HEIGHT_LG, 18.0);
    }

    #[test]
    fn test_cursor_heights_proportional() {
        // Cursor heights should be proportional to text sizes
        // Use const blocks to satisfy clippy::assertions_on_constants
        const _: () = {
            assert!(CURSOR_HEIGHT_SM < CURSOR_HEIGHT_MD);
        };
        const _: () = {
            assert!(CURSOR_HEIGHT_MD < CURSOR_HEIGHT_LG);
        };
    }

    #[test]
    fn test_cursor_style_default_is_large() {
        let style = CursorStyle::default();
        assert_eq!(style.height, CURSOR_HEIGHT_LG);
        assert_eq!(style.width, CURSOR_WIDTH);
    }

    #[test]
    fn test_cursor_style_constructors() {
        let large = CursorStyle::large();
        assert_eq!(large.height, CURSOR_HEIGHT_LG);

        let medium = CursorStyle::medium();
        assert_eq!(medium.height, CURSOR_HEIGHT_MD);

        let small = CursorStyle::small();
        assert_eq!(small.height, CURSOR_HEIGHT_SM);
    }

    #[test]
    fn test_cursor_margin_constant() {
        // Margin should be 2px for proper vertical centering
        assert_eq!(CURSOR_MARGIN_Y, 2.0);
    }
}

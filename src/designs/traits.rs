#![allow(dead_code)]
//! Design Traits and Tokens
//!
//! This module defines the traits that all design variants must implement:
//! - `DesignRenderer`: For rendering the script list UI
//! - `DesignTokens`: For providing complete design tokens (colors, spacing, typography, visuals)
//!
//! The traits provide a consistent interface while allowing each design to have its own unique visual style.

use gpui::*;

use super::DesignVariant;

// ============================================================================
// Design Token Structs (Copy/Clone for efficient closure use)
// ============================================================================

/// Color tokens for a design variant
///
/// All colors are stored as u32 hex values (0xRRGGBB format).
/// Use `gpui::rgb()` to convert to GPUI colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignColors {
    // Background colors
    /// Primary background color
    pub background: u32,
    /// Secondary/surface background (for cards, panels)
    pub background_secondary: u32,
    /// Tertiary background (for nested elements)
    pub background_tertiary: u32,
    /// Background for selected items
    pub background_selected: u32,
    /// Background for hovered items
    pub background_hover: u32,

    // Text colors
    /// Primary text color (headings, names)
    pub text_primary: u32,
    /// Secondary text color (descriptions, labels)
    pub text_secondary: u32,
    /// Muted text color (placeholders, hints)
    pub text_muted: u32,
    /// Dimmed text color (disabled, inactive)
    pub text_dimmed: u32,
    /// Text color on selected/accent backgrounds
    pub text_on_accent: u32,

    // Accent colors
    /// Primary accent color (selection highlight, links)
    pub accent: u32,
    /// Secondary accent color (buttons, interactive)
    pub accent_secondary: u32,
    /// Success state color
    pub success: u32,
    /// Warning state color
    pub warning: u32,
    /// Error state color
    pub error: u32,

    // Border colors
    /// Primary border color
    pub border: u32,
    /// Subtle/light border color
    pub border_subtle: u32,
    /// Focused element border color
    pub border_focus: u32,

    // Shadow color (with alpha in 0xRRGGBBAA format)
    /// Shadow color (typically black with alpha)
    pub shadow: u32,
}

impl DesignColors {
    /// Combine a hex color (0xRRGGBB) with an alpha value (0-255)
    /// Returns a value suitable for gpui::rgba() in 0xRRGGBBAA format
    #[inline]
    pub fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
        (hex << 8) | (alpha as u32)
    }
}

impl Default for DesignColors {
    fn default() -> Self {
        // Default dark theme colors
        Self {
            background: 0x1e1e1e,
            background_secondary: 0x2d2d30,
            background_tertiary: 0x3c3c3c,
            background_selected: 0x2a2a2a,
            background_hover: 0x323232,

            text_primary: 0xffffff,
            text_secondary: 0xcccccc,
            text_muted: 0x808080,
            text_dimmed: 0x666666,
            text_on_accent: 0x000000,

            accent: 0xfbbf24,           // Script Kit yellow/gold
            accent_secondary: 0xfbbf24, // Same as primary for consistency
            success: 0x00ff00,
            warning: 0xf59e0b,
            error: 0xef4444,

            border: 0x464647,
            border_subtle: 0x3a3a3a,
            border_focus: 0x007acc,

            shadow: 0x00000040,
        }
    }
}

/// Spacing tokens for a design variant
///
/// All values are in pixels (f32). Use `gpui::px()` to convert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignSpacing {
    // Padding variants
    /// Extra small padding (4px)
    pub padding_xs: f32,
    /// Small padding (8px)
    pub padding_sm: f32,
    /// Medium/base padding (12px)
    pub padding_md: f32,
    /// Large padding (16px)
    pub padding_lg: f32,
    /// Extra large padding (24px)
    pub padding_xl: f32,

    // Gap variants (for flexbox)
    /// Small gap between items (4px)
    pub gap_sm: f32,
    /// Medium gap between items (8px)
    pub gap_md: f32,
    /// Large gap between items (16px)
    pub gap_lg: f32,

    // Margin variants
    /// Small margin (4px)
    pub margin_sm: f32,
    /// Medium margin (8px)
    pub margin_md: f32,
    /// Large margin (16px)
    pub margin_lg: f32,

    // Component-specific spacing
    /// Horizontal padding for list items
    pub item_padding_x: f32,
    /// Vertical padding for list items
    pub item_padding_y: f32,
    /// Gap between icon and text in list items
    pub icon_text_gap: f32,
}

impl Default for DesignSpacing {
    fn default() -> Self {
        Self {
            padding_xs: 4.0,
            padding_sm: 8.0,
            padding_md: 12.0,
            padding_lg: 16.0,
            padding_xl: 24.0,

            gap_sm: 4.0,
            gap_md: 8.0,
            gap_lg: 16.0,

            margin_sm: 4.0,
            margin_md: 8.0,
            margin_lg: 16.0,

            item_padding_x: 16.0,
            item_padding_y: 8.0,
            icon_text_gap: 8.0,
        }
    }
}

/// Typography tokens for a design variant
#[derive(Debug, Clone, PartialEq)]
pub struct DesignTypography {
    // Font families
    /// Primary font family (for UI text)
    pub font_family: &'static str,
    /// Monospace font family (for code, terminal)
    pub font_family_mono: &'static str,

    // Font sizes (in pixels)
    /// Extra small text size (10px)
    pub font_size_xs: f32,
    /// Small text size (12px)
    pub font_size_sm: f32,
    /// Base/medium text size (14px)
    pub font_size_md: f32,
    /// Large text size (16px)
    pub font_size_lg: f32,
    /// Extra large text size (20px)
    pub font_size_xl: f32,
    /// Title text size (24px)
    pub font_size_title: f32,

    // Font weights
    /// Thin font weight (100)
    pub font_weight_thin: FontWeight,
    /// Light font weight (300)
    pub font_weight_light: FontWeight,
    /// Normal font weight (400)
    pub font_weight_normal: FontWeight,
    /// Medium font weight (500)
    pub font_weight_medium: FontWeight,
    /// Semibold font weight (600)
    pub font_weight_semibold: FontWeight,
    /// Bold font weight (700)
    pub font_weight_bold: FontWeight,

    // Line heights (as multipliers)
    /// Tight line height (1.2)
    pub line_height_tight: f32,
    /// Normal line height (1.5)
    pub line_height_normal: f32,
    /// Relaxed line height (1.75)
    pub line_height_relaxed: f32,
}

impl Default for DesignTypography {
    fn default() -> Self {
        Self {
            font_family: ".AppleSystemUIFont",
            font_family_mono: "Menlo",

            font_size_xs: 10.0,
            font_size_sm: 12.0,
            font_size_md: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_title: 24.0,

            font_weight_thin: FontWeight::THIN,
            font_weight_light: FontWeight::LIGHT,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::MEDIUM,
            font_weight_semibold: FontWeight::SEMIBOLD,
            font_weight_bold: FontWeight::BOLD,

            line_height_tight: 1.2,
            line_height_normal: 1.5,
            line_height_relaxed: 1.75,
        }
    }
}

// Implement Copy for DesignTypography by storing only static str references
impl Copy for DesignTypography {}

impl DesignTypography {
    /// Calculate the cursor height for text input fields.
    ///
    /// The cursor should be slightly shorter than the line height for visual balance.
    /// This follows the pattern from editor.rs where cursor_height = line_height - 4.0
    ///
    /// # Arguments
    /// * `font_size` - The font size in pixels (e.g., font_size_lg for .text_lg())
    ///
    /// # Returns
    /// The cursor height in pixels, slightly shorter than the line height.
    #[inline]
    pub fn cursor_height_for_font(&self, font_size: f32) -> f32 {
        // Calculate line height based on normal multiplier
        let line_height = font_size * self.line_height_normal;
        // Subtract 4px for visual balance (matches editor.rs pattern)
        // This leaves 2px margin on top and bottom for vertical centering
        (line_height - 4.0).max(12.0) // Minimum 12px for visibility
    }

    /// Calculate cursor height for large text (used with .text_lg())
    ///
    /// GPUI's .text_lg() is approximately 18px font with ~1.55 line height.
    /// Returns a cursor height that aligns properly with GPUI's text rendering.
    #[inline]
    pub fn cursor_height_lg(&self) -> f32 {
        // For GPUI .text_lg() compatibility:
        // - GPUI text_lg is ~18px font size
        // - Natural line height ~28px (1.55 multiplier)
        // - Cursor should be ~20px with 4px margin for centering
        //
        // We use 18px as a good middle ground that works with various line heights
        18.0
    }

    /// Calculate vertical margin for cursor centering within text line
    ///
    /// Returns the top/bottom margin needed to vertically center the cursor.
    #[inline]
    pub fn cursor_margin_y(&self) -> f32 {
        2.0
    }
}

/// Visual effect tokens for a design variant
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignVisual {
    // Border radius variants
    /// No border radius (0px)
    pub radius_none: f32,
    /// Small border radius (4px)
    pub radius_sm: f32,
    /// Medium border radius (8px)
    pub radius_md: f32,
    /// Large border radius (12px)
    pub radius_lg: f32,
    /// Extra large border radius (16px)
    pub radius_xl: f32,
    /// Full/pill border radius (9999px)
    pub radius_full: f32,

    // Shadow properties
    /// Shadow blur radius
    pub shadow_blur: f32,
    /// Shadow spread radius
    pub shadow_spread: f32,
    /// Shadow X offset
    pub shadow_offset_x: f32,
    /// Shadow Y offset
    pub shadow_offset_y: f32,
    /// Shadow opacity (0.0 - 1.0)
    pub shadow_opacity: f32,

    // Opacity variants
    /// Disabled element opacity
    pub opacity_disabled: f32,
    /// Hover state opacity
    pub opacity_hover: f32,
    /// Pressed/active state opacity
    pub opacity_pressed: f32,
    /// Background overlay opacity (for modals, dialogs)
    pub opacity_overlay: f32,

    // Animation durations (ms)
    /// Fast animation (100ms)
    pub animation_fast: u32,
    /// Normal animation (200ms)
    pub animation_normal: u32,
    /// Slow animation (300ms)
    pub animation_slow: u32,

    // Border widths
    /// Thin border (1px)
    pub border_thin: f32,
    /// Normal border (2px)
    pub border_normal: f32,
    /// Thick border (4px)
    pub border_thick: f32,
}

impl Default for DesignVisual {
    fn default() -> Self {
        Self {
            radius_none: 0.0,
            radius_sm: 4.0,
            radius_md: 8.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,

            shadow_blur: 8.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 4.0,
            shadow_opacity: 0.25,

            opacity_disabled: 0.5,
            opacity_hover: 0.8,
            opacity_pressed: 0.6,
            opacity_overlay: 0.5,

            animation_fast: 100,
            animation_normal: 200,
            animation_slow: 300,

            border_thin: 1.0,
            border_normal: 2.0,
            border_thick: 4.0,
        }
    }
}

// ============================================================================
// DesignTokens Trait
// ============================================================================

/// Trait for design token providers
///
/// Each design variant implements this trait to provide its complete set of
/// design tokens. This enables consistent theming across the entire application
/// while allowing each design to have its own unique visual identity.
///
pub trait DesignTokens: Send + Sync {
    /// Get the color tokens for this design
    fn colors(&self) -> DesignColors;

    /// Get the spacing tokens for this design
    fn spacing(&self) -> DesignSpacing;

    /// Get the typography tokens for this design
    fn typography(&self) -> DesignTypography;

    /// Get the visual effect tokens for this design
    fn visual(&self) -> DesignVisual;

    /// Get the list item height for this design (in pixels)
    ///
    /// This is used by uniform_list for virtualization.
    fn item_height(&self) -> f32;

    /// Get the design variant this token set represents
    fn variant(&self) -> DesignVariant;
}

/// Default token implementation for the standard design
#[derive(Debug, Clone, Copy)]
pub struct DefaultDesignTokens;

impl DesignTokens for DefaultDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors::default()
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing::default()
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography::default()
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual::default()
    }

    fn item_height(&self) -> f32 {
        40.0 // Standard list item height matching LIST_ITEM_HEIGHT constant
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Default
    }
}

// ============================================================================
// Design-Specific Token Implementations
// ============================================================================

/// Minimal design tokens
#[derive(Debug, Clone, Copy)]
pub struct MinimalDesignTokens;

impl DesignTokens for MinimalDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            // Same base colors, but minimal uses more transparency
            background: 0x1e1e1e,
            background_secondary: 0x1e1e1e, // Same as bg for minimal look
            background_tertiary: 0x1e1e1e,
            background_selected: 0x1e1e1e, // No visible selection bg
            background_hover: 0x1e1e1e,

            text_primary: 0xffffff,
            text_secondary: 0xcccccc,
            text_muted: 0x808080,
            text_dimmed: 0x666666,
            text_on_accent: 0x000000,

            accent: 0xfbbf24,           // Gold accent for selected text
            accent_secondary: 0xfbbf24, // Same as primary for consistency
            success: 0x00ff00,
            warning: 0xf59e0b,
            error: 0xef4444,

            border: 0x1e1e1e, // No visible borders
            border_subtle: 0x1e1e1e,
            border_focus: 0xfbbf24,

            shadow: 0x00000000, // No shadows
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            // Generous spacing for minimal design
            padding_xs: 8.0,
            padding_sm: 16.0,
            padding_md: 24.0,
            padding_lg: 32.0,
            padding_xl: 48.0,

            gap_sm: 8.0,
            gap_md: 16.0,
            gap_lg: 24.0,

            margin_sm: 8.0,
            margin_md: 16.0,
            margin_lg: 24.0,

            item_padding_x: 80.0, // Very generous horizontal padding
            item_padding_y: 24.0, // Tall items
            icon_text_gap: 16.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: ".AppleSystemUIFont",
            font_family_mono: "Menlo",

            font_size_xs: 10.0,
            font_size_sm: 12.0,
            font_size_md: 16.0, // Larger base for minimal
            font_size_lg: 18.0,
            font_size_xl: 22.0,
            font_size_title: 28.0,

            // Minimal uses thin/light weights
            font_weight_thin: FontWeight::THIN,
            font_weight_light: FontWeight::LIGHT,
            font_weight_normal: FontWeight::THIN, // Default to thin
            font_weight_medium: FontWeight::LIGHT,
            font_weight_semibold: FontWeight::NORMAL,
            font_weight_bold: FontWeight::MEDIUM,

            line_height_tight: 1.3,
            line_height_normal: 1.6,
            line_height_relaxed: 1.8,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            // No rounded corners for minimal
            radius_none: 0.0,
            radius_sm: 0.0,
            radius_md: 0.0,
            radius_lg: 0.0,
            radius_xl: 0.0,
            radius_full: 0.0,

            // No shadows
            shadow_blur: 0.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_opacity: 0.0,

            // Subtle opacity effects
            opacity_disabled: 0.4,
            opacity_hover: 0.8,
            opacity_pressed: 0.6,
            opacity_overlay: 0.3,

            animation_fast: 150,
            animation_normal: 250,
            animation_slow: 350,

            // No visible borders
            border_thin: 0.0,
            border_normal: 0.0,
            border_thick: 0.0,
        }
    }

    fn item_height(&self) -> f32 {
        64.0 // Taller items for minimal
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Minimal
    }
}

/// Retro Terminal design tokens
#[derive(Debug, Clone, Copy)]
pub struct RetroTerminalDesignTokens;

impl DesignTokens for RetroTerminalDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0x000000, // Pure black
            background_secondary: 0x001100,
            background_tertiary: 0x002200,
            background_selected: 0x00ff00, // Inverted: green bg when selected
            background_hover: 0x003300,

            text_primary: 0x00ff00, // Phosphor green
            text_secondary: 0x00cc00,
            text_muted: 0x00aa00,
            text_dimmed: 0x008800,
            text_on_accent: 0x000000, // Black on green

            accent: 0x00ff00,
            accent_secondary: 0x00cc00,
            success: 0x00ff00,
            warning: 0xffff00, // Yellow for terminal warnings
            error: 0xff0000,

            border: 0x00aa00, // Dim green borders
            border_subtle: 0x003300,
            border_focus: 0x00ff00,

            shadow: 0x00ff0040, // Green glow
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            // Dense terminal spacing
            padding_xs: 2.0,
            padding_sm: 4.0,
            padding_md: 8.0,
            padding_lg: 12.0,
            padding_xl: 16.0,

            gap_sm: 2.0,
            gap_md: 4.0,
            gap_lg: 8.0,

            margin_sm: 2.0,
            margin_md: 4.0,
            margin_lg: 8.0,

            item_padding_x: 8.0, // Tight horizontal
            item_padding_y: 4.0, // Dense vertical
            icon_text_gap: 8.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: "Menlo", // Monospace for terminal
            font_family_mono: "Menlo",

            font_size_xs: 10.0,
            font_size_sm: 12.0, // Terminal uses smaller text
            font_size_md: 13.0,
            font_size_lg: 14.0,
            font_size_xl: 16.0,
            font_size_title: 18.0,

            font_weight_thin: FontWeight::NORMAL,
            font_weight_light: FontWeight::NORMAL,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::NORMAL,
            font_weight_semibold: FontWeight::BOLD,
            font_weight_bold: FontWeight::BOLD,

            line_height_tight: 1.1,
            line_height_normal: 1.3,
            line_height_relaxed: 1.5,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            // No rounded corners for terminal
            radius_none: 0.0,
            radius_sm: 0.0,
            radius_md: 0.0,
            radius_lg: 0.0,
            radius_xl: 0.0,
            radius_full: 0.0,

            // Green glow effect
            shadow_blur: 8.0,
            shadow_spread: 2.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_opacity: 0.6,

            opacity_disabled: 0.5,
            opacity_hover: 1.0,
            opacity_pressed: 0.9,
            opacity_overlay: 0.8,

            animation_fast: 0, // Instant for terminal feel
            animation_normal: 0,
            animation_slow: 100,

            border_thin: 1.0,
            border_normal: 1.0,
            border_thick: 2.0,
        }
    }

    fn item_height(&self) -> f32 {
        28.0 // Dense terminal items
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::RetroTerminal
    }
}

/// Glassmorphism design tokens
#[derive(Debug, Clone, Copy)]
pub struct GlassmorphismDesignTokens;

impl DesignTokens for GlassmorphismDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0xffffff20, // White with transparency
            background_secondary: 0xffffff30,
            background_tertiary: 0xffffff40,
            background_selected: 0xffffff50,
            background_hover: 0xffffff40,

            text_primary: 0xffffff,
            text_secondary: 0xffffffcc,
            text_muted: 0xffffff99,
            text_dimmed: 0xffffff66,
            text_on_accent: 0x000000,

            accent: 0x007aff,           // iOS blue
            accent_secondary: 0x5856d6, // iOS purple
            success: 0x34c759,          // iOS green
            warning: 0xff9500,          // iOS orange
            error: 0xff3b30,            // iOS red

            border: 0xffffff30,
            border_subtle: 0xffffff20,
            border_focus: 0x007aff,

            shadow: 0x00000020,
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            padding_xs: 6.0,
            padding_sm: 12.0,
            padding_md: 16.0,
            padding_lg: 20.0,
            padding_xl: 28.0,

            gap_sm: 6.0,
            gap_md: 12.0,
            gap_lg: 20.0,

            margin_sm: 6.0,
            margin_md: 12.0,
            margin_lg: 20.0,

            item_padding_x: 20.0,
            item_padding_y: 14.0,
            icon_text_gap: 12.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography::default()
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            // Large rounded corners for glass effect
            radius_none: 0.0,
            radius_sm: 8.0,
            radius_md: 16.0,
            radius_lg: 24.0,
            radius_xl: 32.0,
            radius_full: 9999.0,

            // Soft shadows
            shadow_blur: 20.0,
            shadow_spread: -2.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 10.0,
            shadow_opacity: 0.2,

            opacity_disabled: 0.4,
            opacity_hover: 0.9,
            opacity_pressed: 0.7,
            opacity_overlay: 0.6,

            animation_fast: 150,
            animation_normal: 300,
            animation_slow: 500,

            border_thin: 1.0,
            border_normal: 1.0,
            border_thick: 2.0,
        }
    }

    fn item_height(&self) -> f32 {
        56.0
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Glassmorphism
    }
}

/// Brutalist design tokens
#[derive(Debug, Clone, Copy)]
pub struct BrutalistDesignTokens;

impl DesignTokens for BrutalistDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0xffffff, // White
            background_secondary: 0xf5f5f5,
            background_tertiary: 0xeeeeee,
            background_selected: 0x000000, // Black selection
            background_hover: 0xf0f0f0,

            text_primary: 0x000000, // Black
            text_secondary: 0x333333,
            text_muted: 0x666666,
            text_dimmed: 0x999999,
            text_on_accent: 0xffffff, // White on black

            accent: 0x000000,
            accent_secondary: 0xff0000, // Red accent
            success: 0x00ff00,
            warning: 0xffff00,
            error: 0xff0000,

            border: 0x000000, // Black borders
            border_subtle: 0x333333,
            border_focus: 0x000000,

            shadow: 0x00000040,
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            padding_xs: 4.0,
            padding_sm: 8.0,
            padding_md: 16.0,
            padding_lg: 24.0,
            padding_xl: 32.0,

            gap_sm: 4.0,
            gap_md: 8.0,
            gap_lg: 16.0,

            margin_sm: 4.0,
            margin_md: 8.0,
            margin_lg: 16.0,

            item_padding_x: 16.0,
            item_padding_y: 12.0,
            icon_text_gap: 12.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: "Helvetica Neue",
            font_family_mono: "Courier",

            font_size_xs: 10.0,
            font_size_sm: 12.0,
            font_size_md: 14.0,
            font_size_lg: 18.0,
            font_size_xl: 24.0,
            font_size_title: 32.0,

            // Bold typography for brutalist
            font_weight_thin: FontWeight::NORMAL,
            font_weight_light: FontWeight::NORMAL,
            font_weight_normal: FontWeight::MEDIUM,
            font_weight_medium: FontWeight::SEMIBOLD,
            font_weight_semibold: FontWeight::BOLD,
            font_weight_bold: FontWeight::BLACK,

            line_height_tight: 1.1,
            line_height_normal: 1.4,
            line_height_relaxed: 1.6,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            // No rounded corners - raw edges
            radius_none: 0.0,
            radius_sm: 0.0,
            radius_md: 0.0,
            radius_lg: 0.0,
            radius_xl: 0.0,
            radius_full: 0.0,

            // Hard shadows
            shadow_blur: 0.0,
            shadow_spread: 0.0,
            shadow_offset_x: 4.0,
            shadow_offset_y: 4.0,
            shadow_opacity: 1.0,

            opacity_disabled: 0.5,
            opacity_hover: 1.0,
            opacity_pressed: 0.9,
            opacity_overlay: 0.9,

            animation_fast: 0, // No animations
            animation_normal: 0,
            animation_slow: 0,

            // Thick borders
            border_thin: 2.0,
            border_normal: 4.0,
            border_thick: 8.0,
        }
    }

    fn item_height(&self) -> f32 {
        40.0 // Standard list item height matching LIST_ITEM_HEIGHT constant
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Brutalist
    }
}

/// Compact design tokens (for power users)
#[derive(Debug, Clone, Copy)]
pub struct CompactDesignTokens;

impl DesignTokens for CompactDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors::default() // Use default colors
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            // Very tight spacing for density
            padding_xs: 2.0,
            padding_sm: 4.0,
            padding_md: 6.0,
            padding_lg: 8.0,
            padding_xl: 12.0,

            gap_sm: 2.0,
            gap_md: 4.0,
            gap_lg: 8.0,

            margin_sm: 2.0,
            margin_md: 4.0,
            margin_lg: 8.0,

            item_padding_x: 8.0,
            item_padding_y: 2.0,
            icon_text_gap: 6.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: ".AppleSystemUIFont",
            font_family_mono: "SF Mono",

            // Smaller text sizes
            font_size_xs: 9.0,
            font_size_sm: 10.0,
            font_size_md: 11.0,
            font_size_lg: 12.0,
            font_size_xl: 14.0,
            font_size_title: 16.0,

            font_weight_thin: FontWeight::LIGHT,
            font_weight_light: FontWeight::LIGHT,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::MEDIUM,
            font_weight_semibold: FontWeight::SEMIBOLD,
            font_weight_bold: FontWeight::BOLD,

            // Tighter line heights
            line_height_tight: 1.1,
            line_height_normal: 1.2,
            line_height_relaxed: 1.3,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            // Small radius
            radius_none: 0.0,
            radius_sm: 2.0,
            radius_md: 4.0,
            radius_lg: 6.0,
            radius_xl: 8.0,
            radius_full: 9999.0,

            // Minimal shadows
            shadow_blur: 2.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 1.0,
            shadow_opacity: 0.15,

            opacity_disabled: 0.5,
            opacity_hover: 0.9,
            opacity_pressed: 0.7,
            opacity_overlay: 0.5,

            animation_fast: 50,
            animation_normal: 100,
            animation_slow: 150,

            border_thin: 1.0,
            border_normal: 1.0,
            border_thick: 2.0,
        }
    }

    fn item_height(&self) -> f32 {
        24.0 // Very compact
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Compact
    }
}

// ============================================================================
// Placeholder implementations for remaining variants
// ============================================================================

/// Neon Cyberpunk design tokens
#[derive(Debug, Clone, Copy)]
pub struct NeonCyberpunkDesignTokens;

impl DesignTokens for NeonCyberpunkDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0x0a0a0f, // Near black
            background_secondary: 0x12121a,
            background_tertiary: 0x1a1a24,
            background_selected: 0x1e1e2e,
            background_hover: 0x16161f,

            text_primary: 0xffffff,
            text_secondary: 0xb0b0d0,
            text_muted: 0x8080a0,
            text_dimmed: 0x606080,
            text_on_accent: 0x000000,

            accent: 0x00ffff,           // Cyan neon
            accent_secondary: 0xff00ff, // Magenta neon
            success: 0x00ff88,
            warning: 0xffaa00,
            error: 0xff0055,

            border: 0x00ffff40, // Neon border with glow
            border_subtle: 0x00ffff20,
            border_focus: 0x00ffff,

            shadow: 0x00ffff30, // Cyan glow
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing::default()
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography::default()
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            radius_none: 0.0,
            radius_sm: 2.0,
            radius_md: 4.0,
            radius_lg: 8.0,
            radius_xl: 12.0,
            radius_full: 9999.0,

            // Neon glow
            shadow_blur: 15.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_opacity: 0.8,

            opacity_disabled: 0.4,
            opacity_hover: 1.0,
            opacity_pressed: 0.8,
            opacity_overlay: 0.7,

            animation_fast: 100,
            animation_normal: 200,
            animation_slow: 300,

            border_thin: 1.0,
            border_normal: 2.0,
            border_thick: 3.0,
        }
    }

    fn item_height(&self) -> f32 {
        34.0 // Compact list item height matching LIST_ITEM_HEIGHT constant
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::NeonCyberpunk
    }
}

/// Paper design tokens
#[derive(Debug, Clone, Copy)]
pub struct PaperDesignTokens;

impl DesignTokens for PaperDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0xfaf8f5, // Warm off-white
            background_secondary: 0xf5f3f0,
            background_tertiary: 0xf0ede8,
            background_selected: 0xe8e5e0,
            background_hover: 0xf0ede8,

            text_primary: 0x2c2825, // Warm dark brown
            text_secondary: 0x4a4540,
            text_muted: 0x78736e,
            text_dimmed: 0xa09a95,
            text_on_accent: 0xffffff,

            accent: 0xc04030,           // Warm red
            accent_secondary: 0x2060a0, // Ink blue
            success: 0x408040,
            warning: 0xc08020,
            error: 0xc04040,

            border: 0xd0ccc5,
            border_subtle: 0xe0dcd5,
            border_focus: 0xc04030,

            shadow: 0x20180010,
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            padding_xs: 6.0,
            padding_sm: 10.0,
            padding_md: 14.0,
            padding_lg: 20.0,
            padding_xl: 28.0,

            gap_sm: 6.0,
            gap_md: 10.0,
            gap_lg: 18.0,

            margin_sm: 6.0,
            margin_md: 10.0,
            margin_lg: 18.0,

            item_padding_x: 18.0,
            item_padding_y: 10.0,
            icon_text_gap: 10.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: "Georgia",
            font_family_mono: "Courier New",

            font_size_xs: 10.0,
            font_size_sm: 12.0,
            font_size_md: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_title: 24.0,

            font_weight_thin: FontWeight::NORMAL,
            font_weight_light: FontWeight::NORMAL,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::MEDIUM,
            font_weight_semibold: FontWeight::SEMIBOLD,
            font_weight_bold: FontWeight::BOLD,

            line_height_tight: 1.3,
            line_height_normal: 1.6,
            line_height_relaxed: 1.8,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            radius_none: 0.0,
            radius_sm: 2.0,
            radius_md: 4.0,
            radius_lg: 6.0,
            radius_xl: 8.0,
            radius_full: 9999.0,

            // Soft paper shadows
            shadow_blur: 12.0,
            shadow_spread: -2.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 4.0,
            shadow_opacity: 0.1,

            opacity_disabled: 0.5,
            opacity_hover: 0.95,
            opacity_pressed: 0.85,
            opacity_overlay: 0.4,

            animation_fast: 150,
            animation_normal: 250,
            animation_slow: 400,

            border_thin: 1.0,
            border_normal: 1.0,
            border_thick: 2.0,
        }
    }

    fn item_height(&self) -> f32 {
        34.0 // Compact list item height matching LIST_ITEM_HEIGHT constant
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Paper
    }
}

/// Apple HIG design tokens
#[derive(Debug, Clone, Copy)]
pub struct AppleHIGDesignTokens;

impl DesignTokens for AppleHIGDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0x1c1c1e, // iOS dark background
            background_secondary: 0x2c2c2e,
            background_tertiary: 0x3a3a3c,
            background_selected: 0x3a3a3c,
            background_hover: 0x2c2c2e,

            text_primary: 0xffffff,
            text_secondary: 0xebebf5, // iOS secondary label
            text_muted: 0x8e8e93,     // iOS tertiary label
            text_dimmed: 0x636366,    // iOS quaternary label
            text_on_accent: 0xffffff,

            accent: 0x0a84ff,           // iOS blue
            accent_secondary: 0x5e5ce6, // iOS indigo
            success: 0x30d158,          // iOS green
            warning: 0xff9f0a,          // iOS orange
            error: 0xff453a,            // iOS red

            border: 0x38383a,
            border_subtle: 0x2c2c2e,
            border_focus: 0x0a84ff,

            shadow: 0x00000040,
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            padding_xs: 4.0,
            padding_sm: 8.0,
            padding_md: 12.0,
            padding_lg: 16.0,
            padding_xl: 20.0,

            gap_sm: 4.0,
            gap_md: 8.0,
            gap_lg: 12.0,

            margin_sm: 4.0,
            margin_md: 8.0,
            margin_lg: 16.0,

            item_padding_x: 16.0,
            item_padding_y: 11.0,
            icon_text_gap: 12.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: ".AppleSystemUIFont",
            font_family_mono: "SF Mono",

            font_size_xs: 11.0,
            font_size_sm: 13.0,
            font_size_md: 15.0, // iOS body
            font_size_lg: 17.0, // iOS headline
            font_size_xl: 20.0,
            font_size_title: 28.0, // iOS title1

            font_weight_thin: FontWeight::THIN,
            font_weight_light: FontWeight::LIGHT,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::MEDIUM,
            font_weight_semibold: FontWeight::SEMIBOLD,
            font_weight_bold: FontWeight::BOLD,

            line_height_tight: 1.2,
            line_height_normal: 1.4,
            line_height_relaxed: 1.6,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            radius_none: 0.0,
            radius_sm: 6.0,
            radius_md: 10.0, // iOS standard
            radius_lg: 14.0,
            radius_xl: 20.0,
            radius_full: 9999.0,

            shadow_blur: 10.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 4.0,
            shadow_opacity: 0.3,

            opacity_disabled: 0.38, // iOS disabled
            opacity_hover: 0.85,
            opacity_pressed: 0.75,
            opacity_overlay: 0.5,

            animation_fast: 150,
            animation_normal: 250,
            animation_slow: 350,

            border_thin: 0.5, // iOS uses hairline borders
            border_normal: 1.0,
            border_thick: 2.0,
        }
    }

    fn item_height(&self) -> f32 {
        44.0 // iOS standard row height
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::AppleHIG
    }
}

/// Material Design 3 tokens
#[derive(Debug, Clone, Copy)]
pub struct Material3DesignTokens;

impl DesignTokens for Material3DesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0x1c1b1f, // M3 dark surface
            background_secondary: 0x2b2930,
            background_tertiary: 0x36343b,
            background_selected: 0x4f378b, // M3 primary container
            background_hover: 0x36343b,

            text_primary: 0xe6e1e5,   // M3 on-surface
            text_secondary: 0xcac4d0, // M3 on-surface-variant
            text_muted: 0x938f99,
            text_dimmed: 0x79747e,
            text_on_accent: 0xeaddff, // M3 on-primary-container

            accent: 0xd0bcff,           // M3 primary
            accent_secondary: 0xccc2dc, // M3 secondary
            success: 0xa5d6a7,
            warning: 0xffcc80,
            error: 0xf2b8b5, // M3 error

            border: 0x49454f, // M3 outline
            border_subtle: 0x36343b,
            border_focus: 0xd0bcff,

            shadow: 0x00000040,
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            padding_xs: 4.0,
            padding_sm: 8.0,
            padding_md: 12.0,
            padding_lg: 16.0,
            padding_xl: 24.0,

            gap_sm: 4.0,
            gap_md: 8.0,
            gap_lg: 16.0,

            margin_sm: 4.0,
            margin_md: 8.0,
            margin_lg: 16.0,

            item_padding_x: 16.0,
            item_padding_y: 12.0,
            icon_text_gap: 16.0, // M3 uses larger icon gaps
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: ".AppleSystemUIFont", // Would be Roboto on Android
            font_family_mono: "Roboto Mono",

            font_size_xs: 11.0,
            font_size_sm: 12.0, // M3 label-small
            font_size_md: 14.0, // M3 body-medium
            font_size_lg: 16.0, // M3 title-medium
            font_size_xl: 22.0, // M3 headline-small
            font_size_title: 28.0,

            font_weight_thin: FontWeight::THIN,
            font_weight_light: FontWeight::LIGHT,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::MEDIUM,
            font_weight_semibold: FontWeight::MEDIUM, // M3 uses medium more
            font_weight_bold: FontWeight::BOLD,

            line_height_tight: 1.2,
            line_height_normal: 1.5,
            line_height_relaxed: 1.75,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            radius_none: 0.0,
            radius_sm: 8.0,
            radius_md: 12.0, // M3 uses larger radius
            radius_lg: 16.0,
            radius_xl: 28.0, // M3 extra-large
            radius_full: 9999.0,

            // M3 elevation shadows
            shadow_blur: 8.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 2.0,
            shadow_opacity: 0.3,

            opacity_disabled: 0.38, // M3 standard
            opacity_hover: 0.08,    // M3 state layer
            opacity_pressed: 0.12,
            opacity_overlay: 0.5,

            animation_fast: 100,
            animation_normal: 200,
            animation_slow: 300,

            border_thin: 1.0,
            border_normal: 1.0,
            border_thick: 2.0,
        }
    }

    fn item_height(&self) -> f32 {
        56.0 // M3 list item height
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Material3
    }
}

/// Playful design tokens
#[derive(Debug, Clone, Copy)]
pub struct PlayfulDesignTokens;

impl DesignTokens for PlayfulDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors {
            background: 0xfef3e2, // Warm cream
            background_secondary: 0xfff8ed,
            background_tertiary: 0xffffff,
            background_selected: 0xffe5b4, // Peach
            background_hover: 0xfff0d4,

            text_primary: 0x2d1b4e, // Deep purple
            text_secondary: 0x4a3a6d,
            text_muted: 0x7a6a9d,
            text_dimmed: 0xa09ac0,
            text_on_accent: 0xffffff,

            accent: 0xff6b6b,           // Coral
            accent_secondary: 0x4ecdc4, // Teal
            success: 0x2ecc71,
            warning: 0xf39c12,
            error: 0xe74c3c,

            border: 0xe0d0c0,
            border_subtle: 0xf0e8e0,
            border_focus: 0xff6b6b,

            shadow: 0x2d1b4e20,
        }
    }

    fn spacing(&self) -> DesignSpacing {
        DesignSpacing {
            padding_xs: 6.0,
            padding_sm: 10.0,
            padding_md: 14.0,
            padding_lg: 20.0,
            padding_xl: 28.0,

            gap_sm: 6.0,
            gap_md: 10.0,
            gap_lg: 18.0,

            margin_sm: 6.0,
            margin_md: 10.0,
            margin_lg: 18.0,

            item_padding_x: 20.0,
            item_padding_y: 12.0,
            icon_text_gap: 12.0,
        }
    }

    fn typography(&self) -> DesignTypography {
        DesignTypography {
            font_family: ".AppleSystemUIFont",
            font_family_mono: "Menlo",

            font_size_xs: 10.0,
            font_size_sm: 12.0,
            font_size_md: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_title: 24.0,

            font_weight_thin: FontWeight::LIGHT,
            font_weight_light: FontWeight::LIGHT,
            font_weight_normal: FontWeight::NORMAL,
            font_weight_medium: FontWeight::MEDIUM,
            font_weight_semibold: FontWeight::SEMIBOLD,
            font_weight_bold: FontWeight::BOLD,

            line_height_tight: 1.2,
            line_height_normal: 1.5,
            line_height_relaxed: 1.75,
        }
    }

    fn visual(&self) -> DesignVisual {
        DesignVisual {
            // Very rounded for playful feel
            radius_none: 0.0,
            radius_sm: 8.0,
            radius_md: 16.0,
            radius_lg: 24.0,
            radius_xl: 32.0,
            radius_full: 9999.0,

            // Colorful soft shadows
            shadow_blur: 16.0,
            shadow_spread: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 6.0,
            shadow_opacity: 0.15,

            opacity_disabled: 0.5,
            opacity_hover: 0.95,
            opacity_pressed: 0.85,
            opacity_overlay: 0.4,

            // Bouncy animations
            animation_fast: 150,
            animation_normal: 300,
            animation_slow: 450,

            border_thin: 2.0,
            border_normal: 3.0,
            border_thick: 4.0,
        }
    }

    fn item_height(&self) -> f32 {
        56.0
    }

    fn variant(&self) -> DesignVariant {
        DesignVariant::Playful
    }
}

// ============================================================================
// Boxed Token Type for Dynamic Dispatch
// ============================================================================

/// Type alias for boxed design tokens (for dynamic dispatch)
pub type DesignTokensBox = Box<dyn DesignTokens>;

/// Trait for design renderers
///
/// Each design variant implements this trait to provide its own rendering
/// of the script list UI. The trait is designed to work with GPUI's
/// component model and follows the existing patterns in the codebase.
///
/// # Type Parameters
///
/// * `App` - The application type that this renderer works with
///
/// # Implementation Notes
///
/// - Use `AnyElement` as the return type to allow flexible element trees
/// - Access app state through the provided app reference
/// - Follow the project's theme system
/// - Use `LIST_ITEM_HEIGHT` (34.0) for consistent item sizing
pub trait DesignRenderer<App>: Send + Sync {
    /// Render the script list in this design's style
    ///
    /// This method should return a complete script list UI element
    /// that can be composed into the main application view.
    ///
    /// # Arguments
    ///
    /// * `app` - Reference to the app for accessing state
    /// * `cx` - GPUI context for creating elements and handling events
    ///
    /// # Returns
    ///
    /// An `AnyElement` containing the rendered script list.
    fn render_script_list(&self, app: &App, cx: &mut Context<App>) -> AnyElement;

    /// Get the variant this renderer implements
    fn variant(&self) -> DesignVariant;

    /// Get the display name for this design
    fn name(&self) -> &'static str {
        self.variant().name()
    }

    /// Get a description of this design
    fn description(&self) -> &'static str {
        self.variant().description()
    }
}

/// Type alias for boxed design renderers
///
/// Use this when storing or passing design renderers as trait objects.
pub type DesignRendererBox<App> = Box<dyn DesignRenderer<App>>;

// Note: Tests for DesignTypography cursor methods are validated at compile time
// through usage in src/panel.rs (CursorStyle) and editor.rs patterns.
// The cursor_height_lg() returns 18.0 for GPUI .text_lg() compatibility.
// The cursor_margin_y() returns 2.0 for vertical centering (matching editor.rs pattern).

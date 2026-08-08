use super::tokens::{
    AppleHIGDesignTokens, BrutalistDesignTokens, CompactDesignTokens, DefaultDesignTokens,
    GlassmorphismDesignTokens, Material3DesignTokens, MinimalDesignTokens,
    NeonCyberpunkDesignTokens, PaperDesignTokens, PlayfulDesignTokens, RetroTerminalDesignTokens,
};

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
    ///
    /// GOV-003: delegates to the typed `theme::alpha::pack_rgb_alpha` owner.
    /// The `u8` arity is TRANSITIONAL for pre-existing callers outside this
    /// lane's owned paths (`src/actions/dialog.rs` + its tests); new code
    /// should call [`Self::hex_with_alpha_byte`] with an authored
    /// [`crate::theme::AlphaByte`].
    #[inline]
    pub fn hex_with_alpha(hex: u32, alpha: u8) -> u32 {
        Self::hex_with_alpha_byte(hex, crate::theme::AlphaByte::authored(alpha))
    }

    /// Typed packing boundary: an already-quantized authored alpha byte.
    #[inline]
    pub fn hex_with_alpha_byte(hex: u32, alpha: crate::theme::AlphaByte) -> u32 {
        crate::theme::alpha::pack_rgb_alpha(hex, alpha)
    }
}

impl Default for DesignColors {
    fn default() -> Self {
        // Default dark theme colors
        Self {
            background: 0x1e1e1e,
            background_secondary: 0x2d2d30,
            background_tertiary: 0x3c3c3c,
            background_selected: 0xffffff, // White - subtle brightening like Raycast
            background_hover: 0xffffff,    // White - barely visible hover

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

pub(crate) trait DesignColorTokens {
    fn colors(&self) -> DesignColors;
}

impl DesignColorTokens for DefaultDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors::default()
    }
}

impl DesignColorTokens for MinimalDesignTokens {
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
}

impl DesignColorTokens for RetroTerminalDesignTokens {
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
}

impl DesignColorTokens for GlassmorphismDesignTokens {
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
}

impl DesignColorTokens for BrutalistDesignTokens {
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
}

impl DesignColorTokens for CompactDesignTokens {
    fn colors(&self) -> DesignColors {
        DesignColors::default() // Use default colors
    }
}

impl DesignColorTokens for NeonCyberpunkDesignTokens {
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
}

impl DesignColorTokens for PaperDesignTokens {
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
}

impl DesignColorTokens for AppleHIGDesignTokens {
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
}

impl DesignColorTokens for Material3DesignTokens {
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
}

impl DesignColorTokens for PlayfulDesignTokens {
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
}

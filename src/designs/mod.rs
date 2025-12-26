#![allow(unused_imports)]

//! Design System Module
//!
//! This module provides a pluggable design system for the script list UI.
//! Each design variant implements the `DesignRenderer` trait to provide
//! its own visual style while maintaining the same functionality.
//!
//! # Usage
//! ```ignore
//! use designs::{DesignVariant, uses_default_renderer};
//!
//! // Check if we should use the default implementation
//! if !uses_default_renderer(variant) {
//!     // Use custom renderer (when implemented)
//! }
//! ```

mod traits;
mod minimal;
mod glassmorphism;
pub mod brutalist;
pub mod compact;
pub mod material3;
pub mod retro_terminal;
pub mod playful;
pub mod paper;
pub mod apple_hig;
pub mod neon_cyberpunk;

// Re-export the trait and types
pub use traits::{DesignRenderer, DesignRendererBox};
pub use minimal::{MinimalRenderer, MinimalColors, MinimalConstants, render_minimal_search_bar, render_minimal_empty_state, render_minimal_list, MINIMAL_ITEM_HEIGHT};
pub use glassmorphism::GlassmorphismRenderer;
pub use brutalist::{BrutalistRenderer, BrutalistColors, render_brutalist_list};
pub use compact::{CompactRenderer, CompactListItem, COMPACT_ITEM_HEIGHT};
pub use material3::Material3Renderer;
pub use retro_terminal::{RetroTerminalRenderer, TerminalColors, TERMINAL_ITEM_HEIGHT};
pub use playful::PlayfulRenderer;
pub use paper::PaperRenderer;
pub use apple_hig::{AppleHIGRenderer, ITEM_HEIGHT as APPLE_HIG_ITEM_HEIGHT};
pub use neon_cyberpunk::NeonCyberpunkRenderer;

/// Design variant enumeration
///
/// Each variant represents a distinct visual style for the script list.
/// Use `Cmd+1` through `Cmd+0` to switch between designs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DesignVariant {
    /// Default design (uses existing implementation)
    /// Keyboard: Cmd+1
    #[default]
    Default = 1,

    /// Minimal design with reduced visual elements
    /// Keyboard: Cmd+2
    Minimal = 2,

    /// Retro terminal aesthetic with monospace fonts and green-on-black
    /// Keyboard: Cmd+3
    RetroTerminal = 3,

    /// Glassmorphism with frosted glass effects and transparency
    /// Keyboard: Cmd+4
    Glassmorphism = 4,

    /// Brutalist design with raw, bold typography
    /// Keyboard: Cmd+5
    Brutalist = 5,

    /// Neon cyberpunk with glowing accents and dark backgrounds
    /// Keyboard: Cmd+6
    NeonCyberpunk = 6,

    /// Paper-like design with warm tones and subtle shadows
    /// Keyboard: Cmd+7
    Paper = 7,

    /// Apple Human Interface Guidelines inspired design
    /// Keyboard: Cmd+8
    AppleHIG = 8,

    /// Material Design 3 (Material You) inspired design
    /// Keyboard: Cmd+9
    Material3 = 9,

    /// Compact design with smaller items for power users
    /// Keyboard: Cmd+0
    Compact = 10,

    /// Playful design with rounded corners and vibrant colors
    /// Not directly accessible via keyboard shortcut
    Playful = 11,
}

impl DesignVariant {
    /// Get all available design variants
    #[allow(dead_code)]
    pub fn all() -> &'static [DesignVariant] {
        &[
            DesignVariant::Default,
            DesignVariant::Minimal,
            DesignVariant::RetroTerminal,
            DesignVariant::Glassmorphism,
            DesignVariant::Brutalist,
            DesignVariant::NeonCyberpunk,
            DesignVariant::Paper,
            DesignVariant::AppleHIG,
            DesignVariant::Material3,
            DesignVariant::Compact,
            DesignVariant::Playful,
        ]
    }

    /// Get the display name for this variant
    pub fn name(&self) -> &'static str {
        match self {
            DesignVariant::Default => "Default",
            DesignVariant::Minimal => "Minimal",
            DesignVariant::RetroTerminal => "Retro Terminal",
            DesignVariant::Glassmorphism => "Glassmorphism",
            DesignVariant::Brutalist => "Brutalist",
            DesignVariant::NeonCyberpunk => "Neon Cyberpunk",
            DesignVariant::Paper => "Paper",
            DesignVariant::AppleHIG => "Apple HIG",
            DesignVariant::Material3 => "Material 3",
            DesignVariant::Compact => "Compact",
            DesignVariant::Playful => "Playful",
        }
    }

    /// Get the keyboard shortcut number for this variant (1-10, where 0 = 10)
    #[allow(dead_code)]
    pub fn shortcut_number(&self) -> Option<u8> {
        match self {
            DesignVariant::Default => Some(1),
            DesignVariant::Minimal => Some(2),
            DesignVariant::RetroTerminal => Some(3),
            DesignVariant::Glassmorphism => Some(4),
            DesignVariant::Brutalist => Some(5),
            DesignVariant::NeonCyberpunk => Some(6),
            DesignVariant::Paper => Some(7),
            DesignVariant::AppleHIG => Some(8),
            DesignVariant::Material3 => Some(9),
            DesignVariant::Compact => Some(0), // Cmd+0 maps to 10
            DesignVariant::Playful => None,    // No direct shortcut
        }
    }

    /// Create a variant from a keyboard number (1-9, 0 for 10)
    pub fn from_keyboard_number(num: u8) -> Option<DesignVariant> {
        match num {
            1 => Some(DesignVariant::Default),
            2 => Some(DesignVariant::Minimal),
            3 => Some(DesignVariant::RetroTerminal),
            4 => Some(DesignVariant::Glassmorphism),
            5 => Some(DesignVariant::Brutalist),
            6 => Some(DesignVariant::NeonCyberpunk),
            7 => Some(DesignVariant::Paper),
            8 => Some(DesignVariant::AppleHIG),
            9 => Some(DesignVariant::Material3),
            0 => Some(DesignVariant::Compact),
            _ => None,
        }
    }

    /// Get a short description of this design variant
    pub fn description(&self) -> &'static str {
        match self {
            DesignVariant::Default => "The standard Script Kit appearance",
            DesignVariant::Minimal => "Clean and minimal with reduced visual noise",
            DesignVariant::RetroTerminal => "Classic terminal aesthetics with green phosphor glow",
            DesignVariant::Glassmorphism => "Frosted glass effects with soft transparency",
            DesignVariant::Brutalist => "Bold, raw typography with strong contrast",
            DesignVariant::NeonCyberpunk => "Dark backgrounds with vibrant neon accents",
            DesignVariant::Paper => "Warm, paper-like tones with subtle textures",
            DesignVariant::AppleHIG => "Following Apple Human Interface Guidelines",
            DesignVariant::Material3 => "Google Material Design 3 (Material You) inspired",
            DesignVariant::Compact => "Dense layout for power users with many scripts",
            DesignVariant::Playful => "Fun, rounded design with vibrant colors",
        }
    }
}

/// Check if a variant uses the default renderer
///
/// When true, ScriptListApp should use its built-in render_script_list()
/// instead of delegating to a custom design renderer.
///
/// Currently all variants use the default renderer until custom implementations
/// are added. In the future, only DesignVariant::Default will return true here.
pub fn uses_default_renderer(variant: DesignVariant) -> bool {
    // When a custom renderer is added for a variant, remove it from this match
    // Minimal, RetroTerminal now have custom renderers
    matches!(
        variant,
        DesignVariant::Default
            | DesignVariant::Glassmorphism
            | DesignVariant::Brutalist
            | DesignVariant::NeonCyberpunk
            | DesignVariant::Paper
            | DesignVariant::AppleHIG
            | DesignVariant::Material3
            | DesignVariant::Compact
            | DesignVariant::Playful
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_count() {
        assert_eq!(DesignVariant::all().len(), 11);
    }

    #[test]
    fn test_keyboard_number_round_trip() {
        for num in 0..=9 {
            let variant = DesignVariant::from_keyboard_number(num);
            assert!(variant.is_some(), "Keyboard number {} should map to a variant", num);
            
            let v = variant.unwrap();
            let shortcut = v.shortcut_number();
            
            // All variants except Playful should have shortcuts
            if v != DesignVariant::Playful {
                assert!(shortcut.is_some(), "Variant {:?} should have a shortcut", v);
                assert_eq!(shortcut.unwrap(), num, "Round-trip failed for number {}", num);
            }
        }
    }

    #[test]
    fn test_playful_has_no_shortcut() {
        assert_eq!(DesignVariant::Playful.shortcut_number(), None);
    }

    #[test]
    fn test_variant_names_not_empty() {
        for variant in DesignVariant::all() {
            assert!(!variant.name().is_empty(), "Variant {:?} should have a name", variant);
            assert!(!variant.description().is_empty(), "Variant {:?} should have a description", variant);
        }
    }

    #[test]
    fn test_default_variant() {
        assert_eq!(DesignVariant::default(), DesignVariant::Default);
    }
    
    #[test]
    fn test_uses_default_renderer() {
        // Minimal and RetroTerminal now have custom renderers
        assert!(!uses_default_renderer(DesignVariant::Minimal), "Minimal should NOT use default renderer");
        assert!(!uses_default_renderer(DesignVariant::RetroTerminal), "RetroTerminal should NOT use default renderer");
        
        // Default still uses default renderer
        assert!(uses_default_renderer(DesignVariant::Default), "Default should use default renderer");
        
        // Other variants still use default renderer (until implemented)
        assert!(uses_default_renderer(DesignVariant::Brutalist));
        assert!(uses_default_renderer(DesignVariant::NeonCyberpunk));
    }
}

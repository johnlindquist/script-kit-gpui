#![allow(unused_imports)]

//! Design System Module
//!
//! This module provides a pluggable design system for the script list UI.
//! Each design variant implements the `DesignRenderer` trait to provide
//! its own visual style while maintaining the same functionality.
//!

pub mod apple_hig;
pub mod brutalist;
pub mod compact;
mod glassmorphism;
pub mod group_header_variations;
pub mod icon_variations;
pub mod material3;
mod minimal;
pub mod neon_cyberpunk;
pub mod paper;
pub mod playful;
pub mod retro_terminal;
pub mod separator_variations;
mod traits;

// Re-export the trait and types
pub use apple_hig::{
    render_apple_hig_header, render_apple_hig_log_panel, render_apple_hig_preview_panel,
    render_apple_hig_window_container, AppleHIGRenderer, ITEM_HEIGHT as APPLE_HIG_ITEM_HEIGHT,
};
pub use brutalist::{
    render_brutalist_header, render_brutalist_list, render_brutalist_log_panel,
    render_brutalist_preview_panel, render_brutalist_window_container, BrutalistColors,
    BrutalistRenderer,
};
pub use compact::{
    render_compact_header, render_compact_log_panel, render_compact_preview_panel,
    render_compact_window_container, CompactListItem, CompactRenderer, COMPACT_ITEM_HEIGHT,
};
pub use glassmorphism::{
    render_glassmorphism_header, render_glassmorphism_log_panel,
    render_glassmorphism_preview_panel, render_glassmorphism_window_container, GlassColors,
    GlassmorphismRenderer,
};
pub use material3::{
    render_material3_header, render_material3_log_panel, render_material3_preview_panel,
    render_material3_window_container, Material3Renderer,
};
pub use minimal::{
    render_minimal_action_button, render_minimal_divider, render_minimal_empty_state,
    render_minimal_header, render_minimal_list, render_minimal_log_panel,
    render_minimal_preview_panel, render_minimal_search_bar, render_minimal_status,
    render_minimal_window_container, MinimalColors, MinimalConstants, MinimalRenderer,
    MinimalWindowConfig, MINIMAL_ITEM_HEIGHT,
};
pub use neon_cyberpunk::{
    render_neon_cyberpunk_header, render_neon_cyberpunk_log_panel,
    render_neon_cyberpunk_preview_panel, render_neon_cyberpunk_window_container,
    NeonCyberpunkRenderer,
};
pub use paper::{
    render_paper_header, render_paper_log_panel, render_paper_preview_panel,
    render_paper_window_container, PaperRenderer,
};
pub use playful::{
    render_playful_header, render_playful_log_panel, render_playful_preview_panel,
    render_playful_window_container, PlayfulColors, PlayfulRenderer,
};
pub use retro_terminal::{RetroTerminalRenderer, TerminalColors, TERMINAL_ITEM_HEIGHT};
pub use traits::{
    AppleHIGDesignTokens, BrutalistDesignTokens, CompactDesignTokens, DefaultDesignTokens,
    DesignColors, DesignSpacing, DesignTokens, DesignTokensBox, DesignTypography, DesignVisual,
    GlassmorphismDesignTokens, Material3DesignTokens, MinimalDesignTokens,
    NeonCyberpunkDesignTokens, PaperDesignTokens, PlayfulDesignTokens, RetroTerminalDesignTokens,
};
pub use traits::{DesignRenderer, DesignRendererBox};

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

    /// Get the next design variant in the cycle
    ///
    /// Cycles through all designs: Default -> Minimal -> RetroTerminal -> ... -> Playful -> Default
    pub fn next(self) -> DesignVariant {
        let all = Self::all();
        let current_idx = all.iter().position(|&v| v == self).unwrap_or(0);
        let next_idx = (current_idx + 1) % all.len();
        all[next_idx]
    }

    /// Get the previous design variant in the cycle
    #[allow(dead_code)]
    pub fn prev(self) -> DesignVariant {
        let all = Self::all();
        let current_idx = all.iter().position(|&v| v == self).unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            all.len() - 1
        } else {
            current_idx - 1
        };
        all[prev_idx]
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
    #[allow(dead_code)]
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
#[allow(dead_code)]
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

/// Get the item height for a design variant
///
/// Different designs use different item heights for their aesthetic.
/// This should be used when setting up uniform_list.
///
/// Note: This function now uses the DesignTokens system for consistency.
/// The constants MINIMAL_ITEM_HEIGHT, TERMINAL_ITEM_HEIGHT, etc. are
/// kept for backward compatibility with existing renderers.
#[allow(dead_code)]
pub fn get_item_height(variant: DesignVariant) -> f32 {
    // Use tokens for authoritative item heights
    get_tokens(variant).item_height()
}

/// Get design tokens for a design variant
///
/// Returns a boxed trait object that provides the complete design token set
/// for the specified variant. Use this when you need dynamic dispatch.
///
pub fn get_tokens(variant: DesignVariant) -> Box<dyn DesignTokens> {
    match variant {
        DesignVariant::Default => Box::new(DefaultDesignTokens),
        DesignVariant::Minimal => Box::new(MinimalDesignTokens),
        DesignVariant::RetroTerminal => Box::new(RetroTerminalDesignTokens),
        DesignVariant::Glassmorphism => Box::new(GlassmorphismDesignTokens),
        DesignVariant::Brutalist => Box::new(BrutalistDesignTokens),
        DesignVariant::NeonCyberpunk => Box::new(NeonCyberpunkDesignTokens),
        DesignVariant::Paper => Box::new(PaperDesignTokens),
        DesignVariant::AppleHIG => Box::new(AppleHIGDesignTokens),
        DesignVariant::Material3 => Box::new(Material3DesignTokens),
        DesignVariant::Compact => Box::new(CompactDesignTokens),
        DesignVariant::Playful => Box::new(PlayfulDesignTokens),
    }
}

/// Get design tokens for a design variant (static dispatch version)
///
/// Returns the concrete token type for the specified variant.
/// Use this when you know the variant at compile time for better performance.
///
#[allow(dead_code)]
pub fn get_tokens_static<T: DesignTokens + Copy + Default>() -> T {
    T::default()
}

use crate::list_item::ListItemColors;
use crate::scripts::SearchResult;
use gpui::{AnyElement, IntoElement};

/// Render a single list item for the given design variant
///
/// This is the main dispatch function for design-specific item rendering.
/// It renders a single item based on the current design, with proper styling.
///
/// # Arguments
/// * `variant` - The design variant to render
/// * `result` - The search result to render
/// * `index` - The item index (for element ID and alternating styles)
/// * `is_selected` - Whether this item is currently selected (full focus styling)
/// * `is_hovered` - Whether this item is currently hovered (subtle visual feedback)
/// * `list_colors` - Pre-computed theme colors for the default design
///
/// # Returns
/// An `AnyElement` containing the rendered item
pub fn render_design_item(
    variant: DesignVariant,
    result: &SearchResult,
    index: usize,
    is_selected: bool,
    is_hovered: bool,
    list_colors: ListItemColors,
) -> AnyElement {
    crate::logging::log_debug(
        "DESIGN",
        &format!(
            "Rendering item {} with design {:?}, selected={}, hovered={}",
            index, variant, is_selected, is_hovered
        ),
    );

    match variant {
        DesignVariant::Minimal => {
            let colors = MinimalColors {
                text_primary: list_colors.text_primary,
                text_muted: list_colors.text_muted,
                accent_selected: list_colors.accent_selected,
                background: list_colors.background,
            };
            MinimalRenderer::new()
                .render_item(result, index, is_selected, colors)
                .into_any_element()
        }
        DesignVariant::RetroTerminal => RetroTerminalRenderer::new()
            .render_item(result, index, is_selected)
            .into_any_element(),
        // All other variants use the default ListItem renderer
        _ => {
            use crate::list_item::{IconKind, ListItem};

            // Extract name, description, shortcut, and icon based on result type
            let (name, description, shortcut, icon_kind) = match result {
                SearchResult::Script(sm) => {
                    // Use script's icon metadata if present, otherwise default to "Code" SVG
                    let icon = match &sm.script.icon {
                        Some(icon_name) => IconKind::Svg(icon_name.clone()),
                        None => IconKind::Svg("Code".to_string()),
                    };
                    (
                        sm.script.name.clone(),
                        sm.script.description.clone(),
                        None,
                        Some(icon),
                    )
                }
                SearchResult::Scriptlet(sm) => {
                    // Scriptlets use BoltFilled SVG for quick actions
                    (
                        sm.scriptlet.name.clone(),
                        sm.scriptlet.description.clone(),
                        sm.scriptlet.shortcut.clone(),
                        Some(IconKind::Svg("BoltFilled".to_string())),
                    )
                }
                SearchResult::BuiltIn(bm) => {
                    // Built-ins: try to map their icon to SVG, fallback to Settings
                    let icon = match &bm.entry.icon {
                        Some(emoji) => {
                            // Try to infer SVG from common emoji patterns
                            match emoji.as_str() {
                                "⚙️" | "🔧" => IconKind::Svg("Settings".to_string()),
                                "📋" => IconKind::Svg("Copy".to_string()),
                                "🔍" | "🔎" => IconKind::Svg("MagnifyingGlass".to_string()),
                                "📁" => IconKind::Svg("Folder".to_string()),
                                "🖥️" | "💻" => IconKind::Svg("Terminal".to_string()),
                                "⚡" | "🔥" => IconKind::Svg("BoltFilled".to_string()),
                                "⭐" | "🌟" => IconKind::Svg("StarFilled".to_string()),
                                "✓" | "✅" => IconKind::Svg("Check".to_string()),
                                "▶️" | "🎬" => IconKind::Svg("PlayFilled".to_string()),
                                "🗑️" => IconKind::Svg("Trash".to_string()),
                                "➕" => IconKind::Svg("Plus".to_string()),
                                _ => IconKind::Svg("Settings".to_string()),
                            }
                        }
                        None => IconKind::Svg("Settings".to_string()),
                    };
                    (
                        bm.entry.name.clone(),
                        Some(bm.entry.description.clone()),
                        None,
                        Some(icon),
                    )
                }
                SearchResult::App(am) => {
                    // Apps use pre-decoded icons, fallback to File SVG
                    let icon = match &am.app.icon {
                        Some(img) => IconKind::Image(img.clone()),
                        None => IconKind::Svg("File".to_string()),
                    };
                    (am.app.name.clone(), None, None, Some(icon))
                }
                SearchResult::Window(wm) => {
                    // Windows get a generic File icon, title as name, app as description
                    (
                        wm.window.title.clone(),
                        Some(wm.window.app.clone()),
                        None,
                        Some(IconKind::Svg("File".to_string())),
                    )
                }
            };

            ListItem::new(name, list_colors)
                .index(index)
                .icon_kind_opt(icon_kind)
                .description_opt(description)
                .shortcut_opt(shortcut)
                .selected(is_selected)
                .hovered(is_hovered)
                .with_accent_bar(true)
                .into_any_element()
        }
    }
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
            assert!(
                variant.is_some(),
                "Keyboard number {} should map to a variant",
                num
            );

            let v = variant.unwrap();
            let shortcut = v.shortcut_number();

            // All variants except Playful should have shortcuts
            if v != DesignVariant::Playful {
                assert!(shortcut.is_some(), "Variant {:?} should have a shortcut", v);
                assert_eq!(
                    shortcut.unwrap(),
                    num,
                    "Round-trip failed for number {}",
                    num
                );
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
            assert!(
                !variant.name().is_empty(),
                "Variant {:?} should have a name",
                variant
            );
            assert!(
                !variant.description().is_empty(),
                "Variant {:?} should have a description",
                variant
            );
        }
    }

    #[test]
    fn test_default_variant() {
        assert_eq!(DesignVariant::default(), DesignVariant::Default);
    }

    #[test]
    fn test_uses_default_renderer() {
        // Minimal and RetroTerminal now have custom renderers
        assert!(
            !uses_default_renderer(DesignVariant::Minimal),
            "Minimal should NOT use default renderer"
        );
        assert!(
            !uses_default_renderer(DesignVariant::RetroTerminal),
            "RetroTerminal should NOT use default renderer"
        );

        // Default still uses default renderer
        assert!(
            uses_default_renderer(DesignVariant::Default),
            "Default should use default renderer"
        );

        // Other variants still use default renderer (until implemented)
        assert!(uses_default_renderer(DesignVariant::Brutalist));
        assert!(uses_default_renderer(DesignVariant::NeonCyberpunk));
    }

    #[test]
    fn test_get_item_height() {
        // Minimal uses taller items (64px)
        assert_eq!(get_item_height(DesignVariant::Minimal), MINIMAL_ITEM_HEIGHT);
        assert_eq!(get_item_height(DesignVariant::Minimal), 64.0);

        // RetroTerminal uses denser items (28px)
        assert_eq!(
            get_item_height(DesignVariant::RetroTerminal),
            TERMINAL_ITEM_HEIGHT
        );
        assert_eq!(get_item_height(DesignVariant::RetroTerminal), 28.0);

        // Compact uses the smallest items (24px)
        assert_eq!(get_item_height(DesignVariant::Compact), COMPACT_ITEM_HEIGHT);
        assert_eq!(get_item_height(DesignVariant::Compact), 24.0);

        // Default and others use standard height (40px - from design tokens)
        // Note: This differs from LIST_ITEM_HEIGHT (48.0) which is used for actual rendering
        assert_eq!(get_item_height(DesignVariant::Default), 40.0);
        assert_eq!(get_item_height(DesignVariant::Brutalist), 40.0);
    }

    #[test]
    fn test_design_variant_dispatch_coverage() {
        // Ensure all variants are covered by the dispatch logic
        // This test verifies the match arms in render_design_item cover all cases
        for variant in DesignVariant::all() {
            let uses_default = uses_default_renderer(*variant);
            let height = get_item_height(*variant);

            // All variants should have a defined height
            assert!(
                height > 0.0,
                "Variant {:?} should have positive item height",
                variant
            );

            // Minimal and RetroTerminal should use custom renderers
            if *variant == DesignVariant::Minimal || *variant == DesignVariant::RetroTerminal {
                assert!(
                    !uses_default,
                    "Variant {:?} should use custom renderer",
                    variant
                );
            }
        }
    }

    #[test]
    fn test_design_keyboard_coverage() {
        // Verify all keyboard shortcuts 1-0 are mapped
        let mut mapped_variants = Vec::new();
        for num in 0..=9 {
            if let Some(variant) = DesignVariant::from_keyboard_number(num) {
                mapped_variants.push(variant);
            }
        }
        // Should have 10 mapped variants (Cmd+1 through Cmd+0)
        assert_eq!(
            mapped_variants.len(),
            10,
            "Expected 10 keyboard-mapped variants"
        );

        // All mapped variants should be unique
        let mut unique = mapped_variants.clone();
        unique.sort_by_key(|v| *v as u8);
        unique.dedup_by_key(|v| *v as u8);
        assert_eq!(unique.len(), 10, "All keyboard mappings should be unique");
    }

    #[test]
    fn test_design_cycling() {
        // Test that next() cycles through all designs
        let all = DesignVariant::all();
        let mut current = DesignVariant::Default;

        // Cycle through all designs
        for (i, expected) in all.iter().enumerate() {
            assert_eq!(
                current, *expected,
                "Cycle iteration {} should be {:?}",
                i, expected
            );
            current = current.next();
        }

        // After cycling through all, we should be back at Default
        assert_eq!(
            current,
            DesignVariant::Default,
            "Should cycle back to Default"
        );
    }

    #[test]
    fn test_design_prev() {
        // Test that prev() goes backwards
        let current = DesignVariant::Default;
        let prev = current.prev();

        // Default.prev() should be Playful (last in list)
        assert_eq!(prev, DesignVariant::Playful);

        // And prev of that should be Compact
        assert_eq!(prev.prev(), DesignVariant::Compact);
    }

    // =========================================================================
    // DesignTokens Tests
    // =========================================================================

    #[test]
    fn test_get_tokens_returns_correct_variant() {
        // Verify get_tokens returns tokens with matching variant
        for variant in DesignVariant::all() {
            let tokens = get_tokens(*variant);
            assert_eq!(
                tokens.variant(),
                *variant,
                "get_tokens({:?}) returned tokens for {:?}",
                variant,
                tokens.variant()
            );
        }
    }

    #[test]
    fn test_get_tokens_item_height_matches() {
        // Verify token item_height matches get_item_height function
        for variant in DesignVariant::all() {
            let tokens = get_tokens(*variant);
            let fn_height = get_item_height(*variant);
            let token_height = tokens.item_height();

            assert_eq!(
                fn_height, token_height,
                "Item height mismatch for {:?}: get_item_height={}, tokens.item_height={}",
                variant, fn_height, token_height
            );
        }
    }

    #[test]
    fn test_design_colors_defaults() {
        let colors = DesignColors::default();

        // Verify expected defaults
        assert_eq!(colors.background, 0x1e1e1e);
        assert_eq!(colors.text_primary, 0xffffff);
        assert_eq!(colors.accent, 0xfbbf24);
        assert_eq!(colors.border, 0x464647);
    }

    #[test]
    fn test_design_spacing_defaults() {
        let spacing = DesignSpacing::default();

        // Verify expected defaults
        assert_eq!(spacing.padding_xs, 4.0);
        assert_eq!(spacing.padding_md, 12.0);
        assert_eq!(spacing.gap_md, 8.0);
        assert_eq!(spacing.item_padding_x, 16.0);
    }

    #[test]
    fn test_design_typography_defaults() {
        let typography = DesignTypography::default();

        // Verify expected defaults
        assert_eq!(typography.font_family, ".AppleSystemUIFont");
        assert_eq!(typography.font_family_mono, "Menlo");
        assert_eq!(typography.font_size_md, 14.0);
    }

    #[test]
    fn test_design_visual_defaults() {
        let visual = DesignVisual::default();

        // Verify expected defaults
        assert_eq!(visual.radius_sm, 4.0);
        assert_eq!(visual.radius_md, 8.0);
        assert_eq!(visual.shadow_opacity, 0.25);
        assert_eq!(visual.border_thin, 1.0);
    }

    #[test]
    fn test_design_tokens_are_copy() {
        // Verify all token structs are Copy (needed for closure efficiency)
        fn assert_copy<T: Copy>() {}

        assert_copy::<DesignColors>();
        assert_copy::<DesignSpacing>();
        assert_copy::<DesignTypography>();
        assert_copy::<DesignVisual>();
    }

    #[test]
    fn test_minimal_tokens_distinctive() {
        let tokens = MinimalDesignTokens;

        // Minimal should have taller items and more generous padding
        assert_eq!(tokens.item_height(), 64.0);
        assert_eq!(tokens.spacing().item_padding_x, 80.0);
        assert_eq!(tokens.visual().radius_md, 0.0); // No borders
    }

    #[test]
    fn test_retro_terminal_tokens_distinctive() {
        let tokens = RetroTerminalDesignTokens;

        // Terminal should have dense items and phosphor green colors
        assert_eq!(tokens.item_height(), 28.0);
        assert_eq!(tokens.colors().text_primary, 0x00ff00); // Phosphor green
        assert_eq!(tokens.colors().background, 0x000000); // Pure black
        assert_eq!(tokens.typography().font_family, "Menlo");
    }

    #[test]
    fn test_compact_tokens_distinctive() {
        let tokens = CompactDesignTokens;

        // Compact should have smallest items
        assert_eq!(tokens.item_height(), 24.0);
        assert!(tokens.spacing().padding_md < DesignSpacing::default().padding_md);
    }

    #[test]
    fn test_all_variants_have_positive_item_height() {
        for variant in DesignVariant::all() {
            let tokens = get_tokens(*variant);
            assert!(
                tokens.item_height() > 0.0,
                "Variant {:?} has non-positive item height",
                variant
            );
        }
    }

    #[test]
    fn test_all_variants_have_valid_colors() {
        for variant in DesignVariant::all() {
            let tokens = get_tokens(*variant);
            let colors = tokens.colors();

            // Background should be different from text (for contrast)
            assert_ne!(
                colors.background, colors.text_primary,
                "Variant {:?} has no contrast between bg and text",
                variant
            );
        }
    }
}

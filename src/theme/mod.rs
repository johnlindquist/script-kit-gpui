//! Theme module - Color schemes and styling
//!
//! This module provides functionality for:
//! - Loading theme from ~/.scriptkit/theme.json
//! - Color scheme definitions (dark/light mode)
//! - Terminal ANSI color palette
//! - gpui-component theme integration
//! - Global theme service for multi-window theme sync
//!
//! # Module Structure
//!
//! - `hex_color` - Hex color parsing and serialization
//! - `types` - Theme struct definitions
//! - `helpers` - Lightweight color extraction for render closures
//! - `gpui_integration` - gpui-component theme mapping
//! - `service` - Global theme watcher service
//!
//! # Style Resolution Order
//!
//! Colors and chrome values flow through one pipeline from disk to pixels:
//!
//! 1. **theme.json on disk** - `~/.scriptkit/theme.json` (with user-preference
//!    and appearance fallbacks) is parsed by `types::load_theme()`. This is the
//!    only step that touches the filesystem.
//! 2. **THEME_CACHE** - the loaded `Theme` is stored in the global cache in
//!    `types.rs`. `service.rs` owns `THEME_REVISION`, bumped whenever the cache
//!    reloads: by the theme.json file watcher, by appearance flips, or by
//!    `service::reapply_runtime_theme_overrides` after a dev-tool color edit.
//!    `set_cached_theme_for_preview` (used by the theme chooser built-in) is a
//!    side door that swaps the cached theme for live preview without touching
//!    disk or the override channel.
//! 4. **get_cached_theme() consumers** - render and service code read the
//!    cached theme; revision checks let windows notice cross-window changes.
//! 5. **Per-surface token defs** - surface-level token structs resolve from the
//!    cached theme and then apply their own dev-tool override channels:
//!    `MainMenuThemeVariant::def()` runs `apply_to_main_menu_def`, the actions
//!    popup def runs `apply_to_actions_popup_def`, and Agent Chat / confirm
//!    modal read `effective_agent_chat_style()` / `effective_confirm_modal_style()`.
//! 6. **Render-time color structs** - per-frame `Copy` snapshots are extracted
//!    for render closures: `AppChromeColors::from_theme`, `ListItemColors`,
//!    and `PromptColors`.
//!
//! Note: `DesignVariant` (`designs/core/registry.rs` + `designs/traits/**`) is
//! a separate catalog path for non-default designs; the pipeline above covers
//! the default theme-driven path.

mod audit;
mod chrome;
mod color_resolver;
pub(crate) mod gpui_integration;
mod helpers;
pub mod hex_color;
pub mod opacity;
pub mod prelude;
pub mod presets;
pub(crate) mod scrollbar;
pub mod service;
pub(crate) mod types;
pub mod user_themes;
pub(crate) mod validation;

// Re-export shared chrome contract for app surfaces
pub(crate) use chrome::{AppChromeColors, DangerActionColors, SemanticChipColors};

// Re-export contrast audit helpers
#[allow(unused_imports)]
pub use audit::{
    audit_theme_contrast, theme_contrast_score, worst_theme_contrast, ThemeContrastSample,
};

// Re-export types used externally
pub(crate) use types::relative_luminance_srgb;
#[allow(unused_imports)]
pub use types::{
    BackgroundGradient, ColorScheme, FontConfig, Theme, VibrancyMaterial, DARK_ROW_HOVER_OPACITY,
    DARK_ROW_SELECTED_OPACITY, LIGHT_ROW_HOVER_OPACITY, LIGHT_ROW_SELECTED_OPACITY,
};

// Re-export helper types for lightweight color extraction
pub use helpers::{
    accent_color_name, best_readable_text_hex, contrast_ratio, hard_readable_text_hex,
    hover_overlay_bg, modal_overlay_bg, ListItemColors, PromptColors, ACCENT_PALETTE,
};

// Re-export color resolver for unified color access
#[allow(unused_imports)]
pub use color_resolver::{
    ColorResolver, SpacingResolver, SurfaceColorStrategy, TypographyResolver,
};

// Re-export loader functions
#[allow(unused_imports)] // Re-exported for the library target and startup paths.
pub use types::load_theme;

// Re-export cached theme access (use in render code instead of load_theme)
pub use types::{get_cached_theme, init_theme_cache, reload_theme_cache};

// Re-export appearance cache invalidation (called when system appearance changes)
pub use types::invalidate_appearance_cache;

// Re-export gpui integration
pub use gpui_integration::sync_gpui_component_theme;

// Keep cross-target theme exports reachable in both lib and binary builds.
const _: fn(u32) -> f32 = relative_luminance_srgb;
const _: fn() -> &'static [presets::ThemePreset] = presets::presets_cached;
const _: fn() -> &'static [presets::PresetPreviewColors] = presets::preset_preview_colors_cached;
const _: fn(usize) -> std::sync::Arc<Theme> = presets::preset_theme_cached;
const _: fn(&str) -> Vec<usize> = presets::filtered_preset_indices_cached;
const _: usize = core::mem::size_of::<ListItemColors>();
const _: fn(u32, u32) -> f32 = contrast_ratio;
const _: fn(u32) -> u32 = best_readable_text_hex;
const _: usize = core::mem::size_of::<SemanticChipColors>();
const _: usize = core::mem::size_of::<ThemeContrastSample>();

#[cfg(test)]
#[path = "lightweight_colors_test.rs"]
mod legacy_lightweight_colors_test;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

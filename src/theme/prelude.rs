//! Curated theme API surface for modules that want explicit, stable imports.

pub use super::color_resolver::{ColorResolver, SpacingResolver, TypographyResolver};
pub use super::gpui_integration::sync_gpui_component_theme;
pub use super::helpers::{hover_overlay_bg, modal_overlay_bg, ListItemColors, PromptColors};
pub use super::types::{
    get_cached_theme, get_theme_snapshot, invalidate_appearance_cache, load_theme, ColorScheme,
    FontConfig, Theme, VibrancyMaterial,
};
// Keep curated re-exports lint-clean when the prelude isn't imported internally.
const _: () = {
    let _ = ColorResolver::new;
    let _ = SpacingResolver::new;
    let _ = TypographyResolver::new;
    let _ = sync_gpui_component_theme;
    let _ = hover_overlay_bg;
    let _ = modal_overlay_bg;
    let _ = core::mem::size_of::<ListItemColors>();
    let _ = core::mem::size_of::<PromptColors>();
    let _ = get_cached_theme;
    let _ = get_theme_snapshot;
    let _ = invalidate_appearance_cache;
    let _ = load_theme;
    let _ = core::mem::size_of::<ColorScheme>();
    let _ = core::mem::size_of::<FontConfig>();
    let _ = core::mem::size_of::<Theme>();
    let _ = core::mem::size_of::<VibrancyMaterial>();
};

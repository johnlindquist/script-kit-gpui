//! Asset path resolution for Script Kit GPUI

use std::borrow::Cow;

/// Script Kit's own SVG icons (`assets/icons/*.svg`), embedded at compile
/// time so `EmbeddedIcon` paths like "icons/copy.svg" resolve in dev builds,
/// driver sandboxes, and the released .app alike.
#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
#[include = "logo.svg"]
struct ScriptKitEmbeddedAssets;

/// The application AssetSource: Script Kit's embedded icons first (exact
/// snake_case art wins on a name collision like "icons/check.svg"), then
/// gpui-component's embedded Lucide set (kebab-case, e.g.
/// "icons/monitor.svg" used by `IconName::path()`).
///
/// REGRESSION GUARD: `gpui_platform::application()` defaults to the unit
/// asset source, which returns `Ok(None)` for every path — with it, every
/// icon rendered through `svg().path(...)` (all Lucide + EmbeddedIcon call
/// sites) silently paints NOTHING. This source must stay registered via
/// `.with_assets(AppAssets)` on the Application in app_run_setup.rs.
pub struct AppAssets;

impl gpui::AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        if let Some(file) = ScriptKitEmbeddedAssets::get(path) {
            return Ok(Some(file.data));
        }
        gpui::AssetSource::load(&gpui_component_assets::Assets, path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<gpui::SharedString>> {
        let mut entries: Vec<gpui::SharedString> = ScriptKitEmbeddedAssets::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| p.to_string().into())
            .collect();
        entries.extend(gpui::AssetSource::list(
            &gpui_component_assets::Assets,
            path,
        )?);
        entries.sort();
        entries.dedup();
        Ok(entries)
    }
}

/// Whether `path` resolves through the embedded [`AppAssets`] source.
/// Used to validate icon tokens without touching the filesystem.
pub fn embedded_asset_exists(path: &str) -> bool {
    matches!(gpui::AssetSource::load(&AppAssets, path), Ok(Some(_)))
}

/// Load an embedded application asset as owned bytes for native AppKit
/// bridges that cannot consume GPUI's `AssetSource` directly.
pub fn embedded_asset_bytes(path: &str) -> Option<Vec<u8>> {
    gpui::AssetSource::load(&AppAssets, path)
        .ok()
        .flatten()
        .map(|bytes| bytes.into_owned())
}

/// Attach an icon source to an `svg()` builder, picking the right loader:
/// relative paths ("icons/foo.svg", "logo.svg") load from the embedded
/// [`AppAssets`] source, absolute paths from disk (script-provided icons).
pub fn svg_icon_source(svg: gpui::Svg, path: &str) -> gpui::Svg {
    if std::path::Path::new(path).is_absolute() {
        svg.external_path(path.to_string())
    } else {
        svg.path(path.to_string())
    }
}

#[cfg(test)]
mod asset_source_tests {
    use super::AppAssets;
    use gpui::AssetSource;

    /// The two icon families the launcher renders through svg().path():
    /// EmbeddedIcon snake_case paths and Lucide kebab-case paths. If either
    /// stops resolving, list rows silently lose their icons (P0 2026-06-11).
    #[test]
    fn app_assets_resolve_embedded_and_lucide_icons() {
        for path in [
            "icons/copy.svg",            // Script Kit EmbeddedIcon
            "icons/file_code.svg",       // Script Kit EmbeddedIcon
            "icons/monitor.svg",         // Lucide (builtin "Show Desktop")
            "icons/sun-moon.svg",        // Lucide (builtin "Toggle Dark Mode")
            "icons/square-terminal.svg", // Lucide
            "icons/lightbulb.svg",       // Native floating-footer tip glyph
        ] {
            let loaded = AppAssets.load(path).expect("asset load must not error");
            assert!(
                loaded.map(|bytes| !bytes.is_empty()).unwrap_or(false),
                "asset {path:?} must resolve to non-empty bytes"
            );
        }
        assert!(
            AppAssets.load("icons/definitely-not-an-icon.svg").is_err()
                || AppAssets
                    .load("icons/definitely-not-an-icon.svg")
                    .ok()
                    .flatten()
                    .is_none(),
            "unknown assets must not resolve"
        );
    }
}

// NOTE: the old get_asset_path()/get_logo_path() helpers were removed
// (P0 2026-06-11): they resolved icons through compile-time
// CARGO_MANIFEST_DIR filesystem paths, which point at the CI runner in
// released bundles, silently blanking every icon. All app icons now load
// through the embedded [`AppAssets`] source via relative paths.

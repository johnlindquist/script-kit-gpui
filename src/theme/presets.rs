//! Theme presets - Curated color schemes for the theme chooser
//!
//! Each preset returns a complete Theme struct with all colors configured.
//! Presets are designed to work well with Script Kit's vibrancy/blur effects.

// --- merged from part_01.rs ---
use super::types::{
    AccentColors, AppearanceMode, BackgroundColors, BackgroundOpacity, ColorScheme, DropShadow,
    FontConfig, TerminalColors, TextColors, Theme, UIColors, VibrancySettings,
};
use std::{collections::HashMap, sync::LazyLock};

/// A theme preset with metadata for the chooser UI
#[derive(Debug, Clone)]
pub struct ThemePreset {
    /// Unique identifier for the preset (used in tests and for persistence)
    pub id: &'static str,
    /// Display name shown in the chooser
    pub name: &'static str,
    /// Short description of the theme style
    pub description: &'static str,
    /// Whether this is a dark or light theme
    pub is_dark: bool,
    /// Function to create the theme
    theme: fn() -> Theme,
}

impl ThemePreset {
    /// Create the theme for this preset
    pub fn create_theme(&self) -> Theme {
        (self.theme)()
    }
}

/// Preview colors for rendering palette swatches in the theme chooser
#[derive(Debug, Clone, Copy)]
pub struct PresetPreviewColors {
    pub bg: u32,
    pub accent: u32,
    pub text: u32,
    pub secondary: u32,
    pub border: u32,
}

/// Get all available theme presets (dark themes first, then light)
pub fn all_presets() -> Vec<ThemePreset> {
    vec![
        // ── Dark Themes ──────────────────────────────────────────
        ThemePreset {
            id: "script-kit-dark",
            name: "Script Kit Dark",
            description: "Default dark theme with yellow accent",
            is_dark: true,
            theme: theme_script_kit_dark,
        },
        ThemePreset {
            id: "dracula",
            name: "Dracula",
            description: "Dark theme with vibrant purple and pink",
            is_dark: true,
            theme: theme_dracula,
        },
        ThemePreset {
            id: "nord",
            name: "Nord",
            description: "Arctic, north-bluish color palette",
            is_dark: true,
            theme: theme_nord,
        },
        ThemePreset {
            id: "catppuccin-mocha",
            name: "Catppuccin Mocha",
            description: "Soothing pastel dark theme",
            is_dark: true,
            theme: theme_catppuccin_mocha,
        },
        ThemePreset {
            id: "one-dark",
            name: "One Dark",
            description: "Atom-inspired balanced dark theme",
            is_dark: true,
            theme: theme_one_dark,
        },
        ThemePreset {
            id: "tokyo-night",
            name: "Tokyo Night",
            description: "Clean dark theme with muted tones",
            is_dark: true,
            theme: theme_tokyo_night,
        },
        ThemePreset {
            id: "gruvbox-dark",
            name: "Gruvbox Dark",
            description: "Retro groove with warm earthy tones",
            is_dark: true,
            theme: theme_gruvbox_dark,
        },
        ThemePreset {
            id: "rose-pine",
            name: "Rosé Pine",
            description: "Elegant dark theme with muted rose",
            is_dark: true,
            theme: theme_rose_pine,
        },
        ThemePreset {
            id: "solarized-dark",
            name: "Solarized Dark",
            description: "Precision colors for machines and people",
            is_dark: true,
            theme: theme_solarized_dark,
        },
        ThemePreset {
            id: "github-dark",
            name: "GitHub Dark",
            description: "GitHub's dark default color scheme",
            is_dark: true,
            theme: theme_github_dark,
        },
        ThemePreset {
            id: "github-dimmed",
            name: "GitHub Dimmed",
            description: "GitHub's softer dimmed dark palette",
            is_dark: true,
            theme: theme_github_dimmed,
        },
        ThemePreset {
            id: "monokai-pro",
            name: "Monokai Pro",
            description: "Classic vibrant syntax theme",
            is_dark: true,
            theme: theme_monokai_pro,
        },
        ThemePreset {
            id: "everforest-dark",
            name: "Everforest Dark",
            description: "Nature-inspired warm green palette",
            is_dark: true,
            theme: theme_everforest_dark,
        },
        ThemePreset {
            id: "kanagawa",
            name: "Kanagawa",
            description: "Muted wave-inspired Japanese palette",
            is_dark: true,
            theme: theme_kanagawa,
        },
        ThemePreset {
            id: "ayu-dark",
            name: "Ayu Dark",
            description: "Minimal and modern dark theme",
            is_dark: true,
            theme: theme_ayu_dark,
        },
        ThemePreset {
            id: "material-ocean",
            name: "Material Ocean",
            description: "Material Design oceanic dark theme",
            is_dark: true,
            theme: theme_material_ocean,
        },
        ThemePreset {
            id: "monokai",
            name: "Monokai",
            description: "Classic high-contrast editor theme",
            is_dark: true,
            theme: theme_monokai,
        },
        ThemePreset {
            id: "one-dark-pro",
            name: "One Dark Pro",
            description: "Popular Atom-style dark theme",
            is_dark: true,
            theme: theme_one_dark_pro,
        },
        ThemePreset {
            id: "tokyo-night-storm",
            name: "Tokyo Night Storm",
            description: "Storm variant with deeper indigo tones",
            is_dark: true,
            theme: theme_tokyo_night_storm,
        },
        ThemePreset {
            id: "rose-pine-moon",
            name: "Rosé Pine Moon",
            description: "Rosé Pine variant with dusky violets",
            is_dark: true,
            theme: theme_rose_pine_moon,
        },
        ThemePreset {
            id: "poimandres",
            name: "Poimandres",
            description: "Electric dark theme with icy neon accents",
            is_dark: true,
            theme: theme_poimandres,
        },
        ThemePreset {
            id: "palenight",
            name: "Palenight",
            description: "Material-inspired night theme with soft purple",
            is_dark: true,
            theme: theme_palenight,
        },
        ThemePreset {
            id: "horizon-dark",
            name: "Horizon Dark",
            description: "Moody dusk palette with coral and cyan",
            is_dark: true,
            theme: theme_horizon_dark,
        },
        ThemePreset {
            id: "andromeda",
            name: "Andromeda",
            description: "Vibrant cosmic palette with neon cyan",
            is_dark: true,
            theme: theme_andromeda,
        },
        ThemePreset {
            id: "synthwave-84",
            name: "SynthWave '84",
            description: "Retro neon theme with electric glow",
            is_dark: true,
            theme: theme_synthwave_84,
        },
        ThemePreset {
            id: "shades-of-purple",
            name: "Shades of Purple",
            description: "Deep violet theme with luminous gold",
            is_dark: true,
            theme: theme_shades_of_purple,
        },
        ThemePreset {
            id: "cobalt2",
            name: "Cobalt2",
            description: "Bold blue theme with vivid amber accents",
            is_dark: true,
            theme: theme_cobalt2,
        },
        ThemePreset {
            id: "ayu-mirage",
            name: "Ayu Mirage",
            description: "Balanced dark theme with warm gold accents",
            is_dark: true,
            theme: theme_ayu_mirage,
        },
        ThemePreset {
            id: "night-owl",
            name: "Night Owl",
            description: "Beloved dark theme tuned for long coding sessions",
            is_dark: true,
            theme: theme_night_owl,
        },
        ThemePreset {
            id: "vitesse-dark",
            name: "Vitesse Dark",
            description: "Elegant dark theme with muted earth tones",
            is_dark: true,
            theme: theme_vitesse_dark,
        },
        ThemePreset {
            id: "catppuccin-frappe",
            name: "Catppuccin Frappé",
            description: "Soothing pastel mid-dark theme",
            is_dark: true,
            theme: theme_catppuccin_frappe,
        },
        ThemePreset {
            id: "catppuccin-macchiato",
            name: "Catppuccin Macchiato",
            description: "Soothing pastel warm-dark theme",
            is_dark: true,
            theme: theme_catppuccin_macchiato,
        },
        ThemePreset {
            id: "darcula",
            name: "Darcula",
            description: "JetBrains classic dark IDE theme",
            is_dark: true,
            theme: theme_darcula,
        },
        ThemePreset {
            id: "moonlight",
            name: "Moonlight II",
            description: "Soft neon dark theme with deep blues",
            is_dark: true,
            theme: theme_moonlight,
        },
        ThemePreset {
            id: "nightfly",
            name: "Nightfly",
            description: "Dark blue theme with vivid syntax colors",
            is_dark: true,
            theme: theme_nightfly,
        },
        ThemePreset {
            id: "oxocarbon-dark",
            name: "Oxocarbon Dark",
            description: "IBM Carbon-inspired dark theme",
            is_dark: true,
            theme: theme_oxocarbon_dark,
        },
        ThemePreset {
            id: "flexoki-dark",
            name: "Flexoki Dark",
            description: "Inky dark theme with warm tones",
            is_dark: true,
            theme: theme_flexoki_dark,
        },
        ThemePreset {
            id: "kanagawa-dragon",
            name: "Kanagawa Dragon",
            description: "Darker Kanagawa variant with muted palette",
            is_dark: true,
            theme: theme_kanagawa_dragon,
        },
        ThemePreset {
            id: "iceberg-dark",
            name: "Iceberg Dark",
            description: "Well-designed dark blue theme",
            is_dark: true,
            theme: theme_iceberg_dark,
        },
        ThemePreset {
            id: "bluloco-dark",
            name: "Bluloco Dark",
            description: "Fancy italic dark theme with vivid colors",
            is_dark: true,
            theme: theme_bluloco_dark,
        },
        ThemePreset {
            id: "aura-dark",
            name: "Aura Dark",
            description: "Dark purple theme with soft neon accents",
            is_dark: true,
            theme: theme_aura_dark,
        },
        ThemePreset {
            id: "panda-syntax",
            name: "Panda Syntax",
            description: "Superminimal dark theme with vivid accents",
            is_dark: true,
            theme: theme_panda_syntax,
        },
        ThemePreset {
            id: "laserwave",
            name: "Laserwave",
            description: "Retro synthwave theme with warm neons",
            is_dark: true,
            theme: theme_laserwave,
        },
        ThemePreset {
            id: "fairy-floss",
            name: "Fairy Floss",
            description: "Candy-colored pastel purple theme",
            is_dark: true,
            theme: theme_fairy_floss,
        },
        ThemePreset {
            id: "zenburn",
            name: "Zenburn",
            description: "Low-contrast warm dark theme for long sessions",
            is_dark: true,
            theme: theme_zenburn,
        },
        ThemePreset {
            id: "srcery",
            name: "Srcery",
            description: "Dark theme with vivid saturated colors",
            is_dark: true,
            theme: theme_srcery,
        },
        ThemePreset {
            id: "papercolor-dark",
            name: "PaperColor Dark",
            description: "Print-inspired dark theme",
            is_dark: true,
            theme: theme_papercolor_dark,
        },
        ThemePreset {
            id: "vesper",
            name: "Vesper",
            description: "Warm amber-tinted dark theme for night coding",
            is_dark: true,
            theme: theme_vesper,
        },
        ThemePreset {
            id: "midnight-blue",
            name: "Midnight Blue",
            description: "Deep navy dark theme with steel-blue accents",
            is_dark: true,
            theme: theme_midnight_blue,
        },
        ThemePreset {
            id: "ember",
            name: "Ember",
            description: "Warm charcoal theme with amber glow",
            is_dark: true,
            theme: theme_ember,
        },
        ThemePreset {
            id: "arctic",
            name: "Arctic",
            description: "Cool dark theme with icy cyan accents",
            is_dark: true,
            theme: theme_arctic,
        },
        // ── Light Themes ─────────────────────────────────────────
        ThemePreset {
            id: "script-kit-light",
            name: "Script Kit Light",
            description: "Default light theme with blue accent",
            is_dark: false,
            theme: theme_script_kit_light,
        },
        ThemePreset {
            id: "catppuccin-latte",
            name: "Catppuccin Latte",
            description: "Soothing pastel light theme",
            is_dark: false,
            theme: theme_catppuccin_latte,
        },
        ThemePreset {
            id: "solarized-light",
            name: "Solarized Light",
            description: "Warm light theme with balanced contrast",
            is_dark: false,
            theme: theme_solarized_light,
        },
        ThemePreset {
            id: "github-light",
            name: "GitHub Light",
            description: "GitHub's clean light color scheme",
            is_dark: false,
            theme: theme_github_light,
        },
        ThemePreset {
            id: "rose-pine-dawn",
            name: "Rosé Pine Dawn",
            description: "Rosé Pine in a soft morning palette",
            is_dark: false,
            theme: theme_rose_pine_dawn,
        },
        ThemePreset {
            id: "everforest-light",
            name: "Everforest Light",
            description: "Nature-inspired light theme with muted greens",
            is_dark: false,
            theme: theme_everforest_light,
        },
        ThemePreset {
            id: "vitesse-light",
            name: "Vitesse Light",
            description: "Elegant light theme with calm contrast",
            is_dark: false,
            theme: theme_vitesse_light,
        },
        ThemePreset {
            id: "ayu-light",
            name: "Ayu Light",
            description: "Clean light variant of the Ayu palette",
            is_dark: false,
            theme: theme_ayu_light,
        },
        ThemePreset {
            id: "night-owl-light",
            name: "Night Owl Light",
            description: "Light companion to the Night Owl palette",
            is_dark: false,
            theme: theme_night_owl_light,
        },
        ThemePreset {
            id: "tokyo-day",
            name: "Tokyo Day",
            description: "Airy light theme with slate-blue accents",
            is_dark: false,
            theme: theme_tokyo_day,
        },
        ThemePreset {
            id: "gruvbox-light",
            name: "Gruvbox Light",
            description: "Retro groove with warm earthy light tones",
            is_dark: false,
            theme: theme_gruvbox_light,
        },
        ThemePreset {
            id: "intellij-light",
            name: "IntelliJ Light",
            description: "JetBrains classic light IDE theme",
            is_dark: false,
            theme: theme_intellij_light,
        },
        ThemePreset {
            id: "flexoki-light",
            name: "Flexoki Light",
            description: "Paper-like light theme with warm tones",
            is_dark: false,
            theme: theme_flexoki_light,
        },
        ThemePreset {
            id: "kanagawa-lotus",
            name: "Kanagawa Lotus",
            description: "Light Kanagawa variant with warm paper tones",
            is_dark: false,
            theme: theme_kanagawa_lotus,
        },
        ThemePreset {
            id: "iceberg-light",
            name: "Iceberg Light",
            description: "Well-designed light blue theme",
            is_dark: false,
            theme: theme_iceberg_light,
        },
        ThemePreset {
            id: "bluloco-light",
            name: "Bluloco Light",
            description: "Fancy italic light theme with vivid colors",
            is_dark: false,
            theme: theme_bluloco_light,
        },
        ThemePreset {
            id: "atom-one-light",
            name: "Atom One Light",
            description: "Atom editor's classic light theme",
            is_dark: false,
            theme: theme_atom_one_light,
        },
        ThemePreset {
            id: "papercolor-light",
            name: "PaperColor Light",
            description: "Print-inspired light theme",
            is_dark: false,
            theme: theme_papercolor_light,
        },
        ThemePreset {
            id: "alabaster",
            name: "Alabaster",
            description: "Minimal white theme with subtle warm tones",
            is_dark: false,
            theme: theme_alabaster,
        },
        ThemePreset {
            id: "linen",
            name: "Linen",
            description: "Warm cream light theme with soft brown accents",
            is_dark: false,
            theme: theme_linen,
        },
        ThemePreset {
            id: "slate-morning",
            name: "Slate Morning",
            description: "Cool gray light theme with teal accents",
            is_dark: false,
            theme: theme_slate_morning,
        },
        ThemePreset {
            id: "coral-reef",
            name: "Coral Reef",
            description: "Warm light theme with coral and sea-green accents",
            is_dark: false,
            theme: theme_coral_reef,
        },
    ]
}

fn normalize_preset_search_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_space = true;
    for ch in input.chars().flat_map(char::to_lowercase) {
        let normalized = match ch {
            '-' | '_' | '/' | '\\' | '\'' | '\u{2019}' | '.' | ',' | ':' | ';' | '(' | ')'
            | '[' | ']' | '\u{00E9}' => ' ',
            ch if ch.is_whitespace() => ' ',
            ch => ch,
        };
        if normalized == ' ' {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(normalized);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

fn build_preset_search_blob(preset: &ThemePreset) -> String {
    let tone = if preset.is_dark { "dark" } else { "light" };
    normalize_preset_search_text(&format!(
        "{} {} {} {}",
        preset.id, preset.name, preset.description, tone
    ))
}

struct PresetsCache {
    presets: Vec<ThemePreset>,
    preset_themes: Vec<std::sync::Arc<Theme>>,
    preset_preview_colors: Vec<PresetPreviewColors>,
    preset_search_blobs: Vec<String>,
    first_light_theme_index: usize,
    preset_index_by_bg_accent: HashMap<u64, usize>,
}

impl PresetsCache {
    fn new() -> Self {
        let presets = all_presets();
        let first_light_theme_index = presets.iter().position(|p| !p.is_dark).unwrap_or(0);
        let mut preset_themes = Vec::with_capacity(presets.len());
        let mut preset_preview_colors = Vec::with_capacity(presets.len());
        let mut preset_search_blobs = Vec::with_capacity(presets.len());
        let mut preset_index_by_bg_accent = HashMap::with_capacity(presets.len());

        for (index, preset) in presets.iter().enumerate() {
            let theme = std::sync::Arc::new(preset.create_theme());
            let bg_main = theme.colors.background.main;
            let accent_selected = theme.colors.accent.selected;

            preset_preview_colors.push(PresetPreviewColors {
                bg: bg_main,
                accent: accent_selected,
                text: theme.colors.text.primary,
                secondary: theme.colors.text.secondary,
                border: theme.colors.ui.border,
            });
            preset_search_blobs.push(build_preset_search_blob(preset));
            preset_index_by_bg_accent.insert(preset_bg_accent_key(bg_main, accent_selected), index);
            preset_themes.push(theme);
        }

        Self {
            presets,
            preset_themes,
            preset_preview_colors,
            preset_search_blobs,
            first_light_theme_index,
            preset_index_by_bg_accent,
        }
    }
}

static PRESETS_CACHE: LazyLock<PresetsCache> = LazyLock::new(PresetsCache::new);

fn preset_bg_accent_key(bg_main: u32, accent_selected: u32) -> u64 {
    ((bg_main as u64) << 32) | (accent_selected as u64)
}

fn presets_cache() -> &'static PresetsCache {
    &PRESETS_CACHE
}

pub(crate) fn presets_cached() -> &'static [ThemePreset] {
    &presets_cache().presets
}

pub(crate) fn preset_preview_colors_cached() -> &'static [PresetPreviewColors] {
    &presets_cache().preset_preview_colors
}

pub(crate) fn preset_theme_cached(index: usize) -> std::sync::Arc<Theme> {
    presets_cache().preset_themes[index].clone()
}

pub(crate) fn filtered_preset_indices_cached(filter: &str) -> Vec<usize> {
    let cache = presets_cache();
    let needle = normalize_preset_search_text(filter);

    let results = if needle.is_empty() {
        (0..cache.presets.len()).collect::<Vec<_>>()
    } else {
        cache
            .preset_search_blobs
            .iter()
            .enumerate()
            .filter_map(|(index, blob)| blob.contains(&needle).then_some(index))
            .collect::<Vec<_>>()
    };

    results
}

/// Find the index of the preset matching the given theme, or 0 if not found.
/// Matches on (background.main, accent.selected) which is unique per preset.
pub fn find_current_preset_index(theme: &Theme) -> usize {
    let key = preset_bg_accent_key(theme.colors.background.main, theme.colors.accent.selected);
    presets_cache()
        .preset_index_by_bg_accent
        .get(&key)
        .copied()
        .unwrap_or(0)
}

/// Result of classifying how a theme relates to stock presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetMatchKind {
    /// Theme is byte-for-byte identical to a stock preset.
    ExactMatch,
    /// Theme shares (background.main, accent.selected) with a preset but differs
    /// in accent, opacity, vibrancy, or other fields.
    Modified,
    /// Theme does not match any stock preset by key fields.
    Custom,
}

/// Full classification of a theme against the preset library.
#[derive(Debug, Clone)]
pub struct PresetMatchResult {
    /// Index into `presets_cached()` of the closest preset (0 if Custom).
    pub preset_index: usize,
    /// Classification kind.
    pub kind: PresetMatchKind,
}

impl PresetMatchResult {
    pub fn is_exact(&self) -> bool {
        self.kind == PresetMatchKind::ExactMatch
    }
}

/// Classify a theme against all stock presets.
///
/// Uses JSON serialization for exact comparison, which is reliable but not
/// suitable for hot render paths. Call once per preset resolution, not per frame.
pub fn classify_theme_preset_match(theme: &Theme) -> PresetMatchResult {
    let cache = presets_cache();
    let key = preset_bg_accent_key(theme.colors.background.main, theme.colors.accent.selected);

    if let Some(&preset_index) = cache.preset_index_by_bg_accent.get(&key) {
        let stock_theme = &cache.preset_themes[preset_index];

        // Exact match: compare via JSON serialization for reliability
        let exact = themes_are_equal(theme, stock_theme);

        let kind = if exact {
            PresetMatchKind::ExactMatch
        } else {
            PresetMatchKind::Modified
        };

        PresetMatchResult { preset_index, kind }
    } else {
        // No key match — fully custom theme. Fall back to index 0.
        PresetMatchResult {
            preset_index: 0,
            kind: PresetMatchKind::Custom,
        }
    }
}

/// Compare two themes by serialized JSON for field-level equality.
/// Handles f32 fields correctly via serde's representation.
fn themes_are_equal(a: &Theme, b: &Theme) -> bool {
    let Ok(json_a) = serde_json::to_string(a) else {
        return false;
    };
    let Ok(json_b) = serde_json::to_string(b) else {
        return false;
    };
    json_a == json_b
}

/// Index of the first light theme in all_presets() (used for section separator rendering)
pub fn first_light_theme_index() -> usize {
    presets_cache().first_light_theme_index
}

/// Pre-compute preview colors for all presets (avoids creating themes in render closures)
#[cfg(test)]
pub fn all_preset_preview_colors() -> Vec<PresetPreviewColors> {
    preset_preview_colors_cached().to_vec()
}

// ============================================================================
// Helper to build a theme from a color scheme
// ============================================================================

fn build_dark_theme(colors: ColorScheme) -> Theme {
    let opacity = BackgroundOpacity::dark_default();
    let colors = normalize_dark_interactive_tokens(colors, &opacity);
    Theme {
        colors,
        focus_aware: None,
        opacity: Some(opacity),
        drop_shadow: Some(DropShadow::default()),
        vibrancy: Some(VibrancySettings::default()),
        background_gradient: None,
        fonts: Some(FontConfig::default()),
        appearance: AppearanceMode::Dark,
    }
}

fn build_light_theme(colors: ColorScheme) -> Theme {
    let opacity = BackgroundOpacity::light_default();
    let colors = normalize_light_interactive_tokens(colors, &opacity);
    Theme {
        colors,
        focus_aware: None,
        opacity: Some(opacity),
        drop_shadow: Some(DropShadow {
            opacity: 0.12,
            ..DropShadow::default()
        }),
        vibrancy: Some(VibrancySettings::default()),
        background_gradient: None,
        fonts: Some(FontConfig::default()),
        appearance: AppearanceMode::Light,
    }
}

/// Minimum contrast ratio between composited selection background and plain
/// background for interactive state visibility.  1.10:1 matches the dark
/// theme's whisper-subtle selection and guarantees a perceptible darkening on
/// any light surface.
const MIN_SELECTION_VISIBILITY_RATIO: f32 = 1.10;

/// Minimum contrast ratio for hover at the (lower) hover opacity.  Hover is
/// deliberately whisper-subtle, but must still register as a perceptible
/// state change; 1.03:1 matches the third-party-preset audit threshold.
const MIN_HOVER_VISIBILITY_RATIO: f32 = 1.03;

/// Normalize `selected_subtle` and `on_accent` for light themes so the shared
/// chrome contract (selection highlights, hover states, accent badges) stays
/// legible regardless of which preset is active.
fn normalize_light_interactive_tokens(
    mut colors: ColorScheme,
    opacity: &BackgroundOpacity,
) -> ColorScheme {
    let bg = colors.background.main;

    // Keep primary body text as a hard black/white decision from the main
    // surface, rather than preserving tinted preset-specific text colors.
    colors.text.primary = super::helpers::hard_readable_text_hex(bg);

    // --- selected_subtle: ensure both selection AND hover highlights register ---
    if !passes_interactive_visibility(colors.accent.selected_subtle, bg, opacity) {
        let fixed = find_min_visible_selected_subtle(bg, opacity);
        colors.accent.selected_subtle = fixed;
    }

    // --- on_accent: ensure accent badge text is readable ---------------------
    let on_accent_ratio =
        super::helpers::contrast_ratio(colors.text.on_accent, colors.accent.selected);
    if on_accent_ratio < 3.0 {
        let fixed = super::best_readable_text_hex(colors.accent.selected);
        colors.text.on_accent = fixed;
    }

    colors
}

/// Normalize `selected_subtle` for dark themes using the same visibility
/// contract as light themes.  Dark presets with very dark `selected_subtle`
/// values (close to their background) get brightened toward white.
fn normalize_dark_interactive_tokens(
    mut colors: ColorScheme,
    opacity: &BackgroundOpacity,
) -> ColorScheme {
    let bg = colors.background.main;
    colors.text.primary = super::helpers::hard_readable_text_hex(bg);
    if !passes_interactive_visibility(colors.accent.selected_subtle, bg, opacity) {
        let fixed = find_min_visible_selected_subtle_dark(bg, opacity);
        colors.accent.selected_subtle = fixed;
    }

    colors
}

/// Returns true when `subtle` composited at the theme's selected and hover
/// opacities both clear their respective visibility thresholds against `bg`.
fn passes_interactive_visibility(subtle: u32, bg: u32, opacity: &BackgroundOpacity) -> bool {
    let sel = composite_over(subtle, opacity.selected, bg);
    let hov = composite_over(subtle, opacity.hover, bg);
    selection_visibility_ratio(sel, bg) >= MIN_SELECTION_VISIBILITY_RATIO
        && selection_visibility_ratio(hov, bg) >= MIN_HOVER_VISIBILITY_RATIO
}

/// Composite a foreground color at `alpha` over an opaque background.
fn composite_over(fg: u32, alpha: f32, bg: u32) -> u32 {
    let blend = |shift: u32| {
        let f = ((fg >> shift) & 0xFF) as f32;
        let b = ((bg >> shift) & 0xFF) as f32;
        (f * alpha + b * (1.0 - alpha)).round() as u32
    };
    (blend(16) << 16) | (blend(8) << 8) | blend(0)
}

/// WCAG-style contrast ratio between two opaque colors.
fn selection_visibility_ratio(composited: u32, bg: u32) -> f32 {
    let l1 = super::types::relative_luminance_srgb(composited);
    let l2 = super::types::relative_luminance_srgb(bg);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

/// Binary search for the closest-to-bg `selected_subtle` that still clears
/// both the selection and hover visibility thresholds at the theme's
/// selected/hover opacities.  For light themes: blends toward black.
fn find_min_visible_selected_subtle(bg: u32, opacity: &BackgroundOpacity) -> u32 {
    find_min_visible_selected_subtle_toward(bg, opacity, 0x000000)
}

/// Binary search counterpart for dark themes: blends toward white.
fn find_min_visible_selected_subtle_dark(bg: u32, opacity: &BackgroundOpacity) -> u32 {
    find_min_visible_selected_subtle_toward(bg, opacity, 0xFFFFFF)
}

/// Generic blend-toward-target binary search shared by light/dark normalization.
fn find_min_visible_selected_subtle_toward(
    bg: u32,
    opacity: &BackgroundOpacity,
    target: u32,
) -> u32 {
    let bg_r = ((bg >> 16) & 0xFF) as f32;
    let bg_g = ((bg >> 8) & 0xFF) as f32;
    let bg_b = (bg & 0xFF) as f32;
    let tgt_r = ((target >> 16) & 0xFF) as f32;
    let tgt_g = ((target >> 8) & 0xFF) as f32;
    let tgt_b = (target & 0xFF) as f32;

    let make_color = |t: f32| -> u32 {
        let r = (bg_r + (tgt_r - bg_r) * t).round() as u32;
        let g = (bg_g + (tgt_g - bg_g) * t).round() as u32;
        let b = (bg_b + (tgt_b - bg_b) * t).round() as u32;
        (r << 16) | (g << 8) | b
    };

    let check = |t: f32| -> bool { passes_interactive_visibility(make_color(t), bg, opacity) };

    let mut lo: f32 = 0.0;
    let mut hi: f32 = 1.0;
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        if check(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    make_color(hi)
}

// ============================================================================
// Theme Definitions
// ============================================================================

fn theme_script_kit_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x0f0f0f,
            title_bar: 0x1a1a1a,
            search_box: 0x2a2a2a,
            log_panel: 0x080808,
        },
        text: TextColors {
            primary: 0xffffff,
            secondary: 0xffffff,
            tertiary: 0xffffff,
            muted: 0xffffff,
            dimmed: 0xffffff,
            on_accent: 0x0f0f0f,
        },
        accent: AccentColors {
            selected: 0xfbbf24,
            selected_subtle: 0x5a5a5a,
        },
        ui: UIColors {
            border: 0x343434,
            success: 0x00ff00,
            error: 0xef4444,
            warning: 0xf59e0b,
            info: 0x3b82f6,
        },
        terminal: TerminalColors::dark_default(),
    })
}

fn theme_script_kit_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xffffff,
            title_bar: 0xffffff,
            search_box: 0xffffff,
            log_panel: 0xf5f5f5,
        },
        text: TextColors {
            primary: 0x111111,
            secondary: 0x111111,
            tertiary: 0x111111,
            muted: 0x111111,
            dimmed: 0x111111,
            on_accent: 0x000000,
        },
        accent: AccentColors {
            selected: 0x2f86d3,
            selected_subtle: 0xc0c0c0,
        },
        ui: UIColors {
            border: 0xe0e0e0,
            success: 0x22c55e,
            error: 0xdc2626,
            warning: 0xd97706,
            info: 0x2563eb,
        },
        terminal: TerminalColors::light_default(),
    })
}

// --- merged from part_02.rs ---
fn theme_dracula() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x282a36,
            title_bar: 0x21222c,
            search_box: 0x44475a,
            log_panel: 0x191a21,
        },
        text: TextColors {
            primary: 0xf8f8f2,
            secondary: 0xbfbfbf,
            tertiary: 0x6272a4,
            muted: 0x6272a4,
            dimmed: 0x44475a,
            on_accent: 0x282a36,
        },
        accent: AccentColors {
            selected: 0xbd93f9,
            selected_subtle: 0x5d5f6b,
        },
        ui: UIColors {
            border: 0x44475a,
            success: 0x50fa7b,
            error: 0xff5555,
            warning: 0xf1fa8c,
            info: 0x8be9fd,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x21222c,
            red: 0xff5555,
            green: 0x50fa7b,
            yellow: 0xf1fa8c,
            blue: 0xbd93f9,
            magenta: 0xff79c6,
            cyan: 0x8be9fd,
            white: 0xf8f8f2,
            bright_black: 0x6272a4,
            bright_red: 0xff6e6e,
            bright_green: 0x69ff94,
            bright_yellow: 0xffffa5,
            bright_blue: 0xd6acff,
            bright_magenta: 0xff92df,
            bright_cyan: 0xa4ffff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_nord() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x2e3440,
            title_bar: 0x3b4252,
            search_box: 0x434c5e,
            log_panel: 0x242933,
        },
        text: TextColors {
            primary: 0xeceff4,
            secondary: 0xd8dee9,
            tertiary: 0x81a1c1,
            muted: 0x7b88a1,
            dimmed: 0x4c566a,
            on_accent: 0x2e3440,
        },
        accent: AccentColors {
            selected: 0x88c0d0,
            selected_subtle: 0x627283,
        },
        ui: UIColors {
            border: 0x4c566a,
            success: 0xa3be8c,
            error: 0xbf616a,
            warning: 0xebcb8b,
            info: 0x81a1c1,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x3b4252,
            red: 0xbf616a,
            green: 0xa3be8c,
            yellow: 0xebcb8b,
            blue: 0x81a1c1,
            magenta: 0xb48ead,
            cyan: 0x88c0d0,
            white: 0xe5e9f0,
            bright_black: 0x4c566a,
            bright_red: 0xbf616a,
            bright_green: 0xa3be8c,
            bright_yellow: 0xebcb8b,
            bright_blue: 0x81a1c1,
            bright_magenta: 0xb48ead,
            bright_cyan: 0x8fbcbb,
            bright_white: 0xeceff4,
        },
    })
}

fn theme_catppuccin_mocha() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1e1e2e,
            title_bar: 0x181825,
            search_box: 0x313244,
            log_panel: 0x11111b,
        },
        text: TextColors {
            primary: 0xcdd6f4,
            secondary: 0xbac2de,
            tertiary: 0xa6adc8,
            muted: 0x7f849c,
            dimmed: 0x585b70,
            on_accent: 0x1e1e2e,
        },
        accent: AccentColors {
            selected: 0xcba6f7,
            selected_subtle: 0x53547f,
        },
        ui: UIColors {
            border: 0x45475a,
            success: 0xa6e3a1,
            error: 0xf38ba8,
            warning: 0xf9e2af,
            info: 0x89b4fa,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x45475a,
            red: 0xf38ba8,
            green: 0xa6e3a1,
            yellow: 0xf9e2af,
            blue: 0x89b4fa,
            magenta: 0xcba6f7,
            cyan: 0x94e2d5,
            white: 0xbac2de,
            bright_black: 0x585b70,
            bright_red: 0xf38ba8,
            bright_green: 0xa6e3a1,
            bright_yellow: 0xf9e2af,
            bright_blue: 0x89b4fa,
            bright_magenta: 0xcba6f7,
            bright_cyan: 0x94e2d5,
            bright_white: 0xa6adc8,
        },
    })
}

fn theme_catppuccin_latte() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xeff1f5,
            title_bar: 0xe6e9ef,
            search_box: 0xdce0e8,
            log_panel: 0xccd0da,
        },
        text: TextColors {
            primary: 0x4c4f69,
            secondary: 0x5c5f77,
            tertiary: 0x6c6f85,
            muted: 0x7e8194,
            dimmed: 0x9ca0b0,
            on_accent: 0xeff1f5,
        },
        accent: AccentColors {
            selected: 0x8839ef,
            selected_subtle: 0xb5b7c5,
        },
        ui: UIColors {
            border: 0xbcc0cc,
            success: 0x40a02b,
            error: 0xd20f39,
            warning: 0xdf8e1d,
            info: 0x1e66f5,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x5c5f77,
            red: 0xd20f39,
            green: 0x40a02b,
            yellow: 0xdf8e1d,
            blue: 0x1e66f5,
            magenta: 0x8839ef,
            cyan: 0x179299,
            white: 0xacb0be,
            bright_black: 0x6c6f85,
            bright_red: 0xd20f39,
            bright_green: 0x40a02b,
            bright_yellow: 0xdf8e1d,
            bright_blue: 0x1e66f5,
            bright_magenta: 0x8839ef,
            bright_cyan: 0x179299,
            bright_white: 0xbcc0cc,
        },
    })
}

fn theme_one_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x282c34,
            title_bar: 0x21252b,
            search_box: 0x3a3f4b,
            log_panel: 0x1b1d23,
        },
        text: TextColors {
            primary: 0xabb2bf,
            secondary: 0x9da5b4,
            tertiary: 0x7f848e,
            muted: 0x6e7990,
            dimmed: 0x4b5263,
            on_accent: 0x282c34,
        },
        accent: AccentColors {
            selected: 0x61afef,
            selected_subtle: 0x3e4452,
        },
        ui: UIColors {
            border: 0x3e4452,
            success: 0x98c379,
            error: 0xe06c75,
            warning: 0xe5c07b,
            info: 0x61afef,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x3f4451,
            red: 0xe06c75,
            green: 0x98c379,
            yellow: 0xe5c07b,
            blue: 0x61afef,
            magenta: 0xc678dd,
            cyan: 0x56b6c2,
            white: 0xabb2bf,
            bright_black: 0x4f5666,
            bright_red: 0xbe5046,
            bright_green: 0x98c379,
            bright_yellow: 0xd19a66,
            bright_blue: 0x61afef,
            bright_magenta: 0xc678dd,
            bright_cyan: 0x56b6c2,
            bright_white: 0xd7dae0,
        },
    })
}

fn theme_tokyo_night() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1a1b26,
            title_bar: 0x16161e,
            search_box: 0x292e42,
            log_panel: 0x13131a,
        },
        text: TextColors {
            primary: 0xc0caf5,
            secondary: 0xa9b1d6,
            tertiary: 0x737aa2,
            muted: 0x606a95,
            dimmed: 0x414868,
            on_accent: 0x1a1b26,
        },
        accent: AccentColors {
            selected: 0x7aa2f7,
            selected_subtle: 0x4c4f73,
        },
        ui: UIColors {
            border: 0x3b4261,
            success: 0x9ece6a,
            error: 0xf7768e,
            warning: 0xe0af68,
            info: 0x7dcfff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x414868,
            red: 0xf7768e,
            green: 0x9ece6a,
            yellow: 0xe0af68,
            blue: 0x7aa2f7,
            magenta: 0xbb9af7,
            cyan: 0x7dcfff,
            white: 0xa9b1d6,
            bright_black: 0x565f89,
            bright_red: 0xf7768e,
            bright_green: 0x9ece6a,
            bright_yellow: 0xe0af68,
            bright_blue: 0x7aa2f7,
            bright_magenta: 0xbb9af7,
            bright_cyan: 0x7dcfff,
            bright_white: 0xc0caf5,
        },
    })
}

fn theme_gruvbox_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x282828,
            title_bar: 0x1d2021,
            search_box: 0x3c3836,
            log_panel: 0x1d2021,
        },
        text: TextColors {
            primary: 0xebdbb2,
            secondary: 0xd5c4a1,
            tertiary: 0xa89984,
            muted: 0x928374,
            dimmed: 0x665c54,
            on_accent: 0x282828,
        },
        accent: AccentColors {
            selected: 0xfe8019,
            selected_subtle: 0x5e5650,
        },
        ui: UIColors {
            border: 0x504945,
            success: 0xb8bb26,
            error: 0xfb4934,
            warning: 0xfabd2f,
            info: 0x83a598,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x282828,
            red: 0xcc241d,
            green: 0x98971a,
            yellow: 0xd79921,
            blue: 0x458588,
            magenta: 0xb16286,
            cyan: 0x689d6a,
            white: 0xa89984,
            bright_black: 0x928374,
            bright_red: 0xfb4934,
            bright_green: 0xb8bb26,
            bright_yellow: 0xfabd2f,
            bright_blue: 0x83a598,
            bright_magenta: 0xd3869b,
            bright_cyan: 0x8ec07c,
            bright_white: 0xebdbb2,
        },
    })
}

fn theme_rose_pine() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x191724,
            title_bar: 0x1f1d2e,
            search_box: 0x26233a,
            log_panel: 0x13111e,
        },
        text: TextColors {
            primary: 0xe0def4,
            secondary: 0xc4a7e7,
            tertiary: 0x908caa,
            muted: 0x6e6a86,
            dimmed: 0x524f67,
            on_accent: 0x191724,
        },
        accent: AccentColors {
            selected: 0xebbcba,
            selected_subtle: 0x4a465d,
        },
        ui: UIColors {
            border: 0x403d52,
            success: 0x31748f,
            error: 0xeb6f92,
            warning: 0xf6c177,
            info: 0x9ccfd8,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x26233a,
            red: 0xeb6f92,
            green: 0x31748f,
            yellow: 0xf6c177,
            blue: 0x9ccfd8,
            magenta: 0xc4a7e7,
            cyan: 0xebbcba,
            white: 0xe0def4,
            bright_black: 0x6e6a86,
            bright_red: 0xeb6f92,
            bright_green: 0x31748f,
            bright_yellow: 0xf6c177,
            bright_blue: 0x9ccfd8,
            bright_magenta: 0xc4a7e7,
            bright_cyan: 0xebbcba,
            bright_white: 0xe0def4,
        },
    })
}

// --- merged from part_03.rs ---
fn theme_solarized_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x002b36,
            title_bar: 0x073642,
            search_box: 0x073642,
            log_panel: 0x001e26,
        },
        text: TextColors {
            primary: 0xfdf6e3,
            secondary: 0xeee8d5,
            tertiary: 0x93a1a1,
            muted: 0x839496,
            dimmed: 0x657b83,
            on_accent: 0x002b36,
        },
        accent: AccentColors {
            selected: 0x268bd2,
            selected_subtle: 0x1c5e6e,
        },
        ui: UIColors {
            border: 0x586e75,
            success: 0x859900,
            error: 0xdc322f,
            warning: 0xb58900,
            info: 0x268bd2,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x073642,
            red: 0xdc322f,
            green: 0x859900,
            yellow: 0xb58900,
            blue: 0x268bd2,
            magenta: 0xd33682,
            cyan: 0x2aa198,
            white: 0xeee8d5,
            bright_black: 0x586e75,
            bright_red: 0xcb4b16,
            bright_green: 0x859900,
            bright_yellow: 0xb58900,
            bright_blue: 0x268bd2,
            bright_magenta: 0x6c71c4,
            bright_cyan: 0x2aa198,
            bright_white: 0xfdf6e3,
        },
    })
}

fn theme_solarized_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfdf6e3,
            title_bar: 0xeee8d5,
            search_box: 0xeee8d5,
            log_panel: 0xe8e1cd,
        },
        text: TextColors {
            primary: 0x073642,
            secondary: 0x586e75,
            tertiary: 0x657b83,
            muted: 0x7b8c8e,
            dimmed: 0x93a1a1,
            on_accent: 0xfdf6e3,
        },
        accent: AccentColors {
            selected: 0x268bd2,
            selected_subtle: 0xbdb6a2,
        },
        ui: UIColors {
            border: 0x93a1a1,
            success: 0x859900,
            error: 0xdc322f,
            warning: 0xb58900,
            info: 0x268bd2,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x073642,
            red: 0xdc322f,
            green: 0x859900,
            yellow: 0xb58900,
            blue: 0x268bd2,
            magenta: 0xd33682,
            cyan: 0x2aa198,
            white: 0xeee8d5,
            bright_black: 0x586e75,
            bright_red: 0xcb4b16,
            bright_green: 0x859900,
            bright_yellow: 0xb58900,
            bright_blue: 0x268bd2,
            bright_magenta: 0x6c71c4,
            bright_cyan: 0x2aa198,
            bright_white: 0xfdf6e3,
        },
    })
}

fn theme_github_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x0d1117,
            title_bar: 0x161b22,
            search_box: 0x21262d,
            log_panel: 0x010409,
        },
        text: TextColors {
            primary: 0xf0f6fc,
            secondary: 0xc9d1d9,
            tertiary: 0x8b949e,
            muted: 0x6e7681,
            dimmed: 0x484f58,
            on_accent: 0x0d1117,
        },
        accent: AccentColors {
            selected: 0x58a6ff,
            selected_subtle: 0x3e4654,
        },
        ui: UIColors {
            border: 0x30363d,
            success: 0x3fb950,
            error: 0xf85149,
            warning: 0xd29922,
            info: 0x58a6ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x484f58,
            red: 0xff7b72,
            green: 0x3fb950,
            yellow: 0xd29922,
            blue: 0x58a6ff,
            magenta: 0xbc8cff,
            cyan: 0x39c5cf,
            white: 0xb1bac4,
            bright_black: 0x6e7681,
            bright_red: 0xffa198,
            bright_green: 0x56d364,
            bright_yellow: 0xe3b341,
            bright_blue: 0x79c0ff,
            bright_magenta: 0xd2a8ff,
            bright_cyan: 0x56d4dd,
            bright_white: 0xf0f6fc,
        },
    })
}

fn theme_github_dimmed() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x22272e,
            title_bar: 0x2d333b,
            search_box: 0x2d333b,
            log_panel: 0x1b1f24,
        },
        text: TextColors {
            primary: 0xadbac7,
            secondary: 0x909dab,
            tertiary: 0x768390,
            muted: 0x6e7a87,
            dimmed: 0x545d68,
            on_accent: 0x0d1117,
        },
        accent: AccentColors {
            selected: 0x539bf5,
            selected_subtle: 0x335480,
        },
        ui: UIColors {
            border: 0x444c56,
            success: 0x57ab5a,
            error: 0xe5534b,
            warning: 0xc69026,
            info: 0x539bf5,
        },
        terminal: TerminalColors {
            foreground: Some(0xadbac7),
            background: Some(0x1b1f24),
            ..TerminalColors::dark_default()
        },
    })
}

fn theme_github_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xffffff,
            title_bar: 0xf6f8fa,
            search_box: 0xf6f8fa,
            log_panel: 0xf0f2f4,
        },
        text: TextColors {
            primary: 0x1f2328,
            secondary: 0x424a53,
            tertiary: 0x656d76,
            muted: 0x818b98,
            dimmed: 0xafb8c1,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x0969da,
            selected_subtle: 0xc5c5c5,
        },
        ui: UIColors {
            border: 0xd0d7de,
            success: 0x1a7f37,
            error: 0xcf222e,
            warning: 0x9a6700,
            info: 0x0969da,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x24292f,
            red: 0xcf222e,
            green: 0x116329,
            yellow: 0x4d2d00,
            blue: 0x0550ae,
            magenta: 0x8250df,
            cyan: 0x1b7c83,
            white: 0x6e7781,
            bright_black: 0x57606a,
            bright_red: 0xa40e26,
            bright_green: 0x1a7f37,
            bright_yellow: 0x633c01,
            bright_blue: 0x0969da,
            bright_magenta: 0x8250df,
            bright_cyan: 0x1b7c83,
            bright_white: 0x8c959f,
        },
    })
}

fn theme_monokai_pro() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x2d2a2e,
            title_bar: 0x221f22,
            search_box: 0x403e41,
            log_panel: 0x19181a,
        },
        text: TextColors {
            primary: 0xfcfcfa,
            secondary: 0xc1c0c0,
            tertiary: 0x939293,
            muted: 0x7a787a,
            dimmed: 0x5b595c,
            on_accent: 0x2d2a2e,
        },
        accent: AccentColors {
            selected: 0xffd866,
            selected_subtle: 0x625f62,
        },
        ui: UIColors {
            border: 0x403e41,
            success: 0xa9dc76,
            error: 0xff6188,
            warning: 0xfc9867,
            info: 0x78dce8,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x403e41,
            red: 0xff6188,
            green: 0xa9dc76,
            yellow: 0xffd866,
            blue: 0x78dce8,
            magenta: 0xab9df2,
            cyan: 0x78dce8,
            white: 0xfcfcfa,
            bright_black: 0x727072,
            bright_red: 0xff6188,
            bright_green: 0xa9dc76,
            bright_yellow: 0xffd866,
            bright_blue: 0x78dce8,
            bright_magenta: 0xab9df2,
            bright_cyan: 0x78dce8,
            bright_white: 0xfcfcfa,
        },
    })
}

fn theme_everforest_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x2d353b,
            title_bar: 0x272e33,
            search_box: 0x343f44,
            log_panel: 0x232a2e,
        },
        text: TextColors {
            primary: 0xd3c6aa,
            secondary: 0x9da9a0,
            tertiary: 0x859289,
            muted: 0x7a8478,
            dimmed: 0x56635f,
            on_accent: 0x2d353b,
        },
        accent: AccentColors {
            selected: 0xa7c080,
            selected_subtle: 0x616c6e,
        },
        ui: UIColors {
            border: 0x475258,
            success: 0xa7c080,
            error: 0xe67e80,
            warning: 0xdbbc7f,
            info: 0x7fbbb3,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x343f44,
            red: 0xe67e80,
            green: 0xa7c080,
            yellow: 0xdbbc7f,
            blue: 0x7fbbb3,
            magenta: 0xd699b6,
            cyan: 0x83c092,
            white: 0xd3c6aa,
            bright_black: 0x56635f,
            bright_red: 0xe67e80,
            bright_green: 0xa7c080,
            bright_yellow: 0xdbbc7f,
            bright_blue: 0x7fbbb3,
            bright_magenta: 0xd699b6,
            bright_cyan: 0x83c092,
            bright_white: 0xd3c6aa,
        },
    })
}

fn theme_kanagawa() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1f1f28,
            title_bar: 0x1a1a22,
            search_box: 0x2a2a37,
            log_panel: 0x16161d,
        },
        text: TextColors {
            primary: 0xdcd7ba,
            secondary: 0xc8c093,
            tertiary: 0x727169,
            muted: 0x736f6b,
            dimmed: 0x54546d,
            on_accent: 0x1f1f28,
        },
        accent: AccentColors {
            selected: 0x7e9cd8,
            selected_subtle: 0x555363,
        },
        ui: UIColors {
            border: 0x54546d,
            success: 0x76946a,
            error: 0xc34043,
            warning: 0xc0a36e,
            info: 0x7fb4ca,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x2a2a37,
            red: 0xc34043,
            green: 0x76946a,
            yellow: 0xc0a36e,
            blue: 0x7e9cd8,
            magenta: 0x957fb8,
            cyan: 0x6a9589,
            white: 0xdcd7ba,
            bright_black: 0x54546d,
            bright_red: 0xe82424,
            bright_green: 0x98bb6c,
            bright_yellow: 0xe6c384,
            bright_blue: 0x7fb4ca,
            bright_magenta: 0x938aa9,
            bright_cyan: 0x7aa89f,
            bright_white: 0xc8c093,
        },
    })
}

fn theme_ayu_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x0a0e14,
            title_bar: 0x070a0f,
            search_box: 0x1d2631,
            log_panel: 0x050709,
        },
        text: TextColors {
            primary: 0xb3b1ad,
            secondary: 0x9a9892,
            tertiary: 0x626a73,
            muted: 0x586177,
            dimmed: 0x3d4455,
            on_accent: 0x0a0e14,
        },
        accent: AccentColors {
            selected: 0xe6b450,
            selected_subtle: 0x3a4251,
        },
        ui: UIColors {
            border: 0x1d2631,
            success: 0xc2d94c,
            error: 0xff3333,
            warning: 0xff8f40,
            info: 0x59c2ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1d2631,
            red: 0xff3333,
            green: 0xc2d94c,
            yellow: 0xe6b450,
            blue: 0x59c2ff,
            magenta: 0xd2a6ff,
            cyan: 0x95e6cb,
            white: 0xb3b1ad,
            bright_black: 0x626a73,
            bright_red: 0xff3333,
            bright_green: 0xc2d94c,
            bright_yellow: 0xe6b450,
            bright_blue: 0x59c2ff,
            bright_magenta: 0xd2a6ff,
            bright_cyan: 0x95e6cb,
            bright_white: 0xb3b1ad,
        },
    })
}

fn theme_material_ocean() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x0f111a,
            title_bar: 0x090b10,
            search_box: 0x1f2233,
            log_panel: 0x070810,
        },
        text: TextColors {
            primary: 0xeeffff,
            secondary: 0xb0bec5,
            tertiary: 0x8f93a2,
            muted: 0x717cb4,
            dimmed: 0x3b3f51,
            on_accent: 0x0f111a,
        },
        accent: AccentColors {
            selected: 0x84ffff,
            selected_subtle: 0x3f4457,
        },
        ui: UIColors {
            border: 0x1f2233,
            success: 0xc3e88d,
            error: 0xff5370,
            warning: 0xffcb6b,
            info: 0x82aaff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1f2233,
            red: 0xff5370,
            green: 0xc3e88d,
            yellow: 0xffcb6b,
            blue: 0x82aaff,
            magenta: 0xc792ea,
            cyan: 0x89ddff,
            white: 0xeeffff,
            bright_black: 0x3b3f51,
            bright_red: 0xff5370,
            bright_green: 0xc3e88d,
            bright_yellow: 0xffcb6b,
            bright_blue: 0x82aaff,
            bright_magenta: 0xc792ea,
            bright_cyan: 0x89ddff,
            bright_white: 0xeeffff,
        },
    })
}

// ============================================================================
// New Theme Definitions
// ============================================================================

fn theme_monokai() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x272822,
            title_bar: 0x1e1f1c,
            search_box: 0x414339,
            log_panel: 0x1e1f1c,
        },
        text: TextColors {
            primary: 0xf8f8f2,
            secondary: 0xccccc7,
            tertiary: 0xc2c2bf,
            muted: 0x88846f,
            dimmed: 0x464741,
            on_accent: 0x272822,
        },
        accent: AccentColors {
            selected: 0xf92672,
            selected_subtle: 0x5d5e57,
        },
        ui: UIColors {
            border: 0x414339,
            success: 0xa6e22e,
            error: 0xf92672,
            warning: 0xe6db74,
            info: 0x66d9ef,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x333333,
            red: 0xc4265e,
            green: 0x86b42b,
            yellow: 0xb3b42b,
            blue: 0x6a7ec8,
            magenta: 0x8c6bc8,
            cyan: 0x56adbc,
            white: 0xe3e3dd,
            bright_black: 0x666666,
            bright_red: 0xf92672,
            bright_green: 0xa6e22e,
            bright_yellow: 0xe2e22e,
            bright_blue: 0x819aff,
            bright_magenta: 0xae81ff,
            bright_cyan: 0x66d9ef,
            bright_white: 0xf8f8f2,
        },
    })
}

fn theme_one_dark_pro() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x282c34,
            title_bar: 0x21252b,
            search_box: 0x1d1f23,
            log_panel: 0x21252b,
        },
        text: TextColors {
            primary: 0xabb2bf,
            secondary: 0x9da5b4,
            tertiary: 0x7f848e,
            muted: 0x6e7682,
            dimmed: 0x495162,
            on_accent: 0x282c34,
        },
        accent: AccentColors {
            selected: 0x528bff,
            selected_subtle: 0x3e4452,
        },
        ui: UIColors {
            border: 0x3e4452,
            success: 0x98c379,
            error: 0xe06c75,
            warning: 0xe5c07b,
            info: 0x528bff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x3f4451,
            red: 0xe05561,
            green: 0x8cc265,
            yellow: 0xd18f52,
            blue: 0x4aa5f0,
            magenta: 0xc162de,
            cyan: 0x42b3c2,
            white: 0xd7dae0,
            bright_black: 0x4f5666,
            bright_red: 0xff616e,
            bright_green: 0xa5e075,
            bright_yellow: 0xf0a45d,
            bright_blue: 0x4dc4ff,
            bright_magenta: 0xde73ff,
            bright_cyan: 0x4cd1e0,
            bright_white: 0xe6e6e6,
        },
    })
}

fn theme_tokyo_night_storm() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x24283b,
            title_bar: 0x1f2335,
            search_box: 0x1b1e2e,
            log_panel: 0x1f2335,
        },
        text: TextColors {
            primary: 0xa9b1d6,
            secondary: 0x8891bc,
            tertiary: 0x545c7e,
            muted: 0x6a75a3,
            dimmed: 0x414868,
            on_accent: 0x1f2335,
        },
        accent: AccentColors {
            selected: 0x7aa2f7,
            selected_subtle: 0x535975,
        },
        ui: UIColors {
            border: 0x3b4261,
            success: 0x9ece6a,
            error: 0xf7768e,
            warning: 0xe0af68,
            info: 0x7dcfff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x414868,
            red: 0xf7768e,
            green: 0x73daca,
            yellow: 0xe0af68,
            blue: 0x7aa2f7,
            magenta: 0xbb9af7,
            cyan: 0x7dcfff,
            white: 0x8089b3,
            bright_black: 0x414868,
            bright_red: 0xf7768e,
            bright_green: 0x73daca,
            bright_yellow: 0xe0af68,
            bright_blue: 0x7aa2f7,
            bright_magenta: 0xbb9af7,
            bright_cyan: 0x7dcfff,
            bright_white: 0xa9b1d6,
        },
    })
}

fn theme_rose_pine_moon() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x232136,
            title_bar: 0x232136,
            search_box: 0x393552,
            log_panel: 0x2a273f,
        },
        text: TextColors {
            primary: 0xe0def4,
            secondary: 0xc4a7e7,
            tertiary: 0x908caa,
            muted: 0x6e6a86,
            dimmed: 0x817c9c,
            on_accent: 0x232136,
        },
        accent: AccentColors {
            selected: 0xea9a97,
            selected_subtle: 0x575170,
        },
        ui: UIColors {
            border: 0x817c9c,
            success: 0x3e8fb0,
            error: 0xeb6f92,
            warning: 0xf6c177,
            info: 0x9ccfd8,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x393552,
            red: 0xeb6f92,
            green: 0x3e8fb0,
            yellow: 0xf6c177,
            blue: 0x9ccfd8,
            magenta: 0xc4a7e7,
            cyan: 0xea9a97,
            white: 0xe0def4,
            bright_black: 0x908caa,
            bright_red: 0xeb6f92,
            bright_green: 0x3e8fb0,
            bright_yellow: 0xf6c177,
            bright_blue: 0x9ccfd8,
            bright_magenta: 0xc4a7e7,
            bright_cyan: 0xea9a97,
            bright_white: 0xe0def4,
        },
    })
}

fn theme_poimandres() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1b1e28,
            title_bar: 0x1b1e28,
            search_box: 0x303340,
            log_panel: 0x1b1e28,
        },
        text: TextColors {
            primary: 0xa6accd,
            secondary: 0xa6accd,
            tertiary: 0x767c9d,
            muted: 0x767c9d,
            dimmed: 0x3d4050,
            on_accent: 0x1b1e28,
        },
        accent: AccentColors {
            selected: 0x89ddff,
            selected_subtle: 0x4d5265,
        },
        ui: UIColors {
            border: 0x303340,
            success: 0x5de4c7,
            error: 0xd0679d,
            warning: 0xfffac2,
            info: 0x89ddff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1b1e28,
            red: 0xd0679d,
            green: 0x5de4c7,
            yellow: 0xfffac2,
            blue: 0x89ddff,
            magenta: 0xf087bd,
            cyan: 0x89ddff,
            white: 0xffffff,
            bright_black: 0xa6accd,
            bright_red: 0xd0679d,
            bright_green: 0x5de4c7,
            bright_yellow: 0xfffac2,
            bright_blue: 0xadd7ff,
            bright_magenta: 0xf087bd,
            bright_cyan: 0xadd7ff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_palenight() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x292d3e,
            title_bar: 0x282c3d,
            search_box: 0x313850,
            log_panel: 0x292d3e,
        },
        text: TextColors {
            primary: 0xbfc7d5,
            secondary: 0x929ac9,
            tertiary: 0x6c739a,
            muted: 0x7078a2,
            dimmed: 0x4c5374,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x8b65ce,
            selected_subtle: 0x4c5374,
        },
        ui: UIColors {
            border: 0x282b3c,
            success: 0xc3e88d,
            error: 0xff5572,
            warning: 0xffcb6b,
            info: 0x82aaff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x676e95,
            red: 0xff5572,
            green: 0xa9c77d,
            yellow: 0xffcb6b,
            blue: 0x82aaff,
            magenta: 0xc792ea,
            cyan: 0x89ddff,
            white: 0xffffff,
            bright_black: 0x676e95,
            bright_red: 0xff5572,
            bright_green: 0xc3e88d,
            bright_yellow: 0xffcb6b,
            bright_blue: 0x82aaff,
            bright_magenta: 0xc792ea,
            bright_cyan: 0x89ddff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_horizon_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1c1e26,
            title_bar: 0x1c1e26,
            search_box: 0x2e303e,
            log_panel: 0x1a1c23,
        },
        text: TextColors {
            primary: 0xd5d8da,
            secondary: 0xbbbbbb,
            tertiary: 0x6c6f93,
            muted: 0xbbbbbb,
            dimmed: 0x6c6f93,
            on_accent: 0x1c1e26,
        },
        accent: AccentColors {
            selected: 0xe95378,
            selected_subtle: 0x4e515f,
        },
        ui: UIColors {
            border: 0x2e303e,
            success: 0x29d398,
            error: 0xf43e5c,
            warning: 0xfab795,
            info: 0x26bbd9,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1c1e26,
            red: 0xe95678,
            green: 0x29d398,
            yellow: 0xfab795,
            blue: 0x26bbd9,
            magenta: 0xee64ac,
            cyan: 0x59e1e3,
            white: 0xd5d8da,
            bright_black: 0x6c6f93,
            bright_red: 0xec6a88,
            bright_green: 0x3fdaa4,
            bright_yellow: 0xfbc3a7,
            bright_blue: 0x3fc4de,
            bright_magenta: 0xf075b5,
            bright_cyan: 0x6be4e6,
            bright_white: 0xd5d8da,
        },
    })
}

fn theme_andromeda() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x23262e,
            title_bar: 0x23262e,
            search_box: 0x2b303b,
            log_panel: 0x1b1d23,
        },
        text: TextColors {
            primary: 0xd5ced9,
            secondary: 0xbaafc0,
            tertiary: 0x999999,
            muted: 0xa0a1a7,
            dimmed: 0x746f77,
            on_accent: 0x20232b,
        },
        accent: AccentColors {
            selected: 0x00e8c6,
            selected_subtle: 0x545861,
        },
        ui: UIColors {
            border: 0x1b1d23,
            success: 0x96e072,
            error: 0xfc644d,
            warning: 0xffe66d,
            info: 0x7cb7ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x23262e,
            red: 0xee5d43,
            green: 0x96e072,
            yellow: 0xffe66d,
            blue: 0x7cb7ff,
            magenta: 0xff00aa,
            cyan: 0x00e8c6,
            white: 0xd5ced9,
            bright_black: 0x746f77,
            bright_red: 0xee5d43,
            bright_green: 0x96e072,
            bright_yellow: 0xffe66d,
            bright_blue: 0x7cb7ff,
            bright_magenta: 0xff00aa,
            bright_cyan: 0x00e8c6,
            bright_white: 0xd5ced9,
        },
    })
}

fn theme_synthwave_84() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x262335,
            title_bar: 0x241b2f,
            search_box: 0x2a2139,
            log_panel: 0x171520,
        },
        text: TextColors {
            primary: 0xffffff,
            secondary: 0xb6b1b1,
            tertiary: 0x848bbd,
            muted: 0x848bbd,
            dimmed: 0x495495,
            on_accent: 0x171520,
        },
        accent: AccentColors {
            selected: 0xff7edb,
            selected_subtle: 0x5d536d,
        },
        ui: UIColors {
            border: 0x495495,
            success: 0x72f1b8,
            error: 0xfe4450,
            warning: 0xf3e70f,
            info: 0x03edf9,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x171520,
            red: 0xfe4450,
            green: 0x72f1b8,
            yellow: 0xf3e70f,
            blue: 0x03edf9,
            magenta: 0xff7edb,
            cyan: 0x03edf9,
            white: 0xffffff,
            bright_black: 0x848bbd,
            bright_red: 0xfe4450,
            bright_green: 0x72f1b8,
            bright_yellow: 0xfede5d,
            bright_blue: 0x03edf9,
            bright_magenta: 0xff7edb,
            bright_cyan: 0x03edf9,
            bright_white: 0xffffff,
        },
    })
}

fn theme_shades_of_purple() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x2d2b55,
            title_bar: 0x1e1e3f,
            search_box: 0x2d2b55,
            log_panel: 0x1e1e3f,
        },
        text: TextColors {
            primary: 0xffffff,
            secondary: 0xa599e9,
            tertiary: 0xa599e9,
            muted: 0xb362ff,
            dimmed: 0x5c5c61,
            on_accent: 0x222244,
        },
        accent: AccentColors {
            selected: 0xfad000,
            selected_subtle: 0x67638f,
        },
        ui: UIColors {
            border: 0xfad000,
            success: 0x3ad900,
            error: 0xec3a37,
            warning: 0xfad000,
            info: 0x7857fe,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x000000,
            red: 0xec3a37,
            green: 0x3ad900,
            yellow: 0xfad000,
            blue: 0x7857fe,
            magenta: 0xff2c70,
            cyan: 0x80fcff,
            white: 0xffffff,
            bright_black: 0x5c5c61,
            bright_red: 0xec3a37,
            bright_green: 0x3ad900,
            bright_yellow: 0xfad000,
            bright_blue: 0x6943ff,
            bright_magenta: 0xfb94ff,
            bright_cyan: 0x80fcff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_cobalt2() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x193549,
            title_bar: 0x15232d,
            search_box: 0x193549,
            log_panel: 0x122738,
        },
        text: TextColors {
            primary: 0xffffff,
            secondary: 0xaaaaaa,
            tertiary: 0xaaaaaa,
            muted: 0x0088ff,
            dimmed: 0x0050a4,
            on_accent: 0x000000,
        },
        accent: AccentColors {
            selected: 0xffc600,
            selected_subtle: 0x4b6b83,
        },
        ui: UIColors {
            border: 0x0d3a58,
            success: 0x3ad900,
            error: 0xff628c,
            warning: 0xffc600,
            info: 0x0088ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x000000,
            red: 0xff628c,
            green: 0x3ad900,
            yellow: 0xffc600,
            blue: 0x0088ff,
            magenta: 0xfb94ff,
            cyan: 0x80fcff,
            white: 0xffffff,
            bright_black: 0x0050a4,
            bright_red: 0xff628c,
            bright_green: 0x3ad900,
            bright_yellow: 0xffc600,
            bright_blue: 0x0088ff,
            bright_magenta: 0xfb94ff,
            bright_cyan: 0x80fcff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_ayu_mirage() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1f2430,
            title_bar: 0x1f2430,
            search_box: 0x242936,
            log_panel: 0x1f2430,
        },
        text: TextColors {
            primary: 0xcccac2,
            secondary: 0x828ea1,
            tertiary: 0x6e7c8f,
            muted: 0x6e7c8f,
            dimmed: 0x686868,
            on_accent: 0x735923,
        },
        accent: AccentColors {
            selected: 0xffcc66,
            selected_subtle: 0x4f5668,
        },
        ui: UIColors {
            border: 0x171b24,
            success: 0x87d96c,
            error: 0xf28273,
            warning: 0xfcca60,
            info: 0x6acdff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x171b24,
            red: 0xf28273,
            green: 0x87d96c,
            yellow: 0xfcca60,
            blue: 0x6acdff,
            magenta: 0xddbbff,
            cyan: 0x93e2c8,
            white: 0xc7c7c7,
            bright_black: 0x686868,
            bright_red: 0xf28779,
            bright_green: 0xd5ff80,
            bright_yellow: 0xffcd66,
            bright_blue: 0x73d0ff,
            bright_magenta: 0xdfbfff,
            bright_cyan: 0x95e6cb,
            bright_white: 0xffffff,
        },
    })
}

fn theme_night_owl() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x011627,
            title_bar: 0x011627,
            search_box: 0x0b253a,
            log_panel: 0x0b2942,
        },
        text: TextColors {
            primary: 0xd6deeb,
            secondary: 0x89a4bb,
            tertiary: 0x5f7e97,
            muted: 0x637777,
            dimmed: 0x4b6479,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x7e57c2,
            selected_subtle: 0x2b5368,
        },
        ui: UIColors {
            border: 0x5f7e97,
            success: 0x22da6e,
            error: 0xef5350,
            warning: 0xc5e478,
            info: 0x82aaff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x011627,
            red: 0xef5350,
            green: 0x22da6e,
            yellow: 0xc5e478,
            blue: 0x82aaff,
            magenta: 0xc792ea,
            cyan: 0x21c7a8,
            white: 0xffffff,
            bright_black: 0x575656,
            bright_red: 0xef5350,
            bright_green: 0x22da6e,
            bright_yellow: 0xffeb95,
            bright_blue: 0x82aaff,
            bright_magenta: 0xc792ea,
            bright_cyan: 0x7fdbca,
            bright_white: 0xffffff,
        },
    })
}

fn theme_vitesse_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x121212,
            title_bar: 0x121212,
            search_box: 0x181818,
            log_panel: 0x121212,
        },
        text: TextColors {
            primary: 0xdbd7ca,
            secondary: 0xbfbaaa,
            tertiary: 0x959da5,
            muted: 0x758575,
            dimmed: 0x777777,
            on_accent: 0x121212,
        },
        accent: AccentColors {
            selected: 0x4d9375,
            selected_subtle: 0x4b4b4b,
        },
        ui: UIColors {
            border: 0x191919,
            success: 0x4d9375,
            error: 0xcb7676,
            warning: 0xe6cc77,
            info: 0x6394bf,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x393a34,
            red: 0xcb7676,
            green: 0x4d9375,
            yellow: 0xe6cc77,
            blue: 0x6394bf,
            magenta: 0xd9739f,
            cyan: 0x5eaab5,
            white: 0xdbd7ca,
            bright_black: 0x777777,
            bright_red: 0xcb7676,
            bright_green: 0x4d9375,
            bright_yellow: 0xe6cc77,
            bright_blue: 0x6394bf,
            bright_magenta: 0xd9739f,
            bright_cyan: 0x5eaab5,
            bright_white: 0xffffff,
        },
    })
}

fn theme_rose_pine_dawn() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfaf4ed,
            title_bar: 0xfaf4ed,
            search_box: 0xf2e9e1,
            log_panel: 0xfffaf3,
        },
        text: TextColors {
            primary: 0x575279,
            secondary: 0x7a6493,
            tertiary: 0x797593,
            muted: 0x877f98,
            dimmed: 0x9893a5,
            on_accent: 0xfaf4ed,
        },
        accent: AccentColors {
            selected: 0xc06b67,
            selected_subtle: 0xc4bfb7,
        },
        ui: UIColors {
            border: 0x6e6a86,
            success: 0x286983,
            error: 0xb4637a,
            warning: 0xea9d34,
            info: 0x56949f,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0xf2e9e1,
            red: 0xb4637a,
            green: 0x286983,
            yellow: 0xea9d34,
            blue: 0x56949f,
            magenta: 0x907aa9,
            cyan: 0xd7827e,
            white: 0x575279,
            bright_black: 0x797593,
            bright_red: 0xb4637a,
            bright_green: 0x286983,
            bright_yellow: 0xea9d34,
            bright_blue: 0x56949f,
            bright_magenta: 0x907aa9,
            bright_cyan: 0xd7827e,
            bright_white: 0x575279,
        },
    })
}

fn theme_everforest_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfdf6e3,
            title_bar: 0xfdf6e3,
            search_box: 0xEDEAD5,
            log_panel: 0xefebd4,
        },
        text: TextColors {
            primary: 0x5c6a72,
            secondary: 0x627660,
            tertiary: 0x939f91,
            muted: 0x868f80,
            dimmed: 0xa4ad9e,
            on_accent: 0x2d3b1e,
        },
        accent: AccentColors {
            selected: 0x6f8a38,
            selected_subtle: 0xd0ccb8,
        },
        ui: UIColors {
            border: 0xe0dcc7,
            success: 0x8da101,
            error: 0xf85552,
            warning: 0xdfa000,
            info: 0x3a94c5,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x5c6a72,
            red: 0xf85552,
            green: 0x8da101,
            yellow: 0xdfa000,
            blue: 0x3a94c5,
            magenta: 0xdf69ba,
            cyan: 0x35a77c,
            white: 0x939f91,
            bright_black: 0x5c6a72,
            bright_red: 0xf85552,
            bright_green: 0x8da101,
            bright_yellow: 0xdfa000,
            bright_blue: 0x3a94c5,
            bright_magenta: 0xdf69ba,
            bright_cyan: 0x35a77c,
            bright_white: 0xf4f0d9,
        },
    })
}

fn theme_vitesse_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xffffff,
            title_bar: 0xffffff,
            search_box: 0xf7f7f7,
            log_panel: 0xf7f7f7,
        },
        text: TextColors {
            primary: 0x393a34,
            secondary: 0x4e4f47,
            tertiary: 0x6a737d,
            muted: 0x859285,
            dimmed: 0xaaaaaa,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x1c6b48,
            selected_subtle: 0xc5c5c2,
        },
        ui: UIColors {
            border: 0xf0f0f0,
            success: 0x1e754f,
            error: 0xab5959,
            warning: 0xbda437,
            info: 0x296aa3,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x121212,
            red: 0xab5959,
            green: 0x1e754f,
            yellow: 0xbda437,
            blue: 0x296aa3,
            magenta: 0xa13865,
            cyan: 0x2993a3,
            white: 0xdbd7ca,
            bright_black: 0xaaaaaa,
            bright_red: 0xab5959,
            bright_green: 0x1e754f,
            bright_yellow: 0xbda437,
            bright_blue: 0x296aa3,
            bright_magenta: 0xa13865,
            bright_cyan: 0x2993a3,
            bright_white: 0xdddddd,
        },
    })
}

fn theme_ayu_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xf8f9fa,
            title_bar: 0xf8f9fa,
            search_box: 0xfcfcfc,
            log_panel: 0xfafafa,
        },
        text: TextColors {
            primary: 0x5c6166,
            secondary: 0x687484,
            tertiary: 0x687484,
            muted: 0x8e8f92,
            dimmed: 0xc4c4c4,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0xc07810,
            selected_subtle: 0xbbbbbd,
        },
        ui: UIColors {
            border: 0x828e9f,
            success: 0x6cbf43,
            error: 0xf06b6c,
            warning: 0xe7a100,
            info: 0x21a1e2,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x000000,
            red: 0xf06b6c,
            green: 0x6cbf43,
            yellow: 0xe7a100,
            blue: 0x21a1e2,
            magenta: 0xa176cb,
            cyan: 0x4abc96,
            white: 0xc7c7c7,
            bright_black: 0x686868,
            bright_red: 0xf07171,
            bright_green: 0x86b300,
            bright_yellow: 0xeba400,
            bright_blue: 0x22a4e6,
            bright_magenta: 0xa37acc,
            bright_cyan: 0x4cbf99,
            bright_white: 0xd1d1d1,
        },
    })
}

fn theme_night_owl_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfbfbfb,
            title_bar: 0xf0f0f0,
            search_box: 0xf0f0f0,
            log_panel: 0xf6f6f6,
        },
        text: TextColors {
            primary: 0x403f53,
            secondary: 0x403f53,
            tertiary: 0x93a1a1,
            muted: 0x848b9d,
            dimmed: 0x90a7b2,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x2aa298,
            selected_subtle: 0xbebebe,
        },
        ui: UIColors {
            border: 0xd9d9d9,
            success: 0x08916a,
            error: 0xde3d3b,
            warning: 0xe0af02,
            info: 0x288ed7,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x403f53,
            red: 0xde3d3b,
            green: 0x08916a,
            yellow: 0xe0af02,
            blue: 0x288ed7,
            magenta: 0xd6438a,
            cyan: 0x2aa298,
            white: 0x93a1a1,
            bright_black: 0x403f53,
            bright_red: 0xde3d3b,
            bright_green: 0x08916a,
            bright_yellow: 0xdaaa01,
            bright_blue: 0x288ed7,
            bright_magenta: 0xd6438a,
            bright_cyan: 0x2aa298,
            bright_white: 0x93a1a1,
        },
    })
}

fn theme_tokyo_day() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xeef4ff,
            title_bar: 0xffffff,
            search_box: 0xffffff,
            log_panel: 0xe8f1ff,
        },
        text: TextColors {
            primary: 0x1f2329,
            secondary: 0x3b4261,
            tertiary: 0x56617a,
            muted: 0x6b7394,
            dimmed: 0x8b93aa,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x34548a,
            selected_subtle: 0xc8d8f0,
        },
        ui: UIColors {
            border: 0xd0daf0,
            success: 0x33635c,
            error: 0x8c4351,
            warning: 0x8f5e15,
            info: 0x2959aa,
        },
        terminal: TerminalColors {
            foreground: Some(0x1f2329),
            background: Some(0xe8f1ff),
            ..TerminalColors::light_default()
        },
    })
}

// ============================================================================
// Batch 2 — Additional Theme Definitions
// ============================================================================

fn theme_gruvbox_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfbf1c7,
            title_bar: 0xf2e5bc,
            search_box: 0xebdbb2,
            log_panel: 0xf2e5bc,
        },
        text: TextColors {
            primary: 0x3c3836,
            secondary: 0x504945,
            tertiary: 0x665c54,
            muted: 0x7c6f64,
            dimmed: 0x928374,
            on_accent: 0xfbf1c7,
        },
        accent: AccentColors {
            selected: 0xd65d0e,
            selected_subtle: 0xbfb59f,
        },
        ui: UIColors {
            border: 0xd5c4a1,
            success: 0x79740e,
            error: 0x9d0006,
            warning: 0xb57614,
            info: 0x076678,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x282828,
            red: 0x9d0006,
            green: 0x79740e,
            yellow: 0xb57614,
            blue: 0x076678,
            magenta: 0x8f3f71,
            cyan: 0x427b58,
            white: 0xa89984,
            bright_black: 0x928374,
            bright_red: 0xcc241d,
            bright_green: 0x98971a,
            bright_yellow: 0xd79921,
            bright_blue: 0x458588,
            bright_magenta: 0xb16286,
            bright_cyan: 0x689d6a,
            bright_white: 0xebdbb2,
        },
    })
}

fn theme_catppuccin_frappe() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x303446,
            title_bar: 0x292c3c,
            search_box: 0x414559,
            log_panel: 0x232634,
        },
        text: TextColors {
            primary: 0xc6d0f5,
            secondary: 0xb5bfe2,
            tertiary: 0xa5adce,
            muted: 0x838ba7,
            dimmed: 0x626880,
            on_accent: 0x303446,
        },
        accent: AccentColors {
            selected: 0xca9ee6,
            selected_subtle: 0x616883,
        },
        ui: UIColors {
            border: 0x51576d,
            success: 0xa6d189,
            error: 0xe78284,
            warning: 0xe5c890,
            info: 0x8caaee,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x51576d,
            red: 0xe78284,
            green: 0xa6d189,
            yellow: 0xe5c890,
            blue: 0x8caaee,
            magenta: 0xca9ee6,
            cyan: 0x81c8be,
            white: 0xb5bfe2,
            bright_black: 0x626880,
            bright_red: 0xe78284,
            bright_green: 0xa6d189,
            bright_yellow: 0xe5c890,
            bright_blue: 0x8caaee,
            bright_magenta: 0xca9ee6,
            bright_cyan: 0x81c8be,
            bright_white: 0xa5adce,
        },
    })
}

fn theme_catppuccin_macchiato() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x24273a,
            title_bar: 0x1e2030,
            search_box: 0x363a4f,
            log_panel: 0x181926,
        },
        text: TextColors {
            primary: 0xcad3f5,
            secondary: 0xb8c0e0,
            tertiary: 0xa5adcb,
            muted: 0x8087a2,
            dimmed: 0x5b6078,
            on_accent: 0x24273a,
        },
        accent: AccentColors {
            selected: 0xc6a0f6,
            selected_subtle: 0x575c78,
        },
        ui: UIColors {
            border: 0x494d64,
            success: 0xa6da95,
            error: 0xed8796,
            warning: 0xeed49f,
            info: 0x8aadf4,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x494d64,
            red: 0xed8796,
            green: 0xa6da95,
            yellow: 0xeed49f,
            blue: 0x8aadf4,
            magenta: 0xc6a0f6,
            cyan: 0x8bd5ca,
            white: 0xb8c0e0,
            bright_black: 0x5b6078,
            bright_red: 0xed8796,
            bright_green: 0xa6da95,
            bright_yellow: 0xeed49f,
            bright_blue: 0x8aadf4,
            bright_magenta: 0xc6a0f6,
            bright_cyan: 0x8bd5ca,
            bright_white: 0xa5adcb,
        },
    })
}

fn theme_darcula() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x2b2b2b,
            title_bar: 0x3c3f41,
            search_box: 0x45494a,
            log_panel: 0x242424,
        },
        text: TextColors {
            primary: 0xa9b7c6,
            secondary: 0x959595,
            tertiary: 0x6a8759,
            muted: 0x629755,
            dimmed: 0x4e5254,
            on_accent: 0x2b2b2b,
        },
        accent: AccentColors {
            selected: 0x6897bb,
            selected_subtle: 0x4e5254,
        },
        ui: UIColors {
            border: 0x4e5254,
            success: 0x6a8759,
            error: 0xcc7832,
            warning: 0xffc66d,
            info: 0x6897bb,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x2b2b2b,
            red: 0xcc7832,
            green: 0x6a8759,
            yellow: 0xffc66d,
            blue: 0x6897bb,
            magenta: 0x9876aa,
            cyan: 0x629755,
            white: 0xa9b7c6,
            bright_black: 0x555555,
            bright_red: 0xcc7832,
            bright_green: 0x6a8759,
            bright_yellow: 0xffc66d,
            bright_blue: 0x6897bb,
            bright_magenta: 0x9876aa,
            bright_cyan: 0x629755,
            bright_white: 0xffffff,
        },
    })
}

fn theme_intellij_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xffffff,
            title_bar: 0xf2f2f2,
            search_box: 0xf2f2f2,
            log_panel: 0xf7f7f7,
        },
        text: TextColors {
            primary: 0x080808,
            secondary: 0x000080,
            tertiary: 0x808080,
            muted: 0x8b8b8b,
            dimmed: 0xbbbbbb,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x4a86c8,
            selected_subtle: 0xc5c5c5,
        },
        ui: UIColors {
            border: 0xd1d1d1,
            success: 0x008000,
            error: 0xff0000,
            warning: 0xebc700,
            info: 0x4a86c8,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x000000,
            red: 0xff0000,
            green: 0x008000,
            yellow: 0xebc700,
            blue: 0x0000ff,
            magenta: 0x800080,
            cyan: 0x008080,
            white: 0xc0c0c0,
            bright_black: 0x808080,
            bright_red: 0xff0000,
            bright_green: 0x008000,
            bright_yellow: 0xebc700,
            bright_blue: 0x4a86c8,
            bright_magenta: 0x9876aa,
            bright_cyan: 0x008080,
            bright_white: 0xffffff,
        },
    })
}

fn theme_moonlight() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x222436,
            title_bar: 0x1e2030,
            search_box: 0x2f334d,
            log_panel: 0x191a2a,
        },
        text: TextColors {
            primary: 0xc8d3f5,
            secondary: 0xa9b8e8,
            tertiary: 0x828bb8,
            muted: 0x636da6,
            dimmed: 0x444a73,
            on_accent: 0x222436,
        },
        accent: AccentColors {
            selected: 0x82aaff,
            selected_subtle: 0x535874,
        },
        ui: UIColors {
            border: 0x444a73,
            success: 0xc3e88d,
            error: 0xff757f,
            warning: 0xffc777,
            info: 0x82aaff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x444a73,
            red: 0xff757f,
            green: 0xc3e88d,
            yellow: 0xffc777,
            blue: 0x82aaff,
            magenta: 0xc099ff,
            cyan: 0x86e1fc,
            white: 0xc8d3f5,
            bright_black: 0x636da6,
            bright_red: 0xff757f,
            bright_green: 0xc3e88d,
            bright_yellow: 0xffc777,
            bright_blue: 0x82aaff,
            bright_magenta: 0xc099ff,
            bright_cyan: 0x86e1fc,
            bright_white: 0xc8d3f5,
        },
    })
}

fn theme_nightfly() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x011627,
            title_bar: 0x011627,
            search_box: 0x0b2942,
            log_panel: 0x010e1a,
        },
        text: TextColors {
            primary: 0xbdc1c6,
            secondary: 0xacb4c2,
            tertiary: 0x7c8f8f,
            muted: 0x5c6773,
            dimmed: 0x3a5068,
            on_accent: 0x011627,
        },
        accent: AccentColors {
            selected: 0x82aaff,
            selected_subtle: 0x2a5168,
        },
        ui: UIColors {
            border: 0x1d3b53,
            success: 0xa1cd5e,
            error: 0xfc514e,
            warning: 0xe3d367,
            info: 0x82aaff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1d3b53,
            red: 0xfc514e,
            green: 0xa1cd5e,
            yellow: 0xe3d367,
            blue: 0x82aaff,
            magenta: 0xc792ea,
            cyan: 0x7fdbca,
            white: 0xa1aab8,
            bright_black: 0x7c8f8f,
            bright_red: 0xfc514e,
            bright_green: 0xa1cd5e,
            bright_yellow: 0xe3d367,
            bright_blue: 0x82aaff,
            bright_magenta: 0xc792ea,
            bright_cyan: 0x7fdbca,
            bright_white: 0xd6deeb,
        },
    })
}

fn theme_oxocarbon_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x161616,
            title_bar: 0x262626,
            search_box: 0x353535,
            log_panel: 0x0e0e0e,
        },
        text: TextColors {
            primary: 0xf2f4f8,
            secondary: 0xdde1e6,
            tertiary: 0xb6b8bb,
            muted: 0x878d96,
            dimmed: 0x525252,
            on_accent: 0x161616,
        },
        accent: AccentColors {
            selected: 0x78a9ff,
            selected_subtle: 0x4d4d4d,
        },
        ui: UIColors {
            border: 0x393939,
            success: 0x42be65,
            error: 0xee5396,
            warning: 0xf1c21b,
            info: 0x78a9ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x262626,
            red: 0xee5396,
            green: 0x42be65,
            yellow: 0xf1c21b,
            blue: 0x78a9ff,
            magenta: 0xbe95ff,
            cyan: 0x33b1ff,
            white: 0xdde1e6,
            bright_black: 0x525252,
            bright_red: 0xff7eb6,
            bright_green: 0x42be65,
            bright_yellow: 0xf1c21b,
            bright_blue: 0x78a9ff,
            bright_magenta: 0xbe95ff,
            bright_cyan: 0x33b1ff,
            bright_white: 0xf2f4f8,
        },
    })
}

fn theme_flexoki_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x100f0f,
            title_bar: 0x1c1b1a,
            search_box: 0x282726,
            log_panel: 0x0a0908,
        },
        text: TextColors {
            primary: 0xececec,
            secondary: 0xb7b5ac,
            tertiary: 0x878580,
            muted: 0x666562,
            dimmed: 0x403e3c,
            on_accent: 0x100f0f,
        },
        accent: AccentColors {
            selected: 0xd0a215,
            selected_subtle: 0x484746,
        },
        ui: UIColors {
            border: 0x343331,
            success: 0x879a39,
            error: 0xd14d41,
            warning: 0xda702c,
            info: 0x4385be,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1c1b1a,
            red: 0xd14d41,
            green: 0x879a39,
            yellow: 0xd0a215,
            blue: 0x4385be,
            magenta: 0xce5d97,
            cyan: 0x3aa99f,
            white: 0xb7b5ac,
            bright_black: 0x575653,
            bright_red: 0xd14d41,
            bright_green: 0x879a39,
            bright_yellow: 0xd0a215,
            bright_blue: 0x4385be,
            bright_magenta: 0xce5d97,
            bright_cyan: 0x3aa99f,
            bright_white: 0xececec,
        },
    })
}

fn theme_flexoki_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfffcf0,
            title_bar: 0xf2f0e5,
            search_box: 0xe6e4d9,
            log_panel: 0xf2f0e5,
        },
        text: TextColors {
            primary: 0x100f0f,
            secondary: 0x403e3c,
            tertiary: 0x575653,
            muted: 0x878580,
            dimmed: 0xb7b5ac,
            on_accent: 0xfffcf0,
        },
        accent: AccentColors {
            selected: 0xad8301,
            selected_subtle: 0xc5c0b0,
        },
        ui: UIColors {
            border: 0xd0cec5,
            success: 0x66800b,
            error: 0xaf3029,
            warning: 0xbc5215,
            info: 0x205ea6,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x100f0f,
            red: 0xaf3029,
            green: 0x66800b,
            yellow: 0xad8301,
            blue: 0x205ea6,
            magenta: 0xa02f6f,
            cyan: 0x24837b,
            white: 0xb7b5ac,
            bright_black: 0x878580,
            bright_red: 0xd14d41,
            bright_green: 0x879a39,
            bright_yellow: 0xd0a215,
            bright_blue: 0x4385be,
            bright_magenta: 0xce5d97,
            bright_cyan: 0x3aa99f,
            bright_white: 0xececec,
        },
    })
}

fn theme_kanagawa_dragon() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x181616,
            title_bar: 0x0d0c0c,
            search_box: 0x282727,
            log_panel: 0x0d0c0c,
        },
        text: TextColors {
            primary: 0xc5c9c5,
            secondary: 0xa6a69c,
            tertiary: 0x8a8980,
            muted: 0x737c73,
            dimmed: 0x625e5a,
            on_accent: 0x181616,
        },
        accent: AccentColors {
            selected: 0x8ba4b0,
            selected_subtle: 0x4d4b4b,
        },
        ui: UIColors {
            border: 0x393836,
            success: 0x87a987,
            error: 0xc4746e,
            warning: 0xc4b28a,
            info: 0x8ba4b0,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x0d0c0c,
            red: 0xc4746e,
            green: 0x87a987,
            yellow: 0xc4b28a,
            blue: 0x8ba4b0,
            magenta: 0xa292a3,
            cyan: 0x8ea4a2,
            white: 0xc5c9c5,
            bright_black: 0x625e5a,
            bright_red: 0xe46876,
            bright_green: 0x87a987,
            bright_yellow: 0xe6c384,
            bright_blue: 0x7fb4ca,
            bright_magenta: 0x938aa9,
            bright_cyan: 0x7aa89f,
            bright_white: 0xc5c9c5,
        },
    })
}

fn theme_kanagawa_lotus() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xf2ecbc,
            title_bar: 0xe7dba0,
            search_box: 0xd5cea3,
            log_panel: 0xe7dba0,
        },
        text: TextColors {
            primary: 0x545464,
            secondary: 0x43436c,
            tertiary: 0x8a8980,
            muted: 0x868378,
            dimmed: 0xb5b3aa,
            on_accent: 0xf2ecbc,
        },
        accent: AccentColors {
            selected: 0xc84053,
            selected_subtle: 0xbab494,
        },
        ui: UIColors {
            border: 0xc7c7a5,
            success: 0x6f894e,
            error: 0xc84053,
            warning: 0x77713f,
            info: 0x4d699b,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x545464,
            red: 0xc84053,
            green: 0x6f894e,
            yellow: 0x77713f,
            blue: 0x4d699b,
            magenta: 0xb35b79,
            cyan: 0x597b75,
            white: 0xd5cea3,
            bright_black: 0x8a8980,
            bright_red: 0xd7474b,
            bright_green: 0x6f894e,
            bright_yellow: 0x836f4a,
            bright_blue: 0x6693bf,
            bright_magenta: 0x624c83,
            bright_cyan: 0x5e857a,
            bright_white: 0xf2ecbc,
        },
    })
}

fn theme_iceberg_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x161821,
            title_bar: 0x1e2132,
            search_box: 0x2a2f45,
            log_panel: 0x0f1117,
        },
        text: TextColors {
            primary: 0xc6c8d1,
            secondary: 0x9a9ca5,
            tertiary: 0x6b7089,
            muted: 0x636880,
            dimmed: 0x3e4359,
            on_accent: 0x161821,
        },
        accent: AccentColors {
            selected: 0x84a0c6,
            selected_subtle: 0x4a4e64,
        },
        ui: UIColors {
            border: 0x2a2f45,
            success: 0xb4be82,
            error: 0xe27878,
            warning: 0xe2a478,
            info: 0x84a0c6,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1e2132,
            red: 0xe27878,
            green: 0xb4be82,
            yellow: 0xe2a478,
            blue: 0x84a0c6,
            magenta: 0xa093c7,
            cyan: 0x89b8c2,
            white: 0xc6c8d1,
            bright_black: 0x6b7089,
            bright_red: 0xe98989,
            bright_green: 0xc0ca8e,
            bright_yellow: 0xe9b189,
            bright_blue: 0x91acd1,
            bright_magenta: 0xada0d3,
            bright_cyan: 0x95c4ce,
            bright_white: 0xd2d4de,
        },
    })
}

fn theme_iceberg_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xe8e9ec,
            title_bar: 0xdcdfe7,
            search_box: 0xd0d3dc,
            log_panel: 0xdcdfe7,
        },
        text: TextColors {
            primary: 0x33374c,
            secondary: 0x454964,
            tertiary: 0x6b7089,
            muted: 0x757b95,
            dimmed: 0x9fa4bf,
            on_accent: 0xe8e9ec,
        },
        accent: AccentColors {
            selected: 0x2d539e,
            selected_subtle: 0xb4b5bd,
        },
        ui: UIColors {
            border: 0xc7c9d1,
            success: 0x668e3d,
            error: 0xcc517a,
            warning: 0xc57339,
            info: 0x2d539e,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x33374c,
            red: 0xcc517a,
            green: 0x668e3d,
            yellow: 0xc57339,
            blue: 0x2d539e,
            magenta: 0x7759b4,
            cyan: 0x3f83a6,
            white: 0xdcdfe7,
            bright_black: 0x6b7089,
            bright_red: 0xcc517a,
            bright_green: 0x668e3d,
            bright_yellow: 0xc57339,
            bright_blue: 0x2d539e,
            bright_magenta: 0x7759b4,
            bright_cyan: 0x3f83a6,
            bright_white: 0xe8e9ec,
        },
    })
}

fn theme_bluloco_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x282c34,
            title_bar: 0x21252b,
            search_box: 0x363c46,
            log_panel: 0x1c1f26,
        },
        text: TextColors {
            primary: 0xabb2bf,
            secondary: 0x9da5b4,
            tertiary: 0x7a82da,
            muted: 0x6e7990,
            dimmed: 0x474c5a,
            on_accent: 0x282c34,
        },
        accent: AccentColors {
            selected: 0x3691ff,
            selected_subtle: 0x596170,
        },
        ui: UIColors {
            border: 0x3f4451,
            success: 0x3fc56b,
            error: 0xff6480,
            warning: 0xf9c859,
            info: 0x3691ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x41444d,
            red: 0xfc2f52,
            green: 0x25a45c,
            yellow: 0xff936a,
            blue: 0x3476ff,
            magenta: 0x7a82da,
            cyan: 0x4fa1c5,
            white: 0xabb2bf,
            bright_black: 0x4f5666,
            bright_red: 0xff6480,
            bright_green: 0x3fc56b,
            bright_yellow: 0xf9c859,
            bright_blue: 0x10b1fe,
            bright_magenta: 0xff78f8,
            bright_cyan: 0x5fb9bc,
            bright_white: 0xffffff,
        },
    })
}

fn theme_bluloco_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xf9f9f9,
            title_bar: 0xf0f0f0,
            search_box: 0xe8e8e8,
            log_panel: 0xf0f0f0,
        },
        text: TextColors {
            primary: 0x383a42,
            secondary: 0x4a4e56,
            tertiary: 0x828fa1,
            muted: 0x888990,
            dimmed: 0xc4c4c4,
            on_accent: 0xffffff,
        },
        accent: AccentColors {
            selected: 0x275fe4,
            selected_subtle: 0xbdbdbd,
        },
        ui: UIColors {
            border: 0xe0e0e0,
            success: 0x23974a,
            error: 0xd52753,
            warning: 0xdf631c,
            info: 0x275fe4,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x373a41,
            red: 0xd52753,
            green: 0x23974a,
            yellow: 0xdf631c,
            blue: 0x275fe4,
            magenta: 0x823ff1,
            cyan: 0x2e8f82,
            white: 0xf9f9f9,
            bright_black: 0x7f8290,
            bright_red: 0xff6480,
            bright_green: 0x3cbc66,
            bright_yellow: 0xc5a332,
            bright_blue: 0x0098dd,
            bright_magenta: 0xc54bdb,
            bright_cyan: 0x27a7b2,
            bright_white: 0xffffff,
        },
    })
}

fn theme_atom_one_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfafafa,
            title_bar: 0xf0f0f0,
            search_box: 0xeaeaeb,
            log_panel: 0xf0f0f0,
        },
        text: TextColors {
            primary: 0x383a42,
            secondary: 0x4f525e,
            tertiary: 0x696c77,
            muted: 0x888990,
            dimmed: 0xc4c4c4,
            on_accent: 0xfafafa,
        },
        accent: AccentColors {
            selected: 0x4078f2,
            selected_subtle: 0xbebebe,
        },
        ui: UIColors {
            border: 0xdbdbdc,
            success: 0x50a14f,
            error: 0xe45649,
            warning: 0xc18401,
            info: 0x4078f2,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x383a42,
            red: 0xe45649,
            green: 0x50a14f,
            yellow: 0xc18401,
            blue: 0x4078f2,
            magenta: 0xa626a4,
            cyan: 0x0184bc,
            white: 0xa0a1a7,
            bright_black: 0x696c77,
            bright_red: 0xe45649,
            bright_green: 0x50a14f,
            bright_yellow: 0xc18401,
            bright_blue: 0x4078f2,
            bright_magenta: 0xa626a4,
            bright_cyan: 0x0184bc,
            bright_white: 0xfafafa,
        },
    })
}

fn theme_aura_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x15141b,
            title_bar: 0x110f18,
            search_box: 0x1c1b22,
            log_panel: 0x110f18,
        },
        text: TextColors {
            primary: 0xedecee,
            secondary: 0xbdbdbd,
            tertiary: 0x6d6d6d,
            muted: 0x65646a,
            dimmed: 0x3d3b45,
            on_accent: 0x15141b,
        },
        accent: AccentColors {
            selected: 0xa277ff,
            selected_subtle: 0x4e4d55,
        },
        ui: UIColors {
            border: 0x29263c,
            success: 0x61ffca,
            error: 0xff6767,
            warning: 0xffca85,
            info: 0x82e2ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x110f18,
            red: 0xff6767,
            green: 0x61ffca,
            yellow: 0xffca85,
            blue: 0x82e2ff,
            magenta: 0xa277ff,
            cyan: 0x82e2ff,
            white: 0xedecee,
            bright_black: 0x525156,
            bright_red: 0xff6767,
            bright_green: 0x61ffca,
            bright_yellow: 0xffca85,
            bright_blue: 0x82e2ff,
            bright_magenta: 0xa277ff,
            bright_cyan: 0x82e2ff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_panda_syntax() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x292a2b,
            title_bar: 0x242526,
            search_box: 0x3b3c3d,
            log_panel: 0x1f2021,
        },
        text: TextColors {
            primary: 0xe6e6e6,
            secondary: 0xcccccc,
            tertiary: 0x757575,
            muted: 0x727685,
            dimmed: 0x4d4f56,
            on_accent: 0x292a2b,
        },
        accent: AccentColors {
            selected: 0x19f9d8,
            selected_subtle: 0x5f6162,
        },
        ui: UIColors {
            border: 0x3b3c3d,
            success: 0x19f9d8,
            error: 0xff2c6d,
            warning: 0xffb86c,
            info: 0x6fc1ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1f2021,
            red: 0xff2c6d,
            green: 0x19f9d8,
            yellow: 0xffb86c,
            blue: 0x45a9f9,
            magenta: 0xff75b5,
            cyan: 0x6fc1ff,
            white: 0xe6e6e6,
            bright_black: 0x757575,
            bright_red: 0xff4b82,
            bright_green: 0x19f9d8,
            bright_yellow: 0xffcc95,
            bright_blue: 0x6fc1ff,
            bright_magenta: 0xff9ac1,
            bright_cyan: 0x89ddff,
            bright_white: 0xffffff,
        },
    })
}

fn theme_laserwave() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x27212e,
            title_bar: 0x211b27,
            search_box: 0x332d3b,
            log_panel: 0x1d1722,
        },
        text: TextColors {
            primary: 0xffffff,
            secondary: 0xb4a5c8,
            tertiary: 0x91889b,
            muted: 0x746c89,
            dimmed: 0x4c4456,
            on_accent: 0x27212e,
        },
        accent: AccentColors {
            selected: 0xeb64b9,
            selected_subtle: 0x5f5668,
        },
        ui: UIColors {
            border: 0x40394a,
            success: 0x74dfc4,
            error: 0xeb64b9,
            warning: 0xffe261,
            info: 0x40b4c4,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x27212e,
            red: 0xeb64b9,
            green: 0x74dfc4,
            yellow: 0xffe261,
            blue: 0x40b4c4,
            magenta: 0xb381c5,
            cyan: 0x40b4c4,
            white: 0xffffff,
            bright_black: 0x91889b,
            bright_red: 0xeb64b9,
            bright_green: 0x74dfc4,
            bright_yellow: 0xffe261,
            bright_blue: 0x40b4c4,
            bright_magenta: 0xb381c5,
            bright_cyan: 0x40b4c4,
            bright_white: 0xffffff,
        },
    })
}

fn theme_fairy_floss() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x5a5475,
            title_bar: 0x4f4a68,
            search_box: 0x6b6589,
            log_panel: 0x4f4a68,
        },
        text: TextColors {
            primary: 0xf8f8f2,
            secondary: 0xf0cc20,
            tertiary: 0xc2c0c4,
            muted: 0xb0a8ae,
            dimmed: 0x8078a8,
            on_accent: 0x5a5475,
        },
        accent: AccentColors {
            selected: 0xffb8d1,
            selected_subtle: 0x928bb0,
        },
        ui: UIColors {
            border: 0x716799,
            success: 0xc2ffdf,
            error: 0xff857f,
            warning: 0xe6c000,
            info: 0xc5a3ff,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x5a5475,
            red: 0xff857f,
            green: 0xc2ffdf,
            yellow: 0xe6c000,
            blue: 0xc5a3ff,
            magenta: 0xffb8d1,
            cyan: 0xc2ffdf,
            white: 0xf8f8f2,
            bright_black: 0x716799,
            bright_red: 0xff857f,
            bright_green: 0xc2ffdf,
            bright_yellow: 0xe6c000,
            bright_blue: 0xc5a3ff,
            bright_magenta: 0xffb8d1,
            bright_cyan: 0xc2ffdf,
            bright_white: 0xffffff,
        },
    })
}

fn theme_zenburn() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x3f3f3f,
            title_bar: 0x383838,
            search_box: 0x4f4f4f,
            log_panel: 0x333333,
        },
        text: TextColors {
            primary: 0xdcdccc,
            secondary: 0xc0c0a0,
            tertiary: 0x9fafaf,
            muted: 0x7f9f7f,
            dimmed: 0x5f7f5f,
            on_accent: 0x3f3f3f,
        },
        accent: AccentColors {
            selected: 0xf0dfaf,
            selected_subtle: 0x6e6e6b,
        },
        ui: UIColors {
            border: 0x5f5f5f,
            success: 0x7f9f7f,
            error: 0xcc9393,
            warning: 0xf0dfaf,
            info: 0x8cd0d3,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1e2320,
            red: 0xcc9393,
            green: 0x7f9f7f,
            yellow: 0xf0dfaf,
            blue: 0x8cd0d3,
            magenta: 0xdc8cc3,
            cyan: 0x93e0e3,
            white: 0xdcdccc,
            bright_black: 0x5f7f5f,
            bright_red: 0xdca3a3,
            bright_green: 0xbfebbf,
            bright_yellow: 0xf0efd0,
            bright_blue: 0x94bff3,
            bright_magenta: 0xec93d3,
            bright_cyan: 0x93e0e3,
            bright_white: 0xffffff,
        },
    })
}

fn theme_srcery() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1c1b19,
            title_bar: 0x151413,
            search_box: 0x2d2c29,
            log_panel: 0x121110,
        },
        text: TextColors {
            primary: 0xfce8c3,
            secondary: 0xd0bfa1,
            tertiary: 0x918175,
            muted: 0x767064,
            dimmed: 0x504a45,
            on_accent: 0x1c1b19,
        },
        accent: AccentColors {
            selected: 0xfbb829,
            selected_subtle: 0x575350,
        },
        ui: UIColors {
            border: 0x2d2c29,
            success: 0x98bc37,
            error: 0xef2f27,
            warning: 0xff5f00,
            info: 0x68a8e4,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1c1b19,
            red: 0xef2f27,
            green: 0x519f50,
            yellow: 0xfbb829,
            blue: 0x2c78bf,
            magenta: 0xe02c6d,
            cyan: 0x0aaeb3,
            white: 0xbaa67f,
            bright_black: 0x918175,
            bright_red: 0xf75341,
            bright_green: 0x98bc37,
            bright_yellow: 0xfed06e,
            bright_blue: 0x68a8e4,
            bright_magenta: 0xff5c8f,
            bright_cyan: 0x53fde9,
            bright_white: 0xfce8c3,
        },
    })
}

fn theme_papercolor_dark() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1c1c1c,
            title_bar: 0x1c1c1c,
            search_box: 0x303030,
            log_panel: 0x121212,
        },
        text: TextColors {
            primary: 0xd0d0d0,
            secondary: 0xb2b2b2,
            tertiary: 0x808080,
            muted: 0x6b6b6b,
            dimmed: 0x3e3e3e,
            on_accent: 0x1c1c1c,
        },
        accent: AccentColors {
            selected: 0x5fafd7,
            selected_subtle: 0x545454,
        },
        ui: UIColors {
            border: 0x303030,
            success: 0x5faf5f,
            error: 0xaf005f,
            warning: 0xd7af5f,
            info: 0x5fafd7,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1c1c1c,
            red: 0xaf005f,
            green: 0x5faf00,
            yellow: 0xd7af5f,
            blue: 0x5fafd7,
            magenta: 0x808080,
            cyan: 0xd7875f,
            white: 0xd0d0d0,
            bright_black: 0x585858,
            bright_red: 0x5faf5f,
            bright_green: 0xafd700,
            bright_yellow: 0xaf87d7,
            bright_blue: 0xffaf00,
            bright_magenta: 0xff5faf,
            bright_cyan: 0x00afaf,
            bright_white: 0x5f8787,
        },
    })
}

fn theme_papercolor_light() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xeeeeee,
            title_bar: 0xe4e4e4,
            search_box: 0xd7d7d7,
            log_panel: 0xe4e4e4,
        },
        text: TextColors {
            primary: 0x444444,
            secondary: 0x4a7070,
            tertiary: 0x878787,
            muted: 0x858585,
            dimmed: 0xbcbcbc,
            on_accent: 0xeeeeee,
        },
        accent: AccentColors {
            selected: 0x005faf,
            selected_subtle: 0xb4b4b4,
        },
        ui: UIColors {
            border: 0xbcbcbc,
            success: 0x008700,
            error: 0xaf0000,
            warning: 0xd75f00,
            info: 0x005faf,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0xeeeeee,
            red: 0xaf0000,
            green: 0x008700,
            yellow: 0x5f8700,
            blue: 0x005faf,
            magenta: 0x878787,
            cyan: 0x005f87,
            white: 0x444444,
            bright_black: 0xbcbcbc,
            bright_red: 0xd70000,
            bright_green: 0xd70087,
            bright_yellow: 0x8700af,
            bright_blue: 0xd75f00,
            bright_magenta: 0xd75f00,
            bright_cyan: 0x005faf,
            bright_white: 0x005f87,
        },
    })
}

fn theme_vesper() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x101010,
            title_bar: 0x181818,
            search_box: 0x1c1c1c,
            log_panel: 0x141414,
        },
        text: TextColors {
            primary: 0xd4cfc9,
            secondary: 0xa09a93,
            tertiary: 0x7b756f,
            muted: 0x756e67,
            dimmed: 0x403a34,
            on_accent: 0x101010,
        },
        accent: AccentColors {
            selected: 0xffc799,
            selected_subtle: 0x3d3528,
        },
        ui: UIColors {
            border: 0x2a2520,
            success: 0x6bbd6b,
            error: 0xd47766,
            warning: 0xdba16b,
            info: 0x7ab0df,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x101010,
            red: 0xd47766,
            green: 0x6bbd6b,
            yellow: 0xdba16b,
            blue: 0x7ab0df,
            magenta: 0xc49ec4,
            cyan: 0x5db5a4,
            white: 0xd4cfc9,
            bright_black: 0x605a54,
            bright_red: 0xe89a8c,
            bright_green: 0x8cd48c,
            bright_yellow: 0xe8bd8c,
            bright_blue: 0x9ec6e8,
            bright_magenta: 0xd4b8d4,
            bright_cyan: 0x80ccbd,
            bright_white: 0xede8e2,
        },
    })
}

fn theme_alabaster() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xf7f1e3,
            title_bar: 0xeee8d5,
            search_box: 0xe8e0cc,
            log_panel: 0xeee8d5,
        },
        text: TextColors {
            primary: 0x434343,
            secondary: 0x6a6a5e,
            tertiary: 0x8b8b7a,
            muted: 0x8a8578,
            dimmed: 0xccc7b8,
            on_accent: 0xf7f1e3,
        },
        accent: AccentColors {
            selected: 0x007acc,
            selected_subtle: 0xc8d8e8,
        },
        ui: UIColors {
            border: 0xd5cfbb,
            success: 0x448c27,
            error: 0xaa3731,
            warning: 0xc18401,
            info: 0x007acc,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0xf7f1e3,
            red: 0xaa3731,
            green: 0x448c27,
            yellow: 0xc18401,
            blue: 0x325cc0,
            magenta: 0x7a3e9d,
            cyan: 0x0083b2,
            white: 0x434343,
            bright_black: 0xada99e,
            bright_red: 0xd32f2f,
            bright_green: 0x558b2f,
            bright_yellow: 0xf9a825,
            bright_blue: 0x1565c0,
            bright_magenta: 0x9c27b0,
            bright_cyan: 0x00838f,
            bright_white: 0x2c2c2c,
        },
    })
}

fn theme_midnight_blue() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x0d1b2a,
            title_bar: 0x1b2838,
            search_box: 0x1b3a4b,
            log_panel: 0x1b2838,
        },
        text: TextColors {
            primary: 0xd6e4f0,
            secondary: 0x8eacc5,
            tertiary: 0x6b8faa,
            muted: 0x5a7d99,
            dimmed: 0x2e4a5e,
            on_accent: 0x0d1b2a,
        },
        accent: AccentColors {
            selected: 0x5b9bd5,
            selected_subtle: 0x1e3a5f,
        },
        ui: UIColors {
            border: 0x2e4a5e,
            success: 0x5cad6a,
            error: 0xd9534f,
            warning: 0xe0a458,
            info: 0x5b9bd5,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x0d1b2a,
            red: 0xd9534f,
            green: 0x5cad6a,
            yellow: 0xe0a458,
            blue: 0x5b9bd5,
            magenta: 0xb48ead,
            cyan: 0x5fb3b3,
            white: 0xd6e4f0,
            bright_black: 0x2e4a5e,
            bright_red: 0xef6b6b,
            bright_green: 0x7cc98a,
            bright_yellow: 0xf0c674,
            bright_blue: 0x81b5e8,
            bright_magenta: 0xc9a0dc,
            bright_cyan: 0x7fcfcf,
            bright_white: 0xeef4fa,
        },
    })
}

fn theme_ember() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x1a1210,
            title_bar: 0x241c18,
            search_box: 0x2e2420,
            log_panel: 0x241c18,
        },
        text: TextColors {
            primary: 0xe8d5c4,
            secondary: 0xb89a82,
            tertiary: 0x9a7d66,
            muted: 0x8a6f5a,
            dimmed: 0x4a3828,
            on_accent: 0x1a1210,
        },
        accent: AccentColors {
            selected: 0xe0954a,
            selected_subtle: 0x4a3020,
        },
        ui: UIColors {
            border: 0x4a3828,
            success: 0x7eb563,
            error: 0xd95050,
            warning: 0xe0954a,
            info: 0x6ba3be,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x1a1210,
            red: 0xd95050,
            green: 0x7eb563,
            yellow: 0xe0954a,
            blue: 0x6ba3be,
            magenta: 0xc07eb5,
            cyan: 0x6bbeb5,
            white: 0xe8d5c4,
            bright_black: 0x4a3828,
            bright_red: 0xf06666,
            bright_green: 0x98cc7d,
            bright_yellow: 0xf0aa66,
            bright_blue: 0x88bbd5,
            bright_magenta: 0xd898cc,
            bright_cyan: 0x88d5cc,
            bright_white: 0xf5ebe0,
        },
    })
}

fn theme_arctic() -> Theme {
    build_dark_theme(ColorScheme {
        background: BackgroundColors {
            main: 0x101820,
            title_bar: 0x18222e,
            search_box: 0x1e2c3a,
            log_panel: 0x18222e,
        },
        text: TextColors {
            primary: 0xdce8f0,
            secondary: 0x96b0c4,
            tertiary: 0x7494ae,
            muted: 0x607e96,
            dimmed: 0x304050,
            on_accent: 0x101820,
        },
        accent: AccentColors {
            selected: 0x5ccfe6,
            selected_subtle: 0x1a3a4a,
        },
        ui: UIColors {
            border: 0x304050,
            success: 0x6ad4a0,
            error: 0xe06060,
            warning: 0xe0b050,
            info: 0x5ccfe6,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0x101820,
            red: 0xe06060,
            green: 0x6ad4a0,
            yellow: 0xe0b050,
            blue: 0x5ccfe6,
            magenta: 0xc48aff,
            cyan: 0x5ccfe6,
            white: 0xdce8f0,
            bright_black: 0x304050,
            bright_red: 0xf07878,
            bright_green: 0x88e8b8,
            bright_yellow: 0xf0c868,
            bright_blue: 0x7ce0f0,
            bright_magenta: 0xd8a4ff,
            bright_cyan: 0x7ce0f0,
            bright_white: 0xf0f6fa,
        },
    })
}

fn theme_linen() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xf5efe6,
            title_bar: 0xeae2d6,
            search_box: 0xe0d8ca,
            log_panel: 0xeae2d6,
        },
        text: TextColors {
            primary: 0x3d3530,
            secondary: 0x6b5e52,
            tertiary: 0x8a7d70,
            muted: 0x887862,
            dimmed: 0xbfb5a5,
            on_accent: 0xf5efe6,
        },
        accent: AccentColors {
            selected: 0x8b6542,
            selected_subtle: 0xd8c8b0,
        },
        ui: UIColors {
            border: 0xd5cab8,
            success: 0x5a8a3a,
            error: 0xb54040,
            warning: 0xc08030,
            info: 0x4a7a9a,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0xf5efe6,
            red: 0xb54040,
            green: 0x5a8a3a,
            yellow: 0xc08030,
            blue: 0x4a6aaa,
            magenta: 0x8a5090,
            cyan: 0x4a8a8a,
            white: 0x3d3530,
            bright_black: 0xb5a898,
            bright_red: 0xd04a4a,
            bright_green: 0x6aa04a,
            bright_yellow: 0xd89840,
            bright_blue: 0x5580c0,
            bright_magenta: 0xa060a8,
            bright_cyan: 0x5aa0a0,
            bright_white: 0x2a2420,
        },
    })
}

fn theme_slate_morning() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xeef1f5,
            title_bar: 0xe2e6ec,
            search_box: 0xd8dce4,
            log_panel: 0xe2e6ec,
        },
        text: TextColors {
            primary: 0x2d333b,
            secondary: 0x556070,
            tertiary: 0x72808e,
            muted: 0x78838f,
            dimmed: 0xaab2bc,
            on_accent: 0xeef1f5,
        },
        accent: AccentColors {
            selected: 0x2a8a7a,
            selected_subtle: 0xb8d8d0,
        },
        ui: UIColors {
            border: 0xc8cdd5,
            success: 0x3a8a5a,
            error: 0xc04040,
            warning: 0xc09030,
            info: 0x2a8a7a,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0xeef1f5,
            red: 0xc04040,
            green: 0x3a8a5a,
            yellow: 0xc09030,
            blue: 0x3a6aaa,
            magenta: 0x7a5090,
            cyan: 0x2a8a7a,
            white: 0x2d333b,
            bright_black: 0xa8b0b8,
            bright_red: 0xd85050,
            bright_green: 0x4aa06a,
            bright_yellow: 0xd8a840,
            bright_blue: 0x5080c0,
            bright_magenta: 0x9060a8,
            bright_cyan: 0x40a090,
            bright_white: 0x1a2028,
        },
    })
}

fn theme_coral_reef() -> Theme {
    build_light_theme(ColorScheme {
        background: BackgroundColors {
            main: 0xfaf5f2,
            title_bar: 0xf0e8e4,
            search_box: 0xe8ddd8,
            log_panel: 0xf0e8e4,
        },
        text: TextColors {
            primary: 0x3a3232,
            secondary: 0x685858,
            tertiary: 0x887070,
            muted: 0x988888,
            dimmed: 0xccc0b8,
            on_accent: 0xfaf5f2,
        },
        accent: AccentColors {
            selected: 0xd06050,
            selected_subtle: 0xf0c8c0,
        },
        ui: UIColors {
            border: 0xd8ccc5,
            success: 0x4a9060,
            error: 0xd06050,
            warning: 0xc89040,
            info: 0x4888a8,
        },
        terminal: TerminalColors {
            foreground: None,
            background: None,
            black: 0xfaf5f2,
            red: 0xd06050,
            green: 0x4a9060,
            yellow: 0xc89040,
            blue: 0x4870a0,
            magenta: 0xa05888,
            cyan: 0x3a9090,
            white: 0x3a3232,
            bright_black: 0xb0a8a0,
            bright_red: 0xe87060,
            bright_green: 0x60a878,
            bright_yellow: 0xe0a850,
            bright_blue: 0x6088b8,
            bright_magenta: 0xb86aa0,
            bright_cyan: 0x50a8a8,
            bright_white: 0x2a2222,
        },
    })
}

// --- merged from part_04.rs ---
/// Write a theme to the user's theme.json file
pub fn write_theme_to_disk(theme: &Theme) -> Result<(), std::io::Error> {
    super::persistence::write_theme_to_disk(theme).map_err(std::io::Error::other)
}

pub(super) fn recover_theme_save() -> anyhow::Result<()> {
    super::persistence::recover_theme_save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_are_valid() {
        let presets = all_presets();
        assert!(presets.len() >= 10, "Should have at least 10 theme presets");

        for preset in &presets {
            let theme = preset.create_theme();
            // Verify the theme has valid colors (non-zero background)
            assert!(
                theme.colors.background.main != 0 || preset.id == "github-dark",
                "Theme '{}' has zero background color",
                preset.name
            );
            let expected_primary =
                super::super::helpers::hard_readable_text_hex(theme.colors.background.main);
            assert!(
                theme.colors.text.primary == expected_primary,
                "Theme '{}' should normalize primary text from its main background",
                preset.name
            );
        }
    }

    #[test]
    fn test_all_presets_default_surface_opacity_is_fifty_percent() {
        let presets = all_presets();

        for preset in &presets {
            let theme = preset.create_theme();
            let opacity = theme.get_opacity();

            let surface_opacities = [
                ("main", opacity.main),
                ("title_bar", opacity.title_bar),
                ("search_box", opacity.search_box),
                ("log_panel", opacity.log_panel),
                ("preview", opacity.preview),
                ("dialog", opacity.dialog),
                ("input", opacity.input),
                ("panel", opacity.panel),
                ("input_inactive", opacity.input_inactive),
                ("input_active", opacity.input_active),
            ];

            for (field, value) in surface_opacities {
                assert!(
                    (value - 0.50).abs() < f32::EPSILON,
                    "Preset '{}' should default opacity.{} to 50%, got {}",
                    preset.name,
                    field,
                    value
                );
            }

            // The window-root tint is intentionally higher than the surface
            // preset so legibility stays backdrop-independent.
            let vibrancy_background = opacity
                .vibrancy_background
                .unwrap_or(crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND);
            assert!(
                (vibrancy_background - crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND).abs()
                    < f32::EPSILON,
                "Preset '{}' should default opacity.vibrancy_background to the canonical token, got {}",
                preset.name,
                vibrancy_background
            );
        }
    }

    #[test]
    fn test_preset_ids_are_unique() {
        let presets = all_presets();
        let ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
        for (i, id) in ids.iter().enumerate() {
            for (j, other) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(id, other, "Duplicate preset ID: {}", id);
                }
            }
        }
    }

    #[test]
    fn test_dark_presets_have_dark_appearance() {
        for preset in all_presets() {
            if preset.is_dark {
                let theme = preset.create_theme();
                assert_eq!(
                    theme.appearance,
                    AppearanceMode::Dark,
                    "Dark preset '{}' should have Dark appearance mode",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn test_light_presets_have_light_appearance() {
        for preset in all_presets() {
            if !preset.is_dark {
                let theme = preset.create_theme();
                assert_eq!(
                    theme.appearance,
                    AppearanceMode::Light,
                    "Light preset '{}' should have Light appearance mode",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn test_theme_serialization() {
        for preset in all_presets() {
            let theme = preset.create_theme();
            let json = serde_json::to_string_pretty(&theme);
            assert!(
                json.is_ok(),
                "Theme '{}' should serialize to JSON",
                preset.name
            );
        }
    }

    #[test]
    fn test_presets_force_primary_text_to_hard_black_or_white() {
        for preset in all_presets() {
            let theme = preset.create_theme();
            let expected =
                super::super::helpers::hard_readable_text_hex(theme.colors.background.main);

            assert_eq!(
                theme.colors.text.primary, expected,
                "Theme '{}' should normalize primary text from its main background",
                preset.name
            );
            assert!(
                matches!(theme.colors.text.primary, 0x000000 | 0xFFFFFF),
                "Theme '{}' should use pure black or white for primary text",
                preset.name
            );
        }
    }

    #[test]
    fn test_presets_cached_matches_all_presets_order_and_metadata() {
        let all = all_presets();
        let cached = presets_cached();
        assert_eq!(cached.len(), all.len());

        for (cached_preset, all_preset) in cached.iter().zip(all.iter()) {
            assert_eq!(cached_preset.id, all_preset.id);
            assert_eq!(cached_preset.name, all_preset.name);
            assert_eq!(cached_preset.description, all_preset.description);
            assert_eq!(cached_preset.is_dark, all_preset.is_dark);
        }
    }

    #[test]
    fn test_find_current_preset_index_does_lookup_for_each_cached_preset() {
        for (index, preset) in presets_cached().iter().enumerate() {
            let theme = preset.create_theme();
            assert_eq!(find_current_preset_index(&theme), index);
        }
    }

    #[test]
    fn test_find_current_preset_index_returns_zero_when_theme_not_in_cache() {
        let mut theme = presets_cached()[0].create_theme();
        let missing_bg = u32::MAX;
        let missing_accent = 1;
        let missing_key = preset_bg_accent_key(missing_bg, missing_accent);
        assert!(!presets_cache()
            .preset_index_by_bg_accent
            .contains_key(&missing_key));

        theme.colors.background.main = missing_bg;
        theme.colors.accent.selected = missing_accent;

        assert_eq!(find_current_preset_index(&theme), 0);
    }

    #[test]
    fn test_first_light_theme_index_uses_cached_value() {
        let expected = presets_cached()
            .iter()
            .position(|p| !p.is_dark)
            .unwrap_or(0);
        assert_eq!(first_light_theme_index(), expected);
    }

    #[test]
    fn test_all_preset_preview_colors_matches_cached_preview_colors() {
        let all_preview_colors = all_preset_preview_colors();
        let cached_preview_colors = preset_preview_colors_cached();
        assert_eq!(all_preview_colors.len(), cached_preview_colors.len());

        for (all_colors, cached_colors) in
            all_preview_colors.iter().zip(cached_preview_colors.iter())
        {
            assert_eq!(all_colors.bg, cached_colors.bg);
            assert_eq!(all_colors.accent, cached_colors.accent);
            assert_eq!(all_colors.text, cached_colors.text);
            assert_eq!(all_colors.secondary, cached_colors.secondary);
            assert_eq!(all_colors.border, cached_colors.border);
        }
    }

    #[test]
    fn test_filtered_preset_indices_cached_rose_finds_rose_pine_family() {
        let rose_ids: Vec<&str> = filtered_preset_indices_cached("rose")
            .into_iter()
            .map(|idx| presets_cached()[idx].id)
            .collect();
        assert!(
            rose_ids.contains(&"rose-pine"),
            "Expected rose-pine in results: {:?}",
            rose_ids
        );
        assert!(
            rose_ids.contains(&"rose-pine-moon"),
            "Expected rose-pine-moon in results: {:?}",
            rose_ids
        );
        assert!(
            rose_ids.contains(&"rose-pine-dawn"),
            "Expected rose-pine-dawn in results: {:?}",
            rose_ids
        );
    }

    #[test]
    fn test_filtered_preset_indices_cached_frappe_finds_catppuccin_frappe() {
        let frappe_ids: Vec<&str> = filtered_preset_indices_cached("frappe")
            .into_iter()
            .map(|idx| presets_cached()[idx].id)
            .collect();
        assert!(
            frappe_ids.contains(&"catppuccin-frappe"),
            "Expected catppuccin-frappe in results: {:?}",
            frappe_ids
        );
    }

    #[test]
    fn test_preset_theme_cached_returns_same_arc_instance() {
        let a = preset_theme_cached(0);
        let b = preset_theme_cached(0);
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "Repeated preset_theme_cached calls should return the same Arc instance"
        );
    }

    #[test]
    fn test_filtered_preset_indices_cached_empty_returns_all() {
        let all = filtered_preset_indices_cached("");
        assert_eq!(all.len(), presets_cached().len());
    }

    #[test]
    fn test_normalize_preset_search_text_strips_accents_and_punctuation() {
        assert_eq!(normalize_preset_search_text("Rosé Pine"), "ros pine");
        assert_eq!(normalize_preset_search_text("Frappé"), "frapp");
        assert_eq!(normalize_preset_search_text("one-dark-pro"), "one dark pro");
    }

    // ── PresetMatchResult / classify_theme_preset_match tests ────────

    #[test]
    fn test_classify_exact_match_for_all_stock_presets() {
        for (index, preset) in presets_cached().iter().enumerate() {
            let theme = preset.create_theme();
            let result = classify_theme_preset_match(&theme);
            assert_eq!(
                result.kind,
                PresetMatchKind::ExactMatch,
                "Stock preset '{}' (index {}) should be ExactMatch, got {:?}",
                preset.id,
                index,
                result.kind
            );
            assert_eq!(result.preset_index, index);
            assert!(result.is_exact());
        }
    }

    #[test]
    fn test_classify_accent_only_change_is_modified() {
        let mut theme = presets_cached()[0].create_theme();
        // Change accent but keep background.main the same
        // (find a different accent that doesn't collide with another preset's key)
        let original_accent = theme.colors.accent.selected;
        theme.colors.accent.selected = if original_accent != 0xFF0000 {
            0xFF0000
        } else {
            0x00FF00
        };
        // Since accent changed, the bg+accent key won't match any preset → Custom
        let result = classify_theme_preset_match(&theme);
        assert_ne!(
            result.kind,
            PresetMatchKind::ExactMatch,
            "Accent-only change should not be ExactMatch"
        );
    }

    #[test]
    fn test_classify_opacity_only_change_is_modified() {
        let mut theme = presets_cached()[0].create_theme();
        // Modify opacity but keep bg+accent the same
        let mut opacity = theme.opacity.clone().unwrap_or_default();
        opacity.main = 0.99;
        theme.opacity = Some(opacity);
        let result = classify_theme_preset_match(&theme);
        assert_eq!(
            result.kind,
            PresetMatchKind::Modified,
            "Opacity-only change should be Modified (bg+accent still match preset)"
        );
        assert!(!result.is_exact());
    }

    #[test]
    fn test_classify_vibrancy_only_change_is_modified() {
        let mut theme = presets_cached()[0].create_theme();
        // Toggle vibrancy enabled
        if let Some(ref mut v) = theme.vibrancy {
            v.enabled = !v.enabled;
        }
        let result = classify_theme_preset_match(&theme);
        assert_eq!(
            result.kind,
            PresetMatchKind::Modified,
            "Vibrancy-only change should be Modified"
        );
    }

    #[test]
    fn test_classify_fully_custom_theme_is_custom() {
        let mut theme = presets_cached()[0].create_theme();
        // Use bg+accent that no preset has
        theme.colors.background.main = 0x010101;
        theme.colors.accent.selected = 0x020202;
        let result = classify_theme_preset_match(&theme);
        assert_eq!(
            result.kind,
            PresetMatchKind::Custom,
            "Fully custom theme should be Custom"
        );
    }

    #[test]
    fn test_classify_preserves_preset_index_for_modified() {
        use crate::theme::VibrancyMaterial;
        let preset = &presets_cached()[2]; // pick a non-zero preset
        let mut theme = preset.create_theme();
        // Modify vibrancy material to make it non-exact
        if let Some(ref mut v) = theme.vibrancy {
            v.material = match v.material {
                VibrancyMaterial::Popover => VibrancyMaterial::Hud,
                _ => VibrancyMaterial::Popover,
            };
        }
        let result = classify_theme_preset_match(&theme);
        assert_eq!(result.preset_index, 2);
        assert_eq!(result.kind, PresetMatchKind::Modified);
    }
}

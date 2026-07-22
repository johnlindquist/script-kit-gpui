//! Design contract exporter.
//!
//! Serializes the **resolved** main-menu visual contract — the same values
//! production rendering computes — to `design/mockups/generated/tokens.json`
//! and `tokens.css` so HTML mockups consume Rust-derived values instead of
//! hand-transcribed ones. Rust is the single authority: mockups may only
//! style through the generated `--sk-*` custom properties, and proposed
//! design changes round-trip back through the Rust token layer.
//!
//! Three token stages keep the contract honest:
//! - `source`: authored Rust leaves (e.g. `selected_fill_alpha: 0x20`).
//! - `resolved`: values after opacity packing, `max()` floors, row-kind and
//!   theme logic — what the renderer actually paints. Never hand-edited.
//! - `emulator`: browser-only calibration (blur radii etc.). These live in
//!   the mockup CSS, are prefixed `--sk-emulator-`, and never map to Rust.
//!
//! Checked-in artifacts use `base_def()` and the stock `script-kit-dark`
//! preset so they do not depend on local runtime overrides or user themes.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::designs::{MainMenuThemeDef, MainMenuThemeVariant};
use crate::list_item::{resolved_main_menu_row_fill, ListItemMetricsOverride, MainMenuRowFillBase};
use crate::theme::{AppChromeColors, Theme};

pub const TOKENS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTokenBundle {
    pub schema_version: u32,
    pub profile: ExportProfileRecord,
    /// SHA-256 over the serialized `tokens` map — ties edits.json proposals
    /// and published mockups to the exact contract they were built against.
    pub bundle_hash: String,
    pub tokens: BTreeMap<String, TokenRecord>,
    /// Places where two live code paths disagree about the same visual value.
    /// Recorded, never silently collapsed.
    pub conflicts: Vec<ContractConflict>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfileRecord {
    pub theme_id: String,
    pub appearance: String,
    pub main_menu_variant: String,
    /// Which actions-popup theme definition the bundle reads ("base").
    pub actions_popup_theme: String,
    /// Action rows inherit chrome from this main-menu variant.
    pub actions_row_main_menu_variant: String,
    /// Which `DesignVariant` spacing/typography tokens the exporter resolves
    /// with (the renderer reads `self.current_design`; checked-in artifacts
    /// pin `Default`).
    pub design_variant: String,
    pub runtime_overrides: String,
    pub background_effect: String,
    pub background_effect_intensity: f32,
    pub scale_factor: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRecord {
    pub stage: TokenStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css_var: Option<String>,
    pub value: TokenValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_path: Option<String>,
    pub writable: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub derived_from: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenStage {
    Source,
    Resolved,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TokenValue {
    /// Logical px (GPUI points).
    Length {
        value: f64,
    },
    Color {
        rgba8: String,
        css: String,
    },
    Number {
        value: f64,
    },
    FontWeight {
        value: f64,
    },
    DurationMs {
        value: u64,
    },
    Text {
        value: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractConflict {
    pub id: String,
    pub values: BTreeMap<String, String>,
    pub severity: String,
    pub explanation: String,
}

/// Format a `0xRRGGBBAA`-packed color as the exact CSS the renderer's bytes
/// imply. Alpha keeps full precision (e.g. `0xA5` → `0.6470588235`, not the
/// authored `0.65`) so the mockup rounds the same way GPUI does.
fn color_value(packed_rgba: u32) -> TokenValue {
    let r = (packed_rgba >> 24) & 0xFF;
    let g = (packed_rgba >> 16) & 0xFF;
    let b = (packed_rgba >> 8) & 0xFF;
    let a = packed_rgba & 0xFF;
    TokenValue::Color {
        rgba8: format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
        css: if a == 0xFF {
            format!("rgb({r} {g} {b})")
        } else {
            format!("rgb({r} {g} {b} / {:.10})", a as f64 / 255.0)
        },
    }
}

fn hex_color_value(hex_rgb: u32) -> TokenValue {
    color_value((hex_rgb << 8) | 0xFF)
}

/// Convert an `Hsla` (as handed to the shader) to a packed RGBA color token.
fn hsla_color_value(color: gpui::Hsla) -> TokenValue {
    let rgba: gpui::Rgba = color.into();
    let to_byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    color_value(
        (to_byte(rgba.r) << 24)
            | (to_byte(rgba.g) << 16)
            | (to_byte(rgba.b) << 8)
            | to_byte(rgba.a),
    )
}

struct BundleBuilder {
    tokens: BTreeMap<String, TokenRecord>,
    conflicts: Vec<ContractConflict>,
}

impl BundleBuilder {
    fn new() -> Self {
        Self {
            tokens: BTreeMap::new(),
            conflicts: Vec::new(),
        }
    }

    fn add(
        &mut self,
        id: &str,
        stage: TokenStage,
        css_var: Option<&str>,
        value: TokenValue,
        rust_path: Option<&str>,
        writable: bool,
        derived_from: &[&str],
    ) {
        let record = TokenRecord {
            stage,
            css_var: css_var.map(str::to_string),
            value,
            rust_path: rust_path.map(str::to_string),
            writable,
            derived_from: derived_from.iter().map(|s| s.to_string()).collect(),
        };
        let previous = self.tokens.insert(id.to_string(), record);
        debug_assert!(previous.is_none(), "duplicate design token id: {id}");
    }

    fn source_len(&mut self, id: &str, var: &str, value: f32, rust_path: &str) {
        self.add(
            id,
            TokenStage::Source,
            Some(var),
            TokenValue::Length {
                value: value as f64,
            },
            Some(rust_path),
            true,
            &[],
        );
    }

    fn resolved_color(&mut self, id: &str, var: &str, packed: u32, derived_from: &[&str]) {
        self.add(
            id,
            TokenStage::Resolved,
            Some(var),
            color_value(packed),
            None,
            false,
            derived_from,
        );
    }

    fn conflict(&mut self, id: &str, values: &[(&str, String)], severity: &str, explanation: &str) {
        self.conflicts.push(ContractConflict {
            id: id.to_string(),
            values: values
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            severity: severity.to_string(),
            explanation: explanation.to_string(),
        });
    }
}

/// Build the checked-in baseline bundle: stock `script-kit-dark` theme,
/// `InfoBarBase` main-menu variant via `base_def()` (no runtime overrides),
/// base actions-popup theme, default Starfield effect at the fresh-install
/// intensity. Covers every screen the mockup contract has reached so far
/// (main menu, actions dialog).
pub fn checked_in_design_bundle() -> Result<DesignTokenBundle, String> {
    let theme: Theme = crate::theme::presets::all_presets()
        .into_iter()
        .find(|preset| preset.id == "script-kit-dark")
        .ok_or_else(|| "missing required script-kit-dark preset".to_string())?
        .create_theme();

    let variant = MainMenuThemeVariant::InfoBarBase;
    // Checked-in artifacts always read the base definition.
    let def: MainMenuThemeDef = variant.base_def();
    let opacity = theme.get_opacity();
    let chrome = AppChromeColors::from_theme(&theme);
    let metrics = ListItemMetricsOverride::from_main_menu_def(def);
    let fill = resolved_main_menu_row_fill(def.row_kind, &metrics, opacity.hover);

    let mut b = BundleBuilder::new();

    // ── Window / shell ──────────────────────────────────────────────────
    b.source_len(
        "window.width",
        "--sk-window-main-width",
        crate::window_resize::MAIN_WINDOW_WIDTH,
        "crate::window_resize::MAIN_WINDOW_WIDTH",
    );
    b.source_len(
        "window.height",
        "--sk-window-main-height",
        crate::window_resize::main_window_full_height(),
        "crate::window_resize::main_window_full_height",
    );
    b.source_len(
        "window.radius",
        "--sk-window-radius",
        crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX,
        "crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX",
    );
    b.source_len(
        "window.nativeFooterHostHeight",
        "--sk-window-native-footer-host-height",
        crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT,
        "crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT",
    );
    b.source_len(
        "window.dividerHeight",
        "--sk-window-divider-height",
        crate::panel::HEADER_DIVIDER_HEIGHT,
        "crate::panel::HEADER_DIVIDER_HEIGHT",
    );

    // ── Vibrancy ────────────────────────────────────────────────────────
    let vibrancy = theme.vibrancy.clone().unwrap_or_default();
    b.add(
        "vibrancy.material",
        TokenStage::Source,
        Some("--sk-vibrancy-material"),
        TokenValue::Text {
            value: format!("{:?}", vibrancy.material).to_lowercase(),
        },
        Some("crate::theme::VibrancySettings::material"),
        true,
        &[],
    );
    b.add(
        "vibrancy.backdropSaturation",
        TokenStage::Source,
        Some("--sk-vibrancy-backdrop-saturation"),
        TokenValue::Number {
            value: vibrancy.backdrop_saturation as f64,
        },
        Some("crate::theme::VibrancySettings::backdrop_saturation"),
        true,
        &[],
    );
    let vibrancy_tint_opacity = opacity
        .vibrancy_background
        .unwrap_or(crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND)
        .clamp(0.0, 1.0);
    b.add(
        "resolved.window.vibrancyTint",
        TokenStage::Resolved,
        Some("--sk-window-vibrancy-tint"),
        color_value(crate::ui_foundation::hex_to_rgba_with_opacity(
            theme.colors.background.main,
            vibrancy_tint_opacity,
        )),
        None,
        false,
        &[
            "theme.colors.background.main",
            "theme.opacity.vibrancyBackground",
        ],
    );

    // ── Base palette (authored hexes) ───────────────────────────────────
    let colors = &theme.colors;
    for (id, var, hex, path) in [
        (
            "theme.colors.background.main",
            "--sk-color-background-main",
            colors.background.main,
            "Theme.colors.background.main",
        ),
        (
            "theme.colors.text.primary",
            "--sk-color-text-primary",
            colors.text.primary,
            "Theme.colors.text.primary",
        ),
        (
            "theme.colors.text.onAccent",
            "--sk-color-text-on-accent",
            colors.text.on_accent,
            "Theme.colors.text.on_accent",
        ),
        (
            "theme.colors.accent.selected",
            "--sk-color-accent",
            colors.accent.selected,
            "Theme.colors.accent.selected",
        ),
        (
            "theme.colors.accent.selectedSubtle",
            "--sk-color-accent-subtle",
            colors.accent.selected_subtle,
            "Theme.colors.accent.selected_subtle",
        ),
        (
            "theme.colors.ui.border",
            "--sk-color-border",
            colors.ui.border,
            "Theme.colors.ui.border",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            Some(var),
            hex_color_value(hex),
            Some(path),
            true,
            &[],
        );
    }

    // ── Resolved chrome (byte-quantized ladder) ─────────────────────────
    b.resolved_color(
        "resolved.chrome.textName",
        "--sk-text-name",
        (colors.text.primary << 8) | 0xFF,
        &["theme.colors.text.primary"],
    );
    b.resolved_color(
        "resolved.chrome.textStrong",
        "--sk-text-strong",
        chrome.text_strong_rgba,
        &["theme.colors.text.primary", "theme.opacity.textStrong"],
    );
    b.resolved_color(
        "resolved.chrome.textMuted",
        "--sk-text-muted",
        chrome.text_muted_rgba,
        &["theme.colors.text.primary", "theme.opacity.textMuted"],
    );
    b.resolved_color(
        "resolved.chrome.textHint",
        "--sk-text-hint",
        chrome.text_hint_rgba,
        &["theme.colors.text.primary", "theme.opacity.textHint"],
    );
    b.resolved_color(
        "resolved.chrome.textPlaceholder",
        "--sk-text-placeholder",
        chrome.placeholder_text_rgba,
        &["theme.colors.text.primary", "theme.opacity.textPlaceholder"],
    );
    b.resolved_color(
        "resolved.chrome.textIcon",
        "--sk-text-icon",
        chrome.text_icon_rgba,
        &["theme.colors.text.primary", "theme.opacity.textIcon"],
    );
    b.resolved_color(
        "resolved.chrome.selection",
        "--sk-theme-selection-background",
        chrome.selection_rgba,
        &["theme.colors.text.primary", "theme.opacity.selected"],
    );
    b.resolved_color(
        "resolved.chrome.hover",
        "--sk-theme-hover-background",
        chrome.hover_rgba,
        &["theme.colors.text.primary", "theme.opacity.hover"],
    );
    b.resolved_color(
        "resolved.chrome.divider",
        "--sk-chrome-divider",
        chrome.divider_rgba,
        &["theme.colors.ui.border", "theme.opacity.borderInactive"],
    );
    b.resolved_color(
        "resolved.chrome.border",
        "--sk-chrome-border",
        chrome.border_rgba,
        &["theme.colors.ui.border", "theme.opacity.borderActive"],
    );
    b.resolved_color(
        "resolved.chrome.windowSurface",
        "--sk-chrome-window-surface",
        chrome.window_surface_rgba,
        &["theme.colors.background.main", "theme.opacity.main"],
    );

    // ── Header context zone (info bar) ──────────────────────────────────
    let info = def.header_info_bar;
    b.source_len(
        "mainMenu.shell.headerPaddingX",
        "--sk-main-menu-header-padding-x",
        def.shell.header_padding_x,
        "MainMenuShellTokens.header_padding_x",
    );
    b.source_len(
        "mainMenu.shell.headerPaddingY",
        "--sk-main-menu-header-padding-y",
        def.shell.header_padding_y,
        "MainMenuShellTokens.header_padding_y",
    );
    b.source_len(
        "mainMenu.shell.headerGap",
        "--sk-main-menu-header-gap",
        def.shell.header_gap,
        "MainMenuShellTokens.header_gap",
    );
    b.source_len(
        "mainMenu.shell.contentInsetX",
        "--sk-main-menu-content-inset-x",
        def.shell.content_inset_x,
        "MainMenuShellTokens.content_inset_x",
    );
    b.add(
        "mainMenu.context.fontFamily",
        TokenStage::Source,
        Some("--sk-main-menu-context-font-family"),
        TokenValue::Text {
            value: info.font_family.to_string(),
        },
        Some("HeaderInfoBarTokens.font_family"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.context.fontSize",
        "--sk-main-menu-context-font-size",
        info.font_size,
        "HeaderInfoBarTokens.font_size",
    );
    b.add(
        "mainMenu.context.opacity",
        TokenStage::Source,
        Some("--sk-main-menu-context-opacity"),
        TokenValue::Number {
            value: info.opacity as f64,
        },
        Some("HeaderInfoBarTokens.opacity"),
        true,
        &[],
    );
    b.add(
        "mainMenu.context.keyOpacity",
        TokenStage::Source,
        Some("--sk-main-menu-context-key-opacity"),
        TokenValue::Number {
            value: info.key_opacity as f64,
        },
        Some("HeaderInfoBarTokens.key_opacity"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.context.height",
        "--sk-main-menu-context-height",
        info.height_px,
        "HeaderInfoBarTokens.height_px",
    );
    b.source_len(
        "mainMenu.context.gap",
        "--sk-main-menu-context-gap",
        info.gap_px,
        "HeaderInfoBarTokens.gap_px",
    );
    b.source_len(
        "mainMenu.context.pillPaddingX",
        "--sk-main-menu-context-pill-padding-x",
        info.pill_padding_x,
        "HeaderInfoBarTokens.pill_padding_x",
    );
    b.source_len(
        "mainMenu.context.pillRadius",
        "--sk-main-menu-context-pill-radius",
        info.pill_radius,
        "HeaderInfoBarTokens.pill_radius",
    );
    b.source_len(
        "mainMenu.context.edgeOutsetX",
        "--sk-main-menu-context-edge-outset-x",
        info.context_edge_outset_x,
        "HeaderInfoBarTokens.context_edge_outset_x",
    );
    b.add(
        "resolved.mainMenu.context.keycapFontSize",
        TokenStage::Resolved,
        Some("--sk-main-menu-context-keycap-font-size"),
        TokenValue::Length {
            value: crate::components::main_view_chrome::context_zone_keycap_font_size(&info) as f64,
        },
        None,
        false,
        &["mainMenu.context.fontSize"],
    );
    b.add(
        "resolved.mainMenu.context.keycapHeight",
        TokenStage::Resolved,
        Some("--sk-main-menu-context-keycap-height"),
        TokenValue::Length {
            value: crate::components::main_view_chrome::context_zone_keycap_height(&info) as f64,
        },
        None,
        false,
        &["mainMenu.context.fontSize"],
    );
    b.add(
        "mainMenu.context.separator",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: info.separator.to_string(),
        },
        Some("HeaderInfoBarTokens.separator"),
        true,
        &[],
    );

    // ── Search input ────────────────────────────────────────────────────
    b.source_len(
        "mainMenu.search.height",
        "--sk-main-menu-search-height",
        def.search.height,
        "MainMenuSearchTokens.height",
    );
    b.source_len(
        "mainMenu.search.textInsetX",
        "--sk-main-menu-search-text-inset-x",
        def.search.text_inset_x,
        "MainMenuSearchTokens.text_inset_x",
    );
    b.source_len(
        "mainMenu.search.fontSize",
        "--sk-main-menu-search-font-size",
        def.search.font_size,
        "MainMenuSearchTokens.font_size",
    );
    b.add(
        "mainMenu.search.fontWeight",
        TokenStage::Source,
        Some("--sk-main-menu-search-font-weight"),
        TokenValue::FontWeight {
            value: def.search.font_weight.0 as f64,
        },
        Some("MainMenuSearchTokens.font_weight"),
        true,
        &[],
    );
    b.add(
        "mainMenu.search.placeholder",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: crate::ROOT_LAUNCHER_PLACEHOLDER.to_string(),
        },
        Some("crate::ROOT_LAUNCHER_PLACEHOLDER"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.caret.width",
        "--sk-caret-width",
        crate::panel::CURSOR_WIDTH,
        "crate::panel::CURSOR_WIDTH",
    );
    b.source_len(
        "mainMenu.caret.height",
        "--sk-caret-height",
        crate::panel::CURSOR_HEIGHT_LG,
        "crate::panel::CURSOR_HEIGHT_LG",
    );

    // ── List / sections ─────────────────────────────────────────────────
    b.source_len(
        "mainMenu.list.rowHeight",
        "--sk-main-menu-row-height",
        metrics.item_height,
        "MainMenuListTokens.item_height",
    );
    b.source_len(
        "mainMenu.list.sectionSlotHeight",
        "--sk-main-menu-section-slot-height",
        metrics.section_header_height,
        "MainMenuListTokens.section_header_height",
    );
    b.source_len(
        "mainMenu.list.firstSectionSlotHeight",
        "--sk-main-menu-first-section-slot-height",
        metrics.first_section_header_height,
        "MainMenuListTokens.first_section_header_height",
    );
    b.source_len(
        "mainMenu.section.paddingX",
        "--sk-main-menu-section-padding-x",
        metrics.section_padding_x,
        "MainMenuListTokens.section_padding_x",
    );
    b.source_len(
        "mainMenu.section.paddingTop",
        "--sk-main-menu-section-padding-top",
        metrics.section_padding_top,
        "MainMenuListTokens.section_padding_top",
    );
    b.source_len(
        "mainMenu.section.firstPaddingTop",
        "--sk-main-menu-first-section-padding-top",
        metrics.first_section_padding_top,
        "ListItemMetricsOverride.first_section_padding_top",
    );
    b.source_len(
        "mainMenu.section.paddingBottom",
        "--sk-main-menu-section-padding-bottom",
        metrics.section_padding_bottom,
        "MainMenuListTokens.section_padding_bottom",
    );
    b.source_len(
        "mainMenu.section.gap",
        "--sk-main-menu-section-gap",
        metrics.section_gap,
        "MainMenuListTokens.section_gap",
    );
    b.source_len(
        "mainMenu.section.iconSize",
        "--sk-main-menu-section-icon-size",
        metrics.section_icon_size,
        "MainMenuListTokens.section_icon_size",
    );
    b.source_len(
        "mainMenu.section.fontSize",
        "--sk-main-menu-section-font-size",
        metrics.section_header_font_size,
        "MainMenuTypographyTokens.section_font_size",
    );
    b.add(
        "mainMenu.section.fontWeight",
        TokenStage::Source,
        Some("--sk-main-menu-section-font-weight"),
        TokenValue::FontWeight {
            value: metrics.section_weight.0 as f64,
        },
        Some("MainMenuTypographyTokens.section_weight"),
        true,
        &[],
    );

    // ── Row geometry + resolved fills ───────────────────────────────────
    b.source_len(
        "mainMenu.row.outerPaddingX",
        "--sk-main-menu-row-outer-padding-x",
        metrics.row_outer_padding_x,
        "MainMenuRowTokens.outer_padding_x",
    );
    b.source_len(
        "mainMenu.row.outerPaddingY",
        "--sk-main-menu-row-outer-padding-y",
        metrics.row_outer_padding_y,
        "MainMenuRowTokens.outer_padding_y",
    );
    b.source_len(
        "mainMenu.row.innerPaddingX",
        "--sk-main-menu-row-inner-padding-x",
        metrics.row_inner_padding_x,
        "MainMenuRowTokens.inner_padding_x",
    );
    b.source_len(
        "mainMenu.row.innerPaddingY",
        "--sk-main-menu-row-inner-padding-y",
        metrics.row_inner_padding_y,
        "MainMenuRowTokens.inner_padding_y",
    );
    b.source_len(
        "mainMenu.row.radius",
        "--sk-main-menu-row-radius",
        metrics.row_radius,
        "MainMenuRowTokens.radius",
    );
    b.source_len(
        "mainMenu.row.iconTextGap",
        "--sk-main-menu-row-icon-text-gap",
        metrics.icon_text_gap,
        "MainMenuRowTokens.icon_text_gap",
    );
    b.source_len(
        "mainMenu.row.nameDescGap",
        "--sk-main-menu-row-name-description-gap",
        metrics.name_desc_gap,
        "MainMenuRowTokens.name_desc_gap",
    );
    b.source_len(
        "mainMenu.row.accessoryGap",
        "--sk-main-menu-row-accessory-gap",
        metrics.accessory_gap,
        "MainMenuRowTokens.accessory_gap",
    );

    let selected_fill = match fill.base {
        MainMenuRowFillBase::TextPrimary => (colors.text.primary << 8) | fill.selected_alpha as u32,
        MainMenuRowFillBase::Accent => (colors.accent.selected << 8) | fill.selected_alpha as u32,
    };
    let hover_fill = match fill.base {
        MainMenuRowFillBase::TextPrimary => (colors.text.primary << 8) | fill.hover_alpha as u32,
        MainMenuRowFillBase::Accent => (colors.accent.selected << 8) | fill.hover_alpha as u32,
    };
    b.resolved_color(
        "resolved.mainMenu.row.selectedBackground",
        "--sk-main-menu-row-selected-background",
        selected_fill,
        &[
            "theme.colors.text.primary",
            "mainMenu.row.selectedFillAlpha",
        ],
    );
    b.resolved_color(
        "resolved.mainMenu.row.hoverBackground",
        "--sk-main-menu-row-hover-background",
        hover_fill,
        &[
            "theme.colors.text.primary",
            "theme.opacity.hover",
            "mainMenu.row.hoverFillAlpha",
        ],
    );

    // ── Icon tile ───────────────────────────────────────────────────────
    b.source_len(
        "mainMenu.icon.containerSize",
        "--sk-main-menu-icon-container-size",
        metrics.icon_container_size,
        "MainMenuIconTokens.container_size",
    );
    b.source_len(
        "mainMenu.icon.svgSize",
        "--sk-main-menu-icon-svg-size",
        metrics.icon_svg_size,
        "MainMenuIconTokens.svg_size",
    );
    b.source_len(
        "mainMenu.icon.tileSize",
        "--sk-main-menu-icon-tile-size",
        metrics.icon_tile_size,
        "MainMenuIconTokens.tile_size",
    );
    b.add(
        "resolved.mainMenu.icon.tileRadius",
        TokenStage::Resolved,
        Some("--sk-main-menu-icon-tile-radius"),
        TokenValue::Length {
            value: fill.icon_tile_radius as f64,
        },
        None,
        false,
        &["MainMenuIconTokens.tile_radius"],
    );
    b.resolved_color(
        "resolved.mainMenu.icon.tileBackground",
        "--sk-main-menu-icon-tile-background",
        (colors.accent.selected << 8) | fill.icon_tile_alpha,
        &[
            "theme.colors.accent.selected",
            "MainMenuIconTokens.tile_fill_alpha",
        ],
    );

    // ── Typography ──────────────────────────────────────────────────────
    b.source_len(
        "mainMenu.type.nameFontSize",
        "--sk-main-menu-name-font-size",
        metrics.name_font_size,
        "MainMenuTypographyTokens.name_font_size",
    );
    b.source_len(
        "mainMenu.type.nameLineHeight",
        "--sk-main-menu-name-line-height",
        metrics.name_line_height,
        "MainMenuTypographyTokens.name_line_height",
    );
    b.add(
        "mainMenu.type.nameWeight",
        TokenStage::Source,
        Some("--sk-main-menu-name-font-weight"),
        TokenValue::FontWeight {
            value: metrics.name_weight.0 as f64,
        },
        Some("MainMenuTypographyTokens.name_weight"),
        true,
        &[],
    );
    b.add(
        "mainMenu.type.selectedNameWeight",
        TokenStage::Source,
        Some("--sk-main-menu-selected-name-font-weight"),
        TokenValue::FontWeight {
            value: metrics.selected_name_weight.0 as f64,
        },
        Some("MainMenuTypographyTokens.selected_name_weight"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.type.descFontSize",
        "--sk-main-menu-description-font-size",
        metrics.desc_font_size,
        "MainMenuTypographyTokens.desc_font_size",
    );
    b.source_len(
        "mainMenu.type.descLineHeight",
        "--sk-main-menu-description-line-height",
        metrics.desc_line_height,
        "MainMenuTypographyTokens.desc_line_height",
    );
    b.add(
        "mainMenu.type.uiFontFamily",
        TokenStage::Source,
        Some("--sk-font-ui"),
        TokenValue::Text {
            value: crate::list_item::FONT_SYSTEM_UI.to_string(),
        },
        Some("crate::list_item::FONT_SYSTEM_UI"),
        true,
        &[],
    );
    b.add(
        "mainMenu.type.monoFontFamily",
        TokenStage::Source,
        Some("--sk-font-mono"),
        TokenValue::Text {
            value: crate::list_item::FONT_MONO.to_string(),
        },
        Some("crate::list_item::FONT_MONO"),
        true,
        &[],
    );

    // ── Footer (def-driven rail inside the native host) ────────────────
    let fm = def.footer.metrics;
    b.source_len(
        "footer.railHeight",
        "--sk-footer-rail-height",
        fm.height_px,
        "FooterMetricsTokens.height_px",
    );
    b.source_len(
        "footer.sideInset",
        "--sk-footer-side-inset",
        fm.side_inset_px,
        "FooterMetricsTokens.side_inset_px",
    );
    b.source_len(
        "footer.itemGap",
        "--sk-footer-item-gap",
        fm.item_gap_px,
        "FooterMetricsTokens.item_gap_px",
    );
    b.source_len(
        "footer.contentGap",
        "--sk-footer-content-gap",
        fm.content_gap,
        "FooterMetricsTokens.content_gap",
    );
    b.source_len(
        "footer.buttonPaddingX",
        "--sk-footer-button-padding-x",
        fm.button_padding_x,
        "FooterMetricsTokens.button_padding_x",
    );
    b.source_len(
        "footer.buttonPaddingY",
        "--sk-footer-button-padding-y",
        fm.button_padding_y,
        "FooterMetricsTokens.button_padding_y",
    );
    b.source_len(
        "footer.runButtonPaddingX",
        "--sk-footer-run-padding-x",
        fm.run_button_padding_x,
        "FooterMetricsTokens.run_button_padding_x",
    );
    b.source_len(
        "footer.buttonRadius",
        "--sk-footer-button-radius",
        fm.button_radius,
        "FooterMetricsTokens.button_radius",
    );
    b.source_len(
        "footer.labelFontSize",
        "--sk-footer-label-font-size",
        fm.label_font_size,
        "FooterMetricsTokens.label_font_size",
    );
    b.source_len(
        "footer.keycapHeight",
        "--sk-footer-keycap-height",
        fm.keycap_height,
        "FooterMetricsTokens.keycap_height",
    );
    b.source_len(
        "footer.keycapRadius",
        "--sk-footer-keycap-radius",
        fm.keycap_radius,
        "FooterMetricsTokens.keycap_radius",
    );
    b.source_len(
        "footer.keycapFontSize",
        "--sk-footer-keycap-font-size",
        fm.keycap_font_size,
        "FooterMetricsTokens.keycap_font_size",
    );
    b.source_len(
        "footer.runSlotMinWidth",
        "--sk-footer-run-min-width",
        fm.run_slot_min_width,
        "FooterMetricsTokens.run_slot_min_width",
    );
    b.source_len(
        "footer.runSlotMaxWidth",
        "--sk-footer-run-max-width",
        fm.run_slot_max_width,
        "FooterMetricsTokens.run_slot_max_width",
    );
    b.source_len(
        "footer.actionsSlotWidth",
        "--sk-footer-actions-width",
        fm.actions_slot_width,
        "FooterMetricsTokens.actions_slot_width",
    );
    b.source_len(
        "footer.aiSlotWidth",
        "--sk-footer-agent-width",
        fm.ai_slot_width,
        "FooterMetricsTokens.ai_slot_width",
    );
    // NOTE: keycaps render min_w(keycap_height) + px(keycap_padding_x) — the
    // def's padding (0) is authoritative for GPUI footers, not the AppKit
    // FOOTER_KEYCAP_PADDING_X_PX const.
    b.source_len(
        "footer.keycapPaddingX",
        "--sk-footer-keycap-padding-x",
        fm.keycap_padding_x,
        "FooterMetricsTokens.keycap_padding_x",
    );
    // Universal footer buttons hug content: height = host - 2*padding_y,
    // centered edge padding = button_padding_x + trailing_extra/2. Slot
    // widths are max bounds only (never force min-width in mockups).
    b.add(
        "resolved.footer.buttonHeight",
        TokenStage::Resolved,
        Some("--sk-footer-button-height"),
        TokenValue::Length {
            value: (crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT
                - 2.0 * fm.button_padding_y) as f64,
        },
        None,
        false,
        &["window.nativeFooterHostHeight", "footer.buttonPaddingY"],
    );
    b.add(
        "resolved.footer.centeredEdgePaddingX",
        TokenStage::Resolved,
        Some("--sk-footer-centered-edge-padding-x"),
        TokenValue::Length {
            value: (fm.button_padding_x
                + crate::components::footer_chrome::FOOTER_TRAILING_ACTION_EXTRA_PADDING_X_PX / 2.0)
                as f64,
        },
        None,
        false,
        &["footer.buttonPaddingX"],
    );
    let button = def.footer.button;
    b.resolved_color(
        "resolved.footer.buttonBorder",
        "--sk-footer-keycap-border",
        (colors.text.primary << 8) | button.border_alpha,
        &[
            "theme.colors.text.primary",
            "FooterButtonTheme.border_alpha",
        ],
    );
    b.resolved_color(
        "resolved.footer.buttonHover",
        "--sk-footer-button-hover",
        (colors.text.primary << 8) | button.hover,
        &["theme.colors.text.primary", "FooterButtonTheme.hover"],
    );
    b.resolved_color(
        "resolved.footer.buttonActive",
        "--sk-footer-button-active",
        (colors.text.primary << 8) | button.active,
        &["theme.colors.text.primary", "FooterButtonTheme.active"],
    );
    b.resolved_color(
        "resolved.footer.divider",
        "--sk-footer-divider",
        (colors.ui.border << 8) | def.footer.divider_alpha,
        &["theme.colors.ui.border", "FooterTheme.divider_alpha"],
    );
    b.resolved_color(
        "resolved.footer.text",
        "--sk-footer-text",
        chrome.text_strong_rgba,
        &["theme.colors.text.primary", "theme.opacity.textStrong"],
    );

    // ── Background effect (Starfield palette) ───────────────────────────
    let effect = crate::effects::DEFAULT_BACKGROUND_EFFECT;
    let intensity = crate::config::EffectsPreferences::DEFAULT_INTENSITY;
    let (color_a, color_b) = crate::effects::background_effect_palette(&theme, effect, intensity);
    b.add(
        "effect.slug",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: effect.slug().to_string(),
        },
        Some("crate::effects::DEFAULT_BACKGROUND_EFFECT"),
        true,
        &[],
    );
    b.add(
        "effect.shaderId",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: effect.shader_id() as f64,
        },
        Some("BackgroundEffect::shader_id"),
        false,
        &[],
    );
    b.add(
        "resolved.effect.colorA",
        TokenStage::Resolved,
        Some("--sk-starfield-color-a"),
        hsla_color_value(color_a),
        None,
        false,
        &["theme.colors.accent.selected", "effect.slug"],
    );
    b.add(
        "resolved.effect.colorB",
        TokenStage::Resolved,
        Some("--sk-starfield-color-b"),
        hsla_color_value(color_b),
        None,
        false,
        &["theme.colors.accent.selected", "effect.slug"],
    );

    // ── Actions dialog (Cmd+K popup) ────────────────────────────────────
    // Base definition only — checked-in artifacts always match the base
    // token definition from `base_actions_popup_theme`.
    let popup = crate::designs::base_actions_popup_theme();
    let default_spacing =
        crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing();
    let row_chrome = crate::actions::resolved_actions_dialog_row_chrome(&popup, def, &theme);
    let search_chrome =
        crate::actions::resolved_actions_dialog_search_chrome(&popup, &default_spacing, &theme);
    let section_chrome = crate::actions::resolved_actions_dialog_section_chrome(&popup, &theme);
    // The reference fixture: 5 actions in 3 header sections, search shown,
    // no context header, footerless by contract.
    let actions_fixture_height = crate::actions::resolved_actions_popup_height(
        &popup,
        5,
        3,
        false,
        false,
        false,
        popup.shell.max_height,
        popup.list.row_height,
    );

    b.source_len(
        "actionsDialog.shell.width",
        "--sk-actions-dialog-width",
        popup.shell.width,
        "ActionsPopupShellTokens.width (crate::actions::constants::POPUP_WIDTH)",
    );
    b.source_len(
        "actionsDialog.shell.maxHeight",
        "--sk-actions-dialog-max-height",
        popup.shell.max_height,
        "ActionsPopupShellTokens.max_height (POPUP_MAX_HEIGHT)",
    );
    b.source_len(
        "actionsDialog.shell.radius",
        "--sk-actions-dialog-radius",
        popup.shell.radius,
        "ActionsPopupShellTokens.radius (LIQUID_GLASS_POPUP_RADIUS_PX)",
    );
    b.source_len(
        "actionsDialog.shell.borderHeight",
        "--sk-actions-dialog-shell-border-height",
        popup.shell.border_height,
        "ActionsPopupShellTokens.border_height",
    );
    b.source_len(
        "actionsDialog.search.height",
        "--sk-actions-dialog-search-height",
        popup.search.height,
        "ActionsPopupSearchTokens.height (SEARCH_INPUT_HEIGHT)",
    );
    b.source_len(
        "actionsDialog.search.innerHeight",
        "--sk-actions-dialog-search-inner-height",
        popup.search.inner_height,
        "ActionsPopupSearchTokens.inner_height",
    );
    b.source_len(
        "actionsDialog.search.paddingX",
        "--sk-actions-dialog-search-padding-x",
        popup.search.padding_x,
        "ActionsPopupSearchTokens.padding_x (ACTION_PADDING_X)",
    );
    b.source_len(
        "actionsDialog.search.fontSize",
        "--sk-actions-dialog-search-font-size",
        popup.search.font_size,
        "ActionsPopupSearchTokens.font_size",
    );
    b.source_len(
        "actionsDialog.search.caretWidth",
        "--sk-actions-dialog-caret-width",
        popup.search.cursor_width,
        "ActionsPopupSearchTokens.cursor_width",
    );
    b.source_len(
        "actionsDialog.search.caretHeight",
        "--sk-actions-dialog-caret-height",
        popup.search.cursor_height,
        "ActionsPopupSearchTokens.cursor_height",
    );
    b.add(
        "actionsDialog.search.paddingYExtra",
        TokenStage::Source,
        None,
        TokenValue::Length {
            value: popup.search.padding_y_extra as f64,
        },
        Some("ActionsPopupSearchTokens.padding_y_extra"),
        true,
        &[],
    );
    b.source_len(
        "actionsDialog.list.rowHeight",
        "--sk-actions-dialog-row-height",
        popup.list.row_height,
        "ActionsPopupListTokens.row_height (ACTION_ITEM_HEIGHT)",
    );
    b.source_len(
        "actionsDialog.list.sectionHeaderHeight",
        "--sk-actions-dialog-section-height",
        popup.list.section_header_height,
        "ActionsPopupListTokens.section_header_height",
    );
    b.source_len(
        "actionsDialog.list.paddingTop",
        "--sk-actions-dialog-list-padding-top",
        popup.list.padding_top,
        "ActionsPopupListTokens.padding_top",
    );
    b.source_len(
        "actionsDialog.list.paddingBottom",
        "--sk-actions-dialog-list-padding-bottom",
        popup.list.padding_bottom,
        "ActionsPopupListTokens.padding_bottom",
    );
    b.source_len(
        "actionsDialog.row.wrapperInsetX",
        "--sk-actions-dialog-row-wrapper-inset-x",
        popup.row.inset_x,
        "ActionsPopupRowTokens.inset_x (ACTION_ROW_INSET)",
    );
    b.source_len(
        "actionsDialog.row.titleFontSize",
        "--sk-actions-dialog-row-title-font-size",
        popup.row.title_font_size,
        "ActionsPopupRowTokens.title_font_size",
    );
    b.source_len(
        "actionsDialog.section.paddingX",
        "--sk-actions-dialog-section-padding-x",
        section_chrome.padding_x,
        "ActionsPopupSectionTokens.padding_x (ACTION_PADDING_X)",
    );
    b.source_len(
        "actionsDialog.section.fontSize",
        "--sk-actions-dialog-section-font-size",
        section_chrome.font_size,
        "ActionsPopupSectionTokens.font_size",
    );
    b.add(
        "actionsDialog.section.fontWeight",
        TokenStage::Source,
        Some("--sk-actions-dialog-section-font-weight"),
        TokenValue::FontWeight {
            value: section_chrome.font_weight.0 as f64,
        },
        Some("ActionsPopupSectionTokens.font_weight"),
        true,
        &[],
    );

    // Declared-but-ineffective fields: exported to JSON so drift stays
    // visible, but with no CSS var and writable:false — the workbench must
    // not advertise edits that produce no pixels.
    for (id, value, path) in [
        (
            "actionsDialog.row.configuredRadius",
            popup.row.radius as f64,
            "ActionsPopupRowTokens.radius (ACTIONS_ROW_RADIUS) — NOT applied to the shared ListItem",
        ),
        (
            "actionsDialog.row.selectionOpacity",
            popup.row.selection_opacity as f64,
            "ActionsPopupRowTokens.selection_opacity — read into fallback style, never passed to ListItem",
        ),
        (
            "actionsDialog.row.hoverOpacity",
            popup.row.hover_opacity as f64,
            "ActionsPopupRowTokens.hover_opacity — read into fallback style, never passed to ListItem",
        ),
        (
            "actionsDialog.section.paddingTop",
            popup.section.padding_top as f64,
            "ActionsPopupSectionTokens.padding_top — section renderer centers vertically instead",
        ),
        (
            "actionsDialog.section.paddingBottom",
            popup.section.padding_bottom as f64,
            "ActionsPopupSectionTokens.padding_bottom — section renderer centers vertically instead",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number { value },
            Some(path),
            false,
            &[],
        );
    }
    for (id, value, path) in [
        (
            "actionsDialog.contract.searchPosition",
            crate::actions::constants::ACTIONS_DIALOG_EXPECT_SEARCH_POSITION.to_string(),
            "ACTIONS_DIALOG_EXPECT_SEARCH_POSITION",
        ),
        (
            "actionsDialog.contract.sectionMode",
            crate::actions::constants::ACTIONS_DIALOG_EXPECT_SECTION_MODE.to_string(),
            "ACTIONS_DIALOG_EXPECT_SECTION_MODE",
        ),
        (
            "actionsDialog.contract.searchDivider",
            crate::actions::constants::ACTIONS_DIALOG_EXPECT_SEARCH_DIVIDER.to_string(),
            "ACTIONS_DIALOG_EXPECT_SEARCH_DIVIDER",
        ),
        (
            "actionsDialog.contract.containerBorder",
            crate::actions::constants::ACTIONS_DIALOG_EXPECT_CONTAINER_BORDER.to_string(),
            "ACTIONS_DIALOG_EXPECT_CONTAINER_BORDER",
        ),
        (
            "actionsDialog.contract.footerVisible",
            (crate::actions::constants::ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT > 0).to_string(),
            "ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text { value },
            Some(path),
            false,
            &[],
        );
    }

    // Resolved actions-dialog paint values (what production actually draws).
    b.add(
        "resolved.actionsDialog.shell.fixtureHeight",
        TokenStage::Resolved,
        Some("--sk-actions-dialog-height"),
        TokenValue::Length {
            value: actions_fixture_height as f64,
        },
        None,
        false,
        &[
            "actionsDialog.list.rowHeight",
            "actionsDialog.search.height",
            "actionsDialog.list.sectionHeaderHeight",
        ],
    );
    b.add(
        "resolved.actionsDialog.shell.bottomResidualHeight",
        TokenStage::Resolved,
        Some("--sk-actions-dialog-bottom-residual-height"),
        TokenValue::Length {
            value: (popup.list.padding_bottom + popup.shell.border_height) as f64,
        },
        None,
        false,
        &[
            "actionsDialog.list.paddingBottom",
            "actionsDialog.shell.borderHeight",
        ],
    );
    b.add(
        "resolved.actionsDialog.search.paddingY",
        TokenStage::Resolved,
        Some("--sk-actions-dialog-search-padding-y"),
        TokenValue::Length {
            value: search_chrome.padding_y as f64,
        },
        None,
        false,
        &[
            "DesignSpacing.item_padding_y",
            "actionsDialog.search.paddingYExtra",
        ],
    );
    b.resolved_color(
        "resolved.actionsDialog.search.caretColor",
        "--sk-actions-dialog-caret",
        search_chrome.caret_rgba,
        &["theme.colors.accent.selected"],
    );
    b.resolved_color(
        "resolved.actionsDialog.search.placeholderColor",
        "--sk-actions-dialog-search-placeholder",
        search_chrome.placeholder_rgba,
        &["theme.colors.text.primary", "theme.opacity.textPlaceholder"],
    );
    b.resolved_color(
        "resolved.actionsDialog.search.textColor",
        "--sk-actions-dialog-search-text",
        search_chrome.input_text_rgba,
        &["theme.colors.text.primary"],
    );
    b.resolved_color(
        "resolved.actionsDialog.section.textColor",
        "--sk-actions-dialog-section-text",
        section_chrome.text_rgba,
        &["theme.colors.text.primary", "theme.opacity.textMutedAlpha"],
    );
    for (id, var, value, derived) in [
        (
            "resolved.actionsDialog.row.outerPaddingX",
            "--sk-actions-dialog-row-outer-padding-x",
            row_chrome.metrics.row_outer_padding_x,
            "MainMenuRowTokens.outer_padding_x",
        ),
        (
            "resolved.actionsDialog.row.outerPaddingY",
            "--sk-actions-dialog-row-outer-padding-y",
            row_chrome.metrics.row_outer_padding_y,
            "MainMenuRowTokens.outer_padding_y",
        ),
        (
            "resolved.actionsDialog.row.innerPaddingX",
            "--sk-actions-dialog-row-inner-padding-x",
            row_chrome.metrics.row_inner_padding_x,
            "MainMenuRowTokens.inner_padding_x",
        ),
        (
            "resolved.actionsDialog.row.surfaceInsetX",
            "--sk-actions-dialog-row-surface-inset-x",
            row_chrome.surface_inset_x,
            "actionsDialog.row.wrapperInsetX + row outer padding",
        ),
        (
            "resolved.actionsDialog.row.textOriginX",
            "--sk-actions-dialog-row-text-origin-x",
            row_chrome.text_origin_x,
            "wrapperInsetX + outer + inner padding",
        ),
        (
            "resolved.actionsDialog.row.shortcutRightInsetX",
            "--sk-actions-dialog-shortcut-right-inset-x",
            row_chrome.text_origin_x,
            "trailing mirror of textOriginX",
        ),
        (
            "resolved.actionsDialog.row.radius",
            "--sk-actions-dialog-row-radius",
            row_chrome.metrics.row_radius,
            "MainMenuRowTokens.radius — NOT ActionsPopupRowTokens.radius",
        ),
        (
            "resolved.actionsDialog.row.titleLineHeight",
            "--sk-actions-dialog-row-title-line-height",
            row_chrome.metrics.name_line_height,
            "max(main-menu name_line_height, actions title_font_size)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Resolved,
            Some(var),
            TokenValue::Length {
                value: value as f64,
            },
            None,
            false,
            &[derived],
        );
    }
    b.add(
        "resolved.actionsDialog.row.nameWeight",
        TokenStage::Resolved,
        Some("--sk-actions-dialog-row-name-font-weight"),
        TokenValue::FontWeight {
            value: row_chrome.metrics.name_weight.0 as f64,
        },
        None,
        false,
        &["MainMenuTypographyTokens.name_weight"],
    );
    b.add(
        "resolved.actionsDialog.row.selectedNameWeight",
        TokenStage::Resolved,
        Some("--sk-actions-dialog-row-selected-name-font-weight"),
        TokenValue::FontWeight {
            value: row_chrome.metrics.selected_name_weight.0 as f64,
        },
        None,
        false,
        &["MainMenuTypographyTokens.selected_name_weight"],
    );
    b.resolved_color(
        "resolved.actionsDialog.row.selectedBackground",
        "--sk-actions-dialog-row-selected-background",
        row_chrome.selected_background_rgba,
        &[
            "theme.colors.text.primary",
            "mainMenu.row.selectedFillAlpha",
        ],
    );
    b.resolved_color(
        "resolved.actionsDialog.row.hoverBackground",
        "--sk-actions-dialog-row-hover-background",
        row_chrome.hover_background_rgba,
        &["theme.colors.text.primary", "theme.opacity.hover"],
    );
    b.resolved_color(
        "resolved.actionsDialog.shell.popupTint",
        "--sk-actions-dialog-popup-tint",
        chrome.popup_surface_rgba,
        &["AppChromeColors.popup_surface_rgba"],
    );

    // ── Confirm prompt (in-window surface) ──────────────────────────────
    // Anatomy (pixel-validated 2026-07-11): main context header (8+22+8=38)
    // stacked above a FIXED-height `STANDARD_HEIGHT` shell that overflows the
    // window and is clipped; inside the shell a flex-centered title/body stack
    // sits above a footer spacer equal to the shared footer rail height. The
    // stack therefore centers on [headerHeight, headerHeight + (500 - 32)],
    // ~10.5pt below naive between-chrome centering.
    let confirm_metrics = crate::confirm::resolved_confirm_prompt_metrics(
        crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing(),
        fm.height_px,
    );
    let confirm_danger = crate::confirm::resolved_confirm_prompt_colors(&theme, true);
    let confirm_default = crate::confirm::resolved_confirm_prompt_colors(&theme, false);
    let confirm_header_height = (2.0 * crate::panel::HEADER_PADDING_Y + info.height_px)
        .max(crate::panel::HEADER_BUTTON_HEIGHT);

    b.source_len(
        "confirmPrompt.window.height",
        "--sk-confirm-window-height",
        f32::from(crate::window_resize::layout::STANDARD_HEIGHT),
        "crate::window_resize::layout::STANDARD_HEIGHT",
    );
    b.source_len(
        "confirmPrompt.content.padding",
        "--sk-confirm-content-padding",
        confirm_metrics.content_padding,
        "DesignSpacing.padding_xl",
    );
    b.source_len(
        "confirmPrompt.header.paddingX",
        "--sk-confirm-header-padding-x",
        crate::panel::HEADER_PADDING_X,
        "crate::panel::HEADER_PADDING_X (non-list views use 16, not the InfoBarBase shell 2)",
    );
    b.source_len(
        "confirmPrompt.header.paddingY",
        "--sk-confirm-header-padding-y",
        crate::panel::HEADER_PADDING_Y,
        "crate::panel::HEADER_PADDING_Y",
    );
    b.source_len(
        "confirmPrompt.stack.gap",
        "--sk-confirm-stack-gap",
        confirm_metrics.stack_gap,
        "DesignSpacing.padding_md (renderer gap; layout model claims 16 — see conflict)",
    );
    b.source_len(
        "confirmPrompt.title.fontSize",
        "--sk-confirm-title-font-size",
        confirm_metrics.title_font_size,
        "crate::confirm::CONFIRM_PROMPT_TITLE_FONT_SIZE_PX",
    );
    b.add(
        "confirmPrompt.title.fontWeight",
        TokenStage::Source,
        Some("--sk-confirm-title-font-weight"),
        TokenValue::FontWeight {
            value: gpui::FontWeight::SEMIBOLD.0 as f64,
        },
        Some("gpui::FontWeight::SEMIBOLD in render_confirm_prompt"),
        true,
        &[],
    );
    b.source_len(
        "confirmPrompt.body.fontSize",
        "--sk-confirm-body-font-size",
        confirm_metrics.body_font_size,
        "crate::confirm::CONFIRM_PROMPT_BODY_FONT_SIZE_PX",
    );
    b.source_len(
        "confirmPrompt.stack.maxWidth",
        "--sk-confirm-stack-max-width",
        confirm_metrics.body_max_width,
        "crate::confirm::CONFIRM_PROMPT_BODY_MAX_WIDTH_PX (body max_w; title is intrinsic — see conflict)",
    );
    for (id, var, value, derived) in [
        (
            "resolved.confirmPrompt.title.lineHeight",
            "--sk-confirm-title-line-height",
            confirm_metrics.title_line_height,
            "gpui TextStyle default phi() line height, rounded (20 → 32)",
        ),
        (
            "resolved.confirmPrompt.body.lineHeight",
            "--sk-confirm-body-line-height",
            confirm_metrics.body_line_height,
            "gpui TextStyle default phi() line height, rounded (14 → 23)",
        ),
        (
            "resolved.confirmPrompt.footerSpacerHeight",
            "--sk-confirm-footer-spacer-height",
            confirm_metrics.footer_spacer_height,
            "footer rail height via render_native_main_window_footer_spacer",
        ),
        (
            "resolved.confirmPrompt.headerHeight",
            "--sk-confirm-header-height",
            confirm_header_height,
            "HEADER_PADDING_Y*2 + HeaderInfoBarTokens.height_px, min HEADER_BUTTON_HEIGHT",
        ),
    ] {
        b.add(
            id,
            TokenStage::Resolved,
            Some(var),
            TokenValue::Length {
                value: value as f64,
            },
            None,
            false,
            &[derived],
        );
    }
    b.resolved_color(
        "resolved.confirmPrompt.titleDanger",
        "--sk-confirm-title-danger",
        confirm_danger.title_rgba,
        &["theme.colors.ui.error"],
    );
    b.resolved_color(
        "resolved.confirmPrompt.titleDefault",
        "--sk-confirm-title-default",
        confirm_default.title_rgba,
        &["theme.colors.text.primary"],
    );
    b.resolved_color(
        "resolved.confirmPrompt.bodyText",
        "--sk-confirm-body-text",
        confirm_danger.body_rgba,
        &["theme.colors.text.secondary"],
    );

    b.conflict(
        "confirmLayout.protocolModelVsRendererTruth",
        &[
            (
                "protocol layout model",
                "content at (0,0), title slot y=189, footer host 38".to_string(),
            ),
            (
                "renderer + pixels",
                format!(
                    "context header {confirm_header_height} above a fixed-{} shell; stack centers on the shell's flex region; footer band = rail {} at the window bottom",
                    f32::from(crate::window_resize::layout::STANDARD_HEIGHT),
                    fm.height_px
                ),
            ),
        ],
        "warning",
        "getLayoutInfo's confirm branch reports content-local coordinates that ignore \
         the main context header and reserve a 38px footer. Pixel measurement of the \
         2026-07-11 capture places the title ~59pt lower than the model claims. Trust \
         the renderer + raster, not the synthetic model, until the model is fixed.",
    );
    b.conflict(
        "confirmGap.rendererSpacingVsLayoutOracle",
        &[
            (
                "renderer DesignSpacing.padding_md",
                format!("{}", confirm_metrics.stack_gap),
            ),
            ("layout model title→body gap", "16".to_string()),
        ],
        "info",
        "The renderer's title/body gap is padding_md (12); the protocol layout model \
         hardcodes 16. Cross-capture pixel measurement confirms 12.",
    );
    b.conflict(
        "confirmTypography.implicitLineHeightVsModeledSlots",
        &[
            (
                "resolved phi line heights",
                format!(
                    "title {} / body {}",
                    confirm_metrics.title_line_height, confirm_metrics.body_line_height
                ),
            ),
            ("layout model slots", "title 28 / body 40".to_string()),
        ],
        "info",
        "The renderer never sets line heights; GPUI's default phi() line height \
         (rounded) applies. Body line spacing measured at exactly 23.0pt.",
    );
    b.conflict(
        "confirmFooter.heightLadder",
        &[
            (
                "footer rail (shell spacer + visible band)",
                format!("{}", fm.height_px),
            ),
            (
                "main-menu native host",
                format!(
                    "{}",
                    crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT
                ),
            ),
            ("protocol confirm claim", "38".to_string()),
        ],
        "info",
        "Multiple footer heights coexist. The confirm capture's visible band starts at \
         window bottom minus the 32px rail; the protocol model's 38 has no renderer \
         authority.",
    );
    b.conflict(
        "confirmFooter.slotVsInnerFrame",
        &[
            ("Apply/Close slot maxima", "84".to_string()),
            (
                "native inner frames",
                "78 wide, 8 gap, 16 right inset".to_string(),
            ),
        ],
        "info",
        "Native AppKit insets the shared 84px action slots into 78px visible frames. \
         Do not rewrite slot tokens from measured frames.",
    );
    b.conflict(
        "confirmFooter.selectedNeutralVsDangerTitle",
        &[
            (
                "danger semantic",
                "title uses theme.colors.ui.error".to_string(),
            ),
            (
                "selected footer fill",
                "neutral text-primary at the footer active byte".to_string(),
            ),
        ],
        "info",
        "Danger affects only the title color; the focused Delete button paints the \
         neutral shared footer active fill, not red.",
    );
    b.conflict(
        "confirmStack.rendererIntrinsicVsLayoutModel",
        &[
            (
                "renderer",
                "no stack wrapper; title intrinsic width; body max_w 560".to_string(),
            ),
            (
                "layout model",
                "stack/title/body all reported as x=95 w=560".to_string(),
            ),
        ],
        "info",
        "The 560px reading column exists only as the body's max width; the title is \
         intrinsic-width centered. The model's uniform 560 boxes are synthetic.",
    );
    b.conflict(
        "confirmCapture.stockThemeVsReferenceRaster",
        &[
            ("stock danger", format!("#{:06X}", theme.colors.ui.error)),
            (
                "capture title sample",
                "~#E85841 (profile/vibrancy shift)".to_string(),
            ),
        ],
        "info",
        "Reference-capture hues drift from stock bytes via color profile and vibrancy \
         blending. Geometry stays blocking; hue does not.",
    );

    // ── Actions-dialog conflicts (recorded, not collapsed) ─────────────
    b.conflict(
        "actionsRow.compactSlotVsInheritedItemHeight",
        &[
            (
                "ActionsPopupListTokens.row_height",
                format!("{}", popup.list.row_height),
            ),
            (
                "MainMenuListTokens.item_height",
                format!("{}", def.list.item_height),
            ),
            (
                "crate::list_item::LIST_ITEM_HEIGHT",
                format!("{}", crate::list_item::LIST_ITEM_HEIGHT),
            ),
        ],
        "info",
        "The action row wrapper constrains the shared ListItem into a 36px slot; the \
         inherited main-menu item height (44) and legacy constant (40) never paint here.",
    );
    b.conflict(
        "actionsRow.radiusConfiguredVsPainted",
        &[
            (
                "ActionsPopupRowTokens.radius",
                format!("{}", popup.row.radius),
            ),
            (
                "resolved ListItemMetricsOverride.row_radius",
                format!("{}", row_chrome.metrics.row_radius),
            ),
        ],
        "info",
        "The renderer never applies the configured actions row radius; the shared \
         ListItem paints the main-menu radius (14). HTML must paint 14.",
    );
    b.conflict(
        "actionsRow.selectionConfiguredVsPainted",
        &[
            (
                "ActionsPopupRowTokens.selection_opacity",
                format!("{}", popup.row.selection_opacity),
            ),
            ("theme.opacity.selected", format!("{}", opacity.selected)),
            ("painted component byte", "0x20".to_string()),
        ],
        "info",
        "The actions selection opacity is read into a fallback style but never passed \
         to ListItem; the painted selected fill is the shared component byte #FFFFFF20.",
    );
    b.conflict(
        "actionsRow.hoverConfiguredVsPainted",
        &[
            (
                "ActionsPopupRowTokens.hover_opacity",
                format!("{}", popup.row.hover_opacity),
            ),
            ("theme.opacity.hover", format!("{}", opacity.hover)),
            ("painted component byte", "0x12".to_string()),
        ],
        "info",
        "Actual hover uses the shared row resolver's component floor, not the actions \
         hover opacity field.",
    );
    b.conflict(
        "actionsSection.paddingDeclaredVsCenteredRenderer",
        &[
            (
                "ActionsPopupSectionTokens.padding_top",
                format!("{}", popup.section.padding_top),
            ),
            (
                "ActionsPopupSectionTokens.padding_bottom",
                format!("{}", popup.section.padding_bottom),
            ),
            (
                "renderer",
                "vertically centered in the 24px slot".to_string(),
            ),
        ],
        "info",
        "The actions section renderer consumes height, X padding, font and color but \
         not the declared vertical padding fields.",
    );
    b.conflict(
        "actionsShortcut.popupTokensVsFooterRenderer",
        &[
            (
                "live renderer",
                "footer_chrome::render_footer_row_shortcut_keycaps_from_tokens".to_string(),
            ),
            (
                "keycap metrics",
                "FooterMetricsTokens (footer.keycap*)".to_string(),
            ),
        ],
        "info",
        "Action-row shortcut keycaps are painted by the shared footer-chrome renderer; \
         the mockup must reuse .sk-keycap and footer keycap tokens, not invent \
         actions-specific duplicates.",
    );
    b.conflict(
        "actionsFooter.legacyHeightVsFooterlessContract",
        &[
            ("legacy popup footer height", "32".to_string()),
            ("contract footerVisible", "false".to_string()),
            (
                "resolved bottom residual",
                format!("{}", popup.list.padding_bottom + popup.shell.border_height),
            ),
        ],
        "info",
        "The 32px popup footer height survives in generic sizing paths but is forbidden \
         for the normal actions dialog; the visible bottom band is list padding (6) plus \
         shell border reserve (2), sharing the shell material — not a footer.",
    );
    b.conflict(
        "actionsCaret.stockProfileVsReferenceCapture",
        &[
            (
                "stock accent",
                format!("#{:06X}", theme.colors.accent.selected),
            ),
            (
                "2026-07-11 capture caret",
                "orange-leaning sample (color-profile or live-theme drift)".to_string(),
            ),
        ],
        "info",
        "The devtools reference capture's caret does not visually match the stock amber \
         accent. Never overwrite the stock token from a PNG sample; retake the capture \
         under the stock profile or treat caret hue as non-blocking.",
    );
    b.conflict(
        "actionsAlpha.truncateVsRoundedChromeHelpers",
        &[
            (
                "actions dialog",
                "(opacity * 255.0) as u8 (truncating)".to_string(),
            ),
            ("generic helpers", "some use .round()".to_string()),
        ],
        "info",
        "One-byte alpha differences are possible between truncating and rounding \
         helpers; the exporter calls the dialog's own truncating path.",
    );

    // ── Notes window (separate NSPanel) ─────────────────────────────────
    // App-authored chrome + the layout model come from the production
    // contract (`notes::window::contract`) — the SAME typed source
    // window_ops, the titlebar renderer, and autosize consume (and
    // explicitly NOT the feature-sensitive `adopted_style()`). Editor
    // typography/caret resolve through the notes-editor contract (the
    // theme → gpui-component bridge, NOT `FontConfig::default()`), the
    // painted footer band through the shared footer_chrome formula owner,
    // and markdown capture styles through the real highlight-theme resolver
    // beside `register_markdown_highlighter`.
    let notes_chrome = crate::notes::window::contract::production_notes_window_contract();
    let notes_layout = crate::notes::window::contract::production_notes_layout_model();
    let notes_editor =
        crate::components::notes_editor::contract::resolved_notes_editor_metrics(&theme);
    let notes_markdown =
        crate::notes::markdown_highlighting::resolved_notes_markdown_styles(&theme, true);
    let notes_markdown_runtime =
        crate::notes::markdown_highlighting::markdown_editor_runtime_info();
    let notes_footer_intrinsic =
        crate::notes::window::contract::resolved_notes_footer_intrinsic_height(fm.button_padding_y);

    // Source: app-authored Notes chrome (writable leaves).
    for (id, var, value, path) in [
        (
            "notes.window.defaultWidth",
            "--sk-notes-window-width",
            notes_chrome.default_width,
            "notes::window::contract::NOTES_DEFAULT_WIDTH",
        ),
        (
            "notes.window.defaultHeight",
            "--sk-notes-window-height",
            notes_chrome.default_height,
            "notes::window::contract::NOTES_DEFAULT_HEIGHT",
        ),
        (
            "notes.titlebar.height",
            "--sk-notes-titlebar-height",
            notes_chrome.titlebar_height,
            "NotesWindowStyle::current().titlebar_height",
        ),
        (
            "notes.titlebar.paddingX",
            "--sk-notes-titlebar-padding-x",
            notes_chrome.titlebar_padding_x,
            "notes::window::contract::NOTES_TITLEBAR_PADDING_X",
        ),
        (
            "notes.titlebar.leadingReserveWidth",
            "--sk-notes-titlebar-traffic-width",
            notes_chrome.titlebar_leading_reserve_width,
            "notes::window::TITLEBAR_TRAFFIC_LIGHT_W",
        ),
        (
            "notes.titlebar.trailingReserveWidth",
            "--sk-notes-titlebar-icons-width",
            notes_chrome.titlebar_trailing_reserve_width,
            "notes::window::TITLEBAR_ICONS_W",
        ),
        (
            "notes.titlebar.trafficLightOriginX",
            "--sk-notes-traffic-x",
            notes_chrome.traffic_light_origin_x,
            "notes::window::contract::NOTES_TRAFFIC_LIGHT_ORIGIN_X",
        ),
        (
            "notes.titlebar.trafficLightOriginY",
            "--sk-notes-traffic-y",
            notes_chrome.traffic_light_origin_y,
            "notes::window::contract::NOTES_TRAFFIC_LIGHT_ORIGIN_Y",
        ),
        (
            "notes.editor.paddingX",
            "--sk-notes-editor-padding-x",
            notes_chrome.editor_padding_x,
            "NotesWindowStyle::current().editor_padding_x",
        ),
        (
            "notes.editor.paddingY",
            "--sk-notes-editor-padding-y",
            notes_chrome.editor_padding_y,
            "NotesWindowStyle::current().editor_padding_y",
        ),
        (
            "notes.footer.statusMinWidth",
            "--sk-notes-footer-status-min-width",
            notes_chrome.footer_status_min_width,
            "notes::window::MIN_TARGET_SIZE",
        ),
        (
            "notes.footer.contentInsetX",
            "--sk-notes-footer-content-inset-x",
            notes_chrome.footer_content_inset_x,
            "crate::window_resize::main_layout::HINT_STRIP_PADDING_X",
        ),
        (
            "notes.footer.actionGap",
            "--sk-notes-footer-action-gap",
            crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX,
            "footer_chrome::FOOTER_ACTION_ITEM_GAP_PX",
        ),
    ] {
        b.source_len(id, var, value, path);
    }
    b.add(
        "notes.window.defaultEdgePadding",
        TokenStage::Source,
        None,
        TokenValue::Length {
            value: notes_chrome.default_edge_padding as f64,
        },
        Some("notes::window::contract::NOTES_DEFAULT_EDGE_PADDING"),
        true,
        &[],
    );
    b.add(
        "notes.titlebar.titleRestOpacity",
        TokenStage::Source,
        Some("--sk-notes-titlebar-title-rest-opacity"),
        TokenValue::Number {
            value: notes_chrome.title_rest_opacity as f64,
        },
        Some("notes::window::OPACITY_MUTED"),
        true,
        &[],
    );
    b.add(
        "notes.footer.restOpacity",
        TokenStage::Source,
        Some("--sk-notes-footer-rest-opacity"),
        TokenValue::Number {
            value: notes_chrome.footer_rest_opacity as f64,
        },
        Some("notes::window::OPACITY_SUBTLE"),
        true,
        &[],
    );

    // Layout MODEL (autosize + automation_layout_info reservation), under
    // honest model names — NOT painted geometry. The 28px footer
    // reservation deliberately stays 28 (see the conflict below).
    b.add(
        "notes.layout.footerReservationHeight",
        TokenStage::Source,
        None,
        TokenValue::Length {
            value: notes_layout.footer_reservation_height as f64,
        },
        Some("NotesLayoutMetrics::footer_height (autosize + automation_layout_info)"),
        true,
        &[],
    );
    for (id, value, path) in [
        (
            "notes.layout.autoResize.maxHeight",
            notes_layout.auto_resize_max_height as f64,
            "NotesLayoutMetrics::auto_resize_max_height",
        ),
        (
            "notes.layout.autoResize.assumedLineHeight",
            notes_layout.auto_resize_assumed_line_height as f64,
            "NotesLayoutMetrics::auto_resize_line_height — an autosize ASSUMPTION, not the Input's painted line box",
        ),
        (
            "notes.layout.autoResize.applyThreshold",
            notes_layout.auto_resize_threshold as f64,
            "NotesLayoutMetrics::auto_resize_threshold",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Length { value },
            Some(path),
            false,
            &[],
        );
    }

    // Contract facts (JSON-only text records; not visual numbers).
    for (id, value, path) in [
        (
            "notes.footer.presentation",
            crate::notes::window::contract::NOTES_FOOTER_PRESENTATION.to_string(),
            "notes::window::contract::NOTES_FOOTER_PRESENTATION",
        ),
        (
            "notes.footer.nativeOverlay",
            crate::notes::window::contract::NOTES_FOOTER_NATIVE_OVERLAY.to_string(),
            "notes::window::contract::NOTES_FOOTER_NATIVE_OVERLAY",
        ),
        (
            "notes.footer.visibility",
            crate::notes::window::contract::NOTES_FOOTER_VISIBILITY.to_string(),
            "notes::window::contract::NOTES_FOOTER_VISIBILITY",
        ),
        (
            "notes.editor.markdown.language",
            notes_markdown_runtime.language.clone(),
            "notes::markdown_highlighting::MARKDOWN_LANGUAGE",
        ),
        (
            "notes.editor.markdown.highlightQueryFingerprint",
            notes_markdown_runtime.highlight_query_fingerprint.clone(),
            "markdown_editor_runtime_info().highlight_query_fingerprint",
        ),
        (
            "notes.editor.markdown.injectionQueryFingerprint",
            notes_markdown_runtime.injection_query_fingerprint.clone(),
            "markdown_editor_runtime_info().injection_query_fingerprint",
        ),
        (
            "notes.editor.markdown.inlineHighlightQueryFingerprint",
            notes_markdown_runtime
                .inline_highlight_query_fingerprint
                .clone(),
            "markdown_editor_runtime_info().inline_highlight_query_fingerprint",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text { value },
            Some(path),
            false,
            &[],
        );
    }

    // Resolved: what the Notes window actually paints (never writable).
    for (id, var, value, derived) in [
        (
            "resolved.notes.editor.baseFontSize",
            "--sk-notes-editor-font-size",
            notes_editor.base_font_size,
            "theme.fonts.monoSize via sync_gpui_component_theme → Input::text_size",
        ),
        (
            "resolved.notes.editor.lineBoxHeight",
            "--sk-notes-editor-line-box-height",
            notes_editor.line_box_height,
            "gpui-component Input line_height Rems(1.25) × 16px rem",
        ),
        (
            "resolved.notes.editor.caretWidth",
            "--sk-notes-caret-width",
            notes_editor.caret_width,
            "gpui-component blink_cursor::CURSOR_WIDTH",
        ),
        (
            "resolved.notes.editor.caretHeight",
            "--sk-notes-caret-height",
            notes_editor.caret_height,
            "resolved.notes.editor.lineBoxHeight × 0.85 (Size::Medium)",
        ),
        (
            "resolved.notes.editor.inputPaddingX",
            "--sk-notes-editor-input-padding-x",
            notes_editor.input_padding_x,
            "gpui_component::Size::Medium.input_px() — the REAL vendored accessor, not a copy",
        ),
        (
            "resolved.notes.editor.inputPaddingY",
            "--sk-notes-editor-input-padding-y",
            notes_editor.input_padding_y,
            "gpui_component::Size::Medium.input_py() — the REAL vendored accessor, not a copy",
        ),
        (
            "resolved.notes.footer.intrinsicHeight",
            "--sk-notes-footer-height",
            notes_footer_intrinsic,
            "footer_chrome::footer_button_height_in(HINT_STRIP_HEIGHT, footer.buttonPaddingY)",
        ),
        (
            "resolved.notes.titlebar.titleFontSize",
            "--sk-notes-titlebar-title-font-size",
            14.0,
            "gpui text_sm (0.875rem × 16px rem) in render_editor_titlebar",
        ),
        (
            "resolved.notes.footer.statusFontSize",
            "--sk-notes-footer-status-font-size",
            12.0,
            "gpui text_xs (0.75rem × 16px rem) in render_editor_footer",
        ),
    ] {
        b.add(
            id,
            TokenStage::Resolved,
            Some(var),
            TokenValue::Length {
                value: value as f64,
            },
            None,
            false,
            &[derived],
        );
    }
    // CSS-exposed since the Day Page slice: the editor's family is the theme
    // bridge's mono family, NOT list_item::FONT_MONO (--sk-font-mono). Both
    // say "JetBrains Mono" today, but the authorities differ.
    b.add(
        "resolved.notes.editor.fontFamily",
        TokenStage::Resolved,
        Some("--sk-notes-editor-font-family"),
        TokenValue::Text {
            value: notes_editor.base_font_family.clone(),
        },
        None,
        false,
        &["theme.fonts.monoFamily via sync_gpui_component_theme"],
    );

    // Resolved editor/link colors, read from the SAME theme bridge
    // (map_scriptkit_to_gpui_theme) the renderer's cx.theme() carries. The
    // link label accent and the markdown TITLE color are separate
    // authorities that happen to both be amber in the stock theme.
    b.add(
        "resolved.notes.editor.textColor",
        TokenStage::Resolved,
        Some("--sk-notes-editor-text-color"),
        hsla_color_value(notes_editor.text_color),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "window text_style — host roots install .text_color(text.primary)",
        ],
    );
    b.add(
        "resolved.notes.editor.caretColor",
        TokenStage::Resolved,
        Some("--sk-notes-caret-color"),
        hsla_color_value(notes_editor.caret_color),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "map_scriptkit_to_gpui_theme → theme_color.caret (no focused-cursor override in script-kit-dark)",
        ],
    );
    b.add(
        "resolved.notes.editor.linkLabelColor",
        TokenStage::Resolved,
        Some("--sk-notes-editor-link-label"),
        hsla_color_value(notes_editor.link_label_color),
        None,
        false,
        &[
            "theme.colors.accent.selected",
            "map_scriptkit_to_gpui_theme → theme_color.accent (markdown link highlighter)",
        ],
    );
    b.add(
        "resolved.notes.editor.linkDestinationRestColor",
        TokenStage::Resolved,
        Some("--sk-notes-editor-link-destination-rest"),
        hsla_color_value(notes_editor.link_destination_rest_color),
        None,
        false,
        &[
            "resolved.notes.editor.linkLabelColor",
            "notesEditor.link.destinationCompactOpacity",
        ],
    );
    // Authored leaf behind the rest color — JSON-only (the mockup consumes
    // the resolved color above, never a browser opacity layer).
    b.add(
        "notesEditor.link.destinationCompactOpacity",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: notes_editor.link_destination_compact_opacity as f64,
        },
        Some("notes_editor::component::MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY"),
        true,
        &[],
    );
    // Behavior fact: the destination is compact unless the selection
    // overlaps OR TOUCHES the link's full range (collapsed caret included).
    b.add(
        "notesEditor.link.destinationStateRule",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: "compactUnlessSelectionOverlapsOrTouchesFullRange".to_string(),
        },
        Some("notes_editor::component::markdown_link_destination_color"),
        false,
        &[],
    );

    // Resolved markdown capture styles, read from the SAME highlight theme
    // the Input paints with (build_markdown_highlight_theme). Copying color
    // literals here is forbidden; if the resolver ever loses access, emit
    // the notesMarkdown.exporterVisibilityMissing conflict instead.
    match (
        notes_markdown.title.color,
        notes_markdown.heading_marker.color,
        notes_markdown.list_marker.color,
    ) {
        (Some(title_color), Some(heading_marker_color), Some(list_marker_color)) => {
            b.add(
                "resolved.notes.editor.markdown.titleColor",
                TokenStage::Resolved,
                Some("--sk-notes-markdown-title-color"),
                hsla_color_value(title_color),
                None,
                false,
                &[
                    "theme.colors.accent.selected",
                    "highlight_theme.syntax.title",
                ],
            );
            b.add(
                "resolved.notes.editor.markdown.headingMarkerColor",
                TokenStage::Resolved,
                Some("--sk-notes-markdown-heading-marker-color"),
                hsla_color_value(heading_marker_color),
                None,
                false,
                &[
                    "theme.colors.text.muted",
                    "highlight_theme.syntax.punctuation_special",
                ],
            );
            b.add(
                "resolved.notes.editor.markdown.listMarkerColor",
                TokenStage::Resolved,
                Some("--sk-notes-markdown-list-marker-color"),
                hsla_color_value(list_marker_color),
                None,
                false,
                &[
                    "theme.colors.accent.selected",
                    "highlight_theme.syntax.punctuation_list_marker",
                ],
            );
            if let Some(weight) = notes_markdown.title.font_weight {
                b.add(
                    "resolved.notes.editor.markdown.titleFontWeight",
                    TokenStage::Resolved,
                    Some("--sk-notes-markdown-title-font-weight"),
                    TokenValue::FontWeight {
                        value: weight as f64,
                    },
                    None,
                    false,
                    &["highlight_theme.syntax.title"],
                );
            }
            if let Some(weight) = notes_markdown.list_marker.font_weight {
                b.add(
                    "resolved.notes.editor.markdown.listMarkerFontWeight",
                    TokenStage::Resolved,
                    Some("--sk-notes-markdown-list-marker-font-weight"),
                    TokenValue::FontWeight {
                        value: weight as f64,
                    },
                    None,
                    false,
                    &["highlight_theme.syntax.punctuation_list_marker"],
                );
            }
        }
        _ => {
            b.conflict(
                "notesMarkdown.exporterVisibilityMissing",
                &[
                    (
                        "query captures",
                        "title / punctuation.special / punctuation.list_marker".to_string(),
                    ),
                    (
                        "observed raster",
                        "accent bold title, dimmer # marker, accent list dashes".to_string(),
                    ),
                    (
                        "exporter access",
                        "highlight theme returned no color for a contract capture".to_string(),
                    ),
                ],
                "warning",
                "The markdown capture styles could not be resolved from the real highlight \
                 theme; the color tokens are intentionally OMITTED rather than copied from \
                 screenshots. Fix the resolver, never hardcode the bytes.",
            );
        }
    }

    // ── Notes conflicts (recorded, not collapsed) ───────────────────────
    b.conflict(
        "notesFooter.layoutReservationVsIntrinsicPaint",
        &[
            (
                "NotesLayoutMetrics.footer_height / autosize",
                format!("{}", notes_layout.footer_reservation_height),
            ),
            (
                "automation_layout_info footer bounds",
                format!("{}", notes_layout.footer_reservation_height),
            ),
            (
                "GPUI universal footer action row",
                format!("{notes_footer_intrinsic}"),
            ),
        ],
        "warning",
        "The layout model reserves 28px for the Notes footer while the painted \
         universal action-button row is 32px: autosize and the layout oracle \
         under-reserve the visible band by 4px. The 280px default-height fixture \
         masks it (initial-height floor). Mockups must paint the 32px resolved \
         truth; do NOT change NotesWindowStyle.footer_height here — that would be \
         an app behavior fix, not a contract record.",
    );
    b.conflict(
        "notesFooter.buttonHeightSourceDuplication",
        &[
            (
                "Notes renderer host band",
                format!(
                    "main_layout::HINT_STRIP_HEIGHT = {}",
                    crate::window_resize::main_layout::HINT_STRIP_HEIGHT
                ),
            ),
            (
                "main window native host",
                format!(
                    "NATIVE_MAIN_WINDOW_FOOTER_HEIGHT = {}",
                    crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT
                ),
            ),
        ],
        "info",
        "Notes derives its 32px button row from HINT_STRIP_HEIGHT while the \
         exported shared --sk-footer-button-height derives from the main window's \
         native footer host. The numbers coincide (both 36-hosted) but the \
         provenance differs; Notes has NO native 36px footer host — its footer is \
         an in-window GPUI strip (notes.footer.presentation).",
    );
    b.conflict(
        "notesMarkdown.titleGlyphExtentsVsLineBox",
        &[
            (
                "Input painted line box",
                format!("{}", notes_editor.line_box_height),
            ),
            (
                "resolved title style",
                format!(
                    "bold (weight {}) at the shared {}px editor size — no capture font-size \
                     exists (gpui HighlightStyle is uniformly sized)",
                    notes_markdown
                        .title
                        .font_weight
                        .map(|w| w.to_string())
                        .unwrap_or_else(|| "unset".to_string()),
                    notes_editor.base_font_size
                ),
            ),
            (
                "observed raster",
                "heading glyph tops clipped in the 2026-07-11 reference capture".to_string(),
            ),
        ],
        "warning",
        "Bold markdown title runs paint inside the Input's fixed 20px line box and \
         their upper glyph area clips. Expected consequence of the Input primitive, \
         but a screen-level renderer defect — the heading is NOT a larger nominal \
         font size (same mono advance as body lines). Mockups must reproduce the \
         clip via the line box, never by inflating the heading font.",
    );

    // ── Settings hub (built-in list surface) ────────────────────────────
    // Owners split per the 2026-07-11 Oracle review: the list padding stays
    // canonical under `design.spacing.*`; the trailing count label is owned
    // by the SHARED builtin main-input helper (every builtin browser paints
    // it); and the first "Settings" separator is owned by the LEGACY
    // list-item default metrics that `render_section_header` actually paints
    // with (`resolved_list_item_metrics()` → `default_main_menu()`, 26/6 —
    // NOT the themed InfoBarBase 28/4). Settings itself mints NO alias
    // tokens: count inset reuses `--sk-main-menu-search-text-inset-x`, count
    // color reuses `--sk-text-hint`. Content facts (labels, census,
    // pluralization) are JSON-only records with no CSS role. The search
    // placeholder ("Search settings...") is deliberately NOT exported until
    // it has a shared Rust constant (it currently lives inline in
    // `builtin_execution.rs`); duplicating the literal here is forbidden.
    let settings_layout =
        crate::settings_hub_contract::resolved_settings_hub_layout(default_spacing);
    let count_label_style =
        crate::builtin_main_input_contract::resolved_builtin_main_input_count_label_style(
            def, &chrome,
        );
    let list_item_default_metrics = ListItemMetricsOverride::default_main_menu();
    let settings_facts_fresh = crate::settings_hub_contract::settings_hub_contract_facts(false);
    let settings_facts_custom = crate::settings_hub_contract::settings_hub_contract_facts(true);
    debug_assert_eq!(settings_layout.list_padding_y, default_spacing.padding_xs);
    debug_assert_eq!(count_label_style.inset_right, def.search.text_inset_x);
    debug_assert_eq!(count_label_style.text_rgba, chrome.text_hint_rgba);

    b.source_len(
        "design.spacing.paddingXs",
        "--sk-spacing-padding-xs",
        settings_layout.list_padding_y,
        "DesignSpacing.padding_xs (Default variant; render_settings maps its content padding-block here via resolved_settings_hub_layout)",
    );
    b.add(
        "resolved.builtinMainInput.countLabel.fontSize",
        TokenStage::Resolved,
        Some("--sk-builtin-main-input-count-font-size"),
        TokenValue::Length {
            value: count_label_style.font_size_px as f64,
        },
        None,
        false,
        &["gpui Styled::text_sm() rems(0.875) × 16px rem (render_builtin_main_input_count_label)"],
    );
    b.add(
        "resolved.builtinMainInput.countLabel.lineHeight",
        TokenStage::Resolved,
        Some("--sk-builtin-main-input-count-line-height"),
        TokenValue::Length {
            value: count_label_style.line_height_px as f64,
        },
        None,
        false,
        &["gpui TextStyle default phi() line height, rounded (14 → 23)"],
    );
    b.add(
        "resolved.builtinMainInput.countLabel.fontWeight",
        TokenStage::Resolved,
        Some("--sk-builtin-main-input-count-font-weight"),
        TokenValue::FontWeight {
            value: count_label_style.font_weight.0 as f64,
        },
        None,
        false,
        &["gpui::FontWeight::NORMAL — the count helper sets no weight; it must not inherit the search body's 430"],
    );
    for (id, var, value, derived) in [
        (
            "resolved.listItem.default.firstSectionSlotHeight",
            "--sk-list-item-default-first-section-slot-height",
            list_item_default_metrics.first_section_header_height,
            "crate::list_item::SECTION_HEADER_HEIGHT − MAIN_MENU_SECTION_PADDING_TOP/2 (render_section_header legacy default path, is_first)",
        ),
        (
            "resolved.listItem.default.firstSectionPaddingTop",
            "--sk-list-item-default-first-section-padding-top",
            list_item_default_metrics.first_section_padding_top,
            "MAIN_MENU_SECTION_PADDING_TOP / 2 (render_section_header legacy default path, is_first)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Resolved,
            Some(var),
            TokenValue::Length {
                value: value as f64,
            },
            None,
            false,
            &[derived],
        );
    }

    // JSON-only settings facts (text/number records; no CSS role, never
    // writable through the design-token reverse path).
    for (id, value, path) in [
        (
            "settingsHub.section.emptyFilterLabel",
            settings_facts_fresh.empty_filter_section_label.to_string(),
            "settings_hub_contract::SETTINGS_HUB_EMPTY_FILTER_SECTION_LABEL (persistent leading separator, empty filter)",
        ),
        (
            "settingsHub.section.filteredLabel",
            settings_facts_fresh.filtered_section_label.to_string(),
            "settings_hub_contract::SETTINGS_HUB_FILTERED_SECTION_LABEL (persistent leading separator, active filter)",
        ),
        (
            "settingsHub.countLabel.counts",
            "visibleFilteredRows".to_string(),
            "render_settings item_count = filtered_settings_items(items, filter).len()",
        ),
        (
            "settingsHub.countLabel.pluralization",
            format!(
                "{} / {}",
                crate::settings_hub_contract::format_settings_count_label(1),
                crate::settings_hub_contract::format_settings_count_label(2),
            ),
            "settings_hub_contract::format_settings_count_label",
        ),
        (
            "settingsHub.census.optionalRow",
            crate::settings_hub_contract::SETTINGS_HUB_OPTIONAL_ROW_NAME.to_string(),
            "get_settings_items_for(has_custom_positions) conditional push",
        ),
        (
            "settingsHub.census.optionalPredicate",
            "windowState.hasCustomPositions".to_string(),
            "crate::window_state::has_custom_positions via the bin-side get_settings_items wrapper",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text { value },
            Some(path),
            false,
            &[],
        );
    }
    for (id, value, path) in [
        (
            "settingsHub.census.baseCount",
            settings_facts_fresh.row_count as f64,
            "settings_hub_contract_facts(false).row_count",
        ),
        (
            "settingsHub.census.customPositionsCount",
            settings_facts_custom.row_count as f64,
            "settings_hub_contract_facts(true).row_count",
        ),
        (
            "settingsHub.icons.resolvedRowIconCount",
            settings_facts_custom.resolved_icon_rows as f64,
            "IconKind::from_icon_hint over get_settings_items_for(true) — authored hints currently never parse",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number { value },
            Some(path),
            false,
            &[],
        );
    }

    // ── Settings conflicts (recorded, not collapsed) ────────────────────
    b.conflict(
        "settingsSection.firstSlotLegacyVsThemed",
        &[
            (
                "render_section_header (resolved_list_item_metrics → default_main_menu)",
                format!(
                    "{} slot / {} top padding",
                    list_item_default_metrics.first_section_header_height,
                    list_item_default_metrics.first_section_padding_top
                ),
            ),
            (
                "InfoBarBase themed def (from_main_menu_def)",
                format!(
                    "{} slot / {} top padding",
                    metrics.first_section_header_height, metrics.first_section_padding_top
                ),
            ),
        ],
        "info",
        "render_section_header ignores the themed main-menu metrics and paints the \
         LEGACY default_main_menu() first-section branch: the settings 'Settings' \
         separator occupies a 26px slot with 6px top padding while themed rows paint \
         44px. The global sectionHeader.slotVsLegacy conflict compares the NON-first \
         slots (32 vs 28) and does not capture this. Mockups must consume \
         --sk-list-item-default-first-section-* for this separator, never the \
         --sk-main-menu-first-section-* pair.",
    );
    b.conflict(
        "settingsRows.authoredIconHintsVsResolvedNone",
        &[
            (
                "authored icon hints",
                format!(
                    "{} rows / {} distinct hints ({} rows / {} distinct without {})",
                    settings_facts_custom.authored_icon_hint_rows,
                    settings_facts_custom.distinct_authored_icon_hints,
                    settings_facts_fresh.authored_icon_hint_rows,
                    settings_facts_fresh.distinct_authored_icon_hints,
                    crate::settings_hub_contract::SETTINGS_HUB_OPTIONAL_ROW_NAME,
                ),
            ),
            (
                "resolved by IconKind::from_icon_hint",
                format!("{} row icons", settings_facts_custom.resolved_icon_rows),
            ),
            (
                "painted",
                format!(
                    "icon slot omitted; row text origin {}px (outer {} + inner {})",
                    metrics.row_outer_padding_x + metrics.row_inner_padding_x,
                    metrics.row_outer_padding_x,
                    metrics.row_inner_padding_x
                ),
            ),
        ],
        "warning",
        "get_settings_items authors lucide-style icon hints, but icon_name_from_str \
         recognizes none of them and the ASCII content rejects the emoji fallback, so \
         every settings row paints ICONLESS — a real configuration/renderer mismatch, \
         not an environmental difference. Present-tense conflict: when the parser \
         learns these names (or settings adopts recognized ones), delete this record, \
         add icons to the mockup, shift the 18px name origins, and regenerate the \
         reference receipt in the same change. Locked by \
         settings_hub_contract_behavior::authored_icon_hints_resolve_to_zero_row_icons.",
    );
    b.conflict(
        "settingsFooter.nativeRunVsGpuiOpenHint",
        &[
            (
                "native footer (standard_main_window_footer_buttons)",
                "Run ↵ + Actions ⌘K (default primary label)".to_string(),
            ),
            (
                "GPUI fallback hint strip (render_settings)",
                "↵ Open / Esc Back".to_string(),
            ),
        ],
        "info",
        "Two live code paths advertise different verbs for the same Enter on the \
         settings hub: the native AppKit footer says Run while the in-window GPUI \
         fallback hint strip says Open. Recorded, not collapsed; only a live \
         activeFooter probe determines what the composed surface shows.",
    );

    // ── Shared main-view / component-theme owners (Day Page slice) ──────
    // Per the 2026-07-11 Oracle review: the Day Page mints NO editor, link,
    // caret, color, or footer tokens — it consumes shared owners. These
    // records are the shared side; the Day-owned geometry follows below.
    let columns = crate::components::main_view_chrome::main_view_content_columns(def);
    b.add(
        "resolved.mainView.contentRightInsetX",
        TokenStage::Resolved,
        Some("--sk-main-view-content-right-inset-x"),
        TokenValue::Length {
            value: columns.content_right_inset_x as f64,
        },
        None,
        false,
        &["main_view_content_columns(def).content_right_inset_x = shell.header_padding_x"],
    );
    // gpui-component theme colors every cx.theme() consumer paints with
    // (Day shelf toggle rest/hover, compact resource row rest/hover, …).
    let bridge_theme =
        crate::theme::gpui_integration::map_scriptkit_to_gpui_theme(&theme, theme.is_dark_mode());
    b.add(
        "resolved.componentTheme.mutedForeground",
        TokenStage::Resolved,
        Some("--sk-component-theme-muted-foreground"),
        hsla_color_value(bridge_theme.muted_foreground),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "theme.opacity.textPlaceholder",
            "map_scriptkit_to_gpui_theme → theme_color.muted_foreground",
        ],
    );
    b.add(
        "resolved.componentTheme.foreground",
        TokenStage::Resolved,
        Some("--sk-component-theme-foreground"),
        hsla_color_value(bridge_theme.foreground),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "map_scriptkit_to_gpui_theme → theme_color.foreground",
        ],
    );
    // Shared compact resource row (render_compact_resource_row) — the Day
    // shelf's expanded rows and any future kit:// resource lists share it.
    let compact_row =
        crate::components::resource_preview::resolved_compact_resource_row_style(&theme);
    debug_assert_eq!(compact_row.rest_color, bridge_theme.muted_foreground);
    debug_assert_eq!(compact_row.hover_color, bridge_theme.foreground);
    b.source_len(
        "resourcePreview.compactRow.paddingX",
        "--sk-compact-resource-row-padding-x",
        compact_row.padding_x,
        "components::INFO_SPACING.xs",
    );
    b.source_len(
        "resourcePreview.compactRow.paddingY",
        "--sk-compact-resource-row-padding-y",
        compact_row.padding_y,
        "components::INFO_SPACING.xxs",
    );
    b.add(
        "resolved.resourcePreview.compactRow.gap",
        TokenStage::Resolved,
        Some("--sk-compact-resource-row-gap"),
        TokenValue::Length {
            value: compact_row.gap as f64,
        },
        None,
        false,
        &["gpui Styled::gap_2 (0.5rem × 16px rem) — resource_preview mirror tripwire"],
    );
    // Framework text helpers (gpui `Styled`, rem-relative, no accessor):
    // one shared resolved token each, consumed by the shelf toggle AND the
    // compact row instead of per-surface copies.
    b.add(
        "resolved.framework.textXsFontSize",
        TokenStage::Resolved,
        Some("--sk-framework-text-xs-font-size"),
        TokenValue::Length {
            value: compact_row.font_size as f64,
        },
        None,
        false,
        &["gpui Styled::text_xs (0.75rem × 16px rem) — resource_preview mirror tripwire"],
    );
    b.add(
        "resolved.framework.gap1",
        TokenStage::Resolved,
        Some("--sk-framework-gap-1"),
        TokenValue::Length { value: 4.0 },
        None,
        false,
        &["gpui Styled::gap_1 (0.25rem × 16px rem) — Day shelf toggle glyph/label gap"],
    );

    // ── Day Page (Today view, main window) ──────────────────────────────
    // Anatomy: shared main-view chrome; context-only header = inert context
    // row (30 total, with no phantom input/gap); shared NotesEditor markdown input (adopted
    // 16/12 wrapper + Size::Medium 12/8 → text origin x=30, first text top
    // y=50; mono 16 in the 20px line box); clipboard shelf accessory
    // (6 + 20 [+ 4 + 24·n expanded] + 12); GPUI footer band = empty 32pt
    // rail spacer (the native overlay owns the buttons). Day Page owns ONLY
    // the shelf/accessory geometry below — everything else resolves through
    // the shared owners above.
    b.source_len(
        "dayPage.editor.minHeight",
        "--sk-day-page-editor-min-height",
        crate::day_page::layout::DAY_PAGE_MIN_EDITOR_HEIGHT_PX,
        "day_page::layout::DAY_PAGE_MIN_EDITOR_HEIGHT_PX",
    );
    b.source_len(
        "dayPage.shelf.topPadding",
        "--sk-day-page-shelf-top-padding",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX",
    );
    b.source_len(
        "dayPage.shelf.toggleHeight",
        "--sk-day-page-shelf-toggle-height",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX",
    );
    // Toggle ↔ expanded-list gap. NOT the toggle's inline glyph/label gap
    // (that is the framework .gap_1 — a different authority, also 4 today).
    b.source_len(
        "dayPage.shelf.expandedListGap",
        "--sk-day-page-shelf-expanded-list-gap",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_GAP_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_GAP_PX",
    );
    // The Day renderer's fixed 24px row wrapper (the compact resource row
    // renders inside this slot).
    b.source_len(
        "dayPage.shelf.rowSlotHeight",
        "--sk-day-page-shelf-row-slot-height",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX",
    );
    // Authored responsive cap — layout source, deliberately NO CSS variable.
    b.add(
        "dayPage.shelf.maxBodyFraction",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_MAX_BODY_FRACTION as f64,
        },
        Some("day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_MAX_BODY_FRACTION"),
        true,
        &[],
    );

    // JSON-only Day Page contract facts (markup/behavior, not tokens).
    let day_spine =
        crate::components::notes_editor::spine::NotesEditorHostSpineContract::day_page();
    let day_spine_overlay = match day_spine.local_overlay {
        crate::components::notes_editor::spine::NotesEditorLocalSpineOverlay::Disabled => {
            "disabled"
        }
        crate::components::notes_editor::spine::NotesEditorLocalSpineOverlay::Overlay {
            ..
        } => "overlay",
    };
    let day_spine_mentions = match day_spine.context_mentions {
        crate::components::notes_editor::spine::NotesEditorContextMentionBehavior::MainMenuRoundTrip => {
            "mainMenuRoundTrip"
        }
        crate::components::notes_editor::spine::NotesEditorContextMentionBehavior::Ignore => {
            "ignore"
        }
    };
    for (id, value, path) in [
        (
            "dayPage.header.contextInteraction",
            "inert",
            "render_inert_main_view_context_zone (src/app_impl/ui_window.rs) — same chips, no-op handlers, NO keycaps",
        ),
        (
            "dayPage.header.inputSlot",
            "none",
            "DayPageView::render — MainViewHeaderChrome::context_only",
        ),
        (
            "dayPage.header.dividerVisible",
            "false",
            "DayPageView::render — MainViewDividerChrome { visible: false }",
        ),
        (
            "dayPage.editor.spine.localOverlay",
            day_spine_overlay,
            "NotesEditorHostSpineContract::day_page().local_overlay",
        ),
        (
            "dayPage.editor.spine.contextMentions",
            day_spine_mentions,
            "NotesEditorHostSpineContract::day_page().context_mentions",
        ),
        (
            "dayPage.shelf.defaultExpanded",
            "false",
            "DayPageView::new — clipboard_shelf_expanded: false (collapsed is the shipped rest state)",
        ),
        (
            "dayPage.shelf.hiddenWhenEmpty",
            "true",
            "DayPageView::render_clipboard_shelf — returns None when clipboard_shelf is empty",
        ),
        (
            "dayPage.shelf.hiddenDuringKitPreview",
            "true",
            "DayPageView::render_clipboard_shelf — returns None while kit_resource_preview is open",
        ),
        (
            "dayPage.shelf.sourceLines",
            "liftedFromEditor",
            "adopt_clipboard_shelf_from / day_page::split_day_page_clipboard_shelf (rejoined on save)",
        ),
        (
            "dayPage.footer.presentation",
            "gpuiSpacerPlusNativeOverlay",
            "render_native_main_window_footer_spacer + native AppKit overlay (day_page_footer_buttons)",
        ),
        (
            "dayPage.footer.defaultAction",
            "actions",
            "day_page_footer_buttons — plain Day Page paints a single Actions ⌘K native button",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text {
                value: value.to_string(),
            },
            Some(path),
            false,
            &[],
        );
    }

    // ── Day Page conflicts (recorded, not collapsed) ─────────────────────
    b.conflict(
        "dayPageFooter.spacerVsNativeHostBand",
        &[
            (
                "GPUI Day Page footer spacer (footer.railHeight)",
                format!("{}", fm.height_px),
            ),
            (
                "native footer HOST band (window.nativeFooterHostHeight)",
                format!(
                    "{}",
                    crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT
                ),
            ),
        ],
        "warning",
        "The Day Page GPUI layer reserves the 32px footer rail \
         (render_native_main_window_footer_spacer = current_main_menu_footer_height) \
         while the native AppKit footer HOST band is modeled at 36px. The footer \
         height ladder continues — do NOT 'fix' either value in the exporter; \
         painted truth for the bottom band needs an activeFooter probe + pixel check.",
    );

    // ── Agent Chat (embedded Pi chat surface) ────────────────────────────
    // Production contract only: `style_contract::production_agent_chat_style()`.
    // Every theme-color × authored-alpha byte routes through the SAME
    // `style_contract` resolvers `components/transcript.rs` and `view.rs`
    // paint with, so exporter and renderer literally share bytes.
    use crate::ai::agent_chat::ui::style_contract as agent_chat_contract;
    let chat = agent_chat_contract::production_agent_chat_style();
    let chat_resolved = agent_chat_contract::resolved_agent_chat_transcript_colors(&chat, &theme);
    let chat_send_disabled = agent_chat_contract::resolved_agent_chat_send_state_chrome(
        false,
        false,
        colors.accent.selected,
        colors.text.primary,
    );
    let chat_send_enabled = agent_chat_contract::resolved_agent_chat_send_state_chrome(
        false,
        true,
        colors.accent.selected,
        colors.text.primary,
    );
    let chat_send_queue = agent_chat_contract::resolved_agent_chat_send_state_chrome(
        true,
        true,
        colors.accent.selected,
        colors.text.primary,
    );
    let chat_send_streaming = agent_chat_contract::resolved_agent_chat_send_state_chrome(
        true,
        false,
        colors.accent.selected,
        colors.text.primary,
    );

    // Source geometry (writable leaves; CSS-consumable).
    for (id, var, value, path) in [
        (
            "agentChat.transcript.rowPaddingX",
            "--sk-agent-chat-row-padding-x",
            chat.transcript.row_padding_x,
            "AgentChatTranscriptStyle.row_padding_x",
        ),
        (
            "agentChat.transcript.rowPaddingBottom",
            "--sk-agent-chat-row-padding-bottom",
            chat.transcript.row_padding_bottom,
            "AgentChatTranscriptStyle.row_padding_bottom",
        ),
        (
            "agentChat.transcript.responseStartMarginTop",
            "--sk-agent-chat-response-start-margin-top",
            chat.transcript.response_start_margin_top,
            "AgentChatTranscriptStyle.response_start_margin_top",
        ),
        (
            "agentChat.transcript.turnMarginTop",
            "--sk-agent-chat-turn-margin-top",
            chat.transcript.turn_margin_top,
            "AgentChatTranscriptStyle.turn_margin_top",
        ),
        (
            "agentChat.transcript.turnPaddingTop",
            "--sk-agent-chat-turn-padding-top",
            chat.transcript.turn_padding_top,
            "AgentChatTranscriptStyle.turn_padding_top",
        ),
        (
            "agentChat.markdown.bodyFontSize",
            "--sk-agent-chat-md-body-font-size",
            chat.markdown.body_font_size,
            "AgentChatMarkdownStyle.body_font_size",
        ),
        (
            "agentChat.markdown.h1FontSize",
            "--sk-agent-chat-md-h1-font-size",
            chat.markdown.heading_1_font_size,
            "AgentChatMarkdownStyle.heading_1_font_size",
        ),
        (
            "agentChat.markdown.h2FontSize",
            "--sk-agent-chat-md-h2-font-size",
            chat.markdown.heading_2_font_size,
            "AgentChatMarkdownStyle.heading_2_font_size",
        ),
        (
            "agentChat.markdown.h3FontSize",
            "--sk-agent-chat-md-h3-font-size",
            chat.markdown.heading_3_font_size,
            "AgentChatMarkdownStyle.heading_3_font_size",
        ),
        (
            "agentChat.markdown.codeFontSize",
            "--sk-agent-chat-md-code-font-size",
            chat.markdown.code_block_font_size,
            "AgentChatMarkdownStyle.code_block_font_size",
        ),
        (
            "agentChat.markdown.codePaddingX",
            "--sk-agent-chat-md-code-padding-x",
            chat.markdown.code_block_padding_x,
            "AgentChatMarkdownStyle.code_block_padding_x",
        ),
        (
            "agentChat.markdown.codePaddingY",
            "--sk-agent-chat-md-code-padding-y",
            chat.markdown.code_block_padding_y,
            "AgentChatMarkdownStyle.code_block_padding_y",
        ),
        (
            "agentChat.markdown.codeRadius",
            "--sk-agent-chat-md-code-radius",
            chat.markdown.code_block_radius,
            "AgentChatMarkdownStyle.code_block_radius",
        ),
        (
            "agentChat.markdown.blockquotePaddingX",
            "--sk-agent-chat-md-blockquote-padding-x",
            chat.markdown.blockquote_padding_x,
            "AgentChatMarkdownStyle.blockquote_padding_x",
        ),
        (
            "agentChat.markdown.blockquotePaddingY",
            "--sk-agent-chat-md-blockquote-padding-y",
            chat.markdown.blockquote_padding_y,
            "AgentChatMarkdownStyle.blockquote_padding_y",
        ),
        (
            "agentChat.markdown.blockquoteRadius",
            "--sk-agent-chat-md-blockquote-radius",
            chat.markdown.blockquote_radius,
            "AgentChatMarkdownStyle.blockquote_radius",
        ),
        (
            "agentChat.user.paddingX",
            "--sk-agent-chat-user-padding-x",
            chat.user_message.padding_x,
            "AgentChatMessageStyle(user).padding_x",
        ),
        (
            "agentChat.user.paddingY",
            "--sk-agent-chat-user-padding-y",
            chat.user_message.padding_y,
            "AgentChatMessageStyle(user).padding_y",
        ),
        (
            "agentChat.user.radius",
            "--sk-agent-chat-user-radius",
            chat.user_message.radius,
            "AgentChatMessageStyle(user).radius",
        ),
        (
            "agentChat.assistant.paddingX",
            "--sk-agent-chat-assistant-padding-x",
            chat.assistant_message.padding_x,
            "AgentChatMessageStyle(assistant).padding_x",
        ),
        (
            "agentChat.assistant.paddingY",
            "--sk-agent-chat-assistant-padding-y",
            chat.assistant_message.padding_y,
            "AgentChatMessageStyle(assistant).padding_y",
        ),
        (
            "agentChat.block.paddingX",
            "--sk-agent-chat-block-padding-x",
            chat.collapsible.padding_x,
            "AgentChatCollapsibleStyle.padding_x",
        ),
        (
            "agentChat.block.paddingY",
            "--sk-agent-chat-block-padding-y",
            chat.collapsible.padding_y,
            "AgentChatCollapsibleStyle.padding_y",
        ),
        (
            "agentChat.block.bodyPaddingTop",
            "--sk-agent-chat-block-body-padding-top",
            chat.collapsible.body_padding_top,
            "AgentChatCollapsibleStyle.body_padding_top",
        ),
        (
            "agentChat.block.maxBodyHeight",
            "--sk-agent-chat-block-max-body-height",
            chat.collapsible.max_body_height,
            "AgentChatCollapsibleStyle.max_body_height",
        ),
        (
            "agentChat.block.borderWidth",
            "--sk-agent-chat-block-border-width",
            agent_chat_contract::AGENT_CHAT_BLOCK_BORDER_WIDTH,
            "style_contract::AGENT_CHAT_BLOCK_BORDER_WIDTH",
        ),
        (
            "agentChat.block.headerGap",
            "--sk-agent-chat-block-header-gap",
            agent_chat_contract::AGENT_CHAT_BLOCK_HEADER_GAP,
            "style_contract::AGENT_CHAT_BLOCK_HEADER_GAP",
        ),
        (
            "agentChat.system.paddingX",
            "--sk-agent-chat-system-padding-x",
            chat.system.padding_x,
            "AgentChatSystemStyle.padding_x",
        ),
        (
            "agentChat.system.paddingY",
            "--sk-agent-chat-system-padding-y",
            chat.system.padding_y,
            "AgentChatSystemStyle.padding_y",
        ),
        (
            "agentChat.error.paddingX",
            "--sk-agent-chat-error-padding-x",
            chat.error.padding_x,
            "AgentChatErrorStyle.padding_x",
        ),
        (
            "agentChat.error.paddingY",
            "--sk-agent-chat-error-padding-y",
            chat.error.padding_y,
            "AgentChatErrorStyle.padding_y",
        ),
        (
            "agentChat.error.radius",
            "--sk-agent-chat-error-radius",
            chat.error.radius,
            "AgentChatErrorStyle.radius",
        ),
        (
            "agentChat.send.size",
            "--sk-agent-chat-send-size",
            agent_chat_contract::AGENT_CHAT_SEND_SIZE,
            "style_contract::AGENT_CHAT_SEND_SIZE",
        ),
        (
            "agentChat.send.radius",
            "--sk-agent-chat-send-radius",
            agent_chat_contract::AGENT_CHAT_SEND_RADIUS,
            "style_contract::AGENT_CHAT_SEND_RADIUS",
        ),
    ] {
        b.source_len(id, var, value, path);
    }

    // Embedded Agent Chat aliases the canonical main-menu search typography;
    // these records are resolved/non-writable so there is still one owner.
    b.add(
        "agentChat.composer.fontFamily",
        TokenStage::Resolved,
        None,
        TokenValue::Text {
            value: crate::list_item::FONT_SYSTEM_UI.to_string(),
        },
        None,
        false,
        &["mainMenu.type.uiFontFamily"],
    );
    b.add(
        "agentChat.composer.fontSize",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-font-size"),
        TokenValue::Length {
            value: def.search.font_size as f64,
        },
        None,
        false,
        &["mainMenu.search.fontSize"],
    );
    b.add(
        "agentChat.composer.fontWeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-font-weight"),
        TokenValue::FontWeight {
            value: def.search.font_weight.0 as f64,
        },
        None,
        false,
        &["mainMenu.search.fontWeight"],
    );
    b.add(
        "agentChat.composer.lineHeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-line-height"),
        TokenValue::Length {
            value: def.search.height as f64,
        },
        None,
        false,
        &["mainMenu.search.height"],
    );

    // Source opacities (writable Numbers; CSS-consumable). Thought and tool
    // header opacities stay SEPARATE tokens even while both equal 0.75.
    for (id, var, value, path) in [
        (
            "agentChat.block.thoughtHeaderOpacity",
            "--sk-agent-chat-thought-header-opacity",
            chat.collapsible.thought_header_opacity,
            "AgentChatCollapsibleStyle.thought_header_opacity",
        ),
        (
            "agentChat.block.toolHeaderOpacity",
            "--sk-agent-chat-tool-header-opacity",
            chat.collapsible.tool_header_opacity,
            "AgentChatCollapsibleStyle.tool_header_opacity",
        ),
        (
            "agentChat.block.statusOpacity",
            "--sk-agent-chat-block-status-opacity",
            chat.collapsible.status_opacity,
            "AgentChatCollapsibleStyle.status_opacity",
        ),
        (
            "agentChat.diff.contextOpacity",
            "--sk-agent-chat-diff-context-opacity",
            agent_chat_contract::AGENT_CHAT_DIFF_CONTEXT_OPACITY,
            "style_contract::AGENT_CHAT_DIFF_CONTEXT_OPACITY",
        ),
        (
            "agentChat.system.opacity",
            "--sk-agent-chat-system-opacity",
            chat.system.opacity,
            "AgentChatSystemStyle.opacity",
        ),
        (
            "agentChat.error.labelOpacity",
            "--sk-agent-chat-error-label-opacity",
            chat.error.label_opacity,
            "AgentChatErrorStyle.label_opacity",
        ),
        (
            "agentChat.error.hintOpacity",
            "--sk-agent-chat-error-hint-opacity",
            chat.error.hint_opacity,
            "AgentChatErrorStyle.hint_opacity",
        ),
        (
            "agentChat.send.disabledOpacity",
            "--sk-agent-chat-send-disabled-opacity",
            agent_chat_contract::AGENT_CHAT_SEND_DISABLED_OPACITY,
            "style_contract::AGENT_CHAT_SEND_DISABLED_OPACITY",
        ),
        (
            "agentChat.send.enabledOpacity",
            "--sk-agent-chat-send-enabled-opacity",
            agent_chat_contract::AGENT_CHAT_SEND_ENABLED_OPACITY,
            "style_contract::AGENT_CHAT_SEND_ENABLED_OPACITY",
        ),
        (
            "agentChat.send.queueOpacity",
            "--sk-agent-chat-send-queue-opacity",
            agent_chat_contract::AGENT_CHAT_SEND_QUEUE_OPACITY,
            "style_contract::AGENT_CHAT_SEND_QUEUE_OPACITY",
        ),
        (
            "agentChat.send.streamingOpacity",
            "--sk-agent-chat-send-streaming-opacity",
            agent_chat_contract::AGENT_CHAT_SEND_STREAMING_OPACITY,
            "style_contract::AGENT_CHAT_SEND_STREAMING_OPACITY",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            Some(var),
            TokenValue::Number {
                value: value as f64,
            },
            Some(path),
            true,
            &[],
        );
    }

    // Authored alpha leaves (writable source records; JSON-only — the HTML
    // consumes the resolved final colors, but the app-authored byte must not
    // disappear inside derived_from).
    for (id, value, path) in [
        (
            "agentChat.transcript.turnDividerAlpha",
            chat.transcript.turn_divider_alpha,
            "AgentChatTranscriptStyle.turn_divider_alpha (0x18)",
        ),
        (
            "agentChat.markdown.codeBgAlpha",
            chat.markdown.code_block_bg_alpha,
            "AgentChatMarkdownStyle.code_block_bg_alpha (0xA0)",
        ),
        (
            "agentChat.markdown.codeBorderAlpha",
            chat.markdown.code_block_border_alpha,
            "AgentChatMarkdownStyle.code_block_border_alpha (0x40)",
        ),
        (
            "agentChat.markdown.blockquoteBgAlpha",
            chat.markdown.blockquote_bg_alpha,
            "AgentChatMarkdownStyle.blockquote_bg_alpha (0x10)",
        ),
        (
            "agentChat.markdown.blockquoteBorderAlpha",
            chat.markdown.blockquote_border_alpha,
            "AgentChatMarkdownStyle.blockquote_border_alpha (0x40)",
        ),
        (
            "agentChat.user.bgAlpha",
            chat.user_message.bg_alpha,
            "AgentChatMessageStyle(user).bg_alpha (0x06)",
        ),
        (
            "agentChat.block.thoughtBorderAlpha",
            chat.collapsible.thought_border_alpha,
            "AgentChatCollapsibleStyle.thought_border_alpha (0x7F)",
        ),
        (
            "agentChat.block.toolBorderAlpha",
            chat.collapsible.tool_border_alpha,
            "AgentChatCollapsibleStyle.tool_border_alpha (0x7F)",
        ),
        (
            "agentChat.tool.statusPendingAlpha",
            agent_chat_contract::AGENT_CHAT_TOOL_STATUS_PENDING_ALPHA,
            "style_contract::AGENT_CHAT_TOOL_STATUS_PENDING_ALPHA (0x80)",
        ),
        (
            "agentChat.diff.tintAlpha",
            agent_chat_contract::AGENT_CHAT_DIFF_TINT_ALPHA,
            "style_contract::AGENT_CHAT_DIFF_TINT_ALPHA (0x14)",
        ),
        (
            "agentChat.system.borderAlpha",
            chat.system.border_alpha,
            "AgentChatSystemStyle.border_alpha (0x30)",
        ),
        (
            "agentChat.error.bgAlpha",
            chat.error.bg_alpha,
            "AgentChatErrorStyle.bg_alpha — authored DECIMAL 50 (= 0x32); see agentChat.error.bgAlphaUnits",
        ),
        (
            "agentChat.error.borderAlpha",
            chat.error.border_alpha,
            "AgentChatErrorStyle.border_alpha (0x80)",
        ),
        (
            "agentChat.send.disabledBgAlpha",
            agent_chat_contract::AGENT_CHAT_SEND_DISABLED_BG_ALPHA,
            "style_contract::AGENT_CHAT_SEND_DISABLED_BG_ALPHA (0x06)",
        ),
        (
            "agentChat.send.enabledBgAlpha",
            agent_chat_contract::AGENT_CHAT_SEND_ENABLED_BG_ALPHA,
            "style_contract::AGENT_CHAT_SEND_ENABLED_BG_ALPHA (0x30)",
        ),
        (
            "agentChat.send.queueBgAlpha",
            agent_chat_contract::AGENT_CHAT_SEND_QUEUE_BG_ALPHA,
            "style_contract::AGENT_CHAT_SEND_QUEUE_BG_ALPHA (0x24)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number {
                value: value as f64,
            },
            Some(path),
            true,
            &[],
        );
    }
    // paragraph_gap is authored in REMS (framework-relative), not px — a
    // Number source record with no CSS variable; the mockup's rem
    // conversion is emulator calibration.
    b.add(
        "agentChat.markdown.paragraphGapRems",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: chat.markdown.paragraph_gap as f64,
        },
        Some("AgentChatMarkdownStyle.paragraph_gap (rems scalar)"),
        true,
        &[],
    );
    // Composer paddings: app-authored, but the shell height derives from
    // the shared search height + line growth, so these are JSON-only (the
    // Y padding feeds picker-lane math, the X padding measurement lanes).
    for (id, value, path) in [
        (
            "agentChat.composer.paddingX",
            agent_chat_contract::AGENT_CHAT_INPUT_PADDING_X,
            "style_contract::AGENT_CHAT_INPUT_PADDING_X (picker clamping/measurement; not shell geometry)",
        ),
        (
            "agentChat.composer.paddingY",
            agent_chat_contract::AGENT_CHAT_INPUT_PADDING_Y,
            "style_contract::AGENT_CHAT_INPUT_PADDING_Y (picker lane positioning; not shell height)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Length {
                value: value as f64,
            },
            Some(path),
            true,
            &[],
        );
    }

    // Resolved paint (never writable) — the SAME resolver bytes the
    // transcript renderer paints.
    b.resolved_color(
        "resolved.agentChat.transcript.turnDivider",
        "--sk-agent-chat-turn-divider",
        chat_resolved.turn_divider_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.transcript.turnDividerAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.codeBg",
        "--sk-agent-chat-md-code-bg",
        chat_resolved.code_bg_rgba,
        &[
            "theme.colors.background.searchBox",
            "agentChat.markdown.codeBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.codeBorder",
        "--sk-agent-chat-md-code-border",
        chat_resolved.code_border_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.markdown.codeBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.blockquoteBg",
        "--sk-agent-chat-md-blockquote-bg",
        chat_resolved.blockquote_bg_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.markdown.blockquoteBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.blockquoteBorder",
        "--sk-agent-chat-md-blockquote-border",
        chat_resolved.blockquote_border_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.markdown.blockquoteBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.user.bg",
        "--sk-agent-chat-user-bg",
        chat_resolved.user_bg_rgba,
        &["theme.colors.text.primary", "agentChat.user.bgAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.thought.border",
        "--sk-agent-chat-thought-border",
        chat_resolved.thought_border_rgba,
        &[
            "theme.colors.text.primary",
            "agentChat.block.thoughtBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.tool.border",
        "--sk-agent-chat-tool-border",
        chat_resolved.tool_border_rgba,
        &[
            "theme.colors.accent.selected",
            "agentChat.block.toolBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.tool.borderError",
        "--sk-agent-chat-tool-border-error",
        chat_resolved.tool_border_error_rgba,
        &["theme.colors.ui.error", "agentChat.block.toolBorderAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.tool.statusPending",
        "--sk-agent-chat-tool-status-pending",
        chat_resolved.tool_status_pending_rgba,
        &[
            "theme.colors.text.primary",
            "agentChat.tool.statusPendingAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.tool.statusComplete",
        "--sk-agent-chat-tool-status-complete",
        chat_resolved.tool_status_complete_rgba,
        &["theme.colors.ui.success"],
    );
    b.resolved_color(
        "resolved.agentChat.tool.statusFailed",
        "--sk-agent-chat-tool-status-failed",
        chat_resolved.tool_status_failed_rgba,
        &["theme.colors.ui.error"],
    );
    b.resolved_color(
        "resolved.agentChat.diff.addedBg",
        "--sk-agent-chat-diff-added-bg",
        chat_resolved.diff_added_bg_rgba,
        &["theme.colors.ui.success", "agentChat.diff.tintAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.diff.removedBg",
        "--sk-agent-chat-diff-removed-bg",
        chat_resolved.diff_removed_bg_rgba,
        &["theme.colors.ui.error", "agentChat.diff.tintAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.system.border",
        "--sk-agent-chat-system-border",
        chat_resolved.system_border_rgba,
        &["theme.colors.ui.border", "agentChat.system.borderAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.error.bg",
        "--sk-agent-chat-error-bg",
        chat_resolved.error_bg_rgba,
        &["theme.colors.ui.error", "agentChat.error.bgAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.error.border",
        "--sk-agent-chat-error-border",
        chat_resolved.error_border_rgba,
        &["theme.colors.ui.error", "agentChat.error.borderAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.send.disabledBg",
        "--sk-agent-chat-send-disabled-bg",
        chat_send_disabled.bg_rgba,
        &[
            "theme.colors.text.primary",
            "agentChat.send.disabledBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.send.enabledBg",
        "--sk-agent-chat-send-enabled-bg",
        chat_send_enabled.bg_rgba,
        &[
            "theme.colors.accent.selected",
            "agentChat.send.enabledBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.send.queueBg",
        "--sk-agent-chat-send-queue-bg",
        chat_send_queue.bg_rgba,
        &[
            "theme.colors.accent.selected",
            "agentChat.send.queueBgAlpha",
        ],
    );
    debug_assert_eq!(chat_send_streaming.bg_rgba, 0x0000_0000);

    // Markdown body line box: renderer never sets a line height; GPUI's
    // implicit phi() default applies — resolved through the shared app-side
    // helper (confirm_prompt_line_height_px), never a fresh 1.618034
    // literal here.
    b.add(
        "resolved.agentChat.markdown.bodyLineHeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-md-body-line-height"),
        TokenValue::Length {
            value: agent_chat_contract::resolved_agent_chat_markdown_body_line_height(&chat) as f64,
        },
        None,
        false,
        &[
            "agentChat.markdown.bodyFontSize",
            "gpui TextStyle default phi() line height, rounded",
        ],
    );
    // Single-line composer shell height: the shared main-menu search height
    // grows by one composer line per extra visible line (shared formula
    // owner `main_view_multiline_input_height`). Fixture-resolved — NOT a
    // universal composer height (multiline/expanded composers are taller).
    b.add(
        "resolved.agentChat.composer.singleLineHeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-single-line-height"),
        TokenValue::Length {
            value: agent_chat_contract::resolved_agent_chat_composer_single_line_height(
                def.search.height,
            ) as f64,
        },
        None,
        false,
        &["mainMenu.search.height", "agentChat.composer.lineHeight"],
    );
    // Send glyph typography: production uses gpui `text_sm` (a framework
    // authority — NOT the markdown body size, which merely coincides at 14).
    b.add(
        "resolved.framework.textSmFontSize",
        TokenStage::Resolved,
        Some("--sk-framework-text-sm-font-size"),
        TokenValue::Length { value: 14.0 },
        None,
        false,
        &["gpui Styled::text_sm (0.875rem × 16px rem) — send glyph typography"],
    );

    // JSON-only Agent Chat facts (no CSS role, never writable).
    for (id, value, path) in [
        (
            "agentChat.composer.placeholderEmpty",
            agent_chat_contract::AGENT_CHAT_PLACEHOLDER_ASK.to_string(),
            "style_contract::AGENT_CHAT_PLACEHOLDER_ASK",
        ),
        (
            "agentChat.composer.placeholderFollowUp",
            agent_chat_contract::AGENT_CHAT_PLACEHOLDER_FOLLOW_UP.to_string(),
            "style_contract::AGENT_CHAT_PLACEHOLDER_FOLLOW_UP (non-empty transcript state)",
        ),
        (
            "agentChat.legacyComposer.fontFamily",
            agent_chat_contract::AGENT_CHAT_INPUT_FONT_FAMILY.to_string(),
            "style_contract::AGENT_CHAT_INPUT_FONT_FAMILY — detached/experimental Agent Chat and Focused Text Mini only",
        ),
        (
            "agentChat.transcript.alignment",
            "bottomFollowTailWithSyntheticActivityTail".to_string(),
            "AgentChatTranscript::new — ListState::new(len+1, ListAlignment::Bottom).measure_all() + follow_tail(true)",
        ),
        (
            "agentChat.footer.presentation",
            "gpuiSpacerPlusNativeOverlay".to_string(),
            "render_native_main_window_footer_spacer for surface \"agent_chat\" — the GPUI band is EMPTY in captures; button truth needs an activeFooter probe",
        ),
        (
            "agentChat.tool.defaultExpansion",
            "collapsedExceptDiffOrError".to_string(),
            "AgentChatTranscript::default_expanded — tools with a diff or is_error start expanded",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text { value },
            Some(path),
            false,
            &[],
        );
    }
    // Variant-limited / ineffective-in-Standard numbers (JSON-only, not
    // writable through the token reverse path; see the
    // agentChat.standard.roleSplitOnlyFields conflict).
    for (id, value, path) in [
        (
            "agentChat.user.maxWidthRoleSplitOnly",
            chat.user_message.max_width as f64,
            "AgentChatMessageStyle(user).max_width — applied ONLY under RoleSplit presentation",
        ),
        (
            "agentChat.assistant.maxWidthRoleSplitOnly",
            chat.assistant_message.max_width as f64,
            "AgentChatMessageStyle(assistant).max_width — applied ONLY under RoleSplit presentation",
        ),
        (
            "agentChat.assistant.radius",
            chat.assistant_message.radius as f64,
            "AgentChatMessageStyle(assistant).radius — 0; assistant bg only paints when bg_alpha > 0",
        ),
        (
            "agentChat.assistant.bgAlpha",
            chat.assistant_message.bg_alpha as f64,
            "AgentChatMessageStyle(assistant).bg_alpha — 0: no assistant surface painted in Standard",
        ),
        (
            "agentChat.activity.dotSize",
            agent_chat_contract::AGENT_CHAT_ACTIVITY_DOT_SIZE as f64,
            "style_contract::AGENT_CHAT_ACTIVITY_DOT_SIZE — hidden (0px row) in the idle fixture",
        ),
        (
            "agentChat.activity.gap",
            agent_chat_contract::AGENT_CHAT_ACTIVITY_GAP as f64,
            "style_contract::AGENT_CHAT_ACTIVITY_GAP",
        ),
        (
            "agentChat.activity.labelAlpha",
            agent_chat_contract::AGENT_CHAT_ACTIVITY_LABEL_ALPHA as f64,
            "style_contract::AGENT_CHAT_ACTIVITY_LABEL_ALPHA (0xB0)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number { value },
            Some(path),
            false,
            &[],
        );
    }

    // ── Agent Chat conflicts (recorded, not collapsed) ──────────────────
    b.conflict(
        "agentChat.error.bgAlphaUnits",
        &[
            (
                "AgentChatErrorStyle.bg_alpha",
                format!("{} (DECIMAL — 0x32)", chat.error.bg_alpha),
            ),
            (
                "sibling alphas",
                "hex-authored bytes (0x18, 0xA0, 0x7F, 0x80, …)".to_string(),
            ),
        ],
        "info",
        "The error background alpha is authored as decimal 50 while every sibling \
         alpha is hex-authored — a real edit foot-gun. Recorded, not normalized; the \
         shared pack_rgb_alpha resolver rounds it to 0x32 either way.",
    );
    b.conflict(
        "agentChat.standard.roleSplitOnlyFields",
        &[
            (
                "declared",
                format!(
                    "user.max_width {} / assistant.max_width {} / assistant.radius {} / assistant.bg_alpha {}",
                    chat.user_message.max_width,
                    chat.assistant_message.max_width,
                    chat.assistant_message.radius,
                    chat.assistant_message.bg_alpha
                ),
            ),
            (
                "Standard presentation",
                "full-width rows; max_width applies only under RoleSplit, assistant bg only \
                 when bg_alpha > 0"
                    .to_string(),
            ),
        ],
        "info",
        "Real source controls that are variant-limited: exported as JSON-only facts with \
         no CSS variable so the Standard mockup cannot consume phantom geometry, and the \
         workbench cannot advertise edits that paint nothing on this screen.",
    );

    // ── Chat prompt (SDK chat surface, AppView::ChatPrompt) ─────────────
    // Geometry/colors the HTML mockup previously staged under
    // --sk-emulator-staged-chat-*. Rows of constants live next to the
    // renderer (prompts/chat/mod.rs); resolved colors pack the same theme
    // bytes production paints.
    {
        use crate::prompts::chat::{
            CHAT_LAYOUT_BORDER_ALPHA, CHAT_LAYOUT_CARD_PADDING_X, CHAT_LAYOUT_CARD_PADDING_Y,
            CHAT_LAYOUT_PADDING_X, CHAT_LAYOUT_SECTION_PADDING_Y,
        };

        b.source_len(
            "chatPrompt.layout.paddingX",
            "--sk-chat-layout-padding-x",
            CHAT_LAYOUT_PADDING_X,
            "prompts::chat::CHAT_LAYOUT_PADDING_X",
        );
        b.source_len(
            "chatPrompt.layout.sectionPaddingY",
            "--sk-chat-section-padding-y",
            CHAT_LAYOUT_SECTION_PADDING_Y,
            "prompts::chat::CHAT_LAYOUT_SECTION_PADDING_Y",
        );
        b.source_len(
            "chatPrompt.header.gap",
            "--sk-chat-header-gap",
            8.0,
            "prompts::chat::render_input::render_header gap",
        );
        b.source_len(
            "chatPrompt.title.fontSize",
            "--sk-chat-title-font-size",
            14.0,
            "gpui text_sm (0.875rem)",
        );
        b.add(
            "resolved.chatPrompt.title.lineHeight",
            TokenStage::Resolved,
            Some("--sk-chat-title-line-height"),
            TokenValue::Length { value: 23.0 },
            None,
            false,
            &[
                "chatPrompt.title.fontSize",
                "gpui phi() rounded line height",
            ],
        );
        b.add(
            "chatPrompt.title.fontWeight",
            TokenStage::Source,
            Some("--sk-chat-title-font-weight"),
            TokenValue::FontWeight { value: 500.0 },
            Some("FontWeight::MEDIUM"),
            true,
            &[],
        );
        b.resolved_color(
            "resolved.chatPrompt.divider",
            "--sk-chat-divider",
            (theme.colors.ui.border << 8) | CHAT_LAYOUT_BORDER_ALPHA,
            &[
                "theme.colors.ui.border",
                "prompts::chat::CHAT_LAYOUT_BORDER_ALPHA",
            ],
        );
        b.resolved_color(
            "resolved.chatPrompt.text.secondary",
            "--sk-chat-text-secondary",
            (theme.colors.text.secondary << 8) | 0xFF,
            &["theme.colors.text.secondary"],
        );
        b.source_len(
            "chatPrompt.input.minHeight",
            "--sk-chat-input-min-height",
            28.0,
            "prompts::chat::render_input field min_h",
        );
        b.source_len(
            "chatPrompt.input.fontSize",
            "--sk-chat-input-font-size",
            14.0,
            "prompts::chat::render_input input_font_size (full mode)",
        );
        // search_box fill at search_box opacity — same ladder the confirm input uses.
        let search_box = theme.colors.background.search_box;
        let search_alpha = (opacity.search_box.clamp(0.0, 1.0) * 255.0).round() as u32;
        b.resolved_color(
            "resolved.chatPrompt.input.surface",
            "--sk-chat-input-surface",
            (search_box << 8) | search_alpha,
            &[
                "theme.colors.background.search_box",
                "theme.opacity.search_box",
            ],
        );
        b.resolved_color(
            "resolved.chatPrompt.input.placeholder",
            "--sk-chat-placeholder-text",
            (theme.colors.text.muted << 8) | 0xFF,
            &["theme.colors.text.muted"],
        );
        b.source_len(
            "chatPrompt.turn.gap",
            "--sk-chat-turn-gap",
            8.0,
            "prompts::chat::render_core turns pb",
        );
        b.source_len(
            "chatPrompt.card.paddingX",
            "--sk-chat-card-padding-x",
            CHAT_LAYOUT_CARD_PADDING_X,
            "prompts::chat::CHAT_LAYOUT_CARD_PADDING_X",
        );
        b.source_len(
            "chatPrompt.card.paddingY",
            "--sk-chat-card-padding-y",
            CHAT_LAYOUT_CARD_PADDING_Y,
            "prompts::chat::CHAT_LAYOUT_CARD_PADDING_Y",
        );
        b.source_len(
            "chatPrompt.card.radius",
            "--sk-chat-card-radius",
            8.0,
            "prompts::chat::render_turns card rounded",
        );
        // Dark-mode turn card: text.primary @ 0x15 (≈8.2%).
        b.resolved_color(
            "resolved.chatPrompt.card.background",
            "--sk-chat-card-background",
            (theme.colors.text.primary << 8) | 0x15,
            &[
                "theme.colors.text.primary",
                "prompts::chat::TURN_CARD_OVERLAY_ALPHA_DARK (0x15)",
            ],
        );
        b.source_len(
            "chatPrompt.card.rowGap",
            "--sk-chat-card-row-gap",
            8.0,
            "prompts::chat::render_turns card gap",
        );
        b.source_len(
            "chatPrompt.card.contentGap",
            "--sk-chat-card-content-gap",
            6.0,
            "prompts::chat::render_turns content gap",
        );
        b.add(
            "chatPrompt.user.fontWeight",
            TokenStage::Source,
            Some("--sk-chat-user-font-weight"),
            TokenValue::FontWeight { value: 600.0 },
            Some("FontWeight::SEMIBOLD"),
            true,
            &[],
        );
        b.source_len(
            "chatPrompt.copy.buttonSize",
            "--sk-chat-copy-button-size",
            24.0,
            "prompts::chat::render_turns copy button w/h",
        );
        b.source_len(
            "chatPrompt.copy.buttonRadius",
            "--sk-chat-copy-button-radius",
            4.0,
            "prompts::chat::render_turns copy rounded",
        );
        b.source_len(
            "chatPrompt.copy.iconSize",
            "--sk-chat-copy-icon-size",
            16.0,
            "prompts::chat::render_turns copy svg size",
        );
        b.add(
            "chatPrompt.copy.opacity",
            TokenStage::Source,
            Some("--sk-chat-copy-opacity"),
            TokenValue::Number { value: 0.7 },
            Some("prompts::chat::render_turns copy opacity"),
            true,
            &[],
        );
        b.source_len(
            "chatPrompt.md.blockGap",
            "--sk-chat-md-block-gap",
            6.0,
            "prompts::markdown::api root gap",
        );
        b.source_len(
            "chatPrompt.md.fontSize",
            "--sk-chat-md-font-size",
            14.0,
            "prompts::markdown text_sm",
        );
        b.add(
            "resolved.chatPrompt.md.lineHeight",
            TokenStage::Resolved,
            Some("--sk-chat-md-line-height"),
            TokenValue::Length { value: 23.0 },
            None,
            false,
            &["chatPrompt.md.fontSize", "gpui phi() rounded line height"],
        );
        b.source_len(
            "chatPrompt.code.marginY",
            "--sk-chat-code-margin-y",
            4.0,
            "prompts::markdown::code_table container mt/mb",
        );
        b.source_len(
            "chatPrompt.code.radius",
            "--sk-chat-code-radius",
            6.0,
            "prompts::markdown::code_table container rounded",
        );
        b.resolved_color(
            "resolved.chatPrompt.code.background",
            "--sk-chat-code-background",
            (search_box << 8) | 0xE0,
            &[
                "theme.colors.background.search_box",
                "markdown code alpha 0xE0",
            ],
        );
        b.resolved_color(
            "resolved.chatPrompt.code.headerBorder",
            "--sk-chat-code-header-border",
            (theme.colors.ui.border << 8) | 0x30,
            &["theme.colors.ui.border", "markdown code header alpha 0x30"],
        );
        b.source_len(
            "chatPrompt.code.headerPaddingX",
            "--sk-chat-code-header-padding-x",
            10.0,
            "prompts::markdown::code_table header px",
        );
        b.source_len(
            "chatPrompt.code.headerPaddingY",
            "--sk-chat-code-header-padding-y",
            4.0,
            "prompts::markdown::code_table header py",
        );
        b.source_len(
            "chatPrompt.code.labelFontSize",
            "--sk-chat-code-label-font-size",
            12.0,
            "gpui text_xs",
        );
        b.source_len(
            "chatPrompt.code.bodyPaddingX",
            "--sk-chat-code-body-padding-x",
            10.0,
            "prompts::markdown::code_table body px",
        );
        b.source_len(
            "chatPrompt.code.bodyPaddingY",
            "--sk-chat-code-body-padding-y",
            8.0,
            "prompts::markdown::code_table body py",
        );
        b.source_len(
            "chatPrompt.code.lineGap",
            "--sk-chat-code-line-gap",
            2.0,
            "prompts::markdown::code_table line gap",
        );
        b.source_len(
            "chatPrompt.code.lineMinHeight",
            "--sk-chat-code-line-min-height",
            16.0,
            "prompts::markdown::code_table line min-h",
        );
        b.source_len(
            "chatPrompt.inlineCode.paddingX",
            "--sk-chat-inline-code-padding-x",
            4.0,
            "prompts::markdown inline code px",
        );
        b.source_len(
            "chatPrompt.inlineCode.paddingY",
            "--sk-chat-inline-code-padding-y",
            1.0,
            "prompts::markdown inline code py",
        );
        b.source_len(
            "chatPrompt.inlineCode.radius",
            "--sk-chat-inline-code-radius",
            3.0,
            "prompts::markdown inline code radius",
        );
        b.resolved_color(
            "resolved.chatPrompt.inlineCode.background",
            "--sk-chat-inline-code-background",
            (search_box << 8) | search_alpha,
            &[
                "theme.colors.background.search_box",
                "theme.opacity.search_box",
            ],
        );
        b.add(
            "resolved.chatPrompt.shell.fixtureHeight",
            TokenStage::Resolved,
            Some("--sk-chat-window-height"),
            TokenValue::Length {
                value: f32::from(crate::window_resize::layout::STANDARD_HEIGHT) as f64,
            },
            None,
            false,
            &["window_resize::layout::STANDARD_HEIGHT (DivPrompt/chat full)"],
        );
        b.source_len(
            "chatPrompt.inputArea.gap",
            "--sk-chat-input-area-gap",
            8.0,
            "prompts::chat::render_input input area gap",
        );
    }

    // ── Terminal prompt (SDK term surface, AppView::TermPrompt) ─────────
    {
        use crate::config::defaults::{DEFAULT_LAYOUT_MAX_HEIGHT, DEFAULT_TERMINAL_FONT_SIZE};
        use crate::term_prompt::{LINE_HEIGHT_MULTIPLIER, MEASURED_CELL_WIDTH_AT_14};
        use crate::window_resize::layout::FOOTER_HEIGHT;

        let term_font = DEFAULT_TERMINAL_FONT_SIZE; // 14
        let cell_height = term_font * LINE_HEIGHT_MULTIPLIER; // 18.2
        let entity_height = DEFAULT_LAYOUT_MAX_HEIGHT - FOOTER_HEIGHT; // 670
        let pad_top = crate::config::defaults::DEFAULT_PADDING_TOP;
        let pad_left = crate::config::defaults::DEFAULT_PADDING_LEFT;
        let pad_right = crate::config::defaults::DEFAULT_PADDING_RIGHT;

        b.source_len(
            "terminalPrompt.shell.height",
            "--sk-terminal-window-height",
            DEFAULT_LAYOUT_MAX_HEIGHT,
            "config::defaults::DEFAULT_LAYOUT_MAX_HEIGHT",
        );
        b.source_len(
            "terminalPrompt.entity.height",
            "--sk-terminal-entity-height",
            entity_height,
            "MAX_HEIGHT − window_resize::layout::FOOTER_HEIGHT",
        );
        b.source_len(
            "terminalPrompt.font.size",
            "--sk-terminal-font-size",
            term_font,
            "config::defaults::DEFAULT_TERMINAL_FONT_SIZE",
        );
        b.source_len(
            "terminalPrompt.cell.width",
            "--sk-terminal-cell-width",
            MEASURED_CELL_WIDTH_AT_14,
            "term_prompt::MEASURED_CELL_WIDTH_AT_14 (conservative_cell_width(8.4287))",
        );
        b.source_len(
            "terminalPrompt.cell.height",
            "--sk-terminal-cell-height",
            cell_height,
            "DEFAULT_TERMINAL_FONT_SIZE × term_prompt::LINE_HEIGHT_MULTIPLIER",
        );
        b.source_len(
            "terminalPrompt.padding.top",
            "--sk-terminal-padding-top",
            pad_top,
            "config::defaults::DEFAULT_PADDING_TOP",
        );
        b.source_len(
            "terminalPrompt.padding.bottom",
            "--sk-terminal-padding-bottom",
            pad_top, // effective_padding_bottom = top for SDK terminals
            "term_prompt::effective_padding bottom = top",
        );
        b.source_len(
            "terminalPrompt.padding.left",
            "--sk-terminal-padding-left",
            pad_left,
            "config::defaults::DEFAULT_PADDING_LEFT",
        );
        b.source_len(
            "terminalPrompt.padding.right",
            "--sk-terminal-padding-right",
            pad_right,
            "config::defaults::DEFAULT_PADDING_RIGHT",
        );
        b.resolved_color(
            "resolved.terminalPrompt.cursor.background",
            "--sk-terminal-cursor-background",
            (theme.colors.accent.selected << 8) | 0xFF,
            &["theme.colors.accent.selected"],
        );
        b.resolved_color(
            "resolved.terminalPrompt.selection.background",
            "--sk-terminal-selection-background",
            (theme.colors.accent.selected_subtle << 8) | 0x0F,
            &[
                "theme.colors.accent.selected_subtle",
                "selection alpha 0x0f",
            ],
        );
        b.resolved_color(
            "resolved.terminalPrompt.ansi.green",
            "--sk-terminal-ansi-green",
            (theme.colors.terminal.green << 8) | 0xFF,
            &["theme.colors.terminal.green"],
        );
        // Hint strip geometry reuses the universal chrome constants.
        b.source_len(
            "terminalPrompt.hint.height",
            "--sk-terminal-hint-height",
            crate::window_resize::main_layout::HINT_STRIP_HEIGHT,
            "window_resize::main_layout::HINT_STRIP_HEIGHT",
        );
        b.source_len(
            "terminalPrompt.hint.paddingX",
            "--sk-terminal-hint-padding-x",
            14.0,
            "HINT_STRIP_PADDING_X",
        );
        b.source_len(
            "terminalPrompt.hint.paddingY",
            "--sk-terminal-hint-padding-y",
            8.0,
            "HINT_STRIP_PADDING_Y",
        );
        b.source_len(
            "terminalPrompt.hint.gap",
            "--sk-terminal-hint-gap",
            8.0,
            "HINT_STRIP_CONTENT_GAP",
        );
        b.source_len(
            "terminalPrompt.hint.textSize",
            "--sk-terminal-hint-text-size",
            12.5,
            "FOOTER_HINT_TEXT_SIZE",
        );
        b.source_len(
            "terminalPrompt.hint.iconSize",
            "--sk-terminal-hint-icon-size",
            14.0,
            "KEY_ICON_SIZE",
        );
        b.source_len(
            "terminalPrompt.hint.iconGap",
            "--sk-terminal-hint-icon-gap",
            3.0,
            "KEY_ICON_LABEL_GAP",
        );
        b.source_len(
            "terminalPrompt.hint.keycapPaddingX",
            "--sk-terminal-hint-keycap-padding-x",
            6.0,
            "KEYCAP_PADDING_X",
        );
        b.source_len(
            "terminalPrompt.hint.keycapPaddingY",
            "--sk-terminal-hint-keycap-padding-y",
            1.0,
            "KEYCAP_PADDING_Y",
        );
        b.source_len(
            "terminalPrompt.hint.keycapRadius",
            "--sk-terminal-hint-keycap-radius",
            5.0,
            "KEYCAP_RADIUS",
        );
        b.resolved_color(
            "resolved.terminalPrompt.hint.keycapBackground",
            "--sk-terminal-hint-keycap-background",
            (theme.colors.text.primary << 8) | 0x1F, // ≈0.12
            &["theme.colors.text.primary", "KEYCAP_BG_OPACITY ~0.12"],
        );
    }

    // ── Arg prompt (script-driven choices/input, AppView::ArgPrompt) ────
    // Rows intentionally emit NO new row tokens — they are the shared
    // ListItem with no metrics override, so --sk-main-menu-row-* /
    // --sk-main-menu-name-* are the row contract. Only shell/input/footer
    // slot values are arg-specific (briefs/arg-prompt-brief.md).
    {
        use crate::panel::{
            CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y, HEADER_DIVIDER_MARGIN, HEADER_PADDING_X,
            HEADER_PADDING_Y,
        };
        use crate::window_resize::{height_for_view, ViewType};

        let arg_input_height = CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0); // 22
                                                                           // Painted input line box tracks the gpui-component Input font
                                                                           // (Size::Size(font_size_xl) → line box = 1.25 × font), NOT the
                                                                           // CURSOR_HEIGHT_LG model. Runtime capture 2026-07-12: input band
                                                                           // spans 41px (= 8 + 25 + 8) between context zone and divider.
        let arg_input_line_height =
            crate::theme::TypographyResolver::new(&theme, crate::designs::DesignVariant::Default)
                .font_size_xl()
                * 1.25; // 25
        let arg_header_height = (HEADER_PADDING_Y * 2.0) + arg_input_line_height; // 41
        let arg_header_model_height = (HEADER_PADDING_Y * 2.0) + arg_input_height; // 38
                                                                                   // Vendor gpui-component caret: width 2px, height 0.85 × line height.
                                                                                   // For Size::Size(px(20)) font the line box tracks the font size.
        let arg_caret_height = 0.85 * 20.0; // 17
                                            // The launcher opens in MINI mode by default, so a script-driven arg
                                            // prompt sizes through ViewType::MiniPrompt (5-visible-row clamp),
                                            // NOT ArgPromptWithChoices. Runtime window capture 2026-07-12: 279.
        let arg_fixture_height = f32::from(height_for_view(ViewType::MiniPrompt, 6)); // 279
        let arg_full_mode_height = f32::from(height_for_view(ViewType::ArgPromptWithChoices, 6)); // 319
        let arg_footer_spacer = crate::components::footer_chrome::current_main_menu_footer_height(); // 32

        b.source_len(
            "argPrompt.header.paddingX",
            "--sk-arg-header-padding-x",
            HEADER_PADDING_X,
            "crate::panel::HEADER_PADDING_X",
        );
        b.add(
            "resolved.argPrompt.header.height",
            TokenStage::Resolved,
            Some("--sk-arg-header-height"),
            TokenValue::Length {
                value: arg_header_height as f64,
            },
            None,
            false,
            &["argPrompt.input.lineHeight", "panel.HEADER_PADDING_Y"],
        );
        b.add(
            "resolved.argPrompt.input.lineHeight",
            TokenStage::Resolved,
            Some("--sk-arg-input-line-height"),
            TokenValue::Length {
                value: arg_input_line_height as f64,
            },
            None,
            false,
            &[
                "argPrompt.input.fontSize",
                "gpui-component 1.25×font line box",
            ],
        );
        b.add(
            "resolved.argPrompt.input.height",
            TokenStage::Resolved,
            Some("--sk-arg-input-height"),
            TokenValue::Length {
                value: arg_input_height as f64,
            },
            None,
            false,
            &["panel.CURSOR_HEIGHT_LG", "panel.CURSOR_MARGIN_Y"],
        );
        b.add(
            "resolved.argPrompt.input.fontSize",
            TokenStage::Resolved,
            Some("--sk-arg-input-font-size"),
            TokenValue::Length {
                value: crate::theme::TypographyResolver::new(
                    &theme,
                    crate::designs::DesignVariant::Default,
                )
                .font_size_xl() as f64,
            },
            None,
            false,
            &["theme.fonts.uiSize"],
        );
        b.source_len(
            "argPrompt.input.caretWidth",
            "--sk-arg-input-caret-width",
            2.0,
            "gpui_component::input::blink_cursor::CURSOR_WIDTH",
        );
        b.add(
            "resolved.argPrompt.input.caretHeight",
            TokenStage::Resolved,
            Some("--sk-arg-input-caret-height"),
            TokenValue::Length {
                value: arg_caret_height,
            },
            None,
            false,
            &[
                "argPrompt.input.fontSize",
                "gpui-component 0.85×line_height",
            ],
        );
        b.source_len(
            "argPrompt.divider.marginX",
            "--sk-arg-divider-margin-x",
            HEADER_DIVIDER_MARGIN,
            "crate::panel::HEADER_DIVIDER_MARGIN",
        );
        b.add(
            "resolved.argPrompt.footer.spacerHeight",
            TokenStage::Resolved,
            Some("--sk-arg-footer-spacer-height"),
            TokenValue::Length {
                value: arg_footer_spacer as f64,
            },
            None,
            false,
            &["footer.railHeight"],
        );
        b.add(
            "resolved.argPrompt.shell.fixtureHeight",
            TokenStage::Resolved,
            Some("--sk-arg-window-height"),
            TokenValue::Length {
                value: arg_fixture_height as f64,
            },
            None,
            false,
            &[
                "window_resize::height_for_view(MiniPrompt, 6) — launcher opens mini",
                "argPrompt.header.height",
            ],
        );

        b.conflict(
            "argPrompt.windowHeight.miniVsFull",
            &[
                (
                    "height_for_view(MiniPrompt, 6) — default mini launcher",
                    format!("{arg_fixture_height}"),
                ),
                (
                    "height_for_view(ArgPromptWithChoices, 6) — full-mode model",
                    format!("{arg_full_mode_height}"),
                ),
            ],
            "info",
            "Script-driven arg prompts open from the MINI launcher and clamp to 5 model \
             rows (279); the full-mode ArgPromptWithChoices model reserves 319.",
        );
        b.conflict(
            "argPrompt.headerHeight.modelVsPaint",
            &[
                (
                    "CURSOR_HEIGHT_LG input model (py*2 + 22)",
                    format!("{arg_header_model_height}"),
                ),
                (
                    "painted gpui-component line box (py*2 + 25)",
                    format!("{arg_header_height}"),
                ),
            ],
            "info",
            "The input host paints a 1.25×font line box (25px @ 20px font), not the \
             CURSOR_HEIGHT_LG 22px model; the painted header band is 41px.",
        );
        b.conflict(
            "argPrompt.rowHeight.modelVsPaint",
            &[
                (
                    "window_resize::LIST_ITEM_HEIGHT (model)",
                    format!("{}", crate::list_item::LIST_ITEM_HEIGHT),
                ),
                (
                    "InfoBarBase item_height (paint)",
                    format!("{}", def.list.item_height),
                ),
            ],
            "high",
            "Resize model reserves 40px/row but InfoBarBase paints 44px — the 6th \
             choice clips under a full list. Recorded, not smoothed.",
        );
        b.conflict(
            "argPrompt.listPadding.deadModelField",
            &[
                (
                    "ARG_LIST_PADDING_Y (counted in height model)",
                    format!("{}", crate::window_resize::layout::ARG_LIST_PADDING_Y),
                ),
                (
                    "arg renderer list padding",
                    "0 (no list pad painted)".into(),
                ),
            ],
            "info",
            "ARG_LIST_PADDING_Y is counted in height_for_view but the arg list paints no pad.",
        );
        b.conflict(
            "argPrompt.footerHeight.modelVsPaint",
            &[
                (
                    "window_resize::layout::FOOTER_HEIGHT",
                    format!("{}", crate::window_resize::layout::FOOTER_HEIGHT),
                ),
                (
                    "current_main_menu_footer_height (spacer)",
                    format!("{arg_footer_spacer}"),
                ),
            ],
            "info",
            "Model FOOTER_HEIGHT 30 vs painted native spacer 32 vs main-menu host 36.",
        );
    }

    // ── Known live conflicts (recorded, not collapsed) ──────────────────
    b.conflict(
        "rowHeight.legacyVsThemed",
        &[
            (
                "crate::list_item::LIST_ITEM_HEIGHT",
                format!("{}", crate::list_item::LIST_ITEM_HEIGHT),
            ),
            (
                "MainMenuListTokens.item_height",
                format!("{}", def.list.item_height),
            ),
        ],
        "info",
        "The themed main-menu path (from_main_menu_def) paints 44px rows; the legacy \
         constant still says 40px and is used by non-themed surfaces.",
    );
    b.conflict(
        "sectionHeader.slotVsLegacy",
        &[
            (
                "crate::list_item::SECTION_HEADER_HEIGHT",
                format!("{}", crate::list_item::SECTION_HEADER_HEIGHT),
            ),
            (
                "MainMenuListTokens.section_header_height",
                format!("{}", def.list.section_header_height),
            ),
        ],
        "info",
        "Themed section slot is 28px; the legacy constant is 32px.",
    );
    b.conflict(
        "selectedFill.componentVsTheme",
        &[
            (
                "MainMenuRowTokens.selected_fill_alpha",
                format!("0x{:02X}", def.row.selected_fill_alpha),
            ),
            ("theme.opacity.selected", format!("{}", opacity.selected)),
        ],
        "info",
        "The IconTile row paints the component alpha byte (0x20 ≈ 12.5% white), not \
         theme.opacity.selected (20%). Editing the theme opacity will not change the \
         launcher's selected row.",
    );

    let tokens_json =
        serde_json::to_string(&b.tokens).map_err(|e| format!("serialize tokens: {e}"))?;
    let bundle_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(tokens_json.as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    };

    Ok(DesignTokenBundle {
        schema_version: TOKENS_SCHEMA_VERSION,
        profile: ExportProfileRecord {
            theme_id: "script-kit-dark".to_string(),
            appearance: "dark".to_string(),
            main_menu_variant: "infoBarBase".to_string(),
            actions_popup_theme: "base".to_string(),
            actions_row_main_menu_variant: "infoBarBase".to_string(),
            design_variant: "default".to_string(),
            runtime_overrides: "disabled".to_string(),
            background_effect: effect.slug().to_string(),
            background_effect_intensity: intensity,
            scale_factor: 2.0,
        },
        bundle_hash,
        tokens: b.tokens,
        conflicts: b.conflicts,
    })
}

/// Render the generated `tokens.css` from a bundle: one `:root` block, one
/// custom property per token that declares a `css_var`, deterministic order.
pub fn render_css(bundle: &DesignTokenBundle) -> String {
    let mut css = String::from(
        "/* GENERATED by `export_design_tokens` — do not edit by hand.\n * Propose design changes via design/mockups/workbench/*.edits.json\n * and re-run the exporter; Rust is the single authority.\n */\n:root {\n",
    );
    css.push_str(&format!("  /* bundleHash: {} */\n", bundle.bundle_hash));
    for record in bundle.tokens.values() {
        let Some(var) = &record.css_var else { continue };
        let value = match &record.value {
            TokenValue::Length { value } => format_px(*value),
            TokenValue::Color { css, .. } => css.clone(),
            TokenValue::Number { value } => trim_float(*value),
            TokenValue::FontWeight { value } => trim_float(*value),
            TokenValue::DurationMs { value } => format!("{value}ms"),
            TokenValue::Text { value } => format!("\"{value}\""),
        };
        css.push_str(&format!("  {var}: {value};\n"));
    }
    css.push_str("}\n");
    css
}

fn format_px(value: f64) -> String {
    format!("{}px", trim_float(value))
}

fn trim_float(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locks the renderer-resolved bytes the HTML mockups depend on. If this
    /// test moves, regenerate design/mockups/generated and re-verify the
    /// published mockups before shipping the visual change.
    #[test]
    fn checked_in_bundle_matches_renderer_resolution() {
        let bundle = checked_in_design_bundle().expect("bundle builds");

        let rgba8 = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Color { rgba8, .. } => rgba8.clone(),
            other => panic!("{id} is not a color: {other:?}"),
        };
        let length = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Length { value } => *value,
            other => panic!("{id} is not a length: {other:?}"),
        };

        // Selected row: text_primary (#ffffff) at the component byte 0x20.
        assert_eq!(
            rgba8("resolved.mainMenu.row.selectedBackground"),
            "#FFFFFF20"
        );
        // Hover: max(theme hover 0.06*255=15, component 0x12=18) = 0x12.
        assert_eq!(rgba8("resolved.mainMenu.row.hoverBackground"), "#FFFFFF12");
        // Icon tile: accent #fbbf24 at max(def 0x80, IconTile floor 0xF2).
        assert_eq!(rgba8("resolved.mainMenu.icon.tileBackground"), "#FBBF24F2");
        // Themed geometry (44px rows), not the legacy 40px constant.
        assert_eq!(length("mainMenu.list.rowHeight"), 44.0);
        assert_eq!(length("mainMenu.row.radius"), 14.0);
        assert_eq!(length("window.width"), 750.0);
        assert_eq!(length("window.height"), 480.0);

        // The drift this system exists to expose stays recorded.
        assert!(bundle
            .conflicts
            .iter()
            .any(|c| c.id == "selectedFill.componentVsTheme"));

        // ── Actions dialog ──────────────────────────────────────────────
        // Shell + fixture composition (5 actions, 3 headers, footerless).
        assert_eq!(length("actionsDialog.shell.width"), 340.0);
        assert_eq!(length("actionsDialog.shell.maxHeight"), 400.0);
        assert_eq!(length("actionsDialog.shell.radius"), 18.0);
        assert_eq!(length("actionsDialog.shell.borderHeight"), 2.0);
        assert_eq!(length("actionsDialog.search.height"), 40.0);
        assert_eq!(length("actionsDialog.list.sectionHeaderHeight"), 24.0);
        assert_eq!(length("actionsDialog.list.rowHeight"), 36.0);
        assert_eq!(length("actionsDialog.list.paddingTop"), 0.0);
        assert_eq!(length("actionsDialog.list.paddingBottom"), 6.0);
        assert_eq!(
            length("resolved.actionsDialog.shell.bottomResidualHeight"),
            8.0
        );
        assert_eq!(length("resolved.actionsDialog.shell.fixtureHeight"), 300.0);
        // The height formula itself, not just its output snapshot.
        let popup = crate::designs::base_actions_popup_theme();
        assert_eq!(
            crate::actions::resolved_actions_popup_height(
                &popup, 5, 3, false, false, false, 400.0, 36.0
            ),
            300.0
        );

        // Search chrome.
        assert_eq!(length("actionsDialog.search.paddingX"), 12.0);
        assert_eq!(length("resolved.actionsDialog.search.paddingY"), 10.0);
        assert_eq!(
            rgba8("resolved.actionsDialog.search.caretColor"),
            "#FBBF24FF"
        );
        assert_eq!(
            rgba8("resolved.actionsDialog.search.placeholderColor"),
            "#FFFFFF66"
        );
        assert_eq!(
            rgba8("resolved.actionsDialog.search.textColor"),
            "#FFFFFFFF"
        );

        // Section chrome: centered 24px slot, muted label.
        assert_eq!(length("actionsDialog.section.paddingX"), 12.0);
        assert_eq!(
            rgba8("resolved.actionsDialog.section.textColor"),
            "#FFFFFFA5"
        );

        // Row geometry and paint: shared ListItem seeded from InfoBarBase.
        assert_eq!(length("actionsDialog.row.wrapperInsetX"), 8.0);
        assert_eq!(length("resolved.actionsDialog.row.outerPaddingX"), 4.0);
        assert_eq!(length("resolved.actionsDialog.row.innerPaddingX"), 14.0);
        assert_eq!(length("resolved.actionsDialog.row.surfaceInsetX"), 12.0);
        assert_eq!(length("resolved.actionsDialog.row.textOriginX"), 26.0);
        assert_eq!(length("resolved.actionsDialog.row.radius"), 14.0);
        assert_eq!(length("actionsDialog.row.titleFontSize"), 14.0);
        assert_eq!(length("resolved.actionsDialog.row.titleLineHeight"), 16.0);
        assert_eq!(
            rgba8("resolved.actionsDialog.row.selectedBackground"),
            "#FFFFFF20"
        );
        assert_eq!(
            rgba8("resolved.actionsDialog.row.hoverBackground"),
            "#FFFFFF12"
        );

        // Contract flags stay footerless / top-search / header-grouped.
        let text = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Text { value } => value.clone(),
            other => panic!("{id} is not text: {other:?}"),
        };
        assert_eq!(text("actionsDialog.contract.searchPosition"), "top");
        assert_eq!(text("actionsDialog.contract.sectionMode"), "headers");
        assert_eq!(text("actionsDialog.contract.footerVisible"), "false");

        // Action-specific drift stays recorded.
        for conflict_id in [
            "actionsRow.radiusConfiguredVsPainted",
            "actionsRow.selectionConfiguredVsPainted",
            "actionsRow.compactSlotVsInheritedItemHeight",
            "actionsShortcut.popupTokensVsFooterRenderer",
            "actionsFooter.legacyHeightVsFooterlessContract",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }

        // ── Confirm prompt (in-window) ──────────────────────────────────
        // Geometry pixel-validated 2026-07-11 (see module comment).
        assert_eq!(length("confirmPrompt.window.height"), 500.0);
        assert_eq!(length("confirmPrompt.content.padding"), 24.0);
        assert_eq!(length("confirmPrompt.stack.gap"), 12.0);
        assert_eq!(length("confirmPrompt.title.fontSize"), 20.0);
        assert_eq!(length("confirmPrompt.body.fontSize"), 14.0);
        assert_eq!(length("confirmPrompt.stack.maxWidth"), 560.0);
        // GPUI's implicit phi() line heights, rounded like line_height_in_pixels.
        assert_eq!(length("resolved.confirmPrompt.title.lineHeight"), 32.0);
        assert_eq!(length("resolved.confirmPrompt.body.lineHeight"), 23.0);
        assert_eq!(length("resolved.confirmPrompt.footerSpacerHeight"), 32.0);
        // HEADER_PADDING_Y*2 + context height 22 = 38 (min 28 not binding).
        assert_eq!(length("resolved.confirmPrompt.headerHeight"), 38.0);
        assert_eq!(rgba8("resolved.confirmPrompt.titleDanger"), "#EF4444FF");
        assert_eq!(rgba8("resolved.confirmPrompt.titleDefault"), "#FFFFFFFF");
        assert_eq!(rgba8("resolved.confirmPrompt.bodyText"), "#FFFFFFFF");
        for conflict_id in [
            "confirmLayout.protocolModelVsRendererTruth",
            "confirmGap.rendererSpacingVsLayoutOracle",
            "confirmTypography.implicitLineHeightVsModeledSlots",
            "confirmFooter.heightLadder",
            "confirmFooter.slotVsInnerFrame",
            "confirmStack.rendererIntrinsicVsLayoutModel",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }

        // ── Notes window ────────────────────────────────────────────────
        let text = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Text { value } => value.clone(),
            other => panic!("{id} is not text: {other:?}"),
        };
        let number = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Number { value } => *value,
            other => panic!("{id} is not a number: {other:?}"),
        };
        let weight = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::FontWeight { value } => *value,
            other => panic!("{id} is not a font weight: {other:?}"),
        };
        let record = |id: &str| bundle.tokens.get(id).expect(id);

        // App-authored chrome (source stage, writable).
        assert_eq!(length("notes.window.defaultWidth"), 350.0);
        assert_eq!(length("notes.window.defaultHeight"), 280.0);
        assert_eq!(length("notes.window.defaultEdgePadding"), 20.0);
        assert_eq!(length("notes.titlebar.height"), 36.0);
        assert_eq!(length("notes.titlebar.paddingX"), 12.0);
        assert_eq!(length("notes.titlebar.leadingReserveWidth"), 60.0);
        assert_eq!(length("notes.titlebar.trailingReserveWidth"), 100.0);
        assert_eq!(length("notes.titlebar.trafficLightOriginX"), 8.0);
        assert_eq!(length("notes.titlebar.trafficLightOriginY"), 7.0);
        assert_eq!(length("notes.editor.paddingX"), 16.0);
        assert_eq!(length("notes.editor.paddingY"), 12.0);
        assert_eq!(length("notes.footer.statusMinWidth"), 24.0);
        assert_eq!(length("notes.footer.contentInsetX"), 14.0);
        assert_eq!(length("notes.footer.actionGap"), 2.0);
        // Opacity numbers cross the f32→f64 bridge (0.7f32 is not exactly
        // 0.7), matching the existing exported-number precedent
        // (--sk-main-menu-context-opacity).
        assert_eq!(number("notes.titlebar.titleRestOpacity"), 0.7f32 as f64);
        assert_eq!(number("notes.footer.restOpacity"), 0.5);
        for id in ["notes.window.defaultWidth", "notes.titlebar.height"] {
            let r = record(id);
            assert!(matches!(r.stage, TokenStage::Source), "{id} must be source");
            assert!(r.writable, "{id} must be writable");
        }

        // Layout MODEL: honest 28px reservation, distinct from paint.
        assert_eq!(length("notes.layout.footerReservationHeight"), 28.0);
        assert_eq!(length("notes.layout.autoResize.maxHeight"), 600.0);
        assert_eq!(length("notes.layout.autoResize.assumedLineHeight"), 20.0);
        assert_eq!(length("notes.layout.autoResize.applyThreshold"), 5.0);
        assert!(
            record("notes.layout.footerReservationHeight")
                .css_var
                .is_none(),
            "the 28px model reservation must not leak into mockup CSS"
        );
        assert!(!record("notes.layout.autoResize.assumedLineHeight").writable);

        // Footer presentation facts.
        assert_eq!(text("notes.footer.presentation"), "inWindowGpui");
        assert_eq!(text("notes.footer.nativeOverlay"), "false");
        assert_eq!(text("notes.footer.visibility"), "selectedNoteOnly");
        assert_eq!(text("notes.editor.markdown.language"), "markdown");
        for id in [
            "notes.editor.markdown.highlightQueryFingerprint",
            "notes.editor.markdown.injectionQueryFingerprint",
            "notes.editor.markdown.inlineHighlightQueryFingerprint",
        ] {
            assert!(
                text(id).starts_with("fnv1a64:"),
                "{id} must be a stable query fingerprint"
            );
            assert!(!record(id).writable);
        }

        // Resolved editor paint metrics (theme bridge + Input internals).
        assert_eq!(length("resolved.notes.editor.baseFontSize"), 16.0);
        assert_eq!(length("resolved.notes.editor.lineBoxHeight"), 20.0);
        assert_eq!(length("resolved.notes.editor.caretWidth"), 2.0);
        assert_eq!(length("resolved.notes.editor.caretHeight"), 17.0);
        assert_eq!(text("resolved.notes.editor.fontFamily"), "JetBrains Mono");
        assert_eq!(length("resolved.notes.titlebar.titleFontSize"), 14.0);
        assert_eq!(length("resolved.notes.footer.statusFontSize"), 12.0);
        // Painted footer band (32) vs the 28px model above.
        assert_eq!(length("resolved.notes.footer.intrinsicHeight"), 32.0);
        for id in [
            "resolved.notes.editor.baseFontSize",
            "resolved.notes.editor.lineBoxHeight",
            "resolved.notes.footer.intrinsicHeight",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // Markdown capture styles from the real highlight theme: accent bold
        // title, muted (separate) heading marker, accent bold list marker —
        // and NO heading font-size token (gpui HighlightStyle is uniformly
        // sized; the heading clips instead).
        assert_eq!(
            rgba8("resolved.notes.editor.markdown.titleColor"),
            "#FBBF24FF"
        );
        assert_eq!(
            rgba8("resolved.notes.editor.markdown.headingMarkerColor"),
            "#FFFFFFFF"
        );
        assert_eq!(
            rgba8("resolved.notes.editor.markdown.listMarkerColor"),
            "#FBBF24FF"
        );
        assert_eq!(
            weight("resolved.notes.editor.markdown.titleFontWeight"),
            700.0
        );
        assert_eq!(
            weight("resolved.notes.editor.markdown.listMarkerFontWeight"),
            700.0
        );
        assert!(
            !bundle
                .tokens
                .keys()
                .any(|k| k.contains("markdown.titleFontSize")
                    || k.contains("markdown.headingFontSize")),
            "no markdown heading font-size token may exist"
        );
        assert!(
            !bundle
                .conflicts
                .iter()
                .any(|c| c.id == "notesMarkdown.exporterVisibilityMissing"),
            "the highlight theme is reachable; the visibility conflict must not fire"
        );

        // Notes drift stays recorded.
        for conflict_id in [
            "notesFooter.layoutReservationVsIntrinsicPaint",
            "notesFooter.buttonHeightSourceDuplication",
            "notesMarkdown.titleGlyphExtentsVsLineBox",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }
        assert_eq!(
            bundle
                .conflicts
                .iter()
                .find(|c| c.id == "notesFooter.layoutReservationVsIntrinsicPaint")
                .expect("reservation conflict")
                .severity,
            "warning"
        );

        // ── Settings hub ────────────────────────────────────────────────
        // Canonical shared owners only — settings mints NO alias tokens
        // (2026-07-11 Oracle correction). The profile records the design
        // variant the exporter resolves spacing with.
        assert_eq!(bundle.profile.design_variant, "default");
        assert_eq!(length("design.spacing.paddingXs"), 4.0);
        {
            let r = record("design.spacing.paddingXs");
            assert!(matches!(r.stage, TokenStage::Source));
            assert!(r.writable);
        }
        // Shared builtin-input count-label typography: text_sm size, gpui
        // default phi line height, NORMAL weight (never search 430).
        assert_eq!(
            length("resolved.builtinMainInput.countLabel.fontSize"),
            14.0
        );
        assert_eq!(
            length("resolved.builtinMainInput.countLabel.lineHeight"),
            23.0
        );
        assert_eq!(
            weight("resolved.builtinMainInput.countLabel.fontWeight"),
            400.0
        );
        // The first "Settings" separator paints the LEGACY list-item default
        // path (26/6) while the themed InfoBarBase pair stays 28/4.
        assert_eq!(
            length("resolved.listItem.default.firstSectionSlotHeight"),
            26.0
        );
        assert_eq!(
            length("resolved.listItem.default.firstSectionPaddingTop"),
            6.0
        );
        assert_eq!(length("mainMenu.list.firstSectionSlotHeight"), 28.0);
        assert_eq!(length("mainMenu.section.firstPaddingTop"), 4.0);
        for id in [
            "resolved.builtinMainInput.countLabel.fontSize",
            "resolved.builtinMainInput.countLabel.lineHeight",
            "resolved.builtinMainInput.countLabel.fontWeight",
            "resolved.listItem.default.firstSectionSlotHeight",
            "resolved.listItem.default.firstSectionPaddingTop",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // JSON-only settings facts (no CSS role, never writable).
        assert_eq!(text("settingsHub.section.emptyFilterLabel"), "Settings");
        assert_eq!(text("settingsHub.section.filteredLabel"), "Results");
        assert_eq!(text("settingsHub.countLabel.counts"), "visibleFilteredRows");
        assert_eq!(
            text("settingsHub.countLabel.pluralization"),
            "1 setting / 2 settings"
        );
        assert_eq!(number("settingsHub.census.baseCount"), 11.0);
        assert_eq!(number("settingsHub.census.customPositionsCount"), 12.0);
        assert_eq!(
            text("settingsHub.census.optionalRow"),
            "Reset Window Positions"
        );
        assert_eq!(
            text("settingsHub.census.optionalPredicate"),
            "windowState.hasCustomPositions"
        );
        assert_eq!(number("settingsHub.icons.resolvedRowIconCount"), 0.0);
        for id in [
            "settingsHub.section.emptyFilterLabel",
            "settingsHub.section.filteredLabel",
            "settingsHub.countLabel.counts",
            "settingsHub.countLabel.pluralization",
            "settingsHub.census.baseCount",
            "settingsHub.census.customPositionsCount",
            "settingsHub.census.optionalRow",
            "settingsHub.census.optionalPredicate",
            "settingsHub.icons.resolvedRowIconCount",
        ] {
            let r = record(id);
            assert!(r.css_var.is_none(), "{id} is a JSON-only fact");
            assert!(!r.writable, "{id} must not be writable");
        }

        // The rejected settings.* alias family must not exist: the count
        // inset reuses --sk-main-menu-search-text-inset-x and the count
        // color reuses --sk-text-hint directly.
        assert!(
            bundle.tokens.keys().all(|k| !k.starts_with("settings.")),
            "settings must not mint alias tokens under settings.*"
        );

        // Settings drift stays recorded.
        for conflict_id in [
            "settingsSection.firstSlotLegacyVsThemed",
            "settingsRows.authoredIconHintsVsResolvedNone",
            "settingsFooter.nativeRunVsGpuiOpenHint",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }
        assert_eq!(
            bundle
                .conflicts
                .iter()
                .find(|c| c.id == "settingsRows.authoredIconHintsVsResolvedNone")
                .expect("icon conflict")
                .severity,
            "warning"
        );

        // ── Day Page (2026-07-11 Oracle-corrected slice) ────────────────
        // The five Day-owned geometry tokens (source, writable) — the ONLY
        // --sk-day-page-* CSS variables allowed to exist.
        assert_eq!(length("dayPage.editor.minHeight"), 180.0);
        assert_eq!(length("dayPage.shelf.topPadding"), 6.0);
        assert_eq!(length("dayPage.shelf.toggleHeight"), 20.0);
        assert_eq!(length("dayPage.shelf.expandedListGap"), 4.0);
        assert_eq!(length("dayPage.shelf.rowSlotHeight"), 24.0);
        for id in [
            "dayPage.editor.minHeight",
            "dayPage.shelf.topPadding",
            "dayPage.shelf.toggleHeight",
            "dayPage.shelf.expandedListGap",
            "dayPage.shelf.rowSlotHeight",
        ] {
            let r = record(id);
            assert!(matches!(r.stage, TokenStage::Source), "{id} must be source");
            assert!(r.writable, "{id} must be writable");
        }
        assert_eq!(number("dayPage.shelf.maxBodyFraction"), 0.4f32 as f64);
        assert!(record("dayPage.shelf.maxBodyFraction").css_var.is_none());

        // Shared owners the Day Page consumes (NO Day copies).
        assert_eq!(length("resolved.mainView.contentRightInsetX"), 2.0);
        assert_eq!(length("resolved.notes.editor.inputPaddingX"), 12.0);
        assert_eq!(length("resolved.notes.editor.inputPaddingY"), 8.0);
        assert_eq!(rgba8("resolved.notes.editor.textColor"), "#FFFFFFFF");
        assert_eq!(rgba8("resolved.notes.editor.caretColor"), "#FFFFFFFF");
        assert_eq!(rgba8("resolved.notes.editor.linkLabelColor"), "#FBBF24FF");
        // Rest destination: accent through the ACTUAL highlighter helper
        // (accent.opacity(0.45)) — 0.45 × 255 rounds to 0x73, resolved by
        // the color conversion, never a hand-entered byte.
        assert_eq!(
            rgba8("resolved.notes.editor.linkDestinationRestColor"),
            "#FBBF2473"
        );
        assert_eq!(
            number("notesEditor.link.destinationCompactOpacity"),
            0.45f32 as f64
        );
        assert!(record("notesEditor.link.destinationCompactOpacity")
            .css_var
            .is_none());
        assert_eq!(
            text("notesEditor.link.destinationStateRule"),
            "compactUnlessSelectionOverlapsOrTouchesFullRange"
        );
        // muted_foreground = text.primary @ opacity.text_placeholder (0.40).
        assert_eq!(
            rgba8("resolved.componentTheme.mutedForeground"),
            "#FFFFFF66"
        );
        assert_eq!(rgba8("resolved.componentTheme.foreground"), "#FFFFFFFF");
        assert_eq!(length("resourcePreview.compactRow.paddingX"), 8.0);
        assert_eq!(length("resourcePreview.compactRow.paddingY"), 4.0);
        assert_eq!(length("resolved.resourcePreview.compactRow.gap"), 8.0);
        assert_eq!(length("resolved.framework.textXsFontSize"), 12.0);
        assert_eq!(length("resolved.framework.gap1"), 4.0);
        for id in [
            "resolved.mainView.contentRightInsetX",
            "resolved.notes.editor.inputPaddingX",
            "resolved.notes.editor.inputPaddingY",
            "resolved.notes.editor.textColor",
            "resolved.notes.editor.caretColor",
            "resolved.notes.editor.linkLabelColor",
            "resolved.notes.editor.linkDestinationRestColor",
            "resolved.componentTheme.mutedForeground",
            "resolved.componentTheme.foreground",
            "resolved.resourcePreview.compactRow.gap",
            "resolved.framework.textXsFontSize",
            "resolved.framework.gap1",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // JSON-only Day Page facts (no CSS role, never writable).
        assert_eq!(text("dayPage.header.contextInteraction"), "inert");
        assert_eq!(text("dayPage.header.inputSlot"), "none");
        assert_eq!(text("dayPage.header.dividerVisible"), "false");
        assert_eq!(text("dayPage.editor.spine.localOverlay"), "disabled");
        assert_eq!(
            text("dayPage.editor.spine.contextMentions"),
            "mainMenuRoundTrip"
        );
        assert_eq!(text("dayPage.shelf.defaultExpanded"), "false");
        assert_eq!(text("dayPage.shelf.hiddenWhenEmpty"), "true");
        assert_eq!(text("dayPage.shelf.hiddenDuringKitPreview"), "true");
        assert_eq!(text("dayPage.shelf.sourceLines"), "liftedFromEditor");
        assert_eq!(
            text("dayPage.footer.presentation"),
            "gpuiSpacerPlusNativeOverlay"
        );
        assert_eq!(text("dayPage.footer.defaultAction"), "actions");
        for id in [
            "dayPage.header.contextInteraction",
            "dayPage.header.inputSlot",
            "dayPage.header.dividerVisible",
            "dayPage.editor.spine.localOverlay",
            "dayPage.editor.spine.contextMentions",
            "dayPage.shelf.defaultExpanded",
            "dayPage.shelf.hiddenWhenEmpty",
            "dayPage.shelf.hiddenDuringKitPreview",
            "dayPage.shelf.sourceLines",
            "dayPage.footer.presentation",
            "dayPage.footer.defaultAction",
        ] {
            let r = record(id);
            assert!(r.css_var.is_none(), "{id} is a JSON-only fact");
            assert!(!r.writable, "{id} must not be writable");
        }

        // No Day-prefixed duplicates of shared editor/link/caret/footer
        // tokens: the ONLY --sk-day-page-* variables are the five geometry
        // tokens above.
        let day_vars: Vec<&str> = bundle
            .tokens
            .values()
            .filter_map(|r| r.css_var.as_deref())
            .filter(|v| v.starts_with("--sk-day-page-"))
            .collect();
        let mut day_vars_sorted = day_vars.clone();
        day_vars_sorted.sort_unstable();
        assert_eq!(
            day_vars_sorted,
            vec![
                "--sk-day-page-editor-min-height",
                "--sk-day-page-shelf-expanded-list-gap",
                "--sk-day-page-shelf-row-slot-height",
                "--sk-day-page-shelf-toggle-height",
                "--sk-day-page-shelf-top-padding",
            ],
            "Day Page may only own its five geometry variables"
        );

        // Every CSS variable has exactly ONE token owner (bundle-wide).
        {
            let mut seen = std::collections::BTreeMap::new();
            for (id, r) in &bundle.tokens {
                if let Some(var) = &r.css_var {
                    if let Some(previous) = seen.insert(var.clone(), id.clone()) {
                        panic!("css var {var} owned by both {previous} and {id}");
                    }
                }
            }
        }

        // Canonical reference-fixture geometry (750×480, context-only header
        // 30, footer 32, one kept entry): collapsed 418/380/38/0 — the formula itself,
        // not just a snapshot.
        let collapsed = crate::day_page::layout::day_page_layout_budget(
            length("window.height") as f32,
            30.0,
            length("resolved.confirmPrompt.footerSpacerHeight") as f32,
            1,
            false,
            length("notes.editor.paddingY") as f32,
        );
        assert_eq!(collapsed.body_height, 418.0);
        assert_eq!(collapsed.editor_height, 380.0);
        assert_eq!(collapsed.shelf_height, 38.0);
        assert_eq!(collapsed.shelf_list_height, 0.0);

        // Day Page drift stays recorded (the footer height ladder).
        assert_eq!(
            bundle
                .conflicts
                .iter()
                .find(|c| c.id == "dayPageFooter.spacerVsNativeHostBand")
                .expect("day page footer conflict")
                .severity,
            "warning"
        );

        // CSS renders every var exactly once.
        let css = render_css(&bundle);
        assert_eq!(css.matches("--sk-main-menu-row-height:").count(), 1);
        assert!(css
            .contains("--sk-main-menu-row-selected-background: rgb(255 255 255 / 0.1254901961);"));
        assert_eq!(
            css.matches("--sk-actions-dialog-row-selected-background:")
                .count(),
            1
        );
        assert!(css.contains(
            "--sk-actions-dialog-row-selected-background: rgb(255 255 255 / 0.1254901961);"
        ));
        assert!(css.contains("--sk-actions-dialog-height: 300px;"));
        assert!(!css.contains("--sk-actions-dialog-footer-height:"));
        assert!(css.contains("--sk-confirm-window-height: 500px;"));
        assert!(css.contains("--sk-confirm-stack-max-width: 560px;"));
        assert!(css.contains("--sk-confirm-title-danger: rgb(239 68 68);"));
        assert!(css.contains("--sk-confirm-body-line-height: 23px;"));

        // Every --sk-notes-* var appears exactly once, with resolved values.
        for var in [
            "--sk-notes-window-width",
            "--sk-notes-window-height",
            "--sk-notes-titlebar-height",
            "--sk-notes-titlebar-padding-x",
            "--sk-notes-titlebar-traffic-width",
            "--sk-notes-titlebar-icons-width",
            "--sk-notes-titlebar-title-font-size",
            "--sk-notes-titlebar-title-rest-opacity",
            "--sk-notes-traffic-x",
            "--sk-notes-traffic-y",
            "--sk-notes-editor-padding-x",
            "--sk-notes-editor-padding-y",
            "--sk-notes-editor-font-size",
            "--sk-notes-editor-font-family",
            "--sk-notes-editor-line-box-height",
            "--sk-notes-editor-input-padding-x",
            "--sk-notes-editor-input-padding-y",
            "--sk-notes-editor-text-color",
            "--sk-notes-editor-link-label",
            "--sk-notes-editor-link-destination-rest",
            "--sk-notes-caret-width",
            "--sk-notes-caret-height",
            "--sk-notes-caret-color",
            "--sk-notes-footer-height",
            "--sk-notes-footer-content-inset-x",
            "--sk-notes-footer-rest-opacity",
            "--sk-notes-footer-status-min-width",
            "--sk-notes-footer-status-font-size",
            "--sk-notes-footer-action-gap",
            "--sk-notes-markdown-title-color",
            "--sk-notes-markdown-title-font-weight",
            "--sk-notes-markdown-heading-marker-color",
            "--sk-notes-markdown-list-marker-color",
            "--sk-notes-markdown-list-marker-font-weight",
        ] {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert!(css.contains("--sk-notes-footer-height: 32px;"));
        assert!(css.contains("--sk-notes-editor-line-box-height: 20px;"));
        assert!(css.contains("--sk-notes-markdown-title-color: rgb(251 191 36);"));
        assert!(!css.contains("--sk-notes-editor-line-height:"));
        assert!(!css.contains("--sk-notes-markdown-heading-font-size"));

        // Settings-slice vars render exactly once, under their shared
        // owners; NO --sk-settings-* alias vars may exist.
        for var in [
            "--sk-spacing-padding-xs",
            "--sk-builtin-main-input-count-font-size",
            "--sk-builtin-main-input-count-line-height",
            "--sk-builtin-main-input-count-font-weight",
            "--sk-list-item-default-first-section-slot-height",
            "--sk-list-item-default-first-section-padding-top",
        ] {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert!(css.contains("--sk-spacing-padding-xs: 4px;"));
        assert!(css.contains("--sk-builtin-main-input-count-font-size: 14px;"));
        assert!(css.contains("--sk-builtin-main-input-count-line-height: 23px;"));
        assert!(css.contains("--sk-builtin-main-input-count-font-weight: 400;"));
        assert!(css.contains("--sk-list-item-default-first-section-slot-height: 26px;"));
        assert!(css.contains("--sk-list-item-default-first-section-padding-top: 6px;"));
        assert!(!css.contains("--sk-settings-"));

        // Day Page slice vars render exactly once; NO rejected Day aliases.
        for var in [
            "--sk-day-page-editor-min-height",
            "--sk-day-page-shelf-top-padding",
            "--sk-day-page-shelf-toggle-height",
            "--sk-day-page-shelf-expanded-list-gap",
            "--sk-day-page-shelf-row-slot-height",
            "--sk-main-view-content-right-inset-x",
            "--sk-component-theme-muted-foreground",
            "--sk-component-theme-foreground",
            "--sk-compact-resource-row-padding-x",
            "--sk-compact-resource-row-padding-y",
            "--sk-compact-resource-row-gap",
            "--sk-framework-text-xs-font-size",
            "--sk-framework-gap-1",
        ] {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert!(css.contains("--sk-day-page-editor-min-height: 180px;"));
        assert!(css.contains("--sk-main-view-content-right-inset-x: 2px;"));
        assert!(css
            .contains("--sk-notes-editor-link-destination-rest: rgb(251 191 36 / 0.4509803922);"));
        assert!(css.contains("--sk-notes-editor-font-family: \"JetBrains Mono\";"));
        // Rejected Day-prefixed duplicates must never exist.
        for rejected in [
            "--sk-day-page-content-inset-x:",
            "--sk-day-page-editor-padding",
            "--sk-day-page-editor-input-padding",
            "--sk-day-page-editor-font-size:",
            "--sk-day-page-editor-line-height:",
            "--sk-day-page-editor-text:",
            "--sk-day-page-link-",
            "--sk-day-page-caret-",
            "--sk-day-page-shelf-gap:",
            "--sk-day-page-shelf-row-height:",
            "--sk-day-page-shelf-row-padding",
            "--sk-day-page-shelf-font-size:",
            "--sk-day-page-shelf-muted:",
            "--sk-day-page-footer-spacer-height:",
        ] {
            assert!(
                !css.contains(rejected),
                "rejected Day Page alias {rejected} must not exist"
            );
        }

        // ── Agent Chat (2026-07-11 Oracle-corrected slice) ──────────────
        // Source geometry (writable) straight off production_agent_chat_style.
        assert_eq!(length("agentChat.transcript.rowPaddingX"), 16.0);
        assert_eq!(length("agentChat.transcript.rowPaddingBottom"), 4.0);
        assert_eq!(length("agentChat.markdown.bodyFontSize"), 14.0);
        assert_eq!(length("agentChat.markdown.codeFontSize"), 13.0);
        assert_eq!(length("agentChat.block.borderWidth"), 2.0);
        assert_eq!(length("agentChat.block.headerGap"), 4.0);
        // Embedded default composer aliases the canonical main-menu search.
        assert_eq!(length("agentChat.composer.fontSize"), 20.0);
        assert_eq!(length("agentChat.composer.lineHeight"), 26.0);
        assert_eq!(length("agentChat.send.size"), 24.0);
        assert_eq!(length("agentChat.send.radius"), 6.0);
        {
            let r = record("agentChat.transcript.rowPaddingX");
            assert!(matches!(r.stage, TokenStage::Source));
            assert!(r.writable);
        }

        // Thought and tool header opacities stay SEPARATE tokens (both 0.75).
        assert_eq!(number("agentChat.block.thoughtHeaderOpacity"), 0.75);
        assert_eq!(number("agentChat.block.toolHeaderOpacity"), 0.75);
        assert_ne!(
            record("agentChat.block.thoughtHeaderOpacity").css_var,
            record("agentChat.block.toolHeaderOpacity").css_var,
            "thought/tool header opacities must not collapse into one var"
        );

        // Authored alpha leaves: JSON-only, and the decimal-50 foot-gun
        // stays authored decimal (0x32 only after the shared packer).
        assert_eq!(number("agentChat.error.bgAlpha"), 50.0);
        assert_eq!(number("agentChat.user.bgAlpha"), 6.0);
        assert_eq!(number("agentChat.block.toolBorderAlpha"), 127.0);
        assert_eq!(number("agentChat.diff.tintAlpha"), 20.0);
        assert_eq!(
            number("agentChat.markdown.paragraphGapRems"),
            0.28f32 as f64
        );
        for id in [
            "agentChat.transcript.turnDividerAlpha",
            "agentChat.markdown.codeBgAlpha",
            "agentChat.markdown.codeBorderAlpha",
            "agentChat.markdown.blockquoteBgAlpha",
            "agentChat.markdown.blockquoteBorderAlpha",
            "agentChat.user.bgAlpha",
            "agentChat.block.thoughtBorderAlpha",
            "agentChat.block.toolBorderAlpha",
            "agentChat.tool.statusPendingAlpha",
            "agentChat.diff.tintAlpha",
            "agentChat.system.borderAlpha",
            "agentChat.error.bgAlpha",
            "agentChat.error.borderAlpha",
            "agentChat.send.disabledBgAlpha",
            "agentChat.send.enabledBgAlpha",
            "agentChat.send.queueBgAlpha",
            "agentChat.markdown.paragraphGapRems",
            "agentChat.composer.paddingX",
            "agentChat.composer.paddingY",
        ] {
            assert!(
                record(id).css_var.is_none(),
                "{id} must stay JSON-only (no CSS variable)"
            );
        }

        // Resolved paint bytes — the SAME resolver output the renderer packs.
        assert_eq!(
            rgba8("resolved.agentChat.transcript.turnDivider"),
            "#34343418"
        );
        assert_eq!(rgba8("resolved.agentChat.user.bg"), "#FFFFFF06");
        assert_eq!(rgba8("resolved.agentChat.markdown.codeBg"), "#2A2A2AA0");
        assert_eq!(rgba8("resolved.agentChat.markdown.codeBorder"), "#34343440");
        assert_eq!(rgba8("resolved.agentChat.thought.border"), "#FFFFFF7F");
        assert_eq!(rgba8("resolved.agentChat.tool.border"), "#FBBF247F");
        assert_eq!(rgba8("resolved.agentChat.tool.borderError"), "#EF44447F");
        assert_eq!(rgba8("resolved.agentChat.tool.statusPending"), "#FFFFFF80");
        assert_eq!(rgba8("resolved.agentChat.tool.statusComplete"), "#00FF00FF");
        assert_eq!(rgba8("resolved.agentChat.tool.statusFailed"), "#EF4444FF");
        assert_eq!(rgba8("resolved.agentChat.diff.addedBg"), "#00FF0014");
        assert_eq!(rgba8("resolved.agentChat.diff.removedBg"), "#EF444414");
        assert_eq!(rgba8("resolved.agentChat.system.border"), "#34343430");
        // Decimal 50 → 0x32 through the shared pack_rgb_alpha owner.
        assert_eq!(rgba8("resolved.agentChat.error.bg"), "#EF444432");
        assert_eq!(rgba8("resolved.agentChat.error.border"), "#EF444480");
        assert_eq!(rgba8("resolved.agentChat.send.disabledBg"), "#FFFFFF06");
        assert_eq!(rgba8("resolved.agentChat.send.enabledBg"), "#FBBF2430");
        assert_eq!(rgba8("resolved.agentChat.send.queueBg"), "#FBBF2424");

        // Resolved typography/geometry through the shared app helpers.
        assert_eq!(length("resolved.agentChat.markdown.bodyLineHeight"), 23.0);
        assert_eq!(length("resolved.agentChat.composer.singleLineHeight"), 26.0);
        assert_eq!(length("resolved.framework.textSmFontSize"), 14.0);
        for id in [
            "resolved.agentChat.transcript.turnDivider",
            "resolved.agentChat.markdown.bodyLineHeight",
            "resolved.agentChat.composer.singleLineHeight",
            "resolved.framework.textSmFontSize",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // JSON-only Agent Chat facts.
        assert_eq!(
            text("agentChat.composer.placeholderEmpty"),
            "Ask anything\u{2026}"
        );
        assert_eq!(
            text("agentChat.composer.placeholderFollowUp"),
            "Follow up\u{2026}"
        );
        assert_eq!(
            text("agentChat.composer.fontFamily"),
            crate::list_item::FONT_SYSTEM_UI
        );
        assert_eq!(text("agentChat.legacyComposer.fontFamily"), ".SystemUIFont");
        let embedded_composer_family = record("agentChat.composer.fontFamily");
        assert!(matches!(
            embedded_composer_family.stage,
            TokenStage::Resolved
        ));
        assert_eq!(
            embedded_composer_family.derived_from,
            vec!["mainMenu.type.uiFontFamily".to_string()]
        );
        assert_eq!(
            text("agentChat.transcript.alignment"),
            "bottomFollowTailWithSyntheticActivityTail"
        );
        assert_eq!(
            text("agentChat.footer.presentation"),
            "gpuiSpacerPlusNativeOverlay"
        );
        assert_eq!(
            text("agentChat.tool.defaultExpansion"),
            "collapsedExceptDiffOrError"
        );
        // Variant-limited numbers stay JSON-only facts.
        assert_eq!(number("agentChat.user.maxWidthRoleSplitOnly"), 520.0);
        assert_eq!(number("agentChat.assistant.maxWidthRoleSplitOnly"), 620.0);
        assert_eq!(number("agentChat.assistant.radius"), 0.0);
        assert_eq!(number("agentChat.assistant.bgAlpha"), 0.0);
        assert_eq!(number("agentChat.activity.dotSize"), 7.0);
        assert_eq!(number("agentChat.activity.gap"), 8.0);
        assert_eq!(number("agentChat.activity.labelAlpha"), 176.0);
        for id in [
            "agentChat.composer.placeholderEmpty",
            "agentChat.composer.placeholderFollowUp",
            "agentChat.composer.fontFamily",
            "agentChat.legacyComposer.fontFamily",
            "agentChat.transcript.alignment",
            "agentChat.footer.presentation",
            "agentChat.tool.defaultExpansion",
            "agentChat.user.maxWidthRoleSplitOnly",
            "agentChat.assistant.maxWidthRoleSplitOnly",
            "agentChat.assistant.radius",
            "agentChat.assistant.bgAlpha",
            "agentChat.activity.dotSize",
            "agentChat.activity.gap",
            "agentChat.activity.labelAlpha",
        ] {
            let r = record(id);
            assert!(r.css_var.is_none(), "{id} is a JSON-only fact");
            assert!(!r.writable, "{id} must not be writable");
        }

        // Remaining Agent Chat drift stays recorded as explicit conflicts.
        for (conflict_id, severity) in [
            ("agentChat.error.bgAlphaUnits", "info"),
            ("agentChat.standard.roleSplitOnlyFields", "info"),
        ] {
            let conflict = bundle
                .conflicts
                .iter()
                .find(|c| c.id == conflict_id)
                .unwrap_or_else(|| panic!("missing conflict {conflict_id}"));
            assert_eq!(conflict.severity, severity, "{conflict_id} severity");
        }

        // The explicit Agent Chat CSS-variable manifest: every var renders
        // exactly once, and nothing outside this list may exist.
        let agent_chat_manifest = [
            "--sk-agent-chat-row-padding-x",
            "--sk-agent-chat-row-padding-bottom",
            "--sk-agent-chat-response-start-margin-top",
            "--sk-agent-chat-turn-margin-top",
            "--sk-agent-chat-turn-padding-top",
            "--sk-agent-chat-turn-divider",
            "--sk-agent-chat-md-body-font-size",
            "--sk-agent-chat-md-body-line-height",
            "--sk-agent-chat-md-h1-font-size",
            "--sk-agent-chat-md-h2-font-size",
            "--sk-agent-chat-md-h3-font-size",
            "--sk-agent-chat-md-code-font-size",
            "--sk-agent-chat-md-code-padding-x",
            "--sk-agent-chat-md-code-padding-y",
            "--sk-agent-chat-md-code-radius",
            "--sk-agent-chat-md-code-bg",
            "--sk-agent-chat-md-code-border",
            "--sk-agent-chat-md-blockquote-padding-x",
            "--sk-agent-chat-md-blockquote-padding-y",
            "--sk-agent-chat-md-blockquote-radius",
            "--sk-agent-chat-md-blockquote-bg",
            "--sk-agent-chat-md-blockquote-border",
            "--sk-agent-chat-user-padding-x",
            "--sk-agent-chat-user-padding-y",
            "--sk-agent-chat-user-radius",
            "--sk-agent-chat-user-bg",
            "--sk-agent-chat-assistant-padding-x",
            "--sk-agent-chat-assistant-padding-y",
            "--sk-agent-chat-block-padding-x",
            "--sk-agent-chat-block-padding-y",
            "--sk-agent-chat-block-body-padding-top",
            "--sk-agent-chat-block-max-body-height",
            "--sk-agent-chat-block-border-width",
            "--sk-agent-chat-block-header-gap",
            "--sk-agent-chat-thought-header-opacity",
            "--sk-agent-chat-tool-header-opacity",
            "--sk-agent-chat-block-status-opacity",
            "--sk-agent-chat-thought-border",
            "--sk-agent-chat-tool-border",
            "--sk-agent-chat-tool-border-error",
            "--sk-agent-chat-tool-status-pending",
            "--sk-agent-chat-tool-status-complete",
            "--sk-agent-chat-tool-status-failed",
            "--sk-agent-chat-diff-added-bg",
            "--sk-agent-chat-diff-removed-bg",
            "--sk-agent-chat-diff-context-opacity",
            "--sk-agent-chat-system-padding-x",
            "--sk-agent-chat-system-padding-y",
            "--sk-agent-chat-system-opacity",
            "--sk-agent-chat-system-border",
            "--sk-agent-chat-error-padding-x",
            "--sk-agent-chat-error-padding-y",
            "--sk-agent-chat-error-radius",
            "--sk-agent-chat-error-bg",
            "--sk-agent-chat-error-border",
            "--sk-agent-chat-error-label-opacity",
            "--sk-agent-chat-error-hint-opacity",
            "--sk-agent-chat-composer-font-size",
            "--sk-agent-chat-composer-font-weight",
            "--sk-agent-chat-composer-line-height",
            "--sk-agent-chat-composer-single-line-height",
            "--sk-agent-chat-send-size",
            "--sk-agent-chat-send-radius",
            "--sk-agent-chat-send-disabled-bg",
            "--sk-agent-chat-send-disabled-opacity",
            "--sk-agent-chat-send-enabled-bg",
            "--sk-agent-chat-send-enabled-opacity",
            "--sk-agent-chat-send-queue-bg",
            "--sk-agent-chat-send-queue-opacity",
            "--sk-agent-chat-send-streaming-opacity",
        ];
        for var in agent_chat_manifest {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert_eq!(
            css.matches("--sk-agent-chat-").count(),
            agent_chat_manifest.len(),
            "no --sk-agent-chat-* variable may exist outside the manifest"
        );
        assert_eq!(css.matches("--sk-framework-text-sm-font-size:").count(), 1);
        assert!(css.contains("--sk-agent-chat-md-body-font-size: 14px;"));
        assert!(css.contains("--sk-agent-chat-md-body-line-height: 23px;"));
        assert!(css.contains("--sk-agent-chat-composer-font-size: 20px;"));
        assert!(css.contains("--sk-agent-chat-composer-font-weight: 430;"));
        assert!(css.contains("--sk-agent-chat-composer-line-height: 26px;"));
        assert!(css.contains("--sk-agent-chat-composer-single-line-height: 26px;"));
        assert!(css.contains("--sk-agent-chat-thought-header-opacity: 0.75;"));
        assert!(css.contains("--sk-agent-chat-tool-header-opacity: 0.75;"));
        assert!(css.contains("--sk-agent-chat-tool-status-complete: rgb(0 255 0);"));
        assert!(css.contains("--sk-agent-chat-turn-divider: rgb(52 52 52 / 0.0941176471);"));
        assert!(css.contains("--sk-framework-text-sm-font-size: 14px;"));
        // Rejected Agent Chat vars must never exist.
        for rejected in [
            "--sk-agent-chat-footer-dot",
            "--sk-agent-chat-composer-height:",
            "--sk-agent-chat-block-header-opacity:",
            "--sk-agent-chat-md-paragraph-gap",
            "--sk-agent-chat-user-max-width",
            "--sk-agent-chat-assistant-max-width",
            "--sk-emulator-",
        ] {
            assert!(
                !css.contains(rejected),
                "rejected Agent Chat var {rejected} must not exist"
            );
        }
    }
}

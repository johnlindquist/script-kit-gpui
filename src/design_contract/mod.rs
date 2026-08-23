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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContractConflictLifecycleKind {
    IntentionalFact,
    ModelDrift,
    ConsumerDrift,
    EvidencePending,
    Compatibility,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractConflictLifecycle {
    pub kind: ContractConflictLifecycleKind,
    pub owner: String,
    pub intended_contract: String,
    pub model_measurement_id: Option<String>,
    pub render_measurement_id: Option<String>,
    pub task: String,
    pub blocker: Option<String>,
    pub removal_condition: String,
    pub last_receipt: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractConflict {
    pub id: String,
    pub values: BTreeMap<String, String>,
    pub severity: String,
    pub explanation: String,
    pub lifecycle: ContractConflictLifecycle,
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

    #[expect(
        clippy::too_many_arguments,
        reason = "the exporter accepts the seven canonical fields of each persisted design-token record"
    )]
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
        let task = if id.starts_with("confirm") {
            "GEO-003"
        } else if id.starts_with("actions") {
            "GEO-008"
        } else if id.starts_with("notesFooter") {
            "GEO-004"
        } else if id.starts_with("notesMarkdown") {
            "GEO-005"
        } else if id.starts_with("settings") {
            "GEO-006"
        } else if id.starts_with("argPrompt") {
            "GEO-002"
        } else if id.starts_with("rowHeight")
            || id.starts_with("sectionHeader")
            || id.starts_with("selectedFill")
        {
            "GEO-009"
        } else {
            "GOV-005"
        };
        let kind = match severity {
            "high" => ContractConflictLifecycleKind::ConsumerDrift,
            "warning" => ContractConflictLifecycleKind::ModelDrift,
            _ if id.contains("Capture") || id.contains("GlyphExtents") => {
                ContractConflictLifecycleKind::EvidencePending
            }
            _ if id.contains("legacy") || id.contains("Legacy") => {
                ContractConflictLifecycleKind::Compatibility
            }
            _ => ContractConflictLifecycleKind::IntentionalFact,
        };
        let measurement_id = |needles: &[&str], fallback_index: usize| {
            values
                .iter()
                .find(|(key, _)| {
                    let key = key.to_ascii_lowercase();
                    needles.iter().any(|needle| key.contains(needle))
                })
                .or_else(|| values.get(fallback_index))
                .or_else(|| values.first())
                .map(|(key, _)| format!("{id}:{key}"))
        };
        let blocker = matches!(kind, ContractConflictLifecycleKind::EvidencePending)
            .then(|| "Fresh rendered evidence is required before reclassification.".to_string());
        self.conflicts.push(ContractConflict {
            id: id.to_string(),
            values: values
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            severity: severity.to_string(),
            explanation: explanation.to_string(),
            lifecycle: ContractConflictLifecycle {
                kind,
                owner: format!("design-contract:{task}"),
                intended_contract: explanation.to_string(),
                model_measurement_id: measurement_id(&["model", "declared", "source"], 0),
                render_measurement_id: measurement_id(
                    &["render", "paint", "resolved", "measured"],
                    values.len().saturating_sub(1),
                ),
                task: task.to_string(),
                blocker,
                removal_condition: format!(
                    "Remove only when {task} proves all declared values converge or the intentional difference no longer exists."
                ),
                last_receipt: Some(format!(".artifacts/consistency/{task}/task.json")),
            },
        });
    }
}

include!("bundle_header.rs");
include!("bundle_notes.rs");
include!("bundle_settings_day.rs");
include!("bundle_agent_chat.rs");
include!("bundle_prompts.rs");

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
    let list_colors = crate::theme::ListItemColors::from_theme(&theme);
    let row_states = crate::theme::resolve_main_menu_row_state_palette_from_parts(
        crate::theme::MainMenuRowColorInputs {
            row_kind: def.row_kind,
            row_hover_fill_alpha: metrics.row_hover_fill_alpha as u8,
            row_selected_fill_alpha: metrics.row_selected_fill_alpha as u8,
            theme_hover_opacity: list_colors.hover_opacity,
            text_primary_hex: list_colors.text_primary,
            accent_selected_hex: list_colors.accent_selected,
            text_on_accent_hex: list_colors.text_on_accent,
            primary_name_alpha: list_colors.alpha_name as u8,
        },
    );

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

    let info = def.header_info_bar;
    append_header_design_tokens(&mut b, def, &metrics, fill, colors);
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
        row_states
            .hover
            .background_rgba
            .ok_or_else(|| "main-menu hover rows must provide a background fill".to_string())?,
        &["resolved.mainMenu.row.hoverBackground"],
    );
    b.resolved_color(
        "resolved.footer.buttonActive",
        "--sk-footer-button-active",
        row_states
            .active
            .background_rgba
            .ok_or_else(|| "main-menu active rows must provide a background fill".to_string())?,
        &["resolved.mainMenu.row.selectedBackground"],
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
        row_states.rest.primary_foreground_rgba,
        &["resolved.mainMenu.row.primaryForeground"],
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
        (5, 3),
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
            (crate::actions::constants::ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT != 0).to_string(),
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

    append_notes_design_tokens(&mut b, &theme, fm);
    append_settings_and_day_design_tokens(
        &mut b,
        &theme,
        def,
        &chrome,
        &metrics,
        fm,
        default_spacing,
    );
    append_agent_chat_design_tokens(&mut b, &theme, def, colors);
    append_prompt_design_tokens(&mut b, &theme, def, &opacity);
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
include!("mod_tests.rs");

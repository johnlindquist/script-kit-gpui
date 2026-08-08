use crate::ui_foundation::hex_to_rgba_with_opacity;

use super::opacity::{
    OPACITY_GHOST, OPACITY_GHOST_SOFT, OPACITY_WHISPER_BORDER_IDLE,
    OPACITY_WHISPER_SURFACE_FOCUSED, OPACITY_WHISPER_SURFACE_IDLE,
};
use super::Theme;

fn composite_over(fg_hex: u32, alpha: f32, bg_hex: u32) -> u32 {
    let blend = |fg_ch: u32, bg_ch: u32| -> u32 {
        ((fg_ch as f32 * alpha + bg_ch as f32 * (1.0 - alpha)).round() as u32).min(255)
    };
    let r = blend((fg_hex >> 16) & 0xFF, (bg_hex >> 16) & 0xFF);
    let g = blend((fg_hex >> 8) & 0xFF, (bg_hex >> 8) & 0xFF);
    let b = blend(fg_hex & 0xFF, bg_hex & 0xFF);
    (r << 16) | (g << 8) | b
}

/// Shared chrome contract for app surfaces, badges, selection, and hover.
///
/// All color/opacity decisions route through `Theme` — view code consumes
/// resolved RGBA values instead of computing them locally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AppChromeColors {
    pub text_primary_hex: u32,
    pub text_secondary_hex: u32,
    pub text_muted_hex: u32,
    pub text_dimmed_hex: u32,
    pub accent_hex: u32,

    pub text_strong_rgba: u32,
    pub text_muted_rgba: u32,
    pub text_hint_rgba: u32,
    pub text_icon_rgba: u32,

    /// `text_primary` composited with `opacity.text_placeholder` (0.65 by default).
    /// The canonical semantic token for placeholder-tier chrome text: launcher
    /// microcopy hints, empty-state search placeholders, and quiet trailing hints.
    pub placeholder_text_rgba: u32,

    pub window_surface_rgba: u32,
    pub surface_rgba: u32,
    pub input_surface_rgba: u32,
    pub preview_surface_rgba: u32,
    pub panel_surface_rgba: u32,
    pub dialog_surface_rgba: u32,
    /// Low-opacity card/field surface matching form-field whisper chrome.
    pub whisper_surface_rgba: u32,
    /// Focused low-opacity card/field surface matching form-field whisper chrome.
    pub whisper_surface_focused_rgba: u32,
    /// Low-opacity card/field border matching form-field whisper chrome.
    pub whisper_border_rgba: u32,
    /// Dialog surface with vibrancy-aware opacity floor — use for all popup
    /// windows (actions dialog, slash picker, composer picker) so they share
    /// the same apparent background density.
    pub popup_surface_rgba: u32,
    /// Footer-matching ultra-low-opacity surface for inline dropdowns.
    /// Dark: `selected_subtle` at alpha `0x1f` (~12%). Light: opaque main bg.
    pub inline_dropdown_surface_rgba: u32,
    pub log_panel_surface_rgba: u32,
    pub input_active_rgba: u32,
    pub divider_rgba: u32,
    pub border_rgba: u32,

    pub selection_rgba: u32,
    pub hover_rgba: u32,

    pub badge_bg_rgba: u32,
    pub badge_border_rgba: u32,
    pub badge_text_hex: u32,

    pub accent_badge_bg_rgba: u32,
    pub accent_badge_border_rgba: u32,
    pub accent_badge_text_hex: u32,

    pub drop_target_bg_rgba: u32,
    pub drop_target_border_rgba: u32,
    pub drop_target_active_bg_rgba: u32,
    pub drop_target_active_border_rgba: u32,
}

/// Which theme color owns a main-menu row's hover/active fill.
///
/// This lives in the renderer-neutral theme layer so list rows and floating
/// buttons cannot independently choose different fill bases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainMenuRowFillBase {
    TextPrimary,
    Accent,
}

/// Renderer-neutral row state flags. Families keep their own geometry and
/// provide only the paint inputs needed by the shared state resolver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RowStateFlags {
    pub selected: bool,
    pub hovered: bool,
    pub active: bool,
    pub disabled: bool,
}

/// Visual state resolved from [`RowStateFlags`].
///
/// The ordering is deliberate: disabled selection keeps location feedback,
/// disabled rows cannot brighten on hover, and an active/pressed control never
/// dims merely because the pointer remains over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowVisualState {
    Rest,
    Hovered,
    Selected,
    Active,
    Disabled,
    DisabledSelected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowStateColors {
    pub background_rgba: Option<u32>,
    pub primary_foreground_rgba: u32,
    pub secondary_foreground_rgba: u32,
    pub icon_foreground_rgba: u32,
    pub accessory_foreground_rgba: u32,
}

/// Foreground tiers supplied by each row family. This is intentionally paint
/// only: row height, padding, typography, icon sizing, and accessory geometry
/// remain owned by the family renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowForegroundColors {
    pub primary_rgba: u32,
    pub secondary_rgba: u32,
    pub icon_rgba: u32,
    pub accessory_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowStateColorInputs {
    pub rest_background_rgba: Option<u32>,
    pub hover_background_rgba: Option<u32>,
    pub selected_background_rgba: Option<u32>,
    pub active_background_rgba: Option<u32>,
    pub rest_foregrounds: RowForegroundColors,
    pub selected_foregrounds: RowForegroundColors,
    pub disabled_foregrounds: RowForegroundColors,
    pub disabled_selected_foregrounds: RowForegroundColors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowStatePalette {
    pub rest: RowStateColors,
    pub hovered: RowStateColors,
    pub selected: RowStateColors,
    pub active: RowStateColors,
    pub disabled: RowStateColors,
    pub disabled_selected: RowStateColors,
}

impl RowStatePalette {
    pub(crate) fn for_state(self, state: RowVisualState) -> RowStateColors {
        match state {
            RowVisualState::Rest => self.rest,
            RowVisualState::Hovered => self.hovered,
            RowVisualState::Selected => self.selected,
            RowVisualState::Active => self.active,
            RowVisualState::Disabled => self.disabled,
            RowVisualState::DisabledSelected => self.disabled_selected,
        }
    }

    pub(crate) fn for_flags(self, flags: RowStateFlags) -> RowStateColors {
        self.for_state(row_visual_state_from_flags(flags))
    }
}

fn row_state_colors(
    background_rgba: Option<u32>,
    foregrounds: RowForegroundColors,
) -> RowStateColors {
    RowStateColors {
        background_rgba,
        primary_foreground_rgba: foregrounds.primary_rgba,
        secondary_foreground_rgba: foregrounds.secondary_rgba,
        icon_foreground_rgba: foregrounds.icon_rgba,
        accessory_foreground_rgba: foregrounds.accessory_rgba,
    }
}

pub(crate) fn resolve_row_state_palette(inputs: RowStateColorInputs) -> RowStatePalette {
    RowStatePalette {
        rest: row_state_colors(inputs.rest_background_rgba, inputs.rest_foregrounds),
        hovered: row_state_colors(inputs.hover_background_rgba, inputs.rest_foregrounds),
        selected: row_state_colors(inputs.selected_background_rgba, inputs.selected_foregrounds),
        active: row_state_colors(inputs.active_background_rgba, inputs.selected_foregrounds),
        disabled: row_state_colors(inputs.rest_background_rgba, inputs.disabled_foregrounds),
        disabled_selected: row_state_colors(
            inputs.selected_background_rgba,
            inputs.disabled_selected_foregrounds,
        ),
    }
}

pub(crate) fn row_visual_state_from_flags(flags: RowStateFlags) -> RowVisualState {
    if flags.disabled && flags.selected {
        RowVisualState::DisabledSelected
    } else if flags.disabled {
        RowVisualState::Disabled
    } else if flags.active {
        RowVisualState::Active
    } else if flags.selected {
        RowVisualState::Selected
    } else if flags.hovered {
        RowVisualState::Hovered
    } else {
        RowVisualState::Rest
    }
}

/// Semantic state shared by main-menu rows and floating footer buttons.
///
/// Active deliberately wins over hover so an open/selected Actions button
/// cannot dim merely because the pointer is still over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainMenuRowState {
    Rest,
    Hover,
    Active,
}

/// Compatibility name retained for native footer and main-menu consumers.
/// The underlying color record is the shared renderer-neutral row record.
pub(crate) type MainMenuRowStateColors = RowStateColors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MainMenuRowStatePalette {
    pub rest: MainMenuRowStateColors,
    pub hover: MainMenuRowStateColors,
    pub active: MainMenuRowStateColors,
    pub disabled: MainMenuRowStateColors,
    pub disabled_selected: MainMenuRowStateColors,
}

impl MainMenuRowStatePalette {
    pub(crate) fn for_state(self, state: MainMenuRowState) -> MainMenuRowStateColors {
        match state {
            MainMenuRowState::Rest => self.rest,
            MainMenuRowState::Hover => self.hover,
            MainMenuRowState::Active => self.active,
        }
    }

    pub(crate) fn for_flags(self, flags: RowStateFlags) -> MainMenuRowStateColors {
        match row_visual_state_from_flags(flags) {
            RowVisualState::Rest => self.rest,
            RowVisualState::Hovered => self.hover,
            RowVisualState::Selected | RowVisualState::Active => self.active,
            RowVisualState::Disabled => self.disabled,
            RowVisualState::DisabledSelected => self.disabled_selected,
        }
    }
}

/// Primitive row-color inputs used by renderers with effective metric
/// overrides. No renderer type crosses this theme-layer boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MainMenuRowColorInputs {
    pub row_kind: crate::designs::MainMenuRowKind,
    pub row_hover_fill_alpha: u8,
    pub row_selected_fill_alpha: u8,
    pub theme_hover_opacity: f32,
    pub text_primary_hex: u32,
    pub accent_selected_hex: u32,
    pub text_on_accent_hex: u32,
    pub primary_name_alpha: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedMainMenuRowStateFill {
    pub base: MainMenuRowFillBase,
    pub hover_alpha: u8,
    pub active_alpha: u8,
}

pub(crate) fn resolve_main_menu_row_state_fill(
    row_kind: crate::designs::MainMenuRowKind,
    row_hover_fill_alpha: u8,
    row_selected_fill_alpha: u8,
    theme_hover_opacity: f32,
) -> ResolvedMainMenuRowStateFill {
    use crate::designs::MainMenuRowKind as K;

    let base = match row_kind {
        K::IconTile
        | K::GraphitePill
        | K::Smoke
        | K::BlueGlass
        | K::LiquidPrism
        | K::MilkGlass
        | K::SpotlightLuxe => MainMenuRowFillBase::TextPrimary,
        K::WarmGold
        | K::FrostedCommand
        | K::AuroraSlate
        | K::OceanGlass
        | K::StudioPaperGlass
        | K::OperatorMonoGlass
        | K::ProConsole
        | K::CarbonNeon => MainMenuRowFillBase::Accent,
    };
    let hover_alpha = if matches!(row_kind, K::IconTile) {
        // GOV-003: the normalized theme opacity quantizes ONCE, through the
        // named truncating constructor; the byte-domain max then stays in
        // authored-byte space.
        let theme_hover = crate::theme::AlphaByte::from_normalized(theme_hover_opacity);
        crate::theme::AlphaByte::authored(theme_hover.get().max(row_hover_fill_alpha)).get()
    } else {
        row_hover_fill_alpha
    };

    ResolvedMainMenuRowStateFill {
        base,
        hover_alpha,
        active_alpha: row_selected_fill_alpha,
    }
}

pub(crate) fn resolve_main_menu_row_state_palette_from_parts(
    inputs: MainMenuRowColorInputs,
) -> MainMenuRowStatePalette {
    use crate::designs::MainMenuRowKind as K;

    let fill = resolve_main_menu_row_state_fill(
        inputs.row_kind,
        inputs.row_hover_fill_alpha,
        inputs.row_selected_fill_alpha,
        inputs.theme_hover_opacity,
    );
    let fill_hex = match fill.base {
        MainMenuRowFillBase::TextPrimary => inputs.text_primary_hex,
        MainMenuRowFillBase::Accent => inputs.accent_selected_hex,
    };
    let primary_foreground_rgba = crate::theme::alpha::pack_rgb_alpha(
        inputs.text_primary_hex,
        crate::theme::AlphaByte::authored(inputs.primary_name_alpha),
    );
    let active_foreground_hex = match inputs.row_kind {
        K::CarbonNeon => inputs.text_on_accent_hex,
        K::OperatorMonoGlass => inputs.accent_selected_hex,
        _ => inputs.text_primary_hex,
    };
    let active_foreground_rgba = crate::theme::alpha::pack_rgb_alpha(
        active_foreground_hex,
        crate::theme::AlphaByte::authored(0xFF),
    );

    let rest_foregrounds = RowForegroundColors {
        primary_rgba: primary_foreground_rgba,
        secondary_rgba: primary_foreground_rgba,
        icon_rgba: primary_foreground_rgba,
        accessory_rgba: primary_foreground_rgba,
    };
    let selected_foregrounds = RowForegroundColors {
        primary_rgba: active_foreground_rgba,
        secondary_rgba: active_foreground_rgba,
        icon_rgba: active_foreground_rgba,
        accessory_rgba: active_foreground_rgba,
    };
    let shared = resolve_row_state_palette(RowStateColorInputs {
        rest_background_rgba: None,
        hover_background_rgba: Some(crate::theme::alpha::pack_rgb_alpha(
            fill_hex,
            crate::theme::AlphaByte::authored(fill.hover_alpha),
        )),
        selected_background_rgba: Some(crate::theme::alpha::pack_rgb_alpha(
            fill_hex,
            crate::theme::AlphaByte::authored(fill.active_alpha),
        )),
        active_background_rgba: Some(crate::theme::alpha::pack_rgb_alpha(
            fill_hex,
            crate::theme::AlphaByte::authored(fill.active_alpha),
        )),
        rest_foregrounds,
        selected_foregrounds,
        // Main/footer compatibility consumers do not currently expose disabled
        // state, so preserving their rest tier is the exact legacy behavior.
        disabled_foregrounds: rest_foregrounds,
        disabled_selected_foregrounds: rest_foregrounds,
    });

    MainMenuRowStatePalette {
        rest: shared.rest,
        hover: shared.hovered,
        active: shared.active,
        disabled: shared.disabled,
        disabled_selected: shared.disabled_selected,
    }
}

pub(crate) fn resolve_main_menu_row_state_palette(
    theme: &Theme,
    variant: crate::designs::MainMenuThemeVariant,
) -> MainMenuRowStatePalette {
    let def = variant.def();
    let opacity = theme.get_opacity();
    resolve_main_menu_row_state_palette_from_parts(MainMenuRowColorInputs {
        row_kind: def.row_kind,
        row_hover_fill_alpha: def.row.hover_fill_alpha as u8,
        row_selected_fill_alpha: def.row.selected_fill_alpha as u8,
        theme_hover_opacity: opacity.hover,
        text_primary_hex: theme.colors.text.primary,
        accent_selected_hex: theme.colors.accent.selected,
        text_on_accent_hex: theme.colors.text.on_accent,
        primary_name_alpha: crate::theme::AlphaByte::from_normalized(opacity.text_name).get(),
    })
}

pub(crate) fn main_menu_row_state_from_flags(active: bool, hovered: bool) -> MainMenuRowState {
    if active {
        MainMenuRowState::Active
    } else if hovered {
        MainMenuRowState::Hover
    } else {
        MainMenuRowState::Rest
    }
}

/// Contrast-safe colors for semantic status chips (OK, Err, Warn, Info).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SemanticChipColors {
    pub bg_rgba: u32,
    pub border_rgba: u32,
    pub text_hex: u32,
}

/// Danger (destructive/deny) action colors layered over the theme's
/// `ui.error` color with the shared `DANGER_ACTION_*` alphas, so no surface
/// hardcodes a danger hue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DangerActionColors {
    pub rest_rgba: u32,
    pub hover_rgba: u32,
    pub border_rgba: u32,
}

impl DangerActionColors {
    pub(crate) fn from_theme(theme: &Theme) -> Self {
        let error = theme.colors.ui.error;
        // GOV-003: the DANGER_ACTION_* constants are authored bytes typed
        // `u32` in a forbidden owner (`src/ui/chrome/tokens.rs`); wrap them
        // at THIS owned packing boundary with a range assert instead of
        // truncating silently. Typing the constants themselves is an
        // integration request.
        let danger_alpha = |value: u32| -> crate::theme::AlphaByte {
            assert!(value <= 0xFF, "danger action alpha out of byte range");
            crate::theme::AlphaByte::authored(value as u8)
        };
        Self {
            rest_rgba: crate::theme::alpha::pack_rgb_alpha(
                error,
                danger_alpha(crate::ui::chrome::DANGER_ACTION_REST_ALPHA),
            ),
            hover_rgba: crate::theme::alpha::pack_rgb_alpha(
                error,
                danger_alpha(crate::ui::chrome::DANGER_ACTION_HOVER_ALPHA),
            ),
            border_rgba: crate::theme::alpha::pack_rgb_alpha(
                error,
                danger_alpha(crate::ui::chrome::DANGER_ACTION_BORDER_ALPHA),
            ),
        }
    }
}

impl AppChromeColors {
    /// Resolve contrast-safe chip colors for a given semantic base color.
    #[allow(dead_code)] // used by binary target (theme_chooser.rs)
    pub(crate) fn semantic_chip_colors(&self, theme: &Theme, base_hex: u32) -> SemanticChipColors {
        let opacity = theme.get_opacity();
        let bg_alpha = opacity.hover.max(0.18);
        let border_alpha = opacity.selected.max(0.28);
        let text_hex = super::best_readable_text_hex(base_hex);
        tracing::debug!(
            base_hex,
            text_hex,
            bg_alpha,
            border_alpha,
            "theme_semantic_chip_resolved"
        );
        SemanticChipColors {
            bg_rgba: hex_to_rgba_with_opacity(base_hex, bg_alpha),
            border_rgba: hex_to_rgba_with_opacity(base_hex, border_alpha),
            text_hex,
        }
    }

    pub(crate) fn from_theme(theme: &Theme) -> Self {
        let opacity = theme.get_opacity();
        let colors = &theme.colors;
        let accent_badge_bg_rgba = hex_to_rgba_with_opacity(colors.accent.selected, opacity.hover);
        let accent_badge_surface = composite_over(
            colors.accent.selected,
            opacity.hover,
            colors.background.main,
        );

        Self {
            text_primary_hex: colors.text.primary,
            text_secondary_hex: colors.text.secondary,
            text_muted_hex: colors.text.muted,
            text_dimmed_hex: colors.text.dimmed,
            accent_hex: colors.accent.selected,

            text_strong_rgba: hex_to_rgba_with_opacity(colors.text.primary, opacity.text_strong),
            text_muted_rgba: hex_to_rgba_with_opacity(
                colors.text.primary,
                opacity.text_muted_alpha,
            ),
            text_hint_rgba: hex_to_rgba_with_opacity(colors.text.primary, opacity.text_hint),
            text_icon_rgba: hex_to_rgba_with_opacity(colors.text.primary, opacity.text_icon),

            placeholder_text_rgba: hex_to_rgba_with_opacity(
                colors.text.primary,
                opacity.text_placeholder,
            ),

            window_surface_rgba: hex_to_rgba_with_opacity(colors.background.main, opacity.main),
            surface_rgba: hex_to_rgba_with_opacity(colors.background.title_bar, opacity.title_bar),
            input_surface_rgba: hex_to_rgba_with_opacity(
                colors.background.search_box,
                opacity.search_box,
            ),
            preview_surface_rgba: hex_to_rgba_with_opacity(colors.background.main, opacity.preview),
            panel_surface_rgba: hex_to_rgba_with_opacity(
                colors.background.title_bar,
                opacity.panel,
            ),
            dialog_surface_rgba: hex_to_rgba_with_opacity(colors.background.main, opacity.dialog),
            whisper_surface_rgba: hex_to_rgba_with_opacity(
                colors.background.search_box,
                OPACITY_WHISPER_SURFACE_IDLE,
            ),
            whisper_surface_focused_rgba: hex_to_rgba_with_opacity(
                colors.background.main,
                OPACITY_WHISPER_SURFACE_FOCUSED,
            ),
            whisper_border_rgba: hex_to_rgba_with_opacity(
                colors.ui.border,
                OPACITY_WHISPER_BORDER_IDLE,
            ),
            popup_surface_rgba: {
                // Match the actions dialog: use vibrancy_background.
                // so all popup windows share the same apparent density.
                // Opaque mode: near-full floor for readability.
                let popup_opacity = if theme.is_vibrancy_enabled() {
                    opacity
                        .vibrancy_background
                        .unwrap_or(crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND)
                        .clamp(0.0, 1.0)
                } else {
                    opacity.dialog.max(0.95)
                };
                hex_to_rgba_with_opacity(colors.background.main, popup_opacity)
            },
            inline_dropdown_surface_rgba: if theme.is_dark_mode() {
                // Match PromptFooter dark mode: selected_subtle @ ~12% opacity.
                crate::theme::alpha::pack_rgb_alpha(
                    colors.accent.selected_subtle,
                    crate::theme::AlphaByte::authored(0x1f),
                )
            } else {
                // Match PromptFooter light mode: opaque surface.
                crate::theme::alpha::pack_rgb_alpha(
                    colors.background.main,
                    crate::theme::AlphaByte::authored(0xff),
                )
            },
            log_panel_surface_rgba: hex_to_rgba_with_opacity(
                colors.background.log_panel,
                opacity.log_panel,
            ),
            input_active_rgba: hex_to_rgba_with_opacity(
                colors.background.search_box,
                opacity.input_active,
            ),
            divider_rgba: hex_to_rgba_with_opacity(colors.ui.border, opacity.border_inactive),
            border_rgba: hex_to_rgba_with_opacity(
                colors.ui.border,
                opacity.border_active.max(opacity.border_inactive),
            ),

            selection_rgba: hex_to_rgba_with_opacity(colors.text.primary, opacity.selected),
            hover_rgba: hex_to_rgba_with_opacity(colors.text.primary, opacity.hover),

            badge_bg_rgba: hex_to_rgba_with_opacity(
                colors.background.search_box,
                opacity.input_inactive,
            ),
            badge_border_rgba: hex_to_rgba_with_opacity(colors.ui.border, opacity.border_inactive),
            badge_text_hex: colors.text.secondary,

            accent_badge_bg_rgba,
            accent_badge_border_rgba: hex_to_rgba_with_opacity(
                colors.accent.selected,
                opacity.selected,
            ),
            accent_badge_text_hex: super::best_readable_text_hex(accent_badge_surface),

            drop_target_bg_rgba: hex_to_rgba_with_opacity(
                colors.background.search_box,
                OPACITY_GHOST_SOFT,
            ),
            drop_target_border_rgba: hex_to_rgba_with_opacity(colors.ui.border, OPACITY_GHOST),
            drop_target_active_bg_rgba: hex_to_rgba_with_opacity(
                colors.accent.selected_subtle,
                OPACITY_GHOST,
            ),
            drop_target_active_border_rgba: hex_to_rgba_with_opacity(
                colors.accent.selected_subtle,
                OPACITY_GHOST,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        main_menu_row_state_from_flags, resolve_main_menu_row_state_palette,
        resolve_main_menu_row_state_palette_from_parts, resolve_row_state_palette,
        row_visual_state_from_flags, AppChromeColors, MainMenuRowColorInputs, MainMenuRowState,
        RowForegroundColors, RowStateColorInputs, RowStateFlags, RowVisualState,
    };
    use crate::designs::{MainMenuRowKind, MainMenuThemeVariant};
    use crate::theme::Theme;
    use crate::ui_foundation::hex_to_rgba_with_opacity;

    fn custom_row_inputs(row_kind: MainMenuRowKind) -> MainMenuRowColorInputs {
        MainMenuRowColorInputs {
            row_kind,
            row_hover_fill_alpha: 0x12,
            row_selected_fill_alpha: 0x20,
            theme_hover_opacity: 0.0,
            text_primary_hex: 0x112233,
            accent_selected_hex: 0x445566,
            text_on_accent_hex: 0xF0E0D0,
            primary_name_alpha: 0xA1,
        }
    }

    #[test]
    fn main_menu_row_state_palette_matches_info_bar_base_bytes() {
        for theme in [Theme::dark_default(), Theme::light_default()] {
            let palette =
                resolve_main_menu_row_state_palette(&theme, MainMenuThemeVariant::InfoBarBase);
            let primary = theme.colors.text.primary;

            assert_eq!(palette.rest.background_rgba, None);
            assert_eq!(palette.hover.background_rgba, Some((primary << 8) | 0x12));
            assert_eq!(palette.active.background_rgba, Some((primary << 8) | 0x20));
            assert_eq!(palette.rest.primary_foreground_rgba, (primary << 8) | 0xFF);
            assert_eq!(palette.hover.primary_foreground_rgba, (primary << 8) | 0xFF);
            assert_eq!(
                palette.active.primary_foreground_rgba,
                (primary << 8) | 0xFF
            );
        }
    }

    #[test]
    fn main_menu_row_state_palette_respects_text_name_and_hover_overrides() {
        let palette = resolve_main_menu_row_state_palette_from_parts(MainMenuRowColorInputs {
            theme_hover_opacity: 0.22,
            ..custom_row_inputs(MainMenuRowKind::IconTile)
        });

        assert_eq!(palette.rest.primary_foreground_rgba, 0x112233A1);
        assert_eq!(palette.hover.primary_foreground_rgba, 0x112233A1);
        assert_eq!(palette.active.primary_foreground_rgba, 0x112233FF);
        assert_eq!(palette.hover.background_rgba, Some(0x11223338));
        assert_eq!(palette.active.background_rgba, Some(0x11223320));
    }

    #[test]
    fn main_menu_row_state_palette_resolves_accent_row_kinds() {
        let accent = resolve_main_menu_row_state_palette_from_parts(custom_row_inputs(
            MainMenuRowKind::WarmGold,
        ));
        assert_eq!(accent.hover.background_rgba, Some(0x44556612));
        assert_eq!(accent.active.background_rgba, Some(0x44556620));
        assert_eq!(accent.active.primary_foreground_rgba, 0x112233FF);

        let on_accent = resolve_main_menu_row_state_palette_from_parts(custom_row_inputs(
            MainMenuRowKind::CarbonNeon,
        ));
        assert_eq!(on_accent.active.primary_foreground_rgba, 0xF0E0D0FF);

        let accent_text = resolve_main_menu_row_state_palette_from_parts(custom_row_inputs(
            MainMenuRowKind::OperatorMonoGlass,
        ));
        assert_eq!(accent_text.active.primary_foreground_rgba, 0x445566FF);
    }

    #[test]
    fn main_menu_row_state_prefers_active_over_hover() {
        assert_eq!(
            main_menu_row_state_from_flags(true, true),
            MainMenuRowState::Active
        );
        assert_eq!(
            main_menu_row_state_from_flags(false, true),
            MainMenuRowState::Hover
        );
        assert_eq!(
            main_menu_row_state_from_flags(false, false),
            MainMenuRowState::Rest
        );
    }

    #[test]
    fn shared_row_state_precedence_is_disabled_selected_then_active_then_selected_then_hovered() {
        let state = |selected, hovered, active, disabled| {
            row_visual_state_from_flags(RowStateFlags {
                selected,
                hovered,
                active,
                disabled,
            })
        };

        assert_eq!(
            state(true, true, true, true),
            RowVisualState::DisabledSelected
        );
        assert_eq!(state(false, true, true, true), RowVisualState::Disabled);
        assert_eq!(state(true, true, true, false), RowVisualState::Active);
        assert_eq!(state(true, true, false, false), RowVisualState::Selected);
        assert_eq!(state(false, true, false, false), RowVisualState::Hovered);
        assert_eq!(state(false, false, false, false), RowVisualState::Rest);
    }

    #[test]
    fn shared_row_palette_preserves_selected_location_when_disabled_without_hover_brightening() {
        let rest = RowForegroundColors {
            primary_rgba: 0x112233FF,
            secondary_rgba: 0x223344FF,
            icon_rgba: 0x334455FF,
            accessory_rgba: 0x445566FF,
        };
        let selected = RowForegroundColors {
            primary_rgba: 0xAABBCCFF,
            secondary_rgba: 0xBBCCDDEE,
            icon_rgba: 0xCCDDEEFF,
            accessory_rgba: 0xDDEEFFAA,
        };
        let disabled = RowForegroundColors {
            primary_rgba: 0x11223366,
            secondary_rgba: 0x22334466,
            icon_rgba: 0x33445566,
            accessory_rgba: 0x44556666,
        };
        let palette = resolve_row_state_palette(RowStateColorInputs {
            rest_background_rgba: None,
            hover_background_rgba: Some(0xFFFFFF10),
            selected_background_rgba: Some(0xFFFFFF20),
            active_background_rgba: Some(0xFFFFFF30),
            rest_foregrounds: rest,
            selected_foregrounds: selected,
            disabled_foregrounds: disabled,
            disabled_selected_foregrounds: disabled,
        });

        assert_eq!(palette.hovered.background_rgba, Some(0xFFFFFF10));
        assert_eq!(palette.selected.background_rgba, Some(0xFFFFFF20));
        assert_eq!(palette.active.background_rgba, Some(0xFFFFFF30));
        assert_eq!(palette.disabled.background_rgba, None);
        assert_eq!(palette.disabled.primary_foreground_rgba, 0x11223366);
        assert_eq!(palette.disabled_selected.background_rgba, Some(0xFFFFFF20));
        assert_eq!(
            palette.disabled_selected.accessory_foreground_rgba,
            0x44556666
        );
        assert_eq!(
            palette.for_flags(RowStateFlags {
                selected: false,
                hovered: true,
                active: false,
                disabled: true,
            }),
            palette.disabled,
            "hover must never brighten a disabled row"
        );
    }

    #[test]
    fn light_theme_selection_follows_selected_subtle_and_theme_selected_opacity() {
        let theme = Theme::light_default();
        let chrome = AppChromeColors::from_theme(&theme);
        let opacity = theme.get_opacity();

        assert_eq!(
            chrome.selection_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.selected,)
        );
        assert_eq!(
            chrome.hover_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.hover,)
        );
    }

    #[test]
    fn placeholder_text_rgba_uses_shared_text_placeholder_alpha() {
        let theme = Theme::dark_default();
        let chrome = AppChromeColors::from_theme(&theme);
        let opacity = theme.get_opacity();

        assert_eq!(
            chrome.placeholder_text_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.text_placeholder),
            "placeholder text must resolve from text_primary + shared placeholder alpha"
        );
    }

    #[test]
    fn semantic_text_rgba_tokens_use_primary_text_ladder() {
        let theme = Theme::dark_default();
        let chrome = AppChromeColors::from_theme(&theme);
        let opacity = theme.get_opacity();

        assert_eq!(
            chrome.text_strong_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.text_strong)
        );
        assert_eq!(
            chrome.text_muted_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.text_muted_alpha)
        );
        assert_eq!(
            chrome.text_hint_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.text_hint)
        );
        assert_eq!(
            chrome.text_icon_rgba,
            hex_to_rgba_with_opacity(theme.colors.text.primary, opacity.text_icon)
        );
    }

    #[test]
    fn text_dimmed_and_window_surface_resolve_from_theme() {
        let theme = Theme::light_default();
        let chrome = AppChromeColors::from_theme(&theme);
        assert_eq!(chrome.text_dimmed_hex, theme.colors.text.dimmed);
        assert_eq!(
            chrome.window_surface_rgba,
            hex_to_rgba_with_opacity(theme.colors.background.main, theme.get_opacity().main,)
        );
    }

    #[test]
    fn inline_dropdown_surface_matches_footer_contract() {
        let dark = Theme::dark_default();
        let dark_chrome = AppChromeColors::from_theme(&dark);
        assert_eq!(
            dark_chrome.inline_dropdown_surface_rgba,
            (dark.colors.accent.selected_subtle << 8) | 0x1f
        );

        let light = Theme::light_default();
        let light_chrome = AppChromeColors::from_theme(&light);
        assert_eq!(
            light_chrome.inline_dropdown_surface_rgba,
            (light.colors.background.main << 8) | 0xff
        );
    }

    #[test]
    fn whisper_surface_tokens_match_form_field_contract() {
        use crate::theme::opacity::{
            OPACITY_WHISPER_BORDER_IDLE, OPACITY_WHISPER_SURFACE_FOCUSED,
            OPACITY_WHISPER_SURFACE_IDLE,
        };

        for theme in [Theme::dark_default(), Theme::light_default()] {
            let chrome = AppChromeColors::from_theme(&theme);

            assert_eq!(
                chrome.whisper_surface_rgba,
                hex_to_rgba_with_opacity(
                    theme.colors.background.search_box,
                    OPACITY_WHISPER_SURFACE_IDLE,
                ),
                "idle whisper surface must match FormFieldColors::whisper_surface(false)"
            );
            assert_eq!(
                chrome.whisper_surface_focused_rgba,
                hex_to_rgba_with_opacity(
                    theme.colors.background.main,
                    OPACITY_WHISPER_SURFACE_FOCUSED,
                ),
                "focused whisper surface must match FormFieldColors::whisper_surface(true)"
            );
            assert_eq!(
                chrome.whisper_border_rgba,
                hex_to_rgba_with_opacity(theme.colors.ui.border, OPACITY_WHISPER_BORDER_IDLE,),
                "idle whisper border must match FormFieldColors::whisper_surface(false)"
            );
        }
    }

    #[test]
    fn dark_theme_accent_badges_follow_accent_and_hover_selected_opacity() {
        let theme = Theme::dark_default();
        let chrome = AppChromeColors::from_theme(&theme);
        let opacity = theme.get_opacity();

        assert_eq!(
            chrome.accent_badge_bg_rgba,
            hex_to_rgba_with_opacity(theme.colors.accent.selected, opacity.hover)
        );
        assert_eq!(
            chrome.accent_badge_border_rgba,
            hex_to_rgba_with_opacity(theme.colors.accent.selected, opacity.selected)
        );
    }
}

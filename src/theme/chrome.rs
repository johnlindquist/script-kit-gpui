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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MainMenuRowStateColors {
    pub background_rgba: Option<u32>,
    /// One primary foreground serves labels and shortcut glyphs. Keeping one
    /// field makes their parity structural rather than a rendering convention.
    pub primary_foreground_rgba: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MainMenuRowStatePalette {
    pub rest: MainMenuRowStateColors,
    pub hover: MainMenuRowStateColors,
    pub active: MainMenuRowStateColors,
}

impl MainMenuRowStatePalette {
    pub(crate) fn for_state(self, state: MainMenuRowState) -> MainMenuRowStateColors {
        match state {
            MainMenuRowState::Rest => self.rest,
            MainMenuRowState::Hover => self.hover,
            MainMenuRowState::Active => self.active,
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
        ((theme_hover_opacity * 255.0) as u32)
            .max(u32::from(row_hover_fill_alpha))
            .min(u32::from(u8::MAX)) as u8
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
    let primary_foreground_rgba =
        (inputs.text_primary_hex << 8) | u32::from(inputs.primary_name_alpha);
    let active_foreground_hex = match inputs.row_kind {
        K::CarbonNeon => inputs.text_on_accent_hex,
        K::OperatorMonoGlass => inputs.accent_selected_hex,
        _ => inputs.text_primary_hex,
    };
    let active_foreground_rgba = (active_foreground_hex << 8) | 0xFF;

    MainMenuRowStatePalette {
        rest: MainMenuRowStateColors {
            background_rgba: None,
            primary_foreground_rgba,
        },
        hover: MainMenuRowStateColors {
            background_rgba: Some((fill_hex << 8) | u32::from(fill.hover_alpha)),
            primary_foreground_rgba,
        },
        active: MainMenuRowStateColors {
            background_rgba: Some((fill_hex << 8) | u32::from(fill.active_alpha)),
            primary_foreground_rgba: active_foreground_rgba,
        },
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
        primary_name_alpha: crate::theme::types::opacity_to_alpha(opacity.text_name) as u8,
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
        Self {
            rest_rgba: (error << 8) | crate::ui::chrome::DANGER_ACTION_REST_ALPHA,
            hover_rgba: (error << 8) | crate::ui::chrome::DANGER_ACTION_HOVER_ALPHA,
            border_rgba: (error << 8) | crate::ui::chrome::DANGER_ACTION_BORDER_ALPHA,
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
                (colors.accent.selected_subtle << 8) | 0x1f
            } else {
                // Match PromptFooter light mode: opaque surface.
                (colors.background.main << 8) | 0xff
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
        resolve_main_menu_row_state_palette_from_parts, AppChromeColors, MainMenuRowColorInputs,
        MainMenuRowState,
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

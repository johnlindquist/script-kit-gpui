const ACTIONS_DIALOG_COLOR_ALPHA_MAX: f32 = 255.0;
const ACTIONS_DIALOG_SEARCH_BORDER_ALPHA_SCALE: f32 = 2.0;
const ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA: u8 = 0x80;
const ACTIONS_DIALOG_OPAQUE_DIALOG_MIN_OPACITY: f32 = 0.95;
// The actions dialog renders in its own native NSPanel with a real
// NSVisualEffectView blur layer.  A low opacity floor lets the system
// blur show through prominently while still tinting the background
// enough for text contrast.
const ACTIONS_DIALOG_VIBRANT_INLINE_MIN_OPACITY: f32 = 0.25;

fn actions_dialog_alpha_u8(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * ACTIONS_DIALOG_COLOR_ALPHA_MAX) as u8
}

fn actions_dialog_search_border_alpha(border_inactive_opacity: f32) -> u8 {
    let scaled_border_opacity =
        (border_inactive_opacity * ACTIONS_DIALOG_SEARCH_BORDER_ALPHA_SCALE).min(1.0);
    actions_dialog_alpha_u8(scaled_border_opacity)
}

fn actions_dialog_container_border_alpha(border_inactive_opacity: f32) -> u8 {
    actions_dialog_search_border_alpha(border_inactive_opacity)
        .max(ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA)
}

fn actions_dialog_container_background_alpha(dialog_opacity: f32, use_vibrancy: bool) -> u8 {
    // The actions dialog has its own native NSPanel with NSVisualEffectView,
    // so a low opacity floor lets the system blur show through prominently.
    // Opaque (non-vibrancy) mode keeps a near-full readability floor.
    let resolved_opacity = if use_vibrancy {
        dialog_opacity.max(ACTIONS_DIALOG_VIBRANT_INLINE_MIN_OPACITY)
    } else {
        dialog_opacity.max(ACTIONS_DIALOG_OPAQUE_DIALOG_MIN_OPACITY)
    };
    actions_dialog_alpha_u8(resolved_opacity)
}

fn actions_dialog_rgba_with_alpha(hex: u32, alpha: u8) -> gpui::Rgba {
    rgba(hex_with_alpha(hex, alpha))
}

#[inline]
fn semantic_text_rgba(text_primary: u32, opacity: f32) -> gpui::Rgba {
    rgba(hex_with_alpha(
        text_primary,
        actions_dialog_alpha_u8(opacity.clamp(0.0, 1.0)),
    ))
}

#[inline]
fn actions_dialog_search_text_colors(
    text_primary: u32,
    opacity: &BackgroundOpacity,
) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    (
        semantic_text_rgba(text_primary, opacity.text_muted_alpha),
        semantic_text_rgba(text_primary, opacity.text_placeholder),
        semantic_text_rgba(text_primary, opacity.text_strong),
    )
}

#[inline]
fn actions_dialog_container_text_color(
    text_primary: u32,
    opacity: &BackgroundOpacity,
) -> gpui::Rgba {
    semantic_text_rgba(text_primary, opacity.text_muted_alpha)
}

fn actions_dialog_main_window_background_alpha(theme: &theme::Theme) -> u8 {
    let popup_surface = AppChromeColors::from_theme(theme).popup_surface_rgba;
    (popup_surface & 0xff) as u8
}

#[cfg(test)]
mod actions_dialog_opacity_consistency_tests {
    use super::{
        actions_dialog_container_background_alpha, actions_dialog_container_border_alpha,
        actions_dialog_container_text_color, actions_dialog_main_window_background_alpha,
        actions_dialog_rgba_with_alpha, actions_dialog_search_border_alpha,
        actions_dialog_search_text_colors, semantic_text_rgba,
        ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA,
    };
    use crate::theme::{AppChromeColors, Theme};
    use gpui::rgba;

    #[test]
    fn test_actions_dialog_search_border_alpha_scales_border_inactive_opacity() {
        assert_eq!(actions_dialog_search_border_alpha(0.20), 102);
    }

    #[test]
    fn test_actions_dialog_container_border_alpha_enforces_minimum_contrast() {
        assert_eq!(
            actions_dialog_container_border_alpha(0.10),
            ACTIONS_DIALOG_CONTAINER_BORDER_MIN_ALPHA
        );
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_uses_vibrant_floor() {
        // 0.15 dialog opacity is clamped up to 0.25 vibrant floor → 63
        assert_eq!(actions_dialog_container_background_alpha(0.15, true), 63);
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_keeps_non_vibrancy_floor() {
        assert_eq!(actions_dialog_container_background_alpha(0.15, false), 242);
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_passes_through_above_floor() {
        // 0.80 is above the 0.25 vibrant floor → passes through → 204
        assert_eq!(actions_dialog_container_background_alpha(0.80, true), 204);
    }

    #[test]
    fn test_actions_dialog_container_background_alpha_uses_higher_theme_value_above_floor() {
        // 0.90 is above the 0.25 floor → passes through → 229
        assert_eq!(actions_dialog_container_background_alpha(0.90, true), 229);
    }

    #[test]
    fn test_actions_dialog_main_window_background_alpha_matches_dark_window_default() {
        let theme = Theme::dark_default();
        let expected = (crate::theme::opacity::OPACITY_VIBRANCY_BACKGROUND * 255.0) as u8;
        assert_eq!(
            actions_dialog_main_window_background_alpha(&theme),
            expected
        );
    }

    #[test]
    fn test_actions_dialog_main_window_background_alpha_uses_shared_popup_surface_token() {
        let mut theme = Theme::light_default();
        let mut opacity = theme.get_opacity();
        opacity.vibrancy_background = Some(0.40);
        theme.opacity = Some(opacity);

        assert_eq!(
            actions_dialog_main_window_background_alpha(&theme),
            (AppChromeColors::from_theme(&theme).popup_surface_rgba & 0xff) as u8
        );
    }

    #[test]
    fn test_actions_dialog_rgba_with_alpha_combines_hex_and_alpha_channels() {
        let theme = Theme::default();
        let background = theme.colors.background.main;

        assert_eq!(
            actions_dialog_rgba_with_alpha(background, 0x44),
            rgba((background << 8) | 0x44)
        );
    }

    #[test]
    fn test_actions_dialog_search_and_container_text_follow_shared_theme_opacity_ladder() {
        let theme = Theme::dark_default();
        let opacity = theme.get_opacity();
        let (muted_text, hint_text, strong_text) =
            actions_dialog_search_text_colors(theme.colors.text.primary, &opacity);
        let container_text =
            actions_dialog_container_text_color(theme.colors.text.primary, &opacity);

        assert_eq!(
            muted_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_muted_alpha),
            "search muted text must use primary text plus shared muted alpha"
        );
        assert_eq!(
            hint_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_placeholder),
            "search hint text must use primary text plus shared placeholder alpha"
        );
        assert_eq!(
            strong_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_strong),
            "search strong text must use primary text plus shared strong alpha"
        );
        assert_eq!(
            container_text,
            semantic_text_rgba(theme.colors.text.primary, opacity.text_muted_alpha),
            "container text must use primary text plus shared muted alpha"
        );
    }
}

use super::types::{
    BUTTON_CONTENT_GAP_PX, BUTTON_GHOST_PADDING_X, BUTTON_GHOST_PADDING_Y, BUTTON_ICON_PADDING_X,
    BUTTON_ICON_PADDING_Y, BUTTON_PRIMARY_PADDING_X, BUTTON_PRIMARY_PADDING_Y, BUTTON_RADIUS_PX,
    BUTTON_SHORTCUT_MARGIN_LEFT_PX,
};
use super::{Button, ButtonColors, ButtonVariant};
use crate::designs::DesignColors;
use crate::theme::Theme;
use gpui::SharedString;

#[test]
fn test_should_show_pointer_only_when_button_is_interactive() {
    assert!(Button::should_show_pointer(true, false, false));
    assert!(!Button::should_show_pointer(false, false, false));
    assert!(!Button::should_show_pointer(true, true, false));
    assert!(!Button::should_show_pointer(true, false, true));
}

#[test]
fn test_can_activate_from_key_requires_interactive_activation_key() {
    assert!(Button::can_activate_from_key("enter", true, false, false));
    assert!(Button::can_activate_from_key(" ", true, false, false));
    assert!(!Button::can_activate_from_key("x", true, false, false));
    assert!(!Button::can_activate_from_key("enter", false, false, false));
    assert!(!Button::can_activate_from_key("enter", true, true, false));
    assert!(!Button::can_activate_from_key("enter", true, false, true));
}

#[test]
fn test_resolve_element_id_prefers_explicit_id_when_present() {
    let label: SharedString = "Run".into();
    let explicit_id: SharedString = "footer-run".into();

    assert_eq!(
        Button::resolve_element_id(Some(&explicit_id), &label),
        explicit_id
    );
    assert_eq!(Button::resolve_element_id(None, &label), label);
}

#[test]
fn test_button_colors_from_theme_uses_selected_subtle_for_hover_overlay() {
    let mut theme = Theme::dark_default();
    theme.colors.accent.selected_subtle = 0x112233;
    let mut opacity = theme.get_opacity();
    opacity.hover = 0.22;
    theme.opacity = Some(opacity);

    let colors = ButtonColors::from_theme(&theme);
    assert_eq!(colors.hover_overlay, 0x11223338);
}

#[test]
fn test_button_colors_from_design_uses_design_background_for_hover_overlay() {
    let design = DesignColors {
        background_selected: 0x445566,
        ..Default::default()
    };

    let colors = ButtonColors::from_design_with_dark_mode(&design, true);
    assert_eq!(colors.hover_overlay, 0x4455662e);
}

#[test]
fn test_button_colors_from_theme_uses_light_theme_hover_opacity() {
    let mut theme = Theme::light_default();
    theme.colors.accent.selected_subtle = 0x112233;

    let colors = ButtonColors::from_theme(&theme);
    assert_eq!(colors.hover_overlay, 0x1122330a);
}

#[test]
fn test_hover_background_token_uses_background_hover_for_primary_variant() {
    let colors = ButtonColors {
        background_hover: 0x123456,
        hover_overlay: 0xabcdef26,
        ..ButtonColors::default()
    };

    assert_eq!(
        Button::hover_background_token(ButtonVariant::Primary, colors),
        0x123456b0
    );
}

#[test]
fn test_hover_background_token_uses_overlay_for_ghost_and_icon_variants() {
    let colors = ButtonColors {
        background_hover: 0x123456,
        hover_overlay: 0xabcdef26,
        ..ButtonColors::default()
    };

    assert_eq!(
        Button::hover_background_token(ButtonVariant::Ghost, colors),
        0xabcdef26
    );
    assert_eq!(
        Button::hover_background_token(ButtonVariant::Icon, colors),
        0xabcdef26
    );
}

#[test]
fn test_resolve_focus_state_prefers_runtime_focus_handle_state() {
    assert!(Button::resolve_focus_state(false, Some(true)));
    assert!(!Button::resolve_focus_state(true, Some(false)));
    assert!(Button::resolve_focus_state(true, None));
    assert!(!Button::resolve_focus_state(false, None));
}

#[test]
fn test_should_show_focus_indicator_does_not_render_for_non_interactive_states() {
    assert!(Button::should_show_focus_indicator(
        true, true, false, false
    ));
    assert!(!Button::should_show_focus_indicator(
        false, true, false, false
    ));
    assert!(!Button::should_show_focus_indicator(
        true, false, false, false
    ));
    assert!(!Button::should_show_focus_indicator(
        true, true, true, false
    ));
    assert!(!Button::should_show_focus_indicator(
        true, true, false, true
    ));
}

#[test]
fn test_button_shortcuts_use_canonical_shared_tokens() {
    assert_eq!(Button::resolve_shortcut_tokens("Cmd+K"), vec!["⌘", "K"]);
    assert_eq!(Button::resolve_shortcut_tokens("cmd++"), vec!["⌘", "+"]);
    assert_eq!(Button::resolve_shortcut_tokens("ctrl+\\"), vec!["⌃", "\\"]);
}

#[test]
fn test_button_layout_tokens_stay_consistent_when_render_spacing_is_updated() {
    assert_eq!(BUTTON_PRIMARY_PADDING_X, 12.0);
    assert_eq!(BUTTON_PRIMARY_PADDING_Y, 6.0);
    assert_eq!(BUTTON_GHOST_PADDING_X, 8.0);
    assert_eq!(BUTTON_GHOST_PADDING_Y, 4.0);
    assert_eq!(BUTTON_ICON_PADDING_X, 6.0);
    assert_eq!(BUTTON_ICON_PADDING_Y, 6.0);
    assert_eq!(BUTTON_CONTENT_GAP_PX, 2.0);
    assert_eq!(BUTTON_SHORTCUT_MARGIN_LEFT_PX, 4.0);
    // BUTTON_RADIUS_PX is intentionally the shared Liquid Glass compact radius
    // (6fe004fcc re-pointed it at LIQUID_GLASS_COMPACT_RADIUS_PX = 10.0;
    // tests/liquid_glass_chrome_token_dedrift_contract.rs guards the token reference).
    assert_eq!(BUTTON_RADIUS_PX, 10.0);
}

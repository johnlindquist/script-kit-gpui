use std::time::Duration;

use crate::components::overlay_modal::{
    compute_overlay_appear_style, OVERLAY_ANIMATION_DURATION_MS,
};
use crate::theme::Theme;

use super::types::{
    is_clear_alias_shortcut, is_command_modifier, validate_alias_input, ALIAS_INPUT_PLACEHOLDER,
};
use super::{AliasInputAction, AliasInputColors, AliasValidationError};

#[gpui::test]
fn retained_alias_semantics_follow_edits_not_theme_or_inspection(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let input = cx.new(|cx| {
        super::AliasInput::new(cx, std::sync::Arc::new(Theme::default()))
            .with_command_id("builtin/clipboard-history")
            .with_command_name("Clipboard History")
            .with_current_alias(Some("alpha".into()))
    });
    input.update(cx, |input, _| {
        let before = input.semantic_token();
        input.input.set_text("bravo");
        let edited = input.semantic_token();
        assert_ne!(before, edited, "equal-length edits change source semantics");
        let elements = input.automation_elements();
        assert_eq!(elements, input.automation_elements());
        assert_eq!(input.semantic_token(), edited);
        let mut theme = Theme::default();
        theme.colors.background.main = 0x123456;
        input.update_theme(std::sync::Arc::new(theme));
        assert_eq!(
            input.semantic_token(),
            edited,
            "appearance is not an alias edit"
        );
    });
}

#[gpui::test]
fn alias_semantic_controls_use_real_validation_and_pending_actions(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let input = cx.new(|cx| {
        super::AliasInput::new(cx, std::sync::Arc::new(Theme::default()))
            .with_current_alias(Some("clip".into()))
    });
    input.update(cx, |input, cx| {
        input.input.set_text("two words");
        let save = input.automation_elements().into_iter().find(|element| element.semantic_id == "alias-input:save").unwrap();
        assert_eq!(save.action_disabled.as_deref(), Some("alias_validation_failed"));
        assert!(input.activate_semantic_action("alias-input:save", cx).is_err());
        assert!(input.take_pending_action().is_none());
        input.input.set_text("release");
        assert_eq!(input.activate_semantic_action("alias-input:save", cx), Ok(true));
        input.automation_elements();
        assert!(matches!(input.take_pending_action(), Some(AliasInputAction::Save(value)) if value == "release"));
        assert!(input.take_pending_action().is_none());
        assert_eq!(input.activate_semantic_action("alias-input:clear", cx), Ok(true));
        assert!(matches!(input.take_pending_action(), Some(AliasInputAction::Clear)));
        input.set_current_alias(None);
        assert!(!input.automation_elements().iter().any(|element| element.semantic_id == "alias-input:clear"));
        assert_eq!(input.activate_semantic_action("alias-input:clear", cx), Ok(false));
    });
}

#[test]
fn test_alias_input_colors_default() {
    let colors = AliasInputColors::default();
    assert_eq!(colors.accent, 0xfbbf24);
    assert_eq!(colors.text_primary, 0xffffff);
}

#[test]
fn test_alias_input_colors_from_theme_uses_theme_overlay_token() {
    let mut theme = Theme::default();
    theme.colors.background.main = 0x1a2b3c;

    let colors = AliasInputColors::from_theme(&theme);
    assert_eq!(colors.overlay_bg, 0x1a2b3c);
}

#[test]
fn test_alias_input_action_variants() {
    let save_action = AliasInputAction::Save("test".to_string());
    let cancel_action = AliasInputAction::Cancel;
    let clear_action = AliasInputAction::Clear;

    assert!(matches!(save_action, AliasInputAction::Save(_)));
    assert!(matches!(cancel_action, AliasInputAction::Cancel));
    assert!(matches!(clear_action, AliasInputAction::Clear));
}

#[test]
fn test_alias_placeholder_copy_is_clear() {
    assert_eq!(ALIAS_INPUT_PLACEHOLDER, "e.g. ch, clip, todo");
}

#[test]
fn test_validate_alias_rejects_empty_and_whitespace() {
    assert!(matches!(
        validate_alias_input("   "),
        Err(AliasValidationError::Empty)
    ));
    assert!(matches!(
        validate_alias_input("two words"),
        Err(AliasValidationError::ContainsWhitespace)
    ));
}

#[test]
fn test_validate_alias_accepts_trimmed_valid_input() {
    assert_eq!(
        validate_alias_input("  clip  ").expect("alias should be valid"),
        "clip"
    );
}

#[test]
fn test_validate_alias_rejects_invalid_characters() {
    assert!(matches!(
        validate_alias_input("clip!"),
        Err(AliasValidationError::InvalidCharacters)
    ));
    assert!(matches!(
        validate_alias_input("clip.history"),
        Err(AliasValidationError::InvalidCharacters)
    ));
}

#[test]
fn test_alias_command_modifier_uses_platform_or_control() {
    assert!(is_command_modifier(true, false));
    assert!(is_command_modifier(false, true));
    assert!(!is_command_modifier(false, false));
}

#[test]
fn test_alias_clear_shortcut_requires_modifier_and_existing_alias() {
    assert!(is_clear_alias_shortcut("backspace", true, true));
    assert!(is_clear_alias_shortcut("delete", true, true));
    assert!(!is_clear_alias_shortcut("backspace", false, true));
    assert!(!is_clear_alias_shortcut("backspace", true, false));
}

#[test]
fn test_compute_overlay_appear_style_starts_hidden_offset_and_transparent() {
    let style = compute_overlay_appear_style(Duration::from_millis(0));
    assert_eq!(style.backdrop_opacity, 0.0);
    assert!(style.modal_offset_y > 0.0);
    assert!(style.modal_opacity < 1.0);
    assert!(!style.complete);
}

#[test]
fn test_compute_overlay_appear_style_reaches_full_visibility_after_duration() {
    let style = compute_overlay_appear_style(Duration::from_millis(OVERLAY_ANIMATION_DURATION_MS));
    assert!((style.backdrop_opacity - 1.0).abs() < 0.001);
    assert!((style.modal_offset_y - 0.0).abs() < 0.001);
    assert!((style.modal_opacity - 1.0).abs() < 0.001);
    assert!(style.complete);
}

#[test]
fn test_input_hover_border_token_uses_accent_color_with_visible_alpha() {
    let colors = AliasInputColors {
        accent: 0x123456,
        ..Default::default()
    };

    assert_eq!(
        super::AliasInput::input_hover_border_token(colors),
        0x12345690
    );
}

#[test]
fn test_backdrop_hover_bg_token_uses_overlay_color_with_hover_alpha() {
    let colors = AliasInputColors {
        overlay_bg: 0x654321,
        ..Default::default()
    };

    assert_eq!(
        super::AliasInput::backdrop_hover_bg_token(colors),
        0x65432196
    );
}

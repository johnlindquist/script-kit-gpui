use super::types::{
    BUTTON_CONTENT_GAP_PX, BUTTON_GHOST_PADDING_X, BUTTON_GHOST_PADDING_Y, BUTTON_ICON_PADDING_X,
    BUTTON_ICON_PADDING_Y, BUTTON_PRIMARY_PADDING_X, BUTTON_PRIMARY_PADDING_Y, BUTTON_RADIUS_PX,
    BUTTON_SHORTCUT_MARGIN_LEFT_PX,
};
use super::{Button, ButtonColors, ButtonVariant};
use crate::designs::DesignColors;
use crate::theme::Theme;
use gpui::{
    App, AppContext as _, ClickEvent, Context, FocusHandle, IntoElement, Keystroke, Render,
    TestAppContext, Window,
};
use std::{cell::Cell, rc::Rc};

struct KeyboardButtonProbe {
    focus_handle: FocusHandle,
    clicks: Rc<Cell<usize>>,
    disabled: bool,
    loading: bool,
}

impl Render for KeyboardButtonProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.clicks.clone();
        Button::new(
            "keyboard-probe:activate",
            "Duplicate",
            ButtonColors::default(),
        )
        .focus_handle(self.focus_handle.clone())
        .disabled(self.disabled)
        .loading(self.loading)
        .on_click(Box::new(
            move |_: &ClickEvent, _: &mut Window, _: &mut App| {
                clicks.set(clicks.get() + 1);
            },
        ))
    }
}

fn dispatch_to_keyboard_probe(disabled: bool, loading: bool, keys: &[&str]) -> usize {
    let clicks = Rc::new(Cell::new(0));
    let probe_clicks = clicks.clone();
    let mut cx = TestAppContext::single();
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|cx| KeyboardButtonProbe {
                focus_handle: cx.focus_handle(),
                clicks: probe_clicks,
                disabled,
                loading,
            })
        })
        .expect("keyboard button test window should open")
    });

    window
        .update(&mut cx, |probe, window, cx| {
            window.focus(&probe.focus_handle, cx);
        })
        .expect("keyboard button test window should remain available");

    for key in keys {
        cx.dispatch_keystroke(
            *window,
            Keystroke::parse(key).expect("valid test keystroke"),
        );
    }
    clicks.get()
}

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
fn real_gpui_enter_and_space_dispatch_once_each_to_the_focused_button() {
    assert_eq!(
        dispatch_to_keyboard_probe(false, false, &["enter", "space"]),
        2
    );
}

#[test]
fn real_gpui_keyboard_dispatch_keeps_disabled_and_loading_buttons_inert() {
    assert_eq!(
        dispatch_to_keyboard_probe(true, false, &["enter", "space"]),
        0
    );
    assert_eq!(
        dispatch_to_keyboard_probe(false, true, &["enter", "space"]),
        0
    );
}

#[test]
fn button_identity_is_required_and_independent_from_duplicate_labels() {
    let colors = ButtonColors::default();
    let first = Button::new("dialog-a:open", "Open", colors);
    let second = Button::new("dialog-b:open", "Open", colors);

    assert_eq!(first.stable_id(), "dialog-a:open");
    assert_eq!(second.stable_id(), "dialog-b:open");
    assert_ne!(first.stable_id(), second.stable_id());
}

#[test]
fn changing_button_label_does_not_change_identity() {
    let button =
        Button::new("download:primary", "Download", ButtonColors::default()).label("Downloading…");

    assert_eq!(button.stable_id(), "download:primary");
    assert_eq!(button.display_label(), "Downloading…");
}

#[test]
#[should_panic(expected = "Button stable ID must not be empty")]
fn empty_button_identity_is_rejected() {
    let _ = Button::new("", "Open", ButtonColors::default());
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

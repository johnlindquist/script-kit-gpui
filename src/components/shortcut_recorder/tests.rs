use crate::theme::Theme;
use gpui::{AppContext as _, Keystroke, TestAppContext};
use std::sync::Arc;

use super::types::{RecorderAction, ShortcutRecorderFocusedAction};
use super::{RecordedShortcut, ShortcutRecorder, ShortcutRecorderColors};

fn recorder_window(cx: &mut TestAppContext) -> gpui::WindowHandle<ShortcutRecorder> {
    cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|cx| ShortcutRecorder::new(cx, Arc::new(Theme::default())))
        })
        .expect("shortcut recorder test window should open")
    })
}

#[test]
fn real_gpui_tab_skips_disabled_actions_and_enter_cancels() {
    let mut cx = TestAppContext::single();
    let window = recorder_window(&mut cx);
    window
        .update(&mut cx, |recorder, window, cx| {
            window.focus(&recorder.focus_handle, cx);
        })
        .expect("shortcut recorder test window should remain available");

    cx.dispatch_keystroke(
        *window,
        Keystroke::parse("tab").expect("valid Tab keystroke"),
    );
    cx.dispatch_keystroke(
        *window,
        Keystroke::parse("enter").expect("valid Enter keystroke"),
    );

    window
        .read_with(&cx, |recorder, _| {
            assert_eq!(
                recorder.focused_action,
                ShortcutRecorderFocusedAction::Cancel
            );
            assert!(matches!(
                recorder.pending_action,
                Some(RecorderAction::Cancel)
            ));
        })
        .expect("shortcut recorder test window should remain available");
}

#[test]
fn real_gpui_chord_then_enter_dispatches_save() {
    let mut cx = TestAppContext::single();
    let window = recorder_window(&mut cx);
    window
        .update(&mut cx, |recorder, window, cx| {
            window.focus(&recorder.focus_handle, cx);
        })
        .expect("shortcut recorder test window should remain available");

    cx.dispatch_keystroke(
        *window,
        Keystroke::parse("cmd-k").expect("valid shortcut chord"),
    );
    cx.dispatch_keystroke(
        *window,
        Keystroke::parse("enter").expect("valid Enter keystroke"),
    );

    window
        .read_with(&cx, |recorder, _| {
            assert_eq!(recorder.focused_action, ShortcutRecorderFocusedAction::Save);
            assert!(matches!(
                recorder.pending_action,
                Some(RecorderAction::Save(_))
            ));
        })
        .expect("shortcut recorder test window should remain available");
}

#[test]
fn test_recorded_shortcut_to_display_string() {
    let mut shortcut = RecordedShortcut::new();
    shortcut.cmd = true;
    shortcut.shift = true;
    shortcut.key = Some("K".to_string());

    assert_eq!(shortcut.to_display_string(), "⌘⇧K");
}

#[test]
fn test_recorded_shortcut_to_config_string() {
    let mut shortcut = RecordedShortcut::new();
    shortcut.cmd = true;
    shortcut.shift = true;
    shortcut.key = Some("K".to_string());

    assert_eq!(shortcut.to_config_string(), "cmd+shift+k");
}

#[test]
fn test_recorded_shortcut_is_empty() {
    let shortcut = RecordedShortcut::new();
    assert!(shortcut.is_empty());

    let mut shortcut_with_mod = RecordedShortcut::new();
    shortcut_with_mod.cmd = true;
    assert!(!shortcut_with_mod.is_empty());
}

#[test]
fn test_recorded_shortcut_is_complete() {
    let mut shortcut = RecordedShortcut::new();
    shortcut.cmd = true;
    assert!(!shortcut.is_complete()); // No key yet

    shortcut.key = Some("K".to_string());
    assert!(shortcut.is_complete()); // Has modifier + key
}

#[test]
fn test_recorded_shortcut_to_keycaps() {
    let mut shortcut = RecordedShortcut::new();
    shortcut.ctrl = true;
    shortcut.alt = true;
    shortcut.shift = true;
    shortcut.cmd = true;
    shortcut.key = Some("K".to_string());

    let keycaps = shortcut.to_keycaps();
    assert_eq!(keycaps, vec!["⌃", "⌥", "⌘", "⇧", "K"]);
}

#[test]
fn test_format_key_display_special_keys() {
    assert_eq!(RecordedShortcut::format_key_display("enter"), "↵");
    assert_eq!(RecordedShortcut::format_key_display("escape"), "⎋");
    assert_eq!(RecordedShortcut::format_key_display("tab"), "⇥");
    assert_eq!(RecordedShortcut::format_key_display("backspace"), "⌫");
    assert_eq!(RecordedShortcut::format_key_display("space"), "␣");
    assert_eq!(RecordedShortcut::format_key_display("up"), "↑");
    assert_eq!(RecordedShortcut::format_key_display("arrowdown"), "↓");
}

#[test]
fn test_shortcut_recorder_colors_default() {
    let colors = ShortcutRecorderColors::default();
    assert_eq!(colors.accent, 0xfbbf24);
    assert_eq!(colors.warning, 0xf59e0b);
}

#[test]
fn test_shortcut_recorder_colors_from_theme_uses_theme_overlay_token() {
    let mut theme = Theme::default();
    theme.colors.background.main = 0x2b3c4d;

    let colors = ShortcutRecorderColors::from_theme(&theme);
    assert_eq!(colors.overlay_bg, 0x2b3c4d);
}

#[test]
fn test_shortcut_recorder_focus_cycles_forward_through_enabled_actions() {
    let mut focused = ShortcutRecorderFocusedAction::Save;

    focused = focused.next(true, true);
    assert_eq!(focused, ShortcutRecorderFocusedAction::Clear);

    focused = focused.next(true, true);
    assert_eq!(focused, ShortcutRecorderFocusedAction::Cancel);

    focused = focused.next(true, true);
    assert_eq!(focused, ShortcutRecorderFocusedAction::Save);
}

#[test]
fn test_shortcut_recorder_focus_cycles_backward_through_enabled_actions() {
    let mut focused = ShortcutRecorderFocusedAction::Save;

    focused = focused.previous(true, true);
    assert_eq!(focused, ShortcutRecorderFocusedAction::Cancel);

    focused = focused.previous(true, true);
    assert_eq!(focused, ShortcutRecorderFocusedAction::Clear);

    focused = focused.previous(true, true);
    assert_eq!(focused, ShortcutRecorderFocusedAction::Save);
}

#[test]
fn shortcut_recorder_focus_skips_unavailable_save_and_clear() {
    assert_eq!(
        ShortcutRecorderFocusedAction::Save.next(false, false),
        ShortcutRecorderFocusedAction::Cancel
    );
    assert_eq!(
        ShortcutRecorderFocusedAction::Clear.previous(false, false),
        ShortcutRecorderFocusedAction::Cancel
    );
    assert_eq!(
        ShortcutRecorderFocusedAction::eligible(false, false),
        vec![ShortcutRecorderFocusedAction::Cancel]
    );
}

#[test]
fn shortcut_recorder_focus_skips_only_the_unavailable_action() {
    assert_eq!(
        ShortcutRecorderFocusedAction::Save.next(false, true),
        ShortcutRecorderFocusedAction::Clear
    );
    assert_eq!(
        ShortcutRecorderFocusedAction::Clear.next(true, false),
        ShortcutRecorderFocusedAction::Save
    );
}

/// Handle the current Notes ghost autocomplete prediction through the live
/// Notes window, for target-scoped DevTools `simulateKey` proof.
pub fn handle_notes_ghost_key_for_automation(
    cx: &mut App,
    key: &str,
) -> Result<serde_json::Value, String> {
    let (entity, handle) =
        get_notes_app_entity_and_handle().ok_or_else(|| "Notes window is not open".to_string())?;
    // Must not lease Root: the escape ladder reaches `close_actions_panel` →
    // `request_focus_surface`, which reads Root via `window.has_active_dialog`.
    update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let key = key.to_ascii_lowercase();
            let (action, handled) = match key.as_str() {
                "escape" | "esc" => app.escape_dismiss_ladder(window, cx),
                "tab" => (
                    "acceptNotesGhostWord",
                    app.try_accept_notes_ghost(
                        super::keyboard::NotesGhostAcceptMode::Word,
                        window,
                        cx,
                    ),
                ),
                "`" | "backtick" => (
                    "acceptNotesGhostFull",
                    app.try_accept_notes_ghost(
                        super::keyboard::NotesGhostAcceptMode::Full,
                        window,
                        cx,
                    ),
                ),
                _ => ("unsupportedNotesGhostKey", false),
            };
            serde_json::json!({
                "handled": handled,
                "target": "notes",
                "action": action,
            })
        })
    })
    .map_err(|error| format!("Failed to handle Notes ghost autocomplete key: {error}"))
}

/// Handle Notes editor keys that automation cannot route through the root
/// `capture_key_down` path, while still calling the same surface methods as the
/// live keyboard handler.
pub fn handle_notes_editor_key_for_automation(
    cx: &mut App,
    key: &str,
    platform: bool,
    shift: bool,
    control: bool,
    alt: bool,
) -> Result<serde_json::Value, String> {
    let (entity, handle) =
        get_notes_app_entity_and_handle().ok_or_else(|| "Notes window is not open".to_string())?;
    update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let key = key.to_ascii_lowercase();
            let mut action = "unsupportedNotesEditorKey";
            let mut handled = false;

            if !platform
                && !shift
                && !control
                && !alt
                && crate::ui_foundation::is_key_escape(&key)
                && crate::confirm::is_confirm_window_open()
            {
                action = "cancelNotesParentConfirm";
                handled = crate::confirm::route_key_to_confirm_popup("escape", cx);
            } else if let Some(descriptor) = crate::notes::notes_action_for_keystroke(
                app.notes_action_context(),
                &key,
                &gpui::Modifiers {
                    platform,
                    shift,
                    control,
                    alt,
                    ..Default::default()
                },
            ) {
                action = "executeNotesActionDescriptor";
                app.handle_action(descriptor.action, window, cx);
                handled = true;
            } else if !platform && !control && !alt {
                if crate::ui_foundation::is_key_up(&key) {
                    action = "moveNotesSpineSelectionUp";
                    handled = app.move_notes_spine_selection(-1, cx);
                } else if crate::ui_foundation::is_key_down(&key) {
                    action = "moveNotesSpineSelectionDown";
                    handled = app.move_notes_spine_selection(1, cx);
                } else if crate::ui_foundation::is_key_enter(&key) {
                    action = "acceptNotesSpineSelection";
                    handled = app.accept_notes_spine_selection(window, cx);
                } else if crate::ui_foundation::is_key_tab(&key) && !shift {
                    action = "acceptNotesSpineSelectionFromTab";
                    handled = app.accept_notes_spine_selection(window, cx);
                }
            } else if platform && !shift && !control && !alt && key == "." {
                action = "activateNotesDeeplinkOrFocusMode";
                if !app.activate_deeplink_under_cursor(window, cx) {
                    app.toggle_focus_mode(cx);
                }
                handled = true;
            }

            serde_json::json!({
                "handled": handled,
                "target": "notes",
                "action": action,
            })
        })
    })
    .map_err(|error| format!("Failed to handle Notes editor key: {error}"))
}

/// Toggle a Notes popup from the automation/simulateKey path, mirroring the
/// live Cmd+K ("actions" command bar) and Cmd+P ("noteSwitcher") keyboard arms
/// so target-scoped `simulateKey` can drive the same popups the user sees.
pub fn toggle_notes_popup_for_automation(
    cx: &mut App,
    popup: &str,
) -> Result<serde_json::Value, String> {
    let (entity, handle) =
        get_notes_app_entity_and_handle().ok_or_else(|| "Notes window is not open".to_string())?;
    // Must not lease Root: popup open/close reaches `request_focus_surface`,
    // which reads Root via `window.has_active_dialog`.
    update_notes_window_detached(handle, cx, |window, cx| {
        entity.update(cx, |app, cx| {
            let action = match popup {
                "actions" => {
                    if app.command_bar.is_open() {
                        app.close_actions_panel(window, cx);
                        "closeActionsPanel"
                    } else {
                        app.open_actions_panel(window, cx);
                        "openActionsPanel"
                    }
                }
                "noteSwitcher" => {
                    app.close_actions_panel(window, cx);
                    if app.note_switcher.is_open() {
                        app.close_browse_panel(window, cx);
                        "closeNoteSwitcher"
                    } else {
                        app.open_browse_panel(window, cx);
                        "openNoteSwitcher"
                    }
                }
                other => {
                    return serde_json::json!({
                        "handled": false,
                        "target": "notes",
                        "popup": other,
                    });
                }
            };
            serde_json::json!({
                "handled": true,
                "target": "notes",
                "popup": popup,
                "action": action,
            })
        })
    })
    .map_err(|error| format!("Failed to toggle Notes popup: {error}"))
}

/// Backward-compatible helper for older target-scoped DevTools `simulateKey Tab`
/// proof. New callers should use `handle_notes_ghost_key_for_automation`.
pub fn accept_notes_ghost_for_automation(cx: &mut App) -> Result<serde_json::Value, String> {
    handle_notes_ghost_key_for_automation(cx, "tab")
}

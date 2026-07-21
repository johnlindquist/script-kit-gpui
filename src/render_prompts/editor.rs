mod __render_prompts_editor_docs {
    //! Editor prompt rendering helpers and `ScriptListApp::render_editor_prompt`.
    //! Key routines include footer/status builders and shortcut filtering for editor-reserved bindings.
    //! This file depends on `editor`, `ui_foundation`, and actions-dialog utilities from the main app.
}

// Editor prompt render method - extracted from render_prompts.rs
// This file is included via include!() macro in main.rs

const EDITOR_PROMPT_KEY_CONTEXT: &str = "editor_prompt";

#[inline]
fn is_editor_escape_key_variant(key: &str) -> bool {
    ui_foundation::is_key_escape(key)
}

#[inline]
fn editor_reserved_shortcut_reason(key: &str, modifiers: &gpui::Modifiers) -> Option<&'static str> {
    // Keep native editor bindings in control of text editing, navigation, and submit.
    if !modifiers.platform || modifiers.control || modifiers.alt {
        return None;
    }

    if ui_foundation::is_key_enter(key) {
        return Some("submit");
    }

    if ui_foundation::is_key_left(key)
        || ui_foundation::is_key_right(key)
        || ui_foundation::is_key_up(key)
        || ui_foundation::is_key_down(key)
    {
        return Some("cursor_navigation");
    }

    match key {
        "s" => Some("save_submit"),
        "z" | "y" => Some("undo_redo"),
        "f" | "g" => Some("find"),
        "a" | "c" | "v" | "x" => Some("clipboard_selection"),
        _ => None,
    }
}

impl ScriptListApp {
    fn render_editor_prompt(
        &mut self,
        entity: Entity<EditorPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let actions_dialog_top = render_context.actions_dialog_top;
        let actions_dialog_right = render_context.actions_dialog_right;
        let has_actions = self.has_nonempty_sdk_actions();

        tracing::info!(
            surface = "render_prompts::editor",
            actions_open = self.show_actions_popup,
            "prompt_surface_rendered"
        );

        crate::components::emit_prompt_chrome_audit(&crate::components::PromptChromeAudit::editor(
            "render_prompts::editor",
            has_actions,
        ));

        // Sync suppress_keys with actions popup state so editor ignores keys when popup is open
        let show_actions = self.show_actions_popup;
        entity.update(cx, |editor, _| {
            editor.suppress_keys = show_actions;
        });

        // VIBRANCY: Use foundation helper - returns None when vibrancy enabled (let Root handle bg)
        let vibrancy_bg = get_vibrancy_background(theme);

        // Use explicit height from layout constants instead of h_full()
        // h_full() doesn't work at the root level because there's no parent to fill
        let content_height = window_resize::layout::MAX_HEIGHT;

        // Key handler for Cmd+K actions toggle (at parent level to intercept before editor)
        let has_actions_for_handler = has_actions;
        let entity_for_escape = entity.clone();
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                if handle_prompt_key_preamble(
                    this,
                    event,
                    window,
                    cx,
                    PromptKeyPreambleCfg {
                        stop_propagation_on_global_shortcut: false,
                        stop_propagation_when_handled: false,
                        host: ActionsDialogHost::EditorPrompt,
                    },
                    |this, event, window, cx| {
                        let key_str = event.keystroke.key.to_lowercase();
                        let has_cmd = event.keystroke.modifiers.platform;

                        // For ScratchPadView (built-in utility): ESC returns to main menu or closes window
                        if matches!(this.current_view, AppView::ScratchPadView { .. }) {
                            if is_editor_escape_key_variant(&key_str) && !this.show_actions_popup {
                                logging::log("KEY", "ESC in ScratchPadView");
                                this.go_back_or_close(window, cx);
                                return true;
                            }

                            if has_cmd && key_str == "w" {
                                logging::log("KEY", "Cmd+W - closing window");
                                this.close_and_reset_window(cx);
                                return true;
                            }
                        }

                        // SDK editor prompt: Escape cancels with a two-press
                        // guard so a stray Escape cannot discard unsaved
                        // content. First press arms + HUD; second press within
                        // the window cancels (script receives None).
                        if matches!(this.current_view, AppView::EditorPrompt { .. })
                            && is_editor_escape_key_variant(&key_str)
                            && !this.show_actions_popup
                        {
                            const EDITOR_ESCAPE_GUARD_MS: u128 = 2000;
                            let armed_recently = this
                                .editor_escape_armed_at
                                .map(|at| at.elapsed().as_millis() <= EDITOR_ESCAPE_GUARD_MS)
                                .unwrap_or(false);
                            if armed_recently {
                                this.editor_escape_armed_at = None;
                                logging::log(
                                    "KEY",
                                    "ESC ESC in EditorPrompt - canceling prompt (script receives None)",
                                );
                                entity_for_escape.update(cx, |editor, _| editor.submit_cancel());
                            } else {
                                this.editor_escape_armed_at = Some(std::time::Instant::now());
                                this.show_hud(
                                    "Press Esc again to discard and cancel".to_string(),
                                    None,
                                    cx,
                                );
                            }
                            return true;
                        }

                        false
                    },
                    |key, _key_char, modifiers| {
                        modifiers.platform && ui_foundation::is_key_k(key) && has_actions_for_handler
                    },
                    |this, window, cx| {
                        let correlation_id = logging::current_correlation_id();
                        logging::log(
                            "KEY",
                            &format!(
                                "{EDITOR_PROMPT_KEY_CONTEXT}: Cmd+K toggles actions (correlation_id={correlation_id})"
                            ),
                        );
                        this.toggle_arg_actions(cx, window);
                    },
                    |this, action_id, cx| {
                        this.trigger_action_by_name(action_id, cx);
                    },
                    |key, _key_char, modifiers| {
                        let key_lower = key.to_lowercase();
                        let shortcut_key = shortcuts::keystroke_to_shortcut(&key_lower, modifiers);
                        if let Some(reason) = editor_reserved_shortcut_reason(&key_lower, modifiers)
                        {
                            let correlation_id = logging::current_correlation_id();
                            logging::log_debug(
                                "KEY",
                                &format!(
                                    "{EDITOR_PROMPT_KEY_CONTEXT}: reserved shortcut preserved (reason={reason}, shortcut={shortcut_key}, correlation_id={correlation_id})"
                                ),
                            );
                            return false;
                        }

                        true
                    },
                    |this, matched_shortcut, cx| {
                        let correlation_id = logging::current_correlation_id();
                        logging::log(
                            "KEY",
                            &format!(
                                "{EDITOR_PROMPT_KEY_CONTEXT}: SDK action shortcut matched (action={}, shortcut={}, correlation_id={correlation_id})",
                                matched_shortcut.action_name, matched_shortcut.shortcut_key
                            ),
                        );
                        this.trigger_action_by_name(&matched_shortcut.action_name, cx);
                    },
                ) {}
                // Let other keys fall through to the editor
            },
        );

        // Truthful editor footer: Enter inserts a newline here (submit is
        // ⌘↵/⌘S) and ⌘↵ is editor-reserved for submit, so the universal
        // "↵ Run · ⌘K Actions · ⌘↵ Agent" set would lie twice on this surface.
        let editor_hints = crate::components::editor_prompt_hints();
        crate::components::emit_surface_prompt_hint_audit(
            "render_prompts::editor",
            &editor_hints,
            "editor_submit_cancel_footer",
        );
        let entity_for_submit = entity.clone();
        let on_submit = move |_: &gpui::ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            entity_for_submit.update(cx, |editor, cx| editor.submit(cx));
        };
        let on_actions = cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
            this.dispatch_main_window_footer_action(
                crate::footer_popup::FooterAction::Actions,
                window,
                cx,
                "gpui_footer",
            );
        });
        let entity_for_cancel = entity.clone();
        let on_cancel = move |_: &gpui::ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            entity_for_cancel.update(cx, |editor, _| editor.submit_cancel());
        };
        let mut editor_hint_handlers = [
            Some(Box::new(on_submit) as crate::components::footer_chrome::FooterHintClickHandler),
            Some(Box::new(on_actions) as _),
            Some(Box::new(on_cancel) as _),
        ]
        .into_iter();
        let editor_footer =
            crate::components::footer_chrome::render_clickable_footer_hint_action_rail(
                "editor-prompt-footer-rail",
                editor_hints
                    .into_iter()
                    .map(|hint| (hint, editor_hint_handlers.next().flatten())),
            );

        // NOTE: The EditorPrompt entity has its own track_focus and on_key_down in its render method.
        // We do NOT add track_focus here to avoid duplicate focus tracking on the same handle.
        //
        // Container with flex layout:
        // - Editor wrapper using flex_1 to fill remaining space above footer
        // - Footer slot is resolved through main_window_footer_slot(); native/GPUI height
        //   stays owned by the shared main-window footer contract.
        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .when_some(vibrancy_bg, |d, bg| d.bg(bg)) // VIBRANCY: Only apply bg when vibrancy disabled
            // NOTE: No shadow - shadows on transparent elements cause gray fill with vibrancy
            .w_full()
            .h(content_height) // Explicit 700px height (window height for editor view)
            .overflow_hidden() // Clip content to rounded corners
            .rounded(px(0.0))
            .key_context(EDITOR_PROMPT_KEY_CONTEXT)
            .on_key_down(handle_key)
            // Editor entity fills remaining space (flex_1)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.)) // Required for flex children to shrink properly
                    .w_full()
                    .overflow_hidden()
                    .child(entity),
            )
            // Editor-truthful hint strip footer (native or GPUI)
            .when_some(self.main_window_footer_slot(editor_footer), |d, footer| {
                d.child(footer)
            })
            // Actions dialog overlay
            .when_some(
                render_actions_backdrop(
                    self.show_actions_popup,
                    self.actions_dialog.clone(),
                    actions_dialog_top,
                    actions_dialog_right,
                    ActionsBackdropConfig {
                        backdrop_id: "editor-actions-backdrop",
                        close_host: ActionsDialogHost::EditorPrompt,
                        backdrop_log_message: "Editor actions backdrop clicked - dismissing dialog",
                        show_pointer_cursor: true,
                    },
                    cx,
                ),
                |d, backdrop_overlay| d.child(backdrop_overlay),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod editor_prompt_tests {
    use super::*;
    use std::collections::HashMap;

    fn cmd_modifiers() -> gpui::Modifiers {
        gpui::Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_check_sdk_action_shortcut_matches_registered_shortcut() {
        let mut action_shortcuts = HashMap::new();
        let modifiers = cmd_modifiers();
        let shortcut_key = shortcuts::keystroke_to_shortcut("k", &modifiers);
        action_shortcuts.insert(shortcut_key.clone(), "open-actions".to_string());

        let shortcut_match = check_sdk_action_shortcut(&action_shortcuts, "K", &modifiers);
        assert_eq!(
            shortcut_match,
            Some(SdkActionShortcutMatch {
                action_name: "open-actions".to_string(),
                shortcut_key,
            })
        );
    }

    #[test]
    fn test_check_sdk_action_shortcut_returns_none_when_modifiers_do_not_match() {
        let mut action_shortcuts = HashMap::new();
        let cmd_modifiers = cmd_modifiers();
        action_shortcuts.insert(
            shortcuts::keystroke_to_shortcut("k", &cmd_modifiers),
            "open-actions".to_string(),
        );

        let no_modifiers = gpui::Modifiers::default();
        assert_eq!(
            check_sdk_action_shortcut(&action_shortcuts, "k", &no_modifiers),
            None
        );
    }

    /// The editor footer must stay truthful: plain Enter inserts a newline
    /// (submit is ⌘↵/⌘S), so the universal "↵ Run" strip lied on the surface
    /// where trusting it is most destructive.
    #[test]
    fn test_editor_uses_truthful_hint_strip_footer() {
        const EDITOR_RENDER_SOURCE: &str = include_str!("editor.rs");
        assert!(
            EDITOR_RENDER_SOURCE.contains("editor_prompt_hints()"),
            "editor prompt should use the editor-truthful hint set (⌘↵ Submit · ⌘K Actions · Esc Cancel)"
        );
        // Split literal so this test's own source can't satisfy the match.
        let universal_rail_call = "clickable_universal_".to_owned() + "footer_action_rail(";
        assert!(
            !EDITOR_RENDER_SOURCE.contains(&universal_rail_call),
            "editor prompt must not advertise the universal footer-button rail's '↵ Run' action — Enter is a newline here"
        );
    }

    #[test]
    fn test_editor_action_shortcuts_do_not_override_reserved_editing_bindings() {
        let cmd = cmd_modifiers();

        for key in ["f", "z", "return", "s", "arrowleft", "arrowup", "down", "c"] {
            assert!(
                editor_reserved_shortcut_reason(key, &cmd).is_some(),
                "{key} should stay editor-owned",
            );
        }

        assert!(
            editor_reserved_shortcut_reason("k", &cmd).is_none(),
            "Cmd+K remains available for actions",
        );

        let cmd_and_ctrl = gpui::Modifiers {
            platform: true,
            control: true,
            ..Default::default()
        };
        assert!(
            editor_reserved_shortcut_reason("f", &cmd_and_ctrl).is_none(),
            "Cmd+Ctrl+F is not treated as an editor-reserved shortcut",
        );
    }

    #[test]
    fn test_editor_chrome_audit_uses_editor_layout_mode() {
        const EDITOR_RENDER_SOURCE: &str = include_str!("editor.rs");
        assert!(
            EDITOR_RENDER_SOURCE.contains("PromptChromeAudit::editor("),
            "editor prompt should emit an editor-type chrome audit"
        );
        assert!(
            EDITOR_RENDER_SOURCE.contains("\"render_prompts::editor\""),
            "editor chrome audit should use correct surface name"
        );
    }

    #[test]
    fn test_editor_actions_backdrop_uses_cursor_pointer_when_clickable() {
        const EDITOR_RENDER_SOURCE: &str = include_str!("editor.rs");

        assert!(
            EDITOR_RENDER_SOURCE.contains("render_actions_backdrop("),
            "editor render should delegate backdrop overlay creation to shared helper"
        );
        assert!(
            EDITOR_RENDER_SOURCE.contains("\"editor-actions-backdrop\""),
            "editor render should pass its backdrop id to shared helper"
        );
        assert!(
            EDITOR_RENDER_SOURCE.contains("ActionsDialogHost::EditorPrompt"),
            "editor render should preserve actions host routing when helper is used"
        );
        assert!(
            EDITOR_RENDER_SOURCE.contains("show_pointer_cursor: true"),
            "editor render should keep backdrop cursor pointer enabled"
        );
    }

    #[test]
    fn test_editor_key_handling_uses_preamble_helper_with_reserved_shortcut_filter() {
        const EDITOR_RENDER_SOURCE: &str = include_str!("editor.rs");

        assert!(
            EDITOR_RENDER_SOURCE.contains("handle_prompt_key_preamble("),
            "editor key handling should delegate shared preamble behavior to helper"
        );
        // Escape is entity-guarded, not shell-dismissable: the dismiss policy
        // table (PromptChildContent → LetViewHandle) keeps the shell out, and
        // the two-press guard owns cancellation.
        assert!(
            EDITOR_RENDER_SOURCE.contains("editor_escape_armed_at"),
            "editor escape must keep the two-press cancel guard"
        );
        assert!(
            EDITOR_RENDER_SOURCE.contains("editor.submit_cancel()"),
            "editor escape guard must cancel to the script (None), not close the window"
        );
        assert!(
            EDITOR_RENDER_SOURCE.contains("editor_reserved_shortcut_reason(&key_lower, modifiers)"),
            "editor key handling should preserve reserved shortcut filtering before SDK shortcut matching"
        );
    }

    #[test]
    fn test_is_editor_escape_key_variant_accepts_short_and_long_forms() {
        assert!(is_editor_escape_key_variant("escape"));
        assert!(is_editor_escape_key_variant("Escape"));
        assert!(is_editor_escape_key_variant("esc"));
        assert!(is_editor_escape_key_variant("Esc"));
        assert!(!is_editor_escape_key_variant("enter"));
    }
}

mod __render_prompts_term_docs {
    //! Terminal prompt rendering for script terminals and quick terminal view.
    //! Key helpers map action IDs to `TerminalAction` values and drive `render_term_prompt` behavior.
    //! This file depends on `terminal`, shared actions-dialog routing, and global shortcut handling in `ScriptListApp`.
}

// Term prompt render method - extracted from render_prompts.rs
// This file is included via include!() macro in main.rs

const TERM_PROMPT_KEY_CONTEXT: &str = "term_prompt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TermPromptActionsMode {
    SdkActions,
    TerminalCommands,
}

#[inline]
fn term_prompt_actions_mode(has_sdk_actions: bool) -> TermPromptActionsMode {
    if has_sdk_actions {
        TermPromptActionsMode::SdkActions
    } else {
        TermPromptActionsMode::TerminalCommands
    }
}

#[inline]
fn should_block_escape_for_non_dismissable_term(
    is_quick_terminal: bool,
    show_actions_popup: bool,
    key: &str,
) -> bool {
    !is_quick_terminal && !show_actions_popup && ui_foundation::is_key_escape(key)
}

#[inline]
fn terminal_action_from_id(action_id: &str) -> Option<crate::terminal::TerminalAction> {
    use crate::terminal::TerminalAction;

    match action_id {
        "clear" => Some(TerminalAction::Clear),
        "copy" => Some(TerminalAction::Copy),
        "copy_all" => Some(TerminalAction::CopyAll),
        "copy_last_command" => Some(TerminalAction::CopyLastCommand),
        "copy_last_output" => Some(TerminalAction::CopyLastOutput),
        "paste" => Some(TerminalAction::Paste),
        "select_all" => Some(TerminalAction::SelectAll),
        "scroll_to_top" => Some(TerminalAction::ScrollToTop),
        "scroll_to_bottom" => Some(TerminalAction::ScrollToBottom),
        "scroll_page_up" => Some(TerminalAction::ScrollPageUp),
        "scroll_page_down" => Some(TerminalAction::ScrollPageDown),
        "find" => Some(TerminalAction::Find),
        "interrupt" => Some(TerminalAction::Interrupt),
        "kill" => Some(TerminalAction::Kill),
        "suspend" => Some(TerminalAction::Suspend),
        "quit" => Some(TerminalAction::Quit),
        "send_eof" => Some(TerminalAction::SendEOF),
        "reset" => Some(TerminalAction::Reset),
        "new_shell" => Some(TerminalAction::NewShell),
        "restart" => Some(TerminalAction::Restart),
        "zoom_in" => Some(TerminalAction::ZoomIn),
        "zoom_out" => Some(TerminalAction::ZoomOut),
        "reset_zoom" => Some(TerminalAction::ResetZoom),
        _ => None,
    }
}

#[inline]
fn is_term_prompt_clear_shortcut(has_cmd: bool, has_shift: bool, key: &str) -> bool {
    has_cmd && !has_shift && ui_foundation::is_key_k(key)
}

#[inline]
fn is_term_prompt_actions_toggle_shortcut(has_cmd: bool, has_shift: bool, key: &str) -> bool {
    has_cmd && has_shift && ui_foundation::is_key_k(key)
}

#[inline]
fn terminal_return_hint_label(return_view: Option<&AppView>) -> &'static str {
    match return_view {
        Some(AppView::ScriptList) => "⌘W Main Menu",
        Some(AppView::AcpChatView { .. }) => "⌘W ACP",
        Some(_) => "⌘W Return",
        None => "⌘W Close",
    }
}

#[inline]
fn terminal_escape_hint_label(return_view: Option<&AppView>) -> Option<&'static str> {
    match return_view {
        Some(_) => Some("Esc stays in tool"),
        None => None,
    }
}

#[inline]
fn render_terminal_prompt_hint_strip(
    route: Option<&crate::ai::TabAiApplyBackRoute>,
    return_view: Option<&AppView>,
) -> AnyElement {
    let show_script_verification_hint = return_view.is_some();
    let can_apply_back = route.is_some() && return_view.is_some();
    let return_label = terminal_return_hint_label(return_view);

    tracing::debug!(
        target: "script_kit::keyboard",
        event = "quick_terminal_hint_strip_rendered",
        return_label,
        can_apply_back,
        show_script_verification_hint,
    );

    let mut items: Vec<String> = Vec::new();

    if show_script_verification_hint {
        items.push("Bun verify required".to_string());
    }

    if can_apply_back {
        items.push("⌘↩ Apply".to_string());
    }

    if let Some(escape_label) = terminal_escape_hint_label(return_view) {
        items.push(escape_label.to_string());
    }

    items.push(return_label.to_string());

    let leading = if show_script_verification_hint {
        let theme = crate::theme::get_cached_theme();
        Some(
            crate::components::prompt_layout_shell::render_hint_strip_leading_text(
                "This session must bun build, run with SK_VERIFY=1, and fix failures before reporting success.",
                theme.colors.text.primary,
            ),
        )
    } else {
        None
    };

    crate::components::prompt_layout_shell::render_simple_hint_strip(items.join(" · "), leading)
}

impl ScriptListApp {
    #[inline]
    fn toggle_term_prompt_actions(
        &mut self,
        actions_mode: TermPromptActionsMode,
        cx: &mut Context<Self>,
        window: &mut Window,
    ) {
        match actions_mode {
            TermPromptActionsMode::SdkActions => self.toggle_arg_actions(cx, window),
            TermPromptActionsMode::TerminalCommands => self.toggle_terminal_commands(cx, window),
        }
    }

    #[inline]
    fn execute_term_prompt_action_by_id(
        &mut self,
        action_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(action) = terminal_action_from_id(action_id) else {
            return false;
        };

        let terminal_entity = match &self.current_view {
            AppView::TermPrompt { entity, .. } => entity.clone(),
            AppView::QuickTerminalView { entity } => entity.clone(),
            _ => return false,
        };

        terminal_entity.update(cx, |term_prompt, cx| {
            term_prompt.execute_action(action, cx);
        });

        true
    }

    fn render_term_prompt(
        &mut self,
        entity: Entity<term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_actions = self.has_nonempty_sdk_actions();

        let mut chrome_audit =
            crate::components::PromptChromeAudit::editor("render_prompts::term", has_actions);
        chrome_audit.footer_mode = "custom_hint_strip";
        chrome_audit.hint_count = 0;
        chrome_audit.exception_reason = Some("terminal_owns_contextual_footer");
        crate::components::emit_prompt_chrome_audit(&chrome_audit);

        let render_context = PromptRenderContext::new(self.theme.as_ref(), self.current_design);
        let theme = render_context.theme;
        let actions_dialog_right = render_context.actions_dialog_right;
        let actions_dialog_bottom = window_resize::layout::FOOTER_HEIGHT
            + render_context.design_spacing.padding_sm
            - render_context.design_visual.border_thin;
        let actions_mode = term_prompt_actions_mode(has_actions);

        // Sync suppress_keys with actions popup state so terminal ignores keys when popup is open
        let show_actions = self.show_actions_popup;
        let is_quick_terminal = matches!(self.current_view, AppView::QuickTerminalView { .. });
        entity.update(cx, |term, _| {
            term.suppress_keys = show_actions;
            term.escape_cancels = !is_quick_terminal;
            term.edge_to_edge = is_quick_terminal;
        });

        // VIBRANCY: Use foundation helper - returns None when vibrancy enabled (let Root handle bg)
        let vibrancy_bg = get_vibrancy_background(theme);

        // Use explicit height from layout constants instead of h_full()
        // h_full() doesn't work at the root level because there's no parent to fill
        let content_height = window_resize::layout::MAX_HEIGHT;

        // Key handler for Cmd+K actions toggle
        let actions_mode_for_handler = actions_mode;
        let has_actions_for_toggle = true;
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
                        is_dismissable: false,
                        stop_propagation_on_global_shortcut: false,
                        stop_propagation_when_handled: false,
                        host: ActionsDialogHost::TermPrompt,
                    },
                    |this, event, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let has_cmd = event.keystroke.modifiers.platform;
                        let has_shift = event.keystroke.modifiers.shift;
                        let is_quick_terminal =
                            matches!(this.current_view, AppView::QuickTerminalView { .. });

                        if should_block_escape_for_non_dismissable_term(
                            is_quick_terminal,
                            this.show_actions_popup,
                            key,
                        ) {
                            let correlation_id = logging::current_correlation_id();
                            logging::log_debug(
                                "KEY",
                                &format!(
                                    "{TERM_PROMPT_KEY_CONTEXT}: swallow non-dismissable escape (correlation_id={correlation_id})"
                                ),
                            );
                            return true;
                        }

                        // QuickTerminalView wrapper semantics contract:
                        // - Plain Escape is forwarded to the PTY (harness TUI owns it).
                        // - Cmd+Enter pastes back the terminal selection or last output
                        //   to the captured source context.
                        // - Cmd+W closes the wrapper and restores the previous view/focus.
                        // This is intentional: CLI harnesses (Claude Code, Codex, etc.)
                        // use Escape for their own TUI navigation (cancel, dismiss, etc.).
                        //
                        // Verification-bearing script-authoring launches use
                        // QuickTerminalView intentionally so the Bun gate happens
                        // inside the live harness terminal session.
                        if is_quick_terminal
                            && has_cmd
                            && !has_shift
                            && crate::ui_foundation::is_key_enter(key)
                        {
                            // Use the de-raced helper that primes clipboard
                            // then waits a tick before applying, so the
                            // clipboard write completes before the read.
                            if let AppView::QuickTerminalView { entity } = &this.current_view {
                                this.apply_tab_ai_result_from_terminal(entity.clone(), cx);
                            }
                            return true;
                        }

                        if is_quick_terminal
                            && has_cmd
                            && key.eq_ignore_ascii_case("w")
                        {
                            this.close_tab_ai_harness_terminal_with_window(window, cx);
                            return true;
                        }

                        // Cmd+K clears terminal output.
                        if is_term_prompt_clear_shortcut(has_cmd, has_shift, key) {
                            let correlation_id = logging::current_correlation_id();
                            logging::log(
                                "KEY",
                                &format!(
                                    "{TERM_PROMPT_KEY_CONTEXT}: Cmd+K clears terminal (correlation_id={correlation_id})"
                                ),
                            );
                            this.execute_term_prompt_action_by_id(
                                crate::actions_toggle::TERM_PROMPT_CLEAR_ACTION_ID,
                                cx,
                            );
                            if this.show_actions_popup {
                                this.close_actions_popup(ActionsDialogHost::TermPrompt, window, cx);
                            }
                            return true;
                        }

                        false
                    },
                    |key, _key_char, modifiers| {
                        is_term_prompt_actions_toggle_shortcut(
                            modifiers.platform,
                            modifiers.shift,
                            key,
                        ) && has_actions_for_toggle
                    },
                    |this, window, cx| {
                        let correlation_id = logging::current_correlation_id();
                        logging::log(
                            "KEY",
                            &format!(
                                "{TERM_PROMPT_KEY_CONTEXT}: Cmd+Shift+K toggles actions (mode={actions_mode_for_handler:?}, correlation_id={correlation_id})"
                            ),
                        );
                        this.toggle_term_prompt_actions(actions_mode_for_handler, cx, window);
                    },
                    |this, action_id, cx| {
                        if action_id == crate::actions_toggle::TERM_PROMPT_ACTIONS_TOGGLE_ACTION_ID
                        {
                            return;
                        }

                        if this.trigger_action_by_name(action_id, cx) {
                            return;
                        }

                        if this.execute_term_prompt_action_by_id(action_id, cx) {
                            return;
                        }

                        let correlation_id = logging::current_correlation_id();
                        logging::log_debug(
                            "KEY",
                            &format!(
                                "{TERM_PROMPT_KEY_CONTEXT}: unhandled actions dialog selection (action_id={action_id}, correlation_id={correlation_id})"
                            ),
                        );
                    },
                    |_key, _key_char, _modifiers| true,
                    |this, matched_shortcut, cx| {
                        let correlation_id = logging::current_correlation_id();
                        logging::log(
                            "KEY",
                            &format!(
                                "{TERM_PROMPT_KEY_CONTEXT}: SDK action shortcut matched (action={}, shortcut={}, correlation_id={correlation_id})",
                                matched_shortcut.action_name, matched_shortcut.shortcut_key
                            ),
                        );
                        this.trigger_action_by_name(&matched_shortcut.action_name, cx);
                    },
                ) {}
                // Let other keys fall through to the terminal
            },
        );

        // Terminal actions open as a separate vibrancy popup window, so the
        // inline backdrop is not needed (the dialog renders in its own window).
        let show_inline_actions_backdrop = false;

        // Container with explicit height. We wrap the entity in a sized div because
        // GPUI entities don't automatically inherit parent flex sizing.
        // NOTE: No rounded corners for terminal - it should fill edge-to-edge
        div()
            .relative() // Needed for absolute positioned actions dialog overlay
            .flex()
            .flex_col()
            .when_some(vibrancy_bg, |d, bg| d.bg(bg)) // VIBRANCY: Only apply bg when vibrancy disabled
            // NOTE: No shadow - shadows on transparent elements cause gray fill with vibrancy
            .w_full()
            .h(content_height)
            .key_context(TERM_PROMPT_KEY_CONTEXT)
            .capture_key_down(handle_key)
            // Terminal content takes remaining space.
            // Keep overflow clipping scoped to terminal output so the actions popup can
            // preserve vibrancy/translucency compositing when rendered as an overlay.
            // For QuickTerminal (harness), forward wrapper wheel events into the
            // TermPrompt so scroll works even when GPUI targets the wrapper div.
            // Regular script terminals handle scroll natively inside TermPrompt.
            .child({
                let terminal_entity_for_scroll = entity.clone();
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .when(is_quick_terminal, |d| {
                        d.on_scroll_wheel(cx.listener(
                            move |_this: &mut Self,
                                  event: &gpui::ScrollWheelEvent,
                                  _window: &mut Window,
                                  cx: &mut Context<Self>| {
                                let entity = terminal_entity_for_scroll.clone();
                                entity.update(cx, |term_prompt, cx| {
                                    term_prompt.handle_external_scroll_wheel(event, cx);
                                });
                                cx.stop_propagation();
                            },
                        ))
                    })
                    .child(entity)
            })
            // Terminal-specific footer: advertise the route-aware paste-back
            // action plus close while the PTY keeps full control of plain
            // Escape and unhandled keys.
            // When the native main-window footer is active, suppress the GPUI hint strip.
            .when_some(
                self.main_window_footer_slot(render_terminal_prompt_hint_strip(
                    if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
                        self.tab_ai_harness_apply_back_route.as_ref()
                    } else {
                        None
                    },
                    if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
                        self.tab_ai_harness_return_view.as_ref()
                    } else {
                        None
                    },
                )),
                |d, footer| d.child(footer),
            )
            // Actions dialog overlay
            .when_some(
                render_actions_backdrop_bottom_anchored(
                    show_inline_actions_backdrop,
                    self.actions_dialog.clone(),
                    actions_dialog_bottom,
                    actions_dialog_right,
                    ActionsBackdropConfig {
                        backdrop_id: "term-actions-backdrop",
                        close_host: ActionsDialogHost::TermPrompt,
                        backdrop_log_message: "Term actions backdrop clicked - dismissing dialog",
                        show_pointer_cursor: false,
                    },
                    cx,
                ),
                |d, backdrop_overlay| d.child(backdrop_overlay),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod term_prompt_render_tests {
    use super::*;

    #[test]
    fn test_term_prompt_actions_mode_uses_sdk_actions_when_present() {
        assert_eq!(
            term_prompt_actions_mode(true),
            TermPromptActionsMode::SdkActions
        );
    }

    #[test]
    fn test_term_prompt_actions_mode_defaults_to_terminal_commands_without_sdk_actions() {
        assert_eq!(
            term_prompt_actions_mode(false),
            TermPromptActionsMode::TerminalCommands,
        );
    }

    #[test]
    fn test_term_prompt_actions_backdrop_uses_shared_helper() {
        const TERM_RENDER_SOURCE: &str = include_str!("term.rs");

        assert!(
            TERM_RENDER_SOURCE.contains("render_actions_backdrop_bottom_anchored("),
            "term render should delegate backdrop overlay creation to shared helper"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("\"term-actions-backdrop\""),
            "term render should pass its backdrop id to shared helper"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("ActionsDialogHost::TermPrompt"),
            "term render should preserve actions host routing when helper is used"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("show_pointer_cursor: false"),
            "term render should keep backdrop cursor pointer disabled"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("let actions_dialog_bottom ="),
            "term render should derive a footer-relative actions popup offset"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("window_resize::layout::FOOTER_HEIGHT"),
            "term actions popup should stay anchored from the shared footer height constant"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("let show_inline_actions_backdrop = false;"),
            "term render should disable inline backdrop since terminal uses a vibrancy popup window"
        );
    }

    #[test]
    fn test_term_actions_overlay_keeps_root_unclipped_for_vibrancy_compositing() {
        const TERM_RENDER_SOURCE: &str = include_str!("term.rs");

        assert!(
            !TERM_RENDER_SOURCE.contains(
                ".h(content_height)\n            .overflow_hidden()\n            .key_context(TERM_PROMPT_KEY_CONTEXT)"
            ),
            "term root container should not clip overflow before key handling, so overlay vibrancy can composite correctly"
        );
        assert!(
            TERM_RENDER_SOURCE.contains(".overflow_hidden()")
                && TERM_RENDER_SOURCE.contains("on_scroll_wheel"),
            "term render should keep overflow clipping scoped to the terminal content child and forward wheel events"
        );
    }

    #[test]
    fn test_term_key_handler_uses_shared_preamble_helper() {
        const TERM_RENDER_SOURCE: &str = include_str!("term.rs");

        assert!(
            TERM_RENDER_SOURCE.contains("handle_prompt_key_preamble("),
            "term key handling should delegate preamble logic to shared helper"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("PromptKeyPreambleCfg"),
            "term key handling should configure the shared helper via PromptKeyPreambleCfg"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("let has_actions_for_toggle = true;"),
            "term key handling should keep action toggling enabled in terminal-commands mode"
        );
    }

    #[test]
    fn test_term_prompt_escape_does_not_close_sdk_terminal_when_non_dismissable() {
        assert!(should_block_escape_for_non_dismissable_term(
            false, false, "escape"
        ));
        assert!(should_block_escape_for_non_dismissable_term(
            false, false, "Esc"
        ));
        assert!(!should_block_escape_for_non_dismissable_term(
            false, false, "enter"
        ));
        assert!(!should_block_escape_for_non_dismissable_term(
            true, false, "escape"
        ));
        assert!(!should_block_escape_for_non_dismissable_term(
            false, true, "escape"
        ));
    }

    #[test]
    fn test_terminal_action_from_id_maps_primary_command_palette_actions() {
        use crate::terminal::TerminalAction;

        assert_eq!(
            terminal_action_from_id("clear"),
            Some(TerminalAction::Clear)
        );
        assert_eq!(
            terminal_action_from_id("copy_all"),
            Some(TerminalAction::CopyAll),
        );
        assert_eq!(
            terminal_action_from_id("scroll_to_top"),
            Some(TerminalAction::ScrollToTop),
        );
        assert_eq!(
            terminal_action_from_id("reset_zoom"),
            Some(TerminalAction::ResetZoom),
        );
        assert_eq!(terminal_action_from_id("unknown"), None);
    }

    #[test]
    fn test_term_prompt_clear_shortcut_matches_cmd_k_without_shift() {
        assert!(is_term_prompt_clear_shortcut(true, false, "k"));
        assert!(is_term_prompt_clear_shortcut(true, false, "K"));
        assert!(!is_term_prompt_clear_shortcut(true, true, "k"));
        assert!(!is_term_prompt_clear_shortcut(false, false, "k"));
    }

    #[test]
    fn test_term_chrome_audit_uses_editor_layout_with_custom_footer() {
        const TERM_RENDER_SOURCE: &str = include_str!("term.rs");
        assert!(
            TERM_RENDER_SOURCE.contains("layout_mode: \"editor\""),
            "term prompt should emit an editor layout mode"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("footer_mode: \"custom_hint_strip\""),
            "term prompt should declare a custom hint strip footer, not the universal one"
        );
        assert!(
            TERM_RENDER_SOURCE
                .contains("exception_reason: Some(\"terminal_owns_contextual_footer\")"),
            "term prompt should document why it has a custom footer"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("render_terminal_prompt_hint_strip("),
            "term prompt should use the terminal-specific hint strip footer"
        );
    }

    #[test]
    fn test_quick_terminal_escape_passes_through_to_pty() {
        const TERM_RENDER_SOURCE: &str = include_str!("term.rs");

        // QuickTerminalView must NOT intercept plain Escape — it should fall through
        // to the terminal PTY so the harness TUI can handle it.
        assert!(
            !TERM_RENDER_SOURCE.contains("is_key_escape(key) && !this.show_actions_popup {\n                                this.close_tab_ai_harness_terminal"),
            "QuickTerminalView must not close on plain Escape — Escape belongs to the PTY"
        );

        // Cmd+W is the designated close shortcut for QuickTerminalView.
        assert!(
            TERM_RENDER_SOURCE.contains("eq_ignore_ascii_case(\"w\")"),
            "QuickTerminalView must use Cmd+W to close the wrapper"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("close_tab_ai_harness_terminal"),
            "Cmd+W must restore the previous view via close_tab_ai_harness_terminal"
        );
        assert!(
            TERM_RENDER_SOURCE.contains("term.edge_to_edge = is_quick_terminal;"),
            "QuickTerminalView must switch the terminal into edge-to-edge rendering"
        );
    }

    #[test]
    fn test_term_prompt_actions_toggle_shortcut_matches_cmd_shift_k() {
        assert!(is_term_prompt_actions_toggle_shortcut(true, true, "k"));
        assert!(is_term_prompt_actions_toggle_shortcut(true, true, "K"));
        assert!(!is_term_prompt_actions_toggle_shortcut(true, false, "k"));
        assert!(!is_term_prompt_actions_toggle_shortcut(false, true, "k"));
    }

    #[test]
    fn test_terminal_return_hint_label_reflects_return_destination() {
        assert_eq!(
            terminal_return_hint_label(Some(&AppView::ScriptList)),
            "⌘W Main Menu"
        );
        assert_eq!(terminal_return_hint_label(None), "⌘W Close");
        // Any non-ScriptList saved view should say "Return"
        assert!(terminal_return_hint_label(Some(&AppView::ScriptList)).contains("Main Menu"));
    }

    #[test]
    fn test_terminal_escape_hint_present_only_with_return_view() {
        assert!(terminal_escape_hint_label(Some(&AppView::ScriptList)).is_some());
        assert!(terminal_escape_hint_label(None).is_none());
    }
}

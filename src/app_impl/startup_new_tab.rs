        // Add global AI-entry and Tab-behavior interception. This fires BEFORE
        // normal key handling so launcher surfaces can claim Cmd+Enter and
        // views with local Tab ownership can keep it.
        let app_entity_for_tab = cx.entity().downgrade();
        let tab_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_tab;
            move |event, window, cx| {
                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // Skip keystrokes from secondary windows — interceptors are
                // GLOBAL and fire for ALL windows.  Secondary windows own
                // their own Tab/Cmd+Enter handling.
                let is_notes = crate::notes::is_notes_window(window);
                let is_detached_agent_chat = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
                let is_actions = crate::actions::is_actions_window(window);
                let is_shortcut_recorder = super::shortcut_recorder::is_shortcut_recorder_window(window);
                if is_notes || is_detached_agent_chat || is_actions || is_shortcut_recorder {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "tab_interceptor_skipped_secondary_window",
                        is_notes,
                        is_detached_agent_chat,
                        is_actions,
                        is_shortcut_recorder,
                    );
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_lower = key.to_ascii_lowercase();
                let is_tab_key = key.eq_ignore_ascii_case("tab");
                let has_shift = event.keystroke.modifiers.shift;
                let has_cmd = event.keystroke.modifiers.platform;
                let has_alt = event.keystroke.modifiers.alt;
                let has_ctrl = event.keystroke.modifiers.control;
                let is_plain_enter = crate::ui_foundation::is_key_enter(key)
                    && !has_cmd
                    && !has_shift
                    && !has_alt
                    && !has_ctrl;
                let is_global_ai_chord = crate::ui_foundation::is_key_enter(key)
                    && has_cmd
                    && !has_shift
                    && !has_alt
                    && !has_ctrl;

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                if let Some(app) = app_entity.upgrade() {
                    let mut handled = false;
                    app.update(cx, |this, cx| {
                        handled = this.try_execute_root_file_action_shortcut(
                            &key_lower,
                            &event.keystroke.modifiers,
                            window,
                            cx,
                        );
                        if handled {
                            cx.stop_propagation();
                        }
                    });
                    if handled {
                        return;
                    }
                }

                if is_global_ai_chord {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if this.try_route_global_cmd_enter_to_agent_chat_context_capture(cx) {
                                cx.stop_propagation();
                            }
                        });
                    }
                    return;
                }

                // Check for Tab key (no cmd/alt/ctrl modifiers, but shift is allowed)
                if is_tab_key
                    && !has_cmd
                    && !has_alt
                    && !has_ctrl
                {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            let owner = match &this.current_view {
                                AppView::ScriptList => "script_list",
                                AppView::AgentChatView { .. } => "embedded_agent_chat",
                                AppView::QuickTerminalView { .. } => "quick_terminal",
                                AppView::FileSearchView { .. } => "file_search",
                                AppView::ChatPrompt { .. } => "chat_prompt",
                                _ => "main_window_other",
                            };
                            tracing::debug!(
                                target: "script_kit::keyboard",
                                event = "tab_interceptor_owner_path",
                                owner,
                                has_shift,
                                show_actions_popup = this.show_actions_popup,
                                save_offer_open = this.tab_ai_save_offer_state.is_some(),
                            );

                            if this.main_window_modal_owns_keyboard() {
                                tracing::debug!(
                                    target: "script_kit::keyboard",
                                    event = "tab_interceptor_suppressed_for_modal",
                                    owner,
                                    has_shift,
                                );
                                cx.stop_propagation();
                                return;
                            }

                            // File search owns Tab locally: plain Tab browses
                            // into the selected directory and Shift+Tab goes up.
                            if matches!(this.current_view, AppView::FileSearchView { .. }) {
                                cx.stop_propagation();
                                if this.show_actions_popup {
                                    return;
                                }

                                if has_shift {
                                    let current_query = match &this.current_view {
                                        AppView::FileSearchView { query, .. } => query.clone(),
                                        _ => String::new(),
                                    };

                                    let next_path = if let Some(parsed) =
                                        crate::file_search::parse_directory_path(&current_query)
                                    {
                                        if parsed.filter.is_some() {
                                            Some(parsed.directory)
                                        } else {
                                            crate::file_search::parent_dir_display(&parsed.directory)
                                        }
                                    } else {
                                        None
                                    };

                                    if let Some(parent_path) = next_path {
                                        crate::logging::log(
                                            "KEY",
                                            &format!(
                                                "Shift+Tab: Navigating up from '{}' to '{}'",
                                                current_query, parent_path
                                            ),
                                        );
                                        this.gpui_input_state.update(cx, |state, cx| {
                                            state.set_value(parent_path.clone(), window, cx);
                                            let len = parent_path.len();
                                            state.set_selection(len, len, window, cx);
                                        });
                                        cx.notify();
                                    } else {
                                        crate::logging::log(
                                            "KEY",
                                            &format!(
                                                "Shift+Tab: Already at root '{}', no-op",
                                                current_query
                                            ),
                                        );
                                    }
                                } else if !this.navigate_file_search_into_selected_directory(cx) {
                                    crate::logging::log(
                                        "KEY",
                                        "Tab: no selected directory to navigate into",
                                    );
                                }
                                return;
                            }

                            // ChatPrompt keeps Shift+Tab for local setup
                            // navigation and leaves plain Tab local.
                            if matches!(this.current_view, AppView::ChatPrompt { .. }) {
                                if has_shift {
                                    if let AppView::ChatPrompt { entity, .. } = &this.current_view {
                                        let handled = entity.update(cx, |chat, cx| {
                                            chat.handle_setup_key("tab", true, cx)
                                        });
                                        if handled {
                                            cx.stop_propagation();
                                            return;
                                        }
                                    }
                                }
                            }

                            // Menu-syntax trigger picker owns Tab when it is visible —
                            // Tab applies the selected row (keep-open for open-value
                            // qualifiers like `source:`, close-after-apply for bare
                            // qualifiers or capture targets). Runs BEFORE the Agent Chat
                            // plain-Tab routing branch so menu-syntax keyboard stays
                            // consistent with the Agent Chat slash / @ composer pickers.
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_object_selector_owns_main_keyboard()
                            {
                                let intent = if has_shift {
                                    crate::menu_syntax::InlinePickerKeyIntent::MoveUp
                                } else {
                                    crate::menu_syntax::InlinePickerKeyIntent::Apply
                                };
                                if this.apply_menu_syntax_object_selector_intent(
                                    intent, window, cx,
                                ) {
                                    cx.stop_propagation();
                                    return;
                                }
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_trigger_picker_owns_main_keyboard()
                            {
                                let intent = if has_shift {
                                    crate::menu_syntax::InlinePickerKeyIntent::MoveUp
                                } else {
                                    crate::menu_syntax::InlinePickerKeyIntent::Apply
                                };
                                if this.apply_menu_syntax_trigger_picker_intent(intent, window, cx) {
                                    cx.stop_propagation();
                                    return;
                                }
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && this.handle_menu_syntax_form_key_input(
                                    key,
                                    event.keystroke.key_char.as_deref(),
                                    &event.keystroke.modifiers,
                                    window,
                                    cx,
                                )
                            {
                                cx.stop_propagation();
                                return;
                            }

                            if this.menu_syntax_capture_form_owns_input() {
                                if has_shift {
                                    this.focus_previous_menu_syntax_form_field(window, cx);
                                } else {
                                    this.focus_next_menu_syntax_form_field(window, cx);
                                }
                                cx.stop_propagation();
                                return;
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && this.try_navigate_root_file_directory_with_tab(
                                    has_shift, window, cx,
                                )
                            {
                                cx.stop_propagation();
                                return;
                            }

                            // Tab-to-Agent deprecated: Cmd+Enter is the AI entry.
                            // Ghost text acceptance (above) now owns plain Tab.

                            // Agent Chat owns Tab locally. Plain Tab stays swallowed so
                            // the global interceptor cannot re-open a fresh chat.
                            // Shift+Tab is the documented Profile Switcher
                            // shortcut, so route it to the in-chat Profile picker
                            // via the window-aware entry point.
                            if let AppView::AgentChatView { entity, .. } = &this.current_view {
                                if has_shift {
                                    cx.stop_propagation();
                                    // Preserve the prior swallow when another popup
                                    // already owns keyboard input — don't open the
                                    // picker underneath the Actions dialog.
                                    if this.show_actions_popup
                                        || crate::actions::is_actions_window_open()
                                    {
                                        return;
                                    }
                                    tracing::info!(
                                        target: "script_kit::keyboard",
                                        event = "agent_chat_shift_tab_profile_switcher",
                                        "Opening Agent Chat Profile picker from Shift+Tab"
                                    );
                                    let entity = entity.clone();
                                    entity.update(cx, |chat, cx| {
                                        chat.open_profile_trigger_picker_in_window(window, cx);
                                    });
                                    return;
                                }
                                let handled = entity.update(cx, |chat, cx| {
                                    chat.handle_tab_key(false, window, cx)
                                });
                                if handled {
                                    cx.stop_propagation();
                                    return;
                                }
                            }

                            // Forward Tab/Shift+Tab directly to the harness
                            // terminal PTY.  We must NOT call cx.propagate()
                            // here because GPUI's built-in focus-traversal
                            // would consume the Tab keystroke before it reaches
                            // the TermPrompt key handler.  Instead, write the
                            // raw byte to the PTY and stop propagation.
                            if let AppView::QuickTerminalView { entity, .. } = &this.current_view {
                                entity.update(cx, |term, _cx| {
                                    let running = term.terminal.is_running();
                                    let bytes: &[u8] = if has_shift {
                                        b"\x1b[Z" // Shift+Tab (backtab)
                                    } else {
                                        b"\t" // Tab
                                    };
                                    if !running {
                                        tracing::warn!(
                                            event = "quick_terminal_tab_pty_dead",
                                            has_shift,
                                            "Tab intercepted but PTY is not running"
                                        );
                                        return;
                                    }
                                    match term.terminal.input(bytes) {
                                        Ok(()) => tracing::debug!(
                                            event = "quick_terminal_tab_sent",
                                            has_shift,
                                            "Tab byte written to PTY"
                                        ),
                                        Err(e) => tracing::warn!(
                                            event = "quick_terminal_tab_write_failed",
                                            error = %e,
                                            has_shift,
                                            "Failed to write Tab to PTY"
                                        ),
                                    }
                                });
                                cx.stop_propagation();
                                return;
                            }

                            // Block Tab while the save-offer overlay is visible
                            if this.tab_ai_save_offer_state.is_some() {
                                cx.stop_propagation();
                                return;
                            }
                        });
                    }
                }

                // Keep plain Enter routed to Agent Chat mention acceptance in the
                // embedded main-window host when the picker is open, and to
                // the menu-syntax trigger picker when it is open on
                // ScriptList (Accept the selected qualifier / capture
                // target — same behavior as the Agent Chat composer picker).
                if is_plain_enter {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_object_selector_owns_main_keyboard()
                            {
                                if this.apply_menu_syntax_object_selector_intent(
                                    crate::menu_syntax::InlinePickerKeyIntent::Accept,
                                    window,
                                    cx,
                                ) {
                                    cx.stop_propagation();
                                    return;
                                }
                            }
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_trigger_picker_owns_main_keyboard()
                            {
                                if this.apply_menu_syntax_trigger_picker_intent(
                                    crate::menu_syntax::InlinePickerKeyIntent::Accept,
                                    window,
                                    cx,
                                ) {
                                    this.arm_menu_syntax_trigger_picker_enter_guard(
                                        "global_plain_enter_script_list",
                                    );
                                    cx.stop_propagation();
                                    return;
                                }
                            }
                            if let AppView::AgentChatView { entity, .. } = &this.current_view {
                                let handled =
                                    entity.update(cx, |chat, cx| chat.handle_enter_key(cx));
                                if handled {
                                    cx.stop_propagation();
                                }
                            }
                        });
                    }
                }
            }
        });
        app.gpui_input_subscriptions.push(tab_interceptor);

        // Prewarm the Agent Chat agent config on a background thread so AI-entry
        // shortcuts do not block on bun transpile of ~/.scriptkit/config.ts.
        crate::ai::agent_chat::ui::prewarm_agent_config();

        // Prewarm Agent Chat Chat and the Tab AI harness asynchronously so AI-entry
        // shortcuts do not pay subprocess/session startup cost on submit.
        let app_entity_for_tab_ai_warm = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;
            let _ = cx.update(|cx| {
                let Some(app) = app_entity_for_tab_ai_warm.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.warm_agent_chat_on_startup(cx);
                    this.warm_tab_ai_harness_on_startup(cx);
                });
            });
        })
        .detach();

        // Add interceptor for actions popup in FileSearchView and ScriptList
        // This handles Cmd+K (toggle), Escape (close), Enter (submit), and typing
        let app_entity_for_actions = cx.entity().downgrade();
        let actions_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_actions;
            move |event, window, cx| {
                let is_notes = crate::notes::is_notes_window(window);
                let is_detached_agent_chat = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
                let is_actions = crate::actions::is_actions_window(window);
                let is_shortcut_recorder = super::shortcut_recorder::is_shortcut_recorder_window(window);

                // A detached actions popup hosted by a secondary window (Notes,
                // detached Agent Chat) owns its keys via ActionsWindow::on_key_down.
                // Routing them through the main app's dialog router would land on
                // the wrong (or absent) dialog entity and swallow every keystroke.
                if is_actions && !crate::actions::is_actions_window_open_for_main() {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;
                let has_shift = event.keystroke.modifiers.shift;
                let key_char = event.keystroke.key_char.as_deref();
                let is_actions_close_key = crate::ui_foundation::is_key_escape(key)
                    || (has_cmd && key.eq_ignore_ascii_case("k") && !has_shift);

                // Agent Chat can open the shared actions dialog from its own focused
                // composer even when the launcher visibility flag is false.
                // Close keys still need to reach the shared dialog before the
                // hidden-window guard below has a chance to skip them.
                if is_actions_close_key {
                    let mut close_key_routed = false;
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if !is_actions
                                && !this.show_actions_popup
                                && !crate::actions::is_actions_window_open_for_main()
                            {
                                return;
                            }
                            if let crate::app_impl::actions_dialog::ActionsSupport::SharedDialog(
                                host,
                            ) = this.actions_support_for_view()
                            {
                                match this.route_key_to_actions_dialog(
                                    key,
                                    key_char,
                                    &event.keystroke.modifiers,
                                    host,
                                    window,
                                    cx,
                                ) {
                                    ActionsRoute::NotHandled => {}
                                    ActionsRoute::Handled | ActionsRoute::Execute { .. } => {
                                        tracing::info!(
                                            target: "script_kit::actions",
                                            event = if is_actions {
                                                "actions_interceptor_routed_from_actions_window"
                                            } else {
                                                "actions_interceptor_routed_close_before_visibility_guard"
                                            },
                                            host = ?host,
                                            key = %key,
                                        );
                                        cx.stop_propagation();
                                        close_key_routed = true;
                                    }
                                }
                            }
                        });
                    }
                    if close_key_routed {
                        return;
                    }
                }

                if is_actions {
                    let mut actions_key_routed = false;
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            logging::log(
                                "KEY",
                                &format!(
                                    "Actions interceptor saw actions-window key={} shortcut={} popup={} focused_input={:?}",
                                    key,
                                    crate::shortcuts::keystroke_to_shortcut(
                                        key,
                                        &event.keystroke.modifiers,
                                    ),
                                    this.show_actions_popup,
                                    this.focused_input
                                ),
                            );
                            if let crate::app_impl::actions_dialog::ActionsSupport::SharedDialog(
                                host,
                            ) = this.actions_support_for_view()
                            {
                                match this.route_key_to_actions_dialog(
                                    key,
                                    key_char,
                                    &event.keystroke.modifiers,
                                    host,
                                    window,
                                    cx,
                                ) {
                                    ActionsRoute::NotHandled => {}
                                    ActionsRoute::Handled => {
                                        tracing::debug!(
                                            target: "script_kit::actions",
                                            event = "actions_interceptor_routed_from_actions_window",
                                            host = ?host,
                                            key = %key,
                                        );
                                        cx.stop_propagation();
                                        actions_key_routed = true;
                                    }
                                    ActionsRoute::Execute {
                                        action_id,
                                        should_close,
                                    } => {
                                        this.execute_actions_route_action(
                                            host,
                                            action_id,
                                            should_close,
                                            window,
                                            cx,
                                        );
                                        cx.stop_propagation();
                                        actions_key_routed = true;
                                    }
                                }
                            }
                        });
                    }
                    if actions_key_routed {
                        return;
                    }
                    return;
                }

                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // Skip keystrokes from secondary windows — interceptors are
                // GLOBAL and fire for ALL windows.  Secondary windows own
                // their own Cmd+K/Escape/Enter handling.
                if is_notes || is_detached_agent_chat || is_shortcut_recorder {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "actions_interceptor_skipped_secondary_window",
                        is_notes,
                        is_detached_agent_chat,
                        is_actions,
                        is_shortcut_recorder,
                    );
                    return;
                }

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, cx| {
                        tracing::debug!(
                            target: "script_kit::keyboard",
                            event = "actions_interceptor_owner_path",
                            current_view = %this.app_view_name(),
                            key = %key,
                            has_cmd,
                            has_shift,
                            has_key_char = key_char.is_some(),
                            show_actions_popup = this.show_actions_popup,
                        );
                        if has_cmd {
                            logging::log(
                                "KEY",
                                &format!(
                                    "Actions interceptor saw key={} shortcut={} view={} popup={} focused_input={:?}",
                                    key,
                                    crate::shortcuts::keystroke_to_shortcut(
                                        key,
                                        &event.keystroke.modifiers,
                                    ),
                                    this.app_view_name(),
                                    this.show_actions_popup,
                                    this.focused_input
                                ),
                            );
                        }

                        if this.main_window_modal_owns_keyboard() {
                            return;
                        }

                        // Route displayed action shortcuts even when the popup is closed.
                        if this.try_execute_main_list_action_shortcut_from_display(
                            key,
                            &event.keystroke.modifiers,
                            window,
                            cx,
                        ) {
                            cx.stop_propagation();
                            return;
                        }

                        // Handle Cmd+K to toggle actions popup (works in ScriptList, FileSearchView, ArgPrompt, etc.)
                        // This MUST be intercepted here because the Input component has focus and
                        // normal on_key_down handlers won't receive the event.
                        // Delegates to the shared per-view dispatcher so that Cmd+K and footer
                        // clicks always route through the same code path.
                        if has_cmd && key.eq_ignore_ascii_case("k") && !has_shift {
                            if this.dispatch_actions_toggle_for_current_view(window, cx, "cmd_k_interceptor") {
                                cx.stop_propagation();
                                return;
                            }
                        }

                        // Handle Cmd+W for AgentChatView (close the window entirely)
                        if has_cmd && key.eq_ignore_ascii_case("w") && !has_shift
                            && matches!(this.current_view, AppView::AgentChatView { .. })
                        {
                            tracing::info!(
                                target: "script_kit::keyboard",
                                event = "embedded_agent_chat_cmd_w_close_window",
                            );
                            logging::log("KEY", "Interceptor: Cmd+W -> close window from Agent Chat");
                            this.close_tab_ai_harness_terminal_with_window(window, cx);
                            this.close_and_reset_window(cx);
                            cx.stop_propagation();
                            return;
                        }

                        let agent_chat_escape_popup_open = match &this.current_view {
                            AppView::AgentChatView { entity, .. } => {
                                entity.read(cx).has_escape_dismissible_popup()
                            }
                            _ => false,
                        };
                        let agent_chat_escape_focused_text_origin = match &this.current_view {
                            AppView::AgentChatView { entity, .. } => {
                                let chat = entity.read(cx);
                                chat.is_focused_text_mini()
                                    || chat.focused_text_originated_from_quick_prompt()
                            }
                            _ => false,
                        };

                        let agent_chat_escape_cancelled_streaming = if crate::ui_foundation::is_key_escape(key)
                            && !has_cmd
                            && !has_shift
                            && !agent_chat_escape_focused_text_origin
                        {
                            match &this.current_view {
                                AppView::AgentChatView { entity, .. } => entity.update(cx, |chat, cx| {
                                    chat.cancel_streaming_from_escape(cx)
                                }),
                                _ => false,
                            }
                        } else {
                            false
                        };
                        if agent_chat_escape_cancelled_streaming {
                            logging::log(
                                "KEY",
                                "Interceptor: Escape -> cancel Agent Chat streaming",
                            );
                            cx.stop_propagation();
                            return;
                        }

                        // Handle Escape for AgentChatView.
                        if crate::ui_foundation::is_key_escape(key) && !has_cmd && !has_shift
                            && !this.show_actions_popup
                            && !agent_chat_escape_popup_open
                            && matches!(this.current_view, AppView::AgentChatView { .. })
                        {
                            if agent_chat_escape_focused_text_origin {
                                tracing::info!(
                                    target: "script_kit::keyboard",
                                    event = "focused_text_quick_prompt_escape_hide_requested",
                                );
                                this.close_agent_chat_main_window_state_first(cx);
                                logging::log("KEY", "Interceptor: Escape -> hide focused-text quick prompt Agent Chat");
                                cx.stop_propagation();
                                return;
                            }
                            if this.opened_from_main_menu {
                                tracing::info!(
                                    target: "script_kit::keyboard",
                                    event = "embedded_agent_chat_escape_return_to_origin",
                                );
                                this.close_tab_ai_harness_terminal_with_window(window, cx);
                                logging::log("KEY", "Interceptor: Escape -> return to main menu from Agent Chat");
                            } else {
                                tracing::info!(
                                    target: "script_kit::keyboard",
                                    event = "embedded_agent_chat_escape_close_window",
                                );
                                this.close_agent_chat_main_window_state_first(cx);
                                logging::log("KEY", "Interceptor: Escape -> close Agent Chat window");
                            }
                            cx.stop_propagation();
                            return;
                        }

                        // Handle Cmd+I to toggle info panel in ScriptList
                        // Must be intercepted here because the Input component has focus
                        if has_cmd && key.eq_ignore_ascii_case("i") && !has_shift
                            && matches!(this.current_view, AppView::ScriptList)
                        {
                            logging::log("KEY", "Interceptor: Cmd+I -> toggle_info (ScriptList)");
                            this.toggle_info_panel(cx);
                            cx.stop_propagation();
                            return;
                        }

                        // Window tweaker shortcuts (only enabled with SCRIPT_KIT_WINDOW_TWEAKER=1)
                        let window_tweaker_enabled = std::env::var("SCRIPT_KIT_WINDOW_TWEAKER")
                            .map(|v| v == "1")
                            .unwrap_or(false);

                        if window_tweaker_enabled {
                            // Handle Cmd+- to decrease light theme opacity
                            if has_cmd
                                && !has_shift
                                && (key == "-" || key.eq_ignore_ascii_case("minus"))
                            {
                                logging::log("KEY", &format!("Interceptor: Cmd+- (key={}) -> decrease light opacity", key));
                                this.adjust_light_opacity(-0.05, cx);
                                cx.stop_propagation();
                                return;
                            }

                            // Handle Cmd+= (or Cmd+Shift+=) to increase light theme opacity
                            if has_cmd
                                && (key == "="
                                    || key.eq_ignore_ascii_case("equal")
                                    || key.eq_ignore_ascii_case("plus"))
                            {
                                logging::log("KEY", &format!("Interceptor: Cmd+= (key={}) -> increase light opacity", key));
                                this.adjust_light_opacity(0.05, cx);
                                cx.stop_propagation();
                                return;
                            }

                            // Handle Cmd+M to cycle vibrancy material (blur effect)
                            if has_cmd && !has_shift && key.eq_ignore_ascii_case("m") {
                                logging::log("KEY", "Interceptor: Cmd+M -> cycle vibrancy material");
                                let description = platform::cycle_vibrancy_material();
                                this.toast_manager.push(
                                    components::toast::Toast::info(
                                        description,
                                        &this.theme,
                                    )
                                    .duration_ms(Some(TOAST_INFO_MS)),
                                );
                                cx.notify();
                                cx.stop_propagation();
                                return;
                            }

                            // Handle Cmd+Shift+A to cycle vibrancy appearance (VibrantLight, VibrantDark, etc.)
                            if has_cmd && has_shift && key.eq_ignore_ascii_case("a") {
                                logging::log("KEY", "Interceptor: Cmd+Shift+A -> cycle vibrancy appearance");
                                let description = platform::cycle_appearance();
                                this.toast_manager.push(
                                    components::toast::Toast::info(
                                        description,
                                        &this.theme,
                                    )
                                    .duration_ms(Some(TOAST_INFO_MS)),
                                );
                                cx.notify();
                                cx.stop_propagation();
                                return;
                            }
                        }

                        // Only handle remaining keys if in FileSearchView with actions popup open
                        if !matches!(this.current_view, AppView::FileSearchView { .. }) {
                            // Arrow keys are handled by arrow_interceptor to avoid double-processing
                            // (which can skip 2 items per keypress when both interceptors handle arrows).
                            if crate::ui_foundation::is_key_up(key)
                                || crate::ui_foundation::is_key_down(key)
                            {
                                return;
                            }

                            // Route modal actions keys for all views that support actions dialogs.
                            // This ensures enter, escape, backspace, and character keys are
                            // routed to the actions dialog when it's open, regardless of view type.
                            // Uses the canonical resolver so host decisions live in one place.
                            if let crate::app_impl::actions_dialog::ActionsSupport::SharedDialog(host)
                                = this.actions_support_for_view()
                            {
                                match this.route_key_to_actions_dialog(
                                    key,
                                    key_char,
                                    &event.keystroke.modifiers,
                                    host,
                                    window,
                                    cx,
                                ) {
                                    ActionsRoute::NotHandled => {}
                                    ActionsRoute::Handled => {
                                        cx.stop_propagation();
                                        return;
                                    }
                                    ActionsRoute::Execute {
                                        action_id,
                                        should_close,
                                    } => {
                                        this.execute_actions_route_action(
                                            host,
                                            action_id,
                                            should_close,
                                            window,
                                            cx,
                                        );
                                        cx.stop_propagation();
                                        return;
                                    }
                                }
                            }
                            return;
                        }

                        // Handle Enter/Escape for AI preset views
                        // These don't have actions dialogs but need key routing through the interceptor
                        // because the Input component has focus
                        if matches!(
                            this.current_view,
                            AppView::SearchAiPresetsView { .. }
                                | AppView::CreateAiPresetView { .. }
                        ) {
                            if matches!(this.current_view, AppView::CreateAiPresetView { .. }) {
                                this.handle_create_ai_preset_key(key, window, cx);
                                cx.stop_propagation();
                            } else {
                                this.handle_search_ai_presets_key(key, window, cx);
                                if crate::ui_foundation::is_key_enter(key)
                                    || crate::ui_foundation::is_key_escape(key)
                                {
                                    cx.stop_propagation();
                                }
                            }
                            return;
                        }

                        // Handle keys for Favorites view
                        if matches!(this.current_view, AppView::FavoritesBrowseView { .. }) {
                            let has_cmd = event.keystroke.modifiers.platform;
                            this.handle_favorites_browse_key(key, has_cmd, window, cx);
                            if crate::ui_foundation::is_key_enter(key)
                                || crate::ui_foundation::is_key_escape(key)
                                || (has_cmd && key.eq_ignore_ascii_case("k"))
                                || (key.len() == 1
                                    && this.filter_text.is_empty()
                                    && matches!(
                                        key,
                                        "d" | "D" | "u" | "U" | "j" | "J"
                                    ))
                            {
                                cx.stop_propagation();
                            }
                            return;
                        }


                        // Only handle remaining keys if actions popup is open (FileSearchView)
                        if !this.show_actions_popup {
                            return;
                        }

                        // Handle Escape to close actions popup
                        if crate::ui_foundation::is_key_escape(key) {
                            this.close_actions_popup(ActionsDialogHost::FileSearch, window, cx);
                            cx.stop_propagation();
                            return;
                        }

                        // Handle Enter to submit selected action
                        if crate::ui_foundation::is_key_enter(key) {
                            if let Some(ref dialog) = this.actions_dialog {
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();

                                if let Some(action_id) = action_id {
                                    crate::logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "FileSearch actions executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );

                                    if should_close {
                                        this.close_actions_popup(
                                            ActionsDialogHost::FileSearch,
                                            window,
                                            cx,
                                        );
                                    }

                                    if action_id.starts_with("day_page:")
                                        && this.execute_day_page_action(&action_id, window, cx)
                                    {
                                        cx.stop_propagation();
                                        return;
                                    }

                                    // Use handle_action instead of trigger_action_by_name
                                    // handle_action supports both built-in actions (open_file, quick_look, etc.)
                                    // and SDK actions
                                    this.handle_action(action_id, window, cx);
                                }
                            }
                            cx.stop_propagation();
                            return;
                        }

                        // Handle Backspace for actions search
                        if key.eq_ignore_ascii_case("backspace") {
                            if let Some(ref dialog) = this.actions_dialog {
                                dialog.update(cx, |d, cx| {
                                    d.backspace_search_input(window, cx);
                                });
                                crate::actions::notify_actions_window(cx);
                                crate::actions::resize_actions_window(cx, dialog);
                            }
                            cx.stop_propagation();
                            return;
                        }

                        // Handle printable character input for actions search
                        if let Some(chars) = key_char {
                            if let Some(ch) = chars.chars().next() {
                                if ch.is_ascii_graphic() || ch == ' ' {
                                    if let Some(ref dialog) = this.actions_dialog {
                                        dialog.update(cx, |d, cx| {
                                            d.insert_search_text(ch.to_string(), window, cx);
                                        });
                                        crate::actions::notify_actions_window(cx);
                                        crate::actions::resize_actions_window(cx, dialog);
                                    }
                                    cx.stop_propagation();
                                }
                            }
                        }
                    });
                }
            }
        });
        app.gpui_input_subscriptions.push(actions_interceptor);

        // CRITICAL FIX: Sync list state on initialization
        // This was removed when state mutations were moved out of render(),
        // but we still need to sync once during initialization so the list
        // knows about the scripts that were loaded.
        // Without this, the first render shows "No scripts or snippets found"
        // because main_list_state starts with 0 items.
        app.sync_list_state();
        app.validate_selection_bounds(cx);

        app

impl Focusable for ScriptListApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ScriptListApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let global_theme_revision = crate::theme::service::theme_revision();
        if self.theme_revision_seen != global_theme_revision {
            self.theme_revision_seen = global_theme_revision;
            self.update_theme(cx);
        }

        // Track render timing for filter perf analysis
        let render_start = std::time::Instant::now();
        let filter_snapshot = self.filter_text.clone();
        self.log_current_view_transition_if_changed("render");
        if matches!(self.current_view, AppView::ScriptList)
            && self.computed_filter_text == filter_snapshot
            && self
                .history_filter_render_pending
                .as_deref()
                .is_some_and(|pending| pending == filter_snapshot)
        {
            let rendered_filter = filter_snapshot.clone();
            let app_entity = cx.entity().downgrade();
            window.defer(cx, move |_window, cx| {
                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, _cx| {
                        if this.history_filter_render_pending.as_deref()
                            == Some(rendered_filter.as_str())
                        {
                            this.history_filter_render_pending = None;
                            tracing::info!(
                                target: "script_kit::input_history",
                                event = "history_filter_render_ack",
                                filter_len = rendered_filter.len(),
                            );
                        }
                    });
                }
            });
        }

        // Always log render start for "gr" prefix filters to debug the issue
        if filter_snapshot.starts_with("gr") {
            crate::logging::log(
                "FILTER_PERF",
                &format!(
                    "[FRAME_START] filter='{}' selected_idx={} view={:?}",
                    filter_snapshot,
                    self.selected_index,
                    match &self.current_view {
                        AppView::ScriptList => "ScriptList",
                        _ => "Other",
                    }
                ),
            );
        }

        // Flush any pending toasts to gpui-component's NotificationList
        // This is needed because toast push sites don't have window access
        self.flush_pending_toasts(window, cx);

        // Check for API key configuration completion (from built-in commands)
        // The EnvPrompt callback signals completion via channel
        if let Ok((provider, success)) = self.api_key_completion_receiver.try_recv() {
            let app_entity = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, cx| {
                        this.handle_api_key_completion(provider, success, window, cx);
                    });
                } else {
                    tracing::warn!(
                        "API key completion deferred update skipped because app entity was dropped"
                    );
                }
            });
        }

        // Check for inline chat escape (from built-in ChatPrompt)
        // The ChatPrompt escape callback signals via channel
        if self.inline_chat_escape_receiver.try_recv().is_ok() {
            crate::logging::log(
                "CHAT",
                "Inline chat escape received - returning to main menu",
            );
            self.capture_mini_ai_close_snapshot(MiniAiCloseSource::Escape, cx);
            self.go_back_or_close(window, cx);
        }

        // Flow-session Threadline callbacks (submit / Esc-background / ⌘K).
        // ChatPrompt callbacks have no app access; they post here and the
        // render pass (which has the window) applies them.
        while let Ok(request) = self.flow_chat_receiver.try_recv() {
            match request {
                crate::flows::session::FlowChatRequest::Submit { session_id, text } => {
                    // The composer text already left the input when ChatPrompt
                    // posted this request, so an unconsumed submit (busy /
                    // missing session) must re-stage it as the draft.
                    let result = self.submit_flow_chat_message(session_id, text.clone(), cx);
                    self.stage_unconsumed_flow_message(text, result, cx);
                }
                crate::flows::session::FlowChatRequest::Recovery {
                    session_id,
                    message_id,
                    action,
                } => {
                    tracing::info!(
                        target: "script_kit::flows",
                        event = "flow_turn_recovery_action",
                        session_id,
                        message_id = %message_id,
                        action = ?action,
                        "Recovery action chosen on a failed transcript turn"
                    );
                    self.dispatch_flow_recovery_action(session_id, action, window, cx);
                }
                crate::flows::session::FlowChatRequest::ShowActions { session_id } => {
                    let in_session = matches!(
                        self.current_view,
                        AppView::FlowSessionView { session_id: current } if current == session_id
                    );
                    if in_session {
                        self.dispatch_actions_toggle_for_current_view(
                            window,
                            cx,
                            "flow_session_chat",
                        );
                    }
                }
            }
        }

        while let Ok(request) = self.inline_chat_actions_receiver.try_recv() {
            match request {
                MiniAiUiRequest::ToggleActions { prompt_id, source } => {
                    tracing::info!(
                        target: "script_kit::mini_ai",
                        event = "mini_ai_actions_dispatch",
                        prompt_id = %prompt_id,
                        source,
                        main_window_mode = ?self.main_window_mode,
                        "Dispatching Mini AI actions request through parent window"
                    );
                    self.dispatch_actions_toggle_for_current_view(window, cx, source);
                }
            }
        }

        // Check for inline chat continue (Continue in Harness Terminal → hide main window)
        if self.inline_chat_continue_receiver.try_recv().is_ok() {
            crate::logging::log(
                "CHAT",
                "Inline chat continue received - hiding main window for AI handoff",
            );
            // Reset state and visibility tracking
            script_kit_gpui::set_main_window_visible(false);
            self.is_pinned = false;
            self.set_main_window_mode(MainWindowMode::Mini, window, cx, "inline_chat_continue");
            self.reset_to_script_list(cx);
            // Hide the main window directly via GPUI. We cannot use
            // defer_hide_main_window here because the window_manager may not
            // have the NSPanel handle registered. We cannot use cx.hide()
            // because that hides the entire app including the AI window.
            // window.remove_window() is GPUI's window-level close which
            // hides this specific window without affecting other windows.
            window.remove_window();
        }

        // Check for inline chat configure (user wants to set up API key)
        // The ChatPrompt configure callback signals via channel
        if self.inline_chat_configure_receiver.try_recv().is_ok() {
            crate::logging::log(
                "CHAT",
                "Inline chat configure received - showing API key setup",
            );
            // First close the chat prompt
            self.go_back_or_close(window, cx);
            // Then show the direct-provider API key configuration prompt
            self.show_api_key_prompt(
                "SCRIPT_KIT_OPENAI_API_KEY",
                "Enter your OpenAI API key",
                "OpenAI",
                cx,
            );
        }

        // Check for inline chat Claude Code (user wants to enable Claude Code)
        // The ChatPrompt Claude Code callback signals via channel
        if self.inline_chat_claude_code_receiver.try_recv().is_ok() {
            crate::logging::log(
                "CHAT",
                "Inline chat Claude Code received - enabling Claude Code",
            );
            // Enable Claude Code in config.ts
            self.enable_claude_code_in_config(window, cx);
        }

        // Check for naming dialog completion (submit or cancel)
        if let Ok(payload) = self.naming_submit_receiver.try_recv() {
            self.handle_naming_dialog_completion(payload, window, cx);
        }

        // Focus-lost auto-dismiss: Close dismissable prompts when the main window loses focus
        // This includes focus loss to other app windows like Notes/AI.
        // When is_pinned is true, the window stays open on blur (only closes via ESC/Cmd+W)
        let is_window_focused = platform::is_main_window_focused();
        if !self.was_window_focused && is_window_focused {
            logging::log("FOCUS", "Main window gained focus");

            if matches!(self.current_view, AppView::FileSearchView { .. }) {
                let stopped_drag = cx.stop_active_drag(window);
                self.pending_focus = Some(FocusTarget::MainFilter);
                self.focused_input = FocusedInput::MainFilter;
                logging::log(
                    "FOCUS",
                    &format!(
                        "FileSearch focus gained: stopped_drag={} pending_focus=MainFilter",
                        stopped_drag
                    ),
                );
                tracing::debug!(
                    target: "script_kit::keyboard",
                    event = "file_search_refocus_restored_keyboard",
                    stopped_drag,
                    "File search restored main-filter keyboard focus after refocus"
                );
            }

            // Close popups when the main window regains focus (user clicked on it)
            if confirm::is_confirm_window_open() {
                logging::log(
                    "FOCUS",
                    "Main window regained focus - closing confirm popup",
                );
                confirm::route_key_to_confirm_popup("escape", cx);
            }
            if actions::is_actions_window_open() {
                logging::log(
                    "FOCUS",
                    "Main window regained focus - closing actions popup via canonical close path",
                );
                self.close_actions_popup_for_current_view(window, cx);
            }
        }
        if self.was_window_focused && !is_window_focused {
            let actions_popup_active_or_closing = self.show_actions_popup
                || self.actions_dialog.is_some()
                || actions::is_actions_window_open();
            if matches!(self.current_view, AppView::FileSearchView { .. }) {
                logging::log(
                    "FOCUS",
                    &format!(
                        "FileSearch focus lost: visible={} pinned={} actions_open={} confirm_open={}",
                        script_kit_gpui::is_main_window_visible(),
                        self.is_pinned,
                        actions::is_actions_window_open(),
                        confirm::is_confirm_window_open()
                    ),
                );
            }
            // Window just lost focus (user clicked another window)
            // Only auto-dismiss if we're in a dismissable view AND window is visible AND not pinned
            // AND the actions popup is not open (actions popup is a companion window, not "losing focus")
            if self.is_dismissable_view()
                && script_kit_gpui::is_main_window_visible()
                && !self.is_pinned
                && !actions_popup_active_or_closing
                && !confirm::is_confirm_window_open()
                && !ai::agent_chat::ui::chat_window::is_chat_window_open()
                && !crate::dictation::is_dictation_overlay_open()
                && !crate::dictation::is_dictation_recording()
                && self.tab_ai_save_offer_state.is_none()
                && self.shortcut_recorder_state.is_none()
            {
                if matches!(self.current_view, AppView::ScriptList) {
                    logging::log(
                        "FOCUS",
                        "Main window lost focus in ScriptList - hiding while preserving state",
                    );
                    self.hide_main_window_preserving_state_for_focus_loss(cx);
                } else {
                    logging::log(
                        "FOCUS",
                        "Main window lost focus in dismissable non-ScriptList view - closing and resetting",
                    );
                    self.close_and_reset_window(cx);
                }
            } else if actions_popup_active_or_closing {
                logging::log(
                    "FOCUS",
                    "Main window lost focus but actions popup is open or closing - staying open",
                );
            } else if self.shortcut_recorder_state.is_some() {
                logging::log(
                    "FOCUS",
                    "Main window lost focus but shortcut recorder is open - staying open",
                );
            } else if self.tab_ai_save_offer_state.is_some() {
                logging::log(
                    "FOCUS",
                    "Main window lost focus but Tab AI save-offer is active - staying open",
                );
            } else if confirm::is_confirm_window_open() {
                logging::log(
                    "FOCUS",
                    "Main window lost focus but confirm popup is open - staying open",
                );
            } else if self.is_pinned {
                logging::log(
                    "FOCUS",
                    "Main window lost focus but is pinned - staying open",
                );
            }
        }
        self.was_window_focused = is_window_focused;

        // Apply pending focus request (if any). This is the new "apply once" mechanism
        // that replaces the old "perpetually enforce focus in render()" pattern.
        // Focus is applied exactly once when pending_focus is set, then cleared.
        self.apply_pending_focus(window, cx);

        self.sync_main_window_resize_lock(window, cx);

        // Sync filter input if needed (views that use shared input)
        if matches!(
            self.current_view,
            AppView::ScriptList
                | AppView::ClipboardHistoryView { .. }
                | AppView::EmojiPickerView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::BrowserTabsView { .. }
                | AppView::FileSearchView { .. }
                | AppView::ProfileSearchView { .. }
                | AppView::ThemeChooserView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::FlowUxView { .. }
                | AppView::FlowSessionView { .. }
                | AppView::SettingsView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::FavoritesBrowseView { .. }
                | AppView::AgentChatHistoryView { .. }
                | AppView::BrowserHistoryView { .. }
                | AppView::DictationHistoryView { .. }
                | AppView::NotesBrowseView { .. }
                | AppView::TipsView { .. }
                | AppView::MiniPrompt { .. }
                | AppView::ArgPrompt { .. }
                | AppView::DayPage { .. }
        ) {
            self.sync_filter_input_if_needed(window, cx);
        }

        // NOTE: Prompt messages are now handled via event-driven async_channel listener
        // spawned in execute_interactive() - no polling needed in render()

        self.sync_main_footer_popup(window, cx);

        // Native footer active in glass mode: the GPUI fallback rail isn't
        // rendered, so no capsule sync ever prunes the groups it registered
        // during startup frames — drop them or they linger as ghost capsules
        // at the container's bottom edge.
        if crate::footer_popup::glass_scroll_bands_active() && self.main_window_uses_native_footer()
        {
            crate::components::footer_chrome::remove_glass_capsule_window(window);
        }

        // P0-4: Clone current_view only for dispatch (needed to call &mut self methods)
        // The clone is unavoidable due to borrow checker: we need &mut self for render methods
        // but also need to match on self.current_view. Future optimization: refactor render
        // methods to take &str/&[T] references instead of owned values.
        //
        // HUD is now handled by hud_manager as a separate floating window
        // No need to render it as part of this view
        let view_owns_main_window_shell = self.current_view.uses_view_owned_main_window_shell(&*cx);
        let current_view = self.current_view.clone();
        let main_window_modal_dim_active = confirm::is_confirm_window_open()
            || self.shortcut_recorder_state.is_some()
            || self.shortcut_recorder_entity.is_some();
        let main_window_modal_dim_layer =
            crate::components::modal_dim::render_main_window_modal_dim_layer(
                main_window_modal_dim_active,
                &self.theme,
            );
        let main_content: AnyElement = match current_view {
            AppView::ScriptList => self.render_script_list(cx).into_any_element(),
            AppView::About {
                state,
                update_state,
                ..
            } => {
                let app = cx.entity().downgrade();
                let dismiss_app = app.clone();
                let github_app = app.clone();
                let discord_app = app.clone();
                let follow_app = app.clone();
                let check_app = app.clone();
                let release_app = app.clone();
                let toggle_app = app.clone();
                let key_app = app;
                let actions = crate::about::render::AboutSurfaceActions {
                    dismiss: std::rc::Rc::new(move |_event, _window, cx| {
                        if let Some(app) = dismiss_app.upgrade() {
                            app.update(cx, |this, cx| this.dismiss_about(cx));
                        }
                    }),
                    open_github: std::rc::Rc::new(move |_event, _window, _cx| {
                        if let Err(error) = open::that(crate::branding::URL_GITHUB) {
                            logging::log("ABOUT", &format!("Failed to open GitHub: {}", error));
                        }
                        let _ = github_app.upgrade();
                    }),
                    open_discord: std::rc::Rc::new(move |_event, _window, _cx| {
                        if let Err(error) = open::that(crate::branding::URL_DISCORD) {
                            logging::log("ABOUT", &format!("Failed to open Discord: {}", error));
                        }
                        let _ = discord_app.upgrade();
                    }),
                    follow_x: std::rc::Rc::new(move |_event, _window, _cx| {
                        if let Err(error) = open::that(crate::branding::URL_FOLLOW_US) {
                            logging::log("ABOUT", &format!("Failed to open X: {}", error));
                        }
                        let _ = follow_app.upgrade();
                    }),
                    check_updates: std::rc::Rc::new({
                        let update_state = update_state.clone();
                        move |_event, _window, cx| {
                            let state = update_state.clone();
                            let app = check_app.clone();
                            let (complete_tx, complete_rx) = async_channel::bounded(1);
                            crate::updates::check_now(
                                state,
                                crate::updates::CheckKind::Manual,
                                move || {
                                    let _ = complete_tx.send_blocking(());
                                },
                            );
                            if let Some(app) = app.upgrade() {
                                app.update(cx, |_this, cx| cx.notify());
                            }
                            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                                let _ = complete_rx.recv().await;
                                cx.update(move |cx| {
                                    if let Some(app) = app.upgrade() {
                                        app.update(cx, |_this, cx| cx.notify());
                                    }
                                });
                            })
                            .detach();
                        }
                    }),
                    open_release: std::rc::Rc::new({
                        let update_state = update_state.clone();
                        move |_event, _window, _cx| {
                            let snapshot = update_state
                                .read()
                                .map(|guard| guard.clone())
                                .unwrap_or_else(|_| crate::updates::UpdateState::Idle);
                            if let Some(url) = snapshot.release_page_url() {
                                if let Err(error) = open::that(url) {
                                    logging::log(
                                        "ABOUT",
                                        &format!("Failed to open release page: {}", error),
                                    );
                                }
                            }
                            let _ = release_app.upgrade();
                        }
                    }),
                    toggle_acknowledgements: std::rc::Rc::new(move |_event, _window, cx| {
                        if let Some(app) = toggle_app.upgrade() {
                            app.update(cx, |this, cx| this.toggle_about_acknowledgements(cx));
                        }
                    }),
                    key_down: std::rc::Rc::new(move |event, _window, cx| {
                        if crate::ui_foundation::is_key_escape(event.keystroke.key.as_str()) {
                            if let Some(app) = key_app.upgrade() {
                                app.update(cx, |this, cx| this.dismiss_about(cx));
                            }
                            cx.stop_propagation();
                        } else {
                            cx.propagate();
                        }
                    }),
                };
                crate::about::render::render_about_surface(
                    &state,
                    update_state,
                    &self.focus_handle,
                    actions,
                    window,
                    cx,
                )
                .into_any_element()
            }
            AppView::ActionsDialog => self.render_actions_dialog(cx),
            AppView::ArgPrompt {
                id,
                placeholder,
                choices,
                actions,
            } => self
                .render_arg_prompt(id, placeholder, choices, actions, cx)
                .into_any_element(),
            AppView::DivPrompt { id, entity } => {
                self.render_div_prompt(id, entity, cx).into_any_element()
            }
            AppView::FormPrompt { entity, .. } => {
                self.render_form_prompt(entity, cx).into_any_element()
            }
            AppView::TermPrompt { entity, .. } => {
                self.render_term_prompt(entity, cx).into_any_element()
            }
            AppView::EditorPrompt { entity, .. } => {
                self.render_editor_prompt(entity, cx).into_any_element()
            }
            AppView::SelectPrompt { entity, .. } => {
                self.render_select_prompt(entity, cx).into_any_element()
            }
            AppView::PathPrompt { entity, .. } => {
                self.render_path_prompt(entity, cx).into_any_element()
            }
            AppView::EnvPrompt { entity, .. } => {
                self.render_env_prompt(entity, cx).into_any_element()
            }
            AppView::DropPrompt { entity, .. } => {
                self.render_drop_prompt(entity, cx).into_any_element()
            }
            AppView::TemplatePrompt { entity, .. } => {
                self.render_template_prompt(entity, cx).into_any_element()
            }
            AppView::HotkeyPrompt { entity, .. } => self
                .render_hotkey_prompt(entity, window, cx)
                .into_any_element(),
            AppView::ChatPrompt { entity, .. } => {
                self.render_chat_prompt(entity, cx).into_any_element()
            }
            AppView::MiniPrompt {
                id,
                placeholder,
                choices,
            } => self
                .render_mini_prompt(id, placeholder, choices, cx)
                .into_any_element(),
            AppView::MicroPrompt {
                id,
                placeholder,
                choices,
            } => self
                .render_micro_prompt(id, placeholder, choices, cx)
                .into_any_element(),
            // P0 FIX: View state only - data comes from self.cached_clipboard_entries
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => self
                .render_clipboard_history(filter, selected_index, cx)
                .into_any_element(),
            AppView::EmojiPickerView {
                filter,
                selected_index,
                selected_category,
            } => self
                .render_emoji_picker(filter, selected_index, selected_category, cx)
                .into_any_element(),
            // P0 FIX: View state only - data comes from self.apps
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => self
                .render_app_launcher(filter, selected_index, cx)
                .into_any_element(),
            // P0 FIX: View state only - data comes from self.cached_windows
            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => self
                .render_window_switcher(filter, selected_index, cx)
                .into_any_element(),
            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => self
                .render_browser_tabs(filter, selected_index, cx)
                .into_any_element(),
            AppView::WebcamView { entity } => {
                self.render_webcam_prompt(entity, cx).into_any_element()
            }
            AppView::ScratchPadView { entity, .. } => {
                self.render_editor_prompt(entity, cx).into_any_element()
            }
            AppView::QuickTerminalView { entity, .. } => {
                self.render_term_prompt(entity, cx).into_any_element()
            }
            AppView::FlowSessionView { session_id } => self.render_flow_session(session_id, cx),
            AppView::FileSearchView {
                ref query,
                selected_index,
                presentation,
            } => self
                .render_file_search(query, selected_index, presentation, cx)
                .into_any_element(),
            AppView::ProfileSearchView {
                ref filter,
                selected_index,
            } => self
                .render_profile_search(filter.clone(), selected_index, cx)
                .into_any_element(),
            AppView::ThemeChooserView {
                ref filter,
                selected_index,
            } => self.render_theme_chooser(filter, selected_index, window, cx),
            AppView::NamingPrompt { entity, .. } => {
                self.render_naming_prompt(entity, cx).into_any_element()
            }
            AppView::CreationFeedback { ref payload } => {
                self.render_creation_feedback(payload.clone(), cx)
            }
            AppView::ScriptIssuesView { ref report } => {
                self.render_script_issues_view(report.clone(), cx)
            }
            AppView::SdkReferenceView {
                ref filter,
                selected_index,
                ref entries,
            } => self.render_sdk_reference_view(filter, selected_index, entries.clone(), cx),
            AppView::TipsView {
                ref filter,
                selected_index,
                ref entries,
            } => self.render_tips_view(filter, selected_index, entries.clone(), cx),
            AppView::ScriptTemplateCatalogView {
                ref filter,
                selected_index,
                ref templates,
            } => self.render_script_template_catalog_view(
                filter,
                selected_index,
                templates.clone(),
                cx,
            ),
            AppView::BrowseKitsView {
                ref query,
                selected_index,
                results,
            } => self
                .render_browse_kits(query, selected_index, results, cx)
                .into_any_element(),
            AppView::MigrateV1View {
                ref filter,
                selected_index,
                board,
            } => self
                .render_migrate_v1(filter, selected_index, board, cx)
                .into_any_element(),
            AppView::InstalledKitsView {
                ref filter,
                selected_index,
                kits,
            } => self
                .render_installed_kits(filter, selected_index, kits, cx)
                .into_any_element(),
            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => self
                .render_process_manager(filter, selected_index, cx)
                .into_any_element(),
            AppView::FlowUxView {
                variant,
                filter,
                selected_index,
                inline_run,
            } => self
                .render_flow_ux(variant, filter, selected_index, inline_run, cx)
                .into_any_element(),
            AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } => self
                .render_current_app_commands(filter, selected_index, cx)
                .into_any_element(),
            AppView::SearchAiPresetsView {
                filter,
                selected_index,
            } => self
                .render_search_ai_presets(filter, selected_index, cx)
                .into_any_element(),
            AppView::CreateAiPresetView {
                name,
                system_prompt,
                model,
                active_field,
            } => self
                .render_create_ai_preset(name, system_prompt, model, active_field, cx)
                .into_any_element(),
            AppView::SettingsView {
                filter,
                selected_index,
            } => self.render_settings(filter, selected_index, cx),
            AppView::PermissionsWizardView { selected_index } => {
                self.render_permissions_wizard(selected_index, cx)
            }
            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => self
                .render_favorites_browse(filter, selected_index, cx)
                .into_any_element(),
            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => self
                .render_agent_chat_history(filter, selected_index, cx)
                .into_any_element(),
            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => self
                .render_browser_history(filter, selected_index, cx)
                .into_any_element(),
            AppView::DictationHistoryView {
                filter,
                selected_index,
                visible_limit,
            } => self
                .render_dictation_history(filter, selected_index, visible_limit, cx)
                .into_any_element(),
            AppView::NotesBrowseView { search } => self
                .render_notes_browse_portal(search, cx)
                .into_any_element(),
            AppView::AgentChatView { entity } => entity.into_any_element(),
            AppView::DayPage { entity } => entity.into_any_element(),
            AppView::ConfirmPrompt {
                options,
                focused_button,
                ..
            } => self
                .render_confirm_prompt(options, focused_button, cx)
                .into_any_element(),
        };

        // Wrap content in a container that can have the debug grid overlay
        let window_bounds = window.bounds();
        let window_size = gpui::size(window_bounds.size.width, window_bounds.size.height);
        let detached_regions = crate::footer_popup::main_window_detached_footer_regions_gpui(
            f32::from(window_size.width),
            f32::from(window_size.height),
            if crate::footer_popup::glass_scroll_bands_active() {
                crate::components::footer_chrome::current_main_menu_footer_height()
            } else {
                0.0
            },
            if crate::footer_popup::glass_scroll_bands_active() {
                crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
            } else {
                0.0
            },
            window.scale_factor(),
        );
        let main_content_size = gpui::size(
            px(detached_regions.main_content.width),
            px(detached_regions.main_content.height),
        );
        let main_content_width = detached_regions.main_content.width;
        let main_content_height = detached_regions.main_content.height;

        // Clone grid_config for use in the closure
        let grid_config = self.grid_config.clone();

        // Build component bounds for the current view (for debug overlay)
        // P0 FIX: Only compute bounds when grid overlay is actually enabled
        // Previously this was computed unconditionally on every frame
        let component_bounds = if grid_config.is_some() {
            self.build_component_bounds(main_content_size, &*cx)
        } else {
            Vec::new()
        };

        // Build warning banner if needed (bun not available)
        let warning_banner = if self.show_bun_warning {
            let banner_colors = WarningBannerColors::from_theme(&self.theme);
            let entity = cx.entity().downgrade();
            let entity_for_dismiss = entity.clone();

            Some(
                div().w_full().px(px(12.)).pt(px(8.)).child(
                    WarningBanner::new("bun is not installed. Install from bun.sh", banner_colors)
                        .on_click(Box::new(move |_event, _window, cx| {
                            if let Some(app) = entity.upgrade() {
                                app.update(cx, |this, _cx| {
                                    this.open_bun_website();
                                });
                            }
                        }))
                        .on_dismiss(Box::new(move |_event, _window, cx| {
                            if let Some(app) = entity_for_dismiss.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.dismiss_bun_warning(cx);
                                });
                            }
                        })),
                ),
            )
        } else {
            None
        };

        // Build alias input overlay if state is set
        let alias_input_overlay = self.render_alias_input_overlay(window, cx);

        // Build Tab AI save-offer overlay if a successful run is awaiting save decision
        let tab_ai_save_offer_overlay = self.render_tab_ai_save_offer_overlay(window, cx);

        // Log render timing for filter perf analysis - always log for "gr" filters
        let render_elapsed = render_start.elapsed();
        if filter_snapshot.starts_with("gr") {
            crate::logging::log(
                "FILTER_PERF",
                &format!(
                    "[FRAME_END] filter='{}' total={:.2}ms",
                    filter_snapshot,
                    render_elapsed.as_secs_f64() * 1000.0
                ),
            );
        }

        // Clear perf tracking after one complete render cycle
        if self
            .main_menu_render_diagnostics
            .filter_perf_start
            .is_some()
            && !self.filter_text.is_empty()
        {
            self.main_menu_render_diagnostics.filter_perf_start = None;
        }

        let theme_background_gradients =
            crate::ui_foundation::theme_background_gradient_layers("bg-layer", &self.theme);

        // Procedural background effect: hydrate the clock/ticker lazily so a
        // preference loaded at startup animates without a dedicated init path.
        if self.background_effect.is_some() {
            if self.background_effect_started_at.is_none() {
                self.background_effect_started_at = Some(std::time::Instant::now());
            }
            if self._background_effect_ticker.is_none() {
                self.start_background_effect_ticker(cx);
            }
        }
        let background_effect_layer = self.background_effect.map(|effect| {
            let elapsed = self
                .background_effect_started_at
                .map(|started| started.elapsed().as_secs_f32())
                .unwrap_or(0.0);
            // The main filter caret paints inside gpui-component's input
            // element, so the shared TextInput caret probe never sees it;
            // report it here from the input's last painted caret bounds
            // (dedupe in the recorder makes stationary re-reports free).
            if let Some(caret) = self.gpui_input_state.read(cx).last_cursor_bounds() {
                let (vw, vh) = (main_content_width, main_content_height);
                if vw > 0.0 && vh > 0.0 {
                    let center = caret.center();
                    crate::effects::note_effect_focus(
                        crate::effects::EffectFocusSource::TextCursor,
                        f32::from(center.x) / vw,
                        f32::from(center.y) / vh,
                    );
                }
            }
            crate::effects::background_effect_layer(
                &self.theme,
                effect,
                self.background_effect_intensity,
                elapsed,
            )
        });

        // Braille constellation loading layer: four large, slow-cycling
        // braille cells behind the list while a slow source fills the main
        // list (explicit tabs:/history: fetch, visible root file search).
        // Clock + ticker are owned by `app_impl/main_list_loading.rs`.
        let main_list_loading_layer = self.main_list_loading_kind().map(|_kind| {
            crate::components::braille_loading::constellation_loading_layer(
                &self.theme,
                self.main_list_loading_elapsed_secs(),
            )
        });

        // Capture mouse_cursor_hidden for use in div builder
        let mouse_cursor_hidden = self.mouse_cursor_hidden;

        // Get themed border color with 25% opacity (0x40 = 64/255)
        let border_color = rgba((self.theme.colors.ui.border << 8) | 0x40);

        let actions_window_open = actions::is_actions_window_open();
        let actions_background_shield = if actions_window_open {
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_any_mouse_down(cx.listener(|this, _, window, cx| {
                        logging::log(
                            "FOCUS",
                            "Main window shield clicked - closing actions popup via canonical close path",
                        );
                        this.close_actions_popup_for_current_view(window, cx);
                        cx.stop_propagation();
                    }))
                    .on_mouse_move(cx.listener(|_this, _: &MouseMoveEvent, _window, cx| {
                        cx.stop_propagation();
                    }))
                    .on_scroll_wheel(cx.listener(|_this, _event: &gpui::ScrollWheelEvent, _window, cx| {
                        cx.stop_propagation();
                    })),
            )
        } else {
            None
        };

        // Outer container: holds both the clipped main content and the dialog
        // layer which must NOT be clipped (same pattern as Notes window).
        let main_content_container: AnyElement = if view_owns_main_window_shell {
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.))
                .child(main_content)
                .into_any_element()
        } else {
            let menu_def = self.current_main_menu_theme.def();
            div()
                .id(crate::components::main_view_chrome::MAIN_VIEW_SHELL_ID)
                .debug_selector(|| {
                    crate::components::main_view_chrome::MAIN_VIEW_SHELL_ID.to_string()
                })
                .flex_1()
                .w_full()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .child(self.render_clickable_main_view_context_header(menu_def, cx))
                .child(
                    div()
                        .id(crate::components::main_view_chrome::MAIN_VIEW_MAIN_ID)
                        .debug_selector(|| {
                            crate::components::main_view_chrome::MAIN_VIEW_MAIN_ID.to_string()
                        })
                        .flex_1()
                        .w_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(main_content),
                )
                .into_any_element()
        };

        // The physical window is the complete main+gap+footer group. GPUI
        // paints only the bounded main-content stage; the gutter and native
        // footer region remain transparent above the same Metal host.
        div()
            .id("main-window-root")
            .size_full()
            .relative()
            // Route keys to confirm popup when it's open (Escape/Enter/Tab).
            // This must be at the outermost level to intercept before any
            // view-specific handlers.
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                // The internal brain-memory preview is a sessionless DivPrompt:
                // Enter/Escape close it back to the launcher list here instead
                // of routing through the protocol prompt machinery.
                if let AppView::DivPrompt { id, .. } = &this.current_view {
                    if id == BRAIN_MEMORY_PREVIEW_PROMPT_ID
                        && (crate::ui_foundation::is_key_escape(key)
                            || key.eq_ignore_ascii_case("enter"))
                    {
                        logging::log("BRAIN", "Brain memory preview closed via key");
                        this.reset_to_script_list(cx);
                        cx.stop_propagation();
                        return;
                    }
                }

                if matches!(this.current_view, AppView::FileSearchView { .. })
                    && !actions::is_actions_window_open()
                {
                    if crate::ui_foundation::is_key_escape(key) {
                        logging::log(
                            "KEY",
                            "Escape - closing FileSearchView from root capture",
                        );
                        if this.is_in_attachment_portal() {
                            this.close_attachment_portal_cancel(cx);
                        } else if !this.clear_builtin_view_filter(cx) {
                            this.go_back_or_close(window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }

                    if event.keystroke.modifiers.platform && key.eq_ignore_ascii_case("w") {
                        logging::log("KEY", "Cmd+W - closing FileSearchView from root capture");
                        this.close_and_reset_window(cx);
                        cx.stop_propagation();
                    }
                }
            }))
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                if matches!(this.current_view, AppView::FileSearchView { .. }) {
                    let needs_app_reactivate =
                        take_file_search_native_drag_awaiting_app_reactivate();
                    let stopped_drag = cx.stop_active_drag(window);
                    if needs_app_reactivate || stopped_drag {
                        if needs_app_reactivate {
                            platform::activate_main_window();
                        }
                        window.activate_window();
                        let input_state = this.gpui_input_state.clone();
                        this.focus_main_filter(window, cx);
                        logging::log(
                            "FOCUS",
                            &format!(
                                "FileSearch root mouse capture: stopped_drag={} restored_main_filter_focus=true activated_window=true needs_app_reactivate={} main_window_focused={}",
                                stopped_drag,
                                needs_app_reactivate,
                                platform::is_main_window_focused()
                            ),
                        );
                        if needs_app_reactivate {
                            window.defer(cx, move |window, cx| {
                                platform::activate_main_window();
                                window.activate_window();
                                input_state.update(cx, |state, cx| {
                                    state.focus(window, cx);
                                });
                                logging::log(
                                    "FOCUS",
                                    &format!(
                                        "FileSearch deferred root refocus: rekeyed_panel=true activated_app=true main_window_focused={}",
                                        platform::is_main_window_focused()
                                    ),
                                );
                            });
                        }
                        cx.stop_propagation();
                    }
                }
            }))
            .on_drag_move::<file_search::FileDragPayload>(
                cx.listener(|this, _event, window, cx| {
                    if matches!(this.current_view, AppView::FileSearchView { .. }) {
                        let stopped_drag = cx.stop_active_drag(window);
                        if stopped_drag {
                            logging::log(
                                "FOCUS",
                                &format!(
                                    "FileSearch root drag move cleanup: stopped_drag=true main_window_focused={}",
                                    platform::is_main_window_focused()
                                ),
                            );
                            cx.stop_propagation();
                        }
                    }
                }),
            )
            // Close popups when the user clicks anywhere on the main window.
            // Uses on_any_mouse_down to handle left, right, and middle clicks.
            .on_any_mouse_down(cx.listener(|this, _, window, cx| {
                if matches!(this.current_view, AppView::FileSearchView { .. }) {
                    window.activate_window();
                    this.focus_main_filter(window, cx);
                    logging::log(
                        "FOCUS",
                        &format!(
                            "FileSearch root mouse down: restored_main_filter_focus=true activated_window=true main_window_focused={} actions_open={} confirm_open={}",
                            platform::is_main_window_focused(),
                            actions::is_actions_window_open(),
                            confirm::is_confirm_window_open()
                        ),
                    );
                }
                if confirm::is_confirm_window_open() {
                    logging::log("FOCUS", "Main window clicked - closing confirm popup");
                    confirm::route_key_to_confirm_popup("escape", cx);
                }
                if actions::is_actions_window_open() {
                    logging::log(
                        "FOCUS",
                        "Main window clicked - closing actions popup via canonical close path",
                    );
                    this.close_actions_popup_for_current_view(window, cx);
                }
                if this.shortcut_recorder_state.is_some() {
                    logging::log("SHORTCUT", "Main window clicked - closing shortcut recorder");
                    this.close_shortcut_recorder(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .id("main-content-stage")
                    .debug_selector(|| "main-content-stage".to_string())
                    .w_full()
                    .h(px(main_content_height))
                    .relative()
                    .flex()
                    .flex_col()
                    // The main window's gpui-component Root is intentionally
                    // transparent because the native footer lives below this
                    // stage. Paint the same theme veil here so every opacity
                    // setting stops at the content boundary and the detached
                    // gutter remains physically empty.
                    .bg(cx.theme().background)
                    // Hide mouse cursor while typing
                    .when(mouse_cursor_hidden, |d| d.cursor(CursorStyle::None))
                    // Hide cursor and clear hover on any keyboard interaction
                    .capture_key_down(cx.listener(|this, _: &KeyDownEvent, _window, cx| {
                        this.input_mode = InputMode::Keyboard;
                        this.hovered_index = None;
                        this.hide_mouse_cursor(cx);
                    }))
                    // Show cursor when mouse moves; a moving mouse also
                    // steers the background effect focus (no notify — the
                    // effect ticker repaints while an effect is active).
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                        this.show_mouse_cursor(cx);
                        if this.background_effect.is_some() {
                            let (vw, vh) = (main_content_width, main_content_height);
                            if vw > 0.0 && vh > 0.0 {
                                crate::effects::note_effect_focus(
                                    crate::effects::EffectFocusSource::Mouse,
                                    f32::from(event.position.x) / vw,
                                    f32::from(event.position.y) / vh,
                                );
                            }
                        }
                    }))
                    // A user-authored gradient is explicit theme content, so it
                    // renders even when vibrancy is enabled.
                    .children(theme_background_gradients)
                    // Procedural shader effect layer, behind content, clipped
                    // by the rounded window container.
                    .children(background_effect_layer)
                    // Transient braille constellation while the main list is
                    // slow-filling, above the ambient effect and behind all
                    // content.
                    .children(main_list_loading_layer)
                    // Visual styling - rounded corners, subtle border, clip content
                    .rounded(px(crate::ui::chrome::MAIN_WINDOW_CONTENT_RADIUS_PX))
                    .border_1()
                    .border_color(border_color)
                    .overflow_hidden()
                    // Warning banner appears at the top when bun is not available
                    .when_some(warning_banner, |container, banner| container.child(banner))
                    // Main content takes remaining space
                    .child(main_content_container)
                    // Alias input overlay (on top of main content when entering alias)
                    .when_some(alias_input_overlay, |container, overlay| {
                        container.child(overlay)
                    })
                    // Tab AI save-offer overlay (on top after successful Tab AI execution)
                    .when_some(tab_ai_save_offer_overlay, |container, overlay| {
                        container.child(overlay)
                    })
                    .when_some(main_window_modal_dim_layer, |container, overlay| {
                        container.child(overlay)
                    })
                    .when_some(actions_background_shield, |container, overlay| {
                        container.child(overlay)
                    })
                    .when_some(grid_config, |container, config| {
                        let overlay_bounds = gpui::Bounds {
                            origin: gpui::point(px(0.), px(0.)),
                            size: main_content_size,
                        };
                        container.child(debug_grid::render_grid_overlay(
                            &config,
                            overlay_bounds,
                            &component_bounds,
                        ))
                    })
                    // Root dialogs and their modal hit shields belong to the
                    // same bounded stage as every other GPUI overlay. Keeping
                    // them below this stage's height + overflow clip prevents
                    // paint or input from entering the transparent gutter or
                    // the native footer strip.
                    .child(
                        div()
                            .id("main-window-dialog-layer-boundary")
                            .debug_selector(|| "main-window-dialog-layer-boundary".to_string())
                            .absolute()
                            .inset_0()
                            .overflow_hidden()
                            .children(gpui_component::Root::render_dialog_layer(window, cx)),
                    )
                    .child(
                        div()
                            .id("main-window-notification-layer-boundary")
                            .debug_selector(|| {
                                "main-window-notification-layer-boundary".to_string()
                            })
                            .absolute()
                            .inset_0()
                            .overflow_hidden()
                            .children(gpui_component::Root::render_notification_layer(window, cx)),
                    ),
            )
    }
}

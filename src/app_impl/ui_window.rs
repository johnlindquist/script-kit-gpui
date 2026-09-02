use super::*;

/// Thin wrapper delegating to the canonical implementation in `window_resize`.
fn main_window_sizing_from_grouped_items(
    grouped_items: &[GroupedListItem],
) -> crate::window_resize::MainWindowSizing {
    crate::window_resize::main_window_sizing_from_grouped_items(grouped_items)
}

impl ScriptListApp {
    pub(crate) fn main_window_primary_action_label(&self) -> String {
        let frontmost_app_name = footer_frontmost_app_name();

        match &self.current_view {
            AppView::ClipboardHistoryView { .. } => {
                return if has_selected_clipboard_entry(self) {
                    paste_into_frontmost_app_label(frontmost_app_name.as_deref())
                } else {
                    "Run".to_string()
                };
            }
            AppView::EmojiPickerView { .. } => {
                return if has_selected_emoji_entry(self) {
                    paste_into_frontmost_app_label(frontmost_app_name.as_deref())
                } else {
                    "Run".to_string()
                };
            }
            AppView::DictationHistoryView { .. } => {
                return if !has_selected_dictation_history_entry(self) {
                    "Run".to_string()
                } else if self.is_in_attachment_portal() {
                    "Attach Transcript".to_string()
                } else {
                    paste_into_frontmost_app_label(frontmost_app_name.as_deref())
                };
            }
            AppView::ThemeChooserView { .. } => {
                return "Apply".to_string();
            }
            AppView::ProfileSearchView { .. } => {
                return "Switch Profile".to_string();
            }
            AppView::ScriptList => {}
            _ => return "Run".to_string(),
        }

        // Unarmed empty colon mode: no row is selected, so the footer must
        // not advertise the internal selection's verb ("Attach ↵") while
        // Enter is consumed without attaching. Mirror the ghost-text
        // affordance instead.
        if self.spine_empty_subsearch_selection_suppressed() {
            return "Type to Search".to_string();
        }

        if let Some(label) = self.menu_syntax_filter_accept_primary_label() {
            return label.to_string();
        }

        match self.resolved_main_menu_selected_subject() {
            Some(ResolvedMainMenuSelection::SearchResult { result, .. }) => {
                main_window_result_action_label(result, frontmost_app_name.as_deref())
            }
            Some(ResolvedMainMenuSelection::Calculator { .. }) => "Copy".to_string(),
            None => "Run".to_string(),
        }
    }

    pub(crate) fn dispatch_main_window_footer_action(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
        source: &'static str,
    ) {
        if matches!(action, crate::footer_popup::FooterAction::Run)
            && matches!(self.current_view, AppView::ScriptList)
        {
            self.set_main_menu_dispatch_observation(None);
            self.flush_pending_main_menu_query(cx);
        }
        let Some(mut config) = self.main_window_footer_config_with_cx(Some(&*cx)) else {
            tracing::info!(target: "script_kit::footer_popup", source, action = ?action, reason = "no_current_footer", "Ignored footer action without a live host surface");
            return;
        };
        self.enrich_footer_config_with_agent_chat_info(&mut config);

        let live_header_affordance = source == "main_view_context_click"
            && match action {
                crate::footer_popup::FooterAction::Cwd => self.main_view_context_chip_has_action(
                    crate::components::main_view_chrome::MAIN_VIEW_CONTEXT_CWD_BUTTON_ID,
                    crate::components::main_view_chrome::SemanticChipAction::OpenSelector,
                ),
                crate::footer_popup::FooterAction::AgentModel => self
                    .main_view_context_chip_has_action(
                        crate::components::main_view_chrome::MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID,
                        crate::components::main_view_chrome::SemanticChipAction::OpenSelector,
                    ),
                _ => false,
            };

        match config.action_dispatch_authorization(action, live_header_affordance) {
            crate::footer_popup::FooterActionDispatchAuthorization::PresentedButton
            | crate::footer_popup::FooterActionDispatchAuthorization::PresentedLeftAffordance
            | crate::footer_popup::FooterActionDispatchAuthorization::PresentedHeaderAffordance => {
            }
            crate::footer_popup::FooterActionDispatchAuthorization::Disabled { reason } => {
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_action_blocked",
                    source,
                    action = ?action,
                    disabled_reason = ?reason,
                    "Ignored disabled main-window footer action"
                );
                return;
            }
            crate::footer_popup::FooterActionDispatchAuthorization::NotPresented => {
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_action_blocked",
                    source,
                    action = ?action,
                    surface = config.surface,
                    reason = "action_not_presented",
                    "Ignored stale or invisible main-window footer action"
                );
                return;
            }
        }

        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_action_dispatch",
            source,
            action = ?action,
            view = ?self.current_view,
            main_window_mode = ?self.main_window_mode,
            "Dispatching main-window footer action"
        );

        // Standard macOS menu dismissal: clicking a real footer button while
        // the actions popup is open closes the popup AND performs the clicked
        // action in the same event. Swallowing the click (close-only) made
        // visible, enabled-looking footer buttons dead until a second click.
        let shared_actions_open = self.show_actions_popup;
        let detached_actions_open = crate::actions::is_actions_window_open();
        if (shared_actions_open || detached_actions_open) && !action.is_actions() {
            let mut closed = false;
            if let super::actions_dialog::ActionsSupport::SharedDialog(host) =
                self.actions_support_for_view()
            {
                self.close_actions_popup(host, window, cx);
                closed = true;
            }
            if detached_actions_open {
                crate::actions::close_actions_window(cx);
                closed = true;
            }
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_action_closed_actions_then_dispatched",
                source,
                action = ?action,
                main_window_mode = ?self.main_window_mode,
                closed,
                "Closed actions dialog from footer click, dispatching the clicked action"
            );
        }

        match action {
            crate::footer_popup::FooterAction::Tips => {
                let builtins = self.config.get_builtins();
                if let Some(entry) =
                    crate::builtins::resolve_builtin_entry("builtin/tips", &builtins)
                {
                    let _outcome = self.execute_builtin(&entry, cx);
                }
            }
            crate::footer_popup::FooterAction::Run => {
                if let AppView::SettingsView {
                    filter,
                    selected_index,
                } = &self.current_view
                {
                    let items = self.get_settings_items();
                    if let Some(descriptor) = selected_settings_action_descriptor(
                        &items,
                        filter,
                        *selected_index,
                        self.settings_action_availability(),
                    ) {
                        self.submit_settings_action(
                            descriptor.action_id,
                            SettingsActivationSource::NativeFooter,
                            window,
                            cx,
                        );
                    }
                    return;
                }
                if matches!(self.current_view, AppView::ScriptList) {
                    if self.should_consume_menu_syntax_trigger_picker_press_enter(source)
                        || self.should_consume_script_list_enter_after_submit(source)
                        || self.try_handle_spine_enter(window, cx)
                    {
                        return;
                    }
                    let _dispatch = self.execute_selected(cx);
                } else if matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
                    self.dispatch_permissions_wizard_action(
                        crate::permissions_wizard::PermissionsWizardAction::GrantSelected,
                        window,
                        cx,
                    );
                } else if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.submit_with_expanded_tokens(cx);
                    });
                } else if let AppView::TermPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |term, _cx| {
                        if let Err(error) = term.send_raw_input("\r") {
                            tracing::warn!(
                                target: "script_kit::footer_popup",
                                event = "term_prompt_footer_continue_failed",
                                %error,
                                "Failed to send Enter from terminal footer"
                            );
                        }
                    });
                } else if let AppView::MicroPrompt { id, .. } = &self.current_view {
                    let prompt_id = id.clone();
                    self.submit_arg_prompt_from_current_state(&prompt_id, cx);
                } else if matches!(self.current_view, AppView::SdkReferenceView { .. }) {
                    let _ = self.copy_selected_sdk_reference_markdown(cx);
                } else if matches!(self.current_view, AppView::ScriptTemplateCatalogView { .. }) {
                    if let Some(template) = self.selected_script_template() {
                        self.show_naming_dialog_for_script_template(template, window, cx);
                    }
                } else if matches!(self.current_view, AppView::CreateAiPresetView { .. }) {
                    self.handle_create_ai_preset_key("enter", window, cx);
                } else if matches!(self.current_view, AppView::NotesBrowseView { .. }) {
                    let _ = self.activate_selected_note_search_result_from_footer(cx);
                } else if let AppView::FlowSessionView { session_id } = self.current_view {
                    // One shared draft transaction with the keyboard Enter
                    // path: the draft clears only when the submit consumed it,
                    // so a Busy race can never destroy it (Oracle 2026-07-21).
                    let _ = self.submit_flow_session_draft(session_id, window, cx);
                } else if matches!(self.current_view, AppView::FlowUxView { .. }) {
                    self.flow_desk_activate_selected(false, window, cx);
                } else if let AppView::ScriptIssuesView { report } = &self.current_view {
                    let report = report.clone();
                    self.fix_script_issues_in_agent(&report, cx);
                } else if self.dispatch_day_page_preview_footer_action(action, window, cx)
                    || self.dispatch_kit_store_primary_footer_action(cx)
                    || self.dispatch_migrate_v1_primary_footer_action(cx)
                {
                } else if matches!(self.current_view, AppView::ThemeChooserView { .. }) {
                    self.submit_theme_chooser_from_input_enter(window, cx);
                } else if matches!(self.current_view, AppView::TipsView { .. }) {
                    self.tips_copy_selected_example(cx);
                } else if let AppView::TemplatePrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.submit(cx));
                } else if let AppView::EditorPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |editor, cx| editor.submit(cx));
                } else if matches!(self.current_view, AppView::WebcamView { .. }) {
                    if self.capture_webcam_photo(cx) {
                        self.hide_main_and_reset(cx);
                    }
                } else if let AppView::PathPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.handle_enter(cx));
                } else if let AppView::EnvPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.submit(cx));
                } else if let AppView::DropPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, _cx| prompt.submit());
                } else if let AppView::FormPrompt { id, entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    let prompt_id = id.clone();
                    let validation_message = entity.read(cx).submit_validation_message(cx);
                    if let Some(message) = validation_message {
                        self.show_hud(message, Some(HUD_LONG_MS), cx);
                        return;
                    }
                    let values = entity.read(cx).collect_values(cx);
                    self.submit_prompt_response(prompt_id, Some(values), cx);
                } else if !self.try_run_ready_agent_chat_script(cx) {
                    let _dispatch = self.execute_selected(cx);
                }
            }
            crate::footer_popup::FooterAction::Actions => {
                let handled = self.dispatch_actions_toggle_for_current_view(window, cx, source);
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_actions_routed",
                    source,
                    handled,
                    selected_index = self.selected_index,
                    show_actions_popup = self.show_actions_popup,
                    actions_window_open = crate::actions::is_actions_window_open(),
                    "Routed footer Actions through shared dispatcher"
                );
            }
            crate::footer_popup::FooterAction::Ai => {
                if matches!(self.current_view, AppView::CreateAiPresetView { .. }) {
                    self.handle_create_ai_preset_key("tab", window, cx);
                } else if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.open_profile_trigger_picker_in_window(window, cx);
                    });
                } else if self.dispatch_day_page_preview_footer_action(action, window, cx) {
                } else if self.day_page_context_return.is_some() {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_ai_ignored_day_page_context_return",
                        "Ignored stale Agent footer action while Day Page @ context round trip is active"
                    );
                } else if matches!(self.current_view, AppView::DayPage { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_ai_ignored_day_page",
                        "Ignored stale Day Page Agent footer action"
                    );
                } else if let AppView::QuickTerminalView { entity } = &self.current_view {
                    let entity = entity.clone();
                    self.open_agent_chat_with_quick_terminal_output(entity, cx);
                } else if let AppView::TemplatePrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.next_input(cx));
                } else if let AppView::FormPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |form, cx| form.focus_next(window, cx));
                } else {
                    self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                }
            }
            crate::footer_popup::FooterAction::Stop => {
                if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        let _ = chat.stop_streaming_explicitly(cx);
                    });
                } else if let AppView::FlowSessionView { session_id } = self.current_view {
                    // The flow session footer shows `⌘. Stop` while a turn is in
                    // flight, so the button must reach the same cancellation the
                    // key press does. Without this arm the button rendered and
                    // did nothing but log.
                    self.stop_flow_session(session_id, cx);
                } else {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_stop_ignored",
                        view = ?self.current_view,
                        "Ignored Stop footer action outside Agent Chat chat"
                    );
                }
            }
            crate::footer_popup::FooterAction::PasteResponse => {
                self.paste_latest_agent_chat_response_to_frontmost(None, cx);
            }
            crate::footer_popup::FooterAction::Replace
            | crate::footer_popup::FooterAction::Append
            | crate::footer_popup::FooterAction::Copy
            | crate::footer_popup::FooterAction::Expand
            | crate::footer_popup::FooterAction::Retry => {
                if action == crate::footer_popup::FooterAction::Copy
                    && matches!(self.current_view, AppView::SdkReferenceView { .. })
                {
                    let _ = self.copy_selected_sdk_reference_markdown(cx);
                    return;
                }
                if action == crate::footer_popup::FooterAction::Copy
                    && matches!(self.current_view, AppView::ScriptTemplateCatalogView { .. })
                {
                    let _ = self.copy_selected_script_template_markdown(cx);
                    return;
                }
                if self.dispatch_day_page_preview_footer_action(action, window, cx) {
                    return;
                }
                if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.dispatch_footer_button(action, window, cx);
                    });
                }
            }
            crate::footer_popup::FooterAction::Apply => {
                if matches!(self.current_view, AppView::FlowUxView { .. }) {
                    self.flow_desk_activate_selected(true, window, cx);
                } else if let AppView::ScriptIssuesView { report } = &self.current_view {
                    let report = report.clone();
                    self.copy_script_issues_to_clipboard(&report, cx);
                } else if self.dispatch_kit_store_remove_footer_action(cx) {
                } else if matches!(self.current_view, AppView::ConfirmPrompt { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "confirm_prompt_footer_apply",
                        "Confirming in-window confirm prompt from native footer"
                    );
                    self.resolve_confirm_prompt(true, window, cx);
                } else if let AppView::QuickTerminalView { entity } = &self.current_view {
                    let entity = entity.clone();
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "quick_terminal_footer_apply",
                        "Applying quick-terminal result from native footer"
                    );
                    self.apply_tab_ai_result_from_terminal(entity, cx);
                } else {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_apply_ignored",
                        view = ?self.current_view,
                        "Ignored Apply footer action outside QuickTerminalView"
                    );
                }
            }
            crate::footer_popup::FooterAction::Close => {
                if matches!(self.current_view, AppView::About { .. }) {
                    self.dismiss_about(cx);
                } else if matches!(self.current_view, AppView::MicroPrompt { .. }) {
                    self.go_back_or_close(window, cx);
                } else if matches!(
                    self.current_view,
                    AppView::SdkReferenceView { .. } | AppView::ScriptTemplateCatalogView { .. }
                ) {
                    if !self.clear_builtin_view_filter(cx) {
                        self.go_back_or_close(window, cx);
                    }
                } else if matches!(self.current_view, AppView::CreateAiPresetView { .. }) {
                    self.go_back_or_close(window, cx);
                } else if matches!(self.current_view, AppView::NotesBrowseView { .. }) {
                    if self.is_in_attachment_portal() {
                        self.close_attachment_portal_cancel(cx);
                    } else if !self.clear_builtin_view_filter(cx) {
                        self.go_back_or_close(window, cx);
                    }
                } else if matches!(self.current_view, AppView::FlowSessionView { .. }) {
                    // Native `Esc Background` matches keyboard Esc: BACKGROUND the
                    // session (keep it alive in the Desk). Terminate is the
                    // destructive expert command in ⌘K Actions with the ⇧⌘⎋
                    // shortcut (Oracle 2026-07-21 footer adjudication — the
                    // native Close used to terminate, destroying conversations
                    // from a non-destructive-looking control).
                    self.background_flow_session(window, cx);
                } else if matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
                    self.dispatch_permissions_wizard_action(
                        crate::permissions_wizard::PermissionsWizardAction::Done,
                        window,
                        cx,
                    );
                } else if self.dispatch_kit_store_browse_back_footer_action(window, cx)
                    || self.dispatch_day_page_preview_footer_action(action, window, cx)
                {
                } else if matches!(self.current_view, AppView::ConfirmPrompt { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "confirm_prompt_footer_close",
                        "Cancelling in-window confirm prompt from native footer"
                    );
                    self.resolve_confirm_prompt(false, window, cx);
                } else if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "quick_terminal_footer_close",
                        "Closing quick terminal from native footer"
                    );
                    self.close_quick_terminal_main_window_state_first(cx);
                } else if let AppView::TermPrompt { id, .. } = &self.current_view {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "term_prompt_footer_cancel",
                        "Cancelling terminal prompt from native footer"
                    );
                    self.submit_prompt_response(id.clone(), None, cx);
                } else if let AppView::HotkeyPrompt { id, .. } = &self.current_view {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "hotkey_prompt_footer_cancel",
                        "Cancelling hotkey prompt from native footer"
                    );
                    self.submit_prompt_response(id.clone(), None, cx);
                    self.cancel_script_execution(cx);
                } else if let AppView::EditorPrompt { entity, .. } = &self.current_view {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "editor_prompt_footer_cancel",
                        "Cancelling editor prompt from native footer (script receives None)"
                    );
                    let entity = entity.clone();
                    entity.update(cx, |editor, _| editor.submit_cancel());
                } else if matches!(self.current_view, AppView::TipsView { .. }) {
                    // Mirror the in-view Escape ladder: clear the filter
                    // first, then go back to the launcher.
                    if !self.clear_builtin_view_filter(cx) {
                        self.go_back_or_close(window, cx);
                    }
                } else {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_close_ignored",
                        view = ?self.current_view,
                        "Ignored Close footer action outside QuickTerminalView"
                    );
                }
            }
            crate::footer_popup::FooterAction::Cwd => {
                // Click on the CWD chip → open the directory picker the
                // same way Tab does (see startup.rs tab_interceptor
                // ScriptList arm). Works from ScriptList; from other views
                // we first return to the launcher.
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_cwd_chip_clicked",
                    view = ?self.current_view,
                    "Opening CWD picker from footer chip"
                );
                if !matches!(self.current_view, AppView::ScriptList) {
                    self.current_view = AppView::ScriptList;
                    self.note_main_route_changed();
                    self.reset_main_menu_selection_intent();
                }
                self.cwd_pick_mode = true;
                self.open_file_search_view("~/".to_string(), FileSearchPresentation::Full, cx);
                self.suppress_filter_events = true;
                self.gpui_input_state.update(cx, |state, cx| {
                    state.set_value("~/".to_string(), window, cx);
                    let len = "~/".len();
                    state.set_selection(len, len, window, cx);
                });
                self.suppress_filter_events = false;
                cx.notify();
            }
            crate::footer_popup::FooterAction::AgentModel => {
                // Click on the profile/model chip. From Agent Chat, keep the
                // chat surface alive and use the same in-chat Profile picker
                // path as Shift+Tab. From ScriptList, use the global
                // Profile Switcher.
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_agent_model_chip_clicked",
                    view = ?self.current_view,
                    "Opening Profile Switcher from footer chip"
                );
                if let AppView::AgentChatView { entity, .. } = &self.current_view {
                    if self.show_actions_popup || crate::actions::is_actions_window_open() {
                        tracing::info!(
                            target: "script_kit::footer_popup",
                            event = "main_window_footer_agent_model_chip_ignored_actions_open",
                            view = ?self.current_view,
                            "Ignored profile/model chip while actions dialog owns input"
                        );
                        return;
                    }
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.open_profile_trigger_picker_in_window(window, cx);
                    });
                    return;
                }
                if self.day_page_context_return.is_some() {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_agent_model_ignored_day_page_context_return",
                        "Ignored stale Agent/Model footer action while Day Page @ context round trip is active"
                    );
                    return;
                }
                if matches!(self.current_view, AppView::DayPage { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_agent_model_ignored_day_page",
                        "Ignored stale Day Page Agent/Model footer action"
                    );
                    return;
                }
                if !matches!(self.current_view, AppView::ScriptList) {
                    self.current_view = AppView::ScriptList;
                    self.note_main_route_changed();
                    self.reset_main_menu_selection_intent();
                }
                self.open_profile_search(cx);
            }
        }
    }

    /// One receiver task per owning main root and GPUI window lifetime.
    fn ensure_main_footer_action_listener(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.main_footer_action_task.is_some() {
            return;
        }
        let rx = crate::footer_popup::footer_action_receiver(window);
        let lifetime = crate::footer_popup::footer_owner_subscription(window, cx);
        self.main_footer_action_task = Some(cx.spawn_in(window, async move |this, cx| {
            let _lifetime = lifetime;
            while let Ok(event) = rx.recv().await {
                if this
                    .update_in(cx, |app, window, cx| {
                        if let Some(action) = event.accept(window) {
                            app.handle_main_footer_action(action, window, cx);
                            event.complete(window);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn standard_main_window_footer_buttons(&self) -> Vec<crate::footer_popup::FooterButtonConfig> {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        let footer_disabled = self.main_window_footer_buttons_blocked();
        let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
        let run_label = self.main_window_primary_action_label();

        let mut buttons = Vec::new();

        buttons.push(main_window_run_footer_button(
            run_label,
            footer_disabled,
            self.main_window_selected_command_block_reason(),
        ));

        if self.current_view_supports_shared_actions() {
            let chip = FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                .selected(actions_open);
            // Selection-gated views (clipboard/dictation/favorites) only have
            // per-entry actions, so the advertised ⌘K would be a dead key
            // without a selected row — grey it out with the reason instead of
            // lying (audit finding #29). Stay enabled while the popup is open
            // so the chip can still close it.
            let chip = match self.actions_toggle_dead_without_selection_reason() {
                Some(reason) if !actions_open && !footer_disabled => chip.disabled_reason(reason),
                _ => chip.enabled(!footer_disabled),
            };
            buttons.push(chip);
        }
        if matches!(self.current_view, AppView::ScriptList) {
            // Style-only input (`.professional`): Enter already rewrites
            // via Agent Chat, so the Agent ⌘↵ button is dropped here.
            let style_owns_submit = self.spine_enabled
                && crate::spine::prompt_plan::spine_parse_is_style_only(&self.spine_parse);
            if !style_owns_submit {
                buttons.push(
                    FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent")
                        .enabled(!footer_disabled),
                );
            }
        }
        crate::footer_popup::apply_footer_descriptor_test_fixture(&mut buttons);
        buttons
    }

    fn main_window_footer_buttons_blocked(&self) -> bool {
        crate::confirm::is_confirm_window_open()
    }

    fn main_window_selected_command_block_reason(&self) -> Option<&'static str> {
        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }

        match self.resolved_main_menu_selected_subject() {
            Some(ResolvedMainMenuSelection::SearchResult { result, .. }) => {
                crate::main_window_preflight::command_block_reason(self, result)
            }
            Some(ResolvedMainMenuSelection::Calculator { .. }) => None,
            None => Some("No current selected result."),
        }
    }

    /// Views whose actions are all per-entry have a dead ⌘K toggle when no
    /// row is selected. Returns the user-facing reason in that state so both
    /// the footer chip (disabled) and the key press (HUD) explain themselves.
    pub(crate) fn actions_toggle_dead_without_selection_reason(&self) -> Option<&'static str> {
        match &self.current_view {
            AppView::ClipboardHistoryView { .. } if !has_selected_clipboard_entry(self) => {
                Some("Select an entry to see actions")
            }
            AppView::DictationHistoryView { .. } if !has_selected_dictation_history_entry(self) => {
                Some("Select an entry to see actions")
            }
            AppView::FavoritesBrowseView { .. } if self.selected_favorite_id().is_none() => {
                Some("Select a favorite to see actions")
            }
            _ => None,
        }
    }

    fn main_window_footer_surface(&self) -> Option<&'static str> {
        self.current_view.native_footer_surface()
    }

    /// Quick Terminal footer buttons. Scoped to actions actually meaningful in
    /// the Quick Terminal surface: always Close (⌘W), plus Apply (⌘↩) only
    /// when a tab-AI apply-back route AND its return view are both present.
    /// Run/AI/Actions are intentionally omitted — Quick Terminal shares the
    /// main menu's native footer chrome but not its main-menu-specific actions.
    pub(crate) fn quick_terminal_can_apply_back(&self) -> bool {
        self.tab_ai_harness_apply_back_route.is_some() && self.tab_ai_harness_return_view.is_some()
    }

    pub(crate) fn quick_terminal_can_attach_to_agent_chat(&self) -> bool {
        matches!(self.current_view, AppView::QuickTerminalView { .. })
            && !self.quick_terminal_can_apply_back()
    }

    fn main_window_footer_buttons_for_current_view(
        &self,
        cx: Option<&gpui::App>,
    ) -> Vec<crate::footer_popup::FooterButtonConfig> {
        if matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let enabled = !self.main_window_footer_buttons_blocked();
            return crate::permissions_wizard::PermissionsWizardActions::ALL
                .iter()
                .map(|spec| {
                    let footer_action = match spec.action {
                        crate::permissions_wizard::PermissionsWizardAction::GrantSelected => {
                            FooterAction::Run
                        }
                        crate::permissions_wizard::PermissionsWizardAction::Done => {
                            FooterAction::Close
                        }
                    };
                    FooterButtonConfig::new(footer_action, spec.key, spec.label).enabled(enabled)
                })
                .collect();
        }

        let enabled = !self.main_window_footer_buttons_blocked();
        if let AppView::SettingsView {
            filter,
            selected_index,
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};
            let items = self.get_settings_items();
            let descriptor = selected_settings_action_descriptor(
                &items,
                filter,
                *selected_index,
                self.settings_action_availability(),
            );
            let mut buttons = Vec::new();
            if let Some(descriptor) = descriptor {
                let mut button =
                    FooterButtonConfig::new(FooterAction::Run, "↵", descriptor.primary_verb)
                        .enabled(enabled && descriptor.enabled);
                if let Some(reason) = descriptor.disabled_reason {
                    button = button.disabled_reason(reason);
                }
                buttons.push(button);
            }
            buttons
                .push(FooterButtonConfig::new(FooterAction::Close, "Esc", "Back").enabled(enabled));
            return buttons;
        }

        if matches!(self.current_view, AppView::About { .. }) {
            return about_footer_buttons(enabled);
        }

        if matches!(self.current_view, AppView::MicroPrompt { .. }) {
            return micro_prompt_footer_buttons(enabled);
        }

        if let AppView::SdkReferenceView {
            filter,
            selected_index,
            entries,
        } = &self.current_view
        {
            let has_selection = crate::mcp_resources::sdk_reference_visible_rows(entries, filter)
                .get(*selected_index)
                .is_some();
            return sdk_reference_footer_buttons(enabled, has_selection);
        }

        if let AppView::ScriptTemplateCatalogView {
            filter,
            selected_index,
            templates,
        } = &self.current_view
        {
            let has_selection =
                crate::mcp_resources::script_template_catalog_visible_rows(templates, filter)
                    .get(*selected_index)
                    .is_some();
            return script_template_catalog_footer_buttons(enabled, has_selection);
        }

        if let AppView::CreateAiPresetView { name, .. } = &self.current_view {
            return create_ai_preset_footer_buttons(enabled, !name.trim().is_empty());
        }

        if let AppView::NotesBrowseView { search } = &self.current_view {
            let has_selection = Self::notes_browse_selected_visible_row(search).is_some();
            return notes_browse_footer_buttons(enabled, search.destination, has_selection);
        }

        // ConfirmPrompt: Apply (Confirm) + Close (Cancel) labeled per options.
        if let AppView::ConfirmPrompt {
            options,
            focused_button,
            ..
        } = &self.current_view
        {
            let buttons = self.confirm_prompt_footer_buttons(options, *focused_button);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved ConfirmPrompt footer buttons"
            );
            return buttons;
        }

        // Quick Terminal: scoped Close (+ optional Apply) — never Run/AI/Actions.
        if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
            let buttons = self.quick_terminal_footer_buttons();
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Quick Terminal footer buttons"
            );
            return buttons;
        }

        // Full terminal prompt: mirror the keyboard grammar previously shown
        // by its GPUI hint strip through the persistent native footer.
        if matches!(self.current_view, AppView::TermPrompt { .. }) {
            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let buttons = term_prompt_footer_buttons(!footer_disabled, actions_open);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved terminal prompt footer buttons"
            );
            return buttons;
        }

        if let AppView::SelectPrompt { entity, .. } = &self.current_view {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let has_safe_submission = cx.is_some_and(|cx| {
                let prompt = entity.read(cx);
                crate::prompts::select_submission_is_allowed(prompt.multiple, prompt.selected.len())
            });
            let run = if has_safe_submission {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Run")
                    .enabled(!self.main_window_footer_buttons_blocked())
            } else {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Select one")
                    .disabled_reason("no_selection")
            };
            return vec![run];
        }

        // Flow session (Threadline): the footer mirrors the chat grammar —
        // Send + Actions, with Send honestly disabled while a turn runs.
        if let AppView::FlowSessionView { session_id } = self.current_view {
            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let working = self
                .conversations
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == session_id)
                .is_some_and(|(meta, _)| meta.active_turn.is_some());
            let buttons = flow_session_footer_buttons(working, enabled, actions_open);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Flow Session footer buttons"
            );
            return buttons;
        }

        // Flow Desk: selected-row verbs come from the same descriptor used by
        // Enter, paint, Actions, and getElements.
        if let AppView::FlowUxView {
            filter,
            selected_index,
            ..
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let descriptor = self
                .flow_desk_rows(filter)
                .get(*selected_index)
                .map(|row| self.flow_desk_row_descriptor(row));
            let mut buttons = Vec::new();
            if let Some(descriptor) = descriptor {
                buttons.push(
                    FooterButtonConfig::new(FooterAction::Run, "↵", descriptor.primary.label())
                        .enabled(enabled),
                );
                if let Some(secondary) = descriptor.secondary {
                    buttons.push(
                        FooterButtonConfig::new(FooterAction::Apply, "⇧↵", secondary.label())
                            .enabled(enabled),
                    );
                }
                if descriptor.actions_available {
                    buttons.push(
                        FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                            .selected(actions_open)
                            .enabled(enabled),
                    );
                }
            }
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Flow Desk footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::DayPage { .. }) {
            let buttons = day_page_footer_buttons(self, cx);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Day Page footer buttons"
            );
            return buttons;
        }

        // Agent Chat owns its own footer state: Send/Paste Response/Stop + Actions.
        if matches!(self.current_view, AppView::AgentChatView { .. }) {
            let buttons = self.agent_chat_footer_buttons();
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Agent Chat footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::TemplatePrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Ai, "⇥", "Next Field").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved TemplatePrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::FormPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "⌘↵", "Submit").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Ai, "⇥", "Next Field").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved FormPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::HotkeyPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel").enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved HotkeyPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::ScriptIssuesView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Fix in Agent").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Apply, "⌘C", "Copy Issues").enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Script Issues footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::EnvPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled;
            let buttons =
                vec![FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(enabled)];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved EnvPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::WebcamView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Capture Photo").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Webcam footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::PathPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Select").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved PathPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::DropPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let has_files = match (&self.current_view, cx) {
                (AppView::DropPrompt { entity, .. }, Some(cx)) => {
                    !entity.read(cx).dropped_files.is_empty()
                }
                _ => false,
            };
            let submit_button = if footer_disabled {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(false)
            } else if has_files {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(true)
            } else {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit")
                    .disabled_reason("no_files")
            };
            let buttons = vec![
                submit_button,
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                has_files,
                "Resolved DropPrompt footer buttons"
            );
            return buttons;
        }

        if let AppView::BrowseKitsView {
            selected_index,
            results,
            query,
            ..
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled && results.get(*selected_index).is_some();
            let secondary_label = if query.is_empty() {
                "Back"
            } else {
                "Clear Search"
            };
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Install").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Close, "Esc", secondary_label)
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Kit Store browse footer buttons"
            );
            return buttons;
        }

        if let AppView::MigrateV1View {
            filter,
            selected_index,
            board,
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let visible = Self::migrate_visible_rows(&board.rows, filter);
            let selected = visible
                .get(*selected_index)
                .and_then(|row_ix| board.rows.get(*row_ix));
            let buttons = match board.phase {
                MigrateBoardPhase::Report => vec![
                    FooterButtonConfig::new(FooterAction::Run, "↵", "Port all")
                        .enabled(!footer_disabled && !board.rows.is_empty()),
                    FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")
                        .enabled(!footer_disabled),
                ],
                MigrateBoardPhase::Porting => vec![
                    FooterButtonConfig::new(FooterAction::Run, "", "Porting…").enabled(false),
                    FooterButtonConfig::new(FooterAction::Close, "Esc", "Hide").enabled(true),
                ],
                MigrateBoardPhase::Done => vec![
                    FooterButtonConfig::new(FooterAction::Run, "↵", "Port with AI").enabled(
                        !footer_disabled
                            && selected
                                .and_then(|row| row.status.as_deref())
                                .is_some_and(|status| status == "needs-review"),
                    ),
                    FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")
                        .enabled(!footer_disabled),
                ],
                _ => vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")
                    .enabled(!footer_disabled)],
            };
            return buttons;
        }

        if let AppView::InstalledKitsView {
            filter,
            selected_index,
            kits,
            ..
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled
                && Self::kit_store_installed_selected_visible_kit(kits, filter, *selected_index)
                    .is_some();
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Update").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Apply, "⌦", "Remove").enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Kit Store installed footer buttons"
            );
            return buttons;
        }

        // Tips browser: Copy Example + Back, honestly disabled when the
        // selected tip has no example to copy.
        if let AppView::TipsView {
            filter,
            selected_index,
            entries,
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let visible = script_kit_gpui::tips::visible_tip_indices(entries, filter);
            let has_example = visible
                .get(*selected_index)
                .and_then(|index| entries.get(*index))
                .is_some_and(|tip| !tip.examples.is_empty());
            let secondary_label = if filter.trim().is_empty() {
                "Back"
            } else {
                "Clear Search"
            };
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Copy Example")
                    .enabled(!footer_disabled && has_example),
                FooterButtonConfig::new(FooterAction::Close, "Esc", secondary_label)
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Tips footer buttons"
            );
            return buttons;
        }

        // EditorPrompt: Enter inserts a newline (submit is ⌘↵/⌘S), so the
        // standard "↵ Run" native footer would lie on this surface.
        if matches!(self.current_view, AppView::EditorPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "⌘↵", "Submit")
                    .enabled(!footer_disabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(!footer_disabled),
                FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel")
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved EditorPrompt footer buttons"
            );
            return buttons;
        }

        let buttons = self.standard_main_window_footer_buttons();
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_buttons_resolved",
            view = ?self.current_view,
            button_count = buttons.len(),
            "Resolved main-window native footer buttons"
        );
        buttons
    }

    /// Build footer buttons for the Agent Chat chat surface from the child-owned
    /// composer/thread state snapshot.
    fn agent_chat_footer_buttons(&self) -> Vec<crate::footer_popup::FooterButtonConfig> {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        let footer_disabled = self.main_window_footer_buttons_blocked();
        let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
        let enabled = !footer_disabled;

        if let Some(snapshot) = self.agent_chat_footer_snapshot.as_ref() {
            if !snapshot.visible {
                return Vec::new();
            }
            return snapshot
                .buttons
                .iter()
                .map(|button| {
                    let mut config =
                        FooterButtonConfig::new(button.action, button.key, button.label)
                            .selected(button.selected)
                            .enabled(enabled && button.enabled);
                    if let Some(reason) = button.disabled_reason {
                        config = config.disabled_reason(reason);
                    }
                    config
                })
                .collect();
        }

        vec![
            FooterButtonConfig::new(FooterAction::Run, "↵", "Send")
                .disabled_reason("loading_agent_chat"),
            FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                .selected(actions_open)
                .enabled(enabled),
        ]
    }

    pub(crate) fn main_window_footer_config(
        &self,
    ) -> Option<crate::footer_popup::MainWindowFooterConfig> {
        self.main_window_footer_config_with_cx(None)
    }

    pub(crate) fn main_window_footer_config_with_cx(
        &self,
        cx: Option<&gpui::App>,
    ) -> Option<crate::footer_popup::MainWindowFooterConfig> {
        use crate::footer_popup::MainWindowFooterConfig;

        if let AppView::AgentChatView { entity } = &self.current_view {
            let controls_visible = cx
                .map(|cx| entity.read(cx).main_window_footer_visible(cx))
                .or_else(|| {
                    self.agent_chat_footer_snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.visible)
                });
            if !main_window_footer_chrome_should_render(true, controls_visible) {
                return None;
            }
        }

        let surface = self.main_window_footer_surface()?;
        let buttons = self.main_window_footer_buttons_for_current_view(cx);

        // debug!: resolved on every render frame and every state collection;
        // info-level logging here is per-frame I/O during arrow-key scroll.
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_config_resolved",
            view = ?self.current_view,
            surface,
            button_count = buttons.len(),
            "Resolved main-window native footer config"
        );

        let mut config = MainWindowFooterConfig::new(surface, buttons);
        if matches!(self.current_view, AppView::ScriptList)
            && self.filter_text.trim().is_empty()
            && self.config.is_tips_enabled()
        {
            if let Some(tip) = script_kit_gpui::tips::current_footer_tip() {
                config.left_info = Some(crate::footer_popup::FooterLeftInfo {
                    model_name: tip.hint.clone(),
                    icon_token: Some("lightbulb".to_string()),
                    keycap: tip.hint_key.clone(),
                    action: Some(crate::footer_popup::FooterAction::Tips),
                    ..Default::default()
                });
            }
        }
        // Main list slow-filling (explicit tabs:/history: fetch, visible
        // root file search): pair the constellation layer with a braille
        // spinner + per-kind status label in the footer's left slot. Tips
        // only render on an empty query, so this never clobbers them.
        if let Some(kind) = self.main_list_loading_kind() {
            config.left_info = Some(main_list_loading_left_info(
                kind,
                self.main_list_loading_elapsed_secs(),
            ));
        }
        Some(config)
    }

    pub(crate) fn main_window_uses_native_footer(&self) -> bool {
        self.main_window_footer_surface()
            .is_some_and(|expected_surface| {
                // Tahoe glass has one canonical footer owner: the native host
                // in the main NSWindow. Logical visibility flips false before
                // AppKit's exit fade completes, so it must not participate in
                // this ownership decision; doing so paints an in-stage GPUI
                // fallback beside the still-visible detached footer.
                //
                // Do not paint a GPUI fallback on the installation frame
                // either. Non-glass mode still waits for the installed-host
                // receipt before suppressing its fallback.
                crate::footer_popup::glass_scroll_bands_active()
                    || crate::footer_popup::active_main_window_footer_surface()
                        == Some(expected_surface)
            })
    }

    /// When the native main-window footer is active, replace the GPUI footer
    /// with either the blur-era spacer or, in Tahoe glass mode, an absolute
    /// hover blocker that lets scroll content continue beneath the glass band.
    pub(crate) fn main_window_footer_slot(
        &self,
        gpui_footer: gpui::AnyElement,
    ) -> Option<gpui::AnyElement> {
        let is_agent_chat = matches!(self.current_view, AppView::AgentChatView { .. });
        let agent_chat_controls_visible = self
            .agent_chat_footer_snapshot
            .as_ref()
            .map(|snapshot| snapshot.visible);
        if !main_window_footer_chrome_should_render(is_agent_chat, agent_chat_controls_visible) {
            return None;
        }
        if self.main_window_uses_native_footer() {
            if crate::footer_popup::glass_scroll_bands_active() {
                Some(
                    crate::components::prompt_layout_shell::render_native_main_window_footer_hover_blocker(),
                )
            } else {
                Some(
                    crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(
                    ),
                )
            }
        } else {
            Some(gpui_footer)
        }
    }

    fn handle_main_footer_action(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_action_dispatch",
            source = "native_footer",
            action = ?action,
            view = ?self.current_view,
            main_window_mode = ?self.main_window_mode,
            "Dispatching main-window footer action"
        );

        if self.main_window_footer_config_with_cx(Some(&*cx)).is_none()
            || (!window.is_owned_hidden() && !crate::is_main_window_visible())
        {
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_action_ignored_inactive_surface",
                source = "native_footer",
                action = ?action,
                view = ?self.current_view,
                main_window_mode = ?self.main_window_mode,
                "Ignored native footer action because current view is not using the native footer"
            );
            return;
        }

        self.dispatch_main_window_footer_action(action, window, cx, "native_footer");
    }

    pub(crate) fn sync_main_footer_popup(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_main_footer_action_listener(window, cx);

        let mut config = if window.is_owned_hidden() || crate::is_main_window_visible() {
            self.main_window_footer_config_with_cx(Some(&*cx))
        } else {
            None
        };

        // Enrich with Agent Chat streaming/model info when on the Agent Chat chat view.
        if let Some(ref mut cfg) = config {
            self.enrich_footer_config_with_agent_chat_info(cfg);
        }

        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_sync",
            view = ?self.current_view,
            show = config.is_some(),
            surface = config.as_ref().map(|c| c.surface).unwrap_or("none"),
            button_count = config.as_ref().map(|c| c.buttons.len()).unwrap_or(0),
            "Syncing native main window footer"
        );

        if !crate::footer_popup::footer_config_matches(window.window_handle(), config.as_ref()) {
            self.mark_main_presentation_changed();
        }
        crate::footer_popup::sync_main_footer_popup(window, config.as_ref(), &mut *cx);
    }

    /// The global working-directory footer chip, sourced from `spine_cwd_label`
    /// so the main menu and Agent Chat show the same persistent cwd. Returns
    /// `None` only when no cwd is established.
    pub(crate) fn global_footer_cwd_chip(&self) -> Option<crate::footer_popup::FooterCwdChip> {
        self.spine_cwd_label
            .as_ref()
            .map(|label| crate::footer_popup::FooterCwdChip {
                label: label.clone(),
                icon_token: "folder".to_string(),
                key: Some("⇥".to_string()),
                tooltip: None,
            })
    }

    /// Combined "Agent · Model" footer label, derived from the persisted Pi
    /// provider/model selection (`spine_agent_label` / `spine_model_label`).
    /// Returns `None` when neither label is known so the chip stays hidden.
    pub(crate) fn agent_model_footer_label(&self) -> Option<String> {
        match (
            self.spine_agent_label.as_ref(),
            self.spine_model_label.as_ref(),
        ) {
            (Some(agent), Some(model)) => Some(format!("{agent} · {model}")),
            (Some(agent), None) => Some(agent.clone()),
            (None, Some(model)) => Some(model.clone()),
            (None, None) => None,
        }
    }

    pub(crate) fn prompt_header_context(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> crate::prompts::base::PromptHeaderContext {
        let zone = self.main_view_context_zone_spec();
        let app = cx.entity().downgrade();
        let handler: crate::components::main_view_chrome::SemanticChipActionHandler =
            std::rc::Rc::new(move |invocation, window, cx| {
                let Some(app) = app.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    use crate::components::main_view_chrome::{
                        SemanticChipAction, MAIN_VIEW_CONTEXT_CWD_BUTTON_ID,
                        MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID, MAIN_VIEW_CONTEXT_QUICK_AI_BUTTON_ID,
                        MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID,
                    };
                    match (invocation.semantic_id.as_ref(), invocation.action) {
                        (MAIN_VIEW_CONTEXT_CWD_BUTTON_ID, SemanticChipAction::OpenSelector) => {
                            this.dispatch_main_window_footer_action(
                                crate::footer_popup::FooterAction::Cwd,
                                window,
                                cx,
                                "main_view_context_click",
                            );
                        }
                        (MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID, SemanticChipAction::OpenSelector) => {
                            this.dispatch_main_window_footer_action(
                                crate::footer_popup::FooterAction::AgentModel,
                                window,
                                cx,
                                "main_view_context_click",
                            );
                        }
                        (MAIN_VIEW_CONTEXT_QUICK_AI_BUTTON_ID, SemanticChipAction::OpenSurface) => {
                            let query = this.filter_text.clone();
                            this.open_quick_ai_from_launcher(query, window, cx);
                        }
                        (
                            MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID,
                            SemanticChipAction::OpenDetails,
                        ) => {
                            tracing::info!(
                                target: "script_kit::selection_hint",
                                event = "selection_context_details_opened",
                            );
                            this.open_selection_context_details("selection_hint_chip", cx);
                        }
                        _ => {
                            tracing::warn!(
                                target: "script_kit::main_view_chrome",
                                semantic_id = %invocation.semantic_id,
                                action = ?invocation.action,
                                "Ignored unsupported semantic chip invocation"
                            );
                        }
                    }
                });
            });
        crate::prompts::base::PromptHeaderContext {
            zone,
            on_action: handler,
        }
    }

    pub(crate) fn render_clickable_main_view_context_zone(
        &self,
        menu_def: crate::designs::MainMenuThemeDef,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        self.prompt_header_context(cx).render(&self.theme, menu_def)
    }

    pub(crate) fn render_clickable_main_view_context_header(
        &self,
        menu_def: crate::designs::MainMenuThemeDef,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        crate::components::main_view_chrome::render_main_view_context_header(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
        )
    }

    #[allow(dead_code)]
    pub(crate) fn render_inert_main_view_context_zone(
        &self,
        menu_def: crate::designs::MainMenuThemeDef,
    ) -> gpui::AnyElement {
        crate::components::main_view_chrome::render_main_view_context_zone_required(
            &self.theme,
            menu_def,
            self.main_view_context_zone_spec(),
            std::rc::Rc::new(|_, _, _| {}),
        )
    }

    pub(crate) fn enrich_footer_config_with_agent_chat_info(
        &self,
        config: &mut crate::footer_popup::MainWindowFooterConfig,
    ) {
        if matches!(self.current_view, AppView::AgentChatView { .. }) {
            // Cwd and Agent/Model now live in the shared main-view header. Keep
            // the native footer scoped to surface actions only, and make sure
            // stale Agent Chat left-info state cannot reintroduce duplicate model/cwd
            // chips beside the footer buttons.
            config.left_info = None;
        }
    }

    pub(crate) fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        self.mark_main_data_changed();
        self.mark_main_presentation_changed();
        cx.notify();
    }

    /// Toggle the focused-info panel visibility (Cmd+I / "Show Info" action).
    pub(crate) fn toggle_info_panel(&mut self, cx: &mut Context<Self>) {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.show_info_panel = !self.show_info_panel;
        tracing::info!(
            category = "UI",
            event = "toggle_info_panel",
            visible = self.show_info_panel,
            "Info panel toggled"
        );
        cx.notify();
    }

    /// Calculate view type and item count for window sizing.
    /// Extracted from update_window_size for reuse.
    pub(crate) fn calculate_window_size_params(&mut self) -> Option<(ViewType, usize)> {
        self.calculate_window_size_params_with_app(None)
    }

    pub(crate) fn calculate_window_size_params_with_app(
        &mut self,
        cx: Option<&gpui::App>,
    ) -> Option<(ViewType, usize)> {
        match &self.current_view {
            AppView::ScriptList => {
                // Get grouped results which includes section headers (cached)
                let (grouped_items, _) = self.get_grouped_results_cached();
                let count = grouped_items.len();
                let view_type = match self.main_window_mode {
                    MainWindowMode::Full => ViewType::ScriptList,
                    MainWindowMode::Mini => ViewType::MainWindow,
                };
                Some((view_type, count))
            }
            AppView::About { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::ArgPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                if filtered.is_empty() && choices.is_empty() {
                    Some((ViewType::ArgPromptNoChoices, 0))
                } else {
                    Some((ViewType::ArgPromptWithChoices, filtered.len()))
                }
            }
            AppView::MiniPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                if filtered.is_empty() && choices.is_empty() {
                    Some((mini_prompt_view_type(), 0))
                } else {
                    Some((mini_prompt_view_type(), filtered.len()))
                }
            }
            AppView::MicroPrompt { .. } => Some((ViewType::MicroPrompt, 0)),
            AppView::DivPrompt { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::FormPrompt { .. } => Some((ViewType::DivPrompt, 0)), // Use DivPrompt size for forms
            AppView::EditorPrompt { .. } => Some((ViewType::EditorPrompt, 0)),
            AppView::SelectPrompt { entity, .. } => Some((
                ViewType::SelectPrompt,
                cx.map(|cx| entity.read(cx).filtered_choices.len())
                    .unwrap_or(1),
            )),
            AppView::PathPrompt { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::EnvPrompt { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::DropPrompt { .. } => Some((ViewType::DivPrompt, 0)), // Drop prompt uses div size for drop zone
            AppView::TemplatePrompt { .. } => Some((ViewType::DivPrompt, 0)), // Template prompt uses div size
            AppView::HotkeyPrompt { .. } => Some((ViewType::DivPrompt, 0)), // Hotkey prompt uses compact recorder surface
            AppView::ChatPrompt { .. } => {
                Some((compact_ai_view_type_for_mode(self.main_window_mode), 0))
            }
            AppView::TermPrompt { .. } => Some((ViewType::TermPrompt, 0)),
            AppView::ActionsDialog => {
                // Actions dialog is an overlay, don't resize
                None
            }
            // Preview/detail builtins widen from the mini launcher without
            // increasing height, so the shared header/input stays fixed.
            // View state only - data comes from self fields
            AppView::ClipboardHistoryView { filter, .. } => {
                let entries = &self.cached_clipboard_entries;
                let filtered_count = if filter.is_empty() {
                    entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries
                        .iter()
                        .filter(|e| e.text_preview.to_lowercase().contains(&filter_lower))
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::EmojiPickerView {
                filter,
                selected_category,
                ..
            } => {
                let row_count = crate::emoji::filtered_grid_row_count(filter, *selected_category);
                Some((ViewType::MainWindow, row_count))
            }
            AppView::AppLauncherView { filter, .. } => {
                let apps = &self.apps;
                let filtered_count = if filter.is_empty() {
                    apps.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    apps.iter()
                        .filter(|a| a.name.to_lowercase().contains(&filter_lower))
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::WindowSwitcherView { filter, .. } => {
                let windows = &self.cached_windows;
                let filtered_count = if filter.is_empty() {
                    windows.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    windows
                        .iter()
                        .filter(|w| {
                            w.title.to_lowercase().contains(&filter_lower)
                                || w.app.to_lowercase().contains(&filter_lower)
                        })
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::ProcessManagerView { filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    self.cached_processes.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    self.cached_processes
                        .iter()
                        .filter(|p| p.script_path.to_lowercase().contains(&filter_lower))
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::FlowUxView { filter, .. } => {
                let cwd = self.flow_ux_cwd();
                let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
                let count = crate::flows::catalog::filter_flows(&roster.flows, filter).len();
                Some((ViewType::MainWindow, count))
            }
            AppView::CurrentAppCommandsView { filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    self.cached_current_app_entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    self.cached_current_app_entries
                        .iter()
                        .filter(|e| {
                            e.name.to_lowercase().contains(&filter_lower)
                                || e.keywords.iter().any(|k| k.contains(&filter_lower))
                        })
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::BrowserTabsView { filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    self.cached_browser_tabs.len()
                } else {
                    crate::browser_tabs::fuzzy_search_browser_tabs(
                        &self.cached_browser_tabs,
                        filter,
                    )
                    .len()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::ScratchPadView { .. } => Some((ViewType::EditorPrompt, 0)),
            AppView::QuickTerminalView { .. } => Some((ViewType::TermPrompt, 0)),
            AppView::FlowSessionView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::WebcamView { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::FileSearchView {
                ref query,
                presentation,
                ..
            } => {
                let results = &self.cached_file_results;
                let filtered_count = if query.is_empty() {
                    results.len()
                } else {
                    let query_lower = query.to_lowercase();
                    results
                        .iter()
                        .filter(|r| r.name.to_lowercase().contains(&query_lower))
                        .count()
                };
                let view_type = match presentation {
                    FileSearchPresentation::Mini => ViewType::MainWindow,
                    FileSearchPresentation::Full => ViewType::MainWindow,
                };
                Some((view_type, filtered_count))
            }
            AppView::ProfileSearchView { filter, .. } => Some((
                ViewType::MainWindow,
                self.profile_search_visible_len(filter),
            )),
            AppView::ThemeChooserView { ref filter, .. } => {
                // Size against the unified catalog (user themes + presets) so
                // the window height matches what the gallery actually shows.
                let catalog = Self::theme_chooser_catalog();
                let filtered_count =
                    Self::theme_chooser_catalog_filtered_indices(filter, &catalog).len();
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::CreationFeedback { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::ScriptIssuesView { .. } => Some((ViewType::ArgPromptNoChoices, 0)),
            AppView::SdkReferenceView {
                entries, filter, ..
            } => {
                let (_, count) =
                    crate::mcp_resources::sdk_reference_dataset_and_visible_counts(entries, filter);
                Some((ViewType::MainWindow, count))
            }
            AppView::TipsView {
                entries, filter, ..
            } => {
                let count = script_kit_gpui::tips::visible_tip_indices(entries, filter).len();
                Some((ViewType::MainWindow, count))
            }
            AppView::ScriptTemplateCatalogView {
                templates, filter, ..
            } => {
                let (_, count) =
                    crate::mcp_resources::script_template_catalog_dataset_and_visible_counts(
                        templates, filter,
                    );
                Some((ViewType::MainWindow, count))
            }
            AppView::NamingPrompt { .. } => Some((ViewType::ArgPromptNoChoices, 0)),
            AppView::BrowseKitsView { results, .. } => Some((ViewType::MainWindow, results.len())),
            AppView::MigrateV1View { filter, board, .. } => Some((
                ViewType::MainWindow,
                Self::migrate_visible_rows(&board.rows, filter).len(),
            )),
            AppView::InstalledKitsView { filter, kits, .. } => Some((
                ViewType::MainWindow,
                Self::kit_store_installed_visible_rows(kits, filter).len(),
            )),
            AppView::SearchAiPresetsView { .. } => {
                // Presets list - defaults (5) + user presets
                let count = crate::ai::presets::load_presets()
                    .map(|p| 5 + p.len())
                    .unwrap_or(5);
                Some((ViewType::MainWindow, count))
            }
            AppView::CreateAiPresetView { .. } => {
                // Fixed-size form with 3 fields
                Some((ViewType::ArgPromptNoChoices, 0))
            }
            AppView::SettingsView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::PermissionsWizardView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::FavoritesBrowseView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::AgentChatHistoryView { filter, .. } => {
                let entries = crate::ai::agent_chat::ui::history::load_history();
                let filtered_count = if filter.is_empty() {
                    entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries
                        .iter()
                        .filter(|entry| {
                            entry.first_message.to_lowercase().contains(&filter_lower)
                                || entry.timestamp.to_lowercase().contains(&filter_lower)
                        })
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::BrowserHistoryView { filter, .. } => Some((
                ViewType::MainWindow,
                crate::browser_history::fuzzy_search_browser_history(
                    &self.cached_browser_history,
                    filter,
                )
                .len(),
            )),
            AppView::DictationHistoryView {
                filter,
                visible_limit,
                ..
            } => Some((
                ViewType::MainWindow,
                self.dictation_history_current_or_previous_page(filter, *visible_limit)
                    .map(|page| page.visible_count)
                    .unwrap_or(0),
            )),
            AppView::NotesBrowseView { search } => {
                Some((ViewType::MainWindow, search.state.rows().len()))
            }
            AppView::AgentChatView { entity } => {
                if let Some(cx) = cx {
                    if let Some(item_count) = entity.read(cx).focused_text_mini_sizing_count(cx) {
                        return Some((ViewType::FocusedTextMini, item_count));
                    }
                }
                Some((compact_ai_view_type_for_mode(self.main_window_mode), 0))
            }
            AppView::DayPage { .. } => Some((ViewType::MainWindow, 0)),
            // In-window confirm participates in the canonical 480px main
            // shell; only its body controls differ from searchable views.
            AppView::ConfirmPrompt { .. } => Some((ViewType::MainWindow, 0)),
        }
    }

    pub(crate) fn set_main_window_mode(
        &mut self,
        mode: MainWindowMode,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
        source: &'static str,
    ) {
        let old = self.main_window_mode;
        if old == mode {
            return;
        }

        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.main_window_mode = mode;

        if let AppView::ChatPrompt { entity, .. } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat, _cx| {
                chat.set_mini_mode(mode == MainWindowMode::Mini);
            });
        }

        let shared_actions_open = self.show_actions_popup;
        let detached_actions_open = crate::actions::is_actions_window_open();
        if shared_actions_open {
            if let super::actions_dialog::ActionsSupport::SharedDialog(host) =
                self.actions_support_for_view()
            {
                self.close_actions_popup(host, window, cx);
            } else {
                self.clear_actions_popup_state();
            }
        }
        if detached_actions_open {
            crate::actions::close_actions_window(cx);
        }

        self.update_window_size_deferred(window, cx);
        self.sync_main_footer_popup(window, cx);
        tracing::info!(
            target: "script_kit::window_mode",
            event = "main_window_mode_changed",
            source,
            old = ?old,
            new = ?mode,
            view = ?self.current_view,
            "Main window mode changed atomically"
        );
    }

    pub(crate) fn set_main_window_mode_state_only(
        &mut self,
        mode: MainWindowMode,
        cx: &mut Context<Self>,
        source: &'static str,
    ) {
        let old = self.main_window_mode;
        if old == mode {
            return;
        }

        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.main_window_mode = mode;
        if let AppView::ChatPrompt { entity, .. } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat, _cx| {
                chat.set_mini_mode(mode == MainWindowMode::Mini);
            });
        }
        tracing::info!(
            target: "script_kit::window_mode",
            event = "main_window_mode_changed",
            source,
            old = ?old,
            new = ?mode,
            view = ?self.current_view,
            "Main window mode changed without window handle"
        );
    }

    /// Calculate sizing only when the current view still matches the caller's
    /// expected async resize target.
    pub(crate) fn calculate_window_size_params_if_current_view(
        &mut self,
        reason: &'static str,
        is_expected_view: impl FnOnce(&AppView) -> bool,
    ) -> Option<(ViewType, usize)> {
        if !is_expected_view(&self.current_view) {
            tracing::debug!(
                target: "WINDOW_RESIZE",
                reason,
                current_view = ?self.current_view,
                "Skipping stale deferred resize for inactive view"
            );
            return None;
        }

        self.calculate_window_size_params()
    }

    /// Returns the focused button when the active view is `ConfirmPrompt`.
    pub(crate) fn confirm_prompt_focused_button(&self) -> Option<ConfirmFocusedButton> {
        if let AppView::ConfirmPrompt { focused_button, .. } = &self.current_view {
            Some(*focused_button)
        } else {
            None
        }
    }

    /// Flip Tab focus between Confirm and Cancel inside an active `ConfirmPrompt`.
    pub(crate) fn toggle_confirm_prompt_focus(&mut self, cx: &mut Context<Self>) {
        if let AppView::ConfirmPrompt { focused_button, .. } = &mut self.current_view {
            *focused_button = focused_button.toggled();
            cx.notify();
        }
    }

    /// Send the confirm/cancel result to the awaiting caller and restore the
    /// previous launcher view. No-op if the active view is not `ConfirmPrompt`.
    pub(crate) fn resolve_confirm_prompt(
        &mut self,
        confirmed: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let AppView::ConfirmPrompt {
            sender, previous, ..
        } = &self.current_view
        else {
            return;
        };
        if let Err(error) = sender.try_send(confirmed) {
            self.show_error_toast(format!("Unable to deliver confirmation: {error}"), cx);
            return;
        }
        if self
            .prompt_completion
            .as_ref()
            .is_some_and(|binding| binding.is_confirm_lifetime() && !binding.observation().retired)
        {
            // The bound completion owner restores only after the SDK/local sink succeeds.
            return;
        }
        let previous = (**previous).clone();
        self.transition_current_view_and_rekey_main_automation_surface(previous);
        if matches!(self.current_view, AppView::ScriptList) {
            self.flush_pending_main_menu_query(cx);
        }
        self.sync_main_footer_popup(window, cx);
        cx.notify();
    }

    /// Update window size using deferred execution (SAFE during render/event cycles).
    ///
    /// Uses Window::defer to schedule the resize at the end of the current effect cycle,
    /// preventing RefCell borrow conflicts that can occur when calling platform APIs
    /// during GPUI's render or event processing.
    ///
    /// Use this version when you have access to `window` and `cx`.
    pub(crate) fn update_window_size_deferred(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Content-aware mini mode sizing bypasses the flat (ViewType, item_count) path.
        if matches!(self.current_view, AppView::ScriptList)
            && self.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let sizing = main_window_sizing_from_grouped_items(&grouped_items);
            let target_height = crate::window_resize::height_for_main_window(sizing);
            crate::window_resize::log_main_window_sizing(
                crate::window_resize::ResizeReason::FilterChanged,
                sizing,
                f32::from(target_height),
            );
            crate::window_resize::defer_resize_to_main_window(sizing, window, &mut *cx);
            return;
        }

        if let Some((view_type, item_count)) = self.calculate_window_size_params() {
            crate::window_resize::defer_resize_to_view(view_type, item_count, window, &mut *cx);
        }
    }

    /// Update window size synchronously.
    ///
    /// SAFETY: Only call from async handlers (cx.spawn closures, message handlers)
    /// that run OUTSIDE the GPUI render cycle. Calling during render will cause
    /// RefCell borrow panics.
    ///
    /// Prefer `update_window_size_deferred` when you have window/cx access.
    pub(crate) fn update_window_size(&mut self) {
        // Content-aware mini mode sizing bypasses the flat (ViewType, item_count) path.
        if matches!(self.current_view, AppView::ScriptList)
            && self.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let sizing = main_window_sizing_from_grouped_items(&grouped_items);
            let target_height = crate::window_resize::height_for_main_window(sizing);
            crate::window_resize::log_main_window_sizing(
                crate::window_resize::ResizeReason::GroupedResultsChanged,
                sizing,
                f32::from(target_height),
            );
            crate::window_resize::resize_to_main_window_sync(sizing);
            return;
        }

        if let Some((view_type, item_count)) = self.calculate_window_size_params() {
            crate::window_resize::resize_to_view_sync(view_type, item_count);
        }
    }

    /// Resize the current surface to its canonical height while restoring an
    /// explicit width.
    pub(crate) fn resize_current_view_to_width(&mut self, target_width: f32) {
        if !target_width.is_finite() || target_width <= 0.0 {
            self.update_window_size();
            return;
        }

        // Content-aware mini mode sizing bypasses the flat (ViewType, item_count) path.
        if matches!(self.current_view, AppView::ScriptList)
            && self.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let sizing = main_window_sizing_from_grouped_items(&grouped_items);
            let target_height = crate::window_resize::height_for_main_window(sizing);
            crate::window_resize::log_main_window_sizing(
                crate::window_resize::ResizeReason::GroupedResultsChanged,
                sizing,
                f32::from(target_height),
            );
            let width = if self.main_window_mode == MainWindowMode::Mini {
                crate::window_resize::width_for_view(ViewType::MainWindow).unwrap_or(target_width)
            } else {
                target_width
            };
            crate::window_resize::resize_first_window_to_size(target_height, Some(width));
            return;
        }

        if let Some((view_type, item_count)) = self.calculate_window_size_params() {
            let target_height = crate::window_resize::height_for_view(view_type, item_count);
            let width = if self.main_window_mode == MainWindowMode::Mini {
                crate::window_resize::width_for_view(ViewType::MainWindow).unwrap_or(target_width)
            } else {
                target_width
            };
            crate::window_resize::resize_first_window_to_size(target_height, Some(width));
        }
    }

    /// Try to insert text into the current prompt's input field.
    ///
    /// Returns `true` when the current view accepted the text (i.e. there is an
    /// active prompt with an input field), `false` otherwise.  Used by dictation
    /// to decide whether to fall back to paste-to-frontmost-app.
    /// Returns `true` when the launcher/main-menu filter is active and can
    /// accept dictated text (i.e. `AppView::ScriptList`).
    pub(crate) fn can_accept_dictation_into_main_filter(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
    }

    /// Returns `true` when the current view can accept dictated text directly.
    pub(crate) fn can_accept_dictation_into_prompt(&self) -> bool {
        matches!(
            &self.current_view,
            AppView::ArgPrompt { .. }
                | AppView::MiniPrompt { .. }
                | AppView::MicroPrompt { .. }
                | AppView::PathPrompt { .. }
                | AppView::SelectPrompt { .. }
                | AppView::EnvPrompt { .. }
                | AppView::TemplatePrompt { .. }
                | AppView::FormPrompt { .. }
                | AppView::FileSearchView { .. }
        )
    }

    pub(crate) fn try_set_prompt_input(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        match &mut self.current_view {
            AppView::ArgPrompt { .. } => {
                self.filter_text = text.clone();
                self.pending_filter_sync = true;
                self.arg_input.set_text(text);
                self.set_arg_selected_index(0);
                self.arg_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.mark_main_data_changed();
                if !crate::runtime_policy::is_owned_evaluation() {
                    self.update_window_size();
                }
                cx.notify();
                true
            }
            AppView::MiniPrompt { .. } | AppView::MicroPrompt { .. } => {
                self.filter_text = text.clone();
                self.pending_filter_sync = true;
                self.arg_input.set_text(text);
                self.set_arg_selected_index(0);
                self.arg_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.mark_main_data_changed();
                if !crate::runtime_policy::is_owned_evaluation() {
                    self.update_window_size();
                }
                cx.notify();
                true
            }
            AppView::PathPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::SelectPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::EnvPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::TemplatePrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::FormPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::AgentChatView { entity } => {
                entity.update(cx, |view, cx| view.set_input(text, cx));
                true
            }
            AppView::ChatPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::FileSearchView {
                query,
                selected_index,
                ..
            } => {
                let results = match ScriptListApp::resolve_file_search_results(&text) {
                    Ok(results) => results,
                    Err(error) => {
                        self.show_error_toast(format!("File search failed: {error}"), cx);
                        return true;
                    }
                };
                logging::log(
                    "EXEC",
                    &format!(
                        "File search setInput '{}' found {} results",
                        text,
                        results.len()
                    ),
                );
                *query = text.clone();
                *selected_index = 0;
                self.update_file_search_results(results);
                self.file_search_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.filter_text = text;
                self.pending_filter_sync = true;
                cx.notify();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn set_prompt_input(&mut self, text: String, cx: &mut Context<Self>) {
        let _ = self.try_set_prompt_input(text, cx);
    }

    /// Helper to get filtered arg choices without cloning
    pub(crate) fn get_filtered_arg_choices<'a>(&self, choices: &'a [Choice]) -> Vec<&'a Choice> {
        if self.arg_input.is_empty() {
            choices.iter().collect()
        } else {
            let filter = self.arg_input.text().to_lowercase();
            choices
                .iter()
                .filter(|c| c.name.to_lowercase().contains(&filter))
                .collect()
        }
    }

    pub(crate) fn focus_main_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focused_input = FocusedInput::MainFilter;
        let input_state = self.gpui_input_state.clone();
        input_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    /// Apply a dictated transcript to the launcher's shared main-filter input.
    ///
    /// Returns `true` when the launcher was active and the text was applied,
    /// `false` otherwise (caller should fall back to frontmost-app paste).
    pub(crate) fn try_set_main_window_filter_from_dictation(
        &mut self,
        text: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_accept_dictation_into_main_filter() {
            return false;
        }

        let text = crate::components::text_input::core::normalize_single_line_text(text);

        tracing::info!(
            category = "DICTATION",
            event = "dictation_set_main_window_filter",
            text_len = text.len(),
            "Applying dictated transcript to launcher filter"
        );

        self.filter_text = text.clone();
        self.pending_filter_sync = true;
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        self.hovered_index = None;
        self.selected_index = 0;
        self.queue_filter_compute(text, cx);
        cx.notify();
        true
    }

    /// Clear the cached preflight receipt so it is rebuilt on the next
    /// call to `rebuild_main_window_preflight_if_needed`.
    /// Kept as explicit API for context-chip toggles and view transitions.
    #[allow(dead_code)]
    pub(crate) fn invalidate_main_window_preflight(&mut self) {
        self.cached_main_window_preflight = None;
        self.main_window_preflight_cache_key.clear();
    }

    /// Rebuild the preflight receipt when the cache key has changed.
    /// Call this from mutation paths (filter change, selection change)
    /// — never from `render()`.
    ///
    /// The cache key covers the row-shaping inputs (filter text + view).
    /// When only `selected_index` changed — the arrow-key scroll hot path —
    /// the cached receipt's visible rows/fingerprints/counts are still valid,
    /// so only the selection-dependent fields are refreshed (O(1)) instead of
    /// rebuilding the full O(visible rows) receipt on every keypress.
    pub(crate) fn rebuild_main_window_preflight_if_needed(&mut self) {
        let rows_key = format!("{}:{:?}", self.filter_text, self.current_view);
        if rows_key == self.main_window_preflight_cache_key {
            let Some(mut receipt) = self.cached_main_window_preflight.take() else {
                // Rows unchanged and the view is not preflight-eligible;
                // selection changes cannot make it eligible.
                return;
            };
            if receipt.selected_index != self.selected_index {
                crate::main_window_preflight::refresh_main_window_preflight_selection(
                    self,
                    &mut receipt,
                );
                if crate::logging::filter_perf_trace_enabled() {
                    crate::main_window_preflight::log_main_window_preflight_receipt(&receipt);
                }
            }
            self.cached_main_window_preflight = Some(receipt);
            return;
        }
        self.main_window_preflight_cache_key = rows_key;
        let receipt = crate::main_window_preflight::build_main_window_preflight_receipt(self);
        if crate::logging::filter_perf_trace_enabled() {
            if let Some(ref r) = receipt {
                crate::main_window_preflight::log_main_window_preflight_receipt(r);
            }
        }
        self.cached_main_window_preflight = receipt;
    }
}

#[cfg(test)]
#[path = "ui_window_tests.rs"]
mod tests;

include!("ui_window_footer_helpers.rs");
include!("ui_window_context_chips.rs");
include!("ui_window_interaction_helpers.rs");

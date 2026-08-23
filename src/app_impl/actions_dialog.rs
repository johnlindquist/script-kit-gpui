use super::*;

/// Whether a view supports the shared ActionsDialog, and if so, which host
/// identity to use for focus-restore and key routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionsSupport {
    /// View participates in the shared ActionsDialog with the given host.
    SharedDialog(ActionsDialogHost),
    /// View does not support the shared ActionsDialog.
    None,
}

fn menu_syntax_displayed_shortcut_should_consume(canonical_shortcut: &str) -> bool {
    canonical_shortcut == "enter"
}

impl ScriptListApp {
    fn is_builtin_list_actions_view(view: &AppView) -> bool {
        matches!(
            view,
            AppView::BrowserHistoryView { .. }
                | AppView::BrowserTabsView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::CreateAiPresetView { .. }
                | AppView::SettingsView { .. }
                | AppView::FavoritesBrowseView { .. }
                | AppView::BrowseKitsView { .. }
                | AppView::MigrateV1View { .. }
                | AppView::InstalledKitsView { .. }
        )
    }

    /// Static host identity for actions-related routing/focus contracts.
    ///
    /// This is broader than live Cmd+K popup support. Use
    /// `live_actions_host_for_view` or `current_actions_host` when deciding
    /// whether the visible view may open the shared ActionsDialog.
    pub(crate) fn actions_host_for_view(view: &AppView) -> Option<ActionsDialogHost> {
        match view {
            AppView::ScriptList | AppView::DayPage { .. } => Some(ActionsDialogHost::MainList),
            AppView::ClipboardHistoryView { .. } => Some(ActionsDialogHost::ClipboardHistory),
            AppView::DictationHistoryView { .. } => Some(ActionsDialogHost::DictationHistory),
            AppView::FavoritesBrowseView { .. } => Some(ActionsDialogHost::Favorites),
            AppView::ThemeChooserView { .. } => Some(ActionsDialogHost::ThemeChooser),
            AppView::EmojiPickerView { .. } => Some(ActionsDialogHost::EmojiPicker),
            AppView::FileSearchView { .. } => Some(ActionsDialogHost::FileSearch),
            AppView::ChatPrompt { .. } => Some(ActionsDialogHost::ChatPrompt),
            AppView::ArgPrompt { .. } => Some(ActionsDialogHost::ArgPrompt),
            AppView::DivPrompt { .. } => Some(ActionsDialogHost::DivPrompt),
            AppView::EditorPrompt { .. } => Some(ActionsDialogHost::EditorPrompt),
            AppView::TemplatePrompt { .. } => Some(ActionsDialogHost::TemplatePrompt),
            AppView::TermPrompt { .. } => Some(ActionsDialogHost::TermPrompt),
            AppView::FormPrompt { .. } => Some(ActionsDialogHost::FormPrompt),
            AppView::WebcamView { .. } => Some(ActionsDialogHost::WebcamPrompt),
            AppView::AgentChatView { .. } => Some(ActionsDialogHost::AgentChat),
            AppView::AgentChatHistoryView { .. } => Some(ActionsDialogHost::AgentChatHistory),
            AppView::AppLauncherView { .. } => Some(ActionsDialogHost::AppLauncher),
            AppView::FlowUxView { .. } | AppView::FlowSessionView { .. } => {
                Some(ActionsDialogHost::FlowDesk)
            }
            AppView::BrowserHistoryView { .. }
            | AppView::BrowserTabsView { .. }
            | AppView::WindowSwitcherView { .. }
            | AppView::CurrentAppCommandsView { .. }
            | AppView::ProcessManagerView { .. }
            | AppView::SearchAiPresetsView { .. }
            | AppView::CreateAiPresetView { .. }
            | AppView::SettingsView { .. }
            | AppView::BrowseKitsView { .. }
            | AppView::MigrateV1View { .. }
            | AppView::InstalledKitsView { .. } => Some(ActionsDialogHost::BuiltinList),
            _ => None,
        }
    }

    /// Live routing host for main-window popup handling.
    ///
    /// Generic BuiltinList views remain in `actions_host_for_view` so static
    /// routing and focus-restore code can identify them, but they are not live
    /// Cmd+K hosts until each view provides selection-specific actions.
    pub(crate) fn live_actions_host_for_view(view: &AppView) -> Option<ActionsDialogHost> {
        if Self::is_builtin_list_actions_view(view) {
            return None;
        }

        Self::actions_host_for_view(view)
    }

    pub(crate) fn current_actions_host(&self) -> Option<ActionsDialogHost> {
        Self::live_actions_host_for_view(&self.current_view)
    }

    /// Canonical static resolver: map the current view to shared-actions
    /// identity for routing/focus contracts.
    ///
    /// This is not proof that the visible view may open the shared
    /// ActionsDialog. Live popup-open decisions must use
    /// `live_actions_host_for_view` or `current_actions_host`.
    pub(crate) fn actions_support_for_view(&self) -> ActionsSupport {
        match Self::actions_host_for_view(&self.current_view) {
            Some(host) => ActionsSupport::SharedDialog(host),
            None => ActionsSupport::None,
        }
    }

    /// Convenience: does the current view participate in the shared actions dialog?
    pub(crate) fn current_view_supports_shared_actions(&self) -> bool {
        self.current_actions_host().is_some()
    }

    pub(crate) fn make_actions_dialog_activation_callback(
        app_entity: Entity<Self>,
        host: ActionsDialogHost,
    ) -> crate::actions::ActivationCallback {
        std::sync::Arc::new(move |activation, window, cx| {
            let app_entity = app_entity.clone();
            window.defer(cx, move |window, cx| {
                app_entity.update(cx, |app, cx| {
                    app.handle_actions_dialog_activation(host, activation.clone(), window, cx);
                });
            });
        })
    }

    pub(crate) fn handle_actions_dialog_activation(
        &mut self,
        host: ActionsDialogHost,
        activation: crate::actions::ActionsDialogActivation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match activation {
            crate::actions::ActionsDialogActivation::DrillDownPushed { .. } => {
                // Agent & Model picker: provider ("Agent") rows are drill-only;
                // nothing persists until a model is chosen.
                crate::actions::notify_actions_window(cx);
                if let Some(dialog) = self.actions_dialog.as_ref() {
                    let (route_id, search_placeholder, route_depth, escape_hint) = {
                        let dialog_ref = dialog.read(cx);
                        (
                            dialog_ref.current_route_id().map(str::to_string),
                            dialog_ref.current_search_placeholder().map(str::to_string),
                            dialog_ref.route_depth(),
                            dialog_ref.route_hint_label(),
                        )
                    };
                    tracing::info!(
                        target: "script_kit::actions",
                        host = ?host,
                        route_id = ?route_id,
                        route_depth,
                        escape_hint,
                        search_placeholder = ?search_placeholder,
                        "actions_dialog_route_visible"
                    );
                }
            }
            crate::actions::ActionsDialogActivation::Executed {
                action_id,
                should_close,
            } => {
                let root_unified_context = if should_close
                    && matches!(host, ActionsDialogHost::MainList)
                    && crate::root_unified_result_actions::RootUnifiedResultAction::from_action_id(
                        &action_id,
                    )
                    .is_some()
                {
                    self.pending_root_unified_actions_subject.clone()
                } else {
                    None
                };
                let root_file_context = if root_unified_context.is_none()
                    && should_close
                    && matches!(host, ActionsDialogHost::MainList)
                    && crate::action_helpers::is_root_file_action_id(&action_id)
                {
                    self.pending_root_file_actions_file
                        .clone()
                        .or_else(|| self.selected_root_file_result_owned())
                } else {
                    None
                };
                if should_close {
                    self.close_actions_popup(host, window, cx);
                }
                if let Some(subject) = root_unified_context {
                    if crate::root_unified_result_actions::execute_root_unified_result_action(
                        self, &action_id, &subject, window, cx,
                    ) {
                        return;
                    }
                }
                if let Some(file) = root_file_context {
                    if self.execute_root_file_action(&action_id, &file, window, cx) {
                        return;
                    }
                }
                self.execute_action_for_actions_host(host, action_id, window, cx);
            }
            crate::actions::ActionsDialogActivation::Blocked { action_id, reason } => {
                tracing::info!(
                    target: "script_kit::actions",
                    action_id = %action_id,
                    reason_fingerprint = %crate::actions::ActionsDialog::devtools_text_fingerprint(&reason),
                    "actions_dialog_host_activation_blocked"
                );
            }
            crate::actions::ActionsDialogActivation::NoSelection => {}
        }
    }

    pub(crate) fn execute_actions_route_action(
        &mut self,
        host: ActionsDialogHost,
        action_id: String,
        should_close: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_actions_dialog_activation(
            host,
            crate::actions::ActionsDialogActivation::Executed {
                action_id,
                should_close,
            },
            window,
            cx,
        );
    }

    pub(crate) fn execute_action_for_actions_host(
        &mut self,
        host: ActionsDialogHost,
        action_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::actions",
            event = "actions_host_execute",
            host = ?host,
            action_id = %action_id,
        );

        // Run 12 Pass 9 — cmdk-safe-action-effects dispatch. When the dialog's
        // chosen row id is `menu_syntax:<original-id>`, route through
        // [[src/menu_syntax/action_effects.rs#apply_safe_effect]] BEFORE the
        // legacy host fallback so the launcher applies the effect (cancel /
        // setFilter / clipboard) instead of falling through to handle_action's
        // unknown-id path.
        if let Some(stripped) = action_id.strip_prefix("menu_syntax:") {
            self.dispatch_menu_syntax_safe_effect(host, stripped, window, cx);
            return;
        }

        // Day Page "Today" section rows (host_section in the shared dialog).
        if action_id.starts_with("day_page:")
            && self.execute_day_page_action(&action_id, window, cx)
        {
            return;
        }

        if matches!(self.current_view, AppView::DayPage { .. })
            && crate::ai::agent_prompt_handoff::is_prompt_action_id(&action_id)
        {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "day_page_prompt_action_blocked",
                action_id = %action_id,
                host = ?host,
                "Blocked stale prompt handoff action while Day Page owns the editor"
            );
            return;
        }

        match host {
            ActionsDialogHost::MainList => {
                // Agent & Model picker (Shift+Tab): persist the selected model
                // or agent to user preferences instead of dispatching it as an
                // ordinary launcher action. Gated so it only intercepts while
                // the picker owns the dialog. Action IDs are globally unique.
                if self.agent_model_picker_active {
                    // The model row carries the namespaced "provider/model" id;
                    // persisting it records both the agent and the model.
                    if let Some(model_id) =
                        crate::actions::agent_chat_switch_model_id_from_action(&action_id)
                    {
                        Self::persist_agent_model_picker_model(model_id);
                        self.refresh_agent_model_footer_labels();
                        return;
                    }
                }
                if let Some(subject) = self.pending_root_unified_actions_subject.clone() {
                    if crate::root_unified_result_actions::execute_root_unified_result_action(
                        self, &action_id, &subject, window, cx,
                    ) {
                        return;
                    }
                }
                if crate::root_unified_result_actions::RootUnifiedResultAction::from_action_id(
                    &action_id,
                )
                .is_some()
                {
                    tracing::warn!(
                        target: "script_kit::actions",
                        event = "root_unified_result_action_missing_subject",
                        action_id = %action_id,
                        "Root result action ignored because no pending subject was captured"
                    );
                    return;
                }
                if crate::action_helpers::is_root_file_action_id(&action_id) {
                    if let Some(file) = self
                        .pending_root_file_actions_file
                        .clone()
                        .or_else(|| self.selected_root_file_result_owned())
                    {
                        if self.execute_root_file_action(&action_id, &file, window, cx) {
                            return;
                        }
                    }
                }
                self.handle_action(action_id, window, cx);
            }
            ActionsDialogHost::ChatPrompt => self.execute_chat_action(&action_id, cx),
            ActionsDialogHost::ArgPrompt => {
                self.trigger_action_by_name(&action_id, cx);
            }
            ActionsDialogHost::WebcamPrompt => {
                let start = std::time::Instant::now();
                let dctx = crate::action_helpers::DispatchContext::for_builtin("builtin/webcam");
                let outcome = self.execute_webcam_action(&action_id, &dctx, cx);
                Self::log_builtin_outcome(
                    "builtin/webcam",
                    &dctx,
                    "webcam_action",
                    &outcome,
                    &start,
                );
            }
            ActionsDialogHost::AgentChatDetached => {
                let dispatched =
                    crate::ai::agent_chat::ui::chat_window::dispatch_action_to_detached(
                        &action_id, cx,
                    );
                tracing::info!(
                    target: "script_kit::actions",
                    event = "actions_host_execute_agent_chat_detached",
                    action_id = %action_id,
                    dispatched,
                );
            }
            ActionsDialogHost::ThemeChooser => {
                self.execute_theme_chooser_action(&action_id, window, cx);
            }
            ActionsDialogHost::FlowDesk => {
                self.execute_flow_desk_action(&action_id, window, cx);
            }
            _ => {
                self.handle_action(action_id, window, cx);
            }
        }
    }

    /// Run 12 Pass 9 — `cmdk-safe-action-effects` dispatch. Looks up the
    /// original `MenuSyntaxAction` by id, calls `apply_safe_effect` to
    /// resolve the effect, then applies it (Cancel → close, SetFilterText →
    /// `set_filter_text_immediate`, WriteClipboard → `cx.write_to_clipboard`).
    /// Always closes the popup at the end so the dispatch is atomic.
    pub(crate) fn dispatch_menu_syntax_safe_effect(
        &mut self,
        host: ActionsDialogHost,
        original_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::menu_syntax::{
            action_effects::{apply_safe_effect, ActionEffect},
            builtin_schema, current_menu_syntax_actions, MenuSyntaxActionState,
        };

        let raw = self.filter_text().to_string();
        let mode = &self.menu_syntax_mode;

        // Reconstruct the live state (mirror of Pass 7's actions_toggle.rs
        // pre-closure block). Borrow lifetimes constrain us to a flat match
        // here.
        let effect: ActionEffect = if let Some(invocation) = mode.capture_for(&raw) {
            let target = invocation.target.clone();
            let schema = builtin_schema(&target);
            let state = MenuSyntaxActionState::CaptureComposer {
                target: &target,
                payload: invocation,
                schema: schema.as_ref(),
            };
            let actions = current_menu_syntax_actions(&state);
            match actions.into_iter().find(|a| a.id == original_id) {
                Some(action) => apply_safe_effect(&state, &action.kind),
                None => ActionEffect::Unsupported,
            }
        } else if let Some(argv) = mode.command_for(&raw) {
            let state = MenuSyntaxActionState::CommandComposer {
                head: &argv.head,
                argv: &argv.argv,
            };
            let actions = current_menu_syntax_actions(&state);
            match actions.into_iter().find(|a| a.id == original_id) {
                Some(action) => apply_safe_effect(&state, &action.kind),
                None => ActionEffect::Unsupported,
            }
        } else if let Some(query) = mode.advanced_query_for(&raw) {
            let state = MenuSyntaxActionState::RefineQuery { query };
            let actions = current_menu_syntax_actions(&state);
            match actions.into_iter().find(|a| a.id == original_id) {
                Some(action) => apply_safe_effect(&state, &action.kind),
                None => ActionEffect::Unsupported,
            }
        } else {
            ActionEffect::Unsupported
        };

        tracing::info!(
            target: "script_kit::actions",
            event = "menu_syntax_safe_effect_dispatched",
            host = ?host,
            original_id = %original_id,
            effect = ?effect,
        );

        match effect {
            ActionEffect::Cancel => {
                // Close popup AND clear the composer filter so Cancel is
                // unambiguous. Mirrors the user expectation "Cancel without
                // saving".
                self.set_filter_text_immediate(String::new(), window, cx);
            }
            ActionEffect::SetFilterText { new_text } => {
                self.set_filter_text_immediate(new_text, window, cx);
            }
            ActionEffect::WriteClipboard { content } => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
            }
            ActionEffect::Unsupported => {
                // Quiet no-op — the dialog row should not have been clickable
                // for an unsupported effect, but if it slipped through, just
                // close cleanly.
            }
        }

        self.close_actions_popup(host, window, cx);
    }

    pub(crate) fn toggle_actions_for_host(
        &mut self,
        host: ActionsDialogHost,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        tracing::info!(
            target: "script_kit::actions",
            event = "actions_host_toggle_requested",
            host = ?host,
            show_actions_popup = self.show_actions_popup,
        );

        match host {
            ActionsDialogHost::MainList => {
                if let Some(result) = self.selected_main_list_search_result_owned() {
                    match crate::root_unified_result_actions::root_unified_action_owner_for_result(&result) {
                        crate::root_unified_result_actions::RootUnifiedResultActionOwner::RootSubject(subject) => {
                            self.toggle_root_unified_result_actions(*subject, window, cx);
                        }
                        crate::root_unified_result_actions::RootUnifiedResultActionOwner::ExistingScriptActions => {
                            if self.has_actions() {
                                self.toggle_actions(cx, window);
                            }
                        }
                        crate::root_unified_result_actions::RootUnifiedResultActionOwner::None => {
                            if self.has_actions() {
                                self.toggle_actions(cx, window);
                            }
                        }
                    }
                } else if self.has_actions() {
                    self.toggle_actions(cx, window);
                }
                true
            }
            ActionsDialogHost::FileSearch => {
                let selected = self.selected_file_search_result_owned();
                self.toggle_file_search_actions(
                    selected.as_ref().map(|(_, file)| file),
                    window,
                    cx,
                );
                true
            }
            ActionsDialogHost::ClipboardHistory => {
                if let Some(entry) = self.selected_clipboard_entry() {
                    self.toggle_clipboard_actions(entry, window, cx);
                    true
                } else if self.show_actions_popup || crate::actions::is_actions_window_open() {
                    self.toggle_actions(cx, window);
                    true
                } else {
                    false
                }
            }
            ActionsDialogHost::DictationHistory => {
                if let Some(entry) = self.selected_dictation_history_entry() {
                    self.toggle_dictation_history_actions(entry, window, cx);
                    true
                } else if self.show_actions_popup || crate::actions::is_actions_window_open() {
                    self.toggle_actions(cx, window);
                    true
                } else {
                    false
                }
            }
            ActionsDialogHost::Favorites => {
                if self.selected_favorite_id().is_some()
                    || self.show_actions_popup
                    || crate::actions::is_actions_window_open()
                {
                    self.toggle_favorites_actions(window, cx);
                    true
                } else {
                    false
                }
            }
            ActionsDialogHost::ThemeChooser => {
                self.toggle_theme_chooser_actions(window, cx);
                true
            }
            ActionsDialogHost::ArgPrompt => {
                self.toggle_arg_actions(cx, window);
                true
            }
            ActionsDialogHost::ChatPrompt => {
                self.toggle_chat_actions(cx, window);
                true
            }
            ActionsDialogHost::WebcamPrompt => {
                self.toggle_webcam_actions(cx, window);
                true
            }
            ActionsDialogHost::EmojiPicker
            | ActionsDialogHost::AppLauncher
            | ActionsDialogHost::AgentChat
            | ActionsDialogHost::AgentChatHistory
            | ActionsDialogHost::DivPrompt
            | ActionsDialogHost::EditorPrompt
            | ActionsDialogHost::TemplatePrompt
            | ActionsDialogHost::TermPrompt
            | ActionsDialogHost::FormPrompt => {
                self.toggle_actions(cx, window);
                true
            }
            ActionsDialogHost::FlowDesk => {
                self.toggle_flow_desk_actions(window, cx);
                true
            }
            ActionsDialogHost::BuiltinList => {
                if self.show_actions_popup || crate::actions::is_actions_window_open() {
                    self.close_actions_popup(host, window, cx);
                    true
                } else {
                    tracing::info!(
                        target: "script_kit::actions",
                        event = "actions_host_toggle_ignored_builtin_list",
                        host = ?host,
                        view = ?self.current_view,
                        "Ignored BuiltinList actions open because it has no selection-specific dialog"
                    );
                    false
                }
            }
            ActionsDialogHost::AgentChatDetached => {
                // The detached window has its own Cmd+K handler
                // (`toggle_detached_actions` in `src/ai/agent_chat/ui/chat_window.rs`).
                // The main view never advertises AgentChatDetached via
                // `current_actions_host()`, so this arm is defensive-only
                // and should never run in practice; leave the main view
                // untouched and report "not handled".
                false
            }
        }
    }

    fn main_list_actions_for_shortcut_routing(
        &mut self,
    ) -> (
        Option<crate::actions::ScriptInfo>,
        Vec<crate::actions::Action>,
    ) {
        let script_info = self.get_focused_script_info();
        let mut actions = Vec::new();

        if let Some(ref script) = script_info {
            if script.is_scriptlet {
                let focused_scriptlet = self.get_focused_scriptlet_with_actions();
                actions.extend(crate::actions::get_scriptlet_context_actions_with_custom(
                    script,
                    focused_scriptlet.as_ref(),
                ));
            } else {
                actions.extend(crate::actions::get_script_context_actions(script));
            }
        }

        actions.extend(crate::actions::get_global_actions());
        (script_info, actions)
    }

    pub(crate) fn sync_main_list_displayed_action_shortcut_keybindings(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }
        if self.menu_syntax_trigger_picker_owns_main_keyboard()
            || crate::menu_syntax::active_filter_head_owns_main_list(&self.filter_text)
        {
            return;
        }

        // Memo: this runs on every render frame, but the displayed-shortcut
        // specs only depend on the focused row (and the append-only
        // registered set). Rebuilding the full script-context + global
        // action vectors per frame was the arrow-key scroll render hotspot
        // (O(rows) cache clones + config load per frame).
        let sync_key = {
            let (grouped_items, flat_results) = self.get_grouped_results_cached();
            let selected_name = match grouped_items.get(self.selected_index) {
                Some(GroupedListItem::Item(idx)) => {
                    flat_results.get(*idx).map(|result| result.name())
                }
                _ => None,
            };
            format!(
                "{}|{}|{}|{}",
                self.selected_index,
                selected_name.unwrap_or(""),
                grouped_items.len(),
                self.filter_text
            )
        };
        if self.main_list_shortcut_sync_key.as_deref() == Some(sync_key.as_str()) {
            return;
        }
        self.main_list_shortcut_sync_key = Some(sync_key);

        let (_, actions) = self.main_list_actions_for_shortcut_routing();
        let filtered_actions: Vec<usize> = (0..actions.len()).collect();
        let specs = crate::actions::displayed_action_keybinding_specs(&actions, &filtered_actions);
        let mut bindings = Vec::new();
        let registered_before = self.registered_main_list_displayed_shortcuts.len();

        for spec in specs {
            if !self
                .registered_main_list_displayed_shortcuts
                .insert(spec.canonical.clone())
            {
                continue;
            }
            logging::log(
                "KEY_BIND",
                &format!(
                    "MAIN_LIST_SHORTCUT_BIND canonical={} gpui={} context=script_list action=MainListDisplayedActionShortcut",
                    spec.canonical, spec.gpui_keystroke
                ),
            );
            bindings.push(gpui::KeyBinding::new(
                &spec.gpui_keystroke,
                crate::actions::MainListDisplayedActionShortcut {
                    shortcut: spec.canonical,
                },
                Some("script_list"),
            ));
        }

        if !bindings.is_empty() {
            cx.bind_keys(bindings);
        }
        if self.registered_main_list_displayed_shortcuts.len() != registered_before {
            logging::log(
                "KEY_SETUP",
                &format!(
                    "MAIN_LIST_SHORTCUT_SYNC context=script_list current_view={} actions={} new_bindings={} registered_total={} env_shortcut_debug={}",
                    self.app_view_name(),
                    actions.len(),
                    self.registered_main_list_displayed_shortcuts.len() - registered_before,
                    self.registered_main_list_displayed_shortcuts.len(),
                    std::env::var("SCRIPT_KIT_SHORTCUT_DEBUG")
                        .unwrap_or_else(|_| "<unset>".to_string())
                ),
            );
        }
    }

    pub(crate) fn try_execute_main_list_action_shortcut_canonical(
        &mut self,
        canonical_shortcut: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.show_actions_popup || !matches!(self.current_view, AppView::ScriptList) {
            logging::log(
                "KEY_ROUTE",
                &format!(
                    "MAIN_LIST_DISPLAYED_SHORTCUT_BYPASS canonical={} reason=inactive_context popup={} view={}",
                    canonical_shortcut,
                    self.show_actions_popup,
                    self.app_view_name()
                ),
            );
            return false;
        }

        if self.menu_syntax_trigger_picker_owns_main_keyboard()
            || crate::menu_syntax::active_filter_head_owns_main_list(&self.filter_text)
        {
            if menu_syntax_displayed_shortcut_should_consume(canonical_shortcut) {
                if !self.menu_syntax_trigger_picker_owns_main_keyboard() {
                    let text = self.filter_text.clone();
                    self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);
                }
                if self.menu_syntax_trigger_picker_owns_main_keyboard()
                    && self.apply_menu_syntax_trigger_picker_intent(
                        crate::menu_syntax::InlinePickerKeyIntent::Accept,
                        window,
                        cx,
                    )
                {
                    logging::log(
                        "KEY_ROUTE",
                        "Displayed shortcut enter consumed by menu-syntax trigger picker",
                    );
                    return true;
                }
            }
            logging::log(
                "KEY_ROUTE",
                &format!(
                    "Displayed shortcut {} passed through: menu-syntax owns main list but input owns editing keys",
                    canonical_shortcut
                ),
            );
            return false;
        }

        let (script_info, actions) = self.main_list_actions_for_shortcut_routing();
        let filtered_actions: Vec<usize> = (0..actions.len()).collect();
        let Some(action_id) = crate::actions::matching_action_id_for_canonical_shortcut(
            &actions,
            &filtered_actions,
            canonical_shortcut,
        ) else {
            logging::log(
                "KEY_ROUTE",
                &format!(
                    "Displayed shortcut route miss canonical={} actions={} focused={}",
                    canonical_shortcut,
                    actions.len(),
                    script_info
                        .as_ref()
                        .map(|script| format!(
                            "{} path={} shortcut={:?} alias={:?}",
                            script.name, script.path, script.shortcut, script.alias
                        ))
                        .unwrap_or_else(|| "<none>".to_string())
                ),
            );
            return false;
        };

        logging::log(
            "KEY_ROUTE",
            &format!(
                "Displayed shortcut {} -> {} via Action.shortcut metadata",
                canonical_shortcut, action_id
            ),
        );
        self.handle_action(action_id, window, cx);
        true
    }

    pub(crate) fn try_execute_main_list_action_shortcut_from_display(
        &mut self,
        key: &str,
        modifiers: &gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // The global actions interceptor runs before AgentChatView's focused
        // key handler. When the embedded composer's Spine owns the list, bare
        // Enter must reach that existing acceptance path before this helper's
        // non-ScriptList inactive-context bypass returns control to a route
        // that never delivers the key to the child view.
        if !self.show_actions_popup
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
            && crate::ui_foundation::is_key_enter(key)
        {
            if let AppView::AgentChatView { entity } = &self.current_view {
                let entity = entity.clone();
                let spine_consumed = entity.update(cx, |chat, cx| {
                    chat.agent_chat_spine_owns_list()
                        && chat.accept_agent_chat_spine_projection_row(window, cx)
                });
                if spine_consumed {
                    logging::log(
                        "KEY_ROUTE",
                        "Agent Chat Spine accepted Enter before main-list displayed shortcut routing",
                    );
                    return true;
                }
            }
        }

        // Bare navigation keys can never be displayed action shortcuts: the
        // arrow/home-end interceptors consume them before the actions
        // interceptor on the live keyboard path. Skip the O(actions) routing
        // (full context-action rebuild + per-action logging) on the key-repeat
        // scroll hot path instead of rebuilding the action list per keypress.
        if !modifiers.platform && !modifiers.control && !modifiers.alt && !modifiers.shift {
            let is_bare_navigation_key = crate::ui_foundation::is_key_up(key)
                || crate::ui_foundation::is_key_down(key)
                || crate::ui_foundation::is_key_left(key)
                || crate::ui_foundation::is_key_right(key)
                || matches!(key, "home" | "end" | "pageup" | "pagedown");
            if is_bare_navigation_key {
                return false;
            }
        }

        if self.show_actions_popup || !matches!(self.current_view, AppView::ScriptList) {
            logging::log(
                "KEY_ROUTE",
                &format!(
                    "MAIN_LIST_DISPLAYED_SHORTCUT_KEY_BYPASS key={} shortcut={} reason=inactive_context popup={} view={}",
                    key,
                    crate::shortcuts::keystroke_to_shortcut(key, modifiers),
                    self.show_actions_popup,
                    self.app_view_name()
                ),
            );
            return false;
        }

        if self.menu_syntax_trigger_picker_owns_main_keyboard()
            || crate::menu_syntax::active_filter_head_owns_main_list(&self.filter_text)
        {
            let canonical_keystroke = crate::shortcuts::keystroke_to_shortcut(key, modifiers);
            if menu_syntax_displayed_shortcut_should_consume(&canonical_keystroke) {
                if !self.menu_syntax_trigger_picker_owns_main_keyboard() {
                    let text = self.filter_text.clone();
                    self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);
                }
                if self.menu_syntax_trigger_picker_owns_main_keyboard()
                    && self.apply_menu_syntax_trigger_picker_intent(
                        crate::menu_syntax::InlinePickerKeyIntent::Accept,
                        window,
                        cx,
                    )
                {
                    logging::log(
                        "KEY_ROUTE",
                        "Shortcut route enter consumed by menu-syntax trigger picker",
                    );
                    return true;
                }
            }
            logging::log(
                "KEY_ROUTE",
                &format!(
                    "Shortcut route passed through key={}: menu-syntax owns main list but input owns editing keys",
                    canonical_keystroke
                ),
            );
            return false;
        }

        let (script_info, actions) = self.main_list_actions_for_shortcut_routing();
        let canonical_keystroke = crate::shortcuts::keystroke_to_shortcut(key, modifiers);
        let shortcut_bindings: Vec<_> = actions
            .iter()
            .filter_map(|action| {
                let shortcut = action.shortcut.as_deref()?;
                let canonical = crate::components::hint_strip::canonical_shortcut_hint(shortcut);
                Some((
                    action.id.as_str(),
                    action.title.as_str(),
                    shortcut,
                    canonical,
                ))
            })
            .collect();
        logging::log(
            "KEY_ROUTE",
            &format!(
                "Shortcut route attempt key={} actions={} displayed_shortcuts={} view={} focused={}",
                canonical_keystroke,
                actions.len(),
                shortcut_bindings.len(),
                self.app_view_name(),
                script_info
                    .as_ref()
                    .map(|script| format!(
                        "{} path={} shortcut={:?} alias={:?}",
                        script.name, script.path, script.shortcut, script.alias
                    ))
                    .unwrap_or_else(|| "<none>".to_string())
            ),
        );
        for (action_id, title, shortcut, canonical) in &shortcut_bindings {
            logging::log(
                "KEY_BIND",
                &format!(
                    "Shortcut binding action_id={} title={:?} display={} canonical={}",
                    action_id, title, shortcut, canonical
                ),
            );
        }

        if !self.try_execute_main_list_action_shortcut_canonical(&canonical_keystroke, window, cx) {
            logging::log(
                "KEY_ROUTE",
                &format!(
                    "Shortcut route miss key={} displayed_shortcuts={}",
                    canonical_keystroke,
                    shortcut_bindings.len()
                ),
            );
            return false;
        }
        true
    }

    pub(crate) fn route_key_to_actions_dialog(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        modifiers: &gpui::Modifiers,
        host: ActionsDialogHost,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ActionsRoute {
        // Not open - let caller handle the key. Detached actions popups keep
        // parent focus, so the parent router must stay active while their
        // native child window is open even if the inline flag has already been
        // cleared. Only a MAIN-hosted detached popup counts: a popup hosted by
        // a secondary window (Notes, etc.) belongs to that window's router and
        // must not pull this one into swallowing keys against the wrong dialog.
        if !self.show_actions_popup && !crate::actions::is_actions_window_open_for_main() {
            return ActionsRoute::NotHandled;
        }

        // A main-window CommandBar hosted by a child surface (Day Page Cmd+P)
        // uses the shared detached actions window without storing its dialog
        // in ScriptListApp.actions_dialog. Let that popup/window or the child
        // view handle keys instead of swallowing them through the stale
        // shared-actions route.
        let Some(ref dialog) = self.actions_dialog else {
            return ActionsRoute::NotHandled;
        };

        // Use allocation-free key helpers from ui_foundation
        use crate::ui_foundation::{
            is_key_backspace, is_key_down, is_key_enter, is_key_escape, is_key_up, printable_char,
        };

        // Cmd+K toggles the popup closed through the shared close path
        if modifiers.platform
            && !modifiers.shift
            && !modifiers.control
            && !modifiers.alt
            && key.eq_ignore_ascii_case("k")
        {
            self.close_actions_popup(host, window, cx);
            return ActionsRoute::Handled;
        }

        if is_key_up(key) {
            dialog.update(cx, |d, cx| d.move_up(cx));
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        if is_key_down(key) {
            dialog.update(cx, |d, cx| d.move_down(cx));
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        let is_home = key.eq_ignore_ascii_case("home");
        let is_end = key.eq_ignore_ascii_case("end");
        let is_page_up = key.eq_ignore_ascii_case("pageup");
        let is_page_down = key.eq_ignore_ascii_case("pagedown");
        const ACTIONS_PAGE_JUMP: usize = 8;

        if is_home || is_end || is_page_up || is_page_down {
            dialog.update(cx, |d, cx| {
                if d.grouped_items.is_empty() {
                    return;
                }

                if is_home || is_page_up {
                    let steps = if is_home {
                        d.grouped_items.len()
                    } else {
                        ACTIONS_PAGE_JUMP
                    };
                    for _ in 0..steps {
                        let previous = d.selected_index;
                        d.move_up(cx);
                        if d.selected_index == previous {
                            break;
                        }
                    }
                    return;
                }

                let steps = if is_end {
                    d.grouped_items.len()
                } else {
                    ACTIONS_PAGE_JUMP
                };
                for _ in 0..steps {
                    let previous = d.selected_index;
                    d.move_down(cx);
                    if d.selected_index == previous {
                        break;
                    }
                }
            });
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        // Cmd+Enter: send selected action to Agent Chat Chat as a canonical target chip.
        // Day/Today is editor-owned; its @ context route must stay on the
        // main-menu round trip instead of reopening prompt-builder handoff UI.
        // Must precede the generic Enter branch to avoid being swallowed.
        if modifiers.platform
            && !modifiers.shift
            && !modifiers.control
            && !modifiers.alt
            && is_key_enter(key)
        {
            if matches!(self.current_view, AppView::DayPage { .. }) {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "tab_ai_actions_dialog_cmd_enter_ignored_day_page",
                    host = ?host,
                    "Ignored Actions Cmd+Enter handoff while Day Page owns the editor"
                );
                return ActionsRoute::Handled;
            }
            if let Some(action) = dialog.read(cx).get_selected_action().cloned() {
                let host_label = format!("{:?}", host);
                let target = crate::ai::build_action_target_for_ai(&action, &host_label);
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "tab_ai_actions_dialog_cmd_enter",
                    host = %host_label,
                    action_id = %action.id,
                    semantic_id = %target.semantic_id,
                );
                self.close_actions_popup(host, window, cx);
                self.open_tab_ai_agent_chat_with_explicit_target_preserving_return(target, cx);
                return ActionsRoute::Handled;
            }
        }

        if is_key_enter(key) {
            match dialog.update(cx, |d, cx| d.activate_selected(cx)) {
                crate::actions::ActionsDialogActivation::DrillDownPushed { .. } => {
                    // Agent & Model picker: a provider ("Agent") row is drill-only
                    // — nothing persists until a model is chosen, because the
                    // provider is encoded in the namespaced model id.
                    crate::actions::notify_actions_window(cx);
                    let (route_id, search_placeholder, route_depth, escape_hint) = {
                        let dialog_ref = dialog.read(cx);
                        (
                            dialog_ref.current_route_id().map(str::to_string),
                            dialog_ref.current_search_placeholder().map(str::to_string),
                            dialog_ref.route_depth(),
                            dialog_ref.route_hint_label(),
                        )
                    };
                    tracing::info!(
                        target: "script_kit::actions",
                        host = ?host,
                        route_id = ?route_id,
                        route_depth,
                        escape_hint,
                        search_placeholder = ?search_placeholder,
                        "actions_dialog_route_visible"
                    );
                    return ActionsRoute::Handled;
                }
                crate::actions::ActionsDialogActivation::Executed {
                    action_id,
                    should_close,
                } => {
                    logging::log(
                        "ACTIONS",
                        &format!(
                            "Actions dialog executing action: {} (close={}, host={:?})",
                            action_id, should_close, host
                        ),
                    );
                    return ActionsRoute::Execute {
                        action_id,
                        should_close,
                    };
                }
                crate::actions::ActionsDialogActivation::Blocked { .. }
                | crate::actions::ActionsDialogActivation::NoSelection => {
                    return ActionsRoute::Handled;
                }
            }
        }

        if is_key_escape(key) {
            let outcome = dialog.update(cx, |d, cx| d.handle_escape(cx));
            match outcome {
                crate::actions::ActionsDialogEscapeOutcome::PoppedRoute => {
                    dialog.update(cx, |d, cx| {
                        d.sync_search_input_from_model(window, cx);
                    });
                    crate::actions::notify_actions_window(cx);
                    let (route_id, search_placeholder, route_depth, escape_hint) = {
                        let dialog_ref = dialog.read(cx);
                        (
                            dialog_ref.current_route_id().map(str::to_string),
                            dialog_ref.current_search_placeholder().map(str::to_string),
                            dialog_ref.route_depth(),
                            dialog_ref.route_hint_label(),
                        )
                    };
                    tracing::info!(
                        target: "script_kit::actions",
                        host = ?host,
                        route_id = ?route_id,
                        route_depth,
                        escape_hint,
                        search_placeholder = ?search_placeholder,
                        "actions_dialog_route_visible"
                    );
                }
                crate::actions::ActionsDialogEscapeOutcome::CloseDialog => {
                    self.close_actions_popup(host, window, cx);
                }
            }
            return ActionsRoute::Handled;
        }

        if is_key_backspace(key) {
            // Option+Backspace deletes a word, like the main search input.
            // Cmd+Backspace is intentionally NOT a clear-search binding: hosts
            // bind it to destructive actions (e.g. Delete Note), so it falls
            // through to shortcut matching below.
            if modifiers.alt && !modifiers.platform && !modifiers.control {
                dialog.update(cx, |d, cx| {
                    d.delete_previous_search_word(window, cx);
                });
                crate::actions::notify_actions_window(cx);
                return ActionsRoute::Handled;
            }
            if !modifiers.platform && !modifiers.control {
                dialog.update(cx, |d, cx| {
                    d.backspace_search_input(window, cx);
                });
                crate::actions::notify_actions_window(cx);
                return ActionsRoute::Handled;
            }
        }

        if key.eq_ignore_ascii_case("left")
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
        {
            dialog.update(cx, |d, cx| {
                d.move_search_cursor_left(modifiers.shift, window, cx);
            });
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }
        if key.eq_ignore_ascii_case("right")
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
        {
            dialog.update(cx, |d, cx| {
                d.move_search_cursor_right(modifiers.shift, window, cx);
            });
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        // Check for printable character input (only when no modifiers are held).
        // The route chooses the character, but InputState owns insertion and selection.
        if !modifiers.platform && !modifiers.control && !modifiers.alt {
            if let Some(ch) = printable_char(key_char) {
                dialog.update(cx, |d, cx| {
                    d.insert_search_text(ch.to_string(), window, cx);
                });
                crate::actions::notify_actions_window(cx);
                return ActionsRoute::Handled;
            }
        }

        // Check if keystroke matches any action shortcut in the dialog
        // This allows Cmd+E, Cmd+L, etc. to execute the corresponding action
        let keystroke_shortcut = shortcuts::keystroke_to_shortcut(key, modifiers);
        let matched_action_id = {
            let dialog_ref = dialog.read(cx);
            crate::actions::matching_filtered_action_id_for_keystroke(
                &dialog_ref.actions,
                &dialog_ref.filtered_actions,
                key,
                modifiers,
            )
        };

        if let Some(action_id) = matched_action_id {
            logging::log(
                "ACTIONS",
                &format!(
                    "Actions dialog shortcut matched: {} -> {} (host={:?})",
                    keystroke_shortcut, action_id, host
                ),
            );

            match dialog.update(cx, |d, cx| d.activate_action_id(action_id.clone(), cx)) {
                crate::actions::ActionsDialogActivation::DrillDownPushed { .. } => {
                    dialog.update(cx, |d, cx| {
                        d.sync_search_input_from_model(window, cx);
                    });
                    crate::actions::notify_actions_window(cx);
                    let (route_id, search_placeholder, route_depth, escape_hint) = {
                        let dialog_ref = dialog.read(cx);
                        (
                            dialog_ref.current_route_id().map(str::to_string),
                            dialog_ref.current_search_placeholder().map(str::to_string),
                            dialog_ref.route_depth(),
                            dialog_ref.route_hint_label(),
                        )
                    };
                    tracing::info!(
                        target: "script_kit::actions",
                        host = ?host,
                        route_id = ?route_id,
                        route_depth,
                        escape_hint,
                        search_placeholder = ?search_placeholder,
                        "actions_dialog_route_visible"
                    );
                    return ActionsRoute::Handled;
                }
                crate::actions::ActionsDialogActivation::Executed {
                    action_id,
                    should_close,
                } => {
                    return ActionsRoute::Execute {
                        action_id,
                        should_close,
                    };
                }
                crate::actions::ActionsDialogActivation::Blocked { .. }
                | crate::actions::ActionsDialogActivation::NoSelection => {
                    return ActionsRoute::Handled;
                }
            }
        }

        // Cmd+V pastes into the popup search, like the main search input.
        // Runs AFTER shortcut matching so a host action that binds ⌘V keeps
        // its row shortcut.
        if modifiers.platform
            && !modifiers.shift
            && !modifiers.control
            && !modifiers.alt
            && key.eq_ignore_ascii_case("v")
        {
            dialog.update(cx, |d, cx| {
                d.paste_search_input(window, cx);
            });
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        if modifiers.platform
            && !modifiers.shift
            && !modifiers.control
            && !modifiers.alt
            && key.eq_ignore_ascii_case("a")
        {
            dialog.update(cx, |d, cx| {
                d.select_all_search_input(window, cx);
            });
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        if modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && key.eq_ignore_ascii_case("z")
        {
            dialog.update(cx, |d, cx| {
                if modifiers.shift {
                    d.redo_search_input(window, cx);
                } else {
                    d.undo_search_input(window, cx);
                }
            });
            crate::actions::notify_actions_window(cx);
            return ActionsRoute::Handled;
        }

        // Modal behavior: swallow all other keys while popup is open
        ActionsRoute::Handled
    }

    /// Convert a display shortcut (⌘⇧E) to normalized form (cmd+shift+e)
    pub(crate) fn normalize_display_shortcut(hint: &str) -> String {
        crate::components::hint_strip::canonical_shortcut_hint(hint)
    }

    fn should_preserve_main_filter_while_actions_open(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
            || self.current_view_uses_shared_filter_input()
    }

    pub(crate) fn mark_filter_resync_after_actions_if_needed(&mut self) {
        if !self.should_preserve_main_filter_while_actions_open() {
            return;
        }

        self.pending_filter_sync = true;
        logging::log(
            "ACTIONS",
            &format!(
                "ACTIONS_FILTER_RESYNC marked pending (show_actions_popup={}, filter_text='{}')",
                self.show_actions_popup, self.filter_text
            ),
        );
    }

    pub(crate) fn resync_filter_input_after_actions_if_needed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mark_filter_resync_after_actions_if_needed();
        self.sync_filter_input_if_needed(window, cx);
    }

    pub(crate) fn request_focus_restore_for_actions_host(&mut self, host: ActionsDialogHost) {
        let request =
            Self::focus_restore_request_for_actions_host(host, self.current_view.surface_kind());

        self.focus_coordinator.request(request);
        self.sync_coordinator_to_legacy();
    }

    /// Pure mapping from (actions host, current surface) to the focus target
    /// restored when the shared actions dialog closes.
    ///
    /// This is the single owner of the "Escape from Cmd+K returns focus to the
    /// host's input" contract; keep every host mapped here and unit-tested in
    /// `actions_host_focus_restore_tests` rather than special-casing at call
    /// sites.
    fn focus_restore_request_for_actions_host(
        host: ActionsDialogHost,
        surface: SurfaceKind,
    ) -> crate::focus_coordinator::FocusRequest {
        use crate::focus_coordinator::FocusRequest;

        match host {
            ActionsDialogHost::ArgPrompt => FocusRequest::arg_prompt(),
            ActionsDialogHost::ChatPrompt => FocusRequest::chat_prompt(),
            ActionsDialogHost::EditorPrompt => FocusRequest::editor_prompt(),
            ActionsDialogHost::TemplatePrompt => FocusRequest::template_prompt(),
            ActionsDialogHost::FormPrompt => FocusRequest::form_prompt(),
            ActionsDialogHost::DivPrompt => FocusRequest::div_prompt(),
            ActionsDialogHost::TermPrompt => FocusRequest::term_prompt(),
            ActionsDialogHost::WebcamPrompt => FocusRequest::div_prompt(),
            ActionsDialogHost::AgentChat => FocusRequest::agent_chat(),
            ActionsDialogHost::MainList if surface == SurfaceKind::DayPage => {
                FocusRequest::editor_prompt()
            }
            // FlowDesk covers both the desk list and an open flow session.
            // Flow sessions compose in the shared MAIN input (see
            // `current_view_uses_shared_filter_input`), so both restore to the
            // main filter — there is no session PTY to hand focus to anymore.
            ActionsDialogHost::MainList
            | ActionsDialogHost::FileSearch
            | ActionsDialogHost::ClipboardHistory
            | ActionsDialogHost::DictationHistory
            | ActionsDialogHost::Favorites
            | ActionsDialogHost::ThemeChooser
            | ActionsDialogHost::EmojiPicker
            | ActionsDialogHost::AppLauncher
            | ActionsDialogHost::BuiltinList
            | ActionsDialogHost::FlowDesk
            | ActionsDialogHost::AgentChatHistory
            | ActionsDialogHost::AgentChatDetached => FocusRequest::main_filter(),
        }
    }

    /// Check if the actions popup was closed very recently (within 300ms).
    ///
    /// This guards against a race where clicking the footer ⌘K button causes
    /// the actions window's activation observer to close the dialog (deferred)
    /// before the click handler fires `toggle_actions`. Without this debounce
    /// the toggle would see the dialog as closed and immediately reopen it.
    pub(crate) fn was_actions_recently_closed(&self) -> bool {
        const ACTIONS_CLOSE_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(300);
        self.actions_closed_at
            .map(|t| t.elapsed() < ACTIONS_CLOSE_DEBOUNCE)
            .unwrap_or(false)
    }

    /// Mark the shared actions popup as opening.
    ///
    /// This is the mutation owner for the popup-open flag and debounce reset.
    /// Keep the `Clear debounce on open` phrase here so source audits protect
    /// the footer-Cmd+K race guard without requiring every call site to repeat
    /// the raw field writes.
    pub(crate) fn mark_actions_popup_opening(&mut self) {
        self.show_actions_popup = true;
        self.actions_closed_at = None; // Clear debounce on open
    }

    /// Clear shared actions popup state without recording a recent-close debounce.
    ///
    /// Use this for route changes, resets, and stale-overlay cleanup where the
    /// UI is not handling a user close gesture that should debounce footer Cmd+K.
    pub(crate) fn clear_actions_popup_state(&mut self) {
        self.show_actions_popup = false;
        self.actions_dialog = None;
    }

    pub(crate) fn clear_actions_context_for_host(&mut self, host: ActionsDialogHost) {
        if matches!(host, ActionsDialogHost::MainList) {
            self.pending_root_file_actions_file = None;
            self.pending_root_unified_actions_subject = None;
        }
    }

    /// Mark the shared actions popup as closed.
    ///
    /// This is the mutation owner for the popup-open flag and close timestamp.
    /// Keep the `Record debounce on close` phrase here so close paths share the
    /// same 300ms recent-close behavior.
    pub(crate) fn mark_actions_popup_closed(&mut self) {
        self.clear_actions_popup_state();
        self.actions_closed_at = Some(std::time::Instant::now()); // Record debounce on close
    }

    /// Close the actions popup and restore focus based on host type.
    ///
    /// This centralizes close behavior, ensuring cx.notify() is always called
    /// and focus is correctly restored based on which prompt hosted the dialog.
    pub(crate) fn close_actions_popup(
        &mut self,
        host: ActionsDialogHost,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let closing_from_actions_window = crate::actions::is_actions_window(window);
        if closing_from_actions_window {
            crate::platform::activate_main_window();
        }

        let overlay_depth_before_on_close = self.focus_coordinator.overlay_depth();
        let on_close_callback = self
            .actions_dialog
            .as_ref()
            .and_then(|dialog| dialog.read(cx).on_close.clone());

        if let Some(on_close) = on_close_callback {
            logging::log(
                "ACTIONS",
                &format!(
                    "ACTIONS_CLOSE_POPUP invoking on_close callback (host={:?}, overlay_depth_before={})",
                    host, overlay_depth_before_on_close
                ),
            );
            on_close(cx);
        }

        let overlay_depth_after_on_close = self.focus_coordinator.overlay_depth();
        let callback_restored_focus = overlay_depth_after_on_close < overlay_depth_before_on_close;

        self.mark_actions_popup_closed();
        self.resync_filter_input_after_actions_if_needed(window, cx);

        // Close the separate actions window if open
        // This ensures consistent behavior whether closing via Cmd+K, Escape, backdrop click,
        // or any other close mechanism
        if is_actions_window_open() {
            cx.spawn(async move |_this, cx| {
                cx.update(|cx| {
                    close_actions_window(cx);
                });
            })
            .detach();
        }

        // Use coordinator to pop overlay and restore previous focus.
        // Skip pop when the dialog callback already restored focus to avoid double-pop.
        if !callback_restored_focus {
            self.pop_focus_overlay(cx);
        }

        self.request_focus_restore_for_actions_host(host);
        self.clear_actions_context_for_host(host);

        // Apply restored focus immediately rather than deferring to next render.
        // pop_focus_overlay sets pending_focus to the saved target (e.g. ChatPrompt).
        // Applying it now avoids race conditions with the async window close.
        if closing_from_actions_window {
            logging::log(
                "FOCUS",
                "Actions popup closed from actions window; pending focus will apply on the main window",
            );
        } else if !self.apply_pending_focus(window, cx) {
            // Fallback: focus app root if no pending focus was applied
            window.focus(&self.focus_handle, cx);
        }
        logging::log(
            "FOCUS",
            &format!(
                "Actions popup closed (host={:?}), focus restored via coordinator",
                host
            ),
        );
        cx.notify();

        // Check for a pending Agent Chat handoff target enqueued by the detached
        // actions window's Cmd+Enter handler. The slot is only populated
        // when a secondary surface explicitly requested the handoff.
        if let Some(target) = crate::ai::take_pending_explicit_agent_chat_target() {
            if matches!(self.current_view, AppView::DayPage { .. }) {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "day_page_pending_agent_chat_target_dropped",
                    item_source = %target.source,
                    semantic_id = %target.semantic_id,
                    "Dropped pending Agent Chat target while Day Page owns the editor"
                );
                return;
            }
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_pending_agent_chat_target_picked_up",
                item_source = %target.source,
                semantic_id = %target.semantic_id,
                current_view = ?self.current_view,
            );
            self.open_tab_ai_agent_chat_with_explicit_target_preserving_return(target, cx);
        }
    }

    pub(crate) fn close_actions_popup_for_current_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(host) = self.current_actions_host() {
            tracing::info!(
                target: "script_kit::actions",
                event = "actions_close_current_view",
                host = ?host,
            );
            self.close_actions_popup(host, window, cx);
            return;
        }

        tracing::warn!(
            target: "script_kit::actions",
            event = "actions_close_without_live_host",
            view = ?self.current_view,
        );

        self.mark_actions_popup_closed();
        self.mark_filter_resync_after_actions_if_needed();
        if is_actions_window_open() {
            close_actions_window(cx);
        }
        self.pop_focus_overlay(cx);
        cx.notify();
    }
}

#[cfg(test)]
include!("actions_dialog_tests.rs");

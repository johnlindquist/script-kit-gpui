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
    ) -> std::sync::Arc<
        dyn Fn(crate::actions::ActionsDialogActivation, &mut Window, &mut gpui::App) + Send + Sync,
    > {
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
                    crate::actions::resize_actions_window(cx, dialog);
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
            MenuSyntaxActionState,
            action_effects::{ActionEffect, apply_safe_effect},
            builtin_schema, current_menu_syntax_actions,
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
                            self.toggle_root_unified_result_actions(subject, window, cx);
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
                    crate::actions::resize_actions_window(cx, dialog);
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
                    crate::actions::resize_actions_window(cx, dialog);
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
                crate::actions::resize_actions_window(cx, dialog);
                return ActionsRoute::Handled;
            }
            if !modifiers.platform && !modifiers.control {
                dialog.update(cx, |d, cx| {
                    d.backspace_search_input(window, cx);
                });
                crate::actions::notify_actions_window(cx);
                crate::actions::resize_actions_window(cx, dialog);
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
                crate::actions::resize_actions_window(cx, dialog);
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
                    crate::actions::resize_actions_window(cx, dialog);
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
            crate::actions::resize_actions_window(cx, dialog);
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
            crate::actions::resize_actions_window(cx, dialog);
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
mod close_actions_popup_regression_tests {
    use std::fs;

    #[test]
    fn test_close_actions_popup_invokes_on_close_before_clearing_dialog_state() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let on_close_pos = close_fn
            .find("on_close(cx);")
            .expect("close_actions_popup must invoke on_close callback");
        let clear_dialog_pos = close_fn
            .find("self.actions_dialog = None;")
            .expect("close_actions_popup must clear actions_dialog state");

        assert!(
            on_close_pos < clear_dialog_pos,
            "close_actions_popup must invoke on_close before clearing actions_dialog state"
        );
    }

    #[test]
    fn test_close_actions_popup_resyncs_filter_input_after_clearing_dialog_state() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let clear_dialog_pos = close_fn
            .find("self.mark_actions_popup_closed();")
            .expect("close_actions_popup must clear actions_dialog state");
        let resync_pos = close_fn
            .find("self.resync_filter_input_after_actions_if_needed(window, cx);")
            .expect("close_actions_popup must resync canonical filter input state");

        assert!(
            clear_dialog_pos < resync_pos,
            "close_actions_popup must resync filter input after clearing actions dialog state"
        );
    }

    #[test]
    fn test_close_actions_popup_notifies_after_focus_restore_paths() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let fallback_focus_pos = close_fn
            .find("window.focus(&self.focus_handle, cx);")
            .expect("close_actions_popup must keep fallback root focus");
        let notify_pos = close_fn
            .find("cx.notify();")
            .expect("close_actions_popup must notify after closing popup");

        assert!(
            fallback_focus_pos < notify_pos,
            "close_actions_popup must notify after focus restore paths complete"
        );
    }

    #[test]
    fn test_close_actions_popup_restores_host_focus_before_apply_pending_focus() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let host_restore_pos = close_fn
            .find("self.request_focus_restore_for_actions_host(host);")
            .expect("close_actions_popup must request host-specific focus restore");
        let apply_pending_pos = close_fn
            .find("self.apply_pending_focus(window, cx)")
            .expect("close_actions_popup must apply pending focus after host restore");

        assert!(
            host_restore_pos < apply_pending_pos,
            "close_actions_popup should request host-specific focus before applying pending focus"
        );
    }

    #[test]
    fn test_close_actions_popup_defers_focus_apply_from_actions_window() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let detect_pos = close_fn
            .find("let closing_from_actions_window = crate::actions::is_actions_window(window);")
            .expect("close_actions_popup must detect actions-window-originated closes");
        let activate_pos = close_fn
            .find("crate::platform::activate_main_window();")
            .expect("actions-window-originated close must activate the main window");
        let host_restore_pos = close_fn
            .find("self.request_focus_restore_for_actions_host(host);")
            .expect("host focus restore request missing");
        // The first `if closing_from_actions_window` is the early main-window
        // activation guard; the deferred-apply branch is the one after the
        // host focus restore, so search from there.
        let deferred_branch_pos = close_fn[host_restore_pos..]
            .find("if closing_from_actions_window")
            .map(|pos| host_restore_pos + pos)
            .expect("actions-window-originated close must skip child-window focus apply");
        let apply_pending_pos = close_fn
            .find("self.apply_pending_focus(window, cx)")
            .expect("non-actions-window close path must still apply focus immediately");

        assert!(
            detect_pos < activate_pos && activate_pos < host_restore_pos,
            "close_actions_popup should activate the main window before queuing host focus restore"
        );
        assert!(
            host_restore_pos < deferred_branch_pos && deferred_branch_pos < apply_pending_pos,
            "actions-window closes must defer focus application instead of focusing the popup window"
        );
    }

    #[test]
    fn day_page_command_bar_keys_are_not_swallowed_by_missing_shared_dialog() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let helper_start = source
            .find("fn route_key_to_actions_dialog")
            .expect("route_key_to_actions_dialog function not found");
        let helper_fn = &source[helper_start..];

        assert!(
            helper_fn.contains("let Some(ref dialog) = self.actions_dialog else")
                && helper_fn.contains("return ActionsRoute::NotHandled;"),
            "route_key_to_actions_dialog must not swallow keys when a child CommandBar owns the detached actions window"
        );
    }
}

#[cfg(test)]
mod actions_host_focus_restore_tests {
    use super::*;
    use crate::focus_coordinator::FocusRequest;

    fn restore(host: ActionsDialogHost, surface: SurfaceKind) -> FocusRequest {
        ScriptListApp::focus_restore_request_for_actions_host(host, surface)
    }

    /// Closing Cmd+K over a flow session must hand focus back to the shared
    /// MAIN input — the flow composer IS the main filter input
    /// (`current_view_uses_shared_filter_input`). A `term_prompt()` restore
    /// here silently no-ops (FlowSessionView has no terminal entity) and
    /// strands focus, which is exactly the regression this locks out.
    #[test]
    fn flow_session_actions_restore_main_input_focus() {
        assert_eq!(
            restore(ActionsDialogHost::FlowDesk, SurfaceKind::FlowSession),
            FocusRequest::main_filter(),
        );
        assert_eq!(
            restore(ActionsDialogHost::FlowDesk, SurfaceKind::FlowUx),
            FocusRequest::main_filter(),
        );
    }

    /// Day Page Cmd+K close must restore focus to the Day editor, not the
    /// main filter.
    #[test]
    fn day_page_mainlist_actions_restore_editor_focus() {
        assert_eq!(
            restore(ActionsDialogHost::MainList, SurfaceKind::DayPage),
            FocusRequest::editor_prompt(),
        );
        assert_eq!(
            restore(ActionsDialogHost::MainList, SurfaceKind::ScriptList),
            FocusRequest::main_filter(),
        );
    }

    /// Every prompt host restores focus to its own input when the dialog
    /// closes; list-like hosts restore the shared main filter.
    #[test]
    fn prompt_hosts_restore_their_own_inputs() {
        let cases = [
            (ActionsDialogHost::ArgPrompt, FocusRequest::arg_prompt()),
            (ActionsDialogHost::ChatPrompt, FocusRequest::chat_prompt()),
            (ActionsDialogHost::AgentChat, FocusRequest::agent_chat()),
            (
                ActionsDialogHost::EditorPrompt,
                FocusRequest::editor_prompt(),
            ),
            (
                ActionsDialogHost::TemplatePrompt,
                FocusRequest::template_prompt(),
            ),
            (ActionsDialogHost::FormPrompt, FocusRequest::form_prompt()),
            (ActionsDialogHost::DivPrompt, FocusRequest::div_prompt()),
            (ActionsDialogHost::TermPrompt, FocusRequest::term_prompt()),
            (ActionsDialogHost::WebcamPrompt, FocusRequest::div_prompt()),
            (ActionsDialogHost::FileSearch, FocusRequest::main_filter()),
            (
                ActionsDialogHost::ClipboardHistory,
                FocusRequest::main_filter(),
            ),
            (ActionsDialogHost::EmojiPicker, FocusRequest::main_filter()),
            (ActionsDialogHost::AppLauncher, FocusRequest::main_filter()),
        ];

        for (host, expected) in cases {
            assert_eq!(
                restore(host, SurfaceKind::ScriptList),
                expected,
                "host {host:?} must restore its own input focus",
            );
        }
    }
}

#[cfg(test)]
mod actions_host_mapping_tests {
    use super::*;

    #[test]
    fn actions_host_for_view_maps_in_scope_surfaces() {
        let cases = vec![
            (AppView::ScriptList, Some(ActionsDialogHost::MainList)),
            (
                AppView::AppLauncherView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::AppLauncher),
            ),
            (
                AppView::ThemeChooserView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::ThemeChooser),
            ),
            (
                AppView::SettingsView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::BuiltinList),
            ),
            (
                AppView::AgentChatHistoryView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::AgentChatHistory),
            ),
            (
                AppView::ClipboardHistoryView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::ClipboardHistory),
            ),
            (
                AppView::BrowserHistoryView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::BuiltinList),
            ),
            (
                AppView::EmojiPickerView {
                    filter: String::new(),
                    selected_index: 0,
                    selected_category: None,
                },
                Some(ActionsDialogHost::EmojiPicker),
            ),
            (
                AppView::MiniPrompt {
                    id: String::new(),
                    placeholder: String::new(),
                    choices: Vec::new(),
                },
                None,
            ),
        ];

        for (view, expected) in cases {
            assert_eq!(ScriptListApp::actions_host_for_view(&view), expected);
        }
    }

    #[test]
    fn live_actions_host_for_view_excludes_generic_builtin_list_views() {
        let browser_history = AppView::BrowserHistoryView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&browser_history),
            None
        );

        let settings = AppView::SettingsView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(ScriptListApp::live_actions_host_for_view(&settings), None);

        let theme_chooser = AppView::ThemeChooserView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&theme_chooser),
            Some(ActionsDialogHost::ThemeChooser)
        );

        let current_app_commands = AppView::CurrentAppCommandsView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&current_app_commands),
            None
        );

        let process_manager = AppView::ProcessManagerView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&process_manager),
            None
        );
    }

    #[test]
    fn live_actions_host_for_view_keeps_selection_specific_hosts() {
        let file_search = AppView::FileSearchView {
            query: String::new(),
            selected_index: 0,
            presentation: FileSearchPresentation::Full,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&file_search),
            Some(ActionsDialogHost::FileSearch)
        );

        let clipboard = AppView::ClipboardHistoryView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&clipboard),
            Some(ActionsDialogHost::ClipboardHistory)
        );
    }
}

#[cfg(test)]
mod actions_dialog_wiring_regression_tests {
    use std::fs;

    use super::menu_syntax_displayed_shortcut_should_consume;

    #[test]
    fn render_script_list_routes_popup_keys_before_generic_cmd_shortcuts() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        let route_pos = source
            .find("this.route_key_to_actions_dialog(")
            .expect("render_script_list must use the shared actions router");
        let cmd_pos = source
            .find("if has_cmd")
            .expect("render_script_list cmd shortcut block not found");

        assert!(
            route_pos < cmd_pos,
            "render_script_list must route popup keys before generic Cmd shortcuts"
        );
    }

    #[test]
    fn menu_syntax_displayed_shortcut_only_consumes_enter() {
        assert!(menu_syntax_displayed_shortcut_should_consume("enter"));

        for shortcut in ["s", "t", "backspace", "escape", "cmd-k"] {
            assert!(
                !menu_syntax_displayed_shortcut_should_consume(shortcut),
                "menu-syntax main-list ownership must not swallow {shortcut}; the focused input or normal key handlers still own editing keys"
            );
        }
    }

    #[test]
    fn route_key_to_actions_dialog_notifies_after_arrow_navigation() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        let route_start = source
            .find("pub(crate) fn route_key_to_actions_dialog")
            .expect("route_key_to_actions_dialog not found");
        let route_fn = &source[route_start..];

        let up_start = route_fn
            .find("if is_key_up(key)")
            .expect("up branch missing");
        let down_start = route_fn
            .find("if is_key_down(key)")
            .expect("down branch missing");
        let jump_start = route_fn
            .find("let is_home = key.eq_ignore_ascii_case(\"home\")")
            .expect("jump-key section missing");

        assert!(
            route_fn[up_start..down_start].contains("crate::actions::notify_actions_window(cx);"),
            "up branch must notify the actions window"
        );
        assert!(
            route_fn[down_start..jump_start].contains("crate::actions::notify_actions_window(cx);"),
            "down branch must notify the actions window"
        );
    }

    #[test]
    fn route_key_to_actions_dialog_handles_cmd_k_close() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        assert!(
            source.contains("key.eq_ignore_ascii_case(\"k\")")
                && source.contains("self.close_actions_popup(host, window, cx);"),
            "shared actions router should close the popup on Cmd+K"
        );
    }

    #[test]
    fn route_key_to_actions_dialog_keeps_detached_window_routable() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        assert!(
            source
                .contains("!self.show_actions_popup && !crate::actions::is_actions_window_open()"),
            "shared actions router must keep routing keys while the detached actions window is open"
        );
    }

    #[test]
    fn route_key_to_actions_dialog_preserves_return_origin_for_explicit_agent_chat_handoff() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        assert!(
            source.contains(
                "self.open_tab_ai_agent_chat_with_explicit_target_preserving_return(target, cx);"
            ),
            "shared actions Cmd+Enter handoff should seed Agent Chat return origin before opening Agent Chat"
        );
    }

    #[test]
    fn render_script_list_has_no_duplicate_popup_handler() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        // The old inline popup handler used this pattern - it should be gone
        assert!(
            !source.contains("if this.show_actions_popup {"),
            "render_script_list must not contain a duplicate inline popup key handler"
        );
    }
}

#[cfg(test)]
mod agent_chat_spine_dispatch_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct DispatchTestConnection;

    impl crate::ai::agent_chat::runtime::AgentChatConnection for DispatchTestConnection {
        fn start_turn(
            &self,
            _request: crate::ai::agent_chat::runtime::AgentChatTurnRequest,
        ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
        {
            Err(crate::ai::reliability::AiAdapterError::from_record(
                crate::ai::reliability::provider_failure(
                    sk_protocol::ai_reliability::ProtocolComponent::Provider,
                    "OF-35b dispatch test must not submit",
                ),
            ))
        }

        fn cancel_turn(
            &self,
            _ui_thread_id: String,
        ) -> crate::ai::reliability::AiAdapterResult<()> {
            Ok(())
        }

        fn prepare_session(
            &self,
            _ui_thread_id: String,
            _cwd: std::path::PathBuf,
        ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
        {
            let (_tx, rx) = async_channel::bounded(1);
            Ok(rx)
        }
    }

    /// OF-35b: bare Enter from the main-window actions interceptor must reach
    /// the embedded Agent Chat Spine before the inactive-main-list shortcut
    /// bypass. This exercises the real dispatcher with a live AgentChatView;
    /// it must accept the selected profile without submitting the transcript.
    #[gpui::test]
    fn embedded_agent_chat_profile_spine_enter_reaches_existing_acceptance_path(
        cx: &mut gpui::TestAppContext,
    ) {
        let catalog = crate::flows::catalog::flow_catalog();
        catalog.set_notify_hook(|| {});
        catalog.prime_ready_for_test(&crate::flows::resolve_flow_cwd(None));

        let app_slot = Arc::new(Mutex::new(None));
        let app_slot_for_window = Arc::clone(&app_slot);
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            cx.open_window(Default::default(), |window, cx| {
                let mut config = crate::config::Config::default();
                let mut built_ins = config.get_builtins();
                built_ins.app_launcher = false;
                config.built_ins = Some(built_ins);
                let app = cx.new(|cx| ScriptListApp::new(config, false, window, cx));
                *app_slot_for_window.lock().expect("store OF-35b test app") = Some(app);
                cx.new(|_| crate::MainMenuSelectionTestHost)
            })
            .expect("open OF-35b dispatch test window")
        });
        let app = app_slot
            .lock()
            .expect("read OF-35b test app")
            .take()
            .expect("OF-35b test app initialized");

        let selected_profiles = Arc::new(Mutex::new(Vec::<String>::new()));
        let selected_profiles_for_callback = Arc::clone(&selected_profiles);
        let (handled, before, after) = cx.update(|cx| {
            window
                .update(cx, |_host, window, cx| {
                    app.update(cx, |app, cx| {
                    let (_broker, permission_rx) =
                        crate::ai::agent_chat::ui::AgentChatPermissionBroker::new();
                    let thread = cx.new(|cx| {
                        crate::ai::agent_chat::ui::AgentChatThread::new(
                            Arc::new(DispatchTestConnection),
                            permission_rx,
                            crate::ai::agent_chat::ui::AgentChatThreadInit {
                                ui_thread_id: "of35b-dispatch-test".to_string(),
                                cwd: std::env::temp_dir().join("of35b-dispatch-test"),
                                initial_input: None,
                                initial_context_parts: Vec::new(),
                                display_name: "OF-35b Test".into(),
                                profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID
                                    .to_string(),
                                profile_display_name: Some("OF-35b Test".into()),
                                profile_icon_name: None,
                                selected_agent: None,
                                available_agents: Vec::new(),
                                launch_requirements:
                                    crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
                                available_models: Vec::new(),
                                selected_model_id: None,
                                session_policy:
                                    crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::Full,
                            },
                            cx,
                        )
                    });
                    thread.update(cx, |thread, cx| {
                        thread.mark_context_bootstrap_ready(cx);
                        thread
                            .apply_test_fixture(
                                "assistantText",
                                Some("OF-35b user message".to_string()),
                                Some("OF-35b assistant message".to_string()),
                                None,
                                cx,
                            )
                            .expect("seed OF-35b transcript");
                    });
                    let entity = cx.new(|cx| {
                        crate::ai::agent_chat::ui::AgentChatView::new(thread, cx)
                    });
                    app.current_view = AppView::AgentChatView {
                        entity: entity.clone(),
                    };
                    entity.update(cx, |chat, cx| {
                        let thread = chat.thread().expect("fixture must have a live thread");
                        let mut draft = thread.read(cx).draft_snapshot();
                        draft.pending_context_parts = vec![
                            crate::ai::message_parts::AiContextPart::SkillFile {
                                path: "/tmp/of35b-context.md".to_string(),
                                label: "OF-35b context".to_string(),
                                skill_name: "of35b-context".to_string(),
                                owner_label: "test".to_string(),
                                slash_name: "of35b-context".to_string(),
                            },
                        ];
                        thread.update(cx, |thread, cx| {
                            thread.restore_draft_snapshot(draft, cx);
                        });
                        chat.set_on_profile_selected(move |profile_id, _cx| {
                            selected_profiles_for_callback
                                .lock()
                                .expect("record selected profile")
                                .push(profile_id);
                        });
                        // The GPUI test platform has no native display handle;
                        // opening the same profile trigger without popup geometry
                        // produces the identical Spine projection used by dispatch.
                        chat.open_profile_trigger_picker(cx);
                        chat.set_input("|text".to_string(), cx);
                        chat.refresh_agent_chat_spine_from_composer(cx);
                    });

                    let before = entity.read(cx).collect_agent_chat_state_snapshot(cx);
                    let spine = before
                        .spine
                        .as_ref()
                        .expect("exact profile query must project through Spine");
                    assert!(spine.owns_list);
                    assert_eq!(spine.active_segment_kind, "profile");
                    assert_eq!(spine.row_count, 1);
                    assert_eq!(spine.selectable_row_count, 1);
                    assert_eq!(spine.selected_index, 0);
                    assert_eq!(before.message_count, 2);
                    assert_eq!(before.context_chip_count, 1);

                    let handled = app.try_execute_main_list_action_shortcut_from_display(
                        "enter",
                        &gpui::Modifiers::default(),
                        window,
                        cx,
                    );
                    let after = entity.read(cx).collect_agent_chat_state_snapshot(cx);
                    (handled, before, after)
                    })
                })
                .expect("update OF-35b dispatch test window")
        });
        cx.run_until_parked();

        assert!(handled, "the interceptor must consume Spine profile Enter");
        assert_eq!(
            selected_profiles
                .lock()
                .expect("read selected profiles")
                .as_slice(),
            ["text"]
        );
        assert_eq!(after.input_text, "", "profile trigger must clear exactly");
        assert!(
            !after.spine.as_ref().is_some_and(|spine| spine.owns_list),
            "accepted profile must release Spine list ownership"
        );
        assert_eq!(after.message_count, before.message_count, "must not submit");
        assert_eq!(
            after.context_chip_count, before.context_chip_count,
            "profile acceptance must preserve staged context"
        );
    }
}

#[cfg(test)]
mod modal_backdrop_policy_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn backdrop_test_flow_meta(id: u64) -> crate::flows::session::FlowSessionMeta {
        crate::flows::session::FlowSessionMeta {
            id,
            flow_id: "project:backdrop-test".into(),
            flow_name: "flow-backdrop-test".into(),
            friendly_name: "Backdrop Test".into(),
            origin: "Project".into(),
            engine: "codex".into(),
            flow_path: "/tmp/flow-backdrop-test.md".into(),
            flow_mtime_ms: 0,
            cwd: "/tmp".into(),
            transport: crate::flows::session::SessionTransport::CodexThread,
            state: crate::flows::session::SessionState::NeedsYou,
            started_at: std::time::Instant::now(),
            last_activity: std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(1_000),
            turns: vec![],
            active_turn: None,
            thread_ready: true,
            needs_rethread: false,
            reliability: crate::flows::session::FlowReliability::new(
                "project:backdrop-test",
                "/tmp/flow-backdrop-test.md",
                "codex",
            ),
        }
    }

    /// Oracle step 8 policy lock (deferred by the plan until the step-6
    /// store existed): a modal-backdrop click DISMISSES THE MODAL ONLY.
    /// The topmost modal owns and consumes the click; it must never
    /// background, touch, remove, or otherwise mutate the underlying AI
    /// session, and it must not change the surface underneath. Ruling:
    /// user submission 98cab5e5-…641, confirmed verbatim by Oracle consult
    /// `floating-capsule-entry-material`.
    ///
    /// This drives `close_actions_popup` — the exact method the backdrop
    /// click handler routes to (`src/render_prompts/arg/helpers.rs`) — so
    /// the assertion covers the real dismissal path, not a re-derived one.
    #[gpui::test]
    fn backdrop_dismissal_never_mutates_backgrounded_sessions(cx: &mut gpui::TestAppContext) {
        let catalog = crate::flows::catalog::flow_catalog();
        catalog.set_notify_hook(|| {});
        catalog.prime_ready_for_test(&crate::flows::resolve_flow_cwd(None));

        let app_slot = Arc::new(Mutex::new(None));
        let app_slot_for_window = Arc::clone(&app_slot);
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            cx.open_window(Default::default(), |window, cx| {
                let mut config = crate::config::Config::default();
                let mut built_ins = config.get_builtins();
                built_ins.app_launcher = false;
                config.built_ins = Some(built_ins);
                let app = cx.new(|cx| ScriptListApp::new(config, false, window, cx));
                *app_slot_for_window.lock().expect("store backdrop test app") = Some(app);
                cx.new(|_| crate::MainMenuSelectionTestHost)
            })
            .expect("open backdrop policy test window")
        });
        let app = app_slot
            .lock()
            .expect("read backdrop test app")
            .take()
            .expect("backdrop test app initialized");

        let (store_before, view_before) = cx.update(|cx| {
            app.update(cx, |app, cx| {
                // One backgrounded flow session in the canonical store.
                let focus_handle = cx.focus_handle();
                let entity = cx.new(|_| {
                    crate::prompts::ChatPrompt::new(
                        "flow-session-11".to_string(),
                        None,
                        vec![],
                        None,
                        None,
                        focus_handle,
                        Arc::new(|_, _| {}) as crate::prompts::ChatSubmitCallback,
                        Arc::new(crate::theme::Theme::default()),
                    )
                });
                app.conversations
                    .flow_sessions
                    .push((backdrop_test_flow_meta(11), entity));

                // Open a minimal actions dialog — the modal under test.
                let theme_arc = Arc::clone(&app.theme);
                let dialog = cx.new(move |cx| {
                    let focus_handle = cx.focus_handle();
                    ActionsDialog::with_config(
                        focus_handle,
                        Arc::new(|_action_id| {}),
                        vec![],
                        theme_arc,
                        crate::actions::ActionsDialogConfig::default(),
                    )
                });
                app.actions_dialog = Some(dialog);
                app.mark_actions_popup_opening();

                (
                    app.conversations.snapshot(),
                    format!("{:?}", std::mem::discriminant(&app.current_view)),
                )
            })
        });

        // The backdrop click: exactly what the shield's on_click listener
        // does (arg/helpers.rs backdrop_click -> close_actions_popup).
        cx.update(|cx| {
            window
                .update(cx, |_host, window, cx| {
                    app.update(cx, |app, cx| {
                        app.close_actions_popup(ActionsDialogHost::MainList, window, cx);
                    });
                })
                .expect("drive backdrop dismissal");
        });

        cx.update(|cx| {
            app.update(cx, |app, _cx| {
                // DismissModal happened…
                assert!(
                    app.actions_dialog.is_none(),
                    "backdrop click must dismiss the modal"
                );
                // …and ONLY DismissModal: the store is byte-identical (same
                // count, ids, liveness, turnInFlight, and last_activity —
                // dismissal is NOT semantic session activity), and the
                // surface underneath did not change or background.
                assert_eq!(
                    app.conversations.snapshot(),
                    store_before,
                    "modal dismissal must not mutate any backgrounded session"
                );
                assert_eq!(
                    format!("{:?}", std::mem::discriminant(&app.current_view)),
                    view_before,
                    "modal dismissal must not change the underlying surface"
                );
            })
        });
    }
}

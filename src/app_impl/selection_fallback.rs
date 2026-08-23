use super::*;

#[cfg(test)]
fn resolve_grouped_result_index(
    grouped_items: &[GroupedListItem],
    selected_index: usize,
) -> Option<(usize, usize)> {
    let coerced_index = crate::list_item::coerce_selection(grouped_items, selected_index)?;
    match grouped_items.get(coerced_index) {
        Some(GroupedListItem::Item(result_idx)) => Some((coerced_index, *result_idx)),
        _ => None,
    }
}

fn fallback_keeps_window_open(fallback: &crate::fallbacks::FallbackItem) -> bool {
    match fallback {
        crate::fallbacks::FallbackItem::Builtin(builtin) => matches!(
            builtin.id,
            "run-in-terminal"
                | crate::fallbacks::builtins::SEARCH_FILES_FALLBACK_ID
                | crate::fallbacks::builtins::DO_IN_CURRENT_APP_FALLBACK_ID
                | crate::fallbacks::builtins::SEND_TO_AI_FALLBACK_ID
        ),
        crate::fallbacks::FallbackItem::Script(_) => true,
    }
}

impl ScriptListApp {
    #[allow(dead_code)]
    pub(crate) fn filtered_scripts(&self) -> Vec<Arc<scripts::Script>> {
        let filter_text = self.filter_text();
        if filter_text.is_empty() {
            self.scripts.clone()
        } else {
            let filter_lower = filter_text.to_lowercase();
            self.scripts
                .iter()
                .filter(|s| s.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect()
        }
    }

    /// Find a script or scriptlet by alias (case-insensitive exact match)
    /// Uses O(1) registry lookup instead of O(n) iteration
    pub(crate) fn find_alias_match(&self, alias: &str) -> Option<AliasMatch> {
        let alias_lower = alias.to_lowercase();

        // O(1) lookup in registry
        if let Some(command_id) = self.alias_registry.get(&alias_lower) {
            // Check for builtin/{id} command IDs
            if command_id.starts_with("builtin/") {
                let config = crate::config::BuiltInConfig::default();
                if let Some(entry) = builtins::get_builtin_entries(&config)
                    .into_iter()
                    .find(|e| e.id == *command_id)
                {
                    tracing::info!(
                        alias_sha256 = %crate::logging::log_private_user_value(alias),
                        alias_bytes = alias.len(),
                        command_id_sha256 = %crate::logging::log_private_user_value(command_id),
                        "alias_builtin_match_resolved"
                    );
                    return Some(AliasMatch::BuiltIn(std::sync::Arc::new(entry)));
                }
            }

            // Check for app/{bundle_id} command IDs
            if let Some(bundle_id) = command_id.strip_prefix("app/") {
                if let Some(app) = self
                    .apps
                    .iter()
                    .find(|a| a.bundle_id.as_deref() == Some(bundle_id))
                {
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Found app match: alias={} app={}",
                            crate::logging::log_private_user_value(alias),
                            crate::logging::log_private_user_value(&app.name),
                        ),
                    );
                    return Some(AliasMatch::App(std::sync::Arc::new(app.clone())));
                }
            }

            // Resolve exact source-owned aliases and historical plugin/name IDs.
            if let Some(identifier) = command_id.strip_prefix("script/") {
                let found = self
                    .scripts
                    .iter()
                    .find(|script| script.matches_launcher_command_identifier(identifier));
                if let Some(script) = found {
                    tracing::info!(
                        alias_sha256 = %crate::logging::log_private_user_value(alias),
                        alias_bytes = alias.len(),
                        command_id_sha256 = %crate::logging::log_private_user_value(command_id),
                        "alias_script_match_resolved"
                    );
                    return Some(AliasMatch::Script(script.clone()));
                }
            }

            // Scriptlet source/anchor digests coexist with their legacy aliases.
            if let Some(identifier) = command_id.strip_prefix("scriptlet/") {
                let found = self
                    .scriptlets
                    .iter()
                    .find(|scriptlet| scriptlet.matches_launcher_command_identifier(identifier));
                if let Some(scriptlet) = found {
                    tracing::info!(
                        alias_sha256 = %crate::logging::log_private_user_value(alias),
                        alias_bytes = alias.len(),
                        command_id_sha256 = %crate::logging::log_private_user_value(command_id),
                        "alias_scriptlet_match_resolved"
                    );
                    return Some(AliasMatch::Scriptlet(scriptlet.clone()));
                }
            }

            // Legacy: find script/scriptlet by path (metadata-defined aliases store paths)
            for script in &self.scripts {
                if script.path.to_string_lossy() == *command_id {
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Found script match: alias={} script={}",
                            crate::logging::log_private_user_value(alias),
                            crate::logging::log_private_user_value(&script.name),
                        ),
                    );
                    return Some(AliasMatch::Script(script.clone()));
                }
            }

            // Legacy: check scriptlets by file_path or name
            for scriptlet in &self.scriptlets {
                let scriptlet_path = scriptlet.file_path.as_ref().unwrap_or(&scriptlet.name);
                if scriptlet_path == command_id {
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Found scriptlet match: alias={} scriptlet={}",
                            crate::logging::log_private_user_value(alias),
                            crate::logging::log_private_user_value(&scriptlet.name),
                        ),
                    );
                    return Some(AliasMatch::Scriptlet(scriptlet.clone()));
                }
            }

            // Command ID in registry but not found (stale entry)
            logging::log(
                "ALIAS",
                &format!(
                    "Stale registry entry: alias={} command_id={} (not found)",
                    crate::logging::log_private_user_value(alias),
                    crate::logging::log_private_user_value(command_id),
                ),
            );
        }

        None
    }

    fn record_blocked_script_list_submit(&mut self, route: &'static str) {
        let value = self.filter_text.clone();
        tracing::info!(
            event = "script_list_submit_blocked",
            route,
            filter_sha256 = %crate::logging::log_private_user_value(&value),
            filter_bytes = value.len(),
            computed_filter_sha256 = %crate::logging::log_private_user_value(
                &self.computed_filter_text
            ),
            computed_filter_bytes = self.computed_filter_text.len(),
            grouped_cache_key_sha256 = %crate::logging::log_private_user_value(
                self.main_menu_result_caches.grouped_cache_key()
            ),
            selected_index = self.selected_index,
        );
        self.record_submit_diagnostic("launcher", route, None, Some(value.as_str()), true);
    }

    fn live_script_list_flat_selection_for_submit(&mut self) -> Option<(usize, usize)> {
        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }

        if self.filter_text != self.computed_filter_text {
            self.record_blocked_script_list_submit("main_list_submit_blocked.stale_filter_domain");
            return None;
        }

        self.get_grouped_results_cached();

        if !self
            .main_menu_result_caches
            .has_grouped_results_for_filter_text(&self.computed_filter_text)
        {
            self.record_blocked_script_list_submit(
                "main_list_submit_blocked.stale_grouped_cache_domain",
            );
            return None;
        }

        let Some((resolved_index, flat_result_index)) = self
            .main_menu_result_caches
            .flat_result_index_for_coerced_grouped_selection(self.selected_index)
        else {
            self.record_blocked_script_list_submit("main_list_submit_blocked.no_live_selected_row");
            return None;
        };

        if resolved_index != self.selected_index {
            self.selected_index = resolved_index;
            self.rebuild_main_window_preflight_if_needed();
        }

        Some((resolved_index, flat_result_index))
    }

    pub(crate) fn execute_selected(&mut self, cx: &mut Context<Self>) {
        if self.should_consume_script_list_enter_after_submit("execute_selected") {
            logging::log(
                "KEY",
                "Ignoring execute_selected: prompt submit already consumed this Enter",
            );
            return;
        }
        if self.menu_syntax_trigger_picker_owns_main_keyboard()
            || crate::menu_syntax::active_filter_head_owns_main_list(&self.filter_text)
        {
            logging::log(
                "KEY",
                "Ignoring execute_selected: menu-syntax owns main list",
            );
            return;
        }
        if self.should_consume_menu_syntax_filter_accept_submit("execute_selected") {
            logging::log(
                "KEY",
                "Ignoring execute_selected: accepted menu-syntax filter hint is active",
            );
            return;
        }

        if let Some(invocation) = self
            .menu_syntax_mode
            .command_for(&self.filter_text)
            .cloned()
        {
            let value = self.filter_text.clone();
            self.record_submit_diagnostic(
                "launcher",
                "execute_selected.menu_syntax_command",
                None,
                Some(value.as_str()),
                false,
            );
            self.execute_menu_syntax_command_invocation(invocation, cx);
            return;
        }

        if let Some(invocation) = self
            .menu_syntax_mode
            .capture_for(&self.filter_text)
            .cloned()
        {
            let value = self.filter_text.clone();
            self.record_submit_diagnostic(
                "launcher",
                "execute_selected.menu_syntax_capture",
                None,
                Some(value.as_str()),
                false,
            );
            let mut ranked_handlers =
                crate::menu_syntax::rank_handlers_for_target(&self.scripts, &invocation);
            if let Some(handler) = ranked_handlers.first() {
                if crate::menu_syntax::ranked_handler_is_user_authored(handler) {
                    self.execute_menu_syntax_capture_script(handler.script.clone(), invocation, cx);
                    return;
                }
            }
            match self.try_execute_app_owned_menu_syntax_capture(&invocation, cx) {
                AppOwnedCaptureOutcome::Handled | AppOwnedCaptureOutcome::Invalid => return,
                AppOwnedCaptureOutcome::NotOwned => {}
            }
            if let Some(handler) = ranked_handlers.drain(..).next() {
                self.execute_menu_syntax_capture_script(handler.script, invocation, cx);
            } else {
                self.show_hud(
                    format!("No capture handler for +{}", invocation.target),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
            }
            return;
        }

        if self
            .menu_syntax_mode
            .capture_composer_owns_input_for(&self.filter_text)
        {
            self.show_hud(
                "Type something to capture".to_string(),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            return;
        }

        if self
            .menu_syntax_mode
            .command_owns_input_for(&self.filter_text)
        {
            self.show_hud(
                "Choose a Script Kit command".to_string(),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            return;
        }

        let history_query = (!self.filter_text.trim().is_empty()).then(|| self.filter_text.clone());

        let Some((_resolved_index, idx)) = self.live_script_list_flat_selection_for_submit() else {
            return;
        };
        {
            let selected_result = self
                .main_menu_result_caches
                .cloned_search_result_for_flat_index(idx);
            if let Some(result) = selected_result.as_ref() {
                if let Err(reason) = result.authorize_launcher_submit() {
                    tracing::warn!(
                        target: "script_kit::execution",
                        source = ?result.command_source(),
                        reason,
                        "launcher_command_refused_before_frecency_or_execution"
                    );
                    self.show_hud(reason.to_string(), Some(HUD_MEDIUM_MS), cx);
                    return;
                }
            }

            if let Some(query) = history_query.as_deref() {
                self.input_history.add_entry_with_selection(
                    query,
                    selected_result
                        .as_ref()
                        .and_then(|result| result.history_result_key()),
                );
                if let Err(e) = self.input_history.save() {
                    tracing::warn!(
                        error_sha256 = %crate::logging::log_private_user_value(&e.to_string()),
                        "Failed to save input history"
                    );
                }
                self.invalidate_grouped_cache();
            }

            if let Some(formatted_value) = self
                .inline_calculator_for_result_index(idx)
                .map(|calculator| calculator.formatted.clone())
            {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(formatted_value.clone()));
                self.show_hud(
                    format!("Copied: {}", formatted_value),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
                self.close_and_reset_window(cx);
                return;
            }

            if self.is_in_attachment_portal() && matches!(self.current_view, AppView::ScriptList) {
                if let Some(part) =
                    self.build_attachment_portal_part_for_selected_script_list_result()
                {
                    self.close_attachment_portal_with_part(part, cx);
                }
                return;
            }

            if let Some(result) = selected_result {
                let submitted_value = result.launcher_command_name();
                tracing::info!(
                    target: "script_kit::submit",
                    event = "launcher_execute_selected_resolved",
                    filter_sha256 = %crate::logging::log_private_user_value(&self.filter_text),
                    filter_bytes = self.filter_text.len(),
                    computed_filter_sha256 = %crate::logging::log_private_user_value(
                        &self.computed_filter_text
                    ),
                    computed_filter_bytes = self.computed_filter_text.len(),
                    selected_index = self.selected_index,
                    flat_result_index = idx,
                    submitted_value_sha256 = %crate::logging::log_private_user_value(
                        &submitted_value
                    ),
                    result_name_sha256 = %crate::logging::log_private_user_value(result.name()),
                    result_type = %result.type_label(),
                    "launcher execute_selected resolved visible row"
                );
                self.record_submit_diagnostic(
                    "launcher",
                    "execute_selected",
                    None,
                    Some(submitted_value.as_str()),
                    false,
                );
                // Record frecency usage before executing (unless excluded).
                // Skills and scriptlets use plugin-qualified keys.
                let frecency_path: Option<String> = match &result {
                    scripts::SearchResult::Script(sm) => {
                        Some(sm.script.path.to_string_lossy().to_string())
                    }
                    scripts::SearchResult::App(am) => {
                        Some(am.app.path.to_string_lossy().to_string())
                    }
                    scripts::SearchResult::BuiltIn(bm) => {
                        // Skip frecency tracking for excluded builtins (e.g., "Quit Script Kit")
                        let excluded = &self.config.get_suggested().excluded_commands;
                        if bm.entry.should_exclude_from_frecency(excluded) {
                            None
                        } else {
                            Some(format!("builtin:{}", bm.entry.id))
                        }
                    }
                    scripts::SearchResult::Scriptlet(sm) => Some(format!(
                        "scriptlet:{}:{}",
                        sm.scriptlet.plugin_id, sm.scriptlet.name
                    )),
                    scripts::SearchResult::Skill(sm) => Some(format!(
                        "skill:{}:{}",
                        sm.skill.plugin_id, sm.skill.skill_id
                    )),
                    // Same frecency key as history_result_key so ranking and
                    // input-history preference agree on the flow identity.
                    scripts::SearchResult::Flow(fm) => {
                        fm.flow.as_ref().map(|flow| format!("flow:{}", flow.id))
                    }
                    scripts::SearchResult::Window(wm) => {
                        Some(format!("window:{}:{}", wm.window.app, wm.window.title))
                    }
                    // Root file opens record frecency only after the OS open succeeds;
                    // execute_root_file_open is shared by Enter and the root-file Open action.
                    scripts::SearchResult::File(_) => None,
                    scripts::SearchResult::Note(_) => None,
                    scripts::SearchResult::BrainHit(_) => None,
                    scripts::SearchResult::BrainInboxItem(_) => None,
                    scripts::SearchResult::Todo(_) => None,
                    scripts::SearchResult::AgentChatHistory(_) => None,
                    scripts::SearchResult::AiVault(_) => None,
                    scripts::SearchResult::ClipboardHistory(_) => None,
                    scripts::SearchResult::DictationHistory(_) => None,
                    scripts::SearchResult::BrowserTab(_) => None,
                    scripts::SearchResult::BrowserHistory(_) => None,
                    // Suppressed: agents don't track frecency in the launcher
                    scripts::SearchResult::Agent(_) => None,
                    // Fallbacks don't track frecency - they're utility commands
                    scripts::SearchResult::Fallback(_) => None,
                    // Script Issues is a synthetic diagnostic row — no frecency.
                    scripts::SearchResult::ScriptIssue(_) => None,
                    // Spine projections don't track frecency.
                    scripts::SearchResult::SpineProjection(_) => None,
                };
                if let Some(path) = frecency_path {
                    self.frecency_store.record_use(&path);
                    self.frecency_store.save().ok(); // Best-effort save
                    self.invalidate_grouped_cache(); // Invalidate cache so next show reflects frecency
                }

                // Log the action being performed using the same label path as the footer/preflight.
                let action_text = self.main_window_primary_action_label();
                logging::log(
                    "EXEC",
                    &format!(
                        "Action: '{}' on {} (type: {})",
                        action_text,
                        crate::logging::log_private_user_value(result.name()),
                        result.type_label()
                    ),
                );

                // Menu-syntax capture rows intercept the normal Script path.
                // `MenuSyntaxMode` is raw-guarded against the current filter
                // text, so a stale parse can never route a non-capture
                // selection into the capture pipeline.
                let capture_invocation = self
                    .menu_syntax_mode
                    .capture_for(&self.computed_filter_text)
                    .cloned();
                if let (Some(invocation), scripts::SearchResult::Script(script_match)) =
                    (capture_invocation, &result)
                {
                    let script = script_match.script.clone();
                    let selected_handler_is_user_authored =
                        crate::menu_syntax::rank_handlers_for_target(&self.scripts, &invocation)
                            .into_iter()
                            .find(|entry| entry.script.path == script.path)
                            .map(|entry| {
                                crate::menu_syntax::ranked_handler_is_user_authored(&entry)
                            })
                            .unwrap_or(true);
                    if !selected_handler_is_user_authored {
                        match self.try_execute_app_owned_menu_syntax_capture(&invocation, cx) {
                            AppOwnedCaptureOutcome::Handled | AppOwnedCaptureOutcome::Invalid => {
                                return;
                            }
                            AppOwnedCaptureOutcome::NotOwned => {}
                        }
                    }
                    self.execute_menu_syntax_capture_script(script, invocation, cx);
                    return;
                }

                self.mark_opened_from_main_menu("execute_selected");
                match result {
                    scripts::SearchResult::Script(script_match) => {
                        // Run 13 Pass 2 (user bug report) — if the user
                        // selected a menu-syntax CAPTURE handler script
                        // directly from the main list (no `;target …`
                        // composer active), running it would crash on the
                        // missing `KIT_MENU_SYNTAX_PAYLOAD_PATH` env var.
                        // Pivot to the power-syntax composer by writing
                        // `;target ` into the filter so the user can
                        // complete the capture invocation in-place.
                        if let Some(target) =
                            crate::menu_syntax::first_concrete_capture_target_for_script(
                                &script_match.script,
                            )
                        {
                            let new_filter = format!(";{} ", target);
                            tracing::info!(
                                target: "script_kit::menu_syntax",
                                event = "script_list_pivot_to_power_syntax_composer",
                                script_sha256 = %crate::logging::log_private_user_value(
                                    &script_match.script.name
                                ),
                                pivot_filter_sha256 = %crate::logging::log_private_user_value(
                                    &new_filter
                                ),
                                pivot_filter_bytes = new_filter.len(),
                                "Pivoting main-list capture-handler launch into ;target composer"
                            );
                            self.filter_text = new_filter.clone();
                            self.computed_filter_text = new_filter.clone();
                            self.pending_filter_sync = true;
                            self.set_menu_syntax_mode_from_filter(&new_filter);
                            self.invalidate_grouped_cache();
                            cx.notify();
                            return;
                        }
                        // Run 13 Pass 4 — symmetric pivot for `command.v1`
                        // handlers: pivot into the `!head ` command composer
                        // instead of running the script process bare.
                        if let Some(head) =
                            crate::menu_syntax::first_command_head_for_script(&script_match.script)
                        {
                            let new_filter = format!("!{} ", head);
                            tracing::info!(
                                target: "script_kit::menu_syntax",
                                event = "script_list_pivot_to_command_composer",
                                script_sha256 = %crate::logging::log_private_user_value(
                                    &script_match.script.name
                                ),
                                pivot_filter_sha256 = %crate::logging::log_private_user_value(
                                    &new_filter
                                ),
                                pivot_filter_bytes = new_filter.len(),
                                "Pivoting main-list command-handler launch into !head composer"
                            );
                            self.filter_text = new_filter.clone();
                            self.computed_filter_text = new_filter.clone();
                            self.pending_filter_sync = true;
                            self.set_menu_syntax_mode_from_filter(&new_filter);
                            self.invalidate_grouped_cache();
                            cx.notify();
                            return;
                        }
                        self.execute_interactive(&script_match.script, cx);
                    }
                    scripts::SearchResult::Scriptlet(scriptlet_match) => {
                        self.execute_scriptlet(&scriptlet_match.scriptlet, cx);
                    }
                    scripts::SearchResult::BuiltIn(builtin_match) => {
                        self.execute_builtin(&builtin_match.entry, cx);
                    }
                    scripts::SearchResult::App(app_match) => {
                        self.execute_app(&app_match.app, cx);
                    }
                    scripts::SearchResult::Window(window_match) => {
                        self.execute_window_focus(&window_match.window, cx);
                    }
                    scripts::SearchResult::File(file_match) => {
                        self.execute_root_file_open(&file_match.file, cx);
                    }
                    scripts::SearchResult::Note(note_match) => {
                        self.execute_root_note_open(note_match.hit.id, cx);
                    }
                    scripts::SearchResult::BrainHit(brain_match) => {
                        self.execute_root_brain_hit_open(&brain_match.hit, cx);
                    }
                    scripts::SearchResult::BrainInboxItem(inbox_match) => {
                        self.execute_root_brain_inbox_open(inbox_match.item, cx);
                    }
                    scripts::SearchResult::Todo(todo_match) => {
                        self.execute_root_todo_copy(&todo_match.hit, cx);
                    }
                    scripts::SearchResult::AgentChatHistory(agent_chat_history_match) => {
                        self.resume_agent_chat_conversation_from_history(
                            &agent_chat_history_match.entry.session_id,
                            agent_chat_history_match.entry.first_message.as_str(),
                            cx,
                        );
                    }
                    scripts::SearchResult::AiVault(ai_vault_match) => {
                        self.execute_root_ai_vault_paste_resume_command(&ai_vault_match.hit, cx);
                    }
                    scripts::SearchResult::ClipboardHistory(clipboard_match) => {
                        self.execute_root_clipboard_history_paste(&clipboard_match.entry.id, cx);
                    }
                    scripts::SearchResult::DictationHistory(dictation_match) => {
                        self.execute_root_dictation_history_paste(&dictation_match.id, cx);
                    }
                    scripts::SearchResult::BrowserTab(browser_tab_match) => {
                        self.execute_root_browser_tab_switch(&browser_tab_match.hit, cx);
                    }
                    scripts::SearchResult::BrowserHistory(browser_match) => {
                        self.execute_root_browser_history_open(&browser_match.hit.url, cx);
                    }
                    scripts::SearchResult::Flow(flow_match) => {
                        match &flow_match.target {
                            // Conversation rows resume the EXACT retained
                            // entity in the BackgroundedSessionStore,
                            // dispatched by tagged session id.
                            scripts::ConversationRowTarget::Conversation(id) => {
                                self.resume_backgrounded_conversation(id.clone(), cx);
                            }
                            // Flow identity rows resume the flow's
                            // conversation (live session, then persisted
                            // transcript) and only start a blank Threadline
                            // when none exists.
                            scripts::ConversationRowTarget::FlowIdentity { flow_id } => {
                                let Some(flow) = flow_match.flow.as_ref() else {
                                    // Identity rows are built from the flow
                                    // corpus, so a missing descriptor is a
                                    // construction bug, not a user state.
                                    tracing::error!(
                                        event = "flow_identity_row_missing_descriptor",
                                        flow_id = %flow_id,
                                        "Flow identity row lost its descriptor; cannot launch"
                                    );
                                    return;
                                };
                                tracing::info!(
                                    event = "flow_session_launch_requested",
                                    flow_id = %flow.id,
                                    flow_name = %flow.name,
                                    engine = %flow.engine,
                                    "Flow selected from main menu"
                                );
                                self.resume_or_start_flow_session(flow, None, cx);
                            }
                        }
                    }
                    scripts::SearchResult::Skill(skill_match) => {
                        // Skills always open Agent Chat with the selected skill staged
                        let owner = if skill_match.skill.plugin_title.is_empty() {
                            skill_match.skill.plugin_id.as_str()
                        } else {
                            skill_match.skill.plugin_title.as_str()
                        };
                        tracing::info!(
                            event = "agent_chat_skill_launch_requested",
                            plugin_id = %skill_match.skill.plugin_id,
                            skill_id = %skill_match.skill.skill_id,
                            path_sha256 = %crate::logging::log_private_user_value(
                                &skill_match.skill.path.to_string_lossy()
                            ),
                            owner,
                            "Skill selected from main menu"
                        );
                        self.show_hud(
                            format!("Opening {} \u{b7} {}", owner, skill_match.skill.title),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                        tracing::info!(
                            event = "agent_chat_skill_launch_hud_shown",
                            plugin_id = %skill_match.skill.plugin_id,
                            skill_id = %skill_match.skill.skill_id,
                            owner,
                            "Displayed Agent Chat skill launch HUD"
                        );
                        self.open_agent_chat_with_selected_skill(&skill_match.skill, cx);
                    }
                    scripts::SearchResult::Agent(agent_match) => {
                        // Suppressed: agents are not launchable from the main menu.
                        // Skills replace agents as the first-class reusable AI artifact.
                        // Agent Chat agent catalog/provider selection remains in src/ai/agent_chat/ui/.
                        tracing::info!(
                            event = "legacy_agent_result_suppressed",
                            agent_name_sha256 = %crate::logging::log_private_user_value(
                                &agent_match.agent.name
                            ),
                            agent_path_sha256 = %crate::logging::log_private_user_value(
                                &agent_match.agent.path.to_string_lossy()
                            ),
                            "Agent execution suppressed in main menu - use skills or Agent Chat"
                        );
                    }
                    scripts::SearchResult::Fallback(fallback_match) => {
                        // Execute the fallback with the current filter text as input
                        self.execute_fallback_item(&fallback_match.fallback, cx);
                    }
                    scripts::SearchResult::ScriptIssue(_) => {
                        self.open_script_issues_view(cx);
                    }
                    scripts::SearchResult::SpineProjection(_) => {
                        // Spine projection rows are not actionable from the main menu.
                    }
                }
            }
        }
    }

    pub(crate) fn selected_main_list_search_result_owned(
        &mut self,
    ) -> Option<scripts::SearchResult> {
        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }

        let (_resolved_index, result_idx) = self.live_script_list_flat_selection_for_submit()?;

        if self
            .inline_calculator_for_result_index(result_idx)
            .is_some()
        {
            return None;
        }

        self.main_menu_result_caches
            .cloned_search_result_for_flat_index(result_idx)
    }

    pub(crate) fn selected_root_file_result_owned(
        &mut self,
    ) -> Option<crate::file_search::FileResult> {
        match self.selected_main_list_search_result_owned()? {
            scripts::SearchResult::File(file_match) => Some(file_match.file),
            _ => None,
        }
    }

    pub(crate) fn execute_root_note_open(
        &mut self,
        note_id: crate::notes::NoteId,
        cx: &mut Context<Self>,
    ) {
        match crate::notes::open_note_in_notes_window(cx, note_id) {
            Ok(()) => self.hide_main_and_reset(cx),
            Err(error) => {
                logging::log("ERROR", &format!("Failed to open root note: {error}"));
                self.show_hud("Failed to open note".to_string(), Some(HUD_MEDIUM_MS), cx);
            }
        }
    }

    /// Enter on a "From Your Brain" passive row. Routes by document source:
    /// notes open in the Notes editor, chat turns resume their Agent Chat
    /// conversation, and everything else opens a read-only preview of the
    /// memory itself (audit F5: plain Enter must never auto-submit an AI
    /// turn, and the user must be able to actually read the memory).
    pub(crate) fn execute_root_brain_hit_open(
        &mut self,
        hit: &crate::brain::RootBrainSearchHit,
        cx: &mut Context<Self>,
    ) {
        match hit.source {
            crate::brain::DocSource::Note => match crate::notes::NoteId::parse(&hit.source_id) {
                Some(note_id) => self.execute_root_note_open(note_id, cx),
                None => {
                    logging::log(
                        "ERROR",
                        &format!("Brain hit has invalid note id: {}", hit.source_id),
                    );
                    self.show_hud("Failed to open note".to_string(), Some(HUD_MEDIUM_MS), cx);
                }
            },
            crate::brain::DocSource::ChatTurn => {
                // source_id is "{thread_id}#{turn_index}"; resume by thread id.
                let thread_id = hit
                    .source_id
                    .split('#')
                    .next()
                    .unwrap_or(hit.source_id.as_str());
                if crate::ai::agent_chat::ui::history::conversation_exists(thread_id) {
                    self.resume_agent_chat_conversation_from_history(thread_id, &hit.excerpt, cx);
                } else {
                    // Conversation file gone: the old fallback parked the raw
                    // "user: …" excerpt as a composer draft, making re-sending
                    // your own old message the default action. Show the
                    // stored memory instead, like every other non-note source.
                    self.open_brain_memory_preview(hit, cx);
                }
            }
            crate::brain::DocSource::Clipboard
            | crate::brain::DocSource::Activity
            | crate::brain::DocSource::Capture
            | crate::brain::DocSource::DayPage
            | crate::brain::DocSource::Fragment => {
                self.open_brain_memory_preview(hit, cx);
            }
        }
    }

    /// Open a read-only preview of a brain memory in the main window. Reuses
    /// the DivPrompt surface (theme-aware HTML rendering, scrolling, the
    /// standard prompt shell) without a protocol session: Enter/Escape close
    /// it back to the launcher via the root capture_key_down handler and the
    /// simulateKey dispatcher, both keyed on
    /// [`BRAIN_MEMORY_PREVIEW_PROMPT_ID`].
    fn open_brain_memory_preview(
        &mut self,
        hit: &crate::brain::RootBrainSearchHit,
        cx: &mut Context<Self>,
    ) {
        let doc = crate::brain::get_doc(hit.source, &hit.source_id)
            .ok()
            .flatten();
        let (content, updated_at) = match &doc {
            Some(doc) if !doc.content.trim().is_empty() => {
                (doc.content.clone(), Some(doc.updated_at))
            }
            _ => (hit.excerpt.clone(), None),
        };
        let title = if hit.title.trim().is_empty() {
            hit.source_label.to_string()
        } else {
            hit.title.clone()
        };
        let html = crate::brain::root_brain_memory_preview_html(
            &title,
            hit.source_label,
            updated_at,
            &content,
        );
        tracing::info!(
            target: "script_kit::brain",
            source = %hit.source_label,
            source_id = %hit.source_id,
            "opening brain memory preview"
        );
        // Sessionless prompt: closing is owned by the key paths above, so the
        // prompt's own submit callback has nothing to do.
        let on_submit: crate::prompts::SubmitCallback = std::sync::Arc::new(|_id, _value| {});
        let id = BRAIN_MEMORY_PREVIEW_PROMPT_ID.to_string();
        let div_prompt = crate::prompts::DivPrompt::new(
            id.clone(),
            html,
            None,
            cx.focus_handle(),
            on_submit,
            std::sync::Arc::clone(&self.theme),
        );
        let entity = cx.new(|_| div_prompt);
        self.current_view = AppView::DivPrompt { id, entity };
        self.focused_input = FocusedInput::None;
        self.pending_focus = Some(FocusTarget::AppRoot);
        resize_to_view_sync(ViewType::DivPrompt, 0);
        cx.notify();
    }

    /// Enter on a pinned "Brain Inbox" row. Always opens Agent Chat with the
    /// full inbox follow-up staged as context plus a visible local assistant
    /// prompt. The launcher must not auto-submit the follow-up, prefill the
    /// composer, or resolve the inbox item: resolution is destructive and
    /// belongs to an explicit action after the user reviews the staged item.
    pub(crate) fn execute_root_brain_inbox_open(
        &mut self,
        item: crate::brain::InboxItem,
        cx: &mut Context<Self>,
    ) {
        let context_part = Self::brain_inbox_context_part(&item);
        if item.source.as_str() == "chat_turn" {
            // source_id is "{thread_id}#{turn_index}"; load the saved
            // conversation when possible, then stage the inbox as context
            // instead of sending a surprise turn.
            let thread_id = item
                .source_id
                .split('#')
                .next()
                .unwrap_or(item.source_id.as_str());
            self.open_agent_chat_with_staged_brain_inbox_prompt(Some(thread_id), context_part, cx);
        } else {
            self.open_agent_chat_with_staged_brain_inbox_prompt(None, context_part, cx);
        }
    }

    fn open_agent_chat_with_staged_brain_inbox_prompt(
        &mut self,
        history_thread_id: Option<&str>,
        context_part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        if let Some(chat_entity) =
            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
        {
            chat_entity.update(cx, |chat_view, cx| {
                if let Some(thread_id) = history_thread_id {
                    chat_view.resume_from_history(thread_id, cx);
                }
                Self::stage_brain_inbox_context_on_agent_chat(chat_view, context_part.clone(), cx);
            });
            self.reset_to_script_list(cx);
            return;
        }

        self.open_clean_agent_chat(cx);

        if let AppView::AgentChatView { entity } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat_view, cx| {
                if let Some(thread_id) = history_thread_id {
                    chat_view.resume_from_history(thread_id, cx);
                }
                Self::stage_brain_inbox_context_on_agent_chat(chat_view, context_part, cx);
            });
        } else {
            tracing::warn!(
                target: "script_kit::brain",
                event = "brain_inbox_stage_agent_chat_unavailable",
                "Unable to stage Brain Inbox prompt because Agent Chat did not open"
            );
        }
    }

    fn stage_brain_inbox_context_on_agent_chat(
        chat_view: &mut crate::ai::agent_chat::ui::AgentChatView,
        context_part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<crate::ai::agent_chat::ui::AgentChatView>,
    ) {
        let assistant_message = Self::brain_inbox_local_assistant_message(&context_part);
        if let Some(thread) = chat_view.thread() {
            thread.update(cx, |thread, cx| {
                thread.replace_pending_context_parts(vec![context_part], "brain_inbox_enter", cx);
                thread.push_local_assistant_message(assistant_message, cx);
            });
        }
    }

    fn brain_inbox_local_assistant_message(
        context_part: &crate::ai::message_parts::AiContextPart,
    ) -> String {
        let crate::ai::message_parts::AiContextPart::TextBlock { text, .. } = context_part else {
            return "What should we do with this inbox item?".to_string();
        };
        let mut visible_context = text
            .split_once("\nTask:\n")
            .map(|(context, _)| context)
            .unwrap_or(text.as_str())
            .trim();
        visible_context = visible_context
            .strip_prefix("Follow up on this Brain Inbox item.\n\n")
            .unwrap_or(visible_context)
            .trim();
        if visible_context.is_empty() {
            return "What should we do with this inbox item?".to_string();
        }
        format!("What should we do with this?\n\n{visible_context}")
    }

    fn brain_inbox_context_part(
        item: &crate::brain::InboxItem,
    ) -> crate::ai::message_parts::AiContextPart {
        crate::ai::message_parts::AiContextPart::TextBlock {
            label: Self::brain_inbox_context_label(item),
            source: format!("brain-inbox://{}", item.id),
            text: crate::brain::response_prompt_for_inbox_item(item),
            mime_type: Some("text/markdown".to_string()),
        }
    }

    fn brain_inbox_context_label(item: &crate::brain::InboxItem) -> String {
        let title = Self::brain_inbox_short_title(item);
        if title.is_empty() {
            "Brain Inbox".to_string()
        } else {
            format!("Brain Inbox: {title}")
        }
    }

    fn brain_inbox_short_title(item: &crate::brain::InboxItem) -> String {
        let collapsed = item.title.split_whitespace().collect::<Vec<_>>().join(" ");
        const MAX_CHARS: usize = 80;
        if collapsed.chars().count() <= MAX_CHARS {
            return collapsed;
        }
        let split_at = collapsed
            .char_indices()
            .nth(MAX_CHARS)
            .map(|(idx, _)| idx)
            .unwrap_or(collapsed.len());
        format!("{}...", collapsed[..split_at].trim_end())
    }

    pub(crate) fn selected_root_directory_query_owned(&mut self) -> Option<String> {
        let file = self.selected_root_file_result_owned()?;
        Self::root_file_search_in_folder_query(&file)
    }

    fn record_root_file_open_use(&mut self, file: &crate::file_search::FileResult) {
        let keys = root_file_open_frecency_keys(file);
        for key in [keys.file.as_deref(), keys.directory.as_deref()]
            .into_iter()
            .flatten()
        {
            self.frecency_store.record_use(key);
        }
        if let Err(error) = self.frecency_store.save() {
            tracing::warn!(
                path_sha256 = %crate::logging::log_private_user_value(&file.path),
                path_bytes = file.path.len(),
                error_sha256 = %crate::logging::log_private_user_value(&error.to_string()),
                "Failed to save root file frecency after open"
            );
        }
        self.invalidate_grouped_cache();
    }

    pub(crate) fn execute_root_file_open(
        &mut self,
        file: &crate::file_search::FileResult,
        cx: &mut Context<Self>,
    ) {
        // Journal the query → choice pair so the brain can answer
        // "what did I just search for?".
        let query = self.filter_text.trim();
        if !query.is_empty() {
            crate::brain::record_activity(
                "searched for",
                &format!("\"{}\" and opened {}", query, file.path),
            );
        }
        if let Err(error) = crate::file_search::open_file(&file.path) {
            logging::log(
                "ROOT_FILE_SEARCH",
                &format!(
                    "failed_to_open path={} error={}",
                    crate::logging::log_private_user_value(&file.path),
                    crate::logging::log_private_user_value(&error.to_string()),
                ),
            );
            self.show_hud(
                format!("Failed to open {}", file.name),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            return;
        }
        self.record_root_file_open_use(file);
        self.close_and_reset_window(cx);
    }

    pub(crate) fn execute_root_clipboard_history_paste(
        &mut self,
        entry_id: &str,
        cx: &mut Context<Self>,
    ) {
        match crate::clipboard_history::copy_entry_to_clipboard(entry_id) {
            Ok(()) => {
                logging::log("EXEC", &format!("Root clipboard entry copied: {entry_id}"));
                self.hide_main_and_reset(cx);
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if let Err(error) = crate::selected_text::simulate_paste_with_cg() {
                        logging::log(
                            "ERROR",
                            &format!("Failed to simulate root clipboard paste: {error}"),
                        );
                    } else {
                        logging::log("EXEC", "Simulated root clipboard paste");
                    }
                });
            }
            Err(error) => {
                logging::log(
                    "ERROR",
                    &format!("Failed to copy root clipboard entry: {error}"),
                );
                self.show_hud(
                    "Failed to paste clipboard entry".to_string(),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
            }
        }
    }

    pub(crate) fn execute_root_todo_copy(
        &mut self,
        hit: &crate::menu_syntax::RootTodoSearchHit,
        cx: &mut Context<Self>,
    ) {
        let text = if hit.body.trim().is_empty() {
            hit.title.clone()
        } else {
            hit.body.clone()
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        logging::log("EXEC", &format!("Copied root todo: {}", hit.stable_key));
        self.show_hud("Copied todo".to_string(), Some(HUD_SHORT_MS), cx);
    }

    pub(crate) fn execute_root_dictation_history_paste(
        &mut self,
        entry_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = crate::dictation::get_history_entry(entry_id) else {
            logging::log(
                "ERROR",
                &format!("Root dictation history entry not found: {entry_id}"),
            );
            self.show_hud(
                "Failed to paste dictation".to_string(),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            return;
        };

        let transcript = entry.transcript;
        self.hide_main_and_reset(cx);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let injector = crate::text_injector::TextInjector::new();
            if let Err(error) = injector.paste_text(&transcript) {
                logging::log(
                    "ERROR",
                    &format!("Failed to paste root dictation history: {error}"),
                );
            } else {
                logging::log("EXEC", "Pasted root dictation history transcript");
            }
        });
    }

    pub(crate) fn execute_root_browser_history_open(&mut self, url: &str, cx: &mut Context<Self>) {
        match crate::browser_history::open_browser_history_url(url) {
            Ok(()) => {
                logging::log(
                    "EXEC",
                    &format!(
                        "Opened root browser history URL: {}",
                        crate::logging::log_private_user_value(url)
                    ),
                );
                self.hide_main_and_reset(cx);
            }
            Err(error) => {
                logging::log(
                    "ERROR",
                    &format!("Failed to open root browser history URL: {error}"),
                );
                self.show_hud(
                    "Failed to open browser history page".to_string(),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
            }
        }
    }

    pub(crate) fn execute_root_browser_tab_switch(
        &mut self,
        hit: &crate::browser_tabs::RootBrowserTabSearchHit,
        cx: &mut Context<Self>,
    ) {
        match crate::browser_tabs::focus_root_browser_tab(hit) {
            Ok(()) => {
                logging::log(
                    "EXEC",
                    &format!(
                        "Focused root browser tab: {}",
                        crate::logging::log_private_user_value(&hit.title)
                    ),
                );
                self.hide_main_and_reset(cx);
            }
            Err(error) => {
                logging::log(
                    "ERROR",
                    &format!("Failed to focus root browser tab: {error}"),
                );
                self.show_hud(
                    "Failed to switch browser tab".to_string(),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
            }
        }
    }

    pub(crate) fn root_file_search_in_folder_query(
        file: &crate::file_search::FileResult,
    ) -> Option<String> {
        (file.file_type == crate::file_search::FileType::Directory)
            .then(|| crate::file_search::ensure_trailing_slash(&file.path))
    }

    pub(crate) fn root_file_browse_parent_folder_query(
        file: &crate::file_search::FileResult,
    ) -> Option<String> {
        if file.file_type == crate::file_search::FileType::Directory {
            return None;
        }
        crate::file_search::parent_folder_search_query(&file.path)
    }

    pub(crate) fn root_file_parent_query_for_filter(filter_text: &str) -> Option<String> {
        if !crate::file_search::looks_like_root_directory_browse_query(filter_text) {
            return None;
        }

        let parsed = crate::file_search::parse_directory_path(filter_text)?;
        if parsed.filter.is_some() {
            Some(parsed.directory)
        } else {
            crate::file_search::parent_dir_display(&parsed.directory)
        }
    }

    fn clear_main_list_selection_for_root_file_handoff(&mut self) {
        self.selected_index = 0;
        self.main_list_state
            .scroll_to_reveal_item(self.selected_index);
        self.last_scrolled_index = Some(self.selected_index);
    }

    pub(crate) fn try_navigate_root_file_directory_with_tab(
        &mut self,
        has_shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::ScriptList) || self.show_actions_popup {
            return false;
        }

        let next_query = if has_shift {
            Self::root_file_parent_query_for_filter(&self.filter_text)
        } else {
            self.selected_root_directory_query_owned()
        };

        let Some(next_query) = next_query else {
            return false;
        };

        self.set_filter_text_immediate(next_query, window, cx);
        true
    }

    pub(crate) fn execute_root_file_action(
        &mut self,
        action_id: &str,
        file: &crate::file_search::FileResult,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match action_id {
            crate::action_helpers::ROOT_FILE_OPEN_ACTION_ID => {
                self.pending_root_file_actions_file = None;
                self.execute_root_file_open(file, cx);
                true
            }
            crate::action_helpers::ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID => {
                if let Err(error) = crate::file_search::reveal_in_finder(&file.path) {
                    logging::log(
                        "ROOT_FILE_SEARCH",
                        &format!(
                            "failed_to_reveal path={} error={}",
                            crate::logging::log_private_user_value(&file.path),
                            crate::logging::log_private_user_value(&error.to_string()),
                        ),
                    );
                    self.show_hud(
                        format!("Failed to reveal {}", file.name),
                        Some(HUD_MEDIUM_MS),
                        cx,
                    );
                    return true;
                }
                self.pending_root_file_actions_file = None;
                self.close_and_reset_window(cx);
                true
            }
            crate::action_helpers::ROOT_FILE_COPY_PATH_ACTION_ID => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(file.path.clone()));
                self.show_hud(
                    format!("Copied path: {}", file.path),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
                true
            }
            crate::action_helpers::ROOT_FILE_COPY_NAME_ACTION_ID => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(file.name.clone()));
                self.show_hud(
                    format!("Copied name: {}", file.name),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
                true
            }
            crate::action_helpers::ROOT_FILE_QUICK_LOOK_ACTION_ID => {
                match crate::file_search::quick_look(&file.path) {
                    Ok(()) => {
                        self.show_hud(format!("Previewing {}", file.name), Some(HUD_MEDIUM_MS), cx);
                    }
                    Err(error) => {
                        logging::log(
                            "ROOT_FILE_SEARCH",
                            &format!(
                                "failed_to_quick_look path={} error={}",
                                crate::logging::log_private_user_value(&file.path),
                                crate::logging::log_private_user_value(&error.to_string()),
                            ),
                        );
                        self.show_hud(
                            format!("Failed to preview {}: {}", file.name, error),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                    }
                }
                true
            }
            crate::action_helpers::ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID => {
                let Some(query) = Self::root_file_search_in_folder_query(file) else {
                    self.show_hud(
                        format!("Not a folder: {}", file.name),
                        Some(HUD_MEDIUM_MS),
                        cx,
                    );
                    return true;
                };
                self.pending_root_file_actions_file = None;
                self.open_file_search(query, cx);
                true
            }
            crate::action_helpers::ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID => {
                let Some(query) = Self::root_file_browse_parent_folder_query(file) else {
                    self.show_hud(
                        format!("No parent folder for {}", file.name),
                        Some(HUD_MEDIUM_MS),
                        cx,
                    );
                    return true;
                };
                self.pending_root_file_actions_file = None;
                self.clear_main_list_selection_for_root_file_handoff();
                self.open_file_search(query, cx);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn try_execute_root_file_action_shortcut(
        &mut self,
        key_lower: &str,
        modifiers: &gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::ScriptList)
            || self.show_actions_popup
            || crate::actions::is_actions_window_open()
        {
            return false;
        }
        if !modifiers.platform || modifiers.alt || modifiers.control {
            return false;
        }

        let action_id = match (key_lower, modifiers.shift) {
            ("y", false) => crate::action_helpers::ROOT_FILE_QUICK_LOOK_ACTION_ID,
            ("c", true) => crate::action_helpers::ROOT_FILE_COPY_PATH_ACTION_ID,
            ("f", true) => crate::action_helpers::ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID,
            _ => return false,
        };

        let Some(file) = self.selected_root_file_result_owned() else {
            return false;
        };
        self.execute_root_file_action(action_id, &file, window, cx)
    }

    /// Execute a fallback item (from the "Use with..." section in search results)
    /// This is called when a fallback is selected from the grouped list
    pub fn execute_fallback_item(
        &mut self,
        fallback: &crate::fallbacks::FallbackItem,
        cx: &mut Context<Self>,
    ) {
        let input = self.filter_text.clone();

        logging::log(
            "EXEC",
            &format!(
                "Executing fallback item: {} with input: {} bytes={}",
                fallback.display_name(),
                crate::logging::log_private_user_value(&input),
                input.len(),
            ),
        );

        let should_close = !fallback_keeps_window_open(fallback);

        // Execute the fallback action
        match fallback {
            crate::fallbacks::FallbackItem::Builtin(builtin) => {
                let fallback_id = builtin.id.to_string();
                self.execute_builtin_fallback_inline(&fallback_id, &input, cx);
            }
            crate::fallbacks::FallbackItem::Script(config) => {
                self.execute_interactive(&config.script, cx);
            }
        }

        // Close the window after executing (unless it's a stay-open action)
        if should_close {
            self.close_and_reset_window(cx);
        }
    }

    /// Execute the currently selected fallback command
    /// This is called from keyboard handler, so we need to defer window access
    pub fn execute_selected_fallback(&mut self, cx: &mut Context<Self>) {
        if self.should_consume_script_list_enter_after_submit("execute_selected_fallback") {
            logging::log(
                "KEY",
                "Ignoring execute_selected_fallback: prompt submit already consumed this Enter",
            );
            return;
        }

        if let Some(scripts::SearchResult::Fallback(fallback_match)) =
            self.selected_main_list_search_result_owned()
        {
            let submitted_value = fallback_match.display_name();
            self.record_submit_diagnostic(
                "launcher_fallback",
                "execute_selected_fallback.grouped",
                None,
                Some(submitted_value.as_str()),
                false,
            );
            logging::log(
                "EXEC",
                &format!(
                    "Executing selected grouped fallback: {}",
                    fallback_match.display_name()
                ),
            );
            self.execute_fallback_item(&fallback_match.fallback, cx);
            return;
        }

        self.record_blocked_script_list_submit(
            "execute_selected_fallback.no_live_grouped_fallback",
        );
    }

    /// Execute a built-in fallback action without window reference
    pub(crate) fn execute_builtin_fallback_inline(
        &mut self,
        fallback_id: &str,
        input: &str,
        cx: &mut Context<Self>,
    ) {
        use crate::fallbacks::builtins::{get_builtin_fallbacks, FallbackResult};

        logging::log(
            "FALLBACK",
            &format!(
                "Executing fallback '{}' with input: {} bytes={}",
                fallback_id,
                crate::logging::log_private_user_value(input),
                input.len(),
            ),
        );

        // Find the fallback by ID
        let fallbacks = get_builtin_fallbacks();
        let fallback = fallbacks.iter().find(|f| f.id == fallback_id);

        let Some(fallback) = fallback else {
            logging::log("FALLBACK", &format!("Unknown fallback ID: {}", fallback_id));
            return;
        };

        // Execute the fallback and get the result
        match fallback.execute(input) {
            Ok(result) => match result {
                FallbackResult::RunTerminal { command } => {
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "RunTerminal: {} bytes={}",
                            crate::logging::log_private_user_value(&command),
                            command.len(),
                        ),
                    );
                    // Open the built-in terminal with the command
                    self.open_terminal_with_command(command, cx);
                }
                FallbackResult::AddNote { content } => {
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "AddNote: {} bytes={}",
                            crate::logging::log_private_user_value(&content),
                            content.len(),
                        ),
                    );
                    let item = gpui::ClipboardItem::new_string(content);
                    cx.write_to_clipboard(item);
                    if let Err(e) = crate::notes::open_notes_window(cx) {
                        logging::log("FALLBACK", &format!("Failed to open Notes: {}", e));
                    }
                }
                FallbackResult::Copy { text } => {
                    logging::log("FALLBACK", &format!("Copy: {} chars", text.len()));
                    let item = gpui::ClipboardItem::new_string(text);
                    cx.write_to_clipboard(item);
                    crate::hud_manager::show_hud(
                        "Copied to clipboard".to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    );
                }
                FallbackResult::OpenUrl { url } => {
                    logging::log(
                        "FALLBACK",
                        &format!("OpenUrl: {}", crate::logging::log_private_user_value(&url)),
                    );
                    let _ = open::that(&url);
                }
                FallbackResult::Calculate { expression } => {
                    // Evaluate the expression using meval
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "Calculate: {} bytes={}",
                            crate::logging::log_private_user_value(&expression),
                            expression.len(),
                        ),
                    );
                    match meval::eval_str(&expression) {
                        Ok(result) => {
                            let item = gpui::ClipboardItem::new_string(result.to_string());
                            cx.write_to_clipboard(item);
                            crate::hud_manager::show_hud(
                                format!("{} = {}", expression, result),
                                Some(HUD_LONG_MS),
                                cx,
                            );
                        }
                        Err(e) => {
                            logging::log("FALLBACK", &format!("Calculate error: {}", e));
                            let message = calculate_fallback_error_message(&expression);
                            crate::hud_manager::show_hud(message, Some(HUD_LONG_MS), cx);
                        }
                    }
                }
                FallbackResult::OpenFile { path } => {
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "OpenFile: {}",
                            crate::logging::log_private_user_value(&path)
                        ),
                    );
                    let expanded = if path.starts_with("~") {
                        if let Some(home) = dirs::home_dir() {
                            path.replacen("~", &home.to_string_lossy(), 1)
                        } else {
                            path.clone()
                        }
                    } else {
                        path.clone()
                    };
                    let _ = open::that(&expanded);
                }
                FallbackResult::SearchFiles { query } => {
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "SearchFiles: {} bytes={}",
                            crate::logging::log_private_user_value(&query),
                            query.len(),
                        ),
                    );
                    self.open_file_search(query, cx);
                }
                FallbackResult::ExecuteBuiltin { builtin_id } => {
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "ExecuteBuiltin: builtin_id='{}' input_len={}",
                            builtin_id,
                            input.len()
                        ),
                    );

                    let builtin_entry = self
                        .builtin_entries
                        .iter()
                        .find(|entry| entry.id == builtin_id)
                        .cloned();

                    let Some(entry) = builtin_entry else {
                        logging::log(
                            "FALLBACK",
                            &format!(
                                "state=failed attempted=execute_builtin_fallback reason=builtin_not_found builtin_id={}",
                                builtin_id
                            ),
                        );
                        return;
                    };

                    self.execute_builtin_with_query(&entry, Some(input), cx);
                }
                FallbackResult::SendToAiHarness { query } => {
                    logging::log(
                        "FALLBACK",
                        &format!(
                            "SendToAiHarness: {} bytes={}",
                            crate::logging::log_private_user_value(&query),
                            query.len(),
                        ),
                    );
                    let normalized = query.trim().to_string();
                    let intent = if normalized.is_empty() {
                        None
                    } else {
                        Some(normalized)
                    };
                    self.open_tab_ai_agent_chat_with_entry_intent(intent, cx);
                }
            },
            Err(e) => {
                logging::log("FALLBACK", &format!("Fallback execution error: {}", e));
            }
        }
    }

    /// Enter action for the synthetic "Script Issues" pinned row. Snapshots the
    /// current [`ValidationReport`] into the new [`AppView::ScriptIssuesView`]
    /// so authors can read fatal issues and related colliders, then Escape back
    /// to the launcher or Cmd+C to copy diagnostics to the clipboard.
    pub(crate) fn open_script_issues_view(&mut self, cx: &mut Context<Self>) {
        let Some(report) = self.script_validation_report.clone() else {
            crate::hud_manager::show_hud(
                "No script validation report available".to_string(),
                Some(2500),
                cx,
            );
            return;
        };

        tracing::info!(
            event = "script_issues_row_activated",
            failed_count = report.failed_scripts.len(),
            fatal_count = report.fatal_count,
            warning_count = report.warning_count,
            "Script Issues row activated"
        );

        self.current_view = AppView::ScriptIssuesView { report };
        self.request_script_list_main_filter_focus(cx);
        cx.notify();
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RootFileOpenFrecencyKeys {
    file: Option<String>,
    directory: Option<String>,
}

fn root_file_open_frecency_keys(file: &crate::file_search::FileResult) -> RootFileOpenFrecencyKeys {
    let is_directory = file.file_type == crate::file_search::FileType::Directory;
    let file_key = (!is_directory && crate::file_search::root_global_file_result_is_eligible(file))
        .then(|| format!("file/{}", file.path));
    let directory_key = if is_directory {
        Some(format!("dir/{}", file.path))
    } else {
        std::path::Path::new(&file.path)
            .parent()
            .and_then(|parent| parent.to_str())
            .map(|parent| format!("dir/{parent}"))
    };

    RootFileOpenFrecencyKeys {
        file: file_key,
        directory: directory_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_grouped_result_index_coerces_section_header_selection() {
        let grouped_items = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(3),
            GroupedListItem::Item(4),
        ];

        assert_eq!(
            resolve_grouped_result_index(&grouped_items, 0),
            Some((1, 3))
        );
    }

    #[test]
    fn test_resolve_grouped_result_index_clamps_out_of_bounds_selection() {
        let grouped_items = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(8),
            GroupedListItem::SectionHeader("Main".to_string(), None),
        ];

        assert_eq!(
            resolve_grouped_result_index(&grouped_items, 100),
            Some((1, 8))
        );
    }

    #[test]
    fn test_resolve_grouped_result_index_returns_none_for_header_only_rows() {
        let grouped_items = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::SectionHeader("Main".to_string(), None),
        ];

        assert_eq!(resolve_grouped_result_index(&grouped_items, 0), None);
    }

    #[test]
    fn test_fallback_keeps_window_open_for_send_to_ai() {
        let fallback = crate::fallbacks::FallbackItem::Builtin(
            crate::fallbacks::builtins::get_builtin_fallbacks()
                .into_iter()
                .find(|fallback| fallback.id == crate::fallbacks::builtins::SEND_TO_AI_FALLBACK_ID)
                .expect("send-to-ai fallback should exist"),
        );

        assert!(fallback_keeps_window_open(&fallback));
    }

    #[test]
    fn test_fallback_keeps_window_open_for_do_in_current_app() {
        let fallback = crate::fallbacks::FallbackItem::Builtin(
            crate::fallbacks::builtins::get_builtin_fallbacks()
                .into_iter()
                .find(|fallback| {
                    fallback.id == crate::fallbacks::builtins::DO_IN_CURRENT_APP_FALLBACK_ID
                })
                .expect("do-in-current-app fallback should exist"),
        );

        assert!(fallback_keeps_window_open(&fallback));
    }

    #[test]
    fn test_fallback_keeps_window_open_is_false_for_regular_builtin() {
        let fallback = crate::fallbacks::FallbackItem::Builtin(
            crate::fallbacks::builtins::get_builtin_fallbacks()
                .into_iter()
                .find(|fallback| fallback.id == "search-google")
                .expect("search-google fallback should exist"),
        );

        assert!(!fallback_keeps_window_open(&fallback));
    }

    fn root_file_result(
        path: &str,
        file_type: crate::file_search::FileType,
    ) -> crate::file_search::FileResult {
        crate::file_search::FileResult {
            path: path.to_string(),
            name: std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_string(),
            size: 0,
            modified: 0,
            file_type,
        }
    }

    #[test]
    fn recent_files_frecency_plan_separates_files_and_directories() {
        let directory = root_file_result(
            "/tmp/example-folder",
            crate::file_search::FileType::Directory,
        );
        assert_eq!(
            root_file_open_frecency_keys(&directory),
            RootFileOpenFrecencyKeys {
                file: None,
                directory: Some("dir//tmp/example-folder".to_string()),
            }
        );

        let document = root_file_result(
            "/tmp/example-folder/report.pdf",
            crate::file_search::FileType::Document,
        );
        assert_eq!(
            root_file_open_frecency_keys(&document),
            RootFileOpenFrecencyKeys {
                file: Some("file//tmp/example-folder/report.pdf".to_string()),
                directory: Some("dir//tmp/example-folder".to_string()),
            }
        );
    }

    #[test]
    fn root_file_search_in_folder_query_accepts_directories() {
        let file = root_file_result(
            "/tmp/example-folder",
            crate::file_search::FileType::Directory,
        );

        assert_eq!(
            ScriptListApp::root_file_search_in_folder_query(&file),
            Some("/tmp/example-folder/".to_string())
        );
    }

    #[test]
    fn root_file_search_in_folder_query_rejects_regular_files() {
        let file = root_file_result("/tmp/example.txt", crate::file_search::FileType::Document);

        assert_eq!(ScriptListApp::root_file_search_in_folder_query(&file), None);
    }

    #[test]
    fn root_file_browse_parent_folder_query_accepts_regular_files() {
        let file = root_file_result(
            "/tmp/example-folder/readme.md",
            crate::file_search::FileType::Document,
        );

        assert_eq!(
            ScriptListApp::root_file_browse_parent_folder_query(&file),
            Some("/tmp/example-folder/".to_string())
        );
    }

    #[test]
    fn root_file_browse_parent_folder_query_rejects_directories() {
        let file = root_file_result(
            "/tmp/example-folder",
            crate::file_search::FileType::Directory,
        );

        assert_eq!(
            ScriptListApp::root_file_browse_parent_folder_query(&file),
            None
        );
    }

    #[test]
    fn root_file_parent_query_for_filter_accepts_directory_browse_queries() {
        let base = std::env::temp_dir().join(format!(
            "script-kit-root-parent-test-{}",
            std::process::id()
        ));
        let nested = base.join("example-folder");
        std::fs::create_dir_all(&nested).expect("create nested temp directory");
        let nested_query = format!("{}/", nested.display());

        assert_eq!(
            ScriptListApp::root_file_parent_query_for_filter(&nested_query),
            Some(format!("{}/", base.display()))
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_file_parent_query_for_filter_clears_child_fragment_first() {
        let base = std::env::temp_dir().join(format!(
            "script-kit-root-filter-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&base).expect("create temp directory");
        let filtered_query = format!("{}/al", base.display());

        assert_eq!(
            ScriptListApp::root_file_parent_query_for_filter(&filtered_query),
            Some(format!("{}/", base.display()))
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_file_parent_query_for_filter_rejects_plain_search_queries() {
        assert_eq!(
            ScriptListApp::root_file_parent_query_for_filter("fix"),
            None
        );
    }
}

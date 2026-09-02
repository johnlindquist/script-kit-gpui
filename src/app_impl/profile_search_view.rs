use super::*;

impl ScriptListApp {
    fn persist_profile_search_selection_for_host(&mut self, id: &str, quick_ai: bool) -> bool {
        if self.main_services.is_production() {
            return if quick_ai {
                crate::profile_search::persist_quick_ai_profile_search_selection(id)
            } else {
                crate::profile_search::persist_profile_search_selection(id)
            };
        }
        let context = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        let selected = if quick_ai {
            crate::ai::agent_chat::profiles::persist_quick_ai_profile_selection(
                &mut self.main_preferences.ai,
                id,
                &context,
            )
            .is_some()
        } else {
            crate::ai::agent_chat::profiles::persist_agent_chat_profile_selection(
                &mut self.main_preferences.ai,
                id,
                &context,
            )
            .is_some()
        };
        if selected {
            self.mark_main_data_changed();
        }
        selected
    }

    pub(crate) fn arm_return_to_script_list_enter_guard_from_profile_search(&mut self) {
        self.return_to_script_list_key_guard = Some(ReturnToScriptListKeyGuard {
            key: "enter",
            source: ReturnToScriptListKeyGuardSource::ProfileSearch,
            reason: "profile_search_select_return_to_script_list",
            armed_at: std::time::Instant::now(),
            consumed_count: 0,
        });
        tracing::info!(
            target: "script_kit::keyboard",
            event = "return_to_script_list_enter_guard_armed",
            source = "profile_search",
            reason = "profile_search_select_return_to_script_list",
            "Armed ProfileSearch Enter transition guard"
        );
    }

    pub(crate) fn consume_return_to_script_list_enter_guard(
        &mut self,
        key: &str,
        modifiers: &gpui::Modifiers,
    ) -> bool {
        if !crate::ui_foundation::is_key_enter(key)
            || modifiers.platform
            || modifiers.shift
            || modifiers.alt
            || modifiers.control
        {
            return false;
        }

        let Some(guard) = self.return_to_script_list_key_guard.as_mut() else {
            return false;
        };
        if guard.key != "enter" {
            return false;
        }
        if guard.armed_at.elapsed() > std::time::Duration::from_millis(1200) {
            self.return_to_script_list_key_guard = None;
            return false;
        }

        guard.consumed_count = guard.consumed_count.saturating_add(1);
        tracing::warn!(
            target: "script_kit::keyboard",
            event = "return_to_script_list_enter_guard_consumed",
            source = ?guard.source,
            reason = guard.reason,
            consumed_count = guard.consumed_count,
            "Suppressed leaked Enter after returning to ScriptList"
        );
        true
    }

    pub(crate) fn clear_return_to_script_list_enter_guard_on_key_up(&mut self, key: &str) {
        if crate::ui_foundation::is_key_enter(key)
            && self.return_to_script_list_key_guard.take().is_some()
        {
            tracing::info!(
                target: "script_kit::keyboard",
                event = "return_to_script_list_enter_guard_cleared",
                key,
                "Cleared ProfileSearch Enter transition guard on key-up"
            );
        }
    }

    pub(crate) fn open_profile_search(&mut self, cx: &mut Context<Self>) {
        tracing::info!(
            target: "script_kit::spine",
            event = "profile_search_open",
            "Opening Profile Search"
        );
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search profiles...".to_string());
        self.current_view = AppView::ProfileSearchView {
            filter: String::new(),
            selected_index: 0,
        };
        self.note_main_route_changed();
        self.rekey_main_automation_surface_from_current_view();
        self.hovered_index = None;
        self.opened_from_main_menu = true;
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    pub(crate) fn try_open_profile_search_from_script_list_shift_tab(
        &mut self,
        key: &str,
        modifiers: &gpui::Modifiers,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }
        if !crate::ui_foundation::is_key_tab(key) {
            return false;
        }
        if !modifiers.shift || modifiers.platform || modifiers.alt || modifiers.control {
            return false;
        }
        if !self.spine_enabled
            || self.show_actions_popup
            || self.menu_syntax_capture_form_owns_input()
            || !self.main_view_context_chip_has_action(
                crate::components::main_view_chrome::MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID,
                crate::components::main_view_chrome::SemanticChipAction::OpenSelector,
            )
        {
            return false;
        }

        tracing::info!(
            target: "script_kit::spine",
            event = "profile_switcher_open_shift_tab",
            source,
            "Shift+Tab -> Profile Search"
        );
        self.open_profile_search(cx);
        true
    }

    pub(crate) fn profile_search_results_for_filter(
        &self,
        filter: &str,
    ) -> Vec<crate::profile_search::ProfileSearchResult> {
        let prefs = if self.main_services.is_production() {
            crate::config::load_user_preferences()
        } else {
            self.main_preferences.clone()
        };
        let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        crate::profile_search::profile_search_results(&prefs.ai, &ctx, filter)
    }

    pub(crate) fn profile_search_visible_len(&self, filter: &str) -> usize {
        self.profile_search_results_for_filter(filter).len()
    }

    pub(crate) fn selected_profile_search_result_owned(
        &self,
    ) -> Option<crate::profile_search::ProfileSearchResult> {
        let AppView::ProfileSearchView {
            filter,
            selected_index,
        } = &self.current_view
        else {
            return None;
        };
        self.profile_search_results_for_filter(filter)
            .get(*selected_index)
            .cloned()
    }

    pub(crate) fn move_profile_search_selection(&mut self, is_up: bool, cx: &mut Context<Self>) {
        let (filter, current_index) = match &self.current_view {
            AppView::ProfileSearchView {
                filter,
                selected_index,
            } => (filter.clone(), *selected_index),
            _ => return,
        };
        let filtered_len = self.profile_search_visible_len(&filter);
        let mut next_index = current_index;
        if filtered_len == 0 {
            next_index = 0;
        } else {
            if next_index >= filtered_len {
                next_index = filtered_len - 1;
            }
            if is_up && next_index > 0 {
                next_index -= 1;
            } else if !is_up && next_index + 1 < filtered_len {
                next_index += 1;
            }
        }
        if let AppView::ProfileSearchView { selected_index, .. } = &mut self.current_view {
            *selected_index = next_index;
        }
        self.list_scroll_handle
            .scroll_to_item(next_index, ScrollStrategy::Nearest);
        self.input_mode = InputMode::Keyboard;
        self.hovered_index = None;
        cx.notify();
    }

    pub(crate) fn select_profile_search_result(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(result) = self.selected_profile_search_result_owned() else {
            return false;
        };
        // The built-in Quick AI row is not a valid Agent Chat default — Enter
        // on it performs the Quick AI assignment instead (same as Tab).
        if result.profile.id == crate::ai::agent_chat::profiles::BUILTIN_QUICK_AI_PROFILE_ID {
            return self.select_profile_search_result_for_quick_ai(cx);
        }
        // The trailing action row writes a starter markdown profile and hands
        // it to the editor instead of switching profiles.
        if result.profile.id == crate::profile_search::CREATE_PROFILE_ROW_ID {
            return self.create_profile_from_profile_search(cx);
        }
        let persisted = self.persist_profile_search_selection_for_host(&result.profile.id, false);
        if persisted {
            if self.main_services.is_production() {
                self.refresh_agent_model_footer_labels();
            }
            self.arm_return_to_script_list_enter_guard_from_profile_search();
            self.reset_to_script_list(cx);
            if self.main_services.is_production() {
                self.refresh_agent_model_footer_labels();
            } else {
                self.spine_agent_label = Some(result.profile.name.clone());
                self.spine_model_label = result.profile.model.clone();
                self.mark_main_data_changed();
            }
        }
        cx.notify();
        persisted
    }

    /// Enter on the "Create New Profile…" row: write a starter markdown
    /// profile (mdflow format) into `<kit>/profiles/` and open it in the
    /// configured editor.
    pub(crate) fn create_profile_from_profile_search(&mut self, cx: &mut Context<Self>) -> bool {
        // This command creates and opens a profile. Refuse the unavailable
        // external half before creating a file or resolving operator setup.
        if let Err(refusal) =
            crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
        {
            tracing::warn!(target: "script_kit::spine", code = refusal.code, "Profile editor launch refused");
            return false;
        }
        let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        match crate::ai::agent_chat::mdflow_profiles::create_mdflow_profile_from_template(&ctx) {
            Ok(path) => {
                tracing::info!(
                    target: "script_kit::spine",
                    event = "profile_search_create_profile",
                    path = %path.display(),
                    "Created markdown profile from template"
                );
                if let Err(error) = crate::script_creation::open_in_editor(&path, &self.config) {
                    tracing::warn!(
                        target: "script_kit::spine",
                        %error,
                        path = %path.display(),
                        "Created profile but failed to open it in the editor"
                    );
                }
                self.arm_return_to_script_list_enter_guard_from_profile_search();
                self.reset_to_script_list(cx);
                cx.notify();
                true
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::spine",
                    %error,
                    "Failed to create markdown profile from template"
                );
                cx.notify();
                false
            }
        }
    }

    /// Tab in Profile Search: use the highlighted profile for the launcher's
    /// Quick AI (Tab-with-text) mode. Mirrored by the SimulateKey protocol in
    /// `simulate_key_dispatch.rs` — keep the two paths in lockstep.
    pub(crate) fn select_profile_search_result_for_quick_ai(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(result) = self.selected_profile_search_result_owned() else {
            return false;
        };
        let persisted = self.persist_profile_search_selection_for_host(&result.profile.id, true);
        if persisted {
            self.arm_return_to_script_list_enter_guard_from_profile_search();
            self.reset_to_script_list(cx);
        }
        cx.notify();
        persisted
    }
}

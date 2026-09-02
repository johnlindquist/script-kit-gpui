use super::*;

/// Split `conflicts` into the subset not yet announced, and the replacement
/// announced set (the full current conflicts). Persistent conflicts that every
/// watcher refresh re-detects stay silent after their first announcement; a
/// conflict that resolves and later reappears is announced again because the
/// set is replaced (not accumulated) on every rebuild.
pub(crate) fn split_unannounced_registry_conflicts(
    announced: &std::collections::HashSet<String>,
    conflicts: Vec<String>,
) -> (Vec<String>, std::collections::HashSet<String>) {
    let fresh: Vec<String> = conflicts
        .iter()
        .filter(|conflict| !announced.contains(*conflict))
        .cloned()
        .collect();
    (fresh, conflicts.into_iter().collect())
}

impl ScriptListApp {
    /// HUD-gating wrapper around [`split_unannounced_registry_conflicts`]:
    /// returns only conflicts the user has not seen yet and records the
    /// current set for the next rebuild.
    pub(crate) fn take_unannounced_registry_conflicts(
        &mut self,
        conflicts: Vec<String>,
    ) -> Vec<String> {
        let (fresh, announced) =
            split_unannounced_registry_conflicts(&self.announced_registry_conflicts, conflicts);
        self.announced_registry_conflicts = announced;
        fresh
    }

    pub(crate) fn rebuild_registries(&mut self) -> Vec<String> {
        let mut conflicts = Vec::new();
        self.alias_registry.clear();
        self.shortcut_registry.clear();

        // Register script aliases
        for script in &self.scripts {
            if let Some(ref alias) = script.alias {
                let alias_lower = alias.to_lowercase();
                if let Some(existing_path) = self.alias_registry.get(&alias_lower) {
                    conflicts.push(format!(
                        "Alias conflict: '{}' already used by {}",
                        alias,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Conflict: alias '{}' in {} blocked (already used by {})",
                            alias,
                            script.path.display(),
                            existing_path
                        ),
                    );
                } else {
                    self.alias_registry
                        .insert(alias_lower, script.path.to_string_lossy().to_string());
                }
            }
        }

        // Register scriptlet aliases
        for scriptlet in &self.scriptlets {
            if let Some(ref alias) = scriptlet.alias {
                let alias_lower = alias.to_lowercase();
                if let Some(existing_path) = self.alias_registry.get(&alias_lower) {
                    conflicts.push(format!(
                        "Alias conflict: '{}' already used by {}",
                        alias,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Conflict: alias '{}' in {} blocked (already used by {})",
                            alias, scriptlet.name, existing_path
                        ),
                    );
                } else {
                    let path = scriptlet
                        .file_path
                        .clone()
                        .unwrap_or_else(|| scriptlet.name.clone());
                    self.alias_registry.insert(alias_lower, path);
                }
            }

            // Register scriptlet shortcuts
            if let Some(ref shortcut) = scriptlet.shortcut {
                let shortcut_lower = shortcut.to_lowercase();
                if let Some(existing_path) = self.shortcut_registry.get(&shortcut_lower) {
                    conflicts.push(format!(
                        "Shortcut conflict: '{}' already used by {}",
                        shortcut,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "SHORTCUT",
                        &format!(
                            "Conflict: shortcut '{}' in {} blocked (already used by {})",
                            shortcut, scriptlet.name, existing_path
                        ),
                    );
                } else {
                    let path = scriptlet
                        .file_path
                        .clone()
                        .unwrap_or_else(|| scriptlet.name.clone());
                    self.shortcut_registry.insert(shortcut_lower, path);
                }
            }
        }

        // Load alias overrides from ~/.scriptkit/aliases.json
        // These provide aliases for built-ins, apps, and other commands
        // that don't have their own alias metadata
        let alias_overrides = match self.main_services.owned_sources() {
            Some(sources) => Some(sources.alias_overrides.clone()),
            None => crate::aliases::load_alias_overrides().ok(),
        };
        if let Some(alias_overrides) = alias_overrides {
            let mut alias_overrides: Vec<_> = alias_overrides.into_iter().collect();
            alias_overrides.sort_by(|(left_id, _), (right_id, _)| {
                let left_is_exact = left_id.contains(":source-sha256-");
                let right_is_exact = right_id.contains(":source-sha256-");
                right_is_exact
                    .cmp(&left_is_exact)
                    .then_with(|| left_id.cmp(right_id))
            });
            for (command_id, alias) in alias_overrides {
                let alias_lower = alias.to_lowercase();
                if let Some(existing_path) = self.alias_registry.get(&alias_lower) {
                    let compatibility_shadow = if let (Some(exact), Some(legacy)) = (
                        existing_path.strip_prefix("script/"),
                        command_id.strip_prefix("script/"),
                    ) {
                        self.scripts.iter().any(|script| {
                            script.matches_launcher_command_identifier(exact)
                                && script.matches_launcher_command_identifier(legacy)
                        })
                    } else if let (Some(exact), Some(legacy)) = (
                        existing_path.strip_prefix("scriptlet/"),
                        command_id.strip_prefix("scriptlet/"),
                    ) {
                        self.scriptlets.iter().any(|scriptlet| {
                            scriptlet.matches_launcher_command_identifier(exact)
                                && scriptlet.matches_launcher_command_identifier(legacy)
                        })
                    } else {
                        false
                    };
                    if compatibility_shadow {
                        continue;
                    }
                    conflicts.push(format!(
                        "Alias conflict: '{}' already used by {}",
                        alias,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Conflict: alias '{}' for {} blocked (already used by {})",
                            alias, command_id, existing_path
                        ),
                    );
                } else {
                    // Use the command_id as the path identifier
                    // This allows find_alias_match to find built-ins and apps
                    self.alias_registry.insert(alias_lower, command_id);
                }
            }
        }

        logging::log(
            "REGISTRY",
            &format!(
                "Rebuilt registries: {} aliases, {} shortcuts, {} conflicts",
                self.alias_registry.len(),
                self.shortcut_registry.len(),
                conflicts.len()
            ),
        );

        conflicts
    }

    /// Reset all state and return to the script list view.
    /// This clears all prompt state and resizes the window appropriately.
    pub(crate) fn reset_to_script_list(&mut self, cx: &mut Context<Self>) {
        self.reset_to_script_list_impl(false, cx);
    }

    pub(crate) fn reset_to_script_list_after_agent_chat_prepared(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.reset_to_script_list_impl(true, cx);
    }

    fn reset_to_script_list_impl(
        &mut self,
        agent_chat_already_prepared: bool,
        cx: &mut Context<Self>,
    ) {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        clear_main_state_restore_after_focus_loss();
        // Any detached floating popups (Agent Chat composer picker state / history popup /
        // menu-syntax trigger picker) are owned by the
        // outgoing view. Returning to the script list abandons that owner,
        // so make sure none of them survive past the transition.
        self.close_floating_popups_for_owner_loss("reset_to_script_list", cx);
        let closing_agent_chat = matches!(self.current_view, AppView::AgentChatView { .. });
        let old_view = match &self.current_view {
            AppView::ScriptList => "ScriptList",
            AppView::About { .. } => "About",
            AppView::ActionsDialog => "ActionsDialog",
            AppView::ArgPrompt { .. } => "ArgPrompt",
            AppView::DivPrompt { .. } => "DivPrompt",
            AppView::FormPrompt { .. } => "FormPrompt",
            AppView::TermPrompt { .. } => "TermPrompt",
            AppView::EditorPrompt { .. } => "EditorPrompt",
            AppView::SelectPrompt { .. } => "SelectPrompt",
            AppView::PathPrompt { .. } => "PathPrompt",
            AppView::EnvPrompt { .. } => "EnvPrompt",
            AppView::DropPrompt { .. } => "DropPrompt",
            AppView::TemplatePrompt { .. } => "TemplatePrompt",
            AppView::HotkeyPrompt { .. } => "HotkeyPrompt",
            AppView::ChatPrompt { .. } => "ChatPrompt",
            AppView::MiniPrompt { .. } => "MiniPrompt",
            AppView::MicroPrompt { .. } => "MicroPrompt",
            AppView::ClipboardHistoryView { .. } => "ClipboardHistoryView",
            AppView::EmojiPickerView { .. } => "EmojiPickerView",
            AppView::AppLauncherView { .. } => "AppLauncherView",
            AppView::WindowSwitcherView { .. } => "WindowSwitcherView",
            AppView::BrowserTabsView { .. } => "BrowserTabsView",
            AppView::ScratchPadView { .. } => "ScratchPadView",
            AppView::QuickTerminalView { .. } => "QuickTerminalView",
            AppView::FlowSessionView { .. } => "FlowSessionView",
            AppView::WebcamView { .. } => "WebcamView",
            AppView::FileSearchView { .. } => "FileSearchView",
            AppView::ProfileSearchView { .. } => "ProfileSearchView",
            AppView::ThemeChooserView { .. } => "ThemeChooserView",
            AppView::CreationFeedback { .. } => "CreationFeedback",
            AppView::NamingPrompt { .. } => "NamingPrompt",
            AppView::BrowseKitsView { .. } => "BrowseKitsView",
            AppView::MigrateV1View { .. } => "MigrateV1View",
            AppView::InstalledKitsView { .. } => "InstalledKitsView",
            AppView::ProcessManagerView { .. } => "ProcessManagerView",
            AppView::FlowUxView { .. } => "FlowUxView",
            AppView::CurrentAppCommandsView { .. } => "CurrentAppCommandsView",
            AppView::SearchAiPresetsView { .. } => "SearchAiPresetsView",
            AppView::CreateAiPresetView { .. } => "CreateAiPresetView",
            AppView::SettingsView { .. } => "SettingsView",
            AppView::PermissionsWizardView { .. } => "PermissionsWizardView",
            AppView::FavoritesBrowseView { .. } => "FavoritesBrowseView",
            AppView::AgentChatHistoryView { .. } => "AgentChatHistoryView",
            AppView::BrowserHistoryView { .. } => "BrowserHistoryView",
            AppView::DictationHistoryView { .. } => "DictationHistoryView",
            AppView::NotesBrowseView { .. } => "NotesBrowseView",
            AppView::AgentChatView { .. } => "AgentChatView",
            AppView::DayPage { .. } => "DayPage",
            AppView::ScriptIssuesView { .. } => "ScriptIssuesView",
            AppView::SdkReferenceView { .. } => "SdkReferenceView",
            AppView::TipsView { .. } => "TipsView",
            AppView::ScriptTemplateCatalogView { .. } => "ScriptTemplateCatalogView",
            AppView::ConfirmPrompt { .. } => "ConfirmPrompt",
        };

        let old_focused_input = self.focused_input;
        logging::log(
            "UI",
            &format!(
                "Resetting to script list (was: {}, focused_input: {:?})",
                old_view, old_focused_input
            ),
        );

        // Belt-and-suspenders: Force-kill the process group using stored PID
        // This runs BEFORE clearing channels to ensure cleanup even if Drop doesn't fire
        if let Some(pid) = self.current_script_pid.take() {
            logging::log(
                "CLEANUP",
                &format!("Force-killing script process group {} during reset", pid),
            );
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &format!("-{}", pid)])
                    .output();
            }
        }

        // Stop process manager refresh if it was running
        self.stop_process_manager_refresh();

        // Day Page autosave is debounced (SAVE_DEBOUNCE_MS) and its trailing
        // flush is driven by render side effects, which stop once the window
        // hides. Every teardown path (blur close, Cmd+W, hidden re-show reset)
        // funnels through here, so flush synchronously before the entity drops
        // or the last ~300ms of typing never lands on disk.
        if let AppView::DayPage { entity, .. } = &self.current_view {
            entity.update(cx, |view, cx| {
                if view.is_dirty() {
                    let saved = view.save_before_unmount(cx);
                    tracing::info!(
                        target: "script_kit::day_page",
                        event = "day_page_flushed_on_reset",
                        saved,
                    );
                }
            });
        }

        // If reset bypasses the normal Agent Chat close button/Escape route, still
        // hide Agent Chat-owned popups before the view is dropped. This covers
        // window hide/reset paths after launcher-triggered "/" opens.
        if !agent_chat_already_prepared {
            if let AppView::AgentChatView { entity } = &self.current_view {
                self.embedded_agent_chat = Some(entity.clone());
                entity.update(cx, |view, cx| {
                    view.prepare_for_host_hide(cx);
                });
            }
        }

        // Reset view
        self.editor_escape_armed_at = None;
        self.transition_current_view_and_rekey_main_automation_surface(AppView::ScriptList);
        self.reset_main_menu_selection_intent();
        self.set_main_window_mode_state_only(MainWindowMode::Mini, cx, "reset_to_script_list");
        // A full reset lands on the launcher root, so there is nowhere left to
        // "go back" to. A stale `opened_from_main_menu` here makes the next
        // Escape on an empty ScriptList run a no-op go_back_or_close (visible
        // as "Escape does nothing, second Escape closes"). Launcher sub-modes
        // that legitimately keep the flag while hosting ScriptList (attachment
        // portals, mini→full Main Window, vault filter) all set it back to
        // `true` after their own view setup, never through this reset.
        self.opened_from_main_menu = false;

        // A force-reset bypasses the portal close paths; drop any
        // ScriptList-hosted spine file portal so `is_in_attachment_portal`
        // cannot stay latched after the window reopens.
        if self.spine_mention_portal_segment.take().is_some() {
            self.attachment_portal_return_view = None;
            self.attachment_portal_return_focus_target = None;
            self.attachment_portal_return_width = None;
            self.attachment_portal_host_snapshot = None;
            self.active_attachment_portal_kind = None;
            tracing::info!(
                target: "script_kit::spine",
                event = "spine_attachment_portal_force_reset",
            );
        }

        if closing_agent_chat {
            self.agent_chat_ready_script_path = None;
            self.agent_chat_footer_dot_status = None;
            self.agent_chat_footer_model_display = None;
            self.agent_chat_footer_snapshot = None;
            self.tab_ai_harness_return_view = None;
            self.tab_ai_harness_return_focus_target = None;
            crate::windows::update_automation_semantic_surface(
                "main",
                Some("scriptList".to_string()),
            );
            self.transition_agent_chat_surface(
                crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::EmbeddedClosed,
            );
        }

        // CRITICAL: Reset focused_input to MainFilter so the cursor appears
        // This was a bug where focused_input could remain as ArgPrompt/None after
        // script exit, causing the cursor to not show in the main filter.
        self.gpui_input_focused = false;
        self.request_script_list_main_filter_focus(cx);
        // Reset placeholder back to the root launcher discovery copy.
        self.pending_placeholder = Some(crate::ROOT_LAUNCHER_PLACEHOLDER.to_string());
        logging::log(
            "FOCUS",
            "Reset focused_input to MainFilter for cursor display",
        );

        // Clear arg prompt state
        self.arg_input.clear();
        self.set_arg_selected_index(0);
        // P0: Reset arg scroll handle
        self.arg_list_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);

        // Clear filter and selection state for fresh menu
        self.reset_script_list_filter_and_selection_state(cx);

        // Clear favorites filter
        self.active_favorites = None;

        // NOTE: Window resize is NOT done here to avoid RefCell borrow conflicts.
        // Callers that need resize should use deferred resize via window_ops::queue_resize
        // after the update closure completes. The show_main_window_helper handles this
        // for the visibility flow. Other callers rely on next render to resize.

        // Clear output
        self.last_output = None;

        // Clear channels (they will be dropped, closing the connections)
        self.prompt_receiver = None;
        self.response_sender = self.default_response_sender.clone();

        // Clear script session (parking_lot mutex never poisons)
        *self.script_session.lock() = None;

        // Clear actions popup state (prevents stale actions dialog from persisting)
        self.clear_actions_popup_state();

        // Clear pending path action and close signal
        if let Ok(mut guard) = self.pending_path_action.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.close_path_actions.lock() {
            *guard = false;
        }
        self.flush_pending_main_menu_query(cx);

        logging::log(
            "UI",
            "State reset complete - view is now ScriptList (filter, selection, scroll cleared)",
        );
        cx.notify();
    }

    /// Ensure the selection is at the first selectable item.
    ///
    /// This is a lightweight method that only resets the selection position,
    /// without clearing the filter or other state. Call this when showing
    /// the main menu to ensure the user always starts at the top.
    ///
    /// FIX: Resolves bug where main menu sometimes opened with a random item
    /// selected instead of the first item (e.g., "Reset Window Positions"
    /// instead of "Agent Chat Chat").
    pub fn ensure_selection_at_first_item(&mut self, cx: &mut Context<Self>) {
        // Only reset selection if we're in the script list view
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }

        self.reset_main_menu_selection_intent();
        // Invalidate cache to ensure fresh data
        self.invalidate_grouped_cache();
        self.flush_pending_main_menu_query(cx);
        self.sync_list_state();

        // Reset selection to first item
        self.hovered_index = None; // Reset hover state to prevent stale highlight on reopen
        self.validate_selection_bounds(cx);
        self.reset_main_menu_viewport_intent();

        // Scroll to top
        self.main_list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.),
        });
        self.last_scrolled_index = Some(self.selected_index);

        logging::log(
            "UI",
            &format!(
                "Selection reset to first item: selected_index={}, hovered_index=None",
                self.selected_index
            ),
        );
        cx.notify();
    }
}

#[cfg(test)]
mod registry_conflict_hud_tests {
    use super::split_unannounced_registry_conflicts;
    use std::collections::HashSet;

    fn conflicts(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// A standing conflict (e.g. two scriptlets both binding ctrl+shift+t)
    /// is re-detected by every watcher refresh; it must toast at most once,
    /// not once per refresh.
    #[test]
    fn persistent_conflict_announces_once() {
        let announced = HashSet::new();
        let (fresh, announced) = split_unannounced_registry_conflicts(
            &announced,
            conflicts(&["Shortcut conflict: 'ctrl shift t' already used by main.md"]),
        );
        assert_eq!(fresh.len(), 1);

        let (fresh, _) = split_unannounced_registry_conflicts(
            &announced,
            conflicts(&["Shortcut conflict: 'ctrl shift t' already used by main.md"]),
        );
        assert!(fresh.is_empty(), "repeat refresh must not re-toast");
    }

    #[test]
    fn new_conflict_announces_alongside_existing() {
        let announced: HashSet<String> = conflicts(&["old"]).into_iter().collect();
        let (fresh, _) =
            split_unannounced_registry_conflicts(&announced, conflicts(&["old", "new"]));
        assert_eq!(fresh, conflicts(&["new"]));
    }

    #[test]
    fn resolved_then_reintroduced_conflict_announces_again() {
        let announced: HashSet<String> = conflicts(&["flaky"]).into_iter().collect();
        // Refresh with the conflict resolved: announced set is replaced.
        let (fresh, announced) = split_unannounced_registry_conflicts(&announced, Vec::new());
        assert!(fresh.is_empty());
        // Conflict reappears: it should toast again.
        let (fresh, _) = split_unannounced_registry_conflicts(&announced, conflicts(&["flaky"]));
        assert_eq!(fresh, conflicts(&["flaky"]));
    }
}

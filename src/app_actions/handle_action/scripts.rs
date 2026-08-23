// Script management action handlers for handle_action dispatch.
//
// Contains: create_script, run_script, view_logs, edit_script,
// remove_script/delete_script, reload_scripts, copy_content,
// reset_ranking, settings, quit.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptSourceHandlerAction {
    Edit,
    CopyContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsEditorLaunchPlan {
    ReuseWindowWithProject,
    FileOnlyZed,
    AddToSublimeProject,
    GenericFileOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptManagementHandlerAction {
    CreateScript,
    ReloadScripts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptRemovalHandlerAction {
    MoveToTrash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptRemovalTargetError {
    NoSelection,
    UnsupportedItemType,
    MissingPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptRankingHandlerAction {
    ResetRanking,
}

impl ScriptSourceHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "edit_script" => Some(Self::Edit),
            "copy_content" => Some(Self::CopyContent),
            _ => None,
        }
    }

    fn unsupported_message(self) -> &'static str {
        match self {
            Self::Edit => "Cannot edit this item type",
            Self::CopyContent => "Cannot copy content for this item type",
        }
    }

    fn copied_hud(self) -> &'static str {
        match self {
            Self::CopyContent => "Content copied to clipboard",
            Self::Edit => "Opened in editor",
        }
    }

    fn read_error(self, error: std::io::Error) -> String {
        match self {
            Self::CopyContent => format!("Failed to read file: {error}"),
            Self::Edit => format!("Failed to open file: {error}"),
        }
    }

    fn path_from_result(self, result: &scripts::SearchResult) -> Option<std::path::PathBuf> {
        match (self, result) {
            (Self::Edit, scripts::SearchResult::Script(m)) => Some(m.script.path.clone()),
            (Self::Edit, scripts::SearchResult::Agent(m)) => Some(m.agent.path.clone()),
            (Self::CopyContent, scripts::SearchResult::Script(m)) => Some(m.script.path.clone()),
            (Self::CopyContent, scripts::SearchResult::Agent(m)) => Some(m.agent.path.clone()),
            (Self::CopyContent, scripts::SearchResult::Scriptlet(m)) => m
                .scriptlet
                .file_path
                .as_ref()
                .map(|p| std::path::PathBuf::from(p.split('#').next().unwrap_or(p))),
            _ => None,
        }
    }
}

impl ScriptManagementHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "create_script" => Some(Self::CreateScript),
            "reload_scripts" => Some(Self::ReloadScripts),
            _ => None,
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::CreateScript => "Opened scripts folder",
            Self::ReloadScripts => "Scripts reloaded",
        }
    }

    fn open_failure_message(self, error: impl std::fmt::Display) -> Option<String> {
        match self {
            Self::CreateScript => Some(format!("Failed to open scripts folder: {error}")),
            Self::ReloadScripts => None,
        }
    }
}

impl ScriptRemovalHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "remove_script" | "delete_script" => Some(Self::MoveToTrash),
            _ => None,
        }
    }

    fn target_error_message(
        self,
        error: ScriptRemovalTargetError,
        action_id: &str,
        target: Option<&ScriptRemovalTarget>,
    ) -> String {
        match (self, error) {
            (Self::MoveToTrash, ScriptRemovalTargetError::NoSelection) => {
                selection_required_message_for_action(action_id).to_string()
            }
            (Self::MoveToTrash, ScriptRemovalTargetError::UnsupportedItemType) => {
                "Cannot remove this item type".to_string()
            }
            (Self::MoveToTrash, ScriptRemovalTargetError::MissingPath) => {
                let name = target
                    .map(|target| target.name.as_str())
                    .unwrap_or("Selected item");
                format!("{name} no longer exists")
            }
        }
    }

    fn confirm_title(self) -> &'static str {
        match self {
            Self::MoveToTrash => "Move to Trash",
        }
    }

    fn confirm_body(self, target: &ScriptRemovalTarget) -> String {
        match self {
            Self::MoveToTrash => format!("Move \"{}\" to Trash?", target.name),
        }
    }

    fn success_hud(self, target: &ScriptRemovalTarget) -> String {
        match self {
            Self::MoveToTrash => format!("Moved '{}' to Trash", target.name),
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::MoveToTrash => format!("Failed to remove: {error}"),
        }
    }
}

impl SettingsEditorLaunchPlan {
    fn from_editor(editor: &str) -> Self {
        match editor {
            "code" | "cursor" => Self::ReuseWindowWithProject,
            "zed" => Self::FileOnlyZed,
            "subl" => Self::AddToSublimeProject,
            _ => Self::GenericFileOnly,
        }
    }

    fn spawn(
        self,
        editor: &str,
        config_dir: &str,
        config_file: &str,
    ) -> std::io::Result<std::process::Child> {
        use std::process::Command;
        match self {
            Self::ReuseWindowWithProject => Command::new(editor)
                .arg("-r")
                .arg(config_dir)
                .arg(config_file)
                .spawn(),
            Self::FileOnlyZed => Command::new("zed").arg(config_file).spawn(),
            Self::AddToSublimeProject => Command::new("subl")
                .arg("-a")
                .arg(config_dir)
                .arg(config_file)
                .spawn(),
            Self::GenericFileOnly => Command::new(editor).arg(config_file).spawn(),
        }
    }

    fn success_hud(self, editor: &str) -> String {
        match self {
            Self::ReuseWindowWithProject
            | Self::FileOnlyZed
            | Self::AddToSublimeProject
            | Self::GenericFileOnly => format!("Opening config.ts in {editor}"),
        }
    }

    fn failure_message(self, editor: &str, error: impl std::fmt::Display) -> String {
        match self {
            Self::ReuseWindowWithProject
            | Self::FileOnlyZed
            | Self::AddToSublimeProject
            | Self::GenericFileOnly => {
                format!("Failed to open {editor} for settings: {error}")
            }
        }
    }
}

impl ScriptRankingHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "reset_ranking" => Some(Self::ResetRanking),
            _ => None,
        }
    }

    fn reset_hud(self, script_name: &str) -> String {
        match self {
            Self::ResetRanking => format!("Ranking reset for \"{script_name}\""),
        }
    }

    fn no_ranking_message(self) -> &'static str {
        match self {
            Self::ResetRanking => "Item has no ranking to reset",
        }
    }
}

impl ScriptListApp {
    /// Handle script management actions. Returns `DispatchOutcome` indicating if handled.
    fn handle_script_action(
        &mut self,
        action_id: &str,
        dctx: &DispatchContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let trace_id = &dctx.trace_id;
        match action_id {
            "create_script" => {
                let Some(management_action) =
                    ScriptManagementHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", trace_id = %trace_id, "create script action - opening scripts folder");
                let scripts_dir = crate::script_creation::scripts_dir()
                    .to_string_lossy()
                    .to_string();
                let trace_id = trace_id.to_string();
                let start = std::time::Instant::now();
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            use std::process::Command;
                            Command::new("open").arg(&scripts_dir).spawn()
                        })
                        .await;
                    let _ = this.update(cx, |this, cx| match result {
                        Ok(_) => {
                            tracing::info!(
                                category = "UI",
                                trace_id = %trace_id,
                                status = "completed",
                                duration_ms = start.elapsed().as_millis() as u64,
                                "Async action completed: create_script"
                            );
                            this.show_hud(
                                management_action.success_hud().to_string(),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                            this.hide_main_and_reset(cx);
                        }
                        Err(e) => {
                            tracing::error!(
                                trace_id = %trace_id,
                                status = "failed",
                                duration_ms = start.elapsed().as_millis() as u64,
                                error = %e,
                                "Async action failed: create_script"
                            );
                            if let Some(message) = management_action.open_failure_message(e) {
                                this.show_error_toast(message, cx);
                            }
                        }
                    });
                })
                .detach();
                DispatchOutcome::success()
            }
            "run_script" => {
                tracing::info!(category = "UI", "run script action");
                self.execute_selected(cx);
                DispatchOutcome::success()
            }
            "toggle_info" => {
                tracing::info!(
                    category = "UI",
                    trace_id = %trace_id,
                    event = "toggle_info_action",
                    "Toggle info panel action"
                );
                self.toggle_info_panel(cx);
                DispatchOutcome::success()
            }
            "view_logs" => {
                tracing::info!(category = "UI", "view logs action");
                self.toggle_logs(cx);
                DispatchOutcome::success()
            }
            "edit_script" => {
                let Some(source_action) = ScriptSourceHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "edit script action");
                if let Some(result) = self.get_selected_result() {
                    if let Some(path) = source_action.path_from_result(&result) {
                        let editor_launch_rx =
                            self.launch_editor_with_feedback_async(&path, trace_id);
                        let trace_id = trace_id.to_string();
                        let start = std::time::Instant::now();
                        cx.spawn(async move |this, cx| {
                            let Ok(launch_result) = editor_launch_rx.recv().await else {
                                return;
                            };

                            let _ = this.update(cx, |this, cx| match launch_result {
                                Ok(()) => {
                                    tracing::info!(
                                        trace_id = %trace_id,
                                        status = "completed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        "Async action completed: edit_script"
                                    );
                                    this.hide_main_and_reset(cx);
                                }
                                Err(message) => {
                                    tracing::error!(
                                        trace_id = %trace_id,
                                        status = "failed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        error = %message,
                                        "Async action failed: edit_script"
                                    );
                                    this.show_error_toast_with_code(
                                        message,
                                        Some(crate::action_helpers::ERROR_LAUNCH_FAILED),
                                        cx,
                                    );
                                }
                            });
                        })
                        .detach();
                    } else {
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            source_action.unsupported_message(),
                        );
                    }
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        selection_required_message_for_action(action_id),
                    );
                }
                DispatchOutcome::success()
            }
            "remove_script" | "delete_script" => {
                let Some(removal_action) = ScriptRemovalHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", action = action_id, "action triggered");

                let Some(result) = self.get_selected_result() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        removal_action.target_error_message(
                            ScriptRemovalTargetError::NoSelection,
                            action_id,
                            None,
                        ),
                    );
                };

                let Some(target) = script_removal_target_from_result(&result) else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        removal_action.target_error_message(
                            ScriptRemovalTargetError::UnsupportedItemType,
                            action_id,
                            None,
                        ),
                    );
                };

                if !target.path.exists() {
                    self.refresh_scripts(cx);
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        removal_action.target_error_message(
                            ScriptRemovalTargetError::MissingPath,
                            action_id,
                            Some(&target),
                        ),
                    );
                }

                let body: gpui::SharedString = removal_action.confirm_body(&target).into();

                let trace_id = trace_id.to_string();
                let start = std::time::Instant::now();
                let weak_entity = cx.entity().downgrade();
                let owner = weak_entity.clone();

                crate::confirm::open_parent_confirm_dialog_for_entity(
                    window,
                    cx,
                    owner,
                    crate::confirm::ParentConfirmOptions {
                        title: removal_action.confirm_title().into(),
                        body,
                        confirm_text: removal_action.confirm_title().into(),
                        cancel_text: "Cancel".into(),
                        confirm_variant: gpui_component::button::ButtonVariant::Danger,
                        width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
                    },
                    {
                        let trace_id = trace_id.clone();
                        let target = target.clone();
                        move |_window, cx| {
                            tracing::info!(
                                trace_id = %trace_id,
                                event = "remove_script_confirmed",
                                item_kind = target.item_kind,
                                name = %target.name,
                                "remove_script_confirmed"
                            );
                            let trace_id = trace_id.clone();
                            let target = target.clone();
                            if let Some(entity) = weak_entity.upgrade() {
                                entity.update(cx, move |this, cx| {
                                    match move_path_to_trash(&target.path) {
                                        Ok(()) => {
                                            tracing::info!(
                                                category = "UI",
                                                trace_id = %trace_id,
                                                status = "completed",
                                                duration_ms = start.elapsed().as_millis() as u64,
                                                item_kind = target.item_kind,
                                                name = %target.name,
                                                path = %target.path.display(),
                                                "Async action completed: remove_script"
                                            );
                                            this.refresh_scripts(cx);
                                            this.show_hud(
                                                removal_action.success_hud(&target),
                                                Some(HUD_2200_MS),
                                                cx,
                                            );
                                            this.hide_main_and_reset(cx);
                                            cx.notify();
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                trace_id = %trace_id,
                                                status = "failed",
                                                duration_ms = start.elapsed().as_millis() as u64,
                                                item_kind = target.item_kind,
                                                name = %target.name,
                                                path = %target.path.display(),
                                                error = %e,
                                                "Async action failed: remove_script"
                                            );
                                            this.show_error_toast_with_code(
                                                removal_action.failure_message(e),
                                                Some(crate::action_helpers::ERROR_TRASH_FAILED),
                                                cx,
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    },
                    {
                        let trace_id = trace_id.clone();
                        move |_window, _cx| {
                            tracing::info!(
                                trace_id = %trace_id,
                                status = "cancelled",
                                event = "remove_script_cancelled",
                                "remove_script_cancelled"
                            );
                        }
                    },
                );
                DispatchOutcome::success()
            }
            "reload_scripts" => {
                let Some(management_action) =
                    ScriptManagementHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "reload scripts action");
                self.refresh_scripts(cx);
                self.show_hud(
                    management_action.success_hud().to_string(),
                    Some(HUD_SHORT_MS),
                    cx,
                );
                DispatchOutcome::success()
            }
            "settings" => {
                tracing::info!(category = "UI", "settings action - opening config.ts");

                // Get editor from config
                let editor = self.config.get_editor();
                let config_dir = shellexpand::tilde("~/.scriptkit").to_string();
                let config_file = format!("{}/config.ts", config_dir);

                let editor_for_hud = editor.clone();
                let trace_id = trace_id.to_string();
                let start = std::time::Instant::now();

                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let launch_plan = SettingsEditorLaunchPlan::from_editor(&editor);
                            launch_plan
                                .spawn(&editor, &config_dir, &config_file)
                                .map(|child| (launch_plan, child))
                        })
                        .await;
                    let _ = this.update(cx, |this, cx| match result {
                        Ok((launch_plan, _)) => {
                            tracing::info!(
                                category = "UI",
                                trace_id = %trace_id,
                                status = "completed",
                                duration_ms = start.elapsed().as_millis() as u64,
                                editor = %editor_for_hud,
                                "Async action completed: settings"
                            );
                            this.show_hud(
                                launch_plan.success_hud(&editor_for_hud),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                            this.hide_main_and_reset(cx);
                        }
                        Err(e) => {
                            tracing::error!(
                                trace_id = %trace_id,
                                status = "failed",
                                duration_ms = start.elapsed().as_millis() as u64,
                                editor = %editor_for_hud,
                                error = %e,
                                "Async action failed: settings"
                            );
                            let launch_plan =
                                SettingsEditorLaunchPlan::from_editor(&editor_for_hud);
                            this.show_error_toast(
                                launch_plan.failure_message(&editor_for_hud, e),
                                cx,
                            );
                        }
                    });
                })
                .detach();
                DispatchOutcome::success()
            }
            "open_settings_menu" | "setup_dictation" | "check_permissions" | "sdk_reference" => {
                let builtin_id = match action_id {
                    "open_settings_menu" => "builtin/settings",
                    "setup_dictation" => "builtin/dictation-setup",
                    "check_permissions" => "builtin/check-permissions",
                    _ => "builtin/sdk-reference",
                };
                tracing::info!(category = "UI", builtin_id, "global discovery action");
                match crate::builtins::resolve_builtin_entry(
                    builtin_id,
                    &self.config.get_builtins(),
                ) {
                    Some(entry) => {
                        self.execute_builtin(&entry, cx);
                        DispatchOutcome::success()
                    }
                    None => DispatchOutcome::not_handled(),
                }
            }
            "open_help" => {
                tracing::info!(category = "UI", "open help guide action");

                let editor = self.config.get_editor();
                let kit_dir = shellexpand::tilde("~/.scriptkit").to_string();
                let guide_file = format!("{kit_dir}/GUIDE.md");

                if !std::path::Path::new(&guide_file).exists() {
                    self.show_error_toast(
                        "GUIDE.md not found — run Script Kit setup to restore it".to_string(),
                        cx,
                    );
                    return DispatchOutcome::success();
                }

                let editor_for_hud = editor.clone();
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let launch_plan = SettingsEditorLaunchPlan::from_editor(&editor);
                            launch_plan
                                .spawn(&editor, &kit_dir, &guide_file)
                                .map(|child| (launch_plan, child))
                        })
                        .await;
                    let _ = this.update(cx, |this, cx| match result {
                        Ok((launch_plan, _)) => {
                            this.show_hud(
                                launch_plan.success_hud(&editor_for_hud),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                            this.hide_main_and_reset(cx);
                        }
                        Err(e) => {
                            let launch_plan =
                                SettingsEditorLaunchPlan::from_editor(&editor_for_hud);
                            this.show_error_toast(
                                launch_plan.failure_message(&editor_for_hud, e),
                                cx,
                            );
                        }
                    });
                })
                .detach();
                DispatchOutcome::success()
            }
            "ask_ai_settings" => {
                tracing::info!(category = "UI", "ask AI to change settings action");
                self.open_tab_ai_agent_chat_with_entry_intent(
                    Some("Update my Script Kit settings in ~/.scriptkit/config.ts: ".to_string()),
                    cx,
                );
                DispatchOutcome::success()
            }
            "quit" => {
                tracing::info!(category = "UI", "quit action");

                let owner = cx.entity().downgrade();

                crate::confirm::open_parent_confirm_dialog_for_entity(
                    window,
                    cx,
                    owner,
                    Self::quit_script_kit_confirm_options(),
                    move |_window, cx| {
                        tracing::info!(category = "UI", event = "quit_confirmed", "quit_confirmed");
                        Self::prepare_script_kit_shutdown();
                        cx.quit();
                    },
                    move |_window, _cx| {
                        tracing::info!(category = "UI", event = "quit_cancelled", "quit_cancelled");
                    },
                );

                DispatchOutcome::success()
            }
            "copy_content" => {
                let Some(source_action) = ScriptSourceHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "copy content action");
                if let Some(result) = self.get_selected_result() {
                    if let Some(file_path) = source_action.path_from_result(&result) {
                        // Read the file content
                        match std::fs::read_to_string(&file_path) {
                            Ok(content) => {
                                tracing::info!(category = "UI", path = %file_path.display(), "copying content to clipboard");
                                self.copy_to_clipboard_with_feedback(
                                    &content,
                                    source_action.copied_hud().to_string(),
                                    true,
                                    cx,
                                );
                            }
                            Err(e) => {
                                tracing::error!(path = %file_path.display(), error = %e, "failed to read file");
                                return DispatchOutcome::error(
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    source_action.read_error(e),
                                );
                            }
                        }
                    } else {
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            source_action.unsupported_message(),
                        );
                    }
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        selection_required_message_for_action(action_id),
                    );
                }
                DispatchOutcome::success()
            }
            "reset_ranking" => {
                let Some(ranking_action) = ScriptRankingHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "reset ranking action");
                // Get the frecency path from the focused script info
                if let Some(script_info) = self.get_focused_script_info() {
                    if let Some(ref frecency_path) = script_info.frecency_path {
                        // Remove the frecency entry for this item
                        if self.frecency_store.remove(frecency_path).is_some() {
                            // Save the updated frecency store
                            if let Err(e) = self.frecency_store.save() {
                                tracing::error!(
                                    error = %e,
                                    "failed to save frecency after reset"
                                );
                            }
                            // Invalidate the grouped cache AND refresh scripts to rebuild the list
                            self.invalidate_grouped_cache();
                            self.refresh_scripts(cx);
                            tracing::info!(category = "UI", name = %script_info.name, "reset ranking");
                            self.show_hud(
                                ranking_action.reset_hud(&script_info.name),
                                Some(HUD_MEDIUM_MS),
                                cx,
                            );
                        } else {
                            tracing::info!(category = "UI", frecency_path = %frecency_path, "no frecency entry found");
                            self.show_hud(
                                ranking_action.no_ranking_message().to_string(),
                                Some(HUD_MEDIUM_MS),
                                cx,
                            );
                        }
                    } else {
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            ranking_action.no_ranking_message(),
                        );
                    }
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        selection_required_message_for_action(action_id),
                    );
                }
                // Don't hide main window - stay in the main menu so user can see the change
                DispatchOutcome::success()
            }
            _ => DispatchOutcome::not_handled(),
        }
    }
}

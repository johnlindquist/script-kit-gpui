// Scriptlet-specific action handlers for handle_action dispatch.
//
// Contains: edit_scriptlet, reveal_scriptlet_in_finder, copy_scriptlet_path,
// and dynamic scriptlet_action:* handlers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptletSourceHandlerAction {
    Edit,
    RevealInFinder,
    CopyPath,
}

struct ScriptletSourceTarget {
    path: std::path::PathBuf,
    path_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptletSourceTargetError {
    NoSelection,
    NotScriptlet,
    MissingSourcePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScriptletDynamicHandlerAction {
    command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScriptletDynamicExecutionResult {
    Success,
    Failed(String),
    LaunchFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScriptletDynamicFailureDetail {
    Stderr(String),
    Unknown,
}

impl ScriptletSourceHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "edit_scriptlet" => Some(Self::Edit),
            "reveal_scriptlet_in_finder" => Some(Self::RevealInFinder),
            "copy_scriptlet_path" => Some(Self::CopyPath),
            _ => None,
        }
    }

    fn trace_name(self) -> &'static str {
        match self {
            Self::Edit => "edit_scriptlet",
            Self::RevealInFinder => "reveal_scriptlet_in_finder",
            Self::CopyPath => "copy_scriptlet_path",
        }
    }

    fn copied_hud(self, path_text: &str) -> String {
        match self {
            Self::CopyPath => format!("Copied: {path_text}"),
            Self::Edit | Self::RevealInFinder => path_text.to_string(),
        }
    }

    fn reveal_success_hud(self) -> &'static str {
        match self {
            Self::RevealInFinder => "Opened in Finder",
            Self::Edit | Self::CopyPath => "Opened in Finder",
        }
    }

    fn target_error_message(self, error: ScriptletSourceTargetError, action_id: &str) -> String {
        match error {
            ScriptletSourceTargetError::NoSelection => {
                selection_required_message_for_action(action_id).to_string()
            }
            ScriptletSourceTargetError::NotScriptlet => {
                "Selected item is not a scriptlet".to_string()
            }
            ScriptletSourceTargetError::MissingSourcePath => {
                "Scriptlet has no source file path".to_string()
            }
        }
    }
}

impl ScriptletDynamicHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        let command = action_id.strip_prefix("scriptlet_action:")?;
        Some(Self {
            command: command.to_string(),
        })
    }

    fn command(&self) -> &str {
        &self.command
    }

    fn not_found_message(&self) -> &'static str {
        "Scriptlet action not found"
    }
}

impl ScriptletDynamicExecutionResult {
    fn from_exec_result(result: executor::ScriptletResult) -> Self {
        if result.success {
            Self::Success
        } else {
            Self::Failed(ScriptletDynamicFailureDetail::from_stderr(result.stderr).message())
        }
    }

    fn from_launch_error(error: impl std::fmt::Display) -> Self {
        Self::LaunchFailed(error.to_string())
    }

    fn success_hud(&self, action_name: &str) -> Option<String> {
        match self {
            Self::Success => Some(format!("Executed: {action_name}")),
            Self::Failed(_) | Self::LaunchFailed(_) => None,
        }
    }

    fn error_toast(&self) -> Option<String> {
        match self {
            Self::Success => None,
            Self::Failed(message) | Self::LaunchFailed(message) => {
                Some(format!("Failed to execute action: {message}"))
            }
        }
    }
}

impl ScriptletDynamicFailureDetail {
    fn from_stderr(stderr: String) -> Self {
        if stderr.is_empty() {
            Self::Unknown
        } else {
            Self::Stderr(stderr)
        }
    }

    fn message(self) -> String {
        match self {
            Self::Stderr(stderr) => stderr,
            Self::Unknown => "Unknown error".to_string(),
        }
    }
}

fn scriptlet_source_target(
    selected: Option<scripts::SearchResult>,
) -> Result<ScriptletSourceTarget, ScriptletSourceTargetError> {
    let Some(result) = selected else {
        return Err(ScriptletSourceTargetError::NoSelection);
    };
    let scripts::SearchResult::Scriptlet(m) = result else {
        return Err(ScriptletSourceTargetError::NotScriptlet);
    };
    let Some(ref file_path) = m.scriptlet.file_path else {
        return Err(ScriptletSourceTargetError::MissingSourcePath);
    };
    let path_text = file_path
        .split('#')
        .next()
        .unwrap_or(file_path)
        .to_string();
    Ok(ScriptletSourceTarget {
        path: std::path::PathBuf::from(&path_text),
        path_text,
    })
}

impl ScriptListApp {
    /// Handle scriptlet-specific actions. Returns `DispatchOutcome` indicating if handled.
    fn handle_scriptlet_action(
        &mut self,
        action_id: &str,
        dctx: &DispatchContext,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let trace_id = &dctx.trace_id;
        match action_id {
            "edit_scriptlet" | "reveal_scriptlet_in_finder" | "copy_scriptlet_path" => {
                let Some(source_action) = ScriptletSourceHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(
                    category = "UI",
                    action = source_action.trace_name(),
                    "scriptlet source action"
                );
                let target = match scriptlet_source_target(self.get_selected_result()) {
                    Ok(target) => target,
                    Err(error) => {
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            source_action.target_error_message(error, action_id),
                        );
                    }
                };

                match source_action {
                    ScriptletSourceHandlerAction::Edit => {
                        let editor_launch_rx =
                            self.launch_editor_with_feedback_async(&target.path, trace_id);
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
                                        "Async action completed: edit_scriptlet"
                                    );
                                    this.hide_main_and_reset(cx);
                                }
                                Err(message) => {
                                    tracing::error!(
                                        trace_id = %trace_id,
                                        status = "failed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        error = %message,
                                        "Async action failed: edit_scriptlet"
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
                    }
                    ScriptletSourceHandlerAction::RevealInFinder => {
                        let reveal_result_rx =
                            self.reveal_in_finder_with_feedback_async(&target.path, trace_id);
                        let trace_id = trace_id.to_string();
                        let start = std::time::Instant::now();
                        cx.spawn(async move |this, cx| {
                            let Ok(reveal_result) = reveal_result_rx.recv().await else {
                                return;
                            };

                            let _ = this.update(cx, |this, cx| match reveal_result {
                                Ok(()) => {
                                    tracing::info!(
                                        trace_id = %trace_id,
                                        status = "completed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        "Async action completed: reveal_scriptlet_in_finder"
                                    );
                                    this.show_hud(
                                        source_action.reveal_success_hud().to_string(),
                                        Some(HUD_SHORT_MS),
                                        cx,
                                    );
                                    this.hide_main_and_reset(cx);
                                }
                                Err(message) => {
                                    tracing::error!(
                                        trace_id = %trace_id,
                                        status = "failed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        error = %message,
                                        "Async action failed: reveal_scriptlet_in_finder"
                                    );
                                    this.show_error_toast_with_code(
                                        message,
                                        Some(crate::action_helpers::ERROR_REVEAL_FAILED),
                                        cx,
                                    );
                                }
                            });
                        })
                        .detach();
                    }
                    ScriptletSourceHandlerAction::CopyPath => {
                        tracing::info!(category = "UI", path = %target.path_text, "copying scriptlet path to clipboard");
                        self.copy_to_clipboard_with_feedback(
                            &target.path_text,
                            source_action.copied_hud(&target.path_text),
                            true,
                            cx,
                        );
                    }
                }
                DispatchOutcome::success()
            }
            // Handle scriptlet actions defined via H3 headers
            action_id => {
                let Some(dynamic_action) = ScriptletDynamicHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", action = %dynamic_action.command(), "scriptlet action triggered");

                // Find the scriptlet and execute its action
                if let Some(result) = self.get_selected_result() {
                    if let scripts::SearchResult::Scriptlet(scriptlet_match) = result {
                        // Get the file path from the UI scriptlet type
                        let file_path = scriptlet_match.scriptlet.file_path.clone();
                        let scriptlet_command = scriptlet_match.scriptlet.command.clone();

                        // We need to re-parse the markdown file to get the full scriptlet with actions
                        let action_found = if let Some(ref path_with_anchor) = file_path {
                            // Extract just the file path (before #anchor)
                            let file_only = path_with_anchor
                                .split('#')
                                .next()
                                .unwrap_or(path_with_anchor);

                            // Read and parse the markdown file
                            if let Ok(content) = std::fs::read_to_string(file_only) {
                                let parsed_scriptlets = scriptlets::parse_markdown_as_scriptlets(
                                    &content,
                                    Some(file_only),
                                );

                                // Find the matching scriptlet by command
                                let target_command = scriptlet_command.clone().unwrap_or_default();
                                if let Some(full_scriptlet) = parsed_scriptlets
                                    .iter()
                                    .find(|s| s.command == target_command)
                                {
                                    // Find the action in the scriptlet
                                    if let Some(action) = full_scriptlet
                                        .actions
                                        .iter()
                                        .find(|a| a.command == dynamic_action.command())
                                    {
                                        // Create a scriptlet for executing the action
                                        let action_scriptlet = scriptlets::Scriptlet {
                                            name: action.name.clone(),
                                            command: action.command.clone(),
                                            tool: action.tool.clone(),
                                            scriptlet_content: action.code.clone(),
                                            inputs: action.inputs.clone(),
                                            group: full_scriptlet.group.clone(),
                                            preview: None,
                                            metadata: scriptlets::ScriptletMetadata {
                                                shortcut: action.shortcut.clone(),
                                                description: action.description.clone(),
                                                ..Default::default()
                                            },
                                            typed_metadata: None,
                                            schema: None,
                                            kit: full_scriptlet.kit.clone(),
                                            source_path: full_scriptlet.source_path.clone(),
                                            actions: vec![],
                                        };

                                        // Pass the parent scriptlet's content to the action
                                        let mut inputs = std::collections::HashMap::new();
                                        inputs.insert(
                                            "content".to_string(),
                                            full_scriptlet.scriptlet_content.trim().to_string(),
                                        );
                                        let options = executor::ScriptletExecOptions {
                                            inputs,
                                            ..Default::default()
                                        };
                                        let execution_result = match executor::run_scriptlet(
                                            &action_scriptlet,
                                            options,
                                        ) {
                                            Ok(exec_result) => {
                                                ScriptletDynamicExecutionResult::from_exec_result(
                                                    exec_result,
                                                )
                                            }
                                            Err(e) => {
                                                tracing::error!(action = %action.name, error = %e, "failed to execute scriptlet action");
                                                ScriptletDynamicExecutionResult::from_launch_error(
                                                    e,
                                                )
                                            }
                                        };

                                        match &execution_result {
                                            ScriptletDynamicExecutionResult::Success => {
                                                if let Some(message) =
                                                    execution_result.success_hud(&action.name)
                                                {
                                                    tracing::info!(category = "UI", action = %action.name, "scriptlet action executed successfully");
                                                    self.show_hud(message, Some(HUD_MEDIUM_MS), cx);
                                                }
                                            }
                                            ScriptletDynamicExecutionResult::Failed(error_msg)
                                            | ScriptletDynamicExecutionResult::LaunchFailed(
                                                error_msg,
                                            ) => {
                                                tracing::error!(action = %action.name, error = %error_msg, "scriptlet action failed");
                                                if let Some(message) =
                                                    execution_result.error_toast()
                                                {
                                                    self.show_error_toast(message, cx);
                                                }
                                            }
                                        }
                                        self.hide_main_and_reset(cx);
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                tracing::error!(path = %file_only, "failed to read scriptlet file");
                                false
                            }
                        } else {
                            false
                        };

                        if !action_found {
                            tracing::error!(action = %dynamic_action.command(), "scriptlet action not found");
                            return DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                dynamic_action.not_found_message(),
                            );
                        }
                    } else {
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            "Selected item is not a scriptlet",
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
        }
    }
}

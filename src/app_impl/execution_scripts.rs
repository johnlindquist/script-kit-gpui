use super::*;
use std::io::Write;
use std::path::{Path, PathBuf};

const NO_MAIN_WINDOW_BUILTINS: &[&str] = &[
    "builtin/ai-chat",
    "builtin/open-ai",
    "builtin/open-notes",
    "builtin/quick-capture",
    "builtin/new-conversation",
    "builtin/dictation",
    "builtin/dictation-to-ai",
    "builtin/dictation-to-app",
    "builtin/dictation-to-notes",
    "builtin/script-kit-selfie",
];

fn builtin_needs_main_window_for_command_id(identifier: &str) -> bool {
    !NO_MAIN_WINDOW_BUILTINS.contains(&identifier)
}

fn builtin_entry_needs_main_window(entry: &builtins::BuiltInEntry) -> bool {
    match entry.feature {
        builtins::BuiltInFeature::App(_) => false,
        builtins::BuiltInFeature::MenuBarAction(_) => false,
        builtins::BuiltInFeature::SystemAction(_) => false,
        builtins::BuiltInFeature::FrecencyCommand(_) => false,
        builtins::BuiltInFeature::SettingsCommand(command) => match command {
            builtins::SettingsCommandType::ResetWindowPositions => false,
            builtins::SettingsCommandType::ChooseTheme
            | builtins::SettingsCommandType::SelectMicrophone
            | builtins::SettingsCommandType::DictationSetup
            | builtins::SettingsCommandType::ConfigureSnapMode => true,
        },
        builtins::BuiltInFeature::PermissionCommand(command) => match command {
            builtins::PermissionCommandType::CheckPermissions
            | builtins::PermissionCommandType::RequestAccessibility
            | builtins::PermissionCommandType::OpenAccessibilitySettings
            | builtins::PermissionCommandType::AllowAccessibility
            | builtins::PermissionCommandType::AllowScreenRecording => false,
            // The wizard renders inside the main window.
            builtins::PermissionCommandType::SetupPermissions => true,
        },
        builtins::BuiltInFeature::UtilityCommand(command) => match command {
            builtins::UtilityCommandType::StopAllProcesses
            | builtins::UtilityCommandType::ScriptKitSelfie => false,
            builtins::UtilityCommandType::MainWindow
            | builtins::UtilityCommandType::ScratchPad
            | builtins::UtilityCommandType::QuickTerminal
            | builtins::UtilityCommandType::ClaudeCode
            | builtins::UtilityCommandType::ProcessManager
            | builtins::UtilityCommandType::DoInCurrentApp
            | builtins::UtilityCommandType::TurnThisIntoCommand
            | builtins::UtilityCommandType::CurrentAppCommands => true,
        },
        _ => builtin_needs_main_window_for_command_id(&entry.id),
    }
}

fn interactive_script_needs_main_window() -> bool {
    false
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum InteractiveTempFileMode {
    Executable,
    InterpreterFed,
}

#[cfg(unix)]
fn interactive_tempfile_unix_mode(mode: InteractiveTempFileMode) -> u32 {
    match mode {
        InteractiveTempFileMode::Executable => 0o700,
        InteractiveTempFileMode::InterpreterFed => 0o600,
    }
}

#[cfg(unix)]
fn apply_interactive_temp_permissions(
    file: &std::fs::File,
    mode: InteractiveTempFileMode,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let unix_mode = interactive_tempfile_unix_mode(mode);
    let mut permissions = file
        .metadata()
        .map_err(|error| {
            format!(
                "interactive_tempfile_metadata_failed: attempted=read_metadata mode={:o} error={}",
                unix_mode, error
            )
        })?
        .permissions();
    permissions.set_mode(unix_mode);
    file.set_permissions(permissions).map_err(|error| {
        format!(
            "interactive_tempfile_permissions_failed: attempted=set_permissions mode={:o} error={}",
            unix_mode, error
        )
    })
}

pub(crate) fn create_interactive_temp_script(
    content: &str,
    suffix: &str,
    mode: InteractiveTempFileMode,
) -> Result<PathBuf, String> {
    let mut temp_file = tempfile::Builder::new()
        .prefix("scriptlet-")
        .suffix(suffix)
        .tempfile()
        .map_err(|error| {
            format!(
                "interactive_tempfile_create_failed: attempted=create_tempfile suffix={} error={}",
                suffix, error
            )
        })?;

    temp_file
        .as_file_mut()
        .write_all(content.as_bytes())
        .map_err(|error| {
            format!(
                "interactive_tempfile_write_failed: attempted=write_content suffix={} error={}",
                suffix, error
            )
        })?;
    temp_file.as_file_mut().flush().map_err(|error| {
        format!(
            "interactive_tempfile_flush_failed: attempted=flush_content suffix={} error={}",
            suffix, error
        )
    })?;

    #[cfg(unix)]
    apply_interactive_temp_permissions(temp_file.as_file(), mode)?;

    let (_persisted_file, path) = temp_file.keep().map_err(|error| {
        format!(
            "interactive_tempfile_keep_failed: attempted=persist_tempfile suffix={} error={}",
            suffix, error
        )
    })?;

    Ok(path)
}

fn validate_terminal_program(program: &str) -> Result<(), String> {
    let is_safe_program = !program.is_empty()
        && program
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if is_safe_program {
        Ok(())
    } else {
        Err(format!(
            "terminal_program_validation_failed: attempted=build_terminal_command program={} reason=contains_unsafe_characters",
            program
        ))
    }
}

#[cfg(unix)]
fn quote_terminal_arg(arg: &str) -> String {
    let escaped = arg.replace('\'', "'\"'\"'");
    format!("'{}'", escaped)
}

#[cfg(windows)]
fn quote_terminal_arg(arg: &str) -> String {
    // Cmd-compatible quoting: wrap in double quotes and escape internal quotes.
    let escaped = arg.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

fn build_terminal_command(program: &str, script_path: &Path) -> Result<String, String> {
    build_terminal_command_with_args(program, script_path, &[])
}

fn build_terminal_command_with_args(
    program: &str,
    script_path: &Path,
    argv: &[String],
) -> Result<String, String> {
    build_terminal_command_with_env_and_args(program, script_path, &[], argv)
}

fn build_terminal_command_with_env_and_args(
    program: &str,
    script_path: &Path,
    extra_env: &[(String, String)],
    argv: &[String],
) -> Result<String, String> {
    validate_terminal_program(program)?;
    let path_str = script_path.to_str().ok_or_else(|| {
        "terminal_command_build_failed: attempted=convert_path_to_utf8 reason=invalid_utf8"
            .to_string()
    })?;
    let mut command = String::new();
    for (key, value) in extra_env {
        validate_terminal_env_key(key)?;
        command.push_str(key);
        command.push('=');
        command.push_str(&quote_terminal_arg(value));
        command.push(' ');
    }
    command.push_str(program);
    command.push(' ');
    command.push_str(&quote_terminal_arg(path_str));
    for arg in argv {
        command.push(' ');
        command.push_str(&quote_terminal_arg(arg));
    }
    Ok(command)
}

fn validate_terminal_env_key(key: &str) -> Result<(), String> {
    let is_safe_key = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if is_safe_key {
        Ok(())
    } else {
        Err(format!(
            "terminal_env_key_validation_failed: attempted=build_terminal_command key={} reason=contains_unsafe_characters",
            key
        ))
    }
}

fn scriptlet_plugin_source(scriptlet: &scripts::Scriptlet) -> String {
    scriptlet
        .plugin_title
        .clone()
        .or_else(|| {
            if scriptlet.plugin_id.is_empty() {
                scriptlet.group.clone()
            } else {
                Some(scriptlet.plugin_id.clone())
            }
        })
        .unwrap_or_else(|| "Unknown Plugin".to_string())
}

impl ScriptListApp {
    pub(crate) fn execute_scriptlet(
        &mut self,
        scriptlet: &scripts::Scriptlet,
        cx: &mut Context<Self>,
    ) {
        self.execute_scriptlet_with_env_and_args(scriptlet, Vec::new(), Vec::new(), cx);
    }

    pub(crate) fn execute_scriptlet_with_args(
        &mut self,
        scriptlet: &scripts::Scriptlet,
        argv: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.execute_scriptlet_with_env_and_args(scriptlet, Vec::new(), argv, cx);
    }

    pub(crate) fn execute_scriptlet_with_env_and_args(
        &mut self,
        scriptlet: &scripts::Scriptlet,
        extra_env: Vec<(String, String)>,
        argv: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let plugin_source = scriptlet_plugin_source(scriptlet);
        logging::log(
            "EXEC",
            &format!(
                "Executing scriptlet: {} (tool: {}, plugin: {}, argv: {:?})",
                scriptlet.name, scriptlet.tool, plugin_source, argv
            ),
        );

        let tool = scriptlet.tool.to_lowercase();

        // TypeScript/Kit scriptlets need to run interactively (they may use SDK prompts)
        // These should be spawned like regular scripts, not run synchronously
        if matches!(tool.as_str(), "kit" | "ts" | "bun" | "deno" | "js") {
            logging::log(
                "EXEC",
                &format!(
                    "TypeScript scriptlet '{}' - running interactively",
                    scriptlet.name
                ),
            );

            let temp_file = match create_interactive_temp_script(
                &scriptlet.code,
                ".ts",
                InteractiveTempFileMode::InterpreterFed,
            ) {
                Ok(path) => path,
                Err(e) => {
                    logging::log(
                        "ERROR",
                        &format!("Failed to write temp scriptlet file: {}", e),
                    );
                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("{} · {} failed: {}", plugin_source, scriptlet.name, e),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                    return;
                }
            };

            // Create a Script struct and run it interactively
            let script = scripts::Script {
                name: scriptlet.name.clone(),
                description: scriptlet.description.clone(),
                path: temp_file,
                extension: "ts".to_string(),
                icon: None,
                alias: None,
                shortcut: None,
                typed_metadata: None,
                schema: None,
                plugin_id: scriptlet.plugin_id.clone(),
                plugin_title: scriptlet.plugin_title.clone(),
                kit_name: if scriptlet.plugin_id.is_empty() {
                    scriptlet.group.clone()
                } else {
                    Some(scriptlet.plugin_id.clone())
                },
                body: None,
            };

            tracing::info!(
                plugin_id = %script.plugin_id,
                plugin_title = ?script.plugin_title,
                scriptlet = %script.name,
                "interactive_scriptlet_launch"
            );

            self.execute_interactive_with_env_and_args(&script, extra_env, argv, cx);
            return;
        }

        // Shell tools (bash, zsh, sh, fish, etc.) run in the built-in terminal
        // so users can see output interactively
        if scriptlets::SHELL_TOOLS.contains(&tool.as_str()) {
            logging::log(
                "EXEC",
                &format!(
                    "Shell scriptlet '{}' (tool: {}) - running in terminal",
                    scriptlet.name, tool
                ),
            );

            // Write scriptlet code to a temp file and execute it
            let extension = match tool.as_str() {
                "bash" | "zsh" | "sh" => "sh",
                "fish" => "fish",
                "powershell" | "pwsh" => "ps1",
                "cmd" => "bat",
                _ => "sh",
            };
            let suffix = format!(".{}", extension);
            let temp_file = match create_interactive_temp_script(
                &scriptlet.code,
                &suffix,
                InteractiveTempFileMode::Executable,
            ) {
                Ok(path) => path,
                Err(e) => {
                    logging::log(
                        "ERROR",
                        &format!("Failed to write temp extension file: {}", e),
                    );
                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("{} · {} failed: {}", plugin_source, scriptlet.name, e),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                    return;
                }
            };

            match build_terminal_command_with_env_and_args(&tool, &temp_file, &extra_env, &argv) {
                Ok(shell_command) => self.open_terminal_with_command(shell_command, cx),
                Err(e) => {
                    logging::log("ERROR", &format!("Failed to build terminal command: {}", e));
                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("{} · {} failed: {}", plugin_source, scriptlet.name, e),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                }
            }
            return;
        }

        // For other tools (python, ruby, template, etc.), run synchronously
        // These don't use the SDK and won't block waiting for input

        // Convert scripts::Scriptlet to scriptlets::Scriptlet for executor
        let exec_scriptlet = scriptlets::Scriptlet {
            name: scriptlet.name.clone(),
            command: scriptlet.command.clone().unwrap_or_else(|| {
                // Generate command slug from name if not present
                scriptlet.name.to_lowercase().replace(' ', "-")
            }),
            tool: scriptlet.tool.clone(),
            scriptlet_content: scriptlet.code.clone(),
            inputs: vec![], // KNOWN: Runtime args are passed via ScriptletExecOptions; declarative input parsing requires a metadata change.
            group: scriptlet.group.clone().unwrap_or_default(),
            preview: None,
            metadata: scriptlets::ScriptletMetadata {
                shortcut: scriptlet.shortcut.clone(),
                keyword: scriptlet.keyword.clone(),
                description: scriptlet.description.clone(),
                ..Default::default()
            },
            typed_metadata: None,
            schema: None,
            kit: None,
            source_path: scriptlet.file_path.clone(),
            actions: vec![], // Scriptlet actions parsed from H3 headers
        };

        // Execute with default options (no inputs for now)
        let options = executor::ScriptletExecOptions {
            positional_args: argv,
            extra_env,
            ..Default::default()
        };

        match executor::run_scriptlet(&exec_scriptlet, options) {
            Ok(result) => {
                if result.success {
                    logging::log(
                        "EXEC",
                        &format!(
                            "Scriptlet '{}' succeeded: exit={}",
                            scriptlet.name, result.exit_code
                        ),
                    );

                    // Handle special tool types that need interactive prompts
                    if tool == "template" && !result.stdout.is_empty() {
                        let id = format!("scriptlet-template-{}", uuid::Uuid::new_v4());
                        let plan = crate::snippet::analysis::build_hybrid_snippet_plan(
                            &result.stdout,
                            &crate::template_variables::VariableContext::new(),
                        );

                        tracing::info!(
                            category = "EXEC",
                            kind = ?plan.kind,
                            unresolved = ?plan.unresolved_variables,
                            has_explicit_tabstops = plan.has_explicit_tabstops,
                            "Template scriptlet '{}' resolved into hybrid snippet plan",
                            scriptlet.name,
                        );

                        self.handle_prompt_message(
                            PromptMessage::ShowTemplate {
                                id,
                                template: plan.template.clone(),
                            },
                            cx,
                        );
                        return;
                    }

                    // Store output if any
                    if !result.stdout.is_empty() {
                        self.last_output = Some(SharedString::from(result.stdout.clone()));
                    }

                    // Hide window after successful execution
                    script_kit_gpui::set_main_window_visible(false);
                    crate::platform::defer_hide_main_window(cx);
                } else {
                    // Execution failed (non-zero exit code)
                    let error_msg = if !result.stderr.is_empty() {
                        result.stderr.clone()
                    } else {
                        format!("Exit code: {}", result.exit_code)
                    };

                    logging::log(
                        "ERROR",
                        &format!("Scriptlet '{}' failed: {}", scriptlet.name, error_msg),
                    );

                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!(
                                "{} · {} failed: {}",
                                plugin_source, scriptlet.name, error_msg
                            ),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                }
            }
            Err(e) => {
                logging::log(
                    "ERROR",
                    &format!("Failed to execute scriptlet '{}': {}", scriptlet.name, e),
                );

                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("{} · {} failed: {}", plugin_source, scriptlet.name, e),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
                cx.notify();
            }
        }
    }

    /// Execute a script or scriptlet by its file path
    /// Used by global shortcuts to directly invoke scripts
    #[allow(dead_code)]
    pub(crate) fn execute_script_by_path(&mut self, path: &str, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Executing script by path: {}", path));

        // Check if it's a scriptlet (contains #)
        if path.contains('#') {
            // It's a scriptlet path like "/path/to/file.md#command"
            if let Some(scriptlet) = self
                .scriptlets
                .iter()
                .find(|s| s.file_path.as_ref().map(|p| p == path).unwrap_or(false))
            {
                let scriptlet_clone = scriptlet.clone();
                self.execute_scriptlet(&scriptlet_clone, cx);
                return;
            }
            logging::log("ERROR", &format!("Scriptlet not found: {}", path));
            return;
        }

        // It's a regular script - find by path
        if let Some(script) = self
            .scripts
            .iter()
            .find(|s| s.path.to_string_lossy() == path)
        {
            let script_clone = script.clone();
            self.execute_interactive(&script_clone, cx);
            return;
        }

        // Not found in loaded scripts - try to execute directly as a file
        let script_path = std::path::PathBuf::from(path);
        if script_path.exists() {
            let name = script_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("script")
                .to_string();

            let script = scripts::Script {
                name,
                path: script_path.clone(),
                extension: script_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ts")
                    .to_string(),
                description: None,
                icon: None,
                alias: None,
                shortcut: None,
                typed_metadata: None,
                schema: None,
                plugin_id: String::new(),
                plugin_title: None,
                kit_name: None,
                body: None,
            };

            self.execute_interactive(&script, cx);
        } else {
            logging::log("ERROR", &format!("Script file not found: {}", path));
        }
    }

    /// Execute by command ID or legacy file path.
    ///
    /// Command IDs have formats like:
    /// - "scriptlet/my-scriptlet" - execute a scriptlet
    /// - "builtin/ai-chat" - execute a builtin
    /// - "app/com.apple.Finder" - launch an app
    /// - Otherwise: treated as a file path (legacy behavior)
    ///
    /// Returns `true` if the main window should be shown immediately, `false` if not.
    /// Interactive scripts start headless and prompt messages reopen the window on demand.
    /// Apps and certain builtins (Agent Chat, Notes) open their own windows
    /// and don't need the main window.
    pub fn execute_by_command_id_or_path(
        &mut self,
        command_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        logging::log(
            "EXEC",
            &format!("Executing by command ID or path: {}", command_id),
        );

        // Try structured command ID resolution first
        if let Ok((category, identifier)) = crate::config::parse_command_id(command_id) {
            match category {
                crate::config::CommandCategory::Script => {
                    // Plugin-qualified: "script/{plugin_id}:{name}"
                    // Legacy: "script/{name}" (no colon)
                    let script = if let Some((plugin_id, name)) = identifier.split_once(':') {
                        self.scripts.iter().find(|s| {
                            s.name == name
                                && (s.plugin_id == plugin_id
                                    || (s.plugin_id.is_empty()
                                        && s.kit_name.as_deref() == Some(plugin_id)))
                        })
                    } else {
                        self.scripts.iter().find(|s| s.name == identifier)
                    };
                    if let Some(script) = script {
                        tracing::info!(
                            command_id = %command_id,
                            script = %script.name,
                            plugin_id = %script.plugin_id,
                            "script_command_resolved"
                        );
                        let path = script.path.to_string_lossy().to_string();
                        self.execute_script_by_path(&path, cx);
                        return interactive_script_needs_main_window();
                    }
                    tracing::warn!(command_id = %command_id, "script_command_not_found");
                    return false;
                }
                crate::config::CommandCategory::Scriptlet => {
                    logging::bench_log("scriptlet_lookup_start");
                    // Plugin-qualified: "scriptlet/{plugin_id}:{name}"
                    // Legacy: "scriptlet/{name}" (no colon)
                    let scriptlet = if let Some((plugin_id, name)) = identifier.split_once(':') {
                        self.scriptlets.iter().find(|s| {
                            s.name == name
                                && (s.plugin_id == plugin_id
                                    || (s.plugin_id.is_empty()
                                        && s.group.as_deref() == Some(plugin_id)))
                        })
                    } else {
                        self.scriptlets.iter().find(|s| s.name == identifier)
                    };
                    if let Some(scriptlet) = scriptlet {
                        logging::bench_log("scriptlet_found");
                        let scriptlet_clone = scriptlet.clone();
                        tracing::info!(command_id = %command_id, "scriptlet_command_resolved");
                        self.execute_scriptlet(&scriptlet_clone, cx);
                        return false;
                    }
                    tracing::warn!(command_id = %command_id, "scriptlet_command_not_found");
                    return false;
                }
                crate::config::CommandCategory::Builtin => {
                    let canonical_id = crate::config::canonical_builtin_command_id(identifier);
                    let config = crate::config::BuiltInConfig::default();
                    if let Some(entry) = builtins::resolve_builtin_entry(&canonical_id, &config) {
                        tracing::info!(command_id = %canonical_id, "builtin_command_resolved");
                        self.execute_builtin(&entry, cx);
                        return builtin_entry_needs_main_window(&entry);
                    }
                    tracing::warn!(command_id = %canonical_id, "builtin_command_not_found");
                    return false;
                }
                crate::config::CommandCategory::App => {
                    tracing::info!(
                        command_id = %command_id,
                        bundle_id = %identifier,
                        "app_command_resolved"
                    );
                    let apps = crate::app_launcher::get_cached_apps();
                    if let Some(app) = apps
                        .iter()
                        .find(|a| a.bundle_id.as_deref() == Some(identifier))
                    {
                        if let Err(error) = crate::app_launcher::launch_application(app) {
                            tracing::error!(%error, bundle_id = %identifier, "app_command_launch_failed");
                        }
                    } else {
                        tracing::warn!(bundle_id = %identifier, "app_command_not_found");
                    }
                    return false;
                }
                crate::config::CommandCategory::PromptTarget => {
                    let Some(adapter_id) =
                        crate::ai::agent_prompt_handoff::adapter_from_action_id(command_id)
                    else {
                        tracing::warn!(command_id = %command_id, "prompt_target_command_not_found");
                        return true;
                    };
                    match self.launch_prompt_target_from_main_prompt(adapter_id, cx) {
                        Ok(receipt) => {
                            tracing::info!(
                                target: "script_kit::agent_handoff",
                                command_id = %command_id,
                                adapter_id = %receipt.adapter_id,
                                prompt_sha256 = %receipt.prompt_sha256,
                                "prompt_target_command_resolved"
                            );
                        }
                        Err(error) => {
                            self.show_error_toast_with_code(
                                error.user_message(),
                                Some(crate::action_helpers::ERROR_ACTION_FAILED),
                                cx,
                            );
                        }
                    }
                    return true;
                }
                crate::config::CommandCategory::PromptAction => {
                    let Some(prompt_action) =
                        crate::ai::agent_prompt_handoff::prompt_action_from_action_id(command_id)
                    else {
                        tracing::warn!(command_id = %command_id, "prompt_action_command_not_found");
                        return true;
                    };
                    match self.export_prompt_from_main_prompt(prompt_action, cx) {
                        Ok(receipt) => {
                            tracing::info!(
                                target: "script_kit::agent_handoff",
                                command_id = %command_id,
                                export_kind = %receipt.export_kind,
                                prompt_sha256 = %receipt.prompt_sha256,
                                "prompt_action_command_resolved"
                            );
                        }
                        Err(error) => {
                            self.show_error_toast_with_code(
                                error.user_message(),
                                Some(crate::action_helpers::ERROR_ACTION_FAILED),
                                cx,
                            );
                        }
                    }
                    return true;
                }
            }
        }

        // Check if command_id matches a scriptlet by name or file_path
        // Scriptlets don't need immediate window show - they control their own visibility
        if let Some(scriptlet) = self.scriptlets.iter().find(|s| {
            s.name == command_id
                || s.file_path
                    .as_ref()
                    .map(|p| p == command_id)
                    .unwrap_or(false)
        }) {
            logging::log(
                "EXEC",
                &format!("Found scriptlet by name/path: {}", scriptlet.name),
            );
            let scriptlet_clone = scriptlet.clone();
            self.execute_scriptlet(&scriptlet_clone, cx);
            return false; // Scriptlets don't need immediate window show
        }

        // Legacy raw builtin IDs still accepted at the boundary, never re-emitted.
        let canonical_id = crate::config::canonical_builtin_command_id(command_id);
        if canonical_id != command_id {
            let config = crate::config::BuiltInConfig::default();
            if let Some(entry) = builtins::resolve_builtin_entry(&canonical_id, &config) {
                tracing::info!(
                    raw_command_id = %command_id,
                    command_id = %canonical_id,
                    "legacy_builtin_command_resolved"
                );
                self.execute_builtin(&entry, cx);
                return builtin_entry_needs_main_window(&entry);
            }
        }

        // Fall back to path-based execution (legacy behavior).
        // Interactive scripts show the main window only if they later emit a prompt.
        self.execute_script_by_path(command_id, cx);
        interactive_script_needs_main_window()
    }
}

#[cfg(test)]
mod builtin_command_window_visibility_tests {
    use super::{
        build_terminal_command, build_terminal_command_with_env_and_args,
        builtin_entry_needs_main_window, builtin_needs_main_window_for_command_id,
        create_interactive_temp_script, interactive_script_needs_main_window,
        InteractiveTempFileMode,
    };
    use std::path::Path;

    #[test]
    fn test_builtin_needs_main_window_false_for_open_ai_and_open_notes() {
        assert!(!builtin_needs_main_window_for_command_id("builtin/open-ai"));
        assert!(!builtin_needs_main_window_for_command_id(
            "builtin/open-notes"
        ));
    }

    #[test]
    fn test_builtin_needs_main_window_true_for_unlisted_builtin() {
        assert!(builtin_needs_main_window_for_command_id(
            "builtin/refresh-scripts"
        ));
    }

    #[test]
    fn test_system_action_builtins_do_not_show_main_window_after_hotkey_execution() {
        let config = crate::config::BuiltInConfig::default();
        let volume = crate::builtins::resolve_builtin_entry("builtin/volume-0", &config)
            .expect("volume builtin should resolve");
        assert!(
            !builtin_entry_needs_main_window(&volume),
            "Volume shortcuts should execute headlessly and rely on their HUD, not open the main menu"
        );
    }

    #[test]
    fn test_headless_builtin_commands_do_not_show_main_window_after_hotkey_execution() {
        let entry = crate::builtins::BuiltInEntry {
            id: "builtin/reset-window-positions".to_string(),
            name: "Reset Window Positions".to_string(),
            description: "Restore all windows to default positions".to_string(),
            keywords: vec!["reset".to_string(), "window".to_string(), "position".to_string()],
            feature: crate::builtins::BuiltInFeature::SettingsCommand(
                crate::builtins::SettingsCommandType::ResetWindowPositions,
            ),
            icon: Some("rotate-ccw".to_string()),
            group: crate::builtins::BuiltInGroup::Core,
        };
        assert!(
            !builtin_entry_needs_main_window(&entry),
            "reset-window-positions should execute headlessly from a shortcut"
        );
    }

    #[test]
    fn test_interactive_script_does_not_need_immediate_main_window() {
        assert!(!interactive_script_needs_main_window());
    }

    #[test]
    fn test_build_terminal_command_quotes_path_when_path_contains_single_quote() {
        #[cfg(unix)]
        {
            let command = build_terminal_command("bash", Path::new("/tmp/it'works.sh"))
                .expect("valid command");
            assert_eq!(command, "bash '/tmp/it'\"'\"'works.sh'");
        }
    }

    #[test]
    fn test_build_terminal_command_rejects_unsafe_program_value() {
        let err = build_terminal_command("bash;rm", Path::new("/tmp/script.sh"))
            .expect_err("unsafe program should be rejected");
        assert!(
            err.contains("terminal_program_validation_failed"),
            "expected validation error, got: {}",
            err
        );
    }

    #[test]
    fn test_build_terminal_command_prefixes_extra_env_and_args() {
        #[cfg(unix)]
        {
            let command = build_terminal_command_with_env_and_args(
                "bash",
                Path::new("/tmp/script.sh"),
                &[(
                    "KIT_MENU_SYNTAX_COMMAND_TAGS".to_string(),
                    r#"["release"]"#.to_string(),
                )],
                &["--dry-run".to_string()],
            )
            .expect("valid terminal command");
            assert_eq!(
                command,
                "KIT_MENU_SYNTAX_COMMAND_TAGS='[\"release\"]' bash '/tmp/script.sh' '--dry-run'"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_create_interactive_temp_script_sets_mode_700_when_executable() {
        use std::os::unix::fs::PermissionsExt;

        let path = create_interactive_temp_script(
            "echo secure-tempfiles",
            ".sh",
            InteractiveTempFileMode::Executable,
        )
        .expect("should create executable temp file");

        let mode = std::fs::metadata(&path)
            .expect("temp file metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700, "expected secure executable mode 0o700");

        std::fs::remove_file(&path).expect("test temp file should be removable");
    }

    #[cfg(unix)]
    #[test]
    fn test_create_interactive_temp_script_sets_mode_600_when_interpreter_fed() {
        use std::os::unix::fs::PermissionsExt;

        let path = create_interactive_temp_script(
            "console.log('secure-tempfiles')",
            ".ts",
            InteractiveTempFileMode::InterpreterFed,
        )
        .expect("should create interpreter temp file");

        let mode = std::fs::metadata(&path)
            .expect("temp file metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected secure interpreter mode 0o600");

        std::fs::remove_file(&path).expect("test temp file should be removable");
    }
}

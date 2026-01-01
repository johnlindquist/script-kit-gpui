// Prompt message handling methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs

impl ScriptListApp {
    /// Handle a prompt message from the script
    fn handle_prompt_message(&mut self, msg: PromptMessage, cx: &mut Context<Self>) {
        match msg {
            PromptMessage::ShowArg {
                id,
                placeholder,
                choices,
                actions,
            } => {
                logging::log(
                    "UI",
                    &format!(
                        "Showing arg prompt: {} with {} choices, {} actions",
                        id,
                        choices.len(),
                        actions.as_ref().map(|a| a.len()).unwrap_or(0)
                    ),
                );
                let choice_count = choices.len();

                // If actions were provided, store them in the SDK actions system
                // so they can be triggered via shortcuts and Cmd+K
                if let Some(ref action_list) = actions {
                    // Store SDK actions for trigger_action_by_name lookup
                    self.sdk_actions = Some(action_list.clone());

                    // Register keyboard shortcuts for SDK actions
                    self.action_shortcuts.clear();
                    for action in action_list {
                        if let Some(shortcut) = &action.shortcut {
                            self.action_shortcuts.insert(
                                shortcuts::normalize_shortcut(shortcut),
                                action.name.clone(),
                            );
                        }
                    }
                } else {
                    // Clear any previous SDK actions
                    self.sdk_actions = None;
                    self.action_shortcuts.clear();
                }

                self.current_view = AppView::ArgPrompt {
                    id,
                    placeholder,
                    choices,
                    actions,
                };
                self.arg_input.clear();
                self.arg_selected_index = 0;
                self.focused_input = FocusedInput::ArgPrompt;
                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                defer_resize_to_view(view_type, choice_count, cx);
                cx.notify();
            }
            PromptMessage::ShowDiv {
                id,
                html,
                container_classes,
                actions,
                placeholder: _placeholder, // TODO: render in header
                hint: _hint,               // TODO: render hint
                footer: _footer,           // TODO: render footer
                container_bg,
                container_padding,
                opacity,
            } => {
                logging::log("UI", &format!("Showing div prompt: {}", id));
                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for div prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send div response: {}", e));
                            }
                        }
                    });

                // Create focus handle for div prompt
                let div_focus_handle = cx.focus_handle();

                // Build container options from protocol message
                let container_options = ContainerOptions {
                    background: container_bg,
                    padding: container_padding.and_then(|v| {
                        if v.is_string() && v.as_str() == Some("none") {
                            Some(ContainerPadding::None)
                        } else if let Some(n) = v.as_f64() {
                            Some(ContainerPadding::Pixels(n as f32))
                        } else {
                            v.as_i64().map(|n| ContainerPadding::Pixels(n as f32))
                        }
                    }),
                    opacity,
                    container_classes,
                };

                // Create DivPrompt entity with proper HTML rendering
                let div_prompt = DivPrompt::with_options(
                    id.clone(),
                    html,
                    None, // tailwind param deprecated - use container_classes in options
                    div_focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                    crate::designs::DesignVariant::Default,
                    container_options,
                );

                let entity = cx.new(|_| div_prompt);
                self.current_view = AppView::DivPrompt { id, entity };
                self.focused_input = FocusedInput::None; // DivPrompt has no text input
                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowForm { id, html, actions } => {
                logging::log("UI", &format!("Showing form prompt: {}", id));

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create form field colors from theme
                let colors = FormFieldColors::from_theme(&self.theme);

                // Create FormPromptState entity with parsed fields
                let form_state = FormPromptState::new(id.clone(), html, colors, cx);
                let field_count = form_state.fields.len();
                let entity = cx.new(|_| form_state);

                self.current_view = AppView::FormPrompt { id, entity };
                self.focused_input = FocusedInput::None; // FormPrompt has its own focus handling

                // Resize based on field count (more fields = taller window)
                let view_type = if field_count > 0 {
                    ViewType::ArgPromptWithChoices
                } else {
                    ViewType::DivPrompt
                };
                defer_resize_to_view(view_type, field_count, cx);
                cx.notify();
            }
            PromptMessage::ShowTerm {
                id,
                command,
                actions,
            } => {
                logging::log(
                    "UI",
                    &format!("Showing term prompt: {} (command: {:?})", id, command),
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for terminal
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send terminal response: {}", e),
                                );
                            }
                        }
                    });

                // Get the target height for terminal view
                let term_height = window_resize::layout::MAX_HEIGHT;

                // Create terminal with explicit height - GPUI entities don't inherit parent flex sizing
                match term_prompt::TermPrompt::with_height(
                    id.clone(),
                    command,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                    std::sync::Arc::new(self.config.clone()),
                    Some(term_height),
                ) {
                    Ok(term_prompt) => {
                        let entity = cx.new(|_| term_prompt);
                        self.current_view = AppView::TermPrompt { id, entity };
                        self.focused_input = FocusedInput::None; // Terminal handles its own cursor
                        defer_resize_to_view(ViewType::TermPrompt, 0, cx);
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to create terminal");
                        logging::log("ERROR", &format!("Failed to create terminal: {}", e));
                    }
                }
            }
            PromptMessage::ShowEditor {
                id,
                content,
                language,
                template,
                actions,
            } => {
                logging::log(
                    "UI",
                    &format!(
                        "Showing editor prompt: {} (language: {:?}, template: {})",
                        id,
                        language,
                        template.is_some()
                    ),
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for editor
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send editor response: {}", e),
                                );
                            }
                        }
                    });

                // CRITICAL: Create a SEPARATE focus handle for the editor.
                // Using the parent's focus handle causes keyboard event routing issues
                // because the parent checks is_focused() in its render and both parent
                // and child would be tracking the same handle.
                let editor_focus_handle = cx.focus_handle();

                // Get the target height for editor view
                let editor_height = window_resize::layout::MAX_HEIGHT;

                // Create editor: use with_template if template provided, otherwise with_height
                let editor_prompt = if let Some(template_str) = template {
                    EditorPrompt::with_template(
                        id.clone(),
                        template_str,
                        language.unwrap_or_else(|| "plaintext".to_string()),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::new(self.theme.clone()),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                } else {
                    EditorPrompt::with_height(
                        id.clone(),
                        content.unwrap_or_default(),
                        language.unwrap_or_else(|| "markdown".to_string()),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::new(self.theme.clone()),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                };

                let entity = cx.new(|_| editor_prompt);
                self.current_view = AppView::EditorPrompt {
                    id,
                    entity,
                    focus_handle: editor_focus_handle,
                };
                self.focused_input = FocusedInput::None; // Editor handles its own focus

                defer_resize_to_view(ViewType::EditorPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ScriptExit => {
                logging::log("VISIBILITY", "=== ScriptExit message received ===");
                let was_visible = script_kit_gpui::is_main_window_visible();
                logging::log(
                    "VISIBILITY",
                    &format!("WINDOW_VISIBLE was: {}", was_visible),
                );

                // CRITICAL: Update visibility state so hotkey toggle works correctly
                script_kit_gpui::set_main_window_visible(false);
                logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");

                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                logging::log("VISIBILITY", "NEEDS_RESET set to: true");

                self.reset_to_script_list(cx);
                logging::log("VISIBILITY", "reset_to_script_list() called");

                // Hide window when script completes - scripts only stay active while code is running
                cx.hide();
                logging::log(
                    "VISIBILITY",
                    "cx.hide() called - window hidden on script completion",
                );
            }
            PromptMessage::HideWindow => {
                logging::log("VISIBILITY", "=== HideWindow message received ===");
                let was_visible = script_kit_gpui::is_main_window_visible();
                logging::log(
                    "VISIBILITY",
                    &format!("WINDOW_VISIBLE was: {}", was_visible),
                );

                // CRITICAL: Update visibility state so hotkey toggle works correctly
                script_kit_gpui::set_main_window_visible(false);
                logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");

                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                logging::log("VISIBILITY", "NEEDS_RESET set to: true");

                cx.hide();
                logging::log(
                    "VISIBILITY",
                    "cx.hide() called - window should now be hidden",
                );
            }
            PromptMessage::OpenBrowser { url } => {
                logging::log("UI", &format!("Opening browser: {}", url));
                #[cfg(target_os = "macos")]
                {
                    match std::process::Command::new("open").arg(&url).spawn() {
                        Ok(_) => logging::log(
                            "UI",
                            &format!("Successfully opened URL in browser: {}", url),
                        ),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e))
                        }
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    match std::process::Command::new("xdg-open").arg(&url).spawn() {
                        Ok(_) => logging::log(
                            "UI",
                            &format!("Successfully opened URL in browser: {}", url),
                        ),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e))
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    match std::process::Command::new("cmd")
                        .args(["/C", "start", &url])
                        .spawn()
                    {
                        Ok(_) => logging::log(
                            "UI",
                            &format!("Successfully opened URL in browser: {}", url),
                        ),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e))
                        }
                    }
                }
            }
            PromptMessage::RunScript { path } => {
                logging::log("EXEC", &format!("RunScript command received: {}", path));

                // Create a Script struct from the path
                let script_path = std::path::PathBuf::from(&path);
                let script_name = script_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let extension = script_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ts")
                    .to_string();

                let script = scripts::Script {
                    name: script_name.clone(),
                    description: Some(format!("External script: {}", path)),
                    path: script_path,
                    extension,
                    icon: None,
                    alias: None,
                    shortcut: None,
                    typed_metadata: None,
                    schema: None,
                };

                logging::log("EXEC", &format!("Executing script: {}", script_name));
                self.execute_interactive(&script, cx);
            }
            PromptMessage::ScriptError {
                error_message,
                stderr_output,
                exit_code,
                stack_trace,
                script_path,
                suggestions,
            } => {
                logging::log(
                    "ERROR",
                    &format!(
                        "Script error received: {} (exit code: {:?})",
                        error_message, exit_code
                    ),
                );

                // Create error toast with expandable details
                // Use stderr_output if available, otherwise use stack_trace
                let details_text = stderr_output.clone().or_else(|| stack_trace.clone());
                let toast = Toast::error(error_message.clone(), &self.theme)
                    .details_opt(details_text.clone())
                    .duration_ms(Some(10000)); // 10 seconds for errors

                // Add copy button action if we have stderr/stack trace
                let toast = if let Some(ref trace) = details_text {
                    let trace_clone = trace.clone();
                    toast.action(ToastAction::new(
                        "Copy Error",
                        Box::new(move |_, _, _| {
                            // Copy to clipboard
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(trace_clone.clone());
                                logging::log("UI", "Error copied to clipboard");
                            }
                        }),
                    ))
                } else {
                    toast
                };

                // Log suggestions if present
                if !suggestions.is_empty() {
                    logging::log("ERROR", &format!("Suggestions: {:?}", suggestions));
                }

                // Push toast to manager
                let toast_id = self.toast_manager.push(toast);
                logging::log(
                    "UI",
                    &format!(
                        "Toast created for script error: {} (id: {})",
                        script_path, toast_id
                    ),
                );

                cx.notify();
            }
            PromptMessage::ProtocolError {
                correlation_id,
                summary,
                details,
                severity,
                script_path,
            } => {
                tracing::warn!(
                    correlation_id = %correlation_id,
                    script_path = %script_path,
                    summary = %summary,
                    "Protocol parse issue received"
                );

                let mut toast = Toast::from_severity(summary.clone(), severity, &self.theme)
                    .details_opt(details.clone())
                    .duration_ms(Some(8000));

                if let Some(ref detail_text) = details {
                    let detail_clone = detail_text.clone();
                    toast = toast.action(ToastAction::new(
                        "Copy Details",
                        Box::new(move |_, _, _| {
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(detail_clone.clone());
                            }
                        }),
                    ));
                }

                self.toast_manager.push(toast);
                cx.notify();
            }
            PromptMessage::UnhandledMessage { message_type } => {
                logging::log(
                    "WARN",
                    &format!("Displaying unhandled message warning: {}", message_type),
                );

                let toast = Toast::warning(
                    format!("'{}' is not yet implemented", message_type),
                    &self.theme,
                )
                .duration_ms(Some(5000));

                self.toast_manager.push(toast);
                cx.notify();
            }
            PromptMessage::GetState { request_id } => {
                logging::log(
                    "UI",
                    &format!("Collecting state for request: {}", request_id),
                );

                // Collect current UI state
                let (
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                ) = match &self.current_view {
                    AppView::ScriptList => {
                        let filtered_len = self.filtered_results().len();
                        let selected_value = if self.selected_index < filtered_len {
                            self.filtered_results()
                                .get(self.selected_index)
                                .map(|r| match r {
                                    scripts::SearchResult::Script(m) => m.script.name.clone(),
                                    scripts::SearchResult::Scriptlet(m) => m.scriptlet.name.clone(),
                                    scripts::SearchResult::BuiltIn(m) => m.entry.name.clone(),
                                    scripts::SearchResult::App(m) => m.app.name.clone(),
                                    scripts::SearchResult::Window(m) => m.window.title.clone(),
                                })
                        } else {
                            None
                        };
                        (
                            "none".to_string(),
                            None,
                            None,
                            self.filter_input.text().to_string(),
                            self.scripts.len()
                                + self.scriptlets.len()
                                + self.builtin_entries.len()
                                + self.apps.len(),
                            filtered_len,
                            self.selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ArgPrompt {
                        id,
                        placeholder,
                        choices,
                        actions: _,
                    } => {
                        let filtered = self.get_filtered_arg_choices(choices);
                        let selected_value = if self.arg_selected_index < filtered.len() {
                            filtered
                                .get(self.arg_selected_index)
                                .map(|c| c.value.clone())
                        } else {
                            None
                        };
                        (
                            "arg".to_string(),
                            Some(id.clone()),
                            Some(placeholder.clone()),
                            self.arg_input.text().to_string(),
                            choices.len(),
                            filtered.len(),
                            self.arg_selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::DivPrompt { id, .. } => (
                        "div".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::FormPrompt { id, .. } => (
                        "form".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::TermPrompt { id, .. } => (
                        "term".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::EditorPrompt { id, .. } => (
                        "editor".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::SelectPrompt { id, .. } => (
                        "select".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::PathPrompt { id, .. } => (
                        "path".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::EnvPrompt { id, .. } => (
                        "env".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::DropPrompt { id, .. } => (
                        "drop".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::TemplatePrompt { id, .. } => (
                        "template".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::ActionsDialog => (
                        "actions".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::ClipboardHistoryView {
                        entries,
                        filter,
                        selected_index,
                    } => {
                        let filtered_count = if filter.is_empty() {
                            entries.len()
                        } else {
                            let filter_lower = filter.to_lowercase();
                            entries
                                .iter()
                                .filter(|e| e.content.to_lowercase().contains(&filter_lower))
                                .count()
                        };
                        (
                            "clipboardHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            entries.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::AppLauncherView {
                        apps,
                        filter,
                        selected_index,
                    } => {
                        let filtered_count = if filter.is_empty() {
                            apps.len()
                        } else {
                            let filter_lower = filter.to_lowercase();
                            apps.iter()
                                .filter(|a| a.name.to_lowercase().contains(&filter_lower))
                                .count()
                        };
                        (
                            "appLauncher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            apps.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::WindowSwitcherView {
                        windows,
                        filter,
                        selected_index,
                    } => {
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
                        (
                            "windowSwitcher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            windows.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::DesignGalleryView {
                        filter,
                        selected_index,
                    } => {
                        let total_items = designs::separator_variations::SeparatorStyle::count()
                            + designs::icon_variations::total_icon_count()
                            + 8
                            + 6; // headers
                        (
                            "designGallery".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total_items,
                            total_items,
                            *selected_index as i32,
                            None,
                        )
                    }
                };

                // Focus state: we use focused_input as a proxy since we don't have Window access here.
                // When window is visible and we're tracking an input, we're focused.
                let window_visible = script_kit_gpui::is_main_window_visible();
                let is_focused = window_visible && self.focused_input != FocusedInput::None;

                // Create the response
                let response = Message::state_result(
                    request_id.clone(),
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                    is_focused,
                    window_visible,
                );

                logging::log(
                    "UI",
                    &format!("Sending state result for request: {}", request_id),
                );

                // Send the response
                if let Some(ref sender) = self.response_sender {
                    if let Err(e) = sender.send(response) {
                        logging::log("ERROR", &format!("Failed to send state result: {}", e));
                    }
                } else {
                    logging::log("ERROR", "No response sender available for state result");
                }
            }
            PromptMessage::ForceSubmit { value } => {
                logging::log(
                    "UI",
                    &format!("ForceSubmit received with value: {:?}", value),
                );

                // Get the current prompt ID and submit the value
                let prompt_id = match &self.current_view {
                    AppView::ArgPrompt { id, .. } => Some(id.clone()),
                    AppView::DivPrompt { id, .. } => Some(id.clone()),
                    AppView::FormPrompt { id, .. } => Some(id.clone()),
                    AppView::TermPrompt { id, .. } => Some(id.clone()),
                    AppView::EditorPrompt { id, .. } => Some(id.clone()),
                    _ => None,
                };

                if let Some(id) = prompt_id {
                    // Convert serde_json::Value to String for submission
                    let value_str = match &value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    };

                    logging::log(
                        "UI",
                        &format!(
                            "ForceSubmit: submitting '{}' for prompt '{}'",
                            value_str, id
                        ),
                    );
                    self.submit_prompt_response(id, Some(value_str), cx);
                } else {
                    logging::log(
                        "WARN",
                        "ForceSubmit received but no active prompt to submit to",
                    );
                }
            }
            // ============================================================
            // NEW PROMPT TYPES (scaffolding - TODO: implement full UI)
            // ============================================================
            PromptMessage::ShowPath {
                id,
                start_path,
                hint,
            } => {
                logging::log(
                    "UI",
                    &format!(
                        "Showing path prompt: {} (start: {:?}, hint: {:?})",
                        id, start_path, hint
                    ),
                );

                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        logging::log(
                            "UI",
                            &format!(
                                "PathPrompt submit_callback called: id={}, value={:?}",
                                id, value
                            ),
                        );
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send path response: {}", e));
                            }
                        }
                    });

                // Clone the pending_path_action Arc for the callback
                let pending_path_action_clone = self.pending_path_action.clone();

                let show_actions_callback: std::sync::Arc<dyn Fn(PathInfo) + Send + Sync> =
                    std::sync::Arc::new(move |path_info| {
                        logging::log(
                            "UI",
                            &format!("Path actions requested for: {}", path_info.path),
                        );
                        if let Ok(mut guard) = pending_path_action_clone.lock() {
                            *guard = Some(path_info);
                        }
                    });

                // Clone the close_path_actions Arc for the close callback
                let close_path_actions_clone = self.close_path_actions.clone();

                let close_actions_callback: std::sync::Arc<dyn Fn() + Send + Sync> =
                    std::sync::Arc::new(move || {
                        logging::log("UI", "Path close actions callback triggered");
                        if let Ok(mut guard) = close_path_actions_clone.lock() {
                            *guard = true;
                        }
                    });

                // Clone the path_actions_showing and search_text Arcs for header display
                let path_actions_showing = self.path_actions_showing.clone();
                let path_actions_search_text = self.path_actions_search_text.clone();

                let focus_handle = cx.focus_handle();
                let path_prompt = PathPrompt::new(
                    id.clone(),
                    start_path,
                    hint,
                    focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                )
                .with_show_actions(show_actions_callback)
                .with_close_actions(close_actions_callback)
                .with_actions_showing(path_actions_showing)
                .with_actions_search_text(path_actions_search_text);

                let entity = cx.new(|_| path_prompt);
                self.current_view = AppView::PathPrompt {
                    id,
                    entity,
                    focus_handle,
                };
                self.focused_input = FocusedInput::None;

                // Clear any previous pending action and reset showing state
                if let Ok(mut guard) = self.pending_path_action.lock() {
                    *guard = None;
                }
                if let Ok(mut guard) = self.path_actions_showing.lock() {
                    *guard = false;
                }

                defer_resize_to_view(ViewType::ScriptList, 20, cx);
                cx.notify();
            }
            PromptMessage::ShowEnv {
                id,
                key,
                prompt,
                secret,
            } => {
                tracing::info!(id, key, ?prompt, secret, "ShowEnv received");
                logging::log(
                    "UI",
                    &format!(
                        "ShowEnv prompt received: {} (key: {}, secret: {})",
                        id, key, secret
                    ),
                );

                // Create submit callback for env prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send env response: {}", e));
                            }
                        }
                    });

                // Create EnvPrompt entity
                let focus_handle = self.focus_handle.clone();
                let mut env_prompt = prompts::EnvPrompt::new(
                    id.clone(),
                    key,
                    prompt,
                    secret,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );

                // Check keyring first - if value exists, auto-submit without showing UI
                if env_prompt.check_keyring_and_auto_submit() {
                    logging::log("UI", "EnvPrompt: value found in keyring, auto-submitted");
                    // Don't switch view, the callback already submitted
                    cx.notify();
                    return;
                }

                let entity = cx.new(|_| env_prompt);
                self.current_view = AppView::EnvPrompt { id, entity };
                self.focused_input = FocusedInput::None; // EnvPrompt has its own focus handling

                defer_resize_to_view(ViewType::ArgPromptNoChoices, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowDrop {
                id,
                placeholder,
                hint,
            } => {
                tracing::info!(id, ?placeholder, ?hint, "ShowDrop received");
                logging::log(
                    "UI",
                    &format!(
                        "ShowDrop prompt received: {} (placeholder: {:?})",
                        id, placeholder
                    ),
                );

                // Create submit callback for drop prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send drop response: {}", e));
                            }
                        }
                    });

                // Create DropPrompt entity
                let focus_handle = self.focus_handle.clone();
                let drop_prompt = prompts::DropPrompt::new(
                    id.clone(),
                    placeholder,
                    hint,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );

                let entity = cx.new(|_| drop_prompt);
                self.current_view = AppView::DropPrompt { id, entity };
                self.focused_input = FocusedInput::None;

                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowTemplate { id, template } => {
                tracing::info!(id, template, "ShowTemplate received");
                logging::log(
                    "UI",
                    &format!(
                        "ShowTemplate prompt received: {} (template: {})",
                        id, template
                    ),
                );

                // Create submit callback for template prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send template response: {}", e),
                                );
                            }
                        }
                    });

                // Create TemplatePrompt entity
                let focus_handle = self.focus_handle.clone();
                let template_prompt = prompts::TemplatePrompt::new(
                    id.clone(),
                    template,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );

                let entity = cx.new(|_| template_prompt);
                self.current_view = AppView::TemplatePrompt { id, entity };
                self.focused_input = FocusedInput::None;

                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowSelect {
                id,
                placeholder,
                choices,
                multiple,
            } => {
                tracing::info!(
                    id,
                    ?placeholder,
                    choice_count = choices.len(),
                    multiple,
                    "ShowSelect received"
                );
                logging::log(
                    "UI",
                    &format!(
                        "ShowSelect prompt received: {} ({} choices, multiple: {})",
                        id,
                        choices.len(),
                        multiple
                    ),
                );

                // Create submit callback for select prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send select response: {}", e),
                                );
                            }
                        }
                    });

                // Create SelectPrompt entity
                let choice_count = choices.len();
                let select_prompt = prompts::SelectPrompt::new(
                    id.clone(),
                    placeholder,
                    choices,
                    multiple,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );
                let entity = cx.new(|_| select_prompt);
                self.current_view = AppView::SelectPrompt { id, entity };
                self.focused_input = FocusedInput::None; // SelectPrompt has its own focus handling

                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                defer_resize_to_view(view_type, choice_count, cx);
                cx.notify();
            }
            PromptMessage::ShowHud { text, duration_ms } => {
                self.show_hud(text, duration_ms, cx);
            }
            PromptMessage::SetInput { text } => {
                self.set_prompt_input(text, cx);
            }
            PromptMessage::SetActions { actions } => {
                logging::log(
                    "ACTIONS",
                    &format!("Received setActions with {} actions", actions.len()),
                );

                // Store SDK actions for trigger_action_by_name lookup
                self.sdk_actions = Some(actions.clone());

                // Build action shortcuts map for keyboard handling
                self.action_shortcuts.clear();
                for action in &actions {
                    if let Some(ref shortcut) = action.shortcut {
                        let normalized = shortcuts::normalize_shortcut(shortcut);
                        logging::log(
                            "ACTIONS",
                            &format!(
                                "Registering action shortcut: '{}' -> '{}' (normalized: '{}')",
                                shortcut, action.name, normalized
                            ),
                        );
                        self.action_shortcuts
                            .insert(normalized, action.name.clone());
                    }
                }

                // Update ActionsDialog if it exists and is open
                if let Some(ref dialog) = self.actions_dialog {
                    dialog.update(cx, |d, _cx| {
                        d.set_sdk_actions(actions);
                    });
                }

                cx.notify();
            }
        }
    }
}

// Actions handling methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs
// Contains: handle_action, trigger_action_by_name

impl ScriptListApp {
    /// Handle action selection from the actions dialog
    fn handle_action(&mut self, action_id: String, cx: &mut Context<Self>) {
        logging::log("UI", &format!("Action selected: {}", action_id));

        // Close the dialog and return to script list
        self.current_view = AppView::ScriptList;

        match action_id.as_str() {
            "create_script" => {
                logging::log("UI", "Create script action - opening scripts folder");
                // Open ~/.kenv/scripts/ in Finder for now (future: create script dialog)
                let scripts_dir = shellexpand::tilde("~/.kenv/scripts").to_string();
                std::thread::spawn(move || {
                    use std::process::Command;
                    match Command::new("open").arg(&scripts_dir).spawn() {
                        Ok(_) => {
                            logging::log("UI", &format!("Opened scripts folder: {}", scripts_dir))
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open scripts folder: {}", e))
                        }
                    }
                });
                self.last_output = Some(SharedString::from("Opened scripts folder"));
                // Hide window after opening folder and set reset flag
                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                NEEDS_RESET.store(true, Ordering::SeqCst);
                cx.hide();
            }
            "run_script" => {
                logging::log("UI", "Run script action");
                self.execute_selected(cx);
            }
            "view_logs" => {
                logging::log("UI", "View logs action");
                self.toggle_logs(cx);
            }
            "reveal_in_finder" => {
                logging::log("UI", "Reveal in Finder action");
                if let Some(result) = self.get_selected_result() {
                    match result {
                        scripts::SearchResult::Script(script_match) => {
                            let path_str = script_match.script.path.to_string_lossy().to_string();
                            std::thread::spawn(move || {
                                use std::process::Command;
                                match Command::new("open").arg("-R").arg(&path_str).spawn() {
                                    Ok(_) => logging::log(
                                        "UI",
                                        &format!("Revealed in Finder: {}", path_str),
                                    ),
                                    Err(e) => logging::log(
                                        "ERROR",
                                        &format!("Failed to reveal in Finder: {}", e),
                                    ),
                                }
                            });
                            self.last_output = Some(SharedString::from("Revealed in Finder"));
                            // Hide window after revealing in Finder and set reset flag
                            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                            NEEDS_RESET.store(true, Ordering::SeqCst);
                            cx.hide();
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot reveal scriptlets in Finder"));
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot reveal built-in features"));
                        }
                        scripts::SearchResult::App(app_match) => {
                            let path_str = app_match.app.path.to_string_lossy().to_string();
                            std::thread::spawn(move || {
                                use std::process::Command;
                                match Command::new("open").arg("-R").arg(&path_str).spawn() {
                                    Ok(_) => logging::log(
                                        "UI",
                                        &format!("Revealed app in Finder: {}", path_str),
                                    ),
                                    Err(e) => logging::log(
                                        "ERROR",
                                        &format!("Failed to reveal app in Finder: {}", e),
                                    ),
                                }
                            });
                            self.last_output = Some(SharedString::from("Revealed app in Finder"));
                            // Hide window after revealing app in Finder and set reset flag
                            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                            NEEDS_RESET.store(true, Ordering::SeqCst);
                            cx.hide();
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot reveal windows in Finder"));
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No item selected"));
                }
            }
            "copy_path" => {
                logging::log("UI", "Copy path action");
                if let Some(result) = self.get_selected_result() {
                    let path_opt = match result {
                        scripts::SearchResult::Script(script_match) => {
                            Some(script_match.script.path.to_string_lossy().to_string())
                        }
                        scripts::SearchResult::App(app_match) => {
                            Some(app_match.app.path.to_string_lossy().to_string())
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot copy scriptlet path"));
                            None
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot copy built-in path"));
                            None
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output = Some(SharedString::from("Cannot copy window path"));
                            None
                        }
                    };

                    if let Some(path_str) = path_opt {
                        // Use pbcopy on macOS for reliable clipboard access
                        #[cfg(target_os = "macos")]
                        {
                            use std::io::Write;
                            use std::process::{Command, Stdio};

                            match Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
                                Ok(mut child) => {
                                    if let Some(ref mut stdin) = child.stdin {
                                        if stdin.write_all(path_str.as_bytes()).is_ok() {
                                            let _ = child.wait();
                                            logging::log(
                                                "UI",
                                                &format!("Copied path to clipboard: {}", path_str),
                                            );
                                            self.last_output = Some(SharedString::from(format!(
                                                "Copied: {}",
                                                path_str
                                            )));
                                        } else {
                                            logging::log(
                                                "ERROR",
                                                "Failed to write to pbcopy stdin",
                                            );
                                            self.last_output =
                                                Some(SharedString::from("Failed to copy path"));
                                        }
                                    }
                                }
                                Err(e) => {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to spawn pbcopy: {}", e),
                                    );
                                    self.last_output =
                                        Some(SharedString::from("Failed to copy path"));
                                }
                            }
                        }

                        // Fallback for non-macOS platforms
                        #[cfg(not(target_os = "macos"))]
                        {
                            use arboard::Clipboard;
                            match Clipboard::new() {
                                Ok(mut clipboard) => match clipboard.set_text(&path_str) {
                                    Ok(_) => {
                                        logging::log(
                                            "UI",
                                            &format!("Copied path to clipboard: {}", path_str),
                                        );
                                        self.last_output = Some(SharedString::from(format!(
                                            "Copied: {}",
                                            path_str
                                        )));
                                    }
                                    Err(e) => {
                                        logging::log(
                                            "ERROR",
                                            &format!("Failed to copy path: {}", e),
                                        );
                                        self.last_output =
                                            Some(SharedString::from("Failed to copy path"));
                                    }
                                },
                                Err(e) => {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to access clipboard: {}", e),
                                    );
                                    self.last_output =
                                        Some(SharedString::from("Failed to access clipboard"));
                                }
                            }
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No item selected"));
                }
            }
            "edit_script" => {
                logging::log("UI", "Edit script action");
                if let Some(result) = self.get_selected_result() {
                    match result {
                        scripts::SearchResult::Script(script_match) => {
                            self.edit_script(&script_match.script.path);
                            // Hide window after opening editor and set reset flag
                            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                            NEEDS_RESET.store(true, Ordering::SeqCst);
                            cx.hide();
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit scriptlets"));
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot edit built-in features"));
                        }
                        scripts::SearchResult::App(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit applications"));
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit windows"));
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No script selected"));
                }
            }
            "reload_scripts" => {
                logging::log("UI", "Reload scripts action");
                self.refresh_scripts(cx);
                self.last_output = Some(SharedString::from("Scripts reloaded"));
            }
            "settings" => {
                logging::log("UI", "Settings action");
                self.last_output = Some(SharedString::from("Settings (TODO)"));
            }
            "quit" => {
                logging::log("UI", "Quit action");
                // Clean up processes and PID file before quitting
                PROCESS_MANAGER.kill_all_processes();
                PROCESS_MANAGER.remove_main_pid();
                cx.quit();
            }
            "__cancel__" => {
                logging::log("UI", "Actions dialog cancelled");
            }
            _ => {
                // Check if this is an SDK action with has_action=true
                if let Some(ref actions) = self.sdk_actions {
                    if let Some(action) = actions.iter().find(|a| a.name == action_id) {
                        if action.has_action {
                            // Send ActionTriggered back to SDK
                            logging::log(
                                "ACTIONS",
                                &format!(
                                    "SDK action with handler: '{}' (has_action=true), sending ActionTriggered",
                                    action_id
                                ),
                            );
                            if let Some(ref sender) = self.response_sender {
                                let msg = protocol::Message::action_triggered(
                                    action_id.clone(),
                                    action.value.clone(),
                                    self.arg_input_text.clone(),
                                );
                                if let Err(e) = sender.send(msg) {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to send ActionTriggered: {}", e),
                                    );
                                }
                            }
                        } else if let Some(ref value) = action.value {
                            // Submit value directly (has_action=false with value)
                            logging::log(
                                "ACTIONS",
                                &format!(
                                    "SDK action without handler: '{}' (has_action=false), submitting value: {:?}",
                                    action_id, value
                                ),
                            );
                            if let Some(ref sender) = self.response_sender {
                                let msg = protocol::Message::Submit {
                                    id: "action".to_string(),
                                    value: Some(value.clone()),
                                };
                                if let Err(e) = sender.send(msg) {
                                    logging::log("ERROR", &format!("Failed to send Submit: {}", e));
                                }
                            }
                        } else {
                            logging::log(
                                "ACTIONS",
                                &format!(
                                    "SDK action '{}' has no value and has_action=false",
                                    action_id
                                ),
                            );
                        }
                    } else {
                        logging::log("UI", &format!("Unknown action: {}", action_id));
                    }
                } else {
                    logging::log("UI", &format!("Unknown action: {}", action_id));
                }
            }
        }

        cx.notify();
    }

    /// Trigger an SDK action by name
    /// Returns true if the action was found and triggered
    fn trigger_action_by_name(&mut self, action_name: &str, cx: &mut Context<Self>) -> bool {
        if let Some(ref actions) = self.sdk_actions {
            if let Some(action) = actions.iter().find(|a| a.name == action_name) {
                logging::log(
                    "ACTIONS",
                    &format!(
                        "Triggering SDK action '{}' via shortcut (has_action={})",
                        action_name, action.has_action
                    ),
                );

                if action.has_action {
                    // Send ActionTriggered back to SDK
                    if let Some(ref sender) = self.response_sender {
                        let msg = protocol::Message::action_triggered(
                            action_name.to_string(),
                            action.value.clone(),
                            self.arg_input_text.clone(),
                        );
                        if let Err(e) = sender.send(msg) {
                            logging::log(
                                "ERROR",
                                &format!("Failed to send ActionTriggered: {}", e),
                            );
                        }
                    }
                } else if let Some(ref value) = action.value {
                    // Submit value directly
                    if let Some(ref sender) = self.response_sender {
                        let msg = protocol::Message::Submit {
                            id: "action".to_string(),
                            value: Some(value.clone()),
                        };
                        if let Err(e) = sender.send(msg) {
                            logging::log("ERROR", &format!("Failed to send Submit: {}", e));
                        }
                    }
                }

                cx.notify();
                return true;
            }
        }
        false
    }
}

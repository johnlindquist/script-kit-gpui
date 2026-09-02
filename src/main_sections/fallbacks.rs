/// Execute a fallback action based on the fallback ID and input text.
///
/// This handles the various fallback action types:
/// - run-in-terminal: Open terminal with command
/// - add-to-notes: Open Notes window with quick capture
/// - copy-to-clipboard: Copy text to clipboard
/// - search-google/search-duckduckgo: Open browser with search URL
/// - open-url: Open the input as a URL
/// - calculate: Evaluate math expression (basic)
/// - open-file: Open file/folder with default app
fn execute_fallback_action(
    app: &mut ScriptListApp,
    fallback_id: &str,
    input: &str,
    _window: &mut Window,
    cx: &mut Context<ScriptListApp>,
) {
    use fallbacks::builtins::{get_builtin_fallbacks, FallbackResult};

    logging::log(
        "FALLBACK",
        &format!("Executing fallback '{}' with input: {}", fallback_id, input),
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
        Ok(result) => {
            let external_effect = match &result {
                FallbackResult::RunTerminal { .. } => {
                    Some(crate::runtime_policy::ExternalEffect::Process)
                }
                FallbackResult::OpenUrl { .. } | FallbackResult::OpenFile { .. } => {
                    Some(crate::runtime_policy::ExternalEffect::OpenExternal)
                }
                FallbackResult::AddNote { .. }
                | FallbackResult::Copy { .. }
                | FallbackResult::Calculate { .. }
                | FallbackResult::SearchFiles { .. }
                | FallbackResult::ExecuteBuiltin { .. }
                | FallbackResult::SendToAiHarness { .. } => None,
            };
            if let Some(effect) = external_effect {
                if let Err(refusal) = crate::runtime_policy::check(effect) {
                    app.show_error_toast(refusal.to_string(), cx);
                    return;
                }
            }
            match result {
                FallbackResult::RunTerminal { command } => {
                    logging::log("FALLBACK", &format!("RunTerminal: {}", command));
                    // Open Terminal.app with the command
                    #[cfg(target_os = "macos")]
                    {
                        // Use AppleScript to open Terminal and run the command
                        let script = format!(
                            r#"tell application "Terminal"
                                activate
                                do script "{}"
                            end tell"#,
                            crate::utils::escape_applescript_string(&command)
                        );
                        match std::process::Command::new("osascript")
                            .arg("-e")
                            .arg(&script)
                            .spawn()
                        {
                            Ok(_) => logging::log("FALLBACK", "Opened Terminal with command"),
                            Err(e) => {
                                logging::log("FALLBACK", &format!("Failed to open Terminal: {}", e))
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        logging::log("FALLBACK", "RunTerminal not implemented for this platform");
                    }
                }

                FallbackResult::AddNote { content } => {
                    logging::log("FALLBACK", &format!("AddNote: {}", content));
                    let receipt = match crate::platform::copy_text(&content) {
                        Ok(receipt) => receipt,
                        Err(error) => {
                            app.show_error_toast(error, cx);
                            return;
                        }
                    };
                    if let Err(e) = notes::open_notes_window(cx) {
                        logging::log("FALLBACK", &format!("Failed to open Notes: {}", e));
                    } else {
                        hud_manager::show_hud(
                            match receipt.destination() {
                                crate::runtime_policy::CopyDestination::SystemClipboard => {
                                    "Text copied - paste into Notes".to_string()
                                }
                                crate::runtime_policy::CopyDestination::OwnedProcessLocal => {
                                    "Text stored in process-local copy sink; Notes opened"
                                        .to_string()
                                }
                            },
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                    }
                }

                FallbackResult::Copy { text } => {
                    logging::log("FALLBACK", &format!("Copy: {} chars", text.len()));
                    match crate::platform::copy_text(&text) {
                        Ok(receipt) => {
                            logging::log("FALLBACK", &receipt.feedback("Text copied".into()))
                        }
                        Err(error) => app.show_error_toast(error, cx),
                    }
                }

                FallbackResult::OpenUrl { url } => {
                    logging::log("FALLBACK", &format!("OpenUrl: {}", url));
                    // Open URL in default browser
                    if let Err(e) = open::that(&url) {
                        logging::log("FALLBACK", &format!("Failed to open URL: {}", e));
                    } else {
                        logging::log("FALLBACK", "URL opened in browser");
                    }
                }

                FallbackResult::Calculate { expression } => {
                    logging::log("FALLBACK", &format!("Calculate: {}", expression));
                    // Basic math evaluation using meval crate
                    match meval::eval_str(&expression) {
                        Ok(result) => {
                            let result_str = result.to_string();
                            logging::log("FALLBACK", &format!("Result: {}", result_str));
                            let receipt = match crate::platform::copy_text(&result_str) {
                                Ok(receipt) => receipt,
                                Err(error) => {
                                    app.show_error_toast(error, cx);
                                    return;
                                }
                            };
                            hud_manager::show_hud(
                                receipt.feedback(format!("= {}", result_str)),
                                Some(HUD_MEDIUM_MS),
                                cx,
                            );
                        }
                        Err(e) => {
                            logging::log("FALLBACK", &format!("Calculation error: {}", e));
                            hud_manager::show_hud(format!("Error: {}", e), Some(HUD_LONG_MS), cx);
                        }
                    }
                }

                FallbackResult::OpenFile { path } => {
                    logging::log("FALLBACK", &format!("OpenFile: {}", path));
                    // Expand ~ to home directory
                    let expanded = shellexpand::tilde(&path).to_string();
                    // Open with default application
                    if let Err(e) = open::that(&expanded) {
                        logging::log("FALLBACK", &format!("Failed to open file: {}", e));
                    } else {
                        logging::log("FALLBACK", "File opened with default application");
                    }
                }

                FallbackResult::SearchFiles { query } => {
                    logging::log("FALLBACK", &format!("SearchFiles: {}", query));
                    app.open_file_search(query, cx);
                }

                FallbackResult::ExecuteBuiltin { builtin_id } => {
                    logging::log(
                        "FALLBACK",
                        &format!("ExecuteBuiltin: builtin_id='{}'", builtin_id),
                    );

                    let builtin_entry = app
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

                    let _outcome = app.execute_builtin_with_query(&entry, Some(input), cx);
                }

                FallbackResult::SendToAiHarness { query } => {
                    logging::log("FALLBACK", &format!("SendToAiHarness: {}", query));
                    let normalized = query.trim().to_string();
                    let intent = if normalized.is_empty() {
                        None
                    } else {
                        Some(normalized)
                    };
                    app.open_tab_ai_agent_chat_with_entry_intent(intent, cx);
                }
            }
        }
        Err(e) => {
            logging::log("FALLBACK", &format!("Fallback execution failed: {}", e));
        }
    }
}

// App execution methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs
// Contains: execute_builtin, execute_app, execute_window_focus

impl ScriptListApp {
    fn execute_builtin(&mut self, entry: &builtins::BuiltInEntry, cx: &mut Context<Self>) {
        logging::log(
            "EXEC",
            &format!("Executing built-in: {} (id: {})", entry.name, entry.id),
        );

        match &entry.feature {
            builtins::BuiltInFeature::ClipboardHistory => {
                logging::log("EXEC", "Opening Clipboard History");
                // Use cached entries for faster loading
                let entries = clipboard_history::get_cached_entries(100);
                logging::log(
                    "EXEC",
                    &format!("Loaded {} clipboard entries (cached)", entries.len()),
                );
                // Initial selected_index should be 0 (first entry)
                // Note: clipboard history uses a flat list without section headers
                self.current_view = AppView::ClipboardHistoryView {
                    entries,
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for clipboard history view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::AppLauncher => {
                logging::log("EXEC", "Opening App Launcher");
                let apps = app_launcher::scan_applications().clone();
                logging::log("EXEC", &format!("Loaded {} applications", apps.len()));
                self.current_view = AppView::AppLauncherView {
                    apps,
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for app launcher view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::App(app_name) => {
                logging::log("EXEC", &format!("Launching app: {}", app_name));
                // Find and launch the specific application
                let apps = app_launcher::scan_applications();
                if let Some(app) = apps.iter().find(|a| a.name == *app_name) {
                    if let Err(e) = app_launcher::launch_application(app) {
                        logging::log("ERROR", &format!("Failed to launch {}: {}", app_name, e));
                        self.last_output = Some(SharedString::from(format!(
                            "Failed to launch: {}",
                            app_name
                        )));
                    } else {
                        logging::log("EXEC", &format!("Launched app: {}", app_name));
                        // Hide window after launching app and set reset flag
                        // so filter_text is cleared when window is shown again
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        cx.hide();
                    }
                } else {
                    logging::log("ERROR", &format!("App not found: {}", app_name));
                    self.last_output =
                        Some(SharedString::from(format!("App not found: {}", app_name)));
                }
                cx.notify();
            }
            builtins::BuiltInFeature::WindowSwitcher => {
                logging::log("EXEC", "Opening Window Switcher");
                // Load windows when view is opened (windows change frequently)
                match window_control::list_windows() {
                    Ok(windows) => {
                        logging::log("EXEC", &format!("Loaded {} windows", windows.len()));
                        self.current_view = AppView::WindowSwitcherView {
                            windows,
                            filter: String::new(),
                            selected_index: 0,
                        };
                        // Use standard height for window switcher view
                        defer_resize_to_view(ViewType::ScriptList, 0, cx);
                    }
                    Err(e) => {
                        logging::log("ERROR", &format!("Failed to list windows: {}", e));
                        self.toast_manager.push(
                            components::toast::Toast::error(
                                format!("Failed to list windows: {}", e),
                                &self.theme,
                            )
                            .duration_ms(Some(5000)),
                        );
                    }
                }
                cx.notify();
            }
            builtins::BuiltInFeature::DesignGallery => {
                logging::log("EXEC", "Opening Design Gallery");
                self.current_view = AppView::DesignGalleryView {
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for design gallery view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::AiChat => {
                logging::log("EXEC", "Opening AI Chat window");
                // Hide the main window (NOT the entire app) and open AI window
                script_kit_gpui::set_main_window_visible(false);
                NEEDS_RESET.store(true, Ordering::SeqCst);
                // Use hide_main_window() to only hide main window, not the whole app
                platform::hide_main_window();
                // Open AI window
                if let Err(e) = ai::open_ai_window(cx) {
                    logging::log("ERROR", &format!("Failed to open AI window: {}", e));
                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("Failed to open AI: {}", e),
                            &self.theme,
                        )
                        .duration_ms(Some(5000)),
                    );
                    cx.notify();
                }
            }
            builtins::BuiltInFeature::Notes => {
                logging::log("EXEC", "Opening Notes window");
                // Hide the main window (NOT the entire app) and open Notes window
                script_kit_gpui::set_main_window_visible(false);
                NEEDS_RESET.store(true, Ordering::SeqCst);
                // Use hide_main_window() to only hide main window, not the whole app
                platform::hide_main_window();
                // Open Notes window
                if let Err(e) = notes::open_notes_window(cx) {
                    logging::log("ERROR", &format!("Failed to open Notes window: {}", e));
                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("Failed to open Notes: {}", e),
                            &self.theme,
                        )
                        .duration_ms(Some(5000)),
                    );
                    cx.notify();
                }
            }
        }
    }

    /// Execute an application directly from the main search results
    fn execute_app(&mut self, app: &app_launcher::AppInfo, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Launching app from search: {}", app.name));

        if let Err(e) = app_launcher::launch_application(app) {
            logging::log("ERROR", &format!("Failed to launch {}: {}", app.name, e));
            self.last_output = Some(SharedString::from(format!(
                "Failed to launch: {}",
                app.name
            )));
            cx.notify();
        } else {
            logging::log("EXEC", &format!("Launched app: {}", app.name));
            // Hide window after launching app and set reset flag
            // so filter_text is cleared when window is shown again
            script_kit_gpui::set_main_window_visible(false);
            NEEDS_RESET.store(true, Ordering::SeqCst);
            cx.hide();
        }
    }

    /// Focus a window from the main search results
    fn execute_window_focus(
        &mut self,
        window: &window_control::WindowInfo,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "EXEC",
            &format!("Focusing window: {} - {}", window.app, window.title),
        );

        if let Err(e) = window_control::focus_window(window.id) {
            logging::log("ERROR", &format!("Failed to focus window: {}", e));
            self.toast_manager.push(
                components::toast::Toast::error(
                    format!("Failed to focus window: {}", e),
                    &self.theme,
                )
                .duration_ms(Some(5000)),
            );
            cx.notify();
        } else {
            logging::log("EXEC", &format!("Focused window: {}", window.title));
            // Hide Script Kit after focusing window and set reset flag
            // so filter_text is cleared when window is shown again
            script_kit_gpui::set_main_window_visible(false);
            NEEDS_RESET.store(true, Ordering::SeqCst);
            cx.hide();
        }
    }
}

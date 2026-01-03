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
            builtins::BuiltInFeature::MenuBarAction(action) => {
                logging::log(
                    "EXEC",
                    &format!(
                        "Executing menu bar action: {} -> {}",
                        action.bundle_id,
                        action.menu_path.join(" → ")
                    ),
                );
                // Execute menu action via accessibility API
                #[cfg(target_os = "macos")]
                {
                    match script_kit_gpui::menu_executor::execute_menu_action(
                        &action.bundle_id,
                        &action.menu_path,
                    ) {
                        Ok(()) => {
                            logging::log("EXEC", "Menu action executed successfully");
                            // Hide window and set reset flag
                            script_kit_gpui::set_main_window_visible(false);
                            NEEDS_RESET.store(true, Ordering::SeqCst);
                            cx.hide();
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Menu action failed: {}", e));
                            self.toast_manager.push(
                                components::toast::Toast::error(
                                    format!("Menu action failed: {}", e),
                                    &self.theme,
                                )
                                .duration_ms(Some(5000)),
                            );
                            cx.notify();
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    logging::log("WARN", "Menu bar actions only supported on macOS");
                    self.toast_manager.push(
                        components::toast::Toast::warning(
                            "Menu bar actions are only supported on macOS",
                            &self.theme,
                        )
                        .duration_ms(Some(3000)),
                    );
                    cx.notify();
                }
            }

            // =========================================================================
            // System Actions
            // =========================================================================
            builtins::BuiltInFeature::SystemAction(action_type) => {
                logging::log(
                    "EXEC",
                    &format!("Executing system action: {:?}", action_type),
                );

                #[cfg(target_os = "macos")]
                {
                    use builtins::SystemActionType;

                    let result = match action_type {
                        // Power management
                        SystemActionType::EmptyTrash => system_actions::empty_trash(),
                        SystemActionType::LockScreen => system_actions::lock_screen(),
                        SystemActionType::Sleep => system_actions::sleep(),
                        SystemActionType::Restart => system_actions::restart(),
                        SystemActionType::ShutDown => system_actions::shut_down(),
                        SystemActionType::LogOut => system_actions::log_out(),

                        // UI controls
                        SystemActionType::ToggleDarkMode => system_actions::toggle_dark_mode(),
                        SystemActionType::ShowDesktop => system_actions::show_desktop(),
                        SystemActionType::MissionControl => system_actions::mission_control(),
                        SystemActionType::Launchpad => system_actions::launchpad(),
                        SystemActionType::ForceQuitApps => system_actions::force_quit_apps(),

                        // Volume controls
                        SystemActionType::VolumeUp => system_actions::volume_up(),
                        SystemActionType::VolumeDown => system_actions::volume_down(),
                        SystemActionType::VolumeMute => system_actions::volume_mute(),

                        // Brightness controls
                        SystemActionType::BrightnessUp => system_actions::brightness_up(),
                        SystemActionType::BrightnessDown => system_actions::brightness_down(),

                        // System utilities
                        SystemActionType::ToggleDoNotDisturb => {
                            system_actions::toggle_do_not_disturb()
                        }
                        SystemActionType::StartScreenSaver => system_actions::start_screen_saver(),

                        // System Preferences
                        SystemActionType::OpenSystemPreferences => {
                            system_actions::open_system_preferences_main()
                        }
                        SystemActionType::OpenPrivacySettings => {
                            system_actions::open_privacy_settings()
                        }
                        SystemActionType::OpenDisplaySettings => {
                            system_actions::open_display_settings()
                        }
                        SystemActionType::OpenSoundSettings => {
                            system_actions::open_sound_settings()
                        }
                        SystemActionType::OpenNetworkSettings => {
                            system_actions::open_network_settings()
                        }
                        SystemActionType::OpenKeyboardSettings => {
                            system_actions::open_keyboard_settings()
                        }
                        SystemActionType::OpenBluetoothSettings => {
                            system_actions::open_bluetooth_settings()
                        }
                        SystemActionType::OpenNotificationsSettings => {
                            system_actions::open_notifications_settings()
                        }
                    };

                    match result {
                        Ok(()) => {
                            logging::log("EXEC", "System action executed successfully");
                            // Hide window for most actions
                            script_kit_gpui::set_main_window_visible(false);
                            NEEDS_RESET.store(true, Ordering::SeqCst);
                            cx.hide();
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("System action failed: {}", e));
                            self.toast_manager.push(
                                components::toast::Toast::error(
                                    format!("System action failed: {}", e),
                                    &self.theme,
                                )
                                .duration_ms(Some(5000)),
                            );
                            cx.notify();
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    logging::log("WARN", "System actions only supported on macOS");
                    self.toast_manager.push(
                        components::toast::Toast::warning(
                            "System actions are only supported on macOS",
                            &self.theme,
                        )
                        .duration_ms(Some(3000)),
                    );
                    cx.notify();
                }
            }

            // =========================================================================
            // Window Actions (for frontmost window)
            // =========================================================================
            builtins::BuiltInFeature::WindowAction(action_type) => {
                logging::log(
                    "EXEC",
                    &format!("Executing window action: {:?}", action_type),
                );

                // Get the frontmost window first
                match window_control::list_windows() {
                    Ok(windows) => {
                        if let Some(frontmost) = windows.first() {
                            use builtins::WindowActionType;
                            use window_control::TilePosition;

                            let result = match action_type {
                                WindowActionType::TileLeft => window_control::tile_window(
                                    frontmost.id,
                                    TilePosition::LeftHalf,
                                ),
                                WindowActionType::TileRight => window_control::tile_window(
                                    frontmost.id,
                                    TilePosition::RightHalf,
                                ),
                                WindowActionType::TileTop => {
                                    window_control::tile_window(frontmost.id, TilePosition::TopHalf)
                                }
                                WindowActionType::TileBottom => window_control::tile_window(
                                    frontmost.id,
                                    TilePosition::BottomHalf,
                                ),
                                WindowActionType::Maximize => {
                                    window_control::maximize_window(frontmost.id)
                                }
                                WindowActionType::Minimize => {
                                    window_control::minimize_window(frontmost.id)
                                }
                            };

                            match result {
                                Ok(()) => {
                                    logging::log("EXEC", "Window action executed successfully");
                                    // Hide window after action
                                    script_kit_gpui::set_main_window_visible(false);
                                    NEEDS_RESET.store(true, Ordering::SeqCst);
                                    cx.hide();
                                }
                                Err(e) => {
                                    logging::log("ERROR", &format!("Window action failed: {}", e));
                                    self.toast_manager.push(
                                        components::toast::Toast::error(
                                            format!("Window action failed: {}", e),
                                            &self.theme,
                                        )
                                        .duration_ms(Some(5000)),
                                    );
                                    cx.notify();
                                }
                            }
                        } else {
                            logging::log("WARN", "No windows available for window action");
                            self.toast_manager.push(
                                components::toast::Toast::warning(
                                    "No windows available",
                                    &self.theme,
                                )
                                .duration_ms(Some(3000)),
                            );
                            cx.notify();
                        }
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
                        cx.notify();
                    }
                }
            }

            // =========================================================================
            // Notes Commands
            // =========================================================================
            builtins::BuiltInFeature::NotesCommand(cmd_type) => {
                logging::log("EXEC", &format!("Executing notes command: {:?}", cmd_type));

                use builtins::NotesCommandType;

                match cmd_type {
                    NotesCommandType::OpenNotes => {
                        // Same as BuiltInFeature::Notes
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
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
                    NotesCommandType::NewNote => {
                        // Open notes and create a new note
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
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
                        // Note: The new note will be created by the Notes window upon opening
                    }
                    NotesCommandType::SearchNotes => {
                        // Open notes (search is built into the notes window)
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
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
                    NotesCommandType::QuickCapture => {
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
                        if let Err(e) = notes::quick_capture(cx) {
                            logging::log("ERROR", &format!("Failed to quick capture: {}", e));
                            self.toast_manager.push(
                                components::toast::Toast::error(
                                    format!("Failed to quick capture: {}", e),
                                    &self.theme,
                                )
                                .duration_ms(Some(5000)),
                            );
                            cx.notify();
                        }
                    }
                }
            }

            // =========================================================================
            // AI Commands
            // =========================================================================
            builtins::BuiltInFeature::AiCommand(cmd_type) => {
                logging::log("EXEC", &format!("Executing AI command: {:?}", cmd_type));

                use builtins::AiCommandType;

                match cmd_type {
                    AiCommandType::OpenAi => {
                        // Same as BuiltInFeature::AiChat
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
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
                    AiCommandType::NewConversation => {
                        // Open AI window (new conversation is default behavior)
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
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
                    AiCommandType::ClearConversation => {
                        // For now, just open AI window
                        // TODO: Add clear conversation functionality to AI window
                        script_kit_gpui::set_main_window_visible(false);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        platform::hide_main_window();
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
                }
            }

            // =========================================================================
            // Script Commands
            // =========================================================================
            builtins::BuiltInFeature::ScriptCommand(cmd_type) => {
                logging::log("EXEC", &format!("Executing script command: {:?}", cmd_type));

                use builtins::ScriptCommandType;

                match cmd_type {
                    ScriptCommandType::NewScript => {
                        // Create a new script and open it in the editor
                        // For now, use a default name - in the future this could prompt
                        match script_creation::create_new_script("untitled") {
                            Ok(path) => {
                                logging::log("EXEC", &format!("Created new script: {:?}", path));
                                // Open in editor
                                if let Err(e) = script_creation::open_in_editor(&path, &self.config)
                                {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to open in editor: {}", e),
                                    );
                                    self.toast_manager.push(
                                        components::toast::Toast::error(
                                            format!(
                                                "Created script but failed to open editor: {}",
                                                e
                                            ),
                                            &self.theme,
                                        )
                                        .duration_ms(Some(5000)),
                                    );
                                } else {
                                    self.toast_manager.push(
                                        components::toast::Toast::success(
                                            "New script created and opened in editor",
                                            &self.theme,
                                        )
                                        .duration_ms(Some(3000)),
                                    );
                                }
                                // Hide window
                                script_kit_gpui::set_main_window_visible(false);
                                NEEDS_RESET.store(true, Ordering::SeqCst);
                                cx.hide();
                            }
                            Err(e) => {
                                logging::log("ERROR", &format!("Failed to create script: {}", e));
                                self.toast_manager.push(
                                    components::toast::Toast::error(
                                        format!("Failed to create script: {}", e),
                                        &self.theme,
                                    )
                                    .duration_ms(Some(5000)),
                                );
                                cx.notify();
                            }
                        }
                    }
                    ScriptCommandType::NewScriptlet => {
                        // Create a new scriptlet and open it in the editor
                        match script_creation::create_new_scriptlet("untitled") {
                            Ok(path) => {
                                logging::log("EXEC", &format!("Created new scriptlet: {:?}", path));
                                // Open in editor
                                if let Err(e) = script_creation::open_in_editor(&path, &self.config)
                                {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to open in editor: {}", e),
                                    );
                                    self.toast_manager.push(
                                        components::toast::Toast::error(
                                            format!(
                                                "Created scriptlet but failed to open editor: {}",
                                                e
                                            ),
                                            &self.theme,
                                        )
                                        .duration_ms(Some(5000)),
                                    );
                                } else {
                                    self.toast_manager.push(
                                        components::toast::Toast::success(
                                            "New scriptlet created and opened in editor",
                                            &self.theme,
                                        )
                                        .duration_ms(Some(3000)),
                                    );
                                }
                                // Hide window
                                script_kit_gpui::set_main_window_visible(false);
                                NEEDS_RESET.store(true, Ordering::SeqCst);
                                cx.hide();
                            }
                            Err(e) => {
                                logging::log(
                                    "ERROR",
                                    &format!("Failed to create scriptlet: {}", e),
                                );
                                self.toast_manager.push(
                                    components::toast::Toast::error(
                                        format!("Failed to create scriptlet: {}", e),
                                        &self.theme,
                                    )
                                    .duration_ms(Some(5000)),
                                );
                                cx.notify();
                            }
                        }
                    }
                }
            }

            // =========================================================================
            // Permission Commands
            // =========================================================================
            builtins::BuiltInFeature::PermissionCommand(cmd_type) => {
                logging::log(
                    "EXEC",
                    &format!("Executing permission command: {:?}", cmd_type),
                );

                use builtins::PermissionCommandType;

                match cmd_type {
                    PermissionCommandType::CheckPermissions => {
                        let status = permissions_wizard::check_all_permissions();
                        if status.all_granted() {
                            self.toast_manager.push(
                                components::toast::Toast::success(
                                    "All permissions granted!",
                                    &self.theme,
                                )
                                .duration_ms(Some(3000)),
                            );
                        } else {
                            let missing: Vec<_> = status
                                .missing_permissions()
                                .iter()
                                .map(|p| p.permission_type.name())
                                .collect();
                            self.toast_manager.push(
                                components::toast::Toast::warning(
                                    format!("Missing permissions: {}", missing.join(", ")),
                                    &self.theme,
                                )
                                .duration_ms(Some(5000)),
                            );
                        }
                        cx.notify();
                    }
                    PermissionCommandType::RequestAccessibility => {
                        let granted = permissions_wizard::request_accessibility_permission();
                        if granted {
                            self.toast_manager.push(
                                components::toast::Toast::success(
                                    "Accessibility permission granted!",
                                    &self.theme,
                                )
                                .duration_ms(Some(3000)),
                            );
                        } else {
                            self.toast_manager.push(
                                components::toast::Toast::warning(
                                    "Accessibility permission not granted. Some features may not work.",
                                    &self.theme,
                                )
                                .duration_ms(Some(5000)),
                            );
                        }
                        cx.notify();
                    }
                    PermissionCommandType::OpenAccessibilitySettings => {
                        if let Err(e) = permissions_wizard::open_accessibility_settings() {
                            logging::log(
                                "ERROR",
                                &format!("Failed to open accessibility settings: {}", e),
                            );
                            self.toast_manager.push(
                                components::toast::Toast::error(
                                    format!("Failed to open settings: {}", e),
                                    &self.theme,
                                )
                                .duration_ms(Some(5000)),
                            );
                        } else {
                            // Hide window after opening settings
                            script_kit_gpui::set_main_window_visible(false);
                            NEEDS_RESET.store(true, Ordering::SeqCst);
                            cx.hide();
                        }
                        cx.notify();
                    }
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

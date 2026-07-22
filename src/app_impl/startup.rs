use super::*;

pub(super) fn calculate_fallback_error_message(expression: &str) -> String {
    format!(
        "Could not evaluate expression \"{}\". Check the syntax and try again.",
        expression
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MainWindowGlobalKeyIntent {
    OpenAgentChatWithCurrentContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MainWindowActionsKeyIntent {
    ToggleActions,
    CloseEmbeddedAgentChatWindow,
}

fn main_window_global_key_intent(
    event: &gpui::KeystrokeEvent,
) -> Option<MainWindowGlobalKeyIntent> {
    let key = event.keystroke.key.as_str();
    let has_shift = event.keystroke.modifiers.shift;

    if crate::ui_foundation::is_key_enter(key)
        && event.keystroke.modifiers.platform
        && !has_shift
        && !event.keystroke.modifiers.alt
        && !event.keystroke.modifiers.control
    {
        return Some(MainWindowGlobalKeyIntent::OpenAgentChatWithCurrentContext);
    }

    None
}

#[inline]
fn is_plain_platform_cmd_w(event: &gpui::KeystrokeEvent) -> bool {
    let key = event.keystroke.key.as_str();
    let modifiers = &event.keystroke.modifiers;
    modifiers.platform
        && !modifiers.shift
        && !modifiers.alt
        && !modifiers.control
        && key.eq_ignore_ascii_case("w")
}

fn main_window_actions_key_intent(
    current_view: &AppView,
    event: &gpui::KeystrokeEvent,
) -> Option<MainWindowActionsKeyIntent> {
    let key = event.keystroke.key.as_str();
    let has_cmd = event.keystroke.modifiers.platform;
    let has_shift = event.keystroke.modifiers.shift;

    if has_cmd && key.eq_ignore_ascii_case("k") && !has_shift {
        return Some(MainWindowActionsKeyIntent::ToggleActions);
    }

    if has_cmd
        && key.eq_ignore_ascii_case("w")
        && !has_shift
        && matches!(current_view, AppView::AgentChatView { .. })
    {
        return Some(MainWindowActionsKeyIntent::CloseEmbeddedAgentChatWindow);
    }

    None
}

impl ScriptListApp {
    fn handle_main_window_global_key_intent(
        &mut self,
        intent: MainWindowGlobalKeyIntent,
        cx: &mut Context<Self>,
    ) -> bool {
        match intent {
            MainWindowGlobalKeyIntent::OpenAgentChatWithCurrentContext => {
                self.try_route_global_cmd_enter_to_agent_chat_context_capture(cx)
            }
        }
    }

    fn handle_main_window_actions_key_intent(
        &mut self,
        intent: MainWindowActionsKeyIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match intent {
            MainWindowActionsKeyIntent::ToggleActions => {
                self.handle_cmd_k_actions_toggle(window, cx)
            }
            MainWindowActionsKeyIntent::CloseEmbeddedAgentChatWindow => {
                tracing::info!(
                    target: "script_kit::keyboard",
                    event = "embedded_agent_chat_cmd_w_close_window",
                );
                logging::log("KEY", "Interceptor: Cmd+W -> close window from Agent Chat");
                self.close_tab_ai_harness_terminal_with_window(window, cx);
                self.close_and_reset_window(cx);
                true
            }
        }
    }

    pub(crate) fn new(
        config: config::Config,
        bun_available: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // The detached chat window code is compiled into the lib, which
        // cannot name ScriptListApp; register the binary-side reattach hook
        // it dispatches through.
        crate::ai::agent_chat::ui::chat_window::register_reattach_into_main_hook(
            Self::reattach_detached_chat_hook,
        );
        // PERF: Parallelize script + scriptlet loading to reduce startup wall time.
        let load_start = std::time::Instant::now();
        let (script_report, scriptlets, scripts_elapsed, scriptlets_elapsed) = std::thread::scope(
            |scope| {
                let scripts_handle = scope.spawn(|| {
                    let start = std::time::Instant::now();
                    let loaded = scripts::read_scripts_report();
                    (loaded, start.elapsed())
                });

                let scriptlets_handle = scope.spawn(|| {
                    let start = std::time::Instant::now();
                    // Use load_scriptlets() to load from all plugins (plugins/*/scriptlets/*.md)
                    // This includes built-in extensions like CleanShot and user extensions
                    let loaded = scripts::load_scriptlets();
                    (loaded, start.elapsed())
                });

                let (script_report, scripts_elapsed) = match scripts_handle.join() {
                    Ok(result) => result,
                    Err(_) => {
                        logging::log(
                            "PERF",
                            "Script loading thread panicked; retrying read_scripts_report synchronously",
                        );
                        let retry_start = std::time::Instant::now();
                        (scripts::read_scripts_report(), retry_start.elapsed())
                    }
                };

                let (scriptlets, scriptlets_elapsed) = match scriptlets_handle.join() {
                    Ok(result) => result,
                    Err(_) => {
                        logging::log(
                            "PERF",
                            "Scriptlet loading thread panicked; retrying load_scriptlets synchronously",
                        );
                        let retry_start = std::time::Instant::now();
                        (scripts::load_scriptlets(), retry_start.elapsed())
                    }
                };

                (
                    script_report,
                    scriptlets,
                    scripts_elapsed,
                    scriptlets_elapsed,
                )
            },
        );

        let scripts: Vec<std::sync::Arc<scripts::Script>> =
            script_report.scripts.iter().cloned().collect();
        let script_validation_report = Some(script_report.validation.clone());

        // Theme cache was initialized earlier in app startup before window creation.
        // Reuse it here so ScriptListApp construction does not re-read theme files
        // or re-run system appearance detection.
        let theme_load_started = std::time::Instant::now();
        let theme = std::sync::Arc::new(theme::get_cached_theme());
        logging::log(
            "PERF",
            &format!(
                "Startup theme reuse: source=cached elapsed_ms={:.2}",
                theme_load_started.elapsed().as_secs_f64() * 1000.0
            ),
        );
        // Config is now passed in from main() to avoid duplicate load (~100-300ms savings)

        // Load frecency data for suggested section tracking
        let suggested_config = config.get_suggested();
        let mut frecency_store = FrecencyStore::with_config(&suggested_config);
        frecency_store.load().ok(); // Ignore errors - starts fresh if file doesn't exist

        // Load built-in entries based on config, filtering out commands hidden via
        // `hiddenCommands` or per-command `commands.*.hidden` overrides.
        let builtin_entries: Vec<_> = builtins::get_builtin_entries(&config.get_builtins())
            .into_iter()
            .filter(|entry| !config.is_command_hidden(&entry.id))
            .collect();

        // Apps are loaded in the background to avoid blocking startup
        // Start with empty list, will be populated asynchronously
        let apps = Vec::new();

        let total_elapsed = load_start.elapsed();
        logging::log(
            "PERF",
            &format!(
                "Startup loading: {:.2}ms total ({} scripts in {:.2}ms, {} scriptlets in {:.2}ms, apps loading in background)",
                total_elapsed.as_secs_f64() * 1000.0,
                scripts.len(),
                scripts_elapsed.as_secs_f64() * 1000.0,
                scriptlets.len(),
                scriptlets_elapsed.as_secs_f64() * 1000.0
            ),
        );
        logging::log(
            "APP",
            &format!(
                "Loaded {} scripts from ~/.scriptkit/plugins/*/scripts",
                scripts.len()
            ),
        );
        logging::log(
            "APP",
            &format!(
                "Loaded {} scriptlets from ~/.scriptkit/plugins/*/scriptlets",
                scriptlets.len()
            ),
        );
        logging::log(
            "APP",
            &format!("Loaded {} built-in features", builtin_entries.len()),
        );
        logging::log("APP", "Applications loading in background...");
        logging::log("APP", "Loaded theme with system appearance detection");
        logging::log(
            "APP",
            &format!(
                "Loaded config: hotkey={:?}+{}, bun_path={:?}",
                config.hotkey.modifiers, config.hotkey.key, config.bun_path
            ),
        );

        // Load apps in background thread to avoid blocking startup
        let app_launcher_enabled = config.get_builtins().app_launcher;
        if app_launcher_enabled {
            // Use an async channel so the UI task can await completion without polling.
            let (tx, rx) =
                async_channel::bounded::<(Vec<app_launcher::AppInfo>, std::time::Duration)>(1);

            // Spawn background thread for app scanning
            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                let apps = app_launcher::scan_applications().clone();
                let elapsed = start.elapsed();
                if tx.send_blocking((apps, elapsed)).is_err() {
                    logging::log(
                        "APP",
                        "Background app loading result dropped: receiver unavailable",
                    );
                }
            });

            // Event-driven receive: no timer wakeups while waiting for app scan completion.
            cx.spawn(async move |this, cx| {
                let Ok((apps, elapsed)) = rx.recv().await else {
                    logging::log(
                        "APP",
                        "Background app loading failed to deliver result: channel closed",
                    );
                    return;
                };

                let app_count = apps.len();
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.apps = apps;
                        // Invalidate caches since apps changed
                        app.main_menu_result_caches.mark_apps_loaded();
                        app.rebuild_root_windows_after_app_icon_cache_update(
                            "apps_loaded_root_windows_icons",
                            cx,
                        );
                        logging::log(
                            "APP",
                            &format!(
                                "Background app loading complete: {} apps in {:.2}ms",
                                app_count,
                                elapsed.as_secs_f64() * 1000.0
                            ),
                        );
                        // CRITICAL: Sync list state after cache invalidation
                        // Without this, the GPUI list component doesn't know
                        // about the new apps and may render stale item counts
                        let old_count = app.main_list_state.item_count();
                        app.sync_list_state();
                        let new_count = app.main_list_state.item_count();
                        app.validate_selection_bounds(cx);
                        logging::log(
                            "APP",
                            &format!(
                                "List state synced after app load: {} -> {} items (filter='{}')",
                                old_count, new_count, app.computed_filter_text
                            ),
                        );
                        cx.notify();
                    })
                });
            })
            .detach();
        }

        #[cfg(not(test))]
        {
            let share_rx = crate::script_sharing::spawn_clipboard_share_watcher();
            cx.spawn(async move |this, cx| {
                while let Ok(import) = share_rx.recv().await {
                    tracing::info!(
                        share_uri = %import.uri,
                        title = %import.bundle.title,
                        kind = ?import.bundle.kind,
                        "clipboard_share_bundle_detected"
                    );
                    script_kit_gpui::request_show_main_window();
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(180))
                        .await;

                    let options = crate::confirm::ParentConfirmOptions {
                        title: import.bundle.prompt_title().into(),
                        body: import.bundle.prompt_body().into(),
                        confirm_text: "Install".into(),
                        cancel_text: "Ignore".into(),
                        ..Default::default()
                    };
                    let trace_id = format!(
                        "share-import-{}-{}",
                        import.bundle.kind.display_name().to_lowercase(),
                        import.bundle.title.to_lowercase().replace(' ', "-")
                    );

                    let confirmed =
                        match crate::confirm::confirm_with_parent_dialog(cx, options, &trace_id)
                            .await
                        {
                            Ok(confirmed) => confirmed,
                            Err(error) => {
                                tracing::error!(
                                    ?error,
                                    title = %import.bundle.title,
                                    "clipboard_share_confirm_failed"
                                );
                                continue;
                            }
                        };
                    if !confirmed {
                        continue;
                    }

                    let install_result =
                        crate::script_sharing::install_share_bundle(&import.bundle);
                    let title = import.bundle.title.clone();
                    let kind = import.bundle.kind.display_name().to_lowercase();
                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| match install_result {
                            Ok(outcome) => {
                                app.refresh_scripts(cx);
                                app.refresh_skills(cx);
                                app.current_view = AppView::ScriptList;
                                app.reset_main_menu_selection_user_moved();
                                app.show_hud(
                                    format!("Installed shared {} into {}", kind, outcome.plugin_id),
                                    Some(2000),
                                    cx,
                                );
                            }
                            Err(error) => {
                                app.show_error_toast(
                                    format!(
                                        "Failed to install shared {} '{}': {}",
                                        kind, title, error
                                    ),
                                    cx,
                                );
                            }
                        })
                    });
                }
            })
            .detach();
        }
        logging::log("UI", "Script Kit logo SVG loaded for header rendering");

        let gpui_input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::ROOT_LAUNCHER_PLACEHOLDER)
                .inline_completion_visible_without_focus(true)
        });
        let gpui_input_subscription = cx.subscribe_in(&gpui_input_state, window, {
            move |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Focus => {
                    this.gpui_input_focused = true;
                    // Set focused_input based on current view
                    if matches!(
                        this.current_view,
                        AppView::MiniPrompt { .. } | AppView::ArgPrompt { .. }
                    ) {
                        this.focused_input = FocusedInput::ArgPrompt;
                    } else {
                        this.focused_input = FocusedInput::MainFilter;
                    }

                    // Close actions popup when main input receives focus
                    // This ensures consistent behavior: clicking the input closes actions
                    // just like pressing Cmd+K would
                    if this.show_actions_popup || is_actions_window_open() {
                        logging::log(
                            "FOCUS",
                            "Main input focused while actions open - closing actions via shared close path",
                        );
                        this.close_actions_popup(ActionsDialogHost::MainList, window, cx);
                    }

                    cx.notify();
                }
                InputEvent::Blur => {
                    this.gpui_input_focused = false;
                    if this.focused_input == FocusedInput::MainFilter {
                        this.focused_input = FocusedInput::None;
                    }
                    cx.notify();
                }
                InputEvent::Change => {
                    let current_value = this.gpui_input_state.read(cx).value().to_string();

                    if matches!(
                        this.current_view,
                        AppView::MiniPrompt { .. } | AppView::ArgPrompt { .. }
                    ) {
                        // Sync text from Input component to arg_input for choice filtering
                        let prev_original_idx = this
                            .filtered_arg_choices()
                            .get(this.arg_selected_index)
                            .map(|(orig_idx, _)| *orig_idx);
                        this.arg_input.set_text(&current_value);
                        this.sync_arg_prompt_after_text_change(
                            prev_original_idx,
                            window,
                            cx,
                        );
                        cx.notify();
                    } else if let AppView::AppLauncherView {
                        filter,
                        selected_index,
                    } = &mut this.current_view
                    {
                        if *filter != current_value {
                            *filter = current_value;
                            *selected_index = 0;
                            this.list_scroll_handle
                                .scroll_to_item(0, gpui::ScrollStrategy::Nearest);
                            cx.notify();
                        }
                    } else if let AppView::BrowseKitsView { query, .. } = &this.current_view {
                        if *query != current_value {
                            this.kit_store_set_browse_query(current_value, cx);
                        }
                    } else if let AppView::InstalledKitsView {
                        filter,
                        selected_index,
                        ..
                    } = &mut this.current_view
                    {
                        if *filter != current_value {
                            *filter = current_value;
                            *selected_index = 0;
                            this.list_scroll_handle
                                .scroll_to_item(0, gpui::ScrollStrategy::Nearest);
                            cx.notify();
                        }
                    } else {
                        let input_received_at = std::time::Instant::now();
                        if logging::filter_perf_trace_enabled() {
                            logging::log(
                                "FILTER_PERF",
                                &format!(
                                    "[1/5] INPUT_CHANGE value='{}' len={} at {:?}",
                                    current_value,
                                    current_value.len(),
                                    input_received_at
                                ),
                            );
                        }
                        this.main_menu_render_diagnostics.filter_perf_start =
                            Some(input_received_at);
                        this.handle_filter_input_change(window, cx);
                    }
                }
                InputEvent::PressEnter { .. } => {
                    // Block Enter when confirm picker is active — the confirm
                    // popup's key routing already handled this Enter via
                    // capture_key_down in render_impl.
                    if confirm::is_confirm_window_open() {
                        logging::log("KEY", "Ignoring PressEnter: confirm picker is active");
                        return;
                    }

                    if matches!(this.current_view, AppView::ThemeChooserView { .. }) {
                        if !this.show_actions_popup && !actions::is_actions_window_open() {
                            this.submit_theme_chooser_from_input_enter(window, cx);
                        } else {
                            logging::log(
                                "KEY",
                                "Ignoring ThemeChooser PressEnter: actions picker is active",
                            );
                        }
                        return;
                    }

                    // Handle Enter for mini/arg prompts — submit the arg value
                    let prompt_id = match &this.current_view {
                        AppView::MiniPrompt { id, .. } | AppView::ArgPrompt { id, .. } => {
                            Some(id.clone())
                        }
                        _ => None,
                    };
                    if let Some(prompt_id) = prompt_id {
                        this.submit_arg_prompt_from_current_state(&prompt_id, cx);
                        return;
                    }

                    logging::log(
                        "KEY",
                        &format!(
                            "PressEnter event: visible={}, view={:?}, actions_popup={}, fallback_mode={}, selected_index={}, filter='{}', computed_filter='{}'",
                            script_kit_gpui::is_main_window_visible(),
                            std::mem::discriminant(&this.current_view),
                            this.show_actions_popup,
                            this.main_menu_fallback_state.is_active(),
                            this.selected_index,
                            this.filter_text,
                            this.computed_filter_text
                        ),
                    );

                    if !script_kit_gpui::is_main_window_visible() {
                        logging::log("KEY", "Ignoring PressEnter: main window not visible");
                        return;
                    }

                    if matches!(this.current_view, AppView::ScriptList) && !this.show_actions_popup
                    {
                        if this.should_consume_menu_syntax_trigger_picker_press_enter(
                            "input_press_enter_script_list",
                        ) {
                            logging::log(
                                "KEY",
                                "Ignoring PressEnter: menu-syntax trigger picker consumed same physical Enter",
                            );
                            return;
                        }
                        if this.should_consume_script_list_enter_after_submit(
                            "input_press_enter_script_list",
                        ) {
                            logging::log(
                                "KEY",
                                "Ignoring PressEnter: prompt submit already consumed this Enter",
                            );
                            return;
                        }
                        if this.try_handle_spine_enter(window, cx) {
                            logging::log("KEY", "PressEnter: spine consumed");
                            return;
                        }
                        // Check if we're in fallback mode first
                        if this.main_menu_fallback_state.is_active() {
                            this.execute_selected_fallback(cx);
                        } else {
                            this.execute_selected(cx);
                        }
                    }
                }
                InputEvent::PressTab { secondary } => {
                    if matches!(this.current_view, AppView::ScriptList)
                        && this.menu_syntax_capture_form_owns_input()
                    {
                        if *secondary {
                            this.focus_previous_menu_syntax_form_field(window, cx);
                        } else {
                            this.focus_next_menu_syntax_form_field(window, cx);
                        }
                    }
                }
                InputEvent::SelectionChange => {}
            }
        });

        // Create channel for API key configuration completion signals
        // Small buffer (4) prevents blocking, more than enough for normal use
        let (api_key_tx, api_key_rx) = mpsc::sync_channel(4);

        // Legacy chat channels (retained for inline chat compatibility — not the primary Tab AI surface)
        let (inline_chat_escape_tx, inline_chat_escape_rx) = mpsc::sync_channel(4);
        let (flow_chat_tx, flow_chat_rx) = mpsc::sync_channel(32);
        let (inline_chat_actions_tx, inline_chat_actions_rx) = mpsc::sync_channel(4);
        let (inline_chat_continue_tx, inline_chat_continue_rx) = mpsc::sync_channel(4);
        let (inline_chat_configure_tx, inline_chat_configure_rx) = mpsc::sync_channel(4);
        let (inline_chat_claude_code_tx, inline_chat_claude_code_rx) = mpsc::sync_channel(4);
        // Create channel for naming dialog completion signals
        let (naming_submit_tx, naming_submit_rx) = mpsc::sync_channel(4);
        let default_response_sender = create_stdout_response_sender();
        // Discover plugin skills for main-menu search
        let plugin_skills: Vec<std::sync::Arc<crate::plugins::PluginSkill>> = {
            let skills = crate::plugins::discover_plugins()
                .and_then(|index| crate::plugins::discover_plugin_skills(&index))
                .unwrap_or_default();
            skills.into_iter().map(std::sync::Arc::new).collect()
        };
        crate::dictation::hydrate_dictation_resource_from_history();
        let window_search_test_provider =
            std::env::var_os("SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER").is_some();
        let initial_cached_windows = if window_search_test_provider {
            crate::window_control::list_windows().unwrap_or_default()
        } else {
            Vec::new()
        };
        let initial_root_windows_provider_status = if window_search_test_provider {
            crate::window_control::RootWindowsProviderStatus::Ready {
                count: initial_cached_windows.len(),
            }
        } else {
            crate::window_control::RootWindowsProviderStatus::Unknown
        };
        let root_search = RootSearchStore::with_root_windows(
            &initial_cached_windows,
            &apps,
            initial_root_windows_provider_status,
        );

        // Restore the persisted global working directory (footer cwd chip) so it
        // survives app restarts; the user tends to stay in one directory. A
        // restored cwd counts as an explicit pick (revision 1) so the agent
        // launches there. Falls back to ~/.scriptkit (revision 0) when no valid
        // persisted directory exists.
        let (initial_spine_cwd, initial_spine_cwd_label, initial_spine_cwd_revision) = {
            let persisted = crate::config::load_user_preferences()
                .ai
                .cwd
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_dir());
            match persisted {
                Some(path) => {
                    let label = crate::file_search::shorten_path(&path.to_string_lossy())
                        .trim_end_matches('/')
                        .to_string();
                    (Some(path), Some(label), 1_u64)
                }
                None => (
                    dirs::home_dir().map(|h| h.join(".scriptkit")),
                    Some("~/.scriptkit".to_string()),
                    0_u64,
                ),
            }
        };

        // Prewarm the flow roster for the RESTORED effective cwd so flows
        // are already in the main-menu corpus by the first open. Must run
        // after the spine_cwd restore above — resolve_flow_cwd(None) would
        // warm the wrong cache key when a persisted cwd exists.
        {
            let restored_cwd = initial_spine_cwd
                .as_ref()
                .map(|path| path.to_string_lossy().to_string());
            crate::flows::catalog::flow_catalog()
                .roster_for(&crate::flows::resolve_flow_cwd(restored_cwd));
        }

        // Push-driven roster arrival: rosters land on a background fetch
        // thread; without this hook an idle open window never repaints when
        // flows appear. The generation poll in filtering_cache stays as the
        // fallback for the same signal.
        #[cfg(not(test))]
        {
            let (roster_tx, roster_rx) = async_channel::bounded::<()>(4);
            crate::flows::catalog::flow_catalog().set_notify_hook(move || {
                let _ = roster_tx.try_send(());
            });
            cx.spawn(async move |this, cx| {
                while roster_rx.recv().await.is_ok() {
                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            app.invalidate_filter_cache();
                            app.invalidate_grouped_cache();
                            cx.notify();
                        })
                    });
                }
            })
            .detach();
        }

        // Resolve the persisted profile/model into header display labels so the
        // selection (Shift+Tab Profile Switcher) is visible on first paint.
        let (initial_spine_agent_label, initial_spine_model_label) =
            Self::resolve_agent_model_footer_labels();

        let mut app = ScriptListApp {
            scripts,
            scriptlets,
            skills: plugin_skills,
            script_validation_report,
            builtin_entries,
            apps,
            // P0 FIX: Cached data for builtin views (avoids cloning per frame)
            cached_clipboard_entries: Vec::new(),
            paste_sequential_state: None,
            focused_clipboard_entry_id: None,
            cached_windows: initial_cached_windows,
            cached_browser_tabs: Vec::new(),
            cached_browser_history: Vec::new(),
            browser_history_loading: false,
            cached_file_results: Vec::new(),
            root_search,
            spine_file_search_query: String::new(),
            spine_file_search_generation: 0,
            spine_file_search_loading: false,
            spine_file_search_results: Vec::new(),
            spine_file_search_cancel: None,
            spine_empty_subsearch_armed_for: None,
            pending_root_file_actions_file: None,
            pending_root_unified_actions_subject: None,
            cached_processes: Vec::new(),
            process_manager_refresh_task: None,
            cached_current_app_entries: Vec::new(),
            current_app_commands_session: None,
            selected_index: 0,
            main_menu_selection_user_moved: false,
            filter_text: String::new(),
            inline_calculator: None,
            gpui_input_state,
            gpui_input_focused: false,
            ghost_prediction: None,
            prediction_revision: Default::default(),
            ghost_llm_generation: 0,
            ghost_llm_cancel: None,
            ghost_llm_cache: std::collections::VecDeque::new(),
            launcher_context: Default::default(),
            launcher_context_generation: 0,
            gpui_input_subscriptions: vec![gpui_input_subscription],
            bounds_subscription: None,     // Set later after window setup
            appearance_subscription: None, // Set later after window setup
            suppress_filter_events: false,
            pending_programmatic_filter_echo: None,
            pending_filter_sync: false,
            history_filter_render_pending: None,
            return_to_script_list_key_guard: None,
            pending_placeholder: None,
            last_output: None,
            focus_handle: cx.focus_handle(),
            show_logs: false,
            show_info_panel: false,
            quick_terminal_warm_pty: None,
            quick_terminal_warm_inflight: false,
            quick_terminal_warm_created_at: None,
            theme,
            config,
            // Scroll activity tracking: start with scrollbar hidden
            scrollbar_visibility: crate::transitions::Opacity::INVISIBLE,
            scrollbar_fade_gen: 0,
            last_scroll_time: None,
            current_view: AppView::ScriptList,
            last_logged_app_view_variant: None,
            submit_diagnostics: SubmitDiagnosticsState::default(),
            main_window_mode: MainWindowMode::Mini,
            script_session: Arc::new(ParkingMutex::new(None)),
            arg_input: TextInputState::new(),
            arg_selected_index: 0,
            prompt_receiver: None,
            response_sender: Some(default_response_sender.clone()),
            default_response_sender: Some(default_response_sender),
            // Variable-height list state for main menu (section headers at 24px, items at 48px)
            // Start with 0 items; filter replacement installs a fresh state without measuring all rows.
            main_list_state: ListState::new(
                0,
                ListAlignment::Top,
                px(crate::list_item::effective_average_item_height_for_scroll()),
            ),
            main_list_row_generation: 0,
            menu_syntax_main_hint_scroll_handle: ScrollHandle::new(),
            builtin_row_stack_scroll_handle: ScrollHandle::new(),
            menu_syntax_form_field_bounds: Default::default(),
            list_scroll_handle: UniformListScrollHandle::new(),
            arg_list_scroll_handle: UniformListScrollHandle::new(),
            clipboard_list_scroll_handle: UniformListScrollHandle::new(),
            emoji_scroll_handle: UniformListScrollHandle::new(),
            emoji_frequent_snapshot: Vec::new(),
            window_list_scroll_handle: UniformListScrollHandle::new(),
            tips_list_scroll_handle: UniformListScrollHandle::new(),
            browser_tabs_scroll_handle: UniformListScrollHandle::new(),
            process_list_scroll_handle: UniformListScrollHandle::new(),
            flow_ux_scroll_handle: UniformListScrollHandle::new(),
            flow_ux_seen_generation: 0,
            flow_ux_tick_running: false,
            flow_sessions: Vec::new(),
            flow_session_counter: 0,
            flow_session_return_to_desk: false,
            flow_chat_sender: flow_chat_tx,
            flow_chat_receiver: flow_chat_rx,
            current_app_commands_scroll_handle: UniformListScrollHandle::new(),
            agent_chat_history_scroll_handle: ScrollHandle::new(),
            browser_history_scroll_handle: ScrollHandle::new(),
            dictation_history_scroll_handle: ScrollHandle::new(),
            notes_browse_scroll_handle: ScrollHandle::new(),
            tracked_builtin_list_states: std::collections::HashMap::new(),
            file_search_scroll_handle: UniformListScrollHandle::new(),
            theme_chooser_list_state: ListState::new(0, ListAlignment::Top, px(100.)).measure_all(),
            file_search_loading: false,
            file_search_debounce_task: None,
            file_search_current_dir: None,
            file_search_current_dir_show_hidden: false,
            file_search_frozen_filter: None,
            file_search_actions_path: None,
            file_search_sort_mode: crate::actions::FileSearchSortMode::default(),
            file_search_gen: 0,
            file_search_cancel: None,
            file_search_display_indices: Vec::new(),
            file_search_selection_mode: FileSearchSelectionMode::AutoFirst,
            file_search_preview_thumbnail: FileSearchThumbnailPreviewState::Idle,
            show_actions_popup: false,
            registered_main_list_displayed_shortcuts: std::collections::HashSet::new(),
            main_list_shortcut_sync_key: None,
            actions_closed_at: None,
            actions_dialog: None,
            cursor_visible: true,
            focused_input: FocusedInput::MainFilter,
            current_script_pid: None,
            main_menu_result_caches: MainMenuResultCacheState::default(),
            // P3: Two-stage filter coalescing
            computed_filter_text: String::new(),
            filter_coalescer: FilterCoalescer::new(),
            menu_syntax_mode: crate::menu_syntax::MenuSyntaxMode::default(),
            spine_enabled: true,
            spine_parse: crate::spine::SpineParse {
                segments: vec![],
                input: String::new(),
            },
            spine_projection: None,
            spine_cwd: initial_spine_cwd,
            spine_cwd_label: initial_spine_cwd_label,
            spine_cwd_revision: initial_spine_cwd_revision,
            ghost_context_cache: crate::scripts::search::ghost::GhostContextCache::default(),
            cwd_pick_mode: false,
            agent_model_picker_active: false,
            spine_agent_label: initial_spine_agent_label,
            spine_model_label: initial_spine_model_label,
            spine_live_preview_cache: Default::default(),
            shown_selection_hint_text: None,
            shown_selection_hint_token: 0,
            menu_syntax_trigger_picker_state:
                crate::menu_syntax_trigger_picker::MenuSyntaxTriggerPickerState::default(),
            menu_syntax_object_selector_state:
                crate::menu_syntax::MenuSyntaxObjectSelectorState::default(),
            menu_syntax_filter_accept_hint_label: None,
            menu_syntax_filter_accept_hint_filter: None,
            menu_syntax_filter_accept_hint_selected_index: None,
            menu_syntax_form_focused_index: 0,
            menu_syntax_form_signature: None,
            menu_syntax_form_inputs: Vec::new(),
            menu_syntax_form_input_subscriptions: Vec::new(),
            menu_syntax_form_syncing_from_input: false,
            menu_syntax_form_input_active: false,
            menu_syntax_form_draft_field_id: None,
            menu_syntax_form_draft_value: String::new(),
            menu_syntax_form_suggestion_field_id: None,
            menu_syntax_form_suggestion_selected_index: None,
            pending_menu_syntax_ai_proposal: None,
            menu_syntax_trigger_picker_suppressed_filter: None,
            // Scroll stabilization: start with no last scrolled index
            last_scrolled_index: None,
            // Preview cache: start empty, will populate on first render
            preview_cache_path: None,
            preview_cache_match_signature: None,
            preview_cache_lines: Vec::new(),
            // Scriptlet preview cache: avoid re-highlighting on every render
            scriptlet_preview_cache_key: None,
            scriptlet_preview_cache_lines: Vec::new(),
            // Design system: start with default design
            current_design: DesignVariant::default(),
            // Cohesive main-menu theme exploration: start on the first variation.
            current_main_menu_theme: crate::designs::MainMenuThemeVariant::default(),
            // Toast manager: initialize for error notifications
            toast_manager: ToastManager::new(),
            // Clipboard image cache: decoded RenderImages for thumbnails/preview
            clipboard_image_cache: std::collections::HashMap::new(),
            // Frecency store for tracking script usage
            frecency_store,
            // Mouse hover tracking - starts as None (no item hovered)
            hovered_index: None,
            // Input mode: starts as Mouse (default), switches to Keyboard on arrow keys
            input_mode: InputMode::Mouse,
            main_menu_fallback_state: MainMenuFallbackState::default(),
            theme_before_chooser: None,
            background_effect: crate::effects::initial_background_effect(),
            background_effect_intensity: crate::effects::initial_background_effect_intensity(),
            background_effect_started_at: None,
            _background_effect_ticker: None,
            main_list_loading_started_at: None,
            main_list_loading_ticker_epoch: 0,
            _main_list_loading_ticker: None,
            theme_chooser_management: None,
            theme_chooser_controls: None,
            theme_chooser_panel_mode: ThemeChooserPanelMode::default(),
            main_menu_render_diagnostics: MainMenuRenderDiagnosticsState::default(),
            main_list_scroll_frame_trace: MainListScrollFrameTrace::from_env(),
            last_list_interaction_source:
                crate::scrolling::list_interaction::ListViewportInputSource::Refresh,
            // Pending path action - starts as None (Arc<Mutex<>> for callback access)
            pending_path_action: Arc::new(Mutex::new(None)),
            // Signal to close path actions dialog
            close_path_actions: Arc::new(Mutex::new(false)),
            // Shared state: path actions dialog visibility (for toggle behavior)
            path_actions_showing: Arc::new(Mutex::new(false)),
            // Shared state: path actions search text (for header display)
            path_actions_search_text: Arc::new(Mutex::new(String::new())),
            // Pending path action result - action_id + path_info to execute
            pending_path_action_result: Arc::new(Mutex::new(None)),
            // Alias/shortcut registries - populated below
            alias_registry: std::collections::HashMap::new(),
            shortcut_registry: std::collections::HashMap::new(),
            announced_registry_conflicts: std::collections::HashSet::new(),
            // SDK actions - starts empty, populated by setActions() from scripts
            sdk_actions: None,
            action_shortcuts: std::collections::HashMap::new(),
            // Debug grid overlay - check env var at startup
            grid_config: if std::env::var("SCRIPT_KIT_DEBUG_GRID").is_ok() {
                logging::log(
                    "DEBUG_GRID",
                    "SCRIPT_KIT_DEBUG_GRID env var set - enabling grid overlay",
                );
                Some(debug_grid::GridConfig::default())
            } else {
                None
            },
            // Navigation coalescing for rapid arrow key events
            nav_coalescer: NavCoalescer::new(),
            main_list_boundary_affordance:
                crate::scrolling::boundary_affordance::BoundaryAffordanceState::new(
                    crate::platform::prefers_reduced_motion(),
                ),
            list_suppress_hover_until_pointer_move: false,
            menu_syntax_trigger_picker_suppress_next_launcher_click: false,
            menu_syntax_trigger_picker_enter_guard: None,
            // Window focus tracking - for detecting focus lost and auto-dismissing prompts
            was_window_focused: false,
            // Pin state - when true, window stays open on blur
            is_pinned: false,
            editor_escape_armed_at: None,
            // Pending focus: start with MainFilter since that's what we want focused initially
            // DEPRECATED: Use focus_coordinator instead. This remains for gradual migration.
            pending_focus: Some(FocusTarget::MainFilter),
            // Focus coordinator: unified focus management with push/pop overlay semantics
            focus_coordinator: focus_coordinator::FocusCoordinator::with_main_filter_focus(),
            // Scroll stabilization: track last scrolled index for each handle
            last_scrolled_main: None,
            last_scrolled_arg: None,
            last_scrolled_clipboard: None,
            last_scrolled_window: None,
            // Show warning banner when bun is not available
            show_bun_warning: !bun_available,
            // Menu bar integration: Now handled by frontmost_app_tracker module
            // which pre-fetches menu items in background when apps activate
            // Shortcut recorder state - starts as None (no recorder showing)
            shortcut_recorder_state: None,
            // Shortcut recorder entity - persisted to maintain focus
            shortcut_recorder_entity: None,
            // Alias input state - starts as None (no alias input showing)
            alias_input_state: None,
            // Alias input entity - persisted to maintain focus
            alias_input_entity: None,
            pending_tab_ai_execution: None,
            tab_ai_save_offer_state: None,
            tab_ai_harness: None,
            tab_ai_harness_capture_generation: 0,
            tab_ai_harness_return_view: None,
            tab_ai_harness_return_focus_target: None,
            tab_ai_harness_script_list_trigger: None,
            tab_ai_harness_apply_back_route: None,
            embedded_agent_chat: None,
            embedded_agent_chat_focus_handle: None,
            prewarmed_agent_chat: None,
            active_agent_chat_warm_lease: None,
            agent_chat_ready_script_path: None,
            agent_chat_footer_dot_status: None,
            agent_chat_footer_model_display: None,
            agent_chat_footer_snapshot: None,
            attachment_portal_host_snapshot: None,
            attachment_portal_return_view: None,
            attachment_portal_return_focus_target: None,
            attachment_portal_return_width: None,
            active_attachment_portal_kind: None,
            spine_mention_portal_segment: None,
            spine_mention_aliases: std::collections::HashMap::new(),
            day_page_context_return: None,
            agent_chat_surface_state:
                crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceState::Hidden,
            // Input history for shell-like up/down navigation
            input_history: {
                let mut history = input_history::InputHistory::new();
                if let Err(e) = history.load() {
                    tracing::warn!("Failed to load input history: {}", e);
                }
                history
            },
            // API key configuration state
            pending_api_key_config: None,
            // API key completion channel - for EnvPrompt callback to signal completion
            // The channel is created here and both ends are stored
            api_key_completion_sender: api_key_tx,
            api_key_completion_receiver: api_key_rx,
            // Navigation tracking: starts false, set to true when opening built-in views from main menu
            opened_from_main_menu: false,
            active_favorites: None,
            // Inline chat escape channel - for ChatPrompt escape callback to signal return to main menu
            inline_chat_escape_sender: inline_chat_escape_tx,
            inline_chat_escape_receiver: inline_chat_escape_rx,
            inline_chat_actions_sender: inline_chat_actions_tx,
            inline_chat_actions_receiver: inline_chat_actions_rx,
            mini_ai_last_close_snapshot: None,
            // Inline chat continue channel - for ChatPrompt continue callback to hide main window
            inline_chat_continue_sender: inline_chat_continue_tx,
            inline_chat_continue_receiver: inline_chat_continue_rx,
            // Inline chat configure channel - for ChatPrompt configure callback to trigger API key setup
            inline_chat_configure_sender: inline_chat_configure_tx,
            inline_chat_configure_receiver: inline_chat_configure_rx,
            // Inline chat Claude Code channel - for ChatPrompt Claude Code callback to enable Claude Code
            inline_chat_claude_code_sender: inline_chat_claude_code_tx,
            inline_chat_claude_code_receiver: inline_chat_claude_code_rx,
            // Naming dialog completion channel - for NamingPrompt callback to signal submit/cancel
            naming_submit_sender: naming_submit_tx,
            naming_submit_receiver: naming_submit_rx,
            // Light theme opacity adjustment offset (Cmd+Shift+[/])
            light_opacity_offset: 0.0,
            // Mouse cursor hidden state - hidden while typing, shown on mouse move
            mouse_cursor_hidden: false,
            // Cached provider registry - built in background, None until ready
            cached_provider_registry: None,
            cached_main_window_preflight: None,
            main_window_preflight_cache_key: String::from("\0_UNINITIALIZED_\0"),
            window_orchestrator: crate::window_orchestrator::OrchestratorState::default(),
        };

        // Build initial alias/shortcut registries (conflicts logged, not shown via HUD on startup)
        let conflicts = app.rebuild_registries();
        if !conflicts.is_empty() {
            logging::log(
                "STARTUP",
                &format!(
                    "Found {} alias/shortcut conflicts on startup",
                    conflicts.len()
                ),
            );
        }
        // Mark startup conflicts announced so the first watcher refresh does
        // not toast what startup deliberately kept silent; only conflicts
        // introduced by later file edits HUD.
        app.announced_registry_conflicts = conflicts.into_iter().collect();
        // Build provider registry in background to avoid blocking UI when opening AI chat
        {
            let config_clone = app.config.clone();
            let (tx, rx) = async_channel::bounded::<crate::ai::ProviderRegistry>(1);

            std::thread::spawn(move || {
                let registry =
                    crate::ai::ProviderRegistry::from_environment_with_config(Some(&config_clone));
                if tx.send_blocking(registry).is_err() {
                    logging::log(
                        "APP",
                        "Provider registry build result dropped: receiver unavailable",
                    );
                }
            });

            cx.spawn(async move |this, cx| {
                let Ok(registry) = rx.recv().await else {
                    logging::log(
                        "APP",
                        "Background provider registry build failed: channel closed",
                    );
                    return;
                };

                let provider_count = registry.provider_ids().len();
                let _ = cx.update(|cx| {
                    this.update(cx, |app, _cx| {
                        app.cached_provider_registry = Some(registry);
                        logging::log(
                            "APP",
                            &format!(
                                "Background provider registry ready: {} providers",
                                provider_count
                            ),
                        );
                    })
                });
            })
            .detach();
        }

        let app_entity_for_cmd_w = cx.entity().downgrade();
        let cmd_w_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_cmd_w;
            move |event, window, cx| {
                if !is_plain_platform_cmd_w(event) {
                    return;
                }

                let is_actions = crate::actions::is_actions_window(window);
                if is_secondary_surface_window(window) {
                    return;
                }

                // Actions popups hosted by secondary windows are not the main
                // launcher's to close.
                if is_actions && !crate::actions::is_actions_window_open_for_main() {
                    return;
                }

                if !script_kit_gpui::is_main_window_visible() && !is_actions {
                    return;
                }

                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, cx| {
                        this.close_main_window_from_top_level_cmd_w(window, cx);
                        cx.stop_propagation();
                    });
                }
            }
        });
        app.gpui_input_subscriptions.push(cmd_w_interceptor);

        // Add Tab key interceptor for "Ask AI" feature and file search directory navigation
        // This fires BEFORE normal key handling, allowing us to intercept Tab
        // even when the Input component has focus
        let app_entity_for_tab = cx.entity().downgrade();
        let tab_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_tab;
            move |event, window, cx| {
                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // Skip keystrokes from secondary windows — interceptors are
                // GLOBAL and fire for ALL windows.  Secondary windows own
                // their own Tab/Cmd+Enter handling.
                let is_notes = crate::notes::is_notes_window(window);
                let is_detached_agent_chat = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
                let is_actions = crate::actions::is_actions_window(window);
                let is_shortcut_recorder =
                    super::shortcut_recorder::is_shortcut_recorder_window(window);
                if is_notes
                    || is_detached_agent_chat
                    || is_actions
                    || is_shortcut_recorder
                {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "tab_interceptor_skipped_secondary_window",
                        is_notes,
                        is_detached_agent_chat,
                        is_actions,
                        is_shortcut_recorder,
                    );
                    return;
                }

                let key = event.keystroke.key.as_str();
                let is_tab_key = key.eq_ignore_ascii_case("tab");
                let is_backquote_key = key == "`"
                    || key.eq_ignore_ascii_case("backquote")
                    || key.eq_ignore_ascii_case("backtick");
                let has_shift = event.keystroke.modifiers.shift;
                let is_plain_enter = crate::ui_foundation::is_key_enter(key)
                    && !event.keystroke.modifiers.platform
                    && !has_shift
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control;

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                // cwd-pick mode (Tab → FileSearchView) owns Escape/Backspace so
                // the launcher's progressive-escape semantics apply: one Escape
                // returns to the main menu, Backspace from "~/" collapses to the
                // disk root, and Backspace from "/" returns to the main menu.
                // Must fire before the Input component eats Backspace.
                {
                    let mut consumed = false;
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            consumed = this.try_handle_cwd_pick_nav_key(event, window, cx);
                        });
                    }
                    if consumed {
                        cx.stop_propagation();
                        return;
                    }
                }

                // Flow session host chord: ⌘⇧D backgrounds the conversation
                // (process stays alive; desk shows it under Active).
                // Intercepted here so the agent TUI never sees the chord.
                if key.eq_ignore_ascii_case("d")
                    && event.keystroke.modifiers.platform
                    && event.keystroke.modifiers.shift
                {
                    let mut handled = false;
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if matches!(this.current_view, AppView::FlowSessionView { .. }) {
                                this.background_flow_session(window, cx);
                                handled = true;
                            }
                        });
                    }
                    if handled {
                        cx.stop_propagation();
                        return;
                    }
                }

                let global_key_intent = main_window_global_key_intent(event);
                if let Some(intent) = global_key_intent {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if this.handle_main_window_global_key_intent(intent, cx) {
                                cx.stop_propagation();
                            }
                        });
                    }
                    return;
                }

                if is_backquote_key
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.shift
                {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.accept_full_ghost_prediction(window, cx)
                            {
                                cx.stop_propagation();
                            }
                        });
                    }
                    return;
                }

                // Check for Tab key (no cmd/alt/ctrl modifiers, but shift is allowed)
                if is_tab_key
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control
                {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            let owner = match &this.current_view {
                                AppView::ScriptList => "script_list",
                                AppView::AgentChatView { .. } => "embedded_agent_chat",
                                AppView::QuickTerminalView { .. } => "quick_terminal",
                                AppView::FileSearchView { .. } => "file_search",
                                AppView::ChatPrompt { .. } => "chat_prompt",
                                _ => "main_window_other",
                            };
                            tracing::debug!(
                                target: "script_kit::keyboard",
                                event = "tab_interceptor_owner_path",
                                owner,
                                has_shift,
                                show_actions_popup = this.show_actions_popup,
                                save_offer_open = this.tab_ai_save_offer_state.is_some(),
                            );

                            if this.main_window_modal_owns_keyboard() {
                                tracing::debug!(
                                    target: "script_kit::keyboard",
                                    event = "tab_interceptor_suppressed_for_modal",
                                    owner,
                                    has_shift,
                                );
                                cx.stop_propagation();
                                return;
                            }

                            // File search owns Tab locally: plain Tab browses
                            // into the selected directory and Shift+Tab goes up.
                            if matches!(this.current_view, AppView::FileSearchView { .. }) {
                                cx.stop_propagation();
                                if this.show_actions_popup {
                                    return;
                                }

                                if has_shift {
                                    let current_query = match &this.current_view {
                                        AppView::FileSearchView { query, .. } => query.clone(),
                                        _ => String::new(),
                                    };

                                    let parent_path_opt = if let Some(parsed) =
                                        crate::file_search::parse_directory_path(&current_query)
                                    {
                                        if parsed.filter.is_some() {
                                            Some(parsed.directory)
                                        } else {
                                            crate::file_search::parent_dir_display(
                                                &parsed.directory,
                                            )
                                        }
                                    } else {
                                        None
                                    };

                                    if let Some(parent_path) = parent_path_opt {
                                        crate::logging::log(
                                            "KEY",
                                            &format!(
                                                "Shift+Tab: Navigating up from '{}' to '{}'",
                                                current_query, parent_path
                                            ),
                                        );
                                        this.gpui_input_state.update(cx, |state, cx| {
                                            state.set_value(parent_path.clone(), window, cx);
                                            let len = parent_path.len();
                                            state.set_selection(len, len, window, cx);
                                        });
                                        cx.notify();
                                    } else {
                                        crate::logging::log(
                                            "KEY",
                                            &format!(
                                                "Shift+Tab: Already at root '{}', no-op",
                                                current_query
                                            ),
                                        );
                                    }
                                } else if !this.navigate_file_search_into_selected_directory(cx) {
                                    crate::logging::log(
                                        "KEY",
                                        "Tab: no selected directory to navigate into",
                                    );
                                }
                                return;
                            }

                            // Profile Search owns plain Tab: assign the
                            // highlighted profile to Quick AI ("Use for Quick
                            // AI") while Enter keeps switching the Agent Chat
                            // default. Mirrored by the SimulateKey protocol in
                            // simulate_key_dispatch.rs — keep in lockstep.
                            if matches!(this.current_view, AppView::ProfileSearchView { .. }) {
                                cx.stop_propagation();
                                if this.show_actions_popup {
                                    return;
                                }
                                if !has_shift {
                                    let assigned =
                                        this.select_profile_search_result_for_quick_ai(cx);
                                    tracing::info!(
                                        target: "script_kit::spine",
                                        event = "profile_search_quick_ai_tab_assign",
                                        assigned,
                                        "Tab in Profile Search → Use for Quick AI"
                                    );
                                }
                                return;
                            }

                            // ChatPrompt keeps Shift+Tab for local setup
                            // navigation and leaves plain Tab local.
                            if matches!(this.current_view, AppView::ChatPrompt { .. })
                                && has_shift {
                                    if let AppView::ChatPrompt { entity, .. } = &this.current_view {
                                        let handled = entity.update(cx, |chat, cx| {
                                            chat.handle_setup_key("tab", true, cx)
                                        });
                                        if handled {
                                            cx.stop_propagation();
                                            return;
                                        }
                                    }
                                }

                            // Menu-syntax trigger picker owns Tab when it is
                            // visible — Tab applies the selected row (keep-open
                            // for open-value qualifiers like `source:`,
                            // close-after-apply for bare qualifiers or capture
                            // targets). Runs BEFORE the Agent Chat plain-Tab routing
                            // branch so menu-syntax keyboard stays consistent
                            // with the Agent Chat slash / @ composer pickers.
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_object_selector_owns_main_keyboard()
                            {
                                let intent = if has_shift {
                                    crate::menu_syntax::InlinePickerKeyIntent::MoveUp
                                } else {
                                    crate::menu_syntax::InlinePickerKeyIntent::Apply
                                };
                                if this.apply_menu_syntax_object_selector_intent(
                                    intent, window, cx,
                                ) {
                                    cx.stop_propagation();
                                    return;
                                }
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_trigger_picker_owns_main_keyboard()
                            {
                                let intent = if has_shift {
                                    crate::menu_syntax::InlinePickerKeyIntent::MoveUp
                                } else {
                                    crate::menu_syntax::InlinePickerKeyIntent::Apply
                                };
                                if this.apply_menu_syntax_trigger_picker_intent(
                                    intent, window, cx,
                                ) {
                                    cx.stop_propagation();
                                    return;
                                }
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && this.handle_menu_syntax_form_key_input(
                                    key,
                                    event.keystroke.key_char.as_deref(),
                                    &event.keystroke.modifiers,
                                    window,
                                    cx,
                                )
                            {
                                cx.stop_propagation();
                                return;
                            }

                            if this.menu_syntax_capture_form_owns_input() {
                                if has_shift {
                                    this.focus_previous_menu_syntax_form_field(window, cx);
                                } else {
                                    this.focus_next_menu_syntax_form_field(window, cx);
                                }
                                cx.stop_propagation();
                                return;
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && !has_shift
                                && this.accept_ghost_prediction(window, cx)
                            {
                                cx.stop_propagation();
                                return;
                            }

                            // Tab on ScriptList opens the cwd picker — the
                            // chip-as-button affordance. Fires only when
                            // nothing else above (menu-syntax pickers, ghost
                            // prediction, capture form, Agent Chat/terminal locals)
                            // claimed the keystroke. The picker is the same
                            // FileSearchView that `>` used to open; the
                            // user's first typed char inside it transitions
                            // into ordinary file navigation.
                            //
                            // A2 decision (2026-06-09): the picker only opens
                            // when the main input is EMPTY. With text typed,
                            // Tab must never teleport the user into file
                            // search mid-query.
                            if matches!(this.current_view, AppView::ScriptList)
                                && !has_shift
                                && this.spine_enabled
                                && !this.show_actions_popup
                                && this.filter_text.trim().is_empty()
                            {
                                tracing::info!(
                                    target: "script_kit::spine",
                                    event = "cwd_pick_enter_file_search_tab",
                                    "Tab → FileSearchView (cwd pick)"
                                );
                                this.cwd_pick_mode = true;
                                this.open_file_search_view(
                                    "~/".to_string(),
                                    FileSearchPresentation::Full,
                                    cx,
                                );
                                this.suppress_filter_events = true;
                                this.gpui_input_state.update(cx, |state, cx| {
                                    state.set_value("~/".to_string(), window, cx);
                                    let len = "~/".len();
                                    state.set_selection(len, len, window, cx);
                                });
                                this.suppress_filter_events = false;
                                cx.stop_propagation();
                                return;
                            }

                            if this.try_open_profile_search_from_script_list_shift_tab(
                                key,
                                &event.keystroke.modifiers,
                                "runtime",
                                cx,
                            ) {
                                cx.stop_propagation();
                                return;
                            }

                            // Quick AI: Tab with a non-empty query sends the
                            // typed text to the zero-context Quick AI profile
                            // (spark model, no tools/skills/memories). This is
                            // the header Tab chip's advertised action whenever
                            // the input has text, so it runs BEFORE root-file
                            // directory completion — only path-shaped queries
                            // (~/, /) keep Tab for browsing, matching the
                            // chip's predicate in `main_header_tab_chip_action`.
                            // (2026-07-10: restored after the Conversation Desk
                            // pivot briefly routed Tab-with-text to the flow
                            // router — flows stay reachable as main-menu rows.)
                            if matches!(this.current_view, AppView::ScriptList)
                                && !has_shift
                                && !this.show_actions_popup
                                && !this.filter_text.trim().is_empty()
                                && !crate::file_search::looks_like_root_directory_browse_query(
                                    &this.filter_text,
                                )
                            {
                                let query = this.filter_text.clone();
                                tracing::info!(
                                    target: "script_kit::tab_ai",
                                    event = "quick_ai_tab_entry",
                                    "Tab → Quick AI (zero-context spark)"
                                );
                                this.open_quick_ai_from_launcher(query, window, cx);
                                cx.stop_propagation();
                                return;
                            }

                            if matches!(this.current_view, AppView::ScriptList)
                                && this.try_navigate_root_file_directory_with_tab(
                                    has_shift, window, cx,
                                )
                            {
                                cx.stop_propagation();
                                return;
                            }

                            // Tab-to-Agent deprecated: Cmd+Enter is the AI entry.
                            // Ghost text acceptance (above) now owns plain Tab.

                            // Agent Chat owns Tab locally. Plain Tab stays
                            // swallowed so the global interceptor cannot re-open a
                            // fresh chat. Shift+Tab is the documented Profile
                            // Switcher shortcut, so route it to the in-chat
                            // Profile picker via the window-aware entry point. We
                            // Keep this on the in-chat profile picker; the
                            // main-window Profile Search is only for ScriptList.
                            if let AppView::AgentChatView { entity, .. } = &this.current_view {
                                if has_shift {
                                    cx.stop_propagation();
                                    if this.show_actions_popup
                                        || crate::actions::is_actions_window_open()
                                    {
                                        return;
                                    }
                                    tracing::info!(
                                        target: "script_kit::keyboard",
                                        event = "agent_chat_shift_tab_profile_switcher",
                                        "Opening Agent Chat Profile picker from Shift+Tab"
                                    );
                                    let entity = entity.clone();
                                    entity.update(cx, |chat, cx| {
                                        chat.open_profile_trigger_picker_in_window(window, cx);
                                    });
                                    return;
                                }
                                let handled = entity
                                    .update(cx, |chat, cx| chat.handle_tab_key(false, window, cx));
                                if handled {
                                    cx.stop_propagation();
                                    return;
                                }
                            }

                            // Forward Tab/Shift+Tab directly to the harness
                            // terminal PTY.  We must NOT call cx.propagate()
                            // here because GPUI's built-in focus-traversal
                            // would consume the Tab keystroke before it reaches
                            // the TermPrompt key handler.  Instead, write the
                            // raw byte to the PTY and stop propagation.
                            if let AppView::QuickTerminalView { entity, .. } = &this.current_view {
                                entity.update(cx, |term, _cx| {
                                    let running = term.terminal.is_running();
                                    let bytes: &[u8] = if has_shift {
                                        b"\x1b[Z" // Shift+Tab (backtab)
                                    } else {
                                        b"\t" // Tab
                                    };
                                    if !running {
                                        tracing::warn!(
                                            event = "quick_terminal_tab_pty_dead",
                                            has_shift,
                                            "Tab intercepted but PTY is not running"
                                        );
                                        return;
                                    }
                                    match term.terminal.input(bytes) {
                                        Ok(()) => tracing::debug!(
                                            event = "quick_terminal_tab_sent",
                                            has_shift,
                                            "Tab byte written to PTY"
                                        ),
                                        Err(e) => tracing::warn!(
                                            event = "quick_terminal_tab_write_failed",
                                            error = %e,
                                            has_shift,
                                            "Failed to write Tab to PTY"
                                        ),
                                    }
                                });
                                cx.stop_propagation();
                                return;
                            }


                            // Block Tab while the save-offer overlay is visible
                            if this.tab_ai_save_offer_state.is_some() {
                                cx.stop_propagation();
                            }
                        });
                    }
                }

                // Keep plain Enter routed to Agent Chat mention acceptance in the
                // embedded main-window host when the picker is open.
                if is_plain_enter {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if let AppView::ScriptIssuesView { report } = &this.current_view {
                                let report = report.clone();
                                this.fix_script_issues_in_agent(&report, cx);
                                cx.stop_propagation();
                                return;
                            }
                            // Menu-syntax trigger picker owns Enter when it is
                            // visible on ScriptList — Accept the selected row
                            // the same way the Agent Chat composer picker does.
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_object_selector_owns_main_keyboard()
                                && this.apply_menu_syntax_object_selector_intent(
                                    crate::menu_syntax::InlinePickerKeyIntent::Accept,
                                    window,
                                    cx,
                                ) {
                                    cx.stop_propagation();
                                    return;
                                }
                            if matches!(this.current_view, AppView::ScriptList)
                                && this.menu_syntax_trigger_picker_owns_main_keyboard()
                                && this.apply_menu_syntax_trigger_picker_intent(
                                    crate::menu_syntax::InlinePickerKeyIntent::Accept,
                                    window,
                                    cx,
                                ) {
                                    this.arm_menu_syntax_trigger_picker_enter_guard(
                                        "global_plain_enter_script_list",
                                    );
                                    cx.stop_propagation();
                                    return;
                                }
                            if matches!(this.current_view, AppView::ScriptList)
                                && !this.show_actions_popup
                                && !crate::actions::is_actions_window_open()
                            {
                                if this.should_consume_script_list_enter_after_submit(
                                    "global_plain_enter_script_list",
                                ) {
                                    logging::log(
                                        "KEY",
                                        "Ignoring global plain Enter: prompt submit already consumed this Enter",
                                    );
                                    cx.stop_propagation();
                                    return;
                                }
                                if this.try_handle_spine_enter(window, cx) {
                                    logging::log("KEY", "Global plain Enter: spine consumed");
                                    cx.stop_propagation();
                                    return;
                                }
                                if this.main_menu_fallback_state.is_active() {
                                    this.execute_selected_fallback(cx);
                                } else {
                                    this.execute_selected(cx);
                                }
                                cx.stop_propagation();
                                return;
                            }
                            if let AppView::AgentChatView { entity, .. } = &this.current_view {
                                let handled =
                                    entity.update(cx, |chat, cx| chat.handle_enter_key(cx));
                                if handled {
                                    cx.stop_propagation();
                                }
                            }
                        });
                    }
                }
            }
        });
        app.gpui_input_subscriptions.push(tab_interceptor);

        // Prewarm Agent Chat config and the hidden Agent Chat connection so the first
        // compatible Agent Chat submit can reuse an initialized runtime/session.
        crate::ai::agent_chat::ui::prewarm_agent_config();

        // Prewarm Agent Chat and the Tab AI harness asynchronously so AI-entry
        // shortcuts do not pay subprocess/session startup cost on submit.
        let app_entity_for_tab_ai_warm = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;
            cx.update(|cx| {
                let Some(app) = app_entity_for_tab_ai_warm.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.warm_agent_chat_on_startup(cx);
                    this.warm_tab_ai_harness_on_startup(cx);
                    this.warm_quick_terminal_pty(cx);
                });
            });
        })
        .detach();

        // Add arrow key interceptor for builtin views with Input components
        // This fires BEFORE Input component handles arrow keys, allowing list navigation
        let app_entity_for_arrows = cx.entity().downgrade();
        let arrow_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_arrows;
            move |event, window, cx| {
                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // intercept_keystrokes is GLOBAL and fires for ALL windows in the app.
                // Keep main list arrow routing scoped to the main window so notes/AI/actions
                // windows receive their own navigation key events.
                if crate::notes::is_notes_window(window)
                    || crate::ai::agent_chat::ui::chat_window::is_chat_window(window)
                    || crate::actions::is_actions_window(window)
                {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let is_up = crate::ui_foundation::is_key_up(key);
                let is_down = crate::ui_foundation::is_key_down(key);
                let is_left = crate::ui_foundation::is_key_left(key);
                let is_right = crate::ui_foundation::is_key_right(key);

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }
                let no_direction_modifiers = !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.alt
                    && !event.keystroke.modifiers.control;

                // Emoji picker uses Left/Right to navigate the grid and must consume
                // those keys before the search input moves its text cursor.
                if (is_left || is_right) && no_direction_modifiers {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            // Keep left/right from moving any focused input while actions picker is active.
                            if this.show_actions_popup {
                                cx.stop_propagation();
                                return;
                            }

                            let frequent_snapshot = this.emoji_frequent_snapshot.clone();
                            if let AppView::EmojiPickerView {
                                selected_index,
                                filter,
                                selected_category,
                            } = &mut this.current_view
                            {
                                let display = crate::emoji::display_ordered_emojis(
                                    filter,
                                    *selected_category,
                                    &frequent_snapshot,
                                );
                                let filtered_len = display.emojis.len();
                                if filtered_len == 0 {
                                    *selected_index = 0;
                                    this.hovered_index = None;
                                    cx.notify();
                                    cx.stop_propagation();
                                    return;
                                }

                                if *selected_index >= filtered_len {
                                    *selected_index = filtered_len - 1;
                                }

                                let old_idx = *selected_index;
                                *selected_index = if is_left {
                                    old_idx.saturating_sub(1)
                                } else {
                                    (old_idx + 1).min(filtered_len.saturating_sub(1))
                                };

                                let row = crate::emoji::compute_display_scroll_row(
                                    *selected_index,
                                    &display,
                                );
                                this.emoji_scroll_handle
                                    .scroll_to_item(row, ScrollStrategy::Nearest);

                                this.input_mode = InputMode::Keyboard;
                                this.hovered_index = None;
                                cx.notify();
                                cx.stop_propagation();
                            }
                        });
                    }
                }
                // Check for Up/Down arrow keys (no modifiers except shift for selection)
                if (is_up || is_down) && no_direction_modifiers {
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if this.route_day_page_note_switcher_key(
                                key,
                                key_char,
                                &event.keystroke.modifiers,
                                window,
                                cx,
                            ) {
                                cx.stop_propagation();
                                return;
                            }

                            // Universal: Route arrow keys to actions dialog when picker is active
                            // This ensures ALL views (ChatPrompt, ArgPrompt, etc.) route
                            // arrows to the dialog, not just the few views with explicit cases below.
                            if this.show_actions_popup {
                                if let Some(ref dialog) = this.actions_dialog {
                                    if is_up {
                                        dialog.update(cx, |d, cx| d.move_up(cx));
                                    } else if is_down {
                                        dialog.update(cx, |d, cx| d.move_down(cx));
                                    }
                                    crate::actions::notify_actions_window(cx);
                                }
                                cx.stop_propagation();
                                return;
                            }

                            let emoji_frequent_snapshot = this.emoji_frequent_snapshot.clone();
                            // Only intercept in views that use Input + list navigation
                            match &mut this.current_view {
                                AppView::FileSearchView {
                                    selected_index,
                                    query,
                                    ..
                                } => {
                                    // CRITICAL: If actions picker is active, route to actions dialog instead
                                    if this.show_actions_popup {
                                        if let Some(ref dialog) = this.actions_dialog {
                                            if is_up {
                                                dialog.update(cx, |d, cx| d.move_up(cx));
                                            } else if is_down {
                                                dialog.update(cx, |d, cx| d.move_down(cx));
                                            }
                                            // Notify the actions window to re-render
                                            crate::actions::notify_actions_window(cx);
                                        }
                                        cx.stop_propagation();
                                        return;
                                    }

                                    // Compute filtered length using same logic as render
                                    let filter_pattern = if let Some(parsed) =
                                        crate::file_search::parse_directory_path(query)
                                    {
                                        parsed.filter
                                    } else if !query.is_empty() {
                                        Some(query.clone())
                                    } else {
                                        None
                                    };

                                    // Use Nucleo fuzzy matching for consistent filtering with render
                                    let filtered_len = if let Some(ref pattern) = filter_pattern {
                                        crate::file_search::filter_results_nucleo_simple(
                                            &this.cached_file_results,
                                            pattern,
                                        )
                                        .len()
                                    } else {
                                        this.cached_file_results.len()
                                    };

                                    let mut moved_selection = false;
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        moved_selection = true;
                                        this.file_search_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        moved_selection = true;
                                        this.file_search_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    if moved_selection {
                                        this.lock_file_search_selection_to_user_choice();
                                    }
                                    // Stop propagation so Input doesn't handle it
                                    cx.stop_propagation();
                                }
                                AppView::ClipboardHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    // CRITICAL: If actions picker is active, route to actions dialog instead
                                    if this.show_actions_popup {
                                        if let Some(ref dialog) = this.actions_dialog {
                                            if is_up {
                                                dialog.update(cx, |d, cx| d.move_up(cx));
                                            } else if is_down {
                                                dialog.update(cx, |d, cx| d.move_down(cx));
                                            }
                                            // Notify the actions window to re-render
                                            crate::actions::notify_actions_window(cx);
                                        }
                                        cx.stop_propagation();
                                        return;
                                    }

                                    let filtered_entries: Vec<_> = if filter.is_empty() {
                                        this.cached_clipboard_entries.iter().enumerate().collect()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        this.cached_clipboard_entries
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, e)| {
                                                e.text_preview
                                                    .to_lowercase()
                                                    .contains(&filter_lower)
                                                    || e.ocr_text
                                                        .as_deref()
                                                        .unwrap_or("")
                                                        .to_lowercase()
                                                        .contains(&filter_lower)
                                            })
                                            .collect()
                                    };
                                    let filtered_len = filtered_entries.len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.clipboard_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.clipboard_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                    }
                                    this.focused_clipboard_entry_id = filtered_entries
                                        .get(*selected_index)
                                        .map(|(_, entry)| entry.id.clone());
                                    cx.notify();
                                    cx.stop_propagation();
                                }
                                AppView::ProfileSearchView { .. } => {
                                    this.move_profile_search_selection(is_up, cx);
                                    cx.stop_propagation();
                                }
                                AppView::AppLauncherView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len =
                                        Self::app_launcher_filtered_entries(&this.apps, filter)
                                            .len();
                                    let old_index = *selected_index;

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                    }

                                    if *selected_index != old_index {
                                        tracing::debug!(
                                            target: "script_kit::scroll",
                                            event = "builtin_selection_nav",
                                            view = "app_launcher",
                                            old_index,
                                            new_index = *selected_index,
                                            total_items = filtered_len,
                                            strategy = "nearest",
                                        );

                                        this.list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        this.input_mode = InputMode::Keyboard;
                                        this.hovered_index = None;
                                        cx.notify();
                                    }

                                    cx.stop_propagation();
                                }
                                AppView::WindowSwitcherView {
                                    selected_index,
                                    filter: _,
                                } => {
                                    let filtered_len = this.cached_windows.len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.window_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.window_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::ProcessManagerView {
                                    selected_index,
                                    filter: _,
                                } => {
                                    let filtered_len = this.cached_processes.len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.process_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.process_list_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::CurrentAppCommandsView {
                                    selected_index,
                                    filter: _,
                                } => {
                                    let filtered_len = this.cached_current_app_entries.len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.current_app_commands_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.current_app_commands_scroll_handle.scroll_to_item(
                                            *selected_index,
                                            gpui::ScrollStrategy::Nearest,
                                        );
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::AgentChatHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len = if filter.is_empty() {
                                        crate::ai::agent_chat::ui::history::load_history().len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        crate::ai::agent_chat::ui::history::load_history()
                                            .into_iter()
                                            .filter(|entry| {
                                                entry
                                                    .first_message
                                                    .to_lowercase()
                                                    .contains(&filter_lower)
                                                    || entry
                                                        .timestamp
                                                        .to_lowercase()
                                                        .contains(&filter_lower)
                                            })
                                            .count()
                                    };
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.agent_chat_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.agent_chat_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::BrowserHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len =
                                        crate::browser_history::fuzzy_search_browser_history(
                                            &this.cached_browser_history,
                                            filter,
                                        )
                                        .len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.browser_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.browser_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::DictationHistoryView {
                                    selected_index,
                                    filter,
                                } => {
                                    let filtered_len =
                                        crate::dictation::search_history(filter, 100).len();
                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.dictation_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.dictation_history_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::SearchAiPresetsView {
                                    selected_index,
                                    filter,
                                } => {
                                    // Replicate render-time filtering from ai_presets.rs
                                    let default_presets: Vec<(&str, &str, &str)> = vec![
                                        (
                                            "general",
                                            "General Assistant",
                                            "Helpful AI assistant for any task",
                                        ),
                                        (
                                            "coder",
                                            "Code Assistant",
                                            "Expert programmer and debugger",
                                        ),
                                        (
                                            "writer",
                                            "Writing Assistant",
                                            "Help with writing and editing",
                                        ),
                                        (
                                            "researcher",
                                            "Research Assistant",
                                            "Deep analysis and research",
                                        ),
                                        (
                                            "creative",
                                            "Creative Partner",
                                            "Brainstorming and creative ideas",
                                        ),
                                    ];
                                    let all_presets =
                                        crate::ai::presets::load_presets().unwrap_or_default();
                                    let mut items: Vec<(String, String, String)> = Vec::new();
                                    for (id, name, desc) in &default_presets {
                                        items.push((
                                            id.to_string(),
                                            name.to_string(),
                                            desc.to_string(),
                                        ));
                                    }
                                    for preset in &all_presets {
                                        if !default_presets
                                            .iter()
                                            .any(|(did, _, _)| *did == preset.id)
                                        {
                                            items.push((
                                                preset.id.clone(),
                                                preset.name.clone(),
                                                preset.description.clone(),
                                            ));
                                        }
                                    }
                                    let filtered_len = if filter.is_empty() {
                                        items.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        items
                                            .iter()
                                            .filter(|(id, name, desc)| {
                                                name.to_lowercase().contains(&filter_lower)
                                                    || desc
                                                        .to_lowercase()
                                                        .contains(&filter_lower)
                                                    || id.to_lowercase().contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::FavoritesBrowseView {
                                    selected_index,
                                    filter,
                                } => {
                                    // Replicate render-time filtering from favorites.rs
                                    let favorites = script_kit_gpui::favorites::load_favorites()
                                        .unwrap_or_default();
                                    let resolved: Vec<(String, String)> = favorites
                                        .script_ids
                                        .iter()
                                        .map(|id| {
                                            let display_name = this
                                                .scripts
                                                .iter()
                                                .find(|s| s.name == *id)
                                                .map(|s| s.name.clone())
                                                .or_else(|| {
                                                    this.scriptlets
                                                        .iter()
                                                        .find(|sl| sl.name == *id)
                                                        .map(|sl| sl.name.clone())
                                                })
                                                .unwrap_or_else(|| id.clone());
                                            let description = this
                                                .scripts
                                                .iter()
                                                .find(|s| s.name == *id)
                                                .and_then(|s| s.description.clone())
                                                .or_else(|| {
                                                    this.scriptlets
                                                        .iter()
                                                        .find(|sl| sl.name == *id)
                                                        .and_then(|sl| sl.description.clone())
                                                })
                                                .unwrap_or_default();
                                            (display_name, description)
                                        })
                                        .collect();
                                    let filtered_len = if filter.is_empty() {
                                        resolved.len()
                                    } else {
                                        let filter_lower = filter.to_lowercase();
                                        resolved
                                            .iter()
                                            .filter(|(name, desc)| {
                                                name.to_lowercase().contains(&filter_lower)
                                                    || desc
                                                        .to_lowercase()
                                                        .contains(&filter_lower)
                                            })
                                            .count()
                                    };

                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    if is_up && *selected_index > 0 {
                                        *selected_index -= 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    } else if is_down && *selected_index + 1 < filtered_len {
                                        *selected_index += 1;
                                        this.builtin_row_stack_scroll_handle
                                            .scroll_to_item(*selected_index);
                                        cx.notify();
                                    }
                                    cx.stop_propagation();
                                }
                                AppView::EmojiPickerView {
                                    filter,
                                    selected_index,
                                    selected_category,
                                } => {
                                    let display = crate::emoji::display_ordered_emojis(
                                        filter,
                                        *selected_category,
                                        &emoji_frequent_snapshot,
                                    );
                                    let filtered_len = display.emojis.len();
                                    if filtered_len == 0 {
                                        *selected_index = 0;
                                        this.hovered_index = None;
                                        cx.notify();
                                        cx.stop_propagation();
                                        return;
                                    }

                                    if *selected_index >= filtered_len {
                                        *selected_index = filtered_len - 1;
                                    }

                                    let layout = crate::emoji::build_display_grid_layout(
                                        &display,
                                        crate::emoji::GRID_COLS,
                                    );
                                    let direction = if is_up {
                                        crate::emoji::EmojiNavDirection::Up
                                    } else {
                                        crate::emoji::EmojiNavDirection::Down
                                    };
                                    *selected_index = layout.move_index(*selected_index, direction);
                                    let row = layout.scroll_row_for_index(*selected_index);
                                    this.emoji_scroll_handle
                                        .scroll_to_item(row, ScrollStrategy::Nearest);

                                    this.input_mode = InputMode::Keyboard;
                                    this.hovered_index = None;
                                    cx.notify();
                                    cx.stop_propagation();
                                }
                                AppView::ScriptList => {
                                    // CRITICAL: If actions picker is active, route to actions dialog instead
                                    if this.show_actions_popup {
                                        if let Some(ref dialog) = this.actions_dialog {
                                            if is_up {
                                                dialog.update(cx, |d, cx| d.move_up(cx));
                                            } else if is_down {
                                                dialog.update(cx, |d, cx| d.move_down(cx));
                                            }
                                            // Notify the actions window to re-render
                                            crate::actions::notify_actions_window(cx);
                                        }
                                        cx.stop_propagation();
                                        return;
                                    }

                                    // Main menu: handle list navigation + input history
                                    const HISTORY: &str = "HISTORY";
                                    if is_up {
                                        if let Some(pending_filter) =
                                            this.history_filter_render_pending.as_ref()
                                        {
                                            tracing::info!(
                                                target: "script_kit::input_history",
                                                event = "history_key_repeat_coalesced_until_render",
                                                key = %key,
                                                pending_filter_len = pending_filter.len(),
                                                history_index = ?this.input_history.current_index(),
                                                selected_index = this.selected_index,
                                            );
                                            cx.stop_propagation();
                                            return;
                                        }
                                        let (grouped_items, _) = this.get_grouped_results_cached();
                                        let first_item_position =
                                            grouped_items.iter().position(|item| {
                                                matches!(
                                                    item,
                                                    crate::list_item::GroupedListItem::Item(_)
                                                )
                                            });
                                        let at_top_of_list = first_item_position
                                            .map(|position| this.selected_index <= position)
                                            .unwrap_or(true);
                                        let in_history =
                                            this.input_history.current_index().is_some();
                                        let source_filter_mode =
                                            this.source_filter_mode_blocks_input_history_recall();
                                        let filter_has_text = !this.filter_text.is_empty()
                                            || !this.computed_filter_text.is_empty();

                                        tracing::info!(
                                            target: "script_kit::input_history",
                                            event = "main_menu_arrow_history_decision",
                                            key = %key,
                                            action_resolved = event.action.is_some(),
                                            context_depth = event.context_stack.len(),
                                            selected_index = this.selected_index,
                                            first_item_position = ?first_item_position,
                                            at_top_of_list,
                                            in_history,
                                            source_filter_mode,
                                            filter_has_text,
                                            history_index = ?this.input_history.current_index(),
                                            grouped_item_count = grouped_items.len(),
                                            route = if source_filter_mode {
                                                "source_filter_list_up"
                                            } else if filter_has_text && !in_history {
                                                "filter_text_up_noop"
                                            } else if in_history || at_top_of_list {
                                                "history_up"
                                            } else {
                                                "list_up"
                                            },
                                        );
                                        // A8 decision (2026-06-09): history
                                        // recall only ENTERS from an empty
                                        // input at the top of the list, but
                                        // once in history, Up keeps walking
                                        // older entries even though the
                                        // recalled text fills the input.
                                        if !source_filter_mode
                                            && filter_has_text
                                            && at_top_of_list
                                            && !in_history
                                        {
                                            cx.stop_propagation();
                                            return;
                                        }

                                        if !source_filter_mode && (in_history || at_top_of_list) {
                                            if let Some(text) = this.input_history.navigate_up() {
                                                let safe = logging::log_user_value(&text);
                                                tracing::info!(
                                                    target: "script_kit::input_history",
                                                    event = "history_recalled",
                                                    direction = "up",
                                                    filter_preview = %safe,
                                                    filter_bytes = safe.raw_bytes,
                                                    filter_safe_bytes = safe.safe_bytes,
                                                    filter_truncated = safe.truncated,
                                                    history_index = ?this.input_history.current_index(),
                                                );
                                                logging::log(
                                                    HISTORY,
                                                    &format!("Recalled len={}", text.len()),
                                                );
                                                if text != this.filter_text
                                                    || text != this.computed_filter_text
                                                {
                                                    this.history_filter_render_pending =
                                                        Some(text.clone());
                                                    this.set_filter_text_immediate(
                                                        text, window, cx,
                                                    );
                                                } else {
                                                    tracing::info!(
                                                        target: "script_kit::input_history",
                                                        event = "history_recall_noop_already_rendered",
                                                        direction = "up",
                                                        filter_len = text.len(),
                                                        history_index = ?this.input_history.current_index(),
                                                    );
                                                }
                                            }
                                            cx.stop_propagation();
                                            return;
                                        }

                                        this.move_selection_up(cx);
                                    } else if is_down {
                                        let in_history =
                                            this.input_history.current_index().is_some();
                                        let source_filter_mode =
                                            this.source_filter_mode_blocks_input_history_recall();
                                        if !source_filter_mode && in_history {
                                            let history_index_before =
                                                this.input_history.current_index();
                                            let filter_len_before = this.filter_text.len();
                                            this.input_history.reset_navigation();
                                            tracing::info!(
                                                target: "script_kit::input_history",
                                                event = "history_exit_to_list_down",
                                                history_index_before = ?history_index_before,
                                                history_index_after = ?this.input_history.current_index(),
                                                filter_len_before,
                                                filter_len_after = this.filter_text.len(),
                                                selected_index_before = this.selected_index,
                                            );
                                            this.move_selection_down(cx);
                                            cx.stop_propagation();
                                            return;
                                        }
                                        if let Some(pending_filter) =
                                            this.history_filter_render_pending.as_ref()
                                        {
                                            tracing::info!(
                                                target: "script_kit::input_history",
                                                event = "history_key_repeat_coalesced_until_render",
                                                key = %key,
                                                pending_filter_len = pending_filter.len(),
                                                history_index = ?this.input_history.current_index(),
                                                selected_index = this.selected_index,
                                            );
                                            cx.stop_propagation();
                                            return;
                                        }
                                        tracing::info!(
                                            target: "script_kit::input_history",
                                            event = "main_menu_arrow_history_decision",
                                            key = %key,
                                            action_resolved = event.action.is_some(),
                                            context_depth = event.context_stack.len(),
                                            selected_index = this.selected_index,
                                            in_history,
                                            source_filter_mode,
                                            history_index = ?this.input_history.current_index(),
                                            route = if source_filter_mode {
                                                "source_filter_list_down"
                                            } else if in_history {
                                                "history_down"
                                            } else {
                                                "list_down"
                                            },
                                        );
                                        this.move_selection_down(cx);
                                    }
                                    cx.stop_propagation();
                                }
                                _ => {
                                    // Don't intercept arrows for other views (let normal handling work)
                                }
                            }
                        });
                    }
                }
            }
        });
        app.gpui_input_subscriptions.push(arrow_interceptor);

        // Add Home/End/PageUp/PageDown key interceptor for jump navigation
        let app_entity_for_home_end = cx.entity().downgrade();
        let home_end_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_home_end;
            move |event, window, cx| {
                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    return;
                }

                // Skip processing if this keystroke is from a secondary window
                if crate::notes::is_notes_window(window)
                    || crate::ai::agent_chat::ui::chat_window::is_chat_window(window)
                    || crate::actions::is_actions_window(window)
                {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let has_platform_mod = event.keystroke.modifiers.platform; // Cmd on macOS

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                // Home key or Cmd+Up → jump to first item
                // End key or Cmd+Down → jump to last item
                let is_home = key.eq_ignore_ascii_case("home")
                    || (has_platform_mod && crate::ui_foundation::is_key_up(key));
                let is_end = key.eq_ignore_ascii_case("end")
                    || (has_platform_mod && crate::ui_foundation::is_key_down(key));
                // Page Up/Down → move by ~10 selectable items
                let is_page_up = key.eq_ignore_ascii_case("pageup");
                let is_page_down = key.eq_ignore_ascii_case("pagedown");

                if !is_home && !is_end && !is_page_up && !is_page_down {
                    return;
                }

                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, cx| {
                        // Only handle in ScriptList view
                        if !matches!(this.current_view, AppView::ScriptList) {
                            return;
                        }

                        // Don't handle if actions picker is active
                        if this.show_actions_popup {
                            return;
                        }

                        if is_home {
                            this.move_selection_to_first(cx);
                        } else if is_end {
                            this.move_selection_to_last(cx);
                        } else if is_page_up {
                            this.move_selection_page_up(cx);
                        } else if is_page_down {
                            this.move_selection_page_down(cx);
                        }

                        cx.stop_propagation();
                    });
                }
            }
        });
        app.gpui_input_subscriptions.push(home_end_interceptor);

        // Add interceptor for actions popup in FileSearchView and ScriptList
        // This handles Cmd+K (toggle), Escape (close), Enter (submit), and typing
        let app_entity_for_actions = cx.entity().downgrade();
        let actions_interceptor = cx.intercept_keystrokes({
            let app_entity = app_entity_for_actions;
            move |event, window, cx| {
                let is_actions = crate::actions::is_actions_window(window);
                let secondary_window = SecondarySurfaceWindowState::inspect(window);
                let is_secondary_surface_window = secondary_window.is_secondary();
                let actions_open_for_main = crate::actions::is_actions_window_open_for_main();

                // A detached actions popup hosted by a secondary window (Notes,
                // detached Agent Chat) owns its keys via ActionsWindow::on_key_down.
                // Routing them through the main app's dialog router would land on
                // the wrong (or absent) dialog entity and swallow every keystroke.
                if is_actions && !actions_open_for_main {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_lower = key.to_ascii_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;
                let has_shift = event.keystroke.modifiers.shift;
                let has_alt = event.keystroke.modifiers.alt;
                let has_ctrl = event.keystroke.modifiers.control;
                let key_char = event.keystroke.key_char.as_deref();
                let is_actions_close_key = crate::ui_foundation::is_key_escape(key)
                    || (has_cmd && key.eq_ignore_ascii_case("k") && !has_shift);

                // The global interceptor sees every app window. Secondary parent
                // surfaces own their own Cmd+K/Escape routing; do not lease the
                // main ScriptListApp before their host callbacks can handle it.
                if !is_actions && is_secondary_surface_window {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "actions_interceptor_skipped_secondary_window",
                        is_notes = secondary_window.is_notes,
                        is_detached_agent_chat = secondary_window.is_detached_agent_chat,
                        is_actions,
                        is_shortcut_recorder = secondary_window.is_shortcut_recorder,
                        key = %key,
                        is_actions_close_key,
                    );
                    return;
                }

                if is_actions {
                    let routed = crate::actions::route_key_to_detached_actions_window(
                        key,
                        key_char,
                        &event.keystroke.modifiers,
                        cx,
                    );
                    if routed {
                        tracing::debug!(
                            target: "script_kit::actions",
                            event = "actions_interceptor_routed_from_actions_window_owner",
                            key = %key,
                        );
                        cx.stop_propagation();
                    }
                    return;
                }

                // Agent Chat can open the shared actions dialog from its own focused
                // composer even when the launcher visibility flag is false.
                // Close keys still need to reach the shared dialog before the
                // hidden-window guard below has a chance to skip them.
                if is_actions_close_key {
                    let mut close_key_routed = false;
                    if let Some(app) = app_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            if !is_actions
                                && !this.show_actions_popup
                                && !actions_open_for_main
                            {
                                return;
                            }
                            let Some(host) = this.current_actions_host() else {
                                return;
                            };
                            match this.route_key_to_actions_dialog(
                                key,
                                key_char,
                                &event.keystroke.modifiers,
                                host,
                                window,
                                cx,
                            ) {
                                ActionsRoute::NotHandled => {}
                                ActionsRoute::Handled | ActionsRoute::Execute { .. } => {
                                    tracing::info!(
                                        target: "script_kit::actions",
                                        event = if is_actions {
                                            "actions_interceptor_routed_from_actions_window"
                                        } else {
                                            "actions_interceptor_routed_close_before_visibility_guard"
                                        },
                                        host = ?host,
                                        key = %key,
                                    );
                                    cx.stop_propagation();
                                    close_key_routed = true;
                                }
                            }
                        });
                    }
                    if close_key_routed {
                        return;
                    }
                }

                // When the main window is hidden (e.g. Notes/AI open), main-menu
                // key interceptors must not consume keystrokes from secondary windows.
                if !script_kit_gpui::is_main_window_visible() {
                    tracing::debug!(
                        target: "script_kit::keyboard",
                        event = "actions_interceptor_main_window_hidden",
                        is_notes = secondary_window.is_notes,
                        is_detached_agent_chat = secondary_window.is_detached_agent_chat,
                        is_actions,
                    );
                    return;
                }

                if confirm::consume_main_window_key_while_confirm_open(
                    key,
                    &event.keystroke.modifiers,
                    cx,
                ) {
                    cx.stop_propagation();
                    return;
                }

                if let Some(app) = app_entity.upgrade() {
                    app.update(cx, |this, cx| {
                        if has_cmd {
                            logging::log(
                                "KEY",
                                &format!(
                                    "Actions interceptor saw key={} shortcut={} view={} popup={} focused_input={:?}",
                                    key,
                                    crate::shortcuts::keystroke_to_shortcut(
                                        key,
                                        &event.keystroke.modifiers,
                                    ),
                                    this.app_view_name(),
                                    this.show_actions_popup,
                                    this.focused_input
                                ),
                            );
                        }
                        // Route shared actions-dialog keys first; local actions
                        // key intents run only after the dialog declines the key.
                        if this.route_day_page_note_switcher_key(
                            key,
                            key_char,
                            &event.keystroke.modifiers,
                            window,
                            cx,
                        ) {
                            cx.stop_propagation();
                            return;
                        }

                        if this.main_window_modal_owns_keyboard() {
                            return;
                        }

                        let host = this.current_actions_host();

                        // Arrow keys are handled by arrow_interceptor to avoid double-processing
                        // (which can skip 2 items per keypress when both interceptors handle arrows).
                        if this.show_actions_popup
                            && (crate::ui_foundation::is_key_up(key)
                                || crate::ui_foundation::is_key_down(key))
                        {
                            return;
                        }

                        if let Some(host) = host {
                            match this.route_key_to_actions_dialog(
                                key,
                                key_char,
                                &event.keystroke.modifiers,
                                host,
                                window,
                                cx,
                            ) {
                                ActionsRoute::NotHandled => {}
                                ActionsRoute::Handled => {
                                    tracing::debug!(
                                        target: "script_kit::actions",
                                        event = "actions_interceptor_routed",
                                        host = ?host,
                                        key = %key,
                                    );
                                    cx.stop_propagation();
                                    return;
                                }
                                ActionsRoute::Execute {
                                    action_id,
                                    should_close,
                                } => {
                                    this.execute_actions_route_action(
                                        host,
                                        action_id,
                                        should_close,
                                        window,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                    return;
                                }
                            }
                        }

                        if this.try_execute_root_file_action_shortcut(
                            &key_lower, has_cmd, has_shift, has_alt, has_ctrl, window, cx,
                        ) {
                            cx.stop_propagation();
                            return;
                        }

                        // Route displayed action shortcuts even when the popup is closed.
                        if this.try_execute_main_list_action_shortcut_from_display(
                            key,
                            &event.keystroke.modifiers,
                            window,
                            cx,
                        ) {
                            cx.stop_propagation();
                            return;
                        }

                        // A5 decision (2026-06-09): Cmd+V on the ScriptList must be
                        // intercepted BEFORE the focused filter input's paste handler,
                        // which strips newlines. Multi-line/large pastes route to
                        // Agent Chat; small single-line pastes fall through to the
                        // input's normal paste.
                        if has_cmd
                            && !has_shift
                            && !has_alt
                            && !has_ctrl
                            && key_lower == "v"
                            && matches!(this.current_view, AppView::ScriptList)
                            && !this.show_actions_popup
                            && this.route_large_script_list_paste_to_agent_chat(cx)
                        {
                            logging::log(
                                "KEY",
                                "Interceptor Cmd+V -> route_large_script_list_paste_to_agent_chat",
                            );
                            cx.stop_propagation();
                            return;
                        }

                        if let Some(intent) =
                            main_window_actions_key_intent(&this.current_view, event)
                        {
                            if this.handle_main_window_actions_key_intent(intent, window, cx) {
                                if matches!(intent, MainWindowActionsKeyIntent::ToggleActions) {
                                    tracing::info!(
                                        target: "script_kit::actions",
                                        event = "actions_interceptor_toggled",
                                        host = ?host,
                                    );
                                }
                                cx.stop_propagation();
                                return;
                            }
                        }

                        let agent_chat_escape_popup_open = match &this.current_view {
                            AppView::AgentChatView { entity, .. } => {
                                entity.read(cx).has_escape_dismissible_popup()
                            }
                            _ => false,
                        };
                        let agent_chat_escape_focused_text_origin = match &this.current_view {
                            AppView::AgentChatView { entity, .. } => {
                                let chat = entity.read(cx);
                                chat.is_focused_text_mini()
                                    || chat.focused_text_originated_from_quick_prompt()
                            }
                            _ => false,
                        };

                        let agent_chat_escape_cancelled_streaming = if crate::ui_foundation::is_key_escape(key)
                            && !has_cmd
                            && !has_shift
                            && !agent_chat_escape_focused_text_origin
                        {
                            match &this.current_view {
                                AppView::AgentChatView { entity, .. } => entity.update(cx, |chat, cx| {
                                    chat.cancel_streaming_from_escape(cx)
                                }),
                                _ => false,
                            }
                        } else {
                            false
                        };
                        if agent_chat_escape_cancelled_streaming {
                            logging::log(
                                "KEY",
                                "Interceptor: Escape -> cancel Agent Chat streaming",
                            );
                            cx.stop_propagation();
                            return;
                        }

                        // Handle Escape for AgentChatView.
                        if crate::ui_foundation::is_key_escape(key)
                            && !has_cmd
                            && !has_shift
                            && !this.show_actions_popup
                            && !agent_chat_escape_popup_open
                            && matches!(this.current_view, AppView::AgentChatView { .. })
                        {
                            if agent_chat_escape_focused_text_origin {
                                tracing::info!(
                                    target: "script_kit::keyboard",
                                    event = "focused_text_quick_prompt_escape_hide_requested",
                                );
                                this.close_agent_chat_main_window_state_first(cx);
                                logging::log(
                                    "KEY",
                                    "Interceptor: Escape -> hide focused-text quick prompt Agent Chat",
                                );
                                cx.stop_propagation();
                                return;
                            }
                            if this.opened_from_main_menu {
                                tracing::info!(
                                    target: "script_kit::keyboard",
                                    event = "embedded_agent_chat_escape_return_to_origin",
                                );
                                this.close_tab_ai_harness_terminal_with_window(window, cx);
                                logging::log(
                                    "KEY",
                                    "Interceptor: Escape -> return to main menu from Agent Chat",
                                );
                            } else {
                                tracing::info!(
                                    target: "script_kit::keyboard",
                                    event = "embedded_agent_chat_escape_close_window",
                                );
                                this.close_agent_chat_main_window_state_first(cx);
                                logging::log(
                                    "KEY",
                                    "Interceptor: Escape -> close Agent Chat window",
                                );
                            }
                            cx.stop_propagation();
                            return;
                        }

                        // Window tweaker shortcuts (only enabled with SCRIPT_KIT_WINDOW_TWEAKER=1)
                        let window_tweaker_enabled = std::env::var("SCRIPT_KIT_WINDOW_TWEAKER")
                            .map(|v| v == "1")
                            .unwrap_or(false);

                        if window_tweaker_enabled {
                            // Handle Cmd+- to decrease light theme opacity
                            if has_cmd
                                && !has_shift
                                && (key == "-" || key.eq_ignore_ascii_case("minus"))
                            {
                                logging::log(
                                    "KEY",
                                    &format!(
                                        "Interceptor: Cmd+- (key={}) -> decrease light opacity",
                                        key
                                    ),
                                );
                                this.adjust_light_opacity(-0.05, cx);
                                cx.stop_propagation();
                                return;
                            }

                            // Handle Cmd+= (or Cmd+Shift+=) to increase light theme opacity
                            if has_cmd
                                && (key == "="
                                    || key.eq_ignore_ascii_case("equal")
                                    || key.eq_ignore_ascii_case("plus"))
                            {
                                logging::log(
                                    "KEY",
                                    &format!(
                                        "Interceptor: Cmd+= (key={}) -> increase light opacity",
                                        key
                                    ),
                                );
                                this.adjust_light_opacity(0.05, cx);
                                cx.stop_propagation();
                                return;
                            }

                            // Handle Cmd+M to cycle vibrancy material (blur effect)
                            if has_cmd && !has_shift && key.eq_ignore_ascii_case("m") {
                                logging::log(
                                    "KEY",
                                    "Interceptor: Cmd+M -> cycle vibrancy material",
                                );
                                let description = platform::cycle_vibrancy_material();
                                this.toast_manager.push(
                                    components::toast::Toast::info(description, &this.theme)
                                        .duration_ms(Some(TOAST_INFO_MS)),
                                );
                                cx.notify();
                                cx.stop_propagation();
                                return;
                            }

                            // Handle Cmd+Shift+A to cycle vibrancy appearance (VibrantLight, VibrantDark, etc.)
                            if has_cmd && has_shift && key.eq_ignore_ascii_case("a") {
                                logging::log(
                                    "KEY",
                                    "Interceptor: Cmd+Shift+A -> cycle vibrancy appearance",
                                );
                                let description = platform::cycle_appearance();
                                this.toast_manager.push(
                                    components::toast::Toast::info(description, &this.theme)
                                        .duration_ms(Some(TOAST_INFO_MS)),
                                );
                                cx.notify();
                                cx.stop_propagation();
                                return;
                            }
                        }

                        // Only handle remaining keys if in FileSearchView with actions popup open
                        if !matches!(this.current_view, AppView::FileSearchView { .. }) {
                        }
                    });
                }
            }
        });
        app.gpui_input_subscriptions.push(actions_interceptor);

        // CRITICAL FIX: Sync list state on initialization
        // This was removed when state mutations were moved out of render(),
        // but we still need to sync once during initialization so the list
        // knows about the scripts that were loaded.
        // Without this, the first render shows "No scripts or snippets found"
        // because main_list_state starts with 0 items.
        app.sync_list_state();
        app.validate_selection_bounds(cx);
        app.rebuild_main_window_preflight_if_needed();

        app
    }
}

include!("startup/navigation_routes.rs");

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn atfile_regression_input_press_enter_routes_spine_before_default_execute_selected() {
        let source = fs::read_to_string("src/app_impl/startup.rs")
            .expect("Failed to read src/app_impl/startup.rs");
        let press_enter_pos = source
            .find("InputEvent::PressEnter")
            .expect("InputEvent::PressEnter handler not found");
        let press_enter_section =
            &source[press_enter_pos..(press_enter_pos + 4500).min(source.len())];
        let consume_pos = press_enter_section
            .find("should_consume_script_list_enter_after_submit")
            .expect("submit-consume guard not found in PressEnter handler");
        let spine_pos = press_enter_section
            .find("this.try_handle_spine_enter(window, cx)")
            .expect("PressEnter must route Spine row acceptance");
        let execute_pos = press_enter_section
            .find("this.execute_selected(cx);")
            .expect("default execute_selected call not found in PressEnter handler");

        assert!(
            consume_pos < spine_pos && spine_pos < execute_pos,
            "InputEvent::PressEnter must run submit guard, then Spine row acceptance, before default execute_selected"
        );
    }
}

        // Create channel for API key configuration completion signals
        // Small buffer (4) prevents blocking, more than enough for normal use
        let (api_key_tx, api_key_rx) = mpsc::sync_channel(4);

        // Create channel for builtin confirmation modal signals
        // When a dangerous action (Quit, Shut Down, etc.) requires confirmation,
        // the modal callback sends (entry_id, confirmed) through this channel
        // Legacy chat channels (retained for TabAiChat compatibility — not the primary Tab AI surface)
        let (inline_chat_escape_tx, inline_chat_escape_rx) = mpsc::sync_channel(4);
        let (flow_chat_tx, flow_chat_rx) = mpsc::sync_channel(32);
        let (inline_chat_actions_tx, inline_chat_actions_rx) = mpsc::sync_channel(4);
        let (inline_chat_continue_tx, inline_chat_continue_rx) = mpsc::sync_channel(4);
        let (inline_chat_configure_tx, inline_chat_configure_rx) = mpsc::sync_channel(4);
        let (inline_chat_claude_code_tx, inline_chat_claude_code_rx) = mpsc::sync_channel(4);
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

        let mut app = ScriptListApp {
            scripts,
            scriptlets,
            skills: plugin_skills,
            builtin_entries,
            apps,
            // P0 FIX: Cached data for builtin views (avoids cloning per frame)
            cached_clipboard_entries: Vec::new(),
            paste_sequential_state: None,
            focused_clipboard_entry_id: None,
            cached_windows: initial_cached_windows,
            cached_browser_tabs: Vec::new(),
            cached_browser_history: Vec::new(),
            cached_file_results: Vec::new(),
            root_search,
            pending_root_file_actions_file: None,
            pending_root_unified_actions_subject: None,
            cached_processes: Vec::new(),
            process_manager_refresh_task: None,
            cached_current_app_entries: Vec::new(),
            current_app_commands_session: None,
            selected_index: 0,
            filter_text: String::new(),
            gpui_input_state,
            gpui_input_focused: false,
            gpui_input_subscriptions: vec![gpui_input_subscription],
            bounds_subscription: None,     // Set later after window setup
            appearance_subscription: None, // Set later after window setup
            frontmost_menu_subscription_task: None,
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
            arg_selection_revision: 0,
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
            arg_prompt_geometry: std::rc::Rc::new(std::cell::Cell::new(None)),
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
            flow_session_prompt_history_index: None,
            flow_session_prompt_draft: None,
            conversations: crate::ai::conversations::BackgroundedSessionStore::new(),
            flow_session_return_route: FlowConversationReturnRoute::Direct,
            flow_chat_sender: flow_chat_tx,
            flow_chat_receiver: flow_chat_rx,
            current_app_commands_scroll_handle: UniformListScrollHandle::new(),
            agent_chat_history_scroll_handle: ScrollHandle::new(),
            browser_history_scroll_handle: ScrollHandle::new(),
            dictation_history_scroll_handle: ScrollHandle::new(),
            dictation_history_previous_page: None,
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
            #[cfg(any(test, feature = "owned-ui-evaluation"))]
            file_search_stream_state: None,
            file_search_cancel: None,
            file_search_display_indices: Vec::new(),
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
            menu_syntax_filter_accept_hint_label: None,
            menu_syntax_filter_accept_hint_filter: None,
            menu_syntax_filter_accept_hint_selected_index: None,
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
            // Light theme opacity adjustment offset (Cmd+Shift+[/])
            light_opacity_offset: 0.0,
            // Mouse cursor hidden state - hidden while typing, shown on mouse move
            mouse_cursor_hidden: false,
            // Cached provider registry - built in background, None until ready
            cached_provider_registry: None,
            // Window orchestrator - pure state machine for visibility and focus
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
}
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

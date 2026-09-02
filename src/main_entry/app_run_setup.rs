{
    initialize_startup_diagnostics();
    register_startup_window_routers();
    let setup_result = prepare_startup_environment();

    let startup_boot_started = std::time::Instant::now();
    let startup_profile = crate::startup_profile::StartupProfile::from_env();
    logging::log(
        "STARTUP",
        &format!("Startup profile selected: {}", startup_profile.label()),
    );

    // Load config early so we can use it for hotkey registration AND clipboard history settings
    // This avoids duplicate config::load_config() calls (~100-300ms startup savings)
    let loaded_config = config::load_config();
    logging::log(
        "APP",
        &format!(
            "Loaded config: hotkey={:?}+{}, bun_path={:?}",
            loaded_config.hotkey.modifiers, loaded_config.hotkey.key, loaded_config.bun_path
        ),
    );
    initialize_startup_background_services(&loaded_config);

    // Clone before start_hotkey_listener consumes original
    let config_for_app = loaded_config.clone();

    let mcp_computer_runtime = std::sync::Arc::new(
        crate::computer_use::gpui_runtime_bridge::GpuiComputerUseRuntimeBridge::new(
            std::time::Duration::from_secs(10),
        ),
    );
    let mcp_computer_runtime_for_server: std::sync::Arc<
        dyn crate::computer_use::runtime_bridge::ComputerUseRuntimeBridge + Send + Sync,
    > = mcp_computer_runtime.clone();
    let (mcp_notes_ui_tx, mcp_notes_ui_rx) = async_channel::bounded(16);
    let mcp_notes_bridge: std::sync::Arc<dyn crate::mcp_notes_tools::McpNotesMutationBridge> =
        std::sync::Arc::new(crate::mcp_control::GpuiNotesMcpBridge::with_default_timeout(
            mcp_notes_ui_tx,
        ));
    let (mcp_kit_runtime_tx, mcp_kit_runtime_rx) = async_channel::bounded(100);
    let mcp_kit_runtime_bridge: std::sync::Arc<
        dyn crate::mcp_kit_tools::McpKitRuntimeBridge + Send + Sync,
    > = std::sync::Arc::new(crate::mcp_kit_tools::GpuiKitRuntimeBridge::with_default_timeout(
        mcp_kit_runtime_tx,
    ));

    // Start MCP server for AI agent integration
    // Server runs on localhost:43210 with Bearer token authentication
    // Discovery file written to ~/.scriptkit/server.json
    match mcp_server::McpServer::with_defaults() {
        Ok(server) => {
            let server = server
                .with_computer_runtime(mcp_computer_runtime_for_server)
                .with_notes_mutation_bridge(mcp_notes_bridge)
                .with_kit_runtime_bridge(mcp_kit_runtime_bridge);
            let server_url = server.url();

            match server.start() {
                Ok(handle) => {
                    logging::log(
                        "MCP",
                        &format!(
                            "MCP server started on {} (token in ~/.scriptkit/agent-token)",
                            server_url
                        ),
                    );
                    mcp_server::retain_server_handle(handle);
                }
                Err(e) => {
                    logging::log("MCP", &format!("Failed to start MCP server: {}", e));
                }
            }
        }
        Err(e) => {
            logging::log("MCP", &format!("Failed to create MCP server: {}", e));
        }
    };

    hotkeys::start_hotkey_listener(loaded_config);

    // Start watchers and track which ones succeeded
    // We only spawn poll loops for watchers that successfully started
    // Note: Appearance watching is handled by GPUI's observe_window_appearance (set up on the window)
    let (mut config_watcher, config_rx) = watcher::ConfigWatcher::new();
    logging::log(
        "APP",
        "Starting config watcher with preloaded startup config (no extra config.ts evaluation)",
    );
    let config_watcher_ok = match config_watcher.start_with_config(&config_for_app) {
        Ok(()) => {
            logging::log("APP", "Config watcher started");
            true
        }
        Err(e) => {
            logging::log("APP", &format!("Failed to start config watcher: {}", e));
            false
        }
    };

    let watcher_boot_started = std::time::Instant::now();

    let (mut script_watcher, script_rx) =
        watcher::ScriptWatcher::new_with_config(&config_for_app);
    logging::log(
        "APP",
        "Starting script watcher with preloaded startup config (no extra config.ts evaluation)",
    );
    let script_watcher_ok = match script_watcher.start() {
        Ok(()) => {
            logging::log("APP", "Script watcher started");
            true
        }
        Err(e) => {
            logging::log("APP", &format!("Failed to start script watcher: {}", e));
            false
        }
    };

    // Create AppWatcher for live application cache updates
    let (mut app_watcher, app_rx) = watcher::AppWatcher::new_with_config(&config_for_app);
    logging::log(
        "APP",
        "Starting app watcher with preloaded startup config (no extra config.ts evaluation)",
    );
    let app_watcher_ok = match app_watcher.start() {
        Ok(()) => {
            logging::log("APP", "App watcher started");
            true
        }
        Err(e) => {
            logging::log("APP", &format!("Failed to start app watcher: {}", e));
            false
        }
    };

    logging::log(
        "STARTUP",
        &format!(
            "STARTUP_WATCHERS_READY source=preloaded_config elapsed_ms={:.2} config={} script={} apps={}",
            watcher_boot_started.elapsed().as_secs_f64() * 1000.0,
            config_watcher_ok,
            script_watcher_ok,
            app_watcher_ok,
        ),
    );

    // Initialize script scheduler. In the dev-fast startup profile we move the
    // initial scan/start until after the UI is already usable.
    let (mut scheduler, scheduler_rx) = scheduler::Scheduler::new();

    if !startup_profile.should_defer_scheduler() {
        let scheduled_count = scripts::register_scheduled_scripts(&scheduler);
        logging::log(
            "APP",
            &format!("Registered {} scheduled scripts", scheduled_count),
        );

        if scheduled_count > 0 {
            if let Err(e) = scheduler.start() {
                logging::log("APP", &format!("Failed to start scheduler: {}", e));
            } else {
                logging::log("APP", "Scheduler started successfully");
            }
        } else {
            logging::log("APP", "No scheduled scripts found, scheduler not started");
        }
    } else {
        logging::log(
            "STARTUP",
            &format!(
                "Deferring scheduler bootstrap until after core readiness (profile={})",
                startup_profile.label()
            ),
        );
    }

    // Wrap scheduler in Arc<Mutex<>> for thread-safe access (needed for re-scanning on file changes)
    let scheduler = Arc::new(Mutex::new(scheduler));

    // Register URL scheme handler for scriptkit:// deeplinks
    // This must be done before .run() as it's called on Application
    // Register the embedded asset source BEFORE run(): without it the
    // Application falls back to the unit source (always Ok(None)) and every
    // svg().path(...) icon — Lucide builtin rows, EmbeddedIcon — paints
    // nothing (P0 2026-06-11). See src/utils/assets.rs.
    let app = gpui_platform::application().with_assets(crate::utils::assets::AppAssets);
    app.on_open_urls(|urls| {
        logging::log("DEEPLINK", &format!("Received {} URL(s)", urls.len()));
        for url in urls {
            logging::log("DEEPLINK", &format!("Processing URL: {}", url));
            if let Some(command_id) = parse_deeplink_url(&url) {
                logging::log("DEEPLINK", &format!("Parsed command_id: {}", command_id));
                // Send to channel for processing inside the app
                if deeplink_channel().0.try_send(command_id).is_err() {
                    logging::log(
                        "DEEPLINK",
                        "Failed to send command to channel (full or closed)",
                    );
                }
            }
        }
    });
app.run(move |cx: &mut App| {
        logging::log("APP", "GPUI Application starting");

        // Drop any loaded on-device ghost-text model before the process tears
        // down ggml's Metal backend. With the `local-llm` feature this prevents
        // a llama.cpp static-destructor abort at exit; it is a cheap no-op when
        // no local model was ever loaded. Runs on every quit path.
        cx.on_app_quit(|_cx| {
            crate::ai::local_llm::shutdown_local_ghost_llm();
            // Reap every registered child (scripts, flow runs, AI sidecars) on
            // ANY quit path, not just the tray/builtin/signal paths that call
            // kill_all_processes explicitly. Idempotent when they already ran.
            PROCESS_MANAGER.kill_all_processes();
            PROCESS_MANAGER.remove_main_pid();
            async {}
        })
        .detach();

        // Warm up the secrets cache in background thread
        // This pre-decrypts secrets.age so AI chat opens instantly instead of
        // waiting ~7s for sequential keyring lookups
        secrets::warmup_cache();

        // Configure as accessory app FIRST, before any windows are created
        // This is equivalent to LSUIElement=true in Info.plist:
        // - No Dock icon
        // - No menu bar ownership (critical for window actions to work)
        platform::configure_as_accessory_app();

        // Start frontmost app tracker - watches for app activations and pre-fetches menu bar items
        // Must be started after configure_as_accessory_app() so we're correctly classified
        #[cfg(target_os = "macos")]
        frontmost_app_tracker::start_tracking();

        if let Err(error) = window_control::load_snap_mode_from_preferences() {
            tracing::warn!(
                target: "script_kit::snap_mode",
                %error,
                "failed to hydrate snap mode from config-backed preferences"
            );
        }

        // Install the snap drag monitor — detects external window drags and
        // drives the desktop snap overlay lifecycle (start on drag, finish on release).
        if let Err(e) = window_control::install_snap_drag_monitor(cx) {
            tracing::warn!(
                target: "script_kit::snap_monitor",
                %e,
                "failed to install snap drag monitor"
            );
        }

        // Register bundled JetBrains Mono font
        // This makes "JetBrains Mono" available as a font family for the editor
        register_bundled_fonts(cx);

        // Initialize gpui-component (theme, context providers)
        // Must be called before opening windows that use Root wrapper
        gpui_component::init(cx);

        // Publish theme, component colors, and revision together before roots exist.
        if let Err(error) = theme::service::initialize_theme(cx) {
            tracing::error!(%error, "startup_theme_initialization_failed");
            return;
        }

        // Start the centralized theme service for hot-reload
        // This replaces per-window theme watchers and ensures all windows
        // stay in sync with theme.json changes
        theme::service::ensure_theme_service(cx);

        // Calculate window bounds: try saved position first, then eye-line
        let window_size = crate::window_resize::initial_window_size();
        let default_bounds = calculate_eye_line_bounds_on_mouse_display(window_size);
        let displays = platform::get_macos_displays();
        let bounds = window_state::get_initial_bounds(
            window_state::WindowRole::Main,
            default_bounds,
            &displays,
        );

        // Load theme to determine window background appearance (vibrancy)
        let initial_theme = theme::get_cached_theme();
        let window_background = if initial_theme.is_vibrancy_enabled() {
            if crate::platform::tahoe_native_glass_composition_available() {
                // Tahoe: the native NSGlassEffectView backdrop (installed by
                // configure_tahoe_window_backdrop) supplies the material. A
                // Blurred appearance would stack the fork's NSVisualEffectView
                // above the backmost glass view and hide it entirely.
                WindowBackgroundAppearance::Transparent
            } else {
                WindowBackgroundAppearance::Blurred
            }
        } else {
            WindowBackgroundAppearance::Opaque
        };
        logging::log(
            "THEME",
            &format!(
                "Window background appearance: {:?} (vibrancy_enabled={})",
                window_background,
                initial_theme.is_vibrancy_enabled()
            ),
        );

        // Store the ScriptListApp entity for direct access (needed since Root wraps the view)
        let app_entity_holder: Arc<Mutex<Option<Entity<ScriptListApp>>>> = Arc::new(Mutex::new(None));
        let app_entity_for_closure = app_entity_holder.clone();

        // Capture bun_available for use in window creation
        let bun_available = setup_result.bun_available;
        let config_for_tray_actions = config_for_app.clone();

        // Root is required for gpui_component's InputState focus tracking
        let window: WindowHandle<Root> = match cx.open_window(
            match main_window_options(bounds, window_background, crate::runtime_policy::WindowHostPolicy::Interactive) {
                Ok(options) => options,
                Err(error) => { tracing::error!(%error, "main_window_policy_refused"); return; }
            },
            |window, cx| {
                logging::log("APP", "Window opened, creating ScriptListApp wrapped in Root");
                let view = cx.new(|cx| ScriptListApp::new(config_for_app, bun_available, window, cx));
                let view_for_close = view.clone();
                window.on_window_should_close(cx, move |window, cx| {
                    view_for_close.update(cx, |this, cx| {
                        if matches!(this.current_view, AppView::AgentChatView { .. }) {
                            tracing::info!(
                                target: "script_kit::keyboard",
                                event = "embedded_agent_chat_native_close_window",
                            );
                            this.close_tab_ai_harness_terminal_with_window(window, cx);
                        }
                        this.close_and_reset_window(cx);
                        this.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::SurfaceClosedBySystem(
                                crate::window_orchestrator::SurfaceId::Main,
                            ),
                            cx,
                        );
                    });
                    false
                });
                // Store the entity for external access
                *app_entity_for_closure.lock().unwrap_or_else(|e| e.into_inner()) = Some(view.clone());
                cx.new(|cx| Root::new_transparent(view, window, cx))
            },
        ) {
            Ok(window) => {
                // Store the main window handle globally so async contexts can
                // open parent dialogs without needing a Window reference.
                if let Err(error) = register_main_runtime_window(window.into(), bounds, cx) {
                    tracing::error!(%error, "main_window_registration_failed");
                    let _ = window.update(cx, |_, window, _| window.remove_window());
                    return;
                }
                window
            }
            Err(error) => {
                logging::log(
                    "APP",
                    &format!("Failed to open main window during startup: {}", error),
                );
                return;
            }
        };

        // Extract the app entity for use in callbacks
        let app_entity = match app_entity_holder
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            Some(app_entity) => app_entity,
            None => {
                logging::log(
                    "APP",
                    "Main app entity missing after window creation; aborting startup initialization",
                );
                return;
            }
        };

        // Permission onboarding: fresh installs (or installs that never
        // completed onboarding) with missing required permissions get the
        // wizard; completed installs get a reminder. Deferred past first
        // paint so it never races window setup.
        {
            let is_fresh_install = setup_result.is_fresh_install;
            let app_entity_for_permissions = app_entity.clone();
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(800))
                    .await;
                cx.update(|cx| {
                    app_entity_for_permissions.update(cx, |this, cx| {
                        this.apply_permission_startup_intent(is_fresh_install, cx);
                    });
                });
            })
            .detach();
        }

        let (mcp_computer_ui_tx, mcp_computer_ui_rx) = async_channel::bounded(16);
        mcp_computer_runtime.install(mcp_computer_ui_tx);
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            while let Ok(command) = async_channel::Receiver::recv(&mcp_notes_ui_rx).await {
                let result = cx.update(|cx| {
                    crate::notes::apply_mcp_notes_mutation_on_main_thread(command.request, cx)
                });
                let _ = command.response_tx.send(result);
            }
        })
        .detach();

        let app_entity_for_mcp_computer = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            while let Ok(request) = mcp_computer_ui_rx.recv().await {
                match request {
                    crate::computer_use::gpui_runtime_bridge::GpuiComputerUseRequest::InspectAutomationWindow {
                        request_id,
                        request,
                        response_tx,
                    } => {
                        let app_entity = app_entity_for_mcp_computer.clone();
                        let snapshot = cx.update(|cx| {
                            app_entity.update(cx, |app, app_cx| {
                                app.build_automation_inspect_snapshot(
                                    &request_id,
                                    request.target.as_ref(),
                                    request.hi_dpi,
                                    &request.probes,
                                    app_cx,
                                )
                            })
                        });
                        let _ = response_tx.send(Ok(snapshot));
                    }
                    crate::computer_use::gpui_runtime_bridge::GpuiComputerUseRequest::ListRunningApps {
                        request,
                        response_tx,
                        ..
                    } => {
                        let result = cx.update(|_| {
                            crate::computer_use::gpui_runtime_bridge::list_running_apps_on_gpui_thread(
                                &request,
                            )
                        });
                        let _ = response_tx.send(result);
                    }
                    crate::computer_use::gpui_runtime_bridge::GpuiComputerUseRequest::ListAppWindows {
                        request,
                        response_tx,
                        ..
                    } => {
                        let result = cx.update(|_| {
                            crate::computer_use::gpui_runtime_bridge::list_app_windows_on_gpui_thread(
                                &request,
                            )
                        });
                        let _ = response_tx.send(result);
                    }
                    crate::computer_use::gpui_runtime_bridge::GpuiComputerUseRequest::CaptureNativeWindow {
                        request,
                        response_tx,
                        ..
                    } => {
                        let result = cx.update(|_| {
                            crate::computer_use::gpui_runtime_bridge::capture_native_window_on_gpui_thread(
                                &request,
                            )
                        });
                        let _ = response_tx.send(result);
                    }
                    crate::computer_use::gpui_runtime_bridge::GpuiComputerUseRequest::CaptureRenderWindow {
                        request,
                        response_tx,
                        ..
                    } => {
                        let result = cx.update(|cx| {
                            crate::computer_use::gpui_runtime_bridge::capture_render_window_on_gpui_thread(
                                &request,
                                cx,
                            )
                        });
                        let _ = response_tx.send(result);
                    }
                }
            }
        })
        .detach();

        // Set initial focus via the Root window
        // We access the app entity within the window context to properly focus it
        let app_entity_for_focus = app_entity.clone();
        let update_result = window
            .update(cx, |_root, win, root_cx| {
                app_entity_for_focus.update(root_cx, |view, ctx| {
                    let focus_handle = view.focus_handle(ctx);
                    win.focus(&focus_handle, ctx);
                    logging::log("APP", "Focus set on ScriptListApp via Root");
                    // Subscribe to window bounds changes to save position when user drags the window.
                    // This persists the position per-display so it can be restored on next show.
                    // Store subscription in view to keep it alive.
                    view.bounds_subscription = Some(ctx.observe_window_bounds(win, |view, win, ctx| {
                        // Only save if window is visible (avoid saving during initial positioning)
                        if is_main_window_visible() {
                            if let Some((x, y, width, height)) = platform::get_main_window_bounds() {
                                let displays = platform::get_macos_displays();
                                let bounds = window_state::PersistedWindowBounds::new(x, y, width, height);
                                if let Some(display) = window_state::find_display_for_bounds(&bounds, &displays) {
                                    logging::log(
                                        "WINDOW_BOUNDS",
                                        &format!(
                                            "Bounds changed - saving position for display {}: ({:.0}, {:.0})",
                                            window_state::display_key(display),
                                            x,
                                            y
                                        ),
                                    );
                                    window_state::save_main_position_for_display(display, bounds);
                                }
                            }
                        }
                        sync_main_automation_window(
                            Some(automation_window_bounds_from_gpui(win.bounds())),
                            script_kit_gpui::is_main_window_visible(),
                            crate::platform::is_main_window_focused(),
                        );
                        view.sync_main_footer_popup(win, ctx);
                        // Suppress unused variable warning - we need win to access window bounds
                        let _ = win;
                    }));

                    // Observe window appearance changes (GPUI fires this when macOS changes light/dark mode)
                    view.appearance_subscription = Some(ctx.observe_window_appearance(win, |view, win, ctx| {
                        logging::log("APP", "System appearance changed, reloading theme");

                        // Invalidate the cached appearance detection so
                        // detect_system_appearance() gets the fresh value
                        theme::invalidate_appearance_cache();

                        if let Err(error) = theme::service::reload_theme(ctx, theme::service::ThemePublicationSource::Appearance) {
                            tracing::warn!(%error, "appearance_theme_reload_failed");
                            return;
                        }

                        // Update the app entity theme
                        view.update_theme(ctx);
                        view.sync_main_footer_popup(win, ctx);

                    }));
                });
            });
        if let Err(error) = update_result {
            logging::log(
                "APP",
                &format!(
                    "Failed to initialize main window focus/subscriptions during startup: {}",
                    error
                ),
            );
            return;
        }

        // Emit STARTUP_READY marker — autonomous agents can begin interacting now.
        if startup_profile.ready_log_enabled() {
            logging::log(
                "STARTUP",
                &format!(
                    "STARTUP_READY profile={} core_boot_ms={:.2}",
                    startup_profile.label(),
                    startup_boot_started.elapsed().as_secs_f64() * 1000.0
                ),
            );
        }

        // Deferred scheduler bootstrap — runs after core readiness so agents
        // don't have to wait for the full script-tree scan before interacting.
        if startup_profile.should_defer_scheduler() {
            let scheduler_for_startup = scheduler.clone();
            let startup_boot_started_for_scheduler = startup_boot_started;
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                let delay = startup_profile.deferred_scheduler_delay();
                cx.background_executor().timer(delay).await;

                if let Ok(mut scheduler_guard) = scheduler_for_startup.lock() {
                    let count = scripts::register_scheduled_scripts(&scheduler_guard);
                    logging::log("APP", &format!("Registered {} scheduled scripts", count));
                    if count > 0 {
                        if let Err(e) = scheduler_guard.start() {
                            logging::log("APP", &format!("Failed to start scheduler: {}", e));
                        } else {
                            logging::log("APP", "Scheduler started successfully");
                        }
                    } else {
                        logging::log("APP", "No scheduled scripts found, scheduler not started");
                    }
                }

                logging::log(
                    "STARTUP",
                    &format!(
                        "STARTUP_DEFERRED_SCHEDULER profile={} elapsed_ms={:.2}",
                        startup_profile.label(),
                        startup_boot_started_for_scheduler.elapsed().as_secs_f64() * 1000.0
                    ),
                );
            })
            .detach();
        }

        // Register the main window with WindowManager before tray init
        // This must happen after GPUI creates the window but before tray creates its windows
        // so we can reliably find our main window by its expected size (~750x500)
        window_manager::find_and_register_main_window();

        // KNOWN: Swizzle GPUI's BlurredView must happen immediately after window creation
        // GPUI hides the native macOS CAChameleonLayer (vibrancy tint) on every frame.
        // By swizzling now (before any rendering), we preserve the native tint effect.
        // This gives us Raycast/Spotlight-like vibrancy appearance.
        platform::swizzle_gpui_blurred_view();

        // Window starts hidden - no activation, no panel configuration yet
        // Panel will be configured on first show via hotkey
        // WINDOW_VISIBLE is already false by default (static initializer)
        logging::log("HOTKEY", "Window created but not shown (use hotkey to show)");

        // Defer tray initialization until after window creation so startup-to-first-render
        // is not blocked by tray icon rendering/menu construction.
        let tray_ready = Arc::new(AtomicBool::new(false));
        let tray_ready_for_fallback = Arc::clone(&tray_ready);
        let window_for_tray = window;
        let app_entity_for_tray = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            // Yield once so window creation and initial render can proceed first.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;

            let update_state = std::sync::Arc::new(std::sync::RwLock::new(
                crate::updates::UpdateState::Idle,
            ));
            // Mirror the user's global hotkeys on the tray rows (launcher,
            // Notes, Agent Chat) so the menu and the shortcuts stay in
            // lockstep instead of hiding the bindings.
            let tray_shortcuts = tray::TrayShortcuts {
                main: tray::main_shortcut_accelerator(&config_for_tray_actions.hotkey),
                notes: config_for_tray_actions
                    .get_notes_hotkey()
                    .as_ref()
                    .and_then(tray::main_shortcut_accelerator),
                agent_chat: config_for_tray_actions
                    .get_ai_hotkey()
                    .as_ref()
                    .and_then(tray::main_shortcut_accelerator),
            };
            let tray_manager = cx.update(|_cx| match TrayManager::new(update_state.clone(), tray_shortcuts.clone()) {
                Ok(tm) => {
                    logging::log("TRAY", "Tray icon initialized successfully (deferred)");
                    Some(tm)
                }
                Err(e) => {
                    logging::log(
                        "TRAY",
                        &format!("Failed to initialize tray icon (deferred): {}", e),
                    );
                    None
                }
            });

            let Some(tray_mgr) = tray_manager else {
                return;
            };

            // Kick off a background update check shortly after launch.
            // We poll `UpdateState` on the dispatcher's GPUI task whenever we
            // need to refresh the Version row — the worker thread itself
            // cannot mutate muda menu items (main-thread only on macOS).
            if !std::env::var("SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
            {
                let state = tray_mgr.update_state_handle();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    crate::updates::check_now(
                        state,
                        crate::updates::CheckKind::Automatic,
                        || {},
                    );
                });
            }

            tray_ready.store(true, Ordering::SeqCst);
            logging::log("TRAY", "Tray menu event handler started (event-driven)");
            let _menu_event_receiver = tray_mgr.menu_event_receiver();

            let (tray_event_tx, tray_event_rx) = async_channel::bounded(32);
            std::thread::spawn(move || {
                logging::log("TRAY", "Tray menu receiver bridge started (blocking recv)");
                let menu_rx = tray_icon::menu::MenuEvent::receiver();

                while let Ok(event) = menu_rx.recv() {
                    if tray_event_tx.send_blocking(event).is_err() {
                        logging::log(
                            "TRAY",
                            "Tray menu receiver bridge exiting: event channel closed",
                        );
                        break;
                    }
                }

                logging::log("TRAY", "Tray menu receiver bridge exiting");
            });

            while let Ok(event) = tray_event_rx.recv().await {
                // Refresh dynamic rows from main-thread state before every
                // tray dispatch; tray-icon/muda do not expose menu-will-open.
                cx.update(|_cx| {
                    tray_mgr.refresh_current_app_label();
                });

                // Convert event to action using type-safe IDs (pure function)
                let action = TrayManager::action_from_event(&event);

                match action {
                    Some(TrayMenuAction::OpenScriptKit) => {
                        logging::log("TRAY", "Open Script Kit menu item clicked");
                        let window_inner = window_for_tray;
                        let app_entity_inner = app_entity_for_tray.clone();
                        cx.update(|cx| {
                            show_main_window_helper(window_inner, app_entity_inner, cx);
                        });
                    }
                    Some(TrayMenuAction::OpenCurrentAppCommands) => {
                        logging::log("TRAY", "Current App Commands menu item clicked");
                        let window_inner = window_for_tray;
                        let app_entity_inner = app_entity_for_tray.clone();
                        cx.update(|cx| {
                            show_main_window_helper(window_inner, app_entity_inner.clone(), cx);
                            app_entity_inner.update(cx, |view, cx| {
                                view.mark_opened_directly("tray");
                                if let Err(e) = view.open_current_app_commands_from_tray(cx) {
                                    let message = e.to_string();
                                    tracing::warn!(
                                        error = %message,
                                        "tray.open_current_app_commands_failed"
                                    );
                                    view.show_error_toast(message, cx);
                                }
                            });
                        });
                    }
                    Some(TrayMenuAction::OpenNotes) => {
                        logging::log("TRAY", "Open Notes menu item clicked");
                        cx.update(|cx| {
                            if let Err(e) = notes::open_notes_window_without_launcher_restore(cx) {
                                logging::log(
                                    "TRAY",
                                    &format!("Failed to open Notes window: {}", e),
                                );
                            }
                        });
                    }
                    Some(TrayMenuAction::OpenAgentChat) => {
                        logging::log("TRAY", "Open Agent Chat menu item clicked");
                        let window_inner = window_for_tray;
                        let app_entity_inner = app_entity_for_tray.clone();
                        cx.update(|cx| {
                            show_main_window_helper(window_inner, app_entity_inner.clone(), cx);
                            app_entity_inner.update(cx, |view, cx| {
                                view.mark_opened_directly("tray");
                                view.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                            });
                        });
                    }
                    Some(TrayMenuAction::Settings) => {
                        logging::log("TRAY", "Settings menu item clicked");
                        // Open config file in editor
                        let editor = config_for_tray_actions.get_editor();
                        let config_path =
                            shellexpand::tilde("~/.scriptkit/config.ts").to_string();

                        logging::log(
                            "TRAY",
                            &format!("Opening {} in editor '{}'", config_path, editor),
                        );
                        match std::process::Command::new(&editor)
                            .arg(&config_path)
                            .spawn()
                        {
                            Ok(_) => logging::log("TRAY", &format!("Spawned editor: {}", editor)),
                            Err(e) => logging::log(
                                "TRAY",
                                &format!("Failed to spawn editor '{}': {}", editor, e),
                            ),
                        }
                    }
                    Some(TrayMenuAction::ReloadScripts) => {
                        logging::log("TRAY", "Reload Scripts menu item clicked");
                        let app_entity_inner = app_entity_for_tray.clone();
                        cx.update(|cx| {
                            app_entity_inner.update(cx, |view, ctx| {
                                view.refresh_scripts(ctx);
                            });
                        });
                    }
                    Some(TrayMenuAction::CheckForUpdates) => {
                        logging::log("TRAY", "Check for Updates clicked");
                        let state = tray_mgr.update_state_handle();
                        let (complete_tx, complete_rx) = async_channel::bounded(1);
                        crate::updates::check_now(
                            state,
                            crate::updates::CheckKind::Manual,
                            move || {
                                let _ = complete_tx.send_blocking(());
                            },
                        );
                        tray_mgr.refresh_version_label();
                        let _ = complete_rx.recv().await;
                        tray_mgr.refresh_version_label();
                    }
                    Some(TrayMenuAction::OpenReleasePage) => {
                        let snapshot = tray_mgr.update_state_snapshot();
                        if let Some(url) = snapshot.release_page_url() {
                            logging::log("TRAY", &format!("Opening release page: {}", url));
                            if let Err(e) = open::that(url) {
                                logging::log("TRAY", &format!("Failed to open release page: {}", e));
                            }
                        }
                    }
                    Some(TrayMenuAction::SendFeedback) => {
                        if let Err(e) = open::that(tray::URL_FEEDBACK) {
                            logging::log("TRAY", &format!("Failed to open feedback: {}", e));
                        }
                    }
                    Some(TrayMenuAction::FollowUs) => {
                        if let Err(e) = open::that(tray::URL_FOLLOW_US) {
                            logging::log("TRAY", &format!("Failed to open Follow Us URL: {}", e));
                        }
                    }
                    Some(TrayMenuAction::OpenGitHub) => {
                        if let Err(e) = open::that(tray::URL_GITHUB) {
                            logging::log("TRAY", &format!("Failed to open GitHub: {}", e));
                        }
                    }
                    Some(TrayMenuAction::JoinDiscord) => {
                        if let Err(e) = open::that(tray::URL_DISCORD) {
                            logging::log("TRAY", &format!("Failed to open Discord: {}", e));
                        }
                    }
                    Some(TrayMenuAction::OpenAbout) => {
                        logging::log("TRAY", "About Script Kit clicked");
                        let window_inner = window_for_tray;
                        let app_entity_inner = app_entity_for_tray.clone();
                        let state = tray_mgr.update_state_handle();
                        cx.update(|cx| {
                            show_main_window_helper(window_inner, app_entity_inner.clone(), cx);
                        });
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(10))
                            .await;
                        cx.update(|cx| {
                            app_entity_inner.update(cx, |view, cx| {
                                view.open_about_surface(state, cx);
                            });
                        });
                    }
                    Some(TrayMenuAction::Quit) => {
                        logging::log("TRAY", "Quit menu item clicked");
                        // Set shutdown flag FIRST - prevents new script spawns
                        // and triggers the shutdown monitor task for unified cleanup
                        SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);

                        // Clean up processes and PID file before quitting
                        PROCESS_MANAGER.kill_all_processes();
                        PROCESS_MANAGER.remove_main_pid();
                        cx.update(|cx| {
                            cx.quit();
                        });
                        break;
                    }
                    None => {
                        logging::log("TRAY", "Unknown menu event received");
                    }
                }
            }

            logging::log("TRAY", "Tray menu event handler exiting");
        })
        .detach();

        // Fallback: If both hotkey AND tray fail, the user has no way to access the app!
        // Wait a short time for hotkey registration, then check if we need to show the window.
        let window_for_fallback = window;
        let app_entity_for_fallback = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            // Wait 500ms for hotkey registration to complete (it runs in a separate thread)
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;

            let hotkey_ok = hotkeys::is_main_hotkey_registered();
            let tray_ok = tray_ready_for_fallback.load(Ordering::SeqCst);

            if !hotkey_ok && !tray_ok {
                logging::log("APP", "");
                logging::log("APP", "╔════════════════════════════════════════════════════════════════════════════╗");
                logging::log("APP", "║  WARNING: Both hotkey and tray initialization failed!                     ║");
                logging::log("APP", "║  Showing window at startup as fallback entry point.                       ║");
                logging::log("APP", "║  Check logs for specific errors.                                          ║");
                logging::log("APP", "╚════════════════════════════════════════════════════════════════════════════╝");
                logging::log("APP", "");

                // Show window using the centralized helper
                cx.update(|cx| {
                    show_main_window_helper(window_for_fallback, app_entity_for_fallback, cx);
                });
            } else {
                logging::log("APP", &format!("Entry points available: hotkey={}, tray={}", hotkey_ok, tray_ok));
            }
        }).detach();

        // Main window hotkey listener — gesture classifier (key-down show, tap toggle, double-tap Agent Chat)
        spawn_main_hotkey_gesture_listener(cx, app_entity.clone(), window);

        // Notes hotkey listener - event-driven via async_channel
        // The hotkey thread dispatches via GPUI's ForegroundExecutor, which wakes this task
        // This works even before main window activates because the executor is initialized first
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Notes hotkey listener started (event-driven)");
            // Event-driven: .recv().await blocks until a message arrives
            // This is more efficient than polling and responds immediately
            while let Ok(hotkey_event) = hotkeys::notes_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                logging::log("HOTKEY", "Notes hotkey triggered - opening notes window");
                cx.update(|cx: &mut gpui::App| {
                    if let Err(e) = notes::open_notes_window(cx) {
                        logging::log("HOTKEY", &format!("Failed to open notes window: {}", e));
                    }
                });
            }
            logging::log("HOTKEY", "Notes hotkey listener exiting (channel closed)");
        }).detach();

        // Dev marker listener — only receives events when ./dev.sh opted in via
        // SCRIPT_KIT_DEV_MARKER_HOTKEY=1.
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Dev marker hotkey listener started (event-driven)");
            while let Ok(hotkey_event) = hotkeys::dev_marker_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                logging::log("HOTKEY", "Dev marker hotkey triggered");
                cx.update(|cx: &mut gpui::App| {
                    crate::dev_marker::handle_hotkey(cx);
                });
            }
            logging::log("HOTKEY", "Dev marker hotkey listener exiting (channel closed)");
        }).detach();

        // AI hotkey listener - event-driven via async_channel
        // Same pattern as Notes hotkey - works immediately on app launch
        let app_entity_for_ai_hotkey = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "AI hotkey listener started (event-driven)");
            // Event-driven: .recv().await blocks until a message arrives
            while let Ok(hotkey_event) = hotkeys::ai_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                logging::log("HOTKEY", "AI hotkey triggered - opening Agent Chat");
                cx.update(|cx: &mut gpui::App| {
                    app_entity_for_ai_hotkey.update(cx, |view, cx| {
                        view.mark_opened_directly("shortcut");
                        view.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                    });
                });
            }
            logging::log("HOTKEY", "AI hotkey listener exiting (channel closed)");
        }).detach();

        // Inline AI text-edit listener. Capture happens inside the foreground
        // update before any inline overlay window is opened, preserving the
        // external app's focused text field as the accessibility target.
        let app_entity_for_inline_ai = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Inline AI hotkey listener started (event-driven)");
            while let Ok(hotkey_event) = hotkeys::inline_ai_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                logging::log(
                    "HOTKEY",
                    "Inline AI hotkey triggered - capturing focused text field",
                );
                cx.update(|cx: &mut gpui::App| {
                    app_entity_for_inline_ai.update(cx, |view, cx| {
                        view.dismiss_focused_text_agent_chat_before_recapture(cx);
                    });
                    let capture_started = std::time::Instant::now();
                    match crate::platform::accessibility::capture_focused_text_field(
                        crate::platform::accessibility::CaptureFocusedTextOptions::default(),
                    ) {
                        Ok(snapshot) => {
                            logging::log(
                                "HOTKEY",
                                &format!(
                                    "Inline AI focused text captured before Agent Chat open (chars={}, elapsed_ms={})",
                                    snapshot.metrics.chars,
                                    capture_started.elapsed().as_millis()
                                ),
                            );
                            tracing::info!(
                                target: "script_kit::focused_text",
                                event = "focused_text_capture_complete_before_agent_chat",
                                source = "app_run_setup",
                                session_id = %snapshot.session_id,
                                app_name = %snapshot.app.name,
                                chars = snapshot.metrics.chars,
                                elapsed_ms = capture_started.elapsed().as_millis() as u64,
                            );
                            app_entity_for_inline_ai.update(cx, |view, cx| {
                                view.open_focused_text_agent_chat_from_snapshot(
                                    snapshot,
                                    None,
                                    "inline_ai_hotkey",
                                    cx,
                                );
                            });
                        }
                        Err(error) => {
                            logging::log(
                                "HOTKEY",
                                &format!("Failed to capture focused text for inline AI: {}", error),
                            );
                            tracing::warn!(
                                target: "script_kit::focused_text",
                                event = "focused_text_capture_failed",
                                source = "app_run_setup",
                                error = %error,
                            );
                            app_entity_for_inline_ai.update(cx, |view, cx| {
                                view.open_focused_text_agent_chat_from_capture_failure(
                                    error,
                                    None,
                                    "inline_ai_hotkey",
                                    cx,
                                );
                            });
                        }
                    }
                });
            }
            logging::log("HOTKEY", "Inline AI hotkey listener exiting (channel closed)");
        }).detach();

        // Rewrite hotkey listener. Captures the focused field and immediately
        // streams three rewrite variations in the mini UI (no typing needed).
        let app_entity_for_rewrite = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Rewrite hotkey listener started (event-driven)");
            while let Ok(hotkey_event) = hotkeys::rewrite_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                logging::log(
                    "HOTKEY",
                    "Rewrite hotkey triggered - starting instant rewrite capture",
                );
                cx.update(|cx: &mut gpui::App| {
                    app_entity_for_rewrite.update(cx, |view, cx| {
                        view.open_instant_rewrite_mini("rewrite_hotkey", cx);
                    });
                });
            }
            logging::log("HOTKEY", "Rewrite hotkey listener exiting (channel closed)");
        }).detach();

        // Dictation hotkey listener - event-driven via async_channel
        // The global dictation shortcut routes through the config-governed
        // dictation builtin: `dictation.target` decides the destination
        // (sticky last chip pick by default, context capture or an explicit
        // label otherwise). The forced Agent Chat route stays available as
        // the builtin/dictation-to-ai menu entry.
        let app_entity_for_dictation = app_entity.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Dictation hotkey listener started (event-driven)");
            while let Ok(hotkey_event) = hotkeys::dictation_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                logging::log(
                    "HOTKEY",
                    "Dictation hotkey triggered - toggling dictation via builtin",
                );
                let app_entity_inner = app_entity_for_dictation.clone();
                cx.update(move |cx: &mut gpui::App| {
                    let should_show_window = app_entity_inner.update(cx, |view, ctx| {
                        view.execute_by_command_id_or_path("builtin/dictation", ctx)
                    });
                    if should_show_window {
                        logging::log(
                            "HOTKEY",
                            "Dictation returned should_show_window=true unexpectedly; suppressing main window",
                        );
                    }
                });
            }
            logging::log("HOTKEY", "Dictation hotkey listener exiting (channel closed)");
        }).detach();

        // Script/Scriptlet/App hotkey listener - event-driven via async_channel
        // Handles config.ts and metadata shortcuts for scriptlets, builtins, and apps
        let app_entity_for_scripts = app_entity.clone();
        let window_for_scripts = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("HOTKEY", "Script shortcut listener started (event-driven)");
            while let Ok(hotkey_event) = hotkeys::script_hotkey_channel().1.recv().await {
                let _guard = logging::set_correlation_id(hotkey_event.correlation_id.clone());
                let command_id = hotkey_event.command_id;
                logging::log(
                    "HOTKEY",
                    &format!("Script shortcut received in main.rs: {}", command_id),
                );

                let id_clone = command_id.clone();
                let app_entity_inner = app_entity_for_scripts.clone();
                let window_inner = window_for_scripts;

                cx.update(move |cx: &mut gpui::App| {
                    logging::log(
                        "HOTKEY",
                        &format!("Executing command_id: {}", id_clone),
                    );

                    // Clear NEEDS_RESET before executing new command - we're starting a fresh script
                    // and shouldn't reset based on previous script's exit state
                    if NEEDS_RESET.swap(false, Ordering::SeqCst) {
                        logging::log(
                            "HOTKEY",
                            "Cleared NEEDS_RESET before executing new command",
                        );
                    }

                    // Use app_entity.update to access ScriptListApp directly.
                    // Interactive scripts reopen the window later if they emit a prompt.
                    let should_show_window = app_entity_inner.update(cx, |view, ctx| {
                        logging::log(
                            "HOTKEY",
                            "Inside app_entity update, calling execute_by_command_id_or_path",
                        );
                        view.mark_opened_directly("shortcut");
                        view.execute_by_command_id_or_path(&id_clone, ctx)
                    });

                    // Only show window if command needs it AND it's currently hidden
                    if should_show_window && !script_kit_gpui::is_main_window_visible() {
                        logging::log("HOTKEY", "Command needs main window, showing it");
                        show_main_window_helper(window_inner, app_entity_inner.clone(), cx);
                    } else if !should_show_window {
                        logging::log("HOTKEY", "Command does not need an immediate main window");
                    }
                });
            }
            logging::log("HOTKEY", "Script shortcut listener exiting (channel closed)");
        }).detach();

        // Deeplink listener - handles scriptkit:// URLs (same logic as hotkeys)
        let app_entity_for_deeplinks = app_entity.clone();
        let window_for_deeplinks = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("DEEPLINK", "Deeplink listener started (event-driven)");
            while let Ok(command_id) = deeplink_channel().1.recv().await {
                logging::log(
                    "DEEPLINK",
                    &format!("Processing deeplink command: {}", command_id),
                );

                // Handle special deeplink types
                if command_id.starts_with("notes/") {
                    // Notes deeplink - open the specific note when a valid id is present,
                    // otherwise just toggle the notes window.
                    let note_id = command_id.strip_prefix("notes/").unwrap_or("");
                    logging::log("DEEPLINK", &format!("Opening notes (note_id: {})", note_id));
                    let parsed_note_id = notes::NoteId::parse(note_id);
                    cx.update(move |cx| {
                        let result = match parsed_note_id {
                            Some(id) => notes::open_note_in_notes_window(cx, id),
                            None => notes::open_notes_window(cx),
                        };
                        if let Err(e) = result {
                            logging::log("DEEPLINK", &format!("Failed to open notes: {}", e));
                        }
                    });
                    continue;
                }

                if command_id.starts_with("agent-chat/") {
                    // Provenance deeplink from a note's `source:` frontmatter.
                    // Opens Agent Chat; thread-specific restore can layer on
                    // once history navigation is addressable by thread id.
                    logging::log("DEEPLINK", &format!("Opening Agent Chat ({})", command_id));
                    let app_entity_inner = app_entity_for_deeplinks.clone();
                    cx.update(move |cx: &mut gpui::App| {
                        app_entity_inner.update(cx, |view, cx| {
                            view.mark_opened_directly("deeplink");
                            view.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                        });
                    });
                    continue;
                }

                // Security gate: the scriptkit:// scheme is web-reachable, so
                // refuse to silently execute scripts/scriptlets/paths from a
                // drive-by deeplink unless the user has opted in. UI-opening and
                // app-launch deeplinks still pass through.
                if !deeplink_execution_allowed(&command_id) {
                    logging::log(
                        "DEEPLINK",
                        &format!(
                            "Blocked script-executing deeplink '{}' (set SCRIPT_KIT_ALLOW_DEEPLINK_SCRIPTS=1 to allow)",
                            command_id
                        ),
                    );
                    continue;
                }

                let id_clone = command_id.clone();
                let app_entity_inner = app_entity_for_deeplinks.clone();
                let window_inner = window_for_deeplinks;

                cx.update(move |cx: &mut gpui::App| {
                    logging::log(
                        "DEEPLINK",
                        &format!("Executing command_id: {}", id_clone),
                    );

                    // Clear NEEDS_RESET before executing new command - we're starting a fresh script
                    // and shouldn't reset based on previous script's exit state
                    if NEEDS_RESET.swap(false, Ordering::SeqCst) {
                        logging::log(
                            "DEEPLINK",
                            "Cleared NEEDS_RESET before executing new command",
                        );
                    }

                    // Use app_entity.update to access ScriptListApp directly.
                    // Interactive scripts reopen the window later if they emit a prompt.
                    let should_show_window = app_entity_inner.update(cx, |view, ctx| {
                        logging::log(
                            "DEEPLINK",
                            "Inside app_entity update, calling execute_by_command_id_or_path",
                        );
                        view.mark_opened_directly("deeplink");
                        view.execute_by_command_id_or_path(&id_clone, ctx)
                    });

                    // Only show window if command needs it AND it's currently hidden
                    if should_show_window && !script_kit_gpui::is_main_window_visible() {
                        logging::log("DEEPLINK", "Command needs main window, showing it");
                        show_main_window_helper(window_inner, app_entity_inner.clone(), cx);
                    } else if !should_show_window {
                        logging::log("DEEPLINK", "Command does not need an immediate main window");
                    }
                });
            }
            logging::log("DEEPLINK", "Deeplink listener exiting (channel closed)");
        }).detach();

        // Show window listener - handles requests to show main window after script exit
        // This is needed because prompt_handler doesn't have access to show_main_window_helper
        let app_entity_for_show = app_entity.clone();
        let window_for_show = window;
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            logging::log("VISIBILITY", "Show window listener started (event-driven)");
            let (_, rx) = script_kit_gpui::show_window_channel();
            while let Ok(()) = rx.recv().await {
                logging::log(
                    "VISIBILITY",
                    "Show window request received - bringing main menu back",
                );
                let app_entity_inner = app_entity_for_show.clone();
                let window_inner = window_for_show;
                cx.update(move |cx: &mut gpui::App| {
                    show_main_window_helper(window_inner, app_entity_inner, cx);
                });
            }
            logging::log("VISIBILITY", "Show window listener exiting (channel closed)");
        }).detach();

        // Note: Appearance watching is now handled by GPUI's observe_window_appearance
        // (set up during window creation above), replacing the custom AppearanceWatcher.

        // Config reload watcher - watches ~/.scriptkit/config.ts for changes
        // Only spawn if watcher started successfully
        // Uses adaptive polling: starts at 200ms, increases to 2s when idle
        if config_watcher_ok {
            let app_entity_for_config = app_entity.clone();
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                let mut idle_count = 0u32;
                loop {
                    // Adaptive polling: 200ms when active, up to 2000ms when idle
                    // After 5 idle checks (1s), increase to 500ms
                    // After 10 idle checks (3.5s), increase to 2000ms
                    let poll_interval = if idle_count < 5 {
                        200
                    } else if idle_count < 10 {
                        500
                    } else {
                        2000
                    };
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(poll_interval))
                        .await;

                    if config_rx.try_recv().is_ok() {
                        idle_count = 0; // Reset on activity
                        logging::log("APP", "Config file changed, reloading");
                        cx.update(|cx| {
                            app_entity_for_config.update(cx, |view, ctx| {
                                view.update_config(ctx);
                            });
                        });
                    } else {
                        idle_count = idle_count.saturating_add(1);
                    }
                }
            }).detach();
        }

        // Script/scriptlets reload watcher - watches ~/.scriptkit/plugins/*/scripts/ and ~/.scriptkit/plugins/*/scriptlets/
        // Uses incremental updates for scriptlet files, full reload for scripts
        // Also re-scans for scheduled scripts to pick up new/modified schedules
        // Only spawn if watcher started successfully
        // Uses adaptive polling: starts at 200ms, increases to 2s when idle
        if script_watcher_ok {
            let app_entity_for_scripts = app_entity.clone();
            let scheduler_for_scripts = scheduler.clone();
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                use watcher::ScriptReloadEvent;
                let mut idle_count = 0u32;
                let is_skill_file = |path: &std::path::Path| {
                    let file_name = match path.file_name().and_then(|name| name.to_str()) {
                        Some(name) => name,
                        None => return false,
                    };

                    if file_name != "SKILL.md" {
                        return false;
                    }

                    let skill_root = match path.parent() {
                        Some(parent) => parent,
                        None => return false,
                    };

                    let skill_container = match skill_root.parent() {
                        Some(parent) => parent,
                        None => return false,
                    };

                    skill_container
                        .file_name()
                        .and_then(|name| name.to_str())
                        == Some("skills")
                };

                loop {
                    // Adaptive polling: 200ms when active, up to 2000ms when idle
                    let poll_interval = if idle_count < 5 {
                        200
                    } else if idle_count < 10 {
                        500
                    } else {
                        2000
                    };
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(poll_interval))
                        .await;

                    // Drain all pending events
                    let mut had_events = false;
                    while let Ok(event) = script_rx.try_recv() {
                        had_events = true;
                        match event {
                            ScriptReloadEvent::FileChanged(path) | ScriptReloadEvent::FileCreated(path) => {
                                // Check if it's a scriptlet file or a skill definition file
                                let is_skill = is_skill_file(&path);
                                let is_scriptlet = path.extension().map(|e| e == "md").unwrap_or(false);

                                if is_skill {
                                    logging::log("APP", &format!("Skill file changed: {}", path.display()));
                                    cx.update(|cx| {
                                        app_entity_for_scripts.update(cx, |view, ctx| {
                                            view.refresh_skills(ctx);
                                        });
                                    });
                                } else if is_scriptlet {
                                    logging::log("APP", &format!("Scriptlet file changed: {}", path.display()));
                                    let path_clone = path.clone();
                                    cx.update(|cx| {
                                        app_entity_for_scripts.update(cx, |view, ctx| {
                                            view.handle_scriptlet_file_change(&path_clone, false, ctx);
                                        });
                                    });
                                } else {
                                    logging::log("APP", &format!("Script file changed: {}", path.display()));
                                    // Re-scan for scheduled scripts when script files change
                                    if let Ok(scheduler_guard) = scheduler_for_scripts.lock() {
                                        let new_count = scripts::register_scheduled_scripts(&scheduler_guard);
                                        if new_count > 0 {
                                            logging::log("APP", &format!("Re-registered {} scheduled scripts after file change", new_count));
                                        }
                                    }
                                    cx.update(|cx| {
                                        app_entity_for_scripts.update(cx, |view, ctx| {
                                            view.refresh_scripts(ctx);
                                        });
                                    });
                                }
                            }
                            ScriptReloadEvent::FileDeleted(path) => {
                                let is_scriptlet = path.extension().map(|e| e == "md").unwrap_or(false);
                                let is_skill = is_skill_file(&path);

                                if is_skill {
                                    logging::log("APP", &format!("Skill file deleted: {}", path.display()));
                                    cx.update(|cx| {
                                        app_entity_for_scripts.update(cx, |view, ctx| {
                                            view.refresh_skills(ctx);
                                        });
                                    });
                                } else if is_scriptlet {
                                    logging::log("APP", &format!("Scriptlet file deleted: {}", path.display()));
                                    let path_clone = path.clone();
                                    cx.update(|cx| {
                                        app_entity_for_scripts.update(cx, |view, ctx| {
                                            view.handle_scriptlet_file_change(&path_clone, true, ctx);
                                        });
                                    });
                                } else {
                                    logging::log("APP", &format!("Script file deleted: {}", path.display()));
                                    cx.update(|cx| {
                                        app_entity_for_scripts.update(cx, |view, ctx| {
                                            view.refresh_scripts(ctx);
                                        });
                                    });
                                }
                            }
                            ScriptReloadEvent::FullReload => {
                                logging::log("APP", "Full script/scriptlet reload requested");
                                // Re-scan for scheduled scripts
                                if let Ok(scheduler_guard) = scheduler_for_scripts.lock() {
                                    let new_count = scripts::register_scheduled_scripts(&scheduler_guard);
                                    if new_count > 0 {
                                        logging::log("APP", &format!("Re-registered {} scheduled scripts after full reload", new_count));
                                    }
                                }
                                cx.update(|cx| {
                                    app_entity_for_scripts.update(cx, |view, ctx| {
                                        view.refresh_scripts(ctx);
                                        view.refresh_skills(ctx);
                                    });
                                });
                            }
                        }
                    }

                    // Update idle count for adaptive polling
                    if had_events {
                        idle_count = 0;
                    } else {
                        idle_count = idle_count.saturating_add(1);
                    }
                }
            }).detach();
        }

        // App watcher poll loop - watches /Applications and ~/Applications for changes
        // When apps are added/removed/updated, refresh the app launcher cache
        // Only spawn if watcher started successfully
        if app_watcher_ok {
            let app_entity_for_apps = app_entity.clone();
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                use watcher::AppReloadEvent;

                logging::log("APP", "App watcher poll loop started (event-driven)");

                // Event-driven: blocks until app change event received
                while let Ok(event) = app_rx.recv().await {
                    logging::log("APP", &format!("App watcher event: {:?}", event));

                    match event {
                        AppReloadEvent::AppAdded(path) => {
                            logging::log("APP", &format!("App added: {}", path.display()));
                        }
                        AppReloadEvent::AppRemoved(path) => {
                            logging::log("APP", &format!("App removed: {}", path.display()));
                        }
                        AppReloadEvent::AppUpdated(path) => {
                            logging::log("APP", &format!("App updated: {}", path.display()));
                        }
                        AppReloadEvent::FullReload => {
                            logging::log("APP", "Full app reload requested");
                        }
                    }

                    // The app catalogue owner starts one fallible native scan
                    // off the UI thread and retains last-good rows on failure.
                    cx.update(|cx| {
                        app_entity_for_apps.update(cx, |view, ctx| {
                            view.refresh_apps(ctx);
                        });
                    });
                }

                logging::log("APP", "App watcher poll loop exiting (channel closed)");
            }).detach();
        }

        // NOTE: Prompt message listener is now spawned per-script in execute_interactive()
        // using event-driven async_channel instead of 50ms polling

        // Scheduler event handler - runs scripts when their cron schedule triggers
        // Uses std::sync::mpsc::Receiver which requires a polling approach
        let _window_for_scheduler = window;
        std::thread::spawn(move || {
            logging::log("APP", "Scheduler event handler started");

            loop {
                // Check shutdown flag - exit loop if shutting down
                if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
                    logging::log("SCHEDULER", "Shutdown requested, exiting scheduler event handler");
                    break;
                }

                // Use recv_timeout to periodically check for events without blocking forever
                match scheduler_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(event) => {
                        match event {
                            scheduler::SchedulerEvent::RunScript(path) => {
                                // Check shutdown flag before spawning new scripts
                                if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
                                    logging::log("SCHEDULER", &format!("Skipping scheduled script (shutdown in progress): {}", path.display()));
                                    continue;
                                }

                                logging::log("SCHEDULER", &format!("Executing scheduled script: {}", path.display()));

                                // Execute the script using the existing executor infrastructure
                                // This spawns it in the background without blocking the scheduler
                                let path_str = path.to_string_lossy().to_string();

                                if !scheduler::try_acquire_scheduled_run_slot() {
                                    logging::log(
                                        "SCHEDULER",
                                        &format!(
                                            "Skipping scheduled script (concurrency limit reached: active={}, limit={}): {}",
                                            scheduler::active_scheduled_run_count(),
                                            scheduler::SCHEDULED_SCRIPT_MAX_CONCURRENT_RUNS,
                                            path.display()
                                        ),
                                    );
                                    continue;
                                }

                                // Use bun to run the script directly (non-interactive for scheduled scripts)
                                // Find bun path (same logic as executor)
                                let bun_path = std::env::var("BUN_PATH")
                                    .ok()
                                    .or_else(|| {
                                        // Check common locations
                                        for candidate in &[
                                            "/opt/homebrew/bin/bun",
                                            "/usr/local/bin/bun",
                                            std::env::var("HOME").ok().map(|h| format!("{}/.bun/bin/bun", h)).unwrap_or_default().as_str(),
                                        ] {
                                            if std::path::Path::new(candidate).exists() {
                                                return Some(candidate.to_string());
                                            }
                                        }
                                        None
                                    })
                                    .unwrap_or_else(|| "bun".to_string());

                                // Spawn bun process to run the script
                                match std::process::Command::new(&bun_path)
                                    .arg("run")
                                    .arg("--preload")
                                    .arg(format!("{}/.scriptkit/sdk/kit-sdk.ts", std::env::var("HOME").unwrap_or_default()))
                                    .arg(&path_str)
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::piped())
                                    .spawn()
                                {
                                    Ok(mut child) => {
                                        let pid = child.id();
                                        // Track the process
                                        PROCESS_MANAGER.register_process(pid, &path_str);
                                        logging::log("SCHEDULER", &format!("Spawned scheduled script PID {}: {}", pid, path_str));

                                        // Wait for completion in a separate thread to not block scheduler
                                        let path_for_log = path_str.clone();
                                        std::thread::spawn(move || {
                                            let mut captured_stderr = String::new();
                                            let mut capture_was_truncated = false;

                                            if let Some(stderr) = child.stderr.take() {
                                                match scheduler::read_limited_stderr(
                                                    stderr,
                                                    scheduler::SCHEDULED_SCRIPT_STDERR_CAPTURE_MAX_BYTES,
                                                ) {
                                                    Ok((stderr_output, was_truncated)) => {
                                                        captured_stderr = stderr_output;
                                                        capture_was_truncated = was_truncated;
                                                    }
                                                    Err(e) => {
                                                        logging::log(
                                                            "SCHEDULER",
                                                            &format!(
                                                                "Failed to read scheduled script stderr stream: {} (pid={}): {}",
                                                                path_for_log, pid, e
                                                            ),
                                                        );
                                                    }
                                                }
                                            }

                                            let wait_result = child.wait();

                                            // Unregister and release slot now that execution has finished
                                            PROCESS_MANAGER.unregister_process(pid);
                                            scheduler::release_scheduled_run_slot();

                                            match wait_result {
                                                Ok(status) => {
                                                    if status.success() {
                                                        logging::log(
                                                            "SCHEDULER",
                                                            &format!(
                                                                "Scheduled script completed: {} (pid={})",
                                                                path_for_log, pid
                                                            ),
                                                        );
                                                    } else {
                                                        let (mut stderr_for_log, log_was_truncated) =
                                                            scheduler::truncate_scheduler_stderr_for_log(
                                                                &captured_stderr,
                                                                scheduler::SCHEDULED_SCRIPT_STDERR_LOG_MAX_BYTES,
                                                            );
                                                        if stderr_for_log.trim().is_empty() {
                                                            stderr_for_log = "<no stderr output>".to_string();
                                                        }
                                                        if capture_was_truncated {
                                                            stderr_for_log.push_str(
                                                                " [stderr capture truncated at 64KB]",
                                                            );
                                                        }
                                                        if log_was_truncated {
                                                            stderr_for_log.push_str(
                                                                " [stderr log truncated at 4KB]",
                                                            );
                                                        }
                                                        logging::log(
                                                            "SCHEDULER",
                                                            &format!(
                                                                "Scheduled script failed: {} (pid={}, status={}): {}",
                                                                path_for_log, pid, status, stderr_for_log
                                                            ),
                                                        );
                                                    }
                                                }
                                                Err(e) => {
                                                    logging::log(
                                                        "SCHEDULER",
                                                        &format!(
                                                            "Scheduled script wait error: {} (pid={}): {}",
                                                            path_for_log, pid, e
                                                        ),
                                                    );
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        scheduler::release_scheduled_run_slot();
                                        logging::log("SCHEDULER", &format!("Failed to spawn scheduled script: {} - {}", path_str, e));
                                    }
                                }
                            }
                            scheduler::SchedulerEvent::Error(msg) => {
                                logging::log("SCHEDULER", &format!("Scheduler error: {}", msg));
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Normal timeout, continue loop
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        logging::log("APP", "Scheduler event channel disconnected, exiting handler");
                        break;
                    }
                }
            }
        });

        // Test command file watcher - allows smoke tests to trigger script execution
        // SECURITY: This feature is ONLY enabled in debug builds to prevent local privilege escalation.
        // In release builds, any process that can write to /tmp could trigger script execution.
        #[cfg(debug_assertions)]
        {
            let app_entity_for_test = app_entity.clone();
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                logging::log("TEST", "Debug command file watcher enabled (debug build only)");
                let cmd_file = std::path::PathBuf::from("/tmp/script-kit-gpui-cmd.txt");
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;

                    if cmd_file.exists() {
                        if let Ok(content) = std::fs::read_to_string(&cmd_file) {
                            let _ = std::fs::remove_file(&cmd_file); // Remove immediately to prevent re-processing

                            for line in content.lines() {
                                if line.starts_with("run:") {
                                    let script_name = line.strip_prefix("run:").unwrap_or("").trim();
                                    logging::log("TEST", &format!("Test command: run script '{}'", script_name));

                                    let script_name_owned = script_name.to_string();
                                    let app_entity_inner = app_entity_for_test.clone();
                                    cx.update(|cx| {
                                        app_entity_inner.update(cx, |view, ctx| {
                                            // Find and run the script interactively
                                            if let Some(script) = view.scripts.iter().find(|s| s.name == script_name_owned || s.path.to_string_lossy().contains(&script_name_owned)).cloned() {
                                                logging::log("TEST", &format!("Found script: {}", script.name));
                                                view.execute_interactive(&script, ctx);
                                            } else {
                                                logging::log("TEST", &format!("Script not found: {}", script_name_owned));
                                            }
                                        });
                                    });
                                }
                            }
                        }
                    }
                }
            }).detach();
        }

// External command listener - receives commands via stdin (event-driven, no polling)
let stdin_response_sender = app_entity
    .read(cx)
    .response_sender
    .clone()
    .or_else(|| app_entity.read(cx).default_response_sender.clone());
let stdin_rx = start_stdin_listener(stdin_response_sender);
let window_for_stdin = window;
let app_entity_for_stdin = app_entity.clone();

enum RuntimeCommandIngress {
    Stdin(StdinCommandEnvelope),
    Mcp(crate::mcp_kit_tools::McpKitRuntimeCommand),
}

let (runtime_command_tx, runtime_command_rx) = async_channel::bounded(100);
let runtime_command_stdin_tx = runtime_command_tx.clone();
cx.spawn(async move |_cx: &mut gpui::AsyncApp| {
    while let Ok(envelope) = stdin_rx.recv().await {
        if runtime_command_stdin_tx
            .send(RuntimeCommandIngress::Stdin(envelope))
            .await
            .is_err()
        {
            break;
        }
    }
})
.detach();

let runtime_command_mcp_tx = runtime_command_tx.clone();
cx.spawn(async move |_cx: &mut gpui::AsyncApp| {
    while let Ok(command) = mcp_kit_runtime_rx.recv().await {
        if runtime_command_mcp_tx
            .send(RuntimeCommandIngress::Mcp(command))
            .await
            .is_err()
        {
            break;
        }
    }
})
.detach();
drop(runtime_command_tx);

// Track if we've received any stdin commands (for timeout warning)
static STDIN_RECEIVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy)]
enum DevtoolsSessionLifecycleAction {
    None,
    Touch {
        command_type: &'static str,
        reason: &'static str,
    },
    ExplicitClose {
        command_type: &'static str,
        reason: &'static str,
    },
}

fn devtools_keep_actions_window_open_enabled() -> bool {
    std::env::var("SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN").ok().as_deref() == Some("1")
}

fn devtools_lifecycle_action_for_stdin(cmd: &StdinCommand) -> DevtoolsSessionLifecycleAction {
    let command_type = cmd.command_type();
    match cmd {
        StdinCommand::External(ExternalCommand::Hide { .. }) => {
            DevtoolsSessionLifecycleAction::ExplicitClose {
                command_type,
                reason: "explicit_hide",
            }
        }
        _ if devtools_keep_actions_window_open_enabled() => DevtoolsSessionLifecycleAction::Touch {
            command_type,
            reason: "stdin_devtools_activity",
        },
        _ => DevtoolsSessionLifecycleAction::None,
    }
}

fn apply_devtools_lifecycle_action(action: DevtoolsSessionLifecycleAction) {
    match action {
        DevtoolsSessionLifecycleAction::None => {}
        DevtoolsSessionLifecycleAction::Touch {
            command_type,
            reason,
        } => {
            script_kit_gpui::mark_window_shown();
            tracing::info!(
                event = "devtools_session_activity",
                keep_actions_window_open = true,
                command_type,
                reason
            );
        }
        DevtoolsSessionLifecycleAction::ExplicitClose {
            command_type,
            reason,
        } => {
            tracing::info!(
                event = "devtools_session_explicit_close",
                keep_actions_window_open = devtools_keep_actions_window_open_enabled(),
                command_type,
                reason
            );
        }
    }
}

// Spawn a timeout warning task - helps AI agents detect when they forgot to use stdin protocol
cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    cx.background_executor()
        .timer(std::time::Duration::from_secs(2))
        .await;
    if !STDIN_RECEIVED.load(std::sync::atomic::Ordering::SeqCst) {
        logging::log("STDIN", "");
        logging::log(
            "STDIN",
            "╔════════════════════════════════════════════════════════════════════════════╗",
        );
        logging::log(
            "STDIN",
            "║  WARNING: No stdin JSON received after 2 seconds                          ║",
        );
        logging::log(
            "STDIN",
            "║                                                                            ║",
        );
        logging::log(
            "STDIN",
            "║  If you're testing, use the stdin JSON protocol:                          ║",
        );
        logging::log(
            "STDIN",
            "║  echo '{\"type\":\"run\",\"path\":\"...\"}' | ./target/debug/script-kit-gpui     ║",
        );
        logging::log(
            "STDIN",
            "║                                                                            ║",
        );
        logging::log(
            "STDIN",
            "║  Command line args do NOT work:                                           ║",
        );
        logging::log(
            "STDIN",
            "║  ./target/debug/script-kit-gpui test.ts  # WRONG - does nothing!          ║",
        );
        logging::log(
            "STDIN",
            "╚════════════════════════════════════════════════════════════════════════════╝",
        );
        logging::log("STDIN", "");
    }
})
.detach();

cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("STDIN", "Async stdin command handler started");

    // Event-driven: recv().await yields until a command arrives
    while let Ok(ingress) = runtime_command_rx.recv().await
    {
        let (cmd, correlation_id, mcp_response_tx) = match ingress {
            RuntimeCommandIngress::Stdin(StdinCommandEnvelope {
                command,
                correlation_id,
            }) => (command, correlation_id, None),
            RuntimeCommandIngress::Mcp(command) => (
                StdinCommand::External(command.command),
                command.correlation_id,
                Some(command.response_tx),
            ),
        };
        let command_type = cmd.command_type();
        let request_id = cmd.request_id().map(ToString::to_string);
        let _guard = logging::set_correlation_id(correlation_id);
        // Mark that we've received stdin (clears the timeout warning)
        STDIN_RECEIVED.store(true, std::sync::atomic::Ordering::SeqCst);
        logging::log(
            "STDIN",
            &format!("Processing external command type={}", command_type),
        );

        let lifecycle_action = devtools_lifecycle_action_for_stdin(&cmd);
        let app_entity_inner = app_entity_for_stdin.clone();
        cx.update(|cx| {
            apply_devtools_lifecycle_action(lifecycle_action);
            // Use the Root window to get Window reference, then update the app entity
            let _ = window_for_stdin.update(cx, |_root, window, root_cx| {
                app_entity_inner.update(root_cx, |view, ctx| {
                    // Note: We have both `window` from Root and `view` from entity here
                    // ctx is Context<ScriptListApp>, window is &mut Window
                    match cmd {
                        StdinCommand::External(cmd) => match cmd {
                            ExternalCommand::Run { ref path, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Executing script: {}", rid, path));

                                // NOTE: This is a simplified show path for script execution.
                                // We show the window, then immediately run the script.
                                // The core logic matches show_main_window_helper().

                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown(); // Focus grace period
                                platform::ensure_move_to_active_space();

                                // Use Window::defer via window_ops to coalesce and defer window move.
                                // This avoids RefCell borrow conflicts from synchronous macOS window operations.
                                let window_size = crate::window_resize::initial_window_size();
                                let bounds = platform::calculate_eye_line_bounds_on_mouse_display(window_size);
                                window_ops::queue_move(bounds, window, ctx);

                                // Oracle-Session `window-activation-invariants-guard` PR1 —
                                // replaces the duplicated PANEL_CONFIGURED block; only
                                // stores `true` after a successful post-configure invariant
                                // report.
                                platform::ensure_main_panel_configured("app_run_setup::stdin_run");

                                // Show window WITHOUT activating (floating panel behavior)
                                platform::show_main_window_without_activation();
                                window.activate_window();

                                // Send AI window to back so it doesn't come forward with main menu
                                platform::send_ai_window_to_back();

                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                                sync_main_automation_window(None, true, true);

                                // Send RunScript message to be handled
                                view.handle_prompt_message(PromptMessage::RunScript { path: path.clone() }, ctx);
                            }
                            ExternalCommand::Show { ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Showing window", rid));

                                // NOTE: This is a simplified show path for explicit stdin commands.
                                // Unlike the hotkey handler, we don't need NEEDS_RESET handling
                                // because this is an explicit show (not a toggle).
                                // The core logic matches show_main_window_helper().

                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown(); // Focus grace period
                                platform::ensure_move_to_active_space();

                                view.ensure_selection_at_first_item(ctx);

                                // Compute dynamic window size matching the hotkey path.
                                let current_bounds = platform::get_main_window_bounds();
                                let current_window_width = current_bounds.map(|(_, _, width, _)| width as f32);
                                let window_size = if matches!(view.current_view, AppView::ScriptList)
                                    && view.main_window_mode == MainWindowMode::Mini
                                {
                                    let (grouped_items, _) = view.get_grouped_results_cached();
                                    let sizing = crate::window_resize::main_window_sizing_from_grouped_items(&grouped_items);
                                    gpui::size(
                                        px(crate::window_resize::width_for_view(ViewType::MainWindow).unwrap_or(crate::window_resize::MAIN_WINDOW_WIDTH)),
                                        crate::window_resize::height_for_main_window(sizing),
                                    )
                                } else if let Some((view_type, item_count)) = view.calculate_window_size_params_with_app(Some(&*ctx)) {
                                    gpui::size(
                                        px(crate::window_resize::width_for_view(view_type)
                                            .or(current_window_width)
                                            .unwrap_or(crate::window_resize::MAIN_WINDOW_WIDTH)),
                                        crate::window_resize::height_for_view(view_type, item_count),
                                    )
                                } else {
                                    gpui::size(
                                        px(current_window_width.unwrap_or(crate::window_resize::MAIN_WINDOW_WIDTH)),
                                        crate::window_resize::height_for_view(ViewType::ScriptList, 0),
                                    )
                                };
                                let displays = platform::get_macos_displays();
                                let bounds = if let Some((mouse_x, mouse_y)) = platform::get_global_mouse_position() {
                                    // Try to restore saved position for the mouse display
                                    if let Some((saved, display)) =
                                        window_state::get_main_position_for_mouse_display(mouse_x, mouse_y, &displays)
                                    {
                                        // Validate the saved position is still visible
                                        if window_state::is_bounds_visible(&saved, &displays) {
                                            logging::log(
                                                "STDIN",
                                                &format!(
                                                    "Restoring saved position for display {}: ({:.0}, {:.0})",
                                                    window_state::display_key(&display),
                                                    saved.x,
                                                    saved.y
                                                ),
                                            );
                                            // Use saved position but with current window height (may have changed)
                                            gpui::Bounds {
                                                origin: gpui::point(px(saved.x as f32), px(saved.y as f32)),
                                                size: window_size,
                                            }
                                        } else {
                                            logging::log("STDIN", "Saved position no longer visible, using eye-line");
                                            platform::calculate_eye_line_bounds_on_mouse_display(window_size)
                                        }
                                    } else {
                                        logging::log("STDIN", "No saved position for this display, using eye-line");
                                        platform::calculate_eye_line_bounds_on_mouse_display(window_size)
                                    }
                                } else {
                                    logging::log("STDIN", "Could not get mouse position, using eye-line");
                                    platform::calculate_eye_line_bounds_on_mouse_display(window_size)
                                };
                                window_ops::queue_move(bounds, window, ctx);

                                // Oracle-Session `window-activation-invariants-guard` PR1 —
                                // replaces the duplicated PANEL_CONFIGURED block; only
                                // stores `true` after a successful post-configure invariant
                                // report.
                                platform::ensure_main_panel_configured("app_run_setup::stdin_show");

                                // Show window WITHOUT activating (floating panel behavior)
                                platform::show_main_window_without_activation();
                                window.activate_window();

                                // Send AI window to back so it doesn't come forward with main menu
                                platform::send_ai_window_to_back();

                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                                sync_main_automation_window(None, true, true);

                                // Ensure render-loop focus state is set so the input autofocuses
                                view.focused_input = FocusedInput::MainFilter;
                                view.pending_focus = Some(FocusTarget::MainFilter);

                                // Passive AX-only selection sniff for the "Rewrite
                                // selection" hint chip — mirrors show_main_window_helper.
                                view.refresh_shown_selection_hint(ctx);

                                // Run-14 Pass-13 fix: echo a windowVisibilityAck
                                // back so `session.sh rpc … {"type":"show",
                                // "requestId":"x"}` no longer hits the 5-second
                                // timeout. Closes
                                // `tool-window-mutator-rpcs-never-echo-response`
                                // (Pass-12 finding).
                                if let Some(ref rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::window_visibility_ack(
                                                rid.to_string(),
                                                true,
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::Hide { ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Hiding main window", rid));

                                // Save window position for the current display BEFORE hiding
                                if let Some((x, y, width, height)) = platform::get_main_window_bounds() {
                                    let displays = platform::get_macos_displays();
                                    let bounds = window_state::PersistedWindowBounds::new(x, y, width, height);
                                    if let Some(display) = window_state::find_display_for_bounds(&bounds, &displays) {
                                        logging::log(
                                            "STDIN",
                                            &format!(
                                                "Saving position for display {}: ({:.0}, {:.0})",
                                                window_state::display_key(display),
                                                x,
                                                y
                                            ),
                                        );
                                        window_state::save_main_position_for_display(display, bounds);
                                    }
                                }

                                script_kit_gpui::set_main_window_visible(false);
                                sync_main_automation_window(current_main_automation_bounds(), false, false);

                                // Reset the hidden view back to ScriptList after the
                                // native hide has been enqueued. This preserves the
                                // Pass #19 automation re-key fix without rendering a
                                // visible ScriptList frame while AppKit is still
                                // closing the panel.
                                // Full teardown for actions-dialog
                                // (`id: "actions-dialog"`). Pass #29 fix
                                // (`cmd-k-on-unfocused-clipboard-pops-overlay-not-actions`):
                                // upgraded from bare `remove_automation_window` to full
                                // `close_actions_window`. Pass #23's bare registry op
                                // left the `ACTIONS_WINDOW` static holding a stale handle;
                                // a later `simulateKey cmd+k` on an unfocused window read
                                // `is_actions_window_open()=true` and took the CLOSE branch,
                                // popping whichever overlay was on top instead of opening
                                // the actions dialog. `close_actions_window` clears the
                                // static AND the registry; idempotent.
                                crate::actions::close_actions_window(ctx);
                                // Sibling teardown for confirm-popup
                                // (`id: "confirm-popup"`, PromptPopup kind).
                                // Pass #25 fix: close_confirm_window at
                                // src/confirm/window.rs:385 is the only
                                // production removal path; no hide dispatcher
                                // calls it (`attacker-hide-path-confirm-popup-registry-stale`).
                                // Pure registry op; idempotent.
                                crate::windows::remove_automation_window("confirm-popup");

                                // Check if Notes is open for logging only.
                                let notes_open = notes::is_notes_window_open();

                                // CRITICAL: Always hide only the main panel. `ctx.hide()`
                                // app-hides all windows, so a stale/false-negative Notes
                                // handle can hide Notes together with main.
                                logging::log(
                                    "STDIN",
                                    &format!(
                                        "Using defer_hide_main_window() - main-only hide, secondary_windows_open={}",
                                        notes_open
                                    ),
                                );
                                let visibility_generation =
                                    script_kit_gpui::main_window_visibility_generation();
                                view.defer_calibrated_main_window_hide(
                                    ctx,
                                    visibility_generation,
                                    None,
                                    MainWindowPostHide::ResetScriptList {
                                        reason: "stdin_hide_rpc",
                                        reset_mini_bounds_after_hidden_reset: false,
                                    },
                                );

                                // Run-14 Pass-13 fix: echo a windowVisibilityAck
                                // back so `session.sh rpc … {"type":"hide",
                                // "requestId":"x"}` no longer hits the 5-second
                                // timeout. Closes
                                // `tool-window-mutator-rpcs-never-echo-response`
                                // (Pass-12 finding).
                                if let Some(ref rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::window_visibility_ack(
                                                rid.to_string(),
                                                false,
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetFilter { ref text, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Setting filter to: '{}'", rid, text));
                                view.menu_syntax_form_input_active = false;
                                view.menu_syntax_form_draft_field_id = None;
                                view.menu_syntax_form_draft_value.clear();
                                view.set_filter_text_immediate(text.clone(), window, ctx);
                                let _ = view.get_filtered_results_cached(); // Update cache
                                ctx.notify();
                            }
                            ExternalCommand::SetMenuSyntaxFormField { ref field, ref value, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log(
                                    "STDIN",
                                    &format!(
                                        "[{}] Setting menu-syntax form field {:?} to: '{}'",
                                        rid, field, value
                                    ),
                                );
                                let _ = view.update_menu_syntax_form_field(
                                    field.as_deref(),
                                    value.clone(),
                                    window,
                                    ctx,
                                );
                                let _ = view.get_filtered_results_cached();
                                ctx.notify();
                            }
                            ref cmd @ ExternalCommand::TriggerBuiltin { .. } => {
                                // All payload normalization (`builtinId` vs
                                // deprecated `name`), registry lookup,
                                // exhaustive dispatch, and rate-limited
                                // unknown / deprecated / invalid logging
                                // live in the shared helper — see
                                // src/app_impl/trigger_builtin_dispatch.rs.
                                logging::log("STDIN", "Triggering built-in (see structured logs)");
                                let _ = view.dispatch_trigger_builtin(cmd, window, ctx);
                                let _ = view
                                    .rekey_main_automation_surface_after_trigger_builtin_dispatch();
                            }

                            ExternalCommand::SimulateKey { ref key, ref modifiers, ref target, ref request_id } => {
                                // SimulateKey: Enter - accept menu-syntax picker
                                // SimulateKey: Enter - execute selected
                                let simulate_key_response = request_id
                                    .as_ref()
                                    .and_then(|rid| {
                                        view.response_sender
                                            .clone()
                                            .map(|sender| (rid.to_string(), sender))
                                    });
                                let outcome = view.dispatch_simulate_key(
                                    window,
                                    ctx,
                                    crate::simulate_key_dispatch::SimulatedKeyInput {
                                        key,
                                        modifiers,
                                        target: target.as_ref(),
                                    },
                                );
                                if let Some((rid, sender)) = simulate_key_response {
                                    let _ = sender.try_send(
                                        crate::protocol::Message::external_command_result(
                                            rid,
                                            "simulateKey".to_string(),
                                            outcome.status == crate::action_helpers::ActionOutcomeStatus::Success,
                                            outcome.error_code.map(str::to_string),
                                            outcome.user_message.or(outcome.detail),
                                        ),
                                    );
                                }
                            }

                            ExternalCommand::SimulateMainHotkeyGesture { ref phase, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log(
                                    "STDIN",
                                    &format!("[{}] SimulateMainHotkeyGesture phase={}", rid, phase),
                                );
                                let normalized = phase.trim().to_ascii_lowercase();
                                if let Some(hotkey_phase) = match normalized.as_str() {
                                    "down" | "keydown" | "key-down" => {
                                        Some(hotkeys::MainHotkeyPhase::KeyDown)
                                    }
                                    "up" | "keyup" | "key-up" => {
                                        Some(hotkeys::MainHotkeyPhase::KeyUp)
                                    }
                                    _ => None,
                                } {
                                    hotkeys::inject_main_hotkey_phase_for_agentic(hotkey_phase);
                                    if let Some(rid) = request_id {
                                        if let Some(sender) = view.response_sender.clone() {
                                            let _ = sender.try_send(
                                                crate::protocol::Message::external_command_result(
                                                    rid.to_string(),
                                                    "simulateMainHotkeyGesture".to_string(),
                                                    true,
                                                    None,
                                                    None,
                                                ),
                                            );
                                        }
                                    }
                                } else if let Some(rid) = request_id {
                                    if let Some(sender) = view.response_sender.clone() {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "simulateMainHotkeyGesture".to_string(),
                                                false,
                                                Some("invalid_phase".to_string()),
                                                Some(format!(
                                                    "expected 'down' or 'up', got '{}'",
                                                    normalized
                                                )),
                                            ),
                                        );
                                    }
                                }
                            }

                            ExternalCommand::TriggerAction {
                                action_id,
                                host,
                                request_id,
                            } => {
                                // Fire an action by ID without driving the actions-dialog
                                // popup through Cmd+K / arrow-nav / Enter. AFK harnesses and
                                // other agentic-testing clients discover valid ids via
                                // `getElements` on the actions-dialog window, then fire via
                                // this command to avoid flaky key simulation. When the detached
                                // Actions dialog is live, route through its typed availability
                                // guard instead of closing it and calling the host directly.
                                let native_footer_activation =
                                    (host.as_deref() == Some("nativeFooter")).then(|| {
                                        crate::footer_popup::activate_native_main_footer_button(
                                            window,
                                            &action_id,
                                        )
                                    });
                                let detached_window_open = crate::actions::is_actions_window_open();
                                let actions_surface_open = native_footer_activation.is_none()
                                    && (detached_window_open || view.show_actions_popup);
                                let detached_activation = if native_footer_activation.is_none()
                                    && detached_window_open
                                {
                                    crate::actions::activate_detached_actions_window_action(
                                        action_id.clone(),
                                        ctx,
                                    )
                                } else {
                                    None
                                };
                                let resolved_host = if native_footer_activation.is_none()
                                    && !actions_surface_open
                                {
                                    match host.as_deref() {
                                        Some("argPrompt") => Some(ActionsDialogHost::ArgPrompt),
                                        Some("divPrompt") => Some(ActionsDialogHost::DivPrompt),
                                        Some("editorPrompt") => {
                                            Some(ActionsDialogHost::EditorPrompt)
                                        }
                                        Some("templatePrompt") => {
                                            Some(ActionsDialogHost::TemplatePrompt)
                                        }
                                        Some("termPrompt") => Some(ActionsDialogHost::TermPrompt),
                                        Some("formPrompt") => Some(ActionsDialogHost::FormPrompt),
                                        Some("chatPrompt") => Some(ActionsDialogHost::ChatPrompt),
                                        Some("mainList") => Some(ActionsDialogHost::MainList),
                                        Some("fileSearch") => Some(ActionsDialogHost::FileSearch),
                                        Some("clipboardHistory") => {
                                            Some(ActionsDialogHost::ClipboardHistory)
                                        }
                                        Some("dictationHistory") => {
                                            Some(ActionsDialogHost::DictationHistory)
                                        }
                                        Some("emojiPicker") => {
                                            Some(ActionsDialogHost::EmojiPicker)
                                        }
                                        Some("appLauncher") => {
                                            Some(ActionsDialogHost::AppLauncher)
                                        }
                                        Some("builtinList") => {
                                            Some(ActionsDialogHost::BuiltinList)
                                        }
                                        Some("webcamPrompt") => {
                                            Some(ActionsDialogHost::WebcamPrompt)
                                        }
                                        Some("agentChatChat") => {
                                            Some(ActionsDialogHost::AgentChat)
                                        }
                                        Some("agent_chatHistory") => {
                                            Some(ActionsDialogHost::AgentChatHistory)
                                        }
                                        Some("agentChatDetached") => {
                                            Some(ActionsDialogHost::AgentChatDetached)
                                        }
                                        Some(other) => {
                                            logging::log(
                                                "STDIN",
                                                &format!(
                                                    "TriggerAction: unknown host '{}'; falling back to current view host",
                                                    other
                                                ),
                                            );
                                            view.current_actions_host()
                                        }
                                        None => view.current_actions_host(),
                                    }
                                } else {
                                    None
                                };

                                let mut receipt_host = None;
                                let mut receipt_ok = false;
                                let mut receipt_error_code = None;
                                let mut popup_closed = false;

                                if let Some(activation) = native_footer_activation.as_ref() {
                                    receipt_host = Some("NativeFooter".to_string());
                                    receipt_ok = activation.dispatched;
                                    receipt_error_code = activation.error_code.clone();
                                } else if actions_surface_open {
                                    receipt_host = Some("ActionsDialog".to_string());
                                    match detached_activation {
                                        Some(crate::actions::ActionsDialogActivation::Executed {
                                            should_close, ..
                                        }) => {
                                            receipt_ok = true;
                                            popup_closed = should_close;
                                        }
                                        Some(
                                            crate::actions::ActionsDialogActivation::DrillDownPushed {
                                                ..
                                            },
                                        ) => {
                                            receipt_ok = true;
                                        }
                                        Some(crate::actions::ActionsDialogActivation::Blocked {
                                            ..
                                        }) => {
                                            receipt_error_code = Some("action_disabled".to_string());
                                        }
                                        Some(crate::actions::ActionsDialogActivation::NoSelection) => {
                                            receipt_error_code = Some("action_not_found".to_string());
                                        }
                                        None => {
                                            // Fail closed if the host still reports an Actions
                                            // surface but no detached dialog handle can activate it.
                                            // Falling through would bypass the availability guard.
                                            receipt_error_code =
                                                Some("actions_dialog_unavailable".to_string());
                                        }
                                    }
                                } else {
                                    match resolved_host {
                                        Some(host_value) => {
                                            receipt_host = Some(format!("{host_value:?}"));
                                            receipt_ok = true;
                                            logging::log(
                                                "STDIN",
                                                &format!(
                                                    "TriggerAction: host={:?} action_id='{}' popup_open={}",
                                                    host_value,
                                                    action_id,
                                                    view.show_actions_popup
                                                ),
                                            );
                                            if view.show_actions_popup {
                                                view.close_actions_popup(host_value, window, ctx);
                                                popup_closed = true;
                                            }
                                            view.execute_action_for_actions_host(
                                                host_value,
                                                action_id.clone(),
                                                window,
                                                ctx,
                                            );
                                        }
                                        None => {
                                            receipt_error_code = Some("no_host".to_string());
                                            logging::log(
                                                "STDIN",
                                                "TriggerAction: no host supplied and current view has no shared-actions host; skipping",
                                            );
                                        }
                                    }
                                }
                                if let Some(ref rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::trigger_action_result(
                                                rid.to_string(),
                                                action_id.to_string(),
                                                receipt_host,
                                                receipt_ok,
                                                popup_closed,
                                                receipt_error_code,
                                                native_footer_activation.as_ref().and_then(|receipt| {
                                                    serde_json::to_value(receipt).ok()
                                                }),
                                            ),
                                        );
                                    }
                                }
                            }

                            ExternalCommand::InjectClipboardCaptureFixture {
                                payload,
                                source_bundle_id,
                                concealed_types,
                                change_generation,
                                request_id,
                            } => {
                                let outcome = crate::clipboard_history::inject_clipboard_capture_fixture(
                                    payload,
                                    source_bundle_id.as_deref(),
                                    &concealed_types,
                                    change_generation,
                                );
                                let (ok, outcome_code, entry_id, error_code, error_message) =
                                    match outcome {
                                        crate::clipboard_history::ClipboardCaptureFixtureOutcome::Stored { entry_id } => {
                                            (true, "stored", Some(entry_id), None, None)
                                        }
                                        crate::clipboard_history::ClipboardCaptureFixtureOutcome::Rejected { reason } => {
                                            (true, "rejected", None, Some(format!("{reason:?}")), None)
                                        }
                                        crate::clipboard_history::ClipboardCaptureFixtureOutcome::Oversized { text_len, max_len } => {
                                            tracing::info!(
                                                category = "STDIN",
                                                text_len,
                                                max_len,
                                                change_generation,
                                                "clipboard_capture_fixture_oversized"
                                            );
                                            (true, "oversized", None, None, None)
                                        }
                                        crate::clipboard_history::ClipboardCaptureFixtureOutcome::Empty => {
                                            (true, "empty", None, None, None)
                                        }
                                        crate::clipboard_history::ClipboardCaptureFixtureOutcome::UnchangedGeneration => {
                                            (true, "unchanged_generation", None, None, None)
                                        }
                                        crate::clipboard_history::ClipboardCaptureFixtureOutcome::Failed { error } => {
                                            (false, "failed", None, Some("capture_failed".to_string()), Some(error))
                                        }
                                    };
                                tracing::info!(
                                    category = "STDIN",
                                    event = "clipboard_capture_fixture_result",
                                    change_generation,
                                    concealed_type_count = concealed_types.len(),
                                    source_bundle_id_present = source_bundle_id.is_some(),
                                    outcome = outcome_code,
                                    entry_id = ?entry_id,
                                    error_code = ?error_code,
                                    "clipboard capture fixture processed without NSPasteboard access"
                                );
                                if let (Some(request_id), Some(sender)) =
                                    (request_id.as_ref(), view.response_sender.as_ref())
                                {
                                    let _ = sender.try_send(
                                        crate::protocol::Message::external_command_result(
                                            request_id.to_string(),
                                            "injectClipboardCaptureFixture".to_string(),
                                            ok,
                                            error_code,
                                            error_message,
                                        ),
                                    );
                                }
                            }

                            ExternalCommand::OpenNotes => {
                                logging::log("STDIN", "Opening notes window via stdin command");
                                if let Err(e) = notes::open_notes_window(ctx) {
                                    logging::log("STDIN", &format!("Failed to open notes window: {}", e));
                                }
                            }
                            ExternalCommand::OpenAbout => {
                                logging::log("STDIN", "Opening About surface via stdin command");
                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown();
                                platform::show_main_window_without_activation();
                                window.activate_window();
                                sync_main_automation_window(current_main_automation_bounds(), true, true);
                                view.open_about_surface(
                                    std::sync::Arc::new(std::sync::RwLock::new(
                                        crate::updates::UpdateState::Idle,
                                    )),
                                    ctx,
                                );
                            }
                            ExternalCommand::OpenCreationFeedback { path, receipt_path, receipt_status, verification_status, request_id: _ } => {
                                logging::log("STDIN", "Opening CreationFeedback surface via stdin command");
                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown();
                                platform::show_main_window_without_activation();
                                window.activate_window();
                                sync_main_automation_window(current_main_automation_bounds(), true, true);
                                let artifact_path = path
                                    .map(std::path::PathBuf::from)
                                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp/script-kit-liquid-glass-feedback-fixture.ts"));
                                let payload = crate::prompts::CreationFeedbackPayload::fixture(
                                    artifact_path,
                                    receipt_path.map(std::path::PathBuf::from),
                                    receipt_status
                                        .as_deref()
                                        .map(crate::prompts::CreationFeedbackReceiptStatus::from_fixture_str),
                                    verification_status,
                                );
                                view.open_creation_feedback_payload(payload, ctx);
                            }
                            ExternalCommand::OpenConfirmPrompt { title, body, confirm_text, cancel_text, request_id: _ } => {
                                logging::log("STDIN", "Opening ConfirmPrompt surface via stdin command");
                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown();
                                platform::show_main_window_without_activation();
                                window.activate_window();
                                window_ops::queue_move(
                                    gpui::Bounds {
                                        origin: gpui::point(gpui::px(585.), gpui::px(177.)),
                                        size: gpui::size(
                                            gpui::px(750.),
                                            crate::window_resize::height_for_view(
                                                crate::window_resize::ViewType::MainWindow,
                                                0,
                                            ),
                                        ),
                                    },
                                    window,
                                    ctx,
                                );
                                sync_main_automation_window(current_main_automation_bounds(), true, true);
                                let (sender, _receiver) = async_channel::bounded(1);
                                let options = crate::confirm::ParentConfirmOptions {
                                    title: title.unwrap_or_else(|| "Delete saved item?".to_string()).into(),
                                    body: body.unwrap_or_else(|| "This action changes local Script Kit state. Confirm to continue or cancel to return to the launcher.".to_string()).into(),
                                    confirm_text: confirm_text.unwrap_or_else(|| "Delete".to_string()).into(),
                                    cancel_text: cancel_text.unwrap_or_else(|| "Cancel".to_string()).into(),
                                    confirm_variant: gpui_component::button::ButtonVariant::Danger,
                                    width: gpui::px(448.),
                                };
                                view.open_confirm_prompt(options, sender, ctx);
                            }
                            ExternalCommand::OpenAi => {
                                logging::log("STDIN", "Opening Agent Chat via openAi compatibility alias");
                                view.open_tab_ai_agent_chat_with_entry_intent(None, ctx);
                            }
                            ExternalCommand::OpenAgentChatDetachedFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                let result = view.open_detached_agent_chat_mock_fixture(ctx);
                                tracing::info!(
                                    category = "STDIN",
                                    event = "agent_chat_detached_fixture_opened",
                                    command = "openAgentChatDetachedFixture",
                                    request_id = ?rid,
                                    ok = result.as_ref().map(|moved| *moved).unwrap_or(false),
                                    error = result.err().map(|err| err.to_string()),
                                    "Detached Agent Chat fixture open result"
                                );
                            }
                            ExternalCommand::OpenAgentChatHistoryPopupFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                let opened = crate::ai::agent_chat::ui::chat_window::open_agent_chat_history_popup_fixture(ctx);
                                tracing::info!(
                                    category = "STDIN",
                                    event = "agent_chat_history_popup_fixture_opened",
                                    command = "openAgentChatHistoryPopupFixture",
                                    request_id = ?rid,
                                    opened,
                                    "Detached Agent Chat history popup fixture result"
                                );
                            }
                            ExternalCommand::OpenChatPromptFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                view.handle_prompt_message(
                                    PromptMessage::ShowChat {
                                        id: "fixture:ordinary-chat-prompt".to_string(),
                                        placeholder: Some("Message".to_string()),
                                        messages: vec![crate::protocol::ChatPromptMessage::assistant(
                                            "Fixture response",
                                        )],
                                        hint: Some("Ordinary Script ChatPrompt".to_string()),
                                        footer: None,
                                        actions: None,
                                        model: None,
                                        models: Vec::new(),
                                        save_history: false,
                                        use_builtin_ai: false,
                                    },
                                    ctx,
                                );
                                tracing::info!(
                                    category = "STDIN",
                                    event = "chat_prompt_fixture_opened",
                                    command = "openChatPromptFixture",
                                    request_id = ?rid,
                                );
                            }
                            ExternalCommand::ClosePromptPopupNatively { ref target, ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                match crate::components::inline_popup_window::close_prompt_popup_target_natively(target, ctx) {
                                    Ok((id, generation, native_window_number)) => tracing::info!(
                                        category = "STDIN",
                                        event = "prompt_popup_native_close_requested",
                                        command = "closePromptPopupNatively",
                                        request_id = ?rid,
                                        window_id = %id,
                                        generation,
                                        native_window_number,
                                    ),
                                    Err(error) => tracing::warn!(
                                        category = "STDIN",
                                        event = "prompt_popup_native_close_refused",
                                        command = "closePromptPopupNatively",
                                        request_id = ?rid,
                                        error = %error,
                                    ),
                                }
                            }
                            ExternalCommand::OpenMiniAi => {
                                logging::log("STDIN", "Opening Agent Chat via openMiniAi compatibility alias");
                                view.open_tab_ai_agent_chat_with_entry_intent(None, ctx);
                            }
                            ExternalCommand::OpenAiWithMockData => {
                                logging::log("STDIN", "Opening standard Agent Chat mock fixture");
                                view.open_standard_agent_chat_mock_fixture(ctx);
                            }
                            ExternalCommand::OpenMiniAiWithMockData => {
                                logging::log(
                                    "STDIN",
                                    "Ignoring deprecated mini mock-data AI alias and opening Agent Chat",
                                );
                                view.open_tab_ai_agent_chat_with_entry_intent(None, ctx);
                            }
                            ExternalCommand::OpenFocusedTextAgentChatWithMockData { text, instruction, request_id } => {
                                logging::log("STDIN", "Opening focused-text Agent Chat mock fixture");
                                let text_length = text.as_ref().map(|value| value.len()).unwrap_or("Hello world".len());
                                let instruction_length = instruction
                                    .as_ref()
                                    .map(|value| value.trim().len())
                                    .unwrap_or(0);
                                let requested_submit = instruction_length > 0;
                                let result = view.open_focused_text_agent_chat_fixture(
                                    text,
                                    instruction,
                                    "focused_text_mock_fixture",
                                    ctx,
                                );
                                let ok = result.is_ok();
                                if let Err(error) = result {
                                    logging::log(
                                        "STDIN",
                                        &format!("Failed to open focused-text Agent Chat mock fixture: {error}"),
                                    );
                                }
                                if let Some(rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::focused_text_agent_chat_fixture_open_result(
                                                rid.to_string(),
                                                "mock".to_string(),
                                                ok,
                                                ok && requested_submit,
                                                text_length,
                                                instruction_length,
                                                if ok { None } else { Some("open_failed".to_string()) },
                                                if ok {
                                                    None
                                                } else {
                                                    Some("Focused-text Agent Chat mock fixture open failed".to_string())
                                                },
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::OpenFocusedTextAgentChatFromFocusedFieldWithMockData { instruction, request_id } => {
                                logging::log("STDIN", "Opening focused-text Agent Chat live mock fixture");
                                let instruction_length = instruction
                                    .as_ref()
                                    .map(|value| value.trim().len())
                                    .unwrap_or(0);
                                let requested_submit = instruction_length > 0;
                                let result = view.open_focused_text_agent_chat_from_focused_field_mock_fixture(
                                    instruction,
                                    ctx,
                                );
                                let (ok, text_length, error_code, error_message) = match result {
                                    Ok(text_length) => (true, text_length, None, None),
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to open focused-text Agent Chat live mock fixture: {error}"),
                                        );
                                        let error_code = if error.contains("SCRIPT_KIT_FOCUSED_TEXT_LIVE_FIXTURE") {
                                            "gated_off"
                                        } else {
                                            "open_failed"
                                        };
                                        (
                                            false,
                                            0,
                                            Some(error_code.to_string()),
                                            Some("Focused-text Agent Chat live mock fixture open failed".to_string()),
                                        )
                                    }
                                };
                                if let Some(rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::focused_text_agent_chat_fixture_open_result(
                                                rid.to_string(),
                                                "live-mock".to_string(),
                                                ok,
                                                ok && requested_submit,
                                                text_length,
                                                instruction_length,
                                                error_code,
                                                error_message,
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::OpenFocusedTextAgentChatWithPiData { text, instruction, request_id } => {
                                logging::log("STDIN", "Opening focused-text Agent Chat real Pi fixture");
                                let text_length = text.as_ref().map(|value| value.len()).unwrap_or("Hello world".len());
                                let instruction_length = instruction
                                    .as_ref()
                                    .map(|value| value.trim().len())
                                    .unwrap_or(0);
                                let requested_submit = instruction_length > 0;
                                let result = view.open_focused_text_agent_chat_fixture(
                                    text,
                                    instruction,
                                    "focused_text_pi_fixture",
                                    ctx,
                                );
                                let ok = result.is_ok();
                                let (error_code, error_message) = match result {
                                    Ok(()) => (None, None),
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to open focused-text Agent Chat real Pi fixture: {error}"),
                                        );
                                        let error_text = error.to_string();
                                        if error_text.contains("SCRIPT_KIT_INLINE_AGENT_REAL_PI_FIXTURE") {
                                            (
                                                Some("gated_off".to_string()),
                                                Some("Focused-text Agent Chat real Pi fixture is gated off".to_string()),
                                            )
                                        } else {
                                            (
                                                Some("open_failed".to_string()),
                                                Some("Focused-text Agent Chat real Pi fixture open failed".to_string()),
                                            )
                                        }
                                    }
                                };
                                if let Some(rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::focused_text_agent_chat_fixture_open_result(
                                                rid.to_string(),
                                                "pi".to_string(),
                                                ok,
                                                ok && requested_submit,
                                                text_length,
                                                instruction_length,
                                                error_code,
                                                error_message,
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::ShowAiCommandBar => {
                                logging::log("STDIN", "Ignoring showAiCommandBar: legacy AI window removed");
                            }
                            ExternalCommand::SimulateAiKey { key, modifiers, .. } => {
                                logging::log(
                                    "STDIN",
                                    &format!("Ignoring simulateAiKey '{}' (modifiers: {:?}): legacy AI window removed", key, modifiers),
                                );
                            }
                            ExternalCommand::CaptureWindow { title, path, .. } => {
                                logging::log("STDIN", &format!("Capturing window with title '{}' to '{}'", title, path));
                                match validate_capture_window_output_path(&path) {
                                    Ok(validated_path) => {
                                        match capture_window_by_title_via_resolver(&title, false) {
                                            Ok((png_data, width, height)) => {
                                                let mut can_write = true;
                                                if let Some(parent) = validated_path.parent() {
                                                    if let Err(e) = std::fs::create_dir_all(parent) {
                                                        can_write = false;
                                                        logging::log(
                                                            "STDIN",
                                                            &format!(
                                                                "Failed to create screenshot directory '{}': {}",
                                                                parent.display(),
                                                                e
                                                            ),
                                                        );
                                                    }
                                                }

                                                if can_write {
                                                    if let Err(e) = std::fs::write(&validated_path, &png_data) {
                                                        logging::log(
                                                            "STDIN",
                                                            &format!("Failed to write screenshot: {}", e),
                                                        );
                                                    } else {
                                                        logging::log(
                                                            "STDIN",
                                                            &format!(
                                                                "Screenshot saved: {} ({}x{})",
                                                                validated_path.display(),
                                                                width,
                                                                height
                                                            ),
                                                        );
                                                    }
                                                } else {
                                                    tracing::warn!(
                                                        category = "STDIN",
                                                        event_type = "stdin_capture_window_dir_create_failed",
                                                        requested_path = %path,
                                                        resolved_path = %validated_path.display(),
                                                        correlation_id = %logging::current_correlation_id(),
                                                        "Skipping screenshot write due to directory creation failure"
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    category = "STDIN",
                                                    event_type = "stdin_capture_window_failed",
                                                    requested_title = %title,
                                                    requested_path = %path,
                                                    error = %e,
                                                    correlation_id = %logging::current_correlation_id(),
                                                    "captureWindow failed before writing screenshot"
                                                );
                                                logging::log("STDIN", &format!("Failed to capture window: {}", e));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let correlation_id = logging::current_correlation_id();
                                        tracing::warn!(
                                            category = "STDIN",
                                            event_type = "stdin_capture_window_path_rejected",
                                            requested_path = %path,
                                            reason = %e,
                                            correlation_id = %correlation_id,
                                            "Rejected captureWindow output path"
                                        );
                                        logging::log(
                                            "STDIN",
                                            &format!("Rejected captureWindow path '{}': {}", path, e),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAiSearch { text, ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_received",
                                    command = "setAiSearch",
                                    request_id = ?request_id,
                                    text_len = text.len(),
                                    "STDIN AI command received"
                                );
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_finished",
                                    command = "setAiSearch",
                                    request_id = ?request_id,
                                    status = "unsupported",
                                    "setAiSearch removed with the legacy AI window"
                                );
                            }
                            ExternalCommand::SetAiInput { text, submit, ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_received",
                                    command = "setAiInput",
                                    request_id = ?request_id,
                                    submit,
                                    text_len = text.len(),
                                    "STDIN AI command received"
                                );
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_finished",
                                    command = "setAiInput",
                                    request_id = ?request_id,
                                    submit,
                                    status = "unsupported",
                                    "setAiInput removed with the legacy AI window; use setAgentChatInput"
                                );
                            }
                            ExternalCommand::SetAgentChatInput { text, submit, ref request_id } => {
                                let request_id_value = request_id.clone();
                                let request_id = request_id_value.as_deref();
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_agent_chat_command_received",
                                    command = "setAgentChatInput",
                                    request_id = ?request_id,
                                    submit,
                                    text_len = text.len(),
                                    "STDIN Agent Chat command received"
                                );
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.set_input_in_window(text.clone(), window, cx);
                                            if submit {
                                                if chat.is_focused_text_mini() {
                                                    let _ = chat.submit_focused_text_from_enter(cx);
                                                } else if let Some(thread) = chat.thread() {
                                                    let _ = thread
                                                        .update(cx, |thread, cx| thread.submit_input(cx));
                                                }
                                            }
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                match &result {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatInput",
                                            request_id = ?request_id,
                                            submit,
                                            status = "success",
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to set Agent Chat input: {}", error),
                                        );
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatInput",
                                            request_id = ?request_id,
                                            submit,
                                            status = "error",
                                            error = %error,
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                }
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatInput".to_string(),
                                                result.is_ok(),
                                                result
                                                    .as_ref()
                                                    .err()
                                                    .map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAgentChatTestFixture {
                                ref phase,
                                ref user_text,
                                ref assistant_text,
                                ref message_count,
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let request_id = request_id_value.as_deref();
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_agent_chat_command_received",
                                    command = "setAgentChatTestFixture",
                                    request_id = ?request_id,
                                    phase = %phase,
                                    user_text_len = user_text.as_ref().map(|text| text.len()).unwrap_or(0),
                                    assistant_text_len = assistant_text.as_ref().map(|text| text.len()).unwrap_or(0),
                                    message_count = message_count.unwrap_or(0),
                                    "STDIN Agent Chat command received"
                                );
                                let result = if let Some(origin) = phase.strip_prefix("c06AgentReturn:") {
                                    view.set_agent_chat_return_route_fixture(origin, ctx)
                                } else if let Some(origin) = phase.strip_prefix("c06FlowReturn:") {
                                    view.set_flow_conversation_return_route_fixture(origin, ctx)
                                } else {
                                    match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.apply_test_fixture(
                                                phase,
                                                user_text.clone(),
                                                assistant_text.clone(),
                                                *message_count,
                                                cx,
                                            )
                                        })
                                    }
                                    AppView::FlowSessionView { .. } => view
                                        .apply_flow_conversation_test_fixture(
                                            phase,
                                            user_text.clone(),
                                            assistant_text.clone(),
                                            ctx,
                                        ),
                                    AppView::ChatPrompt { entity, .. } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.apply_transcript_geometry_fixture(
                                                phase,
                                                user_text.clone(),
                                                assistant_text.clone(),
                                                cx,
                                            )
                                        })
                                    }
                                    _ => crate::ai::agent_chat::ui::chat_window::apply_detached_agent_chat_test_fixture(
                                        phase,
                                        user_text.clone(),
                                        assistant_text.clone(),
                                        *message_count,
                                        ctx,
                                    ),
                                    }
                                };
                                match &result {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatTestFixture",
                                            request_id = ?request_id,
                                            phase = %phase,
                                            status = "success",
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to set Agent Chat test fixture: {}", error),
                                        );
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatTestFixture",
                                            request_id = ?request_id,
                                            phase = %phase,
                                            status = "error",
                                            error = %error,
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                }
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatTestFixture".to_string(),
                                                result.is_ok(),
                                                result
                                                    .as_ref()
                                                    .err()
                                                    .map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAgentChatTranscriptScroll {
                                item_ix,
                                offset_px,
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.scroll_test_transcript_to(item_ix, offset_px, cx)
                                        })
                                    }
                                    AppView::FlowSessionView { session_id } => view
                                        .conversations.flow_sessions
                                        .iter()
                                        .find(|(meta, _)| meta.id == *session_id)
                                        .map(|(_, entity)| entity.clone())
                                        .ok_or_else(|| "FlowSession entity is not active".to_string())
                                        .map(|entity| {
                                            entity.update(ctx, |chat, cx| {
                                                chat.set_transcript_geometry_scroll(
                                                    item_ix, offset_px, cx,
                                                );
                                            });
                                        }),
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatTranscriptScroll".to_string(),
                                                result.is_ok(),
                                                result
                                                    .as_ref()
                                                    .err()
                                                    .map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::PasteClipboardIntoAgentChat { ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_agent_chat_command_received",
                                    command = "pasteClipboardIntoAgentChat",
                                    request_id = ?request_id,
                                    "STDIN Agent Chat command received"
                                );
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        let pasted = entity
                                            .update(ctx, |chat, cx| chat.paste_text_from_clipboard(cx));
                                        if pasted {
                                            Ok(())
                                        } else {
                                            Err("clipboard is empty or text fetch failed"
                                                .to_string())
                                        }
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                match result {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "pasteClipboardIntoAgentChat",
                                            request_id = ?request_id,
                                            status = "success",
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to paste clipboard into Agent Chat: {}", error),
                                        );
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "pasteClipboardIntoAgentChat",
                                            request_id = ?request_id,
                                            status = "error",
                                            error = %error,
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::PushDictationResult {
                                ref transcript,
                                ref partial_transcript,
                                ref target,
                                freeze_only,
                                use_frozen_selection,
                                ref request_id,
                            } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                let target_label = target.as_deref().unwrap_or("unspecified");
                                let resolution =
                                    crate::dictation::resolve_final_or_partial_transcript(
                                        transcript,
                                        partial_transcript.as_deref(),
                                    );
                                match view.deliver_stdin_dictation_result(
                                    transcript.clone(),
                                    partial_transcript.as_deref(),
                                    target.as_deref(),
                                    freeze_only,
                                    use_frozen_selection,
                                    ctx,
                                ) {
                                    Ok(delivery_target) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "push_dictation_result_delivered",
                                            command = "pushDictationResult",
                                            request_id = ?rid,
                                            transcript_len = resolution.transcript.as_ref().map_or(0, String::len),
                                            final_transcript_len = resolution.final_len,
                                            partial_transcript_len = ?resolution.partial_len,
                                            partial_fallback_used = resolution.used_partial_fallback,
                                            requested_target = target_label,
                                            delivery_target = ?delivery_target,
                                            "pushDictationResult RPC delivered through dictation pipeline"
                                        );
                                    }
                                    Err(error) => {
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "push_dictation_result_failed",
                                            command = "pushDictationResult",
                                            request_id = ?rid,
                                            transcript_len = resolution.transcript.as_ref().map_or(0, String::len),
                                            final_transcript_len = resolution.final_len,
                                            partial_transcript_len = ?resolution.partial_len,
                                            partial_fallback_used = resolution.used_partial_fallback,
                                            requested_target = target_label,
                                            error = %error,
                                            "pushDictationResult RPC failed"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::GetAiWindowState { ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "ai_window_state_result",
                                    command = "getAiWindowState",
                                    request_id = ?request_id,
                                    ok = false,
                                    error_code = "ai_window_removed",
                                    "legacy AI window removed"
                                );
                            }
                            ExternalCommand::OpenDictationOverlayFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                crate::dictation::set_dictation_overlay_fixture_mode(true);
                                match crate::dictation::open_dictation_overlay(ctx) {
                                    Ok(handle) => {
                                        let fixture_bounds = gpui::Bounds {
                                            origin: gpui::point(gpui::px(585.0), gpui::px(177.0)),
                                            size: gpui::size(gpui::px(560.0), gpui::px(100.0)),
                                        };
                                        let _ = handle.update(ctx, |_view, window, cx| {
                                            crate::components::inline_popup_window::set_inline_popup_window_bounds(window, fixture_bounds, cx);
                                        });
                                        crate::windows::set_automation_bounds(
                                            "dictation",
                                            Some(crate::protocol::AutomationWindowBounds {
                                                x: 585.0,
                                                y: 177.0,
                                                width: 560.0,
                                                height: 100.0,
                                            }),
                                        );
                                        let state = crate::dictation::DictationOverlayState {
                                            phase: crate::dictation::DictationSessionPhase::Recording,
                                            elapsed: std::time::Duration::from_secs(7),
                                            bars: [0.12, 0.34, 0.62, 0.88, 0.55, 0.31, 0.74, 0.42, 0.18],
                                            transcript: gpui::SharedString::default(),
                                            target: crate::dictation::DictationTarget::ExternalApp,
                                        };
                                        let _ = crate::dictation::update_dictation_overlay(state, ctx);
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "dictation_overlay_fixture_opened",
                                            command = "openDictationOverlayFixture",
                                            request_id = ?rid,
                                            "Dictation overlay fixture opened without media capture"
                                        );
                                    }
                                    Err(error) => {
                                        crate::dictation::set_dictation_overlay_fixture_mode(false);
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "dictation_overlay_fixture_failed",
                                            command = "openDictationOverlayFixture",
                                            request_id = ?rid,
                                            error = %error,
                                            "Dictation overlay fixture failed"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::OpenDictationMicrophonePopupFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                match crate::dictation::open_dictation_microphone_popup_fixture(ctx) {
                                    Ok(()) => tracing::info!(
                                        category = "STDIN",
                                        event = "dictation_microphone_popup_fixture_opened",
                                        command = "openDictationMicrophonePopupFixture",
                                        request_id = ?rid,
                                        "Dictation microphone popup fixture opened without persistence"
                                    ),
                                    Err(error) => tracing::error!(
                                        category = "STDIN",
                                        event = "dictation_microphone_popup_fixture_failed",
                                        command = "openDictationMicrophonePopupFixture",
                                        request_id = ?rid,
                                        error = %error,
                                        "Dictation microphone popup fixture failed"
                                    ),
                                }
                            }
                            ExternalCommand::GetConfigFingerprint { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                match crate::config::current_config_fingerprint_receipt() {
                                    Some(receipt) => {
                                        let json = serde_json::to_string(&receipt).unwrap_or_default();
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "config_fingerprint_result",
                                            command = "getConfigFingerprint",
                                            request_id = ?rid,
                                            ok = true,
                                            state = %json,
                                            "config.ts fingerprint snapshot"
                                        );
                                    }
                                    None => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "config_fingerprint_result",
                                            command = "getConfigFingerprint",
                                            request_id = ?rid,
                                            ok = false,
                                            error_code = "config_file_missing",
                                            "config.ts not found or metadata unreadable"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::ShowGrid { grid_size, show_bounds, show_box_model, show_alignment_guides, show_dimensions, ref depth, .. } => {
                                logging::log("STDIN", &format!(
                                    "ShowGrid: size={}, bounds={}, box_model={}, guides={}, dimensions={}, depth={:?}",
                                    grid_size, show_bounds, show_box_model, show_alignment_guides, show_dimensions, depth
                                ));
                                let options = protocol::GridOptions {
                                    grid_size,
                                    show_bounds,
                                    show_box_model,
                                    show_alignment_guides,
                                    show_dimensions,
                                    depth: depth.clone(),
                                    color_scheme: None,
                                };
                                view.show_grid(options, ctx);
                            }
                            ExternalCommand::HideGrid => {
                                logging::log("STDIN", "HideGrid: hiding debug grid overlay");
                                view.hide_grid(ctx);
                            }
                            ExternalCommand::ExecuteFallback { ref fallback_id, ref input, .. } => {
                                logging::log("STDIN", &format!("ExecuteFallback: id='{}', input='{}'", fallback_id, input));
                                execute_fallback_action(view, fallback_id, input, window, ctx);
                            }
                            ExternalCommand::ShowShortcutRecorder { ref command_id, ref command_name, .. } => {
                                logging::log("STDIN", &format!("ShowShortcutRecorder: command_id='{}', command_name='{}'", command_id, command_name));
                                view.show_shortcut_recorder(command_id.clone(), command_name.clone(), window, ctx);
                            }
                            ExternalCommand::SetAgentChatScopeInput { ref text, ref request_id } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            let was_visible = chat.scope_visible;
                                            chat.scope_input = crate::ai::agent_chat::ui::view::AgentChatView::normalize_focused_text_scope_input_public(text);
                                            chat.scope_visible = !chat.scope_input.is_empty();
                                            if chat.scope_visible != was_visible {
                                                chat.resize_focused_text_mini_for_scope_change_public(&*cx);
                                            }
                                            cx.notify();
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(crate::protocol::Message::external_command_result(
                                            rid.to_string(), "setAgentChatScopeInput".to_string(), result.is_ok(),
                                            result.as_ref().err().map(|_| "agent_chat_inactive".to_string()),
                                            result.as_ref().err().cloned(),
                                        ));
                                    }
                                }
                            }
                            ExternalCommand::SelectAgentChatVariation { index, edit, ref request_id } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.select_focused_text_variation(index, cx);
                                            if edit {
                                                let _ = chat.enter_focused_text_variation_editor(cx);
                                            }
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(crate::protocol::Message::external_command_result(
                                            rid.to_string(), "selectAgentChatVariation".to_string(), result.is_ok(),
                                            result.as_ref().err().map(|_| "agent_chat_inactive".to_string()),
                                            result.as_ref().err().cloned(),
                                        ));
                                    }
                                }
                            }
                            ExternalCommand::GetAgentChatVariations { ref request_id } => {
                                let request_id_value = request_id.clone();
                                let summary = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let snapshots = entity.read(ctx).focused_text_variation_snapshots();
                                        if snapshots.is_empty() {
                                            "no_variations".to_string()
                                        } else {
                                            snapshots.iter().map(|s| {
                                                let preview = crate::logging::truncate_for_log(&s.text, 100);
                                                format!("[{}] {} ({}) sel={} len={}: {}", s.index, s.label, s.status.state_id(), s.selected, s.text.len(), preview)
                                            }).collect::<Vec<_>>().join(" | ")
                                        }
                                    }
                                    _ => "not_agent_chat".to_string(),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(crate::protocol::Message::external_command_result(
                                            rid.to_string(), "getAgentChatVariations".to_string(), true, None, Some(summary),
                                        ));
                                    }
                                }
                            }
                            ExternalCommand::AgentChatEscape { ref request_id } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.handle_protocol_escape(window, cx);
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(crate::protocol::Message::external_command_result(
                                            rid.to_string(), "agent_chatEscape".to_string(), result.is_ok(),
                                            result.as_ref().err().map(|_| "agent_chat_inactive".to_string()),
                                            result.as_ref().err().cloned(),
                                        ));
                                    }
                                }
                            }
                        },
                        StdinCommand::Protocol(message) => {
                            logging::log("STDIN", "Routing stdin protocol message");
                            view.handle_stdin_protocol_message(*message, window, ctx);
                        }
                    }
                    view.sync_main_footer_popup(window, ctx);
                    ctx.notify();
                }); // close app_entity_inner.update
            }); // close window_for_stdin.update
        }); // close cx.update
        if let Some(response_tx) = mcp_response_tx {
            let _ = response_tx.send(Ok(crate::mcp_kit_tools::KitRuntimeCommandResult {
                accepted: true,
                command_type: command_type.to_string(),
                request_id,
            }));
        }
    }

    logging::log("STDIN", "Async stdin command handler exiting");
})
.detach();

        // Shutdown monitor task - checks SHUTDOWN_REQUESTED flag set by signal handler
        // Performs all cleanup on the main thread where it's safe to call logging,
        // mutexes, and other non-async-signal-safe functions.
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            loop {
                // Check every 500ms for shutdown signal
                // 500ms is acceptable latency for graceful shutdown while reducing CPU wakeups
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;

                if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
                    logging::log("SHUTDOWN", "Shutdown signal detected, performing graceful cleanup");

                    // Kill all tracked child processes
                    logging::log("SHUTDOWN", "Killing all child processes");
                    PROCESS_MANAGER.kill_all_processes();

                    // Remove main PID file
                    PROCESS_MANAGER.remove_main_pid();

                    logging::log("SHUTDOWN", "Cleanup complete, quitting application");

                    // Quit the GPUI application
                    cx.update(|cx| {
                        cx.quit();
                    });

                    break;
                }
            }
        }).detach();

        logging::log("APP", "Application ready - Cmd+; to show, Esc to hide, Cmd+K for actions");

});
}

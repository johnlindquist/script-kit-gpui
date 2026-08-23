fn initialize_startup_diagnostics() {
    logging::init();
    logging::log(
        "KEY_SETUP",
        &format!(
            "SHORTCUT_DEBUG_BOOT pid={} exe={} ai_log={} rust_log={} session_name={} session_generation={} protocol_responses_path={} shortcut_debug={} keep_actions_window_open={}",
            std::process::id(),
            std::env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("<error:{error}>")),
            std::env::var("SCRIPT_KIT_AI_LOG").unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("RUST_LOG").unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("SCRIPT_KIT_AGENTIC_SESSION_NAME")
                .unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("SCRIPT_KIT_AGENTIC_SESSION_GENERATION")
                .unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("SCRIPT_KIT_AGENTIC_PROTOCOL_RESPONSES_PATH")
                .unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("SCRIPT_KIT_SHORTCUT_DEBUG")
                .unwrap_or_else(|_| "<unset>".to_string()),
            std::env::var("SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN")
                .unwrap_or_else(|_| "<unset>".to_string()),
        ),
    );
}

fn register_startup_window_routers() {
    // Register the in-window confirm router so `confirm_with_parent_dialog`
    // can push `AppView::ConfirmPrompt` onto the main `ScriptListApp` entity
    // instead of opening the popup window when the main window is active.
    //
    // The window's root view is `gpui_component::Root` wrapping the actual
    // `ScriptListApp` AnyView, so the router unwraps Root → inner AnyView →
    // ScriptListApp before pushing the confirm prompt.
    crate::confirm::parent_dialog::register_in_window_router(Box::new(
        |any_view, options, sender, cx| {
            let root = match any_view.downcast::<gpui_component::Root>() {
                Ok(r) => r,
                Err(_) => return false,
            };
            let inner_any = root.read(cx).view().clone();
            if let Ok(entity) = inner_any.downcast::<ScriptListApp>() {
                entity.update(cx, |app, cx| {
                    app.open_confirm_prompt(options, sender, cx);
                });
                true
            } else {
                false
            }
        },
    ));

    // Notes → main Agent Chat handoff: the dual-compiled Notes window code
    // cannot name `ScriptListApp`, so the binary registers the
    // downcast-and-stage closure here. Staging happens BEFORE ShowMain so the
    // first visible main-window frame already has the note chip and prefill.
    crate::notes::window::ai_handoff::register_notes_ai_main_handoff_hook(|payload, cx| {
        let Some(handle) = crate::get_main_window_handle() else {
            return Err(
                crate::notes::window::ai_handoff::NotesAiMainHandoffFailure::new(
                    crate::notes::window::ai_handoff::NotesAiHandoffError::MainWindowUnavailable,
                    "notes_main_window_handle_missing",
                ),
            );
        };
        handle
            .update(cx, move |any_view, _window, cx| {
                let root = any_view.downcast::<gpui_component::Root>().map_err(|_| {
                    crate::notes::window::ai_handoff::NotesAiMainHandoffFailure::new(
                        crate::notes::window::ai_handoff::NotesAiHandoffError::MainStagingFailed,
                        "notes_main_window_root_unavailable",
                    )
                })?;
                let inner = root.read(cx).view().clone();
                let app = inner.downcast::<ScriptListApp>().map_err(|_| {
                    crate::notes::window::ai_handoff::NotesAiMainHandoffFailure::new(
                        crate::notes::window::ai_handoff::NotesAiHandoffError::MainStagingFailed,
                        "notes_main_window_app_unavailable",
                    )
                })?;
                Ok(app.update(cx, |app, cx| {
                    let outcome = app.open_agent_chat_from_notes(payload, cx);
                    if outcome.primary.is_consumable() {
                        app.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::ShowMain {
                                activate_app: true,
                            },
                            cx,
                        );
                    }
                    outcome
                }))
            })
            .map_err(|_| {
                crate::notes::window::ai_handoff::NotesAiMainHandoffFailure::new(
                    crate::notes::window::ai_handoff::NotesAiHandoffError::MainWindowUnavailable,
                    "notes_main_window_update_failed",
                )
            })?
    });
}

fn prepare_startup_environment() -> setup::SetupResult {
    // Fail-loud-at-startup validation of the trigger-builtin registry.
    // A duplicate alias or a typoed canonical id would previously only
    // surface as a silent runtime no-op — see Run 7 Pass #8/#9.
    if let Err(e) = crate::builtins::validate_trigger_registry() {
        panic!("invalid triggerBuiltin registry detected at startup: {e}");
    }

    // Migrate from legacy ~/.kenv to new ~/.scriptkit structure (one-time migration)
    // This must happen BEFORE ensure_kit_setup() so the new path is used
    if setup::migrate_from_kenv() {
        logging::log("APP", "Migrated from ~/.kenv to ~/.scriptkit");
    }

    // Ensure ~/.scriptkit environment is properly set up (directories, SDK, config, etc.)
    // This is idempotent - it creates missing directories and files without overwriting user configs
    let setup_result = setup::ensure_kit_setup();
    if setup_result.is_fresh_install {
        logging::log(
            "APP",
            &format!(
                "Fresh install detected - created ~/.scriptkit at {}",
                setup_result.kit_path.display()
            ),
        );
    }
    for warning in &setup_result.warnings {
        logging::log("APP", &format!("Setup warning: {}", warning));
    }
    if !setup_result.bun_available {
        logging::log(
            "APP",
            "Warning: bun not found in PATH or common locations. Scripts may not run.",
        );
    }

    // Clean up orphans from previous crashes. The sweep scans per-instance
    // registry files and skips any whose owning instance is still alive, so
    // it is safe to run alongside parallel dev instances.
    let orphans_killed = PROCESS_MANAGER.cleanup_orphans();
    if orphans_killed > 0 {
        logging::log(
            "APP",
            &format!(
                "Cleaned up {} orphaned process(es) from previous session",
                orphans_killed
            ),
        );
    }

    // Write main PID file for orphan detection on crash
    if let Err(e) = PROCESS_MANAGER.write_main_pid() {
        logging::log("APP", &format!("Failed to write main PID file: {}", e));
    } else {
        logging::log("APP", "Main PID file written");
    }

    // Register signal handlers for graceful shutdown
    // SAFETY: Signal handlers can only safely call async-signal-safe functions.
    // We ONLY set an atomic flag here. All cleanup (logging, killing processes,
    // removing PID files) happens in a GPUI task that monitors this flag.
    #[cfg(unix)]
    {
        extern "C" fn handle_signal(_sig: libc::c_int) {
            // ASYNC-SIGNAL-SAFE: Only set atomic flag
            // Do NOT call: logging, mutexes, heap allocation, or any Rust code
            // that might allocate or lock. The GPUI shutdown monitor task will
            // handle all cleanup on the main thread.
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        }

        unsafe {
            // Register handlers for common termination signals
            libc::signal(
                libc::SIGINT,
                handle_signal as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGTERM,
                handle_signal as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGHUP,
                handle_signal as *const () as libc::sighandler_t,
            );
            logging::log(
                "APP",
                "Signal handlers registered (SIGINT, SIGTERM, SIGHUP) - cleanup via GPUI task",
            );
        }
    }

    setup_result
}

fn initialize_startup_background_services(loaded_config: &config::Config) {
    clipboard_history::set_max_text_content_len(
        loaded_config.get_clipboard_history_max_text_length(),
    );
    let secret_rejection = loaded_config.get_clipboard_history_secret_rejection();
    clipboard_history::configure_secret_rejection(clipboard_history::SecretRejectionConfig {
        extra_blocked_source_apps: secret_rejection.extra_blocked_source_apps,
        extra_secret_patterns: secret_rejection.extra_secret_patterns,
    });

    // Initialize clipboard history monitoring (background thread)
    if let Err(e) = clipboard_history::init_clipboard_history() {
        logging::log(
            "APP",
            &format!("Failed to initialize clipboard history: {}", e),
        );
    } else {
        logging::log("APP", "Clipboard history monitoring initialized");
    }

    // Initialize the brain (local memory store) and its background indexer.
    // Never blocks startup: the indexer thread sleeps before its first cycle
    // and embedding work happens in the ghost-llm-helper subprocess.
    match crate::brain::init_brain_db() {
        Ok(()) => {
            crate::brain::start_brain_indexer();
            // Opt-in Telegram remote access to the brain (no-op unless the
            // config enables it with a token and a non-empty allowlist).
            crate::brain::telegram::start_telegram_bridge();
            if let Err(e) = crate::brain::seed::seed_constitution_if_needed() {
                logging::log("BRAIN", &format!("Constitution seeding skipped: {}", e));
            }
            logging::log("BRAIN", "Brain store initialized; indexer started");
        }
        Err(e) => {
            logging::log("BRAIN", &format!("Failed to initialize brain store: {}", e));
        }
    }

    // Initialize text expansion system (background thread with keyboard monitoring)
    // This must be done early, before the GPUI run loop starts
    // Uses a global singleton so the manager can be updated when scriptlet files change
    #[cfg(target_os = "macos")]
    {
        // Spawn initialization in a thread to not block startup
        std::thread::spawn(move || {
            logging::log("KEYWORD", "Initializing text expansion system");

            match keyword_manager::init_keyword_manager() {
                Ok(Some(count)) => {
                    logging::log(
                        "KEYWORD",
                        &format!("Text expansion system enabled with {} triggers", count),
                    );
                }
                Ok(None) => {
                    logging::log(
                        "KEYWORD",
                        "Accessibility permissions not granted - text expansion disabled",
                    );
                    logging::log(
                        "KEYWORD",
                        "Enable in System Preferences > Privacy & Security > Accessibility",
                    );
                    // Keep this thread waiting so granting Accessibility arms
                    // text expansion without an app restart.
                    match keyword_manager::init_keyword_manager_when_accessibility_granted() {
                        Ok(Some(count)) => logging::log(
                            "KEYWORD",
                            &format!(
                                "Accessibility granted - text expansion enabled with {} triggers",
                                count
                            ),
                        ),
                        Ok(None) => {}
                        Err(e) => logging::log(
                            "KEYWORD",
                            &format!("Failed to initialize text expansion after grant: {}", e),
                        ),
                    }
                }
                Err(e) => {
                    logging::log(
                        "KEYWORD",
                        &format!("Failed to initialize text expansion: {}", e),
                    );
                }
            }
        });
    }
}

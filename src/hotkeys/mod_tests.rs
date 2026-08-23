#[cfg(test)]
mod tests {
    use super::*;
    use async_channel::TryRecvError;

    // =============================================================================
    // Unified Routing Table Tests
    // =============================================================================
    mod routing_table_tests {
        use super::*;

        #[test]
        fn route_summary_lines_stay_under_log_hygiene_budget() {
            // Why: a single GLOBAL_HOTKEY_ROUTE_SUMMARY line enumerating every
            // route crossed 2048 bytes (2136 observed 2026-07-18) and failed
            // the typing-lag benchmark's maxLogLineBytes gate. Details must
            // chunk across lines while preserving every route entry.
            let details: Vec<String> = (0..200)
                .map(|i| format!("{i}:cmd+alt+shift+f{}=>script:/Users/example/.scriptkit/scripts/some-long-script-name-{i}.ts", i % 12))
                .collect();
            let lines = route_summary_lines(
                "GLOBAL_HOTKEY_ROUTE_SUMMARY scripts=200 routes=200".to_string(),
                &details,
            );
            assert!(lines.len() > 1, "long route sets must chunk");
            for line in &lines {
                assert!(
                    line.len() <= ROUTE_SUMMARY_LINE_BUDGET,
                    "line exceeds budget ({} bytes): {}",
                    line.len(),
                    &line[..80.min(line.len())]
                );
            }
            let joined = lines.join("\n");
            for detail in &details {
                assert!(
                    joined.contains(detail.as_str()),
                    "route entry dropped: {detail}"
                );
            }
        }

        #[test]
        fn test_hotkey_routes_new() {
            let routes = HotkeyRoutes::new();
            assert!(routes.routes.is_empty());
            assert!(routes.script_paths.is_empty());
            assert!(routes.main_id.is_none());
            assert!(routes.notes_id.is_none());
            assert!(routes.ai_id.is_none());
        }

        #[test]
        fn test_add_main_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META), Code::Semicolon);
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Main,
                display: "cmd+;".to_string(),
            };
            routes.add_route(hotkey.id(), entry);

            assert_eq!(routes.main_id, Some(hotkey.id()));
            assert!(routes.routes.contains_key(&hotkey.id()));
            assert_eq!(routes.get_action(hotkey.id()), Some(HotkeyAction::Main));
        }

        #[test]
        fn test_add_script_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyT);
            let path = "/test/script.ts".to_string();
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Script(path.clone()),
                display: "cmd+shift+t".to_string(),
            };
            routes.add_route(hotkey.id(), entry);

            assert_eq!(routes.script_paths.get(&path), Some(&hotkey.id()));
            assert_eq!(routes.get_script_id(&path), Some(hotkey.id()));
            assert_eq!(
                routes.get_action(hotkey.id()),
                Some(HotkeyAction::Script(path))
            );
        }

        #[test]
        fn test_remove_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META), Code::KeyN);
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Notes,
                display: "cmd+n".to_string(),
            };
            routes.add_route(hotkey.id(), entry);
            assert!(routes.notes_id.is_some());

            let removed = routes.remove_route(hotkey.id());
            assert!(removed.is_some());
            assert!(routes.notes_id.is_none());
            assert!(routes.get_action(hotkey.id()).is_none());
        }

        #[test]
        fn test_remove_script_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META), Code::KeyS);
            let path = "/test/script.ts".to_string();
            let entry = RegisteredHotkey {
                hotkey,
                action: HotkeyAction::Script(path.clone()),
                display: "cmd+s".to_string(),
            };
            routes.add_route(hotkey.id(), entry);
            assert!(routes.script_paths.contains_key(&path));

            routes.remove_route(hotkey.id());
            assert!(!routes.script_paths.contains_key(&path));
        }

        #[test]
        fn shortcut_conflict_reports_existing_live_route() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyK);
            routes.add_route(
                hotkey.id(),
                RegisteredHotkey {
                    hotkey,
                    action: HotkeyAction::Script("builtin/existing-command".to_string()),
                    display: "cmd+shift+k".to_string(),
                },
            );

            let conflict =
                shortcut_conflict_in_routes(&routes, "builtin/new-command", "cmd+shift+k")
                    .expect("expected conflict with existing live route");

            assert_eq!(conflict.command_id, "builtin/existing-command");
            assert_eq!(conflict.command_name, "builtin/existing-command");
            assert_eq!(conflict.shortcut, "cmd+shift+k");
        }

        #[test]
        fn shortcut_conflict_ignores_current_command_rebind() {
            let mut routes = HotkeyRoutes::new();
            let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyK);
            routes.add_route(
                hotkey.id(),
                RegisteredHotkey {
                    hotkey,
                    action: HotkeyAction::Script("builtin/current-command".to_string()),
                    display: "cmd+shift+k".to_string(),
                },
            );

            assert!(
                shortcut_conflict_in_routes(&routes, "builtin/current-command", "cmd+shift+k",)
                    .is_none(),
                "updating the selected command's current shortcut should not self-conflict"
            );
        }

        #[test]
        fn test_hotkey_action_equality() {
            assert_eq!(HotkeyAction::Main, HotkeyAction::Main);
            assert_eq!(HotkeyAction::Notes, HotkeyAction::Notes);
            assert_eq!(HotkeyAction::Ai, HotkeyAction::Ai);
            assert_eq!(HotkeyAction::ToggleLogs, HotkeyAction::ToggleLogs);
            assert_eq!(
                HotkeyAction::Script("/a.ts".to_string()),
                HotkeyAction::Script("/a.ts".to_string())
            );
            assert_ne!(HotkeyAction::Main, HotkeyAction::Notes);
            assert_ne!(HotkeyAction::Main, HotkeyAction::ToggleLogs);
            assert_ne!(
                HotkeyAction::Script("/a.ts".to_string()),
                HotkeyAction::Script("/b.ts".to_string())
            );
        }
    }

    #[test]
    fn hotkey_channels_are_independent() {
        while hotkey_channel().1.try_recv().is_ok() {}
        while script_hotkey_channel().1.try_recv().is_ok() {}

        hotkey_channel()
            .0
            .send_blocking(HotkeyEvent {
                correlation_id: "cid-main".to_string(),
            })
            .expect("send hotkey");
        assert!(matches!(
            script_hotkey_channel().1.try_recv(),
            Err(TryRecvError::Empty)
        ));
        let hotkey_event = hotkey_channel().1.try_recv().expect("recv hotkey");
        assert_eq!(hotkey_event.correlation_id, "cid-main");

        script_hotkey_channel()
            .0
            .send_blocking(ScriptHotkeyEvent {
                command_id: "script".to_string(),
                correlation_id: "cid-script".to_string(),
            })
            .expect("send script hotkey");
        let script_event = script_hotkey_channel()
            .1
            .try_recv()
            .expect("recv script hotkey");
        assert_eq!(script_event.command_id, "script");
        assert_eq!(script_event.correlation_id, "cid-script");
    }

    #[test]
    fn test_hotkey_handler_mutex_poison_recovery() {
        let storage = std::sync::Arc::new(std::sync::Mutex::new(Some(
            Arc::new(|| {}) as HotkeyHandler
        )));
        let poison_storage = std::sync::Arc::clone(&storage);

        let _ = std::thread::spawn(move || {
            let _guard = poison_storage.lock().unwrap();
            panic!("poison handler lock");
        })
        .join();

        assert!(
            storage.is_poisoned(),
            "mutex should be poisoned for this test"
        );

        let recovered = clone_hotkey_handler_with_poison_recovery(
            storage.as_ref(),
            "test_hotkey_handler_mutex_poison_recovery",
        );
        assert!(
            recovered.is_some(),
            "poison recovery should still return the existing handler"
        );

        // Verify the mutex remains usable after recovery.
        *storage.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    // =============================================================================
    // ScriptHotkeyManager Unit Tests
    // =============================================================================
    // Note: These tests cannot actually register system hotkeys in the test environment
    // because GlobalHotKeyManager requires a running event loop and proper OS permissions.
    // Instead, we test the logic of the manager's internal tracking.

    mod script_hotkey_manager_tests {
        use super::*;

        /// Helper to create a manager for testing.
        /// Note: Registration will fail without an event loop, but we can test tracking logic.
        fn create_test_manager() -> Option<ScriptHotkeyManager> {
            // GlobalHotKeyManager::new() may fail in test environment
            GlobalHotKeyManager::new()
                .ok()
                .map(ScriptHotkeyManager::new)
        }

        #[test]
        fn test_manager_creation() {
            // Just verify we can create the struct (manager creation may fail in CI)
            if let Some(manager) = create_test_manager() {
                assert!(manager.hotkey_map.is_empty());
                assert!(manager.path_to_id.is_empty());
            }
        }

        #[test]
        fn test_get_registered_hotkeys_empty() {
            if let Some(manager) = create_test_manager() {
                assert!(manager.get_registered_hotkeys().is_empty());
            }
        }

        #[test]
        fn test_is_registered_false_for_unknown_path() {
            if let Some(manager) = create_test_manager() {
                assert!(!manager.is_registered("/some/unknown/path.ts"));
            }
        }

        #[test]
        fn test_unregister_nonexistent_is_noop() {
            if let Some(mut manager) = create_test_manager() {
                // Should not error when unregistering a path that was never registered
                let result = manager.unregister("/nonexistent/path.ts");
                assert!(result.is_ok());
            }
        }

        #[test]
        fn test_update_none_to_none_is_noop() {
            if let Some(mut manager) = create_test_manager() {
                // No old, no new -> no-op, should succeed
                let result = manager.update("/some/path.ts", None, None);
                assert!(result.is_ok());
            }
        }

        // Note: The following tests would require a working GlobalHotKeyManager
        // which may not be available in all test environments.
        // In a real CI environment, these would be integration tests.

        #[test]
        fn test_register_tracks_mapping() {
            if let Some(mut manager) = create_test_manager() {
                // Try to register - this may fail in test environment, that's OK
                let result = manager.register("/test/script.ts", "cmd+shift+t");
                if result.is_ok() {
                    // If registration succeeded, verify tracking
                    assert!(manager.is_registered("/test/script.ts"));
                    let hotkeys = manager.get_registered_hotkeys();
                    assert_eq!(hotkeys.len(), 1);
                    assert_eq!(hotkeys[0].0, "/test/script.ts");
                }
                // If it failed (no event loop), that's expected in test env
            }
        }

        #[test]
        fn test_unregister_removes_tracking() {
            if let Some(mut manager) = create_test_manager() {
                // Try to register first
                if manager.register("/test/script.ts", "cmd+shift+u").is_ok() {
                    assert!(manager.is_registered("/test/script.ts"));

                    // Now unregister
                    let result = manager.unregister("/test/script.ts");
                    assert!(result.is_ok());
                    assert!(!manager.is_registered("/test/script.ts"));
                }
            }
        }

        #[test]
        fn test_update_add_hotkey() {
            if let Some(mut manager) = create_test_manager() {
                // None -> Some = add
                let result = manager.update("/test/add.ts", None, Some("cmd+shift+a"));
                if result.is_ok() {
                    assert!(manager.is_registered("/test/add.ts"));
                }
            }
        }

        #[test]
        fn test_update_remove_hotkey() {
            if let Some(mut manager) = create_test_manager() {
                // First register
                if manager.register("/test/remove.ts", "cmd+shift+r").is_ok() {
                    // Some -> None = remove
                    let result = manager.update("/test/remove.ts", Some("cmd+shift+r"), None);
                    assert!(result.is_ok());
                    assert!(!manager.is_registered("/test/remove.ts"));
                }
            }
        }

        #[test]
        fn test_update_change_hotkey() {
            if let Some(mut manager) = create_test_manager() {
                // First register with old shortcut
                if manager.register("/test/change.ts", "cmd+shift+c").is_ok() {
                    // Some -> Some (different) = change
                    let result =
                        manager.update("/test/change.ts", Some("cmd+shift+c"), Some("cmd+alt+c"));
                    if result.is_ok() {
                        // Should still be registered (with new shortcut)
                        assert!(manager.is_registered("/test/change.ts"));
                    }
                }
            }
        }

        #[test]
        fn test_get_script_path() {
            if let Some(mut manager) = create_test_manager() {
                if let Ok(hotkey_id) = manager.register("/test/lookup.ts", "cmd+shift+l") {
                    let path = manager.get_script_path(hotkey_id);
                    assert_eq!(path, Some(&"/test/lookup.ts".to_string()));

                    // Unknown ID returns None
                    assert!(manager.get_script_path(99999).is_none());
                }
            }
        }
    }
}

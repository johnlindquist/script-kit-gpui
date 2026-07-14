//! Tests for window_state persistence module

#[cfg(test)]
mod tests {
    use crate::window_state::*;
    use crate::windows::DisplayBounds;
    use gpui::{point, px, size, Bounds, WindowBounds};
    use std::env;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static HOME_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_temp_state_dir<F: FnOnce()>(f: F) {
        let _home_guard = HOME_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = TempDir::new().unwrap();
        let old_home = env::var("HOME").ok();
        env::set_var("HOME", temp_dir.path());
        let kit_dir = temp_dir.path().join(".sk").join("kit");
        std::fs::create_dir_all(&kit_dir).unwrap();
        f();
        if let Some(home) = old_home {
            env::set_var("HOME", home);
        }
    }

    #[test]
    fn test_persisted_bounds_to_gpui_windowed() {
        let bounds = PersistedWindowBounds {
            mode: PersistedWindowMode::Windowed,
            x: 100.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        };
        let gpui_bounds = bounds.to_gpui();
        match gpui_bounds {
            WindowBounds::Windowed(b) => {
                assert_eq!(f64::from(b.origin.x), 100.0);
                assert_eq!(f64::from(b.origin.y), 200.0);
                assert_eq!(f64::from(b.size.width), 800.0);
                assert_eq!(f64::from(b.size.height), 600.0);
            }
            _ => panic!("Expected Windowed bounds"),
        }
    }

    #[test]
    fn test_persisted_bounds_from_gpui() {
        let gpui_bounds = WindowBounds::Windowed(Bounds {
            origin: point(px(150.0), px(250.0)),
            size: size(px(750.0), px(500.0)),
        });
        let persisted = PersistedWindowBounds::from_gpui(gpui_bounds);
        assert_eq!(persisted.mode, PersistedWindowMode::Windowed);
        assert_eq!(persisted.x, 150.0);
        assert_eq!(persisted.y, 250.0);
    }

    #[test]
    fn test_persisted_bounds_roundtrip() {
        let original = PersistedWindowBounds {
            mode: PersistedWindowMode::Maximized,
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let gpui = original.to_gpui();
        let back = PersistedWindowBounds::from_gpui(gpui);
        assert_eq!(back.mode, original.mode);
        assert_eq!(back.x, original.x);
    }

    #[test]
    fn test_is_bounds_visible_fully_on_screen() {
        let displays = vec![DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }];
        let bounds = PersistedWindowBounds::new(100.0, 100.0, 800.0, 600.0);
        assert!(is_bounds_visible(&bounds, &displays));
    }

    #[test]
    fn test_display_key_uses_shared_windowing_display_bounds() {
        let display = DisplayBounds {
            origin_x: 10.0,
            origin_y: 20.0,
            width: 3000.0,
            height: 2000.0,
        };

        assert_eq!(display_key(&display), "3000x2000@10,20");
    }

    #[test]
    fn test_is_bounds_visible_completely_off_screen() {
        let displays = vec![DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }];
        let bounds = PersistedWindowBounds::new(2000.0, 100.0, 800.0, 600.0);
        assert!(!is_bounds_visible(&bounds, &displays));
    }

    #[test]
    fn test_is_bounds_visible_multi_monitor() {
        let displays = vec![
            DisplayBounds {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            DisplayBounds {
                origin_x: 1920.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        ];
        let bounds = PersistedWindowBounds::new(2000.0, 100.0, 800.0, 600.0);
        assert!(is_bounds_visible(&bounds, &displays));
    }

    #[test]
    fn test_clamp_bounds_off_right() {
        let displays = vec![DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }];
        let bounds = PersistedWindowBounds::new(2500.0, 100.0, 800.0, 600.0);
        let clamped = clamp_bounds_to_displays(&bounds, &displays).unwrap();
        assert!(clamped.x + clamped.width <= 1920.0);
    }

    #[test]
    fn test_save_and_load_state_file() {
        with_temp_state_dir(|| {
            let state = WindowStateFile {
                version: 3,
                main: Some(PersistedWindowBounds::new(100.0, 200.0, 750.0, 475.0)),
                main_per_display: std::collections::HashMap::new(),
                notes: None,
                notes_per_display: std::collections::HashMap::new(),
                ai: None,
                ai_per_display: std::collections::HashMap::new(),
                ai_mini: None,
                agent_chat: None,
            };
            assert!(save_state_file(&state));
            let loaded = load_state_file().expect("Should load saved state");
            assert_eq!(loaded.version, 3);
            assert!(loaded.main.is_some());
        });
    }

    #[test]
    fn test_reset_all_positions() {
        with_temp_state_dir(|| {
            save_window_bounds(
                WindowRole::Main,
                PersistedWindowBounds::new(100.0, 100.0, 750.0, 475.0),
            );
            assert!(has_custom_positions());
            reset_all_positions();
            assert!(!has_custom_positions());
        });
    }

    #[test]
    fn test_display_key_generation() {
        // First display at origin (0,0)
        let display = DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        assert_eq!(display_key(&display), "2560x1440@0,0");

        // Second display to the right of the first
        let display2 = DisplayBounds {
            origin_x: 2560.0,
            origin_y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert_eq!(display_key(&display2), "1920x1080@2560,0");

        // Third display with same resolution as first but different position
        // This should generate a DIFFERENT key (the main fix!)
        let display3 = DisplayBounds {
            origin_x: 5120.0, // Further to the right
            origin_y: 0.0,
            width: 2560.0, // Same resolution as display1
            height: 1440.0,
        };
        assert_eq!(display_key(&display3), "2560x1440@5120,0");

        // Verify same-resolution displays at different positions have unique keys
        assert_ne!(display_key(&display), display_key(&display3));
    }

    #[test]
    fn test_per_display_save_and_load() {
        with_temp_state_dir(|| {
            let display = DisplayBounds {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 2560.0,
                height: 1440.0,
            };
            let bounds = PersistedWindowBounds::new(100.0, 200.0, 750.0, 475.0);

            // Save position for this display
            save_main_position_for_display(&display, bounds);

            // Load and verify
            let loaded = get_main_position_for_display(&display);
            assert!(loaded.is_some());
            let loaded = loaded.unwrap();
            assert!((loaded.x - 100.0).abs() < 0.1);
            assert!((loaded.y - 200.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_per_display_multiple_displays() {
        with_temp_state_dir(|| {
            let display1 = DisplayBounds {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 2560.0,
                height: 1440.0,
            };
            let display2 = DisplayBounds {
                origin_x: 2560.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
            };

            // Save different positions for each display
            save_main_position_for_display(
                &display1,
                PersistedWindowBounds::new(100.0, 100.0, 750.0, 475.0),
            );
            save_main_position_for_display(
                &display2,
                PersistedWindowBounds::new(300.0, 300.0, 750.0, 475.0),
            );

            // Verify each display has its own position
            let loaded1 = get_main_position_for_display(&display1).unwrap();
            let loaded2 = get_main_position_for_display(&display2).unwrap();

            assert!((loaded1.x - 100.0).abs() < 0.1);
            assert!((loaded2.x - 300.0).abs() < 0.1);
        });
    }

    #[test]
    fn test_find_display_containing_point() {
        let displays = vec![
            DisplayBounds {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            DisplayBounds {
                origin_x: 1920.0,
                origin_y: 0.0,
                width: 2560.0,
                height: 1440.0,
            },
        ];

        // Point on first display
        let found = find_display_containing_point(500.0, 500.0, &displays);
        assert!(found.is_some());
        assert!((found.unwrap().width - 1920.0).abs() < 0.1);

        // Point on second display
        let found = find_display_containing_point(2500.0, 500.0, &displays);
        assert!(found.is_some());
        assert!((found.unwrap().width - 2560.0).abs() < 0.1);

        // Point not on any display
        let found = find_display_containing_point(-100.0, -100.0, &displays);
        assert!(found.is_none());
    }

    #[test]
    fn test_save_main_position_with_display_detection_saves_per_display_when_match_found() {
        with_temp_state_dir(|| {
            let displays = vec![DisplayBounds {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
            }];
            let bounds = PersistedWindowBounds::new(100.0, 100.0, 750.0, 475.0);

            let outcome = save_main_position_with_display_detection(bounds, &displays);

            assert_eq!(outcome, MainPositionSaveOutcome::SavedPerDisplay);
            let loaded = load_state_file().expect("state file should exist");
            let saved_main = loaded.main.expect("main bounds should be saved");
            assert!((saved_main.x - bounds.x).abs() < 0.1);
            assert!((saved_main.y - bounds.y).abs() < 0.1);
            assert!((saved_main.width - bounds.width).abs() < 0.1);
            assert!((saved_main.height - bounds.height).abs() < 0.1);
            assert_eq!(loaded.main_per_display.len(), 1);
        });
    }

    #[test]
    fn test_save_main_position_with_display_detection_uses_first_display_when_no_overlap() {
        with_temp_state_dir(|| {
            let displays = vec![DisplayBounds {
                origin_x: 2000.0,
                origin_y: 2000.0,
                width: 1920.0,
                height: 1080.0,
            }];
            let bounds = PersistedWindowBounds::new(100.0, 100.0, 750.0, 475.0);

            let outcome = save_main_position_with_display_detection(bounds, &displays);

            assert_eq!(outcome, MainPositionSaveOutcome::SavedPerDisplay);
            let loaded = load_state_file().expect("state file should exist");
            let saved_main = loaded.main.expect("main bounds should be saved");
            assert!((saved_main.x - bounds.x).abs() < 0.1);
            assert!((saved_main.y - bounds.y).abs() < 0.1);
            assert!((saved_main.width - bounds.width).abs() < 0.1);
            assert!((saved_main.height - bounds.height).abs() < 0.1);
            assert_eq!(loaded.main_per_display.len(), 1);
        });
    }

    #[test]
    fn test_save_main_position_with_display_detection_returns_suppressed_when_saving_disabled() {
        with_temp_state_dir(|| {
            suppress_save();

            let displays = vec![DisplayBounds {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 1920.0,
                height: 1080.0,
            }];
            let bounds = PersistedWindowBounds::new(100.0, 100.0, 750.0, 475.0);

            let outcome = save_main_position_with_display_detection(bounds, &displays);
            assert_eq!(outcome, MainPositionSaveOutcome::Suppressed);
            assert!(load_state_file().is_none());

            allow_save();
        });
    }
}

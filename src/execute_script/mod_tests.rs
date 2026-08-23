#[cfg(test)]
mod execute_script_session_tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn test_take_active_script_session_returns_error_when_session_missing() {
        let shared_session: SharedSession = Arc::new(ParkingMutex::new(None));

        let result = take_active_script_session(
            &shared_session,
            "example-script",
            Path::new("/tmp/example-script.ts"),
        );
        assert!(
            result.is_err(),
            "missing interactive session should be reported as an error"
        );

        let error = result.err().unwrap_or_default();

        assert!(error.contains("interactive_session_missing"));
        assert!(error.contains("script='example-script'"));
        assert!(error.contains("state=script_session:none"));
        assert!(error.contains("operation=split_interactive_session"));
    }

    #[test]
    fn test_truncate_clipboard_history_preview_returns_original_when_under_limit() {
        let content = "hello clipboard";
        let truncated = truncate_clipboard_history_preview(content);

        assert_eq!(truncated, content);
    }

    #[test]
    fn test_truncate_clipboard_history_preview_does_not_split_utf8_when_over_limit() {
        let content = format!(
            "{}😀😀",
            "a".repeat(CLIPBOARD_HISTORY_PREVIEW_CHAR_LIMIT - 1)
        );
        let truncated = truncate_clipboard_history_preview(&content);

        let expected = format!(
            "{}😀...",
            "a".repeat(CLIPBOARD_HISTORY_PREVIEW_CHAR_LIMIT - 1)
        );
        assert_eq!(truncated, expected);
    }

    #[test]
    fn test_execute_script_build_macos_beep_command_spec_uses_tink_sound() {
        let spec = execute_script_build_beep_command_spec();

        assert_eq!(spec.program, "afplay");
        assert_eq!(
            spec.args,
            vec!["/System/Library/Sounds/Tink.aiff".to_string()]
        );
    }

    #[test]
    fn test_execute_script_normalize_notify_fields_passes_through_title_and_body() {
        let normalized = execute_script_normalize_notify_fields(
            Some("Build \"Done\"".to_string()),
            Some("Line 1 \\ Line 2".to_string()),
        )
        .expect("notify payload should normalize when both fields are present");

        assert_eq!(
            normalized,
            ("Build \"Done\"".to_string(), "Line 1 \\ Line 2".to_string())
        );
    }

    #[test]
    fn test_execute_script_normalize_notify_fields_defaults_missing_fields() {
        let title_only =
            execute_script_normalize_notify_fields(Some("Build Finished".to_string()), None)
                .expect("title-only payload should fall back to reusing title as body");
        assert_eq!(
            title_only,
            ("Build Finished".to_string(), "Build Finished".to_string())
        );

        let body_only = execute_script_normalize_notify_fields(None, Some("All green".to_string()))
            .expect("body-only payload should default to the Script Kit title");
        assert_eq!(
            body_only,
            ("Script Kit".to_string(), "All green".to_string())
        );

        assert!(
            execute_script_normalize_notify_fields(None, None).is_none(),
            "notify dispatch should short-circuit when title and body are both empty"
        );
        assert!(
            execute_script_normalize_notify_fields(
                Some("   ".to_string()),
                Some("\t\n".to_string()),
            )
            .is_none(),
            "notify dispatch should short-circuit when title and body are whitespace-only"
        );
    }

    #[test]
    fn test_execute_script_build_say_command_spec_includes_voice_when_present() {
        let spec = execute_script_build_say_command_spec(
            "Hello from Script Kit".to_string(),
            Some("Samantha".to_string()),
        )
        .expect("say command should be built for non-empty text");

        assert_eq!(spec.program, "say");
        assert_eq!(
            spec.args,
            vec![
                "-v".to_string(),
                "Samantha".to_string(),
                "Hello from Script Kit".to_string(),
            ]
        );
    }
}

#[cfg(test)]
mod window_dispatch_tests {
    // NOTE: no `use super::*` — this file is include!()d beneath `use gpui::*`
    // in main.rs; the glob would shadow `#[test]` (gpui-test-macro-shadowing).

    fn refreshed_provider(fixture: &str) -> crate::window_control::provider_test_env::EnvGuard {
        crate::window_control::provider_test_env::EnvGuard::set(fixture)
    }

    #[test]
    fn missing_required_fields_keep_the_historical_error_strings() {
        let error = super::dispatch_legacy_window_action(
            &crate::protocol::WindowActionType::Focus,
            None,
            None,
            None,
        )
        .expect_err("missing id must fail");
        assert_eq!(error.to_string(), "Missing window_id");

        let error = super::dispatch_legacy_window_action(
            &crate::protocol::WindowActionType::Move,
            Some(1),
            None,
            None,
        )
        .expect_err("missing bounds must fail");
        assert_eq!(error.to_string(), "Missing window_id or bounds");

        let error = super::dispatch_legacy_window_action(
            &crate::protocol::WindowActionType::Tile,
            Some(1),
            None,
            None,
        )
        .expect_err("missing tile position must fail");
        assert_eq!(error.to_string(), "Missing window_id or tile_position");
    }

    #[test]
    fn all_21_public_tile_positions_map_to_window_control() {
        use crate::protocol::TilePosition as P;
        use crate::window_control::TilePosition as WC;
        let mappings = [
            (P::Left, WC::LeftHalf),
            (P::Right, WC::RightHalf),
            (P::Top, WC::TopHalf),
            (P::Bottom, WC::BottomHalf),
            (P::TopLeft, WC::TopLeft),
            (P::TopRight, WC::TopRight),
            (P::BottomLeft, WC::BottomLeft),
            (P::BottomRight, WC::BottomRight),
            (P::LeftThird, WC::LeftThird),
            (P::CenterThird, WC::CenterThird),
            (P::RightThird, WC::RightThird),
            (P::TopThird, WC::TopThird),
            (P::MiddleThird, WC::MiddleThird),
            (P::BottomThird, WC::BottomThird),
            (P::FirstTwoThirds, WC::FirstTwoThirds),
            (P::LastTwoThirds, WC::LastTwoThirds),
            (P::TopTwoThirds, WC::TopTwoThirds),
            (P::BottomTwoThirds, WC::BottomTwoThirds),
            (P::Center, WC::Center),
            (P::AlmostMaximize, WC::AlmostMaximize),
            (P::Maximize, WC::Fullscreen),
        ];
        assert_eq!(
            mappings.len(),
            21,
            "the public wire vocabulary is 21 strings"
        );
        for (wire, expected) in mappings {
            assert_eq!(super::protocol_tile_to_window_control(&wire), expected);
        }
    }

    #[test]
    fn dispatched_action_routes_through_the_engine_on_the_provider() {
        let _guard = refreshed_provider(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Doc","pid":9,
                 "bounds":{"x":0,"y":0,"width":800,"height":600}}
            ]}"#,
        );
        crate::window_control::refresh_window_registry().expect("refresh");
        super::dispatch_legacy_window_action(
            &crate::protocol::WindowActionType::Move,
            Some(1),
            Some(&crate::protocol::TargetWindowBounds {
                x: 42,
                y: 24,
                width: 800,
                height: 600,
            }),
            None,
        )
        .expect("move via dispatch");
    }

    #[test]
    fn protocol_display_output_equals_topology_output() {
        let _guard = refreshed_provider(
            r#"{
                "windows": [{"app":"A","title":"T"}],
                "displays": [
                    {"id": 1, "uuid": "fixture-primary", "name": "Main",
                     "fullBounds": {"x":0,"y":0,"width":1920,"height":1080},
                     "visibleBounds": {"x":0,"y":25,"width":1920,"height":1055},
                     "scaleFactor": 2.0, "isPrimary": true}
                ]
            }"#,
        );
        let protocol_displays = super::get_displays().expect("protocol displays");
        let topology = crate::window_control::list_displays().expect("topology");
        assert_eq!(protocol_displays.len(), topology.len());
        for (wire, descriptor) in protocol_displays.iter().zip(&topology) {
            assert_eq!(wire.display_id, descriptor.id.0);
            assert_eq!(wire.name, descriptor.localized_name);
            assert_eq!(wire.is_primary, descriptor.is_primary);
            assert_eq!(wire.bounds.x, descriptor.full_bounds.x);
            assert_eq!(wire.bounds.width, descriptor.full_bounds.width);
            assert_eq!(wire.visible_bounds.y, descriptor.visible_bounds.y);
            assert_eq!(wire.visible_bounds.height, descriptor.visible_bounds.height);
            assert_eq!(wire.scale_factor, Some(descriptor.backing_scale_factor));
        }
    }
}

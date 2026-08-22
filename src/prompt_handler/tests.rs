mod prompt_handler_message_tests {
    use super::{
        build_script_error_agent_chat_prompt, build_script_error_report_markdown,
        classify_prompt_message_route, convert_protocol_prompt_message,
        escape_windows_cmd_open_target, persist_script_error_agent_chat_context_bundle_in_dir,
        prompt_coming_soon_warning, prompt_message_from_protocol_message,
        resolve_ai_start_chat_provider, should_restore_main_window_after_script_exit,
        unhandled_message_warning, PromptConversionSource, PromptMessageRoute,
    };
    use crate::ai::providers::OpenAiProvider;
    use crate::PromptMessage;

    #[test]
    fn test_handle_prompt_message_routes_confirm_request_to_confirm_window() {
        let message = PromptMessage::ShowConfirm {
            id: "confirm-id".to_string(),
            message: "Continue?".to_string(),
            confirm_text: Some("Yes".to_string()),
            cancel_text: Some("No".to_string()),
        };
        assert_eq!(
            classify_prompt_message_route(&message),
            PromptMessageRoute::ConfirmDialog
        );
    }

    #[test]
    fn test_handle_prompt_message_ignores_unknown_message_without_state_corruption() {
        let message = PromptMessage::UnhandledMessage {
            message_type: "widget".to_string(),
        };
        assert_eq!(
            classify_prompt_message_route(&message),
            PromptMessageRoute::UnhandledWarning
        );

        let warning = unhandled_message_warning("widget");
        assert!(warning.contains("'widget'"));
        assert!(warning.contains("not supported yet"));
    }

    #[test]
    fn fields_protocol_message_converts_to_show_fields_for_every_source() {
        fn assert_show_fields(source: PromptConversionSource) {
            let message = crate::protocol::Message::Fields {
                id: "fields-id".to_string(),
                fields: Vec::new(),
                actions: Some(Vec::new()),
            };

            let converted = match convert_protocol_prompt_message(message, source) {
                Ok(converted) => converted,
                Err(_) => panic!("Message::Fields from {source:?} must be handled as a prompt"),
            };
            let PromptMessage::ShowFields {
                id,
                fields,
                actions,
            } = converted
            else {
                panic!(
                    "Message::Fields from {source:?} must convert to PromptMessage::ShowFields"
                );
            };

            assert_eq!(id, "fields-id");
            assert!(fields.is_empty());
            let actions = actions.expect("ShowFields must preserve the actions option");
            assert!(actions.is_empty());
        }

        for source in [
            PromptConversionSource::InteractiveSession,
            PromptConversionSource::Sessionless,
        ] {
            assert_show_fields(source);
        }
    }

    #[test]
    fn sessionless_prompt_conversion_covers_every_session_prompt_constructor() {
        use crate::protocol::Message;

        fn converted_kind(message: Message) -> &'static str {
            match prompt_message_from_protocol_message(message) {
                Some(PromptMessage::ShowArg { .. }) => "arg",
                Some(PromptMessage::ShowDiv { .. }) => "div",
                Some(PromptMessage::ShowForm { .. }) => "form",
                Some(PromptMessage::ShowFields { .. }) => "fields",
                Some(PromptMessage::ShowTerm { .. }) => "term",
                Some(PromptMessage::ShowEditor { .. }) => "editor",
                Some(PromptMessage::ShowPath { .. }) => "path",
                Some(PromptMessage::ShowEnv { .. }) => "env",
                Some(PromptMessage::ShowDrop { .. }) => "drop",
                Some(PromptMessage::ShowHotkey { .. }) => "hotkey",
                Some(PromptMessage::ShowTemplate { .. }) => "template",
                Some(PromptMessage::ShowSelect { .. }) => "select",
                Some(PromptMessage::ShowMini { .. }) => "mini",
                Some(PromptMessage::ShowMicro { .. }) => "micro",
                Some(PromptMessage::ShowConfirm { .. }) => "confirm",
                Some(PromptMessage::ShowChat { .. }) => "chat",
                Some(other) => panic!("unexpected conversion: {other:?}"),
                None => panic!("sessionless conversion dropped a session prompt constructor"),
            }
        }

        let id = || "parity-id".to_string();
        let cases = vec![
            (
                "arg",
                Message::Arg {
                    id: id(),
                    placeholder: String::new(),
                    choices: vec![],
                    actions: None,
                },
            ),
            (
                "div",
                Message::Div {
                    id: id(),
                    html: String::new(),
                    container_classes: None,
                    actions: None,
                    placeholder: None,
                    hint: None,
                    footer: None,
                    container_bg: None,
                    container_padding: None,
                    opacity: None,
                },
            ),
            (
                "form",
                Message::Form {
                    id: id(),
                    html: String::new(),
                    actions: None,
                },
            ),
            (
                "fields",
                Message::Fields {
                    id: id(),
                    fields: vec![],
                    actions: None,
                },
            ),
            (
                "term",
                Message::Term {
                    id: id(),
                    command: None,
                    actions: None,
                },
            ),
            (
                "editor",
                Message::Editor {
                    id: id(),
                    content: None,
                    language: None,
                    template: None,
                    on_init: None,
                    on_submit: None,
                    actions: None,
                },
            ),
            (
                "path",
                Message::Path {
                    id: id(),
                    start_path: None,
                    hint: None,
                },
            ),
            (
                "env",
                Message::Env {
                    id: id(),
                    key: String::new(),
                    prompt: None,
                    title: None,
                    secret: None,
                },
            ),
            ("drop", Message::Drop { id: id() }),
            (
                "hotkey",
                Message::Hotkey {
                    id: id(),
                    placeholder: None,
                },
            ),
            (
                "template",
                Message::Template {
                    id: id(),
                    template: String::new(),
                },
            ),
            (
                "select",
                Message::Select {
                    id: id(),
                    placeholder: String::new(),
                    choices: vec![],
                    multiple: None,
                },
            ),
            (
                "mini",
                Message::Mini {
                    id: id(),
                    placeholder: String::new(),
                    choices: vec![],
                },
            ),
            (
                "micro",
                Message::Micro {
                    id: id(),
                    placeholder: String::new(),
                    choices: vec![],
                },
            ),
            (
                "confirm",
                Message::Confirm {
                    id: id(),
                    message: String::new(),
                    confirm_text: None,
                    cancel_text: None,
                },
            ),
            (
                "chat",
                Message::Chat {
                    id: id(),
                    placeholder: None,
                    messages: vec![],
                    hint: None,
                    footer: None,
                    actions: None,
                    model: None,
                    models: vec![],
                    save_history: false,
                    use_builtin_ai: false,
                },
            ),
        ];

        for (expected, message) in cases {
            assert_eq!(
                converted_kind(message),
                expected,
                "{expected} conversion drifted"
            );
        }
    }

    #[test]
    fn test_unhandled_message_warning_includes_recovery_guidance() {
        let message = unhandled_message_warning("widget");
        assert!(message.contains("'widget'"));
        assert!(message.contains("Update the script to a supported message type"));
        assert!(message.contains("update Script Kit GPUI"));
    }

    #[test]
    fn test_prompt_coming_soon_warning_uses_function_style_name() {
        assert_eq!(
            prompt_coming_soon_warning("fields()"),
            "fields() prompt coming soon."
        );
    }

    #[test]
    fn test_truncate_str_chars_returns_valid_utf8_boundary_when_message_is_multibyte() {
        let message = "🙂".repeat(50);
        let truncated = crate::utils::truncate_str_chars(&message, 30);

        assert_eq!(truncated.chars().count(), 30);
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn test_escape_windows_cmd_open_target_escapes_shell_metacharacters() {
        let escaped = escape_windows_cmd_open_target(r#"https://example.com/?x=1&y=2|3"#);
        assert_eq!(escaped, r#"https://example.com/?x=1^&y=2^|3"#);
    }

    #[test]
    fn test_script_exit_restores_hidden_window_only_for_active_follow_up_ui() {
        assert!(should_restore_main_window_after_script_exit(true, true));
        assert!(!should_restore_main_window_after_script_exit(true, false));
        assert!(!should_restore_main_window_after_script_exit(false, true));
    }

    #[test]
    fn test_resolve_ai_start_chat_provider_returns_registered_provider_for_model() {
        let mut registry = crate::ai::ProviderRegistry::new();
        registry.register(std::sync::Arc::new(OpenAiProvider::new("test-key")));

        assert_eq!(
            resolve_ai_start_chat_provider(&registry, "gpt-4o"),
            Some("openai".to_string())
        );
    }

    #[test]
    fn test_resolve_ai_start_chat_provider_returns_none_for_unknown_model() {
        let mut registry = crate::ai::ProviderRegistry::new();
        registry.register(std::sync::Arc::new(OpenAiProvider::new("test-key")));

        assert_eq!(
            resolve_ai_start_chat_provider(&registry, "claude-3-5-sonnet-20241022"),
            None
        );
    }

    #[test]
    fn test_build_script_error_agent_chat_prompt_includes_fix_and_verification_guidance() {
        let prompt = build_script_error_agent_chat_prompt(
            "/tmp/failing-script.ts",
            "ReferenceError: foo is not defined",
            Some(1),
            &["Check the missing symbol".to_string()],
        );

        assert!(prompt.contains("failing-script.ts"));
        assert!(prompt.contains("fix it"));
        assert!(prompt.contains("verify the fix"));
        assert!(prompt.contains("Exit code: 1"));
        assert!(prompt.contains("Check the missing symbol"));
    }

    #[test]
    fn test_build_script_error_report_markdown_includes_all_available_sections() {
        let report = build_script_error_report_markdown(
            "/tmp/failing-script.ts",
            "ReferenceError: foo is not defined",
            Some("stderr line 1\nstderr line 2"),
            Some(1),
            Some("stack line 1\nstack line 2"),
            &["Check the missing symbol".to_string()],
        );

        assert!(report.contains("# Script Failure Report"));
        assert!(report.contains("## Script Path"));
        assert!(report.contains("## Error Summary"));
        assert!(report.contains("## Exit Code"));
        assert!(report.contains("## Suggestions"));
        assert!(report.contains("## Stderr"));
        assert!(report.contains("## Stack Trace"));
    }

    #[test]
    fn test_persist_script_error_agent_chat_context_bundle_writes_snapshot_and_report() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let script_path = temp_dir.path().join("failing-script.ts");
        std::fs::write(&script_path, "throw new Error('boom');").expect("write script");

        let bundle = persist_script_error_agent_chat_context_bundle_in_dir(
            temp_dir.path(),
            script_path.to_str().expect("utf8 path"),
            "ReferenceError: foo is not defined",
            Some("stderr output"),
            Some(1),
            Some("stack trace"),
            &["Check the missing symbol".to_string()],
        )
        .expect("persist Agent Chat context bundle");

        let script_snapshot =
            std::fs::read_to_string(&bundle.script_snapshot_path).expect("read script snapshot");
        let error_report =
            std::fs::read_to_string(&bundle.error_report_path).expect("read error report");

        assert_eq!(bundle.script_snapshot_label, "failing-script.ts");
        assert_eq!(bundle.error_report_label, "failing-script-error-report.md");
        assert_eq!(script_snapshot, "throw new Error('boom');");
        assert!(error_report.contains("ReferenceError: foo is not defined"));
        assert!(error_report.contains("stderr output"));
        assert!(error_report.contains("stack trace"));
    }
}

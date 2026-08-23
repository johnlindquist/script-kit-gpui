#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use std::collections::BTreeSet;
    use std::io::Cursor;
    use std::path::Path;
    use tempfile::TempDir;

    static PROTOCOL_VERSION_STATS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn noninteractive_policy_allows_only_hidden_read_only_and_filter_operations() {
        let safe = [
            r#"{"type":"hide"}"#,
            r#"{"type":"setFilter","text":"notes"}"#,
            r#"{"type":"getState","requestId":"state"}"#,
            r#"{"type":"getElements","requestId":"elements"}"#,
            r#"{"type":"listAutomationWindows","requestId":"windows"}"#,
            r#"{"type":"batch","requestId":"batch","commands":[{"type":"setInput","text":"notes"}]}"#,
        ];
        for payload in safe {
            assert!(
                parse_stdin_command(payload)
                    .unwrap()
                    .is_noninteractive_safe(),
                "safe hidden automation was refused: {payload}"
            );
        }

        let unsafe_payloads = [
            r#"{"type":"show"}"#,
            r#"{"type":"simulateKey","key":"enter"}"#,
            r#"{"type":"openNotes"}"#,
            r#"{"type":"captureWindow","title":"Script Kit","path":"capture.png"}"#,
            r#"{"type":"captureScreenshot","requestId":"capture"}"#,
            r#"{"type":"inspectAutomationWindow","requestId":"inspect","target":{"type":"main"}}"#,
            r#"{"type":"getState","requestId":"focused","target":{"type":"focused"}}"#,
            r#"{"type":"setAiInput","text":"hello","submit":true}"#,
            r#"{"type":"waitFor","requestId":"visible","condition":"windowVisible"}"#,
            r#"{"type":"batch","requestId":"batch","commands":[{"type":"openActions"}]}"#,
            r#"{"type":"batch","requestId":"batch","commands":[{"type":"typeAndSubmit","text":"danger"}]}"#,
            r#"{"type":"batch","requestId":"batch","commands":[{"type":"selectByValue","value":"danger","submit":true}]}"#,
        ];
        for payload in unsafe_payloads {
            assert!(
                !parse_stdin_command(payload)
                    .unwrap()
                    .is_noninteractive_safe(),
                "unsafe operator-facing automation escaped policy: {payload}"
            );
        }
    }

    /// Pins `EXTERNAL_COMMAND_VERBS` to the exhaustive
    /// [`ExternalCommand::command_type`] match: every sample variant's verb
    /// MUST appear in the slice. Adding a new variant forces both sides to
    /// grow — the match arm below is exhaustive.
    #[test]
    fn external_command_verbs_cover_every_variant() {
        let variants: Vec<ExternalCommand> = vec![
            ExternalCommand::Run {
                path: String::new(),
                request_id: None,
            },
            ExternalCommand::Show { request_id: None },
            ExternalCommand::Hide { request_id: None },
            ExternalCommand::SetFilter {
                text: String::new(),
                request_id: None,
            },
            ExternalCommand::SetMenuSyntaxFormField {
                field: None,
                value: String::new(),
                request_id: None,
            },
            ExternalCommand::TriggerBuiltin {
                builtin_id: None,
                name: None,
                request_id: None,
            },
            ExternalCommand::SimulateKey {
                key: String::new(),
                modifiers: Vec::new(),
                target: None,
                request_id: None,
            },
            ExternalCommand::OpenNotes,
            ExternalCommand::OpenAbout,
            ExternalCommand::OpenCreationFeedback {
                path: None,
                receipt_path: None,
                receipt_status: None,
                verification_status: None,
                request_id: None,
            },
            ExternalCommand::OpenConfirmPrompt {
                title: None,
                body: None,
                confirm_text: None,
                cancel_text: None,
                request_id: None,
            },
            ExternalCommand::OpenAgentChatDetachedFixture { request_id: None },
            ExternalCommand::OpenAgentChatHistoryPopupFixture { request_id: None },
            ExternalCommand::OpenChatPromptFixture { request_id: None },
            ExternalCommand::ClosePromptPopupNatively {
                target: crate::protocol::AutomationWindowTarget::Instance {
                    id: "fixture-popup".to_string(),
                    generation: 1,
                },
                request_id: None,
            },
            ExternalCommand::OpenAi,
            ExternalCommand::OpenMiniAi,
            ExternalCommand::OpenAiWithMockData,
            ExternalCommand::OpenMiniAiWithMockData,
            ExternalCommand::OpenFocusedTextAgentChatWithMockData {
                text: None,
                instruction: None,
                request_id: None,
            },
            ExternalCommand::OpenFocusedTextAgentChatFromFocusedFieldWithMockData {
                instruction: None,
                request_id: None,
            },
            ExternalCommand::OpenFocusedTextAgentChatWithPiData {
                text: None,
                instruction: None,
                request_id: None,
            },
            ExternalCommand::ShowAiCommandBar,
            ExternalCommand::SimulateAiKey {
                key: String::new(),
                modifiers: Vec::new(),
                request_id: None,
            },
            ExternalCommand::CaptureWindow {
                title: String::new(),
                path: String::new(),
                request_id: None,
            },
            ExternalCommand::SetAiSearch {
                text: String::new(),
                request_id: None,
            },
            ExternalCommand::SetAiInput {
                text: String::new(),
                submit: false,
                request_id: None,
            },
            ExternalCommand::SetAgentChatInput {
                text: String::new(),
                submit: false,
                request_id: None,
            },
            ExternalCommand::SetAgentChatScopeInput {
                text: String::new(),
                request_id: None,
            },
            ExternalCommand::SelectAgentChatVariation {
                index: 0,
                edit: false,
                request_id: None,
            },
            ExternalCommand::GetAgentChatVariations { request_id: None },
            ExternalCommand::AgentChatEscape { request_id: None },
            ExternalCommand::SetAgentChatTestFixture {
                phase: "awaitingFirstAssistantText".to_string(),
                user_text: None,
                assistant_text: None,
                message_count: None,
                request_id: None,
            },
            ExternalCommand::GetAiWindowState { request_id: None },
            ExternalCommand::ShowGrid {
                grid_size: 8,
                show_bounds: false,
                show_box_model: false,
                show_alignment_guides: false,
                show_dimensions: false,
                depth: GridDepthOption::default(),
                request_id: None,
            },
            ExternalCommand::HideGrid,
            ExternalCommand::ShowShortcutRecorder {
                command_id: String::new(),
                command_name: String::new(),
                request_id: None,
            },
            ExternalCommand::ExecuteFallback {
                fallback_id: String::new(),
                input: String::new(),
                request_id: None,
            },
            ExternalCommand::TriggerAction {
                action_id: String::new(),
                host: None,
                request_id: None,
            },
            ExternalCommand::InjectClipboardCaptureFixture {
                payload: crate::clipboard_history::ClipboardCaptureFixturePayload::Text {
                    text: String::new(),
                },
                source_bundle_id: None,
                concealed_types: Vec::new(),
                change_generation: 0,
                request_id: None,
            },
            ExternalCommand::PasteClipboardIntoAgentChat { request_id: None },
            ExternalCommand::PushDictationResult {
                transcript: String::new(),
                partial_transcript: None,
                target: None,
                freeze_only: false,
                use_frozen_selection: false,
                request_id: None,
            },
            ExternalCommand::OpenDictationOverlayFixture { request_id: None },
            ExternalCommand::OpenDictationMicrophonePopupFixture { request_id: None },
            ExternalCommand::GetConfigFingerprint { request_id: None },
            ExternalCommand::SimulateMainHotkeyGesture {
                phase: String::new(),
                request_id: None,
            },
            ExternalCommand::SetAgentChatTranscriptScroll {
                item_ix: 0,
                offset_px: 0.0,
                request_id: None,
            },
        ];

        let declared: BTreeSet<&str> = EXTERNAL_COMMAND_VERBS.iter().copied().collect();
        for variant in &variants {
            let verb = variant.command_type();
            assert!(
                declared.contains(verb),
                "verb {verb:?} produced by an ExternalCommand variant is not in EXTERNAL_COMMAND_VERBS"
            );
        }

        assert_eq!(
            declared.len(),
            EXTERNAL_COMMAND_VERBS.len(),
            "EXTERNAL_COMMAND_VERBS must be de-duplicated"
        );
        assert_eq!(
            declared.len(),
            variants.len(),
            "sample list in this test must cover every ExternalCommand verb"
        );
    }

    #[test]
    fn inject_clipboard_capture_fixture_deserializes_redacted_metadata_and_file_payload() {
        let command: ExternalCommand = serde_json::from_str(
            r#"{"type":"injectClipboardCaptureFixture","payload":{"type":"textFile","path":"/tmp/sandbox/.scriptkit/devtools-fixtures/oversize.txt"},"sourceBundleId":"com.apple.TextEdit","concealedTypes":["org.nspasteboard.ConcealedType"],"changeGeneration":42,"requestId":"clip-fixture-1"}"#,
        )
        .expect("clipboard fixture command parses");

        match command {
            ExternalCommand::InjectClipboardCaptureFixture {
                payload: crate::clipboard_history::ClipboardCaptureFixturePayload::TextFile { path },
                source_bundle_id,
                concealed_types,
                change_generation,
                request_id,
            } => {
                assert!(path.ends_with("oversize.txt"));
                assert_eq!(source_bundle_id.as_deref(), Some("com.apple.TextEdit"));
                assert_eq!(concealed_types, ["org.nspasteboard.ConcealedType"]);
                assert_eq!(change_generation, 42);
                assert_eq!(request_id.as_deref(), Some("clip-fixture-1"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn test_read_stdin_line_bounded_skips_oversized_line_and_recovers() -> anyhow::Result<()> {
        let oversized_payload = "x".repeat(20_000);
        let input = format!(
            r#"{{"type":"setFilter","text":"{}"}}
{{"type":"show"}}
"#,
            oversized_payload
        );

        let mut reader = Cursor::new(input);
        let mut byte_buffer = Vec::new();

        let first = read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)
            .context("Expected bounded line reader to process input")?;
        match first {
            StdinLineRead::TooLong { raw_len, .. } => {
                assert!(raw_len > MAX_STDIN_COMMAND_BYTES);
            }
            _ => panic!("Expected first line to be marked too long"),
        }

        let second =
            read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)
                .context("Expected second line to be readable")?;
        match second {
            StdinLineRead::Line(line) => {
                assert_eq!(line.trim_end(), r#"{"type":"show"}"#);
            }
            _ => panic!("Expected second line to be a valid command"),
        }

        Ok(())
    }

    #[test]
    fn oversized_line_with_early_request_id_builds_typed_error() -> anyhow::Result<()> {
        let input = format!(
            "{{\"requestId\":\"of19-early\",\"type\":\"setInput\",\"text\":\"{}\"}}\n",
            "x".repeat(20_000)
        );
        let mut reader = Cursor::new(input);
        let mut byte_buffer = Vec::new();
        let read = read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)?;
        let StdinLineRead::TooLong { raw, raw_len } = read else {
            panic!("expected oversized line");
        };
        assert_eq!(raw.len(), MAX_STDIN_COMMAND_BYTES);

        let response = oversized_request_error(&raw, raw_len)
            .expect("early requestId must survive in the retained prefix");
        match response {
            crate::protocol::Message::ExternalCommandResult {
                request_id,
                command,
                ok,
                error_code,
                error_message,
            } => {
                assert_eq!(request_id, "of19-early");
                assert_eq!(command, "setInput");
                assert!(!ok);
                assert_eq!(error_code.as_deref(), Some("line_too_long"));
                let message = error_message.expect("length diagnostic");
                assert!(message.contains("16384"));
                assert!(message.contains(&raw_len.to_string()));
            }
            other => panic!("expected externalCommandResult, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn oversized_line_with_late_request_id_remains_log_only() -> anyhow::Result<()> {
        let input = format!(
            "{{\"type\":\"setInput\",\"text\":\"{}\",\"requestId\":\"of19-late\"}}\n",
            "x".repeat(20_000)
        );
        let mut reader = Cursor::new(input);
        let mut byte_buffer = Vec::new();
        let read = read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)?;
        let StdinLineRead::TooLong { raw, raw_len } = read else {
            panic!("expected oversized line");
        };
        assert!(!raw.contains("of19-late"));
        assert!(oversized_request_error(&raw, raw_len).is_none());
        Ok(())
    }

    #[test]
    fn test_external_command_run_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "run", "path": "/path/to/script.ts"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::Run { path, request_id } => {
                assert_eq!(path, "/path/to/script.ts");
                assert!(request_id.is_none());
            }
            _ => panic!("Expected Run command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_run_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "run", "path": "/path/to/script.ts", "requestId": "req-123"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::Run { path, request_id } => {
                assert_eq!(path, "/path/to/script.ts");
                assert_eq!(request_id, Some("req-123".to_string().into()));
            }
            _ => panic!("Expected Run command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "show"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::Show { request_id: None }));
        Ok(())
    }

    #[test]
    fn test_external_command_show_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "show", "requestId": "req-456"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::Show { request_id } => {
                assert_eq!(request_id, Some("req-456".to_string().into()));
            }
            _ => panic!("Expected Show command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_hide_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "hide"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::Hide { request_id: None }));
        Ok(())
    }

    #[test]
    fn test_external_command_set_filter_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "setFilter", "text": "hello world"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SetFilter { text, request_id } => {
                assert_eq!(text, "hello world");
                assert!(request_id.is_none());
            }
            _ => panic!("Expected SetFilter command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_set_filter_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "setFilter", "text": "hello", "requestId": "req-789"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SetFilter { text, request_id } => {
                assert_eq!(text, "hello");
                assert_eq!(request_id, Some("req-789".to_string().into()));
            }
            _ => panic!("Expected SetFilter command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_trigger_builtin_deserialization() -> anyhow::Result<()> {
        // Deprecated `name` path still parses in v1 so the pre-v1 Bun
        // SDK keeps working until callers migrate to `builtinId`.
        let json = r#"{"type": "triggerBuiltin", "name": "clipboardHistory"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match &cmd {
            ExternalCommand::TriggerBuiltin {
                name: Some(n),
                builtin_id: None,
                ..
            } => assert_eq!(n, "clipboardHistory"),
            _ => panic!("Expected TriggerBuiltin with deprecated `name` only"),
        }
        assert_eq!(
            cmd.trigger_builtin_ref().unwrap(),
            Some(BuiltinRef::LegacyAlias("clipboardHistory"))
        );
        Ok(())
    }

    #[test]
    fn trigger_builtin_prefers_canonical_builtin_id() -> anyhow::Result<()> {
        let json = r#"{"type":"triggerBuiltin","builtinId":"builtin/clipboard-history"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(
            cmd.trigger_builtin_ref().unwrap(),
            Some(BuiltinRef::CanonicalId("builtin/clipboard-history"))
        );
        Ok(())
    }

    #[test]
    fn trigger_builtin_rejects_both_fields() {
        let json = r#"{"type":"triggerBuiltin","builtinId":"builtin/clipboard-history","name":"clipboard"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        let err = cmd.trigger_builtin_ref().unwrap_err();
        assert!(
            err.contains("either `builtinId` or deprecated `name`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn trigger_builtin_rejects_neither_field() {
        let json = r#"{"type":"triggerBuiltin"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        let err = cmd.trigger_builtin_ref().unwrap_err();
        assert!(
            err.contains("requires `builtinId`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_stdin_command_defaults_missing_protocol_version_to_v1() -> anyhow::Result<()> {
        // No `protocolVersion` field → treated as v1. Preserves
        // compatibility with the pre-v1 Bun SDK.
        let parsed = parse_stdin_command(r#"{"type":"show"}"#)?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::Show { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v1_protocol_version() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(r#"{"type":"show","protocolVersion":1}"#)?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::Show { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v2_external_command_protocol_version() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(r#"{"type":"show","protocolVersion":2}"#)?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::Show { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v2_protocol_message() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"getState","requestId":"state-v2","protocolVersion":2}"#,
        )?;
        assert!(matches!(
            parsed,
            StdinCommand::Protocol(message)
                if matches!(*message, crate::protocol::Message::GetState { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v2_trigger_builtin() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"triggerBuiltin","builtinId":"builtin/clipboard-history","protocolVersion":2}"#,
        )?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::TriggerBuiltin { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_rejects_unsupported_protocol_version_and_counts_it() {
        let _guard = PROTOCOL_VERSION_STATS_TEST_LOCK.lock().unwrap();
        crate::protocol_stats::reset_for_test();

        let err = parse_stdin_command(r#"{"type":"show","protocolVersion":999}"#)
            .expect_err("future version must be rejected");
        assert!(
            err.to_string().contains("unsupported protocolVersion"),
            "unexpected error: {err}"
        );
        assert_eq!(
            crate::protocol_stats::PROTOCOL_STATS
                .stdin_unsupported_protocol_version_total
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn parse_stdin_command_rejects_non_integer_protocol_version_without_unsupported_count() {
        let _guard = PROTOCOL_VERSION_STATS_TEST_LOCK.lock().unwrap();
        crate::protocol_stats::reset_for_test();

        let err = parse_stdin_command(r#"{"type":"show","protocolVersion":"one"}"#)
            .expect_err("non-integer protocolVersion must be rejected");
        assert!(
            err.to_string().contains("not an unsigned integer"),
            "unexpected error: {err}"
        );
        assert_eq!(
            crate::protocol_stats::PROTOCOL_STATS
                .stdin_unsupported_protocol_version_total
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn test_external_command_simulate_key_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "simulateKey", "key": "enter", "modifiers": ["cmd", "shift"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey {
                key,
                modifiers,
                target,
                ..
            } => {
                assert_eq!(key, "enter");
                assert_eq!(modifiers, vec![KeyModifier::Cmd, KeyModifier::Shift]);
                assert!(target.is_none());
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_target_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type":"simulateKey","target":{"type":"kind","kind":"notes"},"key":"p","modifiers":["cmd","shift"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey {
                key,
                modifiers,
                target,
                ..
            } => {
                assert_eq!(key, "p");
                assert_eq!(modifiers, vec![KeyModifier::Cmd, KeyModifier::Shift]);
                match target {
                    Some(protocol::AutomationWindowTarget::Kind { kind, index }) => {
                        assert_eq!(kind, protocol::AutomationWindowKind::Notes);
                        assert_eq!(index, None);
                    }
                    other => panic!("Expected targeted Notes simulateKey, got {other:?}"),
                }
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_no_modifiers() -> anyhow::Result<()> {
        let json = r#"{"type": "simulateKey", "key": "escape"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey { key, modifiers, .. } => {
                assert_eq!(key, "escape");
                assert!(modifiers.is_empty());
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_modifier_aliases() -> anyhow::Result<()> {
        let json = r#"{"type":"simulateKey","key":"k","modifiers":["meta","option","control"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey { modifiers, .. } => {
                assert_eq!(
                    modifiers,
                    vec![KeyModifier::Cmd, KeyModifier::Alt, KeyModifier::Ctrl]
                );
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_unknown_modifier_rejected() {
        let json = r#"{"type":"simulateKey","key":"enter","modifiers":["capslock"]}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_invalid_json_fails() {
        let json = r#"{"type": "unknown"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_rejects_unknown_fields() {
        let json = r#"{"type":"show","unexpected":"field"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_missing_required_field_fails() {
        // Run command requires path field
        let json = r#"{"type": "run"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_clone() {
        let cmd = ExternalCommand::Run {
            path: "/test".to_string(),
            request_id: None,
        };
        let cloned = cmd.clone();
        match cloned {
            ExternalCommand::Run { path, .. } => assert_eq!(path, "/test"),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_external_command_debug() {
        let cmd = ExternalCommand::Show { request_id: None };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Show"));
    }

    #[test]
    fn test_external_command_request_id_accessor() {
        let cmd = ExternalCommand::SetFilter {
            text: "hello".to_string(),
            request_id: Some("req-42".to_string().into()),
        };
        assert_eq!(cmd.request_id(), Some("req-42"));
    }

    #[test]
    fn test_external_command_type_accessor() {
        let cmd = ExternalCommand::Show { request_id: None };
        assert_eq!(cmd.command_type(), "show");
    }

    #[test]
    fn test_parse_stdin_command_supports_external_commands() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(r#"{"type":"show","requestId":"show-1"}"#)?;
        assert_eq!(parsed.command_type(), "show");
        assert_eq!(parsed.request_id(), Some("show-1"));
        assert!(matches!(parsed, StdinCommand::External(_)));
        Ok(())
    }

    #[test]
    fn test_parse_stdin_command_supports_protocol_messages() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"waitFor","requestId":"wf-1","condition":"choicesRendered"}"#,
        )?;
        assert_eq!(parsed.command_type(), "waitFor");
        assert_eq!(parsed.request_id(), Some("wf-1"));
        assert!(matches!(parsed, StdinCommand::Protocol(_)));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_supports_computer_see_protocol_message() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"inspectAutomationWindow","requestId":"cu-see-1","target":{"type":"focused"},"hiDpi":false,"probes":[{"x":10,"y":20}]}"#,
        )?;

        assert_eq!(parsed.command_type(), "inspectAutomationWindow");
        assert_eq!(parsed.request_id(), Some("cu-see-1"));

        match parsed {
            StdinCommand::Protocol(message) => match message.as_ref() {
                crate::protocol::Message::InspectAutomationWindow {
                    request_id,
                    target,
                    hi_dpi,
                    probes,
                } => {
                    assert_eq!(request_id, "cu-see-1");
                    assert_eq!(
                        target,
                        &Some(crate::protocol::AutomationWindowTarget::Focused)
                    );
                    assert_eq!(hi_dpi, &Some(false));
                    assert_eq!(probes, &vec![crate::protocol::PixelProbe { x: 10, y: 20 }]);
                }
                other => panic!("expected InspectAutomationWindow, got {other:?}"),
            },
            other => panic!("expected protocol message, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn parse_stdin_command_surfaces_external_command_error_for_known_verb_with_wrong_field() {
        let err = parse_stdin_command(r#"{"type":"setFilter","value":"foo"}"#)
            .expect_err("wrong field name should fail parse");
        let display = format!("{err:#}");
        assert!(
            display.contains("automation_payload_mismatch"),
            "expected context to tag the error as automation_payload_mismatch; got: {display}"
        );
        assert!(
            display.contains("\"setFilter\""),
            "expected context to name the attempted verb; got: {display}"
        );
        assert!(
            !display.contains("unknown variant `setFilter`"),
            "must NOT fall back to SDK Message error text; got: {display}"
        );
    }

    #[test]
    fn parse_stdin_command_surfaces_external_command_error_for_missing_required_field() {
        let err = parse_stdin_command(r#"{"type":"setFilter"}"#)
            .expect_err("missing required field should fail parse");
        let display = format!("{err:#}");
        assert!(
            display.contains("automation_payload_mismatch"),
            "expected automation_payload_mismatch context; got: {display}"
        );
    }

    #[test]
    fn parse_stdin_command_unknown_type_still_uses_sdk_message_fallback() {
        let err = parse_stdin_command(r#"{"type":"totallyFakeVerbXyz","foo":"bar"}"#)
            .expect_err("unknown verb should fail parse");
        let display = format!("{err:#}");
        assert!(
            !display.contains("automation_payload_mismatch"),
            "truly unknown verbs must NOT be tagged as automation_payload_mismatch; got: {display}"
        );
        assert!(
            display.contains("unknown variant"),
            "unknown verbs should surface the Message-enum unknown-variant error; got: {display}"
        );
    }

    #[test]
    fn test_external_command_open_notes_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openNotes"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenNotes));
        Ok(())
    }

    #[test]
    fn test_external_command_open_about_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openAbout"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenAbout));
        Ok(())
    }

    #[test]
    fn test_external_command_open_creation_feedback_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openCreationFeedback", "path": "/tmp/fixture.ts"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenCreationFeedback {
                path,
                receipt_path,
                receipt_status,
                verification_status,
                ..
            } => {
                assert_eq!(path.as_deref(), Some("/tmp/fixture.ts"));
                assert!(receipt_path.is_none());
                assert!(receipt_status.is_none());
                assert!(verification_status.is_none());
            }
            other => panic!("Expected OpenCreationFeedback, got: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_creation_feedback_receipt_fixture_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type": "openCreationFeedback", "path": "/tmp/fixture.ts", "receiptPath": "/tmp/fixture.scriptkit.json", "receiptStatus": "present", "verificationStatus": "blocked"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenCreationFeedback {
                path,
                receipt_path,
                receipt_status,
                verification_status,
                ..
            } => {
                assert_eq!(path.as_deref(), Some("/tmp/fixture.ts"));
                assert_eq!(receipt_path.as_deref(), Some("/tmp/fixture.scriptkit.json"));
                assert_eq!(receipt_status.as_deref(), Some("present"));
                assert_eq!(
                    verification_status,
                    Some(crate::ai::GeneratedScriptVerificationStatus::Blocked)
                );
            }
            other => panic!("Expected OpenCreationFeedback, got: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_confirm_prompt_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openConfirmPrompt", "title": "Delete?", "body": "This cannot be undone.", "confirmText": "Delete", "cancelText": "Keep"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenConfirmPrompt {
                title,
                body,
                confirm_text,
                cancel_text,
                ..
            } => {
                assert_eq!(title.as_deref(), Some("Delete?"));
                assert_eq!(body.as_deref(), Some("This cannot be undone."));
                assert_eq!(confirm_text.as_deref(), Some("Delete"));
                assert_eq!(cancel_text.as_deref(), Some("Keep"));
            }
            other => panic!("Expected OpenConfirmPrompt, got: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_ai_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openAi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenAi));
        Ok(())
    }

    #[test]
    fn test_external_command_open_mini_ai_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openMiniAi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenMiniAi));
        Ok(())
    }

    #[test]
    fn test_external_command_open_ai_with_mock_data_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openAiWithMockData"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenAiWithMockData));
        Ok(())
    }

    #[test]
    fn test_external_command_open_mini_ai_with_mock_data_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openMiniAiWithMockData"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenMiniAiWithMockData));
        Ok(())
    }

    #[test]
    fn test_external_command_open_focused_text_agent_chat_with_mock_data_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type":"openFocusedTextAgentChatWithMockData","text":"Hello world","instruction":"Translate","requestId":"ft-mock"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenFocusedTextAgentChatWithMockData {
                text,
                instruction,
                request_id,
            } => {
                assert_eq!(text.as_deref(), Some("Hello world"));
                assert_eq!(instruction.as_deref(), Some("Translate"));
                assert_eq!(request_id.as_deref(), Some("ft-mock"));
            }
            other => panic!("Expected OpenFocusedTextAgentChatWithMockData, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_focused_text_agent_chat_with_pi_data_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type":"openFocusedTextAgentChatWithPiData","text":"Hello world","instruction":"Translate","requestId":"ft-pi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenFocusedTextAgentChatWithPiData {
                text,
                instruction,
                request_id,
            } => {
                assert_eq!(text.as_deref(), Some("Hello world"));
                assert_eq!(instruction.as_deref(), Some("Translate"));
                assert_eq!(request_id.as_deref(), Some("ft-pi"));
            }
            other => panic!("Expected OpenFocusedTextAgentChatWithPiData, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_get_ai_window_state_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "getAiWindowState"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(
            cmd,
            ExternalCommand::GetAiWindowState { request_id: None }
        ));
        assert_eq!(cmd.command_type(), "getAiWindowState");
        Ok(())
    }

    #[test]
    fn test_external_command_get_ai_window_state_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "getAiWindowState", "requestId": "req-42"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.request_id(), Some("req-42"));
        Ok(())
    }

    #[test]
    fn test_external_command_set_agent_chat_input_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "setAgentChatInput", "text": "hello world", "submit": true}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.command_type(), "setAgentChatInput");
        match cmd {
            ExternalCommand::SetAgentChatInput {
                text,
                submit,
                request_id,
            } => {
                assert_eq!(text, "hello world");
                assert!(submit);
                assert!(request_id.is_none());
            }
            _ => panic!("Expected SetAgentChatInput command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_set_agent_chat_input_with_request_id() -> anyhow::Result<()> {
        let json =
            r#"{"type": "setAgentChatInput", "text": "hello", "requestId": "req-agent_chat"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.request_id(), Some("req-agent_chat"));
        Ok(())
    }

    #[test]
    fn test_external_command_set_agent_chat_test_fixture_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "setAgentChatTestFixture", "phase": "awaitingFirstAssistantText", "userText": "hello", "requestId": "req-fixture"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.command_type(), "setAgentChatTestFixture");
        assert_eq!(cmd.request_id(), Some("req-fixture"));
        match cmd {
            ExternalCommand::SetAgentChatTestFixture {
                phase,
                user_text,
                assistant_text,
                message_count,
                ..
            } => {
                assert_eq!(phase, "awaitingFirstAssistantText");
                assert_eq!(user_text.as_deref(), Some("hello"));
                assert!(assistant_text.is_none());
                assert_eq!(message_count, None);
            }
            _ => panic!("Expected SetAgentChatTestFixture command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_capture_window_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "captureWindow", "title": "Script Kit Agent Chat", "path": "/tmp/screenshot.png"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::CaptureWindow { title, path, .. } => {
                assert_eq!(title, "Script Kit Agent Chat");
                assert_eq!(path, "/tmp/screenshot.png");
            }
            _ => panic!("Expected CaptureWindow command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_grid_defaults() -> anyhow::Result<()> {
        let json = r#"{"type": "showGrid"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ShowGrid {
                grid_size,
                show_bounds,
                show_box_model,
                show_alignment_guides,
                show_dimensions,
                depth,
                ..
            } => {
                assert_eq!(grid_size, 8); // default
                assert!(!show_bounds); // default false
                assert!(!show_box_model); // default false
                assert!(!show_alignment_guides); // default false
                assert!(!show_dimensions); // default false
                assert!(matches!(depth, GridDepthOption::Preset(_))); // default
            }
            _ => panic!("Expected ShowGrid command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_grid_with_options() -> anyhow::Result<()> {
        let json = r#"{"type": "showGrid", "gridSize": 16, "showBounds": true, "showBoxModel": true, "showAlignmentGuides": true, "showDimensions": true, "depth": "all"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ShowGrid {
                grid_size,
                show_bounds,
                show_box_model,
                show_alignment_guides,
                show_dimensions,
                depth,
                ..
            } => {
                assert_eq!(grid_size, 16);
                assert!(show_bounds);
                assert!(show_box_model);
                assert!(show_alignment_guides);
                assert!(show_dimensions);
                match depth {
                    GridDepthOption::Preset(s) => assert_eq!(s, "all"),
                    _ => panic!("Expected Preset depth"),
                }
            }
            _ => panic!("Expected ShowGrid command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_grid_with_components() -> anyhow::Result<()> {
        let json = r#"{"type": "showGrid", "depth": ["header", "footer"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ShowGrid { depth, .. } => match depth {
                GridDepthOption::Components(components) => {
                    assert_eq!(components, vec!["header", "footer"]);
                }
                _ => panic!("Expected Components depth"),
            },
            _ => panic!("Expected ShowGrid command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_hide_grid_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "hideGrid"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::HideGrid));
        Ok(())
    }

    #[test]
    fn test_external_command_execute_fallback_deserialization() -> anyhow::Result<()> {
        let json =
            r#"{"type": "executeFallback", "fallbackId": "search-google", "input": "hello world"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ExecuteFallback {
                fallback_id, input, ..
            } => {
                assert_eq!(fallback_id, "search-google");
                assert_eq!(input, "hello world");
            }
            _ => panic!("Expected ExecuteFallback command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_execute_fallback_copy() -> anyhow::Result<()> {
        let json = r#"{"type": "executeFallback", "fallbackId": "copy-to-clipboard", "input": "test text"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ExecuteFallback {
                fallback_id, input, ..
            } => {
                assert_eq!(fallback_id, "copy-to-clipboard");
                assert_eq!(input, "test text");
            }
            _ => panic!("Expected ExecuteFallback command"),
        }
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_allows_dot_test_screenshots() -> anyhow::Result<()>
    {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = std::fs::canonicalize(temp.path()).context("canonicalize temp dir")?;
        let kit_root = cwd.join("kit-root");
        std::fs::create_dir_all(&kit_root).context("create kit root")?;

        let resolved = validate_capture_window_output_path_with_roots(
            ".test-screenshots/shot.png",
            &cwd,
            &kit_root,
        )
        .context("path should be accepted")?;

        assert_eq!(resolved, cwd.join(".test-screenshots/shot.png"));
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_rejects_traversal() -> anyhow::Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = temp.path();
        let kit_root = cwd.join("kit-root");
        std::fs::create_dir_all(&kit_root).context("create kit root")?;

        let error = validate_capture_window_output_path_with_roots(
            ".test-screenshots/../escape.png",
            cwd,
            &kit_root,
        )
        .err()
        .context("path traversal should be rejected")?;

        assert!(matches!(
            error,
            CaptureWindowPathPolicyError::PathOutsideAllowedRoots { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_rejects_symlink_parent() -> anyhow::Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = temp.path();
        let kit_root = cwd.join("kit-root");
        std::fs::create_dir_all(&kit_root).context("create kit root")?;

        let screenshots_root = cwd.join(".test-screenshots");
        std::fs::create_dir_all(&screenshots_root).context("create screenshots root")?;

        let outside = cwd.join("outside");
        std::fs::create_dir_all(&outside).context("create outside dir")?;

        let symlink_path = screenshots_root.join("linked");
        create_symlink(&outside, &symlink_path)?;

        let error = validate_capture_window_output_path_with_roots(
            ".test-screenshots/linked/shot.png",
            cwd,
            &kit_root,
        )
        .err()
        .context("symlink target should be rejected")?;

        assert!(matches!(
            error,
            CaptureWindowPathPolicyError::SymlinkInPath { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_allows_scriptkit_screenshots_root(
    ) -> anyhow::Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = std::fs::canonicalize(temp.path()).context("canonicalize temp dir")?;
        let kit_root = cwd.join("kit-root");
        let screenshots_root = kit_root.join("screenshots");
        std::fs::create_dir_all(&screenshots_root).context("create screenshots root")?;

        let target = screenshots_root.join("shot.png");
        let resolved = validate_capture_window_output_path_with_roots(
            target.to_string_lossy().as_ref(),
            &cwd,
            &kit_root,
        )
        .context("path should be accepted")?;

        assert_eq!(resolved, target);
        Ok(())
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
        std::os::unix::fs::symlink(target, link).context("create symlink")?;
        Ok(())
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
        std::os::windows::fs::symlink_dir(target, link).context("create symlink")?;
        Ok(())
    }

    #[test]
    fn test_external_command_open_focused_text_agent_chat_from_focused_field_with_mock_data_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type":"openFocusedTextAgentChatFromFocusedFieldWithMockData","instruction":"Translate","requestId":"ft-live"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenFocusedTextAgentChatFromFocusedFieldWithMockData {
                instruction,
                request_id,
            } => {
                assert_eq!(instruction.as_deref(), Some("Translate"));
                assert_eq!(request_id.as_deref(), Some("ft-live"));
            }
            other => panic!(
                "Expected OpenFocusedTextAgentChatFromFocusedFieldWithMockData, got {other:?}"
            ),
        }
        Ok(())
    }
}

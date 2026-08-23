#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_config_default_is_claude_code() {
        let config = HarnessConfig::default();
        assert_eq!(config.schema_version, TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION);
        assert_eq!(config.backend, HarnessBackendKind::ClaudeCode);
        assert_eq!(config.command, "claude");
        assert!(config.args.is_empty());
        assert!(config.warm_on_startup);
        // Default working_directory resolves to the Script Kit root (~/.scriptkit)
        assert!(
            config.working_directory.is_some(),
            "default working_directory should be set to scriptkit root"
        );
        let wd = config.working_directory.as_ref().unwrap();
        assert!(
            wd.contains("scriptkit") || wd.contains("script-kit"),
            "working_directory should point to scriptkit root, got: {wd}"
        );
        assert!(config.env.is_empty());
    }

    #[test]
    fn harness_config_missing_warm_on_startup_field_defaults_to_true() {
        let json = r#"{
            "schemaVersion": 1,
            "backend": "claudeCode",
            "command": "claude"
        }"#;
        let parsed: HarnessConfig = serde_json::from_str(json).expect("deserialize");
        assert!(parsed.warm_on_startup);
    }

    #[test]
    fn harness_config_explicit_false_preserves_opt_out() {
        let json = r#"{
            "schemaVersion": 1,
            "backend": "claudeCode",
            "command": "claude",
            "warmOnStartup": false
        }"#;
        let parsed: HarnessConfig = serde_json::from_str(json).expect("deserialize");
        assert!(!parsed.warm_on_startup);
    }

    #[test]
    fn harness_config_command_line_quotes_args_and_directory() {
        let config = HarnessConfig {
            schema_version: TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION,
            backend: HarnessBackendKind::Custom,
            command: "claude".to_string(),
            args: vec!["--resume".to_string(), "project with space".to_string()],
            warm_on_startup: false,
            working_directory: Some("/tmp/my dir".to_string()),
            env: std::collections::BTreeMap::from([("FOO".to_string(), "bar baz".to_string())]),
        };
        assert_eq!(
            config.command_line(),
            "cd '/tmp/my dir' && FOO='bar baz' claude --resume 'project with space'"
        );
    }

    #[test]
    fn harness_config_command_line_ignores_invalid_env_keys() {
        let config = HarnessConfig {
            schema_version: TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION,
            backend: HarnessBackendKind::Custom,
            command: "claude".to_string(),
            args: Vec::new(),
            warm_on_startup: false,
            working_directory: None,
            env: std::collections::BTreeMap::from([
                ("GOOD_KEY".to_string(), "1".to_string()),
                ("BAD-KEY".to_string(), "2".to_string()),
            ]),
        };

        assert_eq!(config.command_line(), "GOOD_KEY=1 claude");
    }

    #[test]
    fn harness_config_command_line_no_working_directory() {
        let config = HarnessConfig {
            command: "codex".to_string(),
            args: vec!["--fast".to_string()],
            working_directory: None,
            ..HarnessConfig::default()
        };
        assert_eq!(config.command_line(), "codex --fast");
    }

    #[test]
    fn harness_config_serde_roundtrip() {
        let config = HarnessConfig {
            schema_version: 1,
            backend: HarnessBackendKind::ClaudeCode,
            command: "claude".to_string(),
            args: vec!["--resume".to_string()],
            warm_on_startup: false,
            working_directory: None,
            env: std::collections::BTreeMap::new(),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: HarnessConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn harness_submission_wraps_context_and_optional_intent() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "FileSearch".to_string(),
                input_text: Some("readme".to_string()),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-29T04:39:58Z".to_string(),
        );

        // With intent (Submit mode)
        let with_intent = build_tab_ai_harness_submission(
            &context,
            Some("rename this file"),
            TabAiHarnessSubmissionMode::Submit,
            None,
            None,
            &[],
        )
        .expect("should build");
        assert!(with_intent.contains("Script Kit context"));
        assert!(with_intent.contains("prompt type: FileSearch"));
        assert!(with_intent.contains("User intent:\nrename this file"));
        assert!(!with_intent.contains("Await the user"));

        // Without intent (Submit mode) — sentinel present
        let without_intent = build_tab_ai_harness_submission(
            &context,
            None,
            TabAiHarnessSubmissionMode::Submit,
            None,
            None,
            &[],
        )
        .expect("should build");
        assert!(without_intent.contains("Script Kit context"));
        assert!(without_intent.contains("Await the user's next terminal input."));
        assert!(!without_intent.contains("User intent:"));
    }

    #[test]
    fn harness_paste_only_omits_sentinel() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-29T07:07:06Z".to_string(),
        );

        let paste = build_tab_ai_harness_submission(
            &context,
            None,
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("should build");
        assert!(paste.contains("Script Kit context"));
        assert!(!paste.contains("Await the user's next terminal input."));
        assert!(!paste.contains("User intent:"));
    }

    #[test]
    fn harness_paste_only_with_intent_still_includes_intent() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-29T07:07:06Z".to_string(),
        );

        let paste = build_tab_ai_harness_submission(
            &context,
            Some("open settings"),
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("should build");
        assert!(paste.contains("User intent:\nopen settings"));
        assert!(!paste.contains("Await the user's next terminal input."));
    }

    #[test]
    fn validate_rejects_empty_command() {
        let config = HarnessConfig {
            command: "".to_string(),
            ..HarnessConfig::default()
        };
        let err = validate_tab_ai_harness_config(&config).unwrap_err();
        assert!(err.contains("config.ts"), "must mention config file: {err}");
    }

    #[test]
    fn validate_rejects_whitespace_only_command() {
        let config = HarnessConfig {
            command: "   ".to_string(),
            ..HarnessConfig::default()
        };
        let err = validate_tab_ai_harness_config(&config).unwrap_err();
        assert!(err.contains("empty"), "must say command is empty: {err}");
    }

    #[test]
    fn validate_rejects_missing_binary() {
        let config = HarnessConfig {
            command: "nonexistent-binary-xyz-42".to_string(),
            ..HarnessConfig::default()
        };
        let err = validate_tab_ai_harness_config(&config).unwrap_err();
        assert!(
            err.contains("not found on PATH"),
            "must mention PATH: {err}"
        );
        assert!(err.contains("config.ts"), "must mention config file: {err}");
    }

    #[test]
    fn validate_accepts_known_binary() {
        // `sh` is universally available
        let config = HarnessConfig {
            command: "sh".to_string(),
            ..HarnessConfig::default()
        };
        assert!(validate_tab_ai_harness_config(&config).is_ok());
    }

    fn sample_context_with_focused_window() -> crate::ai::TabAiContextBlob {
        crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: Some("finder".to_string()),
                ..Default::default()
            },
            crate::context_snapshot::AiContextSnapshot {
                focused_window: Some(crate::context_snapshot::FocusedWindowContext {
                    title: "Finder — Downloads".to_string(),
                    width: 1440,
                    height: 900,
                    used_fallback: false,
                }),
                ..Default::default()
            },
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
            "2026-03-29T18:10:15Z".to_string(),
        )
    }

    #[test]
    fn paste_only_submission_stages_context_without_sentinel() {
        let submission = build_tab_ai_harness_submission(
            &sample_context_with_focused_window(),
            None,
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("submission");
        assert!(submission.contains("Script Kit context"));
        assert!(submission.contains("focused window title: Finder — Downloads"));
        assert!(!submission.contains("focusedWindowImage"));
        assert!(!submission.contains("Await the user's next terminal input."));
        assert!(!submission.contains("User intent:"));
    }

    #[test]
    fn submit_without_intent_appends_wait_sentinel() {
        let submission = build_tab_ai_harness_submission(
            &sample_context_with_focused_window(),
            None,
            TabAiHarnessSubmissionMode::Submit,
            None,
            None,
            &[],
        )
        .expect("submission");
        assert!(submission.contains("Await the user's next terminal input."));
    }

    #[test]
    fn paste_only_submission_ends_on_fresh_line() {
        let submission = build_tab_ai_harness_submission(
            &sample_context_with_focused_window(),
            None,
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("submission");
        assert!(
            submission.ends_with('\n'),
            "PasteOnly must leave the cursor on the next line after the context block: {submission:?}"
        );
        assert!(!submission.contains("Await the user's next terminal input."));
        assert!(!submission.contains("User intent:"));
    }

    #[test]
    fn paste_only_submission_keeps_next_user_input_separate_from_context_block() {
        let submission = build_tab_ai_harness_submission(
            &sample_context_with_focused_window(),
            None,
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("submission");
        let composed = format!("{submission}rename this file\n");
        assert!(
            composed.contains("rename this file\n"),
            "user input must start on a fresh line after the context block: {composed:?}"
        );
    }

    #[test]
    fn shell_quote_handles_edge_cases() {
        // Safe string passes through
        assert_eq!(shell_quote("hello"), "hello");
        assert_eq!(shell_quote("/usr/bin/claude"), "/usr/bin/claude");
        assert_eq!(shell_quote("FOO=bar"), "FOO=bar");

        // Spaces get quoted
        assert_eq!(shell_quote("hello world"), "'hello world'");

        // Single quotes get escaped
        assert_eq!(shell_quote("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn paste_only_submission_omits_hints_block_even_with_receipt_or_suggestions() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "FileSearch".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-29T18:10:15Z".to_string(),
        );

        let receipt = crate::ai::TabAiInvocationReceipt {
            schema_version: crate::ai::TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION,
            prompt_type: "FileSearch".to_string(),
            input_status: crate::ai::TabAiFieldStatus::Captured,
            focus_status: crate::ai::TabAiFieldStatus::Captured,
            elements_status: crate::ai::TabAiFieldStatus::Captured,
            element_count: 3,
            warning_count: 0,
            has_focus_target: true,
            has_input_text: false,
            degradation_reasons: vec![],
            rich: true,
        };

        let suggestions = vec![
            crate::ai::TabAiSuggestedIntentSpec::new("Summarize", "summarize this file"),
            crate::ai::TabAiSuggestedIntentSpec::new("Rename", "rename this file"),
        ];

        let submission = build_tab_ai_harness_submission(
            &context,
            None,
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            Some(&receipt),
            &suggestions,
        )
        .expect("submission");

        assert!(!submission.contains("<scriptKitHints>"));
        assert!(submission.contains("Script Kit context"));
        assert!(submission.ends_with('\n'));
    }

    #[test]
    fn paste_only_submission_omits_hints_block_when_no_receipt_or_suggestions() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-29T18:10:15Z".to_string(),
        );

        let submission = build_tab_ai_harness_submission(
            &context,
            None,
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("submission");

        assert!(!submission.contains("<scriptKitHints>"));
        assert!(submission.contains("Script Kit context"));
    }

    #[test]
    fn root_claude_md_documents_agent_chat_boundary_and_quick_terminal_pty_contract() {
        let doc = include_str!("../../../kit-init/ROOT_CLAUDE.md");
        assert!(
            doc.contains(
                "Command+Enter in `AppView::ScriptList` routes through the Agent Chat entry path"
            ),
            "ROOT_CLAUDE.md must document Command+Enter as the Agent Chat entry path"
        );
        assert!(
            doc.contains("`Tab` / `Shift+Tab` inside `AppView::QuickTerminalView`"),
            "ROOT_CLAUDE.md must document PTY-owned Tab handling inside QuickTerminalView"
        );
        assert!(
            !doc.contains("Plain `Tab` opens the harness terminal"),
            "ROOT_CLAUDE.md must not describe plain Tab as opening the harness terminal"
        );
        assert!(
            !doc.contains("Plain `Tab` in `AppView::ScriptList` routes through"),
            "ROOT_CLAUDE.md must not describe plain Tab as the Agent Chat entry path"
        );
        assert!(
            !doc.contains("`Shift+Tab` in `AppView::ScriptList` with non-empty filter text"),
            "ROOT_CLAUDE.md must not describe Shift+Tab in ScriptList as the default quick-submit path"
        );
    }

    #[test]
    fn standard_startup_shift_tab_no_longer_routes_into_harness_entry_intent() {
        let source = include_str!("../../app_impl/startup.rs");
        // Split at the test module boundary so assertions only inspect
        // production code, not their own string literals.
        let production = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("file has content before #[cfg(test)]");
        assert!(
            !production.contains("submit_to_current_or_new_tab_ai_harness_from_text"),
            "Shift+Tab in ScriptList must no longer route the filter text through the quick-submit planner"
        );
    }

    fn extract_tab_ai_quick_terminal_section(doc: &str) -> &str {
        let start = doc
            .find("## Tab AI — Quick Terminal with Flat Context Injection")
            .expect("doc must contain Tab AI quick terminal section");
        let rest = &doc[start..];
        let end = rest[1..]
            .find("\n## ")
            .map(|idx| idx + 1)
            .unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn agent_docs_keep_quick_terminal_section_identical() {
        const CLAUDE_DOC: &str = include_str!("../../../kit-init/ROOT_CLAUDE.md");
        const AGENTS_DOC: &str = include_str!("../../../kit-init/ROOT_AGENTS.md");
        assert_eq!(
            extract_tab_ai_quick_terminal_section(CLAUDE_DOC),
            extract_tab_ai_quick_terminal_section(AGENTS_DOC),
            "ROOT_CLAUDE.md and ROOT_AGENTS.md must keep the Tab AI quick-terminal section byte-for-byte identical"
        );
    }

    #[test]
    fn agent_docs_match_actual_lifecycle_and_submit_semantics() {
        const CLAUDE_DOC: &str = include_str!("../../../kit-init/ROOT_CLAUDE.md");
        const AGENTS_DOC: &str = include_str!("../../../kit-init/ROOT_AGENTS.md");
        for (label, text) in [
            ("ROOT_CLAUDE.md", CLAUDE_DOC),
            ("ROOT_AGENTS.md", AGENTS_DOC),
        ] {
            let section = extract_tab_ai_quick_terminal_section(text);
            assert!(
                section.contains("one-shot spawn"),
                "{label} must describe one-shot spawn lifecycle"
            );
            assert!(
                section.contains("Await the user's next terminal input."),
                "{label} must describe sentinel behavior"
            );
            assert!(
                !section.contains("First Tab press spawns the configured harness CLI in a PTY"),
                "{label} must not claim first-Tab spawn as the default lifecycle"
            );
            assert!(
                !section
                    .contains("`Shift+Tab` in `AppView::ScriptList` with non-empty filter text"),
                "{label} must not claim Shift+Tab in ScriptList is the default quick-submit entry"
            );
            assert!(
                !section.contains("Plain `Tab` opens the harness terminal"),
                "{label} must not claim plain Tab opens the harness terminal"
            );
        }
    }

    #[test]
    fn agent_docs_describe_quick_terminal_contract() {
        const CLAUDE_DOC: &str = include_str!("../../../kit-init/ROOT_CLAUDE.md");
        const AGENTS_DOC: &str = include_str!("../../../kit-init/ROOT_AGENTS.md");

        for (label, text) in [
            ("ROOT_CLAUDE.md", CLAUDE_DOC),
            ("ROOT_AGENTS.md", AGENTS_DOC),
        ] {
            assert!(
                text.contains("QuickTerminalView"),
                "{label} must mention QuickTerminalView"
            );
            assert!(
                text.contains("build_tab_ai_harness_submission"),
                "{label} must mention harness submission"
            );
            assert!(
                text.contains("CaptureContextOptions::tab_ai_submit()"),
                "{label} must mention text-safe PTY capture"
            );
            assert!(
                text.contains("claudeCode"),
                "{label} must mention claudeCode config block"
            );
            assert!(
                text.contains("claudeCode"),
                "{label} must mention claudeCode"
            );
            assert!(
                text.contains("Cmd+W"),
                "{label} must document wrapper close"
            );
            assert!(
                text.contains("Escape"),
                "{label} must mention PTY escape passthrough"
            );
            assert!(
                text.contains("Agent Chat"),
                "{label} must mention Agent Chat as the default AI chat surface"
            );
            assert!(
                !text.contains("Plain `Tab` opens the harness terminal"),
                "{label} must not describe plain Tab as the default quick terminal destination"
            );
        }
    }

    #[test]
    fn agent_docs_match_current_context_builder_contract() {
        const CLAUDE_DOC: &str = include_str!("../../../kit-init/ROOT_CLAUDE.md");
        const AGENTS_DOC: &str = include_str!("../../../kit-init/ROOT_AGENTS.md");
        for (label, text) in [
            ("ROOT_CLAUDE.md", CLAUDE_DOC),
            ("ROOT_AGENTS.md", AGENTS_DOC),
        ] {
            let section = extract_tab_ai_quick_terminal_section(text);
            assert!(
                section.contains("`build_tab_ai_context_from()`"),
                "{label} must describe the current context builder entrypoint"
            );
            assert!(
                section.contains("CaptureContextOptions::tab_ai_submit()"),
                "{label} must reference text-safe PTY capture profile"
            );
            assert!(
                !section.contains("`build_tab_ai_context()`"),
                "{label} must not mention the removed build_tab_ai_context() wording"
            );
            assert!(
                !section.contains("bundle_id + warning count"),
                "{label} must not describe the old TabAiResolvedContext shape"
            );
        }
    }

    #[test]
    fn install_time_root_claude_md_contains_current_quick_terminal_contract() {
        const ROOT_CLAUDE_DOC: &str = include_str!("../../../kit-init/ROOT_CLAUDE.md");
        assert!(
            ROOT_CLAUDE_DOC.contains("`build_tab_ai_context_from()`"),
            "ROOT_CLAUDE.md must describe the current context builder entrypoint"
        );
        assert!(
            ROOT_CLAUDE_DOC.contains("CaptureContextOptions::tab_ai_submit()"),
            "ROOT_CLAUDE.md must reference text-safe PTY capture profile"
        );
        assert!(
            ROOT_CLAUDE_DOC.contains("claudeCode"),
            "ROOT_CLAUDE.md must mention claudeCode config block"
        );
        assert!(
            ROOT_CLAUDE_DOC.contains("claudeCode"),
            "ROOT_CLAUDE.md must mention claudeCode"
        );
        assert!(
            !ROOT_CLAUDE_DOC.contains("`build_tab_ai_context()`"),
            "ROOT_CLAUDE.md must not mention the removed build_tab_ai_context() wording"
        );
        assert!(
            !ROOT_CLAUDE_DOC.contains("bundle_id + warning count"),
            "ROOT_CLAUDE.md must not describe the old TabAiResolvedContext shape"
        );
    }

    #[test]
    fn standard_startup_quick_terminal_tab_writes_directly_to_pty() {
        let source = include_str!("../../app_impl/startup.rs");
        assert!(
            source.contains("b\"\\t\""),
            "QuickTerminal must forward Tab directly to the PTY"
        );
        assert!(
            source.contains("b\"\\x1b[Z\""),
            "QuickTerminal must forward Shift+Tab/backtab directly to the PTY"
        );
        assert!(
            source.contains("term.terminal.input(bytes)"),
            "QuickTerminal Tab handling must write raw bytes to the PTY"
        );
    }

    #[test]
    fn harness_context_block_is_flat_labeled_text() {
        let blob = crate::ai::TabAiContextBlob::from_parts_with_targets(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: Some("calculate fibonacci".to_string()),
                focused_semantic_id: Some("input:filter".to_string()),
                selected_semantic_id: Some("choice:0:fibonacci-ts".to_string()),
                visible_elements: vec![],
            },
            Some(crate::ai::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:fibonacci-ts".to_string(),
                label: "fibonacci.ts".to_string(),
                metadata: None,
            }),
            vec![],
            crate::context_snapshot::AiContextSnapshot {
                selected_text: Some(
                    "function fib(n) { return n <= 1 ? n : fib(n - 1) + fib(n - 2); }".to_string(),
                ),
                frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                    pid: 42,
                    bundle_id: "com.microsoft.VSCode".to_string(),
                    name: "VS Code".to_string(),
                }),
                menu_bar_items: vec![],
                browser: Some(crate::context_snapshot::BrowserContext::from_url(
                    "https://docs.rs/gpui".to_string(),
                )),
                focused_window: Some(crate::context_snapshot::FocusedWindowContext {
                    title: "fibonacci.ts".to_string(),
                    width: 1440,
                    height: 900,
                    used_fallback: false,
                }),
                ..Default::default()
            },
            vec!["fib".to_string()],
            Some(crate::ai::TabAiClipboardContext {
                content_type: "text".to_string(),
                preview: "fn fib(n)".to_string(),
                ocr_text: None,
            }),
            vec![],
            vec![crate::ai::TabAiMemorySuggestion {
                slug: "run-fibonacci".to_string(),
                bundle_id: "com.microsoft.VSCode".to_string(),
                raw_query: "run fibonacci".to_string(),
                effective_query: "run fibonacci".to_string(),
                prompt_type: "QuickTerminal".to_string(),
                written_at: "2026-03-30T12:00:00Z".to_string(),
                score: 1.0,
            }],
            "2026-03-31T04:58:57Z".to_string(),
        )
        .with_deferred_capture_fields(
            Some(crate::ai::TabAiSourceType::RunningCommand),
            Some("/tmp/scriptkit-screenshot-abc123.png".to_string()),
            Some(crate::ai::TabAiApplyBackHint {
                action: "pasteToPrompt".to_string(),
                target_label: Some("Active prompt".to_string()),
            }),
        );

        let block = build_tab_ai_harness_context_block(&blob).expect("context block");

        assert!(block.contains("Script Kit context"));
        assert!(block.contains("prompt type: ScriptList"));
        assert!(block.contains("current input:\ncalculate fibonacci"));
        assert!(block.contains("browser url: https://docs.rs/gpui"));
        assert!(block.contains("screenshot path: /tmp/scriptkit-screenshot-abc123.png"));
        assert!(!block.contains("<scriptKitContext"));
        assert!(!block.contains("```json"));

        // Frontmost app is now separate labeled lines, not pipe-delimited
        assert!(block.contains("frontmost app name: VS Code"));
        assert!(block.contains("frontmost app bundle id: com.microsoft.VSCode"));
        assert!(block.contains("frontmost app pid: 42"));
        assert!(
            !block.contains("bundle_id="),
            "no pipe-delimited compound fields"
        );

        // Focused window is now separate labeled lines
        assert!(block.contains("focused window title: fibonacci.ts"));
        assert!(block.contains("focused window width: 1440"));
        assert!(block.contains("focused window height: 900"));
        assert!(block.contains("focused window used fallback: false"));
        assert!(
            !block.contains("used_fallback="),
            "no pipe-delimited compound fields"
        );

        // Prior automation is now separate labeled lines
        assert!(block.contains("prior automation 1 slug: run-fibonacci"));
        assert!(block.contains("prior automation 1 prompt type: QuickTerminal"));
        assert!(block.contains("prior automation 1 score: 1.000"));
        assert!(
            !block.contains("slug="),
            "no pipe-delimited compound fields"
        );
    }

    #[test]
    fn context_block_suppresses_visible_elements_when_visible_targets_exist() {
        let blob = crate::ai::TabAiContextBlob::from_parts_with_targets(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: None,
                focused_semantic_id: None,
                selected_semantic_id: None,
                visible_elements: vec![crate::protocol::ElementInfo {
                    semantic_id: "choice:0:apple".to_string(),
                    element_type: crate::protocol::ElementType::Choice,
                    text: Some("Apple".to_string()),
                    value: Some("apple".to_string()),
                    content: None,
                    selected: Some(true),
                    focused: None,
                    index: Some(0),
                    role: None,
                    kind: None,
                    source: None,
                    source_name: None,
                    selectable: None,
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                }],
            },
            None,
            vec![crate::ai::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:apple".to_string(),
                label: "Apple".to_string(),
                metadata: None,
            }],
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-31T00:00:00Z".to_string(),
        );

        let block = build_tab_ai_harness_context_block(&blob).expect("context block");

        // Visible target should be present
        assert!(
            block.contains("visible target 1 source: ScriptList"),
            "visible target should appear"
        );
        // Raw visible element should be suppressed
        assert!(
            !block.contains("visible element 1"),
            "raw visible elements must be suppressed when visible targets exist"
        );
    }

    #[test]
    fn context_block_emits_visible_elements_when_no_visible_targets() {
        let blob = crate::ai::TabAiContextBlob::from_parts_with_targets(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: None,
                focused_semantic_id: None,
                selected_semantic_id: None,
                visible_elements: vec![crate::protocol::ElementInfo {
                    semantic_id: "choice:0:banana".to_string(),
                    element_type: crate::protocol::ElementType::Choice,
                    text: Some("Banana".to_string()),
                    value: None,
                    content: None,
                    selected: None,
                    focused: None,
                    index: Some(0),
                    role: None,
                    kind: None,
                    source: None,
                    source_name: None,
                    selectable: None,
                    status_kind: None,
                    action_disabled: None,
                    style: None,
                }],
            },
            None,
            vec![], // no visible targets
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-31T00:00:00Z".to_string(),
        );

        let block = build_tab_ai_harness_context_block(&blob).expect("context block");

        assert!(
            block.contains("visible element 1 semantic id: choice:0:banana"),
            "raw visible elements should appear when no visible targets exist"
        );
    }

    // -----------------------------------------------------------------------
    // Artifact authoring guidance classifier tests
    // -----------------------------------------------------------------------

    #[test]
    fn authoring_guidance_triggers_on_verb_plus_artifact() {
        assert!(should_include_artifact_authoring_guidance(Some(
            "create a script"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "build an extension bundle"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "generate a snippet"
        )));
    }

    #[test]
    fn authoring_guidance_triggers_on_prefix_plus_artifact() {
        assert!(should_include_artifact_authoring_guidance(Some(
            "new script for clipboard"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "add a snippet"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "need a quick command"
        )));
    }

    #[test]
    fn authoring_guidance_triggers_on_bare_artifact_noun() {
        assert!(should_include_artifact_authoring_guidance(Some("snippet")));
        assert!(should_include_artifact_authoring_guidance(Some("a script")));
        assert!(should_include_artifact_authoring_guidance(Some(
            "new extension"
        )));
        assert!(should_include_artifact_authoring_guidance(Some("my agent")));
    }

    #[test]
    fn authoring_guidance_triggers_on_descriptive_artifact_phrase() {
        // Acceptance criteria: these natural asks must include guidance
        assert!(should_include_artifact_authoring_guidance(Some(
            "need a date snippet"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "PR review agent"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "new script for clipboard cleanup"
        )));
        // Other descriptive phrases ending with artifact nouns
        assert!(should_include_artifact_authoring_guidance(Some(
            "clipboard cleanup script"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "email sign-off snippet"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "quick date template"
        )));
    }

    #[test]
    fn authoring_guidance_skips_non_authoring_intents() {
        assert!(!should_include_artifact_authoring_guidance(Some(
            "rename this file"
        )));
        assert!(!should_include_artifact_authoring_guidance(Some(
            "open settings"
        )));
        assert!(!should_include_artifact_authoring_guidance(None));
        assert!(!should_include_artifact_authoring_guidance(Some("")));
    }

    #[test]
    fn authoring_guidance_triggers_on_bundle_requests() {
        assert!(should_include_artifact_authoring_guidance(Some(
            "make a bundle for quick notes"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "new bundle with two snippets"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "create a scriptlet bundle"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "new extension bundle for dates"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "snippet bundle for greetings"
        )));
    }

    #[test]
    fn authoring_guidance_skips_non_creation_bundle_intents() {
        assert!(!should_include_artifact_authoring_guidance(Some(
            "open this bundle"
        )));
        assert!(!should_include_artifact_authoring_guidance(Some(
            "edit bundle metadata"
        )));
        assert!(!should_include_artifact_authoring_guidance(Some(
            "run bundle tests"
        )));
        assert!(!should_include_artifact_authoring_guidance(Some(
            "delete the old bundle"
        )));
    }

    #[test]
    fn authoring_guidance_triggers_on_command_like_artifact_requests() {
        // Acceptance criteria from START_HERE alignment
        assert!(should_include_artifact_authoring_guidance(Some(
            "make a clipboard cleanup command"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "new jira helper"
        )));
        // Other command-like synonyms
        assert!(should_include_artifact_authoring_guidance(Some(
            "build a deployment tool"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "create a release workflow"
        )));
        assert!(should_include_artifact_authoring_guidance(Some(
            "daily standup helper"
        )));
    }

    #[test]
    fn authoring_guidance_skips_non_creation_command_like_intents() {
        // "run this command" — non-creation verb
        assert!(!should_include_artifact_authoring_guidance(Some(
            "run this command"
        )));
        // "make this command work" — "work" is not an artifact synonym,
        // and "command" is not at the end
        assert!(!should_include_artifact_authoring_guidance(Some(
            "make this command work"
        )));
        // Non-creation verbs with command-like nouns
        assert!(!should_include_artifact_authoring_guidance(Some(
            "fix this tool"
        )));
        assert!(!should_include_artifact_authoring_guidance(Some(
            "edit the helper"
        )));
        assert!(!should_include_artifact_authoring_guidance(Some(
            "delete old commands"
        )));
    }

    /// START_HERE.md was simplified ("Simplify seeded examples workspace"):
    /// the scriptlet bundle section is gone and the launchpad now ships one
    /// runnable script example.
    #[test]
    fn authoring_guidance_block_routes_through_example_starter() {
        let block = build_tab_ai_artifact_authoring_guidance_block();
        assert!(block.contains("# Script Kit Example Starter"));
        assert!(block.contains("scripts/todo-app.ts"));
    }

    #[test]
    fn authoring_guidance_block_references_exact_files() {
        let block = build_tab_ai_artifact_authoring_guidance_block();
        assert!(block.contains("--- Script Kit artifact authoring guidance ---"));
        assert!(block.contains("--- end artifact authoring guidance ---"));
        assert!(block.contains("scripts/todo-app.ts"));
        assert!(block.contains("~/.scriptkit/plugins/main/scripts/"));
        assert!(block.contains("skills/new-script/SKILL.md"));
    }

    /// The command/helper/tool decision section was removed with the
    /// simplified launchpad; the surviving load-bearing instruction is the
    /// prompt API sequencing rule (no concurrent prompt calls).
    #[test]
    fn start_here_includes_prompt_api_sequencing_rules() {
        let block = build_tab_ai_artifact_authoring_guidance_block();
        assert!(block.contains("Prompt API Sequencing"));
        assert!(block.contains("Promise.all"));
        assert!(block.contains("Do not start multiple prompts concurrently."));
    }

    #[test]
    fn start_here_omits_deprecated_gemini_cli_suffixes() {
        let block = build_tab_ai_artifact_authoring_guidance_block();
        assert!(!block.contains(".gemini.md"));
        assert!(!block.contains(".i.gemini.md"));
    }

    /// Fast Picks were removed with the simplified launchpad; the concrete
    /// paths that must survive are the authoring target, the copy-from
    /// example, and the non-interactive verification command.
    #[test]
    fn start_here_includes_concrete_authoring_paths() {
        let block = build_tab_ai_artifact_authoring_guidance_block();
        assert!(block.contains("~/.scriptkit/plugins/main/scripts/<name>.ts"));
        assert!(block.contains("~/.scriptkit/plugins/examples/scripts/todo-app.ts"));
        assert!(block.contains("SK_VERIFY=1"));
    }

    // =========================================================================
    // ScriptList submit forces artifact authoring guidance (no artifact words)
    // =========================================================================

    #[test]
    fn script_list_submit_forces_authoring_guidance_without_artifact_words() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: Some("clipboard cleanup".to_string()),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-04-04T00:00:00Z".to_string(),
        );

        let submission = build_tab_ai_harness_submission(
            &context,
            Some("clipboard cleanup"),
            TabAiHarnessSubmissionMode::Submit,
            None,
            None,
            &[],
        )
        .expect("submission");

        assert!(submission.contains("--- Script Kit artifact authoring guidance ---"));
        assert!(submission.contains("~/.scriptkit/plugins/scriptkit/skills/new-script/SKILL.md"));
        assert!(submission.contains(
            "bun build ~/.scriptkit/plugins/main/scripts/<name>.ts --target=bun --outfile ~/.scriptkit/tmp/test-scripts/<name>.verify.mjs"
        ));
        assert!(submission.contains("SK_VERIFY=1 bun ~/.scriptkit/plugins/main/scripts/<name>.ts"));
    }

    #[test]
    fn script_list_paste_only_does_not_force_authoring_guidance() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: Some("clipboard cleanup".to_string()),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-04-04T00:00:00Z".to_string(),
        );

        let submission = build_tab_ai_harness_submission(
            &context,
            Some("clipboard cleanup"),
            TabAiHarnessSubmissionMode::PasteOnly,
            None,
            None,
            &[],
        )
        .expect("submission");

        assert!(
            !submission.contains("--- Script Kit artifact authoring guidance ---"),
            "PasteOnly must not force the authoring block for non-artifact intents"
        );
    }

    #[test]
    fn script_list_submit_with_empty_intent_does_not_force_authoring_guidance() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-04-04T00:00:00Z".to_string(),
        );

        let submission = build_tab_ai_harness_submission(
            &context,
            None,
            TabAiHarnessSubmissionMode::Submit,
            None,
            None,
            &[],
        )
        .expect("submission");

        assert!(
            !submission.contains("--- Script Kit artifact authoring guidance ---"),
            "Submit with no intent must not force the authoring block"
        );
    }

    #[test]
    fn non_script_list_submit_does_not_force_authoring_guidance() {
        let context = crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "FileSearch".to_string(),
                input_text: Some("clipboard cleanup".to_string()),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-04-04T00:00:00Z".to_string(),
        );

        let submission = build_tab_ai_harness_submission(
            &context,
            Some("clipboard cleanup"),
            TabAiHarnessSubmissionMode::Submit,
            None,
            None,
            &[],
        )
        .expect("submission");

        assert!(
            !submission.contains("--- Script Kit artifact authoring guidance ---"),
            "Non-ScriptList prompt types must not force the authoring block"
        );
    }
}

// -----------------------------------------------------------------
// Source-level cleanup contract audits
//
// These tests use `include_str!` to verify that the harness-first
// cleanup contracts remain intact: lifecycle teardown, prewarm,
// fallback routing, and legacy command redirection.
// -----------------------------------------------------------------
#[cfg(test)]
mod cleanup_contract_audits {
    fn compact(text: &str) -> String {
        text.chars().filter(|ch| !ch.is_whitespace()).collect()
    }

    #[test]
    fn close_tab_ai_harness_terminal_clears_session_and_rewarms() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let start = source
            .find("fn close_tab_ai_harness_terminal_impl(")
            .expect("close_tab_ai_harness_terminal_impl should exist");
        let rest = &source[start..];
        // Scope to the next function definition so we only audit the close fn.
        let end = rest
            .find("pub(crate) fn close_tab_ai_harness_terminal_with_window(")
            .expect("close wrappers should follow close impl");
        let body = compact(&rest[..end]);

        // Close must still invalidate capture + clear apply-back for both paths.
        for needle in [
            "self.tab_ai_harness_capture_generation+=1;",
            "self.tab_ai_harness_apply_back_route=None;",
        ] {
            assert!(
                body.contains(&compact(needle)),
                "close_tab_ai_harness_terminal must contain: {needle}"
            );
        }

        // PTY teardown + prewarm are now conditional on QuickTerminalView.
        assert!(
            body.contains(&compact("if closing_quick_terminal {")),
            "close must branch PTY teardown on closing_quick_terminal"
        );
        assert!(
            body.contains("terminate_tab_ai_harness_session"),
            "close must delegate PTY teardown to terminate_tab_ai_harness_session"
        );
        assert!(
            body.contains(&compact(
                "self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);"
            )),
            "close must queue a silent fresh prewarm for the PTY path"
        );
    }

    #[test]
    fn selection_fallback_send_to_ai_opens_agent_chat() {
        let source = compact(include_str!("../../app_impl/selection_fallback.rs"));

        assert!(
            source.contains(&compact("FallbackResult::SendToAiHarness { query } =>")),
            "selection fallback must handle the harness-native send-to-ai result"
        );
        assert!(
            source.contains(&compact("self.open_tab_ai_agent_chat_with_entry_intent(")),
            "send-to-ai fallback must route to Agent Chat chat"
        );
    }

    #[test]
    fn builtin_execution_routes_generate_script_to_harness() {
        let source = compact(include_str!("../../app_execute/builtin_execution.rs"));

        assert!(
            source.contains(&compact("AiCommandType::GenerateScript =>")),
            "GenerateScript arm should exist in builtin execution"
        );
        // The generate path was refactored into `AiGenerateBuiltinAction`,
        // which derives an entry intent and submits it through the harness.
        assert!(
            source.contains(&compact(
                "self.open_tab_ai_chat_with_entry_intent(intent, cx);"
            )),
            "GenerateScript should submit through the harness"
        );
        assert!(
            !source.contains("show_script_generation_chat"),
            "builtin execution must not call the legacy script-generation chat"
        );
    }

    #[test]
    fn explicit_tab_entry_reuses_fresh_prewarm_once_then_forces_fresh() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let start = source
            .find("fn open_tab_ai_harness_terminal_from_request")
            .expect("open_tab_ai_harness_terminal_from_request should exist");
        let rest = &source[start..];
        let end = rest
            .find("fn warm_tab_ai_harness_silently")
            .expect("warm_tab_ai_harness_silently should follow open fn");
        let body = compact(&rest[..end]);

        assert!(
            body.contains("is_fresh_prewarm"),
            "explicit Tab must check for a fresh silently-prewarmed session"
        );
        assert!(
            body.contains("mark_consumed"),
            "explicit Tab must consume a fresh prewarm exactly once"
        );
        assert!(
            body.contains(&compact(
                "ensure_tab_ai_harness_terminal(!reuse_fresh_prewarm, cx)"
            )),
            "explicit Tab must reuse a fresh prewarm once, then force fresh thereafter"
        );

        // Verify the terminal becomes visible before deferred context injection.
        let view_switch = body
            .find(&compact("self.current_view=AppView::QuickTerminalView"))
            .expect("must switch to quick terminal");
        let deferred_inject = body
            .rfind(&compact("cx.spawn(async move|_this,cx|"))
            .expect("must spawn deferred injection task");
        assert!(
            view_switch < deferred_inject,
            "the terminal must become visible before deferred context injection begins"
        );
    }

    #[test]
    fn prewarm_tags_cold_start_sessions_as_fresh() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let start = source
            .find("fn warm_tab_ai_harness_silently")
            .expect("warm_tab_ai_harness_silently should exist");
        let rest = &source[start..];
        let end = rest[1..]
            .find("\n    fn ")
            .or_else(|| rest[1..].find("\n    pub"))
            .unwrap_or(rest.len());
        let body = compact(&rest[..end]);

        assert!(
            body.contains("mark_fresh_prewarm"),
            "silent prewarm must use the encapsulated mark_fresh_prewarm() helper"
        );
        assert!(
            body.contains(&compact("ensure_tab_ai_harness_terminal(false, cx)")),
            "silent prewarm must use force_fresh=false to avoid killing existing sessions"
        );
    }

    #[test]
    fn session_state_exposes_explicit_one_shot_prewarm_api() {
        let source = include_str!("mod.rs");
        assert!(
            source.contains("pub enum TabAiHarnessWarmState"),
            "session state enum must exist"
        );
        assert!(
            source.contains("FreshPrewarm"),
            "FreshPrewarm variant must exist"
        );
        assert!(source.contains("Consumed"), "Consumed variant must exist");
        assert!(
            source.contains("pub fn is_fresh_prewarm(&self) -> bool"),
            "session must expose is_fresh_prewarm()"
        );
        assert!(
            source.contains("pub fn mark_fresh_prewarm(&mut self)"),
            "session must expose mark_fresh_prewarm()"
        );
        assert!(
            source.contains("pub fn mark_consumed(&mut self)"),
            "session must expose mark_consumed()"
        );
    }

    #[test]
    fn startup_prewarm_delegates_to_silent_helper_with_opt_out() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let start = source
            .find("pub(crate) fn warm_tab_ai_harness_on_startup")
            .expect("warm_tab_ai_harness_on_startup should exist");
        let rest = &source[start..];
        let end = rest[1..]
            .find("\n    fn ")
            .or_else(|| rest[1..].find("\n    pub"))
            .unwrap_or(rest.len());
        let body = compact(&rest[..end]);

        assert!(
            body.contains(&compact("self.warm_tab_ai_harness_silently(true, cx);")),
            "startup prewarm must delegate to silent helper with respect_startup_opt_out=true"
        );
    }

    #[test]
    fn silent_prewarm_helper_uses_encapsulated_helpers_not_raw_field_writes() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let start = source
            .find("fn warm_tab_ai_harness_silently")
            .expect("warm_tab_ai_harness_silently should exist");
        let rest = &source[start..];
        let end = rest[1..]
            .find("\n    fn ")
            .or_else(|| rest[1..].find("\n    pub"))
            .unwrap_or(rest.len());
        let body = &rest[..end];

        assert!(
            !body.contains("warm_state ="),
            "silent prewarm must not directly write warm_state — use mark_fresh_prewarm() instead"
        );
        assert!(
            body.contains("mark_fresh_prewarm()"),
            "silent prewarm must use the encapsulated mark_fresh_prewarm() helper"
        );
    }

    #[test]
    fn close_path_tears_down_session_and_reprewarms() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");

        // The close fn delegates PTY teardown to the extracted helper.
        let close_body = compact(&extract_fn_body(
            source,
            "fn close_tab_ai_harness_terminal_impl(",
        ));
        assert!(
            close_body.contains("terminate_tab_ai_harness_session"),
            "close must delegate PTY teardown to terminate_tab_ai_harness_session"
        );
        assert!(
            close_body.contains(&compact(
                "self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);"
            )),
            "close must queue a silent fresh prewarm for the next Tab press"
        );

        // The extracted helper must terminate first, then clear the handle on success.
        let helper_body = compact(&extract_fn_body(
            source,
            "fn terminate_tab_ai_harness_session",
        ));
        assert!(
            helper_body.contains(&compact("self.tab_ai_harness.as_ref()")),
            "terminate helper must read the harness session before attempting shutdown"
        );
        assert!(
            helper_body.contains("terminate_session"),
            "terminate helper must kill the PTY"
        );
        assert!(
            helper_body.contains(&compact("self.tab_ai_harness = None;")),
            "terminate helper must clear the harness handle after successful shutdown"
        );
    }

    #[test]
    fn prompt_ai_dispatch_routes_script_generation_to_harness() {
        let source = compact(include_str!("../../app_impl/prompt_ai.rs"));

        assert!(
            source.contains(&compact(
                "pub(crate) fn dispatch_ai_script_generation_from_query("
            )),
            "dispatch_ai_script_generation_from_query should exist"
        );
        assert!(
            source.contains(&compact(
                "self.open_tab_ai_chat_with_entry_intent_suppressing_focused_part(Some(query), cx);"
            )),
            "dispatch_ai_script_generation_from_query must route to the harness"
        );
        assert!(
            !source.contains(&compact("show_script_generation_chat()")),
            "dispatch_ai_script_generation_from_query must not call the legacy chat"
        );
    }

    #[test]
    fn force_fresh_path_propagates_terminate_failures() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let start = source
            .find("fn ensure_tab_ai_harness_terminal")
            .expect("ensure_tab_ai_harness_terminal should exist");
        let rest = &source[start..];
        let end = rest[1..]
            .find("\n    fn ")
            .or_else(|| rest[1..].find("\n    pub"))
            .unwrap_or(rest.len());
        let body = compact(&rest[..end]);

        // The force-fresh path must propagate terminate failures via `?`
        // instead of silently discarding them with `let _ = ...`.
        assert!(
            body.contains(&compact(
                "existing.entity.update(cx, |term, _cx| { term.terminate_session().map_err(|e| e.to_string()) })?;"
            )),
            "force-fresh path must propagate terminate_session failures with `?`"
        );
        assert!(
            !body.contains(&compact("let _ = existing.entity.update")),
            "force-fresh path must not discard terminate failures"
        );
        // Handle must NOT be cleared before terminate succeeds.
        assert!(
            !body.contains(&compact("self.tab_ai_harness.take()")),
            "force-fresh path must not use .take() which clears the handle before terminate"
        );
    }

    // ── Acceptance-criteria contract tests ──────────────────────

    fn extract_fn_body(source: &str, signature: &str) -> String {
        let start = source.find(signature).expect("signature must exist");
        let rest = &source[start..];
        let open = rest.find('{').expect("function body must open");
        let mut depth = 0usize;
        let mut end = None;
        for (idx, ch) in rest[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + idx + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        rest[..end.expect("function body must close")].to_string()
    }

    #[test]
    fn tab_ai_open_path_reuses_fresh_prewarm_once_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let body = compact(&extract_fn_body(
            source,
            "fn open_tab_ai_harness_terminal_from_request",
        ));

        assert!(
            body.contains("is_fresh_prewarm"),
            "explicit Tab must check for a fresh silently-prewarmed session"
        );
        assert!(
            body.contains("mark_consumed"),
            "explicit Tab must consume a fresh prewarm exactly once"
        );
        assert!(
            body.contains(&compact(
                "ensure_tab_ai_harness_terminal(!reuse_fresh_prewarm, cx)"
            )),
            "explicit Tab must reuse a fresh prewarm once, then force fresh thereafter"
        );
    }

    #[test]
    fn force_fresh_path_clears_session_only_after_successful_terminate_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let body = compact(&extract_fn_body(
            source,
            "fn ensure_tab_ai_harness_terminal",
        ));

        let terminate_pos = body
            .find(&compact(
                "existing.entity.update(cx, |term, _cx| { term.terminate_session().map_err(|e| e.to_string()) })?;"
            ))
            .expect("terminate_session call must exist in force-fresh path");

        let clear_pos = body
            .find(&compact("self.tab_ai_harness = None;"))
            .expect("session clear must exist after terminate success");

        assert!(
            terminate_pos < clear_pos,
            "force-fresh path must clear self.tab_ai_harness only after terminate_session succeeds"
        );
    }

    #[test]
    fn tab_ai_silent_prewarm_is_marked_fresh_on_cold_start_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let body = compact(&extract_fn_body(source, "fn warm_tab_ai_harness_silently"));
        assert!(
            body.contains(&compact("if was_cold_start {")),
            "silent prewarm helper must gate FreshPrewarm tagging on a newly created session"
        );
        assert!(
            body.contains(&compact("session.mark_fresh_prewarm();")),
            "cold-started prewarm must be marked reusable once"
        );
        assert!(
            body.contains(&compact("self.ensure_tab_ai_harness_terminal(false, cx)")),
            "silent prewarm helper must never force-fresh kill an existing live session"
        );
    }

    #[test]
    fn tab_ai_close_path_reseeds_future_prewarm_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let body = compact(&extract_fn_body(
            source,
            "fn close_tab_ai_harness_terminal_impl(",
        ));
        assert!(
            body.contains("terminate_tab_ai_harness_session"),
            "close path must delegate PTY session teardown"
        );
        assert!(
            body.contains(&compact(
                "self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);"
            )),
            "close path must schedule a fresh prewarm for the next Tab press"
        );
        // Agent Chat close must NOT schedule prewarm.
        assert!(
            body.contains(&compact("if closing_quick_terminal {")),
            "prewarm must be conditional on closing_quick_terminal"
        );
    }

    #[test]
    fn tab_ai_open_path_switches_view_before_waiting_for_capture_contract() {
        let body = extract_fn_body(
            include_str!("../../app_impl/agent_handoff/mod.rs"),
            "fn open_tab_ai_harness_terminal_from_request",
        );

        let view_switch = body
            .find("self.current_view = AppView::QuickTerminalView")
            .expect("QuickTerminalView switch must exist");

        // Find the cx.notify() that comes AFTER the view switch (not the
        // error-path notify that precedes it).
        let notify = body[view_switch..]
            .find("cx.notify()")
            .map(|offset| view_switch + offset)
            .expect("cx.notify() must follow the view switch");

        let capture_wait = body
            .find("capture_rx.recv().await")
            .expect("deferred capture await must exist");

        assert!(
            view_switch < notify,
            "the harness view must be selected before notifying the UI"
        );
        assert!(
            notify < capture_wait,
            "the terminal must become visible before waiting for deferred capture"
        );
    }

    // ── Post-close prewarm split contracts ─────────────────────

    #[test]
    fn post_close_prewarm_uses_dedicated_helper_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let schedule_body = compact(&extract_fn_body(
            source,
            "fn schedule_tab_ai_harness_prewarm",
        ));

        assert!(
            schedule_body.contains(&compact("this.warm_tab_ai_harness_after_close(cx);")),
            "close-cycle scheduler must call warm_tab_ai_harness_after_close()"
        );
        assert!(
            !schedule_body.contains(&compact("this.warm_tab_ai_harness_on_startup(cx);")),
            "close-cycle scheduler must not route through startup-only prewarm"
        );
    }

    #[test]
    fn startup_and_post_close_prewarm_split_opt_out_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");

        let startup_body = compact(&extract_fn_body(
            source,
            "pub(crate) fn warm_tab_ai_harness_on_startup",
        ));
        assert!(
            startup_body.contains(&compact("self.warm_tab_ai_harness_silently(true, cx);")),
            "startup prewarm must continue respecting warmOnStartup=false via true arg"
        );

        let after_close_body = compact(&extract_fn_body(
            source,
            "fn warm_tab_ai_harness_after_close",
        ));
        assert!(
            after_close_body.contains(&compact("self.warm_tab_ai_harness_silently(false, cx);")),
            "post-close prewarm must bypass the startup-only opt-out via false arg"
        );
    }

    #[test]
    fn silent_prewarm_helper_still_marks_cold_start_as_fresh_contract() {
        let source = include_str!("../../app_impl/agent_handoff/mod.rs");
        let body = compact(&extract_fn_body(source, "fn warm_tab_ai_harness_silently"));

        assert!(
            body.contains(&compact("if was_cold_start {")),
            "silent prewarm helper must still gate fresh tagging on newly created sessions"
        );
        assert!(
            body.contains(&compact("session.mark_fresh_prewarm();")),
            "silent prewarm helper must still mark cold-started sessions as reusable once"
        );
        assert!(
            body.contains(&compact("self.ensure_tab_ai_harness_terminal(false, cx)")),
            "silent prewarm helper must never force-fresh kill an existing live session"
        );
    }

    // ── Source/apply-back provenance unification contracts ─────

    // ── Screenshot helper & builtin registry audits ────────────

    const SCREENSHOT_FILES_SOURCE: &str = include_str!("screenshot_files.rs");
    const BUILTINS_SOURCE: &str = include_str!("../../builtins/mod.rs");

    #[test]
    fn full_screen_capture_helper_contract_is_preserved() {
        assert!(
            SCREENSHOT_FILES_SOURCE.contains("pub fn capture_tab_ai_screen_screenshot_file()"),
            "full-screen screenshot helper must exist as a public function",
        );
        assert!(
            SCREENSHOT_FILES_SOURCE.contains("capture_screen_screenshot()"),
            "full-screen screenshot helper must call the platform full-screen screenshot API",
        );
        assert!(
            SCREENSHOT_FILES_SOURCE.contains("cleanup_old_tab_ai_screenshot_files"),
            "full-screen screenshot helper must clean up old screenshot temp files",
        );
        assert!(
            SCREENSHOT_FILES_SOURCE.contains("TAB_AI_SCREENSHOT_MAX_KEEP"),
            "full-screen screenshot helper must use the shared screenshot retention limit",
        );
        assert!(
            SCREENSHOT_FILES_SOURCE.contains("title: \"Full Screen\".to_string()"),
            "full-screen screenshot helper must label the artifact as Full Screen",
        );
        assert!(
            SCREENSHOT_FILES_SOURCE.contains("used_fallback: false"),
            "full-screen screenshot helper must set used_fallback to false",
        );
    }

    #[test]
    fn builtin_registry_keeps_harness_entries_and_manual_paths_only() {
        let fn_start = BUILTINS_SOURCE
            .find("pub fn get_builtin_entries(")
            .expect("get_builtin_entries must exist");
        let fn_body = &BUILTINS_SOURCE[fn_start..];
        let fn_end = fn_body.find("\n#[cfg(test)]").unwrap_or(fn_body.len());
        let registration_section = &fn_body[..fn_end];

        for legacy_id in [
            "builtin/open-ai-chat",
            "builtin/mini-ai-chat",
            "builtin/new-conversation",
            "builtin/clear-conversation",
            "builtin/send-screen-area-to-ai",
        ] {
            let quoted = format!("\"{}\"", legacy_id);
            assert!(
                !registration_section.contains(&quoted),
                "{legacy_id} must not be registered in the main builtin list",
            );
        }

        for kept_id in [
            "builtin/generate-script-with-ai",
            "builtin/generate-script-from-current-app",
            "builtin/send-screen-to-ai",
            "builtin/send-selected-text-to-ai",
            "builtin/send-browser-tab-to-ai",
            "builtin/new-script",
            "builtin/new-extension",
        ] {
            let quoted = format!("\"{}\"", kept_id);
            assert!(
                registration_section.contains(&quoted),
                "{kept_id} must stay registered in the main builtin list",
            );
        }
    }

    #[test]
    fn focused_window_builtin_uses_canonical_id() {
        let fn_start = BUILTINS_SOURCE
            .find("pub fn get_builtin_entries(")
            .expect("get_builtin_entries must exist");
        let fn_body = &BUILTINS_SOURCE[fn_start..];
        let fn_end = fn_body.find("\n#[cfg(test)]").unwrap_or(fn_body.len());
        let registration_section = &fn_body[..fn_end];

        assert!(
            registration_section.contains("\"builtin/send-focused-window-to-ai\""),
            "SendFocusedWindowToAi must use the canonical focused-window builtin id",
        );
        assert!(
            !registration_section.contains("\"builtin/send-window-to-ai\""),
            "legacy short focused-window builtin id must not remain in the main builtin list",
        );
    }

    #[test]
    fn detect_source_type_delegates_to_canonical_function() {
        let source = include_str!("../../app_impl/agent_handoff/source_classification.rs");
        let body = compact(&extract_fn_body(source, "fn detect_tab_ai_source_type("));

        assert!(
            body.contains(&compact(
                "crate::ai::detect_tab_ai_source_type_from_prompt("
            )),
            "detect_tab_ai_source_type must delegate to canonical crate::ai function"
        );
        assert!(
            body.contains(&compact("app_view_to_prompt_type_str(source_view),")),
            "detect_tab_ai_source_type must convert AppView via app_view_to_prompt_type_str"
        );
    }

    #[test]
    fn build_apply_back_hint_delegates_to_canonical_function() {
        let source = include_str!("../../app_impl/agent_handoff/source_classification.rs");
        let body = compact(&extract_fn_body(source, "fn build_tab_ai_apply_back_hint("));

        assert!(
            body.contains(&compact(
                "crate::ai::build_tab_ai_apply_back_hint_from_source(source_type)"
            )),
            "build_tab_ai_apply_back_hint must delegate to canonical crate::ai function"
        );
    }

    // ── Cached guidance block and marker detection tests ─────────────

    #[test]
    fn guidance_block_is_cached_across_calls() {
        let first = super::build_tab_ai_artifact_authoring_guidance_block();
        let second = super::build_tab_ai_artifact_authoring_guidance_block();
        // Same &'static str means same allocation — LazyLock is doing its job.
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn verification_markers_detect_all_three_in_guidance_block() {
        let guidance = super::build_tab_ai_artifact_authoring_guidance_block();
        let markers = super::TabAiVerificationGuidanceMarkers::from_guidance(guidance);
        assert!(
            markers.includes_script_authoring_skill,
            "cached guidance must reference the new-script skill"
        );
        assert!(
            markers.includes_bun_build_verification,
            "cached guidance must reference the bun build command"
        );
        assert!(
            markers.includes_bun_execute_verification,
            "cached guidance must reference the SK_VERIFY bun execute command"
        );
    }

    #[test]
    fn verification_markers_are_all_false_for_non_authoring_text() {
        let markers = super::TabAiVerificationGuidanceMarkers::from_guidance("rename this file");
        assert!(!markers.includes_script_authoring_skill);
        assert!(!markers.includes_bun_build_verification);
        assert!(!markers.includes_bun_execute_verification);
    }

    #[test]
    fn agent_chat_initial_input_authoring_case_appends_guidance_with_all_markers_true() {
        let input = super::build_tab_ai_agent_chat_initial_input_for_prompt(
            "ScriptList",
            "clipboard cleanup",
        );
        assert!(input.guidance_appended);
        assert!(input.forced_by_script_list_submit);
        assert_eq!(input.artifact_kind, Some(super::TabAiArtifactKind::Script));
        assert!(input.use_quick_terminal);
        assert!(input.includes_script_authoring_skill);
        assert!(input.includes_bun_build_verification);
        assert!(input.includes_bun_execute_verification);
        assert!(input
            .text
            .starts_with("--- Script Kit artifact authoring guidance ---"));
        assert!(input.text.contains("User intent:\nclipboard cleanup"));
    }

    #[test]
    fn agent_chat_initial_input_non_authoring_case_omits_guidance_with_all_markers_false() {
        let input = super::build_tab_ai_agent_chat_initial_input_for_prompt(
            "FileSearch",
            "rename this file",
        );
        assert!(!input.guidance_appended);
        assert!(!input.forced_by_script_list_submit);
        assert!(input.artifact_kind.is_none());
        assert!(!input.use_quick_terminal);
        assert!(!input.includes_script_authoring_skill);
        assert!(!input.includes_bun_build_verification);
        assert!(!input.includes_bun_execute_verification);
        assert_eq!(input.text, "rename this file");
    }

    #[test]
    fn agent_chat_initial_input_agent_intent_does_not_use_quick_terminal() {
        let input = super::build_tab_ai_agent_chat_initial_input_for_prompt(
            "ScriptList",
            "review PR agent",
        );
        assert!(input.guidance_appended);
        assert_eq!(input.artifact_kind, Some(super::TabAiArtifactKind::Agent));
        assert!(
            !input.use_quick_terminal,
            "Agent artifacts must not route to quick terminal"
        );
    }

    #[test]
    fn appendix_builder_returns_static_str_not_fresh_allocation() {
        let first = super::build_tab_ai_artifact_authoring_appendix_for_prompt(
            "ScriptList",
            Some("clipboard cleanup"),
            super::TabAiHarnessSubmissionMode::Submit,
        );
        let second = super::build_tab_ai_artifact_authoring_appendix_for_prompt(
            "ScriptList",
            Some("make a snippet"),
            super::TabAiHarnessSubmissionMode::Submit,
        );
        // Both should return the same &'static str pointer.
        assert!(std::ptr::eq(
            first.unwrap().guidance,
            second.unwrap().guidance,
        ));
    }

    // ── Surface-preference helper tests ──────────────────────────────

    #[test]
    fn surface_preference_script_list_submit_uses_quick_terminal() {
        let pref = super::tab_ai_surface_preference_for_prompt(
            "ScriptList",
            Some("clipboard cleanup"),
            super::TabAiHarnessSubmissionMode::Submit,
        );
        assert!(
            pref.use_quick_terminal,
            "script authoring flow must prefer quick terminal"
        );
        assert!(pref.includes_script_authoring_skill);
        assert!(pref.includes_bun_build_verification);
        assert!(pref.includes_bun_execute_verification);
    }

    #[test]
    fn surface_preference_non_authoring_stays_agent_chat() {
        let pref = super::tab_ai_surface_preference_for_prompt(
            "FileSearch",
            Some("rename this file"),
            super::TabAiHarnessSubmissionMode::Submit,
        );
        assert!(
            !pref.use_quick_terminal,
            "non-authoring flow must stay on Agent Chat"
        );
        assert!(!pref.includes_script_authoring_skill);
        assert!(!pref.includes_bun_build_verification);
        assert!(!pref.includes_bun_execute_verification);
    }

    #[test]
    fn surface_preference_no_appendix_returns_all_false() {
        // PasteOnly on a non-ScriptList prompt with no artifact words → no appendix
        let pref = super::tab_ai_surface_preference_for_prompt(
            "FileSearch",
            Some("hello"),
            super::TabAiHarnessSubmissionMode::PasteOnly,
        );
        assert!(!pref.use_quick_terminal);
        assert!(!pref.includes_script_authoring_skill);
        assert!(!pref.includes_bun_build_verification);
        assert!(!pref.includes_bun_execute_verification);
    }

    #[test]
    fn surface_preference_none_intent_returns_all_false() {
        let pref = super::tab_ai_surface_preference_for_prompt(
            "ScriptList",
            None,
            super::TabAiHarnessSubmissionMode::Submit,
        );
        assert!(!pref.use_quick_terminal);
        assert!(!pref.includes_script_authoring_skill);
        assert!(!pref.includes_bun_build_verification);
        assert!(!pref.includes_bun_execute_verification);
    }

    // ── Acceptance-criteria: shared appendix builder contract ───────────

    #[test]
    fn harness_submission_builder_appends_guidance_for_script_list_submit() {
        let appendix = super::build_tab_ai_artifact_authoring_appendix_for_prompt(
            "ScriptList",
            Some("clipboard cleanup"),
            super::TabAiHarnessSubmissionMode::Submit,
        )
        .expect("ScriptList + Submit + non-empty intent must produce an appendix");

        assert_eq!(
            appendix.artifact_kind,
            Some(super::TabAiArtifactKind::Script),
            "ScriptList + Submit + terse intent must resolve to Script"
        );
        assert!(appendix.use_quick_terminal);
        assert!(appendix.forced_by_script_list_submit);
        assert!(appendix.has_script_verification_gate_header);
        assert!(appendix.markers.includes_script_authoring_skill);
        assert!(appendix.markers.includes_bun_build_verification);
        assert!(appendix.markers.includes_bun_execute_verification);
        assert!(
            appendix.guidance.contains("MANDATORY SCRIPT VERIFICATION"),
            "guidance must include the verification gate header"
        );
        assert!(
            appendix
                .guidance
                .contains("SK_VERIFY=1 bun ~/.scriptkit/plugins/main/scripts/<name>.ts"),
            "guidance must include the SK_VERIFY bun run command"
        );
    }

    #[test]
    fn agent_intent_appendix_has_agent_kind_and_no_quick_terminal() {
        let appendix = super::build_tab_ai_artifact_authoring_appendix_for_prompt(
            "ScriptList",
            Some("review PR agent"),
            super::TabAiHarnessSubmissionMode::Submit,
        )
        .expect("ScriptList + Submit + agent intent must produce an appendix");

        assert_eq!(
            appendix.artifact_kind,
            Some(super::TabAiArtifactKind::Agent),
            "agent keyword must resolve to Agent kind"
        );
        assert!(
            !appendix.use_quick_terminal,
            "Agent artifacts must not route to quick terminal"
        );
        assert!(appendix.forced_by_script_list_submit);
    }

    #[test]
    fn authoring_submission_includes_all_verification_markers() {
        // Non-authoring prompt must NOT produce an appendix.
        let none_appendix = super::build_tab_ai_artifact_authoring_appendix_for_prompt(
            "FileSearch",
            Some("rename this file"),
            super::TabAiHarnessSubmissionMode::Submit,
        );
        assert!(
            none_appendix.is_none(),
            "FileSearch + non-script-creation intent must not produce an appendix"
        );

        // Script creation prompts must produce appendix with all verification markers.
        let appendix = super::build_tab_ai_artifact_authoring_appendix_for_prompt(
            "ScriptList",
            Some("clipboard cleanup"),
            super::TabAiHarnessSubmissionMode::Submit,
        )
        .expect("script creation appendix");
        assert!(appendix.markers.includes_script_authoring_skill);
        assert!(appendix.markers.includes_bun_build_verification);
        assert!(appendix.markers.includes_bun_execute_verification);
        assert!(appendix.has_script_verification_gate_header);
        assert!(appendix.use_quick_terminal);
    }
}

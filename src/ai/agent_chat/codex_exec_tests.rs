#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::content::{ImageContent, TextContent};

    fn fake_active_startup(generation: u64) -> ActiveExecTurn {
        ActiveExecTurn {
            generation,
            pid: 42,
            pgid: 42,
            cancel_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn startup_failure_requires_exact_generation_cleanup() {
        let pending = plan_quick_ai_startup_cleanup(7, Some(7), true, false, false);
        assert!(pending.terminate_owned_group);
        assert!(!pending.remove_owned_scratch);
        assert!(!pending.release_active_turn);

        let verified = plan_quick_ai_startup_cleanup(7, Some(7), true, true, false);
        assert!(!verified.terminate_owned_group);
        assert!(verified.remove_owned_scratch);
        assert!(verified.release_active_turn);
    }

    #[test]
    fn failed_startup_never_blocks_retry_after_verified_cleanup() {
        let mut turns = HashMap::from([("chat".to_owned(), fake_active_startup(4))]);

        assert!(!release_owned_quick_ai_turn(&mut turns, "chat", 4, false));
        assert!(turns.contains_key("chat"));
        assert!(release_owned_quick_ai_turn(&mut turns, "chat", 4, true));
        assert!(!turns.contains_key("chat"));

        turns.insert("chat".to_owned(), fake_active_startup(5));
        assert_eq!(turns.get("chat").map(|turn| turn.generation), Some(5));
    }

    #[test]
    fn stale_startup_cleanup_cannot_remove_newer_turn() {
        let mut turns = HashMap::from([("chat".to_owned(), fake_active_startup(8))]);

        assert!(!release_owned_quick_ai_turn(&mut turns, "chat", 7, true));
        assert_eq!(turns.get("chat").map(|turn| turn.generation), Some(8));
    }

    #[test]
    fn worker_ownership_transfer_disarms_startup_cleanup() {
        let plan = plan_quick_ai_startup_cleanup(7, Some(7), true, false, true);

        assert!(!plan.terminate_owned_group);
        assert!(!plan.remove_owned_scratch);
        assert!(!plan.release_active_turn);
    }

    #[test]
    fn startup_reservation_is_atomic_and_releases_before_retry() {
        let turns = Arc::new(Mutex::new(HashMap::new()));
        let first = QuickAiStartupGuard::reserve(
            Arc::clone(&turns),
            "chat".to_owned(),
            7,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("first pure reservation succeeds");

        assert!(QuickAiStartupGuard::reserve(
            Arc::clone(&turns),
            "chat".to_owned(),
            8,
            Arc::new(AtomicBool::new(false)),
        )
        .is_err());

        drop(first);
        assert!(turns.lock().unwrap().is_empty());

        let retry = QuickAiStartupGuard::reserve(
            Arc::clone(&turns),
            "chat".to_owned(),
            9,
            Arc::new(AtomicBool::new(false)),
        );
        assert!(retry.is_ok());
    }

    fn accumulator() -> CodexExecTurnAccumulator {
        CodexExecTurnAccumulator::new("test".to_string())
    }

    #[cfg(unix)]
    fn fake_codex(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("fake-codex");
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn turn_request() -> AgentChatTurnRequest {
        AgentChatTurnRequest {
            ui_thread_id: "quick-ai-test-thread".into(),
            cwd: PathBuf::from("/must/not/be/used"),
            blocks: vec![ContentBlock::Text(TextContent::new("latest Rust"))],
            model_id: Some(QUICK_AI_SELECTED_MODEL_ID.into()),
            tool_policy:
                crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy::WebSearchOnly,
        }
    }

    fn apply_line(
        acc: &mut CodexExecTurnAccumulator,
        line: &str,
    ) -> Result<Vec<AgentChatEvent>, CodexTurnFailure> {
        match apply_line_decision(acc, line)? {
            CodexEventDecision::Continue(events) => Ok(events),
            CodexEventDecision::CompleteEarly(_) => Err(CodexTurnFailure::protocol(
                "unexpected_early_completion_in_test",
            )),
            CodexEventDecision::StopForRecovery(_) => Err(CodexTurnFailure::protocol(
                "unexpected_typed_recovery_in_test",
            )),
        }
    }

    fn apply_line_decision(
        acc: &mut CodexExecTurnAccumulator,
        line: &str,
    ) -> Result<CodexEventDecision, CodexTurnFailure> {
        apply_codex_exec_event(acc, parse_codex_exec_line(line).unwrap())
    }

    #[test]
    fn codex_quick_ai_command_matches_measured_benchmark_contract() {
        let dir = tempfile::tempdir().unwrap();
        let spec = CodexQuickAiExecSpec::from_builtin_contract(dir.path().to_path_buf());
        let command = build_codex_exec_command(&spec, dir.path(), "query").unwrap();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.first().map(String::as_str), Some("--search"));
        assert_eq!(args.last().map(String::as_str), Some("query"));
        for required in [
            "--disable",
            "plugins",
            "skills.bundled.enabled=false",
            "model_reasoning_effort=\"low\"",
            "tools.web_search.context_size=\"low\"",
            // Quick AI's only allowed tool is web_search; every other tool
            // has to be removed from the turn, not rejected after it runs.
            "features.shell_tool=false",
            "mcp_servers={}",
            "features.enable_mcp_apps=false",
            "features.apps=false",
            "features.tool_search=false",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--output-schema",
            "--json",
        ] {
            assert!(args.contains(&required.to_string()));
        }
    }

    #[test]
    fn codex_quick_ai_requires_exact_model_and_single_text_block() {
        let request = AgentChatTurnRequest {
            ui_thread_id: "t".into(),
            cwd: PathBuf::from("/tmp"),
            blocks: vec![ContentBlock::Text(TextContent::new("query"))],
            model_id: Some(QUICK_AI_SELECTED_MODEL_ID.into()),
            tool_policy:
                crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy::WebSearchOnly,
        };
        assert_eq!(
            extract_zero_context_query(&request, QUICK_AI_SELECTED_MODEL_ID).unwrap(),
            "query"
        );
        let mut wrong = request.clone();
        wrong.model_id = Some("other/model".into());
        assert!(extract_zero_context_query(&wrong, QUICK_AI_SELECTED_MODEL_ID).is_err());
        wrong.model_id = Some(QUICK_AI_SELECTED_MODEL_ID.into());
        wrong
            .blocks
            .push(ContentBlock::Text(TextContent::new("context")));
        assert!(extract_zero_context_query(&wrong, QUICK_AI_SELECTED_MODEL_ID).is_err());
    }

    #[test]
    fn codex_quick_ai_rejects_image_or_additional_context_blocks() {
        let request = AgentChatTurnRequest {
            ui_thread_id: "t".into(),
            cwd: PathBuf::from("/tmp"),
            blocks: vec![ContentBlock::Image(ImageContent::new("data", "image/png"))],
            model_id: Some(QUICK_AI_SELECTED_MODEL_ID.into()),
            tool_policy:
                crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy::WebSearchOnly,
        };
        assert!(extract_zero_context_query(&request, QUICK_AI_SELECTED_MODEL_ID).is_err());
    }

    #[test]
    fn codex_quick_ai_search_start_maps_to_existing_tool_start() {
        let mut acc = accumulator();
        let events = apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}"#).unwrap();
        assert!(
            matches!(events.as_slice(), [AgentChatEvent::ToolCallStarted { tool_call_id, tool_name: None, raw_input: None, .. }] if tool_call_id == QUICK_AI_WEB_ROW_ID)
        );
    }

    #[test]
    fn codex_quick_ai_page_follow_after_search_is_budget_exceeded() {
        for action in [
            r#"{"type":"open_page","url":"https://blog.rust-lang.org/a"}"#,
            r#"{"type":"find_in_page","url":"https://blog.rust-lang.org/b","pattern":"release"}"#,
        ] {
            let mut acc = accumulator();
            apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"a","type":"web_search","action":{"type":"search","query":"Rust"}}}"#).unwrap();
            let line = format!(
                r#"{{"type":"item.completed","item":{{"id":"b","type":"web_search","action":{action}}}}}"#
            );
            assert!(matches!(
                apply_line_decision(&mut acc, &line).unwrap(),
                CodexEventDecision::StopForRecovery(_)
            ));
        }
    }

    #[test]
    fn codex_quick_ai_same_item_lifecycle_consumes_one_permit() {
        let mut acc = accumulator();
        for phase in ["item.started", "item.updated", "item.completed"] {
            let line = format!(
                r#"{{"type":"{phase}","item":{{"id":"one","type":"web_search","action":{{"type":"search","query":"Rust release"}}}}}}"#
            );
            assert!(matches!(
                apply_line_decision(&mut acc, &line).unwrap(),
                CodexEventDecision::Continue(_)
            ));
        }
        assert_eq!(acc.web_budget.admitted_item_id.as_deref(), Some("one"));
        assert!(acc.web_budget.search_completed);
        assert_eq!(acc.web_budget.excess_action_count, 0);
    }

    #[test]
    fn codex_quick_ai_second_item_same_query_is_budget_exceeded() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"one","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap();
        assert!(matches!(
            apply_line_decision(&mut acc, r#"{"type":"item.started","item":{"id":"two","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap(),
            CodexEventDecision::StopForRecovery(_)
        ));
        assert!(!acc.search_items.contains_key("two"));
    }

    #[test]
    fn codex_quick_ai_multiple_queries_in_one_action_are_budget_exceeded() {
        let mut acc = accumulator();
        assert!(matches!(
            apply_line_decision(&mut acc, r#"{"type":"item.started","item":{"id":"one","type":"web_search","action":{"type":"search","queries":["Rust release","Rust blog"]}}}"#).unwrap(),
            CodexEventDecision::StopForRecovery(_)
        ));
    }

    #[test]
    fn codex_quick_ai_duplicate_queries_in_one_action_are_one_search() {
        let mut acc = accumulator();
        assert!(matches!(
            apply_line_decision(&mut acc, r#"{"type":"item.completed","item":{"id":"one","type":"web_search","action":{"type":"search","queries":[" Rust   release ","rust release"]}}}"#).unwrap(),
            CodexEventDecision::Continue(_)
        ));
        assert!(acc.web_budget.search_completed);
        assert_eq!(acc.web_budget.excess_action_count, 0);
    }

    #[test]
    fn codex_quick_ai_schema_allows_truthful_empty_sources() {
        let answer = parse_structured_answer_candidate(
            r#"{"answer":"The search returned no usable result.","sources":[]}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(answer.source_count, 0);
        assert_eq!(answer.rendered, "The search returned no usable result.");
    }

    #[test]
    fn codex_quick_ai_app_validation_enforces_schema_limits() {
        let too_long = format!(
            r#"{{"answer":"{}","sources":[]}}"#,
            "x".repeat(QUICK_AI_MAX_ANSWER_CHARS + 1)
        );
        assert_eq!(
            parse_structured_answer_candidate(&too_long)
                .unwrap_err()
                .message,
            "quick_ai_output_schema_answer_too_long"
        );
        assert_eq!(
            parse_structured_answer_candidate(
                r#"{"answer":"ok","sources":["https://a.example","https://b.example","https://c.example","https://d.example"]}"#,
            )
            .unwrap_err()
            .message,
            "quick_ai_output_schema_too_many_sources"
        );
        assert_eq!(
            parse_structured_answer_candidate(
                r#"{"answer":"ok","sources":["https://a.example","https://a.example"]}"#,
            )
            .unwrap_err()
            .message,
            "quick_ai_output_schema_source_duplicate"
        );
        assert_eq!(
            parse_structured_answer_candidate(
                r#"{"answer":"ok","sources":["file:///tmp/provider-secret"]}"#,
            )
            .unwrap_err()
            .message,
            "quick_ai_output_schema_source_invalid"
        );
    }

    #[test]
    fn codex_quick_ai_structured_sourced_answer_completes_early() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"one","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap();
        let decision = apply_line_decision(&mut acc, r#"{"type":"item.completed","item":{"id":"answer","type":"agent_message","text":"{\"answer\":\"Rust 1.97.1\",\"sources\":[\"https://blog.rust-lang.org/releases/latest/\"]}"}}"#).unwrap();
        let CodexEventDecision::CompleteEarly(answer) = decision else {
            panic!("schema-valid sourced answer should complete early");
        };
        assert_eq!(answer.source_count, 1);
        assert!(answer.rendered.contains("https://blog.rust-lang.org/"));
    }

    /// Replay a captured `codex exec --json` stream through the real turn
    /// state machine and return what a user would see.
    ///
    /// Hand-written event lines drift from the provider. These fixtures are
    /// verbatim streams captured from `gpt-5.3-codex-spark` with the exact
    /// production command (see
    /// `scripts/agentic/quick-ai-codex-stream-corpus.ts`), so a change in how
    /// Codex reports searches shows up here as a failing test instead of as a
    /// silent production regression.
    fn replay_quick_ai_stream(stream: &str) -> Result<String, CodexTurnFailure> {
        let mut acc = accumulator();
        for line in stream.lines().filter(|line| !line.trim().is_empty()) {
            match apply_line_decision(&mut acc, line)? {
                CodexEventDecision::Continue(_) => {}
                CodexEventDecision::CompleteEarly(answer) => return Ok(answer.rendered),
                CodexEventDecision::StopForRecovery(record) => {
                    return Err(CodexTurnFailure::protocol(format!(
                        "policy_recovery:{:?}",
                        record.failure.code
                    )));
                }
            }
        }
        match finalize_successful_turn(&acc)?.first() {
            Some(AgentChatEvent::AgentMessageDelta(answer)) => Ok(answer.clone()),
            _ => Err(CodexTurnFailure::protocol("replay_produced_no_answer")),
        }
    }

    const STREAM_NO_WEB: &str = include_str!("testdata/quick-ai-streams/no-web-1.ndjson");
    const STREAM_SEARCH_SOURCES_IN_TEXT: &str =
        include_str!("testdata/quick-ai-streams/rust-release-2.ndjson");
    const STREAM_SEARCH_SOURCES_ONLY: &str =
        include_str!("testdata/quick-ai-streams/weather-ish-2.ndjson");
    const STREAM_SINGLE_SOURCE: &str =
        include_str!("testdata/quick-ai-streams/bun-version-2.ndjson");

    /// A question the model can answer from its own knowledge must succeed.
    ///
    /// The provenance gate used to require `search_completed` before ANY
    /// answer was allowed through, so "What does the Rust `?` operator do?"
    /// — answered with an empty `sources` array and no web item at all —
    /// failed with `quick_ai_structured_sources_unavailable`. Requiring a web
    /// search to answer a non-web question is backwards: an empty sources
    /// array is the honest shape, not a missing citation.
    #[test]
    fn codex_quick_ai_answers_a_knowledge_question_without_searching() {
        let answer = replay_quick_ai_stream(STREAM_NO_WEB)
            .expect("a sourceless knowledge answer must not need a web search");
        assert!(answer.contains("`?` operator"), "answer was: {answer}");
        assert!(
            !answer.contains("http"),
            "a knowledge answer must not gain a citation: {answer}"
        );
    }

    /// Schema `sources` reach the reader after one focused search.
    #[test]
    fn codex_quick_ai_renders_validated_sources_from_a_single_search() {
        for stream in [STREAM_SEARCH_SOURCES_ONLY, STREAM_SINGLE_SOURCE] {
            let answer =
                replay_quick_ai_stream(stream).expect("one focused search must produce an answer");
            assert!(
                !http_urls_in_text(&answer).is_empty(),
                "sourced answer lost its citations: {answer}"
            );
        }
    }

    /// Codex still hands the model a shell tool, and it uses it.
    ///
    /// This fixture is a verbatim production-command stream in which
    /// `gpt-5.3-codex-spark` ran `/bin/zsh -lc 'recall context'` — reading the
    /// user's shared agent memory — while answering "what is the latest stable
    /// Rust release?". `--sandbox read-only` does not prevent it, because the
    /// read IS permitted by a read-only sandbox.
    ///
    /// Quick AI fails closed, which is correct, but the user gets an error
    /// instead of an answer and the private read already happened. The command
    /// shape must remove the tool rather than reject its output; a measured A/B
    /// (`scripts/agentic/quick-ai-shell-tool-gate-probe.ts`) shows
    /// `features.shell_tool=false` does exactly that.
    #[test]
    fn codex_quick_ai_fails_closed_on_a_real_shell_command_it_should_never_have_been_offered() {
        let error = replay_quick_ai_stream(STREAM_SEARCH_SOURCES_IN_TEXT)
            .expect_err("a shell command in a Quick AI turn must fail closed");
        assert_eq!(
            error.message,
            "quick_ai_codex_forbidden_item:command_execution:item_0"
        );
    }

    /// The reader must never be shown a URL that skipped schema validation.
    ///
    /// With snippets-only searching there is no page visit to verify a host
    /// against, so the one guarantee still available is that every URL in the
    /// rendered answer came through `validate_structured_answer_fields`. A
    /// URL smuggled in the prose alone bypassed that entirely.
    #[test]
    fn codex_quick_ai_rejects_an_answer_url_missing_from_validated_sources() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"one","type":"web_search","action":{"type":"search","query":"f1 winner"}}}"#).unwrap();
        let error = apply_line_decision(&mut acc, r#"{"type":"item.completed","item":{"id":"answer","type":"agent_message","text":"{\"answer\":\"See https://totally-made-up.example/race for details.\",\"sources\":[\"https://www.formula1.com/en/latest\"]}"}}"#)
            .expect_err("an unvalidated prose URL must not reach the reader");
        assert_eq!(
            error.message,
            "quick_ai_answer_url_not_in_validated_sources"
        );
    }

    /// Citing sources without having searched is fabrication, and stays fatal.
    #[test]
    fn codex_quick_ai_rejects_cited_sources_when_no_search_ran() {
        let mut acc = accumulator();
        // No search completed, so nothing short-circuits: the answer is
        // buffered and the turn is judged at finalization.
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"answer","type":"agent_message","text":"{\"answer\":\"Rust 1.97.1 shipped today.\",\"sources\":[\"https://blog.rust-lang.org/releases/latest/\"]}"}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert_eq!(
            finalize_successful_turn(&acc).unwrap_err().message,
            "quick_ai_structured_sources_unavailable"
        );
    }

    #[test]
    fn codex_quick_ai_trace_redacts_provider_item_and_raw_action() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.ndjson");
        let trace = TraceSink::new(
            Some(trace_path.clone()),
            "safe-run".to_string(),
            Instant::now(),
        );
        let event = parse_codex_exec_line(r#"{"type":"item.started","item":{"id":"provider-secret-id","type":"web_search","query":"private query text","action":{"type":"search","query":"private query text"}}}"#).unwrap();
        trace_event_for_protocol(&trace, &event);
        let output = std::fs::read_to_string(trace_path).unwrap();
        assert!(!output.contains("provider-secret-id"));
        assert!(!output.contains("private query text"));
        assert!(!output.contains(&sha256_hex("private query text")));
        assert!(
            output.contains(&crate::logging::log_private_user_value("private query text").sha256)
        );
        assert!(!output.contains("\"action\":"));
        assert!(output.contains("\"actionOrdinal\":1"));
    }

    #[test]
    fn codex_quick_ai_trace_query_fingerprints_cannot_be_guessed_from_typed_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("private-trace.ndjson");
        let trace = TraceSink::new(
            Some(trace_path.clone()),
            "private-query-run".to_string(),
            Instant::now(),
        );

        for (index, query) in ["private-secre", "private-secret"].into_iter().enumerate() {
            let line = serde_json::json!({
                "type": "item.started",
                "item": {
                    "id": format!("provider-item-{index}"),
                    "type": "web_search",
                    "query": query,
                    "action": { "type": "search", "query": query },
                },
            });
            let event = parse_codex_exec_line(&line.to_string()).unwrap();
            trace_event_for_protocol(&trace, &event);
        }

        let output = std::fs::read_to_string(trace_path).unwrap();
        let records = output
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|record| record["event"] == "native_web_action")
            .collect::<Vec<_>>();

        assert_eq!(records.len(), 2);
        for (record, query) in records.iter().zip(["private-secre", "private-secret"]) {
            let actual = record["querySha256"].as_str().unwrap();
            assert_eq!(actual, crate::logging::log_private_user_value(query).sha256);
            assert_ne!(actual, sha256_hex(query));
            assert!(!output.contains(query));
        }
        assert_ne!(records[0]["querySha256"], records[1]["querySha256"]);
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_trace_private_files_repair_legacy_permissions_before_append() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated Quick AI trace fixture");
        let path = directory.path().join("quick.ndjson");
        let trace = TraceSink::new(Some(path.clone()), "private-run".into(), Instant::now());
        trace.write("start_turn_entered", json!({}));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        trace.write("first_protocol_event", json!({}));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_trace_private_files_reject_symlinks_without_touching_foreign_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated Quick AI trace symlink fixture");
        let external = directory.path().join("foreign.txt");
        let planted = directory.path().join("quick.ndjson");
        std::fs::write(&external, "foreign trace must remain untouched").unwrap();
        symlink(&external, &planted).unwrap();

        let trace = TraceSink::new(Some(planted.clone()), "private-run".into(), Instant::now());
        trace.write("start_turn_entered", json!({}));

        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            "foreign trace must remain untouched"
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn codex_quick_ai_trace_private_model_and_diagnostic_fingerprints_cannot_be_guessed() {
        let directory = tempfile::tempdir().expect("isolated Quick AI private trace fixture");
        let path = directory.path().join("quick.ndjson");
        let trace = TraceSink::new(Some(path.clone()), "private-run".into(), Instant::now());
        let answer = "my private assistant answer";
        let diagnostic = "my private provider diagnostic";

        for line in [
            json!({
                "type": "item.completed",
                "item": { "id": "answer", "type": "agent_message", "text": answer },
            }),
            json!({
                "type": "item.completed",
                "item": { "id": "failure", "type": "error", "message": diagnostic },
            }),
        ] {
            let event = parse_codex_exec_line(&line.to_string()).unwrap();
            trace_event_for_protocol(&trace, &event);
        }

        let output = std::fs::read_to_string(path).unwrap();
        let records = output
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        for (event, field, secret) in [
            ("agent_message_buffered", "textSha256", answer),
            ("diagnostic", "messageSha256", diagnostic),
        ] {
            let record = records
                .iter()
                .find(|record| record["event"] == event)
                .unwrap();
            let actual = record[field].as_str().unwrap();
            assert_eq!(
                actual,
                crate::logging::log_private_user_value(secret).sha256
            );
            assert_ne!(actual, sha256_hex(secret));
            assert!(!output.contains(secret));
            assert!(!output.contains(&sha256_hex(secret)));
        }
    }

    #[test]
    fn codex_quick_ai_agent_message_containing_web_search_is_not_a_tool() {
        let event = parse_codex_exec_line(r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"I used web_search"}}"#).unwrap();
        assert!(matches!(
            event,
            CodexExecEvent::Item {
                item: CodexItem::AgentMessage { .. },
                ..
            }
        ));
    }

    #[test]
    fn codex_quick_ai_buffers_preamble_and_emits_only_last_agent_message() {
        let mut acc = accumulator();
        for line in [
            r#"{"type":"item.completed","item":{"id":"m1","type":"agent_message","text":"preamble"}}"#,
            r#"{"type":"item.completed","item":{"id":"s","type":"web_search","action":{"type":"search","query":"example source"}}}"#,
            // No URL: this test is about which message wins, not provenance.
            r#"{"type":"item.completed","item":{"id":"m2","type":"agent_message","text":"final answer text"}}"#,
            r#"{"type":"turn.completed","usage":{}}"#,
        ] {
            apply_line(&mut acc, line).unwrap();
        }
        let events = finalize_successful_turn(&acc).unwrap();
        assert!(
            matches!(&events[0], AgentChatEvent::AgentMessageDelta(text) if text.starts_with("final"))
        );
    }

    #[test]
    fn codex_quick_ai_duplicate_item_completion_and_terminal_are_idempotent() {
        let mut acc = accumulator();
        let line =
            r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"answer"}}"#;
        apply_line(&mut acc, line).unwrap();
        apply_line(&mut acc, line).unwrap();
        assert_eq!(acc.completed_agent_messages.len(), 1);
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert!(acc.terminal_seen);
    }

    #[test]
    fn codex_quick_ai_second_focused_search_returns_typed_budget_recovery() {
        let mut acc = accumulator();
        for line in [
            r#"{"type":"item.started","item":{"id":"a","type":"web_search","action":{"type":"search","query":"first"}}}"#,
            r#"{"type":"item.completed","item":{"id":"a","type":"web_search","action":{"type":"search","query":"first"}}}"#,
            r#"{"type":"item.completed","item":{"id":"partial","type":"agent_message","text":"A completed partial answer. https://example.com/source"}}"#,
        ] {
            apply_line(&mut acc, line).unwrap();
        }
        let decision = apply_line_decision(
            &mut acc,
            r#"{"type":"item.started","item":{"id":"b","type":"web_search","action":{"type":"search","query":"second"}}}"#,
        )
        .unwrap();
        let CodexEventDecision::StopForRecovery(record) = decision else {
            panic!("the second focused search must stop for typed recovery");
        };
        assert!(matches!(
            record.failure.kind,
            sk_protocol::ai_reliability::AiFailureKind::Policy(
                sk_protocol::ai_reliability::PolicyFailure::QuickAiSearchBudgetExceeded {
                    completed_searches: 1,
                    budget: 1,
                    partial_answer_available: true,
                    source_count: 1,
                }
            )
        ));
        assert_eq!(
            partial_answer(&acc).as_deref(),
            Some("A completed partial answer. https://example.com/source")
        );
        assert!(!acc.search_items.contains_key("b"));
        assert!(!acc.search_order.iter().any(|item| item == "b"));
    }

    #[test]
    fn codex_quick_ai_budget_recovery_never_fabricates_answer_from_urls() {
        let mut acc = accumulator();
        for line in [
            r#"{"type":"item.completed","item":{"id":"a","type":"web_search","action":{"type":"search","query":"first"}}}"#,
        ] {
            apply_line(&mut acc, line).unwrap();
        }
        let decision = apply_line_decision(
            &mut acc,
            r#"{"type":"item.started","item":{"id":"b","type":"web_search","action":{"type":"open_page","url":"https://example.com/source"}}}"#,
        )
        .unwrap();
        let CodexEventDecision::StopForRecovery(record) = decision else {
            panic!("the second focused search must stop for typed recovery");
        };
        assert!(matches!(
            record.failure.kind,
            sk_protocol::ai_reliability::AiFailureKind::Policy(
                sk_protocol::ai_reliability::PolicyFailure::QuickAiSearchBudgetExceeded {
                    partial_answer_available: false,
                    source_count: 1,
                    ..
                }
            )
        ));
        assert!(partial_answer(&acc).is_none());
    }

    #[test]
    fn codex_quick_ai_forbidden_and_unknown_items_fail_closed() {
        for item_type in ["command_execution", "file_change", "mcp_tool_call"] {
            let mut acc = accumulator();
            let line =
                format!(r#"{{"type":"item.started","item":{{"id":"x","type":"{item_type}"}}}}"#);
            assert!(apply_line(&mut acc, &line).is_err());
        }
        assert!(parse_codex_exec_line(
            r#"{"type":"item.started","item":{"id":"x","type":"future_tool"}}"#
        )
        .is_err());
    }

    #[test]
    fn codex_quick_ai_current_wire_diagnostic_is_ignored_but_url_visit_is_denied() {
        let mut acc = accumulator();
        assert!(apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"warning","type":"error","message":"Skill descriptions were shortened to fit the 2% skills context budget. Codex can still see every skill, but some descriptions are shorter. Disable unused skills or plugins to leave more room for the rest."}}"#).is_ok());
        assert!(matches!(
            apply_line_decision(&mut acc, r#"{"type":"item.completed","item":{"id":"source","type":"web_search","query":"https://blog.rust-lang.org/releases/latest/","action":{"type":"other"}}}"#).unwrap(),
            CodexEventDecision::StopForRecovery(_)
        ));
        assert_eq!(
            acc.recovery_only_urls,
            ["https://blog.rust-lang.org/releases/latest/"]
        );
    }

    #[test]
    fn codex_quick_ai_unexpected_error_item_fails_closed() {
        let mut acc = accumulator();
        let result = apply_line(
            &mut acc,
            r#"{"type":"item.completed","item":{"id":"error","type":"error","message":"unexpected provider failure"}}"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn codex_quick_ai_final_answer_url_without_structured_source_fails() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"answer https://example.com"}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert_eq!(
            finalize_successful_turn(&acc).unwrap_err().message,
            "quick_ai_structured_sources_unavailable"
        );
    }

    #[test]
    fn codex_quick_ai_output_schema_renders_answer_and_source() {
        let rendered = render_final_answer(
            r#"{"answer":"Rust 1.97.0 was released July 9, 2026.","sources":["https://blog.rust-lang.org/releases/latest/"]}"#,
        )
        .unwrap();
        assert_eq!(
            rendered,
            "Rust 1.97.0 was released July 9, 2026.\n\nSource: https://blog.rust-lang.org/releases/latest/"
        );
    }

    /// A bare-prose URL is no longer accepted just because a search ran.
    ///
    /// This test previously asserted the opposite. "A search completed, so any
    /// URL in the reply is fine" was the entire source check on the normal
    /// path — a search item carries no result URLs, so nothing else was ever
    /// compared. Under `--output-schema` the model always answers as JSON, so
    /// a URL that appears only in prose skipped
    /// `validate_structured_answer_fields` and reached the reader unchecked.
    #[test]
    fn codex_quick_ai_prose_only_url_after_native_search_is_rejected() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","action":{"type":"other"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"s","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"Rust 1.97.0: https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/"}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert_eq!(
            finalize_successful_turn(&acc).unwrap_err().message,
            "quick_ai_answer_url_not_in_validated_sources"
        );
    }

    /// The same URL IS accepted when it came through the validated schema.
    #[test]
    fn codex_quick_ai_schema_source_url_after_native_search_is_accepted() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","action":{"type":"other"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"s","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap();
        let decision = apply_line_decision(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"{\"answer\":\"Rust 1.97.0: https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/\",\"sources\":[\"https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/\"]}"}}"#).unwrap();
        let CodexEventDecision::CompleteEarly(answer) = decision else {
            panic!("a validated sourced answer should complete early");
        };
        assert!(answer.rendered.contains("Rust-1.97.0"));
    }

    #[test]
    fn codex_quick_ai_structured_answer_appends_missing_source_url() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","action":{"type":"other"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"s","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap();
        let decision = apply_line_decision(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"{\"answer\":\"Rust 1.97.0 was released July 9, 2026.\",\"sources\":[\"https://blog.rust-lang.org/releases/latest/\"]}"}}"#).unwrap();
        let CodexEventDecision::CompleteEarly(answer) = decision else {
            panic!("structured answer should complete early");
        };
        assert!(answer
            .rendered
            .ends_with("Source: https://blog.rust-lang.org/releases/latest/"));
    }

    #[test]
    fn codex_quick_ai_prepare_session_does_not_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary: dir.path().join("missing-codex"),
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection
            .prepare_session("t".into(), dir.path().into())
            .unwrap();
        assert!(matches!(
            rx.recv_blocking().unwrap(),
            AgentChatEvent::ModelsAvailable { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_success_reaps_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let binary = fake_codex(
            dir.path(),
            r#"
if IFS= read -r ignored; then exit 91; fi
printf '%s\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.started","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"{\"answer\":\"Rust\",\"sources\":[\"https://blog.rust-lang.org/source\"]}"}}'
printf '%s\n' '{"type":"turn.completed","usage":{}}'
sleep 1
"#,
        );
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary,
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection.start_turn(turn_request()).unwrap();
        let mut answer = false;
        let mut finished = false;
        while let Ok(event) = rx.recv_blocking() {
            answer |= matches!(event, AgentChatEvent::AgentMessageDelta(_));
            finished |= matches!(event, AgentChatEvent::TurnCompleted { .. });
        }
        assert!(answer && finished);
        assert!(connection.active_turns.lock().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(dir.path().join("scratch"))
                .unwrap()
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_budget_recovery_preserves_partial_and_reaps_process() {
        let dir = tempfile::tempdir().unwrap();
        let binary = fake_codex(
            dir.path(),
            r#"
printf '%s\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.started","item":{"id":"a","type":"web_search","action":{"type":"search","query":"first"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"a","type":"web_search","action":{"type":"search","query":"first"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"partial","type":"agent_message","text":"A real partial answer. https://example.com/source"}}'
printf '%s\n' '{"type":"item.started","item":{"id":"b","type":"web_search","action":{"type":"search","query":"second"}}}'
sleep 60
"#,
        );
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary,
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection.start_turn(turn_request()).unwrap();
        let mut tool_ids = Vec::new();
        let mut partial = None;
        let mut policy_failure = None;
        while let Ok(event) = rx.recv_blocking() {
            match event {
                AgentChatEvent::ToolCallStarted { tool_call_id, .. } => {
                    tool_ids.push(tool_call_id);
                }
                AgentChatEvent::AgentMessageDelta(text) => partial = Some(text),
                AgentChatEvent::TurnFailed { failure } => {
                    policy_failure = Some(failure.failure.kind);
                }
                _ => {}
            }
        }
        assert_eq!(
            partial.as_deref(),
            Some("A real partial answer. https://example.com/source")
        );
        assert!(!tool_ids.iter().any(|id| id == "b"));
        assert!(matches!(
            policy_failure,
            Some(sk_protocol::ai_reliability::AiFailureKind::Policy(
                sk_protocol::ai_reliability::PolicyFailure::QuickAiSearchBudgetExceeded {
                    completed_searches: 1,
                    budget: 1,
                    partial_answer_available: true,
                    source_count: 1,
                }
            ))
        ));
        assert!(connection.active_turns.lock().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(dir.path().join("scratch"))
                .unwrap()
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_deadline_preserves_partial_and_reaps_process() {
        let dir = tempfile::tempdir().unwrap();
        let binary = fake_codex(
            dir.path(),
            r#"
printf '%s\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"one","type":"web_search","action":{"type":"search","query":"Rust release"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"partial","type":"agent_message","text":"A real partial answer. https://example.com/source"}}'
sleep 60
"#,
        );
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary,
            work_deadline: Duration::from_millis(80),
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection.start_turn(turn_request()).unwrap();
        let mut partial = None;
        let mut failure = None;
        while let Ok(event) = rx.recv_blocking() {
            match event {
                AgentChatEvent::AgentMessageDelta(text) => partial = Some(text),
                AgentChatEvent::TurnFailed { failure: record } => {
                    failure = Some(record.failure.kind)
                }
                _ => {}
            }
        }
        assert_eq!(
            partial.as_deref(),
            Some("A real partial answer. https://example.com/source")
        );
        assert!(matches!(
            failure,
            Some(sk_protocol::ai_reliability::AiFailureKind::Policy(
                sk_protocol::ai_reliability::PolicyFailure::QuickAiDeadlineExceeded {
                    completed_searches: 1,
                    partial_answer_available: true,
                    source_count: 1,
                    ..
                }
            ))
        ));
        assert!(connection.active_turns.lock().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(dir.path().join("scratch"))
                .unwrap()
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_cancel_escalates_and_reaps_parent_and_grandchild() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.ndjson");
        let binary = fake_codex(
            dir.path(),
            r#"
trap '' TERM
(trap '' TERM; while :; do sleep 60; done) &
printf '%s\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.started","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}'
while :; do sleep 60; done
"#,
        );
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary,
            trace_path: Some(trace_path.clone()),
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection.start_turn(turn_request()).unwrap();
        let started = rx.recv_blocking().unwrap();
        assert!(matches!(started, AgentChatEvent::ToolCallStarted { .. }));
        let pgid = connection
            .active_turns
            .lock()
            .unwrap()
            .get("quick-ai-test-thread")
            .unwrap()
            .pgid;
        connection
            .cancel_turn("quick-ai-test-thread".into())
            .unwrap();
        let mut cancelled = false;
        while let Ok(event) = rx.recv_blocking() {
            cancelled |= matches!(
                event,
                AgentChatEvent::TurnCompleted {
                    outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Cancelled { .. }
                }
            );
        }
        assert!(cancelled);
        assert!(!process_group_alive(pgid));
        let trace = std::fs::read_to_string(trace_path).unwrap();
        assert!(trace.contains("\"killSent\":true"));
        assert!(connection.active_turns.lock().unwrap().is_empty());
    }
}

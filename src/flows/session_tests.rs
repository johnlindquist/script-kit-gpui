#[cfg(test)]
mod tests {
    use super::*;

    /// The cause is the whole difference between the two rethread entry
    /// points, so it is asserted exhaustively rather than at one call site.
    #[test]
    fn only_a_user_requested_reset_discards_the_transcript() {
        assert!(
            FlowConversationResetCause::Recovery.preserves_transcript(),
            "the engine died, the conversation did not — recovery rolls the \
             transcript into the new thread"
        );
        assert!(
            !FlowConversationResetCause::UserRequested.preserves_transcript(),
            "'New Conversation' that kept the old turns would not be a new \
             conversation"
        );
    }

    /// The defect this helper exists to prevent: the in-flight turn is already
    /// in `turns` with an empty `assistant`, so the obvious `turns.last()`
    /// copies `""` — and writing `""` to the clipboard SUCCEEDS. The user sees
    /// a copy that worked and pastes nothing.
    #[test]
    fn copying_mid_stream_reaches_past_the_empty_in_flight_turn() {
        let turns = ["first answer", "second answer", ""];
        assert_eq!(
            resolve_last_copyable_response(turns.iter().copied()),
            Some("second answer"),
            "the newest turn with an actual answer wins, not the newest turn"
        );
    }

    #[test]
    fn a_whitespace_only_turn_is_not_an_answer() {
        let turns = ["real answer", "   \n\t "];
        assert_eq!(
            resolve_last_copyable_response(turns.iter().copied()),
            Some("real answer")
        );
    }

    /// `None` and `Some("")` must not be confusable — only `None` may reach the
    /// "nothing to copy" toast, and only a non-empty string may reach the
    /// clipboard.
    #[test]
    fn a_conversation_with_no_answer_yet_copies_nothing() {
        assert_eq!(resolve_last_copyable_response([].iter().copied()), None);
        assert_eq!(
            resolve_last_copyable_response(["", "  "].iter().copied()),
            None
        );
    }

    #[test]
    fn the_copied_answer_preserves_exact_assistant_bytes() {
        assert_eq!(
            resolve_last_copyable_response(["\n  answer body  \n"].iter().copied()),
            Some("\n  answer body  \n")
        );
    }

    /// A reset while a turn is in flight would orphan a running engine turn:
    /// it keeps spending, and the user has no route back to it.
    #[test]
    fn a_reset_is_refused_while_a_turn_is_in_flight() {
        for cause in [
            FlowConversationResetCause::Recovery,
            FlowConversationResetCause::UserRequested,
        ] {
            assert_eq!(
                resolve_flow_conversation_reset_guard(cause, true),
                FlowConversationResetGuard::BlockedByActiveTurn,
                "{cause:?} must not reset over a live turn"
            );
            assert_eq!(
                resolve_flow_conversation_reset_guard(cause, false),
                FlowConversationResetGuard::Allowed,
                "{cause:?} is allowed on an idle session"
            );
        }
    }

    /// Empty active-thread metadata is a real persisted conversation state.
    /// New Conversation must not turn emptiness into deletion or revive the
    /// replaced transcript.
    #[test]
    fn an_empty_active_thread_persists_without_restoring_old_turns() {
        let dir = std::env::temp_dir().join(format!(
            "sk-flow-empty-snapshot-{}",
            std::process::id() as u64
        ));
        let _ = std::fs::create_dir_all(&dir);
        let flow_id = "project:reset-probe";
        let flow_path = "/tmp/reset-probe.md";

        persist_conversation_to(
            &dir,
            flow_id,
            flow_path,
            &[SessionTurn {
                user: "first question".into(),
                assistant: "first answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            }],
        )
        .expect("seed snapshot");
        assert!(
            load_persisted_conversation_from(&dir, flow_id, flow_path).is_some(),
            "a real transcript must load, or this test proves nothing"
        );

        persist_conversation_to(&dir, flow_id, flow_path, &[]).expect("replacement snapshot");
        let replacement = load_persisted_conversation_from(&dir, flow_id, flow_path)
            .expect("empty active metadata remains loadable");
        assert!(canonical_session_turns(&replacement).is_empty());
        assert_eq!(replacement.threads.len(), 1);
        assert_eq!(
            replacement.threads[0].state,
            PersistedFlowThreadState::Active
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mission_resolution_strips_frontmatter_and_substitutes_task() {
        let markdown =
            "---\ndescription: GitHub examples\n---\nSearch GitHub for examples.\n\n{{ _task }}\n";
        assert_eq!(
            resolve_flow_mission(markdown, "bun shell scripts"),
            "Search GitHub for examples.\n\nbun shell scripts"
        );
    }

    #[test]
    fn mission_without_task_slot_appends_message() {
        assert_eq!(
            resolve_flow_mission("Reply tersely.", "hello"),
            "Reply tersely.\n\nhello"
        );
        assert_eq!(resolve_flow_mission("", "hello"), "hello");
    }

    #[test]
    fn transport_picks_codex_thread_only_for_codex() {
        assert_eq!(
            SessionTransport::for_engine("codex"),
            SessionTransport::CodexThread
        );
        assert_eq!(
            SessionTransport::for_engine("claude"),
            SessionTransport::MdflowTurns
        );
        assert_eq!(
            SessionTransport::for_engine("fasteng"),
            SessionTransport::MdflowTurns
        );
    }

    #[test]
    fn first_turn_task_is_verbatim() {
        assert_eq!(
            build_turn_task(&[], "what did vercel email me?"),
            "what did vercel email me?"
        );
    }

    #[cfg(unix)]
    fn write_fake_md(path: &std::path::Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, body).expect("write fake md");
        let mut permissions = std::fs::metadata(path)
            .expect("fake md metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("make fake md executable");
    }

    #[cfg(unix)]
    mod of38 {
        use super::*;
        use std::os::unix::process::ExitStatusExt;
        use std::time::Duration;

        fn rejected_explain() -> MdExplainOutput {
            MdExplainOutput {
                output: std::process::Output {
                    status: std::process::ExitStatus::from_raw(1 << 8),
                    stdout: Vec::new(),
                    stderr: b"positional rejected".to_vec(),
                },
                explain: None,
            }
        }

        #[test]
        fn turn_arg_resolution_uses_one_deadline_for_both_shapes() {
            for iteration in 0..20 {
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut calls: Vec<(Vec<String>, Instant)> = Vec::new();
                let result = resolve_mdflow_turn_arg_with_runner(
                    "md",
                    "flow.md",
                    "/tmp",
                    "hello",
                    deadline,
                    |_binary, _flow_path, _cwd, args, received_deadline| {
                        calls.push((
                            args.iter().map(|arg| (*arg).to_string()).collect(),
                            received_deadline,
                        ));
                        if calls.len() == 1 {
                            Ok(rejected_explain())
                        } else {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "shared deadline exhausted",
                            ))
                        }
                    },
                );

                assert!(result.is_err(), "iteration {iteration} must fail closed");
                assert_eq!(calls.len(), 2, "both shapes are attempted in order");
                assert_eq!(calls[0].0, ["hello"]);
                assert_eq!(calls[1].0, ["--_task", "hello"]);
                assert_eq!(calls[0].1, deadline);
                assert_eq!(
                    calls[1].1, deadline,
                    "iteration {iteration} must not grant a fresh deadline"
                );
            }
        }
    }

    #[cfg(unix)]
    mod of39 {
        use super::*;

        #[test]
        fn turn_arg_resolution_uses_parsed_missing_template_vars() {
            let dir = tempfile::tempdir().expect("tempdir");
            let binary = dir.path().join("md");
            let calls = dir.path().join("of39-calls.txt");
            write_fake_md(
                &binary,
                "#!/bin/sh\nprintf '%s %s\\n' \"$2\" \"$3\" >> of39-calls.txt\nif [ \"$2\" = \"ordinary.md\" ]; then\n  printf '%s\\n' '{\"protocolVersion\":1,\"flowId\":\"project:ordinary\",\"path\":\"ordinary.md\",\"engine\":\"pi\",\"command\":\"pi\",\"args\":[],\"cwd\":\".\",\"prompt\":\"ok\",\"promptTokensEstimate\":1,\"inputs\":[],\"warnings\":[],\"configFingerprint\":\"sha256:ordinary\",\"templateVars\":[\"_1\"],\"missingTemplateVars\":[]}'\n  exit 0\nfi\nif [ \"$3\" = \"--_task\" ]; then\n  missing='[]'\nelse\n  missing='[\"_task\"]'\nfi\nprintf '%s\\n' \"{\\\"protocolVersion\\\":1,\\\"flowId\\\":\\\"project:named\\\",\\\"path\\\":\\\"named.md\\\",\\\"engine\\\":\\\"pi\\\",\\\"command\\\":\\\"pi\\\",\\\"args\\\":[],\\\"cwd\\\":\\\".\\\",\\\"prompt\\\":\\\"ok\\\",\\\"promptTokensEstimate\\\":1,\\\"inputs\\\":[],\\\"warnings\\\":[],\\\"configFingerprint\\\":\\\"sha256:named\\\",\\\"templateVars\\\":[\\\"_task\\\"],\\\"missingTemplateVars\\\":$missing}\"\n",
            );

            let ordinary = resolve_mdflow_turn_arg(
                binary.to_str().expect("utf8 binary"),
                "ordinary.md",
                dir.path().to_str().expect("utf8 cwd"),
                "hello ordinary",
            )
            .expect("ordinary positional input resolves");
            assert_eq!(ordinary, MdflowTurnArg::Positional("hello ordinary".into()));

            let named = resolve_mdflow_turn_arg(
                binary.to_str().expect("utf8 binary"),
                "named.md",
                dir.path().to_str().expect("utf8 cwd"),
                "hello named",
            )
            .expect("named input resolves through fallback");
            assert_eq!(named, MdflowTurnArg::NamedTask("hello named".into()));

            let calls = std::fs::read_to_string(calls).expect("explain calls logged");
            assert_eq!(
                calls.lines().collect::<Vec<_>>(),
                [
                    "ordinary.md hello ordinary",
                    "named.md hello named",
                    "named.md --_task",
                ],
                "ordinary resolves once; named tries positional then --_task"
            );
        }
    }

    const GMAIL_PATH: &str = "/pkg/flows/flow-gog-gmail.codex.md";

    #[test]
    fn conversation_persistence_round_trips_every_turn_without_a_cap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns: Vec<SessionTurn> = (0..20)
            .map(|i| SessionTurn {
                user: format!("question {i}"),
                assistant: format!("answer {i}"),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            })
            .collect();
        persist_conversation_to(dir.path(), "package:flow-gog-gmail", GMAIL_PATH, &turns)
            .expect("persist");

        let restored =
            load_persisted_conversation_from(dir.path(), "package:flow-gog-gmail", GMAIL_PATH)
                .expect("snapshot must load");
        assert_eq!(restored.flow_id, "package:flow-gog-gmail");
        assert_eq!(restored.flow_path, GMAIL_PATH);
        let restored_turns = canonical_session_turns(&restored);
        assert_eq!(restored_turns.len(), 20);
        assert_eq!(restored_turns.first().unwrap().user, "question 0");
        assert_eq!(restored_turns.last().unwrap().assistant, "answer 19");
    }

    fn thousand_turn_fixture() -> Vec<SessionTurn> {
        (0..1_000)
            .map(|index| {
                let outcome = match index % 3 {
                    0 => PersistedTurnOutcome::Ok,
                    1 => PersistedTurnOutcome::Stopped,
                    _ => PersistedTurnOutcome::Failed,
                };
                SessionTurn {
                    user: format!("question-{index:04}"),
                    assistant: format!("answer-{index:04}"),
                    outcome,
                    failure: (outcome == PersistedTurnOutcome::Failed)
                        .then(PersistedAiFailure::unknown_default),
                }
            })
            .collect()
    }

    #[test]
    fn one_thousand_turns_round_trip_without_a_cap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = thousand_turn_fixture();
        persist_conversation_to(
            dir.path(),
            "project:thousand",
            "/p/flows/thousand.md",
            &turns,
        )
        .expect("persist one thousand turns");
        let restored = load_persisted_conversation_from(
            dir.path(),
            "project:thousand",
            "/p/flows/thousand.md",
        )
        .expect("restore one thousand turns");
        let restored_turns = canonical_session_turns(&restored);
        assert_eq!(restored_turns.len(), 1_000);
        for index in [0, 500, 999] {
            assert_eq!(restored_turns[index].user, turns[index].user);
            assert_eq!(restored_turns[index].assistant, turns[index].assistant);
            assert_eq!(restored_turns[index].outcome, turns[index].outcome);
            assert_eq!(restored_turns[index].failure, turns[index].failure);
        }
    }

    #[test]
    #[ignore = "architecture benchmark; run twice in fresh test processes"]
    fn benchmark_v4_manifest_1000_turns() {
        let turns = thousand_turn_fixture();
        let snapshot = snapshot_from_turns(
            "project:thousand-benchmark",
            "/p/flows/thousand-benchmark.md",
            &turns,
        );

        for _ in 0..3 {
            let bytes = serde_json::to_vec(&snapshot).expect("warm serialization");
            let _: PersistedFlowConversation =
                serde_json::from_slice(&bytes).expect("warm deserialization");
        }

        let mut serialize_ms = Vec::new();
        let mut deserialize_ms = Vec::new();
        let mut encoded_len = 0;
        for _ in 0..21 {
            let started = Instant::now();
            let bytes = serde_json::to_vec(&snapshot).expect("serialize benchmark snapshot");
            serialize_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
            encoded_len = bytes.len();

            let started = Instant::now();
            let restored: PersistedFlowConversation =
                serde_json::from_slice(&bytes).expect("deserialize benchmark snapshot");
            deserialize_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
            assert_eq!(canonical_session_turns(&restored).len(), 1_000);
        }
        serialize_ms.sort_by(f64::total_cmp);
        deserialize_ms.sort_by(f64::total_cmp);
        let serialize_median_ms = serialize_ms[serialize_ms.len() / 2];
        let deserialize_median_ms = deserialize_ms[deserialize_ms.len() / 2];
        let combined_median_ms = serialize_median_ms + deserialize_median_ms;
        println!(
            "{{\"event\":\"flowManifestBenchmark\",\"turns\":1000,\"samples\":21,\"serializeMedianMs\":{serialize_median_ms:.3},\"deserializeMedianMs\":{deserialize_median_ms:.3},\"combinedMedianMs\":{combined_median_ms:.3},\"encodedBytes\":{encoded_len}}}"
        );
    }

    #[test]
    fn new_conversation_archives_populated_and_empty_active_metadata() {
        for populated in [false, true] {
            let mut meta = FlowSessionMeta::test_fixture();
            let original_id = meta.active_thread_id.clone();
            if populated {
                meta.turns.push(turn("question", "answer"));
            }
            meta.archive_active_and_start_empty();

            assert_ne!(meta.active_thread_id, original_id);
            assert!(meta.turns.is_empty());
            assert_eq!(meta.archived_threads.len(), 1);
            assert_eq!(meta.archived_threads[0].id, original_id);
            assert_eq!(meta.archived_threads[0].turns.len(), usize::from(populated));
            assert_eq!(meta.transcript_selection, FlowTranscriptSelection::Active);
        }
    }

    #[test]
    fn continue_as_new_retains_archive_and_records_lineage() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.turns = vec![turn("source-1", "answer-1"), turn("source-2", "answer-2")];
        meta.archive_active_and_start_empty();
        let source_id = meta.archived_threads[0].id.clone();
        let source_turns = meta.archived_threads[0].turns.clone();
        assert!(meta.select_archive(&source_id));

        assert!(meta.continue_archive_as_new(&source_id));
        assert_eq!(meta.transcript_selection, FlowTranscriptSelection::Active);
        assert_eq!(
            meta.active_parent_thread_id.as_deref(),
            Some(source_id.as_str())
        );
        assert_eq!(meta.turns, source_turns);
        assert_eq!(meta.inherited_turn_count, source_turns.len());
        assert!(meta
            .archived_threads
            .iter()
            .any(|thread| thread.id == source_id));
    }

    #[test]
    fn selected_delete_removes_only_active_or_selected_archive() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.turns = vec![turn("first", "one")];
        meta.archive_active_and_start_empty();
        meta.turns = vec![turn("second", "two")];
        meta.archive_active_and_start_empty();
        meta.turns = vec![turn("current", "three")];
        let first_archive_id = meta.archived_threads[0].id.clone();
        let second_archive_id = meta.archived_threads[1].id.clone();
        let active_id = meta.active_thread_id.clone();
        let runtime_generation = meta.runtime_generation;

        assert!(meta.select_archive(&first_archive_id));
        let deleted_archive = meta.delete_selected_thread();
        assert_eq!(deleted_archive.id, first_archive_id);
        assert_eq!(deleted_archive.kind, DeletedFlowThreadKind::Archived);
        assert!(meta
            .archived_threads
            .iter()
            .any(|thread| thread.id == second_archive_id));
        assert_eq!(meta.active_thread_id, active_id);
        assert_eq!(meta.turns, vec![turn("current", "three")]);
        assert_eq!(meta.runtime_generation, runtime_generation);

        let deleted_active = meta.delete_selected_thread();
        assert_eq!(deleted_active.id, active_id);
        assert_eq!(deleted_active.kind, DeletedFlowThreadKind::Active);
        assert!(meta.turns.is_empty());
        assert_ne!(meta.active_thread_id, active_id);
        assert!(meta
            .archived_threads
            .iter()
            .any(|thread| thread.id == second_archive_id));
    }

    #[test]
    fn archive_navigation_preserves_the_hidden_active_draft() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.active_draft = "private draft canary".to_string();
        meta.turns.push(turn("question", "answer"));
        meta.archive_active_and_start_empty();
        let archive_id = meta.archived_threads[0].id.clone();
        assert!(meta.select_archive(&archive_id));
        assert_eq!(meta.active_draft, "private draft canary");
        meta.select_active();
        assert_eq!(meta.active_draft, "private draft canary");
    }

    #[test]
    fn flow_identity_snapshot_is_typed_redacted_and_lineage_aware() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.engine = "codex".to_string();
        meta.model = Some("gpt-test".to_string());
        meta.model_source = FlowModelSource::Runtime;
        meta.cwd = "/private/path-canary/project-alpha".to_string();
        meta.active_draft = "PRIVATE_DRAFT_CANARY".to_string();
        meta.draft_generation = 4;
        meta.runtime_generation = 7;
        meta.needs_rethread = true;
        meta.turns = vec![turn("active", "answer")];
        meta.archived_threads.push(FlowArchivedThread {
            id: "archive-child".to_string(),
            parent_thread_id: Some("missing-parent".to_string()),
            created_at: "2026-08-04T00:00:00Z".to_string(),
            archived_at: "2026-08-04T01:00:00Z".to_string(),
            inherited_turn_count: 1,
            turns: vec![turn("archived", "answer")],
        });
        meta.transcript_selection = FlowTranscriptSelection::Archived("archive-child".to_string());

        let identity = FlowSessionIdentitySnapshot::from_meta(&meta);
        assert_eq!(identity.engine, "codex");
        assert_eq!(identity.model.as_deref(), Some("gpt-test"));
        assert_eq!(identity.model_source, FlowModelSource::Runtime);
        assert_eq!(identity.cwd_display, "path-canary/project-alpha");
        assert!(!identity.cwd_fingerprint.contains("/private/"));
        assert_eq!(identity.selection, "archive");
        assert!(identity.read_only);
        assert_eq!(identity.parent_retained, Some(false));
        assert_eq!(identity.inherited_turn_count, 1);
        assert_eq!(identity.active_turn_count, 1);
        assert_eq!(identity.selected_turn_count, 1);
        assert_eq!(identity.total_turn_count, 2);
        assert_eq!(identity.draft_chars, "PRIVATE_DRAFT_CANARY".chars().count());
        assert!(!identity
            .draft_fingerprint
            .as_deref()
            .unwrap_or_default()
            .contains("PRIVATE_DRAFT_CANARY"));
        let debug = format!("{identity:?}");
        assert!(!debug.contains("/private/path-canary"));
        assert!(!debug.contains("PRIVATE_DRAFT_CANARY"));
    }

    #[test]
    fn flow_identity_origin_and_cwd_labels_are_closed_typed_sets() {
        let home = std::env::var("HOME").expect("HOME");
        assert_eq!(safe_cwd_display(&home), "~");
        assert_eq!(safe_cwd_display(""), "Working directory");
        assert_eq!(safe_cwd_display("/a/b/c"), "b/c");

        for (kind, label) in [
            (FlowOriginKind::Project, "Project"),
            (FlowOriginKind::Package, "Package"),
            (FlowOriginKind::Global, "Global"),
            (FlowOriginKind::BuiltIn, "Built-in"),
            (FlowOriginKind::Unknown, "Unknown"),
        ] {
            let mut meta = FlowSessionMeta::test_fixture();
            meta.origin_kind = kind;
            let identity = FlowSessionIdentitySnapshot::from_meta(&meta);
            assert_eq!(identity.origin_label, label);
        }
    }

    #[test]
    fn retention_copy_states_the_app_policy_without_promising_storage() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.turns = thousand_turn_fixture();
        let identity = FlowSessionIdentitySnapshot::from_meta(&meta);
        assert_eq!(
            identity.retention_text(),
            "No Script Kit turn cap · 1000 turns retained across 1 threads"
        );
    }

    #[test]
    fn persisted_flow_turn_roundtrips_outcome() {
        for turn in [
            PersistedFlowTurn {
                user: "stop".into(),
                assistant: "partial\n\n*Stopped.*".into(),
                outcome: PersistedTurnOutcome::Stopped,
                error: None,
                failure: None,
            },
            PersistedFlowTurn {
                user: "fail".into(),
                assistant: "partial".into(),
                outcome: PersistedTurnOutcome::Failed,
                error: Some("transport failed".into()),
                failure: None,
            },
            PersistedFlowTurn {
                user: "typed fail".into(),
                assistant: "partial".into(),
                outcome: PersistedTurnOutcome::Failed,
                error: None,
                failure: Some(PersistedAiFailure::from_legacy_error(
                    "protocol violation: x",
                )),
            },
        ] {
            let json = serde_json::to_string(&turn).expect("serialize persisted turn");
            let restored: PersistedFlowTurn =
                serde_json::from_str(&json).expect("deserialize persisted turn");
            assert_eq!(restored.outcome, turn.outcome);
            assert_eq!(restored.error, turn.error);
            assert_eq!(restored.failure, turn.failure);
            assert_eq!(restored.assistant, turn.assistant);
        }

        let legacy: PersistedFlowTurn =
            serde_json::from_str(r#"{"user":"old question","assistant":"old answer"}"#)
                .expect("legacy two-field turn must deserialize");
        assert_eq!(legacy.outcome, PersistedTurnOutcome::Ok);
        assert_eq!(legacy.error, None);
        assert_eq!(legacy.failure, None);
    }

    /// S09: a v4 snapshot persists ONLY the typed failure — the legacy raw
    /// caption field is never written again.
    #[test]
    fn v4_snapshots_never_write_the_legacy_error_field() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = vec![SessionTurn {
            user: "q".into(),
            assistant: "partial".into(),
            outcome: PersistedTurnOutcome::Failed,
            failure: Some(PersistedAiFailure::unknown_default()),
        }];
        persist_conversation_to(dir.path(), "project:t", "/w/flows/t.md", &turns).expect("persist");
        let raw = std::fs::read_to_string(
            dir.path()
                .join(conversation_file_name("project:t", "/w/flows/t.md")),
        )
        .expect("snapshot file");
        assert!(
            !raw.contains("\"error\""),
            "v3 must not write the legacy raw error field: {raw}"
        );
        assert!(raw.contains("\"failure\""), "typed failure must persist");
        let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).expect("parse");
        assert_eq!(snapshot.version, SNAPSHOT_VERSION);
    }

    /// S09 migration: v2 failed turns carry only the raw caption; loading
    /// classifies it into a typed record from the closed legacy string set.
    #[test]
    fn v2_legacy_errors_are_classified_while_loading() {
        use sk_protocol::ai_reliability::AiFailureCode;
        let cases = [
            (
                "mdflow CLI not found on PATH (npm i -g mdflow)",
                AiFailureCode::MdflowMissing,
                PersistedAiFailureCategory::Configuration,
            ),
            (
                "protocol violation: unknown event",
                AiFailureCode::ProtocolMalformedResponse,
                PersistedAiFailureCategory::Protocol,
            ),
            (
                "failed to spawn md: no such file",
                AiFailureCode::SpawnFailed,
                PersistedAiFailureCategory::Runtime,
            ),
            (
                "Flow definition unreadable: /w/flows/x.md (gone)",
                AiFailureCode::InvalidConfiguration,
                PersistedAiFailureCategory::Configuration,
            ),
            (
                "totally novel failure text",
                AiFailureCode::Unknown,
                PersistedAiFailureCategory::Unknown,
            ),
        ];
        for (legacy, code, category) in cases {
            let snapshot = snapshot_with(
                2,
                vec![PersistedFlowTurn {
                    user: "q".into(),
                    assistant: "partial".into(),
                    outcome: PersistedTurnOutcome::Failed,
                    error: Some(legacy.into()),
                    failure: None,
                }],
            );
            let turns = canonical_session_turns(&snapshot);
            let failure = turns[0].failure.as_ref().expect("failed turn is typed");
            assert_eq!(failure.code, code, "{legacy}");
            assert_eq!(failure.category, category, "{legacy}");
            assert_eq!(failure.safe_summary, legacy);
        }
    }

    fn snapshot_with(version: u32, turns: Vec<PersistedFlowTurn>) -> PersistedFlowConversation {
        let (active_thread_id, threads, legacy_turns) = if version >= SNAPSHOT_VERSION {
            let id = "flow-thread-test".to_string();
            (
                id.clone(),
                vec![PersistedFlowThread {
                    id,
                    state: PersistedFlowThreadState::Active,
                    parent_thread_id: None,
                    created_at: "2026-07-21T00:00:00Z".into(),
                    archived_at: None,
                    inherited_turn_count: 0,
                    turns,
                }],
                Vec::new(),
            )
        } else {
            (String::new(), Vec::new(), turns)
        };
        PersistedFlowConversation {
            flow_id: "project:test".into(),
            flow_path: "/w/flows/test.md".into(),
            saved_at: "2026-07-21T00:00:00Z".into(),
            version,
            revision: u64::from(version >= SNAPSHOT_VERSION),
            active_thread_id,
            threads,
            turns: legacy_turns,
        }
    }

    fn fixed_canonical_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .expect("fixed timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn legacy_turns_for_version(version: u32) -> Vec<PersistedFlowTurn> {
        vec![
            PersistedFlowTurn {
                user: "question one".into(),
                assistant: if version < 2 {
                    format!("partial\n\n{FLOW_STOPPED_CAPTION}")
                } else {
                    "partial".into()
                },
                outcome: PersistedTurnOutcome::Stopped,
                error: None,
                failure: None,
            },
            PersistedFlowTurn {
                user: "question two".into(),
                assistant: "answer two".into(),
                outcome: PersistedTurnOutcome::Failed,
                error: (version < 3).then(|| "protocol violation: legacy".into()),
                failure: (version >= 3).then(PersistedAiFailure::unknown_default),
            },
        ]
    }

    fn assert_legacy_migration(version: u32) {
        let raw = snapshot_with(version, legacy_turns_for_version(version));
        let canonical = canonicalize_persisted_conversation(
            raw,
            "project:test",
            "/w/flows/test.md",
            fixed_canonical_now(),
        )
        .expect("legacy version migrates")
        .snapshot;
        assert_eq!(canonical.version, SNAPSHOT_VERSION);
        assert_eq!(canonical.revision, 1);
        assert!(canonical.turns.is_empty(), "v4 never writes legacy turns");
        assert_eq!(canonical.threads.len(), 1);
        assert_eq!(canonical.active_thread_id, canonical.threads[0].id);
        assert_eq!(canonical.threads[0].state, PersistedFlowThreadState::Active);
        let turns = canonical_session_turns(&canonical);
        assert_eq!(turns.len(), 2, "migration must not lose turns");
        assert_eq!(turns[0].user, "question one");
        assert_eq!(turns[0].assistant, "partial");
        assert_eq!(turns[0].outcome, PersistedTurnOutcome::Stopped);
        assert_eq!(turns[1].user, "question two");
        assert_eq!(turns[1].assistant, "answer two");
        assert_eq!(turns[1].outcome, PersistedTurnOutcome::Failed);
        assert!(turns[1].failure.is_some());
    }

    #[test]
    fn v0_migrates_to_one_v4_active_thread_without_turn_loss() {
        assert_legacy_migration(0);
    }

    #[test]
    fn v1_migrates_to_one_v4_active_thread_without_turn_loss() {
        assert_legacy_migration(1);
    }

    #[test]
    fn v2_migrates_legacy_failures_without_turn_loss() {
        assert_legacy_migration(2);
    }

    #[test]
    fn v3_migrates_typed_failures_without_turn_loss() {
        assert_legacy_migration(3);
    }

    #[test]
    fn malformed_v4_manifest_is_canonicalized_without_turn_loss() {
        let make_turn = |label: &str| PersistedFlowTurn {
            user: format!("user-{label}"),
            assistant: format!("assistant-{label}"),
            outcome: PersistedTurnOutcome::Ok,
            error: None,
            failure: None,
        };
        let mut raw = PersistedFlowConversation {
            flow_id: "project:test".into(),
            flow_path: "/w/flows/test.md".into(),
            saved_at: "not-a-time".into(),
            version: SNAPSHOT_VERSION,
            revision: 0,
            active_thread_id: "duplicate".into(),
            threads: vec![
                PersistedFlowThread {
                    id: "duplicate".into(),
                    state: PersistedFlowThreadState::Archived,
                    parent_thread_id: Some("duplicate".into()),
                    created_at: "bad".into(),
                    archived_at: None,
                    inherited_turn_count: 99,
                    turns: vec![make_turn("first")],
                },
                PersistedFlowThread {
                    id: "duplicate".into(),
                    state: PersistedFlowThreadState::Active,
                    parent_thread_id: Some("missing-parent".into()),
                    created_at: "2026-08-04T10:00:00Z".into(),
                    archived_at: Some("2026-08-04T09:00:00Z".into()),
                    inherited_turn_count: 1,
                    turns: vec![make_turn("second")],
                },
                PersistedFlowThread {
                    id: String::new(),
                    state: PersistedFlowThreadState::Active,
                    parent_thread_id: None,
                    created_at: "bad".into(),
                    archived_at: None,
                    inherited_turn_count: 0,
                    turns: vec![make_turn("third")],
                },
            ],
            turns: vec![make_turn("ignored-top-level")],
        };
        let canonical = canonicalize_persisted_conversation(
            raw.clone(),
            "project:test",
            "/w/flows/test.md",
            fixed_canonical_now(),
        )
        .expect("malformed v4 is repairable")
        .snapshot;
        assert_eq!(canonical.flow_id, "project:test");
        assert_eq!(canonical.flow_path, "/w/flows/test.md");
        assert_eq!(canonical.revision, 1);
        assert_eq!(canonical.threads.len(), 3);
        assert_eq!(
            canonical
                .threads
                .iter()
                .map(|thread| thread.turns.len())
                .sum::<usize>(),
            3,
            "non-empty v4 threads are authoritative; top-level turns are ignored"
        );
        assert!(canonical.turns.is_empty());
        assert_eq!(
            canonical
                .threads
                .iter()
                .filter(|thread| thread.state == PersistedFlowThreadState::Active)
                .count(),
            1
        );
        assert_eq!(
            canonical.active_thread_id,
            canonical.threads.last().unwrap().id
        );
        let unique: std::collections::HashSet<_> =
            canonical.threads.iter().map(|thread| &thread.id).collect();
        assert_eq!(unique.len(), canonical.threads.len());
        assert!(canonical.threads.iter().all(|thread| !thread.id.is_empty()));
        assert!(canonical
            .threads
            .iter()
            .all(|thread| thread.inherited_turn_count <= thread.turns.len()));
        assert!(canonical.threads.last().unwrap().archived_at.is_none());

        raw.version = SNAPSHOT_VERSION + 1;
        assert_eq!(
            canonicalize_persisted_conversation(
                raw,
                "project:test",
                "/w/flows/test.md",
                fixed_canonical_now(),
            ),
            Err(PersistedConversationLoadError::FutureVersion(
                SNAPSHOT_VERSION + 1
            ))
        );
    }

    #[test]
    fn empty_active_metadata_is_present_while_missing_store_is_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:empty", "/w/flows/empty.md")
                .is_none()
        );
        persist_conversation_to(dir.path(), "project:empty", "/w/flows/empty.md", &[])
            .expect("persist empty active metadata");
        let loaded =
            load_persisted_conversation_from(dir.path(), "project:empty", "/w/flows/empty.md")
                .expect("empty active metadata must remain present");
        assert_eq!(loaded.threads.len(), 1);
        assert!(canonical_session_turns(&loaded).is_empty());
    }

    /// WP-A4: canonical conversion migrates transitional caption-bearing
    /// Stopped records to raw text and normalizes outcome/error invariants.
    #[test]
    fn canonical_session_turns_migrates_and_normalizes() {
        let snapshot = snapshot_with(
            0,
            vec![
                // Transitional Phase-A record: caption baked into assistant.
                PersistedFlowTurn {
                    user: "stop".into(),
                    assistant: format!("partial\n\n{FLOW_STOPPED_CAPTION}"),
                    outcome: PersistedTurnOutcome::Stopped,
                    error: None,
                    failure: None,
                },
                // Caption-only stopped record (empty raw output).
                PersistedFlowTurn {
                    user: "stop2".into(),
                    assistant: FLOW_STOPPED_CAPTION.into(),
                    outcome: PersistedTurnOutcome::Stopped,
                    error: None,
                    failure: None,
                },
                // Failed with blank error → nonblank typed fallback.
                PersistedFlowTurn {
                    user: "fail".into(),
                    assistant: "partial".into(),
                    outcome: PersistedTurnOutcome::Failed,
                    error: Some("   ".into()),
                    failure: None,
                },
                // Stopped with an impossible error → dropped.
                PersistedFlowTurn {
                    user: "odd".into(),
                    assistant: "text".into(),
                    outcome: PersistedTurnOutcome::Stopped,
                    error: Some("junk".into()),
                    failure: None,
                },
            ],
        );
        let turns = canonical_session_turns(&snapshot);
        assert_eq!(turns[0].assistant, "partial");
        assert_eq!(turns[0].outcome, PersistedTurnOutcome::Stopped);
        assert_eq!(turns[1].assistant, "");
        assert_eq!(
            turns[2].failure.as_ref().map(|f| f.safe_summary.as_str()),
            Some(FLOW_TURN_FAILED_SUMMARY)
        );
        assert_eq!(turns[3].failure, None, "Stopped never carries a failure");
    }

    /// Current-version snapshots are NOT caption-stripped: raw text that
    /// legitimately ends with the caption phrase stays verbatim.
    #[test]
    fn canonical_session_turns_leaves_current_version_raw() {
        let raw = format!("The literal marker is\n\n{FLOW_STOPPED_CAPTION}");
        let snapshot = snapshot_with(
            SNAPSHOT_VERSION,
            vec![PersistedFlowTurn {
                user: "u".into(),
                assistant: raw.clone(),
                outcome: PersistedTurnOutcome::Stopped,
                error: None,
                failure: None,
            }],
        );
        assert_eq!(canonical_session_turns(&snapshot)[0].assistant, raw);
    }

    fn turn(user: &str, assistant: &str) -> SessionTurn {
        SessionTurn {
            user: user.into(),
            assistant: assistant.into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }
    }

    fn store_snapshot(revision: u64, turns: Vec<SessionTurn>) -> PersistedFlowConversation {
        let mut snapshot = snapshot_from_turns("project:t", "/w/flows/t.md", &turns);
        snapshot.revision = revision;
        snapshot
    }

    #[test]
    fn conversation_store_newer_revision_wins() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        assert_eq!(
            store.persist_snapshot_and_wait(store_snapshot(1, vec![turn("q1", "a1")])),
            Ok(ConversationStoreReceipt::Written)
        );
        assert_eq!(
            store.persist_snapshot_and_wait(store_snapshot(
                2,
                vec![turn("q1", "a1"), turn("q2", "a2")],
            )),
            Ok(ConversationStoreReceipt::Written)
        );
        assert_eq!(
            store.persist_snapshot_and_wait(store_snapshot(1, vec![turn("old", "old")])),
            Ok(ConversationStoreReceipt::IgnoredStaleRevision)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("snapshot present");
        assert_eq!(loaded.revision, 2);
        assert_eq!(canonical_session_turns(&loaded).len(), 2);
    }

    fn snapshot_with_archive(revision: u64) -> PersistedFlowConversation {
        let mut snapshot = store_snapshot(revision, vec![turn("active", "answer")]);
        snapshot.threads.insert(
            0,
            PersistedFlowThread {
                id: "archive-b".into(),
                state: PersistedFlowThreadState::Archived,
                parent_thread_id: None,
                created_at: "2026-08-04T10:00:00Z".into(),
                archived_at: Some("2026-08-04T11:00:00Z".into()),
                inherited_turn_count: 0,
                turns: vec![PersistedFlowTurn::from(&turn("archived", "answer"))],
            },
        );
        snapshot
    }

    #[test]
    fn selected_thread_tombstone_rejects_late_stale_persist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let held = snapshot_with_archive(1);
        assert_eq!(
            store.persist_snapshot_and_wait(held.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut deletion = held.clone();
        deletion.revision = 2;
        deletion.threads.retain(|thread| thread.id != "archive-b");
        assert_eq!(
            store.persist_selected_deletion_and_wait(deletion, "archive-b".into()),
            Ok(ConversationStoreReceipt::Written)
        );
        assert_eq!(
            store.persist_snapshot_and_wait(held),
            Ok(ConversationStoreReceipt::IgnoredStaleRevision)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("manifest remains present");
        assert_eq!(loaded.revision, 2);
        assert!(loaded.threads.iter().all(|thread| thread.id != "archive-b"));
    }

    #[test]
    fn selected_thread_tombstone_rejects_forged_higher_revision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let original = snapshot_with_archive(1);
        assert_eq!(
            store.persist_snapshot_and_wait(original.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut deletion = original.clone();
        deletion.revision = 2;
        deletion.threads.retain(|thread| thread.id != "archive-b");
        assert_eq!(
            store.persist_selected_deletion_and_wait(deletion, "archive-b".into()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut forged = original;
        forged.revision = 3;
        assert_eq!(
            store.persist_snapshot_and_wait(forged),
            Ok(ConversationStoreReceipt::IgnoredTombstonedThread)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("manifest remains present");
        assert_eq!(loaded.revision, 2);
        assert!(loaded.threads.iter().all(|thread| thread.id != "archive-b"));
    }

    #[test]
    fn deleting_active_persists_one_empty_replacement_thread() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let original = store_snapshot(1, vec![turn("active", "answer")]);
        let deleted_id = original.active_thread_id.clone();
        assert_eq!(
            store.persist_snapshot_and_wait(original.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut replacement = snapshot_from_turns("project:t", "/w/flows/t.md", &[]);
        replacement.revision = 2;
        replacement.active_thread_id = "active-replacement".into();
        replacement.threads[0].id = replacement.active_thread_id.clone();
        assert_eq!(
            store.persist_selected_deletion_and_wait(replacement, deleted_id.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("empty replacement remains present");
        assert_eq!(loaded.threads.len(), 1);
        assert_ne!(loaded.active_thread_id, deleted_id);
        assert!(canonical_session_turns(&loaded).is_empty());
    }

    #[test]
    fn deleting_archive_preserves_active_and_other_archives() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let mut original = snapshot_with_archive(1);
        original.threads.insert(
            1,
            PersistedFlowThread {
                id: "archive-c".into(),
                state: PersistedFlowThreadState::Archived,
                parent_thread_id: None,
                created_at: "2026-08-04T10:00:00Z".into(),
                archived_at: Some("2026-08-04T11:00:00Z".into()),
                inherited_turn_count: 0,
                turns: vec![PersistedFlowTurn::from(&turn("other", "answer"))],
            },
        );
        let active_id = original.active_thread_id.clone();
        assert_eq!(
            store.persist_snapshot_and_wait(original.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut deletion = original;
        deletion.revision = 2;
        deletion.threads.retain(|thread| thread.id != "archive-b");
        assert_eq!(
            store.persist_selected_deletion_and_wait(deletion, "archive-b".into()),
            Ok(ConversationStoreReceipt::Written)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("manifest remains present");
        assert_eq!(loaded.active_thread_id, active_id);
        assert!(loaded.threads.iter().any(|thread| thread.id == "archive-c"));
        assert!(loaded.threads.iter().all(|thread| thread.id != "archive-b"));
    }

    /// WP-A3: rollup is outcome-aware — stopped/failed partials are labeled
    /// as partial, and the UI caption never enters the engine prompt.
    #[test]
    fn build_turn_task_labels_partial_outcomes_and_excludes_caption() {
        let turns = vec![
            SessionTurn {
                user: "q1".into(),
                assistant: "full answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            },
            SessionTurn {
                user: "q2".into(),
                assistant: "cut short".into(),
                outcome: PersistedTurnOutcome::Stopped,
                failure: None,
            },
            SessionTurn {
                user: "q3".into(),
                assistant: "broke".into(),
                outcome: PersistedTurnOutcome::Failed,
                failure: Some(PersistedAiFailure::from_legacy_error("transport exploded")),
            },
        ];
        let task = build_turn_task(&turns, "next question");
        assert!(task.contains("Assistant: full answer"));
        assert!(task.contains("Assistant (partial; turn stopped): cut short"));
        assert!(task.contains("Assistant (partial; turn failed): broke"));
        assert!(
            !task.contains(FLOW_STOPPED_CAPTION),
            "UI caption must never enter the engine prompt"
        );
        assert!(
            !task.contains("transport exploded"),
            "transport error text must never enter the engine prompt"
        );
    }

    #[test]
    fn persisted_conversation_distinguishes_missing_from_empty_active() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .is_none()
        );
        persist_conversation_to(dir.path(), "project:scout", "/a/flows/scout.md", &[])
            .expect("persist empty");
        let empty =
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .expect("empty active metadata is persisted");
        assert!(canonical_session_turns(&empty).is_empty());
    }

    /// Two projects with the same `project:review` id must never share a
    /// transcript (2026-07-11 audit P0: cross-project restore was both a
    /// correctness and privacy failure).
    #[test]
    fn same_flow_id_in_different_projects_gets_separate_transcripts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turn = |text: &str| {
            vec![SessionTurn {
                user: text.to_string(),
                assistant: format!("re: {text}"),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            }]
        };
        persist_conversation_to(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
            &turn("alpha secrets"),
        )
        .expect("persist alpha");
        persist_conversation_to(
            dir.path(),
            "project:review",
            "/work/beta/flows/review.md",
            &turn("beta question"),
        )
        .expect("persist beta");

        let alpha = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .expect("alpha loads");
        let beta = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/beta/flows/review.md",
        )
        .expect("beta loads");
        assert_eq!(canonical_session_turns(&alpha)[0].user, "alpha secrets");
        assert_eq!(canonical_session_turns(&beta)[0].user, "beta question");
    }

    #[test]
    fn conversation_identity_digest_prevents_lossy_slug_and_truncated_path_collisions() {
        let punctuated_path = "/work/a-b/flows/review.md";
        let nested_path = "/work/a/b-flows/review.md";
        assert_eq!(
            legacy_path_qualified_conversation_file_name("project:review", punctuated_path),
            legacy_path_qualified_conversation_file_name("project:review", nested_path),
            "the old filesystem slug really collides"
        );
        assert_ne!(
            conversation_file_name("project:review", punctuated_path),
            conversation_file_name("project:review", nested_path),
            "full original paths must own distinct private files"
        );

        let shared_tail = format!("/{}/review.md", "x".repeat(180));
        let alpha = format!("/work/alpha{shared_tail}");
        let beta = format!("/work/beta{shared_tail}");
        assert_eq!(
            legacy_path_qualified_conversation_file_name("project:review", &alpha),
            legacy_path_qualified_conversation_file_name("project:review", &beta),
            "the old 160-byte tail discards the distinct projects"
        );
        assert_ne!(
            conversation_file_name("project:review", &alpha),
            conversation_file_name("project:review", &beta)
        );
        assert_ne!(
            conversation_file_name("project:a/b", punctuated_path),
            conversation_file_name("project:a-b", punctuated_path),
            "flow IDs cannot collide after filesystem sanitization either"
        );
        assert!(
            conversation_file_name(&"i".repeat(500), &"p".repeat(500)).len() <= 255,
            "long identities must remain valid on conservative filesystems"
        );
    }

    #[test]
    fn colliding_legacy_flow_paths_keep_their_conversations_isolated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let alpha_path = "/work/a-b/flows/review.md";
        let beta_path = "/work/a/b-flows/review.md";
        let turn = |user: &str| {
            vec![SessionTurn {
                user: user.to_string(),
                assistant: "private answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            }]
        };

        persist_conversation_to(
            dir.path(),
            "project:review",
            alpha_path,
            &turn("alpha secret"),
        )
        .expect("persist alpha");
        persist_conversation_to(
            dir.path(),
            "project:review",
            beta_path,
            &turn("beta secret"),
        )
        .expect("persist beta");

        let alpha = load_persisted_conversation_from(dir.path(), "project:review", alpha_path)
            .expect("alpha conversation");
        let beta = load_persisted_conversation_from(dir.path(), "project:review", beta_path)
            .expect("beta conversation");
        assert_eq!(canonical_session_turns(&alpha)[0].user, "alpha secret");
        assert_eq!(canonical_session_turns(&beta)[0].user, "beta secret");
    }

    #[cfg(unix)]
    #[test]
    fn private_flow_snapshots_are_owner_only_and_repair_existing_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated flow privacy fixture");
        let flow_id = "project:private";
        let flow_path = "/workspace/private/flow.md";
        let turns = vec![SessionTurn {
            user: "private flow question".to_owned(),
            assistant: "private flow answer".to_owned(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        persist_conversation_to(directory.path(), flow_id, flow_path, &turns)
            .expect("private flow persistence");
        let path = directory
            .path()
            .join(conversation_file_name(flow_id, flow_path));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let loaded = load_persisted_conversation_from(directory.path(), flow_id, flow_path)
            .expect("legacy permissions repair before transcript exposure");
        assert_eq!(
            canonical_session_turns(&loaded)[0].user,
            "private flow question"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_flow_snapshots_refuse_primary_and_both_legacy_symlink_targets() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated flow symlink fixture");
        let flow_id = "project:private";
        let flow_path = "/workspace/private/flow.md";
        let external = directory.path().join("another-project.json");
        let snapshot = snapshot_from_turns(flow_id, flow_path, &[]);
        let original = serde_json::to_vec_pretty(&snapshot).unwrap();
        std::fs::write(&external, &original).unwrap();

        let primary = directory
            .path()
            .join(conversation_file_name(flow_id, flow_path));
        symlink(&external, &primary).unwrap();
        assert!(load_persisted_conversation_from(directory.path(), flow_id, flow_path).is_none());
        assert!(persist_conversation_snapshot_to(directory.path(), &snapshot).is_err());
        assert_eq!(std::fs::read(&external).unwrap(), original);
        std::fs::remove_file(&primary).unwrap();

        let qualified = directory
            .path()
            .join(legacy_path_qualified_conversation_file_name(
                flow_id, flow_path,
            ));
        symlink(&external, &qualified).unwrap();
        assert!(load_persisted_conversation_from(directory.path(), flow_id, flow_path).is_none());
        assert!(std::fs::symlink_metadata(&qualified)
            .unwrap()
            .file_type()
            .is_symlink());
        std::fs::remove_file(&qualified).unwrap();

        let legacy = directory
            .path()
            .join(legacy_conversation_file_name(flow_id));
        symlink(&external, &legacy).unwrap();
        assert!(load_persisted_conversation_from(directory.path(), flow_id, flow_path).is_none());
        assert!(std::fs::symlink_metadata(&legacy)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(&external).unwrap(), original);
    }

    #[test]
    fn canonical_conversation_refuses_foreign_flow_identity() {
        let snapshot = snapshot_with(SNAPSHOT_VERSION, Vec::new());
        assert_eq!(
            canonicalize_persisted_conversation(
                snapshot.clone(),
                "project:another",
                "/w/flows/test.md",
                fixed_canonical_now(),
            ),
            Err(PersistedConversationLoadError::IdentityMismatch)
        );
        assert_eq!(
            canonicalize_persisted_conversation(
                snapshot,
                "project:test",
                "/w/another/test.md",
                fixed_canonical_now(),
            ),
            Err(PersistedConversationLoadError::IdentityMismatch)
        );
    }

    #[test]
    fn foreign_snapshot_at_primary_path_is_never_returned_or_rewritten() {
        let dir = tempfile::tempdir().expect("tempdir");
        let foreign = snapshot_from_turns("project:foreign", "/w/foreign.md", &[]);
        let path = dir
            .path()
            .join(conversation_file_name("project:expected", "/w/expected.md"));
        let original = serde_json::to_vec_pretty(&foreign).expect("serialize foreign snapshot");
        std::fs::write(&path, &original).expect("install foreign snapshot");

        assert!(
            load_persisted_conversation_from(dir.path(), "project:expected", "/w/expected.md")
                .is_none()
        );
        assert_eq!(
            std::fs::read(&path).expect("foreign snapshot remains untouched"),
            original
        );
    }

    #[test]
    fn path_qualified_legacy_snapshot_migrates_only_for_its_exact_owner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let owner_path = "/work/a-b/flows/review.md";
        let colliding_path = "/work/a/b-flows/review.md";
        let turns = vec![SessionTurn {
            user: "owner-only secret".into(),
            assistant: "owner-only answer".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        let snapshot = snapshot_from_turns("project:review", owner_path, &turns);
        let legacy = dir
            .path()
            .join(legacy_path_qualified_conversation_file_name(
                "project:review",
                owner_path,
            ));
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();

        assert!(
            load_persisted_conversation_from(dir.path(), "project:review", colliding_path)
                .is_none(),
            "a colliding slug cannot claim another project's snapshot"
        );
        assert!(
            legacy.exists(),
            "refusal must not destroy the owner's history"
        );

        let migrated = load_persisted_conversation_from(dir.path(), "project:review", owner_path)
            .expect("the exact owner adopts its own legacy snapshot");
        assert_eq!(
            canonical_session_turns(&migrated)[0].user,
            "owner-only secret"
        );
        assert!(!legacy.exists(), "legacy name is consumed exactly once");
        assert!(
            dir.path()
                .join(conversation_file_name("project:review", owner_path))
                .exists(),
            "the collision-resistant destination owns the migrated transcript"
        );
    }

    /// Legacy id-only snapshots are adopted once (re-keyed under the
    /// path-qualified name, legacy file removed) so they can never leak
    /// into a second project later.
    #[test]
    fn legacy_snapshot_is_adopted_once_and_removed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let legacy = dir
            .path()
            .join(legacy_conversation_file_name("project:review"));
        let snapshot = PersistedFlowConversation {
            flow_id: "project:review".into(),
            flow_path: String::new(),
            saved_at: "2026-07-10T00:00:00Z".into(),
            version: 0,
            revision: 0,
            active_thread_id: String::new(),
            threads: Vec::new(),
            turns: vec![PersistedFlowTurn {
                user: "old question".into(),
                assistant: "old answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                error: None,
                failure: None,
            }],
        };
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();

        let adopted = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .expect("legacy snapshot adopted");
        assert_eq!(canonical_session_turns(&adopted)[0].user, "old question");
        assert_eq!(adopted.flow_path, "/work/alpha/flows/review.md");
        assert!(!legacy.exists(), "legacy file must be consumed");
        assert!(
            load_persisted_conversation_from(
                dir.path(),
                "project:review",
                "/work/beta/flows/review.md",
            )
            .is_none(),
            "a second project must not inherit the adopted transcript"
        );
    }

    #[test]
    fn id_only_legacy_snapshot_with_foreign_flow_id_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let legacy = dir
            .path()
            .join(legacy_conversation_file_name("project:review"));
        let mut snapshot = snapshot_with(0, legacy_turns_for_version(0));
        snapshot.flow_id = "project:someone-else".into();
        snapshot.flow_path.clear();
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();

        assert!(load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .is_none());
        assert!(
            legacy.exists(),
            "foreign history must never be claimed or removed"
        );
    }

    #[test]
    fn failed_legacy_claim_never_returns_or_shares_private_turns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let flow_id = "project:review";
        let flow_path = "/work/alpha/flows/review.md";
        let legacy = dir.path().join(legacy_conversation_file_name(flow_id));
        let snapshot = PersistedFlowConversation {
            flow_id: flow_id.into(),
            flow_path: String::new(),
            saved_at: "2026-07-10T00:00:00Z".into(),
            version: 0,
            revision: 0,
            active_thread_id: String::new(),
            threads: Vec::new(),
            turns: vec![PersistedFlowTurn {
                user: "unclaimed private turn".into(),
                assistant: "unclaimed private answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                error: None,
                failure: None,
            }],
        };
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();
        let destination = dir.path().join(conversation_file_name(flow_id, flow_path));
        std::fs::create_dir(&destination).expect("block the atomic file claim");

        assert!(
            load_persisted_conversation_from(dir.path(), flow_id, flow_path).is_none(),
            "private turns cannot be exposed before their old shared name is consumed"
        );
        assert!(
            legacy.exists(),
            "failed migration preserves the original history"
        );

        std::fs::remove_dir(&destination).expect("remove isolated test obstruction");
        let adopted = load_persisted_conversation_from(dir.path(), flow_id, flow_path)
            .expect("history recovers after a later successful atomic claim");
        assert_eq!(
            canonical_session_turns(&adopted)[0].user,
            "unclaimed private turn"
        );
        assert!(!legacy.exists());
    }

    #[test]
    fn later_turns_roll_up_history_then_message() {
        let turns = vec![SessionTurn {
            user: "find bun shell examples".into(),
            assistant: "Here are three repos …".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        let task = build_turn_task(&turns, "show me the second one");
        assert!(task.starts_with("Conversation so far"));
        assert!(task.contains("User: find bun shell examples"));
        assert!(task.contains("Assistant: Here are three repos …"));
        assert!(task.ends_with("show me the second one"));
    }

    #[test]
    fn history_budget_drops_oldest_turns_first() {
        let big = "x".repeat(6_000);
        let turns = vec![
            SessionTurn {
                user: "oldest".into(),
                assistant: big.clone(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            },
            SessionTurn {
                user: "newest".into(),
                assistant: big,
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            },
        ];
        let task = build_turn_task(&turns, "next");
        assert!(!task.contains("oldest"));
        assert!(task.contains("newest"));
        assert!(task.ends_with("next"));
    }

    #[test]
    fn templated_flow_contract_keeps_mission_in_first_prompt() {
        let markdown =
            "---\ndescription: GitHub examples\n---\nSearch GitHub for examples.\n\n{{ _task }}\n";
        let contract = resolve_flow_thread_contract(markdown, "bun shell scripts");
        assert_eq!(contract.profile.developer_instructions, None);
        assert_eq!(
            contract.first_prompt,
            "Search GitHub for examples.\n\nbun shell scripts"
        );
    }

    #[test]
    fn plain_flow_contract_pins_mission_as_developer_instructions() {
        let markdown = "---\ndescription: Terse\n---\nYou are gmail-agent. Reply tersely.\n";
        let contract = resolve_flow_thread_contract(markdown, "what did vercel email me?");
        assert_eq!(
            contract.profile.developer_instructions.as_deref(),
            Some("You are gmail-agent. Reply tersely.")
        );
        assert_eq!(contract.first_prompt, "what did vercel email me?");
    }

    #[test]
    fn empty_body_contract_sends_task_verbatim_with_no_instructions() {
        let contract = resolve_flow_thread_contract("---\nmodel: gpt-5\n---\n", "hello");
        assert_eq!(contract.profile.developer_instructions, None);
        assert_eq!(contract.first_prompt, "hello");
    }

    #[test]
    fn frontmatter_model_and_sandbox_pass_through_with_quotes_stripped() {
        let markdown =
            "---\nmodel: \"gpt-5.6-sol\"\nsandbox: 'read-only'\nother: x\n---\nMission.\n";
        let contract = resolve_flow_thread_contract(markdown, "go");
        assert_eq!(contract.profile.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(contract.profile.sandbox.as_deref(), Some("read-only"));
        let bare = resolve_flow_thread_contract("Mission.", "go");
        assert_eq!(bare.profile.model, None);
        assert_eq!(bare.profile.sandbox, None);
    }

    #[test]
    fn rethread_contract_carries_mission_and_transcript_rollup() {
        // Engine died mid-conversation: the submit path resolves the
        // contract again with build_turn_task(turns, message) as the task,
        // so the fresh thread gets BOTH the flow's identity and the prior
        // conversation — never a generic new thread.
        let turns = vec![SessionTurn {
            user: "find bun shell examples".into(),
            assistant: "Here are three repos …".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        let rollup = build_turn_task(&turns, "show me the second one");

        let plain = resolve_flow_thread_contract("You are repo-scout.", &rollup);
        assert_eq!(
            plain.profile.developer_instructions.as_deref(),
            Some("You are repo-scout.")
        );
        assert!(plain.first_prompt.contains("User: find bun shell examples"));
        assert!(plain.first_prompt.ends_with("show me the second one"));

        let templated = resolve_flow_thread_contract("Scout repos.\n\n{{ _task }}", &rollup);
        assert!(templated.first_prompt.starts_with("Scout repos."));
        assert!(templated
            .first_prompt
            .contains("Assistant: Here are three repos …"));
    }

    fn active_turn(assistant_acc: &str) -> ActiveTurn {
        ActiveTurn {
            run_id: None,
            message_id: "m".into(),
            assistant_acc: assistant_acc.into(),
            current_item_id: None,
            item_acc: String::new(),
            user_text: "u".into(),
        }
    }

    #[test]
    fn entering_a_new_item_after_text_needs_a_paragraph_break() {
        let mut turn = active_turn("First item ends with a period.");
        turn.current_item_id = Some("item-1".into());
        turn.item_acc = "First item ends with a period.".into();
        assert!(turn.enter_item("item-2"));
        assert_eq!(turn.item_acc, "");
        assert_eq!(turn.current_item_id.as_deref(), Some("item-2"));
    }

    #[test]
    fn same_item_and_first_item_never_break() {
        let mut turn = active_turn("");
        assert!(
            !turn.enter_item("item-1"),
            "first item: nothing to separate"
        );
        turn.item_acc = "streaming".into();
        assert!(!turn.enter_item("item-1"), "same item: no-op");
        assert_eq!(
            turn.item_acc, "streaming",
            "same item must keep its accumulator"
        );
    }

    #[test]
    fn existing_paragraph_break_is_not_doubled() {
        let mut turn = active_turn("First item.\n\n");
        turn.current_item_id = Some("item-1".into());
        assert!(!turn.enter_item("item-2"));
    }

    /// S09: the reducer-driven session reliability state makes a failed turn
    /// actionable (AwaitingRecovery with a Retry path), keeps a user Stop
    /// quiet (no recovery card), and makes an idle engine death actionable
    /// through the outside-turn projection.
    #[test]
    fn flow_reliability_failure_is_actionable_and_stop_stays_quiet() {
        use sk_protocol::ai_reliability::{
            AiFailure, AiFailureKind, AiPhaseTag, RetrySafety, RuntimeFailure,
        };
        let failure = || {
            AiFailure::new(
                AiFailureKind::Runtime(RuntimeFailure::RuntimeClosed),
                RetrySafety::SameSelectionReadOnly,
            )
        };

        // Failed turn → actionable recovery → manual Retry reaches Running.
        let mut failed = FlowReliability::new("project:test", "/w/flows/test.md", "codex");
        failed.begin_turn("project:test", "/w/flows/test.md", 0);
        assert_eq!(failed.state().phase.tag(), AiPhaseTag::Running);
        failed.fail_turn(failure());
        assert!(failed.awaiting_recovery(), "failure must become actionable");
        assert!(
            failed.retry_turn("project:test", 0),
            "manual retry must be accepted"
        );
        assert_eq!(failed.state().phase.tag(), AiPhaseTag::Running);

        // User stop → truthful cancellation, never the recovery treatment.
        let mut stopped = FlowReliability::new("project:test", "/w/flows/test.md", "codex");
        stopped.begin_turn("project:test", "/w/flows/test.md", 0);
        stopped.cancel_turn(true);
        assert_eq!(stopped.state().phase.tag(), AiPhaseTag::Cancelled);
        assert!(!stopped.awaiting_recovery(), "stop must stay quiet");

        // Engine death while idle → actionable without fabricating a turn.
        let mut idle = FlowReliability::new("project:test", "/w/flows/test.md", "codex");
        idle.fail_outside_turn(failure());
        assert!(idle.awaiting_recovery());

        // Rethread selection acknowledges through the reducer.
        assert!(idle.select_rethread(), "rethread must be selectable");
    }

    /// S09: persisted failures round-trip codes through `to_failure`, so
    /// restore-time recovery planning stays code-accurate.
    #[test]
    fn persisted_failure_round_trips_code_through_to_failure() {
        use sk_protocol::ai_reliability::AiFailureCode;
        for legacy in [
            "mdflow CLI not found on PATH (npm i -g mdflow)",
            "protocol violation: junk",
            "totally novel",
        ] {
            let persisted = PersistedAiFailure::from_legacy_error(legacy);
            assert_eq!(persisted.to_failure().code, persisted.code, "{legacy}");
        }
        assert_eq!(
            PersistedAiFailure::unknown_default().to_failure().code,
            AiFailureCode::Unknown
        );
    }

    #[test]
    fn state_labels_are_honest() {
        assert_eq!(SessionState::Working.label(), "working");
        assert_eq!(SessionState::NeedsYou.label(), "needs you");
        assert_eq!(SessionState::Done(Some(0)).label(), "done");
        assert_eq!(SessionState::Done(Some(1)).label(), "failed");
        assert!(SessionState::Working.is_live());
        assert!(SessionState::NeedsYou.is_live());
        assert!(!SessionState::Done(None).is_live());
    }
}

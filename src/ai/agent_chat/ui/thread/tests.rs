use super::*;

/// Helper to build an `AgentChatThread` without a real connection or GPUI context.
/// Only for testing pure logic methods that don't need cx or connection.
fn fork_point(entry_id: &str, text: &str) -> super::super::events::AgentChatForkPoint {
    super::super::events::AgentChatForkPoint {
        entry_id: entry_id.to_string(),
        text: text.to_string(),
    }
}

#[test]
fn cwd_resolution_decision_respawns_only_when_idle_or_error() {
    let current = Path::new("/tmp/old");
    let selected = Path::new("/tmp/new");

    assert_eq!(
        decide_agent_chat_cwd_resolution(current, current, AgentChatThreadStatus::Streaming),
        AgentChatCwdResolutionDecision::Unchanged
    );
    assert_eq!(
        decide_agent_chat_cwd_resolution(current, selected, AgentChatThreadStatus::Idle),
        AgentChatCwdResolutionDecision::RespawnNow
    );
    assert_eq!(
        decide_agent_chat_cwd_resolution(current, selected, AgentChatThreadStatus::Error),
        AgentChatCwdResolutionDecision::RespawnNow
    );
}

#[test]
fn cwd_resolution_decision_blocks_in_flight_turns() {
    let current = Path::new("/tmp/old");
    let selected = Path::new("/tmp/new");

    assert_eq!(
        decide_agent_chat_cwd_resolution(current, selected, AgentChatThreadStatus::Streaming),
        AgentChatCwdResolutionDecision::BlockInFlight
    );
    assert_eq!(
        decide_agent_chat_cwd_resolution(
            current,
            selected,
            AgentChatThreadStatus::WaitingForPermission
        ),
        AgentChatCwdResolutionDecision::BlockInFlight
    );
}

#[test]
fn fork_points_event_replaces_rewind_list() {
    let mut thread = test_thread(Vec::new(), false);
    thread.apply_event_test(AgentChatEvent::ForkPointsAvailable {
        entries: vec![
            fork_point("e0", "first ask"),
            fork_point("e1", "second ask"),
        ],
    });
    assert_eq!(thread.fork_points().len(), 2);
    assert_eq!(thread.fork_points()[0].entry_id, "e0");

    thread.apply_event_test(AgentChatEvent::ForkPointsAvailable {
        entries: vec![fork_point("e0", "first ask")],
    });
    assert_eq!(
        thread.fork_points().len(),
        1,
        "list is replaced, not appended"
    );
}

#[test]
fn fork_completed_truncates_at_user_ordinal_and_prefills_composer() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "first answer");
    thread.push_message(AgentChatThreadMessageRole::User, "second ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "second answer");
    thread.fork_points = vec![
        fork_point("e0", "first ask"),
        fork_point("e1", "second ask"),
    ];
    thread.pending_fork_ordinal = Some(1);

    thread.apply_event_test(AgentChatEvent::ForkCompleted {
        text: "second ask".to_string(),
    });

    assert_eq!(
        thread.messages.len(),
        2,
        "second user message and its answer are dropped"
    );
    assert_eq!(thread.messages[0].body.as_ref(), "first ask");
    assert_eq!(thread.messages[1].body.as_ref(), "first answer");
    assert_eq!(thread.input.text(), "second ask");
    assert_eq!(thread.status, AgentChatThreadStatus::Idle);
    assert!(thread.pending_fork_ordinal.is_none());
}

#[test]
fn fork_completed_without_pending_request_is_ignored() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "only ask");

    thread.apply_event_test(AgentChatEvent::ForkCompleted {
        text: "stray".to_string(),
    });

    assert_eq!(thread.messages.len(), 1, "transcript untouched");
    assert!(thread.input.text().is_empty(), "composer untouched");
}

#[test]
fn fork_point_for_message_id_maps_by_user_ordinal() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "first answer");
    thread.push_message(AgentChatThreadMessageRole::User, "second ask");
    let second_user_id = thread.messages[2].id;
    let fork_points = vec![
        fork_point("entry-0", "stale first text from pi"),
        fork_point("entry-1", "stale second text from pi"),
    ];

    let point =
        AgentChatThread::fork_point_for_message_id(&thread.messages, &fork_points, second_user_id)
            .expect("second user message should resolve by ordinal");

    assert_eq!(point.entry_id, "entry-1");
}

#[test]
fn fork_point_for_message_id_falls_back_to_text_when_lengths_mismatch() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "first answer");
    thread.push_message(AgentChatThreadMessageRole::User, "second ask");
    let second_user_id = thread.messages[2].id;
    let fork_points = vec![fork_point("entry-second", "second ask")];

    let point =
        AgentChatThread::fork_point_for_message_id(&thread.messages, &fork_points, second_user_id)
            .expect("mismatched fork list should resolve by exact text");

    assert_eq!(point.entry_id, "entry-second");
}

#[test]
fn fork_point_for_message_id_returns_none_when_unresolvable() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    let first_user_id = thread.messages[0].id;
    let fork_points = Vec::new();

    assert!(AgentChatThread::fork_point_for_message_id(
        &thread.messages,
        &fork_points,
        first_user_id,
    )
    .is_none());
    assert!(AgentChatThread::fork_point_for_message_id(
        &thread.messages,
        &fork_points,
        first_user_id + 999,
    )
    .is_none());
}

#[test]
fn truncate_at_user_ordinal_zero_clears_from_first_user_message() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::System, "context note");
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "answer");

    AgentChatThread::truncate_messages_at_user_ordinal(&mut thread.messages, 0);

    assert_eq!(thread.messages.len(), 1);
    assert_eq!(thread.messages[0].body.as_ref(), "context note");
}

fn test_thread(
    pending_context_blocks: Vec<ContentBlock>,
    pending_context_consumed: bool,
) -> AgentChatThread {
    test_thread_with_profile(
        crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID,
        pending_context_blocks,
        pending_context_consumed,
    )
}

fn test_thread_with_profile(
    profile_id: &str,
    pending_context_blocks: Vec<ContentBlock>,
    pending_context_consumed: bool,
) -> AgentChatThread {
    let (_perm_tx, perm_rx) = async_channel::bounded(1);
    // We create a dummy connection channel — tests that call prepare_turn_blocks
    // and append_chunk don't need a live connection.
    let dummy_connection: Arc<dyn AgentChatConnection> = Arc::new(super::TestAgentChatConnection);

    AgentChatThread {
        connection: dummy_connection,
        permission_rx: perm_rx,
        ui_thread_id: "test-thread".to_string(),
        cwd: PathBuf::from("."),
        display_name: "Test Agent".into(),
        profile_id: profile_id.to_string(),
        messages: Vec::new(),
        input: TextInputState::new(),
        status: AgentChatThreadStatus::Idle,
        reliability_state: AiOperationState::ready(
            AgentChatThread::reliability_identity(
                AgentChatSessionPolicy::Full,
                profile_id,
                None,
                Path::new("."),
            ),
            AgentChatThread::reliability_selection(
                profile_id,
                None,
                SelectionOrigin::PersistedUserChoice,
            ),
            AiWorkSnapshot {
                key: WorkKey::from("test-thread"),
                transcript: PreservationReceipt::NotApplicable,
                draft: PreservationReceipt::NotApplicable,
                attachments: PreservationReceipt::NotApplicable,
                partial_output: PreservationReceipt::NotApplicable,
            },
            RetryPolicy {
                automatic_max: 0,
                manual_max: 2,
            },
        ),
        context_resolution_id: 0,
        pending_permission: None,
        pending_context_blocks,
        pending_context_consumed,
        pending_context_parts: Vec::new(),
        pending_ambient_context_enabled: false,
        context_bootstrap_state: AgentChatContextBootstrapState::Ready,
        queued_submit_while_bootstrapping: false,
        context_bootstrap_note: None,
        queued_messages: VecDeque::new(),
        queue_paused: false,
        active_plan_entries: Vec::new(),
        active_mode_id: None,
        available_commands: Vec::new(),
        active_tool_calls: Vec::new(),
        tool_call_lookup: HashMap::new(),
        standing_approvals: Vec::new(),
        fork_points: Vec::new(),
        pending_fork_ordinal: None,
        selected_agent: None,
        available_agents: Vec::new(),
        launch_requirements: crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
        setup_state: None,
        usage_tokens: None,
        usage_cost_usd: None,
        stream_started_at: None,
        ttft_pending: false,
        stream_task: None,
        permission_task: None,
        streaming_text_buffer: StreamingTextBuffer::default(),
        streaming_text_drain_task: None,
        transcript_generation: 0,
        next_message_id: 1,
        host_window_state: None,
        notification_debounce: AgentChatNotificationDebounce::default(),
        current_turn_id: 0,
        llm_title_attempted: false,
        session_policy: crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::Full,
        available_models: Vec::new(),
        selected_model_id: None,
        model_selection_mismatch: None,
        selected_model_display_name: None,
        profile_display_name: None,
        profile_icon_name: None,
    }
}

fn block_text(block: &ContentBlock) -> &str {
    match block {
        ContentBlock::Text(text) => text.text.as_str(),
        other => panic!("expected text block, got {other:?}"),
    }
}

#[test]
fn brain_profile_prepends_recall_and_records_ask_signal() {
    let mut thread = test_thread_with_profile(
        crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID,
        Vec::new(),
        false,
    );
    let signal_calls = std::cell::Cell::new(0);

    let prepared = thread.prepare_turn_blocks_with_receipt_using(
        "What is the handoff port?",
        |_| Some("Brain recall\n- [Note] The handoff port is 49217.".to_string()),
        |_| signal_calls.set(signal_calls.get() + 1),
    );

    assert_eq!(signal_calls.get(), 1);
    assert_eq!(prepared.blocks.len(), 2);
    assert!(block_text(&prepared.blocks[0]).contains("Brain recall"));
    assert_eq!(
        block_text(&prepared.blocks[1]),
        "--- USER REQUEST ---\nWhat is the handoff port?"
    );
}

#[test]
fn non_brain_profile_does_not_call_recall_or_record_ask_signal() {
    let mut thread = test_thread_with_profile(
        crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID,
        Vec::new(),
        false,
    );

    let prepared = thread.prepare_turn_blocks_with_receipt_using(
        "What is the handoff port?",
        |_| panic!("non-Brain profile must not read brain recall"),
        |_| panic!("non-Brain profile must not record brain ask signals"),
    );

    assert_eq!(prepared.blocks.len(), 1);
    assert_eq!(block_text(&prepared.blocks[0]), "What is the handoff port?");
}

#[test]
fn brain_recall_sits_before_pending_context_and_user_request() {
    let mut thread = test_thread_with_profile(
        crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID,
        vec![ContentBlock::Text(TextContent::new("staged context"))],
        false,
    );

    let prepared = thread.prepare_turn_blocks_with_receipt_using(
        "Summarize this",
        |_| Some("Brain recall\n- [Day page] remembered context".to_string()),
        |_| {},
    );

    assert_eq!(prepared.blocks.len(), 3);
    assert!(block_text(&prepared.blocks[0]).starts_with("Brain recall"));
    assert_eq!(block_text(&prepared.blocks[1]), "staged context");
    assert_eq!(
        block_text(&prepared.blocks[2]),
        "--- USER REQUEST ---\nSummarize this"
    );
}

#[test]
fn completed_turn_ingest_payload_uses_latest_turn_and_stable_index() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "first answer");
    thread.push_message(AgentChatThreadMessageRole::User, "second ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "second answer");

    let payload = thread
        .completed_chat_turn_ingest(Some("History Title".to_string()))
        .expect("completed turn should produce ingest payload");

    assert_eq!(payload.thread_id, "test-thread");
    assert_eq!(payload.turn_index, 1);
    assert_eq!(payload.user_text, "second ask");
    assert_eq!(payload.assistant_text, "second answer");
    assert_eq!(payload.trace_label, "History Title");

    let fallback = thread
        .completed_chat_turn_ingest(None)
        .expect("completed turn should produce fallback ingest payload");
    assert_eq!(fallback.trace_label, "first ask");
}

#[test]
fn completed_turn_ingest_payload_is_not_brain_profile_gated() {
    let mut thread = test_thread_with_profile(
        crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID,
        Vec::new(),
        false,
    );
    thread.push_message(AgentChatThreadMessageRole::User, "general profile ask");
    thread.push_message(
        AgentChatThreadMessageRole::Assistant,
        "general profile answer",
    );

    let payload = thread
        .completed_chat_turn_ingest(None)
        .expect("all completed Agent Chat turns should become memory");

    assert_eq!(payload.turn_index, 0);
    assert_eq!(payload.user_text, "general profile ask");
    assert_eq!(payload.assistant_text, "general profile answer");
}

/// WP3-C (Oracle phase-b audit P0): a zero-retention thread must produce NO
/// automatic memory — the Brain ingest + day-trace payload is retention just
/// like the history files, and it used to be built unconditionally.
#[test]
fn zero_retention_thread_produces_no_brain_ingest_payload() {
    use crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy;

    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    thread.push_message(AgentChatThreadMessageRole::User, "quick question");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "quick answer");

    assert!(
        thread
            .completed_chat_turn_ingest(Some("label".to_string()))
            .is_none(),
        "zero-retention turns must not become Brain memories or day traces"
    );

    thread.set_session_policy_test(AgentChatSessionPolicy::Full);
    assert!(
        thread.completed_chat_turn_ingest(None).is_some(),
        "retention-enabled turns still produce the ingest payload"
    );
}

/// WP-B1: a finished Quick AI turn performs ZERO automatic egress. Asserts the
/// three thread-owned policy helpers deny (transcript retention, retained-thread
/// reuse, fork state), that the Brain/day-trace ingest payload is suppressed,
/// and that a Full thread retains all three — the policy is the sole authority.
#[test]
fn quick_ai_turn_finished_has_zero_automatic_egress() {
    use crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy;

    // Policy helpers: Quick AI denies every automatic egress vector.
    let quick = AgentChatSessionPolicy::QuickAi;
    assert!(!quick.allows_automatic_transcript_retention());
    assert!(!quick.allows_retained_thread_reuse());
    assert!(!quick.allows_fork_state());

    let full = AgentChatSessionPolicy::Full;
    assert!(full.allows_automatic_transcript_retention());
    assert!(full.allows_retained_thread_reuse());
    assert!(full.allows_fork_state());

    // Ingest payload gate: a completed Quick AI turn produces no Brain memory
    // or day-trace payload; the thread's live policy is the gate.
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    thread.push_message(AgentChatThreadMessageRole::User, "quick question");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "quick answer");
    assert!(
        thread.completed_chat_turn_ingest(None).is_none(),
        "Quick AI turn must not produce an automatic ingest payload",
    );
    assert_eq!(
        thread.session_policy_test(),
        AgentChatSessionPolicy::QuickAi
    );

    // Fork gate: the TurnFinished path only refreshes fork points when the
    // policy allows fork state — Quick AI never does.
    assert!(!thread.session_policy_test().allows_fork_state());
}

#[test]
fn pending_context_is_only_consumed_once() {
    let mut thread = test_thread(vec![ContentBlock::Text(TextContent::new("context"))], false);

    let first = thread.prepare_turn_blocks("hello");
    let second = thread.prepare_turn_blocks("again");

    // First turn: context block + user input = 2 blocks
    assert_eq!(first.len(), 2, "first turn should include context + input");

    // Second turn: only user input = 1 block
    assert_eq!(second.len(), 1, "second turn should only include input");
}

#[test]
fn awaiting_first_assistant_text_tracks_pre_text_streaming_gap() {
    let mut thread = test_thread(Vec::new(), true);

    thread.push_message(AgentChatThreadMessageRole::User, "Follow up");
    thread.set_status(AgentChatThreadStatus::Streaming);

    assert!(thread.awaiting_first_assistant_text());

    thread.push_message(AgentChatThreadMessageRole::Thought, "Inspecting files");
    thread.push_message(AgentChatThreadMessageRole::Tool, "Read file completed");

    assert!(
        thread.awaiting_first_assistant_text(),
        "thought/tool events before text should keep the activity row visible"
    );

    thread.push_message(AgentChatThreadMessageRole::Assistant, "I found the issue.");

    assert!(!thread.awaiting_first_assistant_text());
}

#[test]
fn awaiting_first_assistant_text_is_false_without_streaming_user_turn() {
    let mut thread = test_thread(Vec::new(), true);

    assert!(!thread.awaiting_first_assistant_text());

    thread.push_message(AgentChatThreadMessageRole::User, "Follow up");
    assert!(!thread.awaiting_first_assistant_text());

    thread.set_status(AgentChatThreadStatus::Streaming);
    assert!(thread.awaiting_first_assistant_text());

    thread.set_status(AgentChatThreadStatus::Idle);
    assert!(!thread.awaiting_first_assistant_text());
}

#[test]
fn assistant_chunks_append_to_last_assistant_message() {
    let mut thread = test_thread(Vec::new(), true);

    thread.append_chunk(AgentChatThreadMessageRole::Assistant, "Hello".to_string());
    thread.append_chunk(AgentChatThreadMessageRole::Assistant, " world".to_string());

    assert_eq!(thread.messages.len(), 1, "chunks should coalesce");
    assert_eq!(
        thread.messages[0].body.to_string(),
        "Hello world",
        "chunks should be concatenated"
    );
}

#[test]
fn chunks_of_different_roles_create_separate_messages() {
    let mut thread = test_thread(Vec::new(), true);

    thread.append_chunk(AgentChatThreadMessageRole::Assistant, "Hello".to_string());
    thread.append_chunk(
        AgentChatThreadMessageRole::Thought,
        "thinking...".to_string(),
    );
    thread.append_chunk(AgentChatThreadMessageRole::Assistant, "world".to_string());

    assert_eq!(
        thread.messages.len(),
        3,
        "different roles should create separate messages"
    );
}

#[test]
fn prepare_turn_blocks_no_guidance_in_exploration_mode() {
    let mut thread = test_thread(vec![ContentBlock::Text(TextContent::new("context"))], false);

    // Even authoring-like intents get no guidance — users invoke /new-script explicitly
    let blocks = thread.prepare_turn_blocks("build a clipboard cleanup script");

    // context + input = 2 blocks (no guidance, exploration mode)
    assert_eq!(
        blocks.len(),
        2,
        "exploration mode: context + input only, no guidance"
    );
}

#[test]
fn prepare_turn_blocks_no_guidance_for_any_intent() {
    let mut thread = test_thread(vec![ContentBlock::Text(TextContent::new("context"))], false);

    let blocks = thread.prepare_turn_blocks("explain this selection");

    // context + input = 2 blocks
    assert_eq!(
        blocks.len(),
        2,
        "non-authoring intent should include context + input only"
    );
}

#[test]
fn alloc_id_is_monotonically_increasing() {
    let mut thread = test_thread(Vec::new(), true);

    let id1 = thread.alloc_id();
    let id2 = thread.alloc_id();
    let id3 = thread.alloc_id();

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn context_already_consumed_skips_on_first_turn() {
    let mut thread = test_thread(
        vec![ContentBlock::Text(TextContent::new("context"))],
        true, // already consumed
    );

    let blocks = thread.prepare_turn_blocks("hello");
    assert_eq!(blocks.len(), 1, "consumed context should not be prepended");
}

// ── Structured state tests ────────────────────────────────────

/// Helper that applies an event without a GPUI context (for pure logic tests).
/// Delegates to the instance method `apply_event_test` on `AgentChatThread`.
fn apply_event_test(thread: &mut AgentChatThread, event: AgentChatEvent) {
    thread.apply_event_test(event);
}

#[test]
fn plan_updated_stores_in_dedicated_field() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::PlanUpdated {
            entries: vec!["Step 1".into(), "Step 2".into()],
        },
    );

    assert_eq!(thread.active_plan_entries(), &["Step 1", "Step 2"]);
    // Plan updates should not create messages — the view reads the field.
    assert!(
        thread.messages.is_empty(),
        "plan updates should not produce messages"
    );
}

#[test]
fn mode_changed_stores_in_dedicated_field() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::ModeChanged {
            mode_id: "architect".into(),
        },
    );

    assert_eq!(thread.active_mode_id(), Some("architect"));
    assert!(
        thread.messages.is_empty(),
        "mode changes should not produce messages"
    );
}

#[test]
fn models_available_replaces_list_and_surfaces_new_models() {
    use super::super::config::AgentChatModelEntry;

    let mut thread = test_thread(Vec::new(), true);
    // Seed the thread with the old hardcoded fallback list so we can
    // prove that ModelsAvailable actually replaces it.
    thread.available_models = vec![
        AgentChatModelEntry {
            id: "claude-sonnet-4-6".into(),
            display_name: Some("Sonnet 4.6".into()),
            context_window: Some(200_000),
        },
        AgentChatModelEntry {
            id: "claude-opus-4-6".into(),
            display_name: Some("Opus 4.6".into()),
            context_window: Some(200_000),
        },
    ];

    // Simulate what the Agent Chat client produces when claude-code-agent_chat advertises
    // Opus 4.7 in its session/new response.
    let agent_list = vec![
        AgentChatModelEntry {
            id: "claude-opus-4-7".into(),
            display_name: Some("Opus 4.7".into()),
            context_window: None,
        },
        AgentChatModelEntry {
            id: "claude-sonnet-4-6".into(),
            display_name: Some("Sonnet 4.6".into()),
            context_window: None,
        },
        AgentChatModelEntry {
            id: "claude-haiku-4-5".into(),
            display_name: Some("Haiku 4.5".into()),
            context_window: None,
        },
    ];

    apply_event_test(
        &mut thread,
        AgentChatEvent::ModelsAvailable {
            current_model_id: Some("claude-opus-4-7".into()),
            models: agent_list.clone(),
        },
    );

    let ids: Vec<&str> = thread
        .available_models()
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["claude-opus-4-7", "claude-sonnet-4-6", "claude-haiku-4-5"],
        "agent-advertised list should replace the hardcoded fallback"
    );
    assert!(
        ids.contains(&"claude-opus-4-7"),
        "Opus 4.7 must surface when the agent advertises it"
    );
    // The stale fallback-only entry must be gone.
    assert!(
        !ids.contains(&"claude-opus-4-6"),
        "old fallback entries should not leak through"
    );
}

#[test]
fn models_available_preserves_user_selection_when_still_valid() {
    use super::super::config::AgentChatModelEntry;

    let mut thread = test_thread(Vec::new(), true);
    thread.selected_model_id = Some("claude-sonnet-4-6".into());
    thread.selected_model_display_name = Some(SharedString::from("Sonnet 4.6"));

    apply_event_test(
        &mut thread,
        AgentChatEvent::ModelsAvailable {
            current_model_id: Some("claude-opus-4-7".into()),
            models: vec![
                AgentChatModelEntry {
                    id: "claude-opus-4-7".into(),
                    display_name: Some("Opus 4.7".into()),
                    context_window: None,
                },
                AgentChatModelEntry {
                    id: "claude-sonnet-4-6".into(),
                    display_name: Some("Sonnet 4.6".into()),
                    context_window: None,
                },
            ],
        },
    );

    assert_eq!(
        thread.selected_model_id(),
        Some("claude-sonnet-4-6"),
        "user's persisted selection must be preserved when still in the new list"
    );
}

#[test]
fn models_available_reports_recovery_without_changing_dropped_selection() {
    use super::super::config::AgentChatModelEntry;

    let mut thread = test_thread(Vec::new(), true);
    // User had a selection that the agent no longer lists.
    thread.selected_model_id = Some("claude-retired-model".into());

    apply_event_test(
        &mut thread,
        AgentChatEvent::ModelsAvailable {
            current_model_id: Some("claude-opus-4-7".into()),
            models: vec![
                AgentChatModelEntry {
                    id: "claude-opus-4-7".into(),
                    display_name: Some("Opus 4.7".into()),
                    context_window: None,
                },
                AgentChatModelEntry {
                    id: "claude-sonnet-4-6".into(),
                    display_name: Some("Sonnet 4.6".into()),
                    context_window: None,
                },
            ],
        },
    );

    assert_eq!(
        thread.selected_model_id(),
        Some("claude-retired-model"),
        "runtime current model must not silently replace the user's selection"
    );
    let mismatch = thread
        .model_selection_mismatch()
        .expect("missing selection should produce recovery state");
    assert_eq!(
        mismatch.runtime_model_id.as_deref(),
        Some("claude-opus-4-7")
    );
    assert_eq!(
        mismatch.candidate_model_ids,
        vec!["claude-opus-4-7", "claude-sonnet-4-6"]
    );
}

#[test]
fn models_available_empty_catalog_blocks_turn_for_recovery() {
    let mut thread = test_thread(Vec::new(), true);
    thread.selected_model_id = Some("gpt-5.6-sol".into());

    apply_event_test(
        &mut thread,
        AgentChatEvent::ModelsAvailable {
            current_model_id: None,
            models: Vec::new(),
        },
    );

    let mismatch = thread
        .model_selection_mismatch()
        .expect("empty catalog must produce recovery state");
    assert_eq!(mismatch.requested_model_id.as_deref(), Some("gpt-5.6-sol"));
    assert!(mismatch.candidate_model_ids.is_empty());
}

#[test]
fn available_commands_stores_in_dedicated_field() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::AvailableCommandsUpdated {
            command_names: vec!["plan".into(), "compact".into()],
        },
    );

    assert_eq!(thread.available_commands(), &["plan", "compact"]);
    assert!(
        thread.messages.is_empty(),
        "command updates should not produce messages"
    );
}

#[test]
fn tool_call_started_creates_tracked_state_and_message() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallStarted {
            tool_call_id: "tc-1".into(),
            title: "Read file".into(),
            status: "running".into(),
            tool_name: None,
            raw_input: None,
        },
    );

    assert_eq!(thread.active_tool_calls().len(), 1);
    assert_eq!(thread.active_tool_calls()[0].tool_call_id, "tc-1");
    assert_eq!(thread.active_tool_calls()[0].title, "Read file");
    assert_eq!(thread.active_tool_calls()[0].status, "running");

    assert_eq!(thread.messages.len(), 1);
    assert_eq!(thread.messages[0].role, AgentChatThreadMessageRole::Tool);
    assert_eq!(thread.messages[0].tool_call_id.as_deref(), Some("tc-1"));
}

#[test]
fn tool_call_updated_modifies_existing_message_in_place() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallStarted {
            tool_call_id: "tc-1".into(),
            title: "Read file".into(),
            status: "running".into(),
            tool_name: None,
            raw_input: None,
        },
    );

    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallUpdated {
            tool_call_id: "tc-1".into(),
            title: None,
            status: Some("completed".into()),
            body: Some("file contents here".into()),
            raw_input: None,
            diff: None,
            is_error: false,
        },
    );

    // Should still be 1 message, updated in-place.
    assert_eq!(
        thread.messages.len(),
        1,
        "tool update should modify existing message, not create a new one"
    );

    let msg = &thread.messages[0];
    assert!(
        msg.body.contains("completed"),
        "message body should reflect updated status"
    );
    assert!(
        msg.body.contains("file contents here"),
        "message body should include updated body"
    );

    // Tracked state should also be updated.
    let tc = &thread.active_tool_calls()[0];
    assert_eq!(tc.status, "completed");
    assert_eq!(tc.body.as_deref(), Some("file contents here"));
}

#[test]
fn orphan_tool_update_creates_standalone_message() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallUpdated {
            tool_call_id: "unknown".into(),
            title: None,
            status: Some("done".into()),
            body: None,
            raw_input: None,
            diff: None,
            is_error: false,
        },
    );

    assert_eq!(
        thread.messages.len(),
        1,
        "orphan update should create a standalone message"
    );
    // Orphan update now creates a full tool call entry with default title + provided status.
    assert!(thread.messages[0].body.contains("done"));
}

#[test]
fn turn_finished_does_not_create_message() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(&mut thread, AgentChatEvent::completed("end_turn"));

    assert!(
        thread.messages.is_empty(),
        "turn finished should not produce a message"
    );
    assert_eq!(thread.status, AgentChatThreadStatus::Idle);
}

#[test]
fn submit_while_streaming_queues_and_clears_composer() {
    let mut thread = test_thread(Vec::new(), true);
    thread.status = AgentChatThreadStatus::Streaming;
    thread.input.set_text("follow up".to_string());
    thread
        .pending_context_parts
        .push(crate::ai::message_parts::AiContextPart::TextBlock {
            label: "ctx".to_string(),
            source: "test".to_string(),
            text: "ctx".to_string(),
            mime_type: None,
        });

    let text = thread.input.text().trim().to_string();
    thread.resume_queue_for_manual_submit();
    thread.queue_current_composer(text);

    assert_eq!(thread.queued_messages().len(), 1);
    assert_eq!(thread.queued_messages()[0].text, "follow up");
    assert_eq!(thread.queued_messages()[0].context_parts.len(), 1);
    assert!(thread.input.text().is_empty());
    assert!(thread.pending_context_parts().is_empty());
    assert_eq!(thread.status, AgentChatThreadStatus::Streaming);
}

#[test]
fn turn_finished_auto_sends_front_of_queue() {
    let mut thread = test_thread(Vec::new(), true);
    thread.status = AgentChatThreadStatus::Streaming;
    thread
        .queued_messages
        .push_back(AgentChatQueuedMessage::new(
            "first queued".to_string(),
            Vec::new(),
        ));
    thread
        .queued_messages
        .push_back(AgentChatQueuedMessage::new(
            "second queued".to_string(),
            Vec::new(),
        ));

    thread.apply_event_test(AgentChatEvent::completed("end_turn"));

    assert_eq!(thread.status, AgentChatThreadStatus::Streaming);
    assert_eq!(
        thread.messages.last().unwrap().body.as_ref(),
        "first queued"
    );
    assert_eq!(thread.queued_messages().len(), 1);
    assert_eq!(thread.queued_messages()[0].text, "second queued");
}

#[test]
fn paused_queue_does_not_auto_send_on_turn_finished() {
    let mut thread = test_thread(Vec::new(), true);
    thread.status = AgentChatThreadStatus::Streaming;
    thread.queue_paused = true;
    thread
        .queued_messages
        .push_back(AgentChatQueuedMessage::new(
            "held queued".to_string(),
            Vec::new(),
        ));

    thread.apply_event_test(AgentChatEvent::completed("cancelled"));

    assert_eq!(thread.status, AgentChatThreadStatus::Idle);
    assert!(thread.messages.is_empty());
    assert_eq!(thread.queued_messages().len(), 1);
}

#[test]
fn manual_submit_clears_queue_pause() {
    let mut thread = test_thread(Vec::new(), true);
    thread.queue_paused = true;

    thread.resume_queue_for_manual_submit();

    assert!(!thread.queue_paused());
}

#[test]
fn closed_stream_without_terminal_unlocks_after_assistant_text() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::AgentMessageDelta("done".into()),
    );
    assert_eq!(thread.status, AgentChatThreadStatus::Streaming);

    assert!(thread.finish_stream_closed_without_terminal());

    assert_eq!(
        thread.status,
        AgentChatThreadStatus::Idle,
        "missing terminal event must not leave composer blocked"
    );
    assert_eq!(thread.messages.len(), 1);
    assert_eq!(
        thread.messages[0].role,
        AgentChatThreadMessageRole::Assistant
    );
}

#[test]
fn closed_stream_without_terminal_errors_without_assistant_text() {
    let mut thread = test_thread(Vec::new(), true);
    thread.status = AgentChatThreadStatus::Streaming;

    assert!(thread.finish_stream_closed_without_terminal());

    assert_eq!(
        thread.status,
        AgentChatThreadStatus::Error,
        "missing terminal event without content should still unlock follow-up"
    );
    assert_eq!(thread.messages.len(), 1);
    assert_eq!(thread.messages[0].role, AgentChatThreadMessageRole::Error);
}

#[test]
fn failed_event_creates_error_message_and_retryable_recovery_card() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");

    apply_event_test(
        &mut thread,
        AgentChatEvent::failed(
            sk_protocol::ai_reliability::ProtocolComponent::Provider,
            "connection lost",
        ),
    );

    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[1].role, AgentChatThreadMessageRole::Error);
    assert!(!thread.messages[1].body.contains("connection lost"));
    assert_eq!(thread.status, AgentChatThreadStatus::Error);
    let card = thread
        .recovery_card_spec()
        .expect("failed turn arms recovery card");
    assert!(!card.body.is_empty());
    assert!(card
        .actions
        .iter()
        .any(|action| { action.enabled && action.action.kind() == RecoveryActionKind::Retry }));
}

#[test]
fn usage_limit_failure_surfaces_account_recovery_without_raw_json_as_message() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    let raw_error = r#"{"error":{"type":"usage_limit_reached","status":429}}"#;

    apply_event_test(
        &mut thread,
        AgentChatEvent::failed(
            sk_protocol::ai_reliability::ProtocolComponent::Provider,
            raw_error,
        ),
    );

    let card = thread
        .recovery_card_spec()
        .expect("failure arms recovery card");
    assert_eq!(card.title.as_ref(), "Usage limit reached");
    assert!(card.actions.iter().any(|action| {
        action.action.kind() == RecoveryActionKind::SwitchAccount && action.enabled
    }));
    assert!(!thread.messages[1].body.contains("{\"error\""));
    assert!(thread.messages[1].body.contains("usage limit"));
}

#[test]
fn codex_upgrade_error_becomes_shared_recovery_without_raw_json() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(
        AgentChatThreadMessageRole::User,
        "What's a popular post from the past 10 minutes?",
    );
    let raw = r#"{"type":"error","status":400,"error":{"type":"invalid_request_error","message":"The 'gpt-5.6-sol' model requires a newer version of Codex. Please upgrade to the latest app or CLI and try again."}}"#;
    thread.apply_event_test(AgentChatEvent::failed(
        sk_protocol::ai_reliability::ProtocolComponent::Pi,
        raw,
    ));

    let card = thread.recovery_card_spec().expect("recovery card");
    assert_eq!(card.title.as_ref(), "Codex needs an update for this model");
    assert!(card.body.contains("Your turn is saved"));
    assert!(card.actions.iter().any(|action| {
        action.enabled && action.action.kind() == RecoveryActionKind::ChooseCompatibleModel
    }));
    assert!(thread
        .messages
        .iter()
        .all(|message| !message.body.contains("invalid_request_error")));
}

#[test]
fn setup_required_uses_same_typed_recovery_projection_as_failed_turns() {
    let mut thread = test_thread(Vec::new(), true);
    thread.apply_event_test(AgentChatEvent::SetupRequired {
        reason: "authentication required".to_string(),
        auth_methods: vec!["oauth".to_string()],
    });

    assert!(thread.setup_state().is_some());
    let card = thread.recovery_card_spec().expect("shared recovery card");
    assert_eq!(card.title.as_ref(), "Sign in required");
    assert!(card
        .actions
        .iter()
        .any(|action| { action.enabled && action.action.kind() == RecoveryActionKind::SignIn }));
}

#[test]
fn repeated_manual_failures_exhaust_retry_without_duplicating_user_turn() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try once");
    for attempt in 0..2 {
        thread.apply_event_test(AgentChatEvent::failed(
            sk_protocol::ai_reliability::ProtocolComponent::Provider,
            "connection lost",
        ));
        let before_users = thread
            .messages
            .iter()
            .filter(|message| matches!(message.role, AgentChatThreadMessageRole::User))
            .count();
        thread.retry_last_user_turn_test().unwrap();
        assert_eq!(
            thread
                .messages
                .iter()
                .filter(|message| matches!(message.role, AgentChatThreadMessageRole::User))
                .count(),
            before_users,
            "retry attempt {attempt} must not duplicate the user turn"
        );
    }
    thread.apply_event_test(AgentChatEvent::failed(
        sk_protocol::ai_reliability::ProtocolComponent::Provider,
        "connection lost",
    ));
    let card = thread
        .recovery_card_spec()
        .expect("recovery remains visible");
    assert!(!card
        .actions
        .iter()
        .any(|action| { action.enabled && action.action.kind() == RecoveryActionKind::Retry }));
}

#[test]
fn retry_from_error_reenters_streaming_without_duplicate_user_message() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    thread.apply_event_test(AgentChatEvent::failed(
        sk_protocol::ai_reliability::ProtocolComponent::Provider,
        "connection lost",
    ));
    let before = thread.messages.len();

    thread.retry_last_user_turn_test().unwrap();

    assert_eq!(thread.status, AgentChatThreadStatus::Streaming);
    assert_eq!(thread.messages.len(), before);
    assert_eq!(
        thread
            .messages
            .iter()
            .filter(|message| matches!(message.role, AgentChatThreadMessageRole::User))
            .count(),
        1
    );
    assert!(thread.recovery_card_spec().is_none());
}

#[test]
fn dismiss_clears_failed_turn_recovery_card() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    thread.apply_event_test(AgentChatEvent::failed(
        sk_protocol::ai_reliability::ProtocolComponent::Provider,
        "connection lost",
    ));

    thread.dismiss_recovery_test();

    assert!(thread.recovery_card_spec().is_none());
}

#[test]
fn starting_new_turn_clears_failed_turn_recovery_card() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    thread.apply_event_test(AgentChatEvent::failed(
        sk_protocol::ai_reliability::ProtocolComponent::Provider,
        "connection lost",
    ));

    thread.retry_last_user_turn_test().unwrap();

    assert!(thread.recovery_card_spec().is_none());
}

#[test]
fn multiple_tool_calls_tracked_independently() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallStarted {
            tool_call_id: "tc-1".into(),
            title: "Read file".into(),
            status: "running".into(),
            tool_name: None,
            raw_input: None,
        },
    );
    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallStarted {
            tool_call_id: "tc-2".into(),
            title: "Write file".into(),
            status: "running".into(),
            tool_name: None,
            raw_input: None,
        },
    );

    // Update only tc-1.
    apply_event_test(
        &mut thread,
        AgentChatEvent::ToolCallUpdated {
            tool_call_id: "tc-1".into(),
            title: None,
            status: Some("completed".into()),
            body: None,
            raw_input: None,
            diff: None,
            is_error: false,
        },
    );

    assert_eq!(thread.active_tool_calls().len(), 2);
    assert_eq!(thread.active_tool_calls()[0].status, "completed");
    assert_eq!(thread.active_tool_calls()[1].status, "running");

    // Two messages, one per tool call.
    assert_eq!(thread.messages.len(), 2);
}

fn approval_request_with_options(
    reply_tx: async_channel::Sender<Option<String>>,
) -> AgentChatApprovalRequest {
    use super::super::permission_broker::AgentChatApprovalOption;
    AgentChatApprovalRequest {
        id: 1,
        title: "Run command".into(),
        body: "Agent wants to run a command".into(),
        preview: Some(
            super::super::permission_broker::AgentChatApprovalPreview::new("bash", "tc-1")
                .with_subject(Some("cargo test".to_string())),
        ),
        options: vec![
            AgentChatApprovalOption {
                option_id: "allow-once".into(),
                name: "Allow".into(),
                kind: "AllowOnce".into(),
            },
            AgentChatApprovalOption {
                option_id: "allow-always".into(),
                name: "Allow always".into(),
                kind: "AllowAlways".into(),
            },
            AgentChatApprovalOption {
                option_id: "deny".into(),
                name: "Deny".into(),
                kind: "RejectOnce".into(),
            },
        ],
        reply_tx,
    }
}

#[test]
fn persistent_allow_records_standing_approval_once() {
    let mut thread = test_thread(Vec::new(), true);
    let (reply_tx, _reply_rx) = async_channel::bounded(1);
    let request = approval_request_with_options(reply_tx);

    // One-shot allow must NOT record a standing grant.
    thread.record_standing_approval(&request, Some("allow-once"));
    assert!(thread.standing_approvals().is_empty());

    // Denial must not record either.
    thread.record_standing_approval(&request, Some("deny"));
    assert!(thread.standing_approvals().is_empty());

    // Persistent allow records the grant with tool/subject context.
    thread.record_standing_approval(&request, Some("allow-always"));
    assert_eq!(thread.standing_approvals().len(), 1);
    let grant = &thread.standing_approvals()[0];
    assert_eq!(grant.tool_title, "bash");
    assert_eq!(grant.subject.as_deref(), Some("cargo test"));
    assert_eq!(grant.option_label, "Allow always (AllowAlways)");

    // Repeating the same grant dedupes by (tool, subject).
    thread.record_standing_approval(&request, Some("allow-always"));
    assert_eq!(thread.standing_approvals().len(), 1);
}

#[test]
fn plan_updated_replaces_previous_plan() {
    let mut thread = test_thread(Vec::new(), true);

    apply_event_test(
        &mut thread,
        AgentChatEvent::PlanUpdated {
            entries: vec!["Step 1".into()],
        },
    );
    apply_event_test(
        &mut thread,
        AgentChatEvent::PlanUpdated {
            entries: vec!["Step A".into(), "Step B".into()],
        },
    );

    assert_eq!(
        thread.active_plan_entries(),
        &["Step A", "Step B"],
        "plan should be fully replaced, not appended"
    );
}

// ── Chip lifecycle regression tests ───────────────────────────

/// Helper: build a minimal `TabAiContextBlob` for testing stage operations.
fn minimal_blob() -> crate::ai::TabAiContextBlob {
    crate::ai::TabAiContextBlob::from_parts(
        crate::ai::tab_context::TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            input_text: None,
            focused_semantic_id: None,
            selected_semantic_id: None,
            visible_elements: Vec::new(),
        },
        crate::context_snapshot::AiContextSnapshot::default(),
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        "2026-01-01T00:00:00Z".to_string(),
    )
}

/// Helper: build an Ask Anything `ResourceUri` part.
fn ask_anything_part() -> crate::ai::message_parts::AiContextPart {
    crate::ai::message_parts::AiContextPart::ResourceUri {
        uri: crate::ai::message_parts::ASK_ANYTHING_RESOURCE_URI.to_string(),
        label: crate::ai::message_parts::ASK_ANYTHING_LABEL.to_string(),
    }
}

/// Helper: build a focused-target part.
fn focused_target_part(name: &str) -> crate::ai::message_parts::AiContextPart {
    crate::ai::message_parts::AiContextPart::FocusedTarget {
        target: crate::ai::tab_context::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: format!("choice:0:{name}"),
            label: name.to_string(),
            metadata: None,
        },
        label: name.to_string(),
    }
}

/// Helper: build the explicit screenshot resource part.
fn screenshot_part() -> crate::ai::message_parts::AiContextPart {
    crate::ai::context_contract::ContextAttachmentKind::Screenshot.part()
}

/// Regression: Ask Anything chip removed before capture completes.
///
/// When the user arms Ask Anything then removes the chip while the deferred
/// capture is still running, the thread must disable ambient context so that
/// `stage_ask_anything_context` becomes a no-op and no stale blocks are
/// attached to the first submit.
#[test]
fn ask_anything_removed_before_capture_completes() {
    let mut thread = test_thread(Vec::new(), false);

    // 1. Arm the Ask Anything chip (simulates Tab from a fallback surface).
    thread.add_context_part_test(ask_anything_part());
    assert!(thread.pending_ambient_context_enabled);
    assert_eq!(
        thread.context_bootstrap_state,
        AgentChatContextBootstrapState::Preparing
    );
    assert_eq!(thread.pending_context_parts.len(), 1);

    // 2. User removes the chip before capture finishes.
    thread.remove_context_part_test(0);

    // 3. Assert: ambient disabled, no blocks, bootstrap ready, chip gone.
    assert!(!thread.pending_ambient_context_enabled);
    assert!(thread.pending_context_blocks.is_empty());
    assert_eq!(
        thread.context_bootstrap_state,
        AgentChatContextBootstrapState::Ready
    );
    assert_eq!(
        thread.context_bootstrap_note.as_ref().map(|s| s.as_ref()),
        Some("Ask Anything removed")
    );
    assert!(thread.pending_context_parts.is_empty());

    // 4. Deferred capture completes — should be a no-op.
    let blob = minimal_blob();
    thread
        .stage_ask_anything_context_test(&blob)
        .expect("stage should succeed");
    assert!(
        thread.pending_context_blocks.is_empty(),
        "blocks should remain empty after late capture"
    );

    // 5. First submit should carry no ambient context.
    thread.input.set_text("hello");
    let blocks = thread.prepare_turn_blocks("hello");
    assert_eq!(blocks.len(), 1, "only user input, no ambient context");
}

/// Regression: Ask Anything chip removed after ambient promotion.
///
/// After capture completes and the chip is promoted from `ResourceUri` to
/// `AmbientContext`, removing the promoted chip must clear the hidden
/// `pending_context_blocks` so the first submit sends no ambient context.
#[test]
fn ask_anything_removed_after_ambient_promotion() {
    let mut thread = test_thread(Vec::new(), false);

    // 1. Arm the Ask Anything chip.
    thread.add_context_part_test(ask_anything_part());
    assert!(thread.pending_ambient_context_enabled);

    // 2. Capture completes — promotes chip to AmbientContext, stages blocks.
    let blob = minimal_blob();
    thread
        .stage_ask_anything_context_test(&blob)
        .expect("stage should succeed");

    // Verify promotion happened.
    assert_eq!(thread.pending_context_parts.len(), 1);
    assert!(
        thread.pending_context_parts[0].is_ambient_context_chip(),
        "chip should be promoted to AmbientContext"
    );
    assert!(
        !thread.pending_context_blocks.is_empty(),
        "blocks should be staged"
    );
    assert_eq!(
        thread.context_bootstrap_note.as_ref().map(|s| s.as_ref()),
        Some("Ask Anything ready")
    );

    // 3. User removes the promoted chip.
    thread.remove_context_part_test(0);

    // 4. Assert: ambient disabled, blocks cleared, chip gone.
    assert!(!thread.pending_ambient_context_enabled);
    assert!(
        thread.pending_context_blocks.is_empty(),
        "removing promoted chip must clear hidden blocks"
    );
    assert!(thread.pending_context_parts.is_empty());

    // 5. First submit should carry no ambient context.
    thread.input.set_text("hello");
    let blocks = thread.prepare_turn_blocks("hello");
    assert_eq!(blocks.len(), 1, "only user input, no ambient context");
}

/// Regression: Focused-target chip consumed on first submit.
///
/// After a focused-target chip is staged and the first message is submitted,
/// the chip must be consumed (removed from `pending_context_parts`) so the
/// composer shows no stale chips on the second turn.
#[test]
fn focused_target_chip_consumed_on_first_submit() {
    let mut thread = test_thread(Vec::new(), false);

    // 1. Stage a focused-target chip (simulates Tab from a focused surface).
    thread.add_context_part_test(focused_target_part("my-script"));
    assert_eq!(thread.pending_context_parts.len(), 1);
    assert!(!thread.pending_context_consumed);

    // Mark bootstrap as ready (focused path doesn't use deferred capture).
    thread.context_bootstrap_state = AgentChatContextBootstrapState::Ready;

    // 2. First submit.
    let blocks = thread.prepare_turn_blocks("explain this script");

    // Should have: resolved context part block + USER REQUEST marker + input.
    assert!(
        blocks.len() >= 2,
        "first submit should include context + input, got {} blocks",
        blocks.len()
    );
    assert!(thread.pending_context_consumed);

    // 3. Chip stays visible after submit (not drained).
    assert_eq!(
        thread.pending_context_parts.len(),
        1,
        "chip must persist after submit so it remains visible in the composer"
    );

    // 4. Second submit should carry no context.
    let blocks2 = thread.prepare_turn_blocks("what else?");
    assert_eq!(
        blocks2.len(),
        1,
        "second turn should only have user input, no context"
    );
}

#[test]
fn submit_snapshot_consumes_context_without_resolving_it_on_the_caller() {
    let mut thread = test_thread(
        vec![ContentBlock::Text(TextContent::new(
            "hidden ambient context",
        ))],
        false,
    );
    thread.add_context_part_test(screenshot_part());

    // This is the exact synchronous seam used by submit_input. If it called
    // either resolver, the screenshot part would attempt a real screen capture.
    let job = thread
        .take_pending_context_for_background_resolution()
        .expect("staged context should produce a background job");

    assert!(thread.pending_context_consumed);
    assert!(!thread.pending_ambient_context_enabled);
    assert!(thread.pending_context_blocks.is_empty());
    assert_eq!(job.blocks.len(), 1);
    assert_eq!(job.parts, vec![screenshot_part()]);
    assert_eq!(job.attachments.len(), 1);
}

#[test]
fn follow_up_screenshot_chip_emits_special_attachment_block() {
    let mut thread = test_thread(Vec::new(), false);

    // First turn consumes the existing focused target context.
    thread.add_context_part_test(focused_target_part("choose-theme"));
    let first_blocks = thread.prepare_turn_blocks("summarize this command");
    assert!(
        first_blocks.len() >= 2,
        "first turn should include focused target context"
    );
    assert!(thread.pending_context_consumed);

    // Follow-up: user explicitly types @screenshot.
    thread.add_context_part_test(screenshot_part());
    assert!(
        !thread.pending_context_consumed,
        "new explicit screenshot chip must re-arm pending context"
    );

    let turn = thread
        .take_pending_context_for_turn_with(|part| {
            if AgentChatThread::is_explicit_screenshot_part(part) {
                return Ok(Some(ContentBlock::Text(TextContent::new(
                    "__test_screenshot_block__",
                ))));
            }
            Ok(None)
        })
        .expect("follow-up screenshot turn should resolve");

    assert_eq!(
        turn.receipt.attempted, 2,
        "follow-up submit should resolve both the focused target and the explicit screenshot"
    );
    assert_eq!(
        turn.receipt.resolved, 2,
        "both follow-up context parts should resolve"
    );
    assert!(
        turn.receipt.failures.is_empty(),
        "follow-up screenshot should not fail: {:?}",
        turn.receipt.failures
    );
    assert!(
        !turn
            .receipt
            .prompt_prefix
            .contains("kit://context?screenshot=1"),
        "explicit screenshot should not fall back to the text-only MCP resource when the attachment block succeeds"
    );
    assert!(
        turn.receipt.prompt_prefix.contains("focusedTarget"),
        "focused target should still resolve through the normal prompt-prefix path"
    );
    assert_eq!(
        turn.blocks.len(),
        1,
        "only the explicit screenshot should become a special attachment block"
    );
    match &turn.blocks[0] {
        ContentBlock::Text(text) => assert_eq!(text.text, "__test_screenshot_block__"),
        other => panic!("expected test screenshot block, got {other:?}"),
    }
    assert!(
        thread.pending_context_consumed,
        "follow-up screenshot submit should mark pending context consumed"
    );
}

#[test]
fn non_ambient_part_marks_bootstrap_ready_when_no_ambient_capture_is_pending() {
    let mut thread = test_thread(Vec::new(), false);
    thread.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
    thread.context_bootstrap_note = Some("Queued · sending when context is attached…".into());

    thread.add_context_part_test(focused_target_part("my-script"));

    assert_eq!(
        thread.context_bootstrap_state,
        AgentChatContextBootstrapState::Ready,
        "typed context attachments should not leave the composer stuck in Preparing"
    );
    assert_eq!(
        thread.context_bootstrap_note, None,
        "manual non-ambient attachments should clear the queued bootstrap note"
    );
    assert_eq!(thread.pending_context_parts.len(), 1);
}

#[test]
fn current_context_selector_part_marks_bootstrap_ready_instead_of_waiting_for_ambient_capture() {
    let mut thread = test_thread(Vec::new(), false);
    thread.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
    thread.context_bootstrap_note = Some("Capturing Current Context…".into());

    thread.add_context_part_test(crate::ai::message_parts::AiContextPart::ResourceUri {
        uri: crate::ai::message_parts::ASK_ANYTHING_RESOURCE_URI.to_string(),
        label: "Current Context".to_string(),
    });

    assert_eq!(
        thread.context_bootstrap_state,
        AgentChatContextBootstrapState::Ready
    );
    assert_eq!(thread.context_bootstrap_note, None);
    assert!(!thread.pending_ambient_context_enabled);
}

#[test]
fn successful_context_resolution_clears_prior_failure_note() {
    let mut thread = test_thread(Vec::new(), false);

    thread.add_context_part_test(crate::ai::message_parts::AiContextPart::FilePath {
        path: "/tmp/script-kit-gpui-missing-context.txt".to_string(),
        label: "Missing Context".to_string(),
    });

    let failed = thread.prepare_turn_blocks_with_receipt("first");
    assert!(
        failed
            .receipt
            .as_ref()
            .is_some_and(|receipt| !receipt.failures.is_empty()),
        "missing file should surface as a context resolution failure"
    );
    thread.set_context_resolution_note(failed.receipt.as_ref());
    assert_eq!(
        thread
            .context_bootstrap_note
            .as_ref()
            .map(|note| note.as_ref()),
        Some("1 context attachment unavailable · Missing Context")
    );

    thread.remove_context_part_test(0);
    thread.add_context_part_test(focused_target_part("my-script"));

    let successful = thread.prepare_turn_blocks_with_receipt("second");
    assert!(
        successful
            .receipt
            .as_ref()
            .is_some_and(|receipt| receipt.failures.is_empty()),
        "focused target should resolve cleanly"
    );
    thread.set_context_resolution_note(successful.receipt.as_ref());

    assert_eq!(
        thread.context_bootstrap_note, None,
        "a clean follow-up submit should clear stale failure messaging"
    );
}

/// The submitted user message must carry a visible receipt of what text
/// was attached and where it came from (e.g. `Draft — TextEdit` plus a
/// snippet), so a rewrite never sends invisible context.
#[test]
fn prepared_turn_carries_attachment_receipts_for_transcript() {
    let mut thread = test_thread(Vec::new(), false);
    thread.add_context_part_test(crate::ai::message_parts::AiContextPart::TextBlock {
        label: "Draft \u{2014} TextEdit".to_string(),
        source: "frontmost-app#selection=full".to_string(),
        text: "This  draft\nspans   whitespace and should collapse.".to_string(),
        mime_type: None,
    });

    let prepared = thread.prepare_turn_blocks_with_receipt("rewrite this");

    assert_eq!(prepared.attachments.len(), 1);
    let attachment = &prepared.attachments[0];
    assert_eq!(attachment.label.as_ref(), "Draft \u{2014} TextEdit");
    assert_eq!(
        attachment.snippet.as_ref().map(|s| s.as_ref()),
        Some("This draft spans whitespace and should collapse."),
        "snippet must be whitespace-collapsed attached text"
    );

    // No pending context → no receipts.
    let mut clean = test_thread(Vec::new(), false);
    let empty = clean.prepare_turn_blocks_with_receipt("hello");
    assert!(empty.attachments.is_empty());
}

// ── current_setup_requirements tests ─────────────────────

#[test]
fn current_setup_requirements_default_when_empty() {
    let thread = test_thread(Vec::new(), false);
    let reqs = thread.current_setup_requirements();
    assert!(
        !reqs.needs_embedded_context,
        "no pending parts/blocks → no embedded context"
    );
    assert!(!reqs.needs_image, "no screenshot parts → no image");
}

#[test]
fn current_setup_requirements_reflects_pending_blocks() {
    let thread = test_thread(
        vec![ContentBlock::Text(TextContent::new("some context"))],
        false,
    );
    let reqs = thread.current_setup_requirements();
    assert!(
        reqs.needs_embedded_context,
        "pending_context_blocks should set needs_embedded_context"
    );
    assert!(!reqs.needs_image, "text block should not set needs_image");
}

#[test]
fn current_setup_requirements_reflects_pending_parts() {
    let mut thread = test_thread(Vec::new(), false);
    thread.add_context_part_test(crate::ai::message_parts::AiContextPart::ResourceUri {
        uri: "kit://context?profile=minimal".to_string(),
        label: "Current Context".to_string(),
    });
    let reqs = thread.current_setup_requirements();
    assert!(
        reqs.needs_embedded_context,
        "pending_context_parts should set needs_embedded_context"
    );
    assert!(
        !reqs.needs_image,
        "non-screenshot part should not set needs_image"
    );
}

#[test]
fn current_setup_requirements_detects_screenshot_part() {
    let mut thread = test_thread(Vec::new(), false);
    thread.add_context_part_test(crate::ai::message_parts::AiContextPart::ResourceUri {
        uri: "kit://context?screenshot=1".to_string(),
        label: "Screenshot".to_string(),
    });
    let reqs = thread.current_setup_requirements();
    assert!(
        reqs.needs_embedded_context,
        "screenshot part implies embedded context"
    );
    assert!(reqs.needs_image, "screenshot part should set needs_image");
}

#[test]
fn current_setup_requirements_unions_with_launch_requirements() {
    let mut thread = test_thread(Vec::new(), false);
    thread.launch_requirements = crate::ai::agent_chat::ui::AgentChatLaunchRequirements {
        needs_embedded_context: true,
        needs_image: false,
    };
    // No pending parts/blocks — should still reflect launch_requirements.
    let reqs = thread.current_setup_requirements();
    assert!(
        reqs.needs_embedded_context,
        "should preserve launch needs_embedded_context"
    );
    assert!(!reqs.needs_image, "no screenshot added → false");

    // Now add screenshot part — should union to true.
    thread.add_context_part_test(crate::ai::message_parts::AiContextPart::ResourceUri {
        uri: "kit://context?screenshot=1".to_string(),
        label: "Screenshot".to_string(),
    });
    let reqs = thread.current_setup_requirements();
    assert!(reqs.needs_embedded_context, "still true from launch");
    assert!(reqs.needs_image, "screenshot part added after open → true");
}

#[test]
fn reset_pending_context_for_new_entry_intent_preserves_messages_but_clears_context_state() {
    let mut thread = test_thread(vec![ContentBlock::Text(TextContent::new("context"))], false);
    thread.messages.push(AgentChatThreadMessage::new(
        1,
        AgentChatThreadMessageRole::Assistant,
        "existing response",
    ));
    thread.add_context_part_test(focused_target_part("existing-chip"));
    thread.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
    thread.context_bootstrap_note = Some("Capturing Current Context…".into());
    thread.queued_submit_while_bootstrapping = true;

    thread.reset_pending_context_for_new_entry_intent();

    assert_eq!(thread.messages.len(), 1, "transcript history should remain");
    assert!(
        thread.pending_context_parts.is_empty(),
        "stale composer chips must be cleared before reusing the thread"
    );
    assert!(
        thread.pending_context_blocks.is_empty(),
        "hidden staged context must be cleared before reusing the thread"
    );
    assert_eq!(
        thread.context_bootstrap_state,
        AgentChatContextBootstrapState::Ready,
        "reused entry intents must not stay stuck behind old bootstrap work"
    );
    assert_eq!(
        thread.context_bootstrap_note, None,
        "stale bootstrap messaging should be cleared"
    );
    assert!(
        !thread.queued_submit_while_bootstrapping,
        "reused entry intents should not inherit an old queued submit"
    );
}

#[test]
fn replace_pending_context_parts_clears_previous_parts_and_resets_consumption() {
    let mut thread = test_thread(vec![ContentBlock::Text(TextContent::new("hidden"))], false);
    thread.add_context_part_test(focused_target_part("old-chip"));
    thread.pending_context_consumed = true;
    thread.pending_ambient_context_enabled = true;
    thread.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
    thread.context_bootstrap_note = Some("Capturing Current Context…".into());
    thread.queued_submit_while_bootstrapping = true;

    let replacement = vec![crate::ai::message_parts::AiContextPart::TextBlock {
        label: "Selected Text".to_string(),
        source: "notes://123#selection=0-5".to_string(),
        text: "hello".to_string(),
        mime_type: None,
    }];

    thread.replace_pending_context_parts_test(replacement.clone(), "test_replace");

    assert_eq!(thread.pending_context_parts, replacement);
    assert!(
        thread.pending_context_blocks.is_empty(),
        "replacing pending parts should clear hidden staged blocks"
    );
    assert!(
        !thread.pending_context_consumed,
        "replacing pending parts should re-arm first-submit consumption"
    );
    assert!(
        !thread.pending_ambient_context_enabled,
        "non-ambient replacement should disable stale ambient state"
    );
    assert_eq!(
        thread.context_bootstrap_state,
        AgentChatContextBootstrapState::Ready,
        "non-ambient replacement should clear stale bootstrap state"
    );
    assert_eq!(
        thread.context_bootstrap_note, None,
        "non-ambient replacement should clear stale bootstrap note"
    );
    assert!(
        !thread.queued_submit_while_bootstrapping,
        "replacement should clear stale queued submit state"
    );
}

// ── WP-B2: context/tool admission boundary ───────────────────────────────

use crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy;

fn file_context_part() -> AiContextPart {
    AiContextPart::FilePath {
        path: "/tmp/secret.txt".to_string(),
        label: "secret.txt".to_string(),
    }
}

fn skill_context_part() -> AiContextPart {
    AiContextPart::SkillFile {
        path: "/tmp/skill.md".to_string(),
        label: "Deploy".to_string(),
        skill_name: "deploy".to_string(),
        owner_label: "owner".to_string(),
        slash_name: "deploy".to_string(),
    }
}

fn tool_started(tool_name: &str) -> AgentChatEvent {
    AgentChatEvent::ToolCallStarted {
        tool_call_id: "tc-1".to_string(),
        title: "Run".to_string(),
        status: "in_progress".to_string(),
        tool_name: Some(tool_name.to_string()),
        raw_input: Some(serde_json::json!({ "command": "rm -rf /" })),
    }
}

/// WP-B2: the constructor is a context ingress — a Quick AI launch drops every
/// forbidden initial part, while a Full launch keeps them.
#[test]
fn quick_ai_init_rejects_nonempty_context_parts() {
    let parts = vec![file_context_part(), skill_context_part()];
    let denied = AgentChatThread::filter_admissible_parts_for_policy(
        AgentChatSessionPolicy::QuickAi,
        parts.clone(),
    );
    assert!(
        denied.is_empty(),
        "Quick AI must be born holding zero context parts"
    );
    let kept = AgentChatThread::filter_admissible_parts_for_policy(
        AgentChatSessionPolicy::Full,
        parts.clone(),
    );
    assert_eq!(kept.len(), parts.len(), "Full keeps every initial part");
}

/// WP-B2: a draft captured on a Full surface cannot smuggle its context into a
/// Quick AI thread on restore.
#[test]
fn quick_ai_restore_draft_rejects_cross_policy_context() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    thread.restore_draft_snapshot_inner(AgentChatThreadDraftSnapshot {
        input: "what is rust".to_string(),
        input_cursor: 0,
        pending_context_parts: vec![file_context_part()],
        pending_context_consumed: false,
    });
    assert_eq!(thread.input.text(), "what is rust");
    assert!(
        thread.pending_context_parts().is_empty(),
        "cross-policy draft context must be stripped on restore"
    );

    // A Full thread keeps the restored context.
    let mut full = test_thread(Vec::new(), false);
    full.restore_draft_snapshot_inner(AgentChatThreadDraftSnapshot {
        input: "keep".to_string(),
        input_cursor: 0,
        pending_context_parts: vec![file_context_part()],
        pending_context_consumed: false,
    });
    assert_eq!(full.pending_context_parts().len(), 1);
}

/// WP-B2: even if a part is force-planted into pending state, queueing the
/// composer strips it so a later dequeue cannot resurrect forbidden context.
#[test]
fn quick_ai_queue_cannot_smuggle_context() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    // Bypass the ingress guard to simulate a smuggled part already resident.
    thread.add_context_part_test(file_context_part());
    assert_eq!(thread.pending_context_parts().len(), 1);

    thread.queue_current_composer("follow up".to_string());
    let queued = thread.queued_messages();
    assert_eq!(queued.len(), 1);
    assert!(
        queued[0].context_parts.is_empty(),
        "queued message must carry no forbidden context forward"
    );
}

/// WP-B2: the provider boundary sees no context blocks for a Quick AI turn.
#[test]
fn quick_ai_turn_request_contains_no_context_blocks() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    // Replace routes through the filtering ingress → denied → empty.
    thread.replace_pending_context_parts_test(vec![file_context_part()], "test");
    assert!(thread.pending_context_parts().is_empty());

    let blocks = thread.prepare_turn_blocks("what is rust");
    assert_eq!(
        blocks.len(),
        1,
        "only the user-text block reaches the provider"
    );
    match &blocks[0] {
        ContentBlock::Text(text) => assert_eq!(text.text, "what is rust"),
        other => panic!("expected a single user-text block, got {other:?}"),
    }
}

/// WP-B2: the turn request carries the web-search-only tool policy for Quick AI
/// and the full policy otherwise.
#[test]
fn quick_ai_turn_request_has_web_search_only_tool_policy() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    let request = thread.turn_request(vec![ContentBlock::Text(TextContent::new("hi"))]);
    assert_eq!(request.tool_policy, AgentChatToolPolicy::WebSearchOnly);

    thread.set_session_policy_test(AgentChatSessionPolicy::Full);
    let request = thread.turn_request(vec![ContentBlock::Text(TextContent::new("hi"))]);
    assert_eq!(request.tool_policy, AgentChatToolPolicy::Full);
}

#[test]
fn quick_ai_handoff_seed_preserves_question_and_only_safe_source_urls() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    thread.push_message(
        AgentChatThreadMessageRole::User,
        "What changed in the latest Rust release?",
    );
    thread.upsert_tool_call_start(
        "search-1".to_string(),
        "Web search".to_string(),
        "running".to_string(),
        Some("web_search".to_string()),
        Some(serde_json::json!({"query": "latest Rust release"})),
    );
    thread.apply_tool_call_update(
        "search-1".to_string(),
        None,
        Some("complete".to_string()),
        Some(
            "https://blog.rust-lang.org/releases/latest/ not-a-url https://blog.rust-lang.org/releases/latest/"
                .to_string(),
        ),
        None,
        None,
        false,
    );
    thread.push_message(
        AgentChatThreadMessageRole::Assistant,
        "The release notes are also mirrored at https://doc.rust-lang.org/releases.html; \
         ignore file:///tmp/provider-debug.json and token=secret.",
    );

    let seed = thread.quick_ai_handoff_seed().expect("Quick AI handoff");
    assert!(seed.starts_with("Quick AI handoff"));
    assert!(seed.contains("Original question:\nWhat changed in the latest Rust release?"));
    assert!(seed.contains("- https://blog.rust-lang.org/releases/latest/"));
    assert_eq!(
        seed.matches("https://blog.rust-lang.org/releases/latest/")
            .count(),
        1
    );
    assert!(seed.contains("- https://doc.rust-lang.org/releases.html"));
    assert!(!seed.contains("not-a-url"));
    assert!(!seed.contains("provider-debug"));
    assert!(!seed.contains("token=secret"));
    assert!(!seed.contains("search-1"));
    assert!(!seed.contains("web_search"));
}

/// WP-B2: a forbidden tool-call start fails the turn closed and never renders
/// the tool call (nor its raw args).
#[test]
fn quick_ai_forbidden_tool_event_fails_closed() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    thread.push_message(AgentChatThreadMessageRole::User, "run a command");

    thread.apply_event_test(tool_started("bash"));

    assert_eq!(thread.status, AgentChatThreadStatus::Error);
    assert!(
        thread.active_tool_calls().is_empty(),
        "a forbidden tool call must never be tracked/rendered"
    );

    // The canonical web-search tool is allowed and IS tracked.
    let mut allowed = test_thread(Vec::new(), false);
    allowed.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    allowed.apply_event_test(tool_started("web_search"));
    assert_eq!(allowed.active_tool_calls().len(), 1);
    assert_ne!(allowed.status, AgentChatThreadStatus::Error);
}

/// WP-B2: a web-search-only session never presents a permission prompt — its
/// single tool needs no approval, so the listener rejects any request.
#[test]
fn quick_ai_forbidden_permission_request_is_never_presented() {
    let quick = AgentChatSessionPolicy::QuickAi;
    assert!(!quick.tool_policy().allows_permission_prompts());
    let full = AgentChatSessionPolicy::Full;
    assert!(full.tool_policy().allows_permission_prompts());
}

/// WP-B2: skills are denied for Quick AI (Oracle seat 2 overrules the earlier
/// slash-skill allowance) and admitted for Full.
#[test]
fn quick_ai_skill_file_is_denied_or_promotes_to_full() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    assert!(thread.admit_context_part(&skill_context_part()).is_err());

    thread.set_session_policy_test(AgentChatSessionPolicy::Full);
    assert!(thread.admit_context_part(&skill_context_part()).is_ok());
}

/// WP-B2: literal flow text (`-`) typed into the composer stays plain user
/// text — it is never converted into a context part.
#[test]
fn quick_ai_literal_flow_staging_remains_plain_user_text() {
    let mut thread = test_thread(Vec::new(), false);
    thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
    thread.input.set_text("- deploy the app");

    let blocks = thread.prepare_turn_blocks("- deploy the app");
    assert!(
        thread.pending_context_parts().is_empty(),
        "literal flow text must not become a context part"
    );
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Text(text) => assert_eq!(text.text, "- deploy the app"),
        other => panic!("expected plain user text, got {other:?}"),
    }
}

// ===========================================================================
// WP-B3 real-stream behavior contracts
//
// These drive the SAME reduction the live `bind_stream` drain task feeds — the
// `StreamingTextBuffer` push/drain and `append_assistant_stream_delta` /
// `append_chunk` coalescing — synchronously, so they are deterministic (the
// live 16 ms `smol::Timer` drain loop cannot run under gpui's deterministic
// test scheduler; the channel+spawn ingress is exercised end-to-end by the
// runtime probes). Every assertion is on the EXACT final source text/bytes.
// ===========================================================================

/// Drive the real per-tick drain to completion, exactly as the live drain task
/// loops it, and return how many drain ticks committed visible text.
fn drain_streaming_to_completion(thread: &mut AgentChatThread) -> usize {
    let mut committing_ticks = 0;
    // Safety bound: far above any realistic backlog, guards a logic bug from
    // hanging the test rather than looping forever.
    for _ in 0..100_000 {
        if thread.streaming_text_buffer.is_empty() {
            break;
        }
        if thread.drain_streaming_text_once() {
            committing_ticks += 1;
        }
    }
    committing_ticks
}

/// The last assistant row's exact body text.
fn assistant_body(thread: &AgentChatThread) -> String {
    thread
        .messages
        .iter()
        .rev()
        .find(|m| m.role == AgentChatThreadMessageRole::Assistant)
        .map(|m| m.body.to_string())
        .unwrap_or_default()
}

#[test]
fn agent_real_stream_preserves_exact_final_utf8_bytes() {
    let mut thread = test_thread(Vec::new(), false);
    // Multi-byte graphemes split across arbitrary chunk boundaries, markdown
    // delimiters crossing chunks, plus an empty delta in the middle.
    let chunks = [
        "The **caf",
        "é** ",
        "",
        "— 日本",
        "語 — 🚀 dep",
        "loys `na",
        "ïve()`",
    ];
    let expected: String = chunks.concat();
    for chunk in chunks {
        thread.streaming_text_buffer.push_chunk(chunk.to_string());
    }
    drain_streaming_to_completion(&mut thread);

    let body = assistant_body(&thread);
    // Exact final bytes, not a length check — a mid-grapheme split would either
    // panic in the drain or corrupt the bytes here.
    assert_eq!(body, expected);
    assert_eq!(body.as_bytes(), expected.as_bytes());
}

#[test]
fn agent_queue_preserves_order_and_no_duplicate_text() {
    let mut thread = test_thread(Vec::new(), false);
    // Interleave pushes and drains within (simulated) separate ticks — several
    // deltas can arrive inside one drain window and later ones after.
    thread.streaming_text_buffer.push_chunk("one ".to_string());
    thread.streaming_text_buffer.push_chunk("two ".to_string());
    drain_streaming_to_completion(&mut thread);
    thread
        .streaming_text_buffer
        .push_chunk("three ".to_string());
    thread.streaming_text_buffer.push_chunk("four".to_string());
    drain_streaming_to_completion(&mut thread);

    assert_eq!(assistant_body(&thread), "one two three four");
    // Exactly one assistant row — deltas coalesce, they never spawn duplicates.
    let assistant_rows = thread
        .messages
        .iter()
        .filter(|m| m.role == AgentChatThreadMessageRole::Assistant)
        .count();
    assert_eq!(
        assistant_rows, 1,
        "streaming deltas must not duplicate rows"
    );
}

#[test]
fn agent_terminal_flush_commits_exact_tail_once() {
    let mut thread = test_thread(Vec::new(), false);
    thread
        .streaming_text_buffer
        .push_chunk("partial reveal then ".to_string());
    // One drain tick reveals a prefix, then a terminal event flushes the rest
    // before the next scheduled drain would have run.
    let _ = thread.drain_streaming_text_once();
    thread
        .streaming_text_buffer
        .push_chunk("the exact tail.".to_string());
    let flushed = thread.flush_streaming_text_buffer();
    assert!(flushed, "a non-empty terminal flush commits");

    assert_eq!(
        assistant_body(&thread),
        "partial reveal then the exact tail."
    );
    // A second flush with an empty buffer must be a no-op (tail committed once).
    assert!(!thread.flush_streaming_text_buffer());
    assert_eq!(
        assistant_body(&thread),
        "partial reveal then the exact tail."
    );
}

#[test]
fn agent_history_resume_forces_full_reset_without_duplicate_rows() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "first answer");
    // A history resume bumps the transcript generation and repopulates rows;
    // the freshly streamed assistant text must land in ONE new row, never
    // re-append onto the resumed history row.
    thread.bump_transcript_generation("history_resume_test");
    thread.push_message(AgentChatThreadMessageRole::User, "second ask");
    thread
        .streaming_text_buffer
        .push_chunk("second answer".to_string());
    drain_streaming_to_completion(&mut thread);

    assert_eq!(assistant_body(&thread), "second answer");
    let answers: Vec<&str> = thread
        .messages
        .iter()
        .filter(|m| m.role == AgentChatThreadMessageRole::Assistant)
        .map(|m| m.body.as_ref())
        .collect();
    assert_eq!(answers, vec!["first answer", "second answer"]);
}

#[test]
fn agent_fork_forces_full_reset_without_stale_append() {
    let mut thread = test_thread(Vec::new(), false);
    thread.push_message(AgentChatThreadMessageRole::User, "first ask");
    thread.push_message(AgentChatThreadMessageRole::Assistant, "stale answer");
    thread.push_message(AgentChatThreadMessageRole::User, "second ask");
    // Stage a fork at the second user turn, then a ForkCompleted truncates back
    // and stages the edited text into the composer.
    thread.pending_fork_ordinal = Some(1);
    thread.apply_event_test(AgentChatEvent::ForkCompleted {
        text: "second ask".to_string(),
    });
    // The user re-submits the edited turn (a fresh User row), then a new answer
    // streams. It must open a FRESH assistant row, never re-append onto any
    // pre-fork assistant row.
    thread.push_message(AgentChatThreadMessageRole::User, "second ask edited");
    thread
        .streaming_text_buffer
        .push_chunk("fresh answer".to_string());
    drain_streaming_to_completion(&mut thread);

    assert_eq!(assistant_body(&thread), "fresh answer");
    assert!(
        !thread
            .messages
            .iter()
            .any(|m| m.body.as_ref().contains("stale answerfresh answer")),
        "streamed text must not re-append onto a forked-away row"
    );
}

// ---- WP-B3 text-engine append contracts (bin target: `text_append`) -------

#[gpui::test]
fn text_append_updates_logical_and_parsed_source(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    cx.update(gpui_component::init);
    let state = cx.new(|cx| gpui_component::text::TextViewState::markdown_immediate("Hello ", cx));
    state.update(cx, |state, cx| {
        state.push_str_immediate("brave ", cx);
        state.push_str_immediate("new world", cx);
        assert_eq!(state.source_string_for_test(), "Hello brave new world");
    });
}

#[gpui::test]
fn text_full_then_append_coalescing_does_not_duplicate_source(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    cx.update(gpui_component::init);
    let state = cx.new(|cx| gpui_component::text::TextViewState::markdown_immediate("", cx));
    state.update(cx, |state, cx| {
        // A full replacement, then a streaming append — the prefix must appear
        // exactly once (the coalescer/transactional-append bug would duplicate).
        state.set_markdown_text_immediate("Base document.", cx);
        state.push_str_immediate(" Streamed tail.", cx);
        assert_eq!(
            state.source_string_for_test(),
            "Base document. Streamed tail."
        );
    });
}

#[gpui::test]
fn text_append_then_full_replacement_wins(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    cx.update(gpui_component::init);
    let state = cx.new(|cx| gpui_component::text::TextViewState::markdown_immediate("start", cx));
    state.update(cx, |state, cx| {
        state.push_str_immediate(" appended", cx);
        // A subsequent full replacement wins outright — no residue of the append.
        state.set_markdown_text_immediate("completely replaced", cx);
        assert_eq!(state.source_string_for_test(), "completely replaced");
    });
}

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
        active_callout: None,
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
        available_models: Vec::new(),
        selected_model_id: None,
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
fn models_available_falls_back_to_current_when_selection_dropped() {
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
        Some("claude-opus-4-7"),
        "selection should fall back to the agent's declared current model"
    );
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

    apply_event_test(
        &mut thread,
        AgentChatEvent::TurnFinished {
            stop_reason: "end_turn".into(),
        },
    );

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

    thread.apply_event_test(AgentChatEvent::TurnFinished {
        stop_reason: "end_turn".into(),
    });

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

    thread.apply_event_test(AgentChatEvent::TurnFinished {
        stop_reason: "cancelled".into(),
    });

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
fn failed_event_creates_error_message_and_retryable_callout() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");

    apply_event_test(
        &mut thread,
        AgentChatEvent::Failed {
            error: "connection lost".into(),
        },
    );

    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[1].role, AgentChatThreadMessageRole::Error);
    assert_eq!(thread.messages[1].body.as_ref(), "connection lost");
    assert_eq!(thread.status, AgentChatThreadStatus::Error);
    let callout = thread.active_callout().expect("failed turn arms callout");
    assert_eq!(callout.severity, AgentChatCalloutSeverity::Error);
    assert_eq!(callout.title.as_ref(), "Turn failed");
    assert_eq!(callout.detail.as_ref().unwrap().as_ref(), "connection lost");
    assert!(callout.can_retry);
}

#[test]
fn usage_limit_failure_surfaces_account_recovery_without_raw_json_as_message() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    let raw_error = r#"{"error":{"type":"usage_limit_reached","status":429}}"#;

    apply_event_test(
        &mut thread,
        AgentChatEvent::Failed {
            error: raw_error.into(),
        },
    );

    let callout = thread.active_callout().expect("failure arms callout");
    assert_eq!(callout.title.as_ref(), "Account usage limit reached");
    assert_eq!(
        callout.auth_recovery,
        Some(AgentChatAuthRecovery::UsageLimitReached)
    );
    assert!(callout.detail.as_ref().unwrap().contains("Switch accounts"));
    assert_eq!(callout.raw_detail.as_ref().unwrap().as_ref(), raw_error);
    assert!(!thread.messages[1].body.contains("{\"error\""));
    assert!(thread.messages[1].body.contains("Switch accounts"));
}

#[test]
fn retry_from_error_reenters_streaming_without_duplicate_user_message() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    thread.apply_event_test(AgentChatEvent::Failed {
        error: "connection lost".into(),
    });
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
    assert!(thread.active_callout().is_none());
}

#[test]
fn dismiss_clears_failed_turn_callout() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    thread.apply_event_test(AgentChatEvent::Failed {
        error: "connection lost".into(),
    });

    thread.dismiss_active_callout_test();

    assert!(thread.active_callout().is_none());
}

#[test]
fn starting_new_turn_clears_failed_turn_callout() {
    let mut thread = test_thread(Vec::new(), true);
    thread.push_message(AgentChatThreadMessageRole::User, "please try");
    thread.apply_event_test(AgentChatEvent::Failed {
        error: "connection lost".into(),
    });

    thread.retry_last_user_turn_test().unwrap();

    assert!(thread.active_callout().is_none());
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

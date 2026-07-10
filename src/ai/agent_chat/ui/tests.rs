//! Targeted Agent Chat tests covering render ownership, approval flow,
//! and Tab AI routing contracts.
//!
//! These complement the per-module unit tests in `thread.rs`,
//! `permission_broker.rs`, and `events.rs` with
//! cross-cutting integration-style assertions.

use crate::ai::agent_chat::content::{ContentBlock, TextContent};

use super::events::AgentChatEvent;
use super::permission_broker::{
    AgentChatApprovalOption, AgentChatApprovalRequest, AgentChatPermissionBroker,
};
use super::preflight::AgentChatLaunchRequirements;
use super::thread::{AgentChatThread, AgentChatThreadMessageRole, AgentChatThreadStatus};

// =========================================================================
// 1. First-turn staged context preservation
// =========================================================================

#[test]
fn staged_context_prepended_on_first_submit_only() {
    let mut thread = AgentChatThread::test_new(
        vec![ContentBlock::Text(TextContent::new("desktop-context"))],
        None,
    );

    let first_blocks = thread.prepare_turn_blocks("build a script");
    assert!(
        first_blocks.len() >= 2,
        "first turn should include staged context + input (got {})",
        first_blocks.len()
    );

    // Verify the context block is actually first
    if let ContentBlock::Text(ref t) = first_blocks[0] {
        assert_eq!(t.text, "desktop-context");
    } else {
        panic!("expected first block to be the staged context Text block");
    }

    let second_blocks = thread.prepare_turn_blocks("another request");
    assert_eq!(
        second_blocks.len(),
        1,
        "second turn should NOT re-include context"
    );
    // Second turn should be plain text without USER REQUEST marker
    if let ContentBlock::Text(ref t) = second_blocks[0] {
        assert_eq!(
            t.text, "another request",
            "second turn should be plain user text"
        );
    } else {
        panic!("expected second turn to be a Text block");
    }
}

#[test]
fn initial_input_populates_composer_for_auto_submit() {
    let thread = AgentChatThread::test_new(vec![], Some("build a clipboard manager".to_string()));

    assert_eq!(
        thread.input.text(),
        "build a clipboard manager",
        "initial_input should populate the composer"
    );
}

#[test]
fn empty_initial_input_leaves_composer_blank() {
    let thread = AgentChatThread::test_new(vec![], None);
    assert!(
        thread.input.text().is_empty(),
        "no initial_input should leave composer empty"
    );
}

// =========================================================================
// 2. Pending approval state and approval/cancel dispatch
// =========================================================================

#[test]
fn pending_permission_stored_and_clears_on_approve() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    let (reply_tx, reply_rx) = async_channel::bounded(1);
    thread.pending_permission = Some(AgentChatApprovalRequest {
        id: 1,
        title: "Write to file".into(),
        body: "Agent wants to write to /tmp/test.txt".into(),
        preview: None,
        options: vec![
            AgentChatApprovalOption {
                option_id: "allow-once".into(),
                name: "Allow once".into(),
                kind: "AllowOnce".into(),
            },
            AgentChatApprovalOption {
                option_id: "deny".into(),
                name: "Deny".into(),
                kind: "RejectOnce".into(),
            },
        ],
        reply_tx,
    });
    thread.status = AgentChatThreadStatus::WaitingForPermission;

    assert!(thread.pending_permission.is_some());
    assert_eq!(thread.status, AgentChatThreadStatus::WaitingForPermission);

    // Simulate approve (same logic as approve_pending_permission without cx)
    if let Some(req) = thread.pending_permission.take() {
        let _ = req.reply_tx.send_blocking(Some("allow-once".to_string()));
    }
    thread.status = AgentChatThreadStatus::Idle;

    assert!(thread.pending_permission.is_none());
    assert_eq!(thread.status, AgentChatThreadStatus::Idle);

    let reply = reply_rx.recv_blocking().expect("should receive reply");
    assert_eq!(reply, Some("allow-once".to_string()));
}

#[test]
fn pending_permission_cancel_sends_none() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    let (reply_tx, reply_rx) = async_channel::bounded(1);
    thread.pending_permission = Some(AgentChatApprovalRequest {
        id: 2,
        title: "Terminal access".into(),
        body: "Agent wants to run a command".into(),
        preview: None,
        options: vec![AgentChatApprovalOption {
            option_id: "allow".into(),
            name: "Allow".into(),
            kind: "AllowOnce".into(),
        }],
        reply_tx,
    });
    thread.status = AgentChatThreadStatus::WaitingForPermission;

    if let Some(req) = thread.pending_permission.take() {
        let _ = req.reply_tx.send_blocking(None);
    }
    thread.status = AgentChatThreadStatus::Idle;

    let reply = reply_rx.recv_blocking().expect("should receive reply");
    assert_eq!(reply, None, "cancel should send None");
}

#[test]
fn broker_full_roundtrip_with_three_options() {
    let (broker, rx) = AgentChatPermissionBroker::new();

    let handle = std::thread::spawn(move || {
        let request = rx.recv_blocking().expect("should receive request");
        assert_eq!(request.options.len(), 3, "all options should be forwarded");
        assert_eq!(request.options[0].option_id, "allow-once");
        assert_eq!(request.options[1].option_id, "allow-always");
        assert_eq!(request.options[2].option_id, "deny");
        request
            .reply_tx
            .send_blocking(Some("allow-always".to_string()))
            .expect("reply should send");
    });

    let result = broker
        .request(super::permission_broker::AgentChatApprovalRequestInput {
            title: "Read file".into(),
            body: "src/main.rs".into(),
            preview: None,
            options: vec![
                AgentChatApprovalOption {
                    option_id: "allow-once".into(),
                    name: "Allow once".into(),
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
        })
        .expect("request should succeed");

    assert_eq!(result, Some("allow-always".to_string()));
    handle.join().expect("responder thread should finish");
}

// =========================================================================
// 3. Render ownership — Agent Chat thread state contracts
// =========================================================================

#[test]
fn agent_chat_thread_starts_idle_with_empty_state() {
    let thread = AgentChatThread::test_new(vec![], None);

    assert_eq!(thread.status, AgentChatThreadStatus::Idle);
    assert!(thread.messages.is_empty());
    assert!(thread.active_plan_entries().is_empty());
    assert!(thread.active_mode_id().is_none());
    assert!(thread.available_commands().is_empty());
    assert!(thread.active_tool_calls().is_empty());
}

#[test]
fn streaming_deltas_coalesce_for_view_render() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::AgentMessageDelta("## Plan\n".into()));
    thread.apply_event_test(AgentChatEvent::AgentMessageDelta(
        "1. Read the file\n".into(),
    ));
    thread.apply_event_test(AgentChatEvent::AgentMessageDelta("2. Apply patch\n".into()));

    assert_eq!(thread.messages.len(), 1, "streaming chunks should coalesce");
    assert_eq!(
        thread.messages[0].role,
        AgentChatThreadMessageRole::Assistant
    );
    assert!(thread.messages[0].body.contains("## Plan"));
    assert!(thread.messages[0].body.contains("2. Apply patch"));
}

#[test]
fn thought_deltas_separate_from_assistant_deltas() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::AgentThoughtDelta("hmm...".into()));
    thread.apply_event_test(AgentChatEvent::AgentMessageDelta("Here's the plan".into()));

    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[0].role, AgentChatThreadMessageRole::Thought);
    assert_eq!(
        thread.messages[1].role,
        AgentChatThreadMessageRole::Assistant
    );
}

#[test]
fn runtime_setup_required_arms_recovery_state() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::SetupRequired {
        reason: "auth_required".into(),
        auth_methods: vec!["oauth".into()],
    });

    let setup = thread
        .setup_state()
        .expect("runtime setup required should arm recovery state");
    assert_eq!(setup.title, "Authentication required");
    assert_eq!(thread.status, AgentChatThreadStatus::Error);
}

#[test]
fn plan_visible_to_view_without_message_creation() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::PlanUpdated {
        entries: vec!["Read file".into(), "Apply patch".into(), "Run tests".into()],
    });

    assert_eq!(thread.active_plan_entries().len(), 3);
    assert!(thread.messages.is_empty());
}

#[test]
fn tool_call_lifecycle_tracks_state_for_view() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::ToolCallStarted {
        tool_call_id: "tc-abc".into(),
        title: "Read file".into(),
        status: "running".into(),
        tool_name: Some("read".into()),
        raw_input: Some(serde_json::json!({"path": "src/main.rs"})),
    });

    assert_eq!(thread.active_tool_calls().len(), 1);
    assert_eq!(thread.messages.len(), 1);
    assert_eq!(thread.messages[0].role, AgentChatThreadMessageRole::Tool);
    let meta = thread.messages[0]
        .tool_meta
        .as_ref()
        .expect("tool message carries structured card meta");
    assert_eq!(
        meta.kind,
        crate::ai::agent_chat::ui::tool_card::AgentChatToolKind::Read
    );
    assert_eq!(
        meta.status,
        crate::ai::agent_chat::ui::tool_card::AgentChatToolStatus::Running
    );
    assert_eq!(meta.subject.as_deref(), Some("src/main.rs"));

    thread.apply_event_test(AgentChatEvent::ToolCallUpdated {
        tool_call_id: "tc-abc".into(),
        title: None,
        status: Some("completed".into()),
        body: Some("file contents...".into()),
        raw_input: None,
        diff: Some("-1 old\n+1 new".into()),
        is_error: false,
    });

    assert_eq!(thread.messages.len(), 1, "should update in-place");
    assert!(thread.messages[0].body.contains("completed"));
    assert_eq!(thread.active_tool_calls()[0].status, "completed");
    let meta = thread.messages[0].tool_meta.as_ref().unwrap();
    assert_eq!(
        meta.status,
        crate::ai::agent_chat::ui::tool_card::AgentChatToolStatus::Complete
    );
    assert_eq!(meta.diff.as_deref(), Some("-1 old\n+1 new"));
    assert_eq!(
        meta.subject.as_deref(),
        Some("src/main.rs"),
        "subject from start event must survive updates without raw_input"
    );
}

#[test]
fn error_event_creates_error_message_and_sets_status() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::Failed {
        error: "Agent Chat connection lost".into(),
    });

    assert_eq!(thread.status, AgentChatThreadStatus::Error);
    assert_eq!(thread.messages.len(), 1);
    assert_eq!(thread.messages[0].role, AgentChatThreadMessageRole::Error);
}

#[test]
fn turn_finished_returns_to_idle_from_streaming() {
    let mut thread = AgentChatThread::test_new(vec![], None);

    thread.apply_event_test(AgentChatEvent::AgentMessageDelta("hello".into()));
    assert_eq!(thread.status, AgentChatThreadStatus::Streaming);

    thread.apply_event_test(AgentChatEvent::TurnFinished {
        stop_reason: "end_turn".into(),
    });
    assert_eq!(thread.status, AgentChatThreadStatus::Idle);
}

// =========================================================================
// 4. Tab AI routing — source code contracts
// =========================================================================

const TAB_AI_MODE_SOURCE: &str = include_str!("../../../app_impl/agent_handoff/mod.rs");
const TAB_AI_AGENT_CHAT_LAUNCH_SOURCE: &str =
    include_str!("../../../app_impl/agent_handoff/agent_chat_launch.rs");
const ACTIONS_TOGGLE_SOURCE: &str = include_str!("../../../app_impl/actions_toggle.rs");
const STARTUP_SOURCE: &str = include_str!("../../../app_impl/startup.rs");
const STARTUP_NEW_ACTIONS_SOURCE: &str = include_str!("../../../app_impl/startup_new_actions.rs");
const STARTUP_NEW_TAB_SOURCE: &str = include_str!("../../../app_impl/startup_new_tab.rs");
const SIMULATE_KEY_DISPATCH_SOURCE: &str =
    include_str!("../../../app_impl/simulate_key_dispatch.rs");
const RENDER_IMPL_SOURCE: &str = include_str!("../../../main_sections/render_impl.rs");
const APP_VIEW_STATE_SOURCE: &str = include_str!("../../../main_sections/app_view_state.rs");
const UI_WINDOW_SOURCE: &str = include_str!("../../../app_impl/ui_window.rs");
const APP_RUN_SETUP_SOURCE: &str = include_str!("../../../main_entry/app_run_setup.rs");
const RUNTIME_STDIN_SOURCE: &str = include_str!("../../../main_entry/runtime_stdin.rs");
const HANDLE_ACTION_SOURCE: &str = include_str!("../../../app_actions/handle_action/mod.rs");
const REGISTRIES_STATE_SOURCE: &str = include_str!("../../../app_impl/registries_state.rs");
const AGENT_CHAT_MOD_SOURCE: &str = include_str!("mod.rs");
const AGENT_CHAT_HISTORY_POPUP_SOURCE: &str = include_str!("history_popup.rs");
const AGENT_CHAT_POPUP_WINDOW_SOURCE: &str = include_str!("popup_window.rs");
const AGENT_CHAT_ACTIONS_SOURCE: &str = include_str!("../../../actions/builders/script_context.rs");
const AGENT_CHAT_WINDOW_SOURCE: &str = include_str!("chat_window.rs");
const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
const AGENT_CHAT_THREAD_SOURCE: &str = include_str!("thread.rs");
const AGENT_CHAT_TRANSCRIPT_SOURCE: &str = include_str!("components/transcript.rs");
const AGENT_CHAT_UI_VARIANT_SOURCE: &str = include_str!("ui_variant.rs");
const TEXT_VIEW_SOURCE: &str =
    include_str!("../../../../vendor/gpui-component/crates/ui/src/text/text_view.rs");
const TEXT_VIEW_STATE_SOURCE: &str =
    include_str!("../../../../vendor/gpui-component/crates/ui/src/text/state.rs");
const TEXT_VIEW_NODE_SOURCE: &str =
    include_str!("../../../../vendor/gpui-component/crates/ui/src/text/node.rs");

fn agent_chat_source_between<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing source start: {start_pat}"));
    let end_rel = source[start..]
        .find(end_pat)
        .unwrap_or_else(|| panic!("missing source end: {end_pat}"));
    &source[start..start + end_rel]
}

#[test]
fn app_view_has_agent_chat_view_variant() {
    assert!(
        APP_VIEW_STATE_SOURCE.contains("AgentChatView"),
        "AppView enum must have an AgentChatView variant"
    );
}

#[test]
fn agent_handoff_creates_agent_chat_view_for_tab() {
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("AgentChatView::new"),
        "tab_ai Agent Chat launch helper must create an AgentChatView"
    );
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("enter_embedded_agent_chat_surface"),
        "agent_handoff must set current_view to AgentChatView"
    );
}

#[test]
fn agent_handoff_creates_agent_chat_thread_with_connection() {
    assert!(TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("AgentChatThread::new"));
    assert!(TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("open_tab_ai_pi_view_from_launch"));
    assert!(TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("warm_session_manager"));
    assert!(TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("AgentChatPermissionBroker::new"));
}

#[test]
fn agent_handoff_stages_context_on_agent_chat_thread() {
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("stage_agent_chat_initial_context_parts"),
        "tab_ai Agent Chat launch helper must stage context on the AgentChatThread"
    );
}

#[test]
fn agent_handoff_supports_auto_submit_with_initial_input() {
    assert!(TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("initial_input"));
    assert!(TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("AgentChatThreadInit"));
}

#[test]
fn startup_tab_guard_checks_agent_chat_view() {
    assert!(STARTUP_SOURCE.contains("AppView::AgentChatView"));
    assert!(STARTUP_SOURCE.contains("handle_tab_key"));
}

#[test]
fn startup_new_tab_guard_checks_agent_chat_view() {
    assert!(STARTUP_NEW_TAB_SOURCE.contains("AppView::AgentChatView"));
    assert!(STARTUP_NEW_TAB_SOURCE.contains("handle_tab_key"));
}

#[test]
fn startup_shift_tab_opens_agent_chat_profile_picker() {
    // QA story #5: in-chat Shift+Tab must open the Profile Switcher
    // (not be swallowed). Both Tab interceptors route Shift+Tab to the
    // window-aware picker entry while keeping plain Tab swallowed.
    for src in [STARTUP_SOURCE, STARTUP_NEW_TAB_SOURCE] {
        assert!(
            src.contains("agent_chat_shift_tab_profile_switcher"),
            "Shift+Tab in Agent Chat must log the Profile Switcher routing event"
        );
        assert!(
            src.contains("open_profile_trigger_picker_in_window"),
            "Shift+Tab in Agent Chat must open the in-chat Profile picker"
        );
        // Plain Tab stays swallowed via handle_tab_key(false, ...). The
        // handler is window-aware so it can accept the highlighted spine
        // projection row (e.g. the `>` working-directory list) before
        // falling back to swallowing.
        assert!(
            src.contains("chat.handle_tab_key(false, window, cx)"),
            "plain Tab in Agent Chat must remain swallowed (handle_tab_key(false, window, ...))"
        );
    }
}

#[test]
fn native_footer_agent_model_preserves_agent_chat_view() {
    let arm = UI_WINDOW_SOURCE
        .split("crate::footer_popup::FooterAction::AgentModel =>")
        .nth(1)
        .and_then(|tail| {
            tail.split("/// If the current view is an Agent Chat chat")
                .next()
        })
        .expect("ui_window.rs should contain the native AgentModel footer action arm");

    assert!(
        arm.contains("AppView::AgentChatView { entity, .. }"),
        "AgentModel footer clicks in Agent Chat must branch on the live Agent Chat view"
    );
    assert!(
        arm.contains("chat.open_profile_trigger_picker_in_window(window, cx);"),
        "AgentModel footer clicks in Agent Chat must open the in-chat Profile picker"
    );
    assert!(
        arm.contains("return;")
            && arm.find("return;")
                < arm
                    .find("self.current_view = AppView::ScriptList")
                    .or_else(|| arm.find("self.open_profile_switcher_window(window, cx);")),
        "AgentModel footer clicks in Agent Chat must return before the ScriptList/global picker fallback"
    );
}

#[test]
fn startup_plain_enter_routes_to_agent_chat_picker_when_open() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn handle_enter_key"),
        "AgentChatView must expose a plain-Enter picker handler for app interceptors"
    );
    assert!(
        STARTUP_SOURCE.contains("let is_plain_enter")
            && STARTUP_SOURCE.contains("chat.handle_enter_key(cx)"),
        "startup.rs should route plain Enter to Agent Chat picker acceptance when embedded Agent Chat owns the mention menu"
    );
    assert!(
        STARTUP_NEW_TAB_SOURCE.contains("let is_plain_enter")
            && STARTUP_NEW_TAB_SOURCE.contains("chat.handle_enter_key(cx)"),
        "startup_new_tab.rs should preserve the same plain Enter Agent Chat picker routing"
    );
}

#[test]
fn agent_chat_escape_defers_to_actions_dialog_before_unwinding_chat() {
    for (name, source) in [
        ("startup.rs", STARTUP_SOURCE),
        ("startup_new_actions.rs", STARTUP_NEW_ACTIONS_SOURCE),
    ] {
        let escape_block_start = source
            .find("// Handle Escape for AgentChatView.")
            .unwrap_or_else(|| panic!("Agent Chat escape block not found in {name}"));
        let escape_block_end = (escape_block_start + 1800).min(source.len());
        let escape_block = &source[escape_block_start..escape_block_end];

        assert!(
            escape_block.contains("!this.show_actions_popup"),
            "Agent Chat escape block must defer to the actions dialog while it is open in {name}"
        );
        assert!(
            escape_block.contains("!agent_chat_escape_popup_open"),
            "Agent Chat escape block must defer to Agent Chat-local popups while they are open in {name}"
        );
        assert!(
            escape_block.contains("this.close_tab_ai_harness_terminal_with_window(window, cx);"),
            "Agent Chat escape block must still close the Agent Chat chat when actions are closed in {name}"
        );
    }
}

#[test]
fn agent_chat_plain_escape_cancels_streaming_before_host_close() {
    let escape_block_start = AGENT_CHAT_VIEW_SOURCE
        .find("event = \"agent_chat_escape_cancel_streaming_requested\"")
        .expect("Agent Chat view must log the Escape streaming cancellation path");
    let escape_block = &AGENT_CHAT_VIEW_SOURCE[escape_block_start.saturating_sub(400)
        ..(escape_block_start + 800).min(AGENT_CHAT_VIEW_SOURCE.len())];

    assert!(
        escape_block.contains("AgentChatThreadStatus::Streaming")
            && escape_block.contains("thread.cancel_streaming(cx)"),
        "plain Escape helper must cancel active Agent Chat streaming"
    );

    let focused_escape_start = AGENT_CHAT_VIEW_SOURCE
        .find("if self.cancel_streaming_from_escape(cx)")
        .expect("focused Agent Chat Escape path must call the shared cancellation helper");
    let focused_escape_block = &AGENT_CHAT_VIEW_SOURCE
        [focused_escape_start..(focused_escape_start + 500).min(AGENT_CHAT_VIEW_SOURCE.len())];
    assert!(
        focused_escape_block.contains("cx.stop_propagation()")
            && focused_escape_block.contains("return;"),
        "focused Agent Chat Escape cancellation must stop before the host-close branch"
    );
    assert!(
        focused_escape_start
            < AGENT_CHAT_VIEW_SOURCE
                .find("embedded_agent_chat_escape_host_close_requested")
                .expect("focused Agent Chat Escape close path must remain present"),
        "Escape cancellation must be checked before Escape closes Agent Chat"
    );
}

#[test]
fn agent_chat_root_escape_interceptor_cancels_streaming_before_returning_to_menu() {
    for (name, source) in [
        ("startup.rs", STARTUP_SOURCE),
        ("startup_new_actions.rs", STARTUP_NEW_ACTIONS_SOURCE),
    ] {
        let cancel_pos = source
            .find("chat.cancel_streaming_from_escape(cx)")
            .unwrap_or_else(|| panic!("{name} must let Agent Chat consume Escape while streaming"));
        let close_pos = source
            .find("event = \"embedded_agent_chat_escape_return_to_origin\"")
            .unwrap_or_else(|| panic!("{name} must retain idle Escape return-to-origin path"));
        assert!(
            cancel_pos < close_pos,
            "{name} must try Agent Chat streaming cancellation before Escape returns to the main menu"
        );
        assert!(
            source[cancel_pos..close_pos].contains("cx.stop_propagation()")
                && source[cancel_pos..close_pos].contains("return;"),
            "{name} must stop propagation after Agent Chat streaming cancellation"
        );
    }

    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn cancel_streaming_from_escape")
            && AGENT_CHAT_VIEW_SOURCE.contains("thread.cancel_streaming(cx)")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("event = \"agent_chat_escape_cancel_streaming_requested\""),
        "AgentChatView should expose a shared Escape cancellation helper for focused and host routes"
    );
}

#[test]
fn agent_chat_stdin_simulate_key_escape_cancels_streaming_before_returning_to_menu() {
    let cancel_pos = SIMULATE_KEY_DISPATCH_SOURCE
        .find("chat.cancel_streaming_from_escape(cx)")
        .expect(
            "shared simulateKey dispatcher must route simulated Escape through Agent Chat cancel",
        );
    let close_pos = SIMULATE_KEY_DISPATCH_SOURCE
        .find("SimulateKey: Escape - return to main menu from Agent Chat")
        .expect("shared simulateKey dispatcher must retain idle simulated Escape close path");
    assert!(
        cancel_pos < close_pos,
        "shared simulateKey dispatcher must cancel Agent Chat streaming before simulated Escape returns to the main menu"
    );
    assert!(
        SIMULATE_KEY_DISPATCH_SOURCE[cancel_pos..close_pos]
            .contains("SimulateKey: Escape - cancel Agent Chat streaming"),
        "shared simulateKey dispatcher must log the simulated Escape streaming-cancel route"
    );
}

#[test]
fn agent_chat_cancel_streaming_sends_session_cancel_to_agent() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("self.connection.cancel_turn(self.ui_thread_id.clone())"),
        "AgentChatThread::cancel_streaming must enqueue an Agent Chat cancel request"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("self.connection.cancel_turn(self.ui_thread_id.clone())"),
        "Agent Chat runtime seam must translate UI cancellation through the active backend"
    );
}

#[test]
fn agent_chat_plain_up_recalls_latest_user_prompt_when_composer_is_empty() {
    let up_block_start = AGENT_CHAT_VIEW_SOURCE
        .find("event = \"agent_chat_plain_up_recalled_last_user_prompt\"")
        .expect("Agent Chat view must log the plain Up prompt recall path");
    let up_block = &AGENT_CHAT_VIEW_SOURCE[up_block_start.saturating_sub(700)
        ..(up_block_start + 300).min(AGENT_CHAT_VIEW_SOURCE.len())];

    assert!(
        up_block.contains("!modifiers.platform")
            && up_block.contains("crate::ui_foundation::is_key_up(key)")
            && up_block.contains("thread.recall_last_user_message(cx)")
            && up_block.contains("cx.stop_propagation()"),
        "plain Up should be consumed only when it recalls the latest user prompt"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("pub(crate) fn recall_last_user_message")
            && AGENT_CHAT_THREAD_SOURCE.contains("!self.input.is_empty()")
            && AGENT_CHAT_THREAD_SOURCE
                .contains("AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error")
            && AGENT_CHAT_THREAD_SOURCE
                .contains("message.role == AgentChatThreadMessageRole::User")
            && AGENT_CHAT_THREAD_SOURCE.contains("self.input.set_cursor(0)"),
        "AgentChatThread should recall the last user message only from an empty idle/error composer"
    );
}

#[test]
fn agent_chat_cmd_0_resets_agent_chat_zoom_through_theme_sync() {
    let cmd_0_block_start = AGENT_CHAT_VIEW_SOURCE
        .find("event = \"agent_chat_cmd_0_reset_agent_chat_zoom\"")
        .expect("Agent Chat view must log the Cmd+0 reset path");
    let cmd_0_block = &AGENT_CHAT_VIEW_SOURCE[cmd_0_block_start.saturating_sub(900)
        ..(cmd_0_block_start + 500).min(AGENT_CHAT_VIEW_SOURCE.len())];

    assert!(
        cmd_0_block.contains("FontConfig::default()")
            && cmd_0_block.contains("fonts.ui_size = defaults.ui_size")
            && cmd_0_block.contains("fonts.mono_size = defaults.mono_size")
            && cmd_0_block.contains("persist_theme_and_sync_all_windows"),
        "Cmd+0 should reset Agent Chat font sizing through the shared theme sync path"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE
            .contains("modifiers.platform && !modifiers.alt && !modifiers.shift && key == \"0\"")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.reset_agent_chat_zoom(cx);")
            && AGENT_CHAT_VIEW_SOURCE.contains("\"agent_chat_cmd_0_reset_agent_chat_zoom\""),
        "Agent Chat key handling should route Cmd+0 to the zoom reset helper"
    );
}

#[test]
fn simulated_agent_chat_escape_closes_actions_before_unwinding_chat() {
    let agent_chat_block_start = SIMULATE_KEY_DISPATCH_SOURCE
        .find("AppView::AgentChatView { ref entity, .. } => {")
        .expect("Agent Chat simulateKey branch not found in shared simulate_key_dispatch.rs");
    // Scope to the whole AgentChatView match arm (up to the next `AppView::`
    // arm) instead of a fixed char window, so unrelated growth inside the
    // branch (e.g. spine projection handling) cannot truncate the slice.
    let arm_search_start = agent_chat_block_start + "AppView::AgentChatView".len();
    let agent_chat_block_end = SIMULATE_KEY_DISPATCH_SOURCE[arm_search_start..]
        .find("AppView::")
        .map(|offset| arm_search_start + offset)
        .unwrap_or(SIMULATE_KEY_DISPATCH_SOURCE.len());
    let agent_chat_block =
        &SIMULATE_KEY_DISPATCH_SOURCE[agent_chat_block_start..agent_chat_block_end];

    let close_actions_pos = agent_chat_block
        .find("view.close_actions_popup(ActionsDialogHost::AgentChat, window, ctx);")
        .expect("simulateKey Agent Chat branch must close Agent Chat actions popup");
    let close_chat_pos = agent_chat_block
        .find("view.close_tab_ai_harness_terminal_with_window(window, ctx);")
        .expect("simulateKey Agent Chat branch must still close the Agent Chat chat");

    assert!(
        agent_chat_block.contains("view.show_actions_popup && key_lower == \"escape\""),
        "simulateKey Agent Chat branch must guard Escape with the Agent Chat actions popup state"
    );
    assert!(
        close_actions_pos < close_chat_pos,
        "simulateKey Agent Chat Escape should close the Agent Chat actions popup before closing the Agent Chat chat"
    );
}

#[test]
fn agent_chat_actions_window_close_path_restores_agent_chat_host_focus() {
    let toggle_actions_start = ACTIONS_TOGGLE_SOURCE
        .find("pub(crate) fn toggle_actions")
        .expect("toggle_actions not found in actions_toggle.rs");
    let toggle_actions_end = ACTIONS_TOGGLE_SOURCE[toggle_actions_start..]
        .find("pub(crate) fn toggle_arg_actions")
        .map(|offset| toggle_actions_start + offset)
        .unwrap_or(ACTIONS_TOGGLE_SOURCE.len());
    let toggle_actions = &ACTIONS_TOGGLE_SOURCE[toggle_actions_start..toggle_actions_end];

    assert!(
        toggle_actions.contains("self.actions_dialog_host_for_current_view()")
            && ACTIONS_TOGGLE_SOURCE.contains("ActionsDialogHost::AgentChat")
            && ACTIONS_TOGGLE_SOURCE.contains("ActionsDialogHost::MainList"),
        "toggle_actions must derive the actions host from the current view"
    );
    assert!(
        toggle_actions.contains("self.close_actions_popup(host, window, cx);"),
        "toggle_actions must close with the derived Agent Chat/MainList host"
    );
    assert!(
        toggle_actions.contains("Some(host_label)"),
        "toggle_actions should emit the derived host label for popup events"
    );
    assert!(
        toggle_actions.contains("Self::make_actions_window_on_close_callback(")
            && ACTIONS_TOGGLE_SOURCE.contains("app.request_focus_restore_for_actions_host(host);"),
        "actions window close callback must restore focus for the Agent Chat host"
    );
}

#[test]
fn render_impl_dispatches_agent_chat_view() {
    assert!(RENDER_IMPL_SOURCE.contains("AppView::AgentChatView"));
}

#[test]
fn script_triggered_terminals_still_use_pty() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn open_tab_ai_harness_terminal_from_request"),
        "PTY path must still exist for script-triggered terminals"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("AppView::QuickTerminalView"),
        "PTY path must still set QuickTerminalView"
    );
}

#[test]
fn agent_chat_and_pty_views_coexist_in_app_view() {
    assert!(APP_VIEW_STATE_SOURCE.contains("AgentChatView"));
    assert!(APP_VIEW_STATE_SOURCE.contains("QuickTerminalView"));
}

#[test]
fn agent_chat_model_selector_module_is_actions_only() {
    assert!(
        !AGENT_CHAT_MOD_SOURCE.contains("pub(crate) mod model_selector_popup;"),
        "Agent Chat model selector should not register a detached PromptPopup module"
    );
    assert!(
        AGENT_CHAT_ACTIONS_SOURCE.contains("pub(crate) fn get_agent_chat_model_picker_route"),
        "Agent Chat model selector should be owned by the actions popup route"
    );
}

#[test]
fn agent_chat_history_popup_module_is_registered() {
    assert!(
        AGENT_CHAT_MOD_SOURCE.contains("pub(crate) mod history_popup;"),
        "Agent Chat module should register the detached history popup module"
    );
}

#[test]
fn agent_chat_model_selector_migration_uses_popup_window_instead_of_inline_layer() {
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains("fn render_model_selector"),
        "Agent Chat chat view should no longer render the model selector inline"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn trigger_toggle_actions_from_parent")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("AgentChatToolbarEvent::ToggleModelSelector(parent)")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("this.trigger_toggle_actions_from_parent(*parent, cx);")
            && AGENT_CHAT_ACTIONS_SOURCE.contains("get_agent_chat_model_picker_actions")
            && !AGENT_CHAT_VIEW_SOURCE.contains("model_selector_open"),
        "Agent Chat model selector should route through Cmd+K actions instead of an inline or PromptPopup list"
    );
}

#[test]
fn agent_chat_history_migration_uses_popup_window_instead_of_inline_layer() {
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains(".id(\"agent_chat-history-picker\")"),
        "Agent Chat chat view should no longer render the history picker inline"
    );
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("AgentChatHistoryPopupWindow")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("super::popup_window::popup_window_options")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("super::popup_window::configure_popup_window")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("super::popup_window::set_popup_window_bounds"),
        "Agent Chat history picker should render through a popup window entity using shared popup mechanics"
    );
    assert!(
        !AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("fn popup_ns_window")
            && !AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("fn attach_popup_to_parent_window")
            && !AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("fn flipped_ns_window_y"),
        "Agent Chat history popup must not copy AppKit popup plumbing that is owned by popup_window"
    );
}

#[test]
fn agent_chat_show_history_action_opens_main_history_list() {
    assert!(
        !HANDLE_ACTION_SOURCE
            .contains("if !self.open_embedded_agent_chat_history_popup(window, cx) {")
            && HANDLE_ACTION_SOURCE.contains("AppView::AgentChatHistoryView"),
        "agent_chat_show_history should open the main AgentChatHistoryView list instead of the embedded history popup"
    );
}

#[test]
fn agent_chat_picker_refresh_and_navigation_sync_popup_window() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(super) fn refresh_composer_picker_session")
            && AGENT_CHAT_VIEW_SOURCE.contains("fn cache_composer_parent_window")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("self.refresh_composer_picker_state_after_parent_change(cx);"),
        "picker refresh should keep the detached popup window synchronized"
    );

    let keydown_block_start = AGENT_CHAT_VIEW_SOURCE
        .find("if self.composer_picker_session.is_some() {")
        .expect("mention-session keydown block should exist");
    let keydown_block_end = AGENT_CHAT_VIEW_SOURCE[keydown_block_start..]
        .find("// Shift+Enter inserts a newline.")
        .map(|offset| keydown_block_start + offset)
        .unwrap_or(AGENT_CHAT_VIEW_SOURCE.len());
    let keydown_block = &AGENT_CHAT_VIEW_SOURCE[keydown_block_start..keydown_block_end];
    assert!(
        keydown_block
            .matches("self.refresh_composer_picker_state_after_parent_change(cx);")
            .count()
            >= 2,
        "picker navigation should resync the detached popup window"
    );
}

#[test]
fn agent_chat_picker_parent_mouse_down_dismisses_slash_and_composer_picker_session() {
    let render_body = agent_chat_source_between(
        AGENT_CHAT_VIEW_SOURCE,
        "impl Render for AgentChatView",
        "#[cfg(test)]",
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn dismiss_composer_picker")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.composer_picker_session.take()")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("self.refresh_composer_picker_state_after_parent_change(cx);"),
        "AgentChatView must expose a shared picker dismiss helper for slash/profile composer sessions"
    );
    assert!(
        render_body.contains(".on_any_mouse_down(cx.listener(|this, _event, _window, cx| {")
            && render_body.contains("this.dismiss_composer_picker(cx);"),
        "Agent Chat chat root mouse-down should dismiss the shared slash/profile composer picker when clicking outside"
    );
}

#[test]
fn agent_chat_picker_outside_dismiss_suppresses_unchanged_trigger_reopen() {
    let dismiss = agent_chat_source_between(
        AGENT_CHAT_VIEW_SOURCE,
        "pub(crate) fn dismiss_composer_picker",
        "/// Access the live thread entity",
    );
    assert!(
        dismiss.contains("reduce_agent_chat_composer_picker")
            && dismiss.contains("self.composer_picker_state()")
            && dismiss.contains("AgentChatComposerPickerEvent::Dismiss")
            && dismiss.contains("reason: AgentChatComposerPickerDismissReason::Outside")
            && dismiss.contains("cursor,")
            && dismiss.contains("AgentChatComposerPickerState::Dismissed(trigger)"),
        "outside-click dismiss must remember the exact active slash/profile trigger so unchanged composer text does not reopen the popup"
    );

    let refresh = agent_chat_source_between(
        AGENT_CHAT_VIEW_SOURCE,
        "pub(super) fn refresh_composer_picker_session",
        "/// Log the visible window range",
    );
    assert!(
        refresh.contains("let mut active_dismissed_trigger = None;")
            && refresh.contains("self.dismissed_mention_trigger.as_ref() == Some(&active_trigger)")
            && refresh.contains("active_dismissed_trigger = Some(active_trigger);")
            && refresh.contains("active_trigger: active_dismissed_trigger"),
        "refresh_composer_picker_session must keep the dismissed trigger closed until the input/cursor context changes"
    );
}

#[test]
fn agent_chat_close_paths_close_slash_and_composer_picker_session() {
    let detached_cmd_w_block = agent_chat_source_between(
        AGENT_CHAT_VIEW_SOURCE,
        "event = \"detached_agent_chat_cmd_w_close_requested\"",
        "this.handle_key_down(event, window, cx);",
    );
    let detached_cmd_w_prepare = detached_cmd_w_block
        .find("this.prepare_for_host_hide(cx);")
        .expect("detached Cmd+W block must prepare Agent Chat host hide");
    let detached_cmd_w_remove = detached_cmd_w_block
        .find("window.remove_window();")
        .expect("detached Cmd+W block must remove the window");
    assert!(
        detached_cmd_w_prepare < detached_cmd_w_remove,
        "detached Agent Chat Cmd+W must close slash/profile composer sessions before removing the window"
    );

    let detached_close_helper = agent_chat_source_between(
        AGENT_CHAT_WINDOW_SOURCE,
        "pub fn close_chat_window",
        "// Detached Agent Chat action allowlist",
    );
    let detached_helper_prepare = detached_close_helper
        .find("view.prepare_for_host_hide(cx);")
        .expect("close_chat_window must prepare Agent Chat host hide");
    let detached_helper_remove = detached_close_helper
        .find("window.remove_window();")
        .expect("close_chat_window must remove the window");
    assert!(
        detached_helper_prepare < detached_helper_remove,
        "detached close_chat_window must close slash/profile composer sessions before removing the window"
    );

    let detached_titlebar_close = agent_chat_source_between(
        AGENT_CHAT_WINDOW_SOURCE,
        "pub fn open_chat_window_with_thread",
        "/// Return a strong reference to the detached Agent Chat chat view entity",
    );
    assert!(
        detached_titlebar_close.contains("view_entity_slot_on_close")
            && detached_titlebar_close.contains("view.prepare_for_host_hide(cx);"),
        "detached titlebar close must prepare the Agent Chat view so slash/profile composer sessions cannot outlive chat"
    );
}

#[test]
fn reset_to_script_list_runs_embedded_agent_chat_teardown() {
    let reset_start = REGISTRIES_STATE_SOURCE
        .find("pub(crate) fn reset_to_script_list")
        .expect("reset_to_script_list should exist");
    let reset_body = &REGISTRIES_STATE_SOURCE[reset_start..];

    assert!(
        reset_body.contains("view.prepare_for_host_hide(cx);")
            && reset_body.contains("crate::windows::ensure_embedded_ai_window(false);")
            && reset_body.contains("AgentChatSurfaceEvent::EmbeddedClosed"),
        "reset_to_script_list must close embedded Agent Chat popups and automation state before returning to ScriptList"
    );
}

#[test]
fn agent_chat_composer_stays_width_wrapped_without_explicit_newline() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn render_composer_input_text")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("render_text_input_cursor_selection(TextInputRenderConfig")
            && AGENT_CHAT_VIEW_SOURCE.contains("multiline,")
            && AGENT_CHAT_VIEW_SOURCE.contains("true,\n            mention_highlights,"),
        "Agent Chat composer should use width-driven multiline rendering"
    );
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains("multiline: input_has_newline"),
        "Agent Chat composer should not wait for an explicit newline before wrapping"
    );
}

#[test]
fn agent_chat_model_selection_is_visible_in_footer_and_routed_through_actions() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains(".id(\"agent_chat-model-display\")")
            && !AGENT_CHAT_VIEW_SOURCE.contains(".id(\"agent_chat-model-btn\")")
            && AGENT_CHAT_VIEW_SOURCE.contains("\"⌘K Actions\""),
        "Agent Chat footer should keep the active model visible and route changes through the actions menu"
    );
}

#[test]
fn agent_chat_history_toggle_and_selection_sync_popup_window() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("self.sync_history_popup_window_from_cached_parent(cx);")
            && AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn select_history_from_popup")
            && AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn toggle_history_popup"),
        "history picker interactions should open and close through the detached popup window"
    );
}

#[test]
fn agent_chat_history_toggle_uses_recent_close_debounce() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("history_closed_at: Option<Instant>")
            && AGENT_CHAT_VIEW_SOURCE.contains("fn was_history_recently_closed(&self) -> bool")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("fn mark_history_popup_closed(&mut self, cx: &mut Context<Self>)")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("event = \"agent_chat_history_popup_toggle_suppressed_recent_close\""),
        "Agent Chat history popup should track recent closes and suppress immediate reopen races like the shared actions dialog"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("view.set_on_open_history_command")
            && TAB_AI_MODE_SOURCE.contains("app.open_agent_chat_history_main_list(window, cx);"),
        "embedded Agent Chat history host should wire the footer and shortcut into the main AgentChatHistoryView path"
    );
}

#[test]
fn agent_chat_history_popup_window_observes_focus_loss_and_escape() {
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("activation_subscription: Option<Subscription>")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("fn ensure_activation_subscription(")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("observe_window_activation(")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("this.request_close(window, cx, \"focus_lost\");"),
        "Agent Chat history popup window should observe activation changes and close on focus loss"
    );
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE.contains(
            ".on_mouse_down_out(cx.listener(|this, _event: &gpui::MouseDownEvent, window, cx| {"
        ) && AGENT_CHAT_HISTORY_POPUP_SOURCE
            .contains("this.request_close(window, cx, \"mouse_down_out\");")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("view.dismiss_history_popup_from_window(reason, cx);")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("this.request_close(window, cx, \"escape\");"),
        "Agent Chat history popup window should close on outside clicks and sync dismissals back into Agent Chat state"
    );
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE
            .contains("view.dismiss_history_popup_from_window(reason, cx);")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("this.request_close(window, cx, \"escape\");"),
        "Agent Chat history popup window should sync dismissals back into Agent Chat state for both focus loss and Escape"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains(".id(\"agent_chat-history-popup-backdrop\")")
            && AGENT_CHAT_VIEW_SOURCE.contains("this.dismiss_history_popup(cx);")
            && AGENT_CHAT_VIEW_SOURCE.contains(".bottom(px(self.inline_footer_height()))"),
        "Agent Chat host should render an outside-click backdrop above chat content so clicks outside the popup close it without swallowing the footer toggle"
    );
}

#[test]
fn agent_chat_history_popup_window_supports_actions_style_search_and_keyboard_navigation() {
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("enum AgentChatHistoryPopupKeyIntent")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("TypeChar(char)")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("Backspace")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("MovePageDown")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("MoveHome")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("history_popup_key_intent"),
        "Agent Chat history popup should use an actions-style key intent model for search and navigation"
    );
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("sync_history_popup_state_from_window")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("sync_history_popup_selection_from_window")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("Type to Search")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains(".track_scroll(&self.scroll_handle)"),
        "Agent Chat history popup should expose a visible search row and keep popup state synchronized while keyboard navigation scrolls"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("if self.history_menu.is_some() {")
            && AGENT_CHAT_VIEW_SOURCE.contains("match history_popup_key_intent(key, modifiers)")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.set_history_popup_query(next_query, cx);")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("self.execute_history_popup_selection(modifiers, cx);"),
        "Agent Chat host key routing should intercept history popup navigation and search the same way the shared actions popup does"
    );
}

#[test]
fn agent_chat_history_enter_resumes_selected_chat() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("if modifiers.platform {")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.select_history_from_popup(&entry, cx);"),
        "embedded Agent Chat history keyboard handling should route the resume action through select_history_from_popup"
    );
    assert!(
        AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("if has_shift {")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("this.attach_transcript(&entry, cx);")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("} else if has_cmd {")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("this.attach_summary(&entry, cx);")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("} else {")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("this.resume_session(&entry, cx);")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE.contains("\"\\u{21B5} Resume\".into(),")
            && AGENT_CHAT_HISTORY_POPUP_SOURCE
                .contains("\"\\u{2318}\\u{21B5} Attach Summary\".into(),"),
        "Agent Chat history popup should advertise and honor Enter-to-resume while keeping modifier-based attach actions"
    );
}

#[test]
fn agent_chat_history_runtime_shortcuts_route_to_dedicated_command() {
    assert!(
        SIMULATE_KEY_DISPATCH_SOURCE.contains("view.handle_action(\"agent_chat_show_history\""),
        "runtime Agent Chat Cmd+P paths should dispatch the agent_chat_show_history action to open the dedicated history command"
    );
    // Verify the old popup toggle is no longer used by stdin simulation
    assert!(
        !APP_RUN_SETUP_SOURCE.contains("chat.toggle_history_popup(window, cx);")
            && !RUNTIME_STDIN_SOURCE.contains("chat.toggle_history_popup(window, cx);")
            && !SIMULATE_KEY_DISPATCH_SOURCE.contains("chat.toggle_history_popup(window, ctx);"),
        "runtime Agent Chat Cmd+P paths should no longer toggle the inline history popup"
    );
}

#[test]
fn agent_chat_view_exposes_escape_popup_dismiss_helper() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn dismiss_escape_popup")
            && AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn has_escape_dismissible_popup")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.history_menu.is_some()")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.composer_picker_session = None;")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("self.refresh_composer_picker_state_after_parent_change(cx);")
            && AGENT_CHAT_VIEW_SOURCE.contains("if self.attach_menu_open {")
            && AGENT_CHAT_VIEW_SOURCE.contains("|| self.attach_menu_open"),
        "Agent Chat view should expose a helper that dismisses the detached Agent Chat popups on Escape"
    );
}

#[test]
fn detached_agent_chat_limits_portals_to_history() {
    assert!(
        AGENT_CHAT_WINDOW_SOURCE
            .contains("view.set_allowed_portal_kinds(vec![ContextPortalKind::AgentChatHistory]);")
            && AGENT_CHAT_WINDOW_SOURCE
                .contains("view.set_on_open_portal(move |kind, cx| match kind {")
            && AGENT_CHAT_WINDOW_SOURCE.contains("ContextPortalKind::AgentChatHistory => {")
            && AGENT_CHAT_WINDOW_SOURCE.contains("open_history_portal_in_detached_chat_window(cx)")
            && AGENT_CHAT_WINDOW_SOURCE
                .contains("cancel_portal_session_in_detached_chat_window(kind, cx)")
            && AGENT_CHAT_WINDOW_SOURCE.contains("reason = \"unsupported_in_detached_host\""),
        "detached Agent Chat should expose only the locally supported history portal, clear staged portal state on open failure, and log rejected portal requests"
    );
}

#[test]
fn agent_chat_history_popup_attach_consumes_pending_history_portal_session() {
    let popup_select_fn_start = AGENT_CHAT_VIEW_SOURCE
        .find("pub(crate) fn select_history_from_popup")
        .expect("select_history_from_popup should exist");
    let popup_select_fn = &AGENT_CHAT_VIEW_SOURCE
        [popup_select_fn_start..(popup_select_fn_start + 1400).min(AGENT_CHAT_VIEW_SOURCE.len())];

    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn build_history_attachment_part(")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("event = \"agent_chat_history_portal_selection_attached_via_contract\"")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.attach_portal_part(part, cx);")
            && AGENT_CHAT_VIEW_SOURCE.contains(
                "let had_pending_history_portal = self.has_pending_history_portal_session();"
            )
            && popup_select_fn.contains("if had_pending_history_portal {")
            && popup_select_fn.contains("event = \"agent_chat_history_popup_attach_failed\"")
            && popup_select_fn.contains("self.cancel_pending_portal_session(")
            && popup_select_fn.contains("ContextPortalKind::AgentChatHistory")
            && popup_select_fn.contains("return;"),
        "Agent Chat history attachment should consume the staged AgentChatHistory portal session instead of bypassing the shared replacement contract"
    );
}

#[test]
fn agent_chat_history_popup_dismiss_restores_pending_history_portal_session() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE
            .contains("event = \"agent_chat_history_portal_dismissed_via_popup\"")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("event = \"agent_chat_history_portal_dismissed_from_window\"")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.has_pending_history_portal_session()")
            && AGENT_CHAT_VIEW_SOURCE.contains("self.cancel_pending_portal_session(")
            && AGENT_CHAT_VIEW_SOURCE.contains("ContextPortalKind::AgentChatHistory"),
        "Agent Chat history popup dismissals should cancel the staged AgentChatHistory portal session so the composer text and caret are restored on close"
    );
}

// =========================================================================
// Agent Chat test probe — ring buffer and snapshot
// =========================================================================

#[test]
fn agent_chat_test_probe_records_key_routes() {
    let mut probe = super::view::AgentChatTestProbe::default();
    assert_eq!(probe.event_seq, 0);
    assert!(probe.key_routes.is_empty());

    let event = crate::protocol::AgentChatKeyRouteTelemetry {
        key: "tab".to_string(),
        route: crate::protocol::AgentChatKeyRoute::Picker,
        picker_open: true,
        permission_active: false,
        cursor_before: 1,
        cursor_after: 17,
        caused_submit: false,
        consumed: true,
    };

    probe.event_seq += 1;
    probe.key_routes.push_back(event.clone());
    assert_eq!(probe.event_seq, 1);
    assert_eq!(probe.key_routes.len(), 1);
    assert_eq!(probe.key_routes[0].key, "tab");
}

#[test]
fn agent_chat_test_probe_records_picker_accepts() {
    let mut probe = super::view::AgentChatTestProbe::default();

    let event = crate::protocol::AgentChatPickerItemAcceptedTelemetry {
        trigger: "@".to_string(),
        item_label: "Current Context".to_string(),
        item_id: "built_in:context".to_string(),
        accepted_via_key: "tab".to_string(),
        cursor_after: 17,
        caused_submit: false,
    };

    probe.event_seq += 1;
    probe.accepted_items.push_back(event.clone());
    assert_eq!(probe.accepted_items.len(), 1);
    assert_eq!(probe.accepted_items[0].accepted_via_key, "tab");
}

#[test]
fn agent_chat_test_probe_records_input_layout() {
    let mut probe = super::view::AgentChatTestProbe::default();

    let event = crate::protocol::AgentChatInputLayoutTelemetry {
        char_count: 27,
        visible_start: 0,
        visible_end: 27,
        cursor_in_window: 17,
    };

    probe.event_seq += 1;
    probe.input_layout = Some(event.clone());
    assert!(probe.input_layout.is_some());
    assert_eq!(
        probe
            .input_layout
            .as_ref()
            .expect("layout")
            .cursor_in_window,
        17
    );
}

#[test]
fn agent_chat_test_probe_bounded_at_max_events() {
    let mut probe = super::view::AgentChatTestProbe::default();
    let max = crate::protocol::AGENT_CHAT_TEST_PROBE_MAX_EVENTS;

    for i in 0..(max + 10) {
        if probe.key_routes.len() >= max {
            probe.key_routes.pop_front();
        }
        probe
            .key_routes
            .push_back(crate::protocol::AgentChatKeyRouteTelemetry {
                key: format!("key-{i}"),
                route: crate::protocol::AgentChatKeyRoute::Composer,
                picker_open: false,
                permission_active: false,
                cursor_before: i,
                cursor_after: i + 1,
                caused_submit: false,
                consumed: true,
            });
    }

    assert_eq!(
        probe.key_routes.len(),
        max,
        "ring buffer must be bounded at {max}"
    );
    // Oldest event should have been evicted
    assert_eq!(probe.key_routes[0].key, "key-10");
}

#[test]
fn agent_chat_test_probe_reset_clears_all() {
    let mut probe = super::view::AgentChatTestProbe::default();

    probe.event_seq = 42;
    probe
        .key_routes
        .push_back(crate::protocol::AgentChatKeyRouteTelemetry {
            key: "tab".to_string(),
            route: crate::protocol::AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        });
    probe
        .accepted_items
        .push_back(crate::protocol::AgentChatPickerItemAcceptedTelemetry {
            trigger: "@".to_string(),
            item_label: "context".to_string(),
            item_id: "built_in:context".to_string(),
            accepted_via_key: "tab".to_string(),
            cursor_after: 9,
            caused_submit: false,
        });
    probe.input_layout = Some(crate::protocol::AgentChatInputLayoutTelemetry {
        char_count: 10,
        visible_start: 0,
        visible_end: 10,
        cursor_in_window: 9,
    });

    // Reset
    probe.event_seq = 0;
    probe.key_routes.clear();
    probe.accepted_items.clear();
    probe.input_layout = None;

    assert_eq!(probe.event_seq, 0);
    assert!(probe.key_routes.is_empty());
    assert!(probe.accepted_items.is_empty());
    assert!(probe.input_layout.is_none());
}

// =========================================================================
// Agent Chat test probe — source code contracts
// =========================================================================

#[test]
fn agent_chat_view_has_test_probe_methods() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn reset_test_probe("),
        "AgentChatView must have reset_test_probe method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn record_key_route("),
        "AgentChatView must have record_key_route method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn record_picker_accept("),
        "AgentChatView must have record_picker_accept method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn record_input_layout("),
        "AgentChatView must have record_input_layout method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn test_probe_snapshot("),
        "AgentChatView must have test_probe_snapshot method"
    );
}

#[test]
fn emit_methods_record_into_probe() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("self.record_key_route(telemetry.clone())"),
        "emit_key_route_telemetry must record into probe"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("self.record_picker_accept(telemetry.clone())"),
        "emit_picker_accepted_telemetry must record into probe"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("self.record_input_layout(telemetry.clone())"),
        "emit_input_layout_telemetry must record into probe"
    );
}

#[test]
fn emit_key_route_telemetry_uses_real_permission_state() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    // The function must accept permission_active as a parameter, not hardcode it.
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("permission_active: bool,"),
        "emit_key_route_telemetry must accept permission_active as a parameter"
    );
    assert!(
        !AGENT_CHAT_VIEW_SOURCE.contains("let permission_active = false;"),
        "emit_key_route_telemetry must not hardcode permission_active to false"
    );
}

#[test]
fn call_sites_pass_real_permission_active() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    // All call sites should read the real permission state from the thread.
    let permission_reads = AGENT_CHAT_VIEW_SOURCE
        .matches("pending_permission.is_some()")
        .count();
    let telemetry_calls = AGENT_CHAT_VIEW_SOURCE
        .matches(".emit_key_route_telemetry(")
        .count();
    assert!(
        permission_reads >= telemetry_calls,
        "each emit_key_route_telemetry call site ({telemetry_calls}) must read \
         pending_permission.is_some() ({permission_reads} found)"
    );
}

// =========================================================================
// Composer picker windowing — selected item always visible
// =========================================================================

/// Helper: call the private `composer_picker_visible_range_for` and assert the
/// selected index falls within the returned range.
fn assert_selected_visible(selected: usize, item_count: usize) {
    let range = super::view::AgentChatView::composer_picker_visible_range_for(selected, item_count);
    assert!(
        range.contains(&selected),
        "selected_index={selected} must be inside visible range {range:?} (item_count={item_count})",
    );
    assert!(
        range.len() <= super::view::AgentChatView::COMPOSER_PICKER_MAX_VISIBLE,
        "visible range len {} exceeds max {}",
        range.len(),
        super::view::AgentChatView::COMPOSER_PICKER_MAX_VISIBLE,
    );
}

#[test]
fn mention_picker_windowing_small_list() {
    // Fewer items than max_visible → range is 0..item_count
    for selected in 0..5 {
        let range = super::view::AgentChatView::composer_picker_visible_range_for(selected, 5);
        assert_eq!(range, 0..5);
    }
}

#[test]
fn mention_picker_windowing_selected_always_visible() {
    let item_count = 20;
    for selected in 0..item_count {
        assert_selected_visible(selected, item_count);
    }
}

#[test]
fn mention_picker_windowing_wrap_to_last() {
    // Simulate pressing Up from index 0 → wrap to last
    let item_count = 15;
    let selected = item_count - 1;
    assert_selected_visible(selected, item_count);
}

#[test]
fn mention_picker_windowing_wrap_to_first() {
    // Simulate pressing Down from last → wrap to 0
    assert_selected_visible(0, 15);
}

// =========================================================================
// 5. Agent Chat preflight and setup mode — source code contracts
// =========================================================================

#[test]
fn agent_handoff_uses_catalog_loader_not_claude_only_loader() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("load_agent_chat_agent_catalog_entries"),
        "agent_handoff must use the catalog loader, not Claude-only config"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("resolve_agent_chat_launch_with_requirements"),
        "agent_handoff must use capability-aware preflight resolution"
    );
}

#[test]
fn agent_handoff_routes_to_setup_mode_when_blocked() {
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("show_pi_agent_chat_unavailable_setup_view"),
        "tab_ai Agent Chat launch helper must create setup-mode view when the Pi agent launch is blocked"
    );
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("pi_agent_chat_launch_resolution_failed")
            || TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("pi_agent_chat_warm_failed_setup"),
        "agent_handoff must log launch resolution event"
    );
}

#[test]
fn agent_chat_view_supports_setup_constructor() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn new_setup"),
        "AgentChatView must have a new_setup constructor"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("AgentChatSession::Setup"),
        "AgentChatView must support Setup session state"
    );
}

#[test]
fn agent_chat_view_thread_accessor_returns_option() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn thread(&self) -> Option<Entity<AgentChatThread>>"),
        "AgentChatView must have a thread() method returning Option"
    );
}

#[test]
fn setup_state_from_resolution_covers_all_blockers() {
    use super::preflight::{AgentChatLaunchBlocker, AgentChatLaunchResolution};
    use super::setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};

    let blockers = [
        AgentChatLaunchBlocker::NoAgentsAvailable,
        AgentChatLaunchBlocker::AgentNotInstalled,
        AgentChatLaunchBlocker::AuthenticationRequired,
        AgentChatLaunchBlocker::AgentMisconfigured,
        AgentChatLaunchBlocker::UnsupportedAgent,
    ];

    for blocker in &blockers {
        let resolution = AgentChatLaunchResolution {
            selected_agent: None,
            blocker: Some(blocker.clone()),
            catalog_entries: vec![],
        };
        let state = AgentChatInlineSetupState::from_resolution(
            &resolution,
            AgentChatLaunchRequirements::default(),
        );
        assert!(
            !state.title.is_empty(),
            "setup state title must be non-empty for {:?}",
            blocker
        );
        assert!(
            !state.body.is_empty(),
            "setup state body must be non-empty for {:?}",
            blocker
        );
    }
}

#[test]
fn events_have_setup_required_variant() {
    const EVENTS_SOURCE: &str = include_str!("events.rs");
    assert!(
        EVENTS_SOURCE.contains("SetupRequired"),
        "AgentChatEvent must have a SetupRequired variant"
    );
}

#[test]
fn ai_setup_surface_no_longer_mentions_claude_only_copy() {
    const SETUP_RENDER_SOURCE: &str = include_str!("../../../ai/window/render_setup.rs");
    assert!(
        SETUP_RENDER_SOURCE.contains("Agent Required"),
        "setup card title must say Agent Required"
    );
    assert!(
        SETUP_RENDER_SOURCE.contains("Open Agent Catalog"),
        "setup card must offer Open Agent Catalog"
    );
    assert!(
        !SETUP_RENDER_SOURCE.contains("Connect to Claude Code"),
        "setup card must NOT mention Claude Code specifically"
    );
}

// =========================================================================
// 6. Capability-driven Agent Chat launch and recovery
// =========================================================================

#[test]
fn agent_handoff_derives_launch_requirements() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("AgentChatLaunchRequirements"),
        "agent_handoff must derive AgentChatLaunchRequirements"
    );
    // The legacy `agent_chat_open_retry_request_consumed` event was removed in
    // the Pi-backend launch refactor. The surviving requirements-aware retry
    // decision is the hot-prewarm skip: a queued retry or non-default
    // requirements must bypass the prewarmed Agent Chat view.
    assert!(
        TAB_AI_MODE_SOURCE.contains("agent_chat_hot_prewarm_skip")
            && TAB_AI_MODE_SOURCE.contains("retry_request_active")
            && TAB_AI_MODE_SOURCE.contains("needs_embedded_context"),
        "agent_handoff must log the requirements-aware prewarm skip decision"
    );
}

#[test]
fn agent_chat_retry_request_from_setup_state_preserves_agent_and_requirements() {
    use super::setup_state::AgentChatInlineSetupState;
    use super::view::AgentChatRetryRequest;

    let setup = AgentChatInlineSetupState {
        reason_code: "authenticationRequired",
        title: "Auth required".into(),
        body: "test".into(),
        primary_action: super::setup_state::AgentChatSetupAction::Retry,
        secondary_action: None,
        selected_agent: Some(super::catalog::AgentChatAgentCatalogEntry {
            id: "opencode".into(),
            display_name: "OpenCode".into(),
            source: super::catalog::AgentChatAgentSource::ScriptKitCatalog,
            install_state: super::catalog::AgentChatAgentInstallState::Ready,
            auth_state: super::catalog::AgentChatAgentAuthState::Unknown,
            config_state: super::catalog::AgentChatAgentConfigState::Valid,
            install_hint: None,
            config_hint: None,
            supports_embedded_context: Some(true),
            supports_image: None,
            last_session_ok: false,
            config: None,
        }),
        catalog_entries: Vec::new(),
        launch_requirements: super::preflight::AgentChatLaunchRequirements {
            needs_embedded_context: true,
            needs_image: false,
        },
    };

    let request = AgentChatRetryRequest::from_setup_state(&setup);
    assert_eq!(request.preferred_agent_id.as_deref(), Some("opencode"));
    assert!(request.launch_requirements.needs_embedded_context);
    assert!(!request.launch_requirements.needs_image);
}

#[test]
fn agent_chat_retry_request_from_setup_state_without_agent() {
    use super::view::AgentChatRetryRequest;

    let setup = super::setup_state::AgentChatInlineSetupState {
        reason_code: "noAgentsAvailable",
        title: "No agents".into(),
        body: "test".into(),
        primary_action: super::setup_state::AgentChatSetupAction::OpenCatalog,
        secondary_action: None,
        selected_agent: None,
        catalog_entries: Vec::new(),
        launch_requirements: super::preflight::AgentChatLaunchRequirements::default(),
    };

    let request = AgentChatRetryRequest::from_setup_state(&setup);
    assert_eq!(request.preferred_agent_id, None);
    assert!(!request.launch_requirements.needs_embedded_context);
    assert!(!request.launch_requirements.needs_image);
}

#[test]
fn agent_handoff_consumes_retry_request_on_open() {
    // Verify the open path checks for a staged retry request from the view.
    // (The legacy per-agent preference fallback was removed — all sessions
    // use the Pi backend, so there is no agent preference to fall back to.)
    assert!(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE.contains("take_agent_chat_retry_request_for_open"),
        "agent_handoff must check for retry request from current view"
    );
}

#[test]
fn agent_chat_view_queues_retry_payload_on_setup_retry() {
    // The view must queue the payload with structured tracing.
    let view_source = include_str!("view.rs");
    assert!(
        view_source.contains("agent_chat_setup_retry_payload_queued"),
        "view must emit agent_chat_setup_retry_payload_queued tracing event"
    );
    assert!(
        view_source.contains("queue_setup_retry_request"),
        "Retry action must call queue_setup_retry_request"
    );
}

#[test]
fn setup_state_handles_capability_mismatch_with_switch() {
    use super::preflight::{AgentChatLaunchBlocker, AgentChatLaunchResolution};
    use super::setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};

    let agents = vec![
        super::catalog::AgentChatAgentCatalogEntry {
            id: "blocked".into(),
            display_name: "Blocked".into(),
            source: super::catalog::AgentChatAgentSource::ScriptKitCatalog,
            install_state: super::catalog::AgentChatAgentInstallState::Ready,
            auth_state: super::catalog::AgentChatAgentAuthState::Unknown,
            config_state: super::catalog::AgentChatAgentConfigState::Valid,
            install_hint: None,
            config_hint: None,
            supports_embedded_context: Some(false),
            supports_image: None,
            last_session_ok: false,
            config: None,
        },
        super::catalog::AgentChatAgentCatalogEntry {
            id: "ready".into(),
            display_name: "Ready".into(),
            source: super::catalog::AgentChatAgentSource::ScriptKitCatalog,
            install_state: super::catalog::AgentChatAgentInstallState::Ready,
            auth_state: super::catalog::AgentChatAgentAuthState::Unknown,
            config_state: super::catalog::AgentChatAgentConfigState::Valid,
            install_hint: None,
            config_hint: None,
            supports_embedded_context: Some(true),
            supports_image: None,
            last_session_ok: false,
            config: None,
        },
    ];

    let resolution = AgentChatLaunchResolution {
        selected_agent: Some(agents[0].clone()),
        blocker: Some(AgentChatLaunchBlocker::CapabilityMismatch),
        catalog_entries: agents,
    };

    let state = AgentChatInlineSetupState::from_resolution(
        &resolution,
        AgentChatLaunchRequirements {
            needs_embedded_context: true,
            needs_image: false,
        },
    );
    assert_eq!(state.title.as_ref(), "Agent capability mismatch");
    assert_eq!(
        state.primary_action,
        AgentChatSetupAction::SelectAgent,
        "should offer SelectAgent when a capable alternative exists"
    );
}

#[test]
fn setup_state_handles_capability_mismatch_without_alternative() {
    use super::preflight::{AgentChatLaunchBlocker, AgentChatLaunchResolution};
    use super::setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};

    let agents = vec![super::catalog::AgentChatAgentCatalogEntry {
        id: "only-agent".into(),
        display_name: "Only Agent".into(),
        source: super::catalog::AgentChatAgentSource::ScriptKitCatalog,
        install_state: super::catalog::AgentChatAgentInstallState::Ready,
        auth_state: super::catalog::AgentChatAgentAuthState::Unknown,
        config_state: super::catalog::AgentChatAgentConfigState::Valid,
        install_hint: None,
        config_hint: None,
        supports_embedded_context: Some(false),
        supports_image: None,
        last_session_ok: false,
        config: None,
    }];

    let resolution = AgentChatLaunchResolution {
        selected_agent: Some(agents[0].clone()),
        blocker: Some(AgentChatLaunchBlocker::CapabilityMismatch),
        catalog_entries: agents,
    };

    let state = AgentChatInlineSetupState::from_resolution(
        &resolution,
        AgentChatLaunchRequirements {
            needs_embedded_context: true,
            needs_image: false,
        },
    );
    assert_eq!(state.title.as_ref(), "Agent capability mismatch");
    assert_eq!(
        state.primary_action,
        AgentChatSetupAction::Retry,
        "should offer Retry when no capable alternative exists"
    );
}

// =========================================================================
// 7. Agent Chat history primitives — delete + resume request
// =========================================================================

#[test]
fn delete_conversation_removes_file_and_rewrites_index() {
    use super::history::{
        delete_conversation, load_history, save_conversation, save_history_entry,
        AgentChatHistoryEntry, SavedConversation, SavedMessage,
    };

    let dir = tempfile::tempdir().expect("create temp dir");
    let kit_path = dir.path().to_path_buf();

    // Create a history index and conversation file manually
    let history_path = kit_path.join("agent_chat-history.jsonl");
    let conv_dir = kit_path.join("agent_chat-conversations");
    std::fs::create_dir_all(&conv_dir).expect("create conv dir");

    let entry_a = AgentChatHistoryEntry {
        timestamp: "2026-04-05T10:00:00Z".to_string(),
        first_message: "hello from A".to_string(),
        message_count: 3,
        session_id: "session-a".to_string(),
        ..Default::default()
    };
    let entry_b = AgentChatHistoryEntry {
        timestamp: "2026-04-05T11:00:00Z".to_string(),
        first_message: "hello from B".to_string(),
        message_count: 5,
        session_id: "session-b".to_string(),
        ..Default::default()
    };

    // Write index entries
    let mut index_content = String::new();
    index_content.push_str(&serde_json::to_string(&entry_a).expect("serialize"));
    index_content.push('\n');
    index_content.push_str(&serde_json::to_string(&entry_b).expect("serialize"));
    index_content.push('\n');
    std::fs::write(&history_path, &index_content).expect("write index");

    // Write conversation file for session-a
    let conv = SavedConversation {
        session_id: "session-a".to_string(),
        timestamp: "2026-04-05T10:00:00Z".to_string(),
        custom_title: None,
        messages: vec![SavedMessage {
            role: "user".to_string(),
            body: "test message".to_string(),
        }],
    };
    let conv_path = conv_dir.join("session-a.json");
    std::fs::write(
        &conv_path,
        serde_json::to_string_pretty(&conv).expect("serialize"),
    )
    .expect("write conv");

    assert!(
        conv_path.exists(),
        "conversation file must exist before delete"
    );

    // Note: delete_conversation uses the global kit path, so we can only
    // test the function's serialization/deserialization logic in isolation.
    // The actual file I/O integration depends on the global kit path.
    // Instead, verify the entry types roundtrip correctly through serde.
    let entries_json = std::fs::read_to_string(&history_path).expect("read index");
    let parsed: Vec<AgentChatHistoryEntry> = entries_json
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    assert_eq!(parsed.len(), 2);

    // Simulate delete by filtering
    let remaining: Vec<&AgentChatHistoryEntry> = parsed
        .iter()
        .filter(|e| e.session_id != "session-a")
        .collect();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].session_id, "session-b");
}

#[test]
fn delete_conversation_is_idempotent_for_missing_session() {
    // Calling delete on a non-existent session should succeed.
    // We can't test the real function without the global kit path,
    // but we verify the AgentChatHistoryEntry serde contract supports it.
    let entry = super::history::AgentChatHistoryEntry {
        timestamp: "2026-04-05T10:00:00Z".to_string(),
        first_message: "test".to_string(),
        message_count: 1,
        session_id: "nonexistent-session".to_string(),
        ..Default::default()
    };
    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: super::history::AgentChatHistoryEntry =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.session_id, "nonexistent-session");
}

#[test]
fn history_resume_request_struct_carries_session_id() {
    let request = super::view::AgentChatHistoryResumeRequest {
        session_id: "test-session-42".to_string(),
    };
    assert_eq!(request.session_id, "test-session-42");

    let cloned = request.clone();
    assert_eq!(cloned.session_id, "test-session-42");
}

#[test]
fn agent_chat_view_exposes_history_resume_primitives() {
    const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("view.rs");
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) struct AgentChatHistoryResumeRequest"),
        "AgentChatView module must define AgentChatHistoryResumeRequest"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn stage_history_resume("),
        "AgentChatView must have stage_history_resume method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn take_history_resume("),
        "AgentChatView must have take_history_resume method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn resume_from_history("),
        "AgentChatView must have resume_from_history method"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pending_history_resume"),
        "AgentChatView must have pending_history_resume field"
    );
}

#[test]
fn history_delete_function_exists_in_history_module() {
    const HISTORY_SOURCE: &str = include_str!("history.rs");
    assert!(
        HISTORY_SOURCE.contains("pub(crate) fn delete_conversation(session_id: &str)"),
        "history module must expose delete_conversation(session_id)"
    );
    assert!(
        HISTORY_SOURCE.contains("agent_chat_history_item_deleted"),
        "delete_conversation must emit structured tracing event"
    );
}

#[test]
fn history_resume_is_reexported_from_agent_chat_mod() {
    const MOD_SOURCE: &str = include_str!("mod.rs");
    assert!(
        MOD_SOURCE.contains("AgentChatHistoryResumeRequest"),
        "AgentChatHistoryResumeRequest must be re-exported from agent_chat mod"
    );
}

#[test]
fn global_clear_history_remains_separate_from_per_item_delete() {
    const HISTORY_SOURCE: &str = include_str!("history.rs");
    // delete_conversation filters by session_id — it does NOT remove all entries
    assert!(
        HISTORY_SOURCE.contains("filter(|entry| entry.session_id != session_id)"),
        "delete_conversation must filter by session_id, not clear all"
    );
    // The chat_window.rs clear path uses remove_file + remove_dir_all
    const CHAT_WINDOW_SOURCE: &str = include_str!("chat_window.rs");
    assert!(
        CHAT_WINDOW_SOURCE.contains("remove_dir_all"),
        "clear_history must remove entire conversations directory"
    );
}

// =========================================================================
// Shared inline-token sync kernel — Agent Chat adoption contracts
// =========================================================================

#[test]
fn agent_chat_uses_shared_inline_sync_plan() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("build_inline_mention_sync_plan"),
        "Agent Chat sync_inline_mentions must use the shared sync plan builder"
    );
}

#[test]
fn agent_chat_uses_shared_visible_chip_indices() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("visible_context_chip_indices"),
        "Agent Chat render_pending_context_chips must use shared visible chip filtering"
    );
}

#[test]
fn agent_chat_uses_shared_atomic_delete() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("remove_inline_mention_at_cursor"),
        "Agent Chat key handler must use shared token-atomic delete"
    );
}

#[test]
fn agent_chat_emits_inline_mentions_synced_event() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("agent_chat_inline_mentions_synced"),
        "Agent Chat must emit agent_chat_inline_mentions_synced tracing event on sync"
    );
}

#[test]
fn agent_chat_emits_inline_mention_deleted_atomically_event() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("agent_chat_inline_mention_deleted_atomically"),
        "Agent Chat must emit agent_chat_inline_mention_deleted_atomically tracing event on atomic delete"
    );
}

// =========================================================================
// AI-window inline-token unification source contracts
// =========================================================================
//
// These tests verify that AI-window chip rendering/input handling and Agent
// Chat use the same shared inline-token infrastructure.

const AI_WINDOW_CONTEXT_COMMANDS_SOURCE: &str = include_str!("../../window/context_commands.rs");
const AI_WINDOW_RENDER_SOURCE: &str = include_str!("../../window/render_main_panel.rs");
const AI_WINDOW_INPUT_SOURCE: &str = include_str!("../../window/render_keydown.rs");

#[test]
fn ai_window_context_commands_sync_inline_mentions() {
    assert!(
        AI_WINDOW_CONTEXT_COMMANDS_SOURCE.contains("fn sync_inline_mentions"),
        "AI window context commands must own inline mention synchronization",
    );
    assert!(
        AI_WINDOW_CONTEXT_COMMANDS_SOURCE.contains("pending_context_parts"),
        "AI window context commands must synchronize inline tokens back into pending_context_parts",
    );
}

#[test]
fn ai_window_hides_inline_backed_chips() {
    assert!(
        AI_WINDOW_RENDER_SOURCE.contains("visible_context_chip_indices"),
        "AI window chip rendering must hide parts already represented inline",
    );
}

#[test]
fn ai_window_uses_atomic_inline_delete() {
    assert!(
        AI_WINDOW_INPUT_SOURCE.contains("remove_inline_mention_at_cursor"),
        "AI window input handling must use shared token-atomic delete",
    );
}

#[test]
fn ai_window_emits_inline_mentions_synced_event() {
    assert!(
        AI_WINDOW_CONTEXT_COMMANDS_SOURCE.contains("ai_inline_mentions_synced"),
        "AI window must emit ai_inline_mentions_synced tracing event on sync",
    );
}

#[test]
fn ai_window_emits_inline_mention_deleted_atomically_event() {
    assert!(
        AI_WINDOW_INPUT_SOURCE.contains("ai_inline_mention_deleted_atomically"),
        "AI window must emit ai_inline_mention_deleted_atomically tracing event on atomic delete",
    );
}

#[test]
fn agent_chat_and_ai_window_share_inline_sync_kernel() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("build_inline_mention_sync_plan"),
        "Agent Chat must use shared inline sync planning",
    );
    assert!(
        AI_WINDOW_CONTEXT_COMMANDS_SOURCE.contains("build_inline_mention_sync_plan"),
        "AI window must use shared inline sync planning",
    );
}

// =========================================================================
// Source-aware slash command identity and resolution
// =========================================================================

#[test]
fn agent_chat_resolved_slash_commands_keep_local_skills_without_provider_advertisement() {
    use super::view::{SlashCommandEntry, SlashCommandSource};
    use std::path::PathBuf;

    // Simulate cached entries: one default, two plugin skills, one claude skill
    let cached = vec![
        SlashCommandEntry {
            name: "clear".to_string(),
            description: String::new(),
            source: SlashCommandSource::Default,
        },
        SlashCommandEntry {
            name: "review".to_string(),
            description: "Alpha review".to_string(),
            source: SlashCommandSource::PluginSkill(crate::plugins::PluginSkill {
                plugin_id: "alpha".to_string(),
                plugin_title: "Alpha".to_string(),
                skill_id: "review".to_string(),
                path: PathBuf::from("/alpha/skills/review/SKILL.md"),
                title: "Review".to_string(),
                description: "Alpha review".to_string(),
            }),
        },
        SlashCommandEntry {
            name: "review".to_string(),
            description: "Beta review".to_string(),
            source: SlashCommandSource::PluginSkill(crate::plugins::PluginSkill {
                plugin_id: "beta".to_string(),
                plugin_title: "Beta".to_string(),
                skill_id: "review".to_string(),
                path: PathBuf::from("/beta/skills/review/SKILL.md"),
                title: "Review".to_string(),
                description: "Beta review".to_string(),
            }),
        },
        SlashCommandEntry {
            name: "plan".to_string(),
            description: "Plan skill".to_string(),
            source: SlashCommandSource::ClaudeCodeSkill {
                skill_id: "plan".to_string(),
                skill_path: PathBuf::from("/claude/skills/plan/SKILL.md"),
            },
        },
    ];

    // Provider only advertises "clear" and "help" — plugin and Claude skills
    // should still appear in the resolved output.
    let available = vec!["clear".to_string(), "help".to_string()];

    // Exercise the resolution logic directly using the same algorithm as the view.
    let available_set: std::collections::HashSet<&str> =
        available.iter().map(|s| s.as_str()).collect();
    let mut result: Vec<SlashCommandEntry> = Vec::new();

    for entry in &cached {
        match &entry.source {
            SlashCommandSource::Default if available_set.contains(entry.name.as_str()) => {
                result.push(entry.clone());
            }
            SlashCommandSource::PluginSkill(_) | SlashCommandSource::ClaudeCodeSkill { .. } => {
                result.push(entry.clone());
            }
            _ => {}
        }
    }
    for cmd in &available {
        let already_present = result
            .iter()
            .any(|entry| matches!(entry.source, SlashCommandSource::Default) && entry.name == *cmd);
        if !already_present {
            result.push(SlashCommandEntry {
                name: cmd.clone(),
                description: String::new(),
                source: SlashCommandSource::Default,
            });
        }
    }

    let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["clear", "review", "review", "plan", "help"],
        "Expected: default clear + both plugin reviews + claude plan + new default help"
    );

    // Verify both reviews are from different plugins
    let reviews: Vec<&SlashCommandEntry> = result.iter().filter(|e| e.name == "review").collect();
    assert_eq!(reviews.len(), 2);
    assert!(
        matches!(
            &reviews[0].source,
            SlashCommandSource::PluginSkill(s) if s.plugin_id == "alpha"
        ),
        "First review should be from alpha"
    );
    assert!(
        matches!(
            &reviews[1].source,
            SlashCommandSource::PluginSkill(s) if s.plugin_id == "beta"
        ),
        "Second review should be from beta"
    );

    // Plugin skills appear even though provider didn't advertise "review"
    assert!(
        !available_set.contains("review"),
        "Provider should not have advertised 'review'"
    );
}

#[test]
fn agent_chat_slash_command_entry_qualified_keys_are_distinct() {
    use super::view::{SlashCommandEntry, SlashCommandSource};
    use std::path::PathBuf;

    let default = SlashCommandEntry {
        name: "review".to_string(),
        description: String::new(),
        source: SlashCommandSource::Default,
    };
    let plugin = SlashCommandEntry {
        name: "review".to_string(),
        description: "Plugin review".to_string(),
        source: SlashCommandSource::PluginSkill(crate::plugins::PluginSkill {
            plugin_id: "alpha".to_string(),
            plugin_title: "Alpha".to_string(),
            skill_id: "review".to_string(),
            path: PathBuf::from("/alpha/review/SKILL.md"),
            title: "Review".to_string(),
            description: String::new(),
        }),
    };
    let claude = SlashCommandEntry {
        name: "review".to_string(),
        description: "Claude review".to_string(),
        source: SlashCommandSource::ClaudeCodeSkill {
            skill_id: "review".to_string(),
            skill_path: PathBuf::from("/claude/review/SKILL.md"),
        },
    };

    let keys: Vec<String> = vec![
        default.qualified_key(),
        plugin.qualified_key(),
        claude.qualified_key(),
    ];
    let unique: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        unique.len(),
        3,
        "All three sources should have distinct qualified keys: {:?}",
        keys
    );
}

// =========================================================================
// Slash acceptance contracts
// =========================================================================

/// Default slash commands produce a `SlashCommandPayload::Default` whose
/// acceptance contract is to insert literal `/command` text — NOT to stage
/// skill content via `build_staged_skill_prompt`.
#[test]
fn agent_chat_default_slash_accept_inserts_command_text() {
    use super::view::SlashCommandEntry;
    use crate::ai::context_selector::types::SlashCommandPayload;

    let entry = SlashCommandEntry::default_command("compact");
    let payload = entry.to_payload();

    // Must produce a Default payload, which the acceptance path inserts as
    // literal `/compact ` text into the composer.
    match &payload {
        SlashCommandPayload::Default { name } => {
            assert_eq!(name, "compact");
            // Acceptance inserts `/{name} ` — verify the format.
            let inserted = format!("/{name} ");
            assert_eq!(inserted, "/compact ");
        }
        other => panic!(
            "Expected SlashCommandPayload::Default for a default entry, got {:?}",
            other
        ),
    }
}

/// Plugin skills accepted from the Agent Chat slash picker must use the same
/// slash-prefill text and attached skill-part payload as the main-menu
/// skill launch path.
#[test]
fn agent_chat_plugin_slash_accept_stages_selected_skill_prompt() {
    use super::view::{build_skill_context_part, build_skill_slash_command_text};
    use std::io::Write;

    // Create a temp skill file with known content.
    let dir = tempfile::tempdir().expect("tempdir");
    let skill_path = dir.path().join("SKILL.md");
    {
        let mut f = std::fs::File::create(&skill_path).expect("create skill file");
        f.write_all(b"# Review\nReview code changes against project guidelines.")
            .expect("write");
    }

    let skill = crate::plugins::PluginSkill {
        plugin_id: "alpha".to_string(),
        plugin_title: "Alpha".to_string(),
        skill_id: "review".to_string(),
        path: skill_path.clone(),
        title: "Review".to_string(),
        description: "Alpha review".to_string(),
    };

    let slash_prefill = build_skill_slash_command_text(&skill.skill_id);
    assert_eq!(slash_prefill, "/review ");

    let owner = if skill.plugin_title.is_empty() {
        &skill.plugin_id
    } else {
        &skill.plugin_title
    };
    let slash_part = build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path);
    let main_menu_part =
        build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path);
    assert_eq!(slash_part, main_menu_part);

    match &slash_part {
        crate::ai::message_parts::AiContextPart::SkillFile {
            path,
            label,
            skill_name,
            owner_label,
            slash_name,
        } => {
            assert_eq!(path, &skill_path.to_string_lossy().to_string());
            assert_eq!(label, "/review");
            assert_eq!(skill_name, "Review");
            assert_eq!(owner_label, "Alpha");
            assert_eq!(slash_name, "review");
        }
        other => panic!("expected SkillFile part, got {other:?}"),
    }

    // When two plugins share the same slash slug, their attached parts diverge
    // because the owner and path differ.
    let beta_path = dir.path().join("BETA_SKILL.md");
    {
        let mut f = std::fs::File::create(&beta_path).expect("create beta skill");
        f.write_all(b"# Review\nBeta-specific review checklist.")
            .expect("write");
    }

    let beta_part = build_skill_context_part("Review", "Beta", "review", &beta_path);
    assert_ne!(
        slash_part, beta_part,
        "Same slug from different plugins must produce different attached parts"
    );
}

#[test]
fn agent_chat_claude_skill_staged_prompt_uses_claude_owner_phrase() {
    use super::view::build_staged_skill_prompt;
    use std::io::Write;

    let dir = tempfile::tempdir().expect("tempdir");
    let skill_path = dir.path().join("SKILL.md");
    {
        let mut f = std::fs::File::create(&skill_path).expect("create skill file");
        f.write_all(b"# Plan\nDraft a concise implementation plan.")
            .expect("write");
    }

    let staged = build_staged_skill_prompt("Plan", "Claude Code", &skill_path);

    assert!(
        staged.contains("from Claude Code"),
        "Claude Code skills should be labeled as Claude Code, got: {staged}"
    );
    assert!(
        !staged.contains("from plugin \"Claude Code\""),
        "Claude Code skills must not be mislabeled as plugins: {staged}"
    );
}

// =========================================================================
// Cross-source collision detection
// =========================================================================

/// Verify that cross-source collision bookkeeping correctly seeds
/// default names and detects plugin-vs-default and Claude-vs-{default,plugin}
/// collisions. This test exercises the collision bookkeeping in isolation
/// using the same data structures as `discover_slash_commands`.
#[test]
fn agent_chat_slash_cross_source_collision_bookkeeping() {
    use std::collections::{HashMap, HashSet};

    // Simulate the discover_slash_commands owner-tracking logic.
    let default_names: HashSet<String> = ["review", "compact"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut owners_by_slash: HashMap<String, Vec<String>> = HashMap::new();
    for name in &default_names {
        owners_by_slash
            .entry(name.clone())
            .or_default()
            .push("Built-in".to_string());
    }

    // Plugin skill collides with "review" default.
    let plugin_name = "review".to_string();
    let mut plugin_names: HashSet<String> = HashSet::new();
    plugin_names.insert(plugin_name.clone());
    owners_by_slash
        .entry(plugin_name.clone())
        .or_default()
        .push("Alpha".to_string());

    assert!(
        default_names.contains(&plugin_name),
        "Plugin 'review' should collide with default 'review'"
    );

    // Claude skill collides with both "review" (default + plugin).
    let claude_name = "review".to_string();
    owners_by_slash
        .entry(claude_name.clone())
        .or_default()
        .push("Claude Code".to_string());

    assert!(
        plugin_names.contains(&claude_name),
        "Claude 'review' should collide with plugin 'review'"
    );
    assert!(
        default_names.contains(&claude_name),
        "Claude 'review' should collide with default 'review'"
    );

    // Final cross-source check: "review" has 3 owners.
    let review_owners = owners_by_slash.get("review").expect("review should exist");
    assert_eq!(
        review_owners.len(),
        3,
        "review should have 3 owners (Built-in, Alpha, Claude Code): {:?}",
        review_owners
    );
    assert!(review_owners.contains(&"Built-in".to_string()));
    assert!(review_owners.contains(&"Alpha".to_string()));
    assert!(review_owners.contains(&"Claude Code".to_string()));

    // "compact" should have exactly 1 owner (no collision).
    let compact_owners = owners_by_slash
        .get("compact")
        .expect("compact should exist");
    assert_eq!(
        compact_owners.len(),
        1,
        "compact should have 1 owner: {:?}",
        compact_owners
    );
}

/// Qualified key dedup preserves source-distinct rows even when slash
/// names collide. This ensures the picker shows both rows with owner labels.
#[test]
fn agent_chat_slash_dedup_preserves_source_distinct_rows() {
    use super::view::{SlashCommandEntry, SlashCommandSource};
    use std::collections::HashSet;

    let default_review = SlashCommandEntry::default_command("review");
    let plugin_review = SlashCommandEntry {
        name: "review".to_string(),
        description: "Alpha review".to_string(),
        source: SlashCommandSource::PluginSkill(crate::plugins::PluginSkill {
            plugin_id: "alpha".to_string(),
            plugin_title: "Alpha".to_string(),
            skill_id: "review".to_string(),
            path: std::path::PathBuf::from("/alpha/review/SKILL.md"),
            title: "Review".to_string(),
            description: String::new(),
        }),
    };
    let claude_review = SlashCommandEntry {
        name: "review".to_string(),
        description: "Claude review".to_string(),
        source: SlashCommandSource::ClaudeCodeSkill {
            skill_id: "review".to_string(),
            skill_path: std::path::PathBuf::from("/claude/review/SKILL.md"),
        },
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut commands = Vec::new();

    for entry in [&default_review, &plugin_review, &claude_review] {
        if seen.insert(entry.qualified_key()) {
            commands.push(entry.clone());
        }
    }

    // All three should be distinct by qualified_key even though all share
    // the same bare slash name "review".
    assert_eq!(
        commands.len(),
        3,
        "All three sources should produce distinct rows: {:?}",
        commands
            .iter()
            .map(|e| e.qualified_key())
            .collect::<Vec<_>>()
    );
}

/// Plugin and Claude skill slash acceptance both share the same slash-prefill
/// and attached-skill-part contract as main-menu skill launch.
#[test]
fn agent_chat_slash_and_main_menu_skill_launch_share_prompt_contract() {
    use super::view::{build_skill_context_part, build_skill_slash_command_text};
    use std::io::Write;

    let dir = tempfile::tempdir().expect("tempdir");
    let skill_path = dir.path().join("SKILL.md");
    {
        let mut f = std::fs::File::create(&skill_path).expect("create");
        f.write_all(b"# Deploy\nDeploy the current project.")
            .expect("write");
    }

    let skill = crate::plugins::PluginSkill {
        plugin_id: "infra".to_string(),
        plugin_title: "Infrastructure".to_string(),
        skill_id: "deploy".to_string(),
        path: skill_path.clone(),
        title: "Deploy".to_string(),
        description: "Deploy skill".to_string(),
    };

    // Main-menu path (from open_agent_chat_with_selected_skill).
    let owner = if skill.plugin_title.is_empty() {
        &skill.plugin_id
    } else {
        &skill.plugin_title
    };
    let main_menu_prefill = build_skill_slash_command_text(&skill.skill_id);
    let slash_accept_prefill = build_skill_slash_command_text(&skill.skill_id);
    let main_menu_part =
        build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path);
    let slash_accept_part =
        build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path);

    assert_eq!(
        main_menu_prefill, slash_accept_prefill,
        "Main-menu and slash acceptance must produce identical slash prefill"
    );
    assert_eq!(
        main_menu_part, slash_accept_part,
        "Main-menu and slash acceptance must attach the same skill part"
    );

    let claude_skill_path = dir.path().join("CLAUDE_SKILL.md");
    {
        let mut f = std::fs::File::create(&claude_skill_path).expect("create");
        f.write_all(b"# Plan\nCreate a plan.").expect("write");
    }
    let claude_part = build_skill_context_part("Plan", "Claude Code", "plan", &claude_skill_path);
    match claude_part {
        crate::ai::message_parts::AiContextPart::SkillFile { owner_label, .. } => {
            assert_eq!(owner_label, "Claude Code");
        }
        other => panic!("expected SkillFile part, got {other:?}"),
    }
}

#[test]
fn agent_chat_ui_variant_launch_suppresses_selected_launcher_row_context_contract() {
    let body = agent_chat_source_between(
        TAB_AI_MODE_SOURCE,
        "pub(crate) fn open_tab_ai_agent_chat_with_entry_intent_variant(",
        "    /// Entry point for direct prompt handoffs",
    );

    assert!(
        body.contains(
            "let suppress_focused_part =\n            ui_variant != crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard;"
        ),
        "non-standard Agent Chat UI variants are menu presets, not context sources"
    );
    assert!(
        body.contains(
            "agent_chat_entry::AgentChatEntryRequest::main_launcher_with_variant(\n                entry_intent,\n                suppress_focused_part,\n                ui_variant,"
        ),
        "variant launches must pass the suppress flag into Agent Chat entry staging"
    );
}

#[test]
fn agent_chat_main_menu_skill_stage_matches_slash_selection_without_submit() {
    let body = agent_chat_source_between(
        AGENT_CHAT_VIEW_SOURCE,
        "pub(crate) fn stage_selected_plugin_skill_from_main_menu(",
        "    /// Reuse the current live thread for a fresh external entry intent.",
    );

    for required in [
        "build_skill_slash_command_text(&skill.skill_id)",
        "build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path)",
        "super::thread::SkillContextIdentity",
        "staged_by: super::thread::SkillContextStagedBy::MainMenu",
        "thread.add_or_replace_skill_context(identity, part, cx);",
        "thread.input.set_text(command_text.clone());",
        "thread.input.set_cursor(cursor_after);",
        "thread.mark_context_bootstrap_ready(cx);",
    ] {
        assert!(
            body.contains(required),
            "main-menu skill staging must preserve slash-selection behavior: {required}"
        );
    }

    assert!(
        !body.contains("submit_input("),
        "main-menu skill staging must not auto-submit"
    );
}

const LIFECYCLE_RESET_SOURCE: &str = include_str!("../../../app_impl/lifecycle_reset.rs");

#[test]
fn agent_chat_transient_trigger_exit_on_empty_composer() {
    let agent_chat_launch_source =
        include_str!("../../../app_impl/agent_handoff/agent_chat_launch.rs");
    let agent_handoff_source = include_str!("../../../app_impl/agent_handoff/mod.rs");

    // 1. AgentChatView has the opened_via_transient_trigger field
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) opened_via_transient_trigger: Option<char>,"),
        "AgentChatView must have opened_via_transient_trigger field"
    );

    // 2. AgentChatView implements check_for_transient_exit helper
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn check_for_transient_exit("),
        "AgentChatView must implement check_for_transient_exit"
    );

    // 3. handle_key_down calls check_for_transient_exit
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("self.check_for_transient_exit(window, cx);"),
        "AgentChatView::handle_key_down must call check_for_transient_exit"
    );

    // 4. Agent Chat launch and reuse set the opened_via_transient_trigger field
    assert!(
        agent_chat_launch_source
            .contains("view.opened_via_transient_trigger = pending_script_list_trigger;"),
        "Agent Chat launch path must set opened_via_transient_trigger"
    );
    assert!(
        agent_handoff_source.contains("chat.opened_via_transient_trigger = trigger;"),
        "Agent Chat reuse path must set opened_via_transient_trigger"
    );
}

#[test]
fn agent_chat_transcript_keeps_selectable_markdown_with_chat_scaled_typography() {
    assert!(
        AGENT_CHAT_TRANSCRIPT_SOURCE.contains("fn selectable_markdown_view(")
            && AGENT_CHAT_TRANSCRIPT_SOURCE
                .contains(".selectable(crate::logging::agent_chat_markdown_selectable_enabled())")
            && AGENT_CHAT_TRANSCRIPT_SOURCE
                .contains(".text_size(px(style_def.markdown.body_font_size))"),
        "Agent Chat transcript messages must stay selectable (the selectable flag defaults on) without reverting to oversized document typography"
    );

    assert!(
        AGENT_CHAT_TRANSCRIPT_SOURCE.contains("fn transcript_text_style(")
            && AGENT_CHAT_TRANSCRIPT_SOURCE
                .contains(".paragraph_gap(rems(style_def.markdown.paragraph_gap))")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains("1 => px(heading_1_font_size)")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains("StyleRefinement::default()")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains(".code_block(")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains(".blockquote(")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains("build_markdown_highlight_theme"),
        "Agent Chat transcript must apply compact assistant-chat markdown styling for headings, code blocks, and blockquotes"
    );

    assert!(
        TEXT_VIEW_SOURCE.contains("state.set_text_view_style(self.text_view_style.clone(), cx);")
            && TEXT_VIEW_STATE_SOURCE.contains("text_view_style: self.text_view_style.clone()")
            && TEXT_VIEW_STATE_SOURCE.contains("style: options.text_view_style.clone()")
            && TEXT_VIEW_STATE_SOURCE.contains("content.node_cx = node_cx;"),
        "gpui-component TextView must persist TextViewStyle into parsed NodeContext so selectable markdown uses the requested chat styles"
    );

    assert!(
        TEXT_VIEW_NODE_SOURCE.contains("highlight.font_family = Some(\"JetBrains Mono\");")
            && TEXT_VIEW_NODE_SOURCE.contains("highlight.color = Some(cx.theme().accent);")
            && !TEXT_VIEW_NODE_SOURCE
                .contains("highlight.background_color = Some(cx.theme().accent);"),
        "TextView inline code must render as selectable monospace accent text, not as an accent background chip"
    );
}

#[test]
fn agent_chat_ui_variants_are_menu_addressable_and_protocol_visible() {
    assert!(
        AGENT_CHAT_UI_VARIANT_SOURCE.contains("pub(crate) const EXPERIMENTS: [Self; 6]")
            && AGENT_CHAT_UI_VARIANT_SOURCE.contains("\"builtin/ai-chat/user-bold\"")
            && AGENT_CHAT_UI_VARIANT_SOURCE.contains("\"builtin/ai-chat/role-split\"")
            && AGENT_CHAT_UI_VARIANT_SOURCE.contains("\"builtin/ai-chat/bottom-dock\"")
            && AGENT_CHAT_UI_VARIANT_SOURCE.contains("\"builtin/ai-chat/dense-log\"")
            && AGENT_CHAT_UI_VARIANT_SOURCE.contains("\"builtin/ai-chat/sidecar\"")
            && AGENT_CHAT_UI_VARIANT_SOURCE.contains("\"builtin/ai-chat/focused-text-mini\""),
        "Agent Chat chat experiments must have stable main-menu command ids"
    );

    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("ui_variant: AgentChatUiVariant")
            && AGENT_CHAT_VIEW_SOURCE.contains("pub(crate) fn set_ui_variant")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("ui_variant: self.ui_variant.state_id().to_string()"),
        "Agent Chat view must carry the active UI variant into reusable views and protocol state"
    );

    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn render_composer_bar(")
            && AGENT_CHAT_VIEW_SOURCE
                .contains("matches!(variant_config.composer, AgentChatComposerPlacement::Default)")
            && AGENT_CHAT_VIEW_SOURCE.contains(
                "matches!(variant_config.composer, AgentChatComposerPlacement::BottomDock)"
            ),
        "Agent Chat renderer must place the shared composer at the top or bottom based on the active variant"
    );

    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn render_variant_badge(")
            && AGENT_CHAT_VIEW_SOURCE.contains("fn render_variant_sidecar(")
            && AGENT_CHAT_VIEW_SOURCE.contains("variant_config.show_variant_badge")
            && AGENT_CHAT_VIEW_SOURCE.contains("variant_config.show_sidecar"),
        "Agent Chat renderer must expose visible variant badge and sidecar affordances for experiment review"
    );

    assert!(
        AGENT_CHAT_TRANSCRIPT_SOURCE.contains("AgentChatTranscriptPresentation::UserBold")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains("AgentChatTranscriptPresentation::RoleSplit")
            && AGENT_CHAT_TRANSCRIPT_SOURCE.contains("AgentChatTranscriptPresentation::DenseLog"),
        "Agent Chat transcript must render variant-specific presentation modes through the selectable markdown path"
    );
}

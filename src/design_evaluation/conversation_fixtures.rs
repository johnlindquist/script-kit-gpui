//! Closed production conversation seeds and controls. No renderer lives here.
use super::fixture_ids::{AGENT_CHAT_FIXTURE_IDS, FLOW_FIXTURE_IDS, SDK_CHAT_FIXTURE_IDS};
use crate::ai::agent_chat::events::AgentChatEvent;
use crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy;
use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
use crate::ai::agent_chat::ui::{AgentChatThread, AgentChatView};
use gpui::{App, AppContext, Entity, Window};

#[derive(Clone, Debug)]
pub(crate) enum AgentChatFixtureAction {
    Submit { text: String },
    Retry,
    Stop,
    HoldDrain,
    RetainDrain,
    ReleaseDrain { turn_generation: u64 },
    EmitText { turn_generation: u64, text: String },
    Complete { turn_generation: u64 },
    Fail { turn_generation: u64 },
    OpenHistory,
    OpenSlashPicker,
    OpenProfilePicker,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentChatFixtureReceipt {
    pub thread_id: String,
    pub transcript_generation: u64,
    pub turn_id: u64,
    pub stream_generation: u64,
    pub status: String,
    pub started_turns: usize,
    pub local_stream_cancelled: bool,
    pub remote_cancel_requested: bool,
    pub remote_cancel_acknowledged: bool,
    pub permission_reply_count: usize,
    pub drains: Vec<crate::ai::agent_chat::ui::mock_fixture::FixtureDrainReceipt>,
    pub session_policy: String,
    pub capabilities: crate::ai::agent_chat::ui::capabilities::AgentChatCapabilities,
    pub assistant_text: String,
}

fn retryable_failure() -> AgentChatEvent {
    AgentChatEvent::TurnFailed {
        failure: crate::ai::reliability::runtime_closed_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            "controlled fixture transport closed",
        ),
    }
}

pub(crate) fn create_agent_chat_fixture(
    fixture_id: &str,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<Entity<AgentChatView>> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    anyhow::ensure!(
        AGENT_CHAT_FIXTURE_IDS.contains(&fixture_id),
        "unknown_agent_chat_fixture"
    );
    if fixture_id == "agent-chat.initial-setup" {
        let state =
            crate::ai::agent_chat::ui::AgentChatInlineSetupState::from_runtime_setup_required(
                None,
                Vec::new(),
                Default::default(),
                "auth_required",
                &["Fixture sign-in".into()],
            );
        return Ok(cx.new(|cx| {
            AgentChatView::new_setup_with_policy(state, AgentChatSessionPolicy::Full, cx)
        }));
    }
    let variant = match fixture_id {
        "agent-chat.user-bold.awaiting-first-text" => AgentChatUiVariant::UserBold,
        "agent-chat.role-split.streaming" => AgentChatUiVariant::RoleSplit,
        "agent-chat.bottom-dock.stopped" => AgentChatUiVariant::BottomDock,
        "agent-chat.dense-log.retryable-failure" => AgentChatUiVariant::DenseLog,
        "agent-chat.sidecar.permission-pending" => AgentChatUiVariant::Sidecar,
        "agent-chat.focused-text-mini.populated" => AgentChatUiVariant::FocusedTextMini,
        "agent-chat.quick-ai.empty" => AgentChatUiVariant::QuickAi,
        _ => AgentChatUiVariant::Standard,
    };
    let (thread, control) = crate::ai::agent_chat::ui::mock_fixture::create_mock_fixture_thread(
        fixture_id,
        AgentChatSessionPolicy::for_launch_variant(variant),
        cx,
    );
    let view = cx.new(|cx| AgentChatView::new(thread.clone(), cx).with_ui_variant(variant));
    if fixture_id.ends_with(".populated") {
        thread
            .update(cx, |thread, cx| {
                thread.apply_test_fixture(
                    "assistantText",
                    Some("Summarize this fixture".into()),
                    Some("A deterministic production conversation.".into()),
                    None,
                    cx,
                )
            })
            .map_err(anyhow::Error::msg)?;
    }
    if variant == AgentChatUiVariant::FocusedTextMini {
        let mut snapshot =
            crate::platform::accessibility::focused_text::focused_text_snapshot_for_capture_failure(
            );
        snapshot.text = "A short editable fixture paragraph.".into();
        snapshot.metrics = crate::platform::accessibility::TextMetrics::from_text(&snapshot.text);
        view.update(cx, |view, cx| {
            view.stage_focused_text_from_host(
                snapshot,
                Some("Make this concise".into()),
                "owned-fixture",
                cx,
            )
        })
        .map_err(anyhow::Error::msg)?;
    }
    if fixture_id.ends_with(".retryable-failure") {
        control.queue_turn(vec![retryable_failure()])?;
    } else if fixture_id == "agent-chat.runtime-setup" {
        control.queue_turn(vec![AgentChatEvent::SetupRequired {
            reason: "auth_required".into(),
            auth_methods: vec!["Fixture sign-in".into()],
        }])?;
    }
    if matches!(
        fixture_id,
        "agent-chat.user-bold.awaiting-first-text"
            | "agent-chat.role-split.streaming"
            | "agent-chat.bottom-dock.stopped"
            | "agent-chat.dense-log.retryable-failure"
            | "agent-chat.sidecar.permission-pending"
            | "agent-chat.standard.queued"
            | "agent-chat.runtime-setup"
            | "agent-chat.detached.retryable-failure"
    ) {
        drive_agent_chat_fixture(
            &view,
            AgentChatFixtureAction::Submit {
                text: "Fixture request".into(),
            },
            window,
            cx,
        )?;
    }
    if fixture_id == "agent-chat.role-split.streaming" {
        control.emit(
            1,
            AgentChatEvent::AgentMessageDelta("Streaming fixture text ".into()),
        )?;
    }
    if fixture_id == "agent-chat.bottom-dock.stopped" {
        drive_agent_chat_fixture(&view, AgentChatFixtureAction::Stop, window, cx)?;
    }
    if fixture_id == "agent-chat.standard.queued" {
        drive_agent_chat_fixture(
            &view,
            AgentChatFixtureAction::Submit {
                text: "Queued follow-up".into(),
            },
            window,
            cx,
        )?;
    }
    if fixture_id == "agent-chat.sidecar.permission-pending" {
        control.request_permission(crate::ai::agent_chat::ui::approval_request_input(
            "Fixture approval",
            crate::ai::agent_chat::ui::AgentChatApprovalPreview::new(
                "Read fixture",
                "fixture-read",
            ),
            vec![
                crate::ai::agent_chat::ui::AgentChatApprovalOption {
                    option_id: "allow-once".into(),
                    name: "Allow once".into(),
                    kind: "AllowOnce".into(),
                },
                crate::ai::agent_chat::ui::AgentChatApprovalOption {
                    option_id: "deny".into(),
                    name: "Deny".into(),
                    kind: "RejectOnce".into(),
                },
            ],
        ))?;
    }
    if fixture_id == "agent-chat.standard.picker-open" {
        let popup_view = view.clone();
        window.defer(cx, move |window, cx| {
            popup_view.update(cx, |view, cx| view.open_slash_picker_in_window(window, cx));
        });
    }
    Ok(view)
}

pub(crate) fn drive_agent_chat_fixture(
    view: &Entity<AgentChatView>,
    action: AgentChatFixtureAction,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<AgentChatFixtureReceipt> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    let thread = view
        .read(cx)
        .thread()
        .ok_or_else(|| anyhow::anyhow!("fixture_has_no_thread"))?;
    let control = thread
        .read(cx)
        .fixture_control()
        .ok_or_else(|| anyhow::anyhow!("not_a_fixture_connection"))?;
    match action {
        AgentChatFixtureAction::Submit { text } => {
            anyhow::ensure!(
                !text.trim().is_empty() && text.len() <= 64 * 1024,
                "invalid_fixture_input"
            );
            view.update(cx, |view, cx| view.set_input_in_window(text, window, cx));
            thread
                .update(cx, |thread, cx| thread.submit_input(cx))
                .map_err(anyhow::Error::msg)?;
        }
        AgentChatFixtureAction::Retry => thread
            .update(cx, |thread, cx| thread.retry_last_user_turn(cx))
            .map_err(anyhow::Error::msg)?,
        AgentChatFixtureAction::Stop => {
            anyhow::ensure!(
                view.update(cx, |view, cx| view.stop_streaming_explicitly(cx)),
                "fixture_turn_not_stoppable"
            );
        }
        AgentChatFixtureAction::HoldDrain => thread.read(cx).hold_fixture_drain()?,
        AgentChatFixtureAction::RetainDrain => {
            thread.update(cx, |thread, _| thread.retain_fixture_drain())?
        }
        AgentChatFixtureAction::ReleaseDrain { turn_generation } => {
            control.release_drain(turn_generation)?
        }
        AgentChatFixtureAction::EmitText {
            turn_generation,
            text,
        } => {
            anyhow::ensure!(text.len() <= 64 * 1024, "fixture_text_limit");
            emit_current_chat_event(
                &thread,
                &control,
                turn_generation,
                AgentChatEvent::AgentMessageDelta(text),
                cx,
            )?;
        }
        AgentChatFixtureAction::Complete { turn_generation } => emit_current_chat_event(
            &thread,
            &control,
            turn_generation,
            AgentChatEvent::completed("fixture"),
            cx,
        )?,
        AgentChatFixtureAction::Fail { turn_generation } => {
            emit_current_chat_event(&thread, &control, turn_generation, retryable_failure(), cx)?
        }
        AgentChatFixtureAction::OpenHistory => {
            view.update(cx, |view, cx| view.toggle_history_popup(window, cx))
        }
        AgentChatFixtureAction::OpenSlashPicker => {
            view.update(cx, |view, cx| view.open_slash_picker_in_window(window, cx))
        }
        AgentChatFixtureAction::OpenProfilePicker => view.update(cx, |view, cx| {
            view.open_profile_picker_in_window(window, cx)
        }),
    }
    agent_chat_fixture_receipt(&thread, cx)
}

fn emit_current_chat_event(
    thread: &Entity<AgentChatThread>,
    control: &crate::ai::agent_chat::ui::mock_fixture::AgentChatFixtureControl,
    stream_generation: u64,
    event: AgentChatEvent,
    cx: &App,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        thread.read(cx).turn_identity().3 == stream_generation,
        "stale_fixture_stream"
    );
    control.emit(control.latest_generation()?, event)
}

pub(crate) fn agent_chat_fixture_receipt(
    thread: &Entity<AgentChatThread>,
    cx: &App,
) -> anyhow::Result<AgentChatFixtureReceipt> {
    let thread = thread.read(cx);
    let control = thread
        .fixture_control()
        .ok_or_else(|| anyhow::anyhow!("not_a_fixture_connection"))?;
    let (id, transcript, turn, stream) = thread.turn_identity();
    let stop = thread.last_local_stop();
    Ok(AgentChatFixtureReceipt {
        thread_id: id.into(),
        transcript_generation: transcript,
        turn_id: turn,
        stream_generation: stream,
        status: format!("{:?}", thread.status).to_ascii_lowercase(),
        started_turns: control.receipts()?.len(),
        local_stream_cancelled: stop.is_some_and(|receipt| receipt.local_stream_cancelled),
        remote_cancel_requested: stop.is_some_and(|receipt| receipt.remote_cancel_requested),
        remote_cancel_acknowledged: stop.is_some_and(|receipt| receipt.remote_cancel_acknowledged),
        permission_reply_count: control.permission_reply_count()?,
        drains: control.drain_receipts()?,
        session_policy: format!("{:?}", thread.session_policy()),
        capabilities: thread.session_policy().capabilities(),
        assistant_text: thread
            .messages
            .iter()
            .filter(|message| {
                message.role
                    == crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole::Assistant
            })
            .map(|message| message.body.as_ref())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

pub(crate) fn open_detached_agent_chat_fixture(
    cx: &mut App,
) -> anyhow::Result<Entity<AgentChatView>> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    let (thread, control) = crate::ai::agent_chat::ui::mock_fixture::create_mock_fixture_thread(
        "agent-chat.detached.retryable-failure",
        AgentChatSessionPolicy::Full,
        cx,
    );
    control.queue_turn(vec![retryable_failure()])?;
    crate::ai::agent_chat::ui::chat_window::open_chat_window_with_thread_and_policy(
        thread.clone(),
        Some(gpui::Bounds {
            origin: gpui::point(gpui::px(0.), gpui::px(0.)),
            size: gpui::size(gpui::px(640.), gpui::px(520.)),
        }),
        crate::runtime_policy::WindowHostPolicy::OwnedHidden,
        cx,
    )?;
    thread
        .update(cx, |thread, cx| {
            thread.input.set_text("Fixture request".to_string());
            thread.submit_input(cx)
        })
        .map_err(anyhow::Error::msg)?;
    crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
        .ok_or_else(|| anyhow::anyhow!("detached_fixture_entity_missing"))
}

pub(crate) fn seed_owned_flow_catalogue() -> anyhow::Result<Vec<crate::flows::model::FlowDescriptor>>
{
    let scope = crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("owned_flow_required"))?;
    let path = scope.root().join("conversation.fixture");
    scope.require_owned_path(&path)?;
    if !path.exists() {
        std::fs::write(
            &path,
            "---\nmodel: fixture-model\n---\nSummarize the supplied fixture text.",
        )?;
    }
    let flows = vec![crate::flows::model::FlowDescriptor {
        id: "owned:conversation".into(),
        path: path.to_string_lossy().into_owned(),
        source: crate::flows::model::FlowSource::Project,
        name: "flow-conversation".into(),
        description: Some("A controlled production Flow conversation".into()),
        engine: "codex".into(),
        engine_source: Some("fixture".into()),
        inputs: Vec::new(),
        is_workflow: false,
        interactive: false,
        mtime_ms: 0,
        origin: Some("Owned fixture".into()),
        wrapper_command: None,
    }];
    crate::flows::catalog::flow_catalog()
        .install_owned_roster(scope.root().to_string_lossy().into_owned(), flows.clone())?;
    Ok(flows)
}

impl crate::ScriptListApp {
    pub(crate) fn mount_flow_fixture(
        &mut self,
        fixture_id: &str,
        cx: &mut gpui::Context<Self>,
    ) -> anyhow::Result<Option<u64>> {
        anyhow::ensure!(
            FLOW_FIXTURE_IDS.contains(&fixture_id),
            "unknown_flow_fixture"
        );
        let flows = seed_owned_flow_catalogue()?;
        let variant = match fixture_id {
            "flow.desk.dispatch" => crate::flows::model::FlowUxVariant::Dispatch,
            "flow.desk.lens" => crate::flows::model::FlowUxVariant::Lens,
            _ => crate::flows::model::FlowUxVariant::Flash,
        };
        // Give sessions a real Desk return route. A direct-open session closes
        // through the native hide barrier, which owned-hidden evaluation refuses.
        self.current_view = crate::AppView::FlowUxView {
            variant,
            filter: String::new(),
            selected_index: 0,
            inline_run: None,
        };
        self.note_main_route_changed();
        if fixture_id == "flow.session" {
            return self.create_owned_flow_session(&flows[0], cx).map(Some);
        }
        cx.notify();
        Ok(None)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum FlowFixtureAction {
    Submit {
        text: String,
    },
    Text {
        expected_message_id: String,
        text: String,
    },
    Complete {
        expected_message_id: String,
    },
    Fail {
        expected_message_id: String,
    },
    Retry,
    Stop,
    Background,
    Resume,
}

impl crate::ScriptListApp {
    pub(crate) fn drive_flow_fixture(
        &mut self,
        session_id: u64,
        action: FlowFixtureAction,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> anyhow::Result<()> {
        crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
        use crate::flows::codex_client::FlowThreadEvent;
        match action {
            FlowFixtureAction::Submit { text } => {
                anyhow::ensure!(
                    !text.trim().is_empty() && text.len() <= 64 * 1024,
                    "invalid_flow_fixture_input"
                );
                let result = self.submit_flow_chat_message(session_id, text, cx);
                anyhow::ensure!(
                    matches!(result, crate::FlowChatSubmitResult::Dispatched),
                    "flow_submit_not_accepted"
                );
            }
            FlowFixtureAction::Text {
                expected_message_id,
                text,
            } => {
                anyhow::ensure!(text.len() <= 64 * 1024, "flow_fixture_text_limit");
                self.apply_owned_flow_event(
                    session_id,
                    &expected_message_id,
                    FlowThreadEvent::AgentDelta {
                        session_id,
                        item_id: expected_message_id.clone(),
                        delta: text,
                    },
                    cx,
                )?;
            }
            FlowFixtureAction::Complete {
                expected_message_id,
            } => self.apply_owned_flow_event(
                session_id,
                &expected_message_id,
                FlowThreadEvent::TurnCompleted {
                    session_id,
                    outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Completed {
                        stop_reason: Some("fixture".into()),
                    },
                },
                cx,
            )?,
            FlowFixtureAction::Fail {
                expected_message_id,
            } => self.apply_owned_flow_event(
                session_id,
                &expected_message_id,
                FlowThreadEvent::TurnFailed {
                    session_id,
                    failure: crate::ai::reliability::runtime_closed_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Codex,
                        "controlled Flow stream closed",
                    ),
                },
                cx,
            )?,
            FlowFixtureAction::Retry => self.dispatch_flow_recovery_action(
                session_id,
                sk_protocol::ai_reliability::AiRecoveryAction::Retry,
                window,
                cx,
            ),
            FlowFixtureAction::Stop => self.stop_flow_session(session_id, cx),
            FlowFixtureAction::Background => self.background_flow_session(window, cx),
            FlowFixtureAction::Resume => self.open_flow_session(session_id, cx),
        }
        Ok(())
    }
}

pub(crate) fn open_agent_chat_popup_fixture(
    id: &str,
    view: &Entity<AgentChatView>,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<AgentChatFixtureReceipt> {
    let action = match id {
        "agent-chat.popup.history" => {
            use crate::ai::agent_chat::ui::history::{SavedConversation, SavedMessage};
            crate::ai::agent_chat::ui::history::seed_owned_history(&[SavedConversation {
                session_id: "owned-popup-history".into(),
                timestamp: "2026-08-28T12:00:00Z".into(),
                custom_title: Some("Fixture conversation".into()),
                messages: vec![
                    SavedMessage {
                        role: "user".into(),
                        body: "Summarize this fixture".into(),
                    },
                    SavedMessage {
                        role: "assistant".into(),
                        body: "A deterministic production conversation.".into(),
                    },
                ],
            }])?;
            AgentChatFixtureAction::OpenHistory
        }
        "agent-chat.popup.slash" => AgentChatFixtureAction::OpenSlashPicker,
        "agent-chat.popup.profile" => AgentChatFixtureAction::OpenProfilePicker,
        _ => anyhow::bail!("unknown_agent_chat_popup_fixture"),
    };
    drive_agent_chat_fixture(view, action, window, cx)
}

enum SdkFixtureRequest {
    Submit(crate::prompts::chat::ChatPromptPreparedRequest),
    Retry(crate::prompts::chat::ChatPromptPreparedRequest),
    Stop,
}

pub(crate) struct SdkChatFixtureControl {
    requests: async_channel::Receiver<SdkFixtureRequest>,
    pub accepted_requests: Vec<String>,
    pub sink_requests: Vec<serde_json::Value>,
    pub stop_requests: usize,
    next_response: u64,
}

pub(crate) enum SdkChatFixtureAction {
    Submit(String),
    Retry,
    Stop,
    Text { message_id: String, text: String },
    Complete { message_id: String },
    Fail { message_id: String },
}

pub(crate) fn create_sdk_chat_fixture(
    id: &str,
    _window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<(Entity<crate::prompts::ChatPrompt>, SdkChatFixtureControl)> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    anyhow::ensure!(
        SDK_CHAT_FIXTURE_IDS.contains(&id),
        "unknown_sdk_chat_fixture"
    );
    let (tx, requests) = async_channel::bounded(16);
    let retry_tx = tx.clone();
    let stop_tx = tx.clone();
    let prompt = cx.new(|cx| {
        crate::prompts::ChatPrompt::new_sdk(
            id.to_string(),
            Some("Message the fixture".into()),
            Vec::new(),
            None,
            None,
            cx.focus_handle(),
            Some(std::sync::Arc::new(move |request| {
                tx.try_send(SdkFixtureRequest::Submit(request))
                    .map_err(|_| "sdk_fixture_sink_full_or_closed".into())
            })),
            false,
            std::sync::Arc::new(crate::theme::get_cached_theme()),
        )
        .with_retry_callback(std::sync::Arc::new(move |request| {
            retry_tx
                .try_send(SdkFixtureRequest::Retry(request.prepared))
                .map_err(|_| {
                    Box::new(crate::ai::reliability::runtime_closed_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Provider,
                        "SDK fixture retry sink full or closed",
                    ))
                })
        }))
        .with_stop_callback(std::sync::Arc::new(move |_| {
            stop_tx.try_send(SdkFixtureRequest::Stop).map_err(|_| {
                Box::new(crate::ai::reliability::runtime_closed_failure(
                    sk_protocol::ai_reliability::ProtocolComponent::Provider,
                    "SDK fixture Stop sink full or closed",
                ))
            })
        }))
    });
    let mut control = SdkChatFixtureControl {
        requests,
        accepted_requests: Vec::new(),
        sink_requests: Vec::new(),
        stop_requests: 0,
        next_response: 0,
    };
    if id != "sdk-chat.empty" {
        drive_sdk_chat_fixture(
            &prompt,
            &mut control,
            SdkChatFixtureAction::Submit("Fixture request".into()),
            cx,
        )?;
        let message_id = prompt
            .read(cx)
            .current_stream_message_id()
            .ok_or_else(|| anyhow::anyhow!("sdk_fixture_stream_missing"))?
            .to_string();
        if id == "sdk-chat.retryable-failure" {
            drive_sdk_chat_fixture(
                &prompt,
                &mut control,
                SdkChatFixtureAction::Fail { message_id },
                cx,
            )?;
        } else {
            drive_sdk_chat_fixture(
                &prompt,
                &mut control,
                SdkChatFixtureAction::Text {
                    message_id,
                    text: "Streaming fixture response".into(),
                },
                cx,
            )?;
        }
    }
    Ok((prompt, control))
}

pub(crate) fn drive_sdk_chat_fixture(
    prompt: &Entity<crate::prompts::ChatPrompt>,
    control: &mut SdkChatFixtureControl,
    action: SdkChatFixtureAction,
    cx: &mut App,
) -> anyhow::Result<()> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    match action {
        SdkChatFixtureAction::Submit(text) => {
            anyhow::ensure!(
                !text.trim().is_empty() && text.len() <= 64 * 1024,
                "invalid_sdk_fixture_text"
            );
            prompt.update(cx, |prompt, cx| {
                prompt.set_input(text, cx);
                prompt.submit(cx);
            });
        }
        SdkChatFixtureAction::Retry => {
            let result =
                prompt.update(cx, |prompt, cx| {
                    prompt.execute_conversation_command(
                crate::components::conversation_actions::ChatPromptConversationCommand::Retry, cx)
                });
            anyhow::ensure!(
                matches!(
                    result,
                    crate::components::conversation_actions::ConversationCommandExecution::Executed
                ),
                "sdk_retry_not_dispatched"
            );
        }
        SdkChatFixtureAction::Stop => prompt.update(cx, |prompt, cx| prompt.stop_streaming(cx)),
        SdkChatFixtureAction::Text { message_id, text } => {
            anyhow::ensure!(
                prompt.read(cx).current_stream_message_id() == Some(message_id.as_str()),
                "stale_sdk_stream"
            );
            anyhow::ensure!(text.len() <= 64 * 1024, "sdk_fixture_text_limit");
            prompt.update(cx, |prompt, cx| prompt.append_chunk(&message_id, &text, cx));
        }
        SdkChatFixtureAction::Complete { message_id } => {
            anyhow::ensure!(
                prompt.read(cx).current_stream_message_id() == Some(message_id.as_str()),
                "stale_sdk_stream"
            );
            prompt.update(cx, |prompt, cx| prompt.complete_streaming(&message_id, cx));
        }
        SdkChatFixtureAction::Fail { message_id } => {
            anyhow::ensure!(
                prompt.read(cx).current_stream_message_id() == Some(message_id.as_str()),
                "stale_sdk_stream"
            );
            let failure = crate::ai::reliability::runtime_closed_failure(
                sk_protocol::ai_reliability::ProtocolComponent::Provider,
                "controlled SDK stream closed",
            );
            let summary = failure.primary_message().to_string();
            prompt.update(cx, |prompt, cx| {
                prompt.set_message_failure(&message_id, failure.failure, summary, cx)
            });
        }
    }
    drain_sdk_chat_fixture(prompt, control, cx)
}

pub(crate) fn drain_sdk_chat_fixture(
    prompt: &Entity<crate::prompts::ChatPrompt>,
    control: &mut SdkChatFixtureControl,
    cx: &mut App,
) -> anyhow::Result<()> {
    while let Ok(request) = control.requests.try_recv() {
        match request {
            SdkFixtureRequest::Submit(request) | SdkFixtureRequest::Retry(request) => {
                anyhow::ensure!(
                    control.accepted_requests.len() < 32,
                    "sdk_fixture_turn_limit"
                );
                control
                    .accepted_requests
                    .push(request.request_ref().0.clone());
                // Project only the request actually received by the callback sink.
                control.sink_requests.push(serde_json::json!({
                    "requestRef": request.request_ref().0,
                    "promptId": request.prompt_id(),
                    "displayText": request.display_text(),
                    "outboundText": request.outbound_text(),
                    "payloadFingerprint": request.payload_fingerprint().0,
                }));
                control.next_response += 1;
                let response_id = format!("sdk-fixture-response-{}", control.next_response);
                prompt
                    .update(cx, |prompt, cx| {
                        if prompt.accepted_sdk_request().is_some() {
                            prompt.add_message(
                                crate::protocol::ChatPromptMessage::user(request.display_text()),
                                cx,
                            );
                        }
                        prompt.start_sdk_response(request, response_id, cx)
                    })
                    .map_err(anyhow::Error::msg)?;
                crate::runtime_policy::record_completed_fixture_effect();
            }
            SdkFixtureRequest::Stop => control.stop_requests += 1,
        }
    }
    Ok(())
}

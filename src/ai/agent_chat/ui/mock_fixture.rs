//! The existing standard/detached mock connection, shared before host opening.
//! Events enter the real thread stream; fixture installation is not an action receipt.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::{App, AppContext, Entity};

use super::capabilities::AgentChatSessionPolicy;
use super::{AgentChatPermissionBroker, AgentChatThread, AgentChatThreadInit};
use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx};
use crate::ai::agent_chat::runtime::{AgentChatConnection, AgentChatTurnRequest};
use crate::ai::reliability::AiAdapterResult;

const MAX_TURNS: usize = 32;
const MAX_EVENTS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FixtureTurnReceipt {
    pub generation: u64,
    pub thread_id: String,
    pub cancellation_requested: bool,
    /// Sending a local cancellation request is not a provider acknowledgement.
    pub remote_cancellation_acknowledged: bool,
}

struct ActiveFixtureTurn {
    receipt: FixtureTurnReceipt,
    sender: async_channel::Sender<AgentChatEvent>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FixtureDrainReceipt {
    pub stream_generation: u64,
    pub queued: bool,
    pub retained: bool,
    pub released: bool,
    pub callbacks: usize,
    pub stale_rejected: bool,
    pub replacement_stream_generation: u64,
    pub replacement_buffer_unchanged: bool,
    pub replacement_task_present: bool,
    pub replacement_task_preserved: bool,
    pub replacement_transcript_unchanged: bool,
}

struct FixtureDrain {
    receipt: FixtureDrainReceipt,
    sender: async_channel::Sender<()>,
    receiver: Option<async_channel::Receiver<()>>,
    // Retain the actual production task across Stop, as an already queued
    // callback may outlive cancellation. Dropping the fixture still cancels it.
    task: Option<gpui::Task<()>>,
}

#[derive(Default)]
struct FixtureState {
    scripts: VecDeque<Vec<AgentChatEvent>>,
    turns: Vec<ActiveFixtureTurn>,
    permissions: Option<AgentChatPermissionBroker>,
    permission_replies: Vec<async_channel::Receiver<Option<String>>>,
    drains: Vec<FixtureDrain>,
}

#[derive(Clone, Default)]
pub(crate) struct AgentChatFixtureControl(Arc<Mutex<FixtureState>>);

impl AgentChatFixtureControl {
    pub(crate) fn hold_drain(&self, stream_generation: u64) -> anyhow::Result<()> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        anyhow::ensure!(state.drains.len() < MAX_TURNS, "fixture_drain_limit");
        anyhow::ensure!(
            !state
                .drains
                .iter()
                .any(|drain| drain.receipt.stream_generation == stream_generation),
            "fixture_drain_already_held"
        );
        let (sender, receiver) = async_channel::bounded(1);
        state.drains.push(FixtureDrain {
            receipt: FixtureDrainReceipt {
                stream_generation,
                ..Default::default()
            },
            sender,
            receiver: Some(receiver),
            task: None,
        });
        Ok(())
    }

    pub(crate) fn take_drain_gate(
        &self,
        stream_generation: u64,
    ) -> Option<async_channel::Receiver<()>> {
        let mut state = self.0.lock().ok()?;
        let drain = state
            .drains
            .iter_mut()
            .find(|drain| drain.receipt.stream_generation == stream_generation)?;
        let receiver = drain.receiver.take()?;
        drain.receipt.queued = true;
        Some(receiver)
    }

    pub(crate) fn retain_drain(
        &self,
        stream_generation: u64,
        task: gpui::Task<()>,
    ) -> anyhow::Result<()> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        let drain = state
            .drains
            .iter_mut()
            .find(|drain| drain.receipt.stream_generation == stream_generation)
            .ok_or_else(|| anyhow::anyhow!("fixture_drain_not_held"))?;
        anyhow::ensure!(
            drain.receipt.queued && !drain.receipt.released && drain.task.is_none(),
            "fixture_drain_not_retainable"
        );
        drain.task = Some(task);
        drain.receipt.retained = true;
        Ok(())
    }

    pub(crate) fn release_drain(&self, stream_generation: u64) -> anyhow::Result<()> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        let drain = state
            .drains
            .iter_mut()
            .find(|drain| drain.receipt.stream_generation == stream_generation)
            .ok_or_else(|| anyhow::anyhow!("fixture_drain_not_held"))?;
        anyhow::ensure!(
            drain.receipt.queued && !drain.receipt.released,
            "fixture_drain_not_releasable"
        );
        drain
            .sender
            .try_send(())
            .map_err(|_| anyhow::anyhow!("fixture_drain_closed"))?;
        drain.receipt.released = true;
        Ok(())
    }

    pub(crate) fn record_drain_callback(&self, observation: FixtureDrainReceipt) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(drain) = state
                .drains
                .iter_mut()
                .find(|drain| drain.receipt.stream_generation == observation.stream_generation)
            {
                drain.receipt = FixtureDrainReceipt {
                    queued: drain.receipt.queued,
                    retained: drain.receipt.retained,
                    released: drain.receipt.released,
                    callbacks: drain.receipt.callbacks + 1,
                    ..observation
                };
            }
        }
    }

    pub(crate) fn drain_receipts(&self) -> anyhow::Result<Vec<FixtureDrainReceipt>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?
            .drains
            .iter()
            .map(|drain| drain.receipt.clone())
            .collect())
    }

    pub(crate) fn request_permission(
        &self,
        input: super::permission_broker::AgentChatApprovalRequestInput,
    ) -> anyhow::Result<()> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        anyhow::ensure!(
            state.permission_replies.len() < MAX_EVENTS,
            "fixture_permission_limit"
        );
        let reply = state
            .permissions
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("fixture_permission_source_missing"))?
            .try_request(input)?;
        state.permission_replies.push(reply);
        Ok(())
    }

    pub(crate) fn permission_reply_count(&self) -> anyhow::Result<usize> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?
            .permission_replies
            .iter()
            .filter(|reply| !reply.is_empty())
            .count())
    }

    pub(crate) fn queue_turn(&self, events: Vec<AgentChatEvent>) -> anyhow::Result<()> {
        anyhow::ensure!(events.len() <= MAX_EVENTS, "fixture_event_limit");
        let mut state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        anyhow::ensure!(
            state.scripts.len() + state.turns.len() < MAX_TURNS,
            "fixture_turn_limit"
        );
        state.scripts.push_back(events);
        Ok(())
    }

    pub(crate) fn latest_generation(&self) -> anyhow::Result<u64> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?
            .turns
            .last()
            .map(|turn| turn.receipt.generation)
            .ok_or_else(|| anyhow::anyhow!("fixture_turn_missing"))
    }

    /// Exact turn ownership is required even when deliberately delivering an old event.
    pub(crate) fn emit(&self, generation: u64, event: AgentChatEvent) -> anyhow::Result<()> {
        let state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        let turn = state
            .turns
            .iter()
            .find(|turn| turn.receipt.generation == generation)
            .ok_or_else(|| anyhow::anyhow!("fixture_turn_missing"))?;
        turn.sender
            .try_send(event)
            .map_err(|_| anyhow::anyhow!("fixture_stream_full_or_closed"))?;
        Ok(())
    }

    pub(crate) fn close_stream(&self, generation: u64) -> anyhow::Result<()> {
        let state = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        let turn = state
            .turns
            .iter()
            .find(|turn| turn.receipt.generation == generation)
            .ok_or_else(|| anyhow::anyhow!("fixture_turn_missing"))?;
        turn.sender.close();
        Ok(())
    }

    pub(crate) fn receipts(&self) -> anyhow::Result<Vec<FixtureTurnReceipt>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?
            .turns
            .iter()
            .map(|turn| turn.receipt.clone())
            .collect())
    }
}

pub(crate) struct StandardAgentChatMockFixtureConnection {
    control: AgentChatFixtureControl,
}

impl StandardAgentChatMockFixtureConnection {
    pub(crate) fn controlled(
        permissions: Option<AgentChatPermissionBroker>,
    ) -> (Arc<Self>, AgentChatFixtureControl) {
        let control = AgentChatFixtureControl(Arc::new(Mutex::new(FixtureState {
            permissions,
            ..FixtureState::default()
        })));
        (
            Arc::new(Self {
                control: control.clone(),
            }),
            control,
        )
    }
}

impl AgentChatConnection for StandardAgentChatMockFixtureConnection {
    fn fixture_control(&self) -> Option<AgentChatFixtureControl> {
        Some(self.control.clone())
    }
    fn is_provider_free_fixture(&self) -> bool {
        true
    }

    fn start_turn(&self, request: AgentChatTurnRequest) -> AiAdapterResult<AgentChatEventRx> {
        let mut state = self
            .control
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        if state.turns.len() >= MAX_TURNS {
            return Err(anyhow::anyhow!("fixture_turn_limit").into());
        }
        let (sender, rx) = async_channel::bounded(MAX_EVENTS);
        if let Some(events) = state.scripts.pop_front() {
            for event in events {
                sender
                    .try_send(event)
                    .map_err(|_| anyhow::anyhow!("fixture_stream_full"))?;
            }
        }
        let generation = state.turns.len() as u64 + 1;
        state.turns.push(ActiveFixtureTurn {
            receipt: FixtureTurnReceipt {
                generation,
                thread_id: request.ui_thread_id,
                cancellation_requested: false,
                remote_cancellation_acknowledged: false,
            },
            sender,
        });
        crate::runtime_policy::record_completed_fixture_effect();
        Ok(rx)
    }

    fn cancel_turn(&self, thread_id: String) -> AiAdapterResult<()> {
        let mut state = self
            .control
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("fixture_poisoned"))?;
        let turn = state
            .turns
            .iter_mut()
            .rev()
            .find(|turn| turn.receipt.thread_id == thread_id)
            .ok_or_else(|| anyhow::anyhow!("fixture_turn_missing"))?;
        turn.receipt.cancellation_requested = true;
        Ok(())
    }

    fn prepare_session(
        &self,
        _thread_id: String,
        _cwd: PathBuf,
    ) -> AiAdapterResult<AgentChatEventRx> {
        let (tx, rx) = async_channel::bounded(1);
        drop(tx);
        Ok(rx)
    }
}

pub(crate) fn create_mock_fixture_thread(
    id: &str,
    policy: AgentChatSessionPolicy,
    cx: &mut App,
) -> (Entity<AgentChatThread>, AgentChatFixtureControl) {
    let (broker, permission_rx) = AgentChatPermissionBroker::new();
    let (connection, control) = StandardAgentChatMockFixtureConnection::controlled(Some(broker));
    let cwd = crate::runtime_policy::owned_evaluation()
        .map(|scope| scope.root().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("script-kit-agent-chat-fixture"));
    let thread = cx.new(|cx| {
        AgentChatThread::new(
            connection,
            permission_rx,
            AgentChatThreadInit {
                ui_thread_id: id.to_owned(),
                cwd,
                initial_input: None,
                initial_context_parts: Vec::new(),
                display_name: "Agent Chat".into(),
                profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID.to_string(),
                profile_display_name: Some("Agent Chat".into()),
                profile_icon_name: None,
                selected_agent: None,
                available_agents: Vec::new(),
                launch_requirements: super::AgentChatLaunchRequirements::default(),
                available_models: vec![super::config::AgentChatModelEntry {
                    id: "fixture-model".into(),
                    display_name: Some("Fixture Model".into()),
                    context_window: Some(128_000),
                }],
                selected_model_id: Some("fixture-model".into()),
                session_policy: policy,
            },
            cx,
        )
    });
    thread.update(cx, |thread, cx| thread.mark_context_bootstrap_ready(cx));
    (thread, control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy;

    #[gpui::test]
    fn production_fixture_real_retry_and_stop_ownership(cx: &mut gpui::TestAppContext) {
        let (thread, control) = cx.update(|cx| {
            create_mock_fixture_thread("fixture-recovery", AgentChatSessionPolicy::Full, cx)
        });
        cx.update(|cx| {
            thread.update(cx, |thread, cx| {
                thread.set_input("first request", cx);
                thread.submit_input(cx).unwrap();
            })
        });
        control
            .emit(
                1,
                AgentChatEvent::TurnFailed {
                    failure: crate::ai::reliability::runtime_closed_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Provider,
                        "fixture transport closed",
                    ),
                },
            )
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            thread.update(cx, |thread, cx| {
                assert_eq!(
                    thread.status,
                    super::super::thread::AgentChatThreadStatus::Error
                );
                thread.retry_last_user_turn(cx).unwrap();
            })
        });
        assert_eq!(control.receipts().unwrap().len(), 2);
        control
            .emit(
                2,
                AgentChatEvent::AgentMessageDelta("retried output".into()),
            )
            .unwrap();
        control
            .emit(2, AgentChatEvent::completed("fixture"))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            thread.update(cx, |thread, cx| {
                assert_eq!(
                    thread.status,
                    super::super::thread::AgentChatThreadStatus::Idle
                );
                assert!(thread
                    .messages
                    .iter()
                    .any(|message| message.body.as_ref() == "retried output"));
                thread.set_input("stop request", cx);
                thread.submit_input(cx).unwrap();
                thread.stop_streaming(cx);
                let receipt = thread.last_local_stop().unwrap();
                assert!(receipt.local_stream_cancelled);
                assert!(receipt.remote_cancel_requested);
                assert!(!receipt.remote_cancel_acknowledged);
            })
        });
        let _ = control.emit(
            3,
            AgentChatEvent::AgentMessageDelta("late obsolete output".into()),
        );
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(!thread
                .read(cx)
                .messages
                .iter()
                .any(|message| message.body.contains("late obsolete output")));
        });
    }

    fn request() -> AgentChatTurnRequest {
        AgentChatTurnRequest {
            ui_thread_id: "owned".into(),
            cwd: PathBuf::from("."),
            blocks: Vec::new(),
            model_id: None,
            tool_policy: AgentChatToolPolicy::Full,
        }
    }

    #[test]
    fn fixture_controls_bound_events_and_keep_turn_identity() {
        let (connection, control) = StandardAgentChatMockFixtureConnection::controlled(None);
        let first = connection.start_turn(request()).unwrap();
        connection.cancel_turn("owned".into()).unwrap();
        let second = connection.start_turn(request()).unwrap();
        control
            .emit(1, AgentChatEvent::AgentMessageDelta("old".into()))
            .unwrap();
        assert!(
            matches!(first.try_recv().unwrap(), AgentChatEvent::AgentMessageDelta(text) if text == "old")
        );
        assert!(second.try_recv().is_err());
        assert!(control.receipts().unwrap()[0].cancellation_requested);
        assert!(!control.receipts().unwrap()[0].remote_cancellation_acknowledged);
        for _ in 0..MAX_EVENTS {
            control
                .emit(2, AgentChatEvent::AgentMessageDelta("bounded".into()))
                .unwrap();
        }
        assert!(control
            .emit(2, AgentChatEvent::completed("fixture"))
            .is_err());
        control.close_stream(2).unwrap();
        assert!(control
            .emit(2, AgentChatEvent::completed("fixture"))
            .is_err());
    }
}

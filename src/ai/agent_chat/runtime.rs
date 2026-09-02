use std::path::PathBuf;
use std::sync::Arc;

use crate::ai::agent_chat::content::ContentBlock;
use crate::ai::agent_chat::events::AgentChatEventRx;
use crate::ai::reliability::AiAdapterResult;

#[derive(Debug, Clone)]
pub(crate) struct AgentChatTurnRequest {
    pub ui_thread_id: String,
    pub cwd: PathBuf,
    pub blocks: Vec<ContentBlock>,
    pub model_id: Option<String>,
    /// The tool-admission policy for this turn (WP-B2). Carried alongside the
    /// blocks so the backend adapter configures its allowlist from the
    /// authoritative session policy instead of re-deriving it. Defaults to
    /// [`AgentChatToolPolicy::Full`] for constructions that predate the field.
    pub tool_policy: crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy,
}

pub(crate) struct IsolatedTurnHandle {
    pub rx: AgentChatEventRx,
    pub cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl IsolatedTurnHandle {
    pub(crate) fn signal_cancel(&self) {
        if let Some(flag) = &self.cancel {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub(crate) trait AgentChatConnection: Send + Sync + 'static {
    /// Fixture sources are selected before construction; they never change the
    /// immutable Full/QuickAi product policy.
    fn is_provider_free_fixture(&self) -> bool {
        false
    }
    fn fixture_control(
        &self,
    ) -> Option<crate::ai::agent_chat::ui::mock_fixture::AgentChatFixtureControl> {
        None
    }
    fn start_turn(&self, request: AgentChatTurnRequest) -> AiAdapterResult<AgentChatEventRx>;
    fn start_isolated_turn(
        &self,
        request: AgentChatTurnRequest,
    ) -> AiAdapterResult<IsolatedTurnHandle> {
        let rx = self.start_turn(request)?;
        Ok(IsolatedTurnHandle { rx, cancel: None })
    }
    fn cancel_turn(&self, ui_thread_id: String) -> AiAdapterResult<()>;
    fn prepare_session(
        &self,
        ui_thread_id: String,
        cwd: PathBuf,
    ) -> AiAdapterResult<AgentChatEventRx>;
    /// List the user messages the session can rewind to. Responds with a
    /// `ForkPointsAvailable` event. Backends without checkpointing keep the
    /// default refusal so the UI never advertises a rewind it cannot honor.
    fn fork_points(&self) -> AiAdapterResult<AgentChatEventRx> {
        Err(anyhow::anyhow!("this agent connection does not support rewind").into())
    }
    /// Rewind the live session to just before the given user message entry.
    /// Responds with a `ForkCompleted` event carrying the message text.
    fn fork_to_entry(&self, entry_id: String) -> AiAdapterResult<AgentChatEventRx> {
        let _ = entry_id;
        Err(anyhow::anyhow!("this agent connection does not support rewind").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_chat_connection_trait_is_object_safe() {
        fn accepts_trait_object(_: Option<&dyn AgentChatConnection>) {}
        accepts_trait_object(None);
    }
}

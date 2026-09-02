//! Retention-gated saved transcript restoration, memory ingestion, and auto titles.

use gpui::Context;

use super::{
    AgentChatContextBootstrapState, AgentChatThread, AgentChatThreadMessage,
    AgentChatThreadMessageRole, AgentChatThreadStatus,
};
use crate::ai::agent_chat::ui::history;

fn truncate_chars_for_title_prompt(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push('\u{2026}');
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompletedChatTurnIngest {
    pub(super) thread_id: String,
    pub(super) turn_index: usize,
    pub(super) user_text: String,
    pub(super) assistant_text: String,
    pub(super) trace_label: String,
}

impl AgentChatThread {
    pub(super) fn retains_history(&self) -> bool {
        self.session_policy.allows_automatic_transcript_retention()
            && (!self.is_provider_free_fixture() || crate::runtime_policy::is_owned_evaluation())
    }

    pub(super) fn maybe_spawn_auto_title(&mut self, conversation: &history::SavedConversation) {
        if self.is_provider_free_fixture() || crate::runtime_policy::is_owned_evaluation() {
            return;
        }
        if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider).is_err() {
            return;
        }
        if self.llm_title_attempted || conversation.custom_title.is_some() {
            return;
        }
        if !conversation
            .messages
            .iter()
            .any(|message| message.role.eq_ignore_ascii_case("assistant"))
        {
            return;
        }

        let Some(first_user) = conversation
            .messages
            .iter()
            .find(|message| message.role.eq_ignore_ascii_case("user"))
            .map(|message| message.body.clone())
        else {
            return;
        };
        let Some(first_assistant) = conversation
            .messages
            .iter()
            .find(|message| message.role.eq_ignore_ascii_case("assistant"))
            .map(|message| message.body.clone())
        else {
            return;
        };

        self.llm_title_attempted = true;
        let session_id = conversation.session_id.clone();
        let user_excerpt = truncate_chars_for_title_prompt(&first_user, 400);
        let assistant_excerpt = truncate_chars_for_title_prompt(&first_assistant, 400);

        let spawn_result = std::thread::Builder::new()
            .name("agent_chat-auto-title".to_string())
            .spawn(move || {
                let registry =
                    crate::ai::providers::ProviderRegistry::from_environment_with_config(None);
                if !registry.has_any_provider() {
                    return;
                }

                let result = (|| -> anyhow::Result<()> {
                    let (model, provider) =
                        crate::ai::script_generation::select_generation_model(&registry)?;
                    let messages = vec![
                        crate::ai::providers::ProviderMessage::system(
                            "You title chat conversations. Reply with ONLY a concise 3-6 word title. No quotes, no punctuation at the end.",
                        ),
                        crate::ai::providers::ProviderMessage::user(format!(
                            "User: {user_excerpt}\nAssistant: {assistant_excerpt}"
                        )),
                    ];
                    let raw = provider.send_message(&messages, &model.id)?;
                    let title = history::sanitize_conversation_title(&raw);
                    if title.is_empty() {
                        return Ok(());
                    }
                    history::rename_conversation(&session_id, &title)?;
                    Ok(())
                })();

                if let Err(error) = result {
                    let safe_error =
                        crate::logging::log_private_user_value(&error.to_string());
                    tracing::debug!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_auto_title_failed",
                        session_id = %session_id,
                        error_bytes = safe_error.raw_bytes,
                        error_sha256 = %safe_error.sha256,
                    );
                }
            });

        if let Err(error) = spawn_result {
            let safe_error = crate::logging::log_private_user_value(&error.to_string());
            tracing::debug!(
                target: "script_kit::tab_ai",
                event = "agent_chat_auto_title_spawn_failed",
                session_id = %conversation.session_id,
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
            );
        }
    }

    pub(super) fn completed_chat_turn_ingest(
        &self,
        history_trace_label: Option<String>,
    ) -> Option<CompletedChatTurnIngest> {
        // Zero-retention sessions produce NO automatic memory: Brain ingestion
        // and the day trace are retention, same as the history files (Oracle
        // phase-b-counters-quickai-audit P0 — this ran unconditionally and
        // turned every Quick AI "quick question" into recallable Brain state).
        if !self.session_policy.allows_automatic_transcript_retention()
            || self.is_provider_free_fixture()
            || crate::runtime_policy::is_owned_evaluation()
        {
            return None;
        }
        let user_text = self
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, AgentChatThreadMessageRole::User))
            .map(|m| m.body.to_string())?;
        let assistant_text = self
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, AgentChatThreadMessageRole::Assistant))
            .map(|m| m.body.to_string())
            .unwrap_or_default();
        let trace_label = history_trace_label.unwrap_or_else(|| {
            self.messages
                .iter()
                .find(|m| matches!(m.role, AgentChatThreadMessageRole::User))
                .map(|m| m.body.to_string())
                .unwrap_or_default()
        });
        let turn_index = self
            .messages
            .iter()
            .filter(|m| matches!(m.role, AgentChatThreadMessageRole::User))
            .count()
            .saturating_sub(1);

        Some(CompletedChatTurnIngest {
            thread_id: self.ui_thread_id.clone(),
            turn_index,
            user_text,
            assistant_text,
            trace_label,
        })
    }

    pub(super) fn clear_context_for_saved_messages(&mut self) {
        // Pending context has never been part of the persisted Agent Chat
        // history schema. Fail closed at the reload boundary: neither a stale
        // in-memory draft nor an accepted retry payload can become sendable in
        // the loaded conversation.
        self.clear_all_pending_context("load_saved_messages");
        self.context_receipts.clear();
        self.last_prepared_turn = None;
    }

    /// Load saved messages from a conversation history file.
    /// Replaces current messages with the saved ones (read-only view).
    /// Clears all pending context state so loaded history does not inherit
    /// stale chips from the previous conversation.
    pub(crate) fn load_saved_messages(
        &mut self,
        saved: &[history::SavedMessage],
        cx: &mut Context<Self>,
    ) {
        // Restoring a saved conversation into a zero-retention thread would
        // resurrect retained content the policy forbids. Fail closed — Quick
        // AI never loads history (WP-B1). Full surfaces are unaffected.
        if !self.session_policy.allows_automatic_transcript_retention() {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_saved_message_load_denied_by_policy",
                policy = ?self.session_policy,
                saved_message_count = saved.len(),
            );
            return;
        }
        self.bump_transcript_generation("load_saved_messages");
        self.flush_streaming_text_buffer();
        self.stream_task = None;
        self.stream_started_at = None;
        self.pending_permission = None;
        self.status = AgentChatThreadStatus::Idle;
        self.active_plan_entries.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.standing_approvals.clear();
        self.active_mode_id = None;
        self.available_commands.clear();
        self.usage_tokens = None;
        self.usage_cost_usd = None;
        self.next_message_id = 1;
        self.clear_context_for_saved_messages();
        self.messages.clear();
        for msg in saved {
            let role = match msg.role.as_str() {
                "User" => AgentChatThreadMessageRole::User,
                "Assistant" => AgentChatThreadMessageRole::Assistant,
                "Thought" => AgentChatThreadMessageRole::Thought,
                "Tool" => AgentChatThreadMessageRole::Tool,
                "System" => AgentChatThreadMessageRole::System,
                "Error" => AgentChatThreadMessageRole::Error,
                _ => AgentChatThreadMessageRole::System,
            };
            let id = self.alloc_id();
            self.messages
                .push(AgentChatThreadMessage::new(id, role, msg.body.clone()));
        }
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        self.notify_semantic_change(cx);
    }
}

//! Production prompt completion authority shared by the application and libtest.
use crate::protocol::{Message, SubmitValue};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{SyncSender, TrySendError},
    Arc, Mutex,
};
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptInstance {
    pub id: String,
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub(crate) enum PromptOutcome {
    Submitted(SubmitValue),
    Cancelled,
    Confirmed(bool),
    ChatSubmitted(String),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmissionReceipt {
    pub prompt: PromptInstance,
    pub sequence: u64,
    pub outcome: PromptOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubmissionError {
    WrongPrompt,
    Retired,
    AlreadyCompleted,
    ChannelFull,
    Disconnected,
    Poisoned,
    InvalidInput,
    StorageFailure,
}
impl std::fmt::Display for SubmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SubmissionError {}

pub(crate) trait PromptCompletionSink: Send + Sync {
    fn try_complete(
        &self,
        prompt: &PromptInstance,
        outcome: PromptOutcome,
    ) -> Result<SubmissionReceipt, SubmissionError>;
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptCompletionObservation {
    pub receipt: Option<SubmissionReceipt>,
    pub error: Option<SubmissionError>,
    pub retired: bool,
    pub completed: bool,
    pub chat_submission_count: u64,
    pub semantic_revision: u64,
    #[serde(skip)]
    chat_request_refs: Vec<String>,
}

impl PromptCompletionObservation {
    fn advance_revision(&mut self) {
        self.semantic_revision = self.semantic_revision.strict_add(1);
    }

    fn set_error(&mut self, error: SubmissionError) {
        if self.error != Some(error) {
            self.error = Some(error);
            self.advance_revision();
        }
    }
}

enum CompletionDestination {
    Sdk(Option<SyncSender<Message>>),
    Naming(SyncSender<Option<String>>),
    Local,
}
struct BoundCompletionSink {
    instance: PromptInstance,
    destination: CompletionDestination,
    state: Mutex<PromptCompletionObservation>,
}
impl BoundCompletionSink {
    fn deliver(&self, outcome: &PromptOutcome) -> Result<(), SubmissionError> {
        if let CompletionDestination::Sdk(sender) = &self.destination {
            let sender = sender.as_ref().ok_or(SubmissionError::Disconnected)?;
            let id = self.instance.id.clone();
            let response = match outcome {
                PromptOutcome::Submitted(value) => Message::Submit {
                    id,
                    value: value.to_option_string(),
                },
                PromptOutcome::Cancelled => Message::Submit { id, value: None },
                PromptOutcome::Confirmed(value) => Message::Submit {
                    id,
                    value: Some(value.to_string()),
                },
                PromptOutcome::ChatSubmitted(text) => Message::ChatSubmit {
                    id,
                    text: text.clone(),
                },
            };
            sender.try_send(response).map_err(|error| match error {
                TrySendError::Full(_) => SubmissionError::ChannelFull,
                TrySendError::Disconnected(_) => SubmissionError::Disconnected,
            })?;
        }
        if let CompletionDestination::Naming(sender) = &self.destination {
            let value = match outcome {
                PromptOutcome::Submitted(value) => value.to_option_string(),
                PromptOutcome::Cancelled => None,
                _ => return Err(SubmissionError::InvalidInput),
            };
            sender.try_send(value).map_err(|error| match error {
                TrySendError::Full(_) => SubmissionError::ChannelFull,
                TrySendError::Disconnected(_) => SubmissionError::Disconnected,
            })?;
        }
        Ok(())
    }
}
impl PromptCompletionSink for BoundCompletionSink {
    fn try_complete(
        &self,
        prompt: &PromptInstance,
        outcome: PromptOutcome,
    ) -> Result<SubmissionReceipt, SubmissionError> {
        let mut state = self.state.lock().map_err(|_| SubmissionError::Poisoned)?;
        let result = (|| {
            if *prompt != self.instance {
                return Err(SubmissionError::WrongPrompt);
            }
            if state.retired {
                return Err(SubmissionError::Retired);
            }
            if state.completed {
                return Err(SubmissionError::AlreadyCompleted);
            }
            self.deliver(&outcome)?;
            Ok(SubmissionReceipt {
                prompt: prompt.clone(),
                sequence: state.chat_submission_count + 1,
                outcome,
            })
        })();
        match &result {
            Ok(receipt) => {
                state.receipt = Some(receipt.clone());
                state.error = None;
                state.completed = true;
                state.advance_revision();
                if matches!(self.destination, CompletionDestination::Local) {
                    crate::runtime_policy::record_completed_fixture_effect();
                }
            }
            Err(error) => state.set_error(*error),
        }
        result
    }
}

/// One binding per actual prompt lifetime. Old entity callbacks retain this
/// binding, not whichever prompt happens to be active when they run.
#[derive(Clone)]
pub(crate) struct PromptCompletionBinding {
    sink: Arc<BoundCompletionSink>,
    pub(crate) confirm_lifetime: bool,
}
impl std::fmt::Debug for PromptCompletionBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptCompletionBinding")
            .field("instance", self.instance())
            .finish_non_exhaustive()
    }
}
impl PromptCompletionBinding {
    fn new(id: String, destination: CompletionDestination) -> Self {
        static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
        Self {
            sink: Arc::new(BoundCompletionSink {
                instance: PromptInstance {
                    id,
                    generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
                },
                destination,
                state: Mutex::new(PromptCompletionObservation::default()),
            }),
            confirm_lifetime: false,
        }
    }
    pub(crate) fn sdk(id: String, sender: Option<SyncSender<Message>>) -> Self {
        Self::new(id, CompletionDestination::Sdk(sender))
    }
    pub(crate) fn naming(id: String, sender: SyncSender<Option<String>>) -> Self {
        Self::new(id, CompletionDestination::Naming(sender))
    }
    pub(crate) fn local(id: String) -> Self {
        Self::new(id, CompletionDestination::Local)
    }
    pub(crate) fn instance(&self) -> &PromptInstance {
        &self.sink.instance
    }
    pub(crate) fn is_confirm_lifetime(&self) -> bool {
        self.confirm_lifetime
    }
    pub(crate) fn try_complete(
        &self,
        outcome: PromptOutcome,
    ) -> Result<SubmissionReceipt, SubmissionError> {
        self.sink.try_complete(self.instance(), outcome)
    }
    /// SDK Chat is multi-turn: each accepted immutable request is delivered
    /// once, while cancellation remains a separate terminal completion.
    pub(crate) fn try_chat_submission(
        &self,
        request_ref: &str,
        text: &str,
    ) -> Result<SubmissionReceipt, SubmissionError> {
        let mut state = self
            .sink
            .state
            .lock()
            .map_err(|_| SubmissionError::Poisoned)?;
        let result = (|| {
            if state.retired {
                return Err(SubmissionError::Retired);
            }
            if state.completed
                || state
                    .chat_request_refs
                    .iter()
                    .any(|seen| seen == request_ref)
            {
                return Err(SubmissionError::AlreadyCompleted);
            }
            if request_ref.is_empty() || state.chat_request_refs.len() >= 4096 {
                return Err(SubmissionError::InvalidInput);
            }
            let outcome = PromptOutcome::ChatSubmitted(text.to_owned());
            self.sink.deliver(&outcome)?;
            Ok(SubmissionReceipt {
                prompt: self.instance().clone(),
                sequence: state.chat_submission_count + 1,
                outcome,
            })
        })();
        match &result {
            Ok(receipt) => {
                state.chat_request_refs.push(request_ref.to_owned());
                state.chat_submission_count += 1;
                state.receipt = Some(receipt.clone());
                state.error = None;
                state.advance_revision();
                if matches!(self.sink.destination, CompletionDestination::Local) {
                    crate::runtime_policy::record_completed_fixture_effect();
                }
            }
            Err(error) => state.set_error(*error),
        }
        result
    }
    /// Read the completion owner's epoch without cloning receipt payloads.
    /// Poisoning blocks delivery, not read-only inspection of the retained epoch.
    pub(crate) fn semantic_revision(&self) -> u64 {
        self.sink
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .semantic_revision
    }
    pub(crate) fn observation(&self) -> PromptCompletionObservation {
        self.sink
            .state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(PromptCompletionObservation {
                error: Some(SubmissionError::Poisoned),
                ..Default::default()
            })
    }
    pub(crate) fn record_error(&self, error: SubmissionError) {
        if let Ok(mut state) = self.sink.state.lock() {
            state.set_error(error);
        }
    }
    pub(crate) fn chat_submit_callback(&self) -> crate::prompts::ChatSubmitCallback {
        let binding = self.clone();
        Arc::new(move |request| {
            if request.prompt_id() != binding.instance().id {
                return Err("stale_chat_prompt".into());
            }
            let missing = matches!(&binding.sink.destination, CompletionDestination::Sdk(None));
            binding
                .try_chat_submission(&request.request_ref().0, request.outbound_text())
                .map(|_| ())
                .map_err(|error| match error {
                    SubmissionError::Disconnected if missing => {
                        "sdk_response_channel_missing".to_string()
                    }
                    SubmissionError::ChannelFull | SubmissionError::Disconnected => {
                        "sdk_response_channel_full_or_closed".to_string()
                    }
                    other => other.to_string(),
                })
        })
    }
    pub(crate) fn retire(&self) {
        if let Ok(mut state) = self.sink.state.lock() {
            if !state.retired {
                state.retired = true;
                state.advance_revision();
            }
        }
    }
    pub(crate) fn submit_callback(&self) -> crate::prompts::SubmitCallback {
        let binding = self.clone();
        Arc::new(move |id, value| {
            let outcome = value.map_or(PromptOutcome::Cancelled, |value| {
                PromptOutcome::Submitted(SubmitValue::Text(value))
            });
            let mut instance = binding.instance().clone();
            instance.id = id;
            if let Err(error) = binding.sink.try_complete(&instance, outcome) {
                tracing::warn!(%error, "Prompt completion refused");
            }
        })
    }
}

/// Equality includes every wire field: only the registered provider-free Arg
/// entrypoint may enter production prompt transport from the coordinator.
#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub(crate) fn reviewed_sdk_message(
    fixture: crate::protocol::SdkPromptFixtureId,
    message: serde_json::Value,
) -> anyhow::Result<(String, Message)> {
    use crate::protocol::SdkPromptFixtureId;
    use anyhow::{ensure, Context as _};
    use serde_json::{json, Value};
    ensure!(
        fixture == SdkPromptFixtureId::ArgRoundtrip,
        "sdk_fixture_not_registered"
    );
    let id = message
        .get("id")
        .and_then(Value::as_str)
        .context("sdk_prompt_id_missing")?;
    ensure!(id == "1", "sdk_fixture_prompt_id_mismatch");
    let expected = json!({"type":"arg","id":"1","placeholder":"Owned SDK completion",
        "choices":[{"name":"SDK first","value":"sdk-first"},{"name":"SDK second","value":"sdk-second"}]});
    ensure!(message == expected, "sdk_fixture_message_mismatch");
    Ok((id.to_owned(), serde_json::from_value(message)?))
}

#[cfg(test)]
mod completion_epoch_tests {
    use super::*;

    #[test]
    fn poisoned_completion_retains_readable_identity_and_refuses_delivery() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let binding = PromptCompletionBinding::sdk("poisoned".into(), Some(sender));
        let revision = binding.semantic_revision();
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Ok(_state) = binding.sink.state.lock() else {
                return;
            };
            panic!("poison completion fixture");
        }));
        assert!(poisoned.is_err());
        assert_eq!(binding.semantic_revision(), revision);
        assert!(binding.sink.state.is_poisoned());
        assert_eq!(binding.observation().error, Some(SubmissionError::Poisoned));
        assert_eq!(
            binding.try_complete(PromptOutcome::Cancelled),
            Err(SubmissionError::Poisoned)
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn delivery_error_retry_and_retirement_advance_only_changed_authority() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        sender
            .try_send(Message::Submit {
                id: "occupied".into(),
                value: None,
            })
            .unwrap();
        let binding = PromptCompletionBinding::sdk("epoch".into(), Some(sender));
        let initial = binding.semantic_revision();
        let outcome = PromptOutcome::Submitted(SubmitValue::Text("chosen".into()));
        assert_eq!(
            binding.try_complete(outcome.clone()),
            Err(SubmissionError::ChannelFull)
        );
        let full = binding.semantic_revision();
        assert!(full > initial);
        assert!(!binding.observation().completed);
        assert_eq!(
            binding.try_complete(outcome.clone()),
            Err(SubmissionError::ChannelFull)
        );
        assert_eq!(binding.semantic_revision(), full);
        receiver.try_recv().unwrap();
        assert_eq!(binding.try_complete(outcome.clone()).unwrap().sequence, 1);
        let delivered = binding.semantic_revision();
        assert!(delivered > full);
        assert!(
            matches!(receiver.try_recv().unwrap(), Message::Submit { id, value: Some(value) } if id == "epoch" && value == "chosen")
        );
        assert_eq!(
            binding.try_complete(outcome),
            Err(SubmissionError::AlreadyCompleted)
        );
        assert!(receiver.try_recv().is_err());
        let duplicate = binding.semantic_revision();
        assert!(duplicate > delivered);
        binding.retire();
        assert!(binding.semantic_revision() > duplicate);
        let retired = binding.semantic_revision();
        binding.retire();
        assert_eq!(binding.semantic_revision(), retired);
    }

    #[test]
    fn chat_completion_epochs_distinguish_equal_text_and_disconnected_delivery() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(2);
        let binding = PromptCompletionBinding::sdk("chat-epoch".into(), Some(sender));
        binding.try_chat_submission("first", "same text").unwrap();
        let first = binding.semantic_revision();
        binding.try_chat_submission("second", "same text").unwrap();
        assert!(binding.semantic_revision() > first);
        assert_eq!(binding.observation().chat_submission_count, 2);
        assert_eq!(binding.observation().receipt.unwrap().sequence, 2);
        receiver.try_recv().unwrap();
        receiver.try_recv().unwrap();
        drop(receiver);
        let before_disconnect = binding.semantic_revision();
        assert_eq!(
            binding.try_chat_submission("third", "same text"),
            Err(SubmissionError::Disconnected)
        );
        assert!(binding.semantic_revision() > before_disconnect);
        assert_eq!(binding.observation().chat_submission_count, 2);
    }

    #[test]
    fn disconnected_prompt_completion_never_claims_delivery() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        drop(receiver);
        let binding = PromptCompletionBinding::sdk("1".into(), Some(sender));
        assert_eq!(
            binding.try_complete(PromptOutcome::Cancelled),
            Err(SubmissionError::Disconnected)
        );
        assert!(!binding.observation().completed);
        assert!(binding.observation().receipt.is_none());
    }

    #[test]
    fn cancellation_serializes_null_once_and_retirement_revokes_callbacks() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let binding = PromptCompletionBinding::sdk("1".into(), Some(sender));
        let callback = binding.submit_callback();
        callback("1".into(), None);
        callback("1".into(), None);
        assert_eq!(
            serde_json::to_value(receiver.try_recv().unwrap()).unwrap(),
            serde_json::json!({"type":"submit","id":"1","value":null})
        );
        assert!(receiver.try_recv().is_err());
        binding.retire();
        assert_eq!(
            binding.try_complete(PromptOutcome::Cancelled),
            Err(SubmissionError::Retired)
        );
    }

    #[test]
    fn registered_message_refuses_extra_effects_and_altered_payloads() {
        use crate::protocol::SdkPromptFixtureId;
        use serde_json::json;
        let message = json!({"type":"arg","id":"1","placeholder":"Owned SDK completion",
            "choices":[{"name":"SDK first","value":"sdk-first"},{"name":"SDK second","value":"sdk-second"}]});
        assert!(reviewed_sdk_message(SdkPromptFixtureId::ArgRoundtrip, message.clone()).is_ok());
        let mut extra = message.clone();
        extra["actions"] = json!([]);
        assert!(reviewed_sdk_message(SdkPromptFixtureId::ArgRoundtrip, extra).is_err());
        let mut wrong = message;
        wrong["id"] = json!("2");
        assert!(reviewed_sdk_message(SdkPromptFixtureId::ArgRoundtrip, wrong).is_err());
        assert!(serde_json::from_value::<SdkPromptFixtureId>(json!("arbitrary-script")).is_err());
    }
}

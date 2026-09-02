//! A fixed coordinator-owned SDK fixture enters the ordinary JSONL prompt route.
//! The evaluator never executes a process and never replaces the RPC sender.
use std::sync::mpsc::{self, Receiver};

use anyhow::{ensure, Context as _, Result};
use serde_json::{json, Value};

use super::runtime::{Evaluator, RootOwner};
use crate::prompt_completion::{reviewed_sdk_message, PromptCompletionBinding};
use crate::protocol::{
    AutomationTargetIdentitySnapshot, AutomationWindowTarget, Message, SdkCompletionChannel,
    SdkPromptCommand,
};

const BACKPRESSURE_ID: &str = "owned-sdk-channel-capacity";

pub(super) struct SdkPromptControl {
    binding: PromptCompletionBinding,
    receiver: Option<Receiver<Message>>,
    capacity_held: bool,
    forwarded: usize,
}

impl Drop for SdkPromptControl {
    fn drop(&mut self) {
        self.binding.retire();
    }
}

impl SdkPromptControl {
    fn new(id: String, channel: SdkCompletionChannel) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let capacity_held = channel == SdkCompletionChannel::Full;
        if capacity_held {
            sender.try_send(Message::Submit {
                id: BACKPRESSURE_ID.into(),
                value: None,
            })?;
        }
        let receiver = if channel == SdkCompletionChannel::Disconnected {
            drop(receiver);
            None
        } else {
            Some(receiver)
        };
        Ok(Self {
            binding: PromptCompletionBinding::sdk(id, Some(sender)),
            receiver,
            capacity_held,
            forwarded: 0,
        })
    }

    fn release_capacity(&mut self) -> Result<()> {
        ensure!(self.capacity_held, "sdk_capacity_not_held");
        let value = self
            .receiver
            .as_ref()
            .context("sdk_channel_disconnected")?
            .try_recv()?;
        ensure!(
            matches!(value, Message::Submit { ref id, value: None } if id == BACKPRESSURE_ID),
            "sdk_capacity_marker_mismatch"
        );
        self.capacity_held = false;
        Ok(())
    }

    fn drain(&mut self) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        if !self.capacity_held {
            if let Some(receiver) = &self.receiver {
                while let Ok(message) = receiver.try_recv() {
                    ensure!(
                        matches!(&message, Message::Submit { id, .. } if id == &self.binding.instance().id),
                        "sdk_completion_identity_mismatch"
                    );
                    self.forwarded += 1;
                    ensure!(self.forwarded == 1, "sdk_duplicate_completion");
                    messages.push(message);
                }
            }
        }
        Ok(messages)
    }
}

impl Evaluator {
    pub(super) fn sdk_prompt(
        &mut self,
        target: &AutomationWindowTarget,
        expected: &AutomationTargetIdentitySnapshot,
        command: SdkPromptCommand,
    ) -> Result<Value> {
        ensure!(
            self.bootstrap.fixture_ids.contains("sdk.arg-roundtrip.v1"),
            "sdk_fixture_outside_launch_subset"
        );
        self.validate_expected(target, expected)?;
        let mounted = self.resolve(target)?.clone();
        ensure!(
            mounted.fixture_id == "main.script-list",
            "sdk_fixture_root_mismatch"
        );
        let RootOwner::Main(entity) = &mounted.owner else {
            anyhow::bail!("sdk_main_root_required");
        };
        let key = mounted.info.id.clone();
        let mut messages = Vec::new();
        match command {
            SdkPromptCommand::Begin {
                fixture_id,
                message,
                channel,
            } => {
                ensure!(
                    !self.sdk_prompt_controls.contains_key(&key),
                    "sdk_prompt_already_bound"
                );
                let (id, message) = reviewed_sdk_message(fixture_id, message)?;
                let control = SdkPromptControl::new(id.clone(), channel)?;
                mounted.handle.update(&mut **self.cx.app.borrow_mut(), |_, window, cx| {
                    entity.update(cx, |app, cx| -> Result<()> {
                        app.handle_stdin_protocol_message(message, window, cx);
                        ensure!(matches!(&app.current_view, crate::AppView::ArgPrompt { id: current, .. } if current == &id),
                            "sdk_production_prompt_not_mounted");
                        // Arg's production submit owner reads this binding at the
                        // event boundary; it has no captured leaf callback. Install
                        // before returning to the event loop, never borrow RPCs.
                        if let Some(previous) = app.prompt_completion.replace(control.binding.clone()) {
                            previous.retire();
                        }
                        app.mark_main_data_changed();
                        cx.notify();
                        Ok(())
                    })
                })??;
                self.sdk_prompt_controls.insert(key.clone(), control);
            }
            SdkPromptCommand::Drain {} => {
                messages = self
                    .sdk_prompt_controls
                    .get_mut(&key)
                    .context("sdk_prompt_not_bound")?
                    .drain()?;
            }
            SdkPromptCommand::ReleaseCapacity {} => {
                self.sdk_prompt_controls
                    .get_mut(&key)
                    .context("sdk_prompt_not_bound")?
                    .release_capacity()?;
            }
            SdkPromptCommand::Close {} => {
                let control = self
                    .sdk_prompt_controls
                    .remove(&key)
                    .context("sdk_prompt_not_bound")?;
                control.binding.retire();
                return Ok(json!({"operation":"sdkPrompt","ok":true,"closed":true,
                    "completion":control.binding.observation(),"forwarded":control.forwarded,"messages":[]}));
            }
        }
        let control = self
            .sdk_prompt_controls
            .get(&key)
            .context("sdk_prompt_not_bound")?;
        Ok(
            json!({"operation":"sdkPrompt","ok":true,"fixtureId":"sdk.arg-roundtrip.v1",
            "completion":control.binding.observation(),"capacityHeld":control.capacity_held,
            "forwarded":control.forwarded,"messages":messages}),
        )
    }
}

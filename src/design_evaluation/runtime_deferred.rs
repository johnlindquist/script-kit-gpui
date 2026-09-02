use super::runtime::Evaluator;
use crate::protocol::AutomationWindowTarget;
use anyhow::{ensure, Context as _, Result};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

impl Evaluator {
    /// Negative control over the ordinary stdin handler, not the evaluator's action path.
    pub(super) fn probe_deferred_dispatch(
        &mut self,
        target: &AutomationWindowTarget,
    ) -> Result<Value> {
        self.tick(true)?;
        let mounted = self.resolve(target)?.clone();
        let identity_before = self.identity(target)?;
        let before = self.fixture_observation(&mounted)?;
        let prefix = uuid::Uuid::new_v4().to_string();
        let cancel_id = format!("deferred-cancel-{prefix}");
        let event = json!({"type":"keyDown","key":"escape","modifiers":[]});
        let now = || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        };
        let request = json!({"type":"simulateGpuiEvent","requestId":cancel_id,
            "target":target,"event":event,"deadlineUnixMs":now()+5_000});
        ensure!(
            self.response_receiver.try_recv().is_err(),
            "deferred_probe_response_queue_not_empty"
        );
        self.enqueue_main(&request)?;
        ensure!(
            self.response_receiver.try_recv().is_err(),
            "scheduled_ack_must_not_be_terminal"
        );
        self.enqueue_main(&json!({"type":"cancelGpuiEvent","requestId":cancel_id}))?;
        let cancelled = self.collect_deferred_terminal(&cancel_id)?;
        ensure!(
            cancelled["success"] == false
                && cancelled["errorCode"] == "dispatch_cancelled"
                && cancelled["dispatchCompleted"] == false
                && cancelled["dispatchScheduled"] == false,
            "deferred_cancellation_terminal_invalid"
        );
        // Reuse cannot execute or produce another terminal reply, even after completion.
        self.enqueue_main(&request)?;
        self.enqueue_main(&json!({"type":"cancelGpuiEvent","requestId":cancel_id}))?;
        self.pump_deferred_callbacks()?;
        ensure!(
            self.response_receiver.try_recv().is_err(),
            "duplicate_terminal_response"
        );
        ensure!(
            self.identity(target)? == identity_before
                && self.fixture_observation(&mounted)? == before,
            "cancelled_dispatch_mutated_owner"
        );

        let expired_id = format!("deferred-expired-{prefix}");
        self.enqueue_main(&json!({"type":"simulateGpuiEvent","requestId":expired_id,
            "target":target,"event":event,"deadlineUnixMs":now()+100}))?;
        // Stall the foreground effect until its deadline, without delivering
        // input or depending on an artificial dispatch timer.
        self.cx.pump_owned_work(0, Duration::from_millis(200), 0)?;
        let expired = self.collect_deferred_terminal(&expired_id)?;
        ensure!(
            expired["success"] == false
                && expired["errorCode"] == "dispatch_deadline_exceeded"
                && expired["dispatchCompleted"] == false
                && expired["dispatchScheduled"] == false,
            "deferred_deadline_terminal_invalid"
        );
        self.pump_deferred_callbacks()?;
        ensure!(
            self.response_receiver.try_recv().is_err(),
            "duplicate_terminal_response"
        );
        let stale_id = format!("deferred-stale-{prefix}");
        let mut stale_expected = self.identity(target)?;
        stale_expected.data_generation = stale_expected
            .data_generation
            .checked_add(1)
            .context("probe_revision_exhausted")?;
        self.enqueue_main(&json!({"type":"simulateGpuiEvent","requestId":stale_id,
            "target":target,"event":event,"expected":stale_expected,"deadlineUnixMs":now()+5_000}))?;
        let stale = self.collect_deferred_terminal(&stale_id)?;
        ensure!(
            stale["success"] == false
                && stale["errorCode"] == "stale_target_identity"
                && stale["dispatchCompleted"] == false,
            "queued_expectation_not_revalidated"
        );
        let after = self.fixture_observation(&mounted)?;
        let identity_after = self.identity(target)?;
        ensure!(
            identity_after == identity_before && after == before,
            "expired_dispatch_mutated_owner"
        );
        let batch_id = format!("deferred-batch-{prefix}");
        let batch_request = json!({"type":"batch","requestId":batch_id,"target":target,
            "commands":[{"type":"waitFor","condition":{"type":"stateMatch","state":{"promptType":"__no_such_production_prompt__"}},
                "timeout":10_000,"pollInterval":5_000}],
            "options":{"stopOnError":true,"rollbackOnError":false,"timeout":25},"trace":"off"});
        let batch = self.forward_main(&batch_id, &batch_request, true)?;
        ensure!(
            batch["success"] == false
                && batch["results"][0]["error"]["code"] == "wait_condition_timeout"
                && batch["totalElapsed"]
                    .as_u64()
                    .is_some_and(|elapsed| elapsed < 1_000),
            "ordinary_batch_wait_escaped_deadline"
        );
        self.enqueue_main(&batch_request)?;
        self.pump_deferred_callbacks()?;
        ensure!(
            self.response_receiver.try_recv().is_err(),
            "batch_replay_emitted_second_terminal"
        );
        let forged_selection = if mounted.fixture_id == "main.script-list" {
            let elements_id = format!("deferred-elements-{prefix}");
            let elements = self.forward_main(
                &elements_id,
                &json!({"type":"getElements","requestId":elements_id,"target":target,"limit":1000}),
                true,
            )?;
            let semantic_id = elements["elements"]
                .as_array()
                .context("main_elements_missing")?
                .iter()
                .find(|element| element["type"] == "choice" && element["selectable"] != false)
                .and_then(|element| element["semanticId"].as_str())
                .context("main_choice_missing")?;
            let before = self.fixture_observation(&mounted)?;
            let revision_before = self.identity(target)?.data_generation;
            let forged_id = format!("deferred-forged-{prefix}");
            let reply = self.forward_main(&forged_id, &json!({"type":"batch","requestId":forged_id,"target":target,
                "commands":[{"type":"selectBySemanticId","semanticId":format!("{semantic_id}:forged"),"submit":false}]}), true)?;
            ensure!(
                reply["success"] == false
                    && self.identity(target)?.data_generation == revision_before
                    && self.fixture_observation(&mounted)? == before,
                "forged_semantic_suffix_selected_real_row"
            );
            Some(reply)
        } else {
            None
        };
        Ok(
            json!({"owner":"ScriptListApp::handle_stdin_protocol_message",
            "producer":"gpui_event_simulator::dispatch_gpui_event",
            "cancelled":cancelled,"expired":expired,"staleExpectation":stale,"batchWait":batch,
            "cancelTerminalReplies":1,"deadlineTerminalReplies":1,"staleTerminalReplies":1,
            "batchTerminalReplies":1,"batchReplayReplies":0,"forgedSelection":forged_selection,
            "duplicateTerminalReplies":0,"ownerUnchanged":before==after,
            "identityUnchanged":identity_before==identity_after}),
        )
    }

    fn pump_deferred_callbacks(&mut self) -> Result<()> {
        for _ in 0..8 {
            self.cx.pump_owned_work(256, Duration::from_millis(2), 0)?;
        }
        Ok(())
    }

    fn collect_deferred_terminal(&mut self, request_id: &str) -> Result<Value> {
        self.pump_deferred_callbacks()?;
        let response = self
            .response_receiver
            .try_recv()
            .context("deferred_terminal_response_missing")?;
        let response = serde_json::to_value(response)?;
        ensure!(
            response["requestId"].as_str() == Some(request_id)
                && response["type"] == "simulateGpuiEventResult",
            "deferred_terminal_correlation_mismatch"
        );
        ensure!(
            self.response_receiver.try_recv().is_err(),
            "duplicate_terminal_response"
        );
        Ok(response)
    }
}

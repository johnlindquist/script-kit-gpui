//! `flowUx` automation-state payload (docs/ai/flow-ux-protocol.md §6).
//!
//! Devtools probes assert against this snapshot for every red/green receipt.
//! Redaction rule: password input values never appear here — the registry
//! never stores them, so this stays true by construction; the probe suite
//! still asserts it end to end.

use serde_json::{json, Value};

use super::catalog::RosterEntry;
use super::model::FlowUxVariant;
use super::run_registry::flow_run_registry;

pub struct FlowUxSnapshotInputs<'a> {
    pub active_variant: Option<FlowUxVariant>,
    pub selected_flow_id: Option<&'a str>,
    pub roster: Option<(&'a RosterEntry, &'a str)>,
    pub preview: Option<PreviewSnapshot<'a>>,
    pub manager_visible: bool,
    pub manager_focused_run_id: Option<u64>,
    /// Conversational sessions (Conversation Desk). Metadata only — the PTY
    /// entities live on the app.
    pub sessions: Vec<SessionSnapshot>,
}

pub struct SessionSnapshot {
    pub id: u64,
    pub flow_id: String,
    pub flow_name: String,
    pub state: &'static str,
    pub live: bool,
    pub elapsed_ms: u64,
    /// Committed conversation turns (user + assistant pairs).
    pub turns: usize,
    /// True while a turn is in flight on the session's transport.
    pub turn_in_flight: bool,
    /// `codexThread` or `mdflowTurns`.
    pub transport: &'static str,
    /// Engine label as shown in the session chip (e.g. `codex · gpt-5.6-luna`).
    pub engine: String,
    /// Reducer-owned reliability phase tag (S09), camelCase — e.g. `ready`,
    /// `running`, `awaitingRecovery`.
    pub reliability_phase: String,
    /// Stable failure code when the session awaits recovery.
    pub failure_code: Option<String>,
    /// Safe persisted failure summary of the most recent failed turn.
    pub last_failure_summary: Option<String>,
}

pub struct PreviewSnapshot<'a> {
    pub flow_id: &'a str,
    pub fingerprint: Option<&'a str>,
    pub valid: bool,
}

/// Build the `flowUx` JSON value merged into the devtools getState snapshot.
pub fn flow_ux_state(inputs: FlowUxSnapshotInputs<'_>) -> Value {
    let registry = flow_run_registry();
    let selected = registry.selected_id();
    let runs: Vec<Value> = registry
        .snapshot()
        .iter()
        .map(|run| {
            json!({
                "runId": run
                    .protocol_run_id
                    .clone()
                    .unwrap_or_else(|| format!("local-{}", run.local_id)),
                "localId": run.local_id,
                "flowId": run.flow_id,
                "flowName": run.flow_name,
                "variant": run.variant.automation_id(),
                "phase": run.phase.label(),
                "engagement": run.engagement.label(),
                "selected": Some(run.local_id) == selected,
                "exitCode": run.exit_code,
                "errorMessage": run
                    .failure
                    .as_ref()
                    .map(|failure| failure.primary_message()),
                // pgid of the app-spawned `md` (killpg target) + the
                // engine pid mdflow reported — receipts verify OS-level
                // process-group death, not just registry phase.
                "pid": run.pid,
                "enginePid": run.engine_pid,
                "overrideNames": run.override_names,
                "outputTail": run.last_output_line(),
                "outputLineCount": run.stdout_tail.line_count() + run.stderr_tail.line_count(),
                "steps": run
                    .steps
                    .iter()
                    .map(|(id, step)| {
                        json!({
                            "stepId": id,
                            "completed": step.completed,
                            "exitCode": step.exit_code,
                            "cached": step.cached,
                        })
                    })
                    .collect::<Vec<_>>(),
                "elapsedMs": run.elapsed_ms(),
                "launchAckMs": run.timings.launch_ack_ms,
                "spawnMs": run.timings.spawn_ms,
                "firstOutputMs": run.timings.first_output_ms,
            })
        })
        .collect();

    json!({
        "activeVariant": inputs.active_variant.map(|v| v.automation_id()),
        "selectedFlowId": inputs.selected_flow_id,
        "roster": inputs.roster.map(|(entry, cwd)| {
            json!({
                "status": entry.status.automation_label(),
                "count": entry.flows.len(),
                "cwd": cwd,
                "warnings": entry.warnings,
            })
        }),
        "preview": inputs.preview.map(|p| {
            json!({
                "flowId": p.flow_id,
                "fingerprint": p.fingerprint,
                "valid": p.valid,
            })
        }),
        "runs": runs,
        "sessions": inputs
            .sessions
            .iter()
            .map(|s| {
                json!({
                    "sessionId": s.id,
                    "flowId": s.flow_id,
                    "flowName": s.flow_name,
                    "state": s.state,
                    "live": s.live,
                    "elapsedMs": s.elapsed_ms,
                    "turns": s.turns,
                    "turnInFlight": s.turn_in_flight,
                    "transport": s.transport,
                    "engine": s.engine,
                    "reliabilityPhase": s.reliability_phase,
                    "failureCode": s.failure_code,
                    "lastFailureSummary": s.last_failure_summary,
                })
            })
            .collect::<Vec<_>>(),
        "manager": {
            "visible": inputs.manager_visible,
            "focusedRunId": inputs.manager_focused_run_id,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::model::{EngagementMode, FlowUxVariant};

    #[test]
    fn snapshot_shape_matches_protocol_section_six() {
        let registry = flow_run_registry();
        let id = registry.insert_starting(
            "project:snap",
            "snap",
            "/tmp/p/flows/snap.md",
            "/tmp/p",
            FlowUxVariant::Lens,
            EngagementMode::Inline,
        );
        let value = flow_ux_state(FlowUxSnapshotInputs {
            active_variant: Some(FlowUxVariant::Lens),
            selected_flow_id: Some("project:snap"),
            roster: None,
            preview: Some(PreviewSnapshot {
                flow_id: "project:snap",
                fingerprint: Some("sha256:x"),
                valid: true,
            }),
            manager_visible: false,
            manager_focused_run_id: None,
            sessions: vec![SessionSnapshot {
                id: 1,
                flow_id: "package:flow-gmail".into(),
                flow_name: "flow-gmail".into(),
                state: "working",
                live: true,
                elapsed_ms: 5,
                turns: 2,
                turn_in_flight: true,
                transport: "codexThread",
                engine: "codex · gpt-5.6-luna".into(),
                reliability_phase: "running".into(),
                failure_code: None,
                last_failure_summary: None,
            }],
        });
        assert_eq!(value["activeVariant"], "lens");
        assert_eq!(value["preview"]["valid"], true);
        assert_eq!(value["manager"]["visible"], false);
        assert_eq!(value["sessions"][0]["state"], "working");
        assert_eq!(value["sessions"][0]["live"], true);
        assert_eq!(value["sessions"][0]["turns"], 2);
        assert_eq!(value["sessions"][0]["turnInFlight"], true);
        assert_eq!(value["sessions"][0]["transport"], "codexThread");
        let run = value["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["localId"] == id)
            .expect("run appears in snapshot");
        assert_eq!(run["phase"], "Starting");
        assert_eq!(run["engagement"], "Inline");
        assert_eq!(run["variant"], "lens");
    }
}

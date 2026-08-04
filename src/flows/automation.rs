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
    /// Stable engine identity; model is a separate typed field.
    pub engine: String,
    pub model: Option<String>,
    pub model_source: &'static str,
    pub friendly_name: String,
    pub origin: &'static str,
    pub cwd_display: String,
    pub cwd_fingerprint: String,
    pub selection: &'static str,
    pub read_only: bool,
    pub active_thread_fingerprint: String,
    pub selected_thread_fingerprint: String,
    pub parent_thread_fingerprint: Option<String>,
    pub parent_retained: Option<bool>,
    pub inherited_turn_count: usize,
    pub active_turn_count: usize,
    pub selected_turn_count: usize,
    pub archive_count: usize,
    pub thread_count: usize,
    pub total_turn_count: usize,
    pub needs_rethread: bool,
    pub thread_ready: bool,
    pub runtime_generation: u64,
    pub draft_chars: usize,
    pub draft_fingerprint: Option<String>,
    pub draft_generation: u64,
    pub persistence_revision: u64,
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
                "warningCount": entry.warnings.len(),
                "failureCode": entry.failure.as_ref().map(|failure| {
                    format!("{:?}", failure.failure.code)
                }),
                "diagnosticFingerprint": entry
                    .failure
                    .as_ref()
                    .and_then(|failure| failure.failure.diagnostic.as_ref())
                    .map(|diagnostic| diagnostic.fingerprint.0.clone()),
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
                    "model": s.model,
                    "modelSource": s.model_source,
                    "friendlyName": s.friendly_name,
                    "origin": s.origin,
                    "cwdDisplay": s.cwd_display,
                    "cwdFingerprint": s.cwd_fingerprint,
                    "selection": s.selection,
                    "readOnly": s.read_only,
                    "activeThreadFingerprint": s.active_thread_fingerprint,
                    "selectedThreadFingerprint": s.selected_thread_fingerprint,
                    "parentThreadFingerprint": s.parent_thread_fingerprint,
                    "parentRetained": s.parent_retained,
                    "inheritedTurnCount": s.inherited_turn_count,
                    "activeTurnCount": s.active_turn_count,
                    "selectedTurnCount": s.selected_turn_count,
                    "archiveCount": s.archive_count,
                    "threadCount": s.thread_count,
                    "totalTurnCount": s.total_turn_count,
                    "retentionPolicy": "uncappedByApp",
                    "turnCap": serde_json::Value::Null,
                    "needsRethread": s.needs_rethread,
                    "threadReady": s.thread_ready,
                    "runtimeGeneration": s.runtime_generation,
                    "draftChars": s.draft_chars,
                    "draftFingerprint": s.draft_fingerprint,
                    "draftGeneration": s.draft_generation,
                    "persistenceRevision": s.persistence_revision,
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
                engine: "codex".into(),
                model: Some("gpt-5.6-luna".into()),
                model_source: "runtime",
                friendly_name: "Gmail".into(),
                origin: "Package",
                cwd_display: "tmp/p".into(),
                cwd_fingerprint: "cwd-fingerprint".into(),
                selection: "active",
                read_only: false,
                active_thread_fingerprint: "active-thread".into(),
                selected_thread_fingerprint: "active-thread".into(),
                parent_thread_fingerprint: None,
                parent_retained: None,
                inherited_turn_count: 0,
                active_turn_count: 2,
                selected_turn_count: 2,
                archive_count: 0,
                thread_count: 1,
                total_turn_count: 2,
                needs_rethread: false,
                thread_ready: true,
                runtime_generation: 1,
                draft_chars: 0,
                draft_fingerprint: None,
                draft_generation: 0,
                persistence_revision: 2,
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

    #[test]
    fn roster_snapshot_exposes_only_typed_failure_identity() {
        let canary = "PRIVATE_ROSTER_STDERR_CANARY";
        let failure = crate::ai::reliability::process_failure_with_detail(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            crate::ai::reliability::ProcessFailureFacts::ChildExited {
                exit_code: Some(9),
                signal: None,
            },
            canary,
        );
        let roster = crate::flows::catalog::RosterEntry {
            status: crate::flows::catalog::RosterStatus::Error,
            flows: std::sync::Arc::new(Vec::new()),
            warnings: vec![canary.to_string()],
            failure: Some(failure),
            fetched_at: std::time::Instant::now(),
        };
        let value = flow_ux_state(FlowUxSnapshotInputs {
            active_variant: None,
            selected_flow_id: None,
            roster: Some((&roster, "safe-cwd")),
            preview: None,
            manager_visible: false,
            manager_focused_run_id: None,
            sessions: Vec::new(),
        });
        let serialized = serde_json::to_string(&value).expect("snapshot serializes");
        assert!(!serialized.contains(canary));
        assert_eq!(value["roster"]["warningCount"], 1);
        assert_eq!(value["roster"]["failureCode"], "ChildExited");
        assert!(value["roster"]["diagnosticFingerprint"].is_string());
    }
}

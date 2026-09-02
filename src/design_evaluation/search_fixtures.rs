//! Compiled source responses for real launcher provider lifecycles.
//! No fixture accepts grouped rows, selection indices, paths, or executable code.
use crate::RootProviderPublicationPolicy;
use anyhow::{ensure, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::task::{Poll, Waker};
use std::time::{Duration, Instant};

pub(crate) const FIXTURE_ID: &str = "main-search-contract";
const OWNED_DIRECTORY_INPUT: &str = "~/search-contract/";
const OWNED_SLOT_INPUT: &str = "owned-slot";
const SCENARIOS: &[&str] = &[
    "tab-domain-hoist",
    "all-providers",
    "passive-budget",
    "brain-replacement",
    "files-explicit",
    "files-handoff",
    "directory-browse",
    "empty",
    "removal",
    "metadata",
    "replacement",
    "deep-list",
    "error",
    "unavailable",
    "disconnect",
    "cohort-removal-replacement",
    "owner-retirement",
    "eligibility",
    "eligibility-portal",
];

#[derive(serde::Deserialize)]
struct SentenceScenario {
    id: String,
    input: String,
}

static SENTENCE_SCENARIOS: LazyLock<Vec<SentenceScenario>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("search_sentences.json"))
        .expect("compiled sentence search scenarios")
});

fn scenario_ids() -> impl Iterator<Item = &'static str> {
    SCENARIOS.iter().copied().chain(
        SENTENCE_SCENARIOS
            .iter()
            .map(|scenario| scenario.id.as_str()),
    )
}

fn sentence_input(scenario: &str) -> Option<&'static str> {
    SENTENCE_SCENARIOS
        .iter()
        .find(|candidate| candidate.id == scenario)
        .map(|candidate| candidate.input.as_str())
}
pub(crate) const PROVIDERS: &[&str] = &[
    "files",
    "directory",
    "brain-lexical",
    "brain-semantic",
    "tabs",
    "history",
    "windows",
    "icons",
    "notes",
    "todos",
    "clipboard",
    "dictation",
    "conversations",
    "spine",
    "brain-inbox",
    "scripts",
    "apps",
    "skills",
    "validation",
    "flow-roster",
];
const SYNCHRONOUS_PROVIDERS: &[&str] = &["brain-lexical", "brain-inbox"];
const MAX_RUNS: usize = 128;
const MAX_ADVANCE_MS: u32 = 1_000;
const MAX_LOGICAL_MS: u64 = 600_000;
const PREVIEW_COMPLETION_DELAY_MS: u64 = 64;
const OWNER_RETIREMENT_DELIVERY_DELAY_MS: u64 = 64;
const MAX_PENDING_PREVIEW_COMPLETIONS: usize = 128;
static NEXT_RUN: AtomicU64 = AtomicU64::new(1);
static NEXT_PREVIEW_COMPLETION: AtomicU64 = AtomicU64::new(1);
static LOGICAL_TIME_MS: AtomicU64 = AtomicU64::new(0);
// Unpinned Notes fixtures contain 64 characters and precede the held clock by one hour.
// Keep this expectation independent of the renderer and use its process-private hash key.
static EXPECTED_NOTE_SUBTITLE_FINGERPRINT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "sha256:{}",
        crate::logging::log_private_user_value("Updated 1 hour ago · 64 chars").sha256
    )
});

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourcePlan {
    source: &'static str,
    input: String,
    scope: &'static str,
    work_kind: &'static str,
}

pub(crate) fn source_plans(scenario: &str) -> Result<Vec<SourcePlan>> {
    ensure!(
        scenario_ids().any(|id| id == scenario),
        "unknown_search_scenario"
    );
    // Validate owned HOME, but let the production parser see the tilde spelling.
    owned_source_root()?;
    let mut seen = std::collections::HashSet::with_capacity(PROVIDERS.len());
    PROVIDERS
        .iter()
        .map(|&source| {
            ensure!(seen.insert(source), "duplicate_search_source_plan");
            let (input, scope, work_kind) = match source {
                "directory" => (
                    format!("files: {OWNED_DIRECTORY_INPUT}"),
                    "directory",
                    "query-bound",
                ),
                "spine" => (
                    if scenario == "empty" {
                        "@file:"
                    } else {
                        "@file:example.invalid"
                    }
                    .into(),
                    "spine",
                    "query-bound",
                ),
                "brain-lexical" => ("example.invalid".into(), "root", "synchronous"),
                "brain-inbox" => (String::new(), "root", "synchronous"),
                "windows" => ("windows: example.invalid".into(), "root", "query-bound"),
                "icons" => ("windows: example.invalid".into(), "root", "catalogue"),
                "scripts" if matches!(scenario, "eligibility" | "eligibility-portal") => {
                    (OWNED_SLOT_INPUT.into(), "root", "catalogue")
                }
                "scripts" | "apps" | "skills" | "validation" | "flow-roster" => {
                    ("example.invalid".into(), "root", "catalogue")
                }
                "files" | "brain-semantic" | "tabs" | "history" | "notes" | "todos"
                | "clipboard" | "dictation" | "conversations" => {
                    ("example.invalid".into(), "root", "query-bound")
                }
                _ => anyhow::bail!("unknown_search_source_plan"),
            };
            let input = match sentence_input(scenario) {
                Some(sentence) => input.replace("example.invalid", sentence),
                None => input,
            };
            Ok(SourcePlan {
                source,
                input,
                scope,
                work_kind,
            })
        })
        .collect()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileViewInputs {
    full: String,
    mini: String,
    preview: String,
}

pub(crate) fn file_view_inputs() -> Result<FileViewInputs> {
    let root = owned_source_root()?;
    Ok(FileViewInputs {
        full: format!("{}/", root.display()),
        mini: OWNED_DIRECTORY_INPUT.into(),
        preview: "~/search-contract/example.invalid-preview".into(),
    })
}

#[cfg(any(test, feature = "owned-ui-evaluation"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FileSearchStreamPhase {
    Accepted,
    Running,
    Completed,
    Failed,
    Cancelled,
    Unavailable,
}

#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub(crate) struct FileSearchStreamState {
    pub(crate) generation: u64,
    pub(crate) directory: Option<String>,
    pub(crate) show_hidden: bool,
    pub(crate) phase: FileSearchStreamPhase,
    pub(crate) failure: Option<crate::file_search::SearchFailure>,
}

#[cfg(any(test, feature = "owned-ui-evaluation"))]
impl FileSearchStreamState {
    pub(crate) fn finish(&mut self, result: Result<(), crate::file_search::SearchFailure>) {
        use crate::file_search::SearchFailure;
        self.phase = match &result {
            Ok(()) => FileSearchStreamPhase::Completed,
            Err(SearchFailure::Cancelled) => FileSearchStreamPhase::Cancelled,
            Err(SearchFailure::Source(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                FileSearchStreamPhase::Unavailable
            }
            Err(_) => FileSearchStreamPhase::Failed,
        };
        self.failure = result.err();
    }
}

#[cfg(any(test, feature = "owned-ui-evaluation"))]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileSearchStreamSnapshot<'a> {
    pub(crate) generation: u64,
    pub(crate) query: &'a str,
    pub(crate) directory: Option<&'a str>,
    pub(crate) show_hidden: bool,
    pub(crate) phase: FileSearchStreamPhase,
    pub(crate) loading: bool,
    pub(crate) result_count: usize,
    pub(crate) visible_count: usize,
    pub(crate) failure: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Success,
    Empty,
    Error,
    Unavailable,
    Disconnect,
}
impl Outcome {
    pub(crate) fn error(self) -> Option<anyhow::Error> {
        match self {
            Self::Success | Self::Empty => None,
            Self::Error => Some(anyhow::anyhow!("owned_provider_error")),
            Self::Unavailable => Some(
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "owned_provider_unavailable",
                )
                .into(),
            ),
            Self::Disconnect => Some(
                std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "owned_provider_worker_disconnected",
                )
                .into(),
            ),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Empty => "empty",
            Self::Error => "error",
            Self::Unavailable => "unavailable",
            Self::Disconnect => "disconnected",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderTerminal {
    Completed { count: usize },
    Failed,
    Unavailable,
    Disconnected,
    Cancelled,
    StaleDiscarded,
}
impl ProviderTerminal {
    pub(crate) fn for_error(error: &anyhow::Error) -> Self {
        if error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::Unsupported)
        {
            Self::Unavailable
        } else {
            Self::Failed
        }
    }
    pub(crate) fn for_read_outcome(result: std::result::Result<usize, &anyhow::Error>) -> Self {
        match result {
            Ok(count) => Self::Completed { count },
            Err(error) => Self::for_error(error),
        }
    }
    pub(crate) fn for_result<T>(result: &Result<Vec<T>>) -> Self {
        match result {
            Ok(rows) => Self::Completed { count: rows.len() },
            Err(error) => Self::for_error(error),
        }
    }
    fn state(self) -> &'static str {
        match self {
            Self::Completed { .. } => "completed",
            Self::Failed | Self::Disconnected => "failed",
            Self::Unavailable => "unavailable",
            Self::Cancelled => "cancelled",
            Self::StaleDiscarded => "stale-discarded",
        }
    }
    fn outcome(self) -> &'static str {
        match self {
            Self::Completed { count: 0 } => "empty",
            Self::Completed { .. } => "success",
            Self::Failed => "error",
            Self::Unavailable => "unavailable",
            Self::Disconnected => "disconnected",
            Self::Cancelled => "cancelled",
            Self::StaleDiscarded => "stale-discarded",
        }
    }
    fn count(self) -> Option<usize> {
        if let Self::Completed { count } = self {
            Some(count)
        } else {
            None
        }
    }
}

impl From<ProviderTerminal> for crate::RootProviderTerminal {
    fn from(value: ProviderTerminal) -> Self {
        match value {
            ProviderTerminal::Completed { count: 0 } => Self::Empty,
            ProviderTerminal::Completed { .. } => Self::Success,
            ProviderTerminal::Failed => Self::Failed,
            ProviderTerminal::Unavailable => Self::Unavailable,
            ProviderTerminal::Disconnected => Self::Disconnected,
            ProviderTerminal::Cancelled => Self::Cancelled,
            ProviderTerminal::StaleDiscarded => Self::StaleDiscarded,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunKind {
    Worker,
    SourceChange,
    SynchronousRead,
}
impl RunKind {
    fn label(self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::SourceChange => "sourceChange",
            Self::SynchronousRead => "synchronousRead",
        }
    }
}

fn planned_outcome(scenario: &str, source: &str, phase: u32) -> Outcome {
    match scenario {
        "error" if phase > 0 => Outcome::Error,
        "unavailable" if phase > 0 => Outcome::Unavailable,
        "disconnect" if phase > 0 => Outcome::Disconnect,
        "empty" if matches!(source, "spine" | "scripts" | "validation") => Outcome::Success,
        "empty" => Outcome::Empty,
        "removal" if phase > 0 => Outcome::Empty,
        "cohort-removal-replacement" if source == "tabs" && phase > 0 => Outcome::Empty,
        "tab-domain-hoist" if !matches!(source, "tabs" | "icons") => Outcome::Empty,
        _ => Outcome::Success,
    }
}

struct RunState {
    kind: RunKind,
    source: &'static str,
    query: String,
    generation: u64,
    state: &'static str,
    policy: Option<RootProviderPublicationPolicy>,
    outcome: Outcome,
    terminal: Option<ProviderTerminal>,
    payload_phase: u32,
    capability_refusal: Option<&'static str>,
    admission_applied: bool,
    origin_admission_id: Option<u64>,
    payload_prepared: bool,
    delivery_due_at_ms: Option<u64>,
    delivery_attempted: bool,
    sender_dropped: bool,
    waker: Option<Waker>,
}

impl RunState {
    fn observation(&self, id: u64) -> Value {
        json!({
            "id":id,"source":self.source,"query":self.query,"generation":self.generation,
            "kind":self.kind.label(),
            "state":self.state,"publicationPolicy":self.policy,"plannedResponse":self.outcome.label(),
            "outcome":self.terminal.map(ProviderTerminal::outcome),"resultCount":self.terminal.and_then(ProviderTerminal::count),"payloadPhase":self.payload_phase,
            "capabilityRefusal":self.capability_refusal,"admissionApplied":self.admission_applied,
            "originAdmissionId":self.origin_admission_id,
            "payloadPrepared":self.payload_prepared,"pendingDelivery":self.delivery_due_at_ms.is_some(),"deliveryDueAtMs":self.delivery_due_at_ms,
            "deliveryAttempted":self.delivery_attempted,"senderDropped":self.sender_dropped,
        })
    }
}

struct PreviewCompletionWaiter {
    generation: u64,
    query: String,
    work_sequence: u64,
    path: String,
    decoded: bool,
    content_hash: Option<String>,
    due_at_ms: u64,
    waker: Option<Waker>,
}

impl PreviewCompletionWaiter {
    fn observation(&self, logical_time_ms: u64) -> Value {
        json!({"version":1,"generation":self.generation,"query":self.query,
            "workSequence":self.work_sequence,"phase":"held","path":self.path,
            "decoded":self.decoded,"contentHash":self.content_hash,
            "logicalTimeMs":logical_time_ms,"dueAtMs":self.due_at_ms})
    }
}

struct PreviewCompletionWait {
    gate: Arc<SearchGate>,
    id: u64,
}
impl Drop for PreviewCompletionWait {
    fn drop(&mut self) {
        self.gate.state.lock().preview_completions.remove(&self.id);
    }
}
struct GateState {
    retired: bool,
    overflow: bool,
    logical_ms: u64,
    runs: BTreeMap<u64, RunState>,
    source_phases: BTreeMap<&'static str, u32>,
    source_admissions: BTreeMap<&'static str, u64>,
    due_source_changes: BTreeMap<&'static str, u64>,
    preview_completions: BTreeMap<u64, PreviewCompletionWaiter>,
}

pub(crate) struct SearchGate {
    scenario: &'static str,
    state: Mutex<GateState>,
    retired_gate: Mutex<Option<Arc<SearchGate>>>,
}
impl SearchGate {
    pub(crate) fn new(scenario: &str) -> Result<Arc<Self>> {
        ensure!(
            crate::runtime_policy::is_owned_evaluation(),
            "search_gate_requires_owned_runtime"
        );
        let scenario = scenario_ids()
            .find(|id| *id == scenario)
            .ok_or_else(|| anyhow::anyhow!("unknown_search_scenario"))?;
        let mut runs = BTreeMap::new();
        for &source in SYNCHRONOUS_PROVIDERS {
            runs.insert(
                NEXT_RUN.fetch_add(1, Ordering::Relaxed),
                RunState {
                    kind: RunKind::SourceChange,
                    source,
                    query: String::new(),
                    generation: 0,
                    state: "awaiting-admission",
                    policy: None,
                    outcome: planned_outcome(scenario, source, 1),
                    terminal: None,
                    payload_phase: 1,
                    capability_refusal: None,
                    admission_applied: false,
                    origin_admission_id: None,
                    waker: None,
                    payload_prepared: false,
                    delivery_due_at_ms: None,
                    delivery_attempted: false,
                    sender_dropped: false,
                },
            );
        }
        Ok(Arc::new(Self {
            scenario,
            state: Mutex::new(GateState {
                retired: false,
                overflow: false,
                logical_ms: LOGICAL_TIME_MS.load(Ordering::Relaxed),
                runs,
                source_phases: BTreeMap::new(),
                source_admissions: BTreeMap::new(),
                due_source_changes: BTreeMap::new(),
                preview_completions: BTreeMap::new(),
            }),
            retired_gate: Mutex::new(None),
        }))
    }
    pub(crate) fn scenario(&self) -> &'static str {
        self.scenario
    }
    pub(crate) fn now(&self) -> Instant {
        crate::runtime_policy::root_search_now()
    }
    pub(crate) fn retain_retired_gate(&self, old: Arc<SearchGate>) {
        old.retired_gate.lock().take();
        *self.retired_gate.lock() = Some(old);
    }
    pub(crate) fn begin(
        self: &Arc<Self>,
        source: &'static str,
        query: &str,
        generation: u64,
        policy: RootProviderPublicationPolicy,
    ) -> Option<SearchRun> {
        let mut state = self.state.lock();
        if state.retired {
            return None;
        }
        if state.runs.len() >= MAX_RUNS || query.len() > 1_024 || !PROVIDERS.contains(&source) {
            state.overflow = true;
            return None;
        }
        let payload_phase = state.source_phases.get(source).copied().unwrap_or(0);
        let kind = if SYNCHRONOUS_PROVIDERS.contains(&source) {
            RunKind::SynchronousRead
        } else {
            RunKind::Worker
        };
        let outcome = planned_outcome(self.scenario, source, payload_phase);
        let id = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
        let origin_admission_id = state.source_admissions.get(source).copied();
        state.runs.insert(
            id,
            RunState {
                kind,
                source,
                query: query.into(),
                generation,
                state: if kind == RunKind::Worker {
                    "held"
                } else {
                    "reading"
                },
                policy: Some(policy),
                outcome,
                terminal: None,
                payload_phase,
                capability_refusal: None,
                admission_applied: false,
                origin_admission_id,
                payload_prepared: false,
                delivery_due_at_ms: None,
                delivery_attempted: false,
                sender_dropped: false,
                waker: None,
            },
        );
        Some(SearchRun {
            gate: self.clone(),
            id,
            source,
            payload_phase,
        })
    }
    pub(crate) fn release(&self, ids: &[u64]) -> Result<()> {
        crate::protocol::validate_search_run_ids(ids).map_err(anyhow::Error::msg)?;
        let wakes = {
            let mut state = self.state.lock();
            ensure!(
                !state.retired && !state.overflow,
                "search_gate_retired_or_overflowed"
            );
            // Validate the complete batch before mutating any run. No receiver
            // is woken until every selected run is marked released.
            for id in ids {
                let run = state
                    .runs
                    .get(id)
                    .ok_or_else(|| anyhow::anyhow!("unknown_search_run"))?;
                ensure!(
                    matches!(
                        (run.kind, run.state),
                        (RunKind::Worker, "held") | (RunKind::SourceChange, "awaiting-admission")
                    ),
                    "search_run_not_held"
                );
            }
            if let Some(id) = ids.iter().copied().find(|id| {
                state.runs[id].kind == RunKind::SourceChange
                    && state.runs[id].outcome == Outcome::Disconnect
            }) {
                state
                    .runs
                    .get_mut(&id)
                    .expect("validated source change")
                    .capability_refusal = Some("synchronous_source_has_no_worker");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "synchronous_source_has_no_worker",
                )
                .into());
            }
            let mut wakes = Vec::with_capacity(ids.len());
            for id in ids {
                let run = state.runs.get_mut(id).expect("validated release identity");
                run.state = "released";
                if let Some(waker) = run.waker.take() {
                    wakes.push(waker);
                }
            }
            wakes
        };
        for wake in wakes {
            wake.wake();
        }
        Ok(())
    }
    pub(crate) fn take_released_source_changes(self: &Arc<Self>) -> Vec<SearchRun> {
        let mut state = self.state.lock();
        if state.retired {
            return Vec::new();
        }
        let ids: Vec<_> = state
            .runs
            .iter()
            .filter_map(|(&id, run)| {
                (run.kind == RunKind::SourceChange && run.state == "released").then_some(id)
            })
            .collect();
        ids.into_iter()
            .map(|id| {
                let (source, payload_phase) = {
                    let run = state.runs.get_mut(&id).expect("released source change");
                    run.state = "delivered";
                    (run.source, run.payload_phase)
                };
                state.source_phases.insert(source, payload_phase);
                state.source_admissions.insert(source, id);
                SearchRun {
                    gate: self.clone(),
                    id,
                    source,
                    payload_phase,
                }
            })
            .collect()
    }
    pub(crate) fn advance(&self, milliseconds: u32) -> Result<()> {
        let mut state = self.state.lock();
        ensure!(
            !state.retired && !state.overflow,
            "search_gate_retired_or_overflowed"
        );
        ensure!(
            milliseconds > 0 && milliseconds <= MAX_ADVANCE_MS,
            "search_advance_out_of_bounds"
        );
        let next = state.logical_ms + u64::from(milliseconds);
        ensure!(next <= MAX_LOGICAL_MS, "search_logical_time_exhausted");
        crate::runtime_policy::advance_owned_root_search_clock(Duration::from_millis(u64::from(
            milliseconds,
        )))?;
        LOGICAL_TIME_MS.store(next, Ordering::Relaxed);
        state.logical_ms = next;
        drop(state);
        self.wake_due_completions(next);
        Ok(())
    }
    /// The real resource read/highlight precedes this fixed fixture delay.
    /// Retirement does not cancel it: the production preview owner must reject
    /// the actual late completion using its captured subject and query stamp.
    pub(crate) async fn wait_for_preview_completion(
        self: &Arc<Self>,
        generation: u64,
        request: &crate::FileSearchPreviewRequest,
        decoded: std::result::Result<Option<&str>, ()>,
    ) -> Result<()> {
        if self.scenario != "replacement" {
            return Ok(());
        }
        let id = {
            let mut state = self.state.lock();
            ensure!(
                state.preview_completions.len() < MAX_PENDING_PREVIEW_COMPLETIONS,
                "search_preview_completion_limit"
            );
            let id = NEXT_PREVIEW_COMPLETION.fetch_add(1, Ordering::Relaxed);
            let due_at_ms = LOGICAL_TIME_MS.load(Ordering::Relaxed) + PREVIEW_COMPLETION_DELAY_MS;
            state.preview_completions.insert(
                id,
                PreviewCompletionWaiter {
                    generation,
                    query: request.query_text.clone(),
                    work_sequence: request.sequence,
                    path: request.file.path.clone(),
                    decoded: decoded.is_ok(),
                    content_hash: decoded.ok().flatten().map(str::to_owned),
                    due_at_ms,
                    waker: None,
                },
            );
            id
        };
        let wait = PreviewCompletionWait {
            gate: self.clone(),
            id,
        };
        std::future::poll_fn(|cx| {
            let mut state = self.state.lock();
            let waiter = state
                .preview_completions
                .get_mut(&id)
                .expect("registered preview completion");
            if LOGICAL_TIME_MS.load(Ordering::Relaxed) >= waiter.due_at_ms {
                Poll::Ready(())
            } else {
                waiter.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await;
        drop(wait);
        Ok(())
    }

    pub(crate) fn file_search_preview_completion(
        &self,
        generation: u64,
        query: &str,
        work_sequence: u64,
    ) -> Result<Option<Value>> {
        let state = self.state.lock();
        ensure!(
            self.scenario == "replacement" && !state.retired && !state.overflow,
            "file_search_preview_unavailable"
        );
        Ok(state
            .preview_completions
            .values()
            .find(|waiter| {
                waiter.generation == generation
                    && waiter.query == query
                    && waiter.work_sequence == work_sequence
                    && state.logical_ms < waiter.due_at_ms
            })
            .map(|waiter| waiter.observation(state.logical_ms)))
    }

    fn wake_due_completions(&self, now: u64) {
        let wakes = {
            let mut state = self.state.lock();
            let mut wakes: Vec<_> = state
                .preview_completions
                .values_mut()
                .filter(|waiter| waiter.due_at_ms <= now)
                .filter_map(|waiter| waiter.waker.take())
                .collect();
            wakes.extend(
                state
                    .runs
                    .values_mut()
                    .filter(|run| run.delivery_due_at_ms.is_some_and(|due| due <= now))
                    .filter_map(|run| run.waker.take()),
            );
            wakes
        };
        for wake in wakes {
            wake.wake();
        }
        if let Some(old) = self.retired_gate.lock().as_ref() {
            old.wake_due_completions(now);
        }
    }
    /// Compiled source-content changes become due only after an accepted seed
    /// and an explicit clock advance. Main routes these through source-owned
    /// freshness invalidation and the ordinary producer kickoff methods.
    pub(crate) fn take_due_source_changes(&self) -> Result<Vec<&'static str>> {
        let mut state = self.state.lock();
        if state.retired {
            return Ok(Vec::new());
        }
        let due: Vec<_> = state
            .due_source_changes
            .iter()
            .filter_map(|(&source, &at)| (at <= state.logical_ms).then_some(source))
            .collect();
        if matches!(self.scenario, "metadata" | "replacement" | "deep-list")
            && due
                .iter()
                .any(|source| matches!(*source, "files" | "directory" | "spine"))
        {
            let root = owned_source_root()?;
            let count = if self.scenario == "deep-list" { 96 } else { 2 };
            for index in 0..count {
                let key = if self.scenario == "replacement" {
                    index + 100
                } else {
                    index
                };
                write_document(&root, key, true)?;
            }
            stabilize_fixture_modified(&root, true)?;
        }
        for source in &due {
            state.due_source_changes.remove(source);
            state.source_phases.insert(source, 1);
        }
        Ok(due)
    }
    pub(crate) fn retire(&self) {
        let wakes = {
            let mut state = self.state.lock();
            state.retired = true;
            state
                .runs
                .values_mut()
                .filter_map(|run| {
                    if matches!(run.state, "held" | "reading" | "awaiting-admission") {
                        run.state = "cancelled";
                        if run.kind != RunKind::SourceChange && run.capability_refusal.is_none() {
                            run.terminal = Some(ProviderTerminal::Cancelled);
                        }
                        return run.waker.take();
                    }
                    None
                })
                .collect::<Vec<_>>()
        };
        for wake in wakes {
            wake.wake();
        }
    }
    /// Correlate the production ticket without serializing unrelated/retired runs.
    /// Source-change controls have no worker ticket and are deliberately excluded.
    pub(crate) fn provider_run_observation(
        &self,
        source: &str,
        generation: u64,
    ) -> Result<Option<Value>> {
        let state = self.state.lock();
        ensure!(!state.retired, "search_provider_gate_retired");
        ensure!(!state.overflow, "search_provider_gate_overflow");
        let mut matches = state.runs.iter().filter(|(_, run)| {
            run.source == source
                && run.generation == generation
                && run.kind != RunKind::SourceChange
        });
        let run = matches.next();
        ensure!(matches.next().is_none(), "search_provider_run_ambiguous");
        Ok(run.map(|(id, run)| run.observation(*id)))
    }

    pub(crate) fn observation(&self) -> Value {
        let state = self.state.lock();
        let runs: Vec<_> = state
            .runs
            .iter()
            .map(|(id, run)| run.observation(*id))
            .collect();
        let pending: Vec<_> = state
            .runs
            .iter()
            .filter_map(|(id, run)| {
                (run.capability_refusal.is_none()
                    && matches!(
                        run.state,
                        "held" | "reading" | "awaiting-admission" | "released" | "delivered"
                    ))
                .then_some(*id)
            })
            .collect();
        json!({"version":1,"scenario":self.scenario,"logicalTimeMs":state.logical_ms,
            "displayUnixMs":crate::runtime_policy::root_search_display_unix_ms(),
            "expectedNoteSubtitleFingerprint":EXPECTED_NOTE_SUBTITLE_FINGERPRINT.as_str(),
            "retired":state.retired,"overflow":state.overflow,"runs":runs,"pendingRunIds":pending,
            "pendingSourceChanges":state.due_source_changes.iter().map(|(source,at)|json!({"source":source,"dueAtMs":at})).collect::<Vec<_>>(),
            "pendingPreviewCompletions":state.preview_completions.values().filter(|waiter|state.logical_ms < waiter.due_at_ms).map(|waiter|waiter.observation(state.logical_ms)).collect::<Vec<_>>(),
            "retiredGate":self.retired_gate.lock().as_ref().map(|gate|gate.observation())})
    }
}

pub(crate) struct SearchRun {
    gate: Arc<SearchGate>,
    id: u64,
    source: &'static str,
    payload_phase: u32,
}
impl SearchRun {
    pub(crate) fn source(&self) -> &'static str {
        self.source
    }
    pub(crate) fn finish_source_change(&self) {
        let mut state = self.gate.state.lock();
        let run = state
            .runs
            .get_mut(&self.id)
            .expect("registered source change");
        assert!(
            run.kind == RunKind::SourceChange && run.state == "delivered",
            "source-change admission was not delivered"
        );
        run.state = "completed";
        run.admission_applied = true;
    }
    pub(crate) fn row_count(&self) -> usize {
        match self.gate.scenario {
            "deep-list" => 96,
            "passive-budget" => 8,
            _ => 2,
        }
    }
    pub(crate) fn changed_payload(&self) -> bool {
        self.payload_phase > 0
    }
    fn row_key(&self, index: usize) -> usize {
        if self.changed_payload()
            && (self.gate.scenario == "replacement"
                || (self.gate.scenario == "cohort-removal-replacement" && self.source == "notes"))
        {
            index + 100
        } else {
            index
        }
    }
    fn title(&self, source: &str, index: usize) -> String {
        if let Some(sentence) = sentence_input(self.gate.scenario) {
            return format!("{sentence} · {source} {index}");
        }
        let revised = if self.changed_payload()
            && (matches!(self.gate.scenario, "metadata" | "deep-list")
                || SYNCHRONOUS_PROVIDERS.contains(&self.source))
        {
            " revised"
        } else {
            ""
        };
        format!("example.invalid {source} {index}{revised}")
    }
    pub(crate) async fn ready(&self) -> Result<Outcome> {
        std::future::poll_fn(|cx| {
            let mut state = self.gate.state.lock();
            let run = state.runs.get_mut(&self.id).expect("owned registered run");
            if run.kind != RunKind::Worker {
                return Poll::Ready(Err(anyhow::anyhow!("source_change_is_not_a_worker")));
            }
            match run.state {
                "released" => Poll::Ready(Ok(run.outcome)),
                "held" => {
                    run.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
                _ => Poll::Ready(Err(anyhow::anyhow!("search_run_not_pending"))),
            }
        })
        .await
    }
    /// The outer error is a capability refusal before the source read callback
    /// executes. An ordinary native IO error belongs inside the callback's T.
    pub(crate) fn read_synchronously<T>(
        &self,
        build: impl FnOnce(Outcome, &SearchRun) -> T,
    ) -> Result<T> {
        let outcome = {
            let mut state = self.gate.state.lock();
            let run = state.runs.get_mut(&self.id).expect("owned registered run");
            assert!(
                run.kind == RunKind::SynchronousRead,
                "source-change admission is not a source read"
            );
            if run.outcome == Outcome::Disconnect {
                run.capability_refusal = Some("synchronous_source_has_no_worker");
            } else {
                run.state = "delivered";
            }
            run.outcome
        };
        if outcome == Outcome::Disconnect {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "synchronous_source_has_no_worker",
            )
            .into());
        }
        Ok(build(outcome, self))
    }
    /// Keep each producer's native transport. In the disconnect scenario the
    /// captured sender is genuinely dropped without sending any payload.
    pub(crate) async fn deliver<T, R>(
        &self,
        send: impl FnOnce(T) -> R,
        build: impl FnOnce(Outcome, &SearchRun) -> T,
    ) {
        let Ok(outcome) = self.ready().await else {
            return;
        };
        let payload = if outcome == Outcome::Disconnect {
            None
        } else {
            Some(build(outcome, self))
        };
        let delayed = self.gate.scenario == "owner-retirement";
        {
            let mut state = self.gate.state.lock();
            let run = state
                .runs
                .get_mut(&self.id)
                .expect("registered source delivery");
            run.payload_prepared = payload.is_some();
            if delayed {
                run.delivery_due_at_ms = Some(
                    LOGICAL_TIME_MS.load(Ordering::Relaxed) + OWNER_RETIREMENT_DELIVERY_DELAY_MS,
                );
            }
        }
        if delayed {
            std::future::poll_fn(|cx| {
                let mut state = self.gate.state.lock();
                let run = state
                    .runs
                    .get_mut(&self.id)
                    .expect("registered source delivery");
                if LOGICAL_TIME_MS.load(Ordering::Relaxed)
                    >= run.delivery_due_at_ms.expect("pending delivery deadline")
                {
                    Poll::Ready(())
                } else {
                    run.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            })
            .await;
        }
        {
            let mut state = self.gate.state.lock();
            let run = state
                .runs
                .get_mut(&self.id)
                .expect("registered source delivery");
            run.delivery_due_at_ms = None;
            run.delivery_attempted = payload.is_some();
            if run.terminal.is_none() {
                run.state = "delivered";
            }
        }
        if let Some(payload) = payload {
            let _ = send(payload);
        } else {
            drop(send);
            self.gate
                .state
                .lock()
                .runs
                .get_mut(&self.id)
                .expect("registered dropped sender")
                .sender_dropped = true;
        }
    }
    pub(crate) fn finish(&self, terminal: ProviderTerminal, policy: RootProviderPublicationPolicy) {
        let mut state = self.gate.state.lock();
        let source = {
            let run = state.runs.get_mut(&self.id).expect("owned registered run");
            assert!(
                run.kind != RunKind::SourceChange,
                "source-change admission has no read outcome"
            );
            if run.terminal.is_some() {
                return;
            }
            run.policy = Some(policy);
            run.state = terminal.state();
            run.terminal = Some(terminal);
            run.source
        };
        if !state.retired
            && matches!(terminal, ProviderTerminal::Completed { .. })
            && !SYNCHRONOUS_PROVIDERS.contains(&source)
            && (matches!(
                self.gate.scenario,
                "metadata"
                    | "replacement"
                    | "removal"
                    | "deep-list"
                    | "error"
                    | "unavailable"
                    | "disconnect"
            ) || (self.gate.scenario == "cohort-removal-replacement"
                && matches!(source, "tabs" | "notes")))
            && !state.source_phases.contains_key(source)
        {
            let due_at = state.logical_ms + 16;
            state.due_source_changes.entry(source).or_insert(due_at);
        }
    }
}
impl Drop for SearchRun {
    fn drop(&mut self) {
        let mut state = self.gate.state.lock();
        if let Some(run) = state.runs.get_mut(&self.id) {
            run.delivery_due_at_ms = None;
            run.waker = None;
            if matches!(run.state, "held" | "reading" | "released" | "delivered") {
                run.state = "cancelled";
                if run.kind != RunKind::SourceChange && run.capability_refusal.is_none() {
                    run.terminal = Some(ProviderTerminal::Cancelled);
                }
            }
        }
    }
}

pub(crate) fn catalogue() -> Value {
    json!({"fixtureId":FIXTURE_ID,"version":1,"scenarios":scenario_ids().map(|id| json!({"id":id})).collect::<Vec<_>>(),
        "providers":PROVIDERS,"runKinds":["worker","sourceChange","synchronousRead"],"sourceChangeProviders":SYNCHRONOUS_PROVIDERS,
        "limits":{"maxRuns":MAX_RUNS,"maxAdvanceMs":MAX_ADVANCE_MS,"maxLogicalTimeMs":MAX_LOGICAL_MS,"maxPendingPreviewCompletions":MAX_PENDING_PREVIEW_COMPLETIONS},
        "previewCompletion":{"scenario":"replacement","delayMs":PREVIEW_COMPLETION_DELAY_MS},
        "ownerRetirementDelivery":{"scenario":"owner-retirement","delayMs":OWNER_RETIREMENT_DELIVERY_DELAY_MS,"after":"actual-payload-read-before-native-send","clock":"process-global-monotonic-logical-time","retirement":"released-work-survives-held-work-cancels"},
        "control":{"family":"search","operations":["prepare","release","advance"],"releaseIdentity":"observed-run-ids","maxReleaseRuns":crate::protocol::MAX_SEARCH_RELEASE_RUNS,"atomicReleaseValidation":true},
        "displayClock":{"mode":"fixed","unixMs":crate::runtime_policy::OWNED_ROOT_SEARCH_DISPLAY_UNIX_MS,"advancesWithLogicalTime":false},
        "clock":"explicit-logical-advance-not-ui-latency"})
}

fn response<T>(outcome: Outcome, rows: impl FnOnce() -> Vec<T>) -> Result<Vec<T>> {
    if let Some(error) = outcome.error() {
        return Err(error);
    }
    Ok(if outcome == Outcome::Empty {
        Vec::new()
    } else {
        rows()
    })
}

pub(crate) fn tab_result(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::browser_tabs::BrowserTabInfo>> {
    crate::browser_tabs::owned_root_browser_tabs_snapshot(response(outcome, || {
        (0..run.row_count())
            .map(|index| crate::browser_tabs::BrowserTabInfo {
                browser_name: "Google Chrome".into(),
                browser_bundle_id: "com.google.Chrome".into(),
                window_index: 1,
                tab_index: run.row_key(index) + 1,
                title: run.title("tab", index).into(),
                url: format!("https://example.invalid/{}", run.row_key(index)).into(),
            })
            .collect()
    }))
}

pub(crate) fn history_result(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::browser_history::RootBrowserHistorySearchHit>> {
    crate::browser_history::owned_root_browser_history_snapshot(response(outcome, || {
        (0..run.row_count())
            .map(
                |index| crate::browser_history::RootBrowserHistorySearchHit {
                    stable_key: format!("owned-history-{}", run.row_key(index)),
                    provider_label: "Fixture Browser".into(),
                    profile_label: "Owned".into(),
                    title: run.title("history", index),
                    url: format!("https://example.invalid/history/{}", run.row_key(index)),
                    domain: "example.invalid".into(),
                    last_visit_unix_ms: 1_777_593_600_000,
                    visit_count: 3,
                },
            )
            .collect()
    }))
}

pub(crate) fn window_result(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::window_control::WindowInfo>> {
    response(outcome, || {
        (0..run.row_count())
            .map(|index| crate::window_control::WindowInfo {
                id: run.row_key(index) as u32 + 1,
                app: "Fixture Editor".into(),
                title: run.title("window", index),
                bounds: crate::window_control::Bounds::new(0, 0, 800, 600),
                pid: 2_000_000_001,
                bundle_id: Some("dev.scriptkit.fixture-editor".into()),
                app_path: None,
                app_order: 0,
                window_index: index,
                global_order: index,
                is_frontmost_app: false,
                is_focused: false,
                is_main: index == 0,
                is_minimized: false,
                is_on_current_space: false,
                descriptor: "Owned synthetic window".into(),
                handle: crate::window_control::WindowHandle {
                    pid: 2_000_000_001,
                    native_window_id: None,
                    registry_generation: 0,
                    nonce: run.row_key(index) as u64 + 1,
                },
            })
            .collect()
    })
}

pub(crate) fn local_snapshot(
    refresh: &crate::scripts::root_search_contract::RootLocalContentRefresh,
    outcome: Outcome,
    run: &SearchRun,
) -> Result<crate::scripts::root_search_contract::RootLocalContentSnapshot> {
    use crate::scripts::root_search_contract::{
        RootLocalContentRefresh as Refresh, RootLocalContentSnapshot as Snapshot,
    };
    match refresh {
        Refresh::Notes(refresh) => crate::notes::owned_root_notes_search_snapshot(
            refresh,
            response(outcome, || {
                (0..run.row_count())
                    .map(|index| crate::notes::RootNoteSearchHit {
                        id: crate::notes::NoteId(uuid::Uuid::from_u128(
                            0xd0197594111140008000000000000000 + run.row_key(index) as u128,
                        )),
                        title: run.title("note", index),
                        updated_at: chrono::DateTime::from_timestamp(1_777_593_600, 0)
                            .expect("fixed timestamp"),
                        is_pinned: false,
                        char_count: 64,
                        score: 100 - index as i32,
                    })
                    .collect()
            }),
        )
        .map(Snapshot::Notes),
        Refresh::Todos(refresh) => crate::menu_syntax::owned_root_todos_snapshot(
            *refresh,
            response(outcome, || {
                (0..run.row_count())
                    .map(|index| crate::menu_syntax::RootTodoSearchHit {
                        stable_key: format!("owned-todo-{}", run.row_key(index)),
                        title: run.title("todo", index),
                        body: "Owned synthetic task".into(),
                        subtitle: "Owned task".into(),
                        tags: Vec::new(),
                        priority: None,
                        due: None,
                        created_at: Some("2026-05-01T00:00:00Z".into()),
                        path: crate::runtime_policy::owned_evaluation()
                            .expect("owned gate authority")
                            .root()
                            .join("search-fixture-todos.md"),
                        line_number: Some(index + 1),
                        raw_line: format!("- [ ] {}", run.title("todo", index)),
                    })
                    .collect()
            }),
        )
        .map(Snapshot::Todos),
    }
}

pub(crate) fn private_snapshot(
    provider: crate::scripts::root_search_contract::RootPrivateHistoryProvider,
    outcome: Outcome,
    run: &SearchRun,
) -> Result<crate::scripts::root_search_contract::RootPrivateHistorySnapshot> {
    use crate::scripts::root_search_contract::{
        RootPrivateHistoryProvider as Provider, RootPrivateHistorySnapshot as Snapshot,
    };
    match provider {
        Provider::Clipboard => crate::clipboard_history::owned_root_clipboard_history_snapshot(
            response(outcome, || {
                (0..run.row_count())
                    .map(|index| crate::clipboard_history::ClipboardEntryMeta {
                        id: format!("owned-clipboard-{}", run.row_key(index)),
                        content_type: crate::clipboard_history::ContentType::Text,
                        timestamp: 1_777_593_600_000,
                        pinned: false,
                        text_preview: run.title("clipboard", index),
                        image_width: None,
                        image_height: None,
                        byte_size: 64,
                        ocr_text: None,
                    })
                    .collect()
            }),
        )
        .map(Snapshot::Clipboard),
        Provider::Dictation => {
            crate::dictation::owned_root_dictation_history_snapshot(response(outcome, || {
                (0..run.row_count())
                    .map(|index| crate::dictation::DictationHistoryEntry {
                        version: 1,
                        id: format!("owned-dictation-{}", run.row_key(index)),
                        timestamp: "2026-05-01T00:00:00Z".into(),
                        transcript: run.title("dictation", index),
                        preview: run.title("dictation", index),
                        target_id: "main-filter".into(),
                        target_label_snapshot: "Main Filter".into(),
                        audio_duration_ms: 1000,
                    })
                    .collect()
            }))
            .map(Snapshot::Dictation)
        }
    }
}

pub(crate) fn conversation_snapshot(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<crate::ai::agent_chat::ui::history::RootAgentChatHistorySnapshot> {
    crate::ai::agent_chat::ui::history::owned_root_agent_chat_history_snapshot(response(
        outcome,
        || {
            (0..run.row_count())
                .map(
                    |index| crate::ai::agent_chat::ui::history::AgentChatHistoryEntry {
                        timestamp: "2026-05-01T00:00:00Z".into(),
                        session_id: format!("owned-conversation-{}", run.row_key(index)),
                        first_message: run.title("conversation", index),
                        title: run.title("conversation", index),
                        preview: "Owned saved conversation".into(),
                        search_text: run.title("conversation", index),
                        message_count: 2,
                        ..Default::default()
                    },
                )
                .collect()
        },
    ))
}

fn owned_source_root() -> Result<std::path::PathBuf> {
    let policy = crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("source_fixture_requires_owned_runtime"))?;
    // Bootstrap validates this process's isolated HOME. Never consult the
    // operator process or fall back to an OS account home directory.
    let home = std::path::PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("owned_home_missing"))?,
    );
    policy.require_owned_path(&home)?;
    ensure!(home.is_dir(), "owned_home_directory_missing");
    let root = home.join("search-contract");
    policy.require_owned_path(&root)?;
    Ok(root)
}

/// Enumerate the real owned corpus through the production streaming source.
/// The guard runs before directory IO and before following each entry's metadata.
#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub(crate) fn file_view_directory_stream<F>(
    directory: &str,
    cancel: crate::file_search::CancelToken,
    show_hidden: bool,
    mut on_event: F,
) where
    F: FnMut(crate::file_search::SearchEvent),
{
    let root = match owned_source_root() {
        Ok(root) => root,
        Err(error) => {
            on_event(crate::file_search::SearchEvent::Done(Err(
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, error.to_string()).into(),
            )));
            return;
        }
    };
    let policy = crate::runtime_policy::owned_evaluation().expect("validated owned source root");
    crate::file_search::list_directory_streaming_with_path_guard(
        directory,
        cancel,
        false,
        show_hidden,
        |path| {
            if !path.starts_with(&root) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "owned_file_search_path_outside_corpus",
                ));
            }
            policy.require_owned_path(path).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, error.to_string())
            })
        },
        on_event,
    );
}

pub(crate) fn script_catalog(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<(
    Vec<Arc<crate::scripts::Script>>,
    Vec<Arc<crate::scripts::Scriptlet>>,
)> {
    let root = owned_source_root()?;
    let scripts = response(outcome, || {
        (0..run.row_count())
            .map(|index| {
                Arc::new(crate::scripts::Script {
                    name: run.title("script", index),
                    path: root.join(format!("script-{}.ts", run.row_key(index))),
                    extension: "ts".into(),
                    description: Some(run.title("script description", index)),
                    plugin_id: "owned-search-contract".into(),
                    plugin_title: Some("Owned Search Contract".into()),
                    alias: if run.gate.scenario == "empty" {
                        Some("owned-validation-collision".into())
                    } else if matches!(run.gate.scenario, "eligibility" | "eligibility-portal")
                        && index == 0
                    {
                        Some(OWNED_SLOT_INPUT.into())
                    } else {
                        None
                    },
                    body: Some(
                        "// Compiled owned catalogue source; execution is refused.\n".into(),
                    ),
                    ..Default::default()
                })
            })
            .collect()
    })?;
    let scriptlets = if outcome == Outcome::Empty || run.gate.scenario == "empty" {
        Vec::new()
    } else {
        vec![Arc::new(crate::scripts::Scriptlet {
            name: run.title("scriptlet", 0),
            description: Some("Owned catalogue scriptlet".into()),
            code: "Owned search contract text".into(),
            tool: "paste".into(),
            shortcut: None,
            keyword: None,
            group: Some("Owned Search Contract".into()),
            plugin_id: "owned-search-contract".into(),
            plugin_title: Some("Owned Search Contract".into()),
            file_path: Some(root.join("scriptlets.md").to_string_lossy().into_owned()),
            command: Some(format!("scriptlet-{}", run.row_key(0))),
            alias: None,
            icon: None,
        })]
    };
    Ok((scripts, scriptlets))
}

pub(crate) fn validation_catalog(
    outcome: Outcome,
    _run: &SearchRun,
    candidates: Vec<Arc<crate::scripts::Script>>,
) -> Result<crate::scripts::ScriptCatalogReport> {
    if let Some(error) = outcome.error() {
        return Err(error);
    }
    // Validation consumes the exact immutable candidate snapshot captured by
    // the scripts owner, including rejected candidates. It has no source IO.
    Ok(crate::scripts::validate_script_catalog(candidates))
}

pub(crate) fn skill_catalog(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<Arc<crate::plugins::PluginSkill>>> {
    let root = owned_source_root()?;
    response(outcome, || {
        (0..run.row_count())
            .map(|index| {
                Arc::new(crate::plugins::PluginSkill {
                    plugin_id: "owned-search-contract".into(),
                    plugin_title: "Owned Search Contract".into(),
                    skill_id: format!("skill-{}", run.row_key(index)),
                    path: root.join(format!("skill-{}.md", run.row_key(index))),
                    title: run.title("skill", index),
                    description: run.title("skill description", index),
                })
            })
            .collect()
    })
}

pub(crate) fn app_catalog(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::app_launcher::AppInfo>> {
    let root = owned_source_root()?;
    response(outcome, || {
        (0..run.row_count())
            .map(|index| crate::app_launcher::AppInfo {
                name: run.title("application", index),
                path: root.join(format!("App-{}.app", run.row_key(index))),
                bundle_id: Some(format!(
                    "dev.scriptkit.owned-search.app-{}",
                    run.row_key(index)
                )),
                icon: None,
            })
            .collect()
    })
}

pub(crate) fn brain_result(
    outcome: Outcome,
    run: &SearchRun,
    semantic: bool,
) -> Result<Vec<crate::brain::RootBrainSearchHit>> {
    response(outcome, || {
        (0..run.row_count())
            .map(|index| crate::brain::RootBrainSearchHit {
                title: run.title(
                    if semantic {
                        "semantic match"
                    } else {
                        "lexical match"
                    },
                    index,
                ),
                excerpt: run.title("memory excerpt", index),
                source_label: "Note",
                source: crate::brain::DocSource::Note,
                source_id: uuid::Uuid::from_u128(
                    0xd0197594111140008000000000000000
                        + run.row_key(index) as u128
                        + if semantic { 1_000 } else { 0 },
                )
                .to_string(),
            })
            .collect()
    })
}

pub(crate) fn inbox_result(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::brain::InboxItem>> {
    response(outcome, || {
        (0..run.row_count())
            .map(|index| crate::brain::InboxItem {
                id: run.row_key(index) as i64 + 1,
                kind: crate::brain::inbox::InboxKind::Commitment,
                title: run.title("inbox commitment", index),
                detail: run.title("inbox detail", index),
                source: "note".into(),
                source_id: uuid::Uuid::from_u128(
                    0xd0197594111140008000000000000000 + run.row_key(index) as u128,
                )
                .to_string(),
                created_at: 1_777_593_600,
                resolved_at: None,
            })
            .collect()
    })
}

pub(crate) fn file_results(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::file_search::FileResult>> {
    if let Some(error) = outcome.error() {
        return Err(error);
    }
    if outcome == Outcome::Empty {
        return Ok(Vec::new());
    }
    let root = owned_source_root()?;
    Ok((0..run.row_count())
        .map(|index| document_result(&root, run.row_key(index), run.changed_payload()))
        .collect())
}

pub(crate) fn flow_roster(
    outcome: Outcome,
    run: &SearchRun,
) -> Result<Vec<crate::flows::model::FlowDescriptor>> {
    let root = owned_source_root()?;
    response(outcome, || {
        (0..run.row_count().min(32))
            .map(|index| crate::flows::model::FlowDescriptor {
                id: format!("project:owned-search-{}", run.row_key(index)),
                path: root
                    .join(format!("flow-{}.md", run.row_key(index)))
                    .to_string_lossy()
                    .into_owned(),
                source: crate::flows::model::FlowSource::Project,
                name: run.title("flow", index),
                description: Some(run.title("flow description", index)),
                engine: "pi".into(),
                engine_source: None,
                inputs: Vec::new(),
                is_workflow: false,
                interactive: true,
                mtime_ms: 1_777_593_600_000,
                origin: Some("Owned Search Contract".into()),
                wrapper_command: None,
            })
            .collect()
    })
}

fn document_content(key: usize, changed: bool) -> String {
    let revision = if changed { "Revised" } else { "Initial" };
    format!("# Owned document {key}\n{revision} example.invalid provider contents.\n")
}

fn document_result(
    root: &std::path::Path,
    key: usize,
    changed: bool,
) -> crate::file_search::FileResult {
    let name = format!("example.invalid-document-{key}.md");
    crate::file_search::FileResult {
        path: root.join(&name).to_string_lossy().into_owned(),
        name,
        size: document_content(key, changed).len() as u64,
        modified: 1_777_593_600 + u64::from(changed),
        file_type: crate::file_search::FileType::Document,
    }
}

fn write_document(
    root: &std::path::Path,
    key: usize,
    changed: bool,
) -> Result<crate::file_search::FileResult> {
    let result = document_result(root, key, changed);
    crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("source_fixture_requires_owned_runtime"))?
        .require_owned_path(std::path::Path::new(&result.path))?;
    std::fs::write(&result.path, document_content(key, changed))?;
    stabilize_fixture_modified(std::path::Path::new(&result.path), changed)?;
    Ok(result)
}

fn stabilize_fixture_modified(path: &std::path::Path, changed: bool) -> Result<()> {
    crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("source_fixture_requires_owned_runtime"))?
        .require_owned_path(path)?;
    let modified = std::time::UNIX_EPOCH + Duration::from_secs(1_777_593_600 + u64::from(changed));
    std::fs::File::open(path)?.set_times(std::fs::FileTimes::new().set_modified(modified))?;
    Ok(())
}

fn prepare_source_corpus(
    root: &std::path::Path,
    scenario: &str,
) -> Result<Vec<crate::file_search::FileResult>> {
    let policy = crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("source_fixture_requires_owned_runtime"))?;
    let count = match scenario {
        "deep-list" => 96,
        "passive-budget" => 8,
        _ => 2,
    };
    let mut files = Vec::with_capacity(count + if scenario == "replacement" { 2 } else { 0 });
    for key in 0..count {
        files.push(write_document(root, key, false)?);
    }
    if scenario == "replacement" {
        for key in 0..2 {
            let name = format!("example.invalid-preview-{key}.png");
            let path = root.join(&name);
            policy.require_owned_path(&path)?;
            let colour = if key == 0 {
                [210, 48, 52, 255]
            } else {
                [36, 148, 92, 255]
            };
            image::RgbaImage::from_pixel(32, 32, image::Rgba(colour)).save(&path)?;
            stabilize_fixture_modified(&path, false)?;
            files.push(crate::file_search::FileResult {
                path: path.to_string_lossy().into_owned(),
                name,
                size: std::fs::metadata(&path)?.len(),
                modified: 1_777_593_600,
                file_type: crate::file_search::FileType::Image,
            });
        }
    }
    let keys = (0..count).chain(
        (scenario == "replacement")
            .then_some(100..100 + count)
            .into_iter()
            .flatten(),
    );
    for key in keys {
        for (name, content) in [
            (format!("script-{key}.ts"), format!("// Owned script {key}\n// example.invalid catalogue source; execution is refused.\n")),
            (format!("skill-{key}.md"), format!("# Owned skill {key}\nexample.invalid skill contents.\n")),
            (format!("flow-{key}.md"), format!("# Owned flow {key}\nexample.invalid flow contents.\n")),
        ] {
            let path = root.join(name);
            policy.require_owned_path(&path)?;
            std::fs::write(&path, content)?;
            stabilize_fixture_modified(&path, false)?;
        }
        let app = root.join(format!("App-{key}.app"));
        policy.require_owned_path(&app)?;
        std::fs::create_dir_all(&app)?;
        stabilize_fixture_modified(&app, false)?;
    }
    let scriptlets = root.join("scriptlets.md");
    policy.require_owned_path(&scriptlets)?;
    std::fs::write(&scriptlets, "# Owned scriptlets\n\n## example.invalid scriptlet\n\n```paste\nOwned search contract\n```\n")?;
    stabilize_fixture_modified(&scriptlets, false)?;
    stabilize_fixture_modified(root, false)?;
    Ok(files)
}

#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
fn reset_source_stores(app: &mut crate::ScriptListApp) -> Result<()> {
    app.root_search.reset_owned_fixture();
    crate::browser_tabs::reset_owned_root_browser_tabs()?;
    crate::browser_history::reset_owned_root_browser_history()?;
    crate::notes::reset_owned_root_notes_search()?;
    crate::menu_syntax::reset_owned_root_todos()?;
    crate::clipboard_history::reset_owned_root_clipboard_history()?;
    crate::dictation::reset_owned_root_dictation_history()?;
    crate::ai::agent_chat::ui::history::reset_owned_root_agent_chat_history()?;
    Ok(())
}

#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
pub(crate) fn retire(app: &mut crate::ScriptListApp) -> Result<()> {
    let Some(gate) = app.main_services.search_gate() else {
        return Ok(());
    };
    gate.retire();
    reset_source_stores(app)?;
    crate::runtime_policy::disable_owned_root_search_clock()?;
    let policy = crate::runtime_policy::owned_evaluation().expect("owned fixture authority");
    let crate::MainServices::OwnedFixtures(sources) = &mut app.main_services else {
        unreachable!()
    };
    *Arc::make_mut(sources) = crate::OwnedMainSources::launcher(policy.root());
    app.config.unified_search = None;
    app.invalidate_root_passive_and_grouped_cache();
    Ok(())
}

#[cfg(all(feature = "owned-ui-evaluation", target_os = "macos"))]
pub(crate) fn prepare(
    app: &mut crate::ScriptListApp,
    scenario: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<crate::ScriptListApp>,
) -> Result<Arc<SearchGate>> {
    let gate = SearchGate::new(scenario)?;
    ensure!(
        app.main_services.owned_sources().is_some(),
        "search_fixture_requires_owned_services"
    );
    let root = owned_source_root()?;
    app.root_search.retire_query_owner();
    app.filter_coalescer.reset();
    if let Some(old) = app.main_services.search_gate() {
        old.retire();
        gate.retain_retired_gate(old);
    }
    crate::runtime_policy::enable_owned_root_search_clock()?;
    super::main_fixtures::mount_main_fixture(app, FIXTURE_ID, window, cx)?;
    reset_source_stores(app)?;
    std::fs::create_dir_all(&root)?;
    let crate::MainServices::OwnedFixtures(sources) = &mut app.main_services else {
        unreachable!()
    };
    let sources = Arc::make_mut(sources);
    sources.files.clear();
    sources.brain_inbox.clear();
    sources.brain_hits = if matches!(scenario, "tab-domain-hoist" | "empty") {
        Vec::new()
    } else {
        vec![crate::brain::RootBrainSearchHit {
            title: "example.invalid lexical seed".into(),
            excerpt: "Immediate lexical provider batch".into(),
            source_label: "Note",
            source: crate::brain::DocSource::Note,
            source_id: "d0197594-1111-4000-8000-000000000001".into(),
        }]
    };
    let files = prepare_source_corpus(&root, scenario)?;
    // Empty-query spine search consumes the actual recent-file source cache.
    sources.files = files.clone();
    sources.root_file_provider_files = Some(files);
    sources.search_gate = Some(gate.clone());
    app.cached_windows.clear();
    super::conversation_fixtures::seed_owned_flow_catalogue()?;
    app.reset_owned_search_catalogues(cx)?;
    let mut config = crate::config::UnifiedSearchConfig::default();
    config.enabled = true;
    config.files.enabled = true;
    config.files.global_search = true;
    config.files.directory_browse = true;
    config.files.recent_files = true;
    config.browser_tabs.enabled = true;
    let all = scenario != "tab-domain-hoist";
    config.brain.enabled = all;
    config.notes.enabled = all;
    config.todos.enabled = all;
    config.clipboard_history.enabled = all;
    config.dictation_history.enabled = all;
    config.agent_chat_history.enabled = all;
    config.browser_history.enabled = all;
    config.brain_inbox.enabled = all;
    if scenario == "deep-list" {
        config.browser_tabs.max_results = 96;
        config.passive_result_limits.max_total_results = 96;
    }
    app.config.unified_search = Some(config);
    app.refresh_root_recent_file_results();
    app.invalidate_root_passive_and_grouped_cache();
    // Source retirement invalidates computation even when the fixture factory
    // has already cleared the visible input. Let the ordinary setter reset
    // parser/coalescer ownership and reconcile that empty query.
    app.pending_filter_sync = true;
    app.set_filter_text_immediate(String::new(), window, cx);
    if scenario == "eligibility-portal" {
        ensure!(
            matches!(app.current_view, crate::AppView::ScriptList)
                && !app.is_in_attachment_portal(),
            "search_portal_fixture_requires_launcher"
        );
        // The real Chat host owns both return layers; no picker or result rows are injected.
        app.opened_from_main_menu = true;
        app.open_standard_agent_chat_mock_fixture(cx);
        ensure!(
            matches!(app.current_view, crate::AppView::AgentChatView { .. }),
            "search_portal_fixture_chat_unavailable"
        );
        app.open_attachment_portal(
            crate::ai::context_selector::types::ContextPortalKind::ScriptSearch,
            cx,
        );
        ensure!(
            matches!(app.current_view, crate::AppView::ScriptList)
                && app.is_in_attachment_portal()
                && matches!(
                    app.active_attachment_portal_kind,
                    Some(crate::ai::context_selector::types::ContextPortalKind::ScriptSearch)
                )
                && matches!(
                    app.attachment_portal_return_view,
                    Some(crate::AppView::AgentChatView { .. })
                ),
            "search_portal_fixture_initialization_failed"
        );
        app.set_filter_text_immediate(OWNED_SLOT_INPUT.into(), window, cx);
    }
    cx.notify();
    Ok(gate)
}

pub(crate) fn suggested_input(scenario: &str) -> String {
    if let Some(sentence) = sentence_input(scenario) {
        return sentence.into();
    }
    match scenario {
        "files-explicit" => "files: example.invalid".into(),
        "brain-replacement" => "brain: example.invalid".into(),
        "eligibility-portal" => OWNED_SLOT_INPUT.into(),
        "directory-browse" => format!(
            "{}/",
            owned_source_root()
                .expect("prepared owned fixture HOME")
                .display()
        ),
        _ => "example.invalid".into(),
    }
}

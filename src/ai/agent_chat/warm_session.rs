use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use parking_lot::Mutex;

use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx};
use crate::ai::agent_chat::runtime::AgentChatConnection;

const DEFAULT_STALE_ACQUIRED_TTL: Duration = Duration::from_secs(60);
const DEFAULT_PREPARE_READY_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) trait AgentChatWarmRuntimeFactory: Send + Sync + 'static {
    fn spawn_connection(&self) -> Result<Arc<dyn AgentChatConnection>>;
}

impl<F> AgentChatWarmRuntimeFactory for F
where
    F: Fn() -> Result<Arc<dyn AgentChatConnection>> + Send + Sync + 'static,
{
    fn spawn_connection(&self) -> Result<Arc<dyn AgentChatConnection>> {
        self()
    }
}

#[derive(Clone)]
pub(crate) struct AgentChatWarmSessionSpec {
    pub key: String,
    pub cwd: PathBuf,
    pub factory: Arc<dyn AgentChatWarmRuntimeFactory>,
}

#[derive(Clone)]
pub(crate) struct AgentChatWarmSessionLease {
    pub key: String,
    pub generation: u64,
    pub ui_thread_id: String,
    pub cwd: PathBuf,
    pub connection: Arc<dyn AgentChatConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatWarmSessionSnapshot {
    pub key: String,
    pub generation: u64,
    pub ui_thread_id: Option<String>,
    pub state: AgentChatWarmSessionState,
    pub failure: Option<crate::ai::reliability::AppFailureRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatWarmSessionState {
    Empty,
    Preparing,
    Ready,
    Acquired,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatWarmAcquireOrigin {
    Ready,
    /// The slot was still `Preparing` but its connection was already spawned;
    /// the caller joins the in-flight boot instead of cold-spawning a
    /// duplicate runtime. The pi worker queue serializes the prepare
    /// commands ahead of the caller's first turn.
    JoinedPreparing,
    ColdSpawned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentChatWarmReprepareResult {
    Started(AgentChatWarmSessionSnapshot),
    Current(AgentChatWarmSessionSnapshot),
    Missing,
}

struct WarmSlot {
    spec: AgentChatWarmSessionSpec,
    generation: u64,
    ui_thread_id: Option<String>,
    state: AgentChatWarmSessionState,
    connection: Option<Arc<dyn AgentChatConnection>>,
    acquired_at: Option<Instant>,
    failure: Option<crate::ai::reliability::AppFailureRecord>,
}

impl WarmSlot {
    fn snapshot(&self) -> AgentChatWarmSessionSnapshot {
        AgentChatWarmSessionSnapshot {
            key: self.spec.key.clone(),
            generation: self.generation,
            ui_thread_id: self.ui_thread_id.clone(),
            state: self.state,
            failure: self.failure.clone(),
        }
    }
}

pub(crate) struct AgentChatWarmSessionManager {
    inner: Arc<Mutex<WarmSessionInner>>,
    ui_thread_id_source: Arc<dyn Fn() -> String + Send + Sync>,
}

impl Clone for AgentChatWarmSessionManager {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            ui_thread_id_source: self.ui_thread_id_source.clone(),
        }
    }
}

#[derive(Default)]
struct WarmSessionInner {
    slots: HashMap<String, WarmSlot>,
    next_generation: u64,
}

impl Default for AgentChatWarmSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentChatWarmSessionManager {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WarmSessionInner::default())),
            ui_thread_id_source: Arc::new(default_warm_ui_thread_id),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_ui_thread_id_source(
        ui_thread_id_source: Arc<dyn Fn() -> String + Send + Sync>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(WarmSessionInner::default())),
            ui_thread_id_source,
        }
    }

    pub(crate) fn prepare_warm(
        &self,
        spec: AgentChatWarmSessionSpec,
    ) -> Result<AgentChatWarmSessionSnapshot> {
        self.cleanup_stale_acquired(DEFAULT_STALE_ACQUIRED_TTL);
        let (generation, ui_thread_id) = {
            let mut inner = self.inner.lock();
            if let Some(slot) = inner.slots.get(&spec.key) {
                if slot.state != AgentChatWarmSessionState::Failed {
                    return Ok(slot.snapshot());
                }
            }

            inner.next_generation += 1;
            let generation = inner.next_generation;
            let ui_thread_id = (self.ui_thread_id_source)();
            inner.slots.insert(
                spec.key.clone(),
                WarmSlot {
                    spec: spec.clone(),
                    generation,
                    ui_thread_id: Some(ui_thread_id.clone()),
                    state: AgentChatWarmSessionState::Preparing,
                    connection: None,
                    acquired_at: None,
                    failure: None,
                },
            );
            (generation, ui_thread_id)
        };

        let slot = self.prepare_slot_with_generation(spec, generation, ui_thread_id);
        let mut inner = self.inner.lock();
        let Some(current) = inner.slots.get(&slot.spec.key) else {
            let snapshot = slot.snapshot();
            inner.slots.insert(snapshot.key.clone(), slot);
            return Ok(snapshot);
        };
        if current.generation != generation || current.state != AgentChatWarmSessionState::Preparing
        {
            return Ok(current.snapshot());
        }

        let snapshot = slot.snapshot();
        inner.slots.insert(snapshot.key.clone(), slot);
        Ok(snapshot)
    }

    pub(crate) fn acquire_warm(&self, key: &str) -> Option<AgentChatWarmSessionLease> {
        self.cleanup_stale_acquired(DEFAULT_STALE_ACQUIRED_TTL);
        self.acquire_warm_ready(key)
    }

    pub(crate) fn acquire_ready_or_spawn_cold(
        &self,
        spec: AgentChatWarmSessionSpec,
    ) -> Result<(AgentChatWarmSessionLease, AgentChatWarmAcquireOrigin)> {
        self.cleanup_stale_acquired(DEFAULT_STALE_ACQUIRED_TTL);
        if let Some(lease) = self.acquire_warm_ready(&spec.key) {
            return Ok((lease, AgentChatWarmAcquireOrigin::Ready));
        }
        if let Some(lease) = self.acquire_preparing_join(&spec.key) {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "warm_acquire_joined_preparing",
                key = %lease.key,
                generation = lease.generation,
                ui_thread_id = %lease.ui_thread_id,
            );
            return Ok((lease, AgentChatWarmAcquireOrigin::JoinedPreparing));
        }

        let (generation, ui_thread_id, previous_state) = {
            let mut inner = self.inner.lock();
            let previous_state = inner.slots.get(&spec.key).map(|slot| slot.state);
            inner.next_generation += 1;
            let generation = inner.next_generation;
            let ui_thread_id = (self.ui_thread_id_source)();
            inner.slots.insert(
                spec.key.clone(),
                WarmSlot {
                    spec: spec.clone(),
                    generation,
                    ui_thread_id: Some(ui_thread_id.clone()),
                    state: AgentChatWarmSessionState::Preparing,
                    connection: None,
                    acquired_at: None,
                    failure: None,
                },
            );
            (generation, ui_thread_id, previous_state)
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "warm_cold_spawn_begin",
            key = %spec.key,
            generation,
            previous_state = ?previous_state,
        );

        let connection = match spec.factory.spawn_connection() {
            Ok(connection) => connection,
            Err(error) => {
                // S11: the spawn cause goes to the diagnostic vault; the slot
                // carries only classified safe copy. Formatting `{error:#}`
                // here used to print the internal `ai_adapter_error:<Code>`
                // marker straight into the Agent Chat surface.
                let failure = crate::ai::reliability::spawn_failure(
                    sk_protocol::ai_reliability::ProtocolComponent::Pi,
                    &format!("{error:#}"),
                );
                let failure_message = failure.primary_message();
                let mut inner = self.inner.lock();
                if let Some(slot) = inner.slots.get_mut(&spec.key) {
                    if slot.generation == generation {
                        slot.state = AgentChatWarmSessionState::Failed;
                        slot.failure = Some(failure);
                    }
                }
                anyhow::bail!(failure_message);
            }
        };

        {
            let mut inner = self.inner.lock();
            if let Some(slot) = inner.slots.get_mut(&spec.key) {
                if slot.generation == generation {
                    slot.state = AgentChatWarmSessionState::Acquired;
                    slot.connection = Some(connection.clone());
                    slot.acquired_at = Some(Instant::now());
                    slot.failure = None;
                }
            }
        }

        let lease = AgentChatWarmSessionLease {
            key: spec.key.clone(),
            generation,
            ui_thread_id,
            cwd: spec.cwd.clone(),
            connection,
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "warm_cold_spawn_acquired",
            key = %lease.key,
            generation = lease.generation,
            ui_thread_id = %lease.ui_thread_id,
            cwd = %lease.cwd.display(),
        );

        Ok((lease, AgentChatWarmAcquireOrigin::ColdSpawned))
    }

    pub(crate) fn prepare_warm_background(
        &self,
        spec: AgentChatWarmSessionSpec,
    ) -> Result<AgentChatWarmSessionSnapshot> {
        let (snapshot, should_spawn) = {
            let mut inner = self.inner.lock();
            if let Some(slot) = inner.slots.get(&spec.key) {
                if slot.state != AgentChatWarmSessionState::Failed {
                    return Ok(slot.snapshot());
                }
            }

            inner.next_generation += 1;
            let generation = inner.next_generation;
            let ui_thread_id = (self.ui_thread_id_source)();
            let slot = WarmSlot {
                spec: spec.clone(),
                generation,
                ui_thread_id: Some(ui_thread_id.clone()),
                state: AgentChatWarmSessionState::Preparing,
                connection: None,
                acquired_at: None,
                failure: None,
            };
            let snapshot = slot.snapshot();
            inner.slots.insert(spec.key.clone(), slot);
            (snapshot, (spec, generation, ui_thread_id))
        };

        let manager = self.clone();
        let spawn_result = std::thread::Builder::new()
            .name("warm-prepare-background".to_string())
            .spawn(move || {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "warm_background_thread_started",
                );
                let (spec, generation, ui_thread_id) = should_spawn;
                let slot = manager.prepare_slot_with_generation(spec, generation, ui_thread_id);
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "warm_background_thread_finished",
                    state = ?slot.state,
                );
                manager.insert_prepared_slot_if_current(slot, generation);
            })
            .map(|_| ());
        self.finish_background_thread_spawn(&snapshot, spawn_result)?;

        Ok(snapshot)
    }

    fn finish_background_thread_spawn(
        &self,
        snapshot: &AgentChatWarmSessionSnapshot,
        spawn_result: std::io::Result<()>,
    ) -> Result<()> {
        if let Err(error) = spawn_result {
            // S11: a thread that would not start IS a spawn failure. Handing
            // the prose to the free-text classifier matched nothing and
            // produced `Unknown`.
            let failure = crate::ai::reliability::spawn_failure(
                sk_protocol::ai_reliability::ProtocolComponent::Pi,
                &format!("warm prepare worker thread: {error}"),
            );
            let message = failure.primary_message();
            let fingerprint = failure
                .failure
                .diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.fingerprint.0.as_str())
                .unwrap_or("unavailable");
            tracing::error!(
                target: "script_kit::tab_ai",
                event = "warm_background_thread_spawn_failed",
                generation = snapshot.generation,
                key = %snapshot.key,
                failure_code = ?failure.failure.code,
                diagnostic_fingerprint = fingerprint,
            );
            self.mark_preparing_generation_failed(&snapshot.key, snapshot.generation, failure);
            anyhow::bail!(message);
        }
        Ok(())
    }

    /// Replace exactly one failed generation with a fresh background prepare.
    /// Concurrent Retry clicks carrying the same generation are duplicate-safe:
    /// only the first creates a slot and worker; later calls observe `Current`.
    pub(crate) fn reprepare_failed_generation_background(
        &self,
        spec: AgentChatWarmSessionSpec,
        expected_generation: u64,
    ) -> AgentChatWarmReprepareResult {
        let (snapshot, generation, ui_thread_id) = {
            let mut inner = self.inner.lock();
            let Some(current) = inner.slots.get(&spec.key) else {
                return AgentChatWarmReprepareResult::Missing;
            };
            if current.generation != expected_generation
                || current.state != AgentChatWarmSessionState::Failed
            {
                return AgentChatWarmReprepareResult::Current(current.snapshot());
            }

            inner.next_generation += 1;
            let generation = inner.next_generation;
            let ui_thread_id = (self.ui_thread_id_source)();
            let slot = WarmSlot {
                spec: spec.clone(),
                generation,
                ui_thread_id: Some(ui_thread_id.clone()),
                state: AgentChatWarmSessionState::Preparing,
                connection: None,
                acquired_at: None,
                failure: None,
            };
            let snapshot = slot.snapshot();
            inner.slots.insert(spec.key.clone(), slot);
            (snapshot, generation, ui_thread_id)
        };

        let manager = self.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("warm-reprepare-background".to_string())
            .spawn(move || {
                let slot = manager.prepare_slot_with_generation(spec, generation, ui_thread_id);
                manager.insert_prepared_slot_if_current(slot, generation);
            })
        {
            // S11: raw `std::io::Error` text used to become the slot's
            // user-visible failure copy verbatim.
            let failure = crate::ai::reliability::spawn_failure(
                sk_protocol::ai_reliability::ProtocolComponent::Pi,
                &format!("warm retry worker thread: {error}"),
            );
            let mut inner = self.inner.lock();
            if let Some(slot) = inner.slots.get_mut(&snapshot.key) {
                if slot.generation == generation
                    && slot.state == AgentChatWarmSessionState::Preparing
                {
                    slot.state = AgentChatWarmSessionState::Failed;
                    slot.failure = Some(failure);
                }
            }
        }

        AgentChatWarmReprepareResult::Started(snapshot)
    }

    fn mark_preparing_generation_failed(
        &self,
        key: &str,
        generation: u64,
        failure: crate::ai::reliability::AppFailureRecord,
    ) {
        let mut inner = self.inner.lock();
        if let Some(slot) = inner.slots.get_mut(key) {
            if slot.generation == generation && slot.state == AgentChatWarmSessionState::Preparing {
                slot.state = AgentChatWarmSessionState::Failed;
                slot.failure = Some(failure);
            }
        }
    }

    /// Acquire a slot that is still `Preparing` but whose runtime connection
    /// has already been spawned. The warm-up commands are queued ahead of the
    /// caller's first turn on the same worker, so joining is strictly faster
    /// than discarding the in-flight boot and cold-spawning a second runtime.
    fn acquire_preparing_join(&self, key: &str) -> Option<AgentChatWarmSessionLease> {
        let mut inner = self.inner.lock();
        let slot = inner.slots.get_mut(key)?;
        if slot.state != AgentChatWarmSessionState::Preparing {
            return None;
        }

        let connection = slot.connection.as_ref()?.clone();
        let ui_thread_id = slot.ui_thread_id.clone()?;
        slot.state = AgentChatWarmSessionState::Acquired;
        slot.acquired_at = Some(Instant::now());

        Some(AgentChatWarmSessionLease {
            key: key.to_string(),
            generation: slot.generation,
            ui_thread_id,
            cwd: slot.spec.cwd.clone(),
            connection,
        })
    }

    pub(crate) fn acquire_warm_ready(&self, key: &str) -> Option<AgentChatWarmSessionLease> {
        let mut inner = self.inner.lock();
        let slot = inner.slots.get_mut(key)?;
        if slot.state != AgentChatWarmSessionState::Ready {
            return None;
        }

        let connection = slot.connection.as_ref()?.clone();
        let ui_thread_id = slot.ui_thread_id.clone()?;
        slot.state = AgentChatWarmSessionState::Acquired;
        slot.acquired_at = Some(Instant::now());

        Some(AgentChatWarmSessionLease {
            key: key.to_string(),
            generation: slot.generation,
            ui_thread_id,
            cwd: slot.spec.cwd.clone(),
            connection,
        })
    }

    pub(crate) fn dismiss_reset_background(
        &self,
        lease: AgentChatWarmSessionLease,
    ) -> AgentChatWarmSessionSnapshot {
        self.reset_background_inner(lease, true)
    }

    /// Release an acquired lease because the conversation is moving to the
    /// detached chat window, which keeps using the lease's connection.
    ///
    /// Identical to [`Self::dismiss_reset_background`] (the slot is
    /// regenerated so the next launch gets a fresh warm session) except the
    /// old connection's turn is NOT cancelled — cancelling here would kill an
    /// in-flight response in the window the user just detached.
    pub(crate) fn release_for_detach_handoff(
        &self,
        lease: AgentChatWarmSessionLease,
    ) -> AgentChatWarmSessionSnapshot {
        self.reset_background_inner(lease, false)
    }

    fn reset_background_inner(
        &self,
        lease: AgentChatWarmSessionLease,
        cancel_old_turn: bool,
    ) -> AgentChatWarmSessionSnapshot {
        let key = lease.key.clone();
        let generation = lease.generation;
        let old_ui_thread_id = lease.ui_thread_id.clone();
        let old_connection = lease.connection.clone();

        let (snapshot, spec, replacement_generation, replacement_ui_thread_id) = {
            let mut inner = self.inner.lock();
            let Some(slot) = inner.slots.get(&key) else {
                return AgentChatWarmSessionSnapshot {
                    key,
                    generation,
                    ui_thread_id: None,
                    state: AgentChatWarmSessionState::Empty,
                    failure: None,
                };
            };

            if slot.generation != generation || slot.state != AgentChatWarmSessionState::Acquired {
                return slot.snapshot();
            }

            let spec = slot.spec.clone();
            inner.next_generation += 1;
            let replacement_generation = inner.next_generation;
            let replacement_ui_thread_id = (self.ui_thread_id_source)();
            let replacement = WarmSlot {
                spec: spec.clone(),
                generation: replacement_generation,
                ui_thread_id: Some(replacement_ui_thread_id.clone()),
                state: AgentChatWarmSessionState::Preparing,
                connection: None,
                acquired_at: None,
                failure: None,
            };
            let snapshot = replacement.snapshot();
            inner.slots.insert(key.clone(), replacement);
            (
                snapshot,
                spec,
                replacement_generation,
                replacement_ui_thread_id,
            )
        };

        let manager = self.clone();
        std::thread::spawn(move || {
            if cancel_old_turn {
                let _ = old_connection.cancel_turn(old_ui_thread_id);
            }
            drop(lease);
            let slot = manager.prepare_slot_with_generation(
                spec,
                replacement_generation,
                replacement_ui_thread_id,
            );
            manager.insert_prepared_slot_if_current(slot, replacement_generation);
        });

        snapshot
    }

    pub(crate) fn dismiss_reset(
        &self,
        lease: AgentChatWarmSessionLease,
    ) -> Result<AgentChatWarmSessionSnapshot> {
        let spec = {
            let inner = self.inner.lock();
            let Some(slot) = inner.slots.get(&lease.key) else {
                return Ok(AgentChatWarmSessionSnapshot {
                    key: lease.key,
                    generation: lease.generation,
                    ui_thread_id: None,
                    state: AgentChatWarmSessionState::Empty,
                    failure: None,
                });
            };

            if slot.generation != lease.generation
                || slot.state != AgentChatWarmSessionState::Acquired
            {
                return Ok(slot.snapshot());
            }

            slot.spec.clone()
        };

        let _ = lease.connection.cancel_turn(lease.ui_thread_id.clone());
        drop(lease);

        let slot = self.prepare_new_slot(spec);
        let snapshot = slot.snapshot();
        self.inner.lock().slots.insert(snapshot.key.clone(), slot);
        Ok(snapshot)
    }

    pub(crate) fn snapshot(&self, key: &str) -> Option<AgentChatWarmSessionSnapshot> {
        self.inner.lock().slots.get(key).map(WarmSlot::snapshot)
    }

    pub(crate) fn cleanup_stale_acquired(
        &self,
        ttl: Duration,
    ) -> Vec<AgentChatWarmSessionSnapshot> {
        let now = Instant::now();
        let stale = {
            let inner = self.inner.lock();
            inner
                .slots
                .iter()
                .filter_map(|(key, slot)| {
                    if slot.state != AgentChatWarmSessionState::Acquired {
                        return None;
                    }
                    let acquired_at = slot.acquired_at?;
                    if now.duration_since(acquired_at) < ttl {
                        return None;
                    }
                    let connection = slot.connection.as_ref()?.clone();
                    let ui_thread_id = slot.ui_thread_id.clone()?;
                    Some((
                        key.clone(),
                        slot.spec.clone(),
                        AgentChatWarmSessionLease {
                            key: key.clone(),
                            generation: slot.generation,
                            ui_thread_id,
                            cwd: slot.spec.cwd.clone(),
                            connection,
                        },
                    ))
                })
                .collect::<Vec<_>>()
        };

        let mut snapshots = Vec::new();
        for (key, spec, lease) in stale {
            let _ = lease.connection.cancel_turn(lease.ui_thread_id.clone());
            drop(lease);
            let slot = self.prepare_new_slot(spec);
            let snapshot = slot.snapshot();
            self.inner.lock().slots.insert(key, slot);
            snapshots.push(snapshot);
        }
        snapshots
    }

    fn prepare_new_slot(&self, spec: AgentChatWarmSessionSpec) -> WarmSlot {
        let generation = {
            let mut inner = self.inner.lock();
            inner.next_generation += 1;
            inner.next_generation
        };
        let ui_thread_id = (self.ui_thread_id_source)();

        self.prepare_slot_with_generation(spec, generation, ui_thread_id)
    }

    /// Expose the freshly spawned connection on a still-`Preparing` slot so
    /// `acquire_preparing_join` can hand it out while warm-up continues.
    fn publish_preparing_connection(
        &self,
        key: &str,
        generation: u64,
        connection: &Arc<dyn AgentChatConnection>,
    ) {
        let mut inner = self.inner.lock();
        let Some(slot) = inner.slots.get_mut(key) else {
            return;
        };
        if slot.generation == generation
            && slot.state == AgentChatWarmSessionState::Preparing
            && slot.connection.is_none()
        {
            slot.connection = Some(connection.clone());
        }
    }

    fn insert_prepared_slot_if_current(&self, slot: WarmSlot, generation: u64) {
        let mut inner = self.inner.lock();
        let Some(current) = inner.slots.get(&slot.spec.key) else {
            let snapshot = slot.snapshot();
            inner.slots.insert(snapshot.key.clone(), slot);
            return;
        };
        if current.generation != generation || current.state != AgentChatWarmSessionState::Preparing
        {
            return;
        }

        inner.slots.insert(slot.spec.key.clone(), slot);
    }

    fn prepare_slot_with_generation(
        &self,
        spec: AgentChatWarmSessionSpec,
        generation: u64,
        ui_thread_id: String,
    ) -> WarmSlot {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "warm_prepare_slot_begin",
            generation,
            key = %spec.key,
        );
        match spec.factory.spawn_connection() {
            Ok(connection) => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "warm_prepare_slot_connection_spawned",
                    generation,
                    key = %spec.key,
                );
                self.publish_preparing_connection(&spec.key, generation, &connection);
                let prepare_events =
                    connection.prepare_session(ui_thread_id.clone(), spec.cwd.clone());
                let (state, failure) = match prepare_events {
                    Ok(events) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "warm_prepare_slot_session_sent",
                            generation,
                            key = %spec.key,
                        );
                        match wait_for_prepare_ready(events, DEFAULT_PREPARE_READY_TIMEOUT) {
                            PrepareReadyOutcome::Ready => {
                                tracing::info!(
                                    target: "script_kit::tab_ai",
                                    event = "warm_prepare_slot_ready",
                                    generation,
                                    key = %spec.key,
                                );
                                (AgentChatWarmSessionState::Ready, None)
                            }
                            PrepareReadyOutcome::RuntimeFailed(failure) => {
                                tracing::warn!(
                                    target: "script_kit::tab_ai",
                                    event = "warm_prepare_slot_runtime_failed",
                                    generation,
                                    key = %spec.key,
                                    failure_code = ?failure.failure.code,
                                    diagnostic_fingerprint = ?failure.failure.diagnostic.as_ref().map(|d| &d.fingerprint.0),
                                );
                                (AgentChatWarmSessionState::Failed, Some(failure))
                            }
                            PrepareReadyOutcome::Timeout => {
                                tracing::warn!(
                                    target: "script_kit::tab_ai",
                                    event = "warm_prepare_slot_timeout",
                                    generation,
                                    key = %spec.key,
                                );
                                // S11: a warm-up that never reported models is
                                // a connectivity timeout, not free text.
                                (
                                    AgentChatWarmSessionState::Failed,
                                    Some(crate::ai::reliability::provider_failure(
                                        sk_protocol::ai_reliability::ProtocolComponent::Pi,
                                        format!(
                                            "warm prepare timed out after {} ms",
                                            DEFAULT_PREPARE_READY_TIMEOUT.as_millis()
                                        ),
                                    )),
                                )
                            }
                            PrepareReadyOutcome::Closed => {
                                tracing::warn!(
                                    target: "script_kit::tab_ai",
                                    event = "warm_prepare_slot_events_closed",
                                    generation,
                                    key = %spec.key,
                                );
                                // S11: the event stream ending early is a
                                // runtime-closed fact.
                                (
                                    AgentChatWarmSessionState::Failed,
                                    Some(crate::ai::reliability::process_failure(
                                        sk_protocol::ai_reliability::ProtocolComponent::Pi,
                                        crate::ai::reliability::ProcessFailureFacts::RuntimeClosed,
                                    )),
                                )
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "warm_prepare_slot_session_error",
                            generation,
                            key = %spec.key,
                            %error,
                        );
                        // S11: `error` is already a classified
                        // `AiAdapterError`. Formatting it printed the internal
                        // `ai_adapter_error:<Code>` marker on screen; carry the
                        // record instead.
                        (AgentChatWarmSessionState::Failed, Some(error.failure))
                    }
                };

                WarmSlot {
                    spec,
                    generation,
                    ui_thread_id: Some(ui_thread_id),
                    state,
                    connection: (state == AgentChatWarmSessionState::Ready).then_some(connection),
                    acquired_at: None,
                    failure,
                }
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "warm_prepare_slot_spawn_failed",
                    generation,
                    key = %spec.key,
                    %error,
                );
                WarmSlot {
                    spec,
                    generation,
                    ui_thread_id: Some(ui_thread_id),
                    state: AgentChatWarmSessionState::Failed,
                    connection: None,
                    acquired_at: None,
                    failure: Some(crate::ai::reliability::spawn_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Pi,
                        &format!("{error:#}"),
                    )),
                }
            }
        }
    }

    #[cfg(test)]
    fn mark_acquired_as_stale_for_test(&self, key: &str, age: Duration) {
        let mut inner = self.inner.lock();
        if let Some(slot) = inner.slots.get_mut(key) {
            slot.acquired_at = Some(Instant::now() - age);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrepareReadyOutcome {
    Ready,
    RuntimeFailed(crate::ai::reliability::AppFailureRecord),
    Timeout,
    Closed,
}

fn wait_for_prepare_ready(events: AgentChatEventRx, timeout: Duration) -> PrepareReadyOutcome {
    let deadline = Instant::now() + timeout;

    loop {
        match events.try_recv() {
            Ok(AgentChatEvent::ModelsAvailable { .. }) => return PrepareReadyOutcome::Ready,
            Ok(AgentChatEvent::TurnFailed { failure }) => {
                return PrepareReadyOutcome::RuntimeFailed(failure);
            }
            Ok(AgentChatEvent::SetupRequired {
                reason,
                auth_methods,
            }) => {
                // S11: `SetupRequired` is a typed fact from the runtime, not
                // prose to be re-guessed. Routing it through the free-text
                // provider classifier produced `Unknown` (no auth wording in
                // "setup required"/"login required"), so the user got the
                // generic "did not finish" card with no Sign In button.
                return PrepareReadyOutcome::RuntimeFailed(
                    crate::ai::reliability::setup_required_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Pi,
                        &reason,
                        &auth_methods,
                    ),
                );
            }
            Ok(_) => continue,
            Err(async_channel::TryRecvError::Empty) => {
                if Instant::now() >= deadline {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_warm_prepare_ready_timeout",
                        timeout_ms = timeout.as_millis(),
                    );
                    return PrepareReadyOutcome::Timeout;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(async_channel::TryRecvError::Closed) => return PrepareReadyOutcome::Closed,
        }
    }
}

fn default_warm_ui_thread_id() -> String {
    format!("warm:{}", uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::events::AgentChatEventRx;
    use crate::ai::agent_chat::runtime::AgentChatTurnRequest;
    use parking_lot::Mutex;
    use sk_protocol::ai_reliability::AiFailureCode;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;

    #[derive(Default)]
    struct RecordingConnection {
        prepare_calls: Mutex<Vec<(String, PathBuf)>>,
        cancel_calls: Mutex<Vec<String>>,
        fail_prepare: bool,
        prepare_event: Option<AgentChatEvent>,
    }

    impl RecordingConnection {
        fn prepare_calls(&self) -> Vec<(String, PathBuf)> {
            self.prepare_calls.lock().clone()
        }

        fn cancel_calls(&self) -> Vec<String> {
            self.cancel_calls.lock().clone()
        }
    }

    impl AgentChatConnection for RecordingConnection {
        fn start_turn(
            &self,
            _request: AgentChatTurnRequest,
        ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
            let (_tx, rx) = async_channel::bounded(1);
            Ok(rx)
        }

        fn cancel_turn(&self, ui_thread_id: String) -> crate::ai::reliability::AiAdapterResult<()> {
            self.cancel_calls.lock().push(ui_thread_id);
            Ok(())
        }

        fn prepare_session(
            &self,
            ui_thread_id: String,
            cwd: PathBuf,
        ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
            (|| -> Result<AgentChatEventRx> {
                self.prepare_calls.lock().push((ui_thread_id, cwd));
                if self.fail_prepare {
                    anyhow::bail!("prepare failed");
                }
                let (tx, rx) = async_channel::bounded(1);
                if let Some(event) = self.prepare_event.clone() {
                    tx.send_blocking(event)?;
                }
                Ok(rx)
            })()
            .map_err(Into::into)
        }
    }

    #[derive(Default)]
    struct RecordingFactory {
        spawned: Mutex<Vec<Arc<RecordingConnection>>>,
        fail_next_prepare: Mutex<bool>,
        next_prepare_event: Mutex<Option<AgentChatEvent>>,
    }

    impl RecordingFactory {
        fn spawned(&self) -> Vec<Arc<RecordingConnection>> {
            self.spawned.lock().clone()
        }

        fn fail_next_prepare(&self) {
            *self.fail_next_prepare.lock() = true;
        }

        fn set_next_prepare_event(&self, event: Option<AgentChatEvent>) {
            *self.next_prepare_event.lock() = event;
        }
    }

    impl AgentChatWarmRuntimeFactory for RecordingFactory {
        fn spawn_connection(&self) -> Result<Arc<dyn AgentChatConnection>> {
            let fail_prepare = std::mem::take(&mut *self.fail_next_prepare.lock());
            let prepare_event = self.next_prepare_event.lock().take().unwrap_or_else(|| {
                AgentChatEvent::ModelsAvailable {
                    current_model_id: None,
                    models: Vec::new(),
                }
            });
            let connection = Arc::new(RecordingConnection {
                fail_prepare,
                prepare_event: Some(prepare_event),
                ..Default::default()
            });
            self.spawned.lock().push(connection.clone());
            Ok(connection)
        }
    }

    struct BlockingFactory {
        spawned: AtomicUsize,
        started_tx: Mutex<Option<mpsc::Sender<()>>>,
        release_rx: Mutex<mpsc::Receiver<()>>,
    }

    impl BlockingFactory {
        fn new(started_tx: mpsc::Sender<()>, release_rx: mpsc::Receiver<()>) -> Self {
            Self {
                spawned: AtomicUsize::new(0),
                started_tx: Mutex::new(Some(started_tx)),
                release_rx: Mutex::new(release_rx),
            }
        }

        fn spawned_count(&self) -> usize {
            self.spawned.load(Ordering::SeqCst)
        }
    }

    impl AgentChatWarmRuntimeFactory for BlockingFactory {
        fn spawn_connection(&self) -> Result<Arc<dyn AgentChatConnection>> {
            self.spawned.fetch_add(1, Ordering::SeqCst);
            if let Some(started_tx) = self.started_tx.lock().take() {
                let _ = started_tx.send(());
            }
            let _ = self.release_rx.lock().recv();
            Ok(Arc::new(RecordingConnection {
                prepare_event: Some(AgentChatEvent::ModelsAvailable {
                    current_model_id: None,
                    models: Vec::new(),
                }),
                ..Default::default()
            }))
        }
    }

    /// Spawns instantly but blocks inside `prepare_session` until released,
    /// holding the slot in `Preparing` with a published connection.
    struct GatedPrepareFactory {
        spawned: AtomicUsize,
        started_tx: Mutex<Option<mpsc::Sender<()>>>,
        release_rx: Mutex<Option<mpsc::Receiver<()>>>,
    }

    impl GatedPrepareFactory {
        fn new(started_tx: mpsc::Sender<()>, release_rx: mpsc::Receiver<()>) -> Self {
            Self {
                spawned: AtomicUsize::new(0),
                started_tx: Mutex::new(Some(started_tx)),
                release_rx: Mutex::new(Some(release_rx)),
            }
        }

        fn spawned_count(&self) -> usize {
            self.spawned.load(Ordering::SeqCst)
        }
    }

    impl AgentChatWarmRuntimeFactory for GatedPrepareFactory {
        fn spawn_connection(&self) -> Result<Arc<dyn AgentChatConnection>> {
            self.spawned.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(GatedPrepareConnection {
                started_tx: Mutex::new(self.started_tx.lock().take()),
                release_rx: Mutex::new(self.release_rx.lock().take()),
            }))
        }
    }

    struct GatedPrepareConnection {
        started_tx: Mutex<Option<mpsc::Sender<()>>>,
        release_rx: Mutex<Option<mpsc::Receiver<()>>>,
    }

    impl AgentChatConnection for GatedPrepareConnection {
        fn start_turn(
            &self,
            _request: AgentChatTurnRequest,
        ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
            let (_tx, rx) = async_channel::bounded(1);
            Ok(rx)
        }

        fn cancel_turn(
            &self,
            _ui_thread_id: String,
        ) -> crate::ai::reliability::AiAdapterResult<()> {
            Ok(())
        }

        fn prepare_session(
            &self,
            _ui_thread_id: String,
            _cwd: PathBuf,
        ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
            if let Some(tx) = self.started_tx.lock().take() {
                let _ = tx.send(());
            }
            if let Some(rx) = self.release_rx.lock().take() {
                let _ = rx.recv();
            }
            let (tx, rx) = async_channel::bounded(1);
            let _ = tx.send_blocking(AgentChatEvent::ModelsAvailable {
                current_model_id: None,
                models: Vec::new(),
            });
            Ok(rx)
        }
    }

    struct SpawnFailFactory;

    impl AgentChatWarmRuntimeFactory for SpawnFailFactory {
        fn spawn_connection(&self) -> Result<Arc<dyn AgentChatConnection>> {
            anyhow::bail!("spawn failed")
        }
    }

    fn manager() -> AgentChatWarmSessionManager {
        let counter = Arc::new(AtomicUsize::new(0));
        AgentChatWarmSessionManager::with_ui_thread_id_source(Arc::new(move || {
            let next = counter.fetch_add(1, Ordering::SeqCst) + 1;
            format!("thread-{next}")
        }))
    }

    fn spec(factory: Arc<RecordingFactory>) -> AgentChatWarmSessionSpec {
        AgentChatWarmSessionSpec {
            key: "key-a".to_string(),
            cwd: PathBuf::from("/tmp/a"),
            factory,
        }
    }

    fn spec_with_factory(
        factory: Arc<dyn AgentChatWarmRuntimeFactory>,
    ) -> AgentChatWarmSessionSpec {
        AgentChatWarmSessionSpec {
            key: "key-a".to_string(),
            cwd: PathBuf::from("/tmp/a"),
            factory,
        }
    }

    #[test]
    fn prepare_warm_spawns_runtime_and_prepares_session_once() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        let snapshot = manager.prepare_warm(spec(factory.clone())).unwrap();

        assert_eq!(snapshot.state, AgentChatWarmSessionState::Ready);
        assert_eq!(snapshot.ui_thread_id.as_deref(), Some("thread-1"));
        assert_eq!(factory.spawned().len(), 1);
        assert_eq!(
            factory.spawned()[0].prepare_calls(),
            vec![("thread-1".to_string(), PathBuf::from("/tmp/a"))]
        );
    }

    #[test]
    fn prepare_warm_is_idempotent_for_ready_key() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        let first = manager.prepare_warm(spec(factory.clone())).unwrap();
        let second = manager.prepare_warm(spec(factory.clone())).unwrap();

        assert_eq!(first, second);
        assert_eq!(factory.spawned().len(), 1);
    }

    #[test]
    fn prepare_warm_reserves_preparing_slot_before_spawning_runtime() {
        let manager = Arc::new(manager());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let factory = Arc::new(BlockingFactory::new(started_tx, release_rx));

        let first_manager = manager.clone();
        let first_factory: Arc<dyn AgentChatWarmRuntimeFactory> = factory.clone();
        let first_prepare = thread::spawn(move || {
            first_manager
                .prepare_warm(spec_with_factory(first_factory))
                .expect("first prepare")
        });

        started_rx.recv().expect("first prepare started spawning");

        let second_factory: Arc<dyn AgentChatWarmRuntimeFactory> = factory.clone();
        let second = manager
            .prepare_warm(spec_with_factory(second_factory))
            .expect("second prepare");

        assert_eq!(second.state, AgentChatWarmSessionState::Preparing);
        assert_eq!(second.ui_thread_id.as_deref(), Some("thread-1"));
        assert_eq!(factory.spawned_count(), 1);

        release_tx.send(()).expect("release first prepare");
        let first = first_prepare.join().expect("first prepare thread");
        assert_eq!(first.state, AgentChatWarmSessionState::Ready);
        assert_eq!(factory.spawned_count(), 1);
        assert_eq!(
            manager.snapshot("key-a").unwrap().state,
            AgentChatWarmSessionState::Ready
        );
    }

    #[test]
    fn prepare_warm_background_returns_preparing_without_waiting_for_runtime() {
        let manager = manager();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let factory = Arc::new(BlockingFactory::new(started_tx, release_rx));

        let snapshot = manager
            .prepare_warm_background(spec_with_factory(factory.clone()))
            .expect("background prepare");

        assert_eq!(snapshot.state, AgentChatWarmSessionState::Preparing);
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("background prepare started spawning");
        assert_eq!(factory.spawned_count(), 1);
        assert_eq!(
            manager.snapshot("key-a").unwrap().state,
            AgentChatWarmSessionState::Preparing
        );

        release_tx.send(()).expect("release background prepare");
        for _ in 0..100 {
            if manager.snapshot("key-a").unwrap().state == AgentChatWarmSessionState::Ready {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("background prepare did not mark slot ready");
    }

    #[test]
    fn prepare_warm_background_is_idempotent_while_preparing() {
        let manager = manager();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let factory = Arc::new(BlockingFactory::new(started_tx, release_rx));

        let first = manager
            .prepare_warm_background(spec_with_factory(factory.clone()))
            .expect("first background prepare");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("background prepare started spawning");
        let second = manager
            .prepare_warm_background(spec_with_factory(factory.clone()))
            .expect("second background prepare");

        assert_eq!(first, second);
        assert_eq!(factory.spawned_count(), 1);

        release_tx.send(()).expect("release background prepare");
    }

    #[test]
    fn background_thread_spawn_failure_marks_same_generation_failed_and_returns_error() {
        let manager = manager();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let factory = Arc::new(BlockingFactory::new(started_tx, release_rx));
        let snapshot = manager
            .prepare_warm_background(spec_with_factory(factory))
            .expect("reserve background generation");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("real worker reached the blocking factory");

        let error = manager
            .finish_background_thread_spawn(
                &snapshot,
                Err(std::io::Error::other("synthetic spawn failure")),
            )
            .expect_err("spawn failure must be returned");
        let failed = manager.snapshot("key-a").expect("failed slot retained");

        // S11: the raw OS cause is captured into the diagnostic vault, not
        // into anything a user or log line can read. What the slot carries is
        // the classified fact plus a fingerprint that can find the cause.
        assert_eq!(failed.generation, snapshot.generation);
        assert_eq!(failed.state, AgentChatWarmSessionState::Failed);
        let failure = failed.failure.as_ref().expect("classified spawn failure");
        assert_eq!(failure.failure.code, AiFailureCode::SpawnFailed);
        assert!(
            failure.failure.diagnostic.is_some(),
            "the spawn cause must still be recoverable through Copy Details"
        );
        assert_eq!(error.to_string(), failure.primary_message());
        assert!(
            !error.to_string().contains("synthetic spawn failure"),
            "the raw OS cause must not travel in the returned error"
        );

        release_tx.send(()).expect("release real worker");
    }

    #[test]
    fn acquire_warm_returns_ready_session_and_marks_it_acquired() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        let prepared = manager.prepare_warm(spec(factory)).unwrap();
        let lease = manager.acquire_warm("key-a").unwrap();
        let snapshot = manager.snapshot("key-a").unwrap();

        assert_eq!(lease.key, "key-a");
        assert_eq!(lease.generation, prepared.generation);
        assert_eq!(lease.ui_thread_id, "thread-1");
        assert_eq!(snapshot.state, AgentChatWarmSessionState::Acquired);
    }

    #[test]
    fn acquire_warm_consumes_ready_session_exactly_once() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory)).unwrap();
        assert!(manager.acquire_warm("key-a").is_some());
        assert!(manager.acquire_warm("key-a").is_none());
    }

    #[test]
    fn acquire_warm_misses_for_wrong_key_without_spawning() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        assert!(manager.acquire_warm("key-b").is_none());
        assert_eq!(factory.spawned().len(), 1);
    }

    #[test]
    fn acquire_ready_or_spawn_cold_uses_ready_slot_when_available() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        let prepared = manager.prepare_warm(spec(factory.clone())).unwrap();
        let (lease, origin) = manager
            .acquire_ready_or_spawn_cold(spec(factory.clone()))
            .expect("acquire ready");
        let snapshot = manager.snapshot("key-a").unwrap();

        assert_eq!(origin, AgentChatWarmAcquireOrigin::Ready);
        assert_eq!(lease.generation, prepared.generation);
        assert_eq!(lease.ui_thread_id, "thread-1");
        assert_eq!(snapshot.state, AgentChatWarmSessionState::Acquired);
        assert_eq!(factory.spawned().len(), 1);
    }

    #[test]
    fn acquire_ready_or_spawn_cold_creates_acquired_lease_when_empty() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        let (lease, origin) = manager
            .acquire_ready_or_spawn_cold(spec(factory.clone()))
            .expect("cold spawn");
        let snapshot = manager.snapshot("key-a").unwrap();

        assert_eq!(origin, AgentChatWarmAcquireOrigin::ColdSpawned);
        assert_eq!(lease.generation, 1);
        assert_eq!(lease.ui_thread_id, "thread-1");
        assert_eq!(lease.cwd, PathBuf::from("/tmp/a"));
        assert_eq!(snapshot.state, AgentChatWarmSessionState::Acquired);
        assert_eq!(snapshot.generation, 1);
        assert_eq!(factory.spawned().len(), 1);
        assert!(factory.spawned()[0].prepare_calls().is_empty());
    }

    #[test]
    fn acquire_ready_or_spawn_cold_replaces_preparing_slot_without_waiting() {
        let manager = manager();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let blocking_factory = Arc::new(BlockingFactory::new(started_tx, release_rx));

        manager
            .prepare_warm_background(spec_with_factory(blocking_factory.clone()))
            .expect("background prepare");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("background prepare started");

        let cold_factory = Arc::new(RecordingFactory::default());
        let (lease, origin) = manager
            .acquire_ready_or_spawn_cold(spec(cold_factory.clone()))
            .expect("cold spawn");
        assert_eq!(origin, AgentChatWarmAcquireOrigin::ColdSpawned);
        assert_eq!(lease.generation, 2);
        assert_eq!(
            manager.snapshot("key-a").unwrap().state,
            AgentChatWarmSessionState::Acquired
        );

        release_tx.send(()).expect("release stale prepare");
        for _ in 0..100 {
            if blocking_factory.spawned_count() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let snapshot = manager.snapshot("key-a").unwrap();
        assert_eq!(snapshot.state, AgentChatWarmSessionState::Acquired);
        assert_eq!(snapshot.generation, 2);
        assert_eq!(cold_factory.spawned().len(), 1);
    }

    #[test]
    fn acquire_ready_or_spawn_cold_joins_preparing_slot_with_spawned_connection() {
        let manager = manager();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let factory = Arc::new(GatedPrepareFactory::new(started_tx, release_rx));

        let gated_factory: Arc<dyn AgentChatWarmRuntimeFactory> = factory.clone();
        manager
            .prepare_warm_background(spec_with_factory(gated_factory.clone()))
            .expect("background prepare");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("prepare_session entered");

        let (lease, origin) = manager
            .acquire_ready_or_spawn_cold(spec_with_factory(gated_factory))
            .expect("join in-flight prepare");

        assert_eq!(origin, AgentChatWarmAcquireOrigin::JoinedPreparing);
        assert_eq!(lease.generation, 1);
        assert_eq!(lease.ui_thread_id, "thread-1");
        assert_eq!(factory.spawned_count(), 1);
        assert_eq!(
            manager.snapshot("key-a").unwrap().state,
            AgentChatWarmSessionState::Acquired
        );

        release_tx.send(()).expect("release prepare");
        // The prepare thread finishing must not clobber the joined acquisition.
        std::thread::sleep(Duration::from_millis(100));
        let snapshot = manager.snapshot("key-a").unwrap();
        assert_eq!(snapshot.state, AgentChatWarmSessionState::Acquired);
        assert_eq!(snapshot.generation, 1);
        assert_eq!(factory.spawned_count(), 1);
    }

    #[test]
    fn cold_acquired_dismiss_reset_background_prepares_replacement() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        let (lease, origin) = manager
            .acquire_ready_or_spawn_cold(spec(factory.clone()))
            .expect("cold spawn");
        assert_eq!(origin, AgentChatWarmAcquireOrigin::ColdSpawned);
        let replacement = manager.dismiss_reset_background(lease);

        assert_eq!(replacement.state, AgentChatWarmSessionState::Preparing);
        assert_eq!(replacement.generation, 2);
        assert_eq!(replacement.ui_thread_id.as_deref(), Some("thread-2"));
        for _ in 0..100 {
            if manager.snapshot("key-a").unwrap().state == AgentChatWarmSessionState::Ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            factory.spawned()[0].cancel_calls(),
            vec!["thread-1".to_string()]
        );
        assert_eq!(
            factory.spawned()[1].prepare_calls(),
            vec![("thread-2".to_string(), PathBuf::from("/tmp/a"))]
        );
    }

    #[test]
    fn acquire_ready_or_spawn_cold_marks_spawn_failure_failed() {
        let manager = manager();
        let error = match manager
            .acquire_ready_or_spawn_cold(spec_with_factory(Arc::new(SpawnFailFactory)))
        {
            Ok(_) => panic!("spawn should fail"),
            Err(error) => error,
        };
        let snapshot = manager.snapshot("key-a").unwrap();

        assert_eq!(snapshot.state, AgentChatWarmSessionState::Failed);
        assert_eq!(snapshot.generation, 1);
        let failure = snapshot.failure.as_ref().expect("classified spawn failure");
        assert_eq!(failure.failure.code, AiFailureCode::SpawnFailed);
        assert_eq!(error.to_string(), failure.primary_message());
        assert!(
            !error.to_string().contains("spawn failed"),
            "the factory's raw cause must not reach the caller's error"
        );
    }

    #[test]
    fn release_for_detach_handoff_prepares_replacement_without_cancelling_live_turn() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        let lease = manager.acquire_warm("key-a").unwrap();
        let replacement = manager.release_for_detach_handoff(lease);

        assert_eq!(replacement.state, AgentChatWarmSessionState::Preparing);
        assert_eq!(replacement.generation, 2);
        assert_eq!(replacement.ui_thread_id.as_deref(), Some("thread-2"));
        for _ in 0..100 {
            if manager.snapshot("key-a").unwrap().state == AgentChatWarmSessionState::Ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // The detached window keeps streaming on the old connection: the
        // hand-off must NOT cancel its in-flight turn.
        assert!(factory.spawned()[0].cancel_calls().is_empty());
        assert_eq!(
            factory.spawned()[1].prepare_calls(),
            vec![("thread-2".to_string(), PathBuf::from("/tmp/a"))]
        );
    }

    #[test]
    fn dismiss_reset_cancels_acquired_session() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        let lease = manager.acquire_warm("key-a").unwrap();
        manager.dismiss_reset(lease).unwrap();

        assert_eq!(factory.spawned()[0].cancel_calls(), vec!["thread-1"]);
    }

    #[test]
    fn dismiss_reset_prepares_replacement_with_new_ui_thread_id() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        let lease = manager.acquire_warm("key-a").unwrap();
        let replacement = manager.dismiss_reset(lease).unwrap();

        assert_eq!(replacement.state, AgentChatWarmSessionState::Ready);
        assert_eq!(replacement.generation, 2);
        assert_eq!(replacement.ui_thread_id.as_deref(), Some("thread-2"));
        assert_eq!(
            factory.spawned()[1].prepare_calls(),
            vec![("thread-2".to_string(), PathBuf::from("/tmp/a"))]
        );
    }

    #[test]
    fn dismiss_reset_ignores_stale_generation_without_double_replacement() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        let lease = manager.acquire_warm("key-a").unwrap();
        let mut stale = lease.clone();
        stale.generation = 0;

        let stale_snapshot = manager.dismiss_reset(stale).unwrap();
        assert_eq!(stale_snapshot.state, AgentChatWarmSessionState::Acquired);
        assert_eq!(factory.spawned().len(), 1);

        manager.dismiss_reset(lease).unwrap();
        assert_eq!(factory.spawned().len(), 2);
    }

    #[test]
    fn cleanup_stale_acquired_cancels_and_prepares_replacement() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        let lease = manager.acquire_warm("key-a").unwrap();
        assert_eq!(lease.ui_thread_id, "thread-1");
        manager.mark_acquired_as_stale_for_test("key-a", Duration::from_secs(120));

        let snapshots = manager.cleanup_stale_acquired(Duration::from_secs(60));

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].state, AgentChatWarmSessionState::Ready);
        assert_eq!(snapshots[0].generation, 2);
        assert_eq!(factory.spawned()[0].cancel_calls(), vec!["thread-1"]);
        assert_eq!(
            factory.spawned()[1].prepare_calls(),
            vec![("thread-2".to_string(), PathBuf::from("/tmp/a"))]
        );
    }

    #[test]
    fn cleanup_stale_acquired_leaves_recent_acquired_session_alone() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());

        manager.prepare_warm(spec(factory.clone())).unwrap();
        let _lease = manager.acquire_warm("key-a").unwrap();

        let snapshots = manager.cleanup_stale_acquired(Duration::from_secs(60));

        assert!(snapshots.is_empty());
        assert_eq!(
            manager.snapshot("key-a").unwrap().state,
            AgentChatWarmSessionState::Acquired
        );
        assert_eq!(factory.spawned().len(), 1);
        assert!(factory.spawned()[0].cancel_calls().is_empty());
    }

    #[test]
    fn failed_prepare_marks_failed_and_next_prepare_retries() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());
        factory.fail_next_prepare();

        let failed = manager.prepare_warm(spec(factory.clone())).unwrap();
        assert_eq!(failed.state, AgentChatWarmSessionState::Failed);
        // S11: `prepare_session` returns an already-classified adapter error.
        // Formatting it used to print the internal `ai_adapter_error:Unknown`
        // marker into the Agent Chat surface.
        let failure = failed.failure.as_ref().expect("classified prepare failure");
        assert!(
            !failure.primary_message().contains("ai_adapter_error"),
            "internal adapter markers must never become user copy"
        );
        assert!(
            failure.failure.diagnostic.is_some(),
            "the adapter's cause must stay recoverable through Copy Details"
        );

        let retry = manager.prepare_warm(spec(factory.clone())).unwrap();
        assert_eq!(retry.state, AgentChatWarmSessionState::Ready);
        assert_eq!(retry.generation, 2);
        assert_eq!(factory.spawned().len(), 2);
    }

    #[test]
    fn prepare_warm_requires_models_available_readiness_event() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());
        factory.set_next_prepare_event(Some(AgentChatEvent::completed("ignored")));

        let snapshot = manager.prepare_warm(spec(factory.clone())).unwrap();

        assert_eq!(snapshot.state, AgentChatWarmSessionState::Failed);
        let failure = snapshot.failure.as_ref().expect("classified stream close");
        assert_eq!(failure.failure.code, AiFailureCode::RuntimeClosed);
        assert!(manager.acquire_warm("key-a").is_none());
    }

    #[test]
    fn prepare_warm_marks_failed_when_prepare_reports_setup_required() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());
        factory.set_next_prepare_event(Some(AgentChatEvent::SetupRequired {
            reason: "login required".to_string(),
            auth_methods: vec!["browser".to_string()],
        }));

        let snapshot = manager.prepare_warm(spec(factory.clone())).unwrap();

        // S11: `SetupRequired` is a typed fact. Routing it through the
        // free-text classifier produced `Unknown` and the generic
        // "did not finish" card — with no Sign In button, on the one failure
        // Sign In fixes.
        assert_eq!(snapshot.state, AgentChatWarmSessionState::Failed);
        let failure = snapshot
            .failure
            .as_ref()
            .expect("classified setup-required");
        assert_eq!(failure.failure.code, AiFailureCode::AuthenticationMissing);
        assert_eq!(
            failure.primary_message(),
            "Sign in to continue with this AI provider."
        );
        assert_eq!(factory.spawned().len(), 1);
    }

    #[test]
    fn failed_generation_reprepare_is_generation_checked_and_duplicate_safe() {
        let manager = manager();
        let factory = Arc::new(RecordingFactory::default());
        factory.fail_next_prepare();
        let failed = manager.prepare_warm(spec(factory.clone())).unwrap();
        assert_eq!(failed.state, AgentChatWarmSessionState::Failed);

        let first = manager
            .reprepare_failed_generation_background(spec(factory.clone()), failed.generation);
        let second = manager
            .reprepare_failed_generation_background(spec(factory.clone()), failed.generation);

        assert!(matches!(first, AgentChatWarmReprepareResult::Started(_)));
        assert!(matches!(second, AgentChatWarmReprepareResult::Current(_)));
        for _ in 0..100 {
            if manager.snapshot("key-a").unwrap().state == AgentChatWarmSessionState::Ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            factory.spawned().len(),
            2,
            "one initial failure plus one retry"
        );
        assert_eq!(
            manager.snapshot("key-a").unwrap().generation,
            failed.generation + 1
        );
    }
}

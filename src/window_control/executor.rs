//! Per-PID mutation executors.
//!
//! One lazily created standard thread per active PID (`window-ax-{pid}`),
//! bounded queue of 32 commands, 30-second idle timeout. Same-PID work
//! serializes on the worker; different PIDs proceed concurrently. Workers
//! never receive raw AX pointers — they resolve the currently retained
//! reference (or the provider window) on their own thread. Every envelope
//! carries a cancellation flag checked immediately after dequeue, before the
//! first write, and before every retry.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use super::app_profiles::{resolve_profile, AppMutationProfile};
use super::mutation::{
    activate_application, apply_bounds_sequence, provider_close, provider_focus,
    provider_set_bounds, provider_set_minimized, provider_set_position, provider_set_size,
    set_window_minimized, AxMessagingTimeoutGuard, AX_MUTATION_TIMEOUT_SECS,
    BOUNDS_VERIFY_DEADLINE, CLOSE_VERIFY_DEADLINE, FOCUS_VERIFY_DEADLINE, MINIMIZE_VERIFY_DEADLINE,
    VERIFY_POLL_INTERVAL,
};
use super::plan::{PlannedWindowMutation, RequestedMutation};
use super::types::Bounds;
use super::verification::{request_satisfied, ObservedState};

const WORKER_QUEUE_DEPTH: usize = 32;
const WORKER_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// A command executed on the target PID's worker thread.
pub(super) enum WorkerCommand {
    /// Validate identity/capability and snapshot the before-state.
    Preflight(PlannedWindowMutation),
    /// Apply the mutation with attempts + readback verification.
    Apply(PlannedWindowMutation),
    /// Restore a previous state (rollback/undo path, S11).
    Restore { operation: PlannedWindowMutation },
}

/// One verified (or failed) attempt.
#[derive(Debug, Clone)]
pub struct MutationAttempt {
    pub attempt: u8,
    pub setter_error: Option<String>,
    pub observed_bounds: Option<Bounds>,
    pub verified: bool,
}

/// Worker reply for one command.
#[derive(Debug, Clone)]
pub(super) struct WorkerReply {
    pub before: Option<ObservedState>,
    pub after: Option<ObservedState>,
    pub attempts: Vec<MutationAttempt>,
    pub error: Option<String>,
    pub queue_wait: Duration,
}

pub(super) struct WorkerEnvelope {
    command: WorkerCommand,
    cancelled: Arc<AtomicBool>,
    enqueued_at: Instant,
    reply: SyncSender<WorkerReply>,
}

struct WorkerEntry {
    sender: SyncSender<WorkerEnvelope>,
    alive: Arc<AtomicBool>,
}

static PID_EXECUTORS: std::sync::LazyLock<parking_lot::Mutex<HashMap<i32, WorkerEntry>>> =
    std::sync::LazyLock::new(Default::default);

/// Submit a command to the PID's worker. Fails fast when the queue is full.
pub(super) fn submit(
    pid: i32,
    command: WorkerCommand,
    cancelled: Arc<AtomicBool>,
) -> Result<std::sync::mpsc::Receiver<WorkerReply>> {
    let (reply_sender, reply_receiver) = std::sync::mpsc::sync_channel(1);
    let envelope = WorkerEnvelope {
        command,
        cancelled,
        enqueued_at: Instant::now(),
        reply: reply_sender,
    };

    let mut executors = PID_EXECUTORS.lock();
    // Prune dead workers on every submission.
    executors.retain(|_, entry| entry.alive.load(Ordering::SeqCst));

    let entry = executors.entry(pid).or_insert_with(|| spawn_worker(pid));
    match entry.sender.try_send(envelope) {
        Ok(()) => Ok(reply_receiver),
        Err(TrySendError::Full(_)) => bail!("window_engine:pid_queue_full"),
        Err(TrySendError::Disconnected(envelope)) => {
            // Worker exited between the alive check and the send: respawn once.
            let entry = spawn_worker(pid);
            let result = entry.sender.try_send(envelope);
            executors.insert(pid, entry);
            match result {
                Ok(()) => Ok(reply_receiver),
                Err(_) => bail!("window_engine:worker_unavailable"),
            }
        }
    }
}

fn spawn_worker(pid: i32) -> WorkerEntry {
    let (sender, receiver) = std::sync::mpsc::sync_channel::<WorkerEnvelope>(WORKER_QUEUE_DEPTH);
    let alive = Arc::new(AtomicBool::new(true));
    let worker_alive = Arc::clone(&alive);
    let _ = std::thread::Builder::new()
        .name(format!("window-ax-{pid}"))
        .spawn(move || {
            loop {
                match receiver.recv_timeout(WORKER_IDLE_TIMEOUT) {
                    Ok(envelope) => {
                        let queue_wait = envelope.enqueued_at.elapsed();
                        // Cancellation check immediately after dequeue.
                        if envelope.cancelled.load(Ordering::SeqCst) {
                            let _ = envelope.reply.try_send(WorkerReply {
                                before: None,
                                after: None,
                                attempts: Vec::new(),
                                error: Some("window_engine:cancelled".to_string()),
                                queue_wait,
                            });
                            continue;
                        }
                        let cancelled = Arc::clone(&envelope.cancelled);
                        let command = envelope.command;
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                                run_command(command, &cancelled)
                            }));
                        let mut reply = match outcome {
                            Ok(reply) => reply,
                            Err(_) => WorkerReply {
                                before: None,
                                after: None,
                                attempts: Vec::new(),
                                error: Some("window_engine:worker_panicked".to_string()),
                                queue_wait,
                            },
                        };
                        reply.queue_wait = queue_wait;
                        let _ = envelope.reply.try_send(reply);
                    }
                    Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
            worker_alive.store(false, Ordering::SeqCst);
        });
    WorkerEntry { sender, alive }
}

fn failure(error: impl Into<String>) -> WorkerReply {
    WorkerReply {
        before: None,
        after: None,
        attempts: Vec::new(),
        error: Some(error.into()),
        queue_wait: Duration::ZERO,
    }
}

/// Observe the target's current state on the worker thread.
fn observe(operation: &PlannedWindowMutation) -> Result<ObservedState> {
    if super::test_support::is_active() {
        let observation = super::registry::resolve_handle(operation.target)?;
        let state = super::test_support::window_state(observation.legacy_id);
        return Ok(match state {
            Ok(state) => ObservedState {
                bounds: state.bounds,
                minimized: state.minimized,
                focused: state.focused,
                alive: true,
            },
            Err(_) => ObservedState {
                bounds: Bounds::new(0, 0, 0, 0),
                minimized: false,
                focused: false,
                alive: false,
            },
        });
    }
    let window = super::registry::retained_window(operation.target)?;
    let position = super::ax::get_window_position(window.as_ptr());
    let size = super::ax::get_window_size(window.as_ptr());
    match (position, size) {
        (Ok((x, y)), Ok((width, height))) => Ok(ObservedState {
            bounds: Bounds::new(x, y, width, height),
            minimized: super::ax::get_window_bool_attribute(window.as_ptr(), "AXMinimized")
                .unwrap_or(false),
            focused: super::ax::get_window_bool_attribute(window.as_ptr(), "AXFocused")
                .unwrap_or(false),
            alive: true,
        }),
        _ => Ok(ObservedState {
            bounds: Bounds::new(0, 0, 0, 0),
            minimized: false,
            focused: false,
            alive: false,
        }),
    }
}

/// Re-confirm the plan's expected identity against the live registry.
fn confirm_identity(operation: &PlannedWindowMutation) -> Result<()> {
    let observation = super::registry::resolve_handle(operation.target)?;
    let expected = &operation.expected_identity;
    anyhow::ensure!(
        observation.handle.pid == expected.pid,
        "target PID changed: {} != {}",
        observation.handle.pid,
        expected.pid
    );
    anyhow::ensure!(
        observation.handle.nonce == expected.nonce,
        "target nonce changed"
    );
    if let (Some(expected_native), Some(actual_native)) = (
        expected.native_window_id,
        observation.handle.native_window_id,
    ) {
        anyhow::ensure!(
            expected_native == actual_native,
            "target native window id changed"
        );
    }
    if let (Some(expected_bundle), Some(actual_bundle)) =
        (&expected.bundle_id, &observation.app.bundle_id)
    {
        anyhow::ensure!(expected_bundle == actual_bundle, "target bundle id changed");
    }
    // Capability preflight per request kind.
    let capabilities = &observation.capabilities;
    let allowed = match operation.request {
        RequestedMutation::SetPosition { .. } => capabilities.can_move,
        RequestedMutation::SetSize { .. } => capabilities.can_resize,
        RequestedMutation::SetBounds(_) => capabilities.can_move && capabilities.can_resize,
        RequestedMutation::SetMinimized(_) => capabilities.can_minimize,
        RequestedMutation::Focus => capabilities.can_raise || capabilities.actionable,
        RequestedMutation::Close => capabilities.can_close,
    };
    anyhow::ensure!(
        allowed,
        "window capability preflight failed for {:?}",
        operation.request
    );
    Ok(())
}

fn profile_for(operation: &PlannedWindowMutation) -> AppMutationProfile {
    let observation = super::registry::resolve_handle(operation.target).ok();
    let bundle_id = observation
        .as_ref()
        .and_then(|observation| observation.app.bundle_id.clone());
    let electron = observation
        .as_ref()
        .and_then(|observation| observation.app.app_path.as_deref())
        .map(|path| super::app_profiles::is_electron_app(Some(path)))
        .unwrap_or(false);
    resolve_profile(bundle_id.as_deref(), electron)
}

fn verify_deadline(request: &RequestedMutation) -> Duration {
    match request {
        RequestedMutation::SetPosition { .. }
        | RequestedMutation::SetSize { .. }
        | RequestedMutation::SetBounds(_) => BOUNDS_VERIFY_DEADLINE,
        RequestedMutation::SetMinimized(_) => MINIMIZE_VERIFY_DEADLINE,
        RequestedMutation::Focus => FOCUS_VERIFY_DEADLINE,
        RequestedMutation::Close => CLOSE_VERIFY_DEADLINE,
    }
}

/// Perform one write of the request against the active backend.
fn perform_write(
    operation: &PlannedWindowMutation,
    profile: AppMutationProfile,
    cancelled: &Arc<AtomicBool>,
) -> Result<()> {
    if super::test_support::is_active() {
        let observation = super::registry::resolve_handle(operation.target)?;
        let provider_id = observation.legacy_id;
        return match &operation.request {
            RequestedMutation::SetPosition { x, y } => {
                provider_set_position(provider_id, *x, *y, Some(cancelled))
            }
            RequestedMutation::SetSize { width, height } => {
                provider_set_size(provider_id, *width, *height, Some(cancelled))
            }
            RequestedMutation::SetBounds(bounds) => {
                provider_set_bounds(provider_id, *bounds, Some(cancelled))
            }
            RequestedMutation::SetMinimized(minimized) => {
                provider_set_minimized(provider_id, *minimized, Some(cancelled))
            }
            RequestedMutation::Focus => provider_focus(provider_id, Some(cancelled)),
            RequestedMutation::Close => provider_close(provider_id, Some(cancelled)),
        };
    }

    let window = super::registry::retained_window(operation.target)?;
    let _timeout_guard = AxMessagingTimeoutGuard::apply(window.as_ptr(), AX_MUTATION_TIMEOUT_SECS);
    match &operation.request {
        RequestedMutation::SetPosition { x, y } => {
            super::ax::set_window_position(window.as_ptr(), *x, *y)
        }
        RequestedMutation::SetSize { width, height } => {
            super::ax::set_window_size(window.as_ptr(), *width, *height)
        }
        RequestedMutation::SetBounds(bounds) => {
            apply_bounds_sequence(window.as_ptr(), *bounds, profile.sequence)
        }
        RequestedMutation::SetMinimized(minimized) => {
            set_window_minimized(window.as_ptr(), *minimized)
        }
        RequestedMutation::Focus => {
            super::ax::perform_ax_action(window.as_ptr(), "AXRaise")?;
            let observation = super::registry::resolve_handle(operation.target)?;
            activate_application(observation.handle.pid)
        }
        RequestedMutation::Close => {
            let close_button = super::ax::get_ax_attribute(window.as_ptr(), "AXCloseButton")?;
            let result =
                super::ax::perform_ax_action(close_button as super::ffi::AXUIElementRef, "AXPress");
            super::cf::cf_release(close_button);
            result
        }
    }
}

fn run_command(command: WorkerCommand, cancelled: &Arc<AtomicBool>) -> WorkerReply {
    match command {
        WorkerCommand::Preflight(operation) => {
            if let Err(error) = confirm_identity(&operation) {
                return failure(format!("preflight failed: {error}"));
            }
            match observe(&operation) {
                Ok(before) if before.alive => WorkerReply {
                    before: Some(before),
                    after: None,
                    attempts: Vec::new(),
                    error: None,
                    queue_wait: Duration::ZERO,
                },
                Ok(_) => failure("preflight failed: window no longer exists"),
                Err(error) => failure(format!("preflight failed: {error}")),
            }
        }
        WorkerCommand::Apply(operation) | WorkerCommand::Restore { operation } => {
            let profile = profile_for(&operation);
            let before = match observe(&operation) {
                Ok(state) => state,
                Err(error) => return failure(format!("observation failed: {error}")),
            };
            if let Err(error) = confirm_identity(&operation) {
                return failure(format!("identity revalidation failed: {error}"));
            }

            let mut attempts: Vec<MutationAttempt> = Vec::new();
            let mut last_observed: Option<ObservedState> = None;
            for attempt in 1..=profile.max_attempts {
                // Cancellation check before the first write and every retry.
                if cancelled.load(Ordering::SeqCst) {
                    return WorkerReply {
                        before: Some(before),
                        after: last_observed,
                        attempts,
                        error: Some("window_engine:cancelled".to_string()),
                        queue_wait: Duration::ZERO,
                    };
                }
                let setter_error = perform_write(&operation, profile, cancelled)
                    .err()
                    .map(|error| error.to_string());

                // Immediate readback, then poll until the request's deadline.
                let deadline = Instant::now() + verify_deadline(&operation.request);
                let mut verified = false;
                let mut observed = None;
                loop {
                    match observe(&operation) {
                        Ok(state) => {
                            let satisfied = request_satisfied(
                                &operation.request,
                                before.bounds,
                                &state,
                                profile.tolerance,
                            );
                            observed = Some(state);
                            if satisfied {
                                verified = true;
                                break;
                            }
                        }
                        Err(_) => {
                            // Resolution failure mid-verify: treat as gone.
                            observed = Some(ObservedState {
                                bounds: Bounds::new(0, 0, 0, 0),
                                minimized: false,
                                focused: false,
                                alive: false,
                            });
                            if matches!(operation.request, RequestedMutation::Close) {
                                verified = true;
                            }
                            break;
                        }
                    }
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(VERIFY_POLL_INTERVAL);
                }

                last_observed = observed;
                attempts.push(MutationAttempt {
                    attempt,
                    setter_error: setter_error.clone(),
                    observed_bounds: last_observed.map(|state| state.bounds),
                    verified,
                });

                if verified {
                    return WorkerReply {
                        before: Some(before),
                        after: last_observed,
                        attempts,
                        error: None,
                        queue_wait: Duration::ZERO,
                    };
                }
                // A destroyed window cannot be retried.
                if last_observed.is_some_and(|state| !state.alive) {
                    break;
                }
                if attempt < profile.max_attempts && !profile.retry_settle_delay.is_zero() {
                    std::thread::sleep(profile.retry_settle_delay);
                }
            }

            let request = format!("{:?}", operation.request);
            let actual = last_observed
                .map(|state| format!("{:?}", state.bounds))
                .unwrap_or_else(|| "unknown".to_string());
            WorkerReply {
                before: Some(before),
                after: last_observed,
                attempts,
                error: Some(format!(
                    "window rejected requested mutation: requested={request} actual={actual}"
                )),
                queue_wait: Duration::ZERO,
            }
        }
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use gpui::{App, AsyncApp};

use super::types::{Bounds, WindowHandle};

use super::snap::build_snap_targets_for_mode;
use super::snap_mode::{current_snap_mode, SnapMode};
use super::snap_overlay::{hide_snap_overlay, show_snap_overlay};
use super::snap_session::{
    begin_snap_session, build_overlay_scene, cancel_snap_session, finish_snap_session,
    poll_window_bounds, prime_snap_session, tick_snap_session, update_session_display,
    SnapDisplayTargets, SnapSession,
};

/// Polling interval for tracking the dragged window (~60 fps).
const SNAP_POLL_INTERVAL: Duration = Duration::from_millis(16);

// ---------------------------------------------------------------------------
// Active runtime state
// ---------------------------------------------------------------------------

struct ActiveSnapRuntime {
    session: SnapSession,
}

static ACTIVE_SNAP_RUNTIME: Mutex<Option<ActiveSnapRuntime>> = Mutex::new(None);

/// Generation token: bumped on every release/cancel so a late async session
/// begin or poll can never start or feed a stale runtime.
pub(super) static SNAP_ARM_GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn bump_snap_arm_generation() -> u64 {
    SNAP_ARM_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

pub(super) fn snap_arm_generation() -> u64 {
    SNAP_ARM_GENERATION.load(Ordering::SeqCst)
}

/// Poll the tracked window's bounds by handle (background-thread safe).
fn poll_bounds_for_handle(handle: WindowHandle) -> Option<Bounds> {
    let window = super::registry::retained_window(handle)
        .or_else(|_| {
            let observation = super::registry::resolve_nonce(handle.nonce)?;
            super::registry::retained_window(observation.handle)
        })
        .ok()?;
    let (x, y) = super::ax::get_window_position(window.as_ptr()).ok()?;
    let (width, height) = super::ax::get_window_size(window.as_ptr()).ok()?;
    Some(Bounds::new(x, y, width, height))
}

/// The active session's handle, read without AX work (pure lock read).
fn active_session_handle() -> Option<WindowHandle> {
    ACTIVE_SNAP_RUNTIME
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|runtime| runtime.session.window_handle))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check whether a snap runtime is currently active.
pub fn is_snap_runtime_active() -> bool {
    ACTIVE_SNAP_RUNTIME
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

/// Start a live snap runtime: begin tracking the frontmost external window
/// and render the desktop overlay.
pub fn start_snap_runtime(cx: &mut App) -> Result<()> {
    if current_snap_mode() == SnapMode::Off {
        tracing::info!(
            target: "script_kit::snap_runtime",
            event = "snap_runtime_start_blocked_mode_off",
            "snap runtime not started because snap mode is Off"
        );
        return Ok(());
    }

    if is_snap_runtime_active() {
        tracing::info!(
            target: "script_kit::snap_runtime",
            event = "snap_runtime_start_skipped_already_active",
            "snap runtime already active"
        );
        return Ok(());
    }

    // Session begin performs AX reads: run it OFF the GPUI thread. The arm
    // generation token invalidates the result if the drag releases first.
    let arm_generation = snap_arm_generation();
    cx.spawn(async move |cx: &mut AsyncApp| {
        let session = cx
            .background_executor()
            .spawn(async move {
                begin_snap_session().ok().map(|mut session| {
                    prime_snap_session(&mut session, Instant::now());
                    session
                })
            })
            .await;
        let Some(session) = session else {
            return;
        };
        if snap_arm_generation() != arm_generation {
            tracing::info!(
                target: "script_kit::snap_runtime",
                event = "snap_runtime_start_superseded",
                "drag released before session begin completed; not starting"
            );
            return;
        }
        let installed = cx.update(|cx| {
            if snap_arm_generation() != arm_generation || is_snap_runtime_active() {
                return false;
            }
            let scene = build_overlay_scene(&session);
            tracing::info!(
                target: "script_kit::snap_runtime",
                event = "snap_runtime_started",
                window_id = session.window_id,
                app_name = %session.app_name,
                title = %session.window_title,
                "started snap runtime"
            );
            if let Ok(mut guard) = super::snap_lock(&ACTIVE_SNAP_RUNTIME, "runtime") {
                *guard = Some(ActiveSnapRuntime { session });
            } else {
                return false;
            }
            let _ = show_snap_overlay(scene, cx);
            true
        });
        if !installed {
            return;
        }
        // Tick loop: poll AX on the background executor (one poll in flight
        // by construction), then tick pure session logic on the GPUI thread.
        loop {
            cx.background_executor().timer(SNAP_POLL_INTERVAL).await;
            let Some(handle) = active_session_handle() else {
                break;
            };
            let sample = cx
                .background_executor()
                .spawn(async move { poll_bounds_for_handle(handle) })
                .await;
            let keep_running =
                cx.update(|cx| tick_snap_runtime_with_sample(sample, cx).unwrap_or(false));
            if !keep_running {
                break;
            }
        }
    })
    .detach();

    Ok(())
}

/// Advance the runtime by one tick (synchronous helper for direct callers;
/// polls inline).
pub fn tick_snap_runtime(cx: &mut App) -> Result<bool> {
    let sample = {
        let guard = super::snap_lock(&ACTIVE_SNAP_RUNTIME, "runtime")?;
        guard
            .as_ref()
            .and_then(|runtime| poll_window_bounds(&runtime.session))
    };
    tick_snap_runtime_with_sample(sample, cx)
}

/// Advance the runtime by one tick using a PRE-POLLED bounds sample, so the
/// GPUI update closure performs no AX reads.
pub fn tick_snap_runtime_with_sample(sample: Option<Bounds>, cx: &mut App) -> Result<bool> {
    let mut guard = super::snap_lock(&ACTIVE_SNAP_RUNTIME, "runtime")?;

    let Some(runtime) = guard.as_mut() else {
        return Ok(false);
    };

    let Some(current_bounds) = sample else {
        tracing::info!(
            target: "script_kit::snap_runtime",
            event = "snap_runtime_window_gone",
            window_id = runtime.session.window_id,
            "tracked window disappeared"
        );
        let _session = guard.take();
        drop(guard);
        hide_snap_overlay(cx)?;
        return Ok(false);
    };

    update_session_display(&mut runtime.session, &current_bounds);
    let phase = tick_snap_session(&mut runtime.session, current_bounds, Instant::now());
    let overlay_scene = build_overlay_scene(&runtime.session);

    tracing::info!(
        target: "script_kit::snap_runtime",
        event = "snap_runtime_tick",
        window_id = runtime.session.window_id,
        ?phase,
        matched = runtime.session.active_match.is_some(),
        matched_tile = runtime
            .session
            .active_match
            .map(|m| format!("{:?}", m.target.tile)),
        "updated snap runtime"
    );

    // Release lock before overlay update.
    drop(guard);
    show_snap_overlay(overlay_scene, cx)?;

    Ok(true)
}

/// Finish the snap runtime on mouse-up. Commits when there is an active match,
/// otherwise cancels cleanly.
pub fn finish_snap_runtime(cx: &mut App) -> Result<()> {
    bump_snap_arm_generation();
    let mut guard = super::snap_lock(&ACTIVE_SNAP_RUNTIME, "runtime")?;

    let Some(runtime) = guard.take() else {
        return Ok(());
    };
    drop(guard);

    // Hide the overlay IMMEDIATELY on release — never wait on AX.
    hide_snap_overlay(cx)?;

    // Commit through the transaction engine off the GPUI thread. Completion
    // only logs; no overlay is recreated on failure.
    let spawn_result = std::thread::Builder::new()
        .name("snap-commit".to_string())
        .spawn(move || match finish_snap_session(&runtime.session) {
            Ok(outcome) => tracing::info!(
                target: "script_kit::snap_runtime",
                event = "snap_runtime_finished",
                ?outcome,
                "finished snap runtime"
            ),
            Err(error) => tracing::warn!(
                target: "script_kit::snap_runtime",
                event = "snap_runtime_commit_failed",
                %error,
                "snap commit failed"
            ),
        });
    if let Err(error) = spawn_result {
        tracing::warn!(
            target: "script_kit::snap_runtime",
            event = "snap_runtime_commit_spawn_failed",
            %error,
            "failed to spawn snap commit thread"
        );
    }

    Ok(())
}

/// Cancel the snap runtime without applying changes.
pub fn cancel_snap_runtime(cx: &mut App) -> Result<()> {
    bump_snap_arm_generation();
    let mut guard = super::snap_lock(&ACTIVE_SNAP_RUNTIME, "runtime")?;

    if let Some(runtime) = guard.take() {
        let outcome = cancel_snap_session(&runtime.session);
        tracing::info!(
            target: "script_kit::snap_runtime",
            event = "snap_runtime_cancelled",
            ?outcome,
            "cancelled snap runtime"
        );
    }

    // Release lock before overlay call.
    drop(guard);
    hide_snap_overlay(cx)?;

    Ok(())
}

/// Refresh the active runtime after a snap-mode change without losing the
/// currently tracked window or overlay lifecycle.
pub fn refresh_snap_runtime_for_mode(cx: &mut App) -> Result<()> {
    let mut guard = super::snap_lock(&ACTIVE_SNAP_RUNTIME, "runtime")?;

    let Some(runtime) = guard.as_mut() else {
        return Ok(());
    };

    let mode = current_snap_mode();
    runtime.session.mode = mode;
    runtime.session.all_display_targets = runtime
        .session
        .all_display_targets
        .iter()
        .map(|dt| SnapDisplayTargets {
            display: dt.display,
            targets: build_snap_targets_for_mode(&dt.display, mode),
        })
        .collect();

    if let Some(dt) = runtime
        .session
        .all_display_targets
        .iter()
        .find(|dt| dt.display == runtime.session.display)
    {
        runtime.session.targets = dt.targets.clone();
    } else if let Some(first) = runtime.session.all_display_targets.first() {
        runtime.session.display = first.display;
        runtime.session.targets = first.targets.clone();
    } else {
        runtime.session.targets.clear();
    }

    let current_bounds = runtime.session.last_window_bounds;
    update_session_display(&mut runtime.session, &current_bounds);
    let _ = tick_snap_session(&mut runtime.session, current_bounds, Instant::now());
    let scene = build_overlay_scene(&runtime.session);

    tracing::info!(
        target: "script_kit::snap_runtime",
        event = "snap_runtime_mode_refreshed",
        window_id = runtime.session.window_id,
        ?mode,
        target_count = runtime.session.targets.len(),
        display_count = runtime.session.all_display_targets.len(),
        "refreshed snap runtime for snap mode change"
    );

    drop(guard);
    show_snap_overlay(scene, cx)?;

    Ok(())
}

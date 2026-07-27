use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::ax::{get_window_position, get_window_size};
use super::display::{get_all_display_bounds, get_visible_display_bounds};
use super::snap::{
    best_snap_match, build_snap_targets_for_mode, dominant_display_for_window, SnapMatch,
    SnapTarget,
};
use super::snap_mode::{current_snap_mode, SnapMode};
use super::types::{Bounds, TilePosition};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Minimum overlap ratio to consider a snap target as active.
const MIN_OVERLAP_RATIO: f64 = 0.35;

/// Duration of inactivity (no window movement) before settling.
const SETTLE_DURATION: Duration = Duration::from_millis(120);

// ---------------------------------------------------------------------------
// Session types
// ---------------------------------------------------------------------------

/// Phase of the snap session state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSessionPhase {
    /// Tracking window movement, haven't seen any movement yet.
    WaitingForMovement,
    /// Window has moved at least once; actively tracking and updating.
    Tracking,
    /// Movement settled; session is complete.
    Settled,
}

/// Outcome of a completed snap session.
#[derive(Debug, Clone)]
pub enum SnapSessionOutcome {
    /// Session committed: the window was snapped to a target.
    Committed { tile: TilePosition, bounds: Bounds },
    /// Session cancelled: no active target when movement settled.
    Cancelled,
}

/// Data model for the desktop snap overlay (consumed by the renderer).
#[derive(Debug, Clone, PartialEq)]
pub struct SnapOverlayModel {
    /// Visible display bounds for positioning the overlay window.
    pub display_bounds: Bounds,
    /// Snap mode used to build this overlay model.
    pub mode: SnapMode,
    /// Whether this display is the currently dominant/matched display.
    pub is_dominant: bool,
    /// All snap targets with their active/inactive state.
    pub targets: Vec<SnapOverlayTarget>,
}

/// A single snap target for overlay rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapOverlayTarget {
    pub tile: TilePosition,
    pub bounds: Bounds,
    pub active: bool,
}

/// Pre-computed snap targets for a single display.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapDisplayTargets {
    /// The visible bounds of this display.
    pub display: Bounds,
    /// Snap targets computed for the current mode on this display.
    pub targets: Vec<SnapTarget>,
}

/// A complete overlay scene distributed across all connected displays.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapOverlayScene {
    /// The snap mode that produced this scene.
    pub mode: SnapMode,
    /// One overlay model per display.
    pub displays: Vec<SnapOverlayModel>,
}

/// The live snap session state.
#[derive(Debug, Clone)]
pub struct SnapSession {
    /// The tracked external window's ID.
    pub window_id: u32,
    /// The registry handle backing the tracked window (identity-safe).
    pub window_handle: super::types::WindowHandle,
    /// Application name of the tracked window.
    pub app_name: String,
    /// Window title.
    pub window_title: String,
    /// Visible display bounds of the **dominant** display (most overlap with window).
    pub display: Bounds,
    /// Pre-computed snap targets for the dominant display.
    pub targets: Vec<SnapTarget>,
    /// Current best snap match (if any target exceeds the overlap threshold).
    pub active_match: Option<SnapMatch>,
    /// Last known window bounds (updated on each tick).
    pub last_window_bounds: Bounds,
    /// Session phase.
    pub phase: SnapSessionPhase,
    /// Timestamp of the last detected window movement.
    last_movement_time: Option<Instant>,
    /// Whether we've seen at least one movement during this session.
    has_moved: bool,
    /// Unique session ID for trace correlation.
    session_id: u64,
    /// The snap mode active for this session.
    pub mode: SnapMode,
    /// Pre-computed targets for **every** connected display.
    pub all_display_targets: Vec<SnapDisplayTargets>,
}

/// Monotonic session ID counter.
static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

// ---------------------------------------------------------------------------
// Session lifecycle
// ---------------------------------------------------------------------------

/// Begin a new snap session from the frontmost external window.
///
/// Resolves the menu-bar-owning app's focused window, reads its bounds,
/// determines the dominant display by overlap, and pre-computes snap targets
/// for **every** connected display using the current snap mode.
pub fn begin_snap_session() -> Result<SnapSession> {
    let mode = current_snap_mode();
    let window = super::query::get_frontmost_window_of_previous_app()?
        .context("No frontmost external window available for snap session")?;

    // Gather all visible display bounds.  Fall back to the single display
    // containing the window's top-left point if the multi-display query fails.
    let all_displays = get_all_display_bounds()
        .ok()
        .filter(|displays| !displays.is_empty())
        .unwrap_or_else(|| vec![get_visible_display_bounds(window.bounds.x, window.bounds.y)]);

    // Pick the dominant display by window/display overlap (not point ownership).
    let display_bounds = dominant_display_for_window(&window.bounds, &all_displays)
        .unwrap_or_else(|| get_visible_display_bounds(window.bounds.x, window.bounds.y));

    // Pre-compute targets for every display.
    let all_display_targets: Vec<SnapDisplayTargets> = all_displays
        .iter()
        .map(|d| SnapDisplayTargets {
            display: *d,
            targets: build_snap_targets_for_mode(d, mode),
        })
        .collect();

    // Extract dominant display's targets.
    let targets = all_display_targets
        .iter()
        .find(|dt| dt.display == display_bounds)
        .map(|dt| dt.targets.clone())
        .unwrap_or_else(|| build_snap_targets_for_mode(&display_bounds, mode));

    let session_id = SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    tracing::info!(
        target: "script_kit::snap_session",
        event = "snap_session_started",
        session_id,
        window_id = window.id,
        app = %window.app,
        title = %window.title,
        ?mode,
        display_count = all_displays.len(),
        dominant_display_x = display_bounds.x,
        dominant_display_y = display_bounds.y,
        dominant_display_w = display_bounds.width,
        dominant_display_h = display_bounds.height,
        target_count = targets.len(),
        "began snap session"
    );

    Ok(SnapSession {
        window_id: window.id,
        window_handle: window.handle(),
        app_name: window.app,
        window_title: window.title,
        display: display_bounds,
        targets,
        active_match: None,
        last_window_bounds: window.bounds,
        phase: SnapSessionPhase::WaitingForMovement,
        last_movement_time: None,
        has_moved: false,
        session_id,
        mode,
        all_display_targets,
    })
}

/// Poll the tracked window's current AX bounds.
///
/// Returns `None` if the window is no longer accessible (e.g., closed).
pub fn poll_window_bounds(session: &SnapSession) -> Option<Bounds> {
    // Poll path: handle lookup only, no refresh — this runs at 16 ms cadence.
    let window = super::registry::retained_window(session.window_handle)
        .or_else(|_| {
            // Generation moved mid-drag (e.g. a background refresh): recover
            // the SAME window by nonce without a full refresh.
            let observation = super::registry::resolve_nonce(session.window_handle.nonce)?;
            super::registry::retained_window(observation.handle)
        })
        .ok()?;
    let (x, y) = get_window_position(window.as_ptr()).ok()?;
    let (w, h) = get_window_size(window.as_ptr()).ok()?;
    Some(Bounds::new(x, y, w, h))
}

/// Re-evaluate the dominant display based on window/display overlap.
///
/// Uses the pre-built display list from session start, avoiding repeated
/// NSScreen API calls during the 60 fps tick loop.  Falls back to the
/// point-based NSScreen lookup only when the session has a single display
/// (graceful degradation for the rare case where `get_all_display_bounds`
/// failed at session start).
pub fn update_session_display(session: &mut SnapSession, window_bounds: &Bounds) {
    let displays: Vec<Bounds> = session
        .all_display_targets
        .iter()
        .map(|dt| dt.display)
        .collect();

    let new_display = if displays.len() > 1 {
        dominant_display_for_window(window_bounds, &displays).unwrap_or(session.display)
    } else {
        // Single-display fallback: re-query NSScreen (preserves old behavior).
        get_visible_display_bounds(window_bounds.x, window_bounds.y)
    };

    if new_display != session.display {
        // Swap dominant display and its pre-built targets.
        if let Some(dt) = session
            .all_display_targets
            .iter()
            .find(|dt| dt.display == new_display)
        {
            session.display = new_display;
            session.targets = dt.targets.clone();
        } else {
            // Display not in pre-built set (shouldn't happen); rebuild on the fly.
            session.display = new_display;
            session.targets = build_snap_targets_for_mode(&new_display, session.mode);
        }

        tracing::info!(
            target: "script_kit::snap_session",
            event = "snap_session_display_changed",
            session_id = session.session_id,
            display_x = new_display.x,
            display_y = new_display.y,
            display_w = new_display.width,
            display_h = new_display.height,
            "snap session dominant display changed"
        );
    }
}

fn best_snap_match_from_display_targets(
    dragged_window: &Bounds,
    all_display_targets: &[SnapDisplayTargets],
    min_overlap_ratio: f64,
) -> Option<(Bounds, SnapMatch)> {
    let mut best: Option<(Bounds, SnapMatch)> = None;

    for display_targets in all_display_targets {
        if let Some(candidate) =
            best_snap_match(dragged_window, &display_targets.targets, min_overlap_ratio)
        {
            if best
                .as_ref()
                .is_none_or(|(_, current)| candidate.overlap_ratio > current.overlap_ratio)
            {
                best = Some((display_targets.display, candidate));
            }
        }
    }

    best
}

/// Advance the session by one tick with the current window bounds.
///
/// Updates the active snap match and phase. Returns the updated phase.
/// This function is pure computation — no macOS API calls.
pub fn tick_snap_session(
    session: &mut SnapSession,
    current_bounds: Bounds,
    now: Instant,
) -> SnapSessionPhase {
    let moved = current_bounds != session.last_window_bounds;

    if moved {
        session.has_moved = true;
        session.last_movement_time = Some(now);
        session.phase = SnapSessionPhase::Tracking;
    }

    // Recompute snap match when tracking.
    if session.has_moved {
        if let Some((matched_display, matched)) = best_snap_match_from_display_targets(
            &current_bounds,
            &session.all_display_targets,
            MIN_OVERLAP_RATIO,
        ) {
            if matched_display != session.display {
                if let Some(dt) = session
                    .all_display_targets
                    .iter()
                    .find(|dt| dt.display == matched_display)
                {
                    session.display = matched_display;
                    session.targets = dt.targets.clone();
                }

                tracing::info!(
                    target: "script_kit::snap_session",
                    event = "snap_session_match_display_changed",
                    session_id = session.session_id,
                    display_x = matched_display.x,
                    display_y = matched_display.y,
                    display_w = matched_display.width,
                    display_h = matched_display.height,
                    tile = ?matched.target.tile,
                    "switched snap session display from matched target"
                );
            }

            session.active_match = Some(matched);
        } else {
            update_session_display(session, &current_bounds);
            session.active_match =
                best_snap_match(&current_bounds, &session.targets, MIN_OVERLAP_RATIO);
        }

        tracing::info!(
            target: "script_kit::snap_session",
            event = "snap_session_cross_display_match",
            session_id = session.session_id,
            matched = session.active_match.is_some(),
            matched_tile = session.active_match.map(|m| format!("{:?}", m.target.tile)),
            matched_bounds_x = session.active_match.map(|m| m.target.bounds.x),
            matched_bounds_y = session.active_match.map(|m| m.target.bounds.y),
            "evaluated cross-display snap match"
        );
    }

    session.last_window_bounds = current_bounds;

    // Check settling: movement burst happened, then no movement for SETTLE_DURATION.
    if session.has_moved && !moved {
        if let Some(last_move) = session.last_movement_time {
            if now.duration_since(last_move) >= SETTLE_DURATION {
                session.phase = SnapSessionPhase::Settled;
            }
        }
    }

    tracing::info!(
        target: "script_kit::snap_session",
        event = "snap_session_updated",
        session_id = session.session_id,
        window_id = session.window_id,
        x = current_bounds.x,
        y = current_bounds.y,
        w = current_bounds.width,
        h = current_bounds.height,
        moved,
        phase = ?session.phase,
        matched = session.active_match.is_some(),
        matched_tile = session.active_match.map(|m| format!("{:?}", m.target.tile)),
        overlap = session.active_match.map(|m| m.overlap_ratio),
        "tick snap session"
    );

    session.phase
}

/// Build an overlay model from the current session state.
///
/// The caller is responsible for passing this model to the overlay renderer.
pub fn build_overlay_model(session: &SnapSession) -> SnapOverlayModel {
    let active_bounds = session.active_match.map(|m| m.target.bounds);

    SnapOverlayModel {
        display_bounds: session.display,
        mode: session.mode,
        is_dominant: true,
        targets: session
            .targets
            .iter()
            .map(|t| SnapOverlayTarget {
                tile: t.tile,
                bounds: t.bounds,
                active: Some(t.bounds) == active_bounds,
            })
            .collect(),
    }
}

/// Build a complete overlay scene from the session state, with one model per
/// connected display.  Only the dominant display's targets carry an active flag.
pub fn build_overlay_scene(session: &SnapSession) -> SnapOverlayScene {
    let active_bounds = session.active_match.map(|m| m.target.bounds);

    let mut displays: Vec<_> = session
        .all_display_targets
        .iter()
        .map(|dt| {
            let is_dominant = dt.display == session.display;
            SnapOverlayModel {
                display_bounds: dt.display,
                mode: session.mode,
                is_dominant,
                targets: dt
                    .targets
                    .iter()
                    .map(|t| SnapOverlayTarget {
                        tile: t.tile,
                        bounds: t.bounds,
                        active: Some(t.bounds) == active_bounds,
                    })
                    .collect(),
            }
        })
        .collect();

    if displays.is_empty() {
        displays.push(build_overlay_model(session));
    }

    SnapOverlayScene {
        mode: session.mode,
        displays,
    }
}

/// Commit the snap session: apply the active match bounds to the tracked window.
///
/// Returns `SnapSessionOutcome::Committed` on success.
pub fn commit_snap_session(session: &SnapSession) -> Result<SnapSessionOutcome> {
    let active = session
        .active_match
        .context("No active snap target to commit")?;

    tracing::info!(
        target: "script_kit::snap_session",
        event = "snap_session_committed",
        session_id = session.session_id,
        window_id = session.window_id,
        tile = ?active.target.tile,
        target_x = active.target.bounds.x,
        target_y = active.target.bounds.y,
        target_w = active.target.bounds.width,
        target_h = active.target.bounds.height,
        overlap = active.overlap_ratio,
        "committing snap session"
    );

    let plan = build_snap_commit_plan(session)?;
    let receipt = super::transaction::execute_plan(&plan)?;
    if receipt.status != super::transaction::MutationStatus::Succeeded {
        let detail = receipt
            .operations
            .iter()
            .find_map(|operation| operation.error.clone())
            .unwrap_or_else(|| format!("{:?}", receipt.status));
        anyhow::bail!("snap commit failed: {detail}");
    }

    Ok(SnapSessionOutcome::Committed {
        tile: active.target.tile,
        bounds: active.target.bounds,
    })
}

/// Build the strict, undoable SetBounds plan for the active snap target.
///
/// Geometry comes from the CALIBRATED snap target bounds untouched; only the
/// execution path changes (verified transaction instead of raw AX writes).
pub fn build_snap_commit_plan(session: &SnapSession) -> Result<super::plan::WindowMutationPlan> {
    let active = session
        .active_match
        .context("No active snap target to commit")?;
    let observation = super::registry::resolve_handle(session.window_handle).or_else(|_| {
        // Recover the same window by nonce after a generation change.
        super::registry::resolve_nonce(session.window_handle.nonce)
    })?;
    Ok(super::plan::build_explicit_bounds_plan(
        super::diagnostics::OperationSource::SnapSession,
        &observation,
        active.target.bounds,
        super::plan::RollbackPolicy::Strict,
        true,
    ))
}

/// Cancel the snap session without applying any changes.
pub fn cancel_snap_session(session: &SnapSession) -> SnapSessionOutcome {
    tracing::info!(
        target: "script_kit::snap_session",
        event = "snap_session_cancelled",
        session_id = session.session_id,
        window_id = session.window_id,
        had_match = session.active_match.is_some(),
        "cancelled snap session"
    );

    SnapSessionOutcome::Cancelled
}

/// Prime a session from the first observed drag frame so the overlay is
/// visible immediately instead of waiting for a second movement tick.
pub fn prime_snap_session(session: &mut SnapSession, now: Instant) {
    session.has_moved = true;
    session.last_movement_time = Some(now);
    session.phase = SnapSessionPhase::Tracking;
    if let Some((matched_display, matched)) = best_snap_match_from_display_targets(
        &session.last_window_bounds,
        &session.all_display_targets,
        MIN_OVERLAP_RATIO,
    ) {
        if let Some(dt) = session
            .all_display_targets
            .iter()
            .find(|dt| dt.display == matched_display)
        {
            session.display = matched_display;
            session.targets = dt.targets.clone();
        }
        session.active_match = Some(matched);
    } else {
        session.active_match = best_snap_match(
            &session.last_window_bounds,
            &session.targets,
            MIN_OVERLAP_RATIO,
        );
    }

    tracing::info!(
        target: "script_kit::snap_session",
        event = "snap_session_primed",
        session_id = session.session_id,
        window_id = session.window_id,
        x = session.last_window_bounds.x,
        y = session.last_window_bounds.y,
        w = session.last_window_bounds.width,
        h = session.last_window_bounds.height,
        matched = session.active_match.is_some(),
        matched_tile = session.active_match.map(|m| format!("{:?}", m.target.tile)),
        overlap = session.active_match.map(|m| m.overlap_ratio),
        "primed snap session from initial drag frame"
    );
}

/// Finish a settled session: commit if there's an active match, otherwise cancel.
pub fn finish_snap_session(session: &SnapSession) -> Result<SnapSessionOutcome> {
    if session.active_match.is_some() {
        commit_snap_session(session)
    } else {
        Ok(cancel_snap_session(session))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::snap::build_snap_targets;
    use super::*;

    fn make_display() -> Bounds {
        Bounds::new(0, 24, 1440, 876)
    }

    fn make_session() -> SnapSession {
        let display_val = make_display();
        let targets = build_snap_targets(&display_val);
        let all_display_targets = vec![SnapDisplayTargets {
            display: display_val,
            targets: targets.clone(),
        }];
        SnapSession {
            window_id: 0x1234_0000,
            window_handle: super::super::types::WindowHandle {
                pid: 0,
                native_window_id: None,
                registry_generation: 0,
                nonce: 0,
            },
            app_name: "TestApp".to_string(),
            window_title: "Test Window".to_string(),
            display: display_val,
            targets,
            active_match: None,
            last_window_bounds: Bounds::new(200, 100, 800, 600),
            phase: SnapSessionPhase::WaitingForMovement,
            last_movement_time: None,
            has_moved: false,
            session_id: 0,
            mode: SnapMode::Expanded,
            all_display_targets,
        }
    }

    #[test]
    fn tick_detects_movement_and_enters_tracking() {
        let mut session = make_session();
        let now = Instant::now();

        // First tick with same bounds — stays in WaitingForMovement.
        let phase = tick_snap_session(&mut session, Bounds::new(200, 100, 800, 600), now);
        assert_eq!(phase, SnapSessionPhase::WaitingForMovement);

        // Move the window — enters Tracking.
        let phase = tick_snap_session(&mut session, Bounds::new(0, 30, 800, 600), now);
        assert_eq!(phase, SnapSessionPhase::Tracking);
        assert!(session.has_moved);
    }

    #[test]
    fn tick_settles_after_inactivity() {
        let mut session = make_session();
        let t0 = Instant::now();

        // Move at t0.
        tick_snap_session(&mut session, Bounds::new(0, 30, 800, 600), t0);
        assert_eq!(session.phase, SnapSessionPhase::Tracking);

        // No movement, but not enough time — still Tracking.
        let t1 = t0 + Duration::from_millis(50);
        tick_snap_session(&mut session, Bounds::new(0, 30, 800, 600), t1);
        assert_eq!(session.phase, SnapSessionPhase::Tracking);

        // After settle duration — Settled.
        let t2 = t0 + SETTLE_DURATION + Duration::from_millis(1);
        tick_snap_session(&mut session, Bounds::new(0, 30, 800, 600), t2);
        assert_eq!(session.phase, SnapSessionPhase::Settled);
    }

    #[test]
    fn tick_finds_snap_match_for_left_half() {
        let mut session = make_session();
        let now = Instant::now();

        // Drag to left half of display.
        let left_bounds = Bounds::new(0, 24, 720, 876);
        tick_snap_session(&mut session, left_bounds, now);

        let active = session.active_match.expect("should match left half");
        assert_eq!(active.target.tile, TilePosition::LeftHalf);
    }

    #[test]
    fn tick_clears_match_when_window_moves_away() {
        let mut session = make_session();
        let now = Instant::now();

        // First: snap to left half.
        tick_snap_session(&mut session, Bounds::new(0, 24, 720, 876), now);
        assert!(session.active_match.is_some());

        // Move to a position that doesn't overlap enough.
        let t1 = now + Duration::from_millis(16);
        tick_snap_session(&mut session, Bounds::new(500, 300, 100, 100), t1);
        assert!(session.active_match.is_none());
    }

    #[test]
    fn build_overlay_model_marks_active_target() {
        let mut session = make_session();
        let now = Instant::now();

        // Snap to left half.
        tick_snap_session(&mut session, Bounds::new(0, 24, 720, 876), now);

        let model = build_overlay_model(&session);
        assert_eq!(model.display_bounds, session.display);
        assert_eq!(model.mode, SnapMode::Expanded);
        assert!(model.is_dominant);

        let active_targets: Vec<_> = model.targets.iter().filter(|t| t.active).collect();
        assert_eq!(active_targets.len(), 1);
        assert_eq!(active_targets[0].tile, TilePosition::LeftHalf);

        let inactive_count = model.targets.iter().filter(|t| !t.active).count();
        assert_eq!(inactive_count, model.targets.len() - 1);
    }

    #[test]
    fn build_overlay_model_no_active_when_no_match() {
        let session = make_session();
        let model = build_overlay_model(&session);
        assert!(model.targets.iter().all(|t| !t.active));
    }

    #[test]
    fn finish_cancels_when_no_active_match() {
        let session = make_session();
        let outcome = finish_snap_session(&session).expect("cancel should not fail");
        assert!(matches!(outcome, SnapSessionOutcome::Cancelled));
    }

    #[test]
    fn prime_session_marks_tracking_and_computes_current_match() {
        let mut session = make_session();
        session.last_window_bounds = Bounds::new(0, 24, 720, 876);
        prime_snap_session(&mut session, Instant::now());

        assert_eq!(session.phase, SnapSessionPhase::Tracking);
        assert!(session.has_moved);
        assert!(session.last_movement_time.is_some());

        let active = session
            .active_match
            .expect("expected active match after priming");
        assert_eq!(active.target.tile, TilePosition::LeftHalf);
    }

    #[test]
    fn cancel_returns_cancelled_outcome() {
        let session = make_session();
        let outcome = cancel_snap_session(&session);
        assert!(matches!(outcome, SnapSessionOutcome::Cancelled));
    }

    #[test]
    fn session_phase_progression() {
        let mut session = make_session();
        let t0 = Instant::now();

        // Phase 1: WaitingForMovement.
        assert_eq!(session.phase, SnapSessionPhase::WaitingForMovement);

        // Phase 2: Tracking (after movement).
        tick_snap_session(&mut session, Bounds::new(10, 30, 800, 600), t0);
        assert_eq!(session.phase, SnapSessionPhase::Tracking);

        // Phase 3: Settled (after settle duration with no movement).
        let t1 = t0 + SETTLE_DURATION + Duration::from_millis(10);
        tick_snap_session(&mut session, Bounds::new(10, 30, 800, 600), t1);
        assert_eq!(session.phase, SnapSessionPhase::Settled);
    }

    #[test]
    fn movement_resets_settle_timer() {
        let mut session = make_session();
        let t0 = Instant::now();

        // Initial movement.
        tick_snap_session(&mut session, Bounds::new(10, 30, 800, 600), t0);

        // Almost settled...
        let t1 = t0 + SETTLE_DURATION - Duration::from_millis(10);
        tick_snap_session(&mut session, Bounds::new(10, 30, 800, 600), t1);
        assert_eq!(session.phase, SnapSessionPhase::Tracking);

        // New movement resets the timer.
        let t2 = t0 + SETTLE_DURATION;
        tick_snap_session(&mut session, Bounds::new(20, 30, 800, 600), t2);
        assert_eq!(session.phase, SnapSessionPhase::Tracking);

        // Not enough time since the second movement — still Tracking.
        let t3 = t2 + Duration::from_millis(50);
        tick_snap_session(&mut session, Bounds::new(20, 30, 800, 600), t3);
        assert_eq!(session.phase, SnapSessionPhase::Tracking);

        // Now enough time after second movement — Settled.
        let t4 = t2 + SETTLE_DURATION + Duration::from_millis(1);
        tick_snap_session(&mut session, Bounds::new(20, 30, 800, 600), t4);
        assert_eq!(session.phase, SnapSessionPhase::Settled);
    }

    #[test]
    fn right_half_snap_match() {
        let mut session = make_session();
        let now = Instant::now();

        // Drag to right half of display.
        let right_bounds = Bounds::new(720, 24, 720, 876);
        tick_snap_session(&mut session, right_bounds, now);

        let active = session.active_match.expect("should match right half");
        assert_eq!(active.target.tile, TilePosition::RightHalf);
    }

    #[test]
    fn quadrant_snap_match() {
        let mut session = make_session();
        let now = Instant::now();

        // Drag to top-left quadrant.
        let tl_bounds = Bounds::new(0, 24, 720, 438);
        tick_snap_session(&mut session, tl_bounds, now);

        let active = session.active_match.expect("should match a quadrant");
        assert_eq!(active.target.tile, TilePosition::TopLeft);
    }

    // -----------------------------------------------------------------------
    // Multi-display scene tests
    // -----------------------------------------------------------------------

    fn make_dual_display_session() -> SnapSession {
        use super::super::snap::build_snap_targets_for_mode;

        let display_a = Bounds::new(0, 24, 1440, 876);
        let display_b = Bounds::new(1440, 24, 1440, 876);
        let mode = SnapMode::Expanded;

        let targets_a = build_snap_targets_for_mode(&display_a, mode);
        let targets_b = build_snap_targets_for_mode(&display_b, mode);

        let all_display_targets = vec![
            SnapDisplayTargets {
                display: display_a,
                targets: targets_a.clone(),
            },
            SnapDisplayTargets {
                display: display_b,
                targets: targets_b,
            },
        ];

        SnapSession {
            window_id: 0xABCD_0000,
            window_handle: super::super::types::WindowHandle {
                pid: 0,
                native_window_id: None,
                registry_generation: 0,
                nonce: 0,
            },
            app_name: "DualApp".to_string(),
            window_title: "Dual Window".to_string(),
            display: display_a,
            targets: targets_a,
            active_match: None,
            last_window_bounds: Bounds::new(200, 100, 800, 600),
            phase: SnapSessionPhase::WaitingForMovement,
            last_movement_time: None,
            has_moved: false,
            session_id: 99,
            mode,
            all_display_targets,
        }
    }

    #[test]
    fn build_overlay_scene_has_model_per_display() {
        let session = make_dual_display_session();
        let scene = build_overlay_scene(&session);
        assert_eq!(scene.displays.len(), 2);
        assert_eq!(scene.mode, SnapMode::Expanded);
    }

    #[test]
    fn build_overlay_scene_only_dominant_display_has_active_target() {
        let mut session = make_dual_display_session();
        let now = Instant::now();

        // Snap to left half of dominant display (display_a).
        tick_snap_session(&mut session, Bounds::new(0, 24, 720, 876), now);
        assert!(session.active_match.is_some());

        let scene = build_overlay_scene(&session);

        // Dominant display (display_a) should have one active target.
        let dominant_model = &scene.displays[0];
        assert!(dominant_model.is_dominant);
        let active_count = dominant_model.targets.iter().filter(|t| t.active).count();
        assert_eq!(active_count, 1);

        // Non-dominant display (display_b) should have zero active targets.
        let other_model = &scene.displays[1];
        assert!(!other_model.is_dominant);
        let active_count = other_model.targets.iter().filter(|t| t.active).count();
        assert_eq!(active_count, 0);
    }

    #[test]
    fn update_session_display_switches_dominant_on_overlap() {
        let mut session = make_dual_display_session();

        // Window mostly on display_b.
        let window_on_b = Bounds::new(1500, 100, 800, 600);
        update_session_display(&mut session, &window_on_b);

        assert_eq!(session.display, Bounds::new(1440, 24, 1440, 876));
    }

    #[test]
    fn update_session_display_stays_on_same_display_when_no_change() {
        let mut session = make_dual_display_session();
        let original_display = session.display;

        // Window fully on display_a.
        let window_on_a = Bounds::new(100, 100, 800, 600);
        update_session_display(&mut session, &window_on_a);

        assert_eq!(session.display, original_display);
    }

    #[test]
    fn tick_snap_session_switches_to_cross_display_match() {
        let mut session = make_dual_display_session();
        let now = Instant::now();

        // This window lands on the left-half target of display_b and should
        // switch the session to that display immediately.
        let straddling = Bounds::new(1440, 24, 720, 876);
        tick_snap_session(&mut session, straddling, now);

        let active = session
            .active_match
            .expect("should match on second display");
        assert_eq!(active.target.tile, TilePosition::LeftHalf);
        assert_eq!(active.target.bounds.x, 1440);
        assert_eq!(session.display, Bounds::new(1440, 24, 1440, 876));
    }

    #[test]
    fn snap_commit_routes_through_the_transaction_engine() {
        let _lock = super::super::registry::REGISTRY_TEST_LOCK.lock();
        let _env = super::super::test_support::test_env::EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Dragged","pid":9,
                 "bounds":{"x":5,"y":5,"width":800,"height":600}}
            ]}"#,
        );
        super::super::registry::reset_registry_for_tests();
        super::super::registry::refresh_from_test_provider().expect("refresh");
        let handle = super::super::registry::resolve_legacy_window_id(1).expect("resolve");

        let mut session = make_session();
        session.window_id = 1;
        session.window_handle = handle;
        session.active_match = Some(super::super::snap::SnapMatch {
            target: super::super::snap::SnapTarget {
                tile: TilePosition::LeftHalf,
                bounds: Bounds::new(0, 25, 960, 1055),
            },
            overlap_ratio: 0.9,
        });

        let outcome = commit_snap_session(&session).expect("commit");
        assert!(matches!(
            outcome,
            SnapSessionOutcome::Committed {
                tile: TilePosition::LeftHalf,
                ..
            }
        ));
        // The provider window verifiably moved to the CALIBRATED target frame.
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds, Bounds::new(0, 25, 960, 1055));
        // The commit is undoable (engine-recorded).
        assert!(super::super::undo::window_undo_depths().0 >= 1);
        super::super::undo::clear_window_undo_history();
    }

    #[test]
    fn failed_snap_verification_is_not_reported_committed() {
        let _lock = super::super::registry::REGISTRY_TEST_LOCK.lock();
        let _env = super::super::test_support::test_env::EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Sticky","pid":9,
                 "mutation":{"positionDeltaX":40}}
            ]}"#,
        );
        super::super::registry::reset_registry_for_tests();
        super::super::registry::refresh_from_test_provider().expect("refresh");
        let handle = super::super::registry::resolve_legacy_window_id(1).expect("resolve");

        let mut session = make_session();
        session.window_id = 1;
        session.window_handle = handle;
        session.active_match = Some(super::super::snap::SnapMatch {
            target: super::super::snap::SnapTarget {
                tile: TilePosition::LeftHalf,
                bounds: Bounds::new(0, 25, 960, 1055),
            },
            overlap_ratio: 0.9,
        });

        let error = commit_snap_session(&session).expect_err("mismatch must fail");
        assert!(error.to_string().contains("snap commit failed"));
        super::super::undo::clear_window_undo_history();
    }
}

//! Public legacy compatibility wrappers.
//!
//! Every existing signature is preserved, but ALL execution routes through
//! the plan/transaction engine: resolve identity -> compile an immutable
//! plan -> preflight -> bounded per-PID execution -> readback verification.
//! This module contains NO direct AX writes, no close-button presses, no
//! application activation, and never decodes a PID from a numeric window ID.
//! `Ok(())` is returned ONLY for a verified `Succeeded` receipt.

use anyhow::{Context, Result};
use tracing::{info, instrument};

use super::cache::OwnedCachedWindowRef;
use super::legacy::{execute_legacy_window_action, LegacyWindowAction};
use super::transaction::TransactionReceipt;
use super::types::{Bounds, TilePosition, WindowObservation};

/// Resolve a legacy numeric ID to its current observation + retained AX ref.
///
/// On a miss, refreshes the registry once and retries. PID and all authority
/// come from the OBSERVATION — never decoded from the numeric ID. (Used by
/// the snap poll path, which reads but never writes.)
pub(super) fn resolve_action_target(
    window_id: u32,
) -> Result<(WindowObservation, OwnedCachedWindowRef)> {
    fn attempt(window_id: u32) -> Result<(WindowObservation, OwnedCachedWindowRef)> {
        let handle = super::registry::resolve_legacy_window_id(window_id)?;
        let observation = super::registry::resolve_handle(handle)?;
        let window = super::registry::retained_window(handle)?;
        Ok((observation, window))
    }

    attempt(window_id).or_else(|_| {
        let _ = super::registry::refresh_window_registry();
        attempt(window_id)
    })
}

/// Tile a window to a predefined position on the screen.
#[instrument]
pub fn tile_window(window_id: u32, position: TilePosition) -> Result<()> {
    // NextDisplay/PreviousDisplay are routing positions, preserved exactly.
    let receipt = match position {
        TilePosition::NextDisplay => {
            execute_legacy_window_action(LegacyWindowAction::MoveToNextDisplay { window_id })?
        }
        TilePosition::PreviousDisplay => {
            execute_legacy_window_action(LegacyWindowAction::MoveToPreviousDisplay { window_id })?
        }
        _ => execute_legacy_window_action(LegacyWindowAction::Tile {
            window_id,
            position,
        })?,
    };
    let result = TransactionReceipt::into_legacy_result(receipt);
    if result.is_ok() {
        info!(window_id, ?position, "Tiled window");
    }
    result
}

/// Move a window to the next display (cycles through available displays).
#[instrument]
pub fn move_to_next_display(window_id: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::MoveToNextDisplay { window_id })
        .and_then(TransactionReceipt::into_legacy_result)
}

/// Move a window to the previous display (cycles through available displays).
#[instrument]
pub fn move_to_previous_display(window_id: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::MoveToPreviousDisplay { window_id })
        .and_then(TransactionReceipt::into_legacy_result)
}

/// Move a window to a new position.
#[instrument]
pub fn move_window(window_id: u32, x: i32, y: i32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::Move { window_id, x, y })
        .and_then(TransactionReceipt::into_legacy_result)
        .inspect(|_| info!(window_id, x, y, "Moved window"))
}

/// Resize a window to new dimensions.
#[instrument]
pub fn resize_window(window_id: u32, width: u32, height: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::Resize {
        window_id,
        width,
        height,
    })
    .and_then(TransactionReceipt::into_legacy_result)
    .inspect(|_| info!(window_id, width, height, "Resized window"))
}

/// Set the complete bounds (position and size) of a window.
#[instrument]
pub fn set_window_bounds(window_id: u32, bounds: Bounds) -> Result<()> {
    let handle = super::registry::resolve_legacy_window_id(window_id)
        .or_else(|_| {
            let _ = super::registry::refresh_window_registry();
            super::registry::resolve_legacy_window_id(window_id)
        })
        .context("Window not found")?;
    let observation = super::registry::resolve_handle(handle)?;
    let plan = super::plan::build_explicit_bounds_plan(
        super::diagnostics::OperationSource::LegacyAction,
        &observation,
        bounds,
        super::plan::RollbackPolicy::Strict,
        true,
    );
    super::transaction::execute_plan(&plan)
        .and_then(TransactionReceipt::into_legacy_result)
        .inspect(|_| {
            info!(
                window_id,
                x = bounds.x,
                y = bounds.y,
                width = bounds.width,
                height = bounds.height,
                "Set window bounds"
            );
        })
}

/// Minimize a window.
#[instrument]
pub fn minimize_window(window_id: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::Minimize { window_id })
        .and_then(TransactionReceipt::into_legacy_result)
        .inspect(|_| info!(window_id, "Minimized window"))
}

/// Maximize a window (fills the display without entering fullscreen mode).
#[instrument]
pub fn maximize_window(window_id: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::Maximize { window_id })
        .and_then(TransactionReceipt::into_legacy_result)
        .inspect(|_| info!(window_id, "Maximized window"))
}

/// Close a window.
///
/// Note: an application save prompt keeps the window alive; that is
/// truthfully reported as an error rather than silent success.
#[instrument]
pub fn close_window(window_id: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::Close { window_id })
        .and_then(TransactionReceipt::into_legacy_result)
        .inspect(|_| info!(window_id, "Closed window"))
}

/// Focus (bring to front) a window.
#[instrument]
pub fn focus_window(window_id: u32) -> Result<()> {
    execute_legacy_window_action(LegacyWindowAction::Focus { window_id })
        .and_then(TransactionReceipt::into_legacy_result)
        .inspect(|_| info!(window_id, "Focused window"))
}

#[cfg(test)]
mod tests {
    use super::super::registry;
    use super::super::test_support::test_env::EnvGuard;
    use super::*;

    fn refreshed() {
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");
    }

    #[test]
    fn every_wrapper_routes_through_the_engine_with_verified_readback() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Doc","pid":9,
                 "bounds":{"x":0,"y":0,"width":800,"height":600}}
            ]}"#,
        );
        refreshed();

        move_window(1, 40, 30).expect("move");
        assert_eq!(
            super::super::test_support::window_state(1).unwrap().bounds,
            Bounds::new(40, 30, 800, 600)
        );
        resize_window(1, 640, 480).expect("resize");
        assert_eq!(
            super::super::test_support::window_state(1).unwrap().bounds,
            Bounds::new(40, 30, 640, 480)
        );
        set_window_bounds(1, Bounds::new(10, 10, 700, 500)).expect("bounds");
        minimize_window(1).expect("minimize");
        assert!(
            super::super::test_support::window_state(1)
                .unwrap()
                .minimized
        );
        focus_window(1).expect("focus");
        assert!(super::super::test_support::window_state(1).unwrap().focused);
    }

    #[test]
    fn wrapper_failure_uses_the_window_engine_error_channel() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Clamped","pid":9,
                 "mutation":{"minWidth":500,"minHeight":400}}
            ]}"#,
        );
        refreshed();
        let error = resize_window(1, 200, 100).expect_err("clamp must fail");
        assert!(error.to_string().starts_with("window_engine:"));
    }

    #[test]
    fn stale_ids_reject_rather_than_target_a_replacement() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(r#"{"windows":[{"id":7,"app":"A","title":"Doc","pid":9}]}"#);
        refreshed();
        assert!(move_window(999, 5, 5).is_err());
    }
}

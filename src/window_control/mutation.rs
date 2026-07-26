//! Bounded low-level mutation primitives.
//!
//! Every AX wait is bounded: elements get a per-element messaging timeout
//! (reset to 0.0 in a best-effort guard drop) so an unresponsive app cannot
//! block a command indefinitely. `AXEnhancedUserInterface` toggling is
//! forbidden. Provider-backed mutations honor the fixture's scripted delays,
//! clamps, offsets, failures, and cancellation.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::app_profiles::BoundsMutationSequence;
use super::ax::{set_window_position, set_window_size};
use super::cf::cf_release;
use super::ffi::{kAXErrorSuccess, AXUIElementRef, AXUIElementSetMessagingTimeout, CFTypeRef};
use super::test_support::{self, ProviderMutationOutcome};
use super::types::Bounds;

// Locked AX timing constants (window-engine-foundation plan).
pub(super) const AX_BACKGROUND_OBSERVATION_TIMEOUT_SECS: f32 = 0.20;
pub(super) const AX_EXPLICIT_PREFLIGHT_TIMEOUT_SECS: f32 = 0.50;
pub(super) const AX_MUTATION_TIMEOUT_SECS: f32 = 0.75;
pub(super) const AX_ROLLBACK_TIMEOUT_SECS: f32 = 0.50;
pub(super) const VERIFY_POLL_INTERVAL: Duration = Duration::from_millis(16);
pub(super) const BOUNDS_VERIFY_DEADLINE: Duration = Duration::from_millis(250);
pub(super) const MINIMIZE_VERIFY_DEADLINE: Duration = Duration::from_millis(300);
pub(super) const FOCUS_VERIFY_DEADLINE: Duration = Duration::from_millis(500);
pub(super) const CLOSE_VERIFY_DEADLINE: Duration = Duration::from_millis(750);
pub(super) const TRANSACTION_RESPONSE_DEADLINE: Duration = Duration::from_secs(3);

/// RAII guard for a bounded AX messaging timeout on one element.
///
/// Applies the timeout on construction and best-effort resets it to 0.0
/// (the app default) on drop, so timeout changes can never linger.
pub(super) struct AxMessagingTimeoutGuard {
    element: AXUIElementRef,
}

impl AxMessagingTimeoutGuard {
    pub(super) fn apply(element: AXUIElementRef, seconds: f32) -> Result<Self> {
        // SAFETY: element is a live AX element retained by the caller.
        let result = unsafe { AXUIElementSetMessagingTimeout(element, seconds) };
        anyhow::ensure!(
            result == kAXErrorSuccess,
            "failed to set AX messaging timeout: {result}"
        );
        Ok(Self { element })
    }
}

impl Drop for AxMessagingTimeoutGuard {
    fn drop(&mut self) {
        // SAFETY: best-effort reset; element outlives the guard by contract.
        unsafe {
            let _ = AXUIElementSetMessagingTimeout(self.element, 0.0);
        }
    }
}

/// One primitive write within a bounds mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundsWriteOp {
    Position,
    Size,
}

/// The exact write order for each sequence (unit-testable).
pub(super) fn sequence_ops(sequence: BoundsMutationSequence) -> &'static [BoundsWriteOp] {
    match sequence {
        BoundsMutationSequence::PositionThenSize => &[BoundsWriteOp::Position, BoundsWriteOp::Size],
        BoundsMutationSequence::SizeThenPosition => &[BoundsWriteOp::Size, BoundsWriteOp::Position],
        BoundsMutationSequence::PositionSizePosition => &[
            BoundsWriteOp::Position,
            BoundsWriteOp::Size,
            BoundsWriteOp::Position,
        ],
        BoundsMutationSequence::SizePositionSize => &[
            BoundsWriteOp::Size,
            BoundsWriteOp::Position,
            BoundsWriteOp::Size,
        ],
    }
}

/// Apply a bounds mutation in the profile's write order.
pub(super) fn apply_bounds_sequence(
    window: AXUIElementRef,
    requested: Bounds,
    sequence: BoundsMutationSequence,
) -> Result<()> {
    for op in sequence_ops(sequence) {
        match op {
            BoundsWriteOp::Position => set_window_position(window, requested.x, requested.y)?,
            BoundsWriteOp::Size => set_window_size(window, requested.width, requested.height)?,
        }
    }
    Ok(())
}

/// Set AXMinimized on a window element.
pub(super) fn set_window_minimized(window: AXUIElementRef, minimized: bool) -> Result<()> {
    let attr = super::cf::try_create_cf_string("AXMinimized")?;
    // SAFETY: kCFBoolean constants are process-lifetime CF singletons.
    let result = unsafe {
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            static kCFBooleanTrue: CFTypeRef;
            static kCFBooleanFalse: CFTypeRef;
        }
        let value = if minimized {
            kCFBooleanTrue
        } else {
            kCFBooleanFalse
        };
        super::ffi::AXUIElementSetAttributeValue(window, attr, value)
    };
    cf_release(attr);
    if result != kAXErrorSuccess {
        bail!("Failed to set AXMinimized={minimized}: error {result}");
    }
    Ok(())
}

/// Activate an application by PID. NEVER accepts or decodes a window id.
pub(crate) fn activate_application(pid: i32) -> Result<()> {
    // SAFETY: standard NSWorkspace/NSRunningApplication messaging with
    // null-checked pointers.
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        let app_class =
            Class::get("NSRunningApplication").context("Failed to get NSRunningApplication")?;
        let app: *mut Object = msg_send![app_class, runningApplicationWithProcessIdentifier: pid];
        anyhow::ensure!(!app.is_null(), "no running application with pid {pid}");
        // NSApplicationActivateIgnoringOtherApps
        let _: bool = msg_send![app, activateWithOptions: 1u64];
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Provider backend
// ---------------------------------------------------------------------------

/// Outcome adapter: provider outcome -> Result, mirroring AX setter errors.
fn provider_outcome_to_result(outcome: ProviderMutationOutcome) -> Result<()> {
    match outcome {
        ProviderMutationOutcome::Applied => Ok(()),
        ProviderMutationOutcome::Destroyed => bail!("provider window destroyed during mutation"),
        ProviderMutationOutcome::Cancelled => bail!("provider mutation cancelled"),
        ProviderMutationOutcome::SetterError(message) => bail!("provider setter error: {message}"),
    }
}

/// Apply a bounds change against the provider, honoring clamps and deltas.
pub(super) fn provider_set_bounds(
    provider_id: u32,
    requested: Bounds,
    cancelled: Option<&Arc<AtomicBool>>,
) -> Result<()> {
    let outcome = test_support::apply_mutation(provider_id, cancelled, |window| {
        window.bounds =
            test_support::resolve_requested_bounds(&window.definition.mutation, requested);
    })?;
    provider_outcome_to_result(outcome)
}

/// Apply a position-only change against the provider.
pub(super) fn provider_set_position(
    provider_id: u32,
    x: i32,
    y: i32,
    cancelled: Option<&Arc<AtomicBool>>,
) -> Result<()> {
    let outcome = test_support::apply_mutation(provider_id, cancelled, |window| {
        let requested = Bounds::new(x, y, window.bounds.width, window.bounds.height);
        window.bounds =
            test_support::resolve_requested_bounds(&window.definition.mutation, requested);
    })?;
    provider_outcome_to_result(outcome)
}

/// Apply a size-only change against the provider.
pub(super) fn provider_set_size(
    provider_id: u32,
    width: u32,
    height: u32,
    cancelled: Option<&Arc<AtomicBool>>,
) -> Result<()> {
    let outcome = test_support::apply_mutation(provider_id, cancelled, |window| {
        let requested = Bounds::new(window.bounds.x, window.bounds.y, width, height);
        window.bounds =
            test_support::resolve_requested_bounds(&window.definition.mutation, requested);
    })?;
    provider_outcome_to_result(outcome)
}

/// Set minimized state against the provider.
pub(super) fn provider_set_minimized(
    provider_id: u32,
    minimized: bool,
    cancelled: Option<&Arc<AtomicBool>>,
) -> Result<()> {
    let outcome = test_support::apply_mutation(provider_id, cancelled, |window| {
        window.minimized = minimized;
    })?;
    provider_outcome_to_result(outcome)
}

/// Focus a provider window (clears focus from every sibling).
pub(super) fn provider_focus(provider_id: u32, cancelled: Option<&Arc<AtomicBool>>) -> Result<()> {
    let outcome = test_support::apply_mutation(provider_id, cancelled, |window| {
        window.focused = true;
        window.main = true;
    })?;
    provider_outcome_to_result(outcome)?;
    test_support::with_state(|state| {
        for window in state.windows.iter_mut() {
            if window.id != provider_id {
                window.focused = false;
                window.main = false;
            }
        }
    })?;
    Ok(())
}

/// Close a provider window (a save-prompt fixture leaves the window alive).
pub(super) fn provider_close(provider_id: u32, cancelled: Option<&Arc<AtomicBool>>) -> Result<()> {
    let outcome = test_support::apply_mutation(provider_id, cancelled, |window| {
        if !window.definition.mutation.close_leaves_window {
            window.destroyed = true;
        }
    })?;
    match outcome {
        // Destroy-on-attempt fixtures already vanish; that IS a close.
        ProviderMutationOutcome::Destroyed => Ok(()),
        other => provider_outcome_to_result(other),
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry;
    use super::super::test_support::test_env::EnvGuard;
    use super::*;

    #[test]
    fn all_four_sequences_write_in_their_declared_order() {
        use BoundsWriteOp::{Position, Size};
        assert_eq!(
            sequence_ops(BoundsMutationSequence::PositionThenSize),
            &[Position, Size]
        );
        assert_eq!(
            sequence_ops(BoundsMutationSequence::SizeThenPosition),
            &[Size, Position]
        );
        assert_eq!(
            sequence_ops(BoundsMutationSequence::PositionSizePosition),
            &[Position, Size, Position]
        );
        assert_eq!(
            sequence_ops(BoundsMutationSequence::SizePositionSize),
            &[Size, Position, Size]
        );
    }

    #[test]
    fn provider_bounds_mutations_honor_clamps_and_persist() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Clamped",
                 "mutation":{"minWidth":500,"minHeight":400}}
            ]}"#,
        );
        provider_set_bounds(1, Bounds::new(10, 10, 300, 200), None).expect("mutation");
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds, Bounds::new(10, 10, 500, 400));
    }

    #[test]
    fn provider_close_respects_save_prompt_fixture() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Prompt",
                 "mutation":{"closeLeavesWindow":true}},
                {"id":2,"app":"A","title":"Normal"}
            ]}"#,
        );
        provider_close(1, None).expect("close");
        assert!(
            super::super::test_support::window_state(1).is_ok(),
            "save-prompt window must survive"
        );
        provider_close(2, None).expect("close");
        assert!(
            super::super::test_support::window_state(2).is_err(),
            "ordinary close must remove the window"
        );
    }

    #[test]
    fn provider_focus_is_exclusive() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"One","focused":true},
                {"id":2,"app":"A","title":"Two"}
            ]}"#,
        );
        provider_focus(2, None).expect("focus");
        let one = super::super::test_support::window_state(1).expect("state");
        let two = super::super::test_support::window_state(2).expect("state");
        assert!(!one.focused);
        assert!(two.focused);
    }

    #[test]
    fn cancellation_prevents_provider_mutation() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Slow","mutation":{"delayMs":200}}
            ]}"#,
        );
        let cancelled = Arc::new(AtomicBool::new(true));
        let error = provider_set_position(1, 99, 99, Some(&cancelled)).expect_err("must cancel");
        assert!(error.to_string().contains("cancelled"));
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds.x, 0);
    }

    #[test]
    fn scripted_setter_failures_surface_as_errors() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Flaky","mutation":{"failOnAttempt":1}}
            ]}"#,
        );
        let error = provider_set_size(1, 700, 500, None).expect_err("scripted failure");
        assert!(error.to_string().contains("setter error"));
        // Second attempt succeeds (failOnAttempt only fires on attempt 1).
        provider_set_size(1, 700, 500, None).expect("second attempt");
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds.width, 700);
    }
}

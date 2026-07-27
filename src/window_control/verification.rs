//! Readback verification predicates.
//!
//! A successful setter call WITHOUT satisfying readback is never success.
//! Predicates compare the observed post-state against the request within the
//! app profile's tolerance; unrequested components must stay unchanged
//! within tolerance.

use super::app_profiles::BoundsTolerance;
use super::plan::RequestedMutation;
use super::types::Bounds;

fn within(a: i32, b: i32, tolerance: i32) -> bool {
    (a - b).abs() <= tolerance
}

/// Observed post-mutation window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedState {
    pub bounds: Bounds,
    pub minimized: bool,
    pub focused: bool,
    /// False when the window no longer exists / AX element is invalid.
    pub alive: bool,
}

/// Does the observed state satisfy the request?
pub(super) fn request_satisfied(
    request: &RequestedMutation,
    before: Bounds,
    observed: &ObservedState,
    tolerance: BoundsTolerance,
) -> bool {
    match request {
        RequestedMutation::SetPosition { x, y } => {
            observed.alive
                && within(observed.bounds.x, *x, tolerance.position)
                && within(observed.bounds.y, *y, tolerance.position)
                && within(
                    observed.bounds.width as i32,
                    before.width as i32,
                    tolerance.size,
                )
                && within(
                    observed.bounds.height as i32,
                    before.height as i32,
                    tolerance.size,
                )
        }
        RequestedMutation::SetSize { width, height } => {
            observed.alive
                && within(observed.bounds.width as i32, *width as i32, tolerance.size)
                && within(
                    observed.bounds.height as i32,
                    *height as i32,
                    tolerance.size,
                )
                && within(observed.bounds.x, before.x, tolerance.position)
                && within(observed.bounds.y, before.y, tolerance.position)
        }
        RequestedMutation::SetBounds(requested) => {
            observed.alive
                && within(observed.bounds.x, requested.x, tolerance.position)
                && within(observed.bounds.y, requested.y, tolerance.position)
                && within(
                    observed.bounds.width as i32,
                    requested.width as i32,
                    tolerance.size,
                )
                && within(
                    observed.bounds.height as i32,
                    requested.height as i32,
                    tolerance.size,
                )
        }
        RequestedMutation::SetMinimized(minimized) => {
            observed.alive && observed.minimized == *minimized
        }
        RequestedMutation::Focus => observed.alive && observed.focused,
        // Close succeeds when the window is GONE.
        RequestedMutation::Close => !observed.alive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: BoundsTolerance = BoundsTolerance {
        position: 2,
        size: 2,
    };

    fn observed(bounds: Bounds) -> ObservedState {
        ObservedState {
            bounds,
            minimized: false,
            focused: false,
            alive: true,
        }
    }

    #[test]
    fn set_position_requires_position_and_unchanged_size() {
        let before = Bounds::new(0, 0, 800, 600);
        let request = RequestedMutation::SetPosition { x: 100, y: 50 };
        assert!(request_satisfied(
            &request,
            before,
            &observed(Bounds::new(101, 49, 800, 600)),
            TOL
        ));
        // Wrong position fails.
        assert!(!request_satisfied(
            &request,
            before,
            &observed(Bounds::new(120, 50, 800, 600)),
            TOL
        ));
        // Size drift beyond tolerance fails even at the right position.
        assert!(!request_satisfied(
            &request,
            before,
            &observed(Bounds::new(100, 50, 780, 600)),
            TOL
        ));
    }

    #[test]
    fn clamped_bounds_are_failure_not_success() {
        let before = Bounds::new(0, 0, 800, 600);
        let request = RequestedMutation::SetBounds(Bounds::new(10, 10, 300, 200));
        // The app clamped to a larger minimum size: NOT success.
        assert!(!request_satisfied(
            &request,
            before,
            &observed(Bounds::new(10, 10, 500, 400)),
            TOL
        ));
        assert!(request_satisfied(
            &request,
            before,
            &observed(Bounds::new(11, 9, 301, 199)),
            TOL
        ));
    }

    #[test]
    fn close_succeeds_only_when_the_window_is_gone() {
        let before = Bounds::new(0, 0, 800, 600);
        let mut gone = observed(before);
        gone.alive = false;
        assert!(request_satisfied(
            &RequestedMutation::Close,
            before,
            &gone,
            TOL
        ));
        // Save-prompt simulation: window still alive -> failure.
        assert!(!request_satisfied(
            &RequestedMutation::Close,
            before,
            &observed(before),
            TOL
        ));
    }

    #[test]
    fn minimize_and_focus_check_their_flags() {
        let before = Bounds::new(0, 0, 800, 600);
        let mut state = observed(before);
        state.minimized = true;
        assert!(request_satisfied(
            &RequestedMutation::SetMinimized(true),
            before,
            &state,
            TOL
        ));
        assert!(!request_satisfied(
            &RequestedMutation::SetMinimized(false),
            before,
            &state,
            TOL
        ));
        let mut focused = observed(before);
        focused.focused = true;
        assert!(request_satisfied(
            &RequestedMutation::Focus,
            before,
            &focused,
            TOL
        ));
    }
}

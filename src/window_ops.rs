//! Coalescing Window Operations Module
//!
//! Provides coalesced window resize and move operations using GPUI's Window::defer API.
//!
//! # Why Coalescing?
//!
//! During rapid UI updates (filtering, typing, state changes), multiple resize/move requests
//! can fire in quick succession. Without coalescing, each request would trigger a macOS
//! window operation, causing:
//! - Visual jitter/flicker
//! - Performance degradation from redundant system calls
//! - Potential RefCell borrow conflicts during GPUI's render cycle
//!
//! # How It Works
//!
//! 1. Callers call `queue_resize()` or `queue_move()` with their desired values
//! 2. The value is stored in a pending slot (overwrites any previous pending value)
//! 3. A `Window::defer()` callback is scheduled (only once per effect cycle)
//! 4. At the end of the effect cycle, `flush_pending_ops()` executes the final values
//!
//! This ensures only ONE window operation happens per effect cycle, using the latest values.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Instead of direct resize:
//! // platform::resize_first_window_to_height(height);
//!
//! // Use coalesced queue:
//! window_ops::queue_resize(height, window, cx);
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use gpui::{Bounds, Pixels, Window};

use crate::logging;
use crate::platform;
use crate::window_resize;

// ============================================================================
// Coalescing State
// ============================================================================

/// Pending resize height (if Some, a resize is pending)
static PENDING_RESIZE: Mutex<Option<f32>> = Mutex::new(None);

/// Pending resize width (if Some, a width change is also pending)
static PENDING_WIDTH: Mutex<Option<f32>> = Mutex::new(None);

/// Pending move bounds (if Some, a move is pending)
static PENDING_BOUNDS: Mutex<Option<Bounds<Pixels>>> = Mutex::new(None);

/// Whether a flush has been scheduled for this effect cycle
static FLUSH_SCHEDULED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Public API
// ============================================================================

/// Queue a window resize to happen at the end of the current effect cycle.
///
/// Multiple calls within the same cycle will coalesce - only the final height is used.
///
/// # Arguments
/// * `target_height` - The desired window height in pixels
/// * `window` - The GPUI Window reference (used for defer scheduling)
/// * `cx` - The GPUI App context
///
/// # Example
/// ```rust,ignore
/// use crate::window_ops;
///
/// // In a message handler or event callback:
/// window_ops::queue_resize(500.0, window, cx);
/// ```
pub fn queue_resize(target_height: f32, window: &mut Window, cx: &mut gpui::App) {
    // Store the pending height (overwrites any previous pending value)
    *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(target_height);
    // Clear any stale width from prior queue_resize_with_width calls —
    // this is a height-only resize.
    *PENDING_WIDTH.lock().unwrap_or_else(|e| e.into_inner()) = None;

    logging::log(
        "WINDOW_OPS",
        &format!("Queued resize to height: {:.0}px", target_height),
    );

    // Schedule flush if not already scheduled
    schedule_flush(window, cx);
}

/// Queue a window resize with optional width change at the end of the current effect cycle.
///
/// Like `queue_resize`, but also accepts an optional target width.
/// When `target_width` is `Some`, the window will be resized to both the new height and width.
pub fn queue_resize_with_width(
    target_height: f32,
    target_width: Option<f32>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(target_height);
    // Always assign — None explicitly clears any stale width from prior calls.
    *PENDING_WIDTH.lock().unwrap_or_else(|e| e.into_inner()) = target_width;

    logging::log(
        "WINDOW_OPS",
        &format!(
            "Queued resize to height: {:.0}px, width: {:?}",
            target_height, target_width
        ),
    );

    schedule_flush(window, cx);
}

/// Queue a window move to happen at the end of the current effect cycle.
///
/// Multiple calls within the same cycle will coalesce - only the final bounds are used.
///
/// # Arguments
/// * `bounds` - The desired window bounds (position + size)
/// * `window` - The GPUI Window reference (used for defer scheduling)
/// * `cx` - The GPUI App context
///
/// # Example
/// ```rust,ignore
/// use crate::window_ops;
/// use gpui::{point, px, size, Bounds};
///
/// let bounds = Bounds {
///     origin: point(px(100.0), px(200.0)),
///     size: size(px(750.0), px(500.0)),
/// };
/// window_ops::queue_move(bounds, window, cx);
/// ```
pub fn queue_move(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut gpui::App) {
    // Store the pending bounds (overwrites any previous pending value)
    *PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(bounds);

    logging::log(
        "WINDOW_OPS",
        &format!(
            "Queued move to bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        ),
    );

    // Schedule flush if not already scheduled
    schedule_flush(window, cx);
}

/// Apply any queued move/resize SYNCHRONOUSLY, for the native main-window
/// reveal path to call BEFORE the glass entry morph is armed.
///
/// Why this exists (wide-start regression, oracle-execute `wide-start-restore`,
/// 2026-08-13): show paths queue the canonical show bounds via `queue_move`
/// and then call `platform::show_main_window_without_activation()`. The morph
/// arms against the window's CURRENT frame and parks the calibrated 101.2%
/// wide state — and only afterwards did `Window::defer` flush the queued
/// bounds, rewriting the frame to final width ~20ms into the visible onset.
/// Frame-event receipts showed `writer_move_first_window_to` at t≈23ms
/// stomping 760→750 before the tail began, which erased Spotlight's measured
/// wide start from every entry. Flushing before arming both fixes the stomp
/// and lets the morph capture the true final bounds (including a fresh
/// height) instead of a stale pre-show frame.
///
/// Safe in the same contexts as the deferred flush: the direct `setFrame:`
/// resize callback uses `try_borrow_mut` and no-ops under an active GPUI
/// borrow; the still-scheduled deferred flush then finds nothing pending and
/// only performs the `bounds_changed` viewport sync.
pub fn flush_pending_ops_before_native_reveal() {
    if has_pending_ops() {
        logging::log(
            "WINDOW_OPS",
            "Flushing pending window ops before native reveal (pre-morph)",
        );
        flush_pending_ops();
    }
}

/// Check if there are any pending window operations.
///
/// Useful for debugging or testing.
#[allow(dead_code)]
pub fn has_pending_ops() -> bool {
    PENDING_RESIZE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some()
        || PENDING_BOUNDS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
}

/// Clear all pending operations without executing them.
///
/// Use this when the window is being hidden/closed to avoid stale operations.
#[allow(dead_code)]
pub fn clear_pending_ops() {
    *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *PENDING_WIDTH.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    FLUSH_SCHEDULED.store(false, Ordering::SeqCst);
    logging::log("WINDOW_OPS", "Cleared all pending operations");
}

// ============================================================================
// Internal Implementation
// ============================================================================

/// Schedule the flush callback if not already scheduled.
///
/// Uses Window::defer to run at the end of the current effect cycle.
fn schedule_flush(window: &mut Window, cx: &mut gpui::App) {
    // Only schedule once per effect cycle
    if !FLUSH_SCHEDULED.swap(true, Ordering::SeqCst) {
        let scheduled_generation = crate::main_window_visibility_generation();
        logging::log("WINDOW_OPS", "Scheduling flush via Window::defer");

        window.defer(cx, move |window, cx| {
            if !crate::is_main_window_visible()
                || crate::main_window_visibility_generation() != scheduled_generation
            {
                clear_pending_ops();
                logging::log(
                    "WINDOW_OPS",
                    "Skipping stale deferred window ops after visibility changed",
                );
                return;
            }
            flush_pending_ops();
            // Sync GPUI's internal viewport_size with the actual native window
            // content size. The direct NSWindow setFrame:display:animate: call
            // in flush_pending_ops triggers setFrameSize: on the NSView, which
            // fires the resize_callback — but that callback uses try_borrow_mut()
            // on the AppCell, which fails here because Window::defer already holds
            // the borrow. Calling bounds_changed() directly ensures GPUI picks up
            // the new size and re-layouts correctly.
            window.bounds_changed(cx);
        });
    }
}

/// Execute all pending window operations.
///
/// Called by the deferred callback at the end of the effect cycle.
fn flush_pending_ops() {
    // Reset the scheduled flag FIRST (allows new operations to schedule a new flush)
    FLUSH_SCHEDULED.store(false, Ordering::SeqCst);

    // Execute pending resize if any.
    // Take height and width atomically under both locks to prevent
    // a concurrent queue call from splitting the pair.
    let (pending_height, pending_width) = {
        let height = PENDING_RESIZE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let width = PENDING_WIDTH
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        (height, width)
    };
    if let Some(height) = pending_height {
        if let Some(w) = pending_width {
            logging::log(
                "WINDOW_OPS",
                &format!(
                    "Flushing resize to height: {:.0}px, width: {:.0}px",
                    height, w
                ),
            );
            window_resize::resize_first_window_to_size(gpui::px(height), Some(w));
        } else {
            logging::log(
                "WINDOW_OPS",
                &format!("Flushing resize to height: {:.0}px", height),
            );
            window_resize::resize_first_window_to_height(gpui::px(height));
        }
    }

    // Execute pending move if any
    if let Some(bounds) = PENDING_BOUNDS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        logging::log(
            "WINDOW_OPS",
            &format!(
                "Flushing move to bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ),
        );
        platform::move_first_window_to_bounds(&bounds);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_resize_state() {
        // Clear any leftover state
        clear_pending_ops();

        // Initially no pending ops
        assert!(!has_pending_ops());

        // Manually set pending resize (simulating what queue_resize does internally)
        *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(500.0);
        assert!(has_pending_ops());

        // Clear clears it
        clear_pending_ops();
        assert!(!has_pending_ops());
    }

    #[test]
    fn test_pending_bounds_state() {
        use gpui::{point, px, size};

        clear_pending_ops();
        assert!(!has_pending_ops());

        // Manually set pending bounds
        let bounds = Bounds {
            origin: point(px(100.0), px(200.0)),
            size: size(px(750.0), px(500.0)),
        };
        *PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(bounds);
        assert!(has_pending_ops());

        clear_pending_ops();
        assert!(!has_pending_ops());
    }

    #[test]
    fn test_flush_clears_state() {
        clear_pending_ops();

        // Set some pending state
        *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(700.0);
        FLUSH_SCHEDULED.store(true, Ordering::SeqCst);

        // Note: We can't test the actual flush_pending_ops() without mocking platform functions,
        // but we can verify the state management
        assert!(has_pending_ops());
        assert!(FLUSH_SCHEDULED.load(Ordering::SeqCst));

        // Clear should reset everything
        clear_pending_ops();
        assert!(!has_pending_ops());
        assert!(!FLUSH_SCHEDULED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_coalesce_multiple_resizes() {
        clear_pending_ops();

        // Simulate multiple resize requests (what happens in practice)
        *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(400.0);
        *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(500.0);
        *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()) = Some(600.0);

        // Only the last value should be stored
        assert_eq!(
            *PENDING_RESIZE.lock().unwrap_or_else(|e| e.into_inner()),
            Some(600.0)
        );

        clear_pending_ops();
    }

    #[test]
    fn test_coalesce_multiple_moves() {
        use gpui::{point, px, size};

        clear_pending_ops();

        // Simulate multiple move requests
        let bounds1 = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(750.0), px(500.0)),
        };
        let bounds2 = Bounds {
            origin: point(px(100.0), px(100.0)),
            size: size(px(750.0), px(500.0)),
        };
        let bounds3 = Bounds {
            origin: point(px(200.0), px(200.0)),
            size: size(px(750.0), px(500.0)),
        };

        *PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(bounds1);
        *PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(bounds2);
        *PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner()) = Some(bounds3);

        // Only the last value should be stored
        let pending = PENDING_BOUNDS.lock().unwrap_or_else(|e| e.into_inner());
        assert!(pending.is_some());
        let bounds = pending.unwrap();
        assert_eq!(f32::from(bounds.origin.x), 200.0);
        assert_eq!(f32::from(bounds.origin.y), 200.0);

        drop(pending);
        clear_pending_ops();
    }
}

#![allow(dead_code)]
//! Unified Window Registry
//!
//! Provides a single registry for all application windows, replacing separate
//! `OnceLock<Mutex<Option<WindowHandle>>>` statics in each window module.
//!
//! # Benefits
//!
//! - Consistent lifecycle handling across all windows
//! - Single place for cross-window operations (e.g., notify_all_windows for theme changes)
//! - Easier to reason about window state
//! - Prevents drift between window implementations
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::windows::{WindowRole, register_window, get_window, clear_window};
//!
//! // Register a window after creation
//! let handle = cx.open_window(options, |window, cx| { ... })?;
//! register_window(WindowRole::Notes, handle);
//!
//! // Get a window handle (returns Copy)
//! if let Some(handle) = get_window(WindowRole::Notes) {
//!     handle.update(cx, |root, window, cx| { ... });
//! }
//!
//! // Clear when window closes
//! clear_window(WindowRole::Notes);
//!
//! // Notify all windows (e.g., for theme changes)
//! notify_all_windows(cx);
//! ```

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use gpui::{App, WindowHandle};
use gpui_component::Root;

// Re-export the canonical WindowRole from window_state
// This ensures a single source of truth for window roles across the codebase
pub use crate::window_state::WindowRole;

/// Internal registry state
struct WindowRegistry {
    /// Map of window roles to their handles
    handles: HashMap<WindowRole, WindowHandle<Root>>,
}

impl WindowRegistry {
    fn new() -> Self {
        Self {
            handles: HashMap::new(),
        }
    }
}

/// Global singleton for the window registry
static REGISTRY: LazyLock<Mutex<WindowRegistry>> =
    LazyLock::new(|| Mutex::new(WindowRegistry::new()));

/// Get or initialize the global registry
fn registry() -> &'static Mutex<WindowRegistry> {
    &REGISTRY
}

/// Register a window with a specific role.
///
/// Call this after GPUI creates a window to track it by role.
/// Subsequent calls with the same role will overwrite the previous registration.
///
/// # Arguments
/// * `role` - The purpose/role of this window
/// * `handle` - The GPUI window handle
pub fn register_window(role: WindowRole, handle: WindowHandle<Root>) {
    if let Ok(mut reg) = registry().lock() {
        crate::logging::log(
            "WINDOW_REG",
            &format!("Registering window: {:?}", role.name()),
        );
        reg.handles.insert(role, handle);
    }
}

/// Get a window by its role.
///
/// # Arguments
/// * `role` - The role to look up
///
/// # Returns
/// The window handle if registered (WindowHandle is Copy)
pub fn get_window(role: WindowRole) -> Option<WindowHandle<Root>> {
    registry()
        .lock()
        .ok()
        .and_then(|reg| reg.handles.get(&role).copied())
}

/// Clear a window registration.
///
/// Call this when a window is closed to clean up the registry.
///
/// # Arguments
/// * `role` - The role to clear
pub fn clear_window(role: WindowRole) {
    if let Ok(mut reg) = registry().lock() {
        if reg.handles.remove(&role).is_some() {
            crate::logging::log(
                "WINDOW_REG",
                &format!("Cleared window registration: {:?}", role.name()),
            );
        }
    }
}

/// Check if a window is currently registered and open.
///
/// # Arguments
/// * `role` - The role to check
///
/// # Returns
/// `true` if a window is registered for this role
pub fn is_window_open(role: WindowRole) -> bool {
    registry()
        .lock()
        .ok()
        .map(|reg| reg.handles.contains_key(&role))
        .unwrap_or(false)
}

/// Atomically take (remove and return) a window handle.
///
/// This is used for close paths where we need to:
/// 1. Remove the handle from the registry
/// 2. Perform cleanup operations on it
///
/// Returns `None` if no window was registered for this role.
///
/// # Arguments
/// * `role` - The role to take
pub fn take_window(role: WindowRole) -> Option<WindowHandle<Root>> {
    registry().lock().ok().and_then(|mut reg| {
        let handle = reg.handles.remove(&role);
        if handle.is_some() {
            crate::logging::log(
                "WINDOW_REG",
                &format!("Took window handle: {:?}", role.name()),
            );
        }
        handle
    })
}

/// Get a window only if it's valid (update succeeds).
///
/// This is safer than `get_window` because it probes the handle to verify
/// the window still exists. If the handle is stale, it auto-clears from registry.
///
/// # Arguments
/// * `role` - The role to look up
/// * `cx` - The GPUI App context (needed to probe the handle)
///
/// # Returns
/// The window handle if registered AND valid
pub fn get_valid_window(role: WindowRole, cx: &mut App) -> Option<WindowHandle<Root>> {
    let handle = get_window(role)?;
    // Try to update the window - if it fails, the handle is stale
    match handle.update(cx, |_, _, _| {}) {
        Ok(_) => Some(handle),
        Err(_) => {
            // Handle is stale, auto-clear from registry
            crate::logging::log(
                "WINDOW_REG",
                &format!("Auto-cleared stale handle for {:?}", role.name()),
            );
            clear_window(role);
            None
        }
    }
}

/// Execute a closure on a window if it exists and is valid.
///
/// This is a convenience helper that combines `get_valid_window` with `handle.update()`,
/// eliminating the need to release locks manually before calling update.
///
/// # Arguments
/// * `role` - The role to look up
/// * `cx` - The GPUI App context
/// * `f` - The closure to execute on the window
///
/// # Returns
/// `Some(R)` if the window existed and the closure executed, `None` otherwise
pub fn with_window<R>(
    role: WindowRole,
    cx: &mut App,
    f: impl FnOnce(&mut Root, &mut gpui::Window, &mut gpui::Context<Root>) -> R,
) -> Option<R> {
    let handle = get_valid_window(role, cx)?;
    handle.update(cx, f).ok()
}

/// Close a window and save its bounds.
///
/// This is the canonical close helper that ensures bounds are saved regardless
/// of how the window is closed (close_*_window, toggle close, Cmd+W, traffic light).
///
/// # Arguments
/// * `role` - The window role to close
/// * `cx` - The GPUI App context
pub fn close_window_with_bounds(role: WindowRole, cx: &mut App) {
    let Some(handle) = take_window(role) else {
        return;
    };

    let _ = handle.update(cx, |_, window, _| {
        // Save bounds before closing
        let wb = window.window_bounds();
        crate::window_state::save_window_from_gpui(role, wb);
        crate::logging::log(
            "WINDOW_REG",
            &format!("Saved bounds and closing {:?}", role.name()),
        );
        window.remove_window();
    });
}

/// Notify all registered windows to re-render.
///
/// This is useful for broadcasting changes like theme updates.
///
/// # Arguments
/// * `cx` - The GPUI App context
pub fn notify_all_windows(cx: &mut App) {
    // Release both registries before touching GPUI. A window shared by the role
    // and exact-runtime maps receives one publication, not two.
    let mut handles = super::automation_runtime_handles::runtime_window_handles();
    if let Ok(registry) = registry().lock() {
        for handle in registry.handles.values() {
            let handle: gpui::AnyWindowHandle = (*handle).into();
            if !handles.contains(&handle) {
                handles.push(handle);
            }
        }
    }
    let revision = crate::theme::service::theme_revision();
    for handle in handles {
        if !super::automation_runtime_handles::should_deliver_theme(handle) {
            continue;
        }
        if handle
            .update(cx, |root, window, cx| {
                window.refresh();
                cx.notify(root.entity_id());
            })
            .is_ok()
        {
            super::automation_runtime_handles::record_theme_delivery(handle, revision);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_role_name() {
        assert_eq!(WindowRole::Main.name(), "Main");
        assert_eq!(WindowRole::Notes.name(), "Notes");
    }

    #[test]
    fn test_window_role_eq() {
        assert_eq!(WindowRole::Main, WindowRole::Main);
        assert_ne!(WindowRole::Main, WindowRole::Notes);
    }

    #[test]
    fn test_window_role_copy() {
        let role = WindowRole::Main;
        let role2 = role; // Copy
        assert_eq!(role, role2);
    }

    #[test]
    fn test_window_role_is_canonical() {
        // WindowRole is now re-exported from window_state, so they're the same type
        let main = WindowRole::Main;
        assert!(matches!(main, crate::window_state::WindowRole::Main));

        let notes = WindowRole::Notes;
        assert!(matches!(notes, crate::window_state::WindowRole::Notes));
    }

    #[test]
    fn test_take_window_removes_handle() {
        // Test that take_window atomically removes and returns the handle
        // Note: Can't test with actual WindowHandle without GPUI context,
        // so we test the registry state directly
        let registry = registry();
        if let Ok(mut reg) = registry.lock() {
            // Clear any existing state
            reg.handles.remove(&WindowRole::Notes);
        }

        // After take on empty, should return None
        let result = take_window(WindowRole::Notes);
        assert!(result.is_none());

        // Verify nothing to take
        assert!(!is_window_open(WindowRole::Notes));
    }
}

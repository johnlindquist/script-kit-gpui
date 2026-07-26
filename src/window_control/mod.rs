//! Window Control module using macOS Accessibility APIs
//!
//! This module provides window management functionality including:
//! - Listing all visible windows with their properties
//! - Moving, resizing, minimizing, maximizing, and closing windows
//! - Tiling windows to predefined positions (halves, quadrants, fullscreen)
//!
//! ## Architecture
//!
//! Uses macOS Accessibility APIs (AXUIElement) to control windows across applications.
//! The accessibility framework allows querying and modifying window properties for any
//! application, provided the user has granted accessibility permissions.
//!
//! ## Permissions
//!
//! Requires Accessibility permission in System Preferences > Privacy & Security > Accessibility
//!

#![allow(non_upper_case_globals)]
#![allow(dead_code)]

mod actions;
mod ax;
mod cache;
mod cf;
mod diagnostics;
mod display;
mod ffi;
mod identity;
mod query;
mod registry;
mod snap;
mod snap_mode;
mod snap_monitor;
mod snap_overlay;
mod snap_runtime;
mod snap_session;
mod test_support;
mod tiling;
mod types;

use ffi::AXUIElementRef;

fn snap_lock<'a, T>(
    lock: &'a std::sync::Mutex<T>,
    domain: &'static str,
) -> anyhow::Result<std::sync::MutexGuard<'a, T>> {
    lock.lock()
        .map_err(|error| anyhow::anyhow!("snap {domain} lock poisoned: {error}"))
}

pub use actions::{
    close_window, focus_window, maximize_window, minimize_window, move_to_next_display,
    move_to_previous_display, move_window, resize_window, tile_window,
};
pub use query::{get_frontmost_window_of_previous_app, has_accessibility_permission, list_windows};
#[allow(unused_imports)]
pub use snap_mode::{
    current_snap_mode, load_snap_mode_from_preferences, persist_snap_mode, set_snap_mode, SnapMode,
};
pub use snap_monitor::install_snap_drag_monitor;
#[allow(unused_imports)]
pub use snap_runtime::{
    cancel_snap_runtime, finish_snap_runtime, is_snap_runtime_active,
    refresh_snap_runtime_for_mode, start_snap_runtime,
};
pub use types::*;

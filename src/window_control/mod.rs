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
mod app_profiles;
mod ax;
mod cache;
mod capabilities;
mod cf;
mod diagnostics;
mod display;
mod display_topology;
mod executor;
mod ffi;
mod geometry;
mod identity;
mod legacy;
mod mutation;
mod observation;
mod plan;
mod presets;
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
mod transaction;
mod types;
mod undo;
mod verification;

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
pub use display_topology::list_displays;
// Snapshot APIs remain part of the public library even when the binary does not call them.
#[allow(unused_imports)]
pub use display_topology::{display_topology_snapshot, DisplayTopologySnapshot};
pub use query::{get_frontmost_window_of_previous_app, has_accessibility_permission, list_windows};
#[allow(unused_imports)]
pub use registry::{refresh_window_registry, registry_snapshot, RegistrySnapshot};
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
#[allow(unused_imports)]
pub use transaction::{MutationStatus, TransactionReceipt};
pub use types::*;
#[allow(unused_imports)]
pub use undo::{
    clear_window_undo_history, redo_last_window_transaction, undo_last_window_transaction,
};

/// Shared provider-env test guard for OTHER modules' tests (dispatch tests
/// in execute_script). One process-wide lock prevents env races.
#[cfg(test)]
pub(crate) use test_support::test_env as provider_test_env;

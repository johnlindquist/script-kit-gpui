//! Platform-specific window configuration abstraction.
//!
//! This module provides cross-platform abstractions for window behavior configuration,
//! with macOS-specific implementations for floating panel behavior and space management.
//!
//! # macOS Behavior
//!
//! On macOS, this module configures windows as floating panels that:
//! - Float above normal windows (NSFloatingWindowLevel = 3)
//! - Move to the active space when shown (NSWindowCollectionBehaviorMoveToActiveSpace = 2)
//! - Disable window state restoration to prevent position caching
//!
//! # Other Platforms
//!
//! On non-macOS platforms, these functions are no-ops, allowing cross-platform code
//! to call them without conditional compilation at the call site.

pub mod accessibility;
mod display;
#[cfg(target_os = "macos")]
pub(crate) mod glass_button_host;
pub(crate) mod gpui_event_simulator;
pub mod permiso;
pub mod permiso_detect;
pub mod visual_context;

include!("app_window_management.rs");
include!("visibility_focus.rs");
include!("vibrancy_swizzle_materials.rs");
include!("vibrancy_config.rs");
include!("vibrancy_cycle.rs");
include!("secondary_window_config.rs");
include!("positioning.rs");
include!("screenshots_window_open.rs");
include!("selfie_capture.rs");
include!("path_actions.rs");
include!("ai_commands.rs");
include!("screen_capture_sck.rs");
include!("osascript.rs");
include!("cursor.rs");
include!("screen_capture_overlay.rs");
include!("panel_invariants.rs");
include!("tests.rs");

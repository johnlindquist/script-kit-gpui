use anyhow::{bail, Context, Result};
use tracing::{info, instrument};

use super::ax::{
    get_ax_attribute, get_window_position, perform_ax_action, set_window_position, set_window_size,
};
use super::cache::OwnedCachedWindowRef;
use super::cf::{cf_release, try_create_cf_string};
use super::display::get_visible_display_bounds;
use super::ffi::{kAXErrorSuccess, AXUIElementRef, AXUIElementSetAttributeValue, CFTypeRef};
use super::types::{Bounds, TilePosition, WindowObservation};

/// Resolve a legacy numeric ID to its current observation + retained AX ref.
///
/// On a miss, refreshes the registry once and retries. PID and all authority
/// come from the OBSERVATION — never decoded from the numeric ID.
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
pub fn tile_window(window_id: u32, position: TilePosition) -> Result<()> {
    super::tiling::tile_window(window_id, position)
}

/// Move a window to the next display (cycles through available displays).
pub fn move_to_next_display(window_id: u32) -> Result<()> {
    super::tiling::move_to_next_display(window_id)
}

/// Move a window to the previous display (cycles through available displays).
pub fn move_to_previous_display(window_id: u32) -> Result<()> {
    super::tiling::move_to_previous_display(window_id)
}

/// Move a window to a new position.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
/// * `x` - The new X coordinate (screen pixels from left)
/// * `y` - The new Y coordinate (screen pixels from top)
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn move_window(window_id: u32, x: i32, y: i32) -> Result<()> {
    let (_observation, window) = resolve_action_target(window_id)?;

    set_window_position(window.as_ptr(), x, y)?;
    info!(window_id, x, y, "Moved window");
    Ok(())
}

/// Resize a window to new dimensions.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
/// * `width` - The new width in pixels
/// * `height` - The new height in pixels
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn resize_window(window_id: u32, width: u32, height: u32) -> Result<()> {
    let (_observation, window) = resolve_action_target(window_id)?;

    set_window_size(window.as_ptr(), width, height)?;
    info!(window_id, width, height, "Resized window");
    Ok(())
}

/// Set the complete bounds (position and size) of a window.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
/// * `bounds` - The new bounds for the window
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn set_window_bounds(window_id: u32, bounds: Bounds) -> Result<()> {
    let (_observation, window) = resolve_action_target(window_id)?;

    // Set position first, then size
    set_window_position(window.as_ptr(), bounds.x, bounds.y)?;
    set_window_size(window.as_ptr(), bounds.width, bounds.height)?;

    info!(
        window_id,
        x = bounds.x,
        y = bounds.y,
        width = bounds.width,
        height = bounds.height,
        "Set window bounds"
    );
    Ok(())
}

/// Minimize a window.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn minimize_window(window_id: u32) -> Result<()> {
    let (_observation, window) = resolve_action_target(window_id)?;

    // Use AXMinimized attribute to minimize
    let minimize_attr = try_create_cf_string("AXMinimized")?;

    // AXMinimized expects a CFBoolean, so we need to use the attribute differently
    // Actually, we should perform the press action on the minimize button
    // or set the AXMinimized attribute to true

    // Try setting AXMinimized directly with a boolean value
    // SAFETY: kCFBooleanTrue is a well-known CoreFoundation constant. window.as_ptr()
    // returns a valid AXUIElementRef from the cache. minimize_attr is a valid CFStringRef.
    unsafe {
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            static kCFBooleanTrue: CFTypeRef;
        }

        let result = AXUIElementSetAttributeValue(window.as_ptr(), minimize_attr, kCFBooleanTrue);

        cf_release(minimize_attr);

        if result != kAXErrorSuccess {
            bail!("Failed to minimize window: error {}", result);
        }
    }

    info!(window_id, "Minimized window");
    Ok(())
}

/// Maximize a window (fills the display without entering fullscreen mode).
///
/// This positions the window to fill the available display area (excluding
/// dock and menu bar) without entering macOS fullscreen mode.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn maximize_window(window_id: u32) -> Result<()> {
    let (_observation, window) = resolve_action_target(window_id)?;

    // Get current position to determine which display the window is on
    let (current_x, current_y) = get_window_position(window.as_ptr()).unwrap_or((0, 0));

    // Get the display bounds (accounting for menu bar and dock)
    let display_bounds = get_visible_display_bounds(current_x, current_y);

    // Set the window to fill the visible area
    set_window_position(window.as_ptr(), display_bounds.x, display_bounds.y)?;
    set_window_size(window.as_ptr(), display_bounds.width, display_bounds.height)?;

    info!(window_id, "Maximized window");
    Ok(())
}

/// Close a window.
///
/// Note: This performs the close action on the window, which may prompt
/// the user to save unsaved changes depending on the application.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn close_window(window_id: u32) -> Result<()> {
    let (_observation, window) = resolve_action_target(window_id)?;

    // Get the close button and press it
    if let Ok(close_button) = get_ax_attribute(window.as_ptr(), "AXCloseButton") {
        perform_ax_action(close_button as AXUIElementRef, "AXPress")?;
        cf_release(close_button);
    } else {
        bail!("Window does not have a close button");
    }

    info!(window_id, "Closed window");
    Ok(())
}

/// Focus (bring to front) a window.
///
/// # Arguments
/// * `window_id` - The unique window identifier from `list_windows()`
///
/// # Errors
/// Returns error if window not found or operation fails.
#[instrument]
pub fn focus_window(window_id: u32) -> Result<()> {
    let (observation, window) = resolve_action_target(window_id)?;

    // Raise the window
    perform_ax_action(window.as_ptr(), "AXRaise")?;

    // Also activate the owning application. PID comes from the OBSERVATION,
    // never decoded from the numeric window ID.
    let pid = observation.handle.pid;

    // SAFETY: Standard objc messaging to NSWorkspace/NSRunningApplication.
    // We iterate running apps to find the one matching our PID, then activate it.
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        let workspace_class = Class::get("NSWorkspace").context("Failed to get NSWorkspace")?;
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let running_apps: *mut Object = msg_send![workspace, runningApplications];
        let app_count: usize = msg_send![running_apps, count];

        for i in 0..app_count {
            let app: *mut Object = msg_send![running_apps, objectAtIndex: i];
            let app_pid: i32 = msg_send![app, processIdentifier];

            if app_pid == pid {
                let _: bool = msg_send![app, activateWithOptions: 1u64]; // NSApplicationActivateIgnoringOtherApps
                break;
            }
        }
    }

    info!(window_id, "Focused window");
    Ok(())
}

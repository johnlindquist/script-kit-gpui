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

use crate::logging;

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
use crate::window_manager;

// ============================================================================
// Space Management
// ============================================================================

/// Ensure the main window moves to the currently active macOS space when shown.
///
/// This function sets NSWindowCollectionBehaviorMoveToActiveSpace on the main window,
/// which causes it to move to whichever space is currently active when the window
/// becomes visible, rather than forcing the user back to the space where the window
/// was last shown.
///
/// # macOS Behavior
///
/// Uses the WindowManager to get the main window (not keyWindow, which may not exist
/// yet during app startup) and sets the collection behavior.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
///
/// # Safety
///
/// Uses Objective-C message sending internally on macOS.
#[cfg(target_os = "macos")]
pub fn ensure_move_to_active_space() {
    unsafe {
        // Use WindowManager to get the main window (not keyWindow, which may not exist yet)
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log(
                    "PANEL",
                    "WARNING: Main window not registered, cannot set MoveToActiveSpace",
                );
                return;
            }
        };

        // NSWindowCollectionBehaviorMoveToActiveSpace = (1 << 1) = 2
        // This makes the window MOVE to the current active space when shown
        let collection_behavior: u64 = 2;
        let _: () = msg_send![window, setCollectionBehavior:collection_behavior];

        logging::log(
            "PANEL",
            "Set MoveToActiveSpace collection behavior (before activation)",
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_move_to_active_space() {
    // No-op on non-macOS platforms
}

// ============================================================================
// Floating Panel Configuration
// ============================================================================

/// Configure the current key window as a floating macOS panel.
///
/// This function configures the key window (most recently activated window) with:
/// - Floating window level (NSFloatingWindowLevel = 3) - appears above normal windows
/// - MoveToActiveSpace collection behavior - moves to current space when shown
/// - Disabled window restoration - prevents macOS from remembering window position
/// - Empty frame autosave name - prevents position caching
///
/// # macOS Behavior
///
/// Uses NSApp to get the keyWindow and applies all configurations. If no key window
/// is found (e.g., during app startup), logs a warning and returns.
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
///
/// # Safety
///
/// Uses Objective-C message sending internally on macOS.
///
/// # Example
///
/// ```ignore
/// // Call after window is created and visible
/// configure_as_floating_panel();
/// ```
#[cfg(target_os = "macos")]
pub fn configure_as_floating_panel() {
    unsafe {
        let app: id = NSApp();

        // Get the key window (the most recently activated window)
        let window: id = msg_send![app, keyWindow];

        if window != nil {
            // NSFloatingWindowLevel = 3
            // This makes the window float above normal windows
            let floating_level: i32 = 3;
            let _: () = msg_send![window, setLevel:floating_level];

            // NSWindowCollectionBehaviorMoveToActiveSpace = (1 << 1)
            // This makes the window MOVE to the current active space when shown
            // (instead of forcing user back to the space where window was last visible)
            let collection_behavior: u64 = 2;
            let _: () = msg_send![window, setCollectionBehavior:collection_behavior];

            // CRITICAL: Disable macOS window state restoration
            // This prevents macOS from remembering and restoring the window position
            // when the app is relaunched or the window is shown again
            let _: () = msg_send![window, setRestorable:false];

            // Also disable the window's autosave frame name which can cause position caching
            let empty_string: id = msg_send![class!(NSString), string];
            let _: () = msg_send![window, setFrameAutosaveName:empty_string];

            logging::log(
                "PANEL",
                "Configured window as floating panel (level=3, MoveToActiveSpace, restorable=false, no autosave)",
            );
        } else {
            logging::log(
                "PANEL",
                "Warning: No key window found to configure as panel",
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_as_floating_panel() {
    // No-op on non-macOS platforms
}

// ============================================================================
// Constants
// ============================================================================

/// NSFloatingWindowLevel constant value (3)
/// Windows at this level float above normal windows but below modal dialogs.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const NS_FLOATING_WINDOW_LEVEL: i32 = 3;

/// NSWindowCollectionBehaviorMoveToActiveSpace constant value (1 << 1 = 2)
/// When set, the window moves to the currently active space when shown.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE: u64 = 2;

// ============================================================================
// Mouse Position
// ============================================================================

#[cfg(target_os = "macos")]
use core_graphics::event::CGEvent;
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Get the current global mouse cursor position using macOS Core Graphics API.
/// Returns the position in screen coordinates.
#[cfg(target_os = "macos")]
pub fn get_global_mouse_position() -> Option<(f64, f64)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let location = event.location();
    Some((location.x, location.y))
}

#[cfg(not(target_os = "macos"))]
pub fn get_global_mouse_position() -> Option<(f64, f64)> {
    // TODO: Implement for other platforms
    None
}

// ============================================================================
// Display Information
// ============================================================================

/// Represents a display's bounds in macOS global coordinate space
#[derive(Debug, Clone)]
pub struct DisplayBounds {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
use cocoa::foundation::NSRect;

/// Get all displays with their actual bounds in macOS global coordinates.
/// This uses NSScreen directly because GPUI's display.bounds() doesn't return
/// correct origins for secondary displays.
#[cfg(target_os = "macos")]
pub fn get_macos_displays() -> Vec<DisplayBounds> {
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];

        // Get primary screen height for coordinate flipping
        // macOS coordinates: Y=0 at bottom of primary screen
        let main_screen: id = msg_send![screens, firstObject];
        let main_frame: NSRect = msg_send![main_screen, frame];
        let primary_height = main_frame.size.height;

        let mut displays = Vec::with_capacity(count);

        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex:i];
            let frame: NSRect = msg_send![screen, frame];

            // Convert from macOS bottom-left origin to top-left origin
            // macOS: y=0 at bottom, increasing upward
            // We want: y=0 at top, increasing downward
            let flipped_y = primary_height - frame.origin.y - frame.size.height;

            displays.push(DisplayBounds {
                origin_x: frame.origin.x,
                origin_y: flipped_y,
                width: frame.size.width,
                height: frame.size.height,
            });
        }

        displays
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_macos_displays() -> Vec<DisplayBounds> {
    // Fallback: return a single default display
    vec![DisplayBounds {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1920.0,
        height: 1080.0,
    }]
}

// ============================================================================
// Window Movement
// ============================================================================

#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSSize};

/// Move the application's main window to new bounds using WindowManager.
/// This uses the registered main window instead of objectAtIndex:0, which
/// avoids issues with tray icons and other system windows in the array.
///
/// IMPORTANT: macOS uses a global coordinate space where Y=0 is at the BOTTOM of the
/// PRIMARY screen, and Y increases upward. The primary screen's origin is always (0,0)
/// at its bottom-left corner. Secondary displays have their own position in this space.
#[cfg(target_os = "macos")]
pub fn move_first_window_to(x: f64, y: f64, width: f64, height: f64) {
    unsafe {
        // Use WindowManager to get the main window reliably
        let window = match window_manager::get_main_window() {
            Some(w) => w,
            None => {
                logging::log(
                    "POSITION",
                    "WARNING: Main window not registered in WindowManager, cannot move",
                );
                return;
            }
        };

        // Get the PRIMARY screen's height for coordinate conversion
        let screens: id = msg_send![class!(NSScreen), screens];
        let main_screen: id = msg_send![screens, firstObject];
        let main_screen_frame: NSRect = msg_send![main_screen, frame];
        let primary_screen_height = main_screen_frame.size.height;

        // Log current window position before move
        let current_frame: NSRect = msg_send![window, frame];
        logging::log(
            "POSITION",
            &format!(
                "Current window frame: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                current_frame.origin.x,
                current_frame.origin.y,
                current_frame.size.width,
                current_frame.size.height
            ),
        );

        // Convert from top-left origin (y down) to bottom-left origin (y up)
        let flipped_y = primary_screen_height - y - height;

        logging::log(
            "POSITION",
            &format!(
                "Moving window: target=({:.0}, {:.0}) flipped_y={:.0}",
                x, y, flipped_y
            ),
        );

        let new_frame = NSRect::new(NSPoint::new(x, flipped_y), NSSize::new(width, height));

        // Move the window
        let _: () = msg_send![window, setFrame:new_frame display:true animate:false];

        // NOTE: We no longer call makeKeyAndOrderFront here.
        // Window ordering/activation is handled by GPUI's cx.activate() and win.activate_window()
        // which is called AFTER ensure_move_to_active_space() sets the collection behavior.

        // Verify the move worked
        let after_frame: NSRect = msg_send![window, frame];
        logging::log(
            "POSITION",
            &format!(
                "Window moved: actual=({:.0}, {:.0}) size={:.0}x{:.0}",
                after_frame.origin.x,
                after_frame.origin.y,
                after_frame.size.width,
                after_frame.size.height
            ),
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn move_first_window_to(_x: f64, _y: f64, _width: f64, _height: f64) {
    // TODO: Implement for other platforms
    logging::log(
        "POSITION",
        "move_first_window_to is not implemented for this platform",
    );
}

use gpui::{point, px, Bounds, Pixels};

/// Move the first window to new bounds (wrapper for Bounds<Pixels>)
pub fn move_first_window_to_bounds(bounds: &Bounds<Pixels>) {
    let x: f64 = bounds.origin.x.into();
    let y: f64 = bounds.origin.y.into();
    let width: f64 = bounds.size.width.into();
    let height: f64 = bounds.size.height.into();
    move_first_window_to(x, y, width, height);
}

// ============================================================================
// Window Positioning (Eye-line)
// ============================================================================

/// Calculate window bounds positioned at eye-line height on the display containing the mouse cursor.
///
/// - Finds the display where the mouse cursor is located
/// - Centers the window horizontally on that display
/// - Positions the window at "eye-line" height (upper 14% of the screen)
///
/// This matches the behavior of Raycast/Alfred where the prompt appears on the active display.
pub fn calculate_eye_line_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,
) -> Bounds<Pixels> {
    // Use native macOS API to get actual display bounds with correct origins
    // GPUI's cx.displays() returns incorrect origins for secondary displays
    let displays = get_macos_displays();

    logging::log("POSITION", "");
    logging::log(
        "POSITION",
        "╔════════════════════════════════════════════════════════════╗",
    );
    logging::log(
        "POSITION",
        "║  CALCULATING WINDOW POSITION FOR MOUSE DISPLAY             ║",
    );
    logging::log(
        "POSITION",
        "╚════════════════════════════════════════════════════════════╝",
    );
    logging::log(
        "POSITION",
        &format!("Available displays: {}", displays.len()),
    );

    // Log all available displays for debugging
    for (idx, display) in displays.iter().enumerate() {
        let right = display.origin_x + display.width;
        let bottom = display.origin_y + display.height;
        logging::log("POSITION", &format!(
            "  Display {}: origin=({:.0}, {:.0}) size={:.0}x{:.0} [bounds: x={:.0}..{:.0}, y={:.0}..{:.0}]",
            idx, display.origin_x, display.origin_y, display.width, display.height,
            display.origin_x, right, display.origin_y, bottom
        ));
    }

    // Try to get mouse position and find which display contains it
    let target_display = if let Some((mouse_x, mouse_y)) = get_global_mouse_position() {
        logging::log(
            "POSITION",
            &format!("Mouse cursor at ({:.0}, {:.0})", mouse_x, mouse_y),
        );

        // Find the display that contains the mouse cursor
        let found = displays.iter().enumerate().find(|(idx, display)| {
            let contains = mouse_x >= display.origin_x
                && mouse_x < display.origin_x + display.width
                && mouse_y >= display.origin_y
                && mouse_y < display.origin_y + display.height;

            if contains {
                logging::log("POSITION", &format!("  -> Mouse is on display {}", idx));
            }
            contains
        });

        found.map(|(_, d)| d.clone())
    } else {
        logging::log(
            "POSITION",
            "Could not get mouse position, using primary display",
        );
        None
    };

    // Use the found display, or fall back to first display (primary)
    let display = target_display.or_else(|| {
        logging::log(
            "POSITION",
            "No display contains mouse, falling back to primary",
        );
        displays.first().cloned()
    });

    if let Some(display) = display {
        logging::log(
            "POSITION",
            &format!(
                "Selected display: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                display.origin_x, display.origin_y, display.width, display.height
            ),
        );

        // Eye-line: position window top at ~14% from screen top (input bar at eye level)
        let eye_line_y = display.origin_y + display.height * 0.14;

        // Center horizontally on the display
        let window_width: f64 = window_size.width.into();
        let center_x = display.origin_x + (display.width - window_width) / 2.0;

        let final_bounds = Bounds {
            origin: point(px(center_x as f32), px(eye_line_y as f32)),
            size: window_size,
        };

        logging::log(
            "POSITION",
            &format!(
                "Final window bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                center_x,
                eye_line_y,
                f64::from(window_size.width),
                f64::from(window_size.height)
            ),
        );

        final_bounds
    } else {
        logging::log(
            "POSITION",
            "No displays found, using default centered bounds",
        );
        // Fallback: just center on screen using 1512x982 as default (common MacBook)
        Bounds {
            origin: point(px(381.0), px(246.0)),
            size: window_size,
        }
    }
}

// ============================================================================
// Screenshot Capture
// ============================================================================

/// Capture a screenshot of the app window using xcap for cross-platform support.
///
/// Returns a tuple of (png_data, width, height) on success.
/// The function:
/// 1. Uses xcap::Window::all() to enumerate windows
/// 2. Finds the Script Kit window by app name or title
/// 3. Captures the window directly to an image buffer
/// 4. Optionally scales down to 1x resolution if hi_dpi is false
/// 5. Encodes to PNG in memory (no temp files)
///
/// # Arguments
/// * `hi_dpi` - If true, return full retina resolution (2x). If false, scale down to 1x.
pub fn capture_app_screenshot(
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use xcap::Window;

    let windows = Window::all()?;

    for window in windows {
        let title = window.title().unwrap_or_else(|_| String::new());
        let app_name = window.app_name().unwrap_or_else(|_| String::new());

        // Match our app window by name
        let is_our_window = app_name.contains("script-kit-gpui")
            || app_name == "Script Kit"
            || title.contains("Script Kit");

        let is_minimized = window.is_minimized().unwrap_or(true);

        if is_our_window && !is_minimized {
            tracing::debug!(
                app_name = %app_name,
                title = %title,
                hi_dpi = hi_dpi,
                "Found Script Kit window for screenshot"
            );

            let image = window.capture_image()?;
            let original_width = image.width();
            let original_height = image.height();

            // Scale down to 1x if not hi_dpi mode (xcap captures at retina resolution on macOS)
            let (final_image, width, height) = if hi_dpi {
                (image, original_width, original_height)
            } else {
                // Scale down by 2x for 1x resolution
                let new_width = original_width / 2;
                let new_height = original_height / 2;
                let resized = image::imageops::resize(
                    &image,
                    new_width,
                    new_height,
                    image::imageops::FilterType::Lanczos3,
                );
                tracing::debug!(
                    original_width = original_width,
                    original_height = original_height,
                    new_width = new_width,
                    new_height = new_height,
                    "Scaled screenshot to 1x resolution"
                );
                (resized, new_width, new_height)
            };

            // Encode to PNG in memory (no temp files needed)
            let mut png_data = Vec::new();
            let encoder = PngEncoder::new(&mut png_data);
            encoder.write_image(&final_image, width, height, image::ExtendedColorType::Rgba8)?;

            tracing::debug!(
                width = width,
                height = height,
                hi_dpi = hi_dpi,
                file_size = png_data.len(),
                "Screenshot captured with xcap"
            );

            return Ok((png_data, width, height));
        }
    }

    Err("Script Kit window not found".into())
}

// ============================================================================
// Open Path with System Default
// ============================================================================

/// Open a path (file or folder) with the system default application.
/// On macOS: uses `open` command
/// On Linux: uses `xdg-open` command
/// On Windows: uses `cmd /C start` command
///
/// This can be used to open files, folders, URLs, or any path that the
/// system knows how to handle.
#[allow(dead_code)]
pub fn open_path_with_system_default(path: &str) {
    logging::log("UI", &format!("Opening path with system default: {}", path));
    let path_owned = path.to_string();

    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            match std::process::Command::new("open").arg(&path_owned).spawn() {
                Ok(_) => logging::log("UI", &format!("Successfully opened: {}", path_owned)),
                Err(e) => logging::log("ERROR", &format!("Failed to open '{}': {}", path_owned, e)),
            }
        }

        #[cfg(target_os = "linux")]
        {
            match std::process::Command::new("xdg-open")
                .arg(&path_owned)
                .spawn()
            {
                Ok(_) => logging::log("UI", &format!("Successfully opened: {}", path_owned)),
                Err(e) => logging::log("ERROR", &format!("Failed to open '{}': {}", path_owned, e)),
            }
        }

        #[cfg(target_os = "windows")]
        {
            match std::process::Command::new("cmd")
                .args(["/C", "start", "", &path_owned])
                .spawn()
            {
                Ok(_) => logging::log("UI", &format!("Successfully opened: {}", path_owned)),
                Err(e) => logging::log("ERROR", &format!("Failed to open '{}': {}", path_owned, e)),
            }
        }
    });
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that ensure_move_to_active_space can be called without panicking.
    /// This is a characterization test - it verifies the function doesn't crash.
    /// On non-macOS, this is a no-op. On macOS without a window, it logs a warning.
    #[test]
    fn test_ensure_move_to_active_space_does_not_panic() {
        // Should not panic even without a window registered
        ensure_move_to_active_space();
    }

    /// Test that configure_as_floating_panel can be called without panicking.
    /// This is a characterization test - it verifies the function doesn't crash.
    /// On non-macOS, this is a no-op. On macOS without NSApp/keyWindow, it handles gracefully.
    #[test]
    fn test_configure_as_floating_panel_does_not_panic() {
        // Should not panic even without an app running
        configure_as_floating_panel();
    }

    /// Verify the macOS constants have the correct values.
    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_constants() {
        assert_eq!(NS_FLOATING_WINDOW_LEVEL, 3);
        assert_eq!(NS_WINDOW_COLLECTION_BEHAVIOR_MOVE_TO_ACTIVE_SPACE, 2);
    }

    /// Test that both functions can be called in sequence.
    /// This mirrors the typical usage pattern in main.rs where both are called
    /// during window setup.
    #[test]
    fn test_functions_can_be_called_in_sequence() {
        // This is the typical call order in main.rs
        ensure_move_to_active_space();
        configure_as_floating_panel();
        // Should complete without panicking
    }

    /// Test that functions are idempotent - can be called multiple times safely.
    #[test]
    fn test_functions_are_idempotent() {
        for _ in 0..3 {
            ensure_move_to_active_space();
            configure_as_floating_panel();
        }
        // Should complete without panicking or causing issues
    }

    // =========================================================================
    // Mouse Position Tests
    // =========================================================================

    /// Test get_global_mouse_position returns valid coordinates or None.
    /// On macOS with display, returns Some((x, y)).
    /// On other platforms or without display, returns None.
    #[test]
    fn test_get_global_mouse_position_does_not_panic() {
        // Should not panic regardless of whether we can get the position
        let _ = get_global_mouse_position();
    }

    // =========================================================================
    // Display Information Tests
    // =========================================================================

    /// Test DisplayBounds struct creation and field access.
    #[test]
    fn test_display_bounds_struct() {
        let bounds = DisplayBounds {
            origin_x: 100.0,
            origin_y: 200.0,
            width: 1920.0,
            height: 1080.0,
        };

        assert_eq!(bounds.origin_x, 100.0);
        assert_eq!(bounds.origin_y, 200.0);
        assert_eq!(bounds.width, 1920.0);
        assert_eq!(bounds.height, 1080.0);
    }

    /// Test DisplayBounds Clone implementation.
    #[test]
    fn test_display_bounds_clone() {
        let original = DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };

        let cloned = original.clone();
        assert_eq!(cloned.width, 2560.0);
        assert_eq!(cloned.height, 1440.0);
    }

    /// Test get_macos_displays returns at least one display (or fallback).
    #[test]
    fn test_get_macos_displays_returns_at_least_one() {
        let displays = get_macos_displays();
        assert!(!displays.is_empty(), "Should return at least one display");
    }

    /// Test get_macos_displays returns displays with valid dimensions.
    #[test]
    fn test_get_macos_displays_valid_dimensions() {
        let displays = get_macos_displays();
        for display in displays {
            assert!(display.width > 0.0, "Display width must be positive");
            assert!(display.height > 0.0, "Display height must be positive");
        }
    }

    // =========================================================================
    // Window Movement Tests
    // =========================================================================

    /// Test move_first_window_to does not panic without a window.
    #[test]
    fn test_move_first_window_to_does_not_panic() {
        // Should not panic even without a registered window
        move_first_window_to(100.0, 100.0, 800.0, 600.0);
    }

    /// Test move_first_window_to_bounds wrapper function.
    #[test]
    fn test_move_first_window_to_bounds_does_not_panic() {
        use gpui::size;
        let bounds = Bounds {
            origin: point(px(100.0), px(100.0)),
            size: size(px(800.0), px(600.0)),
        };
        // Should not panic
        move_first_window_to_bounds(&bounds);
    }

    // =========================================================================
    // Eye-line Positioning Tests
    // =========================================================================

    /// Test calculate_eye_line_bounds returns valid bounds.
    #[test]
    fn test_calculate_eye_line_bounds_returns_valid() {
        use gpui::size;
        let window_size = size(px(750.0), px(500.0));
        let bounds = calculate_eye_line_bounds_on_mouse_display(window_size);

        // Bounds should have the same size as input
        assert_eq!(bounds.size.width, window_size.width);
        assert_eq!(bounds.size.height, window_size.height);
    }

    /// Test eye-line calculation positions window in upper portion of screen.
    #[test]
    fn test_calculate_eye_line_bounds_upper_portion() {
        use gpui::size;
        let window_size = size(px(750.0), px(500.0));
        let bounds = calculate_eye_line_bounds_on_mouse_display(window_size);

        // Get the display bounds for comparison
        let displays = get_macos_displays();
        if let Some(display) = displays.first() {
            let origin_y: f64 = bounds.origin.y.into();
            let display_height = display.height;

            // Eye-line should be in upper half of screen (top 50%)
            assert!(
                origin_y < display.origin_y + display_height * 0.5,
                "Window should be in upper half: origin_y={}, display_mid={}",
                origin_y,
                display.origin_y + display_height * 0.5
            );
        }
    }
}

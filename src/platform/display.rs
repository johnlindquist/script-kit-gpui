use crate::logging;
use crate::windows::DisplayBounds;
use gpui::{point, px, Bounds, Pixels, Size};

#[cfg(target_os = "macos")]
use super::require_main_thread;

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::NSRect;
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

/// Display bounds with both full screen frame and placement-safe visible area.
///
/// `frame` contains the full display rectangle.
/// `visible_area` uses `NSScreen.visibleFrame`, excluding menu bar and dock.
#[derive(Debug, Clone)]
pub struct VisibleDisplayBounds {
    pub frame: DisplayBounds,
    pub visible_area: DisplayBounds,
}

/// Convert Y coordinate from top-left origin (y increases down) to
/// AppKit bottom-left origin (y increases up).
/// Same formula both directions (mirror transform).
#[allow(dead_code)]
pub fn flip_y(primary_height: f64, y: f64, height: f64) -> f64 {
    primary_height - y - height
}

#[cfg(target_os = "macos")]
const PRIMARY_SCREEN_ARRAY_INDEX: usize = 0;

#[cfg(target_os = "macos")]
fn primary_screen_array_index(screen_count: usize) -> Option<usize> {
    // Cocoa global coordinates are anchored to the primary/menu-bar screen, which is screens[0].
    // The key-window screen API tracks focus instead and cannot be used for coordinate conversion.
    (screen_count > 0).then_some(PRIMARY_SCREEN_ARRAY_INDEX)
}

/// Get the current global mouse cursor position using macOS Core Graphics API.
/// Returns the position in global display coordinates (top-left origin, Y increases down).
///
/// # Implementation Note
/// We use `CGEventCreate(NULL)` directly via FFI because the Rust core-graphics crate's
/// `CGEvent::new(source)` creates a null-type event with undefined location. According to
/// Apple's documentation, when `CGEventCreate` is passed NULL and then `CGEventGetLocation`
/// is called, it returns the CURRENT mouse position. This is the canonical way to get
/// mouse position in Core Graphics.
#[cfg(target_os = "macos")]
pub fn get_global_mouse_position() -> Option<(f64, f64)> {
    if !super::native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) {
        return None;
    }
    use core_foundation::base::CFRelease;
    use core_graphics::geometry::CGPoint;
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *const c_void) -> *const c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    }

    // SAFETY: CGEventCreate(NULL) is the documented way to get the current
    // mouse position. The returned event pointer is nil-checked before use.
    // CFRelease is called to free the event, preventing a memory leak.
    // CGEventGetLocation returns a value type (CGPoint), no pointer issues.
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            logging::log("POSITION", "WARNING: CGEventCreate returned null");
            return None;
        }

        let location = CGEventGetLocation(event);

        CFRelease(event);

        Some((location.x, location.y))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_global_mouse_position() -> Option<(f64, f64)> {
    None
}

/// Get the height of the primary screen (screens[0]) for coordinate conversion.
/// macOS uses bottom-left origin; we convert to top-left origin.
#[cfg(target_os = "macos")]
pub fn primary_screen_height() -> Option<f64> {
    if !super::native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) {
        return None;
    }
    if require_main_thread("primary_screen_height") {
        return None;
    }

    // SAFETY: Main thread verified. NSScreen.screens is a class method.
    // We nil-check the NSArray, require at least one screen, and nil-check
    // the primary screen (screens[0]) before accessing its frame.
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return None;
        }

        let count: usize = msg_send![screens, count];
        let primary_screen_index = primary_screen_array_index(count)?;
        let primary_screen: id = msg_send![screens, objectAtIndex: primary_screen_index];
        if primary_screen == nil {
            return None;
        }

        let frame: NSRect = msg_send![primary_screen, frame];
        Some(frame.size.height)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn primary_screen_height() -> Option<f64> {
    Some(1080.0)
}

#[cfg(target_os = "macos")]
fn nsrect_to_display_bounds(rect: NSRect, primary_height: f64) -> DisplayBounds {
    DisplayBounds {
        origin_x: rect.origin.x,
        origin_y: flip_y(primary_height, rect.origin.y, rect.size.height),
        width: rect.size.width,
        height: rect.size.height,
    }
}

/// Get all displays with both full frame and visible placement frame.
#[cfg(target_os = "macos")]
pub fn get_macos_visible_displays() -> Vec<VisibleDisplayBounds> {
    if !super::native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) {
        return Vec::new();
    }
    if require_main_thread("get_macos_visible_displays") {
        return Vec::new();
    }

    // SAFETY: Main thread verified. NSScreen.screens is a class method.
    // We nil-check the NSArray, ensure the primary screen (screens[0]) exists
    // before reading its frame, and bound all objectAtIndex: access by count.
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return Vec::new();
        }

        let count: usize = msg_send![screens, count];
        let primary_screen_index = match primary_screen_array_index(count) {
            Some(index) => index,
            None => return Vec::new(),
        };

        let primary_screen: id = msg_send![screens, objectAtIndex: primary_screen_index];
        if primary_screen == nil {
            logging::log(
                "POSITION",
                "WARNING: primary screen (screens[0]) returned nil",
            );
            return Vec::new();
        }

        let primary_frame: NSRect = msg_send![primary_screen, frame];
        let primary_height = primary_frame.size.height;

        let mut displays = Vec::with_capacity(count);
        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex:i];
            let frame: NSRect = msg_send![screen, frame];
            let visible_frame: NSRect = msg_send![screen, visibleFrame];

            displays.push(VisibleDisplayBounds {
                frame: nsrect_to_display_bounds(frame, primary_height),
                visible_area: nsrect_to_display_bounds(visible_frame, primary_height),
            });
        }

        displays
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_macos_visible_displays() -> Vec<VisibleDisplayBounds> {
    let fallback = DisplayBounds {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    vec![VisibleDisplayBounds {
        frame: fallback.clone(),
        visible_area: fallback,
    }]
}

/// Return the display containing the currently focused window (key window).
///
/// Uses `NSScreen.mainScreen`, which returns the screen that contains the
/// window receiving keyboard events. This is the correct heuristic for
/// placing UI relative to the user's active workspace, as opposed to
/// `get_global_mouse_position()` which follows the cursor.
///
/// Falls back to the first display (primary) when `mainScreen` is nil.
#[cfg(target_os = "macos")]
pub fn get_active_display() -> Option<VisibleDisplayBounds> {
    if !super::native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) {
        return None;
    }
    if require_main_thread("get_active_display") {
        return None;
    }

    // SAFETY: Main thread verified. NSScreen.mainScreen returns the screen
    // containing the key window, or nil if there is no key window.
    // We nil-check the result and fall back to screens[0] (primary).
    // primary_screen_height() is called first to anchor coordinate conversion.
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        if screens.is_null() {
            return None;
        }

        let count: usize = msg_send![screens, count];
        let primary_screen_index = primary_screen_array_index(count)?;

        let primary_screen: id = msg_send![screens, objectAtIndex: primary_screen_index];
        if primary_screen == nil {
            return None;
        }
        let primary_frame: NSRect = msg_send![primary_screen, frame];
        let primary_height = primary_frame.size.height;

        // Try mainScreen first (screen with key window).
        let main_screen: id = msg_send![class!(NSScreen), mainScreen];
        let target_screen = if main_screen == nil {
            primary_screen
        } else {
            main_screen
        };

        let frame: NSRect = msg_send![target_screen, frame];
        let visible_frame: NSRect = msg_send![target_screen, visibleFrame];

        Some(VisibleDisplayBounds {
            frame: nsrect_to_display_bounds(frame, primary_height),
            visible_area: nsrect_to_display_bounds(visible_frame, primary_height),
        })
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_active_display() -> Option<VisibleDisplayBounds> {
    let fallback = DisplayBounds {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    Some(VisibleDisplayBounds {
        frame: fallback.clone(),
        visible_area: fallback,
    })
}

/// Get all displays with their full frame in top-left global coordinates.
#[cfg(target_os = "macos")]
pub fn get_macos_displays() -> Vec<DisplayBounds> {
    get_macos_visible_displays()
        .into_iter()
        .map(|display| display.frame)
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn get_macos_displays() -> Vec<DisplayBounds> {
    vec![DisplayBounds {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1920.0,
        height: 1080.0,
    }]
}

/// Return the display that contains the given point.
pub fn display_for_point(
    mouse_pt: (f64, f64),
    displays: &[VisibleDisplayBounds],
) -> Option<VisibleDisplayBounds> {
    let (mouse_x, mouse_y) = mouse_pt;

    displays
        .iter()
        .find(|display| {
            mouse_x >= display.frame.origin_x
                && mouse_x < display.frame.origin_x + display.frame.width
                && mouse_y >= display.frame.origin_y
                && mouse_y < display.frame.origin_y + display.frame.height
        })
        .cloned()
}

/// Clamp bounds so the window stays fully inside the display visible area.
pub fn clamp_to_visible(bounds: Bounds<Pixels>, visible_area: &DisplayBounds) -> Bounds<Pixels> {
    if visible_area.width <= 0.0 || visible_area.height <= 0.0 {
        return bounds;
    }

    let x: f64 = bounds.origin.x.into();
    let y: f64 = bounds.origin.y.into();
    let width: f64 = bounds.size.width.into();
    let height: f64 = bounds.size.height.into();

    let clamped_width = width.min(visible_area.width);
    let clamped_height = height.min(visible_area.height);

    let min_x = visible_area.origin_x;
    let max_x = visible_area.origin_x + visible_area.width - clamped_width;
    let min_y = visible_area.origin_y;
    let max_y = visible_area.origin_y + visible_area.height - clamped_height;

    let clamped_x = x.max(min_x).min(max_x);
    let clamped_y = y.max(min_y).min(max_y);

    Bounds::new(
        point(px(clamped_x as f32), px(clamped_y as f32)),
        Size::new(px(clamped_width as f32), px(clamped_height as f32)),
    )
}

/// Query macOS `accessibilityDisplayShouldReduceMotion` preference.
///
/// Returns `true` when the user has enabled "Reduce motion" in
/// System Settings → Accessibility → Display, signalling that
/// animations should be disabled or simplified.
#[cfg(target_os = "macos")]
pub fn prefers_reduced_motion() -> bool {
    if !super::native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) {
        return false;
    }
    // SAFETY: NSWorkspace.sharedWorkspace is a singleton accessor safe to call
    // from the main thread. accessibilityDisplayShouldReduceMotion is a read-only
    // property that queries the system accessibility preference.
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let result: cocoa::base::BOOL =
            msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        result != cocoa::base::NO
    }
}

#[cfg(not(target_os = "macos"))]
pub fn prefers_reduced_motion() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn make_display(
        frame_origin_x: f64,
        frame_origin_y: f64,
        width: f64,
        height: f64,
        visible_origin_x: f64,
        visible_origin_y: f64,
        visible_width: f64,
        visible_height: f64,
    ) -> VisibleDisplayBounds {
        VisibleDisplayBounds {
            frame: DisplayBounds {
                origin_x: frame_origin_x,
                origin_y: frame_origin_y,
                width,
                height,
            },
            visible_area: DisplayBounds {
                origin_x: visible_origin_x,
                origin_y: visible_origin_y,
                width: visible_width,
                height: visible_height,
            },
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_primary_screen_array_index_does_select_screens_zero_when_available() {
        assert_eq!(primary_screen_array_index(0), None);
        assert_eq!(
            primary_screen_array_index(1),
            Some(PRIMARY_SCREEN_ARRAY_INDEX)
        );
        assert_eq!(
            primary_screen_array_index(3),
            Some(PRIMARY_SCREEN_ARRAY_INDEX)
        );
    }

    #[test]
    fn test_display_for_point_does_select_display_when_point_is_inside_frame() {
        let displays = vec![
            make_display(0.0, 0.0, 1440.0, 900.0, 0.0, 24.0, 1440.0, 876.0),
            make_display(1440.0, 0.0, 1920.0, 1080.0, 1440.0, 0.0, 1920.0, 1040.0),
        ];

        let selected = display_for_point((1500.0, 100.0), &displays)
            .expect("point should resolve to the second display");

        assert_eq!(selected.frame.origin_x, 1440.0);
        assert_eq!(selected.frame.width, 1920.0);
    }

    #[test]
    fn test_display_for_point_does_return_none_when_point_outside_all_displays() {
        let displays = vec![make_display(
            0.0, 0.0, 1440.0, 900.0, 0.0, 24.0, 1440.0, 876.0,
        )];

        let selected = display_for_point((2000.0, 1000.0), &displays);

        assert!(selected.is_none());
    }

    #[test]
    fn test_clamp_to_visible_does_clamp_origin_when_bounds_exceed_visible_area() {
        let visible_area = DisplayBounds {
            origin_x: 100.0,
            origin_y: 50.0,
            width: 500.0,
            height: 300.0,
        };

        let original_bounds =
            Bounds::new(point(px(40.0), px(20.0)), Size::new(px(400.0), px(250.0)));

        let clamped = clamp_to_visible(original_bounds, &visible_area);

        let clamped_x: f64 = clamped.origin.x.into();
        let clamped_y: f64 = clamped.origin.y.into();
        assert_eq!(clamped_x, 100.0);
        assert_eq!(clamped_y, 50.0);
    }

    #[test]
    fn test_clamp_to_visible_does_shrink_window_when_window_larger_than_visible_area() {
        let visible_area = DisplayBounds {
            origin_x: 0.0,
            origin_y: 0.0,
            width: 300.0,
            height: 200.0,
        };

        let original_bounds =
            Bounds::new(point(px(-50.0), px(-30.0)), Size::new(px(500.0), px(280.0)));

        let clamped = clamp_to_visible(original_bounds, &visible_area);

        let clamped_x: f64 = clamped.origin.x.into();
        let clamped_y: f64 = clamped.origin.y.into();
        let clamped_width: f64 = clamped.size.width.into();
        let clamped_height: f64 = clamped.size.height.into();

        assert_eq!(clamped_x, 0.0);
        assert_eq!(clamped_y, 0.0);
        assert_eq!(clamped_width, 300.0);
        assert_eq!(clamped_height, 200.0);
    }
}

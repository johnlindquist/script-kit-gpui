// ============================================================================
// Window Movement
// ============================================================================

#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSSize};

pub fn main_window_geometry_trace_enabled() -> bool {
    std::env::var("SCRIPT_KIT_MAIN_WINDOW_GEOM_TRACE")
        .map(|value| {
            let value = value.trim();
            !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn rect_summary(rect: NSRect) -> String {
    format!(
        "{:.1},{:.1},{:.1}x{:.1}",
        rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
    )
}

#[cfg(target_os = "macos")]
fn top_left_rect_summary(rect: NSRect, primary_height: f64) -> String {
    format!(
        "{:.1},{:.1},{:.1}x{:.1}",
        rect.origin.x,
        primary_height - rect.origin.y - rect.size.height,
        rect.size.width,
        rect.size.height
    )
}

#[cfg(target_os = "macos")]
unsafe fn ns_screen_number(screen: id) -> Option<u32> {
    if screen == nil {
        return None;
    }
    let device_description: id = msg_send![screen, deviceDescription];
    if device_description == nil {
        return None;
    }
    let key = CocoaNSString::alloc(nil).init_str("NSScreenNumber");
    let number: id = msg_send![device_description, objectForKey:key];
    if number == nil {
        None
    } else {
        Some(msg_send![number, unsignedIntValue])
    }
}

#[cfg(target_os = "macos")]
unsafe fn screen_summary(screen: id, index: usize, primary_height: f64) -> String {
    if screen == nil {
        return format!("{index}:nil");
    }
    let frame: NSRect = msg_send![screen, frame];
    let visible_frame: NSRect = msg_send![screen, visibleFrame];
    let backing_scale: f64 = msg_send![screen, backingScaleFactor];
    let screen_id = ns_screen_number(screen)
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());
    format!(
        "{index}:id={screen_id}:scale={backing_scale:.2}:frame_bl={}:frame_tl={}:visible_bl={}:visible_tl={}",
        rect_summary(frame),
        top_left_rect_summary(frame, primary_height),
        rect_summary(visible_frame),
        top_left_rect_summary(visible_frame, primary_height),
    )
}

#[cfg(target_os = "macos")]
unsafe fn all_screen_summaries(primary_height: f64) -> String {
    let screens: id = msg_send![class!(NSScreen), screens];
    if screens == nil {
        return "screens=nil".to_string();
    }
    let count: usize = msg_send![screens, count];
    let mut summaries = Vec::with_capacity(count);
    for index in 0..count {
        let screen: id = msg_send![screens, objectAtIndex:index];
        summaries.push(screen_summary(screen, index, primary_height));
    }
    summaries.join("|")
}

#[cfg(target_os = "macos")]
unsafe fn layer_summary(view: id) -> (String, String) {
    if view == nil {
        return ("content_view=nil".to_string(), "layer=nil".to_string());
    }

    let view_frame: NSRect = msg_send![view, frame];
    let view_bounds: NSRect = msg_send![view, bounds];
    let wants_layer: cocoa::base::BOOL = msg_send![view, wantsLayer];
    let content_summary = format!(
        "frame={}:bounds={}:wants_layer={}",
        rect_summary(view_frame),
        rect_summary(view_bounds),
        wants_layer != cocoa::base::NO
    );

    let layer: id = msg_send![view, layer];
    if layer == nil {
        return (content_summary, "layer=nil".to_string());
    }

    let layer_bounds: NSRect = msg_send![layer, bounds];
    let layer_frame: NSRect = msg_send![layer, frame];
    let contents_scale: f64 = msg_send![layer, contentsScale];
    (
        content_summary,
        format!(
            "frame={}:bounds={}:contents_scale={contents_scale:.2}",
            rect_summary(layer_frame),
            rect_summary(layer_bounds),
        ),
    )
}

#[cfg(target_os = "macos")]
unsafe fn sync_layer_tree_contents_scale(layer: id, backing_scale: f64) {
    if layer == nil {
        return;
    }

    let _: () = msg_send![layer, setContentsScale: backing_scale];
    let _: () = msg_send![layer, setNeedsDisplay];

    let sublayers: id = msg_send![layer, sublayers];
    if sublayers == nil {
        return;
    }

    let count: usize = msg_send![sublayers, count];
    for index in 0..count {
        let sublayer: id = msg_send![sublayers, objectAtIndex:index];
        sync_layer_tree_contents_scale(sublayer, backing_scale);
    }
}

#[cfg(target_os = "macos")]
unsafe fn sync_view_layer_tree_contents_scale(view: id, backing_scale: f64) {
    if view == nil {
        return;
    }

    let layer: id = msg_send![view, layer];
    sync_layer_tree_contents_scale(layer, backing_scale);
    let _: () = msg_send![view, setNeedsDisplay: cocoa::base::YES];

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }

    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let subview: id = msg_send![subviews, objectAtIndex:index];
        sync_view_layer_tree_contents_scale(subview, backing_scale);
    }
}

#[cfg(target_os = "macos")]
unsafe fn sync_window_content_layer_scale(window: id) {
    if window == nil {
        return;
    }

    let backing_scale: f64 = msg_send![window, backingScaleFactor];
    let content_view: id = msg_send![window, contentView];
    sync_view_layer_tree_contents_scale(content_view, backing_scale);
}

#[cfg(target_os = "macos")]
pub fn trace_main_window_native_geometry(
    phase: &'static str,
    cycle_id: u64,
    target_bounds: Option<&Bounds<Pixels>>,
    note: Option<&str>,
) {
    if !native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) { return; }
    if !main_window_geometry_trace_enabled() {
        return;
    }
    if require_main_thread("trace_main_window_native_geometry") {
        return;
    }

    let primary_height = primary_screen_height().unwrap_or(0.0);
    let target_summary = target_bounds
        .map(|bounds| {
            let x: f64 = bounds.origin.x.into();
            let y: f64 = bounds.origin.y.into();
            let width: f64 = bounds.size.width.into();
            let height: f64 = bounds.size.height.into();
            let flipped_y = primary_height - y - height;
            format!("top_left={x:.1},{y:.1},{width:.1}x{height:.1}:flipped_y={flipped_y:.1}")
        })
        .unwrap_or_else(|| "none".to_string());
    let note = note.unwrap_or("");

    unsafe {
        let screens = all_screen_summaries(primary_height);
        let window = window_manager::get_main_window();
        let Some(window) = window else {
            tracing::info!(
                event_type = "main_window_geometry",
                layer = "native",
                phase,
                cycle_id,
                primary_height,
                target = %target_summary,
                screens = %screens,
                note,
                "main window geometry native snapshot missing window"
            );
            logging::log(
                "WINDOW_GEOM",
                &format!("SK_GEOM cycle={cycle_id} phase={phase} native missing_window"),
            );
            return;
        };

        let frame: NSRect = msg_send![window, frame];
        let content_layout_rect: NSRect = msg_send![window, contentLayoutRect];
        let window_number: i64 = msg_send![window, windowNumber];
        let is_visible: cocoa::base::BOOL = msg_send![window, isVisible];
        let is_key: cocoa::base::BOOL = msg_send![window, isKeyWindow];
        let is_main: cocoa::base::BOOL = msg_send![window, isMainWindow];
        let backing_scale: f64 = msg_send![window, backingScaleFactor];
        let level: i64 = msg_send![window, level];
        let style_mask: u64 = msg_send![window, styleMask];
        let collection_behavior: u64 = msg_send![window, collectionBehavior];
        let occlusion_state: u64 = msg_send![window, occlusionState];
        let alpha_value: f64 = msg_send![window, alphaValue];
        let screen: id = msg_send![window, screen];
        let screen_id = ns_screen_number(screen)
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string());
        let screen_scale = if screen == nil {
            0.0
        } else {
            msg_send![screen, backingScaleFactor]
        };
        let content_view: id = msg_send![window, contentView];
        let (content_view_summary, layer_summary) = layer_summary(content_view);

        tracing::info!(
            event_type = "main_window_geometry",
            layer = "native",
            phase,
            cycle_id,
            primary_height,
            target = %target_summary,
            screens = %screens,
            window_number,
            is_visible = is_visible != cocoa::base::NO,
            is_key = is_key != cocoa::base::NO,
            is_main = is_main != cocoa::base::NO,
            frame_bl = %rect_summary(frame),
            frame_tl = %top_left_rect_summary(frame, primary_height),
            content_layout_rect = %rect_summary(content_layout_rect),
            backing_scale,
            screen_id = %screen_id,
            screen_scale,
            level,
            style_mask,
            collection_behavior,
            occlusion_state,
            alpha_value,
            content_view = %content_view_summary,
            layer_state = %layer_summary,
            note,
            "main window geometry native snapshot"
        );
        logging::log(
            "WINDOW_GEOM",
            &format!(
                "SK_GEOM cycle={cycle_id} phase={phase} native window={window_number} screen={screen_id} scale={backing_scale:.2} frame_tl={}",
                top_left_rect_summary(frame, primary_height),
            ),
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn trace_main_window_native_geometry(
    _phase: &'static str,
    _cycle_id: u64,
    _target_bounds: Option<&Bounds<Pixels>>,
    _note: Option<&str>,
) {
}

/// Move the application's main window to new bounds using WindowManager.
/// This uses the registered main window instead of objectAtIndex:0, which
/// avoids issues with tray icons and other system windows in the array.
///
/// IMPORTANT: macOS uses a global coordinate space where Y=0 is at the BOTTOM of the
/// PRIMARY screen, and Y increases upward. The primary screen's origin is always (0,0)
/// at its bottom-left corner. Secondary displays have their own position in this space.
#[cfg(target_os = "macos")]
pub fn move_first_window_to(x: f64, y: f64, width: f64, height: f64) {
    if !native_effect_allowed(crate::runtime_policy::ExternalEffect::NativeVisibility) { return; }
    if require_main_thread("move_first_window_to") {
        return;
    }

    let primary_screen_height = match primary_screen_height() {
        Some(h) => h,
        None => {
            logging::log(
                "POSITION",
                "WARNING: Could not determine primary screen height",
            );
            return;
        }
    };

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

    // SAFETY: Main thread verified. Window from WindowManager is valid.
    // primary_screen_height() already resolved the primary screen (screens[0])
    // used for global coordinate conversion. frame and setFrame:display:animate:
    // are standard NSWindow methods.
    unsafe {
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
        // Moving a hidden GPUI-backed NSPanel between displays can update the
        // NSWindow backing scale before the existing content layer tree follows.
        // Sync it while still hidden so the first ordered-front frame is drawn
        // at the destination display's scale.
        sync_window_content_layer_scale(window);

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
    // KNOWN: Non-macOS stub intentionally no-ops; native window positioning requires platform-specific APIs.
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
// Reposition Arbitrary Window by NSView
// ============================================================================

/// Move a window identified by its raw NSView pointer to the given bounds.
///
/// `x`, `y`, `width`, `height` use top-left origin coordinates (y-down), matching
/// `PersistedWindowBounds`. Coordinate flip to macOS bottom-left origin is applied
/// internally.
///
/// This is used by the AI window mode toggle to restore full position+size when
/// GPUI's `Window::resize()` only supports size changes.
#[cfg(target_os = "macos")]
pub fn move_window_by_view(
    ns_view: std::ptr::NonNull<std::ffi::c_void>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    if !native_effect_allowed(crate::runtime_policy::ExternalEffect::NativeVisibility) { return; }
    if require_main_thread("move_window_by_view") {
        return;
    }

    let primary_screen_height = match primary_screen_height() {
        Some(h) => h,
        None => {
            tracing::warn!(target: "platform", "Cannot determine primary screen height for move_window_by_view");
            return;
        }
    };

    // SAFETY: Main thread verified. The NSView pointer comes from raw_window_handle
    // which guarantees it is valid for the lifetime of the window. We obtain the
    // parent NSWindow via the standard -[NSView window] message, nil-check it, and
    // then call setFrame:display:animate: which is a standard NSWindow method.
    unsafe {
        let view = ns_view.as_ptr() as *mut objc::runtime::Object;
        let ns_window: *mut objc::runtime::Object = msg_send![view, window];
        if ns_window.is_null() {
            tracing::warn!(target: "platform", "NSView has no parent NSWindow in move_window_by_view");
            return;
        }

        // Convert from top-left origin (y down) to bottom-left origin (y up)
        let flipped_y = primary_screen_height - y - height;
        let new_frame = NSRect::new(NSPoint::new(x, flipped_y), NSSize::new(width, height));
        let _: () = msg_send![ns_window, setFrame:new_frame display:true animate:false];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn move_window_by_view(
    _ns_view: std::ptr::NonNull<std::ffi::c_void>,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
) {
    // No-op on non-macOS platforms
}

// ============================================================================
// Window Positioning (Eye-line)
// ============================================================================

const EYE_LINE_Y_RATIO: f64 = 0.14;
const FALLBACK_VISIBLE_WIDTH: f64 = 1512.0;
const FALLBACK_VISIBLE_HEIGHT: f64 = 982.0;

#[derive(Clone, Copy)]
enum MouseDisplayPlacement {
    EyeLine,
    Centered,
}

fn display_edges(bounds: &DisplayBounds) -> (f64, f64) {
    (
        bounds.origin_x + bounds.width,
        bounds.origin_y + bounds.height,
    )
}

fn log_positioning_banner(title_line: &str) {
    logging::log("POSITION", "");
    logging::log(
        "POSITION",
        "╔════════════════════════════════════════════════════════════╗",
    );
    logging::log("POSITION", title_line);
    logging::log(
        "POSITION",
        "╚════════════════════════════════════════════════════════════╝",
    );
}

fn log_available_displays(displays: &[VisibleDisplayBounds]) {
    for (idx, display) in displays.iter().enumerate() {
        let (frame_right, frame_bottom) = display_edges(&display.frame);
        let (visible_right, visible_bottom) = display_edges(&display.visible_area);

        logging::log(
            "POSITION",
            &format!(
                "  Display {}: frame=({:.0},{:.0}) {:.0}x{:.0} [x={:.0}..{:.0}, y={:.0}..{:.0}] visible=({:.0},{:.0}) {:.0}x{:.0} [x={:.0}..{:.0}, y={:.0}..{:.0}]",
                idx,
                display.frame.origin_x,
                display.frame.origin_y,
                display.frame.width,
                display.frame.height,
                display.frame.origin_x,
                frame_right,
                display.frame.origin_y,
                frame_bottom,
                display.visible_area.origin_x,
                display.visible_area.origin_y,
                display.visible_area.width,
                display.visible_area.height,
                display.visible_area.origin_x,
                visible_right,
                display.visible_area.origin_y,
                visible_bottom
            ),
        );
    }
}

fn select_display_for_mouse(
    mouse: Option<(f64, f64)>,
    displays: &[VisibleDisplayBounds],
) -> Option<VisibleDisplayBounds> {
    if let Some((mouse_x, mouse_y)) = mouse {
        logging::log(
            "POSITION",
            &format!("Mouse cursor at ({:.0}, {:.0})", mouse_x, mouse_y),
        );

        if let Some(display) = display_for_point((mouse_x, mouse_y), displays) {
            return Some(display);
        }

        logging::log(
            "POSITION",
            "Mouse is not inside any display frame, falling back to primary display",
        );
    } else {
        logging::log(
            "POSITION",
            "Could not get mouse position, using primary display",
        );
    }

    displays.first().cloned()
}

fn fallback_display_bounds() -> VisibleDisplayBounds {
    let fallback = DisplayBounds {
        origin_x: 0.0,
        origin_y: 0.0,
        width: FALLBACK_VISIBLE_WIDTH,
        height: FALLBACK_VISIBLE_HEIGHT,
    };

    VisibleDisplayBounds {
        frame: fallback.clone(),
        visible_area: fallback,
    }
}

fn calculate_bounds_for_snapshot(
    window_size: gpui::Size<Pixels>,
    mouse: Option<(f64, f64)>,
    displays: &[VisibleDisplayBounds],
    placement: MouseDisplayPlacement,
) -> Bounds<Pixels> {
    let title_line = match placement {
        MouseDisplayPlacement::EyeLine => {
            "║  CALCULATING WINDOW POSITION FOR MOUSE DISPLAY             ║"
        }
        MouseDisplayPlacement::Centered => {
            "║  CALCULATING CENTERED POSITION ON MOUSE DISPLAY            ║"
        }
    };
    log_positioning_banner(title_line);
    logging::log(
        "POSITION",
        &format!("Available displays: {}", displays.len()),
    );
    log_available_displays(displays);

    let mut used_fallback_display = false;
    let display = select_display_for_mouse(mouse, displays).unwrap_or_else(|| {
        used_fallback_display = true;
        logging::log(
            "POSITION",
            "No displays found, using default centered bounds",
        );
        fallback_display_bounds()
    });

    let visible = &display.visible_area;
    logging::log(
        "POSITION",
        &format!(
            "Selected display visible area: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            visible.origin_x, visible.origin_y, visible.width, visible.height
        ),
    );

    let window_width: f64 = window_size.width.into();
    let window_height: f64 = window_size.height.into();
    let origin_x = visible.origin_x + (visible.width - window_width) / 2.0;
    let origin_y = match placement {
        // Keep fallback behavior centered while normal eye-line placement uses the ratio.
        MouseDisplayPlacement::EyeLine if used_fallback_display => {
            visible.origin_y + (visible.height - window_height) / 2.0
        }
        MouseDisplayPlacement::EyeLine => visible.origin_y + visible.height * EYE_LINE_Y_RATIO,
        MouseDisplayPlacement::Centered => {
            visible.origin_y + (visible.height - window_height) / 2.0
        }
    };

    let desired_bounds = Bounds {
        origin: point(px(origin_x as f32), px(origin_y as f32)),
        size: window_size,
    };
    let final_bounds = clamp_to_visible(desired_bounds, visible);

    let final_x: f64 = final_bounds.origin.x.into();
    let final_y: f64 = final_bounds.origin.y.into();
    let final_width: f64 = final_bounds.size.width.into();
    let final_height: f64 = final_bounds.size.height.into();
    let final_message = match placement {
        MouseDisplayPlacement::EyeLine => format!(
            "Final window bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            final_x, final_y, final_width, final_height
        ),
        MouseDisplayPlacement::Centered => format!(
            "Final centered bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
            final_x, final_y, final_width, final_height
        ),
    };
    logging::log("POSITION", &final_message);

    final_bounds
}

fn calculate_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,
    placement: MouseDisplayPlacement,
) -> Bounds<Pixels> {
    let mouse = get_global_mouse_position();
    let displays = get_macos_visible_displays();
    calculate_bounds_for_snapshot(window_size, mouse, &displays, placement)
}

/// Calculate window bounds positioned at eye-line height using a single caller-provided
/// mouse/display snapshot.
///
/// - Finds the display where the sampled mouse cursor is located
/// - Centers the window horizontally on that display
/// - Positions the window at "eye-line" height (upper 14% of the screen)
///
/// This matches the behavior of Raycast/Alfred where the prompt appears on the active display.
pub fn calculate_eye_line_bounds_for_snapshot(
    window_size: gpui::Size<Pixels>,
    mouse: Option<(f64, f64)>,
    displays: &[VisibleDisplayBounds],
) -> Bounds<Pixels> {
    calculate_bounds_for_snapshot(window_size, mouse, displays, MouseDisplayPlacement::EyeLine)
}

/// Calculate window bounds positioned at eye-line height on the display containing the mouse cursor.
///
/// This thin wrapper samples mouse position and visible displays once, then delegates to
/// `calculate_eye_line_bounds_for_snapshot`.
pub fn calculate_eye_line_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,
) -> Bounds<Pixels> {
    let mouse = get_global_mouse_position();
    let displays = get_macos_visible_displays();
    calculate_eye_line_bounds_for_snapshot(window_size, mouse, &displays)
}

/// Calculate window bounds centered on the display containing the mouse cursor.
///
/// Similar to `calculate_eye_line_bounds_on_mouse_display` but centers the window
/// both horizontally and vertically on the mouse's display. Used for secondary windows
/// like AI chat that should appear on the same display as the main window.
///
/// # Arguments
/// * `window_size` - The desired size of the window
///
/// # Returns
/// Bounds positioned at center of the display containing the mouse cursor,
/// or centered on primary display if mouse position cannot be determined.
pub fn calculate_centered_bounds_on_mouse_display(
    window_size: gpui::Size<Pixels>,
) -> Bounds<Pixels> {
    calculate_bounds_on_mouse_display(window_size, MouseDisplayPlacement::Centered)
}

#[cfg(test)]
mod positioning_bounds_tests {
    use super::*;
    use gpui::size;

    #[test]
    fn test_fallback_display_bounds_sets_frame_and_visible_area() {
        let fallback = fallback_display_bounds();

        assert_eq!(fallback.frame.origin_x, 0.0);
        assert_eq!(fallback.frame.origin_y, 0.0);
        assert_eq!(fallback.frame.width, FALLBACK_VISIBLE_WIDTH);
        assert_eq!(fallback.frame.height, FALLBACK_VISIBLE_HEIGHT);
        assert_eq!(fallback.visible_area.origin_x, 0.0);
        assert_eq!(fallback.visible_area.origin_y, 0.0);
        assert_eq!(fallback.visible_area.width, FALLBACK_VISIBLE_WIDTH);
        assert_eq!(fallback.visible_area.height, FALLBACK_VISIBLE_HEIGHT);
    }

    #[test]
    fn test_calculate_eye_line_bounds_for_snapshot_uses_selected_display_visible_area() {
        let window_size = size(px(600.0), px(240.0));
        let displays = vec![
            VisibleDisplayBounds {
                frame: DisplayBounds {
                    origin_x: 0.0,
                    origin_y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                visible_area: DisplayBounds {
                    origin_x: 0.0,
                    origin_y: 24.0,
                    width: 1440.0,
                    height: 840.0,
                },
            },
            VisibleDisplayBounds {
                frame: DisplayBounds {
                    origin_x: 1440.0,
                    origin_y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                visible_area: DisplayBounds {
                    origin_x: 1440.0,
                    origin_y: 32.0,
                    width: 1920.0,
                    height: 1008.0,
                },
            },
        ];

        let bounds =
            calculate_eye_line_bounds_for_snapshot(window_size, Some((1700.0, 200.0)), &displays);

        let x: f64 = bounds.origin.x.into();
        let y: f64 = bounds.origin.y.into();

        assert_eq!(x, 2100.0);
        assert!((y - 173.12).abs() < 0.01, "expected y near 173.12, got {y}");
    }

    #[test]
    fn test_calculate_eye_line_bounds_for_snapshot_centers_when_snapshot_has_no_displays() {
        let window_size = size(px(1000.0), px(200.0));

        let bounds = calculate_eye_line_bounds_for_snapshot(window_size, None, &[]);

        let x: f64 = bounds.origin.x.into();
        let y: f64 = bounds.origin.y.into();

        assert_eq!(x, 256.0);
        assert_eq!(y, 391.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_calculate_eye_line_bounds_on_mouse_display_centers_fallback_for_window_size() {
        let window_size = size(px(1000.0), px(200.0));

        let eye_line = calculate_eye_line_bounds_on_mouse_display(window_size);
        let centered = calculate_centered_bounds_on_mouse_display(window_size);

        let eye_line_x: f64 = eye_line.origin.x.into();
        let eye_line_y: f64 = eye_line.origin.y.into();
        let centered_x: f64 = centered.origin.x.into();
        let centered_y: f64 = centered.origin.y.into();

        assert_eq!(eye_line_x, centered_x);
        assert_eq!(eye_line_y, centered_y);
    }
}

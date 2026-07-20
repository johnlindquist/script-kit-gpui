//! Shared inline-popup window helpers.
//!
//! These helpers own the detached child-window mechanics used by any inline
//! popup surface (Agent Chat composer pickers, Agent Chat history popup, and the menu-syntax
//! `:`, `;`, and `!` trigger pickers). They are intentionally
//! neutral: no Agent Chat types, no menu-syntax types, no domain callbacks. Callers
//! layer their own row models and accept behavior on top.
//!
//! Every symbol that used to live in `src/ai/agent_chat/ui/popup_window.rs` under the
//! `DENSE_PICKER_*` / `dense_picker_*` / `popup_*` names has been renamed to a
//! neutral `INLINE_POPUP_*` / `inline_popup_*` form here. Agent Chat keeps a thin
//! compatibility facade via `pub(crate) use ... as old_name;` re-exports so
//! existing call sites and source-text audit tests continue to compile without
//! edits.

use gpui::{
    px, AnyWindowHandle, App, AppContext, Bounds, DisplayId, Pixels, Window, WindowBounds,
    WindowHandle, WindowKind, WindowOptions,
};

#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSRect, NSSize};

/// Maximum rows a dense inline popup shows before scrolling kicks in.
pub const INLINE_POPUP_MAX_VISIBLE_ROWS: usize = 8;

/// Vertical padding applied above and below the popup's row list.
pub const INLINE_POPUP_VERTICAL_PADDING: f32 = 4.0;

/// Height used when the popup has zero rows (empty state).
pub const INLINE_POPUP_EMPTY_HEIGHT: f32 = 56.0;

/// Default popup width cap.
pub const INLINE_POPUP_DEFAULT_WIDTH: f32 = 320.0;

/// Minimum popup width — never goes narrower even when the parent is cramped.
pub const INLINE_POPUP_MIN_WIDTH: f32 = 168.0;

/// Gutter reserved on both sides of the parent window when fitting the popup.
pub const INLINE_POPUP_EDGE_GUTTER: f32 = 12.0;

/// Left margin used by callers that anchor the popup to the composer gutter.
pub const INLINE_POPUP_LEFT_MARGIN: f32 = 8.0;

#[cfg(target_os = "macos")]
const NS_WINDOW_ABOVE: i64 = 1;

/// Compute popup height for a row count and row height.
///
/// Zero rows returns [`INLINE_POPUP_EMPTY_HEIGHT`] so an empty-state popup
/// still has a visible surface.
pub fn inline_popup_height_for_row_height(item_count: usize, row_height: f32) -> f32 {
    if item_count == 0 {
        return INLINE_POPUP_EMPTY_HEIGHT;
    }

    let visible_rows = item_count.min(INLINE_POPUP_MAX_VISIBLE_ROWS) as f32;
    (visible_rows * row_height) + (INLINE_POPUP_VERTICAL_PADDING * 2.0)
}

/// Clamp the popup width to the parent window, honoring the min/default caps
/// and the edge gutter on both sides.
pub fn inline_popup_width_for_window(window_width: f32) -> f32 {
    let max_width =
        (window_width - (INLINE_POPUP_EDGE_GUTTER * 2.0)).min(INLINE_POPUP_DEFAULT_WIDTH);
    max_width.max(INLINE_POPUP_MIN_WIDTH)
}

/// Top anchor for popups that prefer to sit above the mini-shell hint strip.
pub fn footer_anchored_inline_popup_top(parent_height: f32, popup_height: f32) -> f32 {
    let bottom_offset = crate::window_resize::main_layout::HINT_STRIP_HEIGHT + 4.0;
    (parent_height - bottom_offset - popup_height).max(0.0)
}

/// Build screen-relative popup bounds from `(left, top, width, height)`
/// offsets applied to the parent window's origin.
pub fn inline_popup_bounds(
    parent_bounds: Bounds<Pixels>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
) -> Bounds<Pixels> {
    Bounds {
        origin: gpui::point(
            parent_bounds.origin.x + px(left),
            parent_bounds.origin.y + px(top),
        ),
        size: gpui::size(px(width), px(height)),
    }
}

/// Window options for a no-focus-steal popup. Theme-aware so vibrancy callers
/// get a blurred background and opaque callers get a solid one.
pub fn inline_popup_window_options(
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
) -> WindowOptions {
    let theme = crate::theme::get_cached_theme();
    let window_background = if theme.is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        // Popups size from row content via `set_inline_popup_window_bounds`; manual
        // edge resize would fight the left-drawer / dense-picker height contract.
        is_movable: false,
        is_resizable: false,
        display_id,
        ..Default::default()
    }
}

/// Configure the newly-created popup NSWindow: dark-vibrancy + attach as a
/// child of `parent_window_handle` so it follows the parent.
pub fn configure_inline_popup_window<T: 'static>(
    handle: &WindowHandle<T>,
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let is_dark_vibrancy = crate::theme::get_cached_theme().should_use_dark_vibrancy();
        handle
            .update(cx, move |_popup, window, cx| {
                window.defer(cx, move |window, cx| {
                    if let Some(ns_window) = inline_popup_ns_window(window) {
                        // SAFETY: `ns_window` comes from the live GPUI popup window on the
                        // main thread and is nil-checked before configuration.
                        unsafe {
                            crate::platform::configure_inline_dropdown_popup_window(
                                ns_window,
                                is_dark_vibrancy,
                            );
                        }
                        attach_inline_popup_to_parent_window(cx, parent_window_handle, ns_window);

                        tracing::info!(
                            target: "script_kit::inline_popup",
                            event = "inline_popup_attached",
                            dark = is_dark_vibrancy,
                            "Attached inline popup window to parent window"
                        );
                    }
                });
            })
            .map_err(|_| anyhow::anyhow!("failed to configure inline popup window"))?;
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (handle, cx, parent_window_handle);

    Ok(())
}

#[cfg(target_os = "macos")]
fn ns_window_frame_from_screen_relative_bounds(
    bounds: Bounds<Pixels>,
    screen_frame: NSRect,
) -> NSRect {
    NSRect::new(
        NSPoint::new(
            screen_frame.origin.x + f32::from(bounds.origin.x) as f64,
            screen_frame.origin.y + screen_frame.size.height
                - f32::from(bounds.origin.y) as f64
                - f32::from(bounds.size.height) as f64,
        ),
        NSSize::new(
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        ),
    )
}

/// Update the popup NSWindow bounds without animation. GPUI's bounds are
/// screen-relative; we resolve the popup's current NSScreen and convert back
/// into AppKit coords before calling `setFrame` so multi-monitor setups work.
#[cfg(target_os = "macos")]
pub fn set_inline_popup_window_bounds(window: &mut Window, bounds: Bounds<Pixels>, cx: &mut App) {
    if let Some(ns_window) = inline_popup_ns_window(window) {
        // SAFETY: `ns_window` comes from a live GPUI popup window on the AppKit
        // main thread. GPUI `window.bounds()` is screen-relative, so we resolve
        // the popup's current NSScreen and convert back into that screen's
        // AppKit coordinate space before calling `setFrame`.
        unsafe {
            use cocoa::appkit::NSScreen;
            use cocoa::base::nil;

            let screen: cocoa::base::id = msg_send![ns_window, screen];
            let screen_frame = if screen != nil {
                let frame: NSRect = msg_send![screen, frame];
                frame
            } else {
                let screens: cocoa::base::id = NSScreen::screens(nil);
                let primary_screen: cocoa::base::id = msg_send![screens, objectAtIndex: 0u64];
                let frame: NSRect = msg_send![primary_screen, frame];
                frame
            };
            let target_frame = ns_window_frame_from_screen_relative_bounds(bounds, screen_frame);
            let _: () = msg_send![
                ns_window,
                setFrame: target_frame
                display: true
                animate: false
            ];
        }
    }

    window.resize(bounds.size);
    window.bounds_changed(cx);
}

#[cfg(not(target_os = "macos"))]
pub fn set_inline_popup_window_bounds(window: &mut Window, bounds: Bounds<Pixels>, cx: &mut App) {
    let _ = cx;
    window.resize(bounds.size);
}

/// Return the native `NSWindow` handle backing a live GPUI window, or `None`
/// on non-AppKit platforms / failed raw-handle lookup.
#[cfg(target_os = "macos")]
pub fn inline_popup_ns_window(window: &mut Window) -> Option<cocoa::base::id> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use cocoa::base::nil;

            let ns_view = appkit.ns_view.as_ptr() as cocoa::base::id;
            // SAFETY: `ns_view` comes from the live GPUI window on the main thread.
            unsafe {
                let ns_window: cocoa::base::id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some(ns_window);
                }
            }
        }
    }

    None
}

/// Attach the popup NSWindow as a child of the parent launcher/composer
/// window so it follows focus, space moves, and parent closes.
#[cfg(target_os = "macos")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn attach_inline_popup_to_parent_window(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    child_ns_window: cocoa::base::id,
) {
    let _ = cx.update_window(parent_window_handle, move |_, parent_window, _cx| {
        let Some(parent_ns_window) = inline_popup_ns_window(parent_window) else {
            return;
        };

        // SAFETY: both NSWindow pointers come from live GPUI windows on the main
        // thread, and nil/equality are guarded before AppKit receives them.
        unsafe {
            use cocoa::base::nil;

            if parent_ns_window == nil
                || child_ns_window == nil
                || parent_ns_window == child_ns_window
            {
                return;
            }

            let _: () = msg_send![
                parent_ns_window,
                addChildWindow: child_ns_window
                ordered: NS_WINDOW_ABOVE
            ];
            let _: () = msg_send![child_ns_window, orderFrontRegardless];
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        footer_anchored_inline_popup_top, inline_popup_bounds, inline_popup_height_for_row_height,
        inline_popup_width_for_window, INLINE_POPUP_DEFAULT_WIDTH, INLINE_POPUP_MIN_WIDTH,
    };

    #[test]
    fn inline_popup_height_uses_empty_state_when_zero_rows() {
        assert!(inline_popup_height_for_row_height(0, 36.0) > 0.0);
    }

    #[test]
    fn inline_popup_height_caps_at_max_visible_rows() {
        // 12 rows should be equivalent to 8 rows (the max visible cap).
        assert_eq!(
            inline_popup_height_for_row_height(12, 36.0),
            inline_popup_height_for_row_height(8, 36.0),
        );
    }

    #[test]
    fn inline_popup_height_accepts_custom_row_height() {
        assert!(
            inline_popup_height_for_row_height(8, 36.0)
                < inline_popup_height_for_row_height(8, 40.0)
        );
    }

    #[test]
    fn inline_popup_width_matches_window_constraints() {
        assert_eq!(
            inline_popup_width_for_window(900.0),
            INLINE_POPUP_DEFAULT_WIDTH
        );
        assert_eq!(inline_popup_width_for_window(180.0), INLINE_POPUP_MIN_WIDTH);
    }

    #[test]
    fn footer_anchor_keeps_popup_above_hint_strip() {
        assert!(footer_anchored_inline_popup_top(400.0, 80.0) >= 0.0);
    }

    #[test]
    fn inline_popup_window_options_disable_manual_resize() {
        let source = include_str!("inline_popup_window.rs");
        assert!(
            source.contains("is_movable: false") && source.contains("is_resizable: false"),
            "inline popup windows must be sized only by content-driven bounds updates"
        );
    }

    #[test]
    fn inline_popup_bounds_offset_from_parent_origin() {
        let parent = gpui::Bounds {
            origin: gpui::point(gpui::px(100.0), gpui::px(40.0)),
            size: gpui::size(gpui::px(600.0), gpui::px(400.0)),
        };

        let bounds = inline_popup_bounds(parent, 8.0, 16.0, 200.0, 80.0);
        assert_eq!(f32::from(bounds.origin.x), 108.0);
        assert_eq!(f32::from(bounds.origin.y), 56.0);
        assert_eq!(f32::from(bounds.size.width), 200.0);
        assert_eq!(f32::from(bounds.size.height), 80.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn screen_relative_bounds_convert_to_nswindow_frame_on_secondary_display() {
        let bounds = gpui::Bounds {
            origin: gpui::point(gpui::px(24.0), gpui::px(60.0)),
            size: gpui::size(gpui::px(320.0), gpui::px(84.0)),
        };
        let screen_frame = cocoa::foundation::NSRect::new(
            cocoa::foundation::NSPoint::new(1440.0, 0.0),
            cocoa::foundation::NSSize::new(1920.0, 1200.0),
        );

        let frame = super::ns_window_frame_from_screen_relative_bounds(bounds, screen_frame);

        assert_eq!(frame.origin.x, 1464.0);
        assert_eq!(frame.origin.y, 1056.0);
        assert_eq!(frame.size.width, 320.0);
        assert_eq!(frame.size.height, 84.0);
    }
}

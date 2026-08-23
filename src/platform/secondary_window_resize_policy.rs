#[cfg(target_os = "macos")]
pub fn set_window_resizable(window: &mut gpui::Window, resizable: bool) {
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};

    const NS_WINDOW_STYLE_MASK_RESIZABLE: u64 = 1 << 3;

    let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() else {
        return;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        let current_style_mask: u64 = msg_send![ns_window, styleMask];
        let next_style_mask = if resizable {
            current_style_mask | NS_WINDOW_STYLE_MASK_RESIZABLE
        } else {
            current_style_mask & !NS_WINDOW_STYLE_MASK_RESIZABLE
        };
        if next_style_mask != current_style_mask {
            let _: () = msg_send![ns_window, setStyleMask: next_style_mask];
            if Some(ns_window) == crate::window_manager::get_main_window() {
                for button_type in 0..=2 {
                    let button: id = msg_send![ns_window, standardWindowButton: button_type];
                    if !button.is_null() {
                        let _: () = msg_send![button, setHidden: true];
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_window_resizable(_window: &mut gpui::Window, _resizable: bool) {}

/// Apply a complete [`WindowResizePolicy`] to the native window backing a GPUI
/// window: the resizable style bit (policy AND the caller's interaction gate)
/// plus content min/max size constraints. Returns `false` when the native
/// window cannot be resolved.
///
/// The `interaction_enabled` gate lets lifecycle owners keep a policy-resizable
/// shell locked while a calibrated entry/exit morph owns the frame (the glass
/// motion calibration is untouched by this function — it only flips the style
/// bit and constraints).
#[cfg(target_os = "macos")]
pub(crate) fn apply_window_resize_policy(
    window: &gpui::Window,
    policy: crate::window_resize::policy::WindowResizePolicy,
    interaction_enabled: bool,
) -> bool {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSSize;
    use objc::{msg_send, sel, sel_impl};

    const NS_WINDOW_STYLE_MASK_RESIZABLE: u64 = 1 << 3;

    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    unsafe {
        let ns_view = appkit.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![ns_view, window];
        if ns_window == nil {
            return false;
        }
        let before: u64 = msg_send![ns_window, styleMask];
        let should_resize = policy.user_resizable && interaction_enabled;
        let desired = if should_resize {
            before | NS_WINDOW_STYLE_MASK_RESIZABLE
        } else {
            before & !NS_WINDOW_STYLE_MASK_RESIZABLE
        };
        if desired != before {
            let _: () = msg_send![ns_window, setStyleMask: desired];
        }
        let _: () = msg_send![
            ns_window,
            setContentMinSize: NSSize::new(policy.min_content_width, policy.min_content_height)
        ];
        if let (Some(width), Some(height)) = (policy.max_content_width, policy.max_content_height) {
            let _: () = msg_send![ns_window, setContentMaxSize: NSSize::new(width, height)];
        }
        let window_number: i64 = msg_send![ns_window, windowNumber];
        let in_live_resize: bool = msg_send![ns_window, inLiveResize];
        let backing_scale: f64 = msg_send![ns_window, backingScaleFactor];
        tracing::info!(
            target: "script_kit::platform",
            event = "window_resize_policy_applied",
            window_number,
            style_mask_before = before,
            style_mask_after = desired,
            user_resizable = policy.user_resizable,
            interaction_enabled,
            min_content_width = policy.min_content_width,
            min_content_height = policy.min_content_height,
            max_content_width = policy.max_content_width,
            max_content_height = policy.max_content_height,
            backing_scale,
            in_live_resize,
        );
        true
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_window_resize_policy(
    _window: &gpui::Window,
    _policy: crate::window_resize::policy::WindowResizePolicy,
    _interaction_enabled: bool,
) -> bool {
    false
}

/// Dynamically re-partition a GPUI window's Tahoe glass backdrop between the
/// full-window layout (`bottom_inset <= 0`) and the detached-footer layout
/// (`bottom_inset > 0`). Owns geometry + shadow only: tint/material are NOT
/// reapplied (the theme signature did not change), so calling this on every
/// mode switch cannot flash the glass style.
///
/// Returns `false` when the native window or tagged backdrop cannot be
/// resolved (e.g. glass composition unavailable) — callers treat that as
/// "no native backdrop to partition", not an error.
#[cfg(target_os = "macos")]
#[allow(
    dead_code,
    reason = "the independently compiled main-window binary owns native footer/backdrop partitioning"
)]
pub(crate) fn set_gpui_window_backdrop_bottom_inset(
    window: &gpui::Window,
    window_name: &'static str,
    bottom_inset: f64,
) -> bool {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    unsafe {
        let ns_view = appkit.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![ns_view, window];
        if ns_window == nil {
            return false;
        }
        let content_view: id = msg_send![ns_window, contentView];
        if content_view == nil {
            return false;
        }
        let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
        if glass_view == nil {
            return false;
        }
        let layout = if bottom_inset > 0.0 {
            TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset }
        } else {
            TahoeBackdropLayout::FullWindow
        };
        let current_inset: f64 = *(*(glass_view as *const objc::runtime::Object))
            .get_ivar::<f64>("_scriptKitBottomInset");
        let corner_radius = tahoe_backdrop_corner_radius_for(window_name, layout, content_view);
        update_tahoe_backdrop_geometry_and_shadow(
            ns_window,
            content_view,
            glass_view,
            layout,
            corner_radius,
        );
        let window_number: i64 = msg_send![ns_window, windowNumber];
        tracing::info!(
            target: "script_kit::platform",
            event = "window_backdrop_bottom_inset_set",
            window_name,
            window_number,
            bottom_inset_before = current_inset,
            bottom_inset_after = layout.bottom_inset(),
            corner_radius,
        );
        true
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_gpui_window_backdrop_bottom_inset(
    _window: &gpui::Window,
    _window_name: &'static str,
    _bottom_inset: f64,
) -> bool {
    false
}

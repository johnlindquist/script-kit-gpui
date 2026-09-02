// Platform cursor management for non-activating popup windows (macOS).
//
// Problem recap
// ------------
// In a non-activating NSPanel (Spotlight-style), the owning app usually stays inactive.
// When the app is inactive, WindowServer often keeps using the *active* app's cursor
// rect evaluation. If you try to fight that with `[NSCursor set]` from a global
// mouse-move stream, you can end up in a cursor "tug of war" (visible as flicker).
//
// What actually makes it stick
// ----------------------------
// The private connection property "SetsCursorInBackground" is necessary so an
// inactive app is allowed to change the cursor at all.
//
// But the missing piece for non-activating panels is the *window tag*
// `kCGSSetsCursorInBackgroundTagBit`. Without that per-window tag, WindowServer
// can continue to prefer the active app's cursor updates even while the mouse is
// over your panel.
//
// This file does BOTH:
//   1) CGSSetConnectionProperty(..., "SetsCursorInBackground", true)
//   2) CGSSetWindowTags(..., kCGSSetsCursorInBackgroundTagBit)
//      for all NSWindows owned by this process.
//
// GPUI integration
// ----------------
// Keep the legacy dynamic-cursor API surface as no-ops. Cursor behavior is now
// handled via static setup only (`install_cursor_tracking` + cursor rects).

// ============================================================================
// Layer 1 — WindowServer knobs (private)
// ============================================================================

#[cfg(target_os = "macos")]
mod macos_ws {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_void};
    use std::sync::OnceLock;

    use cocoa::appkit::NSApp;
    use cocoa::base::{nil, YES};
    use objc::{sel, sel_impl};

    pub type CGSConnectionID = c_int;
    pub type CGError = c_int;
    pub type CGSWindowTagBit = c_int;
    pub type CGWindowID = u32;

    // "Real" maximum tag size used by public examples for CGSSetWindowTags.
    pub const K_CGS_REAL_MAXIMUM_TAG_SIZE: usize = 0x40;

    // From CGSWindow.h (private): window may set cursor while app is inactive.
    pub const K_CGS_SETS_CURSOR_IN_BACKGROUND_TAG_BIT: CGSWindowTagBit = 1 << 5;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGSMainConnectionID() -> CGSConnectionID;
        pub fn CGSSetConnectionProperty(
            cid: CGSConnectionID,
            target_cid: CGSConnectionID,
            key: cocoa::base::id,
            value: cocoa::base::id,
        ) -> CGError;
    }

    // We *dlsym* CGSSetWindowTags (and the SkyLight name SLSSetWindowTags) to avoid
    // link-time failures across macOS releases.
    extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    // RTLD_DEFAULT is ((void *) -2) on macOS.
    const RTLD_DEFAULT: *mut c_void = (-2isize) as *mut c_void;

    pub type SetWindowTagsFn =
        unsafe extern "C" fn(CGSConnectionID, CGWindowID, *const CGSWindowTagBit, usize) -> CGError;

    fn load_set_window_tags_fn() -> Option<SetWindowTagsFn> {
        static FN: OnceLock<Option<SetWindowTagsFn>> = OnceLock::new();

        // SAFETY: dlsym returns a valid function pointer or null for the given
        // symbol name. Non-null pointers are transmuted to the known FFI signature
        // of CGSSetWindowTags/SLSSetWindowTags. OnceLock ensures single init.
        *FN.get_or_init(|| unsafe {
            for name in [b"CGSSetWindowTags\0", b"SLSSetWindowTags\0"] {
                let ptr = dlsym(RTLD_DEFAULT, name.as_ptr() as *const c_char);
                if !ptr.is_null() {
                    return Some(std::mem::transmute::<*mut c_void, SetWindowTagsFn>(ptr));
                }
            }
            None
        })
    }

    // SAFETY: Caller must ensure `s` is a valid NUL-terminated CStr.
    // Returns an autoreleased NSString.
    unsafe fn nsstring_utf8(s: &CStr) -> cocoa::base::id {
        let ns_str: cocoa::base::id =
            objc::msg_send![objc::class!(NSString), stringWithUTF8String: s.as_ptr()];
        ns_str
    }

    // SAFETY: Must be called from the main thread. Uses CGSMainConnectionID and
    // CGSSetConnectionProperty which require a valid WindowServer connection.
    pub unsafe fn enable_sets_cursor_in_background_for_connection() {
        let connection_id = CGSMainConnectionID();
        if connection_id == 0 {
            crate::logging::log(
                "CURSOR",
                "install_cursor_tracking: CGSMainConnectionID returned 0",
            );
            return;
        }

        let key: cocoa::base::id = nsstring_utf8(c"SetsCursorInBackground");
        if key.is_null() {
            crate::logging::log(
                "CURSOR",
                "install_cursor_tracking: failed to create NSString key",
            );
            return;
        }

        let value: cocoa::base::id = objc::msg_send![objc::class!(NSNumber), numberWithBool: YES];
        if value.is_null() {
            crate::logging::log(
                "CURSOR",
                "install_cursor_tracking: failed to create NSNumber value",
            );
            return;
        }

        let result = CGSSetConnectionProperty(connection_id, connection_id, key, value);

        if result == 0 {
            crate::logging::log("CURSOR", "Enabled SetsCursorInBackground (connection)");
        } else {
            crate::logging::log(
                "CURSOR",
                &format!(
                    "Failed to enable SetsCursorInBackground (connection) (status={})",
                    result
                ),
            );
        }
    }

    // SAFETY: Must be called from the main thread. Iterates NSApp windows via objc
    // messaging and applies WindowServer tags. All pointers are nil-checked.
    pub unsafe fn tag_all_app_windows_sets_cursor_in_background() {
        let Some(set_window_tags) = load_set_window_tags_fn() else {
            crate::logging::log(
                "CURSOR",
                "Could not find CGSSetWindowTags/SLSSetWindowTags via dlsym; skipping window tag",
            );
            return;
        };

        let connection_id = CGSMainConnectionID();
        if connection_id == 0 {
            return;
        }

        let app: cocoa::base::id = NSApp();
        if app == nil {
            return;
        }

        // windows is an NSArray<NSWindow *>
        let windows: cocoa::base::id = objc::msg_send![app, windows];
        if windows == nil {
            return;
        }

        let count: usize = objc::msg_send![windows, count];
        if count == 0 {
            return;
        }

        let tags: [CGSWindowTagBit; 2] = [K_CGS_SETS_CURSOR_IN_BACKGROUND_TAG_BIT, 0];

        let mut tagged = 0usize;
        let mut failed = 0usize;

        for i in 0..count {
            let win: cocoa::base::id = objc::msg_send![windows, objectAtIndex: i];
            if win == nil {
                continue;
            }

            let wid_i32: i32 = objc::msg_send![win, windowNumber];
            if wid_i32 <= 0 {
                continue;
            }

            let wid: CGWindowID = wid_i32 as CGWindowID;

            let err = set_window_tags(
                connection_id,
                wid,
                tags.as_ptr(),
                K_CGS_REAL_MAXIMUM_TAG_SIZE,
            );
            if err == 0 {
                tagged += 1;
            } else {
                failed += 1;
            }
        }

        if tagged > 0 {
            crate::logging::log(
                "CURSOR",
                &format!(
                    "Applied kCGSSetsCursorInBackgroundTagBit to {} window(s) ({} failed)",
                    tagged, failed
                ),
            );
        }
    }
}

/// Install cursor rects on the main window's content view.
///
/// Adds a `resetCursorRects` override so the Window Server uses OUR cursor
/// rects (arrow for the whole view) instead of falling through to the
/// underlying active app's cursor rects (e.g. Terminal's I-beam).
#[cfg(target_os = "macos")]
fn install_cursor_rects() {
    // SAFETY: Main thread. ObjC with null checks.
    unsafe {
        let window = match crate::window_manager::get_main_window() {
            Some(w) => w,
            None => return,
        };

        let content_view: cocoa::base::id = msg_send![window, contentView];
        if content_view.is_null() {
            return;
        }

        let view_class: *const std::ffi::c_void = msg_send![content_view, class];
        if view_class.is_null() {
            return;
        }

        extern "C" fn reset_cursor_rects_impl(
            this: *mut std::ffi::c_void,
            _cmd: objc::runtime::Sel,
        ) {
            // SAFETY: `this` is a valid NSView pointer provided by the ObjC runtime.
            // bounds, arrowCursor, and addCursorRect:cursor: are standard AppKit methods.
            unsafe {
                let this = this as cocoa::base::id;
                let bounds: cocoa::foundation::NSRect = msg_send![this, bounds];
                let arrow: cocoa::base::id = msg_send![class!(NSCursor), arrowCursor];
                let _: () = msg_send![this, addCursorRect:bounds cursor:arrow];
            }
        }

        let method_sel = sel!(resetCursorRects);
        #[allow(clippy::missing_transmute_annotations)]
        let new_imp: objc::runtime::Imp = std::mem::transmute::<_, objc::runtime::Imp>(
            reset_cursor_rects_impl as extern "C" fn(*mut std::ffi::c_void, objc::runtime::Sel),
        );

        // GPUI's view class already defines resetCursorRects, so class_addMethod
        // will fail. Use method_setImplementation to swizzle it instead.
        let existing = objc::runtime::class_getInstanceMethod(
            view_class as *mut objc::runtime::Class,
            method_sel,
        );

        if existing.is_null() {
            let types = c"v@:";
            let added = objc::runtime::class_addMethod(
                view_class as *mut objc::runtime::Class,
                method_sel,
                new_imp,
                types.as_ptr(),
            );
            crate::logging::log(
                "CURSOR",
                &format!("install_cursor_rects: added new method={}", added),
            );
        } else {
            let _old = objc::runtime::method_setImplementation(
                existing as *mut objc::runtime::Method,
                new_imp,
            );
            crate::logging::log("CURSOR", "install_cursor_rects: swizzled existing method");
        }

        let _: () = msg_send![window, invalidateCursorRectsForView: content_view];
    }
}

/// Enable background cursor-setting support for this process and tag our windows.
///
/// Call this after your NSWindow/NSPanel exists (e.g. during window configuration).
///
/// Safe to call multiple times — connection property uses `Once`, window tags
/// are re-applied each call (so new windows get tagged).
#[cfg(target_os = "macos")]
pub fn install_cursor_tracking() {
    if !native_effect_allowed(crate::runtime_policy::ExternalEffect::NativeInput) { return; }
    use std::sync::Once;

    static CONN_ONCE: Once = Once::new();

    // 1) Connection-level permission: do once.
    // SAFETY: enable_sets_cursor_in_background_for_connection sends ObjC messages
    // to set the WindowServer connection property. Called once via Once guard.
    CONN_ONCE.call_once(|| unsafe {
        macos_ws::enable_sets_cursor_in_background_for_connection();
    });

    // 2) Window-level tag: do every call (new windows may have appeared).
    // SAFETY: tag_all_app_windows_sets_cursor_in_background iterates NSApp windows
    // and applies cursor tags via validated function pointer. Nil-checks throughout.
    unsafe {
        macos_ws::tag_all_app_windows_sets_cursor_in_background();
    }

    // 3) Cursor rects on content view: tells WS to use OUR arrow cursor
    //    instead of falling through to the underlying app's cursor rects.
    static RECTS_ONCE: Once = Once::new();
    RECTS_ONCE.call_once(install_cursor_rects);
}

#[cfg(not(target_os = "macos"))]
pub fn install_cursor_tracking() {
    // No-op on non-macOS platforms.
}

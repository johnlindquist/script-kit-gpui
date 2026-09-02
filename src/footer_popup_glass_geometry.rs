/// Canonical partition of the expanded main-window host used by the detached
/// footer composition. The same physical regions are expressed in either
/// GPUI's top-left coordinate space or AppKit's bottom-left coordinate space.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MainWindowDetachedFooterRegions {
    pub host: crate::protocol::LayoutBounds,
    pub main_content: crate::protocol::LayoutBounds,
    pub transparent_gap: crate::protocol::LayoutBounds,
    pub footer: crate::protocol::LayoutBounds,
}

fn round_footer_region_value(value: f32, backing_scale: f32) -> f32 {
    let scale = if backing_scale.is_finite() && backing_scale > 0.0 {
        backing_scale
    } else {
        1.0
    };
    (value * scale).round() / scale
}

fn main_window_detached_footer_region_dimensions(
    width: f32,
    host_height: f32,
    footer_height: f32,
    gap_height: f32,
    backing_scale: f32,
) -> (f32, f32, f32, f32) {
    let width = round_footer_region_value(width.max(0.0), backing_scale);
    let host_height = round_footer_region_value(host_height.max(0.0), backing_scale);
    let footer_height =
        round_footer_region_value(footer_height.max(0.0).min(host_height), backing_scale)
            .min(host_height);
    let gap_height = round_footer_region_value(
        gap_height.max(0.0).min(host_height - footer_height),
        backing_scale,
    )
    .min(host_height - footer_height);
    let main_height = host_height - footer_height - gap_height;
    (width, host_height, main_height, gap_height)
}

/// Partition an expanded host in GPUI's top-left, y-down coordinate space.
pub(crate) fn main_window_detached_footer_regions_gpui(
    width: f32,
    host_height: f32,
    footer_height: f32,
    gap_height: f32,
    backing_scale: f32,
) -> MainWindowDetachedFooterRegions {
    let (width, host_height, main_height, gap_height) =
        main_window_detached_footer_region_dimensions(
            width,
            host_height,
            footer_height,
            gap_height,
            backing_scale,
        );
    let footer_height = host_height - main_height - gap_height;
    MainWindowDetachedFooterRegions {
        host: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: host_height,
        },
        main_content: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: main_height,
        },
        transparent_gap: crate::protocol::LayoutBounds {
            x: 0.0,
            y: main_height,
            width,
            height: gap_height,
        },
        footer: crate::protocol::LayoutBounds {
            x: 0.0,
            y: main_height + gap_height,
            width,
            height: footer_height,
        },
    }
}

/// Partition an expanded host in AppKit's bottom-left, y-up coordinate space.
pub(crate) fn main_window_detached_footer_regions_appkit(
    width: f32,
    host_height: f32,
    footer_height: f32,
    gap_height: f32,
    backing_scale: f32,
) -> MainWindowDetachedFooterRegions {
    let (width, host_height, main_height, gap_height) =
        main_window_detached_footer_region_dimensions(
            width,
            host_height,
            footer_height,
            gap_height,
            backing_scale,
        );
    let footer_height = host_height - main_height - gap_height;
    MainWindowDetachedFooterRegions {
        host: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: host_height,
        },
        main_content: crate::protocol::LayoutBounds {
            x: 0.0,
            y: footer_height + gap_height,
            width,
            height: main_height,
        },
        transparent_gap: crate::protocol::LayoutBounds {
            x: 0.0,
            y: footer_height,
            width,
            height: gap_height,
        },
        footer: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: footer_height,
        },
    }
}

/// Height of the fully transparent strip the main window reserves below its
/// glass container so the footer capsules float over the bare desktop.
/// Both the GPUI root (bottom padding) and the native NSGlassEffectView
/// backdrop (bottom frame inset) subtract this same value; 0 when float
/// chrome is off.
pub(crate) fn main_window_float_footer_strip_height() -> f32 {
    if glass_scroll_bands_active() {
        crate::components::footer_chrome::current_main_menu_footer_height()
            + FLOAT_FOOTER_CONTAINER_GAP_PX
    } else {
        0.0
    }
}

/// Reconcile the platform-managed footer band and header edge strip with the
/// main window's current glass mode and frame. Called beside Tahoe backdrop
/// recreation and again when the footer host is created/refreshed.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn sync_main_window_glass_scroll_bands(ns_window: id) {
    use cocoa::appkit::NSViewWidthSizable;
    use cocoa::base::YES;
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("sync_main_window_glass_scroll_bands") {
        return;
    }
    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }

    let active = glass_scroll_bands_active();
    let search_root = main_window_footer_search_root(ns_window);
    let mut footer_view = find_subview_by_identifier(search_root, FOOTER_EFFECT_ID);
    if footer_view != nil {
        // Mode changed (same-window Tahoe container <-> blur-era in-window
        // VEV): recreate the host in the correct native parent.
        let float_ok = float_footer_host_view_class()
            .map(|cls| {
                let is_float: cocoa::base::BOOL = msg_send![footer_view, isKindOfClass: cls];
                is_float == YES
            })
            .unwrap_or(false);
        if float_ok != active {
            let _: () = msg_send![footer_view, removeFromSuperview];
            if !active {
                remove_main_window_footer_glass_container(ns_window);
            }
            clear_main_window_footer_refresh_signature();
            footer_view = nil;
        }
    }

    let content_bounds: NSRect = msg_send![content_view, bounds];
    if footer_view != nil {
        let footer_frame = cocoa::foundation::NSRect::new(
            cocoa::foundation::NSPoint::new(0.0, 0.0),
            cocoa::foundation::NSSize::new(content_bounds.size.width, footer_height()),
        );
        let _: () = msg_send![footer_view, setFrame: footer_frame];
    }
    if !active && main_window_footer_glass_root(ns_window) != nil {
        remove_main_window_footer_glass_container(ns_window);
    }
    log_strip_views_debug(ns_window);
    let _ = NSViewWidthSizable;
}

/// Identifier for the content view of the floating-footer child window.
#[cfg(target_os = "macos")]
const FLOAT_FOOTER_LAYER_ID: &str = "script-kit-float-footer-layer";

/// Shared styling for every floating footer capsule. The window backdrop and
/// every capsule resolve through the same appearance/RGB/effective-tint
/// policy; only the capsule role may add the shared adaptive separation rim.
#[cfg(target_os = "macos")]
unsafe fn style_float_footer_capsule(capsule: id, theme: &crate::theme::Theme) {
    let style = crate::platform::resolve_native_glass_style(
        theme,
        crate::platform::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let _ = crate::platform::apply_native_glass_style(capsule, style);
}

#[cfg(target_os = "macos")]
thread_local! {
    /// Tahoe main-window footer containers. These are native siblings of the
    /// GPUI Metal view inside the same NSWindow, so WindowServer translates
    /// the complete composition atomically during a live drag.
    static MAIN_WINDOW_FOOTER_GLASS_HOSTS: std::cell::RefCell<
        std::collections::HashMap<usize, crate::platform::glass_button_host::NativeGlassContainerHost>
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(target_os = "macos")]
unsafe fn main_window_footer_glass_root(ns_window: id) -> id {
    MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        let mut hosts = hosts.borrow_mut();
        hosts.retain(|_, host| host.window_is_alive());
        hosts
            .get(&(ns_window as usize))
            .map(|host| host.inner())
            .unwrap_or(nil)
    })
}

#[cfg(target_os = "macos")]
unsafe fn ensure_main_window_footer_glass_container(gpui_view: id, ns_window: id) -> id {
    use cocoa::appkit::NSViewWidthSizable;
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return nil;
    }
    let content_bounds: NSRect = msg_send![content_view, bounds];
    let backing_scale: f64 = msg_send![ns_window, backingScaleFactor];
    let regions = main_window_detached_footer_regions_appkit(
        content_bounds.size.width as f32,
        content_bounds.size.height as f32,
        footer_height() as f32,
        FLOAT_FOOTER_CONTAINER_GAP_PX,
        backing_scale as f32,
    );
    let footer_frame = NSRect::new(
        NSPoint::new(regions.footer.x as f64, regions.footer.y as f64),
        NSSize::new(regions.footer.width as f64, regions.footer.height as f64),
    );

    let existing = MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        let hosts = hosts.borrow();
        hosts.get(&(ns_window as usize)).map(|host| {
            let container = host.container();
            let inner = host.inner();
            let _: () = msg_send![container, setFrame: footer_frame];
            let _: () = msg_send![
                inner,
                setFrame: NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(footer_frame.size.width, footer_frame.size.height)
                )
            ];
            inner
        })
    });
    if let Some(root) = existing {
        return root;
    }

    let Some(host) = crate::platform::glass_button_host::install_native_glass_container(
        ns_window,
        gpui_view,
        footer_frame,
        crate::platform::glass_button_host::NativeViewOrdering::AboveGpui,
        crate::platform::glass_button_host::shared_glass_spacing(),
        FOOTER_GLASS_CONTAINER_ID,
    ) else {
        return nil;
    };
    let _: () = msg_send![host.container(), setAutoresizingMask: NSViewWidthSizable];
    let root = host.inner();
    MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        hosts.borrow_mut().insert(ns_window as usize, host);
    });
    root
}

#[cfg(target_os = "macos")]
unsafe fn remove_main_window_footer_glass_container(ns_window: id) {
    MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        hosts.borrow_mut().remove(&(ns_window as usize));
    });
}

/// Registry of the floating-footer window per parent (`(parent_ptr, footer_ptr)`).
///
/// The footer window is deliberately NOT attached via `addChildWindow:` —
/// attached children join the parent's window-server SHADOW GROUP, which puts
/// the capsule shapes back into the parent's shadow shape (the hairline
/// bridge between capsules, probe-proven). Ordering and visibility are
/// managed manually instead: frame/order in `sync_float_footer_child_frame`
/// (render-driven) and hide in the platform `orderOut:` choke points.
#[cfg(target_os = "macos")]
static FLOAT_FOOTER_WINDOWS: std::sync::Mutex<Vec<(usize, u64, usize)>> =
    std::sync::Mutex::new(Vec::new());

/// Find the floating-footer window registered for `ns_window`, if any.
#[cfg(target_os = "macos")]
unsafe fn float_footer_child_window(ns_window: id) -> id {
    let Some((_, binding, _)) = native_footer_binding(ns_window) else { return nil; };
    let guard = FLOAT_FOOTER_WINDOWS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard
        .iter()
        .find(|(parent, generation, _)| *parent == ns_window as usize && *generation == binding.host_generation)
        .map(|(_, _, footer)| *footer as id)
        .unwrap_or(nil)
}

/// Create (or reuse) the borderless, non-activating child panel that carries
/// the ENTIRE floating footer (host view: buttons, keycaps, left-info, and
/// their per-button glass capsules) below the main container.
///
/// Why a separate window:
/// - NSGlassEffectViews in the SAME window as the Tahoe backdrop auto-merge
///   with it across the 8px container gap (a full-width meniscus "shelf"
///   line bridging the capsules — user-reported).
/// - Any pixels left in the main window's strip (button text, keycaps) put
///   those rows back into the window-server shadow shape, which bridges them
///   into a rectangular rim around the strip.
///
/// Moving the whole footer out empties the strip completely: the main
/// window's shadow hugs the container, and the footer's glass samples the
/// desktop directly. The child has no shadow of its own.
#[cfg(target_os = "macos")]
unsafe fn ensure_float_footer_child_window(ns_window: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility).is_err() { return nil; }
    let Some((_, binding, _)) = native_footer_binding(ns_window) else { return nil; };
    let existing = float_footer_child_window(ns_window);
    if existing != nil {
        return existing;
    }

    let frame: NSRect = msg_send![ns_window, frame];
    let child_frame = NSRect::new(
        NSPoint::new(
            frame.origin.x,
            frame.origin.y - f64::from(main_window_float_footer_strip_height()),
        ),
        NSSize::new(frame.size.width, footer_height()),
    );
    let child: id = msg_send![class!(NSPanel), alloc];
    // styleMask 128 = borderless non-activating panel; backing 2 = buffered.
    let child: id = msg_send![
        child,
        initWithContentRect: child_frame
        styleMask: 128u64
        backing: 2u64
        defer: NO
    ];
    if child == nil {
        return nil;
    }
    let clear: id = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![child, setBackgroundColor: clear];
    let _: () = msg_send![child, setOpaque: NO];
    let _: () = msg_send![child, setHasShadow: NO];
    let _: () = msg_send![child, setReleasedWhenClosed: NO];
    let _: () = msg_send![child, setBecomesKeyOnlyIfNeeded: YES];
    let level: isize = msg_send![ns_window, level];
    let _: () = msg_send![child, setLevel: level];

    let content: id = msg_send![child, contentView];
    if content != nil {
        let identifier = ns_string(FLOAT_FOOTER_LAYER_ID);
        if identifier != nil {
            let _: () = msg_send![content, setIdentifier: identifier];
        }
        let _: () = msg_send![content, setWantsLayer: YES];
    }

    // Match the parent's Spaces/collection behavior so the footer follows the
    // launcher across Spaces and fullscreen setups.
    let collection_behavior: u64 = msg_send![ns_window, collectionBehavior];
    let _: () = msg_send![child, setCollectionBehavior: collection_behavior];

    FLOAT_FOOTER_WINDOWS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .push((ns_window as usize, binding.host_generation, child as usize));

    tracing::info!(
        target: "script_kit::footer_popup",
        event = "float_footer_child_window_installed",
        height = footer_height(),
        "Installed floating-footer window (unattached, shadow-group-free) below the main container"
    );
    child
}

/// Hide and unregister the floating-footer window, if present.
#[cfg(target_os = "macos")]
unsafe fn remove_float_footer_child_window(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};
    let generation = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).values()
        .find(|host| host.native_window == ns_window as usize).map(|host| host.host_generation);
    let Some(generation) = generation else { return; };
    let mut windows = FLOAT_FOOTER_WINDOWS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(index) = windows.iter().position(|(parent, lifetime, _)| *parent == ns_window as usize && *lifetime == generation) {
        let (_, _, child) = windows.remove(index);
        let child = child as id;
        let _: () = msg_send![child, orderOut: nil];
        let _: () = msg_send![child, close];
        let _: () = msg_send![child, release];
    }
}

/// Hide the floating-footer window alongside its parent (called from the
/// platform `orderOut:` choke points — the footer is unattached, so AppKit
/// will not hide it for us). Keeps the registration for the next show.
#[cfg(target_os = "macos")]
pub(crate) fn hide_float_footer_for_window(ns_window: id) {
    if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility).is_err() { return; }
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: called on the main thread from the platform hide paths; the
    // registered footer window pointer is retained for the process lifetime.
    unsafe {
        let child = float_footer_child_window(ns_window);
        if child != nil {
            let _: () = msg_send![child, orderOut: nil];
        }
    }
}

/// Keep the floating-footer window glued to the strip BELOW the main
/// window's frame (the frame ends at the container; the strip is outside it —
/// see `window_resize::physical_main_window_height`) and mirror the parent's
/// on-screen state (unattached window: manual ordering) and appearance (the
/// capsule glass must adapt to the same light/dark appearance as the main
/// window's backdrop, not the child's own resolved appearance).
#[cfg(target_os = "macos")]
unsafe fn sync_float_footer_child_frame(ns_window: id) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility).is_err() { return; }
    let child = float_footer_child_window(ns_window);
    if child == nil {
        return;
    }
    let main_frame: NSRect = msg_send![ns_window, frame];
    let strip = f64::from(main_window_float_footer_strip_height());
    let child_frame = NSRect::new(
        NSPoint::new(main_frame.origin.x, main_frame.origin.y - strip),
        NSSize::new(main_frame.size.width, footer_height()),
    );
    let _: () = msg_send![child, setFrame: child_frame display: YES];

    let parent_appearance: id = msg_send![ns_window, effectiveAppearance];
    if parent_appearance != nil {
        let _: () = msg_send![child, setAppearance: parent_appearance];
    }

    // Re-assert shadowlessness every pass: the capsule shapes otherwise get a
    // window-server shadow whose row spans bridge the gaps between capsules
    // into a hairline shelf (probe-proven).
    let child_has_shadow: cocoa::base::BOOL = msg_send![child, hasShadow];
    if child_has_shadow == YES {
        let _: () = msg_send![child, setHasShadow: NO];
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "float_footer_shadow_reasserted",
            "Floating footer window shadow was re-enabled by AppKit; disabled again"
        );
    }
    let _: () = msg_send![child, invalidateShadow];

    let parent_visible: cocoa::base::BOOL = msg_send![ns_window, isVisible];
    let child_visible: cocoa::base::BOOL = msg_send![child, isVisible];
    if parent_visible == YES {
        let parent_number: isize = msg_send![ns_window, windowNumber];
        let _: () = msg_send![child, orderWindow: 1isize relativeTo: parent_number];
    } else if child_visible == YES {
        let _: () = msg_send![child, orderOut: nil];
    }
}

/// Resolve the view that footer host lookups should search: the floating
/// child window's contentView when the float footer is active, otherwise the
/// main window's contentView (blur-era in-window host).
#[cfg(target_os = "macos")]
unsafe fn reusable_window_footer_search_root(ns_window: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    let child = float_footer_child_window(ns_window);
    if child != nil {
        let content: id = msg_send![child, contentView];
        if content != nil {
            return content;
        }
    }
    msg_send![ns_window, contentView]
}

/// Main-window footer lookup root. Tahoe uses the same-window glass
/// container's inner view; fallback mode keeps the existing in-content host.
#[cfg(target_os = "macos")]
unsafe fn main_window_footer_search_root(ns_window: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    let glass_root = main_window_footer_glass_root(ns_window);
    if glass_root != nil {
        glass_root
    } else {
        msg_send![ns_window, contentView]
    }
}

/// Debug aid (SCRIPT_KIT_GLASS_BAND_DEBUG=1): walk the contentView tree and
/// log every view whose frame intersects the transparent footer strip, with
/// visibility/alpha/layer state — used to find what still contributes pixels
/// (and therefore window-server shape) inside the strip.
#[cfg(target_os = "macos")]
unsafe fn log_strip_views_debug(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    if std::env::var("SCRIPT_KIT_GLASS_BAND_DEBUG").is_err() {
        return;
    }
    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }
    let strip = f64::from(main_window_float_footer_strip_height());
    if strip <= 0.0 {
        return;
    }

    unsafe fn walk(view: id, content_view: id, strip: f64, depth: usize, out: &mut Vec<String>) {
        use objc::{msg_send, sel, sel_impl};
        let subviews: id = msg_send![view, subviews];
        if subviews == nil {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for index in 0..count {
            let child: id = msg_send![subviews, objectAtIndex: index];
            if child == nil {
                continue;
            }
            let frame: cocoa::foundation::NSRect = msg_send![child, frame];
            let superview: id = msg_send![child, superview];
            let origin_in_content: cocoa::foundation::NSPoint = msg_send![
                content_view,
                convertPoint: frame.origin
                fromView: superview
            ];
            if origin_in_content.y < strip + 2.0 {
                let hidden: cocoa::base::BOOL = msg_send![child, isHidden];
                let alpha: f64 = msg_send![child, alphaValue];
                let cls: id = msg_send![child, class];
                let cls_name: id = msg_send![cls, className];
                let utf8: *const std::os::raw::c_char = msg_send![cls_name, UTF8String];
                let cls_name = if utf8.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(utf8)
                        .to_string_lossy()
                        .into_owned()
                };
                let layer: id = msg_send![child, layer];
                let layer_bg_alpha = if layer != nil {
                    let bg: *const std::ffi::c_void = msg_send![layer, backgroundColor];
                    if bg.is_null() {
                        -1.0
                    } else {
                        #[link(name = "CoreGraphics", kind = "framework")]
                        extern "C" {
                            fn CGColorGetAlpha(color: *const std::ffi::c_void) -> f64;
                        }
                        CGColorGetAlpha(bg)
                    }
                } else {
                    -2.0
                };
                out.push(format!(
                    "{}{} y={:.1} frame=({:.1},{:.1},{:.1},{:.1}) hidden={} alpha={:.4} layer_bg_alpha={:.4}",
                    "  ".repeat(depth),
                    cls_name,
                    origin_in_content.y,
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                    hidden == YES,
                    alpha,
                    layer_bg_alpha,
                ));
            }
            walk(child, content_view, strip, depth + 1, out);
        }
    }

    let mut out = Vec::new();
    walk(content_view, content_view, strip, 0, &mut out);
    let has_shadow: cocoa::base::BOOL = msg_send![ns_window, hasShadow];
    tracing::info!(
        target: "script_kit::footer_popup",
        event = "glass_strip_view_dump",
        window_has_shadow = has_shadow == YES,
        views = %out.join(" | "),
        "Views intersecting the transparent footer strip"
    );
}

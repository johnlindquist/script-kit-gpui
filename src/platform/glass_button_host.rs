//! Native per-button Liquid Glass hosted beneath a GPUI window's Metal view.

use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};

const DEFAULT_GLASS_SPACING: f64 = 8.0;

/// Owns a native Liquid Glass container and a reusable pool of effect views.
///
/// Frames passed to [`Self::sync`] use GPUI window coordinates (logical
/// pixels, y-down). AppKit content-view coordinates are points with y-up, so
/// only the y axis is flipped when placing each effect view.
pub(crate) struct GlassButtonHost {
    content_view: id,
    container: id,
    inner: id,
    glass_class: &'static Class,
    views: Vec<id>,
}

impl GlassButtonHost {
    /// Install a Liquid Glass container below the live GPUI Metal view.
    /// Returns `None` when the Tahoe-only AppKit classes or window are absent.
    pub(crate) fn install(window: &gpui::Window) -> Option<Self> {
        let glass_class = Class::get("NSGlassEffectView")?;
        let container_class = Class::get("NSGlassEffectContainerView")?;
        let nsview_class = Class::get("NSView")?;
        let (gpui_view, ns_window) = gpui_view_and_ns_window(window)?;
        let spacing = glass_spacing();

        // SAFETY: GPUI renders and prepaints on the AppKit main thread. The
        // raw handle supplies the live Metal NSView, and every class used here
        // was resolved at runtime before messaging it.
        unsafe {
            let content_view: id = msg_send![ns_window, contentView];
            if content_view == nil {
                return None;
            }
            let bounds: NSRect = msg_send![content_view, bounds];
            let resize_mask: u64 = (1 << 1) | (1 << 4); // width + height sizable

            let container: id = msg_send![container_class, alloc];
            let container: id = msg_send![container, initWithFrame: bounds];
            if container == nil {
                return None;
            }
            let _: () = msg_send![container, setAutoresizingMask: resize_mask];
            let _: () = msg_send![container, setSpacing: spacing];

            let inner: id = msg_send![nsview_class, alloc];
            let inner: id = msg_send![inner, initWithFrame: bounds];
            if inner == nil {
                let _: () = msg_send![container, release];
                return None;
            }
            let _: () = msg_send![inner, setAutoresizingMask: resize_mask];
            let _: () = msg_send![container, setContentView: inner];

            let below: i64 = -1;
            let _: () = msg_send![content_view, addSubview: container positioned: below relativeTo: gpui_view];

            tracing::info!(
                target: "script_kit::dictation",
                event = "dictation_glass_button_host_installed",
                spacing,
                "dictation_glass_button_host_installed"
            );

            Some(Self {
                content_view,
                container,
                inner,
                glass_class,
                views: Vec::new(),
            })
        }
    }

    /// Show one pooled glass view per frame and hide every unused view.
    pub(crate) fn sync(&mut self, frames: &[(f64, f64, f64, f64, f64)]) {
        while self.views.len() < frames.len() {
            // SAFETY: `glass_class` is the runtime-resolved Tahoe class and
            // `inner` remains retained for the lifetime of this host.
            let view = unsafe {
                let view: id = msg_send![self.glass_class, alloc];
                let view: id = msg_send![
                    view,
                    initWithFrame: NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        NSSize::new(10.0, 10.0)
                    )
                ];
                if view == nil {
                    break;
                }
                let _: () = msg_send![view, setHidden: YES];
                let _: () = msg_send![self.inner, addSubview: view];
                view
            };
            self.views.push(view);
        }

        // SAFETY: the content view and pooled glass views remain live while
        // the host is installed. Bounds are in AppKit points, equal to GPUI's
        // logical pixels for this coordinate conversion (no scale factor).
        unsafe {
            let content_bounds: NSRect = msg_send![self.content_view, bounds];
            let content_height = content_bounds.size.height;

            for (index, view) in self.views.iter().copied().enumerate() {
                let Some(&(x, y, width, height, radius)) = frames.get(index) else {
                    let _: () = msg_send![view, setHidden: YES];
                    continue;
                };
                let frame = NSRect::new(
                    NSPoint::new(x, content_height - y - height),
                    NSSize::new(width.max(1.0), height.max(1.0)),
                );
                let _: () = msg_send![view, setFrame: frame];
                let _: () = msg_send![view, setCornerRadius: radius];
                let _: () = msg_send![view, setHidden: NO];
            }
        }

        tracing::debug!(
            target: "script_kit::dictation",
            event = "dictation_glass_button_host_synced",
            visible_views = frames.len().min(self.views.len()),
            pooled_views = self.views.len(),
            "dictation_glass_button_host_synced"
        );
    }
}

impl Drop for GlassButtonHost {
    fn drop(&mut self) {
        // SAFETY: all objects were allocated and retained by this host on the
        // AppKit main thread. Removing the container first detaches the whole
        // native subtree before balancing our ownership retains.
        unsafe {
            let _: () = msg_send![self.container, removeFromSuperview];
            for view in self.views.drain(..) {
                let _: () = msg_send![view, release];
            }
            let _: () = msg_send![self.inner, release];
            let _: () = msg_send![self.container, release];
        }
        tracing::info!(
            target: "script_kit::dictation",
            event = "dictation_glass_button_host_torn_down",
            "dictation_glass_button_host_torn_down"
        );
    }
}

fn glass_spacing() -> f64 {
    std::env::var("SCRIPT_KIT_DICTATION_GLASS_SPACING")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(DEFAULT_GLASS_SPACING)
}

/// Mirrors `footer_popup::main_window_ns_window`, retaining the GPUI NSView
/// because it is also the relative sibling used for native view insertion.
fn gpui_view_and_ns_window(window: &gpui::Window) -> Option<(id, id)> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let gpui_view = appkit.ns_view.as_ptr() as id;

    // SAFETY: `gpui_view` comes from the live GPUI window raw handle on the
    // AppKit main thread. `-[NSView window]` returns its owning window or nil.
    unsafe {
        let ns_window: id = msg_send![gpui_view, window];
        (ns_window != nil).then_some((gpui_view, ns_window))
    }
}

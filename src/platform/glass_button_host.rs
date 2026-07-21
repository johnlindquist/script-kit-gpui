//! Native per-button Liquid Glass hosted beneath a GPUI window's Metal view.

use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use cocoa::quartzcore::CATransform3D;
use objc::rc::WeakPtr;
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

const DEFAULT_GLASS_SPACING: f64 = 4.0;
const DEFAULT_GLASS_HOVER_SCALE: f64 = 1.05;
const GLASS_HOVER_DURATION_SECONDS: f64 = 0.14;

pub(crate) type GlassButtonFrame = (f64, f64, f64, f64, f64);

thread_local! {
    static HOSTS_BY_WINDOW: RefCell<HashMap<usize, WindowGlassState>> =
        RefCell::new(HashMap::new());
}

struct WindowGlassState {
    host: GlassButtonHost,
    groups: BTreeMap<&'static str, Vec<GlassButtonFrame>>,
    hovered: BTreeMap<(&'static str, usize), bool>,
}

impl WindowGlassState {
    fn sync_host(&mut self) {
        let (frames, hovered) = flatten_groups(&self.groups, &self.hovered);
        self.host.sync(&frames, &hovered);
    }

    fn flat_index(&self, target_group: &'static str, target_index: usize) -> Option<usize> {
        let mut offset = 0;
        for (group, frames) in &self.groups {
            if *group == target_group {
                return (target_index < frames.len()).then_some(offset + target_index);
            }
            offset += frames.len();
        }
        None
    }
}

fn flatten_groups(
    groups: &BTreeMap<&'static str, Vec<GlassButtonFrame>>,
    hovered_by_group: &BTreeMap<(&'static str, usize), bool>,
) -> (Vec<GlassButtonFrame>, Vec<bool>) {
    let mut frames = Vec::new();
    let mut hovered = Vec::new();
    for (group, group_frames) in groups {
        for (index, frame) in group_frames.iter().copied().enumerate() {
            frames.push(frame);
            hovered.push(
                hovered_by_group
                    .get(&(*group, index))
                    .copied()
                    .unwrap_or(false),
            );
        }
    }
    (frames, hovered)
}

pub(crate) fn glass_buttons_enabled() -> bool {
    crate::platform::tahoe_liquid_glass_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled()
        && std::env::var("SCRIPT_KIT_GLASS_BUTTONS")
            .map(|value| value != "0")
            .unwrap_or(true)
}

/// Sync one named capsule group in a live GPUI window. Groups are merged in
/// lexical order before the native pool is updated, so independently rendered
/// header/footer regions cannot clobber one another. Empty frames hide only
/// this group without eagerly installing a host.
pub(crate) fn sync_for_window(
    window: &gpui::Window,
    group: &'static str,
    frames: &[GlassButtonFrame],
) {
    let Some((_, ns_window)) = gpui_view_and_ns_window(window) else {
        return;
    };
    let window_key = ns_window as usize;

    HOSTS_BY_WINDOW.with(|registry| {
        let mut registry = registry.borrow_mut();
        registry.retain(|_, state| state.host.window_is_alive());

        if frames.is_empty() {
            if let Some(state) = registry.get_mut(&window_key) {
                state.groups.insert(group, Vec::new());
                state
                    .hovered
                    .retain(|(owned_group, _), _| *owned_group != group);
                state.sync_host();
                tracing::debug!(
                    target: "script_kit::glass_buttons",
                    event = "glass_button_group_synced",
                    window_key,
                    group,
                    group_frames = 0,
                    groups = state.groups.len(),
                    "glass_button_group_synced"
                );
            }
            return;
        }

        if !registry.contains_key(&window_key) {
            let Some(host) = GlassButtonHost::install(window) else {
                return;
            };
            registry.insert(
                window_key,
                WindowGlassState {
                    host,
                    groups: BTreeMap::new(),
                    hovered: BTreeMap::new(),
                },
            );
        }
        if let Some(state) = registry.get_mut(&window_key) {
            state.groups.insert(group, frames.to_vec());
            state
                .hovered
                .retain(|(owned_group, index), _| *owned_group != group || *index < frames.len());
            state.sync_host();
            tracing::debug!(
                target: "script_kit::glass_buttons",
                event = "glass_button_group_synced",
                window_key,
                group,
                group_frames = frames.len(),
                groups = state.groups.len(),
                "glass_button_group_synced"
            );
        }
    });
}

/// Remove one conditional group and immediately re-sync all remaining groups.
pub(crate) fn remove_group(window: &gpui::Window, group: &'static str) {
    let Some((_, ns_window)) = gpui_view_and_ns_window(window) else {
        return;
    };
    HOSTS_BY_WINDOW.with(|registry| {
        if let Some(state) = registry.borrow_mut().get_mut(&(ns_window as usize)) {
            state.groups.remove(group);
            state
                .hovered
                .retain(|(owned_group, _), _| *owned_group != group);
            state.sync_host();
        }
    });
}

/// Animate one logical capsule without changing its layout-derived frame.
pub(crate) fn set_hover(window: &gpui::Window, group: &'static str, index: usize, hovered: bool) {
    let Some((_, ns_window)) = gpui_view_and_ns_window(window) else {
        return;
    };
    HOSTS_BY_WINDOW.with(|registry| {
        let mut registry = registry.borrow_mut();
        let Some(state) = registry.get_mut(&(ns_window as usize)) else {
            return;
        };
        let Some(flat_index) = state.flat_index(group, index) else {
            return;
        };
        let previous = state
            .hovered
            .insert((group, index), hovered)
            .unwrap_or(false);
        if previous == hovered {
            return;
        }
        state.host.set_hover(flat_index, hovered);
        tracing::debug!(
            target: "script_kit::glass_buttons",
            event = "glass_button_hover_animated",
            window_key = ns_window as usize,
            group,
            index,
            hovered,
            scale = glass_hover_scale(),
            "glass_button_hover_animated"
        );
    });
}

/// Remove the host for a window before its native handle is retired.
pub(crate) fn remove_for_window(window: &gpui::Window) {
    let Some((_, ns_window)) = gpui_view_and_ns_window(window) else {
        return;
    };
    HOSTS_BY_WINDOW.with(|registry| {
        registry.borrow_mut().remove(&(ns_window as usize));
    });
}

/// Owns a native Liquid Glass container and a reusable pool of effect views.
///
/// Frames passed to [`Self::sync`] use GPUI window coordinates (logical
/// pixels, y-down). AppKit content-view coordinates are points with y-up, so
/// only the y axis is flipped when placing each effect view.
pub(crate) struct GlassButtonHost {
    window_key: usize,
    window: WeakPtr,
    content_view: id,
    container: id,
    inner: id,
    glass_class: &'static Class,
    views: Vec<id>,
    hovered: Vec<bool>,
}

impl GlassButtonHost {
    /// Install a Liquid Glass container below the live GPUI Metal view.
    /// Returns `None` when the Tahoe-only AppKit classes or window are absent.
    pub(crate) fn install(window: &gpui::Window) -> Option<Self> {
        let glass_class = Class::get("NSGlassEffectView")?;
        let container_class = Class::get("NSGlassEffectContainerView")?;
        let nsview_class = Class::get("NSView")?;
        let (gpui_view, ns_window) = gpui_view_and_ns_window(window)?;
        let window_key = ns_window as usize;
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
                window_key,
                spacing,
                "dictation_glass_button_host_installed"
            );

            Some(Self {
                window_key,
                window: WeakPtr::new(ns_window),
                content_view,
                container,
                inner,
                glass_class,
                views: Vec::new(),
                hovered: Vec::new(),
            })
        }
    }

    /// Show one pooled glass view per frame and hide every unused view.
    fn sync(&mut self, frames: &[GlassButtonFrame], hovered: &[bool]) {
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
                let _: () = msg_send![view, setWantsLayer: YES];
                let _: () = msg_send![view, setHidden: YES];
                let _: () = msg_send![self.inner, addSubview: view];
                view
            };
            self.views.push(view);
            self.hovered.push(false);
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
                    if self.hovered.get(index).copied().unwrap_or(false) {
                        set_glass_view_scale(view, 1.0, false);
                        self.hovered[index] = false;
                    }
                    continue;
                };
                let frame = NSRect::new(
                    NSPoint::new(x, content_height - y - height),
                    NSSize::new(width.max(1.0), height.max(1.0)),
                );
                let _: () = msg_send![view, setFrame: frame];
                let _: () = msg_send![view, setCornerRadius: radius];
                let _: () = msg_send![view, setHidden: NO];
                let desired_hover = hovered.get(index).copied().unwrap_or(false);
                if self.hovered[index] != desired_hover {
                    set_glass_view_scale(
                        view,
                        if desired_hover {
                            glass_hover_scale()
                        } else {
                            1.0
                        },
                        false,
                    );
                    self.hovered[index] = desired_hover;
                }
            }
        }

        tracing::debug!(
            target: "script_kit::dictation",
            event = "dictation_glass_button_host_synced",
            window_key = self.window_key,
            visible_views = frames.len().min(self.views.len()),
            pooled_views = self.views.len(),
            "dictation_glass_button_host_synced"
        );
    }

    fn window_is_alive(&self) -> bool {
        !(*self.window.load()).is_null()
    }

    fn set_hover(&mut self, index: usize, hovered: bool) {
        let Some(view) = self.views.get(index).copied() else {
            return;
        };
        if self.hovered.get(index).copied() == Some(hovered) {
            return;
        }
        let scale = if hovered { glass_hover_scale() } else { 1.0 };
        unsafe { set_glass_view_scale(view, scale, true) };
        self.hovered[index] = hovered;
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
            window_key = self.window_key,
            "dictation_glass_button_host_torn_down"
        );
    }
}

fn glass_spacing() -> f64 {
    std::env::var("SCRIPT_KIT_GLASS_BUTTON_SPACING")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(DEFAULT_GLASS_SPACING)
}

fn glass_hover_scale() -> f64 {
    std::env::var("SCRIPT_KIT_GLASS_HOVER_SCALE")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 1.0)
        .map(|value| value.min(1.2))
        .unwrap_or(DEFAULT_GLASS_HOVER_SCALE)
}

/// Scale only the backing layer, leaving `setFrame:` as the permanent layout
/// authority. The current presentation value becomes the basic animation's
/// start, so fast enter/exit reversals remain smooth.
unsafe fn set_glass_view_scale(view: id, scale: f64, animated: bool) {
    use cocoa::foundation::NSString;

    let layer: id = msg_send![view, layer];
    if layer == nil {
        return;
    }
    let Some(transaction_class) = Class::get("CATransaction") else {
        return;
    };
    let transform = CATransform3D::from_scale(scale, scale, 1.0);

    // Set the model value with actions disabled. A named CABasicAnimation
    // below owns the visual transition and can start from the presentation
    // layer during rapid hover reversals.
    let _: () = msg_send![transaction_class, begin];
    let _: () = msg_send![transaction_class, setDisableActions: YES];
    let _: () = msg_send![layer, setTransform: transform];
    let _: () = msg_send![transaction_class, commit];

    if !animated || glass_hover_scale() <= 1.0 {
        return;
    }
    let (Some(animation_class), Some(number_class)) =
        (Class::get("CABasicAnimation"), Class::get("NSNumber"))
    else {
        return;
    };
    let key_path = NSString::alloc(nil).init_str("transform.scale");
    let animation: id = msg_send![animation_class, animationWithKeyPath: key_path];
    if animation == nil {
        let _: () = msg_send![key_path, release];
        return;
    }
    let presentation: id = msg_send![layer, presentationLayer];
    let value_source = if presentation == nil {
        layer
    } else {
        presentation
    };
    let from_value: id = msg_send![value_source, valueForKeyPath: key_path];
    let to_value: id = msg_send![number_class, numberWithDouble: scale];
    if from_value != nil {
        let _: () = msg_send![animation, setFromValue: from_value];
    }
    let _: () = msg_send![animation, setToValue: to_value];
    let _: () = msg_send![animation, setDuration: GLASS_HOVER_DURATION_SECONDS];
    if let Some(timing_class) = Class::get("CAMediaTimingFunction") {
        let timing_name = NSString::alloc(nil).init_str("easeOut");
        let timing: id = msg_send![timing_class, functionWithName: timing_name];
        let _: () = msg_send![timing_name, release];
        if timing != nil {
            let _: () = msg_send![animation, setTimingFunction: timing];
        }
    }
    let animation_key = NSString::alloc(nil).init_str("scriptKitGlassHoverScale");
    let _: () = msg_send![layer, addAnimation: animation forKey: animation_key];
    let _: () = msg_send![animation_key, release];
    let _: () = msg_send![key_path, release];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_frames_flatten_in_stable_name_order_and_preserve_hover_identity() {
        let footer = (20.0, 30.0, 40.0, 18.0, 6.0);
        let header_first = (1.0, 2.0, 10.0, 12.0, 8.0);
        let header_second = (12.0, 2.0, 10.0, 12.0, 8.0);
        let mut groups = BTreeMap::new();
        groups.insert("footer", vec![footer]);
        groups.insert("header", vec![header_first, header_second]);
        let hovered = BTreeMap::from([(("header", 1), true)]);

        let (frames, hovered) = flatten_groups(&groups, &hovered);

        assert_eq!(frames, vec![footer, header_first, header_second]);
        assert_eq!(hovered, vec![false, false, true]);
    }

    #[test]
    fn empty_group_hides_only_itself() {
        let header = (1.0, 2.0, 10.0, 12.0, 8.0);
        let mut groups = BTreeMap::new();
        groups.insert("footer", Vec::new());
        groups.insert("header", vec![header]);

        let (frames, hovered) = flatten_groups(&groups, &BTreeMap::new());

        assert_eq!(frames, vec![header]);
        assert_eq!(hovered, vec![false]);
    }
}

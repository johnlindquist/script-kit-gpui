//! Native per-button Liquid Glass hosted beneath a GPUI window's Metal view.

use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use cocoa::quartzcore::CATransform3D;
use objc::rc::WeakPtr;
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

const DEFAULT_GLASS_SPACING: f64 = 4.0;
const DEFAULT_GLASS_HOVER_SCALE: f64 = 1.12;
const GLASS_HOVER_DURATION_SECONDS: f64 = 0.18;
/// A capsule group that hasn't re-synced within this window is unmounted.
///
/// GPUI redraws the whole window per frame, so every mounted rail re-syncs
/// its group on every draw; a group whose stamp lags the newest sync by more
/// than this TTL belongs to an element that stopped rendering (view switch,
/// rail swap) and must be dropped — otherwise its capsules linger at stale
/// positions and overlap the new surface's buttons.
const GLASS_GROUP_STALE_TTL: std::time::Duration = std::time::Duration::from_millis(250);

pub(crate) type GlassButtonFrame = (f64, f64, f64, f64, f64);

thread_local! {
    static HOSTS_BY_WINDOW: RefCell<HashMap<usize, WindowGlassState>> =
        RefCell::new(HashMap::new());
}

struct GlassGroup {
    frames: Vec<GlassButtonFrame>,
    last_synced: std::time::Instant,
}

struct WindowGlassState {
    host: GlassButtonHost,
    groups: BTreeMap<&'static str, GlassGroup>,
    hovered: BTreeMap<(&'static str, usize), bool>,
}

impl WindowGlassState {
    fn sync_host(&mut self) {
        let (frames, hovered) = flatten_groups(&self.groups, &self.hovered);
        self.host.sync(&frames, &hovered);
    }

    fn flat_index(&self, target_group: &'static str, target_index: usize) -> Option<usize> {
        let mut offset = 0;
        for (group, glass_group) in &self.groups {
            if *group == target_group {
                return (target_index < glass_group.frames.len()).then_some(offset + target_index);
            }
            offset += glass_group.frames.len();
        }
        None
    }

    /// Drop groups whose elements stopped syncing (see [`GLASS_GROUP_STALE_TTL`]).
    fn prune_stale_groups(&mut self, now: std::time::Instant) -> usize {
        let before = self.groups.len();
        self.groups
            .retain(|_, group| now.duration_since(group.last_synced) <= GLASS_GROUP_STALE_TTL);
        let pruned = before - self.groups.len();
        if pruned > 0 {
            let live: Vec<&'static str> = self.groups.keys().copied().collect();
            self.hovered
                .retain(|(owned_group, _), _| live.contains(owned_group));
        }
        pruned
    }
}

fn flatten_groups(
    groups: &BTreeMap<&'static str, GlassGroup>,
    hovered_by_group: &BTreeMap<(&'static str, usize), bool>,
) -> (Vec<GlassButtonFrame>, Vec<bool>) {
    let mut frames = Vec::new();
    let mut hovered = Vec::new();
    for (group, glass_group) in groups {
        for (index, frame) in glass_group.frames.iter().copied().enumerate() {
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
    crate::platform::tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled()
        && std::env::var("SCRIPT_KIT_GLASS_BUTTONS")
            .map(|value| value != "0")
            .unwrap_or(true)
}

/// Whether AppKit can host a native Liquid Glass container in this process.
///
/// The detached main footer requires both the glass effect view (checked by
/// the platform gate) and its container class. Keeping this as a separate
/// capability check lets fallback mode avoid reserving an empty footer strip
/// on systems where only part of the Tahoe API surface is present.
pub(crate) fn native_glass_container_available() -> bool {
    Class::get("NSGlassEffectContainerView").is_some()
}

/// Relative placement for a native glass container around GPUI's Metal view.
/// The existing button host remains below GPUI; the detached main footer will
/// use the above-GPUI variant when its topology migrates in MWND-03.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeViewOrdering {
    BelowGpui,
    AboveGpui,
}

impl NativeViewOrdering {
    fn appkit_value(self) -> i64 {
        match self {
            Self::BelowGpui => -1,
            Self::AboveGpui => 1,
        }
    }
}

/// Reusable ownership wrapper for an `NSGlassEffectContainerView` and its
/// content view. It contains only native installation/lifetime mechanics;
/// feature-specific glass children remain owned by the caller.
pub(crate) struct NativeGlassContainerHost {
    window: WeakPtr,
    content_view: id,
    container: id,
    inner: id,
}

impl NativeGlassContainerHost {
    pub(crate) fn window_is_alive(&self) -> bool {
        !(*self.window.load()).is_null()
    }

    pub(crate) fn content_view(&self) -> id {
        self.content_view
    }

    pub(crate) fn container(&self) -> id {
        self.container
    }

    pub(crate) fn inner(&self) -> id {
        self.inner
    }
}

impl Drop for NativeGlassContainerHost {
    fn drop(&mut self) {
        // SAFETY: the host owns both allocated views and is dropped on the
        // AppKit main thread by its feature-specific owner.
        unsafe {
            let _: () = msg_send![self.container, removeFromSuperview];
            let _: () = msg_send![self.inner, release];
            let _: () = msg_send![self.container, release];
        }
    }
}

/// Install one native glass container relative to GPUI's live Metal view.
/// Returns `None` when Tahoe's glass class or either required native view is
/// unavailable. The returned host owns the container and its inner view.
pub(crate) unsafe fn install_native_glass_container(
    ns_window: id,
    gpui_view: id,
    frame: NSRect,
    ordering: NativeViewOrdering,
    spacing: f64,
    identifier: &str,
) -> Option<NativeGlassContainerHost> {
    let container_class = Class::get("NSGlassEffectContainerView")?;
    let nsview_class = Class::get("NSView")?;
    if ns_window == nil || gpui_view == nil {
        return None;
    }
    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return None;
    }
    let resize_mask: u64 = (1 << 1) | (1 << 4); // width + height sizable

    let container: id = msg_send![container_class, alloc];
    let container: id = msg_send![container, initWithFrame: frame];
    if container == nil {
        return None;
    }
    let _: () = msg_send![container, setAutoresizingMask: resize_mask];
    let _: () = msg_send![container, setSpacing: spacing.max(0.0)];
    if !identifier.is_empty() {
        let identifier = NSString::alloc(nil).init_str(identifier);
        let _: () = msg_send![container, setIdentifier: identifier];
        let _: () = msg_send![identifier, release];
    }

    let inner_frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(frame.size.width, frame.size.height),
    );
    let inner: id = msg_send![nsview_class, alloc];
    let inner: id = msg_send![inner, initWithFrame: inner_frame];
    if inner == nil {
        let _: () = msg_send![container, release];
        return None;
    }
    let _: () = msg_send![inner, setAutoresizingMask: resize_mask];
    let _: () = msg_send![container, setContentView: inner];

    let _: () = msg_send![
        content_view,
        addSubview: container
        positioned: ordering.appkit_value()
        relativeTo: gpui_view
    ];

    Some(NativeGlassContainerHost {
        window: WeakPtr::new(ns_window),
        content_view,
        container,
        inner,
    })
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

        let now = std::time::Instant::now();

        if frames.is_empty() {
            if let Some(state) = registry.get_mut(&window_key) {
                state.groups.insert(
                    group,
                    GlassGroup {
                        frames: Vec::new(),
                        last_synced: now,
                    },
                );
                state
                    .hovered
                    .retain(|(owned_group, _), _| *owned_group != group);
                state.prune_stale_groups(now);
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

        if let std::collections::hash_map::Entry::Vacant(e) = registry.entry(window_key) {
            let Some(host) = GlassButtonHost::install(window) else {
                return;
            };
            e.insert(WindowGlassState {
                host,
                groups: BTreeMap::new(),
                hovered: BTreeMap::new(),
            });
        }
        if let Some(state) = registry.get_mut(&window_key) {
            state.groups.insert(
                group,
                GlassGroup {
                    frames: frames.to_vec(),
                    last_synced: now,
                },
            );
            state
                .hovered
                .retain(|(owned_group, index), _| *owned_group != group || *index < frames.len());
            let pruned = state.prune_stale_groups(now);
            state.sync_host();
            tracing::debug!(
                target: "script_kit::glass_buttons",
                event = "glass_button_group_synced",
                window_key,
                group,
                group_frames = frames.len(),
                groups = state.groups.len(),
                pruned_stale_groups = pruned,
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
    native: NativeGlassContainerHost,
    glass_class: &'static Class,
    views: Vec<id>,
    hovered: Vec<bool>,
}

impl GlassButtonHost {
    /// Install a Liquid Glass container below the live GPUI Metal view.
    /// Returns `None` when the Tahoe-only AppKit classes or window are absent.
    pub(crate) fn install(window: &gpui::Window) -> Option<Self> {
        let glass_class = Class::get("NSGlassEffectView")?;
        let (gpui_view, ns_window) = gpui_view_and_ns_window(window)?;
        let window_key = ns_window as usize;
        let spacing = shared_glass_spacing();

        // SAFETY: GPUI renders and prepaints on the AppKit main thread. The
        // raw handle supplies the live Metal NSView, and every class used here
        // was resolved at runtime before messaging it.
        unsafe {
            let content_view: id = msg_send![ns_window, contentView];
            if content_view == nil {
                return None;
            }
            let bounds: NSRect = msg_send![content_view, bounds];
            let native = install_native_glass_container(
                ns_window,
                gpui_view,
                bounds,
                NativeViewOrdering::BelowGpui,
                spacing,
                "script-kit-glass-button-container",
            )?;

            tracing::info!(
                target: "script_kit::dictation",
                event = "dictation_glass_button_host_installed",
                window_key,
                spacing,
                "dictation_glass_button_host_installed"
            );

            Some(Self {
                window_key,
                native,
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
                let _: () = msg_send![self.native.inner(), addSubview: view];
                view
            };
            self.views.push(view);
            self.hovered.push(false);
        }

        // SAFETY: the content view and pooled glass views remain live while
        // the host is installed. Bounds are in AppKit points, equal to GPUI's
        // logical pixels for this coordinate conversion (no scale factor).
        unsafe {
            let content_bounds: NSRect = msg_send![self.native.content_view(), bounds];
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
        self.native.window_is_alive()
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
        // SAFETY: pooled views are owned by this host. The native container
        // drops immediately after this method and detaches/releases its tree.
        unsafe {
            for view in self.views.drain(..) {
                let _: () = msg_send![view, release];
            }
        }
        tracing::info!(
            target: "script_kit::dictation",
            event = "dictation_glass_button_host_torn_down",
            window_key = self.window_key,
            "dictation_glass_button_host_torn_down"
        );
    }
}

pub(crate) fn shared_glass_spacing() -> f64 {
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

    fn group(frames: Vec<GlassButtonFrame>) -> GlassGroup {
        GlassGroup {
            frames,
            last_synced: std::time::Instant::now(),
        }
    }

    #[test]
    fn grouped_frames_flatten_in_stable_name_order_and_preserve_hover_identity() {
        let footer = (20.0, 30.0, 40.0, 18.0, 6.0);
        let header_first = (1.0, 2.0, 10.0, 12.0, 8.0);
        let header_second = (12.0, 2.0, 10.0, 12.0, 8.0);
        let mut groups = BTreeMap::new();
        groups.insert("footer", group(vec![footer]));
        groups.insert("header", group(vec![header_first, header_second]));
        let hovered = BTreeMap::from([(("header", 1), true)]);

        let (frames, hovered) = flatten_groups(&groups, &hovered);

        assert_eq!(frames, vec![footer, header_first, header_second]);
        assert_eq!(hovered, vec![false, false, true]);
    }

    #[test]
    fn empty_group_hides_only_itself() {
        let header = (1.0, 2.0, 10.0, 12.0, 8.0);
        let mut groups = BTreeMap::new();
        groups.insert("footer", group(Vec::new()));
        groups.insert("header", group(vec![header]));

        let (frames, hovered) = flatten_groups(&groups, &BTreeMap::new());

        assert_eq!(frames, vec![header]);
        assert_eq!(hovered, vec![false]);
    }

    #[test]
    fn shared_native_container_install_options_preserve_existing_below_gpui_mode() {
        assert_eq!(NativeViewOrdering::BelowGpui.appkit_value(), -1);
        assert_eq!(NativeViewOrdering::AboveGpui.appkit_value(), 1);
        assert!(shared_glass_spacing().is_finite());
        assert!(shared_glass_spacing() >= 0.0);
    }

    /// A group that stops syncing (its element unmounted on a view switch)
    /// must be dropped once it exceeds the stale TTL — lingering frames
    /// overlap the next surface's capsules.
    #[test]
    fn stale_groups_prune_after_ttl_and_drop_their_hover_state() {
        let now = std::time::Instant::now();
        let stale_stamp = now - (GLASS_GROUP_STALE_TTL + std::time::Duration::from_millis(50));
        let mut groups = BTreeMap::new();
        groups.insert(
            "old-rail",
            GlassGroup {
                frames: vec![(1.0, 2.0, 10.0, 12.0, 8.0)],
                last_synced: stale_stamp,
            },
        );
        groups.insert("footer", group(vec![(20.0, 30.0, 40.0, 18.0, 6.0)]));

        let mut hovered = BTreeMap::new();
        hovered.insert(("old-rail", 0usize), true);

        // prune_stale_groups lives on WindowGlassState, which needs a live
        // NSWindow; exercise the same retain predicate directly.
        let before = groups.len();
        groups.retain(|_, group: &mut GlassGroup| {
            now.duration_since(group.last_synced) <= GLASS_GROUP_STALE_TTL
        });
        let live: Vec<&'static str> = groups.keys().copied().collect();
        hovered.retain(|(owned_group, _), _| live.contains(owned_group));

        assert_eq!(before - groups.len(), 1);
        assert!(groups.contains_key("footer"));
        assert!(!groups.contains_key("old-rail"));
        assert!(hovered.is_empty());
    }
}

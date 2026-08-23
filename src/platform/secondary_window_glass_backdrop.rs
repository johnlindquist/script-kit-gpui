/// Stable `tag` sentinel so the backdrop view can be found idempotently via
/// `contentView.viewWithTag:` on repeated configure passes.
#[cfg(target_os = "macos")]
const TAHOE_GLASS_BACKDROP_TAG: isize = 0x5c17_0175;
/// Accessibility/debug identifier for the native glass backdrop view.
#[cfg(target_os = "macos")]
pub(crate) const TAHOE_GLASS_BACKDROP_IDENTIFIER: &str = "script-kit-tahoe-glass-backdrop";
/// `NSWindowBelow` ordering constant for `addSubview:positioned:relativeTo:`.
#[cfg(target_os = "macos")]
const NS_WINDOW_BELOW: isize = -1;

/// Native backdrop partition. Windows with floating footers leave the footer
/// and desktop gutter outside the material frame while retaining one physical
/// NSWindow, so their controls translate atomically with the content.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum TahoeBackdropLayout {
    FullWindow,
    ContentAboveDetachedFooter { bottom_inset: f64 },
}

#[cfg(target_os = "macos")]
impl TahoeBackdropLayout {
    fn frame(self, bounds: cocoa::foundation::NSRect) -> cocoa::foundation::NSRect {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        match self {
            Self::FullWindow => bounds,
            Self::ContentAboveDetachedFooter { bottom_inset } => {
                let bottom_inset = bottom_inset.clamp(0.0, bounds.size.height.max(0.0));
                NSRect::new(
                    NSPoint::new(bounds.origin.x, bounds.origin.y + bottom_inset),
                    NSSize::new(
                        bounds.size.width,
                        (bounds.size.height - bottom_inset).max(0.0),
                    ),
                )
            }
        }
    }

    fn bottom_inset(self) -> f64 {
        match self {
            Self::FullWindow => 0.0,
            Self::ContentAboveDetachedFooter { bottom_inset } => bottom_inset.max(0.0),
        }
    }

    fn is_detached_footer(self) -> bool {
        matches!(self, Self::ContentAboveDetachedFooter { .. })
    }
}

/// Windows whose calibrated glass EXIT is the fixed-frame fade
/// (`GlassExitMode::DetachedRegionsFadeOnly`). This is a motion-calibration
/// contract and deliberately independent of the backdrop layout below: Notes
/// keeps its calibrated fade-only exit in BOTH surface modes, even while its
/// backdrop is full-window (no footer band).
#[cfg(target_os = "macos")]
fn window_name_uses_fixed_frame_exit(window_name: &str) -> bool {
    matches!(window_name, "Main window" | "Notes" | "Dictation overlay")
}

/// Windows whose DEFAULT backdrop reserves the detached-footer band. Notes is
/// intentionally absent: its default (Notes editor mode) is a full-window
/// backdrop; Agent mode opts into the band dynamically via
/// [`set_gpui_window_backdrop_bottom_inset`].
#[cfg(target_os = "macos")]
fn window_name_owns_detached_footer(window_name: &str) -> bool {
    matches!(window_name, "Main window" | "Dictation overlay")
}

#[cfg(target_os = "macos")]
fn tahoe_backdrop_layout(window_name: &str) -> TahoeBackdropLayout {
    let bottom_inset = f64::from(crate::footer_popup::main_window_float_footer_strip_height());
    if window_name_owns_detached_footer(window_name) && bottom_inset > 0.0 {
        TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset }
    } else {
        TahoeBackdropLayout::FullWindow
    }
}

/// Self-heal after the appear morph: window content can resize while the
/// frame animation is in flight (the launcher sizes to its list on show),
/// leaving the glass stuck at stale bounds. Scheduled via
/// `performSelector:afterDelay:` right after the animation ends.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_repin(this: &objc::runtime::Object, _: objc::runtime::Sel) {
    // SAFETY: main thread (performSelector on the main run loop); standard
    // superview/bounds/frame accessors.
    unsafe {
        let this_id = this as *const objc::runtime::Object as id;
        let superview: id = msg_send![this_id, superview];
        if superview == nil {
            return;
        }
        let bounds: cocoa::foundation::NSRect = msg_send![superview, bounds];
        let bottom_inset = *this.get_ivar::<f64>("_scriptKitBottomInset");
        let frame = TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset }.frame(bounds);
        let _: () = msg_send![this_id, setFrame: frame];
    }
}

/// `NSView.tag` is read-only, so the subclass overrides it to return the
/// stable sentinel, enabling idempotent `viewWithTag:` lookup.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_tag(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) -> isize {
    TAHOE_GLASS_BACKDROP_TAG
}

/// Lazily register a dedicated `NSGlassEffectView` subclass that is pass-through
/// for hit testing and reports the stable tag. Superclass is resolved at runtime
/// from `NSGlassEffectView` (macOS 26 Tahoe); returns `None` if unavailable.
#[cfg(target_os = "macos")]
fn tahoe_glass_backdrop_view_class(glass_class: id) -> Option<*const objc::runtime::Class> {
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use std::sync::OnceLock;

    static CLASS: OnceLock<usize> = OnceLock::new();
    let ptr = *CLASS.get_or_init(|| unsafe {
        if glass_class.is_null() {
            return 0;
        }
        if let Some(existing) = Class::get("ScriptKitTahoeGlassBackdropView") {
            return existing as *const Class as usize;
        }
        // SAFETY: `glass_class` came from NSClassFromString("NSGlassEffectView");
        // it is a valid ObjC Class pointer usable as a ClassDecl superclass.
        let superclass = &*(glass_class as *const Class);
        let Some(mut decl) = ClassDecl::new("ScriptKitTahoeGlassBackdropView", superclass) else {
            return Class::get("ScriptKitTahoeGlassBackdropView")
                .map(|class| class as *const Class as usize)
                .unwrap_or(0);
        };
        decl.add_ivar::<f64>("_scriptKitBottomInset");
        decl.add_method(
            sel!(hitTest:),
            tahoe_glass_backdrop_hit_test
                as extern "C" fn(&Object, Sel, cocoa::foundation::NSPoint) -> id,
        );
        decl.add_method(
            sel!(tag),
            tahoe_glass_backdrop_tag as extern "C" fn(&Object, Sel) -> isize,
        );
        decl.add_method(
            sel!(repinToSuperviewBounds),
            tahoe_glass_backdrop_repin as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(settleOwnWindowFrame),
            tahoe_glass_backdrop_settle as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(revealOwnWindowEntryContent),
            tahoe_glass_backdrop_reveal_entry_content as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(beginOwnWindowEntryTail),
            tahoe_glass_backdrop_begin_entry_tail as extern "C" fn(&Object, Sel),
        );
        decl.register() as *const Class as usize
    });
    if ptr == 0 {
        None
    } else {
        Some(ptr as *const objc::runtime::Class)
    }
}

/// Read the content view's layer corner radius (0.0 when no backing layer).
#[cfg(target_os = "macos")]
unsafe fn tahoe_content_corner_radius(content_view: id) -> f64 {
    if content_view == nil {
        return 0.0;
    }
    let layer: id = msg_send![content_view, layer];
    if layer == nil {
        return 0.0;
    }
    msg_send![layer, cornerRadius]
}

/// Recursively count `isKindOfClass:` matches under `view`, skipping the
/// `excluded_subtree` root (used to count NSVisualEffectViews while excluding
/// the glass backdrop itself for the footer non-regression audit).
#[cfg(target_os = "macos")]
unsafe fn tahoe_count_views_kind_of_excluding(
    view: id,
    class_id: *const objc::runtime::Class,
    excluded_subtree: id,
) -> usize {
    if view == nil || view == excluded_subtree {
        return 0;
    }
    let is_kind: bool = msg_send![view, isKindOfClass: class_id];
    let mut count = usize::from(is_kind);
    let subviews: id = msg_send![view, subviews];
    if subviews != nil {
        let subview_count: usize = msg_send![subviews, count];
        for index in 0..subview_count {
            let child: id = msg_send![subviews, objectAtIndex: index];
            count += tahoe_count_views_kind_of_excluding(child, class_id, excluded_subtree);
        }
    }
    count
}

#[cfg(target_os = "macos")]
unsafe fn remove_tahoe_window_backdrop(window: id, window_name: &str) {
    if window == nil {
        return;
    }
    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view != nil {
        let _: () = msg_send![glass_view, removeFromSuperview];
    }
    let _: () = msg_send![window, setHasShadow: true];
    if window_name == "Main window" {
        let content_layer: id = msg_send![content_view, layer];
        if content_layer != nil {
            let _: () = msg_send![
                content_layer,
                setCornerRadius: f64::from(crate::ui::chrome::MAIN_WINDOW_CONTENT_RADIUS_PX)
            ];
            let _: () = msg_send![content_layer, setMasksToBounds: true];
        }
    }
}

/// Resolve the backdrop's corner radius for one owning surface.
///
/// Detached-footer windows expose the backdrop's corners mid-window, where
/// the ordinary NSWindow mask cannot round them. Notes needs its stage radius
/// in BOTH layouts: its NSWindow is transparent, so a square full-window
/// backdrop would poke out of the rounded GPUI stage.
#[cfg(target_os = "macos")]
unsafe fn tahoe_backdrop_corner_radius_for(
    window_name: &str,
    backdrop_layout: TahoeBackdropLayout,
    content_view: id,
) -> f64 {
    let radius = tahoe_content_corner_radius(content_view);
    if radius > 0.0 {
        radius
    } else if window_name == "Notes" {
        f64::from(crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX)
    } else if backdrop_layout.is_detached_footer() {
        if window_name == "Dictation overlay" {
            f64::from(crate::ui::chrome::LIQUID_GLASS_PANEL_RADIUS_PX)
        } else {
            f64::from(crate::ui::chrome::MAIN_WINDOW_CONTENT_RADIUS_PX)
        }
    } else {
        radius
    }
}

#[cfg(target_os = "macos")]
unsafe fn update_tahoe_backdrop_geometry_and_shadow(
    window: id,
    content_view: id,
    glass_view: id,
    backdrop_layout: TahoeBackdropLayout,
    _corner_radius: f64,
) {
    use cocoa::foundation::NSRect;

    let content_bounds: NSRect = msg_send![content_view, bounds];
    let backdrop_frame = backdrop_layout.frame(content_bounds);
    (*glass_view).set_ivar("_scriptKitBottomInset", backdrop_layout.bottom_inset());
    let _: () = msg_send![glass_view, setFrame: backdrop_frame];

    let content_layer: id = msg_send![content_view, layer];
    if backdrop_layout.is_detached_footer() {
        let _: () = msg_send![window, setHasShadow: false];
        if content_layer != nil {
            let _: () = msg_send![content_layer, setCornerRadius: 0.0f64];
            let _: () = msg_send![content_layer, setMasksToBounds: false];
        }
        let _: () = msg_send![glass_view, setWantsLayer: true];
        let backdrop_layer: id = msg_send![glass_view, layer];
        if backdrop_layer != nil {
            // The backdrop ends immediately above the 8pt desktop gutter.
            // Any ordinary blurred drop shadow necessarily paints through that
            // gap and visually reconnects the main material to the footer.
            // Keep depth in the native glass edge itself; the one-window host
            // and bounded backdrop must contribute no footer-facing shadow.
            let _: () = msg_send![backdrop_layer, setMasksToBounds: true];
            let _: () = msg_send![backdrop_layer, setShadowOpacity: 0.0f32];
            let _: () = msg_send![backdrop_layer, setShadowRadius: 0.0f64];
            let _: () = msg_send![
                backdrop_layer,
                setShadowOffset: cocoa::foundation::NSSize::new(0.0, 0.0)
            ];
            let _: () = msg_send![backdrop_layer, setShadowPath: nil];
        }
    } else {
        let _: () = msg_send![window, setHasShadow: true];
        let backdrop_layer: id = msg_send![glass_view, layer];
        if backdrop_layer != nil {
            let _: () = msg_send![backdrop_layer, setShadowOpacity: 0.0f32];
            let _: () = msg_send![backdrop_layer, setShadowPath: nil];
        }
    }
}

/// Refresh only the size-dependent pieces of the main backdrop. This is safe
/// to call from the GPUI bounds observer while a native drag or resize is in
/// progress; it avoids the recursive view walk and material retint performed
/// by the full appearance configuration path.
#[cfg(target_os = "macos")]
unsafe fn refresh_main_tahoe_backdrop_geometry(window: id) {
    if !tahoe_native_glass_composition_available()
        || !crate::theme::get_cached_theme().is_vibrancy_enabled()
    {
        remove_tahoe_window_backdrop(window, "Main window");
        return;
    }
    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view == nil {
        return;
    }
    let layout = tahoe_backdrop_layout("Main window");
    update_tahoe_backdrop_geometry_and_shadow(
        window,
        content_view,
        glass_view,
        layout,
        f64::from(crate::ui::chrome::MAIN_WINDOW_CONTENT_RADIUS_PX),
    );
}

/// Audit the immediate children of `content_view`: how many are glass views and
/// the index of the first glass child.
#[cfg(target_os = "macos")]
unsafe fn tahoe_glass_subview_audit(
    content_view: id,
    glass_class: id,
) -> (usize, Option<usize>, usize) {
    if content_view == nil || glass_class == nil {
        return (0, None, 0);
    }
    let subviews: id = msg_send![content_view, subviews];
    if subviews == nil {
        return (0, None, 0);
    }
    let subview_count: usize = msg_send![subviews, count];
    let mut glass_count = 0usize;
    let mut first_glass_index = None;
    for index in 0..subview_count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        if child == nil {
            continue;
        }
        let is_glass: bool = msg_send![child, isKindOfClass: glass_class];
        if is_glass {
            glass_count += 1;
            if first_glass_index.is_none() {
                first_glass_index = Some(index);
            }
        }
    }
    (glass_count, first_glass_index, subview_count)
}

/// True when `glass_view` is the backmost (index 0) child of `content_view`.
#[cfg(target_os = "macos")]
unsafe fn tahoe_glass_backdrop_is_backmost(content_view: id, glass_view: id) -> bool {
    if content_view == nil || glass_view == nil {
        return false;
    }
    let subviews: id = msg_send![content_view, subviews];
    if subviews == nil {
        return false;
    }
    let subview_count: usize = msg_send![subviews, count];
    if subview_count == 0 {
        return false;
    }
    let first: id = msg_send![subviews, objectAtIndex: 0usize];
    first == glass_view
}

/// Re-pin the glass view to the backmost position without reparenting or
/// touching any sibling (the footer NSVisualEffectView trio is never moved).
#[cfg(target_os = "macos")]
unsafe fn tahoe_pin_glass_backdrop_backmost(content_view: id, glass_view: id) {
    if content_view == nil || glass_view == nil {
        return;
    }
    if tahoe_glass_backdrop_is_backmost(content_view, glass_view) {
        return;
    }
    // SAFETY: retain across the move so removeFromSuperview cannot deallocate
    // the view before addSubview re-retains it.
    let _: id = msg_send![glass_view, retain];
    let _: () = msg_send![glass_view, removeFromSuperview];
    let _: () = msg_send![
        content_view,
        addSubview: glass_view
        positioned: NS_WINDOW_BELOW
        relativeTo: nil
    ];
    let _: () = msg_send![glass_view, release];
}

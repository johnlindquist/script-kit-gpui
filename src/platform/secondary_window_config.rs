// ============================================================================
// Actions Popup Window Configuration
// ============================================================================

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassMorphVariant {
    WindowFrame,
    ContentLayer,
    FadeOnly,
}

#[cfg(target_os = "macos")]
impl GlassMorphVariant {
    fn log_name(self) -> &'static str {
        match self {
            Self::WindowFrame => "window_frame",
            Self::ContentLayer => "detached_window_frame",
            Self::FadeOnly => "fade_only",
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct GlassMorphTuning {
    duration: f64,
    inset_fraction: f64,
    start_scale_x: f64,
    start_scale_y: f64,
    squish_scale_x: f64,
    squish_scale_y: f64,
    phase1: f64,
    phase2: f64,
}

#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_DURATION: f64 = 0.02;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_INSET: f64 = 0.005;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_DURATION: f64 = 2.0;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_INSET: f64 = 0.4;
#[cfg(target_os = "macos")]
const GLASS_MORPH_VERTICAL_DAMPING: f64 = 0.4;
#[cfg(target_os = "macos")]
const GLASS_MORPH_SQUISH_FACTOR: f64 = 0.5;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_SQUISH: f64 = 0.012;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_SQUISH: f64 = 0.03;
#[cfg(target_os = "macos")]
const GLASS_MORPH_PHASE1_FRACTION: f64 = 0.5;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_REBOUND_DURATION: f64 = 0.08;
#[cfg(target_os = "macos")]
const GLASS_MORPH_FADE_FRACTION: f64 = 0.7;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_FADE_DURATION: f64 = 0.10;
#[cfg(target_os = "macos")]
const GLASS_EXIT_DURATION: f64 = 0.12;
const GLASS_EXIT_REMOVE_DELAY_MS: u64 = 135;
#[cfg(target_os = "macos")]
const GLASS_EXIT_GROW_X: f64 = 0.03;
/// Popup/modal shrink-out exit travel (fraction per side). Slightly larger
/// than the grow-out release so the "someone let go of the ball" read is
/// visible at the fast exit duration.
#[cfg(target_os = "macos")]
const GLASS_EXIT_SHRINK_X: f64 = 0.05;
#[cfg(target_os = "macos")]
const GLASS_EXIT_SHRINK_Y: f64 = 0.035;
#[cfg(target_os = "macos")]
const GLASS_EXIT_GROW_Y: f64 = 0.012;
#[cfg(target_os = "macos")]
const GLASS_EXIT_BLUR_RADIUS: f64 = 8.0;

#[cfg(target_os = "macos")]
fn glass_morph_tuning() -> Option<GlassMorphTuning> {
    let opacity = crate::theme::get_cached_theme().get_opacity();
    let duration = f64::from(
        opacity
            .glass_morph_duration
            .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
            .clamp(0.0, GLASS_MORPH_MAX_DURATION as f32),
    );
    let inset_fraction = f64::from(
        opacity
            .glass_morph_inset
            .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET)
            .clamp(0.0, GLASS_MORPH_MAX_INSET as f32),
    );
    glass_morph_tuning_from(duration, inset_fraction)
}

#[cfg(target_os = "macos")]
fn glass_morph_tuning_from(duration: f64, inset_fraction: f64) -> Option<GlassMorphTuning> {
    if duration < GLASS_MORPH_MIN_DURATION || inset_fraction < GLASS_MORPH_MIN_INSET {
        return None;
    }

    let squish_fraction = (inset_fraction * GLASS_MORPH_SQUISH_FACTOR)
        .clamp(GLASS_MORPH_MIN_SQUISH, GLASS_MORPH_MAX_SQUISH);
    let phase1 = duration * GLASS_MORPH_PHASE1_FRACTION;
    Some(GlassMorphTuning {
        duration,
        inset_fraction,
        start_scale_x: 1.0 + inset_fraction * 2.0,
        start_scale_y: 1.0 + inset_fraction * GLASS_MORPH_VERTICAL_DAMPING * 2.0,
        squish_scale_x: 1.0 - squish_fraction * 2.0,
        squish_scale_y: 1.0 - squish_fraction * GLASS_MORPH_VERTICAL_DAMPING * 2.0,
        phase1,
        phase2: (duration - phase1).max(GLASS_MORPH_MIN_REBOUND_DURATION),
    })
}

// SAFETY: Caller must pass a valid NSWindow pointer on the main thread.
// The function nil-checks all derived pointers (content view, appearance).
#[cfg(target_os = "macos")]
unsafe fn configure_window_vibrancy_common(
    window: id,
    log_target: &str,
    window_name: &str,
    is_dark: bool,
    morph_variant: GlassMorphVariant,
) {
    // Clear window appearance so GPUI can detect system appearance changes.
    // Appearance is set on individual NSVisualEffectViews instead.
    let _: () = msg_send![window, setAppearance: nil];
    logging::log(
        log_target,
        &format!(
            "{}: Cleared window appearance (nil) for {} mode; appearance set on views",
            window_name,
            if is_dark { "dark" } else { "light" }
        ),
    );

    // Use windowBackgroundColor for semi-opaque background — except in glass
    // mode, where that base renders UNDER the NSGlassEffectView backdrop and
    // dims the whole material; use the near-clear base instead (0.0001 alpha
    // keeps the window shadow machinery alive, unlike clearColor).
    let glass_mode =
        tahoe_liquid_glass_available() && crate::theme::get_cached_theme().is_vibrancy_enabled();
    let window_bg_color: id = if glass_mode {
        msg_send![
            class!(NSColor),
            colorWithSRGBRed: 0.0f64 green: 0.0f64 blue: 0.0f64 alpha: 0.0001f64
        ]
    } else {
        msg_send![class!(NSColor), windowBackgroundColor]
    };
    let _: () = msg_send![window, setBackgroundColor: window_bg_color];
    logging::log(
        log_target,
        &format!(
            "{}: Set backgroundColor ({} base)",
            window_name,
            if glass_mode {
                "glass near-clear"
            } else {
                "windowBackgroundColor semi-opaque"
            }
        ),
    );

    // Mark window as non-opaque to allow transparency/vibrancy.
    let _: () = msg_send![window, setOpaque: false];

    // Enable shadow for native depth perception.
    let _: () = msg_send![window, setHasShadow: true];

    // Configure NSVisualEffectViews in the window hierarchy.
    let content_view: id = msg_send![window, contentView];
    if !content_view.is_null() {
        let mut count = 0;
        let material = current_window_material();
        configure_visual_effect_views_recursive(content_view, &mut count, is_dark, material);
        let material_name = current_window_material_name(material);
        logging::log(
            log_target,
            &format!(
                "{}: Configured {} NSVisualEffectView(s) with {} material",
                window_name, count, material_name
            ),
        );
    }

    let glass_created = configure_tahoe_window_backdrop(window, log_target, window_name);
    // Secondary/overlay windows (notes, dictation, confirm, actions, AI,
    // flow manager, inline popups) are created per appearance, so a freshly
    // created backdrop means the window just appeared: morph it in.
    // Child-attached panels transform the content layer because animating a
    // child NSWindow frame fights AppKit's parent-child machinery and lags.
    if glass_created {
        match morph_variant {
            GlassMorphVariant::WindowFrame => {
                animate_tahoe_glass_appearance(window, log_target, window_name)
            }
            GlassMorphVariant::ContentLayer => {
                // Runtime-proven (real-pixel capture + static-transform
                // experiment): CALayer transforms on the contentView's
                // NSViewBackingLayer are neutralized by AppKit — even a
                // static 0.85 model scale renders at full size. No
                // layer-transform morph can ever work on AppKit-managed
                // backing layers. Instead: detach from the parent window for
                // the morph's duration and run the SAME NSWindow frame morph
                // the main window uses, then reattach — the frame animation
                // only fights the parent-child machinery while attached.
                animate_tahoe_glass_child_appearance(window, log_target, window_name);
            }
            GlassMorphVariant::FadeOnly => {
                animate_tahoe_glass_fade_appearance(window, log_target, window_name)
            }
        }
    }

    let appearance_name = if is_dark {
        "VibrantDark"
    } else {
        "VibrantLight"
    };
    let material_name = current_window_material_name(current_window_material());
    logging::log(
        log_target,
        &format!(
            "{} vibrancy configured ({} + {} + blur)",
            window_name, appearance_name, material_name
        ),
    );
}

#[cfg(target_os = "macos")]
fn current_window_material() -> crate::theme::VibrancyMaterial {
    crate::theme::get_cached_theme().get_vibrancy().material
}

#[cfg(target_os = "macos")]
fn current_window_material_name(material: crate::theme::VibrancyMaterial) -> &'static str {
    match material {
        crate::theme::VibrancyMaterial::Hud => "HUD_WINDOW",
        crate::theme::VibrancyMaterial::Popover => "POPOVER",
        crate::theme::VibrancyMaterial::Menu => "MENU",
        crate::theme::VibrancyMaterial::Sidebar => "SIDEBAR",
        crate::theme::VibrancyMaterial::Content => "CONTENT_BACKGROUND",
    }
}

#[cfg(target_os = "macos")]
fn tahoe_liquid_glass_class() -> Option<id> {
    // NSGlassEffectView is the AppKit Liquid Glass API introduced in macOS 26
    // Tahoe, so class availability is the capability gate.
    #[link(name = "Foundation", kind = "framework")]
    extern "C" {
        fn NSClassFromString(a_class_name: id) -> id;
    }

    let glass_class_name: id =
        unsafe { msg_send![class!(NSString), stringWithUTF8String: c"NSGlassEffectView".as_ptr()] };
    let glass_class = if glass_class_name.is_null() {
        cocoa::base::nil
    } else {
        unsafe { NSClassFromString(glass_class_name) }
    };
    if glass_class.is_null() {
        None
    } else {
        Some(glass_class)
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Kept for reference: the blur-era themed glass tint.
unsafe fn liquid_glass_tint_color() -> id {
    let theme = crate::theme::get_cached_theme();
    let rgba = crate::ui_foundation::main_window_matched_background_rgba(&theme);
    let red = ((rgba >> 24) & 0xff) as f64 / 255.0;
    let green = ((rgba >> 16) & 0xff) as f64 / 255.0;
    let blue = ((rgba >> 8) & 0xff) as f64 / 255.0;
    let alpha = (rgba & 0xff) as f64 / 255.0;
    msg_send![
        class!(NSColor),
        colorWithCalibratedRed: red
        green: green
        blue: blue
        alpha: alpha
    ]
}

/// True when the AppKit Liquid Glass API (macOS 26 Tahoe) is available, i.e.
/// the tagged NSGlassEffectView backdrop can actually render. Window creation
/// uses this to pick `Transparent` (the glass backdrop supplies the material)
/// over `Blurred`, whose full-window NSVisualEffectView would sit above the
/// backmost glass view and hide it entirely.
#[cfg(target_os = "macos")]
pub fn tahoe_liquid_glass_available() -> bool {
    // Cached: also consulted per-render by the veil-opacity resolver.
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| tahoe_liquid_glass_class().is_some())
}

#[cfg(not(target_os = "macos"))]
pub fn tahoe_liquid_glass_available() -> bool {
    false
}

/// Background appearance for a vibrancy-enabled window: `Transparent` when
/// the Tahoe glass backdrop supplies the material (a `Blurred` appearance
/// would stack the gpui fork's NSVisualEffectView above the glass and hide
/// it), `Blurred` otherwise.
pub fn vibrancy_window_background() -> gpui::WindowBackgroundAppearance {
    if tahoe_liquid_glass_available() {
        gpui::WindowBackgroundAppearance::Transparent
    } else {
        gpui::WindowBackgroundAppearance::Blurred
    }
}

/// Resolve the NSWindow behind a live GPUI window and run the shared
/// secondary vibrancy/glass configuration on it (glass backdrop, VEV
/// handling, glass-mode window base). For overlay windows (confirm popup,
/// AI, flow manager) that have no dedicated native config path.
#[cfg(target_os = "macos")]
pub fn configure_overlay_window_glass(window: &gpui::Window, window_name: &str) {
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    let is_dark = crate::theme::get_cached_theme().should_use_dark_vibrancy();
    // SAFETY: ns_view belongs to the live GPUI window on the main thread;
    // `-[NSView window]` is standard and the result is nil-checked inside
    // configure_secondary_window_vibrancy.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        configure_secondary_window_vibrancy(ns_window, window_name, is_dark);
    }
}

/// Configure a GPUI overlay that will be attached to a parent NSWindow.
#[cfg(target_os = "macos")]
pub fn configure_child_attached_overlay_window_glass(
    window: &gpui::Window,
    log_target: &str,
    window_name: &str,
) {
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    let is_dark = crate::theme::get_cached_theme().should_use_dark_vibrancy();
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        configure_window_vibrancy_common(
            ns_window,
            log_target,
            window_name,
            is_dark,
            GlassMorphVariant::ContentLayer,
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_child_attached_overlay_window_glass(
    _window: &gpui::Window,
    _log_target: &str,
    _window_name: &str,
) {
}

#[cfg(not(target_os = "macos"))]
pub fn configure_overlay_window_glass(_window: &gpui::Window, _window_name: &str) {}

/// Stable `tag` sentinel so the backdrop view can be found idempotently via
/// `contentView.viewWithTag:` on repeated configure passes.
#[cfg(target_os = "macos")]
const TAHOE_GLASS_BACKDROP_TAG: isize = 0x5c17_0175;
/// Accessibility/debug identifier for the native glass backdrop view.
#[cfg(target_os = "macos")]
const TAHOE_GLASS_BACKDROP_IDENTIFIER: &str = "script-kit-tahoe-glass-backdrop";
/// `NSWindowBelow` ordering constant for `addSubview:positioned:relativeTo:`.
#[cfg(target_os = "macos")]
const NS_WINDOW_BELOW: isize = -1;

/// Pass-through hit test: the backdrop never participates in input so it can
/// never steal clicks/scrolls from GPUI content or the footer trio. Mirrors
/// the existing `ScriptKitFooterPassthroughView` principle.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_hit_test(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _: cocoa::foundation::NSPoint,
) -> id {
    cocoa::base::nil
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
        let _: () = msg_send![this_id, setFrame: bounds];
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
            sel!(orderOutOwnWindow),
            tahoe_glass_backdrop_order_out_window as extern "C" fn(&Object, Sel),
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

/// Per-window two-phase bounce settle targets. Multiple HUDs and secondary
/// windows can materialize concurrently, so a single global slot would let one
/// window steal another's rebound target.
#[cfg(target_os = "macos")]
static GLASS_MORPH_SETTLE_TARGETS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, (f64, f64, f64, f64, f64)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Phase 2 of the appear bounce: ease the window from its overshoot
/// (slightly smaller than final) back up to the final frame.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_settle(this: &objc::runtime::Object, _: objc::runtime::Sel) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    // SAFETY: main thread (performSelector on the main run loop); standard
    // NSView/NSWindow accessors and the window animator proxy.
    unsafe {
        let this_id = this as *const objc::runtime::Object as id;
        let window: id = msg_send![this_id, window];
        if window == nil {
            return;
        }
        let target = GLASS_MORPH_SETTLE_TARGETS
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&(window as usize)));
        let Some((x, y, w, h, settle_duration)) = target else {
            return;
        };
        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![ctx, setDuration: settle_duration];
        let _: () = msg_send![ctx, setAllowsImplicitAnimation: true];
        if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
            let name = tahoe_ns_string("easeOut");
            if name != nil {
                let timing: id = msg_send![timing_class, functionWithName: name];
                if timing != nil {
                    let _: () = msg_send![ctx, setTimingFunction: timing];
                }
            }
        }
        let animator: id = msg_send![window, animator];
        let _: () = msg_send![animator, setFrame: frame display: true];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    }
}

/// Deferred exit: order the glass view's window out after the exit fade and
/// restore its alpha for the next show. Runs on the raw main run loop, i.e.
/// outside any GPUI borrow — the safe context the hide-path docs require.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_order_out_window(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) {
    // SAFETY: main thread; standard NSWindow methods, nil-checked.
    unsafe {
        let this_id = this as *const objc::runtime::Object as id;
        let window: id = msg_send![this_id, window];
        if window == nil {
            return;
        }
        let _: () = msg_send![window, orderOut: nil];
        let _: () = msg_send![window, setAlphaValue: 1.0f64];
        logging::log("PANEL", "Glass exit: window ordered out after fade");
    }
}

/// `+[CAMediaTimingFunction functionWithControlPoints::::]` — the objc
/// msg_send! macro cannot express selectors with unnamed arguments, so call
/// through a typed `objc_msgSend` cast. Control points with y outside 0..1
/// give a smooth overshoot (spring feel) in a single continuous animation.
#[cfg(target_os = "macos")]
#[allow(dead_code)] // Kept for curve experiments; the appear morph now uses
                    // explicit squish/rebound keyframes instead of a single overshoot curve.
unsafe fn timing_function_with_control_points(c1x: f32, c1y: f32, c2x: f32, c2y: f32) -> id {
    #[link(name = "objc")]
    extern "C" {
        fn objc_msgSend();
    }

    let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") else {
        return nil;
    };
    let sel = objc::runtime::Sel::register("functionWithControlPoints::::");
    // SAFETY: objc_msgSend with a matching typed signature; on arm64 a single
    // objc_msgSend entry point serves all signatures.
    let send: unsafe extern "C" fn(
        *mut objc::runtime::Class,
        objc::runtime::Sel,
        f32,
        f32,
        f32,
        f32,
    ) -> id = std::mem::transmute(objc_msgSend as *const ());
    send(
        timing_class as *const objc::runtime::Class as *mut objc::runtime::Class,
        sel,
        c1x,
        c1y,
        c2x,
        c2y,
    )
}

/// Remove any exit-dematerialize blur left on the window's content view
/// layer (a superseded exit, or post-hide cleanup before the next show).
#[cfg(target_os = "macos")]
unsafe fn clear_exit_dematerialize_blur(window: id) {
    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return;
    }
    let layer: id = msg_send![content_view, layer];
    if layer == nil {
        return;
    }
    let nil_id: id = nil;
    let _: () = msg_send![layer, setFilters: nil_id];
    let _: () = msg_send![layer, removeAllAnimations];
}

/// Spotlight-style exit dematerialize for the main window, measured from a
/// 57fps recording: ~120ms ease-in-out fade + slight outward growth (the
/// inverse of the entry's compression) + a gaussian blur ramp on the
/// content view layer (private CAFilter — same pattern as the backdrop
/// saturation boost). Returns false when glass/morph is unavailable so the
/// caller hides immediately.
///
/// This runs ABOVE the synchronous hide layer: the caller plays this,
/// waits ~135ms, then runs the NORMAL hide flow. Never defer orderOut:
/// itself — that livelocked the hotkey gesture listener.
#[cfg(target_os = "macos")]
pub fn begin_main_window_exit_dematerialize() -> bool {
    if require_main_thread("begin_main_window_exit_dematerialize") {
        return false;
    }
    let Some(window) = window_manager::get_main_window() else {
        return false;
    };
    // SAFETY: main thread verified; window valid from the manager.
    unsafe { begin_ns_window_exit_dematerialize(window, "PANEL", "Main window") }
}

/// Generalized dematerialize for any live GPUI window (notes, dictation
/// overlay) — same measured recipe as the main window's exit. Returns false
/// when glass/morph is unavailable so callers close instantly instead.
#[cfg(target_os = "macos")]
pub fn begin_gpui_window_exit_dematerialize(
    window: &gpui::Window,
    log_target: &str,
    window_name: &str,
) -> bool {
    if require_main_thread("begin_gpui_window_exit_dematerialize") {
        return false;
    }
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    // SAFETY: ns_view belongs to a live GPUI window on the main thread.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return false;
        }
        begin_ns_window_exit_dematerialize(ns_window, log_target, window_name)
    }
}

/// Start the shared exit and remove the GPUI window after its short visual
/// tail. Registry cleanup and parent-focus handoff should happen before this
/// call; only destruction is delayed, so dismissal remains input-instant.
pub fn dematerialize_then_remove_gpui_window<V: 'static>(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<V>,
    log_target: &'static str,
    window_name: &'static str,
) {
    if begin_gpui_window_exit_dematerialize(window, log_target, window_name) {
        let any_handle = window.window_handle();
        cx.spawn(async move |_this, cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(GLASS_EXIT_REMOVE_DELAY_MS))
                .await;
            cx.update(|cx| {
                let _ = any_handle.update(cx, |_view, window, _cx| {
                    window.remove_window();
                });
            });
        })
        .detach();
    } else {
        window.remove_window();
    }
}

/// App-context counterpart used by global popup/HUD registries.
pub fn dematerialize_then_remove_gpui_window_from_app(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    log_target: &'static str,
    window_name: &'static str,
) {
    if begin_gpui_window_exit_dematerialize(window, log_target, window_name) {
        remove_gpui_window_after_glass_exit_from_app(window, cx);
    } else {
        window.remove_window();
    }
}

/// Schedule only the destruction tail after a caller has already started the
/// exit. This is used by `on_window_should_close` handlers that must return
/// `false` while the visual tail completes.
pub fn remove_gpui_window_after_glass_exit_from_app(window: &mut gpui::Window, cx: &mut gpui::App) {
    let any_handle = window.window_handle();
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor()
            .timer(std::time::Duration::from_millis(GLASS_EXIT_REMOVE_DELAY_MS))
            .await;
        cx.update(|cx| {
            let _ = any_handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            });
        });
    })
    .detach();
}

#[cfg(not(target_os = "macos"))]
pub fn begin_gpui_window_exit_dematerialize(
    _window: &gpui::Window,
    _log_target: &str,
    _window_name: &str,
) -> bool {
    false
}

/// Core of the exit dematerialize, shared by every window kind.
///
/// # Safety
/// `window` must be a valid NSWindow on the main thread.
#[cfg(target_os = "macos")]
unsafe fn begin_ns_window_exit_dematerialize(
    window: id,
    log_target: &str,
    window_name: &str,
) -> bool {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    if !(tahoe_liquid_glass_available() && crate::theme::get_cached_theme().is_vibrancy_enabled()) {
        return false;
    }
    if glass_morph_tuning().is_none() {
        return false;
    }

    // SAFETY: main thread; standard AppKit calls plus the private
    // CAFilter/CABasicAnimation classes resolved at runtime.
    {
        let visible: bool = msg_send![window, isVisible];
        if !visible {
            return false;
        }
        let frame: NSRect = msg_send![window, frame];
        if frame.size.width < 40.0 || frame.size.height < 40.0 {
            return false;
        }

        // Blur ramp: 0 -> 8pt over the fade.
        let content_view: id = msg_send![window, contentView];
        if content_view != nil {
            let layer: id = msg_send![content_view, layer];
            if layer != nil {
                if let (Some(filter_class), Some(anim_class)) = (
                    objc::runtime::Class::get("CAFilter"),
                    objc::runtime::Class::get("CABasicAnimation"),
                ) {
                    let blur_type = tahoe_ns_string("gaussianBlur");
                    let filter: id = msg_send![filter_class, filterWithType: blur_type];
                    if filter != nil {
                        let exit_name = tahoe_ns_string("exitBlur");
                        let _: () = msg_send![filter, setName: exit_name];
                        let radius_key = tahoe_ns_string("inputRadius");
                        let zero: id = msg_send![class!(NSNumber), numberWithDouble: 0.0f64];
                        let _: () = msg_send![filter, setValue: zero forKey: radius_key];
                        let filters: id = msg_send![class!(NSArray), arrayWithObject: filter];
                        let _: () = msg_send![layer, setFilters: filters];

                        let key_path = tahoe_ns_string("filters.exitBlur.inputRadius");
                        let anim: id = msg_send![anim_class, animationWithKeyPath: key_path];
                        if anim != nil {
                            let eight: id = msg_send![class!(NSNumber), numberWithDouble: GLASS_EXIT_BLUR_RADIUS];
                            let _: () = msg_send![anim, setFromValue: zero];
                            let _: () = msg_send![anim, setToValue: eight];
                            let _: () = msg_send![anim, setDuration: GLASS_EXIT_DURATION];
                            let forwards = tahoe_ns_string("forwards");
                            let _: () = msg_send![anim, setFillMode: forwards];
                            let _: () = msg_send![anim, setRemovedOnCompletion: false];
                            let anim_key = tahoe_ns_string("exitBlurRamp");
                            let _: () = msg_send![layer, addAnimation: anim forKey: anim_key];
                        }
                    }
                }
            }
        }

        // Exit travel direction is a per-surface policy (user call
        // 2026-07-21): the main window (and other free-standing windows)
        // keep the outward release, while the actions menu and modal popups
        // SHRINK away — the inverse of their grow-in enter, like a squeezed
        // ball being let go of in reverse. Centralized here so every exit
        // path shares one classification.
        let shrink_out = matches!(
            window_name,
            "Actions popup"
                | "Confirm popup"
                | "Inline popup"
                | "Shortcut recorder popup"
                | "Microphone popup"
                | "Agent Chat history popup"
        );
        let grown = if shrink_out {
            let shrink_x = frame.size.width * GLASS_EXIT_SHRINK_X;
            let shrink_y = frame.size.height * GLASS_EXIT_SHRINK_Y;
            NSRect::new(
                NSPoint::new(frame.origin.x + shrink_x, frame.origin.y + shrink_y),
                NSSize::new(
                    (frame.size.width - shrink_x * 2.0).max(1.0),
                    (frame.size.height - shrink_y * 2.0).max(1.0),
                ),
            )
        } else {
            let grow_x = frame.size.width * GLASS_EXIT_GROW_X;
            let grow_y = frame.size.height * GLASS_EXIT_GROW_Y;
            NSRect::new(
                NSPoint::new(frame.origin.x - grow_x, frame.origin.y - grow_y),
                NSSize::new(
                    frame.size.width + grow_x * 2.0,
                    frame.size.height + grow_y * 2.0,
                ),
            )
        };
        // Child-attached popups: layer transforms are neutralized by AppKit
        // (NSViewBackingLayer, runtime-proven) and frame animation fights the
        // parent-child machinery — so detach for the exit. The window is
        // being destroyed after the 135ms tail; no reattach.
        let parent_window: id = msg_send![window, parentWindow];
        let variant = if parent_window == nil {
            GlassMorphVariant::WindowFrame
        } else {
            GlassMorphVariant::ContentLayer
        };
        if parent_window != nil {
            let _: () = msg_send![parent_window, removeChildWindow: window];
        }

        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![ctx, setDuration: GLASS_EXIT_DURATION];
        let _: () = msg_send![ctx, setAllowsImplicitAnimation: true];
        if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
            let name = tahoe_ns_string("easeInEaseOut");
            if name != nil {
                let timing: id = msg_send![timing_class, functionWithName: name];
                if timing != nil {
                    let _: () = msg_send![ctx, setTimingFunction: timing];
                }
            }
        }
        let animator: id = msg_send![window, animator];
        let _: () = msg_send![animator, setFrame: grown display: true];
        let _: () = msg_send![animator, setAlphaValue: 0.0f64];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];

        logging::log(
            log_target,
            &format!(
                "event=glass_morph window={} variant={} phase=exit duration={:.2}s direction={}",
                window_name,
                variant.log_name(),
                GLASS_EXIT_DURATION,
                if shrink_out { "shrink_out" } else { "grow_out" }
            ),
        );
    }
    true
}

#[cfg(not(target_os = "macos"))]
pub fn begin_main_window_exit_dematerialize() -> bool {
    false
}

/// Park a just-hidden window at alpha 0 so that whichever code path orders
/// it front next cannot flash a full-alpha frame before the appear morph
/// starts (multiple show paths exist: the platform show helpers and GPUI's
/// `activate_window`). The appear morph restores alpha — including on every
/// early exit — so the window can never stay invisible.
#[cfg(target_os = "macos")]
unsafe fn park_hidden_window_for_glass_morph(window: id) {
    if window.is_null() {
        return;
    }
    // Post-hide cleanup: drop any exit-dematerialize blur so the next show
    // starts crisp.
    clear_exit_dematerialize_blur(window);
    if !(tahoe_liquid_glass_available() && crate::theme::get_cached_theme().is_vibrancy_enabled()) {
        return;
    }
    let morph_opacity = crate::theme::get_cached_theme().get_opacity();
    let duration = morph_opacity
        .glass_morph_duration
        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION);
    let inset = morph_opacity
        .glass_morph_inset
        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET);
    if duration < 0.02 || inset < 0.005 {
        return; // morph disabled: leave alpha untouched
    }
    let _: () = msg_send![window, setAlphaValue: 0.0f64];
}

/// True when a morph started within the last 700ms. Rapid re-shows would
/// otherwise capture a mid-animation frame as the "final" target and shrink
/// the window a little more on every trigger.
#[cfg(target_os = "macos")]
fn glass_morph_recently_started() -> bool {
    use std::sync::atomic::Ordering;
    let now_ms = glass_morph_now_ms();
    let last = GLASS_MORPH_LAST_START_MS.load(Ordering::Relaxed);
    if last != u64::MAX && now_ms.saturating_sub(last) < 700 {
        return true;
    }
    GLASS_MORPH_LAST_START_MS.store(now_ms, Ordering::Relaxed);
    false
}

#[cfg(target_os = "macos")]
static GLASS_MORPH_EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
static GLASS_MORPH_LAST_START_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(u64::MAX);
#[cfg(target_os = "macos")]
static GLASS_MORPH_LAST_DURATION_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(target_os = "macos")]
fn glass_morph_now_ms() -> u64 {
    let epoch = *GLASS_MORPH_EPOCH.get_or_init(std::time::Instant::now);
    epoch.elapsed().as_millis() as u64
}

/// Time left (plus a small settle tail) in the morph that most recently
/// started, if it is still in flight. Lets sibling windows (the footer
/// overlay) hide while the main window animates and fade in afterwards.
#[cfg(target_os = "macos")]
pub fn glass_morph_remaining() -> Option<std::time::Duration> {
    use std::sync::atomic::Ordering;
    let start = GLASS_MORPH_LAST_START_MS.load(Ordering::Relaxed);
    if start == u64::MAX {
        return None;
    }
    let duration_ms = GLASS_MORPH_LAST_DURATION_MS.load(Ordering::Relaxed) + 60;
    let end = start.saturating_add(duration_ms);
    let now = glass_morph_now_ms();
    if now >= end {
        None
    } else {
        Some(std::time::Duration::from_millis(end - now))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn glass_morph_remaining() -> Option<std::time::Duration> {
    None
}

/// Park a sibling GPUI window (footer overlay) at alpha 0 when a glass
/// morph is in flight, returning how long until it should fade back in.
/// The overlay is a separate NSWindow that tracks the main window's frame;
/// without this it appears instantly at full alpha and visibly chases the
/// animating frame.
#[cfg(target_os = "macos")]
pub fn park_gpui_window_alpha_if_morphing(window: &gpui::Window) -> Option<std::time::Duration> {
    let remaining = glass_morph_remaining()?;
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    // SAFETY: main thread; standard NSWindow accessors, nil-checked.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return None;
        }
        let _: () = msg_send![ns_window, setAlphaValue: 0.0f64];
    }
    Some(remaining)
}

#[cfg(not(target_os = "macos"))]
pub fn park_gpui_window_alpha_if_morphing(_window: &gpui::Window) -> Option<std::time::Duration> {
    None
}

/// Fade a previously parked sibling window back in (short ease-out).
#[cfg(target_os = "macos")]
pub fn restore_gpui_window_alpha_animated(window: &gpui::Window) {
    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    // SAFETY: main thread; standard NSWindow/NSAnimationContext usage.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![ctx, setDuration: 0.12f64];
        let animator: id = msg_send![ns_window, animator];
        let _: () = msg_send![animator, setAlphaValue: 1.0f64];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn restore_gpui_window_alpha_animated(_window: &gpui::Window) {}

/// Alpha-only appear for child-attached panels (actions popup, confirm
/// popup): animating a child window's FRAME fights the parent-child
/// constraint machinery and lags badly, so they fade in with the same
/// spring duration instead.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_fade_appearance(window: id, log_target: &str, window_name: &str) {
    let Some(tuning) = glass_morph_tuning() else {
        let _: () = msg_send![window, setAlphaValue: 1.0f64];
        return;
    };
    let fade = (tuning.duration * GLASS_MORPH_FADE_FRACTION).max(GLASS_MORPH_MIN_FADE_DURATION);
    let _: () = msg_send![window, setAlphaValue: 0.0f64];
    let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
    let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![ctx, setDuration: fade];
    if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
        let name = tahoe_ns_string("easeOut");
        if name != nil {
            let timing: id = msg_send![timing_class, functionWithName: name];
            if timing != nil {
                let _: () = msg_send![ctx, setTimingFunction: timing];
            }
        }
    }
    let animator: id = msg_send![window, animator];
    let _: () = msg_send![animator, setAlphaValue: 1.0f64];
    let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    logging::log(
        log_target,
        &format!(
            "event=glass_morph window={} variant={} phase=enter duration={:.2}s",
            window_name,
            GlassMorphVariant::FadeOnly.log_name(),
            fade
        ),
    );
}

#[cfg(target_os = "macos")]
/// Child-attached popup enter: detach from the parent NSWindow, run the
/// frame morph in GROW-IN direction, and reattach after it settles.
/// Layer-transform morphs are impossible here — AppKit neutralizes
/// transforms on NSViewBackingLayer (runtime-proven) — and frame-animating
/// while parent-attached fights the parent-child machinery (c598a32bf).
///
/// Direction matters for reflow feel: GPUI re-lays-out the popup at every
/// intermediate size (wanted — the content stays alive), but starting WIDE
/// (the main window's outset enter) lays the content out ~6-12%% wider
/// first and the squeeze reads as the list lagging the container (user
/// report). Growing in from below final size keeps the same physics while
/// the reflow reads as the menu materializing into place.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_child_appearance(window: id, log_target: &str, window_name: &str) {
    let Some(tuning) = glass_morph_tuning() else {
        let _: () = msg_send![window, setAlphaValue: 1.0f64];
        return;
    };

    let parent: id = msg_send![window, parentWindow];
    if parent != nil {
        let _: id = msg_send![parent, retain];
        let _: () = msg_send![parent, removeChildWindow: window];
        let _: id = msg_send![window, retain];
        schedule_child_morph_settle(parent, window, tuning.duration + 0.08);
    }

    animate_tahoe_glass_appearance_directed(window, log_target, window_name, true);
    logging::log(
        log_target,
        &format!(
            "event=glass_morph window={} variant={} phase=enter duration={:.2}s detached={} direction=grow_in",
            window_name,
            GlassMorphVariant::ContentLayer.log_name(),
            tuning.duration,
            parent != nil,
        ),
    );
}

/// After the enter morph settles: re-attach the popup to its parent. Both
/// windows were retained by the caller; releases happen here exactly once.
#[cfg(target_os = "macos")]
unsafe fn schedule_child_morph_settle(parent: id, window: id, delay_seconds: f64) {
    #[link(name = "System", kind = "dylib")]
    extern "C" {
        static _dispatch_main_q: std::ffi::c_void;
        fn dispatch_time(when: u64, delta: i64) -> u64;
        fn dispatch_after_f(
            when: u64,
            queue: *const std::ffi::c_void,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
    }

    struct SettleContext {
        parent: id,
        window: id,
    }
    // SAFETY: raw NSWindow pointers retained by the caller; consumed exactly
    // once on the main queue below.
    unsafe impl Send for SettleContext {}

    extern "C" fn settle(context: *mut std::ffi::c_void) {
        // SAFETY: context is the Box leaked below; main queue; the windows
        // were retained before the hop.
        unsafe {
            let context = Box::from_raw(context as *mut SettleContext);
            let parent = context.parent;
            let window = context.window;
            let window_visible: bool = msg_send![window, isVisible];
            let parent_visible: bool = msg_send![parent, isVisible];
            let current_parent: id = msg_send![window, parentWindow];
            if window_visible && parent_visible && current_parent == nil {
                const NS_WINDOW_ABOVE: i64 = 1;
                let _: () = msg_send![parent, addChildWindow: window ordered: NS_WINDOW_ABOVE];
            }
            let _: () = msg_send![parent, release];
            let _: () = msg_send![window, release];
        }
    }

    const DISPATCH_TIME_NOW: u64 = 0;
    let when = dispatch_time(DISPATCH_TIME_NOW, (delay_seconds * 1e9) as i64);
    let context = Box::into_raw(Box::new(SettleContext { parent, window }));
    dispatch_after_f(
        when,
        &_dispatch_main_q as *const std::ffi::c_void,
        context as *mut std::ffi::c_void,
        settle,
    );
}

/// Morph the whole window into place: frame scales up from a centered inset
/// rect while the window fades in, so the glass backdrop AND the GPUI
/// content arrive together (animating only the glass view left the content
/// popping in at full size over a growing background). The glass tracks the
/// window via its autoresizing mask during the frame animation.
///
/// Duration and inset come from the theme's glass morph sliders; either at
/// (near) zero disables the morph.
///
/// # Safety
/// `window` must be a valid NSWindow on the main thread.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_appearance(window: id, log_target: &str, window_name: &str) {
    animate_tahoe_glass_appearance_directed(window, log_target, window_name, false)
}

/// `grow_in = false`: the Spotlight outset enter (start wider, compress
/// below final, rebound out) used by free-standing windows. `grow_in =
/// true`: the child-popup direction — start BELOW final size, overshoot
/// slightly past it, settle back — same phases and curves, inverted travel,
/// so live GPUI reflow reads as the menu materializing instead of a squeeze.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_appearance_directed(
    window: id,
    log_target: &str,
    window_name: &str,
    grow_in: bool,
) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    // A superseded exit dematerialize may have left a blur on the content
    // view layer — every show must clear it before drawing.
    clear_exit_dematerialize_blur(window);

    // The hide path parks the window at alpha 0 so no show path can flash a
    // full-alpha frame. Every early exit below must therefore restore alpha,
    // or a skipped morph would leave the window invisible.
    let restore_alpha = |window: id| {
        let _: () = msg_send![window, setAlphaValue: 1.0f64];
    };

    let Some(tuning) = glass_morph_tuning() else {
        restore_alpha(window);
        return; // morph disabled via theme sliders
    };
    let is_main_window = crate::window_manager::get_main_window() == Some(window);
    if is_main_window && glass_morph_recently_started() {
        restore_alpha(window);
        return;
    }

    let final_frame: NSRect = msg_send![window, frame];
    if final_frame.size.width < 40.0 || final_frame.size.height < 40.0 {
        restore_alpha(window);
        return;
    }

    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        restore_alpha(window);
        return;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view == nil {
        restore_alpha(window);
        return;
    }

    // Spotlight enters by SHRINKING into place: start larger than final
    // (the "inset" slider is the start outset) and glide down. Measured
    // from the real Spotlight: the morph is WIDTH-DOMINANT — height locks
    // early and barely undershoots — so the vertical deltas are damped.
    let outset_x = final_frame.size.width * tuning.inset_fraction;
    let outset_y = final_frame.size.height * (tuning.inset_fraction * GLASS_MORPH_VERTICAL_DAMPING);
    let outset_sign = if grow_in { -1.0 } else { 1.0 };
    let start = NSRect::new(
        NSPoint::new(
            final_frame.origin.x - outset_x * outset_sign,
            final_frame.origin.y - outset_y * outset_sign,
        ),
        NSSize::new(
            final_frame.size.width + outset_x * 2.0 * outset_sign,
            final_frame.size.height + outset_y * 2.0 * outset_sign,
        ),
    );

    // A show during the exit fade must cancel the pending deferred orderOut,
    // or it would fire mid-appear and vanish the window.
    let _: () = msg_send![
        class!(NSObject),
        cancelPreviousPerformRequestsWithTarget: glass_view
        selector: sel!(orderOutOwnWindow)
        object: nil
    ];

    let _: () = msg_send![window, setFrame: start display: true];
    let _: () = msg_send![window, setAlphaValue: 0.0f64];

    // Record the in-flight duration so sibling windows (footer overlay) can
    // hide until the morph settles (glass_morph_remaining).
    if is_main_window {
        GLASS_MORPH_LAST_DURATION_MS.store(
            (tuning.duration * 1000.0) as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    // Squish target: compress BELOW the final size — the released-elastic
    // physics the Spotlight enter has. Visible (~1.5% of each dimension),
    // scaled off the start outset.
    let squish_fraction = (tuning.inset_fraction * GLASS_MORPH_SQUISH_FACTOR)
        .clamp(GLASS_MORPH_MIN_SQUISH, GLASS_MORPH_MAX_SQUISH);
    let squish_x = final_frame.size.width * squish_fraction;
    let squish_y = final_frame.size.height * (squish_fraction * GLASS_MORPH_VERTICAL_DAMPING);
    // Grow-in inverts the mid-point too: a slight overshoot PAST final.
    let squish = NSRect::new(
        NSPoint::new(
            final_frame.origin.x + squish_x * outset_sign,
            final_frame.origin.y + squish_y * outset_sign,
        ),
        NSSize::new(
            final_frame.size.width - squish_x * 2.0 * outset_sign,
            final_frame.size.height - squish_y * 2.0 * outset_sign,
        ),
    );

    // Phase 1: wide -> squished-under-final, soft on both ends (the window
    // momentarily comes to rest at max compression — physically natural).
    // Measured Spotlight split: compression and rebound take EQUAL time,
    // but the rebound travels ~1/4 the distance, so it reads far gentler.
    let phase1 = tuning.phase1;
    let phase2 = tuning.phase2;

    // Cancel any pending settle from an interrupted previous morph.
    let _: () = msg_send![
        class!(NSObject),
        cancelPreviousPerformRequestsWithTarget: glass_view
        selector: sel!(settleOwnWindowFrame)
        object: nil
    ];

    let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
    let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![ctx, setDuration: phase1];
    let _: () = msg_send![ctx, setAllowsImplicitAnimation: true];
    if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
        let name = tahoe_ns_string("easeInEaseOut");
        if name != nil {
            let timing: id = msg_send![timing_class, functionWithName: name];
            if timing != nil {
                let _: () = msg_send![ctx, setTimingFunction: timing];
            }
        }
    }
    let animator: id = msg_send![window, animator];
    let _: () = msg_send![animator, setFrame: squish display: true];
    let _: () = msg_send![animator, setAlphaValue: 1.0f64];
    let _: () = msg_send![class!(NSAnimationContext), endGrouping];

    // Phase 2: rebound out to the natural size (settle selector, run-loop
    // scheduled, ease-out over the remaining duration).
    if let Ok(mut guard) = GLASS_MORPH_SETTLE_TARGETS.lock() {
        guard.insert(
            window as usize,
            (
                final_frame.origin.x,
                final_frame.origin.y,
                final_frame.size.width,
                final_frame.size.height,
                phase2,
            ),
        );
    }
    let _: () = msg_send![
        glass_view,
        performSelector: sel!(settleOwnWindowFrame)
        withObject: nil
        afterDelay: phase1
    ];

    logging::log(
        log_target,
        &format!(
            "event=glass_morph window={} variant={} phase=enter duration={:.2}s inset={:.3} frames={}x{}->{}x{}->{}x{}",
            window_name,
            GlassMorphVariant::WindowFrame.log_name(),
            tuning.duration,
            tuning.inset_fraction,
            start.size.width as i64,
            start.size.height as i64,
            squish.size.width as i64,
            squish.size.height as i64,
            final_frame.size.width as i64,
            final_frame.size.height as i64,
        ),
    );
}

/// Spotlight-style exit: an extremely fast fade with a slight outward
/// growth, then a deferred `orderOut:` via `orderOutOwnWindow` on the glass
/// view (raw run loop — outside any GPUI borrow, which the hide-path docs
/// require). Returns true when it took over hiding; the caller must then
/// skip its own `orderOut:`.
///
/// # Safety
/// `window` must be a valid NSWindow on the main thread.
#[cfg(target_os = "macos")]
#[allow(dead_code)] // Reverted from the main hide path: deferring orderOut
                    // livelocked the hotkey gesture listener. Kept for a future exit animation
                    // that runs above the synchronous hide layer.
unsafe fn animate_tahoe_glass_disappearance(
    window: id,
    log_target: &str,
    window_name: &str,
) -> bool {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    if !(tahoe_liquid_glass_available() && crate::theme::get_cached_theme().is_vibrancy_enabled()) {
        return false;
    }
    let morph_enabled = crate::theme::get_cached_theme()
        .get_opacity()
        .glass_morph_duration
        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
        >= 0.02;
    if !morph_enabled {
        return false;
    }
    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return false;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view == nil {
        return false;
    }
    let visible: bool = msg_send![window, isVisible];
    if !visible {
        return false;
    }
    let frame: NSRect = msg_send![window, frame];
    if frame.size.width < 40.0 || frame.size.height < 40.0 {
        return false;
    }

    // Hiding during the appear settle must cancel the pending bounce, or it
    // would re-frame the window mid-fade.
    let _: () = msg_send![
        class!(NSObject),
        cancelPreviousPerformRequestsWithTarget: glass_view
        selector: sel!(settleOwnWindowFrame)
        object: nil
    ];

    // Slight outward growth while fading — Spotlight's exit reads as a very
    // fast "release" of the glass.
    let grow_x = frame.size.width * 0.025;
    let grow_y = frame.size.height * 0.025;
    let grown = NSRect::new(
        NSPoint::new(frame.origin.x - grow_x, frame.origin.y - grow_y),
        NSSize::new(
            frame.size.width + grow_x * 2.0,
            frame.size.height + grow_y * 2.0,
        ),
    );

    let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
    let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![ctx, setDuration: 0.11f64];
    let _: () = msg_send![ctx, setAllowsImplicitAnimation: true];
    if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
        let name = tahoe_ns_string("easeIn");
        if name != nil {
            let timing: id = msg_send![timing_class, functionWithName: name];
            if timing != nil {
                let _: () = msg_send![ctx, setTimingFunction: timing];
            }
        }
    }
    let animator: id = msg_send![window, animator];
    let _: () = msg_send![animator, setFrame: grown display: true];
    let _: () = msg_send![animator, setAlphaValue: 0.0f64];
    let _: () = msg_send![class!(NSAnimationContext), endGrouping];

    let _: () = msg_send![
        glass_view,
        performSelector: sel!(orderOutOwnWindow)
        withObject: nil
        afterDelay: 0.13f64
    ];

    logging::log(
        log_target,
        &format!(
            "{}: spotlight exit fade started (0.11s, orderOut deferred)",
            window_name
        ),
    );
    true
}

/// Install (or reuse) a native macOS 26 Tahoe `NSGlassEffectView` as the
/// backmost backdrop of the window's content view.
///
/// Design (Oracle-Session tahoe-native-glass-backdrop):
/// - Gated on `NSGlassEffectView` availability; no-op on older macOS.
/// - A dedicated, tagged, pass-through subclass inserted as a BACKMOST SIBLING
///   of content via `addSubview:positioned:NSWindowBelow relativeTo:nil`. It is
///   NOT a content wrapper (`setContentView:`), so the main-window footer blur
///   trio (NSVisualEffectView + hitTest:nil + transparent hitbox) is untouched.
/// - Idempotent: repeated configure passes find the same tagged view via
///   `viewWithTag:` instead of stacking duplicates.
/// - The view is NOT an NSVisualEffectView, so the vibrancy recursion that runs
///   before this call never reconfigures it as blur material.
///
/// # Safety
/// `window` must be a valid NSWindow on the main thread (checked + null-guarded).
#[cfg(target_os = "macos")]
unsafe fn configure_tahoe_window_backdrop(window: id, log_target: &str, window_name: &str) -> bool {
    use cocoa::appkit::{NSViewHeightSizable, NSViewWidthSizable};
    use cocoa::foundation::NSRect;

    if window.is_null() {
        return false;
    }
    if require_main_thread("configure_tahoe_window_backdrop") {
        return false;
    }

    // Debug-only: skip the glass backdrop to measure whether it contributes
    // anything visible underneath the NSVisualEffectViews.
    if std::env::var("SCRIPT_KIT_DEBUG_NO_GLASS").is_ok() {
        logging::log(
            log_target,
            &format!(
                "{}: DEBUG: Tahoe glass backdrop skipped via SCRIPT_KIT_DEBUG_NO_GLASS",
                window_name
            ),
        );
        return false;
    }

    let Some(glass_class) = tahoe_liquid_glass_class() else {
        logging::log(
            log_target,
            &format!(
                "{}: Tahoe NSGlassEffectView unavailable; native glass backdrop skipped",
                window_name
            ),
        );
        return false;
    };

    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        logging::log(
            log_target,
            &format!(
                "WARNING: {} has no contentView; Tahoe native glass backdrop skipped",
                window_name
            ),
        );
        return false;
    }

    let content_bounds: NSRect = msg_send![content_view, bounds];
    // Floating footer chrome: the main window's glass container ends above a
    // transparent bottom strip so the footer capsules float over the desktop.
    // The GPUI root subtracts the same strip via bottom padding.
    let bottom_strip = if window_name == "Main window" {
        f64::from(crate::footer_popup::main_window_float_footer_strip_height())
    } else {
        0.0
    };
    let backdrop_frame = NSRect::new(
        cocoa::foundation::NSPoint::new(
            content_bounds.origin.x,
            content_bounds.origin.y + bottom_strip,
        ),
        cocoa::foundation::NSSize::new(
            content_bounds.size.width,
            (content_bounds.size.height - bottom_strip).max(0.0),
        ),
    );
    let vev_count_before =
        tahoe_count_views_kind_of_excluding(content_view, class!(NSVisualEffectView), nil);

    let mut created = false;
    let mut glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view != nil {
        let is_glass: bool = msg_send![glass_view, isKindOfClass: glass_class];
        let superview: id = msg_send![glass_view, superview];
        if !is_glass || superview != content_view {
            logging::log(
                log_target,
                &format!(
                    "WARNING: {}: Tahoe glass backdrop tag collision (is_glass={}, direct_child={}); skipped",
                    window_name,
                    is_glass,
                    superview == content_view
                ),
            );
            return false;
        }
    } else {
        let Some(backdrop_class) = tahoe_glass_backdrop_view_class(glass_class) else {
            logging::log(
                log_target,
                &format!(
                    "WARNING: {}: failed to register ScriptKitTahoeGlassBackdropView",
                    window_name
                ),
            );
            return false;
        };
        let allocated: id = msg_send![backdrop_class, alloc];
        glass_view = msg_send![allocated, initWithFrame: backdrop_frame];
        if glass_view == nil {
            logging::log(
                log_target,
                &format!(
                    "WARNING: {}: failed to allocate NSGlassEffectView backdrop",
                    window_name
                ),
            );
            return false;
        }
        let identifier = tahoe_ns_string(TAHOE_GLASS_BACKDROP_IDENTIFIER);
        if identifier != nil {
            let _: () = msg_send![glass_view, setIdentifier: identifier];
        }
        let _: () =
            msg_send![glass_view, setAutoresizingMask: NSViewWidthSizable | NSViewHeightSizable];
        let _: () = msg_send![
            content_view,
            addSubview: glass_view
            positioned: NS_WINDOW_BELOW
            relativeTo: nil
        ];
        created = true;
    }

    let _: () = msg_send![glass_view, setFrame: backdrop_frame];
    let _: () =
        msg_send![glass_view, setAutoresizingMask: NSViewWidthSizable | NSViewHeightSizable];

    // Glass tint follows the theme's glass_tint_opacity slider (theme
    // background hue at that alpha). 0.0/None = untinted demo-parity glass.
    let tint_theme = crate::theme::get_cached_theme();
    let tint_alpha = f64::from(
        tint_theme
            .get_opacity()
            .glass_tint_opacity
            .unwrap_or(0.0)
            .clamp(0.0, 1.0),
    );
    let tint_applied = {
        let responds: bool = msg_send![glass_view, respondsToSelector: sel!(setTintColor:)];
        if responds {
            if tint_alpha > 0.004 {
                let hex = tint_theme.colors.background.main;
                let red = f64::from((hex >> 16) & 0xff) / 255.0;
                let green = f64::from((hex >> 8) & 0xff) / 255.0;
                let blue = f64::from(hex & 0xff) / 255.0;
                let color: id = msg_send![
                    class!(NSColor),
                    colorWithCalibratedRed: red
                    green: green
                    blue: blue
                    alpha: tint_alpha
                ];
                let _: () = msg_send![glass_view, setTintColor: color];
                true
            } else {
                let nil_color: id = nil;
                let _: () = msg_send![glass_view, setTintColor: nil_color];
                false
            }
        } else {
            false
        }
    };

    let corner_radius = {
        let radius = tahoe_content_corner_radius(content_view);
        // Floating footer chrome: the backdrop's bottom edge is now a visible
        // container edge (mid-window, not masked by the window shape), so it
        // must round itself to match the GPUI root container's `rounded(12.)`.
        if bottom_strip > 0.0 && radius <= 0.0 {
            12.0
        } else {
            radius
        }
    };
    let corner_applied = {
        let responds: bool = msg_send![glass_view, respondsToSelector: sel!(setCornerRadius:)];
        if responds {
            let _: () = msg_send![glass_view, setCornerRadius: corner_radius];
            true
        } else {
            false
        }
    };

    tahoe_pin_glass_backdrop_backmost(content_view, glass_view);
    let _: () = msg_send![glass_view, setNeedsDisplay: true];

    let vev_count_after =
        tahoe_count_views_kind_of_excluding(content_view, class!(NSVisualEffectView), glass_view);
    let (glass_count, glass_index, subview_count) =
        tahoe_glass_subview_audit(content_view, glass_class);
    let backmost = tahoe_glass_backdrop_is_backmost(content_view, glass_view);
    let index_label = glass_index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "none".to_string());

    logging::log(
        log_target,
        &format!(
            "{}: Tahoe NSGlassEffectView backdrop {} (glass_count={}, backmost={}, index={}, subviews={}, frame=({:.1},{:.1},{:.1},{:.1}), tint_applied={}, corner_applied={}, corner_radius={:.1}, vev_before={}, vev_after_excl_glass={})",
            window_name,
            if created { "installed" } else { "reused" },
            glass_count,
            backmost,
            index_label,
            subview_count,
            content_bounds.origin.x,
            content_bounds.origin.y,
            content_bounds.size.width,
            content_bounds.size.height,
            tint_applied,
            corner_applied,
            corner_radius,
            vev_count_before,
            vev_count_after,
        ),
    );

    if glass_count != 1 || !backmost || vev_count_before != vev_count_after {
        logging::log(
            log_target,
            &format!(
                "WARNING: {}: Tahoe glass backdrop audit FAILED (glass_count={}, backmost={}, vev_before={}, vev_after_excl_glass={})",
                window_name, glass_count, backmost, vev_count_before, vev_count_after
            ),
        );
    }

    created
}

/// Build an autoreleased NSString from a Rust `&str` (nil on interior NUL).
#[cfg(target_os = "macos")]
fn tahoe_ns_string(text: &str) -> id {
    let Ok(c_string) = std::ffi::CString::new(text) else {
        return nil;
    };
    // SAFETY: `c_string` is a valid NUL-terminated C string for this call.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c_string.as_ptr()] }
}

#[cfg(not(target_os = "macos"))]
fn configure_tahoe_window_backdrop(
    _window: *mut std::ffi::c_void,
    _log_target: &str,
    _window_name: &str,
) -> bool {
    false
}

/// Configure the actions popup window as a non-movable child window with vibrancy.
///
/// This function configures a popup window with:
/// - isMovable = false - prevents window dragging
/// - isMovableByWindowBackground = false - prevents dragging by clicking background
/// - Same window level as main window (NSFloatingWindowLevel = 3)
/// - hidesOnDeactivate = true - auto-hides when app loses focus
/// - hasShadow = true - shadow for depth perception
/// - Disabled restoration - no position caching
/// - animationBehavior = NSWindowAnimationBehaviorNone - no animation on close
/// - Appearance-aware vibrancy (VibrantDark/VibrantLight on views, window appearance nil) + POPOVER material for frosted glass effect
///
/// # Arguments
/// * `window` - The NSWindow pointer to configure
/// * `is_dark` - Whether to use dark vibrancy (true) or light vibrancy (false)
///
/// # Safety
///
/// - `window` must be a valid, non-null NSWindow pointer obtained from GPUI
///   window creation. The pointer is checked for null at entry.
/// - Must be called on the main thread (all AppKit property setters require it).
/// - NSAppearance pointers are nil-checked before use.
/// - Content view is nil-checked before recursing into visual effect views.
#[cfg(target_os = "macos")]
unsafe fn configure_attached_popup_window(
    window: id,
    is_dark: bool,
    log_target: &str,
    window_name: &str,
    morph_variant: GlassMorphVariant,
) {
    if window.is_null() {
        tracing::warn!(
            event = "attached_popup_configure.null_window",
            window_name,
            "Cannot configure null attached popup window"
        );
        return;
    }

    // Disable window dragging
    let _: () = msg_send![window, setMovable: false];
    let _: () = msg_send![window, setMovableByWindowBackground: false];

    // Popups are content-sized by GPUI (`set_inline_popup_window_bounds` / actions
    // resize helpers). Strip AppKit's resizable style mask so edge drags cannot
    // override the computed height.
    const NS_WINDOW_STYLE_MASK_RESIZABLE: u64 = 1 << 3;
    let style_mask: u64 = msg_send![window, styleMask];
    let non_resizable_mask = style_mask & !NS_WINDOW_STYLE_MASK_RESIZABLE;
    let _: () = msg_send![window, setStyleMask: non_resizable_mask];

    // Regression guard:
    // Detached child popups can still take mouse focus even when GPUI opens them
    // with `focus: false`. If AppKit promotes the child to the key panel on click,
    // the parent panel visually drops its active shadow even though our close/focus
    // policy keeps it open. `setBecomesKeyOnlyIfNeeded:true` keeps these popup
    // windows in the "clickable child" role instead of eagerly stealing key status.
    //
    // Keep this for Actions-style child popups unless we intentionally rework the
    // parent/child focus model and verify the shadow behavior again.
    let _: () = msg_send![window, setBecomesKeyOnlyIfNeeded: true];

    // Keep the level GPUI assigned (WindowKind::PopUp → NSPopUpMenuWindowLevel = 101).
    // Do NOT call setLevel here — any override downgrades the popup below the
    // main window which is also at 101. See CLAUDE.md "Window Level Rules".

    // NOTE: We intentionally do NOT set setHidesOnDeactivate:true here.
    // The main window is a non-activating panel (WindowKind::PopUp), so the app
    // is never "active" in the macOS sense. If we set hidesOnDeactivate, the
    // actions popup would immediately hide since the app isn't active.
    // Instead, we manage visibility ourselves via close_actions_window().

    // Disable close animation (NSWindowAnimationBehaviorNone = 2)
    // This prevents the white flash on dismiss
    let _: () = msg_send![window, setAnimationBehavior: NS_WINDOW_ANIMATION_BEHAVIOR_NONE];

    // Disable restoration
    let _: () = msg_send![window, setRestorable: false];

    // Disable frame autosave
    let empty_string: id = msg_send![class!(NSString), string];
    let _: () = msg_send![window, setFrameAutosaveName: empty_string];

    configure_window_vibrancy_common(window, log_target, window_name, is_dark, morph_variant);

    // SAFETY: `window` is a valid, non-null NSWindow pointer (checked at function entry).
    // orderFrontRegardless brings the popup visually above the main panel without
    // activating the app — same pattern as show_main_window_without_activation.
    let _: () = msg_send![window, orderFrontRegardless];
}

#[cfg(target_os = "macos")]
pub unsafe fn configure_actions_popup_window(window: id, is_dark: bool) {
    configure_attached_popup_window(
        window,
        is_dark,
        "ACTIONS",
        "Actions popup",
        GlassMorphVariant::ContentLayer,
    );
}

#[cfg(not(target_os = "macos"))]
pub fn configure_actions_popup_window(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

/// Configure an Agent Chat inline dropdown popup window with the same vibrancy path as
/// actions popups, including the detached AppKit shadow.
///
/// # Safety
/// Same invariants as `configure_actions_popup_window`.
#[cfg(target_os = "macos")]
pub unsafe fn configure_inline_dropdown_popup_window(window: id, is_dark: bool) {
    configure_attached_popup_window(
        window,
        is_dark,
        "POPUP",
        "Inline popup",
        GlassMorphVariant::ContentLayer,
    );

    // Inline dropdowns should read as native child popups with depth.
    let _: () = msg_send![window, setHasShadow: true];

    tracing::info!(
        target: "script_kit::popup",
        event = "inline_dropdown_popup_window_configured",
        dark = is_dark,
        "Configured inline dropdown popup window"
    );
}

#[cfg(not(target_os = "macos"))]
pub fn configure_inline_dropdown_popup_window(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

/// Configure the confirm popup window with the same vibrancy setup as the
/// actions popup. Reuses the shared popup vibrancy path so confirm dialogs
/// get native macOS blur.
///
/// # Safety
/// Same invariants as `configure_actions_popup_window`.
#[cfg(target_os = "macos")]
pub unsafe fn configure_confirm_popup_window(window: id, is_dark: bool) {
    configure_attached_popup_window(
        window,
        is_dark,
        "CONFIRM",
        "Confirm popup",
        GlassMorphVariant::ContentLayer,
    );
}

#[cfg(not(target_os = "macos"))]
pub fn configure_confirm_popup_window(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

/// Configure the child-attached shortcut recorder with its own morph receipt.
#[cfg(target_os = "macos")]
pub unsafe fn configure_shortcut_recorder_popup_window(window: id, is_dark: bool) {
    configure_attached_popup_window(
        window,
        is_dark,
        "SHORTCUT",
        "Shortcut recorder popup",
        GlassMorphVariant::ContentLayer,
    );
}

#[cfg(not(target_os = "macos"))]
pub fn configure_shortcut_recorder_popup_window(_window: *mut std::ffi::c_void, _is_dark: bool) {}

/// Configure the launcher footer popup window. This uses the shared popup
/// vibrancy path, disables its shadow, and ignores mouse events so the launcher
/// content beneath it remains interactive.
///
/// # Safety
/// Same invariants as `configure_actions_popup_window`.
#[cfg(target_os = "macos")]
pub unsafe fn configure_footer_popup_window(window: id, is_dark: bool) {
    // Footer behavior is intentionally unchanged and remains alpha-only; it
    // tracks the parent frame and is outside the floating-surface contract.
    configure_attached_popup_window(
        window,
        is_dark,
        "ACTIONS",
        "Actions popup",
        GlassMorphVariant::FadeOnly,
    );
    let _: () = msg_send![window, setIgnoresMouseEvents: true];

    // SAFETY: `window` is a valid NSWindow. The footer popup sits flush with
    // the parent window, so remove native panel depth that is appropriate for
    // centered confirm/actions popups but wrong for the footer strip.
    let content_view: id = msg_send![window, contentView];
    if content_view != nil {
        let layer: id = msg_send![content_view, layer];
        if layer != nil {
            let _: () = msg_send![layer, setCornerRadius: 0.0_f64];
        }
        let _: () = msg_send![content_view, setWantsLayer: true];
        let layer: id = msg_send![content_view, layer];
        if layer != nil {
            let _: () = msg_send![layer, setCornerRadius: 0.0_f64];
        }
    }
    let _: () = msg_send![window, setHasShadow: false];

    let title: id = msg_send![
        class!(NSString),
        stringWithUTF8String: c"Script Kit Footer".as_ptr()
    ];
    if title != nil {
        let _: () = msg_send![window, setTitle: title];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_footer_popup_window(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

// ============================================================================
// Secondary Window Vibrancy Configuration
// ============================================================================

/// Configure vibrancy for a secondary window (Notes, AI, etc.)
///
/// This applies the same VibrantDark appearance and NSVisualEffectView configuration
/// that the main window and actions popup use, ensuring consistent blur effect
/// across all Script Kit windows.
///
/// # Arguments
/// * `window` - The NSWindow pointer to configure
/// * `window_name` - Name for logging (e.g., "Notes", "AI")
/// * `is_dark` - Whether to use dark vibrancy (true) or light vibrancy (false)
///
/// # Safety
///
/// - `window` must be a valid, non-null NSWindow pointer obtained from GPUI
///   window creation. The pointer is checked for null at entry.
/// - Must be called on the main thread (all AppKit property setters require it).
/// - NSAppearance and content view pointers are nil-checked before use.
#[cfg(target_os = "macos")]
pub unsafe fn configure_secondary_window_vibrancy(window: id, window_name: &str, is_dark: bool) {
    if window.is_null() {
        logging::log(
            "PANEL",
            &format!(
                "WARNING: Cannot configure null window for {} vibrancy",
                window_name
            ),
        );
        return;
    }

    configure_window_vibrancy_common(
        window,
        "PANEL",
        window_name,
        is_dark,
        GlassMorphVariant::WindowFrame,
    );
}

/// Configure the live dictation overlay with the shared native material path.
///
/// The title is intentionally stable so appearance refresh can find the overlay
/// without treating it as a main-window footer surface.
///
/// # Safety
///
/// - `window` must be a valid, non-null NSWindow pointer obtained from GPUI
///   window creation. The pointer is checked for null at entry.
/// - Must be called on the main thread because AppKit property setters are not
///   thread-safe.
#[cfg(target_os = "macos")]
pub unsafe fn configure_dictation_overlay_window(window: id, is_dark: bool) {
    if window.is_null() {
        logging::log(
            "DICTATION",
            "WARNING: Cannot configure null Dictation overlay window vibrancy",
        );
        return;
    }

    tracing::info!(
        category = "DICTATION",
        is_dark,
        "Configuring dictation overlay shared native material"
    );
    configure_window_vibrancy_common(
        window,
        "DICTATION",
        "Dictation overlay",
        is_dark,
        GlassMorphVariant::WindowFrame,
    );

    let title: id = msg_send![
        class!(NSString),
        stringWithUTF8String: c"Script Kit Dictation".as_ptr()
    ];
    if title != nil {
        let _: () = msg_send![window, setTitle: title];
        tracing::info!(
            category = "DICTATION",
            title = "Script Kit Dictation",
            "Set dictation overlay NSWindow title for material refresh"
        );
    } else {
        tracing::warn!(
            category = "DICTATION",
            "Failed to allocate dictation overlay NSWindow title"
        );
    }
}

/// Configure a HUD overlay with the same native background and material path as
/// the main window while preserving HUD-specific level and input behavior in the
/// caller.
///
/// # Safety
///
/// - `window` must be a valid, non-null NSWindow pointer obtained from GPUI
///   window creation. The pointer is checked for null at entry.
/// - Must be called on the main thread because AppKit property setters are not
///   thread-safe.
/// - NSAppearance and content view pointers are nil-checked before use.
#[cfg(target_os = "macos")]
pub unsafe fn configure_hud_window_vibrancy(window: id, is_dark: bool) {
    if window.is_null() {
        logging::log("HUD", "WARNING: Cannot configure null HUD window vibrancy");
        return;
    }

    configure_window_vibrancy_common(
        window,
        "HUD",
        "HUD",
        is_dark,
        GlassMorphVariant::WindowFrame,
    );

    let title: id = msg_send![
        class!(NSString),
        stringWithUTF8String: c"Script Kit HUD".as_ptr()
    ];
    if title != nil {
        let _: () = msg_send![window, setTitle: title];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_secondary_window_vibrancy(
    _window: *mut std::ffi::c_void,
    _window_name: &str,
    _is_dark: bool,
) {
    // No-op on non-macOS platforms
}

#[cfg(not(target_os = "macos"))]
pub fn configure_dictation_overlay_window(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

#[cfg(not(target_os = "macos"))]
pub fn configure_hud_window_vibrancy(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

#[cfg(test)]
mod secondary_window_config_tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn glass_morph_tuning_drives_matching_frame_and_layer_geometry() {
        let tuning = super::glass_morph_tuning_from(0.28, 0.03).expect("morph enabled");
        let epsilon = 1e-12;
        assert!((tuning.start_scale_x - 1.06).abs() < epsilon);
        assert!((tuning.start_scale_y - 1.024).abs() < epsilon);
        assert!((tuning.squish_scale_x - 0.97).abs() < epsilon);
        assert!((tuning.squish_scale_y - 0.988).abs() < epsilon);
        assert!((tuning.phase1 - 0.14).abs() < epsilon);
        assert!((tuning.phase2 - 0.14).abs() < epsilon);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn glass_morph_tuning_respects_slider_disable_thresholds() {
        assert!(super::glass_morph_tuning_from(0.0, 0.03).is_none());
        assert!(super::glass_morph_tuning_from(0.28, 0.0).is_none());
    }

    #[test]
    fn actions_popup_focus_shadow_contract_uses_becomes_key_only_if_needed() {
        let source = include_str!("secondary_window_config.rs");
        assert!(
            source.contains("setBecomesKeyOnlyIfNeeded: true"),
            "actions-style child popups must keep becomesKeyOnlyIfNeeded enabled so clicking them does not visually demote the parent window"
        );
    }

    #[test]
    fn actions_popup_strips_appkit_resizable_style_mask() {
        let source = include_str!("secondary_window_config.rs");
        assert!(
            source.contains("NS_WINDOW_STYLE_MASK_RESIZABLE")
                && source.contains("setStyleMask: non_resizable_mask"),
            "content-sized child popups must not keep AppKit edge-resize affordances"
        );
    }

    #[test]
    fn hud_window_vibrancy_reuses_main_window_material_source() {
        let source = include_str!("secondary_window_config.rs");
        let start = source
            .find("pub unsafe fn configure_hud_window_vibrancy")
            .expect("HUD vibrancy function exists");
        let body = &source[start..];
        let body = body
            .split("#[cfg(not(target_os = \"macos\"))]")
            .next()
            .expect("HUD vibrancy function body");

        assert!(
            body.contains("configure_window_vibrancy_common(")
                && body.contains("\"HUD\",")
                && body.contains("GlassMorphVariant::WindowFrame"),
            "HUD window vibrancy must reuse the shared native background/material configuration"
        );
        assert!(
            body.contains("c\"Script Kit HUD\".as_ptr()")
                && super::should_refresh_secondary_window_appearance("Script Kit HUD"),
            "HUD windows need a stable title so theme/appearance refresh can retint them with the shared material path"
        );
        assert!(
            source.contains("fn current_window_material()")
                && source.contains("get_cached_theme().get_vibrancy().material"),
            "shared native window configuration must source material from the cached theme"
        );
    }

    #[test]
    fn secondary_appearance_refresh_title_predicate_covers_current_and_legacy_secondary_titles() {
        for title in [
            "Notes",
            "Mini AI",
            "Script Kit Agent Chat",
            "Script Kit Agent Chat",
            "Script Kit Notes",
            "Actions",
            "Script Kit Footer",
            "Script Kit Dictation",
            "Script Kit HUD",
        ] {
            assert!(
                super::should_refresh_secondary_window_appearance(title),
                "appearance refresh must cover secondary window title: {title}"
            );
        }
    }

    #[test]
    fn secondary_appearance_refresh_title_predicate_rejects_generic_titles() {
        for title in [
            "Agent Chat",
            "Notes Archive",
            "Mini",
            "AI",
            "Script Kit",
            "Script Kit Main",
            "Random User Window",
        ] {
            assert!(
                !super::should_refresh_secondary_window_appearance(title),
                "appearance refresh predicate must not match generic/non-secondary title: {title}"
            );
        }
    }

    // Source-contract guards for the native Tahoe NSGlassEffectView backdrop
    // (Oracle-Session tahoe-native-glass-backdrop). These do not prove runtime
    // pixels; they prevent a later "simplification" from removing the
    // safety-critical idempotence / backmost-insertion / pass-through / footer
    // non-mutation properties.
    #[test]
    fn tahoe_glass_backdrop_source_contract_is_native_idempotent_backmost_and_passthrough() {
        let source = include_str!("secondary_window_config.rs");
        assert!(
            source.contains("tahoe_liquid_glass_class()"),
            "Tahoe glass must remain gated by NSClassFromString availability"
        );
        assert!(
            source.contains("ScriptKitTahoeGlassBackdropView"),
            "Tahoe glass backdrop must use a dedicated subclass"
        );
        assert!(
            source.contains("viewWithTag: TAHOE_GLASS_BACKDROP_TAG"),
            "Tahoe glass backdrop must be idempotent via a stable tag lookup"
        );
        assert!(
            source.contains("initWithFrame: content_bounds"),
            "Tahoe glass backdrop must be created at contentView bounds"
        );
        assert!(
            source.contains("positioned: NS_WINDOW_BELOW") && source.contains("relativeTo: nil"),
            "Tahoe glass backdrop must be inserted below all existing contentView subviews"
        );
        assert!(
            source.contains("tahoe_glass_backdrop_hit_test") && source.contains("cocoa::base::nil"),
            "Tahoe glass backdrop must be pass-through for hit testing"
        );
        assert!(
            source.contains("sel!(setTintColor:)") && source.contains("setTintColor: tint_color"),
            "Tahoe glass backdrop must apply the theme tint through NSGlassEffectView tintColor"
        );
        assert!(
            source.contains("sel!(setCornerRadius:)")
                && source.contains("setCornerRadius: corner_radius"),
            "Tahoe glass backdrop must apply content corner radius when the selector exists"
        );
    }

    #[test]
    fn tahoe_glass_backdrop_source_contract_does_not_mutate_footer_or_wrap_content() {
        let source = include_str!("secondary_window_config.rs");
        let start = source
            .find("unsafe fn configure_tahoe_window_backdrop")
            .expect("configure_tahoe_window_backdrop exists");
        let body = &source[start
            ..source[start..]
                .find("#[cfg(not(target_os = \"macos\"))]")
                .map(|offset| start + offset)
                .unwrap_or(source.len())];
        assert!(
            !body.contains("setContentView:"),
            "Tahoe backdrop must not wrap/reparent GPUI or footer content"
        );
        assert!(
            !body.contains("setIgnoresMouseEvents:"),
            "NSGlassEffectView must be made pass-through with hitTest:nil, not NSWindow-only ignoresMouseEvents"
        );
        assert!(
            !body.contains("setTag:"),
            "NSView tag is read-only; use subclass tag override instead"
        );
        assert!(
            !body.contains("setMaterial:")
                && !body.contains("setBlendingMode:")
                && !body.contains("setState:")
                && !body.contains("setEmphasized:"),
            "NSGlassEffectView must not be configured with NSVisualEffectView material selectors"
        );
        assert!(
            !body.contains("FOOTER_") && !body.contains("script-kit-footer-effect"),
            "Tahoe backdrop configuration must not special-case or mutate footer internals"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn tahoe_glass_backdrop_ordering_constant_is_backmost() {
        assert_eq!(super::NS_WINDOW_BELOW, -1);
    }
}

fn should_refresh_secondary_window_appearance(title: &str) -> bool {
    const EXACT_SECONDARY_TITLES: &[&str] = &["Notes", "Mini AI", "Script Kit Agent Chat"];
    const EXISTING_SECONDARY_TITLE_MARKERS: &[&str] = &[
        "Script Kit Agent Chat",
        "Script Kit Notes",
        "Actions",
        "Script Kit Footer",
        "Script Kit Dictation",
        "Script Kit HUD",
    ];

    EXACT_SECONDARY_TITLES.contains(&title)
        || EXISTING_SECONDARY_TITLE_MARKERS
            .iter()
            .any(|marker| title.contains(marker))
}

/// Update appearance for all secondary windows (Notes, AI, Actions) when system appearance changes.
/// This ensures consistency across all windows when user toggles light/dark mode.
///
/// # Arguments
/// * `is_dark` - true for dark mode (VibrantDark), false for light mode (VibrantLight)
///
/// # Safety
/// - Must be called on the main thread
/// - Uses Objective-C runtime to enumerate and update windows
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn update_all_secondary_windows_appearance(is_dark: bool) {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: NSApplication.sharedApplication is always valid after app launch.
    // We iterate windows with count-bounded indices. Each window, title, and
    // UTF8String pointer is checked for nil/null before use.
    // Window appearance set to nil; appearance applied to NSVisualEffectViews instead.
    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let windows: id = msg_send![app, windows];
        if windows.is_null() {
            return;
        }
        let count: usize = msg_send![windows, count];

        logging::log(
            "APPEARANCE",
            &format!("Updating {} windows to is_dark={}", count, is_dark),
        );

        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex: i];
            if window == nil {
                continue;
            }

            // Get window title to identify secondary windows
            let title: id = msg_send![window, title];
            if title == nil {
                continue;
            }

            let title_str: *const std::os::raw::c_char = msg_send![title, UTF8String];
            if title_str.is_null() {
                continue;
            }

            let title_string = std::ffi::CStr::from_ptr(title_str)
                .to_string_lossy()
                .to_string();

            // Match secondary window titles
            if should_refresh_secondary_window_appearance(&title_string) {
                // Clear window appearance so GPUI can detect system appearance changes.
                // Set appearance on individual NSVisualEffectViews instead.
                let _: () = msg_send![window, setAppearance: nil];

                // Walk view hierarchy and set appearance + material on each NSVisualEffectView
                let content_view: id = msg_send![window, contentView];
                if content_view != nil {
                    let mut vev_count = 0;
                    let material = current_window_material();
                    configure_visual_effect_views_recursive(
                        content_view,
                        &mut vev_count,
                        is_dark,
                        material,
                    );
                    configure_tahoe_window_backdrop(window, "APPEARANCE", &title_string);
                    logging::log(
                        "APPEARANCE",
                        &format!(
                            "Updated window '{}': cleared window appearance, configured {} NSVisualEffectView(s) for {} using {}",
                            title_string,
                            vev_count,
                            if is_dark { "dark" } else { "light" },
                            current_window_material_name(material),
                        ),
                    );
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn update_all_secondary_windows_appearance(_is_dark: bool) {
    // No-op on non-macOS platforms
}

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

// Re-export display/coordinate helpers from the unified display module.
pub use self::display::{
    clamp_to_visible, display_for_point, flip_y, get_active_display, get_global_mouse_position,
    get_macos_displays, get_macos_visible_displays, prefers_reduced_motion, primary_screen_height,
    VisibleDisplayBounds,
};

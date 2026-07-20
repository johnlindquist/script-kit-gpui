// ============================================================================
// Actions Popup Window Configuration
// ============================================================================

// SAFETY: Caller must pass a valid NSWindow pointer on the main thread.
// The function nil-checks all derived pointers (content view, appearance).
#[cfg(target_os = "macos")]
unsafe fn configure_window_vibrancy_common(
    window: id,
    log_target: &str,
    window_name: &str,
    is_dark: bool,
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
    if glass_created {
        animate_tahoe_glass_appearance(window, log_target, window_name);
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

/// Morph a freshly created glass backdrop into place: the sheet starts
/// slightly inset with capsule-heavy corners and eases out to the full
/// window — the liquid-glass appear from the demo. Runs via the AppKit
/// animator so the glass material (refraction included) animates live.
///
/// # Safety
/// `window` must be a valid NSWindow on the main thread.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_appearance(window: id, log_target: &str, window_name: &str) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view == nil {
        return;
    }
    let bounds: NSRect = msg_send![content_view, bounds];
    if bounds.size.width < 40.0 || bounds.size.height < 40.0 {
        return;
    }
    let responds_radius: bool = msg_send![glass_view, respondsToSelector: sel!(setCornerRadius:)];
    let end_radius: f64 = if responds_radius {
        msg_send![glass_view, cornerRadius]
    } else {
        0.0
    };

    // Collapsed start: centered at ~82% size, capsule-heavy corners.
    let inset_x = bounds.size.width * 0.09;
    let inset_y = bounds.size.height * 0.09;
    let start = NSRect::new(
        NSPoint::new(bounds.origin.x + inset_x, bounds.origin.y + inset_y),
        NSSize::new(
            bounds.size.width - inset_x * 2.0,
            bounds.size.height - inset_y * 2.0,
        ),
    );
    let start_radius = (start.size.width.min(start.size.height) * 0.5).min(48.0);
    let _: () = msg_send![glass_view, setFrame: start];
    if responds_radius {
        let _: () = msg_send![glass_view, setCornerRadius: start_radius];
    }

    let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
    let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![ctx, setDuration: 0.34f64];
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
    let animator: id = msg_send![glass_view, animator];
    let _: () = msg_send![animator, setFrame: bounds];
    if responds_radius {
        let _: () = msg_send![animator, setCornerRadius: end_radius];
    }
    let _: () = msg_send![class!(NSAnimationContext), endGrouping];

    logging::log(
        log_target,
        &format!(
            "{}: glass appear morph started (0.34s ease-out, radius {:.0} -> {:.0})",
            window_name, start_radius, end_radius
        ),
    );
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
        glass_view = msg_send![allocated, initWithFrame: content_bounds];
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

    let _: () = msg_send![glass_view, setFrame: content_bounds];
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

    let corner_radius = tahoe_content_corner_radius(content_view);
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
pub unsafe fn configure_actions_popup_window(window: id, is_dark: bool) {
    if window.is_null() {
        tracing::warn!(
            event = "actions_popup_configure.null_window",
            "Cannot configure null window as actions popup"
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

    configure_window_vibrancy_common(window, "ACTIONS", "Actions popup", is_dark);

    // SAFETY: `window` is a valid, non-null NSWindow pointer (checked at function entry).
    // orderFrontRegardless brings the popup visually above the main panel without
    // activating the app — same pattern as show_main_window_without_activation.
    let _: () = msg_send![window, orderFrontRegardless];
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
    configure_actions_popup_window(window, is_dark);

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
    configure_actions_popup_window(window, is_dark);
}

#[cfg(not(target_os = "macos"))]
pub fn configure_confirm_popup_window(_window: *mut std::ffi::c_void, _is_dark: bool) {
    // No-op on non-macOS platforms
}

/// Configure the launcher footer popup window. This uses the shared popup
/// vibrancy path, disables its shadow, and ignores mouse events so the launcher
/// content beneath it remains interactive.
///
/// # Safety
/// Same invariants as `configure_actions_popup_window`.
#[cfg(target_os = "macos")]
pub unsafe fn configure_footer_popup_window(window: id, is_dark: bool) {
    configure_actions_popup_window(window, is_dark);
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

    configure_window_vibrancy_common(window, "PANEL", window_name, is_dark);
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
    configure_window_vibrancy_common(window, "DICTATION", "Dictation overlay", is_dark);

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

    configure_window_vibrancy_common(window, "HUD", "HUD", is_dark);

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
            body.contains("configure_window_vibrancy_common(window, \"HUD\", \"HUD\", is_dark)"),
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

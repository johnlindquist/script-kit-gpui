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
    // Same visible-entry start policy as the frame-based variants: a visible
    // window never starts below tuning.start_alpha.
    let _: () = msg_send![window, setAlphaValue: tuning.start_alpha];
    record_native_glass_entry_span(window, fade);
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
            "event=glass_morph window={} variant={} phase=enter duration={:.2}s start_alpha={:.2}",
            window_name,
            GlassMorphVariant::FadeOnly.log_name(),
            fade,
            tuning.start_alpha
        ),
    );
}

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
        schedule_child_morph_settle(parent, window, tuning.total_entry_duration() + 0.08);
    }

    animate_tahoe_glass_appearance_profiled(
        window,
        log_target,
        window_name,
        GlassEntrySurface::ChildPopup,
    );
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
/// rect while the window remains at a materially stable full alpha,
/// so the glass backdrop AND the GPUI content arrive together without exposing
/// the desktop as the capsule color. The glass tracks the window via its
/// autoresizing mask during the frame animation.
///
/// Duration and inset come from the theme's glass morph sliders; either at
/// (near) zero disables the morph.
///
/// # Safety
/// `window` must be a valid NSWindow on the main thread.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_main_appearance(window: id, log_target: &str, window_name: &str) {
    animate_tahoe_glass_appearance_profiled(
        window,
        log_target,
        window_name,
        GlassEntrySurface::Main,
    )
}

/// Notes, Dictation, HUD — free-standing secondary windows. Keeps the
/// Spotlight-derived shrink-in profile.
/// Window-frame morphs resolve their surface at the shared owner: the
/// physical main NSWindow gets `GlassEntrySurface::Main` (and with it the
/// soft-materialize onset), every other window-frame surface remains
/// `FreeStandingSecondary` (2026-08-13 onset retune).
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_window_frame_appearance(
    window: id,
    log_target: &str,
    window_name: &str,
) {
    let surface = glass_entry_surface_for_window_frame(
        crate::window_manager::get_main_window() == Some(window),
    );
    animate_tahoe_glass_appearance_profiled(window, log_target, window_name, surface)
}

#[cfg(target_os = "macos")]
#[allow(
    dead_code,
    reason = "the native secondary-window compatibility entry retains its locked glass motion profile"
)]
unsafe fn animate_tahoe_glass_secondary_appearance(
    window: id,
    log_target: &str,
    window_name: &str,
) {
    animate_tahoe_glass_appearance_profiled(
        window,
        log_target,
        window_name,
        GlassEntrySurface::FreeStandingSecondary,
    )
}

/// Run the entry morph under the named surface's policy.
///
/// `GlassEntryDirection::ShrinkIn` is the Spotlight outset enter (start wider,
/// compress below final, rebound out) used by the main window and
/// free-standing secondaries. `GrowIn` is the child-popup direction — start
/// BELOW final size, overshoot slightly past it, settle back — same phases and
/// curves, inverted travel, so live GPUI reflow reads as the menu materializing
/// instead of a squeeze.
///
/// The surface is a PARAMETER, never inferred from `window_name`: the label is
/// log text and must not steer geometry.
#[cfg(target_os = "macos")]
unsafe fn animate_tahoe_glass_appearance_profiled(
    window: id,
    log_target: &str,
    window_name: &str,
    surface: GlassEntrySurface,
) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};

    let policy = glass_entry_policy(surface);
    let outset_sign = policy.direction.geometry_sign();

    // Every show is an exit supersession boundary. Invalidate delayed removal,
    // cancel old settle/order-out callbacks, and clear common-ancestor effects
    // before preparing the next entry frame. Alpha is PRESERVED here: the
    // restoring cancel would flash a full-alpha extreme frame before the
    // start alpha and start geometry are installed below.
    cancel_ns_window_exit_dematerialize_impl(window, false);

    // The hide path parks the window at alpha 0 so no show path can flash a
    // full-alpha frame. Every early exit below must therefore restore alpha,
    // or a skipped morph would leave the window invisible.
    let restore_alpha = |window: id| {
        let _: () = msg_send![window, setAlphaValue: 1.0f64];
    };

    // Instrumentation only: every skipped morph names its reason, so an
    // absent `phase=enter` log line is diagnosable instead of ambiguous
    // ("old binary or missing log capture" vs a silently skipped morph —
    // the 2026-07-26 runtime-contract false INVALID_SETUP).
    let log_skip = |reason: &str| {
        logging::log(
            log_target,
            &format!(
                "event=glass_morph_skip window={} phase=enter reason={}",
                window_name, reason
            ),
        );
    };
    let Some(tuning) = glass_morph_tuning() else {
        restore_alpha(window);
        log_skip("sliders_disabled");
        return; // morph disabled via theme sliders
    };
    let is_main_window = crate::window_manager::get_main_window() == Some(window);
    if is_main_window && glass_morph_recently_started() {
        restore_alpha(window);
        log_skip("recently_started_debounce");
        return;
    }

    let final_frame: NSRect = msg_send![window, frame];
    if final_frame.size.width < 40.0 || final_frame.size.height < 40.0 {
        restore_alpha(window);
        log_skip("frame_too_small");
        return;
    }

    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        restore_alpha(window);
        log_skip("no_content_view");
        return;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view == nil {
        restore_alpha(window);
        log_skip("no_glass_view");
        return;
    }

    // Spotlight enters by SHRINKING into place: start larger than final
    // (the "inset" slider is the start outset) and glide down. Measured
    // from the real Spotlight: the morph is WIDTH-DOMINANT — height locks
    // early and barely undershoots — so the vertical deltas are damped.
    let travel = glass_entry_travel(
        policy.travel,
        final_frame.size.width,
        tuning.inset_fraction,
        GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
    );
    let outset_x = travel.start_per_side;
    let outset_y = final_frame.size.height * (tuning.inset_fraction * GLASS_MORPH_VERTICAL_DAMPING);
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

    // Soft-materialize onset geometry (2026-08-13 retune): the main window's
    // FIRST PHOTON is wider than the calibrated visible-tail start (measured
    // 103.05% vs 101.2%) and eases into the tail frame inside the material
    // prefix. Tail-aligned surfaces resolve to the same frame with a zero
    // duration, i.e. exactly the previous behavior.
    let onset_outset_x = policy
        .onset_geometry
        .start_per_side(final_frame.size.width, outset_x);
    let onset_start = NSRect::new(
        NSPoint::new(
            final_frame.origin.x - onset_outset_x * outset_sign,
            final_frame.origin.y - outset_y * outset_sign,
        ),
        NSSize::new(
            final_frame.size.width + onset_outset_x * 2.0 * outset_sign,
            final_frame.size.height + outset_y * 2.0 * outset_sign,
        ),
    );
    let onset_geometry_duration = policy.onset_geometry.duration();

    // Alpha BEFORE geometry: the extreme calibration frame must never be
    // displayed above the visible-entry start alpha.
    let _: () = msg_send![window, setAlphaValue: tuning.start_alpha];
    let _: () = msg_send![window, setFrame: onset_start display: true];
    if onset_geometry_duration > 0.0 {
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let onset_geometry_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![onset_geometry_ctx, setDuration: onset_geometry_duration];
        let _: () = msg_send![onset_geometry_ctx, setAllowsImplicitAnimation: true];
        if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
            let name = tahoe_ns_string("easeOut");
            if name != nil {
                let timing: id = msg_send![timing_class, functionWithName: name];
                if timing != nil {
                    let _: () = msg_send![onset_geometry_ctx, setTimingFunction: timing];
                }
            }
        }
        let onset_geometry_animator: id = msg_send![window, animator];
        let _: () = msg_send![onset_geometry_animator, setFrame: start display: true];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    }
    record_native_glass_entry_span(window, tuning.total_entry_duration());
    // Instrumentation only (runtime-contract cross-check, no animation value
    // is changed): the moment the enter morph is armed, on the same host
    // clock the lifecycle receipts use.
    let configured_at_host_time_ns = crate::platform::host_clock::host_time_ns();
    // HISTORICAL OVERLOAD: `settle_duration_ns` has only ever carried the
    // VISIBLE TAIL (105ms at default), even though the receipt struct and
    // `record_native_glass_entry_span` both track the full onset+tail entry
    // (149ms). The name is kept for receipt compatibility; every new consumer
    // must read `visible_tail_duration_ns` / `total_entry_duration_ns` below,
    // which state which is which.
    let settle_duration_ns = (tuning.duration * 1_000_000_000.0) as u64;
    let visible_tail_duration_ns =
        (tuning.visible_tail_duration() * 1_000_000_000.0).round() as u64;
    let total_entry_duration_ns = (tuning.total_entry_duration() * 1_000_000_000.0).round() as u64;
    // Whether AppKit still considered this a child window at the moment the
    // morph was armed. `animate_tahoe_glass_child_appearance` intends to
    // detach first, but the Actions open path configures the popup BEFORE
    // attaching it, so the real state here is an empirical question — which is
    // exactly what this field exists to answer.
    let parent_window_at_arm: id = msg_send![window, parentWindow];
    let parent_attached_at_arm = parent_window_at_arm != nil;
    let native_parent_window_number: i64 = if parent_attached_at_arm {
        msg_send![parent_window_at_arm, windowNumber]
    } else {
        0
    };

    // Record the in-flight duration so sibling windows (footer overlay) can
    // hide until the morph settles (glass_morph_remaining).
    if is_main_window {
        GLASS_MORPH_LAST_DURATION_MS.store(
            (tuning.total_entry_duration() * 1000.0) as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    // Squish target: compress BELOW the final size — the released-elastic
    // physics the Spotlight enter has. Measured: −1.3% of TOTAL width
    // (squish_fraction is per side; the ×2 below doubles it).
    let squish_fraction = (tuning.inset_fraction * GLASS_MORPH_SQUISH_FACTOR)
        .clamp(GLASS_MORPH_MIN_SQUISH, GLASS_MORPH_MAX_SQUISH);
    let squish_x = travel.extreme_per_side;
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

    // Phase 1: wide -> squished-under-final. The compression is EASE-OUT
    // (ends at zero velocity — no explicit hold), phase-locked with an
    // independent ease-out alpha ramp 0.85 -> 0.99 over its own (shorter)
    // duration. Separate AppKit contexts because frame and alpha have
    // different durations; alpha must NOT reach 1.0 while the window is
    // wider than natural (Spotlight: "never fully opaque while still larger
    // than its natural size").
    let phase1 = tuning.phase1;
    let phase2 = tuning.phase2;

    // Cancel any pending settle from an interrupted previous morph.
    let _: () = msg_send![
        class!(NSObject),
        cancelPreviousPerformRequestsWithTarget: glass_view
        selector: sel!(settleOwnWindowFrame)
        object: nil
    ];

    // ── Material onset (T=0..88ms): the glass materializes Clear→Regular
    // with its tint at a CONSTANT 0.85 NSWindow alpha — the measured
    // Spotlight prefix. Best-effort on public selectors only; when the
    // runtime lacks them the onset degrades to the plain scheduled tail.
    let style_supported: bool = msg_send![glass_view, respondsToSelector: sel!(setStyle:)];
    let tint_supported: bool = msg_send![glass_view, respondsToSelector: sel!(setTintColor:)];
    // `GlassEntryOnsetPolicy::Full` is the only policy production resolves
    // today, so this gate is always open; it exists so a `TailOnly` surface
    // can skip the prefix without another branch being invented later.
    let onset_supported =
        style_supported && tint_supported && policy.onset == GlassEntryOnsetPolicy::Full;
    if onset_supported {
        // Seed clear/untinted with implicit actions disabled.
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let seed_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![seed_ctx, setDuration: 0.0f64];
        let _: () = msg_send![glass_view, setStyle: 1isize]; // NSGlassEffectViewStyleClear
        let nil_color: id = nil;
        let _: () = msg_send![glass_view, setTintColor: nil_color];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
        // One 88ms animation group back to the resolved production material.
        let final_style = resolve_native_glass_style(
            &crate::theme::get_cached_theme(),
            NativeGlassSurfaceRole::WindowBackdrop,
        );
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let onset_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![onset_ctx, setDuration: tuning.material_onset_duration];
        let _: () = msg_send![onset_ctx, setAllowsImplicitAnimation: true];
        let curve = timing_function_with_control_points(
            GLASS_MATERIAL_ONSET_C1.0,
            GLASS_MATERIAL_ONSET_C1.1,
            GLASS_MATERIAL_ONSET_C2.0,
            GLASS_MATERIAL_ONSET_C2.1,
        );
        if curve != nil {
            let _: () = msg_send![onset_ctx, setTimingFunction: curve];
        }
        let glass_animator: id = msg_send![glass_view, animator];
        let _: () = msg_send![glass_animator, setStyle: 0isize]; // Regular
                                                                 // Resolved production tint, same construction as
                                                                 // apply_native_glass_style_with_reason.
        let red = f64::from((final_style.signature.tint_rgb >> 16) & 0xff) / 255.0;
        let green = f64::from((final_style.signature.tint_rgb >> 8) & 0xff) / 255.0;
        let blue = f64::from(final_style.signature.tint_rgb & 0xff) / 255.0;
        let tint: id = msg_send![
            class!(NSColor),
            colorWithCalibratedRed: red
            green: green
            blue: blue
            alpha: f64::from(final_style.effective_tint_alpha)
        ];
        if tint != nil {
            let _: () = msg_send![glass_animator, setTintColor: tint];
        }
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    }

    // Seed GPUI content roots (never the contentView or the glass itself) at
    // the Spotlight-measured first-photon presence floor: content is faintly
    // readable from photon 1 and blooms to full over the content fade.
    // Transparent GPUI pixels expose the glass, never bare desktop.
    let content_start_alpha = glass_entry_content_start_alpha();
    let mut content_targets: Vec<(usize, f64)> = Vec::new();
    let subviews: id = msg_send![content_view, subviews];
    if subviews != nil {
        let count: usize = msg_send![subviews, count];
        for index in 0..count {
            let child: id = msg_send![subviews, objectAtIndex: index];
            if child == nil || child == glass_view {
                continue;
            }
            let is_glass: bool = if let Some(class) = objc::runtime::Class::get("NSGlassEffectView")
            {
                msg_send![child, isKindOfClass: class]
            } else {
                false
            };
            let is_container: bool =
                if let Some(class) = objc::runtime::Class::get("NSGlassEffectContainerView") {
                    msg_send![child, isKindOfClass: class]
                } else {
                    false
                };
            if is_glass || is_container {
                continue;
            }
            let alpha: f64 = msg_send![child, alphaValue];
            if alpha <= 0.001 {
                continue;
            }
            let _: () = msg_send![child, setAlphaValue: alpha * content_start_alpha];
            content_targets.push((child as usize, alpha));
        }
    }
    // The same-host footer container stays OUT of the GPUI content fade.
    // It participates only through the owning NSWindow's frame and alpha;
    // per-capsule defocus is installed below on clipped NSGlassEffectView
    // layers, never on this gap-spanning alpha target.
    let footer_entry_policy = main_footer_entry_material_policy();
    let footer_entry_alpha_target =
        crate::footer_popup::main_window_footer_entry_alpha_target(window);
    if footer_entry_alpha_target != nil {
        let alpha: f64 = msg_send![footer_entry_alpha_target, alphaValue];
        if alpha > 0.001 || !footer_entry_policy.enroll_in_content_fade {
            let _: () = msg_send![
                footer_entry_alpha_target,
                setAlphaValue: footer_entry_policy.target_alpha
            ];
            if footer_entry_policy.enroll_in_content_fade {
                content_targets.push((footer_entry_alpha_target as usize, alpha));
            }
        }
    }
    let content_root_count = content_targets.len();

    // ── Footer capsule material parity (2026-08-13 retune, user report: the
    // floating buttons "don't match the blur of the main window"). Three
    // per-capsule effects, all clipped to each rounded NSGlassEffectView:
    //   1. the SAME 12pt→0 defocus as the main backdrop (installed below);
    //   2. the SAME Clear→Regular + tint material ramp — this ramp, not the
    //      defocus, is the bloom the eye reads on the main stage;
    //   3. the capsule's foreground contentView joins the shared content fade
    //      at the presence floor, so labels bloom in instead of popping crisp.
    // The gap-spanning container/hints host never receives any of these.
    let footer_capsules = if surface == GlassEntrySurface::Main {
        crate::footer_popup::main_window_footer_entry_capsules(window)
    } else {
        Vec::new()
    };
    let footer_blur_duration = tuning.material_onset_duration;
    let footer_capsule_count = footer_capsules.len();
    let mut footer_blurred_capsule_count = 0usize;
    let mut footer_material_ramp_count = 0usize;
    let mut footer_foreground_fade_count = 0usize;
    for capsule in footer_capsules {
        if footer_entry_policy.defocus_radius > 0.0 {
            let installed_radius = ramp_entry_defocus(
                capsule,
                footer_blur_duration,
                footer_entry_policy.defocus_radius,
                log_target,
            );
            if installed_radius == footer_entry_policy.defocus_radius {
                footer_blurred_capsule_count += 1;
            }
        }
        if footer_entry_policy.material_onset_ramp && onset_supported {
            let capsule_style_supported: bool =
                msg_send![capsule, respondsToSelector: sel!(setStyle:)];
            let capsule_tint_supported: bool =
                msg_send![capsule, respondsToSelector: sel!(tintColor)];
            if capsule_style_supported && capsule_tint_supported {
                // Capture the capsule's resolved production material, seed
                // Clear/untinted with implicit actions disabled, then run one
                // onset-length animation group back — the exact main-backdrop
                // choreography, scoped to this capsule.
                let final_capsule_style: isize = msg_send![capsule, style];
                let final_capsule_tint: id = msg_send![capsule, tintColor];
                let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
                let seed_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
                let _: () = msg_send![seed_ctx, setDuration: 0.0f64];
                let _: () = msg_send![capsule, setStyle: 1isize]; // Clear
                let nil_color: id = nil;
                let _: () = msg_send![capsule, setTintColor: nil_color];
                let _: () = msg_send![class!(NSAnimationContext), endGrouping];
                let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
                let ramp_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
                let _: () = msg_send![ramp_ctx, setDuration: tuning.material_onset_duration];
                let _: () = msg_send![ramp_ctx, setAllowsImplicitAnimation: true];
                let curve = timing_function_with_control_points(
                    GLASS_MATERIAL_ONSET_C1.0,
                    GLASS_MATERIAL_ONSET_C1.1,
                    GLASS_MATERIAL_ONSET_C2.0,
                    GLASS_MATERIAL_ONSET_C2.1,
                );
                if curve != nil {
                    let _: () = msg_send![ramp_ctx, setTimingFunction: curve];
                }
                let capsule_animator: id = msg_send![capsule, animator];
                let _: () = msg_send![capsule_animator, setStyle: final_capsule_style];
                if final_capsule_tint != nil {
                    let _: () = msg_send![capsule_animator, setTintColor: final_capsule_tint];
                }
                let _: () = msg_send![class!(NSAnimationContext), endGrouping];
                footer_material_ramp_count += 1;
            }
        }
        if footer_entry_policy.foreground_content_fade {
            let foreground: id = msg_send![capsule, contentView];
            if foreground != nil {
                let alpha: f64 = msg_send![foreground, alphaValue];
                if alpha > 0.001 {
                    let _: () = msg_send![foreground, setAlphaValue: alpha * content_start_alpha];
                    content_targets.push((foreground as usize, alpha));
                    footer_foreground_fade_count += 1;
                }
            }
        }
    }
    let footer_blur_radius =
        if footer_capsule_count > 0 && footer_blurred_capsule_count == footer_capsule_count {
            footer_entry_policy.defocus_radius
        } else {
            0.0
        };

    if let Ok(mut guard) = GLASS_ENTRY_CONTENT_TARGETS.lock() {
        guard.insert(window as usize, content_targets);
    }

    // Stash the tail and schedule: content reveal @53ms, tail @88ms.
    if let Ok(mut guard) = GLASS_MORPH_TAIL_TARGETS.lock() {
        guard.insert(
            window as usize,
            GlassMorphTailTarget {
                squish_frame: [
                    squish.origin.x,
                    squish.origin.y,
                    squish.size.width,
                    squish.size.height,
                ],
                final_frame: [
                    final_frame.origin.x,
                    final_frame.origin.y,
                    final_frame.size.width,
                    final_frame.size.height,
                ],
                phase1,
                phase2,
                alpha_ramp_duration: tuning.alpha_ramp_duration,
                phase1_alpha_target: tuning.phase1_alpha_target,
                alpha_finish_duration: tuning.alpha_finish_duration,
            },
        );
    }
    // Entry defocus: the main backdrop resolves inside the material prefix;
    // popup and secondary surfaces keep the historical full-entry ramp.
    let entry_blur_duration = surface.entry_blur_duration(tuning);
    let entry_blur_radius = ramp_entry_defocus(
        glass_view,
        entry_blur_duration,
        surface.entry_blur_radius(),
        log_target,
    );

    let _: () = msg_send![
        glass_view,
        performSelector: sel!(revealOwnWindowEntryContent)
        withObject: nil
        afterDelay: tuning.content_hold_duration
    ];
    let _: () = msg_send![
        glass_view,
        performSelector: sel!(beginOwnWindowEntryTail)
        withObject: nil
        afterDelay: tuning.material_onset_duration
    ];
    logging::log(
        log_target,
        &format!(
            "event=native_glass_entry_onset primitive=material_parameters supported={} entry_blur_radius={:.2} entry_blur_to_radius=0.00 footer_blur_radius={:.2} footer_blur_to_radius=0.00 footer_blur_scope={} footer_blur_duration_ns={} footer_capsule_count={} footer_blurred_capsule_count={} footer_material_ramp_count={} footer_foreground_fade_count={} footer_enrolled={} entry_blur_duration_ns={} onset_start_width_scale={:.6} tail_start_width_scale={:.6} onset_geometry_duration_ns={} from_style=clear to_style=regular duration_ns={} content_root_count={} content_hold_ns={} content_fade_ns={} content_start_alpha={:.2} window_alpha={:.2}",
            onset_supported,
            entry_blur_radius,
            footer_blur_radius,
            footer_entry_policy.defocus_scope.log_name(),
            (footer_blur_duration * 1_000_000_000.0).round() as u64,
            footer_capsule_count,
            footer_blurred_capsule_count,
            footer_material_ramp_count,
            footer_foreground_fade_count,
            footer_entry_policy.enroll_in_content_fade,
            (entry_blur_duration * 1_000_000_000.0).round() as u64,
            onset_start.size.width / final_frame.size.width,
            start.size.width / final_frame.size.width,
            (onset_geometry_duration * 1_000_000_000.0).round() as u64,
            (tuning.material_onset_duration * 1_000_000_000.0).round() as u64,
            content_root_count,
            (tuning.content_hold_duration * 1_000_000_000.0).round() as u64,
            (tuning.content_fade_duration * 1_000_000_000.0).round() as u64,
            content_start_alpha,
            tuning.start_alpha,
        ),
    );

    logging::log(
        log_target,
        &format!(
            "event=glass_morph window={} variant={} phase=enter surface_profile={} direction={} travel_policy={} final_width_pt={:.2} start_travel_per_side_pt={:.4} extreme_travel_per_side_pt={:.4} visible_tail_duration_ns={} total_entry_duration_ns={} parent_attached_at_arm={} native_parent_window_number={} duration={:.2}s inset={:.3} start_alpha={:.2} start_alpha_bits={:016x} settle_duration_ns={} configured_at_host_time_ns={} expected_settle_deadline_ns={} frames={}x{}->{}x{}->{}x{} start_scale_x={:.6} start_scale_y={:.6} squish_scale_x={:.6} squish_scale_y={:.6} phase1_ns={} hold_ns={} phase2_ns={} alpha_phase1_target={:.6} alpha_ramp_ns={} alpha_finish_ns={} geometry_curve=easeOut rebound_curve=easeInEaseOut alpha_curve=easeOut",
            window_name,
            GlassMorphVariant::WindowFrame.log_name(),
            surface.log_name(),
            policy.direction.log_name(),
            policy.travel.log_name(),
            final_frame.size.width,
            travel.start_per_side,
            travel.extreme_per_side,
            visible_tail_duration_ns,
            total_entry_duration_ns,
            parent_attached_at_arm,
            native_parent_window_number,
            tuning.duration,
            tuning.inset_fraction,
            tuning.start_alpha,
            tuning.start_alpha.to_bits(),
            settle_duration_ns,
            configured_at_host_time_ns,
            configured_at_host_time_ns.saturating_add(settle_duration_ns),
            start.size.width as i64,
            start.size.height as i64,
            squish.size.width as i64,
            squish.size.height as i64,
            final_frame.size.width as i64,
            final_frame.size.height as i64,
            // Direction-adjusted EFFECTIVE scales: grow-in inverts the
            // travel (start below final, overshoot above), so the logged
            // exact fields must reflect the frames actually animated.
            1.0 + (tuning.start_scale_x - 1.0) * outset_sign,
            1.0 + (tuning.start_scale_y - 1.0) * outset_sign,
            1.0 + (tuning.squish_scale_x - 1.0) * outset_sign,
            1.0 + (tuning.squish_scale_y - 1.0) * outset_sign,
            (phase1 * 1_000_000_000.0).round() as u64,
            (GLASS_MORPH_SQUISH_HOLD * 1_000_000_000.0).round() as u64,
            (phase2 * 1_000_000_000.0).round() as u64,
            tuning.phase1_alpha_target,
            (tuning.alpha_ramp_duration * 1_000_000_000.0).round() as u64,
            (tuning.alpha_finish_duration * 1_000_000_000.0).round() as u64,
        ),
    );
}

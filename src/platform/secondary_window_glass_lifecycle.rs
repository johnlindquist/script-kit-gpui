#[cfg(target_os = "macos")]
static GLASS_EXIT_GENERATIONS: std::sync::Mutex<Vec<(usize, u64)>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct GlassExitLifecycle {
    window_key: usize,
    window_name: String,
    native_window_number: i64,
    generation: u64,
    mode: &'static str,
    original_frame: [f64; 4],
    /// Whether a native glass host (capsule container or footer panel) was
    /// attached when the exit was REQUESTED. The "host must not detach before
    /// the exit resolves" invariant is only meaningful when this is true —
    /// Notes mode legitimately owns no capsules at all.
    host_attached_at_request: bool,
    request_host_time_ns: u64,
    fade_duration_ns: u64,
    expected_removal_deadline_ns: u64,
    cancelled_at_host_time_ns: Option<u64>,
    committed_at_host_time_ns: Option<u64>,
    removed_at_host_time_ns: Option<u64>,
    host_teardown_at_host_time_ns: Option<u64>,
    events: Vec<(&'static str, u64)>,
}

#[cfg(target_os = "macos")]
static GLASS_EXIT_LIFECYCLES: std::sync::Mutex<Vec<GlassExitLifecycle>> =
    std::sync::Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GlassExitTicket {
    window_key: usize,
    generation: u64,
}

impl GlassExitTicket {
    pub(crate) fn generation(self) -> u64 {
        self.generation
    }
}

#[cfg(target_os = "macos")]
fn record_glass_exit_begin(window: id, ticket: GlassExitTicket, window_name: &str) {
    use cocoa::foundation::NSRect;
    let now = crate::platform::host_clock::host_time_ns();
    let frame: NSRect = unsafe { msg_send![window, frame] };
    let native_window_number: i64 = unsafe { msg_send![window, windowNumber] };
    let fade_duration_ns = (GLASS_EXIT_DURATION * 1_000_000_000.0).round() as u64;
    let deadline_ns = now.saturating_add(
        glass_exit_remove_delay()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64,
    );
    let mut records = GLASS_EXIT_LIFECYCLES
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    records.push(GlassExitLifecycle {
        window_key: ticket.window_key,
        window_name: window_name.to_string(),
        native_window_number,
        generation: ticket.generation,
        mode: match glass_exit_mode(window_name) {
            GlassExitMode::DetachedRegionsFadeOnly => "DetachedRegionsFadeOnly",
            GlassExitMode::PopupTransformAndBlur => "PopupTransformAndBlur",
        },
        original_frame: [
            frame.origin.x,
            frame.origin.y,
            frame.size.width,
            frame.size.height,
        ],
        host_attached_at_request: crate::platform::glass_button_host::host_attached(
            ticket.window_key,
        ) || unsafe {
            crate::footer_popup::native_footer_host_attached(window)
        },
        request_host_time_ns: now,
        fade_duration_ns,
        expected_removal_deadline_ns: deadline_ns,
        cancelled_at_host_time_ns: None,
        committed_at_host_time_ns: None,
        removed_at_host_time_ns: None,
        host_teardown_at_host_time_ns: None,
        events: vec![("ticketBegin", now)],
    });
    if records.len() > 64 {
        let drain = records.len() - 64;
        records.drain(0..drain);
    }
}

#[cfg(target_os = "macos")]
fn mutate_glass_exit_lifecycle(
    ticket: GlassExitTicket,
    event: &'static str,
    mutate: impl FnOnce(&mut GlassExitLifecycle, u64),
) {
    let now = crate::platform::host_clock::host_time_ns();
    let mut records = GLASS_EXIT_LIFECYCLES
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(record) = records.iter_mut().rev().find(|record| {
        record.window_key == ticket.window_key && record.generation == ticket.generation
    }) {
        mutate(record, now);
        record.events.push((event, now));
        if record.events.len() > 24 {
            record.events.remove(0);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn record_glass_exit_commit(ticket: GlassExitTicket) {
    mutate_glass_exit_lifecycle(ticket, "ticketCommit", |record, now| {
        record.committed_at_host_time_ns = Some(now);
        // This receipt mutation runs on the main thread immediately before
        // `remove_window`. Treat that commit point as the native removal and
        // host-teardown boundary too: after it returns, `window_key` may be a
        // dangling NSWindow pointer and must never be messaged again.
        record.removed_at_host_time_ns = Some(now);
        record.host_teardown_at_host_time_ns = Some(now);
    });
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn record_glass_exit_commit(_ticket: GlassExitTicket) {}

#[cfg(target_os = "macos")]
pub(crate) fn glass_exit_lifecycle_receipt(window_name: &str) -> serde_json::Value {
    use cocoa::foundation::NSRect;
    let record = GLASS_EXIT_LIFECYCLES
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .rev()
        .find(|record| record.window_name == window_name)
        .cloned();
    let Some(record) = record else {
        return serde_json::json!({
            "schemaVersion": 2,
            "windowName": window_name,
            "phase": "Open",
            "history": [],
        });
    };
    let (current_frame, current_alpha, filter_count, glass_host_attached) =
        if record.removed_at_host_time_ns.is_some() {
            // Do not dereference the retained raw pointer after the commit
            // boundary. The last live invariants are already represented by
            // `originalFrame`; removal guarantees alpha zero, no attached
            // host, and no surviving content-view filters.
            (record.original_frame, 0.0, 0, false)
        } else {
            let window = record.window_key as id;
            unsafe {
                let frame: NSRect = msg_send![window, frame];
                let alpha: f64 = msg_send![window, alphaValue];
                let content_view: id = msg_send![window, contentView];
                let filters: id = if content_view == nil {
                    nil
                } else {
                    let layer: id = msg_send![content_view, layer];
                    if layer == nil {
                        nil
                    } else {
                        msg_send![layer, filters]
                    }
                };
                let count: u64 = if filters == nil {
                    0
                } else {
                    msg_send![filters, count]
                };
                let attached = crate::platform::glass_button_host::host_attached(record.window_key)
                    || crate::footer_popup::native_footer_host_attached(window);
                (
                    [
                        frame.origin.x,
                        frame.origin.y,
                        frame.size.width,
                        frame.size.height,
                    ],
                    alpha,
                    count,
                    attached,
                )
            }
        };
    serde_json::json!({
        "schemaVersion": 2,
        "windowName": record.window_name,
        "nativeWindowNumber": record.native_window_number,
        "exitGeneration": record.generation,
        "exitMode": record.mode,
        "originalFrame": record.original_frame,
        "requestHostTimeNs": record.request_host_time_ns,
        "fadeDurationNs": record.fade_duration_ns,
        "expectedRemovalDeadlineNs": record.expected_removal_deadline_ns,
        "cancelledAtHostTimeNs": record.cancelled_at_host_time_ns,
        "committedAtHostTimeNs": record.committed_at_host_time_ns,
        "removedAtHostTimeNs": record.removed_at_host_time_ns,
        "hostTeardownAtHostTimeNs": record.host_teardown_at_host_time_ns,
        "currentFrame": current_frame,
        "currentAlpha": current_alpha,
        "commonContentViewFilterCount": filter_count,
        "glassHostAttached": glass_host_attached,
        "hostAttachedAtRequest": record.host_attached_at_request,
        "history": record.events.iter().map(|(event, host_time_ns)| {
            serde_json::json!({"event": event, "hostTimeNs": host_time_ns})
        }).collect::<Vec<_>>(),
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn glass_exit_lifecycle_receipt(window_name: &str) -> serde_json::Value {
    serde_json::json!({"schemaVersion": 2, "windowName": window_name, "history": []})
}

#[cfg(target_os = "macos")]
fn advance_glass_exit_generation(window_key: usize) -> GlassExitTicket {
    let mut generations = GLASS_EXIT_GENERATIONS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let generation =
        if let Some((_, generation)) = generations.iter_mut().find(|(key, _)| *key == window_key) {
            *generation = generation.wrapping_add(1).max(1);
            *generation
        } else {
            generations.push((window_key, 1));
            1
        };
    GlassExitTicket {
        window_key,
        generation,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn glass_exit_ticket_is_current(ticket: GlassExitTicket) -> bool {
    GLASS_EXIT_GENERATIONS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|(key, generation)| *key == ticket.window_key && *generation == ticket.generation)
}

#[cfg(target_os = "macos")]
unsafe fn ns_window_from_gpui_window(window: &gpui::Window) -> Option<id> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let ns_view = appkit.ns_view.as_ptr() as id;
    let ns_window: id = msg_send![ns_view, window];
    (ns_window != nil).then_some(ns_window)
}

#[cfg(target_os = "macos")]
pub(crate) fn cancel_gpui_window_exit_dematerialize(window: &gpui::Window) -> bool {
    if require_main_thread("cancel_gpui_window_exit_dematerialize") {
        return false;
    }
    unsafe {
        let Some(ns_window) = ns_window_from_gpui_window(window) else {
            return false;
        };
        cancel_ns_window_exit_dematerialize(ns_window)
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn cancel_gpui_window_exit_dematerialize(_window: &gpui::Window) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn glass_exit_ticket_is_current(_ticket: GlassExitTicket) -> bool {
    false
}

#[cfg(target_os = "macos")]
unsafe fn cancel_pending_glass_window_selectors(window: id) {
    if let Ok(mut targets) = GLASS_MORPH_SETTLE_TARGETS.lock() {
        targets.remove(&(window as usize));
    }
    if let Ok(mut targets) = GLASS_MORPH_TAIL_TARGETS.lock() {
        targets.remove(&(window as usize));
    }
    // Restore any onset-held GPUI content roots exactly once.
    if let Some(content) = GLASS_ENTRY_CONTENT_TARGETS
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(&(window as usize)))
    {
        for (view_ptr, original_alpha) in content {
            let view = view_ptr as id;
            if view != nil {
                let _: () = msg_send![view, setAlphaValue: original_alpha];
            }
        }
    }
    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return;
    }
    let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
    if glass_view == nil {
        return;
    }
    let _: () = msg_send![
        class!(NSObject),
        cancelPreviousPerformRequestsWithTarget: glass_view
        selector: sel!(settleOwnWindowFrame)
        object: nil
    ];
    for selector in [
        sel!(revealOwnWindowEntryContent),
        sel!(beginOwnWindowEntryTail),
    ] {
        let _: () = msg_send![
            class!(NSObject),
            cancelPreviousPerformRequestsWithTarget: glass_view
            selector: selector
            object: nil
        ];
    }
}

#[cfg(target_os = "macos")]
unsafe fn cancel_ns_window_exit_dematerialize(window: id) -> bool {
    cancel_ns_window_exit_dematerialize_impl(window, true)
}

/// Cancel a pending exit. `restore_alpha = true` is the public
/// cancellation/recovery contract (the window must end up presentable at
/// full alpha). Entry animation passes `restore_alpha = false`: restoring
/// 1.0 before the start frame is installed would flash a full-alpha extreme
/// calibration frame — the entry path owns alpha and applies
/// `tuning.start_alpha` BEFORE installing the start geometry.
#[cfg(target_os = "macos")]
unsafe fn cancel_ns_window_exit_dematerialize_impl(window: id, restore_alpha: bool) -> bool {
    if window == nil {
        return false;
    }
    let window_key = window as usize;
    let ticket = {
        let records = GLASS_EXIT_LIFECYCLES
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        records
            .iter()
            .rev()
            .find(|record| record.window_key == window_key)
            .map(|record| GlassExitTicket {
                window_key,
                generation: record.generation,
            })
    };
    if let Some(ticket) = ticket {
        mutate_glass_exit_lifecycle(ticket, "ticketCancel", |record, now| {
            record.cancelled_at_host_time_ns = Some(now);
        });
    }
    advance_glass_exit_generation(window_key);
    cancel_pending_glass_window_selectors(window);
    clear_exit_dematerialize_blur(window);
    let content_view: id = msg_send![window, contentView];
    if content_view != nil {
        let layer: id = msg_send![content_view, layer];
        if layer != nil {
            let _: () = msg_send![layer, removeAllAnimations];
        }
    }
    if restore_alpha {
        let _: () = msg_send![window, setAlphaValue: 1.0f64];
    }
    true
}

/// Per-window two-phase bounce settle targets. Multiple HUDs and secondary
/// windows can materialize concurrently, so a single global slot would let one
/// window steal another's rebound target.
/// Everything the delayed visible tail needs at T=88ms. Stored per-window
/// (same non-retaining pointer-key pattern as the settle targets; the
/// cancellation path clears these before window teardown).
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct GlassMorphTailTarget {
    squish_frame: [f64; 4],
    final_frame: [f64; 4],
    phase1: f64,
    phase2: f64,
    alpha_ramp_duration: f64,
    phase1_alpha_target: f64,
    alpha_finish_duration: f64,
}

#[cfg(target_os = "macos")]
static GLASS_MORPH_TAIL_TARGETS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, GlassMorphTailTarget>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// GPUI content roots hidden during the onset: (view ptr, original alpha).
#[cfg(target_os = "macos")]
type GlassEntryContentTargetMap = std::collections::HashMap<usize, Vec<(usize, f64)>>;

#[cfg(target_os = "macos")]
static GLASS_ENTRY_CONTENT_TARGETS: std::sync::LazyLock<
    std::sync::Mutex<GlassEntryContentTargetMap>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// T=53ms: fade the held GPUI content roots to their original alpha over
/// 35ms (ease-out). Transparent GPUI pixels reveal the already-present
/// glass, never bare desktop.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_reveal_entry_content(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) {
    // SAFETY: main thread (performSelector on the main run loop).
    unsafe {
        let this_id = this as *const objc::runtime::Object as id;
        let window: id = msg_send![this_id, window];
        if window == nil {
            return;
        }
        let targets = GLASS_ENTRY_CONTENT_TARGETS
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&(window as usize)));
        let Some(targets) = targets else { return };
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        // Must resolve the SAME way `glass_morph_tuning_from` does. This used
        // the bare constant while the tuning used the clamped value; they were
        // both 18ms so nothing diverged, but an override would have applied to
        // the receipt and not to the actual animation.
        let _: () = msg_send![ctx, setDuration: glass_entry_content_fade_duration()];
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
        for (view_ptr, original_alpha) in targets {
            let view = view_ptr as id;
            if view != nil {
                let animator: id = msg_send![view, animator];
                let _: () = msg_send![animator, setAlphaValue: original_alpha];
            }
        }
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
    }
}

/// T=88ms: run the (unchanged) visible tail — ease-out compression with the
/// 0.85→0.99 alpha ramp, then the scheduled rebound.
#[cfg(target_os = "macos")]
extern "C" fn tahoe_glass_backdrop_begin_entry_tail(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    // SAFETY: main thread (performSelector on the main run loop).
    unsafe {
        let this_id = this as *const objc::runtime::Object as id;
        let window: id = msg_send![this_id, window];
        if window == nil {
            return;
        }
        let target = GLASS_MORPH_TAIL_TARGETS
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(&(window as usize)));
        let Some(target) = target else { return };
        let [sx, sy, sw, sh] = target.squish_frame;
        let squish = NSRect::new(NSPoint::new(sx, sy), NSSize::new(sw, sh));

        // Frame compression: its own context (phase one, ease-out).
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![ctx, setDuration: target.phase1];
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
        let _: () = msg_send![animator, setFrame: squish display: true];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];

        // Alpha ramp 0.85 -> 0.99 (separate context, shorter, ease-out).
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let alpha_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![alpha_ctx, setDuration: target.alpha_ramp_duration];
        let _: () = msg_send![alpha_ctx, setAllowsImplicitAnimation: true];
        if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
            let name = tahoe_ns_string("easeOut");
            if name != nil {
                let timing: id = msg_send![timing_class, functionWithName: name];
                if timing != nil {
                    let _: () = msg_send![alpha_ctx, setTimingFunction: timing];
                }
            }
        }
        let alpha_animator: id = msg_send![window, animator];
        let _: () = msg_send![alpha_animator, setAlphaValue: target.phase1_alpha_target];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];

        // Schedule the (unchanged) rebound.
        if let Ok(mut guard) = GLASS_MORPH_SETTLE_TARGETS.lock() {
            guard.insert(
                window as usize,
                GlassMorphSettleTarget {
                    frame: target.final_frame,
                    frame_duration: target.phase2,
                    alpha_duration: target.alpha_finish_duration,
                },
            );
        }
        let _: () = msg_send![
            this_id,
            performSelector: sel!(settleOwnWindowFrame)
            withObject: nil
            afterDelay: (target.phase1 + GLASS_MORPH_SQUISH_HOLD)
        ];
    }
}

/// Rebound target for one in-flight entry morph. Named fields (not a
/// positional tuple) so the frame and alpha legs cannot be silently
/// transposed as the schedule grows.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct GlassMorphSettleTarget {
    frame: [f64; 4],
    /// Rebound frame animation duration (phase two, ease-in-out).
    frame_duration: f64,
    /// 0.99 → 1.0 finishing alpha duration (ease-out), starting at rebound.
    alpha_duration: f64,
}

#[cfg(target_os = "macos")]
static GLASS_MORPH_SETTLE_TARGETS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, GlassMorphSettleTarget>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Phase 2 of the appear bounce: ease the window from its compression
/// extreme back up to the final frame (ease-in-out — it starts at zero
/// velocity where the ease-out compression ended), while the finishing
/// alpha leg (0.99 → 1.0, ease-out) runs in its OWN animation context
/// because it has a different duration.
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
        let Some(target) = target else {
            return;
        };
        let [x, y, w, h] = target.frame;
        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));

        // Frame rebound: its own context (duration = phase two).
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![ctx, setDuration: target.frame_duration];
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
        let _: () = msg_send![animator, setFrame: frame display: true];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];

        // Finishing alpha leg: separate context (different duration).
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let alpha_ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![alpha_ctx, setDuration: target.alpha_duration];
        let _: () = msg_send![alpha_ctx, setAllowsImplicitAnimation: true];
        if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
            let name = tahoe_ns_string("easeOut");
            if name != nil {
                let timing: id = msg_send![timing_class, functionWithName: name];
                if timing != nil {
                    let _: () = msg_send![alpha_ctx, setTimingFunction: timing];
                }
            }
        }
        let alpha_animator: id = msg_send![window, animator];
        let _: () = msg_send![alpha_animator, setAlphaValue: 1.0f64];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
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

/// Ramp an entry defocus on the window's content view layer: the window
/// resolves FROM `GLASS_ENTRY_BLUR_RADIUS` INTO sharp focus over the entry.
///
/// The exact inverse of the exit blur, with one deliberate difference: the
/// layer's model value is left at radius 0 and the animation is
/// `removedOnCompletion`, so a finished entry leaves NO residual filter to
/// clean up. (The exit ramp must persist at 8pt, so it keeps
/// `fillMode=forwards` + `removedOnCompletion=false` and relies on
/// `clear_exit_dematerialize_blur`.)
///
/// Must be installed AFTER `cancel_ns_window_exit_dematerialize_impl`, which
/// clears ALL layer filters and animations.
///
/// Returns the radius actually installed (0.0 when the runtime lacks the
/// private `CAFilter` class, so the receipt can distinguish "no blur ran"
/// from "blur ran at 0").
#[cfg(target_os = "macos")]
unsafe fn ramp_entry_defocus(target_view: id, duration: f64, radius: f64, log_target: &str) -> f64 {
    // Every bail-out names itself: a silent 0.0 was indistinguishable from
    // "blur ran and did nothing".
    let bail = |reason: &str| {
        logging::log(
            log_target,
            &format!("event=glass_entry_defocus_skip reason={}", reason),
        );
        0.0
    };
    if radius <= 0.0 || duration <= 0.0 {
        return bail("zero_radius_or_duration");
    }
    // CHOOSE THE TARGET CAREFULLY. Runtime-proven 2026-07-27: on the MAIN
    // window, filtering the contentView layer blurs across the 8pt transparent
    // footer gutter and fills it, which the exit metrics catch as "transparent
    // footer gutter was not preserved". Main therefore passes its glass
    // backdrop, which is already laid out to EXCLUDE that gutter. Main-window
    // footer onset never passes its gap-spanning container here: footer_popup
    // returns only rounded NSGlassEffectView capsule layers. Detached reusable
    // windows may still pass their own bounded content view.
    if target_view == nil {
        return bail("no_target_view");
    }
    // `filters` is a CALayer property and the target is not guaranteed
    // layer-backed at entry time; without this the ramp silently no-ops.
    let wants_layer: bool = msg_send![target_view, wantsLayer];
    if !wants_layer {
        let _: () = msg_send![target_view, setWantsLayer: true];
    }
    let layer: id = msg_send![target_view, layer];
    if layer == nil {
        return bail("no_layer");
    }
    // A layer filter can sample beyond its bounds. Clip the defocus to the
    // owning surface so the main backdrop and detached footer cannot blur
    // across the calibrated transparent gutter between them.
    let _: () = msg_send![layer, setMasksToBounds: cocoa::base::YES];
    let (Some(filter_class), Some(anim_class)) = (
        objc::runtime::Class::get("CAFilter"),
        objc::runtime::Class::get("CABasicAnimation"),
    ) else {
        return bail("no_cafilter_class");
    };
    let blur_type = tahoe_ns_string("gaussianBlur");
    let filter: id = msg_send![filter_class, filterWithType: blur_type];
    if filter == nil {
        return bail("filter_alloc_failed");
    }
    let entry_name = tahoe_ns_string("entryBlur");
    let _: () = msg_send![filter, setName: entry_name];
    let radius_key = tahoe_ns_string("inputRadius");
    // Model value 0: once the animation is removed the window is sharp.
    let zero: id = msg_send![class!(NSNumber), numberWithDouble: 0.0f64];
    let _: () = msg_send![filter, setValue: zero forKey: radius_key];
    let filters: id = msg_send![class!(NSArray), arrayWithObject: filter];
    let _: () = msg_send![layer, setFilters: filters];

    let key_path = tahoe_ns_string("filters.entryBlur.inputRadius");
    let anim: id = msg_send![anim_class, animationWithKeyPath: key_path];
    if anim == nil {
        return bail("animation_alloc_failed");
    }
    let from: id = msg_send![class!(NSNumber), numberWithDouble: radius];
    let _: () = msg_send![anim, setFromValue: from];
    let _: () = msg_send![anim, setToValue: zero];
    let _: () = msg_send![anim, setDuration: duration];
    // Ease-out: most of the defocus resolves early, matching the material
    // onset curve rather than crawling to sharp at the very end.
    if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
        let name = tahoe_ns_string("easeOut");
        if name != nil {
            let timing: id = msg_send![timing_class, functionWithName: name];
            if timing != nil {
                let _: () = msg_send![anim, setTimingFunction: timing];
            }
        }
    }
    let _: () = msg_send![anim, setRemovedOnCompletion: true];
    let anim_key = tahoe_ns_string("entryBlurRamp");
    let _: () = msg_send![layer, addAnimation: anim forKey: anim_key];
    radius
}

/// Remove any exit-dematerialize blur left on the window's content view
/// layer (a superseded exit, or post-hide cleanup before the next show).
#[cfg(target_os = "macos")]
unsafe fn clear_exit_dematerialize_blur(window: id) {
    let content_view: id = msg_send![window, contentView];
    if content_view != nil {
        let layer: id = msg_send![content_view, layer];
        if layer != nil {
            let nil_id: id = nil;
            let _: () = msg_send![layer, setFilters: nil_id];
            let _: () = msg_send![layer, removeAllAnimations];
        }
    }
    crate::footer_popup::clear_main_window_footer_entry_capsule_effects(window);
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
    begin_gpui_window_exit_with_ticket(window, log_target, window_name).is_some()
}

#[cfg(target_os = "macos")]
pub(crate) fn begin_gpui_window_exit_with_ticket(
    window: &gpui::Window,
    log_target: &str,
    window_name: &str,
) -> Option<GlassExitTicket> {
    if require_main_thread("begin_gpui_window_exit_dematerialize") {
        return None;
    }
    // SAFETY: ns_view belongs to a live GPUI window on the main thread.
    unsafe {
        let ns_window = ns_window_from_gpui_window(window)?;
        if !begin_ns_window_exit_dematerialize(ns_window, log_target, window_name) {
            return None;
        }
        let ticket = advance_glass_exit_generation(ns_window as usize);
        record_glass_exit_begin(ns_window, ticket, window_name);
        Some(ticket)
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn begin_gpui_window_exit_with_ticket(
    _window: &gpui::Window,
    _log_target: &str,
    _window_name: &str,
) -> Option<GlassExitTicket> {
    None
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
    if let Some(ticket) = begin_gpui_window_exit_with_ticket(window, log_target, window_name) {
        let any_handle = window.window_handle();
        cx.spawn(async move |_this, cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(glass_exit_remove_delay())
                .await;
            cx.update(|cx| {
                if !glass_exit_ticket_is_current(ticket) {
                    return;
                }
                record_glass_exit_commit(ticket);
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
    if let Some(ticket) = begin_gpui_window_exit_with_ticket(window, log_target, window_name) {
        remove_gpui_window_after_glass_exit_from_app_with_ticket(window, cx, ticket);
    } else {
        window.remove_window();
    }
}

/// Schedule only the destruction tail after a caller has already started the
/// exit. This is used by `on_window_should_close` handlers that must return
/// `false` while the visual tail completes.
pub fn remove_gpui_window_after_glass_exit_from_app(window: &mut gpui::Window, cx: &mut gpui::App) {
    let ticket = unsafe {
        ns_window_from_gpui_window(window)
            .map(|window| advance_glass_exit_generation(window as usize))
    };
    if let Some(ticket) = ticket {
        remove_gpui_window_after_glass_exit_from_app_with_ticket(window, cx, ticket);
    } else {
        window.remove_window();
    }
}

#[cfg(target_os = "macos")]
fn remove_gpui_window_after_glass_exit_from_app_with_ticket(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    ticket: GlassExitTicket,
) {
    let any_handle = window.window_handle();
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor()
            .timer(glass_exit_remove_delay())
            .await;
        cx.update(|cx| {
            if !glass_exit_ticket_is_current(ticket) {
                return;
            }
            record_glass_exit_commit(ticket);
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

    if !(tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled())
    {
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
        // An exit requested while the entry rebound is still pending must
        // freeze the owning window as one partitioned surface. Otherwise the
        // old settle callback can resize the stage while its capsules fade.
        cancel_pending_glass_window_selectors(window);

        if matches!(
            glass_exit_mode(window_name),
            GlassExitMode::DetachedRegionsFadeOnly
        ) {
            clear_exit_dematerialize_blur(window);
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
            let _: () = msg_send![animator, setAlphaValue: 0.0f64];
            let _: () = msg_send![class!(NSAnimationContext), endGrouping];
            logging::log(
                log_target,
                &format!(
                    "event=glass_morph window={} variant=detached_regions_fade_only phase=exit duration={:.2}s direction=fixed_frame",
                    window_name, GLASS_EXIT_DURATION
                ),
            );
            return true;
        }

        // Popup-only blur ramp: 0 -> 8pt over the fade.
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
    if !(tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled())
    {
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
    // Zero-alpha parking is legal ONLY for a window that is not presented.
    // A visible window at alpha zero is wallpaper where UI should be — the
    // exact defect the visible-entry alpha policy forbids.
    let is_visible: bool = msg_send![window, isVisible];
    if is_visible {
        tracing::error!(
            target: "script_kit::native_glass",
            event = "glass_hidden_park_on_visible_window",
            "glass_hidden_park_on_visible_window"
        );
        return;
    }
    let _: () = msg_send![window, setAlphaValue: GLASS_HIDDEN_PARK_ALPHA];
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

/// How long the floating footer stays ordered out before it fades back in.
///
/// The footer is a SEPARATE NSWindow tracking the main window's bounds, so it
/// does not inherit the main entry's fade, blur, or geometry. It is parked
/// (ordered fully out) and re-presented on its own schedule.
///
/// LEGACY (`SCRIPT_KIT_GLASS_FOOTER_SYNC=0`): park for the whole remaining
/// morph plus a 60ms tail, then fade over 120ms — the footer is absent for the
/// entire 149ms entry and finishes arriving ~180ms AFTER the main window has
/// settled. That reads as a second, unrelated arrival (user report
/// 2026-07-27: "why aren't the floating buttons fading/blurring too?").
///
/// The park exists because the footer would otherwise chase the animating
/// frame — which mattered at the old `0.03` inset (106% -> 98%, tens of points
/// of travel). The current calibration moves 4.5pt per side, so the chase it
/// prevents is now far less visible than the late arrival it causes.
///
/// SYNCED (default): hold only to the main window's CONTENT-REVEAL anchor, so
/// the footer fades up on the same clock as the main content and the whole
/// surface materializes as one object.
#[cfg(target_os = "macos")]
fn glass_sibling_reveal_delay() -> Option<std::time::Duration> {
    use std::sync::atomic::Ordering;
    let synced = std::env::var("SCRIPT_KIT_GLASS_FOOTER_SYNC")
        .ok()
        .map(|raw| raw.trim() != "0")
        .unwrap_or(true);
    if !synced {
        return glass_morph_remaining();
    }
    // Still in flight?
    glass_morph_remaining()?;
    let start = GLASS_MORPH_LAST_START_MS.load(Ordering::Relaxed);
    if start == u64::MAX {
        return None;
    }
    let anchor_ms = (GLASS_ENTRY_CONTENT_HOLD_DURATION * 1000.0).round() as u64;
    let reveal_at = start.saturating_add(anchor_ms);
    let now = glass_morph_now_ms();
    Some(std::time::Duration::from_millis(
        reveal_at.saturating_sub(now),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn glass_morph_remaining() -> Option<std::time::Duration> {
    None
}

/// Park a sibling GPUI window (footer overlay) while a glass morph is in
/// flight, returning how long until it should fade back in. The overlay is a
/// separate NSWindow that tracks the main window's frame; without this it
/// appears instantly at full alpha and visibly chases the animating frame.
///
/// Parking means NOT PRESENTED: the window is ordered out for the interval,
/// and only then parked at the hidden alpha. Leaving a visible window on
/// screen at alpha zero is prohibited (wallpaper where UI should be), so the
/// old visible zero-alpha park was replaced by this orderOut-based park.
/// [`restore_gpui_window_alpha_animated`] re-presents it and keeps the
/// locked 0.12s fade.
#[cfg(target_os = "macos")]
pub fn park_gpui_window_alpha_if_morphing(window: &gpui::Window) -> Option<std::time::Duration> {
    let remaining = glass_sibling_reveal_delay()?;
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
        let _: () = msg_send![ns_window, orderOut: nil];
        let is_visible: bool = msg_send![ns_window, isVisible];
        if is_visible {
            // Could not take the window off screen (e.g. still attached as a
            // child). Zero-alpha visible parking is prohibited — leave alpha
            // alone and let the entry fade own it.
            tracing::error!(
                target: "script_kit::native_glass",
                event = "glass_hidden_park_on_visible_window",
                "glass_hidden_park_on_visible_window"
            );
            return Some(remaining);
        }
        let _: () = msg_send![ns_window, setAlphaValue: GLASS_HIDDEN_PARK_ALPHA];
    }
    Some(remaining)
}

#[cfg(not(target_os = "macos"))]
pub fn park_gpui_window_alpha_if_morphing(_window: &gpui::Window) -> Option<std::time::Duration> {
    None
}

/// Re-present a previously parked sibling window and fade it back in.
///
/// The fade duration MATCHES the main window's content fade, so the floating
/// footer materializes on the same clock as the content it belongs to. It was
/// previously a hardcoded `0.12s` chosen independently of the entry, which —
/// combined with the full-morph park delay — made the footer a visibly
/// separate second arrival.
///
/// The park orders the window out, so restore orders it back in (without
/// activating) before the fade.
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
        let _: () = msg_send![ns_window, orderFrontRegardless];
        // Same defocus the main window resolves out of, so the buttons
        // materialize rather than simply appearing.
        // The footer window IS the buttons, so its own content view is the
        // right target — the main window's gutter is a different window and is
        // unaffected.
        let footer_content: id = msg_send![ns_window, contentView];
        let footer_blur = ramp_entry_defocus(
            footer_content,
            glass_entry_content_fade_duration(),
            glass_entry_blur_radius(),
            "FOOTER",
        );
        let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
        let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
        let _: () = msg_send![ctx, setDuration: glass_entry_content_fade_duration()];
        if let Some(timing_class) = objc::runtime::Class::get("CAMediaTimingFunction") {
            let name = tahoe_ns_string("easeOut");
            if name != nil {
                let timing: id = msg_send![timing_class, functionWithName: name];
                if timing != nil {
                    let _: () = msg_send![ctx, setTimingFunction: timing];
                }
            }
        }
        let animator: id = msg_send![ns_window, animator];
        let _: () = msg_send![animator, setAlphaValue: 1.0f64];
        let _: () = msg_send![class!(NSAnimationContext), endGrouping];
        logging::log(
            "FOOTER",
            &format!(
                "event=glass_sibling_reveal fade_ns={} blur_radius={:.2}",
                (glass_entry_content_fade_duration() * 1_000_000_000.0).round() as u64,
                footer_blur
            ),
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn restore_gpui_window_alpha_animated(_window: &gpui::Window) {}

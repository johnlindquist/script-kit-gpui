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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeGlassEntryReceipt {
    pub(crate) window_number: i64,
    pub(crate) configured: bool,
    pub(crate) backdrop_found_or_created: bool,
    pub(crate) native_selectors_supported: bool,
    pub(crate) style_applied: bool,
    pub(crate) style_signature: NativeGlassStyleSignature,
    pub(crate) morph_started: bool,
    pub(crate) settle_duration_ms: u64,
    /// Time from native morph start until phase one first crosses the final
    /// window size. Content can begin revealing here and finish during rebound.
    pub(crate) settled_crossing_delay_ms: u64,
    pub(crate) configured_at_monotonic_ns: u64,
    pub(crate) configured_at_unix_ms: u64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct TahoeGlassBackdropResult {
    created: bool,
    found_or_created: bool,
    native_selectors_supported: bool,
    style_applied: bool,
    style_signature: NativeGlassStyleSignature,
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
    start_alpha: f64,
    start_scale_x: f64,
    start_scale_y: f64,
    squish_scale_x: f64,
    squish_scale_y: f64,
    phase1: f64,
    phase2: f64,
}

#[cfg(target_os = "macos")]
fn cubic_bezier_axis(t: f64, first: f64, second: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * first + 3.0 * inverse * t * t * second + t * t * t
}

/// Convert a phase-one geometry progress to elapsed time for AppKit's
/// `easeInEaseOut` timing function (`cubic-bezier(0.42, 0, 0.58, 1)`).
#[cfg(target_os = "macos")]
fn ease_in_out_time_fraction_for_progress(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..32 {
        let parameter = (low + high) * 0.5;
        let value = cubic_bezier_axis(parameter, 0.0, 1.0);
        if value < progress {
            low = parameter;
        } else {
            high = parameter;
        }
    }
    cubic_bezier_axis((low + high) * 0.5, 0.42, 0.58)
}

#[cfg(target_os = "macos")]
fn settled_size_crossing_delay_ms(tuning: GlassMorphTuning) -> u64 {
    let phase_distance = tuning.start_scale_x - tuning.squish_scale_x;
    if phase_distance <= f64::EPSILON {
        return 0;
    }
    let geometry_progress = ((tuning.start_scale_x - 1.0) / phase_distance).clamp(0.0, 1.0);
    let time_fraction = ease_in_out_time_fraction_for_progress(geometry_progress);
    (tuning.phase1 * time_fraction * 1000.0).round() as u64
}

#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_DURATION: f64 = 0.02;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_INSET: f64 = 0.005;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_DURATION: f64 = 2.0;
/// Hide the deliberately exaggerated calibration frame until it starts moving.
///
/// The main window and child popups intentionally begin on opposite sides of
/// their final geometry. At full alpha those calibration frames read as a huge
/// launcher and a tiny Actions menu. Fade the owning window in with phase one
/// so the measured geometry remains unchanged without exposing either extreme.
#[cfg(target_os = "macos")]
const GLASS_MORPH_ENTRY_START_ALPHA: f64 = 0.0;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlassExitMode {
    DetachedRegionsFadeOnly,
    PopupTransformAndBlur,
}

#[cfg(target_os = "macos")]
fn glass_exit_mode(window_name: &str) -> GlassExitMode {
    if window_name_owns_detached_footer(window_name) {
        GlassExitMode::DetachedRegionsFadeOnly
    } else {
        GlassExitMode::PopupTransformAndBlur
    }
}

pub fn glass_exit_remove_delay() -> std::time::Duration {
    std::time::Duration::from_millis(GLASS_EXIT_REMOVE_DELAY_MS)
}

pub(crate) fn glass_entry_settle_delay() -> std::time::Duration {
    let duration = crate::theme::get_cached_theme()
        .get_opacity()
        .glass_morph_duration
        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
        .clamp(0.0, GLASS_MORPH_MAX_DURATION as f32);
    std::time::Duration::from_secs_f32(duration)
}

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
    let generation = if let Some((_, generation)) = generations
        .iter_mut()
        .find(|(key, _)| *key == window_key)
    {
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
        .any(|(key, generation)| {
            *key == ticket.window_key && *generation == ticket.generation
        })
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
    let _: () = msg_send![
        class!(NSObject),
        cancelPreviousPerformRequestsWithTarget: glass_view
        selector: sel!(orderOutOwnWindow)
        object: nil
    ];
}

#[cfg(target_os = "macos")]
unsafe fn cancel_ns_window_exit_dematerialize(window: id) -> bool {
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
    let _: () = msg_send![window, setAlphaValue: 1.0f64];
    true
}

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
        start_alpha: GLASS_MORPH_ENTRY_START_ALPHA,
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
) -> NativeGlassEntryReceipt {
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
    let glass_mode = tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled();
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

    let backdrop = configure_tahoe_window_backdrop_with_result(window, log_target, window_name);
    let glass_created = backdrop.is_some_and(|result| result.created);
    let morph_tuning = glass_morph_tuning();
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
    let window_number: i64 = msg_send![window, windowNumber];
    let style_signature = backdrop
        .map(|result| result.style_signature)
        .unwrap_or_else(|| {
            resolve_native_glass_style(
                &crate::theme::get_cached_theme(),
                NativeGlassSurfaceRole::WindowBackdrop,
            )
            .signature
        });
    let configured_at_monotonic_ns = crate::platform::host_clock::host_time_ns();
    let configured_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let backdrop_found_or_created = backdrop.is_some_and(|result| result.found_or_created);
    let native_selectors_supported =
        backdrop.is_some_and(|result| result.native_selectors_supported);
    let style_applied = backdrop.is_some_and(|result| result.style_applied);
    NativeGlassEntryReceipt {
        window_number,
        configured: window_number > 0
            && content_view != nil
            && backdrop_found_or_created
            && native_selectors_supported
            && style_applied,
        backdrop_found_or_created,
        native_selectors_supported,
        style_applied,
        style_signature,
        morph_started: glass_created && morph_tuning.is_some(),
        settle_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        settled_crossing_delay_ms: if glass_created {
            morph_tuning
                .map(settled_size_crossing_delay_ms)
                .unwrap_or(0)
        } else {
            0
        },
        configured_at_monotonic_ns,
        configured_at_unix_ms,
    }
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

/// True only when the complete one-window Liquid Glass composition can be
/// installed. The debug no-glass switch is intentionally part of this gate so
/// fallback launches do not keep the detached footer inset or transparent
/// window background after the native backdrop has been disabled.
#[cfg(target_os = "macos")]
pub fn tahoe_native_glass_composition_available() -> bool {
    native_glass_composition_gate(
        tahoe_liquid_glass_available(),
        crate::platform::glass_button_host::native_glass_container_available(),
        std::env::var("SCRIPT_KIT_DEBUG_NO_GLASS").is_ok(),
    )
}

fn native_glass_composition_gate(
    effect_view_available: bool,
    container_view_available: bool,
    debug_no_glass: bool,
) -> bool {
    effect_view_available && container_view_available && !debug_no_glass
}

#[cfg(not(target_os = "macos"))]
pub fn tahoe_native_glass_composition_available() -> bool {
    false
}

/// Background appearance for a vibrancy-enabled window: `Transparent` when
/// the Tahoe glass backdrop supplies the material (a `Blurred` appearance
/// would stack the gpui fork's NSVisualEffectView above the glass and hide
/// it), `Blurred` otherwise.
pub fn vibrancy_window_background() -> gpui::WindowBackgroundAppearance {
    if tahoe_native_glass_composition_available() {
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
                    NSSize::new(bounds.size.width, (bounds.size.height - bottom_inset).max(0.0)),
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

#[cfg(target_os = "macos")]
fn window_name_owns_detached_footer(window_name: &str) -> bool {
    matches!(window_name, "Main window" | "Notes" | "Dictation overlay")
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
        let bottom_inset = *this.get_ivar::<f64>("_scriptKitBottomInset");
        let frame = TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset }
            .frame(bounds);
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
    (*glass_view).set_ivar(
        "_scriptKitBottomInset",
        backdrop_layout.bottom_inset(),
    );
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

    // Every show is an exit supersession boundary. Invalidate delayed removal,
    // cancel old settle/order-out callbacks, and clear common-ancestor effects
    // before preparing the next entry frame.
    cancel_ns_window_exit_dematerialize(window);

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
    let _: () = msg_send![window, setAlphaValue: tuning.start_alpha];

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
            "event=glass_morph window={} variant={} phase=enter duration={:.2}s inset={:.3} start_alpha={:.2} frames={}x{}->{}x{}->{}x{}",
            window_name,
            GlassMorphVariant::WindowFrame.log_name(),
            tuning.duration,
            tuning.inset_fraction,
            tuning.start_alpha,
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

    if !(tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled())
    {
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

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeGlassSurfaceRole {
    WindowBackdrop,
    FloatingCapsule,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeGlassStyleSignature {
    pub(crate) dark: bool,
    pub(crate) tint_rgb: u32,
    pub(crate) requested_tint_alpha_bits: Option<u32>,
    pub(crate) effective_tint_alpha_bits: u32,
    pub(crate) veil_alpha_bits: u32,
    pub(crate) rim_rgba: u32,
    pub(crate) rim_width_bits: u32,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeGlassStyle {
    pub(crate) role: NativeGlassSurfaceRole,
    pub(crate) signature: NativeGlassStyleSignature,
    pub(crate) effective_tint_alpha: f32,
    pub(crate) veil_alpha: f32,
    pub(crate) rim_width: f32,
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_native_glass_style(
    theme: &crate::theme::Theme,
    role: NativeGlassSurfaceRole,
) -> NativeGlassStyle {
    let requested_tint_alpha = theme.get_opacity().glass_tint_opacity;
    let matched = crate::ui_foundation::main_window_matched_background_rgba(theme);
    let tint_rgb = (matched >> 8) & 0x00ff_ffff;
    let tint_floor = crate::ui::chrome::LIQUID_GLASS_STABILITY_TINT_ALPHA_FLOOR;
    let effective_tint_alpha = requested_tint_alpha
        .unwrap_or(tint_floor)
        .max(tint_floor)
        .clamp(0.0, 1.0);
    let capsule = matches!(role, NativeGlassSurfaceRole::FloatingCapsule);
    let veil_alpha = if capsule {
        crate::ui::chrome::LIQUID_GLASS_CAPSULE_VEIL_ALPHA
    } else {
        0.0
    };
    let rim_alpha = if capsule {
        if theme.should_use_dark_vibrancy() {
            crate::ui::chrome::LIQUID_GLASS_CAPSULE_RIM_ALPHA_DARK
        } else {
            crate::ui::chrome::LIQUID_GLASS_CAPSULE_RIM_ALPHA_LIGHT
        }
    } else {
        0.0
    };
    let rim_width = if rim_alpha > 0.0 {
        crate::ui::chrome::LIQUID_GLASS_CAPSULE_RIM_WIDTH_PX
    } else {
        0.0
    };
    let rim_color = if theme.should_use_dark_vibrancy() {
        0xff_ff_ff
    } else {
        0x00_00_00
    };
    let rim_rgba = (rim_color << 8) | (rim_alpha * 255.0).round() as u32;
    NativeGlassStyle {
        role,
        signature: NativeGlassStyleSignature {
            dark: theme.should_use_dark_vibrancy(),
            tint_rgb,
            requested_tint_alpha_bits: requested_tint_alpha.map(f32::to_bits),
            effective_tint_alpha_bits: effective_tint_alpha.to_bits(),
            veil_alpha_bits: veil_alpha.to_bits(),
            rim_rgba,
            rim_width_bits: rim_width.to_bits(),
        },
        effective_tint_alpha,
        veil_alpha,
        rim_width,
    }
}

/// Apply the complete shared native glass policy. AppKit mutations are made
/// in one disabled-actions transaction so a theme refresh cannot expose an
/// intermediate untinted or mismatched frame.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn apply_native_glass_style(
    glass_view: id,
    style: NativeGlassStyle,
) -> bool {
    if glass_view == nil {
        return false;
    }
    let responds: bool = msg_send![glass_view, respondsToSelector: sel!(setTintColor:)];
    if !responds {
        return false;
    }
    let transaction_class = objc::runtime::Class::get("CATransaction");
    if let Some(transaction_class) = transaction_class {
        let _: () = msg_send![transaction_class, begin];
        let _: () = msg_send![transaction_class, setDisableActions: cocoa::base::YES];
    }
    let appearance_name = if style.signature.dark {
        tahoe_ns_string("NSAppearanceNameVibrantDark")
    } else {
        tahoe_ns_string("NSAppearanceNameVibrantLight")
    };
    if appearance_name != nil {
        let appearance: id = msg_send![class!(NSAppearance), appearanceNamed: appearance_name];
        if appearance != nil {
            let _: () = msg_send![glass_view, setAppearance: appearance];
        }
    }
    let red = f64::from((style.signature.tint_rgb >> 16) & 0xff) / 255.0;
    let green = f64::from((style.signature.tint_rgb >> 8) & 0xff) / 255.0;
    let blue = f64::from(style.signature.tint_rgb & 0xff) / 255.0;
    let tint: id = msg_send![
        class!(NSColor),
        colorWithCalibratedRed: red
        green: green
        blue: blue
        alpha: f64::from(style.effective_tint_alpha)
    ];
    let _: () = msg_send![glass_view, setTintColor: tint];

    let _: () = msg_send![glass_view, setWantsLayer: cocoa::base::YES];
    let content_view: id = msg_send![glass_view, contentView];
    let mut content_layer = nil;
    if content_view != nil {
        let _: () = msg_send![content_view, setWantsLayer: cocoa::base::YES];
        content_layer = msg_send![content_view, layer];
        if content_layer != nil {
            let veil: id = msg_send![
                class!(NSColor),
                colorWithCalibratedRed: red
                green: green
                blue: blue
                alpha: f64::from(style.veil_alpha)
            ];
            let veil_cg: *const std::ffi::c_void = msg_send![veil, CGColor];
            let _: () = msg_send![content_layer, setBackgroundColor: veil_cg];
            let _: () = msg_send![content_layer, setMasksToBounds: cocoa::base::YES];
            if matches!(style.role, NativeGlassSurfaceRole::FloatingCapsule) {
                let _: () = msg_send![
                    content_layer,
                    setCornerRadius:
                        f64::from(crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX)
                ];
            }
        }
    }
    let layer: id = msg_send![glass_view, layer];
    if layer != nil {
        if matches!(style.role, NativeGlassSurfaceRole::FloatingCapsule) {
            let _: () = msg_send![
                layer,
                setCornerRadius:
                    f64::from(crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX)
            ];
        }
        let rim_red = f64::from((style.signature.rim_rgba >> 24) & 0xff) / 255.0;
        let rim_green = f64::from((style.signature.rim_rgba >> 16) & 0xff) / 255.0;
        let rim_blue = f64::from((style.signature.rim_rgba >> 8) & 0xff) / 255.0;
        let rim_alpha = f64::from(style.signature.rim_rgba & 0xff) / 255.0;
        let rim: id = msg_send![
            class!(NSColor),
            colorWithCalibratedRed: rim_red
            green: rim_green
            blue: rim_blue
            alpha: rim_alpha
        ];
        let rim_cg: *const std::ffi::c_void = msg_send![rim, CGColor];
        // The foreground content layer is the final visible capsule surface.
        // Put the separation rim there rather than behind NSGlassEffectView's
        // private material hierarchy.
        if content_layer != nil {
            let _: () = msg_send![content_layer, setBorderColor: rim_cg];
            let _: () =
                msg_send![content_layer, setBorderWidth: f64::from(style.rim_width)];
        }
        let _: () = msg_send![layer, setBorderWidth: 0.0f64];
        // R is the locked production treatment. Clear any stale shadow state
        // left by a recycled AppKit view so RS cannot leak into production.
        let _: () = msg_send![layer, setShadowOpacity: 0.0f32];
        let _: () = msg_send![layer, setShadowRadius: 0.0f64];
        let shadow_offset = cocoa::foundation::NSSize::new(0.0, 0.0);
        let _: () = msg_send![layer, setShadowOffset: shadow_offset];
        let _: () = msg_send![layer, setShadowPath: nil];
    }
    if let Some(transaction_class) = transaction_class {
        let _: () = msg_send![transaction_class, commit];
    }
    let window: id = msg_send![glass_view, window];
    let window_number: i64 = if window != nil {
        msg_send![window, windowNumber]
    } else {
        -1
    };
    tracing::info!(
        target: "script_kit::native_glass",
        event = "native_glass_style_applied",
        window_number,
        role = match style.role {
            NativeGlassSurfaceRole::WindowBackdrop => "window_backdrop",
            NativeGlassSurfaceRole::FloatingCapsule => "floating_capsule",
        },
        tint_rgb = style.signature.tint_rgb,
        requested_tint_alpha_bits = ?style.signature.requested_tint_alpha_bits,
        effective_tint_alpha = style.effective_tint_alpha,
        veil_alpha = style.veil_alpha,
        rim_rgba = style.signature.rim_rgba,
        "native_glass_style_applied"
    );
    true
}

/// Compatibility entry point for callers that do not yet own a role.
///
/// # Safety
/// `glass_view` must be a valid NSGlassEffectView (or nil-checked upstream)
/// on the main thread.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn apply_theme_glass_tint(glass_view: id) -> bool {
    let theme = crate::theme::get_cached_theme();
    apply_native_glass_style(
        glass_view,
        resolve_native_glass_style(&theme, NativeGlassSurfaceRole::WindowBackdrop),
    )
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
    configure_tahoe_window_backdrop_with_result(window, log_target, window_name)
        .is_some_and(|result| result.created)
}

#[cfg(target_os = "macos")]
unsafe fn configure_tahoe_window_backdrop_with_result(
    window: id,
    log_target: &str,
    window_name: &str,
) -> Option<TahoeGlassBackdropResult> {
    use cocoa::appkit::{NSViewHeightSizable, NSViewWidthSizable};
    use cocoa::foundation::NSRect;

    if window.is_null() {
        return None;
    }
    if require_main_thread("configure_tahoe_window_backdrop") {
        return None;
    }

    // Reconcile capability loss as well as installation. In particular, the
    // debug no-glass launch must remove any stale inset backdrop and restore
    // the ordinary full-window shadow/fallback material.
    if !tahoe_native_glass_composition_available()
        || !crate::theme::get_cached_theme().is_vibrancy_enabled()
    {
        remove_tahoe_window_backdrop(window, window_name);
        logging::log(
            log_target,
            &format!(
                "{}: Tahoe glass composition unavailable or disabled; restored fallback backdrop",
                window_name
            ),
        );
        return None;
    }

    let Some(glass_class) = tahoe_liquid_glass_class() else {
        logging::log(
            log_target,
            &format!(
                "{}: Tahoe NSGlassEffectView unavailable; native glass backdrop skipped",
                window_name
            ),
        );
        return None;
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
        return None;
    }

    let content_bounds: NSRect = msg_send![content_view, bounds];
    let backdrop_layout = tahoe_backdrop_layout(window_name);
    let backdrop_frame = backdrop_layout.frame(content_bounds);
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
            return None;
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
            return None;
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
            return None;
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
        // The superview now owns the view. Balance `alloc` so teardown via
        // removeFromSuperview can release it instead of leaking a native
        // object for every main-window lifetime.
        let _: () = msg_send![glass_view, release];
        created = true;
    }

    let _: () =
        msg_send![glass_view, setAutoresizingMask: NSViewWidthSizable | NSViewHeightSizable];

    let style = resolve_native_glass_style(
        &crate::theme::get_cached_theme(),
        NativeGlassSurfaceRole::WindowBackdrop,
    );
    let tint_selector_supported: bool =
        msg_send![glass_view, respondsToSelector: sel!(setTintColor:)];
    let corner_selector_supported: bool =
        msg_send![glass_view, respondsToSelector: sel!(setCornerRadius:)];
    let native_selectors_supported = tint_selector_supported && corner_selector_supported;
    let tint_applied = apply_native_glass_style(glass_view, style);

    let corner_radius = {
        let radius = tahoe_content_corner_radius(content_view);
        // Detached-footer windows expose the backdrop's corners mid-window,
        // where the ordinary NSWindow mask cannot round them. Match the GPUI
        // content-stage radius for each owning surface.
        if backdrop_layout.is_detached_footer() && radius <= 0.0 {
            if window_name == "Notes" {
                f64::from(crate::ui::chrome::LIQUID_GLASS_WINDOW_RADIUS_PX)
            } else if window_name == "Dictation overlay" {
                f64::from(crate::ui::chrome::LIQUID_GLASS_PANEL_RADIUS_PX)
            } else {
                f64::from(crate::ui::chrome::MAIN_WINDOW_CONTENT_RADIUS_PX)
            }
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

    // The physical main NSWindow is only a transparent composition host. A
    // native full-window shadow would reveal that host as one rectangular
    // slab and visually bridge the main material to the footer capsules.
    // Cast any depth from the bounded backdrop layer instead.
    update_tahoe_backdrop_geometry_and_shadow(
        window,
        content_view,
        glass_view,
        backdrop_layout,
        corner_radius,
    );

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
            backdrop_frame.origin.x,
            backdrop_frame.origin.y,
            backdrop_frame.size.width,
            backdrop_frame.size.height,
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

    Some(TahoeGlassBackdropResult {
        created,
        found_or_created: glass_count == 1 && backmost,
        native_selectors_supported,
        style_applied: tint_applied,
        style_signature: style.signature,
    })
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
    let _ = configure_secondary_window_vibrancy_with_receipt(window, window_name, is_dark);
}

#[cfg(target_os = "macos")]
pub(crate) unsafe fn configure_secondary_window_vibrancy_with_receipt(
    window: id,
    window_name: &str,
    is_dark: bool,
) -> Option<NativeGlassEntryReceipt> {
    if window.is_null() {
        logging::log(
            "PANEL",
            &format!(
                "WARNING: Cannot configure null window for {} vibrancy",
                window_name
            ),
        );
        return None;
    }

    Some(configure_window_vibrancy_common(
        window,
        "PANEL",
        window_name,
        is_dark,
        GlassMorphVariant::WindowFrame,
    ))
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
    #[test]
    fn detached_glass_requires_both_classes_and_honors_debug_fallback() {
        assert!(super::native_glass_composition_gate(true, true, false));
        assert!(!super::native_glass_composition_gate(false, true, false));
        assert!(!super::native_glass_composition_gate(true, false, false));
        assert!(!super::native_glass_composition_gate(true, true, true));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn glass_morph_tuning_drives_matching_frame_and_layer_geometry() {
        let tuning = super::glass_morph_tuning_from(0.28, 0.03).expect("morph enabled");
        let epsilon = 1e-12;
        assert!((tuning.start_scale_x - 1.06).abs() < epsilon);
        assert!((tuning.start_scale_y - 1.024).abs() < epsilon);
        assert!((tuning.start_alpha - 0.0).abs() < epsilon);
        assert!((tuning.squish_scale_x - 0.97).abs() < epsilon);
        assert!((tuning.squish_scale_y - 0.988).abs() < epsilon);
        assert!((tuning.phase1 - 0.14).abs() < epsilon);
        assert!((tuning.phase2 - 0.14).abs() < epsilon);
        assert_eq!(super::settled_size_crossing_delay_ms(tuning), 84);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn glass_motion_fixture_matches_the_measured_production_calibration() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../scripts/agentic/fixtures/glass-motion-calibration-theme.json"
        ))
        .expect("glass motion calibration fixture should be valid JSON");
        let duration = fixture["opacity"]["glass_morph_duration"]
            .as_f64()
            .expect("fixture duration");
        let inset = fixture["opacity"]["glass_morph_inset"]
            .as_f64()
            .expect("fixture inset");

        let epsilon = 1e-6;
        assert!(
            (duration - f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)).abs()
                < epsilon
        );
        assert!(
            (inset - f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET)).abs() < epsilon
        );
        assert_eq!(super::GLASS_MORPH_ENTRY_START_ALPHA, 0.0);
        assert_eq!(super::GLASS_MORPH_VERTICAL_DAMPING, 0.4);
        assert_eq!(super::GLASS_MORPH_SQUISH_FACTOR, 0.5);
        assert_eq!(super::GLASS_MORPH_PHASE1_FRACTION, 0.5);
        let tuning =
            super::glass_morph_tuning_from(duration, inset).expect("fixture enables glass morph");
        assert_eq!(super::settled_size_crossing_delay_ms(tuning), 84);
        assert_eq!(super::GLASS_EXIT_DURATION, 0.12);
        assert_eq!(super::GLASS_EXIT_REMOVE_DELAY_MS, 135);
        assert_eq!(super::GLASS_EXIT_GROW_X, 0.03);
        assert_eq!(super::GLASS_EXIT_SHRINK_X, 0.05);
        assert_eq!(super::GLASS_EXIT_SHRINK_Y, 0.035);
        assert_eq!(super::GLASS_EXIT_GROW_Y, 0.012);
        assert_eq!(super::GLASS_EXIT_BLUR_RADIUS, 8.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn glass_morph_tuning_respects_slider_disable_thresholds() {
        assert!(super::glass_morph_tuning_from(0.0, 0.03).is_none());
        assert!(super::glass_morph_tuning_from(0.28, 0.0).is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn detached_main_backdrop_excludes_footer_and_exact_eight_point_gutter() {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(750.0, 501.0));
        let layout = super::TahoeBackdropLayout::ContentAboveDetachedFooter {
            bottom_inset: 40.0,
        };
        let frame = layout.frame(bounds);
        assert_eq!(frame.origin.x, 0.0);
        assert_eq!(frame.origin.y, 40.0);
        assert_eq!(frame.size.width, 750.0);
        assert_eq!(frame.size.height, 461.0);
        assert_eq!(layout.bottom_inset(), 40.0);

        let footer_top = 32.0;
        assert_eq!(frame.origin.y - footer_top, 8.0);
        assert!(footer_top <= frame.origin.y);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn main_notes_and_dictation_share_the_detached_footer_backdrop_partition() {
        for window_name in ["Main window", "Notes", "Dictation overlay"] {
            assert!(
                super::window_name_owns_detached_footer(window_name),
                "{window_name} must own the shared floating footer partition"
            );
        }
        assert!(!super::window_name_owns_detached_footer("Actions popup"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn detached_footer_owners_use_region_preserving_exit() {
        for window_name in ["Main window", "Notes", "Dictation overlay"] {
            assert_eq!(
                super::glass_exit_mode(window_name),
                super::GlassExitMode::DetachedRegionsFadeOnly
            );
        }
        for window_name in ["Actions popup", "Confirm popup", "Inline popup"] {
            assert_eq!(
                super::glass_exit_mode(window_name),
                super::GlassExitMode::PopupTransformAndBlur
            );
        }
        assert_eq!(super::glass_exit_remove_delay().as_millis(), 135);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_glass_style_locks_t55_r_and_preserves_requested_tint_semantics() {
        let mut inherited = crate::theme::Theme::default();
        inherited.opacity.as_mut().unwrap().glass_tint_opacity = None;
        let mut explicit_zero = inherited.clone();
        explicit_zero
            .opacity
            .as_mut()
            .unwrap()
            .glass_tint_opacity = Some(0.0);
        let mut below_floor = inherited.clone();
        below_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.54);
        let mut at_floor = inherited.clone();
        at_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.55);
        let mut above_floor = inherited.clone();
        above_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.72);
        let light = crate::theme::Theme::light_default();

        let backdrop = super::resolve_native_glass_style(
            &inherited,
            super::NativeGlassSurfaceRole::WindowBackdrop,
        );
        let capsule = super::resolve_native_glass_style(
            &inherited,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        );
        let explicit = super::resolve_native_glass_style(
            &explicit_zero,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        );
        let below = super::resolve_native_glass_style(
            &below_floor,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        );
        let at = super::resolve_native_glass_style(
            &at_floor,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        );
        let above = super::resolve_native_glass_style(
            &above_floor,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        );
        let light_capsule = super::resolve_native_glass_style(
            &light,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        );

        assert_eq!(backdrop.signature.tint_rgb, capsule.signature.tint_rgb);
        assert_eq!(
            backdrop.signature.effective_tint_alpha_bits,
            capsule.signature.effective_tint_alpha_bits
        );
        assert_eq!(backdrop.signature.requested_tint_alpha_bits, None);
        assert_eq!(
            explicit.signature.requested_tint_alpha_bits,
            Some(0.0_f32.to_bits())
        );
        for style in [backdrop, capsule, explicit, below, at] {
            assert_eq!(style.effective_tint_alpha, 0.55);
        }
        assert_eq!(above.effective_tint_alpha, 0.72);
        assert_eq!(backdrop.veil_alpha, 0.0);
        assert_eq!(backdrop.rim_width, 0.0);
        assert_eq!(backdrop.signature.rim_rgba, 0xFFFF_FF00);
        assert_eq!(capsule.veil_alpha, 0.94);
        assert_eq!(capsule.rim_width, 1.0);
        assert_eq!(capsule.signature.rim_rgba, 0xFFFF_FF3D);
        assert_eq!(light_capsule.veil_alpha, 0.94);
        assert_eq!(light_capsule.rim_width, 1.0);
        assert_eq!(light_capsule.signature.rim_rgba, 0x0000_002E);
        assert_ne!(backdrop.signature, explicit.signature);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn secondary_backdrop_remains_full_window() {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        let bounds = NSRect::new(NSPoint::new(3.0, 5.0), NSSize::new(420.0, 280.0));
        let frame = super::TahoeBackdropLayout::FullWindow.frame(bounds);
        assert_eq!(frame.origin.x, bounds.origin.x);
        assert_eq!(frame.origin.y, bounds.origin.y);
        assert_eq!(frame.size.width, bounds.size.width);
        assert_eq!(frame.size.height, bounds.size.height);
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

use gpui::SharedString;
use std::time::Duration;

use crate::dictation::types::DictationSessionPhase;
use crate::dictation::visualizer::silent_bars;

// ---------------------------------------------------------------------------
// Overlay geometry & waveform contract constants
// ---------------------------------------------------------------------------

/// Glass bar width in pixels.
pub(crate) const OVERLAY_WIDTH_PX: f32 = 560.0;
/// Glass bar base height in pixels.
///
/// The overlay opens at this height (header row + one caption line + native
/// footer rail) and only ever GROWS while a session runs — one
/// [`TRANSCRIPT_LINE_HEIGHT_PX`] per wrapped caption line, up to
/// [`OVERLAY_MAX_EXTRA_CAPTION_LINES`], anchored at the bottom edge so the
/// pill extends upward. Phase changes never resize; content swaps inline.
pub(crate) const OVERLAY_HEIGHT_PX: f32 = 100.0;
/// Height of the header row: timer (left), destination verb chips (center),
/// target badge (right).
pub(crate) const OVERLAY_HEADER_ROW_HEIGHT_PX: f32 = 26.0;
/// Local footer/debug identity for the live overlay, separate from AppView footer ownership.
pub(crate) const DICTATION_OVERLAY_FOOTER_SURFACE: &str = "dictation_overlay";
/// Stable automation target id for the live dictation overlay window.
pub(crate) const DICTATION_OVERLAY_AUTOMATION_ID: &str = "dictation";
/// Confirming phase uses the same bar height so content swaps inline.
/// Rounded corner radius for the standalone glass bar.
pub(crate) const OVERLAY_RADIUS_PX: f32 = crate::ui::chrome::LIQUID_GLASS_PANEL_RADIUS_PX;
/// Horizontal padding inside the signal band content.
pub(crate) const OVERLAY_HORIZONTAL_PADDING_PX: f32 = 11.0;
/// Font size for timer, status labels, and chip verbs.
pub(crate) const STATUS_TEXT_SIZE_PX: f32 = 11.5;
/// Font size for the live transcript / caption line. Matches the main-menu
/// footer label size (`FooterMetricsTokens::label_font_size`) so the caption
/// reads comfortably from a distance.
pub(crate) const TRANSCRIPT_TEXT_SIZE_PX: f32 = 13.0;
/// Icon size inside the destination verb chips.
pub(crate) const CHIP_ICON_SIZE_PX: f32 = 11.0;
/// Width of the right-hand target badge slot (replaces spacer when target is shown).
pub(crate) const TARGET_BADGE_SLOT_WIDTH_PX: f32 = 108.0;
/// App icon size in the external-app destination badge.
pub(crate) const TARGET_BADGE_ICON_SIZE_PX: f32 = 16.0;

/// Number of waveform bars in the compact capsule visualizer.
pub(crate) const WAVEFORM_BAR_COUNT: usize = 9;
/// Width of each waveform bar in pixels.
pub(crate) const WAVEFORM_BAR_WIDTH_PX: f32 = 3.0;
/// Gap between waveform bars in pixels.
pub(crate) const WAVEFORM_BAR_GAP_PX: f32 = 3.0;
/// Minimum waveform bar height (silent level).
pub(crate) const WAVEFORM_BAR_MIN_HEIGHT_PX: f32 = 4.0;
/// Maximum waveform bar height (peak level).
pub(crate) const WAVEFORM_BAR_MAX_HEIGHT_PX: f32 = 18.0;

/// Duration of the fade applied to each newly revealed caption word.
pub(crate) const TRANSCRIPT_FADE_IN_MS: u64 = 280;

/// Line height of the wrapped caption block.
pub(crate) const TRANSCRIPT_LINE_HEIGHT_PX: f32 = 19.0;

/// How many wrapped caption lines beyond the first the window grows to fit
/// (Super Whisper pattern: the pill gets taller as the transcript
/// accumulates). Past this, older lines clip above the block while the
/// bottom-anchored layout keeps the newest words visible.
pub(crate) const OVERLAY_MAX_EXTRA_CAPTION_LINES: usize = 4;

/// Number of transcribing-state dots.
pub(crate) const TRANSCRIBING_DOT_COUNT: usize = 3;
/// Diameter of each transcribing dot.
pub(crate) const TRANSCRIBING_DOT_SIZE_PX: f32 = 4.0;
/// Gap between transcribing dots.
pub(crate) const TRANSCRIBING_DOT_GAP_PX: f32 = 4.0;

/// Threshold: if any bar exceeds this, we treat audio as "active" (green).
const SOUND_THRESHOLD: f32 = 0.10;

/// Bottom offset from the screen edge for dock clearance.
const OVERLAY_BOTTOM_OFFSET_PX: f32 = 15.0;

// ---------------------------------------------------------------------------
// Overlay helper functions
// ---------------------------------------------------------------------------

/// Format elapsed duration as `M:SS` for the compact timer display.
pub(crate) fn format_elapsed(elapsed: Duration) -> SharedString {
    let elapsed_secs = elapsed.as_secs();
    format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60).into()
}

/// Compute waveform bar opacity from a 0.0–1.0 audio level.
///
/// Matches vercel-voice JS: `clamp(0.3, value * 1.5, 1.0)`.
pub(crate) fn waveform_bar_opacity(level: f32) -> f32 {
    (level.clamp(0.0, 1.0) * 1.5).clamp(0.3, 1.0)
}

/// Compute waveform bar height from a 0.0–1.0 audio level.
///
/// Compact capsule curve: `min + pow(v, 0.7) * (max - min)`.
pub(crate) fn waveform_bar_height(level: f32) -> f32 {
    (WAVEFORM_BAR_MIN_HEIGHT_PX
        + level.clamp(0.0, 1.0).powf(0.7)
            * (WAVEFORM_BAR_MAX_HEIGHT_PX - WAVEFORM_BAR_MIN_HEIGHT_PX))
        .min(WAVEFORM_BAR_MAX_HEIGHT_PX)
}

/// Returns true if any bar exceeds the sound threshold.
pub(crate) fn has_sound(bars: &[f32; WAVEFORM_BAR_COUNT]) -> bool {
    bars.iter().any(|&bar| bar > SOUND_THRESHOLD)
}

/// Destination verb chips shown in the header row: explicit one-click
/// targets replacing the removed click-to-cycle badge. Order matches the
/// delivery verbs: Paste (frontmost app), Today (day page capture), Ask
/// (quick AI), Send (Agent Chat).
///
/// The third field is the Lucide icon name, chosen to match how each concept
/// is drawn elsewhere in the app: `clipboard-paste` (paste builtins),
/// `calendar-days` (day/today), `sparkles` (ask-AI builtins), and `bot`
/// (the Agent Chat builtin and footer agent chip).
pub(crate) const DICTATION_CHIP_TARGETS: [(crate::dictation::DictationTarget, &str, &str); 4] = [
    (
        crate::dictation::DictationTarget::ExternalApp,
        "Paste",
        "clipboard-paste",
    ),
    (
        crate::dictation::DictationTarget::DayPageToday,
        "Today",
        "calendar-days",
    ),
    (
        crate::dictation::DictationTarget::QuickAiQuestion,
        "Ask",
        "sparkles",
    ),
    (
        crate::dictation::DictationTarget::TabAiHarness,
        "Send",
        "bot",
    ),
];

/// Resolve a chip's Lucide icon name to an embedded asset path.
pub(crate) fn chip_icon_path(lucide_name: &str) -> Option<gpui::SharedString> {
    use gpui_component::IconNamed;
    crate::icons::lucide_from_str(lucide_name).map(|icon| icon.path())
}

/// Shared icon+verb chip styling for the destination row.
///
/// Used by both the runtime overlay (which adds click/tooltip handlers) and
/// the Storybook preview (which stays static), so the two can never drift.
fn destination_chip_base(
    verb: &'static str,
    icon: &'static str,
    is_active: bool,
    dimmed: bool,
    send_mode: bool,
) -> gpui::Stateful<Div> {
    let theme = get_cached_theme();
    let text_muted = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);
    let text_active = theme.colors.text.primary.with_opacity(OPACITY_ACTIVE);
    let label_color = if is_active { text_active } else { text_muted };

    let mut chip = div()
        .id(SharedString::from(format!("dictation-chip-{verb}")))
        .px(px(8.))
        .py(px(1.))
        .rounded(px(999.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .text_size(px(STATUS_TEXT_SIZE_PX - 1.0))
        .font_family(FONT_SYSTEM_UI)
        .text_color(label_color);

    if let Some(icon_path) = chip_icon_path(icon) {
        chip = chip.child(
            svg()
                .path(icon_path)
                .size(px(CHIP_ICON_SIZE_PX))
                .flex_shrink_0()
                .text_color(label_color),
        );
    }
    chip = chip.child(verb);
    if send_mode {
        chip = chip.child(
            div()
                .text_size(px(STATUS_TEXT_SIZE_PX - 1.0))
                .font_family(FONT_SYSTEM_UI)
                .text_color(label_color)
                .child("\u{21b5}"),
        );
    }

    if is_active {
        chip = chip
            .bg(theme.colors.background.main.with_opacity(OPACITY_ACTIVE))
            .border_1()
            .border_color(theme.colors.ui.border.with_opacity(OPACITY_SELECTED));
    } else {
        chip = chip.border_1().border_color(gpui::transparent_black());
    }

    if dimmed {
        chip = chip.opacity(0.55).cursor_default();
    }

    chip
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChipClickBehavior {
    Ignore,
    Retarget,
    SendTo,
}

pub(crate) fn chip_click_behavior(
    phase: &DictationSessionPhase,
    armed: bool,
    option_held: bool,
) -> ChipClickBehavior {
    if !matches!(
        phase,
        DictationSessionPhase::Recording | DictationSessionPhase::Confirming
    ) {
        return ChipClickBehavior::Ignore;
    }

    if !armed || option_held {
        ChipClickBehavior::Retarget
    } else {
        ChipClickBehavior::SendTo
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DestinationChipMode {
    Aim,
    Send,
}

pub(crate) fn destination_chip_mode(
    phase: &DictationSessionPhase,
    armed: bool,
    option_held: bool,
) -> DestinationChipMode {
    match chip_click_behavior(phase, armed, option_held) {
        ChipClickBehavior::SendTo => DestinationChipMode::Send,
        ChipClickBehavior::Ignore | ChipClickBehavior::Retarget => DestinationChipMode::Aim,
    }
}

/// Tooltip copy for a destination chip, naming the concrete outcome.
pub(crate) fn chip_tooltip_label(
    target: crate::dictation::DictationTarget,
    mode: DestinationChipMode,
) -> SharedString {
    let frontmost_app_name = || {
        crate::frontmost_app_tracker::get_last_real_app()
            .map(|app| app.name.trim().to_string())
            .filter(|name| !name.is_empty())
    };

    match mode {
        DestinationChipMode::Send => match target {
            crate::dictation::DictationTarget::ExternalApp => match frontmost_app_name() {
                Some(name) => format!("Stop & paste into {name}").into(),
                None => "Stop & paste into the frontmost app".into(),
            },
            crate::dictation::DictationTarget::DayPageToday => {
                "Stop & append to today's note".into()
            }
            crate::dictation::DictationTarget::QuickAiQuestion => "Stop & ask AI".into(),
            crate::dictation::DictationTarget::TabAiHarness => "Stop & send to Agent Chat".into(),
            other => other.overlay_label().into(),
        },
        DestinationChipMode::Aim => match target {
            crate::dictation::DictationTarget::ExternalApp => match frontmost_app_name() {
                Some(name) => format!("Dictate into {name}").into(),
                None => "Dictate into the frontmost app".into(),
            },
            crate::dictation::DictationTarget::DayPageToday => "Dictate to today's note".into(),
            crate::dictation::DictationTarget::QuickAiQuestion => "Dictate to AI".into(),
            crate::dictation::DictationTarget::TabAiHarness => "Dictate to Agent Chat".into(),
            other => other.overlay_label().into(),
        },
    }
}

/// Build the overlay badge label for the current delivery target.
///
/// External-app dictation names the tracked frontmost app, matching the
/// clipboard flow's "Paste to <app>" hint while keeping internal targets
/// explicit.
pub(crate) fn target_badge_label(target: crate::dictation::DictationTarget) -> SharedString {
    if matches!(target, crate::dictation::DictationTarget::ExternalApp) {
        if let Some(name) = crate::frontmost_app_tracker::get_last_real_app()
            .map(|app| app.name.trim().to_string())
            .filter(|name| !name.is_empty())
        {
            return name.into();
        }
    }

    target.overlay_label().into()
}

/// Resolve the tracked frontmost app's pre-decoded icon from the app launcher cache.
pub(crate) fn target_badge_frontmost_app_icon() -> Option<crate::app_launcher::DecodedIcon> {
    let bundle_id = crate::frontmost_app_tracker::get_last_real_app()?.bundle_id;
    let bundle_id = bundle_id.trim();
    if bundle_id.is_empty() {
        return None;
    }
    crate::app_launcher::cached_app_icon_for_bundle(bundle_id)
}

fn render_target_badge_content(target: crate::dictation::DictationTarget) -> AnyElement {
    if matches!(target, crate::dictation::DictationTarget::ExternalApp) {
        if let Some(icon) = target_badge_frontmost_app_icon() {
            return crate::icons::render_image(icon, TARGET_BADGE_ICON_SIZE_PX, 1.0);
        }
    }

    let theme = get_cached_theme();
    div()
        .text_size(px(STATUS_TEXT_SIZE_PX - 1.0))
        .font_family(FONT_SYSTEM_UI)
        .text_color(theme.colors.text.primary.with_opacity(OPACITY_ACTIVE))
        .max_w(px(TARGET_BADGE_SLOT_WIDTH_PX - 18.0))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .child(target_badge_label(target))
        .into_any_element()
}

/// Pulse cycle duration in seconds (matches vercel-voice 1.4s).
pub(crate) const TRANSCRIBING_PULSE_PERIOD_SECS: f64 = 1.4;
/// Stagger between consecutive dots in seconds (matches vercel-voice 0.2s).
pub(crate) const TRANSCRIBING_PULSE_STAGGER_SECS: f64 = 0.2;
/// Minimum dot opacity during pulse.
const PULSE_OPACITY_MIN: f32 = 0.3;
/// Maximum dot opacity during pulse.
const PULSE_OPACITY_MAX: f32 = 1.0;

/// Static opacities for reduced-motion fallback (no animation).
pub(crate) fn transcribing_dot_opacities_static() -> [f32; TRANSCRIBING_DOT_COUNT] {
    [OPACITY_SELECTED, OPACITY_ACTIVE, OPACITY_SELECTED]
}

/// Compute time-based staggered pulse opacities for the 3-dot transcribing animation.
///
/// Each dot follows a sine-wave pulse with a per-dot phase offset:
/// `opacity = min + (max - min) * (0.5 + 0.5 * sin(2π * (t - i * stagger) / period))`
///
/// Matches vercel-voice: 1.4s cycle, 0.2s stagger between dots.
pub(crate) fn transcribing_dot_opacities_at(elapsed_secs: f64) -> [f32; TRANSCRIBING_DOT_COUNT] {
    let mut opacities = [0.0_f32; TRANSCRIBING_DOT_COUNT];
    for (i, opacity) in opacities.iter_mut().enumerate() {
        let phase = elapsed_secs - (i as f64 * TRANSCRIBING_PULSE_STAGGER_SECS);
        let t = std::f64::consts::TAU * phase / TRANSCRIBING_PULSE_PERIOD_SECS;
        let wave = 0.5 + 0.5 * t.sin();
        *opacity = PULSE_OPACITY_MIN + (PULSE_OPACITY_MAX - PULSE_OPACITY_MIN) * wave as f32;
    }
    opacities
}

/// Gentle pulse cycle for the caption while it is being processed
/// (transcribing/delivering), signalling "working on this text".
pub(crate) const PROCESSING_PULSE_PERIOD_SECS: f64 = 1.6;
/// Minimum caption opacity during the processing pulse.
pub(crate) const PROCESSING_PULSE_OPACITY_MIN: f32 = 0.45;
/// Maximum caption opacity during the processing pulse.
pub(crate) const PROCESSING_PULSE_OPACITY_MAX: f32 = 0.9;

/// Compute the processing-caption pulse opacity at a point in time.
///
/// A plain sine breath between the min/max opacities; the reduced-motion
/// fallback is the static midpoint.
pub(crate) fn processing_pulse_opacity_at(elapsed_secs: f64) -> f32 {
    let t = std::f64::consts::TAU * elapsed_secs / PROCESSING_PULSE_PERIOD_SECS;
    let wave = 0.5 + 0.5 * t.sin();
    PROCESSING_PULSE_OPACITY_MIN
        + (PROCESSING_PULSE_OPACITY_MAX - PROCESSING_PULSE_OPACITY_MIN) * wave as f32
}

/// Static processing-caption opacity for the reduced-motion fallback.
pub(crate) fn processing_pulse_opacity_static() -> f32 {
    (PROCESSING_PULSE_OPACITY_MIN + PROCESSING_PULSE_OPACITY_MAX) / 2.0
}

/// Snapshot of the dictation overlay's visual state.
///
/// Updated on every level/phase change and consumed by the overlay renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct DictationOverlayState {
    pub phase: DictationSessionPhase,
    pub elapsed: Duration,
    pub bars: [f32; WAVEFORM_BAR_COUNT],
    pub transcript: SharedString,
    pub target: crate::dictation::DictationTarget,
}

impl Default for DictationOverlayState {
    fn default() -> Self {
        Self {
            phase: DictationSessionPhase::Idle,
            elapsed: Duration::ZERO,
            bars: silent_bars(),
            transcript: SharedString::default(),
            target: crate::dictation::DictationTarget::ExternalApp,
        }
    }
}

// ---------------------------------------------------------------------------
// GPUI overlay entity + window lifecycle
// ---------------------------------------------------------------------------

use gpui::{
    div, prelude::*, px, relative, rgba, svg, AnyElement, App, Context, Div, FocusHandle,
    Focusable, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, StyledText, Task, TextRun, Window, WindowBounds,
    WindowOptions,
};

use crate::list_item::FONT_SYSTEM_UI;
use crate::theme::get_cached_theme;
use crate::theme::opacity::{OPACITY_ACTIVE, OPACITY_SELECTED, OPACITY_SUBTLE, OPACITY_TEXT_MUTED};
use crate::ui_foundation::HexColorExt;

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

/// Monotonic generation counter for overlay sessions.
///
/// Incremented each time a new overlay window is opened. Async tasks
/// (pump, scheduled close) compare their captured generation against the
/// current value and bail when stale, preventing a delayed close from
/// killing a newly opened overlay.
static OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Global handle so we can reach the overlay from any callsite.
static DICTATION_OVERLAY_WINDOW: OnceLock<Mutex<Option<gpui::WindowHandle<DictationOverlay>>>> =
    OnceLock::new();
static DICTATION_OVERLAY_EXIT_TICKET: Mutex<Option<crate::platform::GlassExitTicket>> =
    Mutex::new(None);
static DICTATION_OVERLAY_EXIT_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static DICTATION_OVERLAY_FIXTURE_MODE: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_dictation_overlay_fixture_mode(active: bool) {
    DICTATION_OVERLAY_FIXTURE_MODE.store(active, Ordering::SeqCst);
}

pub(crate) fn dictation_overlay_fixture_mode() -> bool {
    DICTATION_OVERLAY_FIXTURE_MODE.load(Ordering::SeqCst)
}

/// Callback type for overlay escape actions (abort dictation).
type OverlayAbortCallback = Box<dyn Fn(&mut App) + Send + Sync + 'static>;
/// Callback type for overlay submit actions (stop and transcribe dictation).
type OverlaySubmitCallback = Box<dyn Fn(&mut App) + Send + Sync + 'static>;

/// Global abort callback set by the dictation runtime.
static OVERLAY_ABORT_CALLBACK: Mutex<Option<OverlayAbortCallback>> = Mutex::new(None);
/// Global submit callback set by the dictation runtime.
static OVERLAY_SUBMIT_CALLBACK: Mutex<Option<OverlaySubmitCallback>> = Mutex::new(None);

/// Register a callback to be invoked when the user confirms stop via
/// Enter or the Stop button in the overlay.
pub fn set_overlay_abort_callback(callback: impl Fn(&mut App) + Send + Sync + 'static) {
    *OVERLAY_ABORT_CALLBACK.lock() = Some(Box::new(callback));
}

/// Register a callback to be invoked when the user clicks Stop on the
/// recording overlay.
pub fn set_overlay_submit_callback(callback: impl Fn(&mut App) + Send + Sync + 'static) {
    *OVERLAY_SUBMIT_CALLBACK.lock() = Some(Box::new(callback));
}

// ---------------------------------------------------------------------------
// Global escape monitor (fires even when overlay is not focused)
// ---------------------------------------------------------------------------

/// Wrapper for NSEvent monitor ID to make it Send+Sync.
///
/// The monitor ID is created and removed on the main thread; the static only
/// provides cross-thread visibility for the cleanup path.
#[cfg(target_os = "macos")]
struct SendableId(cocoa::base::id);
// SAFETY: The NSEvent monitor ID is created on the main thread and only
// accessed behind a Mutex. The cleanup call (removeMonitor:) also runs on
// the main thread. The raw pointer never escapes to a non-main thread.
#[cfg(target_os = "macos")]
unsafe impl Send for SendableId {}
#[cfg(target_os = "macos")]
unsafe impl Sync for SendableId {}

/// Holds the NSEvent global monitor ID so we can remove it on close.
#[cfg(target_os = "macos")]
static GLOBAL_ESCAPE_MONITOR: Mutex<Option<SendableId>> = Mutex::new(None);

/// Install a global key-down monitor that catches Escape pressed in any app.
///
/// `NSEvent addGlobalMonitorForEventsMatchingMask:handler:` fires for events
/// delivered to OTHER applications — our own window receives `KeyDownEvent`
/// via GPUI's normal path, so there's no double-fire.
///
/// The monitor sets `ESCAPE_REQUESTED` to `true`; the overlay pump (16ms tick)
/// picks it up in GPUI context where it can safely mutate state.
#[cfg(target_os = "macos")]
fn install_global_escape_monitor() {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    OPTION_HELD.store(false, Ordering::SeqCst);

    if crate::platform::require_main_thread("install_global_escape_monitor") {
        return;
    }

    // Already installed — don't double-register.
    if GLOBAL_ESCAPE_MONITOR.lock().is_some() {
        return;
    }

    // NSEventMaskKeyDown = 1 << 10, NSEventMaskFlagsChanged = 1 << 12
    let mask: u64 = (1 << 10) | (1 << 12);

    let block = block::ConcreteBlock::new(move |event: id| {
        // SAFETY: `event` is a valid NSEvent passed by AppKit.
        let event_type: u64 = unsafe { msg_send![event, type] };
        if event_type == 12 {
            // NSEventModifierFlagOption = 1 << 19
            let flags: u64 = unsafe { msg_send![event, modifierFlags] };
            OPTION_HELD.store((flags & (1 << 19)) != 0, Ordering::SeqCst);
            return;
        }

        if event_type == 10 {
            // keyCode 53 = Escape, keyCode 36 = Return/Enter.
            let key_code: u16 = unsafe { msg_send![event, keyCode] };
            match key_code {
                53 => {
                    tracing::info!(
                        category = "DICTATION",
                        "Global key monitor: Escape pressed in external app"
                    );
                    ESCAPE_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                36 => {
                    tracing::info!(
                        category = "DICTATION",
                        "Global key monitor: Enter pressed in external app"
                    );
                    ENTER_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                _ => {}
            }
        }
    });
    let block = block.copy();

    // SAFETY: NSEvent is a valid AppKit class on macOS.
    // addGlobalMonitorForEventsMatchingMask:handler: is called on the main
    // thread (open_dictation_overlay runs on main). The returned monitor ID
    // is stored in GLOBAL_ESCAPE_MONITOR for cleanup.
    let monitor: id = unsafe {
        let ns_event_class = class!(NSEvent);
        msg_send![
            ns_event_class,
            addGlobalMonitorForEventsMatchingMask: mask
            handler: &*block
        ]
    };

    if monitor != nil {
        *GLOBAL_ESCAPE_MONITOR.lock() = Some(SendableId(monitor));
        tracing::debug!(category = "DICTATION", "Global escape monitor installed");
    } else {
        tracing::warn!(
            category = "DICTATION",
            "Failed to install global escape monitor"
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn install_global_escape_monitor() {
    OPTION_HELD.store(false, Ordering::SeqCst);
}

/// Remove the global key-down monitor.
#[cfg(target_os = "macos")]
fn remove_global_escape_monitor() {
    use objc::{class, msg_send, sel, sel_impl};

    OPTION_HELD.store(false, Ordering::SeqCst);

    if crate::platform::require_main_thread("remove_global_escape_monitor") {
        return;
    }

    let monitor = GLOBAL_ESCAPE_MONITOR.lock().take();
    if let Some(SendableId(monitor)) = monitor {
        // SAFETY: monitor is a valid id returned by
        // addGlobalMonitorForEventsMatchingMask:handler:.
        // removeMonitor: is the correct cleanup API.
        unsafe {
            let _: () = msg_send![class!(NSEvent), removeMonitor: monitor];
        }
        tracing::debug!(category = "DICTATION", "Global escape monitor removed");
    }
}

#[cfg(not(target_os = "macos"))]
fn remove_global_escape_monitor() {
    OPTION_HELD.store(false, Ordering::SeqCst);
}

/// Flag: the global key monitor detected an Escape press that the overlay
/// needs to process. Checked by `process_global_keys_if_requested` inside
/// GPUI context on every pump tick.
static ESCAPE_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Flag: the global key monitor detected an Enter press while in Confirming
/// phase. Enter in Confirming = stop and transcribe the session.
static ENTER_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Flag: the Option (⌥) key is currently held. Fed by the global
/// FlagsChanged monitor (other app focused) and the overlay's own
/// `on_modifiers_changed` listener (overlay focused); read at render time to
/// flip the destination chips between send and aim affordances. Reset on
/// monitor install/remove so state never leaks across sessions.
static OPTION_HELD: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Confirming-phase copy constants (single source of truth)
// ---------------------------------------------------------------------------

/// Single-word action label for stopping/submitting the current recording.
const ACTION_STOP_LABEL: &str = "Stop";
/// Opens the microphone picker; the live session keeps its opened mic.
const ACTION_MIC_LABEL: &str = "Select Mic";
/// Single-word action label for discarding the current recording.
const ACTION_CANCEL_LABEL: &str = "Cancel";
/// Single-word action label for discarding from the confirmation state.
const ACTION_DISCARD_LABEL: &str = "Discard";
/// Single-word action label for resuming from confirmation.
const ACTION_CONTINUE_LABEL: &str = "Continue";
/// Single-word action label for closing terminal overlay states.
const ACTION_CLOSE_LABEL: &str = "Close";
/// Keycap shown for Escape.
const ESC_KEYCAP: &str = "esc";
/// Keycap shown for Enter.
const ENTER_KEYCAP: &str = "\u{21b5}";
/// Keycap shown for Backspace (discard from confirmation).
const BACKSPACE_KEYCAP: &str = "\u{232b}";
/// Keycap token rendered as a Lucide microphone glyph by footer chrome.
const MIC_KEYCAP: &str = crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN;

/// Interval between animation ticks for the transcribing dot pulse (ms).
const TRANSCRIBING_TICK_MS: u64 = 50;

/// What the overlay should do when Escape is pressed in a given phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayEscapeAction {
    /// First Escape during recording — show confirmation UI, don't abort yet.
    TransitionToConfirming,
    /// Escape during Confirming — dismiss confirmation and resume recording.
    ResumeRecording,
    /// Escape during a short recording (or Backspace during Confirming) —
    /// abort the session and discard the audio.
    AbortSession,
    CloseOverlay,
    Propagate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalKeyProcessResult {
    None,
    StateChanged,
    Closed,
}

/// Return phase-appropriate (headline, action label) copy for the dictation overlay.
///
/// The headline is the primary status text (e.g. "Listening…", "Stop dictation?").
/// The action label names the visible compact action chip for each phase.
pub(crate) fn overlay_phase_copy(phase: &DictationSessionPhase) -> (&'static str, &'static str) {
    match phase {
        DictationSessionPhase::Recording => ("Listening\u{2026}", ACTION_CANCEL_LABEL),
        DictationSessionPhase::Confirming => ("Stop dictation?", ACTION_CONTINUE_LABEL),
        DictationSessionPhase::Transcribing => ("Transcribing\u{2026}", ACTION_CLOSE_LABEL),
        DictationSessionPhase::Delivering => ("Delivering\u{2026}", ACTION_CLOSE_LABEL),
        DictationSessionPhase::Finished => ("Done", ACTION_CLOSE_LABEL),
        DictationSessionPhase::Failed(_) => ("Dictation failed", ACTION_CLOSE_LABEL),
        DictationSessionPhase::Idle => ("", ""),
    }
}

/// Recordings shorter than this abort immediately on Escape; longer ones
/// enter a confirmation state where Escape means "continue" and Enter means
/// "stop."  Mirrors the vercel-voice 5-second threshold.
const ESCAPE_CONFIRM_THRESHOLD: Duration = Duration::from_secs(5);

/// Map a dictation session phase + elapsed time to the appropriate Escape behavior.
///
/// Follows the vercel-voice confirm-first pattern:
/// - Recording (< 5 s) → immediate abort
/// - Recording (≥ 5 s) → show confirmation (first Escape)
/// - Confirming → dismiss confirmation and resume recording
pub(crate) fn overlay_escape_action(
    phase: &DictationSessionPhase,
    elapsed: Duration,
) -> OverlayEscapeAction {
    match phase {
        DictationSessionPhase::Recording if elapsed < ESCAPE_CONFIRM_THRESHOLD => {
            OverlayEscapeAction::AbortSession
        }
        DictationSessionPhase::Recording => OverlayEscapeAction::TransitionToConfirming,
        DictationSessionPhase::Confirming => OverlayEscapeAction::ResumeRecording,
        DictationSessionPhase::Transcribing
        | DictationSessionPhase::Delivering
        | DictationSessionPhase::Finished
        | DictationSessionPhase::Failed(_) => OverlayEscapeAction::CloseOverlay,
        DictationSessionPhase::Idle => OverlayEscapeAction::Propagate,
    }
}

/// Resize the overlay window to a new height, growing toward the screen
/// center so the pill never extends offscreen.
///
/// AppKit frames are bottom-left origin with y increasing upward. When the
/// window sits in the bottom half of its screen (the default resting spot
/// above the dock) the bottom edge stays fixed and the top rises; when the
/// user has moved it to the top half, the top edge stays fixed and the
/// bottom descends.
#[cfg(target_os = "macos")]
fn resize_overlay_height(window: &mut Window, target_height: f32) {
    if let Ok(wh) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = wh.as_raw() {
            use cocoa::base::id;
            use cocoa::foundation::NSRect;
            use objc::{msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as id;
            // SAFETY: ns_view is from the live overlay window on the main thread.
            // We read the current frame, pick the anchored edge from the
            // window's position on its screen, and apply via setFrame:display:.
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window.is_null() {
                    return;
                }
                let current_frame: NSRect = msg_send![ns_window, frame];
                let old_height = current_frame.size.height;
                let new_height = target_height as f64;
                if (new_height - old_height).abs() < 0.5 {
                    return;
                }

                let screen: id = msg_send![ns_window, screen];
                let grow_down = if screen.is_null() {
                    false
                } else {
                    let visible: NSRect = msg_send![screen, visibleFrame];
                    let window_center = current_frame.origin.y + old_height / 2.0;
                    let screen_center = visible.origin.y + visible.size.height / 2.0;
                    window_center > screen_center
                };

                let origin_y = if grow_down {
                    // Top edge fixed: the bottom-left origin descends as the
                    // window gets taller.
                    current_frame.origin.y - (new_height - old_height)
                } else {
                    // Bottom edge fixed: the origin holds and the top rises.
                    current_frame.origin.y
                };

                let new_frame = NSRect::new(
                    cocoa::foundation::NSPoint::new(current_frame.origin.x, origin_y),
                    cocoa::foundation::NSSize::new(current_frame.size.width, new_height),
                );
                let () = msg_send![ns_window, setFrame: new_frame display: cocoa::base::YES];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn resize_overlay_height(_window: &mut Window, _target_height: f32) {}

/// Estimate how many lines `text` wraps to at ~`chars_per_line` characters,
/// using greedy word wrap. Drives window growth only — the text system does
/// the real wrapping — so being off by a character or two just means the
/// block clips a hair earlier or later.
pub(crate) fn estimate_caption_lines(text: &str, chars_per_line: usize) -> usize {
    let chars_per_line = chars_per_line.max(1);
    let mut lines = 0usize;
    let mut current = 0usize;
    for word in text.split_whitespace() {
        let word_len = word.chars().count().min(chars_per_line);
        let needed = if current == 0 {
            word_len
        } else {
            current + 1 + word_len
        };
        if current == 0 || needed > chars_per_line {
            lines += 1;
            current = word_len;
        } else {
            current = needed;
        }
    }
    lines.max(1)
}

#[derive(Clone)]
struct DictationMicrophonePopupLifetime {
    lifecycle: crate::components::inline_popup_window::InlinePopupLifecycleHandle,
    focus_return: crate::components::inline_popup_window::InlinePopupFocusReturn,
}

impl DictationMicrophonePopupLifetime {
    fn generation(&self) -> crate::components::inline_popup_window::InlinePopupGeneration {
        crate::components::inline_popup_window::InlinePopupLifecycle::generation(&self.lifecycle)
    }
}

/// The GPUI entity that renders the compact dictation pill.
pub struct DictationOverlay {
    state: DictationOverlayState,
    display_bars: [f32; WAVEFORM_BAR_COUNT],
    focus_handle: FocusHandle,
    microphone_popup_lifetime: Option<DictationMicrophonePopupLifetime>,
    last_render_logged_phase: Option<DictationSessionPhase>,
    /// When the processing (transcribing/delivering) animation started, for
    /// dot-stagger and caption-pulse phase computation.
    processing_started_at: Option<Instant>,
    /// Whether the user has "Reduce motion" enabled in system accessibility.
    reduced_motion: bool,
    /// Paces the word-by-word caption reveal between raw partial updates.
    caption: crate::dictation::live_caption::LiveCaption,
    /// Extra wrapped caption lines the window has grown to fit this session.
    /// Grows monotonically (never shrinks mid-session) so the pill is stable.
    extra_caption_lines: usize,
    /// Scroll position of the caption block once it exceeds the max window
    /// height — auto-follows the newest text, but the user can scroll back.
    caption_scroll: gpui::ScrollHandle,
    /// Caption generation the scroll last followed, so auto-follow fires
    /// once per new word instead of fighting the user's scrollback.
    caption_scrolled_generation: u64,
    /// Keeps the processing tick loop alive; dropped when phase changes.
    _animation_task: Option<Task<()>>,
    /// Drains native footer button clicks for the dictation window.
    _footer_action_task: Option<Task<()>>,
}

/// Phases where the app is working on the captured audio and the overlay
/// should hold its layout with the caption pulsing.
fn is_processing_phase(phase: &DictationSessionPhase) -> bool {
    matches!(
        phase,
        DictationSessionPhase::Transcribing | DictationSessionPhase::Delivering
    )
}

impl DictationOverlay {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: DictationOverlayState::default(),
            display_bars: silent_bars(),
            focus_handle: cx.focus_handle(),
            microphone_popup_lifetime: None,
            last_render_logged_phase: None,
            processing_started_at: None,
            reduced_motion: crate::platform::prefers_reduced_motion(),
            caption: crate::dictation::live_caption::LiveCaption::default(),
            extra_caption_lines: 0,
            caption_scroll: gpui::ScrollHandle::new(),
            caption_scrolled_generation: 0,
            _animation_task: None,
            _footer_action_task: None,
        }
    }

    fn hide_glass_button_frames(&self, window: &Window) {
        crate::components::footer_chrome::remove_glass_capsule_group(
            window,
            "dictation-action-rail",
        );
        crate::components::footer_chrome::remove_glass_capsule_group(
            window,
            "dictation-header-targets",
        );
    }

    fn teardown_glass_button_host(&self, window: &Window) {
        crate::components::footer_chrome::remove_glass_capsule_window(window);
    }

    /// Scrollable runtime caption container, sharing the preview block's
    /// geometry. Content beyond the max window height scrolls; auto-follow
    /// is driven by [`Self::sync_caption_follow`].
    fn caption_scroll_container(&self) -> gpui::Stateful<Div> {
        render_caption_block_container()
            .id("dictation-caption-scroll")
            .overflow_y_scroll()
            .track_scroll(&self.caption_scroll)
    }

    /// Keep the caption scrolled to the newest text: fires once per caption
    /// change so a user scrolling back is not fought mid-read, but every new
    /// word snaps the view back to the live tail.
    fn sync_caption_follow(&mut self) {
        if self.caption.generation() != self.caption_scrolled_generation {
            self.caption_scrolled_generation = self.caption.generation();
            self.caption_scroll.scroll_to_bottom();
        }
    }

    fn transcript_armed(&self) -> bool {
        !self.caption.visible_text().trim().is_empty()
    }

    fn ensure_native_footer_action_listener(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self._footer_action_task.is_some() {
            return;
        }

        let rx = crate::footer_popup::dictation_footer_action_channel()
            .1
            .clone();
        self._footer_action_task = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(action) = rx.recv().await {
                if let Err(error) = this.update_in(cx, |overlay, window, cx| {
                    overlay.handle_native_footer_action(action, window, cx);
                }) {
                    tracing::warn!(
                        target: "script_kit::dictation",
                        event = "dictation_native_footer_action_dispatch_failed",
                        action = ?action,
                        %error,
                        "Failed to dispatch native footer action into DictationOverlay"
                    );
                }
            }
        }));
    }

    fn handle_native_footer_action(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::footer_popup::FooterAction;

        match action {
            FooterAction::Stop | FooterAction::Run | FooterAction::Apply => {
                if matches!(
                    self.state.phase,
                    DictationSessionPhase::Recording | DictationSessionPhase::Confirming
                ) {
                    if self.transcript_armed() {
                        self.submit_overlay_session(window, cx);
                    } else {
                        tracing::debug!(
                            category = "DICTATION",
                            phase = ?self.state.phase,
                            "Ignoring footer submit action because transcript is unarmed"
                        );
                    }
                } else {
                    self.close_overlay_from_within(window, cx);
                }
            }
            FooterAction::Ai | FooterAction::PasteResponse => {
                if self.state.phase == DictationSessionPhase::Recording {
                    self.open_microphone_picker(window, cx);
                }
            }
            // Discard slot: only shown in Confirming; discards the recording.
            FooterAction::Actions => {
                if self.state.phase == DictationSessionPhase::Confirming {
                    self.abort_overlay_session(window, cx);
                } else {
                    self.close_overlay_from_within(window, cx);
                }
            }
            FooterAction::Close => {
                if self.state.phase == DictationSessionPhase::Confirming {
                    self.resume_recording(window, cx);
                } else if self.state.phase == DictationSessionPhase::Recording {
                    self.abort_overlay_session(window, cx);
                } else {
                    self.close_overlay_from_within(window, cx);
                }
            }
            FooterAction::Replace
            | FooterAction::Append
            | FooterAction::Copy
            | FooterAction::Expand
            | FooterAction::Retry
            | FooterAction::Cwd
            | FooterAction::AgentModel
            | FooterAction::Tips => {}
        }
    }

    /// Enter the confirming state: update phase, notify.
    ///
    /// No resize needed — confirming swaps content inline at the same pill height.
    fn enter_confirming(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.state.phase = DictationSessionPhase::Confirming;
        crate::dictation::set_overlay_phase(DictationSessionPhase::Confirming);
        cx.notify();
    }

    /// Dismiss the confirmation state and resume recording.
    fn resume_recording(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.state.phase = DictationSessionPhase::Recording;
        crate::dictation::set_overlay_phase(DictationSessionPhase::Recording);
        cx.notify();
    }

    /// Open the attached microphone picker.
    ///
    /// The current recording keeps using the device it opened with; this updates
    /// the persisted preference used by the next dictation capture.
    fn open_microphone_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.phase != DictationSessionPhase::Recording {
            crate::dictation::close_dictation_microphone_popup_window(cx);
            return;
        }

        let selection_mode = if dictation_overlay_fixture_mode() {
            crate::dictation::DictationMicrophonePopupSelectionMode::FixtureNoPersistence
        } else {
            crate::dictation::DictationMicrophonePopupSelectionMode::Production
        };
        let menu_items = if selection_mode
            == crate::dictation::DictationMicrophonePopupSelectionMode::FixtureNoPersistence
        {
            vec![
                crate::dictation::DictationDeviceMenuItem {
                    title: "System Default".to_string(),
                    subtitle: "Fixture current device".to_string(),
                    action: crate::dictation::DictationDeviceSelectionAction::UseSystemDefault,
                    is_selected: true,
                },
                crate::dictation::DictationDeviceMenuItem {
                    title: "Studio Microphone".to_string(),
                    subtitle: "Fixture alternate device".to_string(),
                    action: crate::dictation::DictationDeviceSelectionAction::UseDevice(
                        crate::dictation::DictationDeviceId(
                            "fixture-studio-microphone".to_string(),
                        ),
                    ),
                    is_selected: false,
                },
            ]
        } else {
            let prefs = crate::config::load_user_preferences();
            let selected_device_id = prefs.dictation.selected_device_id.as_deref();
            match crate::dictation::list_input_device_menu_items(selected_device_id) {
                Ok(items) if !items.is_empty() => items,
                Ok(_) => {
                    tracing::warn!(
                        category = "DICTATION",
                        "Microphone selector found no input devices"
                    );
                    return;
                }
                Err(error) => {
                    tracing::warn!(
                        category = "DICTATION",
                        error = %error,
                        "Failed to list microphones from overlay selector"
                    );
                    return;
                }
            }
        };

        let parent_bounds = window.bounds();
        let parent_window_handle = window.window_handle();
        let display = window.display(cx);
        let display_id = display.as_ref().map(|display| display.id());
        let display_bounds = display.as_ref().map(|display| display.visible_bounds());
        let width = crate::components::inline_popup_window::inline_popup_width_for_window(
            parent_bounds.size.width.as_f32(),
        );
        let snapshot =
            crate::dictation::build_dictation_microphone_popup_snapshot(menu_items, width);

        let lifetime = self
            .microphone_popup_lifetime
            .as_ref()
            .filter(|lifetime| {
                !matches!(
                    crate::components::inline_popup_window::InlinePopupLifecycle::snapshot(
                        &lifetime.lifecycle,
                    )
                    .1,
                    crate::components::inline_popup_window::InlinePopupPhase::Closing
                        | crate::components::inline_popup_window::InlinePopupPhase::Closed
                )
            })
            .cloned()
            .unwrap_or_else(|| {
                let lifecycle = crate::components::inline_popup_window::InlinePopupLifecycle::new();
                let generation =
                    crate::components::inline_popup_window::InlinePopupLifecycle::generation(
                        &lifecycle,
                    );
                DictationMicrophonePopupLifetime {
                    lifecycle,
                    focus_return: crate::components::inline_popup_window::InlinePopupFocusReturn {
                        generation,
                        parent_automation_id: DICTATION_OVERLAY_AUTOMATION_ID.to_string(),
                        parent_window_handle,
                        focus_handle: self.focus_handle.clone(),
                        semantic_id: "input:dictation-overlay",
                    },
                }
            });
        self.microphone_popup_lifetime = Some(lifetime.clone());

        let request = crate::dictation::DictationMicrophonePopupRequest {
            parent_window_handle,
            parent_automation_id: DICTATION_OVERLAY_AUTOMATION_ID.to_string(),
            parent_bounds,
            display_bounds,
            display_id,
            source_view: cx.entity().downgrade(),
            snapshot,
            selection_mode,
            lifecycle: lifetime.lifecycle.clone(),
            focus_return: lifetime.focus_return.clone(),
        };

        if let Err(error) = crate::dictation::sync_dictation_microphone_popup_window(cx, request) {
            if self
                .microphone_popup_lifetime
                .as_ref()
                .is_some_and(|current| current.generation() == lifetime.generation())
            {
                self.microphone_popup_lifetime = None;
            }
            tracing::warn!(
                category = "DICTATION",
                error = %error,
                "Failed to open dictation microphone popup"
            );
            return;
        }

        cx.notify();
    }

    fn dismiss_microphone_popup_top_layer(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) -> crate::dictation::DictationPopupDismissOutcome {
        let expected_generation = self
            .microphone_popup_lifetime
            .as_ref()
            .map(DictationMicrophonePopupLifetime::generation);
        let outcome = crate::dictation::dismiss_dictation_microphone_popup_from_parent(
            expected_generation,
            reason,
            cx,
        );
        if let Some(generation) = expected_generation {
            self.dismiss_microphone_popup_from_window(generation, reason, cx);
        }
        outcome
    }

    pub(crate) fn dismiss_microphone_popup_from_window(
        &mut self,
        generation: crate::components::inline_popup_window::InlinePopupGeneration,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !self
            .microphone_popup_lifetime
            .as_ref()
            .is_some_and(|lifetime| lifetime.generation() == generation)
        {
            tracing::debug!(
                target: "script_kit::inline_popup",
                event = "dictation_microphone_popup_stale_owner_callback",
                generation = generation.get(),
                reason,
            );
            return;
        }

        self.microphone_popup_lifetime = None;
        tracing::info!(
            target: "script_kit::inline_popup",
            event = "dictation_microphone_popup_owner_reconciled",
            generation = generation.get(),
            reason,
        );
        cx.notify();
    }

    /// Abort the dictation session via the registered callback and close
    /// the overlay window directly.
    ///
    /// This must NOT call `close_dictation_overlay()` because we are
    /// already inside `&mut self` (via `handle_key_down` or a mouse
    /// handler).  Calling `handle.update()` on the same entity would be a
    /// reentrant borrow that silently fails, leaving the window alive but
    /// the global slot empty — causing stacked overlay windows on the
    /// next toggle.
    fn abort_overlay_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::dictation::close_dictation_microphone_popup_window_for_owner_loss(cx);
        let callback = OVERLAY_ABORT_CALLBACK.lock().take();
        *OVERLAY_SUBMIT_CALLBACK.lock() = None;
        remove_global_escape_monitor();
        self.dematerialize_then_remove_overlay(window, cx);
        if let Some(cb) = callback {
            cx.defer(move |cx| {
                cb(cx);
            });
        }
        tracing::info!(
            category = "DICTATION",
            "Overlay closed from within entity (abort)"
        );
    }

    /// Submit the active recording via the registered callback.
    ///
    /// Closing happens before callback dispatch so the app-owned stop path can
    /// Play the Spotlight dematerialize (same as the main window's exit),
    /// then run the native close prep and remove the overlay window. Falls
    /// back to instant close when glass/morph is unavailable.
    fn dematerialize_then_remove_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        set_dictation_overlay_fixture_mode(false);
        if let Some(ticket) = crate::platform::begin_gpui_window_exit_with_ticket(
            window,
            "DICTATION",
            "Dictation overlay",
        ) {
            *DICTATION_OVERLAY_EXIT_TICKET.lock() = Some(ticket);
            DICTATION_OVERLAY_EXIT_IN_PROGRESS.store(true, Ordering::SeqCst);
            let any_handle = window.window_handle();
            cx.spawn(async move |_this, cx: &mut gpui::AsyncApp| {
                cx.background_executor()
                    .timer(crate::platform::glass_exit_remove_delay())
                    .await;
                cx.update(|cx| {
                    let pending_matches = *DICTATION_OVERLAY_EXIT_TICKET.lock() == Some(ticket);
                    if !pending_matches || !crate::platform::glass_exit_ticket_is_current(ticket) {
                        return;
                    }
                    DICTATION_OVERLAY_EXIT_TICKET.lock().take();
                    crate::platform::record_glass_exit_commit(ticket);
                    let _ = any_handle.update(cx, |_view, window, _cx| {
                        crate::components::footer_chrome::remove_glass_capsule_window(window);
                        prepare_overlay_window_for_close(window);
                        window.remove_window();
                    });
                    DICTATION_OVERLAY_WINDOW
                        .get_or_init(|| Mutex::new(None))
                        .lock()
                        .take();
                    crate::windows::remove_runtime_window_handle(DICTATION_OVERLAY_AUTOMATION_ID);
                    crate::windows::remove_automation_window(DICTATION_OVERLAY_AUTOMATION_ID);
                    DICTATION_OVERLAY_EXIT_IN_PROGRESS.store(false, Ordering::SeqCst);
                });
            })
            .detach();
        } else {
            self.teardown_glass_button_host(window);
            prepare_overlay_window_for_close(window);
            window.remove_window();
            DICTATION_OVERLAY_WINDOW
                .get_or_init(|| Mutex::new(None))
                .lock()
                .take();
            crate::windows::remove_runtime_window_handle(DICTATION_OVERLAY_AUTOMATION_ID);
            crate::windows::remove_automation_window(DICTATION_OVERLAY_AUTOMATION_ID);
            DICTATION_OVERLAY_EXIT_IN_PROGRESS.store(false, Ordering::SeqCst);
        }
    }

    /// reopen/update the overlay into its transcribing state without reentrant
    /// updates to this entity.
    fn submit_overlay_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::dictation::close_dictation_microphone_popup_window_for_owner_loss(cx);
        let callback = OVERLAY_SUBMIT_CALLBACK.lock().take();
        *OVERLAY_ABORT_CALLBACK.lock() = None;
        remove_global_escape_monitor();
        self.dematerialize_then_remove_overlay(window, cx);

        if let Some(cb) = callback {
            cx.defer(move |cx| {
                cb(cx);
            });
        }

        tracing::info!(
            category = "DICTATION",
            "Overlay closed from within entity (submit)"
        );
    }

    /// Close the overlay window directly from within the entity.
    ///
    /// Same reentrant-borrow avoidance as `abort_overlay_session`, but
    /// without invoking the abort callback (used for non-recording phases).
    fn close_overlay_from_within(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::dictation::close_dictation_microphone_popup_window_for_owner_loss(cx);
        *OVERLAY_ABORT_CALLBACK.lock() = None;
        *OVERLAY_SUBMIT_CALLBACK.lock() = None;
        remove_global_escape_monitor();
        self.dematerialize_then_remove_overlay(window, cx);
        tracing::info!(category = "DICTATION", "Overlay closed from within entity");
    }

    /// Check whether the global key monitor flagged an Escape or Enter press
    /// and process it.  Called from the overlay pump tick (every 16ms) so the
    /// action runs inside GPUI context with full `&mut self` access.
    fn process_global_keys_if_requested(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> GlobalKeyProcessResult {
        // Enter is only meaningful in the Confirming phase (= stop & transcribe).
        let enter = ENTER_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst);
        if enter && self.state.phase == DictationSessionPhase::Confirming {
            if !self.transcript_armed() {
                tracing::debug!(
                    category = "DICTATION",
                    "Ignoring global Enter request in Confirming phase because transcript is unarmed"
                );
                return GlobalKeyProcessResult::None;
            }
            tracing::info!(
                category = "DICTATION",
                "Processing global Enter request in Confirming phase — stopping to transcribe"
            );
            self.submit_overlay_session(window, cx);
            // Clear any pending escape too — we've already acted.
            ESCAPE_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
            return GlobalKeyProcessResult::Closed;
        }

        if !ESCAPE_REQUESTED.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return GlobalKeyProcessResult::None;
        }

        match self.dismiss_microphone_popup_top_layer("global_escape", cx) {
            crate::dictation::DictationPopupDismissOutcome::ClosedCurrentTopLayer => {
                return GlobalKeyProcessResult::StateChanged;
            }
            crate::dictation::DictationPopupDismissOutcome::ReconciledStaleSlot
            | crate::dictation::DictationPopupDismissOutcome::NotPresent => {}
        }

        let elapsed = crate::dictation::dictation_elapsed().unwrap_or(self.state.elapsed);
        let action = overlay_escape_action(&self.state.phase, elapsed);

        tracing::info!(
            category = "DICTATION",
            ?action,
            phase = ?self.state.phase,
            elapsed_ms = elapsed.as_millis() as u64,
            "Processing global escape request"
        );

        match action {
            OverlayEscapeAction::TransitionToConfirming => {
                self.enter_confirming(window, cx);
                GlobalKeyProcessResult::StateChanged
            }
            OverlayEscapeAction::ResumeRecording => {
                self.resume_recording(window, cx);
                GlobalKeyProcessResult::StateChanged
            }
            OverlayEscapeAction::AbortSession => {
                self.abort_overlay_session(window, cx);
                GlobalKeyProcessResult::Closed
            }
            OverlayEscapeAction::CloseOverlay => {
                self.close_overlay_from_within(window, cx);
                GlobalKeyProcessResult::Closed
            }
            OverlayEscapeAction::Propagate => GlobalKeyProcessResult::None,
        }
    }

    /// Replace the visual state snapshot (called from the dictation runtime).
    pub fn set_state(
        &mut self,
        state: DictationOverlayState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_phase = self.state.phase.clone();
        let entering_processing =
            is_processing_phase(&state.phase) && !is_processing_phase(&self.state.phase);
        let leaving_processing =
            !is_processing_phase(&state.phase) && is_processing_phase(&self.state.phase);

        if previous_phase != state.phase {
            let max_bar = state.bars.iter().copied().fold(0.0_f32, f32::max);
            tracing::info!(
                category = "DICTATION",
                from_phase = ?previous_phase,
                to_phase = ?state.phase,
                elapsed_ms = state.elapsed.as_millis() as u64,
                transcript_len = state.transcript.len(),
                target = ?state.target,
                max_bar,
                "Dictation overlay state phase changed"
            );
        }

        // Smoothly animate bars during recording; snap for other phases.
        self.display_bars =
            if state.phase == DictationSessionPhase::Recording && !self.reduced_motion {
                crate::dictation::visualizer::animate_bars(
                    self.display_bars,
                    state.bars,
                    Duration::from_millis(16),
                )
            } else {
                state.bars
            };

        // Feed the caption reveal: recording/confirming stream words in on
        // the paced clock (the 16 ms pump drives ticks); processing and
        // terminal phases show everything they have immediately so the user
        // sees the full text the app is working on.
        match state.phase {
            DictationSessionPhase::Recording | DictationSessionPhase::Confirming => {
                self.caption.set_target(state.transcript.as_ref());
                if self.reduced_motion {
                    self.caption.reveal_all();
                } else {
                    self.caption.tick(Instant::now());
                }
            }
            DictationSessionPhase::Transcribing
            | DictationSessionPhase::Delivering
            | DictationSessionPhase::Finished
            | DictationSessionPhase::Failed(_) => {
                if !state.transcript.is_empty() {
                    self.caption.set_target(state.transcript.as_ref());
                }
                self.caption.reveal_all();
            }
            DictationSessionPhase::Idle => {}
        }

        // Grow the window toward the screen center (see
        // `resize_overlay_height`) when the accumulated transcript needs more
        // wrapped lines. Sized from the full committed text — not just the
        // revealed prefix — so space is ready before the paced reveal reaches
        // it, and never shrunk mid-session. At the growth cap the caption
        // block scrolls instead.
        if !matches!(state.phase, DictationSessionPhase::Idle) {
            let target_lines =
                estimate_caption_lines(&self.caption.target_text(), TRANSCRIPT_PREVIEW_MAX_CHARS);
            let extra = target_lines
                .saturating_sub(1)
                .min(OVERLAY_MAX_EXTRA_CAPTION_LINES);
            if extra > self.extra_caption_lines {
                self.extra_caption_lines = extra;
                resize_overlay_height(
                    window,
                    OVERLAY_HEIGHT_PX + extra as f32 * TRANSCRIPT_LINE_HEIGHT_PX,
                );
            }
        }

        self.state = state;

        if entering_processing && !self.reduced_motion {
            self.processing_started_at = Some(Instant::now());
            // Spawn a tick loop that re-renders every TRANSCRIBING_TICK_MS so
            // the dot stagger and caption pulse progress smoothly.
            self._animation_task = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(TRANSCRIBING_TICK_MS))
                    .await;
                let should_stop = this
                    .update(cx, |view, cx| {
                        if !is_processing_phase(&view.state.phase) {
                            return true;
                        }
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if should_stop {
                    break;
                }
            }));
        } else if leaving_processing {
            self.processing_started_at = None;
            self._animation_task = None;
        }

        cx.notify();
    }

    /// Render the right-side target badge as a pure destination indicator.
    ///
    /// Destination *selection* happens through the explicit verb chip row
    /// ([`Self::render_destination_chip_row`]) — the badge never cycles on
    /// click, so the current target is always visible and never hidden state
    /// discovered by clicking through it.
    fn render_target_badge_slot(&self, dimmed: bool) -> impl IntoElement {
        let theme = get_cached_theme();

        let target_label = target_badge_label(self.state.target);
        let badge_content = render_target_badge_content(self.state.target);

        let mut badge = div()
            .id("dictation-target-badge")
            .px(px(8.))
            .py(px(2.))
            .rounded(px(999.))
            .bg(theme.colors.background.main.with_opacity(OPACITY_SUBTLE))
            .border_1()
            .border_color(theme.colors.ui.border.with_opacity(OPACITY_SUBTLE))
            .cursor_default()
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(target_label.clone()).build(window, cx)
            })
            .child(badge_content);
        if dimmed {
            // Processing: the destination is locked in — keep it visible but
            // clearly inert.
            badge = badge.opacity(0.55);
        }

        div()
            .w(px(TARGET_BADGE_SLOT_WIDTH_PX))
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .child(badge)
    }

    /// Render the destination verb chips: Paste · Today · Ask · Send.
    ///
    /// Chip clicks follow [`chip_click_behavior`]: with a recognized
    /// transcript ("armed") a click stops the session and delivers there in
    /// one gesture; unarmed — or with Option held — a click only retargets
    /// the session (and becomes the new sticky default) while recording
    /// continues. The active chip stays highlighted so the destination is
    /// always visible, and armed send-mode chips carry a trailing ↵ glyph.
    /// Targets outside the chip set (Notes, Prompt, Filter) highlight no
    /// chip — the badge still tells the truth.
    ///
    /// While the app is processing the capture the chips stay in place but
    /// go inert (`interactive: false`) so the layout never jumps.
    fn render_destination_chip_row(
        &self,
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(6.));
        let mut chips = Vec::new();

        let theme = get_cached_theme();
        let hover_bg = theme.colors.background.main.with_opacity(OPACITY_SELECTED);
        let armed = self.transcript_armed();
        let option_held = OPTION_HELD.load(Ordering::SeqCst);
        let mode = destination_chip_mode(&self.state.phase, armed, option_held);
        let send_mode = matches!(mode, DestinationChipMode::Send);
        for (target, verb, icon) in DICTATION_CHIP_TARGETS {
            let is_active = self.state.target == target;
            let tooltip_label = chip_tooltip_label(target, mode);
            let mut chip = destination_chip_base(verb, icon, is_active, !interactive, send_mode);
            if interactive {
                if send_mode || !is_active {
                    chip = chip.hover(move |style| style.bg(hover_bg));
                }
                chip = chip
                    .cursor_pointer()
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(tooltip_label.clone())
                            .build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                            match chip_click_behavior(
                                &this.state.phase,
                                this.transcript_armed(),
                                OPTION_HELD.load(Ordering::SeqCst),
                            ) {
                                ChipClickBehavior::Ignore => {}
                                ChipClickBehavior::Retarget => {
                                    this.select_destination(target, cx);
                                }
                                ChipClickBehavior::SendTo => {
                                    this.select_destination(target, cx);
                                    this.submit_overlay_session(window, cx);
                                }
                            }
                        }),
                    );
            }
            chips.push(chip.into_any_element());
        }

        if interactive {
            crate::components::footer_chrome::glass_capsule_row(
                "dictation-header-targets",
                0,
                Some(999.0),
                row,
                chips,
            )
        } else {
            let chip_count = chips.len();
            crate::components::footer_chrome::glass_capsule_row(
                "dictation-header-targets",
                chip_count,
                Some(999.0),
                row.children(chips),
                std::iter::empty(),
            )
        }
    }

    /// Apply an explicit destination pick from a verb chip: retarget the live
    /// session and persist it as the sticky default for the next one.
    fn select_destination(
        &mut self,
        target: crate::dictation::DictationTarget,
        cx: &mut Context<Self>,
    ) {
        if !matches!(
            self.state.phase,
            DictationSessionPhase::Recording | DictationSessionPhase::Confirming
        ) {
            return;
        }

        let Some(applied) = crate::dictation::set_dictation_session_target(target) else {
            return;
        };
        self.state.target = applied;

        if let Err(error) = crate::dictation::save_dictation_last_target(applied) {
            tracing::warn!(
                category = "DICTATION",
                error = %error,
                target_label = applied.overlay_label(),
                "Failed to persist sticky dictation destination"
            );
        }

        cx.notify();
    }

    /// Render the header row: timer (left), destination chips (center),
    /// target badge (right).
    ///
    /// Present in every non-idle phase so the overlay anatomy never jumps.
    /// During processing the timer locks at the final duration and the whole
    /// row grays out — the work status shows in the caption band instead.
    fn render_header_row(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = get_cached_theme();
        let interactive = matches!(
            self.state.phase,
            DictationSessionPhase::Recording | DictationSessionPhase::Confirming
        );
        let timer_color = if interactive {
            theme.colors.text.primary.with_opacity(OPACITY_ACTIVE)
        } else {
            theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED)
        };
        let timer_text = format_elapsed(self.state.elapsed);

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(OVERLAY_HEADER_ROW_HEIGHT_PX))
            // Left: timer, same slot width as the badge so the chips center.
            .child(
                div()
                    .w(px(TARGET_BADGE_SLOT_WIDTH_PX))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(STATUS_TEXT_SIZE_PX))
                            .font_family(FONT_SYSTEM_UI)
                            .text_color(timer_color)
                            .child(timer_text),
                    ),
            )
            // Center: destination verb chips.
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .child(self.render_destination_chip_row(interactive, cx)),
            )
            // Right: destination badge (indicator only).
            .child(self.render_target_badge_slot(!interactive))
            .into_any_element()
    }

    /// Render the caption band while the app processes the capture.
    ///
    /// While no text is recognized yet the status label ("Transcribing…",
    /// "Delivering…") plus the staggered dot pulse fill the band. Once text
    /// exists the caption alone pulses gently — the pulse IS the working
    /// indicator, so no status label competes with the transcript for the
    /// band's width and the text stays inside the side padding.
    fn render_processing_band(&self, status: &'static str) -> gpui::AnyElement {
        let theme = get_cached_theme();
        let muted_text = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);

        let caption_text = self.caption.visible_text();
        if caption_text.trim().is_empty() {
            // Nothing recognized yet: status label + staggered dot pulse.
            let dot_opacities = if self.reduced_motion {
                transcribing_dot_opacities_static()
            } else if let Some(started) = self.processing_started_at {
                transcribing_dot_opacities_at(started.elapsed().as_secs_f64())
            } else {
                transcribing_dot_opacities_static()
            };
            return div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(STATUS_TEXT_SIZE_PX))
                        .font_family(FONT_SYSTEM_UI)
                        .text_color(muted_text)
                        .whitespace_nowrap()
                        .child(status),
                )
                .child(render_transcribing_dots(&dot_opacities))
                .into_any_element();
        }

        let pulse = if self.reduced_motion {
            processing_pulse_opacity_static()
        } else if let Some(started) = self.processing_started_at {
            processing_pulse_opacity_at(started.elapsed().as_secs_f64())
        } else {
            processing_pulse_opacity_static()
        };

        // The text being worked on keeps its place and pulses; underneath, a
        // status label plus a real progress bar (fed by the chunked finalize
        // pass) says exactly what is happening and how far along it is.
        div()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .gap(px(4.))
            .child(
                self.caption_scroll_container()
                    .text_color(theme.colors.text.primary.with_opacity(OPACITY_ACTIVE))
                    .child(caption_text.trim().to_string())
                    .opacity(pulse),
            )
            .child(self.render_processing_status_row(status, muted_text))
            .into_any_element()
    }

    /// Status label + finalize progress bar shown under the pulsing caption
    /// while the app processes the capture.
    fn render_processing_status_row(
        &self,
        status: &'static str,
        muted_text: gpui::Hsla,
    ) -> gpui::AnyElement {
        let theme = get_cached_theme();
        let progress = crate::dictation::finalize_progress();

        let mut row = div()
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(6.))
            .pb(px(2.))
            .child(
                div()
                    .text_size(px(STATUS_TEXT_SIZE_PX))
                    .font_family(FONT_SYSTEM_UI)
                    .text_color(muted_text)
                    .whitespace_nowrap()
                    .child(status),
            );

        if let Some(fraction) = progress {
            let track_color = theme.colors.ui.border.with_opacity(OPACITY_SUBTLE);
            let fill_color = theme.colors.accent.selected.with_opacity(OPACITY_ACTIVE);
            row = row.child(
                div()
                    .flex_1()
                    .h(px(3.))
                    .rounded(px(999.))
                    .bg(track_color)
                    .child(
                        div()
                            .h_full()
                            .rounded(px(999.))
                            .bg(fill_color)
                            .w(relative(fraction.clamp(0.02, 1.0))),
                    ),
            );
        }

        row.into_any_element()
    }

    fn handle_parent_mouse_down(
        &mut self,
        _event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dismiss_microphone_popup_top_layer("parent_mouse_down", cx)
            == crate::dictation::DictationPopupDismissOutcome::ClosedCurrentTopLayer
        {
            cx.stop_propagation();
        }
    }

    /// Handle key-down events for the overlay.
    ///
    /// Escape semantics (vercel-voice 5-second threshold pattern):
    /// - Recording + Enter → stop and transcribe
    /// - Recording (< 5 s) + Escape → immediate abort
    /// - Recording (≥ 5 s) + Escape → transition to Confirming
    /// - Confirming + Enter → stop and transcribe (the good path)
    /// - Confirming + Backspace/Delete → discard the recording
    /// - Confirming + Escape → resume Recording
    /// - Confirming + any other key → swallowed (no state change)
    /// - Transcribing / Delivering / Finished / Failed → dismiss overlay only
    /// - Idle → propagate (no-op)
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        tracing::debug!(
            category = "DICTATION",
            key,
            phase = ?self.state.phase,
            "Overlay received key_down"
        );

        if crate::ui_foundation::is_key_escape(key) {
            match self.dismiss_microphone_popup_top_layer("parent_escape", cx) {
                crate::dictation::DictationPopupDismissOutcome::ClosedCurrentTopLayer => {
                    cx.stop_propagation();
                    return;
                }
                crate::dictation::DictationPopupDismissOutcome::ReconciledStaleSlot
                | crate::dictation::DictationPopupDismissOutcome::NotPresent => {}
            }
        }

        // Enter during Recording stops the session and transcribes, matching
        // the footer's Stop affordance.
        if self.state.phase == DictationSessionPhase::Recording
            && crate::ui_foundation::is_key_enter(key)
        {
            if !self.transcript_armed() {
                tracing::debug!(
                    category = "DICTATION",
                    "Enter pressed while recording with unarmed transcript; swallowing"
                );
                cx.stop_propagation();
                return;
            }
            tracing::info!(
                category = "DICTATION",
                "Enter pressed while recording, stopping to transcribe"
            );
            self.submit_overlay_session(window, cx);
            cx.stop_propagation();
            return;
        }

        // In Confirming state, only Enter, Backspace/Delete, and Escape have
        // meaning. All other keys are swallowed without changing state.
        if self.state.phase == DictationSessionPhase::Confirming {
            if crate::ui_foundation::is_key_enter(key) {
                if !self.transcript_armed() {
                    tracing::debug!(
                        category = "DICTATION",
                        "Enter pressed during confirmation with unarmed transcript; swallowing"
                    );
                    cx.stop_propagation();
                    return;
                }
                tracing::info!(
                    category = "DICTATION",
                    "Enter pressed during confirmation, stopping to transcribe"
                );
                self.submit_overlay_session(window, cx);
                cx.stop_propagation();
                return;
            }
            if key == "backspace" || key == "delete" {
                tracing::info!(
                    category = "DICTATION",
                    "Backspace pressed during confirmation, discarding dictation session"
                );
                self.abort_overlay_session(window, cx);
                cx.stop_propagation();
                return;
            }
            if !crate::ui_foundation::is_key_escape(key) {
                // Swallow unrelated keys — no resume, no propagation.
                cx.stop_propagation();
                return;
            }
        }

        if !crate::ui_foundation::is_key_escape(key) {
            cx.propagate();
            return;
        }

        // Use the authoritative runtime elapsed time for threshold decisions,
        // falling back to the pump-snapshot elapsed when no session is active.
        let elapsed = crate::dictation::dictation_elapsed().unwrap_or(self.state.elapsed);

        match overlay_escape_action(&self.state.phase, elapsed) {
            OverlayEscapeAction::TransitionToConfirming => {
                tracing::info!(
                    category = "DICTATION",
                    elapsed_ms = elapsed.as_millis() as u64,
                    "Escape pressed after threshold, showing confirmation"
                );
                self.enter_confirming(window, cx);
                cx.stop_propagation();
            }
            OverlayEscapeAction::ResumeRecording => {
                tracing::info!(
                    category = "DICTATION",
                    elapsed_ms = elapsed.as_millis() as u64,
                    "Escape pressed in confirmation, resuming recording"
                );
                self.resume_recording(window, cx);
                cx.stop_propagation();
            }
            OverlayEscapeAction::AbortSession => {
                tracing::info!(
                    category = "DICTATION",
                    elapsed_ms = elapsed.as_millis() as u64,
                    "Escape pressed before threshold, aborting dictation session"
                );
                self.abort_overlay_session(window, cx);
                cx.stop_propagation();
            }
            OverlayEscapeAction::CloseOverlay => {
                tracing::info!(
                    category = "DICTATION",
                    phase = ?self.state.phase,
                    "Escape pressed, dismissing dictation overlay"
                );
                self.close_overlay_from_within(window, cx);
                cx.stop_propagation();
            }
            OverlayEscapeAction::Propagate => {
                cx.propagate();
            }
        }
    }
}

impl Focusable for DictationOverlay {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DictationOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_caption_follow();
        self.ensure_native_footer_action_listener(window, cx);
        let armed = self.transcript_armed();
        let footer_config = dictation_native_footer_config(&self.state.phase, armed);
        let glass_in_window_footer = crate::platform::tahoe_liquid_glass_available()
            && crate::theme::get_cached_theme().is_vibrancy_enabled();
        let glass_buttons_enabled = crate::components::footer_chrome::glass_capsules_enabled();
        if !glass_in_window_footer {
            crate::footer_popup::sync_window_footer_popup(window, &footer_config);
        }

        let theme = get_cached_theme();
        if self.last_render_logged_phase.as_ref() != Some(&self.state.phase) {
            tracing::info!(
                category = "DICTATION",
                phase = ?self.state.phase,
                elapsed_ms = self.state.elapsed.as_millis() as u64,
                target = ?self.state.target,
                reduced_motion = self.reduced_motion,
                "Rendering dictation overlay phase"
            );
            self.last_render_logged_phase = Some(self.state.phase.clone());
        }
        let window_background = crate::ui_foundation::main_window_matched_background(&theme);
        let theme_background_gradients =
            crate::ui_foundation::theme_background_gradient_layers("dictation-bg-layer", &theme);
        let border_color = rgba((theme.colors.ui.border << 8) | 0x40);

        let muted_text = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);
        let text_color = theme.colors.text.primary.with_opacity(OPACITY_ACTIVE);

        let phase = self.state.phase.clone();
        let action_rail_present = !matches!(phase, DictationSessionPhase::Idle);
        if !glass_buttons_enabled || !action_rail_present {
            self.hide_glass_button_frames(window);
        }
        let bars = &self.display_bars;

        // Every non-idle phase shares one anatomy — header row (timer, chips,
        // badge), caption band, native footer rail — so phase changes swap
        // content inline and nothing jumps or disappears.
        let center: AnyElement = match &phase {
            DictationSessionPhase::Recording => {
                let active = has_sound(bars);
                // Live caption replaces the waveform once words arrive; the
                // timer keeps ticking as the level cue.
                if self.caption.is_empty() {
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .child(render_waveform_bars(bars, active))
                        .into_any_element()
                } else {
                    self.caption_scroll_container()
                        .child(live_caption_text(&self.caption, self.reduced_motion))
                        .into_any_element()
                }
            }
            DictationSessionPhase::Confirming => {
                // Keep the recognized text in place (muted) under the
                // confirmation headline so pausing never blanks the block.
                let caption_text = self.caption.visible_text();
                let mut column = div()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .overflow_hidden();
                if !caption_text.trim().is_empty() {
                    column = column.child(
                        self.caption_scroll_container()
                            .text_color(muted_text)
                            .child(caption_text.trim().to_string()),
                    );
                }
                let (headline, _) = overlay_phase_copy(&phase);
                column
                    .child(
                        div()
                            .w_full()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .justify_center()
                            .pb(px(2.))
                            .text_size(px(TRANSCRIPT_TEXT_SIZE_PX))
                            .font_family(FONT_SYSTEM_UI)
                            .text_color(text_color)
                            .child(headline),
                    )
                    .into_any_element()
            }
            DictationSessionPhase::Transcribing | DictationSessionPhase::Delivering => {
                let (status, _) = overlay_phase_copy(&phase);
                self.render_processing_band(status)
            }
            DictationSessionPhase::Finished => {
                let caption_text = self.caption.visible_text();
                let mut column = div()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .overflow_hidden();
                if !caption_text.trim().is_empty() {
                    column = column.child(
                        self.caption_scroll_container()
                            .text_color(muted_text)
                            .child(caption_text.trim().to_string()),
                    );
                }
                column
                    .child(
                        div()
                            .w_full()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .justify_center()
                            .pb(px(2.))
                            .text_size(px(STATUS_TEXT_SIZE_PX))
                            .font_family(FONT_SYSTEM_UI)
                            .text_color(text_color)
                            .child(finished_label()),
                    )
                    .into_any_element()
            }
            DictationSessionPhase::Failed(msg) => {
                let err_text: SharedString = format!("Error: {msg}").into();
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .text_size(px(STATUS_TEXT_SIZE_PX))
                    .font_family(FONT_SYSTEM_UI)
                    .text_color(muted_text)
                    .overflow_hidden()
                    .child(err_text)
                    .into_any_element()
            }
            DictationSessionPhase::Idle => div().into_any_element(),
        };

        let footer_rail = if glass_in_window_footer {
            render_static_action_rail(footer_config.buttons, Some(cx)).into_any_element()
        } else {
            native_footer_spacer().into_any_element()
        };

        let main_inner = if matches!(phase, DictationSessionPhase::Idle) {
            div()
        } else {
            div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(
                    div()
                        .w_full()
                        .px(px(OVERLAY_HORIZONTAL_PADDING_PX))
                        .pt(px(5.0))
                        .child(self.render_header_row(cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .child(render_glass_signal_band(center)),
                )
        };

        // Same capsule radius for all phases; confirming swaps content inline.
        let radius = OVERLAY_RADIUS_PX;
        let window_size = window.bounds().size;
        let detached_regions = crate::footer_popup::main_window_detached_footer_regions_gpui(
            f32::from(window_size.width),
            f32::from(window_size.height),
            if glass_in_window_footer {
                crate::components::footer_chrome::current_main_menu_footer_height()
            } else {
                0.0
            },
            if glass_in_window_footer {
                crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
            } else {
                0.0
            },
            window.scale_factor(),
        );

        // Capsule chrome only; the signal band uses the main-menu selected-row fill
        // and the action rail carries the launcher window surface underneath.
        let surface = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .w_full()
            .when(glass_in_window_footer, |d| {
                d.h(px(detached_regions.main_content.height))
                    .min_h(px(detached_regions.main_content.height))
            })
            .when(!glass_in_window_footer, |d| d.h_full())
            .relative()
            .overflow_hidden()
            .rounded(px(radius))
            .bg(window_background)
            .children(theme_background_gradients)
            .border_1()
            .border_color(border_color);

        let composition = if glass_in_window_footer {
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .child(surface.child(main_inner))
                .child(
                    div()
                        .w_full()
                        .h(px(detached_regions.transparent_gap.height))
                        .flex_none(),
                )
                .child(footer_rail)
                .into_any_element()
        } else {
            surface
                .child(main_inner)
                .child(footer_rail)
                .into_any_element()
        };

        // The physical window is one atomic composition host. In glass mode
        // its bounded content capsule ends above an 8pt transparent desktop
        // gutter; the shared per-button glass footer remains in the same
        // NSWindow below it, so no follower window can lag during a drag.
        div()
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(Self::handle_parent_mouse_down),
            )
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_modifiers_changed(cx.listener(
                |_, event: &gpui::ModifiersChangedEvent, _window, cx| {
                    OPTION_HELD.store(event.modifiers.alt, Ordering::SeqCst);
                    cx.notify();
                },
            ))
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(composition)
    }
}

/// Format the finished overlay state label.
pub(crate) fn finished_label() -> SharedString {
    "Done".into()
}

/// Approximate characters per wrapped caption line at
/// [`TRANSCRIPT_TEXT_SIZE_PX`]: ~(560 - padding) / ~7 px per char. Drives
/// the window-growth line estimate ([`estimate_caption_lines`]).
pub(crate) const TRANSCRIPT_PREVIEW_MAX_CHARS: usize = 74;

/// Live-transcription marker rendered after the newest word while recording
/// (the Super Whisper "current word" cue).
pub(crate) const LIVE_CAPTION_MARKER: &str = " \u{00B7}\u{00B7}\u{00B7}";

/// Build the recording-time caption text as styled wrapped runs.
///
/// The full committed transcript wraps naturally at the pill width. Only the
/// newest revealed word fades in — everything before it renders at full
/// opacity and never shifts, because new words append at the wrap point
/// instead of pushing the line sideways. A muted dot marker trails the
/// newest word while the session is live.
fn live_caption_text(
    caption: &crate::dictation::live_caption::LiveCaption,
    reduced_motion: bool,
) -> AnyElement {
    let theme = get_cached_theme();
    let base_color = theme.colors.text.primary.with_opacity(OPACITY_ACTIVE);
    let marker_color = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);

    let visible = caption.visible_text();
    if visible.is_empty() {
        return div().into_any_element();
    }

    // Fade computed at render time: the overlay pump re-renders every 16 ms
    // while recording, so the newest word's alpha eases in without a
    // separate animation element (which cannot restyle individual runs).
    let fade = if reduced_motion {
        1.0
    } else {
        caption
            .last_reveal_at()
            .map(|at| {
                (at.elapsed().as_millis() as f32 / TRANSCRIPT_FADE_IN_MS as f32).clamp(0.0, 1.0)
            })
            .unwrap_or(1.0)
    };

    let fresh_chars = caption.fresh_char_offset();
    let fresh_byte = visible
        .char_indices()
        .nth(fresh_chars)
        .map(|(ix, _)| ix)
        .unwrap_or(visible.len());

    let font = gpui::font(FONT_SYSTEM_UI);
    let mut text = visible;
    let mut runs: Vec<TextRun> = Vec::with_capacity(3);
    let run = |len: usize, color| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    if fresh_byte > 0 {
        runs.push(run(fresh_byte, base_color));
    }
    if text.len() > fresh_byte {
        runs.push(run(text.len() - fresh_byte, base_color.opacity(fade)));
    }
    text.push_str(LIVE_CAPTION_MARKER);
    runs.push(run(LIVE_CAPTION_MARKER.len(), marker_color));

    StyledText::new(text).with_runs(runs).into_any_element()
}

/// Shared container for the wrapped caption block: full width, bottom
/// anchored so the newest line stays visible when the text outgrows the
/// window, clipped above.
fn render_caption_block_container() -> Div {
    div()
        .flex_1()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .justify_end()
        .overflow_hidden()
        .px(px(6.))
        .text_size(px(TRANSCRIPT_TEXT_SIZE_PX))
        .font_family(FONT_SYSTEM_UI)
        .line_height(px(TRANSCRIPT_LINE_HEIGHT_PX))
}

/// Render a static transcript as a wrapped bottom-anchored block (processing
/// and terminal phases — same geometry as the live block, no marker).
fn render_transcript_block(transcript: &str, muted: bool) -> Div {
    let theme = get_cached_theme();
    let color = if muted {
        theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED)
    } else {
        theme.colors.text.primary.with_opacity(OPACITY_ACTIVE)
    };
    render_caption_block_container()
        .text_color(color)
        .child(transcript.trim().to_string())
}

/// Render waveform bars for the glass bar.
///
/// Uses theme success color when sound is detected, muted text when silent.
fn render_waveform_bars(bars: &[f32; WAVEFORM_BAR_COUNT], active: bool) -> impl IntoElement {
    let theme = get_cached_theme();
    let bar_hex = if active {
        theme.colors.ui.success
    } else {
        theme.colors.text.primary
    };
    let inactive_opacity_scale = if active {
        1.0
    } else {
        theme.get_opacity().text_hint
    };

    let mut container = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(WAVEFORM_BAR_GAP_PX))
        .h(px(WAVEFORM_BAR_MAX_HEIGHT_PX));

    for &level in bars {
        let bar_color = bar_hex.with_opacity(waveform_bar_opacity(level) * inactive_opacity_scale);
        container = container.child(
            div()
                .w(px(WAVEFORM_BAR_WIDTH_PX))
                .h(px(waveform_bar_height(level)))
                .min_h(px(WAVEFORM_BAR_MIN_HEIGHT_PX))
                .bg(bar_color)
                .rounded(px(WAVEFORM_BAR_WIDTH_PX)),
        );
    }

    container
}

/// Render dots for the transcribing state.
///
/// Uses theme success color at the given per-dot opacities.
/// When animated, opacities come from `transcribing_dot_opacities_at()`;
/// under reduced-motion, from `transcribing_dot_opacities_static()`.
fn render_transcribing_dots(opacities: &[f32; TRANSCRIBING_DOT_COUNT]) -> impl IntoElement {
    let theme = get_cached_theme();

    let mut container = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(TRANSCRIBING_DOT_GAP_PX))
        .h(px(WAVEFORM_BAR_MAX_HEIGHT_PX));

    for &opacity in opacities {
        let dot_color = theme.colors.ui.success.with_opacity(opacity);
        container = container.child(
            div()
                .w(px(TRANSCRIBING_DOT_SIZE_PX))
                .h(px(TRANSCRIBING_DOT_SIZE_PX))
                .rounded(px(TRANSCRIBING_DOT_SIZE_PX / 2.0))
                .bg(dot_color),
        );
    }

    container
}

fn dictation_stop_keycap() -> SharedString {
    crate::config::load_config()
        .get_dictation_hotkey()
        .map(|hotkey| dictation_hotkey_keycap(&hotkey))
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| "click".to_string())
        .into()
}

fn dictation_hotkey_keycap(hotkey: &crate::config::HotkeyConfig) -> String {
    hotkey.to_display_string().replace("Semicolon", ";")
}

fn active_microphone_footer_label() -> SharedString {
    // A mic picked mid-recording only applies to the next session — show it
    // as pending instead of pretending the live capture switched.
    if let Some(pending) = crate::dictation::pending_dictation_device_label()
        .map(|label| crate::dictation::microphone_display_label(&label))
        .filter(|label| !label.trim().is_empty())
    {
        return format!("{pending} (next)").into();
    }
    crate::dictation::get_active_dictation_device()
        .map(|device| crate::dictation::microphone_display_label(&device.name))
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| ACTION_MIC_LABEL.to_string())
        .into()
}

pub(crate) fn dictation_native_footer_config(
    phase: &DictationSessionPhase,
    armed: bool,
) -> crate::footer_popup::MainWindowFooterConfig {
    use crate::footer_popup::{FooterAction, FooterButtonConfig, MainWindowFooterConfig};

    let buttons = match phase {
        DictationSessionPhase::Recording => {
            let mut buttons = vec![FooterButtonConfig::new(
                FooterAction::Ai,
                MIC_KEYCAP,
                active_microphone_footer_label(),
            )];
            if armed {
                buttons.push(FooterButtonConfig::new(
                    FooterAction::Stop,
                    dictation_stop_keycap(),
                    ACTION_STOP_LABEL,
                ));
            }
            buttons.push(FooterButtonConfig::new(
                FooterAction::Close,
                ESC_KEYCAP,
                ACTION_CANCEL_LABEL,
            ));
            buttons
        }
        DictationSessionPhase::Confirming => {
            let mut buttons = Vec::new();
            if armed {
                buttons.push(FooterButtonConfig::new(
                    FooterAction::Stop,
                    ENTER_KEYCAP,
                    ACTION_STOP_LABEL,
                ));
            }
            buttons.push(FooterButtonConfig::new(
                FooterAction::Actions,
                BACKSPACE_KEYCAP,
                ACTION_DISCARD_LABEL,
            ));
            buttons.push(FooterButtonConfig::new(
                FooterAction::Close,
                ESC_KEYCAP,
                ACTION_CONTINUE_LABEL,
            ));
            buttons
        }
        DictationSessionPhase::Idle => Vec::new(),
        DictationSessionPhase::Transcribing
        | DictationSessionPhase::Delivering
        | DictationSessionPhase::Finished
        | DictationSessionPhase::Failed(_) => {
            vec![FooterButtonConfig::new(
                FooterAction::Close,
                ESC_KEYCAP,
                ACTION_CLOSE_LABEL,
            )]
        }
    };

    MainWindowFooterConfig::new(DICTATION_OVERLAY_FOOTER_SURFACE, buttons)
}

fn native_footer_spacer() -> impl IntoElement {
    let rail_chrome = crate::components::footer_chrome::footer_rail_chrome(&get_cached_theme());

    div()
        .id("dictation-action-rail")
        .w_full()
        .h(px(rail_chrome.height_px))
        .min_h(px(rail_chrome.height_px))
}

fn render_glass_signal_band(body: AnyElement) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(OVERLAY_HORIZONTAL_PADDING_PX))
        .py(px(6.0))
        .child(body)
}

fn wrap_dictation_overlay_action_rail(rail: impl IntoElement) -> impl IntoElement {
    div().w_full().child(rail)
}

fn render_static_action_rail(
    buttons: impl IntoIterator<Item = crate::footer_popup::FooterButtonConfig>,
    mut cx: Option<&mut Context<DictationOverlay>>,
) -> AnyElement {
    use crate::components::footer_chrome::{
        footer_action_slot_width, footer_button_height, footer_rail_chrome,
        render_footer_action_rail, render_footer_hint_action_button_frame, FooterActionSlot,
        FooterHintActionButtonFrameSpec, FooterHintButtonLayoutOverrides, FooterHintContentJustify,
    };

    let theme = get_cached_theme();
    let rail_chrome = footer_rail_chrome(&theme);
    let button_height = footer_button_height(rail_chrome.height_px);
    let mut frames = Vec::new();

    for button in buttons {
        let enabled = button.enabled && button.disabled_reason.is_none();
        let key_is_icon =
            crate::components::footer_chrome::is_footer_icon_token(button.key.as_ref());
        let displayed_key = if key_is_icon || button.shortcut_routable {
            button.key.clone()
        } else {
            gpui::SharedString::from("")
        };
        let frame = render_footer_hint_action_button_frame(
            FooterHintActionButtonFrameSpec {
                id: button.id,
                label: button.label,
                key: displayed_key,
                slot_width_px: footer_action_slot_width(FooterActionSlot::Run),
                height_px: button_height,
                selected: false,
                key_first: false,
                justify: FooterHintContentJustify::Center,
                layout: FooterHintButtonLayoutOverrides {
                    shrink_frame_to_content_px: true,
                    hug_frame_to_content: true,
                    ..FooterHintButtonLayoutOverrides::default()
                },
            },
            &theme,
        );
        if enabled {
            if let Some(cx) = cx.as_mut() {
                let action = button.action;
                frames.push(
                    frame
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.handle_native_footer_action(action, window, cx);
                        }))
                        .into_any_element(),
                );
            } else {
                frames.push(frame.into_any_element());
            }
        } else {
            frames.push(frame.opacity(0.45).cursor_default().into_any_element());
        }
    }

    render_footer_action_rail("dictation-action-rail", frames)
}

/// Render the live glass bar from a fixed state for Storybook previews.
///
/// This keeps canonical Dictation stories on the same constants, waveform
/// math, phase copy, and target-badge styling as the runtime overlay without
/// opening a real floating window.
// Storybook-parity preview surface: not reached from the runtime render
// path, but kept canonical (and contract-tested) for design stories.
#[allow(dead_code)]
pub(crate) fn render_dictation_overlay_state_preview(
    state: &DictationOverlayState,
) -> gpui::AnyElement {
    let theme = get_cached_theme();
    let window_background = crate::ui_foundation::main_window_matched_background(&theme);
    let theme_background_gradients = crate::ui_foundation::theme_background_gradient_layers(
        "dictation-preview-bg-layer",
        &theme,
    );
    let border_color = rgba((theme.colors.ui.border << 8) | 0x40);
    let muted_text = theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED);
    let text_color = theme.colors.text.primary.with_opacity(OPACITY_ACTIVE);

    if matches!(state.phase, DictationSessionPhase::Idle) {
        return div()
            .w(px(OVERLAY_WIDTH_PX))
            .h(px(OVERLAY_HEIGHT_PX))
            .into_any_element();
    }

    let center: gpui::AnyElement = match &state.phase {
        DictationSessionPhase::Recording => {
            let active = has_sound(&state.bars);
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .child(render_waveform_bars(&state.bars, active))
                .into_any_element()
        }
        DictationSessionPhase::Confirming => {
            let (headline, _) = overlay_phase_copy(&state.phase);
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .text_size(px(TRANSCRIPT_TEXT_SIZE_PX))
                .font_family(FONT_SYSTEM_UI)
                .text_color(text_color)
                .child(headline)
                .into_any_element()
        }
        DictationSessionPhase::Transcribing | DictationSessionPhase::Delivering => {
            let (status, _) = overlay_phase_copy(&state.phase);
            let band = div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .overflow_hidden();
            if state.transcript.trim().is_empty() {
                band.child(
                    div()
                        .text_size(px(STATUS_TEXT_SIZE_PX))
                        .font_family(FONT_SYSTEM_UI)
                        .text_color(muted_text)
                        .whitespace_nowrap()
                        .child(status),
                )
                .child(render_transcribing_dots(
                    &transcribing_dot_opacities_static(),
                ))
                .into_any_element()
            } else {
                band.child(
                    render_transcript_block(state.transcript.as_ref(), false)
                        .min_w(px(0.))
                        .opacity(processing_pulse_opacity_static()),
                )
                .into_any_element()
            }
        }
        DictationSessionPhase::Finished => {
            let mut band = div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(STATUS_TEXT_SIZE_PX))
                        .font_family(FONT_SYSTEM_UI)
                        .text_color(text_color)
                        .whitespace_nowrap()
                        .child(finished_label()),
                );
            if !state.transcript.trim().is_empty() {
                band = band
                    .child(render_transcript_block(state.transcript.as_ref(), true).min_w(px(0.)));
            }
            band.into_any_element()
        }
        DictationSessionPhase::Failed(msg) => {
            let err_text: SharedString = format!("Error: {msg}").into();
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .text_size(px(STATUS_TEXT_SIZE_PX))
                .font_family(FONT_SYSTEM_UI)
                .text_color(muted_text)
                .overflow_hidden()
                .child(err_text)
                .into_any_element()
        }
        DictationSessionPhase::Idle => div().into_any_element(),
    };

    let armed = !state.transcript.trim().is_empty();
    let footer_config = dictation_native_footer_config(&state.phase, armed);
    let action_rail = render_static_action_rail(footer_config.buttons, None);

    let inner = div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .child(
            div()
                .w_full()
                .px(px(OVERLAY_HORIZONTAL_PADDING_PX))
                .pt(px(5.0))
                .child(render_static_header_row(state)),
        )
        .child(
            div()
                .flex_1()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .child(render_glass_signal_band(center)),
        )
        .child(wrap_dictation_overlay_action_rail(action_rail));

    div()
        .w(px(OVERLAY_WIDTH_PX))
        .h(px(OVERLAY_HEIGHT_PX))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .relative()
        .overflow_hidden()
        .rounded(px(OVERLAY_RADIUS_PX))
        .bg(window_background)
        .children(theme_background_gradients)
        .border_1()
        .border_color(border_color)
        .child(inner)
        .into_any_element()
}

/// Static header row for Storybook previews: timer, chips, badge — same
/// anatomy as the runtime header, without click handlers.
#[allow(dead_code)] // preview-chain helper (see render_dictation_overlay_state_preview)
fn render_static_header_row(state: &DictationOverlayState) -> impl IntoElement {
    let theme = get_cached_theme();
    let live = matches!(
        state.phase,
        DictationSessionPhase::Recording | DictationSessionPhase::Confirming
    );
    let timer_color = if live {
        theme.colors.text.primary.with_opacity(OPACITY_ACTIVE)
    } else {
        theme.colors.text.muted.with_opacity(OPACITY_TEXT_MUTED)
    };

    let mut chip_row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.));
    for (target, verb, icon) in DICTATION_CHIP_TARGETS {
        chip_row = chip_row.child(destination_chip_base(
            verb,
            icon,
            state.target == target,
            !live,
            false,
        ));
    }

    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(OVERLAY_HEADER_ROW_HEIGHT_PX))
        .child(
            div()
                .w(px(TARGET_BADGE_SLOT_WIDTH_PX))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(STATUS_TEXT_SIZE_PX))
                        .font_family(FONT_SYSTEM_UI)
                        .text_color(timer_color)
                        .child(format_elapsed(state.elapsed)),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .child(chip_row),
        )
        .child(render_static_target_badge_slot(state.target, !live))
}

#[allow(dead_code)] // preview-chain helper (see render_dictation_overlay_state_preview)
fn render_static_target_badge_slot(
    target: crate::dictation::DictationTarget,
    dimmed: bool,
) -> impl IntoElement {
    let theme = get_cached_theme();
    let mut badge = div()
        .id("dictation-target-badge")
        .px(px(8.))
        .py(px(2.))
        .rounded(px(999.))
        .bg(theme.colors.background.main.with_opacity(OPACITY_SUBTLE))
        .border_1()
        .border_color(theme.colors.ui.border.with_opacity(OPACITY_SUBTLE))
        .cursor_default()
        .child(render_target_badge_content(target));
    if dimmed {
        badge = badge.opacity(0.55);
    }

    div()
        .w(px(TARGET_BADGE_SLOT_WIDTH_PX))
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .child(badge)
}

// ---------------------------------------------------------------------------
// Public window lifecycle API
// ---------------------------------------------------------------------------

/// Calculate bottom-center bounds for the overlay on the active display.
///
/// Resolves the active display containing the mouse cursor, matching the main window
/// positioning logic.
/// Falls back to the first visible display, then to a hardcoded 1920×1080 default.
/// Positions the pill centered horizontally and `OVERLAY_BOTTOM_OFFSET_PX` above
/// the bottom edge of the chosen display's visible area.
fn calculate_overlay_bottom_center_bounds() -> gpui::Bounds<gpui::Pixels> {
    // Align with main window positioning:
    // Prefer the display containing the mouse cursor.
    // Fall back to the first visible display (primary) if unavailable.
    let displays = crate::platform::get_macos_visible_displays();
    let display_count = displays.len();
    let active_display = crate::platform::get_global_mouse_position()
        .and_then(|mouse_pt| crate::platform::display_for_point(mouse_pt, &displays))
        .or_else(|| displays.into_iter().next());

    let used_display = active_display.is_some();
    let (vis_x, vis_y, vis_w, vis_h) = if let Some(display) = active_display {
        let v = &display.visible_area;
        (
            v.origin_x as f32,
            v.origin_y as f32,
            v.width as f32,
            v.height as f32,
        )
    } else {
        // Fallback: assume 1920x1080 with 24px menu bar.
        (0.0_f32, 24.0_f32, 1920.0_f32, 1056.0_f32)
    };

    // One constant height for every phase — the window never resizes.
    let initial_height = OVERLAY_HEIGHT_PX;
    let x = vis_x + (vis_w - OVERLAY_WIDTH_PX) / 2.0;
    let y = vis_y + vis_h - OVERLAY_BOTTOM_OFFSET_PX - initial_height;

    tracing::info!(
        category = "DICTATION",
        x,
        y,
        width = OVERLAY_WIDTH_PX,
        height = initial_height,
        display_count,
        used_display,
        vis_x,
        vis_y,
        vis_w,
        vis_h,
        "Calculated overlay bottom-center position"
    );

    gpui::Bounds {
        origin: gpui::Point { x: px(x), y: px(y) },
        size: gpui::Size {
            width: px(OVERLAY_WIDTH_PX),
            height: px(initial_height),
        },
    }
}

/// Configure the native window surface for the dictation overlay:
/// clear background, corner radius mask, and masksToBounds to prevent
/// white sliver artifacts at the pill's rounded corners.
#[cfg(target_os = "macos")]
fn configure_overlay_window_surface(window: &mut Window) {
    tracing::info!(
        category = "DICTATION",
        "Configuring dictation overlay native surface"
    );
    if let Ok(wh) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = wh.as_raw() {
            use cocoa::base::{id, NO, YES};
            use objc::{class, msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as id;
            // SAFETY: ns_view belongs to the live dictation overlay window on
            // the main thread.  We set the window to non-opaque with a clear
            // background and apply a corner mask to the content view's layer
            // so the pill shape clips cleanly.
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window.is_null() {
                    tracing::warn!(
                        category = "DICTATION",
                        "Cannot configure dictation overlay surface: NSWindow is null"
                    );
                    return;
                }
                let () = msg_send![ns_window, setOpaque: NO];
                // Match the main/notes/AI windows: paint the system
                // windowBackgroundColor on the NSWindow itself so vibrancy and
                // the ~1px native rim render the same way across all Script
                // Kit windows. The rounded outer shape is clipped via the
                // contentView's layer mask below.
                //
                // Glass mode: the shared vibrancy config already set the
                // near-clear base (0.0001-alpha black, 57220a68c) so the
                // NSGlassEffectView backdrop IS the visible material.
                // Overwriting it here with the opaque system color painted
                // the whole overlay as a dark slab over the glass — this
                // runs AFTER configure_dictation_overlay_window, so it must
                // preserve the glass base, not "restore" the blur-era one.
                let glass_mode = crate::platform::tahoe_liquid_glass_available()
                    && crate::theme::get_cached_theme().is_vibrancy_enabled();
                if !glass_mode {
                    let window_bg_color: id = msg_send![class!(NSColor), windowBackgroundColor];
                    if !window_bg_color.is_null() {
                        let () = msg_send![ns_window, setBackgroundColor: window_bg_color];
                    }
                }

                let content_view: id = msg_send![ns_window, contentView];
                if content_view.is_null() {
                    tracing::warn!(
                        category = "DICTATION",
                        "Cannot configure dictation overlay surface: contentView is null"
                    );
                    return;
                }
                let () = msg_send![content_view, setWantsLayer: YES];
                let layer: id = msg_send![content_view, layer];
                if layer.is_null() {
                    tracing::warn!(
                        category = "DICTATION",
                        "Cannot configure dictation overlay surface: layer is null"
                    );
                    return;
                }
                if glass_mode {
                    // The physical NSWindow is only a transparent composition
                    // host now. Clipping its full bounds would reconnect the
                    // bounded content capsule to the floating footer strip.
                    // The GPUI stage and native backdrop own their own radius.
                    let () = msg_send![layer, setCornerRadius: 0.0f64];
                    let () = msg_send![layer, setMasksToBounds: NO];
                } else {
                    let () = msg_send![layer, setCornerRadius: OVERLAY_RADIUS_PX as f64];
                    let () = msg_send![layer, setMasksToBounds: YES];
                }
                tracing::info!(
                    category = "DICTATION",
                    radius = OVERLAY_RADIUS_PX,
                    detached_footer = glass_mode,
                    "Configured dictation overlay native surface"
                );
            }
        }
    } else {
        tracing::warn!(
            category = "DICTATION",
            "Cannot configure dictation overlay surface: raw window handle unavailable"
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_overlay_window_surface(_window: &mut Window) {}

/// Prepare the overlay window for a clean close: set alpha to 0 so the
/// NSWindow backing store clear doesn't flash white.
#[cfg(target_os = "macos")]
fn prepare_overlay_window_for_close(window: &mut Window) {
    configure_overlay_window_surface(window);
    if let Ok(wh) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = wh.as_raw() {
            use cocoa::base::id;
            use objc::{class, msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as id;
            // SAFETY: ns_view belongs to the live dictation overlay window on
            // the main thread.  We set alpha to 0 before remove_window so the
            // backing store clear is invisible.
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window.is_null() {
                    return;
                }
                let clear: id = msg_send![class!(NSColor), clearColor];
                let () = msg_send![ns_window, setBackgroundColor: clear];
                let () = msg_send![ns_window, setAlphaValue: 0.0_f64];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn prepare_overlay_window_for_close(_window: &mut Window) {}

/// Open the dictation overlay as a compact floating pill.
///
/// Creates a `WindowKind::PopUp` window with blurred background and vibrancy.
/// Returns a handle that can be used to update or close the overlay.
pub fn open_dictation_overlay(
    cx: &mut App,
) -> anyhow::Result<gpui::WindowHandle<DictationOverlay>> {
    use anyhow::Context as _;

    let generation = overlay_generation();
    tracing::info!(
        category = "DICTATION",
        generation,
        "open_dictation_overlay requested"
    );
    ESCAPE_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    ENTER_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);

    // If already open AND the native window is still alive, return the
    // existing handle.  If the handle is stale (window was closed natively),
    // clear the slot and fall through to create a fresh one.
    let slot = DICTATION_OVERLAY_WINDOW.get_or_init(|| Mutex::new(None));
    let existing_handle = { *slot.lock() };
    if let Some(handle) = existing_handle {
        let alive = handle
            .update(cx, |_view, window, _cx| {
                if DICTATION_OVERLAY_EXIT_IN_PROGRESS.swap(false, Ordering::SeqCst) {
                    crate::platform::cancel_gpui_window_exit_dematerialize(window);
                    DICTATION_OVERLAY_EXIT_TICKET.lock().take();
                    tracing::info!(
                        category = "DICTATION",
                        event = "dictation_exit_superseded",
                        "Reused the live Dictation CGWindow and invalidated its exit tail"
                    );
                }
            })
            .is_ok();
        if alive {
            tracing::info!(
                category = "DICTATION",
                generation,
                "Reusing existing live dictation overlay handle"
            );
            return Ok(handle);
        }
        // Stale handle — clear and recreate.
        tracing::warn!(
            category = "DICTATION",
            "Overlay handle was stale, clearing slot"
        );
        let mut guard = slot.lock();
        if guard.is_some() {
            *guard = None;
        }
    }

    let theme = get_cached_theme();
    let window_background = if theme.is_vibrancy_enabled() {
        if crate::platform::tahoe_liquid_glass_available() {
            // Tahoe: the native glass backdrop supplies the material; Blurred
            // would cover it with the fork's NSVisualEffectView.
            gpui::WindowBackgroundAppearance::Transparent
        } else {
            gpui::WindowBackgroundAppearance::Blurred
        }
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };
    tracing::info!(
        category = "DICTATION",
        generation,
        vibrancy_enabled = theme.is_vibrancy_enabled(),
        dark_vibrancy = theme.should_use_dark_vibrancy(),
        "Resolved dictation overlay theme/window background"
    );

    // Bottom-center positioning matching vercel-voice: centered horizontally,
    // 15px above the bottom of the active display's visible area.
    let bounds = calculate_overlay_bottom_center_bounds();
    tracing::info!(
        category = "DICTATION",
        generation,
        x = bounds.origin.x.as_f32(),
        y = bounds.origin.y.as_f32(),
        width = bounds.size.width.as_f32(),
        height = bounds.size.height.as_f32(),
        "Dictation overlay bounds ready"
    );

    // Snapshot main-window visibility BEFORE any native window operations.
    // If it was hidden, we must ensure it stays hidden after creating the
    // overlay — macOS may surface sibling panels at the same level.
    let main_was_visible = crate::is_main_window_visible();
    tracing::info!(
        category = "DICTATION",
        generation,
        main_was_visible,
        "Captured launcher visibility before dictation overlay open"
    );

    // focus: false + show: false — the overlay must not activate the app or
    // surface the main window.  Creating a PopUp with show:true causes macOS
    // to surface sibling panels at the same window level, which briefly
    // flashes the main window.  We create hidden, then bring to front via
    // orderFrontRegardless below (which only surfaces THIS window).
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background,
        focus: false,
        show: false,
        kind: gpui::WindowKind::PopUp,
        is_resizable: false,
        ..Default::default()
    };

    tracing::info!(
        category = "DICTATION",
        generation,
        focus = window_options.focus,
        show = window_options.show,
        resizable = window_options.is_resizable,
        "Opening dictation overlay GPUI popup window"
    );
    let handle = cx
        .open_window(window_options, |_window, cx| cx.new(DictationOverlay::new))
        .context("Failed to open dictation overlay window")?;
    tracing::info!(
        category = "DICTATION",
        generation,
        "Dictation overlay GPUI window handle created"
    );

    // Configure vibrancy on macOS (never call setLevel on PopUp windows).
    #[cfg(target_os = "macos")]
    {
        tracing::info!(
            category = "DICTATION",
            generation,
            "Configuring dictation overlay vibrancy and native surface"
        );
        match handle.update(cx, |_view, window, _cx| {
            let is_dark = theme.should_use_dark_vibrancy();
            // Obtain the NSView from the raw window handle, then get its parent NSWindow.
            if let Ok(wh) = raw_window_handle::HasWindowHandle::window_handle(window) {
                if let raw_window_handle::RawWindowHandle::AppKit(appkit) = wh.as_raw() {
                    use objc::{msg_send, sel, sel_impl};
                    let ns_view = appkit.ns_view.as_ptr() as cocoa::base::id;
                    // SAFETY: ns_view is a valid NSView from a just-created GPUI window.
                    // We obtain the parent NSWindow via the standard -[NSView window] message.
                    // Called on the main thread as required by AppKit.
                    unsafe {
                        let ns_window: cocoa::base::id = msg_send![ns_view, window];
                        crate::platform::configure_dictation_overlay_window(ns_window, is_dark);
                    }
                }
            }

            // Apply clear background + corner mask after vibrancy.
            configure_overlay_window_surface(window);
        }) {
            Ok(()) => tracing::info!(
                category = "DICTATION",
                generation,
                "Configured dictation overlay vibrancy and native surface"
            ),
            Err(error) => tracing::warn!(
                category = "DICTATION",
                generation,
                %error,
                "Failed to configure dictation overlay vibrancy/native surface"
            ),
        }
    }

    // Always make the overlay key window so it receives Escape/Enter key
    // events in both visible-app and hidden-app dictation flows.  The
    // overlay is a NonactivatingPanel (PopUp) and the app uses ACCESSORY
    // activation policy, so makeKeyWindow does not surface the main window
    // or activate the app.  When main is hidden, orderOut: is sent first
    // (below) to prevent sibling-panel flash.
    let should_key_overlay = true;

    #[cfg(target_os = "macos")]
    {
        tracing::info!(
            category = "DICTATION",
            generation,
            main_was_visible,
            should_key_overlay,
            "Surfacing dictation overlay NSWindow"
        );
        match handle.update(cx, |_view, window, _cx| {
            if let Ok(wh) = raw_window_handle::HasWindowHandle::window_handle(window) {
                if let raw_window_handle::RawWindowHandle::AppKit(appkit) = wh.as_raw() {
                    use objc::{msg_send, sel, sel_impl};
                    let ns_view = appkit.ns_view.as_ptr() as cocoa::base::id;
                    // SAFETY: ns_view is a valid NSView from a just-created GPUI window.
                    // orderFrontRegardless brings the window to front without activating the app.
                    // makeKeyWindow is always sent so the overlay receives Escape/Enter key
                    // events in both visible-app and hidden-app dictation flows.
                    unsafe {
                        let ns_window: cocoa::base::id = msg_send![ns_view, window];

                        // Hide the main window BEFORE surfacing the overlay.
                        // orderFrontRegardless on a PopUp can cause macOS to
                        // also surface sibling panels at the same level.  By
                        // sending orderOut: to the main panel first (in the
                        // same synchronous call), it cannot flash on screen.
                        // SAFETY: get_main_window returns the registered main
                        // NSPanel; orderOut: with nil sender is safe.
                        if !main_was_visible {
                            if let Some(main_window) = crate::window_manager::get_main_window() {
                                let () = msg_send![main_window, orderOut: cocoa::base::nil];
                            }
                        }

                        let () = msg_send![ns_window, orderFrontRegardless];
                        if should_key_overlay {
                            let () = msg_send![ns_window, makeKeyWindow];
                        }

                        tracing::info!(
                            target: "script_kit::dictation",
                            event = "dictation_overlay_nswindow_surfaced",
                            main_was_visible,
                            made_key = should_key_overlay,
                            "Surfaced dictation overlay without surfacing sibling launcher panels"
                        );
                    }
                }
            }
        }) {
            Ok(()) => tracing::info!(
                category = "DICTATION",
                generation,
                "Dictation overlay NSWindow surfacing step completed"
            ),
            Err(error) => tracing::warn!(
                category = "DICTATION",
                generation,
                %error,
                "Failed during dictation overlay NSWindow surfacing step"
            ),
        }
    }

    // Focus the GPUI focus handle so key events (Escape) are delivered —
    // but only when we also made the window key, otherwise this would
    // activate the app.
    if should_key_overlay {
        tracing::info!(
            category = "DICTATION",
            generation,
            "Focusing dictation overlay GPUI focus handle"
        );
        match handle.update(cx, |view, window, cx| {
            view.focus_handle.focus(window, cx);
        }) {
            Ok(()) => tracing::info!(
                category = "DICTATION",
                generation,
                "Focused dictation overlay GPUI focus handle"
            ),
            Err(error) => tracing::warn!(
                category = "DICTATION",
                generation,
                %error,
                "Failed to focus dictation overlay GPUI focus handle"
            ),
        }
    }

    // Store handle.
    {
        let mut guard = slot.lock();
        *guard = Some(handle);
    }
    let overlay_any: gpui::AnyWindowHandle = handle.into();
    crate::windows::upsert_runtime_window_handle(DICTATION_OVERLAY_AUTOMATION_ID, overlay_any);
    crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
        id: DICTATION_OVERLAY_AUTOMATION_ID.to_string(),
        kind: crate::protocol::AutomationWindowKind::Dictation,
        title: Some("Script Kit Dictation".to_string()),
        focused: should_key_overlay,
        visible: true,
        semantic_surface: Some("dictation".to_string()),
        bounds: Some(dictation_automation_bounds(bounds)),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
        generation: None,
    });
    tracing::info!(
        category = "DICTATION",
        generation,
        "Stored dictation overlay handle"
    );

    // Install global escape monitor so Escape works even when the overlay
    // doesn't have keyboard focus (user clicked on another app).
    install_global_escape_monitor();

    tracing::info!(
        category = "DICTATION",
        generation,
        "Dictation overlay window opened"
    );
    Ok(handle)
}

pub(crate) fn open_dictation_microphone_popup_fixture(cx: &mut App) -> anyhow::Result<()> {
    if !dictation_overlay_fixture_mode() {
        anyhow::bail!("Dictation microphone fixture requires an active overlay fixture");
    }
    let handle = DICTATION_OVERLAY_WINDOW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .as_ref()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("Dictation overlay fixture is not open"))?;
    handle
        .update(cx, |view, window, cx| {
            view.open_microphone_picker(window, cx);
        })
        .map_err(|error| anyhow::anyhow!("Failed to open Dictation microphone fixture: {error}"))
}

/// Push a new state snapshot into the overlay (no-op if overlay is not open).
pub fn update_dictation_overlay(state: DictationOverlayState, cx: &mut App) -> anyhow::Result<()> {
    let slot = DICTATION_OVERLAY_WINDOW.get_or_init(|| Mutex::new(None));
    let handle = {
        let guard = slot.lock();
        match *guard {
            Some(h) => h,
            None => return Ok(()), // Not open — nothing to update.
        }
    };

    let _ = handle.update(cx, |view, window, cx| {
        // Check for global escape before applying state — the escape may
        // close the overlay, in which case set_state is a no-op.
        match view.process_global_keys_if_requested(window, cx) {
            GlobalKeyProcessResult::None => view.set_state(state, window, cx),
            GlobalKeyProcessResult::StateChanged | GlobalKeyProcessResult::Closed => {
                tracing::debug!(
                    category = "DICTATION",
                    "Skipped stale overlay pump state after global key action"
                );
            }
        }
    });

    Ok(())
}

/// Close the dictation overlay window.
pub fn close_dictation_overlay(cx: &mut App) -> anyhow::Result<()> {
    set_dictation_overlay_fixture_mode(false);
    crate::dictation::close_dictation_microphone_popup_window_for_owner_loss(cx);
    *OVERLAY_ABORT_CALLBACK.lock() = None;
    *OVERLAY_SUBMIT_CALLBACK.lock() = None;
    ESCAPE_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    ENTER_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    // Overlay window already gone is a warning, not an error.
    remove_global_escape_monitor();

    let slot = DICTATION_OVERLAY_WINDOW.get_or_init(|| Mutex::new(None));
    if DICTATION_OVERLAY_EXIT_IN_PROGRESS.load(Ordering::SeqCst) {
        tracing::debug!(
            category = "DICTATION",
            "Dictation overlay exit already in progress"
        );
        return Ok(());
    }
    let handle = { *slot.lock() };

    if let Some(handle) = handle {
        // Keep the native capsule host and owning handle registered throughout
        // the fade. A rapid reopen cancels this ticket and reuses the CGWindow.
        let result = handle.update(cx, |_view, window, cx| {
            if let Some(ticket) = crate::platform::begin_gpui_window_exit_with_ticket(
                window,
                "DICTATION",
                "Dictation overlay",
            ) {
                *DICTATION_OVERLAY_EXIT_TICKET.lock() = Some(ticket);
                DICTATION_OVERLAY_EXIT_IN_PROGRESS.store(true, Ordering::SeqCst);
                let any_handle = window.window_handle();
                cx.spawn(async move |_view, cx: &mut gpui::AsyncApp| {
                    cx.background_executor()
                        .timer(crate::platform::glass_exit_remove_delay())
                        .await;
                    cx.update(|cx| {
                        let pending_matches = *DICTATION_OVERLAY_EXIT_TICKET.lock() == Some(ticket);
                        if !pending_matches
                            || !crate::platform::glass_exit_ticket_is_current(ticket)
                        {
                            return;
                        }
                        DICTATION_OVERLAY_EXIT_TICKET.lock().take();
                        crate::platform::record_glass_exit_commit(ticket);
                        let _ = any_handle.update(cx, |_view, window, _cx| {
                            crate::components::footer_chrome::remove_glass_capsule_window(window);
                            prepare_overlay_window_for_close(window);
                            window.remove_window();
                        });
                        DICTATION_OVERLAY_WINDOW
                            .get_or_init(|| Mutex::new(None))
                            .lock()
                            .take();
                        crate::windows::remove_runtime_window_handle(
                            DICTATION_OVERLAY_AUTOMATION_ID,
                        );
                        crate::windows::remove_automation_window(DICTATION_OVERLAY_AUTOMATION_ID);
                        DICTATION_OVERLAY_EXIT_IN_PROGRESS.store(false, Ordering::SeqCst);
                    });
                })
                .detach();
            } else {
                crate::components::footer_chrome::remove_glass_capsule_window(window);
                prepare_overlay_window_for_close(window);
                window.remove_window();
                DICTATION_OVERLAY_WINDOW
                    .get_or_init(|| Mutex::new(None))
                    .lock()
                    .take();
                crate::windows::remove_runtime_window_handle(DICTATION_OVERLAY_AUTOMATION_ID);
                crate::windows::remove_automation_window(DICTATION_OVERLAY_AUTOMATION_ID);
                DICTATION_OVERLAY_EXIT_IN_PROGRESS.store(false, Ordering::SeqCst);
            }
        });
        if result.is_ok() {
            tracing::info!(category = "DICTATION", "Dictation overlay window closed");
        } else {
            tracing::warn!(
                category = "DICTATION",
                "Overlay window already gone when close was called"
            );
        }
    }
    Ok(())
}

pub(crate) fn dictation_window_lifecycle_receipt() -> serde_json::Value {
    let handle_registered = DICTATION_OVERLAY_WINDOW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .is_some();
    let ticket = *DICTATION_OVERLAY_EXIT_TICKET.lock();
    serde_json::json!({
        "schemaVersion": 2,
        "phase": if DICTATION_OVERLAY_EXIT_IN_PROGRESS.load(Ordering::SeqCst) {
            "Exiting"
        } else if handle_registered {
            "Open"
        } else {
            "Closed"
        },
        "handleRegistered": handle_registered,
        "exitInProgress": DICTATION_OVERLAY_EXIT_IN_PROGRESS.load(Ordering::SeqCst),
        "hasExitTicket": ticket.is_some(),
        "exitGeneration": ticket.map(crate::platform::GlassExitTicket::generation),
        "automationRegistered": crate::windows::list_automation_windows()
            .iter()
            .any(|window| window.id == DICTATION_OVERLAY_AUTOMATION_ID),
        "nativeExit": crate::platform::glass_exit_lifecycle_receipt("Dictation overlay"),
    })
}

fn dictation_automation_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

pub fn automation_layout_info(
    resolved: &crate::protocol::AutomationWindowInfo,
) -> crate::protocol::LayoutInfo {
    use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
    use crate::ui::chrome as chrome_tokens;

    let bounds = resolved
        .bounds
        .clone()
        .unwrap_or(crate::protocol::AutomationWindowBounds {
            x: 0.0,
            y: 0.0,
            width: OVERLAY_WIDTH_PX as f64,
            height: OVERLAY_HEIGHT_PX as f64,
        });
    let width = bounds.width as f32;
    let height = bounds.height as f32;
    let theme = get_cached_theme();
    let detached_footer =
        crate::platform::tahoe_liquid_glass_available() && theme.is_vibrancy_enabled();
    let footer_height = crate::components::footer_chrome::footer_rail_chrome(&theme).height_px;
    let footer_gap = if detached_footer {
        crate::footer_popup::FLOAT_FOOTER_CONTAINER_GAP_PX
    } else {
        0.0
    };
    let regions = crate::footer_popup::main_window_detached_footer_regions_gpui(
        width,
        height,
        footer_height,
        footer_gap,
        1.0,
    );
    let stage_height = regions.main_content.height;
    let content_parent = if detached_footer {
        "DictationContentStage"
    } else {
        "DictationOverlayWindow"
    };
    let header_top = 5.0;
    let header_height = OVERLAY_HEADER_ROW_HEIGHT_PX;
    let caption_top = header_top + header_height;
    let caption_height = (stage_height - caption_top).max(0.0);

    let mut components =
        vec![
            LayoutComponentInfo::new("DictationOverlayWindow", LayoutComponentType::Container)
                .with_bounds(0.0, 0.0, width, height)
                .with_visual_style(
                    if detached_footer {
                        chrome_tokens::CHROME_LAYER_WINDOW_BACKDROP
                    } else {
                        chrome_tokens::CHROME_LAYER_FLOATING
                    },
                    if detached_footer {
                        chrome_tokens::MATERIAL_NATIVE_WINDOW_BACKDROP
                    } else {
                        chrome_tokens::MATERIAL_NS_VISUAL_EFFECT
                    },
                    if detached_footer {
                        None
                    } else {
                        Some(OVERLAY_RADIUS_PX)
                    },
                )
                .with_hit_bounds(0.0, 0.0, width, height)
                .with_padding(0.0, 0.0, 0.0, 0.0),
        ];
    if detached_footer {
        components.push(
            LayoutComponentInfo::new("DictationContentStage", LayoutComponentType::Container)
                .with_bounds(
                    regions.main_content.x,
                    regions.main_content.y,
                    regions.main_content.width,
                    regions.main_content.height,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FLOATING,
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                    Some(OVERLAY_RADIUS_PX),
                )
                .with_hit_bounds(
                    regions.main_content.x,
                    regions.main_content.y,
                    regions.main_content.width,
                    regions.main_content.height,
                )
                .with_depth(1)
                .with_parent("DictationOverlayWindow"),
        );
    }
    components.extend([
        LayoutComponentInfo::new("DictationHeaderRow", LayoutComponentType::Container)
            .with_bounds(0.0, header_top, width, header_height)
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                None,
            )
            .with_hit_bounds(0.0, header_top, width, header_height)
            .with_padding(
                0.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
                0.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
            )
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationTimerSlot", LayoutComponentType::Other)
            .with_bounds(
                OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                TARGET_BADGE_SLOT_WIDTH_PX,
                header_height,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_CONTENT,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                None,
            )
            .with_padding(0.0, 0.0, 0.0, 0.0)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationDestinationChips", LayoutComponentType::Button)
            .with_bounds(
                TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                (width - 2.0 * (TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX))
                    .max(0.0),
                header_height,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(chrome_tokens::LIQUID_GLASS_CONTROL_RADIUS_PX),
            )
            .with_hit_bounds(
                TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                (width - 2.0 * (TARGET_BADGE_SLOT_WIDTH_PX + OVERLAY_HORIZONTAL_PADDING_PX))
                    .max(0.0),
                header_height,
            )
            .with_gap(6.0)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationTargetBadge", LayoutComponentType::Button)
            .with_bounds(
                width - TARGET_BADGE_SLOT_WIDTH_PX - OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                TARGET_BADGE_SLOT_WIDTH_PX,
                header_height,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(chrome_tokens::LIQUID_GLASS_CONTROL_RADIUS_PX),
            )
            .with_hit_bounds(
                width - TARGET_BADGE_SLOT_WIDTH_PX - OVERLAY_HORIZONTAL_PADDING_PX,
                header_top,
                TARGET_BADGE_SLOT_WIDTH_PX,
                header_height,
            )
            .with_padding(2.0, 8.0, 2.0, 8.0)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationSignalBand", LayoutComponentType::Container)
            .with_bounds(0.0, caption_top, width, caption_height)
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(OVERLAY_RADIUS_PX),
            )
            .with_hit_bounds(0.0, caption_top, width, caption_height)
            .with_padding(
                6.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
                6.0,
                OVERLAY_HORIZONTAL_PADDING_PX,
            )
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
        LayoutComponentInfo::new("DictationWaveform", LayoutComponentType::Container)
            .with_bounds(
                (width - 48.0) / 2.0,
                caption_top + (caption_height - WAVEFORM_BAR_MAX_HEIGHT_PX).max(0.0) / 2.0,
                48.0,
                WAVEFORM_BAR_MAX_HEIGHT_PX,
            )
            .with_visual_style(
                chrome_tokens::CHROME_LAYER_CONTENT,
                chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
            )
            .with_gap(WAVEFORM_BAR_GAP_PX)
            .with_depth(if detached_footer { 2 } else { 1 })
            .with_parent(content_parent),
    ]);
    if detached_footer {
        components.push(
            LayoutComponentInfo::new(
                "DictationFooterDesktopGutter",
                LayoutComponentType::Other,
            )
            .with_bounds(
                regions.transparent_gap.x,
                regions.transparent_gap.y,
                regions.transparent_gap.width,
                regions.transparent_gap.height,
            )
            .with_depth(1)
            .with_parent("DictationOverlayWindow")
            .with_explanation(
                "Fully transparent 8-point desktop gutter separating the dictation capsule from its floating controls.",
            ),
        );
    }
    components.push(
        LayoutComponentInfo::new("DictationFooterRail", LayoutComponentType::Panel)
            .with_bounds(
                regions.footer.x,
                regions.footer.y,
                regions.footer.width,
                regions.footer.height,
            )
            .with_visual_style(
                if detached_footer {
                    chrome_tokens::CHROME_LAYER_FLOATING
                } else {
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL
                },
                if detached_footer {
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT
                } else {
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN
                },
                Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
            )
            .with_hit_bounds(
                regions.footer.x,
                regions.footer.y,
                regions.footer.width,
                regions.footer.height,
            )
            .with_padding(0.0, 0.0, 0.0, 0.0)
            .with_depth(1)
            .with_parent("DictationOverlayWindow")
            .with_explanation(if detached_footer {
                "Transparent positioning rail containing discrete native glass capsules; it has no full-width footer surface."
            } else {
                "In-window dictation action rail."
            }),
    );

    LayoutInfo {
        window_width: width,
        window_height: height,
        prompt_type: "dictation".to_string(),
        components,
        fidelity: None,
        handler_form: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

/// Check whether the dictation overlay window is currently open.
///
/// Note: this only checks whether the slot holds a handle.  For true
/// liveness validation (which requires `&mut App`), use
/// `open_dictation_overlay` which probes the handle before reuse.
pub fn is_dictation_overlay_open() -> bool {
    let slot = DICTATION_OVERLAY_WINDOW.get_or_init(|| Mutex::new(None));
    let guard = slot.lock();
    guard.is_some()
}

/// Bump the logical overlay session generation.
///
/// Call this on every `DictationToggleOutcome::Started`, even when reusing
/// the same overlay window handle.  This ensures that stale delayed closes
/// from a prior session see a generation mismatch and bail.
pub fn begin_overlay_session() -> u64 {
    OVERLAY_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

/// Return the current overlay generation counter.
///
/// Async tasks capture this value at spawn time and compare on each tick.
/// When the live value differs, the task is stale and must stop.
pub fn overlay_generation() -> u64 {
    OVERLAY_GENERATION.load(Ordering::SeqCst)
}

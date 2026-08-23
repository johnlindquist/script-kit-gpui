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
    /// `f64::to_bits` of the visible entry start alpha the morph launched
    /// with (bits keep the receipt `Eq`). `None` when no morph ran.
    pub(crate) morph_start_alpha_bits: Option<u64>,
    /// TOTAL entry duration (material onset + visible tail) in ms.
    pub(crate) settle_duration_ms: u64,
    /// Material onset prefix (glass Clear→Regular ramp) in ms.
    pub(crate) material_onset_duration_ms: u64,
    /// Geometry/alpha tail duration in ms.
    pub(crate) visible_tail_duration_ms: u64,
    /// GPUI content hold before its fade, in ms.
    pub(crate) content_hold_duration_ms: u64,
    /// GPUI content fade duration in ms.
    pub(crate) content_fade_duration_ms: u64,
    /// Time from native morph start until phase one first crosses the final
    /// window size — the pure GEOMETRIC crossing (23ms at the default
    /// visible-tail calibration). Content reveal must use
    /// `content_reveal_delay_ms`, which also waits out the alpha ramp.
    pub(crate) settled_crossing_delay_ms: u64,
    /// Material-safe reveal anchor: max(geometric crossing, alpha ramp).
    pub(crate) content_reveal_delay_ms: u64,
    /// Compression duration (phase one) in ms.
    pub(crate) phase_one_duration_ms: u64,
    /// Rebound duration (phase two) in ms.
    pub(crate) phase_two_duration_ms: u64,
    /// 0.85 → 0.99 alpha ramp duration in ms.
    pub(crate) alpha_ramp_duration_ms: u64,
    /// 0.99 → 1.0 finishing alpha duration in ms.
    pub(crate) alpha_finish_duration_ms: u64,
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
    /// Alpha the phase-one ramp targets (0.99 — never fully opaque while the
    /// window is wider than natural size).
    phase1_alpha_target: f64,
    /// Duration of the start_alpha → phase1_alpha_target leg (clamped to
    /// phase1).
    alpha_ramp_duration: f64,
    /// Duration of the phase1_alpha_target → 1.0 leg from rebound start
    /// (clamped to phase2).
    alpha_finish_duration: f64,
    /// Material onset prefix duration before the visible tail begins.
    material_onset_duration: f64,
    /// GPUI content hold before its fade (within the onset).
    content_hold_duration: f64,
    /// GPUI content fade duration (ends at tail start).
    content_fade_duration: f64,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainFooterEntryDefocusScope {
    PerCapsule,
}

#[cfg(target_os = "macos")]
impl MainFooterEntryDefocusScope {
    fn log_name(self) -> &'static str {
        match self {
            Self::PerCapsule => "per_capsule",
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct MainFooterEntryMaterialPolicy {
    target_alpha: f64,
    /// The gap-spanning footer CONTAINER never joins the content fade — a
    /// translucent container would mix desktop pixels through the
    /// inter-capsule gaps (the measured 2026-08-12 capsule-fade defect).
    enroll_in_content_fade: bool,
    defocus_scope: MainFooterEntryDefocusScope,
    /// Radius applied independently to every clipped NSGlassEffectView
    /// capsule; never apply this value to the footer container or hints host.
    defocus_radius: f64,
    /// Each capsule runs the SAME Clear→Regular + tint material ramp as the
    /// main backdrop across the onset prefix (2026-08-13 parity retune, user
    /// report: the capsules "don't match the blur of the main window" — the
    /// material ramp, not the layer defocus, is the visible bloom).
    material_onset_ramp: bool,
    /// Each capsule's own foreground contentView joins the shared content
    /// fade at the content presence floor, so labels bloom in with the main
    /// content instead of popping in crisp. This is PER-CAPSULE foreground
    /// alpha, never the container (see enroll_in_content_fade).
    foreground_content_fade: bool,
}

#[cfg(target_os = "macos")]
fn main_footer_entry_material_policy() -> MainFooterEntryMaterialPolicy {
    MainFooterEntryMaterialPolicy {
        target_alpha: 1.0,
        enroll_in_content_fade: false,
        defocus_scope: MainFooterEntryDefocusScope::PerCapsule,
        defocus_radius: glass_main_entry_blur_radius(),
        material_onset_ramp: true,
        foreground_content_fade: true,
    }
}

/// Which product surface is entering. The geometry function must be told this
/// EXPLICITLY by its caller — never infer it from `window_name`, which is a
/// human-readable log label and not a policy input.
///
/// Introduced by Oracle session `glass-entry-feel-options` WP1 as the typed
/// replacement for the old `grow_in: bool`. The bool could only express
/// direction; the user's request ("make main feel like the Cmd+K Actions
/// menu") needs direction, travel scaling, and onset to vary per surface.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassEntrySurface {
    /// The launcher panel, summoned by the global hotkey.
    Main,
    /// A popup attached to a parent NSWindow (Actions/Cmd+K and friends).
    ChildPopup,
    /// Notes, Dictation, HUD — free-standing secondary windows.
    FreeStandingSecondary,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassEntryDirection {
    /// Spotlight's outset enter: start wider, compress below final, rebound.
    ShrinkIn,
    /// The child-popup enter: start below final, overshoot past it, settle.
    GrowIn,
}

#[cfg(target_os = "macos")]
impl GlassEntryDirection {
    /// Multiplies every per-side travel. `ShrinkIn` keeps the historical
    /// `+1.0` (start outset, squish under final); `GrowIn` inverts it.
    fn geometry_sign(self) -> f64 {
        match self {
            Self::ShrinkIn => 1.0,
            Self::GrowIn => -1.0,
        }
    }

    fn log_name(self) -> &'static str {
        match self {
            Self::ShrinkIn => "shrink_in",
            Self::GrowIn => "grow_in",
        }
    }
}

/// How per-side travel is derived from the settled window width.
///
/// `Fractional` is the only policy production uses today. The other two are
/// the WP2/WP4 candidates from `glass-entry-feel-options` and are unreachable
/// until `glass_entry_policy` returns them.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassEntryTravelPolicy {
    /// Travel = width × fraction. Same percentage on every surface, which
    /// means a wide window physically moves further and faster.
    Fractional,
    /// Travel capped at the Actions popup's point displacement, so a large
    /// window moves the same NUMBER OF POINTS as the popup rather than the
    /// same percentage.
    ///
    /// Not yet resolved by `glass_entry_policy` — this is the WP2 candidate
    /// from `glass-entry-feel-options`. Implemented and unit-tested now so the
    /// retune is a one-line policy change rather than new geometry code.
    #[allow(dead_code)]
    ActionsPointCapped,
    /// No geometry at all — alpha-only entry. The WP4 fade-only candidate;
    /// see `ActionsPointCapped` for why it is implemented ahead of use.
    #[allow(dead_code)]
    FixedFrame,
}

#[cfg(target_os = "macos")]
impl GlassEntryTravelPolicy {
    fn log_name(self) -> &'static str {
        match self {
            Self::Fractional => "fractional",
            Self::ActionsPointCapped => "actions_point_capped",
            Self::FixedFrame => "fixed_frame",
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassEntryOnsetPolicy {
    /// Material onset prefix (glass Clear→Regular + held GPUI content roots)
    /// before the visible tail.
    Full,
    /// Skip the onset; the visible tail starts immediately. The WP4 tail-only
    /// candidate — not yet resolved by `glass_entry_policy`.
    #[allow(dead_code)]
    TailOnly,
}

/// How the FIRST PHOTON's geometry relates to the calibrated visible-tail
/// start frame.
///
/// Retuned 2026-08-13 from the user-supplied 57fps Spotlight reference
/// (`.artifacts/entry-onset-retune/reference/measurements.json`): Spotlight's
/// first photon is ~103.05% of settled width and converges to the ~101.2%
/// tail-start width within the material prefix. The main window restores that
/// soft-materialize stretch; popups and secondary windows stay tail-aligned.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlassEntryOnsetGeometry {
    /// Begin directly on the calibrated visible-tail frame.
    TailAligned,
    /// Restore Spotlight's wider first photon, then converge to the calibrated
    /// visible-tail frame inside the existing material-onset prefix.
    SpotlightSoftMaterialize,
}

#[cfg(target_os = "macos")]
impl GlassEntryOnsetGeometry {
    fn start_per_side(self, final_width: f64, tail_start_per_side: f64) -> f64 {
        match self {
            Self::TailAligned => tail_start_per_side,
            Self::SpotlightSoftMaterialize => {
                final_width * (GLASS_MAIN_ONSET_START_WIDTH_SCALE - 1.0) * 0.5
            }
        }
    }

    fn duration(self) -> f64 {
        match self {
            Self::TailAligned => 0.0,
            Self::SpotlightSoftMaterialize => GLASS_MAIN_ONSET_GEOMETRY_DURATION,
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlassEntryPolicy {
    direction: GlassEntryDirection,
    travel: GlassEntryTravelPolicy,
    onset: GlassEntryOnsetPolicy,
    onset_geometry: GlassEntryOnsetGeometry,
}

/// The per-surface entry policy.
///
/// This resolver is the single place a retune happens. As of WP1 it is
/// deliberately BEHAVIOR-PRESERVING: it reproduces exactly what the old
/// `grow_in: bool` did at each call site, so the typed refactor can land and
/// be verified before any motion changes.
#[cfg(target_os = "macos")]
fn glass_entry_policy(surface: GlassEntrySurface) -> GlassEntryPolicy {
    match surface {
        GlassEntrySurface::Main => GlassEntryPolicy {
            direction: GlassEntryDirection::ShrinkIn,
            travel: GlassEntryTravelPolicy::Fractional,
            onset: GlassEntryOnsetPolicy::Full,
            onset_geometry: GlassEntryOnsetGeometry::SpotlightSoftMaterialize,
        },
        GlassEntrySurface::ChildPopup => GlassEntryPolicy {
            direction: GlassEntryDirection::GrowIn,
            travel: GlassEntryTravelPolicy::Fractional,
            onset: GlassEntryOnsetPolicy::Full,
            onset_geometry: GlassEntryOnsetGeometry::TailAligned,
        },
        GlassEntrySurface::FreeStandingSecondary => GlassEntryPolicy {
            direction: GlassEntryDirection::ShrinkIn,
            travel: GlassEntryTravelPolicy::Fractional,
            onset: GlassEntryOnsetPolicy::Full,
            onset_geometry: GlassEntryOnsetGeometry::TailAligned,
        },
    }
}

#[cfg(target_os = "macos")]
impl GlassEntrySurface {
    fn log_name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::ChildPopup => "child_popup",
            Self::FreeStandingSecondary => "free_standing_secondary",
        }
    }

    /// The main window's stronger onset defocus vs the shared popup/secondary
    /// radius (2026-08-13 soft-materialize retune).
    fn entry_blur_radius(self) -> f64 {
        match self {
            Self::Main => glass_main_entry_blur_radius(),
            Self::ChildPopup | Self::FreeStandingSecondary => glass_entry_blur_radius(),
        }
    }

    /// Main resolves its defocus inside the material-onset prefix; other
    /// surfaces keep the historical full-entry ramp.
    fn entry_blur_duration(self, tuning: GlassMorphTuning) -> f64 {
        match self {
            Self::Main => tuning.material_onset_duration,
            Self::ChildPopup | Self::FreeStandingSecondary => tuning.total_entry_duration(),
        }
    }
}

/// Resolve the physical main NSWindow to `GlassEntrySurface::Main` at the
/// shared window-frame animation owner so the main-only onset policy is
/// actually exercised; every other window-frame surface stays secondary.
#[cfg(target_os = "macos")]
fn glass_entry_surface_for_window_frame(is_main_window: bool) -> GlassEntrySurface {
    if is_main_window {
        GlassEntrySurface::Main
    } else {
        GlassEntrySurface::FreeStandingSecondary
    }
}

/// Per-side HORIZONTAL travel, in points, for one entry.
///
/// Vertical travel is deliberately NOT modelled here: it is derived from the
/// window HEIGHT (not width) and damped by `GLASS_MORPH_VERTICAL_DAMPING`, and
/// routing it through a width-based resolver would change its formula.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct GlassEntryTravel {
    /// Distance per side at the first visible frame.
    start_per_side: f64,
    /// Distance per side at the opposite extreme (max compression for
    /// shrink-in, max overshoot for grow-in).
    extreme_per_side: f64,
}

/// Resolve per-side horizontal travel for a surface.
///
/// `Fractional` reproduces the pre-WP1 arithmetic exactly, so this function is
/// a no-op refactor at today's policies.
#[cfg(target_os = "macos")]
fn glass_entry_travel(
    policy: GlassEntryTravelPolicy,
    final_width: f64,
    inset_fraction: f64,
    actions_reference_width: f64,
) -> GlassEntryTravel {
    let squish_fraction = (inset_fraction * GLASS_MORPH_SQUISH_FACTOR)
        .clamp(GLASS_MORPH_MIN_SQUISH, GLASS_MORPH_MAX_SQUISH);
    let fractional = GlassEntryTravel {
        start_per_side: final_width * inset_fraction,
        extreme_per_side: final_width * squish_fraction,
    };
    match policy {
        GlassEntryTravelPolicy::Fractional => fractional,
        GlassEntryTravelPolicy::ActionsPointCapped => GlassEntryTravel {
            start_per_side: fractional
                .start_per_side
                .min(actions_reference_width * inset_fraction),
            extreme_per_side: fractional
                .extreme_per_side
                .min(actions_reference_width * squish_fraction),
        },
        GlassEntryTravelPolicy::FixedFrame => GlassEntryTravel {
            start_per_side: 0.0,
            extreme_per_side: 0.0,
        },
    }
}

/// Nominal Actions popup width used by `ActionsPointCapped`. Unused until a
/// policy selects that travel mode; WP0 exists to replace this nominal value
/// with the measured median rendered Actions width.
#[cfg(target_os = "macos")]
const GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH: f64 = 340.0;

#[cfg(target_os = "macos")]
impl GlassMorphTuning {
    /// The geometry/alpha tail (compression + rebound) — `duration`.
    fn visible_tail_duration(self) -> f64 {
        self.duration
    }
    /// Onset prefix + visible tail.
    fn total_entry_duration(self) -> f64 {
        self.material_onset_duration + self.visible_tail_duration()
    }
    fn visible_tail_start_delay_ms(self) -> u64 {
        (self.material_onset_duration * 1000.0).round() as u64
    }
}

#[cfg(target_os = "macos")]
fn cubic_bezier_axis(t: f64, first: f64, second: f64) -> f64 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * first + 3.0 * inverse * t * t * second + t * t * t
}

/// Convert a phase-one geometry progress to elapsed time for a CAMediaTiming
/// cubic-bezier whose value axis has control values (0, 1) and whose time
/// axis has control values (`time_c1`, `time_c2`).
#[cfg(target_os = "macos")]
fn bezier_time_fraction_for_progress(progress: f64, time_c1: f64, time_c2: f64) -> f64 {
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
    cubic_bezier_axis((low + high) * 0.5, time_c1, time_c2)
}

/// Time from native morph start until phase one's ease-out compression
/// (`cubic-bezier(0, 0, 0.58, 1)`) first crosses the final window size.
#[cfg(target_os = "macos")]
fn settled_size_crossing_delay_ms(tuning: GlassMorphTuning) -> u64 {
    let phase_distance = tuning.start_scale_x - tuning.squish_scale_x;
    if phase_distance <= f64::EPSILON {
        return 0;
    }
    let geometry_progress = ((tuning.start_scale_x - 1.0) / phase_distance).clamp(0.0, 1.0);
    let time_fraction = bezier_time_fraction_for_progress(geometry_progress, 0.0, 0.58);
    (tuning.phase1 * time_fraction * 1000.0).round() as u64
}

/// Material-safe content reveal anchor: content may begin revealing only
/// once BOTH the geometry has crossed the final size AND the alpha ramp has
/// completed — revealing text while alpha is still below 0.99 would read as
/// a double fade.
#[cfg(target_os = "macos")]
fn entry_content_reveal_delay_ms(tuning: GlassMorphTuning) -> u64 {
    tuning.visible_tail_start_delay_ms()
        + settled_size_crossing_delay_ms(tuning)
            .max((tuning.alpha_ramp_duration * 1000.0).ceil() as u64)
}

#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_DURATION: f64 = 0.02;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_INSET: f64 = 0.005;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_DURATION: f64 = 2.0;
/// Visible entry start alpha for EVERY entry variant (WindowFrame,
/// ContentLayer, FadeOnly).
///
/// NSWindow.alphaValue multiplies every pixel the window contributes, so at
/// 0.0 the user sees pure wallpaper where UI should be — the compositor
/// blends `screen = a*window + (1-a)*desktop` and the early entry frames
/// were up to 100% desktop. 0.85 is the lowest evidence-backed start that
/// keeps every displayed entry frame within the <=5 dE00 color budget while
/// still softening the deliberately exaggerated calibration start frame.
///
/// Calibration note: this is the ONE value the color-consistency premise
/// unlocked (HITL submission 98cab5e5-6f15-4311-8d49-83e31602e641; Oracle
/// plan `floating-capsule-entry-material` step 3). Every other Glass Motion
/// Calibration Lock value is untouched. Do not alias this to a theme token.
#[cfg(target_os = "macos")]
const GLASS_MORPH_ENTRY_START_ALPHA: f64 = 0.85;
/// Alpha for TRULY HIDDEN parking only (window ordered out between shows).
/// Zero-alpha parking of a visible window is a contract violation — the
/// park helpers runtime-check `isVisible == false` before applying this.
#[cfg(target_os = "macos")]
const GLASS_HIDDEN_PARK_ALPHA: f64 = 0.0;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_INSET: f64 = 0.4;
/// Height participation in the entry morph. Spotlight's measured height
/// undershoot is ±0–2px ("~0% height participation in the squish" —
/// https://eager-hollow-dyyf.here.now/): the morph is width-dominant, so the
/// visible-tail calibration removes vertical motion entirely.
#[cfg(target_os = "macos")]
const GLASS_MORPH_VERTICAL_DAMPING: f64 = 0.0;
/// Squish depth as a fraction of the entry inset, applied PER SIDE (the frame
/// math doubles it into total width). Spotlight's measured maximum undershoot
/// is −1.3% of total width (https://eager-hollow-dyyf.here.now/). With the
/// visible-tail inset of 0.006 the factor product (0.0015) sits below the
/// clamp, so the per-side minimum of 0.0065 is what actually renders:
/// 0.0065 ×2 = 1.3% total — exactly the measured squish.
#[cfg(target_os = "macos")]
const GLASS_MORPH_SQUISH_FACTOR: f64 = 0.25;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_SQUISH: f64 = 0.0065;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MAX_SQUISH: f64 = 0.015;
/// No explicit rest at max compression: the compression ease-out ends at
/// zero velocity and the rebound ease-in-out begins at zero velocity, which
/// is what reads as physical settling. (The earlier 50ms dead hold plus a
/// 90ms rebound made the tail feel sticky-then-quick; Spotlight's measured
/// rebound gets a full ~140ms.)
#[cfg(target_os = "macos")]
const GLASS_MORPH_SQUISH_HOLD: f64 = 0.0;
/// Compression is one third of the visible tail (35ms at the 0.105s
/// default), leaving two thirds (70ms) for the rebound.
#[cfg(target_os = "macos")]
const GLASS_MORPH_PHASE1_FRACTION: f64 = 1.0 / 3.0;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_REBOUND_DURATION: f64 = 0.04;
#[cfg(target_os = "macos")]
const GLASS_MORPH_FADE_FRACTION: f64 = 2.0 / 3.0;
#[cfg(target_os = "macos")]
const GLASS_MORPH_MIN_FADE_DURATION: f64 = 0.05;
/// Alpha target for the phase-one ramp. Spotlight's fade completes (≥0.99)
/// at almost exactly the moment width bottoms out — the window is never
/// fully opaque while wider than its natural size. The final 0.99 → 1.0 leg
/// runs from rebound start.
#[cfg(target_os = "macos")]
const GLASS_MORPH_PHASE1_ALPHA_TARGET: f64 = 0.99;
/// Duration of the 0.85 → 0.99 alpha ramp (ease-out), phase-locked so alpha
/// is at 0.99 well before max compression. Clamped to phase one for short
/// custom durations.
#[cfg(target_os = "macos")]
const GLASS_MORPH_ALPHA_RAMP_DURATION: f64 = 0.018;
/// Duration of the 0.99 → 1.0 finishing leg (ease-out) starting at rebound.
/// Clamped to phase two for short custom durations.
#[cfg(target_os = "macos")]
const GLASS_MORPH_ALPHA_FINISH_DURATION: f64 = 0.026;
/// Material-onset prefix (glass-entry-onset-v2, measured from the
/// 2026-07-27 Spotlight footage): Spotlight materializes presence
/// 0.04→0.86 over its prefix BEFORE the visible geometry tail (44ms
/// here — Spotlight's ~88ms halved by the 2026-07-27 2x request). Script Kit
/// reproduces it as a glass material ramp (Clear→Regular + tint) at a
/// constant 0.85 NSWindow alpha — never sub-0.85 window alpha.
#[cfg(target_os = "macos")]
const GLASS_MATERIAL_ONSET_DURATION: f64 = 0.044;
/// The main window's FIRST PHOTON width scale (2026-08-13 soft-materialize
/// retune, measured from the user-supplied 57fps Spotlight reference:
/// first photon 1.0305 of settled width).
#[cfg(target_os = "macos")]
const GLASS_MAIN_ONSET_START_WIDTH_SCALE: f64 = 1.0305;
/// How long the first-photon frame eases into the calibrated 101.2%
/// visible-tail start (reference: converged by ~35ms at 1:1; 18ms at the
/// authorized 2x tempo, matching the content-fade cadence).
#[cfg(target_os = "macos")]
const GLASS_MAIN_ONSET_GEOMETRY_DURATION: f64 = 0.018;
/// GPUI content roots fade in WITH the material from the first photon
/// (2026-08-13 content-timing retune: the user-supplied 57fps Spotlight
/// reference shows the bar's content faintly present from the very first
/// frame, and the previous 26ms hold produced readable empty-body frames
/// once the native footer stopped enrolling in the content fade).
#[cfg(target_os = "macos")]
const GLASS_ENTRY_CONTENT_HOLD_DURATION: f64 = 0.0;
/// Content fade spans the full material prefix, finishing exactly when the
/// tail begins (clamped to onset − hold in glass_entry_content_fade_duration).
#[cfg(target_os = "macos")]
const GLASS_ENTRY_CONTENT_FADE_DURATION: f64 = 0.044;
/// Onset timing curve reproducing the measured normalized presence samples
/// (~0.294/0.535/0.761/1.0, sampled proportionally across the onset).
#[cfg(target_os = "macos")]
#[allow(dead_code)] // consumed by the onset animator (plan steps 3–6)
const GLASS_MATERIAL_ONSET_C1: (f32, f32) = (0.18, 0.00);
#[cfg(target_os = "macos")]
#[allow(dead_code)]
const GLASS_MATERIAL_ONSET_C2: (f32, f32) = (0.14, 0.00);
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

/// Entry defocus radius, in points: the window materializes OUT of a blur.
///
/// Why this exists (user report 2026-07-27: "we're missing the type of
/// fade-in/blur that spotlight does"). Spotlight's entry begins well before
/// our first visible frame, fading up from near-nothing. We deliberately
/// omitted that prefix — see the entry-alpha lock — because `NSWindow`
/// `alphaValue` multiplies EVERY contributed pixel, so a low-alpha visible
/// frame shows wallpaper rather than Spotlight's coherent faint glass. The
/// result is an entry that has Spotlight's geometry but almost none of its
/// materialization: alpha only ever travels 0.85 -> 1.0.
///
/// A layer blur is the missing primitive, because it is a WITHIN-window
/// effect: it defocuses the window's own pixels without letting the desktop
/// through. That buys the "resolving into being" read that the alpha floor
/// forbids, while leaving every locked geometry, timing, and alpha value
/// untouched.
///
/// Mirrors `GLASS_EXIT_BLUR_RADIUS` so entry and exit are inverses.
#[cfg(target_os = "macos")]
const GLASS_ENTRY_BLUR_RADIUS: f64 = 8.0;

/// Live override for the entry defocus radius, in points, so the radius can be
/// judged by feel without a rebuild:
/// `SCRIPT_KIT_GLASS_ENTRY_BLUR=16 script-kit-gpui`. `0` disables the defocus
/// and restores the exact pre-2026-07-27 entry.
///
/// Deliberately an env override rather than a theme slider: this is an open
/// perceptual question, not a settled product value, and it must not enter the
/// calibration fixture until the user picks a radius.
#[cfg(target_os = "macos")]
fn glass_entry_blur_radius() -> f64 {
    std::env::var("SCRIPT_KIT_GLASS_ENTRY_BLUR")
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 64.0)
        .unwrap_or(GLASS_ENTRY_BLUR_RADIUS)
}

/// The main window's onset defocus radius (2026-08-13 soft-materialize
/// retune): stronger than the shared 8pt so the first photon reads soft, and
/// resolved inside the 44ms material prefix rather than the full entry.
#[cfg(target_os = "macos")]
const GLASS_MAIN_ENTRY_BLUR_RADIUS: f64 = 12.0;

/// Same live override contract as `glass_entry_blur_radius`, scoped to the
/// main window's onset defocus.
#[cfg(target_os = "macos")]
fn glass_main_entry_blur_radius() -> f64 {
    std::env::var("SCRIPT_KIT_GLASS_MAIN_ENTRY_BLUR")
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 64.0)
        .unwrap_or(GLASS_MAIN_ENTRY_BLUR_RADIUS)
}

/// Effective GPUI content fade duration for the entry.
///
/// THE DEFAULT IS THE THING THAT MAKES THE ENTRY FEEL UNFADED. The stock value
/// is `GLASS_ENTRY_CONTENT_FADE_DURATION` clamped so the fade ENDS at the
/// material-onset boundary: `0.044.min(0.044 - 0.0)` = 44ms. The content is
/// therefore fully opaque at t=44ms, before the 105ms geometry tail begins — so
/// across the whole visible tail nothing is fading, and the entry reads as a
/// solid panel doing a 1.2% width wiggle rather than something materializing.
///
/// `SCRIPT_KIT_GLASS_CONTENT_FADE=<ms>` overrides it and is INTENTIONALLY
/// UNCLAMPED, so the fade may run concurrently with the geometry tail (the
/// shape Spotlight actually has). Try 90 or 105 to make the fade span the tail.
/// Total entry duration is unchanged either way — this overlaps existing time
/// rather than adding any, so it does not fight the authorized 2x tempo.
#[cfg(target_os = "macos")]
fn glass_entry_content_fade_duration() -> f64 {
    if let Some(seconds) = std::env::var("SCRIPT_KIT_GLASS_CONTENT_FADE")
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 2000.0)
        .map(|ms| ms / 1000.0)
    {
        return seconds;
    }
    GLASS_ENTRY_CONTENT_FADE_DURATION
        .min(GLASS_MATERIAL_ONSET_DURATION - GLASS_ENTRY_CONTENT_HOLD_DURATION)
}

/// First-photon content presence floor (2026-08-13 empty-window retune,
/// user report: "the main window starts with an 'empty' window").
///
/// The 57fps Spotlight reference measures the bar's content at ~21% of its
/// settled presence in the very first visible frame — content is never
/// truly absent, it blooms in WITH the material. Seeding the GPUI content
/// roots at 0.0 reproduced an "empty stage" first photon instead; this floor
/// starts every held content root at `0.21 × its natural alpha` so the body
/// is faintly readable from photon 1, then the existing 44ms fade carries it
/// to full presence.
#[cfg(target_os = "macos")]
const GLASS_ENTRY_CONTENT_START_ALPHA: f64 = 0.21;

/// Same live override contract as `glass_entry_content_fade_duration`,
/// scoped to the content presence floor: `SCRIPT_KIT_GLASS_CONTENT_START=0.4`
/// (fraction of natural alpha, 0 restores the pre-2026-08-13 empty seed).
#[cfg(target_os = "macos")]
fn glass_entry_content_start_alpha() -> f64 {
    std::env::var("SCRIPT_KIT_GLASS_CONTENT_START")
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        .unwrap_or(GLASS_ENTRY_CONTENT_START_ALPHA)
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlassExitMode {
    DetachedRegionsFadeOnly,
    PopupTransformAndBlur,
}

#[cfg(target_os = "macos")]
fn glass_exit_mode(window_name: &str) -> GlassExitMode {
    if window_name_uses_fixed_frame_exit(window_name) {
        GlassExitMode::DetachedRegionsFadeOnly
    } else {
        GlassExitMode::PopupTransformAndBlur
    }
}

pub fn glass_exit_remove_delay() -> std::time::Duration {
    std::time::Duration::from_millis(GLASS_EXIT_REMOVE_DELAY_MS)
}

#[allow(
    dead_code,
    reason = "the calibrated native-glass compatibility delay is retained without changing its locked timing"
)]
pub(crate) fn glass_entry_settle_delay() -> std::time::Duration {
    let tail = crate::theme::get_cached_theme()
        .get_opacity()
        .glass_morph_duration
        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
        .clamp(0.0, GLASS_MORPH_MAX_DURATION as f32);
    // Total entry = material onset prefix + visible tail (298ms default).
    std::time::Duration::from_secs_f64(f64::from(tail) + GLASS_MATERIAL_ONSET_DURATION)
}

include!("secondary_window_glass_lifecycle.rs");
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
    let phase2 =
        (duration - phase1 - GLASS_MORPH_SQUISH_HOLD).max(GLASS_MORPH_MIN_REBOUND_DURATION);
    Some(GlassMorphTuning {
        duration,
        inset_fraction,
        start_alpha: GLASS_MORPH_ENTRY_START_ALPHA,
        start_scale_x: 1.0 + inset_fraction * 2.0,
        start_scale_y: 1.0 + inset_fraction * GLASS_MORPH_VERTICAL_DAMPING * 2.0,
        squish_scale_x: 1.0 - squish_fraction * 2.0,
        squish_scale_y: 1.0 - squish_fraction * GLASS_MORPH_VERTICAL_DAMPING * 2.0,
        phase1,
        phase2,
        phase1_alpha_target: GLASS_MORPH_PHASE1_ALPHA_TARGET,
        alpha_ramp_duration: GLASS_MORPH_ALPHA_RAMP_DURATION.min(phase1),
        alpha_finish_duration: GLASS_MORPH_ALPHA_FINISH_DURATION.min(phase2),
        material_onset_duration: GLASS_MATERIAL_ONSET_DURATION,
        content_hold_duration: GLASS_ENTRY_CONTENT_HOLD_DURATION,
        content_fade_duration: glass_entry_content_fade_duration(),
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
    configure_window_vibrancy_common_impl(window, log_target, window_name, is_dark, morph_variant)
}

include!("secondary_window_vibrancy_impl.rs");

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

include!("secondary_window_glass_backdrop.rs");
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

include!("secondary_window_glass_animation.rs");
include!("secondary_window_glass_style.rs");
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
    let mut backdrop_layout = tahoe_backdrop_layout(window_name);
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
        // Appearance/theme refresh over an existing Notes backdrop must
        // preserve the dynamically selected inset (Agent mode reserves the
        // footer band via `set_gpui_window_backdrop_bottom_inset`); resetting
        // to the static default would silently collapse the Agent partition.
        if window_name == "Notes" {
            let existing_inset: f64 = *(*(glass_view as *const objc::runtime::Object))
                .get_ivar::<f64>("_scriptKitBottomInset");
            backdrop_layout = if existing_inset > 0.0 {
                TahoeBackdropLayout::ContentAboveDetachedFooter {
                    bottom_inset: existing_inset,
                }
            } else {
                TahoeBackdropLayout::FullWindow
            };
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
    // A reconfigure pass over an existing backdrop is a theme/appearance
    // refresh, not a new installation.
    let tint_applied = apply_native_glass_style_with_reason(
        glass_view,
        style,
        if created {
            NativeGlassStyleApplicationReason::Install
        } else {
            NativeGlassStyleApplicationReason::ThemeRefresh
        },
    );

    let corner_radius =
        tahoe_backdrop_corner_radius_for(window_name, backdrop_layout, content_view);
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

/// Configure the Actions popup without activating its parent application.
///
/// # Safety
/// `window` must be a valid, live `NSWindow` pointer, and this function must
/// be called on the AppKit main thread while the window remains owned.
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
///
/// # Safety
/// `window` must be a valid, live `NSWindow` pointer, and this function must
/// be called on the AppKit main thread while the window remains owned.
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
    include!("secondary_window_config_behavior_tests.rs");

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

include!("secondary_window_resize_policy.rs");

// Re-export display/coordinate helpers from the unified display module.
pub use self::display::{
    clamp_to_visible, display_for_point, flip_y, get_active_display, get_global_mouse_position,
    get_macos_displays, get_macos_visible_displays, prefers_reduced_motion, primary_screen_height,
    VisibleDisplayBounds,
};

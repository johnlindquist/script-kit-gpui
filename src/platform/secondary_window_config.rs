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
    enroll_in_content_fade: bool,
    defocus_scope: MainFooterEntryDefocusScope,
    /// Radius applied independently to every clipped NSGlassEffectView
    /// capsule; never apply this value to the footer container or hints host.
    defocus_radius: f64,
}

#[cfg(target_os = "macos")]
fn main_footer_entry_material_policy() -> MainFooterEntryMaterialPolicy {
    MainFooterEntryMaterialPolicy {
        target_alpha: 1.0,
        enroll_in_content_fade: false,
        defocus_scope: MainFooterEntryDefocusScope::PerCapsule,
        defocus_radius: glass_main_entry_blur_radius(),
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

pub(crate) fn glass_entry_settle_delay() -> std::time::Duration {
    let tail = crate::theme::get_cached_theme()
        .get_opacity()
        .glass_morph_duration
        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
        .clamp(0.0, GLASS_MORPH_MAX_DURATION as f32);
    // Total entry = material onset prefix + visible tail (298ms default).
    std::time::Duration::from_secs_f64(f64::from(tail) + GLASS_MATERIAL_ONSET_DURATION)
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
                animate_tahoe_glass_window_frame_appearance(window, log_target, window_name)
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
        morph_start_alpha_bits: if glass_created {
            morph_tuning.map(|tuning| tuning.start_alpha.to_bits())
        } else {
            None
        },
        settle_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.total_entry_duration() * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        material_onset_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| tuning.visible_tail_start_delay_ms())
                .unwrap_or(0)
        } else {
            0
        },
        visible_tail_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.visible_tail_duration() * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        content_hold_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.content_hold_duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        content_fade_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.content_fade_duration * 1000.0).round() as u64)
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
        content_reveal_delay_ms: if glass_created {
            morph_tuning.map(entry_content_reveal_delay_ms).unwrap_or(0)
        } else {
            0
        },
        phase_one_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.phase1 * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        phase_two_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.phase2 * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        alpha_ramp_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.alpha_ramp_duration * 1000.0).round() as u64)
                .unwrap_or(0)
        } else {
            0
        },
        alpha_finish_duration_ms: if glass_created {
            morph_tuning
                .map(|tuning| (tuning.alpha_finish_duration * 1000.0).round() as u64)
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

#[cfg(target_os = "macos")]
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

/// Per-window two-phase bounce settle targets. Multiple HUDs and secondary
/// windows can materialize concurrently, so a single global slot would let one
/// window steal another's rebound target.
#[cfg(target_os = "macos")]
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
static GLASS_ENTRY_CONTENT_TARGETS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, Vec<(usize, f64)>>>,
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

    // Hold GPUI content roots (never the contentView or the glass itself)
    // at alpha 0 until T=53ms; transparent GPUI pixels expose the glass.
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
            let _: () = msg_send![child, setAlphaValue: 0.0f64];
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

    // Footer onset parity is PER CAPSULE. Each NSGlassEffectView layer clips
    // its own 12pt -> 0 ramp to its rounded bounds, so transparent gaps and
    // desktop seams never enter the filter sample. A partially installed ramp
    // reports radius 0 and mismatched counts, making the receipt fail closed.
    let footer_capsules = if surface == GlassEntrySurface::Main {
        crate::footer_popup::main_window_footer_entry_capsules(window)
    } else {
        Vec::new()
    };
    let footer_blur_duration = tuning.material_onset_duration;
    let footer_capsule_count = footer_capsules.len();
    let mut footer_blurred_capsule_count = 0usize;
    if footer_entry_policy.defocus_radius > 0.0 {
        for capsule in footer_capsules {
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
    }
    let footer_blur_radius =
        if footer_capsule_count > 0 && footer_blurred_capsule_count == footer_capsule_count {
            footer_entry_policy.defocus_radius
        } else {
            0.0
        };
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
            "event=native_glass_entry_onset primitive=material_parameters supported={} entry_blur_radius={:.2} entry_blur_to_radius=0.00 footer_blur_radius={:.2} footer_blur_to_radius=0.00 footer_blur_scope={} footer_blur_duration_ns={} footer_capsule_count={} footer_blurred_capsule_count={} footer_enrolled={} entry_blur_duration_ns={} onset_start_width_scale={:.6} tail_start_width_scale={:.6} onset_geometry_duration_ns={} from_style=clear to_style=regular duration_ns={} content_root_count={} content_hold_ns={} content_fade_ns={} window_alpha={:.2}",
            onset_supported,
            entry_blur_radius,
            footer_blur_radius,
            footer_entry_policy.defocus_scope.log_name(),
            (footer_blur_duration * 1_000_000_000.0).round() as u64,
            footer_capsule_count,
            footer_blurred_capsule_count,
            footer_entry_policy.enroll_in_content_fade,
            (entry_blur_duration * 1_000_000_000.0).round() as u64,
            onset_start.size.width / final_frame.size.width,
            start.size.width / final_frame.size.width,
            (onset_geometry_duration * 1_000_000_000.0).round() as u64,
            (tuning.material_onset_duration * 1_000_000_000.0).round() as u64,
            content_root_count,
            (tuning.content_hold_duration * 1_000_000_000.0).round() as u64,
            (tuning.content_fade_duration * 1_000_000_000.0).round() as u64,
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

/// Why a native glass style application happened. The material contract
/// allows exactly two temporal shapes: the initial installation of a surface
/// and an explicitly recorded theme refresh. Anything else that lands between
/// morph start and settle is a per-frame material mutation — the exact class
/// of change the Glass Motion Calibration Lock forbids (tint RGB, tint alpha,
/// veil alpha, and native layer opacity must be static during entry).
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeGlassStyleApplicationReason {
    Install,
    ThemeRefresh,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct NativeGlassStyleApplication {
    window_number: i64,
    surface_id: usize,
    at_ns: u64,
    reason: NativeGlassStyleApplicationReason,
    signature: NativeGlassStyleSignature,
}

/// Pure, testable record of entry spans and style applications so the runtime
/// can prove `styleMutationCountDuringEntry == 0` instead of asserting it
/// from source reading.
#[cfg(target_os = "macos")]
#[derive(Default)]
struct NativeGlassStyleLedger {
    /// `(window_number, morph_start_ns, settle_end_ns)`; one live span per
    /// window (re-entry replaces the previous span).
    entry_spans: Vec<(i64, u64, u64)>,
    applications: Vec<NativeGlassStyleApplication>,
}

#[cfg(target_os = "macos")]
const NATIVE_GLASS_STYLE_LEDGER_CAPACITY: usize = 512;

#[cfg(target_os = "macos")]
impl NativeGlassStyleLedger {
    fn record_entry_span(&mut self, window_number: i64, start_ns: u64, end_ns: u64) {
        self.entry_spans
            .retain(|(window, _, _)| *window != window_number);
        self.entry_spans.push((window_number, start_ns, end_ns));
        if self.entry_spans.len() > NATIVE_GLASS_STYLE_LEDGER_CAPACITY {
            self.entry_spans.remove(0);
        }
    }

    fn entry_span(&self, window_number: i64) -> Option<(u64, u64)> {
        self.entry_spans
            .iter()
            .find(|(window, _, _)| *window == window_number)
            .map(|(_, start, end)| (*start, *end))
    }

    /// Whether an identical `Install` has already styled this exact native
    /// surface during the active entry span. Reapplying the same signature can
    /// still churn NSGlassEffectView's private material tree, so callers skip
    /// it until the entry settles; distinct capsules in the same window remain
    /// independent surfaces and must each receive their initial style.
    fn has_identical_surface_style_during_entry(
        &self,
        window_number: i64,
        surface_id: usize,
        at_ns: u64,
        signature: NativeGlassStyleSignature,
    ) -> bool {
        let in_span = self
            .entry_span(window_number)
            .is_some_and(|(start, end)| at_ns >= start && at_ns <= end);
        in_span
            && self.applications.iter().rev().any(|prior| {
                prior.window_number == window_number
                    && prior.surface_id == surface_id
                    && prior.signature == signature
            })
    }

    /// Record one application; returns `true` when it is a forbidden
    /// mid-entry mutation: an `Install`-shaped (re)application inside the
    /// window's morph span with any earlier application for the same native
    /// surface. The initial installation of each distinct surface and
    /// explicitly tagged theme refreshes are the only allowed in-span shapes.
    fn record_application(&mut self, application: NativeGlassStyleApplication) -> bool {
        let in_span = self
            .entry_span(application.window_number)
            .is_some_and(|(start, end)| application.at_ns >= start && application.at_ns <= end);
        let has_prior = self.applications.iter().any(|prior| {
            prior.window_number == application.window_number
                && prior.surface_id == application.surface_id
        });
        let mutation = in_span
            && has_prior
            && application.reason == NativeGlassStyleApplicationReason::Install;
        self.applications.push(application);
        if self.applications.len() > NATIVE_GLASS_STYLE_LEDGER_CAPACITY {
            self.applications.remove(0);
        }
        mutation
    }

    fn style_mutation_count_during_entry(&self, window_number: i64) -> usize {
        let Some((start, end)) = self.entry_span(window_number) else {
            return 0;
        };
        let mut seen_surfaces: std::collections::HashSet<usize> = self
            .applications
            .iter()
            .filter(|app| app.window_number == window_number && app.at_ns < start)
            .map(|app| app.surface_id)
            .collect();
        let mut count = 0;
        for application in self
            .applications
            .iter()
            .filter(|app| app.window_number == window_number)
            .filter(|app| app.at_ns >= start && app.at_ns <= end)
        {
            if seen_surfaces.contains(&application.surface_id)
                && application.reason == NativeGlassStyleApplicationReason::Install
            {
                count += 1;
            }
            seen_surfaces.insert(application.surface_id);
        }
        count
    }
}

#[cfg(target_os = "macos")]
static NATIVE_GLASS_STYLE_LEDGER: std::sync::Mutex<Option<NativeGlassStyleLedger>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
fn with_native_glass_style_ledger<T>(
    operation: impl FnOnce(&mut NativeGlassStyleLedger) -> T,
) -> T {
    let mut guard = NATIVE_GLASS_STYLE_LEDGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    operation(guard.get_or_insert_with(NativeGlassStyleLedger::default))
}

/// Record the morph span so any style application landing inside it can be
/// classified. Called at morph start by every entry variant.
#[cfg(target_os = "macos")]
unsafe fn record_native_glass_entry_span(window: id, duration_seconds: f64) {
    if window == nil {
        return;
    }
    let window_number: i64 = msg_send![window, windowNumber];
    if window_number <= 0 {
        return;
    }
    let start_ns = crate::platform::host_clock::host_time_ns();
    let end_ns = start_ns.saturating_add((duration_seconds.max(0.0) * 1e9) as u64);
    with_native_glass_style_ledger(|ledger| {
        ledger.record_entry_span(window_number, start_ns, end_ns)
    });
}

/// Runtime count of forbidden mid-entry style mutations for a window. A
/// healthy entry reports 0.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub(crate) fn native_glass_style_mutation_count_during_entry(window_number: i64) -> usize {
    with_native_glass_style_ledger(|ledger| ledger.style_mutation_count_during_entry(window_number))
}

/// Apply the complete shared native glass policy. AppKit mutations are made
/// in one disabled-actions transaction so a theme refresh cannot expose an
/// intermediate untinted or mismatched frame.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn apply_native_glass_style(glass_view: id, style: NativeGlassStyle) -> bool {
    apply_native_glass_style_with_reason(
        glass_view,
        style,
        NativeGlassStyleApplicationReason::Install,
    )
}

/// See [`apply_native_glass_style`]; `reason` feeds the style-application
/// ledger that proves the material stack stays static during entry.
///
/// # Safety
/// Same contract as [`apply_native_glass_style`].
#[cfg(target_os = "macos")]
pub(crate) unsafe fn apply_native_glass_style_with_reason(
    glass_view: id,
    style: NativeGlassStyle,
    reason: NativeGlassStyleApplicationReason,
) -> bool {
    if glass_view == nil {
        return false;
    }
    let responds: bool = msg_send![glass_view, respondsToSelector: sel!(setTintColor:)];
    if !responds {
        return false;
    }
    let window: id = msg_send![glass_view, window];
    let window_number: i64 = if window != nil {
        msg_send![window, windowNumber]
    } else {
        -1
    };
    let surface_id = glass_view as usize;
    let at_ns = crate::platform::host_clock::host_time_ns();
    let skip_identical_install = reason == NativeGlassStyleApplicationReason::Install
        && with_native_glass_style_ledger(|ledger| {
            ledger.has_identical_surface_style_during_entry(
                window_number,
                surface_id,
                at_ns,
                style.signature,
            )
        });
    if skip_identical_install {
        tracing::debug!(
            target: "script_kit::native_glass",
            event = "native_glass_style_identical_install_skipped_during_entry",
            window_number,
            surface_id,
            at_ns,
            "native_glass_style_identical_install_skipped_during_entry"
        );
        return true;
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
            let _: () = msg_send![content_layer, setBorderWidth: f64::from(style.rim_width)];
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
    let mid_entry_mutation = with_native_glass_style_ledger(|ledger| {
        ledger.record_application(NativeGlassStyleApplication {
            window_number,
            surface_id,
            at_ns,
            reason,
            signature: style.signature,
        })
    });
    let role_name = match style.role {
        NativeGlassSurfaceRole::WindowBackdrop => "window_backdrop",
        NativeGlassSurfaceRole::FloatingCapsule => "floating_capsule",
    };
    tracing::info!(
        target: "script_kit::native_glass",
        event = "native_glass_style_applied",
        window_number,
        surface_id,
        at_ns,
        role = role_name,
        reason = match reason {
            NativeGlassStyleApplicationReason::Install => "install",
            NativeGlassStyleApplicationReason::ThemeRefresh => "theme_refresh",
        },
        material = current_window_material_name(current_window_material()),
        dark = style.signature.dark,
        tint_rgb = style.signature.tint_rgb,
        requested_tint_alpha_bits = ?style.signature.requested_tint_alpha_bits,
        effective_tint_alpha_bits = style.signature.effective_tint_alpha_bits,
        effective_tint_alpha = style.effective_tint_alpha,
        veil_alpha_bits = style.signature.veil_alpha_bits,
        veil_alpha = style.veil_alpha,
        rim_rgba = style.signature.rim_rgba,
        rim_width_bits = style.signature.rim_width_bits,
        "native_glass_style_applied"
    );
    if mid_entry_mutation {
        // The material stack must be static between morph start and settle.
        // This is the runtime tripwire the probes assert against: a healthy
        // entry emits zero of these events.
        tracing::error!(
            target: "script_kit::native_glass",
            event = "native_glass_style_mutation_during_entry",
            window_number,
            surface_id,
            at_ns,
            role = role_name,
            "native_glass_style_mutation_during_entry"
        );
    }
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
    apply_native_glass_style_with_reason(
        glass_view,
        resolve_native_glass_style(&theme, NativeGlassSurfaceRole::WindowBackdrop),
        NativeGlassStyleApplicationReason::ThemeRefresh,
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
        // Visible-tail calibration (2026-07-26, Oracle session
        // glass-entry-spotlight-retune): the first safe frame is
        // phase-aligned to Spotlight's measured t≈88ms state — 101.2% width
        // at the 0.85 alpha floor — with a 70ms ease-out compression, no
        // hold, and a 140ms ease-in-out rebound. Height participation is 0.
        let tuning = super::glass_morph_tuning_from(0.105, 0.006).expect("morph enabled");
        let epsilon = 1e-12;
        assert!((tuning.start_scale_x - 1.012).abs() < epsilon);
        assert!((tuning.start_scale_y - 1.0).abs() < epsilon);
        // Visible entry starts at 0.85 — the wallpaper-bleed floor (HITL
        // submission 98cab5e5-6f15-4311-8d49-83e31602e641): below it the
        // compositor shows desktop pixels, not faint glass.
        assert!((tuning.start_alpha - 0.85).abs() < epsilon);
        assert!((tuning.squish_scale_x - 0.987).abs() < epsilon);
        assert!((tuning.squish_scale_y - 1.0).abs() < epsilon);
        assert!((tuning.phase1 - 0.035).abs() < epsilon);
        assert!((tuning.phase2 - 0.07).abs() < epsilon);
        assert!((tuning.phase1_alpha_target - 0.99).abs() < epsilon);
        assert!((tuning.alpha_ramp_duration - 0.018).abs() < epsilon);
        assert!((tuning.alpha_finish_duration - 0.026).abs() < epsilon);
        // Material onset prefix (glass-entry-onset-v2): 88ms Clear→Regular
        // ramp before the unchanged 210ms tail — 298ms total, matching the
        // measured Spotlight first-photon→settled span. Content holds 53ms
        // then fades 35ms, ending exactly at tail start.
        assert!((tuning.material_onset_duration - 0.044).abs() < epsilon);
        assert!((tuning.content_hold_duration - 0.026).abs() < epsilon);
        assert!((tuning.content_fade_duration - 0.018).abs() < epsilon);
        assert!((tuning.visible_tail_duration() - 0.105).abs() < epsilon);
        assert!((tuning.total_entry_duration() - 0.149).abs() < epsilon);
        assert_eq!(tuning.visible_tail_start_delay_ms(), 44);
        // Geometry crossing is TAIL-relative (23ms after tail start = 111ms
        // absolute); the reveal anchor is ABSOLUTE from configure:
        // 88 + max(23, 35) = 123ms.
        assert_eq!(super::settled_size_crossing_delay_ms(tuning), 11);
        assert_eq!(super::entry_content_reveal_delay_ms(tuning), 62);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn main_footer_entry_inherits_window_material_without_independent_effects() {
        let policy = super::main_footer_entry_material_policy();
        assert_eq!(policy.target_alpha, 1.0);
        assert!(!policy.enroll_in_content_fade);
        assert_eq!(
            policy.defocus_scope,
            super::MainFooterEntryDefocusScope::PerCapsule
        );
        assert_eq!(policy.defocus_scope.log_name(), "per_capsule");
        assert_eq!(policy.defocus_radius, super::glass_main_entry_blur_radius());
        assert_eq!(super::GLASS_MAIN_ENTRY_BLUR_RADIUS, 12.0);
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
        // Visible-tail entry calibration (retuned 2026-07-26, Oracle session
        // glass-entry-spotlight-retune, user-authorized): the first safe
        // frame is phase-aligned to Spotlight's measured t≈88ms state (see
        // https://eager-hollow-dyyf.here.now/). Start alpha stays at the
        // 0.85 wallpaper-bleed floor (HITL 98cab5e5-6f15-4311-8d49-
        // 83e31602e641); it ramps to 0.99 over 35ms and finishes to 1.0
        // over 52ms from rebound start — never fully opaque while wider
        // than natural size. Hidden parking keeps 0.0.
        assert_eq!(super::GLASS_MORPH_ENTRY_START_ALPHA, 0.85);
        assert_eq!(super::GLASS_HIDDEN_PARK_ALPHA, 0.0);
        assert_eq!(super::GLASS_MORPH_VERTICAL_DAMPING, 0.0);
        assert_eq!(super::GLASS_MORPH_SQUISH_FACTOR, 0.25);
        assert_eq!(super::GLASS_MORPH_MIN_SQUISH, 0.0065);
        assert_eq!(super::GLASS_MORPH_MAX_SQUISH, 0.015);
        assert_eq!(super::GLASS_MORPH_SQUISH_HOLD, 0.0);
        assert_eq!(super::GLASS_MORPH_PHASE1_FRACTION, 1.0 / 3.0);
        assert_eq!(super::GLASS_MORPH_PHASE1_ALPHA_TARGET, 0.99);
        assert_eq!(super::GLASS_MORPH_ALPHA_RAMP_DURATION, 0.018);
        assert_eq!(super::GLASS_MORPH_ALPHA_FINISH_DURATION, 0.026);
        assert_eq!(super::GLASS_MORPH_FADE_FRACTION, 2.0 / 3.0);
        assert_eq!(super::GLASS_MATERIAL_ONSET_DURATION, 0.044);
        assert_eq!(super::GLASS_MAIN_ONSET_START_WIDTH_SCALE, 1.0305);
        assert_eq!(super::GLASS_MAIN_ONSET_GEOMETRY_DURATION, 0.018);
        assert_eq!(super::GLASS_ENTRY_CONTENT_HOLD_DURATION, 0.0);
        assert_eq!(super::GLASS_ENTRY_CONTENT_FADE_DURATION, 0.044);
        assert_eq!(super::GLASS_ENTRY_BLUR_RADIUS, 8.0);
        assert_eq!(super::GLASS_MAIN_ENTRY_BLUR_RADIUS, 12.0);
        let tuning =
            super::glass_morph_tuning_from(duration, inset).expect("fixture enables glass morph");
        assert_eq!(super::settled_size_crossing_delay_ms(tuning), 11);
        assert_eq!(super::entry_content_reveal_delay_ms(tuning), 62);
        assert_eq!(tuning.visible_tail_start_delay_ms(), 44);
        assert!((tuning.total_entry_duration() - 0.149).abs() < 1e-9);
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
        assert!(super::glass_morph_tuning_from(0.105, 0.0).is_none());
    }

    // ── Entry surface policy (Oracle `glass-entry-feel-options` WP1) ────────
    //
    // These four lock the BASELINE that the typed-policy refactor had to
    // preserve. They are ordinary behavior tests on pure functions, not source
    // audits: they assert what the geometry resolves to, so a future retune
    // fails them loudly with the actual numbers rather than a string diff.
    //
    // WP2 will deliberately flip `Main` to grow-in. When it does, the two
    // `*_before_the_retune` tests are the ones that must be UPDATED (with the
    // measured evidence in the commit), while the two `fractional_*_profile_*`
    // tests must keep passing untouched — they pin the shape of each
    // direction, not which surface uses it.

    /// The shrink-in profile is Spotlight's outset enter: the first visible
    /// frame is 1.2% wider than settled, and phase one compresses to 1.3%
    /// narrower. At the default inset the squish lands on its 0.0065 floor.
    #[cfg(target_os = "macos")]
    #[test]
    fn fractional_shrink_profile_reproduces_1_012_to_0_987() {
        let tuning = super::glass_morph_tuning_from(
            f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION),
            f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET),
        )
        .expect("default sliders enable the morph");
        let sign = super::GlassEntryDirection::ShrinkIn.geometry_sign();
        assert_eq!(sign, 1.0);

        let final_width = 750.0_f64;
        let travel = super::glass_entry_travel(
            super::GlassEntryTravelPolicy::Fractional,
            final_width,
            tuning.inset_fraction,
            super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
        );
        // Width path, expressed the way the motion contract states it.
        let start_scale = 1.0 + (travel.start_per_side * 2.0 / final_width) * sign;
        let extreme_scale = 1.0 - (travel.extreme_per_side * 2.0 / final_width) * sign;
        assert!(
            (start_scale - 1.012).abs() < 1e-9,
            "shrink-in must start at 101.2% width, got {start_scale}"
        );
        assert!(
            (extreme_scale - 0.987).abs() < 1e-9,
            "shrink-in must compress to 98.7% width, got {extreme_scale}"
        );
        // Squish floor: 0.006 × 0.25 = 0.0015 clamps up to 0.0065 per side.
        assert!(
            (travel.extreme_per_side / final_width - super::GLASS_MORPH_MIN_SQUISH).abs() < 1e-9
        );
    }

    /// Grow-in is the same magnitudes with inverted travel: start 1.2% NARROWER
    /// and overshoot 1.3% wider before settling.
    #[cfg(target_os = "macos")]
    #[test]
    fn fractional_grow_profile_reproduces_0_988_to_1_013() {
        let tuning = super::glass_morph_tuning_from(
            f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION),
            f64::from(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET),
        )
        .expect("default sliders enable the morph");
        let sign = super::GlassEntryDirection::GrowIn.geometry_sign();
        assert_eq!(sign, -1.0);

        let final_width = 340.0_f64;
        let travel = super::glass_entry_travel(
            super::GlassEntryTravelPolicy::Fractional,
            final_width,
            tuning.inset_fraction,
            super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
        );
        let start_scale = 1.0 + (travel.start_per_side * 2.0 / final_width) * sign;
        let extreme_scale = 1.0 - (travel.extreme_per_side * 2.0 / final_width) * sign;
        assert!(
            (start_scale - 0.988).abs() < 1e-9,
            "grow-in must start at 98.8% width, got {start_scale}"
        );
        assert!(
            (extreme_scale - 1.013).abs() < 1e-9,
            "grow-in must overshoot to 101.3% width, got {extreme_scale}"
        );

        // The point-travel asymmetry Oracle identified: the SAME fractional
        // policy moves a 750pt main window ~2.2x further per side than this
        // 340pt popup, over the same 35ms phase one.
        let main_travel = super::glass_entry_travel(
            super::GlassEntryTravelPolicy::Fractional,
            750.0,
            tuning.inset_fraction,
            super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
        );
        let ratio = main_travel.start_per_side / travel.start_per_side;
        assert!(
            (ratio - 750.0 / 340.0).abs() < 1e-9,
            "fractional travel must scale linearly with width, got {ratio}"
        );
        // Capping a 750pt window to this popup's reference makes the point
        // travel identical — the WP2 candidate, proven here before it ships.
        let capped = super::glass_entry_travel(
            super::GlassEntryTravelPolicy::ActionsPointCapped,
            750.0,
            tuning.inset_fraction,
            super::GLASS_ENTRY_ACTIONS_REFERENCE_WIDTH,
        );
        assert!((capped.start_per_side - travel.start_per_side).abs() < 1e-9);
        assert!((capped.extreme_per_side - travel.extreme_per_side).abs() < 1e-9);
    }

    /// 2026-08-13 soft-materialize retune lock: main ALONE owns the wider
    /// first-photon onset geometry, while secondary and child-popup
    /// directions, travel, and material onset remain unchanged.
    #[cfg(target_os = "macos")]
    #[test]
    fn main_owns_the_soft_materialize_prefix_while_secondary_remains_tail_aligned() {
        let main = super::glass_entry_policy(super::GlassEntrySurface::Main);
        assert_eq!(main.direction, super::GlassEntryDirection::ShrinkIn);
        assert_eq!(main.travel, super::GlassEntryTravelPolicy::Fractional);
        assert_eq!(main.onset, super::GlassEntryOnsetPolicy::Full);
        assert_eq!(
            main.onset_geometry,
            super::GlassEntryOnsetGeometry::SpotlightSoftMaterialize
        );
        let secondary = super::glass_entry_policy(super::GlassEntrySurface::FreeStandingSecondary);
        assert_eq!(secondary.direction, super::GlassEntryDirection::ShrinkIn);
        assert_eq!(secondary.travel, super::GlassEntryTravelPolicy::Fractional);
        assert_eq!(secondary.onset, super::GlassEntryOnsetPolicy::Full);
        assert_eq!(
            secondary.onset_geometry,
            super::GlassEntryOnsetGeometry::TailAligned
        );
    }

    /// The Cmd+K Actions popup keeps the grow-in direction the user named as
    /// the reference feel, and stays tail-aligned.
    #[cfg(target_os = "macos")]
    #[test]
    fn child_popup_remains_tail_aligned_grow_in() {
        let child = super::glass_entry_policy(super::GlassEntrySurface::ChildPopup);
        assert_eq!(child.direction, super::GlassEntryDirection::GrowIn);
        assert_eq!(child.travel, super::GlassEntryTravelPolicy::Fractional);
        assert_eq!(child.onset, super::GlassEntryOnsetPolicy::Full);
        assert_eq!(
            child.onset_geometry,
            super::GlassEntryOnsetGeometry::TailAligned
        );
        assert_eq!(child.direction.geometry_sign(), -1.0);
        assert_eq!(child.direction.log_name(), "grow_in");
    }

    /// 2026-08-13 soft-materialize retune lock (measured from the
    /// user-supplied 57fps Spotlight reference): main's first photon is
    /// 103.05% of settled width converging to the preserved 101.2% tail start
    /// over 18ms; its onset defocus is 12pt resolved inside the 44ms material
    /// prefix; the total entry stays 149ms; non-main surfaces stay
    /// tail-aligned with the historical full-entry 8pt ramp.
    #[cfg(target_os = "macos")]
    #[test]
    fn main_entry_restores_the_spotlight_soft_materialize_prefix() {
        let tuning = super::glass_morph_tuning_from(0.105, 0.006).expect("morph enabled");
        let main = super::glass_entry_policy(super::GlassEntrySurface::Main);
        assert_eq!(
            super::glass_entry_surface_for_window_frame(true),
            super::GlassEntrySurface::Main
        );
        assert_eq!(
            super::glass_entry_surface_for_window_frame(false),
            super::GlassEntrySurface::FreeStandingSecondary
        );
        let tail_start_per_side = 750.0 * tuning.inset_fraction;
        let onset_start_per_side = main
            .onset_geometry
            .start_per_side(750.0, tail_start_per_side);
        let onset_start_scale = 1.0 + onset_start_per_side * 2.0 / 750.0;
        let tail_start_scale = 1.0 + tail_start_per_side * 2.0 / 750.0;
        let epsilon = 1e-12;

        assert_eq!(
            main.onset_geometry,
            super::GlassEntryOnsetGeometry::SpotlightSoftMaterialize
        );
        assert!((onset_start_scale - 1.0305).abs() < epsilon);
        assert!((tail_start_scale - 1.012).abs() < epsilon);
        assert!((main.onset_geometry.duration() - 0.018).abs() < epsilon);
        assert_eq!(super::GLASS_MAIN_ENTRY_BLUR_RADIUS, 12.0);
        assert!(
            (super::GlassEntrySurface::Main.entry_blur_duration(tuning) - 0.044).abs() < epsilon
        );
        let footer = super::main_footer_entry_material_policy();
        assert_eq!(
            footer.defocus_scope,
            super::MainFooterEntryDefocusScope::PerCapsule
        );
        assert_eq!(
            footer.defocus_radius,
            super::GlassEntrySurface::Main.entry_blur_radius()
        );
        assert!((tuning.material_onset_duration - 0.044).abs() < epsilon);
        assert!((tuning.total_entry_duration() - 0.149).abs() < epsilon);

        for surface in [
            super::GlassEntrySurface::ChildPopup,
            super::GlassEntrySurface::FreeStandingSecondary,
        ] {
            let policy = super::glass_entry_policy(surface);
            assert_eq!(
                policy.onset_geometry,
                super::GlassEntryOnsetGeometry::TailAligned
            );
            assert_eq!(policy.onset_geometry.duration(), 0.0);
            assert!((surface.entry_blur_duration(tuning) - 0.149).abs() < epsilon);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn detached_main_backdrop_excludes_footer_and_exact_eight_point_gutter() {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(750.0, 501.0));
        let layout = super::TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset: 40.0 };
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

    /// The exit-motion contract and the backdrop partition are now SEPARATE
    /// decisions: Notes keeps the calibrated fixed-frame fade exit in both
    /// surface modes while its DEFAULT backdrop is full-window (its Agent
    /// mode reserves the footer band dynamically through
    /// `set_gpui_window_backdrop_bottom_inset`).
    #[cfg(target_os = "macos")]
    #[test]
    fn notes_defaults_to_full_backdrop() {
        // Default (static) partition owners: Main + Dictation only.
        for window_name in ["Main window", "Dictation overlay"] {
            assert!(
                super::window_name_owns_detached_footer(window_name),
                "{window_name} must own the default floating footer partition"
            );
        }
        assert!(
            !super::window_name_owns_detached_footer("Notes"),
            "Notes must default to the full-window backdrop (Notes mode has no footer)"
        );
        assert!(!super::window_name_owns_detached_footer("Actions popup"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn notes_exit_remains_fixed_frame_fade_only_in_both_modes() {
        // The calibrated exit motion keys off the fixed-frame-exit set, NOT
        // the backdrop layout — Notes stays fade-only with zero inset.
        for window_name in ["Main window", "Notes", "Dictation overlay"] {
            assert!(super::window_name_uses_fixed_frame_exit(window_name));
            assert_eq!(
                super::glass_exit_mode(window_name),
                super::GlassExitMode::DetachedRegionsFadeOnly
            );
        }
        for window_name in ["Actions popup", "Confirm popup", "Inline popup"] {
            assert!(!super::window_name_uses_fixed_frame_exit(window_name));
            assert_eq!(
                super::glass_exit_mode(window_name),
                super::GlassExitMode::PopupTransformAndBlur
            );
        }
        assert_eq!(super::glass_exit_remove_delay().as_millis(), 135);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn notes_backdrop_layout_can_toggle_without_changing_outer_frame() {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        // The same outer content bounds partition into full-window and
        // footer-inset layouts; toggling layouts never changes the outer
        // frame, only the backdrop's share of it.
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(350.0, 280.0));
        let full = super::TahoeBackdropLayout::FullWindow.frame(bounds);
        assert_eq!(full.origin.y, 0.0);
        assert_eq!(full.size.height, 280.0);

        let inset = 44.0;
        let partitioned = super::TahoeBackdropLayout::ContentAboveDetachedFooter {
            bottom_inset: inset,
        }
        .frame(bounds);
        assert_eq!(partitioned.origin.y, inset);
        assert_eq!(partitioned.size.height, 280.0 - inset);
        assert_eq!(partitioned.size.width, full.size.width);

        // Round-trip back to full-window restores the exact original frame.
        let restored = super::TahoeBackdropLayout::FullWindow.frame(bounds);
        assert_eq!(restored.origin.y, full.origin.y);
        assert_eq!(restored.size.height, full.size.height);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_glass_style_locks_tint_floor_and_preserves_requested_tint_semantics() {
        let mut inherited = crate::theme::Theme::default();
        inherited.opacity.as_mut().unwrap().glass_tint_opacity = None;
        let mut explicit_zero = inherited.clone();
        explicit_zero.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.0);
        let mut below_floor = inherited.clone();
        below_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.34);
        let mut at_floor = inherited.clone();
        at_floor.opacity.as_mut().unwrap().glass_tint_opacity = Some(0.35);
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
        // Material/appearance parity: both roles resolve the same appearance
        // mode from the same theme — the capsule may not diverge to its own
        // dark/light decision.
        assert_eq!(backdrop.signature.dark, capsule.signature.dark);
        let light_backdrop = super::resolve_native_glass_style(
            &light,
            super::NativeGlassSurfaceRole::WindowBackdrop,
        );
        assert_eq!(
            light_backdrop.signature.tint_rgb,
            light_capsule.signature.tint_rgb
        );
        assert_eq!(
            light_backdrop.signature.effective_tint_alpha_bits,
            light_capsule.signature.effective_tint_alpha_bits
        );
        assert_eq!(light_backdrop.signature.dark, light_capsule.signature.dark);
        // The native capsule veil resolves through the chrome token. It is
        // 0.0 during the 2026-07-27 user-authorized "perfectly match the main
        // window" experiment, so the capsule material intentionally equals the
        // backdrop material; only the separation rim distinguishes them.
        assert_eq!(
            capsule.veil_alpha,
            crate::ui::chrome::LIQUID_GLASS_CAPSULE_VEIL_ALPHA
        );
        assert_eq!(
            backdrop.effective_tint_alpha,
            crate::ui::chrome::LIQUID_GLASS_STABILITY_TINT_ALPHA_FLOOR
        );
        assert_eq!(backdrop.signature.requested_tint_alpha_bits, None);
        assert_eq!(
            explicit.signature.requested_tint_alpha_bits,
            Some(0.0_f32.to_bits())
        );
        for style in [backdrop, capsule, explicit, below, at] {
            assert_eq!(style.effective_tint_alpha, 0.35);
        }
        assert_eq!(above.effective_tint_alpha, 0.72);
        assert_eq!(backdrop.veil_alpha, 0.0);
        assert_eq!(backdrop.rim_width, 0.0);
        assert_eq!(backdrop.signature.rim_rgba, 0xFFFF_FF00);
        assert_eq!(capsule.veil_alpha, 0.0);
        assert_eq!(capsule.rim_width, 1.0);
        assert_eq!(capsule.signature.rim_rgba, 0xFFFF_FF3D);
        assert_eq!(light_capsule.veil_alpha, 0.0);
        assert_eq!(light_capsule.rim_width, 1.0);
        assert_eq!(light_capsule.signature.rim_rgba, 0x0000_002E);
        assert_ne!(backdrop.signature, explicit.signature);
    }

    /// The material stack must be STATIC between morph start and settle: only
    /// the initial installation and explicitly recorded theme refreshes may
    /// apply native styles. A per-frame tint/veil/opacity animation during
    /// entry would land here as an in-span `Install` re-application — the
    /// ledger flags it so runtime receipts (and probes reading
    /// `native_glass_style_mutation_during_entry` error events) can prove
    /// `styleMutationCountDuringEntry == 0`.
    #[cfg(target_os = "macos")]
    #[test]
    fn native_glass_style_ledger_flags_only_mid_entry_reapplications() {
        let theme = crate::theme::Theme::default();
        let signature = super::resolve_native_glass_style(
            &theme,
            super::NativeGlassSurfaceRole::FloatingCapsule,
        )
        .signature;
        let application =
            |window_number: i64,
             surface_id: usize,
             at_ns: u64,
             reason: super::NativeGlassStyleApplicationReason| {
                super::NativeGlassStyleApplication {
                    window_number,
                    surface_id,
                    at_ns,
                    reason,
                    signature,
                }
            };
        use super::NativeGlassStyleApplicationReason::{Install, ThemeRefresh};

        let mut ledger = super::NativeGlassStyleLedger::default();
        ledger.record_entry_span(7, 1_000, 2_000);
        // Initial install before the morph span: allowed.
        assert!(!ledger.record_application(application(7, 70, 900, Install)));
        // Pooled capsule views style themselves before attaching to a window
        // (window number -1): never counted against a real window's span.
        assert!(!ledger.record_application(application(-1, 71, 1_100, Install)));
        // A distinct capsule in the same window gets its own initial install;
        // it is not a reapplication of surface 70.
        assert!(!ledger.record_application(application(7, 71, 1_100, Install)));
        // Theme refresh mid-span is the explicitly recorded exception.
        assert!(!ledger.record_application(application(7, 70, 1_200, ThemeRefresh)));
        assert_eq!(ledger.style_mutation_count_during_entry(7), 0);
        assert!(ledger.has_identical_surface_style_during_entry(7, 70, 1_300, signature));
        assert!(!ledger.has_identical_surface_style_during_entry(7, 72, 1_300, signature));
        // A tint/veil/opacity animation during entry looks exactly like this:
        // an Install-shaped re-application of the same native surface inside
        // the span. It must be flagged — this is the assertion that fails if
        // anyone temporarily mutates capsule tint during entry.
        assert!(ledger.record_application(application(7, 70, 1_500, Install)));
        assert_eq!(ledger.style_mutation_count_during_entry(7), 1);
        // After settle, re-applications are outside the entry contract.
        assert!(!ledger.record_application(application(7, 70, 2_500, Install)));
        assert_eq!(ledger.style_mutation_count_during_entry(7), 1);
        // A first-ever installation landing inside its own span (window whose
        // backdrop is created mid-morph) is the initial install, not a
        // mutation.
        ledger.record_entry_span(9, 1_000, 2_000);
        assert!(!ledger.record_application(application(9, 90, 1_050, Install)));
        assert_eq!(ledger.style_mutation_count_during_entry(9), 0);
        assert!(ledger.record_application(application(9, 90, 1_060, Install)));
        assert_eq!(ledger.style_mutation_count_during_entry(9), 1);
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

/// Apply a complete [`WindowResizePolicy`] to the native window backing a GPUI
/// window: the resizable style bit (policy AND the caller's interaction gate)
/// plus content min/max size constraints. Returns `false` when the native
/// window cannot be resolved.
///
/// The `interaction_enabled` gate lets lifecycle owners keep a policy-resizable
/// shell locked while a calibrated entry/exit morph owns the frame (the glass
/// motion calibration is untouched by this function — it only flips the style
/// bit and constraints).
#[cfg(target_os = "macos")]
pub(crate) fn apply_window_resize_policy(
    window: &gpui::Window,
    policy: crate::window_resize::policy::WindowResizePolicy,
    interaction_enabled: bool,
) -> bool {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSSize;
    use objc::{msg_send, sel, sel_impl};

    const NS_WINDOW_STYLE_MASK_RESIZABLE: u64 = 1 << 3;

    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    unsafe {
        let ns_view = appkit.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![ns_view, window];
        if ns_window == nil {
            return false;
        }
        let before: u64 = msg_send![ns_window, styleMask];
        let should_resize = policy.user_resizable && interaction_enabled;
        let desired = if should_resize {
            before | NS_WINDOW_STYLE_MASK_RESIZABLE
        } else {
            before & !NS_WINDOW_STYLE_MASK_RESIZABLE
        };
        if desired != before {
            let _: () = msg_send![ns_window, setStyleMask: desired];
        }
        let _: () = msg_send![
            ns_window,
            setContentMinSize: NSSize::new(policy.min_content_width, policy.min_content_height)
        ];
        if let (Some(width), Some(height)) = (policy.max_content_width, policy.max_content_height) {
            let _: () = msg_send![ns_window, setContentMaxSize: NSSize::new(width, height)];
        }
        let window_number: i64 = msg_send![ns_window, windowNumber];
        let in_live_resize: bool = msg_send![ns_window, inLiveResize];
        let backing_scale: f64 = msg_send![ns_window, backingScaleFactor];
        tracing::info!(
            target: "script_kit::platform",
            event = "window_resize_policy_applied",
            window_number,
            style_mask_before = before,
            style_mask_after = desired,
            user_resizable = policy.user_resizable,
            interaction_enabled,
            min_content_width = policy.min_content_width,
            min_content_height = policy.min_content_height,
            max_content_width = policy.max_content_width,
            max_content_height = policy.max_content_height,
            backing_scale,
            in_live_resize,
        );
        true
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_window_resize_policy(
    _window: &gpui::Window,
    _policy: crate::window_resize::policy::WindowResizePolicy,
    _interaction_enabled: bool,
) -> bool {
    false
}

/// Dynamically re-partition a GPUI window's Tahoe glass backdrop between the
/// full-window layout (`bottom_inset <= 0`) and the detached-footer layout
/// (`bottom_inset > 0`). Owns geometry + shadow only: tint/material are NOT
/// reapplied (the theme signature did not change), so calling this on every
/// mode switch cannot flash the glass style.
///
/// Returns `false` when the native window or tagged backdrop cannot be
/// resolved (e.g. glass composition unavailable) — callers treat that as
/// "no native backdrop to partition", not an error.
#[cfg(target_os = "macos")]
pub(crate) fn set_gpui_window_backdrop_bottom_inset(
    window: &gpui::Window,
    window_name: &'static str,
    bottom_inset: f64,
) -> bool {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };
    unsafe {
        let ns_view = appkit.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![ns_view, window];
        if ns_window == nil {
            return false;
        }
        let content_view: id = msg_send![ns_window, contentView];
        if content_view == nil {
            return false;
        }
        let glass_view: id = msg_send![content_view, viewWithTag: TAHOE_GLASS_BACKDROP_TAG];
        if glass_view == nil {
            return false;
        }
        let layout = if bottom_inset > 0.0 {
            TahoeBackdropLayout::ContentAboveDetachedFooter { bottom_inset }
        } else {
            TahoeBackdropLayout::FullWindow
        };
        let current_inset: f64 = *(*(glass_view as *const objc::runtime::Object))
            .get_ivar::<f64>("_scriptKitBottomInset");
        let corner_radius = tahoe_backdrop_corner_radius_for(window_name, layout, content_view);
        update_tahoe_backdrop_geometry_and_shadow(
            ns_window,
            content_view,
            glass_view,
            layout,
            corner_radius,
        );
        let window_number: i64 = msg_send![ns_window, windowNumber];
        tracing::info!(
            target: "script_kit::platform",
            event = "window_backdrop_bottom_inset_set",
            window_name,
            window_number,
            bottom_inset_before = current_inset,
            bottom_inset_after = layout.bottom_inset(),
            corner_radius,
        );
        true
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_gpui_window_backdrop_bottom_inset(
    _window: &gpui::Window,
    _window_name: &'static str,
    _bottom_inset: f64,
) -> bool {
    false
}

// Re-export display/coordinate helpers from the unified display module.
pub use self::display::{
    clamp_to_visible, display_for_point, flip_y, get_active_display, get_global_mouse_position,
    get_macos_displays, get_macos_visible_displays, prefers_reduced_motion, primary_screen_height,
    VisibleDisplayBounds,
};

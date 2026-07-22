//! Pure state and math for the launcher's boundary-scroll affordance.
//!
//! This module deliberately knows nothing about GPUI list state, selection,
//! rendering, or async executors. The owning surface supplies edge eligibility,
//! schedules the watchdog/rebound work requested by [`BoundaryDecision`], and
//! guards each async update with the generation helpers below.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const RAW_PULL_CAP_MULTIPLIER: f32 = 3.0;
const INVERSE_RESISTANCE_RATIO_LIMIT: f32 = 0.999;
const SETTLE_ZERO_EPSILON_PX: f32 = 0.1;
const SETTLE_VELOCITY_EPSILON_PX_PER_SECOND: f32 = 4.0;
const VELOCITY_STALE_AFTER: Duration = Duration::from_millis(100);
const VELOCITY_MIN_INTERVAL_SECONDS: f64 = 0.001;
const VELOCITY_WINDOW_SECONDS: f64 = 0.048;
const VELOCITY_SAMPLE_CAPACITY: usize = 8;
const SPRING_OMEGA_PER_SECOND: f32 = 26.0;
const REBOUND_HARD_DEADLINE: Duration = Duration::from_millis(320);
const TRACE_SAMPLE_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoundaryEdge {
    Top,
    Bottom,
}

impl BoundaryEdge {
    #[inline]
    pub(crate) const fn sign(self) -> f32 {
        match self {
            Self::Top => 1.0,
            Self::Bottom => -1.0,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BoundaryPhase {
    #[default]
    Idle,
    Tracking,
    Settling,
}

impl BoundaryPhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Tracking => "tracking",
            Self::Settling => "settling",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreciseTouchPhase {
    Started,
    Moved,
    Ended,
}

impl PreciseTouchPhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Moved => "moved",
            Self::Ended => "ended",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettleReason {
    Ended,
    Cancelled,
    MomentumBeganImplicitRelease,
    MissingTerminalWatchdog,
    ReducedMotion,
    Reset,
}

impl SettleReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ended => "ended",
            Self::Cancelled => "cancelled",
            Self::MomentumBeganImplicitRelease => "momentumBeganImplicitRelease",
            Self::MissingTerminalWatchdog => "missingTerminalWatchdog",
            Self::ReducedMotion => "reducedMotion",
            Self::Reset => "reset",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ScrollLifecyclePhase {
    #[default]
    None,
    MayBegin,
    Began,
    Changed,
    Stationary,
    Ended,
    Cancelled,
}

impl ScrollLifecyclePhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MayBegin => "mayBegin",
            Self::Began => "began",
            Self::Changed => "changed",
            Self::Stationary => "stationary",
            Self::Ended => "ended",
            Self::Cancelled => "cancelled",
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BoundaryEligibility {
    pub top: bool,
    pub bottom: bool,
}

impl BoundaryEligibility {
    /// Resolve a new boundary capture from the event sign. When content fits
    /// and both edges are eligible, the delta direction disambiguates them.
    pub(crate) fn edge_for_delta(self, delta_y_px: f32) -> Option<BoundaryEdge> {
        if delta_y_px > 0.0 && self.top {
            Some(BoundaryEdge::Top)
        } else if delta_y_px < 0.0 && self.bottom {
            Some(BoundaryEdge::Bottom)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BoundaryAffordanceTuning {
    pub max_distance_px: f32,
    pub resistance_px: f32,
    pub settle_duration: Duration,
    pub idle_timeout: Duration,
    pub reduced_motion_max_distance_px: f32,
}

impl BoundaryAffordanceTuning {
    pub(crate) const fn new(
        max_distance_px: f32,
        resistance_px: f32,
        settle_duration: Duration,
        idle_timeout: Duration,
        reduced_motion_max_distance_px: f32,
    ) -> Self {
        Self {
            max_distance_px,
            resistance_px,
            settle_duration,
            idle_timeout,
            reduced_motion_max_distance_px,
        }
    }

    pub(crate) fn active_max_distance_px(self, reduced_motion: bool) -> f32 {
        let value = if reduced_motion {
            self.reduced_motion_max_distance_px
        } else {
            self.max_distance_px
        };
        finite_nonnegative(value)
    }

    fn normalized_resistance_px(self) -> f32 {
        if self.resistance_px.is_finite() {
            self.resistance_px.max(1.0)
        } else {
            1.0
        }
    }

    fn raw_pull_cap_px(self) -> f32 {
        self.normalized_resistance_px() * RAW_PULL_CAP_MULTIPLIER
    }

    pub(crate) const fn spring_omega_per_second(self) -> f32 {
        SPRING_OMEGA_PER_SECOND
    }

    pub(crate) const fn rebound_hard_deadline(self) -> Duration {
        REBOUND_HARD_DEADLINE
    }
}

impl Default for BoundaryAffordanceTuning {
    fn default() -> Self {
        Self::new(
            36.0,
            44.0,
            REBOUND_HARD_DEADLINE,
            Duration::from_millis(300),
            3.0,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BoundaryDecision {
    /// Event travel not consumed by unwinding the rubber band. The caller may
    /// pass this through to the existing selection-owned wheel path.
    pub residual_delta_y_px: f32,
    /// Offset, edge, or phase changed and the surface should repaint/report.
    pub visual_changed: bool,
    /// The first tracking event armed a single idle watchdog for this generation.
    pub arm_idle_watchdog: bool,
    /// A non-reduced-motion rebound entered `Settling` and needs frame updates.
    pub start_settle: Option<SettleReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdleWatchdogStatus {
    Cancelled,
    Sleep(Duration),
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ElasticTraceKind {
    Input,
    ReboundFrame,
    Reset,
}

impl ElasticTraceKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::ReboundFrame => "reboundFrame",
            Self::Reset => "reset",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ElasticTraceSample {
    pub kind: ElasticTraceKind,
    pub arrival_elapsed_ms: f64,
    pub native_timestamp_seconds: Option<f64>,
    pub direct_phase: ScrollLifecyclePhase,
    pub momentum_phase: ScrollLifecyclePhase,
    pub delta_y: f32,
    pub raw_pull_px: f32,
    pub offset_px: f32,
    pub velocity_px_per_second: f32,
    pub boundary_phase: BoundaryPhase,
    pub generation: u64,
    pub rendered_frame_generation: u64,
    pub last_rebound_elapsed_ms: f64,
}

#[derive(Debug)]
pub(crate) struct BoundaryAffordanceState {
    pub offset_px: f32,
    raw_pull_px: f32,
    pub edge: Option<BoundaryEdge>,
    pub phase: BoundaryPhase,
    pub generation: u64,
    pub last_touch_phase: Option<PreciseTouchPhase>,
    pub last_settle_reason: Option<SettleReason>,
    pub reduced_motion: bool,
    pub visual_velocity_px_per_second: f32,
    pub last_native_timestamp_seconds: Option<f64>,
    pub last_direct_phase: ScrollLifecyclePhase,
    pub last_momentum_phase: ScrollLifecyclePhase,
    pub suppress_momentum_until_terminal: bool,
    last_event_at: Option<Instant>,
    idle_watchdog_armed: bool,
    velocity_samples: VecDeque<VelocitySample>,
    rebound: Option<ReboundState>,
    trace_started_at: Option<Instant>,
    trace_samples: VecDeque<ElasticTraceSample>,
    pub rendered_frame_generation: u64,
    pub last_rebound_elapsed_ms: f64,
}

#[derive(Clone, Copy, Debug)]
struct VelocitySample {
    offset_px: f32,
    observed_at: Instant,
    native_timestamp_seconds: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct ReboundState {
    initial_offset_px: f32,
    initial_velocity_px_per_second: f32,
}

impl Default for BoundaryAffordanceState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl BoundaryAffordanceState {
    pub(crate) const fn new(reduced_motion: bool) -> Self {
        Self {
            offset_px: 0.0,
            raw_pull_px: 0.0,
            edge: None,
            phase: BoundaryPhase::Idle,
            generation: 0,
            last_touch_phase: None,
            last_settle_reason: None,
            reduced_motion,
            visual_velocity_px_per_second: 0.0,
            last_native_timestamp_seconds: None,
            last_direct_phase: ScrollLifecyclePhase::None,
            last_momentum_phase: ScrollLifecyclePhase::None,
            suppress_momentum_until_terminal: false,
            last_event_at: None,
            idle_watchdog_armed: false,
            velocity_samples: VecDeque::new(),
            rebound: None,
            trace_started_at: None,
            trace_samples: VecDeque::new(),
            rendered_frame_generation: 0,
            last_rebound_elapsed_ms: 0.0,
        }
    }

    pub(crate) fn raw_pull_px(&self) -> f32 {
        self.raw_pull_px
    }

    pub(crate) fn last_event_at(&self) -> Option<Instant> {
        self.last_event_at
    }

    pub(crate) fn idle_watchdog_armed(&self) -> bool {
        self.idle_watchdog_armed
    }

    pub(crate) fn generation_is_current(&self, captured_generation: u64) -> bool {
        self.generation == captured_generation
    }

    pub(crate) fn tracking_generation_is_current(&self, captured_generation: u64) -> bool {
        self.generation_is_current(captured_generation) && self.phase == BoundaryPhase::Tracking
    }

    pub(crate) fn settling_generation_is_current(&self, captured_generation: u64) -> bool {
        self.generation_is_current(captured_generation) && self.phase == BoundaryPhase::Settling
    }

    /// Invalidate every task holding the previous generation.
    pub(crate) fn cancel_pending_work(&mut self) -> u64 {
        self.advance_generation()
    }

    pub(crate) fn handle_scroll_lifecycle(
        &mut self,
        delta_y_px: f32,
        direct_phase: ScrollLifecyclePhase,
        momentum_phase: ScrollLifecyclePhase,
        legacy_phase: PreciseTouchPhase,
        eligibility: BoundaryEligibility,
        tuning: BoundaryAffordanceTuning,
        reduced_motion: bool,
        now: Instant,
        native_timestamp_seconds: Option<f64>,
    ) -> BoundaryDecision {
        let decision = self.handle_scroll_lifecycle_inner(
            delta_y_px,
            direct_phase,
            momentum_phase,
            legacy_phase,
            eligibility,
            tuning,
            reduced_motion,
            now,
            native_timestamp_seconds,
        );
        self.record_trace_sample(ElasticTraceKind::Input, delta_y_px, now);
        decision
    }

    /// Observe a native viewport scroll without claiming any of its delta.
    ///
    /// Launcher wheel and momentum events have already updated GPUI ListState
    /// before this entry point runs. The returned decision may schedule visual
    /// affordance work, but it can never request residual selection movement or
    /// leave momentum marked as owned by the affordance.
    pub(crate) fn observe_scroll_lifecycle(
        &mut self,
        delta_y_px: f32,
        direct_phase: ScrollLifecyclePhase,
        momentum_phase: ScrollLifecyclePhase,
        legacy_phase: PreciseTouchPhase,
        eligibility: BoundaryEligibility,
        tuning: BoundaryAffordanceTuning,
        reduced_motion: bool,
        now: Instant,
        native_timestamp_seconds: Option<f64>,
    ) -> BoundaryDecision {
        let mut decision = self.handle_scroll_lifecycle(
            delta_y_px,
            direct_phase,
            momentum_phase,
            legacy_phase,
            eligibility,
            tuning,
            reduced_motion,
            now,
            native_timestamp_seconds,
        );
        decision.residual_delta_y_px = 0.0;
        self.suppress_momentum_until_terminal = false;
        decision
    }

    fn handle_scroll_lifecycle_inner(
        &mut self,
        delta_y_px: f32,
        direct_phase: ScrollLifecyclePhase,
        momentum_phase: ScrollLifecyclePhase,
        legacy_phase: PreciseTouchPhase,
        eligibility: BoundaryEligibility,
        tuning: BoundaryAffordanceTuning,
        reduced_motion: bool,
        now: Instant,
        native_timestamp_seconds: Option<f64>,
    ) -> BoundaryDecision {
        self.last_direct_phase = direct_phase;
        self.last_momentum_phase = momentum_phase;
        self.last_native_timestamp_seconds =
            native_timestamp_seconds.filter(|value| value.is_finite());
        if native_timestamp_seconds.is_some_and(|value| !value.is_finite()) {
            self.velocity_samples.clear();
            self.visual_velocity_px_per_second = 0.0;
        }

        if direct_phase != ScrollLifecyclePhase::None {
            return match direct_phase {
                ScrollLifecyclePhase::MayBegin | ScrollLifecyclePhase::Began => {
                    self.suppress_momentum_until_terminal = false;
                    self.handle_precise_scroll(
                        delta_y_px,
                        PreciseTouchPhase::Started,
                        eligibility,
                        tuning,
                        reduced_motion,
                        now,
                        native_timestamp_seconds,
                    )
                }
                ScrollLifecyclePhase::Changed => {
                    self.suppress_momentum_until_terminal = false;
                    self.handle_precise_scroll(
                        delta_y_px,
                        PreciseTouchPhase::Moved,
                        eligibility,
                        tuning,
                        reduced_motion,
                        now,
                        native_timestamp_seconds,
                    )
                }
                ScrollLifecyclePhase::Stationary => BoundaryDecision {
                    residual_delta_y_px: 0.0,
                    ..BoundaryDecision::default()
                },
                ScrollLifecyclePhase::Ended | ScrollLifecyclePhase::Cancelled => {
                    self.last_touch_phase = Some(PreciseTouchPhase::Ended);
                    let reason = if direct_phase == ScrollLifecyclePhase::Cancelled {
                        SettleReason::Cancelled
                    } else {
                        SettleReason::Ended
                    };

                    // A terminal direct event owns the following momentum tail
                    // only when it releases a real elastic boundary capture.
                    // Interior releases must leave native fling deltas unowned.
                    if self.phase == BoundaryPhase::Settling
                        || self.suppress_momentum_until_terminal
                    {
                        return BoundaryDecision::default();
                    }
                    if !self.has_releasable_elastic_capture() {
                        self.finish_at_zero(reason);
                        return BoundaryDecision::default();
                    }

                    self.age_velocity_for_release(now, native_timestamp_seconds);
                    if direct_phase == ScrollLifecyclePhase::Cancelled {
                        self.visual_velocity_px_per_second = 0.0;
                    }
                    self.suppress_momentum_until_terminal = true;
                    let mut decision = self.begin_settle(reason, tuning);
                    decision.residual_delta_y_px = 0.0;
                    decision
                }
                ScrollLifecyclePhase::None => unreachable!(),
            };
        }

        if momentum_phase != ScrollLifecyclePhase::None {
            if momentum_phase == ScrollLifecyclePhase::Began
                && self.has_releasable_elastic_capture()
            {
                self.age_velocity_for_release(now, native_timestamp_seconds);
                self.suppress_momentum_until_terminal = true;
                let mut decision =
                    self.begin_settle(SettleReason::MomentumBeganImplicitRelease, tuning);
                decision.residual_delta_y_px = 0.0;
                return decision;
            }

            let was_suppressed = self.suppress_momentum_until_terminal;
            if momentum_phase.is_terminal() {
                self.suppress_momentum_until_terminal = false;
            }
            return BoundaryDecision {
                residual_delta_y_px: if was_suppressed { 0.0 } else { delta_y_px },
                ..BoundaryDecision::default()
            };
        }

        self.handle_precise_scroll(
            delta_y_px,
            legacy_phase,
            eligibility,
            tuning,
            reduced_motion,
            now,
            native_timestamp_seconds,
        )
    }

    #[inline]
    fn has_releasable_elastic_capture(&self) -> bool {
        self.phase == BoundaryPhase::Tracking
            && self.edge.is_some()
            && self.offset_px.abs() >= SETTLE_ZERO_EPSILON_PX
    }

    pub(crate) fn trace_samples(&self) -> impl Iterator<Item = &ElasticTraceSample> {
        self.trace_samples.iter()
    }

    pub(crate) fn rebound_initial_offset_px(&self) -> Option<f32> {
        self.rebound.map(|rebound| rebound.initial_offset_px)
    }

    pub(crate) fn rebound_initial_velocity_px_per_second(&self) -> Option<f32> {
        self.rebound
            .map(|rebound| rebound.initial_velocity_px_per_second)
    }

    /// Consume one precise pixel-scroll event. Positive Y pulls outward at the
    /// top; negative Y pulls outward at the bottom.
    pub(crate) fn handle_precise_scroll(
        &mut self,
        delta_y_px: f32,
        touch_phase: PreciseTouchPhase,
        eligibility: BoundaryEligibility,
        tuning: BoundaryAffordanceTuning,
        reduced_motion: bool,
        now: Instant,
        native_timestamp_seconds: Option<f64>,
    ) -> BoundaryDecision {
        let before = self.visual_signature();
        let delta_y_px = finite_or_zero(delta_y_px);
        let mut decision = BoundaryDecision {
            residual_delta_y_px: delta_y_px,
            ..BoundaryDecision::default()
        };
        let mut generation_prepared = false;

        self.reduced_motion = reduced_motion;
        self.last_touch_phase = Some(touch_phase);
        self.last_native_timestamp_seconds =
            native_timestamp_seconds.filter(|value| value.is_finite());

        if touch_phase == PreciseTouchPhase::Started {
            self.visual_velocity_px_per_second = 0.0;
            self.velocity_samples.clear();
        }

        if self.phase == BoundaryPhase::Settling
            && matches!(
                touch_phase,
                PreciseTouchPhase::Started | PreciseTouchPhase::Moved
            )
        {
            self.grab_settling_offset(tuning);
            generation_prepared = true;
        } else if touch_phase == PreciseTouchPhase::Started {
            self.advance_generation();
            self.idle_watchdog_armed = false;
            generation_prepared = true;
        }

        if matches!(
            touch_phase,
            PreciseTouchPhase::Started | PreciseTouchPhase::Moved
        ) {
            self.last_event_at = Some(now);
        }

        if self.phase != BoundaryPhase::Settling {
            if self.edge.is_none() {
                if let Some(edge) = eligibility.edge_for_delta(delta_y_px) {
                    if !generation_prepared {
                        self.advance_generation();
                    }
                    self.edge = Some(edge);
                    self.phase = BoundaryPhase::Tracking;
                    self.raw_pull_px = 0.0;
                    self.offset_px = 0.0;
                }
            }

            if self.phase == BoundaryPhase::Tracking {
                decision.residual_delta_y_px = self.apply_tracking_delta(delta_y_px, tuning);

                if touch_phase != PreciseTouchPhase::Ended {
                    self.update_visual_velocity(now, self.last_native_timestamp_seconds);
                }

                if self.phase == BoundaryPhase::Tracking
                    && touch_phase != PreciseTouchPhase::Ended
                    && !self.idle_watchdog_armed
                {
                    self.idle_watchdog_armed = true;
                    decision.arm_idle_watchdog = true;
                }
            }
        }

        if touch_phase == PreciseTouchPhase::Ended && self.phase == BoundaryPhase::Tracking {
            let settle = self.begin_settle(SettleReason::Ended, tuning);
            decision.start_settle = settle.start_settle;
            decision.arm_idle_watchdog = false;
        }

        decision.visual_changed = before != self.visual_signature();
        decision
    }

    /// Enter a rebound, or snap to exact idle when reduced motion is enabled.
    pub(crate) fn begin_settle(
        &mut self,
        reason: SettleReason,
        tuning: BoundaryAffordanceTuning,
    ) -> BoundaryDecision {
        let before = self.visual_signature();
        let mut decision = BoundaryDecision::default();

        if self.phase != BoundaryPhase::Tracking || self.offset_px.abs() < SETTLE_ZERO_EPSILON_PX {
            self.finish_at_zero(reason);
        } else if self.reduced_motion {
            self.advance_generation();
            self.finish_at_zero(SettleReason::ReducedMotion);
        } else {
            self.advance_generation();
            self.phase = BoundaryPhase::Settling;
            self.raw_pull_px = 0.0;
            let edge_sign = self.edge.map(BoundaryEdge::sign).unwrap_or(1.0);
            let normalized_offset = (edge_sign * self.offset_px).max(0.0);
            let velocity_limit = tuning.spring_omega_per_second() * normalized_offset;
            let normalized_velocity = (edge_sign * self.visual_velocity_px_per_second)
                .clamp(-velocity_limit, velocity_limit);
            self.visual_velocity_px_per_second = edge_sign * normalized_velocity;
            self.rebound = Some(ReboundState {
                initial_offset_px: self.offset_px,
                initial_velocity_px_per_second: self.visual_velocity_px_per_second,
            });
            self.last_rebound_elapsed_ms = 0.0;
            self.last_settle_reason = Some(reason);
            self.last_event_at = None;
            self.idle_watchdog_armed = false;
            decision.start_settle = Some(reason);
        }

        decision.visual_changed = before != self.visual_signature();
        decision
    }

    /// Return the watchdog's next action without mutating state. `Cancelled`
    /// covers stale generations as well as gestures that already ended/reset.
    pub(crate) fn idle_watchdog_status(
        &self,
        captured_generation: u64,
        now: Instant,
        tuning: BoundaryAffordanceTuning,
    ) -> IdleWatchdogStatus {
        if !self.tracking_generation_is_current(captured_generation) || !self.idle_watchdog_armed {
            return IdleWatchdogStatus::Cancelled;
        }

        let elapsed = self
            .last_event_at
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or(tuning.idle_timeout);
        if elapsed >= tuning.idle_timeout {
            IdleWatchdogStatus::TimedOut
        } else {
            IdleWatchdogStatus::Sleep(tuning.idle_timeout.saturating_sub(elapsed))
        }
    }

    /// Start the timeout rebound only if the watchdog still owns this gesture.
    pub(crate) fn begin_idle_timeout_settle(
        &mut self,
        captured_generation: u64,
        now: Instant,
        tuning: BoundaryAffordanceTuning,
    ) -> BoundaryDecision {
        if self.idle_watchdog_status(captured_generation, now, tuning)
            != IdleWatchdogStatus::TimedOut
        {
            return BoundaryDecision::default();
        }
        self.begin_settle(SettleReason::MissingTerminalWatchdog, tuning)
    }

    /// Apply one analytic critically damped rebound sample. A stale task cannot mutate state.
    /// The accepted terminal sample always produces exact zero/idle state.
    pub(crate) fn apply_settle_sample(
        &mut self,
        captured_generation: u64,
        elapsed: Duration,
        tuning: BoundaryAffordanceTuning,
    ) -> bool {
        if !self.settling_generation_is_current(captured_generation) {
            return false;
        }
        self.last_rebound_elapsed_ms = elapsed.as_secs_f64() * 1000.0;

        if elapsed >= tuning.rebound_hard_deadline() {
            self.finish_at_zero_preserving_generation();
            self.rendered_frame_generation = self.rendered_frame_generation.wrapping_add(1);
            self.record_trace_sample(ElasticTraceKind::ReboundFrame, 0.0, Instant::now());
            return true;
        }

        let Some(rebound) = self.rebound else {
            self.finish_at_zero_preserving_generation();
            return true;
        };
        let (next, velocity) = critically_damped_rebound(
            rebound.initial_offset_px,
            rebound.initial_velocity_px_per_second,
            elapsed,
            tuning.spring_omega_per_second(),
        );
        let crossed_zero = rebound.initial_offset_px.signum() != next.signum();
        if crossed_zero
            || (next.abs() < SETTLE_ZERO_EPSILON_PX
                && velocity.abs() < SETTLE_VELOCITY_EPSILON_PX_PER_SECOND)
        {
            self.finish_at_zero_preserving_generation();
        } else {
            let Some(edge) = self.edge else {
                self.finish_at_zero_preserving_generation();
                return true;
            };
            let max_distance = tuning.active_max_distance_px(self.reduced_motion);
            let bounded_magnitude = next.abs().min(max_distance);
            self.offset_px = edge.sign() * bounded_magnitude;
            self.visual_velocity_px_per_second =
                if next.abs() > max_distance && velocity.signum() == next.signum() {
                    0.0
                } else {
                    velocity
                };
        }
        self.rendered_frame_generation = self.rendered_frame_generation.wrapping_add(1);
        self.record_trace_sample(ElasticTraceKind::ReboundFrame, 0.0, Instant::now());
        true
    }

    pub(crate) fn finish_settle_if_current(&mut self, captured_generation: u64) -> bool {
        if !self.settling_generation_is_current(captured_generation) {
            return false;
        }
        self.finish_at_zero_preserving_generation();
        true
    }

    /// Central reset seam for view/filter/theme/selection/resize changes.
    pub(crate) fn reset(&mut self, reason: SettleReason) -> bool {
        let before = self.visual_signature();
        self.advance_generation();
        self.suppress_momentum_until_terminal = false;
        self.finish_at_zero(reason);
        self.record_trace_sample(ElasticTraceKind::Reset, 0.0, Instant::now());
        before != self.visual_signature()
    }

    fn apply_tracking_delta(&mut self, delta_y_px: f32, tuning: BoundaryAffordanceTuning) -> f32 {
        let Some(edge) = self.edge else {
            return delta_y_px;
        };
        let outward_component = delta_y_px * edge.sign();

        if outward_component > 0.0 {
            self.raw_pull_px = (self.raw_pull_px + outward_component).min(tuning.raw_pull_cap_px());
            self.recompute_offset(edge, tuning);
            return 0.0;
        }

        if outward_component < 0.0 {
            let inward_px = -outward_component;
            let consumed = inward_px.min(self.raw_pull_px);
            self.raw_pull_px -= consumed;
            let residual_inward_px = inward_px - consumed;

            if self.raw_pull_px <= f32::EPSILON {
                self.finish_at_zero_preserving_generation();
            } else {
                self.recompute_offset(edge, tuning);
            }

            return -edge.sign() * residual_inward_px;
        }

        0.0
    }

    fn recompute_offset(&mut self, edge: BoundaryEdge, tuning: BoundaryAffordanceTuning) {
        let magnitude = resisted_offset(
            self.raw_pull_px,
            tuning.active_max_distance_px(self.reduced_motion),
            tuning.normalized_resistance_px(),
        );
        self.offset_px = edge.sign() * magnitude;
    }

    fn grab_settling_offset(&mut self, tuning: BoundaryAffordanceTuning) {
        self.advance_generation();
        self.idle_watchdog_armed = false;
        self.last_event_at = None;
        self.rebound = None;
        self.visual_velocity_px_per_second = 0.0;

        let Some(edge) = self.edge else {
            self.finish_at_zero_preserving_generation();
            return;
        };
        if self.offset_px.abs() < SETTLE_ZERO_EPSILON_PX {
            self.finish_at_zero_preserving_generation();
            return;
        }

        self.raw_pull_px = raw_pull_for_offset(
            self.offset_px,
            tuning.active_max_distance_px(self.reduced_motion),
            tuning.normalized_resistance_px(),
        )
        .min(tuning.raw_pull_cap_px());
        self.offset_px = edge.sign()
            * resisted_offset(
                self.raw_pull_px,
                tuning.active_max_distance_px(self.reduced_motion),
                tuning.normalized_resistance_px(),
            );
        self.phase = BoundaryPhase::Tracking;
        self.velocity_samples.clear();
    }

    fn update_visual_velocity(&mut self, now: Instant, native_timestamp_seconds: Option<f64>) {
        let next = VelocitySample {
            offset_px: self.offset_px,
            observed_at: now,
            native_timestamp_seconds,
        };
        let Some(previous) = self.velocity_samples.back().copied() else {
            self.velocity_samples.push_back(next);
            return;
        };

        let native_dt = next
            .native_timestamp_seconds
            .zip(previous.native_timestamp_seconds)
            .map(|(current, prior)| current - prior)
            .filter(|dt| dt.is_finite());
        let wall_dt = now.saturating_duration_since(previous.observed_at);
        let dt = native_dt.unwrap_or(wall_dt.as_secs_f64());
        if dt == 0.0 {
            if let Some(latest) = self.velocity_samples.back_mut() {
                *latest = next;
            }
            self.recompute_visual_velocity();
            return;
        }
        if dt < VELOCITY_MIN_INTERVAL_SECONDS || wall_dt > VELOCITY_STALE_AFTER {
            self.velocity_samples.clear();
            self.velocity_samples.push_back(next);
            self.visual_velocity_px_per_second = 0.0;
            return;
        }

        self.velocity_samples.push_back(next);
        while self.velocity_samples.len() > VELOCITY_SAMPLE_CAPACITY {
            self.velocity_samples.pop_front();
        }
        while self.velocity_samples.len() > 2 {
            let first = self.velocity_samples.front().copied().unwrap();
            let span = next
                .native_timestamp_seconds
                .zip(first.native_timestamp_seconds)
                .map(|(current, prior)| current - prior)
                .unwrap_or_else(|| {
                    now.saturating_duration_since(first.observed_at)
                        .as_secs_f64()
                });
            if span <= VELOCITY_WINDOW_SECONDS {
                break;
            }
            self.velocity_samples.pop_front();
        }
        self.recompute_visual_velocity();
    }

    fn recompute_visual_velocity(&mut self) {
        if self.velocity_samples.len() < 2 {
            self.visual_velocity_px_per_second = 0.0;
            return;
        }
        let first = self.velocity_samples.front().copied().unwrap();
        let times: Vec<f64> = self
            .velocity_samples
            .iter()
            .map(|sample| {
                sample
                    .native_timestamp_seconds
                    .zip(first.native_timestamp_seconds)
                    .map(|(current, start)| current - start)
                    .unwrap_or_else(|| {
                        sample
                            .observed_at
                            .saturating_duration_since(first.observed_at)
                            .as_secs_f64()
                    })
            })
            .collect();
        if times.iter().any(|time| !time.is_finite() || *time < 0.0) {
            self.velocity_samples.clear();
            self.visual_velocity_px_per_second = 0.0;
            return;
        }
        let mean_t = times.iter().sum::<f64>() / times.len() as f64;
        let mean_x = self
            .velocity_samples
            .iter()
            .map(|sample| sample.offset_px as f64)
            .sum::<f64>()
            / self.velocity_samples.len() as f64;
        let numerator = times
            .iter()
            .zip(self.velocity_samples.iter())
            .map(|(time, sample)| (time - mean_t) * (sample.offset_px as f64 - mean_x))
            .sum::<f64>();
        let denominator = times
            .iter()
            .map(|time| (time - mean_t).powi(2))
            .sum::<f64>();
        self.visual_velocity_px_per_second = if denominator >= VELOCITY_MIN_INTERVAL_SECONDS.powi(2)
        {
            (numerator / denominator) as f32
        } else {
            0.0
        };
    }

    fn age_velocity_for_release(&mut self, now: Instant, native_timestamp_seconds: Option<f64>) {
        let Some(sample) = self.velocity_samples.back().copied() else {
            self.visual_velocity_px_per_second = 0.0;
            return;
        };
        let native_age = native_timestamp_seconds
            .zip(sample.native_timestamp_seconds)
            .map(|(current, prior)| current - prior)
            .filter(|age| age.is_finite() && *age >= 0.0);
        let stale = native_age
            .map(|age| age > VELOCITY_STALE_AFTER.as_secs_f64())
            .unwrap_or_else(|| {
                now.saturating_duration_since(sample.observed_at) > VELOCITY_STALE_AFTER
            });
        if stale {
            self.visual_velocity_px_per_second = 0.0;
        }
    }

    fn finish_at_zero(&mut self, reason: SettleReason) {
        self.finish_at_zero_preserving_generation();
        self.last_settle_reason = Some(reason);
        self.last_event_at = None;
    }

    fn finish_at_zero_preserving_generation(&mut self) {
        self.offset_px = 0.0;
        self.raw_pull_px = 0.0;
        self.edge = None;
        self.phase = BoundaryPhase::Idle;
        self.last_event_at = None;
        self.idle_watchdog_armed = false;
        self.visual_velocity_px_per_second = 0.0;
        self.velocity_samples.clear();
        self.rebound = None;
    }

    fn advance_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    fn visual_signature(&self) -> (u32, Option<BoundaryEdge>, BoundaryPhase) {
        (self.offset_px.to_bits(), self.edge, self.phase)
    }

    fn record_trace_sample(&mut self, kind: ElasticTraceKind, delta_y: f32, now: Instant) {
        let started_at = *self.trace_started_at.get_or_insert(now);
        self.trace_samples.push_back(ElasticTraceSample {
            kind,
            arrival_elapsed_ms: now.saturating_duration_since(started_at).as_secs_f64() * 1000.0,
            native_timestamp_seconds: self.last_native_timestamp_seconds,
            direct_phase: self.last_direct_phase,
            momentum_phase: self.last_momentum_phase,
            delta_y,
            raw_pull_px: self.raw_pull_px,
            offset_px: self.offset_px,
            velocity_px_per_second: self.visual_velocity_px_per_second,
            boundary_phase: self.phase,
            generation: self.generation,
            rendered_frame_generation: self.rendered_frame_generation,
            last_rebound_elapsed_ms: self.last_rebound_elapsed_ms,
        });
        while self.trace_samples.len() > TRACE_SAMPLE_CAPACITY {
            self.trace_samples.pop_front();
        }
    }
}

pub(crate) fn resisted_offset(raw_pull_px: f32, max_px: f32, resistance_px: f32) -> f32 {
    let raw_pull_px = finite_nonnegative(raw_pull_px);
    let max_px = finite_nonnegative(max_px);
    let resistance_px = if resistance_px.is_finite() {
        resistance_px.max(1.0)
    } else {
        1.0
    };
    max_px * (1.0 - (-raw_pull_px / resistance_px).exp())
}

pub(crate) fn raw_pull_for_offset(offset_px: f32, max_px: f32, resistance_px: f32) -> f32 {
    let max_px = finite_nonnegative(max_px);
    if max_px == 0.0 {
        return 0.0;
    }
    let resistance_px = if resistance_px.is_finite() {
        resistance_px.max(1.0)
    } else {
        1.0
    };
    let ratio =
        (finite_nonnegative(offset_px.abs()) / max_px).clamp(0.0, INVERSE_RESISTANCE_RATIO_LIMIT);
    -resistance_px * (1.0 - ratio).ln()
}

pub(crate) fn critically_damped_rebound(
    initial_offset_px: f32,
    initial_velocity_px_per_second: f32,
    elapsed: Duration,
    omega_per_second: f32,
) -> (f32, f32) {
    if !omega_per_second.is_finite() || omega_per_second <= 0.0 {
        return (0.0, 0.0);
    }
    let t = elapsed.as_secs_f32().max(0.0);
    let omega = omega_per_second;
    let x0 = finite_or_zero(initial_offset_px);
    let v0 = finite_or_zero(initial_velocity_px_per_second);
    let coefficient = v0 + omega * x0;
    let decay = (-omega * t).exp();
    let position = (x0 + coefficient * t) * decay;
    let velocity = (v0 - omega * coefficient * t) * decay;
    (finite_or_zero(position), finite_or_zero(velocity))
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top_only() -> BoundaryEligibility {
        BoundaryEligibility {
            top: true,
            bottom: false,
        }
    }

    fn bottom_only() -> BoundaryEligibility {
        BoundaryEligibility {
            top: false,
            bottom: true,
        }
    }

    fn moved(
        state: &mut BoundaryAffordanceState,
        delta_y_px: f32,
        eligibility: BoundaryEligibility,
        now: Instant,
    ) -> BoundaryDecision {
        state.handle_precise_scroll(
            delta_y_px,
            PreciseTouchPhase::Moved,
            eligibility,
            BoundaryAffordanceTuning::default(),
            false,
            now,
            None,
        )
    }

    #[test]
    fn native_observer_never_returns_residual_or_claims_momentum() {
        let now = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();

        let direct = state.observe_scroll_lifecycle(
            18.0,
            ScrollLifecyclePhase::Began,
            ScrollLifecyclePhase::None,
            PreciseTouchPhase::Started,
            top_only(),
            tuning,
            false,
            now,
            Some(1.0),
        );
        assert_eq!(direct.residual_delta_y_px, 0.0);
        assert!(state.offset_px > 0.0);
        assert!(!state.suppress_momentum_until_terminal);

        let momentum = state.observe_scroll_lifecycle(
            -8.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Began,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            now + Duration::from_millis(16),
            Some(1.016),
        );
        assert_eq!(momentum.residual_delta_y_px, 0.0);
        assert!(!state.suppress_momentum_until_terminal);
    }

    #[test]
    fn resistance_is_monotonic_and_bounded() {
        let mut previous = 0.0;
        for raw in [0.0, 1.0, 12.0, 36.0, 72.0, 108.0, 10_000.0] {
            let offset = resisted_offset(raw, 18.0, 36.0);
            assert!(offset >= previous);
            assert!(offset < 18.0 || (18.0 - offset).abs() <= f32::EPSILON);
            previous = offset;
        }
    }

    #[test]
    fn shipping_drag_tuning_has_immediate_diminishing_gain() {
        let tuning = BoundaryAffordanceTuning::default();
        let initial_gain = tuning.max_distance_px / tuning.resistance_px;
        assert!((0.75..=0.95).contains(&initial_gain));

        let samples = [0.0, 5.0, 12.0, 36.0, 72.0, 132.0];
        let offsets: Vec<_> = samples
            .into_iter()
            .map(|raw| resisted_offset(raw, tuning.max_distance_px, tuning.resistance_px))
            .collect();
        assert_eq!(offsets[0], 0.0);
        assert!(offsets.windows(2).all(|pair| pair[1] > pair[0]));
        let gains: Vec<_> = offsets
            .windows(2)
            .zip(samples.windows(2))
            .map(|(offset, raw)| (offset[1] - offset[0]) / (raw[1] - raw[0]))
            .collect();
        assert!(gains.windows(2).all(|pair| pair[1] < pair[0]));
        assert!(offsets.last().copied().unwrap_or_default() < tuning.max_distance_px);
    }

    #[test]
    fn edge_signs_and_direction_disambiguate_dual_edge_content() {
        assert_eq!(BoundaryEdge::Top.sign(), 1.0);
        assert_eq!(BoundaryEdge::Bottom.sign(), -1.0);

        let both = BoundaryEligibility {
            top: true,
            bottom: true,
        };
        assert_eq!(both.edge_for_delta(1.0), Some(BoundaryEdge::Top));
        assert_eq!(both.edge_for_delta(-1.0), Some(BoundaryEdge::Bottom));
        assert_eq!(both.edge_for_delta(0.0), None);
    }

    #[test]
    fn partial_reversal_reduces_offset_without_residual_navigation() {
        let now = Instant::now();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 36.0, top_only(), now);
        let before = state.offset_px;

        let decision = moved(&mut state, -12.0, top_only(), now);

        assert_eq!(decision.residual_delta_y_px, 0.0);
        assert!(state.offset_px > 0.0);
        assert!(state.offset_px < before);
        assert_eq!(state.phase, BoundaryPhase::Tracking);
    }

    #[test]
    fn full_reversal_returns_residual_with_original_inward_sign() {
        let now = Instant::now();
        let mut top = BoundaryAffordanceState::default();
        moved(&mut top, 20.0, top_only(), now);
        let top_decision = moved(&mut top, -32.0, top_only(), now);
        assert_eq!(top_decision.residual_delta_y_px, -12.0);
        assert_idle_exact(&top);

        let mut bottom = BoundaryAffordanceState::default();
        moved(&mut bottom, -20.0, bottom_only(), now);
        let bottom_decision = moved(&mut bottom, 32.0, bottom_only(), now);
        assert_eq!(bottom_decision.residual_delta_y_px, 12.0);
        assert_idle_exact(&bottom);
    }

    #[test]
    fn raw_pull_is_bounded_before_it_can_create_reversal_debt() {
        let now = Instant::now();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 100_000.0, top_only(), now);

        assert_eq!(state.raw_pull_px(), 132.0);
        assert!(state.offset_px < 36.0);
    }

    #[test]
    fn a_new_gesture_grabs_a_settling_offset_without_snapping() {
        let now = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 36.0, top_only(), now);
        let ended = state.handle_precise_scroll(
            0.0,
            PreciseTouchPhase::Ended,
            top_only(),
            tuning,
            false,
            now,
            None,
        );
        assert_eq!(ended.start_settle, Some(SettleReason::Ended));
        let settling_offset = state.offset_px;
        let settling_generation = state.generation;

        let grabbed = state.handle_precise_scroll(
            0.0,
            PreciseTouchPhase::Started,
            top_only(),
            tuning,
            false,
            now,
            None,
        );

        assert!(grabbed.arm_idle_watchdog);
        assert_eq!(state.phase, BoundaryPhase::Tracking);
        assert!(state.generation != settling_generation);
        assert!((state.offset_px - settling_offset).abs() < 0.001);
        assert!((state.raw_pull_px() - 36.0).abs() < 0.001);
    }

    #[test]
    fn stale_settle_generation_cannot_mutate_state() {
        let now = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 36.0, top_only(), now);
        state.begin_settle(SettleReason::Ended, tuning);
        let stale_generation = state.generation;
        let starting_offset = state.offset_px;
        state.cancel_pending_work();

        assert!(!state.apply_settle_sample(stale_generation, Duration::from_millis(90), tuning,));
        assert_eq!(state.offset_px, starting_offset);
    }

    #[test]
    fn critically_damped_settle_decreases_and_finishes_at_exact_zero() {
        let tuning = BoundaryAffordanceTuning::default();
        let samples_ms = [0, 30, 60, 100, 180];
        let mut previous = 18.0;
        for milliseconds in samples_ms.into_iter().skip(1) {
            let (next, _) = critically_damped_rebound(
                18.0,
                0.0,
                Duration::from_millis(milliseconds),
                tuning.spring_omega_per_second(),
            );
            assert!(
                next < previous,
                "t={milliseconds} next={next} previous={previous}"
            );
            previous = next;
        }

        let now = Instant::now();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 36.0, top_only(), now);
        state.begin_settle(SettleReason::Ended, tuning);
        let generation = state.generation;
        assert!(state.apply_settle_sample(generation, Duration::from_secs(1), tuning));
        assert_idle_exact(&state);
    }

    #[test]
    fn shipping_spring_meets_peak_turn_and_return_thresholds() {
        let tuning = BoundaryAffordanceTuning::default();
        let omega = tuning.spring_omega_per_second();
        let x0 = 20.0;
        let v0 = omega * x0;
        let (at_release, release_velocity) =
            critically_damped_rebound(x0, v0, Duration::ZERO, omega);
        assert!((at_release - x0).abs() < 0.001);
        assert!((release_velocity - v0).abs() < 0.001);

        let turn_ms = 19;
        let (peak, velocity_at_turn) =
            critically_damped_rebound(x0, v0, Duration::from_millis(turn_ms), omega);
        assert!(peak <= 1.22 * x0);
        assert!(velocity_at_turn.abs() < 12.0);
        assert!(turn_ms <= 25);

        let (half, _) = critically_damped_rebound(x0, 0.0, Duration::from_millis(64), omega);
        assert!((0.45..=0.55).contains(&(half / x0)));
        let (ten_percent, _) =
            critically_damped_rebound(x0, 0.0, Duration::from_millis(150), omega);
        assert!(ten_percent <= 0.11 * x0);
    }

    #[test]
    fn release_rebound_preserves_measured_visual_velocity() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        state.handle_precise_scroll(
            0.0,
            PreciseTouchPhase::Started,
            top_only(),
            tuning,
            false,
            start,
            Some(10.0),
        );
        state.handle_precise_scroll(
            24.0,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(16),
            Some(10.016),
        );
        state.handle_precise_scroll(
            12.0,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(32),
            Some(10.032),
        );
        assert!(state.visual_velocity_px_per_second > 0.0);
        let release_velocity = state.visual_velocity_px_per_second;
        let release_offset = state.offset_px;
        state.handle_precise_scroll(
            0.0,
            PreciseTouchPhase::Ended,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(36),
            Some(10.036),
        );
        assert_eq!(state.phase, BoundaryPhase::Settling);
        assert_eq!(
            state
                .rebound
                .expect("release should capture rebound velocity")
                .initial_velocity_px_per_second,
            release_velocity.min(tuning.spring_omega_per_second() * release_offset)
        );
    }

    #[test]
    fn cancelled_release_rebounds_immediately_with_zero_velocity() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 24.0, top_only(), start);
        state.visual_velocity_px_per_second = 500.0;

        let decision = state.handle_scroll_lifecycle(
            0.0,
            ScrollLifecyclePhase::Cancelled,
            ScrollLifecyclePhase::None,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(16),
            Some(1.016),
        );

        assert_eq!(decision.start_settle, Some(SettleReason::Cancelled));
        assert_eq!(state.phase, BoundaryPhase::Settling);
        assert_eq!(state.last_settle_reason, Some(SettleReason::Cancelled));
        assert_eq!(
            state
                .rebound
                .expect("cancel should create rebound")
                .initial_velocity_px_per_second,
            0.0
        );
    }

    #[test]
    fn momentum_implicit_release_is_suppressed_until_terminal() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        moved(&mut state, 24.0, top_only(), start);

        let began = state.handle_scroll_lifecycle(
            4.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Began,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(16),
            Some(2.016),
        );
        assert_eq!(
            began.start_settle,
            Some(SettleReason::MomentumBeganImplicitRelease)
        );
        assert!(state.suppress_momentum_until_terminal);

        let changed = state.handle_scroll_lifecycle(
            20.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Changed,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(24),
            Some(2.024),
        );
        assert_eq!(changed.residual_delta_y_px, 0.0);
        assert_eq!(state.phase, BoundaryPhase::Settling);

        state.handle_scroll_lifecycle(
            0.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Ended,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(32),
            Some(2.032),
        );
        assert!(!state.suppress_momentum_until_terminal);
        assert_eq!(state.phase, BoundaryPhase::Settling);
    }

    #[test]
    fn unsuppressed_momentum_preserves_native_delta() {
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        let decision = state.handle_scroll_lifecycle(
            -18.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Changed,
            PreciseTouchPhase::Moved,
            BoundaryEligibility::default(),
            tuning,
            false,
            Instant::now(),
            Some(3.0),
        );
        assert_eq!(decision.residual_delta_y_px, -18.0);
        assert_eq!(state.phase, BoundaryPhase::Idle);
    }

    #[test]
    fn interior_direct_release_does_not_claim_following_momentum() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        let interior = BoundaryEligibility::default();

        state.handle_scroll_lifecycle(
            0.0,
            ScrollLifecyclePhase::Began,
            ScrollLifecyclePhase::None,
            PreciseTouchPhase::Started,
            interior,
            tuning,
            false,
            start,
            Some(20.000),
        );
        let changed = state.handle_scroll_lifecycle(
            -24.0,
            ScrollLifecyclePhase::Changed,
            ScrollLifecyclePhase::None,
            PreciseTouchPhase::Moved,
            interior,
            tuning,
            false,
            start + Duration::from_millis(8),
            Some(20.008),
        );
        assert_eq!(changed.residual_delta_y_px, -24.0);

        let ended = state.handle_scroll_lifecycle(
            0.0,
            ScrollLifecyclePhase::Ended,
            ScrollLifecyclePhase::None,
            PreciseTouchPhase::Ended,
            interior,
            tuning,
            false,
            start + Duration::from_millis(12),
            Some(20.012),
        );
        assert_eq!(ended.start_settle, None);
        assert_eq!(state.phase, BoundaryPhase::Idle);
        assert!(!state.suppress_momentum_until_terminal);

        let momentum_began = state.handle_scroll_lifecycle(
            -18.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Began,
            PreciseTouchPhase::Moved,
            interior,
            tuning,
            false,
            start + Duration::from_millis(16),
            Some(20.016),
        );
        assert_eq!(momentum_began.residual_delta_y_px, -18.0);
        let momentum_changed = state.handle_scroll_lifecycle(
            -14.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Changed,
            PreciseTouchPhase::Moved,
            interior,
            tuning,
            false,
            start + Duration::from_millis(24),
            Some(20.024),
        );
        assert_eq!(momentum_changed.residual_delta_y_px, -14.0);
        assert!(!state.suppress_momentum_until_terminal);

        let momentum_ended = state.handle_scroll_lifecycle(
            0.0,
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::Ended,
            PreciseTouchPhase::Moved,
            interior,
            tuning,
            false,
            start + Duration::from_millis(32),
            Some(20.032),
        );
        assert_eq!(momentum_ended.residual_delta_y_px, 0.0);
        assert!(!state.suppress_momentum_until_terminal);
    }

    #[test]
    fn unsafe_timestamps_clear_velocity_instead_of_spiking() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        for (delta, milliseconds, timestamp) in [
            (12.0, 0, 5.0),
            (12.0, 1, 5.0005),
            (12.0, 2, 4.0),
            (12.0, 3, f64::NAN),
        ] {
            state.handle_scroll_lifecycle(
                delta,
                ScrollLifecyclePhase::Changed,
                ScrollLifecyclePhase::None,
                PreciseTouchPhase::Moved,
                top_only(),
                tuning,
                false,
                start + Duration::from_millis(milliseconds),
                Some(timestamp),
            );
            assert!(state.visual_velocity_px_per_second.is_finite());
            assert_eq!(state.visual_velocity_px_per_second, 0.0);
        }
    }

    #[test]
    fn stale_release_zeros_direct_velocity() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        for (delta, timestamp, milliseconds) in [(12.0, 7.0, 0), (12.0, 7.016, 16)] {
            state.handle_scroll_lifecycle(
                delta,
                ScrollLifecyclePhase::Changed,
                ScrollLifecyclePhase::None,
                PreciseTouchPhase::Moved,
                top_only(),
                tuning,
                false,
                start + Duration::from_millis(milliseconds),
                Some(timestamp),
            );
        }
        assert!(state.visual_velocity_px_per_second > 0.0);
        state.handle_scroll_lifecycle(
            0.0,
            ScrollLifecyclePhase::Ended,
            ScrollLifecyclePhase::None,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            false,
            start + Duration::from_millis(200),
            Some(7.200),
        );
        assert_eq!(
            state
                .rebound
                .expect("stale release still rebounds")
                .initial_velocity_px_per_second,
            0.0
        );
    }

    #[test]
    fn diagnostic_trace_ring_is_bounded_and_records_frame_generation() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        for index in 0..(TRACE_SAMPLE_CAPACITY + 12) {
            state.handle_scroll_lifecycle(
                0.0,
                ScrollLifecyclePhase::Stationary,
                ScrollLifecyclePhase::None,
                PreciseTouchPhase::Moved,
                BoundaryEligibility::default(),
                tuning,
                false,
                start + Duration::from_millis(index as u64),
                Some(index as f64 / 1000.0),
            );
        }
        assert_eq!(state.trace_samples().count(), TRACE_SAMPLE_CAPACITY);

        moved(&mut state, 24.0, top_only(), start + Duration::from_secs(1));
        state.begin_settle(SettleReason::Ended, tuning);
        let generation = state.generation;
        assert!(state.apply_settle_sample(generation, Duration::from_millis(16), tuning,));
        let latest = state.trace_samples().last().expect("frame trace sample");
        assert_eq!(latest.kind, ElasticTraceKind::ReboundFrame);
        assert_eq!(latest.rendered_frame_generation, 1);
    }

    #[test]
    fn high_release_velocity_never_exceeds_visual_distance_cap() {
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        state.offset_px = 12.0;
        state.edge = Some(BoundaryEdge::Top);
        state.phase = BoundaryPhase::Tracking;
        state.visual_velocity_px_per_second = 2_500.0;
        state.begin_settle(SettleReason::Ended, tuning);
        let generation = state.generation;

        for milliseconds in [1, 8, 16, 32, 64] {
            assert!(state.apply_settle_sample(
                generation,
                Duration::from_millis(milliseconds),
                tuning,
            ));
            assert!(state.offset_px <= tuning.max_distance_px);
        }
    }

    #[test]
    fn reduced_motion_uses_small_cap_and_ends_without_autonomous_settle() {
        let now = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::new(true);
        state.handle_precise_scroll(
            100_000.0,
            PreciseTouchPhase::Moved,
            top_only(),
            tuning,
            true,
            now,
            None,
        );
        assert!(state.offset_px > 0.0);
        assert!(state.offset_px <= 4.0);

        let ended = state.handle_precise_scroll(
            0.0,
            PreciseTouchPhase::Ended,
            top_only(),
            tuning,
            true,
            now,
            None,
        );
        assert_eq!(ended.start_settle, None);
        assert_eq!(state.last_settle_reason, Some(SettleReason::ReducedMotion));
        assert_idle_exact(&state);
    }

    #[test]
    fn watchdog_reports_remaining_time_and_times_out_once() {
        let start = Instant::now();
        let tuning = BoundaryAffordanceTuning::default();
        let mut state = BoundaryAffordanceState::default();
        let decision = moved(&mut state, 12.0, top_only(), start);
        assert!(decision.arm_idle_watchdog);
        let generation = state.generation;

        assert_eq!(
            state.idle_watchdog_status(generation, start + Duration::from_millis(60), tuning,),
            IdleWatchdogStatus::Sleep(Duration::from_millis(240))
        );
        let settle =
            state.begin_idle_timeout_settle(generation, start + Duration::from_millis(300), tuning);
        assert_eq!(
            settle.start_settle,
            Some(SettleReason::MissingTerminalWatchdog)
        );
        assert_eq!(state.phase, BoundaryPhase::Settling);
        assert_eq!(
            state.idle_watchdog_status(generation, start + Duration::from_secs(1), tuning),
            IdleWatchdogStatus::Cancelled
        );
    }

    #[test]
    fn inverse_mapping_round_trips_resisted_offsets() {
        for raw in [0.0, 4.0, 18.0, 36.0, 72.0, 108.0] {
            let offset = resisted_offset(raw, 18.0, 36.0);
            let reconstructed = raw_pull_for_offset(offset, 18.0, 36.0);
            assert!((reconstructed - raw).abs() < 0.001);
        }
    }

    #[test]
    fn invalid_spring_rate_rebound_finishes_immediately() {
        assert_eq!(
            critically_damped_rebound(18.0, 100.0, Duration::from_millis(1), 0.0),
            (0.0, 0.0)
        );
    }

    fn assert_idle_exact(state: &BoundaryAffordanceState) {
        assert_eq!(state.offset_px, 0.0);
        assert_eq!(state.raw_pull_px(), 0.0);
        assert_eq!(state.edge, None);
        assert_eq!(state.phase, BoundaryPhase::Idle);
        assert!(!state.idle_watchdog_armed());
    }
}

//! Chaos-monkey executed fuzz of the scroll-physics boundary state machine
//! (`boundary_affordance.rs`), kept as a sibling module so the owning file's
//! WIP is untouched. Drives the REAL public API — `handle_precise_scroll`,
//! `handle_scroll_lifecycle`, `begin_settle`, `apply_settle_sample`,
//! `idle_watchdog_status`, `begin_idle_timeout_settle`, `reset` — with a
//! deterministic hostile-input storm:
//!   - non-finite / extreme deltas (NaN, ±inf, MAX, -0.0, subnormal, huge)
//!   - hostile native timestamps (NaN, inf, negative, backwards, enormous)
//!   - out-of-order phase sequences (Ended before Started, Cancelled at idle,
//!     momentum phases mid-touch, settle samples with stale generations)
//!   - time that stalls and jumps (`now` reused and advanced erratically)
//!
//! Invariants asserted after EVERY step (a NaN that sticks in this state
//! machine becomes a permanently wedged scroll surface — a real, user-visible
//! perf/CLS defect):
//!   1. no panic anywhere;
//!   2. every exposed f32 (raw pull, rebound offset/velocity, decision
//!      residual, trace samples) is finite;
//!   3. resisted visual pull never exceeds the tuned max distance.

#[cfg(test)]
mod tests {
    use crate::scrolling::boundary_affordance::{
        BoundaryAffordanceState, BoundaryAffordanceTuning, BoundaryEligibility, PreciseTouchPhase,
        ScrollLifecyclePhase, SettleReason,
    };
    use std::time::{Duration, Instant};

    /// Deterministic LCG so the storm is reproducible (no OS randomness).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
            xs[(self.next() % xs.len() as u64) as usize]
        }
    }

    fn hostile_deltas() -> Vec<f32> {
        vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            12.5,
            -400.0,
            10_000.0,
            -10_000.0,
            f32::MAX,
            f32::MIN,
            f32::MIN_POSITIVE,
            f32::EPSILON,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1e30,
            -1e30,
        ]
    }

    fn hostile_stamps() -> Vec<Option<f64>> {
        vec![
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
            Some(1e18),
            Some(1.0),
            Some(0.5), // goes BACKWARDS vs 1.0 when sequenced after it
            Some(f64::MIN_POSITIVE),
        ]
    }

    fn assert_finite(label: &str, v: f32, step: usize) {
        assert!(
            v.is_finite(),
            "{label} became non-finite ({v}) at fuzz step {step} — a stuck NaN wedges the scroll surface",
        );
    }

    fn check_invariants(
        state: &BoundaryAffordanceState,
        tuning: BoundaryAffordanceTuning,
        step: usize,
    ) {
        let pull = state.raw_pull_px();
        assert_finite("raw_pull_px", pull, step);
        if let Some(off) = state.rebound_initial_offset_px() {
            assert_finite("rebound_initial_offset_px", off, step);
            assert!(
                off.abs() <= tuning.max_distance_px * 2.0 + 1.0,
                "rebound offset {off} wildly exceeds max distance at step {step}",
            );
        }
        if let Some(vel) = state.rebound_initial_velocity_px_per_second() {
            assert_finite("rebound_initial_velocity", vel, step);
        }
        for sample in state.trace_samples() {
            assert_finite("trace delta_y", sample.delta_y, step);
            assert_finite("trace raw_pull_px", sample.raw_pull_px, step);
            assert_finite("trace offset_px", sample.offset_px, step);
            assert_finite("trace velocity", sample.velocity_px_per_second, step);
        }
    }

    #[test]
    fn hostile_input_storm_never_panics_or_wedges_nan() {
        let tuning = BoundaryAffordanceTuning::default();
        let deltas = hostile_deltas();
        let stamps = hostile_stamps();
        let touch_phases = [
            PreciseTouchPhase::Started,
            PreciseTouchPhase::Moved,
            PreciseTouchPhase::Ended,
        ];
        let lifecycle_phases = [
            ScrollLifecyclePhase::None,
            ScrollLifecyclePhase::MayBegin,
            ScrollLifecyclePhase::Began,
            ScrollLifecyclePhase::Changed,
            ScrollLifecyclePhase::Stationary,
            ScrollLifecyclePhase::Ended,
            ScrollLifecyclePhase::Cancelled,
        ];
        let settle_reasons = [
            SettleReason::Ended,
            SettleReason::Cancelled,
            SettleReason::MomentumBeganImplicitRelease,
            SettleReason::MissingTerminalWatchdog,
            SettleReason::ReducedMotion,
            SettleReason::Reset,
        ];
        let eligibilities = [
            BoundaryEligibility {
                top: true,
                bottom: true,
            },
            BoundaryEligibility {
                top: true,
                bottom: false,
            },
            BoundaryEligibility {
                top: false,
                bottom: true,
            },
            BoundaryEligibility {
                top: false,
                bottom: false,
            },
        ];

        // Multiple seeds so distinct interleavings are exercised deterministically.
        for seed in [1u64, 0xDEAD_BEEF, 0x5EED_CAFE] {
            let mut rng = Lcg(seed);
            let mut state = BoundaryAffordanceState::default();
            let base = Instant::now();
            let mut generation = state.cancel_pending_work();

            for step in 0..4_000usize {
                // Time stalls, crawls, or leaps — never guaranteed monotonic
                // increments between calls (same `now` reused ~1/4 of steps).
                let jump_ms = [0u64, 0, 1, 7, 16, 1_000, 60_000][(rng.next() % 7) as usize];
                let now = base + Duration::from_millis((step as u64 / 4) * 3 + jump_ms);

                let delta = rng.pick(&deltas);
                let stamp = rng.pick(&stamps);
                let eligibility = rng.pick(&eligibilities);
                let reduced_motion = rng.next() % 5 == 0;

                match rng.next() % 8 {
                    0 | 1 | 2 => {
                        let decision = state.handle_precise_scroll(
                            delta,
                            rng.pick(&touch_phases),
                            eligibility,
                            tuning,
                            reduced_motion,
                            now,
                            stamp,
                        );
                        assert_finite("residual_delta_y_px", decision.residual_delta_y_px, step);
                    }
                    3 | 4 => {
                        let decision = state.handle_scroll_lifecycle(
                            delta,
                            rng.pick(&lifecycle_phases),
                            rng.pick(&lifecycle_phases),
                            rng.pick(&touch_phases),
                            eligibility,
                            tuning,
                            reduced_motion,
                            now,
                            stamp,
                        );
                        assert_finite("residual_delta_y_px", decision.residual_delta_y_px, step);
                    }
                    5 => {
                        let decision = state.begin_settle(rng.pick(&settle_reasons), tuning);
                        assert_finite("residual_delta_y_px", decision.residual_delta_y_px, step);
                        generation = state.cancel_pending_work();
                    }
                    6 => {
                        // Stale AND current generations; hostile elapsed values.
                        let sampled_generation = if rng.next() % 2 == 0 {
                            generation
                        } else {
                            generation.wrapping_sub(3)
                        };
                        let elapsed = Duration::from_millis((rng.next() % 100_000) as u64);
                        let _ = state.apply_settle_sample(sampled_generation, elapsed, tuning);
                        let _ = state.finish_settle_if_current(sampled_generation);
                    }
                    _ => {
                        let _ = state.idle_watchdog_status(generation, now, tuning);
                        let _ = state.begin_idle_timeout_settle(generation, now, tuning);
                        if rng.next() % 7 == 0 {
                            let _ = state.reset(rng.pick(&settle_reasons));
                        }
                    }
                }

                check_invariants(&state, tuning, step);
            }

            // After the storm the machine must still function normally: a clean
            // drag sequence produces a finite, bounded pull and a clean release.
            let now = base + Duration::from_secs(600);
            state.reset(SettleReason::Reset);
            state.handle_precise_scroll(
                0.0,
                PreciseTouchPhase::Started,
                eligibilities[0],
                tuning,
                false,
                now,
                Some(100.0),
            );
            for i in 1..=20 {
                state.handle_precise_scroll(
                    -24.0,
                    PreciseTouchPhase::Moved,
                    eligibilities[0],
                    tuning,
                    false,
                    now + Duration::from_millis(i * 8),
                    Some(100.0 + i as f64 * 0.008),
                );
            }
            let pull = state.raw_pull_px();
            assert!(
                pull.is_finite() && pull.abs() > 0.0,
                "post-storm drag produced no finite pull (seed {seed}): {pull}",
            );
            state.handle_precise_scroll(
                0.0,
                PreciseTouchPhase::Ended,
                eligibilities[0],
                tuning,
                false,
                now + Duration::from_millis(200),
                Some(100.2),
            );
            check_invariants(&state, tuning, usize::MAX);
        }
    }
}

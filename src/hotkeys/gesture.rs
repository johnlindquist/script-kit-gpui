//! Pure state machine for one-key gesture classification (tap, hold, double-tap).
//!
//! No I/O, no internal clock — callers inject `Instant` timestamps via key events
//! and periodic `poll` calls. See `docs/specs/gesture-grammar.md`.

use std::time::{Duration, Instant};

/// Default hold threshold before `HoldStart` fires while the key is still down.
pub const HOLD_MS: u64 = 250;
/// Default window after a tap's key-up in which a second key-down is a double-tap.
pub const DOUBLE_MS: u64 = 300;

/// Timing thresholds for gesture classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureConfig {
    pub hold_ms: u64,
    pub double_ms: u64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            hold_ms: HOLD_MS,
            double_ms: DOUBLE_MS,
        }
    }
}

impl GestureConfig {
    pub fn hold_duration(&self) -> Duration {
        Duration::from_millis(self.hold_ms)
    }

    pub fn double_duration(&self) -> Duration {
        Duration::from_millis(self.double_ms)
    }
}

/// Physical key transition with an injected timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureInput {
    KeyDown(Instant),
    KeyUp(Instant),
}

/// Classified gesture output events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureEvent {
    /// First key-down from the closed (window hidden) state — must fire synchronously.
    ShowImmediate,
    /// Short tap key-up while the window was already open.
    ///
    /// The final `Tap` still fires after the double-tap window unless routing
    /// consumes this preview and suppresses the delayed fallback.
    TapPreview,
    Tap,
    /// Fires at the hold threshold while the key is still down.
    HoldStart,
    /// Key-up after `HoldStart`.
    HoldEnd,
    /// Second key-down within the double-tap window after a quick release.
    DoubleTap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClassifierState {
    Closed,
    /// Window is visible and no gesture is in flight.
    Open,
    Pressed {
        down_at: Instant,
        /// Set when this press completes a double-tap — release goes straight to Open.
        completes_double_tap: bool,
        began_closed: bool,
    },
    HoldActive {
        down_at: Instant,
    },
    TapPending {
        released_at: Instant,
        /// `TapPreview` may hide the window before the double-tap window closes.
        window_hidden: bool,
    },
}

/// Pure gesture classifier — inject timestamps, poll for time-based transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GestureClassifier {
    config: GestureConfig,
    state: ClassifierState,
}

impl GestureClassifier {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            state: ClassifierState::Closed,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(GestureConfig::default())
    }

    pub fn config(&self) -> GestureConfig {
        self.config
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, ClassifierState::Closed)
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, ClassifierState::Open)
    }

    /// Window became visible through a non-gesture path (tray, deeplink, etc.).
    /// Keeps gesture state aligned so the next hotkey press does not re-emit
    /// `ShowImmediate` while the window is already on screen.
    pub fn sync_window_shown(&mut self) {
        self.state = match self.state {
            ClassifierState::Closed => ClassifierState::Open,
            ClassifierState::TapPending { released_at, .. } => ClassifierState::TapPending {
                released_at,
                window_hidden: false,
            },
            state => state,
        };
    }

    /// Window was dismissed — return to the closed steady state unless a
    /// `TapPreview` hide is still waiting for its double-tap deadline.
    pub fn sync_window_hidden(&mut self) {
        self.state = match self.state {
            ClassifierState::TapPending { released_at, .. } => ClassifierState::TapPending {
                released_at,
                window_hidden: true,
            },
            _ => ClassifierState::Closed,
        };
    }

    /// Next instant at which `poll` may emit `HoldStart` or `Tap`. `None` when idle.
    ///
    /// Must stay consistent with `poll`: a state that `poll` cannot advance
    /// must report NO deadline. A press that completes a double-tap never
    /// emits `HoldStart`, so it must not report the hold deadline — doing so
    /// made the gesture listener spin at 100% CPU (timer fires instantly,
    /// poll returns None, state unchanged, repeat forever) whenever the
    /// matching key-up was missed.
    pub fn next_deadline(&self) -> Option<Instant> {
        match self.state {
            ClassifierState::Pressed {
                down_at,
                completes_double_tap,
                ..
            } => {
                if completes_double_tap {
                    None
                } else {
                    Some(down_at + self.config.hold_duration())
                }
            }
            ClassifierState::TapPending { released_at, .. } => {
                Some(released_at + self.config.double_duration())
            }
            ClassifierState::Closed
            | ClassifierState::Open
            | ClassifierState::HoldActive { .. } => None,
        }
    }

    /// Handle a key-down or key-up event. Time-based events require `poll`.
    pub fn handle_input(&mut self, input: GestureInput) -> Vec<GestureEvent> {
        let at = match input {
            GestureInput::KeyDown(at) | GestureInput::KeyUp(at) => at,
        };
        let mut events = Vec::new();
        // The receiver may win its race with the deadline timer. Settle only
        // strictly overdue transitions first so the exact double-tap boundary
        // remains inclusive for the physical input.
        if self.next_deadline().is_some_and(|deadline| deadline < at) {
            if let Some(event) = self.poll(at) {
                events.push(event);
            }
        }
        events.extend(match input {
            GestureInput::KeyDown(at) => self.on_key_down(at),
            GestureInput::KeyUp(at) => self.on_key_up(at),
        });
        events
    }

    /// Advance time-based transitions: `HoldStart` while pressed, `Tap` after double window.
    pub fn poll(&mut self, now: Instant) -> Option<GestureEvent> {
        match self.state {
            ClassifierState::Pressed {
                down_at,
                completes_double_tap,
                ..
            } => {
                if now >= down_at + self.config.hold_duration() && !completes_double_tap {
                    self.state = ClassifierState::HoldActive { down_at };
                    Some(GestureEvent::HoldStart)
                } else {
                    None
                }
            }
            ClassifierState::TapPending {
                released_at,
                window_hidden,
            } => {
                if now >= released_at + self.config.double_duration() {
                    self.state = if window_hidden {
                        ClassifierState::Closed
                    } else {
                        ClassifierState::Open
                    };
                    Some(GestureEvent::Tap)
                } else {
                    None
                }
            }
            ClassifierState::Closed
            | ClassifierState::Open
            | ClassifierState::HoldActive { .. } => None,
        }
    }

    fn on_key_down(&mut self, at: Instant) -> Vec<GestureEvent> {
        match self.state {
            ClassifierState::Closed => {
                self.state = ClassifierState::Pressed {
                    down_at: at,
                    completes_double_tap: false,
                    began_closed: true,
                };
                vec![GestureEvent::ShowImmediate]
            }
            ClassifierState::Open => {
                self.state = ClassifierState::Pressed {
                    down_at: at,
                    completes_double_tap: false,
                    began_closed: false,
                };
                Vec::new()
            }
            ClassifierState::TapPending { released_at, .. } => {
                if at <= released_at + self.config.double_duration() {
                    self.state = ClassifierState::Pressed {
                        down_at: at,
                        completes_double_tap: true,
                        began_closed: false,
                    };
                    vec![GestureEvent::DoubleTap]
                } else {
                    // Late second press: prior tap window already resolved via poll in
                    // well-behaved callers; treat this as a fresh open-state press.
                    self.state = ClassifierState::Pressed {
                        down_at: at,
                        completes_double_tap: false,
                        began_closed: false,
                    };
                    Vec::new()
                }
            }
            ClassifierState::Pressed { .. } | ClassifierState::HoldActive { .. } => Vec::new(),
        }
    }

    fn on_key_up(&mut self, at: Instant) -> Vec<GestureEvent> {
        match self.state {
            ClassifierState::Pressed {
                down_at,
                completes_double_tap,
                began_closed,
            } => {
                if completes_double_tap {
                    self.state = ClassifierState::Open;
                    return Vec::new();
                }
                let elapsed = at.saturating_duration_since(down_at);
                if elapsed < self.config.hold_duration() {
                    self.state = ClassifierState::TapPending {
                        released_at: at,
                        window_hidden: false,
                    };
                    if began_closed {
                        Vec::new()
                    } else {
                        vec![GestureEvent::TapPreview]
                    }
                } else {
                    // Missed poll while still down — synthesize hold completion on release.
                    self.state = ClassifierState::Open;
                    vec![GestureEvent::HoldStart, GestureEvent::HoldEnd]
                }
            }
            ClassifierState::HoldActive { .. } => {
                self.state = ClassifierState::Open;
                vec![GestureEvent::HoldEnd]
            }
            ClassifierState::Closed
            | ClassifierState::Open
            | ClassifierState::TapPending { .. } => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Instant {
        Instant::now()
    }

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    fn poll_at(
        classifier: &mut GestureClassifier,
        origin: Instant,
        ms: u64,
    ) -> Option<GestureEvent> {
        classifier.poll(at(origin, ms))
    }

    fn drain_until(
        classifier: &mut GestureClassifier,
        origin: Instant,
        ms: u64,
    ) -> Vec<GestureEvent> {
        let mut events = Vec::new();
        let now = at(origin, ms);
        while let Some(deadline) = classifier.next_deadline() {
            if now < deadline {
                break;
            }
            if let Some(event) = classifier.poll(now) {
                events.push(event);
            } else {
                break;
            }
        }
        events
    }

    #[test]
    fn tap_from_closed_emits_show_immediate_then_tap() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        let down = classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        assert_eq!(down, vec![GestureEvent::ShowImmediate]);

        let up = classifier.handle_input(GestureInput::KeyUp(at(origin, 50)));
        assert!(up.is_empty());

        let tap = poll_at(&mut classifier, origin, 350);
        assert_eq!(tap, Some(GestureEvent::Tap));
        assert!(classifier.is_open());
    }

    #[test]
    fn open_short_tap_emits_preview_then_final_tap() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.sync_window_shown();
        assert!(classifier
            .handle_input(GestureInput::KeyDown(at(origin, 0)))
            .is_empty());

        assert_eq!(
            classifier.handle_input(GestureInput::KeyUp(at(origin, 40))),
            vec![GestureEvent::TapPreview]
        );

        assert_eq!(
            poll_at(&mut classifier, origin, 340),
            Some(GestureEvent::Tap)
        );
    }

    #[test]
    fn closed_opening_tap_does_not_emit_preview() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 0))),
            vec![GestureEvent::ShowImmediate]
        );

        assert!(classifier
            .handle_input(GestureInput::KeyUp(at(origin, 40)))
            .is_empty());

        assert_eq!(
            poll_at(&mut classifier, origin, 340),
            Some(GestureEvent::Tap)
        );
    }

    #[test]
    fn of40_double_tap_matrix_preserves_preview_across_hide() {
        struct Case {
            name: &'static str,
            starts_closed: bool,
            second_down_after_release_ms: u64,
        }

        let cases = [
            Case {
                name: "closed-clean-chat-control",
                starts_closed: true,
                second_down_after_release_ms: 60,
            },
            Case {
                name: "open-preview-hide-60ms",
                starts_closed: false,
                second_down_after_release_ms: 60,
            },
            Case {
                name: "open-preview-hide-299ms",
                starts_closed: false,
                second_down_after_release_ms: 299,
            },
            Case {
                name: "open-preview-hide-inclusive-300ms",
                starts_closed: false,
                second_down_after_release_ms: 300,
            },
        ];

        for case in cases {
            let mut classifier = GestureClassifier::with_defaults();
            let origin = base();
            if !case.starts_closed {
                classifier.sync_window_shown();
            }

            let first_down = classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
            assert_eq!(
                first_down,
                if case.starts_closed {
                    vec![GestureEvent::ShowImmediate]
                } else {
                    Vec::new()
                },
                "{}: first down",
                case.name
            );

            let first_up = classifier.handle_input(GestureInput::KeyUp(at(origin, 40)));
            if case.starts_closed {
                assert!(first_up.is_empty(), "{}: first up", case.name);
            } else {
                assert_eq!(
                    first_up,
                    vec![GestureEvent::TapPreview],
                    "{}: first up",
                    case.name
                );
                classifier.sync_window_hidden();
            }

            let second_down_at = 40 + case.second_down_after_release_ms;
            assert_eq!(
                classifier.handle_input(GestureInput::KeyDown(at(origin, second_down_at))),
                vec![GestureEvent::DoubleTap],
                "{}: second down must deepen the opening gesture",
                case.name
            );
            assert!(classifier
                .handle_input(GestureInput::KeyUp(at(origin, second_down_at + 40)))
                .is_empty());
            assert!(
                drain_until(&mut classifier, origin, 1_000).is_empty(),
                "{}: completed double tap must have no delayed fallback",
                case.name
            );
            assert!(classifier.is_open(), "{}: completed double tap", case.name);
        }
    }

    #[test]
    fn of40_preview_hide_expiry_rearms_fresh_opening_tap() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.sync_window_shown();
        assert!(classifier
            .handle_input(GestureInput::KeyDown(at(origin, 0)))
            .is_empty());
        assert_eq!(
            classifier.handle_input(GestureInput::KeyUp(at(origin, 40))),
            vec![GestureEvent::TapPreview]
        );
        classifier.sync_window_hidden();

        assert_eq!(
            poll_at(&mut classifier, origin, 340),
            Some(GestureEvent::Tap)
        );
        assert!(classifier.is_closed());
        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 400))),
            vec![GestureEvent::ShowImmediate]
        );
    }

    #[test]
    fn of40_hold_and_tap_boundaries_use_exact_injected_instants() {
        let origin = base();

        let mut held = GestureClassifier::with_defaults();
        assert_eq!(
            held.handle_input(GestureInput::KeyDown(at(origin, 0))),
            vec![GestureEvent::ShowImmediate]
        );
        assert!(poll_at(&mut held, origin, 249).is_none());
        assert_eq!(held.next_deadline(), Some(at(origin, 250)));
        assert_eq!(
            held.handle_input(GestureInput::KeyUp(at(origin, 250))),
            vec![GestureEvent::HoldStart, GestureEvent::HoldEnd]
        );

        let mut tapped = GestureClassifier::with_defaults();
        tapped.handle_input(GestureInput::KeyDown(at(origin, 0)));
        assert!(tapped
            .handle_input(GestureInput::KeyUp(at(origin, 249)))
            .is_empty());
        assert!(poll_at(&mut tapped, origin, 548).is_none());
        assert_eq!(poll_at(&mut tapped, origin, 549), Some(GestureEvent::Tap));
    }

    #[test]
    fn slow_release_after_hold_is_hold_not_tap() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 0))),
            vec![GestureEvent::ShowImmediate]
        );

        assert_eq!(
            poll_at(&mut classifier, origin, 250),
            Some(GestureEvent::HoldStart)
        );

        assert_eq!(
            classifier.handle_input(GestureInput::KeyUp(at(origin, 400))),
            vec![GestureEvent::HoldEnd]
        );

        assert!(drain_until(&mut classifier, origin, 700).is_empty());
        assert!(classifier.is_open());
    }

    #[test]
    fn double_tap_from_closed() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 0))),
            vec![GestureEvent::ShowImmediate]
        );
        classifier.handle_input(GestureInput::KeyUp(at(origin, 40)));

        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 120))),
            vec![GestureEvent::DoubleTap]
        );

        classifier.handle_input(GestureInput::KeyUp(at(origin, 160)));
        assert!(poll_at(&mut classifier, origin, 500).is_none());
        assert!(classifier.is_open());
    }

    #[test]
    fn adjacent_late_second_down_delivers_expired_tap_before_fresh_press() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 0))),
            vec![GestureEvent::ShowImmediate]
        );
        assert!(classifier
            .handle_input(GestureInput::KeyUp(at(origin, 40)))
            .is_empty());

        assert_eq!(
            classifier.handle_input(GestureInput::KeyDown(at(origin, 341))),
            vec![GestureEvent::Tap],
            "a +301ms receiver event must first deliver the overdue opening Tap"
        );
        assert_eq!(
            classifier.handle_input(GestureInput::KeyUp(at(origin, 381))),
            vec![GestureEvent::TapPreview]
        );
        assert_eq!(
            poll_at(&mut classifier, origin, 681),
            Some(GestureEvent::Tap)
        );
    }

    #[test]
    fn key_repeat_storms_are_ignored() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        for ms in (30..200).step_by(20) {
            assert!(classifier
                .handle_input(GestureInput::KeyDown(at(origin, ms)))
                .is_empty());
        }

        assert_eq!(
            poll_at(&mut classifier, origin, 250),
            Some(GestureEvent::HoldStart)
        );

        for ms in (260..400).step_by(20) {
            assert!(classifier
                .handle_input(GestureInput::KeyDown(at(origin, ms)))
                .is_empty());
        }

        assert_eq!(
            classifier.handle_input(GestureInput::KeyUp(at(origin, 420))),
            vec![GestureEvent::HoldEnd]
        );
    }

    #[test]
    fn hold_then_immediate_retap_is_tap_not_double() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        poll_at(&mut classifier, origin, 250);
        classifier.handle_input(GestureInput::KeyUp(at(origin, 300)));

        assert!(classifier
            .handle_input(GestureInput::KeyDown(at(origin, 310)))
            .is_empty());
        classifier.handle_input(GestureInput::KeyUp(at(origin, 350)));
        assert_eq!(
            poll_at(&mut classifier, origin, 650),
            Some(GestureEvent::Tap)
        );
    }

    #[test]
    fn show_immediate_only_from_closed() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        classifier.handle_input(GestureInput::KeyUp(at(origin, 40)));
        poll_at(&mut classifier, origin, 340);

        assert!(classifier.is_open());
        assert!(classifier
            .handle_input(GestureInput::KeyDown(at(origin, 400)))
            .is_empty());
    }

    #[test]
    fn custom_config_thresholds() {
        let config = GestureConfig {
            hold_ms: 100,
            double_ms: 150,
        };
        let mut classifier = GestureClassifier::new(config);
        let origin = base();

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        classifier.handle_input(GestureInput::KeyUp(at(origin, 30)));
        assert_eq!(
            poll_at(&mut classifier, origin, 180),
            Some(GestureEvent::Tap)
        );

        classifier.handle_input(GestureInput::KeyDown(at(origin, 500)));
        assert_eq!(
            poll_at(&mut classifier, origin, 600),
            Some(GestureEvent::HoldStart)
        );
    }

    #[test]
    fn next_deadline_tracks_hold_and_double_windows() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        assert!(classifier.next_deadline().is_none());

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        assert_eq!(classifier.next_deadline(), Some(at(origin, HOLD_MS)));

        classifier.handle_input(GestureInput::KeyUp(at(origin, 50)));
        assert_eq!(classifier.next_deadline(), Some(at(origin, 50 + DOUBLE_MS)));
    }

    #[test]
    fn sync_window_shown_skips_show_immediate_on_next_press() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        classifier.handle_input(GestureInput::KeyUp(at(origin, 40)));
        poll_at(&mut classifier, origin, 340);
        assert!(classifier.is_open());

        classifier.sync_window_hidden();
        classifier.sync_window_shown();
        assert!(classifier.is_open());

        assert!(classifier
            .handle_input(GestureInput::KeyDown(at(origin, 400)))
            .is_empty());
    }

    #[test]
    fn release_exactly_at_hold_boundary_without_poll_synthesizes_hold() {
        let mut classifier = GestureClassifier::with_defaults();
        let origin = base();

        classifier.handle_input(GestureInput::KeyDown(at(origin, 0)));
        let events = classifier.handle_input(GestureInput::KeyUp(at(origin, 250)));
        assert_eq!(events, vec![GestureEvent::HoldStart, GestureEvent::HoldEnd]);
    }
}

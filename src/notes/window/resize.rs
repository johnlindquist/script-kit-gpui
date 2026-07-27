use super::*;

/// Lifecycle interlock for native (all-edge) user resizing of the Notes shell.
///
/// The Notes shell policy (`window_resize::policy`) says the user MAY resize;
/// this phase says WHEN: the resizable style bit is only set while `Enabled`.
/// The calibrated glass entry morph owns the frame until the full settle
/// duration has elapsed (`EntryLocked`), and the fixed-frame exit fade owns it
/// from the moment an exit ticket is issued (`ExitLocked`). Unlocking at the
/// earlier body-reveal crossing would let a user drag fight the rebound — the
/// unlock anchor is `configured_at + settle_duration_ms`, NOT the reveal
/// crossing. No glass motion constant is read or written here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum NotesNativeResizePhase {
    /// Window created / entry morph in flight: native resizing off.
    #[default]
    EntryLocked,
    /// Entry settled: native resizing on (policy minimums enforced by AppKit).
    Enabled,
    /// Exit fade in flight: native resizing off; the current frame is final.
    ExitLocked,
}

impl NotesNativeResizePhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::EntryLocked => "entryLocked",
            Self::Enabled => "enabled",
            Self::ExitLocked => "exitLocked",
        }
    }
}

/// Pure transition table for the interlock — kept separate from the AppKit
/// side effects so the ordering contract is unit-testable.
pub(super) fn next_resize_phase(
    phase: NotesNativeResizePhase,
    event: NotesResizePhaseEvent,
) -> Option<NotesNativeResizePhase> {
    use NotesNativeResizePhase as Phase;
    use NotesResizePhaseEvent as Event;
    match (phase, event) {
        // Unlock is only meaningful from the entry-locked state; a stale
        // scheduled unlock arriving after an exit began must not re-enable a
        // window that is fading out.
        (Phase::EntryLocked, Event::EntrySettled) => Some(Phase::Enabled),
        (Phase::EntryLocked | Phase::Enabled, Event::ExitStarted) => Some(Phase::ExitLocked),
        // A superseded exit on a still-visible window restores resizability
        // immediately; a restart-hidden supersede re-enters the entry lock and
        // waits for the normal entry unlock.
        (Phase::ExitLocked, Event::ExitSupersededPreserveVisible) => Some(Phase::Enabled),
        (Phase::ExitLocked, Event::ExitSupersededRestartHidden) => Some(Phase::EntryLocked),
        // Everything else is a no-op: stale timers, duplicate exits, or
        // supersede events outside an exit.
        (Phase::Enabled | Phase::ExitLocked, Event::EntrySettled)
        | (Phase::ExitLocked, Event::ExitStarted)
        | (
            Phase::EntryLocked | Phase::Enabled,
            Event::ExitSupersededPreserveVisible | Event::ExitSupersededRestartHidden,
        ) => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotesResizePhaseEvent {
    EntrySettled,
    ExitStarted,
    ExitSupersededPreserveVisible,
    ExitSupersededRestartHidden,
}

/// Monotonic time at which native resizing may unlock: the FULL settle
/// duration after native configuration. Deliberately NOT the body-reveal
/// crossing (`settled_crossing_delay_ms`) — the reveal starts while the glass
/// morph is still compressing/rebounding.
pub(super) const fn native_resize_unlock_target_ns(
    configured_at_monotonic_ns: u64,
    settle_duration_ms: u64,
) -> u64 {
    configured_at_monotonic_ns.saturating_add(settle_duration_ms.saturating_mul(1_000_000))
}

impl NotesApp {
    /// The Notes shell's authored resize policy.
    pub(super) fn notes_shell_resize_policy() -> crate::window_resize::policy::WindowResizePolicy {
        crate::window_resize::policy::resize_policy(
            crate::window_resize::policy::WindowShellKind::Notes,
        )
    }

    pub(super) fn native_resize_enabled(&self) -> bool {
        self.native_resize_phase == NotesNativeResizePhase::Enabled
    }

    fn apply_resize_phase_transition(
        &mut self,
        event: NotesResizePhaseEvent,
        window: &Window,
        reason: &'static str,
    ) -> bool {
        let Some(next) = next_resize_phase(self.native_resize_phase, event) else {
            return false;
        };
        let before = self.native_resize_phase;
        self.native_resize_phase = next;
        let interaction_enabled = next == NotesNativeResizePhase::Enabled;
        let applied = crate::platform::apply_window_resize_policy(
            window,
            Self::notes_shell_resize_policy(),
            interaction_enabled,
        );
        tracing::info!(
            target: "notes",
            event = "notes_native_resize_phase_transition",
            reason,
            phase_before = before.as_str(),
            phase_after = next.as_str(),
            interaction_enabled,
            native_apply_ok = applied,
        );
        true
    }

    /// Enable native resizing once the calibrated entry morph has fully
    /// settled. No-op unless the interlock is still `EntryLocked` (stale
    /// timers, superseded windows, and active exits are all rejected by the
    /// transition table).
    pub(super) fn unlock_native_resize_after_entry(&mut self, window: &Window) -> bool {
        self.apply_resize_phase_transition(
            NotesResizePhaseEvent::EntrySettled,
            window,
            "entry_settled",
        )
    }

    /// Lock native resizing before a Notes exit begins so the user-selected
    /// frame becomes the fixed exit frame; edge drags cannot fight the fade.
    pub(super) fn lock_native_resize_for_exit(&mut self, window: &Window) -> bool {
        self.apply_resize_phase_transition(
            NotesResizePhaseEvent::ExitStarted,
            window,
            "exit_started",
        )
    }

    /// Restore the interlock after a close was superseded by a rapid reopen.
    /// `preserved_visible` mirrors `NotesExitRevealDisposition`.
    pub(super) fn restore_native_resize_after_exit_supersede(
        &mut self,
        preserved_visible: bool,
        window: &Window,
    ) -> bool {
        let event = if preserved_visible {
            NotesResizePhaseEvent::ExitSupersededPreserveVisible
        } else {
            NotesResizePhaseEvent::ExitSupersededRestartHidden
        };
        self.apply_resize_phase_transition(event, window, "exit_superseded")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum NotesBottomResizeRoute {
    IgnoredNonLeftButton,
    /// AppKit's native resizable frame owns edge tracking while the interlock
    /// is `Enabled`; the custom classifier must not race it.
    NativeResizeOwned,
    OutsideBottomEdge,
    CornerGuard,
    EntryMotionActive,
    ExitMotionActive,
    OverlayActive,
    MissingFooterGeometry,
    ProtectedFooterButton {
        group: &'static str,
        index: usize,
    },
    ResizeStarted,
}

impl NotesBottomResizeRoute {
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            Self::IgnoredNonLeftButton => "ignoredNonLeftButton",
            Self::NativeResizeOwned => "nativeResizeOwned",
            Self::OutsideBottomEdge => "outsideBottomEdge",
            Self::CornerGuard => "cornerGuard",
            Self::EntryMotionActive => "entryMotionActive",
            Self::ExitMotionActive => "exitMotionActive",
            Self::OverlayActive => "overlayActive",
            Self::MissingFooterGeometry => "missingFooterGeometry",
            Self::ProtectedFooterButton { .. } => "protectedFooterButton",
            Self::ResizeStarted => "resizeStarted",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct NotesBottomResizeReceipt {
    pub(super) route: NotesBottomResizeRoute,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) before_width: f32,
    pub(super) before_height: f32,
    pub(super) after_width: f32,
    pub(super) after_height: f32,
    pub(super) footer_layout_generation: Option<u64>,
    pub(super) native_window_number: Option<i64>,
    pub(super) recorded_at: Instant,
}

fn classify_bottom_resize(
    button: gpui::MouseButton,
    point: gpui::Point<gpui::Pixels>,
    window_size: gpui::Size<gpui::Pixels>,
    entry_motion_complete: bool,
    exit_motion_active: bool,
    overlay_active: bool,
    footer: Option<&crate::platform::footer_hit_regions::FooterHitRegionSnapshot>,
) -> NotesBottomResizeRoute {
    if button != gpui::MouseButton::Left {
        return NotesBottomResizeRoute::IgnoredNonLeftButton;
    }

    let x = point.x.as_f32();
    let y = point.y.as_f32();
    let width = window_size.width.as_f32();
    let height = window_size.height.as_f32();
    if y < height - contract::NOTES_BOTTOM_RESIZE_HIT_HEIGHT_PX || y > height {
        return NotesBottomResizeRoute::OutsideBottomEdge;
    }
    if x < contract::NOTES_BOTTOM_RESIZE_CORNER_GUARD_PX
        || x > width - contract::NOTES_BOTTOM_RESIZE_CORNER_GUARD_PX
    {
        return NotesBottomResizeRoute::CornerGuard;
    }
    if !entry_motion_complete {
        return NotesBottomResizeRoute::EntryMotionActive;
    }
    if exit_motion_active {
        return NotesBottomResizeRoute::ExitMotionActive;
    }
    if overlay_active {
        return NotesBottomResizeRoute::OverlayActive;
    }

    let Some(footer) = footer else {
        return NotesBottomResizeRoute::MissingFooterGeometry;
    };
    for region in footer.regions.iter().copied().filter(|region| {
        region.intersects_bottom_band(height, contract::NOTES_BOTTOM_RESIZE_HIT_HEIGHT_PX)
    }) {
        if region.contains_with_guard(point, contract::NOTES_BOTTOM_RESIZE_BUTTON_GUARD_PX) {
            return NotesBottomResizeRoute::ProtectedFooterButton {
                group: region.group,
                index: region.index,
            };
        }
    }

    NotesBottomResizeRoute::ResizeStarted
}

impl NotesApp {
    fn entry_motion_complete_for_resize(&self) -> bool {
        let Some(configured_at) = self.entry_reveal.configured_at_monotonic_ns else {
            return false;
        };
        crate::platform::host_clock::host_time_ns()
            >= configured_at.saturating_add(
                self.entry_reveal
                    .settle_duration_ms
                    .saturating_mul(1_000_000),
            )
    }

    pub(super) fn handle_bottom_resize_mouse_down(
        &mut self,
        event: &gpui::MouseDownEvent,
        overlay_was_active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Native and custom drag handlers must never run simultaneously: while
        // the interlock is Enabled, AppKit's resizable frame owns every edge
        // and corner; the bottom-band fallback stays available for the locked
        // phases and for platforms where the native probe fails.
        if self.native_resize_enabled() {
            self.last_bottom_resize_receipt = Some(NotesBottomResizeReceipt {
                route: NotesBottomResizeRoute::NativeResizeOwned,
                x: event.position.x.as_f32(),
                y: event.position.y.as_f32(),
                before_width: window.bounds().size.width.as_f32(),
                before_height: window.bounds().size.height.as_f32(),
                after_width: window.bounds().size.width.as_f32(),
                after_height: window.bounds().size.height.as_f32(),
                footer_layout_generation: None,
                native_window_number: self.entry_reveal.native_window_number,
                recorded_at: Instant::now(),
            });
            return;
        }

        let before = window.bounds().size;
        let footer = crate::platform::footer_hit_regions::snapshot_for_window(window);
        let exit_motion_active = NOTES_EXIT_TICKET
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .is_some();
        let route = classify_bottom_resize(
            event.button,
            event.position,
            before,
            self.entry_motion_complete_for_resize(),
            exit_motion_active,
            overlay_was_active,
            footer.as_ref(),
        );
        let layout_generation = footer.as_ref().map(|snapshot| snapshot.layout_generation);
        let native_window_number = footer
            .as_ref()
            .map(|snapshot| snapshot.native_window_number)
            .or(self.entry_reveal.native_window_number);

        let should_resize = route == NotesBottomResizeRoute::ResizeStarted;
        if should_resize {
            cx.stop_propagation();
        }

        self.last_bottom_resize_receipt = Some(NotesBottomResizeReceipt {
            route: route.clone(),
            x: event.position.x.as_f32(),
            y: event.position.y.as_f32(),
            before_width: before.width.as_f32(),
            before_height: before.height.as_f32(),
            after_width: before.width.as_f32(),
            after_height: before.height.as_f32(),
            footer_layout_generation: layout_generation,
            native_window_number,
            recorded_at: Instant::now(),
        });

        tracing::info!(
            target: "notes",
            event = "notes_bottom_resize_pointer_down",
            route = route.as_str(),
            x = event.position.x.as_f32(),
            y = event.position.y.as_f32(),
            before_width = before.width.as_f32(),
            before_height = before.height.as_f32(),
            footer_layout_generation = layout_generation,
            native_window_number,
        );

        if !should_resize {
            return;
        }

        window.start_window_resize(gpui::ResizeEdge::Bottom);
    }

    pub(super) fn refresh_bottom_resize_observation(&mut self, window: &Window) {
        let Some(receipt) = self.last_bottom_resize_receipt.as_mut() else {
            return;
        };
        if receipt.route != NotesBottomResizeRoute::ResizeStarted {
            return;
        }

        let bounds = window.bounds();
        let after = bounds.size;
        let changed = receipt.after_width != after.width.as_f32()
            || receipt.after_height != after.height.as_f32();
        if !changed {
            return;
        }

        if let Some(receipt) = self.last_bottom_resize_receipt.as_mut() {
            receipt.after_width = after.width.as_f32();
            receipt.after_height = after.height.as_f32();
        }
        crate::windows::set_automation_bounds(
            "notes",
            Some(crate::protocol::AutomationWindowBounds {
                x: bounds.origin.x.as_f32() as f64,
                y: bounds.origin.y.as_f32() as f64,
                width: bounds.size.width.as_f32() as f64,
                height: bounds.size.height.as_f32() as f64,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::footer_hit_regions::{FooterHitRegion, FooterHitRegionSnapshot};

    fn size() -> gpui::Size<gpui::Pixels> {
        gpui::size(gpui::px(350.0), gpui::px(280.0))
    }

    fn snapshot(regions: Vec<FooterHitRegion>) -> FooterHitRegionSnapshot {
        FooterHitRegionSnapshot {
            native_window_number: 42,
            window_width: 350.0,
            window_height: 280.0,
            layout_generation: 7,
            regions,
        }
    }

    fn route(x: f32, y: f32, footer: Option<&FooterHitRegionSnapshot>) -> NotesBottomResizeRoute {
        classify_bottom_resize(
            gpui::MouseButton::Left,
            gpui::point(gpui::px(x), gpui::px(y)),
            size(),
            true,
            false,
            false,
            footer,
        )
    }

    #[test]
    fn bottom_edge_gap_starts_resize() {
        let footer = snapshot(vec![FooterHitRegion {
            group: "notes-footer",
            index: 0,
            x: 250.0,
            y: 250.0,
            width: 80.0,
            height: 28.0,
        }]);
        assert_eq!(
            route(120.0, 279.0, Some(&footer)),
            NotesBottomResizeRoute::ResizeStarted
        );
    }

    #[test]
    fn floating_button_and_guard_never_start_resize() {
        let footer = snapshot(vec![FooterHitRegion {
            group: "notes-footer",
            index: 2,
            x: 250.0,
            y: 250.0,
            width: 80.0,
            height: 28.0,
        }]);
        assert_eq!(
            route(260.0, 277.0, Some(&footer)),
            NotesBottomResizeRoute::ProtectedFooterButton {
                group: "notes-footer",
                index: 2
            }
        );
        assert!(matches!(
            route(249.5, 277.0, Some(&footer)),
            NotesBottomResizeRoute::ProtectedFooterButton { .. }
        ));
    }

    #[test]
    fn native_resize_unlock_waits_for_full_entry_settle() {
        use NotesNativeResizePhase as Phase;
        use NotesResizePhaseEvent as Event;

        // The unlock event only fires from the entry lock; stale timers after
        // an exit began (or after an earlier unlock) are rejected.
        assert_eq!(
            next_resize_phase(Phase::EntryLocked, Event::EntrySettled),
            Some(Phase::Enabled)
        );
        assert_eq!(next_resize_phase(Phase::Enabled, Event::EntrySettled), None);
        assert_eq!(
            next_resize_phase(Phase::ExitLocked, Event::EntrySettled),
            None
        );

        // The unlock anchor is the FULL settle duration, not the earlier
        // body-reveal crossing. With the production calibration (280ms settle,
        // ~97ms crossing) the unlock lands strictly after the reveal anchor.
        let configured_at = 1_000_000_000;
        let settle_ms = 280;
        let reveal_crossing_ms = 97;
        let unlock = native_resize_unlock_target_ns(configured_at, settle_ms);
        assert_eq!(unlock, configured_at + 280_000_000);
        assert!(unlock > configured_at + reveal_crossing_ms * 1_000_000);
    }

    #[test]
    fn native_resize_locks_before_notes_exit() {
        use NotesNativeResizePhase as Phase;
        use NotesResizePhaseEvent as Event;

        // An exit locks resizing from either live phase; a duplicate exit
        // event is a no-op.
        assert_eq!(
            next_resize_phase(Phase::EntryLocked, Event::ExitStarted),
            Some(Phase::ExitLocked)
        );
        assert_eq!(
            next_resize_phase(Phase::Enabled, Event::ExitStarted),
            Some(Phase::ExitLocked)
        );
        assert_eq!(
            next_resize_phase(Phase::ExitLocked, Event::ExitStarted),
            None
        );
    }

    #[test]
    fn exit_supersede_restores_resize_only_for_live_window() {
        use NotesNativeResizePhase as Phase;
        use NotesResizePhaseEvent as Event;

        // PreserveVisible: the window stayed at its user frame — resizable
        // again immediately. RestartHidden: re-enter the entry lock and wait
        // for the normal entry unlock.
        assert_eq!(
            next_resize_phase(Phase::ExitLocked, Event::ExitSupersededPreserveVisible),
            Some(Phase::Enabled)
        );
        assert_eq!(
            next_resize_phase(Phase::ExitLocked, Event::ExitSupersededRestartHidden),
            Some(Phase::EntryLocked)
        );
        // Supersede events outside an exit are stale and change nothing.
        for phase in [Phase::EntryLocked, Phase::Enabled] {
            assert_eq!(
                next_resize_phase(phase, Event::ExitSupersededPreserveVisible),
                None
            );
            assert_eq!(
                next_resize_phase(phase, Event::ExitSupersededRestartHidden),
                None
            );
        }
    }

    #[test]
    fn missing_geometry_and_motion_fail_closed() {
        assert_eq!(
            route(120.0, 279.0, None),
            NotesBottomResizeRoute::MissingFooterGeometry
        );
        assert_eq!(
            classify_bottom_resize(
                gpui::MouseButton::Left,
                gpui::point(gpui::px(120.0), gpui::px(279.0)),
                size(),
                false,
                false,
                false,
                None,
            ),
            NotesBottomResizeRoute::EntryMotionActive
        );
    }
}

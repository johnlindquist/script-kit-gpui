use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum NotesBottomResizeRoute {
    IgnoredNonLeftButton,
    OutsideBottomEdge,
    CornerGuard,
    EntryMotionActive,
    ExitMotionActive,
    OverlayActive,
    MissingFooterGeometry,
    ProtectedFooterButton { group: &'static str, index: usize },
    ResizeStarted,
}

impl NotesBottomResizeRoute {
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            Self::IgnoredNonLeftButton => "ignoredNonLeftButton",
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

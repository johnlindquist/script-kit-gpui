#[inline]
fn main_list_footer_overlay_height() -> gpui::Pixels {
    // Floating footer chrome: the capsules live below the container in the
    // transparent window strip, so the list keeps only the reveal clearance
    // as its resting bottom inset instead of reserving footer height.
    if crate::footer_popup::glass_scroll_bands_active() {
        gpui::px(0.0)
    } else {
        gpui::px(crate::components::footer_chrome::current_main_menu_footer_height())
    }
}

#[inline]
fn main_list_footer_reveal_clearance_height() -> gpui::Pixels {
    gpui::px(crate::list_item::effective_footer_reveal_clearance_height())
}

pub(crate) fn main_list_footer_overlay_total_padding() -> gpui::Pixels {
    main_list_footer_overlay_height() + main_list_footer_reveal_clearance_height()
}

#[inline]
pub(crate) fn main_list_header_glass_strip_height() -> gpui::Pixels {
    // The header glass strip was removed (floating-chrome design 2026-07-21);
    // sticky group separators are the planned header scroll affordance.
    gpui::px(0.0)
}

#[inline]
pub(crate) fn main_list_header_overlay_height(
    def: crate::designs::MainMenuThemeDef,
) -> gpui::Pixels {
    gpui::px(
        crate::components::main_view_chrome::main_view_header_metrics(def, Some(def.search.height))
            .header_height,
    ) + main_list_header_glass_strip_height()
}

type ScriptListRowHeights = crate::scrolling::list_geometry::GroupedListRowHeights;

pub(crate) fn script_list_content_height(items: &[GroupedListItem]) -> f32 {
    script_list_content_height_with(
        items,
        ScriptListRowHeights::for_theme(crate::designs::current_main_menu_theme()),
    )
}

fn script_list_content_height_with(
    items: &[GroupedListItem],
    heights: ScriptListRowHeights,
) -> f32 {
    crate::scrolling::list_geometry::content_height(items.len(), |ix| {
        heights.row_height(&items[ix], ix)
    })
}

fn script_list_pixel_top_for_item(
    items: &[GroupedListItem],
    ix: usize,
    heights: ScriptListRowHeights,
) -> f32 {
    crate::scrolling::list_geometry::pixel_top_for_item(items.len(), ix, |item_ix| {
        heights.row_height(&items[item_ix], item_ix)
    })
}

fn script_list_pixel_top_for_offset(
    items: &[GroupedListItem],
    offset: gpui::ListOffset,
    heights: ScriptListRowHeights,
) -> f32 {
    crate::scrolling::list_geometry::pixel_top_for_offset(items.len(), offset, |ix| {
        heights.row_height(&items[ix], ix)
    })
}

fn script_list_offset_for_pixel_top(
    items: &[GroupedListItem],
    scroll_top_px: f32,
    heights: ScriptListRowHeights,
) -> gpui::ListOffset {
    crate::scrolling::list_geometry::offset_for_pixel_top(items.len(), scroll_top_px, |ix| {
        heights.row_height(&items[ix], ix)
    })
}

fn main_list_visible_range(
    items: &[GroupedListItem],
    scroll_top: f32,
    visible_height: f32,
    heights: ScriptListRowHeights,
) -> (usize, usize) {
    crate::scrolling::list_geometry::visible_range(
        items.len(),
        scroll_top,
        gpui::px(visible_height.max(0.0)),
        |ix| heights.row_height(&items[ix], ix),
    )
    .unwrap_or((items.len(), items.len()))
}

fn main_list_row_stable_key(
    grouped_item: &GroupedListItem,
    grouped_index: usize,
    flat_results: &[scripts::SearchResult],
) -> String {
    match grouped_item {
        GroupedListItem::Item(result_index) => flat_results
            .get(*result_index)
            .and_then(scripts::SearchResult::stable_selection_key)
            .unwrap_or_else(|| format!("generated/item/{grouped_index}/{result_index}")),
        GroupedListItem::SectionHeader(label, icon) => format!(
            "section/{grouped_index}/{label}/{}",
            icon.as_deref().unwrap_or("none")
        ),
        GroupedListItem::Status(status) => format!(
            "status/{}/{}/{grouped_index}/{}",
            status.source.receipt_label(),
            status.status_kind.as_str(),
            status.label
        ),
    }
}

fn main_list_row_semantic_id(stable_key: &str) -> String {
    format!("main-list-row:{stable_key}")
}

fn main_list_safe_scroll_offset_for_item(
    items: &[GroupedListItem],
    current_offset: gpui::ListOffset,
    viewport_height: gpui::Pixels,
    header_overlay_height: gpui::Pixels,
    footer_overlay_height: gpui::Pixels,
    target_ix: usize,
) -> Option<gpui::ListOffset> {
    if items.is_empty()
        || target_ix >= items.len()
        || viewport_height <= header_overlay_height + footer_overlay_height
    {
        return None;
    }

    let heights = ScriptListRowHeights::for_theme(crate::designs::current_main_menu_theme());
    let viewport_height = viewport_height.as_f32();
    let footer_overlay_height = footer_overlay_height.as_f32();
    let safe_viewport_height =
        viewport_height - header_overlay_height.as_f32() - footer_overlay_height;
    let max_scroll_top =
        (script_list_content_height_with(items, heights) - safe_viewport_height).max(0.0);
    let current_scroll_top = script_list_pixel_top_for_offset(items, current_offset, heights);
    let target_top = script_list_pixel_top_for_item(items, target_ix, heights);
    let target_bottom = target_top + heights.row_height(&items[target_ix], target_ix);
    let safe_bottom = current_scroll_top + safe_viewport_height;
    let safe_scroll_top = if target_top < current_scroll_top {
        target_top
    } else if target_bottom > safe_bottom {
        target_bottom - safe_viewport_height
    } else {
        return None;
    }
    .clamp(0.0, max_scroll_top);
    Some(script_list_offset_for_pixel_top(
        items,
        safe_scroll_top,
        heights,
    ))
}

fn leading_context_scroll_offset_for_selection(
    target_ix: usize,
    first_selectable_ix: Option<usize>,
) -> Option<gpui::ListOffset> {
    (first_selectable_ix == Some(target_ix)).then_some(gpui::ListOffset {
        item_ix: 0,
        offset_in_item: gpui::px(0.0),
    })
}

#[inline]
fn scrollbar_fade_duration() -> std::time::Duration {
    crate::transitions::DURATION_MEDIUM + std::time::Duration::from_millis(50)
}

#[inline]
fn scrollbar_fade_opacity(progress: f32) -> crate::transitions::Opacity {
    use crate::transitions::Lerp;
    let eased = crate::transitions::ease_in_quad(progress.clamp(0.0, 1.0));
    crate::transitions::Opacity::VISIBLE.lerp(&crate::transitions::Opacity::INVISIBLE, eased)
}

const MAIN_LIST_EDGE_EPSILON_PX: f32 = 0.5;

fn schedule_main_list_boundary_rebound_frame(
    app: gpui::WeakEntity<ScriptListApp>,
    generation: u64,
    started_at: std::time::Instant,
    tuning: crate::scrolling::boundary_affordance::BoundaryAffordanceTuning,
    window: &mut gpui::Window,
) {
    window.on_next_frame(move |window, cx| {
        let keep_running = app
            .update(cx, |app, cx| {
                if !app.main_list_boundary_affordance.apply_settle_sample(
                    generation,
                    started_at.elapsed(),
                    tuning,
                ) {
                    return false;
                }
                cx.notify();
                app.main_list_boundary_affordance.phase
                    == crate::scrolling::boundary_affordance::BoundaryPhase::Settling
            })
            .unwrap_or(false);
        if keep_running {
            schedule_main_list_boundary_rebound_frame(app, generation, started_at, tuning, window);
        }
    });
    window.request_animation_frame();
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MainListScrollGeometry {
    scroll_top: f32,
    content_height: f32,
    viewport_height: f32,
    footer_height: f32,
    header_height: f32,
    safe_viewport_top: f32,
    safe_viewport_bottom: f32,
    safe_viewport_height: f32,
    max_scroll_top: f32,
    at_top: bool,
    at_bottom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MainListTopFadeSnapshot {
    pub(crate) active: bool,
    pub(crate) progress: f32,
    pub(crate) alpha: u32,
}

fn main_list_scroll_geometry_values(
    content_height: f32,
    viewport_height: f32,
    header_height: f32,
    footer_height: f32,
    scroll_top: f32,
) -> MainListScrollGeometry {
    let viewport_height = viewport_height.max(0.0);
    let header_height = header_height.max(0.0).min(viewport_height);
    let footer_height = footer_height.max(0.0).min(viewport_height);
    let safe_viewport_top = header_height;
    let safe_viewport_bottom = (viewport_height - footer_height).max(safe_viewport_top);
    let safe_viewport_height = (safe_viewport_bottom - safe_viewport_top).max(0.0);
    let max_scroll_top = (content_height - safe_viewport_height).max(0.0);
    let measured = viewport_height > header_height + footer_height && viewport_height > 0.0;
    let scroll_top = scroll_top.clamp(0.0, max_scroll_top);

    MainListScrollGeometry {
        scroll_top,
        content_height,
        viewport_height,
        footer_height,
        header_height,
        safe_viewport_top,
        safe_viewport_bottom,
        safe_viewport_height,
        max_scroll_top,
        at_top: measured && scroll_top <= MAIN_LIST_EDGE_EPSILON_PX,
        at_bottom: measured && (max_scroll_top - scroll_top).abs() <= MAIN_LIST_EDGE_EPSILON_PX,
    }
}

#[inline]
fn main_list_top_fade_progress(scroll_top: f32, ramp: f32) -> f32 {
    if scroll_top <= MAIN_LIST_EDGE_EPSILON_PX {
        return 0.0;
    }
    let t = (scroll_top / ramp.max(1.0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn main_list_boundary_eligibility_values(
    geometry: MainListScrollGeometry,
) -> crate::scrolling::boundary_affordance::BoundaryEligibility {
    crate::scrolling::boundary_affordance::BoundaryEligibility {
        top: geometry.at_top,
        bottom: geometry.at_bottom,
    }
}

fn main_list_scroll_lifecycle_phase(
    phase: gpui::ScrollPhase,
) -> crate::scrolling::boundary_affordance::ScrollLifecyclePhase {
    use crate::scrolling::boundary_affordance::ScrollLifecyclePhase;
    match phase {
        gpui::ScrollPhase::None => ScrollLifecyclePhase::None,
        gpui::ScrollPhase::MayBegin => ScrollLifecyclePhase::MayBegin,
        gpui::ScrollPhase::Began => ScrollLifecyclePhase::Began,
        gpui::ScrollPhase::Changed => ScrollLifecyclePhase::Changed,
        gpui::ScrollPhase::Stationary => ScrollLifecyclePhase::Stationary,
        gpui::ScrollPhase::Ended => ScrollLifecyclePhase::Ended,
        gpui::ScrollPhase::Cancelled => ScrollLifecyclePhase::Cancelled,
    }
}

impl ScriptListApp {
    fn record_main_list_scroll_frame_trace(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let began_gesture = event.phase == gpui::ScrollPhase::Began
            || event.momentum_phase == gpui::ScrollPhase::Began
            || matches!(event.touch_phase, gpui::TouchPhase::Started);
        if !self
            .main_list_scroll_frame_trace
            .record_event(began_gesture, std::time::Instant::now())
        {
            return;
        }

        let app = cx.weak_entity();
        window.on_next_frame(move |_window, cx| {
            let _ = app.update(cx, |app, _cx| {
                app.main_list_scroll_frame_trace
                    .record_frame_callback(std::time::Instant::now());
            });
        });
        window.request_animation_frame();
    }

    fn main_list_boundary_affordance_tuning(
        &self,
    ) -> crate::scrolling::boundary_affordance::BoundaryAffordanceTuning {
        crate::scrolling::boundary_affordance::BoundaryAffordanceTuning::default()
    }

    fn main_list_scroll_geometry(&mut self) -> MainListScrollGeometry {
        let viewport_height = self.main_list_state.viewport_bounds().size.height.as_f32();
        let def = self.current_main_menu_theme.def();
        let header_height = main_list_header_overlay_height(def).as_f32();
        let footer_height = main_list_footer_overlay_total_padding().as_f32();
        let scroll_offset = self.main_list_state.logical_scroll_top();
        let heights = ScriptListRowHeights::for_theme(self.current_main_menu_theme);
        let (content_height, scroll_top) = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            (
                script_list_content_height_with(&grouped_items, heights),
                script_list_pixel_top_for_offset(&grouped_items, scroll_offset, heights),
            )
        };
        main_list_scroll_geometry_values(
            content_height,
            viewport_height,
            header_height,
            footer_height,
            scroll_top,
        )
    }

    pub(crate) fn main_list_top_fade_snapshot(&mut self) -> MainListTopFadeSnapshot {
        let geometry = self.main_list_scroll_geometry();
        let tokens = self.current_main_menu_theme.def().list;
        let progress = if crate::footer_popup::glass_scroll_bands_active() {
            0.0
        } else {
            main_list_top_fade_progress(geometry.scroll_top, tokens.top_occlusion_ramp)
        };
        MainListTopFadeSnapshot {
            active: progress > 0.0,
            progress,
            alpha: crate::components::list_scroll_affordance::top_occlusion_alpha(tokens, progress),
        }
    }

    fn main_list_boundary_eligibility(
        &mut self,
        geometry: MainListScrollGeometry,
    ) -> crate::scrolling::boundary_affordance::BoundaryEligibility {
        main_list_boundary_eligibility_values(geometry)
    }

    pub(crate) fn reset_main_list_boundary_affordance(
        &mut self,
        reason: crate::scrolling::boundary_affordance::SettleReason,
    ) -> bool {
        self.main_list_boundary_affordance.reset(reason)
    }

    fn schedule_main_list_boundary_settle(
        &mut self,
        tuning: crate::scrolling::boundary_affordance::BoundaryAffordanceTuning,
        window: Option<&mut gpui::Window>,
        cx: &mut Context<Self>,
    ) {
        use crate::scrolling::boundary_affordance::BoundaryPhase;

        let generation = self.main_list_boundary_affordance.generation;
        let started_at = std::time::Instant::now();

        if let Some(window) = window {
            schedule_main_list_boundary_rebound_frame(
                cx.weak_entity(),
                generation,
                started_at,
                tuning,
                window,
            );
            return;
        }

        // The idle watchdog has no Window borrow. It is only a recovery path
        // for platforms that omit terminal lifecycle events; native release
        // always uses display-frame callbacks above.
        const FRAME: std::time::Duration = std::time::Duration::from_millis(16);
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(FRAME).await;
            let elapsed = started_at.elapsed();
            let keep_running = cx
                .update(|cx| {
                    this.update(cx, |app, cx| {
                        if !app
                            .main_list_boundary_affordance
                            .apply_settle_sample(generation, elapsed, tuning)
                        {
                            return false;
                        }
                        cx.notify();
                        app.main_list_boundary_affordance.phase == BoundaryPhase::Settling
                    })
                })
                .unwrap_or(false);
            if !keep_running {
                break;
            }
        })
        .detach();
    }

    fn schedule_main_list_boundary_idle_watchdog(
        &mut self,
        tuning: crate::scrolling::boundary_affordance::BoundaryAffordanceTuning,
        cx: &mut Context<Self>,
    ) {
        use crate::scrolling::boundary_affordance::IdleWatchdogStatus;

        let generation = self.main_list_boundary_affordance.generation;
        cx.spawn(async move |this, cx| {
            let mut delay = tuning.idle_timeout;
            loop {
                cx.background_executor().timer(delay).await;
                let next_delay = cx
                    .update(|cx| {
                        this.update(cx, |app, cx| {
                            match app.main_list_boundary_affordance.idle_watchdog_status(
                                generation,
                                std::time::Instant::now(),
                                tuning,
                            ) {
                                IdleWatchdogStatus::Cancelled => None,
                                IdleWatchdogStatus::Sleep(remaining) => Some(remaining),
                                IdleWatchdogStatus::TimedOut => {
                                    let decision = app
                                        .main_list_boundary_affordance
                                        .begin_idle_timeout_settle(
                                            generation,
                                            std::time::Instant::now(),
                                            tuning,
                                        );
                                    if decision.start_settle.is_some() {
                                        app.schedule_main_list_boundary_settle(tuning, None, cx);
                                    }
                                    if decision.visual_changed {
                                        cx.notify();
                                    }
                                    None
                                }
                            }
                        })
                    })
                    .ok()
                    .flatten();
                let Some(remaining) = next_delay else {
                    break;
                };
                delay = remaining;
            }
        })
        .detach();
    }

    pub(crate) fn observe_main_list_scroll_wheel(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        use crate::scrolling::boundary_affordance::SettleReason;

        self.record_main_list_scroll_frame_trace(event, window, cx);
        self.begin_list_viewport_scroll(
            crate::scrolling::list_interaction::ListViewportInputSource::from_event(event),
            cx,
        );
        self.trigger_scroll_activity(cx);

        match event.delta {
            gpui::ScrollDelta::Lines(_) => {
                if self.main_list_boundary_affordance.phase
                    != crate::scrolling::boundary_affordance::BoundaryPhase::Idle
                    && self.reset_main_list_boundary_affordance(SettleReason::Reset)
                {
                    cx.notify();
                }
            }
            gpui::ScrollDelta::Pixels(point) => {
                let delta_y_px = point.y.as_f32();
                let geometry = self.main_list_scroll_geometry();
                let eligibility = self.main_list_boundary_eligibility(geometry);
                let tuning = self.main_list_boundary_affordance_tuning();
                let legacy_phase = match event.touch_phase {
                    gpui::TouchPhase::Started => {
                        crate::scrolling::boundary_affordance::PreciseTouchPhase::Started
                    }
                    gpui::TouchPhase::Moved => {
                        crate::scrolling::boundary_affordance::PreciseTouchPhase::Moved
                    }
                    gpui::TouchPhase::Ended => {
                        crate::scrolling::boundary_affordance::PreciseTouchPhase::Ended
                    }
                };
                let decision = self.main_list_boundary_affordance.observe_scroll_lifecycle(
                    delta_y_px,
                    main_list_scroll_lifecycle_phase(event.phase),
                    main_list_scroll_lifecycle_phase(event.momentum_phase),
                    legacy_phase,
                    eligibility,
                    tuning,
                    crate::platform::prefers_reduced_motion(),
                    std::time::Instant::now(),
                    event.timestamp_seconds,
                );

                if decision.arm_idle_watchdog {
                    self.schedule_main_list_boundary_idle_watchdog(tuning, cx);
                }
                if decision.start_settle.is_some() {
                    self.schedule_main_list_boundary_settle(tuning, Some(window), cx);
                }
                if decision.visual_changed {
                    cx.notify();
                }
            }
        }

        // The native List listener has already updated ListState before this
        // ancestor observer runs. Repaint viewport-derived fade/receipt state;
        // selection and focus deliberately remain untouched.
        cx.notify();
    }

    pub(crate) fn observe_main_list_scrollbar_pointer_down(&mut self, cx: &mut Context<Self>) {
        self.begin_list_viewport_scroll(
            crate::scrolling::list_interaction::ListViewportInputSource::Scrollbar,
            cx,
        );
        if self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        ) {
            cx.notify();
        }
        self.trigger_scroll_activity(cx);
    }

    pub(crate) fn main_list_scroll_receipt(&mut self) -> serde_json::Value {
        let viewport_height = self.main_list_state.viewport_bounds().size.height;
        let footer_height = main_list_footer_overlay_total_padding();
        let scroll_offset = self.main_list_state.logical_scroll_top();
        let heights = ScriptListRowHeights::for_theme(crate::designs::current_main_menu_theme());
        let (
            content_height,
            selected_row_top,
            selected_row_bottom,
            item_count,
            first_visible_index,
            last_visible_index_exclusive,
            first_visible_semantic_id,
            last_visible_semantic_id,
            selected_stable_key,
            selected_semantic_id,
            hovered_semantic_id,
        ) = {
            let (grouped_items, flat_results) = self.get_grouped_results_cached();
            let content_height = script_list_content_height_with(&grouped_items, heights);
            let scroll_top =
                script_list_pixel_top_for_offset(&grouped_items, scroll_offset, heights);
            let visible_height = (viewport_height.as_f32()
                - main_list_header_overlay_height(self.current_main_menu_theme.def()).as_f32())
            .max(0.0);
            let (first_visible_index, last_visible_index_exclusive) =
                main_list_visible_range(&grouped_items, scroll_top, visible_height, heights);
            let stable_key_at = |index: usize| {
                grouped_items
                    .get(index)
                    .map(|item| main_list_row_stable_key(item, index, &flat_results))
            };
            let semantic_id_at =
                |index: usize| stable_key_at(index).map(|key| main_list_row_semantic_id(&key));
            let selected_row_top = grouped_items.get(self.selected_index).map(|_| {
                script_list_pixel_top_for_item(&grouped_items, self.selected_index, heights)
            });
            let selected_row_bottom = grouped_items.get(self.selected_index).map(|item| {
                selected_row_top.unwrap_or(0.0) + heights.row_height(item, self.selected_index)
            });
            (
                content_height,
                selected_row_top,
                selected_row_bottom,
                grouped_items.len(),
                first_visible_index,
                last_visible_index_exclusive,
                semantic_id_at(first_visible_index),
                last_visible_index_exclusive
                    .checked_sub(1)
                    .and_then(semantic_id_at),
                stable_key_at(self.selected_index),
                semantic_id_at(self.selected_index),
                self.hovered_index.and_then(semantic_id_at),
            )
        };
        let scroll_top = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            script_list_pixel_top_for_offset(&grouped_items, scroll_offset, heights)
        };
        let geometry = main_list_scroll_geometry_values(
            content_height,
            viewport_height.as_f32(),
            main_list_header_overlay_height(self.current_main_menu_theme.def()).as_f32(),
            footer_height.as_f32(),
            scroll_top,
        );
        let selected_row_top_in_view =
            selected_row_top.map(|top| geometry.header_height + top - geometry.scroll_top);
        let selected_row_bottom_in_view =
            selected_row_bottom.map(|bottom| geometry.header_height + bottom - geometry.scroll_top);
        let selected_row_visible = selected_row_top_in_view
            .zip(selected_row_bottom_in_view)
            .map(|(top, bottom)| top >= 0.0 && bottom <= geometry.viewport_height)
            .unwrap_or(false);
        let selected_row_above_footer = selected_row_bottom_in_view
            .map(|bottom| bottom <= geometry.safe_viewport_bottom)
            .unwrap_or(false);
        let selected_row_below_header = selected_row_top_in_view
            .map(|top| top >= geometry.safe_viewport_top)
            .unwrap_or(false);
        let selected_row_within_safe_viewport =
            selected_row_below_header && selected_row_above_footer;
        let tokens = self.current_main_menu_theme.def().list;
        let top_fade_progress = if crate::footer_popup::glass_scroll_bands_active() {
            0.0
        } else {
            main_list_top_fade_progress(geometry.scroll_top, tokens.top_occlusion_ramp)
        };
        let top_fade_alpha = crate::components::list_scroll_affordance::top_occlusion_alpha(
            tokens,
            top_fade_progress,
        );
        let tuning = self.main_list_boundary_affordance_tuning();
        let affordance = &self.main_list_boundary_affordance;
        let trace_enabled = std::env::var_os("SCRIPT_KIT_MAIN_LIST_ELASTIC_TRACE").is_some();
        let trace_samples: Vec<_> = if trace_enabled {
            affordance
                .trace_samples()
                .map(|sample| {
                    serde_json::json!({
                        "kind": sample.kind.as_str(),
                        "arrivalElapsedMs": sample.arrival_elapsed_ms,
                        "nativeTimestampSeconds": sample.native_timestamp_seconds,
                        "directPhase": sample.direct_phase.as_str(),
                        "momentumPhase": sample.momentum_phase.as_str(),
                        "deltaY": sample.delta_y,
                        "rawPullPx": sample.raw_pull_px,
                        "offsetPx": sample.offset_px,
                        "velocityPxPerSecond": sample.velocity_px_per_second,
                        "boundaryPhase": sample.boundary_phase.as_str(),
                        "generation": sample.generation,
                        "renderedFrameGeneration": sample.rendered_frame_generation,
                        "reboundElapsedMs": sample.last_rebound_elapsed_ms,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut receipt = serde_json::json!({
            "scrollTop": geometry.scroll_top,
            "scrollTopItem": scroll_offset.item_ix,
            "scrollTopOffset": scroll_offset.offset_in_item.as_f32(),
            "firstVisibleIndex": first_visible_index,
            "lastVisibleIndexExclusive": last_visible_index_exclusive,
            "firstVisibleSemanticId": first_visible_semantic_id,
            "lastVisibleSemanticId": last_visible_semantic_id,
            "contentHeight": geometry.content_height,
            "viewportHeight": geometry.viewport_height,
            "headerOverlayHeight": geometry.header_height,
            "headerGlassStripHeight": main_list_header_glass_strip_height().as_f32(),
            "footerHeight": geometry.footer_height,
            "footerOverlayHeight": main_list_footer_overlay_height().as_f32().max(0.0),
            "footerRevealClearanceHeight": main_list_footer_reveal_clearance_height().as_f32().max(0.0),
            "footerOverlayTotalPadding": geometry.footer_height,
            "safeViewportHeight": geometry.safe_viewport_height,
            "maxScrollTop": geometry.max_scroll_top,
            "selectedIndex": self.selected_index,
            "selectedSemanticId": selected_semantic_id,
            "selectedStableKey": selected_stable_key,
            "selectedRowTop": selected_row_top_in_view,
            "selectedRowBottom": selected_row_bottom_in_view,
            "selectedRowVisible": selected_row_visible,
            "selectedRowAboveFooter": selected_row_above_footer,
            "itemCount": item_count,
            "hoveredIndex": self.hovered_index,
            "hoveredSemanticId": hovered_semantic_id,
            "hoverSuppressedUntilPointerMove": self.list_suppress_hover_until_pointer_move,
            "inputMode": match self.input_mode {
                InputMode::Keyboard => "keyboard",
                InputMode::Mouse => "mouse",
            },
            "focusedSemanticId": "input:filter",
            "lastInteractionSource": self.last_list_interaction_source.as_str(),
            "performance": self.main_list_scroll_frame_trace.receipt(),
            "affordance": {
                "atTop": geometry.at_top,
                "atBottom": geometry.at_bottom,
                "topFadeActive": top_fade_progress > 0.0,
                "topFadeProgress": top_fade_progress,
                "topFadeAlpha": top_fade_alpha,
                "overscrollOffsetPx": affordance.offset_px,
                "overscrollMaxOffsetPx": tuning.active_max_distance_px(affordance.reduced_motion),
                "overscrollEdge": affordance.edge.map(|edge| edge.as_str()),
                "overscrollPhase": affordance.phase.as_str(),
                "generation": affordance.generation,
                "lastTouchPhase": affordance.last_touch_phase.map(|phase| phase.as_str()),
                "lastSettleReason": affordance.last_settle_reason.map(|reason| reason.as_str()),
                "directPhase": affordance.last_direct_phase.as_str(),
                "momentumPhase": affordance.last_momentum_phase.as_str(),
                "nativeTimestampSeconds": affordance.last_native_timestamp_seconds,
                "momentumSuppressed": affordance.suppress_momentum_until_terminal,
                "rawPullPx": affordance.raw_pull_px(),
                "visualVelocityPxPerSecond": affordance.visual_velocity_px_per_second,
                "reboundInitialOffsetPx": affordance.rebound_initial_offset_px(),
                "reboundInitialVelocityPxPerSecond": affordance.rebound_initial_velocity_px_per_second(),
                "reboundElapsedMs": affordance.last_rebound_elapsed_ms,
                "reboundOmegaPerSecond": tuning.spring_omega_per_second(),
                "frameGeneration": affordance.rendered_frame_generation,
                "traceSamples": trace_samples,
                "reducedMotion": affordance.reduced_motion,
            },
        });
        if let Some(object) = receipt.as_object_mut() {
            object.insert("headerOverlayHeight".into(), geometry.header_height.into());
            object.insert("listTopInset".into(), geometry.header_height.into());
            object.insert("safeViewportTop".into(), geometry.safe_viewport_top.into());
            object.insert(
                "safeViewportBottom".into(),
                geometry.safe_viewport_bottom.into(),
            );
            object.insert(
                "selectedRowBelowHeader".into(),
                selected_row_below_header.into(),
            );
            object.insert(
                "selectedRowWithinSafeViewport".into(),
                selected_row_within_safe_viewport.into(),
            );
        }
        receipt
    }

    pub(crate) fn reveal_main_list_selection_above_footer(&mut self, reason: &str) {
        self.scroll_to_selected_if_needed(reason);
    }

    pub(crate) fn schedule_main_list_selection_reveal_above_footer(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        const ATTEMPTS: usize = 5;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(16);

        self.last_scrolled_index = None;
        cx.spawn(async move |this, cx| {
            for _ in 0..ATTEMPTS {
                cx.background_executor().timer(RETRY_DELAY).await;
                let revealed = cx
                    .update(|cx| {
                        this.update(cx, |app, cx| {
                            let viewport_height = app.main_list_state.viewport_bounds().size.height;
                            let def = app.current_main_menu_theme.def();
                            let header_height = main_list_header_overlay_height(def);
                            if viewport_height
                                <= header_height + main_list_footer_overlay_total_padding()
                            {
                                app.last_scrolled_index = None;
                                cx.notify();
                                return false;
                            }

                            app.last_scrolled_index = None;
                            app.reveal_main_list_selection_above_footer(reason);
                            cx.notify();
                            true
                        })
                    })
                    .unwrap_or(false);
                if revealed {
                    break;
                }
            }
        })
        .detach();
    }

    fn adjust_selected_item_above_footer_overlay(&mut self, target: usize) {
        let viewport_height = self.main_list_state.viewport_bounds().size.height;
        if viewport_height <= gpui::px(0.0) {
            return;
        }

        let adjusted_scroll_offset = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            main_list_safe_scroll_offset_for_item(
                &grouped_items,
                self.main_list_state.logical_scroll_top(),
                viewport_height,
                main_list_header_overlay_height(self.current_main_menu_theme.def()),
                main_list_footer_overlay_total_padding(),
                target,
            )
        };

        if let Some(scroll_offset) = adjusted_scroll_offset {
            self.main_list_state.scroll_to(scroll_offset);
        }
    }

    fn scroll_to_selected_if_needed(&mut self, reason: &str) {
        let target = self.selected_index;

        // Check if we've already scrolled to this index
        if self.last_scrolled_index == Some(target) {
            tracing::trace!(
                target: "SCROLL_STATE",
                reason,
                target,
                "skip scroll reveal; target already revealed"
            );
            return;
        }

        let before_top = self.main_list_state.logical_scroll_top().item_ix;

        // Use perf guard for scroll timing
        let _scroll_perf = crate::perf::ScrollPerfGuard::new();

        // Revealing the first selectable row alone can leave a leading section
        // header clipped above the viewport. Restore the true logical top so
        // returning to the first row matches the initial launcher layout.
        if let Some(offset) = leading_context_scroll_offset_for_selection(
            target,
            self.main_menu_result_caches.first_selectable_index(),
        ) {
            self.main_list_state.scroll_to(offset);
        } else {
            // Perform the scroll using ListState for variable-height list.
            self.main_list_state.scroll_to_reveal_item(target);
            self.adjust_selected_item_above_footer_overlay(target);
        }
        if self.main_list_state.viewport_bounds().size.height
            > main_list_footer_overlay_total_padding()
        {
            self.last_scrolled_index = Some(target);
        } else {
            self.last_scrolled_index = None;
        }

        let after_top = self.main_list_state.logical_scroll_top().item_ix;

        tracing::debug!(
            target: "SCROLL_STATE",
            reason,
            target,
            before_top,
            after_top,
            "revealed selected item"
        );
    }

    /// Trigger scroll activity - shows the scrollbar and schedules fade-out
    ///
    /// This should be called whenever scroll-related activity occurs:
    /// - Keyboard up/down navigation
    /// - scroll_to_item calls
    /// - Mouse wheel scrolling (if tracked)
    fn trigger_scroll_activity(&mut self, cx: &mut Context<Self>) {
        const SCROLLBAR_IDLE_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);
        const SCROLLBAR_FADE_TICK: std::time::Duration = std::time::Duration::from_millis(16);

        let now = std::time::Instant::now();
        self.last_scroll_time = Some(now);
        self.scrollbar_visibility = crate::transitions::Opacity::VISIBLE;
        self.scrollbar_fade_gen = self.scrollbar_fade_gen.wrapping_add(1);
        let fade_gen = self.scrollbar_fade_gen;

        tracing::debug!(
            target: "SCROLL_STATE",
            fade_gen,
            "Scrollbar activity detected; scheduling fade-out"
        );

        // Schedule fade-out after 1000ms of inactivity
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SCROLLBAR_IDLE_DELAY).await;

            let should_start_fade = cx
                .update(|cx| {
                    this.update(cx, |app, _cx| {
                        if app.scrollbar_fade_gen != fade_gen {
                            return false;
                        }

                        app.last_scroll_time
                            .map(|last_time| last_time.elapsed() >= SCROLLBAR_IDLE_DELAY)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            if !should_start_fade {
                tracing::trace!(
                    target: "SCROLL_STATE",
                    fade_gen,
                    "Skipping scrollbar fade due to newer activity"
                );
                return;
            }

            let fade_duration = scrollbar_fade_duration();
            let fade_start = std::time::Instant::now();

            loop {
                let elapsed = fade_start.elapsed();
                let t = (elapsed.as_secs_f32() / fade_duration.as_secs_f32()).clamp(0.0, 1.0);
                let opacity = scrollbar_fade_opacity(t);

                let continue_fade = cx
                    .update(|cx| {
                        this.update(cx, |app, cx| {
                            if app.scrollbar_fade_gen != fade_gen {
                                return false;
                            }

                            app.scrollbar_visibility = opacity;
                            cx.notify();
                            t < 1.0
                        })
                    })
                    .unwrap_or(false);

                if !continue_fade {
                    break;
                }

                cx.background_executor().timer(SCROLLBAR_FADE_TICK).await;
            }
        })
        .detach();

        cx.notify();
    }

    /// Apply a coalesced navigation delta in the given direction
    #[allow(dead_code)]
    fn apply_nav_delta(&mut self, dir: NavDirection, delta: i32, cx: &mut Context<Self>) {
        let signed = match dir {
            NavDirection::Up => -delta,
            NavDirection::Down => delta,
        };
        self.move_selection_by(signed, cx);
    }

    /// Move selection by a signed delta (positive = down, negative = up)
    /// Used by NavCoalescer for batched movements
    ///
    /// IMPORTANT: This must use grouped results and skip section headers,
    /// just like move_selection_up/down. Otherwise, holding arrow keys
    /// can land on headers causing navigation to feel "stuck".
    fn move_selection_by(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.enter_keyboard_mode(cx);
        if delta != 0 {
            self.mark_main_menu_selection_user_moved();
        }

        let selection_update = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let len = grouped_items.len();

            if len == 0 {
                None
            } else {
                let clamped_index = self.selected_index.min(len.saturating_sub(1));
                let first_selectable = self.main_menu_result_caches.first_selectable_index();
                let last_selectable = self.main_menu_result_caches.last_selectable_index();

                if let (Some(first), Some(last)) = (first_selectable, last_selectable) {
                    let target =
                        (clamped_index as i32 + delta).clamp(first as i32, last as i32) as usize;

                    let new_index = if delta > 0 {
                        let mut idx = target;
                        while idx < last
                            && matches!(
                                grouped_items.get(idx),
                                Some(
                                    GroupedListItem::SectionHeader(..)
                                        | GroupedListItem::Status(..)
                                )
                            )
                        {
                            idx += 1;
                        }
                        idx
                    } else if delta < 0 {
                        let mut idx = target;
                        while idx > first
                            && matches!(
                                grouped_items.get(idx),
                                Some(
                                    GroupedListItem::SectionHeader(..)
                                        | GroupedListItem::Status(..)
                                )
                            )
                        {
                            idx -= 1;
                        }
                        idx
                    } else {
                        clamped_index
                    };

                    let resolved_index = if matches!(
                        grouped_items.get(new_index),
                        Some(GroupedListItem::SectionHeader(..) | GroupedListItem::Status(..))
                    ) {
                        clamped_index
                    } else {
                        new_index
                    };

                    if resolved_index != clamped_index {
                        Some((resolved_index, "coalesced_nav"))
                    } else {
                        Some((clamped_index, "coalesced_nav_clamp"))
                    }
                } else {
                    Some((clamped_index, "coalesced_nav_clamp"))
                }
            }
        };

        if let Some((new_index, reason)) = selection_update {
            self.set_selected_index(new_index, reason, cx);
        } else {
            self.selected_index = 0;
        }
    }

    /// Handle mouse wheel scroll events by converting to item-based scrolling.
    ///
    /// This bypasses GPUI's pixel-based scroll which has height calculation issues
    /// with variable-height items. Instead, we convert the scroll delta to item
    /// indices and use scroll_to_reveal_item() like keyboard navigation does.
    ///
    /// # Arguments
    /// * `delta_lines` - Scroll delta in "lines" (positive = scroll content up/view down)
    #[allow(dead_code)]
    pub fn handle_scroll_wheel(&mut self, delta_lines: f32, cx: &mut Context<Self>) {
        // Compute wheel movement targets while grouped results are borrowed.
        let (current_item, new_item, items_to_scroll) = {
            let current_item = self.main_list_state.logical_scroll_top().item_ix;
            let (grouped_items, _) = self.get_grouped_results_cached();
            let item_count = grouped_items.len();
            let new_item = wheel_scroll_target_index(current_item, item_count, delta_lines);
            let items_to_scroll = (-delta_lines).round() as i32;
            (current_item, new_item, items_to_scroll)
        };

        tracing::debug!(
            target: "SCROLL_STATE",
            delta_lines,
            current_item,
            new_item,
            items_to_scroll,
            "Mouse wheel scroll"
        );

        // Only scroll if we're moving to a different item
        if new_item != current_item {
            self.main_list_state.scroll_to_reveal_item(new_item);
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    /// Synchronize the GPUI list component state with the current grouped results.
    ///
    /// Call this method after any operation that may change the number of items
    /// in the list (filter changes, data refresh, view transitions).
    ///
    /// This method handles:
    /// - Updating the list component's item count via splice()
    /// - Invalidating scroll tracking when structure changes
    ///
    /// Note: This is separate from validate_selection_bounds() which handles
    /// ensuring the selected index is valid.
    pub fn sync_list_state(&mut self) {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        let (grouped_items, _) = self.get_grouped_results_cached();
        let item_count = grouped_items.len();

        let old_list_count = self.main_list_state.item_count();
        if old_list_count != item_count {
            self.main_list_state.splice(0..old_list_count, item_count);
        }

        // Always invalidate reveal cache: filtering can replace every visible
        // row while preserving the same count, so the selected item can end up
        // offscreen even when item_count is unchanged.
        self.last_scrolled_index = None;

        tracing::debug!(
            target: "SCROLL_STATE",
            old_list_count,
            item_count,
            selected_index = self.selected_index,
            "synced list state"
        );

        if self.selected_index < item_count {
            self.main_list_state
                .scroll_to_reveal_item(self.selected_index);
            self.adjust_selected_item_above_footer_overlay(self.selected_index);
        }
    }

    /// Force GPUI's measured list items to be rebuilt for same-count row replacements.
    ///
    /// Filter-history recalls can replace every row while preserving the same
    /// item count. `sync_list_state` keeps that path cheap for ordinary syncs,
    /// so filter changes replace the list state identity to avoid stale
    /// measured row elements being painted under fresh footer/preflight state.
    pub fn sync_list_state_for_filter_replacement(&mut self) {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        let (grouped_items, _) = self.get_grouped_results_cached();
        let item_count = grouped_items.len();
        let old_list_count = self.main_list_state.item_count();

        self.last_scrolled_index = None;

        if old_list_count != item_count {
            self.main_list_state.splice(0..old_list_count, item_count);

            if crate::logging::filter_perf_trace_enabled() {
                tracing::info!(
                    target: "SCROLL_STATE",
                    old_list_count,
                    item_count,
                    selected_index = self.selected_index,
                    "spliced list state for filter replacement"
                );
            }

            return;
        }

        if item_count == 0 {
            if crate::logging::filter_perf_trace_enabled() {
                tracing::info!(
                    target: "SCROLL_STATE",
                    old_list_count,
                    item_count,
                    selected_index = self.selected_index,
                    "skipped empty list state replacement for filter replacement"
                );
            }

            return;
        }

        self.main_list_row_generation = self.main_list_row_generation.wrapping_add(1);
        self.main_list_state = ListState::new(
            item_count,
            ListAlignment::Top,
            px(
                crate::list_item::effective_average_item_height_for_scroll_for_theme(
                    crate::designs::current_main_menu_theme(),
                ),
            ),
        );

        if crate::logging::filter_perf_trace_enabled() {
            tracing::info!(
                target: "SCROLL_STATE",
                old_list_count,
                item_count,
                selected_index = self.selected_index,
                row_generation = self.main_list_row_generation,
                "replaced list state for filter replacement"
            );
        }
    }

    /// Validate and correct selection bounds after list structure changes.
    ///
    /// Call this method from event handlers after any operation that may change
    /// the number of items in the list (filter changes, data refresh, view transitions).
    ///
    /// This replaces the anti-pattern of mutating selection during render.
    /// By validating in event handlers, render remains a pure function of state.
    ///
    /// # Returns
    /// `true` if selection was changed, `false` if it was already valid.
    pub fn validate_selection_bounds(&mut self, cx: &mut Context<Self>) -> bool {
        enum ValidationState {
            Empty,
            NonEmpty {
                valid_idx: usize,
                has_selectable: bool,
            },
        }

        let validation_state = {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let item_count = grouped_items.len();

            if item_count == 0 {
                ValidationState::Empty
            } else {
                let clamped_index = self.selected_index.min(item_count.saturating_sub(1));
                let has_selectable = self.main_menu_result_caches.has_selectable_grouped_item();
                ValidationState::NonEmpty {
                    valid_idx: validated_selection_index(&grouped_items, clamped_index),
                    has_selectable,
                }
            }
        };

        match validation_state {
            ValidationState::Empty => {
                // Empty list - reset all selection state
                let changed = self.selected_index != 0
                    || self.hovered_index.is_some()
                    || self.last_scrolled_index.is_some();

                self.selected_index = 0;
                self.clear_menu_syntax_filter_accept_hint();
                self.hovered_index = None;
                self.last_scrolled_index = None;

                self.main_menu_fallback_state.clear();

                if changed {
                    cx.notify();
                }
                changed
            }
            ValidationState::NonEmpty {
                valid_idx,
                has_selectable,
            } => {
                // List has items - coerce selection to a valid selectable item
                self.main_menu_fallback_state.clear();

                if valid_idx == 0 && !has_selectable {
                    // No selectable items (list is all headers) - reset to 0
                    if self.selected_index != 0 {
                        self.clear_menu_syntax_filter_accept_hint();
                        self.selected_index = 0;
                        cx.notify();
                        return true;
                    }
                } else if self.selected_index != valid_idx {
                    self.clear_menu_syntax_filter_accept_hint();
                    self.selected_index = valid_idx;
                    cx.notify();
                    return true;
                }

                false
            }
        }
    }

    /// Ensure the navigation flush task is running. Spawns a background task
    /// that periodically flushes pending navigation deltas.
    #[allow(dead_code)]
    fn ensure_nav_flush_task(&mut self, cx: &mut Context<Self>) {
        if self.nav_coalescer.flush_task_running {
            return;
        }
        self.nav_coalescer.flush_task_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(NavCoalescer::WINDOW).await;
                let keep_running = cx
                    .update(|cx| {
                        this.update(cx, |this, cx| {
                            // Flush any pending navigation delta
                            if let Some((dir, delta)) = this.nav_coalescer.flush_pending() {
                                this.apply_nav_delta(dir, delta, cx);
                            }
                            // Check if we should keep running
                            let now = std::time::Instant::now();
                            let recently_active = now.duration_since(this.nav_coalescer.last_event)
                                < NavCoalescer::WINDOW;
                            if !recently_active && this.nav_coalescer.pending_delta == 0 {
                                // No recent activity and no pending delta - stop the task
                                this.nav_coalescer.flush_task_running = false;
                                this.nav_coalescer.reset();
                                false
                            } else {
                                true
                            }
                        })
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod scroll_fade_tests {
    use super::{
        leading_context_scroll_offset_for_selection, main_list_boundary_eligibility_values,
        main_list_row_stable_key, main_list_safe_scroll_offset_for_item,
        main_list_scroll_geometry_values, main_list_scroll_lifecycle_phase,
        main_list_top_fade_progress, main_list_visible_range, script_list_pixel_top_for_offset,
        scrollbar_fade_duration, scrollbar_fade_opacity, ScriptListRowHeights,
    };
    use crate::list_item::GroupedListItem;

    #[test]
    fn test_scrollbar_fade_duration_does_match_medium_plus_50ms_when_computed() {
        assert_eq!(
            scrollbar_fade_duration(),
            crate::transitions::DURATION_MEDIUM + std::time::Duration::from_millis(50)
        );
    }

    #[test]
    fn test_scrollbar_fade_opacity_does_stay_visible_when_progress_is_zero() {
        assert_eq!(
            scrollbar_fade_opacity(0.0),
            crate::transitions::Opacity::VISIBLE
        );
    }

    #[test]
    fn test_scrollbar_fade_opacity_does_turn_invisible_when_progress_is_one() {
        assert_eq!(
            scrollbar_fade_opacity(1.0),
            crate::transitions::Opacity::INVISIBLE
        );
    }

    #[test]
    fn test_scrollbar_fade_opacity_does_use_ease_in_curve_when_progress_is_midpoint() {
        let midpoint = scrollbar_fade_opacity(0.5).value();
        assert!((midpoint - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn top_occlusion_is_exactly_absent_at_logical_top_and_smoothly_ramps() {
        assert_eq!(main_list_top_fade_progress(0.0, 24.0), 0.0);
        assert_eq!(main_list_top_fade_progress(0.5, 24.0), 0.0);
        let midpoint = main_list_top_fade_progress(12.0, 24.0);
        assert!((midpoint - 0.5).abs() < f32::EPSILON);
        assert_eq!(main_list_top_fade_progress(24.0, 24.0), 1.0);
        assert_eq!(main_list_top_fade_progress(200.0, 24.0), 1.0);
    }

    #[test]
    fn top_occlusion_depends_only_on_viewport_offset() {
        let progress = main_list_top_fade_progress(44.0, 96.0);
        assert!(progress > 0.0 && progress < 0.5);
    }

    #[test]
    fn returning_to_first_selectable_restores_leading_section_header() {
        let offset = leading_context_scroll_offset_for_selection(1, Some(1))
            .expect("first selectable row should restore the real list top");
        assert_eq!(offset.item_ix, 0);
        assert_eq!(offset.offset_in_item, gpui::px(0.0));
        assert!(leading_context_scroll_offset_for_selection(2, Some(1)).is_none());
    }

    #[test]
    fn native_terminal_direct_phase_releases_before_momentum() {
        assert_eq!(
            main_list_scroll_lifecycle_phase(gpui::ScrollPhase::Ended),
            crate::scrolling::boundary_affordance::ScrollLifecyclePhase::Ended
        );
        assert_eq!(
            main_list_scroll_lifecycle_phase(gpui::ScrollPhase::Cancelled),
            crate::scrolling::boundary_affordance::ScrollLifecyclePhase::Cancelled
        );
        assert_eq!(
            main_list_scroll_lifecycle_phase(gpui::ScrollPhase::Began),
            crate::scrolling::boundary_affordance::ScrollLifecyclePhase::Began
        );
    }

    #[test]
    fn boundary_geometry_uses_footer_safe_viewport_and_fails_closed_unmeasured() {
        let top = main_list_scroll_geometry_values(1000.0, 420.0, 58.0, 32.0, 0.0);
        assert!(top.at_top);
        assert!(!top.at_bottom);
        assert_eq!(top.safe_viewport_top, 58.0);
        assert_eq!(top.safe_viewport_bottom, 388.0);
        assert_eq!(top.safe_viewport_height, 330.0);
        assert_eq!(top.max_scroll_top, 670.0);

        let bottom = main_list_scroll_geometry_values(1000.0, 420.0, 58.0, 32.0, 670.0);
        assert!(!bottom.at_top);
        assert!(bottom.at_bottom);

        let unmeasured = main_list_scroll_geometry_values(1000.0, 0.0, 58.0, 32.0, 0.0);
        assert!(!unmeasured.at_top);
        assert!(!unmeasured.at_bottom);
    }

    #[test]
    fn boundary_capture_depends_only_on_viewport_endpoints() {
        let top = main_list_scroll_geometry_values(1000.0, 420.0, 58.0, 32.0, 0.0);
        let top_eligibility = main_list_boundary_eligibility_values(top);
        assert!(top_eligibility.top);
        assert!(!top_eligibility.bottom);

        let bottom = main_list_scroll_geometry_values(1000.0, 420.0, 58.0, 32.0, 670.0);
        let bottom_eligibility = main_list_boundary_eligibility_values(bottom);
        assert!(!bottom_eligibility.top);
        assert!(bottom_eligibility.bottom);
    }

    #[test]
    fn test_footer_safe_scroll_offset_moves_selected_row_above_overlay() {
        let rows = vec![
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
            GroupedListItem::Item(2),
            GroupedListItem::Item(3),
            GroupedListItem::Item(4),
            GroupedListItem::Item(5),
            GroupedListItem::Item(6),
            GroupedListItem::Item(7),
            GroupedListItem::Item(8),
        ];

        let adjusted = main_list_safe_scroll_offset_for_item(
            &rows,
            gpui::ListOffset {
                item_ix: 0,
                offset_in_item: gpui::px(0.0),
            },
            gpui::px(360.0),
            gpui::px(58.0),
            gpui::px(30.0),
            8,
        )
        .expect("target should be pushed above the footer overlay");

        assert_eq!(
            script_list_pixel_top_for_offset(
                &rows,
                adjusted,
                ScriptListRowHeights::for_theme(crate::designs::current_main_menu_theme()),
            ),
            124.0
        );
    }

    #[test]
    fn test_footer_safe_scroll_offset_allows_trailing_scroll_budget_for_last_row() {
        let rows = vec![
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
            GroupedListItem::Item(2),
            GroupedListItem::Item(3),
            GroupedListItem::Item(4),
            GroupedListItem::Item(5),
            GroupedListItem::Item(6),
            GroupedListItem::Item(7),
            GroupedListItem::Item(8),
        ];

        let adjusted = main_list_safe_scroll_offset_for_item(
            &rows,
            gpui::ListOffset {
                item_ix: 0,
                offset_in_item: gpui::px(0.0),
            },
            gpui::px(360.0),
            gpui::px(58.0),
            gpui::px(30.0),
            8,
        )
        .expect("last row should get the extra footer-height trailing scroll budget");

        assert_eq!(
            script_list_pixel_top_for_offset(
                &rows,
                adjusted,
                ScriptListRowHeights::for_theme(crate::designs::current_main_menu_theme()),
            ),
            124.0
        );
    }

    #[test]
    fn visible_range_includes_partial_rows_and_variable_headers() {
        let rows = vec![
            GroupedListItem::SectionHeader("Recent".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Status(crate::list_item::SourceChipStatusRow {
                source: crate::menu_syntax::RootUnifiedSourceFilter::Files,
                source_name: "Files".to_string(),
                status_kind: crate::list_item::SourceChipStatusKind::Loading,
                label: "Loading files".to_string(),
                shown: 0,
                loaded: 0,
                total: None,
            }),
            GroupedListItem::Item(1),
        ];
        let heights = ScriptListRowHeights {
            first_section_header: 20.0,
            section_header: 32.0,
            status: 24.0,
            item: 40.0,
        };

        assert_eq!(main_list_visible_range(&rows, 10.0, 55.0, heights), (0, 3));
        assert_eq!(main_list_visible_range(&rows, 61.0, 22.0, heights), (2, 3));
    }

    #[test]
    fn generated_row_identity_is_deterministic_when_model_has_no_stable_key() {
        let row = GroupedListItem::Item(7);
        assert_eq!(
            main_list_row_stable_key(&row, 11, &[]),
            "generated/item/11/7"
        );
    }
}

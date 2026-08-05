use crate::ui_foundation::{is_key_down, is_key_enter, is_key_escape, is_key_space, is_key_up};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DynamicUniformListSelectionSnapshot<K> {
    pub(crate) key: Option<K>,
    pub(crate) fallback_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DynamicUniformListViewportSnapshot<K> {
    /// The current anchor followed by later rows in their old order. If the
    /// anchor disappears, the first surviving successor is deterministic.
    pub(crate) anchor_candidates: Vec<K>,
    pub(crate) fallback_index: usize,
    pub(crate) fractional_offset: f32,
    pub(crate) pending_reveal: Option<(K, gpui::ScrollStrategy)>,
    pub(crate) previous_item_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DynamicUniformListSnapshot<K> {
    pub(crate) selection: DynamicUniformListSelectionSnapshot<K>,
    pub(crate) viewport: DynamicUniformListViewportSnapshot<K>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DynamicUniformListRestore {
    pub(crate) selected_index: usize,
    pub(crate) selected_key_survived: bool,
    pub(crate) viewport_index: usize,
    pub(crate) viewport_key_survived: bool,
    pub(crate) fractional_offset: f32,
    pub(crate) pending_reveal: Option<(usize, gpui::ScrollStrategy)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DynamicTrackedListState {
    filter: String,
    keys: Vec<String>,
    selected_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct DynamicTrackedListSnapshot {
    selection: DynamicUniformListSelectionSnapshot<String>,
    viewport_anchor_candidates: Vec<String>,
    viewport_fallback_index: usize,
    offset_in_anchor_px: f32,
    measured_heights: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DynamicTrackedListRestore {
    selected_index: usize,
    viewport_index: usize,
    offset_in_anchor_px: f32,
    absolute_scroll_top_px: f32,
}

fn capture_dynamic_tracked_list_snapshot(
    keys: &[String],
    selected_index: usize,
    handle: &gpui::ScrollHandle,
) -> DynamicTrackedListSnapshot {
    let (viewport_index, offset_in_anchor) = handle.logical_scroll_top();
    let measured_heights = keys
        .iter()
        .enumerate()
        .map(|(index, _)| {
            handle
                .bounds_for_item(index)
                .map(|bounds| bounds.size.height.as_f32())
                .filter(|height| *height > 0.0)
                .unwrap_or(crate::list_item::LIST_ITEM_HEIGHT)
        })
        .collect();

    DynamicTrackedListSnapshot {
        selection: DynamicUniformListSelectionSnapshot {
            key: keys.get(selected_index).cloned(),
            fallback_index: selected_index,
        },
        viewport_anchor_candidates: keys.iter().skip(viewport_index).cloned().collect(),
        viewport_fallback_index: viewport_index,
        offset_in_anchor_px: offset_in_anchor.as_f32(),
        measured_heights,
    }
}

fn reconcile_dynamic_tracked_list_snapshot(
    snapshot: &DynamicTrackedListSnapshot,
    old_keys: &[String],
    new_keys: &[String],
) -> DynamicTrackedListRestore {
    if new_keys.is_empty() {
        return DynamicTrackedListRestore {
            selected_index: 0,
            viewport_index: 0,
            offset_in_anchor_px: 0.0,
            absolute_scroll_top_px: 0.0,
        };
    }

    let selected_index = snapshot
        .selection
        .key
        .as_ref()
        .and_then(|key| new_keys.iter().position(|candidate| candidate == key))
        .unwrap_or_else(|| {
            snapshot
                .selection
                .fallback_index
                .min(new_keys.len().saturating_sub(1))
        });
    let viewport_index = snapshot
        .viewport_anchor_candidates
        .iter()
        .find_map(|key| new_keys.iter().position(|candidate| candidate == key))
        .unwrap_or_else(|| {
            snapshot
                .viewport_fallback_index
                .min(new_keys.len().saturating_sub(1))
        });
    let fallback_height = snapshot
        .measured_heights
        .iter()
        .copied()
        .filter(|height| *height > 0.0)
        .sum::<f32>()
        / snapshot.measured_heights.len().max(1) as f32;
    let absolute_scroll_top_px = new_keys
        .iter()
        .take(viewport_index)
        .map(|key| {
            old_keys
                .iter()
                .position(|candidate| candidate == key)
                .and_then(|old_index| snapshot.measured_heights.get(old_index).copied())
                .unwrap_or(fallback_height)
        })
        .sum::<f32>()
        - snapshot.offset_in_anchor_px;

    DynamicTrackedListRestore {
        selected_index,
        viewport_index,
        offset_in_anchor_px: snapshot.offset_in_anchor_px,
        absolute_scroll_top_px: absolute_scroll_top_px.max(0.0),
    }
}

pub(crate) fn reconcile_dynamic_tracked_list_on_render(
    state: &mut DynamicTrackedListState,
    filter: &str,
    new_keys: &[String],
    selected_index: usize,
    handle: &gpui::ScrollHandle,
) -> usize {
    let selected_index = selected_index.min(new_keys.len().saturating_sub(1));
    if state.filter != filter || state.keys.is_empty() {
        state.filter = filter.to_string();
        state.keys = new_keys.to_vec();
        state.selected_key = new_keys.get(selected_index).cloned();
        return selected_index;
    }

    if state.keys == new_keys {
        state.selected_key = new_keys.get(selected_index).cloned();
        return selected_index;
    }

    let old_selected_index = state
        .selected_key
        .as_ref()
        .and_then(|key| state.keys.iter().position(|candidate| candidate == key))
        .unwrap_or(selected_index.min(state.keys.len().saturating_sub(1)));
    let snapshot = capture_dynamic_tracked_list_snapshot(&state.keys, old_selected_index, handle);
    let restore = reconcile_dynamic_tracked_list_snapshot(&snapshot, &state.keys, new_keys);
    handle.set_offset(gpui::point(
        handle.offset().x,
        gpui::px(-restore.absolute_scroll_top_px),
    ));
    state.filter = filter.to_string();
    state.keys = new_keys.to_vec();
    state.selected_key = new_keys.get(restore.selected_index).cloned();
    restore.selected_index
}

fn dynamic_uniform_list_snapshot_from_viewport<K: Clone>(
    keys: &[K],
    selected_index: usize,
    viewport_index: usize,
    fractional_offset: f32,
    pending_reveal: Option<(usize, gpui::ScrollStrategy)>,
) -> DynamicUniformListSnapshot<K> {
    DynamicUniformListSnapshot {
        selection: DynamicUniformListSelectionSnapshot {
            key: keys.get(selected_index).cloned(),
            fallback_index: selected_index,
        },
        viewport: DynamicUniformListViewportSnapshot {
            anchor_candidates: keys.iter().skip(viewport_index).cloned().collect(),
            fallback_index: viewport_index,
            fractional_offset: fractional_offset.clamp(0.0, 0.999_999),
            pending_reveal: pending_reveal
                .and_then(|(index, strategy)| keys.get(index).cloned().map(|key| (key, strategy))),
            previous_item_count: keys.len(),
        },
    }
}

pub(crate) fn capture_dynamic_uniform_list_snapshot<K: Clone>(
    keys: &[K],
    selected_index: usize,
    handle: &gpui::UniformListScrollHandle,
) -> DynamicUniformListSnapshot<K> {
    let state = handle.0.borrow();
    let (viewport_index, offset_in_item) = state.base_handle.logical_scroll_top();
    let row_height = state
        .last_item_size
        .filter(|_| !keys.is_empty())
        .map(|size| size.contents.height.as_f32() / keys.len() as f32)
        .filter(|height| *height > 0.0)
        .unwrap_or(crate::list_item::LIST_ITEM_HEIGHT);
    let fractional_offset = (-offset_in_item.as_f32() / row_height).clamp(0.0, 0.999_999);
    let pending_reveal = state
        .deferred_scroll_to_item
        .map(|pending| (pending.item_index, pending.strategy));
    drop(state);

    dynamic_uniform_list_snapshot_from_viewport(
        keys,
        selected_index,
        viewport_index,
        fractional_offset,
        pending_reveal,
    )
}

pub(crate) fn reconcile_dynamic_uniform_list_snapshot<K: PartialEq>(
    snapshot: &DynamicUniformListSnapshot<K>,
    new_keys: &[K],
) -> DynamicUniformListRestore {
    if new_keys.is_empty() {
        return DynamicUniformListRestore {
            selected_index: 0,
            selected_key_survived: false,
            viewport_index: 0,
            viewport_key_survived: false,
            fractional_offset: 0.0,
            pending_reveal: None,
        };
    }

    let surviving_selection = snapshot
        .selection
        .key
        .as_ref()
        .and_then(|key| new_keys.iter().position(|candidate| candidate == key));
    let selected_index = surviving_selection.unwrap_or_else(|| {
        snapshot
            .selection
            .fallback_index
            .min(new_keys.len().saturating_sub(1))
    });

    let surviving_viewport = snapshot
        .viewport
        .anchor_candidates
        .iter()
        .find_map(|key| new_keys.iter().position(|candidate| candidate == key));
    let viewport_index = surviving_viewport.unwrap_or_else(|| {
        snapshot
            .viewport
            .fallback_index
            .min(new_keys.len().saturating_sub(1))
    });
    let pending_reveal = snapshot
        .viewport
        .pending_reveal
        .as_ref()
        .and_then(|(key, strategy)| {
            new_keys
                .iter()
                .position(|candidate| candidate == key)
                .map(|index| (index, *strategy))
        });

    DynamicUniformListRestore {
        selected_index,
        selected_key_survived: surviving_selection.is_some(),
        viewport_index,
        viewport_key_survived: surviving_viewport.is_some(),
        fractional_offset: snapshot.viewport.fractional_offset,
        pending_reveal,
    }
}

pub(crate) fn restore_dynamic_uniform_list_viewport(
    handle: &gpui::UniformListScrollHandle,
    snapshot: &DynamicUniformListSnapshot<impl PartialEq>,
    restore: DynamicUniformListRestore,
) {
    let mut state = handle.0.borrow_mut();
    if let Some((item_index, strategy)) = restore.pending_reveal {
        state.deferred_scroll_to_item = Some(gpui::DeferredScrollToItem {
            item_index,
            strategy,
            offset: 0,
            scroll_strict: false,
        });
        return;
    }

    state.deferred_scroll_to_item = None;
    let row_height = state
        .last_item_size
        .filter(|_| snapshot.viewport.previous_item_count > 0)
        .map(|size| size.contents.height.as_f32() / snapshot.viewport.previous_item_count as f32)
        .filter(|height| *height > 0.0)
        .unwrap_or(crate::list_item::LIST_ITEM_HEIGHT);
    let absolute_offset = (restore.viewport_index as f32 + restore.fractional_offset) * row_height;
    state
        .base_handle
        .set_offset(gpui::point(gpui::px(0.0), gpui::px(-absolute_offset)));
}

pub(crate) fn render_builtin_split_main_content_layout(
    list_pane: gpui::AnyElement,
    preview_pane: gpui::AnyElement,
) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_row()
        .flex_1()
        .min_h(gpui::px(0.))
        .w_full()
        .overflow_hidden()
        .child(
            gpui::div()
                .flex_1()
                .h_full()
                .min_w(gpui::px(0.))
                .min_h(gpui::px(0.))
                .child(list_pane),
        )
        .child(
            gpui::div()
                .flex_1()
                .h_full()
                .min_h(gpui::px(0.))
                .overflow_hidden()
                .child(preview_pane),
        )
        .into_any_element()
}

#[cfg(test)]
mod dynamic_uniform_list_policy_tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum ViewportTransport {
        PixelWheel,
        LineWheel,
        Momentum,
        Scrollbar,
    }

    #[derive(Debug)]
    struct Case {
        name: &'static str,
        old_keys: Vec<u32>,
        new_keys: Vec<u32>,
        selected: usize,
        viewport: usize,
        fraction: f32,
        expected_selected: usize,
        expected_viewport: usize,
        selection_survives: bool,
        viewport_survives: bool,
    }

    #[test]
    fn clipboard_process_manager_kit_store_refresh_reorder_delete_filter_matrix() {
        let long_old: Vec<u32> = (0..64).collect();
        let mut long_new = vec![1000, 1001];
        long_new.extend(0..64);
        let cases = vec![
            Case {
                name: "refresh-reorder-offscreen-selection",
                old_keys: vec![10, 11, 12, 13, 14, 15],
                new_keys: vec![99, 13, 14, 10, 11, 12, 15],
                selected: 1,
                viewport: 3,
                fraction: 0.375,
                expected_selected: 4,
                expected_viewport: 1,
                selection_survives: true,
                viewport_survives: true,
            },
            Case {
                name: "deleted-anchor-falls-forward",
                old_keys: vec![10, 11, 12, 13, 14],
                new_keys: vec![10, 11, 13, 14],
                selected: 0,
                viewport: 2,
                fraction: 0.5,
                expected_selected: 0,
                expected_viewport: 2,
                selection_survives: true,
                viewport_survives: true,
            },
            Case {
                name: "filter-removes-selection-but-keeps-anchor",
                old_keys: vec![10, 11, 12, 13, 14],
                new_keys: vec![13, 14],
                selected: 2,
                viewport: 3,
                fraction: 0.25,
                expected_selected: 1,
                expected_viewport: 0,
                selection_survives: false,
                viewport_survives: true,
            },
            Case {
                name: "short-dataset-endpoint",
                old_keys: vec![7],
                new_keys: vec![7],
                selected: 0,
                viewport: 0,
                fraction: 0.0,
                expected_selected: 0,
                expected_viewport: 0,
                selection_survives: true,
                viewport_survives: true,
            },
            Case {
                name: "long-dataset-insertion-after-scroll",
                old_keys: long_old,
                new_keys: long_new,
                selected: 2,
                viewport: 40,
                fraction: 0.875,
                expected_selected: 4,
                expected_viewport: 42,
                selection_survives: true,
                viewport_survives: true,
            },
            Case {
                name: "deleted-tail-anchor-clamps-old-ordinal",
                old_keys: vec![1, 2, 3, 4],
                new_keys: vec![1, 2],
                selected: 3,
                viewport: 3,
                fraction: 0.75,
                expected_selected: 1,
                expected_viewport: 1,
                selection_survives: false,
                viewport_survives: false,
            },
        ];

        for transport in [
            ViewportTransport::PixelWheel,
            ViewportTransport::LineWheel,
            ViewportTransport::Momentum,
            ViewportTransport::Scrollbar,
        ] {
            for case in &cases {
                let snapshot = dynamic_uniform_list_snapshot_from_viewport(
                    &case.old_keys,
                    case.selected,
                    case.viewport,
                    case.fraction,
                    None,
                );
                let restored = reconcile_dynamic_uniform_list_snapshot(&snapshot, &case.new_keys);
                assert_eq!(
                    restored.selected_index, case.expected_selected,
                    "{} via {transport:?}: viewport transport must not reinterpret selection",
                    case.name
                );
                assert_eq!(
                    restored.viewport_index, case.expected_viewport,
                    "{} via {transport:?}",
                    case.name
                );
                assert_eq!(
                    restored.selected_key_survived, case.selection_survives,
                    "{} via {transport:?}",
                    case.name
                );
                assert_eq!(
                    restored.viewport_key_survived, case.viewport_survives,
                    "{} via {transport:?}",
                    case.name
                );
                assert_eq!(restored.fractional_offset, case.fraction);
            }
        }
    }

    #[test]
    fn pending_keyboard_reveal_is_remapped_by_identity_not_old_ordinal() {
        let snapshot = dynamic_uniform_list_snapshot_from_viewport(
            &[10, 11, 12, 13],
            1,
            3,
            0.4,
            Some((1, gpui::ScrollStrategy::Nearest)),
        );
        let restored = reconcile_dynamic_uniform_list_snapshot(&snapshot, &[99, 13, 12, 11, 10]);
        assert_eq!(restored.selected_index, 3);
        assert_eq!(restored.viewport_index, 1);
        assert_eq!(
            restored.pending_reveal,
            Some((3, gpui::ScrollStrategy::Nearest))
        );
    }

    #[test]
    fn empty_refresh_is_safe_and_stationary_pointer_stays_suppressed() {
        let snapshot = dynamic_uniform_list_snapshot_from_viewport(&[1, 2, 3], 2, 2, 0.9, None);
        let restored = reconcile_dynamic_uniform_list_snapshot(&snapshot, &[]);
        assert_eq!(restored.selected_index, 0);
        assert_eq!(restored.viewport_index, 0);
        assert_eq!(restored.fractional_offset, 0.0);

        let mut pointer = crate::scrolling::list_interaction::ListPointerPolicy {
            hovered_index: Some(2),
            suppress_hover_until_pointer_move: false,
        };
        pointer.begin_viewport_scroll();
        pointer.note_hover_change(1, true);
        assert_eq!(pointer.hovered_index, None);
        pointer.note_pointer_move(1);
        assert_eq!(pointer.hovered_index, Some(1));
    }

    #[test]
    fn tracked_scroll_column_refresh_preserves_independent_mixed_height_anchors() {
        let old_keys = ["short", "wrapped", "multiline", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let new_keys = ["inserted", "multiline", "short", "wrapped", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let snapshot = DynamicTrackedListSnapshot {
            selection: DynamicUniformListSelectionSnapshot {
                key: Some("short".to_string()),
                fallback_index: 0,
            },
            viewport_anchor_candidates: vec!["multiline".to_string(), "tail".to_string()],
            viewport_fallback_index: 2,
            offset_in_anchor_px: -11.5,
            measured_heights: vec![28.0, 54.0, 91.0, 36.0],
        };

        let restore = reconcile_dynamic_tracked_list_snapshot(&snapshot, &old_keys, &new_keys);
        assert_eq!(restore.selected_index, 2, "selection follows its row ID");
        assert_eq!(restore.viewport_index, 1, "viewport follows its own row ID");
        assert_eq!(restore.offset_in_anchor_px, -11.5);
        assert!(
            restore.absolute_scroll_top_px > 11.5,
            "a new leading row contributes measured/fallback height without coupling selection"
        );

        let filtered = vec!["tail".to_string()];
        let filtered_restore =
            reconcile_dynamic_tracked_list_snapshot(&snapshot, &old_keys, &filtered);
        assert_eq!(filtered_restore.selected_index, 0);
        assert_eq!(filtered_restore.viewport_index, 0);

        let empty = reconcile_dynamic_tracked_list_snapshot(&snapshot, &old_keys, &[]);
        assert_eq!(empty.selected_index, 0);
        assert_eq!(empty.viewport_index, 0);
        assert_eq!(empty.absolute_scroll_top_px, 0.0);
    }
}

#[cfg(test)]
mod tracked_scroll_column_behavior_tests {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use gpui::{
        div, point, prelude::*, px, size, AppContext as _, Context, Entity, IntoElement, Modifiers,
        Render, ScrollDelta, ScrollHandle, ScrollPhase, ScrollWheelEvent, TestAppContext,
        TouchPhase, VisualTestContext, Window,
    };

    const VIEWPORT_WIDTH: f32 = 320.0;
    const VIEWPORT_HEIGHT: f32 = 150.0;

    struct Harness {
        handle: ScrollHandle,
        row_heights: Rc<Vec<f32>>,
        selected: Rc<Cell<usize>>,
        hovered: Rc<Cell<Option<usize>>>,
        suppress_hover: Rc<Cell<bool>>,
        observed_sources:
            Rc<RefCell<Vec<crate::scrolling::list_interaction::ListViewportInputSource>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let selected = self.selected.clone();
            let hovered = self.hovered.clone();
            let suppress_hover = self.suppress_hover.clone();
            let handle = self.handle.clone();
            let rows = self
                .row_heights
                .iter()
                .enumerate()
                .map(move |(index, height)| {
                    let click_selected = selected.clone();
                    let click_hovered = hovered.clone();
                    let click_suppress = suppress_hover.clone();
                    let click_handle = handle.clone();
                    let move_hovered = hovered.clone();
                    let move_suppress = suppress_hover.clone();
                    let leave_hovered = hovered.clone();
                    div()
                        .id(("tracked-scroll-test-row", index))
                        .h(px(*height))
                        .w_full()
                        .on_click(move |_event, _window, _cx| {
                            click_selected.set(index);
                            click_hovered.set(Some(index));
                            click_suppress.set(false);
                            click_handle.scroll_to_item(index);
                        })
                        .on_mouse_move(move |_event, _window, _cx| {
                            move_hovered.set(Some(index));
                            move_suppress.set(false);
                        })
                        .on_hover(move |is_hovered, _window, _cx| {
                            if !*is_hovered && leave_hovered.get() == Some(index) {
                                leave_hovered.set(None);
                            }
                        })
                        .when(selected.get() == index, |row| {
                            row.child(div().id("tracked-scroll-test-selection"))
                        })
                });

            let hovered = self.hovered.clone();
            let suppress_hover = self.suppress_hover.clone();
            let observed_sources = self.observed_sources.clone();
            div()
                .relative()
                .size_full()
                .overflow_hidden()
                .on_scroll_wheel(move |event: &ScrollWheelEvent, _window, _cx| {
                    hovered.set(None);
                    suppress_hover.set(true);
                    observed_sources.borrow_mut().push(
                        crate::scrolling::list_interaction::ListViewportInputSource::from_event(
                            event,
                        ),
                    );
                })
                .child(crate::components::scrollbar::render_tracked_scroll_column(
                    "tracked-scroll-test-list",
                    &self.handle,
                    rows,
                ))
        }
    }

    struct Fixture {
        entity: Entity<Harness>,
        handle: ScrollHandle,
        selected: Rc<Cell<usize>>,
        hovered: Rc<Cell<Option<usize>>>,
        suppress_hover: Rc<Cell<bool>>,
        observed_sources:
            Rc<RefCell<Vec<crate::scrolling::list_interaction::ListViewportInputSource>>>,
    }

    fn fixture(cx: &mut TestAppContext, row_heights: Vec<f32>) -> Fixture {
        let handle = ScrollHandle::new();
        let selected = Rc::new(Cell::new(0));
        let hovered = Rc::new(Cell::new(None));
        let suppress_hover = Rc::new(Cell::new(false));
        let observed_sources = Rc::new(RefCell::new(Vec::new()));
        let entity = cx.new(|_| Harness {
            handle: handle.clone(),
            row_heights: Rc::new(row_heights),
            selected: selected.clone(),
            hovered: hovered.clone(),
            suppress_hover: suppress_hover.clone(),
            observed_sources: observed_sources.clone(),
        });
        Fixture {
            entity,
            handle,
            selected,
            hovered,
            suppress_hover,
            observed_sources,
        }
    }

    fn draw(vcx: &mut VisualTestContext, entity: &Entity<Harness>) {
        let entity = entity.clone();
        vcx.draw(
            point(px(0.0), px(0.0)),
            size(px(VIEWPORT_WIDTH), px(VIEWPORT_HEIGHT)),
            move |_window, _cx| entity.into_any_element(),
        );
    }

    fn dispatch(vcx: &mut VisualTestContext, fixture: &Fixture, event: ScrollWheelEvent) {
        vcx.simulate_event(event);
        draw(vcx, &fixture.entity);
    }

    fn pixel_event(delta_y: f32, momentum_phase: ScrollPhase) -> ScrollWheelEvent {
        ScrollWheelEvent {
            position: point(px(40.0), px(40.0)),
            delta: ScrollDelta::Pixels(point(px(0.0), px(delta_y))),
            touch_phase: TouchPhase::Moved,
            phase: if momentum_phase == ScrollPhase::None {
                ScrollPhase::Changed
            } else {
                ScrollPhase::None
            },
            momentum_phase,
            ..Default::default()
        }
    }

    fn line_event(delta_y: f32) -> ScrollWheelEvent {
        ScrollWheelEvent {
            position: point(px(40.0), px(40.0)),
            delta: ScrollDelta::Lines(point(0.0, delta_y)),
            touch_phase: TouchPhase::Moved,
            phase: ScrollPhase::Changed,
            ..Default::default()
        }
    }

    #[gpui::test]
    fn tracked_scroll_column_native_pixel_line_momentum_scrollbar_and_endpoints(
        cx: &mut TestAppContext,
    ) {
        for heights in [vec![28.0, 44.0], vec![28.0, 72.0, 41.0, 96.0, 35.0, 64.0]] {
            let fixture = fixture(cx, heights);
            let mut vcx = cx.add_empty_window();
            draw(&mut vcx, &fixture.entity);
            let selected_before = fixture.selected.get();

            for event in [
                pixel_event(-7.5, ScrollPhase::None),
                line_event(-1.0),
                pixel_event(-5.25, ScrollPhase::Began),
                pixel_event(-4.75, ScrollPhase::Changed),
                pixel_event(0.0, ScrollPhase::Ended),
            ] {
                dispatch(&mut vcx, &fixture, event);
                assert_eq!(fixture.selected.get(), selected_before);
            }

            if fixture.handle.max_offset().y.as_f32() < 0.0 {
                assert!(fixture.handle.offset().y.as_f32() < 0.0);
                dispatch(
                    &mut vcx,
                    &fixture,
                    pixel_event(-100_000.0, ScrollPhase::None),
                );
                assert_eq!(fixture.handle.offset().y, fixture.handle.max_offset().y);
                dispatch(
                    &mut vcx,
                    &fixture,
                    pixel_event(100_000.0, ScrollPhase::None),
                );
                assert_eq!(fixture.handle.offset().y, px(0.0));

                fixture.handle.set_offset(point(px(0.0), px(-33.5)));
                draw(&mut vcx, &fixture.entity);
                assert_eq!(fixture.selected.get(), selected_before);
                assert!(fixture.handle.offset().y.as_f32() < 0.0);
            } else {
                assert_eq!(fixture.handle.offset().y, px(0.0));
            }
        }
    }

    #[gpui::test]
    fn tracked_scroll_column_stationary_pointer_click_and_keyboard_reveal(cx: &mut TestAppContext) {
        let fixture = fixture(cx, vec![28.0, 72.0, 41.0, 96.0, 35.0, 64.0]);
        let mut vcx = cx.add_empty_window();
        draw(&mut vcx, &fixture.entity);

        vcx.simulate_mouse_move(
            fixture.handle.bounds_for_item(1).unwrap().center(),
            None,
            Modifiers::default(),
        );
        draw(&mut vcx, &fixture.entity);
        assert_eq!(fixture.hovered.get(), Some(1));

        dispatch(&mut vcx, &fixture, pixel_event(-18.0, ScrollPhase::None));
        assert_eq!(fixture.selected.get(), 0);
        assert_eq!(fixture.hovered.get(), None);
        assert!(fixture.suppress_hover.get());
        assert_eq!(
            fixture.observed_sources.borrow().last().copied(),
            Some(crate::scrolling::list_interaction::ListViewportInputSource::Wheel)
        );

        let click_bounds = fixture.handle.bounds_for_item(2).unwrap();
        vcx.simulate_click(click_bounds.center(), Modifiers::default());
        draw(&mut vcx, &fixture.entity);
        assert_eq!(fixture.selected.get(), 2);

        let last = 5;
        fixture.selected.set(last);
        fixture.handle.scroll_to_item(last);
        draw(&mut vcx, &fixture.entity);
        assert!(fixture.handle.bottom_item() >= last);

        fixture.handle.scroll_to_item(last);
        draw(&mut vcx, &fixture.entity);
        assert_eq!(
            fixture.selected.get(),
            last,
            "clamped endpoint reveal is repeatable"
        );
    }
}

impl ScriptListApp {
    fn active_list_input_mode(&self) -> &'static str {
        match self.input_mode {
            InputMode::Keyboard => "keyboard",
            InputMode::Mouse => "mouse",
        }
    }

    fn active_uniform_list_scroll_receipt(
        &self,
        surface: &'static str,
        handle: &UniformListScrollHandle,
        semantic_ids: Vec<String>,
        selected_index: usize,
        focused_semantic_id: &'static str,
    ) -> serde_json::Value {
        let item_count = semantic_ids.len();
        let selected_index = (!semantic_ids.is_empty())
            .then_some(selected_index.min(item_count.saturating_sub(1)));
        let hovered_index = self.hovered_index.filter(|index| *index < item_count);
        let state = handle.0.borrow();
        let (scroll_top_item, offset_in_item) = state.base_handle.logical_scroll_top();
        let scroll_top_item = scroll_top_item.min(item_count.saturating_sub(1));
        let measured = state.last_item_size.filter(|_| item_count > 0);
        let row_height = measured
            .map(|size| size.contents.height.as_f32() / item_count as f32)
            .filter(|height| height.is_finite() && *height > 0.0);
        let viewport_height = measured.map(|size| size.item.height.as_f32().max(0.0));
        let content_height = measured.map(|size| size.contents.height.as_f32().max(0.0));
        let scroll_top_offset_px =
            row_height.map(|height| (-offset_in_item.as_f32()).clamp(0.0, height));
        let scroll_top_offset_items = scroll_top_offset_px
            .zip(row_height)
            .map(|(offset, height)| (offset / height).clamp(0.0, 0.999_999));
        let logical_scroll_top =
            scroll_top_offset_items.map(|offset| scroll_top_item as f32 + offset);
        let scroll_top_px = row_height
            .zip(scroll_top_offset_px)
            .map(|(height, offset)| scroll_top_item as f32 * height + offset);
        let last_visible_index_exclusive = viewport_height
            .zip(row_height)
            .zip(scroll_top_offset_px)
            .map(|((viewport, height), offset)| {
                (scroll_top_item + ((viewport + offset) / height).ceil() as usize).min(item_count)
            });
        let first_visible_index = (!semantic_ids.is_empty()).then_some(scroll_top_item);
        let selected_row_top = selected_index
            .zip(row_height)
            .zip(scroll_top_offset_px)
            .map(|((selected, height), offset)| {
                (selected as isize - scroll_top_item as isize) as f32 * height - offset
            });
        let selected_row_within_safe_viewport = selected_row_top
            .zip(row_height)
            .zip(viewport_height)
            .map(|((top, height), viewport)| top >= 0.0 && top + height <= viewport);

        serde_json::json!({
            "surface": surface,
            "implementation": "uniform_list",
            "listKind": "uniform_list",
            "selectedIndex": selected_index,
            "selectedSemanticId": selected_index.and_then(|index| semantic_ids.get(index).cloned()),
            "hoveredIndex": hovered_index,
            "hoveredSemanticId": hovered_index.and_then(|index| semantic_ids.get(index).cloned()),
            "hoverSuppressedUntilPointerMove": self.list_suppress_hover_until_pointer_move,
            "focusedSemanticId": (self.focused_input == FocusedInput::MainFilter).then_some(focused_semantic_id),
            "logicalScrollTop": logical_scroll_top,
            "scrollTopItem": first_visible_index,
            "scrollTopOffsetItems": scroll_top_offset_items,
            "scrollTopOffsetPx": scroll_top_offset_px,
            "scrollTop": scroll_top_px,
            "firstVisibleIndex": first_visible_index,
            "lastVisibleIndexExclusive": last_visible_index_exclusive,
            "firstVisibleSemanticId": first_visible_index.and_then(|index| semantic_ids.get(index).cloned()),
            "lastVisibleSemanticId": last_visible_index_exclusive
                .and_then(|exclusive| exclusive.checked_sub(1))
                .and_then(|index| semantic_ids.get(index).cloned()),
            "itemCount": item_count,
            "contentHeight": content_height,
            "viewportHeight": viewport_height,
            "safeViewportHeight": viewport_height,
            "maxScrollTop": content_height.zip(viewport_height).map(|(content, viewport)| (content - viewport).max(0.0)),
            "selectedRowVisible": selected_row_within_safe_viewport,
            "selectedRowWithinSafeViewport": selected_row_within_safe_viewport,
            "inputMode": self.active_list_input_mode(),
            "lastInteractionSource": self.last_list_interaction_source.as_str(),
        })
    }

    fn active_tracked_list_scroll_receipt(
        &self,
        surface: &'static str,
        handle: &gpui::ScrollHandle,
        semantic_ids: Vec<String>,
        selected_index: usize,
        focused_semantic_id: &'static str,
    ) -> serde_json::Value {
        let item_count = semantic_ids.len();
        let selected_index = (!semantic_ids.is_empty())
            .then_some(selected_index.min(item_count.saturating_sub(1)));
        let hovered_index = self.hovered_index.filter(|index| *index < item_count);
        let (scroll_top_item, offset_in_item) = handle.logical_scroll_top();
        let scroll_top_item = scroll_top_item.min(item_count.saturating_sub(1));
        let row_height = handle
            .bounds_for_item(scroll_top_item)
            .map(|bounds| bounds.size.height.as_f32())
            .filter(|height| height.is_finite() && *height > 0.0);
        let scroll_top_offset_px =
            row_height.map(|height| (-offset_in_item.as_f32()).clamp(0.0, height));
        let scroll_top_offset_items = scroll_top_offset_px
            .zip(row_height)
            .map(|(offset, height)| (offset / height).clamp(0.0, 0.999_999));
        let logical_scroll_top =
            scroll_top_offset_items.map(|offset| scroll_top_item as f32 + offset);
        let first_visible_index = (!semantic_ids.is_empty()).then_some(scroll_top_item);
        let last_visible_index_exclusive = (!semantic_ids.is_empty())
            .then_some(handle.bottom_item().saturating_add(1).min(item_count));
        let viewport_height = handle.bounds().size.height.as_f32().max(0.0);
        let scroll_top_px = (-handle.offset().y.as_f32()).max(0.0);
        let max_scroll_top = (-handle.max_offset().y.as_f32()).max(0.0);
        let selected_row_within_safe_viewport = selected_index
            .and_then(|index| handle.bounds_for_item(index))
            .map(|bounds| {
                let viewport = handle.bounds();
                let top = bounds.top().as_f32() + handle.offset().y.as_f32();
                let bottom = bounds.bottom().as_f32() + handle.offset().y.as_f32();
                top >= viewport.top().as_f32() && bottom <= viewport.bottom().as_f32()
            });

        serde_json::json!({
            "surface": surface,
            "implementation": "tracked_column",
            "listKind": "tracked_column",
            "selectedIndex": selected_index,
            "selectedSemanticId": selected_index.and_then(|index| semantic_ids.get(index).cloned()),
            "hoveredIndex": hovered_index,
            "hoveredSemanticId": hovered_index.and_then(|index| semantic_ids.get(index).cloned()),
            "hoverSuppressedUntilPointerMove": self.list_suppress_hover_until_pointer_move,
            "focusedSemanticId": (self.focused_input == FocusedInput::MainFilter).then_some(focused_semantic_id),
            "logicalScrollTop": logical_scroll_top,
            "scrollTopItem": first_visible_index,
            "scrollTopOffsetItems": scroll_top_offset_items,
            "scrollTopOffsetPx": scroll_top_offset_px,
            "scrollTop": scroll_top_px,
            "firstVisibleIndex": first_visible_index,
            "lastVisibleIndexExclusive": last_visible_index_exclusive,
            "firstVisibleSemanticId": first_visible_index.and_then(|index| semantic_ids.get(index).cloned()),
            "lastVisibleSemanticId": last_visible_index_exclusive
                .and_then(|exclusive| exclusive.checked_sub(1))
                .and_then(|index| semantic_ids.get(index).cloned()),
            "bottomItem": (!semantic_ids.is_empty())
                .then_some(handle.bottom_item().min(item_count.saturating_sub(1))),
            "itemCount": item_count,
            "contentHeight": viewport_height + max_scroll_top,
            "viewportHeight": viewport_height,
            "safeViewportHeight": viewport_height,
            "maxScrollTop": max_scroll_top,
            "selectedRowVisible": selected_row_within_safe_viewport,
            "selectedRowWithinSafeViewport": selected_row_within_safe_viewport,
            "inputMode": self.active_list_input_mode(),
            "lastInteractionSource": self.last_list_interaction_source.as_str(),
        })
    }

    /// One stable semantic/viewport schema for every native-scrolling builtin.
    /// Returns `None` for non-migrated or non-list surfaces so callers fail
    /// closed instead of projecting unrelated state onto this contract.
    pub(crate) fn active_builtin_list_scroll_receipt(&self) -> Option<serde_json::Value> {
        match &self.current_view {
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                let ids = Self::app_launcher_filtered_entries(&self.apps, filter)
                    .into_iter()
                    .map(|(_, app)| {
                        app.bundle_id
                            .clone()
                            .unwrap_or_else(|| app.path.to_string_lossy().into_owned())
                    })
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "app_launcher",
                    &self.list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:app-filter",
                ))
            }
            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                let ids = self
                    .browser_tabs_visible_rows(filter)
                    .iter()
                    .map(crate::browser_tabs::browser_tab_stable_key)
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "browser_tabs",
                    &self.browser_tabs_scroll_handle,
                    ids,
                    *selected_index,
                    "input:browser-tabs-filter",
                ))
            }
            AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } => {
                let ids = Self::current_app_commands_filtered_entries(
                    &self.cached_current_app_entries,
                    filter,
                )
                .into_iter()
                .map(|(source_index, entry)| {
                    format!("current-app-command-{source_index}-{}", entry.id)
                })
                .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "current_app_commands",
                    &self.current_app_commands_scroll_handle,
                    ids,
                    *selected_index,
                    "input:current-app-commands-filter",
                ))
            }
            AppView::TipsView {
                filter,
                selected_index,
                entries,
            } => {
                let ids = script_kit_gpui::tips::visible_tip_indices(entries, filter)
                    .into_iter()
                    .filter_map(|source_index| {
                        entries
                            .get(source_index)
                            .map(|tip| format!("tip-{source_index}-{}", tip.title))
                    })
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "tips",
                    &self.tips_list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:tips-filter",
                ))
            }
            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => {
                let needle = filter.to_lowercase();
                let ids = self
                    .cached_windows
                    .iter()
                    .filter(|window| {
                        filter.is_empty()
                            || window.title.to_lowercase().contains(&needle)
                            || window.app.to_lowercase().contains(&needle)
                    })
                    .map(|window| format!("window-switcher:{}", window.id))
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "window_switcher",
                    &self.window_list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:window-filter",
                ))
            }
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                let ids = self
                    .clipboard_history_visible_rows(filter)
                    .into_iter()
                    .map(|(_, entry)| format!("clipboard-history:{}", entry.id))
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "clipboard_history",
                    &self.clipboard_list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:clipboard-filter",
                ))
            }
            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                let ids = Self::process_manager_filtered_entries(&self.cached_processes, filter)
                    .into_iter()
                    .map(|(_, process)| format!("process-manager:{}", process.pid))
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "process_manager",
                    &self.process_list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:process-filter",
                ))
            }
            AppView::BrowseKitsView {
                selected_index,
                results,
                ..
            } => {
                let ids = Self::kit_store_browse_visible_rows(results)
                    .into_iter()
                    .map(|(_, result)| Self::kit_store_browse_row_semantic_id(result))
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "kit_store_browse",
                    &self.list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:kit-search",
                ))
            }
            AppView::InstalledKitsView {
                filter,
                selected_index,
                kits,
            } => {
                let ids = Self::kit_store_installed_visible_rows(kits, filter)
                    .into_iter()
                    .map(|(_, kit)| Self::kit_store_installed_row_semantic_id(kit))
                    .collect();
                Some(self.active_uniform_list_scroll_receipt(
                    "kit_store_installed",
                    &self.list_scroll_handle,
                    ids,
                    *selected_index,
                    "input:installed-kits-filter",
                ))
            }
            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => {
                let ids = crate::browser_history::fuzzy_search_browser_history(
                    &self.cached_browser_history,
                    filter,
                )
                .into_iter()
                .map(|hit| format!("browser-history:{}", hit.entry.history_key()))
                .collect();
                Some(self.active_tracked_list_scroll_receipt(
                    "browser_history",
                    &self.browser_history_scroll_handle,
                    ids,
                    *selected_index,
                    "input:browser-history-filter",
                ))
            }
            AppView::NotesBrowseView { search } => {
                let ids = Self::notes_browse_visible_rows(search)
                    .into_iter()
                    .map(|row| row.semantic_id())
                    .collect();
                Some(self.active_tracked_list_scroll_receipt(
                    "notes_browse",
                    &self.notes_browse_scroll_handle,
                    ids,
                    search.selected_index(),
                    "input:notes-browse-filter",
                ))
            }
            AppView::DictationHistoryView {
                filter,
                selected_index,
            } => {
                let ids = Self::dictation_history_visible_rows(filter)
                    .into_iter()
                    .map(|entry| format!("dictation-history:{}", entry.id))
                    .collect();
                Some(self.active_tracked_list_scroll_receipt(
                    "dictation_history",
                    &self.dictation_history_scroll_handle,
                    ids,
                    *selected_index,
                    "input:dictation-history-filter",
                ))
            }
            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                let ids = Self::agent_chat_history_visible_rows(filter)
                    .into_iter()
                    .map(|entry| format!("agent-chat-history:{}", entry.session_id))
                    .collect();
                Some(self.active_tracked_list_scroll_receipt(
                    "agent_chat_history",
                    &self.agent_chat_history_scroll_handle,
                    ids,
                    *selected_index,
                    "input:agent_chat-history-filter",
                ))
            }
            _ => None,
        }
    }

    pub(crate) fn begin_list_viewport_scroll(
        &mut self,
        source: crate::scrolling::list_interaction::ListViewportInputSource,
        cx: &mut gpui::Context<Self>,
    ) {
        let mut policy = crate::scrolling::list_interaction::ListPointerPolicy {
            hovered_index: self.hovered_index,
            suppress_hover_until_pointer_move: self.list_suppress_hover_until_pointer_move,
        };
        policy.begin_viewport_scroll();
        self.last_scrolled_index = None;
        if self.hovered_index != policy.hovered_index {
            cx.notify();
        }
        self.hovered_index = policy.hovered_index;
        self.list_suppress_hover_until_pointer_move = policy.suppress_hover_until_pointer_move;
        self.last_list_interaction_source = source;
    }

    /// Observe native list wheel/momentum input without consuming it or
    /// translating viewport movement into selection movement.
    pub(crate) fn observe_builtin_native_list_scroll(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        cx: &mut gpui::Context<Self>,
    ) {
        self.begin_list_viewport_scroll(
            crate::scrolling::list_interaction::ListViewportInputSource::from_event(event),
            cx,
        );
    }

    pub(crate) fn note_list_pointer_move(&mut self, row: usize, cx: &mut gpui::Context<Self>) {
        let mut policy = crate::scrolling::list_interaction::ListPointerPolicy {
            hovered_index: self.hovered_index,
            suppress_hover_until_pointer_move: self.list_suppress_hover_until_pointer_move,
        };
        policy.note_pointer_move(row);
        self.input_mode = InputMode::Mouse;
        if self.hovered_index != policy.hovered_index {
            cx.notify();
        }
        self.hovered_index = policy.hovered_index;
        self.list_suppress_hover_until_pointer_move = policy.suppress_hover_until_pointer_move;
    }

    pub(crate) fn note_list_pointer_leave(&mut self, row: usize, cx: &mut gpui::Context<Self>) {
        let mut policy = crate::scrolling::list_interaction::ListPointerPolicy {
            hovered_index: self.hovered_index,
            suppress_hover_until_pointer_move: self.list_suppress_hover_until_pointer_move,
        };
        policy.note_hover_change(row, false);
        if self.hovered_index != policy.hovered_index {
            cx.notify();
        }
        self.hovered_index = policy.hovered_index;
        self.list_suppress_hover_until_pointer_move = policy.suppress_hover_until_pointer_move;
    }

    pub(crate) fn note_list_pointer_click(&mut self, row: usize, cx: &mut gpui::Context<Self>) {
        let mut policy = crate::scrolling::list_interaction::ListPointerPolicy {
            hovered_index: self.hovered_index,
            suppress_hover_until_pointer_move: self.list_suppress_hover_until_pointer_move,
        };
        policy.note_pointer_click(row);
        self.input_mode = InputMode::Mouse;
        self.hovered_index = policy.hovered_index;
        self.list_suppress_hover_until_pointer_move = policy.suppress_hover_until_pointer_move;
        self.last_list_interaction_source =
            crate::scrolling::list_interaction::ListViewportInputSource::Click;
        cx.notify();
    }

    /// Available vibrancy material presets for the theme customizer
    const VIBRANCY_MATERIALS: &[(theme::VibrancyMaterial, &str)] = &[
        (theme::VibrancyMaterial::Hud, "HUD"),
        (theme::VibrancyMaterial::Popover, "Popover"),
        (theme::VibrancyMaterial::Menu, "Menu"),
        (theme::VibrancyMaterial::Sidebar, "Sidebar"),
        (theme::VibrancyMaterial::Content, "Content"),
    ];

    /// Available font size presets for the theme customizer
    const FONT_SIZE_PRESETS: &[(f32, &str)] = &[
        (12.0, "12"),
        (13.0, "13"),
        (14.0, "14"),
        (15.0, "15"),
        (16.0, "16"),
        (18.0, "18"),
        (20.0, "20"),
    ];

    /// Find the index of a vibrancy material in the presets array
    fn find_vibrancy_material_index(material: theme::VibrancyMaterial) -> usize {
        Self::VIBRANCY_MATERIALS
            .iter()
            .position(|(m, _)| *m == material)
            .unwrap_or(0)
    }

    /// Return a human-readable name for a hex accent color
    fn accent_color_name(color: u32) -> &'static str {
        theme::accent_color_name(color)
    }

    pub(crate) fn theme_font_family(&self) -> String {
        crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design)
            .primary_font()
            .to_string()
    }

    pub(crate) fn theme_font_size_xl(&self) -> f32 {
        crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design)
            .font_size_xl()
    }

    pub(crate) fn render_search_input(&self) -> gpui_component::input::Input {
        let search = self.current_main_menu_theme.def().search;
        let input_font_size = search.font_size;
        gpui_component::input::Input::new(&self.gpui_input_state)
            .w_full()
            .h(gpui::px(search.height))
            .line_height(gpui::px(search.height))
            .font_family(self.theme_font_family())
            .font_weight(search.font_weight)
            .px(gpui::px(0.))
            .py(gpui::px(0.))
            .with_size(gpui_component::Size::Size(gpui::px(input_font_size)))
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
    }

    pub(crate) fn render_search_input_with_ghost(&self, _cx: &gpui::Context<Self>) -> gpui::Div {
        gpui::div().w_full().child(self.render_search_input())
    }

    pub(crate) fn render_builtin_main_input_count_label(
        &self,
        label: impl Into<gpui::SharedString>,
    ) -> gpui::AnyElement {
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        // Shared with the design-token exporter
        // (builtin_main_input_contract.rs): text_sm-sized, gpui default line
        // height and NORMAL weight (never the search body's 430), right
        // inset = search text_inset_x, color = chrome text hint.
        let style = resolved_builtin_main_input_count_label_style(
            self.current_main_menu_theme.def(),
            &chrome,
        );
        gpui::div()
            .flex_none()
            .whitespace_nowrap()
            .pr(gpui::px(style.inset_right))
            .text_size(gpui::px(style.font_size_px))
            .line_height(gpui::px(style.line_height_px))
            .font_weight(style.font_weight)
            .text_color(gpui::rgba(style.text_rgba))
            .child(label.into())
            .into_any_element()
    }

    pub(crate) fn render_builtin_main_input_shell(
        &self,
        trailing: Vec<gpui::AnyElement>,
    ) -> gpui::AnyElement {
        let menu_def = self.current_main_menu_theme.def();
        crate::components::main_view_chrome::render_main_view_input_shell(
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewInputChrome {
                body: self.render_search_input().into_any_element(),
                trailing,
            },
        )
    }

    pub(crate) fn render_builtin_main_input_header(
        &self,
        trailing: Vec<gpui::AnyElement>,
        cx: &mut gpui::Context<Self>,
    ) -> crate::components::main_view_chrome::MainViewHeaderChrome {
        let menu_def = self.current_main_menu_theme.def();
        crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
            self.render_builtin_main_input_shell(trailing),
        )
    }

    pub(crate) fn render_builtin_main_input_surface(
        &self,
        key_context: &'static str,
        trailing: Vec<gpui::AnyElement>,
        main: gpui::AnyElement,
        footer: Option<gpui::AnyElement>,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(gpui::rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context(key_context)
                .track_focus(&self.focus_handle),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(trailing, cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }

    pub(crate) fn render_generic_filterable_search_surface(
        &self,
        key_context: &'static str,
        count_label: String,
        list_element: gpui::AnyElement,
        footer: Option<gpui::AnyElement>,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let content = gpui::div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(gpui::px(0.))
            .w_full()
            .overflow_hidden()
            .child(list_element);

        self.render_builtin_main_input_surface(
            key_context,
            vec![self.render_builtin_main_input_count_label(count_label)],
            content.into_any_element(),
            footer,
            cx,
        )
    }

    pub(crate) fn render_builtin_split_main_content(
        &self,
        list_pane: gpui::AnyElement,
        preview_pane: gpui::AnyElement,
    ) -> gpui::AnyElement {
        render_builtin_split_main_content_layout(list_pane, preview_pane)
    }

    /// Emit a structured scroll log line for builtin views.
    #[allow(clippy::too_many_arguments)]
    fn log_builtin_scroll_event(
        view: &'static str,
        action: &'static str,
        reason: &'static str,
        item_count: usize,
        selected_index: Option<usize>,
        target_item: Option<usize>,
        filter: Option<&str>,
        input_mode: &'static str,
    ) {
        tracing::debug!(
            target: "script_kit::scroll",
            view = view,
            action = action,
            reason = reason,
            item_count = item_count,
            selected_index = selected_index
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".into())
                .as_str(),
            target_item = target_item
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".into())
                .as_str(),
            filter_len = filter
                .map(|v| v.chars().count().to_string())
                .unwrap_or_else(|| "none".into())
                .as_str(),
            input_mode = input_mode,
        );
    }

    /// Scroll a builtin uniform list to the top and emit a structured log.
    fn scroll_builtin_to_top_with_log(
        handle: &UniformListScrollHandle,
        view: &'static str,
        item_count: usize,
        filter: &str,
        input_mode: &'static str,
    ) {
        Self::log_builtin_scroll_event(
            view,
            "scroll_to_item",
            "filter_changed",
            item_count,
            Some(0),
            Some(0),
            Some(filter),
            input_mode,
        );
        handle.scroll_to_item(0, ScrollStrategy::Top);
    }

    /// Compute scrollbar metrics for a tracked uniform list.
    fn builtin_uniform_list_scrollbar_metrics(
        handle: &UniformListScrollHandle,
        total_items: usize,
        fallback_visible_items: usize,
    ) -> Option<(usize, usize, Option<f32>)> {
        if total_items == 0 {
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE metrics.empty",
                total_items,
                fallback_visible_items,
                "SCROLL_TRACE metrics.empty"
            );
            return None;
        }

        let state = handle.0.borrow();
        let live_scroll_top = state.base_handle.logical_scroll_top().0;
        let deferred_item_index = state
            .deferred_scroll_to_item
            .map(|deferred| deferred.item_index);
        let has_item_size = state.last_item_size.is_some();
        let scroll_offset = crate::components::scrollbar::preferred_scroll_offset(
            live_scroll_top,
            deferred_item_index,
            has_item_size,
            total_items,
        );

        let fallback_visible_items = fallback_visible_items.max(1).min(total_items);

        if let Some(item_size) = state.last_item_size {
            let viewport_height = item_size.item.height.as_f32().max(0.0);
            let content_height = item_size.contents.height.as_f32().max(0.0);
            let visible_items = if content_height > 0.0 {
                ((viewport_height / content_height) * total_items as f32)
                    .ceil()
                    .max(1.0) as usize
            } else {
                fallback_visible_items
            };
            let clamped_visible_items = visible_items.clamp(1, total_items);
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE metrics.measured",
                total_items,
                fallback_visible_items,
                live_scroll_top,
                deferred_item_index = ?deferred_item_index,
                has_item_size,
                scroll_offset,
                viewport_height,
                content_height,
                visible_items = clamped_visible_items,
                "SCROLL_TRACE metrics.measured"
            );

            Some((scroll_offset, clamped_visible_items, Some(viewport_height)))
        } else {
            tracing::info!(
                target: "script_kit::scroll_trace",
                event = "SCROLL_TRACE metrics.fallback",
                total_items,
                fallback_visible_items,
                live_scroll_top,
                deferred_item_index = ?deferred_item_index,
                has_item_size,
                scroll_offset,
                visible_items = fallback_visible_items,
                "SCROLL_TRACE metrics.fallback"
            );
            Some((scroll_offset, fallback_visible_items, None))
        }
    }

    /// Build a vendor scrollbar bound to the tracked uniform-list handle.
    fn builtin_uniform_list_scrollbar(
        &self,
        handle: &UniformListScrollHandle,
        total_items: usize,
        fallback_visible_items: usize,
    ) -> AnyElement {
        if Self::builtin_uniform_list_scrollbar_metrics(handle, total_items, fallback_visible_items)
            .is_none()
        {
            return div().into_any_element();
        }
        gpui_component::scroll::Scrollbar::vertical(handle)
            .scrollbar_show(gpui_component::scroll::ScrollbarShow::Always)
            .into_any_element()
    }
}

#[cfg(test)]
mod builtin_scrollbar_contract {
    const SOURCE: &str = include_str!("common.rs");

    #[test]
    fn builtin_uniform_list_scrollbar_uses_vendor_handle_path() {
        assert!(
            SOURCE.contains("gpui_component::scroll::Scrollbar::vertical(handle)"),
            "builtin uniform list scrollbars should be the GPUI vendor scrollbar bound to the real handle"
        );
        assert!(
            SOURCE.contains(".scrollbar_show(gpui_component::scroll::ScrollbarShow::Always)"),
            "builtin uniform list scrollbars should stay visible for launcher-family surfaces"
        );
    }
}

/// Corpus-wide consistency audit for every built-in browser renderer.
///
/// WHY (decision lock, 2026-07-11): the Tips browser shipped with two
/// consistency regressions the shared component contract exists to prevent —
/// a selectable list that never scrolled its keyboard selection into view,
/// and a footer that bypassed the persistent main-window footer. These are
/// architectural invariants of the builtin-browser family, not per-surface
/// styling choices, and no higher enforcement rung can currently express
/// them (renderers are `AnyElement` builders with no inspectable tree in
/// unit tests). The audit asserts the ABSENCE of the dangerous pattern per
/// file and enumerates the grandfathered offenders explicitly; both lists
/// are shrink-only.
#[cfg(test)]
mod builtin_browser_consistency_audit {
    /// Every builtin browser renderer, enumerated explicitly so a new file
    /// cannot join the corpus unaudited (`include!` chain in `mod.rs` and
    /// this table must move together).
    const BUILTIN_BROWSER_SOURCES: &[(&str, &str)] = &[
        ("actions.rs", include_str!("actions.rs")),
        (
            "agent_chat_history.rs",
            include_str!("agent_chat_history.rs"),
        ),
        ("ai_presets.rs", include_str!("ai_presets.rs")),
        ("app_launcher.rs", include_str!("app_launcher.rs")),
        ("browser_history.rs", include_str!("browser_history.rs")),
        ("browser_tabs.rs", include_str!("browser_tabs.rs")),
        ("clipboard.rs", include_str!("clipboard.rs")),
        ("clipboard_preview.rs", include_str!("clipboard_preview.rs")),
        (
            "current_app_commands.rs",
            include_str!("current_app_commands.rs"),
        ),
        ("design_picker.rs", include_str!("design_picker.rs")),
        ("dictation_history.rs", include_str!("dictation_history.rs")),
        ("emoji_picker.rs", include_str!("emoji_picker.rs")),
        ("favorites.rs", include_str!("favorites.rs")),
        ("file_search.rs", include_str!("file_search.rs")),
        ("flow_ux.rs", include_str!("flow_ux.rs")),
        ("kit_store.rs", include_str!("kit_store.rs")),
        ("migrate_v1.rs", include_str!("migrate_v1.rs")),
        ("notes_browse.rs", include_str!("notes_browse.rs")),
        (
            "permissions_wizard.rs",
            include_str!("permissions_wizard.rs"),
        ),
        ("process_manager.rs", include_str!("process_manager.rs")),
        ("profile_search.rs", include_str!("profile_search.rs")),
        ("script_templates.rs", include_str!("script_templates.rs")),
        ("sdk_reference.rs", include_str!("sdk_reference.rs")),
        ("settings.rs", include_str!("settings.rs")),
        ("theme_chooser.rs", include_str!("theme_chooser.rs")),
        ("tips.rs", include_str!("tips.rs")),
        ("window_actions.rs", include_str!("window_actions.rs")),
        ("window_switcher.rs", include_str!("window_switcher.rs")),
    ];

    /// Shrink-only. These files render selectable rows but still never move
    /// their scroll container when the selection moves (the Tips bug class).
    /// Fixing one means DELETING it here — never add a new entry; new
    /// browsers must scroll their selection into view from day one via a
    /// tracked `uniform_list` + `scroll_to_item` (see `window_switcher.rs`)
    /// or a `ListState`/`ScrollHandle` navigation scroll.
    const GRANDFATHERED_NON_SCROLLING_SELECTABLE_LISTS: &[&str] = &[
        "ai_presets.rs",
        "favorites.rs",
        "permissions_wizard.rs",
        "script_templates.rs",
        "sdk_reference.rs",
    ];

    fn renders_selectable_list(source: &str) -> bool {
        source.contains("ListItem::new") && source.contains(".selected(")
    }

    fn scrolls_selection(source: &str) -> bool {
        source.contains("scroll_to_item")
            || source.contains(".track_scroll(")
            || source.contains("_list_state")
    }

    #[test]
    fn selectable_builtin_lists_scroll_selection_into_view() {
        for (name, source) in BUILTIN_BROWSER_SOURCES {
            if !renders_selectable_list(source) {
                continue;
            }
            let grandfathered = GRANDFATHERED_NON_SCROLLING_SELECTABLE_LISTS.contains(name);
            let scrolls = scrolls_selection(source);
            if grandfathered {
                assert!(
                    !scrolls,
                    "{name} now scrolls its selection — delete it from \
                     GRANDFATHERED_NON_SCROLLING_SELECTABLE_LISTS (shrink-only)"
                );
                continue;
            }
            assert!(
                scrolls,
                "{name} renders a selectable list but never scrolls the selection into \
                 view. Use a tracked uniform_list + scroll_to_item on every selection \
                 move (keyboard, wheel, click) — see window_switcher.rs / tips.rs — \
                 instead of a free-scrolling div. Do NOT add this file to the \
                 grandfather list; it is shrink-only."
            );
        }
    }

    #[test]
    fn builtin_footers_route_through_the_persistent_main_window_footer() {
        for (name, source) in BUILTIN_BROWSER_SOURCES {
            if source.contains("render_simple_hint_strip(") {
                assert!(
                    source.contains("main_window_footer_slot("),
                    "{name} renders a GPUI hint strip without offering it to \
                     main_window_footer_slot. Builtin browsers must reuse the \
                     persistent native footer (native_footer_surface + \
                     FooterButtonConfig) and pass the hint strip only as its \
                     GPUI fallback — never render standalone footer chrome."
                );
            }
            // No builtin browser may instantiate footer chrome directly;
            // footer chrome belongs to the persistent main-window footer.
            assert!(
                !source.contains("PromptFooter::new(") && !source.contains("HintStrip::new("),
                "{name} builds footer chrome directly. Route through \
                 main_window_footer_slot(render_simple_hint_strip(...)) so the \
                 surface inherits the shared footer components and native footer."
            );
        }
    }
}

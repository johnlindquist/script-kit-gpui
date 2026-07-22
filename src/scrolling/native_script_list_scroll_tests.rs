use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    div, list, point, prelude::*, px, size, AppContext as _, Context, Entity, InteractiveElement,
    IntoElement, ListAlignment, ListOffset, ListSizingBehavior, ListState, Render, ScrollDelta,
    ScrollPhase, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, Window,
};

const VIEWPORT_WIDTH: f32 = 320.0;
const VIEWPORT_HEIGHT: f32 = 181.0;
const ITEM_COUNT: usize = 220;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowKind {
    Header,
    Status,
    Item,
}

#[derive(Clone, Copy, Debug)]
enum ScrollTransport {
    DirectPixels,
    MomentumPixels,
    Lines,
}

#[derive(Clone, Copy, Debug)]
struct FixtureRow {
    kind: RowKind,
    height: f32,
}

#[derive(Clone, Copy, Debug)]
struct ObserverSample {
    offset: ListOffset,
    phase: ScrollPhase,
    momentum_phase: ScrollPhase,
    touch_phase: TouchPhase,
    timestamp_seconds: Option<f64>,
}

struct NativeScriptListScrollHarness {
    state: ListState,
    rows: Rc<Vec<FixtureRow>>,
    selected_index: Rc<Cell<usize>>,
    observer_samples: Rc<RefCell<Vec<ObserverSample>>>,
}

impl Render for NativeScriptListScrollHarness {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows.clone();
        let selected_index = self.selected_index.clone();
        let native_list = list(self.state.clone(), move |ix, _window, _cx| {
            let row = rows[ix];
            let selected = selected_index.get() == ix;
            div()
                .relative()
                .h(px(row.height))
                .w_full()
                .when(selected, |row| {
                    row.child(div().absolute().inset_0().id("external-selection-marker"))
                })
                .into_any()
        })
        .with_sizing_behavior(ListSizingBehavior::Infer)
        .size_full();

        div()
            .relative()
            .size_full()
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, _cx| {
                this.observer_samples.borrow_mut().push(ObserverSample {
                    offset: this.state.logical_scroll_top(),
                    phase: event.phase,
                    momentum_phase: event.momentum_phase,
                    touch_phase: event.touch_phase,
                    timestamp_seconds: event.timestamp_seconds,
                });
            }))
            .child(native_list)
    }
}

struct HarnessFixture {
    entity: Entity<NativeScriptListScrollHarness>,
    state: ListState,
    rows: Rc<Vec<FixtureRow>>,
    selected_index: Rc<Cell<usize>>,
    observer_samples: Rc<RefCell<Vec<ObserverSample>>>,
}

fn rows_with_first_header(hidden: bool) -> Vec<FixtureRow> {
    let mut rows = Vec::with_capacity(ITEM_COUNT + 16);
    rows.push(FixtureRow {
        kind: RowKind::Header,
        height: if hidden { 0.0 } else { 37.0 },
    });

    for item_ix in 0..ITEM_COUNT {
        if item_ix > 0 && item_ix % 47 == 0 {
            rows.push(FixtureRow {
                kind: RowKind::Header,
                height: 27.0 + (item_ix % 3) as f32,
            });
        }
        if item_ix % 31 == 0 {
            rows.push(FixtureRow {
                kind: RowKind::Status,
                height: 41.0 + (item_ix % 4) as f32,
            });
        }
        rows.push(FixtureRow {
            kind: RowKind::Item,
            height: 32.0 + ((item_ix * 7) % 17) as f32,
        });
    }
    rows
}

fn build_harness(cx: &mut TestAppContext, hidden_first_header: bool) -> HarnessFixture {
    let rows = Rc::new(rows_with_first_header(hidden_first_header));
    assert!(rows.iter().filter(|row| row.kind == RowKind::Item).count() >= ITEM_COUNT);
    assert!(rows.iter().skip(1).any(|row| row.kind == RowKind::Header));
    assert!(rows.iter().skip(1).any(|row| row.kind == RowKind::Status));

    let state = ListState::new(rows.len(), ListAlignment::Top, px(96.0));
    let selected_index = Rc::new(Cell::new(19));
    let observer_samples = Rc::new(RefCell::new(Vec::new()));
    let entity = cx.new(|_| NativeScriptListScrollHarness {
        state: state.clone(),
        rows: rows.clone(),
        selected_index: selected_index.clone(),
        observer_samples: observer_samples.clone(),
    });

    HarnessFixture {
        entity,
        state,
        rows,
        selected_index,
        observer_samples,
    }
}

fn draw(vcx: &mut VisualTestContext, entity: &Entity<NativeScriptListScrollHarness>) {
    let entity = entity.clone();
    vcx.draw(
        point(px(0.0), px(0.0)),
        size(px(VIEWPORT_WIDTH), px(VIEWPORT_HEIGHT)),
        move |_window, _cx| entity.into_any_element(),
    );
}

fn dispatch_and_redraw(
    vcx: &mut VisualTestContext,
    entity: &Entity<NativeScriptListScrollHarness>,
    event: ScrollWheelEvent,
) {
    vcx.simulate_event(event);
    draw(vcx, entity);
}

fn pixel_event(
    delta_y: f32,
    touch_phase: TouchPhase,
    phase: ScrollPhase,
    momentum_phase: ScrollPhase,
    timestamp_seconds: f64,
) -> ScrollWheelEvent {
    ScrollWheelEvent {
        position: point(px(40.0), px(40.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(delta_y))),
        touch_phase,
        phase,
        momentum_phase,
        timestamp_seconds: Some(timestamp_seconds),
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

fn absolute_scroll_top(rows: &[FixtureRow], offset: ListOffset) -> f32 {
    rows.iter()
        .take(offset.item_ix)
        .map(|row| row.height)
        .sum::<f32>()
        + offset.offset_in_item.as_f32()
}

fn assert_near(actual: f32, expected: f32, context: &str) {
    assert!(
        (actual - expected).abs() <= 0.01,
        "{context}: expected {expected}, got {actual}"
    );
}

fn assert_offset_eq(actual: ListOffset, expected: ListOffset, context: &str) {
    assert_eq!(actual.item_ix, expected.item_ix, "{context}: item index");
    assert_near(
        actual.offset_in_item.as_f32(),
        expected.offset_in_item.as_f32(),
        context,
    );
}

fn transport_event(
    transport: ScrollTransport,
    toward_bottom: bool,
    timestamp_seconds: f64,
) -> ScrollWheelEvent {
    let sign = if toward_bottom { -1.0 } else { 1.0 };
    match transport {
        ScrollTransport::DirectPixels => pixel_event(
            sign * 320.0,
            TouchPhase::Moved,
            ScrollPhase::Changed,
            ScrollPhase::None,
            timestamp_seconds,
        ),
        ScrollTransport::MomentumPixels => pixel_event(
            sign * 320.0,
            TouchPhase::Moved,
            ScrollPhase::None,
            ScrollPhase::Changed,
            timestamp_seconds,
        ),
        ScrollTransport::Lines => line_event(sign * 16.0),
    }
}

fn progress_to_endpoint(
    vcx: &mut VisualTestContext,
    fixture: &HarnessFixture,
    transport: ScrollTransport,
    toward_bottom: bool,
) {
    let content_height = fixture.rows.iter().map(|row| row.height).sum::<f32>();
    let expected = if toward_bottom {
        content_height - VIEWPORT_HEIGHT
    } else {
        0.0
    };
    let mut stationary_events = 0;

    for step in 0..fixture.rows.len() * 4 {
        let before = absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top());
        if (before - expected).abs() <= 0.01 {
            return;
        }
        dispatch_and_redraw(
            vcx,
            &fixture.entity,
            transport_event(transport, toward_bottom, step as f64),
        );
        let after = absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top());
        stationary_events = if (after - before).abs() <= 0.01 {
            stationary_events + 1
        } else {
            0
        };
        assert!(
            stationary_events < 4 || (after - expected).abs() <= 0.01,
            "{transport:?} stalled below the exact endpoint: expected {expected}, got {after}"
        );
    }

    let actual = absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top());
    panic!("{transport:?} did not reach the exact endpoint: expected {expected}, got {actual}");
}

#[gpui::test]
fn native_script_list_scroll_observer_sees_post_native_fractional_and_line_offsets(
    cx: &mut TestAppContext,
) {
    let fixture = build_harness(cx, false);
    let mut vcx = cx.add_empty_window();
    draw(&mut vcx, &fixture.entity);
    let selected_before = fixture.selected_index.get();

    dispatch_and_redraw(
        &mut vcx,
        &fixture.entity,
        pixel_event(
            -7.5,
            TouchPhase::Started,
            ScrollPhase::Began,
            ScrollPhase::None,
            10.25,
        ),
    );
    let post_pixel = fixture.state.logical_scroll_top();
    assert_eq!(post_pixel.item_ix, 0);
    assert_near(
        post_pixel.offset_in_item.as_f32(),
        7.5,
        "fractional pixel wheel",
    );

    let samples = fixture.observer_samples.borrow();
    assert_eq!(samples.len(), 1, "the real ancestor observer must run once");
    assert_offset_eq(
        samples[0].offset,
        post_pixel,
        "native List listener must run before the ancestor observer",
    );
    assert_eq!(samples[0].phase, ScrollPhase::Began);
    assert!(matches!(samples[0].touch_phase, TouchPhase::Started));
    assert_eq!(samples[0].timestamp_seconds, Some(10.25));
    drop(samples);

    dispatch_and_redraw(
        &mut vcx,
        &fixture.entity,
        pixel_event(
            -2.25,
            TouchPhase::Moved,
            ScrollPhase::Changed,
            ScrollPhase::None,
            10.5,
        ),
    );
    assert_near(
        absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top()),
        9.75,
        "sub-row deltas accumulate as pixels",
    );

    fixture.state.scroll_to(ListOffset {
        item_ix: 0,
        offset_in_item: px(0.0),
    });
    draw(&mut vcx, &fixture.entity);
    dispatch_and_redraw(&mut vcx, &fixture.entity, line_event(-1.0));
    assert_near(
        absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top()),
        20.0,
        "GPUI line wheel uses its fixed 20px conversion",
    );
    assert_eq!(fixture.selected_index.get(), selected_before);
}

#[gpui::test]
fn native_script_list_scroll_direct_and_momentum_phases_remain_viewport_only(
    cx: &mut TestAppContext,
) {
    let fixture = build_harness(cx, false);
    let mut vcx = cx.add_empty_window();
    draw(&mut vcx, &fixture.entity);
    let selected_before = fixture.selected_index.get();

    let events = [
        pixel_event(
            -5.0,
            TouchPhase::Started,
            ScrollPhase::Began,
            ScrollPhase::None,
            20.0,
        ),
        pixel_event(
            -6.0,
            TouchPhase::Moved,
            ScrollPhase::Changed,
            ScrollPhase::None,
            20.1,
        ),
        pixel_event(
            0.0,
            TouchPhase::Ended,
            ScrollPhase::Ended,
            ScrollPhase::None,
            20.2,
        ),
        pixel_event(
            -3.25,
            TouchPhase::Moved,
            ScrollPhase::None,
            ScrollPhase::Began,
            20.3,
        ),
        pixel_event(
            -4.75,
            TouchPhase::Moved,
            ScrollPhase::None,
            ScrollPhase::Changed,
            20.4,
        ),
        pixel_event(
            0.0,
            TouchPhase::Ended,
            ScrollPhase::None,
            ScrollPhase::Ended,
            20.5,
        ),
    ];
    for event in events {
        dispatch_and_redraw(&mut vcx, &fixture.entity, event);
    }

    assert_near(
        absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top()),
        19.0,
        "direct and momentum deltas",
    );
    assert_eq!(fixture.selected_index.get(), selected_before);

    let samples = fixture.observer_samples.borrow();
    assert_eq!(samples.len(), 6);
    assert_eq!(samples[0].phase, ScrollPhase::Began);
    assert_eq!(samples[2].phase, ScrollPhase::Ended);
    assert_eq!(samples[3].momentum_phase, ScrollPhase::Began);
    assert_eq!(samples[4].momentum_phase, ScrollPhase::Changed);
    assert_eq!(samples[5].momentum_phase, ScrollPhase::Ended);
}

#[gpui::test]
fn native_script_list_scroll_reaches_both_exact_endpoints_without_measure_all(
    cx: &mut TestAppContext,
) {
    for hidden_first_header in [false, true] {
        for transport in [
            ScrollTransport::DirectPixels,
            ScrollTransport::MomentumPixels,
            ScrollTransport::Lines,
        ] {
            let fixture = build_harness(cx, hidden_first_header);
            let mut vcx = cx.add_empty_window();
            draw(&mut vcx, &fixture.entity);
            let selected_before = fixture.selected_index.get();
            let content_height = fixture.rows.iter().map(|row| row.height).sum::<f32>();
            let expected_max = content_height - VIEWPORT_HEIGHT;
            let initial_measured_max = fixture.state.max_offset_for_scrollbar().y.as_f32();
            assert!(
                initial_measured_max < expected_max * 0.25,
                "fixture was accidentally fully measured before {transport:?}: measured={initial_measured_max}, exact={expected_max}"
            );

            progress_to_endpoint(&mut vcx, &fixture, transport, true);
            assert_near(
                absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top()),
                expected_max,
                "exact bottom endpoint",
            );
            let last_ix = fixture.rows.len() - 1;
            let last_bounds = fixture
                .state
                .bounds_for_item(last_ix)
                .expect("the final variable-height row must be measured and mounted at the bottom");
            assert_near(
                last_bounds.bottom().as_f32(),
                fixture.state.viewport_bounds().bottom().as_f32(),
                "last row meets the viewport bottom",
            );

            progress_to_endpoint(&mut vcx, &fixture, transport, false);
            let top = fixture.state.logical_scroll_top();
            assert_near(
                absolute_scroll_top(&fixture.rows, top),
                0.0,
                "native wheel must return to the exact top endpoint",
            );
            assert_eq!(
                top.item_ix,
                usize::from(hidden_first_header),
                "a zero-height leading header canonicalizes exact top to the next row"
            );
            assert_eq!(top.offset_in_item, px(0.0));
            assert_eq!(fixture.selected_index.get(), selected_before);
        }
    }
}

#[gpui::test]
fn native_script_list_scrollbar_changes_only_the_fully_progressed_viewport(
    cx: &mut TestAppContext,
) {
    let fixture = build_harness(cx, true);
    let mut vcx = cx.add_empty_window();
    draw(&mut vcx, &fixture.entity);
    progress_to_endpoint(&mut vcx, &fixture, ScrollTransport::DirectPixels, true);
    let selected_before = fixture.selected_index.get();
    let observer_count_before = fixture.observer_samples.borrow().len();

    fixture.state.scrollbar_drag_started();
    fixture
        .state
        .set_offset_from_scrollbar(point(px(0.0), px(-137.5)));
    fixture.state.scrollbar_drag_ended();
    draw(&mut vcx, &fixture.entity);

    assert_near(
        absolute_scroll_top(&fixture.rows, fixture.state.logical_scroll_top()),
        137.5,
        "scrollbar viewport offset",
    );
    assert_eq!(fixture.selected_index.get(), selected_before);
    assert_eq!(
        fixture.observer_samples.borrow().len(),
        observer_count_before,
        "the direct scrollbar API must not synthesize a wheel observer event"
    );
}

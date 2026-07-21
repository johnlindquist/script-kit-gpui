use std::{
    ops::Range,
    rc::Rc,
    sync::{Arc, Mutex},
};

use gpui::{
    App, BorderStyle, Bounds, CursorStyle, Edges, Element, ElementId, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, StyledText, TextLayout, Window,
    point, px, quad,
};

use crate::{
    ActiveTheme, global_state::GlobalState, input::Selection, text::node::LinkMark,
    text::selection, text::state::HitRun,
};

/// A inline element used to render a inline text and support selectable.
///
/// All text in TextView (including the CodeBlock) used this for text rendering.
pub(super) struct Inline {
    id: ElementId,
    text: SharedString,
    links: Rc<Vec<(Range<usize>, LinkMark)>>,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    styled_text: StyledText,

    state: Arc<Mutex<InlineState>>,
}

/// The inline text state, used RefCell to keep the selection state.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct InlineState {
    hovered_link_range: Option<Range<usize>>,
    /// The text that actually rendering, matched with selection.
    pub(super) text: SharedString,
    /// The run-local slice of the logical selection, refreshed at paint —
    /// used only for painting the highlight, never for copy.
    pub(super) selection: Option<Selection>,
    /// This run's byte range within the flattened selection document,
    /// stamped by the document builder.
    pub(super) document_range: Option<Range<usize>>,
}

impl InlineState {
    /// Save actually rendered text for selected text to use.
    pub(crate) fn set_text(&mut self, text: SharedString) {
        self.text = text;
    }
}

impl Inline {
    pub(super) fn new(
        id: impl Into<ElementId>,
        state: Arc<Mutex<InlineState>>,
        links: Vec<(Range<usize>, LinkMark)>,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
    ) -> Self {
        let text = state.lock().unwrap().text.clone();
        Self {
            id: id.into(),
            links: Rc::new(links),
            highlights,
            text: text.clone(),
            styled_text: StyledText::new(text),
            state,
        }
    }

    /// Get link at given mouse position.
    fn link_for_position(
        layout: &TextLayout,
        links: &Vec<(Range<usize>, LinkMark)>,
        position: Point<Pixels>,
    ) -> Option<LinkMark> {
        let offset = layout.index_for_position(position).ok()?;
        for (range, link) in links.iter() {
            if range.contains(&offset) {
                return Some(link.clone());
            }
        }

        None
    }

    fn link_range_for_index(
        links: &[(Range<usize>, LinkMark)],
        index: Option<usize>,
    ) -> Option<Range<usize>> {
        let index = index?;
        links
            .iter()
            .find_map(|(range, _)| range.contains(&index).then(|| range.clone()))
    }

    /// Paint selected bounds for debug.
    #[allow(unused)]
    fn paint_selected_bounds(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        window.paint_quad(gpui::PaintQuad {
            bounds,
            background: cx.theme().blue.alpha(0.01).into(),
            corner_radii: gpui::Corners::default(),
            border_color: gpui::transparent_black(),
            border_style: BorderStyle::default(),
            border_widths: gpui::Edges::all(px(0.)),
        });
    }

    /// Register this run's layout for mouse hit-testing and intersect the
    /// TextView's logical selection with the run's document range. The
    /// selection itself lives in `TextViewState` as byte positions; this
    /// is purely the per-run projection used for painting.
    fn layout_selections(
        document_range: &Option<Range<usize>>,
        text_layout: &TextLayout,
        bounds: Bounds<Pixels>,
        cx: &mut App,
    ) -> (bool, bool, Option<Selection>) {
        let Some(text_view_state) = GlobalState::global(cx).text_view_state() else {
            return (false, false, None);
        };
        let text_view_state = text_view_state.clone();

        if let Some(range) = document_range.clone() {
            text_view_state.update(cx, |state, _| {
                state.register_hit_run(HitRun {
                    range,
                    layout: text_layout.clone(),
                    bounds,
                });
            });
        }

        let state = text_view_state.read(cx);
        let is_selectable = state.is_selectable();
        let Some(effective) = state.effective_selection_range() else {
            return (is_selectable, false, None);
        };
        if effective.is_empty() {
            return (is_selectable, false, None);
        }
        let selection = document_range
            .as_ref()
            .and_then(|run| selection::run_local_selection(&effective, run))
            .map(Into::into);
        (is_selectable, true, selection)
    }

    /// Paint the selection background.
    fn paint_selection(
        selection: &Selection,
        text_layout: &TextLayout,
        bounds: &Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut start = selection.start;
        let mut end = selection.end;
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }
        let Some(start_position) = text_layout.position_for_index(start) else {
            return;
        };
        let Some(end_position) = text_layout.position_for_index(end) else {
            return;
        };

        let line_height = text_layout.line_height();
        if start_position.y == end_position.y {
            window.paint_quad(quad(
                Bounds::from_corners(
                    start_position,
                    point(end_position.x, end_position.y + line_height),
                ),
                px(0.),
                cx.theme().selection,
                Edges::default(),
                gpui::transparent_black(),
                BorderStyle::default(),
            ));
        } else {
            window.paint_quad(quad(
                Bounds::from_corners(
                    start_position,
                    point(bounds.right(), start_position.y + line_height),
                ),
                px(0.),
                cx.theme().selection,
                Edges::default(),
                gpui::transparent_black(),
                BorderStyle::default(),
            ));

            if end_position.y > start_position.y + line_height {
                window.paint_quad(quad(
                    Bounds::from_corners(
                        point(bounds.left(), start_position.y + line_height),
                        point(bounds.right(), end_position.y),
                    ),
                    px(0.),
                    cx.theme().selection,
                    Edges::default(),
                    gpui::transparent_black(),
                    BorderStyle::default(),
                ));
            }

            window.paint_quad(quad(
                Bounds::from_corners(
                    point(bounds.left(), end_position.y),
                    point(end_position.x, end_position.y + line_height),
                ),
                px(0.),
                cx.theme().selection,
                Edges::default(),
                gpui::transparent_black(),
                BorderStyle::default(),
            ));
        }
    }
}

impl IntoElement for Inline {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Inline {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_element_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text_style = window.text_style();
        let hovered_link_range = self.state.lock().unwrap().hovered_link_range.clone();

        let mut runs = Vec::new();
        let mut ix = 0;
        for (range, highlight) in self.highlights.iter().map(|(range, highlight)| {
            let mut highlight = *highlight;
            if hovered_link_range
                .as_ref()
                .is_some_and(|hovered| hovered.start < range.end && range.start < hovered.end)
            {
                // Script Kit local: preserve link underline/font styling while making hover
                // use the semantic theme token; keep this during upstream syncs.
                highlight.color = Some(cx.theme().link_hover);
            }
            (range, highlight)
        }) {
            if ix < range.start {
                runs.push(text_style.clone().to_run(range.start - ix));
            }
            runs.push(text_style.clone().highlight(highlight).to_run(range.len()));
            ix = range.end;
        }
        if ix < self.text.len() {
            runs.push(text_style.to_run(self.text.len() - ix));
        }

        self.styled_text = StyledText::new(self.text.clone()).with_runs(runs);
        let (layout_id, _) =
            self.styled_text
                .request_layout(global_element_id, inspector_id, window, cx);

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text
            .prepaint(id, inspector_id, bounds, &mut (), window, cx);

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        hitbox
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let current_view = window.current_view();
        let hitbox = prepaint;
        let mut state = self.state.lock().unwrap();
        let document_range = state.document_range.clone();

        let text_layout = self.styled_text.layout().clone();
        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        // Project the logical selection onto this run (and register the
        // run's layout for mouse hit-testing).
        let (is_selectable, is_selection, selection) =
            Self::layout_selections(&document_range, &text_layout, bounds, cx);

        state.selection = selection;

        if is_selection || is_selectable {
            window.set_cursor_style(CursorStyle::IBeam, &hitbox);
        }

        // link cursor pointer
        let mouse_position = window.mouse_position();
        if let Some(_) = Self::link_for_position(&text_layout, &self.links, mouse_position) {
            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
        }

        if let Some(selection) = &state.selection {
            Self::paint_selection(selection, &text_layout, &bounds, window, cx);
        }

        // mouse move, update hovered link
        window.on_mouse_event({
            let hitbox = hitbox.clone();
            let links = self.links.clone();
            let state = self.state.clone();
            let text_layout = text_layout.clone();
            move |event: &MouseMoveEvent, phase, window, cx| {
                if !phase.bubble() {
                    return;
                }

                let hovered_index = hitbox
                    .is_hovered(window)
                    .then(|| text_layout.index_for_position(event.position).ok())
                    .flatten();
                let updated = Self::link_range_for_index(&links, hovered_index);
                let mut state = state.lock().unwrap();
                // Script Kit local: persist only the hovered link range so moving between
                // characters does not redraw, and clear it when leaving the hitbox.
                if state.hovered_link_range != updated {
                    state.hovered_link_range = updated;
                    drop(state);
                    cx.notify(current_view);
                }
            }
        });

        if !is_selection {
            // click to open link
            window.on_mouse_event({
                let links = self.links.clone();
                let text_layout = text_layout.clone();

                move |event: &MouseUpEvent, phase, _, cx| {
                    if !bounds.contains(&event.position) || !phase.bubble() {
                        return;
                    }

                    if let Some(link) =
                        Self::link_for_position(&text_layout, &links, event.position)
                    {
                        cx.stop_propagation();
                        cx.open_url(&link.url);
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Inline;
    use crate::text::node::LinkMark;

    #[test]
    fn test_link_range_for_index() {
        let links = vec![(2..5, LinkMark::default()), (8..12, LinkMark::default())];

        assert_eq!(Inline::link_range_for_index(&links, None), None);
        assert_eq!(Inline::link_range_for_index(&links, Some(1)), None);
        assert_eq!(Inline::link_range_for_index(&links, Some(3)), Some(2..5));
        assert_eq!(Inline::link_range_for_index(&links, Some(9)), Some(8..12));
    }
}

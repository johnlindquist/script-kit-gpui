use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, Entity, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, SharedString, StyleRefinement, Styled, Window, div,
};

use crate::StyledExt;
use crate::scroll::ScrollableElement;
use crate::text::TextViewFormat;
use crate::text::node::CodeBlock;
use crate::text::state::TextViewState;
use crate::{global_state::GlobalState, text::TextViewStyle};

/// Type for code block actions generator function.
pub(crate) type CodeBlockActionsFn =
    dyn Fn(&CodeBlock, &mut Window, &mut App) -> AnyElement + Send + Sync;

#[cfg(feature = "fidelity")]
fn fidelity_text_evidence(
    source: &str,
    selectable: bool,
    scrollable: bool,
) -> (String, serde_json::Value) {
    (
        Window::fidelity_text_hash(source),
        serde_json::json!({
            "sourceByteLength": source.len(),
            "selectable": selectable,
            "scrollable": scrollable,
            "textHashMode": "exact-source-bytes"
        }),
    )
}

/// A text view that can render Markdown or HTML.
///
/// ## Goals
///
/// - Provide a rich text rendering component for such as Markdown or HTML,
/// used to display rich text in GPUI application (e.g., Help messages, Release notes)
/// - Support Markdown GFM and HTML (Simple HTML like Safari Reader Mode) for showing most common used markups.
/// - Support Heading, Paragraph, Bold, Italic, StrikeThrough, Code, Link, Image, Blockquote, List, Table, HorizontalRule, CodeBlock ...
///
/// ## Not Goals
///
/// - Customization of the complex style (some simple styles will be supported)
/// - As a Markdown editor or viewer (If you want to like this, you must fork your version).
/// - As a HTML viewer, we not support CSS, we only support basic HTML tags for used to as a content reader.
///
/// See also [`MarkdownElement`], [`HtmlElement`]
#[derive(Clone)]
pub struct TextView {
    id: ElementId,
    format: Option<TextViewFormat>,
    text: Option<SharedString>,
    pub(crate) state: Option<Entity<TextViewState>>,
    text_view_style: TextViewStyle,
    style: StyleRefinement,
    selectable: bool,
    scrollable: bool,
    code_block_actions: Option<Arc<CodeBlockActionsFn>>,
    #[cfg(feature = "fidelity")]
    fidelity_scope: Option<SharedString>,
}

impl Styled for TextView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl TextView {
    /// Create new TextView with managed state.
    pub fn new(state: &Entity<TextViewState>) -> Self {
        Self {
            id: ElementId::Name(state.entity_id().to_string().into()),
            state: Some(state.clone()),
            format: None,
            text: None,
            text_view_style: TextViewStyle::default(),
            style: StyleRefinement::default(),
            selectable: false,
            scrollable: false,
            code_block_actions: None,
            #[cfg(feature = "fidelity")]
            fidelity_scope: None,
        }
    }

    /// Create a new markdown text view.
    pub fn markdown(id: impl Into<ElementId>, markdown: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            format: Some(TextViewFormat::Markdown),
            text: Some(markdown.into()),
            text_view_style: TextViewStyle::default(),
            style: StyleRefinement::default(),
            state: None,
            selectable: false,
            scrollable: false,
            code_block_actions: None,
            #[cfg(feature = "fidelity")]
            fidelity_scope: None,
        }
    }

    /// Create a new html text view.
    pub fn html(id: impl Into<ElementId>, html: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            format: Some(TextViewFormat::Html),
            text: Some(html.into()),
            text_view_style: TextViewStyle::default(),
            style: StyleRefinement::default(),
            state: None,
            selectable: false,
            scrollable: false,
            code_block_actions: None,
            #[cfg(feature = "fidelity")]
            fidelity_scope: None,
        }
    }

    /// Set [`TextViewStyle`].
    pub fn style(mut self, style: TextViewStyle) -> Self {
        self.text_view_style = style;
        self
    }

    /// Set the text view to be selectable, default is false.
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// Set the text view to be scrollable, default is false.
    ///
    /// ## If true for `scrollable`
    ///
    /// The `scrollable` mode used for large content,
    /// will show scrollbar, but requires the parent to have a fixed height,
    /// and use [`gpui::list`] to render the content in a virtualized way.
    ///
    /// ## If false to fit content
    ///
    /// The TextView will expand to fit all content, no scrollbar.
    /// This mode is suitable for small content, such as a few lines of text, a label, etc.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// Assign a stable capture-only scope for rich-text paint telemetry.
    ///
    /// The source text is represented only by a deterministic hash. This API
    /// is available when gpui-component's `fidelity` feature is enabled.
    #[cfg(feature = "fidelity")]
    pub fn fidelity_scope(mut self, scope: impl Into<SharedString>) -> Self {
        self.fidelity_scope = Some(scope.into());
        self
    }

    /// Set custom block actions for code blocks.
    ///
    /// The closure receives the [`CodeBlock`],
    /// and returns an element to display.
    pub fn code_block_actions<F, E>(mut self, f: F) -> Self
    where
        F: Fn(&CodeBlock, &mut Window, &mut App) -> E + Send + Sync + 'static,
        E: IntoElement,
    {
        self.code_block_actions = Some(Arc::new(move |code_block, window, cx| {
            f(&code_block, window, cx).into_any_element()
        }));
        self
    }
}

impl IntoElement for TextView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub struct TextViewLayoutState {
    state: Entity<TextViewState>,
    element: AnyElement,
}

impl Element for TextView {
    type RequestLayoutState = TextViewLayoutState;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let state = if let Some(state) = self.state.clone() {
            state
        } else {
            let default_format = self.format.unwrap_or(TextViewFormat::Markdown);
            let default_text = self.text.clone().unwrap_or_default();

            let state = window.use_keyed_state(
                SharedString::from(format!("{}/state", self.id)),
                cx,
                move |_, cx| {
                    if default_format == TextViewFormat::Markdown {
                        TextViewState::markdown(default_text.as_str(), cx)
                    } else {
                        TextViewState::html(default_text.as_str(), cx)
                    }
                },
            );
            self.state = Some(state.clone());
            state
        };

        state.update(cx, |state, cx| {
            state.code_block_actions = self.code_block_actions.clone();
            state.selectable = self.selectable;
            state.scrollable = self.scrollable;
            state.set_text_view_style(self.text_view_style.clone(), cx);

            if let Some(text) = self.text.clone() {
                state.set_text(text.as_str(), cx);
            }
        });

        let focus_handle = state.read(cx).focus_handle.clone();
        let list_state = state.read(cx).list_state.clone();

        let mut el = div().key_context("TextView").track_focus(&focus_handle);
        if self.scrollable {
            el = el.size_full();
            #[cfg(feature = "fidelity")]
            {
                el = if let Some(scope) = self.fidelity_scope.clone() {
                    el.vertical_scrollbar_with_fidelity_scope(
                        &list_state,
                        format!("{scope}/scrollbar"),
                    )
                } else {
                    el.vertical_scrollbar(&list_state)
                };
            }
            #[cfg(not(feature = "fidelity"))]
            {
                el = el.vertical_scrollbar(&list_state);
            }
        }
        let mut el = el
            .relative()
            .on_action(window.listener_for(&state, TextViewState::on_action_copy))
            .child(state.clone())
            .refine_style(&self.style)
            .into_any_element();
        let layout_id = el.request_layout(window, cx);
        (layout_id, TextViewLayoutState { state, element: el })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Runs re-register their layout handles during the paint that
        // follows; dropping the old set here keeps the hit map fresh.
        request_layout
            .state
            .update(cx, |state, _| state.clear_hit_runs());
        request_layout.element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = &request_layout.state;
        #[cfg(feature = "fidelity")]
        let fidelity_evidence = self.fidelity_scope.clone().map(|scope| {
            let source = state.read(cx).source();
            let (text_hash, metadata) =
                fidelity_text_evidence(source.as_ref(), self.selectable, self.scrollable);
            (scope, text_hash, metadata)
        });

        let mut paint_element = |window: &mut Window| {
            GlobalState::global_mut(cx)
                .text_view_state_stack
                .push(state.clone());
            request_layout.element.paint(window, cx);
            GlobalState::global_mut(cx).text_view_state_stack.pop();
        };

        #[cfg(feature = "fidelity")]
        if let Some((scope, text_hash, metadata)) = fidelity_evidence {
            window.with_fidelity_scope(
                format!("{scope}/document"),
                gpui::FidelityNodeKind::TextDocument,
                bounds,
                |window| {
                    window.annotate_current_fidelity_scope(Some(text_hash), None, Some(metadata));
                    paint_element(window);
                },
            );
        } else {
            paint_element(window);
        }

        #[cfg(not(feature = "fidelity"))]
        paint_element(window);

        drop(paint_element);

        if self.selectable {
            let parent_view_id = window.current_view();

            // All handlers are registered unconditionally: gating them on
            // state captured at paint time made correctness depend on a
            // repaint landing between mouse events (a real race that forced
            // tests to draw between every event).
            window.on_mouse_event({
                let state = state.clone();

                move |event: &MouseDownEvent, phase, _, cx| {
                    if !phase.bubble() || event.button != MouseButton::Left {
                        return;
                    }

                    if bounds.contains(&event.position) {
                        // Click count sets granularity: drag=graphemes,
                        // double-click=word, triple-click=paragraph.
                        state.update(cx, |state, _| {
                            state.begin_selection(event.position, event.click_count);
                        });
                    } else if state.read(cx).has_selection() {
                        state.update(cx, |state, _| state.clear_selection());
                    } else {
                        return;
                    }
                    cx.notify(parent_view_id);
                }
            });

            window.on_mouse_event({
                let state = state.clone();
                move |event: &MouseMoveEvent, phase, _, cx| {
                    // Only an actual left-button drag extends the selection;
                    // hover after a missed mouse-up must never select.
                    if !phase.bubble()
                        || event.pressed_button != Some(MouseButton::Left)
                        || !state.read(cx).is_selecting
                    {
                        return;
                    }

                    state.update(cx, |state, _| {
                        state.extend_selection(event.position);
                    });
                    cx.notify(parent_view_id);
                }
            });

            window.on_mouse_event({
                let state = state.clone();
                move |event: &MouseUpEvent, phase, _, cx| {
                    if !phase.bubble()
                        || event.button != MouseButton::Left
                        || !state.read(cx).is_selecting
                    {
                        return;
                    }

                    state.update(cx, |state, _| {
                        state.end_selection();
                    });
                    cx.notify(parent_view_id);
                }
            });
        }
    }
}

#[cfg(all(test, feature = "fidelity"))]
mod fidelity_tests {
    use super::fidelity_text_evidence;

    #[test]
    fn fidelity_text_evidence_hashes_exact_source_without_retaining_it() {
        let (text_hash, metadata) = fidelity_text_evidence("abc", true, false);

        assert_eq!(
            text_hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(metadata["sourceByteLength"], 3);
        assert_eq!(metadata["selectable"], true);
        assert_eq!(metadata["scrollable"], false);
        assert_eq!(metadata["textHashMode"], "exact-source-bytes");
        assert!(!metadata.to_string().contains("abc"));
    }
}

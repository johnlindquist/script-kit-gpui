use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

use gpui::{
    App, AppContext as _, Bounds, ClipboardItem, Context, FocusHandle, IntoElement, KeyBinding,
    ListState, ParentElement as _, Pixels, Point, Render, SharedString, Styled as _, Task, Window,
    prelude::FluentBuilder as _, px,
};
use smol::{Timer, stream::StreamExt as _};

use crate::{
    ActiveTheme, ElementExt,
    highlighter::HighlightTheme,
    input::{self, Copy},
    text::{
        CodeBlockActionsFn, TextViewStyle,
        document::ParsedDocument,
        format,
        node::{self, NodeContext},
        selection::{self, SelectionDocument, TextSelection},
    },
    v_flex,
};

const UPDATE_DELAY: Duration = Duration::from_millis(50);

const CONTEXT: &'static str = "TextView";
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys(vec![
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", input::Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", input::Copy, Some(CONTEXT)),
    ]);
}

/// The content format of the text view.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TextViewFormat {
    /// Markdown view
    Markdown,
    /// HTML view
    Html,
}

/// One rendered run's hit-testing handle, registered fresh every paint:
/// the run's document byte range plus the laid-out text and bounds.
pub(crate) struct HitRun {
    pub(crate) range: std::ops::Range<usize>,
    pub(crate) layout: gpui::TextLayout,
    pub(crate) bounds: Bounds<Pixels>,
}

/// The state of a TextView.
pub struct TextViewState {
    pub(super) focus_handle: FocusHandle,
    pub(super) list_state: ListState,

    /// The bounds of the text view
    bounds: Bounds<Pixels>,

    pub(super) selectable: bool,
    pub(super) scrollable: bool,
    pub(super) text_view_style: TextViewStyle,
    pub(super) code_block_actions: Option<Arc<CodeBlockActionsFn>>,

    pub(super) is_selecting: bool,
    /// The logical selection: byte positions into the flattened
    /// selection document. Pixel points are transient mouse input only.
    selection: Option<TextSelection>,
    /// Per-run hit-test handles, keyed by document range start and
    /// refreshed by each run's paint.
    hit_runs: std::collections::HashMap<usize, HitRun>,

    pub(super) parsed_content: Arc<Mutex<ParsedContent>>,
    text: SharedString,
    parsed_error: Option<SharedString>,
    tx: smol::channel::Sender<UpdateOptions>,
    _parse_task: Task<()>,
    _receive_task: Task<()>,
}

impl TextViewState {
    /// Create a Markdown TextViewState.
    pub fn markdown(text: &str, cx: &mut Context<Self>) -> Self {
        Self::new(TextViewFormat::Markdown, text, false, cx)
    }

    /// Create a Markdown TextViewState with its initial document parsed inline.
    ///
    /// Use this when a placeholder must hand off to the first visible text in
    /// the same frame instead of waiting for the normal streaming debounce.
    pub fn markdown_immediate(text: &str, cx: &mut Context<Self>) -> Self {
        Self::new(TextViewFormat::Markdown, text, true, cx)
    }

    /// Create a Markdown TextViewState with its initial document parsed inline.
    ///
    /// Fidelity paint tests use this constructor so their first frame does not
    /// depend on the parser's wall-clock debounce timer. Subsequent updates
    /// still use the regular asynchronous parser.
    #[cfg(feature = "fidelity")]
    pub fn markdown_for_fidelity_test(text: &str, cx: &mut Context<Self>) -> Self {
        Self::markdown_immediate(text, cx)
    }

    /// Create a HTML TextViewState.
    pub fn html(text: &str, cx: &mut Context<Self>) -> Self {
        Self::new(TextViewFormat::Html, text, false, cx)
    }

    /// Create a new TextViewState.
    fn new(
        format: TextViewFormat,
        text: &str,
        parse_initial_synchronously: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let (tx, rx) = smol::channel::unbounded::<UpdateOptions>();
        let (tx_result, rx_result) = smol::channel::unbounded::<Result<(), SharedString>>();
        let _receive_task = cx.spawn({
            async move |weak_self, cx| {
                while let Ok(parsed_result) = rx_result.recv().await {
                    _ = weak_self.update(cx, |state, cx| {
                        if let Err(err) = &parsed_result {
                            state.parsed_error = Some(err.clone());
                        }
                        // Content changed: the old selection's byte ranges
                        // and the painted layout handles are both stale.
                        state.clear_selection();
                        state.clear_hit_runs();
                        cx.notify();
                    });
                }
            }
        });

        let _parse_task = cx.background_spawn(UpdateFuture::new(format, rx, tx_result, cx));

        let mut this = Self {
            focus_handle,
            bounds: Bounds::default(),
            selection: None,
            hit_runs: std::collections::HashMap::new(),
            selectable: false,
            scrollable: false,
            list_state: ListState::new(0, gpui::ListAlignment::Top, px(1000.)),
            text_view_style: TextViewStyle::default(),
            code_block_actions: None,
            is_selecting: false,
            parsed_content: Default::default(),
            parsed_error: None,
            text: text.to_string().into(),
            tx,
            _parse_task,
            _receive_task,
        };
        if parse_initial_synchronously {
            let options = UpdateOptions {
                append: false,
                content: this.parsed_content.clone(),
                pending_text: text.to_string(),
                highlight_theme: cx.theme().highlight_theme.clone(),
                code_block_actions: this.code_block_actions.clone(),
                text_view_style: this.text_view_style.clone(),
            };
            if let Err(err) = parse_content(format, &options) {
                this.parsed_error = Some(err);
            }
        } else {
            this.increment_update(text, false, cx);
        }
        this
    }

    /// Get the text content.
    pub(crate) fn source(&self) -> SharedString {
        self.parsed_content.lock().unwrap().document.source.clone()
    }

    /// Set whether the text is selectable, default false.
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    /// Set whether the text is selectable, default false.
    pub fn set_selectable(&mut self, selectable: bool, cx: &mut Context<Self>) {
        self.selectable = selectable;
        cx.notify();
    }

    /// Set whether the text is selectable, default false.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// Set whether the text is selectable, default false.
    pub fn set_scrollable(&mut self, scrollable: bool, cx: &mut Context<Self>) {
        self.scrollable = scrollable;
        cx.notify();
    }

    pub(super) fn set_text_view_style(&mut self, style: TextViewStyle, cx: &mut Context<Self>) {
        if self.text_view_style == style {
            return;
        }

        self.text_view_style = style;
        let text = self.text.to_string();
        self.increment_update(&text, false, cx);
    }

    /// Set the text content.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.text.as_str() == text {
            return;
        }

        self.text = text.to_string().into();
        self.parsed_error = None;
        self.increment_update(text, false, cx);
    }

    /// Set Markdown text and parse it before returning.
    ///
    /// This is intentionally separate from [`Self::set_text`], whose debounce
    /// is desirable for normal streaming updates.
    pub fn set_markdown_text_immediate(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.text.as_str() == text {
            return;
        }

        self.text = text.to_string().into();
        let options = UpdateOptions {
            append: false,
            content: self.parsed_content.clone(),
            pending_text: text.to_string(),
            highlight_theme: cx.theme().highlight_theme.clone(),
            code_block_actions: self.code_block_actions.clone(),
            text_view_style: self.text_view_style.clone(),
        };
        self.parsed_error = parse_content(TextViewFormat::Markdown, &options).err();
        self.clear_selection();
        cx.notify();
    }

    /// Print parsed blocks for debugging.
    pub fn debug_print(&self) {
        let content = self.parsed_content.lock().unwrap();
        println!("TEXT: {:?}", self.text);
        println!("BLOCKS: {:#?}", content.document.blocks);
    }

    /// Append partial text content to the existing text.
    pub fn push_str(&mut self, new_text: &str, cx: &mut Context<Self>) {
        if new_text.is_empty() {
            return;
        }
        self.increment_update(new_text, true, cx);
    }

    /// Return the selected text: an exact slice of the flattened selection
    /// document (whitespace preserved, independent of what was painted).
    pub fn selected_text(&self) -> String {
        let Some(selection) = &self.selection else {
            return String::new();
        };
        let content = self.parsed_content.lock().unwrap();
        let range = selection.effective_range(&content.selection);
        content.selection.text[range].to_string()
    }

    fn increment_update(&mut self, text: &str, append: bool, cx: &mut Context<Self>) {
        let code_block_actions = self.code_block_actions.clone();
        let update_options = UpdateOptions {
            append,
            content: self.parsed_content.clone(),
            pending_text: text.to_string(),
            highlight_theme: cx.theme().highlight_theme.clone(),
            code_block_actions: code_block_actions.clone(),
            text_view_style: self.text_view_style.clone(),
        };

        // Parse at first time by blocking.
        _ = self.tx.try_send(update_options);
    }

    /// Save bounds. The selection is logical (text-anchored), so reflow and
    /// resize preserve it; only content changes clear it.
    pub(super) fn update_bounds(&mut self, bounds: Bounds<Pixels>) {
        self.bounds = bounds;
    }

    pub(super) fn clear_selection(&mut self) {
        self.selection = None;
        self.is_selecting = false;
    }

    /// Runs re-register their layout handles on every paint; content
    /// changes drop them wholesale so a stale layout is never hit-tested
    /// against a new document.
    pub(super) fn clear_hit_runs(&mut self) {
        self.hit_runs.clear();
    }

    pub(crate) fn register_hit_run(&mut self, run: HitRun) {
        self.hit_runs.insert(run.range.start, run);
    }

    /// Map a window point to a document byte position: the run whose
    /// vertical band contains the point (else the nearest run), then the
    /// layout's nearest caret index, snapped to a grapheme boundary.
    fn hit_test(&self, position: Point<Pixels>) -> Option<usize> {
        let run = self.hit_runs.values().min_by_key(|run| {
            let bounds = run.bounds;
            let dy = if position.y < bounds.top() {
                bounds.top() - position.y
            } else if position.y > bounds.bottom() {
                position.y - bounds.bottom()
            } else {
                Pixels::ZERO
            };
            // Prefer vertical containment; break ties horizontally.
            let dx = if position.x < bounds.left() {
                bounds.left() - position.x
            } else if position.x > bounds.right() {
                position.x - bounds.right()
            } else {
                Pixels::ZERO
            };
            ((f32::from(dy) * 10_000.0) + f32::from(dx)) as i64
        })?;
        let local = match run.layout.index_for_position(position) {
            Ok(index) => index,
            // Outside the glyphs: the layout's nearest caret index.
            Err(index) => index,
        };
        let byte = run.range.start + local.min(run.range.len());
        let content = self.parsed_content.lock().unwrap();
        Some(selection::snap_to_grapheme(&content.selection.text, byte))
    }

    pub(super) fn begin_selection(&mut self, position: Point<Pixels>, click_count: usize) {
        let Some(byte) = self.hit_test(position) else {
            return;
        };
        let content = self.parsed_content.lock().unwrap();
        let selection = TextSelection::begin(&content.selection, byte, click_count);
        drop(content);
        self.selection = Some(selection);
        self.is_selecting = true;
    }

    pub(super) fn extend_selection(&mut self, position: Point<Pixels>) {
        if !self.is_selecting {
            return;
        }
        let Some(byte) = self.hit_test(position) else {
            return;
        };
        if let Some(selection) = &mut self.selection {
            selection.focus = byte;
        }
    }

    pub(super) fn end_selection(&mut self) {
        self.is_selecting = false;
        // A plain click (no drag, grapheme granularity) selects nothing.
        if self
            .effective_selection_range()
            .is_none_or(|range| range.is_empty())
        {
            self.selection = None;
        }
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.effective_selection_range()
            .is_some_and(|range| !range.is_empty())
    }

    /// The selected document byte range, when a selection exists.
    pub(crate) fn effective_selection_range(&self) -> Option<std::ops::Range<usize>> {
        let selection = self.selection.as_ref()?;
        let content = self.parsed_content.lock().unwrap();
        Some(selection.effective_range(&content.selection))
    }

    pub(super) fn on_action_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        // Exact copy: the selected slice verbatim — deliberately selected
        // whitespace and indentation are content, never trimmed.
        let selected_text = self.selected_text();
        if selected_text.is_empty() {
            return;
        }

        cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
    }

    pub(crate) fn is_selectable(&self) -> bool {
        self.selectable
    }
}

impl Render for TextViewState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.entity();
        let (document, node_cx) = {
            let content = self.parsed_content.lock().unwrap();
            (content.document.clone(), content.node_cx.clone())
        };

        v_flex()
            .size_full()
            .map(|this| match &mut self.parsed_error {
                None => this.child(document.render_root(
                    if self.scrollable {
                        Some(self.list_state.clone())
                    } else {
                        None
                    },
                    &node_cx,
                    window,
                    cx,
                )),
                Some(err) => this.child(
                    v_flex()
                        .gap_1()
                        .child("Failed to parse content")
                        .child(err.to_string()),
                ),
            })
            .on_prepaint(move |bounds, _, cx| {
                state.update(cx, |state, _| {
                    state.update_bounds(bounds);
                })
            })
    }
}

#[derive(PartialEq, Default)]
pub(crate) struct ParsedContent {
    pub(crate) document: ParsedDocument,
    pub(crate) node_cx: node::NodeContext,
    /// The rendered document flattened for logical selection and copy.
    pub(crate) selection: SelectionDocument,
}

struct UpdateFuture {
    format: TextViewFormat,
    options: UpdateOptions,
    pending_text: String,
    timer: Timer,
    rx: Pin<Box<smol::channel::Receiver<UpdateOptions>>>,
    tx_result: smol::channel::Sender<Result<(), SharedString>>,
    delay: Duration,
}

impl UpdateFuture {
    fn new(
        format: TextViewFormat,
        rx: smol::channel::Receiver<UpdateOptions>,
        tx_result: smol::channel::Sender<Result<(), SharedString>>,
        cx: &App,
    ) -> Self {
        Self {
            format,
            pending_text: String::new(),
            options: UpdateOptions {
                append: false,
                pending_text: String::new(),
                content: Default::default(),
                highlight_theme: cx.theme().highlight_theme.clone(),
                code_block_actions: None,
                text_view_style: TextViewStyle::default(),
            },
            timer: Timer::never(),
            rx: Box::pin(rx),
            tx_result,
            delay: UPDATE_DELAY,
        }
    }
}

impl Future for UpdateFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.rx.poll_next(cx) {
                Poll::Ready(Some(options)) => {
                    let delay = self.delay;
                    if options.append {
                        self.pending_text.push_str(options.pending_text.as_str());
                    } else {
                        self.pending_text = options.pending_text.clone();
                    }
                    self.options = options;
                    self.timer.set_after(delay);
                    continue;
                }
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => {}
            }

            match self.timer.poll_next(cx) {
                Poll::Ready(Some(_)) => {
                    let pending_text = std::mem::take(&mut self.pending_text);

                    let res = parse_content(
                        self.format,
                        &UpdateOptions {
                            pending_text,
                            ..self.options.clone()
                        },
                    );
                    _ = self.tx_result.try_send(res);
                    continue;
                }
                Poll::Ready(None) | Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[derive(Clone)]
struct UpdateOptions {
    content: Arc<Mutex<ParsedContent>>,
    pending_text: String,
    append: bool,
    highlight_theme: Arc<HighlightTheme>,
    code_block_actions: Option<Arc<CodeBlockActionsFn>>,
    text_view_style: TextViewStyle,
}

fn parse_content(format: TextViewFormat, options: &UpdateOptions) -> Result<(), SharedString> {
    let mut node_cx = NodeContext {
        code_block_actions: options.code_block_actions.clone(),
        style: options.text_view_style.clone(),
        ..NodeContext::default()
    };

    let mut content = options.content.lock().unwrap();
    let mut source = String::new();
    if options.append
        && let Some(last_block) = content.document.blocks.pop()
        && let Some(span) = last_block.span()
    {
        node_cx.offset = span.start;
        let last_source = &content.document.source[span.start..];
        source.push_str(last_source);
        source.push_str(&options.pending_text);
    } else {
        source = options.pending_text.to_string();
    }

    let new_content = match format {
        TextViewFormat::Markdown => {
            format::markdown::parse(&source, &mut node_cx, &options.highlight_theme)
        }
        TextViewFormat::Html => format::html::parse(&source, &mut node_cx),
    }?;

    if options.append {
        content.document.source =
            format!("{}{}", content.document.source, options.pending_text).into();
        content.document.blocks.extend(new_content.blocks);
    } else {
        content.document = new_content;
    }
    content.node_cx = node_cx;
    // Rebuild the flattened selection document and re-stamp every run's
    // document range in the same pass the renderer will read from.
    content.selection = selection::build_selection_document(&content.document);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn immediate_markdown_update_is_visible_before_return(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init);
        let state = cx.new(|cx| TextViewState::markdown_immediate("", cx));

        state.update(cx, |state, cx| {
            state.set_markdown_text_immediate("First token", cx);
            assert_eq!(state.source().as_ref(), "First token");
        });
    }
}

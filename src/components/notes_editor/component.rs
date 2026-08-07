use gpui::{App, AppContext, Context, Entity, FocusHandle, Focusable, Hsla, ScrollHandle, Window};
use gpui_component::input::InputState;
use gpui_component::theme::ActiveTheme;
use std::ops::Range;

use crate::notes::markdown_highlighting::register_markdown_highlighter;

use super::types::{
    NotesEditorConfig, NotesEditorInputSizing, NotesEditorLayout, NotesEditorMarkdownConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkdownLinkTarget {
    pub(crate) href_range: Range<usize>,
    pub(crate) full_range: Range<usize>,
    pub(crate) href: String,
    pub(crate) role: MarkdownLinkTargetRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkdownLinkTargetRole {
    InlineMarkdown,
    ReferenceDefinition,
    Autolink,
    BareUri,
    LocalPath,
}

/// Shared markdown editor used by the Notes window and future Day Page surface.
///
/// Owns markdown editing, formatting entry points, code highlighting registration,
/// preview rendering, and editor focus. The host binds document content, wires
/// save/change callbacks, and supplies chrome-specific empty states.
pub struct NotesEditor {
    pub(crate) input_state: Entity<InputState>,
    pub(crate) preview_scroll_handle: ScrollHandle,
    pub(crate) layout: NotesEditorLayout,
    last_markdown_link_highlight_text: String,
    last_markdown_link_highlight_selection: Range<usize>,
    last_markdown_link_highlight_ranges: Vec<(Range<usize>, Hsla, String)>,
}

impl NotesEditor {
    pub fn new_markdown_pair<T>(
        window: &mut Window,
        cx: &mut Context<T>,
        config: NotesEditorMarkdownConfig,
    ) -> (Entity<InputState>, Entity<Self>)
    where
        T: 'static,
    {
        register_markdown_highlighter();

        let editor_config = config.editor;
        let placeholder = editor_config.placeholder.clone();
        let initial_content = editor_config.initial_content.clone();
        let measurement_surface = editor_config.measurement_surface.clone();
        let sizing = config.sizing;
        let input_state = cx.new(|cx| {
            let state = InputState::new(window, cx)
                .code_editor("markdown")
                .fidelity_scope(measurement_surface)
                .code_editor_dynamic_bottom_margin(false)
                .line_number(false)
                .searchable(true)
                .placeholder(placeholder)
                .default_value(initial_content);
            match sizing {
                NotesEditorInputSizing::Rows(rows) => state.rows(rows),
                NotesEditorInputSizing::AutoGrow { min_rows, max_rows } => {
                    state.auto_grow(min_rows, max_rows)
                }
            }
        });
        let notes_editor = cx.new(|_| NotesEditor::new(input_state.clone(), editor_config));
        notes_editor.update(cx, |editor, cx| editor.sync_markdown_link_highlights(cx));
        (input_state, notes_editor)
    }

    pub fn new(input_state: Entity<InputState>, config: NotesEditorConfig) -> Self {
        register_markdown_highlighter();

        Self {
            input_state,
            preview_scroll_handle: ScrollHandle::new(),
            layout: config.layout,
            last_markdown_link_highlight_text: String::new(),
            last_markdown_link_highlight_selection: 0..0,
            last_markdown_link_highlight_ranges: Vec::new(),
        }
    }

    pub fn input_state(&self) -> Entity<InputState> {
        self.input_state.clone()
    }

    pub fn preview_scroll_handle(&self) -> &ScrollHandle {
        &self.preview_scroll_handle
    }

    pub fn layout(&self) -> NotesEditorLayout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: NotesEditorLayout) {
        self.layout = layout;
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    #[allow(dead_code)] // WIP editor-focus helper; wired once the caller lands.
    pub(crate) fn focus_with_cursor_at_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let cursor = state.value().len();
            state.set_selection(cursor, cursor, window, cx);
            // `set_selection`'s scroll_to is a no-op until the element has
            // painted (last_layout/last_bounds are None on the load/mount
            // frame), so the Day Page would land at the top on open/reopen.
            // This vendor flag is consumed during the next paint
            // (element.rs: scroll_to_bottom_after_layout) to force the scroll
            // offset to the bottom once layout commits.
            state.scroll_to_bottom_after_layout(cx);
            state.scroll_to_bottom(cx);
        });
    }

    #[allow(dead_code)] // WIP editor helper; wired once the caller lands.
    pub(crate) fn scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            state.scroll_to_bottom_after_layout(cx);
            state.scroll_to_bottom(cx);
        });
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input_state.read(cx).focus_handle(cx)
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle(cx).is_focused(window)
    }

    pub fn set_value(
        &mut self,
        value: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.into();
        self.input_state.update(cx, |state, cx| {
            state.set_value(value, window, cx);
        });
        self.sync_markdown_link_highlights(cx);
    }

    pub fn load_value_with_cursor_at_end(
        &mut self,
        value: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.into();
        let cursor = value.len();
        self.input_state.update(cx, |state, cx| {
            state.set_value(value, window, cx);
            state.set_selection(cursor, cursor, window, cx);
            state.scroll_to_bottom_after_layout(cx);
            state.scroll_to_bottom(cx);
        });
        self.sync_markdown_link_highlights(cx);
    }

    pub fn set_value_with_cursor_at_end(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.load_value_with_cursor_at_end(text, window, cx);
    }

    /// Rewrite the buffer in place (normalization, mention fixups, formatting)
    /// without moving the viewport. `set_value` + `set_selection` resets the
    /// scroll to the top and then re-anchors the cursor at the viewport edge,
    /// which reads as a jump while typing.
    pub fn set_value_preserving_scroll(
        &mut self,
        value: impl Into<String>,
        cursor: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.into();
        self.input_state.update(cx, |state, cx| {
            state.set_value_preserving_scroll(value, cursor, window, cx);
        });
        self.sync_markdown_link_highlights(cx);
    }

    pub fn set_selection(
        &mut self,
        start: usize,
        end: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input_state.update(cx, |state, cx| {
            state.set_selection(start, end, window, cx);
        });
    }

    pub fn content(&self, cx: &App) -> String {
        self.input_state.read(cx).value().to_string()
    }

    pub fn selection(&self, cx: &App) -> std::ops::Range<usize> {
        self.input_state.read(cx).selection()
    }

    pub fn soft_wrapped_lines_len(&self, cx: &App) -> usize {
        self.input_state.read(cx).soft_wrapped_lines_len()
    }

    pub fn has_inline_completion(&self, cx: &App) -> bool {
        self.input_state.read(cx).has_inline_completion()
    }

    pub fn sync_inline_completion(&mut self, suffix: Option<String>, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| match suffix {
            Some(suffix) => state.set_inline_completion_text(suffix, cx),
            None => {
                if state.has_inline_completion() {
                    state.clear_inline_completion(cx);
                }
            }
        });
    }

    pub fn read_input<F, R>(&self, cx: &App, f: F) -> R
    where
        F: FnOnce(&InputState) -> R,
    {
        f(self.input_state.read(cx))
    }

    pub fn markdown_runtime_info(&self) -> crate::protocol::ElementEditorRuntimeInfo {
        crate::notes::markdown_highlighting::markdown_editor_runtime_info()
    }

    pub(crate) fn activation_href_at_cursor(&self, cx: &App) -> Option<String> {
        self.read_input(cx, |state| {
            activation_href_at_cursor_in_text(&state.value(), state.selection().start)
        })
    }

    /// Href of the deeplink currently under the mouse, if any.
    ///
    /// The editor's custom highlight ranges are exclusively markdown-link
    /// spans (`sync_markdown_link_highlights`), so the vendor input's hovered
    /// highlight range maps directly to a link; the href resolves through the
    /// same target parser the click path uses, keeping hover and activation
    /// honest with each other.
    pub(crate) fn hovered_deeplink(&self, cx: &App) -> Option<String> {
        self.read_input(cx, |state| {
            let (range, _role) = state.hovered_highlight_range()?;
            activation_href_at_cursor_in_text(&state.value(), range.start)
        })
    }

    pub fn markdown_runtime_info_with_scroll(
        &self,
        cx: &App,
    ) -> crate::protocol::ElementEditorRuntimeInfo {
        let mut info = self.markdown_runtime_info();
        info.editor_scroll_metrics =
            Some(self.read_input(cx, |state| state.automation_scroll_metrics()));
        info.markdown_link_highlight_ranges = Some(self.markdown_link_highlight_runtime_info(cx));
        info
    }

    fn markdown_link_highlight_runtime_info(&self, cx: &App) -> serde_json::Value {
        self.read_input(cx, |state| {
            let text = state.value();
            let roles = state.highlight_range_roles();
            let ranges = state
                .highlight_ranges()
                .iter()
                .enumerate()
                .filter_map(|(index, (range, _color))| {
                    let range_text = text.get(range.clone())?;
                    // Window-coordinate pixel bounds (when the range is laid
                    // out) so probes can mouseMove to a link center and
                    // assert hover state without screenshots.
                    let bounds = state.highlight_range_bounds(range).map(|bounds| {
                        serde_json::json!({
                            "x": bounds.origin.x.as_f32(),
                            "y": bounds.origin.y.as_f32(),
                            "width": bounds.size.width.as_f32(),
                            "height": bounds.size.height.as_f32(),
                        })
                    });
                    Some(serde_json::json!({
                        "range": [range.start, range.end],
                        "content": crate::protocol::RedactedElementContent::new(
                            crate::protocol::ElementContentKind::UserContent,
                            range_text,
                        ),
                        "role": roles
                            .get(index)
                            .cloned()
                            .unwrap_or_else(|| "markdownLink".to_string()),
                        "bounds": bounds,
                    }))
                })
                .collect::<Vec<_>>();
            let hovered = state.hovered_highlight_range().map(|(range, role)| {
                let href = activation_href_at_cursor_in_text(&text, range.start);
                serde_json::json!({
                    "range": [range.start, range.end],
                    "role": role,
                    "hrefContent": href.as_deref().map(|href| {
                        crate::protocol::RedactedElementContent::new(
                            crate::protocol::ElementContentKind::FilePath,
                            href,
                        )
                    }),
                })
            });
            serde_json::json!({
                "count": ranges.len(),
                "ranges": ranges,
                "hovered": hovered,
            })
        })
    }

    pub fn sync_markdown_link_highlights(&mut self, cx: &mut Context<Self>) {
        let (text, selection) = {
            let state = self.input_state.read(cx);
            (state.value().to_string(), state.selection())
        };
        if text == self.last_markdown_link_highlight_text
            && selection == self.last_markdown_link_highlight_selection
        {
            return;
        }

        let accent = cx.theme().accent;
        let ranges = markdown_link_highlight_ranges(&text, accent, selection.clone());
        if ranges != self.last_markdown_link_highlight_ranges {
            self.input_state.update(cx, |state, _cx| {
                state.set_highlight_ranges_with_roles(ranges.clone());
            });
            self.last_markdown_link_highlight_ranges = ranges;
        }
        self.last_markdown_link_highlight_text = text;
        self.last_markdown_link_highlight_selection = selection;
    }
}

pub(crate) fn should_activate_deeplink_from_mouse_up(
    event: &gpui::MouseUpEvent,
    selection: Range<usize>,
) -> bool {
    event.button == gpui::MouseButton::Left
        && event.click_count == 1
        && !event.modifiers.platform
        && !event.modifiers.shift
        && !event.modifiers.control
        && !event.modifiers.alt
        && selection.is_empty()
}

/// Authored opacity for a markdown link destination while no selection
/// overlaps or touches the link ("rest"/compact state). Shared with the
/// design-contract exporter as `notesEditor.link.destinationCompactOpacity`.
pub(crate) const MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY: f32 = 0.45;

/// The rest-state destination color: the shared link accent at the compact
/// opacity. This is the ONE resolver both the highlighter
/// (`markdown_link_destination_color`) and the design-contract exporter use;
/// the active state paints the plain accent.
pub(crate) fn markdown_link_destination_rest_color(accent: Hsla) -> Hsla {
    accent.opacity(MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY)
}

fn markdown_link_highlight_ranges(
    text: &str,
    accent: Hsla,
    selection: Range<usize>,
) -> Vec<(Range<usize>, Hsla, String)> {
    let targets = markdown_link_targets(text);
    let mut ranges = Vec::new();
    for line in markdown_non_code_lines(text) {
        collect_reference_definition_destination(
            text,
            line.clone(),
            accent,
            selection.clone(),
            &targets,
            &mut ranges,
        );
        collect_inline_markdown_links(
            text,
            line.clone(),
            accent,
            selection.clone(),
            &targets,
            &mut ranges,
        );
        collect_autolinks_and_bare_urls(
            text,
            line,
            accent,
            selection.clone(),
            &targets,
            &mut ranges,
        );
    }
    ranges.sort_by(|(a, _, _), (b, _, _)| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    ranges.dedup_by(|(a, _, ar), (b, _, br)| a == b && ar == br);
    ranges
}

fn markdown_link_destination_color(
    accent: Hsla,
    selection: &Range<usize>,
    targets: &[MarkdownLinkTarget],
    range: &Range<usize>,
) -> Hsla {
    let active = targets.iter().any(|target| {
        ranges_overlap_or_touch(&target.full_range, selection)
            && ranges_overlap_or_touch(&target.href_range, range)
    });
    if active {
        accent
    } else {
        markdown_link_destination_rest_color(accent)
    }
}

pub(crate) fn activation_href_at_cursor_in_text(text: &str, cursor: usize) -> Option<String> {
    markdown_link_targets(text)
        .into_iter()
        .filter(|target| cursor >= target.full_range.start && cursor <= target.full_range.end)
        .min_by_key(|target| target.full_range.end - target.full_range.start)
        .map(|target| target.href)
}

pub(crate) fn markdown_link_targets(text: &str) -> Vec<MarkdownLinkTarget> {
    let mut targets = Vec::new();
    for line in markdown_non_code_lines(text) {
        collect_reference_definition_targets(text, line.clone(), &mut targets);
        collect_inline_markdown_link_targets(text, line.clone(), &mut targets);
        collect_autolinks_bare_urls_and_paths(text, line, &mut targets);
    }
    targets.sort_by(|a, b| {
        a.full_range
            .start
            .cmp(&b.full_range.start)
            .then(a.full_range.end.cmp(&b.full_range.end))
            .then(a.href_range.start.cmp(&b.href_range.start))
    });
    targets.dedup_by(|a, b| a.full_range == b.full_range && a.href_range == b.href_range);
    targets
}

fn markdown_non_code_lines(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut in_fence = false;
    let mut offset = 0;
    for segment in text.split_inclusive('\n') {
        let line_end = offset + segment.len();
        let line = segment.trim_end_matches('\n');
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            offset = line_end;
            continue;
        }
        if !in_fence {
            ranges.push(offset..line_end);
        }
        offset = line_end;
    }
    if offset < text.len() && !in_fence {
        ranges.push(offset..text.len());
    }
    ranges
}

fn collect_inline_markdown_link_targets(
    text: &str,
    line: Range<usize>,
    targets: &mut Vec<MarkdownLinkTarget>,
) {
    let bytes = text.as_bytes();
    let mut index = line.start;
    while index < line.end {
        if bytes[index] != b'[' || index > 0 && bytes[index - 1] == b'!' {
            index += 1;
            continue;
        }
        let Some(label_close) = find_unescaped_byte(text, index + 1, line.end, b']') else {
            break;
        };
        if bytes.get(label_close + 1).copied() == Some(b'(') {
            if let Some(dest_end) = find_markdown_destination_end(text, label_close + 2, line.end) {
                let dest = trim_ascii_range(text, label_close + 2..dest_end);
                if dest.start < dest.end {
                    targets.push(MarkdownLinkTarget {
                        href_range: dest.clone(),
                        full_range: index..dest_end + 1,
                        href: text[dest].to_string(),
                        role: MarkdownLinkTargetRole::InlineMarkdown,
                    });
                }
                index = dest_end + 1;
                continue;
            }
        }
        index = label_close + 1;
    }
}

fn collect_reference_definition_targets(
    text: &str,
    line: Range<usize>,
    targets: &mut Vec<MarkdownLinkTarget>,
) {
    let line_text = &text[line.clone()];
    let trimmed_start = line.start + line_text.len() - line_text.trim_start().len();
    if text.as_bytes().get(trimmed_start) != Some(&b'[') {
        return;
    }
    let Some(label_close) = find_unescaped_byte(text, trimmed_start + 1, line.end, b']') else {
        return;
    };
    if text.as_bytes().get(label_close + 1) != Some(&b':') {
        return;
    }
    let dest = trim_ascii_range(text, label_close + 2..line.end);
    if dest.start < dest.end {
        targets.push(MarkdownLinkTarget {
            href_range: dest.clone(),
            full_range: trimmed_start..dest.end,
            href: text[dest].to_string(),
            role: MarkdownLinkTargetRole::ReferenceDefinition,
        });
    }
}

fn collect_autolinks_bare_urls_and_paths(
    text: &str,
    line: Range<usize>,
    targets: &mut Vec<MarkdownLinkTarget>,
) {
    let bytes = text.as_bytes();
    let mut index = line.start;
    while index < line.end {
        if bytes[index] == b'<' {
            if let Some(close) = find_unescaped_byte(text, index + 1, line.end, b'>') {
                let inner = index + 1..close;
                if is_supported_bare_uri(&text[inner.clone()]) {
                    targets.push(MarkdownLinkTarget {
                        href_range: inner.clone(),
                        full_range: index..close + 1,
                        href: text[inner].to_string(),
                        role: MarkdownLinkTargetRole::Autolink,
                    });
                }
                index = close + 1;
                continue;
            }
        }
        if starts_url_at(text, index) || starts_local_path_at(text, index, line.start) {
            let mut end = index;
            while end < line.end && !text.as_bytes()[end].is_ascii_whitespace() {
                end += 1;
            }
            let range = trim_url_trailing_punctuation(text, index..end);
            if range.start < range.end
                && !targets
                    .iter()
                    .any(|target| ranges_overlap(&target.full_range, &range))
            {
                targets.push(MarkdownLinkTarget {
                    href_range: range.clone(),
                    full_range: range.clone(),
                    href: text[range.clone()].to_string(),
                    role: if starts_local_path_at(text, range.start, line.start) {
                        MarkdownLinkTargetRole::LocalPath
                    } else {
                        MarkdownLinkTargetRole::BareUri
                    },
                });
            }
            index = end;
            continue;
        }
        index += 1;
    }
}

fn collect_inline_markdown_links(
    text: &str,
    line: Range<usize>,
    accent: Hsla,
    selection: Range<usize>,
    targets: &[MarkdownLinkTarget],
    ranges: &mut Vec<(Range<usize>, Hsla, String)>,
) {
    let bytes = text.as_bytes();
    let mut index = line.start;
    while index < line.end {
        if bytes[index] != b'[' || index > 0 && bytes[index - 1] == b'!' {
            index += 1;
            continue;
        }
        let Some(label_close) = find_unescaped_byte(text, index + 1, line.end, b']') else {
            break;
        };
        let label = index + 1..label_close;
        match bytes.get(label_close + 1).copied() {
            Some(b'(') => {
                if let Some(dest_end) =
                    find_markdown_destination_end(text, label_close + 2, line.end)
                {
                    if label.start < label.end {
                        ranges.push((label, accent, "markdownLinkText".to_string()));
                    }
                    let dest = trim_ascii_range(text, label_close + 2..dest_end);
                    if dest.start < dest.end {
                        let color =
                            markdown_link_destination_color(accent, &selection, targets, &dest);
                        ranges.push((dest, color, "markdownLinkUri".to_string()));
                    }
                    index = dest_end + 1;
                    continue;
                }
            }
            Some(b'[') => {
                if let Some(ref_close) = find_unescaped_byte(text, label_close + 2, line.end, b']')
                {
                    if label.start < label.end {
                        ranges.push((label, accent, "markdownLinkText".to_string()));
                    }
                    let reference = label_close + 2..ref_close;
                    if reference.start < reference.end {
                        ranges.push((reference, accent, "markdownLinkReference".to_string()));
                    }
                    index = ref_close + 1;
                    continue;
                }
            }
            _ => {}
        }
        index = label_close + 1;
    }
}

fn collect_reference_definition_destination(
    text: &str,
    line: Range<usize>,
    accent: Hsla,
    selection: Range<usize>,
    targets: &[MarkdownLinkTarget],
    ranges: &mut Vec<(Range<usize>, Hsla, String)>,
) {
    let line_text = &text[line.clone()];
    let trimmed_start = line.start + line_text.len() - line_text.trim_start().len();
    if text.as_bytes().get(trimmed_start) != Some(&b'[') {
        return;
    }
    let Some(label_close) = find_unescaped_byte(text, trimmed_start + 1, line.end, b']') else {
        return;
    };
    if text.as_bytes().get(label_close + 1) != Some(&b':') {
        return;
    }
    let label = trimmed_start + 1..label_close;
    if label.start < label.end {
        ranges.push((label, accent, "markdownLinkReference".to_string()));
    }
    let dest = trim_ascii_range(text, label_close + 2..line.end);
    if dest.start < dest.end {
        let color = markdown_link_destination_color(accent, &selection, targets, &dest);
        ranges.push((dest, color, "markdownLinkUri".to_string()));
    }
}

fn collect_autolinks_and_bare_urls(
    text: &str,
    line: Range<usize>,
    accent: Hsla,
    selection: Range<usize>,
    targets: &[MarkdownLinkTarget],
    ranges: &mut Vec<(Range<usize>, Hsla, String)>,
) {
    let bytes = text.as_bytes();
    let mut index = line.start;
    while index < line.end {
        if bytes[index] == b'<' {
            if let Some(close) = find_unescaped_byte(text, index + 1, line.end, b'>') {
                let inner = index + 1..close;
                if text[inner.clone()].starts_with("http://")
                    || text[inner.clone()].starts_with("https://")
                    || text[inner.clone()].starts_with("scriptkit://")
                    || text[inner.clone()].starts_with("kit://")
                    || text[inner.clone()].starts_with("file:")
                {
                    let color =
                        markdown_link_destination_color(accent, &selection, targets, &inner);
                    ranges.push((inner, color, "markdownLinkUri".to_string()));
                }
                index = close + 1;
                continue;
            }
        }
        if starts_url_at(text, index) {
            let mut end = index;
            while end < line.end && !text.as_bytes()[end].is_ascii_whitespace() {
                end += 1;
            }
            let url = trim_url_trailing_punctuation(text, index..end);
            if url.start < url.end
                && !ranges
                    .iter()
                    .any(|(range, _, _)| ranges_overlap(range, &url))
            {
                let color = markdown_link_destination_color(accent, &selection, targets, &url);
                ranges.push((url, color, "markdownLinkUri".to_string()));
            }
            index = end;
            continue;
        }
        index += 1;
    }
}

fn starts_url_at(text: &str, index: usize) -> bool {
    let bytes = &text.as_bytes()[index..];
    bytes.starts_with(b"http://")
        || bytes.starts_with(b"https://")
        || bytes.starts_with(b"scriptkit://")
        || bytes.starts_with(b"kit://")
        || bytes.starts_with(b"file:")
}

fn starts_local_path_at(text: &str, index: usize, line_start: usize) -> bool {
    let bytes = text.as_bytes();
    let starts_path = bytes[index..].starts_with(b"/") || bytes[index..].starts_with(b"~/");
    if !starts_path {
        return false;
    }
    if index > line_start && !bytes[index - 1].is_ascii_whitespace() {
        return false;
    }
    true
}

fn is_supported_bare_uri(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("scriptkit://")
        || value.starts_with("kit://")
        || value.starts_with("file:")
}

fn find_unescaped_byte(text: &str, start: usize, end: usize, needle: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = start;
    while index < end {
        if bytes[index] == needle && (index == 0 || bytes[index - 1] != b'\\') {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn find_markdown_destination_end(text: &str, start: usize, end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = start;
    while index < end {
        match bytes[index] {
            b'\\' => index += 1,
            b'(' => depth += 1,
            b')' if depth == 0 => return Some(index),
            b')' => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    None
}

fn trim_ascii_range(text: &str, range: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let mut start = range.start;
    let mut end = range.end;
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    start..end
}

fn trim_url_trailing_punctuation(text: &str, range: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let mut end = range.end;
    while end > range.start {
        match bytes[end - 1] {
            b'.' | b',' | b';' | b':' => end -= 1,
            _ => break,
        }
    }
    range.start..end
}

fn ranges_overlap(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

fn ranges_overlap_or_touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && b.start <= a.end
}

#[cfg(test)]
mod tests {
    use super::{
        activation_href_at_cursor_in_text, markdown_link_destination_rest_color,
        markdown_link_highlight_ranges, should_activate_deeplink_from_mouse_up,
        MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY,
    };
    use gpui::rgb;
    use std::ops::Range;

    fn highlighted_texts(input: &str) -> Vec<String> {
        markdown_link_highlight_ranges(input, rgb(0xffcc00).into(), 0..0)
            .into_iter()
            .map(|(range, _, role)| format!("{role}:{}", &input[range]))
            .collect()
    }

    fn highlight_for_role(
        input: &str,
        role: &str,
        selection: Range<usize>,
    ) -> (String, gpui::Hsla) {
        markdown_link_highlight_ranges(input, rgb(0xffcc00).into(), selection)
            .into_iter()
            .find_map(|(range, color, candidate_role)| {
                (candidate_role == role).then(|| (input[range].to_string(), color))
            })
            .expect("expected highlight role")
    }

    #[test]
    fn markdown_link_highlights_cover_inline_links_and_urls() {
        let input = "[Screenflow](scriptkit://spine/file/screenflow)\nhttps://example.com/path,\n";
        assert_eq!(
            highlighted_texts(input),
            vec![
                "markdownLinkText:Screenflow",
                "markdownLinkUri:scriptkit://spine/file/screenflow",
                "markdownLinkUri:https://example.com/path",
            ]
        );
    }

    #[test]
    fn markdown_link_destinations_are_muted_until_focused() {
        let input = "Open [Clipboard entry](kit://clipboard-history?id=clip-1) today";
        let href_cursor = input.find("clipboard-history").unwrap();
        let label_cursor = input.find("Clipboard").unwrap();

        let (muted_text, muted_color) = highlight_for_role(input, "markdownLinkUri", 0..0);
        assert_eq!(muted_text, "kit://clipboard-history?id=clip-1");
        assert_eq!(muted_color.a, MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY);
        // The highlighter's rest state IS the shared resolver's output — the
        // design contract exports the same color, never a re-derived one.
        assert_eq!(
            muted_color,
            markdown_link_destination_rest_color(gpui::rgb(0xffcc00).into())
        );

        let (href_text, href_color) =
            highlight_for_role(input, "markdownLinkUri", href_cursor..href_cursor);
        assert_eq!(href_text, "kit://clipboard-history?id=clip-1");
        assert_eq!(href_color.a, 1.0);

        let (label_text, label_color) =
            highlight_for_role(input, "markdownLinkUri", label_cursor..label_cursor);
        assert_eq!(label_text, "kit://clipboard-history?id=clip-1");
        assert_eq!(label_color.a, 1.0);
    }

    #[test]
    fn bare_kit_deeplinks_dim_and_reveal_without_changing_activation() {
        let input = "Review kit://clipboard-history?id=clip-2 now";
        let href_cursor = input.find("clipboard-history").unwrap();

        let (muted_text, muted_color) = highlight_for_role(input, "markdownLinkUri", 0..0);
        assert_eq!(muted_text, "kit://clipboard-history?id=clip-2");
        assert_eq!(muted_color.a, MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY);

        let (focused_text, focused_color) =
            highlight_for_role(input, "markdownLinkUri", href_cursor..href_cursor);
        assert_eq!(focused_text, "kit://clipboard-history?id=clip-2");
        assert_eq!(focused_color.a, 1.0);
        assert_eq!(
            activation_href_at_cursor_in_text(input, href_cursor),
            Some("kit://clipboard-history?id=clip-2".to_string())
        );
    }

    #[test]
    fn markdown_link_highlights_cover_reference_links() {
        let input = "[Guide][guide]\n[guide]: https://scriptkit.com/guide\n";
        assert_eq!(
            highlighted_texts(input),
            vec![
                "markdownLinkText:Guide",
                "markdownLinkReference:guide",
                "markdownLinkReference:guide",
                "markdownLinkUri:https://scriptkit.com/guide",
            ]
        );
    }

    #[test]
    fn markdown_link_highlights_skip_fenced_code() {
        let input = "```md\n[Nope](https://example.com)\n```\n[Yep](https://scriptkit.com)\n";
        assert_eq!(
            highlighted_texts(input),
            vec![
                "markdownLinkText:Yep",
                "markdownLinkUri:https://scriptkit.com",
            ]
        );
    }

    #[test]
    fn markdown_link_highlights_do_not_panic_after_non_ascii_text() {
        let input = "Script Kit memory — the Brain [Guide](https://scriptkit.com/guide)";
        assert_eq!(
            highlighted_texts(input),
            vec![
                "markdownLinkText:Guide",
                "markdownLinkUri:https://scriptkit.com/guide",
            ]
        );
    }

    #[test]
    fn activation_href_finds_inline_markdown_from_label_or_destination() {
        let input = "Open [Project Brief](scriptkit://spine/file/project-brief) today";
        let label_cursor = input.find("Project").unwrap();
        let href_cursor = input.find("spine/file").unwrap();
        assert_eq!(
            activation_href_at_cursor_in_text(input, label_cursor),
            Some("scriptkit://spine/file/project-brief".to_string())
        );
        assert_eq!(
            activation_href_at_cursor_in_text(input, href_cursor),
            Some("scriptkit://spine/file/project-brief".to_string())
        );
    }

    #[test]
    fn activation_href_finds_scriptkit_bare_uri_at_end_boundary() {
        let input = "scriptkit://run/example-script";
        assert_eq!(
            activation_href_at_cursor_in_text(input, input.len()),
            Some("scriptkit://run/example-script".to_string())
        );
    }

    #[test]
    fn activation_href_finds_kit_resource_uri() {
        let input = "Review kit://scripts now";
        assert_eq!(
            activation_href_at_cursor_in_text(input, input.find("scripts").unwrap()),
            Some("kit://scripts".to_string())
        );
    }

    #[test]
    fn activation_href_finds_local_paths() {
        let input = "Open ~/dev/script-kit-gpui/src/main.rs";
        assert_eq!(
            activation_href_at_cursor_in_text(input, input.find("main.rs").unwrap()),
            Some("~/dev/script-kit-gpui/src/main.rs".to_string())
        );
    }

    #[test]
    fn activation_href_ignores_code_fence_links() {
        let input = "```md\nscriptkit://run/nope\n```\nplain";
        assert_eq!(
            activation_href_at_cursor_in_text(input, input.find("run/nope").unwrap()),
            None
        );
    }

    #[test]
    fn activation_href_returns_none_for_plain_text() {
        assert_eq!(activation_href_at_cursor_in_text("plain text", 3), None);
    }

    #[test]
    fn mouse_deeplink_activation_requires_plain_collapsed_left_click() {
        let plain_click = gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        };
        assert!(should_activate_deeplink_from_mouse_up(&plain_click, 4..4));

        let right_click = gpui::MouseUpEvent {
            button: gpui::MouseButton::Right,
            click_count: 1,
            ..Default::default()
        };
        assert!(!should_activate_deeplink_from_mouse_up(&right_click, 4..4));

        let double_click = gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            click_count: 2,
            ..Default::default()
        };
        assert!(!should_activate_deeplink_from_mouse_up(&double_click, 4..4));

        let shifted_click = gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            click_count: 1,
            modifiers: gpui::Modifiers {
                shift: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_activate_deeplink_from_mouse_up(
            &shifted_click,
            4..4
        ));
        assert!(!should_activate_deeplink_from_mouse_up(&plain_click, 4..8));
    }
}

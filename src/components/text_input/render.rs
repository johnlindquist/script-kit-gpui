use super::TextSelection;
use crate::{
    panel::{CURSOR_HEIGHT_LG, CURSOR_WIDTH},
    ui_foundation::ALPHA_SELECTION,
};
use gpui::{
    div, px, rgb, rgba, Animation, AnimationExt, Div, ElementId, Hsla, IntoElement, ParentElement,
    Rgba, SharedString, Styled,
};
use std::time::Duration;

const CURSOR_PULSE_DURATION: Duration = Duration::from_millis(1100);
/// Fraction of each pulse cycle spent holding at an extreme: fully visible
/// at the start of the cycle, fully invisible at the midpoint.
const CURSOR_PULSE_DWELL: f32 = 0.12;

/// Ease-in-out caret pulse with a brief dwell at both extremes.
///
/// Keep in sync with the vendored copy for the gpui-component input caret in
/// vendor/gpui-component/crates/ui/src/input/blink_cursor.rs.
fn caret_pulse_alpha(phase: f32) -> f32 {
    let fade = 0.5 - CURSOR_PULSE_DWELL;
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    if phase < CURSOR_PULSE_DWELL {
        1.0
    } else if phase < 0.5 {
        1.0 - smooth((phase - CURSOR_PULSE_DWELL) / fade)
    } else if phase < 0.5 + CURSOR_PULSE_DWELL {
        0.0
    } else {
        smooth((phase - 0.5 - CURSOR_PULSE_DWELL) / fade)
    }
}

/// Applies the app-wide breathing animation to an eligible text caret.
pub(crate) fn pulse_cursor_bar(cursor_bar: Div, id: impl Into<ElementId>) -> impl IntoElement {
    cursor_bar.with_animation(
        id,
        Animation::new(CURSOR_PULSE_DURATION)
            .repeat()
            .with_easing(caret_pulse_alpha),
        |cursor_bar, opacity| cursor_bar.opacity(opacity),
    )
}

/// Zero-width caret anchor for placeholder (empty-input) states.
///
/// Mirrors `render_cursor`'s anatomy: the anchor occupies no layout space and
/// the bar is absolutely positioned, centered on the text-start boundary, so
/// caret visibility can never shift, clip, or occlude the placeholder that
/// follows it in the same row.
pub(crate) fn placeholder_cursor_anchor(
    cursor_width: f32,
    cursor_height: f32,
    color: u32,
    visible: bool,
    id: impl Into<ElementId>,
) -> Div {
    let anchor = div().relative().w(px(0.0)).h(px(cursor_height));
    if !visible {
        return anchor;
    }
    let cursor_bar = div()
        .absolute()
        .left(px(-(cursor_width / 2.0)))
        .top(px(0.0))
        .w(px(cursor_width))
        .h(px(cursor_height))
        .bg(rgb(color));
    anchor.child(pulse_cursor_bar(cursor_bar, id))
}

/// A character range to render with a specific text color.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TextHighlightRange {
    /// Start character index (inclusive).
    pub start: usize,
    /// End character index (exclusive).
    pub end: usize,
    /// Override text color for this range.
    pub color: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextInputRenderIndicator<'a> {
    pub text: &'a str,
    pub color: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct TextInlinePillRange {
    pub start: usize,
    pub end: usize,
    pub label: String,
    pub text_color: u32,
    pub background_color: u32,
    pub border_color: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextInputRenderConfig<'a> {
    pub text: &'a str,
    pub cursor: usize,
    pub selection: Option<TextSelection>,
    pub window: Option<(usize, usize)>,
    pub multiline: bool,
    pub cursor_visible: bool,
    pub cursor_width: f32,
    pub cursor_height: f32,
    pub cursor_margin_y: f32,
    pub cursor_gap_before: f32,
    pub cursor_gap_after: f32,
    pub cursor_color: u32,
    pub cursor_hidden_color: Option<Hsla>,
    pub text_color: u32,
    pub selection_color: u32,
    pub selection_alpha: u32,
    pub selection_text_color: u32,
    pub container_height: Option<f32>,
    pub overflow_x_hidden: bool,
    pub leading_indicator: Option<TextInputRenderIndicator<'a>>,
    pub trailing_indicator: Option<TextInputRenderIndicator<'a>>,
    pub transform: Option<fn(&str) -> String>,
    /// Character ranges to render with a specific text color (e.g. gold @mentions).
    /// Ranges are in terms of character indices in the full text.
    pub highlight_ranges: &'a [TextHighlightRange],
    /// Inline pill ranges rendered in place of the underlying token text.
    pub pill_ranges: &'a [TextInlinePillRange],
}

/// Render-only search text for surfaces whose keyboard state is owned by a
/// parent controller rather than a `TextInputState` entity.
///
/// ActionsDialog uses this contract because the host routes key events into
/// the dialog directly. Keeping the display/cursor anatomy here still gives
/// those controller-owned fields the shared text-input implementation.
#[derive(Clone, Debug)]
pub(crate) struct CompactSearchTextConfig<'a> {
    pub display: SharedString,
    pub is_placeholder: bool,
    pub prefix_marker: Option<&'a str>,
    pub prefix_gap: f32,
    pub mono_font: bool,
    pub font_size: f32,
    pub inner_height: f32,
    pub cursor_width: f32,
    pub cursor_height: f32,
    pub cursor_visible: bool,
    pub cursor_color: Rgba,
    pub placeholder_color: Rgba,
    pub text_color: Rgba,
}

pub(crate) fn render_compact_search_text(config: CompactSearchTextConfig<'_>) -> Div {
    let mut content = div()
        .flex_1()
        .h(px(config.inner_height))
        .flex()
        .flex_row()
        .items_center()
        .text_size(px(config.font_size))
        .line_height(px(config.font_size))
        .text_color(if config.is_placeholder {
            config.placeholder_color
        } else {
            config.text_color
        });

    if let Some(prefix_marker) = config.prefix_marker {
        content = content.child(
            div()
                .mr(px(config.prefix_gap))
                .text_color(config.placeholder_color)
                .font_family(if config.mono_font {
                    crate::list_item::FONT_MONO
                } else {
                    crate::list_item::FONT_SYSTEM_UI
                })
                .child(prefix_marker.to_string()),
        );
    }

    if config.is_placeholder {
        content = content.child(render_compact_search_cursor(&config));
    }

    content = content.child(config.display.clone());

    if !config.is_placeholder {
        content = content.child(render_compact_search_cursor(&config));
    }

    content
}

fn render_compact_search_cursor(config: &CompactSearchTextConfig<'_>) -> Div {
    let cursor_bar = div()
        .absolute()
        .left(px(-(config.cursor_width / 2.0)))
        .top(px(0.0))
        .w(px(config.cursor_width))
        .h(px(config.cursor_height))
        .rounded(px(config.cursor_width / 2.0));
    let cursor = div().relative().w(px(0.0)).h(px(config.cursor_height));
    if config.cursor_visible {
        cursor.child(pulse_cursor_bar(
            cursor_bar.bg(config.cursor_color),
            "compact-search-cursor-pulse",
        ))
    } else {
        cursor.child(cursor_bar)
    }
}

impl<'a> TextInputRenderConfig<'a> {
    pub(crate) fn default_for_prompt(text: &'a str) -> Self {
        Self {
            text,
            cursor: 0,
            selection: None,
            window: None,
            multiline: false,
            cursor_visible: true,
            cursor_width: CURSOR_WIDTH,
            cursor_height: CURSOR_HEIGHT_LG,
            cursor_margin_y: 0.0,
            cursor_gap_before: 0.0,
            cursor_gap_after: 0.0,
            cursor_color: 0xffffff,
            cursor_hidden_color: None,
            text_color: 0xffffff,
            selection_color: 0x0000ff,
            selection_alpha: u32::from(ALPHA_SELECTION),
            selection_text_color: 0xffffff,
            container_height: None,
            overflow_x_hidden: false,
            leading_indicator: None,
            trailing_indicator: None,
            transform: None,
            highlight_ranges: &[],
            pill_ranges: &[],
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ComputedTextInputSegments {
    before: String,
    selected: String,
    after: String,
    /// Character offset of `before` in the full text.
    before_char_offset: usize,
    /// Character offset of `after` in the full text.
    after_char_offset: usize,
    show_cursor: bool,
    show_leading_indicator: bool,
    show_trailing_indicator: bool,
}

pub(crate) fn render_text_input_cursor_selection(config: TextInputRenderConfig<'_>) -> Div {
    if config.multiline {
        return render_multiline_text_input_cursor_selection(&config);
    }

    if selection_is_empty(config.selection) && !config.pill_ranges.is_empty() {
        return render_single_line_text_input_with_pills(&config);
    }

    let segments = compute_text_input_segments(&config);
    let mut content = div()
        .flex()
        .flex_row()
        .items_center()
        .whitespace_nowrap()
        .text_color(rgb(config.text_color));

    if let Some(height) = config.container_height {
        content = content.h(px(height));
    }
    if config.overflow_x_hidden {
        content = content.overflow_x_hidden();
    }
    if segments.show_leading_indicator {
        if let Some(indicator) = config.leading_indicator {
            content = content.child(
                div()
                    .whitespace_nowrap()
                    .text_color(rgb(indicator.color))
                    .child(indicator.text.to_string()),
            );
        }
    }
    let has_highlights = !config.highlight_ranges.is_empty();
    if !segments.before.is_empty() {
        if has_highlights {
            content = content.child(render_segment_with_highlights(
                &segments.before,
                segments.before_char_offset,
                config.highlight_ranges,
                config.text_color,
                config.transform,
            ));
        } else {
            content = content.child(div().whitespace_nowrap().child(format_single_line_segment(
                &segments.before,
                config.transform,
            )));
        }
    }
    if segments.show_cursor {
        if config.cursor_gap_before > 0.0 {
            content = content.child(div().w(px(config.cursor_gap_before)));
        }
        content = content.child(render_cursor(&config));
        if config.cursor_gap_after > 0.0 {
            content = content.child(div().w(px(config.cursor_gap_after)));
        }
    } else if !segments.selected.is_empty() {
        content = content.child(
            div()
                .whitespace_nowrap()
                .bg(rgba((config.selection_color << 8) | config.selection_alpha))
                .text_color(rgb(config.selection_text_color))
                .child(format_single_line_segment(
                    &segments.selected,
                    config.transform,
                )),
        );
    }
    if !segments.after.is_empty() {
        if has_highlights {
            content = content.child(render_segment_with_highlights(
                &segments.after,
                segments.after_char_offset,
                config.highlight_ranges,
                config.text_color,
                config.transform,
            ));
        } else {
            content = content.child(div().whitespace_nowrap().child(format_single_line_segment(
                &segments.after,
                config.transform,
            )));
        }
    }
    if segments.show_trailing_indicator {
        if let Some(indicator) = config.trailing_indicator {
            content = content.child(
                div()
                    .whitespace_nowrap()
                    .text_color(rgb(indicator.color))
                    .child(indicator.text.to_string()),
            );
        }
    }

    content
}

fn render_single_line_text_input_with_pills(config: &TextInputRenderConfig<'_>) -> Div {
    let text_len = config.text.chars().count();
    let (window_start, window_end) = clamped_window(config.window, text_len);
    let cursor = config.cursor.min(text_len);
    let mut pills: Vec<TextInlinePillRange> = config
        .pill_ranges
        .iter()
        .filter(|&pill| pill.start >= window_start && pill.end <= window_end)
        .cloned()
        .collect();
    pills.sort_by_key(|pill| pill.start);

    let mut content = div()
        .flex()
        .flex_row()
        .items_center()
        .whitespace_nowrap()
        .text_color(rgb(config.text_color));

    if let Some(height) = config.container_height {
        content = content.h(px(height));
    }
    if config.overflow_x_hidden {
        content = content.overflow_x_hidden();
    }

    if window_start > 0 {
        if let Some(indicator) = config.leading_indicator {
            content = content.child(
                div()
                    .whitespace_nowrap()
                    .text_color(rgb(indicator.color))
                    .child(indicator.text.to_string()),
            );
        }
    }

    let mut pos = window_start;
    while pos < window_end {
        if pos == cursor {
            content = content.child(render_cursor(config));
        }

        if let Some(pill) = pills.iter().find(|pill| pill.start == pos) {
            content = content.child(render_inline_pill(pill, config));
            pos = pill.end;
            if cursor > pill.start && cursor < pill.end {
                content = content.child(render_cursor(config));
            }
            continue;
        }

        let next_pill_start = pills
            .iter()
            .find(|pill| pill.start > pos)
            .map(|pill| pill.start)
            .unwrap_or(window_end);
        let next_stop = next_pill_start.min(window_end);
        let segment = slice_chars(config.text, pos, next_stop);
        if !segment.is_empty() {
            content = content.child(render_segment_with_highlights(
                &segment,
                pos,
                config.highlight_ranges,
                config.text_color,
                config.transform,
            ));
        }
        pos = next_stop;
    }

    if cursor == window_end {
        content = content.child(render_cursor(config));
    }

    if window_end < text_len {
        if let Some(indicator) = config.trailing_indicator {
            content = content.child(
                div()
                    .whitespace_nowrap()
                    .text_color(rgb(indicator.color))
                    .child(indicator.text.to_string()),
            );
        }
    }

    content
}

fn render_multiline_text_input_cursor_selection(config: &TextInputRenderConfig<'_>) -> Div {
    let chars: Vec<char> = config.text.chars().collect();
    let text_len = chars.len();
    let cursor = config.cursor.min(text_len);
    let selection_range = config.selection.and_then(|selection| {
        let (start, end) = selection.range();
        let start = start.min(text_len);
        let end = end.min(text_len);
        (start < end).then_some((start, end))
    });
    let line_height = config
        .container_height
        .unwrap_or(config.cursor_height + (config.cursor_margin_y * 2.0))
        .max(config.cursor_height);

    let mut content = div()
        .flex()
        .flex_col()
        .items_start()
        .w_full()
        .min_h(px(line_height))
        .text_color(rgb(config.text_color))
        .line_height(px(line_height));

    let mut line = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .w_full()
        .min_h(px(line_height))
        .line_height(px(line_height));

    let mut line_start = 0;
    while line_start < text_len {
        let mut line_end = line_start;
        while line_end < text_len && chars[line_end] != '\n' {
            line_end += 1;
        }

        line = render_multiline_line(
            line,
            &chars[line_start..line_end],
            line_start,
            cursor,
            selection_range,
            config,
        );

        if line_end == text_len {
            break;
        }

        if selection_range.is_none() && cursor == line_end {
            line = line.child(render_cursor(config));
        }

        content = content.child(line);
        line = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .w_full()
            .min_h(px(line_height))
            .line_height(px(line_height));
        line_start = line_end + 1;
    }

    if line_start == text_len {
        if selection_range.is_none() && cursor == text_len {
            line = line.child(render_cursor(config));
        }
        return content.child(line);
    }

    if selection_range.is_none() && cursor == text_len {
        line = line.child(render_cursor(config));
    }

    content.child(line)
}

fn render_multiline_line(
    mut line: Div,
    line_chars: &[char],
    line_start: usize,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    config: &TextInputRenderConfig<'_>,
) -> Div {
    if selection_range.is_none() && !config.pill_ranges.is_empty() {
        return render_multiline_line_with_pills(line, line_chars, line_start, cursor, config);
    }

    for token in tokenize_multiline_line(line_chars, line_start) {
        if token.can_wrap_inside {
            for (offset, ch) in token.text.chars().enumerate() {
                let char_index = token.start + offset;
                if selection_range.is_none() && char_index == cursor {
                    line = line.child(render_cursor(config));
                }

                line = line.child(render_multiline_char(
                    ch,
                    char_index,
                    selection_range,
                    config,
                ));
            }
            continue;
        }

        line = line.child(render_multiline_token_runs(
            &token,
            cursor,
            selection_range,
            config,
        ));
    }

    line
}

fn render_multiline_line_with_pills(
    mut line: Div,
    line_chars: &[char],
    line_start: usize,
    cursor: usize,
    config: &TextInputRenderConfig<'_>,
) -> Div {
    let line_end = line_start + line_chars.len();
    let mut pills: Vec<TextInlinePillRange> = config
        .pill_ranges
        .iter()
        .filter(|&pill| pill.start >= line_start && pill.end <= line_end)
        .cloned()
        .collect();
    pills.sort_by_key(|pill| pill.start);

    let mut pos = line_start;
    while pos < line_end {
        if let Some(pill) = pills.iter().find(|pill| pill.start == pos) {
            if cursor == pos {
                line = line.child(render_cursor(config));
            }
            line = line.child(render_inline_pill(pill, config));
            if cursor > pill.start && cursor < pill.end {
                line = line.child(render_cursor(config));
            }
            pos = pill.end;
            continue;
        }

        let next_pill_start = pills
            .iter()
            .find(|pill| pill.start > pos)
            .map(|pill| pill.start)
            .unwrap_or(line_end);
        let chunk_end = next_pill_start.min(line_end);
        let chunk_chars = &line_chars[(pos - line_start)..(chunk_end - line_start)];
        for token in tokenize_multiline_line(chunk_chars, pos) {
            if token.can_wrap_inside {
                for (offset, ch) in token.text.chars().enumerate() {
                    let char_index = token.start + offset;
                    if char_index == cursor {
                        line = line.child(render_cursor(config));
                    }

                    line = line.child(render_multiline_char(ch, char_index, None, config));
                }
            } else {
                line = line.child(render_multiline_token_runs(&token, cursor, None, config));
            }
        }
        pos = chunk_end;
    }

    line
}

#[derive(Debug, PartialEq, Eq)]
struct MultilineToken {
    text: String,
    start: usize,
    end: usize,
    can_wrap_inside: bool,
}

fn tokenize_multiline_line(chars: &[char], line_start: usize) -> Vec<MultilineToken> {
    const LONG_WORD_WRAP_THRESHOLD: usize = 24;

    let mut tokens = Vec::new();
    let mut idx = 0;
    while idx < chars.len() {
        let token_start = idx;
        let is_whitespace = chars[idx].is_whitespace();
        while idx < chars.len() && chars[idx].is_whitespace() == is_whitespace {
            idx += 1;
        }

        let text: String = chars[token_start..idx].iter().collect();
        let len = text.chars().count();
        tokens.push(MultilineToken {
            text,
            start: line_start + token_start,
            end: line_start + idx,
            can_wrap_inside: !is_whitespace && len > LONG_WORD_WRAP_THRESHOLD,
        });
    }

    tokens
}

fn render_multiline_char(
    ch: char,
    char_index: usize,
    selection_range: Option<(usize, usize)>,
    config: &TextInputRenderConfig<'_>,
) -> Div {
    let color = color_for_char(config.highlight_ranges, char_index).unwrap_or(config.text_color);
    let text = format_segment(&ch.to_string(), config.transform);
    let mut node = div().text_color(rgb(color)).child(text);

    if let Some((start, end)) = selection_range {
        if (start..end).contains(&char_index) {
            node = node
                .bg(rgba((config.selection_color << 8) | config.selection_alpha))
                .text_color(rgb(config.selection_text_color));
        }
    }

    node
}

fn render_multiline_token_runs(
    token: &MultilineToken,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    config: &TextInputRenderConfig<'_>,
) -> Div {
    let mut token_node = div().flex().flex_row().items_center();
    let mut run_text = String::new();
    let mut run_selected = false;
    let mut run_color = config.text_color;

    for (offset, ch) in token.text.chars().enumerate() {
        let char_index = token.start + offset;
        if selection_range.is_none() && char_index == cursor {
            token_node =
                flush_multiline_run(token_node, &mut run_text, run_selected, run_color, config);
            token_node = token_node.child(render_cursor(config));
        }

        let is_selected =
            selection_range.is_some_and(|(start, end)| (start..end).contains(&char_index));
        let color =
            color_for_char(config.highlight_ranges, char_index).unwrap_or(config.text_color);

        if run_text.is_empty() {
            run_selected = is_selected;
            run_color = color;
        } else if run_selected != is_selected || (!is_selected && run_color != color) {
            token_node =
                flush_multiline_run(token_node, &mut run_text, run_selected, run_color, config);
            run_selected = is_selected;
            run_color = color;
        }

        run_text.push(ch);
    }

    flush_multiline_run(token_node, &mut run_text, run_selected, run_color, config)
}

fn flush_multiline_run(
    node: Div,
    run_text: &mut String,
    selected: bool,
    color: u32,
    config: &TextInputRenderConfig<'_>,
) -> Div {
    if run_text.is_empty() {
        return node;
    }

    let text = format_segment(run_text, config.transform);
    run_text.clear();

    if selected {
        node.child(
            div()
                .bg(rgba((config.selection_color << 8) | config.selection_alpha))
                .text_color(rgb(config.selection_text_color))
                .child(text),
        )
    } else {
        node.child(div().text_color(rgb(color)).child(text))
    }
}

fn color_for_char(highlights: &[TextHighlightRange], char_index: usize) -> Option<u32> {
    highlights
        .iter()
        .find(|range| (range.start..range.end).contains(&char_index))
        .map(|range| range.color)
}

fn selection_is_empty(selection: Option<TextSelection>) -> bool {
    selection.is_none_or(|selection| selection.is_empty())
}

fn render_inline_pill(pill: &TextInlinePillRange, config: &TextInputRenderConfig<'_>) -> Div {
    let mut node = div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(config.cursor_height))
        .px(px(6.0))
        .line_height(px(config.cursor_height))
        .rounded(px(6.0))
        .border_1()
        .border_color(rgb(pill.border_color))
        .bg(rgb(pill.background_color))
        .text_color(rgb(pill.text_color));

    if config.cursor_margin_y > 0.0 {
        node = node.my(px(config.cursor_margin_y));
    }

    node.child(pill.label.clone())
}

fn render_cursor(config: &TextInputRenderConfig<'_>) -> Div {
    let mut cursor = div().relative().w(px(0.0)).h(px(config.cursor_height));
    if config.cursor_margin_y > 0.0 {
        cursor = cursor.my(px(config.cursor_margin_y));
    }
    let mut cursor_bar = div()
        .absolute()
        .left(px(-(config.cursor_width / 2.0)))
        .top(px(0.0))
        .w(px(config.cursor_width))
        .h(px(config.cursor_height));
    if let Some(hidden_color) = config.cursor_hidden_color {
        cursor_bar = cursor_bar.bg(hidden_color);
    }
    let cursor_is_visible = config.cursor_visible;
    if cursor_is_visible {
        cursor_bar = cursor_bar.bg(rgb(config.cursor_color));
    }
    // The caret is where the app's attention is while typing: report its
    // painted position so the background shader effect can react to it
    // (main-window only; animation re-paints dedupe to no-ops).
    cursor_bar = cursor_bar.child(crate::effects::effect_focus_probe(
        crate::effects::EffectFocusSource::TextCursor,
    ));
    if cursor_is_visible {
        cursor.child(pulse_cursor_bar(cursor_bar, "text-input-cursor-pulse"))
    } else {
        cursor.child(cursor_bar)
    }
}

fn format_segment(segment: &str, transform: Option<fn(&str) -> String>) -> String {
    match transform {
        Some(transform_fn) => transform_fn(segment),
        None => segment.to_string(),
    }
}

fn format_single_line_segment(segment: &str, transform: Option<fn(&str) -> String>) -> String {
    format_segment(segment, transform).replace(' ', "\u{00A0}")
}

fn slice_chars(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

/// Render a text segment, splitting it into sub-spans where highlight ranges
/// overlap. Characters outside any highlight range use `default_color`.
fn render_segment_with_highlights(
    segment: &str,
    segment_char_offset: usize,
    highlights: &[TextHighlightRange],
    default_color: u32,
    transform: Option<fn(&str) -> String>,
) -> gpui::AnyElement {
    if segment.is_empty() {
        return div().into_any_element();
    }

    let seg_chars: Vec<char> = segment.chars().collect();
    let seg_len = seg_chars.len();

    // Build a color map for each character in this segment.
    let mut colors: Vec<u32> = vec![default_color; seg_len];
    for hl in highlights {
        if hl.end <= segment_char_offset || hl.start >= segment_char_offset + seg_len {
            continue;
        }
        let local_start = hl.start.saturating_sub(segment_char_offset);
        let local_end = (hl.end - segment_char_offset).min(seg_len);
        for c in &mut colors[local_start..local_end] {
            *c = hl.color;
        }
    }

    // Group consecutive characters with the same color into runs.
    let mut spans: Vec<(String, u32)> = Vec::new();
    let mut run_start = 0;
    while run_start < seg_len {
        let run_color = colors[run_start];
        let mut run_end = run_start + 1;
        while run_end < seg_len && colors[run_end] == run_color {
            run_end += 1;
        }
        let text: String = seg_chars[run_start..run_end].iter().collect();
        spans.push((text, run_color));
        run_start = run_end;
    }

    if spans.len() == 1 {
        let (text, color) = &spans[0];
        return div()
            .whitespace_nowrap()
            .text_color(rgb(*color))
            .child(format_single_line_segment(text, transform))
            .into_any_element();
    }

    let mut container = div().flex().flex_row().whitespace_nowrap();
    for (text, color) in &spans {
        container = container.child(
            div()
                .whitespace_nowrap()
                .text_color(rgb(*color))
                .child(format_single_line_segment(text, transform)),
        );
    }
    container.into_any_element()
}

fn compute_text_input_segments(config: &TextInputRenderConfig<'_>) -> ComputedTextInputSegments {
    let chars: Vec<char> = config.text.chars().collect();
    let text_len = chars.len();
    let (window_start, window_end) = clamped_window(config.window, text_len);
    let visible_chars = &chars[window_start..window_end];
    let local_cursor = config
        .cursor
        .min(window_end)
        .saturating_sub(window_start)
        .min(visible_chars.len());

    let local_selection = config.selection.and_then(|selection| {
        let (selection_start, selection_end) = selection.range();
        let selection_start = selection_start.min(text_len);
        let selection_end = selection_end.min(text_len);
        if selection_start >= selection_end {
            return None;
        }

        let visible_selection_start = selection_start.clamp(window_start, window_end);
        let visible_selection_end = selection_end.clamp(window_start, window_end);
        if visible_selection_start >= visible_selection_end {
            return None;
        }

        Some((
            visible_selection_start - window_start,
            visible_selection_end - window_start,
        ))
    });

    let (before, selected, after, show_cursor, before_char_offset, after_char_offset) =
        if let Some((selection_start, selection_end)) = local_selection {
            (
                visible_chars[..selection_start].iter().collect(),
                visible_chars[selection_start..selection_end]
                    .iter()
                    .collect(),
                visible_chars[selection_end..].iter().collect(),
                false,
                window_start,
                window_start + selection_end,
            )
        } else {
            (
                visible_chars[..local_cursor].iter().collect(),
                String::new(),
                visible_chars[local_cursor..].iter().collect(),
                true,
                window_start,
                window_start + local_cursor,
            )
        };

    ComputedTextInputSegments {
        before,
        selected,
        after,
        before_char_offset,
        after_char_offset,
        show_cursor,
        show_leading_indicator: window_start > 0,
        show_trailing_indicator: window_end < text_len,
    }
}

fn clamped_window(window: Option<(usize, usize)>, text_len: usize) -> (usize, usize) {
    match window {
        Some((start, end)) => {
            let mut start = start.min(text_len);
            let mut end = end.min(text_len);
            if end < start {
                std::mem::swap(&mut start, &mut end);
            }
            (start, end)
        }
        None => (0, text_len),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        color_for_char, compute_text_input_segments, render_multiline_text_input_cursor_selection,
        tokenize_multiline_line, MultilineToken, TextHighlightRange, TextInputRenderConfig,
    };
    use crate::components::TextSelection;

    fn render_config_for(text: &str) -> TextInputRenderConfig<'_> {
        TextInputRenderConfig {
            ..TextInputRenderConfig::default_for_prompt(text)
        }
    }

    #[test]
    fn test_default_for_prompt_sets_canonical_cursor_height_and_selection_alpha() {
        let config = TextInputRenderConfig::default_for_prompt("hello");

        assert_eq!(config.cursor_height, crate::panel::CURSOR_HEIGHT_LG);
        assert_eq!(
            config.selection_alpha,
            u32::from(crate::ui_foundation::ALPHA_SELECTION)
        );
    }

    #[test]
    fn test_compute_text_input_segments_clamps_indices_when_selection_exceeds_text_bounds() {
        let mut config = render_config_for("hello");
        config.cursor = 99;
        config.selection = Some(TextSelection {
            anchor: 1,
            cursor: 99,
        });

        let segments = compute_text_input_segments(&config);

        assert_eq!(segments.before, "h");
        assert_eq!(segments.selected, "ello");
        assert_eq!(segments.after, "");
        assert!(!segments.show_cursor);
    }

    #[test]
    fn test_compute_text_input_segments_windows_visible_selection_when_range_is_truncated() {
        let mut config = render_config_for("abcdefghijklmnopqrstuvwxyz");
        config.window = Some((10, 18));
        config.selection = Some(TextSelection {
            anchor: 5,
            cursor: 20,
        });

        let segments = compute_text_input_segments(&config);

        assert_eq!(segments.before, "");
        assert_eq!(segments.selected, "klmnopqr");
        assert_eq!(segments.after, "");
        assert!(!segments.show_cursor);
        assert!(segments.show_leading_indicator);
        assert!(segments.show_trailing_indicator);
    }

    #[test]
    fn test_compute_text_input_segments_splits_before_and_after_when_cursor_has_no_selection() {
        let mut config = render_config_for("abcdef");
        config.cursor = 3;

        let segments = compute_text_input_segments(&config);

        assert_eq!(segments.before, "abc");
        assert_eq!(segments.selected, "");
        assert_eq!(segments.after, "def");
        assert!(segments.show_cursor);
    }

    #[test]
    fn test_compute_text_input_segments_swaps_window_bounds_when_range_is_reversed() {
        let mut config = render_config_for("abcdefghij");
        config.window = Some((8, 2));
        config.cursor = 5;

        let segments = compute_text_input_segments(&config);

        assert_eq!(segments.before, "cde");
        assert_eq!(segments.after, "fgh");
        assert!(segments.show_leading_indicator);
        assert!(segments.show_trailing_indicator);
    }

    #[test]
    fn test_default_for_prompt_is_single_line_by_default() {
        let config = TextInputRenderConfig::default_for_prompt("hello");

        assert!(!config.multiline);
    }

    #[test]
    fn test_color_for_char_returns_matching_highlight_color() {
        let highlights = [TextHighlightRange {
            start: 2,
            end: 4,
            color: 0xFBBF24,
        }];

        assert_eq!(color_for_char(&highlights, 1), None);
        assert_eq!(color_for_char(&highlights, 2), Some(0xFBBF24));
        assert_eq!(color_for_char(&highlights, 3), Some(0xFBBF24));
        assert_eq!(color_for_char(&highlights, 4), None);
    }

    #[test]
    fn test_multiline_renderer_accepts_newlines() {
        let mut config = render_config_for("hello\nworld");
        config.multiline = true;
        config.cursor = 5;

        let _ = render_multiline_text_input_cursor_selection(&config);
    }

    #[test]
    fn test_multiline_line_rows_center_cursor_with_text() {
        let source = include_str!("render.rs");
        let body = source
            .split("fn render_multiline_text_input_cursor_selection")
            .nth(1)
            .expect("multiline renderer");
        let body = body.split("\nfn ").next().expect("multiline renderer body");

        assert!(
            !body.contains(".flex_wrap()\n        .items_start()")
                && !body.contains(".flex_wrap()\n            .items_start()"),
            "wrapped line rows must not top-align the caret while glyphs sit in the line-height box"
        );
        assert!(
            body.contains(".flex_wrap()\n        .items_center()")
                || body.contains(".flex_wrap()\n            .items_center()"),
            "wrapped line rows should vertically center the caret with glyphs"
        );
    }

    #[test]
    fn test_tokenize_multiline_line_preserves_words_and_spaces() {
        let chars: Vec<char> = "hello   world".chars().collect();

        let tokens = tokenize_multiline_line(&chars, 10);

        assert_eq!(
            tokens,
            vec![
                MultilineToken {
                    text: "hello".to_string(),
                    start: 10,
                    end: 15,
                    can_wrap_inside: false,
                },
                MultilineToken {
                    text: "   ".to_string(),
                    start: 15,
                    end: 18,
                    can_wrap_inside: false,
                },
                MultilineToken {
                    text: "world".to_string(),
                    start: 18,
                    end: 23,
                    can_wrap_inside: false,
                },
            ]
        );
    }

    #[test]
    fn test_tokenize_multiline_line_marks_long_words_for_fallback_wrapping() {
        let chars: Vec<char> = "supercalifragilisticexpialidocious".chars().collect();

        let tokens = tokenize_multiline_line(&chars, 0);

        assert_eq!(tokens.len(), 1);
        assert!(tokens[0].can_wrap_inside);
    }
}

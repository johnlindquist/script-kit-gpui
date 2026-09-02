use super::*;
use crate::list_item::FONT_MONO;

fn style_span(
    text: &str,
    style: &InlineStyle,
    colors: &PromptColors,
    link_url: Option<&str>,
    link_element_id: Option<SharedString>,
) -> AnyElement {
    if style.code {
        return div()
            .px(px(4.0))
            .py(px(1.0))
            .bg(rgba((colors.code_bg << 8) | 0x80))
            .rounded(px(3.0))
            .font_family(FONT_MONO)
            .text_color(rgb(colors.text_primary))
            .child(text.to_string())
            .into_any_element();
    }

    // Clickable link with URL — needs .id() for interactivity
    if style.link {
        if let Some(url) = link_url {
            let fallback_id = NEXT_LINK_ID.fetch_add(1, Ordering::Relaxed);
            let id = link_element_id
                .unwrap_or_else(|| SharedString::from(format!("md-link-{fallback_id}")));
            let url_owned = url.to_string();
            let accent = rgb(colors.accent_color);
            if is_allowed_markdown_url(url) {
                return div()
                    .id(id.clone())
                    .text_color(accent)
                    .border_b_1()
                    .border_color(rgba((colors.accent_color << 8) | 0x40))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.7))
                    .on_click(move |_, _window, _cx| {
                        if let Err(error) = crate::runtime_policy::check(
                            crate::runtime_policy::ExternalEffect::OpenExternal,
                        ) {
                            crate::logging::log(
                                "MARKDOWN",
                                &format!("Failed to open link: {error}"),
                            );
                            return;
                        }
                        if let Err(error) = open::that(&url_owned) {
                            crate::logging::log(
                                "MARKDOWN",
                                &format!("Failed to open link: {error}"),
                            );
                        }
                    })
                    .when(style.bold, |d| d.font_weight(FontWeight::BOLD))
                    .when(style.italic, |d| d.italic())
                    .child(text.to_string())
                    .into_any_element();
            }

            return div()
                .id(id)
                .text_color(accent)
                .border_b_1()
                .border_color(rgba((colors.accent_color << 8) | 0x40))
                .opacity(0.7)
                .when(style.bold, |d| d.font_weight(FontWeight::BOLD))
                .when(style.italic, |d| d.italic())
                .child(text.to_string())
                .into_any_element();
        }
        // Link without URL (fallback — just styled text)
        let mut piece = div()
            .text_color(rgb(colors.accent_color))
            .child(text.to_string());
        if style.bold {
            piece = piece.font_weight(FontWeight::BOLD);
        }
        if style.italic {
            piece = piece.italic();
        }
        return piece.into_any_element();
    }

    let mut piece = div()
        .text_color(rgb(colors.text_primary))
        .child(text.to_string());
    if style.bold {
        piece = piece.font_weight(FontWeight::BOLD);
    }
    if style.italic {
        piece = piece.italic();
    }
    if style.strikethrough {
        // Simulate strikethrough with muted color + line-through border trick
        piece = piece.text_color(rgb(colors.text_tertiary));
    }
    piece.into_any_element()
}

pub(super) fn render_inline_spans(
    spans: &[InlineSpan],
    colors: &PromptColors,
    render_scope_hash: u64,
    block_index: usize,
    span_group_index: usize,
) -> gpui::Div {
    // Fast path: single plain-text span — render as simple text child.
    // Avoids flex_wrap entirely so text wraps naturally at word boundaries.
    if spans.len() == 1 && spans[0].style == InlineStyle::default() && spans[0].link_url.is_none() {
        return div()
            .text_sm()
            .text_color(rgb(colors.text_primary))
            .child(spans[0].text.clone());
    }

    // Mixed styles: split non-code text into word-level flex children.
    // Each word is a separate flex item so flex_wrap breaks at word boundaries
    // instead of character boundaries (which happens when a long text span has
    // min_w_0 allowing it to shrink to width 0).
    let mut row = div().flex().flex_row().flex_wrap().min_w_0().text_sm();
    let link_kind = format!("link-{}", span_group_index);
    let mut link_index: usize = 0;

    for span in spans {
        let url = span.link_url.as_deref();
        if span.style.code {
            // Code spans stay as single units (they have bg/padding)
            row = row.child(style_span(&span.text, &span.style, colors, url, None));
        } else if span.style.link && url.is_some() {
            // Link spans: keep as a single unit so the underline is continuous
            let current_link = link_index;
            link_index += 1;
            row = row.child(style_span(
                &span.text,
                &span.style,
                colors,
                url,
                Some(scoped_markdown_element_id(
                    render_scope_hash,
                    link_kind.as_str(),
                    block_index,
                    current_link,
                )),
            ));
        } else {
            // Split text at whitespace for natural word wrapping while preserving hard breaks.
            for line_segment in span.text.split_inclusive('\n') {
                let has_break = line_segment.ends_with('\n');
                let segment = line_segment.strip_suffix('\n').unwrap_or(line_segment);
                for word in segment.split_inclusive(char::is_whitespace) {
                    if !word.is_empty() {
                        row = row.child(style_span(word, &span.style, colors, url, None));
                    }
                }
                if has_break {
                    row = row.child(div().w_full().h(px(0.0)));
                }
            }
        }
    }

    row
}

pub(super) fn render_hr(colors: &PromptColors) -> gpui::Div {
    div()
        .w_full()
        .h(px(1.0))
        .my(px(8.0))
        .bg(rgb(colors.hr_color))
}

/// Render a code block (kept for backward compatibility; new path uses
/// `build_code_block_element` with pre-highlighted lines).
#[allow(dead_code)]
fn render_code_block(code: &str, lang: Option<&str>, colors: &PromptColors) -> gpui::Div {
    let lang_label = lang.unwrap_or("").trim();
    let lines: Vec<CodeLine> = highlight_code_lines(code, lang, colors.is_dark);

    let mut code_container = div()
        .w_full()
        .mt(px(4.0))
        .mb(px(4.0))
        .rounded(px(6.0))
        .bg(rgba((colors.code_bg << 8) | 0xE0))
        .border_1()
        .border_color(rgba((colors.quote_border << 8) | 0x40))
        .flex()
        .flex_col()
        .overflow_hidden();

    if !lang_label.is_empty() {
        code_container = code_container.child(
            div()
                .w_full()
                .px(px(10.0))
                .py(px(4.0))
                .border_b_1()
                .border_color(rgba((colors.quote_border << 8) | 0x30))
                .text_xs()
                .text_color(rgb(colors.text_tertiary))
                .child(lang_label.to_string()),
        );
    }

    let mut body = div()
        .w_full()
        .px(px(10.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(2.0));

    for line in lines {
        let mut line_div = div()
            .flex()
            .flex_row()
            .w_full()
            .font_family(FONT_MONO)
            .text_sm()
            .min_h(px(16.0));

        if line.spans.is_empty() {
            line_div = line_div.child(" ");
        } else {
            for CodeSpan { text, color } in line.spans {
                line_div = line_div.child(div().text_color(rgb(color)).child(text));
            }
        }

        body = body.child(line_div);
    }

    code_container.child(body)
}

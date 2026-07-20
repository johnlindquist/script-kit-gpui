use gpui::SharedString;
use markdown::{
    ParseOptions,
    mdast::{self, Node},
};

use crate::{
    highlighter::HighlightTheme,
    text::{
        MarkdownLinkLabelPolicy,
        document::ParsedDocument,
        node::{
            self, BlockNode, CodeBlock, ImageNode, InlineNode, LinkMark, NodeContext, Paragraph,
            Span, Table, TableRow, TextMark,
        },
    },
};

const COMPACT_BARE_URL_MIN_CHARS: usize = 80;
const COMPACT_LINK_LABEL_MAX_CHARS: usize = 64;

fn compact_bare_http_label(
    source: &str,
    position: Option<&markdown::unist::Position>,
    link: &mdast::Link,
    policy: MarkdownLinkLabelPolicy,
) -> Option<(usize, String)> {
    if policy != MarkdownLinkLabelPolicy::CompactLongBareHttp {
        return None;
    }

    let position = position?;
    let raw = source.get(position.start.offset..position.end.offset)?;
    let has_http_scheme = raw
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || raw
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"));
    if !has_http_scheme || raw.chars().count() < COMPACT_BARE_URL_MIN_CHARS {
        return None;
    }

    let is_plain_url_child = matches!(
        link.children.as_slice(),
        [Node::Text(text)] if text.value == raw || text.value == link.url
    );
    if !is_plain_url_child {
        return None;
    }

    compact_http_authority_label(raw).map(|label| (position.start.offset, label))
}

fn compact_http_authority_label(raw: &str) -> Option<String> {
    let (_, after_scheme) = raw.split_once("://")?;
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    let host_port = authority.rsplit('@').next()?;
    if host_port.is_empty()
        || host_port
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\')
    {
        return None;
    }

    let host_port = if host_port
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("www."))
    {
        &host_port[4..]
    } else {
        host_port
    };
    if host_port.is_empty() {
        return None;
    }

    let mut label = host_port.to_string();
    if authority_end < after_scheme.len() {
        label.push_str("/…");
    }
    if label.chars().count() > COMPACT_LINK_LABEL_MAX_CHARS {
        label = label
            .chars()
            .take(COMPACT_LINK_LABEL_MAX_CHARS - 1)
            .collect();
        label.push('…');
    }
    Some(label)
}

fn paragraph_needs_link_separator(paragraph: &Paragraph) -> bool {
    paragraph
        .children
        .iter()
        .rev()
        .find_map(|child| child.text.chars().next_back())
        .is_some_and(|ch| {
            ch != ' '
                && (ch.is_whitespace()
                    || ch.is_alphanumeric()
                    || ch == '_'
                    || matches!(ch, ')' | ']' | '}'))
        })
}

/// Parse Markdown into a tree of nodes.
///
/// TODO: Remove `highlight_theme` option, this should in render stage.
pub(crate) fn parse(
    source: &str,
    cx: &mut NodeContext,
    highlight_theme: &HighlightTheme,
) -> Result<ParsedDocument, SharedString> {
    markdown::to_mdast(&source, &ParseOptions::gfm())
        .map(|n| ast_to_document(source, n, cx, highlight_theme))
        .map_err(|e| e.to_string().into())
}

fn parse_table_row(source: &str, table: &mut Table, node: &mdast::TableRow, cx: &mut NodeContext) {
    let mut row = TableRow::default();
    node.children.iter().for_each(|c| {
        match c {
            Node::TableCell(cell) => {
                parse_table_cell(source, &mut row, cell, cx);
            }
            _ => {}
        };
    });
    table.children.push(row);
}

fn parse_table_cell(
    source: &str,
    row: &mut node::TableRow,
    node: &mdast::TableCell,
    cx: &mut NodeContext,
) {
    let mut paragraph = Paragraph::default();
    node.children.iter().for_each(|c| {
        parse_paragraph(source, &mut paragraph, c, cx);
    });
    let table_cell = node::TableCell {
        children: paragraph,
        ..Default::default()
    };
    row.children.push(table_cell);
}

fn parse_paragraph(
    source: &str,
    paragraph: &mut Paragraph,
    node: &mdast::Node,
    cx: &mut NodeContext,
) -> String {
    let span = node.position().map(|pos| Span {
        start: cx.offset + pos.start.offset,
        end: cx.offset + pos.end.offset,
    });
    if let Some(span) = span {
        paragraph.set_span(span);
    }

    let mut text = String::new();

    match node {
        Node::Paragraph(val) => {
            val.children.iter().for_each(|c| {
                text.push_str(&parse_paragraph(source, paragraph, c, cx));
            });
        }
        Node::Text(val) => {
            text = val.value.clone();
            paragraph.push_str(&val.value)
        }
        Node::Emphasis(val) => {
            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(source, &mut child_paragraph, &child, cx));
            }
            paragraph.push(
                InlineNode::new(&text).marks(vec![(0..text.len(), TextMark::default().italic())]),
            );
        }
        Node::Strong(val) => {
            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(source, &mut child_paragraph, &child, cx));
            }
            paragraph.push(
                InlineNode::new(&text).marks(vec![(0..text.len(), TextMark::default().bold())]),
            );
        }
        Node::Delete(val) => {
            let mut child_paragraph = Paragraph::default();
            for child in val.children.iter() {
                text.push_str(&parse_paragraph(source, &mut child_paragraph, &child, cx));
            }
            paragraph.push(
                InlineNode::new(&text)
                    .marks(vec![(0..text.len(), TextMark::default().strikethrough())]),
            );
        }
        Node::InlineCode(val) => {
            text = val.value.clone();
            paragraph.push(
                InlineNode::new(&text).marks(vec![(0..text.len(), TextMark::default().code())]),
            );
        }
        Node::Link(val) => {
            let link_mark = LinkMark {
                url: val.url.clone().into(),
                title: val.title.clone().map(Into::into),
                ..Default::default()
            };

            if let Some((_source_start, label)) = compact_bare_http_label(
                source,
                node.position(),
                val,
                cx.style.markdown_link_label_policy,
            ) {
                if paragraph_needs_link_separator(paragraph) {
                    paragraph.push_str(" ");
                    text.push(' ');
                }
                let label_len = label.len();
                text.push_str(&label);
                paragraph.push(InlineNode::new(label).marks(vec![(
                    0..label_len,
                    TextMark {
                        link: Some(link_mark),
                        ..Default::default()
                    },
                )]));
            } else {
                let link_mark = Some(link_mark);
                let mut child_paragraph = Paragraph::default();
                for child in val.children.iter() {
                    text.push_str(&parse_paragraph(source, &mut child_paragraph, child, cx));
                }

                // FIXME: GPUI InteractiveText does not support inline images yet.
                // So here we push images to the paragraph directly.
                for child in child_paragraph.children.iter_mut() {
                    if let Some(image) = child.image.as_mut() {
                        image.link = link_mark.clone();
                    }

                    child.marks.push((
                        0..child.text.len(),
                        TextMark {
                            link: link_mark.clone(),
                            ..Default::default()
                        },
                    ));
                }

                paragraph.merge(child_paragraph);
            }
        }
        Node::Image(raw) => {
            paragraph.push_image(ImageNode {
                url: raw.url.clone().into(),
                title: raw.title.clone().map(|t| t.into()),
                alt: Some(raw.alt.clone().into()),
                ..Default::default()
            });
        }
        Node::InlineMath(raw) => {
            text = raw.value.clone();
            paragraph.push(
                InlineNode::new(&text).marks(vec![(0..text.len(), TextMark::default().code())]),
            );
        }
        Node::MdxTextExpression(raw) => {
            text = raw.value.clone();
            paragraph
                .push(InlineNode::new(&text).marks(vec![(0..text.len(), TextMark::default())]));
        }
        Node::Html(val) => match super::html::parse(&val.value, cx) {
            Ok(el) => {
                if el
                    .blocks
                    .first()
                    .map(|node| node.is_break())
                    .unwrap_or(false)
                {
                    text = "\n".to_owned();
                    paragraph.push(InlineNode::new(&text));
                } else {
                    if cfg!(debug_assertions) {
                        tracing::warn!("unsupported inline html tag: {:#?}", el);
                    }
                }
            }
            Err(err) => {
                if cfg!(debug_assertions) {
                    tracing::warn!("failed parsing html: {:#?}", err);
                }

                text.push_str(&val.value);
            }
        },
        Node::FootnoteReference(foot) => {
            let prefix = format!("[{}]", foot.identifier);
            paragraph.push(InlineNode::new(&prefix).marks(vec![(
                0..prefix.len(),
                TextMark {
                    italic: true,
                    ..Default::default()
                },
            )]));
        }
        Node::LinkReference(link) => {
            let mut child_paragraph = Paragraph::default();
            let mut child_text = String::new();
            for child in link.children.iter() {
                child_text.push_str(&parse_paragraph(source, &mut child_paragraph, child, cx));
            }

            let link_mark = LinkMark {
                url: "".into(),
                title: link.label.clone().map(Into::into),
                identifier: Some(link.identifier.clone().into()),
            };

            paragraph.push(InlineNode::new(&child_text).marks(vec![(
                0..child_text.len(),
                TextMark {
                    link: Some(link_mark),
                    ..Default::default()
                },
            )]));
        }
        _ => {
            if cfg!(debug_assertions) {
                tracing::warn!("unsupported inline node: {:#?}", node);
            }
        }
    }

    text
}

fn ast_to_document(
    source: &str,
    root: mdast::Node,
    cx: &mut NodeContext,
    highlight_theme: &HighlightTheme,
) -> ParsedDocument {
    let root = match root {
        Node::Root(r) => r,
        _ => panic!("expected root node"),
    };

    let blocks = root
        .children
        .into_iter()
        .map(|c| ast_to_node(source, c, cx, highlight_theme))
        .collect();
    ParsedDocument {
        source: source.to_string().into(),
        blocks,
    }
}

fn new_span(pos: Option<markdown::unist::Position>, cx: &NodeContext) -> Option<Span> {
    let pos = pos?;

    Some(Span {
        start: cx.offset + pos.start.offset,
        end: cx.offset + pos.end.offset,
    })
}

fn ast_to_node(
    source: &str,
    value: mdast::Node,
    cx: &mut NodeContext,
    highlight_theme: &HighlightTheme,
) -> BlockNode {
    match value {
        Node::Root(_) => unreachable!("node::Root should be handled separately"),
        Node::Paragraph(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(source, &mut paragraph, c, cx);
            });
            paragraph.span = new_span(val.position, cx);
            BlockNode::Paragraph(paragraph)
        }
        Node::Blockquote(val) => {
            let children = val
                .children
                .into_iter()
                .map(|c| ast_to_node(source, c, cx, highlight_theme))
                .collect();
            BlockNode::Blockquote {
                children,
                span: new_span(val.position, cx),
            }
        }
        Node::List(list) => {
            let children = list
                .children
                .into_iter()
                .map(|c| ast_to_node(source, c, cx, highlight_theme))
                .collect();
            BlockNode::List {
                ordered: list.ordered,
                children,
                span: new_span(list.position, cx),
            }
        }
        Node::ListItem(val) => {
            let children = val
                .children
                .into_iter()
                .map(|c| ast_to_node(source, c, cx, highlight_theme))
                .collect();
            BlockNode::ListItem {
                children,
                spread: val.spread,
                checked: val.checked,
                span: new_span(val.position, cx),
            }
        }
        Node::Break(val) => BlockNode::Break {
            html: false,
            span: new_span(val.position, cx),
        },
        Node::Code(raw) => BlockNode::CodeBlock(CodeBlock::new(
            raw.value.into(),
            raw.lang.map(|s| s.into()),
            highlight_theme,
            new_span(raw.position, cx),
        )),
        Node::Heading(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(source, &mut paragraph, c, cx);
            });

            BlockNode::Heading {
                level: val.depth,
                children: paragraph,
                span: new_span(val.position, cx),
            }
        }
        Node::Math(val) => BlockNode::CodeBlock(CodeBlock::new(
            val.value.into(),
            None,
            highlight_theme,
            new_span(val.position, cx),
        )),
        Node::Html(val) => match super::html::parse(&val.value, cx) {
            Ok(el) => BlockNode::Root {
                children: el.blocks,
                span: new_span(val.position, cx),
            },
            Err(err) => {
                if cfg!(debug_assertions) {
                    tracing::warn!("error parsing html: {:#?}", err);
                }

                BlockNode::Paragraph(Paragraph::new(val.value))
            }
        },
        Node::MdxFlowExpression(val) => BlockNode::CodeBlock(CodeBlock::new(
            val.value.into(),
            Some("mdx".into()),
            highlight_theme,
            new_span(val.position, cx),
        )),
        Node::Yaml(val) => BlockNode::CodeBlock(CodeBlock::new(
            val.value.into(),
            Some("yml".into()),
            highlight_theme,
            new_span(val.position, cx),
        )),
        Node::Toml(val) => BlockNode::CodeBlock(CodeBlock::new(
            val.value.into(),
            Some("toml".into()),
            highlight_theme,
            new_span(val.position, cx),
        )),
        Node::MdxJsxTextElement(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(source, &mut paragraph, c, cx);
            });
            paragraph.span = new_span(val.position, cx);
            BlockNode::Paragraph(paragraph)
        }
        Node::MdxJsxFlowElement(val) => {
            let mut paragraph = Paragraph::default();
            val.children.iter().for_each(|c| {
                parse_paragraph(source, &mut paragraph, c, cx);
            });
            paragraph.span = new_span(val.position, cx);
            BlockNode::Paragraph(paragraph)
        }
        Node::ThematicBreak(val) => BlockNode::Divider {
            span: new_span(val.position, cx),
        },
        Node::Table(val) => {
            let mut table = Table::default();
            table.column_aligns = val
                .align
                .clone()
                .into_iter()
                .map(|align| align.into())
                .collect();
            val.children.iter().for_each(|c| {
                if let Node::TableRow(row) = c {
                    parse_table_row(source, &mut table, row, cx);
                }
            });
            table.span = new_span(val.position, cx);

            BlockNode::Table(table)
        }
        Node::FootnoteDefinition(def) => {
            let mut paragraph = Paragraph::default();
            let prefix = format!("[{}]: ", def.identifier);
            paragraph.push(InlineNode::new(&prefix).marks(vec![(
                0..prefix.len(),
                TextMark {
                    italic: true,
                    ..Default::default()
                },
            )]));

            def.children.iter().for_each(|c| {
                parse_paragraph(source, &mut paragraph, c, cx);
            });
            paragraph.span = new_span(def.position, cx);
            BlockNode::Paragraph(paragraph)
        }
        Node::Definition(def) => {
            cx.add_ref(
                def.identifier.clone().into(),
                LinkMark {
                    url: def.url.clone().into(),
                    identifier: Some(def.identifier.clone().into()),
                    title: def.title.clone().map(Into::into),
                },
            );

            BlockNode::Definition {
                identifier: def.identifier.clone().into(),
                url: def.url.clone().into(),
                title: def.title.clone().map(|s| s.into()),
                span: new_span(def.position, cx),
            }
        }
        _ => {
            if cfg!(debug_assertions) {
                tracing::warn!("unsupported node: {:#?}", value);
            }
            BlockNode::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_with_policy(source: &str, policy: MarkdownLinkLabelPolicy) -> ParsedDocument {
        let mut cx = NodeContext {
            style: crate::text::TextViewStyle::default().markdown_link_label_policy(policy),
            ..NodeContext::default()
        };
        let highlight_theme = HighlightTheme::default_light();
        parse(source, &mut cx, &highlight_theme).expect("markdown should parse")
    }

    fn first_paragraph(document: &ParsedDocument) -> &Paragraph {
        match document.blocks.first() {
            Some(BlockNode::Paragraph(paragraph)) => paragraph,
            other => panic!("expected a paragraph, got {other:?}"),
        }
    }

    fn visible_text(paragraph: &Paragraph) -> String {
        paragraph
            .children
            .iter()
            .map(|child| child.text.as_ref())
            .collect()
    }

    fn visible_links(paragraph: &Paragraph) -> Vec<(String, String)> {
        paragraph
            .children
            .iter()
            .flat_map(|child| {
                child.marks.iter().filter_map(move |(_, mark)| {
                    mark.link
                        .as_ref()
                        .map(|link| (child.text.to_string(), link.url.to_string()))
                })
            })
            .collect()
    }

    #[test]
    fn compact_long_bare_http_link_preserves_source_href_and_separator() {
        let url = "https://news.google.com/rss/articles/CBMikAFBVV95cUxQbGRpMHZ2OHBuOU0tdVU1ZDFaQ29kd2RUNTZxV0VDWGVDeHBrSmxZaI9LZihWMC14T3pwcUt1RHFfUFJ6bDJYMUgyYjFCd293dVlnUndVLXZRS2VkR1AtUVhyZzVraVczVHRNMmNzWGdlNHBO?oc=5";
        let input = format!("Reuters\n{url}");

        let document = parse_with_policy(&input, MarkdownLinkLabelPolicy::CompactLongBareHttp);
        let paragraph = first_paragraph(&document);

        assert_eq!(document.source.as_ref(), input);
        assert_eq!(visible_text(paragraph), "Reuters\n news.google.com/…");
        assert_eq!(
            visible_links(paragraph),
            vec![("news.google.com/…".to_string(), url.to_string())]
        );
    }

    #[test]
    fn compact_policy_defaults_to_preserve() {
        let url = "https://example.com/a/very/long/path/that/keeps/going/through/many/segments/and/includes?a=long-query-value&b=another";
        let document = parse_with_policy(url, MarkdownLinkLabelPolicy::Preserve);
        let paragraph = first_paragraph(&document);

        assert_eq!(visible_text(paragraph), url);
        assert_eq!(
            visible_links(paragraph),
            vec![(url.to_string(), url.to_string())]
        );
    }

    #[test]
    fn compact_policy_preserves_explicit_links_autolinks_and_code() {
        let url = "https://example.com/a/very/long/path/that/keeps/going/through/many/segments/and/includes?a=long-query-value&b=another";
        let short_url = "https://example.com/short";
        let input = format!("[Publisher]({url}) <{url}> `{url}` {short_url}");
        let document = parse_with_policy(&input, MarkdownLinkLabelPolicy::CompactLongBareHttp);
        let paragraph = first_paragraph(&document);
        let links = visible_links(paragraph);

        assert!(visible_text(paragraph).contains("Publisher"));
        assert!(visible_text(paragraph).contains(url));
        assert_eq!(links.len(), 3);
        assert_eq!(links[0], ("Publisher".to_string(), url.to_string()));
        assert_eq!(links[1], (url.to_string(), url.to_string()));
        assert_eq!(links[2], (short_url.to_string(), short_url.to_string()));
    }
}

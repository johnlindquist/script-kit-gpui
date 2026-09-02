//! Day Page markdown reference parsing, normalization, and fragment path resolution.
//!
//! Fragment-reference and kept-URL lines stay plain markdown on disk and in the
//! editor. This module identifies them only for navigation/automation metadata.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::ai::message_parts::AiContextPart;
use crate::brain::substrate::BrainFrontmatter;

/// Back affordance when viewing a fragment inline on the Day Page surface.
pub const FRAGMENT_BACK_ID: &str = "day-page-fragment-back";

/// A parsed segment of today's day-page markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DayPageSegment {
    Plain {
        text: String,
        start_line: usize,
    },
    FragmentRef {
        timestamp: String,
        excerpt: String,
        relative_link: String,
        start_line: usize,
        line_count: usize,
        index: usize,
    },
    KeptUrl {
        timestamp: String,
        url: String,
        start_line: usize,
        index: usize,
    },
    ClipboardRef {
        timestamp: String,
        entry_id: String,
        start_line: usize,
        index: usize,
    },
}

impl DayPageSegment {
    pub fn start_line(&self) -> usize {
        match self {
            Self::Plain { start_line, .. }
            | Self::FragmentRef { start_line, .. }
            | Self::KeptUrl { start_line, .. }
            | Self::ClipboardRef { start_line, .. } => *start_line,
        }
    }
}

/// Provenance metadata loaded from a fragment file's frontmatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentProvenance {
    pub source_label: String,
    pub created: DateTime<Utc>,
}

/// Parse day-page content into plain, fragment-reference, and kept-URL segments.
pub fn parse_day_page_segments(content: &str) -> Vec<DayPageSegment> {
    let lines: Vec<&str> = content.split('\n').collect();
    let mut segments = Vec::new();
    let mut plain_buffer = String::new();
    let mut plain_start: Option<usize> = None;
    let mut fragment_index = 0usize;
    let mut url_index = 0usize;
    let mut clipboard_ref_index = 0usize;
    let mut line_index = 0usize;

    let flush_plain = |segments: &mut Vec<DayPageSegment>,
                       plain_buffer: &mut String,
                       plain_start: &mut Option<usize>| {
        if plain_buffer.is_empty() {
            *plain_start = None;
            return;
        }
        segments.push(DayPageSegment::Plain {
            text: std::mem::take(plain_buffer),
            start_line: plain_start.unwrap_or(0),
        });
        *plain_start = None;
    };

    while line_index < lines.len() {
        let line = lines[line_index];
        if let Some((timestamp, excerpt, relative_link)) =
            parse_fragment_card_lines(&lines, line_index)
        {
            flush_plain(&mut segments, &mut plain_buffer, &mut plain_start);
            segments.push(DayPageSegment::FragmentRef {
                timestamp,
                excerpt,
                relative_link,
                start_line: line_index,
                line_count: 3,
                index: fragment_index,
            });
            fragment_index += 1;
            line_index += 3;
            continue;
        }

        if let Some((timestamp, excerpt, relative_link)) = parse_fragment_markdown_link_line(line) {
            flush_plain(&mut segments, &mut plain_buffer, &mut plain_start);
            segments.push(DayPageSegment::FragmentRef {
                timestamp,
                excerpt,
                relative_link,
                start_line: line_index,
                line_count: 1,
                index: fragment_index,
            });
            fragment_index += 1;
            line_index += 1;
            continue;
        }

        if let Some((timestamp, excerpt)) = parse_fragment_header_line(line) {
            flush_plain(&mut segments, &mut plain_buffer, &mut plain_start);
            if line_index + 1 < lines.len() {
                let link_line = lines[line_index + 1].trim();
                if let Some(relative_link) = parse_fragment_link_line(link_line) {
                    segments.push(DayPageSegment::FragmentRef {
                        timestamp,
                        excerpt,
                        relative_link,
                        start_line: line_index,
                        line_count: 2,
                        index: fragment_index,
                    });
                    fragment_index += 1;
                    line_index += 2;
                    continue;
                }
            }
        }

        if let Some((timestamp, url)) = parse_kept_url_line(line) {
            flush_plain(&mut segments, &mut plain_buffer, &mut plain_start);
            segments.push(DayPageSegment::KeptUrl {
                timestamp,
                url,
                start_line: line_index,
                index: url_index,
            });
            url_index += 1;
            line_index += 1;
            continue;
        }

        if let Some((timestamp, entry_id)) = parse_clipboard_ref_line(line) {
            flush_plain(&mut segments, &mut plain_buffer, &mut plain_start);
            segments.push(DayPageSegment::ClipboardRef {
                timestamp,
                entry_id,
                start_line: line_index,
                index: clipboard_ref_index,
            });
            clipboard_ref_index += 1;
            line_index += 1;
            continue;
        }

        if plain_start.is_none() {
            plain_start = Some(line_index);
        }
        if !plain_buffer.is_empty() {
            plain_buffer.push('\n');
        }
        plain_buffer.push_str(line);
        line_index += 1;
    }

    flush_plain(&mut segments, &mut plain_buffer, &mut plain_start);
    segments
}

/// One clipboard-history reference lifted out of the Day Page editor buffer.
///
/// Clipboard sediment stays in the day file (canonical markdown, raw-free
/// refs), but the editor does not show the raw `[Clipboard entry](kit://…)`
/// lines — they read as link spam and swamp the note. The Day Page lifts them
/// into a compact clipboard shelf below the editor instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardShelfItem {
    pub timestamp: String,
    pub entry_id: String,
    /// The exact original day-file line, so rejoining is lossless and
    /// `undo_clipboard_sediment_lines` token matching keeps working.
    pub line: String,
}

/// Split day-page content into the visible note body and the clipboard shelf.
///
/// Every `HH:MM [label](kit://clipboard-history?id=…)` line moves to the
/// shelf; all other lines stay verbatim (in order) in the visible body.
pub fn split_day_page_clipboard_shelf(content: &str) -> (String, Vec<ClipboardShelfItem>) {
    let mut shelf = Vec::new();
    let mut visible: Vec<&str> = Vec::new();
    for line in content.split('\n') {
        if let Some((timestamp, entry_id)) = parse_clipboard_ref_line(line) {
            shelf.push(ClipboardShelfItem {
                timestamp,
                entry_id,
                line: line.to_string(),
            });
        } else {
            visible.push(line);
        }
    }
    (visible.join("\n"), shelf)
}

/// Rejoin the visible note body with the clipboard shelf into canonical
/// day-file content. Shelf lines land grouped at the end of the file, so a
/// legacy interleaved day page normalizes once through the ordinary
/// merge-aware save path, and `split → join` is stable from then on.
pub fn join_day_page_clipboard_shelf(visible: &str, shelf: &[ClipboardShelfItem]) -> String {
    if shelf.is_empty() {
        return visible.to_string();
    }
    // Split projects the final line terminator back into the visible body.
    // Preserve its presence instead of manufacturing a newline on reopen.
    let trailing_newline = visible.is_empty() || visible.ends_with('\n');
    let mut out = visible.to_string();
    for item in shelf {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&item.line);
    }
    if trailing_newline {
        out.push('\n');
    }
    out
}

/// Normalize raw link/reference tokens entering the Day Page editor into
/// markdown so they render through the same markdown editor path as Notes.
pub fn normalize_day_page_markdown_references(content: &str) -> String {
    let mut normalized = String::with_capacity(content.len());
    for (line_index, line) in content.split('\n').enumerate() {
        if line_index > 0 {
            normalized.push('\n');
        }
        normalized.push_str(&normalize_day_page_markdown_reference_line(line));
    }
    normalized
}

fn normalize_day_page_markdown_reference_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut index = 0usize;
    while index < line.len() {
        let rest = &line[index..];
        let Some(relative_start) = next_markdown_reference_start(rest) else {
            output.push_str(rest);
            break;
        };
        let start = index + relative_start;
        output.push_str(&line[index..start]);
        let end = raw_reference_end(line, start);
        let token = &line[start..end];
        if raw_reference_is_already_markdown_link(line, start, end) {
            output.push_str(token);
        } else if let Some(link) = markdown_link_for_raw_reference(token) {
            output.push_str(&link);
        } else {
            output.push_str(token);
        }
        index = end;
    }
    output
}

fn next_markdown_reference_start(text: &str) -> Option<usize> {
    [
        "https://",
        "http://",
        "../fragments/",
        "@file:",
        "@project:",
        "@notes:",
        "@scripts:",
        "@clipboard:",
        "@history:",
        "@browser-history:",
        "@skill:",
    ]
    .into_iter()
    .filter_map(|needle| text.find(needle))
    .min()
}

fn raw_reference_end(line: &str, start: usize) -> usize {
    if line[..start].ends_with('<') {
        if let Some(offset) = line[start..].find('>') {
            return start + offset;
        }
    }
    let mut end = line.len();
    for (offset, ch) in line[start..].char_indices() {
        if ch.is_whitespace() {
            end = start + offset;
            break;
        }
    }
    while end > start {
        let Some((ch_start, ch)) = line[..end].char_indices().next_back() else {
            break;
        };
        if matches!(ch, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}') {
            end = ch_start;
        } else {
            break;
        }
    }
    end
}

fn raw_reference_is_already_markdown_link(line: &str, start: usize, end: usize) -> bool {
    line[..start].ends_with("](")
        || (line[..start].ends_with('<') && line[end..].trim_start().starts_with('>'))
}

fn markdown_link_for_raw_reference(token: &str) -> Option<String> {
    if is_single_token_http_url(token) {
        let label = token
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
        let label = if label.is_empty() {
            token.trim()
        } else {
            label
        };
        return Some(format!("[{}]({token})", markdown_link_label(label)));
    }
    if token.starts_with("../fragments/") && token.ends_with(".md") {
        return Some(format!("[Open fragment]({token})"));
    }
    if let Some((prefix, value)) = raw_context_reference_parts(token) {
        let label = markdown_link_label(&value.replace('-', " "));
        return Some(format!(
            "[{label}](scriptkit://spine/{}/{})",
            prefix,
            encode_url_component(&value)
        ));
    }
    None
}

fn markdown_link_label(label: &str) -> String {
    label.replace('[', "\\[").replace(']', "\\]")
}

fn raw_context_reference_parts(token: &str) -> Option<(&str, String)> {
    let token = token.trim();
    let body = token.strip_prefix('@')?;
    let (prefix, value) = body.split_once(':')?;
    if !matches!(
        prefix,
        "file"
            | "project"
            | "notes"
            | "scripts"
            | "clipboard"
            | "history"
            | "browser-history"
            | "skill"
    ) {
        return None;
    }
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some((prefix, value.to_string()))
}

/// Convert an accepted context part into persisted Day Page markdown.
///
/// Day pages are markdown documents first. Launcher context tokens such as
/// `@file:README.md` are useful editing affordances, but they should not leak
/// into the saved brain file when the selected row already has a stable
/// markdown target.
pub fn day_page_markdown_reference_for_context_part(
    token: &str,
    part: Option<&AiContextPart>,
) -> Option<String> {
    let part = part.cloned().or_else(|| {
        crate::ai::context_contract::ContextAttachmentKind::from_mention_line(token)
            .map(|kind| kind.part())
    })?;
    let (label, href) = match part {
        AiContextPart::FilePath { path, label } | AiContextPart::SkillFile { path, label, .. } => {
            (label, file_href(&path))
        }
        AiContextPart::ResourceUri { uri, label } => (label, uri),
        AiContextPart::TextBlock { label, source, .. } => {
            if !source.contains(':') || source.contains(char::is_whitespace) {
                return None;
            }
            (label, source)
        }
        AiContextPart::FocusedTarget { target, label } => {
            if let Some(path) = target
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("path"))
                .and_then(|value| value.as_str())
            {
                (label, file_href(path))
            } else {
                (
                    label,
                    format!("kit://focused-target/{}", target.semantic_id),
                )
            }
        }
        AiContextPart::AmbientContext { label } => (
            label.clone(),
            format!("kit://context?label={}", encode_url_component(&label)),
        ),
    };
    let label = markdown_link_label(label.trim());
    if label.is_empty() || href.trim().is_empty() {
        return None;
    }
    Some(format!("[{label}]({})", markdown_link_destination(&href)))
}

/// Extract context parts from markdown links in a Day Page line.
///
/// This is the reverse of `day_page_markdown_reference_for_context_part` for
/// Cmd+Enter handoff: readable markdown file/resource links should still stage
/// real Agent Chat context.
pub fn context_parts_from_day_page_markdown_links(markdown: &str) -> Vec<AiContextPart> {
    let mut parts = Vec::new();
    for (label, href) in markdown_links(markdown) {
        let Some(part) = context_part_from_markdown_link(&label, &href) else {
            continue;
        };
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    parts
}

fn context_part_from_markdown_link(label: &str, href: &str) -> Option<AiContextPart> {
    let label = label.trim().to_string();
    let href = href.trim();
    if label.is_empty() || href.is_empty() {
        return None;
    }
    if let Some(path) = href.strip_prefix("file://") {
        let path = decode_url_component(path);
        return Some(AiContextPart::FilePath { path, label });
    }
    if href.starts_with("kit://") {
        return Some(AiContextPart::ResourceUri {
            uri: href.to_string(),
            label,
        });
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(AiContextPart::TextBlock {
            label,
            source: href.to_string(),
            text: href.to_string(),
            mime_type: Some("text/uri-list".to_string()),
        });
    }
    None
}

fn markdown_links(markdown: &str) -> Vec<(String, String)> {
    let mut links = Vec::new();
    let bytes = markdown.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'[' {
            index += 1;
            continue;
        }
        let Some(label_end) = find_unescaped_byte(markdown, index + 1, b']') else {
            break;
        };
        if !markdown[label_end..].starts_with("](") {
            index = label_end + 1;
            continue;
        }
        let href_start = label_end + 2;
        let Some(href_end) = find_unescaped_byte(markdown, href_start, b')') else {
            break;
        };
        let label = markdown[index + 1..label_end]
            .replace("\\[", "[")
            .replace("\\]", "]");
        let href = markdown[href_start..href_end].to_string();
        links.push((label, href));
        index = href_end + 1;
    }
    links
}

fn find_unescaped_byte(text: &str, start: usize, needle: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        if bytes[index] == needle {
            let mut slash_count = 0usize;
            let mut cursor = index;
            while cursor > 0 && bytes[cursor - 1] == b'\\' {
                slash_count += 1;
                cursor -= 1;
            }
            if slash_count.is_multiple_of(2) {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

fn file_href(path: &str) -> String {
    format!("file://{}", encode_url_path(path))
}

fn markdown_link_destination(href: &str) -> String {
    href.replace(')', "%29")
}

fn encode_url_path(path: &str) -> String {
    path.chars()
        .map(|ch| match ch {
            ' ' => "%20".to_string(),
            ')' => "%29".to_string(),
            '(' => "%28".to_string(),
            '%' => "%25".to_string(),
            _ => ch.to_string(),
        })
        .collect()
}

fn encode_url_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => ch.to_string(),
            ' ' => "%20".to_string(),
            _ => format!("%{:02X}", ch as u32),
        })
        .collect()
}

fn decode_url_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).to_string())
}

fn parse_fragment_card_lines(
    lines: &[&str],
    line_index: usize,
) -> Option<(String, String, String)> {
    if line_index + 2 >= lines.len() {
        return None;
    }
    let timestamp = parse_fragment_card_header_line(lines[line_index])?;
    let excerpt = parse_fragment_quote_line(lines[line_index + 1])?;
    let relative_link = parse_fragment_link_line(lines[line_index + 2])?;
    Some((timestamp, excerpt, relative_link))
}

fn parse_fragment_card_header_line(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    let (timestamp, rest) = trimmed.split_once(' ')?;
    if !is_timestamp(timestamp) || rest.trim() != "Fragment" {
        return None;
    }
    Some(timestamp.to_string())
}

fn parse_fragment_quote_line(line: &str) -> Option<String> {
    let excerpt = line.trim().strip_prefix('>')?.trim();
    if excerpt.is_empty() {
        return None;
    }
    Some(excerpt.to_string())
}

fn parse_fragment_markdown_link_line(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim_end();
    let (timestamp, rest) = trimmed.split_once(' ')?;
    if !is_timestamp(timestamp) {
        return None;
    }
    let rest = rest.trim();
    let (excerpt, relative_link) = parse_markdown_link(rest)?;
    parse_fragment_link_line(&relative_link).map(|link| (timestamp.to_string(), excerpt, link))
}

fn parse_fragment_header_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_end();
    let (timestamp, rest) = trimmed.split_once(" > ")?;
    if !is_timestamp(timestamp) {
        return None;
    }
    let excerpt = rest.trim();
    if excerpt.is_empty() {
        return None;
    }
    Some((timestamp.to_string(), excerpt.to_string()))
}

fn parse_fragment_link_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let relative_link = parse_markdown_link(trimmed)
        .map(|(_label, url)| url)
        .unwrap_or_else(|| trimmed.to_string());
    if relative_link.starts_with("../fragments/") && relative_link.ends_with(".md") {
        Some(relative_link)
    } else {
        None
    }
}

fn parse_kept_url_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_end();
    let (timestamp, rest) = trimmed.split_once(' ')?;
    if !is_timestamp(timestamp) {
        return None;
    }
    if let Some((_label, url)) = parse_markdown_link(rest.trim()) {
        if is_single_token_http_url(&url) {
            return Some((timestamp.to_string(), url));
        }
    }
    let url = rest.trim();
    if !is_single_token_http_url(url) {
        return None;
    }
    Some((timestamp.to_string(), url.to_string()))
}

fn parse_clipboard_ref_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_end();
    let (timestamp, rest) = trimmed.split_once(' ')?;
    if !is_timestamp(timestamp) {
        return None;
    }
    let (_label, uri) = parse_markdown_link(rest.trim())?;
    crate::clipboard_history::parse_entry_resource_uri(&uri)
        .map(|entry_id| (timestamp.to_string(), entry_id))
}

fn parse_markdown_link(text: &str) -> Option<(String, String)> {
    let label_start = text.find('[')?;
    if label_start != 0 {
        return None;
    }
    let label_end = text[label_start + 1..].find("](")? + label_start + 1;
    let link_start = label_end + 2;
    let link_end = text[link_start..].rfind(')')? + link_start;
    if link_end + 1 != text.len() {
        return None;
    }
    let label = text[label_start + 1..label_end].trim();
    let url = text[link_start..link_end].trim();
    if label.is_empty() || url.is_empty() {
        return None;
    }
    Some((label.to_string(), url.to_string()))
}

fn is_timestamp(value: &str) -> bool {
    value.len() == 5
        && value.as_bytes().get(2) == Some(&b':')
        && value.chars().all(|ch| ch.is_ascii_digit() || ch == ':')
}

fn is_single_token_http_url(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() != 1 {
        return false;
    }
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

/// Resolve a day-page relative fragment link against the bound day file path.
pub fn resolve_fragment_path(day_page_path: &Path, relative_link: &str) -> Option<PathBuf> {
    let brain_dir = day_page_path.parent()?.parent()?;
    let relative = relative_link.strip_prefix("../").unwrap_or(relative_link);
    let path = brain_dir.join(relative);
    let brain_paths = crate::brain::substrate::BrainPaths::new(brain_dir);
    (brain_paths.contains(&path) && path.parent() == Some(brain_paths.fragments_dir().as_path()))
        .then_some(path)
}

/// Load provenance metadata from a fragment markdown file.
pub fn load_fragment_provenance(fragment_path: &Path) -> Option<FragmentProvenance> {
    if fragment_path.parent()?.file_name()?.to_str()? != "fragments" {
        return None;
    }
    let content = crate::brain::substrate::io::read_private_document(fragment_path).ok()?;
    let (frontmatter, _) = BrainFrontmatter::parse(&content).ok()?;
    Some(FragmentProvenance {
        source_label: format_source_label(frontmatter.source.as_deref()),
        created: frontmatter.created,
    })
}

/// Human-readable provenance hint for fragment references.
pub fn format_provenance_hint(provenance: &FragmentProvenance, tz: Tz) -> String {
    let time = provenance
        .created
        .with_timezone(&tz)
        .format("%H:%M")
        .to_string();
    format!("{} · {time}", provenance.source_label)
}

fn format_source_label(source: Option<&str>) -> String {
    match source {
        Some(uri) if uri.starts_with("kit://clipboard-history") => "Clipboard".to_string(),
        Some(uri) if uri.starts_with("scriptkit://clipboard/") => "Clipboard".to_string(),
        Some(uri) if uri.starts_with("scriptkit://agent-chat/") => "Agent Chat".to_string(),
        Some(uri) if uri.starts_with("scriptkit://") => "Script Kit".to_string(),
        Some(uri) if !uri.is_empty() => uri.to_string(),
        Some(_) | None => "Captured".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Tz;

    #[test]
    fn parse_fragment_and_kept_url_segments() {
        let content = "09:00 morning note\n\
            09:15 Fragment\n\
            > First words of the pasted article without cutting mid-word...\n\
            [Open fragment](../fragments/2026-06-11-0942-clipboard.md)\n\
            09:20 [example.com/docs](https://example.com/docs)\n\
            09:30 closing thought";

        let segments = parse_day_page_segments(content);
        assert_eq!(segments.len(), 4);

        match &segments[0] {
            DayPageSegment::Plain { text, .. } => assert_eq!(text, "09:00 morning note"),
            other => panic!("expected plain segment, got {other:?}"),
        }

        match &segments[1] {
            DayPageSegment::FragmentRef {
                excerpt,
                relative_link,
                index,
                line_count,
                ..
            } => {
                assert_eq!(index, &0);
                assert_eq!(line_count, &3);
                assert!(excerpt.contains("First words"));
                assert_eq!(relative_link, "../fragments/2026-06-11-0942-clipboard.md");
            }
            other => panic!("expected fragment segment, got {other:?}"),
        }

        match &segments[2] {
            DayPageSegment::KeptUrl { url, index, .. } => {
                assert_eq!(index, &0);
                assert_eq!(url, "https://example.com/docs");
            }
            other => panic!("expected kept url segment, got {other:?}"),
        }

        match &segments[3] {
            DayPageSegment::Plain { text, .. } => assert_eq!(text, "09:30 closing thought"),
            other => panic!("expected plain segment, got {other:?}"),
        }
    }

    #[test]
    fn parses_legacy_one_line_fragment_reference_for_navigation() {
        let content = "09:15 [First words of the pasted article without cutting mid-word...](../fragments/2026-06-11-0942-clipboard.md)\n";

        let segments = parse_day_page_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            DayPageSegment::FragmentRef {
                excerpt,
                relative_link,
                line_count,
                ..
            } => {
                assert_eq!(line_count, &1);
                assert!(excerpt.contains("First words"));
                assert_eq!(relative_link, "../fragments/2026-06-11-0942-clipboard.md");
            }
            other => panic!("expected fragment segment, got {other:?}"),
        }
    }

    #[test]
    fn parses_legacy_raw_kept_url_lines_for_existing_day_pages() {
        let content = "09:20 https://example.com/docs\n";

        let segments = parse_day_page_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            DayPageSegment::KeptUrl { url, index, .. } => {
                assert_eq!(index, &0);
                assert_eq!(url, "https://example.com/docs");
            }
            other => panic!("expected kept url segment, got {other:?}"),
        }
    }

    #[test]
    fn parses_clipboard_history_refs_without_raw_values() {
        let content = "09:20 [Clipboard entry](kit://clipboard-history?id=entry-1)\n";

        let segments = parse_day_page_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            DayPageSegment::ClipboardRef {
                entry_id,
                timestamp,
                index,
                ..
            } => {
                assert_eq!(timestamp, "09:20");
                assert_eq!(entry_id, "entry-1");
                assert_eq!(index, &0);
            }
            other => panic!("expected clipboard ref segment, got {other:?}"),
        }
    }

    #[test]
    fn parses_legacy_two_line_fragment_reference_for_navigation() {
        let content = "09:15 > First words of the pasted article without cutting mid-word...\n\
              ../fragments/2026-06-11-0942-clipboard.md\n";

        let segments = parse_day_page_segments(content);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            DayPageSegment::FragmentRef {
                excerpt,
                relative_link,
                line_count,
                ..
            } => {
                assert_eq!(line_count, &2);
                assert!(excerpt.contains("First words"));
                assert_eq!(relative_link, "../fragments/2026-06-11-0942-clipboard.md");
            }
            other => panic!("expected fragment segment, got {other:?}"),
        }
    }

    #[test]
    fn normalizes_raw_urls_to_markdown_links() {
        let content = "https://reference-one.example/docs\n\
            see https://reference-two.example/brief now";

        let normalized = normalize_day_page_markdown_references(content);

        assert!(
            normalized.contains("[reference-one.example/docs](https://reference-one.example/docs)")
        );
        assert!(normalized
            .contains("[reference-two.example/brief](https://reference-two.example/brief) now"));
    }

    #[test]
    fn normalizes_completed_typed_url_after_trailing_whitespace() {
        let content = "See https://reference-two.example/brief \nnext";

        let normalized = normalize_day_page_markdown_references(content);

        assert!(normalized
            .contains("See [reference-two.example/brief](https://reference-two.example/brief) "));
    }

    #[test]
    fn context_file_part_persists_as_markdown_link() {
        let part = AiContextPart::FilePath {
            path: "/Users/me/Screen Flow.md".to_string(),
            label: "Screen Flow.md".to_string(),
        };

        let reference =
            day_page_markdown_reference_for_context_part("@file:Screen-Flow.md", Some(&part))
                .expect("file context should become a markdown link");

        assert_eq!(
            reference,
            "[Screen Flow.md](file:///Users/me/Screen%20Flow.md)"
        );
    }

    #[test]
    fn normalizes_raw_context_references_to_markdown_links() {
        let normalized =
            normalize_day_page_markdown_references("@file:project-brief\n@notes:daily");

        assert_eq!(
            normalized,
            "[project brief](scriptkit://spine/file/project-brief)\n[daily](scriptkit://spine/notes/daily)"
        );
    }

    #[test]
    fn markdown_file_links_round_trip_to_context_parts() {
        let parts = context_parts_from_day_page_markdown_links(
            "Review [Screen Flow.md](file:///Users/me/Screen%20Flow.md)",
        );

        assert_eq!(
            parts,
            vec![AiContextPart::FilePath {
                path: "/Users/me/Screen Flow.md".to_string(),
                label: "Screen Flow.md".to_string(),
            }]
        );
    }

    #[test]
    fn normalizes_raw_fragment_references_to_markdown_links() {
        let content = "09:15 > First words\n../fragments/2026-06-11-0942-clipboard.md";

        let normalized = normalize_day_page_markdown_references(content);

        assert!(normalized.contains("[Open fragment](../fragments/2026-06-11-0942-clipboard.md)"));
    }

    #[test]
    fn leaves_existing_markdown_links_unchanged() {
        let content = "09:20 [example.com/docs](https://example.com/docs)\n\
            <https://example.com/autolink>";

        let normalized = normalize_day_page_markdown_references(content);

        assert_eq!(normalized, content);
    }

    #[test]
    fn clipboard_shelf_round_trip_is_identity_for_grouped_files() {
        // Steady state: notes on top, clipboard refs grouped at the end.
        let content = "09:00 morning note\nsecond line\n\
            09:20 [Clipboard entry](kit://clipboard-history?id=entry-1)\n\
            09:25 [Clipboard entry](kit://clipboard-history?id=entry-2)\n";

        let (visible, shelf) = split_day_page_clipboard_shelf(content);
        assert_eq!(visible, "09:00 morning note\nsecond line\n");
        assert_eq!(shelf.len(), 2);
        assert_eq!(shelf[0].timestamp, "09:20");
        assert_eq!(shelf[0].entry_id, "entry-1");
        assert_eq!(shelf[1].entry_id, "entry-2");
        assert_eq!(join_day_page_clipboard_shelf(&visible, &shelf), content);
    }

    #[test]
    fn clipboard_shelf_preserves_visible_body_ending_across_repeated_round_trips() {
        let (_, shelf) = split_day_page_clipboard_shelf(
            "09:20 [Clipboard entry](kit://clipboard-history?id=entry-1)\n\
             09:25 [Clipboard entry](kit://clipboard-history?id=entry-2)\n",
        );
        for items in [&shelf[..1], &shelf[..]] {
            for visible in ["", "body", "body ", "body\n", "body\n\n"] {
                let canonical = join_day_page_clipboard_shelf(visible, items);
                let mut reloaded = canonical.clone();
                for _ in 0..3 {
                    let (body, reloaded_shelf) = split_day_page_clipboard_shelf(&reloaded);
                    assert_eq!(body, visible);
                    assert_eq!(reloaded_shelf, items);
                    reloaded = join_day_page_clipboard_shelf(&body, &reloaded_shelf);
                    assert_eq!(reloaded, canonical);
                }
            }
        }
    }

    #[test]
    fn clipboard_shelf_normalizes_interleaved_legacy_files_once() {
        let content = "09:00 morning note\n\
            09:20 [Clipboard entry](kit://clipboard-history?id=entry-1)\n\
            09:30 closing thought\n";

        let (visible, shelf) = split_day_page_clipboard_shelf(content);
        assert_eq!(visible, "09:00 morning note\n09:30 closing thought\n");
        assert_eq!(shelf.len(), 1);

        // First join sinks the clipboard line to the end…
        let normalized = join_day_page_clipboard_shelf(&visible, &shelf);
        assert_eq!(
            normalized,
            "09:00 morning note\n09:30 closing thought\n\
             09:20 [Clipboard entry](kit://clipboard-history?id=entry-1)\n"
        );

        // …and from then on split → join is stable.
        let (visible_2, shelf_2) = split_day_page_clipboard_shelf(&normalized);
        assert_eq!(visible_2, visible);
        assert_eq!(shelf_2, shelf);
        assert_eq!(
            join_day_page_clipboard_shelf(&visible_2, &shelf_2),
            normalized
        );
    }

    #[test]
    fn clipboard_shelf_split_is_identity_without_clipboard_refs() {
        let content = "09:00 morning note\n\n09:30 closing thought\n";
        let (visible, shelf) = split_day_page_clipboard_shelf(content);
        assert_eq!(visible, content);
        assert!(shelf.is_empty());
        assert_eq!(join_day_page_clipboard_shelf(&visible, &shelf), content);
    }

    #[test]
    fn clipboard_shelf_handles_clipboard_only_files() {
        let content = "09:20 [Clipboard entry](kit://clipboard-history?id=entry-1)\n";
        let (visible, shelf) = split_day_page_clipboard_shelf(content);
        assert_eq!(visible, "");
        assert_eq!(shelf.len(), 1);
        assert_eq!(join_day_page_clipboard_shelf(&visible, &shelf), content);
    }

    #[test]
    fn resolve_fragment_path_from_day_page() {
        let day = PathBuf::from("/tmp/brain/days/2026-06-11.md");
        let resolved = resolve_fragment_path(&day, "../fragments/2026-06-11-0942-clipboard.md")
            .expect("resolve");
        assert_eq!(
            resolved,
            PathBuf::from("/tmp/brain/fragments/2026-06-11-0942-clipboard.md")
        );
    }

    #[test]
    fn private_fragment_links_cannot_escape_the_owned_brain_fragment_directory() {
        let day = PathBuf::from("/tmp/brain/days/2026-08-22.md");

        for hostile in [
            "../../foreign.md",
            "../notes/private-note.md",
            "../fragments/../../foreign.md",
            "/tmp/foreign.md",
            "../fragments/nested/private.md",
        ] {
            assert!(
                resolve_fragment_path(&day, hostile).is_none(),
                "hostile fragment link escaped: {hostile}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn private_fragment_provenance_refuses_hostile_symlink_targets() {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir().expect("isolated fragment provenance fixture");
        let fragments = fixture.path().join("brain").join("fragments");
        std::fs::create_dir_all(&fragments).unwrap();
        let foreign = fixture.path().join("foreign-private-fragment.md");
        std::fs::write(&foreign, "foreign private fragment").unwrap();
        let planted = fragments.join("planted.md");
        symlink(&foreign, &planted).expect("plant hostile fragment link");

        assert!(load_fragment_provenance(&planted).is_none());
        assert_eq!(
            std::fs::read_to_string(foreign).unwrap(),
            "foreign private fragment"
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn format_provenance_hint_uses_source_and_time() {
        let provenance = FragmentProvenance {
            source_label: "Clipboard".to_string(),
            created: Utc.with_ymd_and_hms(2026, 6, 11, 14, 42, 0).unwrap(),
        };
        let hint = format_provenance_hint(&provenance, Tz::UTC);
        assert_eq!(hint, "Clipboard · 14:42");
    }
}

//! Day-page append API.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;

use super::io::{atomic_write, read_private_document, with_brain_write_lock};
use super::paths::BrainPaths;
use super::trash::trash_file;
use super::FragmentReference;

const FRAGMENT_CARD_TITLE: &str = "Fragment";
const FRAGMENT_CARD_LINK_LABEL: &str = "Open fragment";
const CLIPBOARD_REF_LABEL: &str = "Clipboard entry";

/// A single append-only entry for today's day page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DayEntry {
    /// Freeform capture text (dictation, `;note` body, short clipboard paste).
    Capture { text: String },
    /// Task line from `;todo` capture.
    Task {
        body: String,
        tags: Vec<String>,
        due: Option<String>,
    },
    /// Auto-kept URL from clipboard sediment.
    KeptUrl { url: String },
    /// Reference to a clipboard history row; persisted without raw clipboard text.
    ClipboardRef { entry_id: String },
    /// Reference to a long-capture fragment (excerpt + relative link).
    FragmentRef(FragmentReference),
    /// Agent Chat thread trace (one line per thread per day).
    Trace {
        summary: String,
        provenance_link: String,
    },
}

impl DayEntry {
    pub fn format_line(&self, timestamp: &str) -> String {
        match self {
            Self::Capture { text } => format!("{timestamp} {text}"),
            Self::Task { body, tags, due } => {
                let mut line = format!("{timestamp} - [ ] {body}");
                for tag in tags {
                    let normalized = tag.trim().trim_start_matches('#');
                    if !normalized.is_empty() {
                        line.push(' ');
                        line.push('#');
                        line.push_str(normalized);
                    }
                }
                if let Some(due) = due {
                    let due = due.trim();
                    if !due.is_empty() {
                        line.push_str(" due:");
                        line.push_str(due);
                    }
                }
                line
            }
            Self::KeptUrl { url } => {
                let label = markdown_url_label(url);
                format!("{timestamp} [{label}]({url})")
            }
            Self::ClipboardRef { entry_id } => {
                let uri = crate::clipboard_history::entry_resource_uri(entry_id);
                format!("{timestamp} [{CLIPBOARD_REF_LABEL}]({uri})")
            }
            Self::FragmentRef(reference) => format_fragment_reference_card(timestamp, reference),
            Self::Trace {
                summary,
                provenance_link,
            } => {
                let label = markdown_link_label(summary);
                format!("{timestamp} [{label}]({provenance_link})")
            }
        }
    }
}

fn format_fragment_reference_card(timestamp: &str, reference: &FragmentReference) -> String {
    format!(
        "{timestamp} {FRAGMENT_CARD_TITLE}\n> {}\n[{FRAGMENT_CARD_LINK_LABEL}]({})",
        reference.excerpt, reference.relative_link
    )
}

pub fn local_day_and_time(now: DateTime<Utc>, tz: Tz) -> (chrono::NaiveDate, String) {
    let local = now.with_timezone(&tz);
    let date = local.date_naive();
    let time = local.format("%H:%M").to_string();
    (date, time)
}

/// Remove day-page lines (and linked fragments) written by clipboard sediment.
pub fn undo_clipboard_sediment_lines(
    paths: &BrainPaths,
    tz: Tz,
    now: DateTime<Utc>,
    entry_id: &str,
    text: &str,
    kept_url_day: Option<&str>,
    brain_kept: bool,
) -> Result<()> {
    if !brain_kept && kept_url_day.is_none() {
        return Ok(());
    }

    // Hold the process-wide brain write lock across the whole read-modify-write
    // (multiple day-page filters plus fragment trashing) so a concurrent append
    // or editor save cannot interleave and resurrect a line we are removing.
    // The nested helpers use `atomic_write`/`trash_file`, neither of which takes
    // the lock, so this does not re-enter the non-reentrant mutex.
    with_brain_write_lock(|| {
        let source_uri = format!("scriptkit://clipboard/{entry_id}");
        let resource_uri = crate::clipboard_history::entry_resource_uri(entry_id);
        let trimmed = text.trim();

        if let Some(day) = kept_url_day {
            if let Ok(date) = NaiveDate::parse_from_str(day, "%Y-%m-%d") {
                remove_kept_url_line(paths, date, trimmed)?;
                remove_clipboard_ref_line(paths, date, &resource_uri)?;
            }
        }

        if brain_kept {
            let (today, _) = local_day_and_time(now, tz);
            remove_clipboard_ref_line(paths, today, &resource_uri)?;
            remove_capture_line(paths, today, trimmed)?;
            remove_fragment_reference_for_source(paths, today, &source_uri)?;
            trash_fragment_for_source(paths, &source_uri)?;
        }

        Ok(())
    })
}

fn remove_kept_url_line(paths: &BrainPaths, date: NaiveDate, url: &str) -> Result<()> {
    let path = paths.day_page(date);
    filter_day_page_lines(&path, |line| !line_contains_token(line, url))
}

fn remove_capture_line(paths: &BrainPaths, date: NaiveDate, text: &str) -> Result<()> {
    let path = paths.day_page(date);
    filter_day_page_lines(&path, |line| !line_contains_token(line, text))
}

fn remove_clipboard_ref_line(
    paths: &BrainPaths,
    date: NaiveDate,
    resource_uri: &str,
) -> Result<()> {
    let path = paths.day_page(date);
    filter_day_page_lines(&path, |line| !line_contains_token(line, resource_uri))
}

fn remove_fragment_reference_for_source(
    paths: &BrainPaths,
    date: NaiveDate,
    source_uri: &str,
) -> Result<()> {
    let path = paths.day_page(date);
    if !path.exists() {
        return Ok(());
    }

    let original =
        read_private_document(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut lines: Vec<&str> = original.lines().collect();
    let mut changed = false;
    let mut index = 0;
    while index < lines.len() {
        if let Some(line_count) =
            fragment_reference_line_count_for_source(paths, &lines, index, source_uri)
        {
            lines.drain(index..index + line_count);
            changed = true;
            continue;
        }
        index += 1;
    }

    if changed {
        write_filtered_lines(&path, lines)?;
    }
    Ok(())
}

fn fragment_reference_line_count_for_source(
    paths: &BrainPaths,
    lines: &[&str],
    index: usize,
    source_uri: &str,
) -> Option<usize> {
    let first = lines.get(index)?.trim();

    if is_fragment_card_header(first) && index + 2 < lines.len() {
        let quote = lines.get(index + 1)?.trim();
        if quote.strip_prefix('>')?.trim().is_empty() {
            return None;
        }
        return fragment_path_from_day_link(paths, lines.get(index + 2)?.trim())
            .filter(|path| fragment_has_source(path, source_uri))
            .map(|_| 3);
    }

    if parse_legacy_fragment_header(first).is_some() && index + 1 < lines.len() {
        return fragment_path_from_day_link(paths, lines.get(index + 1)?.trim())
            .filter(|path| fragment_has_source(path, source_uri))
            .map(|_| 2);
    }

    fragment_path_from_timestamped_markdown_fragment_link(paths, first)
        .filter(|path| fragment_has_source(path, source_uri))
        .map(|_| 1)
}

fn trash_fragment_for_source(paths: &BrainPaths, source_uri: &str) -> Result<()> {
    let fragments_dir = paths.fragments_dir();
    if !fragments_dir.exists() {
        return Ok(());
    }
    crate::atomic_file::ensure_private_directory(&fragments_dir).with_context(|| {
        format!(
            "preparing private fragments dir {}",
            fragments_dir.display()
        )
    })?;

    for entry in fs::read_dir(&fragments_dir)
        .with_context(|| format!("reading fragments dir {}", fragments_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if fragment_has_source(&path, source_uri) {
            let _ = trash_file(paths, &path);
        }
    }
    Ok(())
}

fn fragment_path_from_day_link(paths: &BrainPaths, link_line: &str) -> Option<PathBuf> {
    let trimmed = link_line.trim();
    let relative_link = parse_markdown_link_destination(trimmed).unwrap_or(trimmed);
    fragment_path_from_relative_link(paths, relative_link)
}

fn fragment_path_from_timestamped_markdown_fragment_link(
    paths: &BrainPaths,
    line: &str,
) -> Option<PathBuf> {
    let (timestamp, rest) = line.trim().split_once(' ')?;
    if !is_timestamp(timestamp) {
        return None;
    }
    let relative_link = parse_markdown_link_destination(rest.trim())?;
    fragment_path_from_relative_link(paths, relative_link)
}

fn fragment_path_from_relative_link(paths: &BrainPaths, relative_link: &str) -> Option<PathBuf> {
    let relative_link = relative_link.trim();
    if !relative_link.starts_with("../fragments/") || !relative_link.ends_with(".md") {
        return None;
    }
    let relative = relative_link.trim_start_matches("../fragments/");
    if relative.is_empty() {
        return None;
    }
    let fragment_id = relative.trim_end_matches(".md");
    Some(paths.fragment_file(fragment_id))
}

fn parse_markdown_link_destination(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let label_start = trimmed.find('[')?;
    if label_start != 0 {
        return None;
    }
    let label_end = trimmed[label_start + 1..].find("](")? + label_start + 1;
    let link_start = label_end + 2;
    let link_end = trimmed[link_start..].rfind(')')? + link_start;
    if link_end + 1 != trimmed.len() {
        return None;
    }
    let label = trimmed[label_start + 1..label_end].trim();
    let url = trimmed[link_start..link_end].trim();
    if label.is_empty() || url.is_empty() {
        return None;
    }
    Some(url)
}

fn parse_legacy_fragment_header(line: &str) -> Option<(&str, &str)> {
    let (timestamp, excerpt) = line.trim().split_once(" > ")?;
    if !is_timestamp(timestamp) {
        return None;
    }
    let excerpt = excerpt.trim();
    if excerpt.is_empty() {
        return None;
    }
    Some((timestamp, excerpt))
}

fn is_fragment_card_header(line: &str) -> bool {
    let Some((timestamp, title)) = line.trim().split_once(' ') else {
        return false;
    };
    is_timestamp(timestamp) && title.trim() == FRAGMENT_CARD_TITLE
}

fn is_timestamp(value: &str) -> bool {
    value.len() == 5
        && value.as_bytes().get(2) == Some(&b':')
        && value.chars().all(|ch| ch.is_ascii_digit() || ch == ':')
}

fn fragment_has_source(fragment_path: &Path, source_uri: &str) -> bool {
    read_private_document(fragment_path)
        .ok()
        .is_some_and(|content| content.contains(&format!("source: {source_uri}")))
}

fn filter_day_page_lines(path: &Path, keep: impl Fn(&str) -> bool) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        read_private_document(path).with_context(|| format!("reading {}", path.display()))?;
    let kept: Vec<&str> = original.lines().filter(|line| keep(line)).collect();
    if kept.len() == original.lines().count() {
        return Ok(());
    }
    write_filtered_lines(path, kept)
}

fn write_filtered_lines(path: &Path, lines: Vec<&str>) -> Result<()> {
    let mut contents = lines.join("\n");
    if !contents.is_empty() {
        contents.push('\n');
    }
    atomic_write(path, &contents)
}

fn line_contains_token(line: &str, token: &str) -> bool {
    let body = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(line);
    body.contains(token)
}

fn markdown_url_label(url: &str) -> String {
    let label = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let label = if label.is_empty() { url.trim() } else { label };
    markdown_link_label(label)
}

fn markdown_link_label(label: &str) -> String {
    label.replace('[', "\\[").replace(']', "\\]")
}

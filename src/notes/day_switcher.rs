use chrono::{DateTime, NaiveDate, Utc};
use std::fs;
use std::path::{Path, PathBuf};

use crate::actions::NoteSwitcherNoteInfo;

const DAY_NOTE_ID_PREFIX: &str = "day:";

#[derive(Debug, Clone)]
pub(crate) struct DayNoteSwitcherEntry {
    pub(crate) date: NaiveDate,
    pub(crate) path: PathBuf,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) updated_at: DateTime<Utc>,
}

pub(crate) fn day_note_action_id(date: NaiveDate) -> String {
    format!("{DAY_NOTE_ID_PREFIX}{date}")
}

pub(crate) fn parse_day_note_action_id(id: &str) -> Option<NaiveDate> {
    id.strip_prefix(DAY_NOTE_ID_PREFIX)
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
}

pub(crate) fn load_day_note_switcher_entries(days_dir: &Path) -> Vec<DayNoteSwitcherEntry> {
    load_day_note_switcher_entries_result(days_dir).unwrap_or_default()
}

/// Load day-note rows without collapsing an IO failure into an empty corpus.
///
/// The compatibility helper above retains the historical best-effort contract
/// for callers that have not adopted typed search state. Canonical Notes search
/// uses this strict result so a failed load remains distinguishable from an
/// honestly empty result set.
pub(crate) fn load_day_note_switcher_entries_result(
    days_dir: &Path,
) -> std::io::Result<Vec<DayNoteSwitcherEntry>> {
    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(days_dir) {
        Ok(read_dir) => read_dir,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
        Err(error) => return Err(error),
    };

    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") else {
            continue;
        };
        let content = fs::read_to_string(&path)?;
        let updated_at = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .map(DateTime::<Utc>::from)?;
        entries.push(DayNoteSwitcherEntry {
            date,
            path,
            title: day_note_title(date),
            content,
            updated_at,
        });
    }

    entries.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(entries)
}

pub(crate) fn day_note_switcher_infos(
    entries: &[DayNoteSwitcherEntry],
    current_date: Option<NaiveDate>,
) -> Vec<NoteSwitcherNoteInfo> {
    entries
        .iter()
        .map(|entry| NoteSwitcherNoteInfo {
            id: day_note_action_id(entry.date),
            title: entry.title.clone(),
            char_count: entry.content.chars().count(),
            is_current: Some(entry.date) == current_date,
            is_pinned: false,
            preview: day_note_preview(&entry.content),
            relative_time: crate::formatting::format_relative_time_short_dt(entry.updated_at),
        })
        .collect()
}

pub(crate) fn day_note_title(date: NaiveDate) -> String {
    format!("{} · {}", date, date.format("%A"))
}

fn day_note_preview(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(100)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_note_switcher_infos_use_shared_note_action_ids() {
        let dir = tempfile::tempdir().expect("tempdir");
        let days = dir.path().join("days");
        std::fs::create_dir_all(&days).expect("days dir");
        std::fs::write(days.join("2026-06-01.md"), "alpha day\nsecond line").expect("write day");

        let entries = load_day_note_switcher_entries(&days);
        let infos = day_note_switcher_infos(
            &entries,
            Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
        );

        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "day:2026-06-01");
        assert_eq!(infos[0].title, "2026-06-01 · Monday");
        assert!(infos[0].is_current);
        assert_eq!(
            parse_day_note_action_id(&infos[0].id),
            Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 1).expect("date"))
        );
    }
}

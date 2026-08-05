//! Canonical Notes search rows, ranking, typed state, and host destinations.
//!
//! Notes Window, Day Page, standalone Notes Browse, and the Agent Chat notes
//! portal share this data model. Hosts own what activating a row does; they do
//! not own a second search corpus or ranking implementation.

use std::cmp::Ordering;
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};

use crate::ai::reliability::AppFailureRecord;

use super::{Note, NoteId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NoteSearchDocumentId {
    Note(NoteId),
    Day(NaiveDate),
}

impl NoteSearchDocumentId {
    pub(crate) fn stable_id(self) -> String {
        match self {
            Self::Note(id) => id.as_str().to_string(),
            Self::Day(date) => super::day_switcher::day_note_action_id(date),
        }
    }

    pub(crate) fn action_id(self) -> String {
        format!("note_{}", self.stable_id())
    }

    pub(crate) fn semantic_id(self) -> String {
        format!("notes-search:{}", self.stable_id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum NoteSearchDocumentKind {
    Note,
    Day,
}

impl NoteSearchDocumentKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Day => "day",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteSearchRow {
    pub(crate) id: NoteSearchDocumentId,
    pub(crate) title: String,
    pub(crate) preview: String,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) char_count: usize,
    pub(crate) pinned: bool,
    pub(crate) kind: NoteSearchDocumentKind,
}

impl NoteSearchRow {
    pub(crate) fn stable_id(&self) -> String {
        self.id.stable_id()
    }

    pub(crate) fn action_id(&self) -> String {
        self.id.action_id()
    }

    pub(crate) fn semantic_id(&self) -> String {
        self.id.semantic_id()
    }

    pub(crate) fn relative_time(&self) -> String {
        crate::formatting::format_relative_time_short_dt(self.updated_at)
    }

    /// The exact searchable metadata used by both canonical ranking and the
    /// ActionsDialog-backed Notes/Today switchers.
    pub(crate) fn search_description(&self) -> String {
        let mut parts = Vec::with_capacity(4);
        if !self.preview.trim().is_empty() {
            parts.push(self.preview.clone());
        }
        parts.push(self.relative_time());
        parts.push(format!(
            "{} char{}",
            self.char_count,
            if self.char_count == 1 { "" } else { "s" }
        ));
        if self.pinned {
            parts.push("pinned".to_string());
        }
        parts.join(" · ")
    }

    /// Stable machine-readable metadata shared by every Notes search host.
    /// Unlike the visible relative-time copy, this never changes as wall clock
    /// time advances while a user moves between hosts.
    pub(crate) fn automation_metadata(&self) -> String {
        format!(
            "kind={};updatedAt={};charCount={};pinned={}",
            self.kind.as_str(),
            self.updated_at.to_rfc3339(),
            self.char_count,
            self.pinned,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoteSearchDestination {
    OpenInNotes,
    OpenHere,
    OpenInNotesWindow,
    AttachNote,
}

impl NoteSearchDestination {
    pub(crate) const fn primary_verb(self) -> &'static str {
        match self {
            Self::OpenInNotes => "Open in Notes",
            Self::OpenHere => "Open Here",
            Self::OpenInNotesWindow => "Open in Notes Window",
            Self::AttachNote => "Attach Note",
        }
    }

    pub(crate) const fn semantic_action(self) -> &'static str {
        match self {
            Self::OpenInNotes => "notes.search.open-in-notes",
            Self::OpenHere => "notes.search.open-here",
            Self::OpenInNotesWindow => "notes.search.open-in-notes-window",
            Self::AttachNote => "notes.search.attach-note",
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::OpenInNotes => "openInNotes",
            Self::OpenHere => "openHere",
            Self::OpenInNotesWindow => "openInNotesWindow",
            Self::AttachNote => "attachNote",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteSearchSnapshot {
    pub(crate) rows: Vec<NoteSearchRow>,
    pub(crate) total_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NoteSearchState {
    Loading {
        generation: u64,
        prior_snapshot: Option<NoteSearchSnapshot>,
    },
    Failed {
        generation: u64,
        failure: AppFailureRecord,
        prior_snapshot: Option<NoteSearchSnapshot>,
    },
    ReadyEmpty {
        generation: u64,
        corpus_empty: bool,
    },
    Ready {
        generation: u64,
        snapshot: NoteSearchSnapshot,
    },
}

impl NoteSearchState {
    pub(crate) fn generation(&self) -> u64 {
        match self {
            Self::Loading { generation, .. }
            | Self::Failed { generation, .. }
            | Self::ReadyEmpty { generation, .. }
            | Self::Ready { generation, .. } => *generation,
        }
    }

    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Loading { .. } => "loading",
            Self::Failed { .. } => "failed",
            Self::ReadyEmpty {
                corpus_empty: true, ..
            } => "readyEmpty",
            Self::ReadyEmpty {
                corpus_empty: false,
                ..
            } => "noMatch",
            Self::Ready { .. } => "ready",
        }
    }

    pub(crate) fn snapshot(&self) -> Option<&NoteSearchSnapshot> {
        match self {
            Self::Loading { prior_snapshot, .. } | Self::Failed { prior_snapshot, .. } => {
                prior_snapshot.as_ref()
            }
            Self::Ready { snapshot, .. } => Some(snapshot),
            Self::ReadyEmpty { .. } => None,
        }
    }

    pub(crate) fn rows(&self) -> &[NoteSearchRow] {
        self.snapshot()
            .map(|snapshot| snapshot.rows.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn failure(&self) -> Option<&AppFailureRecord> {
        match self {
            Self::Failed { failure, .. } => Some(failure),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteSearchHostState {
    pub(crate) query: String,
    pub(crate) selected_id: Option<NoteSearchDocumentId>,
    pub(crate) scroll_anchor: Option<NoteSearchDocumentId>,
    pub(crate) generation: u64,
    pub(crate) destination: NoteSearchDestination,
    pub(crate) state: NoteSearchState,
}

impl NoteSearchHostState {
    pub(crate) fn load(
        query: impl Into<String>,
        destination: NoteSearchDestination,
        days_dir: &Path,
    ) -> Self {
        let query = query.into();
        let generation = 1;
        let state = load_note_search_state(&query, days_dir, generation, None);
        let selected_id = state.rows().first().map(|row| row.id);
        Self {
            query,
            selected_id,
            scroll_anchor: selected_id,
            generation,
            destination,
            state,
        }
    }

    pub(crate) fn refresh(&mut self, query: impl Into<String>, days_dir: &Path) {
        let query = query.into();
        let prior_snapshot = self.state.snapshot().cloned();
        self.generation = self.generation.wrapping_add(1);
        self.query = query;
        self.state = load_note_search_state(
            &self.query,
            days_dir,
            self.generation,
            prior_snapshot.clone(),
        );
        let rows = self.state.rows();
        self.selected_id = self
            .selected_id
            .filter(|selected| rows.iter().any(|row| row.id == *selected))
            .or_else(|| rows.first().map(|row| row.id));
        self.scroll_anchor = self
            .scroll_anchor
            .filter(|anchor| rows.iter().any(|row| row.id == *anchor))
            .or(self.selected_id);
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected_id
            .and_then(|selected| self.state.rows().iter().position(|row| row.id == selected))
            .unwrap_or(0)
    }

    pub(crate) fn select_index(&mut self, index: usize) {
        if let Some(row) = self.state.rows().get(index) {
            self.selected_id = Some(row.id);
            self.scroll_anchor = Some(row.id);
        }
    }

    pub(crate) fn selected_row(&self) -> Option<&NoteSearchRow> {
        let selected = self.selected_id?;
        self.state.rows().iter().find(|row| row.id == selected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteSearchDocument {
    pub(crate) id: NoteSearchDocumentId,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) pinned: bool,
}

pub(crate) fn load_note_search_document(
    id: NoteSearchDocumentId,
    days_dir: &Path,
) -> Result<NoteSearchDocument, AppFailureRecord> {
    match id {
        NoteSearchDocumentId::Note(note_id) => match super::get_note(note_id) {
            Ok(Some(note)) => Ok(NoteSearchDocument {
                id,
                title: note_search_title(&note),
                content: note.content,
                updated_at: note.updated_at,
                pinned: note.is_pinned,
            }),
            Ok(None) => Err(search_failure("selected note no longer exists")),
            Err(error) => Err(search_failure(&error.to_string())),
        },
        NoteSearchDocumentId::Day(date) => {
            let path = days_dir.join(format!("{date}.md"));
            let content = std::fs::read_to_string(&path)
                .map_err(|error| search_failure(&error.to_string()))?;
            let updated_at = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .map(DateTime::<Utc>::from)
                .map_err(|error| search_failure(&error.to_string()))?;
            Ok(NoteSearchDocument {
                id,
                title: super::day_switcher::day_note_title(date),
                content,
                updated_at,
                pinned: false,
            })
        }
    }
}

pub(crate) fn load_note_search_state(
    query: &str,
    days_dir: &Path,
    generation: u64,
    prior_snapshot: Option<NoteSearchSnapshot>,
) -> NoteSearchState {
    // Deterministic fail-closed runtime seam for the C08 Driver matrix. It is
    // unreachable unless the app explicitly runs in test-status mode.
    if std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() == Some("1") {
        if query == "__notes_search_loading__" {
            return NoteSearchState::Loading {
                generation,
                prior_snapshot,
            };
        }
        if query == "__notes_search_failure__" {
            return NoteSearchState::Failed {
                generation,
                failure: search_failure("synthetic notes search failure"),
                prior_snapshot,
            };
        }
    }

    match load_note_search_corpus(days_dir) {
        Ok(rows) => {
            let total_count = rows.len();
            let rows = rank_note_search_rows(rows, query);
            if rows.is_empty() {
                NoteSearchState::ReadyEmpty {
                    generation,
                    corpus_empty: total_count == 0,
                }
            } else {
                NoteSearchState::Ready {
                    generation,
                    snapshot: NoteSearchSnapshot { rows, total_count },
                }
            }
        }
        Err(failure) => NoteSearchState::Failed {
            generation,
            failure,
            prior_snapshot,
        },
    }
}

pub(crate) fn load_note_search_corpus(
    days_dir: &Path,
) -> Result<Vec<NoteSearchRow>, AppFailureRecord> {
    let notes = super::get_all_notes().map_err(|error| search_failure(&error.to_string()))?;
    let days = super::day_switcher::load_day_note_switcher_entries_result(days_dir)
        .map_err(|error| search_failure(&error.to_string()))?;

    let mut rows = notes.into_iter().map(note_search_row).collect::<Vec<_>>();
    rows.extend(days.into_iter().map(|entry| NoteSearchRow {
        id: NoteSearchDocumentId::Day(entry.date),
        title: entry.title,
        preview: note_search_preview(&entry.content),
        updated_at: entry.updated_at,
        char_count: entry.content.chars().count(),
        pinned: false,
        kind: NoteSearchDocumentKind::Day,
    }));
    Ok(rows)
}

pub(crate) fn rank_note_search_rows(
    mut rows: Vec<NoteSearchRow>,
    query: &str,
) -> Vec<NoteSearchRow> {
    let query = query.trim();
    if query.is_empty() {
        rows.sort_by(note_search_default_order);
        return rows;
    }

    let mut scored = rows
        .into_iter()
        .filter_map(|row| note_search_score(&row, query).map(|score| (row, score)))
        .collect::<Vec<_>>();
    scored.sort_by(|(left_row, left_score), (right_row, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| note_search_default_order(left_row, right_row))
    });
    scored.into_iter().map(|(row, _)| row).collect()
}

fn note_search_score(row: &NoteSearchRow, query: &str) -> Option<i32> {
    let query_lower = query.to_lowercase();
    let query_char_count = query_lower.chars().count();
    let mut match_ctx = crate::scripts::search::SearchHighlightMatchCtx::new(query);
    let (title_matched, title_indices) = match_ctx.indices_for(&row.title);
    let mut score = if title_matched {
        title_match_tier(&title_indices, query_char_count)
    } else {
        0
    };
    if row
        .search_description()
        .to_lowercase()
        .contains(&query_lower)
    {
        score += 15;
    }
    (score > 0).then_some(score)
}

fn title_match_tier(indices: &[usize], query_char_count: usize) -> i32 {
    let contiguous = query_char_count > 0
        && indices.len() == query_char_count
        && indices.windows(2).all(|pair| pair[1] == pair[0] + 1);
    if contiguous && indices.first() == Some(&0) {
        100
    } else if contiguous {
        50
    } else {
        25
    }
}

fn note_search_default_order(left: &NoteSearchRow, right: &NoteSearchRow) -> Ordering {
    right
        .pinned
        .cmp(&left.pinned)
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.kind.cmp(&right.kind))
        .then_with(|| left.stable_id().cmp(&right.stable_id()))
}

fn note_search_row(note: Note) -> NoteSearchRow {
    NoteSearchRow {
        id: NoteSearchDocumentId::Note(note.id),
        title: note_search_title(&note),
        preview: note_search_preview(&note.content),
        updated_at: note.updated_at,
        char_count: note.char_count(),
        pinned: note.is_pinned,
        kind: NoteSearchDocumentKind::Note,
    }
}

fn note_search_title(note: &Note) -> String {
    if note.title.trim().is_empty() {
        "Untitled Note".to_string()
    } else {
        note.title.clone()
    }
}

fn note_search_preview(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(100)
        .collect()
}

fn search_failure(detail: &str) -> AppFailureRecord {
    crate::ai::reliability::context_unavailable_failure(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        id: NoteSearchDocumentId,
        title: &str,
        preview: &str,
        updated_at: &str,
        pinned: bool,
    ) -> NoteSearchRow {
        NoteSearchRow {
            id,
            title: title.to_string(),
            preview: preview.to_string(),
            updated_at: updated_at.parse().expect("timestamp"),
            char_count: preview.chars().count(),
            pinned,
            kind: match id {
                NoteSearchDocumentId::Note(_) => NoteSearchDocumentKind::Note,
                NoteSearchDocumentId::Day(_) => NoteSearchDocumentKind::Day,
            },
        }
    }

    fn note_id(value: &str) -> NoteId {
        NoteId::parse(value).expect("note id")
    }

    #[test]
    fn canonical_row_owns_untitled_copy_and_stable_semantics() {
        let note = Note::new();
        let row = note_search_row(note);
        assert_eq!(row.title, "Untitled Note");
        assert_eq!(
            row.semantic_id(),
            format!("notes-search:{}", row.stable_id())
        );
        assert_eq!(row.action_id(), format!("note_{}", row.stable_id()));
    }

    #[test]
    fn four_hosts_share_closed_destination_verbs() {
        assert_eq!(
            [
                NoteSearchDestination::OpenInNotes,
                NoteSearchDestination::OpenHere,
                NoteSearchDestination::OpenInNotesWindow,
                NoteSearchDestination::AttachNote,
            ]
            .map(NoteSearchDestination::primary_verb),
            [
                "Open in Notes",
                "Open Here",
                "Open in Notes Window",
                "Attach Note",
            ]
        );
    }

    #[test]
    fn regular_and_day_rows_use_one_rank_and_stable_tiebreak() {
        let rows = vec![
            row(
                NoteSearchDocumentId::Day(NaiveDate::from_ymd_opt(2026, 7, 1).expect("date")),
                "Meeting day",
                "shared alpha phrase",
                "2026-07-01T09:00:00Z",
                false,
            ),
            row(
                NoteSearchDocumentId::Note(note_id("00000000-0000-0000-0000-000000000001")),
                "Meeting note",
                "shared alpha phrase",
                "2026-07-02T09:00:00Z",
                false,
            ),
            row(
                NoteSearchDocumentId::Note(note_id("00000000-0000-0000-0000-000000000002")),
                "Other",
                "meeting appears in metadata",
                "2026-07-03T09:00:00Z",
                true,
            ),
        ];

        let ranked = rank_note_search_rows(rows, "meeting");
        assert_eq!(
            ranked
                .iter()
                .map(NoteSearchRow::stable_id)
                .collect::<Vec<_>>(),
            vec![
                "00000000-0000-0000-0000-000000000001".to_string(),
                "day:2026-07-01".to_string(),
                "00000000-0000-0000-0000-000000000002".to_string(),
            ]
        );
    }

    #[test]
    fn failed_refresh_keeps_prior_snapshot_and_selected_id() {
        let selected = NoteSearchDocumentId::Note(note_id("00000000-0000-0000-0000-000000000001"));
        let snapshot = NoteSearchSnapshot {
            rows: vec![row(
                selected,
                "Alpha",
                "preview",
                "2026-07-02T09:00:00Z",
                false,
            )],
            total_count: 1,
        };
        let state = NoteSearchState::Failed {
            generation: 2,
            failure: search_failure("synthetic search failure"),
            prior_snapshot: Some(snapshot.clone()),
        };
        assert_eq!(state.kind(), "failed");
        assert_eq!(state.rows(), snapshot.rows);
        assert_eq!(
            state.failure().map(|failure| failure.failure.code),
            Some(sk_protocol::ai_reliability::AiFailureCode::ContextUnavailable)
        );
    }

    #[test]
    fn selection_reconciles_by_stable_id_not_stale_index() {
        let first = NoteSearchDocumentId::Note(note_id("00000000-0000-0000-0000-000000000001"));
        let second = NoteSearchDocumentId::Note(note_id("00000000-0000-0000-0000-000000000002"));
        let mut host = NoteSearchHostState {
            query: String::new(),
            selected_id: Some(second),
            scroll_anchor: Some(second),
            generation: 1,
            destination: NoteSearchDestination::OpenInNotesWindow,
            state: NoteSearchState::Ready {
                generation: 1,
                snapshot: NoteSearchSnapshot {
                    rows: vec![
                        row(first, "Alpha", "", "2026-07-01T09:00:00Z", false),
                        row(second, "Beta", "", "2026-07-02T09:00:00Z", false),
                    ],
                    total_count: 2,
                },
            },
        };
        host.state = NoteSearchState::Ready {
            generation: 2,
            snapshot: NoteSearchSnapshot {
                rows: vec![row(second, "Beta", "", "2026-07-02T09:00:00Z", false)],
                total_count: 1,
            },
        };
        assert_eq!(host.selected_index(), 0);
        assert_eq!(host.selected_row().map(|row| row.id), Some(second));
    }
}

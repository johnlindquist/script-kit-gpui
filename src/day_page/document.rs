//! Bind, save, and external-refresh rules for `brain/days/YYYY-MM-DD.md`.
//!
//! ## External append while the Day Page is open
//!
//! When another writer appends to today's file (for example `;todo`), this
//! session polls the on-disk modification time on [`maybe_refresh_from_disk`].
//! If the editor buffer is **clean** (`!dirty`), the file is re-read and the
//! editor content is replaced. If the buffer is **dirty**, the append still
//! lands in the file but the open editor keeps the in-progress edit; the file
//! on disk is refreshed on the next [`bind_today`] (re-entry or day rollover),
//! which saves any dirty buffer first then reloads from disk.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{Context as _, Result};
use chrono::{DateTime, NaiveDate, Utc};

use crate::brain::substrate::{io, BrainSubstrate};

/// Which markdown file the Day Page editor is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DayPageBinding {
    Day,
    Note {
        note_id: String,
        title: String,
        return_day_path: PathBuf,
        return_day_date: NaiveDate,
    },
    Fragment {
        fragment_path: PathBuf,
        return_day_path: PathBuf,
        return_day_date: NaiveDate,
    },
}

/// Substrate-backed session for a single day-page markdown file.
#[derive(Debug, Clone)]
pub struct DayPageDocumentSession {
    substrate: BrainSubstrate,
    bound_date: Option<NaiveDate>,
    path: Option<PathBuf>,
    binding: DayPageBinding,
    dirty: bool,
    disk_content: String,
    /// The content we last observed on disk (or last wrote). Unlike
    /// `disk_content`, this is NEVER replaced by editor input, so a save can
    /// diff the real on-disk file against it to detect external appends made
    /// while the editor was open.
    base_disk_content: String,
    last_mtime: Option<SystemTime>,
    /// Set by `save_content` when a save had to merge external appends (or
    /// resolve a conflict) rather than write the editor buffer verbatim. Holds
    /// the content actually written to disk so a host can adopt it into the
    /// editor; left `None` for a clean save.
    last_save_merged: Option<String>,
    /// True only for the today-following `Day` binding — one bound to the local
    /// date as it was at bind time (`bind_today`, or `return_to_day` back onto
    /// today). Explicitly-opened past days, notes, and fragments are never
    /// today-following, so [`DayPageDocumentSession::day_has_rolled`] leaves
    /// them bound across midnight instead of dragging them onto the new day.
    follows_today: bool,
}

impl DayPageDocumentSession {
    pub fn new(substrate: BrainSubstrate) -> Self {
        Self {
            substrate,
            bound_date: None,
            path: None,
            binding: DayPageBinding::Day,
            dirty: false,
            disk_content: String::new(),
            base_disk_content: String::new(),
            last_mtime: None,
            last_save_merged: None,
            follows_today: false,
        }
    }

    pub fn binding(&self) -> &DayPageBinding {
        &self.binding
    }

    pub fn is_viewing_fragment(&self) -> bool {
        matches!(self.binding, DayPageBinding::Fragment { .. })
    }

    pub fn is_viewing_note(&self) -> bool {
        matches!(self.binding, DayPageBinding::Note { .. })
    }

    pub fn viewing_note_id(&self) -> Option<&str> {
        match &self.binding {
            DayPageBinding::Note { note_id, .. } => Some(note_id.as_str()),
            _ => None,
        }
    }

    pub fn viewing_note_title(&self) -> Option<&str> {
        match &self.binding {
            DayPageBinding::Note { title, .. } => Some(title.as_str()),
            _ => None,
        }
    }

    pub fn substrate(&self) -> &BrainSubstrate {
        &self.substrate
    }

    pub fn bound_date(&self) -> Option<NaiveDate> {
        self.bound_date
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn disk_content(&self) -> &str {
        &self.disk_content
    }

    /// The content written by the last `save_content` when it had to merge
    /// external appends or resolve a conflict, so a host can adopt it into the
    /// editor. `None` after a clean save.
    pub fn last_save_merged(&self) -> Option<&str> {
        self.last_save_merged.as_deref()
    }

    /// Bind to today's day page for `now` (timezone from substrate). Creates an
    /// empty file when missing. Day rollover binds a new path when the local
    /// date changes.
    pub fn bind_today(&mut self, now: DateTime<Utc>) -> Result<String> {
        let date = now.with_timezone(&self.substrate.timezone()).date_naive();
        self.bind_date(date, now)
    }

    /// True when this document follows "today" but the local day (resolved in
    /// the substrate timezone, the same logic `bind_today` uses) has rolled past
    /// the bound date — the view should save the pre-midnight buffer to the old
    /// day, then rebind to the new day. Never true for an explicitly-opened past
    /// day, a note, or a fragment; those stay put across midnight.
    pub fn day_has_rolled(&self, now: DateTime<Utc>) -> bool {
        if !self.follows_today {
            return false;
        }
        let Some(bound) = self.bound_date else {
            return false;
        };
        let local_today = now.with_timezone(&self.substrate.timezone()).date_naive();
        local_today != bound
    }

    pub fn bind_date(&mut self, date: NaiveDate, now: DateTime<Utc>) -> Result<String> {
        if self.dirty {
            match self.bound_date {
                // Same-day re-bind refreshes from disk so external appends (e.g. `;todo`)
                // appear even when the user had an in-progress edit.
                Some(bound) if bound == date => {}
                _ => self.save(now)?,
            }
        }

        let path = self.substrate.paths().day_page(date);

        // Create-if-missing + initial read under the process-wide brain write
        // lock. Without it, an external appender (`atomic_append_line`, which
        // holds the same lock) can write the first capture of the day between
        // our exists check and the empty create, and the create then clobbers
        // it. Re-test existence INSIDE the lock so we never overwrite a file an
        // appender created first. `atomic_write` intentionally does not take the
        // lock, so calling it here does not re-enter the non-reentrant mutex.
        let content = io::with_brain_write_lock(|| -> Result<String> {
            if io::read_private_document_if_present(&path)?.is_none() {
                io::atomic_write(&path, "")
                    .with_context(|| format!("creating day page {}", path.display()))?;
            }
            io::read_private_document(&path)
                .with_context(|| format!("reading day page {}", path.display()))
        })?;
        let mtime = fs::metadata(&path).and_then(|meta| meta.modified()).ok();

        let local_today = now.with_timezone(&self.substrate.timezone()).date_naive();
        self.bound_date = Some(date);
        self.path = Some(path);
        self.binding = DayPageBinding::Day;
        self.follows_today = date == local_today;
        self.dirty = false;
        self.disk_content = content.clone();
        self.base_disk_content = content.clone();
        self.last_mtime = mtime;

        super::telemetry::log_document_loaded(
            "document::bind_date",
            "day",
            Some(&date.to_string()),
            content.len(),
        );
        Ok(content)
    }

    /// Bind the editor to a fragment file opened from today's day page.
    pub fn bind_fragment(&mut self, fragment_path: PathBuf, now: DateTime<Utc>) -> Result<String> {
        let fragments_dir = self.substrate.paths().fragments_dir();
        if !self.substrate.paths().contains(&fragment_path)
            || fragment_path.parent() != Some(fragments_dir.as_path())
        {
            anyhow::bail!(
                "fragment must belong to the private brain fragments directory: {}",
                fragment_path.display()
            );
        }
        let day_path = self
            .path
            .clone()
            .with_context(|| "fragment open without day bind")?;
        let day_date = self
            .bound_date
            .with_context(|| "fragment open without day date")?;

        if self.dirty {
            self.save(now)?;
        }

        let content = io::read_private_document(&fragment_path)
            .with_context(|| format!("reading fragment {}", fragment_path.display()))?;
        let mtime = fs::metadata(&fragment_path)
            .and_then(|meta| meta.modified())
            .ok();

        self.path = Some(fragment_path.clone());
        self.binding = DayPageBinding::Fragment {
            fragment_path,
            return_day_path: day_path,
            return_day_date: day_date,
        };
        self.follows_today = false;
        self.dirty = false;
        self.disk_content = content.clone();
        self.base_disk_content = content.clone();
        self.last_mtime = mtime;

        super::telemetry::log_document_loaded(
            "document::bind_fragment",
            "fragment",
            Some(&day_date.to_string()),
            content.len(),
        );
        Ok(content)
    }

    fn return_day_anchor(&self, context: &'static str) -> Result<(PathBuf, NaiveDate)> {
        match &self.binding {
            DayPageBinding::Day => {
                let path = self
                    .path
                    .clone()
                    .with_context(|| format!("{context} without day bind"))?;
                let date = self
                    .bound_date
                    .with_context(|| format!("{context} without day date"))?;
                Ok((path, date))
            }
            DayPageBinding::Note {
                return_day_path,
                return_day_date,
                ..
            }
            | DayPageBinding::Fragment {
                return_day_path,
                return_day_date,
                ..
            } => Ok((return_day_path.clone(), *return_day_date)),
        }
    }

    /// Bind the editor to a regular Notes document while keeping the Day Page
    /// surface local to the main window.
    pub fn bind_note_content(
        &mut self,
        note_id: String,
        title: String,
        content: String,
        path: Option<PathBuf>,
        now: DateTime<Utc>,
    ) -> Result<String> {
        let (day_path, day_date) = self.return_day_anchor("note open")?;

        if self.dirty {
            self.save(now)?;
        }

        let mtime = path
            .as_ref()
            .and_then(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok());

        self.path = path;
        self.bound_date = None;
        self.binding = DayPageBinding::Note {
            note_id,
            title: if title.trim().is_empty() {
                "Untitled Note".to_string()
            } else {
                title
            },
            return_day_path: day_path,
            return_day_date: day_date,
        };
        self.follows_today = false;
        self.dirty = false;
        self.disk_content = content.clone();
        self.base_disk_content = content.clone();
        self.last_mtime = mtime;

        super::telemetry::log_document_loaded(
            "document::bind_note_content",
            "note",
            None,
            content.len(),
        );
        Ok(content)
    }

    /// Return from an inline fragment view back to the bound day page.
    pub fn return_to_day(&mut self, now: DateTime<Utc>) -> Result<String> {
        let (return_day_path, return_day_date) = match &self.binding {
            DayPageBinding::Fragment {
                return_day_path,
                return_day_date,
                ..
            }
            | DayPageBinding::Note {
                return_day_path,
                return_day_date,
                ..
            } => (return_day_path.clone(), *return_day_date),
            DayPageBinding::Day => return self.bind_today(now),
        };

        if self.dirty {
            self.save(now)?;
        }

        let content = io::read_private_document(&return_day_path)
            .with_context(|| format!("reading day page {}", return_day_path.display()))?;
        let mtime = fs::metadata(&return_day_path)
            .and_then(|meta| meta.modified())
            .ok();

        let local_today = now.with_timezone(&self.substrate.timezone()).date_naive();
        self.path = Some(return_day_path);
        self.bound_date = Some(return_day_date);
        self.binding = DayPageBinding::Day;
        self.follows_today = return_day_date == local_today;
        self.dirty = false;
        self.disk_content = content.clone();
        self.base_disk_content = content.clone();
        self.last_mtime = mtime;

        super::telemetry::log_document_loaded(
            "document::return_to_day",
            "day",
            Some(&return_day_date.to_string()),
            content.len(),
        );
        Ok(content)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Write `content` through the substrate atomic writer when dirty.
    pub fn save_content(&mut self, content: &str, now: DateTime<Utc>) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let note_binding_id = match &self.binding {
            DayPageBinding::Note { note_id, .. } => Some(note_id.clone()),
            _ => None,
        };

        if let Some(note_id_text) = note_binding_id {
            let note_id = crate::notes::NoteId::parse(&note_id_text)
                .with_context(|| format!("parsing day page note id {note_id_text}"))?;
            crate::notes::init_notes_db().context("initializing notes before day page save")?;
            let mut note = crate::notes::get_note(note_id)?
                .with_context(|| format!("loading note before day page save {note_id}"))?;
            note.set_content(content);
            crate::notes::save_note(&note)
                .with_context(|| format!("saving note from day page {note_id}"))?;
            let saved_path = crate::notes::note_file_path(note.id)?
                .with_context(|| format!("resolving saved note path {note_id}"))?;
            if let DayPageBinding::Note { title, .. } = &mut self.binding {
                *title = note.title.clone();
            }
            self.path = Some(saved_path.clone());
            self.dirty = false;
            self.disk_content = content.to_string();
            self.base_disk_content = content.to_string();
            self.last_save_merged = None;
            self.last_mtime = fs::metadata(&saved_path)
                .and_then(|meta| meta.modified())
                .ok();
            super::telemetry::log_document_saved(
                "document::save_content",
                "note",
                content.len(),
                false,
            );
            let _ = now;
            return Ok(());
        }

        let path = self
            .path
            .clone()
            .with_context(|| "day page save without bind")?;

        // Serialize the read-of-disk + write against background appenders
        // (`;todo`, clipboard sediment, dictation) that also mutate this file.
        // Compare the real on-disk content to the baseline we last saw: if an
        // external writer appended lines since then, MERGE them instead of
        // blindly overwriting, so a capture that landed during the autosave
        // debounce is never silently lost.
        let written = io::with_brain_write_lock(|| -> Result<Written> {
            let disk_now = io::read_private_document_if_present(&path)?.unwrap_or_default();

            if disk_now == self.base_disk_content {
                io::atomic_write(&path, content)
                    .with_context(|| format!("writing day page {}", path.display()))?;
                return Ok(Written::clean(content.to_string()));
            }

            if let Some(suffix) = external_append_suffix(&disk_now, &self.base_disk_content) {
                let merged = merge_editor_with_external_appends(content, suffix, &disk_now);
                io::atomic_write(&path, &merged)
                    .with_context(|| format!("writing merged day page {}", path.display()))?;
                return Ok(Written::merged(merged));
            }

            // Non-append divergence (an external edit rewrote earlier content).
            // Keep both versions: the editor buffer wins the bound file, and the
            // on-disk version is copied to the brain trash for recovery.
            let conflict_path = io::preserve_private_conflict_copy(&path, &disk_now)?;
            let safe_path = crate::logging::log_private_user_value(&path.display().to_string());
            let safe_conflict =
                crate::logging::log_private_user_value(&conflict_path.display().to_string());
            tracing::warn!(
                target: "script_kit::brain",
                path_bytes = safe_path.raw_bytes,
                path_sha256 = %safe_path.sha256,
                conflict_bytes = safe_conflict.raw_bytes,
                conflict_sha256 = %safe_conflict.sha256,
                "day page diverged on disk in a non-append way; kept editor buffer and copied the disk version to trash"
            );
            io::atomic_write(&path, content)
                .with_context(|| format!("writing day page {}", path.display()))?;
            Ok(Written::merged(content.to_string()))
        })?;

        self.dirty = false;
        self.disk_content = written.content.clone();
        self.base_disk_content = written.content.clone();
        if written.adopted {
            // The editor buffer no longer matches disk. Force the next disk poll
            // to re-read and adopt the written content by clearing the mtime
            // fingerprint, and expose it via `last_save_merged` for hosts that
            // want to adopt it immediately.
            self.last_mtime = None;
            self.last_save_merged = Some(written.content);
        } else {
            self.last_mtime = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
            self.last_save_merged = None;
        }

        // Editor saves write the day/fragment file directly via io::atomic_write
        // (not through the substrate append wrappers), so index it here too — a
        // Day Page edit is a capture and must be recallable without waiting for
        // the next indexer cycle. cfg(test) no-op; classifies days/ vs
        // fragments/ by the file's parent directory.
        crate::brain::indexer::index_capture_after_write(&path);

        let kind = match &self.binding {
            DayPageBinding::Fragment { .. } => "fragment",
            _ => "day",
        };
        super::telemetry::log_document_saved(
            "document::save_content",
            kind,
            self.disk_content.len(),
            written.adopted,
        );
        let _ = now;
        Ok(())
    }

    pub fn save(&mut self, now: DateTime<Utc>) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let content = self.disk_content.clone();
        self.save_content(&content, now)
    }

    /// Apply editor buffer; marks the session dirty when content diverges from disk.
    pub fn apply_editor_content(&mut self, content: &str) {
        if content != self.disk_content {
            self.disk_content = content.to_string();
            self.dirty = true;
        }
    }

    /// Align the session with authoritative on-disk content after an external
    /// append (for example hold-to-talk dictation via substrate `append_to_day`).
    pub fn adopt_disk_content_after_external_write(&mut self, content: String) -> Result<()> {
        let path = self
            .path
            .clone()
            .with_context(|| "adopt disk content without bind")?;
        self.disk_content = content.clone();
        self.base_disk_content = content;
        self.dirty = false;
        self.last_mtime = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        Ok(())
    }

    /// Re-read from disk when the file changed externally and the buffer is clean.
    pub fn maybe_refresh_from_disk(&mut self) -> Result<Option<String>> {
        let path = match self.path.as_ref() {
            Some(path) => path.clone(),
            None => return Ok(None),
        };

        let mtime = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        if mtime == self.last_mtime {
            return Ok(None);
        }

        if self.dirty {
            return Ok(None);
        }

        // The bound file vanished (deleted or renamed away externally) while
        // the buffer is clean: the editor now holds the only copy. Same
        // divergence philosophy as `save_content` — the editor buffer wins —
        // so re-dirty the session and let the next autosave recreate the file.
        // Without this, the session silently tracks a missing path and the
        // next re-bind discards the buffer by creating an empty day file.
        if mtime.is_none() && !path.exists() {
            if self.disk_content.trim().is_empty() {
                return Ok(None);
            }
            self.dirty = true;
            tracing::warn!(
                target: "script_kit::brain",
                path = %path.display(),
                "bound day page file vanished while clean; keeping the editor buffer and re-dirtying so autosave recreates it"
            );
            return Ok(None);
        }

        let content = io::read_private_document(&path)
            .with_context(|| format!("re-reading day page {}", path.display()))?;
        self.disk_content = content.clone();
        self.base_disk_content = content.clone();
        self.last_mtime = mtime;
        self.dirty = false;
        self.last_save_merged = None;

        Ok(Some(content))
    }

    /// Simulate an external append for tests — updates mtime without going through the session.
    pub fn simulate_external_append_for_test(&mut self, line: &str) -> Result<()> {
        let path = self
            .path
            .clone()
            .with_context(|| "external append without bind")?;
        io::atomic_append_line(&path, line)?;
        Ok(())
    }

    /// Append to the currently bound day file without mutating the in-memory
    /// editor session. Hosts use this for external writers that should be
    /// picked up by `maybe_refresh_from_disk` after the editor surface returns.
    pub fn append_external_line_to_bound_file(&self, line: &str) -> Result<()> {
        let path = self
            .path
            .clone()
            .with_context(|| "external append without bind")?;
        io::atomic_append_line(&path, line)
    }
}

/// Outcome of a day-file save: the bytes actually written to disk, and whether
/// they diverge from the editor buffer (a merge or conflict) so the editor must
/// re-adopt them.
struct Written {
    content: String,
    adopted: bool,
}

impl Written {
    fn clean(content: String) -> Self {
        Self {
            content,
            adopted: false,
        }
    }

    fn merged(content: String) -> Self {
        Self {
            content,
            adopted: true,
        }
    }
}

/// If `disk_now` is `base` followed by extra appended content, return that
/// suffix. Returns `None` when disk is not a pure append over the baseline
/// (an external edit, truncation, or no real added content).
fn external_append_suffix<'a>(disk_now: &'a str, base: &str) -> Option<&'a str> {
    let suffix = disk_now.strip_prefix(base.trim_end())?;
    if suffix.trim().is_empty() {
        return None;
    }
    Some(suffix)
}

/// Combine the editor buffer with external appends detected on disk, preserving
/// a single joining newline and the trailing newline if the disk file had one.
fn merge_editor_with_external_appends(content: &str, suffix: &str, disk_now: &str) -> String {
    let mut merged = content.trim_end().to_string();
    merged.push('\n');
    merged.push_str(suffix.trim_start_matches('\n'));
    if disk_now.ends_with('\n') && !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Tz;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_session() -> (tempfile::TempDir, DayPageDocumentSession) {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate = BrainSubstrate::with_timezone(dir.path().join("brain"), Tz::UTC);
        (dir, DayPageDocumentSession::new(substrate))
    }

    fn utc(now: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(now)
            .expect("parse time")
            .with_timezone(&Utc)
    }

    fn notes_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn unique_note_content(label: &str) -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        format!("# {label} {millis}\n\nbody {}", crate::notes::NoteId::new())
    }

    fn saved_note(content: &str) -> crate::notes::Note {
        let note = crate::notes::Note::with_content(content);
        crate::notes::save_note(&note).expect("save note");
        note
    }

    #[test]
    fn bind_today_creates_file() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");

        let content = session.bind_today(now).expect("bind");
        assert_eq!(content, "");
        assert!(session.path().expect("path").exists());
        assert_eq!(session.bound_date(), Some(now.date_naive()));
    }

    #[cfg(unix)]
    #[test]
    fn day_page_private_bind_rejects_symlinked_day_files_without_reading_foreign_text() {
        use std::os::unix::fs::symlink;

        let (fixture, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        let foreign = fixture.path().join("foreign-day.md");
        fs::write(&foreign, "foreign private day data").unwrap();
        let planted = session.substrate().paths().day_page(now.date_naive());
        fs::create_dir_all(planted.parent().unwrap()).unwrap();
        symlink(&foreign, &planted).expect("plant hostile day-page symlink");

        assert!(session.bind_today(now).is_err());
        assert_eq!(
            fs::read_to_string(foreign).unwrap(),
            "foreign private day data"
        );
        assert!(fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(session.path().is_none());
    }

    #[test]
    fn day_page_private_fragment_binding_refuses_paths_outside_owned_fragments() {
        let (fixture, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind owned day page");
        let original = session.path().cloned();
        let foreign = fixture.path().join("foreign-fragment.md");
        fs::write(&foreign, "foreign private fragment").unwrap();

        assert!(session.bind_fragment(foreign.clone(), now).is_err());
        assert_eq!(session.path(), original.as_ref());
        assert!(!session.is_viewing_fragment());
        assert_eq!(
            fs::read_to_string(foreign).unwrap(),
            "foreign private fragment"
        );
    }

    #[test]
    fn save_persists_through_substrate() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind");

        session.apply_editor_content("morning thought");
        session.save_content("morning thought", now).expect("save");

        let disk = fs::read_to_string(session.path().expect("path")).expect("read");
        assert_eq!(disk, "morning thought");
        assert!(!session.is_dirty());
    }

    #[test]
    fn day_rollover_binds_new_file() {
        let (_dir, mut session) = test_session();
        let day_one = utc("2026-06-11T23:59:00Z");
        let day_two = utc("2026-06-12T00:01:00Z");

        session.bind_today(day_one).expect("bind day one");
        session.apply_editor_content("june 11");
        session.save_content("june 11", day_one).expect("save");

        let content = session.bind_today(day_two).expect("bind day two");
        assert_eq!(content, "");
        assert_eq!(session.bound_date(), Some(day_two.date_naive()));
        assert_ne!(
            session.path().expect("path"),
            &session.substrate().paths().day_page(day_one.date_naive())
        );
    }

    #[test]
    fn day_has_rolled_false_within_the_bound_day() {
        let (_dir, mut session) = test_session();
        let bind = utc("2026-06-11T09:42:00Z");
        session.bind_today(bind).expect("bind");
        assert!(!session.day_has_rolled(bind), "no roll at bind time");
        assert!(
            !session.day_has_rolled(utc("2026-06-11T23:59:00Z")),
            "no roll later on the same local day"
        );
    }

    #[test]
    fn day_has_rolled_true_after_local_midnight() {
        let (_dir, mut session) = test_session();
        // Bind just before local midnight (UTC substrate), then cross it.
        session
            .bind_today(utc("2026-06-11T23:59:00Z"))
            .expect("bind");
        assert!(
            session.day_has_rolled(utc("2026-06-12T00:01:00Z")),
            "an open today-following page must report a roll after midnight"
        );
    }

    #[test]
    fn day_has_rolled_false_for_note_binding() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind day");
        session
            .bind_note_content(
                "note-id".to_string(),
                "Note Title".to_string(),
                "body".to_string(),
                None,
                now,
            )
            .expect("bind note");
        assert!(
            !session.day_has_rolled(utc("2026-06-13T00:01:00Z")),
            "a note binding never rolls with the day"
        );
    }

    #[test]
    fn day_has_rolled_false_for_explicitly_opened_past_day() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        let past = NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date");
        session.bind_date(past, now).expect("bind past day");
        assert!(
            !session.day_has_rolled(now),
            "an explicitly-opened past day is not today-following"
        );
        assert!(
            !session.day_has_rolled(utc("2026-06-12T00:01:00Z")),
            "a past day must not roll even as wall-clock time advances"
        );
    }

    /// Data-loss regression (Plan 01 follow-up): binding a fresh day must not
    /// clobber a first-of-day capture that an external appender writes at the
    /// same moment. Pre-fix, bind_date's exists-check/create/read ran OUTSIDE the
    /// brain write lock, so an `atomic_append_line` landing between the check and
    /// the empty create was overwritten. Both now share the lock, so the append
    /// always survives. Looped so the interleaving is exercised repeatedly.
    #[test]
    fn bind_date_does_not_clobber_concurrent_first_capture() {
        use std::sync::{Arc, Barrier};

        let dir = tempfile::tempdir().expect("tempdir");
        let substrate = BrainSubstrate::with_timezone(dir.path().join("brain"), Tz::UTC);
        let now = utc("2026-06-11T09:42:00Z");

        for i in 0..50u32 {
            // A distinct fresh day per iteration so the file starts absent and
            // the create path is exercised every time.
            let date = NaiveDate::from_yo_opt(2026, 1 + i).expect("valid ordinal date");
            let path = substrate.paths().day_page(date);
            let line = format!("09:45 first capture {i}");

            let barrier = Arc::new(Barrier::new(2));
            let appender = {
                let path = path.clone();
                let line = line.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    io::atomic_append_line(&path, &line).expect("external append");
                })
            };

            let mut session = DayPageDocumentSession::new(substrate.clone());
            barrier.wait();
            session.bind_date(date, now).expect("bind fresh day");
            appender.join().expect("appender thread joined");

            let disk = fs::read_to_string(&path).expect("read day file");
            assert!(
                disk.contains(&line),
                "iteration {i}: first-of-day capture must survive bind_date create: {disk:?}"
            );
        }
    }

    #[test]
    fn binding_different_day_saves_dirty_current_day_first() {
        let (_dir, mut session) = test_session();
        let day_one = utc("2026-06-11T09:42:00Z");
        let day_two = utc("2026-06-12T09:42:00Z");
        session.bind_today(day_one).expect("bind day one");
        let day_one_path = session.path().expect("day one path").clone();

        session.apply_editor_content("dirty before day switch");
        session
            .bind_today(day_two)
            .expect("switching days saves dirty original");

        assert_eq!(
            fs::read_to_string(day_one_path).expect("read original day"),
            "dirty before day switch"
        );
        assert_eq!(session.bound_date(), Some(day_two.date_naive()));
        assert!(!session.is_dirty());
    }

    #[test]
    fn binding_regular_note_saves_dirty_day_before_local_switch() {
        let (dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind day");
        let day_path = session.path().expect("day path").clone();
        let note_path = dir.path().join("note.md");
        fs::write(&note_path, "note body").expect("write note path");

        session.apply_editor_content("dirty before note switch");
        session
            .bind_note_content(
                "note-id".to_string(),
                "Note Title".to_string(),
                "note body".to_string(),
                Some(note_path),
                now,
            )
            .expect("switch to note");

        assert_eq!(
            fs::read_to_string(day_path).expect("read day"),
            "dirty before note switch"
        );
        assert!(session.is_viewing_note());
        assert!(!session.is_dirty());
    }

    #[test]
    fn regular_note_save_uses_notes_storage_and_updates_index() {
        let _guard = notes_test_guard();
        crate::notes::init_notes_db().expect("init notes db");
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind day");

        let note = saved_note(&unique_note_content("day-page-save-original"));
        let note_path = crate::notes::note_file_path(note.id)
            .expect("note path result")
            .expect("note path");
        session
            .bind_note_content(
                note.id.as_str().to_string(),
                note.title.clone(),
                note.content.clone(),
                Some(note_path.clone()),
                now,
            )
            .expect("bind note");

        let updated = unique_note_content("day-page-save-updated");
        session.apply_editor_content(&updated);
        session.save_content(&updated, now).expect("save note");

        let stored = crate::notes::get_note(note.id)
            .expect("get note")
            .expect("stored note");
        assert_eq!(stored.content, updated);
        assert_eq!(
            stored.title,
            session.viewing_note_title().unwrap_or_default()
        );
        assert!(
            fs::read_to_string(note_path)
                .expect("read canonical note")
                .contains(&updated),
            "canonical note file should contain updated body"
        );
        assert!(!session.is_dirty());
    }

    #[test]
    fn binding_second_regular_note_preserves_original_return_day_anchor() {
        let _guard = notes_test_guard();
        crate::notes::init_notes_db().expect("init notes db");
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind day");
        let original_day_path = session.path().expect("day path").clone();
        let original_day_date = session.bound_date().expect("day date");

        let note_a = saved_note(&unique_note_content("anchor-note-a"));
        let note_b = saved_note(&unique_note_content("anchor-note-b"));
        let note_a_path = crate::notes::note_file_path(note_a.id)
            .expect("note a path result")
            .expect("note a path");
        let note_b_path = crate::notes::note_file_path(note_b.id)
            .expect("note b path result")
            .expect("note b path");

        session
            .bind_note_content(
                note_a.id.as_str().to_string(),
                note_a.title.clone(),
                note_a.content.clone(),
                Some(note_a_path),
                now,
            )
            .expect("bind note a");
        let edited_a = unique_note_content("anchor-note-a-edited");
        session.apply_editor_content(&edited_a);
        session
            .bind_note_content(
                note_b.id.as_str().to_string(),
                note_b.title.clone(),
                note_b.content.clone(),
                Some(note_b_path),
                now,
            )
            .expect("switch note a to note b");

        let stored_a = crate::notes::get_note(note_a.id)
            .expect("get note a")
            .expect("stored note a");
        assert_eq!(stored_a.content, edited_a);

        session.return_to_day(now).expect("return to day");
        assert_eq!(session.path(), Some(&original_day_path));
        assert_eq!(session.bound_date(), Some(original_day_date));
        assert!(!session.is_dirty());
    }

    #[test]
    fn failed_regular_note_save_preserves_dirty_state() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind day");
        session
            .bind_note_content(
                "not-a-note-id".to_string(),
                "Missing Note".to_string(),
                "stale body".to_string(),
                None,
                now,
            )
            .expect("bind synthetic note");

        session.apply_editor_content("dirty missing note");
        let error = session
            .save_content("dirty missing note", now)
            .expect_err("missing note save should fail");

        assert!(
            error.to_string().contains("parsing day page note id"),
            "{error:#}"
        );
        assert!(session.is_dirty());
    }

    #[test]
    fn external_append_refreshes_when_clean() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind");

        session
            .simulate_external_append_for_test("- [ ] buy milk")
            .expect("append");

        let refreshed = session
            .maybe_refresh_from_disk()
            .expect("refresh")
            .expect("should refresh");
        assert!(refreshed.contains("buy milk"));
    }

    /// Chaos-09 lock: deleting/renaming the bound day file while the buffer is
    /// clean must NOT leave the session silently tracking a missing path — the
    /// editor buffer is the only copy, so the poll re-dirties the session and
    /// the next save recreates the file. Red before the fix: the poll errored
    /// silently forever and a later re-bind clobbered the buffer with a fresh
    /// empty day file (observed via notes-editor-hostile-chaos-probe.ts).
    #[test]
    fn vanished_bound_file_while_clean_re_dirties_for_resurrection() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-07-18T10:00:00Z");
        session.bind_today(now).expect("bind");
        session.apply_editor_content("precious buffer line");
        session.save(now).expect("save");
        assert!(!session.is_dirty());

        let path = session.path().expect("bound path").clone();
        fs::remove_file(&path).expect("vanish the bound file");

        let refreshed = session
            .maybe_refresh_from_disk()
            .expect("poll must not error on a vanished file");
        assert!(refreshed.is_none());
        assert!(
            session.is_dirty(),
            "vanished bound file must re-dirty the clean buffer"
        );

        session.save(now).expect("resave recreates the file");
        let disk = fs::read_to_string(&path).expect("recreated file");
        assert!(disk.contains("precious buffer line"));
        assert!(!session.is_dirty());
    }

    #[test]
    fn external_append_targets_bound_day_not_wall_clock_today() {
        let (_dir, mut session) = test_session();
        let bound_day = utc("2026-06-11T09:42:00Z");
        let later_day = utc("2026-06-12T09:42:00Z");
        session.bind_today(bound_day).expect("bind");
        let bound_path = session.path().expect("bound path").clone();
        let later_path = session.substrate().paths().day_page(later_day.date_naive());

        session
            .append_external_line_to_bound_file("09:43 Agent Chat\n\nkeep this")
            .expect("append to bound day");

        assert!(fs::read_to_string(&bound_path)
            .expect("read bound day")
            .contains("keep this"));
        assert!(
            !later_path.exists(),
            "external append must not choose a new path from wall-clock today"
        );
    }

    #[test]
    fn external_append_skipped_when_dirty_until_rebind() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind");
        session.apply_editor_content("typing...");

        session
            .simulate_external_append_for_test("- [ ] buy milk")
            .expect("append");
        assert!(session
            .maybe_refresh_from_disk()
            .expect("refresh")
            .is_none());

        let rebound = session.bind_today(now).expect("rebind");
        assert!(rebound.contains("buy milk"));
        assert!(!rebound.contains("typing..."));
    }

    /// View-entry contract: bind creates today's file, typed content persists, rollover binds anew.
    #[test]
    fn entering_day_page_view_creates_binds_and_rollovers() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");

        let content = session.bind_today(now).expect("bind creates today");
        assert_eq!(content, "");
        assert!(session.path().expect("path").exists());

        session.apply_editor_content("typed on today's page");
        session
            .save_content("typed on today's page", now)
            .expect("save");
        let disk = fs::read_to_string(session.path().expect("path")).expect("read");
        assert_eq!(disk, "typed on today's page");

        let next_day = utc("2026-06-12T00:05:00Z");
        let rebound = session.bind_today(next_day).expect("rollover bind");
        assert_eq!(rebound, "");
        assert_eq!(session.bound_date(), Some(next_day.date_naive()));
        assert_ne!(
            session.path().expect("path"),
            &session.substrate().paths().day_page(now.date_naive())
        );
    }

    /// Data-loss regression: an external append that lands between bind and the
    /// debounced autosave must survive the save. The saved file must contain
    /// BOTH the editor buffer and the appended line.
    #[test]
    fn save_merges_external_append_landing_before_autosave() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind");
        let path = session.path().expect("path").clone();

        // User types (editor is now dirty).
        session.apply_editor_content("morning thought");
        // A background writer (e.g. ;todo) appends before the autosave fires.
        io::atomic_append_line(&path, "09:45 - [ ] buy milk").expect("external append");

        session
            .save_content("morning thought", now)
            .expect("save merges");

        let disk = fs::read_to_string(&path).expect("read day file");
        assert!(
            disk.contains("morning thought"),
            "editor content kept: {disk:?}"
        );
        assert!(disk.contains("buy milk"), "external append kept: {disk:?}");
        assert_eq!(
            session.last_save_merged(),
            Some(disk.as_str()),
            "merged save exposes the written content for adoption"
        );
    }

    /// A clean save (no external writer touched the file since bind) writes the
    /// editor buffer verbatim and reports no merge.
    #[test]
    fn save_writes_exact_editor_content_when_no_external_change() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind");
        let path = session.path().expect("path").clone();

        session.apply_editor_content("just my words");
        session.save_content("just my words", now).expect("save");

        let disk = fs::read_to_string(&path).expect("read day file");
        assert_eq!(disk, "just my words");
        assert_eq!(
            session.last_save_merged(),
            None,
            "clean save is not a merge"
        );
    }

    /// When the disk diverged in a non-append way (an external edit rewrote
    /// earlier content), neither version is destroyed: the editor buffer wins
    /// the bound file and the on-disk version is copied to the brain trash.
    #[test]
    fn save_preserves_disk_on_non_append_divergence() {
        let (_dir, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind creates");
        let path = session.path().expect("path").clone();

        // Establish a non-empty baseline on disk.
        session.apply_editor_content("line one\nline two");
        session
            .save_content("line one\nline two", now)
            .expect("seed save");

        // User edits; meanwhile an external writer REWRITES the file wholesale.
        session.apply_editor_content("line one edited\nline two");
        io::atomic_write(&path, "completely different disk content").expect("external rewrite");

        session
            .save_content("line one edited\nline two", now)
            .expect("save resolves conflict");

        let disk = fs::read_to_string(&path).expect("read day file");
        assert_eq!(
            disk, "line one edited\nline two",
            "editor buffer wins bound path"
        );

        let trash_dir = session.substrate().paths().trash_dir();
        let conflict = fs::read_dir(&trash_dir)
            .expect("trash dir")
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.file_name().to_string_lossy().contains(".conflict-"))
            .expect("a conflict copy should exist in trash");
        let conflict_body = fs::read_to_string(conflict.path()).expect("read conflict copy");
        assert_eq!(
            conflict_body, "completely different disk content",
            "conflict copy preserves the diverged disk version"
        );
        assert_eq!(
            session.last_save_merged(),
            Some("line one edited\nline two"),
            "conflict save exposes the written editor content for adoption"
        );
    }

    #[test]
    fn day_page_private_conflicts_preserve_every_repeated_external_version() {
        let (_fixture, mut session) = test_session();
        let now = utc("2026-06-11T09:42:00Z");
        session.bind_today(now).expect("bind owned day");
        let path = session.path().expect("day path").clone();
        session.apply_editor_content("initial private day");
        session
            .save_content("initial private day", now)
            .expect("seed nonempty private baseline");

        for index in 0..3 {
            let external = format!("external private rewrite {index}");
            let editor = format!("editor private version {index}");
            io::atomic_write(&path, &external).expect("simulate private external edit");
            session.apply_editor_content(&editor);
            session
                .save_content(&editor, now)
                .expect("preserve private conflicting version");
            assert_eq!(io::read_private_document(&path).unwrap(), editor);
        }

        let mut conflicts = fs::read_dir(session.substrate().paths().trash_dir())
            .expect("private trash")
            .map(|entry| io::read_private_document(&entry.expect("owned entry").path()).unwrap())
            .collect::<Vec<_>>();
        conflicts.sort();
        assert_eq!(
            conflicts,
            vec![
                "external private rewrite 0".to_string(),
                "external private rewrite 1".to_string(),
                "external private rewrite 2".to_string(),
            ]
        );
    }
}

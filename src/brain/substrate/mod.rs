//! Brain filesystem substrate — canonical markdown under `~/.scriptkit/brain/`.
//!
//! This module owns every path under `brain/{days,fragments,notes,trash}`.
//! Callers must not construct those locations directly.

mod day;
mod fragment;
mod frontmatter;
pub(crate) mod io;
mod paths;
mod slug;
mod trash;
mod words;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;

pub use day::DayEntry;
pub use fragment::FragmentReference;
pub use frontmatter::BrainFrontmatter;
pub use paths::BrainPaths;
pub use slug::dedupe_slug_in_dir;
pub use trash::{restore_file, trash_file};

/// Which brain subdirectory a slug allocation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainSlugDir {
    Notes,
    Fragments,
}

/// Entry point for brain filesystem operations.
#[derive(Debug, Clone)]
pub struct BrainSubstrate {
    paths: BrainPaths,
    tz: Tz,
}

impl BrainSubstrate {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            paths: BrainPaths::new(base),
            tz: chrono_tz::UTC,
        }
    }

    pub fn with_timezone(base: impl AsRef<Path>, tz: Tz) -> Self {
        Self {
            paths: BrainPaths::new(base),
            tz,
        }
    }

    pub fn default_kit() -> Self {
        Self::with_timezone(BrainPaths::default_kit().base(), default_brain_timezone())
    }

    pub fn paths(&self) -> &BrainPaths {
        &self.paths
    }

    pub fn timezone(&self) -> Tz {
        self.tz
    }

    /// Append a timestamped entry to today's day page. Append-only — earlier
    /// content is never rewritten.
    pub fn append_to_day(&self, now: DateTime<Utc>, entry: DayEntry) -> Result<()> {
        let (date, timestamp) = day::local_day_and_time(now, self.tz);
        let path = self.paths.day_page(date);
        let line = entry.format_line(&timestamp);
        io::atomic_append_line(&path, &line)?;
        // Layering exception: the substrate is otherwise a pure file layer, but
        // every append_to_day caller (clipboard sediment, `;todo`, agent day
        // traces, dictation) is a capture that must be recallable at once.
        // Indexing here — once — instead of at each call site keeps the diff
        // small and derives the doc from the exact file just written, so it can
        // never drift from the periodic sync. `index_capture_after_write` is a
        // cfg(test) no-op so file-layer tests stay pure.
        crate::brain::indexer::index_capture_after_write(&path);
        Ok(())
    }

    /// Write a long capture as a fragment when it exceeds
    /// [`FRAGMENT_WORD_THRESHOLD`] words. Returns excerpt + relative link for a
    /// [`DayEntry::FragmentRef`] line.
    pub fn write_fragment(
        &self,
        now: DateTime<Utc>,
        source_label: &str,
        source_uri: &str,
        content: &str,
    ) -> Result<Option<FragmentReference>> {
        self.write_fragment_with_why(now, source_label, source_uri, content, None)
    }

    /// Write a long capture as a fragment, optionally attaching a post-copy why.
    pub fn write_fragment_with_why(
        &self,
        now: DateTime<Utc>,
        source_label: &str,
        source_uri: &str,
        content: &str,
        why: Option<&str>,
    ) -> Result<Option<FragmentReference>> {
        let reference = fragment::write_fragment_with_why(
            &self.paths,
            now,
            self.tz,
            source_label,
            source_uri,
            content,
            why,
        )?;
        // Same layering exception as append_to_day: index the fragment file the
        // writer just produced so long captures are recallable immediately. The
        // reference's relative_link is `../fragments/<id>.md`; resolve it back to
        // the absolute path under this substrate.
        if let Some(reference) = &reference {
            if let Some(filename) = reference.relative_link.rsplit('/').next() {
                let fragment_path = self.paths.fragments_dir().join(filename);
                crate::brain::indexer::index_capture_after_write(&fragment_path);
            }
        }
        Ok(reference)
    }

    /// Remove sediment lines written for a clipboard entry (T12 reject undo).
    pub fn undo_clipboard_sediment_lines(
        &self,
        now: DateTime<Utc>,
        entry_id: &str,
        text: &str,
        kept_url_day: Option<&str>,
        brain_kept: bool,
    ) -> Result<()> {
        day::undo_clipboard_sediment_lines(
            &self.paths,
            self.tz,
            now,
            entry_id,
            text,
            kept_url_day,
            brain_kept,
        )
    }

    /// Serialize a brain document with frontmatter and write it atomically.
    pub fn write_document(
        &self,
        path: &Path,
        frontmatter: &BrainFrontmatter,
        body: &str,
    ) -> Result<()> {
        if !self.paths.contains(path) {
            bail!("refusing to write outside brain tree: {}", path.display());
        }
        let document = frontmatter.render(body);
        io::atomic_write(path, &document)
    }

    /// Parse a brain markdown document into frontmatter and body.
    pub fn parse_document(&self, content: &str) -> Result<(BrainFrontmatter, String)> {
        BrainFrontmatter::parse(content)
    }

    /// Allocate a unique lowercase hyphenated slug in the given directory.
    pub fn allocate_slug(&self, base: &str, dir: BrainSlugDir) -> String {
        let parent = match dir {
            BrainSlugDir::Notes => self.paths.notes_dir(),
            BrainSlugDir::Fragments => self.paths.fragments_dir(),
        };
        dedupe_slug_in_dir(&parent, base)
    }

    /// Move a brain file into `brain/trash/`.
    pub fn trash(&self, path: &Path) -> Result<PathBuf> {
        trash_file(&self.paths, path)
    }

    /// Restore a trashed file to its original location.
    pub fn restore(&self, trashed: &Path, destination: &Path) -> Result<()> {
        restore_file(&self.paths, trashed, destination)
    }
}

fn default_brain_timezone() -> Tz {
    if let Some(tz) = parse_timezone_env("SCRIPT_KIT_BRAIN_TZ") {
        return tz;
    }
    if let Some(tz) = parse_timezone_env("TZ") {
        return tz;
    }
    if let Some(tz) = detect_local_timezone() {
        return tz;
    }

    tracing::debug!(target: "script_kit::brain", "local brain timezone detection failed; falling back to UTC");
    chrono_tz::UTC
}

fn parse_timezone_env(key: &str) -> Option<Tz> {
    let value = std::env::var(key).ok()?;
    parse_timezone_name(&value)
}

fn parse_timezone_name(name: &str) -> Option<Tz> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let trimmed = trimmed.strip_prefix(':').unwrap_or(trimmed);
    trimmed.parse::<Tz>().ok()
}

fn detect_local_timezone() -> Option<Tz> {
    detect_local_timezone_from_etc_localtime().or_else(detect_local_timezone_from_systemsetup)
}

#[cfg(unix)]
fn detect_local_timezone_from_etc_localtime() -> Option<Tz> {
    let link = std::fs::read_link("/etc/localtime").ok()?;
    let link = link.to_string_lossy();
    for marker in ["zoneinfo/", "zoneinfo.default/"] {
        if let Some((_, tail)) = link.rsplit_once(marker) {
            let tail = tail
                .strip_prefix("posix/")
                .or_else(|| tail.strip_prefix("right/"))
                .unwrap_or(tail);
            if let Some(tz) = parse_timezone_name(tail) {
                return Some(tz);
            }
        }
    }
    None
}

#[cfg(not(unix))]
fn detect_local_timezone_from_etc_localtime() -> Option<Tz> {
    None
}

#[cfg(target_os = "macos")]
fn detect_local_timezone_from_systemsetup() -> Option<Tz> {
    let output = std::process::Command::new("/usr/sbin/systemsetup")
        .arg("-gettimezone")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout
        .lines()
        .find_map(|line| line.split_once(':').map(|(_, value)| value.trim()))?;
    parse_timezone_name(name)
}

#[cfg(not(target_os = "macos"))]
fn detect_local_timezone_from_systemsetup() -> Option<Tz> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;
    use std::fs;
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

    use crate::notes::NoteId;

    fn test_substrate() -> (tempfile::TempDir, BrainSubstrate) {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate = BrainSubstrate::with_timezone(dir.path().join("brain"), chrono_tz::UTC);
        (dir, substrate)
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 11, 9, 42, 0).unwrap()
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn default_kit_uses_script_kit_brain_tz_override_for_local_day() {
        let _guard = env_lock().lock().expect("env lock");
        let previous_brain_tz = std::env::var("SCRIPT_KIT_BRAIN_TZ").ok();
        let previous_tz = std::env::var("TZ").ok();
        unsafe {
            std::env::set_var("SCRIPT_KIT_BRAIN_TZ", "America/Denver");
            std::env::remove_var("TZ");
        }

        let substrate = BrainSubstrate::default_kit();

        match previous_brain_tz {
            Some(value) => unsafe { std::env::set_var("SCRIPT_KIT_BRAIN_TZ", value) },
            None => unsafe { std::env::remove_var("SCRIPT_KIT_BRAIN_TZ") },
        }
        match previous_tz {
            Some(value) => unsafe { std::env::set_var("TZ", value) },
            None => unsafe { std::env::remove_var("TZ") },
        }

        assert_eq!(substrate.timezone(), chrono_tz::America::Denver);
    }

    #[test]
    fn parse_tz_env_accepts_colon_prefixed_local_day_names() {
        assert_eq!(
            parse_timezone_name(":Pacific/Honolulu"),
            Some(chrono_tz::Pacific::Honolulu)
        );
        assert_eq!(parse_timezone_name("Not/AZone"), None);
    }

    #[test]
    fn append_to_day_preserves_order_and_timestamps() {
        let (_dir, substrate) = test_substrate();
        let base = fixed_now();

        substrate
            .append_to_day(
                base,
                DayEntry::Capture {
                    text: "first capture".to_string(),
                },
            )
            .expect("first append");
        substrate
            .append_to_day(
                base + chrono::Duration::minutes(3),
                DayEntry::Task {
                    body: "buy milk".to_string(),
                    tags: vec!["errand".to_string()],
                    due: Some("2026-06-12".to_string()),
                },
            )
            .expect("second append");
        substrate
            .append_to_day(
                base + chrono::Duration::minutes(8),
                DayEntry::ClipboardRef {
                    entry_id: "clip-1".to_string(),
                },
            )
            .expect("third append");

        let path = substrate.paths().day_page(base.date_naive());
        let contents = fs::read_to_string(path).expect("read day page");
        let lines: Vec<&str> = contents.lines().collect();

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "09:42 first capture");
        assert_eq!(lines[1], "09:45 - [ ] buy milk #errand due:2026-06-12");
        assert_eq!(
            lines[2],
            "09:50 [Clipboard entry](kit://clipboard-history?id=clip-1)"
        );
    }

    #[test]
    fn append_to_day_uses_configured_local_day_at_utc_boundary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate =
            BrainSubstrate::with_timezone(dir.path().join("brain"), chrono_tz::America::Denver);
        let utc_boundary = Utc.with_ymd_and_hms(2026, 6, 14, 5, 30, 0).unwrap();

        substrate
            .append_to_day(
                utc_boundary,
                DayEntry::Capture {
                    text: "local day boundary".to_string(),
                },
            )
            .expect("append local day");

        let local_path = substrate
            .paths()
            .day_page(chrono::NaiveDate::from_ymd_opt(2026, 6, 13).unwrap());
        let utc_path = substrate
            .paths()
            .day_page(chrono::NaiveDate::from_ymd_opt(2026, 6, 14).unwrap());
        let contents = fs::read_to_string(local_path).expect("read local day");
        assert!(contents.contains("23:30 local day boundary"));
        assert!(!utc_path.exists(), "UTC day file must not be created");
    }

    #[test]
    fn append_to_day_is_append_only() {
        let (_dir, substrate) = test_substrate();
        let now = fixed_now();

        substrate
            .append_to_day(
                now,
                DayEntry::Capture {
                    text: "original".to_string(),
                },
            )
            .expect("first append");
        substrate
            .append_to_day(
                now + chrono::Duration::minutes(1),
                DayEntry::Capture {
                    text: "second".to_string(),
                },
            )
            .expect("second append");

        let path = substrate.paths().day_page(now.date_naive());
        let contents = fs::read_to_string(path).expect("read day page");
        assert!(contents.contains("09:42 original"));
        assert!(contents.starts_with("09:42 original"));
        assert!(contents.contains("09:43 second"));
    }

    #[test]
    fn fragment_writer_respects_threshold_and_excerpt() {
        let (_dir, substrate) = test_substrate();
        let now = fixed_now();
        let short = "short capture".to_string();
        let long = (0..250)
            .map(|index| format!("word{index}"))
            .collect::<Vec<_>>()
            .join(" ");

        let none = substrate
            .write_fragment(now, "clipboard", "scriptkit://clipboard/entry-1", &short)
            .expect("short fragment write");
        assert!(none.is_none());

        let reference = substrate
            .write_fragment(now, "Slack Paste", "scriptkit://clipboard/entry-2", &long)
            .expect("long fragment write")
            .expect("fragment reference");

        assert_eq!(
            reference.relative_link,
            "../fragments/2026-06-11-0942-slack-paste.md"
        );
        assert!(reference.excerpt.ends_with("..."));
        assert_eq!(reference.excerpt.split_whitespace().count(), 40);

        let fragment_path = substrate
            .paths()
            .fragment_file("2026-06-11-0942-slack-paste");
        let fragment = fs::read_to_string(fragment_path).expect("read fragment");
        assert!(fragment.contains("source: scriptkit://clipboard/entry-2"));
        assert!(fragment.contains("word249"));
    }

    #[test]
    fn fragment_writer_uses_configured_local_day_and_time_in_filename() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate =
            BrainSubstrate::with_timezone(dir.path().join("brain"), chrono_tz::America::Denver);
        let utc_boundary = Utc.with_ymd_and_hms(2026, 6, 14, 5, 30, 0).unwrap();
        let long = (0..250)
            .map(|index| format!("word{index}"))
            .collect::<Vec<_>>()
            .join(" ");

        let reference = substrate
            .write_fragment(
                utc_boundary,
                "Clipboard",
                "scriptkit://clipboard/local-day",
                &long,
            )
            .expect("fragment write")
            .expect("fragment reference");

        assert_eq!(
            reference.relative_link,
            "../fragments/2026-06-13-2330-clipboard.md"
        );
        assert!(substrate
            .paths()
            .fragment_file("2026-06-13-2330-clipboard")
            .exists());
    }

    #[test]
    fn slug_dedup_adds_numeric_suffixes() {
        let (_dir, substrate) = test_substrate();
        let notes_dir = substrate.paths().notes_dir();
        fs::create_dir_all(&notes_dir).expect("notes dir");
        fs::write(notes_dir.join("my-note.md"), "existing").expect("seed note");

        assert_eq!(
            substrate.allocate_slug("My Note", BrainSlugDir::Notes),
            "my-note-2"
        );
        fs::write(notes_dir.join("my-note-2.md"), "existing").expect("seed note 2");
        assert_eq!(
            substrate.allocate_slug("My Note", BrainSlugDir::Notes),
            "my-note-3"
        );
    }

    #[test]
    fn frontmatter_round_trip_preserves_fields() {
        let (_dir, substrate) = test_substrate();
        let id = NoteId::parse("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let created = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let updated = Utc.with_ymd_and_hms(2026, 6, 7, 8, 9, 10).unwrap();
        let frontmatter = BrainFrontmatter {
            id,
            created,
            updated,
            tags: vec!["rust".to_string(), "notes/metadata".to_string()],
            aliases: vec!["Plan".to_string()],
            pinned: true,
            source: Some("scriptkit://agent-chat/thread-123".to_string()),
            why: None,
        };

        let rendered = frontmatter.render("# Title\n\nBody text");
        let (parsed, body) = substrate.parse_document(&rendered).expect("parse");

        assert_eq!(parsed, frontmatter);
        assert_eq!(body, "# Title\n\nBody text");
    }

    #[test]
    fn atomic_write_leaves_no_partial_files() {
        let (_dir, substrate) = test_substrate();
        let path = substrate.paths().notes_dir().join("atomic-note.md");
        let frontmatter = BrainFrontmatter::new(NoteId::new(), fixed_now(), fixed_now());

        substrate
            .write_document(&path, &frontmatter, "complete body")
            .expect("atomic write");

        let parent = path.parent().expect("parent");
        let leftovers: Vec<_> = fs::read_dir(parent)
            .expect("read dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".brain-write.")
            })
            .collect();
        assert!(leftovers.is_empty());

        let contents = fs::read_to_string(path).expect("read note");
        assert!(contents.contains("complete body"));
        assert!(contents.starts_with("---\n"));
    }

    #[test]
    fn trash_and_restore_round_trip() {
        let (_dir, substrate) = test_substrate();
        let note_path = substrate.paths().note_file("restore-me");
        let frontmatter = BrainFrontmatter::new(NoteId::new(), fixed_now(), fixed_now());
        substrate
            .write_document(&note_path, &frontmatter, "keep me")
            .expect("write note");

        let trashed = substrate.trash(&note_path).expect("trash");
        assert!(!note_path.exists());
        assert!(trashed.starts_with(substrate.paths().trash_dir()));

        substrate.restore(&trashed, &note_path).expect("restore");
        assert!(note_path.exists());
        assert!(!trashed.exists());
        let contents = fs::read_to_string(note_path).expect("read restored");
        assert!(contents.contains("keep me"));
    }

    #[test]
    fn trash_collision_adds_timestamp_suffix() {
        let (_dir, substrate) = test_substrate();
        let trash_dir = substrate.paths().trash_dir();
        fs::create_dir_all(&trash_dir).expect("trash dir");
        fs::write(trash_dir.join("collision.md"), "existing").expect("seed trash");

        let note_path = substrate.paths().note_file("collision");
        substrate
            .write_document(
                &note_path,
                &BrainFrontmatter::new(NoteId::new(), fixed_now(), fixed_now()),
                "body",
            )
            .expect("write note");

        let trashed = substrate.trash(&note_path).expect("trash");
        let name = trashed.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("collision-"));
        assert!(name.ends_with(".md"));
    }

    #[test]
    fn trace_and_fragment_ref_lines_render_expected_markdown() {
        let (_dir, substrate) = test_substrate();
        let now = fixed_now();

        substrate
            .append_to_day(
                now,
                DayEntry::FragmentRef(FragmentReference {
                    excerpt: "First words of the pasted article without cutting mid-word..."
                        .to_string(),
                    relative_link: "../fragments/2026-06-11-0942-clipboard.md".to_string(),
                }),
            )
            .expect("fragment ref append");
        substrate
            .append_to_day(
                now + chrono::Duration::minutes(1),
                DayEntry::Trace {
                    summary: "Agent Chat: flaky clock test".to_string(),
                    provenance_link: "scriptkit://agent-chat/thread-9".to_string(),
                },
            )
            .expect("trace append");

        let contents =
            fs::read_to_string(substrate.paths().day_page(now.date_naive())).expect("read day");
        assert!(contents.contains("09:42 Fragment\n"));
        assert!(
            contents.contains("> First words of the pasted article without cutting mid-word...\n")
        );
        assert!(contents.contains("[Open fragment](../fragments/2026-06-11-0942-clipboard.md)"));
        assert!(contents
            .contains("09:43 [Agent Chat: flaky clock test](scriptkit://agent-chat/thread-9)"));
    }

    #[test]
    fn fragment_reference_card_undo_removes_day_card_and_trashes_fragment() {
        let (_dir, substrate) = test_substrate();
        let now = fixed_now();
        let source_uri = "scriptkit://clipboard/entry-card";
        let long = (0..250)
            .map(|index| format!("word{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let reference = substrate
            .write_fragment(now, "clipboard", source_uri, &long)
            .expect("write fragment")
            .expect("fragment reference");
        let fragment_id = reference
            .relative_link
            .trim_start_matches("../fragments/")
            .trim_end_matches(".md")
            .to_string();
        let fragment_path = substrate.paths().fragment_file(&fragment_id);

        substrate
            .append_to_day(now, DayEntry::FragmentRef(reference))
            .expect("append card");
        let before =
            fs::read_to_string(substrate.paths().day_page(now.date_naive())).expect("read day");
        assert!(before.contains("09:42 Fragment\n"));
        assert!(before.contains("[Open fragment](../fragments/2026-06-11-0942-clipboard.md)"));
        assert!(fragment_path.exists());

        substrate
            .undo_clipboard_sediment_lines(now, "entry-card", &long, None, true)
            .expect("undo sediment");

        let after =
            fs::read_to_string(substrate.paths().day_page(now.date_naive())).expect("read day");
        assert!(!after.contains("09:42 Fragment"));
        assert!(!after.contains("Open fragment"));
        assert!(!fragment_path.exists(), "active fragment should be trashed");
        assert!(
            fs::read_dir(substrate.paths().trash_dir())
                .expect("trash dir")
                .filter_map(|entry| entry.ok())
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .contains("2026-06-11-0942-clipboard")),
            "fragment file should move into brain trash"
        );
    }

    #[test]
    fn excerpt_words_never_cuts_mid_word() {
        use crate::brain::substrate::words::excerpt_words;
        let text = "one two three four five";
        assert_eq!(excerpt_words(text, 3), "one two three...");
        assert_eq!(excerpt_words("short", 10), "short");
    }

    #[test]
    fn concurrent_appends_do_not_leave_temp_files() {
        let (_dir, substrate) = test_substrate();
        let now = fixed_now();
        let path = substrate.paths().day_page(now.date_naive());

        for index in 0..5 {
            substrate
                .append_to_day(
                    now + chrono::Duration::seconds(index),
                    DayEntry::Capture {
                        text: format!("capture {index}"),
                    },
                )
                .expect("append");
            thread::sleep(Duration::from_millis(5));
        }

        let contents = fs::read_to_string(&path).expect("read day page");
        for index in 0..5 {
            assert!(contents.contains(&format!("capture {index}")));
        }
    }
}

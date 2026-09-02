//! Captures inverse-browser artifact reader.
//!
//! Oracle iter 007 commit 7: supply a tolerant reader over the local-first
//! JSONL files capture handlers append to under `$SK_PATH/menu-syntax/`, plus
//! the payload tempfile directory. The reader is pure — it does not list or
//! delete files outside the expected set, does not touch GPUI, and skips
//! dirty JSONL lines with a capped warning list so a single bad row never
//! crashes the Captures built-in view.
//!
//! File layout expected under `$SK_PATH/menu-syntax/`:
//!
//! - `events.jsonl` — `+cal` captures.
//! - `notes.jsonl` — `+note` captures (shipped example uses per-day markdown
//!   but scaffolded handlers default to JSONL, so both are valid — the
//!   reader only enumerates JSONL for now).
//! - `drafts.jsonl` — `+social` draft append log.
//! - `bookmarks.jsonl` — `+link` captures.
//!
//! Unchecked `;todo` tasks are read from `$SK_PATH/brain/days/*.md` (most
//! recent 30 day pages).
//! - `payloads/capture_v1-*.json` — per-execution payload tempfiles (written
//!   by `menu_syntax::execute::write_payload_tempfile`).

use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::scripts::root_search_contract::{
    RootOwnedProviderRefresh, RootOwnedProviderRefreshLifecycle,
};

/// How many recent day pages to scan for unchecked todo task lines (v1).
pub const RECENT_DAY_PAGE_SCAN_LIMIT: usize = 30;

/// Maximum warning messages retained per `ReadArtifactReport`. Oracle iter 004
/// explicit rule: the reader must tolerate dirty JSONL, surface warning
/// counts, and never crash the builtin. A 10-warning cap is enough to
/// summarize the problem without flooding the UI with spam for files that
/// are entirely garbage.
pub const MAX_WARNINGS: usize = 10;

/// Maximum characters retained in the per-row snippet preview. The Captures
/// inverse browser renders one row per entry; the snippet is what the user
/// sees before they open the full artifact. Long bodies get truncated with
/// a trailing `…` so row height stays bounded.
pub const MAX_SNIPPET_CHARS: usize = 200;

/// Discriminator for capture artifacts. Matches the on-disk filenames used
/// by shipped examples and by the scaffold emitted from
/// [`super::templates::render_capture_handler_template`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureArtifactKind {
    Todo,
    CalendarEvent,
    Note,
    SocialDraft,
    Bookmark,
    /// A per-execution payload tempfile (`capture_v1-*.json`). These are not
    /// captured artifacts in the user sense — they are the raw payloads the
    /// launcher wrote for the handler to consume. Retention (commit 8) runs
    /// against this kind specifically.
    Payload,
}

impl CaptureArtifactKind {
    /// All kinds that participate in the Captures inverse browser's aggregate
    /// view. Order matches the UI Oracle sketched in iter 004 (todos first
    /// because they're the most common capture, bookmarks last because
    /// clicking them usually hands off to a browser).
    pub const BROWSER_ORDER: &'static [Self] = &[
        Self::Todo,
        Self::CalendarEvent,
        Self::Note,
        Self::SocialDraft,
        Self::Bookmark,
    ];

    pub fn filename(self) -> &'static str {
        match self {
            Self::Todo => "day-pages",
            Self::CalendarEvent => "events.jsonl",
            Self::Note => "notes.jsonl",
            Self::SocialDraft => "drafts.jsonl",
            Self::Bookmark => "bookmarks.jsonl",
            Self::Payload => "payloads",
        }
    }
}

/// One parsed artifact row. `raw_line` is retained so the UI (or the tests)
/// can present the unparsed source when needed; `snippet` is a pre-truncated
/// preview suitable for a single-line list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureArtifact {
    pub kind: CaptureArtifactKind,
    pub path: PathBuf,
    /// 1-based line number within the source file. `None` for payload
    /// tempfiles — they are individual files, not lines in a shared log.
    pub line_number: Option<usize>,
    /// Best-effort creation timestamp extracted from the JSON (e.g. the
    /// `createdAt` field shipped examples write). `None` when absent or
    /// unparseable — the UI falls back to the file's mtime if it needs one.
    pub created_at: Option<String>,
    /// Truncated single-line preview. Never longer than [`MAX_SNIPPET_CHARS`].
    pub snippet: String,
    /// Original source line or filename. Bounded to the raw bytes that made
    /// it into the file — the reader never rewrites this.
    pub raw_line: String,
}

/// Aggregate result of one artifact read. The reader is infallible by design;
/// every failure surfaces as a warning string plus a bumped `skipped` count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadArtifactReport {
    pub entries: Vec<CaptureArtifact>,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootTodoSectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
}

impl Default for RootTodoSectionOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 10,
            min_query_chars: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootTodoSearchHit {
    pub stable_key: String,
    pub title: String,
    pub body: String,
    pub subtitle: String,
    pub tags: Vec<String>,
    pub priority: Option<u8>,
    pub due: Option<String>,
    pub created_at: Option<String>,
    pub path: PathBuf,
    pub line_number: Option<usize>,
    pub raw_line: String,
}

impl ReadArtifactReport {
    /// Merge another report into this one. Entries are appended in order,
    /// skipped counts sum, and warnings are capped at [`MAX_WARNINGS`] so a
    /// single pathological file can't blow up the combined report.
    pub fn merge(&mut self, mut other: ReadArtifactReport) {
        self.entries.append(&mut other.entries);
        self.skipped = self.skipped.saturating_add(other.skipped);
        for warning in other.warnings {
            if self.warnings.len() >= MAX_WARNINGS {
                break;
            }
            self.warnings.push(warning);
        }
    }
}

/// Read a single JSONL artifact file. Missing files yield an empty report
/// (no error). Unreadable files yield one warning and `skipped += 1`. Every
/// non-empty line that fails to parse yields a warning (up to the cap) and
/// bumps `skipped`. Lines that parse but are not objects are still included
/// in `entries` with best-effort snippets.
pub fn read_jsonl_artifact(path: &Path, kind: CaptureArtifactKind) -> ReadArtifactReport {
    let mut report = ReadArtifactReport::default();

    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return report,
        Err(err) => {
            push_warning(
                &mut report,
                format!("could not open {}: {err}", path.display()),
            );
            report.skipped = report.skipped.saturating_add(1);
            return report;
        }
    };

    let reader = BufReader::new(file);
    for (idx, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(err) => {
                push_warning(
                    &mut report,
                    format!("{}:{} read error: {err}", path.display(), idx + 1),
                );
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed = match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(v) => v,
            Err(err) => {
                push_warning(
                    &mut report,
                    format!("{}:{} invalid JSON: {err}", path.display(), idx + 1),
                );
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
        };

        let created_at = extract_created_at(&parsed);
        let snippet = snippet_for_value(&parsed);
        report.entries.push(CaptureArtifact {
            kind,
            path: path.to_path_buf(),
            line_number: Some(idx + 1),
            created_at,
            snippet,
            raw_line: line,
        });
    }

    report
}

/// Read every `capture_v1-*.json` file under `payload_dir`. Missing directory
/// yields an empty report. Each file is read once; the entire JSON object is
/// the entry, so `line_number` is always `None`.
pub fn read_payload_dir(payload_dir: &Path) -> ReadArtifactReport {
    let mut report = ReadArtifactReport::default();

    let entries = match read_dir(payload_dir) {
        Ok(e) => e,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return report,
        Err(err) => {
            push_warning(
                &mut report,
                format!("could not read {}: {err}", payload_dir.display()),
            );
            return report;
        }
    };

    for dirent in entries.flatten() {
        let path = dirent.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("capture_v1-") || !name.ends_with(".json") {
            continue;
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) => {
                push_warning(
                    &mut report,
                    format!("could not read {}: {err}", path.display()),
                );
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
        };
        let parsed = match serde_json::from_str::<serde_json::Value>(raw.trim()) {
            Ok(v) => v,
            Err(err) => {
                push_warning(
                    &mut report,
                    format!("{} invalid JSON: {err}", path.display()),
                );
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
        };
        let created_at = extract_created_at(&parsed);
        let snippet = snippet_for_value(&parsed);
        report.entries.push(CaptureArtifact {
            kind: CaptureArtifactKind::Payload,
            path,
            line_number: None,
            created_at,
            snippet,
            raw_line: raw,
        });
    }

    report
}

/// Read every known artifact kind under `sk_path/menu-syntax/` into a single
/// aggregate report. Caller is expected to pass the resolved `SK_PATH` root
/// (defaulting to `~/.scriptkit`). Reports are merged in
/// [`CaptureArtifactKind::BROWSER_ORDER`] followed by payloads last.
pub fn read_all_artifacts(sk_path: &Path) -> ReadArtifactReport {
    let mut report = ReadArtifactReport::default();
    let base = sk_path.join("menu-syntax");

    for kind in CaptureArtifactKind::BROWSER_ORDER {
        let sub = if *kind == CaptureArtifactKind::Todo {
            read_day_page_task_artifacts(sk_path)
        } else {
            let filename = kind.filename();
            let path = base.join(filename);
            read_jsonl_artifact(&path, *kind)
        };
        report.merge(sub);
    }

    let payload_dir = base.join(CaptureArtifactKind::Payload.filename());
    let payload_sub = read_payload_dir(&payload_dir);
    report.merge(payload_sub);

    report
}

pub fn root_todo_query_is_eligible(query: &str, options: RootTodoSectionOptions) -> bool {
    options.enabled
        && crate::scripts::search::query_meets_min_query_chars(
            query.trim(),
            options.min_query_chars,
        )
        && !query.contains('\n')
}

pub fn search_root_todos_direct(
    query: &str,
    options: RootTodoSectionOptions,
) -> Vec<RootTodoSearchHit> {
    if !root_todo_query_is_eligible(query, options) {
        return Vec::new();
    }
    search_root_todos_in_sk_path(query, options, &default_sk_path())
}

pub fn search_root_todos_in_sk_path(
    query: &str,
    options: RootTodoSectionOptions,
    sk_path: &Path,
) -> Vec<RootTodoSearchHit> {
    if !options.enabled {
        return Vec::new();
    }
    let normalized_query = normalize_match_text(query);
    let mut hits = collect_day_page_todo_hits(sk_path, RECENT_DAY_PAGE_SCAN_LIMIT)
        .unwrap_or_else(|error| {
            tracing::warn!(
                error_bytes = error.to_string().len(),
                "root_todos_read_failed"
            );
            Vec::new()
        })
        .into_iter()
        .filter(|hit| todo_hit_matches(hit, &normalized_query))
        .take(options.max_results)
        .collect::<Vec<_>>();
    hits.shrink_to_fit();
    hits
}

/// Day-page todo snapshot for every root-launcher typing path. Reading up to
/// 30 markdown files never belongs in foreground grouping, even for an explicit
/// `todo:` filter. The launcher owns one generation-fenced background scan.
struct RootTodoSnapshotCache {
    snapshot: Option<RootTodoSnapshot>,
    refresh_needed: bool,
    refresh_lifecycle: RootOwnedProviderRefreshLifecycle,
}

struct RootTodoSnapshot {
    captured_at: std::time::Instant,
    hits: std::sync::Arc<Vec<RootTodoSearchHit>>,
}

pub(crate) struct RootTodoRefreshSnapshot {
    owner: RootOwnedProviderRefresh,
    hits: anyhow::Result<Vec<RootTodoSearchHit>>,
}

impl RootTodoRefreshSnapshot {
    pub(crate) fn read_outcome(&self) -> Result<usize, &anyhow::Error> {
        self.hits.as_ref().map(Vec::len)
    }
}

static ROOT_TODO_SNAPSHOT: std::sync::Mutex<RootTodoSnapshotCache> =
    std::sync::Mutex::new(RootTodoSnapshotCache {
        snapshot: None,
        refresh_needed: false,
        refresh_lifecycle: RootOwnedProviderRefreshLifecycle {
            next_generation: 0,
            in_flight: None,
        },
    });

const ROOT_TODO_SNAPSHOT_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// Nonblocking snapshot-only lookup: one mutex lock plus an in-memory filter.
/// No file I/O or detached worker ever starts from foreground grouping.
pub fn search_root_todos_cached(
    query: &str,
    options: RootTodoSectionOptions,
) -> Vec<RootTodoSearchHit> {
    if !root_todo_query_is_eligible(query, options) {
        return Vec::new();
    }
    let candidates = {
        let Ok(cache) = ROOT_TODO_SNAPSHOT.lock() else {
            return Vec::new();
        };
        match cache.snapshot.as_ref() {
            Some(snapshot) => snapshot.hits.clone(),
            None => return Vec::new(),
        }
    };
    let normalized_query = normalize_match_text(query);
    candidates
        .iter()
        .filter(|hit| todo_hit_matches(hit, &normalized_query))
        .take(options.max_results)
        .cloned()
        .collect()
}

fn root_todos_snapshot_cache_is_fresh(cache: &RootTodoSnapshotCache) -> bool {
    !cache.refresh_needed
        && cache.snapshot.as_ref().is_some_and(|snapshot| {
            crate::runtime_policy::root_search_now().saturating_duration_since(snapshot.captured_at)
                <= ROOT_TODO_SNAPSHOT_TTL
        })
}

pub(crate) fn root_todos_snapshot_is_fresh() -> bool {
    ROOT_TODO_SNAPSHOT
        .lock()
        .is_ok_and(|cache| root_todos_snapshot_cache_is_fresh(&cache))
}

fn fresh_root_todos_snapshot_status(cache: &RootTodoSnapshotCache) -> Option<(u64, usize)> {
    if cache.refresh_lifecycle.in_flight.is_some() || !root_todos_snapshot_cache_is_fresh(cache) {
        return None;
    }
    Some((
        cache.refresh_lifecycle.next_generation,
        cache.snapshot.as_ref()?.hits.len(),
    ))
}

/// Current snapshot state revision and row count, including accepted empty snapshots.
pub(crate) fn root_todos_fresh_cache_status() -> Option<(u64, usize)> {
    let cache = ROOT_TODO_SNAPSHOT.try_lock().ok()?;
    fresh_root_todos_snapshot_status(&cache)
}

fn try_begin_root_todos_snapshot_refresh_in_cache(
    cache: &mut RootTodoSnapshotCache,
) -> Option<RootOwnedProviderRefresh> {
    let cache_is_fresh = root_todos_snapshot_cache_is_fresh(cache);
    let refresh = cache.refresh_lifecycle.begin(
        sk_protocol::command_contract::CommandSource::Todo,
        cache_is_fresh,
    )?;
    tracing::debug!(
        target: "script_kit::search",
        source = "root-todos-snapshot-cache",
        generation = refresh.generation,
        "Started owned Todos snapshot refresh"
    );
    Some(refresh)
}

pub(crate) fn try_begin_root_todos_snapshot_refresh() -> Option<RootOwnedProviderRefresh> {
    let mut cache = ROOT_TODO_SNAPSHOT.lock().ok()?;
    try_begin_root_todos_snapshot_refresh_in_cache(&mut cache)
}

pub(crate) fn read_root_todos_snapshot(
    refresh: RootOwnedProviderRefresh,
) -> RootTodoRefreshSnapshot {
    assert!(
        !crate::runtime_policy::is_owned_evaluation(),
        "owned_source_snapshot_required"
    );
    RootTodoRefreshSnapshot {
        owner: refresh,
        hits: collect_day_page_todo_hits(&default_sk_path(), RECENT_DAY_PAGE_SCAN_LIMIT),
    }
}

pub(crate) fn owned_root_todos_snapshot(
    refresh: RootOwnedProviderRefresh,
    result: anyhow::Result<Vec<RootTodoSearchHit>>,
) -> anyhow::Result<RootTodoRefreshSnapshot> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    Ok(RootTodoRefreshSnapshot {
        owner: refresh,
        hits: result,
    })
}

pub(crate) fn reset_owned_root_todos() -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut cache = ROOT_TODO_SNAPSHOT
        .lock()
        .map_err(|_| anyhow::anyhow!("todos_cache_poisoned"))?;
    cache.refresh_lifecycle.next_generation = cache
        .refresh_lifecycle
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("todos_generation_exhausted"))?;
    cache.refresh_lifecycle.in_flight = None;
    cache.snapshot = None;
    cache.refresh_needed = false;
    Ok(())
}

pub(crate) fn invalidate_owned_root_todos_freshness() -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut cache = ROOT_TODO_SNAPSHOT
        .lock()
        .map_err(|_| anyhow::anyhow!("todos_cache_poisoned"))?;
    cache.refresh_lifecycle.next_generation = cache
        .refresh_lifecycle
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("todos_generation_exhausted"))?;
    cache.refresh_lifecycle.in_flight = None;
    cache.refresh_needed = true;
    Ok(())
}

fn finish_root_todos_snapshot_refresh_in_cache(
    cache: &mut RootTodoSnapshotCache,
    refresh: RootOwnedProviderRefresh,
    snapshot: RootTodoRefreshSnapshot,
) -> bool {
    if refresh.source != sk_protocol::command_contract::CommandSource::Todo
        || snapshot.owner != refresh
        || !cache.refresh_lifecycle.finish(refresh)
    {
        return false;
    }
    let Ok(hits) = snapshot.hits else {
        return false;
    };
    cache.snapshot = Some(RootTodoSnapshot {
        captured_at: crate::runtime_policy::root_search_now(),
        hits: std::sync::Arc::new(hits),
    });
    cache.refresh_needed = false;
    true
}

pub(crate) fn finish_root_todos_snapshot_refresh(
    refresh: RootOwnedProviderRefresh,
    snapshot: RootTodoRefreshSnapshot,
) -> bool {
    ROOT_TODO_SNAPSHOT.lock().is_ok_and(|mut cache| {
        finish_root_todos_snapshot_refresh_in_cache(&mut cache, refresh, snapshot)
    })
}

pub(crate) fn discard_root_todos_snapshot_refresh(refresh: RootOwnedProviderRefresh) -> bool {
    ROOT_TODO_SNAPSHOT
        .lock()
        .is_ok_and(|mut cache| cache.refresh_lifecycle.finish(refresh))
}

#[cfg(test)]
pub(crate) fn set_root_todo_snapshot_for_tests(hits: Vec<RootTodoSearchHit>) {
    if let Ok(mut cache) = ROOT_TODO_SNAPSHOT.lock() {
        cache.snapshot = Some(RootTodoSnapshot {
            captured_at: crate::runtime_policy::root_search_now(),
            hits: std::sync::Arc::new(hits),
        });
        cache.refresh_lifecycle.in_flight = None;
    }
}

fn brain_days_dir(sk_path: &Path) -> PathBuf {
    sk_path.join("brain").join("days")
}

fn recent_day_page_paths(sk_path: &Path, limit: usize) -> anyhow::Result<Vec<PathBuf>> {
    let days_dir = brain_days_dir(sk_path);
    let entries = match read_dir(&days_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if !path.metadata()?.is_file() {
            continue;
        }
        let Some(date) = path.file_name().and_then(|name| {
            name.to_str()?
                .strip_suffix(".md")?
                .parse::<NaiveDate>()
                .ok()
        }) else {
            continue;
        };
        paths.push((date, path));
    }
    paths.sort_by_key(|left| std::cmp::Reverse(left.0));
    Ok(paths
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect())
}

pub fn read_day_page_task_artifacts(sk_path: &Path) -> ReadArtifactReport {
    let mut report = ReadArtifactReport::default();
    let paths = match recent_day_page_paths(sk_path, RECENT_DAY_PAGE_SCAN_LIMIT) {
        Ok(paths) => paths,
        Err(error) => {
            push_warning(
                &mut report,
                format!("could not enumerate day pages: {error}"),
            );
            return report;
        }
    };
    for path in paths {
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                push_warning(
                    &mut report,
                    format!("could not read {}: {err}", path.display()),
                );
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
        };
        for (idx, line) in contents.lines().enumerate() {
            let Some(parsed) = parse_unchecked_task_line(line) else {
                continue;
            };
            report.entries.push(CaptureArtifact {
                kind: CaptureArtifactKind::Todo,
                path: path.clone(),
                line_number: Some(idx + 1),
                created_at: parsed.day_label.clone(),
                snippet: truncate_snippet(&parsed.body),
                raw_line: line.to_string(),
            });
        }
    }
    report
}

fn collect_day_page_todo_hits(
    sk_path: &Path,
    limit: usize,
) -> anyhow::Result<Vec<RootTodoSearchHit>> {
    let mut hits = Vec::new();
    for path in recent_day_page_paths(sk_path, limit)? {
        let day_label = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string());
        let contents = std::fs::read_to_string(&path)?;
        let lines: Vec<&str> = contents.lines().collect();
        for (idx, line) in lines.iter().enumerate().rev() {
            let Some(parsed) = parse_unchecked_task_line(line) else {
                continue;
            };
            let line_number = idx + 1;
            let stable_key = format!(
                "day/{}:{}",
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown.md".to_string()),
                line_number
            );
            let subtitle = todo_subtitle(
                &parsed.tags,
                parsed.priority,
                parsed.due.as_deref(),
                day_label.as_deref(),
            );
            hits.push(RootTodoSearchHit {
                stable_key,
                title: parsed.body.clone(),
                body: parsed.body,
                subtitle,
                tags: parsed.tags,
                priority: parsed.priority,
                due: parsed.due,
                created_at: day_label.clone(),
                path: path.clone(),
                line_number: Some(line_number),
                raw_line: line.to_string(),
            });
        }
    }
    Ok(hits)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedDayTaskLine {
    body: String,
    tags: Vec<String>,
    due: Option<String>,
    priority: Option<u8>,
    day_label: Option<String>,
}

fn parse_unchecked_task_line(line: &str) -> Option<ParsedDayTaskLine> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.contains("- [x]")
        || trimmed.contains("- [X]")
        || trimmed.starts_with('>')
    {
        return None;
    }
    let marker = "- [ ] ";
    let rest = trimmed.split_once(marker).map(|(_, tail)| tail.trim())?;
    let mut body_parts = Vec::new();
    let mut tags = Vec::new();
    let mut due = None;
    let mut priority = None;
    for token in rest.split_whitespace() {
        if let Some(tag) = token.strip_prefix('#') {
            if !tag.is_empty() {
                tags.push(tag.to_string());
            }
        } else if let Some(due_value) = token.strip_prefix("due:") {
            if !due_value.is_empty() {
                due = Some(due_value.to_string());
            }
        } else if token.len() == 2 {
            let lower = token.to_ascii_lowercase();
            if let Some(digits) = lower.strip_prefix('p') {
                if let Ok(value) = digits.parse::<u8>() {
                    if (1..=4).contains(&value) {
                        priority = Some(value);
                        continue;
                    }
                }
            }
            body_parts.push(token);
        } else {
            body_parts.push(token);
        }
    }
    let body = body_parts.join(" ");
    if body.trim().is_empty() {
        return None;
    }
    Some(ParsedDayTaskLine {
        body,
        tags,
        due,
        priority,
        day_label: None,
    })
}

pub fn search_root_object_candidates_in_sk_path(
    kind: crate::menu_syntax::CaptureObjectKind,
    query: &str,
    max_results: usize,
    sk_path: &Path,
) -> Vec<crate::menu_syntax::ObjectSelectorCandidate> {
    match kind {
        crate::menu_syntax::CaptureObjectKind::Todo => {
            let options = RootTodoSectionOptions {
                enabled: true,
                max_results,
                min_query_chars: 0,
            };
            search_root_todos_in_sk_path(query, options, sk_path)
                .into_iter()
                .map(|hit| crate::menu_syntax::ObjectSelectorCandidate {
                    kind,
                    id: hit.stable_key,
                    label: hit.title,
                    subtitle: hit.subtitle,
                })
                .collect()
        }
        crate::menu_syntax::CaptureObjectKind::Note => Vec::new(),
        crate::menu_syntax::CaptureObjectKind::Link => {
            let needle = normalize_match_text(query);
            let mut candidates =
                crate::scriptlets::link_markdown_store::link_object_candidates_from_markdown(
                    sk_path,
                )
                .unwrap_or_default();
            candidates.extend(search_jsonl_object_candidates(
                sk_path,
                CaptureArtifactKind::Bookmark,
                kind,
                query,
                max_results,
                "url",
                "title",
            ));
            candidates
                .into_iter()
                .filter(|candidate| {
                    if needle.is_empty() {
                        return true;
                    }
                    let haystack = format!(
                        "{} {} {}",
                        candidate.id, candidate.label, candidate.subtitle
                    );
                    normalize_match_text(&haystack).contains(&needle)
                })
                .take(max_results)
                .collect()
        }
        crate::menu_syntax::CaptureObjectKind::Snippet => {
            let needle = normalize_match_text(query);
            crate::scriptlets::snippet_markdown_store::snippet_object_candidates_from_markdown(
                sk_path,
            )
            .unwrap_or_default()
            .into_iter()
            .filter(|candidate| {
                if needle.is_empty() {
                    return true;
                }
                let haystack = format!(
                    "{} {} {}",
                    candidate.id, candidate.label, candidate.subtitle
                );
                normalize_match_text(&haystack).contains(&needle)
            })
            .take(max_results)
            .collect()
        }
    }
}

pub fn search_root_object_candidates_direct(
    kind: crate::menu_syntax::CaptureObjectKind,
    query: &str,
    max_results: usize,
) -> Vec<crate::menu_syntax::ObjectSelectorCandidate> {
    search_root_object_candidates_in_sk_path(kind, query, max_results, &default_sk_path())
}

fn search_jsonl_object_candidates(
    sk_path: &Path,
    artifact_kind: CaptureArtifactKind,
    object_kind: crate::menu_syntax::CaptureObjectKind,
    query: &str,
    max_results: usize,
    id_key: &str,
    label_key: &str,
) -> Vec<crate::menu_syntax::ObjectSelectorCandidate> {
    let filename = match object_kind {
        crate::menu_syntax::CaptureObjectKind::Link => CaptureArtifactKind::Bookmark.filename(),
        crate::menu_syntax::CaptureObjectKind::Snippet => "snippets.jsonl",
        _ => artifact_kind.filename(),
    };
    let path = sk_path.join("menu-syntax").join(filename);
    let report = read_jsonl_artifact(&path, artifact_kind);
    let needle = normalize_match_text(query);
    report
        .entries
        .into_iter()
        .rev()
        .filter_map(|artifact| {
            let parsed = serde_json::from_str::<serde_json::Value>(&artifact.raw_line).ok()?;
            if parsed
                .get("deletedAt")
                .and_then(|value| value.as_str())
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            {
                return None;
            }
            let id = parsed
                .get(id_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            let label = parsed
                .get(label_key)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(id.as_str())
                .to_string();
            let subtitle = match object_kind {
                crate::menu_syntax::CaptureObjectKind::Link => id.clone(),
                crate::menu_syntax::CaptureObjectKind::Snippet => parsed
                    .get("language")
                    .and_then(|value| value.as_str())
                    .map(|lang| format!("Snippet · {lang}"))
                    .unwrap_or_else(|| "Snippet".to_string()),
                _ => artifact.snippet.clone(),
            };
            let mut haystack = String::new();
            haystack.push_str(&id);
            haystack.push(' ');
            haystack.push_str(&label);
            haystack.push(' ');
            haystack.push_str(&subtitle);
            haystack.push(' ');
            haystack.push_str(&artifact.raw_line);
            if !needle.is_empty() && !normalize_match_text(&haystack).contains(&needle) {
                return None;
            }
            Some(crate::menu_syntax::ObjectSelectorCandidate {
                kind: object_kind,
                id,
                label,
                subtitle,
            })
        })
        .take(max_results)
        .collect()
}

fn todo_subtitle(
    tags: &[String],
    priority: Option<u8>,
    due: Option<&str>,
    created_at: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(priority) = priority {
        parts.push(format!("p{priority}"));
    }
    if let Some(due) = due.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("due {due}"));
    }
    if !tags.is_empty() {
        parts.push(
            tags.iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    if let Some(created_at) = created_at.filter(|value| !value.trim().is_empty()) {
        parts.push(created_at.to_string());
    }
    if parts.is_empty() {
        "Captured todo".to_string()
    } else {
        parts.join(" · ")
    }
}

fn todo_hit_matches(hit: &RootTodoSearchHit, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }
    let mut haystack = String::new();
    haystack.push_str(&hit.body);
    haystack.push(' ');
    haystack.push_str(&hit.raw_line);
    haystack.push(' ');
    haystack.push_str(&hit.tags.join(" "));
    if let Some(due) = &hit.due {
        haystack.push(' ');
        haystack.push_str(due);
    }
    normalize_match_text(&haystack).contains(normalized_query)
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn default_sk_path() -> PathBuf {
    if let Ok(path) = std::env::var(crate::setup::SK_PATH_ENV) {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".scriptkit"))
        .unwrap_or_else(|_| PathBuf::from(".scriptkit"))
}

fn push_warning(report: &mut ReadArtifactReport, msg: String) {
    if report.warnings.len() >= MAX_WARNINGS {
        return;
    }
    report.warnings.push(msg);
}

fn extract_created_at(value: &serde_json::Value) -> Option<String> {
    let obj = value.as_object()?;
    // Shipped examples + scaffold use `createdAt`. Payload tempfiles use
    // `timestamp` (see `execute.rs`). Check both without privileging either.
    for key in ["createdAt", "timestamp"] {
        if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}

fn snippet_for_value(value: &serde_json::Value) -> String {
    let text = match value {
        serde_json::Value::Object(map) => {
            // Prefer human-readable fields in order: body, raw, target, then
            // the stringified object as a last resort. This keeps the snippet
            // meaningful for every shipped target shape.
            for key in ["body", "raw", "target", "url", "title"] {
                if let Some(v) = map.get(key).and_then(|v| v.as_str()) {
                    return truncate_snippet(v);
                }
            }
            serde_json::to_string(value).unwrap_or_default()
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    truncate_snippet(&text)
}

fn truncate_snippet(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_SNIPPET_CHARS {
        return collapsed;
    }
    let mut out = String::with_capacity(MAX_SNIPPET_CHARS + 1);
    for (taken, ch) in collapsed.chars().enumerate() {
        if taken >= MAX_SNIPPET_CHARS.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_todo_snapshots_and_resets_require_runtime_authority() {
        let refresh = RootOwnedProviderRefresh {
            source: sk_protocol::command_contract::CommandSource::Todo,
            generation: 1,
        };
        assert!(owned_root_todos_snapshot(refresh, Ok(Vec::new())).is_err());
        assert!(reset_owned_root_todos().is_err());
        assert!(invalidate_owned_root_todos_freshness().is_err());
    }
    use std::fs;
    use tempfile::TempDir;

    fn snapshot_hit(body: &str) -> RootTodoSearchHit {
        RootTodoSearchHit {
            stable_key: format!("day/test.md:{}", body.len()),
            title: body.to_string(),
            body: body.to_string(),
            subtitle: String::new(),
            tags: Vec::new(),
            priority: None,
            due: None,
            created_at: None,
            path: PathBuf::from("/tmp/test.md"),
            line_number: Some(1),
            raw_line: format!("- [ ] {body}"),
        }
    }

    fn root_owned_todo_cache() -> RootTodoSnapshotCache {
        RootTodoSnapshotCache {
            snapshot: None,
            refresh_needed: false,
            refresh_lifecycle: RootOwnedProviderRefreshLifecycle::default(),
        }
    }

    #[test]
    fn root_todos_owned_refresh_accepts_empty_snapshot_without_restarting_forever() {
        let mut cache = root_owned_todo_cache();
        let refresh = try_begin_root_todos_snapshot_refresh_in_cache(&mut cache)
            .expect("cold Todos cache owns one worker");
        assert!(try_begin_root_todos_snapshot_refresh_in_cache(&mut cache).is_none());
        assert!(finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            refresh,
            RootTodoRefreshSnapshot {
                owner: refresh,
                hits: Ok(Vec::new()),
            },
        ));
        assert!(root_todos_snapshot_cache_is_fresh(&cache));
        assert!(cache
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.hits.is_empty()));
        assert!(try_begin_root_todos_snapshot_refresh_in_cache(&mut cache).is_none());
    }

    #[test]
    fn fresh_todos_cache_proof_rejects_missing_stale_invalidated_and_in_flight() {
        let mut cache = root_owned_todo_cache();
        assert!(fresh_root_todos_snapshot_status(&cache).is_none());
        let refresh = try_begin_root_todos_snapshot_refresh_in_cache(&mut cache).unwrap();
        assert!(fresh_root_todos_snapshot_status(&cache).is_none());
        assert!(finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            refresh,
            RootTodoRefreshSnapshot {
                owner: refresh,
                hits: Ok(Vec::new())
            },
        ));
        assert_eq!(
            fresh_root_todos_snapshot_status(&cache),
            Some((refresh.generation, 0))
        );
        cache.refresh_lifecycle.in_flight = Some(refresh);
        assert!(fresh_root_todos_snapshot_status(&cache).is_none());
        cache.refresh_lifecycle.in_flight = None;
        cache.refresh_needed = true;
        assert!(fresh_root_todos_snapshot_status(&cache).is_none());
        cache.refresh_needed = false;
        cache.snapshot.as_mut().unwrap().captured_at = crate::runtime_policy::root_search_now()
            - ROOT_TODO_SNAPSHOT_TTL
            - std::time::Duration::from_secs(1);
        assert!(fresh_root_todos_snapshot_status(&cache).is_none());
    }

    #[test]
    fn root_todos_owned_refresh_rejects_foreign_owner_and_snapshot_before_publication() {
        let mut cache = root_owned_todo_cache();
        let refresh = try_begin_root_todos_snapshot_refresh_in_cache(&mut cache)
            .expect("Todos owns its private day-page snapshot");
        let forged = RootOwnedProviderRefresh {
            source: sk_protocol::command_contract::CommandSource::Note,
            generation: refresh.generation,
        };

        assert!(!finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            forged,
            RootTodoRefreshSnapshot {
                owner: forged,
                hits: Ok(vec![snapshot_hit("foreign private todo")]),
            },
        ));
        assert!(!finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            refresh,
            RootTodoRefreshSnapshot {
                owner: forged,
                hits: Ok(vec![snapshot_hit("wrong private snapshot")]),
            },
        ));
        assert!(cache.snapshot.is_none());
        assert_eq!(cache.refresh_lifecycle.in_flight, Some(refresh));
        assert!(finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            refresh,
            RootTodoRefreshSnapshot {
                owner: refresh,
                hits: Ok(vec![snapshot_hit("the real private todo")]),
            },
        ));
        assert_eq!(
            cache.snapshot.unwrap().hits[0].body,
            "the real private todo"
        );
    }

    #[test]
    fn root_todos_owned_refresh_rejects_stale_completion_after_worker_replacement() {
        let mut cache = root_owned_todo_cache();
        let stale = try_begin_root_todos_snapshot_refresh_in_cache(&mut cache)
            .expect("initial Todos worker");
        assert!(cache.refresh_lifecycle.finish(stale));
        let current = try_begin_root_todos_snapshot_refresh_in_cache(&mut cache)
            .expect("replacement Todos worker");
        assert!(current.generation > stale.generation);

        assert!(!finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            stale,
            RootTodoRefreshSnapshot {
                owner: stale,
                hits: Ok(vec![snapshot_hit("stale private todo")]),
            },
        ));
        assert!(cache.snapshot.is_none());
        assert_eq!(cache.refresh_lifecycle.in_flight, Some(current));
        assert!(finish_root_todos_snapshot_refresh_in_cache(
            &mut cache,
            current,
            RootTodoRefreshSnapshot {
                owner: current,
                hits: Ok(vec![snapshot_hit("fresh private todo")]),
            },
        ));
    }

    #[test]
    fn root_todos_failed_read_is_not_empty_success_and_preserves_last_good_rows() {
        let mut cache = root_owned_todo_cache();
        let refresh = try_begin_root_todos_snapshot_refresh_in_cache(&mut cache).unwrap();
        cache.snapshot = Some(RootTodoSnapshot {
            captured_at: crate::runtime_policy::root_search_now(),
            hits: std::sync::Arc::new(vec![snapshot_hit("last good")]),
        });
        let snapshot = RootTodoRefreshSnapshot {
            owner: refresh,
            hits: Err(anyhow::anyhow!("read failed")),
        };
        assert!(snapshot.read_outcome().is_err());
        assert!(!finish_root_todos_snapshot_refresh_in_cache(
            &mut cache, refresh, snapshot
        ));
        assert_eq!(cache.snapshot.unwrap().hits[0].body, "last good");
        assert!(cache.refresh_lifecycle.in_flight.is_none());
    }

    #[test]
    fn cached_todo_lookup_filters_injected_snapshot_without_io() {
        set_root_todo_snapshot_for_tests(vec![
            snapshot_hit("renew passport"),
            snapshot_hit("buy milk"),
        ]);
        let options = RootTodoSectionOptions {
            enabled: true,
            max_results: 10,
            min_query_chars: 3,
        };

        let hits = search_root_todos_cached("passport", options);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].body, "renew passport");

        // Below min_query_chars: ineligible, no snapshot scan.
        assert!(search_root_todos_cached("pa", options).is_empty());
    }

    fn write_file(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir -p");
        }
        fs::write(&path, contents).expect("write artifact fixture");
        path
    }

    fn write_day_page_task(dir: &Path, date: &str, line: &str) -> PathBuf {
        let path = dir.join("brain").join("days").join(format!("{date}.md"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir day page parent");
        }
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("open day page");
        writeln!(file, "{line}").expect("append day page line");
        path
    }

    #[test]
    fn read_day_page_task_artifacts_returns_unchecked_task_lines() {
        let tmp = TempDir::new().expect("tempdir");
        write_day_page_task(tmp.path(), "2026-05-19", "09:00 - [ ] buy milk");
        write_day_page_task(tmp.path(), "2026-05-19", "09:01 - [ ] walk dog");
        let report = read_day_page_task_artifacts(tmp.path());
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].snippet, "buy milk");
        assert_eq!(report.entries[0].line_number, Some(1));
        assert_eq!(report.entries[1].line_number, Some(2));
    }

    #[test]
    fn search_root_todos_reads_newest_first_and_matches_tags_due_and_body() {
        let tmp = TempDir::new().expect("tempdir");
        write_day_page_task(
            tmp.path(),
            "2026-05-19",
            "09:00 - [ ] renew passport #errands p1",
        );
        write_day_page_task(
            tmp.path(),
            "2026-05-20",
            "10:00 - [ ] book design review #work due:Friday",
        );

        let hits = search_root_todos_in_sk_path(
            "work",
            RootTodoSectionOptions {
                max_results: 10,
                ..Default::default()
            },
            tmp.path(),
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "book design review");
        assert!(hits[0].subtitle.contains("#work"));
        assert!(hits[0].subtitle.contains("due Friday"));

        let hits =
            search_root_todos_in_sk_path("passport", RootTodoSectionOptions::default(), tmp.path());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].priority, Some(1));
        assert_eq!(hits[0].line_number, Some(1));
    }

    #[test]
    fn root_todo_object_candidates_use_day_page_line_keys() {
        let tmp = TempDir::new().expect("tempdir");
        write_day_page_task(
            tmp.path(),
            "2026-05-20",
            "09:00 - [ ] review selected mutation",
        );

        let hits = search_root_object_candidates_in_sk_path(
            crate::menu_syntax::CaptureObjectKind::Todo,
            "selected",
            10,
            tmp.path(),
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "day/2026-05-20.md:1");
        assert_eq!(hits[0].label, "review selected mutation");
    }

    #[test]
    fn link_object_candidates_use_url_as_selected_ref_id_and_filter_deleted() {
        let tmp = TempDir::new().expect("tempdir");
        let base = tmp.path().join("menu-syntax");
        write_file(
            &base,
            "bookmarks.jsonl",
            r#"{"url":"https://old.example","title":"Old","deletedAt":"2026-05-20T10:00:00Z"}
{"url":"https://example.com","title":"Example Docs","tags":["docs"],"deletedAt":null}
"#,
        );

        let hits = search_root_object_candidates_in_sk_path(
            crate::menu_syntax::CaptureObjectKind::Link,
            "docs",
            10,
            tmp.path(),
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "https://example.com");
        assert_eq!(hits[0].label, "Example Docs");
        assert_eq!(hits[0].subtitle, "https://example.com");
    }

    #[test]
    fn snippet_object_candidates_use_trigger_as_selected_ref_id_and_filter_deleted() {
        let tmp = TempDir::new().expect("tempdir");
        let base = tmp.path().join("menu-syntax");
        write_file(
            &base,
            "snippets.jsonl",
            r#"{"trigger":"old","body":"deleted snippet","deletedAt":"2026-05-20T10:00:00Z"}
{"trigger":"fj","body":"const res = await fetch(url)","language":"ts","deletedAt":null}
"#,
        );

        let hits = search_root_object_candidates_in_sk_path(
            crate::menu_syntax::CaptureObjectKind::Snippet,
            "fetch",
            10,
            tmp.path(),
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "fj");
        assert_eq!(hits[0].label, "const res = await fetch(url)");
        assert_eq!(hits[0].subtitle, "Snippet · ts");
    }

    #[test]
    fn search_root_todos_ignores_checked_task_lines() {
        let tmp = TempDir::new().expect("tempdir");
        write_day_page_task(
            tmp.path(),
            "2026-05-19",
            "09:00 - [x] old hidden task\n10:00 - [ ] visible task",
        );

        let hits =
            search_root_todos_in_sk_path("task", RootTodoSectionOptions::default(), tmp.path());

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "visible task");
    }

    #[test]
    fn read_jsonl_artifact_skips_malformed_lines_with_warning() {
        let tmp = TempDir::new().expect("tempdir");
        let path = write_file(
            tmp.path(),
            "notes.jsonl",
            r#"{"body":"ok"}
this is not json
{"body":"also ok"}
{oops: true}
"#,
        );
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::Note);
        assert_eq!(report.entries.len(), 2, "only valid JSON lines surface");
        assert_eq!(report.skipped, 2, "each malformed line bumps skipped");
        assert!(
            report.warnings.iter().any(|w| w.contains("invalid JSON")),
            "should describe the failure"
        );
    }

    #[test]
    fn read_jsonl_artifact_handles_missing_file_gracefully() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("does-not-exist.jsonl");
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::Note);
        assert!(report.entries.is_empty());
        assert_eq!(report.skipped, 0);
        assert!(
            report.warnings.is_empty(),
            "missing file is not a warning — it just means the user hasn't captured yet"
        );
    }

    #[test]
    fn read_jsonl_artifact_ignores_blank_lines() {
        let tmp = TempDir::new().expect("tempdir");
        let path = write_file(
            tmp.path(),
            "notes.jsonl",
            "\n\n{\"body\":\"ok\"}\n  \n{\"body\":\"also\"}\n\n",
        );
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::Note);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.skipped, 0);
    }

    #[test]
    fn read_jsonl_artifact_truncates_snippet_for_long_bodies() {
        let long_body: String = "x".repeat(MAX_SNIPPET_CHARS * 3);
        let line = format!(
            r#"{{"body":"{}","createdAt":"2026-04-24T00:00:00Z"}}"#,
            long_body
        );
        let tmp = TempDir::new().expect("tempdir");
        let path = write_file(tmp.path(), "notes.jsonl", &format!("{line}\n"));
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::Note);
        assert_eq!(report.entries.len(), 1);
        let snippet = &report.entries[0].snippet;
        assert!(
            snippet.chars().count() <= MAX_SNIPPET_CHARS,
            "snippet respects cap, got {} chars",
            snippet.chars().count()
        );
        assert!(
            snippet.ends_with('…'),
            "truncated snippet ends with ellipsis"
        );
    }

    #[test]
    fn snippet_falls_back_to_raw_when_body_is_missing() {
        let tmp = TempDir::new().expect("tempdir");
        let path = write_file(
            tmp.path(),
            "events.jsonl",
            r#"{"title":"Standup","raw":";cal standup tomorrow 3pm"}
"#,
        );
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::CalendarEvent);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].snippet, ";cal standup tomorrow 3pm");
    }

    #[test]
    fn non_object_top_level_json_is_still_included() {
        // Unusual but legal JSONL — a scalar per line. Reader must not drop it.
        let tmp = TempDir::new().expect("tempdir");
        let path = write_file(tmp.path(), "notes.jsonl", "\"just a string\"\n42\n");
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::Note);
        assert_eq!(report.entries.len(), 2);
        assert!(report.entries[0].snippet.contains("just a string"));
        assert_eq!(report.entries[1].snippet, "42");
    }

    #[test]
    fn read_payload_dir_returns_only_capture_v1_files() {
        let tmp = TempDir::new().expect("tempdir");
        let dir = tmp.path().join("payloads");
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(
            dir.join("capture_v1-abc.json"),
            r#"{"body":"hello","timestamp":"2026-04-24T01:00:00Z"}"#,
        )
        .unwrap();
        fs::write(dir.join("unrelated.json"), r#"{"unrelated":true}"#).unwrap();
        fs::write(dir.join("capture_v1-bad.json"), "not json").unwrap();
        let report = read_payload_dir(&dir);
        assert_eq!(report.entries.len(), 1, "only capture_v1-*.json succeeds");
        assert_eq!(report.skipped, 1, "bad payload file counts as skipped");
        assert_eq!(report.entries[0].kind, CaptureArtifactKind::Payload);
        assert!(report.entries[0].line_number.is_none());
        assert_eq!(
            report.entries[0].created_at.as_deref(),
            Some("2026-04-24T01:00:00Z")
        );
    }

    #[test]
    fn read_payload_dir_handles_missing_dir_gracefully() {
        let tmp = TempDir::new().expect("tempdir");
        let report = read_payload_dir(&tmp.path().join("never-existed"));
        assert!(report.entries.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn read_all_artifacts_combines_every_kind() {
        let tmp = TempDir::new().expect("tempdir");
        let sk = tmp.path().join(".scriptkit");
        let base = sk.join("menu-syntax");

        write_day_page_task(&sk, "2026-05-20", "09:00 - [ ] task line");
        write_file(&base, "events.jsonl", "{\"body\":\"e\"}\n");
        write_file(&base, "notes.jsonl", "{\"body\":\"n\"}\n");
        write_file(&base, "drafts.jsonl", "{\"body\":\"d\"}\n");
        write_file(
            &base,
            "bookmarks.jsonl",
            "{\"body\":\"b\",\"url\":\"https://x\"}\n",
        );
        fs::create_dir_all(base.join("payloads")).unwrap();
        fs::write(
            base.join("payloads/capture_v1-1.json"),
            r#"{"body":"payload-body"}"#,
        )
        .unwrap();

        let report = read_all_artifacts(&sk);
        assert_eq!(report.entries.len(), 6);
        let kinds: Vec<_> = report.entries.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                CaptureArtifactKind::Todo,
                CaptureArtifactKind::CalendarEvent,
                CaptureArtifactKind::Note,
                CaptureArtifactKind::SocialDraft,
                CaptureArtifactKind::Bookmark,
                CaptureArtifactKind::Payload,
            ],
            "merge order matches BROWSER_ORDER then Payload last"
        );
    }

    #[test]
    fn read_all_artifacts_counts_warnings_across_files() {
        let tmp = TempDir::new().expect("tempdir");
        let sk = tmp.path().join(".scriptkit");
        let base = sk.join("menu-syntax");
        write_file(&base, "events.jsonl", "nope\n");
        let report = read_all_artifacts(&sk);
        assert_eq!(report.entries.len(), 0, "only valid lines surface");
        assert_eq!(report.skipped, 1, "dirty rows across files accumulate");
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn warning_cap_prevents_unbounded_accumulation() {
        let tmp = TempDir::new().expect("tempdir");
        let mut buf = String::new();
        for _ in 0..(MAX_WARNINGS * 3) {
            buf.push_str("garbage\n");
        }
        let path = write_file(tmp.path(), "notes.jsonl", &buf);
        let report = read_jsonl_artifact(&path, CaptureArtifactKind::Note);
        assert_eq!(report.entries.len(), 0);
        assert_eq!(
            report.warnings.len(),
            MAX_WARNINGS,
            "warnings must not grow unbounded"
        );
        assert!(
            report.skipped >= MAX_WARNINGS,
            "all dirty rows still count as skipped ({} rows)",
            report.skipped
        );
    }

    #[test]
    fn artifact_filename_for_matches_templates_and_shipped_examples() {
        assert_eq!(CaptureArtifactKind::Todo.filename(), "day-pages");
        assert_eq!(
            CaptureArtifactKind::CalendarEvent.filename(),
            "events.jsonl"
        );
        assert_eq!(CaptureArtifactKind::Note.filename(), "notes.jsonl");
        assert_eq!(CaptureArtifactKind::SocialDraft.filename(), "drafts.jsonl");
        assert_eq!(CaptureArtifactKind::Bookmark.filename(), "bookmarks.jsonl");
        assert_eq!(CaptureArtifactKind::Payload.filename(), "payloads");
    }

    #[test]
    fn browser_order_excludes_payload() {
        assert!(
            !CaptureArtifactKind::BROWSER_ORDER
                .iter()
                .any(|k| *k == CaptureArtifactKind::Payload),
            "payload is retention-only, not in the user-facing browser order"
        );
    }
}

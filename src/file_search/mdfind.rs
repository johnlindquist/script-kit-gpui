use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use tracing::{debug, instrument};

use super::{
    build_mdquery, detect_file_type, expand_path, looks_like_advanced_mdquery, FileResult,
};

const FILESYSTEM_FALLBACK_MAX_VISITED: usize = 75_000;
const MDFIND_TIMEOUT: Duration = Duration::from_secs(3);
const MDFIND_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Events emitted during streaming search
#[derive(Debug, Clone)]
pub enum SearchEvent {
    /// A new file result was found
    Result(FileResult),
    /// Exactly one terminal result follows streamed rows; failed batches are not committed.
    Done(Result<(), SearchFailure>),
}

#[derive(Debug, Clone)]
pub enum SearchFailure {
    Source(Arc<std::io::Error>),
    Cancelled,
    Disconnected,
}

impl From<std::io::Error> for SearchFailure {
    fn from(error: std::io::Error) -> Self {
        Self::Source(Arc::new(error))
    }
}

impl std::fmt::Display for SearchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => std::fmt::Display::fmt(error, formatter),
            Self::Cancelled => formatter.write_str("file search cancelled"),
            Self::Disconnected => formatter.write_str("file search output worker disconnected"),
        }
    }
}

impl std::error::Error for SearchFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Cancel token for streaming searches
///
/// Set to `true` to cancel an in-flight search.
/// The search thread will check this token and stop early.
pub type CancelToken = Arc<AtomicBool>;

/// Create a new cancel token
pub fn new_cancel_token() -> CancelToken {
    Arc::new(AtomicBool::new(false))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchFilesStreamingOptions {
    pub skip_metadata: bool,
    pub allow_filesystem_fallback: bool,
}

impl SearchFilesStreamingOptions {
    pub fn dedicated_file_search(skip_metadata: bool) -> Self {
        Self {
            skip_metadata,
            allow_filesystem_fallback: true,
        }
    }

    pub fn root_search() -> Self {
        Self {
            skip_metadata: true,
            allow_filesystem_fallback: false,
        }
    }
}

/// Search for files using macOS mdfind (Spotlight)
///
/// Uses streaming to avoid buffering all results when only `limit` are needed.
/// Converts simple queries to filename-matching mdfind queries.
///
/// # Arguments
/// * `query` - Search query string (will be converted to filename query if simple)
/// * `onlyin` - Optional directory to limit search scope
/// * `limit` - Maximum number of results to return
///
/// # Returns
/// Matching files, or the original source failure. Partial failed batches are discarded.
#[instrument(skip_all, fields(query = %query, onlyin = ?onlyin, limit = limit))]
pub fn search_files(
    query: &str,
    onlyin: Option<&str>,
    limit: usize,
) -> Result<Vec<FileResult>, SearchFailure> {
    let mut results = Vec::new();
    stream_file_search(
        query,
        onlyin,
        limit,
        &new_cancel_token(),
        SearchFilesStreamingOptions::dedicated_file_search(false),
        &mut |event| {
            if let SearchEvent::Result(file) = event {
                results.push(file);
            }
        },
    )?;
    Ok(results)
}

/// Streaming search: yields results as they arrive via callback.
///
/// This is the preferred API for real-time search UX because:
/// - Results appear immediately as mdfind outputs them
/// - Cancellation actually stops work (kills mdfind process)
/// - Caller can batch UI updates however they want
///
/// # Arguments
/// * `query` - Search query string
/// * `onlyin` - Optional directory to limit search scope
/// * `limit` - Maximum number of results to return
/// * `cancel` - Cancel token; set to true to stop search and kill mdfind
/// * `skip_metadata` - If true, skip stat() calls for faster results (size/modified = 0)
/// * `on_event` - Callback receiving SearchEvent for each result and final Done
///
/// # Example
/// ```ignore
/// let cancel = file_search::new_cancel_token();
/// let cancel_clone = cancel.clone();
///
/// // Start search in background thread
/// std::thread::spawn(move || {
///     file_search::search_files_streaming(
///         "query",
///         None,
///         500,
///         cancel_clone,
///         false, // include metadata
///         |event| {
///             // Send event to UI thread via channel
///             let _ = tx.send(event);
///         },
///     );
/// });
///
/// // Later, to cancel:
/// cancel.store(true, Ordering::Relaxed);
/// ```
#[instrument(skip_all, fields(query = %query, onlyin = ?onlyin, limit = limit, skip_metadata = skip_metadata))]
pub fn search_files_streaming<F>(
    query: &str,
    onlyin: Option<&str>,
    limit: usize,
    cancel: CancelToken,
    skip_metadata: bool,
    on_event: F,
) where
    F: FnMut(SearchEvent),
{
    search_files_streaming_with_options(
        query,
        onlyin,
        limit,
        cancel,
        SearchFilesStreamingOptions::dedicated_file_search(skip_metadata),
        on_event,
    );
}

#[instrument(skip_all, fields(query = %query, onlyin = ?onlyin, limit = limit, skip_metadata = options.skip_metadata, allow_filesystem_fallback = options.allow_filesystem_fallback))]
pub fn search_files_streaming_with_options<F>(
    query: &str,
    onlyin: Option<&str>,
    limit: usize,
    cancel: CancelToken,
    options: SearchFilesStreamingOptions,
    mut on_event: F,
) where
    F: FnMut(SearchEvent),
{
    let result = stream_file_search(query, onlyin, limit, &cancel, options, &mut on_event);
    on_event(SearchEvent::Done(result));
}

fn stream_file_search<F: FnMut(SearchEvent)>(
    query: &str,
    onlyin: Option<&str>,
    limit: usize,
    cancel: &CancelToken,
    options: SearchFilesStreamingOptions,
    on_event: &mut F,
) -> Result<(), SearchFailure> {
    if cancel.load(Ordering::Relaxed) {
        return Err(SearchFailure::Cancelled);
    }
    if query.trim().is_empty() || limit == 0 {
        return Ok(());
    }
    let mdquery = build_mdquery(query);
    debug!(mdquery = %mdquery, "Built mdfind query for streaming");
    let mut command = Command::new("mdfind");
    if let Some(directory) = onlyin {
        command.arg("-onlyin").arg(directory);
    }
    let child = command
        .arg(mdquery)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let count = stream_mdfind_child(
        child,
        limit,
        cancel,
        options.skip_metadata,
        Instant::now() + MDFIND_TIMEOUT,
        on_event,
    )?;
    if options.allow_filesystem_fallback && count == 0 && !looks_like_advanced_mdquery(query) {
        for result in search_files_filesystem_fallback(query, onlyin, limit)? {
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchFailure::Cancelled);
            }
            on_event(SearchEvent::Result(result));
        }
    }
    if cancel.load(Ordering::Relaxed) {
        Err(SearchFailure::Cancelled)
    } else {
        Ok(())
    }
}

fn stream_mdfind_child<F: FnMut(SearchEvent)>(
    mut child: Child,
    limit: usize,
    cancel: &CancelToken,
    skip_metadata: bool,
    deadline: Instant,
    on_event: &mut F,
) -> Result<usize, SearchFailure> {
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(std::io::Error::other("mdfind stdout was not piped").into());
    };
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let failed = line.is_err();
            if line_tx.send(line.map(Some)).is_err() || failed {
                return;
            }
        }
        // EOF is an explicit protocol value, not an inferred sender disconnect.
        let _ = line_tx.send(Ok(None));
    });
    let mut count = 0usize;
    let mut limited = false;
    let outcome = loop {
        if cancel.load(Ordering::Relaxed) {
            break Err(SearchFailure::Cancelled);
        }
        if count >= limit {
            limited = true;
            break Ok(());
        }
        match line_rx.recv_timeout(MDFIND_POLL_INTERVAL) {
            Ok(Ok(Some(line))) => {
                if let Some(result) = file_result_from_mdfind_line(line, skip_metadata) {
                    on_event(SearchEvent::Result(result));
                    count += 1;
                }
            }
            Ok(Ok(None)) => break Ok(()),
            Ok(Err(error)) => break Err(error.into()),
            Err(RecvTimeoutError::Disconnected) => break Err(SearchFailure::Disconnected),
            Err(RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    break Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "mdfind exceeded its source deadline",
                    )
                    .into());
                }
            }
        }
    };
    if limited || outcome.is_err() {
        let _ = child.kill();
    }
    let status = child.wait();
    if reader.join().is_err() {
        return Err(SearchFailure::Disconnected);
    }
    outcome?;
    let status = status?;
    if !limited && !status.success() {
        return Err(std::io::Error::other(format!("mdfind exited with {status}")).into());
    }
    Ok(count)
}

fn file_result_from_mdfind_line(line: String, skip_metadata: bool) -> Option<FileResult> {
    // Only skip truly empty lines, not lines with spaces.
    // .lines() already strips newline characters; macOS paths can contain
    // leading/trailing spaces, so do not trim.
    if line.is_empty() {
        return None;
    }

    let path = Path::new(&line);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let (size, modified) = if skip_metadata {
        (0, 0)
    } else {
        std::fs::metadata(path)
            .map(|m| {
                (
                    m.len(),
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                )
            })
            .unwrap_or((0, 0))
    };

    let file_type = detect_file_type(path);

    Some(FileResult {
        path: line,
        name,
        size,
        modified,
        file_type,
    })
}

fn search_files_filesystem_fallback(
    query: &str,
    onlyin: Option<&str>,
    limit: usize,
) -> std::io::Result<Vec<FileResult>> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let roots = if let Some(directory) = onlyin {
        let expanded = expand_path(directory).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid file search scope",
            )
        })?;
        vec![PathBuf::from(expanded).canonicalize()?]
    } else {
        fallback_roots()
    };
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let mut visited = 0usize;
    let mut stack = roots;
    let mut first_directory = true;

    while let Some(dir) = stack.pop() {
        if results.len() >= limit || visited >= FILESYSTEM_FALLBACK_MAX_VISITED {
            break;
        }

        let required = onlyin.is_some() && first_directory;
        first_directory = false;
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if required => return Err(error),
            Err(_) => continue,
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if required => return Err(error),
                Err(_) => continue,
            };
            if results.len() >= limit || visited >= FILESYSTEM_FALLBACK_MAX_VISITED {
                break;
            }
            visited += 1;

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.is_dir() {
                if should_skip_fallback_dir(&name) {
                    continue;
                }
                stack.push(path.clone());
            }

            if !name.to_lowercase().contains(&needle) {
                continue;
            }

            let Some(path_str) = path.to_str().map(str::to_string) else {
                continue;
            };
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            results.push(FileResult {
                path: path_str,
                name,
                size: metadata.len(),
                modified,
                file_type: detect_file_type(&path),
            });
        }
    }

    results.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(results)
}

fn fallback_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(home) = dirs::home_dir() {
        push_fallback_root(&mut roots, home.clone());
        for child in [
            "Desktop",
            "Documents",
            "Downloads",
            "dev",
            "Developer",
            "Projects",
        ] {
            push_fallback_root(&mut roots, home.join(child));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        push_fallback_root(&mut roots, cwd);
    }

    roots
}

fn push_fallback_root(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if !path.is_dir() {
        return;
    }
    let canonical = path.canonicalize().unwrap_or(path);
    if !roots.iter().any(|existing| existing == &canonical) {
        roots.push(canonical);
    }
}

/// Most-recently-modified files under `root`, for scopes Spotlight cannot
/// serve (hidden dot-directory cwds like `~/.scriptkit`). Bounded walk with
/// the same skip rules as the search fallback, sorted by mtime descending.
/// Hidden files are skipped for this landing-state seed; typing a sub-query
/// still finds them through the search fallback.
pub fn recent_files_filesystem(root: &Path, limit: usize) -> std::io::Result<Vec<FileResult>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let mut visited = 0usize;
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if visited >= FILESYSTEM_FALLBACK_MAX_VISITED {
            break;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == root => return Err(error),
            Err(_) => continue,
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if dir == root => return Err(error),
                Err(_) => continue,
            };
            if visited >= FILESYSTEM_FALLBACK_MAX_VISITED {
                break;
            }
            visited += 1;

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.is_dir() {
                if !should_skip_fallback_dir(&name) && !name.starts_with('.') {
                    stack.push(path);
                }
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            let Some(path_str) = path.to_str().map(str::to_string) else {
                continue;
            };

            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            results.push(FileResult {
                path: path_str,
                name,
                size: metadata.len(),
                modified,
                file_type: detect_file_type(&path),
            });
        }
    }

    results.sort_by_key(|a| std::cmp::Reverse(a.modified));
    results.truncate(limit);
    Ok(results)
}

fn should_skip_fallback_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".Trash"
            | ".cache"
            | "Library"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".turbo"
    )
}

#[cfg(test)]
mod tests {
    use super::{search_files_filesystem_fallback, SearchFilesStreamingOptions};

    #[test]
    fn streaming_options_keep_dedicated_file_search_fallback_enabled() {
        let options = SearchFilesStreamingOptions::dedicated_file_search(true);
        assert!(options.skip_metadata);
        assert!(options.allow_filesystem_fallback);
    }

    #[test]
    fn streaming_options_disable_root_search_fallback() {
        let options = SearchFilesStreamingOptions::root_search();
        assert!(options.skip_metadata);
        assert!(!options.allow_filesystem_fallback);
    }

    #[test]
    fn filesystem_fallback_finds_files_inside_onlyin_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let wanted = temp.path().join("script-kit-search-target.txt");
        std::fs::write(&wanted, "fixture").expect("write fixture");

        let results = search_files_filesystem_fallback("search-target", temp.path().to_str(), 10)
            .expect("read search scope");

        // The walker canonicalizes its roots (macOS tempdirs live behind the
        // /var → /private/var symlink), so compare canonical paths.
        let wanted = wanted.canonicalize().expect("canonicalize fixture path");
        assert!(
            results
                .iter()
                .any(|entry| std::path::Path::new(&entry.path) == wanted),
            "fallback should find filename matches under onlyin"
        );
    }

    #[test]
    fn recent_files_walk_skips_hidden_and_noise_dirs() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::write(temp.path().join("visible.txt"), "fixture").expect("write fixture");
        std::fs::write(temp.path().join(".hidden.txt"), "fixture").expect("write fixture");
        std::fs::create_dir(temp.path().join("node_modules")).expect("mkdir");
        std::fs::write(temp.path().join("node_modules/dep.js"), "fixture").expect("write fixture");
        std::fs::create_dir(temp.path().join("src")).expect("mkdir");
        std::fs::write(temp.path().join("src/nested.rs"), "fixture").expect("write fixture");

        let results = super::recent_files_filesystem(temp.path(), 10).expect("read recent scope");
        let names: Vec<&str> = results.iter().map(|entry| entry.name.as_str()).collect();

        assert!(names.contains(&"visible.txt"), "top-level file: {names:?}");
        assert!(names.contains(&"nested.rs"), "nested file: {names:?}");
        assert!(
            !names.contains(&".hidden.txt"),
            "hidden files are not recents seeds: {names:?}"
        );
        assert!(
            !names.contains(&"dep.js"),
            "noise dirs (node_modules) are skipped: {names:?}"
        );
    }

    #[test]
    fn filesystem_fallback_respects_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        for ix in 0..3 {
            std::fs::write(
                temp.path().join(format!("limit-target-{ix}.txt")),
                "fixture",
            )
            .expect("write fixture");
        }

        let results = search_files_filesystem_fallback("limit-target", temp.path().to_str(), 2)
            .expect("read search scope");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filesystem_sources_do_not_turn_missing_scope_into_empty_success() {
        let temp = tempfile::tempdir().expect("temp directory");
        let missing = temp.path().join("missing");
        assert_eq!(
            search_files_filesystem_fallback("needle", missing.to_str(), 10)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            super::recent_files_filesystem(&missing, 10)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert!(super::recent_files_filesystem(&missing, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn native_partial_stdout_with_failed_exit_is_not_success() {
        let child = super::Command::new("/bin/sh")
            .args(["-c", "printf '/tmp/partial-row\\n'; exit 7"])
            .stdout(super::Stdio::piped())
            .spawn()
            .expect("spawn source process");
        let mut rows = Vec::new();
        let outcome = super::stream_mdfind_child(
            child,
            10,
            &super::new_cancel_token(),
            true,
            super::Instant::now() + super::MDFIND_TIMEOUT,
            &mut |event| {
                if let super::SearchEvent::Result(row) = event {
                    rows.push(row);
                }
            },
        );
        assert_eq!(rows.len(), 1);
        assert!(matches!(outcome, Err(super::SearchFailure::Source(_))));
    }

    #[test]
    fn source_io_error_kinds_are_not_control_outcomes() {
        for kind in [
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::BrokenPipe,
            std::io::ErrorKind::Unsupported,
        ] {
            assert!(
                matches!(super::SearchFailure::from(std::io::Error::new(kind, "source IO")), super::SearchFailure::Source(error) if error.kind() == kind)
            );
        }
        let cancel = super::new_cancel_token();
        cancel.store(true, super::Ordering::Relaxed);
        let mut events = Vec::new();
        super::search_files_streaming("needle", None, 10, cancel, true, |event| events.push(event));
        assert!(matches!(
            events.as_slice(),
            [super::SearchEvent::Done(Err(
                super::SearchFailure::Cancelled
            ))]
        ));
    }
}

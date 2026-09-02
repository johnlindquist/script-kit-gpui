use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;

use tracing::{debug, instrument, warn};

use super::mdfind::{CancelToken, SearchEvent, SearchFailure};
use super::{detect_file_type, FileResult, FileType};

/// Internal cap to prevent runaway directory listings
const MAX_DIRECTORY_ENTRIES: usize = 5000;

/// Streaming directory listing: yields results as they're read.
///
/// Similar to `search_files_streaming` but for directory contents.
/// Useful for large directories where you want progressive loading.
///
/// # Arguments
/// * `dir_path` - Directory path (can include ~, ., ..)
/// * `cancel` - Cancel token
/// * `skip_metadata` - If true, skip stat() calls (size/modified = 0)
/// * `on_event` - Callback for each result
#[instrument(skip_all, fields(dir_path = %dir_path, skip_metadata = skip_metadata))]
pub fn list_directory_streaming<F>(
    dir_path: &str,
    cancel: CancelToken,
    skip_metadata: bool,
    on_event: F,
) where
    F: FnMut(SearchEvent),
{
    list_directory_streaming_with_options(dir_path, cancel, skip_metadata, false, on_event);
}

/// Streaming directory listing with optional hidden-file visibility.
#[instrument(skip_all, fields(dir_path = %dir_path, skip_metadata = skip_metadata, show_hidden = show_hidden))]
pub fn list_directory_streaming_with_options<F>(
    dir_path: &str,
    cancel: CancelToken,
    skip_metadata: bool,
    show_hidden: bool,
    on_event: F,
) where
    F: FnMut(SearchEvent),
{
    list_directory_streaming_impl(
        dir_path,
        cancel,
        skip_metadata,
        show_hidden,
        |_| Ok(()),
        on_event,
    );
}

/// Use the native listing lifecycle with a path authority checked before IO.
#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub fn list_directory_streaming_with_path_guard<F, G>(
    dir_path: &str,
    cancel: CancelToken,
    skip_metadata: bool,
    show_hidden: bool,
    check_path: G,
    on_event: F,
) where
    F: FnMut(SearchEvent),
    G: Fn(&Path) -> std::io::Result<()>,
{
    list_directory_streaming_impl(
        dir_path,
        cancel,
        skip_metadata,
        show_hidden,
        check_path,
        on_event,
    );
}

fn list_directory_streaming_impl<F, G>(
    dir_path: &str,
    cancel: CancelToken,
    skip_metadata: bool,
    show_hidden: bool,
    check_path: G,
    mut on_event: F,
) where
    F: FnMut(SearchEvent),
    G: Fn(&Path) -> std::io::Result<()>,
{
    if cancel.load(Ordering::Relaxed) {
        on_event(SearchEvent::Done(Err(SearchFailure::Cancelled)));
        return;
    }
    // Expand the path
    let expanded = match expand_path(dir_path) {
        Some(p) => p,
        None => {
            debug!("Failed to expand path: {}", dir_path);
            on_event(SearchEvent::Done(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("cannot expand directory path {dir_path}"),
            )
            .into())));
            return;
        }
    };

    let path = Path::new(&expanded);
    if let Err(error) = check_path(path) {
        on_event(SearchEvent::Done(Err(error.into())));
        return;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(error = %e, "Failed to read directory: {}", expanded);
            on_event(SearchEvent::Done(Err(e.into())));
            return;
        }
    };

    let mut count = 0usize;

    for entry in entries {
        // Check cancellation
        if cancel.load(Ordering::Relaxed) {
            debug!("Directory listing cancelled");
            on_event(SearchEvent::Done(Err(SearchFailure::Cancelled)));
            return;
        }

        // Internal cap
        if count >= MAX_DIRECTORY_ENTRIES {
            debug!("Hit internal cap {}", MAX_DIRECTORY_ENTRIES);
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                on_event(SearchEvent::Done(Err(error.into())));
                return;
            }
        };

        let entry_path = entry.path();
        let path_str = match entry_path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        let name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Skip hidden files unless the query explicitly opted into them.
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        if let Err(error) = check_path(&entry_path) {
            on_event(SearchEvent::Done(Err(error.into())));
            return;
        }

        let (size, modified) = if skip_metadata {
            (0, 0)
        } else {
            std::fs::metadata(&entry_path)
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

        let file_type = detect_file_type(&entry_path);

        on_event(SearchEvent::Result(FileResult {
            path: path_str,
            name,
            size,
            modified,
            file_type,
        }));
        count += 1;
    }

    debug!(result_count = count, "Directory listing completed");
    on_event(SearchEvent::Done(if cancel.load(Ordering::Relaxed) {
        Err(SearchFailure::Cancelled)
    } else {
        Ok(())
    }));
}

/// Ensure a path string ends with a trailing slash
///
/// Used to normalize directory paths for consistent display and navigation.
///
/// # Examples
/// - `/foo/bar` → `/foo/bar/`
/// - `~/dev/` → `~/dev/` (unchanged)
/// - `` → `/` (empty becomes root)
/// - `~` → `~/`
#[allow(dead_code)]
pub fn ensure_trailing_slash(path: &str) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{}/", path)
    }
}

/// Get the parent directory path for Shift+Tab navigation
///
/// This is a pure string operation for display paths. It handles:
/// - Tilde paths (`~/foo/` → `~/`)
/// - Absolute paths (`/foo/bar/` → `/foo/`)
/// - Relative paths (`./` → `../`, `../` → `../../`)
///
/// Returns `None` for root paths that have no parent:
/// - `/` (filesystem root)
/// - `~/` (home directory root)
///
/// # Arguments
/// * `dir_with_slash` - Directory path (ideally ending with `/`, but handles without)
///
/// # Returns
/// * `Some(parent_path)` - Parent directory path ending with `/`
/// * `None` - If this is a root path with no parent
#[allow(dead_code)]
pub fn parent_dir_display(dir_with_slash: &str) -> Option<String> {
    // Normalize: ensure we're working with a trailing-slash path
    let normalized = if dir_with_slash.ends_with('/') {
        dir_with_slash.to_string()
    } else {
        format!("{}/", dir_with_slash)
    };

    // Handle root cases that have no parent
    if normalized == "/" || normalized == "~/" {
        return None;
    }

    // Handle relative paths specially
    if normalized == "./" {
        // Current dir -> parent dir
        return Some("../".to_string());
    }

    if normalized == "../" {
        // One level up -> two levels up
        return Some("../../".to_string());
    }

    // Handle ../ chains: ../../ -> ../../../
    if normalized.starts_with("../") {
        // Count existing ../ segments and add one more
        return Some(format!("../{}", normalized));
    }

    // For regular paths (absolute or tilde), find the parent by removing last segment
    // e.g., "/foo/bar/" -> "/foo/", "~/dev/kit/" -> "~/dev/"

    // Remove trailing slash for easier processing
    let without_trailing = normalized.trim_end_matches('/');

    // Find the last slash (which separates parent from current dir)
    if let Some(last_slash_pos) = without_trailing.rfind('/') {
        // Special case: tilde prefix
        if without_trailing.starts_with("~/") {
            if last_slash_pos == 1 {
                // "~/foo" -> last_slash at 1 -> parent is "~/"
                return Some("~/".to_string());
            }
            // "~/foo/bar" -> parent is "~/foo/"
            return Some(format!("{}/", &without_trailing[..last_slash_pos]));
        }

        // Absolute path case
        if last_slash_pos == 0 {
            // "/foo" -> parent is "/"
            return Some("/".to_string());
        }

        // General case: "/foo/bar" -> "/foo/"
        return Some(format!("{}/", &without_trailing[..last_slash_pos]));
    }

    // No slash found - shouldn't happen for valid directory paths
    None
}

pub fn shorten_home_prefix_for_display_with_home(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_string();
    }

    if path == home {
        return "~".to_string();
    }

    let home_with_slash = ensure_trailing_slash(home);
    if let Some(stripped) = path.strip_prefix(&home_with_slash) {
        return format!("~/{}", stripped);
    }

    path.to_string()
}

/// Shorten a path for display by using ~ for the home directory.
#[allow(dead_code)]
pub fn shorten_path(path: &str) -> String {
    dirs::home_dir()
        .and_then(|home| {
            home.to_str()
                .map(|home_str| shorten_home_prefix_for_display_with_home(path, home_str))
        })
        .unwrap_or_else(|| path.to_string())
}

/// Expand a path string, replacing ~ with the home directory
/// and resolving relative paths (., ..)
///
/// # Arguments
/// * `path` - Path string that may contain ~, ., or ..
///
/// # Returns
/// Expanded absolute path as a String, or None if expansion fails
pub fn expand_path(path: &str) -> Option<String> {
    let trimmed = path.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Handle home directory expansion
    if trimmed == "~" {
        return dirs::home_dir().and_then(|p| p.to_str().map(|s| s.to_string()));
    }

    if let Some(rest) = trimmed.strip_prefix("~/") {
        return dirs::home_dir().and_then(|home| home.join(rest).to_str().map(|s| s.to_string()));
    }

    // Handle relative paths
    if trimmed == "." || trimmed.starts_with("./") {
        let cwd = std::env::current_dir().ok()?;
        let suffix = trimmed.strip_prefix("./").unwrap_or("");
        if suffix.is_empty() {
            return cwd.to_str().map(|s| s.to_string());
        }
        return cwd.join(suffix).to_str().map(|s| s.to_string());
    }

    if trimmed == ".." || trimmed.starts_with("../") {
        let cwd = std::env::current_dir().ok()?;
        let parent = cwd.parent()?;
        let suffix = trimmed.strip_prefix("../").unwrap_or("");
        if suffix.is_empty() {
            return parent.to_str().map(|s| s.to_string());
        }
        return parent.join(suffix).to_str().map(|s| s.to_string());
    }

    // Already an absolute path
    if trimmed.starts_with('/') {
        return Some(trimmed.to_string());
    }

    // Not a recognized path format
    None
}

/// List contents of a directory
///
/// Returns files and directories sorted with directories first, then by name.
/// Handles ~ expansion and relative paths.
///
/// # Arguments
/// * `dir_path` - Directory path (can include ~, ., ..)
/// * `limit` - Maximum number of results to return (clamped to internal cap)
///
/// # Returns
/// Directory contents, or the actual path/read failure without a partial listing.
#[instrument(skip_all, fields(dir_path = %dir_path, limit = limit))]
pub fn list_directory(dir_path: &str, limit: usize) -> std::io::Result<Vec<FileResult>> {
    list_directory_with_options(dir_path, limit, false)
}

/// List contents of a directory with optional hidden-file visibility.
#[instrument(skip_all, fields(dir_path = %dir_path, limit = limit, show_hidden = show_hidden))]
pub fn list_directory_with_options(
    dir_path: &str,
    limit: usize,
    show_hidden: bool,
) -> std::io::Result<Vec<FileResult>> {
    debug!("Starting directory listing");

    let effective_limit = limit.min(MAX_DIRECTORY_ENTRIES);
    if effective_limit == 0 {
        debug!("Directory listing short-circuited because limit is 0");
        return Ok(Vec::new());
    }

    // Expand the path
    let expanded = match expand_path(dir_path) {
        Some(p) => p,
        None => {
            debug!("Failed to expand path: {}", dir_path);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("cannot expand directory path {dir_path}"),
            ));
        }
    };

    let path = Path::new(&expanded);

    // Read directory contents
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            warn!(error = %e, "Failed to read directory: {}", expanded);
            return Err(e);
        }
    };

    let mut results: Vec<FileResult> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let path_str = match entry_path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        let name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Skip hidden files unless the query explicitly opted into them.
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        // Get metadata
        let (size, modified) = match std::fs::metadata(&entry_path) {
            Ok(meta) => {
                let size = meta.len();
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                (size, modified)
            }
            Err(_) => (0, 0),
        };

        let file_type = detect_file_type(&entry_path);

        results.push(FileResult {
            path: path_str,
            name,
            size,
            modified,
            file_type,
        });
    }

    // Sort: directories first, then alphabetically by name
    results.sort_by(|a, b| {
        let a_is_dir = matches!(a.file_type, FileType::Directory);
        let b_is_dir = matches!(b.file_type, FileType::Directory);

        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    results.truncate(effective_limit);

    debug!(result_count = results.len(), "Directory listing completed");
    Ok(results)
}

/// Result of parsing a directory path with potential filter
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDirPath {
    /// The directory to list (always ends with / after expansion)
    pub directory: String,
    /// Optional filter pattern (the part after the last /)
    pub filter: Option<String>,
    /// Whether this query should include hidden entries in the directory listing.
    pub show_hidden: bool,
}

/// Parse a directory path into its directory component and optional filter
///
/// This handles paths like:
/// - `~/dev/` -> directory=`~/dev/`, filter=None (list all)
/// - `~/dev/fin` -> directory=`~/dev/`, filter=Some("fin") (filter by "fin")
/// - `~/dev/mcp-` -> directory=`~/dev/`, filter=Some("mcp-") (filter by "mcp-")
/// - `/usr/local/bin` -> directory=`/usr/local/`, filter=Some("bin")
/// - `~` -> directory=`~`, filter=None
///
/// Returns None if:
/// - The path doesn't look like a directory path
/// - The parent directory doesn't exist
#[instrument(skip_all, fields(path = %path))]
pub fn parse_directory_path(path: &str) -> Option<ParsedDirPath> {
    let trimmed = path.trim();
    let show_hidden_for_path = path_requests_hidden_entries(trimmed, None);

    // Must be a directory-style path
    if !crate::scripts::input_detection::is_directory_path(trimmed) {
        return None;
    }

    // Parsing performs metadata reads below. Owned evaluation must admit the
    // path before even checking existence, not only before directory listing.
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        let expanded = expand_path(trimmed)?;
        policy.require_owned_path(Path::new(&expanded)).ok()?;
    }

    // Normalize home root so all callers compare the same string.
    if trimmed == "~" || trimmed == "~/" {
        return Some(ParsedDirPath {
            directory: "~/".to_string(),
            filter: None,
            show_hidden: show_hidden_for_path,
        });
    }

    // Handle paths ending with / - they're complete directory paths
    if trimmed.ends_with('/') {
        // Bare disk root ("/" or repeated slashes) trims to an empty string,
        // which would fail `expand_path`. The root is always a valid directory,
        // so recognize it explicitly (e.g. Backspace from "~/" lands here).
        let without_trailing = trimmed.trim_end_matches('/');
        if without_trailing.is_empty() {
            return Some(ParsedDirPath {
                directory: "/".to_string(),
                filter: None,
                show_hidden: path_requests_hidden_entries(trimmed, None),
            });
        }
        // Verify the directory exists
        if let Some(expanded) = expand_path(without_trailing) {
            let p = Path::new(&expanded);
            if p.is_dir() {
                return Some(ParsedDirPath {
                    directory: trimmed.to_string(),
                    filter: None,
                    show_hidden: path_requests_hidden_entries(trimmed, None),
                });
            }
        }
        return None;
    }

    // Path doesn't end with / - split into parent dir and potential filter
    // e.g., ~/dev/fin -> ~/dev/ + fin
    if let Some(last_slash_idx) = trimmed.rfind('/') {
        let parent = &trimmed[..=last_slash_idx]; // Include the slash
        let potential_filter = &trimmed[last_slash_idx + 1..];

        // Verify parent directory exists
        let parent_to_check = if parent == "/" {
            "/"
        } else {
            parent.trim_end_matches('/')
        };

        if let Some(expanded) = expand_path(parent_to_check) {
            let p = Path::new(&expanded);
            if p.is_dir() {
                let filter = if potential_filter.is_empty() {
                    None
                } else {
                    Some(potential_filter.to_string())
                };
                let show_hidden = path_requests_hidden_entries(trimmed, filter.as_deref());
                return Some(ParsedDirPath {
                    directory: parent.to_string(),
                    filter,
                    show_hidden,
                });
            }
        }
    }

    None
}

fn path_requests_hidden_entries(path: &str, filter: Option<&str>) -> bool {
    filter.is_some_and(|value| value.starts_with('.')) || path_contains_hidden_component(path)
}

fn path_contains_hidden_component(path: &str) -> bool {
    path.split('/').any(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && segment.starts_with('.')
    })
}

/// List directory contents with optional filter applied
///
/// This combines directory listing with instant filtering for responsive UX.
/// When the user types `~/dev/fin`, we list `~/dev/` and filter by "fin".
///
/// # Arguments
/// * `dir_path` - Directory path (can include ~, ., ..)
/// * `filter` - Optional filter string to match against filenames
/// * `limit` - Maximum number of results to return
///
/// # Returns
/// Matching directory contents, or the original directory source failure.
#[allow(dead_code)]
#[instrument(skip_all, fields(dir_path = %dir_path, filter = ?filter, limit = limit))]
pub fn list_directory_filtered(
    dir_path: &str,
    filter: Option<&str>,
    limit: usize,
) -> std::io::Result<Vec<FileResult>> {
    // First get additional entries so filtering can still return enough matches.
    let show_hidden = path_requests_hidden_entries(dir_path, filter);
    let mut results = list_directory_with_options(dir_path, limit.saturating_mul(2), show_hidden)?;

    // Apply filter if present
    if let Some(filter_str) = filter {
        let filter_lower = filter_str.to_lowercase();
        results.retain(|r| r.name.to_lowercase().contains(&filter_lower));
    }

    // Apply limit after filtering
    results.truncate(limit);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(directory: &str, filter: Option<&str>, show_hidden: bool) -> Option<ParsedDirPath> {
        Some(ParsedDirPath {
            directory: directory.to_string(),
            filter: filter.map(|value| value.to_string()),
            show_hidden,
        })
    }

    #[test]
    fn directory_stream_reports_missing_source_once() {
        let temp = tempfile::tempdir().expect("temp directory");
        let missing = temp.path().join("missing");
        let mut events = Vec::new();
        list_directory_streaming_with_options(
            missing.to_str().expect("UTF-8 path"),
            super::super::new_cancel_token(),
            true,
            false,
            |event| events.push(event),
        );
        assert!(
            matches!(events.as_slice(), [SearchEvent::Done(Err(SearchFailure::Source(error)))] if error.kind() == std::io::ErrorKind::NotFound)
        );
    }

    #[test]
    fn directory_stream_cancellation_after_last_row_is_not_success() {
        let temp = tempfile::tempdir().expect("temp directory");
        std::fs::write(temp.path().join("one.txt"), "one").expect("write file");
        let cancel = super::super::new_cancel_token();
        let mut events = Vec::new();
        list_directory_streaming_with_options(
            temp.path().to_str().expect("UTF-8 path"),
            cancel.clone(),
            true,
            false,
            |event| {
                if matches!(event, SearchEvent::Result(_)) {
                    cancel.store(true, Ordering::Relaxed);
                }
                events.push(event);
            },
        );
        assert!(matches!(
            events.as_slice(),
            [
                SearchEvent::Result(_),
                SearchEvent::Done(Err(SearchFailure::Cancelled))
            ]
        ));
    }

    #[test]
    fn guarded_directory_stream_refuses_before_directory_existence_read() {
        let temp = tempfile::tempdir().expect("temp directory");
        let missing = temp.path().join("not-admitted");
        let mut events = Vec::new();
        list_directory_streaming_with_path_guard(
            missing.to_str().expect("UTF-8 path"),
            super::super::new_cancel_token(),
            false,
            false,
            |_| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            |event| events.push(event),
        );
        assert!(matches!(
            events.as_slice(),
            [SearchEvent::Done(Err(SearchFailure::Source(error)))]
                if error.kind() == std::io::ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn guarded_directory_stream_does_not_publish_unadmitted_children() {
        let temp = tempfile::tempdir().expect("temp directory");
        std::fs::write(temp.path().join("private.txt"), "private").expect("write file");
        let mut events = Vec::new();
        list_directory_streaming_with_path_guard(
            temp.path().to_str().expect("UTF-8 path"),
            super::super::new_cancel_token(),
            false,
            false,
            |path| {
                if path == temp.path() {
                    Ok(())
                } else {
                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                }
            },
            |event| events.push(event),
        );
        assert!(matches!(
            events.as_slice(),
            [SearchEvent::Done(Err(SearchFailure::Source(error)))]
                if error.kind() == std::io::ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn ensure_trailing_slash_turns_empty_input_into_root() {
        assert_eq!(ensure_trailing_slash(""), "/");
    }

    #[test]
    fn ensure_trailing_slash_preserves_existing_slashes() {
        assert_eq!(ensure_trailing_slash("/"), "/");
        assert_eq!(ensure_trailing_slash("/foo/bar/"), "/foo/bar/");
        assert_eq!(ensure_trailing_slash("~/dev/"), "~/dev/");
    }

    #[test]
    fn ensure_trailing_slash_appends_missing_slash() {
        assert_eq!(ensure_trailing_slash("/foo/bar"), "/foo/bar/");
        assert_eq!(ensure_trailing_slash("~/dev"), "~/dev/");
        assert_eq!(ensure_trailing_slash("~"), "~/");
        assert_eq!(ensure_trailing_slash("."), "./");
        assert_eq!(ensure_trailing_slash(".."), "../");
    }

    #[test]
    fn parent_dir_display_returns_none_for_display_roots() {
        assert_eq!(parent_dir_display(""), None);
        assert_eq!(parent_dir_display("/"), None);
        assert_eq!(parent_dir_display("~"), None);
        assert_eq!(parent_dir_display("~/"), None);
    }

    #[test]
    fn parent_dir_display_handles_absolute_paths() {
        assert_eq!(parent_dir_display("/foo/"), Some("/".to_string()));
        assert_eq!(parent_dir_display("/foo/bar/"), Some("/foo/".to_string()));
        assert_eq!(parent_dir_display("/foo/bar"), Some("/foo/".to_string()));
    }

    #[test]
    fn parent_dir_display_handles_tilde_paths() {
        assert_eq!(parent_dir_display("~/dev/"), Some("~/".to_string()));
        assert_eq!(parent_dir_display("~/dev"), Some("~/".to_string()));
        assert_eq!(
            parent_dir_display("~/dev/script-kit/"),
            Some("~/dev/".to_string())
        );
    }

    #[test]
    fn parent_dir_display_handles_relative_paths() {
        assert_eq!(parent_dir_display("./"), Some("../".to_string()));
        assert_eq!(parent_dir_display("../"), Some("../../".to_string()));
        assert_eq!(parent_dir_display("../../"), Some("../../../".to_string()));
    }

    #[test]
    fn shorten_home_prefix_handles_empty_home() {
        assert_eq!(
            shorten_home_prefix_for_display_with_home("/Users/alice/dev", ""),
            "/Users/alice/dev"
        );
    }

    #[test]
    fn shorten_home_prefix_handles_exact_home() {
        assert_eq!(
            shorten_home_prefix_for_display_with_home("/Users/alice", "/Users/alice"),
            "~"
        );
    }

    #[test]
    fn shorten_home_prefix_shortens_nested_paths() {
        assert_eq!(
            shorten_home_prefix_for_display_with_home(
                "/Users/alice/dev/script-kit-gpui",
                "/Users/alice"
            ),
            "~/dev/script-kit-gpui"
        );
        assert_eq!(
            shorten_home_prefix_for_display_with_home("/Users/alice/dev", "/Users/alice/"),
            "~/dev"
        );
    }

    #[test]
    fn shorten_home_prefix_respects_path_boundaries() {
        assert_eq!(
            shorten_home_prefix_for_display_with_home("/Users/alice-dev/file.txt", "/Users/alice"),
            "/Users/alice-dev/file.txt"
        );
        assert_eq!(
            shorten_home_prefix_for_display_with_home("/Users/alice2/dev", "/Users/alice"),
            "/Users/alice2/dev"
        );
        assert_eq!(
            shorten_home_prefix_for_display_with_home("", "/Users/alice"),
            ""
        );
    }

    #[test]
    fn expand_path_rejects_empty_and_unrecognized_inputs() {
        assert_eq!(expand_path(""), None);
        assert_eq!(expand_path(" "), None);
        assert_eq!(expand_path("notes"), None);
        assert_eq!(expand_path("notes/search"), None);
        assert_eq!(expand_path("~other"), None);
    }

    #[test]
    fn expand_path_returns_trimmed_absolute_paths() {
        assert_eq!(expand_path("/"), Some("/".to_string()));
        assert_eq!(expand_path("/usr/local"), Some("/usr/local".to_string()));
        assert_eq!(
            expand_path(" /Users/alice "),
            Some("/Users/alice".to_string())
        );
    }

    #[test]
    fn expand_path_expands_home_prefix_when_available() {
        match dirs::home_dir() {
            Some(home) => {
                let Some(home_str) = home.to_str() else {
                    return;
                };
                assert_eq!(expand_path("~"), Some(home_str.to_string()));
            }
            None => {
                assert_eq!(expand_path("~"), None);
            }
        }
    }

    #[test]
    fn expand_path_resolves_current_relative_paths() {
        let Some(cwd) = std::env::current_dir().ok() else {
            return;
        };
        let Some(cwd_str) = cwd.to_str() else {
            return;
        };
        assert_eq!(expand_path("."), Some(cwd_str.to_string()));
        let expected_src = cwd.join("src").to_str().map(|value| value.to_string());
        assert_eq!(expand_path("./src"), expected_src);
    }

    #[test]
    fn expand_path_resolves_parent_relative_paths() {
        let Some(cwd) = std::env::current_dir().ok() else {
            return;
        };
        let Some(parent) = cwd.parent() else {
            return;
        };
        let Some(parent_str) = parent.to_str() else {
            return;
        };
        assert_eq!(expand_path(".."), Some(parent_str.to_string()));
        let expected_src = parent.join("src").to_str().map(|value| value.to_string());
        assert_eq!(expand_path("../src"), expected_src);
    }

    #[test]
    fn parse_directory_path_rejects_empty_and_plain_search_terms() {
        assert_eq!(parse_directory_path(""), None);
        assert_eq!(parse_directory_path(" "), None);
        assert_eq!(parse_directory_path("notes"), None);
        assert_eq!(parse_directory_path("plain search"), None);
    }

    #[test]
    fn parse_directory_path_normalizes_home_root() {
        assert_eq!(parse_directory_path("~"), parsed("~/", None, false));
        assert_eq!(parse_directory_path("~/"), parsed("~/", None, false));
        assert_eq!(parse_directory_path(" ~/ "), parsed("~/", None, false));
    }

    #[test]
    fn parse_directory_path_recognizes_bare_disk_root() {
        // Backspace from "~/" in the cwd picker lands on "/". The disk root is
        // a real directory and must parse so it seeds synchronously instead of
        // falling through to the Spotlight search path.
        assert_eq!(parse_directory_path("/"), parsed("/", None, false));
        assert_eq!(parse_directory_path(" / "), parsed("/", None, false));
    }

    #[test]
    fn parse_directory_path_parses_home_filters_when_home_exists() {
        if dirs::home_dir().filter(|path| path.is_dir()).is_some() {
            assert_eq!(
                parse_directory_path("~/Documents"),
                parsed("~/", Some("Documents"), false)
            );
        }
    }

    #[test]
    fn parse_directory_path_parses_relative_directory_and_filters() {
        assert_eq!(parse_directory_path("./"), parsed("./", None, false));
        assert_eq!(
            parse_directory_path("./src"),
            parsed("./", Some("src"), false)
        );
        assert_eq!(
            parse_directory_path(" ./src "),
            parsed("./", Some("src"), false)
        );
    }

    #[test]
    fn parse_directory_path_parses_absolute_filters_with_root_parent() {
        assert_eq!(
            parse_directory_path("/script-kit-filter"),
            parsed("/", Some("script-kit-filter"), false)
        );
    }

    #[test]
    fn parse_directory_path_marks_hidden_filters_as_show_hidden() {
        assert_eq!(
            parse_directory_path("./.env"),
            parsed("./", Some(".env"), true)
        );
        assert_eq!(
            parse_directory_path("/.script-kit-hidden-filter"),
            parsed("/", Some(".script-kit-hidden-filter"), true)
        );
        if dirs::home_dir().filter(|path| path.is_dir()).is_some() {
            assert_eq!(
                parse_directory_path("~/.config"),
                parsed("~/", Some(".config"), true)
            );
        }
    }

    #[test]
    fn parse_directory_path_returns_none_for_missing_complete_directory() {
        assert_eq!(
            parse_directory_path("/script-kit-gpui-definitely-missing-directory-zzzz/"),
            None
        );
    }
}

//! File Search Module using macOS Spotlight (mdfind)
//!
//! This module provides file search functionality using macOS's mdfind command,
//! which interfaces with the Spotlight index for fast file searching.
//!
//! # Streaming API
//!
//! For real-time search UX, use `search_files_streaming()` with a cancel token.
//! This allows:
//! - Cancellation of in-flight searches when query changes
//! - Batched UI updates without blocking on full results
//! - Proper cleanup of mdfind processes
//!
//! # Performance Notes
//!
//! - Metadata (size, modified) is fetched per-result which can be slow
//! - For faster "time to first result", consider skipping metadata in streaming mode
//!   and hydrating it lazily for visible rows only

// --- merged from part_000.rs ---
use std::path::Path;
use std::time::UNIX_EPOCH;
use tracing::{debug, instrument};
/// File type classification based on extension
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileType {
    File,
    Directory,
    Application,
    Image,
    Document,
    Audio,
    Video,
    #[default]
    Other,
}
/// Information about a file for the actions dialog
/// Used as context for file-specific actions (similar to PathInfo and ScriptInfo)
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Full path to the file
    pub path: String,
    /// File name (last component of path)
    pub name: String,
    /// Type of file (used by the actions builder for context-specific actions)
    #[allow(dead_code)]
    pub file_type: FileType,
    /// Whether this is a directory
    pub is_dir: bool,
}
impl FileInfo {
    /// Create FileInfo from a FileResult
    pub fn from_result(result: &FileResult) -> Self {
        FileInfo {
            path: result.path.clone(),
            name: result.name.clone(),
            file_type: result.file_type,
            is_dir: result.file_type == FileType::Directory,
        }
    }

    /// Create FileInfo from path string
    #[allow(dead_code)]
    pub fn from_path(path: &str) -> Self {
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let is_dir = path_obj.is_dir();
        let file_type = if is_dir {
            FileType::Directory
        } else {
            FileType::File
        };

        FileInfo {
            path: path.to_string(),
            name,
            file_type,
            is_dir,
        }
    }
}
/// Result of a file search
#[derive(Debug, Clone)]
pub struct FileResult {
    /// Full path to the file
    pub path: String,
    /// File name (last component of path)
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// Last modified time as Unix timestamp
    pub modified: u64,
    /// Type of file
    pub file_type: FileType,
}
/// Metadata for a single file
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FileMetadata {
    /// Full path to the file
    pub path: String,
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// Last modified time as Unix timestamp
    pub modified: u64,
    /// Type of file
    pub file_type: FileType,
    /// Whether the file is readable
    pub readable: bool,
    /// Whether the file is writable
    pub writable: bool,
}
/// Default limit for UI display (final visible results after filtering)
#[allow(dead_code)]
pub const DEFAULT_LIMIT: usize = 50;
/// Limit for interactive mdfind searches
/// Smaller than directory listing because each result requires a stat() call
/// 500 results is plenty for fuzzy filtering and keeps response time <1s
pub const DEFAULT_SEARCH_LIMIT: usize = 500;
/// Default cache limit for directory listing (fast operation, can handle more)
/// Directory listing is cheaper than mdfind search (single readdir vs many stat calls)
pub const DEFAULT_CACHE_LIMIT: usize = 2000;
/// Maximum Spotlight results collected for root launcher file rows.
pub const ROOT_FILE_SOURCE_LIMIT: usize = 24;
/// Maximum root launcher file rows rendered under the Files section.
pub const ROOT_FILE_RENDER_LIMIT: usize = 6;
/// Maximum frecency-backed recent file rows rendered on the empty root launcher.
pub const ROOT_FILE_RECENT_RENDER_LIMIT: usize = ROOT_FILE_RENDER_LIMIT;
/// Maximum frecency-backed recent file rows cached for non-empty root seeds.
pub const ROOT_FILE_RECENT_SEED_LIMIT: usize = ROOT_FILE_RENDER_LIMIT * 4;
/// Maximum frecency paths to hydrate while refreshing root recent files.
pub const ROOT_FILE_RECENT_HYDRATE_LIMIT: usize = ROOT_FILE_RECENT_SEED_LIMIT * 3;
/// Initial visible rows for explicit root Files source-chip searches.
pub const ROOT_FILE_SOURCE_CHIP_INITIAL_VISIBLE_ROWS: usize = ROOT_FILE_RENDER_LIMIT * 2;
/// Additional rows revealed when explicit root Files source-chip searches page.
pub const ROOT_FILE_SOURCE_CHIP_PAGE_SIZE: usize = ROOT_FILE_RENDER_LIMIT * 2;
/// Maximum directory children collected for root launcher directory browsing.
pub const ROOT_FILE_BROWSE_SOURCE_LIMIT: usize = 96;
/// Maximum directory children rendered for root launcher directory browsing.
pub const ROOT_FILE_BROWSE_RENDER_LIMIT: usize = 12;
/// Minimum visible query length before root launcher file search starts.
pub const ROOT_FILE_MIN_QUERY_CHARS: usize = 3;

/// Spotlight query for files the user actually opened recently (Finder
/// "Recents" semantics: last-used within 30 days, folders excluded). Seeds
/// the empty `@file:` subsearch so it shows real recents instead of only
/// frecency picks made through Script Kit itself.
pub const RECENTLY_USED_FILES_MDQUERY: &str =
    r#"kMDItemLastUsedDate >= $time.now(-2592000) && kMDItemContentTypeTree != "public.folder""#;

/// Source-collection cap for the recently-used seed (sorted by modified
/// time, then truncated to `ROOT_FILE_RECENT_SEED_LIMIT` for display).
pub const RECENTLY_USED_FILES_SOURCE_LIMIT: usize = 96;

/// Drop noisy recently-used Spotlight hits: app bundles, anything under a
/// `Library/` tree (except iCloud Drive's `Mobile Documents`), and files
/// inside hidden (dot) directories.
pub fn is_noisy_recent_file_path(path: &str) -> bool {
    if path.ends_with(".app") || path.contains(".app/") {
        return true;
    }
    if path.contains("/Library/") && !path.contains("/Library/Mobile Documents/") {
        return true;
    }
    path.split('/')
        .any(|component| component.len() > 1 && component.starts_with('.'))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RootFilePromotionPolicy {
    #[default]
    Never,
    ExactFilenameOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootFileSectionOptions {
    pub files_enabled: bool,
    pub recent_files_enabled: bool,
    pub global_search_enabled: bool,
    pub directory_browse_enabled: bool,
    pub promotion_policy: RootFilePromotionPolicy,
    pub query_intent: RootFileQueryIntent,
    pub source_filter_browse_target_visible_rows: Option<usize>,
    pub source_chip_visible_limit: Option<usize>,
}

impl Default for RootFileSectionOptions {
    fn default() -> Self {
        Self {
            files_enabled: true,
            recent_files_enabled: true,
            global_search_enabled: true,
            directory_browse_enabled: true,
            promotion_policy: RootFilePromotionPolicy::Never,
            query_intent: RootFileQueryIntent::OrdinaryRoot,
            source_filter_browse_target_visible_rows: None,
            source_chip_visible_limit: None,
        }
    }
}

/// Deterministic display model for root-launcher inline Files previews.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootFileInlineMatchMode {
    /// A single eligible filename term; keep the compact historical label.
    SingleTerm,
    /// Multi-term query that remains phrase-style because at least one token is short.
    Phrase,
    /// Multi-term query that can use filename word matching in the provider.
    FilenameWords,
    /// Directory path syntax browsing a folder.
    Directory,
}

impl RootFileInlineMatchMode {
    pub fn section_label(self) -> &'static str {
        match self {
            Self::SingleTerm => "Files",
            Self::Phrase => "Files · Phrase match",
            Self::FilenameWords => "Files · Word match",
            Self::Directory => "Files · Folder",
        }
    }

    pub fn handoff_subtitle(self) -> &'static str {
        match self {
            Self::SingleTerm => "Open full File Search",
            Self::Phrase => "Open full File Search · preview matches typed phrase",
            Self::FilenameWords => "Open full File Search · preview matches filename words",
            Self::Directory => "Browse the full folder",
        }
    }

    pub fn receipt_name(self) -> &'static str {
        match self {
            Self::SingleTerm => "SingleTerm",
            Self::Phrase => "Phrase",
            Self::FilenameWords => "FilenameWords",
            Self::Directory => "Directory",
        }
    }
}

/// Which source currently backs the root launcher's `Files` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootFileSectionMode {
    /// Global filename search backed by Spotlight.
    GlobalQuery,
    /// Direct child listing for an explicit directory path query.
    DirectoryBrowse,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RootFileQueryIntent {
    #[default]
    OrdinaryRoot,
    ExplicitFilesSourceFilter,
}

/// Check if the query looks like an advanced mdfind query (with operators)
/// If so, pass it through directly; otherwise wrap as filename query
pub(crate) fn looks_like_advanced_mdquery(q: &str) -> bool {
    let q = q.trim();
    q.contains("kMDItem")
        || q.contains("==")
        || q.contains("!=")
        || q.contains(">=")
        || q.contains("<=")
        || q.contains("&&")
        || q.contains("||")
}
/// Escape special characters for mdfind query string literals
fn escape_md_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
/// Build an mdfind query from user input
/// - If input looks like advanced query syntax, pass through as-is
/// - Otherwise, wrap as case-insensitive filename contains query
fn build_mdquery(user_query: &str) -> String {
    let q = user_query.trim();
    if looks_like_advanced_mdquery(q) {
        return q.to_string();
    }
    let escaped = escape_md_string(q);
    format!(r#"kMDItemFSName == "*{}*"c"#, escaped)
}

/// Build the provider query used by root-launcher global file search.
///
/// Plain single-term queries keep the existing literal filename contains shape.
/// Safe multi-word and separator-token queries keep that phrase branch and add
/// an all-terms filename branch so Spotlight can recall separator-token files
/// such as `client-design-notes.md` for `design notes` or `egghead.svg`.
pub fn root_file_provider_query_for_user_query(user_query: &str) -> String {
    let q = user_query.trim();
    if looks_like_advanced_mdquery(q) {
        return q.to_string();
    }

    let query_terms = root_file_query_terms(q);
    let filename_terms = root_file_filename_query_terms(q);
    if filename_terms.len() < 2 || filename_terms.iter().any(|term| term.chars().count() < 2) {
        return q.to_string();
    }

    let phrase = escape_md_string(q);
    let filename_terms_query = filename_terms
        .iter()
        .map(|term| format!(r#"kMDItemFSName == "*{}*"c"#, escape_md_string(term)))
        .collect::<Vec<_>>()
        .join(" && ");

    let mut branches = vec![
        format!(r#"kMDItemFSName == "*{}*"c"#, phrase),
        format!("({})", filename_terms_query),
    ];
    branches.extend(root_file_path_context_mdquery_branches(&query_terms));

    format!("({})", branches.join(" || "))
}

fn root_file_filename_terms_are_safe_for_provider(terms: &[String]) -> bool {
    !terms.is_empty() && terms.iter().all(|term| term.chars().count() >= 2)
}

fn root_file_path_context_mdquery_branches(terms: &[String]) -> Vec<String> {
    if terms.len() < 2 || terms.len() > ROOT_FILE_PATH_CONTEXT_MAX_TERMS {
        return Vec::new();
    }

    let mut branches = Vec::new();
    for split in 1..terms.len() {
        let (parent_terms, filename_terms) = terms.split_at(split);
        if parent_terms
            .iter()
            .any(|term| !root_file_query_has_safe_global_length(term))
            || !root_file_filename_terms_are_safe_for_provider(filename_terms)
        {
            continue;
        }

        let mut parts = Vec::with_capacity(terms.len());
        parts.extend(
            parent_terms
                .iter()
                .map(|term| format!(r#"kMDItemPath == "*{}*"c"#, escape_md_string(term))),
        );
        parts.extend(
            filename_terms
                .iter()
                .map(|term| format!(r#"kMDItemFSName == "*{}*"c"#, escape_md_string(term))),
        );
        branches.push(format!("({})", parts.join(" && ")));
    }

    branches
}

/// Returns true when the root launcher should ask Spotlight for file rows.
pub fn root_file_global_query_is_eligible(query: &str) -> bool {
    root_file_global_query_is_eligible_for_intent(query, RootFileQueryIntent::OrdinaryRoot)
}

pub fn root_file_global_query_is_eligible_for_intent(
    query: &str,
    intent: RootFileQueryIntent,
) -> bool {
    let q = query.trim();
    root_file_query_has_safe_global_length_for_intent(q, intent)
        && !looks_like_advanced_mdquery(q)
        && !is_directory_path(q)
}

fn root_file_query_has_safe_global_length(query: &str) -> bool {
    query.chars().count() >= ROOT_FILE_MIN_QUERY_CHARS || root_file_short_digit_token_query(query)
}

fn root_file_query_has_safe_global_length_for_intent(
    query: &str,
    intent: RootFileQueryIntent,
) -> bool {
    root_file_query_has_safe_global_length(query)
        || (intent == RootFileQueryIntent::ExplicitFilesSourceFilter
            && (1..=2).contains(&query.chars().count())
            && query.chars().all(|ch| ch.is_ascii_alphanumeric()))
}

fn root_file_short_digit_token_query(query: &str) -> bool {
    query.chars().count() == 2
        && query.chars().all(|ch| ch.is_ascii_alphanumeric())
        && query.chars().any(|ch| ch.is_ascii_digit())
}

/// Returns true when the root launcher should ask Spotlight for file rows.
pub fn should_search_root_files(query: &str) -> bool {
    root_file_global_query_is_eligible(query)
}

/// Returns true when the root launcher should ask Spotlight for file rows for a known intent.
pub fn should_search_root_files_for_intent(query: &str, intent: RootFileQueryIntent) -> bool {
    root_file_global_query_is_eligible_for_intent(query, intent)
}

/// Returns true when a file row belongs in the root launcher's global Files section.
///
/// App bundles stay owned by launcher app results for global queries, while
/// directory browsing and dedicated File Search can still render them.
pub fn root_global_file_result_is_eligible(file: &FileResult) -> bool {
    file.file_type != FileType::Application
        && !path_contains_application_bundle_component(&file.path)
}

fn path_contains_application_bundle_component(path: &str) -> bool {
    Path::new(path).components().any(|component| {
        let std::path::Component::Normal(name) = component else {
            return false;
        };
        Path::new(name)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
    })
}

/// Returns true when the root launcher query is syntactically a directory browse.
///
/// This is intentionally syntax-only so grouping/ranking code can decide layout
/// without touching the filesystem. Provider code still validates existence via
/// `parse_directory_path` before collecting rows.
pub fn looks_like_root_directory_browse_query(query: &str) -> bool {
    let q = query.trim();
    !q.is_empty()
        && (q.starts_with('/')
            || q == "~"
            || q.starts_with("~/")
            || q.starts_with("./")
            || q.starts_with("../"))
        && !looks_like_advanced_mdquery(q)
}

/// Return the folder portion of a root directory-browse query without reading the filesystem.
pub fn root_directory_query_base(query: &str) -> Option<String> {
    let q = query.trim();
    if !looks_like_root_directory_browse_query(q) {
        return None;
    }
    if q == "~" || q == "~/" {
        return Some("~/".to_string());
    }
    if q.ends_with('/') {
        return Some(q.to_string());
    }
    let last_slash = q.rfind('/')?;
    Some(q[..=last_slash].to_string())
}

/// Return the provider identity for a root directory-browse query.
///
/// The visible query may include a child fragment after the final slash, but
/// the source provider is only the containing directory plus hidden-file mode.
pub fn root_directory_browse_source_key(query: &str) -> Option<(String, bool)> {
    let parsed = parse_directory_path(query)?;
    Some((parsed.directory, parsed.show_hidden))
}

/// Returns the root file section mode implied by a query's syntax.
pub fn root_file_section_mode_for_query(query: &str) -> Option<RootFileSectionMode> {
    root_file_section_mode_for_query_with_intent(query, RootFileQueryIntent::OrdinaryRoot)
}

pub fn root_file_section_mode_for_query_with_intent(
    query: &str,
    intent: RootFileQueryIntent,
) -> Option<RootFileSectionMode> {
    if should_search_root_files_for_intent(query, intent) {
        Some(RootFileSectionMode::GlobalQuery)
    } else if looks_like_root_directory_browse_query(query) {
        Some(RootFileSectionMode::DirectoryBrowse)
    } else {
        None
    }
}

/// Return the stable match-mode affordance for root-launcher inline Files previews.
pub fn root_file_inline_match_mode_for_query(
    query: &str,
    intent: RootFileQueryIntent,
) -> Option<RootFileInlineMatchMode> {
    let q = query.trim();
    if looks_like_root_directory_browse_query(q) {
        return Some(RootFileInlineMatchMode::Directory);
    }
    if looks_like_advanced_mdquery(q) || !root_file_global_query_is_eligible_for_intent(q, intent) {
        return None;
    }

    let query_terms = root_file_query_terms(q);
    let filename_terms = root_file_filename_query_terms(q);
    if filename_terms.len() == 1 {
        return Some(RootFileInlineMatchMode::SingleTerm);
    }
    if filename_terms.len() >= 2 && filename_terms.iter().any(|term| term.chars().count() < 2) {
        return Some(RootFileInlineMatchMode::Phrase);
    }
    if query_terms.len() >= 2 || filename_terms.len() >= 2 {
        return Some(RootFileInlineMatchMode::FilenameWords);
    }
    None
}
// NOTE: escape_query() was removed because:
// 1. It was unused dead code
// 2. Command::new() does NOT use a shell, so shell escaping is irrelevant
// 3. Arguments passed via .arg() are automatically handled safely

/// Detect file type based on extension
fn detect_file_type(path: &Path) -> FileType {
    // Get extension first - we need it to check for .app bundles
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // macOS .app bundles are directories but should be classified as Applications
    // Check for .app extension BEFORE checking is_dir()
    if extension.as_deref() == Some("app") {
        return FileType::Application;
    }

    // Check if it's a directory (but not an .app bundle)
    if path.is_dir() {
        return FileType::Directory;
    }

    match extension.as_deref() {
        // Applications (already handled above, but kept for completeness)
        Some("app") => FileType::Application,

        // Images
        Some(
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "heic"
            | "heif",
        ) => FileType::Image,

        // Documents
        Some(
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf" | "odt"
            | "ods" | "odp" | "pages" | "numbers" | "key",
        ) => FileType::Document,

        // Audio
        Some("mp3" | "wav" | "aac" | "flac" | "ogg" | "wma" | "m4a" | "aiff") => FileType::Audio,

        // Video
        Some("mp4" | "mov" | "avi" | "mkv" | "wmv" | "flv" | "webm" | "m4v" | "mpeg" | "mpg") => {
            FileType::Video
        }

        // Check if it's a file (has extension but not matched above)
        Some(_) => FileType::File,

        // No extension - check if it exists to determine type
        None => {
            if path.exists() {
                if path.is_dir() {
                    FileType::Directory
                } else {
                    FileType::File
                }
            } else {
                FileType::Other
            }
        }
    }
}

mod directory;
mod mdfind;
mod os_open;

pub use crate::scripts::input_detection::is_directory_path;
#[allow(unused_imports)]
pub use directory::{
    ensure_trailing_slash, expand_path, list_directory, list_directory_filtered,
    list_directory_streaming, list_directory_streaming_with_options, list_directory_with_options,
    parent_dir_display, parse_directory_path, shorten_home_prefix_for_display_with_home,
    shorten_path, ParsedDirPath,
};
pub use mdfind::{
    new_cancel_token, recent_files_filesystem, search_files, search_files_streaming,
    search_files_streaming_with_options, CancelToken, SearchEvent, SearchFilesStreamingOptions,
};
pub use os_open::{
    duplicate_path, move_path, move_to_trash, open_file, open_with, prompt_move_destination_dir,
    prompt_rename_target_name, quick_look, rename_path, reveal_in_finder, show_info,
};

pub fn parent_folder_search_query(path: &str) -> Option<String> {
    let parent = Path::new(path).parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    let parent = parent.to_str()?;
    Some(shorten_path(&ensure_trailing_slash(parent)))
}

/// Build a root-launcher file result from live metadata for a previously seen path.
///
/// Recent root files are frecency-backed, not search-backed, so this helper only
/// hydrates known paths and applies the same global root eligibility gate used
/// by non-empty root file rows.
pub fn file_result_from_existing_path(path: &str) -> Option<FileResult> {
    let metadata = get_file_metadata(path)?;
    let result = FileResult {
        path: metadata.path,
        name: metadata.name,
        size: metadata.size,
        modified: metadata.modified,
        file_type: metadata.file_type,
    };
    root_global_file_result_is_eligible(&result).then_some(result)
}

/// Convert directory browse results into root-launcher file matches.
pub fn root_directory_file_matches(
    results: &[FileResult],
    child_filter: Option<&str>,
    limit: usize,
) -> Vec<crate::scripts::FileMatch> {
    let filter = child_filter
        .map(str::trim)
        .filter(|filter| !filter.is_empty());

    let Some(filter) = filter else {
        return results
            .iter()
            .take(limit)
            .enumerate()
            .map(|(rank, file)| crate::scripts::FileMatch {
                file: file.clone(),
                score: i32::MAX.saturating_sub(rank as i32),
            })
            .collect();
    };

    let q = filter.to_lowercase();
    let mut nucleo = crate::scripts::NucleoCtx::new(&q);
    let mut ranked: Vec<_> = results
        .iter()
        .filter_map(|file| {
            let stem = Path::new(&file.name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&file.name);
            let name_score = nucleo.score(&file.name);
            let stem_score = nucleo.score(stem);
            let score = name_score.max(stem_score)?;
            let text_tier = root_file_name_relevance_tier(&file.name, &q, true);
            if text_tier < 3 {
                return None;
            }

            Some(crate::scripts::FileMatch {
                file: file.clone(),
                score: text_tier
                    .saturating_mul(ROOT_FILE_TEXT_TIER_MULTIPLIER)
                    .saturating_add(score.min(10_000) as i32),
            })
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.file.name.cmp(&b.file.name))
            .then_with(|| a.file.path.cmp(&b.file.path))
    });
    ranked.truncate(limit);
    ranked
}

const ROOT_FILE_TEXT_TIER_MULTIPLIER: i32 = 20_000;
const ROOT_FILE_PATH_CONTEXT_TIER: i32 = 3;
const ROOT_FILE_PATH_CONTEXT_MAX_TERMS: usize = 4;
const ROOT_FILE_KNOWN_EXTENSIONS: &str = "app png jpg jpeg gif bmp webp svg ico tiff heic heif pdf doc docx xls xlsx ppt pptx txt rtf odt ods odp pages numbers key mp3 wav aac flac ogg wma m4a aiff mp4 mov avi mkv wmv flv webm m4v mpeg mpg md rs json toml yaml yml js jsx ts tsx html css xml csv sh zsh py rb";

fn root_file_name_relevance_tier(name: &str, query: &str, name_matched: bool) -> i32 {
    let name_lc = name.to_lowercase();
    let stem_lc = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name)
        .to_lowercase();

    if name_lc == query || stem_lc == query {
        return 6;
    }
    if name_lc.starts_with(query) || stem_lc.starts_with(query) {
        return 5;
    }
    if root_file_name_separator_prefix_matches_query(name, query) {
        return 5;
    }
    if root_file_name_token_matches_query(name, query) {
        return 4;
    }
    if name_lc.contains(query) || stem_lc.contains(query) {
        return 3;
    }
    if name_matched {
        return 2;
    }
    1
}

/// Return true when a root file query is a high-confidence filename-token match.
pub fn root_file_name_token_matches_query(name: &str, query: &str) -> bool {
    let query_terms = root_file_query_terms(query);
    let terms = root_file_filename_query_terms(query);
    root_file_filename_terms_match_with_extension(name, &terms, terms != query_terms)
}

fn root_file_name_separator_prefix_matches_query(name: &str, query: &str) -> bool {
    let query_terms = root_file_query_terms(query);
    let terms = root_file_filename_query_terms(query);
    if terms == query_terms || terms.len() < 2 {
        return false;
    }

    let Some(extension_term) = terms
        .last()
        .filter(|term| root_file_query_term_is_known_extension(term))
    else {
        return false;
    };
    let path = Path::new(name);
    let extension_matches = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(extension_term));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);

    extension_matches
        && stem.to_lowercase().starts_with(&terms[0])
        && root_file_text_matches_terms_in_order(stem, &terms)
}

pub fn root_file_name_exact_or_stem_matches_query(name: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }

    let name_lc = name.to_lowercase();
    let stem_lc = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name)
        .to_lowercase();

    name_lc == q || stem_lc == q
}

fn root_file_filename_terms_match(name: &str, terms: &[String]) -> bool {
    root_file_filename_terms_match_with_extension(name, terms, false)
}

fn root_file_filename_terms_match_with_extension(
    name: &str,
    terms: &[String],
    extension_aware: bool,
) -> bool {
    if terms.is_empty() {
        return false;
    }

    if terms.len() == 1 {
        return root_file_name_token_matches_single_term(name, &terms[0]);
    }

    if terms.iter().any(|term| term.chars().count() < 2) {
        return false;
    }

    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);

    if let Some(extension_term) = extension_aware
        .then(|| terms.last())
        .flatten()
        .filter(|term| root_file_query_term_is_known_extension(term))
    {
        let extension_matches = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(extension_term));
        return extension_matches
            && root_file_text_matches_terms_in_order(stem, &terms[..terms.len() - 1]);
    }

    root_file_text_matches_terms_in_order(name, terms)
        || root_file_text_matches_terms_in_order(stem, terms)
}

fn root_file_path_context_matches_query(file: &FileResult, query: &str) -> bool {
    let terms = root_file_query_terms(query);
    if terms.len() < 2 || terms.len() > ROOT_FILE_PATH_CONTEXT_MAX_TERMS {
        return false;
    }

    let Some(parent_path) = Path::new(&file.path)
        .parent()
        .and_then(|parent| parent.to_str())
    else {
        return false;
    };

    for split in 1..terms.len() {
        let (parent_terms, filename_terms) = terms.split_at(split);
        if parent_terms
            .iter()
            .any(|term| !root_file_query_has_safe_global_length(term))
            || filename_terms.iter().any(|term| term.chars().count() < 2)
        {
            continue;
        }

        if root_file_text_matches_terms_in_order(parent_path, parent_terms)
            && root_file_filename_terms_match(&file.name, filename_terms)
        {
            return true;
        }
    }

    false
}

fn root_file_query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn root_file_filename_query_terms(query: &str) -> Vec<String> {
    root_file_query_terms(query)
        .into_iter()
        .flat_map(|term| {
            term.split(['.', '-', '_'])
                .map(str::trim)
                .filter(|subtoken| !subtoken.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn root_file_query_term_is_known_extension(term: &str) -> bool {
    ROOT_FILE_KNOWN_EXTENSIONS
        .split_ascii_whitespace()
        .any(|extension| extension == term)
}

fn root_file_name_token_matches_single_term(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }

    let name_lc = name.to_lowercase();
    let stem_lc = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name)
        .to_lowercase();

    name_lc == query
        || stem_lc == query
        || name_lc.starts_with(query)
        || stem_lc.starts_with(query)
        || contains_at_root_file_token_boundary(name, query)
        || contains_at_root_file_token_boundary(
            Path::new(name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(name),
            query,
        )
}

fn root_file_text_matches_terms_in_order(text: &str, terms: &[String]) -> bool {
    let text_lc = text.to_lowercase();
    let mut search_start = 0;

    for term in terms {
        let Some(idx) = find_next_root_file_token_match(text, &text_lc, term, search_start) else {
            return false;
        };
        search_start = idx.saturating_add(term.len());
    }

    true
}

fn find_next_root_file_token_match(
    text: &str,
    text_lc: &str,
    term: &str,
    search_start: usize,
) -> Option<usize> {
    text_lc
        .get(search_start..)?
        .match_indices(term)
        .map(|(offset, _)| search_start + offset)
        .find(|idx| is_root_file_token_boundary_at(text, *idx))
}

/// Return true when a root recent-file seed is a high-confidence filename-side match.
pub fn root_file_name_seed_matches_query(name: &str, query: &str) -> bool {
    root_file_name_token_matches_query(name, query)
}

/// Return true when a recent file can seed a non-empty global root Files section.
pub fn root_file_recent_seed_matches_query(file: &FileResult, query: &str) -> bool {
    root_file_recent_seed_matches_query_for_intent(file, query, RootFileQueryIntent::OrdinaryRoot)
}

pub fn root_file_recent_seed_matches_query_for_intent(
    file: &FileResult,
    query: &str,
    intent: RootFileQueryIntent,
) -> bool {
    let query = query.trim();
    if query.is_empty() || !root_file_global_query_is_eligible_for_intent(query, intent) {
        return false;
    }

    root_file_name_seed_matches_query(&file.name, query)
        || root_file_path_context_matches_query(file, query)
}

fn contains_at_root_file_token_boundary(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }

    let haystack_lc = haystack.to_lowercase();
    let needle_lc = needle.to_lowercase();
    haystack_lc
        .match_indices(&needle_lc)
        .any(|(idx, _)| is_root_file_token_boundary_at(haystack, idx))
}

fn is_root_file_token_boundary_at(haystack: &str, idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    if !haystack.is_char_boundary(idx) {
        return false;
    }

    let Some(previous) = haystack[..idx].chars().next_back() else {
        return true;
    };
    if is_root_file_boundary_char(previous) {
        return true;
    }

    let Some(current) = haystack[idx..].chars().next() else {
        return false;
    };
    if previous.is_ascii_lowercase() && current.is_ascii_uppercase() {
        return true;
    }
    if previous.is_ascii_digit() && current.is_ascii_alphabetic() {
        return true;
    }

    let next_start = idx + current.len_utf8();
    let next = haystack
        .get(next_start..)
        .and_then(|rest| rest.chars().next());
    previous.is_ascii_uppercase()
        && current.is_ascii_uppercase()
        && next.is_some_and(|ch| ch.is_ascii_lowercase())
}

fn is_root_file_boundary_char(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '-' | '_' | '.' | '/' | '(' | ')' | '[' | ']' | '{' | '}'
    )
}

/// Rank a bounded batch of Spotlight results for display in root launcher search.
pub fn rank_root_file_results(
    results: &[FileResult],
    query: &str,
    limit: usize,
    frecency_score: impl Fn(&str) -> f64,
) -> Vec<crate::scripts::FileMatch> {
    let q = query.trim().to_lowercase();
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut nucleo = crate::scripts::NucleoCtx::new(&q);
    let mut seen = std::collections::HashSet::new();
    let mut ranked: Vec<_> = results
        .iter()
        .filter(|file| seen.insert(file.path.clone()))
        .filter(|file| file.file_type != FileType::Application)
        .filter_map(|file| {
            let name_score = nucleo.score(&file.name);
            let (score, name_matched) = match name_score {
                Some(score) => (score, true),
                None => (nucleo.score(&file.path)?, false),
            };
            let text_tier = root_file_name_relevance_tier(&file.name, &q, name_matched).max(
                if root_file_path_context_matches_query(file, &q) {
                    ROOT_FILE_PATH_CONTEXT_TIER
                } else {
                    0
                },
            );
            let frecency_bonus =
                (frecency_score(&format!("file/{}", file.path)) * 100.0).min(500.0) as i32;

            Some(crate::scripts::FileMatch {
                file: file.clone(),
                score: text_tier
                    .saturating_mul(ROOT_FILE_TEXT_TIER_MULTIPLIER)
                    .saturating_add(score.min(10_000) as i32)
                    .saturating_add(frecency_bonus),
            })
        })
        .collect();

    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.file.name.cmp(&b.file.name))
            .then_with(|| a.file.path.cmp(&b.file.path))
    });
    ranked.truncate(limit);
    ranked
}

/// Payload for file drag-out from the mini explorer.
///
/// Stored as the GPUI drag value. When the drag starts, we also initiate
/// a native macOS drag session so the file can be dropped into Finder
/// or other apps.
#[derive(Clone, Debug)]
pub struct FileDragPayload {
    pub name: String,
}

impl FileDragPayload {
    pub fn from_result(result: &FileResult) -> Self {
        Self {
            name: result.name.clone(),
        }
    }
}

/// Render implementation for the drag preview overlay.
impl gpui::Render for FileDragPayload {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{div, px, rgb, rgba, ParentElement, Styled};

        let theme = crate::theme::get_cached_theme();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(rgba(chrome.popup_surface_rgba))
            .border_1()
            .border_color(rgba(chrome.border_rgba))
            .text_sm()
            .text_color(rgb(theme.colors.text.primary))
            .child(self.name.clone())
    }
}

#[cfg(test)]
pub(crate) use os_open::terminal_working_directory;
/// Get detailed metadata for a specific file
///
/// # Arguments
/// * `path` - Path to the file
///
/// # Returns
/// Some(FileMetadata) if the file exists and is readable, None otherwise
///
#[allow(dead_code)]
#[instrument(skip_all, fields(path = %path))]
pub fn get_file_metadata(path: &str) -> Option<FileMetadata> {
    debug!("Getting file metadata");

    let path_obj = Path::new(path);

    let metadata = match std::fs::metadata(path_obj) {
        Ok(m) => m,
        Err(e) => {
            debug!(error = %e, "Failed to get file metadata");
            return None;
        }
    };

    let name = path_obj
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let size = metadata.len();

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let file_type = detect_file_type(path_obj);

    // Check permissions
    let readable = path_obj.exists(); // If we got metadata, it's readable
    let writable = !metadata.permissions().readonly();

    Some(FileMetadata {
        path: path.to_string(),
        name,
        size,
        modified,
        file_type,
        readable,
        writable,
    })
}
// ============================================================================
// UI Helper Functions
// These functions are prepared for file search UI that's being implemented.
// Allow dead_code temporarily until the file search view is complete.
// ============================================================================

/// Get an emoji icon for the file type (used in file search UI)
#[allow(dead_code)]
pub fn file_type_icon(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Directory => "📁",
        FileType::Application => "📦",
        FileType::Image => "🖼️",
        FileType::Document => "📄",
        FileType::Audio => "🎵",
        FileType::Video => "🎬",
        FileType::File => "📃",
        FileType::Other => "📎",
    }
}

/// Return true when a file path supports inline thumbnail previews in file search rows.
///
/// This intentionally matches the product requirement for thumbnail-capable image
/// extensions in the list UI.
#[allow(dead_code)]
pub fn is_thumbnail_preview_supported(path: &str) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);

    matches!(
        extension.as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff")
    )
}
/// Format file size in human-readable format (e.g., "1.2 MB", "456 KB")
#[allow(dead_code)]
pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
/// Format Unix timestamp for file-search modified metadata using Finder-style labels.
#[allow(dead_code)]
pub fn format_relative_time(unix_timestamp: u64) -> String {
    crate::formatting::format_finder_modified_timestamp(unix_timestamp)
}
/// Filter and sort FileResults using Nucleo fuzzy matching
///
/// This function filters cached file results by fuzzy-matching the filter pattern
/// against file names, then sorts by match score (higher = better match).
///
/// # Arguments
/// * `results` - Slice of FileResult to filter
/// * `filter_pattern` - The pattern to fuzzy-match against file names
///
/// # Returns
/// Vector of (original_index, FileResult, score) tuples, sorted by score descending
#[allow(dead_code)]
pub fn filter_results_with_nucleo(
    results: &[FileResult],
    filter_pattern: &str,
) -> Vec<(usize, FileResult, u32)> {
    rank_file_results_nucleo(results, filter_pattern)
        .into_iter()
        .map(|(idx, score)| (idx, results[idx].clone(), score))
        .collect()
}
/// Core nucleo ranking helper returning only (index, score).
///
/// This keeps sorting/ranking allocations minimal and lets callers choose
/// whether they need owned copies or borrowed references.
fn rank_file_results_nucleo(results: &[FileResult], filter_pattern: &str) -> Vec<(usize, u32)> {
    use crate::scripts::NucleoCtx;

    let mut nucleo = NucleoCtx::new(filter_pattern);
    let mut scored: Vec<(usize, u32)> = results
        .iter()
        .enumerate()
        .filter_map(|(idx, r)| nucleo.score(&r.name).map(|score| (idx, score)))
        .collect();

    // Sort by score descending (higher = better match), then by name to
    // keep ranking deterministic when scores tie.
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| results[a.0].name.cmp(&results[b.0].name))
            .then_with(|| a.0.cmp(&b.0))
    });

    scored
}
/// Filter FileResults using Nucleo and return only (index, FileResult) pairs
///
/// This is a convenience wrapper for use in UI code where the score isn't needed.
/// Results are pre-sorted by match quality.
///
/// # Arguments
/// * `results` - Slice of FileResult to filter
/// * `filter_pattern` - The pattern to fuzzy-match against file names
///
/// # Returns
/// Vector of (original_index, &FileResult) tuples, sorted by match quality
#[allow(dead_code)]
pub fn filter_results_nucleo_simple<'a>(
    results: &'a [FileResult],
    filter_pattern: &str,
) -> Vec<(usize, &'a FileResult)> {
    rank_file_results_nucleo(results, filter_pattern)
        .into_iter()
        .map(|(idx, _)| (idx, &results[idx]))
        .collect()
}
// --- merged from part_004.rs ---
#[cfg(test)]
include!("mod_tests.rs");

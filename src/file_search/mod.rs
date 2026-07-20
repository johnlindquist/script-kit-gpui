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
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    // ========================================================================
    // Query Builder Tests
    // ========================================================================

    #[test]
    fn test_looks_like_advanced_mdquery_detects_kmditem() {
        assert!(looks_like_advanced_mdquery("kMDItemFSName == 'test'"));
        assert!(looks_like_advanced_mdquery(
            "kMDItemContentType == 'public.image'"
        ));
    }
    #[test]
    fn test_looks_like_advanced_mdquery_detects_operators() {
        assert!(looks_like_advanced_mdquery("name == test"));
        assert!(looks_like_advanced_mdquery("size != 0"));
        assert!(looks_like_advanced_mdquery("date >= 2024"));
        assert!(looks_like_advanced_mdquery("size <= 1000"));
        assert!(looks_like_advanced_mdquery("type == image && size > 1000"));
        assert!(looks_like_advanced_mdquery("ext == jpg || ext == png"));
    }
    #[test]
    fn test_looks_like_advanced_mdquery_simple_queries() {
        // Simple text queries should NOT be detected as advanced
        assert!(!looks_like_advanced_mdquery("hello"));
        assert!(!looks_like_advanced_mdquery("my document"));
        assert!(!looks_like_advanced_mdquery("test.txt"));
        assert!(!looks_like_advanced_mdquery("file-name"));
    }
    #[test]
    fn test_escape_md_string_basic() {
        assert_eq!(escape_md_string("hello"), "hello");
        assert_eq!(escape_md_string("test file"), "test file");
    }
    #[test]
    fn test_escape_md_string_quotes() {
        assert_eq!(escape_md_string(r#"file"name"#), r#"file\"name"#);
        assert_eq!(escape_md_string(r#""quoted""#), r#"\"quoted\""#);
    }
    #[test]
    fn test_escape_md_string_backslashes() {
        assert_eq!(escape_md_string(r"path\to\file"), r"path\\to\\file");
        assert_eq!(escape_md_string(r"\escaped\"), r"\\escaped\\");
    }
    #[test]
    fn test_escape_md_string_mixed() {
        assert_eq!(escape_md_string(r#"file\"name"#), r#"file\\\"name"#);
    }
    #[test]
    fn noisy_recent_path_filter_drops_library_and_apps_keeps_icloud() {
        assert!(is_noisy_recent_file_path("/Applications/Safari.app"));
        assert!(is_noisy_recent_file_path(
            "/Applications/Safari.app/Contents/Info.plist"
        ));
        assert!(is_noisy_recent_file_path(
            "/Users/me/Library/Application Support/CleanShot/media/shot.png"
        ));
        assert!(is_noisy_recent_file_path("/Users/me/.cache/some/file.txt"));
        assert!(!is_noisy_recent_file_path(
            "/Users/me/Library/Mobile Documents/com~apple~CloudDocs/Documents/notes.md"
        ));
        assert!(!is_noisy_recent_file_path("/Users/me/Downloads/report.pdf"));
    }

    #[test]
    fn recent_file_hydration_skips_missing_paths() {
        let path = std::env::temp_dir()
            .join(format!("sk-missing-recent-file-{}", std::process::id()))
            .to_string_lossy()
            .into_owned();
        assert!(file_result_from_existing_path(&path).is_none());
    }
    #[test]
    fn recent_file_hydration_skips_app_bundles() {
        let app_dir = std::env::temp_dir().join(format!(
            "sk-recent-file-hydration-{}.app",
            std::process::id()
        ));
        std::fs::create_dir_all(&app_dir).expect("create temporary app bundle directory");
        let result = file_result_from_existing_path(&app_dir.to_string_lossy());
        let _ = std::fs::remove_dir_all(&app_dir);
        assert!(result.is_none(), "app bundles should stay in app search");
    }

    #[test]
    fn recent_file_hydration_skips_app_bundle_contents() {
        let app_dir = std::env::temp_dir().join(format!(
            "sk-recent-file-hydration-contents-{}.app",
            std::process::id()
        ));
        let contents_dir = app_dir.join("Contents");
        let plist = contents_dir.join("Info.plist");
        std::fs::create_dir_all(&contents_dir).expect("create temporary app bundle contents");
        std::fs::write(&plist, "bundle internals").expect("write temporary app bundle file");

        let result = file_result_from_existing_path(&plist.to_string_lossy());
        let _ = std::fs::remove_dir_all(&app_dir);

        assert!(
            result.is_none(),
            "app bundle contents should stay out of root Recent Files"
        );
    }

    #[test]
    fn recent_file_hydration_returns_file_result_for_existing_file() {
        let file_path = std::env::temp_dir().join(format!(
            "sk-recent-file-hydration-{}.txt",
            std::process::id()
        ));
        std::fs::write(&file_path, "recent file proof").expect("write temp file");
        let result = file_result_from_existing_path(&file_path.to_string_lossy())
            .expect("hydrate existing file");
        let _ = std::fs::remove_file(&file_path);

        assert_eq!(
            result.name,
            file_path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(result.file_type, FileType::Document);
    }
    #[test]
    fn test_build_mdquery_simple_query() {
        let query = build_mdquery("hello");
        assert_eq!(query, r#"kMDItemFSName == "*hello*"c"#);
    }
    #[test]
    fn test_build_mdquery_with_spaces() {
        let query = build_mdquery("my document");
        assert_eq!(query, r#"kMDItemFSName == "*my document*"c"#);
    }
    #[test]
    fn test_build_mdquery_passes_through_advanced() {
        let advanced = "kMDItemFSName == 'test.txt'";
        let query = build_mdquery(advanced);
        assert_eq!(query, advanced); // Should pass through unchanged
    }
    #[test]
    fn test_build_mdquery_with_special_chars() {
        let query = build_mdquery(r#"file"name"#);
        assert_eq!(query, r#"kMDItemFSName == "*file\"name*"c"#);
    }
    #[test]
    fn test_build_mdquery_trims_whitespace() {
        let query = build_mdquery("  hello  ");
        assert_eq!(query, r#"kMDItemFSName == "*hello*"c"#);
    }

    #[test]
    fn root_file_inline_match_mode_is_deterministic_for_root_queries() {
        use RootFileInlineMatchMode::*;

        assert_eq!(
            root_file_inline_match_mode_for_query("why i", RootFileQueryIntent::OrdinaryRoot),
            Some(Phrase)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("why is", RootFileQueryIntent::OrdinaryRoot),
            Some(FilenameWords)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("a b", RootFileQueryIntent::OrdinaryRoot),
            Some(Phrase)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("design", RootFileQueryIntent::OrdinaryRoot),
            Some(SingleTerm)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("egghead.svg", RootFileQueryIntent::OrdinaryRoot),
            Some(FilenameWords)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("a.b", RootFileQueryIntent::OrdinaryRoot),
            Some(Phrase)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("~/dev/al", RootFileQueryIntent::OrdinaryRoot),
            Some(Directory)
        );
        assert_eq!(
            root_file_inline_match_mode_for_query(
                "kMDItemFSName == 'notes.txt'",
                RootFileQueryIntent::OrdinaryRoot
            ),
            None
        );
        assert_eq!(
            root_file_inline_match_mode_for_query("ab", RootFileQueryIntent::OrdinaryRoot),
            None,
            "ordinary two-letter noise remains ineligible"
        );
        assert_eq!(
            root_file_inline_match_mode_for_query(
                "ab",
                RootFileQueryIntent::ExplicitFilesSourceFilter
            ),
            Some(SingleTerm),
            "explicit files source filter keeps relaxed short-query eligibility"
        );
    }

    #[test]
    fn root_file_inline_match_mode_stays_aligned_with_provider_query_shape() {
        assert_eq!(root_file_provider_query_for_user_query("why i"), "why i");

        let word_query = root_file_provider_query_for_user_query("why is");
        assert!(
            word_query.contains(r#"kMDItemFSName == "*why is*"c"#),
            "word mode should retain the phrase provider branch"
        );
        assert!(
            word_query.contains(r#"kMDItemFSName == "*why*"c && kMDItemFSName == "*is*"c"#),
            "word mode should include the all-filename-terms provider branch"
        );

        let separator_query = root_file_provider_query_for_user_query("egghead.svg");
        assert_eq!(
            root_file_inline_match_mode_for_query("egghead.svg", RootFileQueryIntent::OrdinaryRoot),
            Some(RootFileInlineMatchMode::FilenameWords)
        );
        assert!(
            separator_query.contains(r#"kMDItemFSName == "*egghead.svg*"c"#),
            "separator word mode should retain the exact phrase provider branch"
        );
        assert!(
            separator_query
                .contains(r#"kMDItemFSName == "*egghead*"c && kMDItemFSName == "*svg*"c"#),
            "separator word mode should include the all-subtokens provider branch"
        );
    }

    #[test]
    fn root_file_provider_query_expands_safe_filename_separator_queries() {
        assert_eq!(
            root_file_provider_query_for_user_query("egghead.svg"),
            r#"(kMDItemFSName == "*egghead.svg*"c || (kMDItemFSName == "*egghead*"c && kMDItemFSName == "*svg*"c))"#
        );
    }

    #[test]
    fn root_file_provider_query_expands_safe_multiword_filename_queries() {
        let query = root_file_provider_query_for_user_query("design notes");
        assert_eq!(
            query,
            r#"(kMDItemFSName == "*design notes*"c || (kMDItemFSName == "*design*"c && kMDItemFSName == "*notes*"c) || (kMDItemPath == "*design*"c && kMDItemFSName == "*notes*"c))"#
        );
    }

    #[test]
    fn root_file_path_context_provider_query_adds_safe_directory_filename_branches() {
        let query = root_file_provider_query_for_user_query("src root file");
        assert!(
            query.contains(
                r#"kMDItemFSName == "*src*"c && kMDItemFSName == "*root*"c && kMDItemFSName == "*file*"c"#
            ),
            "all-terms filename branch should remain"
        );
        assert!(
            query.contains(
                r#"kMDItemPath == "*src*"c && kMDItemFSName == "*root*"c && kMDItemFSName == "*file*"c"#
            ),
            "provider should retrieve files whose leading terms are directory context"
        );
    }

    #[test]
    fn root_file_path_context_provider_query_rejects_short_plain_parent_terms() {
        let query = root_file_provider_query_for_user_query("ai readme");
        assert!(
            !query.contains("kMDItemPath"),
            "plain two-letter parent context should not create noisy path-provider branches"
        );

        let digit_query = root_file_provider_query_for_user_query("q2 readme");
        assert!(
            digit_query.contains(r#"kMDItemPath == "*q2*"c && kMDItemFSName == "*readme*"c"#),
            "short digit tokens should remain eligible as directory context"
        );
    }

    #[test]
    fn root_file_provider_query_does_not_expand_one_character_terms() {
        assert_eq!(root_file_provider_query_for_user_query("a b"), "a b");
        assert_eq!(
            root_file_provider_query_for_user_query("q report"),
            "q report"
        );
        assert_eq!(root_file_provider_query_for_user_query("a.b"), "a.b");
    }

    #[test]
    fn root_file_search_requires_simple_name_queries() {
        assert!(!should_search_root_files(""));
        assert!(!should_search_root_files("ab"));
        assert!(should_search_root_files("abc"));
        assert!(should_search_root_files("  abc  "));
        assert!(!should_search_root_files("/Users/example"));
        assert!(!should_search_root_files("~/Documents"));
        assert!(!should_search_root_files("kMDItemFSName == 'notes.txt'"));
    }

    #[test]
    fn root_file_short_digit_queries_are_eligible_without_enabling_two_letter_noise() {
        assert!(should_search_root_files("q2"));
        assert!(should_search_root_files("Q2"));
        assert!(should_search_root_files("v2"));
        assert!(should_search_root_files("3d"));
        assert!(should_search_root_files("x1"));
        assert!(!should_search_root_files("ab"));
        assert!(!should_search_root_files("ai"));
        assert!(!should_search_root_files("ui"));
        assert!(!should_search_root_files("q"));
        assert!(!should_search_root_files("2"));
        assert!(!should_search_root_files("~/q2"));
        assert!(!should_search_root_files("kMDItemFSName == 'q2'"));
    }

    #[test]
    fn explicit_files_source_filter_allows_single_character_file_queries() {
        assert!(!should_search_root_files("s"));
        assert!(should_search_root_files_for_intent(
            "s",
            RootFileQueryIntent::ExplicitFilesSourceFilter
        ));
        assert!(should_search_root_files_for_intent(
            "S",
            RootFileQueryIntent::ExplicitFilesSourceFilter
        ));
        assert!(!should_search_root_files("sc"));
        assert!(should_search_root_files_for_intent(
            "sc",
            RootFileQueryIntent::ExplicitFilesSourceFilter
        ));
        assert!(should_search_root_files_for_intent(
            "SC",
            RootFileQueryIntent::ExplicitFilesSourceFilter
        ));
        assert!(!should_search_root_files_for_intent(
            "~/sc",
            RootFileQueryIntent::ExplicitFilesSourceFilter
        ));
        assert!(!should_search_root_files_for_intent(
            "kMDItemFSName == 'sc'",
            RootFileQueryIntent::ExplicitFilesSourceFilter
        ));
    }

    #[test]
    fn root_file_name_token_match_accepts_separator_boundary_prefixes() {
        assert!(root_file_name_token_matches_query(
            "client-design-notes.md",
            "design"
        ));
        assert!(root_file_name_token_matches_query(
            "Q2 Report.pdf",
            "report"
        ));
        assert!(root_file_name_token_matches_query(
            "project_alpha_summary.txt",
            "alpha"
        ));
        assert!(root_file_name_token_matches_query(
            "design-notes.md",
            "design"
        ));
    }

    #[test]
    fn root_file_name_token_match_accepts_camel_case_boundaries() {
        assert!(root_file_name_token_matches_query(
            "ClientDesignNotes.md",
            "design"
        ));
        assert!(root_file_name_token_matches_query(
            "clientDesignNotes.md",
            "notes"
        ));
        assert!(root_file_name_token_matches_query(
            "ScriptKitGPUI.md",
            "kit"
        ));
        assert!(root_file_name_token_matches_query(
            "ScriptKitGPUI.md",
            "gpui"
        ));
    }

    #[test]
    fn root_file_name_token_match_accepts_acronym_and_digit_boundaries() {
        assert!(root_file_name_token_matches_query(
            "HTTPServerLogs.md",
            "server"
        ));
        assert!(root_file_name_token_matches_query("Q2Report.pdf", "report"));
    }

    #[test]
    fn root_file_name_token_match_rejects_mid_token_and_empty_queries() {
        assert!(!root_file_name_token_matches_query(
            "redesign-notes.md",
            "design"
        ));
        assert!(!root_file_name_token_matches_query(
            "redesignNotes.md",
            "design"
        ));
        assert!(!root_file_name_token_matches_query(
            "myserverLogs.md",
            "server"
        ));
        assert!(!root_file_name_token_matches_query("notes.md", ""));
    }

    #[test]
    fn root_file_exact_or_stem_match_accepts_exact_stem() {
        assert!(root_file_name_exact_or_stem_matches_query(
            "design-notes.md",
            "design-notes"
        ));
        assert!(root_file_name_exact_or_stem_matches_query(
            "design-notes.md",
            "design-notes.md"
        ));
    }

    #[test]
    fn root_file_exact_or_stem_match_rejects_token_prefix() {
        assert!(!root_file_name_exact_or_stem_matches_query(
            "design-notes.md",
            "design"
        ));
    }

    #[test]
    fn root_file_exact_or_stem_match_rejects_boundary_token() {
        assert!(!root_file_name_exact_or_stem_matches_query(
            "client-design-notes.md",
            "design"
        ));
    }

    #[test]
    fn root_file_short_digit_token_matches_filename_boundaries() {
        assert!(root_file_name_token_matches_query("Q2Report.pdf", "q2"));
        assert!(root_file_name_token_matches_query(
            "2026-q2-report.xlsx",
            "q2"
        ));
        assert!(root_file_name_token_matches_query(
            "2026Q2Report.xlsx",
            "q2"
        ));
        assert!(!root_file_name_token_matches_query("myq2report.xlsx", "q2"));
    }

    #[test]
    fn root_file_recent_seed_accepts_ordered_directory_context() {
        let result = file("/tmp/src/README.md", "README.md", FileType::Document);

        assert!(
            root_file_recent_seed_matches_query(&result, "src readme"),
            "recent seeds should accept ordered directory-context plus filename matches"
        );
    }

    #[test]
    fn root_file_recent_seed_rejects_path_only_context() {
        let result = file(
            "/tmp/src/readme/archive.txt",
            "archive.txt",
            FileType::Document,
        );

        assert!(
            !root_file_recent_seed_matches_query(&result, "src readme"),
            "recent seeds must not match when every query term is only in the parent path"
        );
    }

    #[test]
    fn root_file_recent_seed_rejects_reversed_directory_context() {
        let result = file("/tmp/docs/README.md", "README.md", FileType::Document);

        assert!(
            !root_file_recent_seed_matches_query(&result, "readme docs"),
            "directory-context recent seeds must preserve parent-then-filename ordering"
        );
    }

    #[test]
    fn root_file_recent_seed_rejects_short_plain_parent_terms() {
        let result = file("/tmp/ai/README.md", "README.md", FileType::Document);

        assert!(
            !root_file_recent_seed_matches_query(&result, "ai readme"),
            "plain two-letter parent terms should not become recent-seed path context"
        );
    }

    #[test]
    fn root_file_recent_seed_accepts_separator_derived_filename_tokens() {
        let result = file("/tmp/egghead-dark.svg", "egghead-dark.svg", FileType::Image);

        assert!(root_file_recent_seed_matches_query(&result, "egghead.svg"));
        assert!(!root_file_recent_seed_matches_query(
            &file("/tmp/egghead-dark.png", "egghead-dark.png", FileType::Image,),
            "egghead.svg"
        ));
    }

    #[test]
    fn root_file_name_token_match_accepts_multiword_separator_tokens() {
        assert!(root_file_name_token_matches_query(
            "design-notes.md",
            "design notes"
        ));
        assert!(root_file_name_token_matches_query(
            "client-design-notes.md",
            "design notes"
        ));
        assert!(root_file_name_token_matches_query(
            "client-design-notes.md",
            "client notes"
        ));
        assert!(root_file_name_token_matches_query(
            "2026-q2-report.xlsx",
            "q2 report"
        ));
        assert!(root_file_name_token_matches_query(
            "root-file-search.md",
            "root file search"
        ));
    }

    #[test]
    fn root_file_name_token_match_rejects_multiword_mid_token_matches() {
        assert!(!root_file_name_token_matches_query(
            "redesign-notes.md",
            "design notes"
        ));
        assert!(!root_file_name_token_matches_query(
            "client-denotes.md",
            "design notes"
        ));
        assert!(!root_file_name_token_matches_query(
            "myq2report.xlsx",
            "q2 report"
        ));
    }

    #[test]
    fn root_file_name_token_match_rejects_unordered_multiword_queries() {
        assert!(!root_file_name_token_matches_query(
            "client-design-notes.md",
            "notes design"
        ));
    }

    fn file(path: &str, name: &str, file_type: FileType) -> FileResult {
        FileResult {
            path: path.to_string(),
            name: name.to_string(),
            size: 0,
            modified: 0,
            file_type,
        }
    }

    #[test]
    fn root_directory_browse_query_accepts_child_fragments() {
        assert_eq!(
            root_file_section_mode_for_query("~/dev/"),
            Some(RootFileSectionMode::DirectoryBrowse)
        );
        assert_eq!(
            root_file_section_mode_for_query("~/dev/al"),
            Some(RootFileSectionMode::DirectoryBrowse)
        );
        assert_eq!(
            root_directory_query_base("~/dev/al"),
            Some("~/dev/".to_string())
        );
        assert_eq!(
            root_file_section_mode_for_query("fix"),
            Some(RootFileSectionMode::GlobalQuery)
        );
    }

    #[test]
    fn root_directory_browse_source_key_ignores_child_fragments() {
        let root =
            std::env::temp_dir().join(format!("script-kit-root-source-key-{}", std::process::id()));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).expect("create temp root directory");

        let base = format!("{}/", nested.display());
        let with_fragment = format!("{base}al");

        let base_key = root_directory_browse_source_key(&base);
        let fragment_key = root_directory_browse_source_key(&with_fragment);

        assert_eq!(base_key, fragment_key);
        // parse_directory_path keeps the directory slash-terminated so all
        // providers compare the same normalized key.
        assert_eq!(
            base_key,
            Some((format!("{}/", nested.to_string_lossy()), false))
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn root_directory_browse_source_key_keeps_hidden_mode() {
        let root = std::env::temp_dir().join(format!(
            ".script-kit-root-source-key-hidden-{}",
            std::process::id()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).expect("create hidden temp root directory");

        let query = format!("{}/al", nested.display());

        assert_eq!(
            root_directory_browse_source_key(&query),
            Some((format!("{}/", nested.to_string_lossy()), true))
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn root_directory_file_matches_preserves_provider_order_without_filter() {
        let results = vec![
            file("/tmp/beta.txt", "beta.txt", FileType::Document),
            file("/tmp/alpha.txt", "alpha.txt", FileType::Document),
        ];

        let matches = root_directory_file_matches(&results, None, 10);

        assert_eq!(
            matches
                .iter()
                .map(|entry| entry.file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["beta.txt", "alpha.txt"]
        );
    }

    #[test]
    fn root_directory_file_matches_filters_by_child_name() {
        let results = vec![
            file("/tmp/beta-notes.md", "beta-notes.md", FileType::Document),
            file("/tmp/beta-folder", "beta-folder", FileType::Directory),
            file(
                "/tmp/alpha-report.md",
                "alpha-report.md",
                FileType::Document,
            ),
            file("/tmp/alpha-folder", "alpha-folder", FileType::Directory),
        ];

        let matches = root_directory_file_matches(&results, Some("al"), 10);

        assert_eq!(
            matches
                .iter()
                .map(|entry| entry.file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha-folder", "alpha-report.md"],
            "child-fragment filtering should match and rank direct child names only"
        );
    }

    #[test]
    fn root_directory_file_matches_does_not_score_parent_paths() {
        let results = vec![
            file(
                "/tmp/alpha-parent/report.md",
                "report.md",
                FileType::Document,
            ),
            file("/tmp/other/alpha.md", "alpha.md", FileType::Document),
        ];

        let matches = root_directory_file_matches(&results, Some("alpha"), 10);

        assert_eq!(
            matches
                .iter()
                .map(|entry| entry.file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha.md"],
            "directory child filtering should not match text that appears only in the parent path"
        );
    }

    #[test]
    fn root_file_ranking_caps_dedupes_and_skips_apps() {
        let results = vec![
            file("/tmp/fix.txt", "fix.txt", FileType::Document),
            file("/tmp/fix.txt", "fix duplicate.txt", FileType::Document),
            file("/Applications/Fix.app", "Fix.app", FileType::Application),
            file("/tmp/fix-notes.md", "fix-notes.md", FileType::Document),
            file("/tmp/prefix-fix.md", "prefix-fix.md", FileType::Document),
        ];

        let ranked = rank_root_file_results(&results, "fix", 2, |_| 0.0);

        assert_eq!(ranked.len(), 2, "render limit should cap root rows");
        assert!(
            ranked
                .iter()
                .all(|entry| entry.file.file_type != FileType::Application),
            "root search should not duplicate app launcher results"
        );
        assert_eq!(
            ranked
                .iter()
                .filter(|entry| entry.file.path == "/tmp/fix.txt")
                .count(),
            1,
            "duplicate Spotlight paths should collapse to one row"
        );
    }

    #[test]
    fn root_global_file_result_eligibility_rejects_app_bundle_contents() {
        assert!(!root_global_file_result_is_eligible(&file(
            "/Applications/Zed.app/Contents/Info.plist",
            "Info.plist",
            FileType::Document,
        )));
        assert!(!root_global_file_result_is_eligible(&file(
            "/Applications/ZED.APP/Contents/Resources/icon.png",
            "icon.png",
            FileType::Image,
        )));
        assert!(root_global_file_result_is_eligible(&file(
            "/Users/example/Documents/Zed Notes/Info.plist",
            "Info.plist",
            FileType::Document,
        )));
    }

    #[test]
    fn root_file_ranking_applies_frecency_to_close_matches() {
        let results = vec![
            file("/tmp/fix-alpha.txt", "fix-alpha.txt", FileType::Document),
            file("/tmp/fix-beta.txt", "fix-beta.txt", FileType::Document),
        ];

        let ranked = rank_root_file_results(&results, "fix", 2, |key| {
            if key == "file//tmp/fix-beta.txt" {
                10.0
            } else {
                0.0
            }
        });

        assert_eq!(
            ranked.first().map(|entry| entry.file.path.as_str()),
            Some("/tmp/fix-beta.txt"),
            "frecency should break close root-file ranking ties"
        );
    }

    #[test]
    fn root_file_ranking_prefers_stem_exact_over_path_only_frecency() {
        let results = vec![
            file(
                "/tmp/fix/archive/report.md",
                "report.md",
                FileType::Document,
            ),
            file("/tmp/other/fix.md", "fix.md", FileType::Document),
        ];

        let ranked = rank_root_file_results(&results, "fix", 2, |key| {
            if key == "file//tmp/fix/archive/report.md" {
                10.0
            } else {
                0.0
            }
        });

        assert_eq!(
            ranked.first().map(|entry| entry.file.path.as_str()),
            Some("/tmp/other/fix.md"),
            "filename stem exact should beat path-only matches even with frecency"
        );
    }

    #[test]
    fn root_file_ranking_prefers_filename_prefix_over_boundary_contains() {
        let results = vec![
            file("/tmp/prefix-fix.md", "prefix-fix.md", FileType::Document),
            file("/tmp/fix-notes.md", "fix-notes.md", FileType::Document),
        ];

        let ranked = rank_root_file_results(&results, "fix", 2, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("fix-notes.md"),
            "filename prefix should beat separator-boundary contains"
        );
    }

    #[test]
    fn root_file_ranking_tiers_filename_with_extension_results() {
        let results = vec![
            file("/tmp/z/egghead.svg", "egghead.svg", FileType::Image),
            file("/tmp/a/egghead.svg", "egghead.svg", FileType::Image),
            file(
                "/tmp/egghead Symbol SVG.svg",
                "egghead Symbol SVG.svg",
                FileType::Image,
            ),
            file(
                "/tmp/c/egghead-dark.svg",
                "egghead-dark.svg",
                FileType::Image,
            ),
            file(
                "/tmp/a/egghead-dark.svg",
                "egghead-dark.svg",
                FileType::Image,
            ),
            file(
                "/tmp/b/egghead-dark.svg",
                "egghead-dark.svg",
                FileType::Image,
            ),
            file(
                "/tmp/egghead-light.svg",
                "egghead-light.svg",
                FileType::Image,
            ),
            file(
                "/tmp/egghead.some-vector-graphic.png",
                "egghead.some-vector-graphic.png",
                FileType::Image,
            ),
        ];

        let ranked = rank_root_file_results(&results, "egghead.svg", results.len(), |_| 0.0);
        let names = ranked
            .iter()
            .map(|entry| entry.file.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(&names[..2], &["egghead.svg", "egghead.svg"]);
        assert_eq!(
            ranked[..2]
                .iter()
                .map(|entry| entry.file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["/tmp/a/egghead.svg", "/tmp/z/egghead.svg"],
            "exact-name ties should remain deterministic by path"
        );
        assert_eq!(names[2], "egghead Symbol SVG.svg");
        assert!(
            names[3..7]
                .iter()
                .all(|name| root_file_name_token_matches_query(name, "egghead.svg")),
            "separator-derived dash-token matches should follow the stronger prefix-style match"
        );
        assert_eq!(
            names.last().copied(),
            Some("egghead.some-vector-graphic.png"),
            "a fuzzy-only different-extension name should rank last"
        );
        assert!(
            ranked[3..7]
                .iter()
                .all(|entry| entry.score / ROOT_FILE_TEXT_TIER_MULTIPLIER == 4),
            "extension-aware separator matches should use token tier 4"
        );
        assert_eq!(
            ranked[2].score / ROOT_FILE_TEXT_TIER_MULTIPLIER,
            5,
            "a prefix whose stem also contains the extension token should use prefix tier 5"
        );
        assert_eq!(
            ranked
                .last()
                .map(|entry| entry.score / ROOT_FILE_TEXT_TIER_MULTIPLIER),
            Some(2),
            "the fuzzy-only name should remain in fuzzy tier 2"
        );
    }

    #[test]
    fn root_file_ranking_prefers_camel_boundary_over_plain_contains_or_path_only() {
        let results = vec![
            file(
                "/tmp/archive/design/readme.md",
                "readme.md",
                FileType::Document,
            ),
            file(
                "/tmp/redesignNotes.md",
                "redesignNotes.md",
                FileType::Document,
            ),
            file(
                "/tmp/ClientDesignNotes.md",
                "ClientDesignNotes.md",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "design", 3, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("ClientDesignNotes.md"),
            "camel-case filename tokens should rank above lowercase mid-token contains and path-only matches"
        );
    }

    #[test]
    fn root_file_ranking_prefers_exact_stem_over_filename_prefix() {
        let results = vec![
            file(
                "/tmp/notes-backup.md",
                "notes-backup.md",
                FileType::Document,
            ),
            file("/tmp/notes.md", "notes.md", FileType::Document),
            file(
                "/tmp/notes/archive/report.md",
                "report.md",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "notes", 3, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("notes.md"),
            "exact filename stem should beat prefix and path-only matches"
        );
        assert!(
            ranked
                .iter()
                .position(|entry| entry.file.name == "notes-backup.md")
                < ranked
                    .iter()
                    .position(|entry| entry.file.name == "report.md"),
            "filename prefix should rank ahead of path-only matches"
        );
    }

    #[test]
    fn root_file_ranking_prefers_fuzzy_filename_over_path_only() {
        let results = vec![
            file("/tmp/final/report.md", "report.md", FileType::Document),
            file(
                "/tmp/other/fnl-notes.md",
                "fnl-notes.md",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "fnl", 2, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("fnl-notes.md"),
            "fuzzy filename match should beat a path-only match"
        );
    }

    #[test]
    fn root_file_path_context_ranking_prefers_directory_context_over_path_only() {
        let results = vec![
            file("/tmp/docs/README.md", "README.md", FileType::Document),
            file(
                "/tmp/docs/readme/archive.txt",
                "archive.txt",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "docs readme", 2, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.path.as_str()),
            Some("/tmp/docs/README.md"),
            "directory-context filename matches should beat path-only matches"
        );
    }

    #[test]
    fn root_file_path_context_ranking_supports_filename_suffix_terms() {
        let results = vec![
            file(
                "/tmp/src/app_impl/root_file_search.rs",
                "root_file_search.rs",
                FileType::File,
            ),
            file(
                "/tmp/src/root/file/archive.txt",
                "archive.txt",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "src root file", 2, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("root_file_search.rs"),
            "directory context should let remaining query terms match filename tokens in order"
        );
    }

    #[test]
    fn root_file_path_context_ranking_preserves_filename_first() {
        let results = vec![
            file("/tmp/docs/README.md", "README.md", FileType::Document),
            file(
                "/tmp/other/docs-readme.md",
                "docs-readme.md",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "docs readme", 2, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("docs-readme.md"),
            "filename multi-word token matches should stay above directory-context matches"
        );
    }

    #[test]
    fn root_file_path_context_ranking_requires_ordered_parent_then_filename() {
        let results = vec![
            file("/tmp/docs/README.md", "README.md", FileType::Document),
            file(
                "/tmp/other/readme-docs.md",
                "readme-docs.md",
                FileType::Document,
            ),
        ];

        let ranked = rank_root_file_results(&results, "readme docs", 2, |_| 0.0);

        assert_eq!(
            ranked.first().map(|entry| entry.file.name.as_str()),
            Some("readme-docs.md"),
            "directory context should not promote reversed parent/filename term order"
        );
    }
    // ========================================================================
    // File Type Detection Tests
    // ========================================================================

    #[test]
    fn test_detect_file_type_image() {
        assert_eq!(
            detect_file_type(Path::new("/test/photo.png")),
            FileType::Image
        );
        assert_eq!(
            detect_file_type(Path::new("/test/photo.JPG")),
            FileType::Image
        );
        assert_eq!(
            detect_file_type(Path::new("/test/photo.heic")),
            FileType::Image
        );
    }
    #[test]
    fn test_detect_file_type_document() {
        assert_eq!(
            detect_file_type(Path::new("/test/doc.pdf")),
            FileType::Document
        );
        assert_eq!(
            detect_file_type(Path::new("/test/doc.docx")),
            FileType::Document
        );
        assert_eq!(
            detect_file_type(Path::new("/test/doc.txt")),
            FileType::Document
        );
    }
    #[test]
    fn test_detect_file_type_audio() {
        assert_eq!(
            detect_file_type(Path::new("/test/song.mp3")),
            FileType::Audio
        );
        assert_eq!(
            detect_file_type(Path::new("/test/song.wav")),
            FileType::Audio
        );
    }
    #[test]
    fn test_detect_file_type_video() {
        assert_eq!(
            detect_file_type(Path::new("/test/movie.mp4")),
            FileType::Video
        );
        assert_eq!(
            detect_file_type(Path::new("/test/movie.mov")),
            FileType::Video
        );
    }
    #[test]
    fn test_detect_file_type_application() {
        assert_eq!(
            detect_file_type(Path::new("/Applications/Safari.app")),
            FileType::Application
        );
    }
    #[test]
    fn test_detect_file_type_generic_file() {
        assert_eq!(
            detect_file_type(Path::new("/test/script.rs")),
            FileType::File
        );
        assert_eq!(
            detect_file_type(Path::new("/test/config.json")),
            FileType::File
        );
    }
    #[test]
    fn test_search_files_empty_query() {
        let results = search_files("", None, 10);
        assert!(results.is_empty());
    }
    #[test]
    fn test_file_result_creation() {
        let result = FileResult {
            path: "/test/file.txt".to_string(),
            name: "file.txt".to_string(),
            size: 1024,
            modified: 1234567890,
            file_type: FileType::Document,
        };

        assert_eq!(result.path, "/test/file.txt");
        assert_eq!(result.name, "file.txt");
        assert_eq!(result.size, 1024);
        assert_eq!(result.file_type, FileType::Document);
    }
    #[test]
    fn test_file_metadata_creation() {
        let meta = FileMetadata {
            path: "/test/file.txt".to_string(),
            name: "file.txt".to_string(),
            size: 1024,
            modified: 1234567890,
            file_type: FileType::Document,
            readable: true,
            writable: true,
        };

        assert_eq!(meta.path, "/test/file.txt");
        assert!(meta.readable);
        assert!(meta.writable);
    }
    #[test]
    fn test_default_file_type() {
        assert_eq!(FileType::default(), FileType::Other);
    }
    #[cfg(all(target_os = "macos", feature = "slow-tests"))]
    #[test]
    fn test_search_files_real_query() {
        // This test only runs on macOS and verifies mdfind works
        let results = search_files("System Preferences", Some("/System"), 5);
        // We don't assert specific results as they may vary,
        // but the function should not panic
        assert!(results.len() <= 5);
    }
    #[cfg(all(target_os = "macos", feature = "slow-tests"))]
    #[test]
    fn test_get_file_metadata_real_file() {
        // Test with a file that should exist on all macOS systems
        let meta = get_file_metadata("/System/Library/CoreServices/Finder.app");
        // Finder.app should exist on macOS
        if let Some(m) = meta {
            assert!(!m.name.is_empty());
            assert!(m.readable);
        }
        // It's OK if this returns None on some systems
    }
    // ========================================================================
    // UI Helper Function Tests
    // ========================================================================

    #[test]
    fn test_file_type_icon() {
        assert_eq!(file_type_icon(FileType::Directory), "📁");
        assert_eq!(file_type_icon(FileType::Application), "📦");
        assert_eq!(file_type_icon(FileType::Image), "🖼️");
        assert_eq!(file_type_icon(FileType::Document), "📄");
        assert_eq!(file_type_icon(FileType::Audio), "🎵");
        assert_eq!(file_type_icon(FileType::Video), "🎬");
        assert_eq!(file_type_icon(FileType::File), "📃");
        assert_eq!(file_type_icon(FileType::Other), "📎");
    }

    #[test]
    fn test_is_thumbnail_preview_supported_returns_true_for_supported_extensions() {
        assert!(is_thumbnail_preview_supported("/tmp/photo.png"));
        assert!(is_thumbnail_preview_supported("/tmp/photo.JPG"));
        assert!(is_thumbnail_preview_supported("/tmp/photo.jpeg"));
        assert!(is_thumbnail_preview_supported("/tmp/animation.gif"));
        assert!(is_thumbnail_preview_supported("/tmp/icon.webp"));
        assert!(is_thumbnail_preview_supported("/tmp/logo.svg"));
        assert!(is_thumbnail_preview_supported("/tmp/picture.bmp"));
        assert!(is_thumbnail_preview_supported("/tmp/favicon.ico"));
        assert!(is_thumbnail_preview_supported("/tmp/scan.tiff"));
    }

    #[test]
    fn test_is_thumbnail_preview_supported_returns_false_for_unsupported_extensions() {
        assert!(!is_thumbnail_preview_supported("/tmp/photo.heic"));
        assert!(!is_thumbnail_preview_supported("/tmp/document.pdf"));
        assert!(!is_thumbnail_preview_supported("/tmp/README"));
    }

    #[test]
    fn test_format_file_size() {
        // Bytes
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1023), "1023 B");

        // Kilobytes
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(10240), "10.0 KB");

        // Megabytes
        assert_eq!(format_file_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_file_size(1024 * 1024 * 5), "5.0 MB");

        // Gigabytes
        assert_eq!(format_file_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_file_size(1024 * 1024 * 1024 * 2), "2.0 GB");
    }
    #[test]
    fn test_format_relative_time() {
        use chrono::Local;
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let formatted_now = format_relative_time(now);
        assert!(!formatted_now.contains("Today"));
        assert!(!formatted_now.contains(" at "));

        let yesterday = Local::now() - chrono::Duration::days(1);
        let formatted_yesterday = format_relative_time(yesterday.timestamp() as u64);
        assert!(!formatted_yesterday.contains("Yesterday"));
        assert!(!formatted_yesterday.contains(" at "));

        assert_eq!(format_relative_time(0), "—");
    }
    #[test]
    fn test_shorten_path() {
        // Test with a path that doesn't start with home
        assert_eq!(shorten_path("/usr/local/bin"), "/usr/local/bin");
        assert_eq!(shorten_path("/etc/hosts"), "/etc/hosts");

        // Test with home directory path (if home dir is available)
        if let Some(home) = dirs::home_dir() {
            if let Some(home_str) = home.to_str() {
                let test_path = format!("{}/Documents/test.txt", home_str);
                assert_eq!(shorten_path(&test_path), "~/Documents/test.txt");
            }
        }
    }
    // ========================================================================
    // Directory Navigation Tests
    // ========================================================================

    #[test]
    fn test_expand_path_home() {
        // Test ~ expansion
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_str().unwrap();

            // Just ~
            assert_eq!(expand_path("~"), Some(home_str.to_string()));

            // ~/subdir
            let expanded = expand_path("~/Documents");
            assert!(expanded.is_some());
            assert!(expanded.unwrap().starts_with(home_str));
        }
    }
    #[test]
    fn test_expand_path_absolute() {
        // Absolute paths should pass through unchanged
        assert_eq!(expand_path("/usr/local"), Some("/usr/local".to_string()));
        assert_eq!(expand_path("/"), Some("/".to_string()));
        assert_eq!(
            expand_path("/System/Library"),
            Some("/System/Library".to_string())
        );
    }
    #[test]
    fn test_expand_path_relative_current() {
        // Relative paths with .
        let cwd = std::env::current_dir().unwrap();
        let cwd_str = cwd.to_str().unwrap();

        // Just .
        let expanded = expand_path(".");
        assert!(expanded.is_some());
        assert_eq!(expanded.unwrap(), cwd_str);

        // ./subdir
        let expanded = expand_path("./src");
        assert!(expanded.is_some());
        let expected = cwd.join("src");
        assert_eq!(expanded.unwrap(), expected.to_str().unwrap());
    }
    #[test]
    fn test_expand_path_relative_parent() {
        // Relative paths with ..
        let cwd = std::env::current_dir().unwrap();
        if let Some(parent) = cwd.parent() {
            let parent_str = parent.to_str().unwrap();

            // Just ..
            let expanded = expand_path("..");
            assert!(expanded.is_some());
            assert_eq!(expanded.unwrap(), parent_str);
        }
    }
    #[test]
    fn test_expand_path_empty() {
        assert_eq!(expand_path(""), None);
        assert_eq!(expand_path("   "), None);
    }
    #[test]
    fn test_expand_path_not_path() {
        // Regular text should return None
        assert_eq!(expand_path("hello"), None);
        assert_eq!(expand_path("search query"), None);
    }
    #[test]
    fn test_list_directory_nonexistent() {
        // Non-existent directory should return empty
        let results = list_directory("/this/path/does/not/exist/at/all", 50);
        assert!(results.is_empty());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn test_list_directory_system() {
        // List /System which exists on all macOS systems
        let results = list_directory("/System", 10);
        assert!(!results.is_empty(), "Should find items in /System");

        // Should contain Library
        let has_library = results.iter().any(|r| r.name == "Library");
        assert!(has_library, "Should contain Library folder");

        // Library should be marked as directory
        let library = results.iter().find(|r| r.name == "Library");
        if let Some(lib) = library {
            assert_eq!(lib.file_type, FileType::Directory);
        }
    }
    #[test]
    fn test_list_directory_home() {
        // List home directory using ~
        let results = list_directory("~", 100);

        // Home should have at least some contents
        // (assuming it's a valid home directory)
        // Don't assert specific files as they vary by system
        assert!(
            results.is_empty() || !results.is_empty(),
            "Should not panic on home directory"
        );
    }
    #[test]
    fn test_list_directory_dirs_first() {
        // Test using /tmp which usually has both dirs and files
        let results = list_directory("/tmp", 50);

        // If we have results, verify sorting
        if results.len() >= 2 {
            // Find first file (non-directory)
            let first_file_idx = results
                .iter()
                .position(|r| !matches!(r.file_type, FileType::Directory));

            // Find last directory
            let last_dir_idx = results
                .iter()
                .rposition(|r| matches!(r.file_type, FileType::Directory));

            // If we have both dirs and files, dirs should come first
            if let (Some(first_file), Some(last_dir)) = (first_file_idx, last_dir_idx) {
                assert!(
                    last_dir < first_file,
                    "Directories should come before files"
                );
            }
        }
    }
    // --- merged from part_001.rs ---
    #[test]
    fn test_list_directory_limit() {
        let unique = format!(
            "script-kit-file-search-limit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_dir).expect("should create test directory");

        let nested_dir = temp_dir.join("A-dir");
        std::fs::create_dir_all(&nested_dir).expect("should create nested directory");
        std::fs::write(temp_dir.join("b.txt"), b"b").expect("should create b.txt");
        std::fs::write(temp_dir.join("a.txt"), b"a").expect("should create a.txt");
        std::fs::write(temp_dir.join("c.txt"), b"c").expect("should create c.txt");

        let results = list_directory(temp_dir.to_str().expect("utf8 temp path"), 3);
        let names: Vec<&str> = results.iter().map(|result| result.name.as_str()).collect();

        assert_eq!(results.len(), 3, "directory listing should obey limit");
        assert_eq!(
            names,
            vec!["A-dir", "a.txt", "b.txt"],
            "results should be sorted before truncation"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    #[test]
    fn test_list_directory_zero_limit_returns_empty() {
        let tmp_dir = std::env::temp_dir();
        let results = list_directory(tmp_dir.to_str().expect("utf8 temp path"), 0);
        assert!(results.is_empty(), "limit=0 should return no results");
    }
    #[test]
    fn test_list_directory_hides_dotfiles_by_default() {
        let results = list_directory("~", 100);

        for result in &results {
            assert!(
                !result.name.starts_with('.'),
                "default listing should not include hidden files: {}",
                result.name
            );
        }
    }

    #[test]
    fn test_list_directory_with_options_can_include_dotfiles() {
        let unique = format!(
            "script-kit-file-search-hidden-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(temp_dir.join(".hidden-dir")).expect("should create hidden dir");
        std::fs::write(temp_dir.join(".hidden-file"), b"hidden").expect("should create dotfile");
        std::fs::write(temp_dir.join("visible-file"), b"visible").expect("should create file");

        let hidden_results =
            list_directory_with_options(temp_dir.to_str().expect("utf8 temp path"), 10, true);
        let hidden_names: Vec<&str> = hidden_results
            .iter()
            .map(|result| result.name.as_str())
            .collect();
        assert!(
            hidden_names.contains(&".hidden-dir"),
            "hidden listing should include hidden directories"
        );
        assert!(
            hidden_names.contains(&".hidden-file"),
            "hidden listing should include dotfiles"
        );

        let default_results = list_directory(temp_dir.to_str().expect("utf8 temp path"), 10);
        let default_names: Vec<&str> = default_results
            .iter()
            .map(|result| result.name.as_str())
            .collect();
        assert!(
            !default_names.contains(&".hidden-dir"),
            "default listing should still hide hidden directories"
        );
        assert!(
            !default_names.contains(&".hidden-file"),
            "default listing should still hide dotfiles"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    #[test]
    fn test_is_directory_path_reexport() {
        // Verify the re-export works
        assert!(is_directory_path("~/dev"));
        assert!(is_directory_path("/usr/local"));
        assert!(is_directory_path("./src"));
        assert!(!is_directory_path("hello world"));
    }
    // ========================================================================
    // Nucleo Filtering Tests
    // ========================================================================

    #[test]
    fn test_filter_results_nucleo_empty_pattern() {
        let results = vec![
            FileResult {
                path: "/test/apple.txt".to_string(),
                name: "apple.txt".to_string(),
                size: 100,
                modified: 0,
                file_type: FileType::Document,
            },
            FileResult {
                path: "/test/banana.txt".to_string(),
                name: "banana.txt".to_string(),
                size: 200,
                modified: 0,
                file_type: FileType::Document,
            },
        ];

        // Empty pattern with Nucleo matches everything (score 0)
        // This is expected behavior - caller should check for empty pattern before calling
        let filtered = filter_results_nucleo_simple(&results, "");
        assert_eq!(filtered.len(), 2);
    }
    #[test]
    fn test_filter_results_nucleo_empty_pattern_uses_name_tiebreaker() {
        let results = vec![
            FileResult {
                path: "/test/zeta.txt".to_string(),
                name: "zeta.txt".to_string(),
                size: 100,
                modified: 0,
                file_type: FileType::Document,
            },
            FileResult {
                path: "/test/alpha.txt".to_string(),
                name: "alpha.txt".to_string(),
                size: 200,
                modified: 0,
                file_type: FileType::Document,
            },
        ];

        let filtered = filter_results_nucleo_simple(&results, "");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].1.name, "alpha.txt");
        assert_eq!(filtered[1].1.name, "zeta.txt");
    }
    #[test]
    fn test_filter_results_nucleo_exact_match() {
        let results = vec![
            FileResult {
                path: "/test/mcp-final.txt".to_string(),
                name: "mcp-final".to_string(),
                size: 100,
                modified: 0,
                file_type: FileType::File,
            },
            FileResult {
                path: "/test/definitions.txt".to_string(),
                name: "definitions".to_string(),
                size: 200,
                modified: 0,
                file_type: FileType::File,
            },
        ];

        // "final" should match "mcp-final" better than "definitions"
        let filtered = filter_results_nucleo_simple(&results, "final");
        assert!(!filtered.is_empty());
        assert_eq!(filtered[0].1.name, "mcp-final");
    }
    #[test]
    fn test_filter_results_nucleo_fuzzy_ordering() {
        let results = vec![
            FileResult {
                path: "/test/define.txt".to_string(),
                name: "define".to_string(),
                size: 100,
                modified: 0,
                file_type: FileType::File,
            },
            FileResult {
                path: "/test/mcp-final.txt".to_string(),
                name: "mcp-final".to_string(),
                size: 200,
                modified: 0,
                file_type: FileType::File,
            },
            FileResult {
                path: "/test/final-test.txt".to_string(),
                name: "final-test".to_string(),
                size: 300,
                modified: 0,
                file_type: FileType::File,
            },
        ];

        // "fin" should fuzzy match both "mcp-final" and "final-test"
        // Both should rank higher than "define" (which has f, i, n but not consecutive)
        let filtered = filter_results_nucleo_simple(&results, "fin");

        // Should have matches
        assert!(!filtered.is_empty());

        // "final-test" or "mcp-final" should be first (both have "fin" as prefix of "final")
        let first_name = &filtered[0].1.name;
        assert!(
            first_name.contains("final"),
            "Expected 'final' in first result, got: {}",
            first_name
        );
    }
    #[test]
    fn test_filter_results_nucleo_no_matches() {
        let results = vec![
            FileResult {
                path: "/test/apple.txt".to_string(),
                name: "apple".to_string(),
                size: 100,
                modified: 0,
                file_type: FileType::File,
            },
            FileResult {
                path: "/test/banana.txt".to_string(),
                name: "banana".to_string(),
                size: 200,
                modified: 0,
                file_type: FileType::File,
            },
        ];

        // "xyz" should not match anything
        let filtered = filter_results_nucleo_simple(&results, "xyz");
        assert!(filtered.is_empty());
    }
    #[test]
    fn test_filter_results_nucleo_case_insensitive() {
        let results = vec![FileResult {
            path: "/test/MyDocument.txt".to_string(),
            name: "MyDocument".to_string(),
            size: 100,
            modified: 0,
            file_type: FileType::Document,
        }];

        // Should match regardless of case
        let filtered_lower = filter_results_nucleo_simple(&results, "mydoc");
        let filtered_upper = filter_results_nucleo_simple(&results, "MYDOC");
        let filtered_mixed = filter_results_nucleo_simple(&results, "MyDoc");

        assert!(!filtered_lower.is_empty());
        assert!(!filtered_upper.is_empty());
        assert!(!filtered_mixed.is_empty());
    }
    // ========================================================================
    // FileInfo Tests
    // ========================================================================

    #[test]
    fn test_file_info_from_result() {
        let result = FileResult {
            path: "/test/document.pdf".to_string(),
            name: "document.pdf".to_string(),
            size: 1024,
            modified: 1234567890,
            file_type: FileType::Document,
        };

        let info = FileInfo::from_result(&result);
        assert_eq!(info.path, "/test/document.pdf");
        assert_eq!(info.name, "document.pdf");
        assert_eq!(info.file_type, FileType::Document);
        assert!(!info.is_dir);
    }
    #[test]
    fn test_file_info_from_result_directory() {
        let result = FileResult {
            path: "/test/Documents".to_string(),
            name: "Documents".to_string(),
            size: 0,
            modified: 1234567890,
            file_type: FileType::Directory,
        };

        let info = FileInfo::from_result(&result);
        assert_eq!(info.path, "/test/Documents");
        assert_eq!(info.name, "Documents");
        assert_eq!(info.file_type, FileType::Directory);
        assert!(info.is_dir);
    }
    #[test]
    fn test_file_info_from_path() {
        // Test with a path that likely exists
        let info = FileInfo::from_path("/tmp");
        assert_eq!(info.path, "/tmp");
        assert_eq!(info.name, "tmp");
        // /tmp should be a directory on Unix systems
        #[cfg(unix)]
        assert!(info.is_dir);
    }
    // ========================================================================
    // Path Utility Tests (ensure_trailing_slash, parent_dir_display)
    // ========================================================================

    #[test]
    fn test_ensure_trailing_slash_already_has_slash() {
        assert_eq!(ensure_trailing_slash("/foo/bar/"), "/foo/bar/");
        assert_eq!(ensure_trailing_slash("~/dev/"), "~/dev/");
        assert_eq!(ensure_trailing_slash("/"), "/");
        assert_eq!(ensure_trailing_slash("~/"), "~/");
    }
    #[test]
    fn test_ensure_trailing_slash_needs_slash() {
        assert_eq!(ensure_trailing_slash("/foo/bar"), "/foo/bar/");
        assert_eq!(ensure_trailing_slash("~/dev"), "~/dev/");
        assert_eq!(ensure_trailing_slash(".."), "../");
        assert_eq!(ensure_trailing_slash("."), "./");
    }
    #[test]
    fn test_ensure_trailing_slash_edge_cases() {
        // Empty string
        assert_eq!(ensure_trailing_slash(""), "/");
        // Single tilde
        assert_eq!(ensure_trailing_slash("~"), "~/");
    }
    #[test]
    fn parent_folder_search_query_returns_trailing_slashed_parent() {
        assert_eq!(
            parent_folder_search_query("/tmp/projects/readme.md"),
            Some("/tmp/projects/".to_string())
        );
    }
    #[test]
    fn parent_folder_search_query_handles_root_parent() {
        assert_eq!(parent_folder_search_query("/hosts"), Some("/".to_string()));
    }
    #[test]
    fn parent_folder_search_query_rejects_relative_leaf_without_parent() {
        assert_eq!(parent_folder_search_query("readme.md"), None);
    }

    #[test]
    fn parent_folder_search_query_shortens_home_prefix_for_display() {
        let home = dirs::home_dir()
            .and_then(|path| path.to_str().map(|value| value.to_string()))
            .expect("home path should be valid UTF-8");
        let file = format!("{home}/dev/script-kit-gpui/README.md");

        assert_eq!(
            parent_folder_search_query(&file),
            Some("~/dev/script-kit-gpui/".to_string())
        );
    }

    #[test]
    fn shorten_home_prefix_for_display_respects_path_boundaries() {
        assert_eq!(
            shorten_home_prefix_for_display_with_home(
                "/Users/johnlindquist/dev/script-kit-gpui/",
                "/Users/johnlindquist"
            ),
            "~/dev/script-kit-gpui/"
        );
        assert_eq!(
            shorten_home_prefix_for_display_with_home(
                "/Users/johnlindquist",
                "/Users/johnlindquist"
            ),
            "~"
        );
        assert_eq!(
            shorten_home_prefix_for_display_with_home(
                "/Users/johnlindquistness/dev/",
                "/Users/johnlindquist"
            ),
            "/Users/johnlindquistness/dev/"
        );
    }

    #[test]
    fn test_parent_dir_display_root() {
        // "/" has no parent
        assert_eq!(parent_dir_display("/"), None);
    }
    #[test]
    fn test_parent_dir_display_home_root() {
        // "~/" has no parent (home directory is treated as root)
        assert_eq!(parent_dir_display("~/"), None);
    }
    #[test]
    fn test_parent_dir_display_relative_parent() {
        // "../" -> "../../"
        assert_eq!(parent_dir_display("../"), Some("../../".to_string()));
    }
    #[test]
    fn test_parent_dir_display_relative_current() {
        // "./" -> "../"
        assert_eq!(parent_dir_display("./"), Some("../".to_string()));
    }
    #[test]
    fn test_parent_dir_display_tilde_subdir() {
        // "~/foo/" -> "~/"
        assert_eq!(parent_dir_display("~/foo/"), Some("~/".to_string()));
        // "~/foo/bar/" -> "~/foo/"
        assert_eq!(parent_dir_display("~/foo/bar/"), Some("~/foo/".to_string()));
    }
    #[test]
    fn test_parent_dir_display_absolute_subdir() {
        // "/foo/bar/" -> "/foo/"
        assert_eq!(parent_dir_display("/foo/bar/"), Some("/foo/".to_string()));
        // "/foo/" -> "/"
        assert_eq!(parent_dir_display("/foo/"), Some("/".to_string()));
    }
    #[test]
    fn test_parent_dir_display_multiple_levels() {
        // Deep paths
        assert_eq!(parent_dir_display("/a/b/c/d/"), Some("/a/b/c/".to_string()));
        assert_eq!(
            parent_dir_display("~/projects/rust/kit/"),
            Some("~/projects/rust/".to_string())
        );
    }
    #[test]
    fn test_parent_dir_display_no_trailing_slash() {
        // Paths without trailing slash should still work (normalize first)
        // The function expects trailing slash, but should handle edge cases gracefully
        assert_eq!(parent_dir_display("/foo/bar"), Some("/foo/".to_string()));
        assert_eq!(parent_dir_display("~/foo"), Some("~/".to_string()));
    }
    #[test]
    fn test_terminal_working_directory_uses_directory_path_when_is_dir() {
        let resolved = terminal_working_directory("/tmp/projects", true);
        assert_eq!(resolved, "/tmp/projects");
    }
    #[test]
    fn test_terminal_working_directory_uses_parent_for_file_paths() {
        let resolved = terminal_working_directory("/tmp/projects/readme.md", false);
        assert_eq!(resolved, "/tmp/projects");
    }
    #[test]
    fn test_terminal_working_directory_falls_back_to_current_dir_without_parent() {
        let resolved = terminal_working_directory("readme.md", false);
        assert_eq!(resolved, ".");
    }
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_move_to_trash_returns_explicit_unsupported_error_on_non_macos() {
        let error = move_to_trash("/tmp/projects/readme.md").unwrap_err();
        assert!(
            error.contains("only supported on macOS"),
            "error should explain platform limitation, got: {}",
            error
        );
    }
}

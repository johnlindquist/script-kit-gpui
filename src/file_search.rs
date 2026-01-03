//! File Search Module using macOS Spotlight (mdfind)
//!
//! This module provides file search functionality using macOS's mdfind command,
//! which interfaces with the Spotlight index for fast file searching.

use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;
use tracing::{debug, instrument, warn};

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

/// Default limit for search results
pub const DEFAULT_LIMIT: usize = 50;

/// Escape a query string for safe use in shell command
/// Uses single quotes with proper escaping for shell safety
#[allow(dead_code)]
fn escape_query(query: &str) -> String {
    // For mdfind, we wrap in single quotes and escape any single quotes in the query
    // Single quotes prevent shell expansion of special characters
    let escaped = query.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

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

/// Search for files using macOS mdfind (Spotlight)
///
/// # Arguments
/// * `query` - Search query string
/// * `onlyin` - Optional directory to limit search scope
/// * `limit` - Maximum number of results to return
///
/// # Returns
/// Vector of FileResult structs containing file information
///
#[instrument(skip_all, fields(query = %query, onlyin = ?onlyin, limit = limit))]
pub fn search_files(query: &str, onlyin: Option<&str>, limit: usize) -> Vec<FileResult> {
    debug!("Starting mdfind search");

    if query.is_empty() {
        debug!("Empty query, returning empty results");
        return Vec::new();
    }

    let mut cmd = Command::new("mdfind");

    // Add -onlyin if specified
    if let Some(dir) = onlyin {
        cmd.arg("-onlyin").arg(dir);
    }

    // Add the query (no escaping needed as Command handles it)
    cmd.arg(query);

    debug!(command = ?cmd, "Executing mdfind");

    let output = match cmd.output() {
        Ok(output) => output,
        Err(e) => {
            warn!(error = %e, "Failed to execute mdfind");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(stderr = %stderr, "mdfind returned non-zero exit status");
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines().take(limit) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let path = Path::new(line);

        // Get file metadata
        let (size, modified) = match std::fs::metadata(path) {
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

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let file_type = detect_file_type(path);

        results.push(FileResult {
            path: line.to_string(),
            name,
            size,
            modified,
            file_type,
        });
    }

    debug!(result_count = results.len(), "Search completed");
    results
}

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

    let file_type = if metadata.is_dir() {
        FileType::Directory
    } else {
        detect_file_type(path_obj)
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_query_simple() {
        assert_eq!(escape_query("hello"), "'hello'");
    }

    #[test]
    fn test_escape_query_with_spaces() {
        assert_eq!(escape_query("hello world"), "'hello world'");
    }

    #[test]
    fn test_escape_query_with_single_quote() {
        assert_eq!(escape_query("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_escape_query_with_special_chars() {
        assert_eq!(escape_query("file*.txt"), "'file*.txt'");
    }

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

    #[cfg(target_os = "macos")]
    #[test]
    fn test_search_files_real_query() {
        // This test only runs on macOS and verifies mdfind works
        let results = search_files("System Preferences", Some("/System"), 5);
        // We don't assert specific results as they may vary,
        // but the function should not panic
        assert!(results.len() <= 5);
    }

    #[cfg(target_os = "macos")]
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
}

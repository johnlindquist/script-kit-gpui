//! Screenshot-to-file bridge for Tab AI harness context injection.
//!
//! Captures the focused window screenshot to a temporary PNG file so the
//! harness backend can read the image natively via file path — no base64
//! data is pasted into the PTY.

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Maximum number of screenshot files to retain in the temp directory.
pub const TAB_AI_SCREENSHOT_MAX_KEEP: usize = 10;

/// Filename prefix used for Tab AI screenshot temp files.
const TAB_AI_SCREENSHOT_PREFIX: &str = "tab-ai-screenshot-";

/// Monotonic sequence counter to prevent filename collisions when two
/// captures happen within the same second in the same process.
static TAB_AI_SCREENSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Most recent Tab AI screenshot identity (bare filename, no directory).
/// Populated by both capture paths after a successful write so automation
/// can read `getState.screenshotIdentity` to verify the identity-threading
/// chain without grepping the filesystem.
static TAB_AI_SCREENSHOT_LAST_IDENTITY: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn last_identity_slot() -> &'static Mutex<Option<String>> {
    TAB_AI_SCREENSHOT_LAST_IDENTITY.get_or_init(|| Mutex::new(None))
}

/// Record the identity of the most recent Tab AI screenshot (bare filename).
/// Called by the capture paths; exposed for tests.
pub fn record_last_screenshot_identity(identity: String) {
    if let Ok(mut slot) = last_identity_slot().lock() {
        *slot = Some(identity);
    }
}

/// Read the most recent Tab AI screenshot identity, if any.
/// `None` means no screenshot has been captured in this process lifetime.
/// Used by the `stateResult.screenshotIdentity` automation field.
pub fn current_screenshot_identity() -> Option<String> {
    last_identity_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

/// Build a collision-safe screenshot filename with millisecond precision
/// and a monotonic sequence number.
fn build_tab_ai_screenshot_filename(
    now: chrono::DateTime<chrono::Utc>,
    pid: u32,
    sequence: u64,
) -> String {
    format!(
        "{}{}-{}-{}.png",
        TAB_AI_SCREENSHOT_PREFIX,
        now.format("%Y%m%dT%H%M%S%.3fZ"),
        pid,
        sequence,
    )
}

/// Result of writing a focused window screenshot to a temp file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabAiScreenshotFile {
    /// Absolute path to the PNG file on disk.
    pub path: String,
    /// Width of the captured image in pixels.
    pub width: u32,
    /// Height of the captured image in pixels.
    pub height: u32,
    /// Title of the captured window.
    pub title: String,
    /// True if the capture fell back to the first available window
    /// (no focused window was detected).
    pub used_fallback: bool,
}

fn write_private_tab_ai_screenshot(path: &std::path::Path, png: &[u8]) -> Result<()> {
    crate::atomic_file::write_private_atomic(path, png)
        .context("failed to write owner-only private screenshot")
}

fn prepare_private_tab_ai_screenshot_directory(path: &std::path::Path) -> Result<()> {
    crate::atomic_file::ensure_private_directory(path)
        .context("failed to prepare owner-only private screenshot directory")
}

fn log_private_tab_ai_screenshot_written(
    event: &'static str,
    path: &str,
    title: Option<&str>,
    width: u32,
    height: u32,
    used_fallback: bool,
    bytes: usize,
) {
    let safe_path = crate::logging::log_private_user_value(path);
    if let Some(title) = title {
        let safe_title = crate::logging::log_private_user_value(title);
        tracing::debug!(
            event,
            path_bytes = safe_path.raw_bytes,
            path_sha256 = %safe_path.sha256,
            title_bytes = safe_title.raw_bytes,
            title_sha256 = %safe_title.sha256,
            width,
            height,
            used_fallback,
            bytes,
        );
    } else {
        tracing::debug!(
            event,
            path_bytes = safe_path.raw_bytes,
            path_sha256 = %safe_path.sha256,
            width,
            height,
            bytes,
        );
    }
}

fn log_private_tab_ai_screenshot_failure(event: &'static str, error: &impl std::fmt::Display) {
    let safe_error = crate::logging::log_private_user_value(&error.to_string());
    tracing::debug!(
        event,
        error_bytes = safe_error.raw_bytes,
        error_sha256 = %safe_error.sha256,
    );
}

fn log_private_tab_ai_screenshot_cleanup_failure(
    path: &std::path::Path,
    error: &impl std::fmt::Display,
) {
    let safe_path = crate::logging::log_private_user_value(&path.display().to_string());
    let safe_error = crate::logging::log_private_user_value(&error.to_string());
    tracing::debug!(
        event = "tab_ai_screenshot_cleanup_file_failed",
        path_bytes = safe_path.raw_bytes,
        path_sha256 = %safe_path.sha256,
        error_bytes = safe_error.raw_bytes,
        error_sha256 = %safe_error.sha256,
    );
}

/// Capture a screenshot of the focused window and write it to a temp file.
///
/// Returns `Ok(None)` if no suitable window was found. Returns `Ok(Some(...))`
/// on success with the absolute path, dimensions, and metadata. Runs
/// [`cleanup_old_tab_ai_screenshot_files`] after a successful write.
pub fn capture_tab_ai_focused_window_screenshot_file() -> Result<Option<TabAiScreenshotFile>> {
    let capture = match crate::platform::capture_focused_window_screenshot() {
        Ok(c) => c,
        Err(e) => {
            log_private_tab_ai_screenshot_failure("tab_ai_screenshot_capture_failed", &e);
            return Ok(None);
        }
    };

    if capture.png_data.is_empty() {
        return Ok(None);
    }

    let tmp_dir = screenshot_tmp_dir()?;
    prepare_private_tab_ai_screenshot_directory(&tmp_dir)?;

    let filename = build_tab_ai_screenshot_filename(
        chrono::Utc::now(),
        std::process::id(),
        TAB_AI_SCREENSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    );
    let file_path = tmp_dir.join(&filename);

    write_private_tab_ai_screenshot(&file_path, &capture.png_data)?;

    let abs_path = file_path.to_string_lossy().into_owned();
    log_private_tab_ai_screenshot_written(
        "tab_ai_screenshot_file_written",
        &abs_path,
        Some(&capture.window_title),
        capture.width,
        capture.height,
        capture.used_fallback,
        capture.png_data.len(),
    );

    record_last_screenshot_identity(filename.clone());

    // Best-effort cleanup — don't fail the capture if cleanup errors
    if let Err(e) = cleanup_old_tab_ai_screenshot_files(TAB_AI_SCREENSHOT_MAX_KEEP) {
        let safe_error = crate::logging::log_private_user_value(&e.to_string());
        tracing::warn!(
            event = "tab_ai_screenshot_cleanup_failed",
            error_bytes = safe_error.raw_bytes,
            error_sha256 = %safe_error.sha256,
        );
    }

    Ok(Some(TabAiScreenshotFile {
        path: abs_path,
        width: capture.width,
        height: capture.height,
        title: capture.window_title,
        used_fallback: capture.used_fallback,
    }))
}

/// Capture a full-screen screenshot and write it to a temp file.
///
/// Mirror of [`capture_tab_ai_focused_window_screenshot_file`] but captures
/// the entire screen via `platform::capture_screen_screenshot()` instead of
/// just the focused window.
pub fn capture_tab_ai_screen_screenshot_file() -> Result<Option<TabAiScreenshotFile>> {
    let (png_data, width, height) = match crate::platform::capture_screen_screenshot() {
        Ok(data) => data,
        Err(e) => {
            log_private_tab_ai_screenshot_failure("tab_ai_screen_screenshot_capture_failed", &e);
            return Ok(None);
        }
    };

    if png_data.is_empty() {
        return Ok(None);
    }

    let tmp_dir = screenshot_tmp_dir()?;
    prepare_private_tab_ai_screenshot_directory(&tmp_dir)?;

    let filename = build_tab_ai_screenshot_filename(
        chrono::Utc::now(),
        std::process::id(),
        TAB_AI_SCREENSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    );
    let file_path = tmp_dir.join(&filename);

    write_private_tab_ai_screenshot(&file_path, &png_data)?;

    let abs_path = file_path.to_string_lossy().into_owned();
    log_private_tab_ai_screenshot_written(
        "tab_ai_screen_screenshot_file_written",
        &abs_path,
        None,
        width,
        height,
        false,
        png_data.len(),
    );

    record_last_screenshot_identity(filename.clone());

    if let Err(e) = cleanup_old_tab_ai_screenshot_files(TAB_AI_SCREENSHOT_MAX_KEEP) {
        let safe_error = crate::logging::log_private_user_value(&e.to_string());
        tracing::warn!(
            event = "tab_ai_screenshot_cleanup_failed",
            error_bytes = safe_error.raw_bytes,
            error_sha256 = %safe_error.sha256,
        );
    }

    Ok(Some(TabAiScreenshotFile {
        path: abs_path,
        width,
        height,
        title: "Full Screen".to_string(),
        used_fallback: false,
    }))
}

/// Remove old Tab AI screenshot files, keeping at most `max_keep` newest files.
pub fn cleanup_old_tab_ai_screenshot_files(max_keep: usize) -> Result<()> {
    let tmp_dir = screenshot_tmp_dir()?;
    if !tmp_dir.exists() {
        return Ok(());
    }

    let mut screenshot_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

    let entries = std::fs::read_dir(&tmp_dir)
        .with_context(|| format!("failed to read screenshot tmp dir: {}", tmp_dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if name_str.starts_with(TAB_AI_SCREENSHOT_PREFIX) && name_str.ends_with(".png") {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            screenshot_files.push((entry.path(), modified));
        }
    }

    if screenshot_files.len() <= max_keep {
        return Ok(());
    }

    // Sort newest first
    screenshot_files.sort_by_key(|a| std::cmp::Reverse(a.1));

    // Remove everything beyond max_keep
    for (path, _) in screenshot_files.iter().skip(max_keep) {
        if let Err(e) = std::fs::remove_file(path) {
            log_private_tab_ai_screenshot_cleanup_failure(path, &e);
        }
    }

    Ok(())
}

/// Return the directory for Tab AI screenshot temp files.
///
/// Uses `~/.scriptkit/tmp/` to keep temp files alongside other Script Kit data.
fn screenshot_tmp_dir() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".scriptkit").join("tmp"))
}

/// Return the directory for Tab AI screenshot temp files, using a custom root.
///
/// This is exposed for tests that need to control the directory.
pub fn cleanup_old_tab_ai_screenshot_files_in_dir(
    dir: &std::path::Path,
    max_keep: usize,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    let mut screenshot_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read screenshot dir: {}", dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if name_str.starts_with(TAB_AI_SCREENSHOT_PREFIX) && name_str.ends_with(".png") {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            screenshot_files.push((entry.path(), modified));
        }
    }

    if screenshot_files.len() <= max_keep {
        return Ok(());
    }

    // Sort newest first
    screenshot_files.sort_by_key(|a| std::cmp::Reverse(a.1));

    // Remove everything beyond max_keep
    for (path, _) in screenshot_files.iter().skip(max_keep) {
        if let Err(e) = std::fs::remove_file(path) {
            log_private_tab_ai_screenshot_cleanup_failure(path, &e);
        }
    }

    Ok(())
}

/// The screenshot filename prefix, exposed for tests.
pub fn tab_ai_screenshot_prefix() -> &'static str {
    TAB_AI_SCREENSHOT_PREFIX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_screenshot_directory_creates_and_repairs_owner_only_permissions_without_capture() {
        use std::os::unix::fs::PermissionsExt as _;

        let isolated = tempfile::tempdir().expect("isolated synthetic screenshot directory");
        let path = isolated.path().join("private-screenshot-temp");
        prepare_private_tab_ai_screenshot_directory(&path)
            .expect("create owner-only screenshot directory");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("simulate permissive legacy screenshot directory");
        prepare_private_tab_ai_screenshot_directory(&path)
            .expect("repair legacy screenshot directory before any capture bytes are written");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_screenshot_directory_refuses_hostile_links_without_capture() {
        let isolated = tempfile::tempdir().expect("isolated hostile screenshot directory");
        let external = isolated.path().join("another-owner");
        std::fs::create_dir(&external).expect("seed unrelated screenshot directory");
        let planted = isolated.path().join("hostile-screenshot-temp");
        std::os::unix::fs::symlink(&external, &planted)
            .expect("plant hostile screenshot directory link");

        assert!(prepare_private_tab_ai_screenshot_directory(&planted).is_err());
        assert_eq!(std::fs::read_dir(external).unwrap().count(), 0);
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    fn unique_test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "script-kit-gpui-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[cfg(unix)]
    #[test]
    fn private_screenshot_file_is_owner_only_and_atomically_replaced_without_capture() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated synthetic screenshot fixture");
        let path = directory.path().join("private-screenshot.png");
        write_private_tab_ai_screenshot(&path, b"\x89PNG\r\nprivate synthetic pixels")
            .expect("write synthetic PNG bytes without capturing a screen");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        write_private_tab_ai_screenshot(&path, b"\x89PNG\r\nreplacement synthetic pixels")
            .expect("atomically replace synthetic PNG bytes");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"\x89PNG\r\nreplacement synthetic pixels"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn private_screenshot_file_rejects_symlinks_without_capture_or_target_changes() {
        let directory = tempfile::tempdir().expect("isolated synthetic screenshot symlink");
        let external = directory.path().join("unrelated-private-image.png");
        let planted = directory.path().join("capture.png");
        std::fs::write(&external, b"preserve unrelated private pixels").unwrap();
        std::os::unix::fs::symlink(&external, &planted).unwrap();

        assert!(write_private_tab_ai_screenshot(&planted, b"\x89PNG private pixels").is_err());
        assert_eq!(
            std::fs::read(&external).unwrap(),
            b"preserve unrelated private pixels"
        );
        assert!(std::fs::symlink_metadata(&planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn private_screenshot_events_never_expose_raw_window_titles_paths_or_errors() {
        #[derive(Clone)]
        struct EventWriter(std::sync::Arc<Mutex<Vec<u8>>>);

        impl std::io::Write for EventWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let path = "/Users/private/cancer-treatment/screenshot.png";
        let title = "Confidential oncology diagnosis";
        let error = anyhow::anyhow!("private screenshot provider token sk-live-secret");
        let safe_path = crate::logging::log_private_user_value(path);
        let safe_title = crate::logging::log_private_user_value(title);
        let safe_error = crate::logging::log_private_user_value(&error.to_string());
        let captured = std::sync::Arc::new(Mutex::new(Vec::new()));
        let writer = std::sync::Arc::clone(&captured);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || EventWriter(std::sync::Arc::clone(&writer)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            log_private_tab_ai_screenshot_written(
                "tab_ai_screenshot_file_written",
                path,
                Some(title),
                640,
                480,
                false,
                128,
            );
            log_private_tab_ai_screenshot_failure("tab_ai_screenshot_capture_failed", &error);
            log_private_tab_ai_screenshot_cleanup_failure(std::path::Path::new(path), &error);
        });

        let output = String::from_utf8(
            captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        )
        .unwrap();
        assert!(!output.contains(path));
        assert!(!output.contains(title));
        assert!(!output.contains("cancer-treatment"));
        assert!(!output.contains("oncology"));
        assert!(!output.contains("sk-live-secret"));
        let events: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["fields"]["path_sha256"], safe_path.sha256);
        assert_eq!(events[0]["fields"]["title_sha256"], safe_title.sha256);
        assert_eq!(events[0]["fields"]["width"], 640);
        assert_eq!(events[1]["fields"]["error_sha256"], safe_error.sha256);
        assert_eq!(events[2]["fields"]["path_sha256"], safe_path.sha256);
        assert_eq!(events[2]["fields"]["error_sha256"], safe_error.sha256);
    }

    #[test]
    fn screenshot_filename_is_unique_for_same_timestamp_and_pid() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-03-30T14:01:20.936Z")
            .expect("must parse")
            .with_timezone(&chrono::Utc);
        let first = build_tab_ai_screenshot_filename(now, 41231, 0);
        let second = build_tab_ai_screenshot_filename(now, 41231, 1);
        assert_ne!(first, second);
        assert!(first.starts_with(TAB_AI_SCREENSHOT_PREFIX));
        assert!(first.ends_with(".png"));
    }

    #[test]
    fn cleanup_keeps_newest_matching_pngs_only() {
        let dir = unique_test_dir("tab-ai-screenshot-cleanup");
        let now = chrono::DateTime::parse_from_rfc3339("2026-03-30T14:01:20.936Z")
            .expect("must parse")
            .with_timezone(&chrono::Utc);

        for sequence in 0..12_u64 {
            let path = dir.join(build_tab_ai_screenshot_filename(now, 41231, sequence));
            std::fs::write(&path, b"png").expect("write screenshot");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let ignored = dir.join("keep-me.txt");
        std::fs::write(&ignored, b"keep").expect("write ignored file");

        cleanup_old_tab_ai_screenshot_files_in_dir(&dir, 10).expect("cleanup");

        let remaining_matching = std::fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with(TAB_AI_SCREENSHOT_PREFIX) && name.ends_with(".png")
            })
            .count();
        assert_eq!(remaining_matching, 10);
        assert!(ignored.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

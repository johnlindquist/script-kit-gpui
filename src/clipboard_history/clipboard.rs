//! Clipboard operations
//!
//! Functions for copying entries back to the system clipboard.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use arboard::Clipboard;
use rusqlite::params;
use tracing::{debug, info};

use super::blob_store::is_blob_content;
use super::cache::refresh_entry_cache;
use super::database::get_connection;
use super::image::decode_base64_image;
use super::macos_paste::copy_blob_with_file_url;
use super::types::ContentType;

/// Typed errors for clipboard write operations.
///
/// Callers can pattern-match on these to show distinct user-facing messages
/// and emit structured `error_code` fields in logs.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardWriteError {
    /// The requested entry ID does not exist in the clipboard history database.
    #[error("Clipboard entry not found: {id}")]
    EntryNotFound { id: String },

    /// Failed to acquire the database connection lock.
    #[error("Database lock error: {reason}")]
    LockError { reason: String },

    /// The system clipboard write (text or image) failed.
    #[error("Clipboard write failed: {reason}")]
    ClipboardWriteFailed { reason: String },
}

/// Flag to suppress clipboard monitor capture during programmatic writes.
///
/// When `true`, the monitor's `capture_clipboard_content` skips the current
/// poll cycle so that paste-sequential writes don't reorder history.
pub(crate) static SUPPRESS_CLIPBOARD_CAPTURE: AtomicBool = AtomicBool::new(false);

/// RAII guard that clears the suppression flag on drop (including panics).
pub(crate) struct SuppressGuard;

impl Drop for SuppressGuard {
    fn drop(&mut self) {
        SUPPRESS_CLIPBOARD_CAPTURE.store(false, Ordering::SeqCst);
        debug!("clipboard_capture_suppression_cleared");
    }
}

/// Copy an entry back to the clipboard
///
/// # Arguments
/// * `id` - The entry ID to copy
///
/// # Errors
/// Returns error if the entry doesn't exist or clipboard operation fails.
///
/// # Image Pasting Behavior (CleanShot-style)
///
/// For blob-format images (the current storage format), this function uses
/// CleanShot-style pasting which sets BOTH:
/// - Image data: for apps that accept images (Photoshop, Preview, etc.)
/// - File URL: for text fields, terminals, Claude Code, etc.
///
/// This allows pasting images to work naturally in any context - text fields
/// get the file path while image apps get the image data.
///
/// For legacy png: and rgba: formats, falls back to image-only pasting.
#[allow(dead_code)]
pub fn copy_entry_to_clipboard(id: &str) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    let conn = get_connection()?;
    let conn = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

    let (content, content_type): (String, String) = conn
        .query_row(
            "SELECT content, content_type FROM history WHERE id = ?",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .context("Entry not found")?;

    drop(conn); // Release lock before clipboard operation

    match ContentType::from_str(&content_type) {
        ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
            let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
            clipboard
                .set_text(&content)
                .context("Failed to set clipboard text")?;
        }
        ContentType::Image => {
            // Use CleanShot-style pasting for blob images (sets both image AND file URL)
            if is_blob_content(&content) {
                debug!("Using CleanShot-style paste for blob image");
                copy_blob_with_file_url(&content)
                    .context("Failed to copy blob image with file URL")?;
            } else {
                // Legacy png: and rgba: formats - use standard image-only paste
                debug!("Using legacy image-only paste for non-blob image");
                let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
                if let Some(image_data) = decode_base64_image(&content) {
                    clipboard
                        .set_image(image_data)
                        .context("Failed to set clipboard image")?;
                } else {
                    anyhow::bail!("Failed to decode image data");
                }
            }
        }
    }

    // Update timestamp to move entry to top
    let conn = get_connection()?;
    let conn = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
    let timestamp = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE history SET timestamp = ? WHERE id = ?",
        params![timestamp, id],
    )?;

    info!(id = %id, "Copied entry to clipboard");

    drop(conn);
    refresh_entry_cache();

    Ok(())
}

/// Write an entry to the system clipboard without any suppression management.
///
/// This is the raw clipboard-write logic extracted for use by callers that
/// manage `SUPPRESS_CLIPBOARD_CAPTURE` themselves (e.g. the serialized paste
/// worker). Does NOT update timestamps or refresh the cache.
///
/// Returns typed [`ClipboardWriteError`] so callers can distinguish
/// entry-not-found, lock failures, and clipboard API errors.
pub(crate) fn write_entry_to_system_clipboard(id: &str) -> Result<(), ClipboardWriteError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard).map_err(
        |error| ClipboardWriteError::ClipboardWriteFailed {
            reason: error.to_string(),
        },
    )?;
    let conn = get_connection().map_err(|e| ClipboardWriteError::LockError {
        reason: format!("Failed to get DB connection: {e}"),
    })?;
    let conn = conn.lock().map_err(|e| ClipboardWriteError::LockError {
        reason: e.to_string(),
    })?;

    let (content, content_type): (String, String) = conn
        .query_row(
            "SELECT content, content_type FROM history WHERE id = ?",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| ClipboardWriteError::EntryNotFound { id: id.to_string() })?;

    drop(conn); // Release lock before clipboard operation

    match ContentType::from_str(&content_type) {
        ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
            let mut clipboard =
                Clipboard::new().map_err(|e| ClipboardWriteError::ClipboardWriteFailed {
                    reason: format!("Failed to access clipboard: {e}"),
                })?;
            clipboard.set_text(&content).map_err(|e| {
                ClipboardWriteError::ClipboardWriteFailed {
                    reason: format!("Failed to set clipboard text: {e}"),
                }
            })?;
        }
        ContentType::Image => {
            if is_blob_content(&content) {
                debug!(id = %id, "Using CleanShot-style paste for blob image (no reorder)");
                copy_blob_with_file_url(&content).map_err(|e| {
                    ClipboardWriteError::ClipboardWriteFailed {
                        reason: format!("Failed to copy blob image with file URL: {e}"),
                    }
                })?;
            } else {
                debug!(id = %id, "Using legacy image-only paste for non-blob image (no reorder)");
                let mut clipboard =
                    Clipboard::new().map_err(|e| ClipboardWriteError::ClipboardWriteFailed {
                        reason: format!("Failed to access clipboard: {e}"),
                    })?;
                if let Some(image_data) = decode_base64_image(&content) {
                    clipboard.set_image(image_data).map_err(|e| {
                        ClipboardWriteError::ClipboardWriteFailed {
                            reason: format!("Failed to set clipboard image: {e}"),
                        }
                    })?;
                } else {
                    return Err(ClipboardWriteError::ClipboardWriteFailed {
                        reason: "Failed to decode image data".to_string(),
                    });
                }
            }
        }
    }

    info!(id = %id, "Wrote entry to system clipboard");
    Ok(())
}

/// Copy an entry to the clipboard without reordering history
///
/// Identical to [`copy_entry_to_clipboard`] but does NOT update the entry's
/// timestamp and does NOT refresh the entry cache. This preserves the current
/// history order — intended for sequential paste where advancing through
/// entries must not shuffle the snapshot.
///
/// # Arguments
/// * `id` - The entry ID to copy
///
/// # Errors
/// Returns error if the entry doesn't exist or clipboard operation fails.
#[allow(dead_code)]
pub fn copy_entry_to_clipboard_no_reorder(id: &str) -> Result<(), ClipboardWriteError> {
    // Suppress monitor capture so this write doesn't reorder history.
    // The guard clears the flag on drop — even if a panic occurs below.
    SUPPRESS_CLIPBOARD_CAPTURE.store(true, Ordering::SeqCst);
    let _guard = SuppressGuard;
    debug!(id = %id, "clipboard_capture_suppression_set");

    write_entry_to_system_clipboard(id)?;

    info!(id = %id, "Copied entry to clipboard without reorder");

    // _guard drops here, clearing SUPPRESS_CLIPBOARD_CAPTURE
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_suppress_flag_defaults_to_false() {
        // Ensure no prior test left the flag set
        SUPPRESS_CLIPBOARD_CAPTURE.store(false, Ordering::SeqCst);
        assert!(
            !SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst),
            "SUPPRESS_CLIPBOARD_CAPTURE should default to false"
        );
    }

    #[test]
    fn test_suppress_guard_clears_flag_on_drop() {
        SUPPRESS_CLIPBOARD_CAPTURE.store(true, Ordering::SeqCst);
        {
            let _guard = SuppressGuard;
            assert!(
                SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst),
                "Flag should remain true while guard is alive"
            );
        }
        // Guard dropped
        assert!(
            !SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst),
            "Flag should be cleared after guard drops"
        );
    }

    #[test]
    fn test_suppress_guard_clears_flag_on_early_scope_exit() {
        // Simulates the error-path: guard is created, then an error causes
        // the scope to exit early. The RAII guard must still clear the flag.
        SUPPRESS_CLIPBOARD_CAPTURE.store(false, Ordering::SeqCst);

        let result: Result<()> = (|| {
            SUPPRESS_CLIPBOARD_CAPTURE.store(true, Ordering::SeqCst);
            let _guard = SuppressGuard;

            assert!(
                SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst),
                "Flag should be true inside guarded scope"
            );

            // Simulate an error that causes early return via `?`
            anyhow::bail!("simulated clipboard error");
        })();

        assert!(result.is_err(), "Closure should have returned an error");
        assert!(
            !SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst),
            "Flag must be cleared even when scope exits via error"
        );
    }

    #[test]
    fn test_suppress_flag_set_and_cleared_round_trip() {
        SUPPRESS_CLIPBOARD_CAPTURE.store(false, Ordering::SeqCst);
        assert!(!SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst));

        SUPPRESS_CLIPBOARD_CAPTURE.store(true, Ordering::SeqCst);
        assert!(SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst));

        let _guard = SuppressGuard;
        drop(_guard);
        assert!(
            !SUPPRESS_CLIPBOARD_CAPTURE.load(Ordering::SeqCst),
            "SuppressGuard drop must clear the flag"
        );
    }
}

//! File-based blob storage for clipboard images
//!
//! Stores image data as PNG files on disk instead of base64 in SQLite.
//! This reduces SQLite WAL churn and eliminates 33% base64 overhead.
//!
//! Storage location: ~/.scriptkit/clipboard/blobs/<hash>.png
//! Content format in DB: "blob:<hash>" (replaces "png:<base64>")

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, error, warn};

/// Get the blob storage directory path
pub fn get_blob_dir() -> Result<PathBuf> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    let kit_dir = PathBuf::from(shellexpand::tilde("~/.scriptkit").as_ref());
    let blob_dir = kit_dir.join("clipboard").join("blobs");
    if !blob_dir.exists() {
        fs::create_dir_all(&blob_dir).context("Failed to create blob storage directory")?;
    }
    Ok(blob_dir)
}

/// Compute SHA-256 hash of PNG bytes (hex-encoded)
pub fn compute_blob_hash(png_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(png_bytes);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Store PNG bytes as a blob file
///
/// Returns the content reference string: "blob:<hash>"
/// The PNG bytes are stored at ~/.scriptkit/clipboard/blobs/<hash>.png
pub fn store_blob(png_bytes: &[u8]) -> Result<String> {
    let blob_dir = get_blob_dir()?;
    store_blob_in_dir(png_bytes, &blob_dir)
}

fn store_blob_in_dir(png_bytes: &[u8], blob_dir: &Path) -> Result<String> {
    let hash = compute_blob_hash(png_bytes);
    let blob_path = blob_dir.join(format!("{}.png", hash));

    if blob_path.exists() {
        debug!(hash = %hash, "Blob already exists, skipping write");
        return Ok(format!("blob:{}", hash));
    }

    let mut temp_file = tempfile::Builder::new()
        .prefix(&format!("{}.tmp.", hash))
        .suffix(".png")
        .tempfile_in(blob_dir)
        .with_context(|| {
            format!(
                "Failed to create temporary blob file in {} for hash {}",
                blob_dir.display(),
                hash
            )
        })?;

    temp_file.write_all(png_bytes).with_context(|| {
        format!(
            "Failed to write PNG bytes to temporary blob file {} (hash {})",
            temp_file.path().display(),
            hash
        )
    })?;
    temp_file.as_file_mut().sync_all().with_context(|| {
        format!(
            "Failed to sync temporary blob file {} (hash {})",
            temp_file.path().display(),
            hash
        )
    })?;

    match temp_file.persist_noclobber(&blob_path) {
        Ok(_) => {
            debug!(
                hash = %hash,
                size = png_bytes.len(),
                path = %blob_path.display(),
                "Stored new blob atomically"
            );
        }
        Err(persist_error) if persist_error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = persist_error.file.close();
            debug!(
                hash = %hash,
                path = %blob_path.display(),
                "Blob already exists, discarded temporary blob file"
            );
        }
        Err(persist_error) => {
            let temp_path = persist_error.file.path().to_path_buf();
            let rename_error = persist_error.error;
            let _ = persist_error.file.close();
            return Err(rename_error).with_context(|| {
                format!(
                    "Failed to atomically rename temporary blob {} to destination {} (hash {})",
                    temp_path.display(),
                    blob_path.display(),
                    hash
                )
            });
        }
    }

    Ok(format!("blob:{}", hash))
}

/// Load PNG bytes from a blob file
///
/// Input: "blob:<hash>" content reference
/// Returns: PNG bytes or None if not found
pub fn load_blob(content: &str) -> Option<Vec<u8>> {
    let hash = content.strip_prefix("blob:")?;

    let blob_dir = match get_blob_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error!(error = %e, "Failed to get blob directory");
            return None;
        }
    };

    let blob_path = blob_dir.join(format!("{}.png", hash));

    match fs::read(&blob_path) {
        Ok(bytes) => {
            debug!(hash = %hash, size = bytes.len(), "Loaded blob from disk");
            Some(bytes)
        }
        Err(e) => {
            warn!(hash = %hash, error = %e, "Failed to read blob file");
            None
        }
    }
}

/// Delete a blob file by its content reference
///
/// Input: "blob:<hash>" content reference
/// Returns: true if deleted, false if not found or error
#[allow(dead_code)] // Used for maintenance/cleanup operations
pub fn delete_blob(content: &str) -> bool {
    let Some(hash) = content.strip_prefix("blob:") else {
        return false;
    };

    let blob_dir = match get_blob_dir() {
        Ok(dir) => dir,
        Err(_) => return false,
    };

    let blob_path = blob_dir.join(format!("{}.png", hash));

    match fs::remove_file(&blob_path) {
        Ok(_) => {
            debug!(hash = %hash, "Deleted blob file");
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!(hash = %hash, "Blob file not found, nothing to delete");
            false
        }
        Err(e) => {
            warn!(hash = %hash, error = %e, "Failed to delete blob file");
            false
        }
    }
}

/// Check if a content string is a blob reference
#[inline]
pub fn is_blob_content(content: &str) -> bool {
    content.starts_with("blob:")
}

/// Garbage collect orphaned blob files
///
/// Takes a set of valid hashes (from database entries) and removes
/// any blob files not in that set.
#[allow(dead_code)] // Used for maintenance/cleanup operations
pub fn gc_orphaned_blobs(valid_hashes: &std::collections::HashSet<String>) -> Result<usize> {
    let blob_dir = get_blob_dir()?;
    let mut deleted = 0;

    let entries = fs::read_dir(&blob_dir).context("Failed to read blob directory")?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "png") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !valid_hashes.contains(stem) && fs::remove_file(&path).is_ok() {
                    debug!(hash = %stem, "GC'd orphaned blob");
                    deleted += 1;
                }
            }
        }
    }

    if deleted > 0 {
        debug!(deleted, "Garbage collected orphaned blobs");
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_blob_hash() {
        let data = b"test png data";
        let hash1 = compute_blob_hash(data);
        let hash2 = compute_blob_hash(data);
        assert_eq!(hash1, hash2, "Hash should be deterministic");
        assert_eq!(hash1.len(), 64, "SHA-256 hash should be 64 hex chars");
    }

    #[test]
    fn test_is_blob_content() {
        assert!(is_blob_content("blob:abc123"));
        assert!(!is_blob_content("png:somebase64"));
        assert!(!is_blob_content("rgba:100:100:data"));
    }

    #[test]
    fn test_store_blob_in_dir_is_idempotent_when_same_bytes_stored_twice() {
        let temp_dir = tempfile::tempdir().expect("Should create temp dir");
        let png_bytes = b"test png bytes";

        let first_ref =
            store_blob_in_dir(png_bytes, temp_dir.path()).expect("First store succeeds");
        let second_ref =
            store_blob_in_dir(png_bytes, temp_dir.path()).expect("Second store succeeds");
        assert_eq!(first_ref, second_ref, "Content refs should match");

        let hash = first_ref
            .strip_prefix("blob:")
            .expect("Expected blob prefix");
        let blob_path = temp_dir.path().join(format!("{hash}.png"));
        let stored = fs::read(&blob_path).expect("Blob file should be readable");
        assert_eq!(stored, png_bytes, "Stored bytes should match source");

        let file_count = fs::read_dir(temp_dir.path())
            .expect("Should read temp dir")
            .count();
        assert_eq!(file_count, 1, "Only one blob file should exist");
    }

    #[test]
    fn test_store_blob_in_dir_keeps_existing_destination_when_hash_path_exists() {
        let temp_dir = tempfile::tempdir().expect("Should create temp dir");
        let png_bytes = b"test png bytes";
        let hash = compute_blob_hash(png_bytes);
        let blob_path = temp_dir.path().join(format!("{hash}.png"));
        fs::write(&blob_path, b"preexisting").expect("Should seed destination file");

        let blob_ref = store_blob_in_dir(png_bytes, temp_dir.path()).expect("Store should be ok");
        assert_eq!(blob_ref, format!("blob:{hash}"));
        assert_eq!(
            fs::read(&blob_path).expect("Should read destination"),
            b"preexisting",
            "Existing destination should not be overwritten"
        );

        let file_count = fs::read_dir(temp_dir.path())
            .expect("Should read temp dir")
            .count();
        assert_eq!(file_count, 1, "Temporary files should be cleaned up");
    }
}

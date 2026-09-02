//! Temporary file helpers for clipboard history entries.
//!
//! Converts clipboard entries into temp files that can be opened by other apps.

use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::path::PathBuf;
use tempfile::Builder;

use super::image::content_to_png_bytes;
use super::types::{ClipboardEntry, ContentType};

/// Save a clipboard entry to a temp file and return its path.
#[allow(dead_code)]
pub fn save_entry_to_temp_file(entry: &ClipboardEntry) -> Result<PathBuf> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ExternalStorage)?;
    match entry.content_type {
        ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
            let mut temp_file = Builder::new()
                .prefix("script-kit-clipboard-")
                .suffix(".txt")
                .tempfile()
                .context("Failed to create temp file for clipboard text")?;

            temp_file
                .write_all(entry.content.as_bytes())
                .context("Failed to write clipboard text to temp file")?;
            temp_file
                .flush()
                .context("Failed to flush clipboard text temp file")?;

            let (_file, path) = temp_file
                .keep()
                .context("Failed to persist clipboard text temp file")?;
            Ok(path)
        }
        ContentType::Image => {
            let png_bytes = content_to_png_bytes(&entry.content)
                .ok_or_else(|| anyhow!("Failed to decode clipboard image content"))?;

            let mut temp_file = Builder::new()
                .prefix("script-kit-clipboard-")
                .suffix(".png")
                .tempfile()
                .context("Failed to create temp file for clipboard image")?;

            temp_file
                .write_all(&png_bytes)
                .context("Failed to write clipboard image to temp file")?;
            temp_file
                .flush()
                .context("Failed to flush clipboard image temp file")?;

            let (_file, path) = temp_file
                .keep()
                .context("Failed to persist clipboard image temp file")?;
            Ok(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::image::encode_image_as_png;
    use super::super::types::{ClipboardEntry, ContentType};
    use super::save_entry_to_temp_file;

    #[test]
    fn test_save_text_entry_to_temp_file() {
        let entry = ClipboardEntry {
            id: "test-text".to_string(),
            content: "Hello, clipboard!".to_string(),
            content_type: ContentType::Text,
            timestamp: 0,
            pinned: false,
            source_app_name: None,
            source_app_bundle_id: None,
            ocr_text: None,
        };

        let path = save_entry_to_temp_file(&entry).expect("should create temp file");
        assert_eq!(path.extension().and_then(|s| s.to_str()), Some("txt"));

        let contents = std::fs::read_to_string(&path).expect("should read temp file");
        assert_eq!(contents, entry.content);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_image_entry_to_temp_file() {
        let image = arboard::ImageData {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0, 255].into(),
        };
        let content = encode_image_as_png(&image).expect("should encode png");

        let entry = ClipboardEntry {
            id: "test-image".to_string(),
            content,
            content_type: ContentType::Image,
            timestamp: 0,
            pinned: false,
            source_app_name: None,
            source_app_bundle_id: None,
            ocr_text: None,
        };

        let path = save_entry_to_temp_file(&entry).expect("should create temp image file");
        assert_eq!(path.extension().and_then(|s| s.to_str()), Some("png"));

        let bytes = std::fs::read(&path).expect("should read image file");
        assert!(bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));

        let _ = std::fs::remove_file(&path);
    }
}

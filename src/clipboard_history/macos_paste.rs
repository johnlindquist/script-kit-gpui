//! macOS-specific clipboard paste operations
//!
//! Provides CleanShot-style clipboard pasting where BOTH image data AND file URL
//! are placed on the pasteboard. This allows:
//! - Text fields to receive the file path (for Claude Code, terminals, etc.)
//! - Image-accepting apps to receive the image data (for Photoshop, Preview, etc.)

#[cfg(target_os = "macos")]
use cocoa::appkit::NSPasteboard;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSArray, NSData, NSString, NSURL};
#[cfg(target_os = "macos")]
use objc::runtime::Class;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

use anyhow::{Context, Result};
use std::path::Path;
use tracing::debug;

/// Copy an image to the clipboard with BOTH image data and file URL representations.
///
/// This mimics CleanShot's behavior where:
/// - Pasting in a text field gives you the file path
/// - Pasting in an image app gives you the image data
///
/// # Arguments
/// * `png_bytes` - The PNG image data
/// * `file_path` - The path to the PNG file on disk
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` with description on failure
#[cfg(target_os = "macos")]
pub fn copy_image_with_file_url(png_bytes: &[u8], file_path: &Path) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    // SAFETY: All ObjC objects (pasteboard, data, image, url) are nil-checked after
    // creation. NSData is created from a valid slice. NSImage/NSURL are initialized
    // from the validated data/path. Pasteboard writeObjects uses Foundation's copy semantics.
    unsafe {
        // Get the general pasteboard
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        if pasteboard.is_null() {
            anyhow::bail!("Failed to get general pasteboard");
        }

        // Clear the pasteboard and get a new change count
        let _: i64 = msg_send![pasteboard, clearContents];

        // Create NSImage from PNG data
        let data: id = NSData::dataWithBytes_length_(
            nil,
            png_bytes.as_ptr() as *const std::ffi::c_void,
            png_bytes.len() as u64,
        );
        if data.is_null() {
            anyhow::bail!("Failed to create NSData from PNG bytes");
        }

        let nsimage_class = Class::get("NSImage").context("Failed to get NSImage class")?;
        let image: id = msg_send![nsimage_class, alloc];
        let image: id = msg_send![image, initWithData: data];
        if image.is_null() {
            anyhow::bail!("Failed to create NSImage from PNG data");
        }

        // Create NSURL from file path
        let path_str = file_path.to_str().context("File path is not valid UTF-8")?;
        let ns_string: id = NSString::alloc(nil).init_str(path_str);
        if ns_string.is_null() {
            let _: () = msg_send![image, release];
            anyhow::bail!("Failed to create NSString from path");
        }

        let url: id = NSURL::fileURLWithPath_(nil, ns_string);
        if url.is_null() {
            let _: () = msg_send![image, release];
            anyhow::bail!("Failed to create NSURL from path");
        }

        // Create an array containing both the image and the URL
        // The order matters: first item is preferred representation
        // We put the image first so image apps get the image, but file URL is also available
        let objects: id = NSArray::arrayWithObjects(nil, &[image, url]);
        if objects.is_null() {
            let _: () = msg_send![image, release];
            anyhow::bail!("Failed to create NSArray for writeObjects");
        }

        // Write both objects to the pasteboard
        let success: bool = msg_send![pasteboard, writeObjects: objects];

        // Release the image (NSArray retains it, but we need to release our reference)
        let _: () = msg_send![image, release];

        if success {
            debug!(
                path = %file_path.display(),
                png_size = png_bytes.len(),
                "Copied image with file URL to clipboard (CleanShot-style)"
            );
            Ok(())
        } else {
            anyhow::bail!("NSPasteboard writeObjects returned false")
        }
    }
}

/// Fallback for non-macOS: not supported
#[cfg(not(target_os = "macos"))]
pub fn copy_image_with_file_url(_png_bytes: &[u8], _file_path: &Path) -> Result<()> {
    anyhow::bail!("CleanShot-style clipboard paste is only supported on macOS")
}

/// Copy a blob image to the clipboard with both image data and file URL.
///
/// This is a convenience wrapper that loads the blob and calls copy_image_with_file_url.
///
/// # Arguments
/// * `blob_content` - The blob reference string (e.g., "blob:abc123...")
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` with description on failure
pub fn copy_blob_with_file_url(blob_content: &str) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    use super::blob_store::{get_blob_dir, is_blob_content, load_blob};

    if !is_blob_content(blob_content) {
        anyhow::bail!("Content is not a blob reference: {}", blob_content);
    }

    // Load the PNG bytes
    let png_bytes = load_blob(blob_content).context("Failed to load blob from disk")?;

    // Get the file path
    let hash = blob_content
        .strip_prefix("blob:")
        .context("Invalid blob format")?;
    let blob_dir = get_blob_dir()?;
    let file_path = blob_dir.join(format!("{}.png", hash));

    if !file_path.exists() {
        anyhow::bail!("Blob file does not exist: {}", file_path.display());
    }

    copy_image_with_file_url(&png_bytes, &file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires NSPasteboard access, unavailable in CI"]
    fn test_copy_image_with_file_url_requires_valid_png() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a minimal valid PNG
        let png_bytes = create_minimal_png();

        // Create a temp file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(&png_bytes)
            .expect("Failed to write PNG");
        let path = temp_file.path();

        // This should succeed
        let result = copy_image_with_file_url(&png_bytes, path);
        assert!(
            result.is_ok(),
            "Should succeed with valid PNG: {:?}",
            result
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_copy_image_with_invalid_data_fails() {
        use tempfile::NamedTempFile;

        // Invalid PNG data
        let invalid_bytes = b"not a png";

        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let path = temp_file.path();

        // This should fail because NSImage can't parse invalid data
        let result = copy_image_with_file_url(invalid_bytes, path);
        assert!(result.is_err(), "Should fail with invalid PNG data");
    }

    /// Create a minimal valid 1x1 red PNG for testing
    #[cfg(test)]
    fn create_minimal_png() -> Vec<u8> {
        use std::io::Cursor;

        let mut rgba = image::RgbaImage::new(1, 1);
        rgba.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));

        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        rgba.write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to encode PNG");

        png_data
    }
}

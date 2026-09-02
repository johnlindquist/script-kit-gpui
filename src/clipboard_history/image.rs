//! Clipboard image encoding and decoding
//!
//! Handles base64 encoding/decoding of clipboard images, including
//! PNG compression and legacy RGBA format support.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use gpui::RenderImage;
use smallvec::smallvec;
use std::sync::Arc;
use tracing::{debug, warn};

use super::blob_store::{is_blob_content, load_blob, store_blob};

const MAX_RENDER_IMAGE_PIXELS: u64 = 20_000_000;

/// Encode image data as a blob file (PNG stored on disk)
///
/// Format: "blob:{hash}" where hash is SHA-256 of PNG bytes
/// The PNG file is stored at ~/.scriptkit/clipboard/blobs/<hash>.png
///
/// This is the most efficient format:
/// - No base64 overhead (saves 33%)
/// - No SQLite WAL churn for large images
/// - Content-addressed deduplication
pub fn encode_image_as_blob(image: &arboard::ImageData) -> Result<String> {
    let png_bytes = encode_image_to_png_bytes(image)?;
    store_blob(&png_bytes)
}

/// Encode image data as base64 PNG string (compressed, ~90% smaller than raw RGBA)
///
/// Format: "png:{base64_encoded_png_data}"
/// The PNG format is detected by the "png:" prefix for decoding.
///
/// NOTE: For new images, prefer encode_image_as_blob() which avoids base64 overhead.
/// This function is kept for backwards compatibility.
#[allow(dead_code)] // Kept for backwards compatibility with existing clipboard entries
pub fn encode_image_as_png(image: &arboard::ImageData) -> Result<String> {
    let png_bytes = encode_image_to_png_bytes(image)?;
    let base64_data = BASE64.encode(&png_bytes);
    Ok(format!("png:{}", base64_data))
}

/// Internal helper to encode image to PNG bytes
fn encode_image_to_png_bytes(image: &arboard::ImageData) -> Result<Vec<u8>> {
    use std::io::Cursor;

    let width = u32::try_from(image.width).context("Clipboard image width exceeds u32")?;
    let height = u32::try_from(image.height).context("Clipboard image height exceeds u32")?;

    // Create an RgbaImage from the raw bytes
    let rgba_image = image::RgbaImage::from_raw(width, height, image.bytes.to_vec())
        .context("Failed to create RGBA image from clipboard data")?;

    // Encode to PNG in memory
    let mut png_data = Vec::new();
    let mut cursor = Cursor::new(&mut png_data);
    rgba_image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .context("Failed to encode image as PNG")?;

    Ok(png_data)
}

/// Encode image data as base64 raw RGBA string (legacy format, kept for compatibility)
///
/// Format: "rgba:{width}:{height}:{base64_data}"
/// This is the old format - new code should use encode_image_as_png() instead.
#[allow(dead_code)] // Kept for backward compatibility if needed
pub fn encode_image_as_base64(image: &arboard::ImageData) -> Result<String> {
    let base64_data = BASE64.encode(&image.bytes);
    Ok(format!(
        "rgba:{}:{}:{}",
        image.width, image.height, base64_data
    ))
}

/// Decode a clipboard image content string back to ImageData
///
/// Supports three formats:
/// - Blob format: "blob:{hash}" (file-based, most efficient)
/// - PNG format: "png:{base64_encoded_png_data}"
/// - Legacy RGBA format: "rgba:{width}:{height}:{base64_data}"
#[allow(dead_code)]
pub fn decode_base64_image(content: &str) -> Option<arboard::ImageData<'static>> {
    if is_blob_content(content) {
        decode_blob_to_image_data(content)
    } else if content.starts_with("png:") {
        decode_png_to_image_data(content)
    } else if content.starts_with("rgba:") {
        decode_legacy_rgba(content)
    } else {
        warn!("Unknown clipboard image format prefix");
        None
    }
}

/// Decode blob format: "blob:{hash}" -> ImageData for clipboard
fn decode_blob_to_image_data(content: &str) -> Option<arboard::ImageData<'static>> {
    let png_bytes = load_blob(content)?;

    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let width: usize = rgba.width().try_into().ok()?;
    let height: usize = rgba.height().try_into().ok()?;

    Some(arboard::ImageData {
        width,
        height,
        bytes: rgba.into_raw().into(),
    })
}

/// Convert clipboard content into PNG bytes.
///
/// Supports:
/// - blob:{hash} -> load PNG bytes from blob store
/// - png:{base64} -> decode base64 PNG bytes directly
/// - rgba:{width}:{height}:{base64} -> decode raw RGBA, then encode to PNG
pub fn content_to_png_bytes(content: &str) -> Option<Vec<u8>> {
    if is_blob_content(content) {
        return load_blob(content);
    }

    if let Some(base64_data) = content.strip_prefix("png:") {
        return BASE64.decode(base64_data).ok();
    }

    if content.starts_with("rgba:") {
        let image = decode_legacy_rgba(content)?;
        return encode_image_to_png_bytes(&image).ok();
    }

    warn!("Unknown clipboard image format prefix for PNG conversion");
    None
}

/// Decode PNG format: "png:{base64_encoded_png_data}"
fn decode_png_to_image_data(content: &str) -> Option<arboard::ImageData<'static>> {
    let base64_data = content.strip_prefix("png:")?;
    let png_bytes = BASE64.decode(base64_data).ok()?;

    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let width: usize = rgba.width().try_into().ok()?;
    let height: usize = rgba.height().try_into().ok()?;

    Some(arboard::ImageData {
        width,
        height,
        bytes: rgba.into_raw().into(),
    })
}

/// Decode legacy RGBA format: "rgba:{width}:{height}:{base64_data}"
fn decode_legacy_rgba(content: &str) -> Option<arboard::ImageData<'static>> {
    let parts: Vec<&str> = content.splitn(4, ':').collect();
    if parts.len() != 4 || parts[0] != "rgba" {
        return None;
    }

    let width: usize = parts[1].parse().ok()?;
    let height: usize = parts[2].parse().ok()?;
    let bytes = BASE64.decode(parts[3]).ok()?;

    // Validate byte length with overflow-safe math
    // expected = width * height * 4 (RGBA = 4 bytes per pixel)
    let expected = width.checked_mul(height)?.checked_mul(4)?;

    if bytes.len() != expected {
        warn!(
            width,
            height,
            expected,
            actual = bytes.len(),
            "Legacy RGBA image byte length mismatch"
        );
        return None;
    }

    Some(arboard::ImageData {
        width,
        height,
        bytes: bytes.into(),
    })
}

/// Decode a clipboard image content string to GPUI RenderImage
///
/// Supports three formats:
/// - Blob format: "blob:{hash}" (file-based, most efficient)
/// - PNG format: "png:{base64_encoded_png_data}"
/// - Legacy RGBA format: "rgba:{width}:{height}:{base64_data}"
///
/// Returns an Arc<RenderImage> for efficient caching.
///
/// **IMPORTANT**: Call this ONCE per entry and cache the result. Do NOT
/// decode during rendering as this is expensive.
pub fn decode_to_render_image(content: &str) -> Option<Arc<RenderImage>> {
    if is_blob_content(content) {
        decode_blob_to_render_image(content)
    } else if content.starts_with("png:") {
        decode_png_to_render_image(content)
    } else if content.starts_with("rgba:") {
        decode_rgba_to_render_image(content)
    } else {
        warn!("Invalid clipboard image format, expected blob:, png: or rgba: prefix");
        None
    }
}

fn ensure_dimensions_within_limit(width: u32, height: u32, format: &str) -> Option<(u32, u32)> {
    let pixel_count = u64::from(width).checked_mul(u64::from(height))?;

    if pixel_count > MAX_RENDER_IMAGE_PIXELS {
        warn!(
            width,
            height,
            pixel_count,
            max_pixels = MAX_RENDER_IMAGE_PIXELS,
            format,
            "Rejecting oversized clipboard image for RenderImage decode"
        );
        return None;
    }

    Some((width, height))
}

fn decode_png_bytes_to_render_image_with_format(
    png: &[u8],
    format: &'static str,
) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    let img_width = rgba.width();
    let img_height = rgba.height();

    // Convert RGBA to BGRA for Metal/GPUI (swap R and B channels)
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }

    let frame = image::Frame::new(rgba);
    // Use smallvec! macro to avoid cloning the frame buffer
    // (SmallVec::from_elem clones the element, which would duplicate megabytes of pixel data)
    let render_image = RenderImage::new(smallvec![frame]);

    debug!(
        width = img_width,
        height = img_height,
        format,
        "Decoded clipboard image to RenderImage"
    );
    Some(Arc::new(render_image))
}

fn decode_png_bytes_to_render_image(png: &[u8]) -> Option<Arc<RenderImage>> {
    decode_png_bytes_to_render_image_with_format(png, "png")
}

/// Decode blob format to RenderImage
fn decode_blob_to_render_image(content: &str) -> Option<Arc<RenderImage>> {
    let png_bytes = load_blob(content)?;
    let (width, height) = png_dims_from_bytes(&png_bytes)?;
    ensure_dimensions_within_limit(width, height, "blob")?;
    decode_png_bytes_to_render_image_with_format(&png_bytes, "blob")
}

/// Decode PNG format to RenderImage
fn decode_png_to_render_image(content: &str) -> Option<Arc<RenderImage>> {
    let base64_data = content.strip_prefix("png:")?;
    let png_bytes = BASE64.decode(base64_data).ok()?;
    let (width, height) = png_dims_from_bytes(&png_bytes)?;
    ensure_dimensions_within_limit(width, height, "png")?;
    decode_png_bytes_to_render_image(&png_bytes)
}

/// Decode legacy RGBA format to RenderImage
fn decode_rgba_to_render_image(content: &str) -> Option<Arc<RenderImage>> {
    let parts: Vec<&str> = content.splitn(4, ':').collect();
    if parts.len() != 4 || parts[0] != "rgba" {
        warn!("Invalid clipboard image format, expected rgba:W:H:data");
        return None;
    }

    let width: u32 = parts[1].parse().ok()?;
    let height: u32 = parts[2].parse().ok()?;
    let mut rgba_bytes = BASE64.decode(parts[3]).ok()?;

    let expected_bytes: usize = u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(4)?
        .try_into()
        .ok()?;
    if rgba_bytes.len() != expected_bytes {
        warn!(
            "Clipboard image byte count mismatch: expected {}, got {}",
            expected_bytes,
            rgba_bytes.len()
        );
        return None;
    }

    // Convert RGBA to BGRA for Metal/GPUI (swap R and B channels)
    for pixel in rgba_bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }

    let rgba_image = image::RgbaImage::from_raw(width, height, rgba_bytes)?;
    let frame = image::Frame::new(rgba_image);
    // Use smallvec! macro to avoid cloning the frame buffer
    // (SmallVec::from_elem clones the element, which would duplicate megabytes of pixel data)
    let render_image = RenderImage::new(smallvec![frame]);

    debug!(
        width,
        height,
        format = "rgba",
        "Decoded clipboard image to RenderImage"
    );
    Some(Arc::new(render_image))
}

/// Get image dimensions from content string without fully decoding
///
/// Returns (width, height) if the content is a valid image format.
/// For blob format, reads PNG header from file to extract dimensions.
/// For PNG format, reads PNG header to extract dimensions (fast, no full decode).
/// For legacy RGBA format, parses dimensions from metadata prefix.
pub fn get_image_dimensions(content: &str) -> Option<(u32, u32)> {
    if is_blob_content(content) {
        get_blob_dimensions(content)
    } else if content.starts_with("png:") {
        get_png_dimensions(content)
    } else if content.starts_with("rgba:") {
        let parts: Vec<&str> = content.splitn(4, ':').collect();
        if parts.len() >= 4 {
            let width: u32 = parts[1].parse().ok()?;
            let height: u32 = parts[2].parse().ok()?;
            Some((width, height))
        } else {
            None
        }
    } else {
        None
    }
}

/// Extract dimensions from blob file without full decode
fn get_blob_dimensions(content: &str) -> Option<(u32, u32)> {
    let png_bytes = load_blob(content)?;
    png_dims_from_bytes(&png_bytes)
}

/// Extract dimensions from PNG header using fast header-only parsing
///
/// PNG structure:
/// - Bytes 0-7: Signature \x89PNG\r\n\x1a\n
/// - Bytes 8-11: IHDR chunk length (always 13)
/// - Bytes 12-15: "IHDR"
/// - Bytes 16-19: Width (big-endian u32)
/// - Bytes 20-23: Height (big-endian u32)
///
/// We only need to decode 24 PNG bytes = 32 base64 chars.
fn get_png_dimensions(content: &str) -> Option<(u32, u32)> {
    let base64_data = content.strip_prefix("png:")?;

    // Try fast header-only parsing first (only decode 32 base64 chars = 24 PNG bytes)
    if let Some(dims) = get_png_dimensions_fast(base64_data) {
        return Some(dims);
    }

    // Fallback: decode entire PNG and use image crate (handles edge cases)
    let png_bytes = BASE64.decode(base64_data).ok()?;
    png_dims_from_bytes(&png_bytes)
}

fn png_dims_from_bytes(png: &[u8]) -> Option<(u32, u32)> {
    let cursor = std::io::Cursor::new(png);
    let reader = image::ImageReader::with_format(cursor, image::ImageFormat::Png);
    reader.into_dimensions().ok()
}

/// Fast PNG dimension extraction from base64 data (header only)
///
/// Decodes only the first 32 base64 characters (24 bytes) to read PNG header.
/// Returns None if header is invalid, triggering fallback to full decode.
fn get_png_dimensions_fast(base64_data: &str) -> Option<(u32, u32)> {
    // Need at least 32 base64 chars to get 24 PNG bytes
    if base64_data.len() < 32 {
        return None;
    }

    // Decode only the first 32 base64 chars
    let header_b64 = &base64_data[..32];
    let header = BASE64.decode(header_b64).ok()?;

    if header.len() < 24 {
        return None;
    }

    // Verify PNG signature: \x89PNG\r\n\x1a\n
    const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if header[0..8] != PNG_SIGNATURE {
        return None;
    }

    // Verify IHDR chunk type at bytes 12-15
    if &header[12..16] != b"IHDR" {
        return None;
    }

    // Extract width (bytes 16-19) and height (bytes 20-23) as big-endian u32
    let width = u32::from_be_bytes([header[16], header[17], header[18], header[19]]);
    let height = u32::from_be_bytes([header[20], header[21], header[22], header[23]]);

    // Sanity check dimensions (reject obviously invalid values)
    if width == 0 || height == 0 || width > 65535 || height > 65535 {
        return None;
    }

    Some((width, height))
}

/// Compute a simple hash of image data for change detection
pub fn compute_image_hash(image: &arboard::ImageData) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    image.width.hash(&mut hasher);
    image.height.hash(&mut hasher);

    // Hash first 1KB of pixels for quick comparison
    let sample_size = 1024.min(image.bytes.len());
    image.bytes[..sample_size].hash(&mut hasher);

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_hash_deterministic() {
        let image = arboard::ImageData {
            width: 100,
            height: 100,
            bytes: vec![0u8; 40000].into(),
        };

        let hash1 = compute_image_hash(&image);
        let hash2 = compute_image_hash(&image);
        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_base64_image_roundtrip_legacy() {
        let original = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ]
            .into(),
        };

        let encoded = encode_image_as_base64(&original).expect("Should encode");
        assert!(
            encoded.starts_with("rgba:"),
            "Legacy format should have rgba: prefix"
        );
        let decoded = decode_base64_image(&encoded).expect("Should decode");

        assert_eq!(original.width, decoded.width);
        assert_eq!(original.height, decoded.height);
        assert_eq!(original.bytes.as_ref(), decoded.bytes.as_ref());
    }

    #[test]
    fn test_png_image_roundtrip() {
        let original = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ]
            .into(),
        };

        let encoded = encode_image_as_png(&original).expect("Should encode as PNG");
        assert!(
            encoded.starts_with("png:"),
            "PNG format should have png: prefix"
        );

        let decoded = decode_base64_image(&encoded).expect("Should decode");

        assert_eq!(original.width, decoded.width);
        assert_eq!(original.height, decoded.height);
        assert_eq!(original.bytes.as_ref(), decoded.bytes.as_ref());
    }

    #[test]
    fn test_blob_image_roundtrip() {
        let original = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ]
            .into(),
        };

        let encoded = encode_image_as_blob(&original).expect("Should encode as blob");
        assert!(
            encoded.starts_with("blob:"),
            "Blob format should have blob: prefix"
        );

        let decoded = decode_base64_image(&encoded).expect("Should decode blob");

        assert_eq!(original.width, decoded.width);
        assert_eq!(original.height, decoded.height);
        assert_eq!(original.bytes.as_ref(), decoded.bytes.as_ref());
    }

    #[test]
    fn test_png_compression_saves_space() {
        let original = arboard::ImageData {
            width: 100,
            height: 100,
            bytes: vec![128u8; 100 * 100 * 4].into(),
        };

        let png_encoded = encode_image_as_png(&original).expect("Should encode as PNG");
        let rgba_encoded = encode_image_as_base64(&original).expect("Should encode as RGBA");

        assert!(
            png_encoded.len() < rgba_encoded.len(),
            "PNG should be smaller for 100x100 image: PNG={} vs RGBA={}",
            png_encoded.len(),
            rgba_encoded.len()
        );

        let decoded = decode_base64_image(&png_encoded).expect("Should decode");
        assert_eq!(original.width, decoded.width);
        assert_eq!(original.height, decoded.height);
    }

    #[test]
    fn test_content_to_png_bytes_from_png() {
        let original = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ]
            .into(),
        };

        let encoded = encode_image_as_png(&original).expect("Should encode as PNG");
        let bytes = content_to_png_bytes(&encoded).expect("Should decode PNG bytes");

        // PNG signature: 89 50 4E 47 0D 0A 1A 0A
        assert!(bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }

    #[test]
    fn test_content_to_png_bytes_from_rgba() {
        let original = arboard::ImageData {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0, 255].into(),
        };

        let encoded = encode_image_as_base64(&original).expect("Should encode as RGBA");
        let bytes = content_to_png_bytes(&encoded).expect("Should convert RGBA to PNG bytes");

        assert!(bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    }

    #[test]
    fn test_get_image_dimensions_both_formats() {
        let original = arboard::ImageData {
            width: 100,
            height: 50,
            bytes: vec![0u8; 100 * 50 * 4].into(),
        };

        let rgba_encoded = encode_image_as_base64(&original).expect("Should encode");
        let dims = get_image_dimensions(&rgba_encoded).expect("Should get dimensions");
        assert_eq!(dims, (100, 50));

        let png_encoded = encode_image_as_png(&original).expect("Should encode as PNG");
        let dims = get_image_dimensions(&png_encoded).expect("Should get PNG dimensions");
        assert_eq!(dims, (100, 50));
    }

    #[test]
    fn test_get_image_dimensions_rejects_rgba_without_payload() {
        let dims = get_image_dimensions("rgba:10:20");
        assert!(dims.is_none(), "Missing RGBA payload should be rejected");
    }

    #[test]
    fn test_decode_legacy_rgba_rejects_wrong_byte_count() {
        // Create RGBA string with wrong byte count (too few bytes)
        let bad_data = format!(
            "rgba:2:2:{}",
            BASE64.encode([0u8; 8]) // Should be 16 bytes for 2x2
        );
        let result = decode_base64_image(&bad_data);
        assert!(result.is_none(), "Should reject RGBA with wrong byte count");

        // Create RGBA string with too many bytes
        let bad_data = format!(
            "rgba:2:2:{}",
            BASE64.encode([0u8; 32]) // Should be 16 bytes for 2x2
        );
        let result = decode_base64_image(&bad_data);
        assert!(result.is_none(), "Should reject RGBA with too many bytes");
    }

    #[test]
    fn test_decode_legacy_rgba_rejects_overflow_dimensions() {
        // Create RGBA string with dimensions that would overflow when multiplied
        // Using u32::MAX-ish values that would overflow
        let bad_data = format!(
            "rgba:{}:{}:{}",
            u32::MAX,
            u32::MAX,
            BASE64.encode([0u8; 16])
        );
        let result = decode_base64_image(&bad_data);
        assert!(
            result.is_none(),
            "Should reject RGBA with overflow dimensions"
        );
    }

    #[test]
    fn test_fast_png_dimensions_extraction() {
        // Create a test PNG image
        let original = arboard::ImageData {
            width: 123,
            height: 456,
            bytes: vec![0u8; 123 * 456 * 4].into(),
        };

        let encoded = encode_image_as_png(&original).expect("Should encode as PNG");
        let base64_data = encoded
            .strip_prefix("png:")
            .expect("Should have png: prefix");

        // Test fast extraction
        let dims = get_png_dimensions_fast(base64_data);
        assert_eq!(
            dims,
            Some((123, 456)),
            "Fast extraction should get correct dimensions"
        );

        // Test through main API
        let dims = get_image_dimensions(&encoded);
        assert_eq!(
            dims,
            Some((123, 456)),
            "Main API should get correct dimensions"
        );
    }

    #[test]
    fn test_fast_png_dimensions_rejects_invalid() {
        // Too short
        assert!(get_png_dimensions_fast("abc").is_none());

        // Valid length but not PNG
        let not_png = BASE64.encode([0u8; 24]);
        assert!(get_png_dimensions_fast(&not_png).is_none());

        // Valid PNG signature but wrong IHDR
        let mut bad_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG sig
        bad_header.extend_from_slice(&[0, 0, 0, 13]); // chunk len
        bad_header.extend_from_slice(b"XXXX"); // wrong chunk type
        bad_header.extend_from_slice(&[0, 0, 0, 100]); // width
        bad_header.extend_from_slice(&[0, 0, 0, 100]); // height
        let bad_png = BASE64.encode(&bad_header);
        assert!(get_png_dimensions_fast(&bad_png).is_none());
    }

    fn patch_png_ihdr_dimensions(png_bytes: &mut [u8], width: u32, height: u32) {
        assert!(
            png_bytes.len() >= 33,
            "PNG bytes should include IHDR and CRC"
        );

        png_bytes[16..20].copy_from_slice(&width.to_be_bytes());
        png_bytes[20..24].copy_from_slice(&height.to_be_bytes());

        let ihdr_chunk_type_and_data = &png_bytes[12..29];
        let crc = crc32(ihdr_chunk_type_and_data);
        png_bytes[29..33].copy_from_slice(&crc.to_be_bytes());
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;

        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                if (crc & 1) == 1 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }

        !crc
    }

    #[test]
    fn test_decode_to_render_image_rejects_png_above_pixel_limit() {
        let original = arboard::ImageData {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0, 255].into(),
        };
        let encoded = encode_image_as_png(&original).expect("Should encode PNG");
        let mut png_bytes = BASE64
            .decode(
                encoded
                    .strip_prefix("png:")
                    .expect("Should have png prefix"),
            )
            .expect("Should decode PNG base64");

        // Overwrite IHDR dimensions and CRC to exceed MAX_RENDER_IMAGE_PIXELS.
        patch_png_ihdr_dimensions(&mut png_bytes, 5000, 5000);

        let oversized_png = format!("png:{}", BASE64.encode(&png_bytes));
        assert_eq!(get_image_dimensions(&oversized_png), Some((5000, 5000)));
        assert!(
            decode_to_render_image(&oversized_png).is_none(),
            "Oversized PNG should be rejected before full decode"
        );
    }

    #[test]
    fn test_decode_to_render_image_rejects_blob_above_pixel_limit() {
        let original = arboard::ImageData {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0, 255].into(),
        };
        let mut png_bytes = encode_image_to_png_bytes(&original).expect("Should encode PNG bytes");

        // Overwrite IHDR dimensions and CRC to exceed MAX_RENDER_IMAGE_PIXELS.
        patch_png_ihdr_dimensions(&mut png_bytes, 5000, 5000);

        let oversized_blob = store_blob(&png_bytes).expect("Should store oversized blob");
        assert_eq!(get_image_dimensions(&oversized_blob), Some((5000, 5000)));
        assert!(
            decode_to_render_image(&oversized_blob).is_none(),
            "Oversized blob should be rejected before full decode"
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn test_encode_image_to_png_bytes_rejects_width_over_u32() {
        let too_wide = usize::try_from(u32::MAX).expect("u32 should fit in usize") + 1;
        let image = arboard::ImageData {
            width: too_wide,
            height: 1,
            bytes: vec![0, 0, 0, 255].into(),
        };

        let err = encode_image_to_png_bytes(&image).expect_err("Width over u32 should fail");
        assert!(
            err.to_string().contains("width exceeds u32"),
            "Error should mention width conversion failure"
        );
    }
}

/// Decode clipboard image content to raw RGBA bytes for OCR processing
///
/// Returns (width, height, rgba_bytes) suitable for passing to ocr::extract_text_from_rgba().
///
/// Supports:
/// - blob:{hash} -> load PNG, decode to RGBA
/// - png:{base64} -> decode base64, decode PNG to RGBA
/// - rgba:{width}:{height}:{base64} -> decode base64 directly
///
/// Note: Unlike decode_to_render_image which converts to BGRA for Metal/GPUI,
/// this function returns true RGBA bytes as expected by the Vision OCR framework.
#[allow(dead_code)]
pub fn decode_to_rgba_bytes(content: &str) -> Option<(u32, u32, Vec<u8>)> {
    if is_blob_content(content) {
        decode_blob_to_rgba_bytes(content)
    } else if content.starts_with("png:") {
        decode_png_to_rgba_bytes(content)
    } else if content.starts_with("rgba:") {
        decode_legacy_rgba_to_bytes(content)
    } else {
        warn!("Unknown clipboard image format for RGBA decode");
        None
    }
}

/// Decode blob format to raw RGBA bytes
#[allow(dead_code)]
fn decode_blob_to_rgba_bytes(content: &str) -> Option<(u32, u32, Vec<u8>)> {
    let png_bytes = load_blob(content)?;

    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    debug!(
        width = width,
        height = height,
        format = "blob",
        "Decoded blob to RGBA bytes for OCR"
    );

    Some((width, height, rgba.into_raw()))
}

/// Decode PNG format to raw RGBA bytes
#[allow(dead_code)]
fn decode_png_to_rgba_bytes(content: &str) -> Option<(u32, u32, Vec<u8>)> {
    let base64_data = content.strip_prefix("png:")?;
    let png_bytes = BASE64.decode(base64_data).ok()?;

    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    debug!(
        width = width,
        height = height,
        format = "png",
        "Decoded PNG to RGBA bytes for OCR"
    );

    Some((width, height, rgba.into_raw()))
}

/// Decode legacy RGBA format to raw bytes (already in correct format)
#[allow(dead_code)]
fn decode_legacy_rgba_to_bytes(content: &str) -> Option<(u32, u32, Vec<u8>)> {
    let parts: Vec<&str> = content.splitn(4, ':').collect();
    if parts.len() != 4 || parts[0] != "rgba" {
        return None;
    }

    let width: u32 = parts[1].parse().ok()?;
    let height: u32 = parts[2].parse().ok()?;
    let bytes = BASE64.decode(parts[3]).ok()?;

    // Validate byte length
    let expected: usize = u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(4)?
        .try_into()
        .ok()?;
    if bytes.len() != expected {
        warn!(
            width,
            height,
            expected,
            actual = bytes.len(),
            "Legacy RGBA byte length mismatch for OCR"
        );
        return None;
    }

    debug!(
        width = width,
        height = height,
        format = "rgba",
        "Decoded legacy RGBA bytes for OCR"
    );

    Some((width, height, bytes))
}

#[cfg(test)]
mod ocr_decode_tests {
    use super::*;

    #[test]
    fn test_decode_to_rgba_bytes_png() {
        let original = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: vec![
                255, 0, 0, 255, // red pixel
                0, 255, 0, 255, // green pixel
                0, 0, 255, 255, // blue pixel
                255, 255, 255, 255, // white pixel
            ]
            .into(),
        };

        let encoded = encode_image_as_png(&original).expect("Should encode as PNG");
        let (width, height, rgba) = decode_to_rgba_bytes(&encoded).expect("Should decode");

        assert_eq!(width, 2);
        assert_eq!(height, 2);
        assert_eq!(rgba.len(), 16); // 2x2x4 bytes
                                    // First pixel should be red (255, 0, 0, 255)
        assert_eq!(rgba[0], 255);
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn test_decode_to_rgba_bytes_legacy() {
        let original = arboard::ImageData {
            width: 1,
            height: 1,
            bytes: vec![128, 64, 32, 255].into(),
        };

        let encoded = encode_image_as_base64(&original).expect("Should encode");
        let (width, height, rgba) = decode_to_rgba_bytes(&encoded).expect("Should decode");

        assert_eq!(width, 1);
        assert_eq!(height, 1);
        assert_eq!(rgba, vec![128, 64, 32, 255]);
    }
}

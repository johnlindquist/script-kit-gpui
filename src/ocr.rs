//! OCR Module - macOS Vision Framework Integration
//!
//! Provides text extraction from images using the macOS Vision framework (VNRecognizeTextRequest).
//!
//! ## Features
//! - Extract text from RGBA image data
//! - Async wrapper for background thread execution
//! - Automatic Vision framework initialization
//! - Graceful error handling
//!
//!
//! ## Platform Support
//! This module only works on macOS. On other platforms, the functions will return
//! an error indicating OCR is not supported.

use anyhow::{anyhow, Result};
use std::thread;
use tracing::{debug, error, info};

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};

#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
use std::ffi::c_void;

// Core Graphics type aliases for FFI
#[cfg(target_os = "macos")]
type CGImageRef = *mut c_void;
#[cfg(target_os = "macos")]
type CGColorSpaceRef = *mut c_void;
#[cfg(target_os = "macos")]
type CGDataProviderRef = *mut c_void;

// Core Graphics constants
#[cfg(target_os = "macos")]
const K_CG_RENDERING_INTENT_DEFAULT: u32 = 0;
#[cfg(target_os = "macos")]
const K_CG_IMAGE_ALPHA_LAST: u32 = 3; // kCGImageAlphaLast
#[cfg(target_os = "macos")]
const K_CG_BITMAP_BYTE_ORDER_DEFAULT: u32 = 0;

// External Core Graphics functions
#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    fn CGColorSpaceRelease(space: CGColorSpaceRef);
    fn CGDataProviderCreateWithData(
        info: *mut c_void,
        data: *const c_void,
        size: usize,
        releaseData: *const c_void,
    ) -> CGDataProviderRef;
    fn CGDataProviderRelease(provider: CGDataProviderRef);
    fn CGImageCreate(
        width: usize,
        height: usize,
        bitsPerComponent: usize,
        bitsPerPixel: usize,
        bytesPerRow: usize,
        space: CGColorSpaceRef,
        bitmapInfo: u32,
        provider: CGDataProviderRef,
        decode: *const f64,
        shouldInterpolate: bool,
        intent: u32,
    ) -> CGImageRef;
    fn CGImageRelease(image: CGImageRef);
}

// Link Vision framework for OCR
#[cfg(target_os = "macos")]
#[link(name = "Vision", kind = "framework")]
extern "C" {}

/// Extract text from RGBA image data using macOS Vision framework
///
/// This function uses VNRecognizeTextRequest to perform OCR on the provided image.
/// The image data must be in RGBA format (4 bytes per pixel).
///
/// # Arguments
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels  
/// * `rgba_data` - Raw RGBA pixel data (must be exactly width * height * 4 bytes)
///
/// # Returns
/// * `Ok(String)` - Extracted text, may be empty if no text was found
/// * `Err` - If OCR fails or platform is not supported
///
#[cfg(target_os = "macos")]
pub fn extract_text_from_rgba(width: u32, height: u32, rgba_data: &[u8]) -> Result<String> {
    let expected_size = (width as usize) * (height as usize) * 4;
    if rgba_data.len() != expected_size {
        return Err(anyhow!(
            "Invalid RGBA data size: expected {} bytes, got {}",
            expected_size,
            rgba_data.len()
        ));
    }

    if width == 0 || height == 0 {
        return Err(anyhow!("Image dimensions cannot be zero"));
    }

    debug!(
        width = width,
        height = height,
        data_size = rgba_data.len(),
        "Starting OCR text extraction"
    );

    unsafe { extract_text_vision(width, height, rgba_data) }
}

#[cfg(not(target_os = "macos"))]
pub fn extract_text_from_rgba(_width: u32, _height: u32, _rgba_data: &[u8]) -> Result<String> {
    Err(anyhow!("OCR is only supported on macOS"))
}

/// Extract text asynchronously on a background thread
///
/// This function spawns a background thread to perform OCR, avoiding blocking
/// the main thread. The callback is called with the result when OCR completes.
///
/// # Arguments
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
/// * `rgba_data` - Raw RGBA pixel data (ownership transferred to background thread)
/// * `callback` - Function called with OCR result when complete
///
pub fn extract_text_async<F>(width: u32, height: u32, rgba_data: Vec<u8>, callback: F)
where
    F: FnOnce(Result<String>) + Send + 'static,
{
    thread::spawn(move || {
        let result = extract_text_from_rgba(width, height, &rgba_data);
        callback(result);
    });
}

/// Internal Vision framework implementation
#[cfg(target_os = "macos")]
unsafe fn extract_text_vision(width: u32, height: u32, rgba_data: &[u8]) -> Result<String> {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let bytes_per_row = width_usize * 4;

    // Create CGColorSpace (sRGB)
    let color_space = CGColorSpaceCreateDeviceRGB();
    if color_space.is_null() {
        return Err(anyhow!("Failed to create CGColorSpace"));
    }

    // Create CGDataProvider from rgba_data
    // Note: We pass null for releaseData callback since we manage the memory ourselves
    let data_provider = CGDataProviderCreateWithData(
        std::ptr::null_mut(),
        rgba_data.as_ptr() as *const c_void,
        rgba_data.len(),
        std::ptr::null(),
    );
    if data_provider.is_null() {
        CGColorSpaceRelease(color_space);
        return Err(anyhow!("Failed to create CGDataProvider"));
    }

    // Create CGImage
    let bitmap_info = K_CG_BITMAP_BYTE_ORDER_DEFAULT | K_CG_IMAGE_ALPHA_LAST;
    let cg_image = CGImageCreate(
        width_usize,
        height_usize,
        8,  // bits per component
        32, // bits per pixel (RGBA)
        bytes_per_row,
        color_space,
        bitmap_info,
        data_provider,
        std::ptr::null(), // decode array
        false,            // should interpolate
        K_CG_RENDERING_INTENT_DEFAULT,
    );

    // Release intermediate objects (CGImage retains what it needs)
    CGDataProviderRelease(data_provider);
    CGColorSpaceRelease(color_space);

    if cg_image.is_null() {
        return Err(anyhow!("Failed to create CGImage from RGBA data"));
    }

    // Now use Vision framework via objc
    let result = perform_ocr_on_cgimage(cg_image);

    // Release CGImage
    CGImageRelease(cg_image);

    result
}

/// Perform OCR using Vision framework on a CGImage
#[cfg(target_os = "macos")]
unsafe fn perform_ocr_on_cgimage(cg_image: CGImageRef) -> Result<String> {
    // Create VNImageRequestHandler with CGImage
    // VNImageRequestHandler initWithCGImage:options:
    let handler_alloc: id = msg_send![class!(VNImageRequestHandler), alloc];
    let empty_dict: id = msg_send![class!(NSDictionary), dictionary];
    let request_handler: id = msg_send![
        handler_alloc,
        initWithCGImage: cg_image
        options: empty_dict
    ];
    if request_handler == nil {
        return Err(anyhow!("Failed to create VNImageRequestHandler"));
    }

    // Create VNRecognizeTextRequest
    let request_alloc: id = msg_send![class!(VNRecognizeTextRequest), alloc];
    let text_request: id = msg_send![request_alloc, init];
    if text_request == nil {
        let _: () = msg_send![request_handler, release];
        return Err(anyhow!("Failed to create VNRecognizeTextRequest"));
    }

    // Configure recognition level to accurate (1 = VNRequestTextRecognitionLevelAccurate)
    let _: () = msg_send![text_request, setRecognitionLevel: 1i64];

    // Enable language correction for better results
    let _: () = msg_send![text_request, setUsesLanguageCorrection: true];

    // Create NSArray with the request
    let requests: id = msg_send![
        class!(NSArray),
        arrayWithObject: text_request
    ];

    // Perform the request
    let mut error: id = nil;
    let success: bool = msg_send![
        request_handler,
        performRequests: requests
        error: &mut error
    ];

    if !success || error != nil {
        let error_desc = if error != nil {
            let desc: id = msg_send![error, localizedDescription];
            nsstring_to_string(desc)
        } else {
            "Unknown Vision framework error".to_string()
        };

        let _: () = msg_send![text_request, release];
        let _: () = msg_send![request_handler, release];

        error!(error = %error_desc, "Vision OCR request failed");
        return Err(anyhow!("Vision OCR failed: {}", error_desc));
    }

    // Get results from the request
    let results: id = msg_send![text_request, results];
    if results == nil {
        let _: () = msg_send![text_request, release];
        let _: () = msg_send![request_handler, release];
        info!("OCR completed with no text found");
        return Ok(String::new());
    }

    // Iterate through results and collect text
    let count: usize = msg_send![results, count];
    let mut extracted_text = Vec::with_capacity(count);

    for i in 0..count {
        let observation: id = msg_send![results, objectAtIndex: i];
        if observation == nil {
            continue;
        }

        // Get top candidate (most confident recognition)
        let candidates: id = msg_send![observation, topCandidates: 1usize];
        if candidates == nil {
            continue;
        }

        let candidate_count: usize = msg_send![candidates, count];
        if candidate_count == 0 {
            continue;
        }

        let top_candidate: id = msg_send![candidates, objectAtIndex: 0usize];
        if top_candidate == nil {
            continue;
        }

        // Get the string from the candidate
        let text_str: id = msg_send![top_candidate, string];
        if text_str != nil {
            let text = nsstring_to_string(text_str);
            if !text.is_empty() {
                extracted_text.push(text);
            }
        }
    }

    // Release objects
    let _: () = msg_send![text_request, release];
    let _: () = msg_send![request_handler, release];

    let result = extracted_text.join("\n");
    info!(
        text_lines = extracted_text.len(),
        text_length = result.len(),
        "OCR extraction completed"
    );

    Ok(result)
}

/// Convert NSString to Rust String
#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(ns_string: id) -> String {
    if ns_string == nil {
        return String::new();
    }

    let utf8_ptr: *const i8 = msg_send![ns_string, UTF8String];
    if utf8_ptr.is_null() {
        return String::new();
    }

    std::ffi::CStr::from_ptr(utf8_ptr)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_invalid_size() {
        // Test with mismatched data size
        let result = extract_text_from_rgba(100, 100, &[0u8; 100]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid RGBA data size"));
    }

    #[test]
    fn test_extract_text_zero_dimensions() {
        let result = extract_text_from_rgba(0, 100, &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("dimensions cannot be zero"));
    }

    /// Check if Vision framework classes are available
    /// This may fail in certain test environments or CI
    #[cfg(target_os = "macos")]
    fn vision_framework_available() -> bool {
        use objc::runtime::Class;
        Class::get("VNImageRequestHandler").is_some()
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_extract_text_empty_image() {
        // Skip if Vision framework is not available (e.g., in sandboxed test environment)
        if !vision_framework_available() {
            eprintln!("Skipping test: Vision framework not available in test environment");
            return;
        }

        // Create a small blank image (10x10 white)
        let width = 10u32;
        let height = 10u32;
        let rgba_data: Vec<u8> = vec![255u8; (width * height * 4) as usize];

        // This should succeed but return empty text (no text in blank image)
        let result = extract_text_from_rgba(width, height, &rgba_data);
        // The Vision framework might return an error or empty string for blank images
        // We just verify it doesn't panic
        match result {
            Ok(text) => {
                // Empty or whitespace-only text is expected
                assert!(text.trim().is_empty() || text.len() < 10);
            }
            Err(e) => {
                // Some errors are acceptable for blank images
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("vision")
                        || msg.contains("ocr")
                        || msg.contains("no text")
                        || msg.contains("failed"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_async_extraction_calls_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let callback_called = Arc::new(AtomicBool::new(false));
        let callback_called_clone = callback_called.clone();

        // Small test image (10x1 pixels)
        let rgba_data: Vec<u8> = vec![255u8; 40];

        extract_text_async(10, 1, rgba_data, move |_result| {
            callback_called_clone.store(true, Ordering::SeqCst);
        });

        // Wait for callback (with timeout)
        for _ in 0..100 {
            if callback_called.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            callback_called.load(Ordering::SeqCst),
            "Callback should have been called"
        );
    }
}

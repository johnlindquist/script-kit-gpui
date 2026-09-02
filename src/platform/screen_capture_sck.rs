// ============================================================================
// ScreenCaptureKit Screenshot Capture (macOS 14+)
// ============================================================================
//
// `CGWindowListCreateImage`, `CGWindowListCreateImageFromArray`, and
// `CGDisplayCreateImage` were obsoleted in macOS 15 and return null at
// runtime, so the legacy capture path in `ai_commands.rs` cannot produce
// pixels on modern macOS. This module is the replacement backend:
// `SCShareableContent` → `SCContentFilter(display, excludingWindows: <ours>)`
// → `SCScreenshotManager`, capturing the active display (the one the Script
// Kit panel is on) with Script Kit's own windows excluded.

#[cfg(target_os = "macos")]
mod screen_capture_kit_link {
    // Force ScreenCaptureKit to be linked so the SC* classes resolve through
    // the ObjC class registry at runtime.
    #[link(name = "ScreenCaptureKit", kind = "framework")]
    unsafe extern "C" {}
}

/// Render a human-readable description for an NSError pointer.
#[cfg(target_os = "macos")]
fn sck_nserror_description(error: *mut objc::runtime::Object) -> String {
    use std::ffi::CStr;

    // SAFETY: `error` is a non-null NSError delivered by a ScreenCaptureKit
    // completion handler. `localizedDescription` returns an NSString whose
    // UTF8String buffer is valid for the duration of this call.
    unsafe {
        let description: *mut objc::runtime::Object = msg_send![error, localizedDescription];
        if description.is_null() {
            return "unknown NSError".to_string();
        }
        let utf8: *const std::os::raw::c_char = msg_send![description, UTF8String];
        if utf8.is_null() {
            return "unknown NSError".to_string();
        }
        CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}

/// Resolve the CGDirectDisplayID of the display the Script Kit panel is on.
///
/// Matches the center of `capture_target_bounds()` (the active display's
/// visible area, falling back to the main display off the main thread)
/// against `CGDisplayBounds` of each active display.
#[cfg(target_os = "macos")]
fn sck_active_display_id() -> u32 {
    use core_graphics::display::CGDisplay;

    let target = capture_target_bounds();
    let center_x = target.origin.x + target.size.width / 2.0;
    let center_y = target.origin.y + target.size.height / 2.0;

    if let Ok(ids) = CGDisplay::active_displays() {
        for id in ids {
            let bounds = CGDisplay::new(id).bounds();
            if center_x >= bounds.origin.x
                && center_x < bounds.origin.x + bounds.size.width
                && center_y >= bounds.origin.y
                && center_y < bounds.origin.y + bounds.size.height
            {
                return id;
            }
        }
    }

    CGDisplay::main().id
}

/// Appended to ScreenCaptureKit errors when the Screen Recording permission
/// preflight reports the capture would be denied.
#[cfg(target_os = "macos")]
fn sck_permission_hint() -> &'static str {
    if matches!(screen_capture_access_preflight(), Some(false)) {
        " (macOS Screen Recording permission is not granted for Script Kit — enable it in System Settings → Privacy & Security → Screen & System Audio Recording)"
    } else {
        ""
    }
}

/// Capture the active display via ScreenCaptureKit, excluding Script Kit's
/// own windows so the result shows the desktop behind the panel.
///
/// Output is at nominal (1x) resolution to match the legacy capture path.
/// Blocks the calling thread for up to ~16s worst case (two 8s completion
/// timeouts); in practice content enumeration plus capture takes a few
/// hundred milliseconds. ScreenCaptureKit delivers completion handlers on a
/// private queue, so blocking the main thread here cannot deadlock.
#[cfg(target_os = "macos")]
pub fn capture_active_display_screenshot_sck(
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)?;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use objc::runtime::{Class, Object, NO, YES};
    use std::ffi::c_void;
    use std::sync::mpsc;
    use std::time::Duration;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGImageGetWidth(image: *const c_void) -> usize;
        fn CGImageGetHeight(image: *const c_void) -> usize;
        fn CGImageRetain(image: *const c_void) -> *const c_void;
        fn CGImageRelease(image: *mut c_void);
    }

    let (Some(content_class), Some(screenshot_manager_class)) = (
        Class::get("SCShareableContent"),
        Class::get("SCScreenshotManager"),
    ) else {
        return Err("ScreenCaptureKit screenshot API unavailable (requires macOS 14+)".into());
    };

    // ---- 1. Enumerate shareable content (displays + on-screen windows) ----
    let (content_tx, content_rx) = mpsc::channel::<Result<usize, String>>();
    let content_block =
        block::ConcreteBlock::new(move |content: *mut Object, error: *mut Object| {
            let result = if !error.is_null() {
                Err(sck_nserror_description(error))
            } else if content.is_null() {
                Err("SCShareableContent completion returned no content".to_string())
            } else {
                // SAFETY: retain so the pointer stays valid after the
                // handler's autorelease scope; balanced by the release below
                // (or in the SendError arm if the receiver already timed out).
                let _: *mut Object = unsafe { msg_send![content, retain] };
                Ok(content as usize)
            };
            if let Err(mpsc::SendError(Ok(pointer))) = content_tx.send(result) {
                // SAFETY: receiver is gone (timeout); balance the retain above.
                let _: () = unsafe { msg_send![pointer as *mut Object, release] };
            }
        });
    let content_block = content_block.copy();

    // SAFETY: class method with a copied heap block; ScreenCaptureKit invokes
    // the handler exactly once on a private queue.
    let _: () = unsafe {
        msg_send![content_class,
            getShareableContentExcludingDesktopWindows: NO
            onScreenWindowsOnly: YES
            completionHandler: &*content_block]
    };

    let content = content_rx
        .recv_timeout(Duration::from_secs(8))
        .map_err(|_| "Timed out waiting for SCShareableContent".to_string())
        .and_then(|result| result)
        .map_err(|error| format!("SCShareableContent failed: {error}{}", sck_permission_hint()))?
        as *mut Object;

    // From here on, `content` holds a retain we must balance on every path.
    let capture_result = (|| -> Result<(Vec<u8>, u32, u32), String> {
        // ---- 2. Pick the display the Script Kit panel is on ----
        // SAFETY: `content` is a retained SCShareableContent; `displays` and
        // `windows` are NSArray properties owned by it and stay valid while
        // the retain is held.
        let displays: *mut Object = unsafe { msg_send![content, displays] };
        if displays.is_null() {
            return Err("SCShareableContent returned no display list".to_string());
        }
        let display_count: usize = unsafe { msg_send![displays, count] };
        if display_count == 0 {
            return Err("SCShareableContent returned zero displays".to_string());
        }

        let target_display_id = sck_active_display_id();
        let mut display: *mut Object = std::ptr::null_mut();
        for index in 0..display_count {
            let candidate: *mut Object = unsafe { msg_send![displays, objectAtIndex: index] };
            let display_id: u32 = unsafe { msg_send![candidate, displayID] };
            if display_id == target_display_id {
                display = candidate;
                break;
            }
        }
        if display.is_null() {
            display = unsafe { msg_send![displays, objectAtIndex: 0usize] };
        }

        // ---- 3. Exclude Script Kit's own windows from the capture ----
        let my_pid = std::process::id() as i32;
        let excluded: *mut Object = unsafe { msg_send![class!(NSMutableArray), alloc] };
        let excluded: *mut Object = unsafe { msg_send![excluded, init] };
        let mut excluded_count: usize = 0;
        let windows: *mut Object = unsafe { msg_send![content, windows] };
        if !windows.is_null() {
            let window_count: usize = unsafe { msg_send![windows, count] };
            for index in 0..window_count {
                let window: *mut Object = unsafe { msg_send![windows, objectAtIndex: index] };
                let app: *mut Object = unsafe { msg_send![window, owningApplication] };
                if app.is_null() {
                    continue;
                }
                let pid: i32 = unsafe { msg_send![app, processID] };
                if pid == my_pid {
                    let _: () = unsafe { msg_send![excluded, addObject: window] };
                    excluded_count += 1;
                }
            }
        }

        // ---- 4. Build the filter + configuration (1x output) ----
        let filter_class = Class::get("SCContentFilter")
            .ok_or_else(|| "SCContentFilter class unavailable".to_string())?;
        let config_class = Class::get("SCStreamConfiguration")
            .ok_or_else(|| "SCStreamConfiguration class unavailable".to_string())?;

        let filter: *mut Object = unsafe { msg_send![filter_class, alloc] };
        let filter: *mut Object =
            unsafe { msg_send![filter, initWithDisplay: display excludingWindows: excluded] };

        let config: *mut Object = unsafe { msg_send![config_class, alloc] };
        let config: *mut Object = unsafe { msg_send![config, init] };
        let display_width: isize = unsafe { msg_send![display, width] };
        let display_height: isize = unsafe { msg_send![display, height] };
        let _: () = unsafe { msg_send![config, setWidth: display_width.max(1) as usize] };
        let _: () = unsafe { msg_send![config, setHeight: display_height.max(1) as usize] };
        let _: () = unsafe { msg_send![config, setShowsCursor: NO] };

        // ---- 5. Capture the screenshot ----
        let (image_tx, image_rx) = mpsc::channel::<Result<usize, String>>();
        let image_block =
            block::ConcreteBlock::new(move |image: *mut c_void, error: *mut Object| {
                let result = if !error.is_null() {
                    Err(sck_nserror_description(error))
                } else if image.is_null() {
                    Err("SCScreenshotManager returned a null image".to_string())
                } else {
                    // SAFETY: retain the CGImage so it survives the handler;
                    // released after encoding (or in the SendError arm).
                    unsafe { CGImageRetain(image) };
                    Ok(image as usize)
                };
                if let Err(mpsc::SendError(Ok(pointer))) = image_tx.send(result) {
                    // SAFETY: receiver is gone (timeout); balance the retain.
                    unsafe { CGImageRelease(pointer as *mut c_void) };
                }
            });
        let image_block = image_block.copy();

        // SAFETY: SCScreenshotManager retains the filter/configuration for
        // the duration of the async capture and invokes the handler once.
        let _: () = unsafe {
            msg_send![screenshot_manager_class,
                captureImageWithFilter: filter
                configuration: config
                completionHandler: &*image_block]
        };

        let image_result = image_rx
            .recv_timeout(Duration::from_secs(8))
            .map_err(|_| "Timed out waiting for SCScreenshotManager".to_string())
            .and_then(|result| result);

        // SAFETY: our references to the filter/config/excluded array are no
        // longer needed; the capture has completed (or timed out, in which
        // case ScreenCaptureKit still holds its own retains).
        unsafe {
            let _: () = msg_send![filter, release];
            let _: () = msg_send![config, release];
            let _: () = msg_send![excluded, release];
        }

        let cg_image = image_result
            .map_err(|error| format!("SCScreenshotManager failed: {error}"))?
            as *mut c_void;

        // ---- 6. Encode CGImage → RGBA → PNG ----
        // SAFETY: cg_image is a retained, non-null CGImageRef.
        let width = unsafe { CGImageGetWidth(cg_image) } as u32;
        let height = unsafe { CGImageGetHeight(cg_image) } as u32;
        let rgba_result = cgimage_to_rgba(cg_image, width, height);
        // SAFETY: balance the retain from the completion handler exactly once.
        unsafe { CGImageRelease(cg_image) };
        let rgba = rgba_result.map_err(|error| error.to_string())?;

        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(&mut png_data);
        encoder
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|error| error.to_string())?;

        tracing::debug!(
            width,
            height,
            file_size = png_data.len(),
            excluded_windows = excluded_count,
            display_id = target_display_id,
            "Active display screenshot captured via ScreenCaptureKit"
        );

        Ok((png_data, width, height))
    })();

    // SAFETY: balance the retain taken in the shareable-content handler.
    let _: () = unsafe { msg_send![content, release] };

    capture_result.map_err(|error| format!("{error}{}", sck_permission_hint()).into())
}

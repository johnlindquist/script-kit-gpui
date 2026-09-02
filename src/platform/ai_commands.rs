// ============================================================================
// Screen Capture for AI Commands
// ============================================================================

/// Capture a screenshot of the active display, excluding Script Kit windows.
///
/// On macOS, uses ScreenCaptureKit (macOS 14+) to capture the display the
/// Script Kit panel is on with this process's windows excluded, so the
/// capture shows what is behind the panel. The legacy
/// `CGWindowListCreateImageFromArray` composite remains as a fallback for
/// macOS versions without `SCScreenshotManager`; it was obsoleted in
/// macOS 15 and returns null there. Falls back to xcap on other platforms.
///
/// # Returns
/// A tuple of (png_data, width, height) on success.
pub fn capture_screen_screenshot(
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)?;
    #[cfg(target_os = "macos")]
    {
        let sck_error = match capture_active_display_screenshot_sck() {
            Ok(captured) => return Ok(captured),
            Err(error) => error,
        };
        tracing::warn!(
            error = %sck_error,
            "ScreenCaptureKit capture failed; trying legacy CGWindowList capture"
        );
        capture_screen_excluding_self().map_err(|legacy_error| {
            format!(
                "ScreenCaptureKit capture failed: {sck_error}; legacy CGWindowList fallback also failed: {legacy_error}"
            )
            .into()
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        capture_screen_xcap_fallback()
    }
}

/// xcap-based full-screen capture (no window exclusion). Used on non-macOS.
#[cfg(not(target_os = "macos"))]
fn capture_screen_xcap_fallback(
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use xcap::Monitor;

    let monitors = Monitor::all()?;
    let monitor = monitors.into_iter().next().ok_or("No monitors found")?;

    tracing::debug!(
        name = %monitor.name().unwrap_or_default(),
        "Capturing primary monitor screenshot (xcap fallback)"
    );

    let image = monitor.capture_image()?;
    let width = image.width();
    let height = image.height();

    let new_width = width / 2;
    let new_height = height / 2;
    let resized = image::imageops::resize(
        &image,
        new_width,
        new_height,
        image::imageops::FilterType::Lanczos3,
    );

    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(&mut png_data);
    encoder.write_image(&resized, new_width, new_height, image::ExtendedColorType::Rgba8)?;

    tracing::debug!(
        width = new_width,
        height = new_height,
        file_size = png_data.len(),
        "Screen screenshot captured (xcap fallback)"
    );

    Ok((png_data, new_width, new_height))
}

/// The title used when a focused-window screenshot falls back to the
/// self-excluding screen capture because Script Kit is the frontmost app.
fn script_kit_excluded_capture_title() -> String {
    "Screen behind Script Kit panel".to_string()
}

/// Return the capture bounds for self-excluding screen capture.
///
/// Prefers the active display (the one containing the key window) so that
/// multi-monitor setups capture the screen behind the Script Kit panel rather
/// than always defaulting to `CGDisplay::main()`.
#[cfg(target_os = "macos")]
fn capture_target_bounds() -> core_graphics::display::CGRect {
    if let Some(display) = get_active_display() {
        let v = display.visible_area;
        return core_graphics::display::CGRect {
            origin: core_graphics::display::CGPoint {
                x: v.origin_x,
                y: v.origin_y,
            },
            size: core_graphics::display::CGSize {
                width: v.width,
                height: v.height,
            },
        };
    }
    core_graphics::display::CGDisplay::main().bounds()
}

/// macOS-native screen capture that excludes Script Kit windows.
///
/// 1. Enumerates on-screen windows via `CGWindowListCopyWindowInfo`.
/// 2. Filters out windows owned by our PID.
/// 3. Composites the remaining windows into a single image via
///    `CGWindowListCreateImageFromArray`.
#[cfg(target_os = "macos")]
fn capture_screen_excluding_self(
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use std::ffi::c_void;

    // CGWindowList constants
    const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
    const K_CG_NULL_WINDOW_ID: u32 = 0;
    const K_CG_WINDOW_IMAGE_DEFAULT: u32 = 0;
    // Apple's CGWindowImageOption values: kCGWindowImageNominalResolution = 1 << 4.
    // Previously this was set to `1 << 9`, which is undefined and silently
    // ignored by CoreGraphics — so captures were not actually requesting 1x.
    const K_CG_WINDOW_IMAGE_NOMINAL_RESOLUTION: u32 = 1 << 4;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(
            option: u32,
            relative_to_window: u32,
        ) -> core_foundation::array::CFArrayRef;

        fn CGWindowListCreateImageFromArray(
            screen_bounds: core_graphics::display::CGRect,
            window_array: core_foundation::array::CFArrayRef,
            image_option: u32,
        ) -> *mut c_void; // CGImageRef

        fn CGImageGetWidth(image: *const c_void) -> usize;
        fn CGImageGetHeight(image: *const c_void) -> usize;
        fn CGImageRelease(image: *mut c_void);
    }

    let my_pid = std::process::id() as i64;

    // SAFETY: CGWindowListCopyWindowInfo is a CoreGraphics API that returns a
    // CFArray of CFDictionary entries describing on-screen windows. We pass
    // valid constants and a null window ID to enumerate all on-screen windows.
    let window_info_list = unsafe {
        CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY,
            K_CG_NULL_WINDOW_ID,
        )
    };
    if window_info_list.is_null() {
        return Err("CGWindowListCopyWindowInfo returned null".into());
    }

    // SAFETY: We just checked that window_info_list is non-null and it was
    // returned by a CoreGraphics API that produces a valid CFArray.
    let info_array: CFArray =
        unsafe { CFArray::wrap_under_create_rule(window_info_list) };

    let k_owner_pid = CFString::new("kCGWindowOwnerPID");
    let k_window_number = CFString::new("kCGWindowNumber");

    let mut window_ids: Vec<i32> = Vec::new();
    let mut excluded_count: u32 = 0;

    for i in 0..info_array.len() {
        let dict_ref: CFDictionaryRef = match info_array.get(i) {
            Some(item_ref) => *item_ref as CFDictionaryRef,
            None => continue,
        };
        if dict_ref.is_null() {
            continue;
        }

        // Read the owner PID
        let mut pid_value: *const c_void = std::ptr::null();
        // SAFETY: dict_ref is a valid CFDictionary from the window list.
        // CFDictionaryGetValueIfPresent returns a pointer to the value or
        // false if the key is absent. We check the return value before using
        // pid_value.
        let has_pid = unsafe {
            core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict_ref,
                k_owner_pid.as_concrete_TypeRef() as *const c_void,
                &mut pid_value,
            )
        };
        if has_pid == 0 || pid_value.is_null() {
            continue;
        }

        // SAFETY: pid_value points to a CFNumber returned by CoreGraphics for
        // the kCGWindowOwnerPID key. We wrap without retaining because the
        // CFDictionary owns the value and will keep it alive while we read it.
        let pid_cf: CFNumber =
            unsafe { CFNumber::wrap_under_get_rule(pid_value as *const _) };
        let owner_pid: i64 = pid_cf
            .to_i64()
            .unwrap_or(-1);

        if owner_pid == my_pid {
            excluded_count += 1;
            continue;
        }

        // Read the window number (kCGWindowNumber)
        let mut wnum_value: *const c_void = std::ptr::null();
        // SAFETY: Same pattern as above — reading a CFNumber from a valid
        // CFDictionary entry.
        let has_wnum = unsafe {
            core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict_ref,
                k_window_number.as_concrete_TypeRef() as *const c_void,
                &mut wnum_value,
            )
        };
        if has_wnum == 0 || wnum_value.is_null() {
            continue;
        }

        // SAFETY: wnum_value is a CFNumber for the kCGWindowNumber key.
        let wnum_cf: CFNumber =
            unsafe { CFNumber::wrap_under_get_rule(wnum_value as *const _) };
        if let Some(wnum) = wnum_cf.to_i32() {
            window_ids.push(wnum);
        }
    }

    tracing::debug!(
        total_windows = window_ids.len() + excluded_count as usize,
        excluded = excluded_count,
        included = window_ids.len(),
        "Screen capture: filtered Script Kit windows"
    );

    if window_ids.is_empty() {
        return Err("No non-Script-Kit windows found on screen".into());
    }

    // Build a CFArray of CGWindowID (i32) values
    let cf_numbers: Vec<CFNumber> = window_ids.iter().map(|&id| CFNumber::from(id)).collect();
    let cf_array = CFArray::from_CFTypes(&cf_numbers);

    // Use the active display bounds so multi-monitor setups capture the
    // screen behind the Script Kit panel, not always the main display.
    let screen_bounds = capture_target_bounds();

    // SAFETY: CGWindowListCreateImageFromArray composites the listed windows
    // into a CGImage. screen_bounds is the main display rect, cf_array is a
    // valid CFArray of CGWindowID values, and we use nominal resolution to
    // get 1x output (avoids needing to halve retina pixels).
    let cg_image = unsafe {
        CGWindowListCreateImageFromArray(
            screen_bounds,
            cf_array.as_concrete_TypeRef(),
            K_CG_WINDOW_IMAGE_DEFAULT | K_CG_WINDOW_IMAGE_NOMINAL_RESOLUTION,
        )
    };

    if cg_image.is_null() {
        return Err("CGWindowListCreateImageFromArray returned null".into());
    }

    // SAFETY: cg_image is a non-null CGImageRef returned by CoreGraphics.
    let width = unsafe { CGImageGetWidth(cg_image) } as u32;
    let height = unsafe { CGImageGetHeight(cg_image) } as u32;

    // Convert CGImage -> RGBA bytes via a bitmap context.
    let rgba_result = cgimage_to_rgba(cg_image, width, height);

    // SAFETY: We are done reading the CGImage and must release it exactly once
    // on both success and error paths to avoid leaking the CoreGraphics
    // allocation.
    unsafe { CGImageRelease(cg_image) };

    let rgba_data = rgba_result?;

    // Encode to PNG
    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(&mut png_data);
    encoder.write_image(&rgba_data, width, height, image::ExtendedColorType::Rgba8)?;

    tracing::debug!(
        width,
        height,
        file_size = png_data.len(),
        excluded_windows = excluded_count,
        "Screen screenshot captured (Script Kit excluded)"
    );

    Ok((png_data, width, height))
}

/// Convert a CGImageRef to RGBA pixel bytes via a CGBitmapContext.
#[cfg(target_os = "macos")]
fn cgimage_to_rgba(
    cg_image: *mut std::ffi::c_void,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    use std::ffi::c_void;

    const K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST: u32 = 1;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
        fn CGColorSpaceRelease(space: *mut c_void);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            color_space: *mut c_void,
            bitmap_info: u32,
        ) -> *mut c_void;
        fn CGContextDrawImage(
            context: *mut c_void,
            rect: core_graphics::display::CGRect,
            image: *mut c_void,
        );
        fn CGContextRelease(context: *mut c_void);
    }

    let w = width as usize;
    let h = height as usize;
    // Checked math: unchecked `w * 4` / `h * bytes_per_row` could wrap on huge
    // images and hand CGBitmapContextCreate a buffer smaller than its metadata
    // claims, which is out-of-bounds writes in FFI land.
    let bytes_per_row = w
        .checked_mul(4)
        .ok_or("image width overflow computing bytes_per_row")?;
    let total_bytes = h
        .checked_mul(bytes_per_row)
        .ok_or("image dimensions overflow computing buffer size")?;
    let mut rgba = vec![0u8; total_bytes];

    // SAFETY: CGColorSpaceCreateDeviceRGB returns a valid CGColorSpaceRef.
    let color_space = unsafe { CGColorSpaceCreateDeviceRGB() };
    if color_space.is_null() {
        return Err("Failed to create RGB color space".into());
    }

    // Byte-order default, not kCGBitmapByteOrder32Big (1 << 12): macOS 26
    // rejects the explicit 32Big flag (CGBitmapContextCreate returns null for
    // every size), while default byte order is big-endian for 32-bit formats
    // anyway, so the RGBA byte layout is unchanged.
    let bitmap_info = K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST;

    // SAFETY: We pass a valid buffer, correct dimensions, and a valid color
    // space. The buffer is large enough for width * height * 4 bytes.
    let context = unsafe {
        CGBitmapContextCreate(
            rgba.as_mut_ptr() as *mut c_void,
            w,
            h,
            8,
            bytes_per_row,
            color_space,
            bitmap_info,
        )
    };

    // SAFETY: color_space is no longer needed after context creation.
    unsafe { CGColorSpaceRelease(color_space) };

    if context.is_null() {
        return Err(format!("Failed to create bitmap context ({width}x{height})").into());
    }

    let draw_rect = core_graphics::display::CGRect {
        origin: core_graphics::display::CGPoint { x: 0.0, y: 0.0 },
        size: core_graphics::display::CGSize {
            width: width as f64,
            height: height as f64,
        },
    };

    // SAFETY: context is a valid bitmap context, draw_rect matches its
    // dimensions, and cg_image is a valid CGImageRef.
    unsafe { CGContextDrawImage(context, draw_rect, cg_image) };

    // SAFETY: We are done drawing; release the bitmap context.
    unsafe { CGContextRelease(context) };

    // Un-premultiply alpha (CGBitmapContext gives premultiplied RGBA)
    for pixel in rgba.as_chunks_mut::<4>().0 {
        let a = pixel[3] as u16;
        if a > 0 && a < 255 {
            pixel[0] = ((pixel[0] as u16 * 255) / a).min(255) as u8;
            pixel[1] = ((pixel[1] as u16 * 255) / a).min(255) as u8;
            pixel[2] = ((pixel[2] as u16 * 255) / a).min(255) as u8;
        }
    }

    Ok(rgba)
}

/// Result of a focused window capture, including whether a fallback was used.
pub struct FocusedWindowCapture {
    pub png_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub window_title: String,
    /// True if no focused window was found and we fell back to the first available window.
    pub used_fallback: bool,
}

/// Capture a screenshot of the currently focused window (not our app).
///
/// When Script Kit itself is the frontmost (focused) app, this function
/// captures the entire screen *excluding* Script Kit windows via
/// `capture_screen_screenshot()` so the result shows what is behind the
/// panel. In that case `used_fallback` is `true` and `window_title` is
/// `"Screen behind Script Kit panel"`.
///
/// When a non-Script-Kit window is focused, the existing per-window capture
/// path is used.
///
/// # Returns
/// A `FocusedWindowCapture` on success.
pub fn capture_focused_window_screenshot(
) -> Result<FocusedWindowCapture, Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)?;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use xcap::Window;

    let windows = Window::all()?;

    // Find the focused window that is NOT our app
    let mut target_window = None;
    let mut found_focused = false;
    let mut script_kit_is_frontmost = false;

    for window in windows {
        let app_name = window.app_name().unwrap_or_else(|_| String::new());
        let is_minimized = window.is_minimized().unwrap_or(true);
        let is_focused = window.is_focused().unwrap_or(false);

        // Skip our own app
        let is_our_app = app_name.contains("script-kit-gpui")
            || app_name == "Script Kit"
            || app_name.contains("Script Kit");

        // Get window dimensions - skip tiny windows
        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);
        let is_reasonable_size = width >= 100 && height >= 100;

        if is_focused && is_our_app {
            script_kit_is_frontmost = true;
        }

        if !is_our_app && !is_minimized && is_reasonable_size {
            if is_focused {
                target_window = Some(window);
                found_focused = true;
                break;
            }
            // Keep the first reasonable non-our-app window as fallback
            if target_window.is_none() {
                target_window = Some(window);
            }
        }
    }

    // When Script Kit owns the focused window, capture the screen excluding
    // our own windows instead of picking an arbitrary fallback window.
    if script_kit_is_frontmost {
        tracing::debug!(
            "Script Kit is frontmost — using self-excluding screen capture"
        );
        let (png_data, width, height) = capture_screen_screenshot()?;
        return Ok(FocusedWindowCapture {
            png_data,
            width,
            height,
            window_title: script_kit_excluded_capture_title(),
            used_fallback: true,
        });
    }

    let used_fallback = target_window.is_some() && !found_focused;

    let window = target_window.ok_or("No suitable window found to capture")?;
    let title = window.title().unwrap_or_else(|_| "Unknown".to_string());
    let app_name = window.app_name().unwrap_or_else(|_| "Unknown".to_string());

    if used_fallback {
        tracing::warn!(
            app_name = %app_name,
            title = %title,
            "No focused window found, falling back to first available window"
        );
    } else {
        tracing::debug!(
            app_name = %app_name,
            title = %title,
            "Capturing focused window screenshot"
        );
    }

    let image = window.capture_image()?;
    let original_width = image.width();
    let original_height = image.height();

    // Scale down to 1x for efficiency
    let (final_image, width, height) = {
        let new_width = original_width / 2;
        let new_height = original_height / 2;
        let resized = image::imageops::resize(
            &image,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );
        (resized, new_width, new_height)
    };

    // Encode to PNG in memory
    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(&mut png_data);
    encoder.write_image(&final_image, width, height, image::ExtendedColorType::Rgba8)?;

    let display_title = if title.is_empty() {
        app_name
    } else {
        format!("{} - {}", app_name, title)
    };

    tracing::debug!(
        width = width,
        height = height,
        file_size = png_data.len(),
        title = %display_title,
        used_fallback = used_fallback,
        "Focused window screenshot captured"
    );

    Ok(FocusedWindowCapture {
        png_data,
        width,
        height,
        window_title: display_title,
        used_fallback,
    })
}

/// Metadata about a focused window without pixel data.
///
/// Returned by `capture_focused_window_metadata()` for callers that only need
/// the window title and dimensions — avoids the expensive `capture_image()` +
/// PNG-encode path.
pub struct FocusedWindowMetadata {
    pub window_title: String,
    pub width: u32,
    pub height: u32,
    pub used_fallback: bool,
}

/// Return focused-window metadata (title, dimensions) without capturing pixels.
///
/// Uses the same window-enumeration and Script-Kit-frontmost detection logic
/// as `capture_focused_window_screenshot()` but stops before `capture_image()`,
/// making it suitable for the Tab AI submit path where only metadata is needed.
///
/// When Script Kit is the frontmost app, returns the excluded-capture title
/// with `used_fallback = true` to stay consistent with the screenshot path.
pub fn capture_focused_window_metadata(
) -> Result<FocusedWindowMetadata, Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    use xcap::Window;

    let windows = Window::all()?;

    let mut target_window = None;
    let mut found_focused = false;
    let mut script_kit_is_frontmost = false;

    for window in windows {
        let app_name = window.app_name().unwrap_or_else(|_| String::new());
        let is_minimized = window.is_minimized().unwrap_or(true);
        let is_focused = window.is_focused().unwrap_or(false);

        let is_our_app = app_name.contains("script-kit-gpui")
            || app_name == "Script Kit"
            || app_name.contains("Script Kit");

        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);
        let is_reasonable_size = width >= 100 && height >= 100;

        if is_focused && is_our_app {
            script_kit_is_frontmost = true;
        }

        if !is_our_app && !is_minimized && is_reasonable_size {
            if is_focused {
                target_window = Some(window);
                found_focused = true;
                break;
            }
            if target_window.is_none() {
                target_window = Some(window);
            }
        }
    }

    if script_kit_is_frontmost {
        tracing::debug!(
            "Script Kit is frontmost — metadata uses excluded-capture title"
        );
        // Derive dimensions from the active display so metadata stays
        // consistent with the self-excluding screenshot path.
        #[cfg(target_os = "macos")]
        let (w, h) = {
            let bounds = capture_target_bounds();
            (bounds.size.width.max(0.0) as u32, bounds.size.height.max(0.0) as u32)
        };
        #[cfg(not(target_os = "macos"))]
        let (w, h) = (0u32, 0u32);

        return Ok(FocusedWindowMetadata {
            window_title: script_kit_excluded_capture_title(),
            width: w,
            height: h,
            used_fallback: true,
        });
    }

    let used_fallback = target_window.is_some() && !found_focused;

    let window = target_window.ok_or("No suitable window found for metadata")?;
    let title = window.title().unwrap_or_else(|_| "Unknown".to_string());
    let app_name = window.app_name().unwrap_or_else(|_| "Unknown".to_string());
    let width = window.width().unwrap_or(0);
    let height = window.height().unwrap_or(0);

    let display_title = if title.is_empty() {
        app_name.clone()
    } else {
        format!("{} - {}", app_name, title)
    };

    tracing::debug!(
        width,
        height,
        title = %display_title,
        used_fallback,
        "Focused window metadata captured (no screenshot)"
    );

    Ok(FocusedWindowMetadata {
        window_title: display_title,
        width,
        height,
        used_fallback,
    })
}

/// Capture a screenshot of Script Kit's own visible panel window.
///
/// Unlike `capture_focused_window_screenshot()`, this helper intentionally
/// targets our own window so Tab AI can send the launcher state to the harness.
///
/// # Returns
/// A `FocusedWindowCapture` on success with `used_fallback = false`.
pub fn capture_script_kit_panel_screenshot(
) -> Result<FocusedWindowCapture, Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)?;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use xcap::Window;

    let windows = Window::all()?;

    let mut best: Option<(u32, xcap::Window)> = None;
    for window in windows {
        let app_name = window.app_name().unwrap_or_else(|_| String::new());
        let is_minimized = window.is_minimized().unwrap_or(true);
        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);
        let area = width.saturating_mul(height);

        let is_our_app = app_name.contains("script-kit-gpui")
            || app_name == "Script Kit"
            || app_name.contains("Script Kit");

        if !is_our_app || is_minimized || width < 100 || height < 100 {
            continue;
        }

        let should_replace = best
            .as_ref()
            .map(|(best_area, _)| area > *best_area)
            .unwrap_or(true);
        if should_replace {
            best = Some((area, window));
        }
    }

    let (_, window) = best.ok_or("No Script Kit panel window found")?;
    let title = window.title().unwrap_or_else(|_| "Panel".to_string());
    let app_name = window.app_name().unwrap_or_else(|_| "Script Kit".to_string());

    tracing::debug!(
        app_name = %app_name,
        title = %title,
        "Capturing Script Kit panel screenshot"
    );

    let image = window.capture_image()?;
    let original_width = image.width();
    let original_height = image.height();

    // Scale down to 1x for efficiency (retina)
    let width = (original_width / 2).max(1);
    let height = (original_height / 2).max(1);
    let resized = image::imageops::resize(
        &image,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );

    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(&mut png_data);
    encoder.write_image(&resized, width, height, image::ExtendedColorType::Rgba8)?;

    let display_title = format!("Script Kit - {}", title);

    tracing::debug!(
        width = width,
        height = height,
        file_size = png_data.len(),
        title = %display_title,
        "Script Kit panel screenshot captured"
    );

    Ok(FocusedWindowCapture {
        png_data,
        width,
        height,
        window_title: display_title,
        used_fallback: false,
    })
}

/// Get the URL of the currently focused browser tab.
///
/// Supports Safari, Google Chrome, Arc, Brave, Edge, Chromium, Vivaldi, Opera.
///
/// Reads the frontmost app identity from the in-process
/// `frontmost_app_tracker` (populated by the NSWorkspace observer) instead of
/// spawning `osascript` to ask `System Events` who the frontmost app is. Only
/// when the tracked bundle identifier matches a supported browser does this
/// function spawn a single `osascript` to read the URL from the browser's
/// scripting dictionary. For non-browser frontmost apps, no subprocess is
/// spawned at all.
///
/// # Returns
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserUrlSource {
    FocusedBrowser,
    RunningBrowserFallback { app_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTabUrl {
    pub url: String,
    pub source: BrowserUrlSource,
}

#[cfg(target_os = "macos")]
pub fn get_focused_browser_tab_url() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let Some(tracked) = crate::frontmost_app_tracker::get_last_real_app() else {
        return Err(
            "Frontmost app tracker has no tracked app yet — cannot identify browser".into(),
        );
    };

    tracing::debug!(
        bundle_id = %tracked.bundle_id,
        name = %tracked.name,
        "frontmost tracker identified app for browser URL lookup"
    );

    // Match by bundle ID first (stable, not affected by localization), fall
    // back to matching the localized name as a best effort.
    let url_script = match tracked.bundle_id.as_str() {
        "com.apple.Safari" => r#"tell application "Safari" to return URL of front document"#,
        "com.google.Chrome" => {
            r#"tell application "Google Chrome" to return URL of active tab of front window"#
        }
        "company.thebrowser.Browser" => {
            r#"tell application "Arc" to return URL of active tab of front window"#
        }
        "com.brave.Browser" => {
            r#"tell application "Brave Browser" to return URL of active tab of front window"#
        }
        "com.microsoft.edgemac" => {
            r#"tell application "Microsoft Edge" to return URL of active tab of front window"#
        }
        "org.chromium.Chromium" => {
            r#"tell application "Chromium" to return URL of active tab of front window"#
        }
        "com.vivaldi.Vivaldi" => {
            r#"tell application "Vivaldi" to return URL of active tab of front window"#
        }
        "com.operasoftware.Opera" => {
            r#"tell application "Opera" to return URL of active tab of front window"#
        }
        "org.mozilla.firefox" | "org.mozilla.firefoxdeveloperedition" => {
            return Err(
                "Firefox doesn't fully support AppleScript for URL retrieval. Try Safari or Chrome.".into(),
            );
        }
        _ => {
            return Err(format!(
                "Frontmost app '{}' ({}) is not a supported browser. Supported: Safari, Chrome, Arc, Brave, Edge, Chromium, Vivaldi, Opera",
                tracked.name, tracked.bundle_id
            )
            .into());
        }
    };

    tracing::debug!(
        bundle_id = %tracked.bundle_id,
        "Getting URL from browser via single osascript call"
    );

    let url = crate::platform::run_osascript(url_script, "get_focused_browser_tab_url")
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("Failed to get URL from {}: {}", tracked.name, e).into()
        })?;

    let url = url.trim().to_string();

    if url.is_empty() {
        return Err(format!("No URL found in {}", tracked.name).into());
    }

    tracing::debug!(url = %url, app = %tracked.name, "Browser URL retrieved");

    Ok(url)
}

#[cfg(not(target_os = "macos"))]
pub fn get_focused_browser_tab_url() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Err("Browser URL retrieval is only supported on macOS".into())
}

#[cfg(target_os = "macos")]
pub fn get_any_browser_tab_url_with_source() -> Option<BrowserTabUrl> {
    if !native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery) { return None; }
    if let Ok(url) = get_focused_browser_tab_url() {
        return Some(BrowserTabUrl {
            url,
            source: BrowserUrlSource::FocusedBrowser,
        });
    }

    let fallback_browsers: &[(&str, &str)] = &[
        (
            "Google Chrome",
            r#"tell application "Google Chrome" to return URL of active tab of front window"#,
        ),
        (
            "Safari",
            r#"tell application "Safari" to return URL of front document"#,
        ),
        (
            "Arc",
            r#"tell application "Arc" to return URL of active tab of front window"#,
        ),
        (
            "Brave Browser",
            r#"tell application "Brave Browser" to return URL of active tab of front window"#,
        ),
        (
            "Microsoft Edge",
            r#"tell application "Microsoft Edge" to return URL of active tab of front window"#,
        ),
        (
            "Chromium",
            r#"tell application "Chromium" to return URL of active tab of front window"#,
        ),
        (
            "Vivaldi",
            r#"tell application "Vivaldi" to return URL of active tab of front window"#,
        ),
        (
            "Opera",
            r#"tell application "Opera" to return URL of active tab of front window"#,
        ),
    ];

    for (app_name, url_script) in fallback_browsers {
        let script = format!(
            "tell application \"System Events\" to set isRunning to exists process \"{app_name}\"\n\
             if isRunning then\n\
             try\n\
             {url_script}\n\
             end try\n\
             end if\n\
             return \"\""
        );
        let Ok(url) = crate::platform::run_osascript(&script, "get_any_browser_tab_url") else {
            continue;
        };
        let url = url.trim().to_string();
        if !url.is_empty() {
            return Some(BrowserTabUrl {
                url,
                source: BrowserUrlSource::RunningBrowserFallback {
                    app_name: (*app_name).to_string(),
                },
            });
        }
    }
    None
}

#[cfg(target_os = "macos")]
pub fn get_any_browser_tab_url() -> Option<String> {
    get_any_browser_tab_url_with_source().map(|hit| hit.url)
}

#[cfg(not(target_os = "macos"))]
pub fn get_any_browser_tab_url_with_source() -> Option<BrowserTabUrl> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn get_any_browser_tab_url() -> Option<String> {
    None
}

// ============================================================================
// Cursor Visibility
// ============================================================================

/// Hide the mouse cursor until the mouse moves.
///
/// This is the standard macOS pattern used by text editors to hide the cursor
/// while typing. The cursor will automatically reappear when the user moves
/// the mouse, with no additional code needed.
///
/// # macOS Behavior
///
/// Calls `[NSCursor setHiddenUntilMouseMoves:YES]` which:
/// - Immediately hides the system cursor
/// - Automatically shows the cursor when the mouse moves
/// - Is idempotent (safe to call multiple times)
///
/// # Other Platforms
///
/// No-op on non-macOS platforms.
#[cfg(target_os = "macos")]
pub fn hide_cursor_until_mouse_moves() {
    if !native_effect_allowed(crate::runtime_policy::ExternalEffect::NativeInput) { return; }
    // SAFETY: NSCursor.setHiddenUntilMouseMoves: is a class method that is
    // safe to call from any thread (it's one of the few AppKit methods that is).
    // It takes a BOOL value type and returns void.
    unsafe {
        // NSCursor.setHiddenUntilMouseMoves(YES) - hides cursor until mouse moves
        let _: () = msg_send![class!(NSCursor), setHiddenUntilMouseMoves: true];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_cursor_until_mouse_moves() {
    // No-op on non-macOS platforms
}

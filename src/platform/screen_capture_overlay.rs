// ============================================================================
// Interactive Screen Area Capture
// ============================================================================

/// Result of an interactive screen area capture.
pub struct ScreenAreaCapture {
    /// PNG-encoded image data of the selected region.
    pub png_data: Vec<u8>,
    /// Width of the captured region in pixels.
    pub width: u32,
    /// Height of the captured region in pixels.
    pub height: u32,
}

/// Capture a user-selected rectangular region of the screen.
///
/// On macOS, this launches the native `screencapture -i` tool which provides:
/// - A crosshair cursor for drag-to-select
/// - Semi-transparent backdrop with highlighted selection region
/// - Escape key to cancel
/// - Space bar to switch to window capture mode
///
/// # Returns
/// - `Ok(Some(ScreenAreaCapture))` if the user completed a selection
/// - `Ok(None)` if the user cancelled (pressed Escape)
/// - `Err(...)` on system errors
#[cfg(target_os = "macos")]
pub fn capture_screen_area(
) -> Result<Option<ScreenAreaCapture>, Box<dyn std::error::Error + Send + Sync>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)?;
    use std::process::Command;

    tracing::info!(action = "screen_area_capture_start", "Starting interactive screen area selection");

    // Create a temporary file for the capture output
    let temp_dir = std::env::temp_dir();
    let capture_path = temp_dir.join(format!(
        "scriptkit_area_capture_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    let capture_path_str = capture_path.to_string_lossy().to_string();

    tracing::debug!(path = %capture_path_str, "Capture temp file path");

    // Run screencapture -i (interactive selection mode)
    // -i: interactive mode (crosshair cursor, drag to select region)
    // -s: capture screen area (not window) — implicit with -i when user drags
    // The tool shows native macOS selection overlay with semi-transparent backdrop
    let output = Command::new("screencapture")
        .arg("-i") // interactive selection
        .arg(&capture_path_str)
        .output()?;

    // screencapture exits with status 1 if the user presses Escape
    if !output.status.success() {
        tracing::info!(
            action = "screen_area_capture_cancelled",
            exit_code = output.status.code(),
            "User cancelled screen area selection"
        );
        // Clean up temp file if it exists (shouldn't on cancel, but be safe)
        let _ = std::fs::remove_file(&capture_path);
        return Ok(None);
    }

    // Read the captured PNG
    if !capture_path.exists() {
        tracing::warn!(
            action = "screen_area_capture_no_file",
            path = %capture_path_str,
            "Capture completed but no file was created"
        );
        return Ok(None);
    }

    let png_data = std::fs::read(&capture_path)?;

    // Clean up temp file
    if let Err(e) = std::fs::remove_file(&capture_path) {
        tracing::warn!(
            action = "screen_area_capture_cleanup_failed",
            error = %e,
            "Failed to clean up temp capture file"
        );
    }

    if png_data.is_empty() {
        tracing::warn!(action = "screen_area_capture_empty", "Captured file was empty");
        return Ok(None);
    }

    // Decode PNG to get dimensions
    let reader = image::ImageReader::new(std::io::Cursor::new(&png_data))
        .with_guessed_format()?;
    let decoded = reader.decode()?;
    let width = decoded.width();
    let height = decoded.height();

    tracing::info!(
        action = "screen_area_capture_complete",
        width = width,
        height = height,
        file_size = png_data.len(),
        "Screen area captured successfully"
    );

    Ok(Some(ScreenAreaCapture {
        png_data,
        width,
        height,
    }))
}

#[cfg(not(target_os = "macos"))]
pub fn capture_screen_area(
) -> Result<Option<ScreenAreaCapture>, Box<dyn std::error::Error + Send + Sync>> {
    Err("Interactive screen area capture is only supported on macOS".into())
}

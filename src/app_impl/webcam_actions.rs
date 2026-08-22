use super::*;

impl ScriptListApp {
    pub(crate) fn webcam_photo_directory() -> std::path::PathBuf {
        if let Some(home) = dirs::home_dir() {
            let desktop = home.join("Desktop");
            if desktop.exists() {
                return desktop;
            }
        }

        let temp = std::env::temp_dir();
        if temp.exists() {
            temp
        } else {
            std::path::PathBuf::from("/tmp")
        }
    }

    fn save_private_webcam_photo_at(
        directory: &std::path::Path,
        timestamp: &str,
        png_data: &[u8],
    ) -> std::io::Result<std::path::PathBuf> {
        if timestamp.is_empty()
            || !timestamp
                .chars()
                .all(|character| character.is_ascii_digit() || character == '-')
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Webcam photo timestamp must be a safe filename component",
            ));
        }

        crate::atomic_file::write_private_unique_named_file(
            directory,
            &format!("webcam-photo-{timestamp}"),
            "png",
            png_data,
        )
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn encode_webcam_frame_to_png(
        pixel_buffer: &core_video::pixel_buffer::CVPixelBuffer,
    ) -> Result<Vec<u8>, String> {
        use image::ImageEncoder;

        let lock_flags = core_video::pixel_buffer::kCVPixelBufferLock_ReadOnly;
        let lock_status = pixel_buffer.lock_base_address(lock_flags);
        if lock_status != core_video::r#return::kCVReturnSuccess {
            return Err(format!(
                "Failed to lock webcam frame (status={})",
                lock_status
            ));
        }

        let result = (|| -> Result<Vec<u8>, String> {
            if !pixel_buffer.is_planar() || pixel_buffer.get_plane_count() < 2 {
                return Err("Webcam frame format is not NV12".to_string());
            }

            let width = pixel_buffer.get_width_of_plane(0);
            let height = pixel_buffer.get_height_of_plane(0);
            if width == 0 || height == 0 {
                return Err("Webcam frame is empty".to_string());
            }

            let y_stride = pixel_buffer.get_bytes_per_row_of_plane(0);
            let uv_stride = pixel_buffer.get_bytes_per_row_of_plane(1);
            let uv_height = pixel_buffer.get_height_of_plane(1);

            // SAFETY: The pixel buffer is locked for reading above and the plane index is valid (checked via is_planar + plane_count >= 2).
            let y_plane_ptr = unsafe { pixel_buffer.get_base_address_of_plane(0) as *const u8 };
            // SAFETY: Same lock guard as y_plane_ptr; plane 1 exists per the plane_count check.
            let uv_plane_ptr = unsafe { pixel_buffer.get_base_address_of_plane(1) as *const u8 };
            if y_plane_ptr.is_null() || uv_plane_ptr.is_null() {
                return Err("Webcam frame memory is unavailable".to_string());
            }

            let y_plane_len = y_stride.checked_mul(height).ok_or_else(|| {
                format!(
                    "Webcam Y plane size overflow (stride={} height={})",
                    y_stride, height
                )
            })?;
            let uv_plane_len = uv_stride.checked_mul(uv_height).ok_or_else(|| {
                format!(
                    "Webcam UV plane size overflow (stride={} height={})",
                    uv_stride, uv_height
                )
            })?;
            let rgb_len = width
                .checked_mul(height)
                .and_then(|pixels| pixels.checked_mul(3))
                .ok_or_else(|| {
                    format!(
                        "Webcam RGB buffer size overflow (width={} height={})",
                        width, height
                    )
                })?;

            // SAFETY: y_plane_ptr is non-null (checked above), and y_plane_len = y_stride * height is within the locked buffer.
            let y_plane = unsafe { std::slice::from_raw_parts(y_plane_ptr, y_plane_len) };
            // SAFETY: uv_plane_ptr is non-null (checked above), and uv_plane_len = uv_stride * uv_height is within the locked buffer.
            let uv_plane = unsafe { std::slice::from_raw_parts(uv_plane_ptr, uv_plane_len) };

            let mut rgb = vec![0u8; rgb_len];

            for y in 0..height {
                let y_row = y * y_stride;
                let uv_row = (y / 2) * uv_stride;

                for x in 0..width {
                    let y_val = y_plane[y_row + x] as f32;
                    let uv_idx = uv_row + (x / 2) * 2;
                    if uv_idx + 1 >= uv_plane.len() {
                        continue;
                    }

                    let u = uv_plane[uv_idx] as f32 - 128.0;
                    let v = uv_plane[uv_idx + 1] as f32 - 128.0;

                    let r = (y_val + 1.402 * v).clamp(0.0, 255.0) as u8;
                    let g = (y_val - 0.344_136 * u - 0.714_136 * v).clamp(0.0, 255.0) as u8;
                    let b = (y_val + 1.772 * u).clamp(0.0, 255.0) as u8;

                    let idx = (y * width + x) * 3;
                    rgb[idx] = r;
                    rgb[idx + 1] = g;
                    rgb[idx + 2] = b;
                }
            }

            let mut png_data = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
            let png_width = u32::try_from(width)
                .map_err(|_| format!("Webcam frame width out of range: {}", width))?;
            let png_height = u32::try_from(height)
                .map_err(|_| format!("Webcam frame height out of range: {}", height))?;
            encoder
                .write_image(&rgb, png_width, png_height, image::ColorType::Rgb8.into())
                .map_err(|e| format!("Failed to encode webcam frame: {}", e))?;

            Ok(png_data)
        })();

        let unlock_status = pixel_buffer.unlock_base_address(lock_flags);
        if unlock_status != core_video::r#return::kCVReturnSuccess {
            tracing::error!(
                status = unlock_status,
                "Failed to unlock webcam frame after capture"
            );
        }

        result
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn capture_webcam_photo(&mut self, cx: &mut Context<Self>) -> bool {
        let pixel_buffer = match &self.current_view {
            AppView::WebcamView { entity } => entity.read(cx).pixel_buffer.clone(),
            _ => None,
        };

        let Some(pixel_buffer) = pixel_buffer else {
            cx.notify();
            self.show_error_toast("No camera frame available yet", cx);
            return false;
        };

        let png_data = match Self::encode_webcam_frame_to_png(&pixel_buffer) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(error = %e, "Failed to capture webcam photo");
                cx.notify();
                self.show_error_toast(format!("Failed to capture photo: {}", e), cx);
                return false;
            }
        };

        let save_dir = Self::webcam_photo_directory();
        let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();

        match Self::save_private_webcam_photo_at(&save_dir, &timestamp, &png_data) {
            Ok(save_path) => {
                let safe_path =
                    crate::logging::log_private_user_value(&save_path.display().to_string());
                tracing::info!(
                    category = "ACTIONS",
                    path_bytes = safe_path.raw_bytes,
                    path_sha256 = %safe_path.sha256,
                    "Webcam photo saved"
                );
                cx.notify();
                self.show_hud(
                    format!("Photo saved to {}", save_path.display()),
                    Some(HUD_LONG_MS),
                    cx,
                );
                let save_path_str = save_path.to_string_lossy().to_string();
                if let Err(error) = crate::file_search::reveal_in_finder(&save_path_str) {
                    let safe_error = crate::logging::log_private_user_value(&error.to_string());
                    tracing::warn!(
                        path_bytes = safe_path.raw_bytes,
                        path_sha256 = %safe_path.sha256,
                        error_bytes = safe_error.raw_bytes,
                        error_sha256 = %safe_error.sha256,
                        "Failed to reveal saved webcam photo"
                    );
                }
                true
            }
            Err(error) => {
                tracing::error!(reason = ?error.kind(), "Failed to save private webcam photo");
                cx.notify();
                self.show_error_toast(
                    "Photo could not be saved safely. Check Desktop and try again.",
                    cx,
                );
                false
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn capture_webcam_photo(&mut self, cx: &mut Context<Self>) -> bool {
        tracing::warn!(
            category = "ACTIONS",
            "Webcam capture requested on unsupported platform"
        );
        cx.notify();
        self.show_unsupported_platform_toast("Webcam capture", cx);
        false
    }

    pub(crate) fn webcam_actions_for_dialog() -> Vec<crate::actions::Action> {
        use crate::actions::{Action, ActionCategory};

        vec![
            Action::new(
                "capture",
                "Capture Photo",
                Some("Take a photo".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵"),
            Action::new(
                "close",
                "Close",
                Some("Close webcam".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⎋"),
        ]
    }

    pub(crate) fn execute_webcam_action(
        &mut self,
        action_id: &str,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match action_id {
            "capture" => {
                tracing::info!(
                    category = "UI",
                    action = action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    "Webcam action triggered"
                );

                if self.capture_webcam_photo(cx) {
                    self.hide_main_and_reset(cx);
                    crate::action_helpers::DispatchOutcome::success()
                        .with_trace_id(dctx.trace_id.clone())
                        .with_detail("webcam_capture")
                } else {
                    crate::action_helpers::DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        "Failed to capture photo",
                    )
                    .with_trace_id(dctx.trace_id.clone())
                    .with_detail("webcam_capture_failed")
                }
            }
            "close" => {
                tracing::info!(
                    category = "UI",
                    action = action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    "Webcam action triggered"
                );

                cx.notify();
                self.show_hud("Webcam closed".to_string(), Some(HUD_SHORT_MS), cx);
                self.hide_main_and_reset(cx);

                crate::action_helpers::DispatchOutcome::success()
                    .with_trace_id(dctx.trace_id.clone())
                    .with_detail("webcam_close")
            }
            _ => {
                tracing::info!(
                    category = "UI",
                    action = action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    "Webcam action: unknown id, falling back to SDK action routing"
                );
                self.trigger_sdk_action_with_trace(action_id, &dctx.trace_id)
            }
        }
    }
}

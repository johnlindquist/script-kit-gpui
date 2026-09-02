//! Webcam prompt UI component — renders CVPixelBuffer via gpui::surface()
//!
//! This is the inner content component. The app container (render_webcam_prompt)
//! provides the standard chrome: vibrancy, footer with logo/Capture/Actions.

use core_video::pixel_buffer::CVPixelBuffer;
use gpui::{
    div, prelude::*, rgb, Context, FocusHandle, Focusable, ObjectFit, Render, Styled, Window,
};

use super::base::DesignContext;
use super::base::PromptBase;
use super::SubmitCallback;
use crate::camera::CaptureHandle;
use crate::theme;

/// Webcam prompt state
#[derive(Debug, Clone)]
pub enum WebcamState {
    Initializing,
    Live,
    Error(String),
}

/// Webcam prompt component — just the camera preview content.
/// The standard app container provides the footer, vibrancy, etc.
pub struct WebcamPrompt {
    pub base: PromptBase,
    pub state: WebcamState,
    pub mirror: bool,
    /// Latest CVPixelBuffer from camera — rendered via gpui::surface()
    pub pixel_buffer: Option<CVPixelBuffer>,
    pub frame_width: u32,
    pub frame_height: u32,
    /// Owns the AVFoundation capture session — dropped when prompt closes,
    /// which stops the camera and releases all resources.
    pub capture_handle: Option<CaptureHandle>,
}

impl WebcamPrompt {
    pub fn new(
        id: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: std::sync::Arc<theme::Theme>,
    ) -> Self {
        Self {
            base: PromptBase::new(id, focus_handle, on_submit, theme),
            state: WebcamState::Initializing,
            mirror: false,
            pixel_buffer: None,
            frame_width: 0,
            frame_height: 0,
            capture_handle: None,
        }
    }

    /// Feed an owned NV12 frame through the same CVPixelBuffer/Metal surface
    /// as live camera frames. This allocates media memory only; no device opens.
    pub fn from_nv12_frame(
        id: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: std::sync::Arc<theme::Theme>,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> anyhow::Result<Self> {
        use core_foundation::{
            base::{CFType, TCFType},
            boolean::CFBoolean,
            dictionary::CFDictionary,
            string::CFString,
        };
        use core_video::pixel_buffer::{
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, CVPixelBufferKeys,
        };
        anyhow::ensure!(
            width > 0
                && height > 0
                && width.is_multiple_of(2)
                && height.is_multiple_of(2)
                && u64::from(width) * u64::from(height) <= 4_194_304,
            "invalid_webcam_frame_size"
        );
        let row_bytes = width as usize;
        let luma_len = row_bytes * height as usize;
        anyhow::ensure!(
            bytes.len() == luma_len + luma_len / 2,
            "invalid_webcam_frame_bytes"
        );
        let surface_options = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]);
        let options = CFDictionary::from_CFType_pairs(&[
            (
                CFString::from(CVPixelBufferKeys::IOSurfaceProperties),
                surface_options.as_CFType(),
            ),
            (
                CFString::from(CVPixelBufferKeys::MetalCompatibility),
                CFBoolean::true_value().as_CFType(),
            ),
        ]);
        let buffer = CVPixelBuffer::new(
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            width as usize,
            height as usize,
            Some(&options),
        )
        .map_err(|code| anyhow::anyhow!("webcam_frame_allocation_failed:{code}"))?;
        anyhow::ensure!(buffer.lock_base_address(0) == 0, "webcam_frame_lock_failed");
        // SAFETY: both planes belong to a newly allocated, locked NV12 buffer.
        // Each copy fits CoreVideo's stride and the validated source slice.
        let copy_result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(
                buffer.is_planar() && buffer.get_plane_count() == 2,
                "webcam_frame_invalid_planes"
            );
            for plane in 0..2 {
                let rows = if plane == 0 {
                    height as usize
                } else {
                    height as usize / 2
                };
                let source_offset = if plane == 0 { 0 } else { luma_len };
                let stride = buffer.get_bytes_per_row_of_plane(plane);
                let base = unsafe { buffer.get_base_address_of_plane(plane).cast::<u8>() };
                anyhow::ensure!(
                    !base.is_null() && stride >= row_bytes,
                    "webcam_frame_invalid_storage"
                );
                for row in 0..rows {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr().add(source_offset + row * row_bytes),
                            base.add(row * stride),
                            row_bytes,
                        );
                    }
                }
            }
            Ok(())
        })();
        let unlocked = buffer.unlock_base_address(0);
        copy_result?;
        anyhow::ensure!(unlocked == 0, "webcam_frame_unlock_failed");
        let mut prompt = Self::new(id, focus_handle, on_submit, theme);
        prompt.pixel_buffer = Some(buffer);
        prompt.frame_width = width;
        prompt.frame_height = height;
        prompt.state = WebcamState::Live;
        Ok(prompt)
    }

    /// Set the latest CVPixelBuffer from camera (zero-copy)
    pub fn set_pixel_buffer(&mut self, buf: CVPixelBuffer, cx: &mut Context<Self>) {
        let Ok(frame_width) = u32::try_from(buf.get_width()) else {
            self.pixel_buffer = None;
            self.frame_width = 0;
            self.frame_height = 0;
            self.set_error("Webcam frame width exceeds supported range".to_string(), cx);
            return;
        };
        let Ok(frame_height) = u32::try_from(buf.get_height()) else {
            self.pixel_buffer = None;
            self.frame_width = 0;
            self.frame_height = 0;
            self.set_error(
                "Webcam frame height exceeds supported range".to_string(),
                cx,
            );
            return;
        };

        self.frame_width = frame_width;
        self.frame_height = frame_height;
        self.pixel_buffer = Some(buf);
        self.state = WebcamState::Live;
        cx.notify();
    }

    pub fn set_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.state = WebcamState::Error(message);
        cx.notify();
    }

    fn state_label(&self) -> String {
        match &self.state {
            WebcamState::Initializing => "Starting camera...".into(),
            WebcamState::Live => format!("{}x{}", self.frame_width, self.frame_height),
            WebcamState::Error(msg) => msg.clone(),
        }
    }
}

impl Focusable for WebcamPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.base.focus_handle.clone()
    }
}

impl Render for WebcamPrompt {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let dc = DesignContext::new(&self.base.theme, self.base.design_variant);
        let colors = self.base.theme.colors.prompt_colors();

        // Camera preview: use gpui::surface() for zero-copy GPU rendering
        if let Some(ref buf) = self.pixel_buffer {
            div().size_full().child(
                gpui::surface(buf.clone())
                    .object_fit(ObjectFit::Contain)
                    .w_full()
                    .h_full(),
            )
        } else {
            // Placeholder while camera initializes or on error
            div()
                .flex()
                .size_full()
                .items_center()
                .justify_center()
                .bg(dc.bg_secondary())
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors.text_secondary))
                        .child(self.state_label()),
                )
        }
    }
}

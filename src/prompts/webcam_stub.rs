//! Webcam prompt fallback UI for non-macOS platforms.
//!
//! The webcam command remains available so scripts can keep a stable surface,
//! but we show an explicit unsupported state instead of attempting capture.

use gpui::{div, prelude::*, rgb, Context, FocusHandle, Focusable, Render, Styled, Window};

use super::base::DesignContext;
use super::base::PromptBase;
use super::SubmitCallback;
use crate::theme;

#[derive(Debug, Clone)]
pub enum WebcamState {
    Unsupported(String),
    Error(String),
    Live,
}

pub struct WebcamPrompt {
    pub base: PromptBase,
    pub state: WebcamState,
    pub frame_width: u32,
    pub frame_height: u32,
    frame: Option<std::sync::Arc<gpui::RenderImage>>,
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
            state: WebcamState::Unsupported(
                "Webcam capture is not supported on this platform".to_string(),
            ),
            frame_width: 0,
            frame_height: 0,
            frame: None,
        }
    }

    pub fn from_nv12_frame(
        id: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: std::sync::Arc<theme::Theme>,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            width > 0
                && height > 0
                && width % 2 == 0
                && height % 2 == 0
                && u64::from(width) * u64::from(height) <= 4_194_304,
            "invalid_webcam_frame_size"
        );
        let luma_len = width as usize * height as usize;
        anyhow::ensure!(
            bytes.len() == luma_len + luma_len / 2,
            "invalid_webcam_frame_bytes"
        );
        let mut image = image::RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let luma = bytes[y as usize * width as usize + x as usize] as f32;
            let uv = luma_len + (y as usize / 2) * width as usize + (x as usize / 2) * 2;
            let u = bytes[uv] as f32 - 128.0;
            let v = bytes[uv + 1] as f32 - 128.0;
            let r = (luma + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (luma - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
            let b = (luma + 1.772 * u).clamp(0.0, 255.0) as u8;
            *pixel = image::Rgba([b, g, r, 255]);
        }
        let mut prompt = Self::new(id, focus_handle, on_submit, theme);
        prompt.frame = Some(std::sync::Arc::new(gpui::RenderImage::new(
            smallvec::smallvec![image::Frame::new(image)],
        )));
        prompt.frame_width = width;
        prompt.frame_height = height;
        prompt.state = WebcamState::Live;
        Ok(prompt)
    }

    pub fn set_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.state = WebcamState::Error(message);
        cx.notify();
    }

    fn state_label(&self) -> &str {
        match &self.state {
            WebcamState::Unsupported(msg) | WebcamState::Error(msg) => msg.as_str(),
            WebcamState::Live => "Synthetic frame",
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
        if let Some(frame) = &self.frame {
            return div().size_full().child(
                gpui::img(frame.clone())
                    .object_fit(gpui::ObjectFit::Contain)
                    .size_full(),
            );
        }

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

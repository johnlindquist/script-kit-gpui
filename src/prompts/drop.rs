//! DropPrompt - Drag and drop file handling
//!
//! Features:
//! - Drop zone for files
//! - Display dropped file information
//! - Submit file paths

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, Window,
};
use std::sync::Arc;

use crate::logging;
use crate::theme;
use crate::designs::{DesignVariant, get_tokens};

use super::SubmitCallback;

/// DropPrompt - Drag and drop file handling
///
/// Provides a drop zone for files with visual feedback.
/// Returns information about dropped files.
pub struct DropPrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Placeholder text to display
    pub placeholder: Option<String>,
    /// Hint text below the drop zone
    pub hint: Option<String>,
    /// List of dropped files
    pub dropped_files: Vec<DroppedFile>,
    /// Whether files are currently being dragged over
    pub is_drag_over: bool,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
}

/// Information about a dropped file
#[derive(Clone, Debug)]
pub struct DroppedFile {
    /// File path
    pub path: String,
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u64,
}

impl DropPrompt {
    pub fn new(
        id: String,
        placeholder: Option<String>,
        hint: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        logging::log("PROMPTS", &format!("DropPrompt::new id: {}", id));
        
        DropPrompt {
            id,
            placeholder,
            hint,
            dropped_files: Vec::new(),
            is_drag_over: false,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
        }
    }

    /// Submit dropped files as JSON array
    fn submit(&mut self) {
        if !self.dropped_files.is_empty() {
            // Serialize dropped files to JSON
            let files_json: Vec<serde_json::Value> = self.dropped_files.iter()
                .map(|f| serde_json::json!({
                    "path": f.path,
                    "name": f.name,
                    "size": f.size
                }))
                .collect();
            let json_str = serde_json::to_string(&files_json).unwrap_or_else(|_| "[]".to_string());
            (self.on_submit)(self.id.clone(), Some(json_str));
        }
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Handle file drop (would be called from GPUI drag/drop events)
    #[allow(dead_code)]
    fn handle_drop(&mut self, files: Vec<DroppedFile>, cx: &mut Context<Self>) {
        self.dropped_files = files;
        self.is_drag_over = false;
        cx.notify();
    }
}

impl Focusable for DropPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DropPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();

        let handle_key = cx.listener(|this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "enter" => this.submit(),
                "escape" => this.submit_cancel(),
                _ => {}
            }
        });

        let (main_bg, text_color, muted_color, border_color) = if self.design_variant == DesignVariant::Default {
            (
                rgb(self.theme.colors.background.main),
                rgb(self.theme.colors.text.secondary),
                rgb(self.theme.colors.text.muted),
                rgb(self.theme.colors.ui.border),
            )
        } else {
            (
                rgb(colors.background),
                rgb(colors.text_secondary),
                rgb(colors.text_muted),
                rgb(colors.border),
            )
        };

        let placeholder = self.placeholder.clone()
            .unwrap_or_else(|| "Drop files here".to_string());
        
        let hint = self.hint.clone()
            .unwrap_or_else(|| "Drag and drop files to upload".to_string());

        // Drop zone styling
        let drop_zone_bg = if self.is_drag_over {
            rgb(self.theme.colors.accent.selected_subtle)
        } else {
            rgb(self.theme.colors.background.search_box)
        };

        div()
            .id(gpui::ElementId::Name("window:drop".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(main_bg)
            .text_color(text_color)
            .p(px(spacing.padding_lg))
            .key_context("drop_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                // Drop zone
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(200.))
                    .bg(drop_zone_bg)
                    .border_2()
                    .border_color(border_color)
                    .rounded(px(8.))
                    .child(
                        div()
                            .text_2xl()
                            .child("📁")
                    )
                    .child(
                        div()
                            .mt(px(spacing.padding_md))
                            .text_lg()
                            .child(placeholder)
                    )
            )
            .child(
                div()
                    .mt(px(spacing.padding_md))
                    .text_sm()
                    .text_color(muted_color)
                    .child(hint)
            )
            .when(!self.dropped_files.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(spacing.padding_lg))
                        .text_sm()
                        .child(format!("{} file(s) dropped", self.dropped_files.len()))
                )
            })
    }
}

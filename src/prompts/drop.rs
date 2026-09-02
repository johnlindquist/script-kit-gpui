//! DropPrompt - Drag and drop file handling
//!
//! Features:
//! - Drop zone for files
//! - Display dropped file information
//! - Submit file paths

use gpui::{
    div, prelude::*, px, rgb, rgba, Context, ExternalPaths, FocusHandle, Focusable, Render, Window,
};
use std::{fs, path::Path, sync::Arc};

use crate::components::{FocusablePrompt, FocusablePromptInterceptedKey};
use crate::designs::{get_tokens, DesignVariant};
use crate::logging;
use crate::theme;
use crate::theme::ColorResolver;
use crate::ui_foundation::{get_vibrancy_background, is_key_enter};

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

impl DroppedFile {
    pub(crate) fn from_path(path: &Path) -> Self {
        let path_string = path.to_string_lossy().into_owned();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| path_string.clone());
        let size = fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        Self {
            path: path_string,
            name,
            size,
        }
    }

    pub(crate) fn automation_metadata(&self, index: usize) -> serde_json::Value {
        serde_json::json!({
            "index": index,
            "name": self.name,
            "size": self.size,
        })
    }
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

    pub fn with_owned_files(mut self, files: Vec<DroppedFile>) -> anyhow::Result<Self> {
        let policy = crate::runtime_policy::owned_evaluation()
            .ok_or_else(|| anyhow::anyhow!("owned_drop_policy_missing"))?;
        for file in &files {
            policy.require_owned_path(Path::new(&file.path))?;
            anyhow::ensure!(
                Path::new(&file.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some(file.name.as_str()),
                "owned_drop_name_mismatch"
            );
        }
        self.dropped_files = files;
        Ok(self)
    }

    /// Submit dropped files as JSON array
    pub(crate) fn submit(&mut self) {
        if !self.dropped_files.is_empty() {
            // Serialize dropped files to JSON
            let files_json: Vec<serde_json::Value> = self
                .dropped_files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "path": f.path,
                        "name": f.name,
                        "size": f.size
                    })
                })
                .collect();
            let json_str = serde_json::to_string(&files_json).unwrap_or_else(|_| "[]".to_string());
            (self.on_submit)(self.id.clone(), Some(json_str));
        }
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Handle native file drop paths from GPUI without reading file contents.
    pub(crate) fn handle_external_paths(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        if let Err(error) =
            crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        {
            self.hint = Some(error.to_string());
            self.is_drag_over = false;
            cx.notify();
            return;
        }
        let files: Vec<DroppedFile> = paths
            .paths()
            .iter()
            .map(|path| DroppedFile::from_path(path.as_path()))
            .collect();

        self.is_drag_over = false;
        if files.is_empty() {
            cx.notify();
            return;
        }

        tracing::info!(
            target: "script_kit::drop_prompt",
            event = "native_files_dropped",
            file_count = files.len(),
            "DropPrompt received native file drop"
        );
        self.handle_drop(files, cx);
    }

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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let spacing = tokens.spacing();

        // VIBRANCY: Use foundation helper - returns None when vibrancy enabled (let Root handle bg)
        let vibrancy_bg = get_vibrancy_background(&self.theme);

        let chrome = theme::AppChromeColors::from_theme(&self.theme);
        let shell_colors = ColorResolver::new_for_shell(&self.theme, self.design_variant);
        let text_color = rgb(shell_colors.primary_text_color());
        let muted_color = rgb(shell_colors.empty_text_color());

        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| "Drop files here".to_string());

        let hint = self
            .hint
            .clone()
            .unwrap_or_else(|| "Drag and drop files to upload".to_string());

        // Whisper chrome: use shared drop-target contract from AppChromeColors.
        // The theme-side drop target colors are derived from OPACITY_GHOST /
        // OPACITY_GHOST_SOFT so this prompt stays on the shared whisper scale.
        let (drop_zone_bg, drop_zone_border) = if self.is_drag_over {
            (
                rgba(chrome.drop_target_active_bg_rgba),
                rgba(chrome.drop_target_active_border_rgba),
            )
        } else {
            (
                rgba(chrome.drop_target_bg_rgba),
                rgba(chrome.drop_target_border_rgba),
            )
        };
        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(
                // Drop zone — whisper chrome: ghost bg, hairline border, no rounding
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(200.))
                    .bg(drop_zone_bg)
                    .border_1()
                    .border_color(drop_zone_border)
                    .child(div().text_2xl().child("📁"))
                    .child(
                        div()
                            .mt(px(spacing.padding_md))
                            .text_lg()
                            .child(placeholder),
                    ),
            )
            .child(
                div()
                    .mt(px(spacing.padding_md))
                    .text_sm()
                    .text_color(muted_color)
                    .child(hint),
            )
            .when(!self.dropped_files.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(spacing.padding_lg))
                        .text_sm()
                        .child(format!("{} file(s) dropped", self.dropped_files.len())),
                )
            });

        let container = crate::components::render_inset_prompt_body(
            "window:drop",
            body,
            crate::components::PromptBodyInsets::MainMenu(self.design_variant),
        )
        .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
            this.handle_external_paths(paths, cx);
        }))
        .when_some(vibrancy_bg, |d, bg| d.bg(bg))
        .text_color(text_color);

        FocusablePrompt::new(container)
            .key_context("drop_prompt")
            .focus_handle(self.focus_handle.clone())
            .build(
                window,
                cx,
                |this, intercepted_key, _event, _window, _cx| match intercepted_key {
                    FocusablePromptInterceptedKey::Escape => {
                        this.submit_cancel();
                        true
                    }
                    _ => false,
                },
                |this, event, _window, _cx| {
                    let key_str = event.keystroke.key.as_str();
                    if is_key_enter(key_str) {
                        this.submit();
                    }
                },
            )
    }
}

#[cfg(test)]
mod owned_metadata_tests {
    use super::*;

    #[gpui::test]
    fn drop_metadata_reaches_real_submit_handler(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let submitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let output = submitted.clone();
        let prompt = cx.new(|cx| {
            DropPrompt::new(
                "drop-local".into(),
                None,
                None,
                cx.focus_handle(),
                Arc::new(move |_, value| output.lock().push(value)),
                Arc::new(theme::Theme::default()),
            )
        });
        prompt.update(cx, |prompt, cx| {
            prompt.submit();
            assert!(submitted.lock().is_empty());
            prompt.handle_drop(
                vec![DroppedFile {
                    path: "/owned/fixture.txt".into(),
                    name: "fixture.txt".into(),
                    size: 6,
                }],
                cx,
            );
            prompt.submit();
        });
        let result: serde_json::Value =
            serde_json::from_str(submitted.lock()[0].as_deref().unwrap()).unwrap();
        assert_eq!(result[0]["size"], 6);
        assert_eq!(result[0]["name"], "fixture.txt");
    }
}

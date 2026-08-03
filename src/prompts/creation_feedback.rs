//! CreationFeedbackPanel - persistent "Created" feedback UI for newly created paths.
//!
//! This panel is intentionally inline and callback-driven so the app layer can wire
//! platform-specific behavior for each action.

use gpui::{div, prelude::*, px, rgb, App, RenderOnce, SharedString, Window};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::components::button::{Button, ButtonColors, ButtonVariant};
use crate::designs::DesignVariant;
use crate::theme;

/// Callback for path-based quick actions from the creation feedback panel.
pub type CreationFeedbackPathAction = Box<dyn Fn(&PathBuf, &mut Window, &mut App) + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CreationFeedbackArtifactKind {
    LocalArtifact,
    GeneratedScript,
}

impl CreationFeedbackArtifactKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::LocalArtifact => "Local artifact",
            Self::GeneratedScript => "Generated script",
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            Self::LocalArtifact => "local_artifact",
            Self::GeneratedScript => "generated_script",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CreationFeedbackReceiptStatus {
    Present,
    Missing,
    Invalid,
    NotApplicable,
}

impl CreationFeedbackReceiptStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Present => "Receipt present",
            Self::Missing => "Generated-script receipt missing",
            Self::Invalid => "Generated-script receipt unreadable",
            Self::NotApplicable => "Receipt not applicable",
        }
    }

    pub fn kind(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::NotApplicable => "not_applicable",
        }
    }

    pub fn from_fixture_str(value: &str) -> Self {
        match value {
            "present" | "Present" => Self::Present,
            "missing" | "Missing" => Self::Missing,
            "invalid" | "Invalid" => Self::Invalid,
            "notApplicable" | "not_applicable" | "NotApplicable" => Self::NotApplicable,
            _ => Self::Invalid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationFeedbackReceiptSummary {
    pub receipt_path: PathBuf,
    pub receipt_status: CreationFeedbackReceiptStatus,
    pub verification_status: Option<crate::ai::GeneratedScriptVerificationStatus>,
    pub command_kind: Option<String>,
    pub exit_code: Option<i32>,
    pub diagnostics: Vec<String>,
}

impl CreationFeedbackReceiptSummary {
    pub fn verification_status_label(&self) -> &'static str {
        match self.verification_status {
            Some(crate::ai::GeneratedScriptVerificationStatus::Passed) => "Verification passed",
            Some(crate::ai::GeneratedScriptVerificationStatus::Failed) => "Verification failed",
            Some(crate::ai::GeneratedScriptVerificationStatus::Skipped) => "Verification skipped",
            Some(crate::ai::GeneratedScriptVerificationStatus::Blocked) => "Verification blocked",
            None => self.receipt_status.label(),
        }
    }

    pub fn verification_status_kind(&self) -> &'static str {
        match self.verification_status {
            Some(crate::ai::GeneratedScriptVerificationStatus::Passed) => "passed",
            Some(crate::ai::GeneratedScriptVerificationStatus::Failed) => "failed",
            Some(crate::ai::GeneratedScriptVerificationStatus::Skipped) => "skipped",
            Some(crate::ai::GeneratedScriptVerificationStatus::Blocked) => "blocked",
            None => self.receipt_status.kind(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationFeedbackPayload {
    pub artifact_path: PathBuf,
    pub artifact_kind: CreationFeedbackArtifactKind,
    pub receipt: Option<CreationFeedbackReceiptSummary>,
}

impl CreationFeedbackPayload {
    pub fn local_artifact(path: PathBuf) -> Self {
        Self {
            artifact_path: path,
            artifact_kind: CreationFeedbackArtifactKind::LocalArtifact,
            receipt: None,
        }
    }

    pub fn generated_script(path: PathBuf) -> Self {
        let receipt_path = crate::ai::generated_script_receipt_path(&path);
        let receipt = match fs::read_to_string(&receipt_path) {
            Ok(json) => match serde_json::from_str::<crate::ai::GeneratedScriptReceipt>(&json) {
                Ok(receipt) => Some(CreationFeedbackReceiptSummary {
                    receipt_path,
                    receipt_status: CreationFeedbackReceiptStatus::Present,
                    verification_status: Some(receipt.verification.status),
                    command_kind: Some(receipt.verification.command_kind),
                    exit_code: receipt.verification.exit_code,
                    diagnostics: receipt.verification.diagnostics,
                }),
                Err(error) => Some(CreationFeedbackReceiptSummary {
                    receipt_path,
                    receipt_status: CreationFeedbackReceiptStatus::Invalid,
                    verification_status: None,
                    command_kind: None,
                    exit_code: None,
                    diagnostics: vec![error.to_string()],
                }),
            },
            Err(_) => Some(CreationFeedbackReceiptSummary {
                receipt_path,
                receipt_status: CreationFeedbackReceiptStatus::Missing,
                verification_status: None,
                command_kind: None,
                exit_code: None,
                diagnostics: Vec::new(),
            }),
        };

        Self {
            artifact_path: path,
            artifact_kind: CreationFeedbackArtifactKind::GeneratedScript,
            receipt,
        }
    }

    pub fn fixture(
        artifact_path: PathBuf,
        receipt_path: Option<PathBuf>,
        receipt_status: Option<CreationFeedbackReceiptStatus>,
        verification_status: Option<crate::ai::GeneratedScriptVerificationStatus>,
    ) -> Self {
        match (receipt_path, receipt_status, verification_status) {
            (None, None, None) => Self::local_artifact(artifact_path),
            (receipt_path, receipt_status, verification_status) => {
                let receipt_path = receipt_path
                    .unwrap_or_else(|| crate::ai::generated_script_receipt_path(&artifact_path));
                Self {
                    artifact_path,
                    artifact_kind: CreationFeedbackArtifactKind::GeneratedScript,
                    receipt: Some(CreationFeedbackReceiptSummary {
                        receipt_path,
                        receipt_status: receipt_status
                            .unwrap_or(CreationFeedbackReceiptStatus::Present),
                        verification_status,
                        command_kind: None,
                        exit_code: None,
                        diagnostics: Vec::new(),
                    }),
                }
            }
        }
    }

    pub fn artifact_path_text(&self) -> SharedString {
        self.artifact_path.to_string_lossy().to_string().into()
    }

    pub fn artifact_kind_label(&self) -> &'static str {
        self.artifact_kind.label()
    }

    pub fn receipt_path(&self) -> Option<&PathBuf> {
        self.receipt.as_ref().map(|receipt| &receipt.receipt_path)
    }

    pub fn receipt_path_text(&self) -> SharedString {
        self.receipt_path()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| "No receipt for local artifact".to_string())
            .into()
    }

    pub fn receipt_status_label(&self) -> &'static str {
        self.receipt
            .as_ref()
            .map(|receipt| receipt.receipt_status.label())
            .unwrap_or(CreationFeedbackReceiptStatus::NotApplicable.label())
    }

    pub fn receipt_status_kind(&self) -> &'static str {
        self.receipt
            .as_ref()
            .map(|receipt| receipt.receipt_status.kind())
            .unwrap_or(CreationFeedbackReceiptStatus::NotApplicable.kind())
    }

    pub fn verification_status_label(&self) -> &'static str {
        self.receipt
            .as_ref()
            .map(CreationFeedbackReceiptSummary::verification_status_label)
            .unwrap_or(CreationFeedbackReceiptStatus::NotApplicable.label())
    }

    pub fn verification_status_kind(&self) -> &'static str {
        self.receipt
            .as_ref()
            .map(CreationFeedbackReceiptSummary::verification_status_kind)
            .unwrap_or(CreationFeedbackReceiptStatus::NotApplicable.kind())
    }

    pub fn run_disabled_reason(&self) -> &'static str {
        if !self.artifact_path.exists() {
            return "missing_artifact";
        }

        if self.artifact_kind == CreationFeedbackArtifactKind::GeneratedScript
            && self
                .receipt
                .as_ref()
                .and_then(|receipt| receipt.verification_status)
                != Some(crate::ai::GeneratedScriptVerificationStatus::Passed)
        {
            return "verification_not_passed";
        }

        "run_action_unavailable"
    }
}

/// Inline panel that renders post-creation feedback and quick path actions.
#[derive(IntoElement)]
pub struct CreationFeedbackPanel {
    payload: CreationFeedbackPayload,
    theme: Arc<theme::Theme>,
    design_variant: DesignVariant,
    on_reveal_artifact: Option<CreationFeedbackPathAction>,
    on_copy_artifact_path: Option<CreationFeedbackPathAction>,
    on_edit_artifact: Option<CreationFeedbackPathAction>,
    on_run_artifact: Option<CreationFeedbackPathAction>,
    on_copy_receipt_path: Option<CreationFeedbackPathAction>,
    on_open_receipt: Option<CreationFeedbackPathAction>,
}

impl CreationFeedbackPanel {
    pub fn new(payload: CreationFeedbackPayload, theme: Arc<theme::Theme>) -> Self {
        tracing::debug!(
            path = %payload.artifact_path.display(),
            "creation_feedback_panel_initialized"
        );
        Self {
            payload,
            theme,
            design_variant: DesignVariant::Default,
            on_reveal_artifact: None,
            on_copy_artifact_path: None,
            on_edit_artifact: None,
            on_run_artifact: None,
            on_copy_receipt_path: None,
            on_open_receipt: None,
        }
    }

    pub fn design_variant(mut self, design_variant: DesignVariant) -> Self {
        self.design_variant = design_variant;
        self
    }

    pub fn on_reveal_artifact(mut self, callback: CreationFeedbackPathAction) -> Self {
        self.on_reveal_artifact = Some(callback);
        self
    }

    pub fn on_copy_artifact_path(mut self, callback: CreationFeedbackPathAction) -> Self {
        self.on_copy_artifact_path = Some(callback);
        self
    }

    pub fn on_edit_artifact(mut self, callback: CreationFeedbackPathAction) -> Self {
        self.on_edit_artifact = Some(callback);
        self
    }

    pub fn on_run_artifact(mut self, callback: CreationFeedbackPathAction) -> Self {
        self.on_run_artifact = Some(callback);
        self
    }

    pub fn on_copy_receipt_path(mut self, callback: CreationFeedbackPathAction) -> Self {
        self.on_copy_receipt_path = Some(callback);
        self
    }

    pub fn on_open_receipt(mut self, callback: CreationFeedbackPathAction) -> Self {
        self.on_open_receipt = Some(callback);
        self
    }

    fn path_button(
        label: &'static str,
        path: PathBuf,
        button_colors: ButtonColors,
        callback: Option<CreationFeedbackPathAction>,
        event_name: &'static str,
    ) -> Button {
        match callback {
            Some(callback) => Button::new(
                format!("creation-feedback:{event_name}"),
                label,
                button_colors,
            )
            .variant(ButtonVariant::Ghost)
            .on_click(Box::new(move |_event, window, cx| {
                tracing::debug!(
                    path = %path.display(),
                    event = event_name,
                    "creation_feedback_panel_action"
                );
                callback(&path, window, cx);
            })),
            None => Button::new(
                format!("creation-feedback:{event_name}"),
                label,
                button_colors,
            )
            .variant(ButtonVariant::Ghost)
            .disabled(true),
        }
    }
}

impl RenderOnce for CreationFeedbackPanel {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let artifact_path_text = self.payload.artifact_path_text();
        let receipt_path_text = self.payload.receipt_path_text();
        let artifact_kind = self.payload.artifact_kind_label();
        let verification_status = self.payload.verification_status_label();
        let receipt_status = self.payload.receipt_status_label();
        let run_disabled_reason = self.payload.run_disabled_reason();
        let CreationFeedbackPanel {
            payload,
            theme,
            design_variant,
            on_reveal_artifact,
            on_copy_artifact_path,
            on_edit_artifact,
            on_run_artifact,
            on_copy_receipt_path,
            on_open_receipt,
        } = self;

        let text_primary = rgb(theme.colors.text.primary);
        let text_secondary = rgb(theme.colors.text.secondary);
        let text_muted = rgb(theme.colors.text.muted);
        let path_style = crate::components::prompt_field_style(
            &theme,
            crate::components::PromptFieldState::ReadOnly,
            false,
        );
        let button_colors = ButtonColors::from_theme(&theme);

        let artifact_path = payload.artifact_path.clone();
        let receipt_path = payload.receipt_path().cloned();
        let reveal_button = Self::path_button(
            "Reveal in Finder",
            artifact_path.clone(),
            button_colors,
            on_reveal_artifact,
            "reveal_artifact",
        );
        let copy_artifact_path_button = Self::path_button(
            "Copy Path",
            artifact_path.clone(),
            button_colors,
            on_copy_artifact_path,
            "copy_artifact_path",
        );
        let edit_button = Self::path_button(
            "Edit",
            artifact_path.clone(),
            button_colors,
            on_edit_artifact,
            "edit_artifact",
        );
        let run_button = match on_run_artifact {
            Some(callback) if run_disabled_reason.is_empty() => {
                Button::new("creation-feedback:run-artifact", "Run", button_colors)
                    .variant(ButtonVariant::Ghost)
                    .on_click(Box::new(move |_event, window, cx| {
                        callback(&artifact_path, window, cx);
                    }))
            }
            _ => Button::new("creation-feedback:run-artifact", "Run", button_colors)
                .variant(ButtonVariant::Ghost)
                .disabled(true),
        };
        let copy_receipt_path_button = match (receipt_path.clone(), on_copy_receipt_path) {
            (Some(path), callback) => Self::path_button(
                "Copy Receipt Path",
                path,
                button_colors,
                callback,
                "copy_receipt_path",
            ),
            (None, _) => Button::new(
                "creation-feedback:copy-receipt-path",
                "Copy Receipt Path",
                button_colors,
            )
            .variant(ButtonVariant::Ghost)
            .disabled(true),
        };
        let open_receipt_button = match (receipt_path, on_open_receipt) {
            (Some(path), callback) => Self::path_button(
                "Open Receipt",
                path,
                button_colors,
                callback,
                "open_receipt",
            ),
            (None, _) => Button::new(
                "creation-feedback:open-receipt",
                "Open Receipt",
                button_colors,
            )
            .variant(ButtonVariant::Ghost)
            .disabled(true),
        };

        let tokens = crate::designs::get_tokens(design_variant);
        let spacing = tokens.spacing();

        div()
            .id("creation-feedback-panel")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(spacing.gap_lg))
            .child(crate::components::prompt_form_intro(
                "Created",
                "Your new file is ready. Use the local actions below to inspect it.",
                text_primary,
                text_muted,
                spacing.gap_sm,
            ))
            .child(crate::components::prompt_form_section(
                artifact_kind,
                text_secondary,
                spacing.gap_sm,
                crate::components::prompt_surface(path_style.background, path_style.border).child(
                    crate::components::prompt_scroll_value(artifact_path_text, text_primary),
                ),
            ))
            .child(crate::components::prompt_form_section(
                "Verification",
                text_secondary,
                spacing.gap_sm,
                crate::components::prompt_surface(path_style.background, path_style.border).child(
                    crate::components::prompt_scroll_value(
                        SharedString::from(verification_status),
                        text_primary,
                    ),
                ),
            ))
            .child(crate::components::prompt_form_section(
                receipt_status,
                text_secondary,
                spacing.gap_sm,
                crate::components::prompt_surface(path_style.background, path_style.border).child(
                    crate::components::prompt_scroll_value(receipt_path_text, text_primary),
                ),
            ))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(spacing.gap_md))
                    .child(reveal_button)
                    .child(copy_artifact_path_button)
                    .child(edit_button)
                    .child(run_button),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(spacing.gap_md))
                    .child(copy_receipt_path_button)
                    .child(open_receipt_button),
            )
    }
}

#[cfg(test)]
mod create_flow_layout_tests {
    const SOURCE: &str = include_str!("creation_feedback.rs");

    #[test]
    fn creation_feedback_uses_shared_create_flow_helpers() {
        assert!(
            SOURCE.contains("prompt_form_intro("),
            "creation_feedback.rs should use prompt_form_intro"
        );
        assert!(
            SOURCE.contains("prompt_form_section("),
            "creation_feedback.rs should use prompt_form_section"
        );
    }

    #[test]
    fn creation_feedback_uses_prompt_field_style() {
        let production_code = SOURCE.split("#[cfg(test)]").next().unwrap_or(SOURCE);
        assert!(
            production_code.contains("prompt_field_style("),
            "creation_feedback.rs should use prompt_field_style instead of inline color math"
        );
    }

    #[test]
    fn creation_feedback_uses_scroll_value() {
        let production_code = SOURCE.split("#[cfg(test)]").next().unwrap_or(SOURCE);
        assert!(
            production_code.contains("prompt_scroll_value("),
            "creation_feedback.rs should use prompt_scroll_value for long-path handling"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation_feedback_panel_defaults_to_no_callbacks() {
        let panel = CreationFeedbackPanel::new(
            CreationFeedbackPayload::local_artifact(PathBuf::from("/tmp/new-script.ts")),
            Arc::new(theme::Theme::default()),
        );

        assert!(panel.on_reveal_artifact.is_none());
        assert!(panel.on_copy_artifact_path.is_none());
        assert!(panel.on_edit_artifact.is_none());
    }

    #[test]
    fn test_creation_feedback_panel_sets_callbacks_when_provided() {
        let panel = CreationFeedbackPanel::new(
            CreationFeedbackPayload::local_artifact(PathBuf::from("/tmp/new-extension")),
            Arc::new(theme::Theme::default()),
        )
        .on_reveal_artifact(Box::new(|_, _, _| {}))
        .on_copy_artifact_path(Box::new(|_, _, _| {}))
        .on_edit_artifact(Box::new(|_, _, _| {}));

        assert!(panel.on_reveal_artifact.is_some());
        assert!(panel.on_copy_artifact_path.is_some());
        assert!(panel.on_edit_artifact.is_some());
    }

    #[test]
    fn test_creation_feedback_panel_path_text_returns_full_path() {
        let panel = CreationFeedbackPanel::new(
            CreationFeedbackPayload::local_artifact(PathBuf::from(
                "/tmp/projects/script-kit/new-script.ts",
            )),
            Arc::new(theme::Theme::default()),
        );

        assert_eq!(
            panel.payload.artifact_path_text().to_string(),
            "/tmp/projects/script-kit/new-script.ts"
        );
    }

    #[test]
    fn generated_script_missing_receipt_reports_honest_status() {
        let payload =
            CreationFeedbackPayload::generated_script(PathBuf::from("/tmp/no-receipt-script.ts"));

        assert_eq!(payload.artifact_kind_label(), "Generated script");
        assert_eq!(
            payload.receipt_status_label(),
            "Generated-script receipt missing"
        );
        assert_eq!(
            payload.verification_status_label(),
            "Generated-script receipt missing"
        );
    }

    #[test]
    fn fixture_maps_verification_status_label() {
        let payload = CreationFeedbackPayload::fixture(
            PathBuf::from("/tmp/fixture.ts"),
            Some(PathBuf::from("/tmp/fixture.scriptkit.json")),
            Some(CreationFeedbackReceiptStatus::Present),
            Some(crate::ai::GeneratedScriptVerificationStatus::Blocked),
        );

        assert_eq!(payload.receipt_status_label(), "Receipt present");
        assert_eq!(payload.verification_status_label(), "Verification blocked");
        assert_eq!(payload.verification_status_kind(), "blocked");
    }
}

/// Summary information for previewing a context part before submission.
///
/// Derived synchronously from the `AiContextPart` metadata — no network
/// calls or file reads. This is the data model behind the pre-submit
/// preview affordance on each context chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextPreviewInfo {
    pub label: String,
    pub source_uri: String,
    pub profile: ContextPreviewProfile,
    pub has_diagnostics: bool,
    pub description: String,
}

/// Identifies the breadth of a context resource for visual differentiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextPreviewProfile {
    /// `kit://context?profile=minimal` — excludes selected text and menu bar.
    Minimal,
    /// `kit://context` (default) or `?profile=full` — all fields captured.
    Full,
    /// Custom per-field flags (e.g. `?selectedText=1&browserUrl=0`).
    Custom,
    /// Local file attachment — not a resource URI.
    FilePath,
}

impl std::fmt::Display for ContextPreviewProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Minimal => write!(f, "minimal"),
            Self::Full => write!(f, "full"),
            Self::Custom => write!(f, "custom"),
            Self::FilePath => write!(f, "file"),
        }
    }
}

/// Derive a preview summary from an `AiContextPart` without any I/O.
pub(crate) fn derive_context_preview_info(
    part: &crate::ai::message_parts::AiContextPart,
) -> ContextPreviewInfo {
    match part {
        crate::ai::message_parts::AiContextPart::ResourceUri { uri, label } => {
            let profile = classify_profile(uri);
            let has_diagnostics = uri.contains("diagnostics=1");
            let description = build_resource_description(uri, profile, has_diagnostics);

            tracing::info!(
                checkpoint = "context_preview_derived",
                label = %label,
                uri = %uri,
                profile = %profile,
                has_diagnostics = has_diagnostics,
                "derived context preview info"
            );

            ContextPreviewInfo {
                label: label.clone(),
                source_uri: uri.clone(),
                profile,
                has_diagnostics,
                description,
            }
        }
        crate::ai::message_parts::AiContextPart::FilePath { path, label } => {
            let file_size = std::fs::metadata(path)
                .map(|m| format_byte_size(m.len()))
                .unwrap_or_else(|_| "unknown size".to_string());

            tracing::info!(
                checkpoint = "context_preview_derived",
                label = %label,
                path = %path,
                profile = "file",
                "derived file context preview info"
            );

            ContextPreviewInfo {
                label: label.clone(),
                source_uri: path.clone(),
                profile: ContextPreviewProfile::FilePath,
                has_diagnostics: false,
                description: format!("File attachment ({file_size})"),
            }
        }
        crate::ai::message_parts::AiContextPart::SkillFile {
            path,
            label,
            owner_label,
            ..
        } => {
            let file_size = std::fs::metadata(path)
                .map(|m| format_byte_size(m.len()))
                .unwrap_or_else(|_| "unknown size".to_string());

            tracing::info!(
                checkpoint = "context_preview_derived",
                label = %label,
                path = %path,
                owner_label = %owner_label,
                profile = "skill",
                "derived skill context preview info"
            );

            ContextPreviewInfo {
                label: label.clone(),
                source_uri: path.clone(),
                profile: ContextPreviewProfile::FilePath,
                has_diagnostics: false,
                description: format!("Skill attachment from {owner_label} ({file_size})"),
            }
        }
        crate::ai::message_parts::AiContextPart::FocusedTarget { target, label } => {
            tracing::info!(
                checkpoint = "context_preview_derived",
                label = %label,
                source = %target.source,
                kind = %target.kind,
                semantic_id = %target.semantic_id,
                profile = "focused_target",
                "derived focused target context preview info"
            );

            ContextPreviewInfo {
                label: label.clone(),
                source_uri: format!("focused-target://{}:{}", target.source, target.semantic_id),
                profile: ContextPreviewProfile::Custom,
                has_diagnostics: false,
                description: format!("Focused {} from {}", target.kind, target.source),
            }
        }
        crate::ai::message_parts::AiContextPart::AmbientContext { label } => {
            tracing::info!(
                checkpoint = "context_preview_derived",
                label = %label,
                profile = "ambient",
                "derived ambient context preview info"
            );

            ContextPreviewInfo {
                label: label.clone(),
                source_uri: "ambient://ask-anything".to_string(),
                profile: ContextPreviewProfile::Custom,
                has_diagnostics: false,
                description: "Ambient desktop context (staged separately)".to_string(),
            }
        }
        crate::ai::message_parts::AiContextPart::TextBlock {
            label,
            source,
            text,
            mime_type,
        } => {
            let mime = mime_type.as_deref().unwrap_or("text/plain");
            let size = format_byte_size(text.len() as u64);

            tracing::info!(
                checkpoint = "context_preview_derived",
                label = %label,
                source = %source,
                mime_type = %mime,
                profile = "text_block",
                "derived text block context preview info"
            );

            ContextPreviewInfo {
                label: label.clone(),
                source_uri: source.clone(),
                profile: ContextPreviewProfile::Custom,
                has_diagnostics: false,
                description: format!("Text block ({mime}, {size})"),
            }
        }
    }
}

fn classify_profile(uri: &str) -> ContextPreviewProfile {
    if uri.contains("profile=minimal") {
        ContextPreviewProfile::Minimal
    } else if uri.contains("profile=full") || uri == "kit://context" {
        // Bare `kit://context` is the full profile (default when no profile param)
        ContextPreviewProfile::Full
    } else if uri.starts_with("kit://context") {
        // Has per-field flags but no explicit profile
        ContextPreviewProfile::Custom
    } else {
        ContextPreviewProfile::Custom
    }
}

fn build_resource_description(
    uri: &str,
    profile: ContextPreviewProfile,
    has_diagnostics: bool,
) -> String {
    let mut lines = Vec::new();

    match profile {
        ContextPreviewProfile::Minimal => {
            lines.push("Captures: frontmost app, browser URL, focused window".to_string());
            lines.push("Excludes: selected text, menu bar".to_string());
        }
        ContextPreviewProfile::Full => {
            lines.push(
                "Captures all fields: frontmost app, browser URL, focused window, selected text, menu bar"
                    .to_string(),
            );
        }
        ContextPreviewProfile::Custom => {
            let mut fields = Vec::new();
            if uri.contains("selectedText=1") {
                fields.push("selected text");
            }
            if uri.contains("frontmostApp=1") {
                fields.push("frontmost app");
            }
            if uri.contains("menuBar=1") {
                fields.push("menu bar");
            }
            if uri.contains("browserUrl=1") {
                fields.push("browser URL");
            }
            if uri.contains("focusedWindow=1") {
                fields.push("focused window");
            }
            if fields.is_empty() {
                lines.push("Custom context filter".to_string());
            } else {
                lines.push(format!("Captures: {}", fields.join(", ")));
            }
        }
        ContextPreviewProfile::FilePath => {
            // Handled separately in derive_context_preview_info
        }
    }

    if has_diagnostics {
        lines.push("Includes field-level diagnostics (warnings, status per field)".to_string());
    }

    lines.join(". ")
}

fn format_byte_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::message_parts::AiContextPart;

    #[test]
    fn context_preview_ui_derive_minimal_profile() {
        let part = AiContextPart::ResourceUri {
            uri: "kit://context?profile=minimal".to_string(),
            label: "Current Context".to_string(),
        };
        let info = derive_context_preview_info(&part);
        assert_eq!(info.profile, ContextPreviewProfile::Minimal);
        assert!(!info.has_diagnostics);
        assert!(info.description.contains("Excludes: selected text"));
    }

    #[test]
    fn context_preview_ui_derive_full_profile() {
        let part = AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Full Context".to_string(),
        };
        let info = derive_context_preview_info(&part);
        assert_eq!(info.profile, ContextPreviewProfile::Full);
        assert!(info.description.contains("all fields"));
    }

    #[test]
    fn context_preview_ui_derive_diagnostics() {
        let part = AiContextPart::ResourceUri {
            uri: "kit://context?diagnostics=1".to_string(),
            label: "Context Diagnostics".to_string(),
        };
        let info = derive_context_preview_info(&part);
        assert!(info.has_diagnostics);
        assert!(info.description.contains("diagnostics"));
    }

    #[test]
    fn context_preview_ui_full_distinguishes_from_minimal() {
        let minimal = AiContextPart::ResourceUri {
            uri: "kit://context?profile=minimal".to_string(),
            label: "Minimal".to_string(),
        };
        let full = AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Full".to_string(),
        };
        let info_min = derive_context_preview_info(&minimal);
        let info_full = derive_context_preview_info(&full);

        assert_ne!(info_min.profile, info_full.profile);
        assert_ne!(info_min.description, info_full.description);
    }

    #[test]
    fn context_preview_ui_custom_flags_parsed() {
        let part = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Custom".to_string(),
        };
        let info = derive_context_preview_info(&part);
        assert_eq!(info.profile, ContextPreviewProfile::Custom);
        assert!(info.description.contains("selected text"));
        assert!(info.description.contains("browser URL"));
        assert!(!info.description.contains("frontmost app"));
    }

    #[test]
    fn context_preview_ui_file_path_preview() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "fn main() {}").expect("write");

        let part = AiContextPart::FilePath {
            path: file.to_string_lossy().to_string(),
            label: "test.rs".to_string(),
        };
        let info = derive_context_preview_info(&part);
        assert_eq!(info.profile, ContextPreviewProfile::FilePath);
        assert!(info.description.contains("File attachment"));
    }
}

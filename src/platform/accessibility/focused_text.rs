use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use super::app_identity::ActiveAppIdentity;
use super::geometry::FocusedFieldGeometry;
use super::metrics::TextMetrics;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusedTextSessionId(pub String);

impl FocusedTextSessionId {
    pub fn new_for_tests(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for FocusedTextSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CaptureFocusedTextOptions {
    pub allow_secure_fields: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedTextTargetDescriptor {
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub title: Option<String>,
    pub content_kind: FocusedTextContentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedTextContentKind {
    PlainText,
    RichText,
    Secure,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRangeUtf16 {
    pub location: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusedTextCapabilities {
    pub can_replace: bool,
    pub can_append: bool,
    pub can_copy: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FocusedTextSnapshot {
    pub session_id: FocusedTextSessionId,
    pub captured_at_ms: u128,
    pub app: ActiveAppIdentity,
    pub target: FocusedTextTargetDescriptor,
    pub text: String,
    pub selected_range_utf16: Option<TextRangeUtf16>,
    pub caret_range_utf16: Option<TextRangeUtf16>,
    pub metrics: TextMetrics,
    pub geometry: FocusedFieldGeometry,
    pub capabilities: FocusedTextCapabilities,
}

pub const MAX_FOCUSED_TEXT_CAPTURE_CHARS: usize = 50_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FocusedTextError {
    #[error("accessibility permission is required")]
    AccessibilityPermissionRequired,
    #[error("focused field is secure")]
    SecureField,
    #[error("focused field is unsupported")]
    UnsupportedTarget,
    #[error("focused text session is stale")]
    StaleSession,
    #[error("{0}")]
    Platform(String),
}

impl FocusedTextError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::AccessibilityPermissionRequired => "accessibilityPermissionRequired",
            Self::SecureField => "secureField",
            Self::UnsupportedTarget => "unsupportedTarget",
            Self::StaleSession => "staleSession",
            Self::Platform(_) => "platform",
        }
    }

    pub fn user_message(&self) -> &'static str {
        match self {
            Self::AccessibilityPermissionRequired => {
                "Accessibility permission needed. Grant access in System Settings to grab focused text."
            }
            Self::SecureField => "This is a secure field and can't be accessed.",
            Self::UnsupportedTarget => {
                "Unable to grab text from this field. Select text and try again."
            }
            Self::StaleSession => "The focused text session expired. Try again.",
            Self::Platform(_) => "Unable to grab text. Select text and try again.",
        }
    }

    pub fn offers_open_settings(&self) -> bool {
        matches!(self, Self::AccessibilityPermissionRequired)
    }
}

pub fn truncate_focused_text_capture(text: String) -> (String, bool) {
    let char_count = text.chars().count();
    if char_count <= MAX_FOCUSED_TEXT_CAPTURE_CHARS {
        return (text, false);
    }
    (
        text.chars()
            .take(MAX_FOCUSED_TEXT_CAPTURE_CHARS)
            .collect::<String>(),
        true,
    )
}

pub fn focused_text_snapshot_for_capture_failure() -> FocusedTextSnapshot {
    let app = super::app_identity::current_frontmost_app_identity();
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let session_id = FocusedTextSessionId(format!("focused-text-failed-{captured_at_ms}"));

    FocusedTextSnapshot {
        session_id,
        captured_at_ms,
        app,
        target: FocusedTextTargetDescriptor {
            role: None,
            subrole: None,
            title: None,
            content_kind: FocusedTextContentKind::Unsupported,
        },
        metrics: TextMetrics::from_text(""),
        geometry: FocusedFieldGeometry::default(),
        capabilities: FocusedTextCapabilities {
            can_replace: false,
            can_append: false,
            can_copy: true,
        },
        text: String::new(),
        selected_range_utf16: None,
        caret_range_utf16: None,
    }
}

/// Snapshot for text captured via the selection/Cmd+C ladder in AX-opaque
/// apps (Chrome, Electron) where `capture_focused_text_field` fails. There is
/// no registered AX element; Replace applies by pasting over the still-active
/// selection in the source app (see `mutation::register_selection_paste_target`),
/// so Append is unavailable.
pub fn focused_text_snapshot_for_selection_paste(
    text: String,
    source_app: Option<String>,
) -> FocusedTextSnapshot {
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let session_id = FocusedTextSessionId(format!("focused-text-paste-{captured_at_ms}"));
    super::mutation::register_selection_paste_target(&session_id);

    let mut app = super::app_identity::current_frontmost_app_identity();
    if let Some(name) = source_app.filter(|name| !name.trim().is_empty()) {
        app.name = name;
    }

    FocusedTextSnapshot {
        session_id,
        captured_at_ms,
        app,
        target: FocusedTextTargetDescriptor {
            role: None,
            subrole: None,
            title: None,
            content_kind: FocusedTextContentKind::PlainText,
        },
        metrics: TextMetrics::from_text(&text),
        geometry: FocusedFieldGeometry::default(),
        capabilities: FocusedTextCapabilities {
            can_replace: true,
            can_append: false,
            can_copy: true,
        },
        text,
        selected_range_utf16: None,
        caret_range_utf16: None,
    }
}

/// Passive AX-only read of the selected text in `pid`'s focused element
/// (system-wide focused element first, per-app fallback). Secure fields
/// return `None`. Never posts keystrokes and never touches the pasteboard,
/// so it is safe to call speculatively (hint chips, previews).
pub fn selected_text_for_app_ax_only(pid: Option<i32>) -> Result<Option<String>, FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)
        .map_err(|error| FocusedTextError::Platform(error.to_string()))?;
    if !super::permissions::has_accessibility_permission() {
        return Err(FocusedTextError::AccessibilityPermissionRequired);
    }
    selected_text_for_app_ax_only_platform(pid)
}

#[cfg(target_os = "macos")]
fn selected_text_for_app_ax_only_platform(
    pid: Option<i32>,
) -> Result<Option<String>, FocusedTextError> {
    // Per-app-first when the source pid is known: by the time this passive
    // read runs, our own panel may be key, so the system-wide focused element
    // could be Script Kit's own input instead of the app the user came from.
    let element = match pid {
        Some(pid) => super::ax::focused_ui_element_for_pid(pid),
        None => super::ax::focused_ui_element_for_app(None),
    }
    .map_err(|err| FocusedTextError::Platform(err.to_string()))?;
    let role = super::ax::role(element.as_ptr());
    let subrole = super::ax::subrole(element.as_ptr());
    if classify_content_kind(role.as_deref(), subrole.as_deref()) == FocusedTextContentKind::Secure
    {
        return Ok(None);
    }
    Ok(super::ax::selected_text(element.as_ptr()))
}

#[cfg(not(target_os = "macos"))]
fn selected_text_for_app_ax_only_platform(
    _pid: Option<i32>,
) -> Result<Option<String>, FocusedTextError> {
    Err(FocusedTextError::UnsupportedTarget)
}

/// Passive AX-only read of the WHOLE text in `pid`'s focused element,
/// truncated to [`MAX_FOCUSED_TEXT_CAPTURE_CHARS`]. Secure fields return
/// `None`. Never posts keystrokes and never touches the pasteboard.
pub fn focused_text_for_app_ax_only(pid: Option<i32>) -> Result<Option<String>, FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)
        .map_err(|error| FocusedTextError::Platform(error.to_string()))?;
    if !super::permissions::has_accessibility_permission() {
        return Err(FocusedTextError::AccessibilityPermissionRequired);
    }
    focused_text_for_app_ax_only_platform(pid)
}

#[cfg(target_os = "macos")]
fn focused_text_for_app_ax_only_platform(
    pid: Option<i32>,
) -> Result<Option<String>, FocusedTextError> {
    let element = match pid {
        Some(pid) => super::ax::focused_ui_element_for_pid(pid),
        None => super::ax::focused_ui_element_for_app(None),
    }
    .map_err(|err| FocusedTextError::Platform(err.to_string()))?;
    let role = super::ax::role(element.as_ptr());
    let subrole = super::ax::subrole(element.as_ptr());
    if classify_content_kind(role.as_deref(), subrole.as_deref()) == FocusedTextContentKind::Secure
    {
        return Ok(None);
    }
    match super::ax::whole_text(element.as_ptr()) {
        Ok(text) => {
            let (text, _truncated) = truncate_focused_text_capture(text);
            Ok(Some(text).filter(|text| !text.trim().is_empty()))
        }
        Err(err) => Err(FocusedTextError::Platform(err.to_string())),
    }
}

#[cfg(not(target_os = "macos"))]
fn focused_text_for_app_ax_only_platform(
    _pid: Option<i32>,
) -> Result<Option<String>, FocusedTextError> {
    Err(FocusedTextError::UnsupportedTarget)
}

pub fn capture_focused_text_field(
    options: CaptureFocusedTextOptions,
) -> Result<FocusedTextSnapshot, FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)
        .map_err(|error| FocusedTextError::Platform(error.to_string()))?;
    if !super::permissions::has_accessibility_permission() {
        return Err(FocusedTextError::AccessibilityPermissionRequired);
    }

    capture_focused_text_field_platform(options)
}

#[cfg(target_os = "macos")]
fn capture_focused_text_field_platform(
    options: CaptureFocusedTextOptions,
) -> Result<FocusedTextSnapshot, FocusedTextError> {
    let app = super::app_identity::current_frontmost_app_identity();
    let element = super::ax::focused_ui_element_for_app(app.process_id)
        .map_err(|err| FocusedTextError::Platform(err.to_string()))?;
    let role = super::ax::role(element.as_ptr());
    let subrole = super::ax::subrole(element.as_ptr());

    let content_kind = classify_content_kind(role.as_deref(), subrole.as_deref());
    if content_kind == FocusedTextContentKind::Secure && !options.allow_secure_fields {
        return Err(FocusedTextError::SecureField);
    }

    // Read the selection range BEFORE any text fetch: the clipboard fallback
    // below posts ⌘A+⌘C into the app, which replaces the user's real
    // selection with select-all and would make this read lie.
    let selected_range_utf16 = super::ax::selected_text_range(element.as_ptr());
    let mut used_clipboard_fallback = false;
    let text = match super::ax::whole_text(element.as_ptr()) {
        Ok(text) => text,
        Err(err) => {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_ax_whole_text_failed_trying_clipboard_fallback",
                error = %err,
                role = ?role,
                subrole = ?subrole,
            );
            let text = super::clipboard::copy_all_plain_text_preserving_clipboard().map_err(
                |fallback_err| {
                    FocusedTextError::Platform(format!(
                        "{err}; focused-text clipboard fallback failed: {fallback_err}"
                    ))
                },
            )?;
            used_clipboard_fallback = true;
            text
        }
    };
    let caret_range_utf16 = selected_range_utf16.map(|range| TextRangeUtf16 {
        location: range.location + range.length,
        length: 0,
    });
    let geometry = super::ax::focused_geometry(element.as_ptr(), selected_range_utf16);
    let can_edit = super::ax::is_enabled(element.as_ptr()).unwrap_or(true)
        && content_kind != FocusedTextContentKind::Secure
        && (content_kind != FocusedTextContentKind::Unsupported || used_clipboard_fallback);
    let captured_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    let session_id = FocusedTextSessionId(format!("focused-text-{captured_at_ms}"));
    super::ax::register_focused_text_session(
        &session_id,
        element.as_ptr(),
        captured_at_ms,
        text.clone(),
        app.process_id,
    )
    .map_err(|err| FocusedTextError::Platform(err.to_string()))?;

    Ok(FocusedTextSnapshot {
        session_id,
        captured_at_ms,
        app,
        target: FocusedTextTargetDescriptor {
            role,
            subrole,
            title: None,
            content_kind,
        },
        metrics: TextMetrics::from_text(&text),
        geometry,
        capabilities: FocusedTextCapabilities {
            can_replace: can_edit,
            can_append: can_edit,
            can_copy: true,
        },
        text,
        selected_range_utf16,
        caret_range_utf16,
    })
}

#[cfg(not(target_os = "macos"))]
fn capture_focused_text_field_platform(
    _options: CaptureFocusedTextOptions,
) -> Result<FocusedTextSnapshot, FocusedTextError> {
    Err(FocusedTextError::UnsupportedTarget)
}

pub fn focused_text_snapshot_for_tests(text: impl Into<String>) -> FocusedTextSnapshot {
    let text = text.into();
    let session_id = FocusedTextSessionId::new_for_tests("focused-text-test-session");
    // Fixture sessions have no real AX element, so register an in-memory
    // mutable target seeded with the captured text. Replace/Append then act on
    // this buffer and report a truthful `changed_text` receipt, making the full
    // capture → rewrite → paste-back round-trip exercisable without a foreign app.
    super::mutation::register_in_memory_focused_text_target(&session_id, &text);
    FocusedTextSnapshot {
        session_id,
        captured_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        app: ActiveAppIdentity::unknown(),
        target: FocusedTextTargetDescriptor {
            role: Some("AXTextArea".to_string()),
            subrole: None,
            title: None,
            content_kind: FocusedTextContentKind::PlainText,
        },
        metrics: TextMetrics::from_text(&text),
        geometry: FocusedFieldGeometry::default(),
        capabilities: FocusedTextCapabilities {
            can_replace: true,
            can_append: true,
            can_copy: true,
        },
        text,
        selected_range_utf16: None,
        caret_range_utf16: None,
    }
}

pub fn classify_content_kind(role: Option<&str>, subrole: Option<&str>) -> FocusedTextContentKind {
    let role = role.unwrap_or_default();
    let subrole = subrole.unwrap_or_default();
    let combined = format!("{role} {subrole}");

    if combined.contains("Secure") || combined.contains("Password") {
        return FocusedTextContentKind::Secure;
    }
    if combined.contains("TextArea") || combined.contains("TextField") {
        return FocusedTextContentKind::PlainText;
    }
    if combined.contains("Text") {
        return FocusedTextContentKind::RichText;
    }
    FocusedTextContentKind::Unsupported
}

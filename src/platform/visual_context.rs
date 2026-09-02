use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualContextState {
    Idle,
    Capturing,
    ExtractingText,
    Ready,
    Unavailable(String),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct VisualContextCapture {
    pub state: VisualContextState,
    pub excerpt: Option<String>,
    pub source_app: Option<String>,
    pub source_pid: Option<i32>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub char_count: usize,
    pub truncated: bool,
    pub captured_at: Option<Instant>,
    pub generation: u64,
}

impl Default for VisualContextCapture {
    fn default() -> Self {
        Self {
            state: VisualContextState::Idle,
            excerpt: None,
            source_app: None,
            source_pid: None,
            image_width: None,
            image_height: None,
            char_count: 0,
            truncated: false,
            captured_at: None,
            generation: 0,
        }
    }
}

impl VisualContextCapture {
    pub fn is_fresh(&self, ttl_secs: u64) -> bool {
        self.captured_at
            .map(|t| t.elapsed().as_secs() < ttl_secs)
            .unwrap_or(false)
    }

    pub fn chip_label(&self) -> Option<String> {
        match &self.state {
            VisualContextState::Ready => {
                let app = self.source_app.as_deref().unwrap_or("Window");
                Some(format!("Visible Text · {app}"))
            }
            VisualContextState::Capturing => Some("Capturing…".to_string()),
            VisualContextState::ExtractingText => Some("Reading text…".to_string()),
            _ => None,
        }
    }
}

const MAX_EXCERPT_CHARS: usize = 4096;
const MAX_RAW_OCR_CHARS: usize = 16384;

pub fn sanitize_ocr_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() <= MAX_RAW_OCR_CHARS {
        trimmed.to_string()
    } else {
        trimmed[..MAX_RAW_OCR_CHARS].to_string()
    }
}

pub fn build_excerpt(sanitized: &str) -> (String, bool) {
    if sanitized.len() <= MAX_EXCERPT_CHARS {
        (sanitized.to_string(), false)
    } else {
        (format!("{}…", &sanitized[..MAX_EXCERPT_CHARS]), true)
    }
}

#[cfg(target_os = "macos")]
pub fn check_screen_recording_permission() -> bool {
    crate::platform::permiso_detect::screen_capture_authorized()
        == crate::platform::permiso_detect::PermissionStatus::Authorized
}

#[cfg(not(target_os = "macos"))]
pub fn check_screen_recording_permission() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn capture_frontmost_window_screenshot() -> anyhow::Result<(Vec<u8>, u32, u32, String, i32)> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)?;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};

    if !check_screen_recording_permission() {
        anyhow::bail!("Screen Recording permission not granted");
    }

    let (app_name, pid) = unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let frontmost: id = msg_send![workspace, frontmostApplication];
        if frontmost == nil {
            anyhow::bail!("No frontmost application");
        }
        let name: id = msg_send![frontmost, localizedName];
        let pid: i32 = msg_send![frontmost, processIdentifier];
        let name_str = if name != nil {
            let bytes = name.UTF8String() as *const u8;
            if bytes.is_null() {
                "Unknown".to_string()
            } else {
                let len = name.len();
                String::from_utf8_lossy(std::slice::from_raw_parts(bytes, len)).to_string()
            }
        } else {
            "Unknown".to_string()
        };
        (name_str, pid)
    };

    // For now, return a placeholder — full ScreenCaptureKit integration
    // requires async completion handlers that need a Swift bridge
    anyhow::bail!(
        "ScreenCaptureKit capture not yet implemented for {}(pid {}). \
         Use the existing screenshot infrastructure in platform/screenshots_window_open.rs \
         as the capture backend.",
        app_name,
        pid
    )
}

#[cfg(not(target_os = "macos"))]
pub fn capture_frontmost_window_screenshot() -> anyhow::Result<(Vec<u8>, u32, u32, String, i32)> {
    anyhow::bail!("Visual context capture is only supported on macOS")
}

#[cfg(target_os = "macos")]
pub fn run_vision_ocr(_png_data: &[u8]) -> anyhow::Result<String> {
    // Vision OCR requires a Swift helper using VNRecognizeTextRequest.
    // This stub returns an error until the Swift bridge is implemented.
    anyhow::bail!(
        "Vision OCR Swift helper not yet implemented. \
         Create src/platform/macos_vision_ocr.swift with VNRecognizeTextRequest \
         and VNImageRequestHandler, then bridge via a C-callable function."
    )
}

#[cfg(not(target_os = "macos"))]
pub fn run_vision_ocr(_png_data: &[u8]) -> anyhow::Result<String> {
    anyhow::bail!("Vision OCR is only supported on macOS")
}

pub fn capture_visual_context(generation: u64) -> VisualContextCapture {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ScreenCapture)
    {
        return VisualContextCapture {
            state: VisualContextState::Unavailable(error.to_string()),
            generation,
            ..Default::default()
        };
    }
    if !check_screen_recording_permission() {
        return VisualContextCapture {
            state: VisualContextState::Unavailable(
                "Screen Recording permission required".to_string(),
            ),
            generation,
            ..Default::default()
        };
    }

    let mut capture = VisualContextCapture {
        state: VisualContextState::Capturing,
        generation,
        ..Default::default()
    };

    match capture_frontmost_window_screenshot() {
        Ok((png_data, width, height, app_name, pid)) => {
            capture.source_app = Some(app_name);
            capture.source_pid = Some(pid);
            capture.image_width = Some(width);
            capture.image_height = Some(height);
            capture.state = VisualContextState::ExtractingText;

            match run_vision_ocr(&png_data) {
                Ok(raw_text) => {
                    let sanitized = sanitize_ocr_text(&raw_text);
                    let (excerpt, truncated) = build_excerpt(&sanitized);
                    capture.char_count = sanitized.len();
                    capture.truncated = truncated;
                    capture.excerpt = Some(excerpt);
                    capture.captured_at = Some(Instant::now());
                    capture.state = VisualContextState::Ready;
                }
                Err(e) => {
                    capture.state = VisualContextState::Failed(format!("OCR failed: {e}"));
                }
            }
        }
        Err(e) => {
            capture.state = VisualContextState::Failed(format!("Capture failed: {e}"));
        }
    }

    capture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_idle() {
        let cap = VisualContextCapture::default();
        assert_eq!(cap.state, VisualContextState::Idle);
        assert!(cap.excerpt.is_none());
        assert!(!cap.is_fresh(300));
    }

    #[test]
    fn chip_label_varies_by_state() {
        let mut cap = VisualContextCapture::default();

        cap.state = VisualContextState::Capturing;
        assert_eq!(cap.chip_label(), Some("Capturing…".to_string()));

        cap.state = VisualContextState::ExtractingText;
        assert_eq!(cap.chip_label(), Some("Reading text…".to_string()));

        cap.state = VisualContextState::Ready;
        cap.source_app = Some("Safari".to_string());
        assert_eq!(cap.chip_label(), Some("Visible Text · Safari".to_string()));

        cap.state = VisualContextState::Idle;
        assert!(cap.chip_label().is_none());
    }

    #[test]
    fn sanitize_ocr_caps_length() {
        let short = "Hello world";
        assert_eq!(sanitize_ocr_text(short), "Hello world");

        let long = "x".repeat(20000);
        let sanitized = sanitize_ocr_text(&long);
        assert_eq!(sanitized.len(), MAX_RAW_OCR_CHARS);
    }

    #[test]
    fn build_excerpt_truncation() {
        let short = "Hello world";
        let (excerpt, truncated) = build_excerpt(short);
        assert_eq!(excerpt, "Hello world");
        assert!(!truncated);

        let long = "x".repeat(5000);
        let (excerpt, truncated) = build_excerpt(&long);
        assert!(truncated);
        assert!(excerpt.len() < 5000);
        assert!(excerpt.ends_with('…'));
    }

    #[test]
    fn freshness_check() {
        let mut cap = VisualContextCapture::default();
        assert!(!cap.is_fresh(300));

        cap.captured_at = Some(Instant::now());
        assert!(cap.is_fresh(300));
    }

    #[test]
    fn capture_without_permission_returns_unavailable() {
        // This test verifies the state machine flow, not actual capture
        let cap = VisualContextCapture {
            state: VisualContextState::Unavailable(
                "Screen Recording permission required".to_string(),
            ),
            generation: 1,
            ..Default::default()
        };
        assert!(matches!(cap.state, VisualContextState::Unavailable(_)));
        assert!(cap.chip_label().is_none());
    }

    #[test]
    fn failed_capture_state() {
        let cap = VisualContextCapture {
            state: VisualContextState::Failed("Capture failed: timeout".to_string()),
            generation: 1,
            ..Default::default()
        };
        assert!(matches!(cap.state, VisualContextState::Failed(_)));
    }

    #[test]
    fn ready_state_with_excerpt() {
        let cap = VisualContextCapture {
            state: VisualContextState::Ready,
            excerpt: Some("Error: connection refused on port 8080".to_string()),
            source_app: Some("Terminal".to_string()),
            source_pid: Some(1234),
            image_width: Some(1200),
            image_height: Some(800),
            char_count: 38,
            truncated: false,
            captured_at: Some(Instant::now()),
            generation: 1,
        };
        assert_eq!(
            cap.chip_label(),
            Some("Visible Text · Terminal".to_string())
        );
        assert!(cap.is_fresh(300));
        assert_eq!(cap.char_count, 38);
    }
}

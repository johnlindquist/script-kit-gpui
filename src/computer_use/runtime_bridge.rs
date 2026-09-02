use crate::computer_use::window_observation::ComputerUseWindowObservationV1;
use crate::protocol::{
    AutomationInspectSnapshot, AutomationTargetIdentitySnapshot, AutomationWindowTarget,
    CompletedFrameIdentity, PixelProbe, TargetWindowBounds,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerUseInspectRequest {
    pub target: Option<AutomationWindowTarget>,
    pub hi_dpi: Option<bool>,
    pub probes: Vec<PixelProbe>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerUseListAppsRequest {
    pub include_hidden: bool,
    pub include_background: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerUseListAppWindowsRequest {
    pub pid: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerUseCaptureNativeWindowRequest {
    pub pid: i32,
    pub native_window_id: u32,
    pub hi_dpi: bool,
    pub include_image: bool,
    pub expected_bundle_id: Option<String>,
    pub correlation_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputerUseCaptureRenderWindowRequest {
    pub target: AutomationWindowTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<AutomationTargetIdentitySnapshot>,
    #[serde(rename = "hiDpi", default)]
    pub hi_dpi: bool,
    #[serde(default)]
    pub include_image: bool,
    /// Native-resolution probes of the exact retained frame, never a new draw.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<PixelProbe>,
    #[serde(default)]
    pub correlation_id: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseRunningAppInfo {
    pub pid: i32,
    pub bundle_id: Option<String>,
    pub name: String,
    pub is_active: bool,
    pub is_hidden: bool,
    pub activation_policy: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseAppWindowInfo {
    pub native_window_id: u32,
    pub title: Option<String>,
    pub bounds: TargetWindowBounds,
    pub is_on_screen: bool,
    pub layer: i64,
    pub z_order: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<ComputerUseWindowObservationV1>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerUseListAppsSnapshot {
    pub apps: Vec<ComputerUseRunningAppInfo>,
    pub frontmost_pid: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputerUseListAppWindowsSnapshot {
    pub app: Option<ComputerUseRunningAppInfo>,
    pub windows: Vec<ComputerUseAppWindowInfo>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseCaptureNativeWindowSnapshot {
    pub schema_version: u32,
    pub source: &'static str,
    pub scope: &'static str,
    pub status: ComputerUseCaptureNativeWindowStatus,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<ComputerUseRunningAppInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<ComputerUseAppWindowInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<ComputerUseNativeWindowCaptureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ComputerUseCaptureNativeWindowError>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseCaptureRenderWindowSnapshot {
    pub schema_version: u32,
    pub source: &'static str,
    pub scope: &'static str,
    pub status: ComputerUseCaptureRenderWindowStatus,
    pub correlation_id: String,
    pub target: AutomationWindowTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_identity: Option<CompletedFrameIdentity>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub phase_durations_ms: std::collections::BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<ComputerUseNativeWindowCaptureInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pixel_probes: Vec<crate::protocol::PixelProbeResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ComputerUseCaptureNativeWindowError>,
    pub warnings: Vec<String>,
    pub limitation: &'static str,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ComputerUseCaptureRenderWindowStatus {
    Captured,
    Unsupported,
    TargetNotFound,
    BlankImageRejected,
    CaptureFailed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ComputerUseCaptureNativeWindowStatus {
    Captured,
    AppNotFound,
    WindowNotFound,
    OwnershipMismatch,
    NotCaptureCandidate,
    AmbiguousNativeWindowRows,
    AmbiguousNativeWindowId,
    PermissionDenied,
    BlankImageRejected,
    CaptureFailed,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseNativeWindowCaptureInfo {
    pub mime_type: &'static str,
    pub width: u32,
    pub height: u32,
    pub byte_length: usize,
    pub sha256: String,
    #[serde(rename = "hiDpi")]
    pub hi_dpi: bool,
    pub pixel_audit: ComputerUseCapturePixelAudit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub png_base64: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseCapturePixelAudit {
    pub sampled: u64,
    pub non_black: u64,
    pub non_transparent: u64,
    pub unique_bucket_count: usize,
    pub mean_luma: f64,
    pub blank_like: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseCaptureNativeWindowError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_audit: Option<ComputerUseCapturePixelAudit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputerUseRuntimeError {
    Unavailable,
    Disconnected,
    Timeout,
    Failed(String),
}

impl ComputerUseRuntimeError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Unavailable => "runtime_unavailable",
            Self::Disconnected => "runtime_disconnected",
            Self::Timeout => "runtime_timeout",
            Self::Failed(_) => "inspection_failed",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Unavailable => {
                "computer-use requires a live automation runtime bridge".to_string()
            }
            Self::Disconnected => "computer-use runtime bridge disconnected".to_string(),
            Self::Timeout => "computer-use runtime bridge timed out".to_string(),
            Self::Failed(message) => message.clone(),
        }
    }
}

pub trait ComputerUseRuntimeBridge: Send + Sync {
    fn inspect_automation_window(
        &self,
        request: ComputerUseInspectRequest,
    ) -> Result<AutomationInspectSnapshot, ComputerUseRuntimeError>;

    fn list_running_apps(
        &self,
        request: ComputerUseListAppsRequest,
    ) -> Result<ComputerUseListAppsSnapshot, ComputerUseRuntimeError>;

    fn list_app_windows(
        &self,
        request: ComputerUseListAppWindowsRequest,
    ) -> Result<ComputerUseListAppWindowsSnapshot, ComputerUseRuntimeError>;

    fn capture_native_window(
        &self,
        _request: ComputerUseCaptureNativeWindowRequest,
    ) -> Result<ComputerUseCaptureNativeWindowSnapshot, ComputerUseRuntimeError> {
        Err(ComputerUseRuntimeError::Unavailable)
    }

    fn capture_render_window(
        &self,
        _request: ComputerUseCaptureRenderWindowRequest,
    ) -> Result<ComputerUseCaptureRenderWindowSnapshot, ComputerUseRuntimeError> {
        Err(ComputerUseRuntimeError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_native_window_snapshot_serializes_camel_case_contract() {
        let snapshot = ComputerUseCaptureNativeWindowSnapshot {
            schema_version: 1,
            source: "coreGraphicsWindowList+xcap",
            scope: "runningAppPidNativeWindowIdCapture",
            status: ComputerUseCaptureNativeWindowStatus::Captured,
            correlation_id: "capture-1".to_string(),
            app: None,
            window: None,
            capture: Some(ComputerUseNativeWindowCaptureInfo {
                mime_type: "image/png",
                width: 10,
                height: 20,
                byte_length: 30,
                sha256: "a".repeat(64),
                hi_dpi: false,
                pixel_audit: ComputerUseCapturePixelAudit {
                    sampled: 200,
                    non_black: 100,
                    non_transparent: 200,
                    unique_bucket_count: 8,
                    mean_luma: 42.0,
                    blank_like: false,
                },
                png_base64: Some("ZmFrZQ==".to_string()),
            }),
            error: None,
            warnings: Vec::new(),
        };

        let json = serde_json::to_value(&snapshot).expect("serialize capture snapshot");
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["status"], "captured");
        assert_eq!(json["correlationId"], "capture-1");
        assert_eq!(json["capture"]["mimeType"], "image/png");
        assert_eq!(json["capture"]["hiDpi"], false);
        assert_eq!(json["capture"]["pixelAudit"]["blankLike"], false);
        assert_eq!(json["capture"]["pngBase64"], "ZmFrZQ==");
    }

    #[test]
    fn capture_native_window_snapshot_omits_png_base64_when_not_returned() {
        let info = ComputerUseNativeWindowCaptureInfo {
            mime_type: "image/png",
            width: 10,
            height: 20,
            byte_length: 30,
            sha256: "a".repeat(64),
            hi_dpi: false,
            pixel_audit: ComputerUseCapturePixelAudit {
                sampled: 200,
                non_black: 100,
                non_transparent: 200,
                unique_bucket_count: 8,
                mean_luma: 42.0,
                blank_like: false,
            },
            png_base64: None,
        };

        let json = serde_json::to_value(&info).expect("serialize capture info");
        assert!(json.get("pngBase64").is_none());
    }

    #[test]
    fn capture_render_window_snapshot_serializes_limitation_contract() {
        let snapshot = ComputerUseCaptureRenderWindowSnapshot {
            schema_version: 1,
            source: "gpuiRenderReadback",
            scope: "liveAutomationWindowRenderReadback",
            status: ComputerUseCaptureRenderWindowStatus::Unsupported,
            correlation_id: "render-1".to_string(),
            target: AutomationWindowTarget::Focused,
            frame_identity: None,
            phase_durations_ms: Default::default(),
            capture: None,
            pixel_probes: Vec::new(),
            error: Some(ComputerUseCaptureNativeWindowError {
                code: "gpui_readback_unavailable",
                message: "GPUI render readback is not implemented yet".to_string(),
                reason: Some("unsupported".to_string()),
                pixel_audit: None,
            }),
            warnings: vec!["app-render proof is not OS compositor proof".to_string()],
            limitation: "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor/native blur output.",
        };

        let json = serde_json::to_value(&snapshot).expect("serialize render snapshot");
        assert_eq!(json["source"], "gpuiRenderReadback");
        assert_eq!(json["status"], "unsupported");
        assert_eq!(json["target"]["type"], "focused");
        assert_eq!(json["error"]["code"], "gpui_readback_unavailable");
        assert_eq!(
            json["limitation"],
            "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor/native blur output."
        );
    }
}

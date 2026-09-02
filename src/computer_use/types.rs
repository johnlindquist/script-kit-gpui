use crate::protocol::{AutomationTargetIdentitySnapshot, AutomationWindowTarget, PixelProbe};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputerUseSeeArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<AutomationWindowTarget>,
    #[serde(rename = "hiDpi", default, skip_serializing_if = "Option::is_none")]
    pub hi_dpi: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<PixelProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputerUseCaptureNativeWindowArgs {
    pub pid: i32,
    pub native_window_id: u32,
    #[serde(rename = "hiDpi", default)]
    pub hi_dpi: bool,
    #[serde(default)]
    pub include_image: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_bundle_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComputerUseCaptureRenderWindowArgs {
    pub target: AutomationWindowTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<AutomationTargetIdentitySnapshot>,
    #[serde(rename = "hiDpi", default)]
    pub hi_dpi: bool,
    #[serde(default)]
    pub include_image: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<crate::protocol::PixelProbe>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AutomationWindowTarget, PixelProbe};

    #[test]
    fn see_args_serde_roundtrip_preserves_target_and_probes() {
        let args = ComputerUseSeeArgs {
            target: Some(AutomationWindowTarget::Focused),
            hi_dpi: Some(false),
            probes: vec![PixelProbe { x: 10, y: 20 }],
        };

        let json = serde_json::to_string(&args).expect("serialize");
        assert!(json.contains("hiDpi"));

        let parsed: ComputerUseSeeArgs = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, args);
    }

    #[test]
    fn see_args_reject_unknown_fields() {
        let error = serde_json::from_value::<ComputerUseSeeArgs>(serde_json::json!({
            "unexpected": true
        }))
        .expect_err("unknown fields should be rejected");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn capture_native_window_args_defaults_optional_flags() {
        let parsed: ComputerUseCaptureNativeWindowArgs =
            serde_json::from_value(serde_json::json!({
                "pid": 123,
                "nativeWindowId": 456
            }))
            .expect("capture native window args");

        assert_eq!(parsed.pid, 123);
        assert_eq!(parsed.native_window_id, 456);
        assert!(!parsed.hi_dpi);
        assert!(!parsed.include_image);
        assert_eq!(parsed.expected_bundle_id, None);
    }

    #[test]
    fn capture_native_window_args_use_camel_case_wire_names() {
        let args = ComputerUseCaptureNativeWindowArgs {
            pid: 123,
            native_window_id: 456,
            hi_dpi: true,
            include_image: true,
            expected_bundle_id: Some("com.example.App".to_string()),
        };

        let json = serde_json::to_value(&args).expect("serialize capture args");
        assert_eq!(json["nativeWindowId"], 456);
        assert_eq!(json["hiDpi"], true);
        assert_eq!(json["includeImage"], true);
        assert_eq!(json["expectedBundleId"], "com.example.App");
    }

    #[test]
    fn capture_native_window_args_reject_unknown_fields() {
        let error =
            serde_json::from_value::<ComputerUseCaptureNativeWindowArgs>(serde_json::json!({
                "pid": 123,
                "nativeWindowId": 456,
                "unexpected": true
            }))
            .expect_err("unknown fields should be rejected");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn capture_render_window_args_defaults_optional_flags() {
        let parsed: ComputerUseCaptureRenderWindowArgs =
            serde_json::from_value(serde_json::json!({
                "target": { "type": "focused" }
            }))
            .expect("capture render window args");

        assert_eq!(parsed.target, AutomationWindowTarget::Focused);
        assert_eq!(parsed.expected, None);
        assert!(!parsed.hi_dpi);
        assert!(!parsed.include_image);
    }

    #[test]
    fn capture_render_window_args_preserve_exact_expected_identity() {
        let wire = serde_json::json!({
            "target": { "type": "instance", "id": "main", "generation": 7 },
            "expected": {
                "windowId": "main", "windowGeneration": 7,
                "appViewVariant": "ScriptList", "targetGeneration": 1,
                "surfaceGeneration": 2, "dataGeneration": 3,
                "presentationRevision": 4, "themeRevision": 5, "frameGeneration": 6
            },
            "hiDpi": true, "includeImage": true
        });
        let parsed: ComputerUseCaptureRenderWindowArgs =
            serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(parsed.expected.as_ref().unwrap().frame_generation, Some(6));
        assert_eq!(serde_json::to_value(parsed).unwrap(), wire);
        let request: crate::computer_use::runtime_bridge::ComputerUseCaptureRenderWindowRequest =
            serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(
            request.expected.as_ref().unwrap().window_generation,
            Some(7)
        );
        assert!(request.hi_dpi && request.include_image);
        assert!(request.correlation_id.is_empty());
    }

    #[test]
    fn capture_render_window_args_reject_unknown_fields() {
        let error =
            serde_json::from_value::<ComputerUseCaptureRenderWindowArgs>(serde_json::json!({
                "target": { "type": "focused" },
                "focus": true
            }))
            .expect_err("unknown mutating fields should be rejected");

        assert!(error.to_string().contains("unknown field"));
    }
}

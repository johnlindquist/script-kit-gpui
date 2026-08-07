//! Types for the `inspectAutomationWindow` protocol command.
//!
//! Returns a compact, machine-readable snapshot of one exact automation
//! window: resolved target identity, screenshot dimensions, optional
//! pixel probe results, and semantic elements when available.

use serde::{Deserialize, Serialize};

/// Current schema version for automation inspect snapshots.
///
/// v2: added `resolved_bounds`, `target_bounds_in_screenshot`,
///     `surface_hit_point`, and `suggested_hit_points`.
/// v3: added `semantic_quality` — machine-readable semantic proof level.
/// v4: added target/surface/data generation counters for stale-view proofs.
pub const AUTOMATION_INSPECT_SCHEMA_VERSION: u32 = 4;

/// Machine-readable indicator of the semantic element quality in an inspect receipt.
///
/// Agents use this to decide whether the receipt carries sufficient proof
/// without parsing warning strings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticQuality {
    /// Full semantic elements collected (input, list, choices, buttons, etc.).
    Full,
    /// Only a panel-level element was collected (entity unavailable at inspect time).
    PanelOnly,
    /// No collector exists for this window kind.
    Unavailable,
}

/// A point in screenshot-relative coordinates.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InspectPoint {
    pub x: f64,
    pub y: f64,
}

/// Bounding rectangle of the target surface inside the captured screenshot.
///
/// For attached surfaces (ActionsDialog, PromptPopup), this is offset from
/// the parent window's origin. For detached windows, `(x, y)` is `(0, 0)`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InspectBoundsInScreenshot {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A suggested click target inside the screenshot coordinate space.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedHitPoint {
    pub semantic_id: String,
    pub x: f64,
    pub y: f64,
    pub reason: String,
}

/// A pixel coordinate to sample from the captured screenshot.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PixelProbe {
    pub x: u32,
    pub y: u32,
}

/// RGBA color at a sampled pixel coordinate.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PixelProbeResult {
    pub x: u32,
    pub y: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Compact proof-oriented snapshot of one exact automation window.
///
/// Bundles resolved target identity, screenshot dimensions, optional
/// pixel probes, and semantic elements into a single response so agents
/// do not need separate `getElements` + `captureScreenshot` round-trips.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationInspectSnapshot {
    /// Schema version for forward compatibility.
    pub schema_version: u32,

    /// Resolved automation window ID (e.g. `"main:0"`, `"agentChatDetached:thread-2"`).
    pub window_id: String,

    /// Automation window kind as a string (e.g. `"Main"`, `"Notes"`, `"AgentChatDetached"`).
    pub window_kind: String,

    /// Stable launcher surface kind when the target is backed by an AppView.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_kind: Option<String>,

    /// Active AppView variant when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_view_variant: Option<String>,

    /// Native footer surface id for the active AppView, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_footer_surface: Option<String>,

    /// Monotonic target lifecycle generation for stale-target proofs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_generation: Option<u64>,

    /// Monotonic surface identity generation for stale-surface proofs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_generation: Option<u64>,

    /// Monotonic redacted data generation for stale-data proofs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_generation: Option<u64>,

    /// Window title if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Window bounds in screen coordinates (from automation registry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_bounds: Option<super::AutomationWindowBounds>,

    /// Bounding rectangle of the target surface within the captured screenshot.
    /// For attached surfaces this is offset from the parent; for detached windows
    /// `(x, y)` is `(0, 0)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_bounds_in_screenshot: Option<InspectBoundsInScreenshot>,

    /// Default click point for the surface (center of `target_bounds_in_screenshot`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_hit_point: Option<InspectPoint>,

    /// Suggested named click targets inside the screenshot coordinate space.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_hit_points: Vec<SuggestedHitPoint>,

    /// Semantic UI elements (empty when collection is unavailable for this window kind).
    #[serde(default)]
    pub elements: Vec<super::ElementInfo>,

    /// Total element count before any limit was applied.
    #[serde(default)]
    pub total_count: usize,

    /// Semantic ID of the currently focused element, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_semantic_id: Option<String>,

    /// Semantic ID of the currently selected element, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_semantic_id: Option<String>,

    /// Width of the captured screenshot in pixels (None if capture failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_width: Option<u32>,

    /// Height of the captured screenshot in pixels (None if capture failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot_height: Option<u32>,

    /// RGBA values at requested pixel coordinates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pixel_probes: Vec<PixelProbeResult>,

    /// Native OS window ID (CGWindowID on macOS) for strict screenshot capture.
    ///
    /// Present when the inspect handler successfully matched an OS window.
    /// Agents use this for `--capture-window-id` in verify-shot flows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_window_id: Option<u32>,

    /// Machine-readable semantic proof level for this receipt.
    ///
    /// `full` — semantic elements are rich (input, list, choices, buttons).
    /// `panel_only` — only a panel-level element collected (entity unavailable).
    /// `unavailable` — no collector for this window kind.
    ///
    /// Added in schema v3. Absent in older receipts; callers should treat
    /// missing as `unavailable` for backward compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_quality: Option<SemanticQuality>,

    /// Machine-readable warnings (e.g. `"panel_only_agent_chat_detached"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Process ID of the window owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::types::elements_actions_scriptlets::{ElementInfo, ElementType};

    fn make_minimal_snapshot() -> AutomationInspectSnapshot {
        AutomationInspectSnapshot {
            schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
            window_id: "main:0".to_string(),
            window_kind: "Main".to_string(),
            surface_kind: None,
            app_view_variant: None,
            native_footer_surface: None,
            target_generation: None,
            surface_generation: None,
            data_generation: None,
            title: None,
            resolved_bounds: None,
            target_bounds_in_screenshot: None,
            surface_hit_point: None,
            suggested_hit_points: Vec::new(),
            elements: Vec::new(),
            total_count: 0,
            focused_semantic_id: None,
            selected_semantic_id: None,
            screenshot_width: None,
            screenshot_height: None,
            pixel_probes: Vec::new(),
            os_window_id: None,
            semantic_quality: None,
            warnings: Vec::new(),
            pid: None,
        }
    }

    #[test]
    fn snapshot_serde_roundtrip_minimal() {
        let snapshot = make_minimal_snapshot();

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let parsed: AutomationInspectSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, snapshot);
        // Verify camelCase
        assert!(json.contains("schemaVersion"));
        assert!(json.contains("windowId"));
        assert!(json.contains("windowKind"));
    }

    #[test]
    fn snapshot_serde_roundtrip_full() {
        let snapshot = AutomationInspectSnapshot {
            schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
            window_id: "notes:0".to_string(),
            window_kind: "Notes".to_string(),
            surface_kind: Some("NotesWindow".to_string()),
            app_view_variant: None,
            native_footer_surface: None,
            target_generation: Some(2),
            surface_generation: Some(3),
            data_generation: Some(4),
            title: Some("Script Kit Notes".to_string()),
            resolved_bounds: Some(crate::protocol::AutomationWindowBounds {
                x: 100.0,
                y: 200.0,
                width: 800.0,
                height: 600.0,
            }),
            target_bounds_in_screenshot: Some(InspectBoundsInScreenshot {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }),
            surface_hit_point: Some(InspectPoint { x: 400.0, y: 300.0 }),
            suggested_hit_points: vec![SuggestedHitPoint {
                semantic_id: "input:notes-editor".to_string(),
                x: 400.0,
                y: 300.0,
                reason: "surface_center".to_string(),
            }],
            elements: vec![ElementInfo {
                semantic_id: "panel:notes".to_string(),
                element_type: ElementType::Panel,
                text: None,
                value: None,
                content: None,
                selected: None,
                focused: None,
                index: None,
                role: None,
                kind: None,
                source: None,
                source_name: None,
                selectable: None,
                status_kind: None,
                action_disabled: None,
                style: None,
            }],
            total_count: 1,
            focused_semantic_id: Some("panel:notes".to_string()),
            selected_semantic_id: None,
            screenshot_width: Some(1440),
            screenshot_height: Some(900),
            pixel_probes: vec![PixelProbeResult {
                x: 24,
                y: 24,
                r: 28,
                g: 28,
                b: 30,
                a: 255,
            }],
            os_window_id: Some(12345),
            semantic_quality: Some(SemanticQuality::Full),
            warnings: vec!["panel_only_notes".to_string()],
            pid: Some(1234),
        };

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let parsed: AutomationInspectSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, snapshot);
        assert!(json.contains("\"osWindowId\":12345"));
        assert!(json.contains("\"targetBoundsInScreenshot\""));
        assert!(json.contains("\"surfaceHitPoint\""));
        assert!(json.contains("\"suggestedHitPoints\""));
        assert!(json.contains("\"semanticQuality\":\"full\""));
    }

    #[test]
    fn pixel_probe_serde() {
        let probe = PixelProbe { x: 100, y: 200 };
        let json = serde_json::to_string(&probe).expect("serialize");
        assert!(json.contains("\"x\":100"));
        let parsed: PixelProbe = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, probe);
    }

    #[test]
    fn pixel_probe_result_serde() {
        let result = PixelProbeResult {
            x: 10,
            y: 20,
            r: 255,
            g: 128,
            b: 0,
            a: 255,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: PixelProbeResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, result);
    }

    #[test]
    fn empty_collections_skipped_in_json() {
        let snapshot = make_minimal_snapshot();
        let json = serde_json::to_string(&snapshot).expect("serialize");
        // Empty vecs with skip_serializing_if should not appear
        assert!(!json.contains("pixelProbes"));
        assert!(!json.contains("warnings"));
        // None options should not appear
        assert!(!json.contains("title"));
        assert!(!json.contains("surfaceKind"));
        assert!(!json.contains("appViewVariant"));
        assert!(!json.contains("nativeFooterSurface"));
        assert!(!json.contains("screenshotWidth"));
        assert!(!json.contains("osWindowId"));
        // New v2 fields should also be absent when None/empty
        assert!(!json.contains("resolvedBounds"));
        assert!(!json.contains("targetBoundsInScreenshot"));
        assert!(!json.contains("surfaceHitPoint"));
        assert!(!json.contains("suggestedHitPoints"));
        // v3 field absent when None
        assert!(!json.contains("semanticQuality"));
    }

    #[test]
    fn backward_compat_without_os_window_id() {
        // Verify that JSON from older schema (without osWindowId) still parses.
        let json = r#"{"schemaVersion":1,"windowId":"main:0","windowKind":"Main","totalCount":0}"#;
        let parsed: AutomationInspectSnapshot =
            serde_json::from_str(json).expect("should parse without osWindowId");
        assert_eq!(parsed.os_window_id, None);
        assert_eq!(parsed.window_id, "main:0");
    }

    #[test]
    fn backward_compat_without_semantic_quality() {
        // v2 JSON (no semanticQuality) must still parse.
        let json = r#"{"schemaVersion":2,"windowId":"main:0","windowKind":"Main","totalCount":0}"#;
        let parsed: AutomationInspectSnapshot =
            serde_json::from_str(json).expect("should parse without semanticQuality");
        assert_eq!(parsed.semantic_quality, None);
    }

    #[test]
    fn semantic_quality_serde_roundtrip() {
        for (quality, expected_str) in [
            (SemanticQuality::Full, "\"full\""),
            (SemanticQuality::PanelOnly, "\"panel_only\""),
            (SemanticQuality::Unavailable, "\"unavailable\""),
        ] {
            let json = serde_json::to_string(&quality).expect("serialize");
            assert_eq!(json, expected_str, "quality {:?}", quality);
            let back: SemanticQuality = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, quality);
        }
    }

    #[test]
    fn semantic_quality_present_in_snapshot_json() {
        let mut snapshot = make_minimal_snapshot();
        snapshot.semantic_quality = Some(SemanticQuality::PanelOnly);
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"semanticQuality\":\"panel_only\""));
        let parsed: AutomationInspectSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.semantic_quality, Some(SemanticQuality::PanelOnly));
    }

    #[test]
    fn os_window_id_present_in_json() {
        let mut snapshot = make_minimal_snapshot();
        snapshot.window_id = "agentChatDetached:thread-1".to_string();
        snapshot.window_kind = "AgentChatDetached".to_string();
        snapshot.screenshot_width = Some(800);
        snapshot.screenshot_height = Some(600);
        snapshot.os_window_id = Some(42);
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"osWindowId\":42"));
        let parsed: AutomationInspectSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.os_window_id, Some(42));
    }
}

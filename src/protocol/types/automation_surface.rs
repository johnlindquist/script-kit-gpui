use serde::{Deserialize, Serialize};

/// Schema version for the automation surface handshake.
pub const AUTOMATION_SURFACE_SCHEMA_VERSION: u32 = 1;

/// Machine-readable snapshot of a named automation surface.
///
/// Returned by `getAutomationSurface` so that agentic helpers can
/// resolve focus targets, capture titles, and minimum window sizes
/// from the app itself instead of hardcoding heuristics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSurfaceSnapshot {
    /// Schema version (currently 1).
    pub schema_version: u32,
    /// Canonical surface name (e.g. `"agent_chat"`, `"main"`).
    pub surface: String,
    /// The `AppView` variant currently active (e.g. `"AgentChatView"`).
    pub view: String,
    /// Whether the main window is visible.
    pub window_visible: bool,
    /// Whether the main window has focus.
    pub window_focused: bool,
    /// Window title substring for `screencapture` targeting.
    pub capture_title: String,
    /// Process owner name substring for Quartz enumeration.
    pub owner_substring: String,
    /// Minimum width (px) to consider a window valid for capture.
    pub min_width: u32,
    /// Minimum height (px) to consider a window valid for capture.
    pub min_height: u32,
}

/// Schema version for the launcher surface contract snapshot in `getState`.
pub const LAUNCHER_SURFACE_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// Screenshot-free identity observed from the same monotonic owner as
/// `inspectAutomationWindow`. A missing value means the target could not be
/// resolved; consumers must never substitute fabricated generation counters.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationTargetIdentitySnapshot {
    pub window_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_generation: Option<u64>,
    pub app_view_variant: String,
    pub target_generation: u64,
    pub surface_generation: u64,
    pub data_generation: u64,
}

/// The only permitted general-purpose row systems, plus explicitly named
/// specialized conversation/content surfaces. No third generic row owner can
/// be silently introduced into the host presentation contract.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceRowPrimitive {
    LegacyListItem,
    UnifiedListItem,
    ConversationTurn,
    SpecializedContent,
    None,
}

/// Shared anatomy projected from the existing exhaustive AppView contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SurfacePresentationSnapshot {
    pub shell_owner: String,
    pub input_owner: String,
    pub row_primitive: SurfaceRowPrimitive,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions_owner: Option<String>,
    pub theme_owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intentional_divergence: Option<String>,
}

/// Machine-readable projection of the active launcher surface contract.
///
/// Included in main-window `stateResult` receipts so agents can verify the
/// runtime surface against the generated contract matrix without reverse-
/// engineering `promptType` strings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSurfaceContractSnapshot {
    pub schema_version: u32,
    pub surface_kind: String,
    pub family: String,
    pub input_ownership: String,
    pub preview_role: String,
    pub focus_policy: String,
    pub keyboard_policy: String,
    pub actions_policy: String,
    pub proof_policy: String,
    pub visual_policy: String,
    pub automation_semantic_surface: String,
    pub native_footer_surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_identity: Option<AutomationTargetIdentitySnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<SurfacePresentationSnapshot>,
}

/// Schema version for the resolved active footer snapshot in `getState`.
pub const ACTIVE_FOOTER_SCHEMA_VERSION: u32 = 2;

/// Resolved footer owner visible to automation after native-host installation
/// and prompt fallback policy have both been applied.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFooterSnapshot {
    pub schema_version: u32,
    pub owner: String,
    pub expected_surface: Option<String>,
    pub requested_surface: Option<String>,
    pub active_surface: Option<String>,
    pub native_footer_host_installed: bool,
    pub gpui_fallback_visible: bool,
    pub left_info: Option<ActiveFooterLeftInfoSnapshot>,
    pub button_count: usize,
    pub action_slot_count: usize,
    pub context_chip_count: usize,
    pub duplicate_action_ids: Vec<String>,
    pub duplicate_shortcut_keys: Vec<String>,
    pub slot_contract_violation: Option<String>,
    pub buttons: Vec<ActiveFooterButtonSnapshot>,
    pub mismatch: Option<String>,
}

/// Machine-readable footer status/model text for `getState.activeFooter`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFooterLeftInfoSnapshot {
    pub dot_status: String,
    pub model_name: String,
    pub profile_name: Option<String>,
    pub icon_token: Option<String>,
    /// Key/sigil rendered as a keycap chip before the label (footer tips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keycap: Option<String>,
    pub action: Option<String>,
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd_chip: Option<ActiveFooterCwdChipSnapshot>,
}

/// CWD chip surfaced in the footer's left-info slot.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFooterCwdChipSnapshot {
    pub label: String,
    pub icon_token: String,
}

/// Machine-readable footer button state for `getState.activeFooter`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFooterButtonSnapshot {
    pub id: String,
    pub action: String,
    pub key: String,
    pub shortcut_tokens: Vec<String>,
    pub canonical_shortcut: Option<String>,
    pub shortcut_routable: bool,
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
    pub placement: String,
    pub action_disabled: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface_contract() -> LauncherSurfaceContractSnapshot {
        LauncherSurfaceContractSnapshot {
            schema_version: LAUNCHER_SURFACE_CONTRACT_SCHEMA_VERSION,
            surface_kind: "ScriptList".to_string(),
            family: "Launcher".to_string(),
            input_ownership: "Shared".to_string(),
            preview_role: "None".to_string(),
            focus_policy: "MainInput".to_string(),
            keyboard_policy: "Launcher".to_string(),
            actions_policy: "Shared".to_string(),
            proof_policy: "Direct".to_string(),
            visual_policy: "Shared".to_string(),
            automation_semantic_surface: "scriptList".to_string(),
            native_footer_surface: Some("scriptList".to_string()),
            target_identity: None,
            presentation: None,
        }
    }

    #[test]
    fn target_identity_exposes_real_generations_without_screenshot_metadata() {
        let mut snapshot = surface_contract();
        snapshot.target_identity = Some(AutomationTargetIdentitySnapshot {
            window_id: "main".to_string(),
            window_generation: Some(7),
            app_view_variant: "ScriptList".to_string(),
            target_generation: 11,
            surface_generation: 13,
            data_generation: 17,
        });

        let json = serde_json::to_value(&snapshot).expect("serialize target identity");
        let identity = &json["targetIdentity"];
        assert_eq!(identity["windowId"], "main");
        assert_eq!(identity["windowGeneration"], 7);
        assert_eq!(identity["appViewVariant"], "ScriptList");
        assert_eq!(identity["targetGeneration"], 11);
        assert_eq!(identity["surfaceGeneration"], 13);
        assert_eq!(identity["dataGeneration"], 17);
        assert!(identity.get("screenshot").is_none());
        assert!(identity.get("pixelProbes").is_none());
    }

    #[test]
    fn missing_target_identity_is_omitted_and_legacy_snapshots_still_decode() {
        let snapshot = surface_contract();
        let json = serde_json::to_value(&snapshot).expect("serialize legacy-compatible snapshot");
        assert!(json.get("targetIdentity").is_none());

        let decoded: LauncherSurfaceContractSnapshot =
            serde_json::from_value(json).expect("decode snapshot without target identity");
        assert_eq!(decoded.target_identity, None);
        assert_eq!(decoded.presentation, None);
    }

    #[test]
    fn presentation_uses_closed_shared_row_primitive_vocabulary() {
        let mut snapshot = surface_contract();
        snapshot.presentation = Some(SurfacePresentationSnapshot {
            shell_owner: "components::main_view_chrome".to_string(),
            input_owner: "components::text_input".to_string(),
            row_primitive: SurfaceRowPrimitive::LegacyListItem,
            footer_owner: Some("footer_popup::native_footer".to_string()),
            actions_owner: Some("actions::dialog".to_string()),
            theme_owner: "theme::AppChromeColors + ui::chrome::tokens".to_string(),
            intentional_divergence: None,
        });

        let json = serde_json::to_value(snapshot).expect("serialize surface anatomy");
        assert_eq!(json["presentation"]["rowPrimitive"], "legacyListItem");
        assert_eq!(json["presentation"]["inputOwner"], "components::text_input");
        assert!(json["presentation"].get("intentionalDivergence").is_none());
    }
}

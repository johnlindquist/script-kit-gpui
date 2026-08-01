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

use serde::{Deserialize, Serialize};

/// Schema version for the automation window targeting contract.
pub const AUTOMATION_WINDOW_SCHEMA_VERSION: u32 = 3;

/// Specifies which automation window a command should target.
///
/// When omitted from a request, the app defaults to the focused window
/// (equivalent to `Focused`). This keeps existing requests backward-compatible.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AutomationWindowTarget {
    /// The main Script Kit launcher window.
    Main,
    /// Whichever window currently has focus (default when target is omitted).
    Focused,
    /// A specific window by its stable automation ID. Resolves the current lifetime.
    Id { id: String },
    /// One exact window lifetime. Reusing the stable ID for a reopened window
    /// does not make an older instance target valid again.
    Instance { id: String, generation: u64 },
    /// The first (or Nth) window of a given kind.
    Kind {
        kind: AutomationWindowKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// First window whose title contains the given text.
    TitleContains { text: String },
}

/// Well-known window kinds that the automation registry tracks.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, strum::EnumIter)]
#[serde(rename_all = "camelCase")]
pub enum AutomationWindowKind {
    Main,
    Notes,
    AgentChatDetached,
    Dictation,
    ActionsDialog,
    PromptPopup,
    /// Transient HUD notification pill (floating, focusless, auto-dismissing).
    Hud,
    /// Production snap target overlay, independent of a launcher parent.
    SnapOverlay,
}

impl AutomationWindowKind {
    /// Canonical camelCase string for this kind — the exact form carried on
    /// the wire by `serde(rename_all = "camelCase")`. Used by automation
    /// receipts (e.g. `AgentChatResolvedTarget.windowKind`) so agentic callers
    /// see the same value they would receive from `listAutomationWindows`.
    pub fn as_camel_case(self) -> &'static str {
        match self {
            AutomationWindowKind::Main => "main",
            AutomationWindowKind::Notes => "notes",
            AutomationWindowKind::AgentChatDetached => "agentChatDetached",
            AutomationWindowKind::Dictation => "dictation",
            AutomationWindowKind::ActionsDialog => "actionsDialog",
            AutomationWindowKind::PromptPopup => "promptPopup",
            AutomationWindowKind::Hud => "hud",
            AutomationWindowKind::SnapOverlay => "snapOverlay",
        }
    }
}

/// Pixel bounds of an automation window.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationWindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Descriptor for a single automation-addressable window.
///
/// Returned by `listAutomationWindows` and used internally by the
/// window registry to resolve `AutomationWindowTarget` queries.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationWindowInfo {
    /// Stable automation ID (e.g. `"main"`, `"agentChatDetached:thread-1"`).
    pub id: String,
    /// The kind of window this represents.
    pub kind: AutomationWindowKind,
    /// Human-readable window title, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Whether this window currently has keyboard focus.
    pub focused: bool,
    /// Whether this window is currently visible on screen.
    pub visible: bool,
    /// Semantic surface name for element collection routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_surface: Option<String>,
    /// Window bounds in screen coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<AutomationWindowBounds>,
    /// For attached popups: the automation ID of the parent window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_window_id: Option<String>,
    /// Exact parent lifetime; reopening the same stable ID does not reparent a popup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_window_generation: Option<u64>,
    /// For attached popups: the kind of the parent window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_kind: Option<AutomationWindowKind>,
    /// Monotonic lifetime generation for strict instance targeting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    /// Process ID of the window owner (useful for attaching profilers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

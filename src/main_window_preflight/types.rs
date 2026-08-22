/// Receipt types for the main-window Execution Contract rail.
///
/// These types describe "what Enter will do" and "what Command+Enter will send"
/// for the currently selected item in the ScriptList view.  They are
/// serializable so an AI agent (or test harness) can inspect the
/// contract without parsing the UI.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MainWindowPreflightActionKind {
    RunScript,
    RunSnippet,
    RunCommand,
    LaunchApp,
    SwitchWindow,
    OpenFile,
    RunAgent,
    RunFallback,
    OpenSkill,
    AskAi,
    InspectIssues,
    PasteResumeCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainWindowPreflightAction {
    pub kind: MainWindowPreflightActionKind,
    pub label: String,
    pub subject: String,
    pub type_label: String,
    pub source_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MainWindowPreflightResultRole {
    Primary,
    RootFile,
    RootPassive,
    Fallback,
    ScriptIssue,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainWindowPreflightVisibleResult {
    pub visible_rank: usize,
    pub grouped_index: usize,
    pub stable_key: Option<String>,
    pub role: MainWindowPreflightResultRole,
    pub action_kind: MainWindowPreflightActionKind,
    pub type_label: String,
    pub source_name: Option<String>,
    pub description: Option<String>,
    pub leading_icon_present: bool,
    pub leading_icon_kind: Option<String>,
    pub leading_icon_bundle_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RootPassiveSourceReceipt {
    pub enabled: bool,
    pub frame_count: usize,
    pub cache_generation: u64,
    pub frame_generation: u64,
    pub refreshing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RootPassiveFrameReceipt {
    pub query: String,
    pub source_filters: Vec<String>,
    pub notes: RootPassiveSourceReceipt,
    pub clipboard_history: RootPassiveSourceReceipt,
    pub dictation_history: RootPassiveSourceReceipt,
    pub agent_chat_history: RootPassiveSourceReceipt,
    pub ai_vault: RootPassiveSourceReceipt,
    pub browser_tabs: RootPassiveSourceReceipt,
    pub browser_history: RootPassiveSourceReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainWindowPreflightReceipt {
    pub filter_text: String,
    pub computed_search_text: String,
    pub source_filters: Vec<String>,
    pub filter_indicators: Vec<crate::menu_syntax::FilterIndicator>,
    pub selected_index: usize,
    pub selected_result_key: Option<String>,
    pub selected_result_role: Option<MainWindowPreflightResultRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_command: Option<crate::scripts::LauncherCommandReceipt>,
    pub visible_results: Vec<MainWindowPreflightVisibleResult>,
    pub visible_result_key_fingerprint: String,
    pub visible_row_fingerprint: String,
    pub visible_result_count: usize,
    pub root_passive_frame: Option<RootPassiveFrameReceipt>,
    pub enter_action: Option<MainWindowPreflightAction>,
    pub tab_action: Option<MainWindowPreflightAction>,
    pub warnings: Vec<String>,
}

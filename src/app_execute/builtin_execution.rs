/// Delay between hiding the main window and starting a synchronous screenshot capture.
const AI_CAPTURE_HIDE_SETTLE_MS: u64 = 150;

/// Synthetic prompt ID used when the microphone-selection MiniPrompt is open.
/// Checked in `submit_arg_prompt_from_current_state` to intercept the submit
/// and persist the chosen device instead of sending a protocol message.
const BUILTIN_MIC_SELECT_PROMPT_ID: &str = "builtin:select-microphone";

/// Choice value representing "use system default" in the mic-selection prompt.
const BUILTIN_MIC_DEFAULT_VALUE: &str = "__system_default__";

/// Synthetic prompt ID for snap mode configuration.
const BUILTIN_SNAP_MODE_PROMPT_ID: &str = "builtin:configure-snap-mode";

/// Synthetic prompt ID for the dictation model download consent prompt.
/// Checked in `submit_arg_prompt_from_current_state` to intercept submit
/// and either start the Parakeet download or cancel.
const BUILTIN_DICTATION_MODEL_PROMPT_ID: &str = "builtin:dictation-model";

/// Choice value: user wants to download the Parakeet model.
const BUILTIN_DICTATION_MODEL_DOWNLOAD: &str = "download";
const BUILTIN_DICTATION_MODEL_USE_RECOMMENDED: &str = "use-recommended";

/// Choice value: user declines the download for now.
const BUILTIN_DICTATION_MODEL_CANCEL: &str = "cancel";

/// Choice value: user wants to hide the prompt while download continues.
const BUILTIN_DICTATION_MODEL_HIDE: &str = "builtin/dictation-model-hide";
const BUILTIN_DICTATION_MODEL_START_PENDING: &str = "start-pending";

/// Synthetic prompt ID for the full dictation setup prompt.
const BUILTIN_DICTATION_SETUP_PROMPT_ID: &str = "builtin:dictation-setup";
const BUILTIN_DICTATION_SETUP_START: &str = "start-dictation";
const BUILTIN_DICTATION_SETUP_DOWNLOAD_MODEL: &str = "download-model";
const BUILTIN_DICTATION_SETUP_REQUEST_MIC: &str = "request-microphone";
const BUILTIN_DICTATION_SETUP_OPEN_MIC_SETTINGS: &str = "open-microphone-settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiImageCaptureBuiltinAction {
    SendScreen,
    SendFocusedWindow,
    SendScreenArea,
}

impl AiImageCaptureBuiltinAction {
    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SendScreen => format!("Failed to capture screen: {error}"),
            Self::SendFocusedWindow => format!("Failed to capture window: {error}"),
            Self::SendScreenArea => format!("Failed to capture screen area: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiTextCaptureBuiltinAction {
    AgentChatContent,
    CurrentAppContext,
}

impl AiTextCaptureBuiltinAction {
    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::AgentChatContent => {
                format!("Failed to capture content for Agent Chat: {error}")
            }
            Self::CurrentAppContext => {
                format!("Failed to capture current app context: {error}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationBuiltinAction {
    CurrentSurface,
    AgentChat,
    FrontmostApp,
    Notes,
}

impl DictationBuiltinAction {
    fn opening_message(self) -> &'static str {
        match self {
            Self::CurrentSurface => "Opening Dictation",
            Self::AgentChat => "Opening Dictation to Agent Chat",
            Self::FrontmostApp => "Opening Dictation to Frontmost App",
            Self::Notes => "Opening Dictation to Notes",
        }
    }

    fn failure_message(self) -> &'static str {
        match self {
            Self::CurrentSurface => "Failed to toggle dictation",
            Self::AgentChat => "Failed to toggle dictation to Agent Chat",
            Self::FrontmostApp => "Failed to toggle dictation to frontmost app",
            Self::Notes => "Failed to toggle dictation to notes",
        }
    }

    fn preflight_failure_message(self) -> &'static str {
        match self {
            Self::CurrentSurface => "Dictation start preflight failed",
            Self::AgentChat => "Dictation-to-Agent-Chat start preflight failed",
            Self::FrontmostApp => "Dictation-to-app start preflight failed",
            Self::Notes => "Dictation-to-notes start preflight failed",
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::CurrentSurface => "dictation_toggle",
            Self::AgentChat => "dictation_to_ai_toggle",
            Self::FrontmostApp => "dictation_to_frontmost_app_toggle",
            Self::Notes => "dictation_to_notes_toggle",
        }
    }

    fn forced_target(self) -> Option<crate::dictation::DictationTarget> {
        match self {
            Self::CurrentSurface => None,
            Self::AgentChat => Some(crate::dictation::DictationTarget::TabAiHarness),
            Self::FrontmostApp => Some(crate::dictation::DictationTarget::ExternalApp),
            Self::Notes => Some(crate::dictation::DictationTarget::NotesEditor),
        }
    }

    fn stop_fallback_target(self) -> crate::dictation::DictationTarget {
        self.forced_target()
            .unwrap_or(crate::dictation::DictationTarget::ExternalApp)
    }

    fn conceal_before_overlay(self) -> bool {
        matches!(self, Self::AgentChat | Self::FrontmostApp)
    }

    fn dispatch_start_before_overlay(self) -> bool {
        matches!(self, Self::CurrentSurface)
    }

    fn log_forced_route(self) -> bool {
        matches!(self, Self::FrontmostApp | Self::Notes)
    }

    fn pending_target_label(self) -> &'static str {
        match self {
            Self::CurrentSurface => "Dictation",
            Self::AgentChat => "Agent Chat",
            Self::FrontmostApp => "Frontmost App",
            Self::Notes => "Notes",
        }
    }

    fn builtin_id(self) -> &'static str {
        match self {
            Self::CurrentSurface => "builtin/dictation",
            Self::AgentChat => "builtin/dictation-to-ai",
            Self::FrontmostApp => "builtin/dictation-to-app",
            Self::Notes => "builtin/dictation-to-notes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationStartPreflight {
    Ready,
    OpenedSetup,
    DownloadInProgress,
    OpenedModelPrompt,
    Failed,
}

impl DictationStartPreflight {
    fn success_detail(self) -> &'static str {
        match self {
            Self::Ready => "dictation_ready",
            Self::OpenedSetup => "dictation_setup_opened",
            Self::DownloadInProgress => "dictation_model_download_in_progress",
            Self::OpenedModelPrompt => "dictation_model_prompt_opened",
            Self::Failed => "dictation_preflight_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PasteSequentialBuiltinAction {
    PasteEntry(String),
    SequenceExhausted,
    HistoryEmpty,
}

impl PasteSequentialBuiltinAction {
    fn from_outcome(outcome: clipboard_history::PasteSequentialOutcome) -> Self {
        match outcome {
            clipboard_history::PasteSequentialOutcome::Pasted(entry_id) => {
                Self::PasteEntry(entry_id)
            }
            clipboard_history::PasteSequentialOutcome::Exhausted => Self::SequenceExhausted,
            clipboard_history::PasteSequentialOutcome::Empty => Self::HistoryEmpty,
        }
    }

    fn telemetry_event(&self) -> &'static str {
        match self {
            Self::PasteEntry(_) => "paste_entry",
            Self::SequenceExhausted => "sequence_exhausted",
            Self::HistoryEmpty => "history_empty",
        }
    }

    fn log_message(&self) -> &'static str {
        match self {
            Self::PasteEntry(_) => "Enqueuing sequential paste via serialized worker",
            Self::SequenceExhausted => "Sequential paste exhausted all entries",
            Self::HistoryEmpty => "No clipboard history available for sequential paste",
        }
    }

    fn success_detail(&self) -> &'static str {
        match self {
            Self::PasteEntry(_) => "paste_sequential",
            Self::SequenceExhausted => "paste_sequential_exhausted",
            Self::HistoryEmpty => "paste_sequential_empty",
        }
    }

    fn hud_message(&self) -> Option<&'static str> {
        match self {
            Self::PasteEntry(_) => None,
            Self::SequenceExhausted => Some("Sequence complete"),
            Self::HistoryEmpty => Some("No clipboard history"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSnapModeBuiltinAction {
    Disable,
    Simple,
    Expanded,
    Precision,
    Configure,
}

impl SettingsSnapModeBuiltinAction {
    fn from_command(command: builtins::SettingsCommandType) -> Option<Self> {
        match command {
            builtins::SettingsCommandType::ConfigureSnapMode => Some(Self::Configure),
            _ => None,
        }
    }

    fn target_mode(self) -> window_control::SnapMode {
        match self {
            Self::Disable => window_control::SnapMode::Off,
            Self::Simple => window_control::SnapMode::Simple,
            Self::Expanded => window_control::SnapMode::Expanded,
            Self::Precision => window_control::SnapMode::Precision,
            Self::Configure => window_control::SnapMode::Off,
        }
    }

    fn hud_text(self) -> &'static str {
        match self {
            Self::Disable => "Window snapping disabled",
            Self::Simple => "Snap mode: Simple",
            Self::Expanded => "Snap mode: Expanded",
            Self::Precision => "Snap mode: Precision",
            Self::Configure => "Configure snap mode",
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Disable => "set_snap_mode::off",
            Self::Simple => "set_snap_mode::simple",
            Self::Expanded => "set_snap_mode::expanded",
            Self::Precision => "set_snap_mode::precision",
            Self::Configure => "configure_snap_mode",
        }
    }

    fn persistence_failure_code(self) -> &'static str {
        "set_snap_mode_failed"
    }

    fn persistence_failure_log(self) -> &'static str {
        "Failed to persist snap mode from built-in command"
    }

    fn persistence_failure_hud(self, error: &dyn std::fmt::Display) -> String {
        format!("Failed to update snap mode: {error}")
    }

    fn persistence_failure_message(self) -> &'static str {
        "Failed to save snap mode"
    }

    fn runtime_transition_failure_log(self) -> &'static str {
        "Failed to apply runtime transition after snap mode change"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsMicrophoneBuiltinAction {
    Select,
}

impl SettingsMicrophoneBuiltinAction {
    fn from_command(command: builtins::SettingsCommandType) -> Option<Self> {
        match command {
            builtins::SettingsCommandType::SelectMicrophone => Some(Self::Select),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Select => "select_microphone",
        }
    }

    fn enumeration_failure_log(self) -> &'static str {
        match self {
            Self::Select => "Failed to enumerate microphone devices",
        }
    }

    fn failure_hud(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Select => format!("Failed to list microphones: {error}"),
        }
    }

    fn failure_code(self) -> &'static str {
        match self {
            Self::Select => "select_microphone_failed",
        }
    }

    fn failure_message(self) -> &'static str {
        match self {
            Self::Select => "Failed to list microphones",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            Self::Select => "Select microphone...",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionAssistantBuiltinAction {
    Accessibility,
    ScreenRecording,
}

impl PermissionAssistantBuiltinAction {
    fn from_command(command: builtins::PermissionCommandType) -> Option<Self> {
        match command {
            builtins::PermissionCommandType::AllowAccessibility => Some(Self::Accessibility),
            builtins::PermissionCommandType::AllowScreenRecording => Some(Self::ScreenRecording),
            builtins::PermissionCommandType::CheckPermissions
            | builtins::PermissionCommandType::SetupPermissions
            | builtins::PermissionCommandType::RequestAccessibility
            | builtins::PermissionCommandType::OpenAccessibilitySettings => None,
        }
    }

    fn panel(self) -> platform::permiso::PermisoPanel {
        match self {
            Self::Accessibility => platform::permiso::PermisoPanel::Accessibility,
            Self::ScreenRecording => platform::permiso::PermisoPanel::ScreenRecording,
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::Accessibility => "Drag Script Kit into Accessibility",
            Self::ScreenRecording => "Drag Script Kit into Screen Recording",
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Accessibility => "allow_accessibility",
            Self::ScreenRecording => "allow_screen_recording",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::Accessibility => "allow_accessibility_failed",
            Self::ScreenRecording => "allow_screen_recording_failed",
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Accessibility | Self::ScreenRecording => {
                format!("Failed to open Permission Assistant: {error}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionCommandBuiltinAction {
    CheckPermissions,
    SetupPermissions,
    RequestAccessibility,
    OpenAccessibilitySettings,
    Assistant(PermissionAssistantBuiltinAction),
}

impl PermissionCommandBuiltinAction {
    fn from_command(command: builtins::PermissionCommandType) -> Self {
        match command {
            builtins::PermissionCommandType::CheckPermissions => Self::CheckPermissions,
            builtins::PermissionCommandType::SetupPermissions => Self::SetupPermissions,
            builtins::PermissionCommandType::RequestAccessibility => Self::RequestAccessibility,
            builtins::PermissionCommandType::OpenAccessibilitySettings => {
                Self::OpenAccessibilitySettings
            }
            builtins::PermissionCommandType::AllowAccessibility
            | builtins::PermissionCommandType::AllowScreenRecording => Self::Assistant(
                PermissionAssistantBuiltinAction::from_command(command)
                    .expect("permission assistant command should map to assistant action"),
            ),
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::CheckPermissions => "check_permissions",
            Self::SetupPermissions => "setup_permissions",
            Self::RequestAccessibility => "request_accessibility",
            Self::OpenAccessibilitySettings => "open_accessibility_settings",
            Self::Assistant(action) => action.success_detail(),
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::OpenAccessibilitySettings => "open_accessibility_settings_failed",
            Self::Assistant(action) => action.failure_detail(),
            Self::CheckPermissions | Self::SetupPermissions | Self::RequestAccessibility => "",
        }
    }

    fn all_permissions_granted_hud(self) -> &'static str {
        match self {
            Self::CheckPermissions => "All permissions granted!",
            Self::SetupPermissions
            | Self::RequestAccessibility
            | Self::OpenAccessibilitySettings
            | Self::Assistant(_) => "",
        }
    }

    fn accessibility_granted_hud(self) -> &'static str {
        match self {
            Self::RequestAccessibility => "Accessibility permission granted!",
            Self::CheckPermissions
            | Self::SetupPermissions
            | Self::OpenAccessibilitySettings
            | Self::Assistant(_) => "",
        }
    }

    fn accessibility_not_granted_warning(self) -> &'static str {
        match self {
            Self::RequestAccessibility => {
                "Accessibility permission not granted. Some features may not work."
            }
            Self::CheckPermissions
            | Self::SetupPermissions
            | Self::OpenAccessibilitySettings
            | Self::Assistant(_) => "",
        }
    }

    fn open_settings_failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::OpenAccessibilitySettings => format!("Failed to open settings: {error}"),
            Self::CheckPermissions
            | Self::SetupPermissions
            | Self::RequestAccessibility
            | Self::Assistant(_) => String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityOpenBuiltinAction {
    MainWindow,
    ScratchPad,
    QuickTerminal,
    ClaudeCode,
    ProcessManager,
}

impl UtilityOpenBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Option<Self> {
        match command {
            builtins::UtilityCommandType::MainWindow => Some(Self::MainWindow),
            builtins::UtilityCommandType::ScratchPad => Some(Self::ScratchPad),
            builtins::UtilityCommandType::QuickTerminal => Some(Self::QuickTerminal),
            builtins::UtilityCommandType::ClaudeCode => Some(Self::ClaudeCode),
            builtins::UtilityCommandType::ProcessManager => Some(Self::ProcessManager),
            builtins::UtilityCommandType::StopAllProcesses
            | builtins::UtilityCommandType::ScriptKitSelfie
            | builtins::UtilityCommandType::DoInCurrentApp
            | builtins::UtilityCommandType::TurnThisIntoCommand
            | builtins::UtilityCommandType::CurrentAppCommands => None,
        }
    }

    fn opening_message(self) -> Option<&'static str> {
        match self {
            Self::MainWindow => Some("Opening Main Window"),
            Self::ScratchPad | Self::QuickTerminal | Self::ClaudeCode | Self::ProcessManager => {
                None
            }
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::MainWindow => "open_main_window",
            Self::ScratchPad => "open_scratch_pad",
            Self::QuickTerminal => "open_quick_terminal",
            Self::ClaudeCode => "open_claude_code_terminal",
            Self::ProcessManager => "open_process_manager",
        }
    }

    fn opens_from_main_menu(self) -> bool {
        matches!(
            self,
            Self::ScratchPad | Self::QuickTerminal | Self::ClaudeCode
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityProcessBuiltinAction {
    StopAllProcesses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityProcessBuiltinOutcome {
    NoRunningProcesses,
    StopRequested { process_count: usize },
}

impl UtilityProcessBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Option<Self> {
        match command {
            builtins::UtilityCommandType::StopAllProcesses => Some(Self::StopAllProcesses),
            _ => None,
        }
    }

    fn empty_hud(self) -> &'static str {
        match self {
            Self::StopAllProcesses => "No running scripts to stop.",
        }
    }

    fn success_hud(self, process_count: usize) -> String {
        match self {
            Self::StopAllProcesses => {
                format!("Stopped {process_count} running script process(es).")
            }
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::StopAllProcesses => "stop_all_processes",
        }
    }

    fn outcome(self, process_count: usize) -> UtilityProcessBuiltinOutcome {
        let _ = self;
        match process_count {
            0 => UtilityProcessBuiltinOutcome::NoRunningProcesses,
            process_count => UtilityProcessBuiltinOutcome::StopRequested { process_count },
        }
    }
}

impl UtilityProcessBuiltinOutcome {
    fn should_stop_processes(self) -> bool {
        matches!(self, Self::StopRequested { .. })
    }

    fn process_count(self) -> usize {
        match self {
            Self::NoRunningProcesses => 0,
            Self::StopRequested { process_count } => process_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilitySelfieBuiltinAction {
    Selfie,
}

impl UtilitySelfieBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Option<Self> {
        match command {
            builtins::UtilityCommandType::ScriptKitSelfie => Some(Self::Selfie),
            _ => None,
        }
    }

    fn starting_hud(self, state: &str) -> String {
        match self {
            Self::Selfie => format!("Capturing Script Kit selfie: {state}"),
        }
    }

    fn saved_hud(self, receipt: &crate::platform::ScriptKitSelfieReceipt) -> String {
        match self {
            Self::Selfie => format!("Selfie saved: {}", receipt.png_path),
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Selfie => format!("Script Kit Selfie failed: {error}"),
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Selfie => "script_kit_selfie_saved",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::Selfie => "script_kit_selfie_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityRecipeBuiltinAction {
    TurnThisIntoCommand,
}

impl UtilityRecipeBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Option<Self> {
        match command {
            builtins::UtilityCommandType::TurnThisIntoCommand => Some(Self::TurnThisIntoCommand),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command",
        }
    }

    fn clipboard_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_clipboard_failed",
        }
    }

    fn serialize_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_serialize_failed",
        }
    }

    fn serialize_failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::TurnThisIntoCommand => {
                format!("Failed to serialize current app command recipe: {error}")
            }
        }
    }

    fn capture_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_capture_failed",
        }
    }

    fn missing_query_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_missing_query",
        }
    }

    fn drift_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_drift",
        }
    }

    fn missing_entry_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_missing_entry_index",
        }
    }

    fn open_palette_success_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_open_palette",
        }
    }

    fn generate_script_success_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_generate_script",
        }
    }

    fn copied_recipe_hud(self, suggested_script_name: &str) -> String {
        match self {
            Self::TurnThisIntoCommand => {
                format!("Automation recipe copied: {suggested_script_name}")
            }
        }
    }

    fn unknown_action_failure_detail(self) -> &'static str {
        match self {
            Self::TurnThisIntoCommand => "turn_this_into_command_unknown_action",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityDoInCurrentAppBuiltinAction {
    Submit,
}

impl UtilityDoInCurrentAppBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Option<Self> {
        match command {
            builtins::UtilityCommandType::DoInCurrentApp => Some(Self::Submit),
            _ => None,
        }
    }

    fn open_palette_success_detail(self) -> &'static str {
        match self {
            Self::Submit => "do_in_current_app_open_palette",
        }
    }

    fn generate_script_success_detail(self) -> &'static str {
        match self {
            Self::Submit => "do_in_current_app_generate_script_scheduled",
        }
    }

    fn capture_failure_detail(self) -> &'static str {
        match self {
            Self::Submit => "do_in_current_app_capture_failed",
        }
    }

    fn capture_failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Submit => format!("Failed to load frontmost app menu bar: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityCurrentAppCommandsBuiltinAction {
    Open,
}

impl UtilityCurrentAppCommandsBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Option<Self> {
        match command {
            builtins::UtilityCommandType::CurrentAppCommands => Some(Self::Open),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Open => "open_current_app_commands",
        }
    }

    fn capture_failure_detail(self) -> &'static str {
        match self {
            Self::Open => "current_app_commands_capture_failed",
        }
    }

    fn capture_failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Open => format!("Failed to load frontmost app menu bar: {error}"),
        }
    }

    fn refresh_failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Open => format!("Failed to refresh current app commands: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UtilityCommandBuiltinAction {
    Open(UtilityOpenBuiltinAction),
    Process(UtilityProcessBuiltinAction),
    Selfie(UtilitySelfieBuiltinAction),
    Recipe(UtilityRecipeBuiltinAction),
    DoInCurrentApp(UtilityDoInCurrentAppBuiltinAction),
    CurrentAppCommands(UtilityCurrentAppCommandsBuiltinAction),
}

impl UtilityCommandBuiltinAction {
    fn from_command(command: builtins::UtilityCommandType) -> Self {
        match command {
            builtins::UtilityCommandType::MainWindow
            | builtins::UtilityCommandType::ScratchPad
            | builtins::UtilityCommandType::QuickTerminal
            | builtins::UtilityCommandType::ClaudeCode
            | builtins::UtilityCommandType::ProcessManager => Self::Open(
                UtilityOpenBuiltinAction::from_command(command)
                    .expect("utility open command should map to open action"),
            ),
            builtins::UtilityCommandType::StopAllProcesses => Self::Process(
                UtilityProcessBuiltinAction::from_command(command)
                    .expect("utility process command should map to process action"),
            ),
            builtins::UtilityCommandType::ScriptKitSelfie => Self::Selfie(
                UtilitySelfieBuiltinAction::from_command(command)
                    .expect("utility selfie command should map to selfie action"),
            ),
            builtins::UtilityCommandType::TurnThisIntoCommand => Self::Recipe(
                UtilityRecipeBuiltinAction::from_command(command)
                    .expect("utility recipe command should map to recipe action"),
            ),
            builtins::UtilityCommandType::DoInCurrentApp => Self::DoInCurrentApp(
                UtilityDoInCurrentAppBuiltinAction::from_command(command)
                    .expect("utility do-in command should map to do-in action"),
            ),
            builtins::UtilityCommandType::CurrentAppCommands => Self::CurrentAppCommands(
                UtilityCurrentAppCommandsBuiltinAction::from_command(command)
                    .expect("utility current-app command should map to current-app action"),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuBarBuiltinAction {
    Execute,
}

impl MenuBarBuiltinAction {
    fn from_action(_action: &builtins::MenuBarActionInfo) -> Self {
        Self::Execute
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Execute => "menu_bar_action",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::Execute => "menu_bar_action_failed",
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Execute => format!("Menu action failed: {error}"),
        }
    }

    fn unsupported_detail(self) -> &'static str {
        match self {
            Self::Execute => "menu_bar_action_unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemBuiltinAction {
    Dispatch,
}

impl SystemBuiltinAction {
    fn from_action(_action_type: &builtins::SystemActionType) -> Self {
        Self::Dispatch
    }

    fn handler_name(self) -> &'static str {
        match self {
            Self::Dispatch => "Executing system action via inner path",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceOpenBuiltinAction {
    ClipboardHistory,
    Favorites,
    AppLauncher,
    DesignGallery,
    FooterGallery,
    NonListStates,
    AiChat(crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant),
    EmojiPicker,
    Webcam,
    FileSearch,
    Settings,
    AgentChatHistory,
    AiVault,
    DictationHistory,
    SdkReference,
    ScriptTemplateCatalog,
    MigrateV1Scripts,
}

impl SurfaceOpenBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::ClipboardHistory => Some(Self::ClipboardHistory),
            builtins::BuiltInFeature::Favorites => Some(Self::Favorites),
            builtins::BuiltInFeature::AppLauncher => Some(Self::AppLauncher),
            builtins::BuiltInFeature::DesignGallery => Some(Self::DesignGallery),
            builtins::BuiltInFeature::FooterGallery => Some(Self::FooterGallery),
            builtins::BuiltInFeature::DesignNonListStates => Some(Self::NonListStates),
            builtins::BuiltInFeature::AiChat => Some(Self::AiChat(
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            )),
            builtins::BuiltInFeature::AiChatVariant(variant) => Some(Self::AiChat(*variant)),
            builtins::BuiltInFeature::EmojiPicker => Some(Self::EmojiPicker),
            builtins::BuiltInFeature::Webcam => Some(Self::Webcam),
            builtins::BuiltInFeature::FileSearch => Some(Self::FileSearch),
            builtins::BuiltInFeature::Settings => Some(Self::Settings),
            builtins::BuiltInFeature::AgentChatHistory => Some(Self::AgentChatHistory),
            builtins::BuiltInFeature::AiVault => Some(Self::AiVault),
            builtins::BuiltInFeature::DictationHistory => Some(Self::DictationHistory),
            builtins::BuiltInFeature::SdkReference => Some(Self::SdkReference),
            builtins::BuiltInFeature::NewScriptFromTemplate => Some(Self::ScriptTemplateCatalog),
            builtins::BuiltInFeature::MigrateV1Scripts => Some(Self::MigrateV1Scripts),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::ClipboardHistory => "open_clipboard_history",
            Self::Favorites => "open_favorites_view",
            Self::AppLauncher => "open_app_launcher",
            Self::DesignGallery => "open_design_gallery",
            Self::FooterGallery => "open_footer_gallery",
            Self::NonListStates => "open_non_list_states",
            Self::AiChat(crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard) => {
                "open_ai_harness_dispatched"
            }
            Self::AiChat(_) => "open_ai_harness_variant_dispatched",
            Self::EmojiPicker => "open_emoji_picker",
            Self::Webcam => "open_webcam",
            Self::FileSearch => "open_file_search",
            Self::Settings => "open_settings",
            Self::AgentChatHistory => "open_agent_chat_history",
            Self::AiVault => "open_ai_vault",
            Self::DictationHistory => "open_dictation_history",
            Self::SdkReference => "open_sdk_reference",
            Self::ScriptTemplateCatalog => "open_script_template_catalog",
            Self::MigrateV1Scripts => "open_migrate_v1",
        }
    }

    fn log_message(self) -> &'static str {
        match self {
            Self::ClipboardHistory => "Opening Clipboard History",
            Self::Favorites => "Opening Favorites browse view",
            Self::AppLauncher => "Opening App Launcher",
            Self::DesignGallery => "Opening Design Gallery",
            Self::FooterGallery => "Opening Footer Gallery",
            Self::NonListStates => "Opening Non-List States",
            Self::AiChat(_) => "Opening Agent Chat",
            Self::EmojiPicker => "Opening Emoji Picker",
            Self::Webcam => "Opening Webcam",
            Self::FileSearch => "Opening File Search",
            Self::Settings => "Opening Settings",
            Self::AgentChatHistory => "Opening Agent Chat History",
            Self::AiVault => "Opening AI Vault",
            Self::DictationHistory => "Opening Dictation History",
            Self::SdkReference => "Opening SDK Reference",
            Self::ScriptTemplateCatalog => "Opening Script Template Catalog",
            Self::MigrateV1Scripts => "Opening Migrate v1 Scripts",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserTabsBuiltinAction {
    Open,
}

impl BrowserTabsBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::BrowserTabs => Some(Self::Open),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Open => "open_browser_tabs",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::Open => "open_browser_tabs_failed",
        }
    }

    fn opening_message(self) -> &'static str {
        match self {
            Self::Open => "Opening Browser Tabs",
        }
    }

    fn loaded_message(self) -> &'static str {
        match self {
            Self::Open => "Loaded browser tabs",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            Self::Open => "Search open browser tabs...",
        }
    }

    fn failure_message(self, error: &anyhow::Error) -> String {
        match self {
            Self::Open => format!("Failed to list browser tabs: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowSwitcherBuiltinAction {
    Open,
}

impl WindowSwitcherBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::WindowSwitcher => Some(Self::Open),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Open => "open_window_switcher",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::Open => "open_window_switcher_failed",
        }
    }

    fn opening_message(self) -> &'static str {
        match self {
            Self::Open => "Opening Window Switcher",
        }
    }

    fn loaded_message(self) -> &'static str {
        match self {
            Self::Open => "Loaded windows",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            Self::Open => "Search windows...",
        }
    }

    fn failure_message(self, error: &anyhow::Error) -> String {
        match self {
            Self::Open => format!("Failed to list windows: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppLaunchBuiltinAction {
    Launch,
}

impl AppLaunchBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::App(_) => Some(Self::Launch),
            _ => None,
        }
    }

    fn success_detail(self, app_name: &str) -> String {
        match self {
            Self::Launch => format!("launch_app::{app_name}"),
        }
    }

    fn not_found_detail(self, app_name: &str) -> String {
        match self {
            Self::Launch => format!("launch_app_not_found::{app_name}"),
        }
    }

    fn opening_message(self) -> &'static str {
        match self {
            Self::Launch => "Launching app",
        }
    }

    fn launch_failure_message(self, app_name: &str, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Launch => format!("Failed to launch {app_name}: {error}"),
        }
    }

    fn not_found_message(self, app_name: &str) -> String {
        match self {
            Self::Launch => format!("App not found: {app_name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotesBuiltinAction {
    Open,
}

impl NotesBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::Notes => Some(Self::Open),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Open => "open_notes",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::Open => "open_notes_failed",
        }
    }

    fn opening_message(self) -> &'static str {
        match self {
            Self::Open => "Opening Notes window",
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Open => format!("Failed to open Notes: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncToGithubBuiltinAction {
    Dispatch,
}

impl SyncToGithubBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::SyncToGithub => Some(Self::Dispatch),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Dispatch => "sync_to_github_dispatched",
        }
    }

    fn request_log_message(self) -> &'static str {
        match self {
            Self::Dispatch => "Sync to GitHub requested",
        }
    }

    fn start_hud(self) -> &'static str {
        match self {
            Self::Dispatch => "Syncing Script Kit to GitHub...",
        }
    }

    fn completed_log_message(self) -> &'static str {
        match self {
            Self::Dispatch => "Sync to GitHub completed",
        }
    }

    fn failed_log_message(self) -> &'static str {
        match self {
            Self::Dispatch => "Sync to GitHub failed",
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::Dispatch => format!("GitHub sync failed: {error}"),
        }
    }
}

#[cfg(feature = "storybook")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesignExplorerBuiltinAction {
    Open,
}

#[cfg(feature = "storybook")]
impl DesignExplorerBuiltinAction {
    fn from_feature(feature: &builtins::BuiltInFeature) -> Option<Self> {
        match feature {
            builtins::BuiltInFeature::DesignExplorer => Some(Self::Open),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::Open => "open_design_explorer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotesCommandBuiltinAction {
    OpenNotes,
    NewNote,
    SearchNotes,
    QuickCapture,
}

impl NotesCommandBuiltinAction {
    fn from_command(command: builtins::NotesCommandType) -> Self {
        match command {
            builtins::NotesCommandType::OpenNotes => Self::OpenNotes,
            builtins::NotesCommandType::NewNote => Self::NewNote,
            builtins::NotesCommandType::SearchNotes => Self::SearchNotes,
            builtins::NotesCommandType::QuickCapture => Self::QuickCapture,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::OpenNotes => "notes_command::OpenNotes",
            Self::NewNote => "notes_command::NewNote",
            Self::SearchNotes => "notes_command::SearchNotes",
            Self::QuickCapture => "notes_command::QuickCapture",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::OpenNotes => "notes_command_failed::OpenNotes",
            Self::NewNote => "notes_command_failed::NewNote",
            Self::SearchNotes => "notes_command_failed::SearchNotes",
            Self::QuickCapture => "notes_command_failed::QuickCapture",
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::OpenNotes | Self::NewNote | Self::SearchNotes | Self::QuickCapture => {
                format!("Notes command failed: {error}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptCommandBuiltinAction {
    NewScript,
    NewExtension,
}

impl ScriptCommandBuiltinAction {
    fn from_command(command: builtins::ScriptCommandType) -> Self {
        match command {
            builtins::ScriptCommandType::NewScript => Self::NewScript,
            builtins::ScriptCommandType::NewExtension => Self::NewExtension,
        }
    }

    fn naming_target(self) -> prompts::NamingTarget {
        match self {
            Self::NewScript => prompts::NamingTarget::Script,
            Self::NewExtension => prompts::NamingTarget::Extension,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::NewScript => "script_command::NewScript",
            Self::NewExtension => "script_command::NewExtension",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrecencyCommandBuiltinAction {
    ClearSuggested,
}

impl FrecencyCommandBuiltinAction {
    fn from_command(command: builtins::FrecencyCommandType) -> Self {
        match command {
            builtins::FrecencyCommandType::ClearSuggested => Self::ClearSuggested,
        }
    }

    fn hud_text(self) -> &'static str {
        match self {
            Self::ClearSuggested => "Suggested items cleared",
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::ClearSuggested => "clear_suggested",
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::ClearSuggested => "clear_suggested_failed",
        }
    }

    fn failure_message(self, error: &dyn std::fmt::Display) -> String {
        match self {
            Self::ClearSuggested => format!("Failed to clear suggested: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsCommandBuiltinAction {
    ResetWindowPositions,
    ChooseTheme,
    DictationSetup,
    SelectMicrophone(SettingsMicrophoneBuiltinAction),
    SnapMode(SettingsSnapModeBuiltinAction),
}

impl SettingsCommandBuiltinAction {
    fn from_command(command: builtins::SettingsCommandType) -> Self {
        match command {
            builtins::SettingsCommandType::ResetWindowPositions => Self::ResetWindowPositions,
            builtins::SettingsCommandType::ChooseTheme => Self::ChooseTheme,
            builtins::SettingsCommandType::DictationSetup => Self::DictationSetup,
            builtins::SettingsCommandType::SelectMicrophone => Self::SelectMicrophone(
                SettingsMicrophoneBuiltinAction::from_command(command)
                    .expect("select microphone command should map to microphone action"),
            ),
            builtins::SettingsCommandType::ConfigureSnapMode => Self::SnapMode(
                SettingsSnapModeBuiltinAction::from_command(command)
                    .expect("snap mode settings command should map to snap mode action"),
            ),
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::ResetWindowPositions => "reset_window_positions",
            Self::ChooseTheme => "choose_theme",
            Self::DictationSetup => "dictation_setup",
            Self::SelectMicrophone(action) => action.success_detail(),
            Self::SnapMode(action) => action.success_detail(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiCaptureBuiltinAction {
    FullScreen,
    FocusedWindow,
    SelectedText,
    BrowserTab,
}

impl AiCaptureBuiltinAction {
    fn from_command(command: builtins::AiCommandType) -> Option<Self> {
        match command {
            builtins::AiCommandType::SendScreenToAi => Some(Self::FullScreen),
            builtins::AiCommandType::SendFocusedWindowToAi => Some(Self::FocusedWindow),
            builtins::AiCommandType::SendSelectedTextToAi => Some(Self::SelectedText),
            builtins::AiCommandType::SendBrowserTabToAi => Some(Self::BrowserTab),
            _ => None,
        }
    }

    fn capture_kind(self) -> crate::ai::TabAiCaptureKind {
        match self {
            Self::FullScreen => crate::ai::TabAiCaptureKind::FullScreen,
            Self::FocusedWindow => crate::ai::TabAiCaptureKind::FocusedWindow,
            Self::SelectedText => crate::ai::TabAiCaptureKind::SelectedText,
            Self::BrowserTab => crate::ai::TabAiCaptureKind::BrowserTab,
        }
    }

    fn prompt(self) -> &'static str {
        match self {
            Self::FullScreen => "Capture and analyze the full screen.",
            Self::FocusedWindow => "Capture and analyze the focused window.",
            Self::SelectedText => "Use the current selected text as the primary subject.",
            Self::BrowserTab => {
                "Use the current browser tab URL and page context as the primary subject."
            }
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::FullScreen => "ai_send_screen_routed_to_harness",
            Self::FocusedWindow => "ai_send_focused_window_routed_to_harness",
            Self::SelectedText => "ai_send_selected_text_routed_to_harness",
            Self::BrowserTab => "ai_send_browser_tab_routed_to_harness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiGenerateBuiltinAction {
    NewScript,
    CurrentAppScript,
}

impl AiGenerateBuiltinAction {
    fn from_command(command: builtins::AiCommandType) -> Option<Self> {
        match command {
            builtins::AiCommandType::GenerateScript => Some(Self::NewScript),
            builtins::AiCommandType::GenerateScriptFromCurrentApp => Some(Self::CurrentAppScript),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::NewScript => "ai_generate_script_routed_to_harness",
            Self::CurrentAppScript => "ai_generate_script_from_current_app_routed_to_harness",
        }
    }

    fn normalized_request(self, query: &str) -> Option<String> {
        match self {
            Self::NewScript => {
                crate::menu_bar::current_app_commands::normalize_generate_script_request(Some(
                    query,
                ))
                .map(str::to_string)
            }
            Self::CurrentAppScript => {
                crate::menu_bar::current_app_commands::normalize_generate_script_from_current_app_request(
                    Some(query),
                )
                .map(str::to_string)
            }
        }
    }

    fn opens_agent_chat_when_request_empty(self) -> bool {
        match self {
            Self::NewScript => true,
            Self::CurrentAppScript => false,
        }
    }

    fn entry_intent(self, request: Option<String>) -> Option<String> {
        match self {
            Self::NewScript => request,
            Self::CurrentAppScript => Some(match request {
                Some(request) => format!(
                    "Generate a Script Kit script for the frontmost app \
                     using the current menu, selection, and browser context. \
                     User request: {request}"
                ),
                None => "Generate a Script Kit script for the frontmost app \
                     using the current menu, selection, and browser context."
                    .to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiUnavailableBuiltinAction {
    ScreenAreaCapture,
}

impl AiUnavailableBuiltinAction {
    fn from_command(command: builtins::AiCommandType) -> Option<Self> {
        match command {
            builtins::AiCommandType::SendScreenAreaToAi => Some(Self::ScreenAreaCapture),
            _ => None,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::ScreenAreaCapture => {
                "Send Screen Area to Agent Chat is unavailable until selected-area capture is attached to Agent Chat."
            }
        }
    }

    fn failure_detail(self) -> &'static str {
        match self {
            Self::ScreenAreaCapture => "ai_send_screen_area_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiLegacyHarnessBuiltinAction {
    OpenAi,
    MiniAi,
    NewConversation,
    ClearConversation,
}

impl AiLegacyHarnessBuiltinAction {
    fn from_command(command: builtins::AiCommandType) -> Option<Self> {
        match command {
            builtins::AiCommandType::OpenAi => Some(Self::OpenAi),
            builtins::AiCommandType::MiniAi => Some(Self::MiniAi),
            builtins::AiCommandType::NewConversation => Some(Self::NewConversation),
            builtins::AiCommandType::ClearConversation => Some(Self::ClearConversation),
            _ => None,
        }
    }

    fn success_detail(self) -> &'static str {
        match self {
            Self::OpenAi => "ai_OpenAi_routed_to_harness",
            Self::MiniAi => "ai_MiniAi_routed_to_harness",
            Self::NewConversation => "ai_NewConversation_routed_to_harness",
            Self::ClearConversation => "ai_ClearConversation_routed_to_harness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiCommandBuiltinAction {
    Generate(AiGenerateBuiltinAction),
    Capture(AiCaptureBuiltinAction),
    Unavailable(AiUnavailableBuiltinAction),
    LegacyHarness(AiLegacyHarnessBuiltinAction),
}

impl AiCommandBuiltinAction {
    fn from_command(command: builtins::AiCommandType) -> Self {
        match command {
            builtins::AiCommandType::GenerateScript => Self::Generate(
                AiGenerateBuiltinAction::from_command(command)
                    .expect("generate command should map to generate action"),
            ),
            builtins::AiCommandType::GenerateScriptFromCurrentApp => Self::Generate(
                AiGenerateBuiltinAction::from_command(command)
                    .expect("current-app generate command should map to generate action"),
            ),
            builtins::AiCommandType::SendScreenToAi => Self::Capture(
                AiCaptureBuiltinAction::from_command(command)
                    .expect("screen command should map to capture action"),
            ),
            builtins::AiCommandType::SendFocusedWindowToAi => Self::Capture(
                AiCaptureBuiltinAction::from_command(command)
                    .expect("focused-window command should map to capture action"),
            ),
            builtins::AiCommandType::SendSelectedTextToAi => Self::Capture(
                AiCaptureBuiltinAction::from_command(command)
                    .expect("selected-text command should map to capture action"),
            ),
            builtins::AiCommandType::SendBrowserTabToAi => Self::Capture(
                AiCaptureBuiltinAction::from_command(command)
                    .expect("browser-tab command should map to capture action"),
            ),
            builtins::AiCommandType::SendScreenAreaToAi => Self::Unavailable(
                AiUnavailableBuiltinAction::from_command(command)
                    .expect("screen-area command should map to unavailable action"),
            ),
            builtins::AiCommandType::OpenAi
            | builtins::AiCommandType::MiniAi
            | builtins::AiCommandType::NewConversation
            | builtins::AiCommandType::ClearConversation => Self::LegacyHarness(
                AiLegacyHarnessBuiltinAction::from_command(command)
                    .expect("legacy command should map to legacy harness action"),
            ),
        }
    }
}

/// Generate a stable semantic ID for a built-in prompt choice.
///
/// Format: `{prompt_id}:choice:{index}:{value_slug}`
///
/// `prompt_id` already contains the `builtin:` prefix (e.g. `builtin:select-microphone`).
fn builtin_choice_semantic_id(prompt_id: &str, index: usize, value: &str) -> String {
    crate::protocol::generate_semantic_id(&format!("{prompt_id}:choice"), index, value)
}

/// Typed progress events sent from the blocking download thread to the
/// async context for updating the in-prompt progress display.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DictationModelProgressEvent {
    Downloading {
        percentage: u8,
        downloaded_bytes: u64,
        total_bytes: u64,
        speed_bytes_per_sec: u64,
        eta_seconds: Option<u64>,
    },
    Extracting,
}

/// Simple rolling-window speed tracker for download progress.
struct SpeedTracker {
    last_bytes: u64,
    last_time: std::time::Instant,
    speed: u64,
}

impl SpeedTracker {
    fn new() -> Self {
        Self {
            last_bytes: 0,
            last_time: std::time::Instant::now(),
            speed: 0,
        }
    }

    fn update(&mut self, downloaded: u64) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        if elapsed >= 0.5 {
            let delta = downloaded.saturating_sub(self.last_bytes);
            self.speed = (delta as f64 / elapsed) as u64;
            self.last_bytes = downloaded;
            self.last_time = now;
        }
    }

    fn speed_bytes_per_sec(&self) -> u64 {
        self.speed
    }
}

/// Phases tracked by the UI coalescing emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationModelUiPhase {
    Downloading,
    Extracting,
}

/// Snapshot of the last UI-visible state, used to decide whether a new
/// progress event is worth publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DictationModelUiSnapshot {
    phase: DictationModelUiPhase,
    percentage: u8,
    eta_bucket_seconds: Option<u64>,
}

impl DictationModelUiSnapshot {
    fn downloading(percentage: u8, eta_seconds: Option<u64>) -> Self {
        Self {
            phase: DictationModelUiPhase::Downloading,
            percentage,
            eta_bucket_seconds: bucket_dictation_eta_seconds(eta_seconds),
        }
    }

    fn extracting() -> Self {
        Self {
            phase: DictationModelUiPhase::Extracting,
            percentage: 100,
            eta_bucket_seconds: Some(0),
        }
    }
}

/// Gates cosmetic UI updates so the download thread is never blocked on
/// repaints.  Publishes on meaningful change or after a ~300 ms heartbeat.
#[derive(Debug, Default)]
struct DictationModelUiEmitter {
    last_emit_at: Option<std::time::Instant>,
    last_snapshot: Option<DictationModelUiSnapshot>,
}

impl DictationModelUiEmitter {
    fn should_emit(&self, now: std::time::Instant, next: &DictationModelUiSnapshot) -> bool {
        const HEARTBEAT: std::time::Duration = std::time::Duration::from_millis(300);

        let Some(last_snapshot) = self.last_snapshot.as_ref() else {
            return true;
        };
        let Some(last_emit_at) = self.last_emit_at else {
            return true;
        };

        if last_snapshot.phase != next.phase {
            return true;
        }
        if last_snapshot.percentage != next.percentage {
            return true;
        }
        if last_snapshot.eta_bucket_seconds != next.eta_bucket_seconds {
            return true;
        }

        now.duration_since(last_emit_at) >= HEARTBEAT
    }

    fn record_emit(&mut self, now: std::time::Instant, next: &DictationModelUiSnapshot) {
        self.last_emit_at = Some(now);
        self.last_snapshot = Some(next.clone());
    }
}

/// Bucket ETA seconds into human-friendly steps so minor fluctuations
/// don't trigger a UI repaint.
fn bucket_dictation_eta_seconds(eta_seconds: Option<u64>) -> Option<u64> {
    eta_seconds.map(|value| match value {
        0..=15 => value,
        16..=60 => value - (value % 5),
        61..=300 => value - (value % 15),
        _ => value - (value % 60),
    })
}

/// Prevent overlapping Parakeet model downloads when the dictation hotkey is
/// pressed repeatedly while the model is still missing.
static PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static DICTATION_MODEL_PROMPT_STATUS: std::sync::OnceLock<
    parking_lot::Mutex<crate::dictation::DictationModelStatus>,
> = std::sync::OnceLock::new();

fn dictation_model_prompt_status()
-> &'static parking_lot::Mutex<crate::dictation::DictationModelStatus> {
    DICTATION_MODEL_PROMPT_STATUS.get_or_init(|| {
        parking_lot::Mutex::new(crate::dictation::DictationModelStatus::NotDownloaded)
    })
}

static PARAKEET_MODEL_DOWNLOAD_CANCEL: std::sync::OnceLock<
    parking_lot::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
> = std::sync::OnceLock::new();

fn parakeet_model_download_cancel_slot()
-> &'static parking_lot::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> {
    PARAKEET_MODEL_DOWNLOAD_CANCEL.get_or_init(|| parking_lot::Mutex::new(None))
}

static PENDING_DICTATION_MODEL_ACTION: std::sync::OnceLock<
    parking_lot::Mutex<Option<DictationBuiltinAction>>,
> = std::sync::OnceLock::new();

fn pending_dictation_model_action() -> &'static parking_lot::Mutex<Option<DictationBuiltinAction>> {
    PENDING_DICTATION_MODEL_ACTION.get_or_init(|| parking_lot::Mutex::new(None))
}

#[cfg(test)]
fn ai_open_failure_message(error: impl std::fmt::Display) -> String {
    format!("Failed to open AI: {error}")
}

#[derive(Debug)]
enum DeferredAiCapturedText {
    Ready(String),
    Empty(String),
}

fn ai_capture_hide_settle_duration() -> std::time::Duration {
    std::time::Duration::from_millis(AI_CAPTURE_HIDE_SETTLE_MS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiCommandWindowPlan {
    KeepMainWindowVisible,
    HideMainWindowDeferred,
    HideMainWindowForCapture,
}

impl AiCommandWindowPlan {
    fn from_command(cmd_type: &builtins::AiCommandType) -> Self {
        // All active AI commands now route to the harness terminal, which is
        // a view inside the main window — keep it visible.
        match cmd_type {
            builtins::AiCommandType::GenerateScript
            | builtins::AiCommandType::GenerateScriptFromCurrentApp
            | builtins::AiCommandType::SendScreenToAi
            | builtins::AiCommandType::SendFocusedWindowToAi
            | builtins::AiCommandType::SendSelectedTextToAi
            | builtins::AiCommandType::SendBrowserTabToAi
            | builtins::AiCommandType::SendScreenAreaToAi => Self::KeepMainWindowVisible,
            // Legacy aliases (OpenAi, MiniAi, NewConversation, ClearConversation)
            // also open the harness terminal inside the main window.
            cmd if cmd.is_legacy_harness_alias() => Self::KeepMainWindowVisible,
            // Preset commands (debug-only) retain their original behavior.
            _ => Self::HideMainWindowDeferred,
        }
    }

    fn keeps_main_window_visible(self) -> bool {
        matches!(self, Self::KeepMainWindowVisible)
    }

    fn uses_hide_then_capture_flow(self) -> bool {
        matches!(self, Self::HideMainWindowForCapture)
    }
}

fn ai_command_keeps_main_window_visible(cmd_type: &builtins::AiCommandType) -> bool {
    AiCommandWindowPlan::from_command(cmd_type).keeps_main_window_visible()
}

fn ai_command_uses_hide_then_capture_flow(cmd_type: &builtins::AiCommandType) -> bool {
    AiCommandWindowPlan::from_command(cmd_type).uses_hide_then_capture_flow()
}

#[cfg(test)]
fn favorites_loaded_message(count: usize) -> String {
    if count == 1 {
        "Loaded 1 favorite".to_string()
    } else {
        format!("Loaded {} favorites", count)
    }
}

#[cfg(test)]
fn created_file_path_for_feedback(path: &std::path::Path) -> std::path::PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    match std::env::current_dir() {
        Ok(current_dir) => current_dir.join(path),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn applescript_list_literal(values: &[String]) -> String {
    let escaped_values = values
        .iter()
        .map(|value| format!("\"{}\"", crate::utils::escape_applescript_string(value)))
        .join(", ");
    format!("{{{}}}", escaped_values)
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn choose_from_list(
    prompt: &str,
    ok_button: &str,
    values: &[String],
) -> Result<Option<String>, String> {
    if values.is_empty() {
        return Ok(None);
    }

    let list_literal = applescript_list_literal(values);
    let script = format!(
        r#"set selectedItem to choose from list {list_literal} with prompt "{prompt}" OK button name "{ok_button}" cancel button name "Cancel" without multiple selections allowed
if selectedItem is false then
    return ""
end if
return item 1 of selectedItem"#,
        list_literal = list_literal,
        prompt = crate::utils::escape_applescript_string(prompt),
        ok_button = crate::utils::escape_applescript_string(ok_button),
    );

    let selected = crate::platform::run_osascript(&script, "builtin_picker_choose_from_list")
        .map_err(|error| error.to_string())?;
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selected))
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn prompt_for_text(
    prompt: &str,
    default_value: &str,
    ok_button: &str,
) -> Result<Option<String>, String> {
    let script = format!(
        r#"try
set dialogResult to display dialog "{prompt}" default answer "{default_value}" buttons {{"Cancel", "{ok_button}"}} default button "{ok_button}"
return text returned of dialogResult
on error number -128
return ""
end try"#,
        prompt = crate::utils::escape_applescript_string(prompt),
        default_value = crate::utils::escape_applescript_string(default_value),
        ok_button = crate::utils::escape_applescript_string(ok_button),
    );

    let value = crate::platform::run_osascript(&script, "builtin_picker_prompt_for_text")
        .map_err(|error| error.to_string())?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

#[cfg(test)]
fn emoji_picker_label(emoji: &script_kit_gpui::emoji::Emoji) -> String {
    format!("{}  {}", emoji.emoji, emoji.name)
}

impl ScriptListApp {
    fn spawn_send_screen_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
        let capture_action = AiImageCaptureBuiltinAction::SendScreen;
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "SendScreenToAi",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling screen capture for AI"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let capture_result = cx
                .background_executor()
                .spawn(async { platform::capture_screen_screenshot() })
                .await;

            match capture_result {
                Ok((png_data, width, height)) => {
                    let size_bytes = png_data.len();
                    if size_bytes > crate::prompts::chat::MAX_IMAGE_BYTES {
                        tracing::warn!(
                            category = "AI",
                            event = "ai_capture_rejected",
                            source_action = "SendScreenToAi",
                            trace_id = %trace_id,
                            size_bytes,
                            max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                            "Rejecting screen capture larger than 10 MB"
                        );
                        this.update(cx, |this, cx| {
                            this.show_error_toast(
                                "Screen capture exceeds 10 MB limit".to_string(),
                                cx,
                            );
                        })
                        .ok();
                        return;
                    }

                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &png_data,
                    );
                    let message = format!(
                        "[Screenshot captured: {}x{} pixels]\n\nPlease analyze this screenshot.",
                        width, height
                    );

                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendScreenToAi",
                        trace_id = %trace_id,
                        width,
                        height,
                        size_bytes,
                        "Screen captured for AI"
                    );

                    this.update(cx, |this, cx| {
                        this.open_ai_window_after_already_hidden(
                            "SendScreenToAi",
                            &trace_id,
                            DeferredAiWindowAction::SetInputWithImage {
                                text: message,
                                image_base64: base64_data,
                                submit: false,
                            },
                            cx,
                        );
                    })
                    .ok();
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action = "SendScreenToAi",
                        trace_id = %trace_id,
                        error = %error,
                        "Failed to capture screen for AI"
                    );
                    let message = capture_action.failure_message(&error);
                    this.update(cx, |this, cx| {
                        this.show_error_toast(message, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn spawn_send_focused_window_to_ai_after_hide(
        &mut self,
        trace_id: &str,
        cx: &mut Context<Self>,
    ) {
        let capture_action = AiImageCaptureBuiltinAction::SendFocusedWindow;
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "SendFocusedWindowToAi",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling focused window capture for AI"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let capture_result = cx
                .background_executor()
                .spawn(async { platform::capture_focused_window_screenshot() })
                .await;

            match capture_result {
                Ok(capture) => {
                    let size_bytes = capture.png_data.len();
                    if size_bytes > crate::prompts::chat::MAX_IMAGE_BYTES {
                        tracing::warn!(
                            category = "AI",
                            event = "ai_capture_rejected",
                            source_action = "SendFocusedWindowToAi",
                            trace_id = %trace_id,
                            size_bytes,
                            max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                            "Rejecting window capture larger than 10 MB"
                        );
                        this.update(cx, |this, cx| {
                            this.show_error_toast(
                                "Window capture exceeds 10 MB limit".to_string(),
                                cx,
                            );
                        })
                        .ok();
                        return;
                    }

                    let fallback_warning = capture.used_fallback.then(|| {
                        format!(
                            "No focused window found — captured '{}'",
                            capture.window_title
                        )
                    });
                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &capture.png_data,
                    );
                    let message = format!(
                        "[Window: {} - {}x{} pixels]\n\nPlease analyze this window screenshot.",
                        capture.window_title, capture.width, capture.height
                    );

                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendFocusedWindowToAi",
                        trace_id = %trace_id,
                        window_title = %capture.window_title,
                        width = capture.width,
                        height = capture.height,
                        size_bytes,
                        used_fallback = capture.used_fallback,
                        "Focused window captured for AI"
                    );

                    this.update(cx, |this, cx| {
                        if let Some(warning_message) = fallback_warning {
                            this.toast_manager.push(
                                components::toast::Toast::warning(warning_message, &this.theme)
                                    .duration_ms(Some(TOAST_WARNING_MS)),
                            );
                            cx.notify();
                        }

                        this.open_ai_window_after_already_hidden(
                            "SendFocusedWindowToAi",
                            &trace_id,
                            DeferredAiWindowAction::SetInputWithImage {
                                text: message,
                                image_base64: base64_data,
                                submit: false,
                            },
                            cx,
                        );
                    })
                    .ok();
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action = "SendFocusedWindowToAi",
                        trace_id = %trace_id,
                        error = %error,
                        "Failed to capture focused window for AI"
                    );
                    let message = capture_action.failure_message(&error);
                    this.update(cx, |this, cx| {
                        this.show_error_toast(message, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn spawn_send_screen_area_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
        let capture_action = AiImageCaptureBuiltinAction::SendScreenArea;
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "SendScreenAreaToAi",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling screen area capture for AI"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let capture_result = cx
                .background_executor()
                .spawn(async { platform::capture_screen_area() })
                .await;

            match capture_result {
                Ok(Some(capture)) => {
                    let size_bytes = capture.png_data.len();
                    if size_bytes > crate::prompts::chat::MAX_IMAGE_BYTES {
                        tracing::warn!(
                            category = "AI",
                            event = "ai_capture_rejected",
                            source_action = "SendScreenAreaToAi",
                            trace_id = %trace_id,
                            size_bytes,
                            max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                            "Rejecting screen area capture larger than 10 MB"
                        );
                        this.update(cx, |this, cx| {
                            this.show_error_toast(
                                "Screen area capture exceeds 10 MB limit".to_string(),
                                cx,
                            );
                        })
                        .ok();
                        return;
                    }

                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &capture.png_data,
                    );
                    let message = format!(
                        "[Screen area captured: {}x{} pixels]\n\nPlease analyze this selected screen area.",
                        capture.width, capture.height
                    );

                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendScreenAreaToAi",
                        trace_id = %trace_id,
                        width = capture.width,
                        height = capture.height,
                        size_bytes,
                        "Screen area captured for AI"
                    );

                    this.update(cx, |this, cx| {
                        this.open_ai_window_after_already_hidden(
                            "SendScreenAreaToAi",
                            &trace_id,
                            DeferredAiWindowAction::SetInputWithImage {
                                text: message,
                                image_base64: base64_data,
                                submit: false,
                            },
                            cx,
                        );
                    })
                    .ok();
                }
                Ok(None) => {
                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_cancelled",
                        source_action = "SendScreenAreaToAi",
                        trace_id = %trace_id,
                        "Screen area selection cancelled by user"
                    );
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action = "SendScreenAreaToAi",
                        trace_id = %trace_id,
                        error = %error,
                        "Failed to capture screen area for AI"
                    );
                    let message = capture_action.failure_message(&error);
                    this.update(cx, |this, cx| {
                        this.show_error_toast(message, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_capture_text_to_ai_after_already_hidden<C, F>(
        &mut self,
        source_action: &'static str,
        trace_id: &str,
        capture_kind: &'static str,
        capture_fn: C,
        format_fn: F,
        cx: &mut Context<Self>,
    ) where
        C: FnOnce() -> Result<DeferredAiCapturedText, String> + Send + 'static,
        F: FnOnce(String) -> String + Send + 'static,
    {
        let capture_action = AiTextCaptureBuiltinAction::AgentChatContent;
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action,
            trace_id = %trace_id,
            capture_kind,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Scheduled deferred AI text capture"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let (result_tx, result_rx) =
                async_channel::bounded::<Result<DeferredAiCapturedText, String>>(1);

            let trace_id_for_thread = trace_id.clone();
            std::thread::spawn(move || {
                let started_at = std::time::Instant::now();
                let result = capture_fn();

                let (success, result_state) = match &result {
                    Ok(DeferredAiCapturedText::Ready(_)) => (true, "ready"),
                    Ok(DeferredAiCapturedText::Empty(_)) => (true, "empty"),
                    Err(_) => (false, "error"),
                };

                tracing::info!(
                    category = "AI",
                    event = "ai_capture_completed",
                    source_action,
                    trace_id = %trace_id_for_thread,
                    capture_kind,
                    result_state,
                    success,
                    duration_ms = started_at.elapsed().as_millis() as u64,
                    "Deferred AI text capture finished"
                );

                let _ = result_tx.send_blocking(result);
            });

            let Ok(result) = result_rx.recv().await else {
                return;
            };

            let _ = this.update(cx, |this, cx| match result {
                Ok(DeferredAiCapturedText::Ready(captured)) => {
                    this.open_ai_window_after_already_hidden(
                        source_action,
                        &trace_id,
                        DeferredAiWindowAction::SetInput {
                            text: format_fn(captured),
                            submit: false,
                        },
                        cx,
                    );
                }
                Ok(DeferredAiCapturedText::Empty(message)) => {
                    this.toast_manager.push(
                        components::toast::Toast::info(message, &this.theme)
                            .duration_ms(Some(TOAST_INFO_MS)),
                    );
                    cx.notify();
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action,
                        trace_id = %trace_id,
                        capture_kind,
                        error = %error,
                        "Deferred AI text capture failed"
                    );
                    let message = capture_action.failure_message(&error);
                    this.toast_manager.push(
                        components::toast::Toast::error(message, &this.theme)
                            .duration_ms(Some(TOAST_CRITICAL_MS)),
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn spawn_send_selected_text_to_ai_after_hide(
        &mut self,
        trace_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.spawn_capture_text_to_ai_after_already_hidden(
            "SendSelectedTextToAi",
            trace_id,
            "selected_text",
            || {
                crate::selected_text::get_selected_text()
                    .map_err(|error| error.to_string())
                    .map(|text| {
                        let trimmed = text.trim().to_string();
                        if trimmed.is_empty() {
                            DeferredAiCapturedText::Empty(
                                "No text selected. Select some text first.".to_string(),
                            )
                        } else {
                            DeferredAiCapturedText::Ready(trimmed)
                        }
                    })
            },
            |text| {
                format!(
                    "I've selected the following text:\n\n```\n{}\n```\n\nPlease help me with this.",
                    text
                )
            },
            cx,
        );
    }

    fn spawn_send_browser_tab_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
        self.spawn_capture_text_to_ai_after_already_hidden(
            "SendBrowserTabToAi",
            trace_id,
            "browser_url",
            || {
                platform::get_focused_browser_tab_url()
                    .map_err(|error| error.to_string())
                    .map(|url| {
                        let trimmed = url.trim().to_string();
                        if trimmed.is_empty() {
                            DeferredAiCapturedText::Empty(
                                "No browser URL found in the frontmost tab.".to_string(),
                            )
                        } else {
                            DeferredAiCapturedText::Ready(trimmed)
                        }
                    })
            },
            |url| {
                format!(
                    "I'm looking at this webpage:\n\n{}\n\nPlease help me analyze or understand its content.",
                    url
                )
            },
            cx,
        );
    }

    fn spawn_generate_script_from_current_app_after_hide(
        &mut self,
        trace_id: String,
        query_override: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let fallback_query = query_override.unwrap_or_else(|| self.filter_text.clone());

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "GenerateScriptFromCurrentApp",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling context capture for script generation"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let snapshot_result = cx
                .background_executor()
                .spawn(async { crate::menu_bar::load_frontmost_menu_snapshot() })
                .await;

            let selected_text = match crate::selected_text::get_selected_text() {
                Ok(text) if !text.trim().is_empty() => Some(text),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(
                        trace_id = %trace_id,
                        error = %error,
                        "ai_generate_script_from_current_app.selected_text_unavailable"
                    );
                    None
                }
            };

            let browser_url = match platform::get_focused_browser_tab_url() {
                Ok(url) if !url.trim().is_empty() => Some(url),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(
                        trace_id = %trace_id,
                        error = %error,
                        "ai_generate_script_from_current_app.browser_url_unavailable"
                    );
                    None
                }
            };

            // Build prompt outside entity borrow so we can show window safely.
            let prompt_or_error = match snapshot_result {
                Ok(snapshot) => {
                    let user_request =
                        crate::menu_bar::current_app_commands::normalize_generate_script_from_current_app_request(
                            Some(fallback_query.as_str()),
                        );

                    let (prompt, receipt) =
                        crate::menu_bar::current_app_commands::build_generate_script_prompt_from_snapshot(
                            snapshot,
                            user_request,
                            selected_text.as_deref(),
                            browser_url.as_deref(),
                        );

                    tracing::info!(
                        trace_id = %trace_id,
                        app_name = %receipt.app_name,
                        bundle_id = %receipt.bundle_id,
                        total_menu_items = receipt.total_menu_items,
                        included_menu_items = receipt.included_menu_items,
                        included_user_request = receipt.included_user_request,
                        included_selected_text = receipt.included_selected_text,
                        included_browser_url = receipt.included_browser_url,
                        "ai_generate_script_from_current_app.prompt_ready"
                    );

                    Ok(prompt)
                }
                Err(error) => Err(error),
            };

            match prompt_or_error {
                Ok(prompt) => {
                    // Platform calls — trigger macOS delegate callbacks.
                    // Safe here: no AppCell borrow is active.
                    script_kit_gpui::set_main_window_visible(true);
                    tracing::info!(
                        trace_id = %trace_id,
                        "ai_generate_script_from_current_app.showing_window"
                    );
                    crate::platform::show_main_window_without_activation();

                    // GPUI state changes inside entity borrow.
                    let _ = this.update(cx, |app, cx| {
                        app.dispatch_ai_script_generation_from_query(prompt, cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        let message =
                            AiTextCaptureBuiltinAction::CurrentAppContext.failure_message(&error);
                        app.show_error_toast(message.clone(), cx);
                        tracing::error!(
                            trace_id = %trace_id,
                            error = %error,
                            "ai_generate_script_from_current_app.capture_failed"
                        );
                    });
                }
            }
        })
        .detach();
    }

    /// Like `spawn_generate_script_from_current_app_after_hide`, but reuses an
    /// already-built recipe instead of recapturing live context after hide.
    ///
    /// This eliminates prompt drift: the prompt copied in the recipe is
    /// byte-for-byte the prompt sent to the AI generation path.
    fn spawn_generate_script_from_recipe_after_hide(
        &mut self,
        trace_id: String,
        recipe: crate::menu_bar::current_app_commands::CurrentAppCommandRecipe,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            category = "AI",
            event = "ai_recipe_generation_scheduled",
            source_action = "TurnThisIntoCommand",
            trace_id = %trace_id,
            recipe_prompt_bytes = recipe.prompt.len(),
            recipe_bundle_id = %recipe.prompt_receipt.bundle_id,
            recipe_included_selected_text = recipe.prompt_receipt.included_selected_text,
            recipe_included_browser_url = recipe.prompt_receipt.included_browser_url,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling recipe-based script generation (no recapture)"
        );

        platform::defer_hide_main_window(cx);

        let prompt =
            crate::menu_bar::current_app_commands::build_generated_script_prompt_from_recipe(
                &recipe,
            );

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            tracing::info!(
                trace_id = %trace_id,
                recipe_prompt_bytes = prompt.len(),
                recipe_bundle_id = %recipe.prompt_receipt.bundle_id,
                recipe_included_selected_text = recipe.prompt_receipt.included_selected_text,
                recipe_included_browser_url = recipe.prompt_receipt.included_browser_url,
                "ai_generate_script_from_recipe.prompt_ready"
            );

            // Platform calls — trigger macOS delegate callbacks.
            // Safe here: no AppCell borrow is active.
            script_kit_gpui::set_main_window_visible(true);
            tracing::info!(
                trace_id = %trace_id,
                "ai_generate_script_from_recipe.showing_window"
            );
            crate::platform::show_main_window_without_activation();

            // GPUI state changes inside entity borrow.
            let _ = this.update(cx, |app, cx| {
                app.dispatch_ai_script_generation_from_query(prompt, cx);
            });
        })
        .detach();
    }

    /// Schedule the DoInCurrentApp→GenerateScript flow, capturing selected
    /// text and the focused browser URL off the UI thread.
    ///
    /// Both `get_selected_text()` (AX-first, clipboard fallback) and
    /// `get_focused_browser_tab_url()` (single `osascript` call gated by the
    /// in-process frontmost-app tracker) can block for hundreds of
    /// milliseconds. Running them on `cx.background_executor()` keeps the
    /// launcher responsive while macOS answers; the memory lookup, recipe
    /// build, and dispatch run back on the main thread once capture completes.
    fn spawn_generate_script_from_current_app_with_capture(
        &mut self,
        trace_id: String,
        raw_query_owned: String,
        snapshot_for_recipe: crate::menu_bar::FrontmostMenuSnapshot,
        entries: Vec<crate::builtins::BuiltInEntry>,
        snapshot_receipt: crate::menu_bar::FrontmostMenuSnapshotReceipt,
        snapshot_pid: i32,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            trace_id = %trace_id,
            raw_query = %raw_query_owned,
            "do_in_current_app.spawn_context_capture"
        );

        cx.spawn(async move |this, cx| {
            let capture_started_at = std::time::Instant::now();
            let (selected_text, browser_url) = cx
                .background_executor()
                .spawn(async {
                    let selected_text = crate::selected_text::get_selected_text()
                        .ok()
                        .filter(|text| !text.trim().is_empty());
                    let browser_url = crate::platform::get_focused_browser_tab_url()
                        .ok()
                        .filter(|url| !url.trim().is_empty());
                    (selected_text, browser_url)
                })
                .await;

            tracing::info!(
                trace_id = %trace_id,
                capture_ms = capture_started_at.elapsed().as_millis() as u64,
                has_selected_text = selected_text.is_some(),
                has_browser_url = browser_url.is_some(),
                "do_in_current_app.context_capture_complete"
            );

            let _ = this.update(cx, |this, cx| {
                this.continue_generate_script_from_current_app_after_capture(
                    trace_id,
                    raw_query_owned,
                    snapshot_for_recipe,
                    entries,
                    snapshot_receipt,
                    snapshot_pid,
                    selected_text,
                    browser_url,
                    cx,
                );
            });
        })
        .detach();
    }

    /// Continuation of `spawn_generate_script_from_current_app_with_capture`,
    /// invoked back on the main thread once the blocking capture finishes.
    fn continue_generate_script_from_current_app_after_capture(
        &mut self,
        trace_id: String,
        raw_query_owned: String,
        snapshot_for_recipe: crate::menu_bar::FrontmostMenuSnapshot,
        entries: Vec<crate::builtins::BuiltInEntry>,
        snapshot_receipt: crate::menu_bar::FrontmostMenuSnapshotReceipt,
        snapshot_pid: i32,
        selected_text: Option<String>,
        browser_url: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let memory_decision = crate::ai::resolve_current_app_automation_from_memory(
            &raw_query_owned,
            &snapshot_for_recipe,
            &entries,
            selected_text.as_deref(),
            browser_url.as_deref(),
        );

        if let Ok(ref decision) = memory_decision {
            if let Some(ref replay) = decision.replay {
                tracing::info!(
                    category = "CURRENT_APP_AUTOMATION_MEMORY",
                    trace_id = %trace_id,
                    action = %decision.action,
                    best_score = decision.best_score,
                    matched_slug = decision
                        .matched
                        .as_ref()
                        .map(|entry| entry.slug.as_str())
                        .unwrap_or(""),
                    reason = %decision.reason,
                    "do_in_current_app.memory_resolved"
                );

                match decision.action.as_str() {
                    "replay_recipe" => match replay.action.as_str() {
                        "execute_entry" => {
                            if let Some(entry_index) = replay.selected_entry_index {
                                if entry_index < entries.len() {
                                    let entry = entries[entry_index].clone();
                                    let dctx = crate::action_helpers::DispatchContext {
                                        trace_id: trace_id.clone(),
                                        surface: crate::action_helpers::DispatchSurface::Builtin,
                                        action_id: entry.id.clone(),
                                    };
                                    let _ = self.execute_builtin_inner(
                                        &entry,
                                        Some(&raw_query_owned),
                                        &dctx,
                                        cx,
                                    );
                                    return;
                                }
                            }
                        }
                        "open_command_palette" => {
                            let filter = replay.verification.live_recipe.effective_query.clone();
                            self.present_current_app_commands_entries(
                                entries.clone(),
                                &snapshot_receipt,
                                snapshot_pid,
                                &filter,
                                cx,
                            );
                            return;
                        }
                        "generate_script" => {
                            self.spawn_generate_script_from_recipe_after_hide(
                                trace_id.clone(),
                                replay.verification.live_recipe.clone(),
                                cx,
                            );
                            return;
                        }
                        _ => {}
                    },
                    "repair_recipe" => {
                        self.spawn_generate_script_from_recipe_after_hide(
                            trace_id.clone(),
                            replay.verification.live_recipe.clone(),
                            cx,
                        );
                        return;
                    }
                    _ => {}
                }
            }
        }

        let recipe = crate::menu_bar::current_app_commands::build_current_app_command_recipe(
            snapshot_for_recipe,
            Some(&raw_query_owned),
            selected_text.as_deref(),
            browser_url.as_deref(),
        );

        match serde_json::to_string_pretty(&recipe) {
            Ok(json) => {
                tracing::info!(
                    category = "CURRENT_APP_RECIPE",
                    trace_id = %trace_id,
                    app_name = %recipe.prompt_receipt.app_name,
                    bundle_id = %recipe.prompt_receipt.bundle_id,
                    effective_query = %recipe.effective_query,
                    route = %recipe.trace.action,
                    suggested_script_name = %recipe.suggested_script_name,
                    included_selected_text = recipe.prompt_receipt.included_selected_text,
                    included_browser_url = recipe.prompt_receipt.included_browser_url,
                    json_bytes = json.len(),
                    "do_in_current_app.recipe_prepared"
                );
            }
            Err(error) => {
                tracing::warn!(
                    trace_id = %trace_id,
                    error = %error,
                    "do_in_current_app.recipe_serialize_failed"
                );
            }
        }

        self.spawn_generate_script_from_recipe_after_hide(trace_id, recipe, cx);
    }

    fn system_action_feedback_message(
        &self,
        action_type: &builtins::SystemActionType,
    ) -> Option<String> {
        let dark_mode_enabled = if matches!(action_type, builtins::SystemActionType::ToggleDarkMode)
        {
            system_actions::is_dark_mode().ok()
        } else {
            None
        };

        builtins::system_action_hud_message(*action_type, dark_mode_enabled)
    }

    /// Shared dispatch for system actions — used by both the normal and confirmed paths.
    /// Maps a `SystemActionType` to its implementation, handles special cases
    /// (QuitScriptKit), and routes the result through
    /// `handle_system_action_result`.
    /// Structured outcome logger for builtin execution paths.
    ///
    /// Emits a single log line with all fields needed for machine consumption
    /// and human debugging: builtin_id, trace_id, surface, handler, status,
    /// error_code, and duration_ms.
    fn log_builtin_outcome(
        builtin_id: &str,
        dctx: &crate::action_helpers::DispatchContext,
        handler: &str,
        outcome: &crate::action_helpers::DispatchOutcome,
        start: &std::time::Instant,
    ) {
        let duration_ms = start.elapsed().as_millis() as u64;
        let trace_id = outcome
            .trace_id
            .as_deref()
            .unwrap_or(dctx.trace_id.as_str());

        match outcome.status {
            crate::action_helpers::ActionOutcomeStatus::Error => {
                tracing::error!(
                    category = "BUILTIN",
                    builtin_id = %builtin_id,
                    trace_id = %trace_id,
                    surface = %dctx.surface,
                    handler,
                    status = %outcome.status,
                    error_code = outcome.error_code,
                    duration_ms,
                    detail = ?outcome.detail,
                    "Builtin execution finished"
                );
            }
            _ => {
                tracing::info!(
                    category = "BUILTIN",
                    builtin_id = %builtin_id,
                    trace_id = %trace_id,
                    surface = %dctx.surface,
                    handler,
                    status = %outcome.status,
                    error_code = outcome.error_code,
                    duration_ms,
                    detail = ?outcome.detail,
                    "Builtin execution finished"
                );
            }
        }
    }

    /// Build a success outcome carrying the dispatch context's trace_id.
    fn builtin_success(
        dctx: &crate::action_helpers::DispatchContext,
        detail: impl Into<String>,
    ) -> crate::action_helpers::DispatchOutcome {
        crate::action_helpers::DispatchOutcome::success()
            .with_trace_id(dctx.trace_id.clone())
            .with_detail(detail)
    }

    /// Build an error outcome carrying the dispatch context's trace_id.
    fn builtin_error(
        dctx: &crate::action_helpers::DispatchContext,
        code: &'static str,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> crate::action_helpers::DispatchOutcome {
        crate::action_helpers::DispatchOutcome::error(code, message)
            .with_trace_id(dctx.trace_id.clone())
            .with_detail(detail)
    }

    fn execute_menu_bar_builtin(
        &mut self,
        action_state: MenuBarBuiltinAction,
        action: &builtins::MenuBarActionInfo,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            bundle_id = %action.bundle_id,
            "Executing menu bar action"
        );
        #[cfg(target_os = "macos")]
        {
            match script_kit_gpui::menu_executor::execute_menu_action(
                &action.bundle_id,
                &action.menu_path,
            ) {
                Ok(()) => {
                    self.close_and_reset_window(cx);
                    Self::builtin_success(dctx, action_state.success_detail())
                }
                Err(e) => {
                    let message = action_state.failure_message(&e);
                    self.show_error_toast(message.clone(), cx);
                    Self::builtin_error(
                        dctx,
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        message,
                        action_state.failure_detail(),
                    )
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.show_unsupported_platform_toast("Menu bar actions", cx);
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_UNSUPPORTED_PLATFORM,
                "Menu bar actions only supported on macOS",
                action_state.unsupported_detail(),
            )
        }
    }

    fn execute_system_builtin(
        &mut self,
        action: SystemBuiltinAction,
        action_type: &builtins::SystemActionType,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            action_type = ?action_type,
            action = ?action,
            action_name = %action.handler_name(),
            "Executing system action via inner path"
        );
        self.dispatch_system_action(action_type, dctx, cx)
    }

    fn dispatch_system_action(
        &mut self,
        action_type: &builtins::SystemActionType,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let start = std::time::Instant::now();

        tracing::info!(
            category = "BUILTIN",
            builtin_id = %dctx.action_id,
            trace_id = %dctx.trace_id,
            surface = %dctx.surface,
            action_type = ?action_type,
            status = "dispatched",
            "system_action_dispatch"
        );

        #[cfg(target_os = "macos")]
        {
            use builtins::SystemActionType;

            let result = match action_type {
                // Power management
                SystemActionType::EmptyTrash => system_actions::empty_trash(),
                SystemActionType::LockScreen => system_actions::lock_screen(),
                SystemActionType::Sleep => system_actions::sleep(),
                SystemActionType::Restart => system_actions::restart(),
                SystemActionType::ShutDown => system_actions::shut_down(),
                SystemActionType::LogOut => system_actions::log_out(),

                // UI controls
                SystemActionType::ToggleDarkMode => system_actions::toggle_dark_mode(),
                SystemActionType::ShowDesktop => system_actions::show_desktop(),
                SystemActionType::MissionControl => system_actions::mission_control(),
                SystemActionType::Launchpad => system_actions::launchpad(),
                SystemActionType::ForceQuitApps => system_actions::force_quit_apps(),

                // Volume controls (preset levels)
                SystemActionType::Volume0 => system_actions::set_volume(0),
                SystemActionType::Volume25 => system_actions::set_volume(25),
                SystemActionType::Volume50 => system_actions::set_volume(50),
                SystemActionType::Volume75 => system_actions::set_volume(75),
                SystemActionType::Volume100 => system_actions::set_volume(100),
                SystemActionType::VolumeMute => system_actions::volume_mute(),

                // App control
                SystemActionType::QuitScriptKit => {
                    Self::prepare_script_kit_shutdown();
                    cx.quit();
                    return Self::builtin_success(dctx, "quit_script_kit");
                }

                // System utilities
                SystemActionType::ToggleDoNotDisturb => system_actions::toggle_do_not_disturb(),
                SystemActionType::StartScreenSaver => system_actions::start_screen_saver(),

                // System Preferences
                SystemActionType::OpenSystemPreferences => {
                    system_actions::open_system_preferences_main()
                }
                SystemActionType::OpenPrivacySettings => system_actions::open_privacy_settings(),
                SystemActionType::OpenDisplaySettings => system_actions::open_display_settings(),
                SystemActionType::OpenSoundSettings => system_actions::open_sound_settings(),
                SystemActionType::OpenNetworkSettings => system_actions::open_network_settings(),
                SystemActionType::OpenKeyboardSettings => system_actions::open_keyboard_settings(),
                SystemActionType::OpenBluetoothSettings => {
                    system_actions::open_bluetooth_settings()
                }
                SystemActionType::OpenNotificationsSettings => {
                    system_actions::open_notifications_settings()
                }
            };

            self.handle_system_action_result(result, action_type, dctx, start.elapsed(), cx)
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = action_type;
            self.show_unsupported_platform_toast("System actions", cx);
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_UNSUPPORTED_PLATFORM,
                "System actions are not supported on this platform",
                "system_action_unsupported_platform",
            )
        }
    }

    /// Shared result handler for system actions — shows HUD on success, Toast on error.
    /// Returns a `DispatchOutcome` for structured logging at the call boundary.
    fn handle_system_action_result(
        &mut self,
        result: Result<(), String>,
        action_type: &builtins::SystemActionType,
        dctx: &crate::action_helpers::DispatchContext,
        elapsed: std::time::Duration,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let duration_ms = elapsed.as_millis() as u64;
        match result {
            Ok(()) => {
                tracing::info!(
                    category = "BUILTIN",
                    builtin_id = %dctx.action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    action_type = ?action_type,
                    status = "success",
                    duration_ms,
                    "system_action_dispatch"
                );
                if let Some(message) = self.system_action_feedback_message(action_type) {
                    cx.notify();
                    self.show_hud(message, Some(HUD_MEDIUM_MS), cx);
                    self.hide_main_and_reset(cx);
                } else {
                    self.close_and_reset_window(cx);
                }
                Self::builtin_success(dctx, format!("system_action::{action_type:?}"))
            }
            Err(error) => {
                tracing::error!(
                    category = "BUILTIN",
                    builtin_id = %dctx.action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    action_type = ?action_type,
                    status = "error",
                    error_code = crate::action_helpers::ERROR_LAUNCH_FAILED,
                    duration_ms,
                    error = %error,
                    "system_action_dispatch"
                );
                self.show_error_toast(format!("System action failed: {}", error), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_LAUNCH_FAILED,
                    format!("System action failed: {}", error),
                    format!("system_action::{action_type:?}; error={error}"),
                )
            }
        }
    }

    fn prepare_script_kit_shutdown() {
        tracing::info!(
            category = "UI",
            event = "prepare_script_kit_shutdown",
            "prepare_script_kit_shutdown"
        );
        PROCESS_MANAGER.kill_all_processes();
        PROCESS_MANAGER.remove_main_pid();
    }

    fn quit_script_kit_confirm_options() -> crate::confirm::ParentConfirmOptions {
        crate::confirm::ParentConfirmOptions::destructive(
            "Quit Script Kit",
            "Quit Script Kit and stop all running processes?",
            "Quit",
        )
    }

    fn builtin_confirmation_options(
        entry_id: &str,
        entry_name: &str,
    ) -> crate::confirm::ParentConfirmOptions {
        match entry_id {
            "builtin/quit-script-kit" => Self::quit_script_kit_confirm_options(),
            "builtin/shut-down" => crate::confirm::ParentConfirmOptions::destructive(
                "Shut Down Mac",
                "Shut down this Mac now?",
                "Shut Down",
            ),
            "builtin/restart" => crate::confirm::ParentConfirmOptions::destructive(
                "Restart Mac",
                "Restart this Mac now?",
                "Restart",
            ),
            "builtin/log-out" => crate::confirm::ParentConfirmOptions::destructive(
                "Log Out",
                "Log out of the current macOS session?",
                "Log Out",
            ),
            "builtin/empty-trash" => crate::confirm::ParentConfirmOptions::destructive(
                "Empty Trash",
                "Empty Trash now? This cannot be undone.",
                "Empty Trash",
            ),
            "builtin/sleep" => crate::confirm::ParentConfirmOptions {
                title: "Sleep Mac".into(),
                body: "Put this Mac to sleep now?".into(),
                confirm_text: "Sleep".into(),
                cancel_text: "Cancel".into(),
                ..Default::default()
            },
            "builtin/force-quit" => crate::confirm::ParentConfirmOptions::destructive(
                "Open Force Quit Apps",
                "Open Force Quit Apps?",
                "Open",
            ),
            "builtin/stop-all-processes" => crate::confirm::ParentConfirmOptions::destructive(
                "Stop All Processes",
                "Stop all running Script Kit processes?",
                "Stop All",
            ),
            "builtin/clear-suggested" => crate::confirm::ParentConfirmOptions::destructive(
                "Clear Suggested",
                "Clear suggested items and reset their ranking data?",
                "Clear Suggested",
            ),
            "builtin/sync-to-github" => crate::confirm::ParentConfirmOptions {
                title: "Sync to GitHub".into(),
                body: "Write safe .gitignore exclusions, commit Script Kit changes, and sync this workspace to GitHub?".into(),
                confirm_text: "Sync".into(),
                cancel_text: "Cancel".into(),
                ..Default::default()
            },
            _ => crate::confirm::ParentConfirmOptions {
                title: "Confirm".into(),
                body: format!("Are you sure you want to {}?", entry_name).into(),
                confirm_text: "Continue".into(),
                cancel_text: "Cancel".into(),
                ..Default::default()
            },
        }
    }

    pub(crate) fn execute_builtin(
        &mut self,
        entry: &builtins::BuiltInEntry,
        cx: &mut Context<Self>,
    ) {
        self.execute_builtin_with_query(entry, None, cx);
    }

    fn execute_builtin_with_query(
        &mut self,
        entry: &builtins::BuiltInEntry,
        query_override: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let start = std::time::Instant::now();
        let dctx = crate::action_helpers::DispatchContext::for_builtin(&entry.id);

        tracing::info!(
            category = "BUILTIN",
            builtin_id = %entry.id,
            builtin_name = %entry.name,
            trace_id = %dctx.trace_id,
            surface = %dctx.surface,
            "Builtin execution started"
        );

        // Clear any stale actions popup from previous view
        self.clear_actions_popup_state();

        // Check if this command requires confirmation - open modal if so.
        // Quit Script Kit goes through this same path (see builtin_confirmation_options
        // and DEFAULT_CONFIRMATION_COMMANDS) — the parent confirm popup is a separate
        // native window with its own focus, so Tab/Esc work without competing with the
        // launcher or any other view's key handlers.
        if self.config.requires_confirmation(&entry.id) {
            let confirmation_start = std::time::Instant::now();
            let entry_id = entry.id.clone();
            let query_owned = query_override.map(|s| s.to_string());
            let dctx_owned = dctx.clone();
            let confirm_options = Self::builtin_confirmation_options(&entry.id, &entry.name);

            // Spawn a task to show confirmation dialog via shared parent dialog helper
            cx.spawn(async move |this, cx| {
                match crate::confirm::confirm_with_parent_dialog(
                    cx,
                    confirm_options,
                    &dctx_owned.trace_id,
                )
                .await
                {
                    Ok(true) => {
                        let _ = this.update(cx, |this, cx| {
                            this.handle_builtin_confirmation(
                                entry_id,
                                true,
                                query_owned,
                                &dctx_owned,
                                cx,
                            );
                        });
                    }
                    Ok(false) => {
                        let outcome = crate::action_helpers::DispatchOutcome::cancelled()
                            .with_trace_id(dctx_owned.trace_id.clone())
                            .with_detail("builtin_confirmation_cancelled");
                        let _ = this.update(cx, |this, _| {
                            this.clear_menu_origin_after_script_list_confirm_cancel(
                                "builtin_confirmation_cancelled",
                            );
                            Self::log_builtin_outcome(
                                &entry_id,
                                &dctx_owned,
                                "confirmation_gate",
                                &outcome,
                                &confirmation_start,
                            );
                        });
                    }
                    Err(e) => {
                        let _ = this.update(cx, |this, cx| {
                            tracing::error!(
                                builtin_id = %entry_id,
                                trace_id = %dctx_owned.trace_id,
                                error = %e,
                                "failed to open confirmation modal"
                            );
                            this.show_error_toast_with_code(
                                "Failed to open confirmation dialog",
                                Some(crate::action_helpers::ERROR_MODAL_FAILED),
                                cx,
                            );
                            let outcome = Self::builtin_error(
                                &dctx_owned,
                                crate::action_helpers::ERROR_MODAL_FAILED,
                                "Failed to open confirmation dialog",
                                format!("confirmation_modal_error={e}"),
                            );
                            Self::log_builtin_outcome(
                                &entry_id,
                                &dctx_owned,
                                "confirmation_gate",
                                &outcome,
                                &confirmation_start,
                            );
                        });
                    }
                }
            })
            .detach();

            tracing::info!(
                category = "BUILTIN",
                trace_id = %dctx.trace_id,
                builtin_id = %entry.id,
                status = "awaiting_confirmation",
                duration_ms = start.elapsed().as_millis() as u64,
                "Builtin execution deferred to confirmation modal"
            );
            return; // Wait for modal callback
        }

        // All builtins now return DispatchOutcome — system actions are handled
        // inside execute_builtin_inner as well.
        let outcome = self.execute_builtin_inner(entry, query_override, &dctx, cx);

        Self::log_builtin_outcome(&entry.id, &dctx, "builtin_execution", &outcome, &start);
    }

    /// Open a filterable main-window builtin view with a consistent UX contract.
    ///
    /// Every filterable builtin should go through this helper so that focus,
    /// placeholder, filter reset, hover clearing, resize, and opened-from-menu
    /// state are always set the same way.
    ///
    /// `expanded` picks the window sizing contract: `false` matches the main
    /// window's 480px height for light-weight pickers like
    /// emoji / apps / browser tabs / window switcher; `true` uses the wide
    /// full-width window for info-heavy views like clipboard history while
    /// preserving the mini window height so the shared input does not shift.
    fn open_builtin_filterable_view(
        &mut self,
        view: AppView,
        placeholder: &str,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(placeholder.to_string());
        self.builtin_row_stack_scroll_handle.scroll_to_item(0);
        self.current_view = view;
        self.hovered_index = None;
        self.opened_from_main_menu = true;
        if expanded {
            self.set_main_window_mode_state_only(
                MainWindowMode::Full,
                cx,
                "open_builtin_filterable_view",
            );
            resize_to_view_sync(ViewType::MainWindow, 0);
        } else {
            self.set_main_window_mode_state_only(
                MainWindowMode::Mini,
                cx,
                "open_builtin_filterable_view",
            );
            resize_to_view_sync(ViewType::MainWindow, 0);
        }
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    fn open_theme_chooser_view(&mut self, cx: &mut Context<Self>) {
        self.theme_before_chooser = Some(self.theme.clone());
        self.theme_chooser_management = None;
        self.theme_chooser_panel_mode = ThemeChooserPanelMode::default();
        // Use the unified catalog (user themes + built-in presets) so the
        // initial selection, list sizing, and scroll target all agree with
        // what the gallery actually renders.
        let catalog = Self::theme_chooser_catalog();
        let start_index = Self::theme_chooser_catalog_index_for_theme(&catalog, &self.theme);

        self.open_builtin_filterable_view(
            AppView::ThemeChooserView {
                filter: String::new(),
                selected_index: start_index,
            },
            "Search themes or paste #hex...",
            true,
            cx,
        );

        let item_count = catalog.len();
        let old_count = self.theme_chooser_list_state.item_count();
        if old_count != item_count {
            self.theme_chooser_list_state
                .splice(0..old_count, item_count);
        }
        self.theme_chooser_list_state
            .scroll_to_reveal_item(start_index);
    }

    fn open_main_window(&mut self, cx: &mut Context<Self>) {
        self.filter_text.clear();
        self.computed_filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search scripts, apps, and commands…".to_string());
        self.show_script_list_with_main_filter_focus();
        self.set_main_window_mode_state_only(MainWindowMode::Mini, cx, "open_main_window");
        self.hovered_index = None;
        self.selected_index = 0;
        self.opened_from_main_menu = true;
        self.invalidate_grouped_cache();
        self.sync_list_state();
        let (grouped_items, _) = self.get_grouped_results_cached();
        let item_count = grouped_items.len();
        // Skip section headers — select first actual item so cmd+k works immediately
        let first_selectable =
            crate::list_item::GroupedListState::from_items(&grouped_items).first_selectable;
        self.selected_index = first_selectable;
        tracing::info!(
            event = "open_main_window",
            item_count = item_count,
            selected_index = self.selected_index,
            first_selectable = first_selectable,
            grouped_cache_key = %self.main_menu_result_caches.grouped_cache_key(),
            computed_filter = %self.computed_filter_text,
            filter_text = %self.filter_text,
            pending_filter_sync = self.pending_filter_sync,
            "open_main_window: items={}, selected={}",
            item_count,
            self.selected_index,
        );
        resize_to_view_sync(ViewType::MainWindow, item_count);
        cx.notify();
    }

    fn open_ai_vault_source_filter(&mut self, cx: &mut Context<Self>) {
        let filter_text = "vault: ".to_string();
        self.cancel_history_filter_render_pending_if_obsolete(&filter_text);
        self.filter_text = filter_text.clone();
        self.computed_filter_text = filter_text.clone();
        self.pending_programmatic_filter_echo = Some(filter_text.clone());
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search AI Vault sessions...".to_string());
        self.set_menu_syntax_mode_from_filter(&filter_text);
        self.show_script_list_with_main_filter_focus();
        self.hovered_index = None;
        self.selected_index = 0;
        self.opened_from_main_menu = true;
        self.main_menu_fallback_state.clear();
        self.invalidate_grouped_cache();
        self.filter_coalescer.reset();
        self.maybe_start_root_file_search(&filter_text, cx);
        self.reconcile_script_list_after_filter_change("open_ai_vault_source_filter", cx);
        let (grouped_items, _) = self.get_grouped_results_cached();
        let item_count = grouped_items.len();
        self.set_main_window_mode_state_only(
            MainWindowMode::Mini,
            cx,
            "open_ai_vault_source_filter",
        );
        resize_to_view_sync(ViewType::MainWindow, item_count);
        cx.notify();
    }

    /// Open a filterable builtin view with an initial filter value.
    ///
    /// Same UX contract as [`open_builtin_filterable_view`] but pre-fills the
    /// filter input instead of clearing it. Used by `DoInCurrentApp` to open
    /// the command palette with the user's query already typed.
    ///
    /// See `open_builtin_filterable_view` for the meaning of `expanded`.
    fn open_builtin_filterable_view_with_filter(
        &mut self,
        view: AppView,
        filter: &str,
        placeholder: &str,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            view = ?view,
            filter = %filter,
            placeholder = %placeholder,
            expanded,
            "open_builtin_filterable_view_with_filter — setting current_view and filter"
        );
        self.filter_text = filter.to_string();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(placeholder.to_string());
        self.current_view = view;
        self.hovered_index = None;
        self.opened_from_main_menu = true;
        if expanded {
            self.set_main_window_mode_state_only(
                MainWindowMode::Full,
                cx,
                "open_builtin_filterable_view_with_filter",
            );
            resize_to_view_sync(ViewType::MainWindow, 0);
        } else {
            self.set_main_window_mode_state_only(
                MainWindowMode::Mini,
                cx,
                "open_builtin_filterable_view_with_filter",
            );
            resize_to_view_sync(ViewType::MainWindow, 0);
        }
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    pub(crate) fn open_clipboard_history_surface_with_filter(
        &mut self,
        filter: String,
        cx: &mut Context<Self>,
    ) {
        self.cached_clipboard_entries = clipboard_history::get_cached_entries(100);
        self.focused_clipboard_entry_id = self
            .cached_clipboard_entries
            .first()
            .map(|entry| entry.id.clone());

        self.open_builtin_filterable_view_with_filter(
            AppView::ClipboardHistoryView {
                filter: filter.clone(),
                selected_index: 0,
            },
            &filter,
            "Search clipboard history...",
            true,
            cx,
        );
    }

    pub(crate) fn present_current_app_commands_entries(
        &mut self,
        entries: Vec<crate::builtins::BuiltInEntry>,
        receipt: &crate::menu_bar::FrontmostMenuSnapshotReceipt,
        pid: i32,
        filter: &str,
        cx: &mut Context<Self>,
    ) {
        let session = crate::menu_bar::CurrentAppCommandsSession::from_entries_and_receipt(
            entries,
            receipt.clone(),
            pid,
        );
        self.present_current_app_commands_session(session, filter, cx);
    }

    pub(crate) fn present_current_app_commands_session(
        &mut self,
        session: crate::menu_bar::CurrentAppCommandsSession,
        filter: &str,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            pid = session.pid,
            app_name = %session.app_name,
            bundle_id = %session.bundle_id,
            top_level_menu_count = session.top_level_menu_count,
            leaf_entry_count = session.leaf_entry_count,
            placeholder = %session.placeholder,
            source = session.source,
            filter = %filter,
            "current_app_commands.present_session"
        );

        self.current_app_commands_session = Some(session.clone());
        self.cached_current_app_entries = session.entries.clone();
        self.open_builtin_filterable_view_with_filter(
            AppView::CurrentAppCommandsView {
                filter: filter.to_string(),
                selected_index: 0,
            },
            filter,
            &session.placeholder,
            false,
            cx,
        );
        self.current_app_commands_scroll_handle
            .scroll_to_item(0, gpui::ScrollStrategy::Top);

        if self.cached_current_app_entries.is_empty() {
            tracing::info!(
                pid = session.pid,
                app_name = %session.app_name,
                bundle_id = %session.bundle_id,
                "current_app_commands.present_empty_state"
            );
        }
    }

    pub(crate) fn open_current_app_commands_from_tray(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let session = crate::menu_bar::capture_current_app_commands_session()
            .map_err(|e| anyhow::anyhow!("Failed to load frontmost app menu bar: {e}"))?;
        self.present_current_app_commands_session(session, "", cx);
        Ok(())
    }

    pub(crate) fn refresh_current_app_commands_session_if_needed(
        &mut self,
        filter: &str,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<bool> {
        let Some(existing) = self.current_app_commands_session.clone() else {
            return Ok(false);
        };

        let live_identity = crate::menu_bar::load_live_current_app_commands_identity();
        if !crate::menu_bar::current_app_commands_session_identity_changed(
            &existing,
            live_identity.as_ref(),
        ) {
            return Ok(false);
        }

        let refreshed = match crate::menu_bar::capture_current_app_commands_session() {
            Ok(session) => session,
            Err(error) => {
                self.invalidate_current_app_commands_session(filter, cx);
                return Err(error);
            }
        };
        let previous_app_name = existing.app_name.clone();
        let refreshed_app_name = refreshed.app_name.clone();
        tracing::info!(
            previous_pid = existing.pid,
            new_pid = refreshed.pid,
            previous_bundle_id = %existing.bundle_id,
            new_bundle_id = %refreshed.bundle_id,
            previous_app_name = %previous_app_name,
            new_app_name = %refreshed_app_name,
            filter = %filter,
            "current_app_commands.session_switched"
        );
        self.present_current_app_commands_session(refreshed, filter, cx);
        self.show_hud(
            format!("Current app changed: {previous_app_name} -> {refreshed_app_name}"),
            Some(HUD_SHORT_MS),
            cx,
        );
        tracing::info!(
            previous_app_name = %previous_app_name,
            new_app_name = %refreshed_app_name,
            filter = %filter,
            "current_app_commands.session_switch_hud_shown"
        );
        Ok(true)
    }

    pub(crate) fn invalidate_current_app_commands_session(
        &mut self,
        filter: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(existing) = &self.current_app_commands_session {
            tracing::warn!(
                previous_pid = existing.pid,
                previous_bundle_id = %existing.bundle_id,
                previous_app_name = %existing.app_name,
                filter = %filter,
                "current_app_commands.session_invalidated"
            );
        }

        self.current_app_commands_session = None;
        self.cached_current_app_entries.clear();
        self.filter_text = filter.to_string();
        self.pending_placeholder = Some("Search current app commands…".to_string());

        if let AppView::CurrentAppCommandsView {
            filter: current_filter,
            selected_index,
        } = &mut self.current_view
        {
            *current_filter = filter.to_string();
            *selected_index = 0;
        }

        cx.notify();
    }

    pub(crate) fn execute_selected_current_app_command(
        &mut self,
        original_entry_index: usize,
        cx: &mut Context<Self>,
    ) {
        let filter = match &self.current_view {
            AppView::CurrentAppCommandsView { filter, .. } => filter.clone(),
            _ => String::new(),
        };

        match self.refresh_current_app_commands_session_if_needed(&filter, cx) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    entry_index = original_entry_index,
                    "current_app_commands.refresh_failed_before_execute"
                );
                self.show_error_toast(
                    UtilityCurrentAppCommandsBuiltinAction::Open.refresh_failure_message(&error),
                    cx,
                );
                return;
            }
        }

        let Some(entry) = self
            .cached_current_app_entries
            .get(original_entry_index)
            .cloned()
        else {
            tracing::warn!(
                entry_index = original_entry_index,
                cached_entry_count = self.cached_current_app_entries.len(),
                "current_app_commands.execute_selected_missing_index"
            );
            return;
        };

        tracing::info!(
            entry_id = %entry.id,
            entry_name = %entry.name,
            entry_index = original_entry_index,
            "current_app_commands.execute_selected_resolved"
        );
        self.execute_builtin(&entry, cx);
    }

    /// Inner builtin executor — runs the actual action logic.
    /// Called from both the normal path (after confirmation check) and the
    /// confirmed path (after modal approval), ensuring a single implementation.
    ///
    /// Returns a `DispatchOutcome` so callers can log the real result instead
    /// of a synthetic success.
    fn execute_builtin_inner(
        &mut self,
        entry: &builtins::BuiltInEntry,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match &entry.feature {
            builtins::BuiltInFeature::ClipboardHistory => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive ClipboardHistory");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::PasteSequentially => {
                self.execute_paste_sequential_builtin(dctx, cx)
            }
            builtins::BuiltInFeature::Favorites => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive Favorites");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::AppLauncher => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive AppLauncher");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::App(app_name) => {
                let launch_action = AppLaunchBuiltinAction::from_feature(&entry.feature)
                    .expect("app launch arm should only receive App");
                self.execute_app_launch_builtin(launch_action, app_name, dctx, cx)
            }
            builtins::BuiltInFeature::WindowSwitcher => {
                let window_switcher_action =
                    WindowSwitcherBuiltinAction::from_feature(&entry.feature)
                        .expect("window switcher arm should only receive WindowSwitcher");
                self.execute_window_switcher_builtin(window_switcher_action, dctx, cx)
            }
            builtins::BuiltInFeature::BrowserTabs => {
                let browser_tabs_action = BrowserTabsBuiltinAction::from_feature(&entry.feature)
                    .expect("browser tabs arm should only receive BrowserTabs");
                self.execute_browser_tabs_builtin(browser_tabs_action, dctx, cx)
            }
            builtins::BuiltInFeature::DesignGallery => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive DesignGallery");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::FooterGallery => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive FooterGallery");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::DesignNonListStates => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive DesignNonListStates");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            #[cfg(feature = "storybook")]
            builtins::BuiltInFeature::DesignExplorer => {
                let design_action = DesignExplorerBuiltinAction::from_feature(&entry.feature)
                    .expect("design explorer arm should only receive design explorer features");
                self.execute_design_explorer_builtin(design_action, dctx, cx)
            }
            builtins::BuiltInFeature::AiChat | builtins::BuiltInFeature::AiChatVariant(_) => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive AiChat");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::Notes => {
                let notes_action = NotesBuiltinAction::from_feature(&entry.feature)
                    .expect("notes arm should only receive Notes");
                self.execute_notes_builtin(notes_action, dctx, cx)
            }
            builtins::BuiltInFeature::EmojiPicker => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive EmojiPicker");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::SyncToGithub => {
                let sync_action = SyncToGithubBuiltinAction::from_feature(&entry.feature)
                    .expect("sync-to-github arm should only receive SyncToGithub");
                self.execute_sync_to_github_builtin(sync_action, dctx, cx)
            }
            builtins::BuiltInFeature::MenuBarAction(action) => {
                let menu_action = MenuBarBuiltinAction::from_action(action);
                self.execute_menu_bar_builtin(menu_action, action, dctx, cx)
            }

            // =========================================================================
            // System Actions
            // =========================================================================
            builtins::BuiltInFeature::SystemAction(action_type) => {
                let system_action = SystemBuiltinAction::from_action(action_type);
                self.execute_system_builtin(system_action, action_type, dctx, cx)
            }

            // =========================================================================
            // Background Effects
            // =========================================================================
            builtins::BuiltInFeature::BackgroundEffectNext => {
                let next = self
                    .background_effect
                    .map(|effect| effect.next())
                    .unwrap_or(crate::effects::BackgroundEffect::all()[0]);
                self.set_background_effect(Some(next), cx);
                self.toast_manager.push(
                    components::toast::Toast::success(
                        format!("Background Effect: {}", next.name()),
                        &self.theme,
                    )
                    .duration_ms(Some(1500)),
                );
                Self::builtin_success(dctx, format!("background_effect::{}", next.slug()))
            }
            builtins::BuiltInFeature::BackgroundEffectPrevious => {
                let prev = self
                    .background_effect
                    .map(|effect| effect.prev())
                    .unwrap_or_else(|| *crate::effects::BackgroundEffect::all().last().unwrap());
                self.set_background_effect(Some(prev), cx);
                self.toast_manager.push(
                    components::toast::Toast::success(
                        format!("Background Effect: {}", prev.name()),
                        &self.theme,
                    )
                    .duration_ms(Some(1500)),
                );
                Self::builtin_success(dctx, format!("background_effect::{}", prev.slug()))
            }
            builtins::BuiltInFeature::BackgroundEffectOff => {
                self.set_background_effect(None, cx);
                self.toast_manager.push(
                    components::toast::Toast::info("Background Effect: Off", &self.theme)
                        .duration_ms(Some(1500)),
                );
                Self::builtin_success(dctx, "background_effect::off")
            }

            // =========================================================================
            // Flow UX exploration surfaces (docs/ai/flow-ux-protocol.md)
            // =========================================================================
            builtins::BuiltInFeature::FlowUxVariant(variant) => {
                let variant = *variant;
                if variant == crate::flows::model::FlowUxVariant::MissionControl {
                    // Mission Control's home is the detached manager window;
                    // it never occupies the main-window view stack.
                    if let Err(err) = crate::flows::manager_window::open_flow_manager_window(cx) {
                        self.show_error_toast(format!("Flow Manager failed: {err}"), cx);
                        return Self::builtin_error(
                            dctx,
                            "flow_manager_open_failed",
                            err.to_string(),
                            "flow_ux::mission_control_open",
                        );
                    }
                    return Self::builtin_success(dctx, "flow_ux::mission_control");
                }
                // Prewarm the roster for the effective cwd before the view
                // paints, so the first frame already has flows.
                let cwd = self.flow_ux_cwd();
                crate::flows::catalog::flow_catalog().refresh(&cwd);
                self.open_builtin_filterable_view(
                    AppView::FlowUxView {
                        variant,
                        filter: String::new(),
                        selected_index: 0,
                        inline_run: None,
                    },
                    "Search flows...",
                    false,
                    cx,
                );
                self.start_flow_ux_tick(cx);
                Self::builtin_success(
                    dctx,
                    format!("flow_ux::{}", variant.automation_id()),
                )
            }
            builtins::BuiltInFeature::FlowManager => {
                if let Err(err) = crate::flows::manager_window::open_flow_manager_window(cx) {
                    self.show_error_toast(format!("Flow Manager failed: {err}"), cx);
                    return Self::builtin_error(
                        dctx,
                        "flow_manager_open_failed",
                        err.to_string(),
                        "flow_manager::open",
                    );
                }
                Self::builtin_success(dctx, "flow_manager::open")
            }

            // NOTE: Window Actions removed - now handled by window-management extension
            // SDK tileWindow() still works via protocol messages in execute_script.rs

            // =========================================================================
            // Notes Commands
            // =========================================================================
            builtins::BuiltInFeature::NotesCommand(cmd_type) => {
                tracing::info!(
                    target: "script_kit::keyboard",
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    notes_command = ?cmd_type,
                    event = "notes_command_handoff_preserving_launcher_context",
                    filter_text_len = self.filter_text.len(),
                    show_actions_popup = self.show_actions_popup,
                    "Executing notes command (preserving launcher context)"
                );

                let notes_action = NotesCommandBuiltinAction::from_command(*cmd_type);
                self.execute_notes_command_builtin(notes_action, dctx, cx)
            }

            // =========================================================================
            // AI Commands
            // =========================================================================
            builtins::BuiltInFeature::AiCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    ai_command = ?cmd_type,
                    "Executing AI command"
                );

                let window_plan = AiCommandWindowPlan::from_command(cmd_type);
                self.apply_ai_command_window_plan(window_plan, cmd_type, cx);

                let ai_action = AiCommandBuiltinAction::from_command(*cmd_type);
                self.execute_ai_command_builtin(ai_action, query_override, dctx, cx)
            }

            // =========================================================================
            // Script Commands
            // =========================================================================
            builtins::BuiltInFeature::ScriptCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    script_command = ?cmd_type,
                    "Executing script command"
                );

                let script_action = ScriptCommandBuiltinAction::from_command(*cmd_type);
                self.execute_script_command_builtin(script_action, dctx, cx)
            }

            // =========================================================================
            // Permission Commands
            // =========================================================================
            builtins::BuiltInFeature::PermissionCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    permission_command = ?cmd_type,
                    "Executing permission command"
                );

                let permission_action = PermissionCommandBuiltinAction::from_command(*cmd_type);
                self.execute_permission_command_builtin(permission_action, dctx, cx)
            }

            // =========================================================================
            // Frecency/Suggested Commands
            // =========================================================================
            builtins::BuiltInFeature::FrecencyCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    frecency_command = ?cmd_type,
                    "Executing frecency command"
                );

                let frecency_action = FrecencyCommandBuiltinAction::from_command(*cmd_type);
                self.execute_frecency_command_builtin(frecency_action, dctx, cx)
            }

            // =========================================================================
            // Settings Commands (Reset Window Positions, etc.)
            // =========================================================================
            builtins::BuiltInFeature::SettingsCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    settings_command = ?cmd_type,
                    "Executing settings command"
                );

                let settings_action = SettingsCommandBuiltinAction::from_command(*cmd_type);
                self.execute_settings_command_builtin(settings_action, dctx, cx)
            }

            // =========================================================================
            // Utility Commands (Scratch Pad, Quick Terminal, Claude Code Harness, Process Manager)
            // =========================================================================
            builtins::BuiltInFeature::UtilityCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    utility_command = ?cmd_type,
                    "Executing utility command"
                );

                let utility_action = UtilityCommandBuiltinAction::from_command(*cmd_type);
                self.execute_utility_command_builtin(utility_action, query_override, dctx, cx)
            }

            // =========================================================================
            // File Search (Directory Navigation)
            // =========================================================================
            builtins::BuiltInFeature::Webcam => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive Webcam");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::Dictation => self.execute_dictation_builtin_action(
                DictationBuiltinAction::CurrentSurface,
                dctx,
                cx,
            ),
            builtins::BuiltInFeature::DictationToAiHarness => {
                self.execute_dictation_builtin_action(DictationBuiltinAction::AgentChat, dctx, cx)
            }
            builtins::BuiltInFeature::DictationToFrontmostApp => self
                .execute_dictation_builtin_action(DictationBuiltinAction::FrontmostApp, dctx, cx),
            builtins::BuiltInFeature::DictationToNotes => {
                self.execute_dictation_builtin_action(DictationBuiltinAction::Notes, dctx, cx)
            }
            builtins::BuiltInFeature::FileSearch => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive FileSearch");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            // =========================================================================
            // Settings Hub
            // =========================================================================
            builtins::BuiltInFeature::Settings => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive Settings");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            // =========================================================================
            // Agent Chat Conversation History
            // =========================================================================
            builtins::BuiltInFeature::AgentChatHistory => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive AgentChatHistory");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::AiVault => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive AiVault");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::DictationHistory => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive DictationHistory");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            // =========================================================================
            // SDK Reference — browse kit://sdk-reference functions while authoring
            // =========================================================================
            builtins::BuiltInFeature::SdkReference => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive SdkReference");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            // =========================================================================
            // New Script from Template — browse kit://script-templates catalog, pick
            // a starter, then hand off to the naming prompt with template metadata
            // threaded through so handle_naming_dialog_completion can overwrite the
            // freshly-created file with render_script_template_file before the
            // editor opens. See `src/render_builtins/script_templates.rs` and
            // `src/app_impl/naming_dialog.rs`.
            // =========================================================================
            builtins::BuiltInFeature::NewScriptFromTemplate => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive NewScriptFromTemplate");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
            builtins::BuiltInFeature::MigrateV1Scripts => {
                let open_action = SurfaceOpenBuiltinAction::from_feature(&entry.feature)
                    .expect("surface open arm should only receive MigrateV1Scripts");
                self.execute_surface_open_builtin(open_action, dctx, cx)
            }
        }
    }

    // =========================================================================
    // Dictation helpers — overlay pump, transcript delivery, scheduled cleanup
    // =========================================================================

    fn execute_settings_command_builtin(
        &mut self,
        action: SettingsCommandBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match action {
            SettingsCommandBuiltinAction::ResetWindowPositions => {
                self.reset_window_positions_to_default_main_menu(cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            SettingsCommandBuiltinAction::ChooseTheme => {
                self.open_theme_chooser_view(cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            SettingsCommandBuiltinAction::DictationSetup => {
                self.open_dictation_setup_prompt(cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            SettingsCommandBuiltinAction::SelectMicrophone(microphone_action) => {
                self.execute_select_microphone_builtin(microphone_action, dctx, cx)
            }
            SettingsCommandBuiltinAction::SnapMode(snap_action) => {
                self.execute_settings_snap_mode_builtin(snap_action, dctx, cx)
            }
        }
    }

    fn execute_surface_open_builtin(
        &mut self,
        action: SurfaceOpenBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match action {
            SurfaceOpenBuiltinAction::ClipboardHistory => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.open_clipboard_history_surface_with_filter(String::new(), cx);
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    count = self.cached_clipboard_entries.len(),
                    "Loaded clipboard entries"
                );
            }
            SurfaceOpenBuiltinAction::Favorites => {
                tracing::info!(
                    category = "BUILTIN",
                    action = "open_favorites_view",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );

                self.open_builtin_filterable_view(
                    AppView::FavoritesBrowseView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search favorites...",
                    false,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::AppLauncher => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.apps = app_launcher::scan_applications().clone();
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    count = self.apps.len(),
                    "Loaded applications"
                );
                self.invalidate_filter_cache();
                self.invalidate_grouped_cache();
                self.sync_list_state();

                self.open_builtin_filterable_view(
                    AppView::AppLauncherView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search applications...",
                    false,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::DesignGallery => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );

                self.open_builtin_filterable_view(
                    AppView::DesignGalleryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search designs...",
                    false,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::FooterGallery => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );

                self.open_builtin_filterable_view(
                    AppView::FooterGalleryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search footer variations...",
                    false,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::NonListStates => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );

                self.current_view = AppView::NonListStatesView { selected_index: 0 };
                self.filter_text.clear();
                self.pending_focus = Some(FocusTarget::AppRoot);
                cx.notify();
            }
            SurfaceOpenBuiltinAction::AiChat(variant) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    agent_chat_ui_variant = variant.state_id(),
                    "{}",
                    action.log_message()
                );
                self.open_tab_ai_agent_chat_with_entry_intent_variant(None, variant, cx);
            }
            SurfaceOpenBuiltinAction::EmojiPicker => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.emoji_frequent_snapshot = crate::emoji_usage::load_frequent_snapshot(
                    crate::emoji_usage::EMOJI_FREQUENT_LIMIT,
                );
                self.open_builtin_filterable_view(
                    AppView::EmojiPickerView {
                        filter: String::new(),
                        selected_index: 0,
                        selected_category: None,
                    },
                    "Search Emoji & Symbols...",
                    false,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::Webcam => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.opened_from_main_menu = true;
                self.open_webcam(cx);
            }
            SurfaceOpenBuiltinAction::FileSearch => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.opened_from_main_menu = true;
                self.open_file_search(String::new(), cx);
            }
            SurfaceOpenBuiltinAction::Settings => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.open_builtin_filterable_view(
                    AppView::SettingsView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search settings...",
                    false,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::AgentChatHistory => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.open_builtin_filterable_view(
                    AppView::AgentChatHistoryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search conversation history...",
                    true,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::AiVault => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.open_ai_vault_source_filter(cx);
            }
            SurfaceOpenBuiltinAction::DictationHistory => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.open_builtin_filterable_view(
                    AppView::DictationHistoryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search dictation history...",
                    true,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::SdkReference => {
                let entries = crate::mcp_resources::sdk_reference_entries_for_ui();
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    entry_count = entries.len(),
                    "{}",
                    action.log_message()
                );
                self.open_builtin_filterable_view(
                    AppView::SdkReferenceView {
                        filter: String::new(),
                        selected_index: 0,
                        entries,
                    },
                    "Search SDK functions…",
                    true,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::ScriptTemplateCatalog => {
                let templates = crate::mcp_resources::script_template_entries_for_ui();
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    template_count = templates.len(),
                    "{}",
                    action.log_message()
                );
                self.open_builtin_filterable_view(
                    AppView::ScriptTemplateCatalogView {
                        filter: String::new(),
                        selected_index: 0,
                        templates,
                    },
                    "Search starter templates…",
                    true,
                    cx,
                );
            }
            SurfaceOpenBuiltinAction::MigrateV1Scripts => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "{}",
                    action.log_message()
                );
                self.open_migrate_v1_view(cx);
            }
        }

        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_browser_tabs_builtin(
        &mut self,
        action: BrowserTabsBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            "{}",
            action.opening_message()
        );
        match crate::browser_tabs::list_open_tabs() {
            Ok(tabs) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    count = tabs.len(),
                    "{}",
                    action.loaded_message()
                );
                self.cached_browser_tabs = tabs;

                let urls: Vec<String> = self
                    .cached_browser_tabs
                    .iter()
                    .map(|t| t.url.to_string())
                    .collect();
                let domains = crate::favicons::domains_needing_favicons(&urls);
                if !domains.is_empty() {
                    let task = cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .spawn(async move {
                                crate::favicons::fetch_favicons_blocking(&domains);
                            })
                            .await;
                        let _ = cx.update(|cx| {
                            let _ = this.update(cx, |_, cx| cx.notify());
                        });
                    });
                    task.detach();
                }

                self.open_builtin_filterable_view(
                    AppView::BrowserTabsView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    action.placeholder(),
                    false,
                    cx,
                );

                Self::builtin_success(dctx, action.success_detail())
            }
            Err(error) => {
                let message = action.failure_message(&error);
                self.show_error_toast(message.clone(), cx);
                cx.notify();
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    message,
                    action.failure_detail(),
                )
            }
        }
    }

    fn execute_app_launch_builtin(
        &mut self,
        action: AppLaunchBuiltinAction,
        app_name: &str,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            app = %app_name,
            "{}",
            action.opening_message()
        );
        let apps = app_launcher::scan_applications();
        if let Some(app) = apps.iter().find(|a| a.name == *app_name) {
            if let Err(error) = app_launcher::launch_application(app) {
                let message = action.launch_failure_message(app_name, &error);
                self.show_error_toast(message.clone(), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_LAUNCH_FAILED,
                    message,
                    action.success_detail(app_name),
                )
            } else {
                self.close_and_reset_window(cx);
                Self::builtin_success(dctx, action.success_detail(app_name))
            }
        } else {
            let message = action.not_found_message(app_name);
            self.show_error_toast(message.clone(), cx);
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_ACTION_FAILED,
                message,
                action.not_found_detail(app_name),
            )
        }
    }

    fn execute_notes_builtin(
        &mut self,
        action: NotesBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            target: "script_kit::keyboard",
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            event = "notes_handoff_submit_reset",
            filter_text_len = self.filter_text.len(),
            show_actions_popup = self.show_actions_popup,
            "{}",
            action.opening_message()
        );

        // Close companion UI before hiding so it does not stay stale.
        if crate::confirm::is_confirm_window_open() {
            crate::confirm::route_key_to_confirm_popup("escape", cx);
        }
        if crate::actions::is_actions_window_open() {
            crate::actions::close_actions_window(cx);
            self.mark_actions_popup_closed();
            self.mark_filter_resync_after_actions_if_needed();
            self.pop_focus_overlay(cx);
        }

        self.pending_focus = None;
        if let Err(error) = notes::open_notes_window_without_launcher_restore(cx) {
            let message = action.failure_message(&error);
            self.show_error_toast(message.clone(), cx);
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_LAUNCH_FAILED,
                message,
                action.failure_detail(),
            )
        } else {
            self.close_and_reset_window(cx);
            Self::builtin_success(dctx, action.success_detail())
        }
    }

    fn execute_sync_to_github_builtin(
        &mut self,
        action: SyncToGithubBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            "{}",
            action.request_log_message()
        );
        self.show_hud(action.start_hud().to_string(), Some(HUD_SHORT_MS), cx);

        let dctx_owned = dctx.clone();
        let action_for_task = action;
        cx.spawn(async move |this, cx| {
            let sync_result = cx
                .background_executor()
                .spawn(async { crate::sync::github::sync_to_github_workspace() })
                .await;

            let _ = this.update(cx, |this, cx| match sync_result {
                Ok(report) => {
                    tracing::info!(
                        category = "BUILTIN",
                        trace_id = %dctx_owned.trace_id,
                        workspace = %report.workspace.display(),
                        dry_run = report.dry_run,
                        step_count = report.steps.len(),
                        "{}",
                        action_for_task.completed_log_message()
                    );
                    this.show_hud(report.summary_message(), Some(HUD_MEDIUM_MS), cx);
                    this.close_and_reset_window(cx);
                }
                Err(error) => {
                    tracing::error!(
                        category = "BUILTIN",
                        trace_id = %dctx_owned.trace_id,
                        error = %error,
                        "{}",
                        action_for_task.failed_log_message()
                    );
                    this.show_error_toast(action_for_task.failure_message(&error), cx);
                    cx.notify();
                }
            });
        })
        .detach();

        Self::builtin_success(dctx, action.success_detail())
    }

    #[cfg(feature = "storybook")]
    fn execute_design_explorer_builtin(
        &mut self,
        action: DesignExplorerBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            "Opening Design Explorer"
        );

        let explorer = cx.new(|cx| {
            let mut browser = script_kit_gpui::storybook::StoryBrowser::new(cx);
            match action {
                DesignExplorerBuiltinAction::Open => {
                    browser.configure_for_design_explorer(Some(
                        script_kit_gpui::storybook::StorySurface::MainMenu,
                    ));
                    browser.open_compare_mode();
                    let _ = browser.select_variant_id("current-main-menu");
                }
            }
            tracing::info!(
                event = "design_explorer_opened",
                surface = match action {
                    DesignExplorerBuiltinAction::Open => "main-menu",
                },
                preview_mode = "compare",
                variant_id = match action {
                    DesignExplorerBuiltinAction::Open => Some("current-main-menu"),
                },
                "Opened in-app design explorer"
            );
            browser
        });

        self.current_view = AppView::DesignExplorerView { entity: explorer };
        cx.notify();

        Self::builtin_success(dctx, action.success_detail())
    }

    fn apply_ai_command_window_plan(
        &mut self,
        plan: AiCommandWindowPlan,
        cmd_type: &builtins::AiCommandType,
        cx: &mut Context<Self>,
    ) {
        match plan {
            AiCommandWindowPlan::KeepMainWindowVisible => {}
            AiCommandWindowPlan::HideMainWindowDeferred => {
                script_kit_gpui::set_main_window_visible(false);
                self.reset_to_script_list(cx);
                platform::defer_hide_main_window(cx);
            }
            AiCommandWindowPlan::HideMainWindowForCapture => {
                script_kit_gpui::set_main_window_visible(false);
                self.reset_to_script_list(cx);
                tracing::debug!(
                    action = ?cmd_type,
                    hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
                    "Deferring main window hide to async capture flow"
                );
            }
        }
    }

    fn execute_window_switcher_builtin(
        &mut self,
        action: WindowSwitcherBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            "{}",
            action.opening_message()
        );
        match window_control::list_windows() {
            Ok(windows) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    count = windows.len(),
                    "{}",
                    action.loaded_message()
                );
                self.cached_windows = windows;

                self.open_builtin_filterable_view(
                    AppView::WindowSwitcherView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    action.placeholder(),
                    false,
                    cx,
                );

                Self::builtin_success(dctx, action.success_detail())
            }
            Err(error) => {
                let message = action.failure_message(&error);
                self.show_error_toast(message.clone(), cx);
                cx.notify();
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    message,
                    action.failure_detail(),
                )
            }
        }
    }

    fn execute_ai_capture_builtin(
        &mut self,
        action: AiCaptureBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        self.open_tab_ai_chat_with_capture_kind(
            Some(action.prompt().to_string()),
            action.capture_kind(),
            cx,
        );
        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_ai_command_builtin(
        &mut self,
        action: AiCommandBuiltinAction,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match action {
            AiCommandBuiltinAction::Generate(generate_action) => {
                self.execute_ai_generate_builtin(generate_action, query_override, dctx, cx)
            }
            AiCommandBuiltinAction::Capture(capture_action) => {
                self.execute_ai_capture_builtin(capture_action, dctx, cx)
            }
            AiCommandBuiltinAction::Unavailable(unavailable_action) => {
                self.execute_ai_unavailable_builtin(unavailable_action, dctx, cx)
            }
            AiCommandBuiltinAction::LegacyHarness(legacy_action) => {
                self.execute_ai_legacy_harness_builtin(legacy_action, dctx, cx)
            }
        }
    }

    fn execute_ai_generate_builtin(
        &mut self,
        action: AiGenerateBuiltinAction,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let query = query_override.unwrap_or(&self.filter_text);
        let request = action.normalized_request(query);
        let intent = action.entry_intent(request);
        if intent.is_none() && action.opens_agent_chat_when_request_empty() {
            self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
        } else {
            self.open_tab_ai_chat_with_entry_intent(intent, cx);
        }

        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_ai_unavailable_builtin(
        &mut self,
        action: AiUnavailableBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let message = action.message();
        self.toast_manager.push(
            components::toast::Toast::error(message, &self.theme).duration_ms(Some(TOAST_ERROR_MS)),
        );
        cx.notify();
        Self::builtin_error(
            dctx,
            crate::action_helpers::ERROR_ACTION_FAILED,
            message.to_string(),
            action.failure_detail(),
        )
    }

    fn execute_ai_legacy_harness_builtin(
        &mut self,
        action: AiLegacyHarnessBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_settings_snap_mode_builtin(
        &mut self,
        action: SettingsSnapModeBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        if action == SettingsSnapModeBuiltinAction::Configure {
            return self.execute_configure_snap_mode_prompt(dctx, cx);
        }
        let target_mode = action.target_mode();

        let previous = window_control::current_snap_mode();
        let runtime_active = window_control::is_snap_runtime_active();

        let mode = match window_control::persist_snap_mode(target_mode) {
            Ok(mode) => mode,
            Err(error) => {
                tracing::error!(
                    category = "WINDOW",
                    trace_id = %dctx.trace_id,
                    %error,
                    ?target_mode,
                    "{}",
                    action.persistence_failure_log()
                );
                self.show_hud(
                    action.persistence_failure_hud(&error),
                    Some(HUD_SHORT_MS),
                    cx,
                );
                return Self::builtin_error(
                    dctx,
                    action.persistence_failure_code(),
                    action.persistence_failure_message(),
                    error.to_string(),
                );
            }
        };

        if runtime_active {
            let runtime_result = if mode == window_control::SnapMode::Off {
                window_control::cancel_snap_runtime(cx)
            } else {
                window_control::refresh_snap_runtime_for_mode(cx)
            };

            if let Err(error) = runtime_result {
                tracing::warn!(
                    category = "WINDOW",
                    trace_id = %dctx.trace_id,
                    %error,
                    ?mode,
                    "{}",
                    action.runtime_transition_failure_log()
                );
            }
        }

        tracing::info!(
            category = "WINDOW",
            trace_id = %dctx.trace_id,
            previous = ?previous,
            ?mode,
            runtime_active,
            "Updated snap mode from built-in command"
        );

        self.show_hud(action.hud_text().to_string(), Some(HUD_SHORT_MS), cx);
        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_configure_snap_mode_prompt(
        &mut self,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let current = window_control::current_snap_mode();
        let choices = vec![
            Choice {
                name: if current == window_control::SnapMode::Off {
                    "Disable Snapping (current)".to_string()
                } else {
                    "Disable Snapping".to_string()
                },
                value: "off".to_string(),
                description: Some("Turn off drag snapping and hide snap overlays".to_string()),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_SNAP_MODE_PROMPT_ID,
                    0,
                    "off",
                )),
            },
            Choice {
                name: if current == window_control::SnapMode::Simple {
                    "Snap Mode: Simple (current)".to_string()
                } else {
                    "Snap Mode: Simple".to_string()
                },
                value: "simple".to_string(),
                description: Some(
                    "Use halves, quadrants, center, and almost-maximize targets".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_SNAP_MODE_PROMPT_ID,
                    1,
                    "simple",
                )),
            },
            Choice {
                name: if current == window_control::SnapMode::Expanded {
                    "Snap Mode: Expanded (current)".to_string()
                } else {
                    "Snap Mode: Expanded".to_string()
                },
                value: "expanded".to_string(),
                description: Some(
                    "Use halves, quadrants, thirds, and two-thirds targets".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_SNAP_MODE_PROMPT_ID,
                    2,
                    "expanded",
                )),
            },
            Choice {
                name: if current == window_control::SnapMode::Precision {
                    "Snap Mode: Precision (current)".to_string()
                } else {
                    "Snap Mode: Precision".to_string()
                },
                value: "precision".to_string(),
                description: Some(
                    "Use the full snap grid including sixths for finer placement".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_SNAP_MODE_PROMPT_ID,
                    3,
                    "precision",
                )),
            },
        ];

        let start_index = match current {
            window_control::SnapMode::Off => 0,
            window_control::SnapMode::Simple => 1,
            window_control::SnapMode::Expanded => 2,
            window_control::SnapMode::Precision => 3,
        };

        let choice_count = choices.len();
        tracing::info!(
            category = "AUTOMATION",
            prompt_id = BUILTIN_SNAP_MODE_PROMPT_ID,
            choice_count = choice_count,
            selected_index = start_index,
            semantic_ids_populated = choices.iter().all(|c| c.semantic_id.is_some()),
            "opened_builtin_snap_mode_prompt"
        );

        self.opened_from_main_menu = true;
        self.arg_input.clear();
        self.arg_selected_index = start_index;
        self.focused_input = FocusedInput::ArgPrompt;
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Select Snap Mode".to_string());
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.current_view = AppView::MiniPrompt {
            id: BUILTIN_SNAP_MODE_PROMPT_ID.to_string(),
            placeholder: "Select Snap Mode".to_string(),
            choices,
        };
        resize_to_view_sync(ViewType::MiniPrompt, choice_count.min(5));
        cx.notify();

        Self::builtin_success(dctx, "configure_snap_mode")
    }

    fn execute_select_microphone_builtin(
        &mut self,
        action: SettingsMicrophoneBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let prefs = crate::config::load_user_preferences();
        let menu_items = match crate::dictation::list_input_device_menu_items(
            prefs.dictation.selected_device_id.as_deref(),
        ) {
            Ok(items) => items,
            Err(error) => {
                tracing::error!(
                    category = "DICTATION",
                    error = %error,
                    "{}",
                    action.enumeration_failure_log()
                );
                self.show_hud(action.failure_hud(&error), Some(HUD_SHORT_MS), cx);
                return Self::builtin_error(
                    dctx,
                    action.failure_code(),
                    action.failure_message(),
                    error.to_string(),
                );
            }
        };

        let mut start_index: usize = 0;
        let choices: Vec<Choice> = menu_items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let value = match &item.action {
                    crate::dictation::DictationDeviceSelectionAction::UseSystemDefault => {
                        BUILTIN_MIC_DEFAULT_VALUE.to_string()
                    }
                    crate::dictation::DictationDeviceSelectionAction::UseDevice(id) => id.0.clone(),
                };
                let name = if item.is_selected {
                    if start_index == 0 && idx > 0 {
                        start_index = idx;
                    }
                    format!("{} (current)", item.title)
                } else {
                    item.title.clone()
                };
                Choice {
                    name,
                    value: value.clone(),
                    description: Some(item.subtitle.clone()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_MIC_SELECT_PROMPT_ID,
                        idx,
                        &value,
                    )),
                }
            })
            .collect();

        // Follow the canonical ShowMini pattern from prompt_handler
        // (not open_builtin_filterable_view which targets MainFilter focus)
        let choice_count = choices.len();
        tracing::info!(
            category = "AUTOMATION",
            prompt_id = BUILTIN_MIC_SELECT_PROMPT_ID,
            choice_count = choice_count,
            selected_index = start_index,
            semantic_ids_populated = choices.iter().all(|c| c.semantic_id.is_some()),
            "opened_builtin_microphone_prompt"
        );
        self.opened_from_main_menu = true;
        self.arg_input.clear();
        self.arg_selected_index = start_index;
        self.focused_input = FocusedInput::ArgPrompt;
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(action.placeholder().to_string());
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.current_view = AppView::MiniPrompt {
            id: BUILTIN_MIC_SELECT_PROMPT_ID.to_string(),
            placeholder: action.placeholder().to_string(),
            choices,
        };
        resize_to_view_sync(ViewType::MiniPrompt, choice_count.min(5));
        cx.notify();

        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_script_command_builtin(
        &mut self,
        action: ScriptCommandBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        self.show_naming_dialog(action.naming_target(), cx);
        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_frecency_command_builtin(
        &mut self,
        action: FrecencyCommandBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        self.frecency_store.clear();
        if let Err(e) = self.frecency_store.save() {
            let message = action.failure_message(&e);
            self.show_error_toast(message.clone(), cx);
            cx.notify();
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_ACTION_FAILED,
                message,
                action.failure_detail(),
            )
        } else {
            tracing::info!(
                trace_id = %dctx.trace_id,
                "Cleared all suggested items"
            );
            self.invalidate_grouped_cache();
            self.reset_to_script_list(cx);
            resize_to_view_sync(ViewType::ScriptList, 0);
            self.show_hud(action.hud_text().to_string(), Some(HUD_SHORT_MS), cx);
            cx.notify();
            Self::builtin_success(dctx, action.success_detail())
        }
    }

    fn execute_notes_command_builtin(
        &mut self,
        action: NotesCommandBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        // Close companion UI before hiding so it does not stay stale.
        if crate::confirm::is_confirm_window_open() {
            crate::confirm::route_key_to_confirm_popup("escape", cx);
        }
        if crate::actions::is_actions_window_open() {
            crate::actions::close_actions_window(cx);
            self.mark_actions_popup_closed();
            self.mark_filter_resync_after_actions_if_needed();
            self.pop_focus_overlay(cx);
        }

        self.pending_focus = None;

        let result = match action {
            NotesCommandBuiltinAction::OpenNotes => {
                notes::open_notes_window_without_launcher_restore(cx)
            }
            NotesCommandBuiltinAction::SearchNotes => notes::open_notes_search(cx),
            NotesCommandBuiltinAction::NewNote | NotesCommandBuiltinAction::QuickCapture => {
                notes::quick_capture(cx)
            }
        };

        if let Err(e) = result {
            let message = action.failure_message(&e);
            self.show_error_toast(message.clone(), cx);
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_LAUNCH_FAILED,
                message,
                action.failure_detail(),
            )
        } else {
            self.close_and_reset_window(cx);
            Self::builtin_success(dctx, action.success_detail())
        }
    }

    fn execute_utility_open_builtin(
        &mut self,
        action: UtilityOpenBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        if let Some(message) = action.opening_message() {
            tracing::info!(
                category = "BUILTIN",
                trace_id = %dctx.trace_id,
                "{}", message
            );
        }

        if action.opens_from_main_menu() {
            self.opened_from_main_menu = true;
        }

        match action {
            UtilityOpenBuiltinAction::MainWindow => self.open_main_window(cx),
            UtilityOpenBuiltinAction::ScratchPad => self.open_scratch_pad(cx),
            UtilityOpenBuiltinAction::QuickTerminal => self.open_quick_terminal(None, cx),
            UtilityOpenBuiltinAction::ClaudeCode => self.open_claude_code_terminal(cx),
            UtilityOpenBuiltinAction::ProcessManager => {
                let processes =
                    crate::process_manager::PROCESS_MANAGER.get_active_processes_sorted();
                tracing::info!(
                    trace_id = %dctx.trace_id,
                    active_process_count = processes.len(),
                    "process_manager.open_view"
                );

                self.cached_processes = processes;
                self.open_builtin_filterable_view(
                    AppView::ProcessManagerView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search running scripts...",
                    false,
                    cx,
                );
                self.start_process_manager_refresh(cx);
            }
        }

        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_utility_command_builtin(
        &mut self,
        action: UtilityCommandBuiltinAction,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match action {
            UtilityCommandBuiltinAction::Open(open_action) => {
                self.execute_utility_open_builtin(open_action, dctx, cx)
            }
            UtilityCommandBuiltinAction::Process(process_action) => {
                self.execute_utility_process_builtin(process_action, dctx, cx)
            }
            UtilityCommandBuiltinAction::Selfie(selfie_action) => {
                self.execute_utility_selfie_builtin(selfie_action, dctx, cx)
            }
            UtilityCommandBuiltinAction::Recipe(recipe_action) => self
                .execute_utility_turn_this_into_command_builtin(
                    recipe_action,
                    query_override,
                    dctx,
                    cx,
                ),
            UtilityCommandBuiltinAction::DoInCurrentApp(do_in_action) => self
                .execute_utility_do_in_current_app_builtin(do_in_action, query_override, dctx, cx),
            UtilityCommandBuiltinAction::CurrentAppCommands(current_app_action) => {
                self.execute_utility_current_app_commands_builtin(current_app_action, dctx, cx)
            }
        }
    }

    fn execute_utility_process_builtin(
        &mut self,
        action: UtilityProcessBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let process_count = crate::process_manager::PROCESS_MANAGER.active_count();
        tracing::info!(
            trace_id = %dctx.trace_id,
            requested_count = process_count,
            "process_manager.stop_all"
        );

        let outcome = action.outcome(process_count);
        if !outcome.should_stop_processes() {
            self.show_hud(action.empty_hud().to_string(), Some(HUD_2200_MS), cx);
        } else {
            crate::process_manager::PROCESS_MANAGER.kill_all_processes();
            self.show_hud(
                action.success_hud(outcome.process_count()),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            self.close_and_reset_window(cx);
        }
        Self::builtin_success(dctx, action.success_detail())
    }

    fn execute_utility_selfie_builtin(
        &mut self,
        action: UtilitySelfieBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let state = self.app_view_name();
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            state = %state,
            "script_kit_selfie.capture_requested"
        );
        self.show_hud(action.starting_hud(&state), Some(HUD_2200_MS), cx);

        match crate::platform::capture_script_kit_selfie(&state) {
            Ok(receipt) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    image_path = %receipt.png_path,
                    receipt_path = %receipt.receipt_path,
                    "script_kit_selfie.capture_saved"
                );
                self.show_hud(action.saved_hud(&receipt), Some(HUD_MEDIUM_MS), cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            Err(error) => {
                let message = action.failure_message(&error);
                tracing::warn!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    error = %error,
                    "script_kit_selfie.capture_failed"
                );
                self.show_error_toast(message.clone(), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    message,
                    action.failure_detail(),
                )
            }
        }
    }

    fn execute_utility_turn_this_into_command_builtin(
        &mut self,
        action: UtilityRecipeBuiltinAction,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let raw_query_owned = query_override.unwrap_or(&self.filter_text).to_string();

        let effective_query =
            crate::menu_bar::current_app_commands::normalize_turn_this_into_a_command_request(
                Some(&raw_query_owned),
            )
            .unwrap_or_default();

        if effective_query.is_empty() {
            let message =
                "Type what you want to automate after \"Turn This Into a Command\"".to_string();
            self.show_error_toast(message.clone(), cx);
            return Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_ACTION_FAILED,
                message,
                action.missing_query_failure_detail(),
            );
        }

        tracing::info!(
            trace_id = %dctx.trace_id,
            raw_query = %raw_query_owned,
            effective_query = %effective_query,
            "turn_this_into_command.requested"
        );

        match crate::menu_bar::load_frontmost_menu_snapshot() {
            Ok(snapshot) => {
                let selected_text = crate::selected_text::get_selected_text()
                    .ok()
                    .filter(|text| !text.trim().is_empty());

                let browser_url = crate::platform::get_focused_browser_tab_url()
                    .ok()
                    .filter(|url| !url.trim().is_empty());

                let recipe =
                    crate::menu_bar::current_app_commands::build_current_app_command_recipe(
                        snapshot,
                        Some(&raw_query_owned),
                        selected_text.as_deref(),
                        browser_url.as_deref(),
                    );

                match serde_json::to_string_pretty(&recipe) {
                    Ok(json) => {
                        tracing::info!(
                            category = "CURRENT_APP_RECIPE",
                            trace_id = %dctx.trace_id,
                            app_name = %recipe.prompt_receipt.app_name,
                            bundle_id = %recipe.prompt_receipt.bundle_id,
                            effective_query = %recipe.effective_query,
                            route = %recipe.trace.action,
                            suggested_script_name = %recipe.suggested_script_name,
                            candidate_count = recipe.trace.candidates.len(),
                            included_selected_text = recipe.prompt_receipt.included_selected_text,
                            included_browser_url = recipe.prompt_receipt.included_browser_url,
                            json_bytes = json.len(),
                            "turn_this_into_command.recipe_copied"
                        );

                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(json));

                        self.show_hud(
                            action.copied_recipe_hud(&recipe.suggested_script_name),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );

                        self.spawn_generate_script_from_recipe_after_hide(
                            dctx.trace_id.to_string(),
                            recipe.clone(),
                            cx,
                        );

                        Self::builtin_success(dctx, action.success_detail())
                    }
                    Err(e) => {
                        let message = action.serialize_failure_message(&e);
                        tracing::error!(
                            trace_id = %dctx.trace_id,
                            error = %e,
                            "turn_this_into_command.serialize_failed"
                        );
                        self.show_error_toast(message.clone(), cx);
                        Self::builtin_error(
                            dctx,
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            message,
                            action.serialize_failure_detail(),
                        )
                    }
                }
            }
            Err(e) => {
                let message = format!(
                    "Failed to capture current app command recipe: {}. Check Accessibility permission in System Settings → Privacy & Security → Accessibility, then refocus the target app and try again.",
                    e
                );
                tracing::warn!(
                    trace_id = %dctx.trace_id,
                    error = %e,
                    "turn_this_into_command.capture_failed"
                );
                self.show_error_toast(message.clone(), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    message,
                    action.capture_failure_detail(),
                )
            }
        }
    }

    fn execute_utility_do_in_current_app_builtin(
        &mut self,
        action: UtilityDoInCurrentAppBuiltinAction,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let raw_query_owned = query_override.unwrap_or(&self.filter_text).to_string();
        let raw_query_safe = crate::logging::log_user_value(&raw_query_owned);
        let filter_text_safe = crate::logging::log_user_value(&self.filter_text);
        tracing::info!(
            target: "script_kit::do_in_trace",
            event = "DO_IN_TRACE execution.entry",
            trace_id = %dctx.trace_id,
            raw_query_preview = %raw_query_safe,
            raw_query_bytes = raw_query_safe.raw_bytes,
            raw_query_safe_bytes = raw_query_safe.safe_bytes,
            raw_query_truncated = raw_query_safe.truncated,
            filter_text_preview = %filter_text_safe,
            filter_text_bytes = filter_text_safe.raw_bytes,
            filter_text_safe_bytes = filter_text_safe.safe_bytes,
            filter_text_truncated = filter_text_safe.truncated,
            query_override = ?query_override,
            current_view = ?self.current_view,
            "DO_IN_TRACE execution.entry"
        );
        tracing::info!(
            trace_id = %dctx.trace_id,
            raw_query = %raw_query_owned,
            filter_text = %self.filter_text,
            query_override = ?query_override,
            "do_in_current_app.execution_entry — raw inputs"
        );
        let effective_query =
            crate::menu_bar::current_app_commands::effective_do_in_current_app_query_for_submission(
                &raw_query_owned,
                query_override,
            );
        let effective_query_for_router =
            (!effective_query.is_empty()).then_some(effective_query.as_str());
        let effective_query_safe = crate::logging::log_user_value(&effective_query);

        tracing::info!(
            target: "script_kit::do_in_trace",
            event = "DO_IN_TRACE execution.normalized",
            trace_id = %dctx.trace_id,
            query_preview = %effective_query_safe,
            query_bytes = effective_query_safe.raw_bytes,
            query_safe_bytes = effective_query_safe.safe_bytes,
            query_truncated = effective_query_safe.truncated,
            raw_query_preview = %raw_query_safe,
            "DO_IN_TRACE execution.normalized"
        );
        tracing::info!(
            trace_id = %dctx.trace_id,
            query = %effective_query,
            "do_in_current_app.requested"
        );

        match crate::menu_bar::load_frontmost_menu_snapshot() {
            Ok(snapshot) => {
                let snapshot_for_recipe = snapshot.clone();
                let snapshot_pid = snapshot.pid;
                let (entries, snapshot_receipt) = snapshot.into_entries_with_receipt();

                let (resolved_action, intent_receipt) =
                    crate::menu_bar::current_app_commands::resolve_do_in_current_app_intent(
                        &entries,
                        effective_query_for_router,
                    );

                tracing::info!(
                    target: "script_kit::do_in_trace",
                    event = "DO_IN_TRACE execution.resolved",
                    trace_id = %dctx.trace_id,
                    app_name = %snapshot_receipt.app_name,
                    bundle_id = %snapshot_receipt.bundle_id,
                    leaf_entry_count = snapshot_receipt.leaf_entry_count,
                    query_preview = %effective_query_safe,
                    raw_query_preview = %raw_query_safe,
                    filtered_entries = intent_receipt.filtered_entries,
                    exact_matches = intent_receipt.exact_matches,
                    resolved_action = intent_receipt.action,
                    "DO_IN_TRACE execution.resolved"
                );
                tracing::info!(
                    trace_id = %dctx.trace_id,
                    app_name = %snapshot_receipt.app_name,
                    bundle_id = %snapshot_receipt.bundle_id,
                    leaf_entry_count = snapshot_receipt.leaf_entry_count,
                    query = %effective_query,
                    filtered_entries = intent_receipt.filtered_entries,
                    exact_matches = intent_receipt.exact_matches,
                    resolved_action = intent_receipt.action,
                    "do_in_current_app.resolved"
                );

                match resolved_action {
                    crate::menu_bar::current_app_commands::DoInCurrentAppAction::OpenCommandPalette => {
                        tracing::info!(
                            target: "script_kit::do_in_trace",
                            event = "DO_IN_TRACE execution.open_palette",
                            trace_id = %dctx.trace_id,
                            cached_entries = entries.len(),
                            filter_preview = %effective_query_safe,
                            placeholder = %snapshot_receipt.placeholder,
                            "DO_IN_TRACE execution.open_palette"
                        );
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            cached_entries = entries.len(),
                            filter = %effective_query,
                            placeholder = %snapshot_receipt.placeholder,
                            "do_in_current_app.action → OpenCommandPalette — switching to CurrentAppCommandsView"
                        );
                        self.present_current_app_commands_entries(
                            entries,
                            &snapshot_receipt,
                            snapshot_pid,
                            &effective_query,
                            cx,
                        );
                        Self::builtin_success(dctx, action.open_palette_success_detail())
                    }
                    crate::menu_bar::current_app_commands::DoInCurrentAppAction::ExecuteEntry(
                        entry_index,
                    ) => {
                        let entry = match entries.get(entry_index) {
                            Some(e) => e.clone(),
                            None => {
                                let message = format!(
                                    "Failed to execute command: menu entry index {} is out of bounds (total {} entries).",
                                    entry_index,
                                    entries.len()
                                );
                                tracing::warn!(
                                    trace_id = %dctx.trace_id,
                                    entry_index = entry_index,
                                    total_entries = entries.len(),
                                    "do_in_current_app.execute_entry — index out of bounds"
                                );
                                self.show_error_toast(message.clone(), cx);
                                return Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "do_in_current_app_execute_index_out_of_bounds",
                                );
                            }
                        };
                        tracing::info!(
                            target: "script_kit::do_in_trace",
                            event = "DO_IN_TRACE execution.execute_entry",
                            trace_id = %dctx.trace_id,
                            entry_index = entry_index,
                            entry_name = %entry.name,
                            query_preview = %effective_query_safe,
                            raw_query_preview = %raw_query_safe,
                            "DO_IN_TRACE execution.execute_entry"
                        );
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            entry_index = entry_index,
                            entry_name = %entry.name,
                            "do_in_current_app.action → ExecuteEntry — running menu command directly"
                        );
                        self.execute_builtin_inner(&entry, effective_query_for_router, dctx, cx)
                    }
                    crate::menu_bar::current_app_commands::DoInCurrentAppAction::GenerateScript => {
                        tracing::info!(
                            target: "script_kit::do_in_trace",
                            event = "DO_IN_TRACE execution.generate_script",
                            trace_id = %dctx.trace_id,
                            query_preview = %effective_query_safe,
                            raw_query_preview = %raw_query_safe,
                            "DO_IN_TRACE execution.generate_script"
                        );
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            query = %effective_query,
                            "do_in_current_app.action → GenerateScript — scheduling async context capture before recipe flow"
                        );

                        self.spawn_generate_script_from_current_app_with_capture(
                            dctx.trace_id.to_string(),
                            effective_query.clone(),
                            snapshot_for_recipe,
                            entries,
                            snapshot_receipt.clone(),
                            snapshot_pid,
                            cx,
                        );

                        Self::builtin_success(dctx, action.generate_script_success_detail())
                    }
                }
            }
            Err(e) => {
                let message = action.capture_failure_message(&e);
                self.show_error_toast(message.clone(), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    message,
                    action.capture_failure_detail(),
                )
            }
        }
    }

    fn execute_utility_current_app_commands_builtin(
        &mut self,
        action: UtilityCurrentAppCommandsBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            trace_id = %dctx.trace_id,
            "current_app_commands.open_requested"
        );
        match crate::menu_bar::load_frontmost_menu_snapshot() {
            Ok(snapshot) => {
                let pid = snapshot.pid;
                let (entries, receipt) = snapshot.into_entries_with_receipt();

                tracing::info!(
                    trace_id = %dctx.trace_id,
                    pid,
                    app_name = %receipt.app_name,
                    bundle_id = %receipt.bundle_id,
                    top_level_menu_count = receipt.top_level_menu_count,
                    leaf_entry_count = receipt.leaf_entry_count,
                    placeholder = %receipt.placeholder,
                    source = receipt.source,
                    "current_app_commands.snapshot_ready"
                );

                self.present_current_app_commands_entries(entries, &receipt, pid, "", cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            Err(e) => {
                let message = action.capture_failure_message(&e);
                tracing::warn!(
                    trace_id = %dctx.trace_id,
                    error = %e,
                    "current_app_commands.capture_failed"
                );
                self.show_error_toast(message.clone(), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_ACTION_FAILED,
                    message,
                    action.capture_failure_detail(),
                )
            }
        }
    }

    fn execute_permission_command_builtin(
        &mut self,
        action: PermissionCommandBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match action {
            PermissionCommandBuiltinAction::CheckPermissions => {
                // Honest check across all five permission kinds (not just
                // accessibility). Anything ungranted routes into the wizard
                // so the user can fix it in place.
                let snapshot = permissions_wizard::PermissionSnapshot::current();
                let ungranted: Vec<&str> = snapshot
                    .cards
                    .iter()
                    .filter(|card| {
                        card.status != crate::platform::permiso_detect::PermissionStatus::Authorized
                    })
                    .map(|card| card.kind.name())
                    .collect();
                if ungranted.is_empty() {
                    self.show_hud(
                        action.all_permissions_granted_hud().to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    );
                } else {
                    self.toast_manager.push(
                        components::toast::Toast::warning(
                            format!("Not yet granted: {}", ungranted.join(", ")),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_WARNING_MS)),
                    );
                    self.open_permissions_wizard(cx);
                }
                cx.notify();
                Self::builtin_success(dctx, action.success_detail())
            }
            PermissionCommandBuiltinAction::SetupPermissions => {
                self.opened_from_main_menu = true;
                self.open_permissions_wizard(cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            PermissionCommandBuiltinAction::RequestAccessibility => {
                let granted = permissions_wizard::request_accessibility_permission();
                if granted {
                    self.show_hud(
                        action.accessibility_granted_hud().to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    );
                } else {
                    self.toast_manager.push(
                        components::toast::Toast::warning(
                            action.accessibility_not_granted_warning(),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_WARNING_MS)),
                    );
                }
                cx.notify();
                Self::builtin_success(dctx, action.success_detail())
            }
            PermissionCommandBuiltinAction::OpenAccessibilitySettings => {
                if let Err(e) = permissions_wizard::open_accessibility_settings() {
                    let message = action.open_settings_failure_message(&e);
                    self.show_error_toast(message.clone(), cx);
                    Self::builtin_error(
                        dctx,
                        crate::action_helpers::ERROR_LAUNCH_FAILED,
                        message,
                        action.failure_detail(),
                    )
                } else {
                    self.close_and_reset_window(cx);
                    Self::builtin_success(dctx, action.success_detail())
                }
            }
            PermissionCommandBuiltinAction::Assistant(assistant_action) => {
                self.execute_permission_assistant_builtin(assistant_action, dctx, cx)
            }
        }
    }

    fn execute_permission_assistant_builtin(
        &mut self,
        action: PermissionAssistantBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match platform::permiso::PermisoAssistant::present_retained(action.panel()) {
            Ok(()) => {
                self.show_hud(action.success_hud().to_string(), Some(HUD_SHORT_MS), cx);
                Self::builtin_success(dctx, action.success_detail())
            }
            Err(error) => {
                let message = action.failure_message(&error);
                self.show_error_toast(message.clone(), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_LAUNCH_FAILED,
                    message,
                    action.failure_detail(),
                )
            }
        }
    }

    fn execute_paste_sequential_builtin(
        &mut self,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            action = "paste_sequential",
            event = "trigger",
            trace_id = %dctx.trace_id,
            "Paste Sequentially triggered"
        );

        let paste_action = PasteSequentialBuiltinAction::from_outcome(
            clipboard_history::advance_paste_sequence(&mut self.paste_sequential_state),
        );

        match &paste_action {
            PasteSequentialBuiltinAction::PasteEntry(entry_id) => {
                tracing::info!(
                    action = "paste_sequential",
                    event = paste_action.telemetry_event(),
                    entry_id = %entry_id,
                    trace_id = %dctx.trace_id,
                    "{}", paste_action.log_message()
                );
                match clipboard_history::enqueue_sequential_paste(entry_id.clone()) {
                    Ok(()) => {
                        clipboard_history::commit_paste_sequence(&mut self.paste_sequential_state);
                        self.hide_main_and_reset(cx);
                        Self::builtin_success(dctx, paste_action.success_detail())
                    }
                    Err(clipboard_history::EnqueuePasteError::WorkerDisconnected) => {
                        tracing::error!(
                            action = "paste_sequential",
                            event = "enqueue_failed",
                            error_code = "worker_disconnected",
                            trace_id = %dctx.trace_id,
                            "Paste worker is not running"
                        );
                        self.toast_manager.push(
                            components::toast::Toast::error(
                                "Paste worker crashed — restart Script Kit",
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_CRITICAL_MS)),
                        );
                        cx.notify();
                        Self::builtin_error(
                            dctx,
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            "Paste worker crashed",
                            "paste_sequential_worker_disconnected",
                        )
                    }
                }
            }
            PasteSequentialBuiltinAction::SequenceExhausted
            | PasteSequentialBuiltinAction::HistoryEmpty => {
                tracing::info!(
                    action = "paste_sequential",
                    event = paste_action.telemetry_event(),
                    trace_id = %dctx.trace_id,
                    "{}", paste_action.log_message()
                );
                if let Some(message) = paste_action.hud_message() {
                    self.show_hud(message.to_string(), Some(HUD_SHORT_MS), cx);
                }
                Self::builtin_success(dctx, paste_action.success_detail())
            }
        }
    }

    fn execute_dictation_builtin_action(
        &mut self,
        action: DictationBuiltinAction,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        tracing::info!(
            category = "BUILTIN",
            trace_id = %dctx.trace_id,
            "{}", action.opening_message()
        );

        let is_start_edge = !crate::dictation::is_dictation_busy();
        tracing::info!(
            category = "DICTATION",
            action = ?action,
            trace_id = %dctx.trace_id,
            is_start_edge,
            "Dictation builtin shortcut flow reached toggle boundary"
        );
        if is_start_edge {
            let preflight = self.prepare_dictation_builtin_start(action, cx);
            tracing::info!(
                category = "DICTATION",
                action = ?action,
                trace_id = %dctx.trace_id,
                preflight = ?preflight,
                "Dictation builtin start preflight completed"
            );
            if preflight != DictationStartPreflight::Ready {
                return Self::builtin_success(dctx, preflight.success_detail());
            }
        }

        let dictation_target = if is_start_edge {
            self.dictation_start_target(action)
        } else {
            crate::dictation::get_dictation_target()
                .unwrap_or_else(|| action.stop_fallback_target())
        };
        tracing::info!(
            category = "DICTATION",
            action = ?action,
            trace_id = %dctx.trace_id,
            is_start_edge,
            ?dictation_target,
            "Invoking dictation toggle from builtin shortcut flow"
        );

        if !is_start_edge {
            self.request_active_dictation_stop(
                crate::dictation::DictationStopReason::Hotkey,
                dictation_target,
                true,
                cx,
            );
            return Self::builtin_success(dctx, action.success_detail());
        }

        match crate::dictation::toggle_dictation(dictation_target) {
            Ok(crate::dictation::DictationToggleOutcome::Started) => {
                tracing::info!(
                    category = "DICTATION",
                    action = ?action,
                    trace_id = %dctx.trace_id,
                    ?dictation_target,
                    "Dictation toggle returned Started"
                );
                if is_start_edge
                    && matches!(
                        dictation_target,
                        crate::dictation::DictationTarget::ExternalApp
                    )
                {
                    self.close_and_reset_window(cx);
                }
                self.handle_dictation_started(action, dictation_target, cx);
            }
            Ok(crate::dictation::DictationToggleOutcome::Stopped(_)) => {}
            Err(error) => {
                tracing::error!(
                    category = "DICTATION",
                    error = %error,
                    failure_message = action.failure_message(),
                    "Dictation toggle failed"
                );
                let _ = crate::dictation::update_dictation_overlay(
                    crate::dictation::DictationOverlayState {
                        phase: crate::dictation::DictationSessionPhase::Failed(error.to_string()),
                        ..Default::default()
                    },
                    cx,
                );
                self.schedule_dictation_overlay_close(cx, std::time::Duration::from_millis(800));
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::AbortDictation,
                    cx,
                );
            }
        }

        Self::builtin_success(dctx, action.success_detail())
    }

    fn prepare_dictation_builtin_start(
        &mut self,
        action: DictationBuiltinAction,
        cx: &mut Context<Self>,
    ) -> DictationStartPreflight {
        let prefs = crate::config::load_user_preferences();
        let model_id =
            crate::dictation::DictationModelId::from_preference(prefs.dictation.model.as_deref());
        let model = crate::dictation::dictation_model_entry(model_id);
        if !model.is_available() {
            if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS.load(std::sync::atomic::Ordering::Acquire) {
                *pending_dictation_model_action().lock() = Some(action);
                self.open_dictation_model_prompt(cx);
                return DictationStartPreflight::DownloadInProgress;
            }
            tracing::info!(
                category = "DICTATION",
                model_id = model.stable_id,
                model_display_name = model.display_name,
                "Dictation model not downloaded, opening consent prompt"
            );
            *pending_dictation_model_action().lock() = Some(action);
            self.open_dictation_model_prompt(cx);
            return DictationStartPreflight::OpenedModelPrompt;
        }

        if self.open_dictation_setup_if_not_ready(cx) {
            return DictationStartPreflight::OpenedSetup;
        }

        if let Err(error) = self.ensure_dictation_builtin_target_available(action) {
            let error_text = error.to_string();
            tracing::error!(
                category = "DICTATION",
                error = %error_text,
                action = ?action,
                "{}", action.preflight_failure_message()
            );
            self.show_error_toast(format!("Dictation unavailable: {error_text}"), cx);
            return DictationStartPreflight::Failed;
        }

        DictationStartPreflight::Ready
    }

    fn ensure_dictation_builtin_target_available(
        &self,
        action: DictationBuiltinAction,
    ) -> anyhow::Result<()> {
        if let Some(target) = action.forced_target() {
            self.ensure_dictation_delivery_target_available_for(target)
        } else {
            self.ensure_dictation_delivery_target_available()
        }
    }

    fn dictation_start_target(
        &self,
        action: DictationBuiltinAction,
    ) -> crate::dictation::DictationTarget {
        if let Some(target) = action.forced_target() {
            return target;
        }

        let prefs = crate::config::load_user_preferences();
        let target = match prefs.dictation.target_mode() {
            crate::config::DictationTargetMode::Explicit(target) => target,
            crate::config::DictationTargetMode::Sticky => prefs
                .dictation
                .sticky_target()
                .unwrap_or_else(|| self.resolve_dictation_target()),
            crate::config::DictationTargetMode::Context => self.resolve_dictation_target(),
        };

        // A sticky/explicit frontmost-app target is only honorable when the
        // tracker actually has a paste target; fall back to context capture
        // instead of guaranteeing a delivery refusal later.
        if matches!(target, crate::dictation::DictationTarget::ExternalApp)
            && !Self::has_dictation_frontmost_target()
        {
            return self.resolve_dictation_target();
        }

        target
    }

    fn handle_dictation_started(
        &mut self,
        action: DictationBuiltinAction,
        dictation_target: crate::dictation::DictationTarget,
        cx: &mut Context<Self>,
    ) {
        let _ = crate::dictation::set_dictation_target_cycle(
            self.dictation_target_cycle_for(dictation_target),
        );

        if action.log_forced_route() {
            tracing::info!(
                category = "DICTATION",
                ?dictation_target,
                target_label = dictation_target.overlay_label(),
                "Starting forced-route dictation"
            );
        }

        if action.conceal_before_overlay() {
            platform::conceal_main_window();
        }

        let orch_target =
            crate::window_orchestrator::executor::to_orchestrator_target(&dictation_target);
        if action.dispatch_start_before_overlay() {
            self.dispatch_window_event(
                crate::window_orchestrator::WindowEvent::StartDictation {
                    target: orch_target,
                },
                cx,
            );
            self.start_dictation_overlay_session(cx);
        } else {
            self.start_dictation_overlay_session(cx);
            self.dispatch_window_event(
                crate::window_orchestrator::WindowEvent::StartDictation {
                    target: orch_target,
                },
                cx,
            );
        }
    }

    /// Open the overlay, register the abort callback, start the pump.
    ///
    /// Shared by both `BuiltInFeature::Dictation` and
    /// `BuiltInFeature::DictationToAiHarness` so confirm/resume fixes
    /// only need one change.
    fn start_dictation_overlay_session(&mut self, cx: &mut Context<Self>) {
        let _ = crate::dictation::begin_overlay_session();
        let app_entity = cx.entity().downgrade();
        let abort_app_entity = app_entity.clone();
        crate::dictation::set_overlay_abort_callback(move |cx| {
            if let Some(app) = abort_app_entity.upgrade() {
                app.update(cx, |this, cx| {
                    let dictation_target = crate::dictation::get_dictation_target()
                        .unwrap_or_else(|| this.resolve_dictation_target());
                    this.request_active_dictation_stop(
                        crate::dictation::DictationStopReason::OverlayAbort,
                        dictation_target,
                        false,
                        cx,
                    );
                });
            }
        });
        crate::dictation::set_overlay_submit_callback(move |cx| {
            if let Some(app) = app_entity.upgrade() {
                app.update(cx, |this, cx| {
                    this.submit_active_dictation_from_overlay(cx);
                });
            }
        });
        let _ = crate::dictation::open_dictation_overlay(cx);
        let _ = crate::dictation::update_dictation_overlay(
            crate::dictation::DictationOverlayState {
                phase: crate::dictation::DictationSessionPhase::Recording,
                ..Default::default()
            },
            cx,
        );
        self.spawn_dictation_overlay_pump(cx);
    }

    /// Stop the active recording from the overlay Stop action and continue
    /// through the same transcription/delivery path as the dictation hotkey.
    fn submit_active_dictation_from_overlay(&mut self, cx: &mut Context<Self>) {
        let dictation_target = crate::dictation::get_dictation_target()
            .unwrap_or_else(|| self.resolve_dictation_target());

        self.request_active_dictation_stop(
            crate::dictation::DictationStopReason::OverlaySubmit,
            dictation_target,
            true,
            cx,
        );
    }

    fn request_active_dictation_stop(
        &mut self,
        reason: crate::dictation::DictationStopReason,
        _dictation_target: crate::dictation::DictationTarget,
        transcribe: bool,
        cx: &mut Context<Self>,
    ) {
        // Freeze the timer at the moment the user stopped so the overlay can
        // keep it on screen (grayed) instead of resetting to 0:00 while the
        // capture is transcribed.
        let elapsed_at_stop = crate::dictation::dictation_elapsed().unwrap_or_default();
        match crate::dictation::begin_stop_capture(reason) {
            Ok(crate::dictation::BeginStopCapture::Started {
                request_id,
                target,
                job,
            }) => {
                if transcribe {
                    let _ = crate::dictation::begin_overlay_session();
                    let _ = crate::dictation::open_dictation_overlay(cx);
                    // Keep the recognized-so-far text visible while the final
                    // transcription runs — the overlay pulses it instead of
                    // blanking into a bare progress indicator.
                    let partial = crate::dictation::last_partial_transcript().unwrap_or_default();
                    let _ = crate::dictation::update_dictation_overlay(
                        crate::dictation::DictationOverlayState {
                            phase: crate::dictation::DictationSessionPhase::Transcribing,
                            elapsed: elapsed_at_stop,
                            transcript: partial.into(),
                            target,
                            ..Default::default()
                        },
                        cx,
                    );
                }
                cx.spawn(async move |this, cx| {
                    let stop_result = cx
                        .background_executor()
                        .spawn(async move {
                            job.collect_with_deadline(Self::DICTATION_STOP_COLLECT_DEADLINE)
                        })
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if !crate::dictation::finish_stop_capture(request_id, &stop_result) {
                            return;
                        }
                        match stop_result {
                            Ok(Some(capture)) if transcribe => {
                                if capture.truncated {
                                    tracing::warn!(
                                        category = "DICTATION",
                                        request_id,
                                        "Capture collection timed out; transcribing partial audio"
                                    );
                                    this.show_error_toast(
                                        "Dictation stop timed out — the tail of the recording may be missing"
                                            .to_string(),
                                        cx,
                                    );
                                }
                                this.begin_dictation_transcription(capture, target, cx);
                            }
                            Ok(_) => {
                                let _ = crate::dictation::close_dictation_overlay(cx);
                                this.dispatch_window_event(
                                    crate::window_orchestrator::WindowEvent::AbortDictation,
                                    cx,
                                );
                            }
                            Err(error) => {
                                let error_text = error.to_string();
                                tracing::error!(
                                    category = "DICTATION",
                                    error = %error_text,
                                    request_id,
                                    ?reason,
                                    "Failed to stop dictation capture"
                                );
                                let _ = crate::dictation::open_dictation_overlay(cx);
                                let _ = crate::dictation::update_dictation_overlay(
                                    crate::dictation::DictationOverlayState {
                                        phase: crate::dictation::DictationSessionPhase::Failed(
                                            error_text,
                                        ),
                                        target,
                                        ..Default::default()
                                    },
                                    cx,
                                );
                                this.schedule_dictation_overlay_close(
                                    cx,
                                    std::time::Duration::from_millis(800),
                                );
                                this.dispatch_window_event(
                                    crate::window_orchestrator::WindowEvent::AbortDictation,
                                    cx,
                                );
                            }
                        }
                    });
                })
                .detach();
            }
            Ok(crate::dictation::BeginStopCapture::AlreadyStopping { request_id }) => {
                tracing::info!(
                    category = "DICTATION",
                    stop_request_id = request_id,
                    ?reason,
                    "Ignoring dictation stop request while stop is already in flight"
                );
            }
            Ok(crate::dictation::BeginStopCapture::NotRecording) => {
                let _ = crate::dictation::close_dictation_overlay(cx);
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::AbortDictation,
                    cx,
                );
            }
            Err(error) => {
                tracing::error!(
                    category = "DICTATION",
                    error = %error,
                    "Failed to stop dictation from overlay"
                );
                let _ = crate::dictation::open_dictation_overlay(cx);
                let _ = crate::dictation::update_dictation_overlay(
                    crate::dictation::DictationOverlayState {
                        phase: crate::dictation::DictationSessionPhase::Failed(error.to_string()),
                        ..Default::default()
                    },
                    cx,
                );
                self.schedule_dictation_overlay_close(cx, std::time::Duration::from_millis(800));
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::AbortDictation,
                    cx,
                );
            }
        }
    }

    /// Transition a completed capture into the transcribing overlay state
    /// and kick off async transcription.
    ///
    /// Shared by both dictation entry points so the handoff cannot drift.
    fn begin_dictation_transcription(
        &mut self,
        capture: crate::dictation::CompletedDictationCapture,
        target: crate::dictation::DictationTarget,
        cx: &mut Context<Self>,
    ) {
        let _ = crate::dictation::update_dictation_overlay(
            crate::dictation::DictationOverlayState {
                phase: crate::dictation::DictationSessionPhase::Transcribing,
                elapsed: capture.audio_duration,
                transcript: crate::dictation::last_partial_transcript()
                    .unwrap_or_default()
                    .into(),
                target,
                ..Default::default()
            },
            cx,
        );
        let audio_duration = capture.audio_duration;
        let chunks = capture.chunks;
        cx.spawn(async move |this, cx| {
            let transcript_result = cx
                .background_executor()
                .spawn(async move { crate::dictation::transcribe_captured_audio(&chunks) })
                .await;
            let _ = this.update(cx, |this, cx| {
                Self::handle_dictation_transcript(
                    this,
                    transcript_result,
                    audio_duration,
                    target,
                    cx,
                );
            });
        })
        .detach();
    }

    /// Periodically snapshot the live capture session and push state to the
    /// dictation overlay.  Runs every 16 ms (~60 fps) for smooth waveform
    /// animation and stops automatically when the session ends.
    fn spawn_dictation_overlay_pump(&mut self, cx: &mut Context<Self>) {
        let generation = crate::dictation::overlay_generation();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                // Bail if a newer overlay session has started.
                if crate::dictation::overlay_generation() != generation {
                    tracing::debug!(
                        category = "DICTATION",
                        "Overlay pump detected generation change, stopping"
                    );
                    break;
                }
                // Guard against runaway recordings: auto-stop and transcribe
                // once the configured max duration is reached.  The stop
                // coordinator dedupes repeat requests while the stop is in
                // flight.
                if crate::dictation::dictation_auto_stop_due() {
                    tracing::warn!(
                        category = "DICTATION",
                        "Dictation reached its configured max duration; auto-stopping"
                    );
                    let _ = this.update(cx, |this, cx| {
                        this.show_hud(
                            "Dictation reached its time limit — transcribing".to_string(),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                        let target = crate::dictation::get_dictation_target()
                            .unwrap_or_else(|| this.resolve_dictation_target());
                        this.request_active_dictation_stop(
                            crate::dictation::DictationStopReason::MaxDurationReached,
                            target,
                            true,
                            cx,
                        );
                    });
                }
                let Some(state) = crate::dictation::snapshot_overlay_state() else {
                    break;
                };
                cx.update(|cx| {
                    let _ = crate::dictation::update_dictation_overlay(state, cx);
                });
            }
        })
        .detach();
    }

    /// Handle the result of background transcription: deliver the transcript
    /// to the target surface that was active when dictation started, update
    /// the overlay, and schedule cleanup timers.
    fn handle_dictation_transcript(
        &mut self,
        result: anyhow::Result<Option<String>>,
        audio_duration: std::time::Duration,
        target: crate::dictation::DictationTarget,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(transcript)) => {
                let history_entry_id = if crate::config::load_user_preferences()
                    .dictation
                    .save_history_enabled()
                {
                    let history_entry = crate::dictation::record_dictation_history(
                        &transcript,
                        audio_duration,
                        target,
                    );
                    tracing::info!(
                        category = "DICTATION",
                        event = "dictation_history_recorded_before_delivery",
                        entry_id = %history_entry.id,
                        target = %history_entry.target,
                    );
                    history_entry.id
                } else {
                    tracing::info!(
                        category = "DICTATION",
                        event = "dictation_history_skipped_by_preference",
                        "dictation.saveHistory is disabled; transcript not persisted"
                    );
                    String::new()
                };
                // Show the recognized text while it is being delivered so the
                // user gets confirmation of what was heard.
                let _ = crate::dictation::update_dictation_overlay(
                    crate::dictation::DictationOverlayState {
                        phase: crate::dictation::DictationSessionPhase::Delivering,
                        elapsed: audio_duration,
                        transcript: transcript.clone().into(),
                        target,
                        ..Default::default()
                    },
                    cx,
                );
                // Route delivery based on the target that was captured at
                // session start, not the current UI state.
                let mut delivery_insertion_range: Option<serde_json::Value> = None;
                let delivered_internally = match target {
                    crate::dictation::DictationTarget::MainWindowFilter => {
                        if !self.can_accept_dictation_into_main_filter() {
                            self.reset_to_script_list(cx);
                        }
                        self.try_set_main_window_filter_from_dictation(transcript.clone(), cx)
                    }
                    crate::dictation::DictationTarget::MainWindowPrompt => {
                        self.try_set_prompt_input(transcript.clone(), cx)
                    }
                    crate::dictation::DictationTarget::NotesEditor => {
                        match notes::inject_text_into_notes(&mut **cx, &transcript) {
                            Ok(insertion_range) => {
                                delivery_insertion_range = Some(insertion_range);
                                true
                            }
                            Err(error) => {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error,
                                    "Notes delivery failed, falling back to frontmost app"
                                );
                                false
                            }
                        }
                    }
                    crate::dictation::DictationTarget::AiChatComposer => {
                        match ai::set_ai_input(&mut **cx, &transcript, false) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error,
                                    "AI chat delivery failed, falling back to frontmost app"
                                );
                                false
                            }
                        }
                    }
                    crate::dictation::DictationTarget::TabAiHarness => {
                        self.seed_agent_chat_dictation_return_origin();
                        // A detached chat window is an independent workspace:
                        // dictation reveals the embedded chat without
                        // destroying the user's detached conversation.
                        self.open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part(
                            Some(transcript.clone()),
                            cx,
                        );
                        // Let the orchestrator reveal the main window as Agent Chat
                        // chat and focus the composer after the view is
                        // seeded with the dictated prompt.
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::FinishDictation,
                            cx,
                        );
                        true
                    }
                    crate::dictation::DictationTarget::DayPageToday => {
                        // Day pages are line-oriented; collapse interior
                        // newlines so the capture stays one timestamped entry.
                        let capture_text =
                            transcript.split_whitespace().collect::<Vec<_>>().join(" ");
                        match crate::brain::substrate::BrainSubstrate::default_kit().append_to_day(
                            chrono::Utc::now(),
                            crate::brain::substrate::DayEntry::Capture { text: capture_text },
                        ) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error,
                                    "Day Page delivery failed, falling back to frontmost app"
                                );
                                false
                            }
                        }
                    }
                    crate::dictation::DictationTarget::QuickAiQuestion => {
                        let submit = crate::config::load_user_preferences()
                            .dictation
                            .quick_ai_answers();
                        let opened = if submit {
                            ai::open_mini_ai_window_from("dictation_quick_ai", &mut **cx)
                        } else {
                            ai::open_ai_window(&mut **cx)
                        };
                        match opened.map_err(|error| error.to_string()).and_then(|()| {
                            ai::set_ai_input(&mut **cx, &transcript, submit)
                        }) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error,
                                    submit,
                                    "Quick AI delivery failed, falling back to frontmost app"
                                );
                                false
                            }
                        }
                    }
                    crate::dictation::DictationTarget::ExternalApp => false,
                };

                if delivered_internally {
                    // Single source of truth for the target → destination
                    // mapping lives on DictationTarget::destination().
                    let destination = target.destination();
                    let insertion_range = match destination {
                        crate::dictation::DictationDestination::MainWindowFilter
                        | crate::dictation::DictationDestination::ActivePrompt
                        | crate::dictation::DictationDestination::AiChatComposer
                        | crate::dictation::DictationDestination::TabAiHarness
                        | crate::dictation::DictationDestination::QuickAiQuestion => {
                            Some(serde_json::json!({
                                "available": true,
                                "unit": "utf8Bytes",
                                "start": 0,
                                "end": transcript.len(),
                                "insertedLength": transcript.len(),
                                "operation": "replaceInput",
                                "source": "deliveryPipeline",
                                "redacted": true,
                            }))
                        }
                        crate::dictation::DictationDestination::NotesEditor => {
                            delivery_insertion_range
                        }
                        crate::dictation::DictationDestination::FrontmostApp
                        | crate::dictation::DictationDestination::DayPageToday => None,
                    };
                    tracing::info!(
                        category = "DICTATION",
                        ?target,
                        ?destination,
                        transcript_len = transcript.len(),
                        "Internal dictation delivery complete"
                    );
                    let _ = crate::dictation::record_delivery_receipt(
                        &transcript,
                        audio_duration,
                        target,
                        destination,
                        true,
                        &history_entry_id,
                        insertion_range,
                    );

                    // Brief "Done" confirmation with the delivered text, then
                    // close the overlay.
                    let _ = crate::dictation::update_dictation_overlay(
                        crate::dictation::DictationOverlayState {
                            phase: crate::dictation::DictationSessionPhase::Finished,
                            elapsed: audio_duration,
                            transcript: transcript.clone().into(),
                            target,
                            ..Default::default()
                        },
                        cx,
                    );
                    self.schedule_dictation_overlay_close(cx, Self::DICTATION_FINISHED_LINGER);
                    if matches!(target, crate::dictation::DictationTarget::MainWindowFilter)
                        && !script_kit_gpui::is_main_window_visible()
                    {
                        script_kit_gpui::set_main_window_visible(true);
                        crate::platform::ensure_main_panel_configured(
                            "builtin_execution::dictation_main_filter_delivery",
                        );
                        crate::platform::show_main_window_without_activation();
                    }
                    self.schedule_dictation_transcriber_cleanup(
                        cx,
                        std::time::Duration::from_secs(300),
                    );
                    // Notify orchestrator that dictation is complete.
                    // TabAiHarness dispatches this earlier (before overlay
                    // scheduling) to trigger immediate RevealMain; other
                    // targets dispatch here for state bookkeeping.
                    if !matches!(target, crate::dictation::DictationTarget::TabAiHarness) {
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::FinishDictation,
                            cx,
                        );
                    }
                } else {
                    // Guard: verify that a tracked external app target exists
                    // before attempting to paste to the frontmost app.
                    if let Err(error) = Self::ensure_dictation_frontmost_target_available() {
                        let error_text = error.to_string();
                        tracing::error!(
                            category = "DICTATION",
                            error = %error_text,
                            "Failed to resolve frontmost-app dictation target"
                        );
                        self.show_error_toast(format!("Dictation paste failed: {error_text}"), cx);
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(150),
                        );
                        self.schedule_dictation_transcriber_cleanup(
                            cx,
                            std::time::Duration::from_secs(300),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                        return;
                    }

                    let Some(target_app) = crate::frontmost_app_tracker::get_last_real_app() else {
                        tracing::error!(
                            category = "DICTATION",
                            "Frontmost-app dictation target disappeared before paste"
                        );
                        self.show_error_toast(
                            "Dictation paste failed: no tracked frontmost app is available"
                                .to_string(),
                            cx,
                        );
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(150),
                        );
                        self.schedule_dictation_transcriber_cleanup(
                            cx,
                            std::time::Duration::from_secs(300),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                        return;
                    };
                    let target_pid = target_app.pid;
                    let target_bundle_id = target_app.bundle_id;
                    let target_app_name = target_app.name;
                    tracing::info!(
                        category = "DICTATION",
                        target_bundle_id = %target_bundle_id,
                        target_pid,
                        target_app_name = %target_app_name,
                        transcript_len = transcript.len(),
                        "Preparing frontmost-app dictation paste"
                    );

                    cx.spawn(async move |this, cx| {
                        // Close overlay, hide Script Kit, and explicitly
                        // activate the tracked target app so macOS moves
                        // keyboard focus there before the CGEvent paste.
                        let yield_focus_result = match this.update(
                            cx,
                            |this, cx| {
                                this.yield_focus_for_dictation_paste(
                                    target_pid,
                                    &target_bundle_id,
                                    &target_app_name,
                                    cx,
                                )
                            },
                        ) {
                            Ok(result) => result,
                            Err(error) => Err(anyhow::anyhow!(
                                "failed to update app state before paste: {error}"
                            )),
                        };

                        if let Err(error) = yield_focus_result {
                            let error_text = error.to_string();
                            if this.update(cx, |this, cx| {
                                tracing::error!(
                                    category = "DICTATION",
                                    error = %error_text,
                                    "Failed to yield focus before dictation paste"
                                );
                                this.show_error_toast(
                                    format!(
                                        "Dictation paste failed before paste step: {error_text}"
                                    ),
                                    cx,
                                );
                                this.schedule_dictation_transcriber_cleanup(
                                    cx,
                                    std::time::Duration::from_secs(300),
                                );
                            }).is_err() {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error_text,
                                    "Yield-focus failure could not be surfaced (entity released)"
                                );
                            }
                            return;
                        }

                        // Let macOS settle focus back to the target app.
                        cx.background_executor()
                            .timer(Self::dictation_focus_settle_duration())
                            .await;

                        tracing::info!(
                            category = "DICTATION",
                            target_bundle_id = %target_bundle_id,
                            transcript_len = transcript.len(),
                            "Focus yielded to target app; pasting transcript"
                        );

                        let paste_result = cx
                            .background_executor()
                            .spawn({
                                let transcript = transcript.clone();
                                async move {
                                    crate::text_injector::TextInjector::new()
                                        .paste_text(&transcript)
                                }
                            })
                            .await;

                        if this.update(cx, |this, cx| {
                            match paste_result {
                                Ok(()) => {
                                    let _ = crate::dictation::record_delivery_receipt(
                                        &transcript,
                                        audio_duration,
                                        target,
                                        crate::dictation::DictationDestination::FrontmostApp,
                                        false,
                                        &history_entry_id,
                                        None,
                                    );
                                    tracing::info!(
                                        category = "DICTATION",
                                        destination = ?crate::dictation::DictationDestination::FrontmostApp,
                                        transcript_len = transcript.len(),
                                        "Transcript delivered"
                                    );
                                }
                                Err(ref error) => {
                                    tracing::error!(
                                        category = "DICTATION",
                                        error = %error,
                                        "Failed to paste dictation transcript"
                                    );
                                    // Recovery: leave the transcript on the
                                    // clipboard so the dictation isn't lost.
                                    let copied = arboard::Clipboard::new()
                                        .and_then(|mut clipboard| {
                                            clipboard.set_text(transcript.clone())
                                        })
                                        .is_ok();
                                    let message = if copied {
                                        format!(
                                            "Dictation paste failed — transcript copied to clipboard, press \u{2318}V to paste ({error})"
                                        )
                                    } else {
                                        format!("Dictation paste failed: {error}")
                                    };
                                    this.show_error_toast(message, cx);
                                }
                            }
                            this.schedule_dictation_transcriber_cleanup(
                                cx,
                                std::time::Duration::from_secs(300),
                            );
                        }).is_err() {
                            tracing::warn!(
                                category = "DICTATION",
                                transcript_len = transcript.len(),
                                "Paste result could not be reported (entity released)"
                            );
                        }
                    })
                    .detach();
                }
            }
            Ok(None) => {
                // No speech detected — close overlay quietly and treat the
                // session as an abort. For Agent Chat dictation, a successful finish
                // means "reveal Agent Chat with a transcript"; without transcript
                // there is nothing to seed or submit.
                tracing::info!(
                    category = "DICTATION",
                    ?target,
                    "No dictation transcript recognized; aborting delivery"
                );
                self.schedule_dictation_overlay_close(cx, std::time::Duration::from_millis(150));
                self.schedule_dictation_transcriber_cleanup(
                    cx,
                    std::time::Duration::from_secs(300),
                );
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::AbortDictation,
                    cx,
                );
            }
            Err(error) => {
                let error_text = error.to_string();
                let model_path = crate::dictation::resolve_default_model_path();
                tracing::error!(
                    category = "DICTATION",
                    error = %error_text,
                    model_path = %model_path.display(),
                    "Transcription failed"
                );

                if error_text.contains("model not downloaded") {
                    let _ = crate::dictation::close_dictation_overlay(cx);
                    self.dispatch_window_event(
                        crate::window_orchestrator::WindowEvent::AbortDictation,
                        cx,
                    );
                    self.open_dictation_model_prompt(cx);
                    self.schedule_dictation_transcriber_cleanup(
                        cx,
                        std::time::Duration::from_secs(300),
                    );
                    return;
                } else {
                    self.show_error_toast(
                        format!("Dictation transcription failed: {error_text}"),
                        cx,
                    );
                }

                let _ = crate::dictation::update_dictation_overlay(
                    crate::dictation::DictationOverlayState {
                        phase: crate::dictation::DictationSessionPhase::Failed(error_text),
                        elapsed: audio_duration,
                        ..Default::default()
                    },
                    cx,
                );
                self.schedule_dictation_overlay_close(cx, std::time::Duration::from_millis(800));
                self.schedule_dictation_transcriber_cleanup(
                    cx,
                    std::time::Duration::from_secs(300),
                );
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::AbortDictation,
                    cx,
                );
            }
        }
    }

    pub(crate) fn deliver_stdin_dictation_result(
        &mut self,
        transcript: String,
        partial_transcript: Option<&str>,
        target_label: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<crate::dictation::DictationTarget, String> {
        crate::dictation::abort_dictation()
            .map_err(|error| format!("failed to stop active dictation capture: {error}"))?;

        let resolution =
            crate::dictation::resolve_final_or_partial_transcript(&transcript, partial_transcript);
        if resolution.used_partial_fallback {
            tracing::info!(
                category = "DICTATION",
                event = "dictation_partial_transcript_fallback_used",
                final_transcript_len = resolution.final_len,
                partial_transcript_len = ?resolution.partial_len,
                "Using partial dictation transcript because final transcript was empty"
            );
        }
        let transcript_len = resolution.transcript.as_ref().map(String::len);
        let delivery_generation_before = crate::dictation::delivery_receipt_generation();
        let active_session_target = crate::dictation::get_dictation_target();
        let ui_fallback_target = self.resolve_dictation_target();
        let target_resolution = crate::dictation::resolve_delivery_target_request(
            target_label,
            active_session_target,
            ui_fallback_target,
            delivery_generation_before,
        );
        let (target, target_source) = match target_resolution {
            crate::dictation::DictationDeliveryTargetResolution::Deliver { target, source } => {
                (target, source)
            }
            crate::dictation::DictationDeliveryTargetResolution::Refuse(draft) => {
                let receipt = crate::dictation::record_wrong_target_refusal(draft, transcript_len);
                tracing::warn!(
                    category = "DICTATION",
                    event = "push_dictation_result_refused",
                    reason_code = %receipt
                        .get("reasonCode")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown"),
                    requested_target_label_len = ?receipt.get("requestedTargetLabelLen"),
                    transcript_len = ?transcript_len,
                    "Refusing pushDictationResult for an invalid explicit target"
                );
                return Err("dictation delivery target refused".to_string());
            }
        };
        if matches!(
            target_source,
            crate::dictation::DictationDeliveryTargetSource::ExplicitLabel
        ) {
            if let Err(error) = self.ensure_dictation_delivery_target_available_for(target) {
                let draft = crate::dictation::DictationWrongTargetRefusalDraft {
                    reason: crate::dictation::DictationWrongTargetReason::TargetUnavailable,
                    requested_target_label: target_label.map(str::to_owned),
                    requested_target: Some(target),
                    fallback_target: None,
                    delivery_generation_before,
                };
                let receipt = crate::dictation::record_wrong_target_refusal(draft, transcript_len);
                tracing::warn!(
                    category = "DICTATION",
                    event = "push_dictation_result_refused",
                    reason_code = %receipt
                        .get("reasonCode")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown"),
                    requested_target = ?target,
                    requested_target_label_len = ?receipt.get("requestedTargetLabelLen"),
                    transcript_len = ?transcript_len,
                    error = %error,
                    "Refusing pushDictationResult for an unavailable explicit target"
                );
                return Err("dictation delivery target unavailable".to_string());
            }
        }

        let result = Ok(resolution.transcript);
        self.handle_dictation_transcript(result, std::time::Duration::ZERO, target, cx);
        Ok(target)
    }

    fn dictation_target_from_stdin_label(
        target_label: Option<&str>,
    ) -> Option<crate::dictation::DictationTarget> {
        target_label.and_then(crate::dictation::parse_dictation_target_label)
    }

    const DICTATION_FOCUS_SETTLE_MS: u64 = 120;

    /// How long the stop coordinator waits for the capture pipeline to flush
    /// its tail chunk and emit `EndOfStream`.  Hitting this deadline truncates
    /// the recording tail, so it is generous — the flush normally completes in
    /// well under a second.
    const DICTATION_STOP_COLLECT_DEADLINE: std::time::Duration =
        std::time::Duration::from_secs(5);

    /// How long the Finished pill stays visible before the overlay closes.
    const DICTATION_FINISHED_LINGER: std::time::Duration =
        std::time::Duration::from_millis(900);

    fn dictation_focus_settle_duration() -> std::time::Duration {
        std::time::Duration::from_millis(Self::DICTATION_FOCUS_SETTLE_MS)
    }

    /// Start downloading the selected dictation model (Parakeet archive or
    /// Whisper file) in the background, showing progress via in-prompt
    /// updates and HUD fallback.
    fn start_dictation_model_download(&mut self, cx: &mut Context<Self>) {
        if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_err()
        {
            self.show_hud(
                "Dictation model download already in progress".to_string(),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            return;
        }

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *parakeet_model_download_cancel_slot().lock() = Some(cancel.clone());
        // Shallow channel — cosmetic updates use try_send so the download
        // thread is never blocked on UI repaints.
        let (progress_tx, progress_rx) = async_channel::bounded::<DictationModelProgressEvent>(4);
        let ui_emitter =
            std::sync::Arc::new(parking_lot::Mutex::new(DictationModelUiEmitter::default()));

        // Spawn a concurrent reader that updates the in-prompt progress
        // display as events arrive.  HUD only shows when the rich prompt
        // is not visible — they no longer compete.
        cx.spawn({
            let progress_rx = progress_rx.clone();
            async move |this, cx| {
                while let Ok(event) = progress_rx.recv().await {
                    let _ = this.update(cx, |this, cx| match event {
                        DictationModelProgressEvent::Downloading {
                            percentage,
                            downloaded_bytes,
                            total_bytes,
                            speed_bytes_per_sec,
                            eta_seconds,
                        } => {
                            let prompt_visible = this.is_dictation_model_prompt_visible();
                            this.update_dictation_model_prompt_if_visible(
                                crate::dictation::DictationModelStatus::Downloading {
                                    percentage,
                                    downloaded_bytes,
                                    total_bytes,
                                    speed_bytes_per_sec,
                                    eta_seconds,
                                },
                                cx,
                            );
                            if !prompt_visible {
                                let summary = crate::dictation::download::format_progress_summary(
                                    percentage,
                                    downloaded_bytes,
                                    total_bytes,
                                    speed_bytes_per_sec,
                                    eta_seconds,
                                );
                                this.show_hud(
                                    format!("Downloading model\u{2026} {summary}"),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                            }
                        }
                        DictationModelProgressEvent::Extracting => {
                            let prompt_visible = this.is_dictation_model_prompt_visible();
                            this.update_dictation_model_prompt_if_visible(
                                crate::dictation::DictationModelStatus::Extracting,
                                cx,
                            );
                            if !prompt_visible {
                                this.show_hud(
                                    "Extracting dictation model\u{2026}".to_string(),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                            }
                        }
                    });
                }
            }
        })
        .detach();

        // The runner honors the model selected in preferences: the Parakeet
        // archive or the Whisper model file.
        let engine_kind = {
            let prefs = crate::config::load_user_preferences();
            let model_id = crate::dictation::DictationModelId::from_preference(
                prefs.dictation.model.as_deref(),
            );
            crate::dictation::dictation_model_entry(model_id).engine_kind
        };

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let cancel = cancel.clone();
                    async move {
                        let speed_tracker =
                            std::sync::Arc::new(parking_lot::Mutex::new(SpeedTracker::new()));
                        let ui_emitter = ui_emitter.clone();
                        let progress_callback =
                            {
                                let speed_tracker = speed_tracker.clone();
                                let ui_emitter = ui_emitter.clone();
                                let progress_tx = progress_tx;
                                move |phase: crate::dictation::download::DownloadPhase,
                                      progress: crate::dictation::download::DownloadProgress| {
                                    match phase {
                                        crate::dictation::download::DownloadPhase::Downloading => {
                                            let pct = progress.percentage();
                                            let speed = {
                                                let mut tracker = speed_tracker.lock();
                                                tracker.update(progress.downloaded);
                                                tracker.speed_bytes_per_sec()
                                            };
                                            let eta =
                                                crate::dictation::download::estimate_eta_seconds(
                                                    progress, speed,
                                                );

                                            let snapshot =
                                                DictationModelUiSnapshot::downloading(pct, eta);
                                            let now = std::time::Instant::now();
                                            let should_emit = {
                                                let emitter = ui_emitter.lock();
                                                emitter.should_emit(now, &snapshot)
                                            };

                                            if should_emit {
                                                let sent = progress_tx
                                                    .try_send(
                                                        DictationModelProgressEvent::Downloading {
                                                            percentage: pct,
                                                            downloaded_bytes: progress.downloaded,
                                                            total_bytes: progress.total,
                                                            speed_bytes_per_sec: speed,
                                                            eta_seconds: eta,
                                                        },
                                                    )
                                                    .is_ok();
                                                if sent {
                                                    tracing::info!(
                                                        category = "DICTATION",
                                                        pct,
                                                        downloaded = progress.downloaded,
                                                        total = progress.total,
                                                        speed,
                                                        "Model download progress"
                                                    );
                                                    let mut emitter = ui_emitter.lock();
                                                    emitter.record_emit(now, &snapshot);
                                                }
                                            }
                                        }
                                        crate::dictation::download::DownloadPhase::Extracting => {
                                            tracing::info!(
                                                category = "DICTATION",
                                                "Extracting dictation model"
                                            );
                                            let snapshot = DictationModelUiSnapshot::extracting();
                                            let now = std::time::Instant::now();
                                            // Extracting is critical — use blocking send
                                            // so it always reaches the UI.
                                            if progress_tx
                                                .send_blocking(
                                                    DictationModelProgressEvent::Extracting,
                                                )
                                                .is_ok()
                                            {
                                                let mut emitter = ui_emitter.lock();
                                                emitter.record_emit(now, &snapshot);
                                            }
                                        }
                                        crate::dictation::download::DownloadPhase::Failed(_)
                                        | crate::dictation::download::DownloadPhase::Cancelled
                                        | crate::dictation::download::DownloadPhase::Complete => {}
                                    }
                                }
                            };
                        match engine_kind {
                            crate::dictation::DictationEngineKind::Parakeet => {
                                crate::dictation::download::download_parakeet_model(
                                    progress_callback,
                                    cancel,
                                )
                            }
                            crate::dictation::DictationEngineKind::Whisper => {
                                crate::dictation::download::download_whisper_model(
                                    progress_callback,
                                    cancel,
                                )
                            }
                        }
                    }
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                *parakeet_model_download_cancel_slot().lock() = None;
                PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
                    .store(false, std::sync::atomic::Ordering::Release);
                match result {
                    Ok(_path) => {
                        tracing::info!(
                            category = "DICTATION",
                            "Dictation model download complete"
                        );
                        this.update_dictation_model_prompt_if_visible(
                            crate::dictation::DictationModelStatus::Available,
                            cx,
                        );
                        let pending = pending_dictation_model_action().lock().clone();
                        let message = pending
                            .map(|action| {
                                format!(
                                    "Dictation model ready — choose Start to continue to {}",
                                    action.pending_target_label()
                                )
                            })
                            .unwrap_or_else(|| {
                                "Dictation model ready — press hotkey to dictate".to_string()
                            });
                        this.show_hud(message, Some(HUD_MEDIUM_MS), cx);
                    }
                    Err(error) if error.to_string().contains("cancelled") => {
                        let cancelled = "model download cancelled".to_string();
                        tracing::info!(category = "DICTATION", "Dictation model download cancelled");
                        this.update_dictation_model_prompt_if_visible(
                            crate::dictation::DictationModelStatus::DownloadFailed(cancelled),
                            cx,
                        );
                        this.show_hud(
                            "Dictation model download cancelled".to_string(),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                    }
                    Err(error) => {
                        let raw_error = error.to_string();
                        let error_text =
                            crate::dictation::download::classify_download_error(&error);
                        tracing::error!(
                            category = "DICTATION",
                            error = %raw_error,
                            user_error = %error_text,
                            "Dictation model download failed"
                        );
                        this.update_dictation_model_prompt_if_visible(
                            crate::dictation::DictationModelStatus::DownloadFailed(
                                error_text.clone(),
                            ),
                            cx,
                        );
                        this.show_error_toast(
                            format!("Dictation model download failed: {error_text}"),
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }

    /// Build the dictation model prompt for the model selected in preferences.
    /// Downloading copy includes "Cancel download" and explains that retry resumes from the partial file.
    fn build_dictation_model_prompt(
        status: crate::dictation::DictationModelStatus,
    ) -> (String, String, Vec<Choice>) {
        // Downloading state uses format_progress_summary, offers "Cancel download", and retry resumes from the partial file.
        // Cancelled copy says: Partial download kept. Retry resumes from where you stopped.
        let prefs = crate::config::load_user_preferences();
        let model_id =
            crate::dictation::DictationModelId::from_preference(prefs.dictation.model.as_deref());
        let partial_size = crate::dictation::dictation_model_entry(model_id)
            .partial_download_size_bytes();
        Self::build_dictation_model_prompt_for_model(status, model_id, partial_size)
    }

    /// Build the title, placeholder, and choices for the dictation model
    /// prompt based on the current `DictationModelStatus`.  Pure function
    /// with no side effects — suitable for unit testing.
    ///
    /// Both catalog models are downloadable in-app; a missing non-recommended
    /// model additionally offers switching back to the recommended Parakeet.
    fn build_dictation_model_prompt_for_model(
        status: crate::dictation::DictationModelStatus,
        model_id: crate::dictation::DictationModelId,
        partial_archive_size: Option<u64>,
    ) -> (String, String, Vec<Choice>) {
        use crate::dictation::DictationModelStatus;

        let model = crate::dictation::dictation_model_entry(model_id);
        let parakeet = crate::dictation::dictation_model_entry(
            crate::dictation::DictationModelId::ParakeetTdt06bV3,
        );
        let model_marker = if model.recommended {
            format!("{} · Recommended", model.display_name)
        } else {
            model.display_name.to_string()
        };
        let downloaded_size = model
            .downloaded_size_label()
            .map(|size| format!(" · {size} on disk"))
            .unwrap_or_default();
        let archive_size_bytes = model.download_size_bytes();
        let archive_size = crate::dictation::download::format_bytes(archive_size_bytes);
        let model_summary = format!("{model_marker}{downloaded_size} · {}", model.description);

        match status {
            DictationModelStatus::NotDownloaded => {
                let resume_pct = partial_archive_size.filter(|size| *size > 0).map(|size| {
                    ((size.saturating_mul(100)) / archive_size_bytes.max(1)).min(99)
                });
                let title = resume_pct
                    .map(|pct| format!("Resume download ({}% already downloaded)", pct))
                    .unwrap_or_else(|| format!("Download {}", model.display_name));
                let choice_name = resume_pct
                    .map(|pct| format!("Resume download ({}% already downloaded)", pct))
                    .unwrap_or_else(|| format!("Download {} ({archive_size})", model.display_name));
                let placeholder = resume_pct
                    .map(|pct| format!("{model_summary} \u{00b7} {archive_size} download \u{00b7} local after install \u{00b7} resumes from {pct}%"))
                    .unwrap_or_else(|| format!("{model_summary} \u{00b7} {archive_size} download \u{00b7} local after install \u{00b7} resumable if interrupted"));
                let mut choices = vec![Choice {
                    name: choice_name,
                    value: BUILTIN_DICTATION_MODEL_DOWNLOAD.to_string(),
                    description: Some("Required for local dictation".to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_MODEL_PROMPT_ID,
                        0,
                        BUILTIN_DICTATION_MODEL_DOWNLOAD,
                    )),
                }];
                if !model.recommended {
                    let parakeet_size = crate::dictation::download::format_bytes(
                        parakeet.download_size_bytes(),
                    );
                    choices.push(Choice {
                        name: format!("Switch to {}", parakeet.display_name),
                        value: BUILTIN_DICTATION_MODEL_USE_RECOMMENDED.to_string(),
                        description: Some(format!(
                            "Recommended \u{00b7} downloads if needed ({parakeet_size})"
                        )),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID,
                            choices.len(),
                            BUILTIN_DICTATION_MODEL_USE_RECOMMENDED,
                        )),
                    });
                }
                let cancel_index = choices.len();
                choices.push(Choice {
                    name: "Not now".to_string(),
                    value: BUILTIN_DICTATION_MODEL_CANCEL.to_string(),
                    description: Some("Leave dictation unchanged".to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_MODEL_PROMPT_ID,
                        cancel_index,
                        BUILTIN_DICTATION_MODEL_CANCEL,
                    )),
                });
                (title, placeholder, choices)
            }
            DictationModelStatus::Downloading {
                percentage,
                downloaded_bytes,
                total_bytes,
                speed_bytes_per_sec,
                eta_seconds,
            } => {
                let summary = crate::dictation::download::format_progress_summary(
                    percentage,
                    downloaded_bytes,
                    total_bytes,
                    speed_bytes_per_sec,
                    eta_seconds,
                );
                (
                    format!("Downloading local dictation model\u{2026} {percentage}%"),
                    summary,
                    vec![
                        Choice {
                            name: "Cancel download".to_string(),
                            value: BUILTIN_DICTATION_MODEL_CANCEL.to_string(),
                            description: Some(
                                "Stop now; retry resumes from the partial file".to_string(),
                            ),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID,
                                0,
                                BUILTIN_DICTATION_MODEL_CANCEL,
                            )),
                        },
                        Choice {
                            name: "Hide".to_string(),
                            value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                            description: Some("Download continues in background".to_string()),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID,
                                1,
                                BUILTIN_DICTATION_MODEL_HIDE,
                            )),
                        },
                    ],
                )
            }
            DictationModelStatus::Extracting => (
                "Installing local dictation model\u{2026}".to_string(),
                "Download finished. Installing model files locally.".to_string(),
                vec![Choice {
                    name: "Hide".to_string(),
                    value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                    description: Some("Extraction continues in background".to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_MODEL_PROMPT_ID,
                        0,
                        BUILTIN_DICTATION_MODEL_HIDE,
                    )),
                }],
            ),
            DictationModelStatus::DownloadFailed(ref error)
                if error.to_ascii_lowercase().contains("cancelled") =>
            {
                (
                    "Download cancelled".to_string(),
                    "Partial download kept. Retry resumes from where you stopped.".to_string(),
                    vec![
                        Choice {
                            name: "Retry download".to_string(),
                            value: BUILTIN_DICTATION_MODEL_DOWNLOAD.to_string(),
                            description: Some(format!(
                                "Resume the {} download",
                                model.display_name
                            )),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID,
                                0,
                                BUILTIN_DICTATION_MODEL_DOWNLOAD,
                            )),
                        },
                        Choice {
                            name: "Done".to_string(),
                            value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                            description: Some("Close this prompt".to_string()),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID,
                                1,
                                BUILTIN_DICTATION_MODEL_HIDE,
                            )),
                        },
                    ],
                )
            }
            DictationModelStatus::DownloadFailed(error) => (
                "Dictation model download failed".to_string(),
                error,
                vec![
                    Choice {
                        name: "Retry download".to_string(),
                        value: BUILTIN_DICTATION_MODEL_DOWNLOAD.to_string(),
                        description: Some(format!(
                            "Try the {} download again",
                            model.display_name
                        )),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID,
                            0,
                            BUILTIN_DICTATION_MODEL_DOWNLOAD,
                        )),
                    },
                    Choice {
                        name: "Not now".to_string(),
                        value: BUILTIN_DICTATION_MODEL_CANCEL.to_string(),
                        description: Some("Leave dictation unchanged".to_string()),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID,
                            1,
                            BUILTIN_DICTATION_MODEL_CANCEL,
                        )),
                    },
                ],
            ),
            DictationModelStatus::Available => {
                let pending = pending_dictation_model_action().lock().clone();
                let mut choices = Vec::new();
                if let Some(action) = pending {
                    choices.push(Choice {
                        name: format!("Start Dictation to {}", action.pending_target_label()),
                        value: BUILTIN_DICTATION_MODEL_START_PENDING.to_string(),
                        description: Some("Continue the action that needed the model".to_string()),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID,
                            0,
                            BUILTIN_DICTATION_MODEL_START_PENDING,
                        )),
                    });
                }
                let index = choices.len();
                choices.push(Choice {
                    name: "Done".to_string(),
                    value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                    description: Some("Close this prompt".to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_MODEL_PROMPT_ID,
                        index,
                        BUILTIN_DICTATION_MODEL_HIDE,
                    )),
                });
                (
                    format!("{} ready", model.display_name),
                    format!(
                        "{model_summary}. Start dictation from the launcher or the configured hotkey (default: ⌘⇧;)."
                    ),
                    choices,
                )
            }
        }
    }

    fn preferred_dictation_model_prompt_index(
        status: &crate::dictation::DictationModelStatus,
    ) -> usize {
        match status {
            crate::dictation::DictationModelStatus::Downloading { .. } => 1,
            _ => 0,
        }
    }

    /// Render the dictation model prompt with the given status, replacing
    /// whatever is currently on screen.
    fn render_dictation_model_prompt(
        &mut self,
        status: crate::dictation::DictationModelStatus,
        cx: &mut Context<Self>,
    ) {
        let previous_status = dictation_model_prompt_status().lock().clone();
        let phase_changed =
            std::mem::discriminant(&previous_status) != std::mem::discriminant(&status);
        let prompt_visible = self.is_dictation_model_prompt_visible();
        *dictation_model_prompt_status().lock() = status.clone();
        let (title, placeholder, choices) = Self::build_dictation_model_prompt(status.clone());
        if !prompt_visible || phase_changed {
            self.arg_selected_index = Self::preferred_dictation_model_prompt_index(&status)
                .min(choices.len().saturating_sub(1));
        }
        self.open_builtin_filterable_view(
            AppView::MiniPrompt {
                id: BUILTIN_DICTATION_MODEL_PROMPT_ID.to_string(),
                placeholder,
                choices,
            },
            &title,
            false,
            cx,
        );
    }

    /// Returns `true` when the dictation model prompt is currently on-screen.
    fn is_dictation_model_prompt_visible(&self) -> bool {
        matches!(
            &self.current_view,
            AppView::MiniPrompt { id, .. } if id == BUILTIN_DICTATION_MODEL_PROMPT_ID
        )
    }

    /// If the dictation model prompt is currently visible, update it in-place
    /// with the new status.  Otherwise this is a no-op.
    fn update_dictation_model_prompt_if_visible(
        &mut self,
        status: crate::dictation::DictationModelStatus,
        cx: &mut Context<Self>,
    ) {
        // Always persist the latest status so reopening a hidden prompt
        // shows the current state instead of stale progress.
        *dictation_model_prompt_status().lock() = status.clone();

        let is_visible = matches!(
            &self.current_view,
            AppView::MiniPrompt { id, .. } if id == BUILTIN_DICTATION_MODEL_PROMPT_ID
        );
        if is_visible {
            self.render_dictation_model_prompt(status, cx);
        }
    }

    /// Open the dictation model prompt in its initial `NotDownloaded` state.
    pub(crate) fn open_dictation_model_prompt(&mut self, cx: &mut Context<Self>) {
        let prefs = crate::config::load_user_preferences();
        let model_id =
            crate::dictation::DictationModelId::from_preference(prefs.dictation.model.as_deref());
        let model = crate::dictation::dictation_model_entry(model_id);
        let status = if model.is_available() {
            crate::dictation::DictationModelStatus::Available
        } else {
            dictation_model_prompt_status().lock().clone()
        };
        self.render_dictation_model_prompt(status, cx);
    }

    /// Handle a selection from the dictation model download prompt.
    fn handle_dictation_model_selection(&mut self, value: &str, cx: &mut Context<Self>) {
        match value {
            BUILTIN_DICTATION_MODEL_DOWNLOAD => {
                let prefs = crate::config::load_user_preferences();
                let model_id = crate::dictation::DictationModelId::from_preference(
                    prefs.dictation.model.as_deref(),
                );
                let model = crate::dictation::dictation_model_entry(model_id);
                tracing::info!(
                    category = "DICTATION",
                    model_id = model.stable_id,
                    "User accepted dictation model download"
                );
                // Transition the prompt to downloading state instead of closing it.
                self.render_dictation_model_prompt(
                    crate::dictation::DictationModelStatus::Downloading {
                        percentage: 0,
                        downloaded_bytes: 0,
                        total_bytes: model.download_size_bytes(),
                        speed_bytes_per_sec: 0,
                        eta_seconds: None,
                    },
                    cx,
                );
                self.start_dictation_model_download(cx);
            }
            BUILTIN_DICTATION_MODEL_USE_RECOMMENDED => {
                tracing::info!(
                    category = "DICTATION",
                    "User switched dictation model preference back to the recommended default"
                );
                if let Err(error) = crate::dictation::save_dictation_model(None) {
                    tracing::warn!(
                        category = "DICTATION",
                        error = %error,
                        "Failed to save dictation model preference"
                    );
                }
                let parakeet = crate::dictation::dictation_model_entry(
                    crate::dictation::DictationModelId::ParakeetTdt06bV3,
                );
                if parakeet.is_available() {
                    self.render_dictation_model_prompt(
                        crate::dictation::DictationModelStatus::Available,
                        cx,
                    );
                } else {
                    self.render_dictation_model_prompt(
                        crate::dictation::DictationModelStatus::Downloading {
                            percentage: 0,
                            downloaded_bytes: 0,
                            total_bytes: parakeet.download_size_bytes(),
                            speed_bytes_per_sec: 0,
                            eta_seconds: None,
                        },
                        cx,
                    );
                    self.start_dictation_model_download(cx);
                }
            }
            BUILTIN_DICTATION_MODEL_CANCEL => {
                if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS.load(std::sync::atomic::Ordering::Acquire) {
                    tracing::info!(
                        category = "DICTATION",
                        "User requested dictation model download cancellation"
                    );
                    if let Some(cancel) = parakeet_model_download_cancel_slot().lock().clone() {
                        cancel.store(true, std::sync::atomic::Ordering::Release);
                    }
                    self.show_hud(
                        "Cancelling dictation model download\u{2026}".to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    );
                } else {
                    tracing::info!(
                        category = "DICTATION",
                        "User declined dictation model download"
                    );
                    *pending_dictation_model_action().lock() = None;
                    self.reset_to_script_list(cx);
                }
            }
            BUILTIN_DICTATION_MODEL_HIDE => {
                tracing::info!(category = "DICTATION", "User hid dictation model prompt");
                // Dismissing a completed download ("Done") abandons the pending
                // dictation intent; hiding an in-flight download keeps it so
                // the completion prompt can still offer to continue.
                if matches!(
                    *dictation_model_prompt_status().lock(),
                    crate::dictation::DictationModelStatus::Available
                ) {
                    *pending_dictation_model_action().lock() = None;
                }
                self.reset_to_script_list(cx);
            }
            BUILTIN_DICTATION_MODEL_START_PENDING => {
                if let Some(action) = pending_dictation_model_action().lock().take() {
                    let dctx =
                        crate::action_helpers::DispatchContext::for_builtin(action.builtin_id());
                    let _ = self.execute_dictation_builtin_action(action, &dctx, cx);
                } else {
                    self.show_hud(
                        "No pending dictation action".to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    );
                    self.reset_to_script_list(cx);
                }
            }
            _ => {
                tracing::info!(
                    category = "DICTATION",
                    "User declined dictation model download"
                );
                self.reset_to_script_list(cx);
            }
        }
    }

    fn current_dictation_setup_state(&self) -> crate::dictation::DictationSetupState {
        let permission = crate::dictation::microphone_permission_status();
        let prefs = crate::config::load_user_preferences();
        let config = crate::config::load_config();
        let selected_device_id = prefs.dictation.selected_device_id.as_deref();
        let devices = match permission {
            crate::dictation::DictationMicrophonePermissionStatus::Granted
            | crate::dictation::DictationMicrophonePermissionStatus::Unknown => {
                crate::dictation::list_input_devices().map_err(|error| error.to_string())
            }
            crate::dictation::DictationMicrophonePermissionStatus::Denied
            | crate::dictation::DictationMicrophonePermissionStatus::NotDetermined => {
                Ok(Vec::new())
            }
        };
        let model_id =
            crate::dictation::DictationModelId::from_preference(prefs.dictation.model.as_deref());
        let model = crate::dictation::dictation_model_entry(model_id);
        let model_status = if model.is_available() {
            crate::dictation::DictationModelStatus::Available
        } else {
            dictation_model_prompt_status().lock().clone()
        };
        crate::dictation::build_dictation_setup_state(
            model_status,
            permission,
            devices,
            selected_device_id,
            config.get_dictation_hotkey().as_ref(),
            config.is_dictation_hotkey_enabled(),
        )
    }

    fn build_dictation_setup_prompt(
        state: &crate::dictation::DictationSetupState,
        model_id: crate::dictation::DictationModelId,
    ) -> (String, String, Vec<Choice>) {
        let model = crate::dictation::dictation_model_entry(model_id);
        let archive_size =
            crate::dictation::download::format_bytes(crate::dictation::PARAKEET_MODEL_ARCHIVE_SIZE);
        let mut choices = Vec::new();
        let title = if state.ready {
            "Dictation ready".to_string()
        } else {
            "Finish dictation setup".to_string()
        };
        let placeholder = "Check dictation model, microphone, and hotkey readiness".to_string();

        if state.ready {
            choices.push(Choice {
                name: "Start dictation".to_string(),
                value: BUILTIN_DICTATION_SETUP_START.to_string(),
                description: Some("Begin recording now".to_string()),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    BUILTIN_DICTATION_SETUP_START,
                )),
            });
        }

        match &state.model_status {
            crate::dictation::DictationModelStatus::Available => {
                let downloaded_size = model
                    .downloaded_size_label()
                    .map(|size| format!(" · {size} on disk"))
                    .unwrap_or_default();
                choices.push(Choice {
                    name: format!("Model ready: {}", model.display_name),
                    value: "model-ready".to_string(),
                    description: Some(format!("{}{}", model.description, downloaded_size)),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_SETUP_PROMPT_ID,
                        choices.len(),
                        "model-ready",
                    )),
                });
            }
            _ => {
                choices.push(Choice {
                    name: format!("Download {} ({archive_size})", model.display_name),
                    value: BUILTIN_DICTATION_SETUP_DOWNLOAD_MODEL.to_string(),
                    description: Some(model.description.to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_SETUP_PROMPT_ID,
                        choices.len(),
                        BUILTIN_DICTATION_SETUP_DOWNLOAD_MODEL,
                    )),
                });
            }
        }

        match &state.microphone_status {
            crate::dictation::DictationMicrophoneStatus::Ready {
                name,
                using_system_default,
            } => {
                let source = if *using_system_default {
                    "System Default"
                } else {
                    "Selected device"
                };
                choices.push(Choice {
                    name: format!("Microphone ready: {name}"),
                    value: "microphone-ready".to_string(),
                    description: Some(source.to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_SETUP_PROMPT_ID,
                        choices.len(),
                        "microphone-ready",
                    )),
                });
            }
            crate::dictation::DictationMicrophoneStatus::PermissionNeeded(
                crate::dictation::DictationMicrophonePermissionStatus::NotDetermined,
            ) => choices.push(Choice {
                name: "Request microphone access".to_string(),
                value: BUILTIN_DICTATION_SETUP_REQUEST_MIC.to_string(),
                description: Some("macOS will ask for permission".to_string()),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    BUILTIN_DICTATION_SETUP_REQUEST_MIC,
                )),
            }),
            crate::dictation::DictationMicrophoneStatus::PermissionNeeded(
                crate::dictation::DictationMicrophonePermissionStatus::Denied,
            ) => choices.push(Choice {
                name: "Open System Settings".to_string(),
                value: BUILTIN_DICTATION_SETUP_OPEN_MIC_SETTINGS.to_string(),
                description: Some(
                    "Enable Script Kit in Privacy & Security → Microphone".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    BUILTIN_DICTATION_SETUP_OPEN_MIC_SETTINGS,
                )),
            }),
            crate::dictation::DictationMicrophoneStatus::NoDevices => choices.push(Choice {
                name: "Microphone not found".to_string(),
                value: "microphone-no-devices".to_string(),
                description: Some(
                    "Connect an input device or check macOS Sound settings".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    "microphone-no-devices",
                )),
            }),
            crate::dictation::DictationMicrophoneStatus::EnumerationFailed(error) => {
                choices.push(Choice {
                    name: "Could not list microphones".to_string(),
                    value: "microphone-enumeration-failed".to_string(),
                    description: Some(error.clone()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_SETUP_PROMPT_ID,
                        choices.len(),
                        "microphone-enumeration-failed",
                    )),
                })
            }
            crate::dictation::DictationMicrophoneStatus::SavedDeviceMissing { fallback_name } => {
                choices.push(Choice {
                    name: "Saved microphone unavailable".to_string(),
                    value: "microphone-saved-missing".to_string(),
                    description: Some(
                        fallback_name
                            .as_ref()
                            .map(|name| format!("Using System Default ({name})"))
                            .unwrap_or_else(|| "Using System Default".to_string()),
                    ),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_SETUP_PROMPT_ID,
                        choices.len(),
                        "microphone-saved-missing",
                    )),
                })
            }
            crate::dictation::DictationMicrophoneStatus::PermissionNeeded(_) => {
                choices.push(Choice {
                    name: "Microphone permission unknown".to_string(),
                    value: "microphone-permission-unknown".to_string(),
                    description: Some(
                        "Start dictation again after checking permission".to_string(),
                    ),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_SETUP_PROMPT_ID,
                        choices.len(),
                        "microphone-permission-unknown",
                    )),
                })
            }
        }

        match &state.hotkey_status {
            crate::dictation::DictationHotkeyStatus::Ready(combo) => choices.push(Choice {
                name: format!("Hotkey ready: {combo}"),
                value: "hotkey-ready".to_string(),
                description: Some("Optional shortcut configured".to_string()),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    "hotkey-ready",
                )),
            }),
            crate::dictation::DictationHotkeyStatus::Disabled => choices.push(Choice {
                name: "Hotkey disabled".to_string(),
                value: "hotkey-disabled".to_string(),
                description: Some(
                    "You can still start dictation from this prompt or the launcher".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    "hotkey-disabled",
                )),
            }),
            crate::dictation::DictationHotkeyStatus::NotConfigured => choices.push(Choice {
                name: "Hotkey not configured".to_string(),
                value: "hotkey-not-configured".to_string(),
                description: Some(
                    "You can still start dictation from this prompt or the launcher".to_string(),
                ),
                key: None,
                semantic_id: Some(builtin_choice_semantic_id(
                    BUILTIN_DICTATION_SETUP_PROMPT_ID,
                    choices.len(),
                    "hotkey-not-configured",
                )),
            }),
        }

        (title, placeholder, choices)
    }

    fn open_dictation_setup_prompt(&mut self, cx: &mut Context<Self>) {
        let prefs = crate::config::load_user_preferences();
        let model_id =
            crate::dictation::DictationModelId::from_preference(prefs.dictation.model.as_deref());
        let state = self.current_dictation_setup_state();
        let (title, placeholder, choices) = Self::build_dictation_setup_prompt(&state, model_id);
        self.arg_selected_index = 0;
        self.open_builtin_filterable_view(
            AppView::MiniPrompt {
                id: BUILTIN_DICTATION_SETUP_PROMPT_ID.to_string(),
                placeholder,
                choices,
            },
            &title,
            false,
            cx,
        );
    }

    fn handle_dictation_setup_selection(&mut self, value: &str, cx: &mut Context<Self>) {
        match value {
            BUILTIN_DICTATION_SETUP_START => {
                let dctx = crate::action_helpers::DispatchContext::for_builtin("builtin/dictation");
                let _ = self.execute_dictation_builtin_action(
                    DictationBuiltinAction::CurrentSurface,
                    &dctx,
                    cx,
                );
            }
            BUILTIN_DICTATION_SETUP_DOWNLOAD_MODEL => self.open_dictation_model_prompt(cx),
            BUILTIN_DICTATION_SETUP_REQUEST_MIC => {
                crate::dictation::request_microphone_permission_nonblocking(|status| {
                    tracing::info!(category = "DICTATION", status = ?status, "Microphone permission prompt resolved from setup prompt");
                });
                self.show_hud(
                    "Choose Allow when macOS asks for microphone access".to_string(),
                    Some(HUD_MEDIUM_MS),
                    cx,
                );
            }
            BUILTIN_DICTATION_SETUP_OPEN_MIC_SETTINGS => {
                let url = crate::permissions_wizard::PermissionKind::Microphone.settings_url();
                if let Err(error) = open::that(url) {
                    self.show_error_toast(
                        format!("Could not open Microphone settings: {error}"),
                        cx,
                    );
                }
            }
            _ => self.show_hud(
                "Setup item is informational".to_string(),
                Some(HUD_SHORT_MS),
                cx,
            ),
        }
    }

    /// Ensure a new dictation session has somewhere valid to send text.
    ///
    /// Allowed start conditions:
    /// - the launcher/main filter is active, or
    /// - a Script Kit prompt is active and can accept dictated text, or
    /// - the frontmost-app tracker already has a previously tracked external target.
    fn open_dictation_setup_if_not_ready(&mut self, cx: &mut Context<Self>) -> bool {
        let permission = crate::dictation::microphone_permission_status();
        let prefs = crate::config::load_user_preferences();
        let config = crate::config::load_config();
        let selected_device_id = prefs.dictation.selected_device_id.as_deref();
        let devices = match permission {
            crate::dictation::DictationMicrophonePermissionStatus::Granted
            | crate::dictation::DictationMicrophonePermissionStatus::Unknown => {
                crate::dictation::list_input_devices().map_err(|error| error.to_string())
            }
            crate::dictation::DictationMicrophonePermissionStatus::Denied
            | crate::dictation::DictationMicrophonePermissionStatus::NotDetermined => {
                Ok(Vec::new())
            }
        };
        let device_count = devices.as_ref().map(|devices| devices.len()).unwrap_or(0);
        // Real model status, not a hardcoded Available: the builtin start path
        // gates on the model upstream, but this preflight must stay honest for
        // every other entry point so a missing model routes to setup before
        // recording starts instead of after transcription fails.
        let prefs = crate::config::load_user_preferences();
        let model_id =
            crate::dictation::DictationModelId::from_preference(prefs.dictation.model.as_deref());
        let model = crate::dictation::dictation_model_entry(model_id);
        let model_status = if model.is_available() {
            crate::dictation::DictationModelStatus::Available
        } else {
            dictation_model_prompt_status().lock().clone()
        };
        let setup_state = crate::dictation::build_dictation_setup_state(
            model_status,
            permission,
            devices,
            selected_device_id,
            config.get_dictation_hotkey().as_ref(),
            config.is_dictation_hotkey_enabled(),
        );

        if setup_state.ready {
            return false;
        }

        match setup_state.microphone_status {
            crate::dictation::DictationMicrophoneStatus::SavedDeviceMissing { fallback_name } => {
                if let Err(error) = crate::dictation::save_dictation_device_id(None) {
                    tracing::warn!(
                        category = "DICTATION",
                        error = %error,
                        "Failed to clear stale dictation microphone before start"
                    );
                }
                let message = fallback_name
                    .map(|name| {
                        format!("Saved microphone unavailable — using System Default ({name})")
                    })
                    .unwrap_or_else(|| {
                        "Saved microphone unavailable — using System Default".to_string()
                    });
                self.show_hud(message, Some(HUD_MEDIUM_MS), cx);
                return false;
            }
            crate::dictation::DictationMicrophoneStatus::PermissionNeeded(
                crate::dictation::DictationMicrophonePermissionStatus::NotDetermined,
            ) => {
                // Fire the macOS TCC prompt here — nothing else in the app
                // calls requestAccess, and System Settings won't even list
                // Script Kit under Microphone until it has fired once.
                let (permission_tx, permission_rx) = async_channel::bounded(1);
                crate::dictation::request_microphone_permission_nonblocking(move |status| {
                    tracing::info!(
                        category = "DICTATION",
                        status = ?status,
                        "Microphone permission prompt resolved"
                    );
                    let _ = permission_tx.try_send(status);
                });
                cx.spawn(async move |this, cx| {
                    if let Ok(status) = permission_rx.recv().await {
                        let _ = this.update(cx, |this, cx| {
                            if matches!(status, crate::dictation::DictationMicrophonePermissionStatus::Granted) {
                                this.show_hud(
                                    "Microphone ready — start dictation again".to_string(),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                            } else {
                                this.show_hud(
                                    "Microphone access denied — enable it in System Settings → Privacy & Security → Microphone".to_string(),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                            }
                        });
                    }
                }).detach();
                self.show_error_toast(
                    "Choose Allow when macOS asks for microphone access, then start dictation again.",
                    cx,
                );
                self.open_dictation_setup_prompt(cx);
            }
            crate::dictation::DictationMicrophoneStatus::PermissionNeeded(
                crate::dictation::DictationMicrophonePermissionStatus::Denied,
            ) => {
                self.show_error_toast(
                    "Microphone access is off. Enable Script Kit in System Settings → Privacy & Security → Microphone.",
                    cx,
                );
                self.open_dictation_setup_prompt(cx);
            }
            crate::dictation::DictationMicrophoneStatus::NoDevices => {
                self.show_error_toast(
                    "No microphone found. Connect an input device or choose one in macOS Sound settings.",
                    cx,
                );
                self.open_dictation_setup_prompt(cx);
            }
            crate::dictation::DictationMicrophoneStatus::EnumerationFailed(error) => {
                self.show_error_toast(format!("Could not list microphones: {error}"), cx);
                self.open_dictation_setup_prompt(cx);
            }
            crate::dictation::DictationMicrophoneStatus::Ready { .. }
            | crate::dictation::DictationMicrophoneStatus::PermissionNeeded(
                crate::dictation::DictationMicrophonePermissionStatus::Granted
                | crate::dictation::DictationMicrophonePermissionStatus::Unknown,
            ) => {
                // Mic is fine, so the only way to be un-ready here is a
                // missing/in-flight model — route to the download prompt.
                self.open_dictation_model_prompt(cx);
            }
        }
        tracing::info!(
            category = "DICTATION",
            permission = ?permission,
            device_count,
            ready = setup_state.ready,
            "Dictation start preflight opened setup instead of starting capture"
        );
        true
    }

    fn ensure_dictation_delivery_target_available(&self) -> anyhow::Result<()> {
        if self.can_accept_dictation_into_main_filter() || self.can_accept_dictation_into_prompt() {
            return Ok(());
        }
        Self::ensure_dictation_frontmost_target_available()
    }

    /// Verify that the frontmost-app tracker has a previously-tracked
    /// external app target before attempting a dictation paste.
    fn ensure_dictation_frontmost_target_available() -> anyhow::Result<()> {
        use anyhow::Context as _;
        crate::frontmost_app_tracker::get_last_real_app_bundle_id()
            .context("no previously tracked frontmost app is available for dictation paste")?;
        Ok(())
    }

    fn has_dictation_frontmost_target() -> bool {
        crate::frontmost_app_tracker::get_last_real_app_bundle_id().is_some()
    }

    /// Validate that a specific delivery target is reachable before
    /// starting dictation.  Script Kit internal targets (harness, notes,
    /// AI composer, prompt) are always available.  `ExternalApp` requires
    /// the frontmost-app tracker to have a previously-tracked target.
    fn ensure_dictation_delivery_target_available_for(
        &self,
        target: crate::dictation::DictationTarget,
    ) -> anyhow::Result<()> {
        match target {
            crate::dictation::DictationTarget::ExternalApp => {
                Self::ensure_dictation_frontmost_target_available()
            }
            crate::dictation::DictationTarget::MainWindowFilter
            | crate::dictation::DictationTarget::MainWindowPrompt
            | crate::dictation::DictationTarget::NotesEditor
            | crate::dictation::DictationTarget::AiChatComposer
            | crate::dictation::DictationTarget::TabAiHarness
            | crate::dictation::DictationTarget::DayPageToday
            | crate::dictation::DictationTarget::QuickAiQuestion => Ok(()),
        }
    }

    /// Build the small per-session destination cycle exposed by the overlay
    /// badge. Keep it intentionally tight: the primary target plus an
    /// Agent Chat chat-submit fallback, except external-app sessions which expose
    /// the main launcher filter as the alternate target.
    fn dictation_target_cycle_for(
        &self,
        target: crate::dictation::DictationTarget,
    ) -> Vec<crate::dictation::DictationTarget> {
        match target {
            crate::dictation::DictationTarget::ExternalApp => vec![
                crate::dictation::DictationTarget::ExternalApp,
                crate::dictation::DictationTarget::MainWindowFilter,
            ],
            crate::dictation::DictationTarget::MainWindowFilter => vec![
                crate::dictation::DictationTarget::MainWindowFilter,
                crate::dictation::DictationTarget::TabAiHarness,
            ],
            crate::dictation::DictationTarget::MainWindowPrompt => vec![
                crate::dictation::DictationTarget::MainWindowPrompt,
                crate::dictation::DictationTarget::TabAiHarness,
            ],
            crate::dictation::DictationTarget::NotesEditor => vec![
                crate::dictation::DictationTarget::NotesEditor,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::AiChatComposer => vec![
                crate::dictation::DictationTarget::AiChatComposer,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::TabAiHarness => vec![
                crate::dictation::DictationTarget::TabAiHarness,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::DayPageToday => vec![
                crate::dictation::DictationTarget::DayPageToday,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::QuickAiQuestion => vec![
                crate::dictation::DictationTarget::QuickAiQuestion,
                crate::dictation::DictationTarget::ExternalApp,
            ],
        }
    }

    /// Resolve the dictation delivery target, optionally overriding to
    /// `TabAiHarness` so a dedicated "dictate to Agent Chat" action can
    /// force harness delivery even when the harness is not already on-screen.
    pub(crate) fn resolve_dictation_target_with_override(
        &self,
        force_tab_ai_harness: bool,
    ) -> crate::dictation::DictationTarget {
        if force_tab_ai_harness {
            crate::dictation::DictationTarget::TabAiHarness
        } else {
            self.resolve_dictation_target()
        }
    }

    /// Determine the delivery target for a new dictation session based on
    /// which Script Kit surface is currently active.
    ///
    /// Priority: notes editor > AI chat composer > active prompt >
    /// tracked frontmost app > launcher main filter fallback > external app.
    fn resolve_dictation_target(&self) -> crate::dictation::DictationTarget {
        let has_frontmost_app_target = Self::has_dictation_frontmost_target();
        let target = if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
            crate::dictation::DictationTarget::TabAiHarness
        } else if notes::is_notes_window_open() {
            crate::dictation::DictationTarget::NotesEditor
        } else if ai::is_ai_window_open() {
            crate::dictation::DictationTarget::AiChatComposer
        } else if self.can_accept_dictation_into_prompt() {
            crate::dictation::DictationTarget::MainWindowPrompt
        } else if has_frontmost_app_target {
            crate::dictation::DictationTarget::ExternalApp
        } else if self.can_accept_dictation_into_main_filter() {
            crate::dictation::DictationTarget::MainWindowFilter
        } else {
            crate::dictation::DictationTarget::ExternalApp
        };
        tracing::info!(
            category = "DICTATION",
            ?target,
            current_view = ?std::mem::discriminant(&self.current_view),
            notes_open = notes::is_notes_window_open(),
            ai_open = ai::is_ai_window_open(),
            has_frontmost_app_target,
            accepts_main_filter = self.can_accept_dictation_into_main_filter(),
            accepts_prompt = self.can_accept_dictation_into_prompt(),
            "Resolved dictation target"
        );
        target
    }

    /// Close the dictation overlay, hide Script Kit, and activate the tracked
    /// target app before the CGEvent Cmd+V paste fires.
    fn yield_focus_for_dictation_paste(
        &mut self,
        target_pid: i32,
        target_bundle_id: &str,
        target_app_name: &str,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        use anyhow::Context as _;
        tracing::info!(
            category = "DICTATION",
            target_pid,
            target_bundle_id = %target_bundle_id,
            target_app_name = %target_app_name,
            "Yielding focus for dictation paste"
        );
        crate::dictation::close_dictation_overlay(cx)
            .context("failed to close dictation overlay before paste")?;
        if script_kit_gpui::is_main_window_visible() {
            script_kit_gpui::set_main_window_visible(false);
            platform::defer_hide_main_window(cx);
        }
        Self::activate_tracked_app_for_dictation_paste(
            target_pid,
            target_bundle_id,
            target_app_name,
        )?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn activate_tracked_app_for_dictation_paste(
        target_pid: i32,
        target_bundle_id: &str,
        target_app_name: &str,
    ) -> anyhow::Result<()> {
        use anyhow::{Context as _, anyhow};
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        unsafe {
            let workspace_class = Class::get("NSWorkspace").context("NSWorkspace unavailable")?;
            let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
            let running_apps: *mut Object = msg_send![workspace, runningApplications];
            let app_count: usize = msg_send![running_apps, count];

            for i in 0..app_count {
                let app: *mut Object = msg_send![running_apps, objectAtIndex: i];
                let app_pid: i32 = msg_send![app, processIdentifier];
                if app_pid != target_pid {
                    continue;
                }

                let activated: bool = msg_send![app, activateWithOptions: 2u64];
                if activated {
                    tracing::info!(
                        category = "DICTATION",
                        target_pid,
                        target_bundle_id,
                        target_app_name,
                        "Activated tracked app for dictation paste"
                    );
                    return Ok(());
                }

                return Err(anyhow!(
                    "failed to activate {target_app_name} ({target_bundle_id}) for dictation paste"
                ));
            }
        }

        Err(anyhow!(
            "tracked app {target_app_name} ({target_bundle_id}) with pid {target_pid} is no longer running"
        ))
    }

    #[cfg(not(target_os = "macos"))]
    fn activate_tracked_app_for_dictation_paste(
        _target_pid: i32,
        _target_bundle_id: &str,
        _target_app_name: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Schedule the overlay window to close after a delay.
    fn schedule_dictation_overlay_close(
        &mut self,
        cx: &mut Context<Self>,
        delay: std::time::Duration,
    ) {
        let generation = crate::dictation::overlay_generation();
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            // Contract: if crate::dictation::overlay_generation() != gen {
            // Only close if the overlay hasn't been replaced by a newer session.
            if crate::dictation::overlay_generation() != generation {
                tracing::debug!(
                    category = "DICTATION",
                    "Scheduled overlay close skipped — generation changed"
                );
                return;
            }
            cx.update(|cx| {
                let _ = crate::dictation::close_dictation_overlay(cx);
            });
        })
        .detach();
    }

    /// Bring the main window back after a delay.
    ///
    /// Used by the dictation-to-AI path: the main window is concealed so
    /// the overlay is visible, and once the overlay closes we reveal the
    /// main window with the newly-opened Agent Chat chat view.  The delay must
    /// be slightly longer than `schedule_dictation_overlay_close` so the
    /// overlay is gone before the main window reappears.
    fn schedule_deferred_main_window_reveal(
        &mut self,
        cx: &mut Context<Self>,
        delay: std::time::Duration,
    ) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|_cx| {
                platform::show_main_window_without_activation();
            });
        })
        .detach();
    }

    /// Schedule the cached transcriber to be unloaded after an idle timeout.
    fn schedule_dictation_transcriber_cleanup(
        &mut self,
        cx: &mut Context<Self>,
        delay: std::time::Duration,
    ) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|_cx| {
                crate::dictation::maybe_unload_transcriber();
            });
        })
        .detach();
    }
}

#[cfg(test)]
include!("builtin_execution_tests.rs");

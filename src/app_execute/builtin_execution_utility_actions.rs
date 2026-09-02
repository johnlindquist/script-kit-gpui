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
            builtins::UtilityCommandType::MainWindow => Self::Open(
                UtilityOpenBuiltinAction::from_command(command)
                    .unwrap_or(UtilityOpenBuiltinAction::MainWindow),
            ),
            builtins::UtilityCommandType::ScratchPad => Self::Open(
                UtilityOpenBuiltinAction::from_command(command)
                    .unwrap_or(UtilityOpenBuiltinAction::ScratchPad),
            ),
            builtins::UtilityCommandType::QuickTerminal => Self::Open(
                UtilityOpenBuiltinAction::from_command(command)
                    .unwrap_or(UtilityOpenBuiltinAction::QuickTerminal),
            ),
            builtins::UtilityCommandType::ClaudeCode => Self::Open(
                UtilityOpenBuiltinAction::from_command(command)
                    .unwrap_or(UtilityOpenBuiltinAction::ClaudeCode),
            ),
            builtins::UtilityCommandType::ProcessManager => Self::Open(
                UtilityOpenBuiltinAction::from_command(command)
                    .unwrap_or(UtilityOpenBuiltinAction::ProcessManager),
            ),
            builtins::UtilityCommandType::StopAllProcesses => Self::Process(
                UtilityProcessBuiltinAction::from_command(command)
                    .unwrap_or(UtilityProcessBuiltinAction::StopAllProcesses),
            ),
            builtins::UtilityCommandType::ScriptKitSelfie => Self::Selfie(
                UtilitySelfieBuiltinAction::from_command(command)
                    .unwrap_or(UtilitySelfieBuiltinAction::Selfie),
            ),
            builtins::UtilityCommandType::TurnThisIntoCommand => Self::Recipe(
                UtilityRecipeBuiltinAction::from_command(command)
                    .unwrap_or(UtilityRecipeBuiltinAction::TurnThisIntoCommand),
            ),
            builtins::UtilityCommandType::DoInCurrentApp => Self::DoInCurrentApp(
                UtilityDoInCurrentAppBuiltinAction::from_command(command)
                    .unwrap_or(UtilityDoInCurrentAppBuiltinAction::Submit),
            ),
            builtins::UtilityCommandType::CurrentAppCommands => Self::CurrentAppCommands(
                UtilityCurrentAppCommandsBuiltinAction::from_command(command)
                    .unwrap_or(UtilityCurrentAppCommandsBuiltinAction::Open),
            ),
        }
    }
}

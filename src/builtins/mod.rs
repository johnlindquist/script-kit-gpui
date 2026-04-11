//! Built-in Features Registry
//!
//! Provides a registry of built-in features that appear in the main search
//! alongside scripts. Features like Clipboard History and App Launcher are
//! configurable and can be enabled/disabled via config.
//!
//! ## Command Types
//!
//! The registry supports various command types organized by category:
//! - **System Actions**: Power management, UI controls, volume/brightness
//! - **Window Actions**: Window tiling and management for the frontmost window
//! - **Notes Commands**: Notes window operations
//! - **AI Commands**: AI chat window operations  
//! - **Script Commands**: Create new scripts and scriptlets
//! - **Permission Commands**: Accessibility permission management
//!

use crate::config::BuiltInConfig;
use crate::menu_bar::current_app_commands::{
    GENERATE_SCRIPT_FROM_CURRENT_APP_LABEL, GENERATE_SCRIPT_WITH_AI_LABEL,
};
use crate::menu_bar::MenuBarItem;
use tracing::debug;
// ============================================================================
// Command Type Enums
// ============================================================================

/// System action types for macOS system commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemActionType {
    // Power management
    EmptyTrash,
    LockScreen,
    Sleep,
    Restart,
    ShutDown,
    LogOut,

    // UI controls
    ToggleDarkMode,
    ShowDesktop,
    MissionControl,
    Launchpad,
    ForceQuitApps,

    // Volume controls (preset levels)
    Volume0,
    Volume25,
    Volume50,
    Volume75,
    Volume100,
    VolumeMute,

    // Dev/test actions (only available in debug builds)
    #[cfg(debug_assertions)]
    TestConfirmation,

    // App control
    QuitScriptKit,

    // System utilities
    ToggleDoNotDisturb,
    StartScreenSaver,

    // System Preferences
    OpenSystemPreferences,
    OpenPrivacySettings,
    OpenDisplaySettings,
    OpenSoundSettings,
    OpenNetworkSettings,
    OpenKeyboardSettings,
    OpenBluetoothSettings,
    OpenNotificationsSettings,
}
/// Returns user-facing HUD feedback text for system actions when available.
///
/// `dark_mode_enabled` should be populated only for `ToggleDarkMode` after
/// querying current system state.
pub fn system_action_hud_message(
    action_type: SystemActionType,
    dark_mode_enabled: Option<bool>,
) -> Option<String> {
    match action_type {
        SystemActionType::ToggleDarkMode => Some(match dark_mode_enabled {
            Some(true) => "Dark Mode On".to_string(),
            Some(false) => "Dark Mode Off".to_string(),
            None => "Dark Mode Toggled".to_string(),
        }),
        SystemActionType::Volume0 => Some("Volume 0%".to_string()),
        SystemActionType::Volume25 => Some("Volume 25%".to_string()),
        SystemActionType::Volume50 => Some("Volume 50%".to_string()),
        SystemActionType::Volume75 => Some("Volume 75%".to_string()),
        SystemActionType::Volume100 => Some("Volume 100%".to_string()),
        SystemActionType::VolumeMute => Some("Volume Muted".to_string()),
        SystemActionType::ToggleDoNotDisturb => Some("Do Not Disturb toggled".to_string()),
        _ => None,
    }
}
// NOTE: WindowActionType has been removed from built-ins.
// Window management is now handled by the window-management extension.
// SDK tileWindow() function still works via protocol messages in execute_script.rs.

/// Notes window command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NotesCommandType {
    OpenNotes,
    NewNote,
    SearchNotes,
    QuickCapture,
}
/// AI window command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AiCommandType {
    OpenAi,
    MiniAi,
    NewConversation,
    ClearConversation,
    /// Generate a new Script Kit script from the main prompt text
    GenerateScript,
    /// Generate a new Script Kit script using the frontmost app's live context
    GenerateScriptFromCurrentApp,
    /// Send a screenshot of the entire screen to ACP Chat
    SendScreenToAi,
    /// Send a screenshot of the focused window to ACP Chat
    SendFocusedWindowToAi,
    /// Send the currently selected text to ACP Chat
    SendSelectedTextToAi,
    /// Send the focused browser tab URL/content to ACP Chat
    SendBrowserTabToAi,
    /// Send a selected screen area to ACP Chat (interactive selection)
    SendScreenAreaToAi,
    /// Create a new AI chat preset/template
    CreateAiPreset,
    /// Import AI chat presets from file
    ImportAiPresets,
    /// Search through saved AI chat presets
    SearchAiPresets,
    /// Export AI chat presets to a user-chosen file
    ExportAiPresets,
}

impl AiCommandType {
    /// Returns `true` for legacy AI enum variants that now simply open the harness.
    pub fn is_legacy_harness_alias(self) -> bool {
        matches!(
            self,
            Self::OpenAi | Self::MiniAi | Self::NewConversation | Self::ClearConversation
        )
    }
}
/// Script creation command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptCommandType {
    NewScript,
    NewExtension,
}
/// Permission management command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionCommandType {
    CheckPermissions,
    RequestAccessibility,
    OpenAccessibilitySettings,
}
/// Frecency/suggested items command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrecencyCommandType {
    ClearSuggested,
}
/// Settings command types for app configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCommandType {
    /// Reset all window positions to defaults
    ResetWindowPositions,
    /// Configure Vercel AI Gateway API key
    ConfigureVercelApiKey,
    /// Configure OpenAI API key
    ConfigureOpenAiApiKey,
    /// Configure Anthropic API key
    ConfigureAnthropicApiKey,
    /// Browse and apply color themes
    ChooseTheme,
    /// Select microphone for dictation
    SelectMicrophone,
    /// Disable drag snapping entirely
    DisableWindowSnapping,
    /// Use the simplest snap target set
    SnapModeSimple,
    /// Use the default expanded snap target set
    SnapModeExpanded,
    /// Use the densest snap target set
    SnapModePrecision,
}
/// Utility command types for quick access tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilityCommandType {
    /// Open the main launcher in compact mini-window mode
    MiniMainWindow,
    /// Open scratch pad - auto-saving editor
    ScratchPad,
    /// Open quick terminal for running commands
    QuickTerminal,
    /// Open Claude Code in the harness terminal surface used by Tab AI
    ClaudeCode,
    /// Inspect actively running Script Kit child processes
    ProcessManager,
    /// Terminate all actively running Script Kit child processes
    StopAllProcesses,
    /// Interpret a free-text request against the frontmost app:
    /// execute a matching menu command or fall back to script generation
    DoInCurrentApp,
    /// Capture a reusable automation recipe from the frontmost app
    /// and hand it off to script generation
    TurnThisIntoCommand,
    /// Search and run menu bar commands from the frontmost app
    CurrentAppCommands,
    /// Capture a deterministic JSON snapshot of current desktop context and copy to clipboard
    InspectCurrentContext,
    /// Dry-run a free-text request against the frontmost app and copy a JSON routing trace
    TraceCurrentAppIntent,
    /// Verify a currentAppCommand recipe from the clipboard against live context
    VerifyCurrentAppRecipe,
    /// Replay a verified currentAppCommand recipe from the clipboard
    ReplayCurrentAppRecipe,
}
/// Kit Store command types for browsing and managing kits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum KitStoreCommandType {
    BrowseKits,
    InstalledKits,
    UpdateAllKits,
}
/// Menu bar action details for executing menu commands
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuBarActionInfo {
    /// The bundle ID of the app (e.g., "com.apple.Safari")
    pub bundle_id: String,
    /// The path to the menu item (e.g., ["File", "New Window"])
    pub menu_path: Vec<String>,
    /// Whether the menu item is enabled
    pub enabled: bool,
    /// Keyboard shortcut if any (e.g., "⌘N")
    pub shortcut: Option<String>,
}
/// Groups for categorizing built-in entries in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)] // MenuBar variant will be used when menu bar integration is complete
pub enum BuiltInGroup {
    /// Core built-in features (Clipboard History, Window Switcher, etc.)
    #[default]
    Core,
    /// Menu bar items from the frontmost application
    MenuBar,
}
/// Types of built-in features
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Some variants reserved for future use
pub enum BuiltInFeature {
    /// Clipboard history viewer/manager
    ClipboardHistory,
    /// Paste multiple items one-by-one in sequence
    PasteSequentially,
    /// Favorites list and quick access
    Favorites,
    /// Application launcher for opening installed apps (legacy, apps now in main search)
    AppLauncher,
    /// Individual application entry (for future use when apps appear in search)
    App(String),
    /// Window switcher for managing and tiling windows
    WindowSwitcher,
    /// Design gallery for viewing separator and icon variations
    DesignGallery,
    /// In-app StoryBrowser compare/adopt tool (storybook feature only)
    #[cfg(feature = "storybook")]
    DesignExplorer,
    /// ACP Chat window for conversing with AI assistants
    AiChat,
    /// Notes window for quick notes and scratchpad
    Notes,
    /// Emoji picker for selecting and copying emojis
    EmojiPicker,
    /// Menu bar action from the frontmost application
    MenuBarAction(MenuBarActionInfo),

    // === New Command Types ===
    /// System actions (power, UI controls, volume, brightness, settings)
    SystemAction(SystemActionType),
    // NOTE: WindowAction removed - now handled by window-management extension
    /// Notes window commands
    NotesCommand(NotesCommandType),
    /// AI window commands
    AiCommand(AiCommandType),
    /// Script creation commands
    ScriptCommand(ScriptCommandType),
    /// Permission management commands
    PermissionCommand(PermissionCommandType),
    /// Frecency/suggested items commands
    FrecencyCommand(FrecencyCommandType),
    /// Settings commands (window positions, etc.)
    SettingsCommand(SettingsCommandType),
    /// Utility commands (scratch pad, quick terminal)
    UtilityCommand(UtilityCommandType),
    /// Kit Store commands (browse, installed kits, update all)
    KitStoreCommand(KitStoreCommandType),
    /// File search for navigating directories and finding files
    FileSearch,
    /// Webcam capture
    Webcam,
    /// Voice dictation overlay + paste flow
    Dictation,
    /// Voice dictation that always targets the AI harness
    DictationToAiHarness,
    /// Voice dictation that always targets the frontmost external app
    DictationToFrontmostApp,
    /// Voice dictation that always targets the notes editor
    DictationToNotes,
    /// Settings hub for viewing configuration panels
    Settings,
    /// ACP conversation history browser
    AcpHistory,
}
/// A built-in feature entry that appears in the main search
#[derive(Debug, Clone)]
pub struct BuiltInEntry {
    /// Unique identifier for the entry
    pub id: String,
    /// Display name shown in search results
    pub name: String,
    /// Description shown below the name
    pub description: String,
    /// Keywords for fuzzy matching in search
    pub keywords: Vec<String>,
    /// The actual feature this entry represents
    pub feature: BuiltInFeature,
    /// Optional icon name (Lucide kebab-case or legacy emoji) to display
    pub icon: Option<String>,
    /// Group for categorization in the UI (will be used when menu bar integration is complete)
    #[allow(dead_code)]
    pub group: BuiltInGroup,
}
impl BuiltInEntry {
    /// Create a new built-in entry (Core group, no icon)
    #[allow(dead_code)]
    fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        keywords: Vec<&str>,
        feature: BuiltInFeature,
    ) -> Self {
        BuiltInEntry {
            id: crate::config::canonical_builtin_command_id(&id.into()),
            name: name.into(),
            description: description.into(),
            keywords: keywords.into_iter().map(String::from).collect(),
            feature,
            icon: None,
            group: BuiltInGroup::Core,
        }
    }

    /// Create a new built-in entry with an icon (Core group)
    fn new_with_icon(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        keywords: Vec<&str>,
        feature: BuiltInFeature,
        icon: impl Into<String>,
    ) -> Self {
        BuiltInEntry {
            id: crate::config::canonical_builtin_command_id(&id.into()),
            name: name.into(),
            description: description.into(),
            keywords: keywords.into_iter().map(String::from).collect(),
            feature,
            icon: Some(icon.into()),
            group: BuiltInGroup::Core,
        }
    }

    /// Create a new built-in entry with icon and group
    ///
    /// Note: Unlike `new()` and `new_with_icon()`, this does NOT canonicalize the ID
    /// because it is used for non-builtin groups (e.g., MenuBar) with their own ID schemes.
    #[allow(dead_code)]
    pub fn new_with_group(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        keywords: Vec<String>,
        feature: BuiltInFeature,
        icon: Option<String>,
        group: BuiltInGroup,
    ) -> Self {
        BuiltInEntry {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            keywords,
            feature,
            icon,
            group,
        }
    }

    /// Check if this built-in should be excluded from frecency/suggested tracking.
    /// Some commands like "Quit Script Kit" don't make sense to suggest.
    /// Uses the user-configurable excluded_commands list from SuggestedConfig.
    pub fn should_exclude_from_frecency(&self, excluded_commands: &[String]) -> bool {
        excluded_commands.iter().any(|cmd| cmd == &self.id)
    }

    /// Get the leaf name for menu bar items (the actual menu item name, not the full path).
    /// For "Shell → New Tab", returns "New Tab".
    /// For non-menu bar items, returns the full name.
    pub fn leaf_name(&self) -> &str {
        if self.group == BuiltInGroup::MenuBar {
            // Menu bar names are formatted as "Menu → Submenu → Item"
            // Extract the last component (the actual menu item name)
            self.name.rsplit(" → ").next().unwrap_or(&self.name)
        } else {
            &self.name
        }
    }

    pub fn default_action_text(&self) -> &'static str {
        match &self.feature {
            BuiltInFeature::ClipboardHistory => "Open Clipboard History",
            BuiltInFeature::PasteSequentially => "Paste Next Item",
            BuiltInFeature::Favorites => "Open Favorites",
            BuiltInFeature::AppLauncher => "Open App Launcher",
            BuiltInFeature::App(_) => "Launch App",
            BuiltInFeature::WindowSwitcher => "Open Window Switcher",
            BuiltInFeature::DesignGallery => "Open Gallery",
            #[cfg(feature = "storybook")]
            BuiltInFeature::DesignExplorer => "Open Explorer",
            BuiltInFeature::AiChat => "Open ACP Chat",
            BuiltInFeature::Notes => "Open Notes",
            BuiltInFeature::EmojiPicker => "Open Emoji Picker",
            BuiltInFeature::MenuBarAction(_) => "Execute Menu Item",
            BuiltInFeature::SystemAction(action) => match action {
                SystemActionType::EmptyTrash => "Empty Trash",
                SystemActionType::LockScreen => "Lock Screen",
                SystemActionType::Sleep => "Put Mac to Sleep",
                SystemActionType::Restart => "Restart Mac",
                SystemActionType::ShutDown => "Shut Down Mac",
                SystemActionType::LogOut => "Log Out",
                SystemActionType::ToggleDarkMode => "Toggle Dark Mode",
                SystemActionType::ShowDesktop => "Show Desktop",
                SystemActionType::MissionControl => "Open Mission Control",
                SystemActionType::Launchpad => "Open Launchpad",
                SystemActionType::ForceQuitApps => "Open Force Quit",
                SystemActionType::Volume0 => "Set Volume to 0%",
                SystemActionType::Volume25 => "Set Volume to 25%",
                SystemActionType::Volume50 => "Set Volume to 50%",
                SystemActionType::Volume75 => "Set Volume to 75%",
                SystemActionType::Volume100 => "Set Volume to 100%",
                SystemActionType::VolumeMute => "Mute Audio",
                #[cfg(debug_assertions)]
                SystemActionType::TestConfirmation => "Open Test Dialog",
                SystemActionType::QuitScriptKit => "Quit Script Kit",
                SystemActionType::ToggleDoNotDisturb => "Toggle Do Not Disturb",
                SystemActionType::StartScreenSaver => "Start Screen Saver",
                SystemActionType::OpenSystemPreferences => "Open macOS System Settings",
                SystemActionType::OpenPrivacySettings => "Open Privacy Settings",
                SystemActionType::OpenDisplaySettings => "Open Display Settings",
                SystemActionType::OpenSoundSettings => "Open Sound Settings",
                SystemActionType::OpenNetworkSettings => "Open Network Settings",
                SystemActionType::OpenKeyboardSettings => "Open Keyboard Settings",
                SystemActionType::OpenBluetoothSettings => "Open Bluetooth Settings",
                SystemActionType::OpenNotificationsSettings => "Open Notification Settings",
            },
            BuiltInFeature::NotesCommand(action) => match action {
                NotesCommandType::OpenNotes => "Open Notes",
                NotesCommandType::NewNote => "Create Note",
                NotesCommandType::SearchNotes => "Search Notes",
                NotesCommandType::QuickCapture => "Start Quick Capture",
            },
            BuiltInFeature::AiCommand(action) => match action {
                AiCommandType::OpenAi | AiCommandType::MiniAi => "Open ACP Chat",
                AiCommandType::NewConversation => "Start New Chat",
                AiCommandType::ClearConversation => "Clear Conversation",
                AiCommandType::GenerateScript => "Generate Script",
                AiCommandType::GenerateScriptFromCurrentApp => "Generate Script from App",
                AiCommandType::SendScreenToAi => "Send Screen to AI",
                AiCommandType::SendFocusedWindowToAi => "Send Window to AI",
                AiCommandType::SendSelectedTextToAi => "Send Selection to AI",
                AiCommandType::SendBrowserTabToAi => "Send Tab to AI",
                AiCommandType::SendScreenAreaToAi => "Select Area for AI",
                AiCommandType::CreateAiPreset => "Create AI Preset",
                AiCommandType::ImportAiPresets => "Import AI Presets",
                AiCommandType::SearchAiPresets => "Search AI Presets",
                AiCommandType::ExportAiPresets => "Export AI Presets",
            },
            BuiltInFeature::ScriptCommand(action) => match action {
                ScriptCommandType::NewScript => "Create Script",
                ScriptCommandType::NewExtension => "Create Scriptlet Bundle",
            },
            BuiltInFeature::PermissionCommand(action) => match action {
                PermissionCommandType::CheckPermissions => "Check Permissions",
                PermissionCommandType::RequestAccessibility => "Request Accessibility Access",
                PermissionCommandType::OpenAccessibilitySettings => "Open Accessibility Settings",
            },
            BuiltInFeature::FrecencyCommand(action) => match action {
                FrecencyCommandType::ClearSuggested => "Clear Suggested Items",
            },
            BuiltInFeature::SettingsCommand(action) => match action {
                SettingsCommandType::ResetWindowPositions => "Reset Window Positions",
                SettingsCommandType::ConfigureVercelApiKey => "Configure Vercel API Key",
                SettingsCommandType::ConfigureOpenAiApiKey => "Configure OpenAI API Key",
                SettingsCommandType::ConfigureAnthropicApiKey => "Configure Anthropic API Key",
                SettingsCommandType::ChooseTheme => "Open Theme Designer",
                SettingsCommandType::SelectMicrophone => "Select Microphone",
                SettingsCommandType::DisableWindowSnapping => "Disable Window Snapping",
                SettingsCommandType::SnapModeSimple => "Set Snap Mode: Simple",
                SettingsCommandType::SnapModeExpanded => "Set Snap Mode: Expanded",
                SettingsCommandType::SnapModePrecision => "Set Snap Mode: Precision",
            },
            BuiltInFeature::UtilityCommand(action) => match action {
                UtilityCommandType::MiniMainWindow => "Open Mini Launcher",
                UtilityCommandType::ScratchPad => "Open Scratch Pad",
                UtilityCommandType::QuickTerminal => "Open Quick Terminal",
                UtilityCommandType::ClaudeCode => "Open Claude Code Terminal",
                UtilityCommandType::ProcessManager => "Open Process Manager",
                UtilityCommandType::StopAllProcesses => "Stop All Running Scripts",
                UtilityCommandType::DoInCurrentApp => "Do in Current App",
                UtilityCommandType::TurnThisIntoCommand => "Turn Into Command",
                UtilityCommandType::CurrentAppCommands => "Open App Commands",
                UtilityCommandType::InspectCurrentContext => "Copy Current Context",
                UtilityCommandType::TraceCurrentAppIntent => "Copy App Intent Trace",
                UtilityCommandType::VerifyCurrentAppRecipe => "Verify App Recipe",
                UtilityCommandType::ReplayCurrentAppRecipe => "Replay App Recipe",
            },
            BuiltInFeature::KitStoreCommand(action) => match action {
                KitStoreCommandType::BrowseKits => "Browse Kit Store",
                KitStoreCommandType::InstalledKits => "Manage Installed Kits",
                KitStoreCommandType::UpdateAllKits => "Update All Kits",
            },
            BuiltInFeature::FileSearch => "Search Files",
            BuiltInFeature::Webcam => "Open Webcam",
            BuiltInFeature::Dictation => "Start Dictation Here",
            BuiltInFeature::DictationToAiHarness => "Start Dictation to AI",
            BuiltInFeature::DictationToFrontmostApp => "Start Dictation to App",
            BuiltInFeature::DictationToNotes => "Start Dictation to Notes",
            BuiltInFeature::Settings => "Open Script Kit Settings",
            BuiltInFeature::AcpHistory => "Open Conversation History",
        }
    }

    pub fn footer_action_text(&self) -> &'static str {
        match &self.feature {
            BuiltInFeature::ClipboardHistory => "History",
            BuiltInFeature::PasteSequentially => "Paste",
            BuiltInFeature::Favorites => "Favorites",
            BuiltInFeature::AppLauncher => "Apps",
            BuiltInFeature::App(_) => "Launch",
            BuiltInFeature::WindowSwitcher => "Switch",
            BuiltInFeature::DesignGallery => "Gallery",
            #[cfg(feature = "storybook")]
            BuiltInFeature::DesignExplorer => "Explorer",
            BuiltInFeature::AiChat => "Chat",
            BuiltInFeature::Notes => "Notes",
            BuiltInFeature::EmojiPicker => "Emoji",
            BuiltInFeature::MenuBarAction(_) => "Menu Item",
            BuiltInFeature::SystemAction(action) => match action {
                SystemActionType::EmptyTrash => "Empty Trash",
                SystemActionType::LockScreen => "Lock Screen",
                SystemActionType::Sleep => "Sleep",
                SystemActionType::Restart => "Restart",
                SystemActionType::ShutDown => "Shut Down",
                SystemActionType::LogOut => "Log Out",
                SystemActionType::ToggleDarkMode => "Dark Mode",
                SystemActionType::ShowDesktop => "Desktop",
                SystemActionType::MissionControl => "Mission Ctrl",
                SystemActionType::Launchpad => "Launchpad",
                SystemActionType::ForceQuitApps => "Force Quit",
                SystemActionType::Volume0 => "Volume 0%",
                SystemActionType::Volume25 => "Volume 25%",
                SystemActionType::Volume50 => "Volume 50%",
                SystemActionType::Volume75 => "Volume 75%",
                SystemActionType::Volume100 => "Volume 100%",
                SystemActionType::VolumeMute => "Mute",
                #[cfg(debug_assertions)]
                SystemActionType::TestConfirmation => "Test Dialog",
                SystemActionType::QuitScriptKit => "Quit",
                SystemActionType::ToggleDoNotDisturb => "Toggle DND",
                SystemActionType::StartScreenSaver => "Screen Saver",
                SystemActionType::OpenSystemPreferences => "macOS Settings",
                SystemActionType::OpenPrivacySettings => "Privacy",
                SystemActionType::OpenDisplaySettings => "Displays",
                SystemActionType::OpenSoundSettings => "Sound",
                SystemActionType::OpenNetworkSettings => "Network",
                SystemActionType::OpenKeyboardSettings => "Keyboard",
                SystemActionType::OpenBluetoothSettings => "Bluetooth",
                SystemActionType::OpenNotificationsSettings => "Notifications",
            },
            BuiltInFeature::NotesCommand(action) => match action {
                NotesCommandType::OpenNotes => "Open Notes",
                NotesCommandType::NewNote => "New Note",
                NotesCommandType::SearchNotes => "Search Notes",
                NotesCommandType::QuickCapture => "Quick Capture",
            },
            BuiltInFeature::AiCommand(action) => match action {
                AiCommandType::OpenAi | AiCommandType::MiniAi => "Open Chat",
                AiCommandType::NewConversation => "New Chat",
                AiCommandType::ClearConversation => "Clear Chat",
                AiCommandType::GenerateScript => "New Script",
                AiCommandType::GenerateScriptFromCurrentApp => "App Script",
                AiCommandType::SendScreenToAi => "Send Screen",
                AiCommandType::SendFocusedWindowToAi => "Send Window",
                AiCommandType::SendSelectedTextToAi => "Send Text",
                AiCommandType::SendBrowserTabToAi => "Send Tab",
                AiCommandType::SendScreenAreaToAi => "Select Area",
                AiCommandType::CreateAiPreset => "New Preset",
                AiCommandType::ImportAiPresets => "Import",
                AiCommandType::SearchAiPresets => "Find Presets",
                AiCommandType::ExportAiPresets => "Export",
            },
            BuiltInFeature::ScriptCommand(action) => match action {
                ScriptCommandType::NewScript => "New Script",
                ScriptCommandType::NewExtension => "New Bundle",
            },
            BuiltInFeature::PermissionCommand(action) => match action {
                PermissionCommandType::CheckPermissions => "Check Access",
                PermissionCommandType::RequestAccessibility => "Request Access",
                PermissionCommandType::OpenAccessibilitySettings => "Accessibility",
            },
            BuiltInFeature::FrecencyCommand(action) => match action {
                FrecencyCommandType::ClearSuggested => "Clear Suggested",
            },
            BuiltInFeature::SettingsCommand(action) => match action {
                SettingsCommandType::ResetWindowPositions => "Reset Windows",
                SettingsCommandType::ConfigureVercelApiKey => "Vercel Key",
                SettingsCommandType::ConfigureOpenAiApiKey => "OpenAI Key",
                SettingsCommandType::ConfigureAnthropicApiKey => "Anthropic Key",
                SettingsCommandType::ChooseTheme => "Theme",
                SettingsCommandType::SelectMicrophone => "Microphone",
                SettingsCommandType::DisableWindowSnapping => "Snap Off",
                SettingsCommandType::SnapModeSimple => "Simple Snap",
                SettingsCommandType::SnapModeExpanded => "Expanded Snap",
                SettingsCommandType::SnapModePrecision => "Precision Snap",
            },
            BuiltInFeature::UtilityCommand(action) => match action {
                UtilityCommandType::MiniMainWindow => "Mini Launcher",
                UtilityCommandType::ScratchPad => "Scratch Pad",
                UtilityCommandType::QuickTerminal => "Terminal",
                UtilityCommandType::ClaudeCode => "Claude Code",
                UtilityCommandType::ProcessManager => "Processes",
                UtilityCommandType::StopAllProcesses => "Stop Scripts",
                UtilityCommandType::DoInCurrentApp => "Current App",
                UtilityCommandType::TurnThisIntoCommand => "Save Command",
                UtilityCommandType::CurrentAppCommands => "App Commands",
                UtilityCommandType::InspectCurrentContext => "Copy Context",
                UtilityCommandType::TraceCurrentAppIntent => "Trace Intent",
                UtilityCommandType::VerifyCurrentAppRecipe => "Verify Recipe",
                UtilityCommandType::ReplayCurrentAppRecipe => "Replay Recipe",
            },
            BuiltInFeature::KitStoreCommand(action) => match action {
                KitStoreCommandType::BrowseKits => "Browse Kits",
                KitStoreCommandType::InstalledKits => "Installed Kits",
                KitStoreCommandType::UpdateAllKits => "Update Kits",
            },
            BuiltInFeature::FileSearch => "Files",
            BuiltInFeature::Webcam => "Webcam",
            BuiltInFeature::Dictation => "Dictate Here",
            BuiltInFeature::DictationToAiHarness => "Dictate AI",
            BuiltInFeature::DictationToFrontmostApp => "Dictate App",
            BuiltInFeature::DictationToNotes => "Dictate Notes",
            BuiltInFeature::Settings => "Kit Settings",
            BuiltInFeature::AcpHistory => "History",
        }
    }
}
// --- merged from part_001.rs ---
/// Get the list of enabled built-in entries based on configuration
///
/// # Arguments
/// * `config` - The built-in features configuration
///
/// # Returns
/// A vector of enabled built-in entries that should appear in the main search
///
/// Note: AppLauncher built-in is no longer used since apps now appear directly
/// in the main search results. The config option is retained for future use
/// (e.g., to control whether apps are included in search at all).
pub fn get_builtin_entries(config: &BuiltInConfig) -> Vec<BuiltInEntry> {
    let mut entries = Vec::new();

    // --- merged from part_001_entries/entries_000.rs ---
    {
        if config.clipboard_history {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/clipboard-history",
                "Clipboard History",
                "Open clipboard history to view, search, and reuse copied items",
                vec!["clipboard", "history", "paste", "copy"],
                BuiltInFeature::ClipboardHistory,
                "clipboard",
            ));
            debug!("Added Clipboard History built-in entry");

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/paste-sequentially",
                "Paste Next Clipboard Item",
                "Paste the next item from clipboard history",
                vec!["paste", "sequential", "clipboard", "batch", "paseq"],
                BuiltInFeature::PasteSequentially,
                "clipboard-paste",
            ));
            debug!("Added Paste Sequentially built-in entry");
        }

        // Note: AppLauncher built-in removed - apps now appear directly in main search
        // The app_launcher config flag is kept for future use (e.g., to disable app search entirely)
        if config.app_launcher {
            debug!("app_launcher enabled - apps will appear in main search");
        }

        if config.window_switcher {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/window-switcher",
                "Window Switcher",
                "Open window switcher to focus, tile, and manage open windows",
                vec!["window", "switch", "tile", "focus", "manage", "switcher"],
                BuiltInFeature::WindowSwitcher,
                "app-window",
            ));
            debug!("Added Window Switcher built-in entry");
        }

        // ACP Chat is always available from the launcher.
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/ai-chat",
            "ACP Chat",
            "Open ACP Chat with fresh context",
            vec![
                "ai",
                "harness",
                "chat",
                "assistant",
                "claude",
                "gpt",
                "llm",
                "tab",
            ],
            BuiltInFeature::AiChat,
            "bot",
        ));
        debug!("Added ACP Chat built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/acp-history",
            "Conversation History",
            "Browse and manage past AI conversations",
            vec!["history", "conversations", "chat", "ai", "past", "previous"],
            BuiltInFeature::AcpHistory,
            "history",
        ));
        debug!("Added ACP History built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/favorites",
            "Favorites",
            "Open your starred scripts and shortcuts",
            vec![
                "favorites",
                "favorite",
                "starred",
                "star",
                "pinned",
                "saved",
            ],
            BuiltInFeature::Favorites,
            "star",
        ));
        debug!("Added Favorites built-in entry");

        // Notes is always available (Open Notes absorbs legacy "Notes" entry keywords)
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/emoji-picker",
            "Emoji Picker",
            "Pick an emoji from the built-in list and copy it to the clipboard",
            vec!["emoji", "picker", "symbols", "unicode", "copy", "clipboard"],
            BuiltInFeature::EmojiPicker,
            "smile",
        ));
        debug!("Added Emoji Picker built-in entry");

        // Design Gallery is only available in debug builds (developer tool)
        #[cfg(debug_assertions)]
        {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/design-gallery",
                "Design Gallery",
                "Open the design gallery to browse separator styles and icon variations",
                vec![
                    "design",
                    "gallery",
                    "separator",
                    "icon",
                    "style",
                    "theme",
                    "variations",
                ],
                BuiltInFeature::DesignGallery,
                "palette",
            ));
            debug!("Added Design Gallery built-in entry");

            // Test Confirmation entry for testing confirmation UI
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/test-confirmation",
                "Test Confirmation",
                "Open the confirmation dialog test tool (dev only)",
                vec!["test", "confirmation", "dev", "debug"],
                BuiltInFeature::SystemAction(SystemActionType::TestConfirmation),
                "flask-conical",
            ));
            debug!("Added Test Confirmation built-in entry");
        }

        // Design Explorer is only available when storybook feature is enabled
        #[cfg(feature = "storybook")]
        {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/design-explorer",
                "Design Explorer",
                "Open the in-app explorer to compare story variants and adopt a winner",
                vec![
                    "design",
                    "explorer",
                    "storybook",
                    "compare",
                    "variant",
                    "adopt",
                    "ui",
                ],
                BuiltInFeature::DesignExplorer,
                "flask-conical",
            ));
            debug!("Added Design Explorer built-in entry");
        }

        // =========================================================================
    }

    // --- merged from part_001_entries/entries_001.rs ---
    {
        // System Actions
        // =========================================================================

        // Power management
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/empty-trash",
            "Empty Trash",
            "Empty the macOS Trash",
            vec!["empty", "trash", "delete", "clean"],
            BuiltInFeature::SystemAction(SystemActionType::EmptyTrash),
            "trash-2",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/lock-screen",
            "Lock Screen",
            "Lock the screen",
            vec!["lock", "screen", "security"],
            BuiltInFeature::SystemAction(SystemActionType::LockScreen),
            "lock",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/sleep",
            "Sleep",
            "Put the system to sleep",
            vec!["sleep", "suspend", "power"],
            BuiltInFeature::SystemAction(SystemActionType::Sleep),
            "moon",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/restart",
            "Restart",
            "Restart the system",
            vec!["restart", "reboot", "power"],
            BuiltInFeature::SystemAction(SystemActionType::Restart),
            "refresh-cw",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/shut-down",
            "Shut Down",
            "Shut down the system",
            vec!["shut", "down", "shutdown", "power", "off"],
            BuiltInFeature::SystemAction(SystemActionType::ShutDown),
            "power",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/log-out",
            "Log Out",
            "Log out the current user",
            vec!["log", "out", "logout", "user"],
            BuiltInFeature::SystemAction(SystemActionType::LogOut),
            "log-out",
        ));

        // UI controls
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/toggle-dark-mode",
            "Toggle Dark Mode",
            "Switch between light and dark appearance",
            vec!["dark", "mode", "light", "appearance", "theme", "toggle"],
            BuiltInFeature::SystemAction(SystemActionType::ToggleDarkMode),
            "sun-moon",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/show-desktop",
            "Show Desktop",
            "Hide all windows to reveal the desktop",
            vec!["show", "desktop", "hide", "windows"],
            BuiltInFeature::SystemAction(SystemActionType::ShowDesktop),
            "monitor",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/mission-control",
            "Mission Control",
            "Show all windows and desktops",
            vec!["mission", "control", "expose", "spaces", "windows"],
            BuiltInFeature::SystemAction(SystemActionType::MissionControl),
            "layout-grid",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/launchpad",
            "Launchpad",
            "Open Launchpad to show all applications",
            vec!["launchpad", "apps", "applications"],
            BuiltInFeature::SystemAction(SystemActionType::Launchpad),
            "rocket",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/force-quit",
            "Force Quit Apps",
            "Select and force quit running applications",
            vec![
                "force",
                "quit",
                "kill",
                "apps",
                "unresponsive",
                "terminate",
                "stop",
            ],
            BuiltInFeature::SystemAction(SystemActionType::ForceQuitApps),
            "triangle-alert",
        ));

        // Volume controls (preset levels)
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-0",
            "Volume 0%",
            "Set system volume to 0% (mute)",
            vec!["volume", "mute", "0", "percent", "zero", "off"],
            BuiltInFeature::SystemAction(SystemActionType::Volume0),
            "volume-x",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-25",
            "Volume 25%",
            "Set system volume to 25%",
            vec!["volume", "25", "percent", "low", "quiet"],
            BuiltInFeature::SystemAction(SystemActionType::Volume25),
            "volume",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-50",
            "Volume 50%",
            "Set system volume to 50%",
            vec!["volume", "50", "percent", "half", "medium"],
            BuiltInFeature::SystemAction(SystemActionType::Volume50),
            "volume-1",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-75",
            "Volume 75%",
            "Set system volume to 75%",
            vec!["volume", "75", "percent", "high", "loud"],
            BuiltInFeature::SystemAction(SystemActionType::Volume75),
            "volume-1",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-100",
            "Volume 100%",
            "Set system volume to 100% (max)",
            vec!["volume", "100", "percent", "max", "full"],
            BuiltInFeature::SystemAction(SystemActionType::Volume100),
            "volume-2",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-mute",
            "Toggle Mute",
            "Toggle system audio mute on or off",
            vec!["mute", "unmute", "volume", "sound", "audio", "toggle"],
            BuiltInFeature::SystemAction(SystemActionType::VolumeMute),
            "volume-x",
        ));

        // App control
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/quit-script-kit",
            "Quit Script Kit",
            "Quit the Script Kit application",
            vec!["quit", "exit", "close", "script", "kit", "app"],
            BuiltInFeature::SystemAction(SystemActionType::QuitScriptKit),
            "log-out",
        ));

        // System utilities
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/toggle-dnd",
            "Toggle Do Not Disturb",
            "Toggle Focus / Do Not Disturb mode on or off",
            vec![
                "do",
                "not",
                "disturb",
                "dnd",
                "focus",
                "notifications",
                "toggle",
            ],
            BuiltInFeature::SystemAction(SystemActionType::ToggleDoNotDisturb),
            "bell-off",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/screen-saver",
            "Start Screen Saver",
            "Activate the screen saver",
            vec!["screen", "saver", "screensaver"],
            BuiltInFeature::SystemAction(SystemActionType::StartScreenSaver),
            "image",
        ));

        // System Preferences
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/system-preferences",
            "macOS System Settings",
            "Open macOS System Settings",
            vec!["system", "settings", "preferences", "prefs"],
            BuiltInFeature::SystemAction(SystemActionType::OpenSystemPreferences),
            "settings",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/privacy-settings",
            "Privacy & Security Settings",
            "Open Privacy & Security settings",
            vec!["privacy", "security", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenPrivacySettings),
            "shield",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/display-settings",
            "Display Settings",
            "Open Display settings",
            vec!["display", "monitor", "screen", "resolution", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenDisplaySettings),
            "monitor",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/sound-settings",
            "Sound Settings",
            "Open Sound settings",
            vec!["sound", "audio", "volume", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenSoundSettings),
            "volume-2",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/network-settings",
            "Network Settings",
            "Open Network settings",
            vec!["network", "wifi", "ethernet", "internet", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenNetworkSettings),
            "wifi",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/keyboard-settings",
            "Keyboard Settings",
            "Open Keyboard settings",
            vec!["keyboard", "shortcuts", "input", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenKeyboardSettings),
            "keyboard",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/bluetooth-settings",
            "Bluetooth Settings",
            "Open Bluetooth settings",
            vec!["bluetooth", "wireless", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenBluetoothSettings),
            "bluetooth",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/notifications-settings",
            "Notification Settings",
            "Open Notifications settings",
            vec!["notifications", "alerts", "banners", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenNotificationsSettings),
            "bell",
        ));

        // NOTE: Window Actions removed - now handled by window-management extension
        // SDK tileWindow() function still works via protocol messages in execute_script.rs

        // =========================================================================
    }

    // --- merged from part_001_entries/entries_002.rs ---
    {
        // Notes Commands
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/open-notes",
            "Open Notes",
            "Open the Notes window",
            vec![
                "open",
                "notes",
                "window",
                "note",
                "new",
                "create",
                "search",
                "find",
                "scratch",
                "scratchpad",
                "memo",
                "markdown",
                "write",
                "text",
            ],
            BuiltInFeature::NotesCommand(NotesCommandType::OpenNotes),
            "notebook-pen",
        ));

        // NewNote and SearchNotes intentionally collapse into OpenNotes until
        // note creation / search have distinct execution paths.

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/quick-capture",
            "Quick Capture",
            "Capture a new note without opening the full Notes window",
            vec!["quick", "capture", "note", "fast"],
            BuiltInFeature::NotesCommand(NotesCommandType::QuickCapture),
            "zap",
        ));

        // =========================================================================
        // AI Commands
        // =========================================================================

        // Legacy AI window commands (OpenAi, MiniAi, NewConversation, ClearConversation)
        // are no longer registered — all AI entry points route to the harness terminal.

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/generate-script-with-ai",
            GENERATE_SCRIPT_WITH_AI_LABEL,
            "Open ACP Chat to generate a Script Kit script from your prompt text",
            vec![
                "generate",
                "script",
                "ai",
                "create",
                "code",
                "typescript",
                "shift",
                "tab",
            ],
            BuiltInFeature::AiCommand(AiCommandType::GenerateScript),
            "brain",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/generate-script-from-current-app",
            GENERATE_SCRIPT_FROM_CURRENT_APP_LABEL,
            "Generate a Script Kit script using the frontmost app's menu, selection, and browser context",
            vec![
                "generate",
                "script",
                "current",
                "app",
                "automation",
                "menu",
                "frontmost",
                "context",
                "browser",
                "selection",
                "ai",
            ],
            BuiltInFeature::AiCommand(AiCommandType::GenerateScriptFromCurrentApp),
            "wand-sparkles",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/send-screen-to-ai",
            "Send Screen to AI",
            "Capture the full screen and send it to ACP Chat",
            vec![
                "send",
                "screen",
                "screenshot",
                "ai",
                "chat",
                "capture",
                "image",
            ],
            BuiltInFeature::AiCommand(AiCommandType::SendScreenToAi),
            "camera",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/send-focused-window-to-ai",
            "Send Focused Window to AI",
            "Capture the focused window and send it to ACP Chat",
            vec![
                "send",
                "window",
                "focused",
                "ai",
                "chat",
                "capture",
                "screenshot",
            ],
            BuiltInFeature::AiCommand(AiCommandType::SendFocusedWindowToAi),
            "app-window",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/send-selected-text-to-ai",
            "Send Selected Text to AI",
            "Send the currently selected text to ACP Chat",
            vec![
                "send",
                "selected",
                "text",
                "selection",
                "ai",
                "chat",
                "copy",
            ],
            BuiltInFeature::AiCommand(AiCommandType::SendSelectedTextToAi),
            "text-cursor-input",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/send-browser-tab-to-ai",
            "Send Focused Browser Tab to AI",
            "Send the current browser tab URL to ACP Chat",
            vec![
                "send", "browser", "tab", "url", "safari", "chrome", "ai", "chat", "web",
            ],
            BuiltInFeature::AiCommand(AiCommandType::SendBrowserTabToAi),
            "globe",
        ));

        // Preview AI commands are currently stubs and should only appear in debug builds.
        // NOTE: builtin-send-screen-area-to-ai removed — no real region-context attachment yet.
        #[cfg(debug_assertions)]
        {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/create-ai-preset",
                "Create AI Preset",
                "Create a reusable AI preset with name, system prompt, and model",
                vec!["create", "ai", "preset", "template", "save", "new"],
                BuiltInFeature::AiCommand(AiCommandType::CreateAiPreset),
                "flask-conical",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/import-ai-presets",
                "Import AI Presets",
                "Import AI presets from a JSON file via file picker",
                vec!["import", "ai", "preset", "template", "file", "load"],
                BuiltInFeature::AiCommand(AiCommandType::ImportAiPresets),
                "download",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/export-ai-presets",
                "Export AI Presets",
                "Export AI presets to a JSON file via file picker",
                vec![
                    "export", "ai", "preset", "template", "file", "save", "backup",
                ],
                BuiltInFeature::AiCommand(AiCommandType::ExportAiPresets),
                "upload",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/search-ai-presets",
                "Search AI Presets",
                "Search and browse saved AI presets to start a new chat",
                vec!["search", "ai", "preset", "template", "browse", "find"],
                BuiltInFeature::AiCommand(AiCommandType::SearchAiPresets),
                "search",
            ));
        }

        // =========================================================================
        // Script Commands
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/new-script",
            "New Script (Template)",
            "Create a new Script Kit script from a guided starter template",
            vec![
                "new",
                "script",
                "create",
                "template",
                "starter",
                "boilerplate",
                "scaffold",
                "code",
            ],
            BuiltInFeature::ScriptCommand(ScriptCommandType::NewScript),
            "plus",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/new-extension",
            "New Scriptlet Bundle",
            "Create a new scriptlet bundle with YAML frontmatter and scriptlet examples",
            vec![
                "new",
                "scriptlet",
                "bundle",
                "extension",
                "frontmatter",
                "yaml",
                "snippet",
                "create",
            ],
            BuiltInFeature::ScriptCommand(ScriptCommandType::NewExtension),
            "sparkles",
        ));

        // =========================================================================
        // Permission Commands
        // =========================================================================
        // These actions are intentionally routed through the Script Kit Settings hub
        // instead of competing as top-level launcher commands.

        // =========================================================================
        // Frecency/Suggested Commands
        // =========================================================================

        // Clear Suggested is intentionally routed through Script Kit Settings
        // instead of competing as a top-level launcher command.

        // =========================================================================
    }

    // --- merged from part_001_entries/entries_003.rs ---
    {
        // Settings Hub
        // =========================================================================
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/settings",
            "Script Kit Settings",
            "Configure Script Kit settings, API keys, themes, window positions, and more",
            vec![
                "settings",
                "preferences",
                "config",
                "configure",
                "options",
                "setup",
                "permissions",
                "permission",
                "accessibility",
                "privacy",
            ],
            BuiltInFeature::Settings,
            "settings",
        ));

        // Settings Commands
        // =========================================================================
        // These actions are intentionally routed through the Script Kit Settings hub
        // instead of competing as top-level launcher commands.

        let snap_mode = crate::window_control::current_snap_mode();

        if snap_mode != crate::window_control::SnapMode::Off {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/disable-window-snapping",
                "Disable Window Snapping",
                "Turn off drag snapping and hide snap overlays until a snap mode is re-enabled",
                vec![
                    "snap", "snapping", "window", "disable", "off", "drag", "overlay",
                ],
                BuiltInFeature::SettingsCommand(SettingsCommandType::DisableWindowSnapping),
                "ban",
            ));
        }

        if snap_mode != crate::window_control::SnapMode::Simple {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/snap-mode-simple",
                "Snap Mode: Simple",
                "Use halves, quadrants, center, and almost-maximize targets while dragging windows",
                vec![
                    "snap",
                    "snapping",
                    "window",
                    "simple",
                    "basic",
                    "halves",
                    "quadrants",
                ],
                BuiltInFeature::SettingsCommand(SettingsCommandType::SnapModeSimple),
                "square-split-horizontal",
            ));
        }

        if snap_mode != crate::window_control::SnapMode::Expanded {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/snap-mode-expanded",
                "Snap Mode: Expanded",
                "Use halves, quadrants, thirds, and two-thirds targets while dragging windows",
                vec![
                    "snap",
                    "snapping",
                    "window",
                    "expanded",
                    "thirds",
                    "two thirds",
                    "default",
                ],
                BuiltInFeature::SettingsCommand(SettingsCommandType::SnapModeExpanded),
                "columns-3",
            ));
        }

        if snap_mode != crate::window_control::SnapMode::Precision {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/snap-mode-precision",
                "Snap Mode: Precision",
                "Use the full snap grid including sixths for finer placements",
                vec![
                    "snap",
                    "snapping",
                    "window",
                    "precision",
                    "sixths",
                    "grid",
                    "advanced",
                ],
                BuiltInFeature::SettingsCommand(SettingsCommandType::SnapModePrecision),
                "grid-3x2",
            ));
        }

        // =========================================================================
        // Utility Commands
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/scratch-pad",
            "Scratch Pad",
            "Open a scratch pad editor for notes and code (auto-saves to disk)",
            vec![
                "scratch",
                "pad",
                "scratchpad",
                "notes",
                "editor",
                "write",
                "text",
                "quick",
                "jot",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::ScratchPad),
            "notebook-pen",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/mini-main-window",
            "Mini Main Window",
            "Open a compact launcher with the same search and actions",
            vec![
                "mini",
                "compact",
                "launcher",
                "main",
                "window",
                "minimal",
                "spotlight",
                "raycast",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::MiniMainWindow),
            "search",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/quick-terminal",
            "Quick Terminal",
            "Open a quick terminal for running shell commands",
            vec![
                "terminal", "term", "shell", "bash", "zsh", "command", "quick", "console", "cli",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::QuickTerminal),
            "square-terminal",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/claude-code",
            "Claude Code Terminal",
            "Open Claude Code in the terminal surface used for CLI agent sessions",
            vec![
                "claude",
                "code",
                "terminal",
                "cli",
                "repl",
                "anthropic",
                "harness",
                "tab ai",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::ClaudeCode),
            "bot",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/process-manager",
            "Process Manager",
            "Inspect running scripts and copy their process details",
            vec![
                "process", "running", "scripts", "jobs", "pid", "inspect", "manage", "kill",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::ProcessManager),
            "activity",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/do-in-current-app",
            crate::menu_bar::current_app_commands::DO_IN_CURRENT_APP_LABEL,
            "Browse, search, and run menu bar commands from the frontmost app; if no direct command exists, Script Kit generates a script",
            vec![
                "do",
                "current",
                "app",
                "browse",
                "show",
                "search",
                "command",
                "commands",
                "run",
                "action",
                "menu",
                "menubar",
                "frontmost",
                "execute",
                "automation",
                "automate",
                "intent",
                "script",
                "shortcut",
                // Overlap with GenerateScript / GenerateScriptFromCurrentApp so
                // this recipe-backed entry outranks the weaker generic paths.
                "generate",
                "ai",
                "create",
                "code",
                "context",
                "browser",
                "selection",
                // Collapsed alias coverage from Turn This Into a Command.
                "turn this into a command",
                "teach",
                "save",
                "recipe",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::DoInCurrentApp),
            "target",
        ));
        // Turn This Into a Command intentionally collapses into Do in Current App
        // in the launcher registry. The execution path remains for compatibility.

        #[cfg(debug_assertions)]
        {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/inspect-current-context",
                "Inspect Current Context",
                "Capture a deterministic JSON snapshot of what Script Kit can currently see and copy it to the clipboard",
                vec![
                    "inspect",
                    "current",
                    "context",
                    "snapshot",
                    "json",
                    "diagnostics",
                    "debug",
                    "clipboard",
                    "mcp",
                    "agent",
                    "ai",
                ],
                BuiltInFeature::UtilityCommand(UtilityCommandType::InspectCurrentContext),
                "compass",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/trace-current-app-intent",
                "Trace Current App Intent",
                "Dry-run a current-app request and copy a machine-readable JSON trace showing normalized input, candidate commands, route selection, and script prompt preview without executing anything",
                vec![
                    "trace",
                    "current",
                    "app",
                    "intent",
                    "dry",
                    "run",
                    "preview",
                    "debug",
                    "diagnostics",
                    "routing",
                    "json",
                    "agent",
                    "automation",
                    "menu",
                    "command",
                    "shortcut",
                ],
                BuiltInFeature::UtilityCommand(UtilityCommandType::TraceCurrentAppIntent),
                "flask-conical",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/verify-current-app-recipe",
                crate::menu_bar::current_app_commands::VERIFY_CURRENT_APP_RECIPE_LABEL,
                "Read a currentAppCommand recipe JSON from the clipboard, re-capture the frontmost app, and copy a machine-readable drift report",
                vec![
                    "verify",
                    "current",
                    "app",
                    "recipe",
                    "command",
                    "json",
                    "validate",
                    "drift",
                    "trace",
                    "clipboard",
                    "agent",
                    "automation",
                ],
                BuiltInFeature::UtilityCommand(UtilityCommandType::VerifyCurrentAppRecipe),
                "microscope",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/replay-current-app-recipe",
                crate::menu_bar::current_app_commands::REPLAY_CURRENT_APP_RECIPE_LABEL,
                "Read a currentAppCommand recipe JSON from the clipboard, verify it against live context, and replay the resolved action only when it still matches",
                vec![
                    "replay",
                    "run",
                    "current",
                    "app",
                    "recipe",
                    "command",
                    "json",
                    "clipboard",
                    "verify",
                    "execute",
                    "automation",
                    "drift",
                ],
                BuiltInFeature::UtilityCommand(UtilityCommandType::ReplayCurrentAppRecipe),
                "repeat",
            ));
        }

        // =========================================================================
        // Kit Store Commands (debug-only stubs)
        // =========================================================================

        #[cfg(debug_assertions)]
        {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/browse-kit-store",
                "Browse Kit Store",
                "Browse available kits from the Kit Store",
                vec!["kit", "store", "browse", "search", "discover", "extensions"],
                BuiltInFeature::KitStoreCommand(KitStoreCommandType::BrowseKits),
                "search",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/manage-installed-kits",
                "Manage Installed Kits",
                "View and manage kits already installed from the Kit Store",
                vec![
                    "kit",
                    "store",
                    "installed",
                    "manage",
                    "extensions",
                    "packages",
                ],
                BuiltInFeature::KitStoreCommand(KitStoreCommandType::InstalledKits),
                "package",
            ));

            entries.push(BuiltInEntry::new_with_icon(
                "builtin/update-all-kits",
                "Update All Kits",
                "Update every installed kit to the latest version",
                vec![
                    "kit",
                    "store",
                    "update",
                    "upgrade",
                    "refresh",
                    "all",
                    "extensions",
                ],
                BuiltInFeature::KitStoreCommand(KitStoreCommandType::UpdateAllKits),
                "refresh-cw",
            ));
        }

        // =========================================================================
        // File Search (Directory Navigation)
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/file-search",
            "Search Files",
            "Browse directories, search files, and open results",
            vec![
                "file",
                "search",
                "find",
                "directory",
                "folder",
                "browse",
                "navigate",
                "path",
                "open",
                "explorer",
            ],
            BuiltInFeature::FileSearch,
            "folder-search",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/webcam",
            "Webcam",
            "Open the webcam prompt and capture a photo",
            vec!["webcam", "camera", "capture", "photo", "image"],
            BuiltInFeature::Webcam,
            "camera",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/dictation",
            "Dictate Here",
            "Voice dictation for the current destination",
            vec![
                "dictation",
                "voice",
                "speech",
                "microphone",
                "transcribe",
                "whisper",
            ],
            BuiltInFeature::Dictation,
            "mic",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/dictation-to-ai",
            "Dictate to AI",
            "Voice dictation — speak and submit to ACP Chat",
            vec![
                "dictate ai",
                "voice ai",
                "speech ai",
                "dictation harness",
                "speak to ai",
            ],
            BuiltInFeature::DictationToAiHarness,
            "mic",
        ));
    }

    debug!(count = entries.len(), "Built-in entries loaded");
    entries
}
// --- merged from part_002.rs ---
// ============================================================================
// Menu Bar Item Conversion
// ============================================================================

/// Expand a macOS shortcut display string (e.g. `⌘T`) into searchable aliases.
///
/// Returns normalized forms including the lowercased original, a compact form
/// (`cmdt`), a spaced form (`cmd t`), and a chord form (`cmd+t`).
pub fn shortcut_search_tokens(display: &str) -> Vec<String> {
    let display_lower = display.to_lowercase();
    let mut chord_parts: Vec<String> = Vec::new();

    for ch in display_lower.chars() {
        match ch {
            '⌘' => chord_parts.push("cmd".into()),
            '⌥' => chord_parts.push("option".into()),
            '⌃' => chord_parts.push("ctrl".into()),
            '⇧' => chord_parts.push("shift".into()),
            '↩' | '⏎' => chord_parts.push("return".into()),
            '⌫' => chord_parts.push("delete".into()),
            '←' => chord_parts.push("left".into()),
            '→' => chord_parts.push("right".into()),
            '↑' => chord_parts.push("up".into()),
            '↓' => chord_parts.push("down".into()),
            '+' | ' ' => {}
            other => chord_parts.push(other.to_string()),
        }
    }

    let compact = chord_parts.join("");
    let spaced = chord_parts.join(" ");
    let chord = chord_parts.join("+");

    let mut tokens = vec![display_lower];
    if !compact.is_empty() {
        tokens.push(compact);
    }
    if !spaced.is_empty() {
        tokens.push(spaced);
    }
    if !chord.is_empty() {
        tokens.push(chord);
    }

    tokens.sort();
    tokens.dedup();
    tokens
}

/// A machine-readable receipt for a menu-bar filter operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuBarFilterReceipt {
    pub query: String,
    pub normalized_query: String,
    pub total_entries: usize,
    pub matched_entries: usize,
}

fn menu_bar_entry_matches_normalized_query(entry: &BuiltInEntry, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }

    let mut blob = String::new();
    blob.push_str(&entry.name.to_lowercase());
    blob.push(' ');
    blob.push_str(&entry.description.to_lowercase());

    for keyword in &entry.keywords {
        blob.push(' ');
        blob.push_str(&keyword.to_lowercase());
    }

    normalized_query
        .split_whitespace()
        .all(|term| blob.contains(term))
}

/// Match a menu-bar entry against a user query.
///
/// Returns `true` for an empty query. Otherwise applies AND semantics across
/// whitespace-delimited terms, searching the entry's name, description, and
/// keywords.
#[allow(dead_code)] // Used by integration tests via lib.rs
pub fn menu_bar_entry_matches_query(entry: &BuiltInEntry, query: &str) -> bool {
    menu_bar_entry_matches_normalized_query(entry, &query.trim().to_lowercase())
}

/// Filter menu-bar entries and return a machine-readable receipt for logging.
pub fn filter_menu_bar_entries<'a>(
    entries: &'a [BuiltInEntry],
    query: &str,
) -> (Vec<(usize, &'a BuiltInEntry)>, MenuBarFilterReceipt) {
    let normalized_query = query.trim().to_lowercase();
    let filtered: Vec<(usize, &'a BuiltInEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| menu_bar_entry_matches_normalized_query(entry, &normalized_query))
        .collect();

    let receipt = MenuBarFilterReceipt {
        query: query.to_string(),
        normalized_query,
        total_entries: entries.len(),
        matched_entries: filtered.len(),
    };

    (filtered, receipt)
}

/// Convert menu bar items to built-in entries for search
///
/// This flattens the menu hierarchy into searchable entries, skipping the
/// Apple menu (first item) and only including leaf items (no submenus).
///
/// # Arguments
/// * `items` - The menu bar items from the frontmost application
/// * `bundle_id` - The bundle identifier of the application (e.g., "com.apple.Safari")
/// * `app_name` - The display name of the application (e.g., "Safari")
///
/// # Returns
/// A vector of `BuiltInEntry` items that can be added to search results
#[allow(dead_code)] // Will be used when menu bar integration is complete
pub fn menu_bar_items_to_entries(
    items: &[MenuBarItem],
    bundle_id: &str,
    app_name: &str,
) -> Vec<BuiltInEntry> {
    let mut entries = Vec::new();

    // Skip first item (Apple menu)
    for item in items.iter().skip(1) {
        flatten_menu_item(item, bundle_id, app_name, &[], &mut entries);
    }

    debug!(
        count = entries.len(),
        bundle_id = bundle_id,
        app_name = app_name,
        "Menu bar items converted to entries"
    );
    entries
}
/// Recursively flatten a menu item and its children into entries
#[allow(dead_code)] // Will be used when menu bar integration is complete
fn flatten_menu_item(
    item: &MenuBarItem,
    bundle_id: &str,
    app_name: &str,
    parent_path: &[String],
    entries: &mut Vec<BuiltInEntry>,
) {
    // Skip separators and disabled items
    if item.title.is_empty() || item.title == "-" || item.is_separator() || !item.enabled {
        return;
    }

    let mut current_path = parent_path.to_vec();
    current_path.push(item.title.clone());

    // Only add leaf items (items without children) as entries
    if item.children.is_empty() {
        let id = format!(
            "menubar-{}-{}",
            bundle_id,
            current_path.join("-").to_lowercase().replace(' ', "-")
        );
        let name = current_path.join(" → ");
        let description = if let Some(ref shortcut) = item.shortcut {
            format!("{}  {}", app_name, shortcut.to_display_string())
        } else {
            app_name.to_string()
        };
        let mut keywords: Vec<String> = current_path.iter().map(|s| s.to_lowercase()).collect();
        keywords.push(app_name.to_lowercase());

        if let Some(ref shortcut) = item.shortcut {
            keywords.extend(shortcut_search_tokens(&shortcut.to_display_string()));
        }

        keywords.sort();
        keywords.dedup();
        let icon = get_menu_icon(&current_path[0]);

        entries.push(BuiltInEntry {
            id,
            name,
            description,
            keywords,
            feature: BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: bundle_id.to_string(),
                menu_path: current_path,
                enabled: item.enabled,
                shortcut: item.shortcut.as_ref().map(|s| s.to_display_string()),
            }),
            icon: Some(icon.to_string()),
            group: BuiltInGroup::MenuBar,
        });
    } else {
        // Recurse into children
        for child in &item.children {
            flatten_menu_item(child, bundle_id, app_name, &current_path, entries);
        }
    }
}
/// Get an appropriate icon for a top-level menu
#[allow(dead_code)] // Will be used when menu bar integration is complete
fn get_menu_icon(top_menu: &str) -> &'static str {
    match top_menu.to_lowercase().as_str() {
        "file" => "folder",
        "edit" => "clipboard",
        "view" => "eye",
        "window" => "app-window",
        "help" => "circle-help",
        "format" => "palette",
        "tools" => "wrench",
        "go" => "arrow-right",
        "bookmarks" | "favorites" => "star",
        "history" => "clock",
        "develop" | "developer" => "code",
        _ => "pin",
    }
}
// --- merged from part_003.rs ---
#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    use crate::config::BuiltInConfig;
    #[test]
    fn test_builtin_config_default() {
        let config = BuiltInConfig::default();
        assert!(config.clipboard_history);
        assert!(config.app_launcher);
        assert!(config.window_switcher);
    }
    #[test]
    fn test_builtin_config_custom() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: true,
            window_switcher: false,
        };
        assert!(!config.clipboard_history);
        assert!(config.app_launcher);
        assert!(!config.window_switcher);
    }
    #[test]
    fn test_get_builtin_entries_all_enabled() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Core built-ins: Clipboard history, window switcher, AI chat, Notes, design gallery
        // Plus: system actions (28), window actions (6), notes commands (3), AI commands (1),
        // script commands (2), permission commands (3) = 43 new entries
        // Total: 5 + 43 = 48
        assert!(entries.len() >= 5); // At minimum the core built-ins should exist

        // Check clipboard history entry
        let clipboard = entries.iter().find(|e| e.id == "builtin/clipboard-history");
        assert!(clipboard.is_some());
        let clipboard = clipboard.unwrap();
        assert_eq!(clipboard.name, "Clipboard History");
        assert_eq!(clipboard.feature, BuiltInFeature::ClipboardHistory);
        assert!(clipboard.keywords.contains(&"clipboard".to_string()));
        assert!(clipboard.keywords.contains(&"history".to_string()));
        assert!(clipboard.keywords.contains(&"paste".to_string()));
        assert!(clipboard.keywords.contains(&"copy".to_string()));

        // Check paste sequentially entry
        let paste_sequentially = entries
            .iter()
            .find(|e| e.id == "builtin/paste-sequentially");
        assert!(paste_sequentially.is_some());
        let paste_sequentially = paste_sequentially.unwrap();
        assert_eq!(paste_sequentially.name, "Paste Next Clipboard Item");
        assert_eq!(
            paste_sequentially.feature,
            BuiltInFeature::PasteSequentially
        );
        assert!(paste_sequentially.keywords.contains(&"paste".to_string()));
        assert!(paste_sequentially
            .keywords
            .contains(&"sequential".to_string()));
        assert!(paste_sequentially
            .keywords
            .contains(&"clipboard".to_string()));
        assert!(paste_sequentially.keywords.contains(&"batch".to_string()));
        assert!(paste_sequentially.keywords.contains(&"paseq".to_string()));

        // Check window switcher entry
        let window_switcher = entries.iter().find(|e| e.id == "builtin/window-switcher");
        assert!(window_switcher.is_some());
        let window_switcher = window_switcher.unwrap();
        assert_eq!(window_switcher.name, "Window Switcher");
        assert_eq!(window_switcher.feature, BuiltInFeature::WindowSwitcher);
        assert!(window_switcher.keywords.contains(&"window".to_string()));
        assert!(window_switcher.keywords.contains(&"switch".to_string()));
        assert!(window_switcher.keywords.contains(&"tile".to_string()));
        assert!(window_switcher.keywords.contains(&"focus".to_string()));
        assert!(window_switcher.keywords.contains(&"manage".to_string()));
        assert!(window_switcher.keywords.contains(&"switcher".to_string()));

        // Check AI harness entry
        let ai_chat = entries.iter().find(|e| e.id == "builtin/ai-chat");
        assert!(ai_chat.is_some());
        let ai_chat = ai_chat.unwrap();
        assert_eq!(ai_chat.name, "ACP Chat");
        assert_eq!(ai_chat.feature, BuiltInFeature::AiChat);
        assert!(ai_chat.keywords.contains(&"ai".to_string()));
        assert!(ai_chat.keywords.contains(&"harness".to_string()));
        assert!(ai_chat.keywords.contains(&"claude".to_string()));
        assert!(ai_chat.keywords.contains(&"gpt".to_string()));

        // Check Emoji Picker entry
        let emoji_picker = entries.iter().find(|e| e.id == "builtin/emoji-picker");
        assert!(emoji_picker.is_some());
        let emoji_picker = emoji_picker.unwrap();
        assert_eq!(emoji_picker.name, "Emoji Picker");
        assert_eq!(emoji_picker.feature, BuiltInFeature::EmojiPicker);
        assert!(emoji_picker.keywords.contains(&"emoji".to_string()));
        assert!(emoji_picker.keywords.contains(&"picker".to_string()));

        // Note: App Launcher built-in removed - apps now appear directly in main search
    }
    #[test]
    fn test_get_builtin_entries_clipboard_only() {
        let config = BuiltInConfig {
            clipboard_history: true,
            app_launcher: false,
            window_switcher: false,
        };
        let entries = get_builtin_entries(&config);

        // Check that core entries exist (plus all the new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin/clipboard-history"));
        assert!(entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
        assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin/design-gallery"));

        // Window switcher should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin/window-switcher"));
    }
    #[test]
    fn test_get_builtin_entries_app_launcher_only() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: true,
            window_switcher: false,
        };
        let entries = get_builtin_entries(&config);

        // App launcher no longer creates a built-in entry (apps appear in main search)
        // But ACP Chat, Notes and Design Gallery are always enabled (plus new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin/design-gallery"));

        // Clipboard history should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin/clipboard-history"));
        assert!(!entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
    }
    #[test]
    fn test_get_builtin_entries_none_enabled() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: false,
            window_switcher: false,
        };
        let entries = get_builtin_entries(&config);

        // ACP Chat, Notes, and Design Gallery are always enabled (plus new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin/design-gallery"));

        // Clipboard history and window switcher should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin/clipboard-history"));
        assert!(!entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
        assert!(!entries.iter().any(|e| e.id == "builtin/window-switcher"));
    }
    #[test]
    fn test_get_builtin_entries_window_switcher_only() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: false,
            window_switcher: true,
        };
        let entries = get_builtin_entries(&config);

        // Window switcher + ACP Chat + Notes + Design Gallery (always enabled, plus new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin/window-switcher"));
        assert!(entries.iter().any(|e| e.id == "builtin/ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin/design-gallery"));

        // Verify window switcher has correct properties
        let window_switcher = entries
            .iter()
            .find(|e| e.id == "builtin/window-switcher")
            .unwrap();
        assert_eq!(window_switcher.feature, BuiltInFeature::WindowSwitcher);
        assert_eq!(window_switcher.icon, Some("app-window".to_string()));

        // Clipboard history should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin/clipboard-history"));
        assert!(!entries.iter().any(|e| e.id == "builtin/paste-sequentially"));
    }
    #[test]
    fn test_builtin_feature_equality() {
        assert_eq!(
            BuiltInFeature::ClipboardHistory,
            BuiltInFeature::ClipboardHistory
        );
        assert_eq!(BuiltInFeature::AppLauncher, BuiltInFeature::AppLauncher);
        assert_eq!(
            BuiltInFeature::WindowSwitcher,
            BuiltInFeature::WindowSwitcher
        );
        assert_eq!(BuiltInFeature::DesignGallery, BuiltInFeature::DesignGallery);
        assert_eq!(BuiltInFeature::AiChat, BuiltInFeature::AiChat);
        assert_eq!(BuiltInFeature::Favorites, BuiltInFeature::Favorites);
        assert_eq!(BuiltInFeature::EmojiPicker, BuiltInFeature::EmojiPicker);
        assert_eq!(
            BuiltInFeature::PasteSequentially,
            BuiltInFeature::PasteSequentially
        );
        assert_ne!(
            BuiltInFeature::ClipboardHistory,
            BuiltInFeature::AppLauncher
        );
        assert_ne!(
            BuiltInFeature::ClipboardHistory,
            BuiltInFeature::WindowSwitcher
        );
        assert_ne!(BuiltInFeature::AppLauncher, BuiltInFeature::WindowSwitcher);
        assert_ne!(
            BuiltInFeature::DesignGallery,
            BuiltInFeature::ClipboardHistory
        );
        assert_ne!(
            BuiltInFeature::PasteSequentially,
            BuiltInFeature::ClipboardHistory
        );
        assert_ne!(BuiltInFeature::AiChat, BuiltInFeature::ClipboardHistory);
        assert_ne!(BuiltInFeature::AiChat, BuiltInFeature::DesignGallery);
        assert_ne!(BuiltInFeature::Favorites, BuiltInFeature::ClipboardHistory);
        assert_ne!(
            BuiltInFeature::EmojiPicker,
            BuiltInFeature::ClipboardHistory
        );

        // Test App variant
        assert_eq!(
            BuiltInFeature::App("Safari".to_string()),
            BuiltInFeature::App("Safari".to_string())
        );
        assert_ne!(
            BuiltInFeature::App("Safari".to_string()),
            BuiltInFeature::App("Chrome".to_string())
        );
        assert_ne!(
            BuiltInFeature::App("Safari".to_string()),
            BuiltInFeature::AppLauncher
        );
    }
    #[test]
    fn test_builtin_entry_new() {
        let entry = BuiltInEntry::new(
            "test-id",
            "Test Entry",
            "Test description",
            vec!["test", "keyword"],
            BuiltInFeature::ClipboardHistory,
        );

        assert_eq!(entry.id, "builtin/test-id");
        assert_eq!(entry.name, "Test Entry");
        assert_eq!(entry.description, "Test description");
        assert_eq!(
            entry.keywords,
            vec!["test".to_string(), "keyword".to_string()]
        );
        assert_eq!(entry.feature, BuiltInFeature::ClipboardHistory);
        assert_eq!(entry.icon, None);
    }
    #[test]
    fn test_builtin_entry_new_with_icon() {
        let entry = BuiltInEntry::new_with_icon(
            "test-id",
            "Test Entry",
            "Test description",
            vec!["test"],
            BuiltInFeature::ClipboardHistory,
            "clipboard",
        );

        assert_eq!(entry.id, "builtin/test-id");
        assert_eq!(entry.name, "Test Entry");
        assert_eq!(entry.icon, Some("clipboard".to_string()));
    }
    #[test]
    fn test_builtin_entry_clone() {
        let entry = BuiltInEntry::new_with_icon(
            "test-id",
            "Test Entry",
            "Test description",
            vec!["test"],
            BuiltInFeature::AppLauncher,
            "rocket",
        );

        let cloned = entry.clone();
        assert_eq!(entry.id, cloned.id);
        assert_eq!(entry.name, cloned.name);
        assert_eq!(entry.description, cloned.description);
        assert_eq!(entry.keywords, cloned.keywords);
        assert_eq!(entry.feature, cloned.feature);
        assert_eq!(entry.icon, cloned.icon);
    }
    #[test]
    fn test_builtin_config_clone() {
        let config = BuiltInConfig {
            clipboard_history: true,
            app_launcher: false,
            window_switcher: true,
        };

        let cloned = config.clone();
        assert_eq!(config.clipboard_history, cloned.clipboard_history);
        assert_eq!(config.app_launcher, cloned.app_launcher);
        assert_eq!(config.window_switcher, cloned.window_switcher);
    }
    #[test]
    fn test_system_action_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that system action entries exist
        assert!(entries.iter().any(|e| e.id == "builtin/empty-trash"));
        assert!(entries.iter().any(|e| e.id == "builtin/lock-screen"));
        assert!(entries.iter().any(|e| e.id == "builtin/toggle-dark-mode"));
        // Volume presets
        assert!(entries.iter().any(|e| e.id == "builtin/volume-0"));
        assert!(entries.iter().any(|e| e.id == "builtin/volume-50"));
        assert!(entries.iter().any(|e| e.id == "builtin/volume-100"));
        assert!(entries.iter().any(|e| e.id == "builtin/system-preferences"));
    }
    // NOTE: test_window_action_entries_exist removed - window actions now in extension

    #[test]
    fn test_notes_command_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Open Notes absorbs New Note and Search Notes (same execution path).
        assert!(entries.iter().any(|e| e.id == "builtin/open-notes"));
        assert!(
            !entries.iter().any(|e| e.id == "builtin/new-note"),
            "builtin/new-note collapsed into builtin-open-notes"
        );
        assert!(
            !entries.iter().any(|e| e.id == "builtin/search-notes"),
            "builtin/search-notes collapsed into builtin-open-notes"
        );
        assert!(entries.iter().any(|e| e.id == "builtin/quick-capture"));

        // Verify Open Notes absorbed the keywords from the collapsed entries
        let open_notes = entries
            .iter()
            .find(|e| e.id == "builtin/open-notes")
            .unwrap();
        assert!(open_notes.keywords.contains(&"new".to_string()));
        assert!(open_notes.keywords.contains(&"create".to_string()));
        assert!(open_notes.keywords.contains(&"search".to_string()));
        assert!(open_notes.keywords.contains(&"find".to_string()));
    }
    #[test]
    fn test_get_builtin_entries_includes_open_notes_and_generate_script() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let open_notes = entries.iter().find(|e| e.id == "builtin/open-notes");
        assert!(open_notes.is_some(), "builtin/open-notes should exist");
        assert_eq!(
            open_notes.unwrap().feature,
            BuiltInFeature::NotesCommand(NotesCommandType::OpenNotes)
        );

        // Legacy AI window commands (OpenAi, MiniAi, NewConversation) are no longer registered.
        assert!(
            entries.iter().find(|e| e.id == "builtin/open-ai").is_none(),
            "builtin/open-ai should be removed (routes to harness now)"
        );
        assert!(
            entries.iter().find(|e| e.id == "builtin/mini-ai").is_none(),
            "builtin/mini-ai should be removed (routes to harness now)"
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == "builtin/new-conversation")
                .is_none(),
            "builtin/new-conversation should be removed (routes to harness now)"
        );

        let generate_script = entries
            .iter()
            .find(|e| e.id == "builtin/generate-script-with-ai");
        assert!(
            generate_script.is_some(),
            "builtin/generate-script-with-ai should still exist (routes to harness)"
        );
        let generate_script = generate_script.unwrap();
        assert_eq!(
            generate_script.feature,
            BuiltInFeature::AiCommand(AiCommandType::GenerateScript)
        );
    }
    #[test]
    fn test_get_builtin_entries_gates_stub_commands_to_debug_builds() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let stub_ids = [
            "builtin/browse-kit-store",
            "builtin/manage-installed-kits",
            "builtin/update-all-kits",
            "builtin/create-ai-preset",
            "builtin/import-ai-presets",
            "builtin/export-ai-presets",
            "builtin/search-ai-presets",
        ];

        #[cfg(debug_assertions)]
        {
            for id in stub_ids {
                assert!(
                    entries.iter().any(|e| e.id == id),
                    "stub command {id} should be visible in debug builds"
                );
            }
        }

        #[cfg(not(debug_assertions))]
        {
            for id in stub_ids {
                assert!(
                    !entries.iter().any(|e| e.id == id),
                    "stub command {id} should be hidden in release builds"
                );
            }
        }
    }

    #[test]
    fn test_get_builtin_entries_includes_favorites_command() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let favorites = entries.iter().find(|e| e.id == "builtin/favorites");
        assert!(favorites.is_some(), "builtin/favorites should exist");

        let favorites = favorites.unwrap();
        assert_eq!(favorites.name, "Favorites");
        assert_eq!(favorites.feature, BuiltInFeature::Favorites);
        assert!(
            favorites
                .keywords
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case("star")),
            "Favorites command should be discoverable with 'star'"
        );
    }
    #[test]
    fn test_script_command_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that script command entries exist
        assert!(entries.iter().any(|e| e.id == "builtin/new-script"));
        assert!(entries.iter().any(|e| e.id == "builtin/new-extension"));
    }
    #[test]
    fn test_new_creation_commands_are_discoverable() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let new_script = entries
            .iter()
            .find(|e| e.id == "builtin/new-script")
            .expect("builtin/new-script should exist");
        assert!(
            new_script.name.to_lowercase().contains("new"),
            "New Script entry name should prominently include 'new'"
        );
        assert!(
            new_script
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case("template")),
            "New Script entry should be discoverable via 'template'"
        );
        assert!(
            new_script
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case("starter")),
            "New Script entry should be discoverable via 'starter'"
        );

        let new_extension = entries
            .iter()
            .find(|e| e.id == "builtin/new-extension")
            .expect("builtin/new-extension should exist");
        assert!(
            new_extension
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case("scriptlet")),
            "New Scriptlet entry should be discoverable via 'scriptlet'"
        );
        assert!(
            new_extension
                .keywords
                .iter()
                .any(|k| k.eq_ignore_ascii_case("frontmatter")),
            "New Scriptlet entry should be discoverable via 'frontmatter'"
        );
    }
    #[test]
    fn test_permission_commands_are_routed_through_settings_hub() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);
        for id in [
            "builtin/check-permissions",
            "builtin/request-accessibility",
            "builtin/accessibility-settings",
        ] {
            assert!(
                entries.iter().all(|entry| entry.id != id),
                "{id} should be routed through the Script Kit Settings hub instead of the top-level registry"
            );
        }

        let settings = entries
            .iter()
            .find(|entry| entry.id == "builtin/settings")
            .expect("builtin/settings must remain registered");
        for keyword in ["permissions", "permission", "accessibility", "privacy"] {
            assert!(
                settings.keywords.iter().any(|item| item == keyword),
                "Script Kit Settings should preserve {keyword} discoverability from collapsed permission commands"
            );
        }
    }
    #[test]
    fn test_system_action_type_equality() {
        assert_eq!(SystemActionType::EmptyTrash, SystemActionType::EmptyTrash);
        assert_ne!(SystemActionType::EmptyTrash, SystemActionType::LockScreen);
    }
    // NOTE: test_window_action_type_equality removed - WindowActionType no longer in builtins

    #[test]
    fn test_builtin_feature_system_action() {
        let feature = BuiltInFeature::SystemAction(SystemActionType::ToggleDarkMode);
        assert_eq!(
            feature,
            BuiltInFeature::SystemAction(SystemActionType::ToggleDarkMode)
        );
        assert_ne!(
            feature,
            BuiltInFeature::SystemAction(SystemActionType::Sleep)
        );
    }
    // --- merged from part_001.rs ---
    // NOTE: test_builtin_feature_window_action removed - WindowAction no longer in BuiltInFeature

    #[test]
    fn test_file_search_builtin_exists() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that FileSearch entry exists
        let file_search = entries.iter().find(|e| e.id == "builtin/file-search");
        assert!(
            file_search.is_some(),
            "FileSearch builtin should exist in the main menu"
        );

        let file_search = file_search.unwrap();
        assert_eq!(file_search.name, "Search Files");
        assert_eq!(file_search.feature, BuiltInFeature::FileSearch);
        assert!(file_search.keywords.contains(&"file".to_string()));
        assert!(file_search.keywords.contains(&"search".to_string()));
        assert!(file_search.keywords.contains(&"find".to_string()));
        assert!(file_search.keywords.contains(&"directory".to_string()));
        assert!(file_search.icon.is_some());
    }
    #[test]
    fn test_get_builtin_entries_includes_process_manager_command() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let process_manager = entries.iter().find(|e| e.id == "builtin/process-manager");
        assert!(
            process_manager.is_some(),
            "Process Manager builtin should exist in the main menu"
        );

        let process_manager = process_manager.unwrap();
        assert_eq!(
            process_manager.feature,
            BuiltInFeature::UtilityCommand(UtilityCommandType::ProcessManager)
        );
        assert!(process_manager.keywords.iter().any(|k| k == "process"));
        assert!(process_manager.keywords.iter().any(|k| k == "running"));
        assert!(process_manager.keywords.iter().any(|k| k == "kill"));
    }

    #[test]
    fn test_get_builtin_entries_includes_mini_main_window_command() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let mini_main_window = entries.iter().find(|e| e.id == "builtin/mini-main-window");
        assert!(
            mini_main_window.is_some(),
            "Mini Main Window builtin should exist in the main menu"
        );

        let mini_main_window = mini_main_window.unwrap();
        assert_eq!(
            mini_main_window.feature,
            BuiltInFeature::UtilityCommand(UtilityCommandType::MiniMainWindow)
        );
        assert_eq!(mini_main_window.icon.as_deref(), Some("search"));
        assert!(mini_main_window.keywords.iter().any(|k| k == "mini"));
        assert!(mini_main_window.keywords.iter().any(|k| k == "compact"));
        assert!(mini_main_window.keywords.iter().any(|k| k == "launcher"));
    }

    #[test]
    fn test_process_manager_absorbs_stop_all_processes_command() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        assert!(
            entries
                .iter()
                .all(|entry| entry.id != "builtin/stop-all-processes"),
            "Stop all running scripts should be exposed through Process Manager instead of the top-level registry"
        );
    }

    #[test]
    fn test_settings_hub_absorbs_direct_settings_commands() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        for id in [
            "builtin/reset-window-positions",
            "builtin/configure-vercel-api",
            "builtin/configure-openai-api",
            "builtin/configure-anthropic-api",
            "builtin/choose-theme",
            "builtin/select-microphone",
        ] {
            assert!(
                entries.iter().all(|entry| entry.id != id),
                "{id} should be routed through the Script Kit Settings hub instead of the top-level registry"
            );
        }
    }

    #[test]
    fn test_settings_hub_absorbs_clear_suggested_command() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        assert!(
            entries
                .iter()
                .all(|entry| entry.id != "builtin/clear-suggested"),
            "Clear Suggested should be exposed through Script Kit Settings instead of the top-level registry"
        );
    }

    #[test]
    fn test_dictation_hub_absorbs_forced_app_and_notes_entries() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        for id in ["builtin/dictation-to-app", "builtin/dictation-to-notes"] {
            assert!(
                entries.iter().all(|entry| entry.id != id),
                "{id} should remain an internal dictation route instead of a top-level launcher entry"
            );
        }

        assert!(
            entries
                .iter()
                .any(|entry| entry.id == "builtin/dictation-to-ai"),
            "builtin/dictation-to-ai should remain available as an explicit ACP Chat route"
        );
    }

    #[test]
    fn test_builtin_descriptions_use_clear_action_phrasing() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let notes = entries
            .iter()
            .find(|e| e.id == "builtin/open-notes")
            .unwrap();
        assert_eq!(notes.description, "Open the Notes window");

        let quick_capture = entries
            .iter()
            .find(|e| e.id == "builtin/quick-capture")
            .unwrap();
        assert_eq!(
            quick_capture.description,
            "Capture a new note without opening the full Notes window"
        );

        let file_search = entries
            .iter()
            .find(|e| e.id == "builtin/file-search")
            .unwrap();
        assert_eq!(
            file_search.description,
            "Browse directories, search files, and open results"
        );

        let webcam = entries.iter().find(|e| e.id == "builtin/webcam").unwrap();
        assert_eq!(
            webcam.description,
            "Open the webcam prompt and capture a photo"
        );
    }

    #[test]
    fn test_builtin_enter_labels_use_action_specific_phrasing() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let open_notes = entries
            .iter()
            .find(|e| e.id == "builtin/open-notes")
            .unwrap();
        assert_eq!(open_notes.default_action_text(), "Open Notes");

        let quick_capture = entries
            .iter()
            .find(|e| e.id == "builtin/quick-capture")
            .unwrap();
        assert_eq!(quick_capture.default_action_text(), "Start Quick Capture");

        let file_search = entries
            .iter()
            .find(|e| e.id == "builtin/file-search")
            .unwrap();
        assert_eq!(file_search.default_action_text(), "Search Files");

        let settings = entries.iter().find(|e| e.id == "builtin/settings").unwrap();
        assert_eq!(settings.default_action_text(), "Open Script Kit Settings");

        let process_manager = entries
            .iter()
            .find(|e| e.id == "builtin/process-manager")
            .unwrap();
        assert_eq!(
            process_manager.default_action_text(),
            "Open Process Manager"
        );

        let clipboard = entries
            .iter()
            .find(|e| e.id == "builtin/clipboard-history")
            .unwrap();
        assert_eq!(clipboard.default_action_text(), "Open Clipboard History");

        let volume = entries
            .iter()
            .find(|e| e.id == "builtin/volume-25")
            .unwrap();
        assert_eq!(volume.default_action_text(), "Set Volume to 25%");

        let send_browser_tab = entries
            .iter()
            .find(|e| e.id == "builtin/send-browser-tab-to-ai")
            .unwrap();
        assert_eq!(send_browser_tab.default_action_text(), "Send Tab to AI");

        let inspect_context = entries
            .iter()
            .find(|e| e.id == "builtin/inspect-current-context")
            .unwrap();
        assert_eq!(
            inspect_context.default_action_text(),
            "Copy Current Context"
        );

        let webcam = entries.iter().find(|e| e.id == "builtin/webcam").unwrap();
        assert_eq!(webcam.default_action_text(), "Open Webcam");
    }

    #[test]
    fn test_builtin_footer_labels_stay_compact() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        for entry in entries {
            let word_count = entry.footer_action_text().split_whitespace().count();
            assert!(
                word_count <= 2,
                "footer label '{}' for '{}' should stay compact",
                entry.footer_action_text(),
                entry.name
            );
        }

        let settings = get_builtin_entries(&config)
            .into_iter()
            .find(|e| e.id == "builtin/settings")
            .unwrap();
        assert_eq!(settings.footer_action_text(), "Kit Settings");
    }

    #[test]
    fn test_menu_bar_entries_use_menu_specific_enter_label() {
        let entry = BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-file-new-tab",
            "File → New Tab",
            "Safari  ⌘T",
            vec!["file".into(), "new tab".into(), "safari".into()],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["File".into(), "New Tab".into()],
                enabled: true,
                shortcut: Some("⌘T".into()),
            }),
            Some("file".into()),
            BuiltInGroup::MenuBar,
        );

        assert_eq!(entry.default_action_text(), "Execute Menu Item");
        assert_eq!(entry.footer_action_text(), "Menu Item");
    }
    #[test]
    fn test_file_search_feature_equality() {
        assert_eq!(BuiltInFeature::FileSearch, BuiltInFeature::FileSearch);
        assert_ne!(BuiltInFeature::FileSearch, BuiltInFeature::ClipboardHistory);
        assert_ne!(BuiltInFeature::FileSearch, BuiltInFeature::Notes);
    }
    #[test]
    fn test_system_action_hud_message_volume_presets() {
        assert_eq!(
            system_action_hud_message(SystemActionType::Volume0, None),
            Some("Volume 0%".to_string())
        );
        assert_eq!(
            system_action_hud_message(SystemActionType::Volume50, None),
            Some("Volume 50%".to_string())
        );
        assert_eq!(
            system_action_hud_message(SystemActionType::Volume100, None),
            Some("Volume 100%".to_string())
        );
        assert_eq!(
            system_action_hud_message(SystemActionType::VolumeMute, None),
            Some("Volume Muted".to_string())
        );
    }
    #[test]
    fn test_system_action_hud_message_dark_mode() {
        assert_eq!(
            system_action_hud_message(SystemActionType::ToggleDarkMode, Some(true)),
            Some("Dark Mode On".to_string())
        );
        assert_eq!(
            system_action_hud_message(SystemActionType::ToggleDarkMode, Some(false)),
            Some("Dark Mode Off".to_string())
        );
        assert_eq!(
            system_action_hud_message(SystemActionType::ToggleDarkMode, None),
            Some("Dark Mode Toggled".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Builtin metadata audit — unique IDs/names and minimum metadata
    // -----------------------------------------------------------------------

    #[test]
    fn test_builtin_entries_have_unique_ids_and_names() {
        use std::collections::BTreeMap;

        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        assert!(!entries.is_empty(), "Expected at least one builtin entry");

        let mut id_counts: BTreeMap<&str, usize> = BTreeMap::new();
        let mut name_counts: BTreeMap<&str, usize> = BTreeMap::new();

        for entry in &entries {
            *id_counts.entry(&entry.id).or_default() += 1;
            *name_counts.entry(&entry.name).or_default() += 1;
        }

        let duplicate_ids: Vec<&str> = id_counts
            .iter()
            .filter(|(_, &count)| count > 1)
            .map(|(&id, _)| id)
            .collect();
        let duplicate_names: Vec<&str> = name_counts
            .iter()
            .filter(|(_, &count)| count > 1)
            .map(|(&name, _)| name)
            .collect();

        assert!(
            duplicate_ids.is_empty(),
            "Duplicate builtin ids: {duplicate_ids:?}"
        );
        assert!(
            duplicate_names.is_empty(),
            "Duplicate builtin names: {duplicate_names:?}"
        );
    }

    #[test]
    fn test_builtin_entries_use_stable_prefix_and_minimum_metadata() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        let invalid_ids: Vec<&str> = entries
            .iter()
            .filter(|entry| !entry.id.starts_with("builtin/"))
            .map(|entry| entry.id.as_str())
            .collect();
        let missing_descriptions: Vec<&str> = entries
            .iter()
            .filter(|entry| entry.description.trim().is_empty())
            .map(|entry| entry.id.as_str())
            .collect();
        let sparse_keywords: Vec<String> = entries
            .iter()
            .filter(|entry| entry.keywords.len() < 3)
            .map(|entry| format!("{} ({})", entry.id, entry.keywords.len()))
            .collect();

        assert!(
            invalid_ids.is_empty(),
            "Builtin ids must start with 'builtin-': {invalid_ids:?}"
        );
        assert!(
            missing_descriptions.is_empty(),
            "Builtin entries missing descriptions: {missing_descriptions:?}"
        );
        assert!(
            sparse_keywords.is_empty(),
            "Builtin entries need at least 3 keywords: {sparse_keywords:?}"
        );
    }

    // =====================================================================
    // Current App Commands collapsed into Do in Current App
    // =====================================================================

    #[test]
    fn current_app_commands_builtin_is_no_longer_registered() {
        let entries = get_builtin_entries(&BuiltInConfig::default());
        let found = entries
            .iter()
            .find(|e| e.id == "builtin/current-app-commands");
        assert!(
            found.is_none(),
            "builtin/current-app-commands should no longer be registered (collapsed into Do in Current App)"
        );
    }

    #[test]
    fn do_in_current_app_has_commands_keywords() {
        let entries = get_builtin_entries(&BuiltInConfig::default());
        let entry = entries
            .iter()
            .find(|e| e.id == "builtin/do-in-current-app")
            .expect("builtin/do-in-current-app must be registered");
        assert!(
            entry.keywords.contains(&"commands".to_string()),
            "Do in Current App should include 'commands' keyword from collapsed entry"
        );
        assert!(
            entry.keywords.contains(&"menubar".to_string()),
            "Do in Current App should include 'menubar' keyword from collapsed entry"
        );
    }

    #[test]
    fn turn_this_into_command_builtin_is_no_longer_registered() {
        let entries = get_builtin_entries(&BuiltInConfig::default());
        let found = entries
            .iter()
            .find(|e| e.id == "builtin/turn-this-into-a-command");
        assert!(
            found.is_none(),
            "builtin/turn-this-into-a-command should no longer be registered (collapsed into Do in Current App)"
        );
    }

    #[test]
    fn do_in_current_app_absorbs_turn_this_into_command_keywords() {
        let entries = get_builtin_entries(&BuiltInConfig::default());
        let entry = entries
            .iter()
            .find(|e| e.id == "builtin/do-in-current-app")
            .expect("builtin/do-in-current-app must be registered");
        assert!(
            entry
                .keywords
                .contains(&"turn this into a command".to_string()),
            "Do in Current App should include the collapsed turn-this alias phrase"
        );
        assert!(
            entry.keywords.contains(&"teach".to_string()),
            "Do in Current App should include the collapsed 'teach' keyword"
        );
        assert!(
            entry.keywords.contains(&"recipe".to_string()),
            "Do in Current App should include the collapsed 'recipe' keyword"
        );
    }

    #[test]
    fn menu_bar_leaf_name_returns_last_segment() {
        let entry = BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-file-new-tab",
            "File → New Tab",
            "Safari  ⌘T",
            vec!["file".to_string(), "new".to_string(), "tab".to_string()],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["File".into(), "New Tab".into()],
                enabled: true,
                shortcut: Some("⌘T".into()),
            }),
            Some("folder".into()),
            BuiltInGroup::MenuBar,
        );

        assert_eq!(entry.leaf_name(), "New Tab");
    }

    #[test]
    fn leaf_name_single_segment() {
        let entry = BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-quit",
            "Quit",
            "Safari",
            vec!["quit".to_string()],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["Quit".into()],
                enabled: true,
                shortcut: None,
            }),
            Some("pin".into()),
            BuiltInGroup::MenuBar,
        );

        assert_eq!(entry.leaf_name(), "Quit");
    }

    #[test]
    fn leaf_name_core_builtin_returns_full_name() {
        let entry = BuiltInEntry::new_with_icon(
            "builtin/clipboard-history",
            "Clipboard History",
            "View and paste from clipboard",
            vec!["clipboard"],
            BuiltInFeature::ClipboardHistory,
            "clipboard",
        );
        // Core group → leaf_name returns full name unchanged
        assert_eq!(entry.leaf_name(), "Clipboard History");
    }

    #[test]
    fn builtin_frecency_key_uses_id() {
        let entry = BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-file-new-tab",
            "File → New Tab",
            "Safari  ⌘T",
            vec!["file".to_string(), "new".to_string(), "tab".to_string()],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["File".into(), "New Tab".into()],
                enabled: true,
                shortcut: Some("⌘T".into()),
            }),
            Some("folder".into()),
            BuiltInGroup::MenuBar,
        );

        let frecency_key = format!("builtin:{}", entry.id);
        assert_eq!(
            frecency_key,
            "builtin:menubar-com.apple.Safari-file-new-tab"
        );
        // Key is derived from id, not from name
        assert_ne!(frecency_key, format!("builtin:{}", entry.name));
    }

    #[test]
    fn menu_bar_items_to_entries_representative_path() {
        use crate::menu_bar::{KeyboardShortcut, MenuBarItem, ModifierFlags};

        let items = vec![
            // Apple menu (index 0) — should be skipped
            MenuBarItem {
                title: "Apple".into(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![0],
            },
            // File menu with children
            MenuBarItem {
                title: "File".into(),
                enabled: true,
                shortcut: None,
                children: vec![
                    MenuBarItem {
                        title: "New Tab".into(),
                        enabled: true,
                        shortcut: Some(KeyboardShortcut::new("T".into(), ModifierFlags::COMMAND)),
                        children: vec![],
                        ax_element_path: vec![1, 0],
                    },
                    // Separator — should be skipped
                    MenuBarItem::separator(vec![1, 1]),
                    // Disabled item — should be skipped
                    MenuBarItem {
                        title: "Close All".into(),
                        enabled: false,
                        shortcut: None,
                        children: vec![],
                        ax_element_path: vec![1, 2],
                    },
                    MenuBarItem {
                        title: "New Window".into(),
                        enabled: true,
                        shortcut: Some(KeyboardShortcut::new("N".into(), ModifierFlags::COMMAND)),
                        children: vec![],
                        ax_element_path: vec![1, 3],
                    },
                ],
                ax_element_path: vec![1],
            },
        ];

        let entries = menu_bar_items_to_entries(&items, "com.apple.Safari", "Safari");

        // Should have exactly 2 entries (New Tab, New Window) — separator and disabled skipped
        assert_eq!(entries.len(), 2, "expected 2 entries, got: {entries:?}");

        // First entry: File → New Tab
        let new_tab = &entries[0];
        assert_eq!(new_tab.name, "File → New Tab");
        assert_eq!(new_tab.id, "menubar-com.apple.Safari-file-new-tab");
        assert!(
            new_tab.description.contains("Safari"),
            "description should contain app name"
        );
        assert!(
            new_tab.description.contains("⌘T"),
            "description should contain shortcut"
        );
        assert_eq!(new_tab.group, BuiltInGroup::MenuBar);

        // Verify MenuBarAction info
        if let BuiltInFeature::MenuBarAction(ref info) = new_tab.feature {
            assert_eq!(info.bundle_id, "com.apple.Safari");
            assert_eq!(info.menu_path, vec!["File", "New Tab"]);
            assert!(info.enabled);
            assert_eq!(info.shortcut, Some("⌘T".into()));
        } else {
            panic!("expected MenuBarAction feature");
        }

        // Second entry: File → New Window
        let new_window = &entries[1];
        assert_eq!(new_window.name, "File → New Window");
        assert_eq!(new_window.id, "menubar-com.apple.Safari-file-new-window");
    }

    #[test]
    fn menu_bar_items_to_entries_no_shortcut() {
        use crate::menu_bar::MenuBarItem;

        let items = vec![
            // Apple menu — skipped
            MenuBarItem {
                title: "Apple".into(),
                enabled: true,
                shortcut: None,
                children: vec![],
                ax_element_path: vec![0],
            },
            // Edit menu with one child, no shortcut
            MenuBarItem {
                title: "Edit".into(),
                enabled: true,
                shortcut: None,
                children: vec![MenuBarItem {
                    title: "Paste".into(),
                    enabled: true,
                    shortcut: None,
                    children: vec![],
                    ax_element_path: vec![1, 0],
                }],
                ax_element_path: vec![1],
            },
        ];

        let entries = menu_bar_items_to_entries(&items, "com.apple.Finder", "Finder");

        assert_eq!(entries.len(), 1);
        // No shortcut → description is just the app name
        assert_eq!(entries[0].description, "Finder");
    }

    #[test]
    fn menu_bar_items_to_entries_empty_items() {
        let entries: Vec<BuiltInEntry> =
            menu_bar_items_to_entries(&[], "com.example.App", "Example");
        assert!(entries.is_empty());
    }

    #[test]
    fn shortcut_search_tokens_expands_symbolic_shortcuts() {
        let tokens = shortcut_search_tokens("⌘⇧T");

        assert!(tokens.contains(&"⌘⇧t".to_string()));
        assert!(tokens.contains(&"cmdshiftt".to_string()));
        assert!(tokens.contains(&"cmd shift t".to_string()));
        assert!(tokens.contains(&"cmd+shift+t".to_string()));
    }

    #[test]
    fn menu_bar_entry_matches_shortcut_aliases() {
        let entry = BuiltInEntry::new_with_group(
            "menubar-com.apple.Safari-file-new-tab",
            "File → New Tab",
            "Safari  ⌘T",
            vec![
                "file".into(),
                "new".into(),
                "tab".into(),
                "safari".into(),
                "⌘t".into(),
                "cmdt".into(),
                "cmd t".into(),
                "cmd+t".into(),
            ],
            BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                bundle_id: "com.apple.Safari".into(),
                menu_path: vec!["File".into(), "New Tab".into()],
                enabled: true,
                shortcut: Some("⌘T".into()),
            }),
            Some("folder".into()),
            BuiltInGroup::MenuBar,
        );

        assert!(menu_bar_entry_matches_query(&entry, "cmd+t"));
        assert!(menu_bar_entry_matches_query(&entry, "cmd t"));
        assert!(menu_bar_entry_matches_query(&entry, "safari new"));
        assert!(!menu_bar_entry_matches_query(&entry, "cmd+p"));
    }

    #[test]
    fn filter_menu_bar_entries_reports_counts() {
        let entries = vec![
            BuiltInEntry::new_with_group(
                "menubar-com.apple.Safari-file-new-tab",
                "File → New Tab",
                "Safari  ⌘T",
                vec!["file".into(), "new".into(), "tab".into(), "cmd+t".into()],
                BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                    bundle_id: "com.apple.Safari".into(),
                    menu_path: vec!["File".into(), "New Tab".into()],
                    enabled: true,
                    shortcut: Some("⌘T".into()),
                }),
                Some("folder".into()),
                BuiltInGroup::MenuBar,
            ),
            BuiltInEntry::new_with_group(
                "menubar-com.apple.Safari-file-new-window",
                "File → New Window",
                "Safari  ⌘N",
                vec!["file".into(), "new".into(), "window".into(), "cmd+n".into()],
                BuiltInFeature::MenuBarAction(MenuBarActionInfo {
                    bundle_id: "com.apple.Safari".into(),
                    menu_path: vec!["File".into(), "New Window".into()],
                    enabled: true,
                    shortcut: Some("⌘N".into()),
                }),
                Some("folder".into()),
                BuiltInGroup::MenuBar,
            ),
        ];

        let (filtered, receipt) = filter_menu_bar_entries(&entries, "cmd+t");

        assert_eq!(receipt.query, "cmd+t");
        assert_eq!(receipt.normalized_query, "cmd+t");
        assert_eq!(receipt.total_entries, 2);
        assert_eq!(receipt.matched_entries, 1);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1.name, "File → New Tab");
    }
    #[test]
    fn test_generate_script_from_current_app_builtin_is_registered() {
        let entries = get_builtin_entries(&BuiltInConfig::default());

        let entry = entries
            .iter()
            .find(|e| e.id == "builtin/generate-script-from-current-app")
            .expect("builtin/generate-script-from-current-app must exist");

        assert_eq!(entry.name, "Generate Script from Current App");
        assert_eq!(
            entry.feature,
            BuiltInFeature::AiCommand(AiCommandType::GenerateScriptFromCurrentApp)
        );
        assert!(entry.keywords.contains(&"current".to_string()));
        assert!(entry.keywords.contains(&"app".to_string()));
        assert!(entry.keywords.contains(&"menu".to_string()));
        assert!(entry.keywords.contains(&"browser".to_string()));
    }

    #[test]
    fn harness_first_ai_entries_are_registered_and_legacy_window_entries_are_not() {
        let entries = get_builtin_entries(&BuiltInConfig::default());
        let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();

        // Legacy AI window commands must not be registered
        for legacy_id in [
            "builtin/open-ai-chat",
            "builtin/mini-ai-chat",
            "builtin/new-conversation",
            "builtin/clear-conversation",
        ] {
            assert!(
                !ids.contains(&legacy_id),
                "{legacy_id} should not be registered once AI is harness-first"
            );
        }

        // Harness-first AI entries must remain registered
        for expected_id in [
            "builtin/ai-chat",
            "builtin/generate-script-with-ai",
            "builtin/generate-script-from-current-app",
            "builtin/send-screen-to-ai",
            "builtin/send-focused-window-to-ai",
            "builtin/send-selected-text-to-ai",
            "builtin/send-browser-tab-to-ai",
            "builtin/new-script",
            "builtin/new-extension",
        ] {
            assert!(
                ids.contains(&expected_id),
                "{expected_id} should remain registered"
            );
        }

        // Preview-only screen-area AI entry should be hidden until real region context works
        assert!(
            !ids.contains(&"builtin/send-screen-area-to-ai"),
            "hide the preview-only screen-area AI entry until it can attach real region context"
        );
    }
}

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

mod flow_entries;
pub mod trigger_registry;
pub mod trigger_resolve;

// Re-export only the single startup-validation entry point. The canonical
// `TriggerBuiltin` enum and the `registry()` accessor are addressed through
// `crate::builtins::trigger_registry::{...}` so there is exactly one path
// to the registry symbols.
use flow_entries::push_flow_entries;
pub use trigger_registry::validate_trigger_registry;

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
    /// Send a screenshot of the entire screen to Agent Chat
    SendScreenToAi,
    /// Send a screenshot of the focused window to Agent Chat
    SendFocusedWindowToAi,
    /// Send the currently selected text to Agent Chat
    SendSelectedTextToAi,
    /// Send the focused browser tab URL/content to Agent Chat
    SendBrowserTabToAi,
    /// Send a selected screen area to Agent Chat (interactive selection)
    SendScreenAreaToAi,
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
    SetupPermissions,
    RequestAccessibility,
    OpenAccessibilitySettings,
    AllowAccessibility,
    AllowScreenRecording,
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
    /// Browse and apply color themes
    ChooseTheme,
    /// Select microphone for dictation
    SelectMicrophone,
    /// Open dictation setup and readiness guidance
    DictationSetup,
    /// Configure snap mode (choose mode or disable)
    ConfigureSnapMode,
}
/// Utility command types for quick access tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilityCommandType {
    /// Open the main launcher window
    MainWindow,
    /// Open scratch pad - auto-saving editor
    ScratchPad,
    /// Open quick terminal for running commands
    QuickTerminal,
    /// Open Claude Code in the harness terminal surface used by Tab AI
    ClaudeCode,
    /// Inspect actively running Script Kit child processes
    ProcessManager,
    /// Capture a polished screenshot of Script Kit for marketing/social sharing
    ScriptKitSelfie,
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
    /// Browser tabs switcher for searching and activating open tabs
    BrowserTabs,
    /// Design gallery for viewing separator and icon variations
    DesignGallery,
    FooterGallery,
    /// Main-window non-list state design language showcase
    DesignNonListStates,
    /// In-app StoryBrowser compare/adopt tool (storybook feature only)
    #[cfg(feature = "storybook")]
    DesignExplorer,
    /// Agent Chat window for conversing with AI assistants
    AiChat,
    /// Agent Chat presentation experiment backed by the same Agent Chat runtime.
    AiChatVariant(crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant),
    /// Notes window for quick notes and scratchpad
    Notes,
    /// Emoji picker for selecting and copying emojis
    EmojiPicker,
    /// Sync the Script Kit workspace to a GitHub repository
    SyncToGithub,
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
    /// Dictation transcript history browser
    DictationHistory,
    /// Settings hub for viewing configuration panels
    Settings,
    /// Agent Chat conversation history browser
    AgentChatHistory,
    AiVault,
    /// SDK reference browser — in-product view over `kit://sdk-reference`
    SdkReference,
    /// Discoverability browser for Script Kit gestures, sigils, and shortcuts.
    Tips,
    /// Script template catalog — launcher view that picks a starter template
    /// then opens the naming prompt. Kept distinct from
    /// [`ScriptCommandType::NewScript`] so the fast `New Script` path stays
    /// one-keystroke for MCP/API/trigger callers.
    NewScriptFromTemplate,
    /// Script Kit v1-to-v2 migration board.
    MigrateV1Scripts,
    /// Cycle to the next background shader effect.
    BackgroundEffectNext,
    /// Cycle to the previous background shader effect.
    BackgroundEffectPrevious,
    /// Turn off the background shader effect.
    BackgroundEffectOff,
    /// Flow UX exploration surfaces (hidden, query-only): main-window
    /// variations for finding/launching mdflow flows. See
    /// docs/ai/flow-ux-protocol.md.
    FlowUxVariant(crate::flows::model::FlowUxVariant),
    /// Detached Flow Manager window (runs supervision + Mission Control).
    FlowManager,
    /// Create a new mdflow flow: opens `md create` in the Quick Terminal
    /// (the same path as the Flow Desk's "Create a flow…" row).
    NewFlow,
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

/// Query-only built-ins never appear in the empty-query launcher list; they
/// surface only when the user types a matching query. Kept as a predicate
/// (not a field) so the many existing `BuiltInEntry` literals stay untouched.
/// Currently empty: the Flow UX exploration variants and the detached Flow
/// Manager were removed by the Conversation Desk redesign (2026-07-09).
pub fn is_query_only_builtin(id: &str) -> bool {
    let _ = id;
    false
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
            BuiltInFeature::BrowserTabs => "Open Browser Tabs",
            BuiltInFeature::DesignGallery => "Open Gallery",
            BuiltInFeature::FooterGallery => "Open Footer Gallery",
            BuiltInFeature::DesignNonListStates => "Open Non-List States",
            #[cfg(feature = "storybook")]
            BuiltInFeature::DesignExplorer => "Open Explorer",
            BuiltInFeature::AiChat | BuiltInFeature::AiChatVariant(_) => "Open Agent Chat",
            BuiltInFeature::Notes => "Open Notes",
            BuiltInFeature::EmojiPicker => "Open Emoji Picker",
            BuiltInFeature::SyncToGithub => "Sync to GitHub",
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
                SystemActionType::QuitScriptKit => "Quit Script Kit",
                SystemActionType::ToggleDoNotDisturb => "Toggle Do Not Disturb",
                SystemActionType::StartScreenSaver => "Start Screen Saver",
                SystemActionType::OpenSystemPreferences => "Open macOS System Settings",
                SystemActionType::OpenPrivacySettings => "Open Privacy & Security Settings",
                SystemActionType::OpenDisplaySettings => "Open Displays Settings",
                SystemActionType::OpenSoundSettings => "Open Sound Settings",
                SystemActionType::OpenNetworkSettings => "Open Network Settings",
                SystemActionType::OpenKeyboardSettings => "Open Keyboard Settings",
                SystemActionType::OpenBluetoothSettings => "Open Bluetooth Settings",
                SystemActionType::OpenNotificationsSettings => "Open Notifications Settings",
            },
            BuiltInFeature::NotesCommand(action) => match action {
                NotesCommandType::OpenNotes => "Open Notes",
                NotesCommandType::NewNote => "Create Note",
                NotesCommandType::SearchNotes => "Search Notes",
                NotesCommandType::QuickCapture => "Start Quick Capture",
            },
            BuiltInFeature::AiCommand(action) => match action {
                AiCommandType::OpenAi | AiCommandType::MiniAi => "Open Agent Chat",
                AiCommandType::NewConversation => "Start New Chat",
                AiCommandType::ClearConversation => "Clear Conversation",
                AiCommandType::GenerateScript => "Open Agent Chat to Generate Script",
                AiCommandType::GenerateScriptFromCurrentApp => {
                    "Open Agent Chat to Generate App Script"
                }
                AiCommandType::SendScreenToAi => "Send Screen to Agent Chat",
                AiCommandType::SendFocusedWindowToAi => "Send Window to Agent Chat",
                AiCommandType::SendSelectedTextToAi => "Send Selection to Agent Chat",
                AiCommandType::SendBrowserTabToAi => "Send Tab to Agent Chat",
                AiCommandType::SendScreenAreaToAi => "Select Area for Agent Chat",
            },
            BuiltInFeature::ScriptCommand(action) => match action {
                ScriptCommandType::NewScript => "Create Script",
                ScriptCommandType::NewExtension => "Create Scriptlet Bundle",
            },
            BuiltInFeature::PermissionCommand(action) => match action {
                PermissionCommandType::CheckPermissions => "Check Permissions",
                PermissionCommandType::SetupPermissions => "Set Up Permissions",
                PermissionCommandType::RequestAccessibility => "Request Accessibility Access",
                PermissionCommandType::OpenAccessibilitySettings => "Open Accessibility Settings",
                PermissionCommandType::AllowAccessibility => "Open Accessibility Assistant",
                PermissionCommandType::AllowScreenRecording => "Open Screen Recording Assistant",
            },
            BuiltInFeature::FrecencyCommand(action) => match action {
                FrecencyCommandType::ClearSuggested => "Clear Suggested Items",
            },
            BuiltInFeature::SettingsCommand(action) => match action {
                SettingsCommandType::ResetWindowPositions => "Reset Window Positions",
                SettingsCommandType::ChooseTheme => "Open Theme Designer",
                SettingsCommandType::SelectMicrophone => "Select Microphone",
                SettingsCommandType::DictationSetup => "Open Dictation Setup",
                SettingsCommandType::ConfigureSnapMode => "Configure Snap Mode",
            },
            BuiltInFeature::UtilityCommand(action) => match action {
                UtilityCommandType::MainWindow => "Open Launcher",
                UtilityCommandType::ScratchPad => "Open Scratch Pad",
                UtilityCommandType::QuickTerminal => "Open Quick Terminal",
                UtilityCommandType::ClaudeCode => "Open Claude Code Terminal",
                UtilityCommandType::ProcessManager => "Open Process Manager",
                UtilityCommandType::ScriptKitSelfie => "Capture Selfie",
                UtilityCommandType::StopAllProcesses => "Stop All Running Scripts",
                UtilityCommandType::DoInCurrentApp => "Do in Current App",
                UtilityCommandType::TurnThisIntoCommand => "Turn Into Command",
                UtilityCommandType::CurrentAppCommands => "Open App Commands",
            },
            BuiltInFeature::FileSearch => "Search Files",
            BuiltInFeature::Webcam => "Open Webcam",
            BuiltInFeature::Dictation => "Start Dictation to Current App",
            BuiltInFeature::DictationToAiHarness => "Start Dictation to Agent Chat",
            BuiltInFeature::DictationToFrontmostApp => "Start Dictation to App",
            BuiltInFeature::DictationToNotes => "Start Dictation to Notes",
            BuiltInFeature::DictationHistory => "Open Dictation History",
            BuiltInFeature::Settings => "Open Script Kit Settings",
            BuiltInFeature::AgentChatHistory => "Open Agent Chat History",
            BuiltInFeature::AiVault => "Open AI Vault",
            BuiltInFeature::SdkReference => "Open SDK Reference",
            BuiltInFeature::Tips => "Browse Tips",
            BuiltInFeature::NewScriptFromTemplate => "Browse Templates",
            BuiltInFeature::MigrateV1Scripts => "Migrate v1 Scripts",
            BuiltInFeature::BackgroundEffectNext | BuiltInFeature::BackgroundEffectPrevious => {
                "Cycle Background Effect"
            }
            BuiltInFeature::BackgroundEffectOff => "Turn Off Background Effect",
            BuiltInFeature::FlowUxVariant(_) => "Open Flow Launcher",
            BuiltInFeature::FlowManager => "Open Flow Manager",
            BuiltInFeature::NewFlow => "Create Flow",
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
            BuiltInFeature::BrowserTabs => "Tabs",
            BuiltInFeature::DesignGallery => "Gallery",
            BuiltInFeature::FooterGallery => "Footer Gallery",
            BuiltInFeature::DesignNonListStates => "Non-List States",
            #[cfg(feature = "storybook")]
            BuiltInFeature::DesignExplorer => "Explorer",
            BuiltInFeature::AiChat => "Agent",
            BuiltInFeature::AiChatVariant(variant) => variant.footer_label(),
            BuiltInFeature::Notes => "Notes",
            BuiltInFeature::EmojiPicker => "Emoji",
            BuiltInFeature::SyncToGithub => "Sync",
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
                SystemActionType::QuitScriptKit => "Quit",
                SystemActionType::ToggleDoNotDisturb => "Toggle DND",
                SystemActionType::StartScreenSaver => "Screen Saver",
                SystemActionType::OpenSystemPreferences => "macOS Settings",
                SystemActionType::OpenPrivacySettings => "Privacy & Security",
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
                AiCommandType::OpenAi | AiCommandType::MiniAi => "Agent",
                AiCommandType::NewConversation => "New Chat",
                AiCommandType::ClearConversation => "Clear Chat",
                AiCommandType::GenerateScript => "New Script",
                AiCommandType::GenerateScriptFromCurrentApp => "App Script",
                AiCommandType::SendScreenToAi => "Send Screen",
                AiCommandType::SendFocusedWindowToAi => "Send Window",
                AiCommandType::SendSelectedTextToAi => "Send Text",
                AiCommandType::SendBrowserTabToAi => "Send Tab",
                AiCommandType::SendScreenAreaToAi => "Select Area",
            },
            BuiltInFeature::ScriptCommand(action) => match action {
                ScriptCommandType::NewScript => "New Script",
                ScriptCommandType::NewExtension => "New Bundle",
            },
            BuiltInFeature::PermissionCommand(action) => match action {
                PermissionCommandType::CheckPermissions => "Check Access",
                PermissionCommandType::SetupPermissions => "Permissions",
                PermissionCommandType::RequestAccessibility => "Request Access",
                PermissionCommandType::OpenAccessibilitySettings => "Accessibility",
                PermissionCommandType::AllowAccessibility => "Accessibility",
                PermissionCommandType::AllowScreenRecording => "Screen Recording",
            },
            BuiltInFeature::FrecencyCommand(action) => match action {
                FrecencyCommandType::ClearSuggested => "Clear Suggested",
            },
            BuiltInFeature::SettingsCommand(action) => match action {
                SettingsCommandType::ResetWindowPositions => "Reset Windows",
                SettingsCommandType::ChooseTheme => "Theme",
                SettingsCommandType::SelectMicrophone => "Microphone",
                SettingsCommandType::DictationSetup => "Dictation Setup",
                SettingsCommandType::ConfigureSnapMode => "Configure Snap",
            },
            BuiltInFeature::UtilityCommand(action) => match action {
                UtilityCommandType::MainWindow => "Launcher",
                UtilityCommandType::ScratchPad => "Scratch Pad",
                UtilityCommandType::QuickTerminal => "Terminal",
                UtilityCommandType::ClaudeCode => "Claude Code",
                UtilityCommandType::ProcessManager => "Processes",
                UtilityCommandType::ScriptKitSelfie => "Selfie",
                UtilityCommandType::StopAllProcesses => "Stop Scripts",
                UtilityCommandType::DoInCurrentApp => "Current App",
                UtilityCommandType::TurnThisIntoCommand => "Save Command",
                UtilityCommandType::CurrentAppCommands => "App Commands",
            },
            BuiltInFeature::FileSearch => "Files",
            BuiltInFeature::Webcam => "Webcam",
            BuiltInFeature::Dictation => "Dictate App",
            BuiltInFeature::DictationToAiHarness => "Dictate Chat",
            BuiltInFeature::DictationToFrontmostApp => "Dictate App",
            BuiltInFeature::DictationToNotes => "Dictate Notes",
            BuiltInFeature::DictationHistory => "History",
            BuiltInFeature::Settings => "Kit Settings",
            BuiltInFeature::AgentChatHistory => "History",
            BuiltInFeature::AiVault => "Vault",
            BuiltInFeature::SdkReference => "SDK Docs",
            BuiltInFeature::Tips => "Tips",
            BuiltInFeature::NewScriptFromTemplate => "Templates",
            BuiltInFeature::MigrateV1Scripts => "Migrate",
            BuiltInFeature::BackgroundEffectNext => "Next Effect",
            BuiltInFeature::BackgroundEffectPrevious => "Previous Effect",
            BuiltInFeature::BackgroundEffectOff => "Effect Off",
            BuiltInFeature::FlowUxVariant(_) => "Flows",
            BuiltInFeature::FlowManager => "Runs",
            BuiltInFeature::NewFlow => "New Flow",
        }
    }
}

fn tracked_frontmost_app_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        crate::frontmost_app_tracker::get_last_real_app()
            .map(|app| app.name.trim().to_string())
            .filter(|name| !name.is_empty())
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn current_dictation_entry_name() -> String {
    tracked_frontmost_app_name()
        .map(|name| format!("Dictate to {name}"))
        .unwrap_or_else(|| "Dictate to Current App".to_string())
}

fn current_dictation_entry_description() -> String {
    tracked_frontmost_app_name()
        .map(|name| format!("Voice dictation for {name}"))
        .unwrap_or_else(|| "Voice dictation for the tracked frontmost app".to_string())
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

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/browser-tabs",
            "Search Browser Tabs",
            "Search all open browser tabs by title or URL and jump to the selected tab",
            vec![
                "browser", "tabs", "tab", "search", "switch", "chrome", "safari", "arc", "brave",
                "edge", "raycast", "url", "web",
            ],
            BuiltInFeature::BrowserTabs,
            "globe",
        ));
        debug!("Added Browser Tabs built-in entry");

        // Agent Chat is always available from the launcher.
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/ai-chat",
            "Agent Chat",
            "Open Agent Chat with fresh context",
            vec![
                "ai",
                "agent",
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
        debug!("Added Agent Chat built-in entry");

        for variant in crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::EXPERIMENTS {
            entries.push(BuiltInEntry::new_with_icon(
                variant.menu_id(),
                variant.menu_name(),
                variant.menu_description(),
                variant.keywords(),
                BuiltInFeature::AiChatVariant(variant),
                variant.menu_icon(),
            ));
        }
        debug!("Added Agent Chat UI variation built-in entries");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/agent_chat-history",
            "Agent Chat History",
            "Browse and manage past Agent Chat conversations",
            vec!["history", "conversations", "chat", "ai", "past", "previous"],
            BuiltInFeature::AgentChatHistory,
            "history",
        ));
        debug!("Added Agent Chat History built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/vault",
            "Search AI Vault",
            "Search cmux AI conversation vault sessions from the launcher",
            vec![
                "vault",
                "ai",
                "aivault",
                "ai-vault",
                "cmux",
                "conversation",
                "conversations",
                "session",
                "sessions",
            ],
            BuiltInFeature::AiVault,
            "vault",
        ));
        debug!("Added AI Vault built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/sdk-reference",
            "SDK Reference",
            "Browse Script Kit SDK functions while writing scripts",
            vec![
                "sdk",
                "reference",
                "api",
                "docs",
                "documentation",
                "script",
                "scripting",
                "functions",
                "help",
            ],
            BuiltInFeature::SdkReference,
            "book-open",
        ));
        debug!("Added SDK Reference built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/tips",
            "Script Kit Tips",
            "Learn Script Kit's hidden powers — sigils, gestures, and shortcuts",
            vec![
                "tips",
                "help",
                "learn",
                "hints",
                "shortcuts",
                "sigils",
                "discover",
                "onboarding",
            ],
            BuiltInFeature::Tips,
            "lightbulb",
        ));
        debug!("Added Tips built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/migrate-v1-scripts",
            "Migrate v1 Scripts",
            "Port Script Kit v1 scripts from ~/.kenv/scripts to v2",
            vec!["migrate", "v1", "kenv", "import", "port"],
            BuiltInFeature::MigrateV1Scripts,
            "import",
        ));
        debug!("Added Migrate v1 Scripts built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/dictation-history",
            "Dictation History",
            "Browse, search, and reuse saved dictation transcripts",
            vec![
                "dictation",
                "history",
                "voice",
                "speech",
                "transcript",
                "mic",
            ],
            BuiltInFeature::DictationHistory,
            "history",
        ));
        debug!("Added Dictation History built-in entry");

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
            "Pick an emoji from the built-in list and paste it into the frontmost app",
            vec!["emoji", "picker", "symbols", "unicode", "copy", "clipboard"],
            BuiltInFeature::EmojiPicker,
            "smile",
        ));
        debug!("Added Emoji Picker built-in entry");

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/sync-to-github",
            "Sync to GitHub",
            "Initialize git and sync the Script Kit workspace to GitHub",
            vec![
                "sync",
                "github",
                "git",
                "backup",
                "repository",
                "repo",
                "push",
                "scripts",
                "scriptkit",
            ],
            BuiltInFeature::SyncToGithub,
            "github",
        ));
        debug!("Added Sync to GitHub built-in entry");

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
            "Put Mac to Sleep",
            "Put the system to sleep",
            vec!["sleep", "suspend", "power"],
            BuiltInFeature::SystemAction(SystemActionType::Sleep),
            "moon",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/restart",
            "Restart Mac",
            "Restart the system",
            vec!["restart", "reboot", "power"],
            BuiltInFeature::SystemAction(SystemActionType::Restart),
            "refresh-cw",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/shut-down",
            "Shut Down Mac",
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
            "monitor-down",
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
            "Force Quit Applications",
            "Open the macOS Force Quit Applications dialog",
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
            "Set Volume to 0%",
            "Set system volume to 0% (mute)",
            vec!["volume", "mute", "0", "percent", "zero", "off"],
            BuiltInFeature::SystemAction(SystemActionType::Volume0),
            "volume-off",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-25",
            "Set Volume to 25%",
            "Set system volume to 25%",
            vec!["volume", "25", "percent", "low", "quiet"],
            BuiltInFeature::SystemAction(SystemActionType::Volume25),
            "volume",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-50",
            "Set Volume to 50%",
            "Set system volume to 50%",
            vec!["volume", "50", "percent", "half", "medium"],
            BuiltInFeature::SystemAction(SystemActionType::Volume50),
            "volume-1",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-75",
            "Set Volume to 75%",
            "Set system volume to 75%",
            vec!["volume", "75", "percent", "high", "loud"],
            BuiltInFeature::SystemAction(SystemActionType::Volume75),
            "volume-2",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/volume-100",
            "Set Volume to 100%",
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
            "circle-x",
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
            "monitor-play",
        ));

        // System Preferences
        entries.push(BuiltInEntry::new_with_icon(
            "builtin/system-preferences",
            "Open System Settings",
            "Open macOS System Settings",
            vec!["system", "settings", "preferences", "prefs"],
            BuiltInFeature::SystemAction(SystemActionType::OpenSystemPreferences),
            "settings-2",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/privacy-settings",
            "Open Privacy & Security Settings",
            "Open Privacy & Security settings",
            vec!["privacy", "security", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenPrivacySettings),
            "shield",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/display-settings",
            "Open Displays Settings",
            "Open Displays settings",
            vec!["display", "monitor", "screen", "resolution", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenDisplaySettings),
            "monitor",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/sound-settings",
            "Open Sound Settings",
            "Open Sound settings",
            vec!["sound", "audio", "volume", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenSoundSettings),
            "volume-2",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/network-settings",
            "Open Network Settings",
            "Open Network settings",
            vec!["network", "wifi", "ethernet", "internet", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenNetworkSettings),
            "wifi",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/keyboard-settings",
            "Open Keyboard Settings",
            "Open Keyboard settings",
            vec!["keyboard", "shortcuts", "input", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenKeyboardSettings),
            "keyboard",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/bluetooth-settings",
            "Open Bluetooth Settings",
            "Open Bluetooth settings",
            vec!["bluetooth", "wireless", "settings"],
            BuiltInFeature::SystemAction(SystemActionType::OpenBluetoothSettings),
            "bluetooth",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/notifications-settings",
            "Open Notifications Settings",
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

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/new-note",
            "Create Note",
            "Create a new note and open the Notes window",
            vec!["new", "create", "note", "notes", "add", "write"],
            BuiltInFeature::NotesCommand(NotesCommandType::NewNote),
            "file-plus",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/search-notes",
            "Search Notes",
            "Search all notes by title and content",
            vec!["search", "find", "note", "notes", "browse", "switcher"],
            BuiltInFeature::NotesCommand(NotesCommandType::SearchNotes),
            "file-search",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/quick-capture",
            "Quick Note Capture",
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
            "Open Agent Chat to generate a Script Kit script from your prompt text",
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
            "Send Full Screen to Agent Chat",
            "Capture the full screen and send it to Agent Chat",
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
            "monitor-up",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/send-focused-window-to-ai",
            "Send Focused Window to Agent Chat",
            "Capture the focused window and send it to Agent Chat",
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
            "Send Selection to Agent Chat",
            "Send the currently selected text to Agent Chat",
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
            "Send Current Browser Tab to Agent Chat",
            "Send the current browser tab URL to Agent Chat",
            vec![
                "send", "browser", "tab", "url", "safari", "chrome", "ai", "chat", "web",
            ],
            BuiltInFeature::AiCommand(AiCommandType::SendBrowserTabToAi),
            "globe",
        ));

        // NOTE: builtin-send-screen-area-to-ai removed — no real region-context attachment yet.

        // =========================================================================
        // Script Commands
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/new-script",
            "New Script",
            "Create a blank Script Kit script and open it in your editor",
            vec!["new", "script", "create", "blank", "typescript", "code"],
            BuiltInFeature::ScriptCommand(ScriptCommandType::NewScript),
            "plus",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/new-script-from-template",
            "New Script from Template",
            "Choose a starter template, name it, and open the generated script",
            vec![
                "new",
                "script",
                "template",
                "starter",
                "boilerplate",
                "scaffold",
                "choice",
                "arg",
            ],
            BuiltInFeature::NewScriptFromTemplate,
            "layout-template",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/new-extension",
            "Create Scriptlet Bundle",
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
            "scroll-text",
        ));

        // =========================================================================
        // Permission Commands
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/allow-accessibility",
            "Open Accessibility Assistant",
            "Open the Permission Assistant for Accessibility",
            vec![
                "allow",
                "accessibility",
                "permission",
                "privacy",
                "assistant",
            ],
            BuiltInFeature::PermissionCommand(PermissionCommandType::AllowAccessibility),
            "shield-check",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/allow-screen-recording",
            "Open Screen Recording Assistant",
            "Open the Permission Assistant for Screen Recording",
            vec![
                "allow",
                "screen",
                "recording",
                "permission",
                "privacy",
                "assistant",
            ],
            BuiltInFeature::PermissionCommand(PermissionCommandType::AllowScreenRecording),
            "shield-check",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/check-permissions",
            "Check Permissions",
            "Run a check for all required macOS permissions",
            vec!["check", "permissions", "accessibility", "privacy"],
            BuiltInFeature::PermissionCommand(PermissionCommandType::CheckPermissions),
            "circle-check",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/setup-permissions",
            "Set Up Permissions",
            "Open the guided wizard for granting macOS permissions",
            vec![
                "setup",
                "permissions",
                "wizard",
                "onboarding",
                "grant",
                "accessibility",
                "screen",
                "recording",
                "microphone",
                "privacy",
            ],
            BuiltInFeature::PermissionCommand(PermissionCommandType::SetupPermissions),
            "shield-check",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/request-accessibility",
            "Request Accessibility Access",
            "Request accessibility permission for Script Kit in System Settings",
            vec!["request", "accessibility", "permission"],
            BuiltInFeature::PermissionCommand(PermissionCommandType::RequestAccessibility),
            "key-round",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/accessibility-settings",
            "Open Accessibility Settings",
            "Open Accessibility settings in macOS System Settings",
            vec!["accessibility", "settings", "permission", "open"],
            BuiltInFeature::PermissionCommand(PermissionCommandType::OpenAccessibilitySettings),
            "accessibility",
        ));

        // =========================================================================
        // Frecency/Suggested Commands
        // =========================================================================

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/clear-suggested",
            "Clear Suggested Items",
            "Clear all items from Suggested / Recently Used",
            vec![
                "clear",
                "suggested",
                "recent",
                "frecency",
                "reset",
                "history",
            ],
            BuiltInFeature::FrecencyCommand(FrecencyCommandType::ClearSuggested),
            "eraser",
        ));

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

        if crate::window_state::has_custom_positions() {
            entries.push(BuiltInEntry::new_with_icon(
                "builtin/reset-window-positions",
                "Reset Window Positions",
                "Restore all windows to default positions",
                vec![
                    "reset", "window", "position", "default", "restore", "layout", "location",
                ],
                BuiltInFeature::SettingsCommand(SettingsCommandType::ResetWindowPositions),
                "rotate-ccw",
            ));
        }

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/choose-theme",
            "Theme Designer",
            "Design your color theme with live preview",
            vec![
                "theme",
                "appearance",
                "color",
                "dark",
                "light",
                "scheme",
                "designer",
            ],
            BuiltInFeature::SettingsCommand(SettingsCommandType::ChooseTheme),
            "palette",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/background-effect-next",
            "Next Background Effect",
            "Cycle to the next background shader effect",
            vec![
                "background",
                "effect",
                "shader",
                "aurora",
                "plasma",
                "starfield",
                "animation",
                "fun",
            ],
            BuiltInFeature::BackgroundEffectNext,
            "sparkles",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/background-effect-previous",
            "Previous Background Effect",
            "Cycle to the previous background shader effect",
            vec!["background", "effect", "shader", "previous"],
            BuiltInFeature::BackgroundEffectPrevious,
            "sparkles",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/background-effect-off",
            "Turn Off Background Effect",
            "Turn off the background shader effect",
            vec!["background", "effect", "shader", "off", "disable"],
            BuiltInFeature::BackgroundEffectOff,
            "circle-x",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/dictation-setup",
            "Dictation Setup",
            "Check dictation model, microphone, and hotkey readiness",
            vec!["dictation", "setup", "microphone", "parakeet", "hotkey"],
            BuiltInFeature::SettingsCommand(SettingsCommandType::DictationSetup),
            "sliders-horizontal",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/select-microphone",
            "Select Microphone",
            "Choose which microphone to use for dictation",
            vec![
                "microphone",
                "mic",
                "audio",
                "input",
                "dictation",
                "device",
                "recording",
            ],
            BuiltInFeature::SettingsCommand(SettingsCommandType::SelectMicrophone),
            "mic",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/configure-snap-mode",
            "Choose Window Snap Mode",
            "Choose a snapping grid density or disable drag snapping",
            vec![
                "snap",
                "snapping",
                "window",
                "configure",
                "mode",
                "simple",
                "expanded",
                "precision",
                "off",
                "disable",
                "layout",
            ],
            BuiltInFeature::SettingsCommand(SettingsCommandType::ConfigureSnapMode),
            "square-split-horizontal",
        ));

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
            "square-pen",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/main-window",
            "Launcher",
            "Open the launcher with search and actions",
            vec![
                "launcher",
                "main",
                "window",
                "spotlight",
                "raycast",
                "search",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::MainWindow),
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
            "terminal",
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

        push_flow_entries(&mut entries);

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/script-kit-selfie",
            "Script Kit Selfie",
            "Capture Script Kit with the current desktop background and save a receipt",
            vec![
                "selfie",
                "screenshot",
                "glamor",
                "glamour",
                "marketing",
                "landing",
                "social",
                "share",
                "capture",
                "promo",
            ],
            BuiltInFeature::UtilityCommand(UtilityCommandType::ScriptKitSelfie),
            "camera",
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
            "Capture Webcam Photo",
            "Open the webcam prompt and capture a photo",
            vec!["webcam", "camera", "capture", "photo", "image"],
            BuiltInFeature::Webcam,
            "camera",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/dictation",
            current_dictation_entry_name(),
            current_dictation_entry_description(),
            vec![
                "dictation",
                "voice",
                "speech",
                "microphone",
                "transcribe",
                "whisper",
                "here",
                "current app",
                "frontmost app",
            ],
            BuiltInFeature::Dictation,
            "mic",
        ));

        entries.push(BuiltInEntry::new_with_icon(
            "builtin/dictation-to-ai",
            "Dictate to Agent Chat",
            "Voice dictation - speak and submit to Agent Chat",
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

fn hidden_builtin_entry(id: &str) -> Option<BuiltInEntry> {
    match id {
        "builtin/dictation-to-app" => Some(BuiltInEntry::new_with_icon(
            id,
            "Start Dictation to App",
            "Start dictation and paste the result into the frontmost app",
            vec![
                "dictation",
                "dictate",
                "voice",
                "microphone",
                "app",
                "frontmost",
                "paste",
            ],
            BuiltInFeature::DictationToFrontmostApp,
            "mic",
        )),
        "builtin/dictation-to-notes" => Some(BuiltInEntry::new_with_icon(
            id,
            "Start Dictation to Notes",
            "Start dictation and insert the result into the notes editor",
            vec!["dictation", "dictate", "voice", "microphone", "notes"],
            BuiltInFeature::DictationToNotes,
            "mic",
        )),
        _ => None,
    }
}

/// Resolve a builtin command ID even when the route is intentionally hidden
/// from the top-level launcher registry.
pub fn resolve_builtin_entry(id: &str, config: &BuiltInConfig) -> Option<BuiltInEntry> {
    let canonical_id = crate::config::canonical_builtin_command_id(id);
    get_builtin_entries(config)
        .into_iter()
        .find(|entry| entry.id == canonical_id)
        .or_else(|| hidden_builtin_entry(&canonical_id))
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

    let mut chars = display_lower.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '⌘' => chord_parts.push("cmd".into()),
            '⌥' => chord_parts.push("option".into()),
            '⌃' => chord_parts.push("ctrl".into()),
            '⇧' => chord_parts.push("shift".into()),
            '↩' | '⏎' => chord_parts.push("return".into()),
            '⇥' => chord_parts.push("tab".into()),
            '⌫' => chord_parts.push("delete".into()),
            '⌦' => chord_parts.push("forwarddelete".into()),
            '⎋' => chord_parts.push("escape".into()),
            '←' => chord_parts.push("left".into()),
            '→' => chord_parts.push("right".into()),
            '↑' => chord_parts.push("up".into()),
            '↓' => chord_parts.push("down".into()),
            '+' | ' ' => {}
            other if other.is_ascii_alphanumeric() => {
                let mut token = other.to_string();
                while let Some(next) = chars.peek().copied() {
                    if !next.is_ascii_alphanumeric() {
                        break;
                    }
                    token.push(next);
                    chars.next();
                }
                chord_parts.push(token);
            }
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
mod tests;

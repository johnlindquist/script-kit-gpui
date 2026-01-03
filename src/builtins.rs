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

    // Volume controls
    VolumeUp,
    VolumeDown,
    VolumeMute,

    // Brightness controls
    BrightnessUp,
    BrightnessDown,

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

/// Window action types for window management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowActionType {
    TileLeft,
    TileRight,
    TileTop,
    TileBottom,
    Maximize,
    Minimize,
}

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
    NewConversation,
    ClearConversation,
}

/// Script creation command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptCommandType {
    NewScript,
    NewScriptlet,
}

/// Permission management command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionCommandType {
    CheckPermissions,
    RequestAccessibility,
    OpenAccessibilitySettings,
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
    /// Application launcher for opening installed apps (legacy, apps now in main search)
    AppLauncher,
    /// Individual application entry (for future use when apps appear in search)
    App(String),
    /// Window switcher for managing and tiling windows
    WindowSwitcher,
    /// Design gallery for viewing separator and icon variations
    DesignGallery,
    /// AI Chat window for conversing with AI assistants
    AiChat,
    /// Notes window for quick notes and scratchpad
    Notes,
    /// Menu bar action from the frontmost application
    MenuBarAction(MenuBarActionInfo),

    // === New Command Types ===
    /// System actions (power, UI controls, volume, brightness, settings)
    SystemAction(SystemActionType),
    /// Window actions for the frontmost window (tile, maximize, minimize)
    WindowAction(WindowActionType),
    /// Notes window commands
    NotesCommand(NotesCommandType),
    /// AI window commands
    AiCommand(AiCommandType),
    /// Script creation commands
    ScriptCommand(ScriptCommandType),
    /// Permission management commands
    PermissionCommand(PermissionCommandType),
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
    /// Optional icon (emoji) to display
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
            id: id.into(),
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
            id: id.into(),
            name: name.into(),
            description: description.into(),
            keywords: keywords.into_iter().map(String::from).collect(),
            feature,
            icon: Some(icon.into()),
            group: BuiltInGroup::Core,
        }
    }

    /// Create a new built-in entry with icon and group
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
}

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

    if config.clipboard_history {
        entries.push(BuiltInEntry::new_with_icon(
            "builtin-clipboard-history",
            "Clipboard History",
            "View and manage your clipboard history",
            vec!["clipboard", "history", "paste", "copy"],
            BuiltInFeature::ClipboardHistory,
            "📋",
        ));
        debug!("Added Clipboard History built-in entry");
    }

    // Note: AppLauncher built-in removed - apps now appear directly in main search
    // The app_launcher config flag is kept for future use (e.g., to disable app search entirely)
    if config.app_launcher {
        debug!("app_launcher enabled - apps will appear in main search");
    }

    if config.window_switcher {
        entries.push(BuiltInEntry::new_with_icon(
            "builtin-window-switcher",
            "Window Switcher",
            "Switch, tile, and manage open windows",
            vec!["window", "switch", "tile", "focus", "manage", "switcher"],
            BuiltInFeature::WindowSwitcher,
            "🪟",
        ));
        debug!("Added Window Switcher built-in entry");
    }

    // AI Chat is always available
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-ai-chat",
        "AI Chat",
        "Chat with AI assistants (Claude, GPT)",
        vec![
            "ai",
            "chat",
            "assistant",
            "claude",
            "gpt",
            "openai",
            "anthropic",
            "llm",
        ],
        BuiltInFeature::AiChat,
        "🤖",
    ));
    debug!("Added AI Chat built-in entry");

    // Notes is always available
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-notes",
        "Notes",
        "Quick notes and scratchpad",
        vec![
            "notes",
            "note",
            "scratch",
            "scratchpad",
            "memo",
            "markdown",
            "write",
            "text",
        ],
        BuiltInFeature::Notes,
        "📝",
    ));
    debug!("Added Notes built-in entry");

    // Design Gallery is always available (developer tool)
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-design-gallery",
        "Design Gallery",
        "Browse separator styles and icon variations",
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
        "🎨",
    ));
    debug!("Added Design Gallery built-in entry");

    // =========================================================================
    // System Actions
    // =========================================================================

    // Power management
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-empty-trash",
        "Empty Trash",
        "Empty the macOS Trash",
        vec!["empty", "trash", "delete", "clean"],
        BuiltInFeature::SystemAction(SystemActionType::EmptyTrash),
        "🗑️",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-lock-screen",
        "Lock Screen",
        "Lock the screen",
        vec!["lock", "screen", "security"],
        BuiltInFeature::SystemAction(SystemActionType::LockScreen),
        "🔒",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-sleep",
        "Sleep",
        "Put the system to sleep",
        vec!["sleep", "suspend", "power"],
        BuiltInFeature::SystemAction(SystemActionType::Sleep),
        "😴",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-restart",
        "Restart",
        "Restart the system",
        vec!["restart", "reboot", "power"],
        BuiltInFeature::SystemAction(SystemActionType::Restart),
        "🔄",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-shut-down",
        "Shut Down",
        "Shut down the system",
        vec!["shut", "down", "shutdown", "power", "off"],
        BuiltInFeature::SystemAction(SystemActionType::ShutDown),
        "⏻",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-log-out",
        "Log Out",
        "Log out the current user",
        vec!["log", "out", "logout", "user"],
        BuiltInFeature::SystemAction(SystemActionType::LogOut),
        "🚪",
    ));

    // UI controls
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-toggle-dark-mode",
        "Toggle Dark Mode",
        "Switch between light and dark appearance",
        vec!["dark", "mode", "light", "appearance", "theme", "toggle"],
        BuiltInFeature::SystemAction(SystemActionType::ToggleDarkMode),
        "🌙",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-show-desktop",
        "Show Desktop",
        "Hide all windows to reveal the desktop",
        vec!["show", "desktop", "hide", "windows"],
        BuiltInFeature::SystemAction(SystemActionType::ShowDesktop),
        "🖥️",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-mission-control",
        "Mission Control",
        "Show all windows and desktops",
        vec!["mission", "control", "expose", "spaces", "windows"],
        BuiltInFeature::SystemAction(SystemActionType::MissionControl),
        "🪟",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-launchpad",
        "Launchpad",
        "Open Launchpad to show all applications",
        vec!["launchpad", "apps", "applications"],
        BuiltInFeature::SystemAction(SystemActionType::Launchpad),
        "🚀",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-force-quit",
        "Force Quit Apps",
        "Open the Force Quit Applications dialog",
        vec!["force", "quit", "kill", "apps", "unresponsive"],
        BuiltInFeature::SystemAction(SystemActionType::ForceQuitApps),
        "⚠️",
    ));

    // Volume controls
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-volume-up",
        "Volume Up",
        "Increase system volume",
        vec!["volume", "up", "louder", "sound", "audio"],
        BuiltInFeature::SystemAction(SystemActionType::VolumeUp),
        "🔊",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-volume-down",
        "Volume Down",
        "Decrease system volume",
        vec!["volume", "down", "quieter", "sound", "audio"],
        BuiltInFeature::SystemAction(SystemActionType::VolumeDown),
        "🔉",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-volume-mute",
        "Toggle Mute",
        "Toggle audio mute",
        vec!["mute", "unmute", "volume", "sound", "audio", "toggle"],
        BuiltInFeature::SystemAction(SystemActionType::VolumeMute),
        "🔇",
    ));

    // Brightness controls
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-brightness-up",
        "Brightness Up",
        "Increase display brightness",
        vec!["brightness", "up", "brighter", "display", "screen"],
        BuiltInFeature::SystemAction(SystemActionType::BrightnessUp),
        "☀️",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-brightness-down",
        "Brightness Down",
        "Decrease display brightness",
        vec!["brightness", "down", "dimmer", "display", "screen"],
        BuiltInFeature::SystemAction(SystemActionType::BrightnessDown),
        "🔅",
    ));

    // System utilities
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-toggle-dnd",
        "Toggle Do Not Disturb",
        "Toggle Focus/Do Not Disturb mode",
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
        "🔕",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-screen-saver",
        "Start Screen Saver",
        "Activate the screen saver",
        vec!["screen", "saver", "screensaver"],
        BuiltInFeature::SystemAction(SystemActionType::StartScreenSaver),
        "🖼️",
    ));

    // System Preferences
    entries.push(BuiltInEntry::new_with_icon(
        "builtin-system-preferences",
        "Open System Settings",
        "Open System Settings (System Preferences)",
        vec!["system", "settings", "preferences", "prefs"],
        BuiltInFeature::SystemAction(SystemActionType::OpenSystemPreferences),
        "⚙️",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-privacy-settings",
        "Privacy & Security Settings",
        "Open Privacy & Security settings",
        vec!["privacy", "security", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenPrivacySettings),
        "🔐",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-display-settings",
        "Display Settings",
        "Open Display settings",
        vec!["display", "monitor", "screen", "resolution", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenDisplaySettings),
        "🖥️",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-sound-settings",
        "Sound Settings",
        "Open Sound settings",
        vec!["sound", "audio", "volume", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenSoundSettings),
        "🔊",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-network-settings",
        "Network Settings",
        "Open Network settings",
        vec!["network", "wifi", "ethernet", "internet", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenNetworkSettings),
        "📡",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-keyboard-settings",
        "Keyboard Settings",
        "Open Keyboard settings",
        vec!["keyboard", "shortcuts", "input", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenKeyboardSettings),
        "⌨️",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-bluetooth-settings",
        "Bluetooth Settings",
        "Open Bluetooth settings",
        vec!["bluetooth", "wireless", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenBluetoothSettings),
        "🔵",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-notifications-settings",
        "Notification Settings",
        "Open Notifications settings",
        vec!["notifications", "alerts", "banners", "settings"],
        BuiltInFeature::SystemAction(SystemActionType::OpenNotificationsSettings),
        "🔔",
    ));

    // =========================================================================
    // Window Actions (for frontmost window)
    // =========================================================================

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-tile-left",
        "Tile Window Left",
        "Tile the frontmost window to the left half",
        vec!["tile", "left", "window", "half", "snap"],
        BuiltInFeature::WindowAction(WindowActionType::TileLeft),
        "◧",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-tile-right",
        "Tile Window Right",
        "Tile the frontmost window to the right half",
        vec!["tile", "right", "window", "half", "snap"],
        BuiltInFeature::WindowAction(WindowActionType::TileRight),
        "◨",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-tile-top",
        "Tile Window Top",
        "Tile the frontmost window to the top half",
        vec!["tile", "top", "window", "half", "snap"],
        BuiltInFeature::WindowAction(WindowActionType::TileTop),
        "⬒",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-tile-bottom",
        "Tile Window Bottom",
        "Tile the frontmost window to the bottom half",
        vec!["tile", "bottom", "window", "half", "snap"],
        BuiltInFeature::WindowAction(WindowActionType::TileBottom),
        "⬓",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-maximize-window",
        "Maximize Window",
        "Maximize the frontmost window",
        vec!["maximize", "window", "fullscreen", "expand"],
        BuiltInFeature::WindowAction(WindowActionType::Maximize),
        "⬜",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-minimize-window",
        "Minimize Window",
        "Minimize the frontmost window",
        vec!["minimize", "window", "dock", "hide"],
        BuiltInFeature::WindowAction(WindowActionType::Minimize),
        "➖",
    ));

    // =========================================================================
    // Notes Commands
    // =========================================================================

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-new-note",
        "New Note",
        "Create a new note",
        vec!["new", "note", "create"],
        BuiltInFeature::NotesCommand(NotesCommandType::NewNote),
        "📝",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-search-notes",
        "Search Notes",
        "Search through your notes",
        vec!["search", "notes", "find"],
        BuiltInFeature::NotesCommand(NotesCommandType::SearchNotes),
        "🔍",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-quick-capture",
        "Quick Capture",
        "Quickly capture a note",
        vec!["quick", "capture", "note", "fast"],
        BuiltInFeature::NotesCommand(NotesCommandType::QuickCapture),
        "⚡",
    ));

    // =========================================================================
    // AI Commands
    // =========================================================================

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-new-conversation",
        "New AI Conversation",
        "Start a new AI conversation",
        vec!["new", "conversation", "chat", "ai"],
        BuiltInFeature::AiCommand(AiCommandType::NewConversation),
        "💬",
    ));

    // =========================================================================
    // Script Commands
    // =========================================================================

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-new-script",
        "New Script",
        "Create a new Script Kit script",
        vec!["new", "script", "create", "code"],
        BuiltInFeature::ScriptCommand(ScriptCommandType::NewScript),
        "📜",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-new-scriptlet",
        "New Scriptlet",
        "Create a new Script Kit scriptlet",
        vec!["new", "scriptlet", "create", "snippet"],
        BuiltInFeature::ScriptCommand(ScriptCommandType::NewScriptlet),
        "✨",
    ));

    // =========================================================================
    // Permission Commands
    // =========================================================================

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-check-permissions",
        "Check Permissions",
        "Check all required macOS permissions",
        vec!["check", "permissions", "accessibility", "privacy"],
        BuiltInFeature::PermissionCommand(PermissionCommandType::CheckPermissions),
        "✅",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-request-accessibility",
        "Request Accessibility Permission",
        "Request accessibility permission for Script Kit",
        vec!["request", "accessibility", "permission"],
        BuiltInFeature::PermissionCommand(PermissionCommandType::RequestAccessibility),
        "🔑",
    ));

    entries.push(BuiltInEntry::new_with_icon(
        "builtin-accessibility-settings",
        "Open Accessibility Settings",
        "Open Accessibility settings in System Preferences",
        vec!["accessibility", "settings", "permission", "open"],
        BuiltInFeature::PermissionCommand(PermissionCommandType::OpenAccessibilitySettings),
        "♿",
    ));

    debug!(count = entries.len(), "Built-in entries loaded");
    entries
}

#[cfg(test)]
mod tests {
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
        let clipboard = entries.iter().find(|e| e.id == "builtin-clipboard-history");
        assert!(clipboard.is_some());
        let clipboard = clipboard.unwrap();
        assert_eq!(clipboard.name, "Clipboard History");
        assert_eq!(clipboard.feature, BuiltInFeature::ClipboardHistory);
        assert!(clipboard.keywords.contains(&"clipboard".to_string()));
        assert!(clipboard.keywords.contains(&"history".to_string()));
        assert!(clipboard.keywords.contains(&"paste".to_string()));
        assert!(clipboard.keywords.contains(&"copy".to_string()));

        // Check window switcher entry
        let window_switcher = entries.iter().find(|e| e.id == "builtin-window-switcher");
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

        // Check AI chat entry
        let ai_chat = entries.iter().find(|e| e.id == "builtin-ai-chat");
        assert!(ai_chat.is_some());
        let ai_chat = ai_chat.unwrap();
        assert_eq!(ai_chat.name, "AI Chat");
        assert_eq!(ai_chat.feature, BuiltInFeature::AiChat);
        assert!(ai_chat.keywords.contains(&"ai".to_string()));
        assert!(ai_chat.keywords.contains(&"chat".to_string()));
        assert!(ai_chat.keywords.contains(&"claude".to_string()));
        assert!(ai_chat.keywords.contains(&"gpt".to_string()));

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
        assert!(entries.iter().any(|e| e.id == "builtin-clipboard-history"));
        assert!(entries.iter().any(|e| e.id == "builtin-ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin-design-gallery"));

        // Window switcher should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin-window-switcher"));
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
        // But AI Chat, Notes and Design Gallery are always enabled (plus new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin-ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin-design-gallery"));

        // Clipboard history should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin-clipboard-history"));
    }

    #[test]
    fn test_get_builtin_entries_none_enabled() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: false,
            window_switcher: false,
        };
        let entries = get_builtin_entries(&config);

        // AI Chat, Notes, and Design Gallery are always enabled (plus new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin-ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin-design-gallery"));

        // Clipboard history and window switcher should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin-clipboard-history"));
        assert!(!entries.iter().any(|e| e.id == "builtin-window-switcher"));
    }

    #[test]
    fn test_get_builtin_entries_window_switcher_only() {
        let config = BuiltInConfig {
            clipboard_history: false,
            app_launcher: false,
            window_switcher: true,
        };
        let entries = get_builtin_entries(&config);

        // Window switcher + AI Chat + Notes + Design Gallery (always enabled, plus new command entries)
        assert!(entries.iter().any(|e| e.id == "builtin-window-switcher"));
        assert!(entries.iter().any(|e| e.id == "builtin-ai-chat"));
        assert!(entries.iter().any(|e| e.id == "builtin-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin-design-gallery"));

        // Verify window switcher has correct properties
        let window_switcher = entries
            .iter()
            .find(|e| e.id == "builtin-window-switcher")
            .unwrap();
        assert_eq!(window_switcher.feature, BuiltInFeature::WindowSwitcher);
        assert_eq!(window_switcher.icon, Some("🪟".to_string()));

        // Clipboard history should NOT be present
        assert!(!entries.iter().any(|e| e.id == "builtin-clipboard-history"));
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
        assert_ne!(BuiltInFeature::AiChat, BuiltInFeature::ClipboardHistory);
        assert_ne!(BuiltInFeature::AiChat, BuiltInFeature::DesignGallery);

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

        assert_eq!(entry.id, "test-id");
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
            "📋",
        );

        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.name, "Test Entry");
        assert_eq!(entry.icon, Some("📋".to_string()));
    }

    #[test]
    fn test_builtin_entry_clone() {
        let entry = BuiltInEntry::new_with_icon(
            "test-id",
            "Test Entry",
            "Test description",
            vec!["test"],
            BuiltInFeature::AppLauncher,
            "🚀",
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
        assert!(entries.iter().any(|e| e.id == "builtin-empty-trash"));
        assert!(entries.iter().any(|e| e.id == "builtin-lock-screen"));
        assert!(entries.iter().any(|e| e.id == "builtin-toggle-dark-mode"));
        assert!(entries.iter().any(|e| e.id == "builtin-volume-up"));
        assert!(entries.iter().any(|e| e.id == "builtin-brightness-up"));
        assert!(entries.iter().any(|e| e.id == "builtin-system-preferences"));
    }

    #[test]
    fn test_window_action_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that window action entries exist
        assert!(entries.iter().any(|e| e.id == "builtin-tile-left"));
        assert!(entries.iter().any(|e| e.id == "builtin-tile-right"));
        assert!(entries.iter().any(|e| e.id == "builtin-maximize-window"));
        assert!(entries.iter().any(|e| e.id == "builtin-minimize-window"));
    }

    #[test]
    fn test_notes_command_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that notes command entries exist
        assert!(entries.iter().any(|e| e.id == "builtin-new-note"));
        assert!(entries.iter().any(|e| e.id == "builtin-search-notes"));
        assert!(entries.iter().any(|e| e.id == "builtin-quick-capture"));
    }

    #[test]
    fn test_script_command_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that script command entries exist
        assert!(entries.iter().any(|e| e.id == "builtin-new-script"));
        assert!(entries.iter().any(|e| e.id == "builtin-new-scriptlet"));
    }

    #[test]
    fn test_permission_command_entries_exist() {
        let config = BuiltInConfig::default();
        let entries = get_builtin_entries(&config);

        // Check that permission command entries exist
        assert!(entries.iter().any(|e| e.id == "builtin-check-permissions"));
        assert!(entries
            .iter()
            .any(|e| e.id == "builtin-request-accessibility"));
        assert!(entries
            .iter()
            .any(|e| e.id == "builtin-accessibility-settings"));
    }

    #[test]
    fn test_system_action_type_equality() {
        assert_eq!(SystemActionType::EmptyTrash, SystemActionType::EmptyTrash);
        assert_ne!(SystemActionType::EmptyTrash, SystemActionType::LockScreen);
    }

    #[test]
    fn test_window_action_type_equality() {
        assert_eq!(WindowActionType::TileLeft, WindowActionType::TileLeft);
        assert_ne!(WindowActionType::TileLeft, WindowActionType::TileRight);
    }

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

    #[test]
    fn test_builtin_feature_window_action() {
        let feature = BuiltInFeature::WindowAction(WindowActionType::Maximize);
        assert_eq!(
            feature,
            BuiltInFeature::WindowAction(WindowActionType::Maximize)
        );
        assert_ne!(
            feature,
            BuiltInFeature::WindowAction(WindowActionType::Minimize)
        );
    }
}

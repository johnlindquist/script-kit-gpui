//! Configuration type definitions
//!
//! This module contains all the struct and enum definitions for configuration.

// --- merged from part_01.rs ---
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::defaults::*;

// ============================================
// BUILT-IN CONFIG
// ============================================

/// Configuration for built-in features (clipboard history, app launcher, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltInConfig {
    /// Enable clipboard history built-in (default: true)
    #[serde(default = "default_clipboard_history")]
    pub clipboard_history: bool,
    /// Enable app launcher built-in (default: true)
    #[serde(default = "default_app_launcher")]
    pub app_launcher: bool,
    /// Enable window switcher built-in (default: true)
    #[serde(default = "default_window_switcher")]
    pub window_switcher: bool,
}

fn default_clipboard_history() -> bool {
    DEFAULT_CLIPBOARD_HISTORY
}
fn default_app_launcher() -> bool {
    DEFAULT_APP_LAUNCHER
}
fn default_window_switcher() -> bool {
    DEFAULT_WINDOW_SWITCHER
}

impl Default for BuiltInConfig {
    fn default() -> Self {
        BuiltInConfig {
            clipboard_history: DEFAULT_CLIPBOARD_HISTORY,
            app_launcher: DEFAULT_APP_LAUNCHER,
            window_switcher: DEFAULT_WINDOW_SWITCHER,
        }
    }
}

// ============================================
// PROCESS LIMITS
// ============================================

/// Configuration for process resource limits and health monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessLimits {
    /// Maximum memory usage in MB (None = no limit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_mb: Option<u64>,
    /// Maximum runtime in seconds (None = no limit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runtime_seconds: Option<u64>,
    /// Health check interval in milliseconds (default: 5000)
    #[serde(default = "default_health_check_interval_ms")]
    pub health_check_interval_ms: u64,
}

fn default_health_check_interval_ms() -> u64 {
    DEFAULT_HEALTH_CHECK_INTERVAL_MS
}

impl Default for ProcessLimits {
    fn default() -> Self {
        ProcessLimits {
            max_memory_mb: None,
            max_runtime_seconds: None,
            health_check_interval_ms: DEFAULT_HEALTH_CHECK_INTERVAL_MS,
        }
    }
}

// ============================================
// SUGGESTED CONFIG
// ============================================

/// Configuration for the "Suggested" section (frecency-based ranking)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedConfig {
    /// Whether the Suggested section is shown (default: true)
    #[serde(default = "default_suggested_enabled")]
    pub enabled: bool,
    /// Maximum number of items to show in SUGGESTED section (default: 10)
    #[serde(default = "default_suggested_max_items")]
    pub max_items: usize,
    /// Minimum score threshold for items to appear in Suggested (default: 0.1)
    /// Items with scores below this won't appear even if there's room
    #[serde(default = "default_suggested_min_score")]
    pub min_score: f64,
    /// Half-life in days for score decay (default: 7.0)
    /// Lower values = more weight on recent items
    /// Higher values = more weight on frequently used items
    #[serde(default = "default_suggested_half_life_days")]
    pub half_life_days: f64,
    /// Whether to track script usage for suggestions (default: true)
    /// If false, no new usage is recorded but existing data is preserved
    #[serde(default = "default_suggested_track_usage")]
    pub track_usage: bool,
    /// Commands to exclude from frecency tracking (default: ["builtin-quit-script-kit"])
    /// These commands won't appear in the Suggested section
    #[serde(default = "default_suggested_excluded_commands")]
    pub excluded_commands: Vec<String>,
}

fn default_suggested_enabled() -> bool {
    DEFAULT_SUGGESTED_ENABLED
}
fn default_suggested_max_items() -> usize {
    DEFAULT_SUGGESTED_MAX_ITEMS
}
fn default_suggested_min_score() -> f64 {
    DEFAULT_SUGGESTED_MIN_SCORE
}
fn default_suggested_half_life_days() -> f64 {
    DEFAULT_SUGGESTED_HALF_LIFE_DAYS
}
fn default_suggested_track_usage() -> bool {
    DEFAULT_SUGGESTED_TRACK_USAGE
}
fn default_suggested_excluded_commands() -> Vec<String> {
    DEFAULT_FRECENCY_EXCLUDED_COMMANDS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl Default for SuggestedConfig {
    fn default() -> Self {
        SuggestedConfig {
            enabled: DEFAULT_SUGGESTED_ENABLED,
            max_items: DEFAULT_SUGGESTED_MAX_ITEMS,
            min_score: DEFAULT_SUGGESTED_MIN_SCORE,
            half_life_days: DEFAULT_SUGGESTED_HALF_LIFE_DAYS,
            track_usage: DEFAULT_SUGGESTED_TRACK_USAGE,
            excluded_commands: default_suggested_excluded_commands(),
        }
    }
}

// ============================================
// CONTENT PADDING
// ============================================

/// Content padding configuration for prompts (terminal, editor, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPadding {
    #[serde(default = "default_padding_top")]
    pub top: f32,
    #[serde(default = "default_padding_left")]
    pub left: f32,
    #[serde(default = "default_padding_right")]
    pub right: f32,
}

fn default_padding_top() -> f32 {
    DEFAULT_PADDING_TOP
}
fn default_padding_left() -> f32 {
    DEFAULT_PADDING_LEFT
}
fn default_padding_right() -> f32 {
    DEFAULT_PADDING_RIGHT
}

impl Default for ContentPadding {
    fn default() -> Self {
        ContentPadding {
            top: DEFAULT_PADDING_TOP,
            left: DEFAULT_PADDING_LEFT,
            right: DEFAULT_PADDING_RIGHT,
        }
    }
}

// ============================================
// WATCHER + LAYOUT CONFIG
// ============================================

/// File watcher tuning values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherConfig {
    /// Debounce window for file-system events.
    #[serde(default = "default_watcher_debounce_ms")]
    pub debounce_ms: u64,
    /// Event storm threshold before collapsing to full reload.
    #[serde(default = "default_watcher_storm_threshold")]
    pub storm_threshold: usize,
    /// Initial supervisor restart delay.
    #[serde(default = "default_watcher_initial_backoff_ms")]
    pub initial_backoff_ms: u64,
    /// Maximum supervisor restart delay.
    #[serde(default = "default_watcher_max_backoff_ms")]
    pub max_backoff_ms: u64,
    /// Maximum consecutive notify errors before restart.
    #[serde(default = "default_watcher_max_notify_errors")]
    pub max_notify_errors: u32,
}

fn default_watcher_debounce_ms() -> u64 {
    DEFAULT_WATCHER_DEBOUNCE_MS
}
fn default_watcher_storm_threshold() -> usize {
    DEFAULT_WATCHER_STORM_THRESHOLD
}
fn default_watcher_initial_backoff_ms() -> u64 {
    DEFAULT_WATCHER_INITIAL_BACKOFF_MS
}
fn default_watcher_max_backoff_ms() -> u64 {
    DEFAULT_WATCHER_MAX_BACKOFF_MS
}
fn default_watcher_max_notify_errors() -> u32 {
    DEFAULT_WATCHER_MAX_NOTIFY_ERRORS
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: DEFAULT_WATCHER_DEBOUNCE_MS,
            storm_threshold: DEFAULT_WATCHER_STORM_THRESHOLD,
            initial_backoff_ms: DEFAULT_WATCHER_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_WATCHER_MAX_BACKOFF_MS,
            max_notify_errors: DEFAULT_WATCHER_MAX_NOTIFY_ERRORS,
        }
    }
}

/// Core launcher sizing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutConfig {
    /// Standard list/panel height in pixels.
    #[serde(default = "default_layout_standard_height")]
    pub standard_height: f32,
    /// Full-height content views (editor, terminal) in pixels.
    #[serde(default = "default_layout_max_height")]
    pub max_height: f32,
}

fn default_layout_standard_height() -> f32 {
    DEFAULT_LAYOUT_STANDARD_HEIGHT
}
fn default_layout_max_height() -> f32 {
    DEFAULT_LAYOUT_MAX_HEIGHT
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            standard_height: DEFAULT_LAYOUT_STANDARD_HEIGHT,
            max_height: DEFAULT_LAYOUT_MAX_HEIGHT,
        }
    }
}

/// Theme preset selection loaded from `config.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSelectionPreferences {
    /// Optional preset identifier (for example: "catppuccin-mocha").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
}

/// Dictation preferences loaded from `config.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DictationPreferences {
    /// Persisted microphone device ID. `None` means use system default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_device_id: Option<String>,
}

/// Projection of config-backed runtime preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptKitUserPreferences {
    /// Launcher/window layout settings.
    #[serde(default)]
    pub layout: LayoutConfig,
    /// Theme selection settings.
    #[serde(default)]
    pub theme: ThemeSelectionPreferences,
    /// Dictation / microphone settings.
    #[serde(default)]
    pub dictation: DictationPreferences,
    /// AI chat settings.
    #[serde(default)]
    pub ai: AiPreferences,
    /// Window snapping and related desktop window-management settings.
    #[serde(default)]
    pub window_management: WindowManagementPreferences,
}

/// AI chat preferences loaded from `config.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiPreferences {
    /// Last-selected model ID (e.g. "claude-sonnet-4-6").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_model_id: Option<String>,

    /// Last-selected ACP agent ID (e.g. "opencode", "claude-code").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_acp_agent_id: Option<String>,
}

/// Window-management preferences loaded from `config.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowManagementPreferences {
    /// Persisted drag-snap density/mode. `None` falls back to the app default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_mode: Option<crate::window_control::SnapMode>,
}

// ============================================
// COMMAND CONFIG
// ============================================

/// Configuration for a specific command (script, built-in, or app).
///
/// Used to set per-command shortcuts and visibility options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandConfig {
    /// Optional keyboard shortcut to invoke this command directly
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<HotkeyConfig>,
    /// Whether this command should be hidden from the main menu
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Whether this command requires confirmation before execution.
    /// Overrides the default behavior from DEFAULT_CONFIRMATION_COMMANDS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_required: Option<bool>,
}

// Command ID validation and deeplink helpers are now in `crate::config::command_ids`.

// ============================================
// CLAUDE CODE CLI CONFIG
// ============================================

/// Configuration for the Claude Code CLI provider.
///
/// This allows Script Kit to use the local `claude` CLI as an AI provider,
/// speaking JSONL over stdin/stdout for streaming responses.
///
/// # Example
///
/// ```typescript
/// // In ~/.scriptkit/config.ts
/// export default {
///   hotkey: { modifiers: ["meta"], key: "Semicolon" },
///   claudeCode: {
///     enabled: true,
///     permissionMode: "plan",
///     allowedTools: "Read,Edit,Bash(git:*)",
///     addDirs: ["/home/user/projects"]
///   }
/// } satisfies Config;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeConfig {
    /// Enable the Claude Code CLI provider.
    /// When enabled, "Claude Code" models will appear in the AI chat model picker.
    ///
    /// @default false (requires explicit opt-in)
    #[serde(default = "default_claude_code_enabled")]
    pub enabled: bool,

    /// Custom path to the `claude` CLI binary.
    /// If not specified, will look for `claude` in PATH.
    ///
    /// @default undefined (uses "claude" from PATH)
    /// @example "/opt/homebrew/bin/claude"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Permission mode for Claude Code.
    /// - "plan": Safe default - Claude plans but asks before executing tools
    /// - "dontAsk": Agent can execute tools without confirmation (sandbox only!)
    ///
    /// @default "plan"
    #[serde(default = "default_claude_code_permission_mode")]
    pub permission_mode: String,

    /// Comma-separated list of allowed tools.
    /// Restricts which tools Claude Code can use.
    ///
    /// @default undefined (uses Claude Code defaults)
    /// @example "Read,Edit,Bash(git:*)"
    /// @example "Read,Edit,Bash,Write"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<String>,

    /// Additional directories to add to Claude Code's workspace.
    /// Each path is passed as `--add-dir` to the CLI.
    ///
    /// @default [] (empty)
    /// @example ["/home/user/projects", "/tmp/scratch"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_dirs: Vec<String>,
}

fn default_claude_code_enabled() -> bool {
    DEFAULT_CLAUDE_CODE_ENABLED
}

fn default_claude_code_permission_mode() -> String {
    DEFAULT_CLAUDE_CODE_PERMISSION_MODE.to_string()
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        ClaudeCodeConfig {
            enabled: DEFAULT_CLAUDE_CODE_ENABLED,
            path: None,
            permission_mode: DEFAULT_CLAUDE_CODE_PERMISSION_MODE.to_string(),
            allowed_tools: None,
            add_dirs: vec![],
        }
    }
}

// ============================================
// MCP CONFIG
// ============================================

/// External MCP servers that Script Kit scripts and ACP Chat can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    /// Master switch for Script Kit-managed MCP integrations.
    #[serde(default = "default_mcp_enabled")]
    pub enabled: bool,

    /// Named MCP server definitions keyed by server id.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub servers: HashMap<String, McpServerConfig>,
}

fn default_mcp_enabled() -> bool {
    true
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: HashMap::new(),
        }
    }
}

impl McpConfig {
    pub fn enabled_servers(&self) -> impl Iterator<Item = (&String, &McpServerConfig)> {
        self.servers
            .iter()
            .filter(|(_, server)| self.enabled && server.is_enabled())
    }
}

/// Supported MCP server transports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "transport")]
pub enum McpServerConfig {
    #[serde(rename = "stdio")]
    Stdio(McpStdioServerConfig),
    #[serde(rename = "http")]
    Http(McpHttpServerConfig),
}

impl McpServerConfig {
    pub fn is_enabled(&self) -> bool {
        match self {
            McpServerConfig::Stdio(config) => config.enabled,
            McpServerConfig::Http(config) => config.enabled,
        }
    }
}

fn default_mcp_server_enabled() -> bool {
    true
}

/// A local stdio-backed MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStdioServerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_mcp_server_enabled")]
    pub enabled: bool,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// A remote HTTP-backed MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHttpServerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_mcp_server_enabled")]
    pub enabled: bool,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
}

// --- merged from part_02.rs ---
// ============================================
// HOTKEY CONFIG
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotkeyConfig {
    pub modifiers: Vec<String>,
    pub key: String,
}

impl HotkeyConfig {
    /// Create a default AI hotkey (Cmd+Shift+Space)
    pub fn default_ai_hotkey() -> Self {
        HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "Space".to_string(),
        }
    }

    /// Create a default logs capture hotkey (Cmd+Shift+L)
    pub fn default_logs_hotkey() -> Self {
        HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyL".to_string(),
        }
    }

    /// Convert to a human-readable display string using macOS symbols (e.g., "⌘⇧K").
    ///
    /// Uses standard macOS modifier symbols in order: ⌃ (Control), ⌥ (Option), ⇧ (Shift), ⌘ (Command)
    pub fn to_display_string(&self) -> String {
        let mut result = String::new();

        // Standard macOS order: Control, Option, Shift, Command
        let has_ctrl = self.modifiers.iter().any(|m| m == "ctrl" || m == "control");
        let has_alt = self.modifiers.iter().any(|m| m == "alt" || m == "option");
        let has_shift = self.modifiers.iter().any(|m| m == "shift");
        let has_cmd = self.modifiers.iter().any(|m| m == "meta" || m == "cmd");

        if has_ctrl {
            result.push('⌃');
        }
        if has_alt {
            result.push('⌥');
        }
        if has_shift {
            result.push('⇧');
        }
        if has_cmd {
            result.push('⌘');
        }

        // Normalize key for display
        let key_display = if self.key.starts_with("Key") {
            // "KeyA" -> "A"
            self.key[3..].to_uppercase()
        } else if self.key.starts_with("Digit") {
            // "Digit0" -> "0"
            self.key[5..].to_string()
        } else {
            // Keep as-is but uppercase first char for consistency
            let mut chars = self.key.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        };
        result.push_str(&key_display);

        result
    }

    /// Convert to canonical shortcut string format (e.g., "cmd+shift+k").
    ///
    /// Maps modifier names from config format to shortcut format:
    /// - "meta" -> "cmd"
    /// - "ctrl" -> "ctrl"
    /// - "alt" -> "alt"
    /// - "shift" -> "shift"
    ///
    /// Keys are normalized:
    /// - "KeyX" -> "x" (strip Key prefix, lowercase)
    /// - "Digit0" -> "0" (strip Digit prefix)
    /// - Other keys kept as-is but lowercased
    pub fn to_shortcut_string(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        // Convert modifiers (maintain consistent order: alt, cmd, ctrl, shift)
        let has_alt = self.modifiers.iter().any(|m| m == "alt" || m == "option");
        let has_cmd = self.modifiers.iter().any(|m| m == "meta" || m == "cmd");
        let has_ctrl = self.modifiers.iter().any(|m| m == "ctrl" || m == "control");
        let has_shift = self.modifiers.iter().any(|m| m == "shift");

        if has_alt {
            parts.push("alt".to_string());
        }
        if has_cmd {
            parts.push("cmd".to_string());
        }
        if has_ctrl {
            parts.push("ctrl".to_string());
        }
        if has_shift {
            parts.push("shift".to_string());
        }

        // Normalize key
        let key = if self.key.starts_with("Key") {
            // "KeyA" -> "a"
            self.key[3..].to_lowercase()
        } else if self.key.starts_with("Digit") {
            // "Digit0" -> "0"
            self.key[5..].to_string()
        } else {
            // Keep as-is but lowercase
            self.key.to_lowercase()
        };
        parts.push(key);

        parts.join("+")
    }
}

fn default_main_hotkey() -> HotkeyConfig {
    HotkeyConfig {
        modifiers: vec!["meta".to_string()],
        key: "Semicolon".to_string(),
    }
}

fn default_ai_hotkey_enabled() -> bool {
    DEFAULT_AI_HOTKEY_ENABLED
}

fn default_logs_hotkey_enabled() -> bool {
    DEFAULT_LOGS_HOTKEY_ENABLED
}

fn default_dictation_hotkey_enabled() -> bool {
    DEFAULT_DICTATION_HOTKEY_ENABLED
}

// ============================================
// MAIN CONFIG
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_main_hotkey")]
    pub hotkey: HotkeyConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bun_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// Padding for content areas (terminal, editor, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<ContentPadding>,
    /// Font size for the editor prompt (in pixels)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "editorFontSize"
    )]
    pub editor_font_size: Option<f32>,
    /// Font size for the terminal prompt (in pixels)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "terminalFontSize"
    )]
    pub terminal_font_size: Option<f32>,
    /// UI scale factor (1.0 = 100%)
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "uiScale")]
    pub ui_scale: Option<f32>,
    /// Built-in features configuration (clipboard history, app launcher, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "builtIns")]
    pub built_ins: Option<BuiltInConfig>,
    /// Process resource limits and health monitoring configuration
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "processLimits"
    )]
    pub process_limits: Option<ProcessLimits>,
    /// Maximum text length for clipboard history entries (bytes). 0 = no limit.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "clipboardHistoryMaxTextLength"
    )]
    pub clipboard_history_max_text_length: Option<usize>,
    /// Suggested section configuration (frecency-based ranking)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested: Option<SuggestedConfig>,
    /// Hotkey for opening Notes window (no default; user-configured only)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "notesHotkey"
    )]
    pub notes_hotkey: Option<HotkeyConfig>,
    /// Hotkey for opening ACP Chat window (default: Cmd+Shift+Space)
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "aiHotkey")]
    pub ai_hotkey: Option<HotkeyConfig>,
    /// Whether AI hotkey is enabled (default: true)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "aiHotkeyEnabled"
    )]
    pub ai_hotkey_enabled: Option<bool>,
    /// Hotkey for toggling log capture (default: Cmd+Shift+L)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "logsHotkey"
    )]
    pub logs_hotkey: Option<HotkeyConfig>,
    /// Whether logs hotkey is enabled (default: true)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "logsHotkeyEnabled"
    )]
    pub logs_hotkey_enabled: Option<bool>,
    /// Hotkey for toggling dictation (user-configured; no default shortcut)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "dictationHotkey"
    )]
    pub dictation_hotkey: Option<HotkeyConfig>,
    /// Whether dictation hotkey registration is enabled (default: true)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "dictationHotkeyEnabled"
    )]
    pub dictation_hotkey_enabled: Option<bool>,
    /// Watcher tuning settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watcher: Option<WatcherConfig>,
    /// Window/layout sizing settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,
    /// Theme preset selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeSelectionPreferences>,
    /// Dictation runtime preferences, including microphone selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dictation: Option<DictationPreferences>,
    /// ACP Chat runtime preferences, including the preferred agent and model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiPreferences>,
    /// Window-management preferences such as snap mode.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "windowManagement"
    )]
    pub window_management: Option<WindowManagementPreferences>,
    /// Per-command configuration overrides (shortcuts, visibility)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<HashMap<String, CommandConfig>>,
    /// Claude Code CLI provider configuration.
    /// Enable and configure the local `claude` CLI as an AI provider.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "claudeCode"
    )]
    pub claude_code: Option<ClaudeCodeConfig>,
    /// External MCP servers available to scripts and ACP Chat integrations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
}

// --- merged from part_03.rs ---
impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: default_main_hotkey(),
            bun_path: None,           // Will use system PATH if not specified
            editor: None,             // Will use $EDITOR or fallback to "code"
            padding: None,            // Will use ContentPadding::default() via getter
            editor_font_size: None,   // Will use DEFAULT_EDITOR_FONT_SIZE via getter
            terminal_font_size: None, // Will use DEFAULT_TERMINAL_FONT_SIZE via getter
            ui_scale: None,           // Will use DEFAULT_UI_SCALE via getter
            built_ins: None,          // Will use BuiltInConfig::default() via getter
            process_limits: None,     // Will use ProcessLimits::default() via getter
            clipboard_history_max_text_length: None, // Will use default via getter
            suggested: None,          // Will use SuggestedConfig::default() via getter
            notes_hotkey: None,       // No default shortcut; must be explicitly configured
            ai_hotkey: None,          // Will use HotkeyConfig::default_ai_hotkey() via getter
            ai_hotkey_enabled: None,  // Defaults to true via getter
            logs_hotkey: None,        // Will use HotkeyConfig::default_logs_hotkey() via getter
            logs_hotkey_enabled: None, // Defaults to true via getter
            dictation_hotkey: None,   // No default shortcut; must be explicitly configured
            dictation_hotkey_enabled: None, // Defaults to true via getter
            watcher: None,            // Will use WatcherConfig::default() via getter
            layout: None,             // Will use LayoutConfig::default() via getter
            theme: None,              // Will use ThemeSelectionPreferences::default() via getter
            dictation: None,          // Will use DictationPreferences::default() via getter
            ai: None,                 // Will use AiPreferences::default() via getter
            window_management: None,  // Will use WindowManagementPreferences::default() via getter
            commands: None,           // No per-command overrides by default
            claude_code: None,        // Will use ClaudeCodeConfig::default() via getter
            mcp: None,                // External MCP servers are opt-in via config.ts
        }
    }
}

fn sanitize_positive_f32(value: Option<f32>, fallback: f32) -> f32 {
    match value {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => fallback,
    }
}

fn sanitize_process_limits(mut limits: ProcessLimits) -> ProcessLimits {
    if limits.health_check_interval_ms == 0 {
        limits.health_check_interval_ms = DEFAULT_HEALTH_CHECK_INTERVAL_MS;
    }
    limits
}

impl Config {
    /// Returns the configured editor, falling back to $EDITOR env var or "code" (VS Code)
    /// Used by ActionsDialog "Open in Editor" action
    #[allow(dead_code)] // Will be used by ActionsDialog worker
    pub fn get_editor(&self) -> String {
        self.editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "code".to_string())
    }

    /// Returns the content padding, or defaults if not configured
    #[allow(dead_code)] // Will be used by TermPrompt/EditorPrompt workers
    pub fn get_padding(&self) -> ContentPadding {
        self.padding.clone().unwrap_or_default()
    }

    /// Returns the editor font size, or DEFAULT_EDITOR_FONT_SIZE if not configured
    #[allow(dead_code)] // Will be used by EditorPrompt worker
    pub fn get_editor_font_size(&self) -> f32 {
        sanitize_positive_f32(self.editor_font_size, DEFAULT_EDITOR_FONT_SIZE)
    }

    /// Returns the terminal font size, or DEFAULT_TERMINAL_FONT_SIZE if not configured
    #[allow(dead_code)] // Will be used by TermPrompt worker
    pub fn get_terminal_font_size(&self) -> f32 {
        sanitize_positive_f32(self.terminal_font_size, DEFAULT_TERMINAL_FONT_SIZE)
    }

    /// Returns the UI scale factor, or DEFAULT_UI_SCALE if not configured
    #[cfg(test)]
    fn get_ui_scale(&self) -> f32 {
        sanitize_positive_f32(self.ui_scale, DEFAULT_UI_SCALE)
    }

    /// Returns the built-in features configuration, or defaults if not configured
    #[allow(dead_code)] // Will be used by builtins module
    pub fn get_builtins(&self) -> BuiltInConfig {
        self.built_ins.clone().unwrap_or_default()
    }

    /// Returns max clipboard history text length (bytes), or default if not configured
    #[allow(dead_code)] // Used for clipboard history limits
    pub fn get_clipboard_history_max_text_length(&self) -> usize {
        self.clipboard_history_max_text_length
            .unwrap_or(DEFAULT_CLIPBOARD_HISTORY_MAX_TEXT_LENGTH)
    }

    /// Returns the process limits configuration, or defaults if not configured
    pub fn get_process_limits(&self) -> ProcessLimits {
        sanitize_process_limits(self.process_limits.clone().unwrap_or_default())
    }

    /// Returns the suggested section configuration, or defaults if not configured
    pub fn get_suggested(&self) -> SuggestedConfig {
        self.suggested.clone().unwrap_or_default()
    }

    /// Returns the notes hotkey configuration, or None if not configured.
    /// No default shortcut is provided - users must explicitly configure one.
    #[allow(dead_code)]
    pub fn get_notes_hotkey(&self) -> Option<HotkeyConfig> {
        self.notes_hotkey.clone()
    }

    /// Returns true if AI hotkey registration is enabled.
    pub fn is_ai_hotkey_enabled(&self) -> bool {
        self.ai_hotkey_enabled
            .unwrap_or_else(default_ai_hotkey_enabled)
    }

    /// Returns true if logs hotkey registration is enabled.
    pub fn is_logs_hotkey_enabled(&self) -> bool {
        self.logs_hotkey_enabled
            .unwrap_or_else(default_logs_hotkey_enabled)
    }

    /// Returns the AI hotkey configuration when enabled.
    /// Falls back to default (Cmd+Shift+Space) when enabled but not configured.
    #[allow(dead_code)]
    pub fn get_ai_hotkey(&self) -> Option<HotkeyConfig> {
        if !self.is_ai_hotkey_enabled() {
            return None;
        }
        Some(
            self.ai_hotkey
                .clone()
                .unwrap_or_else(HotkeyConfig::default_ai_hotkey),
        )
    }

    /// Returns the logs hotkey configuration when enabled.
    /// Falls back to default (Cmd+Shift+L) when enabled but not configured.
    #[allow(dead_code)]
    pub fn get_logs_hotkey(&self) -> Option<HotkeyConfig> {
        if !self.is_logs_hotkey_enabled() {
            return None;
        }
        Some(
            self.logs_hotkey
                .clone()
                .unwrap_or_else(HotkeyConfig::default_logs_hotkey),
        )
    }

    /// Returns true if dictation hotkey registration is enabled.
    pub fn is_dictation_hotkey_enabled(&self) -> bool {
        self.dictation_hotkey_enabled
            .unwrap_or_else(default_dictation_hotkey_enabled)
    }

    /// Returns the dictation hotkey configuration when enabled.
    /// No default shortcut is provided - users must explicitly configure one.
    #[allow(dead_code)]
    pub fn get_dictation_hotkey(&self) -> Option<HotkeyConfig> {
        if !self.is_dictation_hotkey_enabled() {
            return None;
        }
        self.dictation_hotkey.clone()
    }

    /// Returns watcher tuning config, or defaults.
    pub fn get_watcher(&self) -> WatcherConfig {
        self.watcher.clone().unwrap_or_default()
    }

    /// Returns layout sizing config, or defaults.
    #[cfg(test)]
    fn get_layout(&self) -> LayoutConfig {
        self.layout.clone().unwrap_or_default()
    }

    /// Returns theme preset selection config, or defaults.
    pub fn get_theme_selection(&self) -> ThemeSelectionPreferences {
        self.theme.clone().unwrap_or_default()
    }

    /// Returns dictation preferences, or defaults.
    pub fn get_dictation_preferences(&self) -> DictationPreferences {
        self.dictation.clone().unwrap_or_default()
    }

    /// Returns ACP Chat preferences, or defaults.
    pub fn get_ai_preferences(&self) -> AiPreferences {
        self.ai.clone().unwrap_or_default()
    }

    /// Returns window-management preferences, or defaults.
    pub fn get_window_management_preferences(&self) -> WindowManagementPreferences {
        self.window_management.clone().unwrap_or_default()
    }

    /// Returns command configuration for a specific command ID, or None if not configured.
    #[allow(dead_code)]
    pub fn get_command_config(&self, command_id: &str) -> Option<&CommandConfig> {
        self.commands.as_ref().and_then(|cmds| cmds.get(command_id))
    }

    /// Check if a command should be hidden from the main menu.
    #[cfg(test)]
    fn is_command_hidden(&self, command_id: &str) -> bool {
        self.get_command_config(command_id)
            .and_then(|c| c.hidden)
            .unwrap_or(false)
    }

    /// Get the shortcut for a command, if configured.
    #[allow(dead_code)]
    pub fn get_command_shortcut(&self, command_id: &str) -> Option<&HotkeyConfig> {
        self.get_command_config(command_id)
            .and_then(|c| c.shortcut.as_ref())
    }

    /// Check if a command requires confirmation before execution.
    ///
    /// Returns true if:
    /// - Command is in DEFAULT_CONFIRMATION_COMMANDS AND not explicitly disabled in config
    /// - OR command has confirmationRequired: true in config
    #[allow(dead_code)]
    pub fn requires_confirmation(&self, command_id: &str) -> bool {
        // Check if user has explicitly configured this command
        if let Some(cmd_config) = self.get_command_config(command_id) {
            if let Some(requires) = cmd_config.confirmation_required {
                return requires;
            }
        }
        // Fall back to defaults
        DEFAULT_CONFIRMATION_COMMANDS.contains(&command_id)
    }

    /// Returns the Claude Code CLI configuration, or defaults if not configured.
    ///
    /// Use this to check if Claude Code is enabled and get its settings:
    /// ```ignore
    /// let claude_config = config.get_claude_code();
    /// if claude_config.enabled {
    ///     // Register Claude Code provider
    /// }
    /// ```
    pub fn get_claude_code(&self) -> ClaudeCodeConfig {
        self.claude_code.clone().unwrap_or_default()
    }

    /// Returns the Script Kit MCP configuration, or defaults if not configured.
    pub fn get_mcp(&self) -> McpConfig {
        self.mcp.clone().unwrap_or_default()
    }
}

// --- merged from part_04.rs ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_config_to_shortcut_string_basic() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "KeyK".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+k");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_multiple_modifiers() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyV".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+shift+v");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_all_modifiers() {
        let config = HotkeyConfig {
            modifiers: vec![
                "alt".to_string(),
                "meta".to_string(),
                "ctrl".to_string(),
                "shift".to_string(),
            ],
            key: "KeyA".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "alt+cmd+ctrl+shift+a");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_digit_key() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Digit0".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+0");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_special_key() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "Space".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+shift+space");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_semicolon() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string()],
            key: "Semicolon".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+semicolon");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_ctrl_modifier() {
        let config = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "ctrl".to_string()],
            key: "KeyI".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+ctrl+i");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_option_alias() {
        // "option" should be treated as "alt"
        let config = HotkeyConfig {
            modifiers: vec!["option".to_string()],
            key: "KeyN".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "alt+n");
    }

    #[test]
    fn hotkey_config_to_shortcut_string_cmd_alias() {
        // "cmd" should work as well as "meta"
        let config = HotkeyConfig {
            modifiers: vec!["cmd".to_string()],
            key: "KeyJ".to_string(),
        };
        assert_eq!(config.to_shortcut_string(), "cmd+j");
    }

    // Command ID validation and deeplink tests have moved to config_tests/mod.rs
    // and now use the public crate::config::command_ids module.

    #[test]
    fn test_get_ui_scale_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(config.get_ui_scale(), DEFAULT_UI_SCALE);
    }

    #[test]
    fn test_get_ui_scale_returns_configured_value_when_positive() {
        let config = Config {
            ui_scale: Some(1.5),
            ..Config::default()
        };
        assert_eq!(config.get_ui_scale(), 1.5);
    }

    #[test]
    fn test_get_ui_scale_returns_default_for_invalid_values() {
        for invalid in [0.0, -0.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let config = Config {
                ui_scale: Some(invalid),
                ..Config::default()
            };
            assert_eq!(config.get_ui_scale(), DEFAULT_UI_SCALE);
        }
    }

    #[test]
    fn test_get_layout_returns_default_when_unset() {
        let config = Config::default();
        assert_eq!(
            config.get_layout().standard_height,
            DEFAULT_LAYOUT_STANDARD_HEIGHT
        );
        assert_eq!(config.get_layout().max_height, DEFAULT_LAYOUT_MAX_HEIGHT);
    }

    #[test]
    fn test_get_layout_returns_configured_layout() {
        let config = Config {
            layout: Some(LayoutConfig {
                standard_height: 420.0,
                max_height: 840.0,
            }),
            ..Config::default()
        };

        let layout = config.get_layout();
        assert_eq!(layout.standard_height, 420.0);
        assert_eq!(layout.max_height, 840.0);
    }

    #[test]
    fn test_is_command_hidden_returns_false_when_missing() {
        let config = Config::default();
        assert!(!config.is_command_hidden("script/missing"));
    }

    #[test]
    fn test_is_command_hidden_returns_configured_hidden_value() {
        let mut commands = HashMap::new();
        commands.insert(
            "script/hidden".to_string(),
            CommandConfig {
                shortcut: None,
                hidden: Some(true),
                confirmation_required: None,
            },
        );
        commands.insert(
            "script/visible".to_string(),
            CommandConfig {
                shortcut: None,
                hidden: Some(false),
                confirmation_required: None,
            },
        );

        let config = Config {
            commands: Some(commands),
            ..Config::default()
        };
        assert!(config.is_command_hidden("script/hidden"));
        assert!(!config.is_command_hidden("script/visible"));
    }

    #[test]
    fn test_get_dictation_hotkey_returns_none_when_unset() {
        let config = Config::default();
        assert_eq!(config.get_dictation_hotkey(), None);
    }

    #[test]
    fn test_get_dictation_hotkey_returns_configured_value_when_enabled() {
        let hotkey = HotkeyConfig {
            modifiers: vec!["meta".to_string(), "shift".to_string()],
            key: "KeyD".to_string(),
        };
        let config = Config {
            dictation_hotkey: Some(hotkey.clone()),
            dictation_hotkey_enabled: Some(true),
            ..Config::default()
        };
        assert_eq!(config.get_dictation_hotkey(), Some(hotkey));
    }

    #[test]
    fn test_get_dictation_hotkey_returns_none_when_disabled() {
        let config = Config {
            dictation_hotkey: Some(HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "KeyD".to_string(),
            }),
            dictation_hotkey_enabled: Some(false),
            ..Config::default()
        };
        assert_eq!(config.get_dictation_hotkey(), None);
    }

    #[test]
    fn mcp_config_defaults_to_enabled_with_no_servers() {
        let config = McpConfig::default();
        assert!(config.enabled);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn mcp_server_config_round_trips_stdio_variant() {
        let json = r#"{
            "transport": "stdio",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-memory"]
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).expect("stdio MCP config");
        match config {
            McpServerConfig::Stdio(config) => {
                assert_eq!(config.command, "npx");
                assert_eq!(
                    config.args,
                    vec!["-y", "@modelcontextprotocol/server-memory"]
                );
                assert!(config.enabled);
            }
            McpServerConfig::Http(_) => panic!("expected stdio config"),
        }
    }

    #[test]
    fn config_get_mcp_returns_default_when_missing() {
        let config = Config::default();
        let mcp = config.get_mcp();
        assert!(mcp.enabled);
        assert!(mcp.servers.is_empty());
    }
}

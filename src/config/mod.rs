//! Configuration module - application settings and config-backed runtime preferences
//!
//! This module provides functionality for:
//! - Loading configuration from ~/.scriptkit/kit/config.ts
//! - Default values for all settings
//! - Type definitions for config structures
//!
//! # Module Structure
//!
//! - `defaults` - All default constant values
//! - `types` - Configuration struct definitions (Config, HotkeyConfig, etc.)
//! - `loader` - File system loading and parsing

pub mod command_ids;
pub mod defaults;
pub mod editor;
mod loader;
mod types;

// Re-export command ID helpers
#[allow(unused_imports)]
pub use command_ids::{
    build_command_id, canonical_builtin_command_id, command_id_from_deeplink,
    command_id_to_deeplink, is_valid_command_id, normalize_builtin_identifier, parse_command_id,
    CommandCategory, SUPPORTED_COMMAND_CATEGORIES,
};

// Re-export defaults that are used externally
pub use defaults::DEFAULT_SUGGESTED_HALF_LIFE_DAYS;

// Re-export types that are used externally
#[allow(unused_imports)]
pub use types::{
    AiPreferences, BuiltInConfig, ClaudeCodeConfig, Config, DictationPreferences, HotkeyConfig,
    LayoutConfig, McpConfig, McpHttpServerConfig, McpServerConfig, McpStdioServerConfig,
    ScriptKitUserPreferences, SuggestedConfig, ThemeSelectionPreferences, WatcherConfig,
    WindowManagementPreferences,
};

// Re-export loader
#[allow(unused_imports)]
pub use loader::{load_config, load_user_preferences, save_user_preferences};

// Re-export editor types for safe config writes (public API for other modules)
#[allow(unused_imports)]
pub use editor::{ConfigWriteError, WriteOutcome};

// Additional exports for tests
#[cfg(test)]
pub use defaults::{
    DEFAULT_CLIPBOARD_HISTORY_MAX_TEXT_LENGTH, DEFAULT_CONFIRMATION_COMMANDS,
    DEFAULT_EDITOR_FONT_SIZE, DEFAULT_HEALTH_CHECK_INTERVAL_MS, DEFAULT_PADDING_LEFT,
    DEFAULT_PADDING_RIGHT, DEFAULT_PADDING_TOP, DEFAULT_TERMINAL_FONT_SIZE, DEFAULT_UI_SCALE,
};

#[cfg(test)]
pub use types::{CommandConfig, ContentPadding, ProcessLimits};

#[cfg(test)]
#[path = "config_tests/mod.rs"]
mod tests;

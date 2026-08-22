//! Extension type definitions for Raycast-compatible extensions
//!
//! This module defines the core types for the extension system:
//! - `ExtensionManifest`: Bundle-level metadata (YAML frontmatter)
//! - `CommandMetadata`: Per-command metadata (H2 section metadata)
//! - `Command`: A runnable command within an extension (formerly `Scriptlet`)
//! - Supporting types: `Preference`, `Argument`, `CommandMode`, etc.
//!
//! # Terminology
//! - **Extension**: A markdown file containing one or more commands
//! - **Command**: An individual runnable entry (H2 section) - formerly called "Scriptlet"
//! - **Manifest**: The YAML frontmatter at the top of an extension file

// --- merged from part_000.rs ---
use crate::metadata_parser::TypedMetadata;
use crate::schema_parser::Schema;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
// ============================================================================
// Valid Categories (Raycast-compatible)
// ============================================================================

/// Valid categories for extensions (matches Raycast's fixed set)
pub const VALID_CATEGORIES: &[&str] = &[
    "Applications",
    "Communication",
    "Data",
    "Design Tools",
    "Developer Tools",
    "Documentation",
    "Finance",
    "Fun",
    "Media",
    "News",
    "Productivity",
    "Security",
    "System",
    "Web",
    "Other",
];
// ============================================================================
// Extension Manifest (Bundle-level metadata)
// ============================================================================

/// Extension bundle metadata (YAML frontmatter)
/// Compatible with Raycast manifest for easy porting
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    // === Required for publishing ===
    /// Unique URL-safe identifier (e.g., "cleanshot")
    #[serde(default)]
    pub name: String,
    /// Display name shown in UI (e.g., "CleanShot X")
    #[serde(default)]
    pub title: String,
    /// Full description
    #[serde(default)]
    pub description: String,
    /// Icon path or icon name (supports both)
    #[serde(default)]
    pub icon: String,
    /// Author's handle/username
    #[serde(default)]
    pub author: String,
    /// License identifier (e.g., "MIT")
    #[serde(default = "default_license")]
    pub license: String,
    /// Categories for discovery
    #[serde(default)]
    pub categories: Vec<String>,
    /// Supported platforms (accept but warn if not macOS)
    #[serde(default)]
    pub platforms: Vec<String>,

    // === Optional ===
    /// Additional search keywords
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Active contributors
    #[serde(default)]
    pub contributors: Vec<String>,
    /// Extension version
    pub version: Option<String>,
    /// Repository URL
    pub repository: Option<String>,
    /// Homepage URL
    pub homepage: Option<String>,
    /// Extension-wide preferences
    #[serde(default)]
    pub preferences: Vec<Preference>,

    // === Script Kit specific ===
    /// Required permissions (clipboard, accessibility, etc.)
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Minimum Script Kit version (semver)
    #[serde(alias = "min_version")]
    pub min_version: Option<String>,
    /// Schema version for future format evolution
    pub manifest_version: Option<u32>,

    /// Catch-all for unknown/future Raycast fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}
fn default_license() -> String {
    "MIT".to_string()
}
impl ExtensionManifest {
    /// Check if the extension targets macOS (or targets all platforms)
    pub fn supports_macos(&self) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p.to_lowercase() == "macos")
    }

    /// Validate that all categories are valid
    pub fn validate_categories(&self) -> Result<(), Vec<String>> {
        let invalid: Vec<String> = self
            .categories
            .iter()
            .filter(|c| !VALID_CATEGORIES.contains(&c.as_str()))
            .cloned()
            .collect();

        if invalid.is_empty() {
            Ok(())
        } else {
            Err(invalid)
        }
    }
}
// ============================================================================
// Command Mode
// ============================================================================

/// Command execution mode (Raycast compatible)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CommandMode {
    /// Shows a UI view (default)
    #[default]
    View,
    /// Runs without UI
    NoView,
    /// Shows in menu bar
    MenuBar,
}
// ============================================================================
// Argument Types
// ============================================================================

/// Argument input type (Raycast compatible)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArgumentType {
    /// Plain text input
    #[default]
    Text,
    /// Password input (masked)
    Password,
    /// Dropdown selection
    Dropdown,
}
/// Typed argument definition (Raycast compatible)
/// Commands can have up to 3 arguments in Raycast
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Argument {
    /// Argument identifier
    pub name: String,
    /// Input type
    #[serde(rename = "type")]
    pub arg_type: ArgumentType,
    /// Placeholder text
    pub placeholder: String,
    /// Whether this argument is required
    #[serde(default)]
    pub required: bool,
    /// Options for dropdown type
    #[serde(default)]
    pub data: Vec<DropdownOption>,
}
/// Dropdown option for arguments and preferences
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DropdownOption {
    /// Display title
    pub title: String,
    /// Stored value
    pub value: String,
}
// ============================================================================
// Preference Types
// ============================================================================

/// Preference input type (Raycast compatible)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreferenceType {
    /// Single-line text field
    #[default]
    Textfield,
    /// Password field (stored securely)
    Password,
    /// Boolean checkbox
    Checkbox,
    /// Dropdown selection
    Dropdown,
    /// Application picker
    #[serde(rename = "appPicker")]
    AppPicker,
    /// File picker
    File,
    /// Directory picker
    Directory,
}
/// Preference definition (Raycast compatible)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preference {
    /// Preference identifier
    pub name: String,
    /// Display title
    pub title: String,
    /// Description/tooltip
    pub description: String,
    /// Input type
    #[serde(rename = "type")]
    pub pref_type: PreferenceType,
    /// Whether this preference is required
    #[serde(default)]
    pub required: bool,
    /// Default value
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// Placeholder text (for text fields)
    pub placeholder: Option<String>,
    /// Label for checkbox type
    pub label: Option<String>,
    /// Options for dropdown type
    #[serde(default)]
    pub data: Vec<DropdownOption>,
}
// ============================================================================
// Command Metadata (Per-H2 section)
// ============================================================================

/// Command metadata (per-H2 section)
/// Mirrors Raycast command properties
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandMetadata {
    // === Core fields ===
    /// Description of what the command does
    pub description: Option<String>,
    /// Subtitle shown next to title
    pub subtitle: Option<String>,
    /// Command-specific icon (overrides extension icon)
    pub icon: Option<String>,
    /// Additional search keywords
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Command mode: "view" (default), "no-view", "menu-bar"
    #[serde(default)]
    pub mode: CommandMode,

    /// Background interval for no-view/menu-bar commands (e.g., "1m", "1h", "1d")
    pub interval: Option<String>,
    /// Cron expression (Script Kit extension)
    pub cron: Option<String>,
    /// Natural language schedule (Script Kit extension)
    pub schedule: Option<String>,

    /// Typed arguments (up to 3 in Raycast)
    #[serde(default)]
    pub arguments: Vec<Argument>,

    /// Command-level preferences (override/extend extension prefs)
    #[serde(default)]
    pub preferences: Vec<Preference>,

    /// If true, user must enable manually
    #[serde(default)]
    pub disabled_by_default: bool,

    // === Script Kit extensions ===
    /// Keyboard shortcut (e.g., "cmd shift k")
    pub shortcut: Option<String>,
    /// Alias trigger
    pub alias: Option<String>,
    /// Text expansion trigger
    pub keyword: Option<String>,
    /// Whether to hide from main list
    #[serde(default)]
    pub hidden: bool,
    /// Trigger text
    pub trigger: Option<String>,
    /// Whether to run in background
    #[serde(default)]
    pub background: bool,
    /// File paths to watch
    pub watch: Option<String>,
    /// System event trigger
    pub system: Option<String>,

    /// Catch-all for unknown fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
// ============================================================================
// Command (formerly Scriptlet)
// ============================================================================

/// A command parsed from an extension file (formerly called Scriptlet)
///
/// Each H2 section in an extension markdown file becomes a Command.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Command {
    /// Display name of the command (from H2 header)
    pub name: String,
    /// URL-safe identifier (slugified name)
    pub command: String,
    /// Tool/language type (bash, python, ts, etc.)
    pub tool: String,
    /// The actual code content
    pub content: String,
    /// Named input placeholders (e.g., ["variableName", "otherVar"])
    pub inputs: Vec<String>,
    /// Group name (from H1 header)
    pub group: String,
    /// HTML preview content (if any)
    pub preview: Option<String>,
    /// Typed metadata from codefence ```metadata block (new format)
    pub typed_metadata: Option<TypedMetadata>,
    /// Schema definition from codefence ```schema block
    pub schema: Option<Schema>,
    /// The extension this command belongs to
    pub extension: Option<String>,
    /// Source file path
    pub source_path: Option<PathBuf>,

    // === Raycast-compatible command metadata ===
    /// Command metadata (mode, arguments, preferences, etc.)
    pub metadata: CommandMetadata,
}
impl Default for Command {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            tool: "ts".to_string(),
            content: String::new(),
            inputs: Vec::new(),
            group: String::new(),
            preview: None,
            typed_metadata: None,
            schema: None,
            extension: None,
            source_path: None,
            metadata: CommandMetadata::default(),
        }
    }
}
impl Command {
    /// Create a new command with minimal required fields
    pub fn new(name: String, tool: String, content: String) -> Self {
        let command = slugify(&name);
        let inputs = extract_named_inputs(&content);

        Command {
            name,
            command,
            tool,
            content,
            inputs,
            ..Default::default()
        }
    }

    /// Check if this command uses a shell tool
    pub fn is_shell(&self) -> bool {
        SHELL_TOOLS.contains(&self.tool.as_str())
    }

    /// Check if the tool type is valid
    pub fn is_valid_tool(&self) -> bool {
        VALID_TOOLS.contains(&self.tool.as_str())
    }

    /// Project the existing Raycast-compatible manifest and command metadata
    /// into the single host-facing command contract.
    pub fn command_descriptor(
        &self,
        manifest: Option<&ExtensionManifest>,
        current_version: &str,
    ) -> Result<sk_protocol::command_contract::CommandDescriptor, String> {
        use sk_protocol::command_contract::{
            CommandArgument, CommandArgumentKind, CommandAvailability, CommandCapability,
            CommandContextPolicy, CommandDescriptor, CommandExecutionMode, CommandExecutionPolicy,
            CommandIdentity, CommandSource,
        };

        let owner = self
            .extension
            .as_deref()
            .or_else(|| manifest.map(|entry| entry.name.as_str()))
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("main");
        let identity = CommandIdentity::new(
            CommandSource::Scriptlet,
            format!("{owner}:{}", self.command),
        )
        .map_err(|error| error.to_string())?;
        let primary_label = match self.metadata.mode {
            CommandMode::NoView => "Run Command",
            CommandMode::MenuBar => "Open Menu Bar Command",
            CommandMode::View => "Run Command",
        };
        let mut descriptor = CommandDescriptor::new(identity, &self.name, primary_label)
            .map_err(|error| error.to_string())?;
        descriptor.subtitle = self
            .metadata
            .subtitle
            .clone()
            .or_else(|| self.metadata.description.clone());
        descriptor.source_name = manifest
            .map(|entry| entry.title.as_str())
            .filter(|title| !title.trim().is_empty())
            .map(ToOwned::to_owned);
        descriptor.shortcut = self.metadata.shortcut.clone();
        descriptor
            .keywords
            .extend(self.metadata.keywords.iter().cloned());
        if let Some(alias) = &self.metadata.alias {
            descriptor.aliases.push(alias.clone());
        }
        if let Some(keyword) = &self.metadata.keyword {
            descriptor.keywords.push(keyword.clone());
        }
        descriptor
            .arguments
            .extend(
                self.metadata
                    .arguments
                    .iter()
                    .map(|argument| CommandArgument {
                        name: argument.name.clone(),
                        kind: match argument.arg_type {
                            ArgumentType::Text => CommandArgumentKind::Text,
                            ArgumentType::Password => CommandArgumentKind::Password,
                            ArgumentType::Dropdown => CommandArgumentKind::Selection,
                        },
                        required: argument.required,
                    }),
            );
        descriptor.execution = CommandExecutionPolicy {
            mode: if self.metadata.background || self.metadata.mode == CommandMode::NoView {
                CommandExecutionMode::BackgroundProcess
            } else {
                CommandExecutionMode::ForegroundProcess
            },
            cancellable: true,
            backgroundable: self.metadata.background || self.metadata.mode == CommandMode::NoView,
            streams_output: self.is_shell(),
        };
        descriptor
            .capabilities
            .push(CommandCapability::Cancellation);
        if descriptor.execution.backgroundable {
            descriptor
                .capabilities
                .push(CommandCapability::BackgroundExecution);
        }
        if descriptor.execution.streams_output {
            descriptor.capabilities.push(CommandCapability::Streaming);
        }
        descriptor.context = CommandContextPolicy::ExplicitOnly;

        if let Some(manifest) = manifest {
            let version_requirement = manifest
                .min_version
                .as_deref()
                .map(|required| {
                    let minimum = parse_extension_version(required, "minVersion")?;
                    let current = parse_extension_version(current_version, "current")?;
                    Ok::<_, String>((required, minimum, current))
                })
                .transpose()?;
            let mut unknown_permission = None;

            for permission in &manifest.permissions {
                let capability = match permission.to_ascii_lowercase().as_str() {
                    "accessibility" => Some(CommandCapability::Accessibility),
                    "screen-recording" | "screenrecording" => {
                        Some(CommandCapability::ScreenRecording)
                    }
                    "microphone" => Some(CommandCapability::Microphone),
                    "clipboard" => Some(CommandCapability::Clipboard),
                    "network" => Some(CommandCapability::Network),
                    "filesystem" | "file-system" => Some(CommandCapability::FileSystem),
                    _ => {
                        if unknown_permission.is_none() {
                            unknown_permission = Some(permission.clone());
                        }
                        None
                    }
                };
                if let Some(capability) = capability {
                    if !descriptor.capabilities.contains(&capability) {
                        descriptor.capabilities.push(capability);
                    }
                }
            }

            descriptor.availability = if let Some(permission) = unknown_permission {
                CommandAvailability::UnknownPermission { permission }
            } else if !manifest.supports_macos() {
                CommandAvailability::UnsupportedPlatform {
                    platform: std::env::consts::OS.to_owned(),
                }
            } else if let Some((required, minimum, current)) = version_requirement {
                if current < minimum {
                    CommandAvailability::HostVersionTooOld {
                        minimum_version: required.to_owned(),
                        current_version: current_version.to_owned(),
                    }
                } else {
                    CommandAvailability::Ready
                }
            } else {
                CommandAvailability::Ready
            };
        }
        if !descriptor.availability.is_executable() {
            if let Some(primary) = descriptor.actions.first_mut() {
                primary.availability = descriptor.availability.clone();
            }
        }
        descriptor.validate().map_err(|error| error.to_string())?;
        Ok(descriptor)
    }
}
/// Convert a name to a command slug (lowercase, spaces to hyphens)
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .join("-")
}
/// Extract named input placeholders from command content
/// Finds all {{variableName}} patterns
fn extract_named_inputs(content: &str) -> Vec<String> {
    let mut inputs = Vec::new();
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second {
            let mut name = String::new();

            // Skip if it's a conditional ({{#if, {{else, {{/if)
            if chars.peek() == Some(&'#') || chars.peek() == Some(&'/') {
                continue;
            }

            // Collect the variable name
            while let Some(&ch) = chars.peek() {
                if ch == '}' {
                    break;
                }
                name.push(ch);
                chars.next();
            }

            // Skip closing }}
            if chars.peek() == Some(&'}') {
                chars.next();
                if chars.peek() == Some(&'}') {
                    chars.next();
                }
            }

            // Add if valid identifier and not already present
            let trimmed = name.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with('/')
                && trimmed != "else"
                && !inputs.iter().any(|existing| existing == trimmed)
            {
                inputs.push(trimmed.to_owned());
            }
        }
    }

    inputs
}
// ============================================================================
// Tool Constants
// ============================================================================

/// Valid tool types that can be used in code fences
pub const VALID_TOOLS: &[&str] = &[
    "bash",
    "python",
    "kit",
    "ts",
    "js",
    "transform",
    "template",
    "open",
    "edit",
    "paste",
    "type",
    "submit",
    "applescript",
    "ruby",
    "perl",
    "php",
    "node",
    "deno",
    "bun",
    // Shell variants
    "zsh",
    "sh",
    "fish",
    "cmd",
    "powershell",
    "pwsh",
];
/// Shell tools (tools that execute in a shell environment)
pub const SHELL_TOOLS: &[&str] = &["bash", "zsh", "sh", "fish", "cmd", "powershell", "pwsh"];

// --- merged from part_001.rs ---
// ============================================================================
// Validation Error Types
// ============================================================================

/// Error encountered during command validation.
/// Allows per-command validation with graceful degradation -
/// valid commands can still be loaded even when others fail.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CommandValidationError {
    /// Path to the source file
    pub file_path: PathBuf,
    /// Name of the command that failed (if identifiable)
    pub command_name: Option<String>,
    /// Line number where the error occurred (1-based)
    pub line_number: Option<usize>,
    /// Description of what went wrong
    pub error_message: String,
}
impl CommandValidationError {
    /// Create a new validation error
    pub fn new(
        file_path: impl Into<PathBuf>,
        command_name: Option<String>,
        line_number: Option<usize>,
        error_message: impl Into<String>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            command_name,
            line_number,
            error_message: error_message.into(),
        }
    }
}
impl std::fmt::Display for CommandValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file_path.display())?;
        if let Some(line) = self.line_number {
            write!(f, ":{}", line)?;
        }
        if let Some(ref name) = self.command_name {
            write!(f, " [{}]", name)?;
        }
        write!(f, ": {}", self.error_message)
    }
}
/// Result of parsing commands from an extension file with validation.
/// Contains both successfully parsed commands and any validation errors encountered.
#[derive(Clone, Debug, Default)]
pub struct ExtensionParseResult {
    /// Successfully parsed commands
    pub commands: Vec<Command>,
    /// Validation errors for commands that failed to parse
    pub errors: Vec<CommandValidationError>,
    /// Extension-level manifest (if present)
    pub manifest: Option<ExtensionManifest>,
}
// ============================================================================
// Icon Resolution
// ============================================================================

/// Source of an icon (name or file path)
#[derive(Clone, Debug, PartialEq)]
pub enum IconSource {
    /// Named icon from built-in set
    Named(String),
    /// File path (relative or absolute)
    Path(String),
}
/// Resolve an icon value to either a named icon or file path
pub fn resolve_icon(value: &str) -> IconSource {
    if value.starts_with("./")
        || value.starts_with("/")
        || value.starts_with("../")
        || value.contains('/')
        || value.ends_with(".png")
        || value.ends_with(".svg")
        || value.ends_with(".icns")
    {
        IconSource::Path(value.to_string())
    } else {
        IconSource::Named(value.to_string())
    }
}
// ============================================================================
// Version Checking
// ============================================================================

fn parse_extension_version(value: &str, label: &str) -> Result<semver::Version, String> {
    let unprefixed = value.strip_prefix('v').unwrap_or(value);
    let normalized = if unprefixed.split('.').count() == 2 {
        format!("{unprefixed}.0")
    } else {
        unprefixed.to_owned()
    };

    semver::Version::parse(&normalized)
        .map_err(|_| format!("Invalid {label} version format: {value}"))
}

/// Check if a version requirement is satisfied using strict semantic version
/// parsing while retaining the documented `v1.2.3` and `1.2` legacy forms.
pub fn check_min_version(required: &str, current: &str) -> Result<(), String> {
    let required_v = parse_extension_version(required, "minVersion")?;
    let current_v = parse_extension_version(current, "current")?;

    if current_v >= required_v {
        Ok(())
    } else {
        Err(format!(
            "Extension requires Script Kit {} or newer (current: {})",
            required, current
        ))
    }
}

// --- merged from part_002.rs ---
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    // ========================================
    // ExtensionManifest Tests
    // ========================================

    #[test]
    fn test_extension_manifest_default() {
        let manifest = ExtensionManifest::default();
        assert_eq!(manifest.name, "");
        // Note: Default::default() doesn't trigger serde defaults
        // The "MIT" default only applies during deserialization
        assert!(manifest.categories.is_empty());
        assert!(manifest.platforms.is_empty());
    }
    #[test]
    fn test_extension_manifest_license_default_on_deserialize() {
        // When deserializing without a license field, it defaults to MIT
        let yaml = "name: test";
        let manifest: ExtensionManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.license, "MIT");
    }
    #[test]
    fn test_extension_manifest_parse_yaml() {
        let yaml = r#"
name: cleanshot
title: CleanShot X
description: Capture screenshots
icon: camera
author: scriptkit
license: MIT
categories:
  - Productivity
  - Media
platforms:
  - macOS
keywords:
  - screenshot
  - capture
"#;
        let manifest: ExtensionManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.name, "cleanshot");
        assert_eq!(manifest.title, "CleanShot X");
        assert_eq!(manifest.description, "Capture screenshots");
        assert_eq!(manifest.icon, "camera");
        assert_eq!(manifest.author, "scriptkit");
        assert_eq!(manifest.license, "MIT");
        assert_eq!(manifest.categories, vec!["Productivity", "Media"]);
        assert_eq!(manifest.platforms, vec!["macOS"]);
        assert_eq!(manifest.keywords, vec!["screenshot", "capture"]);
    }
    #[test]
    fn test_extension_manifest_with_preferences() {
        let yaml = r#"
name: chrome
title: Google Chrome
preferences:
  - name: profile
    title: Chrome Profile
    description: Which profile to use
    type: dropdown
    required: false
    data:
      - title: Default
        value: Default
      - title: Work
        value: "Profile 1"
"#;
        let manifest: ExtensionManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.preferences.len(), 1);
        let pref = &manifest.preferences[0];
        assert_eq!(pref.name, "profile");
        assert_eq!(pref.pref_type, PreferenceType::Dropdown);
        assert!(!pref.required);
        assert_eq!(pref.data.len(), 2);
        assert_eq!(pref.data[0].title, "Default");
        assert_eq!(pref.data[1].value, "Profile 1");
    }
    #[test]
    fn test_extension_manifest_supports_macos() {
        let empty_platforms = ExtensionManifest::default();
        assert!(empty_platforms.supports_macos());

        let macos_only: ExtensionManifest = serde_yaml::from_str("platforms: [macOS]").unwrap();
        assert!(macos_only.supports_macos());

        let windows_only: ExtensionManifest = serde_yaml::from_str("platforms: [Windows]").unwrap();
        assert!(!windows_only.supports_macos());

        let both: ExtensionManifest = serde_yaml::from_str("platforms: [macOS, Windows]").unwrap();
        assert!(both.supports_macos());
    }
    #[test]
    fn test_extension_manifest_validate_categories() {
        let valid: ExtensionManifest =
            serde_yaml::from_str("categories: [Productivity, Media]").unwrap();
        assert!(valid.validate_categories().is_ok());

        let invalid: ExtensionManifest =
            serde_yaml::from_str("categories: [Productivity, InvalidCategory]").unwrap();
        let err = invalid.validate_categories().unwrap_err();
        assert_eq!(err, vec!["InvalidCategory"]);
    }
    #[test]
    fn test_extension_manifest_min_version() {
        let yaml = r#"
name: test
minVersion: "2.0.0"
"#;
        let manifest: ExtensionManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.min_version, Some("2.0.0".to_string()));
    }
    #[test]
    fn test_extension_manifest_min_version_alias() {
        let yaml = r#"
name: test
min_version: "2.0.0"
"#;
        let manifest: ExtensionManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.min_version, Some("2.0.0".to_string()));
    }
    #[test]
    fn test_extension_manifest_preserves_extra_fields() {
        let yaml = r#"
name: test
unknownField: some value
anotherField: 123
"#;
        let manifest: ExtensionManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.extra.contains_key("unknownField"));
        assert!(manifest.extra.contains_key("anotherField"));
    }
    // ========================================
    // CommandMetadata Tests
    // ========================================

    #[test]
    fn test_command_metadata_default() {
        let meta = CommandMetadata::default();
        assert_eq!(meta.mode, CommandMode::View);
        assert!(meta.keywords.is_empty());
        assert!(!meta.disabled_by_default);
        assert!(!meta.hidden);
    }
    #[test]
    fn test_command_metadata_parse_json() {
        let json = r#"{
            "description": "Capture a selected area",
            "mode": "no-view",
            "keywords": ["screenshot", "area"],
            "shortcut": "cmd shift 4"
        }"#;
        let meta: CommandMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(
            meta.description,
            Some("Capture a selected area".to_string())
        );
        assert_eq!(meta.mode, CommandMode::NoView);
        assert_eq!(meta.keywords, vec!["screenshot", "area"]);
        assert_eq!(meta.shortcut, Some("cmd shift 4".to_string()));
    }
    #[test]
    fn test_command_metadata_with_arguments() {
        let json = r#"{
            "description": "Search for text",
            "arguments": [
                {
                    "name": "query",
                    "type": "text",
                    "placeholder": "Enter search term",
                    "required": true
                }
            ]
        }"#;
        let meta: CommandMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.arguments.len(), 1);
        let arg = &meta.arguments[0];
        assert_eq!(arg.name, "query");
        assert_eq!(arg.arg_type, ArgumentType::Text);
        assert_eq!(arg.placeholder, "Enter search term");
        assert!(arg.required);
    }
    #[test]
    fn test_command_metadata_with_interval() {
        let json = r#"{
            "description": "Background task",
            "mode": "no-view",
            "interval": "1h"
        }"#;
        let meta: CommandMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(meta.interval, Some("1h".to_string()));
        assert_eq!(meta.mode, CommandMode::NoView);
    }
    #[test]
    fn test_command_mode_serialization() {
        assert_eq!(
            serde_json::to_string(&CommandMode::View).unwrap(),
            "\"view\""
        );
        assert_eq!(
            serde_json::to_string(&CommandMode::NoView).unwrap(),
            "\"no-view\""
        );
        assert_eq!(
            serde_json::to_string(&CommandMode::MenuBar).unwrap(),
            "\"menu-bar\""
        );
    }
    #[test]
    fn test_command_mode_deserialization() {
        assert_eq!(
            serde_json::from_str::<CommandMode>("\"view\"").unwrap(),
            CommandMode::View
        );
        assert_eq!(
            serde_json::from_str::<CommandMode>("\"no-view\"").unwrap(),
            CommandMode::NoView
        );
        assert_eq!(
            serde_json::from_str::<CommandMode>("\"menu-bar\"").unwrap(),
            CommandMode::MenuBar
        );
    }
    // ========================================
    // Preference Tests
    // ========================================

    #[test]
    fn test_preference_type_serialization() {
        assert_eq!(
            serde_json::to_string(&PreferenceType::Textfield).unwrap(),
            "\"textfield\""
        );
        assert_eq!(
            serde_json::to_string(&PreferenceType::Password).unwrap(),
            "\"password\""
        );
        assert_eq!(
            serde_json::to_string(&PreferenceType::AppPicker).unwrap(),
            "\"appPicker\""
        );
    }
    #[test]
    fn test_preference_parsing() {
        let json = r#"{
            "name": "apiKey",
            "title": "API Key",
            "description": "Your API key",
            "type": "password",
            "required": true
        }"#;
        let pref: Preference = serde_json::from_str(json).unwrap();
        assert_eq!(pref.name, "apiKey");
        assert_eq!(pref.pref_type, PreferenceType::Password);
        assert!(pref.required);
    }
    #[test]
    fn test_preference_with_default() {
        let json = r#"{
            "name": "maxResults",
            "title": "Max Results",
            "description": "Maximum results to show",
            "type": "textfield",
            "required": false,
            "default": 10
        }"#;
        let pref: Preference = serde_json::from_str(json).unwrap();
        assert_eq!(pref.default, Some(serde_json::json!(10)));
    }
    // ========================================
    // Argument Tests
    // ========================================

    #[test]
    fn test_argument_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ArgumentType::Text).unwrap(),
            "\"text\""
        );
        assert_eq!(
            serde_json::to_string(&ArgumentType::Password).unwrap(),
            "\"password\""
        );
        assert_eq!(
            serde_json::to_string(&ArgumentType::Dropdown).unwrap(),
            "\"dropdown\""
        );
    }
    #[test]
    fn test_argument_with_dropdown() {
        let json = r#"{
            "name": "priority",
            "type": "dropdown",
            "placeholder": "Select priority",
            "required": true,
            "data": [
                {"title": "High", "value": "high"},
                {"title": "Medium", "value": "medium"},
                {"title": "Low", "value": "low"}
            ]
        }"#;
        let arg: Argument = serde_json::from_str(json).unwrap();
        assert_eq!(arg.name, "priority");
        assert_eq!(arg.arg_type, ArgumentType::Dropdown);
        assert_eq!(arg.data.len(), 3);
        assert_eq!(arg.data[0].value, "high");
    }
    // ========================================
    // Icon Resolution Tests
    // ========================================

    #[test]
    fn test_resolve_icon_named() {
        assert_eq!(
            resolve_icon("camera"),
            IconSource::Named("camera".to_string())
        );
        assert_eq!(resolve_icon("star"), IconSource::Named("star".to_string()));
        assert_eq!(
            resolve_icon("file-code"),
            IconSource::Named("file-code".to_string())
        );
    }
    #[test]
    fn test_resolve_icon_path() {
        assert_eq!(
            resolve_icon("./icon.png"),
            IconSource::Path("./icon.png".to_string())
        );
        assert_eq!(
            resolve_icon("/path/to/icon.png"),
            IconSource::Path("/path/to/icon.png".to_string())
        );
        assert_eq!(
            resolve_icon("../assets/icon.svg"),
            IconSource::Path("../assets/icon.svg".to_string())
        );
        assert_eq!(
            resolve_icon("assets/icon.png"),
            IconSource::Path("assets/icon.png".to_string())
        );
        assert_eq!(
            resolve_icon("icon.png"),
            IconSource::Path("icon.png".to_string())
        );
        assert_eq!(
            resolve_icon("icon.icns"),
            IconSource::Path("icon.icns".to_string())
        );
    }
    // ========================================
    // Version Checking Tests
    // ========================================

    #[test]
    fn test_check_min_version_satisfied() {
        assert!(check_min_version("1.0.0", "1.0.0").is_ok());
        assert!(check_min_version("1.0.0", "1.0.1").is_ok());
        assert!(check_min_version("1.0.0", "1.1.0").is_ok());
        assert!(check_min_version("1.0.0", "2.0.0").is_ok());
        assert!(check_min_version("1.5", "1.6.0").is_ok());
    }
    #[test]
    fn test_check_min_version_not_satisfied() {
        let result = check_min_version("2.0.0", "1.9.9");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires Script Kit 2.0.0"));
    }
    #[test]
    fn test_check_min_version_with_v_prefix() {
        assert!(check_min_version("v1.0.0", "v1.0.0").is_ok());
        assert!(check_min_version("1.0.0", "v1.0.1").is_ok());
        assert!(check_min_version("v1.0.0", "1.0.1").is_ok());
    }
    #[test]
    fn test_check_min_version_invalid() {
        assert!(check_min_version("invalid", "1.0.0").is_err());
        assert!(check_min_version("1.0.0", "invalid").is_err());

        for invalid in ["1.2.broken", "1.2.3.4", "1..2", "vv1.2.3"] {
            assert!(
                check_min_version(invalid, "2.0.0").is_err(),
                "accepted malformed required version {invalid:?}"
            );
            assert!(
                check_min_version("1.0.0", invalid).is_err(),
                "accepted malformed current version {invalid:?}"
            );
        }
    }

    #[test]
    fn test_check_min_version_preserves_semver_prerelease_ordering() {
        assert!(check_min_version("1.2.3", "1.2.3-beta.1").is_err());
        assert!(check_min_version("1.2.3-beta.1", "1.2.3").is_ok());
    }
    // ========================================
    // Valid Categories Tests
    // ========================================

    #[test]
    fn test_valid_categories_contains_all() {
        assert!(VALID_CATEGORIES.contains(&"Applications"));
        assert!(VALID_CATEGORIES.contains(&"Communication"));
        assert!(VALID_CATEGORIES.contains(&"Data"));
        assert!(VALID_CATEGORIES.contains(&"Design Tools"));
        assert!(VALID_CATEGORIES.contains(&"Developer Tools"));
        assert!(VALID_CATEGORIES.contains(&"Documentation"));
        assert!(VALID_CATEGORIES.contains(&"Finance"));
        assert!(VALID_CATEGORIES.contains(&"Fun"));
        assert!(VALID_CATEGORIES.contains(&"Media"));
        assert!(VALID_CATEGORIES.contains(&"News"));
        assert!(VALID_CATEGORIES.contains(&"Productivity"));
        assert!(VALID_CATEGORIES.contains(&"Security"));
        assert!(VALID_CATEGORIES.contains(&"System"));
        assert!(VALID_CATEGORIES.contains(&"Web"));
        assert!(VALID_CATEGORIES.contains(&"Other"));
        assert_eq!(VALID_CATEGORIES.len(), 15);
    }
    // ========================================
    // Command Tests
    // ========================================

    #[test]
    fn test_command_new() {
        let cmd = Command::new(
            "Hello World".to_string(),
            "bash".to_string(),
            "echo 'hello'".to_string(),
        );
        assert_eq!(cmd.name, "Hello World");
        assert_eq!(cmd.command, "hello-world");
        assert_eq!(cmd.tool, "bash");
        assert_eq!(cmd.content, "echo 'hello'");
        assert!(cmd.inputs.is_empty());
    }

    // --- merged from part_001.rs ---
    #[test]
    fn test_command_slugify() {
        let cmd = Command::new(
            "My Cool Script!".to_string(),
            "ts".to_string(),
            "".to_string(),
        );
        assert_eq!(cmd.command, "my-cool-script");
    }
    #[test]
    fn test_command_extract_inputs() {
        let cmd = Command::new(
            "Test".to_string(),
            "bash".to_string(),
            "echo {{name}} and {{value}}".to_string(),
        );
        assert_eq!(cmd.inputs, vec!["name", "value"]);
    }
    #[test]
    fn test_command_extract_inputs_no_duplicates() {
        let cmd = Command::new(
            "Test".to_string(),
            "bash".to_string(),
            "echo {{name}} {{name}} {{name}}".to_string(),
        );
        assert_eq!(cmd.inputs, vec!["name"]);
    }
    #[test]
    fn test_command_extract_inputs_skip_conditionals() {
        let cmd = Command::new(
            "Test".to_string(),
            "bash".to_string(),
            "{{#if flag}}content{{/if}} {{else}} {{name}}".to_string(),
        );
        // Should only extract "name", not "#if", "/if", "else"
        assert_eq!(cmd.inputs, vec!["name"]);
    }
    #[test]
    fn test_command_is_shell() {
        let bash_cmd = Command::new("Test".to_string(), "bash".to_string(), "".to_string());
        assert!(bash_cmd.is_shell());

        let zsh_cmd = Command::new("Test".to_string(), "zsh".to_string(), "".to_string());
        assert!(zsh_cmd.is_shell());

        let ts_cmd = Command::new("Test".to_string(), "ts".to_string(), "".to_string());
        assert!(!ts_cmd.is_shell());
    }
    #[test]
    fn test_command_is_valid_tool() {
        let bash_cmd = Command::new("Test".to_string(), "bash".to_string(), "".to_string());
        assert!(bash_cmd.is_valid_tool());

        let ts_cmd = Command::new("Test".to_string(), "ts".to_string(), "".to_string());
        assert!(ts_cmd.is_valid_tool());

        let invalid_cmd = Command::new(
            "Test".to_string(),
            "invalid_tool".to_string(),
            "".to_string(),
        );
        assert!(!invalid_cmd.is_valid_tool());
    }
    #[test]
    fn test_command_default() {
        let cmd = Command::default();
        assert_eq!(cmd.tool, "ts");
        assert!(cmd.name.is_empty());
        assert!(cmd.content.is_empty());
    }

    #[test]
    fn test_command_descriptor_truthfully_classifies_extension_compatibility() {
        use sk_protocol::command_contract::CommandAvailability;

        let command = Command::new("Hello".to_owned(), "ts".to_owned(), String::new());
        let unsupported_platform = ExtensionManifest {
            platforms: vec!["Windows".to_owned()],
            ..ExtensionManifest::default()
        };
        let descriptor = command
            .command_descriptor(Some(&unsupported_platform), "1.0.0")
            .expect("valid manifest");
        assert_eq!(
            descriptor.availability,
            CommandAvailability::UnsupportedPlatform {
                platform: std::env::consts::OS.to_owned(),
            }
        );
        assert_eq!(
            descriptor.primary_action().unwrap().availability,
            descriptor.availability
        );
        assert!(!descriptor.can_execute());

        let newer_host_required = ExtensionManifest {
            min_version: Some("2.0.0".to_owned()),
            ..ExtensionManifest::default()
        };
        let descriptor = command
            .command_descriptor(Some(&newer_host_required), "1.0.0")
            .expect("valid manifest");
        assert_eq!(
            descriptor.availability,
            CommandAvailability::HostVersionTooOld {
                minimum_version: "2.0.0".to_owned(),
                current_version: "1.0.0".to_owned(),
            }
        );
        assert!(!descriptor.can_execute());
    }

    #[test]
    fn test_command_descriptor_rejects_unknown_permissions_and_malformed_versions() {
        use sk_protocol::command_contract::{CommandAvailability, CommandCapability};

        let command = Command::new("Hello".to_owned(), "ts".to_owned(), String::new());
        let unknown_permission = ExtensionManifest {
            permissions: vec!["accessibility".to_owned(), "screen-scrape-all".to_owned()],
            ..ExtensionManifest::default()
        };
        let descriptor = command
            .command_descriptor(Some(&unknown_permission), "1.0.0")
            .expect("unknown permissions must remain visible and blocked");
        assert_eq!(
            descriptor.availability,
            CommandAvailability::UnknownPermission {
                permission: "screen-scrape-all".to_owned(),
            }
        );
        assert!(descriptor
            .capabilities
            .contains(&CommandCapability::Accessibility));
        assert!(!descriptor.can_execute());

        let malformed = ExtensionManifest {
            min_version: Some("1.2.broken".to_owned()),
            ..ExtensionManifest::default()
        };
        assert!(command
            .command_descriptor(Some(&malformed), "2.0.0")
            .is_err());
    }
    // ========================================
    // CommandValidationError Tests
    // ========================================

    #[test]
    fn test_command_validation_error_display() {
        let err = CommandValidationError::new(
            "/path/to/file.md",
            Some("My Command".to_string()),
            Some(42),
            "No code block found",
        );
        let display = format!("{}", err);
        assert!(display.contains("/path/to/file.md"));
        assert!(display.contains(":42"));
        assert!(display.contains("[My Command]"));
        assert!(display.contains("No code block found"));
    }
    #[test]
    fn test_command_validation_error_display_minimal() {
        let err = CommandValidationError::new("/path/to/file.md", None, None, "Parse error");
        let display = format!("{}", err);
        assert_eq!(display, "/path/to/file.md: Parse error");
    }
    // ========================================
    // ExtensionParseResult Tests
    // ========================================

    #[test]
    fn test_extension_parse_result_default() {
        let result = ExtensionParseResult::default();
        assert!(result.commands.is_empty());
        assert!(result.errors.is_empty());
        assert!(result.manifest.is_none());
    }
}

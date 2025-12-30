//! JSONL Protocol for Script Kit GPUI
//!
//! Defines message types for bidirectional communication between scripts and the GPUI app.
//! Messages are exchanged as newline-delimited JSON (JSONL).
//!
//! Message kinds:
//! - 'arg': Script sends prompt with choices, app responds with selected value
//! - 'div': Script sends HTML content, app responds with acknowledgment
//! - 'submit': App sends selected value or submission
//! - 'update': App sends live updates to script
//! - 'exit': Script or app signals termination

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use tracing::{debug, warn};

/// A choice option for arg() prompts
///
/// Supports Script Kit API: name, value, and optional description.
/// Semantic IDs are generated for AI-driven UX targeting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic ID for AI targeting. Format: choice:{index}:{value_slug}
    /// This field is typically generated at render time, not provided by scripts.
    #[serde(skip_serializing_if = "Option::is_none", rename = "semanticId")]
    pub semantic_id: Option<String>,
}

/// A field definition for form/fields prompts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl Field {
    pub fn new(name: String) -> Self {
        Field {
            name,
            label: None,
            field_type: None,
            placeholder: None,
            value: None,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_type(mut self, field_type: String) -> Self {
        self.field_type = Some(field_type);
        self
    }

    pub fn with_placeholder(mut self, placeholder: String) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
}

/// Clipboard action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardAction {
    Read,
    Write,
}

/// Clipboard format type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardFormat {
    Text,
    Image,
}

/// Keyboard action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardAction {
    Type,
    Tap,
}

/// Mouse action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MouseAction {
    Move,
    Click,
    SetPosition,
}

/// Clipboard entry type for clipboard history
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardEntryType {
    Text,
    Image,
}

/// Clipboard history action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardHistoryAction {
    List,
    Pin,
    Unpin,
    Remove,
    Clear,
}

/// Window action type for window management
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowActionType {
    Focus,
    Close,
    Minimize,
    Maximize,
    Resize,
    Move,
}

/// Window bounds for window management (integer-based for system windows)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TargetWindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Clipboard history entry data for list responses
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClipboardHistoryEntryData {
    #[serde(rename = "entryId")]
    pub entry_id: String,
    pub content: String,
    #[serde(rename = "contentType")]
    pub content_type: ClipboardEntryType,
    pub timestamp: String,
    pub pinned: bool,
}

/// System window information
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SystemWindowInfo {
    #[serde(rename = "windowId")]
    pub window_id: u32,
    pub title: String,
    #[serde(rename = "appName")]
    pub app_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<TargetWindowBounds>,
    #[serde(rename = "isMinimized", skip_serializing_if = "Option::is_none")]
    pub is_minimized: Option<bool>,
    #[serde(rename = "isActive", skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

/// File search result entry
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileSearchResultEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "isDirectory")]
    pub is_directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(rename = "modifiedAt", skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
}

/// Element type for UI element querying (getElements)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Choice,
    Input,
    Button,
    Panel,
    List,
}

/// Information about a UI element returned by getElements
///
/// Contains semantic ID, type, text content, and state information
/// for AI-driven UX targeting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementInfo {
    /// Semantic ID for targeting (e.g., "choice:0:apple")
    pub semantic_id: String,
    /// Element type (choice, input, button, panel, list)
    #[serde(rename = "type")]
    pub element_type: ElementType,
    /// Display text of the element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Value (for choices/inputs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Whether this element is currently selected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Whether this element is currently focused
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
    /// Index in parent container (for list items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

impl ElementInfo {
    /// Create a new ElementInfo for a choice element
    pub fn choice(index: usize, name: &str, value: &str, selected: bool) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id("choice", index, value),
            element_type: ElementType::Choice,
            text: Some(name.to_string()),
            value: Some(value.to_string()),
            selected: Some(selected),
            focused: None,
            index: Some(index),
        }
    }

    /// Create a new ElementInfo for an input element
    pub fn input(name: &str, value: Option<&str>, focused: bool) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id_named("input", name),
            element_type: ElementType::Input,
            text: None,
            value: value.map(|s| s.to_string()),
            selected: None,
            focused: Some(focused),
            index: None,
        }
    }

    /// Create a new ElementInfo for a button element
    pub fn button(index: usize, label: &str) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id("button", index, label),
            element_type: ElementType::Button,
            text: Some(label.to_string()),
            value: None,
            selected: None,
            focused: None,
            index: Some(index),
        }
    }

    /// Create a new ElementInfo for a panel element
    pub fn panel(name: &str) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id_named("panel", name),
            element_type: ElementType::Panel,
            text: None,
            value: None,
            selected: None,
            focused: None,
            index: None,
        }
    }

    /// Create a new ElementInfo for a list element
    pub fn list(name: &str, item_count: usize) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id_named("list", name),
            element_type: ElementType::List,
            text: Some(format!("{} items", item_count)),
            value: None,
            selected: None,
            focused: None,
            index: None,
        }
    }
}

/// Scriptlet metadata for protocol serialization
///
/// Matches the ScriptletMetadata struct from scriptlets.rs but optimized
/// for JSON protocol transmission.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptletMetadataData {
    /// Trigger text that activates this scriptlet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// Keyboard shortcut (e.g., "cmd shift k")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// Raw cron expression (e.g., "*/5 * * * *")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// Natural language schedule (e.g., "every tuesday at 2pm") - converted to cron internally
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Whether to run in background
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    /// File paths to watch for changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<String>,
    /// System event to trigger on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Description of the scriptlet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Text expansion trigger (e.g., "type,,")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,
}

/// Scriptlet data for protocol transmission
///
/// Represents a parsed scriptlet from markdown files, containing
/// the code content, tool type, metadata, and variable inputs.
/// Used to pass scriptlet data between Rust and SDK/bun.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptletData {
    /// Name of the scriptlet (from H2 header)
    pub name: String,
    /// Command identifier (slugified name)
    pub command: String,
    /// Tool type (bash, python, ts, etc.)
    pub tool: String,
    /// The actual code content
    pub content: String,
    /// Named input placeholders (e.g., ["variableName", "otherVar"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// Group name (from H1 header)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// HTML preview content (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Parsed metadata from HTML comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ScriptletMetadataData>,
    /// The kenv this scriptlet belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kenv: Option<String>,
    /// Source file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Whether this is a scriptlet (always true for scriptlets)
    #[serde(default)]
    pub is_scriptlet: bool,
}

impl ScriptletData {
    /// Create a new ScriptletData with required fields
    pub fn new(name: String, command: String, tool: String, content: String) -> Self {
        ScriptletData {
            name,
            command,
            tool,
            content,
            inputs: Vec::new(),
            group: None,
            preview: None,
            metadata: None,
            kenv: None,
            source_path: None,
            is_scriptlet: true,
        }
    }

    /// Add inputs
    pub fn with_inputs(mut self, inputs: Vec<String>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Add group
    pub fn with_group(mut self, group: String) -> Self {
        self.group = Some(group);
        self
    }

    /// Add preview HTML
    pub fn with_preview(mut self, preview: String) -> Self {
        self.preview = Some(preview);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: ScriptletMetadataData) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add kenv
    pub fn with_kenv(mut self, kenv: String) -> Self {
        self.kenv = Some(kenv);
        self
    }

    /// Add source path
    pub fn with_source_path(mut self, path: String) -> Self {
        self.source_path = Some(path);
        self
    }
}

/// Script error data for structured error reporting
///
/// Sent when a script execution fails, providing detailed error information
/// for display in the UI with actionable suggestions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptErrorData {
    /// User-friendly error message
    pub error_message: String,
    /// Raw stderr output if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_output: Option<String>,
    /// Process exit code if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Parsed stack trace if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,
    /// Path to the script that failed
    pub script_path: String,
    /// Actionable fix suggestions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    /// When the error occurred (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

impl ScriptErrorData {
    /// Create a new ScriptErrorData with required fields
    pub fn new(error_message: String, script_path: String) -> Self {
        ScriptErrorData {
            error_message,
            stderr_output: None,
            exit_code: None,
            stack_trace: None,
            script_path,
            suggestions: Vec::new(),
            timestamp: None,
        }
    }

    /// Add stderr output
    pub fn with_stderr(mut self, stderr: String) -> Self {
        self.stderr_output = Some(stderr);
        self
    }

    /// Add exit code
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Add stack trace
    pub fn with_stack_trace(mut self, trace: String) -> Self {
        self.stack_trace = Some(trace);
        self
    }

    /// Add suggestions
    pub fn with_suggestions(mut self, suggestions: Vec<String>) -> Self {
        self.suggestions = suggestions;
        self
    }

    /// Add a single suggestion
    pub fn add_suggestion(mut self, suggestion: String) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Add timestamp
    pub fn with_timestamp(mut self, timestamp: String) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

impl Choice {
    pub fn new(name: String, value: String) -> Self {
        Choice {
            name,
            value,
            description: None,
            semantic_id: None,
        }
    }

    pub fn with_description(name: String, value: String, description: String) -> Self {
        Choice {
            name,
            value,
            description: Some(description),
            semantic_id: None,
        }
    }

    /// Generate and set the semantic ID for this choice.
    /// Format: choice:{index}:{value_slug}
    ///
    /// The value_slug is created by:
    /// - Converting to lowercase
    /// - Replacing spaces and underscores with hyphens
    /// - Removing non-alphanumeric characters (except hyphens)
    /// - Truncating to 20 characters
    pub fn with_semantic_id(mut self, index: usize) -> Self {
        self.semantic_id = Some(generate_semantic_id("choice", index, &self.value));
        self
    }

    /// Set the semantic ID directly (for custom IDs)
    pub fn set_semantic_id(&mut self, id: String) {
        self.semantic_id = Some(id);
    }

    /// Generate the semantic ID without setting it (for external use)
    pub fn generate_id(&self, index: usize) -> String {
        generate_semantic_id("choice", index, &self.value)
    }
}

/// Generate a semantic ID for an element.
///
/// Format: {type}:{index}:{value_slug}
///
/// # Arguments
/// * `element_type` - The element type (e.g., "choice", "button", "input")
/// * `index` - The numeric index of the element
/// * `value` - The value to convert to a slug
///
/// # Returns
/// A semantic ID string in the format: type:index:slug
pub fn generate_semantic_id(element_type: &str, index: usize, value: &str) -> String {
    let slug = value_to_slug(value);
    format!("{}:{}:{}", element_type, index, slug)
}

/// Generate a semantic ID for named elements (no index).
///
/// Format: {type}:{name}
///
/// # Arguments
/// * `element_type` - The element type (e.g., "input", "panel", "window")
/// * `name` - The name of the element
///
/// # Returns
/// A semantic ID string in the format: type:name
pub fn generate_semantic_id_named(element_type: &str, name: &str) -> String {
    let slug = value_to_slug(name);
    format!("{}:{}", element_type, slug)
}

/// Convert a value string to a URL-safe slug suitable for semantic IDs.
///
/// - Converts to lowercase
/// - Replaces spaces and underscores with hyphens
/// - Removes non-alphanumeric characters (except hyphens)
/// - Collapses multiple hyphens to single
/// - Truncates to 20 characters
/// - Removes leading/trailing hyphens
pub fn value_to_slug(value: &str) -> String {
    let slug: String = value
        .to_lowercase()
        .chars()
        .map(|c| match c {
            ' ' | '_' => '-',
            c if c.is_alphanumeric() || c == '-' => c,
            _ => '-',
        })
        .collect();

    // Collapse multiple hyphens and trim
    let mut result = String::with_capacity(20);
    let mut prev_hyphen = false;

    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }

        if result.len() >= 20 {
            break;
        }
    }

    // Remove trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    // Ensure non-empty
    if result.is_empty() {
        result.push_str("item");
    }

    result
}

/// Protocol message with type discrimination via serde tag
///
/// This enum uses the "type" field to discriminate between message kinds.
/// Each variant corresponds to a message kind in the Script Kit v1 API.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    // ============================================================
    // CORE PROMPTS (existing)
    // ============================================================
    /// Script sends arg prompt with choices
    #[serde(rename = "arg")]
    Arg {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },

    /// Script sends div (HTML display)
    #[serde(rename = "div")]
    Div {
        id: String,
        html: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tailwind: Option<String>,
    },

    /// App responds with submission (selected value or null)
    #[serde(rename = "submit")]
    Submit { id: String, value: Option<String> },

    /// App sends live update
    #[serde(rename = "update")]
    Update {
        id: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },

    /// Signal termination
    #[serde(rename = "exit")]
    Exit {
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// Force submit the current prompt with a value (from SDK's submit() function)
    #[serde(rename = "forceSubmit")]
    ForceSubmit { value: serde_json::Value },

    // ============================================================
    // TEXT INPUT PROMPTS
    // ============================================================
    /// Code/text editor with syntax highlighting
    #[serde(rename = "editor")]
    Editor {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(rename = "onInit", skip_serializing_if = "Option::is_none")]
        on_init: Option<String>,
        #[serde(rename = "onSubmit", skip_serializing_if = "Option::is_none")]
        on_submit: Option<String>,
    },

    /// Compact arg prompt (same as Arg but compact display)
    #[serde(rename = "mini")]
    Mini {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },

    /// Tiny arg prompt (same as Arg but tiny display)
    #[serde(rename = "micro")]
    Micro {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },

    // ============================================================
    // SELECTION PROMPTS
    // ============================================================
    /// Select from choices with optional multiple selection
    #[serde(rename = "select")]
    Select {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        #[serde(skip_serializing_if = "Option::is_none")]
        multiple: Option<bool>,
    },

    // ============================================================
    // FORM PROMPTS
    // ============================================================
    /// Multiple input fields
    #[serde(rename = "fields")]
    Fields { id: String, fields: Vec<Field> },

    /// Custom HTML form
    #[serde(rename = "form")]
    Form { id: String, html: String },

    // ============================================================
    // FILE/PATH PROMPTS
    // ============================================================
    /// File/folder path picker
    #[serde(rename = "path")]
    Path {
        id: String,
        #[serde(rename = "startPath", skip_serializing_if = "Option::is_none")]
        start_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },

    /// File drop zone
    #[serde(rename = "drop")]
    Drop { id: String },

    // ============================================================
    // INPUT CAPTURE PROMPTS
    // ============================================================
    /// Hotkey capture
    #[serde(rename = "hotkey")]
    Hotkey {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
    },

    // ============================================================
    // TEMPLATE/TEXT PROMPTS
    // ============================================================
    /// Template string with placeholders
    #[serde(rename = "template")]
    Template { id: String, template: String },

    /// Environment variable prompt
    #[serde(rename = "env")]
    Env {
        id: String,
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        secret: Option<bool>,
    },

    // ============================================================
    // MEDIA PROMPTS
    // ============================================================
    /// Chat interface
    #[serde(rename = "chat")]
    Chat { id: String },

    /// Terminal emulator
    #[serde(rename = "term")]
    Term {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
    },

    /// Custom widget with HTML
    #[serde(rename = "widget")]
    Widget {
        id: String,
        html: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<serde_json::Value>,
    },

    /// Webcam capture
    #[serde(rename = "webcam")]
    Webcam { id: String },

    /// Microphone recording
    #[serde(rename = "mic")]
    Mic { id: String },

    // ============================================================
    // NOTIFICATION/FEEDBACK MESSAGES
    // ============================================================
    /// System notification
    #[serde(rename = "notify")]
    Notify {
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    },

    /// System beep sound
    #[serde(rename = "beep")]
    Beep {},

    /// Text-to-speech
    #[serde(rename = "say")]
    Say {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        voice: Option<String>,
    },

    /// Status bar update
    #[serde(rename = "setStatus")]
    SetStatus {
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },

    /// HUD (heads-up display) overlay message
    #[serde(rename = "hud")]
    Hud {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },

    // ============================================================
    // SYSTEM CONTROL MESSAGES
    // ============================================================
    /// Menu bar icon/scripts
    #[serde(rename = "menu")]
    Menu {
        #[serde(skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scripts: Option<Vec<String>>,
    },

    /// Clipboard operations
    #[serde(rename = "clipboard")]
    Clipboard {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        action: ClipboardAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<ClipboardFormat>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
    },

    /// Keyboard simulation
    #[serde(rename = "keyboard")]
    Keyboard {
        action: KeyboardAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        keys: Option<String>,
    },

    /// Mouse control
    #[serde(rename = "mouse")]
    Mouse {
        action: MouseAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },

    /// Show window
    #[serde(rename = "show")]
    Show {},

    /// Hide window
    #[serde(rename = "hide")]
    Hide {},

    /// Open URL in default browser
    #[serde(rename = "browse")]
    Browse { url: String },

    /// Execute shell command
    #[serde(rename = "exec")]
    Exec {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<serde_json::Value>,
    },

    // ============================================================
    // UI UPDATE MESSAGES
    // ============================================================
    /// Set panel HTML content
    #[serde(rename = "setPanel")]
    SetPanel { html: String },

    /// Set preview HTML content
    #[serde(rename = "setPreview")]
    SetPreview { html: String },

    /// Set prompt HTML content
    #[serde(rename = "setPrompt")]
    SetPrompt { html: String },

    // ============================================================
    // SELECTED TEXT OPERATIONS
    // ============================================================
    /// Get currently selected text from focused application
    #[serde(rename = "getSelectedText")]
    GetSelectedText {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Set (replace) currently selected text in focused application
    #[serde(rename = "setSelectedText")]
    SetSelectedText {
        text: String,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Check if accessibility permissions are granted
    #[serde(rename = "checkAccessibility")]
    CheckAccessibility {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Request accessibility permissions (shows system dialog)
    #[serde(rename = "requestAccessibility")]
    RequestAccessibility {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    // ============================================================
    // WINDOW INFORMATION
    // ============================================================
    /// Get current window bounds (position and size)
    #[serde(rename = "getWindowBounds")]
    GetWindowBounds {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with window bounds
    #[serde(rename = "windowBounds")]
    WindowBounds {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    // ============================================================
    // SELECTED TEXT RESPONSES
    // ============================================================
    /// Response with selected text
    #[serde(rename = "selectedText")]
    SelectedText {
        text: String,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response after setting text
    #[serde(rename = "textSet")]
    TextSet {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with accessibility permission status
    #[serde(rename = "accessibilityStatus")]
    AccessibilityStatus {
        granted: bool,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    // ============================================================
    // CLIPBOARD HISTORY
    // ============================================================
    /// Request clipboard history operation
    #[serde(rename = "clipboardHistory")]
    ClipboardHistory {
        #[serde(rename = "requestId")]
        request_id: String,
        action: ClipboardHistoryAction,
        /// Entry ID for pin/unpin/remove operations
        #[serde(rename = "entryId", skip_serializing_if = "Option::is_none")]
        entry_id: Option<String>,
    },

    /// Response with a clipboard history entry
    #[serde(rename = "clipboardHistoryEntry")]
    ClipboardHistoryEntry {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "entryId")]
        entry_id: String,
        content: String,
        #[serde(rename = "contentType")]
        content_type: ClipboardEntryType,
        timestamp: String,
        pinned: bool,
    },

    /// Response with list of clipboard history entries
    #[serde(rename = "clipboardHistoryList")]
    ClipboardHistoryList {
        #[serde(rename = "requestId")]
        request_id: String,
        entries: Vec<ClipboardHistoryEntryData>,
    },

    /// Response for clipboard history action result
    #[serde(rename = "clipboardHistoryResult")]
    ClipboardHistoryResult {
        #[serde(rename = "requestId")]
        request_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    // ============================================================
    // WINDOW MANAGEMENT (System Windows)
    // ============================================================
    /// Request list of all system windows
    #[serde(rename = "windowList")]
    WindowList {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Perform action on a system window
    #[serde(rename = "windowAction")]
    WindowAction {
        #[serde(rename = "requestId")]
        request_id: String,
        action: WindowActionType,
        #[serde(rename = "windowId", skip_serializing_if = "Option::is_none")]
        window_id: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bounds: Option<TargetWindowBounds>,
    },

    /// Response with list of system windows
    #[serde(rename = "windowListResult")]
    WindowListResult {
        #[serde(rename = "requestId")]
        request_id: String,
        windows: Vec<SystemWindowInfo>,
    },

    /// Response for window action result
    #[serde(rename = "windowActionResult")]
    WindowActionResult {
        #[serde(rename = "requestId")]
        request_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    // ============================================================
    // FILE SEARCH
    // ============================================================
    /// Request file search
    #[serde(rename = "fileSearch")]
    FileSearch {
        #[serde(rename = "requestId")]
        request_id: String,
        query: String,
        #[serde(rename = "onlyin", skip_serializing_if = "Option::is_none")]
        only_in: Option<String>,
    },

    /// Response with file search results
    #[serde(rename = "fileSearchResult")]
    FileSearchResult {
        #[serde(rename = "requestId")]
        request_id: String,
        files: Vec<FileSearchResultEntry>,
    },

    // ============================================================
    // SCREENSHOT CAPTURE
    // ============================================================
    /// Request to capture app window screenshot
    #[serde(rename = "captureScreenshot")]
    CaptureScreenshot {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with screenshot data as base64 PNG
    #[serde(rename = "screenshotResult")]
    ScreenshotResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Base64-encoded PNG data
        data: String,
        width: u32,
        height: u32,
    },

    // ============================================================
    // STATE QUERY
    // ============================================================
    /// Request current UI state without modifying it
    #[serde(rename = "getState")]
    GetState {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with current UI state
    #[serde(rename = "stateResult")]
    StateResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Current prompt type
        #[serde(rename = "promptType")]
        prompt_type: String,
        /// Prompt ID if active
        #[serde(rename = "promptId", skip_serializing_if = "Option::is_none")]
        prompt_id: Option<String>,
        /// Placeholder text if applicable
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
        /// Current input/filter value
        #[serde(rename = "inputValue")]
        input_value: String,
        /// Total number of choices
        #[serde(rename = "choiceCount")]
        choice_count: usize,
        /// Number of visible/filtered choices
        #[serde(rename = "visibleChoiceCount")]
        visible_choice_count: usize,
        /// Currently selected index (-1 if none)
        #[serde(rename = "selectedIndex")]
        selected_index: i32,
        /// Value of the selected choice
        #[serde(rename = "selectedValue", skip_serializing_if = "Option::is_none")]
        selected_value: Option<String>,
        /// Whether the window has focus
        #[serde(rename = "isFocused")]
        is_focused: bool,
        /// Whether the window is visible
        #[serde(rename = "windowVisible")]
        window_visible: bool,
    },

    // ============================================================
    // ELEMENT QUERY (AI-driven UX)
    // ============================================================
    /// Request visible UI elements with semantic IDs
    #[serde(rename = "getElements")]
    GetElements {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Maximum number of elements to return (default: 50)
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
    },

    /// Response with list of visible UI elements
    #[serde(rename = "elementsResult")]
    ElementsResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// List of visible UI elements
        elements: Vec<ElementInfo>,
        /// Total number of elements (may be larger than returned if limit applied)
        #[serde(rename = "totalCount")]
        total_count: usize,
    },

    // ============================================================
    // ERROR REPORTING
    // ============================================================
    /// Script error with structured error information
    #[serde(rename = "setError")]
    SetError {
        /// User-friendly error message
        #[serde(rename = "errorMessage")]
        error_message: String,
        /// Raw stderr output if available
        #[serde(rename = "stderrOutput", skip_serializing_if = "Option::is_none")]
        stderr_output: Option<String>,
        /// Process exit code if available
        #[serde(rename = "exitCode", skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        /// Parsed stack trace if available
        #[serde(rename = "stackTrace", skip_serializing_if = "Option::is_none")]
        stack_trace: Option<String>,
        /// Path to the script that failed
        #[serde(rename = "scriptPath")]
        script_path: String,
        /// Actionable fix suggestions
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        suggestions: Vec<String>,
        /// When the error occurred (ISO 8601 format)
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
    },

    // ============================================================
    // SCRIPTLET OPERATIONS
    // ============================================================
    /// Run a scriptlet with variable substitution
    #[serde(rename = "runScriptlet")]
    RunScriptlet {
        #[serde(rename = "requestId")]
        request_id: String,
        /// The scriptlet data to execute
        scriptlet: ScriptletData,
        /// Named input values for {{variable}} substitution
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inputs: Option<serde_json::Value>,
        /// Positional arguments for $1, $2, etc.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },

    /// Request list of available scriptlets
    #[serde(rename = "getScriptlets")]
    GetScriptlets {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Optional kenv to filter by
        #[serde(skip_serializing_if = "Option::is_none")]
        kenv: Option<String>,
        /// Optional group to filter by
        #[serde(skip_serializing_if = "Option::is_none")]
        group: Option<String>,
    },

    /// Response with list of scriptlets
    #[serde(rename = "scriptletList")]
    ScriptletList {
        #[serde(rename = "requestId")]
        request_id: String,
        /// List of scriptlets
        scriptlets: Vec<ScriptletData>,
    },

    /// Result of scriptlet execution
    #[serde(rename = "scriptletResult")]
    ScriptletResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Whether execution succeeded
        success: bool,
        /// Output from the scriptlet (stdout)
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Error message if failed
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// Exit code if available
        #[serde(rename = "exitCode", skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },

    // ============================================================
    // TEST INFRASTRUCTURE
    // ============================================================
    /// Simulate a mouse click at specific coordinates (for testing)
    ///
    /// This message is used by test infrastructure to simulate mouse clicks
    /// at specified window-relative coordinates. It enables automated visual
    /// testing of click behaviors without requiring actual user interaction.
    #[serde(rename = "simulateClick")]
    SimulateClick {
        #[serde(rename = "requestId")]
        request_id: String,
        /// X coordinate relative to the window
        x: f64,
        /// Y coordinate relative to the window
        y: f64,
        /// Optional button: "left" (default), "right", or "middle"
        #[serde(skip_serializing_if = "Option::is_none")]
        button: Option<String>,
    },

    /// Response after simulating a click
    #[serde(rename = "simulateClickResult")]
    SimulateClickResult {
        #[serde(rename = "requestId")]
        request_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

impl Message {
    /// Create an arg prompt message
    pub fn arg(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Arg {
            id,
            placeholder,
            choices,
        }
    }

    /// Create a div (HTML display) message
    pub fn div(id: String, html: String) -> Self {
        Message::Div {
            id,
            html,
            tailwind: None,
        }
    }

    /// Create a div message with tailwind classes
    pub fn div_with_tailwind(id: String, html: String, tailwind: String) -> Self {
        Message::Div {
            id,
            html,
            tailwind: Some(tailwind),
        }
    }

    /// Create a submit response message
    pub fn submit(id: String, value: Option<String>) -> Self {
        Message::Submit { id, value }
    }

    /// Create an exit message
    pub fn exit(code: Option<i32>, message: Option<String>) -> Self {
        Message::Exit { code, message }
    }

    /// Get the message ID (works for message types that have an ID)
    pub fn id(&self) -> Option<&str> {
        match self {
            // Core prompts
            Message::Arg { id, .. } => Some(id),
            Message::Div { id, .. } => Some(id),
            Message::Submit { id, .. } => Some(id),
            Message::Update { id, .. } => Some(id),
            Message::Exit { .. } => None,
            // Text input prompts
            Message::Editor { id, .. } => Some(id),
            Message::Mini { id, .. } => Some(id),
            Message::Micro { id, .. } => Some(id),
            // Selection prompts
            Message::Select { id, .. } => Some(id),
            // Form prompts
            Message::Fields { id, .. } => Some(id),
            Message::Form { id, .. } => Some(id),
            // File/path prompts
            Message::Path { id, .. } => Some(id),
            Message::Drop { id, .. } => Some(id),
            // Input capture prompts
            Message::Hotkey { id, .. } => Some(id),
            // Template/text prompts
            Message::Template { id, .. } => Some(id),
            Message::Env { id, .. } => Some(id),
            // Media prompts
            Message::Chat { id, .. } => Some(id),
            Message::Term { id, .. } => Some(id),
            Message::Widget { id, .. } => Some(id),
            Message::Webcam { id, .. } => Some(id),
            Message::Mic { id, .. } => Some(id),
            // Notification/feedback (no ID)
            Message::Notify { .. } => None,
            Message::Beep {} => None,
            Message::Say { .. } => None,
            Message::SetStatus { .. } => None,
            Message::Hud { .. } => None,
            // System control (no ID)
            Message::Menu { .. } => None,
            Message::Clipboard { id, .. } => id.as_deref(),
            Message::Keyboard { .. } => None,
            Message::Mouse { .. } => None,
            Message::Show {} => None,
            Message::Hide {} => None,
            Message::Browse { .. } => None,
            Message::Exec { .. } => None,
            // UI update (no ID)
            Message::SetPanel { .. } => None,
            Message::SetPreview { .. } => None,
            Message::SetPrompt { .. } => None,
            // Selected text operations (use request_id)
            Message::GetSelectedText { request_id, .. } => Some(request_id),
            Message::SetSelectedText { request_id, .. } => Some(request_id),
            Message::CheckAccessibility { request_id, .. } => Some(request_id),
            Message::RequestAccessibility { request_id, .. } => Some(request_id),
            Message::SelectedText { request_id, .. } => Some(request_id),
            Message::TextSet { request_id, .. } => Some(request_id),
            Message::AccessibilityStatus { request_id, .. } => Some(request_id),
            // Window information (use request_id)
            Message::GetWindowBounds { request_id, .. } => Some(request_id),
            Message::WindowBounds { request_id, .. } => Some(request_id),
            // Clipboard history (use request_id)
            Message::ClipboardHistory { request_id, .. } => Some(request_id),
            Message::ClipboardHistoryEntry { request_id, .. } => Some(request_id),
            Message::ClipboardHistoryList { request_id, .. } => Some(request_id),
            Message::ClipboardHistoryResult { request_id, .. } => Some(request_id),
            // Window management (use request_id)
            Message::WindowList { request_id, .. } => Some(request_id),
            Message::WindowAction { request_id, .. } => Some(request_id),
            Message::WindowListResult { request_id, .. } => Some(request_id),
            Message::WindowActionResult { request_id, .. } => Some(request_id),
            // File search (use request_id)
            Message::FileSearch { request_id, .. } => Some(request_id),
            Message::FileSearchResult { request_id, .. } => Some(request_id),
            // Screenshot capture (use request_id)
            Message::CaptureScreenshot { request_id, .. } => Some(request_id),
            Message::ScreenshotResult { request_id, .. } => Some(request_id),
            // State query (use request_id)
            Message::GetState { request_id, .. } => Some(request_id),
            Message::StateResult { request_id, .. } => Some(request_id),
            // Element query (use request_id)
            Message::GetElements { request_id, .. } => Some(request_id),
            Message::ElementsResult { request_id, .. } => Some(request_id),
            // Error reporting (no ID)
            Message::SetError { .. } => None,
            // Force submit (no ID - operates on current prompt)
            Message::ForceSubmit { .. } => None,
            // Scriptlet operations (use request_id)
            Message::RunScriptlet { request_id, .. } => Some(request_id),
            Message::GetScriptlets { request_id, .. } => Some(request_id),
            Message::ScriptletList { request_id, .. } => Some(request_id),
            Message::ScriptletResult { request_id, .. } => Some(request_id),
            // Test infrastructure (use request_id)
            Message::SimulateClick { request_id, .. } => Some(request_id),
            Message::SimulateClickResult { request_id, .. } => Some(request_id),
        }
    }

    // ============================================================
    // Constructor methods for new message types
    // ============================================================

    /// Create an editor prompt message
    pub fn editor(id: String) -> Self {
        Message::Editor {
            id,
            content: None,
            language: None,
            on_init: None,
            on_submit: None,
        }
    }

    /// Create an editor with content and language
    pub fn editor_with_content(id: String, content: String, language: Option<String>) -> Self {
        Message::Editor {
            id,
            content: Some(content),
            language,
            on_init: None,
            on_submit: None,
        }
    }

    /// Create a mini prompt message
    pub fn mini(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Mini {
            id,
            placeholder,
            choices,
        }
    }

    /// Create a micro prompt message
    pub fn micro(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Micro {
            id,
            placeholder,
            choices,
        }
    }

    /// Create a select prompt message
    pub fn select(id: String, placeholder: String, choices: Vec<Choice>, multiple: bool) -> Self {
        Message::Select {
            id,
            placeholder,
            choices,
            multiple: if multiple { Some(true) } else { None },
        }
    }

    /// Create a fields prompt message
    pub fn fields(id: String, fields: Vec<Field>) -> Self {
        Message::Fields { id, fields }
    }

    /// Create a form prompt message
    pub fn form(id: String, html: String) -> Self {
        Message::Form { id, html }
    }

    /// Create a path prompt message
    pub fn path(id: String, start_path: Option<String>) -> Self {
        Message::Path {
            id,
            start_path,
            hint: None,
        }
    }

    /// Create a drop zone message
    pub fn drop(id: String) -> Self {
        Message::Drop { id }
    }

    /// Create a hotkey prompt message
    pub fn hotkey(id: String) -> Self {
        Message::Hotkey {
            id,
            placeholder: None,
        }
    }

    /// Create a template prompt message
    pub fn template(id: String, template: String) -> Self {
        Message::Template { id, template }
    }

    /// Create an env prompt message
    pub fn env(id: String, key: String, secret: bool) -> Self {
        Message::Env {
            id,
            key,
            secret: if secret { Some(true) } else { None },
        }
    }

    /// Create a chat prompt message
    pub fn chat(id: String) -> Self {
        Message::Chat { id }
    }

    /// Create a term prompt message
    pub fn term(id: String, command: Option<String>) -> Self {
        Message::Term { id, command }
    }

    /// Create a widget message
    pub fn widget(id: String, html: String) -> Self {
        Message::Widget {
            id,
            html,
            options: None,
        }
    }

    /// Create a webcam prompt message
    pub fn webcam(id: String) -> Self {
        Message::Webcam { id }
    }

    /// Create a mic prompt message
    pub fn mic(id: String) -> Self {
        Message::Mic { id }
    }

    /// Create a notify message
    pub fn notify(title: Option<String>, body: Option<String>) -> Self {
        Message::Notify { title, body }
    }

    /// Create a beep message
    pub fn beep() -> Self {
        Message::Beep {}
    }

    /// Create a say message
    pub fn say(text: String, voice: Option<String>) -> Self {
        Message::Say { text, voice }
    }

    /// Create a set status message
    pub fn set_status(status: String, message: Option<String>) -> Self {
        Message::SetStatus { status, message }
    }

    /// Create a HUD overlay message
    pub fn hud(text: String, duration_ms: Option<u64>) -> Self {
        Message::Hud { text, duration_ms }
    }

    /// Create a menu message
    pub fn menu(icon: Option<String>, scripts: Option<Vec<String>>) -> Self {
        Message::Menu { icon, scripts }
    }

    /// Create a clipboard read message
    pub fn clipboard_read(format: Option<ClipboardFormat>) -> Self {
        Message::Clipboard {
            id: None,
            action: ClipboardAction::Read,
            format,
            content: None,
        }
    }

    /// Create a clipboard write message
    pub fn clipboard_write(content: String, format: Option<ClipboardFormat>) -> Self {
        Message::Clipboard {
            id: None,
            action: ClipboardAction::Write,
            format,
            content: Some(content),
        }
    }

    /// Create a keyboard type message
    pub fn keyboard_type(keys: String) -> Self {
        Message::Keyboard {
            action: KeyboardAction::Type,
            keys: Some(keys),
        }
    }

    /// Create a keyboard tap message
    pub fn keyboard_tap(keys: String) -> Self {
        Message::Keyboard {
            action: KeyboardAction::Tap,
            keys: Some(keys),
        }
    }

    /// Create a mouse message
    pub fn mouse(action: MouseAction, data: Option<serde_json::Value>) -> Self {
        Message::Mouse { action, data }
    }

    /// Create a show message
    pub fn show() -> Self {
        Message::Show {}
    }

    /// Create a hide message
    pub fn hide() -> Self {
        Message::Hide {}
    }

    /// Create a browse message to open URL in default browser
    pub fn browse(url: String) -> Self {
        Message::Browse { url }
    }

    /// Create an exec message
    pub fn exec(command: String, options: Option<serde_json::Value>) -> Self {
        Message::Exec { command, options }
    }

    /// Create a set panel message
    pub fn set_panel(html: String) -> Self {
        Message::SetPanel { html }
    }

    /// Create a set preview message
    pub fn set_preview(html: String) -> Self {
        Message::SetPreview { html }
    }

    /// Create a set prompt message
    pub fn set_prompt(html: String) -> Self {
        Message::SetPrompt { html }
    }

    // ============================================================
    // Constructor methods for selected text operations
    // ============================================================

    /// Create a get selected text request
    pub fn get_selected_text(request_id: String) -> Self {
        Message::GetSelectedText { request_id }
    }

    /// Create a set selected text request
    pub fn set_selected_text_msg(text: String, request_id: String) -> Self {
        Message::SetSelectedText { text, request_id }
    }

    /// Create a check accessibility request
    pub fn check_accessibility(request_id: String) -> Self {
        Message::CheckAccessibility { request_id }
    }

    /// Create a request accessibility request
    pub fn request_accessibility(request_id: String) -> Self {
        Message::RequestAccessibility { request_id }
    }

    /// Create a selected text response
    pub fn selected_text_response(text: String, request_id: String) -> Self {
        Message::SelectedText { text, request_id }
    }

    /// Create a text set response (success)
    pub fn text_set_success(request_id: String) -> Self {
        Message::TextSet {
            success: true,
            error: None,
            request_id,
        }
    }

    /// Create a text set response (error)
    pub fn text_set_error(error: String, request_id: String) -> Self {
        Message::TextSet {
            success: false,
            error: Some(error),
            request_id,
        }
    }

    /// Create an accessibility status response
    pub fn accessibility_status(granted: bool, request_id: String) -> Self {
        Message::AccessibilityStatus {
            granted,
            request_id,
        }
    }

    // ============================================================
    // Constructor methods for window information
    // ============================================================

    /// Create a get window bounds request
    pub fn get_window_bounds(request_id: String) -> Self {
        Message::GetWindowBounds { request_id }
    }

    /// Create a window bounds response
    pub fn window_bounds(x: f64, y: f64, width: f64, height: f64, request_id: String) -> Self {
        Message::WindowBounds {
            x,
            y,
            width,
            height,
            request_id,
        }
    }

    // ============================================================
    // Constructor methods for clipboard history
    // ============================================================

    /// Create a clipboard history list request
    pub fn clipboard_history_list(request_id: String) -> Self {
        Message::ClipboardHistory {
            request_id,
            action: ClipboardHistoryAction::List,
            entry_id: None,
        }
    }

    /// Create a clipboard history pin request
    pub fn clipboard_history_pin(request_id: String, entry_id: String) -> Self {
        Message::ClipboardHistory {
            request_id,
            action: ClipboardHistoryAction::Pin,
            entry_id: Some(entry_id),
        }
    }

    /// Create a clipboard history unpin request
    pub fn clipboard_history_unpin(request_id: String, entry_id: String) -> Self {
        Message::ClipboardHistory {
            request_id,
            action: ClipboardHistoryAction::Unpin,
            entry_id: Some(entry_id),
        }
    }

    /// Create a clipboard history remove request
    pub fn clipboard_history_remove(request_id: String, entry_id: String) -> Self {
        Message::ClipboardHistory {
            request_id,
            action: ClipboardHistoryAction::Remove,
            entry_id: Some(entry_id),
        }
    }

    /// Create a clipboard history clear request
    pub fn clipboard_history_clear(request_id: String) -> Self {
        Message::ClipboardHistory {
            request_id,
            action: ClipboardHistoryAction::Clear,
            entry_id: None,
        }
    }

    /// Create a clipboard history entry response
    pub fn clipboard_history_entry(
        request_id: String,
        entry_id: String,
        content: String,
        content_type: ClipboardEntryType,
        timestamp: String,
        pinned: bool,
    ) -> Self {
        Message::ClipboardHistoryEntry {
            request_id,
            entry_id,
            content,
            content_type,
            timestamp,
            pinned,
        }
    }

    /// Create a clipboard history list response
    pub fn clipboard_history_list_response(
        request_id: String,
        entries: Vec<ClipboardHistoryEntryData>,
    ) -> Self {
        Message::ClipboardHistoryList {
            request_id,
            entries,
        }
    }

    /// Create a clipboard history result (success)
    pub fn clipboard_history_success(request_id: String) -> Self {
        Message::ClipboardHistoryResult {
            request_id,
            success: true,
            error: None,
        }
    }

    /// Create a clipboard history result (error)
    pub fn clipboard_history_error(request_id: String, error: String) -> Self {
        Message::ClipboardHistoryResult {
            request_id,
            success: false,
            error: Some(error),
        }
    }

    // ============================================================
    // Constructor methods for window management
    // ============================================================

    /// Create a window list request
    pub fn window_list(request_id: String) -> Self {
        Message::WindowList { request_id }
    }

    /// Create a window action request
    pub fn window_action(
        request_id: String,
        action: WindowActionType,
        window_id: Option<u32>,
        bounds: Option<TargetWindowBounds>,
    ) -> Self {
        Message::WindowAction {
            request_id,
            action,
            window_id,
            bounds,
        }
    }

    /// Create a window list response
    pub fn window_list_result(request_id: String, windows: Vec<SystemWindowInfo>) -> Self {
        Message::WindowListResult {
            request_id,
            windows,
        }
    }

    /// Create a window action result (success)
    pub fn window_action_success(request_id: String) -> Self {
        Message::WindowActionResult {
            request_id,
            success: true,
            error: None,
        }
    }

    /// Create a window action result (error)
    pub fn window_action_error(request_id: String, error: String) -> Self {
        Message::WindowActionResult {
            request_id,
            success: false,
            error: Some(error),
        }
    }

    // ============================================================
    // Constructor methods for file search
    // ============================================================

    /// Create a file search request
    pub fn file_search(request_id: String, query: String, only_in: Option<String>) -> Self {
        Message::FileSearch {
            request_id,
            query,
            only_in,
        }
    }

    /// Create a file search result response
    pub fn file_search_result(request_id: String, files: Vec<FileSearchResultEntry>) -> Self {
        Message::FileSearchResult { request_id, files }
    }

    // ============================================================
    // Constructor methods for screenshot capture
    // ============================================================

    /// Create a capture screenshot request
    pub fn capture_screenshot(request_id: String) -> Self {
        Message::CaptureScreenshot { request_id }
    }

    /// Create a screenshot result response
    pub fn screenshot_result(request_id: String, data: String, width: u32, height: u32) -> Self {
        Message::ScreenshotResult {
            request_id,
            data,
            width,
            height,
        }
    }

    // ============================================================
    // Constructor methods for state query
    // ============================================================

    /// Create a get state request
    pub fn get_state(request_id: String) -> Self {
        Message::GetState { request_id }
    }

    /// Create a state result response
    #[allow(clippy::too_many_arguments)]
    pub fn state_result(
        request_id: String,
        prompt_type: String,
        prompt_id: Option<String>,
        placeholder: Option<String>,
        input_value: String,
        choice_count: usize,
        visible_choice_count: usize,
        selected_index: i32,
        selected_value: Option<String>,
        is_focused: bool,
        window_visible: bool,
    ) -> Self {
        Message::StateResult {
            request_id,
            prompt_type,
            prompt_id,
            placeholder,
            input_value,
            choice_count,
            visible_choice_count,
            selected_index,
            selected_value,
            is_focused,
            window_visible,
        }
    }

    // ============================================================
    // Constructor methods for element query
    // ============================================================

    /// Create a get elements request
    pub fn get_elements(request_id: String) -> Self {
        Message::GetElements {
            request_id,
            limit: None,
        }
    }

    /// Create a get elements request with limit
    pub fn get_elements_with_limit(request_id: String, limit: usize) -> Self {
        Message::GetElements {
            request_id,
            limit: Some(limit),
        }
    }

    /// Create an elements result response
    pub fn elements_result(
        request_id: String,
        elements: Vec<ElementInfo>,
        total_count: usize,
    ) -> Self {
        Message::ElementsResult {
            request_id,
            elements,
            total_count,
        }
    }

    // ============================================================
    // Constructor methods for error reporting
    // ============================================================

    /// Create a script error message from ScriptErrorData
    pub fn set_error(error_data: ScriptErrorData) -> Self {
        Message::SetError {
            error_message: error_data.error_message,
            stderr_output: error_data.stderr_output,
            exit_code: error_data.exit_code,
            stack_trace: error_data.stack_trace,
            script_path: error_data.script_path,
            suggestions: error_data.suggestions,
            timestamp: error_data.timestamp,
        }
    }

    /// Create a simple script error message with just the message and path
    pub fn script_error(error_message: String, script_path: String) -> Self {
        Message::SetError {
            error_message,
            stderr_output: None,
            exit_code: None,
            stack_trace: None,
            script_path,
            suggestions: Vec::new(),
            timestamp: None,
        }
    }

    /// Create a full script error message with all optional fields
    pub fn script_error_full(
        error_message: String,
        script_path: String,
        stderr_output: Option<String>,
        exit_code: Option<i32>,
        stack_trace: Option<String>,
        suggestions: Vec<String>,
        timestamp: Option<String>,
    ) -> Self {
        Message::SetError {
            error_message,
            stderr_output,
            exit_code,
            stack_trace,
            script_path,
            suggestions,
            timestamp,
        }
    }

    // ============================================================
    // Constructor methods for scriptlet operations
    // ============================================================

    /// Create a run scriptlet request
    pub fn run_scriptlet(
        request_id: String,
        scriptlet: ScriptletData,
        inputs: Option<serde_json::Value>,
        args: Vec<String>,
    ) -> Self {
        Message::RunScriptlet {
            request_id,
            scriptlet,
            inputs,
            args,
        }
    }

    /// Create a get scriptlets request
    pub fn get_scriptlets(request_id: String) -> Self {
        Message::GetScriptlets {
            request_id,
            kenv: None,
            group: None,
        }
    }

    /// Create a get scriptlets request with filters
    pub fn get_scriptlets_filtered(
        request_id: String,
        kenv: Option<String>,
        group: Option<String>,
    ) -> Self {
        Message::GetScriptlets {
            request_id,
            kenv,
            group,
        }
    }

    /// Create a scriptlet list response
    pub fn scriptlet_list(request_id: String, scriptlets: Vec<ScriptletData>) -> Self {
        Message::ScriptletList {
            request_id,
            scriptlets,
        }
    }

    /// Create a successful scriptlet result
    pub fn scriptlet_result_success(
        request_id: String,
        output: Option<String>,
        exit_code: Option<i32>,
    ) -> Self {
        Message::ScriptletResult {
            request_id,
            success: true,
            output,
            error: None,
            exit_code,
        }
    }

    /// Create a failed scriptlet result
    pub fn scriptlet_result_error(
        request_id: String,
        error: String,
        exit_code: Option<i32>,
    ) -> Self {
        Message::ScriptletResult {
            request_id,
            success: false,
            output: None,
            error: Some(error),
            exit_code,
        }
    }

    // ============================================================
    // Constructor methods for test infrastructure
    // ============================================================

    /// Create a simulate click request
    ///
    /// Coordinates are relative to the window's content area.
    pub fn simulate_click(request_id: String, x: f64, y: f64) -> Self {
        Message::SimulateClick {
            request_id,
            x,
            y,
            button: None,
        }
    }

    /// Create a simulate click request with a specific button
    ///
    /// Coordinates are relative to the window's content area.
    /// Button can be "left", "right", or "middle".
    pub fn simulate_click_with_button(request_id: String, x: f64, y: f64, button: String) -> Self {
        Message::SimulateClick {
            request_id,
            x,
            y,
            button: Some(button),
        }
    }

    /// Create a successful simulate click result
    pub fn simulate_click_success(request_id: String) -> Self {
        Message::SimulateClickResult {
            request_id,
            success: true,
            error: None,
        }
    }

    /// Create a failed simulate click result
    pub fn simulate_click_error(request_id: String, error: String) -> Self {
        Message::SimulateClickResult {
            request_id,
            success: false,
            error: Some(error),
        }
    }
}

/// Parse a single JSONL message from a string
///
/// # Arguments
/// * `line` - A JSON string (typically one line from JSONL)
///
/// # Returns
/// * `Result<Message, serde_json::Error>` - Parsed message or deserialization error
pub fn parse_message(line: &str) -> Result<Message, serde_json::Error> {
    serde_json::from_str(line).map_err(|e| {
        // Log the raw input and error for debugging
        warn!(
            raw_input = %line,
            error = %e,
            "Failed to parse JSONL message"
        );
        e
    })
}

/// Result type for graceful message parsing
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ParseResult {
    /// Successfully parsed a known message type
    Ok(Message),
    /// Unknown message type - contains the type name and raw JSON
    UnknownType { message_type: String, raw: String },
    /// JSON parsing failed entirely
    ParseError(serde_json::Error),
}

/// Parse a message with graceful handling of unknown types
///
/// Unlike `parse_message`, this function handles unknown message types
/// gracefully by logging a warning and returning `ParseResult::UnknownType`
/// instead of failing.
///
/// # Arguments
/// * `line` - A JSON string (typically one line from JSONL)
///
/// # Returns
/// * `ParseResult` - Either a parsed message, unknown type info, or parse error
///
/// # Performance
/// This function uses single-parse optimization: it parses to serde_json::Value
/// first, then converts to Message. This avoids double-parsing on unknown types.
pub fn parse_message_graceful(line: &str) -> ParseResult {
    // P1-11 FIX: Single parse - parse to Value first, then convert
    // This avoids double-parsing: previously we tried Message first, then Value on failure
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                raw_input = %line,
                error = %e,
                "Failed to parse JSONL message - invalid JSON"
            );
            return ParseResult::ParseError(e);
        }
    };

    // Check for type field and extract it as owned String before consuming value
    let msg_type: String = match value.get("type").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => {
            // Missing type field - create a synthetic error
            let e = serde_json::from_str::<Message>("{}").unwrap_err();
            warn!(
                raw_input = %line,
                "Failed to parse JSONL message - missing 'type' field"
            );
            return ParseResult::ParseError(e);
        }
    };

    // Try to convert Value to Message (consumes value)
    match serde_json::from_value::<Message>(value) {
        Ok(msg) => {
            debug!(message_id = ?msg.id(), "Successfully parsed message");
            ParseResult::Ok(msg)
        }
        Err(_) => {
            // Valid JSON with "type" field, but unknown type
            warn!(
                message_type = %msg_type,
                raw_input = %line,
                "Unknown message type received - ignoring"
            );
            ParseResult::UnknownType {
                message_type: msg_type,
                raw: line.to_string(),
            }
        }
    }
}

/// Serialize a message to JSONL format
///
/// # Arguments
/// * `msg` - The message to serialize
///
/// # Returns
/// * `Result<String, serde_json::Error>` - JSON string (without newline)
pub fn serialize_message(msg: &Message) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

/// JSONL reader for streaming/chunked message reads
///
/// Provides utilities to read messages one at a time from a reader.
///
/// # Performance
/// Uses a reusable line buffer to avoid allocating a new String per line read.
/// The buffer is cleared and reused between reads (P1-12 optimization).
pub struct JsonlReader<R: Read> {
    reader: BufReader<R>,
    /// Reusable line buffer - cleared and reused per read to avoid allocations
    line_buffer: String,
}

impl<R: Read> JsonlReader<R> {
    /// Create a new JSONL reader
    pub fn new(reader: R) -> Self {
        JsonlReader {
            reader: BufReader::new(reader),
            // Pre-allocate reasonable capacity for typical JSON messages
            line_buffer: String::with_capacity(1024),
        }
    }

    /// Read the next message from the stream
    ///
    /// # Returns
    /// * `Ok(Some(Message))` - Successfully parsed message
    /// * `Ok(None)` - End of stream
    /// * `Err(e)` - Parse error
    pub fn next_message(&mut self) -> Result<Option<Message>, Box<dyn std::error::Error>> {
        // P1-12 FIX: Reuse buffer instead of allocating new String each call
        self.line_buffer.clear();
        match self.reader.read_line(&mut self.line_buffer)? {
            0 => {
                debug!("Reached end of JSONL stream");
                Ok(None)
            }
            bytes_read => {
                debug!(bytes_read, "Read line from JSONL stream");
                let trimmed = self.line_buffer.trim();
                if trimmed.is_empty() {
                    debug!("Skipping empty line in JSONL stream");
                    return self.next_message(); // Skip empty lines
                }
                let msg = parse_message(trimmed)?;
                Ok(Some(msg))
            }
        }
    }

    /// Read the next message with graceful unknown type handling
    ///
    /// Unlike `next_message`, this method uses `parse_message_graceful` to
    /// handle unknown message types without errors. Unknown types are logged
    /// and skipped, continuing to read the next message.
    ///
    /// # Returns
    /// * `Ok(Some(Message))` - Successfully parsed known message
    /// * `Ok(None)` - End of stream
    /// * `Err(e)` - IO error (not parse errors for unknown types)
    pub fn next_message_graceful(&mut self) -> Result<Option<Message>, std::io::Error> {
        loop {
            // P1-12 FIX: Reuse buffer instead of allocating new String each iteration
            self.line_buffer.clear();
            match self.reader.read_line(&mut self.line_buffer)? {
                0 => {
                    debug!("Reached end of JSONL stream");
                    return Ok(None);
                }
                _ => {
                    let trimmed = self.line_buffer.trim();
                    if trimmed.is_empty() {
                        debug!("Skipping empty line in JSONL stream");
                        continue;
                    }

                    match parse_message_graceful(trimmed) {
                        ParseResult::Ok(msg) => return Ok(Some(msg)),
                        ParseResult::UnknownType { message_type, .. } => {
                            // Already logged in parse_message_graceful
                            debug!(
                                message_type = %message_type,
                                "Skipping unknown message type, continuing to next message"
                            );
                            continue;
                        }
                        ParseResult::ParseError(e) => {
                            // Log but continue - graceful degradation
                            warn!(
                                error = %e,
                                "Skipping malformed message, continuing to next message"
                            );
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choice_creation() {
        let choice = Choice::new("Apple".to_string(), "apple".to_string());
        assert_eq!(choice.name, "Apple");
        assert_eq!(choice.value, "apple");
        assert_eq!(choice.description, None);
    }

    #[test]
    fn test_choice_with_description() {
        let choice = Choice::with_description(
            "Apple".to_string(),
            "apple".to_string(),
            "A red fruit".to_string(),
        );
        assert_eq!(choice.name, "Apple");
        assert_eq!(choice.value, "apple");
        assert_eq!(choice.description, Some("A red fruit".to_string()));
    }

    #[test]
    fn test_serialize_arg_message() {
        let choices = vec![
            Choice::new("Apple".to_string(), "apple".to_string()),
            Choice::new("Banana".to_string(), "banana".to_string()),
        ];
        let msg = Message::arg("1".to_string(), "Pick one".to_string(), choices);

        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"arg\""));
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"placeholder\":\"Pick one\""));
        assert!(json.contains("\"Apple\""));
    }

    #[test]
    fn test_parse_arg_message() {
        let json = r#"{"type":"arg","id":"1","placeholder":"Pick one","choices":[{"name":"Apple","value":"apple"},{"name":"Banana","value":"banana"}]}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Arg {
                id,
                placeholder,
                choices,
            } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, "Pick one");
                assert_eq!(choices.len(), 2);
                assert_eq!(choices[0].name, "Apple");
                assert_eq!(choices[0].value, "apple");
            }
            _ => panic!("Expected Arg message"),
        }
    }

    #[test]
    fn test_parse_div_message() {
        let json = r#"{"type":"div","id":"2","html":"<h1>Hello</h1>"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Div { id, html, tailwind } => {
                assert_eq!(id, "2");
                assert_eq!(html, "<h1>Hello</h1>");
                assert_eq!(tailwind, None);
            }
            _ => panic!("Expected Div message"),
        }
    }

    #[test]
    fn test_parse_div_with_tailwind() {
        let json =
            r#"{"type":"div","id":"2","html":"<h1>Hello</h1>","tailwind":"text-2xl font-bold"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Div { id, html, tailwind } => {
                assert_eq!(id, "2");
                assert_eq!(html, "<h1>Hello</h1>");
                assert_eq!(tailwind, Some("text-2xl font-bold".to_string()));
            }
            _ => panic!("Expected Div message"),
        }
    }

    #[test]
    fn test_parse_submit_message() {
        let json = r#"{"type":"submit","id":"1","value":"apple"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Submit { id, value } => {
                assert_eq!(id, "1");
                assert_eq!(value, Some("apple".to_string()));
            }
            _ => panic!("Expected Submit message"),
        }
    }

    #[test]
    fn test_parse_submit_null_value() {
        let json = r#"{"type":"submit","id":"2","value":null}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Submit { id, value } => {
                assert_eq!(id, "2");
                assert_eq!(value, None);
            }
            _ => panic!("Expected Submit message"),
        }
    }

    #[test]
    fn test_parse_exit_message() {
        let json = r#"{"type":"exit","code":0,"message":"Success"}"#;
        let msg = parse_message(json).unwrap();

        match msg {
            Message::Exit { code, message } => {
                assert_eq!(code, Some(0));
                assert_eq!(message, Some("Success".to_string()));
            }
            _ => panic!("Expected Exit message"),
        }
    }

    #[test]
    fn test_message_id() {
        let arg_msg = Message::arg("1".to_string(), "Pick".to_string(), vec![]);
        assert_eq!(arg_msg.id(), Some("1"));

        let div_msg = Message::div("2".to_string(), "<h1>Hi</h1>".to_string());
        assert_eq!(div_msg.id(), Some("2"));

        let exit_msg = Message::exit(None, None);
        assert_eq!(exit_msg.id(), None);
    }

    #[test]
    fn test_jsonl_reader() {
        let _jsonl = "\"type\":\"arg\",\"id\":\"1\",\"placeholder\":\"Pick\",\"choices\":[]}\n{\"type\":\"submit\",\"id\":\"1\",\"value\":\"apple\"}";
        // Note: This test uses a partial JSON to ensure line-by-line reading
        // A real test would need complete valid JSON lines
    }

    // ============================================================
    // FIELD STRUCT TESTS
    // ============================================================

    #[test]
    fn test_field_creation() {
        let field = Field::new("username".to_string());
        assert_eq!(field.name, "username");
        assert_eq!(field.label, None);
        assert_eq!(field.field_type, None);
    }

    #[test]
    fn test_field_builder() {
        let field = Field::new("email".to_string())
            .with_label("Email Address".to_string())
            .with_type("email".to_string())
            .with_placeholder("Enter your email".to_string());

        assert_eq!(field.name, "email");
        assert_eq!(field.label, Some("Email Address".to_string()));
        assert_eq!(field.field_type, Some("email".to_string()));
        assert_eq!(field.placeholder, Some("Enter your email".to_string()));
    }

    // ============================================================
    // TEXT INPUT PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_editor_message() {
        let msg = Message::editor("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"editor\""));
        assert!(json.contains("\"id\":\"1\""));
        // Optional fields should not be present when None
        assert!(!json.contains("\"content\""));
    }

    #[test]
    fn test_parse_editor_message() {
        let json = r#"{"type":"editor","id":"1","content":"hello","language":"javascript"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Editor {
                id,
                content,
                language,
                ..
            } => {
                assert_eq!(id, "1");
                assert_eq!(content, Some("hello".to_string()));
                assert_eq!(language, Some("javascript".to_string()));
            }
            _ => panic!("Expected Editor message"),
        }
    }

    #[test]
    fn test_serialize_mini_message() {
        let choices = vec![Choice::new("A".to_string(), "a".to_string())];
        let msg = Message::mini("1".to_string(), "Quick pick".to_string(), choices);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"mini\""));
    }

    #[test]
    fn test_parse_mini_message() {
        let json = r#"{"type":"mini","id":"1","placeholder":"Quick","choices":[]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Mini {
                id,
                placeholder,
                choices,
            } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, "Quick");
                assert!(choices.is_empty());
            }
            _ => panic!("Expected Mini message"),
        }
    }

    #[test]
    fn test_serialize_micro_message() {
        let msg = Message::micro("1".to_string(), "Tiny".to_string(), vec![]);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"micro\""));
    }

    #[test]
    fn test_parse_micro_message() {
        let json = r#"{"type":"micro","id":"1","placeholder":"Tiny","choices":[]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Micro {
                id, placeholder, ..
            } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, "Tiny");
            }
            _ => panic!("Expected Micro message"),
        }
    }

    // ============================================================
    // SELECTION PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_select_message() {
        let choices = vec![Choice::new("A".to_string(), "a".to_string())];
        let msg = Message::select("1".to_string(), "Pick".to_string(), choices, true);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"select\""));
        assert!(json.contains("\"multiple\":true"));
    }

    #[test]
    fn test_parse_select_message() {
        let json =
            r#"{"type":"select","id":"1","placeholder":"Pick","choices":[],"multiple":true}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Select { id, multiple, .. } => {
                assert_eq!(id, "1");
                assert_eq!(multiple, Some(true));
            }
            _ => panic!("Expected Select message"),
        }
    }

    // ============================================================
    // FORM PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_fields_message() {
        let fields = vec![
            Field::new("name".to_string()).with_label("Name".to_string()),
            Field::new("email".to_string()).with_type("email".to_string()),
        ];
        let msg = Message::fields("1".to_string(), fields);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"fields\""));
        assert!(json.contains("\"name\""));
    }

    #[test]
    fn test_parse_fields_message() {
        let json =
            r#"{"type":"fields","id":"1","fields":[{"name":"username","label":"Username"}]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Fields { id, fields } => {
                assert_eq!(id, "1");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "username");
            }
            _ => panic!("Expected Fields message"),
        }
    }

    #[test]
    fn test_serialize_form_message() {
        let msg = Message::form("1".to_string(), "<form>...</form>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"form\""));
        assert!(json.contains("\"html\""));
    }

    #[test]
    fn test_parse_form_message() {
        let json = r#"{"type":"form","id":"1","html":"<form></form>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Form { id, html } => {
                assert_eq!(id, "1");
                assert_eq!(html, "<form></form>");
            }
            _ => panic!("Expected Form message"),
        }
    }

    // ============================================================
    // FILE/PATH PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_path_message() {
        let msg = Message::path("1".to_string(), Some("/home".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"path\""));
        assert!(json.contains("\"startPath\":\"/home\""));
    }

    #[test]
    fn test_parse_path_message() {
        let json = r#"{"type":"path","id":"1","startPath":"/home","hint":"Select folder"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Path {
                id,
                start_path,
                hint,
            } => {
                assert_eq!(id, "1");
                assert_eq!(start_path, Some("/home".to_string()));
                assert_eq!(hint, Some("Select folder".to_string()));
            }
            _ => panic!("Expected Path message"),
        }
    }

    #[test]
    fn test_serialize_drop_message() {
        let msg = Message::drop("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"drop\""));
    }

    #[test]
    fn test_parse_drop_message() {
        let json = r#"{"type":"drop","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Drop { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Drop message"),
        }
    }

    // ============================================================
    // INPUT CAPTURE PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_hotkey_message() {
        let msg = Message::hotkey("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"hotkey\""));
    }

    #[test]
    fn test_parse_hotkey_message() {
        let json = r#"{"type":"hotkey","id":"1","placeholder":"Press a key"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Hotkey { id, placeholder } => {
                assert_eq!(id, "1");
                assert_eq!(placeholder, Some("Press a key".to_string()));
            }
            _ => panic!("Expected Hotkey message"),
        }
    }

    // ============================================================
    // TEMPLATE/TEXT PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_template_message() {
        let msg = Message::template("1".to_string(), "Hello {{name}}!".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"template\""));
        assert!(json.contains("Hello {{name}}!"));
    }

    #[test]
    fn test_parse_template_message() {
        let json = r#"{"type":"template","id":"1","template":"Hi {{name}}"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Template { id, template } => {
                assert_eq!(id, "1");
                assert_eq!(template, "Hi {{name}}");
            }
            _ => panic!("Expected Template message"),
        }
    }

    #[test]
    fn test_serialize_env_message() {
        let msg = Message::env("1".to_string(), "API_KEY".to_string(), true);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"env\""));
        assert!(json.contains("\"key\":\"API_KEY\""));
        assert!(json.contains("\"secret\":true"));
    }

    #[test]
    fn test_parse_env_message() {
        let json = r#"{"type":"env","id":"1","key":"SECRET","secret":true}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Env { id, key, secret } => {
                assert_eq!(id, "1");
                assert_eq!(key, "SECRET");
                assert_eq!(secret, Some(true));
            }
            _ => panic!("Expected Env message"),
        }
    }

    // ============================================================
    // MEDIA PROMPT TESTS
    // ============================================================

    #[test]
    fn test_serialize_chat_message() {
        let msg = Message::chat("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"chat\""));
    }

    #[test]
    fn test_parse_chat_message() {
        let json = r#"{"type":"chat","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Chat { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Chat message"),
        }
    }

    #[test]
    fn test_serialize_term_message() {
        let msg = Message::term("1".to_string(), Some("ls -la".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"term\""));
        assert!(json.contains("\"command\":\"ls -la\""));
    }

    #[test]
    fn test_parse_term_message() {
        let json = r#"{"type":"term","id":"1","command":"pwd"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Term { id, command } => {
                assert_eq!(id, "1");
                assert_eq!(command, Some("pwd".to_string()));
            }
            _ => panic!("Expected Term message"),
        }
    }

    #[test]
    fn test_serialize_widget_message() {
        let msg = Message::widget("1".to_string(), "<div>Widget</div>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"widget\""));
    }

    #[test]
    fn test_parse_widget_message() {
        let json = r#"{"type":"widget","id":"1","html":"<div></div>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Widget { id, html, options } => {
                assert_eq!(id, "1");
                assert_eq!(html, "<div></div>");
                assert_eq!(options, None);
            }
            _ => panic!("Expected Widget message"),
        }
    }

    #[test]
    fn test_serialize_webcam_message() {
        let msg = Message::webcam("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"webcam\""));
    }

    #[test]
    fn test_parse_webcam_message() {
        let json = r#"{"type":"webcam","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Webcam { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Webcam message"),
        }
    }

    #[test]
    fn test_serialize_mic_message() {
        let msg = Message::mic("1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"mic\""));
    }

    #[test]
    fn test_parse_mic_message() {
        let json = r#"{"type":"mic","id":"1"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Mic { id } => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected Mic message"),
        }
    }

    // ============================================================
    // NOTIFICATION/FEEDBACK MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_notify_message() {
        let msg = Message::notify(Some("Title".to_string()), Some("Body".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"notify\""));
        assert!(json.contains("\"title\":\"Title\""));
        assert!(json.contains("\"body\":\"Body\""));
    }

    #[test]
    fn test_parse_notify_message() {
        let json = r#"{"type":"notify","title":"Alert","body":"Something happened"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Notify { title, body } => {
                assert_eq!(title, Some("Alert".to_string()));
                assert_eq!(body, Some("Something happened".to_string()));
            }
            _ => panic!("Expected Notify message"),
        }
    }

    #[test]
    fn test_serialize_beep_message() {
        let msg = Message::beep();
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"beep\""));
    }

    #[test]
    fn test_parse_beep_message() {
        let json = r#"{"type":"beep"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Beep {} => {}
            _ => panic!("Expected Beep message"),
        }
    }

    #[test]
    fn test_serialize_say_message() {
        let msg = Message::say("Hello".to_string(), Some("Alex".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"say\""));
        assert!(json.contains("\"text\":\"Hello\""));
        assert!(json.contains("\"voice\":\"Alex\""));
    }

    #[test]
    fn test_parse_say_message() {
        let json = r#"{"type":"say","text":"Hi there","voice":"Samantha"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Say { text, voice } => {
                assert_eq!(text, "Hi there");
                assert_eq!(voice, Some("Samantha".to_string()));
            }
            _ => panic!("Expected Say message"),
        }
    }

    #[test]
    fn test_serialize_set_status_message() {
        let msg = Message::set_status("busy".to_string(), Some("Working...".to_string()));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setStatus\""));
    }

    #[test]
    fn test_parse_set_status_message() {
        let json = r#"{"type":"setStatus","status":"idle","message":"Ready"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetStatus { status, message } => {
                assert_eq!(status, "idle");
                assert_eq!(message, Some("Ready".to_string()));
            }
            _ => panic!("Expected SetStatus message"),
        }
    }

    #[test]
    fn test_serialize_hud_message() {
        let msg = Message::hud("Copied!".to_string(), None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"hud\""));
        assert!(json.contains("\"text\":\"Copied!\""));
        // duration_ms should be omitted when None
        assert!(!json.contains("\"duration_ms\""));
    }

    #[test]
    fn test_serialize_hud_message_with_duration() {
        let msg = Message::hud("Warning".to_string(), Some(4000));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"hud\""));
        assert!(json.contains("\"text\":\"Warning\""));
        assert!(json.contains("\"duration_ms\":4000"));
    }

    #[test]
    fn test_parse_hud_message_basic() {
        let json = r#"{"type":"hud","text":"Copied!"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Hud { text, duration_ms } => {
                assert_eq!(text, "Copied!");
                assert_eq!(duration_ms, None);
            }
            _ => panic!("Expected Hud message"),
        }
    }

    #[test]
    fn test_parse_hud_message_with_duration() {
        let json = r#"{"type":"hud","text":"Warning","duration_ms":4000}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Hud { text, duration_ms } => {
                assert_eq!(text, "Warning");
                assert_eq!(duration_ms, Some(4000));
            }
            _ => panic!("Expected Hud message"),
        }
    }

    // ============================================================
    // SYSTEM CONTROL MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_menu_message() {
        let msg = Message::menu(Some("🚀".to_string()), Some(vec!["script1".to_string()]));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"menu\""));
    }

    #[test]
    fn test_parse_menu_message() {
        let json = r#"{"type":"menu","icon":"⚡","scripts":["a","b"]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Menu { icon, scripts } => {
                assert_eq!(icon, Some("⚡".to_string()));
                assert_eq!(scripts, Some(vec!["a".to_string(), "b".to_string()]));
            }
            _ => panic!("Expected Menu message"),
        }
    }

    #[test]
    fn test_serialize_clipboard_read_message() {
        let msg = Message::clipboard_read(Some(ClipboardFormat::Text));
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboard\""));
        assert!(json.contains("\"action\":\"read\""));
        assert!(json.contains("\"format\":\"text\""));
    }

    #[test]
    fn test_serialize_clipboard_write_message() {
        let msg = Message::clipboard_write("content".to_string(), None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"action\":\"write\""));
        assert!(json.contains("\"content\":\"content\""));
    }

    #[test]
    fn test_parse_clipboard_message() {
        let json = r#"{"type":"clipboard","action":"read","format":"image"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Clipboard {
                action,
                format,
                content,
                ..
            } => {
                assert_eq!(action, ClipboardAction::Read);
                assert_eq!(format, Some(ClipboardFormat::Image));
                assert_eq!(content, None);
            }
            _ => panic!("Expected Clipboard message"),
        }
    }

    #[test]
    fn test_serialize_keyboard_type_message() {
        let msg = Message::keyboard_type("Hello".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"keyboard\""));
        assert!(json.contains("\"action\":\"type\""));
        assert!(json.contains("\"keys\":\"Hello\""));
    }

    #[test]
    fn test_serialize_keyboard_tap_message() {
        let msg = Message::keyboard_tap("cmd+c".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"action\":\"tap\""));
    }

    #[test]
    fn test_parse_keyboard_message() {
        let json = r#"{"type":"keyboard","action":"tap","keys":"cmd+v"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Keyboard { action, keys } => {
                assert_eq!(action, KeyboardAction::Tap);
                assert_eq!(keys, Some("cmd+v".to_string()));
            }
            _ => panic!("Expected Keyboard message"),
        }
    }

    #[test]
    fn test_serialize_mouse_message() {
        let msg = Message::mouse(MouseAction::Click, None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"mouse\""));
        assert!(json.contains("\"action\":\"click\""));
    }

    #[test]
    fn test_parse_mouse_message() {
        let json = r#"{"type":"mouse","action":"move","data":{"x":100,"y":200}}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Mouse { action, data } => {
                assert_eq!(action, MouseAction::Move);
                assert!(data.is_some());
            }
            _ => panic!("Expected Mouse message"),
        }
    }

    #[test]
    fn test_serialize_show_message() {
        let msg = Message::show();
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"show\""));
    }

    #[test]
    fn test_parse_show_message() {
        let json = r#"{"type":"show"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Show {} => {}
            _ => panic!("Expected Show message"),
        }
    }

    #[test]
    fn test_serialize_hide_message() {
        let msg = Message::hide();
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"hide\""));
    }

    #[test]
    fn test_parse_hide_message() {
        let json = r#"{"type":"hide"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Hide {} => {}
            _ => panic!("Expected Hide message"),
        }
    }

    #[test]
    fn test_serialize_exec_message() {
        let msg = Message::exec("ls -la".to_string(), None);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"exec\""));
        assert!(json.contains("\"command\":\"ls -la\""));
    }

    #[test]
    fn test_parse_exec_message() {
        let json = r#"{"type":"exec","command":"pwd","options":{"cwd":"/home"}}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::Exec { command, options } => {
                assert_eq!(command, "pwd");
                assert!(options.is_some());
            }
            _ => panic!("Expected Exec message"),
        }
    }

    // ============================================================
    // UI UPDATE MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_set_panel_message() {
        let msg = Message::set_panel("<div>Panel</div>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setPanel\""));
        assert!(json.contains("\"html\":\"<div>Panel</div>\""));
    }

    #[test]
    fn test_parse_set_panel_message() {
        let json = r#"{"type":"setPanel","html":"<p>Info</p>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetPanel { html } => {
                assert_eq!(html, "<p>Info</p>");
            }
            _ => panic!("Expected SetPanel message"),
        }
    }

    #[test]
    fn test_serialize_set_preview_message() {
        let msg = Message::set_preview("<div>Preview</div>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setPreview\""));
    }

    #[test]
    fn test_parse_set_preview_message() {
        let json = r#"{"type":"setPreview","html":"<img src=\"x\">"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetPreview { html } => {
                assert!(html.contains("img"));
            }
            _ => panic!("Expected SetPreview message"),
        }
    }

    #[test]
    fn test_serialize_set_prompt_message() {
        let msg = Message::set_prompt("<span>Prompt</span>".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setPrompt\""));
    }

    #[test]
    fn test_parse_set_prompt_message() {
        let json = r#"{"type":"setPrompt","html":"<b>Enter:</b>"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetPrompt { html } => {
                assert!(html.contains("Enter"));
            }
            _ => panic!("Expected SetPrompt message"),
        }
    }

    // ============================================================
    // MESSAGE ID TESTS FOR NEW TYPES
    // ============================================================

    #[test]
    fn test_new_message_ids() {
        // Messages with IDs
        assert_eq!(Message::editor("1".to_string()).id(), Some("1"));
        assert_eq!(
            Message::mini("2".to_string(), "".to_string(), vec![]).id(),
            Some("2")
        );
        assert_eq!(
            Message::micro("3".to_string(), "".to_string(), vec![]).id(),
            Some("3")
        );
        assert_eq!(
            Message::select("4".to_string(), "".to_string(), vec![], false).id(),
            Some("4")
        );
        assert_eq!(Message::fields("5".to_string(), vec![]).id(), Some("5"));
        assert_eq!(
            Message::form("6".to_string(), "".to_string()).id(),
            Some("6")
        );
        assert_eq!(Message::path("7".to_string(), None).id(), Some("7"));
        assert_eq!(Message::drop("8".to_string()).id(), Some("8"));
        assert_eq!(Message::hotkey("9".to_string()).id(), Some("9"));
        assert_eq!(
            Message::template("10".to_string(), "".to_string()).id(),
            Some("10")
        );
        assert_eq!(
            Message::env("11".to_string(), "".to_string(), false).id(),
            Some("11")
        );
        assert_eq!(Message::chat("12".to_string()).id(), Some("12"));
        assert_eq!(Message::term("13".to_string(), None).id(), Some("13"));
        assert_eq!(
            Message::widget("14".to_string(), "".to_string()).id(),
            Some("14")
        );
        assert_eq!(Message::webcam("15".to_string()).id(), Some("15"));
        assert_eq!(Message::mic("16".to_string()).id(), Some("16"));

        // Messages without IDs
        assert_eq!(Message::notify(None, None).id(), None);
        assert_eq!(Message::beep().id(), None);
        assert_eq!(Message::say("".to_string(), None).id(), None);
        assert_eq!(Message::set_status("".to_string(), None).id(), None);
        assert_eq!(Message::hud("".to_string(), None).id(), None);
        assert_eq!(Message::menu(None, None).id(), None);
        assert_eq!(Message::clipboard_read(None).id(), None);
        assert_eq!(Message::keyboard_type("".to_string()).id(), None);
        assert_eq!(Message::mouse(MouseAction::Click, None).id(), None);
        assert_eq!(Message::show().id(), None);
        assert_eq!(Message::hide().id(), None);
        assert_eq!(Message::exec("".to_string(), None).id(), None);
        assert_eq!(Message::set_panel("".to_string()).id(), None);
        assert_eq!(Message::set_preview("".to_string()).id(), None);
        assert_eq!(Message::set_prompt("".to_string()).id(), None);
    }

    // ============================================================
    // ENUM TYPE TESTS
    // ============================================================

    #[test]
    fn test_clipboard_action_serialization() {
        assert_eq!(
            serde_json::to_string(&ClipboardAction::Read).unwrap(),
            "\"read\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardAction::Write).unwrap(),
            "\"write\""
        );
    }

    #[test]
    fn test_clipboard_format_serialization() {
        assert_eq!(
            serde_json::to_string(&ClipboardFormat::Text).unwrap(),
            "\"text\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardFormat::Image).unwrap(),
            "\"image\""
        );
    }

    #[test]
    fn test_keyboard_action_serialization() {
        assert_eq!(
            serde_json::to_string(&KeyboardAction::Type).unwrap(),
            "\"type\""
        );
        assert_eq!(
            serde_json::to_string(&KeyboardAction::Tap).unwrap(),
            "\"tap\""
        );
    }

    #[test]
    fn test_mouse_action_serialization() {
        // camelCase applies to all variants
        assert_eq!(
            serde_json::to_string(&MouseAction::Move).unwrap(),
            "\"move\""
        );
        assert_eq!(
            serde_json::to_string(&MouseAction::Click).unwrap(),
            "\"click\""
        );
        assert_eq!(
            serde_json::to_string(&MouseAction::SetPosition).unwrap(),
            "\"setPosition\""
        );
    }

    // ============================================================
    // GRACEFUL PARSING TESTS
    // ============================================================

    #[test]
    fn test_parse_message_graceful_known_type() {
        let json = r#"{"type":"arg","id":"1","placeholder":"Pick","choices":[]}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::Arg { id, .. }) => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected ParseResult::Ok with Arg message"),
        }
    }

    #[test]
    fn test_parse_message_graceful_unknown_type() {
        let json = r#"{"type":"futureFeature","id":"1","data":"test"}"#;
        match parse_message_graceful(json) {
            ParseResult::UnknownType { message_type, raw } => {
                assert_eq!(message_type, "futureFeature");
                assert_eq!(raw, json);
            }
            _ => panic!("Expected ParseResult::UnknownType"),
        }
    }

    #[test]
    fn test_parse_message_graceful_invalid_json() {
        let json = "not valid json at all";
        match parse_message_graceful(json) {
            ParseResult::ParseError(_) => {}
            _ => panic!("Expected ParseResult::ParseError"),
        }
    }

    #[test]
    fn test_parse_message_graceful_missing_type_field() {
        let json = r#"{"id":"1","data":"test"}"#;
        match parse_message_graceful(json) {
            ParseResult::ParseError(_) => {}
            _ => panic!("Expected ParseResult::ParseError for missing type field"),
        }
    }

    #[test]
    fn test_jsonl_reader_skips_empty_lines() {
        use std::io::Cursor;

        let jsonl = "\n{\"type\":\"beep\"}\n\n{\"type\":\"show\"}\n";
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);

        // First message should be beep (skipping initial empty line)
        let msg1 = reader.next_message().unwrap();
        assert!(matches!(msg1, Some(Message::Beep {})));

        // Second message should be show (skipping intermediate empty lines)
        let msg2 = reader.next_message().unwrap();
        assert!(matches!(msg2, Some(Message::Show {})));

        // Should be EOF
        let msg3 = reader.next_message().unwrap();
        assert!(msg3.is_none());
    }

    #[test]
    fn test_jsonl_reader_graceful_skips_unknown() {
        use std::io::Cursor;

        let jsonl = r#"{"type":"unknownType","id":"1"}
{"type":"beep"}
{"type":"anotherUnknown","data":"test"}
{"type":"show"}
"#;
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);

        // Should skip unknownType and return beep
        let msg1 = reader.next_message_graceful().unwrap();
        assert!(matches!(msg1, Some(Message::Beep {})));

        // Should skip anotherUnknown and return show
        let msg2 = reader.next_message_graceful().unwrap();
        assert!(matches!(msg2, Some(Message::Show {})));

        // Should be EOF
        let msg3 = reader.next_message_graceful().unwrap();
        assert!(msg3.is_none());
    }

    // ============================================================
    // SELECTED TEXT OPERATION TESTS
    // ============================================================

    #[test]
    fn test_serialize_get_selected_text() {
        let msg = Message::get_selected_text("req-123".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getSelectedText\""));
        assert!(json.contains("\"requestId\":\"req-123\""));
    }

    #[test]
    fn test_parse_get_selected_text() {
        let json = r#"{"type":"getSelectedText","requestId":"req-456"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::GetSelectedText { request_id } => {
                assert_eq!(request_id, "req-456");
            }
            _ => panic!("Expected GetSelectedText message"),
        }
    }

    #[test]
    fn test_serialize_set_selected_text() {
        let msg = Message::set_selected_text_msg("Hello World".to_string(), "req-789".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setSelectedText\""));
        assert!(json.contains("\"text\":\"Hello World\""));
        assert!(json.contains("\"requestId\":\"req-789\""));
    }

    #[test]
    fn test_parse_set_selected_text() {
        let json = r#"{"type":"setSelectedText","text":"New text","requestId":"req-abc"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetSelectedText { text, request_id } => {
                assert_eq!(text, "New text");
                assert_eq!(request_id, "req-abc");
            }
            _ => panic!("Expected SetSelectedText message"),
        }
    }

    #[test]
    fn test_serialize_check_accessibility() {
        let msg = Message::check_accessibility("req-check".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"checkAccessibility\""));
        assert!(json.contains("\"requestId\":\"req-check\""));
    }

    #[test]
    fn test_parse_check_accessibility() {
        let json = r#"{"type":"checkAccessibility","requestId":"req-check-2"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::CheckAccessibility { request_id } => {
                assert_eq!(request_id, "req-check-2");
            }
            _ => panic!("Expected CheckAccessibility message"),
        }
    }

    #[test]
    fn test_serialize_request_accessibility() {
        let msg = Message::request_accessibility("req-request".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"requestAccessibility\""));
        assert!(json.contains("\"requestId\":\"req-request\""));
    }

    #[test]
    fn test_parse_request_accessibility() {
        let json = r#"{"type":"requestAccessibility","requestId":"req-request-2"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::RequestAccessibility { request_id } => {
                assert_eq!(request_id, "req-request-2");
            }
            _ => panic!("Expected RequestAccessibility message"),
        }
    }

    #[test]
    fn test_serialize_selected_text_response() {
        let msg =
            Message::selected_text_response("Selected content".to_string(), "req-111".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"selectedText\""));
        assert!(json.contains("\"text\":\"Selected content\""));
        assert!(json.contains("\"requestId\":\"req-111\""));
    }

    #[test]
    fn test_parse_selected_text_response() {
        let json = r#"{"type":"selectedText","text":"Some text","requestId":"req-222"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SelectedText { text, request_id } => {
                assert_eq!(text, "Some text");
                assert_eq!(request_id, "req-222");
            }
            _ => panic!("Expected SelectedText message"),
        }
    }

    #[test]
    fn test_serialize_text_set_success() {
        let msg = Message::text_set_success("req-333".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"textSet\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"requestId\":\"req-333\""));
        // error field should be omitted when None
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_serialize_text_set_error() {
        let msg = Message::text_set_error("Permission denied".to_string(), "req-444".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"textSet\""));
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Permission denied\""));
        assert!(json.contains("\"requestId\":\"req-444\""));
    }

    #[test]
    fn test_parse_text_set_success() {
        let json = r#"{"type":"textSet","success":true,"requestId":"req-555"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::TextSet {
                success,
                error,
                request_id,
            } => {
                assert!(success);
                assert_eq!(error, None);
                assert_eq!(request_id, "req-555");
            }
            _ => panic!("Expected TextSet message"),
        }
    }

    #[test]
    fn test_parse_text_set_error() {
        let json = r#"{"type":"textSet","success":false,"error":"Failed","requestId":"req-666"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::TextSet {
                success,
                error,
                request_id,
            } => {
                assert!(!success);
                assert_eq!(error, Some("Failed".to_string()));
                assert_eq!(request_id, "req-666");
            }
            _ => panic!("Expected TextSet message"),
        }
    }

    #[test]
    fn test_serialize_accessibility_status() {
        let msg = Message::accessibility_status(true, "req-777".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"accessibilityStatus\""));
        assert!(json.contains("\"granted\":true"));
        assert!(json.contains("\"requestId\":\"req-777\""));
    }

    #[test]
    fn test_parse_accessibility_status() {
        let json = r#"{"type":"accessibilityStatus","granted":false,"requestId":"req-888"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::AccessibilityStatus {
                granted,
                request_id,
            } => {
                assert!(!granted);
                assert_eq!(request_id, "req-888");
            }
            _ => panic!("Expected AccessibilityStatus message"),
        }
    }

    #[test]
    fn test_selected_text_message_ids() {
        // Messages with request_id
        assert_eq!(Message::get_selected_text("a".to_string()).id(), Some("a"));
        assert_eq!(
            Message::set_selected_text_msg("".to_string(), "b".to_string()).id(),
            Some("b")
        );
        assert_eq!(
            Message::check_accessibility("c".to_string()).id(),
            Some("c")
        );
        assert_eq!(
            Message::request_accessibility("d".to_string()).id(),
            Some("d")
        );
        assert_eq!(
            Message::selected_text_response("".to_string(), "e".to_string()).id(),
            Some("e")
        );
        assert_eq!(Message::text_set_success("f".to_string()).id(), Some("f"));
        assert_eq!(
            Message::text_set_error("".to_string(), "g".to_string()).id(),
            Some("g")
        );
        assert_eq!(
            Message::accessibility_status(true, "h".to_string()).id(),
            Some("h")
        );
    }

    // ============================================================
    // WINDOW BOUNDS TESTS
    // ============================================================

    #[test]
    fn test_serialize_get_window_bounds() {
        let msg = Message::get_window_bounds("req-wb-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getWindowBounds\""));
        assert!(json.contains("\"requestId\":\"req-wb-1\""));
    }

    #[test]
    fn test_parse_get_window_bounds() {
        let json = r#"{"type":"getWindowBounds","requestId":"req-wb-2"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::GetWindowBounds { request_id } => {
                assert_eq!(request_id, "req-wb-2");
            }
            _ => panic!("Expected GetWindowBounds message"),
        }
    }

    #[test]
    fn test_serialize_window_bounds() {
        let msg = Message::window_bounds(100.0, 200.0, 750.0, 400.0, "req-wb-3".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"windowBounds\""));
        assert!(json.contains("\"x\":100"));
        assert!(json.contains("\"y\":200"));
        assert!(json.contains("\"width\":750"));
        assert!(json.contains("\"height\":400"));
        assert!(json.contains("\"requestId\":\"req-wb-3\""));
    }

    #[test]
    fn test_parse_window_bounds() {
        let json = r#"{"type":"windowBounds","x":50.5,"y":100.5,"width":800.0,"height":600.0,"requestId":"req-wb-4"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::WindowBounds {
                x,
                y,
                width,
                height,
                request_id,
            } => {
                assert!((x - 50.5).abs() < 0.01);
                assert!((y - 100.5).abs() < 0.01);
                assert!((width - 800.0).abs() < 0.01);
                assert!((height - 600.0).abs() < 0.01);
                assert_eq!(request_id, "req-wb-4");
            }
            _ => panic!("Expected WindowBounds message"),
        }
    }

    #[test]
    fn test_window_bounds_message_ids() {
        assert_eq!(Message::get_window_bounds("a".to_string()).id(), Some("a"));
        assert_eq!(
            Message::window_bounds(0.0, 0.0, 0.0, 0.0, "b".to_string()).id(),
            Some("b")
        );
    }

    // ============================================================
    // CLIPBOARD HISTORY TESTS
    // ============================================================

    #[test]
    fn test_clipboard_entry_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ClipboardEntryType::Text).unwrap(),
            "\"text\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardEntryType::Image).unwrap(),
            "\"image\""
        );
    }

    #[test]
    fn test_clipboard_history_action_serialization() {
        assert_eq!(
            serde_json::to_string(&ClipboardHistoryAction::List).unwrap(),
            "\"list\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardHistoryAction::Pin).unwrap(),
            "\"pin\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardHistoryAction::Unpin).unwrap(),
            "\"unpin\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardHistoryAction::Remove).unwrap(),
            "\"remove\""
        );
        assert_eq!(
            serde_json::to_string(&ClipboardHistoryAction::Clear).unwrap(),
            "\"clear\""
        );
    }

    #[test]
    fn test_serialize_clipboard_history_list() {
        let msg = Message::clipboard_history_list("req-ch-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboardHistory\""));
        assert!(json.contains("\"requestId\":\"req-ch-1\""));
        assert!(json.contains("\"action\":\"list\""));
        assert!(!json.contains("\"entryId\"")); // Should be omitted when None
    }

    #[test]
    fn test_parse_clipboard_history_list() {
        let json = r#"{"type":"clipboardHistory","requestId":"req-ch-2","action":"list"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ClipboardHistory {
                request_id,
                action,
                entry_id,
            } => {
                assert_eq!(request_id, "req-ch-2");
                assert_eq!(action, ClipboardHistoryAction::List);
                assert_eq!(entry_id, None);
            }
            _ => panic!("Expected ClipboardHistory message"),
        }
    }

    #[test]
    fn test_serialize_clipboard_history_pin() {
        let msg = Message::clipboard_history_pin("req-ch-3".to_string(), "entry-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboardHistory\""));
        assert!(json.contains("\"action\":\"pin\""));
        assert!(json.contains("\"entryId\":\"entry-1\""));
    }

    #[test]
    fn test_parse_clipboard_history_pin() {
        let json = r#"{"type":"clipboardHistory","requestId":"req-ch-4","action":"pin","entryId":"entry-2"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ClipboardHistory {
                request_id,
                action,
                entry_id,
            } => {
                assert_eq!(request_id, "req-ch-4");
                assert_eq!(action, ClipboardHistoryAction::Pin);
                assert_eq!(entry_id, Some("entry-2".to_string()));
            }
            _ => panic!("Expected ClipboardHistory message"),
        }
    }

    #[test]
    fn test_serialize_clipboard_history_entry() {
        let msg = Message::clipboard_history_entry(
            "req-che-1".to_string(),
            "entry-1".to_string(),
            "Hello World".to_string(),
            ClipboardEntryType::Text,
            "2024-01-15T10:30:00Z".to_string(),
            true,
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboardHistoryEntry\""));
        assert!(json.contains("\"requestId\":\"req-che-1\""));
        assert!(json.contains("\"entryId\":\"entry-1\""));
        assert!(json.contains("\"content\":\"Hello World\""));
        assert!(json.contains("\"contentType\":\"text\""));
        assert!(json.contains("\"pinned\":true"));
    }

    #[test]
    fn test_parse_clipboard_history_entry() {
        let json = r#"{"type":"clipboardHistoryEntry","requestId":"req-che-2","entryId":"entry-2","content":"Test","contentType":"image","timestamp":"2024-01-15","pinned":false}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ClipboardHistoryEntry {
                request_id,
                entry_id,
                content,
                content_type,
                timestamp,
                pinned,
            } => {
                assert_eq!(request_id, "req-che-2");
                assert_eq!(entry_id, "entry-2");
                assert_eq!(content, "Test");
                assert_eq!(content_type, ClipboardEntryType::Image);
                assert_eq!(timestamp, "2024-01-15");
                assert!(!pinned);
            }
            _ => panic!("Expected ClipboardHistoryEntry message"),
        }
    }

    #[test]
    fn test_serialize_clipboard_history_list_response() {
        let entries = vec![ClipboardHistoryEntryData {
            entry_id: "e1".to_string(),
            content: "Hello".to_string(),
            content_type: ClipboardEntryType::Text,
            timestamp: "2024-01-15".to_string(),
            pinned: false,
        }];
        let msg = Message::clipboard_history_list_response("req-chl-1".to_string(), entries);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboardHistoryList\""));
        assert!(json.contains("\"entries\""));
    }

    #[test]
    fn test_serialize_clipboard_history_result() {
        let msg = Message::clipboard_history_success("req-chr-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"clipboardHistoryResult\""));
        assert!(json.contains("\"success\":true"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_clipboard_history_message_ids() {
        assert_eq!(
            Message::clipboard_history_list("a".to_string()).id(),
            Some("a")
        );
        assert_eq!(
            Message::clipboard_history_pin("b".to_string(), "x".to_string()).id(),
            Some("b")
        );
        assert_eq!(
            Message::clipboard_history_entry(
                "c".to_string(),
                "".to_string(),
                "".to_string(),
                ClipboardEntryType::Text,
                "".to_string(),
                false
            )
            .id(),
            Some("c")
        );
        assert_eq!(
            Message::clipboard_history_list_response("d".to_string(), vec![]).id(),
            Some("d")
        );
        assert_eq!(
            Message::clipboard_history_success("e".to_string()).id(),
            Some("e")
        );
    }

    // ============================================================
    // WINDOW MANAGEMENT TESTS
    // ============================================================

    #[test]
    fn test_window_action_type_serialization() {
        assert_eq!(
            serde_json::to_string(&WindowActionType::Focus).unwrap(),
            "\"focus\""
        );
        assert_eq!(
            serde_json::to_string(&WindowActionType::Close).unwrap(),
            "\"close\""
        );
        assert_eq!(
            serde_json::to_string(&WindowActionType::Minimize).unwrap(),
            "\"minimize\""
        );
        assert_eq!(
            serde_json::to_string(&WindowActionType::Maximize).unwrap(),
            "\"maximize\""
        );
        assert_eq!(
            serde_json::to_string(&WindowActionType::Resize).unwrap(),
            "\"resize\""
        );
        assert_eq!(
            serde_json::to_string(&WindowActionType::Move).unwrap(),
            "\"move\""
        );
    }

    #[test]
    fn test_serialize_target_window_bounds() {
        let bounds = TargetWindowBounds {
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };
        let json = serde_json::to_string(&bounds).unwrap();
        assert!(json.contains("\"x\":100"));
        assert!(json.contains("\"y\":200"));
        assert!(json.contains("\"width\":800"));
        assert!(json.contains("\"height\":600"));
    }

    #[test]
    fn test_parse_target_window_bounds() {
        let json = r#"{"x":-50,"y":100,"width":1024,"height":768}"#;
        let bounds: TargetWindowBounds = serde_json::from_str(json).unwrap();
        assert_eq!(bounds.x, -50);
        assert_eq!(bounds.y, 100);
        assert_eq!(bounds.width, 1024);
        assert_eq!(bounds.height, 768);
    }

    #[test]
    fn test_serialize_window_list() {
        let msg = Message::window_list("req-wl-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"windowList\""));
        assert!(json.contains("\"requestId\":\"req-wl-1\""));
    }

    #[test]
    fn test_parse_window_list() {
        let json = r#"{"type":"windowList","requestId":"req-wl-2"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::WindowList { request_id } => {
                assert_eq!(request_id, "req-wl-2");
            }
            _ => panic!("Expected WindowList message"),
        }
    }

    #[test]
    fn test_serialize_window_action() {
        let bounds = TargetWindowBounds {
            x: 0,
            y: 0,
            width: 500,
            height: 400,
        };
        let msg = Message::window_action(
            "req-wa-1".to_string(),
            WindowActionType::Resize,
            Some(12345),
            Some(bounds),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"windowAction\""));
        assert!(json.contains("\"action\":\"resize\""));
        assert!(json.contains("\"windowId\":12345"));
        assert!(json.contains("\"width\":500"));
    }

    #[test]
    fn test_parse_window_action() {
        let json =
            r#"{"type":"windowAction","requestId":"req-wa-2","action":"focus","windowId":999}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::WindowAction {
                request_id,
                action,
                window_id,
                bounds,
            } => {
                assert_eq!(request_id, "req-wa-2");
                assert_eq!(action, WindowActionType::Focus);
                assert_eq!(window_id, Some(999));
                assert_eq!(bounds, None);
            }
            _ => panic!("Expected WindowAction message"),
        }
    }

    #[test]
    fn test_serialize_window_list_result() {
        let windows = vec![SystemWindowInfo {
            window_id: 1,
            title: "Terminal".to_string(),
            app_name: "Terminal.app".to_string(),
            bounds: Some(TargetWindowBounds {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            }),
            is_minimized: Some(false),
            is_active: Some(true),
        }];
        let msg = Message::window_list_result("req-wlr-1".to_string(), windows);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"windowListResult\""));
        assert!(json.contains("\"windows\""));
        assert!(json.contains("\"windowId\":1"));
        assert!(json.contains("\"appName\":\"Terminal.app\""));
    }

    #[test]
    fn test_serialize_window_action_result() {
        let msg = Message::window_action_success("req-war-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"windowActionResult\""));
        assert!(json.contains("\"success\":true"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_window_management_message_ids() {
        assert_eq!(Message::window_list("a".to_string()).id(), Some("a"));
        assert_eq!(
            Message::window_action("b".to_string(), WindowActionType::Focus, None, None).id(),
            Some("b")
        );
        assert_eq!(
            Message::window_list_result("c".to_string(), vec![]).id(),
            Some("c")
        );
        assert_eq!(
            Message::window_action_success("d".to_string()).id(),
            Some("d")
        );
    }

    // ============================================================
    // FILE SEARCH TESTS
    // ============================================================

    #[test]
    fn test_serialize_file_search() {
        let msg = Message::file_search(
            "req-fs-1".to_string(),
            "*.rs".to_string(),
            Some("/home/user".to_string()),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"fileSearch\""));
        assert!(json.contains("\"requestId\":\"req-fs-1\""));
        assert!(json.contains("\"query\":\"*.rs\""));
        assert!(json.contains("\"onlyin\":\"/home/user\""));
    }

    #[test]
    fn test_parse_file_search() {
        let json = r#"{"type":"fileSearch","requestId":"req-fs-2","query":"test","onlyin":"/tmp"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::FileSearch {
                request_id,
                query,
                only_in,
            } => {
                assert_eq!(request_id, "req-fs-2");
                assert_eq!(query, "test");
                assert_eq!(only_in, Some("/tmp".to_string()));
            }
            _ => panic!("Expected FileSearch message"),
        }
    }

    #[test]
    fn test_parse_file_search_without_onlyin() {
        let json = r#"{"type":"fileSearch","requestId":"req-fs-3","query":"main.rs"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::FileSearch {
                request_id,
                query,
                only_in,
            } => {
                assert_eq!(request_id, "req-fs-3");
                assert_eq!(query, "main.rs");
                assert_eq!(only_in, None);
            }
            _ => panic!("Expected FileSearch message"),
        }
    }

    #[test]
    fn test_serialize_file_search_result() {
        let files = vec![FileSearchResultEntry {
            path: "/home/user/test.rs".to_string(),
            name: "test.rs".to_string(),
            is_directory: false,
            size: Some(1024),
            modified_at: Some("2024-01-15T10:30:00Z".to_string()),
        }];
        let msg = Message::file_search_result("req-fsr-1".to_string(), files);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"fileSearchResult\""));
        assert!(json.contains("\"files\""));
        assert!(json.contains("\"path\":\"/home/user/test.rs\""));
        assert!(json.contains("\"isDirectory\":false"));
    }

    #[test]
    fn test_parse_file_search_result() {
        let json = r#"{"type":"fileSearchResult","requestId":"req-fsr-2","files":[{"path":"/tmp/a","name":"a","isDirectory":true}]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::FileSearchResult { request_id, files } => {
                assert_eq!(request_id, "req-fsr-2");
                assert_eq!(files.len(), 1);
                assert_eq!(files[0].path, "/tmp/a");
                assert!(files[0].is_directory);
            }
            _ => panic!("Expected FileSearchResult message"),
        }
    }

    #[test]
    fn test_file_search_message_ids() {
        assert_eq!(
            Message::file_search("a".to_string(), "".to_string(), None).id(),
            Some("a")
        );
        assert_eq!(
            Message::file_search_result("b".to_string(), vec![]).id(),
            Some("b")
        );
    }

    // ============================================================
    // SCRIPT ERROR TESTS
    // ============================================================

    #[test]
    fn test_script_error_data_creation() {
        let error = ScriptErrorData::new(
            "Failed to import module".to_string(),
            "/home/user/script.ts".to_string(),
        );
        assert_eq!(error.error_message, "Failed to import module");
        assert_eq!(error.script_path, "/home/user/script.ts");
        assert_eq!(error.stderr_output, None);
        assert_eq!(error.exit_code, None);
        assert_eq!(error.stack_trace, None);
        assert!(error.suggestions.is_empty());
        assert_eq!(error.timestamp, None);
    }

    #[test]
    fn test_script_error_data_builder() {
        let error = ScriptErrorData::new("Error".to_string(), "/path/script.ts".to_string())
            .with_stderr("Error: module not found".to_string())
            .with_exit_code(1)
            .with_stack_trace("at line 10\nat line 5".to_string())
            .with_suggestions(vec!["Install the module".to_string()])
            .add_suggestion("Check your imports".to_string())
            .with_timestamp("2024-01-15T10:30:00Z".to_string());

        assert_eq!(
            error.stderr_output,
            Some("Error: module not found".to_string())
        );
        assert_eq!(error.exit_code, Some(1));
        assert_eq!(error.stack_trace, Some("at line 10\nat line 5".to_string()));
        assert_eq!(error.suggestions.len(), 2);
        assert_eq!(error.suggestions[0], "Install the module");
        assert_eq!(error.suggestions[1], "Check your imports");
        assert_eq!(error.timestamp, Some("2024-01-15T10:30:00Z".to_string()));
    }

    #[test]
    fn test_serialize_set_error_message() {
        let error = ScriptErrorData::new(
            "Script crashed".to_string(),
            "/home/user/test.ts".to_string(),
        )
        .with_exit_code(1);

        let msg = Message::set_error(error);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setError\""));
        assert!(json.contains("\"errorMessage\":\"Script crashed\""));
        assert!(json.contains("\"scriptPath\":\"/home/user/test.ts\""));
        assert!(json.contains("\"exitCode\":1"));
    }

    #[test]
    fn test_serialize_set_error_full() {
        let error =
            ScriptErrorData::new("Import failed".to_string(), "/scripts/main.ts".to_string())
                .with_stderr("Error: Cannot find module 'xyz'".to_string())
                .with_exit_code(1)
                .with_stack_trace("at import (/scripts/main.ts:1:1)".to_string())
                .with_suggestions(vec!["Run: npm install xyz".to_string()])
                .with_timestamp("2024-01-15T10:30:00Z".to_string());

        let msg = Message::set_error(error);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"setError\""));
        assert!(json.contains("\"errorMessage\":\"Import failed\""));
        assert!(json.contains("\"stderrOutput\":\"Error: Cannot find module 'xyz'\""));
        assert!(json.contains("\"exitCode\":1"));
        assert!(json.contains("\"stackTrace\":\"at import (/scripts/main.ts:1:1)\""));
        assert!(json.contains("\"scriptPath\":\"/scripts/main.ts\""));
        assert!(json.contains("\"suggestions\":[\"Run: npm install xyz\"]"));
        assert!(json.contains("\"timestamp\":\"2024-01-15T10:30:00Z\""));
    }

    #[test]
    fn test_serialize_set_error_omits_none_fields() {
        let error = ScriptErrorData::new("Simple error".to_string(), "/path/script.ts".to_string());
        let msg = Message::set_error(error);
        let json = serialize_message(&msg).unwrap();

        // Optional fields should be omitted when None
        assert!(!json.contains("\"stderrOutput\""));
        assert!(!json.contains("\"exitCode\""));
        assert!(!json.contains("\"stackTrace\""));
        assert!(!json.contains("\"timestamp\""));
        // Empty suggestions should also be omitted
        assert!(!json.contains("\"suggestions\""));
    }

    #[test]
    fn test_parse_set_error_minimal() {
        let json = r#"{"type":"setError","errorMessage":"Failed","scriptPath":"/test.ts"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetError {
                error_message,
                script_path,
                stderr_output,
                exit_code,
                ..
            } => {
                assert_eq!(error_message, "Failed");
                assert_eq!(script_path, "/test.ts");
                assert_eq!(stderr_output, None);
                assert_eq!(exit_code, None);
            }
            _ => panic!("Expected SetError message"),
        }
    }

    #[test]
    fn test_parse_set_error_full() {
        let json = r#"{
            "type": "setError",
            "errorMessage": "Module not found",
            "stderrOutput": "Error: xyz not found",
            "exitCode": 1,
            "stackTrace": "at line 5",
            "scriptPath": "/home/user/script.ts",
            "suggestions": ["Install xyz", "Check path"],
            "timestamp": "2024-01-15T10:30:00Z"
        }"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SetError {
                error_message,
                stderr_output,
                exit_code,
                stack_trace,
                script_path,
                suggestions,
                timestamp,
            } => {
                assert_eq!(error_message, "Module not found");
                assert_eq!(stderr_output, Some("Error: xyz not found".to_string()));
                assert_eq!(exit_code, Some(1));
                assert_eq!(stack_trace, Some("at line 5".to_string()));
                assert_eq!(script_path, "/home/user/script.ts");
                assert_eq!(suggestions, vec!["Install xyz", "Check path"]);
                assert_eq!(timestamp, Some("2024-01-15T10:30:00Z".to_string()));
            }
            _ => panic!("Expected SetError message"),
        }
    }

    #[test]
    fn test_script_error_constructor() {
        let msg = Message::script_error("Error occurred".to_string(), "/test.ts".to_string());
        match msg {
            Message::SetError {
                error_message,
                script_path,
                ..
            } => {
                assert_eq!(error_message, "Error occurred");
                assert_eq!(script_path, "/test.ts");
            }
            _ => panic!("Expected SetError message"),
        }
    }

    #[test]
    fn test_set_error_message_id() {
        let msg = Message::script_error("Error".to_string(), "/test.ts".to_string());
        // SetError messages don't have an ID
        assert_eq!(msg.id(), None);
    }

    #[test]
    fn test_script_error_data_serialization() {
        // Test the struct serialization directly
        let error = ScriptErrorData::new("Test error".to_string(), "/path.ts".to_string())
            .with_exit_code(42);

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"errorMessage\":\"Test error\""));
        assert!(json.contains("\"scriptPath\":\"/path.ts\""));
        assert!(json.contains("\"exitCode\":42"));
    }

    #[test]
    fn test_script_error_data_deserialization() {
        let json =
            r#"{"errorMessage":"Test","scriptPath":"/p.ts","exitCode":1,"suggestions":["a","b"]}"#;
        let error: ScriptErrorData = serde_json::from_str(json).unwrap();
        assert_eq!(error.error_message, "Test");
        assert_eq!(error.script_path, "/p.ts");
        assert_eq!(error.exit_code, Some(1));
        assert_eq!(error.suggestions, vec!["a", "b"]);
    }

    #[test]
    fn test_script_error_full_constructor() {
        let msg = Message::script_error_full(
            "Error".to_string(),
            "/script.ts".to_string(),
            Some("stderr output".to_string()),
            Some(1),
            Some("stack trace".to_string()),
            vec!["suggestion 1".to_string()],
            Some("2024-01-15T10:30:00Z".to_string()),
        );
        match msg {
            Message::SetError {
                error_message,
                stderr_output,
                exit_code,
                stack_trace,
                script_path,
                suggestions,
                timestamp,
            } => {
                assert_eq!(error_message, "Error");
                assert_eq!(stderr_output, Some("stderr output".to_string()));
                assert_eq!(exit_code, Some(1));
                assert_eq!(stack_trace, Some("stack trace".to_string()));
                assert_eq!(script_path, "/script.ts");
                assert_eq!(suggestions, vec!["suggestion 1"]);
                assert_eq!(timestamp, Some("2024-01-15T10:30:00Z".to_string()));
            }
            _ => panic!("Expected SetError message"),
        }
    }

    // ============================================================
    // SEMANTIC ID TESTS
    // ============================================================

    #[test]
    fn test_value_to_slug_basic() {
        assert_eq!(value_to_slug("apple"), "apple");
        assert_eq!(value_to_slug("Apple"), "apple");
        assert_eq!(value_to_slug("APPLE"), "apple");
    }

    #[test]
    fn test_value_to_slug_spaces() {
        assert_eq!(value_to_slug("red apple"), "red-apple");
        assert_eq!(value_to_slug("red  apple"), "red-apple"); // multiple spaces
        assert_eq!(value_to_slug("  apple  "), "apple"); // leading/trailing spaces become hyphens then trimmed
    }

    #[test]
    fn test_value_to_slug_special_chars() {
        assert_eq!(value_to_slug("apple_pie"), "apple-pie");
        assert_eq!(value_to_slug("apple@pie!"), "apple-pie");
        assert_eq!(value_to_slug("hello-world"), "hello-world");
    }

    #[test]
    fn test_value_to_slug_truncation() {
        let long_value = "this is a very long value that exceeds twenty characters";
        let slug = value_to_slug(long_value);
        assert!(slug.len() <= 20);
        assert_eq!(slug, "this-is-a-very-long");
    }

    #[test]
    fn test_value_to_slug_empty() {
        assert_eq!(value_to_slug(""), "item");
        assert_eq!(value_to_slug("   "), "item");
        assert_eq!(value_to_slug("@#$%"), "item"); // all special chars
    }

    #[test]
    fn test_generate_semantic_id() {
        assert_eq!(generate_semantic_id("choice", 0, "apple"), "choice:0:apple");
        assert_eq!(
            generate_semantic_id("choice", 5, "Red Apple"),
            "choice:5:red-apple"
        );
        assert_eq!(
            generate_semantic_id("button", 1, "Submit Form"),
            "button:1:submit-form"
        );
    }

    #[test]
    fn test_generate_semantic_id_named() {
        assert_eq!(
            generate_semantic_id_named("input", "filter"),
            "input:filter"
        );
        assert_eq!(
            generate_semantic_id_named("panel", "preview"),
            "panel:preview"
        );
        assert_eq!(
            generate_semantic_id_named("window", "Main Window"),
            "window:main-window"
        );
    }

    #[test]
    fn test_choice_with_semantic_id() {
        let choice = Choice::new("Apple".to_string(), "apple".to_string()).with_semantic_id(0);
        assert_eq!(choice.semantic_id, Some("choice:0:apple".to_string()));
    }

    #[test]
    fn test_choice_generate_id() {
        let choice = Choice::new("Red Apple".to_string(), "red_apple".to_string());
        assert_eq!(choice.generate_id(3), "choice:3:red-apple");
    }

    #[test]
    fn test_choice_set_semantic_id() {
        let mut choice = Choice::new("Test".to_string(), "test".to_string());
        choice.set_semantic_id("custom:id:value".to_string());
        assert_eq!(choice.semantic_id, Some("custom:id:value".to_string()));
    }

    #[test]
    fn test_choice_serialization_with_semantic_id() {
        let choice = Choice::new("Apple".to_string(), "apple".to_string()).with_semantic_id(0);
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("\"semanticId\":\"choice:0:apple\""));
    }

    #[test]
    fn test_choice_serialization_without_semantic_id() {
        let choice = Choice::new("Apple".to_string(), "apple".to_string());
        let json = serde_json::to_string(&choice).unwrap();
        assert!(!json.contains("semanticId")); // Should be skipped when None
    }

    #[test]
    fn test_choice_deserialization_with_semantic_id() {
        let json = r#"{"name":"Apple","value":"apple","semanticId":"choice:0:apple"}"#;
        let choice: Choice = serde_json::from_str(json).unwrap();
        assert_eq!(choice.name, "Apple");
        assert_eq!(choice.value, "apple");
        assert_eq!(choice.semantic_id, Some("choice:0:apple".to_string()));
    }

    #[test]
    fn test_choice_deserialization_without_semantic_id() {
        let json = r#"{"name":"Apple","value":"apple"}"#;
        let choice: Choice = serde_json::from_str(json).unwrap();
        assert_eq!(choice.name, "Apple");
        assert_eq!(choice.value, "apple");
        assert_eq!(choice.semantic_id, None);
    }

    // ============================================================
    // STATE QUERY TESTS
    // ============================================================

    #[test]
    fn test_serialize_get_state() {
        let msg = Message::get_state("req-001".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getState\""));
        assert!(json.contains("\"requestId\":\"req-001\""));
    }

    #[test]
    fn test_parse_get_state() {
        let json = r#"{"type":"getState","requestId":"req-002"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::GetState { request_id } => {
                assert_eq!(request_id, "req-002");
            }
            _ => panic!("Expected GetState message"),
        }
    }

    #[test]
    fn test_serialize_state_result() {
        let msg = Message::state_result(
            "req-001".to_string(),
            "arg".to_string(),
            Some("prompt-123".to_string()),
            Some("Pick a fruit".to_string()),
            "app".to_string(),
            15,
            5,
            2,
            Some("apple".to_string()),
            true,
            true,
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"stateResult\""));
        assert!(json.contains("\"requestId\":\"req-001\""));
        assert!(json.contains("\"promptType\":\"arg\""));
        assert!(json.contains("\"promptId\":\"prompt-123\""));
        assert!(json.contains("\"inputValue\":\"app\""));
        assert!(json.contains("\"choiceCount\":15"));
        assert!(json.contains("\"visibleChoiceCount\":5"));
        assert!(json.contains("\"selectedIndex\":2"));
        assert!(json.contains("\"selectedValue\":\"apple\""));
        assert!(json.contains("\"isFocused\":true"));
        assert!(json.contains("\"windowVisible\":true"));
    }

    #[test]
    fn test_parse_state_result() {
        let json = r#"{"type":"stateResult","requestId":"req-003","promptType":"div","inputValue":"","choiceCount":0,"visibleChoiceCount":0,"selectedIndex":-1,"isFocused":true,"windowVisible":true}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::StateResult {
                request_id,
                prompt_type,
                prompt_id,
                input_value,
                choice_count,
                visible_choice_count,
                selected_index,
                selected_value,
                is_focused,
                window_visible,
                ..
            } => {
                assert_eq!(request_id, "req-003");
                assert_eq!(prompt_type, "div");
                assert_eq!(prompt_id, None);
                assert_eq!(input_value, "");
                assert_eq!(choice_count, 0);
                assert_eq!(visible_choice_count, 0);
                assert_eq!(selected_index, -1);
                assert_eq!(selected_value, None);
                assert!(is_focused);
                assert!(window_visible);
            }
            _ => panic!("Expected StateResult message"),
        }
    }

    #[test]
    fn test_get_state_id() {
        let msg = Message::get_state("req-id".to_string());
        assert_eq!(msg.id(), Some("req-id"));
    }

    #[test]
    fn test_state_result_id() {
        let msg = Message::state_result(
            "req-id".to_string(),
            "arg".to_string(),
            None,
            None,
            "".to_string(),
            0,
            0,
            -1,
            None,
            false,
            false,
        );
        assert_eq!(msg.id(), Some("req-id"));
    }

    // ============================================================
    // ELEMENT TYPE AND ELEMENT INFO TESTS
    // ============================================================

    #[test]
    fn test_element_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ElementType::Choice).unwrap(),
            "\"choice\""
        );
        assert_eq!(
            serde_json::to_string(&ElementType::Input).unwrap(),
            "\"input\""
        );
        assert_eq!(
            serde_json::to_string(&ElementType::Button).unwrap(),
            "\"button\""
        );
        assert_eq!(
            serde_json::to_string(&ElementType::Panel).unwrap(),
            "\"panel\""
        );
        assert_eq!(
            serde_json::to_string(&ElementType::List).unwrap(),
            "\"list\""
        );
    }

    #[test]
    fn test_element_info_choice() {
        let elem = ElementInfo::choice(0, "Apple", "apple", true);
        assert_eq!(elem.semantic_id, "choice:0:apple");
        assert_eq!(elem.element_type, ElementType::Choice);
        assert_eq!(elem.text, Some("Apple".to_string()));
        assert_eq!(elem.value, Some("apple".to_string()));
        assert_eq!(elem.selected, Some(true));
        assert_eq!(elem.index, Some(0));
    }

    #[test]
    fn test_element_info_input() {
        let elem = ElementInfo::input("filter", Some("test"), true);
        assert_eq!(elem.semantic_id, "input:filter");
        assert_eq!(elem.element_type, ElementType::Input);
        assert_eq!(elem.value, Some("test".to_string()));
        assert_eq!(elem.focused, Some(true));
    }

    #[test]
    fn test_element_info_button() {
        let elem = ElementInfo::button(1, "Submit");
        assert_eq!(elem.semantic_id, "button:1:submit");
        assert_eq!(elem.element_type, ElementType::Button);
        assert_eq!(elem.text, Some("Submit".to_string()));
    }

    #[test]
    fn test_element_info_panel() {
        let elem = ElementInfo::panel("preview");
        assert_eq!(elem.semantic_id, "panel:preview");
        assert_eq!(elem.element_type, ElementType::Panel);
    }

    #[test]
    fn test_element_info_list() {
        let elem = ElementInfo::list("choices", 15);
        assert_eq!(elem.semantic_id, "list:choices");
        assert_eq!(elem.element_type, ElementType::List);
        assert_eq!(elem.text, Some("15 items".to_string()));
    }

    #[test]
    fn test_element_info_serialization() {
        let elem = ElementInfo::choice(0, "Apple", "apple", true);
        let json = serde_json::to_string(&elem).unwrap();
        assert!(json.contains("\"semanticId\":\"choice:0:apple\""));
        assert!(json.contains("\"type\":\"choice\""));
        assert!(json.contains("\"text\":\"Apple\""));
        assert!(json.contains("\"value\":\"apple\""));
        assert!(json.contains("\"selected\":true"));
        assert!(json.contains("\"index\":0"));
        // Optional fields should be omitted when None
        assert!(!json.contains("\"focused\""));
    }

    #[test]
    fn test_element_info_deserialization() {
        let json = r#"{"semanticId":"choice:0:apple","type":"choice","text":"Apple","value":"apple","selected":true,"index":0}"#;
        let elem: ElementInfo = serde_json::from_str(json).unwrap();
        assert_eq!(elem.semantic_id, "choice:0:apple");
        assert_eq!(elem.element_type, ElementType::Choice);
        assert_eq!(elem.text, Some("Apple".to_string()));
        assert_eq!(elem.value, Some("apple".to_string()));
        assert_eq!(elem.selected, Some(true));
        assert_eq!(elem.index, Some(0));
    }

    // ============================================================
    // GET ELEMENTS MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_get_elements() {
        let msg = Message::get_elements("req-elem-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getElements\""));
        assert!(json.contains("\"requestId\":\"req-elem-1\""));
        // Optional limit should be omitted when None
        assert!(!json.contains("\"limit\""));
    }

    #[test]
    fn test_serialize_get_elements_with_limit() {
        let msg = Message::get_elements_with_limit("req-elem-2".to_string(), 25);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getElements\""));
        assert!(json.contains("\"limit\":25"));
    }

    #[test]
    fn test_parse_get_elements() {
        let json = r#"{"type":"getElements","requestId":"req-elem-3"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::GetElements { request_id, limit } => {
                assert_eq!(request_id, "req-elem-3");
                assert_eq!(limit, None);
            }
            _ => panic!("Expected GetElements message"),
        }
    }

    #[test]
    fn test_parse_get_elements_with_limit() {
        let json = r#"{"type":"getElements","requestId":"req-elem-4","limit":50}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::GetElements { request_id, limit } => {
                assert_eq!(request_id, "req-elem-4");
                assert_eq!(limit, Some(50));
            }
            _ => panic!("Expected GetElements message"),
        }
    }

    #[test]
    fn test_serialize_elements_result() {
        let elements = vec![
            ElementInfo::choice(0, "Apple", "apple", true),
            ElementInfo::choice(1, "Banana", "banana", false),
        ];
        let msg = Message::elements_result("req-elem-5".to_string(), elements, 2);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"elementsResult\""));
        assert!(json.contains("\"requestId\":\"req-elem-5\""));
        assert!(json.contains("\"totalCount\":2"));
        assert!(json.contains("\"elements\""));
        assert!(json.contains("\"semanticId\":\"choice:0:apple\""));
    }

    #[test]
    fn test_parse_elements_result() {
        let json = r#"{"type":"elementsResult","requestId":"req-elem-6","elements":[{"semanticId":"choice:0:apple","type":"choice","text":"Apple","value":"apple","selected":true,"index":0}],"totalCount":15}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ElementsResult {
                request_id,
                elements,
                total_count,
            } => {
                assert_eq!(request_id, "req-elem-6");
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].semantic_id, "choice:0:apple");
                assert_eq!(total_count, 15);
            }
            _ => panic!("Expected ElementsResult message"),
        }
    }

    #[test]
    fn test_get_elements_message_ids() {
        assert_eq!(Message::get_elements("a".to_string()).id(), Some("a"));
        assert_eq!(
            Message::get_elements_with_limit("b".to_string(), 10).id(),
            Some("b")
        );
        assert_eq!(
            Message::elements_result("c".to_string(), vec![], 0).id(),
            Some("c")
        );
    }

    #[test]
    fn test_elements_result_empty() {
        let msg = Message::elements_result("req-empty".to_string(), vec![], 0);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"elements\":[]"));
        assert!(json.contains("\"totalCount\":0"));
    }

    // ============================================================
    // SCRIPTLET DATA TESTS
    // ============================================================

    #[test]
    fn test_scriptlet_metadata_data_default() {
        let metadata = ScriptletMetadataData::default();
        assert_eq!(metadata.trigger, None);
        assert_eq!(metadata.shortcut, None);
        assert_eq!(metadata.schedule, None);
        assert_eq!(metadata.background, None);
        assert_eq!(metadata.watch, None);
        assert_eq!(metadata.system, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.expand, None);
    }

    #[test]
    fn test_scriptlet_metadata_data_serialization() {
        let metadata = ScriptletMetadataData {
            shortcut: Some("cmd k".to_string()),
            description: Some("Test script".to_string()),
            background: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"shortcut\":\"cmd k\""));
        assert!(json.contains("\"description\":\"Test script\""));
        assert!(json.contains("\"background\":true"));
        // None fields should be omitted
        assert!(!json.contains("\"trigger\""));
        assert!(!json.contains("\"schedule\""));
    }

    #[test]
    fn test_scriptlet_metadata_data_deserialization() {
        let json = r#"{"shortcut":"cmd shift k","expand":"hello,,"}"#;
        let metadata: ScriptletMetadataData = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.shortcut, Some("cmd shift k".to_string()));
        assert_eq!(metadata.expand, Some("hello,,".to_string()));
        assert_eq!(metadata.trigger, None);
    }

    #[test]
    fn test_scriptlet_data_creation() {
        let scriptlet = ScriptletData::new(
            "My Script".to_string(),
            "my-script".to_string(),
            "bash".to_string(),
            "echo hello".to_string(),
        );
        assert_eq!(scriptlet.name, "My Script");
        assert_eq!(scriptlet.command, "my-script");
        assert_eq!(scriptlet.tool, "bash");
        assert_eq!(scriptlet.content, "echo hello");
        assert!(scriptlet.inputs.is_empty());
        assert!(scriptlet.is_scriptlet);
        assert_eq!(scriptlet.group, None);
        assert_eq!(scriptlet.kenv, None);
    }

    #[test]
    fn test_scriptlet_data_builder() {
        let metadata = ScriptletMetadataData {
            shortcut: Some("cmd g".to_string()),
            ..Default::default()
        };
        let scriptlet = ScriptletData::new(
            "Greeter".to_string(),
            "greeter".to_string(),
            "ts".to_string(),
            "console.log('Hello {{name}}')".to_string(),
        )
        .with_inputs(vec!["name".to_string()])
        .with_group("Utilities".to_string())
        .with_kenv("main".to_string())
        .with_source_path("/path/to/scriptlets.md".to_string())
        .with_metadata(metadata);

        assert_eq!(scriptlet.inputs, vec!["name"]);
        assert_eq!(scriptlet.group, Some("Utilities".to_string()));
        assert_eq!(scriptlet.kenv, Some("main".to_string()));
        assert_eq!(
            scriptlet.source_path,
            Some("/path/to/scriptlets.md".to_string())
        );
        assert!(scriptlet.metadata.is_some());
        assert_eq!(
            scriptlet.metadata.unwrap().shortcut,
            Some("cmd g".to_string())
        );
    }

    #[test]
    fn test_scriptlet_data_serialization() {
        let scriptlet = ScriptletData::new(
            "Test".to_string(),
            "test".to_string(),
            "bash".to_string(),
            "echo $1".to_string(),
        )
        .with_inputs(vec!["arg".to_string()]);

        let json = serde_json::to_string(&scriptlet).unwrap();
        assert!(json.contains("\"name\":\"Test\""));
        assert!(json.contains("\"command\":\"test\""));
        assert!(json.contains("\"tool\":\"bash\""));
        assert!(json.contains("\"content\":\"echo $1\""));
        assert!(json.contains("\"inputs\":[\"arg\"]"));
        assert!(json.contains("\"isScriptlet\":true"));
        // None fields should be omitted
        assert!(!json.contains("\"group\""));
        assert!(!json.contains("\"kenv\""));
        assert!(!json.contains("\"preview\""));
    }

    #[test]
    fn test_scriptlet_data_serialization_empty_inputs_omitted() {
        let scriptlet = ScriptletData::new(
            "Simple".to_string(),
            "simple".to_string(),
            "bash".to_string(),
            "echo hello".to_string(),
        );

        let json = serde_json::to_string(&scriptlet).unwrap();
        // Empty inputs should be omitted
        assert!(!json.contains("\"inputs\""));
    }

    #[test]
    fn test_scriptlet_data_deserialization() {
        let json = r#"{
            "name": "Greeter",
            "command": "greeter",
            "tool": "ts",
            "content": "console.log('Hello')",
            "inputs": ["name", "age"],
            "group": "Utils",
            "isScriptlet": true
        }"#;
        let scriptlet: ScriptletData = serde_json::from_str(json).unwrap();
        assert_eq!(scriptlet.name, "Greeter");
        assert_eq!(scriptlet.command, "greeter");
        assert_eq!(scriptlet.tool, "ts");
        assert_eq!(scriptlet.inputs, vec!["name", "age"]);
        assert_eq!(scriptlet.group, Some("Utils".to_string()));
        assert!(scriptlet.is_scriptlet);
    }

    #[test]
    fn test_scriptlet_data_deserialization_minimal() {
        let json = r#"{"name":"X","command":"x","tool":"bash","content":"pwd"}"#;
        let scriptlet: ScriptletData = serde_json::from_str(json).unwrap();
        assert_eq!(scriptlet.name, "X");
        assert!(scriptlet.inputs.is_empty());
        assert!(!scriptlet.is_scriptlet); // defaults to false if not present
    }

    // ============================================================
    // SCRIPTLET MESSAGE TESTS
    // ============================================================

    #[test]
    fn test_serialize_run_scriptlet() {
        let scriptlet = ScriptletData::new(
            "Test".to_string(),
            "test".to_string(),
            "bash".to_string(),
            "echo $1".to_string(),
        );
        let msg = Message::run_scriptlet(
            "req-rs-1".to_string(),
            scriptlet,
            Some(serde_json::json!({"name": "World"})),
            vec!["arg1".to_string()],
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"runScriptlet\""));
        assert!(json.contains("\"requestId\":\"req-rs-1\""));
        assert!(json.contains("\"scriptlet\""));
        assert!(json.contains("\"inputs\""));
        assert!(json.contains("\"args\":[\"arg1\"]"));
    }

    #[test]
    fn test_parse_run_scriptlet() {
        let json = r#"{"type":"runScriptlet","requestId":"req-rs-2","scriptlet":{"name":"Test","command":"test","tool":"bash","content":"pwd","isScriptlet":true},"args":["a","b"]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::RunScriptlet {
                request_id,
                scriptlet,
                inputs,
                args,
            } => {
                assert_eq!(request_id, "req-rs-2");
                assert_eq!(scriptlet.name, "Test");
                assert_eq!(scriptlet.tool, "bash");
                assert_eq!(inputs, None);
                assert_eq!(args, vec!["a", "b"]);
            }
            _ => panic!("Expected RunScriptlet message"),
        }
    }

    #[test]
    fn test_serialize_get_scriptlets() {
        let msg = Message::get_scriptlets("req-gs-1".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getScriptlets\""));
        assert!(json.contains("\"requestId\":\"req-gs-1\""));
        // Optional fields should be omitted when None
        assert!(!json.contains("\"kenv\""));
        assert!(!json.contains("\"group\""));
    }

    #[test]
    fn test_serialize_get_scriptlets_filtered() {
        let msg = Message::get_scriptlets_filtered(
            "req-gs-2".to_string(),
            Some("main".to_string()),
            Some("Utilities".to_string()),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"getScriptlets\""));
        assert!(json.contains("\"kenv\":\"main\""));
        assert!(json.contains("\"group\":\"Utilities\""));
    }

    #[test]
    fn test_parse_get_scriptlets() {
        let json = r#"{"type":"getScriptlets","requestId":"req-gs-3","kenv":"dev"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::GetScriptlets {
                request_id,
                kenv,
                group,
            } => {
                assert_eq!(request_id, "req-gs-3");
                assert_eq!(kenv, Some("dev".to_string()));
                assert_eq!(group, None);
            }
            _ => panic!("Expected GetScriptlets message"),
        }
    }

    #[test]
    fn test_serialize_scriptlet_list() {
        let scriptlets = vec![
            ScriptletData::new(
                "A".to_string(),
                "a".to_string(),
                "bash".to_string(),
                "echo A".to_string(),
            ),
            ScriptletData::new(
                "B".to_string(),
                "b".to_string(),
                "ts".to_string(),
                "console.log('B')".to_string(),
            ),
        ];
        let msg = Message::scriptlet_list("req-sl-1".to_string(), scriptlets);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"scriptletList\""));
        assert!(json.contains("\"requestId\":\"req-sl-1\""));
        assert!(json.contains("\"scriptlets\""));
        assert!(json.contains("\"name\":\"A\""));
        assert!(json.contains("\"name\":\"B\""));
    }

    #[test]
    fn test_parse_scriptlet_list() {
        let json = r#"{"type":"scriptletList","requestId":"req-sl-2","scriptlets":[{"name":"X","command":"x","tool":"bash","content":"pwd","isScriptlet":true}]}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ScriptletList {
                request_id,
                scriptlets,
            } => {
                assert_eq!(request_id, "req-sl-2");
                assert_eq!(scriptlets.len(), 1);
                assert_eq!(scriptlets[0].name, "X");
            }
            _ => panic!("Expected ScriptletList message"),
        }
    }

    #[test]
    fn test_serialize_scriptlet_result_success() {
        let msg = Message::scriptlet_result_success(
            "req-sr-1".to_string(),
            Some("Hello World\n".to_string()),
            Some(0),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"scriptletResult\""));
        assert!(json.contains("\"requestId\":\"req-sr-1\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"output\":\"Hello World\\n\""));
        assert!(json.contains("\"exitCode\":0"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_serialize_scriptlet_result_error() {
        let msg = Message::scriptlet_result_error(
            "req-sr-2".to_string(),
            "Command not found".to_string(),
            Some(127),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"scriptletResult\""));
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Command not found\""));
        assert!(json.contains("\"exitCode\":127"));
        assert!(!json.contains("\"output\""));
    }

    #[test]
    fn test_parse_scriptlet_result_success() {
        let json = r#"{"type":"scriptletResult","requestId":"req-sr-3","success":true,"output":"Done","exitCode":0}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ScriptletResult {
                request_id,
                success,
                output,
                error,
                exit_code,
            } => {
                assert_eq!(request_id, "req-sr-3");
                assert!(success);
                assert_eq!(output, Some("Done".to_string()));
                assert_eq!(error, None);
                assert_eq!(exit_code, Some(0));
            }
            _ => panic!("Expected ScriptletResult message"),
        }
    }

    #[test]
    fn test_parse_scriptlet_result_error() {
        let json =
            r#"{"type":"scriptletResult","requestId":"req-sr-4","success":false,"error":"Failed"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ScriptletResult {
                request_id,
                success,
                output,
                error,
                exit_code,
            } => {
                assert_eq!(request_id, "req-sr-4");
                assert!(!success);
                assert_eq!(output, None);
                assert_eq!(error, Some("Failed".to_string()));
                assert_eq!(exit_code, None);
            }
            _ => panic!("Expected ScriptletResult message"),
        }
    }

    #[test]
    fn test_scriptlet_message_ids() {
        let scriptlet = ScriptletData::new(
            "X".to_string(),
            "x".to_string(),
            "bash".to_string(),
            "pwd".to_string(),
        );
        assert_eq!(
            Message::run_scriptlet("a".to_string(), scriptlet, None, vec![]).id(),
            Some("a")
        );
        assert_eq!(Message::get_scriptlets("b".to_string()).id(), Some("b"));
        assert_eq!(
            Message::get_scriptlets_filtered("c".to_string(), None, None).id(),
            Some("c")
        );
        assert_eq!(
            Message::scriptlet_list("d".to_string(), vec![]).id(),
            Some("d")
        );
        assert_eq!(
            Message::scriptlet_result_success("e".to_string(), None, None).id(),
            Some("e")
        );
        assert_eq!(
            Message::scriptlet_result_error("f".to_string(), "err".to_string(), None).id(),
            Some("f")
        );
    }

    // ============================================================
    // FORCE SUBMIT TESTS (stdin/stdout communication regression tests)
    // These tests ensure the SDK submit() -> GPUI -> script stdin flow works
    // ============================================================

    #[test]
    fn test_parse_force_submit_string() {
        // SDK sends forceSubmit when user calls submit("value")
        let json = r#"{"type":"forceSubmit","value":"selected-value"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ForceSubmit { value } => {
                assert_eq!(value, serde_json::json!("selected-value"));
            }
            _ => panic!("Expected ForceSubmit message"),
        }
    }

    #[test]
    fn test_parse_force_submit_object() {
        // SDK can submit complex objects
        let json = r#"{"type":"forceSubmit","value":{"name":"test","count":42}}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ForceSubmit { value } => {
                assert_eq!(value["name"], "test");
                assert_eq!(value["count"], 42);
            }
            _ => panic!("Expected ForceSubmit message"),
        }
    }

    #[test]
    fn test_parse_force_submit_null() {
        // SDK can submit null (cancel/escape)
        let json = r#"{"type":"forceSubmit","value":null}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::ForceSubmit { value } => {
                assert!(value.is_null());
            }
            _ => panic!("Expected ForceSubmit message"),
        }
    }

    #[test]
    fn test_serialize_submit_response() {
        // GPUI sends submit message back to script's stdin
        let msg = Message::Submit {
            id: "1".to_string(),
            value: Some("user-selection".to_string()),
        };
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"submit\""));
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"value\":\"user-selection\""));
    }

    #[test]
    fn test_force_submit_roundtrip() {
        // Simulate the full flow: SDK submit() -> GPUI -> script stdin
        // 1. SDK sends forceSubmit
        let sdk_json = r#"{"type":"forceSubmit","value":"auto-value"}"#;
        let msg = parse_message(sdk_json).unwrap();

        // 2. GPUI extracts value and creates submit response
        match msg {
            Message::ForceSubmit { value } => {
                let value_str = match &value {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Null => None,
                    other => Some(other.to_string()),
                };

                let response = Message::Submit {
                    id: "1".to_string(),
                    value: value_str,
                };

                // 3. Response is serialized and sent to script stdin
                let response_json = serialize_message(&response).unwrap();
                assert!(response_json.contains("\"value\":\"auto-value\""));

                // 4. Script parses the response
                let parsed = parse_message(&response_json).unwrap();
                match parsed {
                    Message::Submit { id, value } => {
                        assert_eq!(id, "1");
                        assert_eq!(value, Some("auto-value".to_string()));
                    }
                    _ => panic!("Expected Submit message after roundtrip"),
                }
            }
            _ => panic!("Expected ForceSubmit message"),
        }
    }

    // ============================================================
    // SIMULATE CLICK TESTS (Test Infrastructure)
    // ============================================================

    #[test]
    fn test_serialize_simulate_click() {
        let msg = Message::simulate_click("req-click-1".to_string(), 100.0, 200.0);
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"simulateClick\""));
        assert!(json.contains("\"requestId\":\"req-click-1\""));
        assert!(json.contains("\"x\":100"));
        assert!(json.contains("\"y\":200"));
        // button should be omitted when None
        assert!(!json.contains("\"button\""));
    }

    #[test]
    fn test_serialize_simulate_click_with_button() {
        let msg = Message::simulate_click_with_button(
            "req-click-2".to_string(),
            50.5,
            75.5,
            "right".to_string(),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"simulateClick\""));
        assert!(json.contains("\"x\":50.5"));
        assert!(json.contains("\"y\":75.5"));
        assert!(json.contains("\"button\":\"right\""));
    }

    #[test]
    fn test_parse_simulate_click_basic() {
        let json = r#"{"type":"simulateClick","requestId":"req-click-3","x":150,"y":250}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SimulateClick {
                request_id,
                x,
                y,
                button,
            } => {
                assert_eq!(request_id, "req-click-3");
                assert!((x - 150.0).abs() < 0.01);
                assert!((y - 250.0).abs() < 0.01);
                assert_eq!(button, None);
            }
            _ => panic!("Expected SimulateClick message"),
        }
    }

    #[test]
    fn test_parse_simulate_click_with_button() {
        let json = r#"{"type":"simulateClick","requestId":"req-click-4","x":100,"y":200,"button":"middle"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SimulateClick {
                request_id,
                x,
                y,
                button,
            } => {
                assert_eq!(request_id, "req-click-4");
                assert!((x - 100.0).abs() < 0.01);
                assert!((y - 200.0).abs() < 0.01);
                assert_eq!(button, Some("middle".to_string()));
            }
            _ => panic!("Expected SimulateClick message"),
        }
    }

    #[test]
    fn test_serialize_simulate_click_result_success() {
        let msg = Message::simulate_click_success("req-click-5".to_string());
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"simulateClickResult\""));
        assert!(json.contains("\"requestId\":\"req-click-5\""));
        assert!(json.contains("\"success\":true"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_serialize_simulate_click_result_error() {
        let msg = Message::simulate_click_error(
            "req-click-6".to_string(),
            "Coordinates out of bounds".to_string(),
        );
        let json = serialize_message(&msg).unwrap();
        assert!(json.contains("\"type\":\"simulateClickResult\""));
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Coordinates out of bounds\""));
    }

    #[test]
    fn test_parse_simulate_click_result_success() {
        let json = r#"{"type":"simulateClickResult","requestId":"req-click-7","success":true}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SimulateClickResult {
                request_id,
                success,
                error,
            } => {
                assert_eq!(request_id, "req-click-7");
                assert!(success);
                assert_eq!(error, None);
            }
            _ => panic!("Expected SimulateClickResult message"),
        }
    }

    #[test]
    fn test_parse_simulate_click_result_error() {
        let json = r#"{"type":"simulateClickResult","requestId":"req-click-8","success":false,"error":"Failed"}"#;
        let msg = parse_message(json).unwrap();
        match msg {
            Message::SimulateClickResult {
                request_id,
                success,
                error,
            } => {
                assert_eq!(request_id, "req-click-8");
                assert!(!success);
                assert_eq!(error, Some("Failed".to_string()));
            }
            _ => panic!("Expected SimulateClickResult message"),
        }
    }

    #[test]
    fn test_simulate_click_message_ids() {
        assert_eq!(
            Message::simulate_click("a".to_string(), 0.0, 0.0).id(),
            Some("a")
        );
        assert_eq!(
            Message::simulate_click_with_button("b".to_string(), 0.0, 0.0, "left".to_string()).id(),
            Some("b")
        );
        assert_eq!(
            Message::simulate_click_success("c".to_string()).id(),
            Some("c")
        );
        assert_eq!(
            Message::simulate_click_error("d".to_string(), "err".to_string()).id(),
            Some("d")
        );
    }
}

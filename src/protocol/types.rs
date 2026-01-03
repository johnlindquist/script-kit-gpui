//! Protocol types for Script Kit GPUI
//!
//! Contains all the helper types used in protocol messages:
//! - Choice, Field for prompts
//! - Clipboard, Keyboard, Mouse action types
//! - ExecOptions, MouseEventData
//! - ScriptletData, ProtocolAction
//! - Element types for UI querying
//! - Error data types

use serde::{Deserialize, Serialize};

use super::semantic_id::{generate_semantic_id, generate_semantic_id_named};

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
    #[serde(rename = "trimOversize")]
    TrimOversize,
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

/// Mouse event data for the mouse action
///
/// Contains coordinates and optional button for click events.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MouseEventData {
    /// Move to position
    Move { x: f64, y: f64 },
    /// Click at position with optional button
    Click {
        x: f64,
        y: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        button: Option<String>,
    },
    /// Set absolute position
    SetPosition { x: f64, y: f64 },
}

impl MouseEventData {
    /// Get coordinates as (x, y) tuple
    pub fn coordinates(&self) -> (f64, f64) {
        match self {
            MouseEventData::Move { x, y } => (*x, *y),
            MouseEventData::Click { x, y, .. } => (*x, *y),
            MouseEventData::SetPosition { x, y } => (*x, *y),
        }
    }
}

/// Exec command options
///
/// Options for the exec command including working directory, environment, and timeout.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExecOptions {
    /// Working directory for the command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Environment variables (key-value pairs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// Whether to capture stdout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_stdout: Option<bool>,
    /// Whether to capture stderr
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_stderr: Option<bool>,
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

/// Protocol action for the Actions API
///
/// Represents an action that can be displayed in the ActionsDialog.
/// The `has_action` field is CRITICAL - it determines the routing behavior:
/// - `has_action=true`: Rust sends ActionTriggered back to SDK (for actions with onAction handlers)
/// - `has_action=false`: Rust submits the value directly (for simple actions)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolAction {
    /// Display name of the action
    pub name: String,
    /// Optional description shown below the name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional keyboard shortcut (e.g., "cmd+c")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// Value to submit or pass to the action handler
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// CRITICAL: If true, send ActionTriggered to SDK; if false, submit value directly
    #[serde(default)]
    pub has_action: bool,
    /// Whether this action is visible in the list
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Whether to close the dialog after triggering
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<bool>,
}

impl ProtocolAction {
    /// Create a new ProtocolAction with just a name
    pub fn new(name: String) -> Self {
        ProtocolAction {
            name,
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: None,
        }
    }

    /// Default visibility is true when unset.
    /// Actions with `visible: false` should be filtered out of the UI.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.visible.unwrap_or(true)
    }

    /// Default close behavior is true when unset.
    /// Actions with `close: false` should keep the dialog open after triggering.
    #[inline]
    pub fn should_close(&self) -> bool {
        self.close.unwrap_or(true)
    }

    /// Create a ProtocolAction with a value that submits directly
    pub fn with_value(name: String, value: String) -> Self {
        ProtocolAction {
            name,
            description: None,
            shortcut: None,
            value: Some(value),
            has_action: false,
            visible: None,
            close: None,
        }
    }

    /// Create a ProtocolAction that triggers an SDK handler
    pub fn with_handler(name: String) -> Self {
        ProtocolAction {
            name,
            description: None,
            shortcut: None,
            value: None,
            has_action: true,
            visible: None,
            close: None,
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
    /// The kit this scriptlet belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kit: Option<String>,
    /// Source file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Whether this is a scriptlet.
    /// Defaults to `false` when deserialized (for backwards compatibility).
    /// The `ScriptletData::new()` constructor sets this to `true`.
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
            kit: None,
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

    /// Add kit
    pub fn with_kit(mut self, kit: String) -> Self {
        self.kit = Some(kit);
        self
    }

    /// Add source path
    pub fn with_source_path(mut self, path: String) -> Self {
        self.source_path = Some(path);
        self
    }
}

// ============================================================
// DEBUG GRID OVERLAY
// ============================================================

/// Options for the debug grid overlay
///
/// Used with ShowGrid message to configure the visual debugging overlay
/// that displays grid lines, component bounds, and alignment guides.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GridOptions {
    /// Grid line spacing in pixels (8 or 16)
    #[serde(default = "default_grid_size")]
    pub grid_size: u32,

    /// Show component bounding boxes with labels
    #[serde(default)]
    pub show_bounds: bool,

    /// Show CSS box model (padding/margin) visualization
    #[serde(default)]
    pub show_box_model: bool,

    /// Show alignment guides between components
    #[serde(default)]
    pub show_alignment_guides: bool,

    /// Show component dimensions in labels (e.g., "Header (500x45)")
    #[serde(default)]
    pub show_dimensions: bool,

    /// Which components to show bounds for
    /// - "prompts": Top-level prompts only
    /// - "all": All rendered elements
    /// - ["name1", "name2"]: Specific component names
    #[serde(default)]
    pub depth: GridDepthOption,

    /// Optional custom color scheme
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_scheme: Option<GridColorScheme>,
}

fn default_grid_size() -> u32 {
    8
}

/// Depth option for grid bounds display
///
/// Controls which components have their bounding boxes shown.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GridDepthOption {
    /// Preset mode: "prompts" or "all"
    Preset(String),
    /// Specific component names to show bounds for
    Components(Vec<String>),
}

impl Default for GridDepthOption {
    fn default() -> Self {
        GridDepthOption::Preset("prompts".to_string())
    }
}

/// Custom color scheme for the debug grid overlay
///
/// All colors are in "#RRGGBBAA" or "#RRGGBB" hex format.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GridColorScheme {
    /// Color for grid lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_lines: Option<String>,

    /// Color for prompt bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_bounds: Option<String>,

    /// Color for input bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_bounds: Option<String>,

    /// Color for button bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_bounds: Option<String>,

    /// Color for list bounding boxes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_bounds: Option<String>,

    /// Fill color for padding visualization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_fill: Option<String>,

    /// Fill color for margin visualization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_fill: Option<String>,

    /// Color for alignment guide lines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment_guide: Option<String>,
}

// ============================================================
// ERROR DATA
// ============================================================

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

// ============================================================
// LAYOUT INFO (AI Agent Debugging)
// ============================================================

/// Computed box model for a component (padding, margin, gap)
///
/// All values are in pixels. This provides the "why" behind spacing -
/// AI agents can understand if space comes from padding, margin, or gap.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComputedBoxModel {
    /// Padding values (inner spacing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<BoxModelSides>,
    /// Margin values (outer spacing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<BoxModelSides>,
    /// Gap between flex/grid children
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
}

/// Box model sides (top, right, bottom, left)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BoxModelSides {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BoxModelSides {
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// Computed flex properties for a component
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComputedFlexStyle {
    /// Flex direction: "row" or "column"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Flex grow value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grow: Option<f32>,
    /// Flex shrink value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shrink: Option<f32>,
    /// Align items: "start", "center", "end", "stretch"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_items: Option<String>,
    /// Justify content: "start", "center", "end", "space-between", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify_content: Option<String>,
}

/// Bounding rectangle in pixels
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LayoutBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Component type for categorization
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LayoutComponentType {
    Prompt,
    Input,
    Button,
    List,
    ListItem,
    Header,
    #[default]
    Container,
    Panel,
    Other,
}

/// Information about a single component in the layout tree
///
/// This is the core data structure for `getLayoutInfo()`.
/// It provides everything an AI agent needs to understand "why"
/// a component is positioned/sized the way it is.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutComponentInfo {
    /// Component name/identifier
    pub name: String,
    /// Component type for categorization
    #[serde(rename = "type")]
    pub component_type: LayoutComponentType,
    /// Bounding rectangle (absolute position and size)
    pub bounds: LayoutBounds,
    /// Computed box model (padding, margin, gap)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_model: Option<ComputedBoxModel>,
    /// Computed flex properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex: Option<ComputedFlexStyle>,
    /// Nesting depth (0 = root, 1 = child of root, etc.)
    pub depth: u32,
    /// Parent component name (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Child component names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    /// Human-readable explanation of why this component has its current size/position
    /// Example: "Height is 45px = padding(8) + content(28) + padding(8) + divider(1)"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

impl LayoutComponentInfo {
    pub fn new(name: impl Into<String>, component_type: LayoutComponentType) -> Self {
        Self {
            name: name.into(),
            component_type,
            bounds: LayoutBounds::default(),
            box_model: None,
            flex: None,
            depth: 0,
            parent: None,
            children: Vec::new(),
            explanation: None,
        }
    }

    pub fn with_bounds(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.bounds = LayoutBounds {
            x,
            y,
            width,
            height,
        };
        self
    }

    pub fn with_padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        let box_model = self.box_model.get_or_insert_with(ComputedBoxModel::default);
        box_model.padding = Some(BoxModelSides {
            top,
            right,
            bottom,
            left,
        });
        self
    }

    pub fn with_margin(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        let box_model = self.box_model.get_or_insert_with(ComputedBoxModel::default);
        box_model.margin = Some(BoxModelSides {
            top,
            right,
            bottom,
            left,
        });
        self
    }

    pub fn with_gap(mut self, gap: f32) -> Self {
        let box_model = self.box_model.get_or_insert_with(ComputedBoxModel::default);
        box_model.gap = Some(gap);
        self
    }

    pub fn with_flex_column(mut self) -> Self {
        let flex = self.flex.get_or_insert_with(ComputedFlexStyle::default);
        flex.direction = Some("column".to_string());
        self
    }

    pub fn with_flex_row(mut self) -> Self {
        let flex = self.flex.get_or_insert_with(ComputedFlexStyle::default);
        flex.direction = Some("row".to_string());
        self
    }

    pub fn with_flex_grow(mut self, grow: f32) -> Self {
        let flex = self.flex.get_or_insert_with(ComputedFlexStyle::default);
        flex.grow = Some(grow);
        self
    }

    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }
}

/// Full layout information for the current UI state
///
/// Returned by `getLayoutInfo()` SDK function.
/// Contains the component tree and window-level information.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutInfo {
    /// Window dimensions
    pub window_width: f32,
    pub window_height: f32,
    /// Current prompt type (e.g., "arg", "div", "editor", "mainMenu")
    pub prompt_type: String,
    /// All components in the layout tree
    pub components: Vec<LayoutComponentInfo>,
    /// Timestamp when layout was captured (ISO 8601)
    pub timestamp: String,
}

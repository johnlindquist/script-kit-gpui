//! Protocol Message enum for Script Kit GPUI
//!
//! This module contains the main Message enum that represents all possible
//! protocol messages exchanged between scripts and the GPUI app.

use serde::{Deserialize, Serialize};

use super::types::*;

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
    /// Script sends arg prompt with choices and optional actions
    #[serde(rename = "arg")]
    Arg {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        /// Optional actions for the actions panel (Cmd+K to open)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<ProtocolAction>>,
    },

    /// Script sends div (HTML display)
    #[serde(rename = "div")]
    Div {
        id: String,
        html: String,
        /// Tailwind classes for the content container
        #[serde(rename = "containerClasses", skip_serializing_if = "Option::is_none")]
        container_classes: Option<String>,
        /// Optional actions for the actions panel (Cmd+K to open)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<ProtocolAction>>,
        /// Placeholder text (shown in header)
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
        /// Hint text
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// Footer text
        #[serde(skip_serializing_if = "Option::is_none")]
        footer: Option<String>,
        /// Container background color: "transparent", "#RRGGBB", "#RRGGBBAA", or Tailwind color name
        #[serde(rename = "containerBg", skip_serializing_if = "Option::is_none")]
        container_bg: Option<String>,
        /// Container padding in pixels, or "none" to disable
        #[serde(rename = "containerPadding", skip_serializing_if = "Option::is_none")]
        container_padding: Option<serde_json::Value>,
        /// Container opacity (0-100)
        #[serde(skip_serializing_if = "Option::is_none")]
        opacity: Option<u8>,
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

    /// Set the current prompt's input text
    #[serde(rename = "setInput")]
    SetInput { text: String },

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
        /// VSCode-style snippet template with tabstops (e.g., "Hello ${1:name}!")
        #[serde(skip_serializing_if = "Option::is_none")]
        template: Option<String>,
        #[serde(rename = "onInit", skip_serializing_if = "Option::is_none")]
        on_init: Option<String>,
        #[serde(rename = "onSubmit", skip_serializing_if = "Option::is_none")]
        on_submit: Option<String>,
        /// Optional actions for the actions panel (Cmd+K to open)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<ProtocolAction>>,
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
    Fields {
        id: String,
        fields: Vec<Field>,
        /// Optional actions for the actions panel (Cmd+K to open)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<ProtocolAction>>,
    },

    /// Custom HTML form
    #[serde(rename = "form")]
    Form {
        id: String,
        html: String,
        /// Optional actions for the actions panel (Cmd+K to open)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<ProtocolAction>>,
    },

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
        /// Optional actions for the actions panel (Cmd+K to open)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actions: Option<Vec<ProtocolAction>>,
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
        /// If true, return full retina resolution (2x). If false (default), scale down to 1x.
        #[serde(rename = "hiDpi", skip_serializing_if = "Option::is_none")]
        hi_dpi: Option<bool>,
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
    // LAYOUT INFO (AI Agent Debugging)
    // ============================================================
    /// Request layout information with component tree and computed styles
    ///
    /// Returns detailed information about every component's position,
    /// size, padding, margin, gap, and flex properties. Designed to
    /// help AI agents understand "why" components are positioned/sized.
    #[serde(rename = "getLayoutInfo")]
    GetLayoutInfo {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with full layout information
    #[serde(rename = "layoutInfoResult")]
    LayoutInfoResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Full layout information including component tree
        #[serde(flatten)]
        info: LayoutInfo,
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
        /// Optional kit to filter by
        #[serde(skip_serializing_if = "Option::is_none")]
        kit: Option<String>,
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

    // ============================================================
    // DEBUG/VISUAL TESTING
    // ============================================================
    /// Show the debug grid overlay with options
    ///
    /// Displays a grid overlay for visual debugging and layout verification.
    /// The grid shows alignment lines, component bounds, and box model visualization.
    #[serde(rename = "showGrid")]
    ShowGrid {
        /// Grid configuration options (flattened into the message)
        #[serde(flatten)]
        options: GridOptions,
    },

    /// Hide the debug grid overlay
    #[serde(rename = "hideGrid")]
    HideGrid,

    // ============================================================
    // ACTIONS API
    // ============================================================
    /// Set actions to display in the ActionsDialog (incoming from SDK)
    ///
    /// Scripts define actions with optional onAction handlers. The `has_action`
    /// field on each action determines routing:
    /// - `has_action=true`: Send ActionTriggered back to SDK
    /// - `has_action=false`: Submit value directly
    #[serde(rename = "setActions")]
    SetActions {
        /// List of actions to display
        actions: Vec<ProtocolAction>,
    },

    /// Notify SDK that an action was triggered (outgoing to SDK)
    ///
    /// Sent when an action with `has_action=true` is triggered.
    /// The SDK's onAction handler will receive this.
    #[serde(rename = "actionTriggered")]
    ActionTriggered {
        /// Name of the triggered action
        action: String,
        /// Value associated with the action (if any)
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        /// Current input/filter text at time of trigger
        input: String,
    },
}

impl Message {
    /// Create an arg prompt message
    pub fn arg(id: String, placeholder: String, choices: Vec<Choice>) -> Self {
        Message::Arg {
            id,
            placeholder,
            choices,
            actions: None,
        }
    }

    /// Create an arg prompt message with actions
    pub fn arg_with_actions(
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        actions: Vec<ProtocolAction>,
    ) -> Self {
        Message::Arg {
            id,
            placeholder,
            choices,
            actions: if actions.is_empty() {
                None
            } else {
                Some(actions)
            },
        }
    }

    /// Create a div (HTML display) message
    pub fn div(id: String, html: String) -> Self {
        Message::Div {
            id,
            html,
            container_classes: None,
            actions: None,
            placeholder: None,
            hint: None,
            footer: None,
            container_bg: None,
            container_padding: None,
            opacity: None,
        }
    }

    /// Create a div message with container classes
    pub fn div_with_classes(id: String, html: String, container_classes: String) -> Self {
        Message::Div {
            id,
            html,
            container_classes: Some(container_classes),
            actions: None,
            placeholder: None,
            hint: None,
            footer: None,
            container_bg: None,
            container_padding: None,
            opacity: None,
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

    /// Get the prompt ID for prompt-type messages (arg, div, editor, etc.)
    ///
    /// These messages have an `id` field that identifies the prompt session.
    /// Returns None for non-prompt messages.
    pub fn prompt_id(&self) -> Option<&str> {
        match self {
            // Core prompts
            Message::Arg { id, .. }
            | Message::Div { id, .. }
            | Message::Submit { id, .. }
            | Message::Update { id, .. }
            // Text input prompts
            | Message::Editor { id, .. }
            | Message::Mini { id, .. }
            | Message::Micro { id, .. }
            // Selection prompts
            | Message::Select { id, .. }
            // Form prompts
            | Message::Fields { id, .. }
            | Message::Form { id, .. }
            // File/path prompts
            | Message::Path { id, .. }
            | Message::Drop { id, .. }
            // Input capture prompts
            | Message::Hotkey { id, .. }
            // Template/text prompts
            | Message::Template { id, .. }
            | Message::Env { id, .. }
            // Media prompts
            | Message::Chat { id, .. }
            | Message::Term { id, .. }
            | Message::Widget { id, .. }
            | Message::Webcam { id, .. }
            | Message::Mic { id, .. } => Some(id),
            // Clipboard has optional id
            Message::Clipboard { id, .. } => id.as_deref(),
            // All other messages don't have prompt IDs
            _ => None,
        }
    }

    /// Get the request ID for request/response type messages
    ///
    /// These messages have a `request_id` field for correlating requests with responses.
    /// Returns None for non-request messages.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            // Selected text operations
            Message::GetSelectedText { request_id, .. }
            | Message::SetSelectedText { request_id, .. }
            | Message::CheckAccessibility { request_id, .. }
            | Message::RequestAccessibility { request_id, .. }
            | Message::SelectedText { request_id, .. }
            | Message::TextSet { request_id, .. }
            | Message::AccessibilityStatus { request_id, .. }
            // Window information
            | Message::GetWindowBounds { request_id, .. }
            | Message::WindowBounds { request_id, .. }
            // Clipboard history
            | Message::ClipboardHistory { request_id, .. }
            | Message::ClipboardHistoryEntry { request_id, .. }
            | Message::ClipboardHistoryList { request_id, .. }
            | Message::ClipboardHistoryResult { request_id, .. }
            // Window management
            | Message::WindowList { request_id, .. }
            | Message::WindowAction { request_id, .. }
            | Message::WindowListResult { request_id, .. }
            | Message::WindowActionResult { request_id, .. }
            // File search
            | Message::FileSearch { request_id, .. }
            | Message::FileSearchResult { request_id, .. }
            // Screenshot capture
            | Message::CaptureScreenshot { request_id, .. }
            | Message::ScreenshotResult { request_id, .. }
            // State query
            | Message::GetState { request_id, .. }
            | Message::StateResult { request_id, .. }
            // Element query
            | Message::GetElements { request_id, .. }
            | Message::ElementsResult { request_id, .. }
            // Layout info
            | Message::GetLayoutInfo { request_id, .. }
            | Message::LayoutInfoResult { request_id, .. }
            // Scriptlet operations
            | Message::RunScriptlet { request_id, .. }
            | Message::GetScriptlets { request_id, .. }
            | Message::ScriptletList { request_id, .. }
            | Message::ScriptletResult { request_id, .. }
            // Test infrastructure
            | Message::SimulateClick { request_id, .. }
            | Message::SimulateClickResult { request_id, .. } => Some(request_id),
            // All other messages don't have request IDs
            _ => None,
        }
    }

    /// Get the message ID (works for message types that have an ID)
    ///
    /// This is a unified accessor that returns either prompt_id or request_id,
    /// whichever is applicable for the message type.
    pub fn id(&self) -> Option<&str> {
        // Try prompt_id first, then request_id
        self.prompt_id().or_else(|| self.request_id())
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
            template: None,
            on_init: None,
            on_submit: None,
            actions: None,
        }
    }

    /// Create an editor with content and language
    pub fn editor_with_content(id: String, content: String, language: Option<String>) -> Self {
        Message::Editor {
            id,
            content: Some(content),
            language,
            template: None,
            on_init: None,
            on_submit: None,
            actions: None,
        }
    }

    /// Create an editor with a VSCode-style snippet template
    pub fn editor_with_template(id: String, template: String, language: Option<String>) -> Self {
        Message::Editor {
            id,
            content: None,
            language,
            template: Some(template),
            on_init: None,
            on_submit: None,
            actions: None,
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
        Message::Fields {
            id,
            fields,
            actions: None,
        }
    }

    /// Create a form prompt message
    pub fn form(id: String, html: String) -> Self {
        Message::Form {
            id,
            html,
            actions: None,
        }
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
        Message::Term {
            id,
            command,
            actions: None,
        }
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

    /// Create a clipboard history trim oversize request
    pub fn clipboard_history_trim_oversize(request_id: String) -> Self {
        Message::ClipboardHistory {
            request_id,
            action: ClipboardHistoryAction::TrimOversize,
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
        Message::CaptureScreenshot {
            request_id,
            hi_dpi: None,
        }
    }

    /// Create a capture screenshot request with hi_dpi option
    pub fn capture_screenshot_with_options(request_id: String, hi_dpi: Option<bool>) -> Self {
        Message::CaptureScreenshot { request_id, hi_dpi }
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
    // Constructor methods for layout info
    // ============================================================

    /// Create a get layout info request
    pub fn get_layout_info(request_id: String) -> Self {
        Message::GetLayoutInfo { request_id }
    }

    /// Create a layout info result response
    pub fn layout_info_result(request_id: String, info: LayoutInfo) -> Self {
        Message::LayoutInfoResult { request_id, info }
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
            kit: None,
            group: None,
        }
    }

    /// Create a get scriptlets request with filters
    pub fn get_scriptlets_filtered(
        request_id: String,
        kit: Option<String>,
        group: Option<String>,
    ) -> Self {
        Message::GetScriptlets {
            request_id,
            kit,
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

    // ============================================================
    // Constructor methods for Actions API
    // ============================================================

    /// Create an ActionTriggered message to send to SDK
    ///
    /// This is sent when an action with `has_action=true` is triggered.
    pub fn action_triggered(action: String, value: Option<String>, input: String) -> Self {
        Message::ActionTriggered {
            action,
            value,
            input,
        }
    }

    /// Create a SetActions message
    pub fn set_actions(actions: Vec<ProtocolAction>) -> Self {
        Message::SetActions { actions }
    }

    /// Create a SetInput message
    pub fn set_input(text: String) -> Self {
        Message::SetInput { text }
    }

    // ============================================================
    // Constructor methods for debug grid
    // ============================================================

    /// Create a ShowGrid message with default options
    pub fn show_grid() -> Self {
        Message::ShowGrid {
            options: GridOptions::default(),
        }
    }

    /// Create a ShowGrid message with custom options
    pub fn show_grid_with_options(options: GridOptions) -> Self {
        Message::ShowGrid { options }
    }

    /// Create a HideGrid message
    pub fn hide_grid() -> Self {
        Message::HideGrid
    }
}

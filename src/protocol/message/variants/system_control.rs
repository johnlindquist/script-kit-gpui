macro_rules! protocol_message_variants_system_control {
    ($callback:ident, $($variants:tt)*) => {
        $callback! {
            $($variants)*
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

    /// Reserved keyboard automation message.
    ///
    /// GPUI does not currently implement receipt-backed native keyboard input
    /// for this variant. SDK keyboard helpers reject before sending protocol
    /// messages, so this shape is not proof that native input occurred.
    #[serde(rename = "keyboard")]
    Keyboard {
        action: KeyboardAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        keys: Option<String>,
    },

    /// Reserved mouse automation message.
    ///
    /// GPUI does not currently implement receipt-backed native mouse input for
    /// this variant. SDK mouse helpers reject before sending protocol messages,
    /// so this shape is not proof that cursor movement or clicks occurred.
    #[serde(rename = "mouse")]
    Mouse {
        action: MouseAction,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<MouseData>,
    },

    /// Show window
    #[serde(rename = "show")]
    Show {},

    /// Hide window
    #[serde(rename = "hide")]
    Hide {},

    /// Acknowledgement that a `show` or `hide` RPC executed. Closes the
    /// Run-14 Pass-12 finding `tool-window-mutator-rpcs-never-echo-response`:
    /// before this message existed, the show/hide handlers fired the
    /// action but never echoed a reply, so callers using
    /// `session.sh rpc default '{"type":"hide","requestId":"x"}'` always
    /// hit the 5-second timeout even though the window had already
    /// hidden. Carries the post-action `window_visible` state so the
    /// caller can verify intent without a follow-up `getState`.
    #[serde(rename = "windowVisibilityAck")]
    WindowVisibilityAck {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "windowVisible")]
        window_visible: bool,
    },

    /// Redacted acknowledgement that a focused-text Agent Chat fixture open
    /// command executed.
    #[serde(rename = "focusedTextAgentChatFixtureOpenResult")]
    FocusedTextAgentChatFixtureOpenResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Fixture mode: "mock" or "pi".
        mode: String,
        ok: bool,
        /// True when a non-empty instruction was submitted after opening.
        submitted: bool,
        #[serde(rename = "targetId")]
        target_id: String,
        #[serde(rename = "targetKind")]
        target_kind: String,
        #[serde(rename = "textLength")]
        text_length: usize,
        #[serde(rename = "instructionLength")]
        instruction_length: usize,
        #[serde(rename = "errorCode", skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
    },

    /// Redacted acknowledgement that an external automation mutator command
    /// executed. This carries command identity and outcome only; it must not
    /// include prompt text, captured text, assistant output, or clipboard
    /// content.
    #[serde(rename = "externalCommandResult")]
    ExternalCommandResult {
        #[serde(rename = "requestId")]
        request_id: String,
        command: String,
        ok: bool,
        #[serde(rename = "errorCode", skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
        #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
    },

    /// Redacted acknowledgement that an external `triggerAction` command was
    /// routed to a shared actions host. This is automation-only receipt
    /// plumbing; it intentionally carries action identity and routing outcome,
    /// not prompt text, selected text, assistant output, or clipboard content.
    #[serde(rename = "triggerActionResult")]
    TriggerActionResult {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "actionId")]
        action_id: String,
        #[serde(rename = "host")]
        host: Option<String>,
        ok: bool,
        #[serde(rename = "popupClosed")]
        popup_closed: bool,
        #[serde(rename = "errorCode", skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
        #[serde(
            rename = "nativeFooterActivation",
            skip_serializing_if = "Option::is_none"
        )]
        native_footer_activation: Option<serde_json::Value>,
    },

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

    /// Capture the whole currently focused text field for Agent Chat editing.
    #[serde(rename = "captureFocusedText")]
    CaptureFocusedText {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Replace the focused text session with Agent Chat output.
    #[serde(rename = "replaceFocusedText")]
    ReplaceFocusedText {
        #[serde(rename = "sessionId")]
        session_id: String,
        text: String,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Append Agent Chat output to the focused text session.
    #[serde(rename = "appendFocusedText")]
    AppendFocusedText {
        #[serde(rename = "sessionId")]
        session_id: String,
        text: String,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Copy focused-text Agent Chat output without mutating the focused text session.
    #[serde(rename = "copyFocusedTextOutput")]
    CopyFocusedTextOutput {
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

    /// Response carrying a whole-field focused text snapshot.
    #[serde(rename = "focusedTextSnapshot")]
    FocusedTextSnapshot {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response after focused text mutation or copy.
    #[serde(rename = "focusedTextMutation")]
    FocusedTextMutation {
        success: bool,
        action: String,
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
        /// Tile position for tile action
        #[serde(rename = "tilePosition", skip_serializing_if = "Option::is_none")]
        tile_position: Option<TilePosition>,
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
        /// Window info returned for frontmostWindow requests
        #[serde(skip_serializing_if = "Option::is_none")]
        window: Option<SystemWindowInfo>,
    },

    /// Request list of displays/monitors
    #[serde(rename = "displayList")]
    DisplayList {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with list of displays
    #[serde(rename = "displayListResult")]
    DisplayListResult {
        #[serde(rename = "requestId")]
        request_id: String,
        displays: Vec<DisplayInfo>,
    },

    /// Request frontmost window of the previous app (before Script Kit was shown)
    #[serde(rename = "frontmostWindow")]
    FrontmostWindow {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with frontmost window info
    #[serde(rename = "frontmostWindowResult")]
    FrontmostWindowResult {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        window: Option<SystemWindowInfo>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

        }
    };
}

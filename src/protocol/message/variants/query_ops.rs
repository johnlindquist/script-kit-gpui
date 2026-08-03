macro_rules! protocol_message_variants_query_ops {
    ($callback:ident, $($variants:tt)*) => {
        $callback! {
            $($variants)*
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
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    // ============================================================
    // STATE QUERY
    // ============================================================
    /// Request current UI state without modifying it
    #[serde(rename = "getState")]
    GetState {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
        /// Return only compact state fields suitable for agent tooling.
        #[serde(rename = "summaryOnly", default)]
        summary_only: bool,
    },

    /// Response with current UI state.
    ///
    /// `request_id` here is a bare `String` — NOT a length-bounded wrapper —
    /// because the stdin verbatim-echo contract (anomaly slug
    /// `attacker-stdin-requestid-unbounded`, pinned in
    /// `tests/stdin_requestid_verbatim_contract.rs`) requires every inbound
    /// `requestId` to round-trip byte-for-byte onto the response envelope.
    /// The sole bound is the stdin line cap applied at ingest in
    /// `src/stdin_commands/mod.rs`.
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
        /// Active launcher surface contract for main-window state receipts.
        #[serde(
            rename = "surfaceContract",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        surface_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>,
        /// Active overlay/popup launcher contract when one is attached to the host view.
        #[serde(
            rename = "activePopupContract",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        active_popup_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>,
        /// Resolved footer ownership after native host installation and
        /// prompt fallback policy have been applied.
        #[serde(
            rename = "activeFooter",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        active_footer: Option<crate::protocol::ActiveFooterSnapshot>,
        /// Last submit owner/route diagnostic for state-first double-submit proof.
        #[serde(
            rename = "submitDiagnostics",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        submit_diagnostics: Option<serde_json::Value>,
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
        /// Inline Mini AI state snapshot for handoff, draft, and close telemetry proofs.
        #[serde(rename = "miniAi", default, skip_serializing_if = "Option::is_none")]
        mini_ai: Option<serde_json::Value>,
        /// Redacted focused-text Agent Chat state for target-scoped DevTools receipts.
        /// Never exposes captured field text, user instructions, prompt bodies,
        /// assistant output, or clipboard content.
        #[serde(
            rename = "focusedTextAgentChat",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        focused_text_agent_chat: Option<serde_json::Value>,
        /// Rendered filter input decoration ranges from the live input state.
        #[serde(
            rename = "filterInputDecorations",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        filter_input_decorations: Option<serde_json::Value>,
        /// ScriptList filter pipeline diagnostic for stale canonical/visual
        /// input proofs. Omitted outside ScriptList.
        #[serde(
            rename = "filterInputDiagnostics",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        filter_input_diagnostics: Option<serde_json::Value>,
        /// Grammar-aware read-only hint shown in the main menu surface
        /// when Power Syntax owns input or a structured query is empty.
        #[serde(
            rename = "menuSyntaxMainHint",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        menu_syntax_main_hint: Option<crate::menu_syntax::MenuSyntaxMainHintSnapshot>,
        /// History-driven autocomplete popup snapshot for the capture
        /// composer. Populated when the cursor is parked on a bare `#`
        /// or fresh `key:` slot AND the matching per-target history
        /// pool is non-empty. None in every other case so automation
        /// can use presence as a "popup is offered" signal.
        #[serde(
            rename = "captureHistoryPicker",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        capture_history_picker:
            Option<crate::menu_syntax::capture_history_picker::HistoryPickerSnapshot>,
        /// Main-window Enter/Tab contract for the currently selected ScriptList row.
        /// Omitted outside ScriptList or when no selected row can execute.
        #[serde(
            rename = "mainWindowPreflight",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        main_window_preflight: Option<serde_json::Value>,
        /// Content-light actions dialog receipt for state-first action menu tests.
        /// Includes IDs and labels only, never row bodies or raw local content.
        #[serde(
            rename = "actionsDialog",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        actions_dialog: Option<serde_json::Value>,
        /// Root file-search provider status used by state-first stability proofs.
        /// Omitted outside ScriptList.
        #[serde(
            rename = "rootFileSearch",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        root_file_search: Option<serde_json::Value>,
        /// Main ScriptList scroll geometry for state-first footer-safe reveal proofs.
        /// Omitted outside ScriptList.
        #[serde(
            rename = "mainListScroll",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        main_list_scroll: Option<serde_json::Value>,
        /// Stable native-list semantic and viewport contract for the active
        /// Script List or migrated built-in list surface.
        #[serde(
            rename = "activeListScroll",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        active_list_scroll: Option<serde_json::Value>,
        /// Identity (bare filename) of the most recent Tab AI screenshot
        /// captured in this process lifetime. `None` when no capture has
        /// occurred. Lets automation verify the identity-threading chain
        /// from capture through Agent Chat context without grepping the filesystem.
        #[serde(
            rename = "screenshotIdentity",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        screenshot_identity: Option<String>,
        /// Redacted DropPrompt file metadata for state-first automation receipts.
        /// Omitted outside DropPrompt; contains count/name/size only, never paths.
        #[serde(rename = "drop", default, skip_serializing_if = "Option::is_none")]
        drop_state: Option<serde_json::Value>,
        /// PathPrompt filesystem load and selection state for edge-case proofs.
        /// Omitted outside PathPrompt.
        #[serde(rename = "path", default, skip_serializing_if = "Option::is_none")]
        path_state: Option<serde_json::Value>,
        /// Passive Notes window state for agent-facing UI/debug receipts.
        /// Omitted outside Notes targets and never exposes raw note content.
        #[serde(rename = "notes", default, skip_serializing_if = "Option::is_none")]
        notes_state: Option<serde_json::Value>,
        /// Passive Day Page state for agent-facing UI/debug receipts.
        /// Omitted outside Day Page and never exposes raw editor content.
        #[serde(rename = "dayPage", default, skip_serializing_if = "Option::is_none")]
        day_page_state: Option<serde_json::Value>,
        /// Passive Dictation runtime state for media/debug receipts.
        /// This never starts capture or exposes transcript content.
        #[serde(
            rename = "dictation",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        dictation_state: Option<serde_json::Value>,
        /// Ghost text prediction state for inline command completion.
        /// Populated when a dominant prefix-match candidate exists.
        #[serde(
            rename = "ghostPrediction",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        ghost_prediction: Option<serde_json::Value>,
        /// Flow UX exploration state: active variant, roster status, runs
        /// with phase/engagement/timings, and Flow Manager visibility.
        /// Shape: docs/ai/flow-ux-protocol.md §6. Password input values are
        /// never present. Omitted when the flow substrate has no state.
        #[serde(rename = "flowUx", default, skip_serializing_if = "Option::is_none")]
        flow_ux: Option<serde_json::Value>,
        /// Canonical backgrounded-AI-session store receipt (spec §8 step 6):
        /// count plus per-session tagged id, surface, liveness, turnInFlight,
        /// and lastActivityUnixMs. Privacy-safe by construction — never
        /// transcripts, prompts, drafts, provider payloads, or titles.
        #[serde(
            rename = "backgroundedSessions",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        backgrounded_sessions: Option<serde_json::Value>,
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
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
        /// Include non-selectable section-header rows (default: false).
        /// Layout-stability probes use this to audit the persistent leading
        /// separator contract; ordinary consumers keep the selectable-only view.
        #[serde(
            rename = "includeHeaders",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        include_headers: Option<bool>,
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
        /// True when limit caused the response to omit elements
        #[serde(default)]
        truncated: bool,
        /// Semantic ID of the currently focused element, if any
        #[serde(rename = "focusedSemanticId", default, skip_serializing_if = "Option::is_none")]
        focused_semantic_id: Option<String>,
        /// Semantic ID of the currently selected element, if any
        #[serde(rename = "selectedSemanticId", default, skip_serializing_if = "Option::is_none")]
        selected_semantic_id: Option<String>,
        /// Machine-readable collection warnings
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        warnings: Vec<String>,
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
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
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
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
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

    // ============================================================
    // MENU BAR INTEGRATION
    // ============================================================
    /// Request menu bar items from the frontmost app or a specific app
    ///
    /// SDK sends this to get the menu bar hierarchy from an application.
    /// If bundle_id is None, uses the frontmost application.
    #[serde(rename = "getMenuBar")]
    GetMenuBar {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Optional bundle ID to get menu bar from a specific app
        #[serde(rename = "bundleId", skip_serializing_if = "Option::is_none")]
        bundle_id: Option<String>,
    },

    /// Response with menu bar items
    ///
    /// App sends this back to SDK with the menu bar hierarchy.
    #[serde(rename = "menuBarResult")]
    MenuBarResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// The menu bar items (hierarchical)
        items: Vec<super::types::MenuBarItemData>,
    },

    /// Execute a menu action by path
    ///
    /// SDK sends this to click a menu item in a specific application.
    #[serde(rename = "executeMenuAction")]
    ExecuteMenuAction {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Bundle ID of the target application
        #[serde(rename = "bundleId")]
        bundle_id: String,
        /// Path of menu titles to the target item (e.g., ["File", "New", "Window"])
        path: Vec<String>,
    },

    /// Result of a menu action execution
    ///
    /// App sends this back to SDK after attempting to execute a menu action.
    #[serde(rename = "menuActionResult")]
    MenuActionResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Whether the action succeeded
        success: bool,
        /// Error message if failed
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    // ============================================================
    // Agent Chat STATE QUERY
    // ============================================================
    /// Request machine-readable Agent Chat chat view state for agentic testing.
    ///
    /// Returns input text, cursor position, picker state, accepted item
    /// metadata, thread status, and layout stability metrics.
    #[serde(rename = "getAgentChatState")]
    GetAgentChatState {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Response with Agent Chat chat view state snapshot.
    #[serde(rename = "agent_chatStateResult")]
    AgentChatStateResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Machine-readable Agent Chat state snapshot.
        #[serde(flatten)]
        state: AgentChatStateSnapshot,
    },

    /// Request redacted reliability state for one automation target.
    #[serde(rename = "getAiReliabilityState")]
    GetAiReliabilityState {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Response with redacted reliability state for the resolved target.
    #[serde(rename = "aiReliabilityStateResult")]
    AiReliabilityStateResult {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(flatten)]
        state: AiReliabilityStateSnapshot,
    },

    /// Install a deterministic, payload-free reliability fixture on a target.
    #[serde(rename = "setAiReliabilityTestFixture")]
    SetAiReliabilityTestFixture {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "fixtureId")]
        fixture_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Result of installing a deterministic reliability fixture.
    #[serde(rename = "aiReliabilityTestFixtureResult")]
    AiReliabilityTestFixtureResult {
        #[serde(rename = "requestId")]
        request_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<AiReliabilityStateSnapshot>,
    },

    // ============================================================
    // Agent Chat CONTEXT PREPARATION PROBE
    // ============================================================
    /// Run a bounded synthetic context-preparation fixture inside the real app.
    #[serde(rename = "inspectContextPreparation")]
    InspectContextPreparation {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "fixtureId")]
        fixture_id: String,
    },

    /// Privacy-safe result from the canonical context-preparation boundary.
    #[serde(rename = "contextPreparationProbeResult")]
    ContextPreparationProbeResult {
        #[serde(rename = "requestId")]
        request_id: String,
        receipt: serde_json::Value,
    },

    // ============================================================
    // Agent Chat TEST PROBE
    // ============================================================
    /// Reset the Agent Chat test probe ring buffer, clearing prior events.
    ///
    /// Accepts an optional `target` to reset a specific Agent Chat surface
    /// (e.g. a detached Agent Chat window). When omitted, resets the main
    /// window's Agent Chat probe (backward-compatible).
    ///
    /// Agents should call this before a test sequence to get a clean
    /// baseline for verifying picker acceptance and key routing.
    #[serde(rename = "resetAgentChatTestProbe")]
    ResetAgentChatTestProbe {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Optional window target (defaults to main Agent Chat view).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Request a bounded snapshot of recent Agent Chat test probe events.
    ///
    /// Returns key-route, picker-acceptance, and input-layout telemetry
    /// from the bounded ring buffer, plus the current Agent Chat state.
    #[serde(rename = "getAgentChatTestProbe")]
    GetAgentChatTestProbe {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Maximum number of events to return per category (default: 32).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tail: Option<usize>,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Response with Agent Chat test probe snapshot.
    #[serde(rename = "agent_chatTestProbeResult")]
    AgentChatTestProbeResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Bounded probe snapshot with telemetry events and current state.
        #[serde(flatten)]
        probe: AgentChatTestProbeSnapshot,
    },

    // ============================================================
    // Agent Chat SETUP ACTIONS
    // ============================================================
    /// Perform a setup action on the Agent Chat setup card.
    ///
    /// Agents use this to drive setup recovery deterministically instead of
    /// simulating Tab/Enter keystrokes. Every success response includes a
    /// fresh `AgentChatStateSnapshot` so the agent can verify the effect.
    #[serde(rename = "performAgentChatSetupAction")]
    PerformAgentChatSetupAction {
        #[serde(rename = "requestId")]
        request_id: String,
        /// The action to perform.
        action: AgentChatSetupActionKind,
        /// Agent ID for `selectAgent` action.
        #[serde(rename = "agentId", default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Result of a `performAgentChatSetupAction` request.
    #[serde(rename = "agent_chatSetupActionResult")]
    AgentChatSetupActionResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Whether the action succeeded.
        success: bool,
        /// Error message if the action failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// Fresh Agent Chat state snapshot after the action.
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<AgentChatStateSnapshot>,
    },

    // ============================================================
    // WAIT / BATCH — Deterministic Transaction Layer
    // ============================================================
    /// Poll until a UI condition is satisfied or timeout expires.
    ///
    /// The app will check the condition at `pollInterval` (default 25 ms)
    /// and reply with `waitForResult` when satisfied or after `timeout` ms.
    #[serde(rename = "waitFor")]
    WaitFor {
        #[serde(rename = "requestId")]
        request_id: String,
        /// The condition to wait for
        condition: WaitCondition,
        /// Timeout in milliseconds (default: 5000)
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
        /// Poll interval in milliseconds (default: 25)
        #[serde(rename = "pollInterval", skip_serializing_if = "Option::is_none")]
        poll_interval: Option<u64>,
        /// Trace mode: off (default), on, or onFailure
        #[serde(default, skip_serializing_if = "super::types::batch_wait::is_trace_off")]
        trace: TransactionTraceMode,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Result of a waitFor request.
    #[serde(rename = "waitForResult")]
    WaitForResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Whether the condition was satisfied before timeout
        success: bool,
        /// Wall-clock time elapsed in milliseconds
        elapsed: u64,
        /// Structured error if the wait failed (e.g., timeout)
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<TransactionError>,
        /// Embedded trace receipt (present when trace mode is on or onFailure+failed)
        #[serde(skip_serializing_if = "Option::is_none")]
        trace: Option<TransactionTrace>,
    },

    /// Execute a sequence of atomic UI commands as a transaction.
    ///
    /// Commands run in order. If `options.stop_on_error` is true (default),
    /// execution halts on the first failure and `failed_at` is set.
    #[serde(rename = "batch")]
    Batch {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Ordered list of commands to execute
        commands: Vec<BatchCommand>,
        /// Execution options (stop-on-error, timeout)
        #[serde(skip_serializing_if = "Option::is_none")]
        options: Option<BatchOptions>,
        /// Trace mode: off (default), on, or onFailure
        #[serde(default, skip_serializing_if = "super::types::batch_wait::is_trace_off")]
        trace: TransactionTraceMode,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
    },

    /// Result of a batch execution.
    #[serde(rename = "batchResult")]
    BatchResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// True if all commands succeeded
        success: bool,
        /// Per-command results
        results: Vec<BatchResultEntry>,
        /// Index of the first failed command, if any
        #[serde(rename = "failedAt", skip_serializing_if = "Option::is_none")]
        failed_at: Option<usize>,
        /// Total wall-clock time for the entire batch, in milliseconds
        #[serde(rename = "totalElapsed")]
        total_elapsed: u64,
        /// Embedded trace receipt (present when trace mode is on or onFailure+failed)
        #[serde(skip_serializing_if = "Option::is_none")]
        trace: Option<TransactionTrace>,
    },

    // ============================================================
    // AUTOMATION WINDOW TARGETING
    // ============================================================
    /// List all automation-addressable windows with their IDs, kinds, and state.
    #[serde(rename = "listAutomationWindows")]
    ListAutomationWindows {
        #[serde(rename = "requestId")]
        request_id: String,
    },

    /// Response with the list of automation windows.
    #[serde(rename = "automationWindowListResult")]
    AutomationWindowListResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// All currently registered automation windows.
        windows: Vec<AutomationWindowInfo>,
        /// ID of the window that currently has focus, if any.
        #[serde(rename = "focusedWindowId", default, skip_serializing_if = "Option::is_none")]
        focused_window_id: Option<String>,
    },

    // ============================================================
    // GPUI EVENT SIMULATION
    // ============================================================
    /// Dispatch a high-fidelity input event through GPUI's real event pipeline.
    #[serde(rename = "simulateGpuiEvent")]
    SimulateGpuiEvent {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Optional window target (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
        /// The event to simulate.
        event: SimulatedGpuiEvent,
    },

    /// Result of a GPUI event simulation.
    #[serde(rename = "simulateGpuiEventResult")]
    SimulateGpuiEventResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Whether the event was dispatched successfully.
        success: bool,
        /// Machine-readable error category: `target_not_found`, `target_ambiguous`,
        /// `handle_unavailable`, or `dispatch_failed`.
        #[serde(rename = "errorCode", skip_serializing_if = "Option::is_none")]
        error_code: Option<String>,
        /// Human-readable error message if dispatch failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// The dispatch path used: `"exact_handle"` or `"window_role_fallback"`.
        #[serde(
            rename = "dispatchPath",
            skip_serializing_if = "Option::is_none",
            default
        )]
        dispatch_path: Option<String>,
        /// The resolved automation window ID, when available.
        #[serde(
            rename = "resolvedWindowId",
            skip_serializing_if = "Option::is_none",
            default
        )]
        resolved_window_id: Option<String>,
        /// True when GPUI applied the event before this response was emitted.
        #[serde(rename = "dispatchCompleted", default)]
        dispatch_completed: bool,
        /// True when GPUI scheduled dispatch for a later update.
        #[serde(rename = "dispatchScheduled", default)]
        dispatch_scheduled: bool,
        /// Raw dispatch does not prove a specific UI handler fired.
        #[serde(
            rename = "activationProof",
            skip_serializing_if = "Option::is_none",
            default
        )]
        activation_proof: Option<String>,
    },

    // ============================================================
    // EXACT-WINDOW INSPECTION
    // ============================================================
    /// Inspect one exact automation window: resolved identity, screenshot
    /// dimensions, optional pixel probes, and semantic elements when available.
    #[serde(rename = "inspectAutomationWindow")]
    InspectAutomationWindow {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Window target to inspect (defaults to focused window).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<AutomationWindowTarget>,
        /// If true, capture at full retina resolution (2x). Default: false (1x).
        #[serde(rename = "hiDpi", default, skip_serializing_if = "Option::is_none")]
        hi_dpi: Option<bool>,
        /// Pixel coordinates to sample from the captured screenshot.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        probes: Vec<super::types::automation_inspect::PixelProbe>,
    },

    /// Result of an `inspectAutomationWindow` request.
    #[serde(rename = "automationInspectResult")]
    AutomationInspectResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// The inspection snapshot.
        #[serde(flatten)]
        snapshot: super::types::automation_inspect::AutomationInspectSnapshot,
    },

    // ============================================================
    // IN-PROCESS LOG QUERY
    // ============================================================
    /// Fetch recent structured log entries from the in-process ring buffer,
    /// so agents can assert on log content in the same receipt as UI state
    /// without touching files on disk.
    #[serde(rename = "getLogs")]
    GetLogs {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Max entries to return (default 100, capped at the ring capacity).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        /// Minimum severity: trace|debug|info|warn|error.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<String>,
        /// Substring match on the tracing target (e.g. "brain", "ai::agent_chat").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        /// Substring match on the message text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contains: Option<String>,
    },

    /// Response with recent structured log entries.
    #[serde(rename = "logsResult")]
    LogsResult {
        #[serde(rename = "requestId")]
        request_id: String,
        /// Matching entries, oldest first.
        entries: Vec<serde_json::Value>,
        /// Total entries in the ring that matched the filters (may exceed
        /// `entries.len()` when `limit` truncated the result).
        matched: usize,
        /// Ring capacity, so agents know the retention window.
        capacity: usize,
    },

        }
    };
}

//! JSONL Protocol for Script Kit GPUI
//!
//! Defines message types for bidirectional communication between scripts and the GPUI app.
//! Messages are exchanged as newline-delimited JSON (JSONL), with each message tagged by a `type` field.
//!
//! # Message Categories
//!
//! ## Prompts (script → app, await user input)
//! - `arg`: Choice selection with optional search
//! - `div`: Display HTML/markdown content
//! - `editor`: Code/text editor
//! - `fields`: Multi-field form
//! - `form`: Custom form layout
//! - `path`: File/directory picker
//! - `drop`: Drag-and-drop target
//! - `hotkey`: Keyboard shortcut capture
//! - `term`: Terminal emulator
//! - `chat`, `mic`, `webcam`: Media prompts
//!
//! ## Responses (app → script)
//! - `submit`: User selection or form submission
//! - `update`: Live updates (keystrokes, selections)
//!
//! ## System Control
//! - `exit`: Terminate script
//! - `show`/`hide`: Window visibility
//! - `setPosition`, `setSize`, `setAlwaysOnTop`: Window management
//! - `setPanel`, `setPreview`, `setPrompt`, `setInput`: UI updates
//! - `setActions`, `actionTriggered`: Actions menu
//!
//! ## State Queries (request/response pattern)
//! - `getState`/`stateResult`: App state
//! - `getSelectedText`/`selectedText`: System selection
//! - `captureScreenshot`/`screenshotResult`: Window capture
//! - `getWindowBounds`/`windowBounds`: Window geometry
//! - `clipboardHistory`/`clipboardHistoryResult`: Clipboard access
//!
//! ## Scriptlets
//! - `runScriptlet`, `getScriptlets`, `scriptletList`, `scriptletResult`
//!
//! # Module Structure
//!
//! - `types`: Helper types (Choice, Field, ClipboardAction, MouseEventData, ExecOptions, etc.)
//! - `message`: The main Message enum (59+ variants) and constructors
//! - `semantic_id`: Semantic ID generation for AI-driven UX
//! - `io`: JSONL parsing with graceful error handling, serialization, streaming readers
//!
//! # API Visibility
//!
//! Parsing classification internals remain crate-private to avoid leaking parser
//! implementation details as a public contract.
//!
//! ```compile_fail
//! use script_kit_gpui::protocol::parse_message_graceful;
//! use script_kit_gpui::protocol::ParseResult;
//! ```

#![allow(dead_code)]

pub mod deprecations;
pub mod ingress;
mod io;
mod message;
mod semantic_id;
pub mod transaction_executor;
pub mod transaction_trace;
mod types;
pub mod version;

#[allow(unused_imports)]
pub(crate) use io::malformed_request_error;
pub use io::{serialize_message, JsonlReader, ParseIssueKind};
#[allow(unused_imports)]
pub use message::{capabilities, Message};
#[allow(unused_imports)]
pub use semantic_id::{generate_semantic_id, generate_semantic_id_named, value_to_slug};
#[allow(unused_imports)]
pub use types::{
    default_suggested_hit_points, default_surface_hit_point, target_bounds_in_screenshot,
    target_bounds_in_screenshot_with_main, ActiveFooterButtonSnapshot, ActiveFooterCwdChipSnapshot,
    ActiveFooterLeftInfoSnapshot, ActiveFooterSnapshot, AgentChatAcceptedItem,
    AgentChatComposerScrollMetrics, AgentChatFocusedTextActionReceipt, AgentChatFocusedTextState,
    AgentChatInputLayoutMetrics, AgentChatInputLayoutTelemetry, AgentChatKeyRoute,
    AgentChatKeyRouteTelemetry, AgentChatLastInteractionTrace,
    AgentChatPickerItemAcceptedTelemetry, AgentChatPickerState, AgentChatResolvedTarget,
    AgentChatSetupActionKind, AgentChatSetupSnapshot, AgentChatSpineSnapshot,
    AgentChatStateSnapshot, AgentChatTestProbeSnapshot, AgentChatTranscriptScrollMetrics,
    AgentChatWaitCondition, AiChatInfo, AiContextPartInput, AiMessageInfo, AppKitFidelityColor,
    AppKitFidelityImage, AppKitFidelityLayer, AppKitFidelityNode, AppKitFidelitySnapshot,
    AppKitFidelityText, AppKitFooterLeftAllocation, AutomationInspectSnapshot,
    AutomationSurfaceSnapshot, AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind,
    AutomationWindowTarget, BatchCommand, BatchOptions, BatchResultEntry, BoxModelSides,
    ChatMessagePosition, ChatMessageRole, ChatPromptConfig, ChatPromptMessage, Choice,
    ClipboardAction, ClipboardEntryType, ClipboardFormat, ClipboardHistoryAction,
    ClipboardHistoryEntryData, ComputedBoxModel, ComputedFlexStyle, DisplayInfo,
    ElementEditorRuntimeInfo, ElementInfo, ElementStyleInfo, ElementType, ExecOptions,
    FidelityCaptureStatus, FidelityLayoutNode, FidelityLayoutSnapshot, FidelityPaintTargetSnapshot,
    FidelityUnscopedPaintSummary, Field, FileSearchResultEntry, GridColorScheme, GridDepthOption,
    GridOptions, InspectBoundsInScreenshot, InspectPoint, LauncherSurfaceContractSnapshot,
    LayoutBounds, LayoutComponentInfo, LayoutComponentType, LayoutInfo, MenuBarItemData,
    MouseAction, MouseData, PixelProbe, PixelProbeResult, ProtocolAction, ScriptErrorData,
    ScriptletData, ScriptletMetadataData, SemanticQuality, SimulatedGpuiEvent,
    SimulatedScrollPhase, SimulatedTouchPhase, StateMatchSpec, SubmitValue, SuggestedHitPoint,
    SystemWindowInfo, TargetWindowBounds, TilePosition, TransactionCommandTrace, TransactionError,
    TransactionErrorCode, TransactionTrace, TransactionTraceMode, TransactionTraceStatus,
    UiStateSnapshot, WaitCondition, WaitDetailedCondition, WaitNamedCondition, WaitPollObservation,
    WindowActionType, ACTIVE_FOOTER_SCHEMA_VERSION, AGENT_CHAT_STATE_SCHEMA_VERSION,
    AGENT_CHAT_TEST_PROBE_MAX_EVENTS, AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION,
    AUTOMATION_INSPECT_SCHEMA_VERSION, AUTOMATION_SURFACE_SCHEMA_VERSION,
    AUTOMATION_WINDOW_SCHEMA_VERSION, LAUNCHER_SURFACE_CONTRACT_SCHEMA_VERSION,
    TRANSACTION_TRACE_SCHEMA_VERSION,
};

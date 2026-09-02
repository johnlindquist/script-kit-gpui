//! Protocol types for Script Kit GPUI
//!
//! Contains all the helper types used in protocol messages:
//! - Choice, Field for prompts
//! - Clipboard, Keyboard, Mouse action types
//! - ExecOptions, MouseEventData
//! - SubmitValue for JSON-capable submit values
//! - ScriptletData, ProtocolAction
//! - Element types for UI querying
//! - Error data types

pub(crate) mod agent_chat_state;
mod ai;
pub mod ai_reliability_state;
pub mod automation_inspect;
pub mod automation_inspect_geometry;
pub mod automation_surface;
pub mod automation_window;
pub(crate) mod batch_wait;
mod chat;
pub mod design_evaluation;
mod elements_actions_scriptlets;
mod grid_layout;
mod input;
mod menu_bar;
mod primitives;
pub mod simulated_gpui_event;
mod system;

pub use agent_chat_state::{
    AgentChatAcceptedItem, AgentChatComposerScrollMetrics, AgentChatContextPartSnapshot,
    AgentChatFocusedTextActionReceipt, AgentChatFocusedTextState, AgentChatInputLayoutMetrics,
    AgentChatInputLayoutTelemetry, AgentChatKeyRoute, AgentChatKeyRouteTelemetry,
    AgentChatLastInteractionTrace, AgentChatPickerItemAcceptedTelemetry, AgentChatPickerState,
    AgentChatResolvedTarget, AgentChatSetupActionKind, AgentChatSetupSnapshot,
    AgentChatSpineSnapshot, AgentChatStateSnapshot, AgentChatTestProbeSnapshot,
    AgentChatTranscriptScrollMetrics, AgentChatWaitCondition, AGENT_CHAT_STATE_SCHEMA_VERSION,
    AGENT_CHAT_TEST_PROBE_MAX_EVENTS, AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION,
};
pub use ai::{AiChatInfo, AiContextPartInput, AiMessageInfo};
pub use ai_reliability_state::{
    AiReliabilityDiagnosticSnapshot, AiReliabilityIdentitySnapshot,
    AiReliabilityPreservationSnapshot, AiReliabilityRetrySnapshot, AiReliabilityStateSnapshot,
    AiReliabilityTransitionSnapshot, AI_RELIABILITY_STATE_SCHEMA_VERSION,
};
#[allow(unused_imports)]
pub use automation_inspect::{
    AutomationInspectSnapshot, InspectBoundsInScreenshot, InspectPoint, PixelProbe,
    PixelProbeResult, SemanticQuality, SuggestedHitPoint, AUTOMATION_INSPECT_SCHEMA_VERSION,
};
pub use automation_inspect_geometry::{
    default_suggested_hit_points, default_surface_hit_point, target_bounds_in_screenshot,
    target_bounds_in_screenshot_with_main,
};
pub use automation_surface::{
    ActiveFooterButtonSnapshot, ActiveFooterCwdChipSnapshot, ActiveFooterLeftInfoSnapshot,
    ActiveFooterSnapshot, AutomationSurfaceSnapshot, AutomationTargetIdentitySnapshot,
    LauncherSurfaceContractSnapshot, SurfacePresentationSnapshot, SurfaceRowPrimitive,
    ACTIVE_FOOTER_SCHEMA_VERSION, AUTOMATION_SURFACE_SCHEMA_VERSION,
    LAUNCHER_SURFACE_CONTRACT_SCHEMA_VERSION,
};
pub use automation_window::{
    AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget,
    AUTOMATION_WINDOW_SCHEMA_VERSION,
};
pub use batch_wait::{
    BatchCommand, BatchOptions, BatchResultEntry, StateMatchSpec, TransactionCommandTrace,
    TransactionError, TransactionErrorCode, TransactionTrace, TransactionTraceMode,
    TransactionTraceStatus, UiStateSnapshot, WaitCondition, WaitDetailedCondition,
    WaitNamedCondition, WaitPollObservation, TRANSACTION_TRACE_SCHEMA_VERSION,
};
pub use chat::{ChatMessagePosition, ChatMessageRole, ChatPromptConfig, ChatPromptMessage};
#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub use design_evaluation::{
    AgentChatFixtureCommand, DesignCommand, DictationFixtureCommand, DictationFixtureDestination,
    EvaluationLimits, FixtureControl, FixtureDescriptor, FlowFixtureCommand, NativeSafetyProbe,
    NotesFixtureCommand, ObservedEffect, OwnedFileSearchPreviewCondition,
    OwnedFileSearchStreamCondition, OwnedFrameCursor, OwnedResponseCodec, OwnedResponseEncoding,
    OwnedRuntimeIdentity, OwnedSearchProviderCondition, OwnedSearchProviderSource,
    OwnedSearchQueryStamp, ScheduledFrameRequirement, ScopedActionReceipt, SdkChatFixtureCommand,
    SdkCompletionChannel, SdkPromptCommand, SdkPromptFixtureId, SearchFixtureCommand,
    ThemeFaultOperation, ThemeFixtureCommand, OWNED_RESPONSE_CODEC,
};
pub use design_evaluation::{
    CompletedFrameIdentity, LiveThemeEdit, ThemeInvalidation, ThemeInvalidationCause,
    MAX_LIVE_THEME_EDITS, OWNED_EVALUATION_LIMITS,
};
pub use elements_actions_scriptlets::{
    ConversationSemanticAction, ConversationSemanticRole, ElementContentDescriptor,
    ElementContentKind, ElementEditorRuntimeInfo, ElementInfo, ElementStyleInfo, ElementType,
    ProjectionQuality, ProjectionReason, ProtocolAction, RedactedElementContent, ScriptletData,
    ScriptletMetadataData,
};
pub use grid_layout::{
    AppKitFidelityColor, AppKitFidelityImage, AppKitFidelityLayer, AppKitFidelityNode,
    AppKitFidelitySnapshot, AppKitFidelityText, AppKitFooterLeftAllocation, BoxModelSides,
    ComputedBoxModel, ComputedFlexStyle, FidelityCaptureStatus, FidelityLayoutNode,
    FidelityLayoutSnapshot, FidelityPaintTargetSnapshot, FidelityUnscopedPaintSummary,
    GeometryRole, GridColorScheme, GridDepthOption, GridOptions, LayoutBounds, LayoutComponentInfo,
    LayoutComponentType, LayoutInfo, ScriptErrorData,
};
pub use input::{ExecOptions, MouseData};
pub use menu_bar::MenuBarItemData;
pub use primitives::{
    Choice, ClipboardAction, ClipboardEntryType, ClipboardFormat, ClipboardHistoryAction, Field,
    KeyboardAction, MouseAction, SubmitValue, TilePosition, WindowActionType,
};
pub use simulated_gpui_event::{SimulatedGpuiEvent, SimulatedScrollPhase, SimulatedTouchPhase};
pub use system::{
    ClipboardHistoryEntryData, DisplayInfo, FileSearchResultEntry, SystemWindowInfo,
    TargetWindowBounds,
};

#[cfg(test)]
mod tests;

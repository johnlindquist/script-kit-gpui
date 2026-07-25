//! AI surfaces and shared contracts.
//!
//! # Architecture
//!
//! ```text
//! src/ai/
//! ├── harness/     - Harness config + flat context-block / artifact-authoring guidance formatting
//! ├── tab_context.rs - Tab AI context, receipts, memory lookup, compatibility types
//! ├── message_parts.rs - MCP/file context-part composition
//! ├── context_contract.rs - Shared context contract enforcement
//! ├── context_preview.rs - Shared context-part preview derivation (used by Agent Chat portals)
//! ├── current_app_automation_memory.rs - Bundle-scoped prior-automation memory
//! └── model.rs / storage.rs / providers.rs - Chat data + BYOK providers
//! ```
//!
//! # Primary Agent Chat contract
//!
//! - User-facing AI chat surface: Agent Chat
//! - Entry points should route to `open_tab_ai_agent_chat_with_entry_intent(...)` when they need the canonical chat UI
//! - Compatibility-named `tab_ai_*` helpers and harness/context types still back Agent Chat plumbing

// Re-exports intentionally cover the module's API surface.
#![allow(unused_imports)]
#![allow(dead_code)]

pub mod agent_chat;
pub(crate) mod agent_prompt_handoff;
pub mod agent_task_dock;
pub(crate) mod config;
pub(crate) mod context_contract;
#[cfg(test)]
mod context_contract_integration_tests;
pub(crate) mod context_mentions;
pub(crate) mod context_preview;
pub(crate) mod context_selector;
pub(crate) mod context_selector_row;
pub(crate) mod current_app_automation_memory;
pub(crate) mod explicit_target_handoff;
pub mod focused_text;
pub(crate) mod harness;
pub(crate) mod local_llm;
pub mod message_parts;
pub(crate) mod model;
pub(crate) mod phase_trace;
pub mod preflight_audit;
pub(crate) mod presets;
pub(crate) mod providers;
#[cfg(test)]
mod public_contract_tests;
pub mod reliability;
pub mod result_cards;
pub(crate) mod script_generation;
pub(crate) mod sdk_handlers;
pub(crate) mod session;
pub(crate) mod storage;
pub(crate) mod subscriptions;
pub(crate) mod tab_context;

// Re-export commonly used types
pub use self::agent_task_dock::{
    AgentTaskDockArchiveDecision, AgentTaskDockResumeDecision, AgentTaskDockSelection,
    AgentTaskDockState, AgentTaskDockStatus, AgentTaskDockSubmitDecision, AgentTaskDockSurface,
    AgentTaskDockTask,
};
pub use self::config::{DetectedKeys, ModelInfo, ProviderConfig};
pub use self::current_app_automation_memory::{
    current_app_automation_memory_index_path, read_current_app_automation_memory_index,
    resolve_current_app_automation_from_memory, upsert_current_app_automation_memory_from_receipt,
    CurrentAppAutomationMemoryDecision, CurrentAppAutomationMemoryIndexEntry,
};
pub use self::harness::{
    build_tab_ai_harness_context_block, build_tab_ai_harness_submission,
    cleanup_old_tab_ai_screenshot_files_in_dir, read_tab_ai_harness_config,
    should_include_artifact_authoring_guidance, tab_ai_harness_config_path,
    tab_ai_screenshot_prefix, tab_ai_surface_preference_for_prompt, validate_tab_ai_harness_config,
    HarnessBackendKind, HarnessConfig, TabAiCaptureKind, TabAiHarnessSessionState,
    TabAiHarnessSubmissionMode, TabAiHarnessWarmState, TabAiScreenshotFile, TabAiSurfacePreference,
    BUN_BUILD_VERIFICATION_MARKER, BUN_EXECUTE_VERIFICATION_MARKER, SCRIPT_AUTHORING_SKILL_MARKER,
    TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION, TAB_AI_HARNESS_CONTEXT_SCHEMA_VERSION,
    TAB_AI_SCREENSHOT_MAX_KEEP,
};
pub use self::harness::{
    plan_tab_ai_quick_submit, TabAiQuickSubmitKind, TabAiQuickSubmitPlan, TabAiQuickSubmitSource,
};
pub use self::message_parts::{
    file_path_parts, merge_context_parts, prepare_user_message_with_receipt,
    resolve_context_part_to_prompt_block, resolve_context_parts_to_prompt_prefix,
    resolve_context_parts_with_receipt, AiContextPart, ContextAssemblyReceipt,
    ContextPartPreparationOutcome, ContextPartPreparationOutcomeKind, ContextResolutionFailure,
    ContextResolutionReceipt, PreparedMessageDecision, PreparedMessageReceipt,
    AI_MESSAGE_PREPARATION_SCHEMA_VERSION,
};
pub use self::model::{Chat, ChatId, ChatSource, Message, MessageRole};
pub use self::preflight_audit::{
    append_preflight_audit, build_actionable_preflight_error, log_preflight_audit,
    ActionableContextFailure, AiPreflightAudit, AI_PREFLIGHT_AUDIT_SCHEMA_VERSION,
};
pub use self::presets::{resolve_agent_chat_preset, AgentChatPresetPlan};
pub use self::providers::{AiProvider, ProviderMessage, ProviderRegistry};
pub use self::result_cards::{
    derive_agent_chat_result_cards_from_assistant_message, AgentChatResultArtifact,
    AgentChatResultArtifactKind, AgentChatResultCards, AgentChatResultFollowUp,
    RESULT_CARD_MAX_ARTIFACTS, RESULT_CARD_MAX_FOLLOW_UPS,
};
pub use self::script_generation::{
    extract_current_app_recipe_from_script, generate_script_from_prompt,
    generate_script_from_prompt_with_receipt, generated_script_receipt_path,
    GeneratedScriptContractAudit, GeneratedScriptMetadataStyle, GeneratedScriptOutput,
    GeneratedScriptReceipt, GeneratedScriptVerificationReceipt, GeneratedScriptVerificationStatus,
    AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION, AI_SCRIPT_GENERATION_SYSTEM_PROMPT,
};
pub use self::sdk_handlers::try_handle_ai_message;
pub use self::storage::{
    clear_all_chats, create_chat, delete_chat, get_all_chats, get_chat, get_chat_messages,
    get_deleted_chats, get_last_message_preparation_audit, init_ai_db, insert_mock_data,
    save_message, save_message_preparation_audit, search_chats, update_chat_title,
};
pub use self::tab_context::{
    append_tab_ai_execution_receipt, append_tab_ai_execution_receipt_to_path,
    build_tab_ai_apply_back_hint_from_source, build_tab_ai_execution_receipt,
    build_tab_ai_experience_intents, build_tab_ai_experience_spec, build_tab_ai_suggested_intents,
    build_tab_ai_user_prompt, cleanup_tab_ai_temp_script, detect_tab_ai_source_type_from_prompt,
    read_tab_ai_memory_index, read_tab_ai_memory_index_from_path,
    recent_tab_ai_automations_for_bundle, recent_tab_ai_automations_for_bundle_from_path,
    resolve_tab_ai_memory_suggestions, resolve_tab_ai_memory_suggestions_from_path,
    resolve_tab_ai_memory_suggestions_with_outcome,
    resolve_tab_ai_memory_suggestions_with_outcome_from_path,
    resolve_tab_ai_prior_automations_for_entry,
    resolve_tab_ai_prior_automations_for_entry_from_path, should_offer_save,
    tab_ai_apply_back_footer_label, tab_ai_execution_audit_path, tab_ai_experience_pack_name,
    tab_ai_experience_pack_subtitle, tab_ai_intent_uses_implicit_target, tab_ai_memory_index_path,
    truncate_tab_ai_text, write_tab_ai_memory_entry, write_tab_ai_memory_entry_to_path,
    TabAiApplyBackHint, TabAiApplyBackRoute, TabAiClipboardContext, TabAiClipboardHistoryEntry,
    TabAiContextBlob, TabAiDegradationReason, TabAiExecutionReceipt, TabAiExecutionRecord,
    TabAiExecutionStatus, TabAiExperienceIntent, TabAiExperiencePack, TabAiExperienceSpec,
    TabAiFieldStatus, TabAiInvocationReceipt, TabAiMemoryEntry, TabAiMemoryResolution,
    TabAiMemoryResolutionOutcome, TabAiMemoryResolutionReason, TabAiMemorySuggestion,
    TabAiSourceType, TabAiSuggestedIntentSpec, TabAiTargetAudit, TabAiTargetContext,
    TabAiUiSnapshot, TAB_AI_CONTEXT_SCHEMA_VERSION, TAB_AI_EXECUTION_RECEIPT_SCHEMA_VERSION,
    TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION, TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION,
    TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION, TAB_AI_TARGET_AUDIT_SCHEMA_VERSION,
};

// Re-export context-composer types for integration tests
pub use self::context_contract::{
    context_attachment_specs, ContextAttachmentKind, ContextAttachmentSpec,
};
pub use self::context_mentions::{
    mention_range_at_cursor, parse_inline_context_mentions, InlineContextMention,
};
pub use self::context_selector::types::{
    ContextPortalKind, ContextPortalPrefixPayload, ContextSelectorRow, ContextSelectorRowKind,
    ContextSelectorTrigger, InlinePortalAttachment, InlinePortalResultPayload, SlashCommandPayload,
};
pub use self::context_selector::{
    context_selector_rows, score_builtin, score_builtin_with_trigger, slash_command_rows,
};
pub(crate) use self::explicit_target_handoff::request_explicit_agent_chat_handoff_from_secondary_window;

// ---------------------------------------------------------------------------
// Pending explicit Agent Chat target — cross-window handoff slot
// ---------------------------------------------------------------------------

use parking_lot::Mutex;
use std::sync::OnceLock;

/// Pending explicit Agent Chat target from a secondary window (e.g. detached actions
/// popup). The main window checks this after the secondary surface closes and
/// hands the target to `open_tab_ai_agent_chat_with_explicit_target`.
static PENDING_EXPLICIT_AGENT_CHAT_TARGET: OnceLock<Mutex<Option<TabAiTargetContext>>> =
    OnceLock::new();

/// Build a canonical Agent Chat handoff target for an action-menu selection.
pub(crate) fn build_action_target_for_ai(
    action: &crate::actions::Action,
    host_label: &str,
) -> TabAiTargetContext {
    TabAiTargetContext {
        source: "ActionsDialog".to_string(),
        kind: "action".to_string(),
        semantic_id: crate::protocol::generate_semantic_id("action", 0, &action.id),
        label: action.title.clone(),
        metadata: Some(serde_json::json!({
            "actionId": action.id,
            "title": action.title,
            "description": action.description,
            "category": format!("{:?}", action.category),
            "shortcut": action.shortcut,
            "host": host_label,
        })),
    }
}

/// Enqueue a canonical target for Agent Chat handoff from a secondary window.
///
/// The main window picks this up via `take_pending_explicit_agent_chat_target` the
/// next time it processes a close-actions-popup event.
pub fn enqueue_explicit_agent_chat_target(target: TabAiTargetContext) {
    let storage = PENDING_EXPLICIT_AGENT_CHAT_TARGET.get_or_init(|| Mutex::new(None));
    *storage.lock() = Some(target);
}

/// Take a pending Agent Chat handoff target, if any, clearing the slot.
pub fn take_pending_explicit_agent_chat_target() -> Option<TabAiTargetContext> {
    PENDING_EXPLICIT_AGENT_CHAT_TARGET
        .get()
        .and_then(|storage| storage.lock().take())
}

/// Build the canonical chip label used by explicit Agent Chat target handoffs.
///
/// Notes, actions, and any future secondary surfaces should use this instead
/// of formatting bespoke labels locally. The main-window Agent Chat entry also
/// delegates here so all chip labels share a single source of truth.
pub(crate) fn format_explicit_target_chip_label(target: &TabAiTargetContext) -> String {
    let prefix = match target.kind.as_str() {
        "file" => "File",
        "directory" => "Folder",
        "search_query" => "Search",
        "input" => "Input",
        "clipboard_entry" => "Clipboard",
        "script" | "scriptlet" | "builtin" => "Command",
        "window" => "Window",
        "app" => "App",
        "process" => "Process",
        "menu_command" => "Menu Command",
        "action" => "Action",
        "note" => "Note",
        "agent" => "Agent",
        "fallback" => "Suggestion",
        _ => "Selection",
    };
    format!("{}: {}", prefix, target.label)
}

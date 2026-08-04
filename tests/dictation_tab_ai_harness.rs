//! Contract tests verifying the dedicated dictation-to-AI-harness entry path.
//!
//! The `DictationToAiHarness` built-in routes through
//! `DictationBuiltinAction::AgentChat`, which forces
//! `DictationTarget::TabAiHarness` and validates it with
//! `ensure_dictation_delivery_target_available_for(target)`.

const BUILTIN_EXECUTION_SOURCE: &str = include_str!("../src/app_execute/builtin_execution.rs");
const BUILTINS_SOURCE: &str = include_str!("../src/builtins/mod.rs");
const DICTATION_TYPES_SOURCE: &str = include_str!("../src/dictation/types.rs");
const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");

// =========================================================================
// Built-in variant exists
// =========================================================================

#[test]
fn dictation_to_ai_harness_variant_exists_in_builtin_feature() {
    assert!(
        BUILTINS_SOURCE.contains("DictationToAiHarness"),
        "BuiltInFeature must include a DictationToAiHarness variant"
    );
}

#[test]
fn dictation_to_ai_harness_entry_registered() {
    assert!(
        BUILTINS_SOURCE.contains("builtin/dictation-to-ai"),
        "A built-in entry with id 'builtin-dictation-to-ai' must be registered"
    );
    assert!(
        BUILTINS_SOURCE.contains("Dictate to Agent Chat"),
        "The entry must have a user-facing name 'Dictate to Agent Chat'"
    );
}

// =========================================================================
// Target override wiring
// =========================================================================

/// The DictationToAiHarness handler must route through the named AgentChat
/// state, which forces `TabAiHarness`, not the generic current-surface state.
#[test]
fn handler_uses_target_override() {
    let handler_start = BUILTIN_EXECUTION_SOURCE
        .find("BuiltInFeature::DictationToAiHarness")
        .expect("DictationToAiHarness match arm must exist in builtin_execution.rs");
    let handler_body = &BUILTIN_EXECUTION_SOURCE[handler_start..];
    // Find the end of this match arm (next top-level BuiltInFeature:: arm)
    let next_arm = handler_body[1..]
        .find("builtins::BuiltInFeature::")
        .unwrap_or(handler_body.len());
    let handler_body = &handler_body[..next_arm];

    assert!(
        handler_body.contains("execute_dictation_builtin_action(DictationBuiltinAction::AgentChat"),
        "DictationToAiHarness handler must route through DictationBuiltinAction::AgentChat"
    );
    assert!(
        BUILTIN_EXECUTION_SOURCE
            .contains("Self::AgentChat => Some(crate::dictation::DictationTarget::TabAiHarness)"),
        "AgentChat action state must force DictationTarget::TabAiHarness"
    );
}

/// The handler must validate using the target-aware validator, not the
/// generic `ensure_dictation_delivery_target_available()`.
#[test]
fn handler_uses_target_aware_validation() {
    assert!(
        BUILTIN_EXECUTION_SOURCE.contains("fn ensure_dictation_builtin_target_available(")
            && BUILTIN_EXECUTION_SOURCE.contains("if let Some(target) = action.forced_target()")
            && BUILTIN_EXECUTION_SOURCE
                .contains("self.ensure_dictation_delivery_target_available_for(target)"),
        "forced dictation action states must use target-aware validation"
    );
}

// =========================================================================
// Target-aware validation accepts TabAiHarness without QuickTerminalView
// =========================================================================

/// The target-aware validation helper must accept `TabAiHarness` as a
/// valid target — it doesn't require any active view to deliver to.
#[test]
fn target_aware_validator_accepts_tab_ai_harness() {
    let fn_start = BUILTIN_EXECUTION_SOURCE
        .find("fn ensure_dictation_delivery_target_available_for")
        .expect("ensure_dictation_delivery_target_available_for must exist");
    let fn_body = &BUILTIN_EXECUTION_SOURCE[fn_start..];
    let fn_end = fn_body[1..]
        .find("\n    fn ")
        .or_else(|| fn_body[1..].find("\n    pub"))
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..fn_end];

    // TabAiHarness must be in the Ok(()) arm, not the ExternalApp arm.
    assert!(
        fn_body.contains("TabAiHarness => Ok(())"),
        "ensure_dictation_delivery_target_available_for must return Ok(()) for TabAiHarness"
    );
}

/// The older override helper still returns TabAiHarness for callers that use it.
#[test]
fn target_override_forces_tab_ai_harness() {
    let fn_start = BUILTIN_EXECUTION_SOURCE
        .find("fn resolve_dictation_target_with_override")
        .expect("resolve_dictation_target_with_override must exist");
    let fn_body = &BUILTIN_EXECUTION_SOURCE[fn_start..];
    let fn_end = fn_body[1..]
        .find("\n    fn ")
        .or_else(|| fn_body[1..].find("\n    pub"))
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..fn_end];

    assert!(
        fn_body.contains("DictationTarget::TabAiHarness"),
        "resolve_dictation_target_with_override must return TabAiHarness when forced"
    );
}

// =========================================================================
// DictationTarget enum has TabAiHarness
// =========================================================================

#[test]
fn dictation_target_enum_has_tab_ai_harness_variant() {
    assert!(
        DICTATION_TYPES_SOURCE.contains("TabAiHarness"),
        "DictationTarget must have a TabAiHarness variant"
    );
}

// =========================================================================
// Dictation delivery handler routes TabAiHarness to Agent Chat entry intent
// =========================================================================

/// When dictation finishes with target=TabAiHarness, the transcript must
/// open Agent Chat as the initial submitted prompt.
#[test]
fn dictation_transcript_delivery_routes_tab_ai_harness() {
    let fn_start = BUILTIN_EXECUTION_SOURCE
        .find("fn handle_dictation_transcript")
        .expect("handle_dictation_transcript must exist");
    let fn_body = &BUILTIN_EXECUTION_SOURCE[fn_start..];
    let fn_end = fn_body[1..]
        .find("\n    fn ")
        .or_else(|| fn_body[1..].find("\n    pub"))
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..fn_end];

    assert!(
        fn_body.contains("DictationTarget::TabAiHarness"),
        "handle_dictation_transcript must have a TabAiHarness arm"
    );
    assert!(
        fn_body.contains("send_dictation_to_agent_chat(transcript.clone(), cx)"),
        "TabAiHarness delivery must use explicit Send without inheriting launcher context"
    );
}

#[test]
fn tab_ai_harness_delivery_seeds_script_list_return_origin_before_open() {
    let fn_start = BUILTIN_EXECUTION_SOURCE
        .find("fn handle_dictation_transcript")
        .expect("handle_dictation_transcript must exist");
    let fn_body = &BUILTIN_EXECUTION_SOURCE[fn_start..];
    let arm_start = fn_body
        .find("DictationTarget::TabAiHarness => {")
        .expect("handle_dictation_transcript must have a TabAiHarness arm");
    let arm = &fn_body[arm_start..];
    let arm_end = arm
        .find("DictationTarget::ExternalApp")
        .expect("TabAiHarness arm must be followed by ExternalApp arm");
    let arm = &arm[..arm_end];

    let seed_idx = arm
        .find("self.seed_agent_chat_dictation_return_origin()")
        .expect("TabAiHarness delivery must seed its close return origin");
    let open_idx = arm
        .find("self.send_dictation_to_agent_chat")
        .expect("TabAiHarness delivery must open embedded Agent Chat");
    let finish_idx = arm
        .find("WindowEvent::FinishDictation")
        .expect("TabAiHarness delivery must reveal/focus Agent Chat through the orchestrator");

    assert!(
        seed_idx < open_idx && open_idx < finish_idx,
        "TabAiHarness delivery must seed ScriptList/MainFilter return origin before opening Agent Chat, then finish dictation"
    );
}

#[test]
fn tab_ai_harness_delivery_closes_detached_agent_chat_before_embedded_open() {
    let fn_start = BUILTIN_EXECUTION_SOURCE
        .find("fn handle_dictation_transcript")
        .expect("handle_dictation_transcript must exist");
    let fn_body = &BUILTIN_EXECUTION_SOURCE[fn_start..];
    let arm_start = fn_body
        .find("DictationTarget::TabAiHarness => {")
        .expect("handle_dictation_transcript must have a TabAiHarness arm");
    let arm = &fn_body[arm_start..];
    let arm_end = arm
        .find("DictationTarget::ExternalApp")
        .expect("TabAiHarness arm must be followed by ExternalApp arm");
    let arm = &arm[..arm_end];

    let guard_idx = arm
        .find("crate::ai::agent_chat::ui::chat_window::is_chat_window_open()")
        .expect("TabAiHarness delivery must check for an already-detached Agent Chat chat");
    let close_idx = arm
        .find("crate::ai::agent_chat::ui::chat_window::close_chat_window(&mut **cx)")
        .expect("TabAiHarness delivery must close detached Agent Chat before embedded reveal");
    let open_idx = arm
        .find("self.send_dictation_to_agent_chat")
        .expect("TabAiHarness delivery must open embedded Agent Chat");
    let finish_idx = arm
        .find("WindowEvent::FinishDictation")
        .expect("TabAiHarness delivery must reveal/focus Agent Chat through the orchestrator");

    assert!(
        guard_idx < close_idx && close_idx < open_idx && open_idx < finish_idx,
        "Agent Chat dictation must close detached Agent Chat before opening the embedded chat, then let the orchestrator reveal/focus main"
    );
}

#[test]
fn dictation_return_origin_helper_targets_script_list_main_filter() {
    let helper_start = TAB_AI_MODE_SOURCE
        .find("pub(crate) fn seed_agent_chat_dictation_return_origin")
        .expect("seed_agent_chat_dictation_return_origin must exist");
    let helper = &TAB_AI_MODE_SOURCE[helper_start..];
    let helper_end = helper
        .find("\n    fn tab_ai_return_focus_target_for_view")
        .expect("helper should live next to the return-focus helpers");
    let helper = &helper[..helper_end];

    assert!(
        helper.contains("self.tab_ai_harness_return_view = Some(AppView::ScriptList)")
            && helper.contains(
                "self.tab_ai_harness_return_focus_target = Some(FocusTarget::MainFilter)"
            )
            && helper.contains("self.tab_ai_harness_script_list_trigger = None"),
        "Agent Chat dictation must close back to ScriptList/MainFilter and clear stale launcher trigger state"
    );
}

// =========================================================================
// Stop edge defaults to TabAiHarness (not ExternalApp) for this handler
// =========================================================================

#[test]
fn stop_edge_defaults_to_tab_ai_harness() {
    assert!(
        BUILTIN_EXECUTION_SOURCE
            .contains("fn stop_fallback_target(self) -> crate::dictation::DictationTarget")
            && BUILTIN_EXECUTION_SOURCE.contains("self.forced_target()")
            && BUILTIN_EXECUTION_SOURCE.contains(
                "Self::AgentChat => Some(crate::dictation::DictationTarget::TabAiHarness)"
            ),
        "AgentChat stop edge must default through its forced TabAiHarness target"
    );
}

#[test]
fn dictation_to_ai_empty_capture_aborts_without_opening_agent_chat() {
    let helper_start = BUILTIN_EXECUTION_SOURCE
        .find("fn execute_dictation_builtin_action(")
        .expect("execute_dictation_builtin_action must exist");
    let helper_body = &BUILTIN_EXECUTION_SOURCE[helper_start..];
    let next_fn = helper_body[1..]
        .find("\n    fn ")
        .unwrap_or(helper_body.len());
    let helper_body = &helper_body[..next_fn];

    let stopped_none_start = helper_body
        .find("Ok(crate::dictation::DictationToggleOutcome::Stopped(None))")
        .expect("dictation action helper must handle Stopped(None)");
    let stopped_none_tail = &helper_body[stopped_none_start..];
    let err_arm_offset = stopped_none_tail
        .find("Err(error) =>")
        .expect("Stopped(None) arm must be followed by the error arm");
    let stopped_none_arm = &stopped_none_tail[..err_arm_offset];

    assert!(
        stopped_none_arm.contains("WindowEvent::AbortDictation"),
        "DictationToAiHarness Stopped(None) must abort the overlay session"
    );
    assert!(
        !stopped_none_arm.contains("WindowEvent::FinishDictation"),
        "DictationToAiHarness Stopped(None) must not finish because no transcript exists"
    );
    assert!(
        !stopped_none_arm.contains("open_tab_ai_agent_chat_with_entry_intent"),
        "DictationToAiHarness Stopped(None) must not open Agent Chat without a transcript"
    );
}

// =========================================================================
// Model download preflight shared with generic Dictation
// =========================================================================

#[test]
fn harness_dictation_checks_model_availability() {
    let helper_start = BUILTIN_EXECUTION_SOURCE
        .find("fn prepare_dictation_builtin_start(")
        .expect("prepare_dictation_builtin_start must exist");
    let helper_body = &BUILTIN_EXECUTION_SOURCE[helper_start..];
    let next_fn = helper_body[1..]
        .find("\n    fn ")
        .unwrap_or(helper_body.len());
    let helper_body = &helper_body[..next_fn];

    assert!(
        helper_body.contains("is_parakeet_model_available()"),
        "DictationToAiHarness must check Parakeet model availability"
    );
    assert!(
        helper_body.contains("open_dictation_model_prompt(cx)"),
        "DictationToAiHarness must open the model download prompt when model is missing"
    );
}

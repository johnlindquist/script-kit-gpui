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

fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .expect("function signature must exist");
    let open = start
        + source[start..]
            .find('{')
            .expect("function body must have an opening brace");
    let mut depth = 0usize;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("function body must have a closing brace");
}

fn compact(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

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
    let fn_body = function_body(
        BUILTIN_EXECUTION_SOURCE,
        "fn ensure_dictation_delivery_target_available_for",
    );

    // TabAiHarness must be in the internal-target Ok(()) arm, not the
    // ExternalApp arm. Compacting keeps the assertion stable across rustfmt.
    assert!(
        compact(fn_body).contains("crate::dictation::DictationTarget::TabAiHarness|crate::dictation::DictationTarget::DayPageToday|crate::dictation::DictationTarget::QuickAiQuestion=>Ok(())"),
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
        fn_body.contains("dispatch_dictation_to_frozen_agent_chat"),
        "TabAiHarness delivery must use the frozen thread policy and suppress launcher context"
    );
}

#[test]
fn tab_ai_harness_delivery_distinguishes_existing_and_fresh_policy() {
    let fn_body = function_body(BUILTIN_EXECUTION_SOURCE, "fn handle_dictation_transcript");
    let arm_start = fn_body
        .find("DictationTarget::TabAiHarness => {")
        .expect("handle_dictation_transcript must have a TabAiHarness arm");
    let arm = &fn_body[arm_start..];
    let arm_end = arm
        .find("DictationTarget::DayPageToday")
        .expect("TabAiHarness arm must be followed by Day Page");
    let arm = &arm[..arm_end];

    assert!(
        arm.contains("FrozenAgentChatPolicy::ExistingThread")
            && arm.contains("dispatch_dictation_to_frozen_agent_chat")
            && arm.contains("false")
            && arm.contains("true"),
        "existing-thread delivery must stay on that thread while fresh delivery creates a new embedded thread"
    );
}

#[test]
fn tab_ai_harness_delivery_preserves_detached_agent_chat() {
    let fn_body = function_body(BUILTIN_EXECUTION_SOURCE, "fn handle_dictation_transcript");
    let arm_start = fn_body
        .find("DictationTarget::TabAiHarness => {")
        .expect("handle_dictation_transcript must have a TabAiHarness arm");
    let arm = &fn_body[arm_start..];
    let arm_end = arm
        .find("DictationTarget::DayPageToday")
        .expect("TabAiHarness arm must be followed by Day Page");
    let arm = &arm[..arm_end];

    assert!(
        !arm.contains("close_chat_window") && !arm.contains("is_chat_window_open"),
        "Agent Chat dictation must preserve the independent detached workspace"
    );
    assert!(
        arm.contains("dispatch_dictation_to_frozen_agent_chat"),
        "Agent Chat dictation must route through the exact existing/fresh host policy"
    );
}

#[test]
fn dictation_return_origin_helper_targets_script_list_main_filter() {
    let helper = function_body(
        TAB_AI_MODE_SOURCE,
        "pub(crate) fn seed_agent_chat_dictation_return_origin",
    );

    assert!(
        helper.contains("self.seed_agent_chat_return_origin_for_view(&AppView::ScriptList, cx)")
            && helper.contains("self.tab_ai_harness_script_list_trigger = None")
            && helper.contains("return_focus_target = \"MainFilter\""),
        "Agent Chat dictation must use the shared ScriptList/MainFilter return owner and clear stale launcher trigger state"
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
    let helper_body = function_body(BUILTIN_EXECUTION_SOURCE, "fn handle_dictation_transcript");
    let empty_start = helper_body
        .find("Ok(None) => {")
        .expect("transcript delivery must handle an empty capture");
    let empty_tail = &helper_body[empty_start..];
    let error_offset = empty_tail
        .find("Err(error) =>")
        .expect("empty-capture arm must be followed by the error arm");
    let empty_arm = &empty_tail[..error_offset];

    assert!(
        empty_arm.contains("WindowEvent::AbortDictation"),
        "an empty Agent Chat capture must abort the overlay session"
    );
    assert!(
        !empty_arm.contains("WindowEvent::FinishDictation")
            && !empty_arm.contains("send_dictation_to_agent_chat")
            && !empty_arm.contains("open_tab_ai_agent_chat_with_entry_intent"),
        "an empty Agent Chat capture must not reveal or seed Agent Chat"
    );
}

// =========================================================================
// Model download preflight shared with generic Dictation
// =========================================================================

#[test]
fn harness_dictation_checks_model_availability() {
    let helper_body = function_body(
        BUILTIN_EXECUTION_SOURCE,
        "fn prepare_dictation_builtin_start(",
    );

    assert!(
        helper_body.contains("DictationModelId::from_preference")
            && helper_body.contains("dictation_model_entry(model_id)")
            && helper_body.contains("if !model.is_available()"),
        "DictationToAiHarness must check the currently selected Dictation model"
    );
    assert!(
        helper_body.contains("open_dictation_model_prompt(cx)"),
        "DictationToAiHarness must open the model download prompt when model is missing"
    );
}

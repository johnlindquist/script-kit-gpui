//! Source-audit tests for the SDK fields() prompt contract.
//!
//! fields() intentionally reuses the FormPrompt renderer and focus model, but
//! it must keep array-by-order SDK response semantics and must not fall back to
//! the old coming-soon path.

use super::{function_body, read_source as read};

const FORM_PROMPT_PATH: &str = "src/form_prompt.rs";
const PROMPT_HANDLER_PATH: &str = "src/prompt_handler/mod.rs";
const PROMPT_MESSAGE_ROUTE_PATH: &str = "src/prompt_handler/message_route.rs";
const COLLECT_ELEMENTS_PATH: &str = "src/app_layout/collect_elements.rs";
const STDIN_SIMULATE_KEY_PATH: &str = "src/main_entry/runtime_stdin_match_simulate_key.rs";
const SIMULATE_KEY_DISPATCH_PATH: &str = "src/app_impl/simulate_key_dispatch.rs";

#[test]
fn fields_protocol_routes_to_real_form_prompt_state() {
    let route = read(PROMPT_MESSAGE_ROUTE_PATH);
    let conversion = function_body(&route, "convert_protocol_prompt_message")
        .expect("convert_protocol_prompt_message implementation must exist");
    let fields_arm = conversion
        .split("Message::Fields")
        .nth(1)
        .and_then(|rest| rest.split("Message::Term").next())
        .expect("convert_protocol_prompt_message must include a fields arm before term");
    assert!(
        fields_arm.contains("PromptMessage::ShowFields"),
        "Message::Fields must route every conversion source to ShowFields"
    );
    for forbidden in [
        "FieldsComingSoon",
        "PromptConversionSource::InteractiveSession",
        "PromptConversionSource::Sessionless",
    ] {
        assert!(
            !fields_arm.contains(forbidden),
            "Message::Fields must not split conversion sources through {forbidden}"
        );
    }

    let handler = read(PROMPT_HANDLER_PATH);
    assert!(
        handler.contains("FormPromptState::from_fields"),
        "ShowFields must reuse FormPromptState through a fields constructor"
    );
    assert!(
        handler.contains("self.current_view = AppView::FormPrompt"),
        "ShowFields must install the real form prompt app view"
    );
    assert!(
        handler.contains("self.pending_focus = Some(FocusTarget::FormPrompt)"),
        "ShowFields must keep keyboard focus inside the prompt surface"
    );
}

#[test]
fn fields_prompt_collects_array_values_by_definition_order() {
    let form_prompt = read(FORM_PROMPT_PATH);

    for required in [
        "FormPromptOutputMode::ArrayByOrder",
        "pub fn from_fields",
        "\"fields\"",
        "serde_json::to_string(&values)",
    ] {
        assert!(
            form_prompt.contains(required),
            "FormPromptState must preserve fields() array semantics: {required}"
        );
    }

    let collect_values = form_prompt
        .split("pub fn collect_values")
        .nth(1)
        .and_then(|rest| rest.split("/// Focus the next field").next())
        .expect("collect_values implementation must exist");
    assert!(
        collect_values.contains("FormPromptOutputMode::ObjectByName"),
        "form() must keep object-by-name output"
    );
    assert!(
        collect_values.contains("FormPromptOutputMode::ArrayByOrder"),
        "fields() must collect array-by-order output"
    );
}

#[test]
fn fields_prompt_is_visible_to_state_and_elements() {
    let handler = read(PROMPT_HANDLER_PATH);
    let elements = read(COLLECT_ELEMENTS_PATH);

    assert!(
        handler.contains("entity.read(cx).prompt_type().to_string()"),
        "getState/current prompt type must report fields when a fields prompt is active"
    );
    assert!(
        handler.contains("prompt.set_input(text.to_string(), cx)"),
        "batch.setInput must be able to mutate the focused form/fields input"
    );
    assert!(
        handler.contains("focus_field_by_semantic_id"),
        "batch.selectBySemanticId must be able to focus form/fields inputs"
    );
    assert!(
        elements.contains("form.semantic_prefix()"),
        "getElements must derive form/fields semantic ids from the prompt source"
    );
    assert!(
        elements.contains("format!(\"{semantic_prefix}-fields\")"),
        "fields prompt element collection must expose fields-fields list identity"
    );
}

#[test]
fn fields_prompt_has_simulate_key_submit_cancel_navigation() {
    let stdin = read(STDIN_SIMULATE_KEY_PATH);
    assert!(
        stdin.contains("view.dispatch_simulate_key("),
        "stdin simulateKey entry point must delegate to shared dispatch_simulate_key helper"
    );

    let dispatch = read(SIMULATE_KEY_DISPATCH_PATH);
    let form_arm = dispatch
        .split("AppView::FormPrompt")
        .nth(1)
        .and_then(|rest| rest.split("AppView::EditorPrompt").next())
        .expect("simulateKey dispatch must have a FormPrompt arm before EditorPrompt");

    for required in [
        "submit_validation_message",
        "collect_values",
        "submit_prompt_response",
        "cancel_script_execution",
        "focus_next",
        "focus_previous",
        "stdin_simulate_key_form_prompt",
    ] {
        assert!(
            form_arm.contains(required),
            "FormPrompt simulateKey arm must cover submit/cancel/focus/actions: {required}"
        );
    }
}

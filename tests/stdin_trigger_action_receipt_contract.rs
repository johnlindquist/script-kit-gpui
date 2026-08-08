use script_kit_gpui::protocol::Message;

const APP_RUN_SETUP: &str = include_str!("../src/main_entry/app_run_setup.rs");
const PROTOCOL_SYSTEM_CONTROL: &str =
    include_str!("../src/protocol/message/variants/system_control.rs");
const PROTOCOL_GENERAL_CONSTRUCTORS: &str =
    include_str!("../src/protocol/message/constructors/general.rs");

#[test]
fn trigger_action_result_is_request_correlated_and_redacted() {
    let message = Message::trigger_action_result(
        "act-1".to_string(),
        "focused-text-action-expand".to_string(),
        Some("AgentChat".to_string()),
        true,
        false,
        None,
        Some(serde_json::json!({
            "semanticId": "footer-action:actions",
            "accessibilityRole": "AXButton",
            "actionSelector": "actionsFooterAction:",
            "dispatched": true
        })),
    );

    assert_eq!(message.request_id(), Some("act-1"));

    let json = serde_json::to_value(&message).expect("serialize triggerAction result");
    assert_eq!(json["type"], "triggerActionResult");
    assert_eq!(json["requestId"], "act-1");
    assert_eq!(json["actionId"], "focused-text-action-expand");
    assert_eq!(json["host"], "AgentChat");
    assert_eq!(json["ok"], true);
    assert_eq!(json["popupClosed"], false);
    assert_eq!(
        json["nativeFooterActivation"]["actionSelector"],
        "actionsFooterAction:"
    );
    assert_eq!(json["nativeFooterActivation"]["dispatched"], true);

    let raw = json.to_string();
    for forbidden in [
        "capturedText",
        "assistantOutput",
        "clipboardText",
        "promptXml",
        "Confidential zeta phrase",
    ] {
        assert!(
            !raw.contains(forbidden),
            "triggerAction result must stay redacted and omit `{forbidden}`"
        );
    }
}

#[test]
fn trigger_action_dispatcher_emits_request_correlated_receipt() {
    for required in [
        "ExternalCommand::TriggerAction",
        "request_id",
        "Message::trigger_action_result",
        "receipt_host",
        "receipt_ok",
        "popup_closed",
        "Some(\"no_host\".to_string())",
        "view.response_sender",
    ] {
        assert!(
            APP_RUN_SETUP.contains(required),
            "TriggerAction dispatcher must include receipt contract fragment `{required}`"
        );
    }

    for required in [
        "TriggerActionResult",
        "#[serde(rename = \"triggerActionResult\")]",
        "#[serde(rename = \"requestId\")]",
        "#[serde(rename = \"actionId\")]",
        "popup_closed",
        "error_code",
    ] {
        assert!(
            PROTOCOL_SYSTEM_CONTROL.contains(required)
                || PROTOCOL_GENERAL_CONSTRUCTORS.contains(required),
            "protocol must include triggerActionResult fragment `{required}`"
        );
    }
}

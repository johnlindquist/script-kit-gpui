use serde_json::{json, Value};

fn to_json(message: crate::protocol::Message) -> Value {
    serde_json::to_value(message).expect("Should serialize protocol message")
}

#[test]
fn test_state_result_empty_state_omits_optional_fields() {
    let response = crate::protocol::Message::state_result(
        "state-empty".to_string(),
        "none".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        "".to_string(),
        0,
        0,
        -1,
        None,
        false,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "stateResult",
            "requestId": "state-empty",
            "promptType": "none",
            "inputValue": "",
            "choiceCount": 0,
            "visibleChoiceCount": 0,
            "selectedIndex": -1,
            "isFocused": false,
            "windowVisible": false
        }),
        "empty stateResult must omit every optional field"
    );
}

#[test]
fn test_state_result_minimal_state_shape() {
    let response = crate::protocol::Message::state_result(
        "state-min".to_string(),
        "scriptList".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        "kit".to_string(),
        12,
        4,
        0,
        None,
        true,
        true,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "stateResult",
            "requestId": "state-min",
            "promptType": "scriptList",
            "inputValue": "kit",
            "choiceCount": 12,
            "visibleChoiceCount": 4,
            "selectedIndex": 0,
            "isFocused": true,
            "windowVisible": true
        }),
        "minimal stateResult JSON shape changed"
    );
}

#[test]
fn test_state_result_representative_with_optional_fields() {
    let response = crate::protocol::Message::state_result(
        "state-1".to_string(),
        "arg".to_string(),
        Some("prompt-123".to_string()),
        None,
        None,
        None,
        Some(json!({
            "owner": "main",
            "route": "submit",
            "doubleSubmitGuard": true
        })),
        Some("Search apps".to_string()),
        "app".to_string(),
        5,
        3,
        1,
        Some("application".to_string()),
        true,
        true,
        Some(json!({ "visible": true, "mode": "draft" })),
        Some(json!({ "attached": true, "targetId": "agent_chat-chat" })),
        Some(json!([{ "start": 0, "end": 3, "kind": "token" }])),
        Some(json!({
            "canonicalFilterText": "app",
            "computedFilterText": "app",
            "rawVisualInputValue": "app",
            "pendingFilterSync": false
        })),
        None,
        None,
        Some(json!({
            "selectedSemanticId": "choice:1:application",
            "willSubmit": true
        })),
        Some(json!({
            "open": true,
            "actions": [{ "id": "copy", "label": "Copy" }]
        })),
        Some(json!({ "provider": "root", "query": "app", "loading": false })),
        Some(json!({ "offset": 12, "viewportHeight": 320, "contentHeight": 900 })),
        Some(json!({
            "surface": "script_list",
            "implementation": "variable_list",
            "selectedIndex": 1
        })),
        Some("tab-ai-2026-05-25.png".to_string()),
        Some(json!({
            "count": 2,
            "files": [
                { "name": "a.txt", "size": 12 },
                { "name": "b.txt", "size": 34 }
            ]
        })),
        Some(json!({ "loading": false, "selected": "README.md" })),
        Some(json!({ "visible": true, "lineCount": 4 })),
        None,
        Some(json!({ "recording": false, "device": "default" })),
        None,
        None,
        None,
    );
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "stateResult",
            "requestId": "state-1",
            "promptType": "arg",
            "promptId": "prompt-123",
            "submitDiagnostics": {
                "owner": "main",
                "route": "submit",
                "doubleSubmitGuard": true
            },
            "placeholder": "Search apps",
            "inputValue": "app",
            "choiceCount": 5,
            "visibleChoiceCount": 3,
            "selectedIndex": 1,
            "selectedValue": "application",
            "isFocused": true,
            "windowVisible": true,
            "miniAi": { "visible": true, "mode": "draft" },
            "focusedTextAgentChat": { "attached": true, "targetId": "agent_chat-chat" },
            "filterInputDecorations": [{ "start": 0, "end": 3, "kind": "token" }],
            "filterInputDiagnostics": {
                "canonicalFilterText": "app",
                "computedFilterText": "app",
                "rawVisualInputValue": "app",
                "pendingFilterSync": false
            },
            "mainWindowPreflight": {
                "selectedSemanticId": "choice:1:application",
                "willSubmit": true
            },
            "actionsDialog": {
                "open": true,
                "actions": [{ "id": "copy", "label": "Copy" }]
            },
            "rootFileSearch": { "provider": "root", "query": "app", "loading": false },
            "mainListScroll": { "offset": 12, "viewportHeight": 320, "contentHeight": 900 },
            "activeListScroll": {
                "surface": "script_list",
                "implementation": "variable_list",
                "selectedIndex": 1
            },
            "screenshotIdentity": "tab-ai-2026-05-25.png",
            "drop": {
                "count": 2,
                "files": [
                    { "name": "a.txt", "size": 12 },
                    { "name": "b.txt", "size": 34 }
                ]
            },
            "path": { "loading": false, "selected": "README.md" },
            "notes": { "visible": true, "lineCount": 4 },
            "dictation": { "recording": false, "device": "default" }
        }),
        "stateResult representative JSON shape changed"
    );
}

#[test]
fn test_screenshot_result_response_shape() {
    let response = crate::protocol::Message::screenshot_result(
        "shot-1".to_string(),
        "iVBORw0KGgo=".to_string(),
        800,
        600,
    );
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "screenshotResult",
            "requestId": "shot-1",
            "data": "iVBORw0KGgo=",
            "width": 800,
            "height": 600
        }),
        "screenshotResult success JSON shape changed"
    );
}

#[test]
fn test_screenshot_error_response_shape() {
    let response = crate::protocol::Message::screenshot_error(
        "shot-err".to_string(),
        "window not found".to_string(),
    );
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "screenshotResult",
            "requestId": "shot-err",
            "data": "",
            "width": 0,
            "height": 0,
            "error": "window not found"
        }),
        "screenshotResult error JSON shape changed"
    );
}

#[test]
fn test_hello_ack_response_shape() {
    let response = crate::protocol::Message::hello_ack(
        1,
        vec![
            "submitJson".to_string(),
            "semanticIdV2".to_string(),
            "forwardCompat".to_string(),
        ],
    );
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "helloAck",
            "protocol": 1,
            "capabilities": ["submitJson", "semanticIdV2", "forwardCompat"]
        }),
        "helloAck JSON shape changed"
    );
}

#[test]
fn test_file_search_result_empty_response_shape() {
    let response = crate::protocol::Message::file_search_result("fs-1".to_string(), Vec::new());
    let actual = to_json(response);
    assert_eq!(
        actual,
        json!({
            "type": "fileSearchResult",
            "requestId": "fs-1",
            "files": []
        }),
        "empty fileSearchResult JSON shape changed"
    );
}

use crate::main_window_preflight::types::{
    MainWindowPreflightAction, MainWindowPreflightActionKind, MainWindowPreflightReceipt,
    MainWindowPreflightResultRole,
};

fn receipt_with_action(
    filter_text: &str,
    selected_index: usize,
    enter_action: MainWindowPreflightAction,
    tab_action: Option<MainWindowPreflightAction>,
    warnings: Vec<String>,
) -> MainWindowPreflightReceipt {
    MainWindowPreflightReceipt {
        filter_text: filter_text.to_string(),
        computed_search_text: filter_text.to_string(),
        source_filters: Vec::new(),
        filter_indicators: Vec::new(),
        selected_index,
        selected_result_key: None,
        selected_result_role: Some(MainWindowPreflightResultRole::Primary),
        selected_command: None,
        visible_results: Vec::new(),
        visible_result_key_fingerprint: String::new(),
        visible_row_fingerprint: String::new(),
        visible_result_count: 0,
        root_passive_frame: None,
        enter_action: Some(enter_action),
        tab_action,
        warnings,
    }
}

#[test]
fn receipt_serializes_with_camel_case_keys() {
    let receipt = receipt_with_action(
        "resize images",
        2,
        MainWindowPreflightAction {
            kind: MainWindowPreflightActionKind::RunScript,
            label: "Run Script".to_string(),
            subject: "Resize Images".to_string(),
            type_label: "Script".to_string(),
            source_name: Some("kit".to_string()),
            description: Some("Batch resize images".to_string()),
        },
        None,
        vec![],
    );

    let value = serde_json::to_value(&receipt).expect("receipt should serialize");
    assert!(
        value.get("filterText").is_some(),
        "expected camelCase filterText key"
    );
    assert!(
        value.get("selectedIndex").is_some(),
        "expected camelCase selectedIndex key"
    );
    assert!(
        value.get("enterAction").is_some(),
        "expected camelCase enterAction key"
    );
}

#[test]
fn ask_ai_kind_serializes_as_camel_case_enum() {
    let value =
        serde_json::to_value(MainWindowPreflightActionKind::AskAi).expect("enum should serialize");
    assert_eq!(value, serde_json::Value::String("askAi".to_string()));
}

#[test]
fn all_action_kinds_round_trip_to_camel_case() {
    let cases = vec![
        (MainWindowPreflightActionKind::RunScript, "runScript"),
        (MainWindowPreflightActionKind::RunSnippet, "runSnippet"),
        (MainWindowPreflightActionKind::RunCommand, "runCommand"),
        (MainWindowPreflightActionKind::LaunchApp, "launchApp"),
        (MainWindowPreflightActionKind::SwitchWindow, "switchWindow"),
        (MainWindowPreflightActionKind::RunAgent, "runAgent"),
        (MainWindowPreflightActionKind::RunFallback, "runFallback"),
        (MainWindowPreflightActionKind::AskAi, "askAi"),
        (
            MainWindowPreflightActionKind::CopyCalculator,
            "copyCalculator",
        ),
        (
            MainWindowPreflightActionKind::PasteResumeCommand,
            "pasteResumeCommand",
        ),
    ];

    for (kind, expected) in cases {
        let value = serde_json::to_value(&kind).expect("should serialize");
        assert_eq!(
            value,
            serde_json::Value::String(expected.to_string()),
            "ActionKind {:?} should serialize to \"{}\"",
            kind,
            expected
        );
    }
}

#[test]
fn receipt_with_tab_action_serializes_correctly() {
    let receipt = receipt_with_action(
        "hello",
        0,
        MainWindowPreflightAction {
            kind: MainWindowPreflightActionKind::RunScript,
            label: "Run Script".to_string(),
            subject: "Hello World".to_string(),
            type_label: "Script".to_string(),
            source_name: None,
            description: None,
        },
        Some(MainWindowPreflightAction {
            kind: MainWindowPreflightActionKind::AskAi,
            label: "Ask AI".to_string(),
            subject: "hello".to_string(),
            type_label: "AI".to_string(),
            source_name: None,
            description: Some("Opens the AI window".to_string()),
        }),
        vec![],
    );

    let value = serde_json::to_value(&receipt).expect("should serialize");
    let tab = value.get("tabAction").expect("tabAction should be present");
    assert_eq!(
        tab.get("kind").and_then(|v| v.as_str()),
        Some("askAi"),
        "tab action kind should be askAi"
    );
    assert_eq!(
        tab.get("typeLabel").and_then(|v| v.as_str()),
        Some("AI"),
        "tab action typeLabel should be AI"
    );
}

#[test]
fn receipt_with_warnings_serializes_correctly() {
    let receipt = receipt_with_action(
        "",
        0,
        MainWindowPreflightAction {
            kind: MainWindowPreflightActionKind::RunAgent,
            label: "Run Agent".to_string(),
            subject: "Test Agent".to_string(),
            type_label: "Agent".to_string(),
            source_name: None,
            description: None,
        },
        None,
        vec![
            "Agent execution is not fully implemented.".to_string(),
            "Command+Enter Agent Chat is inactive.".to_string(),
        ],
    );

    let value = serde_json::to_value(&receipt).expect("should serialize");
    let warnings = value
        .get("warnings")
        .and_then(|v| v.as_array())
        .expect("warnings should be an array");
    assert_eq!(warnings.len(), 2);
}

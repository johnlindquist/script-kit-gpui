//! Contract tests verifying that wait/batch error, trace, and SDK-reference
//! contracts are normalized to the repo's structured diagnostics pattern.

use script_kit_gpui::protocol::transaction_executor::{
    execute_batch, execute_wait_for, TransactionStateProvider,
};
use script_kit_gpui::protocol::{
    BatchCommand, BatchOptions, Message, StateMatchSpec, TransactionError, TransactionErrorCode,
    TransactionTraceMode, UiStateSnapshot, WaitCondition, WaitDetailedCondition,
};

// ---------------------------------------------------------------------------
// Mock state provider
// ---------------------------------------------------------------------------

struct MockProvider {
    input_value: String,
    selected_value: Option<String>,
    window_visible: bool,
    window_focused: bool,
    choice_count: usize,
    visible_semantic_ids: Vec<String>,
    focused_semantic_id: Option<String>,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self {
            input_value: String::new(),
            selected_value: None,
            window_visible: true,
            window_focused: true,
            choice_count: 3,
            visible_semantic_ids: vec![
                "choice:0:alpha".to_string(),
                "choice:1:beta".to_string(),
                "choice:2:gamma".to_string(),
            ],
            focused_semantic_id: Some("choice:0:alpha".to_string()),
        }
    }
}

impl TransactionStateProvider for MockProvider {
    fn snapshot(&self) -> UiStateSnapshot {
        UiStateSnapshot {
            window_visible: self.window_visible,
            window_focused: self.window_focused,
            input_value: Some(self.input_value.clone()),
            selected_value: self.selected_value.clone(),
            focused_semantic_id: self.focused_semantic_id.clone(),
            visible_semantic_ids: self.visible_semantic_ids.clone(),
            choice_count: self.choice_count,
            ..Default::default()
        }
    }

    fn set_input(&mut self, text: &str) -> anyhow::Result<()> {
        self.input_value = text.to_string();
        Ok(())
    }

    fn select_by_value(&mut self, value: &str, _submit: bool) -> anyhow::Result<Option<String>> {
        if self
            .visible_semantic_ids
            .iter()
            .any(|id| id.contains(value))
        {
            self.selected_value = Some(value.to_string());
            Ok(Some(value.to_string()))
        } else {
            Ok(None)
        }
    }

    fn select_by_semantic_id(
        &mut self,
        semantic_id: &str,
        _submit: bool,
    ) -> anyhow::Result<Option<String>> {
        if self.visible_semantic_ids.contains(&semantic_id.to_string()) {
            self.selected_value = Some(semantic_id.to_string());
            Ok(Some(semantic_id.to_string()))
        } else {
            Ok(None)
        }
    }
}

// ============================================================
// TransactionErrorCode — stable wire codes
// ============================================================

#[test]
fn error_code_wait_condition_timeout_serializes_to_snake_case() {
    let code = TransactionErrorCode::WaitConditionTimeout;
    let json = serde_json::to_value(&code).expect("serialize");
    assert_eq!(json, "wait_condition_timeout");
}

#[test]
fn error_code_element_not_found_serializes_to_snake_case() {
    let code = TransactionErrorCode::ElementNotFound;
    let json = serde_json::to_value(&code).expect("serialize");
    assert_eq!(json, "element_not_found");
}

#[test]
fn error_code_unsupported_prompt_serializes_to_snake_case() {
    let code = TransactionErrorCode::UnsupportedPrompt;
    let json = serde_json::to_value(&code).expect("serialize");
    assert_eq!(json, "unsupported_prompt");
}

#[test]
fn error_code_selection_not_found_serializes_to_snake_case() {
    let code = TransactionErrorCode::SelectionNotFound;
    let json = serde_json::to_value(&code).expect("serialize");
    assert_eq!(json, "selection_not_found");
}

#[test]
fn error_code_unsupported_command_serializes_to_snake_case() {
    let code = TransactionErrorCode::UnsupportedCommand;
    let json = serde_json::to_value(&code).expect("serialize");
    assert_eq!(json, "unsupported_command");
}

#[test]
fn error_code_action_failed_serializes_to_snake_case() {
    let code = TransactionErrorCode::ActionFailed;
    let json = serde_json::to_value(&code).expect("serialize");
    assert_eq!(json, "action_failed");
}

#[test]
fn all_error_codes_round_trip() {
    let codes = vec![
        TransactionErrorCode::WaitConditionTimeout,
        TransactionErrorCode::ElementNotFound,
        TransactionErrorCode::SelectionNotFound,
        TransactionErrorCode::InvalidCondition,
        TransactionErrorCode::UnsupportedCommand,
        TransactionErrorCode::UnsupportedPrompt,
        TransactionErrorCode::ActionFailed,
    ];
    for code in codes {
        let json = serde_json::to_value(&code).expect("serialize");
        let back: TransactionErrorCode = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, code);
    }
}

// ============================================================
// TransactionError helper constructors
// ============================================================

#[test]
fn error_helper_element_not_found_has_correct_shape() {
    let err = TransactionError::element_not_found("choice:99:missing");
    assert_eq!(err.code, TransactionErrorCode::ElementNotFound);
    assert!(err.message.contains("choice:99:missing"));
    assert!(err.suggestion.is_some());
}

#[test]
fn error_helper_unsupported_prompt_has_correct_shape() {
    let err = TransactionError::unsupported_prompt("div prompt does not support selection");
    assert_eq!(err.code, TransactionErrorCode::UnsupportedPrompt);
    assert!(err.message.contains("div prompt"));
    assert!(err.suggestion.is_some());
}

#[test]
fn error_helper_wait_timeout_has_correct_shape() {
    let err = TransactionError::wait_timeout("Timeout after 5000ms");
    assert_eq!(err.code, TransactionErrorCode::WaitConditionTimeout);
    assert!(err.suggestion.is_none());
}

// ============================================================
// Batch trace stays at message top level, not inside BatchOptions
// ============================================================

#[test]
fn batch_message_trace_is_top_level() {
    let json = serde_json::json!({
        "type": "batch",
        "requestId": "b-trace",
        "commands": [{"type": "setInput", "text": "x"}],
        "options": {"stopOnError": true, "timeout": 1000},
        "trace": "onFailure"
    });
    let msg: Message = serde_json::from_value(json).expect("parse batch with top-level trace");
    match msg {
        Message::Batch { trace, options, .. } => {
            assert_eq!(trace, TransactionTraceMode::OnFailure);
            // options should NOT contain trace
            let opts_json = serde_json::to_value(&options).expect("serialize options");
            assert!(
                opts_json.get("trace").is_none(),
                "BatchOptions must not contain trace"
            );
        }
        other => panic!("Expected Batch, got: {other:?}"),
    }
}

#[test]
fn batch_options_serializes_only_execution_fields() {
    let opts = BatchOptions {
        stop_on_error: true,
        rollback_on_error: false,
        timeout: 3000,
    };
    let json = serde_json::to_value(&opts).expect("serialize");
    assert!(json.get("stopOnError").is_some());
    assert!(json.get("timeout").is_some());
    assert!(
        json.get("trace").is_none(),
        "trace must not be in BatchOptions"
    );
}

// ============================================================
// waitFor — success path produces correct result shape
// ============================================================

#[test]
fn wait_for_success_has_no_error_and_no_trace_when_off() {
    let mut provider = MockProvider::default();
    provider.input_value = "match".to_string();

    let output = execute_wait_for(
        &mut provider,
        "wf-ok".to_string(),
        &WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
            state: StateMatchSpec {
                input_value: Some("match".to_string()),
                ..Default::default()
            },
        }),
        Some(100),
        Some(10),
        TransactionTraceMode::Off,
    )
    .expect("execute");

    assert!(output.success);
    assert!(output.error.is_none());
    assert!(output.trace.is_none());
}

// ============================================================
// waitFor — failure path produces structured error with code
// ============================================================

#[test]
fn wait_for_timeout_returns_structured_error_with_code() {
    let mut provider = MockProvider::default();
    provider.input_value = "wrong".to_string();

    let output = execute_wait_for(
        &mut provider,
        "wf-fail".to_string(),
        &WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
            state: StateMatchSpec {
                input_value: Some("never".to_string()),
                ..Default::default()
            },
        }),
        Some(30),
        Some(10),
        TransactionTraceMode::Off,
    )
    .expect("execute");

    assert!(!output.success);
    let err = output.error.expect("error on timeout");
    assert_eq!(err.code, TransactionErrorCode::WaitConditionTimeout);
    assert!(!err.message.is_empty());

    // Verify wire shape
    let json = serde_json::to_value(&err).expect("serialize");
    assert_eq!(json["code"], "wait_condition_timeout");
    assert!(json["message"].is_string());
}

// ============================================================
// waitFor — trace-on-failure includes trace only on failure
// ============================================================

#[test]
fn wait_for_trace_on_failure_includes_trace_on_timeout() {
    let mut provider = MockProvider::default();

    let output = execute_wait_for(
        &mut provider,
        "wf-trace".to_string(),
        &WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
            state: StateMatchSpec {
                input_value: Some("never".to_string()),
                ..Default::default()
            },
        }),
        Some(30),
        Some(10),
        TransactionTraceMode::OnFailure,
    )
    .expect("execute");

    assert!(!output.success);
    assert!(output.trace.is_some(), "trace should be present on failure");
}

#[test]
fn wait_for_trace_on_failure_omits_trace_on_success() {
    let mut provider = MockProvider::default();
    provider.input_value = "match".to_string();

    let output = execute_wait_for(
        &mut provider,
        "wf-trace-ok".to_string(),
        &WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
            state: StateMatchSpec {
                input_value: Some("match".to_string()),
                ..Default::default()
            },
        }),
        Some(100),
        Some(10),
        TransactionTraceMode::OnFailure,
    )
    .expect("execute");

    assert!(output.success);
    assert!(
        output.trace.is_none(),
        "trace should be absent on success with onFailure mode"
    );
}

// ============================================================
// batch — failure includes structured error with code
// ============================================================

#[test]
fn batch_selection_failure_returns_structured_error() {
    let mut provider = MockProvider::default();

    let commands = vec![BatchCommand::SelectByValue {
        value: "nonexistent".to_string(),
        submit: false,
    }];

    let output = execute_batch(
        &mut provider,
        "b-sel-fail".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("execute");

    assert!(!output.success);
    assert_eq!(output.failed_at, Some(0));
    let err = output.results[0]
        .error
        .as_ref()
        .expect("error on selection failure");
    assert_eq!(err.code, TransactionErrorCode::SelectionNotFound);
}

#[test]
fn batch_unsupported_command_returns_structured_error() {
    let mut provider = MockProvider::default();

    let commands = vec![BatchCommand::TypeAndSubmit {
        text: "hello".to_string(),
    }];

    let output = execute_batch(
        &mut provider,
        "b-unsup".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("execute");

    assert!(!output.success);
    let err = output.results[0].error.as_ref().expect("error");
    assert_eq!(err.code, TransactionErrorCode::UnsupportedCommand);
}

// ============================================================
// batch — trace-on-failure behavior
// ============================================================

#[test]
fn batch_trace_on_failure_includes_trace_and_commands() {
    let mut provider = MockProvider::default();

    let commands = vec![BatchCommand::WaitFor {
        condition: WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
            state: StateMatchSpec {
                input_value: Some("never".to_string()),
                ..Default::default()
            },
        }),
        timeout: Some(30),
        poll_interval: Some(10),
    }];

    let output = execute_batch(
        &mut provider,
        "b-trace-fail".to_string(),
        &commands,
        None,
        TransactionTraceMode::OnFailure,
    )
    .expect("execute");

    assert!(!output.success);
    let trace = output.trace.expect("trace on failure");
    assert_eq!(trace.request_id, "b-trace-fail");
    assert!(!trace.commands.is_empty());
    assert!(trace.failed_at.is_some());
}

#[test]
fn batch_trace_on_failure_omits_trace_on_success() {
    let mut provider = MockProvider::default();

    let commands = vec![BatchCommand::SetInput {
        text: "ok".to_string(),
    }];

    let output = execute_batch(
        &mut provider,
        "b-trace-ok".to_string(),
        &commands,
        None,
        TransactionTraceMode::OnFailure,
    )
    .expect("execute");

    assert!(output.success);
    assert!(
        output.trace.is_none(),
        "trace should be absent on success with onFailure mode"
    );
}

// ============================================================
// batch — success path shape
// ============================================================

#[test]
fn batch_success_has_correct_result_shape() {
    let mut provider = MockProvider::default();

    let commands = vec![
        BatchCommand::SetInput {
            text: "test".to_string(),
        },
        BatchCommand::WaitFor {
            condition: WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
                state: StateMatchSpec {
                    input_value: Some("test".to_string()),
                    ..Default::default()
                },
            }),
            timeout: Some(100),
            poll_interval: Some(10),
        },
    ];

    let output = execute_batch(
        &mut provider,
        "b-shape".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("execute");

    assert!(output.success);
    assert_eq!(output.results.len(), 2);
    assert!(output.failed_at.is_none());
    assert!(output.total_elapsed < 1000);

    // Each result entry has the expected fields
    assert_eq!(output.results[0].command, "setInput");
    assert!(output.results[0].elapsed.is_some());
    assert!(output.results[0].error.is_none());

    assert_eq!(output.results[1].command, "waitFor");
    assert!(output.results[1].elapsed.is_some());
    assert!(output.results[1].error.is_none());
}

// ============================================================
// kit://sdk-reference — descriptions match wire contract
// ============================================================

#[test]
fn sdk_reference_wait_for_description_documents_error_codes() {
    let content =
        script_kit_gpui::mcp_resources::read_resource("kit://sdk-reference", &[], &[], None)
            .expect("should resolve");
    let doc: serde_json::Value = serde_json::from_str(&content.text).expect("parse JSON");

    let functions = doc["functions"].as_array().expect("functions array");
    let wait_for = functions
        .iter()
        .find(|f| f["name"] == "waitFor")
        .expect("waitFor in sdk-reference");

    let desc = wait_for["description"]
        .as_str()
        .expect("description string");
    assert!(
        desc.contains("wait_condition_timeout"),
        "waitFor description must document wait_condition_timeout code"
    );
    assert!(
        desc.contains("element_not_found"),
        "waitFor description must document element_not_found code"
    );
    assert!(
        desc.contains("unsupported_prompt"),
        "waitFor description must document unsupported_prompt code"
    );
    assert!(
        desc.contains("trace"),
        "waitFor description must mention trace"
    );
}

#[test]
fn sdk_reference_batch_description_documents_error_codes_and_trace() {
    let content =
        script_kit_gpui::mcp_resources::read_resource("kit://sdk-reference", &[], &[], None)
            .expect("should resolve");
    let doc: serde_json::Value = serde_json::from_str(&content.text).expect("parse JSON");

    let functions = doc["functions"].as_array().expect("functions array");
    let batch = functions
        .iter()
        .find(|f| f["name"] == "batch")
        .expect("batch in sdk-reference");

    let desc = batch["description"].as_str().expect("description string");
    assert!(
        desc.contains("failedAt"),
        "batch description must document failedAt"
    );
    assert!(
        desc.contains("totalElapsed"),
        "batch description must document totalElapsed"
    );
    assert!(
        desc.contains("trace"),
        "batch description must mention trace"
    );
    assert!(
        desc.contains("not inside options"),
        "batch description must clarify trace is top-level, not inside options"
    );
    assert!(
        desc.contains("action_failed"),
        "batch description must document action_failed code"
    );
}

// ============================================================
// inspectAutomationWindow — schema-versioned response contracts
// ============================================================

use script_kit_gpui::protocol::{
    AutomationInspectSnapshot, AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind,
    AutomationWindowTarget, InspectBoundsInScreenshot, InspectPoint, SuggestedHitPoint,
    AUTOMATION_INSPECT_SCHEMA_VERSION,
};

use std::sync::atomic::{AtomicU32, Ordering};
static INSPECT_COUNTER: AtomicU32 = AtomicU32::new(80_000);
fn inspect_prefix() -> String {
    let n = INSPECT_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("insp{n}")
}

fn make_window(
    prefix: &str,
    id: &str,
    kind: AutomationWindowKind,
    bounds: Option<AutomationWindowBounds>,
) -> AutomationWindowInfo {
    AutomationWindowInfo {
        id: format!("{prefix}:{id}"),
        kind,
        title: Some(format!("Window {id}")),
        focused: false,
        visible: true,
        semantic_surface: None,
        bounds,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    }
}

fn inspect_cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

/// Schema version must be the current constant (v3).
#[test]
fn inspect_schema_version_is_current() {
    assert_eq!(
        AUTOMATION_INSPECT_SCHEMA_VERSION, 3,
        "Schema version must be 3"
    );
}

/// Popup inspect response must include geometry fields when bounds exist.
#[test]
fn inspect_popup_response_has_geometry_fields() {
    let p = inspect_prefix();

    let main = make_window(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(AutomationWindowBounds {
            x: 100.0,
            y: 50.0,
            width: 800.0,
            height: 600.0,
        }),
    );
    let actions = make_window(
        &p,
        "actions",
        AutomationWindowKind::ActionsDialog,
        Some(AutomationWindowBounds {
            x: 300.0,
            y: 200.0,
            width: 520.0,
            height: 384.0,
        }),
    );
    script_kit_gpui::windows::upsert_automation_window(main);
    script_kit_gpui::windows::upsert_automation_window(actions);

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:actions"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let target_bounds = script_kit_gpui::protocol::target_bounds_in_screenshot(&resolved)
        .expect("must compute target bounds");
    let hit_point = script_kit_gpui::protocol::default_surface_hit_point(&target_bounds);
    let suggested =
        script_kit_gpui::protocol::default_suggested_hit_points(&resolved, Some(&target_bounds));

    // Build a response-shaped snapshot as the handler would
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: resolved.id.clone(),
        window_kind: format!("{:?}", resolved.kind),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: resolved.title.clone(),
        resolved_bounds: resolved.bounds.clone(),
        target_bounds_in_screenshot: Some(target_bounds.clone()),
        surface_hit_point: Some(hit_point.clone()),
        suggested_hit_points: suggested.clone(),
        elements: Vec::new(),
        total_count: 0,
        focused_semantic_id: None,
        selected_semantic_id: None,
        screenshot_width: Some(800),
        screenshot_height: Some(600),
        pixel_probes: Vec::new(),
        os_window_id: None,
        semantic_quality: Some(script_kit_gpui::protocol::SemanticQuality::PanelOnly),
        warnings: vec!["panel_only_actions_dialog".to_string()],
        pid: None,
    };

    // Serialize and verify JSON contract
    let json = serde_json::to_value(&snapshot).expect("serialize");

    assert_eq!(json["schemaVersion"], 3);
    assert_eq!(json["windowKind"], "ActionsDialog");
    assert!(json["targetBoundsInScreenshot"].is_object());
    assert!(json["surfaceHitPoint"].is_object());
    assert!(json["suggestedHitPoints"].is_array());
    assert!(!json["suggestedHitPoints"].as_array().unwrap().is_empty());

    // Target bounds must be offset from main
    let tb = &json["targetBoundsInScreenshot"];
    assert_eq!(tb["x"], 200.0); // 300 - 100
    assert_eq!(tb["y"], 150.0); // 200 - 50
    assert_eq!(tb["width"], 520.0);
    assert_eq!(tb["height"], 384.0);

    // Suggested hit point semantic ID must match the surface kind
    let shp = &json["suggestedHitPoints"][0];
    assert_eq!(shp["semanticId"], "panel:actions-dialog");
    assert_eq!(shp["reason"], "surface_center");

    inspect_cleanup(&p, &["main", "actions"]);
}

/// Detached window inspect response must have origin at (0, 0).
#[test]
fn inspect_detached_response_has_origin_bounds() {
    let p = inspect_prefix();

    let notes = make_window(
        &p,
        "notes",
        AutomationWindowKind::Notes,
        Some(AutomationWindowBounds {
            x: 500.0,
            y: 300.0,
            width: 350.0,
            height: 280.0,
        }),
    );
    script_kit_gpui::windows::upsert_automation_window(notes);

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:notes"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let target_bounds =
        script_kit_gpui::protocol::target_bounds_in_screenshot(&resolved).expect("must compute");

    assert!(
        (target_bounds.x - 0.0).abs() < f64::EPSILON,
        "Detached window bounds must start at x=0"
    );
    assert!(
        (target_bounds.y - 0.0).abs() < f64::EPSILON,
        "Detached window bounds must start at y=0"
    );
    assert!(
        (target_bounds.width - 350.0).abs() < f64::EPSILON,
        "Width must match resolved bounds"
    );
    assert!(
        (target_bounds.height - 280.0).abs() < f64::EPSILON,
        "Height must match resolved bounds"
    );

    inspect_cleanup(&p, &["notes"]);
}

/// AgentChatDetached inspect response must use the correct semantic ID.
#[test]
fn inspect_agent_chat_detached_suggested_hit_uses_correct_semantic_id() {
    let p = inspect_prefix();

    let agent_chat = make_window(
        &p,
        "agent_chat",
        AutomationWindowKind::AgentChatDetached,
        Some(AutomationWindowBounds {
            x: 200.0,
            y: 100.0,
            width: 480.0,
            height: 440.0,
        }),
    );
    script_kit_gpui::windows::upsert_automation_window(agent_chat);

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:agent_chat"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let target_bounds =
        script_kit_gpui::protocol::target_bounds_in_screenshot(&resolved).expect("must compute");
    let suggested =
        script_kit_gpui::protocol::default_suggested_hit_points(&resolved, Some(&target_bounds));

    assert_eq!(suggested.len(), 1);
    assert_eq!(suggested[0].semantic_id, "input:agent_chat-composer");
    assert_eq!(suggested[0].reason, "surface_center");
    // Center of (0, 0, 480, 440) = (240, 220)
    assert!(
        (suggested[0].x - 240.0).abs() < f64::EPSILON,
        "Hit x should be 240, got {}",
        suggested[0].x
    );
    assert!(
        (suggested[0].y - 220.0).abs() < f64::EPSILON,
        "Hit y should be 220, got {}",
        suggested[0].y
    );

    inspect_cleanup(&p, &["agent_chat"]);
}

/// Missing bounds must produce empty suggested hit points (fail closed).
#[test]
fn inspect_no_bounds_produces_no_suggested_hits() {
    let info = AutomationWindowInfo {
        id: "test:no-bounds".to_string(),
        kind: AutomationWindowKind::Notes,
        title: Some("Notes".to_string()),
        focused: false,
        visible: true,
        semantic_surface: None,
        bounds: None,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };

    let target_bounds = script_kit_gpui::protocol::target_bounds_in_screenshot(&info);
    assert!(target_bounds.is_none(), "No bounds → None");

    let suggested = script_kit_gpui::protocol::default_suggested_hit_points(&info, None);
    assert!(suggested.is_empty(), "No bounds → no suggested hit points");
}

/// Error snapshot for failed target resolution must have empty geometry.
#[test]
fn inspect_error_snapshot_has_empty_geometry() {
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: String::new(),
        window_kind: "unknown".to_string(),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: None,
        resolved_bounds: None,
        target_bounds_in_screenshot: None,
        surface_hit_point: None,
        suggested_hit_points: Vec::new(),
        elements: Vec::new(),
        total_count: 0,
        focused_semantic_id: None,
        selected_semantic_id: None,
        screenshot_width: None,
        screenshot_height: None,
        pixel_probes: Vec::new(),
        os_window_id: None,
        semantic_quality: Some(script_kit_gpui::protocol::SemanticQuality::Unavailable),
        warnings: vec!["target_resolution_failed: no such window".to_string()],
        pid: None,
    };

    let json = serde_json::to_value(&snapshot).expect("serialize");

    // Geometry fields must be absent (skip_serializing_if)
    assert!(json.get("targetBoundsInScreenshot").is_none());
    assert!(json.get("surfaceHitPoint").is_none());
    assert!(json.get("resolvedBounds").is_none());
    assert!(json["suggestedHitPoints"].as_array().is_none()); // empty vec is skipped

    // Warning must be present
    let warnings = json["warnings"].as_array().expect("warnings array");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]
        .as_str()
        .unwrap()
        .starts_with("target_resolution_failed"));
}

/// Inspect snapshot round-trips correctly with all v2 geometry fields populated.
#[test]
fn inspect_snapshot_v2_geometry_round_trip() {
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: "actionsDialog:main".to_string(),
        window_kind: "ActionsDialog".to_string(),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: Some("Actions".to_string()),
        resolved_bounds: Some(AutomationWindowBounds {
            x: 620.0,
            y: 242.0,
            width: 520.0,
            height: 384.0,
        }),
        target_bounds_in_screenshot: Some(InspectBoundsInScreenshot {
            x: 380.0,
            y: 118.0,
            width: 520.0,
            height: 384.0,
        }),
        surface_hit_point: Some(InspectPoint { x: 640.0, y: 310.0 }),
        suggested_hit_points: vec![SuggestedHitPoint {
            semantic_id: "panel:actions-dialog".to_string(),
            x: 640.0,
            y: 310.0,
            reason: "surface_center".to_string(),
        }],
        elements: Vec::new(),
        total_count: 0,
        focused_semantic_id: Some("panel:actions-dialog".to_string()),
        selected_semantic_id: None,
        screenshot_width: Some(1280),
        screenshot_height: Some(820),
        pixel_probes: Vec::new(),
        os_window_id: None,
        semantic_quality: Some(script_kit_gpui::protocol::SemanticQuality::Full),
        warnings: Vec::new(),
        pid: None,
    };

    let json_str = serde_json::to_string(&snapshot).expect("serialize");
    let parsed: AutomationInspectSnapshot = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(parsed, snapshot, "v3 snapshot must round-trip exactly");

    // Verify camelCase wire names
    assert!(json_str.contains("targetBoundsInScreenshot"));
    assert!(json_str.contains("surfaceHitPoint"));
    assert!(json_str.contains("suggestedHitPoints"));
    assert!(json_str.contains("resolvedBounds"));
}

/// Each window kind gets the correct suggested hit point semantic ID.
#[test]
fn inspect_suggested_hit_semantic_ids_per_kind() {
    let expected = vec![
        (AutomationWindowKind::ActionsDialog, "panel:actions-dialog"),
        (AutomationWindowKind::PromptPopup, "panel:prompt-popup"),
        (AutomationWindowKind::Notes, "input:notes-editor"),
        (
            AutomationWindowKind::AgentChatDetached,
            "input:agent_chat-composer",
        ),
        (AutomationWindowKind::Main, "panel:window"),
    ];

    let bounds = InspectBoundsInScreenshot {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 300.0,
    };

    for (kind, expected_id) in expected {
        let info = AutomationWindowInfo {
            id: format!("{kind:?}:test"),
            kind,
            title: None,
            focused: false,
            visible: true,
            semantic_surface: None,
            bounds: Some(AutomationWindowBounds {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 300.0,
            }),
            parent_window_id: None,
            parent_kind: None,
            pid: None,
            generation: None,
        };

        let hits = script_kit_gpui::protocol::default_suggested_hit_points(&info, Some(&bounds));
        assert_eq!(
            hits.len(),
            1,
            "{kind:?} should have exactly one suggested hit"
        );
        assert_eq!(
            hits[0].semantic_id, expected_id,
            "{kind:?} semantic_id mismatch"
        );
    }
}

// ============================================================
// ActionsDialog batch mutation — structured log markers
// ============================================================

#[test]
fn prompt_handler_emits_batch_actions_dialog_log() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("automation.batch.actions_dialog.completed"),
        "batch handler must emit ActionsDialog completion log"
    );
}

#[test]
fn prompt_handler_emits_actions_dialog_target_resolution_log() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("automation.target.actions_dialog_resolved"),
        "resolve_automation_read_target must emit ActionsDialog resolution log"
    );
}

#[test]
fn actions_dialog_transaction_provider_emits_set_input_log() {
    let source = include_str!("../../src/windows/automation_transaction_provider.rs");
    assert!(
        source.contains("transaction_actions_dialog_set_input"),
        "ActionsDialogTransactionProvider::set_input must emit structured log"
    );
}

#[test]
fn actions_dialog_transaction_provider_emits_select_log() {
    let source = include_str!("../../src/windows/automation_transaction_provider.rs");
    assert!(
        source.contains("transaction_actions_dialog_select_by_value"),
        "ActionsDialogTransactionProvider::select_by_value must emit structured log"
    );
}

#[test]
fn surface_collector_routes_actions_dialog() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("AutomationWindowKind::ActionsDialog => {"),
        "surface collector must route ActionsDialog targets"
    );
}

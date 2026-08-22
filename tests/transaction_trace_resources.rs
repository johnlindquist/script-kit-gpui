//! Integration tests for transaction trace MCP resources.
//!
//! Validates that `kit://transactions/latest` and `kit://transactions/schema`
//! are registered, resolve correctly, and reject malformed URIs with actionable errors.

use script_kit_gpui::mcp_resources::{get_resource_definitions, read_resource};
use script_kit_gpui::protocol::transaction_trace::append_transaction_trace;
use script_kit_gpui::protocol::{TransactionTrace, TransactionTraceStatus};
use std::sync::Arc;
use tempfile::tempdir;

// ── Helpers ──────────────────────────────────────────────────────────────

fn empty_scripts() -> Vec<Arc<script_kit_gpui::scripts::Script>> {
    Vec::new()
}

fn empty_scriptlets() -> Vec<Arc<script_kit_gpui::scripts::Scriptlet>> {
    Vec::new()
}

// ── Resource listing ─────────────────────────────────────────────────────

#[test]
fn transaction_resources_are_listed_in_definitions() {
    let resources = get_resource_definitions();
    assert!(
        resources
            .iter()
            .any(|r| r.uri == "kit://transactions/latest"),
        "kit://transactions/latest should be in resource definitions"
    );
    assert!(
        resources
            .iter()
            .any(|r| r.uri == "kit://transactions/schema"),
        "kit://transactions/schema should be in resource definitions"
    );
}

// ── Schema resource ──────────────────────────────────────────────────────

#[test]
fn schema_resource_returns_valid_json_with_version() {
    let content = read_resource(
        "kit://transactions/schema",
        &empty_scripts(),
        &empty_scriptlets(),
        None,
    )
    .expect("schema should resolve");

    assert_eq!(content.mime_type, "application/json");
    assert_eq!(content.uri, "kit://transactions/schema");

    let value: serde_json::Value =
        serde_json::from_str(&content.text).expect("schema should be valid JSON");
    assert_eq!(value["kind"], "transaction_trace_schema");
    assert_eq!(value["version"], 1);
    assert!(value["traceModes"].is_array());
    assert!(!value["traceModes"].as_array().unwrap().is_empty());
    assert!(value["examples"].is_array());
    assert!(!value["examples"].as_array().unwrap().is_empty());
}

#[test]
fn schema_trace_modes_match_documented_values() {
    let content = read_resource(
        "kit://transactions/schema",
        &empty_scripts(),
        &empty_scriptlets(),
        None,
    )
    .expect("schema should resolve");

    let value: serde_json::Value = serde_json::from_str(&content.text).unwrap();
    let modes: Vec<String> = value["traceModes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    assert!(modes.contains(&"off".to_string()));
    assert!(modes.contains(&"on".to_string()));
    assert!(modes.contains(&"onFailure".to_string()));
}

// ── Latest resource (empty state) ────────────────────────────────────────

#[test]
fn latest_resource_resolves_to_valid_json() {
    let content = read_resource(
        "kit://transactions/latest",
        &empty_scripts(),
        &empty_scriptlets(),
        None,
    )
    .expect("latest should resolve");

    assert_eq!(content.mime_type, "application/json");
    assert_eq!(content.uri, "kit://transactions/latest");

    let value: serde_json::Value =
        serde_json::from_str(&content.text).expect("should be valid JSON");

    // Either an empty payload or a real trace — both are valid
    let is_empty = value.get("status").and_then(|v| v.as_str()) == Some("empty");
    let is_trace = value.get("requestId").is_some();
    assert!(
        is_empty || is_trace,
        "expected either empty payload or trace, got: {}",
        &content.text[..content.text.len().min(200)]
    );
}

// ── Latest resource with requestId filter ────────────────────────────────

#[test]
fn latest_resource_accepts_request_id_param() {
    // Even without matching traces, should still resolve (empty payload)
    let content = read_resource(
        "kit://transactions/latest?requestId=nonexistent",
        &empty_scripts(),
        &empty_scriptlets(),
        None,
    )
    .expect("latest with requestId should resolve");

    let value: serde_json::Value =
        serde_json::from_str(&content.text).expect("should be valid JSON");
    assert_eq!(value["status"], "empty");
}

// ── Trace roundtrip through resource ─────────────────────────────────────

#[test]
fn latest_resource_reads_persisted_trace() {
    let dir = tempdir().expect("temp dir");
    let log_path = dir.path().join("transactions.jsonl");

    let trace = TransactionTrace {
        schema_version: 1,
        request_id: "resource-test-1".to_string(),
        command_fingerprint: "fp-resource-test-1".to_string(),
        status: TransactionTraceStatus::Ok,
        started_at_ms: 1000,
        total_elapsed_ms: 42,
        failed_at: None,
        commands: Vec::new(),
    };

    append_transaction_trace(Some(&log_path), &trace).expect("append should succeed");

    // Read directly via the transaction_trace module since read_resource
    // uses the default log path. This validates the persistence layer.
    let loaded = script_kit_gpui::protocol::transaction_trace::read_latest_transaction_trace(
        Some(&log_path),
        Some("resource-test-1"),
    )
    .expect("read should succeed")
    .expect("trace should exist");

    assert_eq!(loaded.request_id, "resource-test-1");
    assert_eq!(loaded.status, TransactionTraceStatus::Ok);
    assert_eq!(loaded.total_elapsed_ms, 42);
}

#[test]
fn latest_resource_filters_by_request_id() {
    let dir = tempdir().expect("temp dir");
    let log_path = dir.path().join("transactions.jsonl");

    let trace_a = TransactionTrace {
        schema_version: 1,
        request_id: "txn-a".to_string(),
        command_fingerprint: "fp-txn-a".to_string(),
        status: TransactionTraceStatus::Ok,
        started_at_ms: 1000,
        total_elapsed_ms: 10,
        failed_at: None,
        commands: Vec::new(),
    };
    let trace_b = TransactionTrace {
        schema_version: 1,
        request_id: "txn-b".to_string(),
        command_fingerprint: "fp-txn-b".to_string(),
        status: TransactionTraceStatus::Failed,
        started_at_ms: 2000,
        total_elapsed_ms: 20,
        failed_at: Some(1),
        commands: Vec::new(),
    };

    append_transaction_trace(Some(&log_path), &trace_a).expect("append a");
    append_transaction_trace(Some(&log_path), &trace_b).expect("append b");

    // Without filter, should get the latest (txn-b)
    let latest = script_kit_gpui::protocol::transaction_trace::read_latest_transaction_trace(
        Some(&log_path),
        None,
    )
    .expect("read should succeed")
    .expect("trace should exist");
    assert_eq!(latest.request_id, "txn-b");

    // With filter, should get the specific one (txn-a)
    let filtered = script_kit_gpui::protocol::transaction_trace::read_latest_transaction_trace(
        Some(&log_path),
        Some("txn-a"),
    )
    .expect("read should succeed")
    .expect("trace should exist");
    assert_eq!(filtered.request_id, "txn-a");
}

// ── Error handling ───────────────────────────────────────────────────────

#[test]
fn malformed_transaction_uri_returns_actionable_error() {
    let err = read_resource(
        "kit://transactions/other",
        &empty_scripts(),
        &empty_scriptlets(),
        None,
    )
    .expect_err("should reject unknown transaction path");
    assert!(
        err.contains("Resource not found"),
        "error should indicate resource not found: {err}"
    );
}

#[test]
fn unknown_query_param_on_latest_returns_actionable_error() {
    let err = read_resource(
        "kit://transactions/latest?badparam=1",
        &empty_scripts(),
        &empty_scriptlets(),
        None,
    )
    .expect_err("should reject unknown query parameter");
    assert!(
        err.contains("Unknown query parameter"),
        "error should mention unknown parameter: {err}"
    );
    assert!(
        err.contains("requestId"),
        "error should suggest valid parameter: {err}"
    );
}

// ── Context snapshot tests still pass ────────────────────────────────────

#[test]
fn context_resource_still_listed_after_transaction_resources_added() {
    let resources = get_resource_definitions();
    assert!(
        resources.iter().any(|r| r.uri == "kit://context"),
        "kit://context should still be in resource definitions"
    );
    assert!(
        resources.iter().any(|r| r.uri == "kit://context/schema"),
        "kit://context/schema should still be in resource definitions"
    );
}

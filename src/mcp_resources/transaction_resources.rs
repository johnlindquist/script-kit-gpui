//! MCP resources for transaction flight recorder traces.
//!
//! Exposes `kit://transactions/latest` and `kit://transactions/schema` as
//! machine-readable MCP resources so agents can inspect the most recent
//! transaction execution without tailing log files manually.

use super::{McpResource, ResourceContent};
use crate::protocol::transaction_trace::{
    read_latest_transaction_trace, sanitize_transaction_trace,
};
use serde::Serialize;

/// Self-describing schema document for the transaction trace resource.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransactionSchemaDocument {
    kind: &'static str,
    version: u32,
    trace_modes: Vec<&'static str>,
    examples: Vec<serde_json::Value>,
}

/// Returns MCP resource definitions for transaction traces.
pub fn transaction_resource_definitions() -> Vec<McpResource> {
    vec![
        McpResource {
            uri: "kit://transactions/latest".to_string(),
            name: "Latest Transaction Trace".to_string(),
            description: Some(
                "Privacy-redacted waitFor/batch trace with timings, fingerprinted user values and semantic IDs, and typed actionable failure summaries. Supports ?requestId=<id> to filter by receipt."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://transactions/schema".to_string(),
            name: "Transaction Trace Schema".to_string(),
            description: Some(
                "Self-describing schema and examples for transaction trace resources."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
    ]
}

/// Returns `true` if the URI is a transaction resource URI.
pub fn is_transaction_resource_uri(uri: &str) -> bool {
    uri == "kit://transactions/schema"
        || uri == "kit://transactions/latest"
        || uri.starts_with("kit://transactions/latest?")
}

/// Read a transaction resource by URI.
///
/// # Supported URIs
///
/// - `kit://transactions/schema` — self-describing schema document
/// - `kit://transactions/latest` — most recent persisted trace
/// - `kit://transactions/latest?requestId=<id>` — trace filtered by request ID
///
/// Returns an actionable error for malformed transaction URIs.
pub fn read_transaction_resource(uri: &str) -> Result<ResourceContent, String> {
    tracing::info!(
        target: "script_kit::transaction",
        uri = uri,
        "transaction_resource_read_start"
    );

    if uri == "kit://transactions/schema" {
        return read_schema_resource(uri);
    }

    if uri == "kit://transactions/latest" || uri.starts_with("kit://transactions/latest?") {
        return read_latest_resource(uri);
    }

    Err(format!(
        "Unknown transaction resource URI: {uri}. Valid URIs: kit://transactions/latest, kit://transactions/latest?requestId=<id>, kit://transactions/schema"
    ))
}

fn read_schema_resource(uri: &str) -> Result<ResourceContent, String> {
    let doc = TransactionSchemaDocument {
        kind: "transaction_trace_schema",
        version: 1,
        trace_modes: vec!["off", "on", "onFailure"],
        examples: vec![
            serde_json::json!({
                "type": "waitFor",
                "requestId": "wait-1",
                "condition": "choicesRendered",
                "trace": "onFailure"
            }),
            serde_json::json!({
                "type": "batch",
                "requestId": "txn-1",
                "trace": "onFailure",
                "commands": [
                    {"type": "setInput", "text": "apple"},
                    {"type": "waitFor", "condition": "choicesRendered", "timeout": 1000},
                    {"type": "selectByValue", "value": "apple", "submit": true}
                ]
            }),
        ],
    };

    let text = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize transaction schema: {e}"))?;

    tracing::info!(
        target: "script_kit::transaction",
        uri = uri,
        "transaction_resource_read_complete"
    );

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "application/json".to_string(),
        text,
    })
}

fn read_latest_resource(uri: &str) -> Result<ResourceContent, String> {
    let request_id = parse_request_id_param(uri);

    // Validate query parameters — reject malformed URIs with actionable errors
    if let Some(query) = uri.split_once('?').map(|(_, q)| q) {
        for param in query.split('&') {
            let key = param.split_once('=').map(|(k, _)| k).unwrap_or(param);
            if key != "requestId" {
                return Err(format!(
                    "Unknown query parameter '{key}' on kit://transactions/latest. Supported: ?requestId=<id>"
                ));
            }
        }
    }

    let trace = read_latest_transaction_trace(None, request_id.as_deref())
        .map_err(|e| format!("Failed to read latest transaction trace: {e}"))?;

    let text = serialize_transaction_resource_trace(trace.as_ref())?;

    tracing::info!(
        target: "script_kit::transaction",
        uri = uri,
        found = trace.is_some(),
        request_id_filter = ?request_id,
        "transaction_resource_read_complete"
    );

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "application/json".to_string(),
        text,
    })
}

fn serialize_transaction_resource_trace(
    trace: Option<&crate::protocol::TransactionTrace>,
) -> Result<String, String> {
    let text = match trace {
        Some(t) => serde_json::to_string_pretty(&sanitize_transaction_trace(t))
            .map_err(|e| format!("Failed to serialize transaction trace: {e}"))?,
        None => serde_json::to_string_pretty(&serde_json::json!({
            "kind": "transactionTrace",
            "status": "empty",
            "message": "No transaction traces found"
        }))
        .map_err(|e| format!("Failed to serialize empty payload: {e}"))?,
    };
    Ok(text)
}

/// Extract `requestId` value from a URI query string.
fn parse_request_id_param(uri: &str) -> Option<String> {
    let query = uri.split_once('?').map(|(_, q)| q)?;
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("requestId=") {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_transaction_resource_uri_recognizes_valid_uris() {
        assert!(is_transaction_resource_uri("kit://transactions/schema"));
        assert!(is_transaction_resource_uri("kit://transactions/latest"));
        assert!(is_transaction_resource_uri(
            "kit://transactions/latest?requestId=abc"
        ));
    }

    #[test]
    fn is_transaction_resource_uri_rejects_invalid_uris() {
        assert!(!is_transaction_resource_uri("kit://context"));
        assert!(!is_transaction_resource_uri("kit://transactions"));
        assert!(!is_transaction_resource_uri("kit://transactions/other"));
    }

    #[test]
    fn parse_request_id_param_extracts_value() {
        assert_eq!(
            parse_request_id_param("kit://transactions/latest?requestId=txn-1"),
            Some("txn-1".to_string())
        );
        assert_eq!(parse_request_id_param("kit://transactions/latest"), None);
        assert_eq!(
            parse_request_id_param("kit://transactions/latest?requestId="),
            None
        );
    }

    #[test]
    fn schema_resource_returns_valid_json() {
        let content =
            read_transaction_resource("kit://transactions/schema").expect("schema should resolve");
        assert_eq!(content.mime_type, "application/json");

        let value: serde_json::Value =
            serde_json::from_str(&content.text).expect("should be valid JSON");
        assert_eq!(value["kind"], "transaction_trace_schema");
        assert_eq!(value["version"], 1);
        assert!(value["traceModes"].is_array());
        assert_eq!(value["traceModes"][2], "onFailure");
        assert!(value["examples"].is_array());
    }

    #[test]
    fn latest_resource_returns_empty_payload_when_no_traces() {
        let content =
            read_transaction_resource("kit://transactions/latest?requestId=definitely-missing")
                .expect("latest should resolve even when the request ID is absent");
        let value: serde_json::Value =
            serde_json::from_str(&content.text).expect("should be valid JSON");
        assert_eq!(value["status"], "empty");
    }

    #[test]
    fn malformed_uri_returns_actionable_error() {
        let err = read_transaction_resource("kit://transactions/other")
            .expect_err("should reject unknown transaction URI");
        assert!(err.contains("Unknown transaction resource URI"));
        assert!(err.contains("kit://transactions/latest"));
    }

    #[test]
    fn unknown_query_param_returns_actionable_error() {
        let err = read_transaction_resource("kit://transactions/latest?foo=bar")
            .expect_err("should reject unknown query parameter");
        assert!(err.contains("Unknown query parameter"));
        assert!(err.contains("requestId"));
    }

    #[test]
    fn resource_definitions_include_both_resources() {
        let defs = transaction_resource_definitions();
        assert_eq!(defs.len(), 2);
        assert!(defs.iter().any(|r| r.uri == "kit://transactions/latest"));
        assert!(defs.iter().any(|r| r.uri == "kit://transactions/schema"));
    }

    #[test]
    fn latest_resource_redacts_legacy_trace_before_mcp_serialization() {
        use crate::protocol::{
            BatchCommand, TransactionCommandTrace, TransactionTrace, TransactionTraceStatus,
            UiStateSnapshot,
        };

        let trace = TransactionTrace {
            schema_version: 1,
            request_id: "safe-request".to_owned(),
            command_fingerprint: "{\"input\":\"mcp-fingerprint-canary\"}".to_owned(),
            status: TransactionTraceStatus::Ok,
            started_at_ms: 1,
            total_elapsed_ms: 1,
            failed_at: None,
            commands: vec![TransactionCommandTrace {
                index: 0,
                command: "setInput".to_owned(),
                command_payload: Some(BatchCommand::SetInput {
                    text: "mcp-prompt-canary".to_owned(),
                }),
                started_at_ms: 1,
                elapsed_ms: 1,
                before: UiStateSnapshot {
                    input_value: Some("mcp-snapshot-canary".to_owned()),
                    ..UiStateSnapshot::default()
                },
                after: UiStateSnapshot::default(),
                polls: Vec::new(),
                error: None,
            }],
        };

        let text = serialize_transaction_resource_trace(Some(&trace))
            .expect("MCP transaction resource serializes safely");
        assert!(!text.contains("mcp-fingerprint-canary"));
        assert!(!text.contains("mcp-prompt-canary"));
        assert!(!text.contains("mcp-snapshot-canary"));
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid resource JSON");
        assert_eq!(value["commands"][0]["command"], "setInput");
        assert!(value["commandFingerprint"]
            .as_str()
            .is_some_and(|fingerprint| fingerprint.starts_with("sha256:")));
    }
}

use crate::mcp_kit_tools::{ToolContent, ToolDefinition, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CLIPBOARD_COPY_TOOL: &str = "kit/clipboard_copy";
pub const CLIPBOARD_PIN_TOOL: &str = "kit/clipboard_pin";
pub const CLIPBOARD_UNPIN_TOOL: &str = "kit/clipboard_unpin";
pub const CLIPBOARD_DELETE_TOOL: &str = "kit/clipboard_delete";
pub const CLIPBOARD_CLEAR_UNPINNED_TOOL: &str = "kit/clipboard_clear_unpinned";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMutationEnvelope {
    pub ok: bool,
    pub action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ClipboardMutationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ClipboardMutationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardMutationErrorCode {
    InvalidParams,
    NotFound,
    ConfirmRequired,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMutationError {
    pub code: ClipboardMutationErrorCode,
    pub message: String,
}

impl ClipboardMutationError {
    fn new(code: ClipboardMutationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ClipboardMutationErrorCode::InvalidParams, message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardMutationResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub copied: bool,
    pub pinned: bool,
    pub unpinned: bool,
    pub deleted: bool,
    pub cleared_unpinned: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardIdArgs {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardDeleteArgs {
    pub id: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardClearUnpinnedArgs {
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone)]
pub enum ClipboardMutationRequest {
    Copy(ClipboardIdArgs),
    Pin(ClipboardIdArgs),
    Unpin(ClipboardIdArgs),
    Delete(ClipboardDeleteArgs),
    ClearUnpinned(ClipboardClearUnpinnedArgs),
}

pub fn is_clipboard_tool(name: &str) -> bool {
    matches!(
        name,
        CLIPBOARD_COPY_TOOL
            | CLIPBOARD_PIN_TOOL
            | CLIPBOARD_UNPIN_TOOL
            | CLIPBOARD_DELETE_TOOL
            | CLIPBOARD_CLEAR_UNPINNED_TOOL
    )
}

pub fn get_clipboard_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: CLIPBOARD_COPY_TOOL.to_string(),
            description: "Copy a clipboard-history entry to the system clipboard by entry id."
                .to_string(),
            input_schema: id_schema(false),
        },
        ToolDefinition {
            name: CLIPBOARD_PIN_TOOL.to_string(),
            description: "Pin a clipboard-history entry by entry id.".to_string(),
            input_schema: id_schema(false),
        },
        ToolDefinition {
            name: CLIPBOARD_UNPIN_TOOL.to_string(),
            description: "Unpin a clipboard-history entry by entry id.".to_string(),
            input_schema: id_schema(false),
        },
        ToolDefinition {
            name: CLIPBOARD_DELETE_TOOL.to_string(),
            description: "Delete a clipboard-history entry by entry id. Requires confirm:true."
                .to_string(),
            input_schema: id_schema(true),
        },
        ToolDefinition {
            name: CLIPBOARD_CLEAR_UNPINNED_TOOL.to_string(),
            description: "Clear all unpinned clipboard-history entries. Requires confirm:true."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "confirm": { "type": "boolean", "default": false }
                }
            }),
        },
    ]
}

fn id_schema(confirm: bool) -> Value {
    if confirm {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": { "type": "string" },
                "confirm": { "type": "boolean", "default": false }
            },
            "required": ["id"]
        })
    } else {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": { "type": "string" }
            },
            "required": ["id"]
        })
    }
}

pub fn handle_clipboard_tool_call(name: &str, arguments: Value) -> ToolResult {
    let request = match parse_clipboard_mutation_request(name, arguments) {
        Ok(request) => request,
        Err(error) => return error_tool_result(name, error),
    };

    match apply_clipboard_mutation(request) {
        Ok((action, result)) => success_tool_result(action, result),
        Err((action, error)) => error_tool_result(action, error),
    }
}

pub fn parse_clipboard_mutation_request(
    name: &str,
    arguments: Value,
) -> Result<ClipboardMutationRequest, ClipboardMutationError> {
    match name {
        CLIPBOARD_COPY_TOOL => {
            let args: ClipboardIdArgs = serde_json::from_value(arguments).map_err(|error| {
                ClipboardMutationError::invalid_params(format!(
                    "Invalid clipboard_copy arguments: {error}"
                ))
            })?;
            validate_id(&args.id)?;
            Ok(ClipboardMutationRequest::Copy(args))
        }
        CLIPBOARD_PIN_TOOL => {
            let args: ClipboardIdArgs = serde_json::from_value(arguments).map_err(|error| {
                ClipboardMutationError::invalid_params(format!(
                    "Invalid clipboard_pin arguments: {error}"
                ))
            })?;
            validate_id(&args.id)?;
            Ok(ClipboardMutationRequest::Pin(args))
        }
        CLIPBOARD_UNPIN_TOOL => {
            let args: ClipboardIdArgs = serde_json::from_value(arguments).map_err(|error| {
                ClipboardMutationError::invalid_params(format!(
                    "Invalid clipboard_unpin arguments: {error}"
                ))
            })?;
            validate_id(&args.id)?;
            Ok(ClipboardMutationRequest::Unpin(args))
        }
        CLIPBOARD_DELETE_TOOL => {
            let args: ClipboardDeleteArgs = serde_json::from_value(arguments).map_err(|error| {
                ClipboardMutationError::invalid_params(format!(
                    "Invalid clipboard_delete arguments: {error}"
                ))
            })?;
            validate_id(&args.id)?;
            if !args.confirm {
                return Err(ClipboardMutationError::new(
                    ClipboardMutationErrorCode::ConfirmRequired,
                    "kit/clipboard_delete requires confirm:true",
                ));
            }
            Ok(ClipboardMutationRequest::Delete(args))
        }
        CLIPBOARD_CLEAR_UNPINNED_TOOL => {
            let args: ClipboardClearUnpinnedArgs =
                serde_json::from_value(arguments).map_err(|error| {
                    ClipboardMutationError::invalid_params(format!(
                        "Invalid clipboard_clear_unpinned arguments: {error}"
                    ))
                })?;
            if !args.confirm {
                return Err(ClipboardMutationError::new(
                    ClipboardMutationErrorCode::ConfirmRequired,
                    "kit/clipboard_clear_unpinned requires confirm:true",
                ));
            }
            Ok(ClipboardMutationRequest::ClearUnpinned(args))
        }
        _ => Err(ClipboardMutationError::invalid_params(format!(
            "Unknown clipboard tool: {name}"
        ))),
    }
}

fn apply_clipboard_mutation(
    request: ClipboardMutationRequest,
) -> Result<(&'static str, ClipboardMutationResult), (&'static str, ClipboardMutationError)> {
    match request {
        ClipboardMutationRequest::Copy(args) => {
            let action = "clipboard_copy";
            crate::clipboard_history::copy_entry_to_clipboard(&args.id)
                .map_err(|error| (action, map_clipboard_error(&args.id, error)))?;
            Ok((
                action,
                result_for_id(Some(args.id), true, false, false, false, false),
            ))
        }
        ClipboardMutationRequest::Pin(args) => {
            let action = "clipboard_pin";
            crate::clipboard_history::pin_entry(&args.id)
                .map_err(|error| (action, map_clipboard_error(&args.id, error)))?;
            Ok((
                action,
                result_for_id(Some(args.id), false, true, false, false, false),
            ))
        }
        ClipboardMutationRequest::Unpin(args) => {
            let action = "clipboard_unpin";
            crate::clipboard_history::unpin_entry(&args.id)
                .map_err(|error| (action, map_clipboard_error(&args.id, error)))?;
            Ok((
                action,
                result_for_id(Some(args.id), false, false, true, false, false),
            ))
        }
        ClipboardMutationRequest::Delete(args) => {
            let action = "clipboard_delete";
            crate::clipboard_history::remove_entry(&args.id)
                .map_err(|error| (action, map_clipboard_error(&args.id, error)))?;
            Ok((
                action,
                result_for_id(Some(args.id), false, false, false, true, false),
            ))
        }
        ClipboardMutationRequest::ClearUnpinned(_args) => {
            let action = "clipboard_clear_unpinned";
            crate::clipboard_history::clear_unpinned_history().map_err(|error| {
                (
                    action,
                    ClipboardMutationError::new(
                        ClipboardMutationErrorCode::Internal,
                        format!("Failed to clear unpinned clipboard history: {error}"),
                    ),
                )
            })?;
            Ok((
                action,
                result_for_id(None, false, false, false, false, true),
            ))
        }
    }
}

fn validate_id(id: &str) -> Result<(), ClipboardMutationError> {
    if id.trim().is_empty() {
        return Err(ClipboardMutationError::invalid_params(
            "Clipboard entry id cannot be empty",
        ));
    }
    Ok(())
}

fn map_clipboard_error(id: &str, error: anyhow::Error) -> ClipboardMutationError {
    let message = error.to_string();
    if message.contains("Entry not found") || message.contains("not found") {
        ClipboardMutationError::new(
            ClipboardMutationErrorCode::NotFound,
            format!("Clipboard entry not found: {id}"),
        )
    } else {
        ClipboardMutationError::new(
            ClipboardMutationErrorCode::Internal,
            format!("Clipboard mutation failed for {id}: {message}"),
        )
    }
}

fn result_for_id(
    id: Option<String>,
    copied: bool,
    pinned: bool,
    unpinned: bool,
    deleted: bool,
    cleared_unpinned: bool,
) -> ClipboardMutationResult {
    ClipboardMutationResult {
        id,
        copied,
        pinned,
        unpinned,
        deleted,
        cleared_unpinned,
    }
}

fn success_tool_result(action: &'static str, result: ClipboardMutationResult) -> ToolResult {
    envelope_tool_result(ClipboardMutationEnvelope {
        ok: true,
        action,
        result: Some(result),
        error: None,
    })
}

pub fn error_tool_result(action: &str, error: ClipboardMutationError) -> ToolResult {
    let mut result = envelope_tool_result(ClipboardMutationEnvelope {
        ok: false,
        action: action_label(action),
        result: None,
        error: Some(error),
    });
    result.is_error = Some(true);
    result
}

fn action_label(action: &str) -> &'static str {
    match action {
        CLIPBOARD_COPY_TOOL => "clipboard_copy",
        CLIPBOARD_PIN_TOOL => "clipboard_pin",
        CLIPBOARD_UNPIN_TOOL => "clipboard_unpin",
        CLIPBOARD_DELETE_TOOL => "clipboard_delete",
        CLIPBOARD_CLEAR_UNPINNED_TOOL => "clipboard_clear_unpinned",
        "clipboard_copy" => "clipboard_copy",
        "clipboard_pin" => "clipboard_pin",
        "clipboard_unpin" => "clipboard_unpin",
        "clipboard_delete" => "clipboard_delete",
        "clipboard_clear_unpinned" => "clipboard_clear_unpinned",
        _ => "clipboard_unknown",
    }
}

fn envelope_tool_result(envelope: ClipboardMutationEnvelope) -> ToolResult {
    ToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: serde_json::to_string(&envelope).unwrap_or_else(|error| {
                format!(
                    r#"{{"ok":false,"action":"clipboard_internal","error":{{"code":"internal","message":"Failed to serialize clipboard result: {error}"}}}}"#
                )
            }),
        }],
        is_error: None,
    }
}

#[cfg(test)]
mod mcp_clipboard_fuzz_tests {
    use super::*;
    use serde_json::json;

    /// The clipboard MCP handler's parse layer must never panic on hostile input
    /// (Battery A — parse only; never the apply layer that touches the clipboard).
    #[test]
    fn parse_clipboard_requests_never_panic_on_hostile_args() {
        let tools = [
            CLIPBOARD_COPY_TOOL,
            CLIPBOARD_PIN_TOOL,
            CLIPBOARD_UNPIN_TOOL,
            CLIPBOARD_DELETE_TOOL,
            CLIPBOARD_CLEAR_UNPINNED_TOOL,
        ];
        let hostile_args = vec![
            json!(null),
            json!(42),
            json!("nope"),
            json!([]),
            json!({}),
            json!({ "id": 123 }),
            json!({ "id": null }),
            json!({ "id": "" }),
            json!({ "id": "   " }),
            json!({ "id": "x", "unexpected": true }), // deny_unknown_fields
            json!({ "id": "x", "confirm": "yes" }),   // confirm wrong type
            json!({ "id": { "a": 1 } }),
            json!({ "id": "🎉\u{0000}../../etc" }),
            json!({ "id": "x".repeat(1_000_000) }),
            json!({ "confirm": 1 }),
        ];
        for tool in tools {
            for args in &hostile_args {
                let _ = parse_clipboard_mutation_request(tool, args.clone());
            }
        }
    }
}

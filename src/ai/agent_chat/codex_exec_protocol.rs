//! Strict, fail-closed wire decoding for the zero-context Quick AI adapter.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum CodexExecEvent {
    ThreadStarted { thread_id: String },
    TurnStarted,
    Item { phase: ItemPhase, item: CodexItem },
    TurnCompleted,
    TurnFailed { message: String },
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemPhase {
    Started,
    Updated,
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum CodexItem {
    AgentMessage { id: String, text: String },
    WebSearch(WebSearchItem),
    Diagnostic { id: String, message: String },
    Safe { id: String, item_type: String },
    Forbidden { id: String, item_type: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct WebSearchItem {
    pub(super) id: String,
    pub(super) query: String,
    pub(super) action: WebSearchAction,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum WebSearchAction {
    Search { queries: Vec<String> },
    OpenPage { url: Option<String> },
    FindInPage { url: Option<String> },
    Other,
}

#[derive(Debug)]
pub(crate) struct CodexProtocolError(String);

impl std::fmt::Display for CodexProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn required_string(value: &Value, key: &str, context: &str) -> Result<String, CodexProtocolError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| CodexProtocolError(format!("{context}:missing_{key}")))
}

pub(super) fn parse_codex_exec_line(line: &str) -> Result<CodexExecEvent, CodexProtocolError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| CodexProtocolError(format!("quick_ai_codex_malformed_json:{error}")))?;
    let event_type = required_string(&value, "type", "quick_ai_codex_event")?;
    match event_type.as_str() {
        "thread.started" => Ok(CodexExecEvent::ThreadStarted {
            thread_id: required_string(&value, "thread_id", "thread.started")?,
        }),
        "turn.started" => Ok(CodexExecEvent::TurnStarted),
        "item.started" | "item.updated" | "item.completed" => {
            let item = value
                .get("item")
                .filter(|item| item.is_object())
                .ok_or_else(|| CodexProtocolError(format!("{event_type}:missing_item")))?;
            let phase = match event_type.as_str() {
                "item.started" => ItemPhase::Started,
                "item.updated" => ItemPhase::Updated,
                _ => ItemPhase::Completed,
            };
            Ok(CodexExecEvent::Item {
                phase,
                item: parse_item(item)?,
            })
        }
        "turn.completed" => {
            if value.get("usage").is_some_and(|usage| !usage.is_object()) {
                return Err(CodexProtocolError(
                    "turn.completed:invalid_usage".to_string(),
                ));
            }
            Ok(CodexExecEvent::TurnCompleted)
        }
        "turn.failed" => {
            let error = value
                .get("error")
                .filter(|error| error.is_object())
                .ok_or_else(|| CodexProtocolError("turn.failed:missing_error".to_string()))?;
            Ok(CodexExecEvent::TurnFailed {
                message: required_string(error, "message", "turn.failed")?,
            })
        }
        "error" => Ok(CodexExecEvent::Error {
            message: required_string(&value, "message", "error")?,
        }),
        other => Err(CodexProtocolError(format!(
            "quick_ai_codex_unsupported_event:{other}"
        ))),
    }
}

fn parse_item(item: &Value) -> Result<CodexItem, CodexProtocolError> {
    let id = required_string(item, "id", "codex_item")?;
    let item_type = required_string(item, "type", "codex_item")?;
    match item_type.as_str() {
        "agent_message" => Ok(CodexItem::AgentMessage {
            id,
            text: item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        "web_search" => {
            let query = item
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let action = item
                .get("action")
                .cloned()
                .unwrap_or_else(|| json!({"type": "other"}));
            let action_type = required_string(&action, "type", "web_search.action")?;
            let parsed = match action_type.as_str() {
                "search" => {
                    let mut queries = action
                        .get("queries")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if queries.is_empty() {
                        let fallback = action
                            .get("query")
                            .and_then(Value::as_str)
                            .unwrap_or(&query)
                            .trim();
                        if !fallback.is_empty() {
                            queries.push(fallback.to_string());
                        }
                    }
                    WebSearchAction::Search { queries }
                }
                "open_page" => WebSearchAction::OpenPage {
                    url: action
                        .get("url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                "find_in_page" => WebSearchAction::FindInPage {
                    url: action
                        .get("url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                "other" => WebSearchAction::Other,
                other => {
                    return Err(CodexProtocolError(format!(
                        "quick_ai_codex_unsupported_web_action:{other}"
                    )));
                }
            };
            Ok(CodexItem::WebSearch(WebSearchItem {
                id,
                query,
                action: parsed,
            }))
        }
        "error" => Ok(CodexItem::Diagnostic {
            id,
            message: required_string(item, "message", "error_item")?,
        }),
        "reasoning" | "todo_list" => Ok(CodexItem::Safe { id, item_type }),
        "command_execution" | "file_change" | "mcp_tool_call" | "collab_tool_call"
        | "image_view" | "dynamic_tool_call" => Ok(CodexItem::Forbidden { id, item_type }),
        other => Err(CodexProtocolError(format!(
            "quick_ai_codex_unsupported_item:{other}"
        ))),
    }
}

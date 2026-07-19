// Protocol I/O for JSONL message parsing and serialization
//
// This module provides:
// - `parse_message` / `parse_message_graceful` for parsing JSON messages
// - `serialize_message` for serializing messages to JSON
// - `JsonlReader` for streaming JSONL reads

use std::io::{BufRead, BufReader, ErrorKind, Read};
use tracing::{debug, warn};
use uuid::Uuid;

use super::message::Message;

/// Maximum length for raw JSON in logs (prevents huge base64 data in logs)
const MAX_RAW_LOG_PREVIEW: usize = 200;
/// Maximum JSONL message size accepted from script stdout.
///
/// Large lines are treated as malformed input to prevent memory amplification.
const MAX_PROTOCOL_LINE_BYTES: usize = 64 * 1024;

enum LineRead {
    Eof,
    Line,
    TooLong { raw_len: usize },
}

/// Get a truncated preview of raw JSON for logging
///
/// # Safety
/// This function handles UTF-8 correctly by finding a valid char boundary
/// when truncating. It will never panic on multi-byte UTF-8 characters.
fn log_preview(raw: &str) -> (&str, usize) {
    let len = raw.len();
    if len > MAX_RAW_LOG_PREVIEW {
        // Find a valid UTF-8 char boundary at or before MAX_RAW_LOG_PREVIEW
        // This prevents panics on multi-byte characters (emoji, CJK, etc.)
        let mut end = MAX_RAW_LOG_PREVIEW;
        while end > 0 && !raw.is_char_boundary(end) {
            end -= 1;
        }
        (&raw[..end], len)
    } else {
        (raw, len)
    }
}

/// Parse a single JSONL message from a string
///
/// # Arguments
/// * `line` - A JSON string (typically one line from JSONL)
///
/// # Returns
/// * `Result<Message, serde_json::Error>` - Parsed message or deserialization error
///
/// # Security
/// Raw JSON is truncated to 200 chars in error logs to prevent leaking sensitive data
/// (base64 screenshots, clipboard content, etc.)
#[tracing::instrument(skip_all, fields(line_len = line.len()))]
fn parse_message(line: &str) -> Result<Message, serde_json::Error> {
    serde_json::from_str(line).map_err(|e| {
        // SECURITY: Use truncated preview to avoid logging sensitive data
        // (base64 screenshots, clipboard content, user text, etc.)
        let (preview, raw_len) = log_preview(line);
        warn!(
            raw_preview = %preview,
            raw_len = raw_len,
            error = %e,
            "Failed to parse JSONL message"
        );
        e
    })
}

fn is_unknown_message_type_error(error: &str, message_type: &str) -> bool {
    if !error.contains("unknown variant") {
        return false;
    }

    let quoted_markers = [
        format!("`{message_type}`"),
        format!("'{message_type}'"),
        format!("\"{message_type}\""),
    ];

    quoted_markers.iter().any(|marker| error.contains(marker))
        || error.contains(&format!("unknown variant {message_type}"))
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Build a request-correlated response for a malformed JSON protocol payload.
///
/// Deserialization can fail after the JSON envelope has already supplied a
/// usable `requestId` (for example, a `simulateGpuiEvent` with a missing
/// field). Preserve that id so RPC clients fail immediately instead of
/// waiting for a timeout. Syntax errors and envelopes without a string id
/// remain log-only.
pub(crate) fn malformed_request_error(
    line: &str,
    error: impl std::fmt::Display,
) -> Option<Message> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let request_id = value.get("requestId")?.as_str()?.to_string();
    if request_id.is_empty() {
        return None;
    }
    let command = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("protocol")
        .to_string();
    Some(Message::external_command_result(
        request_id,
        command,
        false,
        Some("invalid_payload".to_string()),
        Some(error.to_string()),
    ))
}

/// Result type for graceful message parsing
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum ParseResult {
    /// Successfully parsed a known message type
    Ok(Message),
    /// Message has no "type" field
    MissingType {
        /// Truncated raw JSON for debugging
        raw: String,
    },
    /// Message has an invalid "type" field shape/value (or non-object root)
    InvalidTypeField {
        /// Error describing why the type field is invalid
        error: String,
        /// Truncated raw JSON for debugging
        raw: String,
    },
    /// Unknown message type value - valid JSON with a "type" field we don't recognize
    UnknownType {
        /// The unrecognized type value
        message_type: String,
        /// Truncated raw JSON for debugging
        raw: String,
    },
    /// Known message type but invalid payload (wrong field types, missing required fields)
    InvalidPayload {
        /// The message type that was recognized
        message_type: String,
        /// Serde error message describing the problem
        error: String,
        /// Truncated raw JSON for debugging
        raw: String,
    },
    /// JSON parsing failed entirely (syntax error)
    ParseError(serde_json::Error),
}

/// Structured parse issue for user-facing error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseIssueKind {
    MissingType,
    InvalidTypeField,
    UnknownType,
    InvalidPayload,
    ParseError,
    LineTooLong,
}

#[derive(Debug, Clone)]
pub(crate) struct ParseIssue {
    pub(crate) correlation_id: String,
    pub(crate) kind: ParseIssueKind,
    pub(crate) message_type: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) raw_preview: String,
    pub(crate) raw_len: usize,
}

impl ParseIssue {
    fn new(
        kind: ParseIssueKind,
        message_type: Option<String>,
        error: Option<String>,
        raw_preview: String,
        raw_len: usize,
    ) -> Self {
        Self {
            correlation_id: Uuid::new_v4().to_string(),
            kind,
            message_type,
            error,
            raw_preview,
            raw_len,
        }
    }
}

/// Parse a message with graceful handling of unknown types
///
/// Unlike `parse_message`, this function handles unknown message types
/// gracefully by returning `ParseResult::UnknownType` instead of failing.
///
/// # Classification Logic
/// - Non-object JSON root → `InvalidTypeField`
/// - Missing "type" field → `MissingType`
/// - Non-string "type" field → `InvalidTypeField`
/// - Unknown type value → `UnknownType`
/// - Known type with invalid payload → `InvalidPayload`
/// - Invalid JSON syntax → `ParseError`
///
/// # Arguments
/// * `line` - A JSON string (typically one line from JSONL)
///
/// # Returns
/// * `ParseResult` - Classified parse result
///
/// # Performance
/// This function takes a happy-path fast path:
/// - First parse directly to `Message` (single parse for normal traffic)
/// - Only on error, parse to `serde_json::Value` to classify the failure
///   (missing type, unknown type, or invalid payload).
///
/// # Security
/// Raw JSON is truncated to 200 chars in logs to prevent leaking sensitive data
/// (base64 screenshots, clipboard content, etc.)
#[tracing::instrument(skip_all, fields(line_len = line.len()))]
fn parse_message_graceful(line: &str) -> ParseResult {
    let (preview, _raw_len) = log_preview(line);

    // Fast path for valid known protocol traffic.
    match serde_json::from_str::<Message>(line) {
        Ok(msg) => ParseResult::Ok(msg),
        Err(message_err) => {
            // On failure, classify by inspecting JSON shape/type field.
            let value: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    // Don't log here - caller (JsonlReader) handles logging
                    return ParseResult::ParseError(e);
                }
            };

            let object = match value.as_object() {
                Some(obj) => obj,
                None => {
                    return ParseResult::InvalidTypeField {
                        error: format!(
                            "Message root must be a JSON object, found {}",
                            json_value_kind(&value)
                        ),
                        raw: preview.to_string(),
                    };
                }
            };

            let type_value = match object.get("type") {
                Some(value) => value,
                None => {
                    return ParseResult::MissingType {
                        raw: preview.to_string(),
                    };
                }
            };

            let message_type: String = match type_value.as_str() {
                Some(t) => t.to_string(),
                None => {
                    return ParseResult::InvalidTypeField {
                        error: format!(
                            "Message 'type' field must be a string, found {}",
                            json_value_kind(type_value)
                        ),
                        raw: preview.to_string(),
                    };
                }
            };

            let error_str = message_err.to_string();
            if is_unknown_message_type_error(&error_str, &message_type) {
                ParseResult::UnknownType {
                    message_type,
                    raw: preview.to_string(),
                }
            } else {
                ParseResult::InvalidPayload {
                    message_type,
                    error: error_str,
                    raw: preview.to_string(),
                }
            }
        }
    }
}

/// Serialize a message to JSONL format
///
/// # Arguments
/// * `msg` - The message to serialize
///
/// # Returns
/// * `Result<String, serde_json::Error>` - JSON string (without newline)
pub fn serialize_message(message: &Message) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

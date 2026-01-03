//! Protocol I/O for JSONL message parsing and serialization
//!
//! This module provides:
//! - `parse_message` / `parse_message_graceful` for parsing JSON messages
//! - `serialize_message` for serializing messages to JSON
//! - `JsonlReader` for streaming JSONL reads

use std::io::{BufRead, BufReader, Read};
use tracing::{debug, warn};
use uuid::Uuid;

use super::message::Message;

/// Maximum length for raw JSON in logs (prevents huge base64 data in logs)
const MAX_RAW_LOG_PREVIEW: usize = 200;

/// Get a truncated preview of raw JSON for logging
pub fn log_preview(raw: &str) -> (&str, usize) {
    let len = raw.len();
    if len > MAX_RAW_LOG_PREVIEW {
        (&raw[..MAX_RAW_LOG_PREVIEW], len)
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
pub fn parse_message(line: &str) -> Result<Message, serde_json::Error> {
    serde_json::from_str(line).map_err(|e| {
        // Log the raw input and error for debugging
        warn!(
            raw_input = %line,
            error = %e,
            "Failed to parse JSONL message"
        );
        e
    })
}

/// Result type for graceful message parsing
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ParseResult {
    /// Successfully parsed a known message type
    Ok(Message),
    /// Message has no "type" field
    MissingType {
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
    UnknownType,
    InvalidPayload,
    ParseError,
}

#[derive(Debug, Clone)]
pub struct ParseIssue {
    pub correlation_id: String,
    pub kind: ParseIssueKind,
    pub message_type: Option<String>,
    pub error: Option<String>,
    pub raw_preview: String,
    pub raw_len: usize,
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
/// - Missing "type" field → `MissingType`
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
/// This function uses single-parse optimization: it parses to serde_json::Value
/// first, then converts to Message. This avoids double-parsing on unknown types.
///
/// # Security
/// Raw JSON is truncated to 200 chars in logs to prevent leaking sensitive data
/// (base64 screenshots, clipboard content, etc.)
pub fn parse_message_graceful(line: &str) -> ParseResult {
    let (preview, _raw_len) = log_preview(line);

    // P1-11 FIX: Single parse - parse to Value first, then convert
    // This avoids double-parsing: previously we tried Message first, then Value on failure
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            // Don't log here - caller (JsonlReader) handles logging
            return ParseResult::ParseError(e);
        }
    };

    // Check for type field and extract it as owned String before consuming value
    let msg_type: String = match value.get("type").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => {
            // Missing type field
            return ParseResult::MissingType {
                raw: preview.to_string(),
            };
        }
    };

    // Try to convert Value to Message (consumes value)
    match serde_json::from_value::<Message>(value) {
        Ok(msg) => ParseResult::Ok(msg),
        Err(e) => {
            let error_str = e.to_string();
            // Check if this is an "unknown variant" error (unknown type)
            // vs a field/payload error (known type, bad data)
            if error_str.contains("unknown variant") {
                ParseResult::UnknownType {
                    message_type: msg_type,
                    raw: preview.to_string(),
                }
            } else {
                // Known type but invalid payload
                ParseResult::InvalidPayload {
                    message_type: msg_type,
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
pub fn serialize_message(msg: &Message) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

/// JSONL reader for streaming/chunked message reads
///
/// Provides utilities to read messages one at a time from a reader.
///
/// # Performance
/// Uses a reusable line buffer to avoid allocating a new String per line read.
/// The buffer is cleared and reused between reads (P1-12 optimization).
pub struct JsonlReader<R: Read> {
    reader: BufReader<R>,
    /// Reusable line buffer - cleared and reused per read to avoid allocations
    line_buffer: String,
}

impl<R: Read> JsonlReader<R> {
    /// Create a new JSONL reader
    pub fn new(reader: R) -> Self {
        JsonlReader {
            reader: BufReader::new(reader),
            // Pre-allocate reasonable capacity for typical JSON messages
            line_buffer: String::with_capacity(1024),
        }
    }

    /// Read the next message from the stream
    ///
    /// # Returns
    /// * `Ok(Some(Message))` - Successfully parsed message
    /// * `Ok(None)` - End of stream
    /// * `Err(e)` - Parse error
    pub fn next_message(&mut self) -> Result<Option<Message>, Box<dyn std::error::Error>> {
        // Use loop instead of recursion to prevent stack overflow on many empty lines
        loop {
            // P1-12 FIX: Reuse buffer instead of allocating new String each call
            self.line_buffer.clear();
            match self.reader.read_line(&mut self.line_buffer)? {
                0 => {
                    debug!("Reached end of JSONL stream");
                    return Ok(None);
                }
                bytes_read => {
                    debug!(bytes_read, "Read line from JSONL stream");
                    let trimmed = self.line_buffer.trim();
                    if trimmed.is_empty() {
                        debug!("Skipping empty line in JSONL stream");
                        continue; // Skip empty lines (loop instead of recursion)
                    }
                    let msg = parse_message(trimmed)?;
                    return Ok(Some(msg));
                }
            }
        }
    }

    /// Read the next message with graceful unknown type handling
    ///
    /// Unlike `next_message`, this method uses `parse_message_graceful` to
    /// handle unknown message types without errors. Unknown types are logged
    /// and skipped, continuing to read the next message.
    ///
    /// # Logging
    /// All logging is consolidated here (reader layer). The parse_message_graceful
    /// function does not log - it returns structured results for this layer to handle.
    ///
    /// # Returns
    /// * `Ok(Some(Message))` - Successfully parsed known message
    /// * `Ok(None)` - End of stream
    /// * `Err(e)` - IO error (not parse errors for unknown types)
    pub fn next_message_graceful(&mut self) -> Result<Option<Message>, std::io::Error> {
        self.next_message_graceful_with_handler(|_| {})
    }

    /// Read the next message with graceful unknown type handling, reporting parse issues.
    pub fn next_message_graceful_with_handler<F>(
        &mut self,
        mut on_issue: F,
    ) -> Result<Option<Message>, std::io::Error>
    where
        F: FnMut(ParseIssue),
    {
        loop {
            // P1-12 FIX: Reuse buffer instead of allocating new String each iteration
            self.line_buffer.clear();
            match self.reader.read_line(&mut self.line_buffer)? {
                0 => {
                    debug!("Reached end of JSONL stream");
                    return Ok(None);
                }
                _ => {
                    let trimmed = self.line_buffer.trim();
                    if trimmed.is_empty() {
                        debug!("Skipping empty line in JSONL stream");
                        continue;
                    }

                    // Get preview for logging (security: truncate large payloads)
                    let (preview, raw_len) = log_preview(trimmed);

                    match parse_message_graceful(trimmed) {
                        ParseResult::Ok(msg) => {
                            debug!(message_id = ?msg.id(), "Successfully parsed message");
                            return Ok(Some(msg));
                        }
                        ParseResult::MissingType { .. } => {
                            let issue = ParseIssue::new(
                                ParseIssueKind::MissingType,
                                None,
                                None,
                                preview.to_string(),
                                raw_len,
                            );
                            warn!(
                                correlation_id = %issue.correlation_id,
                                raw_preview = %issue.raw_preview,
                                raw_len = issue.raw_len,
                                "Skipping message with missing 'type' field"
                            );
                            on_issue(issue);
                            continue;
                        }
                        ParseResult::UnknownType { message_type, .. } => {
                            let issue = ParseIssue::new(
                                ParseIssueKind::UnknownType,
                                Some(message_type.clone()),
                                None,
                                preview.to_string(),
                                raw_len,
                            );
                            warn!(
                                correlation_id = %issue.correlation_id,
                                message_type = %message_type,
                                raw_preview = %issue.raw_preview,
                                raw_len = issue.raw_len,
                                "Skipping unknown message type"
                            );
                            on_issue(issue);
                            continue;
                        }
                        ParseResult::InvalidPayload {
                            message_type,
                            error,
                            ..
                        } => {
                            let issue = ParseIssue::new(
                                ParseIssueKind::InvalidPayload,
                                Some(message_type.clone()),
                                Some(error.clone()),
                                preview.to_string(),
                                raw_len,
                            );
                            warn!(
                                correlation_id = %issue.correlation_id,
                                message_type = %message_type,
                                error = %error,
                                raw_preview = %issue.raw_preview,
                                raw_len = issue.raw_len,
                                "Skipping message with invalid payload"
                            );
                            on_issue(issue);
                            continue;
                        }
                        ParseResult::ParseError(e) => {
                            let issue = ParseIssue::new(
                                ParseIssueKind::ParseError,
                                None,
                                Some(e.to_string()),
                                preview.to_string(),
                                raw_len,
                            );
                            // Log but continue - graceful degradation
                            warn!(
                                correlation_id = %issue.correlation_id,
                                error = %e,
                                raw_preview = %issue.raw_preview,
                                raw_len = issue.raw_len,
                                "Skipping malformed JSON message"
                            );
                            on_issue(issue);
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_preview_truncation() {
        // Short string - should not be truncated
        let short = "hello";
        let (preview, len) = log_preview(short);
        assert_eq!(preview, "hello");
        assert_eq!(len, 5);

        // Long string - should be truncated to 200 chars
        let long = "a".repeat(500);
        let (preview, len) = log_preview(&long);
        assert_eq!(preview.len(), 200);
        assert_eq!(len, 500);
    }

    #[test]
    fn test_parse_message_graceful_known_type() {
        let json = r#"{"type":"arg","id":"1","placeholder":"Pick","choices":[]}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::Arg { id, .. }) => {
                assert_eq!(id, "1");
            }
            _ => panic!("Expected ParseResult::Ok with Arg message"),
        }
    }

    #[test]
    fn test_parse_message_graceful_unknown_type() {
        let json = r#"{"type":"futureFeature","id":"1","data":"test"}"#;
        match parse_message_graceful(json) {
            ParseResult::UnknownType { message_type, raw } => {
                assert_eq!(message_type, "futureFeature");
                assert_eq!(raw, json);
            }
            _ => panic!("Expected ParseResult::UnknownType"),
        }
    }

    #[test]
    fn test_parse_message_graceful_invalid_json() {
        let json = "not valid json at all";
        match parse_message_graceful(json) {
            ParseResult::ParseError(_) => {}
            _ => panic!("Expected ParseResult::ParseError"),
        }
    }

    #[test]
    fn test_parse_message_graceful_missing_type_field() {
        let json = r#"{"id":"1","data":"test"}"#;
        match parse_message_graceful(json) {
            ParseResult::MissingType { raw } => {
                // raw should be truncated preview (but this is short enough to be full)
                assert!(raw.contains("id"));
            }
            other => panic!("Expected ParseResult::MissingType, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_graceful_invalid_payload() {
        // Known type "arg" but missing required "placeholder" field
        let json = r#"{"type":"arg","id":"1"}"#;
        match parse_message_graceful(json) {
            ParseResult::InvalidPayload {
                message_type,
                error,
                ..
            } => {
                assert_eq!(message_type, "arg");
                assert!(error.contains("placeholder")); // should mention missing field
            }
            other => panic!("Expected ParseResult::InvalidPayload, got {:?}", other),
        }
    }

    #[test]
    fn test_jsonl_reader_skips_empty_lines() {
        use std::io::Cursor;

        let jsonl = "\n{\"type\":\"beep\"}\n\n{\"type\":\"show\"}\n";
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);

        // First message should be beep (skipping initial empty line)
        let msg1 = reader.next_message().unwrap();
        assert!(matches!(msg1, Some(Message::Beep {})));

        // Second message should be show (skipping intermediate empty lines)
        let msg2 = reader.next_message().unwrap();
        assert!(matches!(msg2, Some(Message::Show {})));

        // Should be EOF
        let msg3 = reader.next_message().unwrap();
        assert!(msg3.is_none());
    }

    #[test]
    fn test_jsonl_reader_graceful_skips_unknown() {
        use std::io::Cursor;

        let jsonl = r#"{"type":"unknownType","id":"1"}
{"type":"beep"}
{"type":"anotherUnknown","data":"test"}
{"type":"show"}
"#;
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);

        // Should skip unknownType and return beep
        let msg1 = reader.next_message_graceful().unwrap();
        assert!(matches!(msg1, Some(Message::Beep {})));

        // Should skip anotherUnknown and return show
        let msg2 = reader.next_message_graceful().unwrap();
        assert!(matches!(msg2, Some(Message::Show {})));

        // Should be EOF
        let msg3 = reader.next_message_graceful().unwrap();
        assert!(msg3.is_none());
    }

    #[test]
    fn test_jsonl_reader_reports_invalid_payload() {
        use std::io::Cursor;

        let jsonl = r#"{"type":"arg","id":"1"}
{"type":"beep"}
"#;
        let cursor = Cursor::new(jsonl);
        let mut reader = JsonlReader::new(cursor);
        let mut issues: Vec<ParseIssue> = Vec::new();

        let msg = reader
            .next_message_graceful_with_handler(|issue| issues.push(issue))
            .unwrap();

        assert!(matches!(msg, Some(Message::Beep {})));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, ParseIssueKind::InvalidPayload);
        assert_eq!(issues[0].message_type.as_deref(), Some("arg"));
        assert!(issues[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("placeholder"));
    }

    // ============================================================
    // Debug Grid Message Tests
    // ============================================================

    #[test]
    fn test_show_grid_default_options() {
        let json = r#"{"type":"showGrid"}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::ShowGrid { options }) => {
                assert_eq!(options.grid_size, 8); // default
                assert!(!options.show_bounds);
                assert!(!options.show_box_model);
                assert!(!options.show_alignment_guides);
            }
            other => panic!("Expected ParseResult::Ok with ShowGrid, got {:?}", other),
        }
    }

    #[test]
    fn test_show_grid_with_options() {
        let json = r#"{"type":"showGrid","gridSize":16,"showBounds":true,"showBoxModel":true}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::ShowGrid { options }) => {
                assert_eq!(options.grid_size, 16);
                assert!(options.show_bounds);
                assert!(options.show_box_model);
                assert!(!options.show_alignment_guides);
            }
            other => panic!("Expected ParseResult::Ok with ShowGrid, got {:?}", other),
        }
    }

    #[test]
    fn test_show_grid_with_depth_preset() {
        use crate::protocol::types::GridDepthOption;

        let json = r#"{"type":"showGrid","depth":"all"}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::ShowGrid { options }) => match options.depth {
                GridDepthOption::Preset(s) => assert_eq!(s, "all"),
                _ => panic!("Expected Preset depth"),
            },
            other => panic!("Expected ParseResult::Ok with ShowGrid, got {:?}", other),
        }
    }

    #[test]
    fn test_show_grid_with_depth_components() {
        use crate::protocol::types::GridDepthOption;

        let json = r#"{"type":"showGrid","depth":["header","list","footer"]}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::ShowGrid { options }) => match options.depth {
                GridDepthOption::Components(components) => {
                    assert_eq!(components.len(), 3);
                    assert!(components.contains(&"header".to_string()));
                    assert!(components.contains(&"list".to_string()));
                    assert!(components.contains(&"footer".to_string()));
                }
                _ => panic!("Expected Components depth"),
            },
            other => panic!("Expected ParseResult::Ok with ShowGrid, got {:?}", other),
        }
    }

    #[test]
    fn test_show_grid_with_color_scheme() {
        let json = r##"{"type":"showGrid","colorScheme":{"gridLines":"#FF0000AA","promptBounds":"#00FF00"}}"##;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::ShowGrid { options }) => {
                let colors = options.color_scheme.expect("Expected color scheme");
                assert_eq!(colors.grid_lines, Some("#FF0000AA".to_string()));
                assert_eq!(colors.prompt_bounds, Some("#00FF00".to_string()));
                assert!(colors.input_bounds.is_none());
            }
            other => panic!("Expected ParseResult::Ok with ShowGrid, got {:?}", other),
        }
    }

    #[test]
    fn test_hide_grid() {
        let json = r#"{"type":"hideGrid"}"#;
        match parse_message_graceful(json) {
            ParseResult::Ok(Message::HideGrid) => {}
            other => panic!("Expected ParseResult::Ok with HideGrid, got {:?}", other),
        }
    }

    #[test]
    fn test_show_grid_roundtrip() {
        use crate::protocol::types::{GridColorScheme, GridDepthOption, GridOptions};

        let options = GridOptions {
            grid_size: 16,
            show_bounds: true,
            show_box_model: false,
            show_alignment_guides: true,
            depth: GridDepthOption::Components(vec!["header".to_string(), "list".to_string()]),
            color_scheme: Some(GridColorScheme {
                grid_lines: Some("#FF0000".to_string()),
                prompt_bounds: None,
                input_bounds: None,
                button_bounds: None,
                list_bounds: None,
                padding_fill: Some("#00FF0040".to_string()),
                margin_fill: None,
                alignment_guide: None,
            }),
        };

        let msg = Message::show_grid_with_options(options);
        let serialized = serde_json::to_string(&msg).expect("Failed to serialize");

        // Verify the serialized JSON has the expected type
        assert!(serialized.contains(r##""type":"showGrid""##));
        assert!(serialized.contains(r##""gridSize":16"##));
        assert!(serialized.contains(r##""showBounds":true"##));
        assert!(serialized.contains(r##""showAlignmentGuides":true"##));

        // Deserialize back and verify
        let deserialized: Message =
            serde_json::from_str(&serialized).expect("Failed to deserialize");
        match deserialized {
            Message::ShowGrid { options } => {
                assert_eq!(options.grid_size, 16);
                assert!(options.show_bounds);
                assert!(!options.show_box_model);
                assert!(options.show_alignment_guides);
            }
            _ => panic!("Expected ShowGrid message"),
        }
    }
}

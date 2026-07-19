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
fn test_log_preview_utf8_safety() {
    // Test with multi-byte UTF-8 characters (emoji)
    // Each emoji is 4 bytes, so 60 emoji = 240 bytes
    let emoji_string = "🎉".repeat(60);
    let (preview, len) = log_preview(&emoji_string);

    // Should not panic and should be valid UTF-8
    assert!(preview.is_char_boundary(preview.len()));
    assert_eq!(len, 240); // 60 * 4 bytes

    // Preview should be <= 200 bytes AND at a valid char boundary
    // Since each emoji is 4 bytes, max is 200/4 = 50 emoji = 200 bytes
    assert!(preview.len() <= MAX_RAW_LOG_PREVIEW);
    // Should be exactly 200 bytes (50 emoji * 4 bytes each)
    assert_eq!(preview.len(), 200);
    assert_eq!(preview.chars().count(), 50);

    // Test with mixed content ending in multi-byte char
    let mixed = format!("{}{}", "a".repeat(198), "🎉"); // 198 + 4 = 202 bytes
    let (preview, len) = log_preview(&mixed);
    assert_eq!(len, 202);
    // Should truncate before the emoji since 198 + 4 > 200
    assert!(preview.len() <= MAX_RAW_LOG_PREVIEW);

    // Test with CJK characters (3 bytes each)
    let cjk = "中文字符测试内容".repeat(10); // 8 chars * 3 bytes * 10 = 240 bytes
    let (preview, len) = log_preview(&cjk);
    assert!(preview.len() <= MAX_RAW_LOG_PREVIEW);
    assert_eq!(len, 240);
    // Verify it's valid UTF-8 by iterating chars
    for c in preview.chars() {
        assert!(c.is_alphabetic() || c.is_numeric());
    }
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
fn test_parse_message_graceful_invalid_type_field_when_root_is_not_object() {
    let json = r#"[1,2,3]"#;
    match parse_message_graceful(json) {
        ParseResult::InvalidTypeField { error, raw } => {
            assert!(
                error.contains("root must be a JSON object"),
                "unexpected error: {error}"
            );
            assert!(error.contains("array"), "unexpected error: {error}");
            assert_eq!(raw, json);
        }
        other => panic!("Expected ParseResult::InvalidTypeField, got {:?}", other),
    }
}

#[test]
fn test_parse_message_graceful_invalid_type_field_when_type_is_not_string() {
    let json = r#"{"type":123,"id":"1"}"#;
    match parse_message_graceful(json) {
        ParseResult::InvalidTypeField { error, raw } => {
            assert!(
                error.contains("field must be a string"),
                "unexpected error: {error}"
            );
            assert!(error.contains("number"), "unexpected error: {error}");
            assert_eq!(raw, json);
        }
        other => panic!("Expected ParseResult::InvalidTypeField, got {:?}", other),
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
fn test_parse_message_graceful_classifies_invalid_enum_payload_as_invalid_payload_when_type_known()
{
    // Known type "windowAction" with invalid enum value for "action"
    let json = r#"{"type":"windowAction","requestId":"req-1","action":"not-an-action"}"#;
    match parse_message_graceful(json) {
        ParseResult::InvalidPayload {
            message_type,
            error,
            ..
        } => {
            assert_eq!(message_type, "windowAction");
            assert!(
                error.contains("action"),
                "error should mention invalid action field, got: {error}"
            );
        }
        other => panic!("Expected ParseResult::InvalidPayload, got {:?}", other),
    }
}

#[test]
fn malformed_payload_with_request_id_builds_typed_error_response() {
    let json = r#"{"type":"simulateGpuiEvent","requestId":"req-malformed","event":{"type":"scrollWheel","x":120,"y":220,"deltaY":-12,"phase":"began"}}"#;
    let serde_error = serde_json::from_str::<Message>(json).unwrap_err();
    let response = malformed_request_error(json, &serde_error)
        .expect("valid JSON carrying a string requestId must produce a response");

    match response {
        Message::ExternalCommandResult {
            request_id,
            command,
            ok,
            error_code,
            error_message,
        } => {
            assert_eq!(request_id, "req-malformed");
            assert_eq!(command, "simulateGpuiEvent");
            assert!(!ok);
            assert_eq!(error_code.as_deref(), Some("invalid_payload"));
            let message = error_message.expect("serde diagnostic");
            assert!(
                message.contains("began") && message.contains("started"),
                "diagnostic should preserve the serde enum-field error: {message}"
            );
        }
        other => panic!("expected typed externalCommandResult, got {other:?}"),
    }
}

#[test]
fn malformed_or_unidentified_payload_remains_log_only() {
    assert!(malformed_request_error("not json", "syntax error").is_none());
    assert!(malformed_request_error(
        r#"{"type":"simulateGpuiEvent","event":{"type":"scrollWheel"}}"#,
        "missing field",
    )
    .is_none());
    assert!(malformed_request_error(
        r#"{"type":"simulateGpuiEvent","requestId":42,"event":{}}"#,
        "wrong request id type",
    )
    .is_none());
}

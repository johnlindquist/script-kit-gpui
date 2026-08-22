use super::*;

// ============================================================
// getElements request parsing
// ============================================================

#[test]
fn test_get_elements_request_parses_without_limit() {
    let json = r#"{"type":"getElements","requestId":"elm-1"}"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("Should parse getElements");

    match msg {
        crate::protocol::Message::GetElements {
            request_id, limit, ..
        } => {
            assert_eq!(request_id, "elm-1");
            assert_eq!(limit, None);
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn test_get_elements_request_parses_with_limit() {
    let json = r#"{"type":"getElements","requestId":"elm-2","limit":5}"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("Should parse getElements with limit");

    match msg {
        crate::protocol::Message::GetElements {
            request_id, limit, ..
        } => {
            assert_eq!(request_id, "elm-2");
            assert_eq!(limit, Some(5));
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

// ============================================================
// elementsResult response serialization
// ============================================================

#[test]
fn test_elements_result_contains_expected_semantic_ids() {
    let elements = vec![
        ElementInfo::input("filter", Some("app"), true),
        ElementInfo::list("choices", 2),
        ElementInfo::product_static_choice(0, "Apple", "apple", true),
        ElementInfo::product_static_choice(1, "Banana", "banana", false),
    ];

    let response = crate::protocol::Message::elements_result(
        "elm-1".to_string(),
        elements,
        4,
        None,
        None,
        Vec::new(),
    );
    let json = serde_json::to_string(&response).expect("Should serialize elementsResult");

    // Must contain the correct type
    assert!(
        json.contains(r#""type":"elementsResult"#),
        "Missing type field"
    );
    assert!(json.contains(r#""requestId":"elm-1"#), "Missing requestId");

    // Must contain semantic IDs
    assert!(
        json.contains("input:filter"),
        "Missing input:filter semantic ID"
    );
    assert!(
        json.contains("list:choices"),
        "Missing list:choices semantic ID"
    );
    assert!(
        json.contains("choice:0:apple"),
        "Missing choice:0:apple semantic ID"
    );
    assert!(
        json.contains("choice:1:banana"),
        "Missing choice:1:banana semantic ID"
    );

    // Must contain totalCount
    assert!(json.contains(r#""totalCount":4"#), "Missing totalCount");

    // Not truncated when elements.len() == totalCount
    assert!(
        json.contains(r#""truncated":false"#),
        "Should not be truncated"
    );
}

#[test]
fn test_elements_result_serializes_explicit_projection_quality() {
    let response = crate::protocol::Message::elements_result_with_projection(
        "projection-1".to_string(),
        "settings".to_string(),
        1,
        ProjectionQuality::Complete,
        Vec::new(),
        vec![ElementInfo::panel("settings")],
        1,
        None,
        None,
        Vec::new(),
    );
    let json = serde_json::to_value(&response).expect("Should serialize projection");
    assert_eq!(json["semanticSurface"], "settings");
    assert_eq!(json["projectionVersion"], 1);
    assert_eq!(json["projectionQuality"], "complete");
    assert_eq!(json["reasonCodes"], serde_json::Value::Null);
}

#[test]
fn test_elements_result_serializes_partial_projection_reasons() {
    let response = crate::protocol::Message::elements_result_with_projection(
        "projection-2".to_string(),
        "flowSession".to_string(),
        1,
        ProjectionQuality::Partial,
        vec![ProjectionReason::RuntimeEntityMissing],
        vec![ElementInfo::panel("flow-session")],
        1,
        None,
        None,
        vec!["flow_session_entity_missing".to_string()],
    );
    let json = serde_json::to_value(&response).expect("Should serialize projection");
    assert_eq!(json["projectionQuality"], "partial");
    assert_eq!(
        json["reasonCodes"],
        serde_json::json!(["runtimeEntityMissing"]),
    );
}

#[test]
fn test_elements_result_serializes_editor_runtime_metadata() {
    let mut editor = ElementInfo::input("notes-editor", Some("# Heading"), true);
    editor.style = Some(ElementStyleInfo {
        owner: "components.notes_editor".to_string(),
        input_render_path: Some("components.notes_editor.render_input".to_string()),
        editor_runtime: Some(ElementEditorRuntimeInfo {
            owner: "components.notes_editor".to_string(),
            language: "markdown".to_string(),
            markdown_registered: true,
            markdown_inline_registered: true,
            injection_languages: vec!["html".to_string(), "toml".to_string(), "yaml".to_string()],
            inline_markdown_injection_disabled: true,
            highlight_query_fingerprint: "fnv1a64:1111111111111111".to_string(),
            injection_query_fingerprint: "fnv1a64:2222222222222222".to_string(),
            inline_highlight_query_fingerprint: "fnv1a64:3333333333333333".to_string(),
            editor_scroll_metrics: Some(serde_json::json!({
                "available": true,
                "scrollTop": 0,
                "scrollHeight": 1200,
                "clientHeight": 400,
                "canScrollY": true,
            })),
            markdown_link_highlight_ranges: Some(serde_json::json!({
                "count": 1,
                "ranges": [{
                    "range": [0, 9],
                    "content": {
                        "contentKind": "userContent",
                        "charLength": 9,
                        "byteLength": 9,
                        "fingerprint": "sha256:fixture",
                        "rawContentReturned": false
                    },
                    "role": "markdownLink"
                }],
            })),
        }),
        surface_background_rgb: Some(0x112233),
        occlusion_rgba: Some(0x112233ff),
        padding_x: Some(16.0),
        padding_y: Some(12.0),
        font_family_source: Some("theme.mono_font_family".to_string()),
        text_size_source: Some("theme.mono_font_size".to_string()),
    });

    let response = crate::protocol::Message::elements_result(
        "editor-runtime".to_string(),
        vec![editor],
        1,
        Some("input:notes-editor".to_string()),
        None,
        Vec::new(),
    );
    let json = serde_json::to_value(&response).expect("Should serialize");
    let runtime = &json["elements"][0]["style"]["editorRuntime"];
    assert_eq!(runtime["owner"], "components.notes_editor");
    assert_eq!(runtime["language"], "markdown");
    assert_eq!(
        runtime["injectionLanguages"],
        serde_json::json!(["html", "toml", "yaml"])
    );
    assert_eq!(runtime["inlineMarkdownInjectionDisabled"], true);
    assert_eq!(runtime["editorScrollMetrics"]["canScrollY"], true);
    assert_eq!(runtime["markdownLinkHighlightRanges"]["count"], 1);
}

#[test]
fn test_elements_result_roundtrip_preserves_structure() {
    let elements = vec![
        ElementInfo::input("filter", Some("test"), true),
        ElementInfo::list("results", 1),
        ElementInfo::product_static_choice(0, "Item One", "item-one", true),
    ];

    let original = crate::protocol::Message::elements_result(
        "rt-1".to_string(),
        elements.clone(),
        3,
        None,
        None,
        Vec::new(),
    );
    let json = serde_json::to_string(&original).expect("Should serialize");
    let parsed: crate::protocol::Message =
        serde_json::from_str(&json).expect("Should deserialize elementsResult");

    match parsed {
        crate::protocol::Message::ElementsResult {
            request_id,
            semantic_surface,
            projection_version,
            projection_quality,
            reason_codes,
            elements: parsed_elements,
            total_count,
            truncated,
            focused_semantic_id,
            selected_semantic_id,
            warnings,
        } => {
            assert_eq!(request_id, "rt-1");
            assert_eq!(semantic_surface, "legacyUnknown");
            assert_eq!(projection_version, 1);
            assert_eq!(projection_quality, ProjectionQuality::Unsupported);
            assert_eq!(reason_codes, vec![ProjectionReason::CollectorUnavailable]);
            assert_eq!(total_count, 3);
            assert!(!truncated);
            assert!(focused_semantic_id.is_none());
            assert!(selected_semantic_id.is_none());
            assert!(warnings.is_empty());
            assert_eq!(parsed_elements.len(), 3);

            // Verify input element
            assert_eq!(parsed_elements[0].semantic_id, "input:filter");
            assert_eq!(parsed_elements[0].element_type, ElementType::Input);
            assert_eq!(parsed_elements[0].value, None);
            let value = parsed_elements[0]
                .content
                .as_ref()
                .and_then(|content| content.value.as_ref())
                .expect("input value is represented by a redacted descriptor");
            assert_eq!(value.content_kind, ElementContentKind::UserContent);
            assert_eq!(value.char_length, 4);
            assert!(!value.raw_content_returned);
            assert_eq!(parsed_elements[0].focused, Some(true));
            assert!(!json.contains("\"value\":\"test\""));

            // Verify list element
            assert_eq!(parsed_elements[1].semantic_id, "list:results");
            assert_eq!(parsed_elements[1].element_type, ElementType::List);

            // Verify choice element
            assert_eq!(parsed_elements[2].semantic_id, "choice:0:item-one");
            assert_eq!(parsed_elements[2].element_type, ElementType::Choice);
            assert_eq!(parsed_elements[2].text, Some("Item One".to_string()));
            assert_eq!(parsed_elements[2].value, Some("item-one".to_string()));
            assert_eq!(parsed_elements[2].selected, Some(true));
            assert_eq!(parsed_elements[2].index, Some(0));
        }
        other => panic!("Expected ElementsResult, got: {:?}", other),
    }
}

// ============================================================
// ElementInfo constructors
// ============================================================

#[test]
fn test_element_info_choice_semantic_id_format() {
    let el = ElementInfo::product_static_choice(0, "Apple", "apple", true);
    assert_eq!(el.semantic_id, "choice:0:apple");
    assert_eq!(el.element_type, ElementType::Choice);
    assert_eq!(el.text, Some("Apple".to_string()));
    assert_eq!(el.value, Some("apple".to_string()));
    assert_eq!(el.selected, Some(true));
    assert_eq!(el.index, Some(0));
    assert_eq!(el.focused, None);
}

#[test]
fn redacted_choice_uses_fingerprinted_identity_and_returns_no_cleartext() {
    let canary = "PRIVATE-CHOICE-CANARY";
    let element =
        ElementInfo::redacted_choice(3, canary, canary, true, ElementContentKind::ExternalContent);
    let json = serde_json::to_string(&element).expect("redacted choice serializes");

    assert!(element.semantic_id.starts_with("choice:3:sha256-"));
    assert_eq!(element.text, None);
    assert_eq!(element.value, None);
    assert!(!json.contains(canary));
    let content = element.content.expect("redacted descriptor");
    assert_eq!(
        content.text.expect("text descriptor").content_kind,
        ElementContentKind::ExternalContent
    );
    assert_eq!(
        content.value.expect("value descriptor").content_kind,
        ElementContentKind::ExternalContent
    );
}

#[test]
fn private_element_fingerprints_are_process_keyed_and_not_public_guessing_oracles() {
    use sha2::{Digest as _, Sha256};

    let secret = "private account password and spoken phrase";
    let public_sha = format!("{:x}", Sha256::digest(secret.as_bytes()));
    let first = RedactedElementContent::new(ElementContentKind::UserContent, secret);
    let repeated = RedactedElementContent::new(ElementContentKind::UserContent, secret);
    let different = RedactedElementContent::new(ElementContentKind::UserContent, "other secret");

    assert_eq!(first.fingerprint, repeated.fingerprint);
    assert_ne!(first.fingerprint, different.fingerprint);
    assert_ne!(first.fingerprint, format!("sha256:{public_sha}"));
    assert_eq!(first.fingerprint.len(), "sha256:".len() + 64);

    let element =
        ElementInfo::redacted_choice(3, secret, secret, true, ElementContentKind::UserContent);
    assert_ne!(
        element.semantic_id,
        format!("choice:3:sha256-{}", &public_sha[..16])
    );

    let choice = Choice::new(secret.to_string(), secret.to_string());
    assert_ne!(
        choice.generate_id(3),
        format!("choice:3:sha256-{}", &public_sha[..16])
    );
    assert_eq!(choice.generate_id(3), element.semantic_id);
}

#[test]
fn test_element_info_input_semantic_id_format() {
    let el = ElementInfo::input("filter", Some("search text"), false);
    assert_eq!(el.semantic_id, "input:filter");
    assert_eq!(el.element_type, ElementType::Input);
    assert_eq!(el.value, None);
    let value = el
        .content
        .as_ref()
        .and_then(|content| content.value.as_ref())
        .expect("input value is represented by a redacted descriptor");
    assert_eq!(value.content_kind, ElementContentKind::UserContent);
    assert_eq!(value.char_length, 11);
    assert_eq!(value.byte_length, 11);
    assert!(value.fingerprint.starts_with("sha256:"));
    assert!(!value.raw_content_returned);
    assert_eq!(el.focused, Some(false));
    assert_eq!(el.selected, None);
    assert_eq!(el.index, None);
}

#[test]
fn test_element_info_list_semantic_id_format() {
    let el = ElementInfo::list("choices", 5);
    assert_eq!(el.semantic_id, "list:choices");
    assert_eq!(el.element_type, ElementType::List);
    assert_eq!(el.text, Some("5 items".to_string()));
    assert_eq!(el.selected, None);
    assert_eq!(el.focused, None);
}

#[test]
fn test_element_info_panel_semantic_id_format() {
    let el = ElementInfo::panel("div-prompt");
    assert_eq!(el.semantic_id, "panel:div-prompt");
    assert_eq!(el.element_type, ElementType::Panel);
}

// ============================================================
// Missing/wrong response detection
// ============================================================

#[test]
fn test_get_elements_wrong_type_does_not_parse_as_get_elements() {
    let json = r#"{"type":"getState","requestId":"s-1"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("Should parse");
    assert!(
        !matches!(msg, crate::protocol::Message::GetElements { .. }),
        "getState must not parse as GetElements"
    );
}

#[test]
fn test_element_type_unknown_variant_forward_compat() {
    let json = r#""futureType""#;
    let parsed: ElementType = serde_json::from_str(json).expect("Should parse unknown type");
    assert_eq!(parsed, ElementType::Unknown);
}

// ============================================================
// Populated choice prompt produces expected element structure
// ============================================================

#[test]
fn test_simulated_choice_prompt_elements_structure() {
    // Simulate what collect_choice_view_elements would produce
    // for an arg prompt with choices and filter "app"
    let elements = vec![
        ElementInfo::input("filter", Some("app"), true),
        ElementInfo::list("choices", 2),
        ElementInfo::product_static_choice(0, "Apple", "apple", true),
        ElementInfo::product_static_choice(1, "Application", "application", false),
    ];

    let total_count = elements.len();
    let response = crate::protocol::Message::elements_result(
        "sim-1".to_string(),
        elements,
        total_count,
        None,
        None,
        Vec::new(),
    );

    match response {
        crate::protocol::Message::ElementsResult {
            elements,
            total_count,
            truncated,
            ..
        } => {
            assert!(
                !truncated,
                "Should not be truncated when all elements included"
            );
            // Acceptance criteria: input:filter present
            assert!(
                elements.iter().any(|e| e.semantic_id == "input:filter"),
                "Must contain input:filter"
            );

            // Acceptance criteria: list semantic ID present
            assert!(
                elements.iter().any(|e| e.semantic_id.starts_with("list:")),
                "Must contain a list:* semantic ID"
            );

            // Acceptance criteria: at least one choice:*:* element
            assert!(
                elements
                    .iter()
                    .any(|e| e.semantic_id.starts_with("choice:")),
                "Must contain at least one choice:*:* element"
            );

            assert!(
                total_count >= 3,
                "Total count should include input + list + choices"
            );
        }
        other => panic!("Expected ElementsResult, got: {:?}", other),
    }
}

// ============================================================
// Limit enforcement and truncation
// ============================================================

#[test]
fn test_elements_result_marks_truncated_when_elements_are_capped() {
    let response = crate::protocol::Message::elements_result(
        "elm-trunc".to_string(),
        vec![ElementInfo::input("filter", Some("a"), true)],
        3,
        None,
        None,
        Vec::new(),
    );

    match response {
        crate::protocol::Message::ElementsResult {
            request_id,
            total_count,
            truncated,
            ..
        } => {
            assert_eq!(request_id, "elm-trunc");
            assert_eq!(total_count, 3);
            assert!(
                truncated,
                "Must be truncated when elements.len() < totalCount"
            );
        }
        other => panic!("Expected ElementsResult, got: {:?}", other),
    }
}

#[test]
fn test_elements_result_not_truncated_when_complete() {
    let elements = vec![
        ElementInfo::input("filter", Some(""), true),
        ElementInfo::panel("div-prompt"),
    ];
    let response = crate::protocol::Message::elements_result(
        "elm-full".to_string(),
        elements,
        2,
        None,
        None,
        Vec::new(),
    );

    match response {
        crate::protocol::Message::ElementsResult { truncated, .. } => {
            assert!(
                !truncated,
                "Must not be truncated when elements.len() == totalCount"
            );
        }
        other => panic!("Expected ElementsResult, got: {:?}", other),
    }
}

// ============================================================
// Choice key-based semantic ID stability
// ============================================================

#[test]
fn test_choice_generate_id_prefers_stable_key() {
    use crate::protocol::types::Choice;
    let choice =
        Choice::new("Apple".to_string(), "apple".to_string()).with_key("fruit-apple".to_string());

    // Key-based ID should ignore the index entirely
    assert_eq!(choice.generate_id(999), "choice:fruit-apple");
}

#[test]
fn test_choice_generate_id_falls_back_to_index_and_private_fingerprint() {
    use crate::protocol::types::Choice;
    let choice = Choice::new("Banana".to_string(), "banana".to_string());

    let first = choice.generate_id(0);
    let moved = choice.generate_id(5);
    assert!(first.starts_with("choice:0:sha256-"));
    assert!(moved.starts_with("choice:5:sha256-"));
    assert_eq!(first.split("sha256-").nth(1), moved.split("sha256-").nth(1));
    assert!(!first.contains("banana"));
}

#[test]
fn test_elements_result_truncated_field_serializes() {
    let response = crate::protocol::Message::elements_result(
        "ser-1".to_string(),
        vec![ElementInfo::input("filter", Some("x"), true)],
        5,
        None,
        None,
        Vec::new(),
    );
    let json = serde_json::to_string(&response).expect("Should serialize");
    assert!(
        json.contains(r#""truncated":true"#),
        "truncated field must appear in JSON: {json}"
    );
}

#[test]
fn test_select_prompt_scenario_elements_result_structure() {
    // Simulate what SelectPrompt.collect_elements would produce
    let elements = vec![
        ElementInfo::input("select-filter", Some("app"), true),
        ElementInfo::list("select-choices", 2),
        ElementInfo::product_static_choice(0, "Apple", "apple", false),
        ElementInfo::product_static_choice(1, "Application", "application", false),
    ];

    let total_count = elements.len();
    let response = crate::protocol::Message::elements_result(
        "sel-1".to_string(),
        elements,
        total_count,
        None,
        None,
        Vec::new(),
    );

    match response {
        crate::protocol::Message::ElementsResult {
            elements,
            total_count,
            truncated,
            ..
        } => {
            assert!(!truncated);
            assert_eq!(total_count, 4);

            // Must include select-filter input
            assert!(
                elements
                    .iter()
                    .any(|e| e.semantic_id == "input:select-filter"),
                "Must contain input:select-filter"
            );

            // Must include select-choices list
            assert!(
                elements
                    .iter()
                    .any(|e| e.semantic_id == "list:select-choices"),
                "Must contain list:select-choices"
            );

            // Must include at least one choice row
            assert!(
                elements
                    .iter()
                    .any(|e| e.semantic_id.starts_with("choice:")),
                "Must contain at least one choice row"
            );
        }
        other => panic!("Expected ElementsResult, got: {:?}", other),
    }
}

#[test]
fn test_elements_result_truncated_false_serializes() {
    let response = crate::protocol::Message::elements_result(
        "ser-2".to_string(),
        vec![ElementInfo::panel("test")],
        1,
        None,
        None,
        Vec::new(),
    );
    let json = serde_json::to_string(&response).expect("Should serialize");
    assert!(
        json.contains(r#""truncated":false"#),
        "truncated:false must appear in JSON: {json}"
    );
}

// ============================================================
// Observation receipt metadata
// ============================================================

#[test]
fn test_elements_result_includes_observation_receipt_fields() {
    let elements = vec![
        ElementInfo::input("filter", Some("app"), true),
        ElementInfo::list("choices", 100),
        ElementInfo::product_static_choice(0, "Apple", "apple", true),
    ];

    let response = crate::protocol::Message::elements_result(
        "elm-obs-1".to_string(),
        elements,
        100,
        Some("input:filter".to_string()),
        Some("choice:0:apple".to_string()),
        vec!["panel_only_theme_chooser".to_string()],
    );

    let json = serde_json::to_string(&response).expect("Should serialize elementsResult");

    assert!(
        json.contains(r#""truncated":true"#),
        "Missing truncated: {json}"
    );
    assert!(
        json.contains(r#""focusedSemanticId":"input:filter""#),
        "Missing focusedSemanticId: {json}"
    );
    assert!(
        json.contains(r#""selectedSemanticId":"choice:0:apple""#),
        "Missing selectedSemanticId: {json}"
    );
    assert!(
        json.contains(r#""warnings":["panel_only_theme_chooser"]"#),
        "Missing warnings: {json}"
    );
}

#[test]
fn test_elements_result_roundtrip_preserves_observation_receipt() {
    let elements = vec![
        ElementInfo::input("filter", Some("test"), true),
        ElementInfo::list("results", 1),
        ElementInfo::product_static_choice(0, "Item One", "item-one", true),
    ];

    let original = crate::protocol::Message::elements_result(
        "rt-1".to_string(),
        elements.clone(),
        10, // total_count > elements.len() → truncated=true
        Some("input:filter".to_string()),
        Some("choice:0:item-one".to_string()),
        Vec::new(),
    );

    let json = serde_json::to_string(&original).expect("Should serialize");
    let parsed: crate::protocol::Message =
        serde_json::from_str(&json).expect("Should deserialize elementsResult");

    match parsed {
        crate::protocol::Message::ElementsResult {
            request_id,
            semantic_surface,
            projection_version,
            projection_quality,
            reason_codes,
            elements: parsed_elements,
            total_count,
            truncated,
            focused_semantic_id,
            selected_semantic_id,
            warnings,
        } => {
            assert_eq!(request_id, "rt-1");
            assert_eq!(semantic_surface, "legacyUnknown");
            assert_eq!(projection_version, 1);
            assert_eq!(projection_quality, ProjectionQuality::Unsupported);
            assert_eq!(reason_codes, vec![ProjectionReason::CollectorUnavailable]);
            assert_eq!(total_count, 10);
            assert!(truncated);
            assert_eq!(focused_semantic_id.as_deref(), Some("input:filter"));
            assert_eq!(selected_semantic_id.as_deref(), Some("choice:0:item-one"));
            assert!(warnings.is_empty());
            assert_eq!(parsed_elements.len(), 3);
        }
        other => panic!("Expected ElementsResult, got: {:?}", other),
    }
}

use super::*;

#[test]
fn test_mouse_data_with_coordinates() {
    let data = MouseData {
        x: 100.5,
        y: 200.5,
        button: None,
    };
    assert_eq!(data.x, 100.5);
    assert_eq!(data.y, 200.5);
    assert!(data.button.is_none());
}

#[test]
fn test_mouse_data_with_button() {
    let data = MouseData {
        x: 50.0,
        y: 75.0,
        button: Some("left".to_string()),
    };
    assert_eq!(data.button, Some("left".to_string()));
}

#[test]
fn test_mouse_data_serialization() {
    let data = MouseData {
        x: 10.0,
        y: 20.0,
        button: None,
    };
    let json = serde_json::to_string(&data).unwrap();
    // Without button, should not include button field due to skip_serializing_if
    assert!(json.contains("\"x\":10"));
    assert!(json.contains("\"y\":20"));
    assert!(!json.contains("button"));
}

#[test]
fn test_mouse_data_with_button_serialization() {
    let data = MouseData {
        x: 10.0,
        y: 20.0,
        button: Some("right".to_string()),
    };
    let json = serde_json::to_string(&data).unwrap();
    assert!(json.contains("\"button\":\"right\""));
}

#[test]
fn test_mouse_data_deserialization() {
    // Coordinates only (common case)
    let json = r#"{"x":100,"y":200}"#;
    let data: MouseData = serde_json::from_str(json).unwrap();
    assert_eq!(data.x, 100.0);
    assert_eq!(data.y, 200.0);
    assert!(data.button.is_none());
}

#[test]
fn test_mouse_data_deserialization_with_button() {
    let json = r#"{"x":50,"y":75,"button":"left"}"#;
    let data: MouseData = serde_json::from_str(json).unwrap();
    assert_eq!(data.x, 50.0);
    assert_eq!(data.y, 75.0);
    assert_eq!(data.button, Some("left".to_string()));
}

#[test]
fn test_mouse_data_roundtrip() {
    let original = MouseData {
        x: 123.456,
        y: 789.012,
        button: Some("middle".to_string()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: MouseData = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// ============================================================
// Choice Tests (with key field for stable semantic IDs)
// ============================================================

#[test]
fn test_choice_with_key() {
    let choice =
        Choice::new("Apple".to_string(), "apple".to_string()).with_key("fruit-apple".to_string());
    assert_eq!(choice.key, Some("fruit-apple".to_string()));
}

#[test]
fn test_choice_semantic_id_prefers_key() {
    let choice = Choice::new("Apple".to_string(), "apple".to_string())
        .with_key("stable-key".to_string())
        .with_semantic_id(5); // index 5 should be ignored when key exists

    // When key is present, semantic_id should use key, not index
    assert!(choice.semantic_id.as_ref().unwrap().contains("stable-key"));
}

#[test]
fn test_choice_semantic_id_falls_back_to_index() {
    let choice = Choice::new("Banana".to_string(), "banana".to_string()).with_semantic_id(3);

    // Index fallback keeps the private value out of the protocol-visible ID.
    let semantic_id = choice.semantic_id.as_ref().unwrap();
    assert!(semantic_id.starts_with("choice:3:sha256-"));
    assert!(!semantic_id.contains("banana"));
    assert_eq!(semantic_id, &choice.generate_id(3));
}

#[test]
fn test_choice_key_serialization() {
    let choice = Choice::new("Test".to_string(), "test".to_string()).with_key("my-key".to_string());
    let json = serde_json::to_string(&choice).unwrap();
    assert!(json.contains("\"key\":\"my-key\""));
}

#[test]
fn test_choice_key_deserialization() {
    let json = r#"{"name":"Apple","value":"apple","key":"fruit-apple"}"#;
    let choice: Choice = serde_json::from_str(json).unwrap();
    assert_eq!(choice.key, Some("fruit-apple".to_string()));
}

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::protocol::generate_semantic_id_named;

// ============================================================
// SUBMIT VALUE TYPE
// ============================================================

/// A submit value that can be either a string or arbitrary JSON.
///
/// This type provides backwards-compatible handling of submit values:
/// - Old scripts sending `"value": "text"` deserialize as `Text("text")`
/// - New scripts sending `"value": ["a", "b"]` or `"value": {...}` deserialize as `Json(...)`
///
/// The untagged serde representation means no type discrimination field is needed;
/// strings are tried first, then arbitrary JSON.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SubmitValue {
    /// A simple string value (backwards compatible with Option<String>)
    Text(String),
    /// An arbitrary JSON value (for arrays, objects, numbers, booleans, null)
    Json(serde_json::Value),
}

impl SubmitValue {
    /// Create a text value
    pub fn text(s: impl Into<String>) -> Self {
        SubmitValue::Text(s.into())
    }

    /// Create a JSON value
    pub fn json(v: serde_json::Value) -> Self {
        SubmitValue::Json(v)
    }

    /// Try to get the value as a string.
    /// Returns Some for Text variant, None for Json variant.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            SubmitValue::Text(s) => Some(s),
            SubmitValue::Json(_) => None,
        }
    }

    /// Convert to a string representation.
    /// - Text: returns the string
    /// - Json: returns JSON-serialized string
    pub fn to_string_repr(&self) -> String {
        match self {
            SubmitValue::Text(s) => s.clone(),
            SubmitValue::Json(v) => serde_json::to_string(v).unwrap_or_default(),
        }
    }

    /// Convert to an Option<String> for backwards compatibility.
    /// - Text: returns Some(string)
    /// - Json: returns Some(json_serialized)
    pub fn to_option_string(&self) -> Option<String> {
        Some(self.to_string_repr())
    }

    /// Check if this is a text value
    pub fn is_text(&self) -> bool {
        matches!(self, SubmitValue::Text(_))
    }

    /// Check if this is a JSON value
    pub fn is_json(&self) -> bool {
        matches!(self, SubmitValue::Json(_))
    }

    /// Get the underlying serde_json::Value
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            SubmitValue::Text(s) => serde_json::Value::String(s.clone()),
            SubmitValue::Json(v) => v.clone(),
        }
    }
}

impl From<String> for SubmitValue {
    fn from(s: String) -> Self {
        SubmitValue::Text(s)
    }
}

impl From<&str> for SubmitValue {
    fn from(s: &str) -> Self {
        SubmitValue::Text(s.to_string())
    }
}

impl From<serde_json::Value> for SubmitValue {
    fn from(v: serde_json::Value) -> Self {
        // If it's a string JSON value, convert to Text for consistency
        if let serde_json::Value::String(s) = v {
            SubmitValue::Text(s)
        } else {
            SubmitValue::Json(v)
        }
    }
}

impl Default for SubmitValue {
    fn default() -> Self {
        SubmitValue::Text(String::new())
    }
}

/// A choice option for arg() prompts
///
/// Supports Script Kit API: name, value, and optional description.
/// Semantic IDs are generated for AI-driven UX targeting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Choice {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional stable key for deterministic semantic ID generation.
    /// When provided, this takes precedence over index-based IDs.
    /// Useful when list order may change (filtering, sorting, ranking).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Semantic ID for AI targeting.
    /// - With key: `choice:{key}`
    /// - Without key: `choice:{index}:sha256-{fingerprint}`
    ///
    /// This field is typically generated at render time, not provided by scripts.
    #[serde(skip_serializing_if = "Option::is_none", rename = "semanticId")]
    pub semantic_id: Option<String>,
}

fn redacted_choice_semantic_id(index: usize, value: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(value.as_bytes()));
    format!("choice:{index}:sha256-{}", &digest[..16])
}

impl Choice {
    pub fn new(name: String, value: String) -> Self {
        Choice {
            name,
            value,
            description: None,
            key: None,
            semantic_id: None,
        }
    }

    pub fn with_description(name: String, value: String, description: String) -> Self {
        Choice {
            name,
            value,
            description: Some(description),
            key: None,
            semantic_id: None,
        }
    }

    /// Set a stable key for this choice.
    /// When present, semantic ID generation will use this key instead of index.
    pub fn with_key(mut self, key: String) -> Self {
        self.key = Some(key);
        self
    }

    /// Generate and set the semantic ID for this choice.
    ///
    /// If `key` is set, generates: `choice:{key}`
    /// Otherwise, generates a bounded SHA-256 fingerprint identity. Raw choice
    /// values never become protocol-visible semantic IDs.
    pub fn with_semantic_id(mut self, index: usize) -> Self {
        self.semantic_id = Some(if let Some(ref key) = self.key {
            // Stable key takes precedence - use named ID format
            generate_semantic_id_named("choice", key)
        } else {
            // Fallback to index-based ID
            redacted_choice_semantic_id(index, &self.value)
        });
        self
    }

    /// Set the semantic ID directly (for custom IDs)
    pub fn set_semantic_id(&mut self, id: String) {
        self.semantic_id = Some(id);
    }

    /// Generate the semantic ID without setting it (for external use)
    ///
    /// Prefers key-based ID if key is set, otherwise uses index-based ID.
    pub fn generate_id(&self, index: usize) -> String {
        if let Some(ref key) = self.key {
            generate_semantic_id_named("choice", key)
        } else {
            redacted_choice_semantic_id(index, &self.value)
        }
    }
}

/// A field definition for form/fields prompts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl Field {
    pub fn new(name: String) -> Self {
        Field {
            name,
            label: None,
            field_type: None,
            placeholder: None,
            value: None,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_type(mut self, field_type: String) -> Self {
        self.field_type = Some(field_type);
        self
    }

    pub fn with_placeholder(mut self, placeholder: String) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
}

/// Clipboard action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardAction {
    Read,
    Write,
}

/// Clipboard format type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardFormat {
    Text,
    Image,
}

/// Reserved keyboard action type.
///
/// GPUI does not currently provide receipt-backed native keyboard input for
/// these actions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardAction {
    Type,
    Tap,
}

/// Reserved mouse action type.
///
/// GPUI does not currently provide receipt-backed native mouse input for these
/// actions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum MouseAction {
    Move,
    Click,
    SetPosition,
}

/// Clipboard entry type for clipboard history
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardEntryType {
    Text,
    Image,
}

/// Clipboard history action type
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardHistoryAction {
    List,
    Pin,
    Unpin,
    Remove,
    Clear,
    #[serde(rename = "trimOversize")]
    TrimOversize,
}

/// Window action type for window management
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowActionType {
    Focus,
    Close,
    Minimize,
    Maximize,
    Resize,
    Move,
    Tile,
    #[serde(rename = "moveToNextDisplay")]
    MoveToNextDisplay,
    #[serde(rename = "moveToPreviousDisplay")]
    MoveToPreviousDisplay,
}

/// Tile position for window tiling operations
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TilePosition {
    // Half positions
    Left,
    Right,
    Top,
    Bottom,
    // Quadrant positions
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    // Horizontal thirds
    LeftThird,
    CenterThird,
    RightThird,
    // Vertical thirds
    TopThird,
    MiddleThird,
    BottomThird,
    // Horizontal two-thirds
    FirstTwoThirds,
    LastTwoThirds,
    // Vertical two-thirds
    TopTwoThirds,
    BottomTwoThirds,
    // Centered positions
    Center,
    AlmostMaximize,
    // Full screen
    Maximize,
}

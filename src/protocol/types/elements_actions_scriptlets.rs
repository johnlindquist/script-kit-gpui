use serde::{Deserialize, Serialize};

use crate::protocol::{generate_semantic_id, generate_semantic_id_named};

/// Element type for UI element querying (getElements)
///
/// # Forward Compatibility
/// The `Unknown` variant with `#[serde(other)]` ensures forward compatibility:
/// if a newer protocol version adds new element types, older receivers
/// will deserialize them as `Unknown` instead of failing entirely.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Choice,
    Input,
    Button,
    Slider,
    ColorPicker,
    Toggle,
    Panel,
    List,
    /// Unknown element type (forward compatibility fallback)
    /// When deserializing, any unrecognized type string becomes Unknown
    #[serde(other)]
    Unknown,
}

/// Style ownership metadata for semantic UI elements.
///
/// This is intentionally token/source-level metadata, not computed pixel
/// sampling. Runtime probes use it to prove that two surfaces share the same
/// component and theme-token owner.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementStyleInfo {
    pub owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_render_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_runtime: Option<ElementEditorRuntimeInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_background_rgb: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occlusion_rgba: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_size_source: Option<String>,
}

/// Runtime Markdown editor metadata for parity probes.
///
/// This is intentionally configuration-level metadata. It proves which shared
/// editor/highlighter path a surface uses without requiring visual screenshot
/// sampling or peeking into gpui-component internals.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementEditorRuntimeInfo {
    pub owner: String,
    pub language: String,
    pub markdown_registered: bool,
    pub markdown_inline_registered: bool,
    pub injection_languages: Vec<String>,
    pub inline_markdown_injection_disabled: bool,
    pub highlight_query_fingerprint: String,
    pub injection_query_fingerprint: String,
    pub inline_highlight_query_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_scroll_metrics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown_link_highlight_ranges: Option<serde_json::Value>,
}

/// Closed semantic roles shared by conversational context/identity controls.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConversationSemanticRole {
    ContextChip,
    IdentityBadge,
    DestinationSelector,
}

impl ConversationSemanticRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContextChip => "contextChip",
            Self::IdentityBadge => "identityBadge",
            Self::DestinationSelector => "destinationSelector",
        }
    }
}

/// Actions are role-specific: context removal can never be invoked against an
/// identity badge, and destination selection never mutates conversation context.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConversationSemanticAction {
    RemoveContext,
    OpenContextDetails,
    OpenIdentitySelector,
    OpenIdentityDetails,
    SelectDestination,
}

impl ConversationSemanticAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RemoveContext => "removeContext",
            Self::OpenContextDetails => "openContextDetails",
            Self::OpenIdentitySelector => "openIdentitySelector",
            Self::OpenIdentityDetails => "openIdentityDetails",
            Self::SelectDestination => "selectDestination",
        }
    }

    pub const fn is_valid_for(self, role: ConversationSemanticRole) -> bool {
        matches!(
            (role, self),
            (
                ConversationSemanticRole::ContextChip,
                Self::RemoveContext | Self::OpenContextDetails
            ) | (
                ConversationSemanticRole::IdentityBadge,
                Self::OpenIdentitySelector | Self::OpenIdentityDetails
            ) | (
                ConversationSemanticRole::DestinationSelector,
                Self::SelectDestination
            )
        )
    }
}

/// Information about a UI element returned by getElements
///
/// Contains semantic ID, type, text content, and state information
/// for AI-driven UX targeting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementInfo {
    /// Semantic ID for targeting (e.g., "choice:0:apple")
    pub semantic_id: String,
    /// Element type (choice, input, button, panel, list)
    #[serde(rename = "type")]
    pub element_type: ElementType,
    /// Display text of the element
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Value (for choices/inputs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Whether this element is currently selected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Whether this element is currently focused
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
    /// Index in parent container (for list items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Semantic role for richer rows such as non-selectable source status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Stable kind for source-specific or status rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Root unified-search source id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Human-facing root unified-search source name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// Whether this row can be selected/submitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selectable: Option<bool>,
    /// Machine-stable status kind for source status rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_kind: Option<String>,
    /// Machine-stable disabled reason for action-like elements.
    #[serde(rename = "actionDisabled", skip_serializing_if = "Option::is_none")]
    pub action_disabled: Option<String>,
    /// Shared component/style owner metadata for runtime parity probes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<ElementStyleInfo>,
}

impl ElementInfo {
    /// Create a new ElementInfo for a choice element
    pub fn choice(index: usize, name: &str, value: &str, selected: bool) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id("choice", index, value),
            element_type: ElementType::Choice,
            text: Some(name.to_string()),
            value: Some(value.to_string()),
            selected: Some(selected),
            focused: None,
            index: Some(index),
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
    }

    /// Create a new ElementInfo for an input element
    pub fn input(name: &str, value: Option<&str>, focused: bool) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id_named("input", name),
            element_type: ElementType::Input,
            text: None,
            value: value.map(|s| s.to_string()),
            selected: None,
            focused: Some(focused),
            index: None,
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
    }

    /// Create a new ElementInfo for a button element
    pub fn button(index: usize, label: &str) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id("button", index, label),
            element_type: ElementType::Button,
            text: Some(label.to_string()),
            value: None,
            selected: None,
            focused: None,
            index: Some(index),
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
    }

    /// Create a new ElementInfo for a panel element
    pub fn panel(name: &str) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id_named("panel", name),
            element_type: ElementType::Panel,
            text: None,
            value: None,
            selected: None,
            focused: None,
            index: None,
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
    }

    /// Create a new ElementInfo for a list element
    pub fn list(name: &str, item_count: usize) -> Self {
        ElementInfo {
            semantic_id: generate_semantic_id_named("list", name),
            element_type: ElementType::List,
            text: Some(format!("{} items", item_count)),
            value: None,
            selected: None,
            focused: None,
            index: None,
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }
    }
}

/// Protocol action for the Actions API
///
/// Represents an action that can be displayed in the ActionsDialog.
/// The `has_action` field is CRITICAL - it determines the routing behavior:
/// - `has_action=true`: Rust sends ActionTriggered back to SDK (for actions with onAction handlers)
/// - `has_action=false`: Rust submits the value directly (for simple actions)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolAction {
    /// Display name of the action
    pub name: String,
    /// Optional description shown below the name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional keyboard shortcut (e.g., "cmd+c")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// Value to submit or pass to the action handler
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// CRITICAL: If true, send ActionTriggered to SDK; if false, submit value directly
    #[serde(default)]
    pub has_action: bool,
    /// Whether this action is visible in the list
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Whether to close the dialog after triggering
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<bool>,
}

impl ProtocolAction {
    /// Create a new ProtocolAction with just a name
    pub fn new(name: String) -> Self {
        ProtocolAction {
            name,
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: None,
        }
    }

    /// Default visibility is true when unset.
    /// Actions with `visible: false` should be filtered out of the UI.
    #[inline]
    pub fn is_visible(&self) -> bool {
        self.visible.unwrap_or(true)
    }

    /// Default close behavior is true when unset.
    /// Actions with `close: false` should keep the dialog open after triggering.
    #[inline]
    pub fn should_close(&self) -> bool {
        self.close.unwrap_or(true)
    }

    /// Create a ProtocolAction with a value that submits directly
    pub fn with_value(name: String, value: String) -> Self {
        ProtocolAction {
            name,
            description: None,
            shortcut: None,
            value: Some(value),
            has_action: false,
            visible: None,
            close: None,
        }
    }

    /// Create a ProtocolAction that triggers an SDK handler
    pub fn with_handler(name: String) -> Self {
        ProtocolAction {
            name,
            description: None,
            shortcut: None,
            value: None,
            has_action: true,
            visible: None,
            close: None,
        }
    }
}

/// Scriptlet metadata for protocol serialization
///
/// Matches the ScriptletMetadata struct from scriptlets.rs but optimized
/// for JSON protocol transmission.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptletMetadataData {
    /// Trigger text that activates this scriptlet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// Keyboard shortcut (e.g., "cmd shift k")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// Raw cron expression (e.g., "*/5 * * * *")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// Natural language schedule (e.g., "every tuesday at 2pm") - converted to cron internally
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Whether to run in background
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    /// File paths to watch for changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<String>,
    /// System event to trigger on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Description of the scriptlet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Text expansion trigger (e.g., "type,,")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
}

/// Scriptlet data for protocol transmission
///
/// Represents a parsed scriptlet from markdown files, containing
/// the code content, tool type, metadata, and variable inputs.
/// Used to pass scriptlet data between Rust and SDK/bun.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptletData {
    /// Name of the scriptlet (from H2 header)
    pub name: String,
    /// Command identifier (slugified name)
    pub command: String,
    /// Tool type (bash, python, ts, etc.)
    pub tool: String,
    /// The actual code content
    pub content: String,
    /// Named input placeholders (e.g., ["variableName", "otherVar"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// Group name (from H1 header)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// HTML preview content (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Parsed metadata from HTML comments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ScriptletMetadataData>,
    /// The kit this scriptlet belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kit: Option<String>,
    /// Source file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Whether this is a scriptlet.
    /// Defaults to `false` when deserialized (for backwards compatibility).
    /// The `ScriptletData::new()` constructor sets this to `true`.
    #[serde(default)]
    pub is_scriptlet: bool,
}

impl ScriptletData {
    /// Create a new ScriptletData with required fields
    pub fn new(name: String, command: String, tool: String, content: String) -> Self {
        ScriptletData {
            name,
            command,
            tool,
            content,
            inputs: Vec::new(),
            group: None,
            preview: None,
            metadata: None,
            kit: None,
            source_path: None,
            is_scriptlet: true,
        }
    }

    /// Add inputs
    pub fn with_inputs(mut self, inputs: Vec<String>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Add group
    pub fn with_group(mut self, group: String) -> Self {
        self.group = Some(group);
        self
    }

    /// Add preview HTML
    pub fn with_preview(mut self, preview: String) -> Self {
        self.preview = Some(preview);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: ScriptletMetadataData) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add kit
    pub fn with_kit(mut self, kit: String) -> Self {
        self.kit = Some(kit);
        self
    }

    /// Add source path
    pub fn with_source_path(mut self, path: String) -> Self {
        self.source_path = Some(path);
        self
    }
}

#[cfg(test)]
mod conversation_semantic_tests {
    use super::*;

    #[test]
    fn conversation_semantic_roles_and_actions_are_exhaustive_and_role_safe() {
        let roles = [
            ConversationSemanticRole::ContextChip,
            ConversationSemanticRole::IdentityBadge,
            ConversationSemanticRole::DestinationSelector,
        ];
        assert_eq!(
            roles.map(ConversationSemanticRole::as_str),
            ["contextChip", "identityBadge", "destinationSelector"]
        );

        assert!(ConversationSemanticAction::RemoveContext
            .is_valid_for(ConversationSemanticRole::ContextChip));
        assert!(!ConversationSemanticAction::RemoveContext
            .is_valid_for(ConversationSemanticRole::IdentityBadge));
        assert!(ConversationSemanticAction::OpenIdentitySelector
            .is_valid_for(ConversationSemanticRole::IdentityBadge));
        assert!(!ConversationSemanticAction::SelectDestination
            .is_valid_for(ConversationSemanticRole::ContextChip));
        assert!(ConversationSemanticAction::SelectDestination
            .is_valid_for(ConversationSemanticRole::DestinationSelector));
    }
}

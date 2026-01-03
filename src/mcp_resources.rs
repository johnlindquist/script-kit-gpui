//! MCP Resources Handler
//!
//! Implements MCP resources for Script Kit:
//! - `kit://state` - Current app state as JSON
//! - `scripts://` - List of available scripts
//! - `scriptlets://` - List of available scriptlets
//!
//! Resources are read-only data that clients can access without tool calls.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::scripts::Script;
use crate::scripts::Scriptlet;

/// MCP Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Unique URI for this resource (e.g., "scripts://", "kit://state")
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this resource provides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// Resource content returned by resources/read
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// The URI of the resource
    pub uri: String,
    /// MIME type of the content
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    /// The actual content (typically JSON stringified)
    pub text: String,
}

/// Application state exposed via kit://state resource
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppStateResource {
    /// Whether the app window is visible
    pub visible: bool,
    /// Whether the app window is focused
    pub focused: bool,
    /// Number of loaded scripts
    pub script_count: usize,
    /// Number of loaded scriptlets
    pub scriptlet_count: usize,
    /// Current filter text (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,
    /// Currently selected index (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_index: Option<usize>,
}

/// Script metadata for the scripts:// resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResourceEntry {
    /// Script name
    pub name: String,
    /// File path
    pub path: String,
    /// File extension (ts, js)
    pub extension: String,
    /// Description (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether script has a schema (makes it an MCP tool)
    pub has_schema: bool,
}

impl From<&Script> for ScriptResourceEntry {
    fn from(script: &Script) -> Self {
        Self {
            name: script.name.clone(),
            path: script.path.to_string_lossy().to_string(),
            extension: script.extension.clone(),
            description: script.description.clone(),
            has_schema: script.schema.is_some(),
        }
    }
}

/// Scriptlet metadata for the scriptlets:// resource  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptletResourceEntry {
    /// Scriptlet name
    pub name: String,
    /// Tool type (bash, ts, paste, etc.)
    pub tool: String,
    /// Description (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Group name (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Expand trigger (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,
    /// Keyboard shortcut (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

impl From<&Scriptlet> for ScriptletResourceEntry {
    fn from(scriptlet: &Scriptlet) -> Self {
        Self {
            name: scriptlet.name.clone(),
            tool: scriptlet.tool.clone(),
            description: scriptlet.description.clone(),
            group: scriptlet.group.clone(),
            expand: scriptlet.expand.clone(),
            shortcut: scriptlet.shortcut.clone(),
        }
    }
}

/// Get all available MCP resources
pub fn get_resource_definitions() -> Vec<McpResource> {
    vec![
        McpResource {
            uri: "kit://state".to_string(),
            name: "App State".to_string(),
            description: Some(
                "Current Script Kit application state including visibility, focus, and counts"
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "scripts://".to_string(),
            name: "Scripts".to_string(),
            description: Some("List of all available scripts in ~/.sk/kit/scripts/".to_string()),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "scriptlets://".to_string(),
            name: "Scriptlets".to_string(),
            description: Some("List of all available scriptlets from markdown files".to_string()),
            mime_type: "application/json".to_string(),
        },
    ]
}

/// Read a specific resource by URI
///
/// # Arguments
/// * `uri` - The resource URI to read
/// * `scripts` - Available scripts for scripts:// resource
/// * `scriptlets` - Available scriptlets for scriptlets:// resource
/// * `app_state` - Current app state for kit://state resource
///
/// # Returns
/// * `Ok(ResourceContent)` - The resource content
/// * `Err(String)` - Error message if resource not found
pub fn read_resource(
    uri: &str,
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    app_state: Option<&AppStateResource>,
) -> Result<ResourceContent, String> {
    match uri {
        "kit://state" => read_state_resource(app_state),
        "scripts://" => read_scripts_resource(scripts),
        "scriptlets://" => read_scriptlets_resource(scriptlets),
        _ => Err(format!("Resource not found: {}", uri)),
    }
}

/// Read kit://state resource
fn read_state_resource(app_state: Option<&AppStateResource>) -> Result<ResourceContent, String> {
    let state = app_state.cloned().unwrap_or_default();
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize app state: {}", e))?;

    Ok(ResourceContent {
        uri: "kit://state".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Read scripts:// resource
fn read_scripts_resource(scripts: &[Script]) -> Result<ResourceContent, String> {
    let entries: Vec<ScriptResourceEntry> = scripts.iter().map(ScriptResourceEntry::from).collect();
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize scripts: {}", e))?;

    Ok(ResourceContent {
        uri: "scripts://".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Read scriptlets:// resource
fn read_scriptlets_resource(scriptlets: &[Scriptlet]) -> Result<ResourceContent, String> {
    let entries: Vec<ScriptletResourceEntry> = scriptlets
        .iter()
        .map(ScriptletResourceEntry::from)
        .collect();
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize scriptlets: {}", e))?;

    Ok(ResourceContent {
        uri: "scriptlets://".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Convert resource content to JSON-RPC result format
pub fn resource_content_to_value(content: ResourceContent) -> Value {
    serde_json::json!({
        "contents": [{
            "uri": content.uri,
            "mimeType": content.mime_type,
            "text": content.text
        }]
    })
}

/// Convert resource list to JSON-RPC result format
pub fn resource_list_to_value(resources: &[McpResource]) -> Value {
    serde_json::to_value(serde_json::json!({
        "resources": resources
    }))
    .unwrap_or(serde_json::json!({"resources": []}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // =======================================================
    // TDD Tests - Written FIRST per spec requirements
    // =======================================================

    /// Helper to create a test script
    fn test_script(name: &str, description: Option<&str>) -> Script {
        Script {
            name: name.to_string(),
            path: PathBuf::from(format!(
                "/test/{}.ts",
                name.to_lowercase().replace(' ', "-")
            )),
            extension: "ts".to_string(),
            description: description.map(|s| s.to_string()),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: None,
        }
    }

    /// Helper to create a test scriptlet
    fn test_scriptlet(name: &str, tool: &str, description: Option<&str>) -> Scriptlet {
        Scriptlet {
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            code: "echo test".to_string(),
            tool: tool.to_string(),
            shortcut: None,
            expand: None,
            group: None,
            file_path: None,
            command: None,
            alias: None,
        }
    }

    #[test]
    fn test_resources_list_includes_all() {
        // REQUIREMENT: resources/list returns all three resources
        let resources = get_resource_definitions();

        assert_eq!(resources.len(), 3, "Should have exactly 3 resources");

        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"kit://state"), "Should include kit://state");
        assert!(uris.contains(&"scripts://"), "Should include scripts://");
        assert!(
            uris.contains(&"scriptlets://"),
            "Should include scriptlets://"
        );

        // Verify all have required fields
        for resource in &resources {
            assert!(!resource.name.is_empty(), "Resource should have a name");
            assert_eq!(
                resource.mime_type, "application/json",
                "Should be JSON mime type"
            );
            assert!(resource.description.is_some(), "Should have a description");
        }
    }

    #[test]
    fn test_scripts_resource_read() {
        // REQUIREMENT: scripts:// returns array of script metadata
        let scripts = vec![
            test_script("My Script", Some("Does something")),
            test_script("Another Script", None),
        ];

        let result = read_resource("scripts://", &scripts, &[], None);
        assert!(result.is_ok(), "Should successfully read scripts resource");

        let content = result.unwrap();
        assert_eq!(content.uri, "scripts://");
        assert_eq!(content.mime_type, "application/json");

        // Parse the JSON and verify structure
        let parsed: Vec<ScriptResourceEntry> =
            serde_json::from_str(&content.text).expect("Should be valid JSON array");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "My Script");
        assert_eq!(parsed[0].description, Some("Does something".to_string()));
        assert_eq!(parsed[1].name, "Another Script");
        assert_eq!(parsed[1].description, None);
    }

    #[test]
    fn test_scriptlets_resource_read() {
        // REQUIREMENT: scriptlets:// returns array of scriptlet metadata
        let scriptlets = vec![
            test_scriptlet("Open URL", "open", Some("Opens a URL")),
            test_scriptlet("Paste Text", "paste", None),
        ];

        let result = read_resource("scriptlets://", &[], &scriptlets, None);
        assert!(
            result.is_ok(),
            "Should successfully read scriptlets resource"
        );

        let content = result.unwrap();
        assert_eq!(content.uri, "scriptlets://");
        assert_eq!(content.mime_type, "application/json");

        // Parse the JSON and verify structure
        let parsed: Vec<ScriptletResourceEntry> =
            serde_json::from_str(&content.text).expect("Should be valid JSON array");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Open URL");
        assert_eq!(parsed[0].tool, "open");
        assert_eq!(parsed[0].description, Some("Opens a URL".to_string()));
        assert_eq!(parsed[1].name, "Paste Text");
        assert_eq!(parsed[1].tool, "paste");
    }

    #[test]
    fn test_state_resource_read() {
        // REQUIREMENT: kit://state returns current app state
        let app_state = AppStateResource {
            visible: true,
            focused: true,
            script_count: 10,
            scriptlet_count: 5,
            filter_text: Some("test".to_string()),
            selected_index: Some(3),
        };

        let result = read_resource("kit://state", &[], &[], Some(&app_state));
        assert!(result.is_ok(), "Should successfully read state resource");

        let content = result.unwrap();
        assert_eq!(content.uri, "kit://state");
        assert_eq!(content.mime_type, "application/json");

        // Parse and verify
        let parsed: AppStateResource =
            serde_json::from_str(&content.text).expect("Should be valid JSON");

        assert!(parsed.visible);
        assert!(parsed.focused);
        assert_eq!(parsed.script_count, 10);
        assert_eq!(parsed.scriptlet_count, 5);
        assert_eq!(parsed.filter_text, Some("test".to_string()));
        assert_eq!(parsed.selected_index, Some(3));
    }

    #[test]
    fn test_state_resource_read_default() {
        // When no app state is provided, should return defaults
        let result = read_resource("kit://state", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: AppStateResource = serde_json::from_str(&content.text).unwrap();

        assert!(!parsed.visible);
        assert!(!parsed.focused);
        assert_eq!(parsed.script_count, 0);
        assert_eq!(parsed.scriptlet_count, 0);
        assert_eq!(parsed.filter_text, None);
        assert_eq!(parsed.selected_index, None);
    }

    #[test]
    fn test_unknown_resource_returns_error() {
        // REQUIREMENT: Unknown URI returns error
        let result = read_resource("unknown://resource", &[], &[], None);

        assert!(result.is_err(), "Unknown resource should return error");
        let error = result.unwrap_err();
        assert!(
            error.contains("Resource not found"),
            "Error should mention resource not found"
        );
        assert!(
            error.contains("unknown://resource"),
            "Error should include the URI"
        );
    }

    #[test]
    fn test_resource_content_to_value() {
        let content = ResourceContent {
            uri: "test://uri".to_string(),
            mime_type: "application/json".to_string(),
            text: r#"{"foo":"bar"}"#.to_string(),
        };

        let value = resource_content_to_value(content);

        // Should have contents array
        let contents = value.get("contents").and_then(|c| c.as_array());
        assert!(contents.is_some());

        let contents = contents.unwrap();
        assert_eq!(contents.len(), 1);

        let first = &contents[0];
        assert_eq!(
            first.get("uri").and_then(|u| u.as_str()),
            Some("test://uri")
        );
        assert_eq!(
            first.get("mimeType").and_then(|m| m.as_str()),
            Some("application/json")
        );
    }

    #[test]
    fn test_resource_list_to_value() {
        let resources = get_resource_definitions();
        let value = resource_list_to_value(&resources);

        // Should have resources array
        let resource_array = value.get("resources").and_then(|r| r.as_array());
        assert!(resource_array.is_some());

        let resource_array = resource_array.unwrap();
        assert_eq!(resource_array.len(), 3);

        // First resource should have expected fields
        let first = &resource_array[0];
        assert!(first.get("uri").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("mimeType").is_some());
    }

    // =======================================================
    // Additional Unit Tests
    // =======================================================

    #[test]
    fn test_script_resource_entry_from_script() {
        use crate::schema_parser::{FieldDef, FieldType, Schema};
        use std::collections::HashMap;

        // Script without schema
        let script_no_schema = test_script("No Schema", Some("Test"));
        let entry: ScriptResourceEntry = (&script_no_schema).into();
        assert!(!entry.has_schema);

        // Script with schema
        let mut input = HashMap::new();
        input.insert(
            "name".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: true,
                ..Default::default()
            },
        );

        let script_with_schema = Script {
            name: "With Schema".to_string(),
            path: PathBuf::from("/test/with-schema.ts"),
            extension: "ts".to_string(),
            description: None,
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: Some(Schema {
                input,
                output: HashMap::new(),
            }),
        };

        let entry: ScriptResourceEntry = (&script_with_schema).into();
        assert!(entry.has_schema);
    }

    #[test]
    fn test_scriptlet_resource_entry_from_scriptlet() {
        let scriptlet = Scriptlet {
            name: "Full Scriptlet".to_string(),
            description: Some("Test description".to_string()),
            code: "echo test".to_string(),
            tool: "bash".to_string(),
            shortcut: Some("cmd k".to_string()),
            expand: Some(":test".to_string()),
            group: Some("My Group".to_string()),
            file_path: None,
            command: None,
            alias: None,
        };

        let entry: ScriptletResourceEntry = (&scriptlet).into();

        assert_eq!(entry.name, "Full Scriptlet");
        assert_eq!(entry.description, Some("Test description".to_string()));
        assert_eq!(entry.tool, "bash");
        assert_eq!(entry.shortcut, Some("cmd k".to_string()));
        assert_eq!(entry.expand, Some(":test".to_string()));
        assert_eq!(entry.group, Some("My Group".to_string()));
    }

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = McpResource {
            uri: "test://".to_string(),
            name: "Test".to_string(),
            description: Some("Test description".to_string()),
            mime_type: "application/json".to_string(),
        };

        let json = serde_json::to_string(&resource).unwrap();

        // Should have mimeType (camelCase)
        assert!(json.contains("\"mimeType\""));
        assert!(!json.contains("\"mime_type\""));

        // Deserialize back
        let parsed: McpResource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uri, "test://");
        assert_eq!(parsed.mime_type, "application/json");
    }

    #[test]
    fn test_empty_scripts_resource() {
        let result = read_resource("scripts://", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: Vec<ScriptResourceEntry> = serde_json::from_str(&content.text).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_empty_scriptlets_resource() {
        let result = read_resource("scriptlets://", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: Vec<ScriptletResourceEntry> = serde_json::from_str(&content.text).unwrap();
        assert!(parsed.is_empty());
    }
}

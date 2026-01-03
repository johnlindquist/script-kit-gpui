# MCP and Schema System Expert Bundle

## Executive Summary

This bundle contains the complete MCP (Model Context Protocol) and schema parsing system for Script Kit GPUI. The MCP server enables AI agents and other MCP clients to interact with Script Kit via JSON-RPC 2.0 over HTTP, exposing scripts with schemas as callable tools.

### Key Components:
1. **MCP Server** (`mcp_server.rs`): HTTP server on port 43210 with bearer token authentication
2. **Protocol Handler** (`mcp_protocol.rs`): JSON-RPC 2.0 request/response handling
3. **Schema Parser** (`schema_parser.rs`): Extracts `schema = {...}` and `defineSchema({...})` from TypeScript scripts
4. **Tool Generation** (`mcp_kit_tools.rs`, `mcp_script_tools.rs`): Generates MCP tools from kit/* and scripts/* namespaces
5. **Resources** (`mcp_resources.rs`): Read-only access to app state, scripts list, and scriptlets
6. **Streaming/Audit** (`mcp_streaming.rs`): SSE streaming and audit logging infrastructure

### Architecture Flow:
```
AI Agent -> HTTP POST /rpc -> Bearer Token Auth -> JSON-RPC Parser -> Method Router
                                                                          |
                    +-----------------------------------------------------+
                    |                    |                    |           |
                 initialize          tools/list          tools/call   resources/*
                    |                    |                    |           |
               capabilities       kit/* + scripts/*     execute tool   read data
```

### Files Included:
- `src/mcp_server.rs`: HTTP server, token management, discovery file
- `src/mcp_protocol.rs`: JSON-RPC 2.0 parsing, method routing, error handling
- `src/mcp_kit_tools.rs`: kit/show, kit/hide, kit/state tools
- `src/mcp_script_tools.rs`: Auto-generates tools from scripts with schemas
- `src/mcp_resources.rs`: kit://state, scripts://, scriptlets:// resources
- `src/mcp_streaming.rs`: SSE event formatting, audit logging
- `src/schema_parser.rs`: Schema extraction from TypeScript scripts
- `MCP.md`: Full documentation for MCP integration
- `tests/mcp/`: Test suite and example scripts

---
[Original packx output follows]

# Packx Output

This file contains 18 filtered files from the repository.

## Files

### src/mcp_script_tools.rs

```rs
//! MCP Script Namespace Tools
//!
//! Auto-generates MCP tools from Script Kit scripts that have schema definitions.
//! Scripts with `schema = { input: {...} }` are exposed as `scripts/{script-name}` tools.
//!
//! Example script:
//! ```typescript
//! // Name: Create Note
//! // Description: Creates a new note
//! schema = {
//!   input: {
//!     title: { type: "string", required: true, description: "Note title" },
//!     content: { type: "string", description: "Note content" }
//!   }
//! }
//! ```
//!
//! This becomes MCP tool: `scripts/create-note`
//! With inputSchema derived from schema.input

// Allow dead code - ScriptTool struct and generate_script_tool for future tool execution
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp_kit_tools::ToolDefinition;
use crate::scripts::Script;

/// Represents a Script Kit script as an MCP tool
#[derive(Debug, Clone)]
pub struct ScriptTool {
    /// The script this tool wraps
    pub script: Script,
    /// Tool name in format: scripts/{script-name}
    pub tool_name: String,
    /// JSON Schema for the tool's input
    pub input_schema: Value,
    /// Tool description from script metadata
    pub description: String,
}

/// Result of a script tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptToolResult {
    pub content: Vec<ScriptToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content item in script tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Convert a script name to a tool-friendly slug
/// e.g., "Create Note" -> "create-note"
fn slugify_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Generate an MCP tool definition from a script
///
/// Only scripts with `schema.input` will generate tools.
/// Returns `None` if the script has no schema or no input schema.
///
/// Tool name format: `scripts/{script-name-slug}`
/// Tool description: From script description or fallback to name
/// Input schema: From script's schema.input converted to JSON Schema
pub fn generate_tool_from_script(script: &Script) -> Option<ToolDefinition> {
    // Only scripts with schema.input become tools
    let schema = script.schema.as_ref()?;

    // Skip scripts with empty input schema
    if schema.input.is_empty() {
        return None;
    }

    let tool_name = format!("scripts/{}", slugify_name(&script.name));
    let description = script
        .description
        .clone()
        .unwrap_or_else(|| format!("Run the {} script", script.name));
    let input_schema = schema.to_json_schema_input();

    Some(ToolDefinition {
        name: tool_name,
        description,
        input_schema,
    })
}

/// Generate a ScriptTool from a script (includes the script reference)
pub fn generate_script_tool(script: &Script) -> Option<ScriptTool> {
    let schema = script.schema.as_ref()?;

    if schema.input.is_empty() {
        return None;
    }

    let tool_name = format!("scripts/{}", slugify_name(&script.name));
    let description = script
        .description
        .clone()
        .unwrap_or_else(|| format!("Run the {} script", script.name));
    let input_schema = schema.to_json_schema_input();

    Some(ScriptTool {
        script: script.clone(),
        tool_name,
        input_schema,
        description,
    })
}

/// Get all tool definitions from a list of scripts
/// Only returns scripts that have schema.input defined
pub fn get_script_tool_definitions(scripts: &[Script]) -> Vec<ToolDefinition> {
    scripts
        .iter()
        .filter_map(generate_tool_from_script)
        .collect()
}

/// Check if a tool name is in the scripts/* namespace
pub fn is_script_tool(name: &str) -> bool {
    name.starts_with("scripts/")
}

/// Find a script by its tool name
/// Returns None if tool is not in scripts/* namespace or script not found
pub fn find_script_by_tool_name<'a>(scripts: &'a [Script], tool_name: &str) -> Option<&'a Script> {
    if !is_script_tool(tool_name) {
        return None;
    }

    // Extract the slug from "scripts/{slug}"
    let slug = tool_name.strip_prefix("scripts/")?;

    // Find script where slugified name matches
    scripts.iter().find(|s| slugify_name(&s.name) == slug)
}

/// Handle a scripts/* namespace tool call
///
/// This validates the tool exists and returns a placeholder result.
/// Actual script execution should be handled by the caller using the script path.
pub fn handle_script_tool_call(
    scripts: &[Script],
    tool_name: &str,
    arguments: &Value,
) -> ScriptToolResult {
    // Find the script
    let script = match find_script_by_tool_name(scripts, tool_name) {
        Some(s) => s,
        None => {
            return ScriptToolResult {
                content: vec![ScriptToolContent {
                    content_type: "text".to_string(),
                    text: format!("Script tool not found: {}", tool_name),
                }],
                is_error: Some(true),
            }
        }
    };

    // Return success with script path for execution
    // The actual execution should be done by the caller
    ScriptToolResult {
        content: vec![ScriptToolContent {
            content_type: "text".to_string(),
            text: serde_json::json!({
                "status": "pending",
                "script_path": script.path.to_string_lossy(),
                "arguments": arguments,
                "message": format!("Script '{}' queued for execution", script.name)
            })
            .to_string(),
        }],
        is_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_parser::{FieldDef, FieldType, Schema};
    use std::collections::HashMap;
    use std::path::PathBuf;

    // =======================================================
    // TDD Tests - Written FIRST per spec requirements
    // =======================================================

    /// Helper to create a test script with schema
    fn test_script_with_schema(name: &str, description: Option<&str>, schema: Schema) -> Script {
        Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/test/{}.ts", slugify_name(name))),
            extension: "ts".to_string(),
            description: description.map(|s| s.to_string()),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: Some(schema),
        }
    }

    /// Helper to create a simple schema with one input field
    fn simple_input_schema(field_name: &str, field_type: FieldType, required: bool) -> Schema {
        let mut input = HashMap::new();
        input.insert(
            field_name.to_string(),
            FieldDef {
                field_type,
                required,
                description: Some(format!("The {} field", field_name)),
                ..Default::default()
            },
        );
        Schema {
            input,
            output: HashMap::new(),
        }
    }

    /// Helper to create a script without schema
    fn test_script_without_schema(name: &str) -> Script {
        Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/test/{}.ts", slugify_name(name))),
            extension: "ts".to_string(),
            description: None,
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: None,
        }
    }

    // =======================================================
    // test_generate_tool_from_script_with_schema
    // =======================================================

    #[test]
    fn test_generate_tool_from_script_with_schema() {
        let schema = simple_input_schema("title", FieldType::String, true);
        let script = test_script_with_schema("Create Note", Some("Creates a new note"), schema);

        let tool = generate_tool_from_script(&script);

        assert!(
            tool.is_some(),
            "Script with schema.input should generate tool"
        );
        let tool = tool.unwrap();

        // Verify tool properties
        assert_eq!(tool.name, "scripts/create-note");
        assert_eq!(tool.description, "Creates a new note");

        // Verify input schema structure
        assert_eq!(tool.input_schema["type"], "object");
        assert!(tool.input_schema["properties"]["title"].is_object());
        assert_eq!(tool.input_schema["properties"]["title"]["type"], "string");

        // Verify required fields
        let required = tool.input_schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("title")));
    }

    #[test]
    fn test_generate_tool_from_script_with_multiple_fields() {
        let mut input = HashMap::new();
        input.insert(
            "title".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: true,
                description: Some("Note title".to_string()),
                ..Default::default()
            },
        );
        input.insert(
            "content".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: false,
                description: Some("Note content".to_string()),
                ..Default::default()
            },
        );
        input.insert(
            "priority".to_string(),
            FieldDef {
                field_type: FieldType::Number,
                required: false,
                ..Default::default()
            },
        );

        let schema = Schema {
            input,
            output: HashMap::new(),
        };
        let script = test_script_with_schema("Multi Field", None, schema);

        let tool = generate_tool_from_script(&script);
        assert!(tool.is_some());
        let tool = tool.unwrap();

        // All fields should be in properties
        assert!(tool.input_schema["properties"]["title"].is_object());
        assert!(tool.input_schema["properties"]["content"].is_object());
        assert!(tool.input_schema["properties"]["priority"].is_object());

        // Only required fields should be in required array
        let required = tool.input_schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert!(required.contains(&serde_json::json!("title")));
    }

    // =======================================================
    // test_no_tool_for_script_without_schema
    // =======================================================

    #[test]
    fn test_no_tool_for_script_without_schema() {
        let script = test_script_without_schema("Simple Script");

        let tool = generate_tool_from_script(&script);

        assert!(
            tool.is_none(),
            "Script without schema should not generate tool"
        );
    }

    #[test]
    fn test_no_tool_for_script_with_empty_input_schema() {
        let schema = Schema {
            input: HashMap::new(), // Empty input
            output: HashMap::new(),
        };
        let script = test_script_with_schema("Empty Input", None, schema);

        let tool = generate_tool_from_script(&script);

        assert!(
            tool.is_none(),
            "Script with empty input schema should not generate tool"
        );
    }

    #[test]
    fn test_no_tool_for_script_with_output_only_schema() {
        let mut output = HashMap::new();
        output.insert(
            "result".to_string(),
            FieldDef {
                field_type: FieldType::String,
                ..Default::default()
            },
        );
        let schema = Schema {
            input: HashMap::new(), // No input
            output,
        };
        let script = test_script_with_schema("Output Only", None, schema);

        let tool = generate_tool_from_script(&script);

        assert!(
            tool.is_none(),
            "Script with only output schema should not generate tool"
        );
    }

    // =======================================================
    // test_script_tool_name_format
    // =======================================================

    #[test]
    fn test_script_tool_name_format() {
        let test_cases = vec![
            ("Create Note", "scripts/create-note"),
            ("git-commit", "scripts/git-commit"),
            ("Hello World", "scripts/hello-world"),
            ("test_script", "scripts/test-script"),
            ("UPPERCASE", "scripts/uppercase"),
            ("multi  spaces", "scripts/multi-spaces"),
            ("special@chars!", "scripts/special-chars"),
        ];

        for (name, expected_tool_name) in test_cases {
            let schema = simple_input_schema("x", FieldType::String, false);
            let script = test_script_with_schema(name, None, schema);

            let tool = generate_tool_from_script(&script);
            assert!(tool.is_some(), "Tool should be generated for '{}'", name);
            assert_eq!(
                tool.unwrap().name,
                expected_tool_name,
                "Tool name for '{}' should be '{}'",
                name,
                expected_tool_name
            );
        }
    }

    #[test]
    fn test_script_tool_name_starts_with_scripts_prefix() {
        let schema = simple_input_schema("x", FieldType::String, false);
        let script = test_script_with_schema("Any Script", None, schema);

        let tool = generate_tool_from_script(&script);
        assert!(tool.is_some());
        assert!(
            tool.unwrap().name.starts_with("scripts/"),
            "Tool name must start with 'scripts/' prefix"
        );
    }

    // =======================================================
    // test_script_tool_input_schema
    // =======================================================

    #[test]
    fn test_script_tool_input_schema() {
        let mut input = HashMap::new();
        input.insert(
            "name".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: true,
                description: Some("User name".to_string()),
                ..Default::default()
            },
        );
        input.insert(
            "age".to_string(),
            FieldDef {
                field_type: FieldType::Number,
                required: false,
                min: Some(0.0),
                max: Some(150.0),
                ..Default::default()
            },
        );
        input.insert(
            "active".to_string(),
            FieldDef {
                field_type: FieldType::Boolean,
                required: false,
                default: Some(serde_json::json!(true)),
                ..Default::default()
            },
        );

        let schema = Schema {
            input,
            output: HashMap::new(),
        };
        let script = test_script_with_schema("User Profile", None, schema);

        let tool = generate_tool_from_script(&script);
        assert!(tool.is_some());
        let input_schema = &tool.unwrap().input_schema;

        // Type should be object
        assert_eq!(input_schema["type"], "object");

        // Check string field with description
        let name_schema = &input_schema["properties"]["name"];
        assert_eq!(name_schema["type"], "string");
        assert_eq!(name_schema["description"], "User name");

        // Check number field with constraints
        let age_schema = &input_schema["properties"]["age"];
        assert_eq!(age_schema["type"], "number");
        assert_eq!(age_schema["minimum"], 0.0);
        assert_eq!(age_schema["maximum"], 150.0);

        // Check boolean field with default
        let active_schema = &input_schema["properties"]["active"];
        assert_eq!(active_schema["type"], "boolean");
        assert_eq!(active_schema["default"], true);

        // Required array should only have "name"
        let required = input_schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert!(required.contains(&serde_json::json!("name")));
    }

    #[test]
    fn test_script_tool_input_schema_with_enum() {
        let mut input = HashMap::new();
        input.insert(
            "status".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: true,
                enum_values: Some(vec![
                    "pending".to_string(),
                    "active".to_string(),
                    "completed".to_string(),
                ]),
                ..Default::default()
            },
        );

        let schema = Schema {
            input,
            output: HashMap::new(),
        };
        let script = test_script_with_schema("Status Update", None, schema);

        let tool = generate_tool_from_script(&script);
        assert!(tool.is_some());
        let input_schema = &tool.unwrap().input_schema;

        let status_enum = input_schema["properties"]["status"]["enum"].as_array();
        assert!(status_enum.is_some());
        let enum_values = status_enum.unwrap();
        assert_eq!(enum_values.len(), 3);
        assert!(enum_values.contains(&serde_json::json!("pending")));
        assert!(enum_values.contains(&serde_json::json!("active")));
        assert!(enum_values.contains(&serde_json::json!("completed")));
    }

    // =======================================================
    // Additional helper function tests
    // =======================================================

    #[test]
    fn test_is_script_tool() {
        assert!(is_script_tool("scripts/create-note"));
        assert!(is_script_tool("scripts/git-commit"));
        assert!(is_script_tool("scripts/any-name"));

        assert!(!is_script_tool("kit/show"));
        assert!(!is_script_tool("tools/list"));
        assert!(!is_script_tool("scriptsshow")); // No slash
    }

    #[test]
    fn test_get_script_tool_definitions() {
        let schema1 = simple_input_schema("x", FieldType::String, false);
        let schema2 = simple_input_schema("y", FieldType::Number, true);

        let scripts = vec![
            test_script_with_schema("Script One", None, schema1),
            test_script_without_schema("Script Two"), // No schema
            test_script_with_schema("Script Three", None, schema2),
        ];

        let tools = get_script_tool_definitions(&scripts);

        assert_eq!(
            tools.len(),
            2,
            "Only scripts with schema should generate tools"
        );
        assert!(tools.iter().any(|t| t.name == "scripts/script-one"));
        assert!(tools.iter().any(|t| t.name == "scripts/script-three"));
    }

    #[test]
    fn test_find_script_by_tool_name() {
        let schema = simple_input_schema("x", FieldType::String, false);
        let scripts = vec![
            test_script_with_schema("Create Note", None, schema.clone()),
            test_script_with_schema("Git Commit", None, schema.clone()),
        ];

        // Should find by tool name
        let found = find_script_by_tool_name(&scripts, "scripts/create-note");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Create Note");

        // Should find different script
        let found = find_script_by_tool_name(&scripts, "scripts/git-commit");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Git Commit");

        // Should not find non-existent script
        let found = find_script_by_tool_name(&scripts, "scripts/unknown");
        assert!(found.is_none());

        // Should not find kit/* namespace
        let found = find_script_by_tool_name(&scripts, "kit/show");
        assert!(found.is_none());
    }

    #[test]
    fn test_handle_script_tool_call_success() {
        let schema = simple_input_schema("title", FieldType::String, true);
        let scripts = vec![test_script_with_schema(
            "Create Note",
            Some("Creates notes"),
            schema,
        )];

        let result = handle_script_tool_call(
            &scripts,
            "scripts/create-note",
            &serde_json::json!({"title": "Test Note"}),
        );

        assert!(
            result.is_error.is_none() || result.is_error == Some(false),
            "Should succeed for valid tool"
        );
        assert!(!result.content.is_empty());

        // Result should contain script path
        let text = &result.content[0].text;
        assert!(text.contains("create-note"));
    }

    #[test]
    fn test_handle_script_tool_call_not_found() {
        let scripts = vec![];

        let result = handle_script_tool_call(&scripts, "scripts/unknown", &serde_json::json!({}));

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("not found"));
    }

    #[test]
    fn test_script_tool_description_fallback() {
        let schema = simple_input_schema("x", FieldType::String, false);
        let script = test_script_with_schema("My Script", None, schema);

        let tool = generate_tool_from_script(&script);
        assert!(tool.is_some());

        // Should have fallback description when none provided
        let tool = tool.unwrap();
        assert!(tool.description.contains("My Script"));
        assert!(tool.description.contains("Run"));
    }

    #[test]
    fn test_generate_script_tool_struct() {
        let schema = simple_input_schema("title", FieldType::String, true);
        let script = test_script_with_schema("Test Script", Some("Description"), schema);

        let script_tool = generate_script_tool(&script);
        assert!(script_tool.is_some());

        let tool = script_tool.unwrap();
        assert_eq!(tool.tool_name, "scripts/test-script");
        assert_eq!(tool.description, "Description");
        assert_eq!(tool.script.name, "Test Script");
    }

    #[test]
    fn test_slugify_name() {
        assert_eq!(slugify_name("Hello World"), "hello-world");
        assert_eq!(slugify_name("git-commit"), "git-commit");
        assert_eq!(slugify_name("test_script"), "test-script");
        assert_eq!(slugify_name("UPPER CASE"), "upper-case");
        assert_eq!(slugify_name("multi  spaces"), "multi-spaces");
        assert_eq!(slugify_name("special@#$chars"), "special-chars");
        assert_eq!(slugify_name("---leading-trailing---"), "leading-trailing");
    }
}

```

### src/schema_parser.rs

```rs
//! Schema parser for Script Kit scripts
#![allow(dead_code)]
//!
//! Parses the `schema = { input: {...}, output: {...} }` global from scripts.
//! This defines the typed interface for input() and output() functions,
//! enabling MCP tool generation and AI agent integration.
//!
//! Example script with schema:
//! ```typescript
//! schema = {
//!   input: {
//!     title: { type: "string", required: true, description: "Note title" },
//!     tags: { type: "array", items: "string", description: "Tags for the note" }
//!   },
//!   output: {
//!     path: { type: "string", description: "Path to created file" },
//!     wordCount: { type: "number" }
//!   }
//! }
//!
//! const { title, tags } = await input();
//! // ... create note ...
//! output({ path: notePath, wordCount: content.split(' ').length });
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Supported field types for schema definitions
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    #[default]
    String,
    Number,
    Boolean,
    Array,
    Object,
    /// Any type - no validation
    Any,
}

/// Definition of a single field in the schema
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    /// The type of this field
    #[serde(rename = "type", default)]
    pub field_type: FieldType,

    /// Whether this field is required (defaults to false)
    #[serde(default)]
    pub required: bool,

    /// Human-readable description for AI agents and documentation
    pub description: Option<String>,

    /// Default value if not provided
    pub default: Option<serde_json::Value>,

    /// For array types, the type of items
    pub items: Option<String>,

    /// For object types, nested field definitions
    pub properties: Option<HashMap<String, FieldDef>>,

    /// Enum values (for string fields with limited options)
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,

    /// Minimum value (for numbers) or length (for strings/arrays)
    pub min: Option<f64>,

    /// Maximum value (for numbers) or length (for strings/arrays)
    pub max: Option<f64>,

    /// Regex pattern for validation (strings only)
    pub pattern: Option<String>,

    /// Example value for documentation
    pub example: Option<serde_json::Value>,
}

/// Full schema definition with input and output sections
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    /// Input fields - what the script expects to receive
    #[serde(default)]
    pub input: HashMap<String, FieldDef>,

    /// Output fields - what the script will produce
    #[serde(default)]
    pub output: HashMap<String, FieldDef>,
}

/// Result of parsing a script file for schema
#[derive(Debug, Clone)]
pub struct SchemaParseResult {
    /// The parsed schema, if found
    pub schema: Option<Schema>,
    /// Any parse errors encountered (non-fatal)
    pub errors: Vec<String>,
    /// The byte range where schema was found
    pub span: Option<(usize, usize)>,
}

/// Extract schema from script content
///
/// Looks for `schema = { ... }` at the top level of the script.
/// The schema object must contain `input` and/or `output` sections.
///
/// Returns `SchemaParseResult` with the parsed schema and any errors.
pub fn extract_schema(content: &str) -> SchemaParseResult {
    let mut result = SchemaParseResult {
        schema: None,
        errors: vec![],
        span: None,
    };

    // Find `schema = ` or `schema=` pattern
    let schema_pattern = find_schema_assignment(content);

    if let Some((start_idx, obj_start)) = schema_pattern {
        // Extract the object literal
        match extract_object_literal(content, obj_start) {
            Ok((json_str, end_idx)) => {
                result.span = Some((start_idx, end_idx));

                // Normalize and parse
                let normalized = normalize_js_object(&json_str);

                match serde_json::from_str::<Schema>(&normalized) {
                    Ok(schema) => {
                        debug!(
                            input_fields = schema.input.len(),
                            output_fields = schema.output.len(),
                            "Parsed schema"
                        );
                        result.schema = Some(schema);
                    }
                    Err(e) => {
                        result
                            .errors
                            .push(format!("Failed to parse schema JSON: {}", e));
                    }
                }
            }
            Err(e) => {
                result.errors.push(e);
            }
        }
    }

    result
}

/// Find the `schema = ` assignment or `defineSchema({` call in the content
fn find_schema_assignment(content: &str) -> Option<(usize, usize)> {
    // First try direct assignment patterns: schema = { ... }
    let assignment_patterns = ["schema=", "schema =", "schema  ="];

    for pattern in assignment_patterns {
        if let Some(idx) = content.find(pattern) {
            let after_eq = idx + pattern.len();
            let rest = &content[after_eq..];

            for (i, c) in rest.char_indices() {
                if c == '{' {
                    return Some((idx, after_eq + i));
                } else if !c.is_whitespace() {
                    break;
                }
            }
        }
    }

    // Then try defineSchema() function pattern: defineSchema({ ... })
    let define_patterns = ["defineSchema({", "defineSchema ({", "defineSchema  ({"];

    for pattern in define_patterns {
        if let Some(idx) = content.find(pattern) {
            // Find the opening brace after defineSchema
            let after_define = idx + pattern.len() - 1; // -1 because pattern includes '{'
            return Some((idx, after_define));
        }
    }

    None
}

/// Extract a balanced object literal starting at the given index
fn extract_object_literal(content: &str, start: usize) -> Result<(String, usize), String> {
    let bytes = content.as_bytes();
    if start >= bytes.len() || bytes[start] != b'{' {
        return Err("Expected '{' at start of object".to_string());
    }

    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut string_char = '"';

    for (i, &byte) in bytes[start..].iter().enumerate() {
        let c = byte as char;

        if escape_next {
            escape_next = false;
            continue;
        }

        if in_string {
            if c == '\\' {
                escape_next = true;
            } else if c == string_char {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' | '\'' | '`' => {
                in_string = true;
                string_char = c;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + i + 1;
                    return Ok((content[start..end].to_string(), end));
                }
            }
            _ => {}
        }
    }

    Err("Unbalanced braces in schema object".to_string())
}

/// Normalize JavaScript object literal to valid JSON
fn normalize_js_object(js: &str) -> String {
    let mut result = String::with_capacity(js.len());
    let chars: Vec<char> = js.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut string_char = '"';

    while i < len {
        let c = chars[i];

        if in_string {
            if c == '\\' && i + 1 < len {
                result.push(c);
                result.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == string_char {
                in_string = false;
                result.push('"');
                i += 1;
                continue;
            }
            result.push(c);
            i += 1;
            continue;
        }

        if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
            result.push('"');
            i += 1;
            continue;
        }

        // Skip comments
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        // Handle trailing commas
        if c == ',' {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == ']' || chars[j] == '}') {
                i += 1;
                continue;
            }
        }

        // Handle unquoted keys
        if c.is_alphabetic() || c == '_' || c == '$' {
            let mut key_end = i;
            while key_end < len
                && (chars[key_end].is_alphanumeric()
                    || chars[key_end] == '_'
                    || chars[key_end] == '$')
            {
                key_end += 1;
            }

            let mut colon_pos = key_end;
            while colon_pos < len && chars[colon_pos].is_whitespace() {
                colon_pos += 1;
            }

            if colon_pos < len && chars[colon_pos] == ':' {
                let key: String = chars[i..key_end].iter().collect();
                result.push('"');
                result.push_str(&key);
                result.push('"');
                i = key_end;
                continue;
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

/// Generate JSON Schema from our Schema definition
/// Useful for validation and MCP tool definitions
impl Schema {
    /// Convert to JSON Schema format for the input section
    pub fn to_json_schema_input(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, field) in &self.input {
            properties.insert(name.clone(), field_to_json_schema(field));
            if field.required {
                required.push(serde_json::Value::String(name.clone()));
            }
        }

        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        })
    }

    /// Convert to JSON Schema format for the output section
    pub fn to_json_schema_output(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, field) in &self.output {
            properties.insert(name.clone(), field_to_json_schema(field));
            if field.required {
                required.push(serde_json::Value::String(name.clone()));
            }
        }

        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        })
    }
}

fn field_to_json_schema(field: &FieldDef) -> serde_json::Value {
    let mut schema = serde_json::Map::new();

    let type_str = match field.field_type {
        FieldType::String => "string",
        FieldType::Number => "number",
        FieldType::Boolean => "boolean",
        FieldType::Array => "array",
        FieldType::Object => "object",
        FieldType::Any => "any",
    };
    schema.insert(
        "type".to_string(),
        serde_json::Value::String(type_str.to_string()),
    );

    if let Some(desc) = &field.description {
        schema.insert(
            "description".to_string(),
            serde_json::Value::String(desc.clone()),
        );
    }

    if let Some(default) = &field.default {
        schema.insert("default".to_string(), default.clone());
    }

    if let Some(enum_vals) = &field.enum_values {
        let vals: Vec<serde_json::Value> = enum_vals
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect();
        schema.insert("enum".to_string(), serde_json::Value::Array(vals));
    }

    if let Some(min) = field.min {
        if matches!(field.field_type, FieldType::Number) {
            schema.insert("minimum".to_string(), serde_json::json!(min));
        } else {
            schema.insert("minLength".to_string(), serde_json::json!(min as i64));
        }
    }

    if let Some(max) = field.max {
        if matches!(field.field_type, FieldType::Number) {
            schema.insert("maximum".to_string(), serde_json::json!(max));
        } else {
            schema.insert("maxLength".to_string(), serde_json::json!(max as i64));
        }
    }

    if let Some(pattern) = &field.pattern {
        schema.insert(
            "pattern".to_string(),
            serde_json::Value::String(pattern.clone()),
        );
    }

    if let Some(items) = &field.items {
        schema.insert("items".to_string(), serde_json::json!({"type": items}));
    }

    if let Some(props) = &field.properties {
        let mut prop_schemas = serde_json::Map::new();
        for (name, prop_field) in props {
            prop_schemas.insert(name.clone(), field_to_json_schema(prop_field));
        }
        schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(prop_schemas),
        );
    }

    if let Some(example) = &field.example {
        schema.insert("example".to_string(), example.clone());
    }

    serde_json::Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_schema() {
        let content = r#"
schema = {
    input: {
        title: { type: "string", required: true, description: "The title" }
    },
    output: {
        result: { type: "string" }
    }
}
"#;
        let result = extract_schema(content);
        assert!(result.schema.is_some(), "Errors: {:?}", result.errors);
        let schema = result.schema.unwrap();

        assert_eq!(schema.input.len(), 1);
        assert_eq!(schema.output.len(), 1);

        let title_field = schema.input.get("title").unwrap();
        assert_eq!(title_field.field_type, FieldType::String);
        assert!(title_field.required);
        assert_eq!(title_field.description, Some("The title".to_string()));
    }

    #[test]
    fn test_parse_all_field_types() {
        let content = r#"
schema = {
    input: {
        name: { type: "string" },
        count: { type: "number" },
        enabled: { type: "boolean" },
        items: { type: "array", items: "string" },
        config: { type: "object" },
        anything: { type: "any" }
    }
}
"#;
        let result = extract_schema(content);
        let schema = result.schema.unwrap();

        assert_eq!(
            schema.input.get("name").unwrap().field_type,
            FieldType::String
        );
        assert_eq!(
            schema.input.get("count").unwrap().field_type,
            FieldType::Number
        );
        assert_eq!(
            schema.input.get("enabled").unwrap().field_type,
            FieldType::Boolean
        );
        assert_eq!(
            schema.input.get("items").unwrap().field_type,
            FieldType::Array
        );
        assert_eq!(
            schema.input.get("config").unwrap().field_type,
            FieldType::Object
        );
        assert_eq!(
            schema.input.get("anything").unwrap().field_type,
            FieldType::Any
        );
    }

    #[test]
    fn test_parse_field_constraints() {
        let content = r#"
schema = {
    input: {
        username: {
            type: "string",
            required: true,
            min: 3,
            max: 20,
            pattern: "^[a-z]+$"
        },
        age: {
            type: "number",
            min: 0,
            max: 150
        },
        status: {
            type: "string",
            enum: ["active", "inactive", "pending"]
        }
    }
}
"#;
        let result = extract_schema(content);
        let schema = result.schema.unwrap();

        let username = schema.input.get("username").unwrap();
        assert_eq!(username.min, Some(3.0));
        assert_eq!(username.max, Some(20.0));
        assert_eq!(username.pattern, Some("^[a-z]+$".to_string()));

        let status = schema.input.get("status").unwrap();
        assert_eq!(
            status.enum_values,
            Some(vec![
                "active".to_string(),
                "inactive".to_string(),
                "pending".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_with_defaults_and_examples() {
        let content = r#"
schema = {
    input: {
        count: {
            type: "number",
            default: 10,
            example: 42
        }
    }
}
"#;
        let result = extract_schema(content);
        let schema = result.schema.unwrap();

        let count = schema.input.get("count").unwrap();
        assert_eq!(count.default, Some(serde_json::json!(10)));
        assert_eq!(count.example, Some(serde_json::json!(42)));
    }

    #[test]
    fn test_parse_no_schema() {
        let content = r#"
// Just a regular script
const x = await arg("Pick");
"#;
        let result = extract_schema(content);
        assert!(result.schema.is_none());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_parse_input_only() {
        let content = r#"
schema = {
    input: {
        name: { type: "string" }
    }
}
"#;
        let result = extract_schema(content);
        let schema = result.schema.unwrap();
        assert_eq!(schema.input.len(), 1);
        assert_eq!(schema.output.len(), 0);
    }

    #[test]
    fn test_parse_output_only() {
        let content = r#"
schema = {
    output: {
        result: { type: "string" }
    }
}
"#;
        let result = extract_schema(content);
        let schema = result.schema.unwrap();
        assert_eq!(schema.input.len(), 0);
        assert_eq!(schema.output.len(), 1);
    }

    #[test]
    fn test_to_json_schema() {
        let content = r#"
schema = {
    input: {
        title: { type: "string", required: true, description: "Title" },
        count: { type: "number", required: false }
    }
}
"#;
        let result = extract_schema(content);
        let schema = result.schema.unwrap();

        let json_schema = schema.to_json_schema_input();

        assert_eq!(json_schema["type"], "object");
        assert!(json_schema["properties"]["title"].is_object());
        assert_eq!(json_schema["properties"]["title"]["type"], "string");

        let required = json_schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("title")));
        assert!(!required.contains(&serde_json::json!("count")));
    }

    #[test]
    fn test_parse_trailing_commas() {
        let content = r#"
schema = {
    input: {
        name: { type: "string", },
    },
    output: {
        result: { type: "boolean", },
    },
}
"#;
        let result = extract_schema(content);
        assert!(result.schema.is_some(), "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_parse_single_quotes() {
        let content = r#"
schema = {
    input: {
        name: { type: 'string', description: 'The name' }
    }
}
"#;
        let result = extract_schema(content);
        assert!(result.schema.is_some());
        let schema = result.schema.unwrap();
        assert_eq!(
            schema.input.get("name").unwrap().description,
            Some("The name".to_string())
        );
    }

    #[test]
    fn test_span_tracking() {
        let content = r#"// Header
schema = { input: { x: { type: "string" } } }
const y = 1;"#;
        let result = extract_schema(content);
        assert!(result.span.is_some());
        let (start, end) = result.span.unwrap();
        let extracted = &content[start..end];
        assert!(extracted.contains("schema"));
        assert!(extracted.contains("input"));
    }

    #[test]
    fn test_invalid_schema_reports_error() {
        let content = r#"
schema = {
    input: {
        bad: { type: "invalid_type" }
    }
}
"#;
        let result = extract_schema(content);
        // serde should error on unknown enum variant
        assert!(result.schema.is_none() || result.schema.as_ref().unwrap().input.is_empty());
    }

    // TDD: Test for defineSchema() function pattern
    // This pattern should also work for MCP tool detection
    #[test]
    fn test_parse_define_schema_function() {
        let content = r#"
import "@johnlindquist/kit"

const { input, output } = defineSchema({
    input: {
        greeting: { type: "string", required: true, description: "Greeting message" },
        count: { type: "number" }
    },
    output: {
        message: { type: "string", description: "Response message" }
    }
} as const)

const { greeting } = await input()
output({ message: `Hello ${greeting}!` })
"#;
        let result = extract_schema(content);
        assert!(
            result.schema.is_some(),
            "defineSchema() should be parseable. Errors: {:?}",
            result.errors
        );

        let schema = result.schema.unwrap();
        assert_eq!(schema.input.len(), 2, "Should have 2 input fields");
        assert_eq!(schema.output.len(), 1, "Should have 1 output field");

        // Verify input fields
        let greeting = schema
            .input
            .get("greeting")
            .expect("Should have greeting field");
        assert!(greeting.required, "greeting should be required");
        assert_eq!(greeting.description, Some("Greeting message".to_string()));

        let count = schema.input.get("count").expect("Should have count field");
        assert!(!count.required, "count should not be required");

        // Verify output fields
        let message = schema
            .output
            .get("message")
            .expect("Should have message field");
        assert_eq!(message.description, Some("Response message".to_string()));
    }

    // TDD: Test that both patterns work (direct assignment and defineSchema)
    #[test]
    fn test_parse_both_schema_patterns() {
        // Direct assignment pattern
        let direct = r#"
schema = {
    input: { name: { type: "string", required: true } }
}
"#;
        let result1 = extract_schema(direct);
        assert!(result1.schema.is_some(), "Direct assignment should work");

        // defineSchema function pattern
        let define_fn = r#"
const { input, output } = defineSchema({
    input: { name: { type: "string", required: true } }
} as const)
"#;
        let result2 = extract_schema(define_fn);
        assert!(result2.schema.is_some(), "defineSchema() should work");

        // Both should produce same schema
        let schema1 = result1.schema.unwrap();
        let schema2 = result2.schema.unwrap();
        assert_eq!(schema1.input.len(), schema2.input.len());
        assert_eq!(
            schema1.input.get("name").unwrap().required,
            schema2.input.get("name").unwrap().required
        );
    }
}

```

### src/mcp_server.rs

```rs
//! MCP Server Foundation
//!
//! Provides an HTTP server for MCP (Model Context Protocol) integration.
//! Features:
//! - HTTP server on localhost:43210
//! - Bearer token authentication from ~/.sk/kit/agent-token
//! - Health endpoint at GET /health
//! - Discovery file at ~/.sk/kit/server.json

// Allow dead code - ServerHandle methods provide full lifecycle API for future use
#![allow(dead_code)]

use crate::mcp_protocol::{self, JsonRpcResponse};
use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Default port for the MCP server
pub const DEFAULT_PORT: u16 = 43210;

/// MCP Server version for discovery
pub const VERSION: &str = "0.1.0";

/// Server capabilities advertised in discovery
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerCapabilities {
    pub scripts: bool,
    pub prompts: bool,
    pub tools: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            scripts: true,
            prompts: true,
            tools: true,
        }
    }
}

/// Discovery file structure written to ~/.sk/kit/server.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveryInfo {
    pub url: String,
    pub version: String,
    pub capabilities: ServerCapabilities,
}

/// MCP HTTP Server
///
/// Lightweight HTTP server for MCP protocol communication.
/// Uses std::net for simplicity (no async runtime required).
pub struct McpServer {
    port: u16,
    token: String,
    running: Arc<AtomicBool>,
    kenv_path: PathBuf,
}

impl McpServer {
    /// Create a new MCP server instance
    ///
    /// # Arguments
    /// * `port` - Port to listen on (default: 43210)
    /// * `kenv_path` - Path to ~/.sk/kit directory
    pub fn new(port: u16, kenv_path: PathBuf) -> Result<Self> {
        let token = Self::load_or_create_token(&kenv_path)?;

        Ok(Self {
            port,
            token,
            running: Arc::new(AtomicBool::new(false)),
            kenv_path,
        })
    }

    /// Create server with default settings
    pub fn with_defaults() -> Result<Self> {
        let kenv_path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".kenv");
        Self::new(DEFAULT_PORT, kenv_path)
    }

    /// Load existing token or create a new one
    fn load_or_create_token(kenv_path: &PathBuf) -> Result<String> {
        let token_path = kenv_path.join("agent-token");

        if token_path.exists() {
            let token = fs::read_to_string(&token_path)
                .context("Failed to read agent-token file")?
                .trim()
                .to_string();

            if !token.is_empty() {
                info!("Loaded existing agent token from {:?}", token_path);
                return Ok(token);
            }
        }

        // Generate new token
        let token = uuid::Uuid::new_v4().to_string();

        // Ensure kenv directory exists
        fs::create_dir_all(kenv_path).context("Failed to create .kenv directory")?;

        fs::write(&token_path, &token).context("Failed to write agent-token file")?;

        info!("Generated new agent token at {:?}", token_path);
        Ok(token)
    }

    /// Get the authentication token
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Write discovery file to ~/.sk/kit/server.json
    fn write_discovery_file(&self) -> Result<()> {
        let discovery = DiscoveryInfo {
            url: self.url(),
            version: VERSION.to_string(),
            capabilities: ServerCapabilities::default(),
        };

        let discovery_path = self.kenv_path.join("server.json");
        let json = serde_json::to_string_pretty(&discovery)
            .context("Failed to serialize discovery info")?;

        fs::write(&discovery_path, json).context("Failed to write server.json")?;

        info!("Wrote discovery file to {:?}", discovery_path);
        Ok(())
    }

    /// Remove discovery file on shutdown
    fn remove_discovery_file(&self) {
        let discovery_path = self.kenv_path.join("server.json");
        if discovery_path.exists() {
            if let Err(e) = fs::remove_file(&discovery_path) {
                warn!("Failed to remove discovery file: {}", e);
            } else {
                debug!("Removed discovery file");
            }
        }
    }

    /// Start the HTTP server in a background thread
    ///
    /// Returns a handle that can be used to stop the server.
    pub fn start(&self) -> Result<ServerHandle> {
        if self.is_running() {
            anyhow::bail!("Server is already running");
        }

        // Write discovery file before starting
        self.write_discovery_file()?;

        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))
            .with_context(|| format!("Failed to bind to port {}", self.port))?;

        // Set non-blocking for graceful shutdown
        listener
            .set_nonblocking(true)
            .context("Failed to set non-blocking mode")?;

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let token = self.token.clone();
        let kenv_path = self.kenv_path.clone();

        let handle = thread::spawn(move || {
            info!("MCP server started on port {}", DEFAULT_PORT);

            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        debug!("Connection from {}", addr);
                        let token = token.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_connection(stream, &token) {
                                error!("Error handling connection: {}", e);
                            }
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No connection available, sleep briefly
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }

            // Cleanup on shutdown
            let discovery_path = kenv_path.join("server.json");
            if discovery_path.exists() {
                let _ = fs::remove_file(&discovery_path);
            }

            info!("MCP server stopped");
        });

        Ok(ServerHandle {
            running: self.running.clone(),
            thread: Some(handle),
        })
    }

    /// Stop the server
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.remove_discovery_file();
    }
}

/// Handle for controlling the running server
pub struct ServerHandle {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ServerHandle {
    /// Stop the server and wait for it to finish
    pub fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Check if server is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Note: we don't join here to avoid blocking on drop
    }
}

/// Handle a single HTTP connection
fn handle_connection(mut stream: TcpStream, expected_token: &str) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    // Read request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let request_line = request_line.trim();

    debug!("Request: {}", request_line);

    // Parse method and path
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return send_response(&mut stream, 400, "Bad Request", "Invalid request line");
    }

    let method = parts[0];
    let path = parts[1];

    // Read headers
    let mut headers = std::collections::HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), value.trim().to_string());
        }
    }

    // Check authorization for non-health endpoints
    if path != "/health" {
        let auth_valid = headers
            .get("authorization")
            .map(|auth| {
                auth.strip_prefix("Bearer ")
                    .map(|token| token == expected_token)
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if !auth_valid {
            return send_response(&mut stream, 401, "Unauthorized", "Invalid or missing token");
        }
    }

    // Route request
    match (method, path) {
        ("GET", "/health") => send_response(&mut stream, 200, "OK", r#"{"status":"healthy"}"#),
        ("GET", "/") => {
            let info = serde_json::json!({
                "name": "Script Kit MCP Server",
                "version": VERSION,
                "capabilities": ServerCapabilities::default(),
            });
            send_response(&mut stream, 200, "OK", &info.to_string())
        }
        ("POST", "/rpc") => {
            // Handle JSON-RPC request
            handle_rpc_request(&mut reader, &mut stream, &headers)
        }
        _ => send_response(&mut stream, 404, "Not Found", "Endpoint not found"),
    }
}

/// Handle a JSON-RPC request on the /rpc endpoint
fn handle_rpc_request(
    reader: &mut BufReader<TcpStream>,
    stream: &mut TcpStream,
    headers: &std::collections::HashMap<String, String>,
) -> Result<()> {
    // Get Content-Length
    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if content_length == 0 {
        let response = JsonRpcResponse::error(
            serde_json::Value::Null,
            mcp_protocol::error_codes::INVALID_REQUEST,
            "Missing or invalid Content-Length header",
        );
        let body = serde_json::to_string(&response)?;
        return send_response(stream, 400, "Bad Request", &body);
    }

    // Read request body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    let body_str = String::from_utf8_lossy(&body);

    debug!("RPC request body: {}", body_str);

    // Load scripts and scriptlets for context-aware responses
    // This allows resources/read and tools/list to return actual data
    let scripts = crate::scripts::read_scripts();
    let scriptlets = crate::scripts::load_scriptlets();

    // Parse and handle request with full context
    let response = match mcp_protocol::parse_request(&body_str) {
        Ok(request) => {
            mcp_protocol::handle_request_with_context(request, &scripts, &scriptlets, None)
        }
        Err(error_response) => error_response,
    };

    let response_body = serde_json::to_string(&response)?;
    send_response(stream, 200, "OK", &response_body)
}

/// Send an HTTP response
fn send_response(stream: &mut TcpStream, status: u16, reason: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        reason,
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use tempfile::TempDir;

    /// Helper to create a server with a temporary kenv directory
    fn create_test_server(port: u16) -> (McpServer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let server = McpServer::new(port, temp_dir.path().to_path_buf()).unwrap();
        (server, temp_dir)
    }

    /// Helper to send an HTTP request and get the response
    fn http_request(port: u16, method: &str, path: &str, token: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        let mut request = format!("{} {} HTTP/1.1\r\nHost: localhost\r\n", method, path);
        if let Some(token) = token {
            request.push_str(&format!("Authorization: Bearer {}\r\n", token));
        }
        request.push_str("\r\n");

        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        // Parse status code from response
        let status_line = response.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get body (after blank line)
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        (status_code, body)
    }

    #[test]
    fn test_server_starts_and_stops() {
        let (server, _temp_dir) = create_test_server(43211);

        // Server should not be running initially
        assert!(!server.is_running());

        // Start server
        let handle = server.start().unwrap();

        // Give server time to start
        thread::sleep(std::time::Duration::from_millis(100));

        // Server should be running
        assert!(handle.is_running());

        // Stop server
        handle.stop();

        // Server should stop
        assert!(!server.is_running());
    }

    #[test]
    fn test_health_endpoint_returns_200() {
        let (server, _temp_dir) = create_test_server(43212);
        let _handle = server.start().unwrap();

        // Give server time to start
        thread::sleep(std::time::Duration::from_millis(100));

        let (status, body) = http_request(43212, "GET", "/health", None);

        assert_eq!(status, 200);
        assert!(body.contains("healthy"));
    }

    #[test]
    fn test_auth_rejects_invalid_token() {
        let (server, _temp_dir) = create_test_server(43213);
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        // Request to root without token should fail
        let (status, _) = http_request(43213, "GET", "/", None);
        assert_eq!(status, 401);

        // Request with wrong token should fail
        let (status, _) = http_request(43213, "GET", "/", Some("wrong-token"));
        assert_eq!(status, 401);
    }

    #[test]
    fn test_auth_accepts_valid_token() {
        let (server, _temp_dir) = create_test_server(43214);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let (status, body) = http_request(43214, "GET", "/", Some(&token));

        assert_eq!(status, 200);
        assert!(body.contains("Script Kit MCP Server"));
    }

    #[test]
    fn test_discovery_file_created() {
        let (server, temp_dir) = create_test_server(43215);

        // Discovery file should not exist before start
        let discovery_path = temp_dir.path().join("server.json");
        assert!(!discovery_path.exists());

        // Start server
        let handle = server.start().unwrap();
        thread::sleep(std::time::Duration::from_millis(100));

        // Discovery file should exist
        assert!(discovery_path.exists());

        // Verify contents
        let content = fs::read_to_string(&discovery_path).unwrap();
        let discovery: DiscoveryInfo = serde_json::from_str(&content).unwrap();

        assert!(discovery.url.contains("43215"));
        assert_eq!(discovery.version, VERSION);
        assert!(discovery.capabilities.scripts);

        // Stop server
        handle.stop();

        // Discovery file should be removed after stop
        thread::sleep(std::time::Duration::from_millis(100));
        assert!(!discovery_path.exists());
    }

    #[test]
    fn test_generates_token_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let token_path = temp_dir.path().join("agent-token");

        // Token file should not exist
        assert!(!token_path.exists());

        // Create server - should generate token
        let server = McpServer::new(43216, temp_dir.path().to_path_buf()).unwrap();

        // Token file should now exist
        assert!(token_path.exists());

        // Token should be a valid UUID-like string
        let token = server.token();
        assert!(!token.is_empty());
        assert!(token.len() >= 32); // UUID v4 format

        // Token should match file contents
        let file_token = fs::read_to_string(&token_path).unwrap();
        assert_eq!(token, file_token.trim());

        // Creating another server should use the same token
        let server2 = McpServer::new(43217, temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(server.token(), server2.token());
    }

    #[test]
    fn test_url_format() {
        let (server, _temp_dir) = create_test_server(43218);
        assert_eq!(server.url(), "http://localhost:43218");
    }

    /// Helper to send a POST request with a JSON body
    fn http_post_json(port: u16, path: &str, token: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        let request = format!(
            "POST {} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Authorization: Bearer {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            path,
            token,
            body.len(),
            body
        );

        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        // Parse status code from response
        let status_line = response.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Get body (after blank line)
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

        (status_code, body)
    }

    #[test]
    fn test_rpc_endpoint_tools_list() {
        let (server, _temp_dir) = create_test_server(43219);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let (status, body) = http_post_json(43219, "/rpc", &token, request);

        assert_eq!(status, 200);

        // Parse response
        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response["result"]["tools"].is_array());
    }

    #[test]
    fn test_rpc_endpoint_initialize() {
        let (server, _temp_dir) = create_test_server(43220);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{}}"#;
        let (status, body) = http_post_json(43220, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], "init-1");
        assert!(response["result"]["serverInfo"]["name"].is_string());
        assert!(response["result"]["capabilities"].is_object());
    }

    #[test]
    fn test_rpc_endpoint_method_not_found() {
        let (server, _temp_dir) = create_test_server(43221);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":99,"method":"unknown/method","params":{}}"#;
        let (status, body) = http_post_json(43221, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 99);
        assert_eq!(response["error"]["code"], -32601);
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Method not found"));
    }

    #[test]
    fn test_rpc_endpoint_invalid_json() {
        let (server, _temp_dir) = create_test_server(43222);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0", invalid}"#;
        let (status, body) = http_post_json(43222, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["error"]["code"], -32700); // Parse error
    }

    #[test]
    fn test_rpc_endpoint_requires_auth() {
        let (server, _temp_dir) = create_test_server(43223);
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        // Try POST /rpc without token - should fail auth
        let mut stream = TcpStream::connect("127.0.0.1:43223").unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();

        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let request = format!(
            "POST /rpc HTTP/1.1\r\n\
             Host: localhost\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );

        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        let status_line = response.lines().next().unwrap_or("");
        let status_code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        assert_eq!(status_code, 401);
    }

    #[test]
    fn test_rpc_endpoint_resources_list() {
        let (server, _temp_dir) = create_test_server(43224);
        let token = server.token().to_string();
        let _handle = server.start().unwrap();

        thread::sleep(std::time::Duration::from_millis(100));

        let request = r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#;
        let (status, body) = http_post_json(43224, "/rpc", &token, request);

        assert_eq!(status, 200);

        let response: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 2);
        assert!(response["result"]["resources"].is_array());
    }
}

```

### src/mcp_kit_tools.rs

```rs
//! MCP Kit Namespace Tools
//!
//! Implements the kit/* namespace MCP tools for Script Kit:
//! - kit/show: Show the Script Kit window
//! - kit/hide: Hide the Script Kit window
//! - kit/state: Get current app state

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Kit tool definitions for MCP tools/list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Result of a kit tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content item in tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// App state returned by kit/state tool
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    pub visible: bool,
    pub focused: bool,
    #[serde(rename = "activePrompt")]
    pub active_prompt: Option<String>,
}

/// Returns the tool definitions for kit/* namespace tools
pub fn get_kit_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "kit/show".to_string(),
            description: "Show the Script Kit window".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "kit/hide".to_string(),
            description: "Hide the Script Kit window".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDefinition {
            name: "kit/state".to_string(),
            description: "Get current Script Kit app state".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

/// Check if a tool name is in the kit/* namespace
pub fn is_kit_tool(name: &str) -> bool {
    name.starts_with("kit/")
}

/// Handle a kit/* namespace tool call
///
/// Note: This returns a result that the caller should use to actually perform
/// the window operations. The actual show/hide operations require GPUI context
/// which is not available in this module.
pub fn handle_kit_tool_call(name: &str, _arguments: &Value) -> ToolResult {
    match name {
        "kit/show" => ToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: "Window show requested".to_string(),
            }],
            is_error: None,
        },
        "kit/hide" => ToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: "Window hide requested".to_string(),
            }],
            is_error: None,
        },
        "kit/state" => {
            // Return default state - actual state will be injected by caller
            let state = AppState::default();
            ToolResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string(&state).unwrap_or_default(),
                }],
                is_error: None,
            }
        }
        _ => ToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!("Unknown kit tool: {}", name),
            }],
            is_error: Some(true),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================
    // TDD Tests - Written FIRST per spec requirements
    // =======================================================

    #[test]
    fn test_kit_show_tool_definition() {
        let tools = get_kit_tool_definitions();
        let show_tool = tools.iter().find(|t| t.name == "kit/show");

        assert!(show_tool.is_some(), "kit/show tool should be defined");
        let tool = show_tool.unwrap();
        assert_eq!(tool.name, "kit/show");
        assert_eq!(tool.description, "Show the Script Kit window");
        assert!(tool.input_schema.get("type").is_some());
        assert_eq!(tool.input_schema.get("type").unwrap(), "object");
    }

    #[test]
    fn test_kit_hide_tool_definition() {
        let tools = get_kit_tool_definitions();
        let hide_tool = tools.iter().find(|t| t.name == "kit/hide");

        assert!(hide_tool.is_some(), "kit/hide tool should be defined");
        let tool = hide_tool.unwrap();
        assert_eq!(tool.name, "kit/hide");
        assert_eq!(tool.description, "Hide the Script Kit window");
        assert!(tool.input_schema.get("type").is_some());
        assert_eq!(tool.input_schema.get("type").unwrap(), "object");
    }

    #[test]
    fn test_kit_state_tool_definition() {
        let tools = get_kit_tool_definitions();
        let state_tool = tools.iter().find(|t| t.name == "kit/state");

        assert!(state_tool.is_some(), "kit/state tool should be defined");
        let tool = state_tool.unwrap();
        assert_eq!(tool.name, "kit/state");
        assert_eq!(tool.description, "Get current Script Kit app state");
        assert!(tool.input_schema.get("type").is_some());
        assert_eq!(tool.input_schema.get("type").unwrap(), "object");
    }

    #[test]
    fn test_tools_list_includes_kit_tools() {
        let tools = get_kit_tool_definitions();

        assert_eq!(tools.len(), 3, "Should have exactly 3 kit tools");

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"kit/show"));
        assert!(tool_names.contains(&"kit/hide"));
        assert!(tool_names.contains(&"kit/state"));
    }

    #[test]
    fn test_kit_show_call_succeeds() {
        let result = handle_kit_tool_call("kit/show", &serde_json::json!({}));

        assert!(result.is_error.is_none() || result.is_error == Some(false));
        assert!(!result.content.is_empty());
        assert_eq!(result.content[0].content_type, "text");
        assert!(result.content[0].text.contains("show"));
    }

    #[test]
    fn test_kit_hide_call_succeeds() {
        let result = handle_kit_tool_call("kit/hide", &serde_json::json!({}));

        assert!(result.is_error.is_none() || result.is_error == Some(false));
        assert!(!result.content.is_empty());
        assert_eq!(result.content[0].content_type, "text");
        assert!(result.content[0].text.contains("hide"));
    }

    #[test]
    fn test_kit_state_returns_json() {
        let result = handle_kit_tool_call("kit/state", &serde_json::json!({}));

        assert!(result.is_error.is_none() || result.is_error == Some(false));
        assert!(!result.content.is_empty());
        assert_eq!(result.content[0].content_type, "text");

        // Verify the result is valid JSON with expected fields
        let state: Result<AppState, _> = serde_json::from_str(&result.content[0].text);
        assert!(state.is_ok(), "kit/state should return valid JSON");

        let state = state.unwrap();
        // Default state should have visible=false, focused=false
        assert!(!state.visible);
        assert!(!state.focused);
    }

    #[test]
    fn test_is_kit_tool() {
        assert!(is_kit_tool("kit/show"));
        assert!(is_kit_tool("kit/hide"));
        assert!(is_kit_tool("kit/state"));
        assert!(is_kit_tool("kit/custom"));

        assert!(!is_kit_tool("scripts/run"));
        assert!(!is_kit_tool("resources/list"));
        assert!(!is_kit_tool("kitshow")); // No slash
    }

    #[test]
    fn test_unknown_kit_tool_returns_error() {
        let result = handle_kit_tool_call("kit/unknown", &serde_json::json!({}));

        assert_eq!(result.is_error, Some(true));
        assert!(!result.content.is_empty());
        assert!(result.content[0].text.contains("Unknown kit tool"));
    }

    #[test]
    fn test_tool_definition_serialization() {
        let tools = get_kit_tool_definitions();
        let json = serde_json::to_value(&tools);

        assert!(json.is_ok(), "Tool definitions should serialize to JSON");

        let json = json.unwrap();
        assert!(json.is_array());

        // Check the first tool has expected structure
        let first_tool = &json[0];
        assert!(first_tool.get("name").is_some());
        assert!(first_tool.get("description").is_some());
        assert!(first_tool.get("inputSchema").is_some());
    }

    #[test]
    fn test_app_state_serialization() {
        let state = AppState {
            visible: true,
            focused: true,
            active_prompt: Some("arg".to_string()),
        };

        let json = serde_json::to_value(&state);
        assert!(json.is_ok());

        let json = json.unwrap();
        assert_eq!(json.get("visible").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(json.get("focused").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            json.get("activePrompt").and_then(|v| v.as_str()),
            Some("arg")
        );
    }

    #[test]
    fn test_tool_result_serialization() {
        let result = ToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: "test message".to_string(),
            }],
            is_error: None,
        };

        let json = serde_json::to_string(&result);
        assert!(json.is_ok());

        let json = json.unwrap();
        // is_error should be omitted when None
        assert!(!json.contains("isError"));
        assert!(json.contains("content"));
        assert!(json.contains("text"));
    }
}

```

### src/mcp_protocol.rs

```rs
//! MCP JSON-RPC 2.0 Protocol Handler
//!
//! Implements the JSON-RPC 2.0 protocol for MCP (Model Context Protocol).
//! Handles request parsing, method routing, and response generation.
//!
//! JSON-RPC 2.0 format:
//! - Request: {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
//! - Success: {"jsonrpc":"2.0","id":1,"result":{"tools":[]}}
//! - Error: {"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}

// Allow from_str name - we're not implementing FromStr trait as this returns Option, not Result
#![allow(clippy::should_implement_trait)]
// Allow large error variant - JsonRpcResponse needs to carry full error info for JSON-RPC spec
#![allow(clippy::result_large_err)]
// Allow dead code - this module provides complete MCP API surface; some methods for future use
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp_kit_tools;
use crate::mcp_resources;
use crate::mcp_script_tools;
use crate::scripts::Script;
use crate::scripts::Scriptlet;

/// JSON-RPC 2.0 version string
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC 2.0 standard error codes
pub mod error_codes {
    /// Invalid JSON was received
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;
    /// The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Request identifier (can be string, number, or null)
    pub id: Value,
    /// Method name to invoke
    pub method: String,
    /// Optional parameters
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    /// Must be "2.0"
    pub jsonrpc: String,
    /// Request identifier (matches request)
    pub id: Value,
    /// Result on success (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Optional additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP methods supported by this server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpMethod {
    /// Initialize the MCP session
    Initialize,
    /// List available tools
    ToolsList,
    /// Call a specific tool
    ToolsCall,
    /// List available resources
    ResourcesList,
    /// Read a specific resource
    ResourcesRead,
}

impl McpMethod {
    /// Parse method string to enum variant
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "initialize" => Some(Self::Initialize),
            "tools/list" => Some(Self::ToolsList),
            "tools/call" => Some(Self::ToolsCall),
            "resources/list" => Some(Self::ResourcesList),
            "resources/read" => Some(Self::ResourcesRead),
            _ => None,
        }
    }

    /// Get the method string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::ToolsList => "tools/list",
            Self::ToolsCall => "tools/call",
            Self::ResourcesList => "resources/list",
            Self::ResourcesRead => "resources/read",
        }
    }
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Create an error response with additional data
    pub fn error_with_data(id: Value, code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }
}

/// MCP server capabilities returned by initialize
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCapabilities {
    /// Server information
    pub server_info: ServerInfo,
    /// Supported capabilities
    pub capabilities: CapabilitySet,
}

/// Server identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Set of capabilities the server supports
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilitySet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
}

/// Tools capability settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resources capability settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourcesCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    #[serde(rename = "listChanged", skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Parse a JSON string into a JsonRpcRequest
pub fn parse_request(json: &str) -> Result<JsonRpcRequest, JsonRpcResponse> {
    // Try to parse the JSON
    let value: Value = serde_json::from_str(json).map_err(|e| {
        JsonRpcResponse::error(
            Value::Null,
            error_codes::PARSE_ERROR,
            format!("Parse error: {}", e),
        )
    })?;

    // Validate jsonrpc version
    let jsonrpc = value
        .get("jsonrpc")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            JsonRpcResponse::error(
                value.get("id").cloned().unwrap_or(Value::Null),
                error_codes::INVALID_REQUEST,
                "Missing or invalid 'jsonrpc' field",
            )
        })?;

    if jsonrpc != JSONRPC_VERSION {
        return Err(JsonRpcResponse::error(
            value.get("id").cloned().unwrap_or(Value::Null),
            error_codes::INVALID_REQUEST,
            format!(
                "Invalid jsonrpc version: expected '{}', got '{}'",
                JSONRPC_VERSION, jsonrpc
            ),
        ));
    }

    // Validate required fields
    let id = value.get("id").cloned().unwrap_or(Value::Null);

    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            JsonRpcResponse::error(
                id.clone(),
                error_codes::INVALID_REQUEST,
                "Missing 'method' field",
            )
        })?;

    let params = value
        .get("params")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));

    Ok(JsonRpcRequest {
        jsonrpc: JSONRPC_VERSION.to_string(),
        id,
        method: method.to_string(),
        params,
    })
}

/// Handle an MCP JSON-RPC request and return a response
pub fn handle_request(request: JsonRpcRequest) -> JsonRpcResponse {
    // Use empty scripts list for stateless handler
    handle_request_with_scripts(request, &[])
}

/// Handle an MCP JSON-RPC request with script context
/// This allows script tools to be dynamically included based on loaded scripts
pub fn handle_request_with_scripts(request: JsonRpcRequest, scripts: &[Script]) -> JsonRpcResponse {
    // Use empty scriptlets list for backwards compatibility
    handle_request_with_context(request, scripts, &[], None)
}

/// Handle an MCP JSON-RPC request with full context
/// This allows script tools and resources to be dynamically included
pub fn handle_request_with_context(
    request: JsonRpcRequest,
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    app_state: Option<&mcp_resources::AppStateResource>,
) -> JsonRpcResponse {
    // Check for valid jsonrpc version
    if request.jsonrpc != JSONRPC_VERSION {
        return JsonRpcResponse::error(
            request.id,
            error_codes::INVALID_REQUEST,
            format!("Invalid jsonrpc version: {}", request.jsonrpc),
        );
    }

    // Route to appropriate handler based on method
    match McpMethod::from_str(&request.method) {
        Some(McpMethod::Initialize) => handle_initialize(request),
        Some(McpMethod::ToolsList) => handle_tools_list_with_scripts(request, scripts),
        Some(McpMethod::ToolsCall) => handle_tools_call_with_scripts(request, scripts),
        Some(McpMethod::ResourcesList) => handle_resources_list(request),
        Some(McpMethod::ResourcesRead) => {
            handle_resources_read_with_context(request, scripts, scriptlets, app_state)
        }
        None => JsonRpcResponse::error(
            request.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", request.method),
        ),
    }
}

/// Handle initialize request
fn handle_initialize(request: JsonRpcRequest) -> JsonRpcResponse {
    let capabilities = McpCapabilities {
        server_info: ServerInfo {
            name: "script-kit".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        capabilities: CapabilitySet {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            resources: Some(ResourcesCapability {
                subscribe: Some(false),
                list_changed: Some(true),
            }),
        },
    };

    JsonRpcResponse::success(
        request.id,
        serde_json::to_value(capabilities).unwrap_or(Value::Null),
    )
}

/// Handle tools/list request (no script context)
#[allow(dead_code)]
fn handle_tools_list(request: JsonRpcRequest) -> JsonRpcResponse {
    // Use empty scripts list for stateless handler
    handle_tools_list_with_scripts(request, &[])
}

/// Handle tools/list request with script context
/// This allows including dynamically loaded script tools
pub fn handle_tools_list_with_scripts(
    request: JsonRpcRequest,
    scripts: &[Script],
) -> JsonRpcResponse {
    // Get kit/* namespace tools
    let mut all_tools = mcp_kit_tools::get_kit_tool_definitions();

    // Get scripts/* namespace tools (only scripts with schema.input)
    let script_tools = mcp_script_tools::get_script_tool_definitions(scripts);
    all_tools.extend(script_tools);

    // Convert to JSON value
    let tools_json = serde_json::to_value(&all_tools).unwrap_or(serde_json::json!([]));

    JsonRpcResponse::success(
        request.id,
        serde_json::json!({
            "tools": tools_json
        }),
    )
}

/// Handle tools/call request (no script context)
#[allow(dead_code)]
fn handle_tools_call(request: JsonRpcRequest) -> JsonRpcResponse {
    // Use empty scripts list for stateless handler
    handle_tools_call_with_scripts(request, &[])
}

/// Handle tools/call request with script context
/// This allows handling scripts/* namespace tool calls
pub fn handle_tools_call_with_scripts(
    request: JsonRpcRequest,
    scripts: &[Script],
) -> JsonRpcResponse {
    // Validate params
    let params = request.params.as_object();
    if params.is_none() {
        return JsonRpcResponse::error(
            request.id,
            error_codes::INVALID_PARAMS,
            "Invalid params: expected object",
        );
    }

    let params = params.unwrap();
    let tool_name = params.get("name").and_then(|v| v.as_str());

    if tool_name.is_none() {
        return JsonRpcResponse::error(
            request.id,
            error_codes::INVALID_PARAMS,
            "Missing required parameter: name",
        );
    }

    let tool_name = tool_name.unwrap();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Route kit/* namespace tools
    if mcp_kit_tools::is_kit_tool(tool_name) {
        let result = mcp_kit_tools::handle_kit_tool_call(tool_name, &arguments);
        return JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(serde_json::json!({})),
        );
    }

    // Route scripts/* namespace tools
    if mcp_script_tools::is_script_tool(tool_name) {
        let result = mcp_script_tools::handle_script_tool_call(scripts, tool_name, &arguments);
        return JsonRpcResponse::success(
            request.id,
            serde_json::to_value(result).unwrap_or(serde_json::json!({})),
        );
    }

    // Tool not found in any namespace
    JsonRpcResponse::error(
        request.id,
        error_codes::METHOD_NOT_FOUND,
        format!("Tool not found: {}", tool_name),
    )
}

/// Handle resources/list request
fn handle_resources_list(request: JsonRpcRequest) -> JsonRpcResponse {
    let resources = mcp_resources::get_resource_definitions();
    JsonRpcResponse::success(
        request.id,
        mcp_resources::resource_list_to_value(&resources),
    )
}

/// Handle resources/read request (stateless - for backwards compatibility)
#[allow(dead_code)]
fn handle_resources_read(request: JsonRpcRequest) -> JsonRpcResponse {
    handle_resources_read_with_context(request, &[], &[], None)
}

/// Handle resources/read request with full context
fn handle_resources_read_with_context(
    request: JsonRpcRequest,
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    app_state: Option<&mcp_resources::AppStateResource>,
) -> JsonRpcResponse {
    // Validate params
    let params = request.params.as_object();
    if params.is_none() {
        return JsonRpcResponse::error(
            request.id,
            error_codes::INVALID_PARAMS,
            "Invalid params: expected object",
        );
    }

    let params = params.unwrap();
    let uri = params.get("uri").and_then(|v| v.as_str());

    if uri.is_none() {
        return JsonRpcResponse::error(
            request.id,
            error_codes::INVALID_PARAMS,
            "Missing required parameter: uri",
        );
    }

    let uri = uri.unwrap();

    // Read the resource
    match mcp_resources::read_resource(uri, scripts, scriptlets, app_state) {
        Ok(content) => JsonRpcResponse::success(
            request.id,
            mcp_resources::resource_content_to_value(content),
        ),
        Err(err) => JsonRpcResponse::error(request.id, error_codes::METHOD_NOT_FOUND, err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================
    // TDD Tests - Written FIRST per spec requirements
    // =======================================================

    #[test]
    fn test_parse_valid_jsonrpc_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let result = parse_request(json);

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, serde_json::json!(1));
        assert_eq!(request.method, "tools/list");
        assert_eq!(request.params, serde_json::json!({}));
    }

    #[test]
    fn test_parse_invalid_jsonrpc_returns_error() {
        // Test 1: Invalid JSON
        let json = r#"{"jsonrpc":"2.0", invalid}"#;
        let result = parse_request(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error.as_ref().unwrap().code, error_codes::PARSE_ERROR);

        // Test 2: Missing jsonrpc field
        let json = r#"{"id":1,"method":"test"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.error.as_ref().unwrap().code,
            error_codes::INVALID_REQUEST
        );

        // Test 3: Wrong jsonrpc version
        let json = r#"{"jsonrpc":"1.0","id":1,"method":"test"}"#;
        let result = parse_request(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.error.as_ref().unwrap().code,
            error_codes::INVALID_REQUEST
        );

        // Test 4: Missing method field
        let json = r#"{"jsonrpc":"2.0","id":1}"#;
        let result = parse_request(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.error.as_ref().unwrap().code,
            error_codes::INVALID_REQUEST
        );
    }

    #[test]
    fn test_method_not_found_error() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "unknown/method".to_string(),
            params: serde_json::json!({}),
        };

        let response = handle_request(request);

        assert!(response.error.is_some());
        assert!(response.result.is_none());
        let err = response.error.unwrap();
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
        assert!(err.message.contains("Method not found"));
        assert!(err.message.contains("unknown/method"));
    }

    #[test]
    fn test_initialize_returns_capabilities() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "initialize".to_string(),
            params: serde_json::json!({}),
        };

        let response = handle_request(request);

        assert!(response.result.is_some());
        assert!(response.error.is_none());

        let result = response.result.unwrap();

        // Check serverInfo
        assert!(result.get("serverInfo").is_some());
        let server_info = result.get("serverInfo").unwrap();
        assert_eq!(
            server_info.get("name").and_then(|v| v.as_str()),
            Some("script-kit")
        );
        assert!(server_info.get("version").is_some());

        // Check capabilities
        assert!(result.get("capabilities").is_some());
        let caps = result.get("capabilities").unwrap();
        assert!(caps.get("tools").is_some());
        assert!(caps.get("resources").is_some());
    }

    #[test]
    fn test_tools_list_returns_kit_tools() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(42),
            method: "tools/list".to_string(),
            params: serde_json::json!({}),
        };

        let response = handle_request(request);

        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert_eq!(response.id, serde_json::json!(42));

        let result = response.result.unwrap();
        let tools = result.get("tools").and_then(|v| v.as_array());
        assert!(tools.is_some());

        let tools = tools.unwrap();
        // Should have at least the kit/* tools
        assert!(!tools.is_empty(), "tools/list should return kit tools");

        // Verify kit tools are present
        let tool_names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(tool_names.contains(&"kit/show"), "Should include kit/show");
        assert!(tool_names.contains(&"kit/hide"), "Should include kit/hide");
        assert!(
            tool_names.contains(&"kit/state"),
            "Should include kit/state"
        );
    }

    #[test]
    fn test_resources_list_returns_all_resources() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!("req-123"),
            method: "resources/list".to_string(),
            params: serde_json::json!({}),
        };

        let response = handle_request(request);

        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert_eq!(response.id, serde_json::json!("req-123"));

        let result = response.result.unwrap();
        let resources = result.get("resources").and_then(|v| v.as_array());
        assert!(resources.is_some());

        let resources = resources.unwrap();
        assert_eq!(resources.len(), 3, "Should have 3 resources");

        // Verify expected resources are present
        let uris: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.get("uri").and_then(|u| u.as_str()))
            .collect();

        assert!(uris.contains(&"kit://state"), "Should include kit://state");
        assert!(uris.contains(&"scripts://"), "Should include scripts://");
        assert!(
            uris.contains(&"scriptlets://"),
            "Should include scriptlets://"
        );
    }

    // =======================================================
    // Additional tests for completeness
    // =======================================================

    #[test]
    fn test_mcp_method_from_str() {
        assert_eq!(
            McpMethod::from_str("initialize"),
            Some(McpMethod::Initialize)
        );
        assert_eq!(
            McpMethod::from_str("tools/list"),
            Some(McpMethod::ToolsList)
        );
        assert_eq!(
            McpMethod::from_str("tools/call"),
            Some(McpMethod::ToolsCall)
        );
        assert_eq!(
            McpMethod::from_str("resources/list"),
            Some(McpMethod::ResourcesList)
        );
        assert_eq!(
            McpMethod::from_str("resources/read"),
            Some(McpMethod::ResourcesRead)
        );
        assert_eq!(McpMethod::from_str("unknown"), None);
    }

    #[test]
    fn test_mcp_method_as_str() {
        assert_eq!(McpMethod::Initialize.as_str(), "initialize");
        assert_eq!(McpMethod::ToolsList.as_str(), "tools/list");
        assert_eq!(McpMethod::ToolsCall.as_str(), "tools/call");
        assert_eq!(McpMethod::ResourcesList.as_str(), "resources/list");
        assert_eq!(McpMethod::ResourcesRead.as_str(), "resources/read");
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let response =
            JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"key": "value"}));

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, serde_json::json!(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let response = JsonRpcResponse::error(
            serde_json::json!(1),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, serde_json::json!(1));
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let err = response.error.unwrap();
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_tools_call_requires_name_param() {
        // Missing name param
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "tools/call".to_string(),
            params: serde_json::json!({}),
        };

        let response = handle_request(request);
        assert!(response.error.is_some());
        assert_eq!(
            response.error.as_ref().unwrap().code,
            error_codes::INVALID_PARAMS
        );
    }

    #[test]
    fn test_resources_read_requires_uri_param() {
        // Missing uri param
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "resources/read".to_string(),
            params: serde_json::json!({}),
        };

        let response = handle_request(request);
        assert!(response.error.is_some());
        assert_eq!(
            response.error.as_ref().unwrap().code,
            error_codes::INVALID_PARAMS
        );
    }

    #[test]
    fn test_parse_request_with_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"request-123","method":"initialize","params":{}}"#;
        let result = parse_request(json);

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.id, serde_json::json!("request-123"));
    }

    #[test]
    fn test_parse_request_with_null_id() {
        // Notifications have null id (or id is omitted)
        let json = r#"{"jsonrpc":"2.0","id":null,"method":"initialize","params":{}}"#;
        let result = parse_request(json);

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.id, Value::Null);
    }

    #[test]
    fn test_parse_request_without_params() {
        // params is optional
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let result = parse_request(json);

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.params, serde_json::json!({}));
    }

    #[test]
    fn test_response_serialization() {
        let response =
            JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"tools": []}));

        let json = serde_json::to_string(&response).unwrap();

        // Should not contain "error" field when it's None
        assert!(!json.contains("error"));
        assert!(json.contains("result"));
        assert!(json.contains("jsonrpc"));
        assert!(json.contains("2.0"));
    }

    #[test]
    fn test_error_response_serialization() {
        let response = JsonRpcResponse::error(
            serde_json::json!(1),
            error_codes::METHOD_NOT_FOUND,
            "Not found",
        );

        let json = serde_json::to_string(&response).unwrap();

        // Should not contain "result" field when it's None
        assert!(!json.contains("result"));
        assert!(json.contains("error"));
        assert!(json.contains("-32601"));
    }

    // =======================================================
    // Kit Tools Integration Tests
    // =======================================================

    #[test]
    fn test_tools_call_kit_show() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "kit/show",
                "arguments": {}
            }),
        };

        let response = handle_request(request);

        // Should succeed (not return an error)
        assert!(response.error.is_none(), "kit/show call should succeed");
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        // Should have content array
        assert!(result.get("content").is_some());
    }

    #[test]
    fn test_tools_call_kit_hide() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(2),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "kit/hide",
                "arguments": {}
            }),
        };

        let response = handle_request(request);

        assert!(response.error.is_none(), "kit/hide call should succeed");
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert!(result.get("content").is_some());
    }

    #[test]
    fn test_tools_call_kit_state() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(3),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "kit/state",
                "arguments": {}
            }),
        };

        let response = handle_request(request);

        assert!(response.error.is_none(), "kit/state call should succeed");
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert!(result.get("content").is_some());

        // Verify the content is valid JSON with state fields
        let content = result.get("content").and_then(|c| c.as_array());
        assert!(content.is_some());

        let content = content.unwrap();
        assert!(!content.is_empty());

        let text = content[0].get("text").and_then(|t| t.as_str());
        assert!(text.is_some());

        // Should be parseable as AppState JSON
        let state: Result<serde_json::Value, _> = serde_json::from_str(text.unwrap());
        assert!(state.is_ok(), "kit/state should return valid JSON");

        let state = state.unwrap();
        assert!(state.get("visible").is_some());
        assert!(state.get("focused").is_some());
    }

    #[test]
    fn test_tools_call_unknown_kit_tool() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(4),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "kit/unknown",
                "arguments": {}
            }),
        };

        let response = handle_request(request);

        // Should succeed but with isError flag in result
        assert!(
            response.error.is_none(),
            "Should return result, not protocol error"
        );
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        // isError should be true for unknown kit tools
        assert_eq!(result.get("isError").and_then(|e| e.as_bool()), Some(true));
    }

    #[test]
    fn test_tools_call_non_kit_tool_not_found() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(5),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "scripts/run",
                "arguments": {}
            }),
        };

        let response = handle_request(request);

        // scripts/* tools now go through script handler which returns isError: true
        // instead of a protocol error, because it's a valid namespace
        assert!(
            response.error.is_none(),
            "Should return result, not protocol error"
        );
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert_eq!(result.get("isError").and_then(|e| e.as_bool()), Some(true));
    }

    #[test]
    fn test_tools_call_unknown_namespace_returns_error() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(5),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "unknown/tool",
                "arguments": {}
            }),
        };

        let response = handle_request(request);

        // Unknown namespace should return method not found error
        assert!(response.error.is_some());
        assert_eq!(
            response.error.as_ref().unwrap().code,
            error_codes::METHOD_NOT_FOUND
        );
    }

    // =======================================================
    // Script Tools Integration Tests
    // =======================================================

    mod script_tools_tests {
        use super::*;
        use crate::schema_parser::{FieldDef, FieldType, Schema};
        use std::collections::HashMap;
        use std::path::PathBuf;

        /// Helper to create a test script with schema
        fn test_script_with_schema(name: &str, description: Option<&str>) -> Script {
            let mut input = HashMap::new();
            input.insert(
                "title".to_string(),
                FieldDef {
                    field_type: FieldType::String,
                    required: true,
                    description: Some("The title".to_string()),
                    ..Default::default()
                },
            );
            let schema = Schema {
                input,
                output: HashMap::new(),
            };

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
                schema: Some(schema),
            }
        }

        #[test]
        fn test_tools_list_includes_script_tools() {
            let scripts = vec![
                test_script_with_schema("Create Note", Some("Creates a new note")),
                test_script_with_schema("Git Commit", Some("Commits changes")),
            ];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "tools/list".to_string(),
                params: serde_json::json!({}),
            };

            let response = handle_request_with_scripts(request, &scripts);

            assert!(response.result.is_some());
            let result = response.result.unwrap();
            let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();

            // Collect tool names
            let tool_names: Vec<&str> = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .collect();

            // Should include kit/* tools
            assert!(tool_names.contains(&"kit/show"));
            assert!(tool_names.contains(&"kit/hide"));

            // Should include scripts/* tools
            assert!(
                tool_names.contains(&"scripts/create-note"),
                "Should include scripts/create-note"
            );
            assert!(
                tool_names.contains(&"scripts/git-commit"),
                "Should include scripts/git-commit"
            );
        }

        #[test]
        fn test_tools_list_script_tool_has_correct_schema() {
            let scripts = vec![test_script_with_schema(
                "Test Script",
                Some("Test description"),
            )];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "tools/list".to_string(),
                params: serde_json::json!({}),
            };

            let response = handle_request_with_scripts(request, &scripts);
            let result = response.result.unwrap();
            let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();

            // Find the script tool
            let script_tool = tools
                .iter()
                .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("scripts/test-script"));

            assert!(script_tool.is_some(), "Script tool should be in list");
            let tool = script_tool.unwrap();

            // Check description
            assert_eq!(
                tool.get("description").and_then(|d| d.as_str()),
                Some("Test description")
            );

            // Check inputSchema
            let input_schema = tool.get("inputSchema");
            assert!(input_schema.is_some());
            assert_eq!(input_schema.unwrap()["type"], "object");
            assert!(input_schema.unwrap()["properties"]["title"].is_object());
        }

        #[test]
        fn test_tools_call_script_tool() {
            let scripts = vec![test_script_with_schema(
                "Create Note",
                Some("Creates notes"),
            )];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "tools/call".to_string(),
                params: serde_json::json!({
                    "name": "scripts/create-note",
                    "arguments": {"title": "My Note"}
                }),
            };

            let response = handle_request_with_scripts(request, &scripts);

            // Should succeed (return result, not error)
            assert!(response.error.is_none(), "Script tool call should succeed");
            assert!(response.result.is_some());

            let result = response.result.unwrap();
            // Should have content
            assert!(result.get("content").is_some());
        }

        #[test]
        fn test_tools_call_unknown_script_tool() {
            let scripts = vec![test_script_with_schema("Create Note", None)];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "tools/call".to_string(),
                params: serde_json::json!({
                    "name": "scripts/unknown-script",
                    "arguments": {}
                }),
            };

            let response = handle_request_with_scripts(request, &scripts);

            // Should succeed but with isError flag
            assert!(response.error.is_none());
            let result = response.result.unwrap();
            assert_eq!(result.get("isError").and_then(|e| e.as_bool()), Some(true));
        }

        #[test]
        fn test_tools_list_empty_scripts() {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "tools/list".to_string(),
                params: serde_json::json!({}),
            };

            let response = handle_request_with_scripts(request, &[]);

            assert!(response.result.is_some());
            let result = response.result.unwrap();
            let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();

            // Should still have kit/* tools
            let tool_names: Vec<&str> = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .collect();

            assert!(tool_names.contains(&"kit/show"));
            assert!(tool_names.contains(&"kit/hide"));
            assert!(tool_names.contains(&"kit/state"));

            // Should NOT have any scripts/* tools
            let script_tools: Vec<&&str> = tool_names
                .iter()
                .filter(|n| n.starts_with("scripts/"))
                .collect();
            assert!(
                script_tools.is_empty(),
                "No script tools when scripts list is empty"
            );
        }

        #[test]
        fn test_scripts_without_schema_not_in_tools_list() {
            // Script without schema
            let script_no_schema = Script {
                name: "Simple Script".to_string(),
                path: PathBuf::from("/test/simple-script.ts"),
                extension: "ts".to_string(),
                description: Some("No schema".to_string()),
                icon: None,
                alias: None,
                shortcut: None,
                typed_metadata: None,
                schema: None, // No schema!
            };

            let scripts = vec![
                script_no_schema,
                test_script_with_schema("With Schema", Some("Has schema")),
            ];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "tools/list".to_string(),
                params: serde_json::json!({}),
            };

            let response = handle_request_with_scripts(request, &scripts);
            let result = response.result.unwrap();
            let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();

            let tool_names: Vec<&str> = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .collect();

            // Should have the script with schema
            assert!(tool_names.contains(&"scripts/with-schema"));

            // Should NOT have the script without schema
            assert!(
                !tool_names.contains(&"scripts/simple-script"),
                "Script without schema should not be in tools list"
            );
        }
    }

    // =======================================================
    // MCP Resources Integration Tests
    // =======================================================

    mod resources_integration_tests {
        use super::*;
        use crate::scripts::Scriptlet;
        use std::path::PathBuf;

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
        fn test_scriptlet(name: &str, tool: &str) -> Scriptlet {
            Scriptlet {
                name: name.to_string(),
                description: None,
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
        fn test_resources_read_scripts() {
            let scripts = vec![
                test_script("Script One", Some("First script")),
                test_script("Script Two", None),
            ];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(1),
                method: "resources/read".to_string(),
                params: serde_json::json!({"uri": "scripts://"}),
            };

            let response = handle_request_with_context(request, &scripts, &[], None);

            assert!(response.error.is_none(), "Should succeed");
            assert!(response.result.is_some());

            let result = response.result.unwrap();
            let contents = result.get("contents").and_then(|c| c.as_array());
            assert!(contents.is_some());

            let contents = contents.unwrap();
            assert_eq!(contents.len(), 1);

            let content = &contents[0];
            assert_eq!(
                content.get("uri").and_then(|u| u.as_str()),
                Some("scripts://")
            );

            // Parse the text as JSON
            let text = content.get("text").and_then(|t| t.as_str()).unwrap();
            let parsed: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
            assert_eq!(parsed.len(), 2);
        }

        #[test]
        fn test_resources_read_scriptlets() {
            let scriptlets = vec![
                test_scriptlet("Open URL", "open"),
                test_scriptlet("Paste", "paste"),
            ];

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(2),
                method: "resources/read".to_string(),
                params: serde_json::json!({"uri": "scriptlets://"}),
            };

            let response = handle_request_with_context(request, &[], &scriptlets, None);

            assert!(response.error.is_none(), "Should succeed");

            let result = response.result.unwrap();
            let contents = result.get("contents").and_then(|c| c.as_array()).unwrap();
            let text = contents[0].get("text").and_then(|t| t.as_str()).unwrap();
            let parsed: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
            assert_eq!(parsed.len(), 2);
        }

        #[test]
        fn test_resources_read_app_state() {
            let app_state = mcp_resources::AppStateResource {
                visible: true,
                focused: true,
                script_count: 5,
                scriptlet_count: 3,
                filter_text: Some("test".to_string()),
                selected_index: Some(2),
            };

            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(3),
                method: "resources/read".to_string(),
                params: serde_json::json!({"uri": "kit://state"}),
            };

            let response = handle_request_with_context(request, &[], &[], Some(&app_state));

            assert!(response.error.is_none(), "Should succeed");

            let result = response.result.unwrap();
            let contents = result.get("contents").and_then(|c| c.as_array()).unwrap();
            let text = contents[0].get("text").and_then(|t| t.as_str()).unwrap();
            let parsed: mcp_resources::AppStateResource = serde_json::from_str(text).unwrap();

            assert!(parsed.visible);
            assert!(parsed.focused);
            assert_eq!(parsed.script_count, 5);
            assert_eq!(parsed.scriptlet_count, 3);
            assert_eq!(parsed.filter_text, Some("test".to_string()));
        }

        #[test]
        fn test_resources_read_unknown_uri() {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: serde_json::json!(4),
                method: "resources/read".to_string(),
                params: serde_json::json!({"uri": "unknown://resource"}),
            };

            let response = handle_request_with_context(request, &[], &[], None);

            assert!(
                response.error.is_some(),
                "Unknown resource should return error"
            );
            assert_eq!(
                response.error.as_ref().unwrap().code,
                error_codes::METHOD_NOT_FOUND
            );
            assert!(response
                .error
                .as_ref()
                .unwrap()
                .message
                .contains("Resource not found"));
        }

        #[test]
        fn test_resources_read_with_full_context() {
            let scripts = vec![test_script("Test Script", None)];
            let scriptlets = vec![test_scriptlet("Test Snippet", "bash")];
            let app_state = mcp_resources::AppStateResource {
                visible: true,
                focused: false,
                script_count: 1,
                scriptlet_count: 1,
                filter_text: None,
                selected_index: None,
            };

            // Test all three resources work with full context
            for uri in &["kit://state", "scripts://", "scriptlets://"] {
                let request = JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: serde_json::json!(uri),
                    method: "resources/read".to_string(),
                    params: serde_json::json!({"uri": uri}),
                };

                let response =
                    handle_request_with_context(request, &scripts, &scriptlets, Some(&app_state));

                assert!(response.error.is_none(), "Should succeed for {}", uri);
                assert!(response.result.is_some());
            }
        }
    }
}

```

### src/mcp_resources.rs

```rs
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

```

### src/mcp_streaming.rs

```rs
//! MCP Server-Sent Events (SSE) Streaming and Audit Logging
//!
//! Provides:
//! - SSE streaming for real-time event delivery to clients
//! - Audit logging for tool calls to ~/.sk/kit/logs/mcp-audit.jsonl
//!
//! Event format: `event: {type}\ndata: {json}\n\n`

// Allow dead code - SSE streaming and audit logging infrastructure for future features
#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// SSE event types supported by the MCP server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SseEventType {
    Progress,
    Output,
    Error,
    Complete,
}

impl SseEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SseEventType::Progress => "progress",
            SseEventType::Output => "output",
            SseEventType::Error => "error",
            SseEventType::Complete => "complete",
        }
    }
}

impl std::fmt::Display for SseEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// SSE Stream manager for broadcasting events to connected clients
#[derive(Debug)]
pub struct SseStream {
    /// Buffer of formatted SSE messages ready to send
    buffer: Vec<String>,
}

impl SseStream {
    /// Create a new SSE stream
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Format and queue an SSE event for broadcast
    ///
    /// Event format: `event: {type}\ndata: {json}\n\n`
    pub fn broadcast_event(&mut self, event_type: SseEventType, data: &serde_json::Value) {
        let formatted = format_sse_event(event_type, data);
        self.buffer.push(formatted);
    }

    /// Get all pending events and clear the buffer
    pub fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.buffer)
    }

    /// Get the number of pending events
    pub fn pending_count(&self) -> usize {
        self.buffer.len()
    }
}

impl Default for SseStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a single SSE event
///
/// Format: `event: {type}\ndata: {json}\n\n`
pub fn format_sse_event(event_type: SseEventType, data: &serde_json::Value) -> String {
    let json_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    format!("event: {}\ndata: {}\n\n", event_type.as_str(), json_str)
}

/// Format an SSE heartbeat comment
///
/// Format: `: heartbeat\n\n`
pub fn format_sse_heartbeat() -> String {
    ": heartbeat\n\n".to_string()
}

/// Audit log entry for tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Method/tool name that was called
    pub method: String,
    /// Parameters passed to the method (as JSON)
    pub params: serde_json::Value,
    /// Duration of the call in milliseconds
    pub duration_ms: u64,
    /// Whether the call succeeded
    pub success: bool,
    /// Error message if the call failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AuditLogEntry {
    /// Create a new successful audit log entry
    pub fn success(method: &str, params: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            timestamp: iso8601_now(),
            method: method.to_string(),
            params,
            duration_ms,
            success: true,
            error: None,
        }
    }

    /// Create a new failed audit log entry
    pub fn failure(method: &str, params: serde_json::Value, duration_ms: u64, error: &str) -> Self {
        Self {
            timestamp: iso8601_now(),
            method: method.to_string(),
            params,
            duration_ms,
            success: false,
            error: Some(error.to_string()),
        }
    }
}

/// Audit logger that writes to ~/.sk/kit/logs/mcp-audit.jsonl
pub struct AuditLogger {
    log_path: PathBuf,
}

impl AuditLogger {
    /// Create a new audit logger
    ///
    /// # Arguments
    /// * `kenv_path` - Path to ~/.sk/kit directory
    pub fn new(kenv_path: PathBuf) -> Self {
        let log_path = kenv_path.join("logs").join("mcp-audit.jsonl");
        Self { log_path }
    }

    /// Create audit logger with default ~/.sk/kit path
    pub fn with_defaults() -> Result<Self> {
        let kenv_path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".kenv");
        Ok(Self::new(kenv_path))
    }

    /// Get the log file path
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Write an audit log entry
    pub fn log(&self, entry: &AuditLogEntry) -> Result<()> {
        // Ensure logs directory exists
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent).context("Failed to create logs directory")?;
        }

        // Serialize entry to JSON
        let json = serde_json::to_string(entry).context("Failed to serialize audit log entry")?;

        // Append to log file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open audit log file")?;

        writeln!(file, "{}", json).context("Failed to write audit log entry")?;

        Ok(())
    }

    /// Log a successful tool call
    pub fn log_success(
        &self,
        method: &str,
        params: serde_json::Value,
        duration_ms: u64,
    ) -> Result<()> {
        let entry = AuditLogEntry::success(method, params, duration_ms);
        self.log(&entry)
    }

    /// Log a failed tool call
    pub fn log_failure(
        &self,
        method: &str,
        params: serde_json::Value,
        duration_ms: u64,
        error: &str,
    ) -> Result<()> {
        let entry = AuditLogEntry::failure(method, params, duration_ms, error);
        self.log(&entry)
    }
}

/// Get current timestamp in ISO 8601 format
fn iso8601_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();

    // Convert to datetime components (simplified - just for formatting)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    // Simplified calculation - good enough for logging purposes
    let mut year = 1970i32;
    let mut remaining_days = days_since_epoch as i32;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let (month, day) = day_of_year_to_month_day(remaining_days as u32 + 1, is_leap_year(year));

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, minutes, seconds, millis
    )
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn day_of_year_to_month_day(day_of_year: u32, leap: bool) -> (u32, u32) {
    let days_in_months: [u32; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut remaining = day_of_year;
    for (i, &days) in days_in_months.iter().enumerate() {
        if remaining <= days {
            return ((i + 1) as u32, remaining);
        }
        remaining -= days;
    }
    (12, 31) // Fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ==========================================
    // TDD TESTS - Written FIRST before implementation
    // ==========================================

    #[test]
    fn test_sse_event_format() {
        // Test that SSE events are formatted correctly per the SSE spec:
        // event: {type}\ndata: {json}\n\n

        let data = serde_json::json!({"message": "hello", "progress": 50});
        let formatted = format_sse_event(SseEventType::Progress, &data);

        // Must start with "event: progress\n"
        assert!(
            formatted.starts_with("event: progress\n"),
            "Event line must come first"
        );

        // Must contain "data: " line with JSON
        assert!(formatted.contains("data: "), "Must have data line");
        assert!(
            formatted.contains(r#""message":"hello""#),
            "Data must contain JSON"
        );
        assert!(
            formatted.contains(r#""progress":50"#),
            "Data must contain progress"
        );

        // Must end with double newline
        assert!(
            formatted.ends_with("\n\n"),
            "Must end with double newline for SSE"
        );

        // Test all event types format correctly
        for event_type in [
            SseEventType::Progress,
            SseEventType::Output,
            SseEventType::Error,
            SseEventType::Complete,
        ] {
            let formatted = format_sse_event(event_type, &serde_json::json!({}));
            assert!(
                formatted.starts_with(&format!("event: {}\n", event_type.as_str())),
                "Event type {} should format correctly",
                event_type
            );
        }
    }

    #[test]
    fn test_sse_stream_broadcast() {
        let mut stream = SseStream::new();

        // Initially empty
        assert_eq!(stream.pending_count(), 0);

        // Broadcast some events
        stream.broadcast_event(SseEventType::Progress, &serde_json::json!({"step": 1}));
        stream.broadcast_event(SseEventType::Output, &serde_json::json!({"line": "test"}));

        assert_eq!(stream.pending_count(), 2);

        // Drain events
        let events = stream.drain_events();
        assert_eq!(events.len(), 2);
        assert!(events[0].contains("event: progress"));
        assert!(events[1].contains("event: output"));

        // Buffer should be empty after drain
        assert_eq!(stream.pending_count(), 0);
    }

    #[test]
    fn test_sse_heartbeat_format() {
        let heartbeat = format_sse_heartbeat();

        // Heartbeat is a comment (starts with :)
        assert!(
            heartbeat.starts_with(":"),
            "Heartbeat must be SSE comment (start with :)"
        );
        assert!(
            heartbeat.ends_with("\n\n"),
            "Heartbeat must end with double newline"
        );
    }

    #[test]
    fn test_audit_log_written() {
        // Test that audit logs are actually written to the file
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf());

        // Log should not exist yet
        assert!(
            !logger.log_path().exists(),
            "Log file should not exist initially"
        );

        // Log a successful call
        logger
            .log_success(
                "tools/run_script",
                serde_json::json!({"name": "test.ts"}),
                100,
            )
            .expect("Should write log successfully");

        // Log file should now exist
        assert!(logger.log_path().exists(), "Log file should be created");

        // Read and verify content
        let content = fs::read_to_string(logger.log_path()).unwrap();
        assert!(!content.is_empty(), "Log file should have content");

        // Log another entry
        logger
            .log_failure(
                "tools/bad_call",
                serde_json::json!({}),
                50,
                "Invalid params",
            )
            .expect("Should write failure log");

        // Should have two lines
        let content = fs::read_to_string(logger.log_path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "Should have 2 log entries");
    }

    #[test]
    fn test_audit_log_format() {
        // Test that audit log entries have the correct JSONL format
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf());

        let params = serde_json::json!({
            "script": "hello.ts",
            "args": ["--verbose"]
        });

        logger
            .log_success("tools/run_script", params.clone(), 250)
            .expect("Should log successfully");

        // Read and parse the log entry
        let content = fs::read_to_string(logger.log_path()).unwrap();
        let entry: AuditLogEntry =
            serde_json::from_str(content.trim()).expect("Log entry should be valid JSON");

        // Verify all required fields
        assert!(!entry.timestamp.is_empty(), "timestamp must be present");
        assert!(
            entry.timestamp.contains("T"),
            "timestamp must be ISO 8601 format"
        );
        assert_eq!(entry.method, "tools/run_script", "method must match");
        assert_eq!(entry.params, params, "params must match");
        assert_eq!(entry.duration_ms, 250, "duration_ms must match");
        assert!(entry.success, "success must be true");
        assert!(entry.error.is_none(), "error must be None for success");

        // Test failure entry format
        logger
            .log_failure(
                "tools/fail",
                serde_json::json!({}),
                10,
                "Something went wrong",
            )
            .unwrap();

        let content = fs::read_to_string(logger.log_path()).unwrap();
        let last_line = content.lines().last().unwrap();
        let fail_entry: AuditLogEntry = serde_json::from_str(last_line).unwrap();

        assert!(!fail_entry.success, "success must be false for failure");
        assert_eq!(
            fail_entry.error,
            Some("Something went wrong".to_string()),
            "error message must match"
        );
    }

    #[test]
    fn test_audit_entry_constructors() {
        let params = serde_json::json!({"test": true});

        // Test success constructor
        let success = AuditLogEntry::success("my_method", params.clone(), 100);
        assert_eq!(success.method, "my_method");
        assert_eq!(success.params, params);
        assert_eq!(success.duration_ms, 100);
        assert!(success.success);
        assert!(success.error.is_none());

        // Test failure constructor
        let failure = AuditLogEntry::failure("my_method", params.clone(), 50, "oops");
        assert_eq!(failure.method, "my_method");
        assert_eq!(failure.params, params);
        assert_eq!(failure.duration_ms, 50);
        assert!(!failure.success);
        assert_eq!(failure.error, Some("oops".to_string()));
    }

    #[test]
    fn test_sse_event_type_display() {
        assert_eq!(SseEventType::Progress.as_str(), "progress");
        assert_eq!(SseEventType::Output.as_str(), "output");
        assert_eq!(SseEventType::Error.as_str(), "error");
        assert_eq!(SseEventType::Complete.as_str(), "complete");

        assert_eq!(format!("{}", SseEventType::Progress), "progress");
    }

    #[test]
    fn test_iso8601_timestamp_format() {
        let ts = iso8601_now();

        // Should be in format: YYYY-MM-DDTHH:MM:SS.mmmZ
        assert!(ts.len() >= 24, "Timestamp should be at least 24 chars");
        assert!(ts.contains("T"), "Should have T separator");
        assert!(ts.ends_with("Z"), "Should end with Z for UTC");

        // Should be parseable (basic validation)
        let parts: Vec<&str> = ts.split('T').collect();
        assert_eq!(parts.len(), 2, "Should have date and time parts");

        let date_parts: Vec<&str> = parts[0].split('-').collect();
        assert_eq!(date_parts.len(), 3, "Date should have 3 parts");

        // Year should be reasonable
        let year: i32 = date_parts[0].parse().unwrap();
        assert!(year >= 2024, "Year should be current or later");
    }
}

```

### MCP.md

```md
# Script Kit MCP Integration

Script Kit GPUI implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) to expose scripts, scriptlets, and app functionality to AI agents and other MCP clients.

## Quick Start

### 1. Start Script Kit

```bash
./target/release/script-kit-gpui
```

The MCP server starts automatically on port **43210**.

### 2. Get Your Token

```bash
cat ~/.sk/kit/agent-token
```

### 3. Test the Connection

```bash
TOKEN=$(cat ~/.sk/kit/agent-token)

curl -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AI Agent / MCP Client                        │
├─────────────────────────────────────────────────────────────────────┤
│                              │                                       │
│                      JSON-RPC 2.0 over HTTP                          │
│                              │                                       │
├─────────────────────────────────────────────────────────────────────┤
│                       Script Kit MCP Server                          │
│                      (localhost:43210/rpc)                           │
├──────────────┬──────────────┬───────────────┬───────────────────────┤
│   Kit Tools  │ Script Tools │   Resources   │     Authentication    │
│  kit/show    │ scripts/*    │ kit://state   │    Bearer Token       │
│  kit/hide    │              │ scripts://    │  ~/.sk/kit/agent-token  │
│  kit/state   │              │ scriptlets:// │                       │
└──────────────┴──────────────┴───────────────┴───────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────┐
│                         Script Execution                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │   Scripts   │    │  Scriptlets │    │     SDK     │              │
│  │ ~/.sk/kit/    │    │ ~/.sk/kit/    │    │ input()     │              │
│  │  scripts/   │    │ scriptlets/ │    │ output()    │              │
│  └─────────────┘    └─────────────┘    └─────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
```

## Server Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| Port | 43210 | HTTP server port |
| Token File | `~/.sk/kit/agent-token` | Authentication token location |
| Discovery File | `~/.sk/kit/server.json` | Server info for clients |

### Discovery File (`~/.sk/kit/server.json`)

```json
{
  "url": "http://localhost:43210",
  "token": "your-token-here",
  "version": "1.0.0"
}
```

## Authentication

All requests require a Bearer token in the Authorization header:

```bash
curl -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '...'
```

The token is automatically generated on first run and stored at `~/.sk/kit/agent-token`.

## API Reference

### Protocol

- **Transport**: HTTP POST
- **Endpoint**: `http://localhost:43210/rpc`
- **Format**: JSON-RPC 2.0

### Methods

| Method | Description |
|--------|-------------|
| `initialize` | Initialize MCP session |
| `tools/list` | List available tools |
| `tools/call` | Execute a tool |
| `resources/list` | List available resources |
| `resources/read` | Read resource content |

---

## Tools

### Kit Tools

Built-in tools for controlling Script Kit:

#### `kit/show`

Show the Script Kit window.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "kit/show",
    "arguments": {}
  }
}
```

#### `kit/hide`

Hide the Script Kit window.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "kit/hide",
    "arguments": {}
  }
}
```

#### `kit/state`

Get current app state.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "kit/state",
    "arguments": {}
  }
}
```

**Response:**
```json
{
  "visible": false,
  "focused": false,
  "scripts_count": 340,
  "scriptlets_count": 27,
  "current_filter": ""
}
```

### Script Tools

Scripts with a `schema` definition are automatically exposed as MCP tools.

#### Creating a Script Tool

**Option 1: Using `defineSchema()` (Recommended)**

```typescript
import "@johnlindquist/kit"

metadata = {
  name: "My Tool",
  description: "Does something useful",
}

const { input, output } = defineSchema({
  input: {
    message: { type: "string", required: true },
    count: { type: "number", default: 1 },
  },
  output: {
    result: { type: "string" },
  },
} as const)

const { message, count } = await input()
output({ result: `${message} x${count}` })
```

**Option 2: Direct Schema Assignment**

```typescript
import "@johnlindquist/kit"

// Name comes from metadata (preferred) or // Name: comment
metadata = {
  name: "My Tool",
  description: "Does something useful",
}

schema = {
  input: {
    message: { type: "string", required: true },
  },
  output: {
    result: { type: "string" },
  },
}

const data = await input()
output({ result: `Got: ${data.message}` })
```

#### Calling a Script Tool

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "scripts/my-tool",
    "arguments": {
      "message": "Hello",
      "count": 3
    }
  }
}
```

#### Tool Naming

Tool names are derived from `metadata.name` (priority) or `// Name:` comment:

| Source | Example | Tool Name |
|--------|---------|-----------|
| `metadata.name = "My Tool"` | `metadata = { name: "My Tool" }` | `scripts/my-tool` |
| `// Name: My Tool` | Comment at top of file | `scripts/my-tool` |
| Filename | `my-tool.ts` | `scripts/my-tool` |

**Priority**: `metadata.name` > `// Name:` comment > filename

---

## Resources

Resources provide read-only access to Script Kit data.

### `kit://state`

Current app state.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": { "uri": "kit://state" }
}
```

### `scripts://`

List of all scripts.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": { "uri": "scripts://" }
}
```

**Response:**
```json
[
  {
    "name": "Hello World",
    "path": "/Users/x/.sk/kit/scripts/hello-world.ts",
    "extension": "ts",
    "description": "A simple greeting script",
    "has_schema": true
  },
  ...
]
```

### `scriptlets://`

List of all scriptlets.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": { "uri": "scriptlets://" }
}
```

**Response:**
```json
[
  {
    "name": "Current Time",
    "tool": "js",
    "group": "MCP Examples"
  },
  ...
]
```

---

## Schema Reference

### Field Types

| Type | TypeScript | JSON Schema | Example |
|------|------------|-------------|---------|
| `string` | `string` | `{"type": "string"}` | `"hello"` |
| `number` | `number` | `{"type": "number"}` | `42` |
| `boolean` | `boolean` | `{"type": "boolean"}` | `true` |
| `array` | `T[]` | `{"type": "array", "items": {...}}` | `[1, 2, 3]` |
| `object` | `object` | `{"type": "object"}` | `{"a": 1}` |

### Field Properties

| Property | Type | Description |
|----------|------|-------------|
| `type` | string | Field type (required) |
| `description` | string | Human-readable description |
| `required` | boolean | Whether field is required (default: false) |
| `default` | any | Default value if not provided |
| `enum` | array | Allowed values |
| `items` | object | Schema for array items |

### Example Schema

```typescript
const { input, output } = defineSchema({
  input: {
    // Required string with description
    query: {
      type: "string",
      description: "Search query",
      required: true,
    },
    // Optional number with default
    limit: {
      type: "number",
      description: "Max results",
      default: 10,
    },
    // Enum constraint
    sort: {
      type: "string",
      description: "Sort order",
      enum: ["asc", "desc"],
      default: "asc",
    },
    // Array of strings
    tags: {
      type: "array",
      description: "Filter tags",
      items: { type: "string" },
    },
  },
  output: {
    results: {
      type: "array",
      description: "Search results",
    },
    total: {
      type: "number",
      description: "Total count",
    },
  },
} as const)
```

### Generated JSON Schema

The above produces this MCP tool definition:

```json
{
  "name": "scripts/search-tool",
  "description": "Search for items",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Search query"
      },
      "limit": {
        "type": "number",
        "description": "Max results",
        "default": 10
      },
      "sort": {
        "type": "string",
        "description": "Sort order",
        "enum": ["asc", "desc"],
        "default": "asc"
      },
      "tags": {
        "type": "array",
        "description": "Filter tags",
        "items": { "type": "string" }
      }
    },
    "required": ["query"]
  }
}
```

---

## SDK Functions

### `input<T>()`

Get typed input from the agent.

```typescript
const { input } = defineSchema({
  input: {
    name: { type: "string", required: true },
  },
} as const)

const { name } = await input()
// name: string
```

### `output(data)`

Send typed output to the agent. Can be called multiple times - results accumulate.

```typescript
const { output } = defineSchema({
  output: {
    step1: { type: "string" },
    step2: { type: "string" },
  },
} as const)

output({ step1: "Done" })
// Later...
output({ step2: "Also done" })
// Final output: { step1: "Done", step2: "Also done" }
```

### `defineSchema(schema)`

Create typed `input`/`output` functions with full TypeScript inference.

```typescript
const { input, output, schema } = defineSchema({
  input: { /* ... */ },
  output: { /* ... */ },
} as const)

// input() returns typed object
// output() accepts typed object
// schema is the raw schema object
```

### Internal Functions

| Function | Description |
|----------|-------------|
| `_setScriptInput(data)` | Set input data (called by runtime) |
| `_getScriptOutput()` | Get accumulated output |
| `_resetScriptIO()` | Reset input/output state (testing) |

---

## Testing

### Smoke Test Suite

Run the full MCP test suite:

```bash
# Start Script Kit first
./target/release/script-kit-gpui &
sleep 3

# Run tests
./tests/mcp/mcp-smoke-test.sh

# Quick tests only
./tests/mcp/mcp-smoke-test.sh --quick
```

### Manual Testing with curl

```bash
TOKEN=$(cat ~/.sk/kit/agent-token)

# List tools
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq

# Call a tool
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "id":1,
    "method":"tools/call",
    "params":{
      "name":"kit/state",
      "arguments":{}
    }
  }' | jq
```

### Testing with mcp-cli

```bash
# Note: mcp-cli may have OAuth issues - use curl for testing
bunx @wong2/mcp-cli --url "http://localhost:43210/mcp?token=$TOKEN"
```

---

## Example Scripts

See `tests/mcp/scripts/` for complete examples:

| Script | Description |
|--------|-------------|
| `greeting-tool.ts` | Simple greeting with style enum |
| `calculator-tool.ts` | Math operations with validation |
| `file-info-tool.ts` | File system information |
| `text-transform-tool.ts` | Text transformations with arrays |
| `json-tool.ts` | JSON parsing and extraction |
| `no-schema-tool.ts` | Script without schema (not exposed) |

---

## Troubleshooting

### Server Not Responding

```bash
# Check if running
lsof -i :43210

# Check logs
tail -100 ~/.sk/kit/logs/script-kit-gpui.jsonl | grep -i mcp
```

### Token Issues

```bash
# Verify token exists
cat ~/.sk/kit/agent-token

# Token is regenerated on app restart if missing
```

### Tool Not Appearing

1. Ensure script has `schema = {...}` or `defineSchema({...})`
2. Check for syntax errors in schema
3. Restart Script Kit to reload scripts
4. Check logs for parsing errors

### Script Not Executing

Currently, `tools/call` returns `"status": "pending"` - the script is queued but execution and output capture is not yet fully implemented. The `input()`/`output()` functions work correctly when scripts run interactively.

---

## File Locations

| File | Purpose |
|------|---------|
| `~/.sk/kit/agent-token` | Authentication token |
| `~/.sk/kit/server.json` | Server discovery info |
| `~/.sk/kit/scripts/` | User scripts |
| `~/.sk/kit/scriptlets/` | Scriptlet markdown files |
| `~/.sk/kit/logs/script-kit-gpui.jsonl` | Application logs |

---

## Source Files

| File | Description |
|------|-------------|
| `src/mcp_server.rs` | HTTP server and routing |
| `src/mcp_protocol.rs` | JSON-RPC protocol handling |
| `src/mcp_kit_tools.rs` | kit/* tool implementations |
| `src/mcp_script_tools.rs` | scripts/* tool generation |
| `src/mcp_resources.rs` | Resource handlers |
| `src/schema_parser.rs` | Schema extraction from scripts |
| `src/metadata_parser.rs` | Metadata extraction |
| `scripts/kit-sdk.ts` | SDK with input/output functions |

```

### tests/mcp/README.md

```md
# MCP Test Suite

This directory contains smoke tests and example scripts for the Script Kit MCP integration.

## Directory Structure

```
tests/mcp/
├── README.md                 # This file
├── mcp-smoke-test.sh         # Main test runner
├── scripts/                  # Example script tools
│   ├── greeting-tool.ts      # Simple greeting with enum
│   ├── calculator-tool.ts    # Math operations
│   ├── file-info-tool.ts     # File system info
│   ├── text-transform-tool.ts # Text transformations
│   ├── json-tool.ts          # JSON processing
│   └── no-schema-tool.ts     # Script without schema (negative test)
└── scriptlets/
    └── mcp-examples.md       # Example scriptlets
```

## Running Tests

### Prerequisites

1. Build and start Script Kit:
   ```bash
   cargo build --release
   ./target/release/script-kit-gpui &
   sleep 3
   ```

2. Verify MCP server is running:
   ```bash
   lsof -i :43210
   ```

### Run All Tests

```bash
./tests/mcp/mcp-smoke-test.sh
```

### Run Quick Tests Only

```bash
./tests/mcp/mcp-smoke-test.sh --quick
```

### Expected Output

```
╔════════════════════════════════════════════════════════════╗
║          MCP Server Smoke Test Suite                       ║
╚════════════════════════════════════════════════════════════╝

Checking MCP server at http://localhost:43210...
Server is running

=== Initialize ===
  PASS Returns jsonrpc 2.0
  PASS Returns id
  PASS Has result
  PASS Has capabilities
  PASS Server name is script-kit

=== Tools List ===
  PASS Returns tools array
  PASS Has kit/show tool
  PASS Has kit/hide tool
  PASS Has kit/state tool

...

╔════════════════════════════════════════════════════════════╗
║                        Summary                             ║
╚════════════════════════════════════════════════════════════╝

  Tests Run:    35
  Passed:       35
  Failed:       0

All tests passed!
```

## Example Scripts

### Creating a New MCP Tool

1. Create a script in `~/.sk/kit/scripts/`:

```typescript
import "@johnlindquist/kit"

metadata = {
  name: "My Tool Name",
  description: "What this tool does",
}

const { input, output } = defineSchema({
  input: {
    param1: { type: "string", required: true },
    param2: { type: "number", default: 10 },
  },
  output: {
    result: { type: "string" },
  },
} as const)

const { param1, param2 } = await input()
output({ result: `${param1} x ${param2}` })
```

2. The tool will appear as `scripts/my-tool-name` in the MCP tools list.

### Testing Your Tool

```bash
TOKEN=$(cat ~/.sk/kit/agent-token)

# List tools (verify your tool appears)
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | \
  jq '.result.tools[] | select(.name | contains("my-tool"))'

# Call your tool
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "id":1,
    "method":"tools/call",
    "params":{
      "name":"scripts/my-tool-name",
      "arguments":{"param1":"hello","param2":5}
    }
  }' | jq
```

## Test Categories

| Category | Tests | Description |
|----------|-------|-------------|
| Initialize | 5 | MCP session initialization |
| Tools List | 4 | Tool discovery |
| Kit Tools | 4 | Built-in kit/* tools |
| Resources List | 4 | Resource discovery |
| Resources Read | 5 | Reading resource content |
| Script Tools | 5 | Script-based tools |
| Error Handling | 4 | Error responses |
| Authentication | 2 | Token validation |
| Metadata Priority | 2 | metadata.name vs // Name: |

## Adding New Tests

Edit `mcp-smoke-test.sh` and add a new test function:

```bash
test_my_feature() {
  echo -e "${YELLOW}=== My Feature ===${NC}"
  
  local response
  response=$(rpc "method/name" '{"param":"value"}')
  
  assert_json "Test description" "$response" ".path.to.value" "expected"
  
  echo ""
}
```

Then call it from `main()`.

## See Also

- [MCP.md](../../MCP.md) - Full MCP documentation
- [PROTOCOL.md](../../docs/PROTOCOL.md) - Protocol reference
- [SDK Tests](../sdk/) - SDK function tests

```

### tests/mcp/mcp-smoke-test.sh

```sh
#!/bin/bash
# MCP Server Smoke Test Suite
# Tests the MCP JSON-RPC API for Script Kit GPUI
#
# Usage:
#   ./tests/mcp/mcp-smoke-test.sh          # Run all tests
#   ./tests/mcp/mcp-smoke-test.sh --quick  # Run essential tests only
#
# Prerequisites:
#   - Script Kit GPUI app must be running
#   - Token file at ~/.sk/kit/agent-token
#   - curl and jq installed

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Config
MCP_PORT="${MCP_PORT:-43210}"
MCP_HOST="${MCP_HOST:-localhost}"
TOKEN_FILE="${HOME}/.sk/kit/agent-token"
BASE_URL="http://${MCP_HOST}:${MCP_PORT}"

# Counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Get token
get_token() {
  if [[ ! -f "$TOKEN_FILE" ]]; then
    echo -e "${RED}ERROR: Token file not found at $TOKEN_FILE${NC}"
    echo "Make sure Script Kit GPUI is running"
    exit 1
  fi
  cat "$TOKEN_FILE"
}

TOKEN=$(get_token)

# JSON-RPC helper
rpc() {
  local method="$1"
  local params="$2"
  local id="$3"
  
  # Use explicit defaults to avoid bash ${:-} issues with curly braces
  if [[ -z "$params" ]]; then
    params="{}"
  fi
  if [[ -z "$id" ]]; then
    id="1"
  fi
  
  curl -s -X POST "${BASE_URL}/rpc" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":${id},\"method\":\"${method}\",\"params\":${params}}"
}

# Test helper
run_test() {
  local name="$1"
  local expected="$2"
  local actual="$3"
  
  TESTS_RUN=$((TESTS_RUN + 1))
  
  if [[ "$actual" == "$expected" ]]; then
    echo -e "${GREEN}  PASS${NC} $name"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
  else
    echo -e "${RED}  FAIL${NC} $name"
    echo -e "       Expected: $expected"
    echo -e "       Actual:   $actual"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    return 1
  fi
}

# Assert contains helper
assert_contains() {
  local name="$1"
  local haystack="$2"
  local needle="$3"
  
  TESTS_RUN=$((TESTS_RUN + 1))
  
  if echo "$haystack" | grep -q "$needle"; then
    echo -e "${GREEN}  PASS${NC} $name"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
  else
    echo -e "${RED}  FAIL${NC} $name"
    echo -e "       Expected to contain: $needle"
    echo -e "       Actual: ${haystack:0:200}..."
    TESTS_FAILED=$((TESTS_FAILED + 1))
    return 1
  fi
}

# Assert JSON path equals
assert_json() {
  local name="$1"
  local json="$2"
  local path="$3"
  local expected="$4"
  
  local actual
  actual=$(echo "$json" | jq -r "$path" 2>/dev/null || echo "JQ_ERROR")
  run_test "$name" "$expected" "$actual"
}

# Assert JSON path exists
assert_json_exists() {
  local name="$1"
  local json="$2"
  local path="$3"
  
  TESTS_RUN=$((TESTS_RUN + 1))
  
  local value
  value=$(echo "$json" | jq -e "$path" 2>/dev/null)
  
  if [[ $? -eq 0 ]] && [[ "$value" != "null" ]]; then
    echo -e "${GREEN}  PASS${NC} $name"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
  else
    echo -e "${RED}  FAIL${NC} $name"
    echo -e "       Path '$path' not found or null in JSON"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    return 1
  fi
}

# Check server is running
check_server() {
  echo -e "${BLUE}Checking MCP server at ${BASE_URL}...${NC}"
  
  if ! curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/rpc" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}' | grep -q "200"; then
    echo -e "${RED}ERROR: MCP server not responding${NC}"
    echo "Make sure Script Kit GPUI is running"
    exit 1
  fi
  
  echo -e "${GREEN}Server is running${NC}\n"
}

# ============================================================================
# Test Suites
# ============================================================================

test_initialize() {
  echo -e "${YELLOW}=== Initialize ===${NC}"
  
  local response
  response=$(rpc "initialize" '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}')
  
  assert_json "Returns jsonrpc 2.0" "$response" ".jsonrpc" "2.0"
  assert_json "Returns id" "$response" ".id" "1"
  assert_json_exists "Has result" "$response" ".result"
  assert_json_exists "Has capabilities" "$response" ".result.capabilities"
  assert_json "Server name is script-kit" "$response" ".result.serverInfo.name" "script-kit"
  
  echo ""
}

test_tools_list() {
  echo -e "${YELLOW}=== Tools List ===${NC}"
  
  local response
  response=$(rpc "tools/list" "{}")
  
  assert_json_exists "Returns tools array" "$response" ".result.tools"
  
  # Check kit/* tools exist
  local tools
  tools=$(echo "$response" | jq -r '.result.tools[].name')
  
  assert_contains "Has kit/show tool" "$tools" "kit/show"
  assert_contains "Has kit/hide tool" "$tools" "kit/hide"
  assert_contains "Has kit/state tool" "$tools" "kit/state"
  
  echo ""
}

test_kit_tools() {
  echo -e "${YELLOW}=== Kit Tools ===${NC}"
  
  # Test kit/state
  local state_response
  state_response=$(rpc "tools/call" '{"name":"kit/state","arguments":{}}')
  
  assert_json_exists "kit/state returns content" "$state_response" ".result.content"
  assert_json "kit/state content type is text" "$state_response" ".result.content[0].type" "text"
  
  # Parse the state JSON from the text content
  local state_text
  state_text=$(echo "$state_response" | jq -r '.result.content[0].text')
  assert_contains "State contains visible field" "$state_text" '"visible"'
  assert_contains "State contains focused field" "$state_text" '"focused"'
  
  # Test kit/show
  local show_response
  show_response=$(rpc "tools/call" '{"name":"kit/show","arguments":{}}')
  assert_json "kit/show returns success" "$show_response" ".result.content[0].type" "text"
  
  # Test kit/hide
  local hide_response
  hide_response=$(rpc "tools/call" '{"name":"kit/hide","arguments":{}}')
  assert_json "kit/hide returns success" "$hide_response" ".result.content[0].type" "text"
  
  echo ""
}

test_resources_list() {
  echo -e "${YELLOW}=== Resources List ===${NC}"
  
  local response
  response=$(rpc "resources/list" "{}")
  
  assert_json_exists "Returns resources array" "$response" ".result.resources"
  
  local resources
  resources=$(echo "$response" | jq -r '.result.resources[].uri')
  
  assert_contains "Has kit://state resource" "$resources" "kit://state"
  assert_contains "Has scripts:// resource" "$resources" "scripts://"
  assert_contains "Has scriptlets:// resource" "$resources" "scriptlets://"
  
  echo ""
}

test_resources_read() {
  echo -e "${YELLOW}=== Resources Read ===${NC}"
  
  # Read kit://state
  local state_response
  state_response=$(rpc "resources/read" '{"uri":"kit://state"}')
  
  assert_json_exists "State resource returns contents" "$state_response" ".result.contents"
  assert_json "State resource URI" "$state_response" ".result.contents[0].uri" "kit://state"
  
  # Read scripts://
  local scripts_response
  scripts_response=$(rpc "resources/read" '{"uri":"scripts://"}')
  
  assert_json_exists "Scripts resource returns contents" "$scripts_response" ".result.contents"
  
  local scripts_text
  scripts_text=$(echo "$scripts_response" | jq -r '.result.contents[0].text')
  assert_contains "Scripts contain name field" "$scripts_text" '"name"'
  assert_contains "Scripts contain path field" "$scripts_text" '"path"'
  
  # Read scriptlets://
  local scriptlets_response
  scriptlets_response=$(rpc "resources/read" '{"uri":"scriptlets://"}')
  
  assert_json_exists "Scriptlets resource returns contents" "$scriptlets_response" ".result.contents"
  
  echo ""
}

test_script_tools() {
  echo -e "${YELLOW}=== Script Tools ===${NC}"
  
  local response
  response=$(rpc "tools/list" "{}")
  
  # Get script tools (those starting with scripts/)
  local script_tools
  script_tools=$(echo "$response" | jq '[.result.tools[] | select(.name | startswith("scripts/"))]')
  
  local tool_count
  tool_count=$(echo "$script_tools" | jq 'length')
  
  echo "  Found $tool_count script tools"
  
  if [[ "$tool_count" -gt 0 ]]; then
    # Check first script tool has required fields
    local first_tool
    first_tool=$(echo "$script_tools" | jq '.[0]')
    
    assert_json_exists "Script tool has name" "$first_tool" ".name"
    assert_json_exists "Script tool has description" "$first_tool" ".description"
    assert_json_exists "Script tool has inputSchema" "$first_tool" ".inputSchema"
    assert_json "Script tool inputSchema type is object" "$first_tool" ".inputSchema.type" "object"
    
    # Test calling a script tool
    local tool_name
    tool_name=$(echo "$first_tool" | jq -r '.name')
    
    local call_response
    call_response=$(rpc "tools/call" "{\"name\":\"${tool_name}\",\"arguments\":{}}")
    
    assert_json_exists "Script tool call returns content" "$call_response" ".result.content"
    
    local result_text
    result_text=$(echo "$call_response" | jq -r '.result.content[0].text')
    assert_contains "Script tool result has status" "$result_text" '"status"'
  else
    echo -e "${YELLOW}  SKIP${NC} No script tools with schema found"
  fi
  
  echo ""
}

test_error_handling() {
  echo -e "${YELLOW}=== Error Handling ===${NC}"
  
  # Invalid method
  local invalid_method
  invalid_method=$(rpc "invalid/method" "{}")
  assert_json_exists "Invalid method returns error" "$invalid_method" ".error"
  
  # Missing required params
  local missing_params
  missing_params=$(rpc "tools/call" "{}")
  assert_json_exists "Missing params returns error" "$missing_params" ".error"
  
  # Unknown tool
  local unknown_tool
  unknown_tool=$(rpc "tools/call" '{"name":"unknown/tool","arguments":{}}')
  assert_json_exists "Unknown tool returns error" "$unknown_tool" ".error"
  
  # Invalid resource URI
  local invalid_resource
  invalid_resource=$(rpc "resources/read" '{"uri":"invalid://resource"}')
  assert_json_exists "Invalid resource returns error" "$invalid_resource" ".error"
  
  echo ""
}

test_authentication() {
  echo -e "${YELLOW}=== Authentication ===${NC}"
  
  # Test with invalid token
  local bad_response
  bad_response=$(curl -s -X POST "${BASE_URL}/rpc" \
    -H "Authorization: Bearer invalid-token" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')
  
  TESTS_RUN=$((TESTS_RUN + 1))
  if echo "$bad_response" | grep -qi "invalid\|unauthorized\|error"; then
    echo -e "${GREEN}  PASS${NC} Invalid token rejected"
    TESTS_PASSED=$((TESTS_PASSED + 1))
  else
    echo -e "${RED}  FAIL${NC} Invalid token should be rejected"
    TESTS_FAILED=$((TESTS_FAILED + 1))
  fi
  
  # Test without token
  local no_token_response
  no_token_response=$(curl -s -X POST "${BASE_URL}/rpc" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')
  
  TESTS_RUN=$((TESTS_RUN + 1))
  if echo "$no_token_response" | grep -qi "missing\|unauthorized\|error"; then
    echo -e "${GREEN}  PASS${NC} Missing token rejected"
    TESTS_PASSED=$((TESTS_PASSED + 1))
  else
    echo -e "${RED}  FAIL${NC} Missing token should be rejected"
    TESTS_FAILED=$((TESTS_FAILED + 1))
  fi
  
  echo ""
}

test_metadata_name_priority() {
  echo -e "${YELLOW}=== Metadata Name Priority ===${NC}"
  
  local response
  response=$(rpc "tools/list" "{}")
  
  # Check if the test script exists and uses metadata.name
  local test_tool
  test_tool=$(echo "$response" | jq '.result.tools[] | select(.name | contains("mcp-io-test-via-metadata"))')
  
  if [[ -n "$test_tool" ]] && [[ "$test_tool" != "null" ]]; then
    assert_json "Tool uses metadata.name not // Name comment" "$test_tool" ".name" "scripts/mcp-io-test-via-metadata"
    assert_json "Description from metadata" "$test_tool" ".description" "Tests that metadata.name is used for MCP tool naming"
  else
    echo -e "${YELLOW}  SKIP${NC} Test script mcp-test-input-output.ts not found"
  fi
  
  echo ""
}

# ============================================================================
# Main
# ============================================================================

main() {
  echo ""
  echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
  echo -e "${BLUE}║          MCP Server Smoke Test Suite                       ║${NC}"
  echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
  echo ""
  
  check_server
  
  if [[ "$1" == "--quick" ]]; then
    echo -e "${YELLOW}Running quick tests only...${NC}\n"
    test_initialize
    test_tools_list
    test_resources_list
  else
    test_initialize
    test_tools_list
    test_kit_tools
    test_resources_list
    test_resources_read
    test_script_tools
    test_error_handling
    test_authentication
    test_metadata_name_priority
  fi
  
  # Summary
  echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
  echo -e "${BLUE}║                        Summary                             ║${NC}"
  echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
  echo ""
  echo -e "  Tests Run:    ${TESTS_RUN}"
  echo -e "  ${GREEN}Passed:       ${TESTS_PASSED}${NC}"
  echo -e "  ${RED}Failed:       ${TESTS_FAILED}${NC}"
  echo ""
  
  if [[ $TESTS_FAILED -gt 0 ]]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
  else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
  fi
}

main "$@"

```

### tests/mcp/scripts/json-tool.ts

```ts
// Name: JSON Tool
// Description: Parse and manipulate JSON

import "@johnlindquist/kit"

metadata = {
  name: "JSON Processor",
  description: "Parse, format, and extract data from JSON",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    json: {
      type: "string",
      description: "JSON string to process",
      required: true,
    },
    action: {
      type: "string",
      description: "Action to perform on the JSON",
      enum: ["parse", "format", "minify", "extract"],
      default: "format",
    },
    path: {
      type: "string",
      description: "JSON path for extraction (e.g., 'data.users[0].name')",
    },
  },
  output: {
    success: {
      type: "boolean",
      description: "Whether the operation succeeded",
    },
    result: {
      type: "string",
      description: "The processed JSON or extracted value",
    },
    type: {
      type: "string",
      description: "Type of the result value",
    },
    error: {
      type: "string",
      description: "Error message if parsing failed",
    },
  },
} as const)

const { json, action, path } = await input()

try {
  const parsed = JSON.parse(json)
  
  switch (action) {
    case "parse":
      output({
        success: true,
        result: JSON.stringify(parsed),
        type: Array.isArray(parsed) ? "array" : typeof parsed,
      })
      break
      
    case "format":
      output({
        success: true,
        result: JSON.stringify(parsed, null, 2),
        type: Array.isArray(parsed) ? "array" : typeof parsed,
      })
      break
      
    case "minify":
      output({
        success: true,
        result: JSON.stringify(parsed),
        type: Array.isArray(parsed) ? "array" : typeof parsed,
      })
      break
      
    case "extract":
      if (!path) {
        output({
          success: false,
          error: "Path required for extract action",
        })
      } else {
        // Simple path extraction (supports dot notation and array indices)
        const parts = path.replace(/\[(\d+)\]/g, ".$1").split(".")
        let value: unknown = parsed
        for (const part of parts) {
          if (value && typeof value === "object") {
            value = (value as Record<string, unknown>)[part]
          } else {
            value = undefined
            break
          }
        }
        output({
          success: true,
          result: typeof value === "string" ? value : JSON.stringify(value),
          type: Array.isArray(value) ? "array" : typeof value,
        })
      }
      break
  }
} catch (err) {
  output({
    success: false,
    error: err instanceof Error ? err.message : "Invalid JSON",
  })
}

if (!metadata.mcp) {
  const result = _getScriptOutput()
  await div(`<pre class="p-4 text-sm">${JSON.stringify(result, null, 2)}</pre>`)
}

```

### tests/mcp/scripts/text-transform-tool.ts

```ts
// Name: Text Transform Tool
// Description: Transform text in various ways

import "@johnlindquist/kit"

metadata = {
  name: "Text Transformer",
  description: "Apply various transformations to text input",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    text: {
      type: "string",
      description: "The text to transform",
      required: true,
    },
    transforms: {
      type: "array",
      description: "List of transformations to apply in order",
      items: {
        type: "string",
        enum: ["uppercase", "lowercase", "reverse", "trim", "slug", "base64"],
      },
      default: ["trim"],
    },
  },
  output: {
    original: {
      type: "string",
      description: "Original input text",
    },
    result: {
      type: "string",
      description: "Transformed text",
    },
    transforms_applied: {
      type: "array",
      description: "List of transformations that were applied",
    },
  },
} as const)

const { text, transforms } = await input()

let result = text
const applied: string[] = []

for (const transform of transforms || ["trim"]) {
  switch (transform) {
    case "uppercase":
      result = result.toUpperCase()
      applied.push("uppercase")
      break
    case "lowercase":
      result = result.toLowerCase()
      applied.push("lowercase")
      break
    case "reverse":
      result = result.split("").reverse().join("")
      applied.push("reverse")
      break
    case "trim":
      result = result.trim()
      applied.push("trim")
      break
    case "slug":
      result = result
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "")
      applied.push("slug")
      break
    case "base64":
      result = Buffer.from(result).toString("base64")
      applied.push("base64")
      break
  }
}

output({
  original: text,
  result,
  transforms_applied: applied,
})

if (!metadata.mcp) {
  await div(`
    <div class="p-4 space-y-4">
      <div class="text-gray-400">Original: ${text}</div>
      <div class="text-2xl font-mono">${result}</div>
      <div class="text-sm text-gray-500">Applied: ${applied.join(" -> ")}</div>
    </div>
  `)
}

```

### tests/mcp/scripts/greeting-tool.ts

```ts
// Name: Greeting Tool
// Description: Simple greeting example for MCP

import "@johnlindquist/kit"

// Typed metadata takes priority over comments
metadata = {
  name: "Greeting Generator",
  description: "Generate personalized greetings with customizable format",
  author: "Script Kit",
  version: "1.0.0",
}

// Define schema with full type inference
const { input, output } = defineSchema({
  input: {
    name: {
      type: "string",
      description: "Name of the person to greet",
      required: true,
    },
    style: {
      type: "string",
      description: "Greeting style",
      enum: ["formal", "casual", "enthusiastic"],
      default: "casual",
    },
  },
  output: {
    greeting: {
      type: "string",
      description: "The generated greeting message",
    },
    style_used: {
      type: "string",
      description: "The style that was applied",
    },
  },
} as const)

// Get typed input
const { name, style } = await input()

// Generate greeting based on style
let greeting: string
switch (style) {
  case "formal":
    greeting = `Good day, ${name}. It is a pleasure to make your acquaintance.`
    break
  case "enthusiastic":
    greeting = `HEY ${name.toUpperCase()}! SO AWESOME TO SEE YOU! 🎉`
    break
  case "casual":
  default:
    greeting = `Hey ${name}! What's up?`
}

// Send typed output
output({ greeting, style_used: style || "casual" })

// For interactive use, show the greeting
if (!metadata.mcp) {
  await div(`<div class="p-8 text-2xl">${greeting}</div>`)
}

```

### tests/mcp/scripts/file-info-tool.ts

```ts
// Name: File Info Tool  
// Description: Get information about files

import "@johnlindquist/kit"
import { stat } from "node:fs/promises"
import { basename, extname, dirname } from "node:path"

metadata = {
  name: "File Information",
  description: "Get detailed information about a file path",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    path: {
      type: "string",
      description: "Absolute path to the file",
      required: true,
    },
  },
  output: {
    exists: {
      type: "boolean",
      description: "Whether the file exists",
    },
    name: {
      type: "string",
      description: "File name without directory",
    },
    extension: {
      type: "string",
      description: "File extension",
    },
    directory: {
      type: "string",
      description: "Parent directory path",
    },
    size_bytes: {
      type: "number",
      description: "File size in bytes",
    },
    is_directory: {
      type: "boolean",
      description: "Whether path is a directory",
    },
    modified: {
      type: "string",
      description: "Last modified timestamp (ISO)",
    },
    error: {
      type: "string",
      description: "Error message if file access failed",
    },
  },
} as const)

const { path } = await input()

try {
  const stats = await stat(path)
  
  output({
    exists: true,
    name: basename(path),
    extension: extname(path),
    directory: dirname(path),
    size_bytes: stats.size,
    is_directory: stats.isDirectory(),
    modified: stats.mtime.toISOString(),
  })
} catch (err) {
  output({
    exists: false,
    name: basename(path),
    extension: extname(path),
    directory: dirname(path),
    error: err instanceof Error ? err.message : String(err),
  })
}

if (!metadata.mcp) {
  const info = _getScriptOutput()
  await div(`<pre class="p-4 text-sm">${JSON.stringify(info, null, 2)}</pre>`)
}

```

### tests/mcp/scripts/calculator-tool.ts

```ts
// Name: Calculator Tool
// Description: Perform basic math operations

import "@johnlindquist/kit"

metadata = {
  name: "Math Calculator",
  description: "Perform arithmetic operations on two numbers",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    a: {
      type: "number",
      description: "First operand",
      required: true,
    },
    b: {
      type: "number", 
      description: "Second operand",
      required: true,
    },
    operation: {
      type: "string",
      description: "Math operation to perform",
      enum: ["add", "subtract", "multiply", "divide"],
      required: true,
    },
  },
  output: {
    result: {
      type: "number",
      description: "The calculation result",
    },
    expression: {
      type: "string",
      description: "Human-readable expression",
    },
    error: {
      type: "string",
      description: "Error message if operation failed",
    },
  },
} as const)

const { a, b, operation } = await input()

let result: number
let expression: string
let error: string | undefined

const ops: Record<string, { symbol: string; fn: (a: number, b: number) => number }> = {
  add: { symbol: "+", fn: (a, b) => a + b },
  subtract: { symbol: "-", fn: (a, b) => a - b },
  multiply: { symbol: "*", fn: (a, b) => a * b },
  divide: { symbol: "/", fn: (a, b) => a / b },
}

const op = ops[operation]
if (!op) {
  error = `Unknown operation: ${operation}`
  result = NaN
  expression = "ERROR"
} else if (operation === "divide" && b === 0) {
  error = "Division by zero"
  result = NaN
  expression = `${a} / 0 = undefined`
} else {
  result = op.fn(a, b)
  expression = `${a} ${op.symbol} ${b} = ${result}`
}

output({ result, expression, ...(error && { error }) })

if (!metadata.mcp) {
  await div(`<div class="p-8 font-mono text-2xl">${expression}</div>`)
}

```

### tests/mcp/scripts/no-schema-tool.ts

```ts
// Name: No Schema Tool
// Description: A script without schema (should NOT appear as MCP tool)

import "@johnlindquist/kit"

// This script has no schema, so it should NOT be exposed as an MCP tool
// Only scripts with `schema = {...}` or `defineSchema({...})` become tools

const name = await arg("What's your name?")
await div(`<div class="p-8 text-2xl">Hello, ${name}!</div>`)

```

### tests/mcp/scriptlets/mcp-examples.md

```md
# MCP Example Scriptlets

These scriptlets demonstrate various patterns for MCP integration.

## Quick Actions

### Current Time
<!-- 
group: MCP Examples
tool: js
-->
```js
new Date().toISOString()
```

### Random UUID  
<!--
group: MCP Examples
tool: js
-->
```js
crypto.randomUUID()
```

### System Info
<!--
group: MCP Examples
tool: js
-->
```js
JSON.stringify({
  platform: process.platform,
  arch: process.arch,
  nodeVersion: process.version,
  cwd: process.cwd(),
  user: process.env.USER || process.env.USERNAME,
}, null, 2)
```

## Templates

### Greeting Template
<!--
group: MCP Examples
tool: template
inputs:
  - name: string
  - time: enum[morning,afternoon,evening]
-->
Good {{time}}, {{name}}! How can I help you today?

### Meeting Notes Template
<!--
group: MCP Examples
tool: template
inputs:
  - title: string
  - attendees: string
  - date: string
-->
# Meeting: {{title}}
**Date:** {{date}}
**Attendees:** {{attendees}}

## Agenda
- [ ] Item 1
- [ ] Item 2

## Notes


## Action Items
- [ ] 

### Email Template
<!--
group: MCP Examples
tool: template
inputs:
  - recipient: string
  - subject: string
-->
To: {{recipient}}
Subject: {{subject}}

Dear {{recipient}},



Best regards,
${process.env.USER}

## Shell Commands

### List Downloads
<!--
group: MCP Examples
tool: bash
-->
```bash
ls -la ~/Downloads | head -20
```

### Git Status
<!--
group: MCP Examples  
tool: bash
-->
```bash
git status --short 2>/dev/null || echo "Not a git repository"
```

### Disk Usage
<!--
group: MCP Examples
tool: bash
-->
```bash
df -h | head -5
```

## Paste Snippets

### JSON Object Template
<!--
group: MCP Examples
tool: paste
-->
```json
{
  "name": "",
  "version": "1.0.0",
  "description": ""
}
```

### TypeScript Function Template
<!--
group: MCP Examples
tool: paste
-->
```typescript
export function {{name}}({{params}}): {{returnType}} {
  // TODO: Implement
}
```

### Console Debug
<!--
group: MCP Examples
tool: paste
expand: ,debug
-->
console.log('[DEBUG]', JSON.stringify({}, null, 2));

## TypeScript Scriptlets

### Calculate Days Until
<!--
group: MCP Examples
tool: ts
-->
```ts
const targetDate = new Date('2025-12-31');
const today = new Date();
const diff = targetDate.getTime() - today.getTime();
const days = Math.ceil(diff / (1000 * 60 * 60 * 24));
await div(`<div class="p-4 text-2xl">Days until Dec 31: ${days}</div>`);
```

### Clipboard Word Count
<!--
group: MCP Examples
tool: ts
-->
```ts
const text = await clipboard.readText();
const words = text.trim().split(/\s+/).filter(w => w.length > 0).length;
const chars = text.length;
await div(`
  <div class="p-4">
    <div class="text-xl">Words: ${words}</div>
    <div class="text-xl">Characters: ${chars}</div>
  </div>
`);
```

### URL Shortener Preview
<!--
group: MCP Examples
tool: ts
-->
```ts
const url = await arg("Enter URL to preview");
try {
  const parsed = new URL(url);
  await div(`
    <div class="p-4 space-y-2">
      <div><strong>Protocol:</strong> ${parsed.protocol}</div>
      <div><strong>Host:</strong> ${parsed.host}</div>
      <div><strong>Path:</strong> ${parsed.pathname}</div>
      <div><strong>Query:</strong> ${parsed.search || '(none)'}</div>
    </div>
  `);
} catch (e) {
  await div(`<div class="p-4 text-red-500">Invalid URL</div>`);
}
```

```

### tests/smoke/test-define-schema.ts

```ts
import '../../scripts/kit-sdk';

console.error('[TEST] Starting defineSchema test');

// Test defineSchema
const { input, output } = defineSchema({
  input: {
    name: { type: "string", required: true },
    count: { type: "number", default: 1 },
  },
  output: {
    greeting: { type: "string" },
    processed: { type: "boolean" },
  },
} as const);

console.error('[TEST] defineSchema created input/output functions');
console.error('[TEST] typeof input:', typeof input);
console.error('[TEST] typeof output:', typeof output);

// Test _setScriptInput to simulate MCP providing input
console.error('[TEST] Testing _setScriptInput...');
_setScriptInput({ name: "Claude", count: 3 });

// Now input() should return the data we set
const inputData = await input();
console.error('[TEST] input() returned:', JSON.stringify(inputData));

// Verify input values
if (inputData.name !== "Claude") {
  console.error('[TEST] FAIL: name should be "Claude", got:', inputData.name);
  process.exit(1);
}
if (inputData.count !== 3) {
  console.error('[TEST] FAIL: count should be 3, got:', inputData.count);
  process.exit(1);
}
console.error('[TEST] PASS: input() returned correct values');

// Test output - call it multiple times to test accumulation
output({ greeting: `Hello ${inputData.name}!` });
console.error('[TEST] First output() called');

output({ processed: true });
console.error('[TEST] Second output() called');

// Check accumulated output using internal function
const accumulatedOutput = _getScriptOutput();
console.error('[TEST] _getScriptOutput():', JSON.stringify(accumulatedOutput));

// Verify accumulated output
if (accumulatedOutput.greeting !== "Hello Claude!") {
  console.error('[TEST] FAIL: greeting should be "Hello Claude!", got:', accumulatedOutput.greeting);
  process.exit(1);
}
if (accumulatedOutput.processed !== true) {
  console.error('[TEST] FAIL: processed should be true, got:', accumulatedOutput.processed);
  process.exit(1);
}
console.error('[TEST] PASS: output() accumulated correctly');

console.error('[TEST] All tests passed!');
process.exit(0);

```


---
## Implementation Guide

### Adding a New MCP Method

```rust
// File: src/mcp_protocol.rs
// Location: McpMethod enum and from_str implementation

// 1. Add to McpMethod enum
pub enum McpMethod {
    Initialize,
    ToolsList,
    ToolsCall,
    ResourcesList,
    ResourcesRead,
    YourNewMethod,  // Add here
}

// 2. Add to from_str
impl McpMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "initialize" => Some(Self::Initialize),
            // ... existing methods ...
            "your/method" => Some(Self::YourNewMethod),
            _ => None,
        }
    }
}

// 3. Add handler in handle_request_with_context
match McpMethod::from_str(&request.method) {
    // ... existing handlers ...
    Some(McpMethod::YourNewMethod) => handle_your_method(request),
    // ...
}

// 4. Implement handler function
fn handle_your_method(request: JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        request.id,
        serde_json::json!({ "result": "your data" }),
    )
}
```

### Adding a New Kit Tool

```rust
// File: src/mcp_kit_tools.rs

// 1. Add tool definition
pub fn get_kit_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        // ... existing tools ...
        ToolDefinition {
            name: "kit/your-tool".to_string(),
            description: "Description of your tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "param1": { "type": "string", "description": "..." }
                }
            }),
        },
    ]
}

// 2. Handle the call
pub fn handle_kit_tool_call(name: &str, arguments: &Value) -> ToolResult {
    match name {
        // ... existing handlers ...
        "kit/your-tool" => {
            let param1 = arguments.get("param1").and_then(|v| v.as_str());
            ToolResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: format!("Result: {:?}", param1),
                }],
                is_error: None,
            }
        }
        _ => // error handling
    }
}
```

### Adding a New Resource

```rust
// File: src/mcp_resources.rs

// 1. Add to get_resource_definitions()
McpResource {
    uri: "your://resource".to_string(),
    name: "Your Resource".to_string(),
    description: Some("Description".to_string()),
    mime_type: "application/json".to_string(),
}

// 2. Add handler in read_resource()
pub fn read_resource(...) -> Result<ResourceContent, String> {
    match uri {
        // ... existing handlers ...
        "your://resource" => read_your_resource(),
        _ => Err(format!("Resource not found: {}", uri)),
    }
}

fn read_your_resource() -> Result<ResourceContent, String> {
    let data = serde_json::json!({ "key": "value" });
    Ok(ResourceContent {
        uri: "your://resource".to_string(),
        mime_type: "application/json".to_string(),
        text: serde_json::to_string_pretty(&data).unwrap(),
    })
}
```

### Extending Schema Parser for New Patterns

```rust
// File: src/schema_parser.rs

// To support a new schema pattern like `mySchema({...})`:
fn find_schema_assignment(content: &str) -> Option<(usize, usize)> {
    // ... existing patterns ...
    
    // Add new pattern
    let my_patterns = ["mySchema({", "mySchema ({"];
    for pattern in my_patterns {
        if let Some(idx) = content.find(pattern) {
            let after_define = idx + pattern.len() - 1;
            return Some((idx, after_define));
        }
    }
    
    None
}
```

### Testing

```bash
# Run all MCP tests
cargo test mcp

# Run schema parser tests
cargo test schema_parser

# Run MCP smoke tests (requires running app)
./target/release/script-kit-gpui &
sleep 3
./tests/mcp/mcp-smoke-test.sh

# Manual testing with curl
TOKEN=$(cat ~/.sk/kit/agent-token)
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq
```

---
## Instructions For The Next AI Agent

You are reading the "MCP and Schema System Expert Bundle". This file is self-contained and includes all the context you should assume you have.

Your job:

* Design and describe the minimal, safe changes needed to resolve any issues or implement new features in the MCP/schema system.
* Operate **only** on the files and code snippets included in this bundle. If you need additional files or context, clearly say so.

When you propose changes, follow these rules strictly:

1. Always provide **precise code snippets** that can be copy-pasted directly into the repo.
2. Always include **exact file paths** (e.g. `src/mcp_protocol.rs`) and, when possible, line numbers or a clear description of the location (e.g. "replace the existing `handle_request` function").
3. Never describe code changes only in prose. Show the full function or block as it should look **after** the change, or show both "before" and "after" versions.
4. Keep instructions **unmistakable and unambiguous**. A human or tool following your instructions should not need to guess what to do.
5. Assume you cannot see any files outside this bundle. If you must rely on unknown code, explicitly note assumptions and risks.

### Key Technical Details:

- **Port**: 43210 (configurable via `MCP_PORT` env var)
- **Auth**: Bearer token from `~/.sk/kit/agent-token`
- **Discovery**: Server info written to `~/.sk/kit/server.json`
- **Logs**: JSONL to `~/.sk/kit/logs/script-kit-gpui.jsonl`
- **Schema patterns**: Both `schema = {...}` and `defineSchema({...})` are supported
- **Tool naming**: `scripts/{slug}` where slug is derived from `metadata.name` or `// Name:` comment

When you answer, work directly with the code and instructions this bundle contains and return a clear, step-by-step plan plus exact code edits.

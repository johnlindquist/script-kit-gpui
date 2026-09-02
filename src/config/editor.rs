//! TypeScript config file editor
//!
//! Provides robust utilities for programmatically modifying TypeScript config files
//! like `~/.scriptkit/config.ts`.
//!
//! # Design
//!
//! Uses tree-sitter-typescript to parse the AST, giving exact byte offsets for
//! insertion points. This eliminates the fragility of hand-rolled brace counting.

// --- merged from part_01.rs ---
use std::path::Path;
use tree_sitter::Parser;

/// Result of a config edit operation
#[derive(Debug, Clone, PartialEq)]
pub enum EditResult {
    /// Successfully modified the config
    Modified(String),
    /// The property already exists with the desired value
    AlreadySet,
    /// Could not parse or modify the config
    Failed(String),
}

/// A property to add to the config
#[derive(Debug, Clone)]
pub struct ConfigProperty {
    /// Property name (e.g., "claudeCode")
    pub name: String,
    /// Property value as TypeScript code (e.g., "{\n    enabled: true\n  }")
    pub value: String,
}

impl ConfigProperty {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

fn find_top_level_pair<'a>(
    object: tree_sitter::Node<'a>,
    content: &str,
    property_name: &str,
) -> Option<tree_sitter::Node<'a>> {
    (0..object.named_child_count())
        .filter_map(|i| object.named_child(i))
        .find(|pair| {
            if pair.kind() != "pair" {
                return false;
            }
            pair.child_by_field_name("key")
                .map(|key| &content[key.start_byte()..key.end_byte()] == property_name)
                .unwrap_or(false)
        })
}

/// Error type for config write operations
#[derive(Debug)]
pub enum ConfigWriteError {
    /// The edited content failed TypeScript validation
    ValidationFailed(String),
    /// File system operation failed
    IoError(String),
    /// The editor could not modify the content
    EditFailed(String),
}

impl std::fmt::Display for ConfigWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::EditFailed(msg) => write!(f, "Edit failed: {}", msg),
        }
    }
}

/// Result of a successful config write
#[derive(Debug, PartialEq)]
pub enum WriteOutcome {
    /// File was modified and written
    Written,
    /// Property already existed, no write needed
    AlreadySet,
    /// File was created from scratch (was empty or missing)
    Created,
}

/// Add or update a property in a TypeScript config file
///
/// # Arguments
/// * `content` - The current file content
/// * `property` - The property to add/update
///
/// # Returns
/// * `EditResult::Modified(new_content)` - Successfully modified
/// * `EditResult::AlreadySet` - Property already exists with desired value
/// * `EditResult::Failed(reason)` - Could not modify
pub fn add_property(content: &str, property: &ConfigProperty) -> EditResult {
    let tree = match parse_typescript(content) {
        Ok(tree) => tree,
        Err(error) => return EditResult::Failed(error),
    };
    let root = tree.root_node();
    let export = match find_export_statement(root) {
        Some(export) => export,
        None => return EditResult::Failed("No export statement found".to_string()),
    };
    let object = match find_object_in_export(export, content) {
        Some(object) => object,
        None => {
            return EditResult::Failed("Could not find config object in export statement".into());
        }
    };

    if let Some(pair) = find_top_level_pair(object, content, &property.name) {
        let Some(value_node) = pair.child_by_field_name("value") else {
            return EditResult::Failed(format!(
                "Could not find existing value node for {}",
                property.name
            ));
        };
        let existing_value = content[value_node.start_byte()..value_node.end_byte()].trim();
        if existing_value == property.value.trim() {
            return EditResult::AlreadySet;
        }

        let mut new_content = String::with_capacity(
            content.len() - (value_node.end_byte() - value_node.start_byte())
                + property.value.len(),
        );
        new_content.push_str(&content[..value_node.start_byte()]);
        new_content.push_str(&property.value);
        new_content.push_str(&content[value_node.end_byte()..]);
        return EditResult::Modified(new_content);
    }

    match find_object_end(content) {
        Some(insert_info) => {
            let new_content = insert_property(content, &insert_info, property);
            EditResult::Modified(new_content)
        }
        None => EditResult::Failed("Could not find config object closing brace".to_string()),
    }
}

/// Check if the config contains a top-level property with the given name.
///
/// Uses tree-sitter AST to find `pair` nodes within the export default object,
/// correctly ignoring properties in comments, strings, or nested objects.
pub fn contains_property(content: &str, property_name: &str) -> bool {
    let tree = match parse_typescript(content) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let root = tree.root_node();

    let export = match find_export_statement(root) {
        Some(e) => e,
        None => return false,
    };

    let object = match find_object_in_export(export, content) {
        Some(o) => o,
        None => return false,
    };

    // Check only top-level pairs (direct children of the export object)
    for i in 0..object.named_child_count() {
        if let Some(pair) = object.named_child(i) {
            if pair.kind() == "pair" {
                if let Some(key) = pair.child_by_field_name("key") {
                    let key_text = &content[key.start_byte()..key.end_byte()];
                    if key_text == property_name {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Information about where to insert a new property
#[derive(Debug)]
struct InsertInfo {
    /// Position of the closing brace `}`
    close_brace_pos: usize,
    /// Whether the last property has a trailing comma
    has_trailing_comma: bool,
    /// The indentation to use for the new property
    indent: String,
}

// ==========================================================================
// Tree-sitter AST helpers
// ==========================================================================

/// Parse TypeScript content into a tree-sitter AST.
fn parse_typescript(content: &str) -> Result<tree_sitter::Tree, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .map_err(|e| format!("Failed to set TypeScript language: {}", e))?;
    parser
        .parse(content, None)
        .ok_or_else(|| "Failed to parse TypeScript content".to_string())
}

/// Find the `export_statement` node in the root of the AST.
fn find_export_statement(root: tree_sitter::Node) -> Option<tree_sitter::Node> {
    (0..root.named_child_count())
        .filter_map(|i| root.named_child(i))
        .find(|n| n.kind() == "export_statement")
}

/// Find the default export object node within an export_statement.
///
/// Handles three forms:
/// - `export default { ... } satisfies Config;` → satisfies_expression → object
/// - `export default { ... } as Config;` → as_expression → object
/// - `export default { ... };` → object directly
fn find_object_in_export<'a>(
    export_node: tree_sitter::Node<'a>,
    _content: &str,
) -> Option<tree_sitter::Node<'a>> {
    for i in 0..export_node.named_child_count() {
        if let Some(child) = export_node.named_child(i) {
            match child.kind() {
                "satisfies_expression" | "as_expression" => {
                    // The object is the first child of kind "object"
                    for j in 0..child.named_child_count() {
                        if let Some(grandchild) = child.named_child(j) {
                            if grandchild.kind() == "object" {
                                return Some(grandchild);
                            }
                        }
                    }
                }
                "object" => return Some(child),
                _ => {}
            }
        }
    }
    None
}

/// Walk AST to collect ERROR node descriptions for diagnostics.
fn collect_ast_errors(node: tree_sitter::Node, content: &str, out: &mut String) {
    if node.is_error() || node.is_missing() {
        let start = node.start_position();
        let end_byte = node.end_byte().min(node.start_byte() + 30);
        let text = &content[node.start_byte()..end_byte];
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(&format!(
            "line {}:{} near '{}'",
            start.row + 1,
            start.column,
            text.replace('\n', "\\n")
        ));
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_ast_errors(child, content, out);
        }
    }
}

/// Find the export default object and return insertion info using tree-sitter AST.
///
/// Returns byte offsets suitable for direct `&str` slicing.
fn find_object_end(content: &str) -> Option<InsertInfo> {
    let tree = parse_typescript(content).ok()?;
    let root = tree.root_node();

    let export = find_export_statement(root)?;
    let object = find_object_in_export(export, content)?;

    // The object node spans { ... }. end_byte() is exclusive (byte after }).
    let close_brace_byte = object.end_byte() - 1;

    // Verify it's actually a closing brace
    if content.as_bytes().get(close_brace_byte) != Some(&b'}') {
        return None;
    }

    // Collect pair children (actual properties, not comments)
    let pairs: Vec<_> = (0..object.named_child_count())
        .filter_map(|i| object.named_child(i))
        .filter(|n| n.kind() == "pair")
        .collect();

    // Check for trailing comma between last pair and closing brace
    let has_trailing_comma = if let Some(last_pair) = pairs.last() {
        let between = &content[last_pair.end_byte()..close_brace_byte];
        between.contains(',')
    } else {
        false
    };

    // Detect indent from first pair's column position
    let indent = if let Some(first_pair) = pairs.first() {
        " ".repeat(first_pair.start_position().column)
    } else {
        "  ".to_string()
    };

    Some(InsertInfo {
        close_brace_pos: close_brace_byte,
        has_trailing_comma,
        indent,
    })
}

/// Insert a property into the config content
fn insert_property(content: &str, info: &InsertInfo, property: &ConfigProperty) -> String {
    let before = &content[..info.close_brace_pos];
    let after = &content[info.close_brace_pos..];

    // Build the property string with trailing comma for safety
    let comma_prefix = if info.has_trailing_comma { "" } else { "," };
    let property_str = format!(
        "{}\n\n{}{}: {},",
        comma_prefix, info.indent, property.name, property.value
    );

    // Find where to insert (right before the closing brace)
    // We want to maintain nice formatting, so add a newline if needed
    let needs_newline_before = !before.ends_with('\n');
    let newline_before = if needs_newline_before { "\n" } else { "" };

    // Build the result
    let result = format!("{}{}{}\n{}", before, newline_before, property_str, after);

    // VALIDATION: Check for corruption patterns
    // If we detect }{, it means something went wrong with brace matching
    if result.contains("}{") {
        tracing::error!(
            "Config editor detected potential corruption: }}{{ pattern found. \
             close_brace_pos={}, content_len={}, before_len={}, after_starts_with={:?}",
            info.close_brace_pos,
            content.len(),
            before.len(),
            after.chars().take(10).collect::<String>()
        );
    }

    result
}

/// Enable Claude Code in a config file
///
/// This is a convenience function that adds `claudeCode: { enabled: true }` to the config.
/// Used in tests; production code uses `enable_claude_code_safely`.
#[allow(dead_code)]
pub fn enable_claude_code(content: &str) -> EditResult {
    // Use inline format for cleaner insertion
    // The trailing comma is added by insert_property
    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    add_property(content, &property)
}

// --- merged from part_02.rs ---
use std::io::Write;

/// Validate TypeScript content by attempting compilation with bun.
///
/// Writes content to a temp .ts file and runs `bun build` on it.
/// Returns Ok(()) if valid, Err with details if invalid.
pub fn validate_typescript(content: &str, bun_path: Option<&str>) -> Result<(), String> {
    let bun = bun_path.unwrap_or("bun");

    let tmp = tempfile::Builder::new()
        .suffix(".ts")
        .tempfile()
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    std::fs::write(tmp.path(), content).map_err(|e| format!("Failed to write temp file: {}", e))?;

    let output = std::process::Command::new(bun)
        .arg("build")
        .arg("--target=bun")
        .arg("--no-bundle")
        .arg(tmp.path())
        .arg("--outfile=/dev/null")
        .output()
        .map_err(|e| format!("Failed to run bun: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("TypeScript compilation failed: {}", stderr.trim()))
    }
}

/// Structural validation using tree-sitter AST.
///
/// Parses the content with tree-sitter-typescript and checks that:
/// 1. Content contains `export default`
/// 2. The AST has no ERROR nodes (no syntax errors)
/// 3. The export default object can be found
/// 4. Content ends with `satisfies Config;` or `as Config;`
pub fn validate_structure(content: &str) -> Result<(), String> {
    if !content.contains("export default") {
        return Err("Missing 'export default' declaration".into());
    }

    let tree =
        parse_typescript(content).map_err(|e| format!("Failed to parse TypeScript: {}", e))?;
    let root = tree.root_node();

    // Check for parse errors (syntax errors, corruption, etc.)
    if root.has_error() {
        let mut error_msg = String::new();
        collect_ast_errors(root, content, &mut error_msg);
        if !error_msg.is_empty() {
            return Err(format!("TypeScript parse errors: {}", error_msg));
        }
        return Err("TypeScript parse errors detected".into());
    }

    // Find export_statement and its object
    let export = find_export_statement(root).ok_or("No export statement found in AST")?;

    find_object_in_export(export, content)
        .ok_or_else(|| "Could not find config object in export statement".to_string())?;

    let trimmed = content.trim();
    if !trimmed.contains("satisfies Config;") && !trimmed.contains("as Config;") {
        return Err("Missing 'satisfies Config' or 'as Config' type assertion".into());
    }

    Ok(())
}

/// Generate a fresh config.ts with the given property included.
fn generate_fresh_config(property: &ConfigProperty) -> String {
    format!(
        r#"import type {{ Config }} from "@scriptkit/sdk";

export default {{
  hotkey: {{ modifiers: ["meta"], key: "Semicolon" }},
  {}: {},
}} satisfies Config;
"#,
        property.name, property.value
    )
}

/// Prepare a preference mutation without executing config.ts or touching disk.
/// The caller may commit this together with another file in a recoverable
/// transaction. Ordinary config writes retain their existing validation path.
pub(crate) fn prepare_config_property(
    content: &str,
    property: &ConfigProperty,
) -> Result<String, ConfigWriteError> {
    let prepared = if content.trim().is_empty() {
        generate_fresh_config(property)
    } else {
        match add_property(content, property) {
            EditResult::Modified(prepared) => prepared,
            EditResult::AlreadySet => content.to_owned(),
            EditResult::Failed(reason) => return Err(ConfigWriteError::EditFailed(reason)),
        }
    };
    validate_structure(&prepared).map_err(ConfigWriteError::ValidationFailed)?;
    Ok(prepared)
}

/// Decode a fixture config without evaluating JavaScript, imports or getters.
/// Only a literal default-export object and inert import/type syntax is accepted.
pub(crate) fn parse_static_config(content: &str) -> Result<serde_json::Value, String> {
    if content.len() > 1_048_576 {
        return Err("static config exceeds size limit".into());
    }
    let tree = parse_typescript(content)?;
    let root = tree.root_node();
    if root.has_error() {
        return Err("static config contains syntax errors".into());
    }
    let mut exports = 0;
    for index in 0..root.named_child_count() {
        let child = root.named_child(index).ok_or("missing syntax node")?;
        match child.kind() {
            "import_statement" | "comment" | "type_alias_declaration" | "interface_declaration" => {
            }
            "export_statement" => exports += 1,
            _ => return Err(format!("nonliteral config statement: {}", child.kind())),
        }
    }
    if exports != 1 {
        return Err("static config requires one default export".into());
    }
    let export = find_export_statement(root).ok_or("missing default export")?;
    if !(0..export.child_count())
        .filter_map(|i| export.child(i))
        .any(|node| node.kind() == "default")
    {
        return Err("static config requires a default export".into());
    }
    let object =
        find_object_in_export(export, content).ok_or("config export must be a literal object")?;
    decode_static_config_value(object, content, 0)
}

fn decode_static_config_string(text: &str) -> Result<String, String> {
    if text.starts_with('"') {
        return serde_json::from_str(text)
            .map_err(|error| format!("invalid static string: {error}"));
    }
    if !text.starts_with('\'') || !text.ends_with('\'') {
        return Err("unsupported static string".into());
    }
    let mut json = String::with_capacity(text.len() + 2);
    json.push('"');
    let mut chars = text[1..text.len() - 1].chars();
    while let Some(character) = chars.next() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => match chars.next().ok_or("unterminated string escape")? {
                '\'' => json.push('\''),
                escaped => {
                    json.push('\\');
                    json.push(escaped);
                }
            },
            other => json.push(other),
        }
    }
    json.push('"');
    serde_json::from_str(&json).map_err(|error| format!("invalid static string: {error}"))
}

fn decode_static_config_value(
    node: tree_sitter::Node,
    content: &str,
    depth: usize,
) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    if depth > 64 {
        return Err("static config exceeds nesting limit".into());
    }
    let text = node
        .utf8_text(content.as_bytes())
        .map_err(|error| error.to_string())?;
    match node.kind() {
        "object" => {
            let mut object = serde_json::Map::new();
            let mut keys = std::collections::BTreeSet::new();
            for index in 0..node.named_child_count() {
                let pair = node.named_child(index).ok_or("missing object member")?;
                if pair.kind() == "comment" {
                    continue;
                }
                if pair.kind() != "pair" {
                    return Err(format!("nonliteral object member: {}", pair.kind()));
                }
                let key = pair
                    .child_by_field_name("key")
                    .ok_or("missing object key")?;
                let key_text = key
                    .utf8_text(content.as_bytes())
                    .map_err(|error| error.to_string())?;
                let key = match key.kind() {
                    "property_identifier" => key_text.to_string(),
                    "string" => decode_static_config_string(key_text)?,
                    _ => return Err("nonliteral object key".into()),
                };
                let value = pair
                    .child_by_field_name("value")
                    .ok_or("missing object value")?;
                if !keys.insert(key.clone()) {
                    return Err("duplicate static config property".into());
                }
                if matches!(value.kind(), "identifier" | "undefined")
                    && value
                        .utf8_text(content.as_bytes())
                        .map_err(|error| error.to_string())?
                        == "undefined"
                {
                    continue; // Same object omission semantics as JSON.stringify.
                }
                let value = decode_static_config_value(value, content, depth + 1)?;
                object.insert(key, value);
            }
            Ok(Value::Object(object))
        }
        "array" => {
            let mut values = Vec::new();
            let mut needs_value = true;
            for index in 0..node.child_count() {
                let child = node.child(index).ok_or("missing array member")?;
                match child.kind() {
                    "[" | "]" | "comment" => {}
                    "," if needs_value => {
                        return Err("array holes are not static JSON values".into())
                    }
                    "," => needs_value = true,
                    _ => {
                        values.push(decode_static_config_value(child, content, depth + 1)?);
                        needs_value = false;
                    }
                }
            }
            Ok(Value::Array(values))
        }
        "string" => decode_static_config_string(text).map(Value::String),
        "number" | "true" | "false" | "null" => {
            serde_json::from_str(text).map_err(|error| error.to_string())
        }
        "unary_expression" => {
            let argument = node
                .child_by_field_name("argument")
                .ok_or("missing unary argument")?;
            let operator = node
                .child_by_field_name("operator")
                .ok_or("missing unary operator")?;
            if argument.kind() != "number"
                || operator
                    .utf8_text(content.as_bytes())
                    .map_err(|error| error.to_string())?
                    != "-"
            {
                return Err("only negative numeric literals are allowed".into());
            }
            let number = argument
                .utf8_text(content.as_bytes())
                .map_err(|error| error.to_string())?;
            serde_json::from_str(&format!("-{number}")).map_err(|error| error.to_string())
        }
        other => Err(format!("nonliteral static config value: {other}")),
    }
}

fn atomic_write_with_secure_tempfile(
    config_path: &Path,
    content: &str,
) -> Result<(), ConfigWriteError> {
    let parent = config_path.parent().ok_or_else(|| {
        ConfigWriteError::IoError(format!(
            "config_atomic_write_missing_parent: target={}",
            config_path.display()
        ))
    })?;

    let mut temp_file = tempfile::Builder::new()
        .prefix(".config-write.")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|e| {
            ConfigWriteError::IoError(format!(
                "config_atomic_write_tempfile_create_failed: target={} error={}",
                config_path.display(),
                e
            ))
        })?;

    temp_file.write_all(content.as_bytes()).map_err(|e| {
        ConfigWriteError::IoError(format!(
            "config_atomic_write_tempfile_write_failed: target={} error={}",
            config_path.display(),
            e
        ))
    })?;

    temp_file.flush().map_err(|e| {
        ConfigWriteError::IoError(format!(
            "config_atomic_write_tempfile_flush_failed: target={} error={}",
            config_path.display(),
            e
        ))
    })?;

    temp_file.persist(config_path).map_err(|e| {
        ConfigWriteError::IoError(format!(
            "config_atomic_write_rename_failed: from={} to={} error={}",
            e.file.path().display(),
            config_path.display(),
            e.error
        ))
    })?;

    Ok(())
}

/// Safely modify config.ts: edit in memory, validate, backup, atomic-write.
///
/// This is the single entry point for all config file modifications.
/// Guarantees:
/// 1. Output is valid TypeScript (validated by bun, fallback to structural)
/// 2. A backup exists before overwriting
/// 3. The write is atomic (temp file + rename)
pub fn write_config_safely(
    config_path: &Path,
    property: &ConfigProperty,
    bun_path: Option<&str>,
) -> Result<WriteOutcome, ConfigWriteError> {
    // Step 1: Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ConfigWriteError::IoError(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }
    }

    // Step 2: Read existing content
    let content = std::fs::read_to_string(config_path).unwrap_or_default();

    // Step 3: Produce new content
    let (new_content, was_empty) = if content.is_empty() {
        (generate_fresh_config(property), true)
    } else {
        match add_property(&content, property) {
            EditResult::Modified(new_content) => (new_content, false),
            EditResult::AlreadySet => return Ok(WriteOutcome::AlreadySet),
            EditResult::Failed(reason) => {
                return Err(ConfigWriteError::EditFailed(reason));
            }
        }
    };

    // Step 4: Validate the new content
    let validation_result = validate_typescript(&new_content, bun_path);
    match &validation_result {
        Ok(()) => { /* bun says it's valid */ }
        Err(bun_err) => {
            if bun_err.starts_with("Failed to run bun") {
                // bun unavailable - fall back to structural validation
                tracing::warn!("bun unavailable for validation, using structural check");
                validate_structure(&new_content).map_err(ConfigWriteError::ValidationFailed)?;
            } else {
                // bun ran but TypeScript is invalid - do NOT write
                return Err(ConfigWriteError::ValidationFailed(bun_err.clone()));
            }
        }
    }

    // Step 5: Backup existing file (if non-empty)
    if !content.is_empty() {
        let backup_path = config_path.with_extension("ts.bak");
        if let Err(e) = std::fs::copy(config_path, &backup_path) {
            tracing::warn!(
                error = %e,
                path = %backup_path.display(),
                "Failed to create config backup"
            );
        } else {
            tracing::info!(path = %backup_path.display(), "Config backup saved");
        }
    }

    // Step 6: Atomic write (randomized tempfile in same directory + rename)
    atomic_write_with_secure_tempfile(config_path, &new_content)?;

    if was_empty {
        Ok(WriteOutcome::Created)
    } else {
        Ok(WriteOutcome::Written)
    }
}

/// Enable Claude Code in config.ts using the safe write path.
pub fn enable_claude_code_safely(
    config_path: &Path,
    bun_path: Option<&str>,
) -> Result<WriteOutcome, ConfigWriteError> {
    let property = ConfigProperty::new("claudeCode", "{ enabled: true }");
    write_config_safely(config_path, &property, bun_path)
}

pub fn write_json_property_safely<T: serde::Serialize>(
    config_path: &Path,
    property_name: &str,
    value: &T,
    bun_path: Option<&str>,
) -> Result<WriteOutcome, ConfigWriteError> {
    let property = ConfigProperty::new(
        property_name,
        serde_json::to_string(value).map_err(|error| {
            ConfigWriteError::EditFailed(format!(
                "Failed to serialize property {}: {}",
                property_name, error
            ))
        })?,
    );
    write_config_safely(config_path, &property, bun_path)
}

/// Attempt to recover config.ts from its backup.
///
/// Returns Ok(true) if recovery succeeded, Ok(false) if no backup exists.
pub fn recover_from_backup(
    config_path: &Path,
    bun_path: Option<&str>,
) -> Result<bool, ConfigWriteError> {
    let backup_path = config_path.with_extension("ts.bak");

    if !backup_path.exists() {
        return Ok(false);
    }

    let backup_content = std::fs::read_to_string(&backup_path)
        .map_err(|e| ConfigWriteError::IoError(format!("Failed to read backup: {}", e)))?;

    // Validate backup before restoring
    let validation_result = validate_typescript(&backup_content, bun_path);
    match &validation_result {
        Ok(()) => {}
        Err(bun_err) => {
            if bun_err.starts_with("Failed to run bun") {
                validate_structure(&backup_content).map_err(|e| {
                    ConfigWriteError::ValidationFailed(format!("Backup is also invalid: {}", e))
                })?;
            } else {
                return Err(ConfigWriteError::ValidationFailed(format!(
                    "Backup is also invalid: {}",
                    bun_err
                )));
            }
        }
    }

    // Atomic write of backup content
    atomic_write_with_secure_tempfile(config_path, &backup_content)?;

    tracing::info!(
        path = %config_path.display(),
        backup = %backup_path.display(),
        "Config restored from backup"
    );

    Ok(true)
}

#[cfg(test)]
#[path = "editor/tests.rs"]
mod tests;

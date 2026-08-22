//! Scriptlet codefence metadata parser
//!
//! Parses `\`\`\`metadata` and `\`\`\`schema` codefence blocks from markdown scriptlets.
//! These blocks provide an alternative to the HTML comment metadata format, using
//! JSON directly in labeled code fences.
//!
//! # Example scriptlet with codefences:
//! ````markdown
//! # Quick Todo
//!
//! ```metadata
//! { "name": "Quick Todo", "description": "Add a todo item" }
//! ```
//!
//! ```schema
//! { "input": { "item": { "type": "string", "required": true } } }
//! ```
//!
//! ```ts
//! const { item } = await input();
//! await addTodo(item);
//! ```
//! ````

// --- merged from part_000.rs ---
use crate::metadata_parser::TypedMetadata;
use crate::schema_parser::Schema;
use tracing::debug;
/// Result of parsing codefence metadata from a scriptlet
#[derive(Debug, Clone, Default)]
pub struct CodefenceParseResult {
    /// Parsed metadata from ```metadata block
    pub metadata: Option<TypedMetadata>,
    /// Parsed schema from ```schema block
    pub schema: Option<Schema>,
    /// The code content from the main code block (e.g., ```ts)
    pub code: Option<CodeBlock>,
    /// Parse errors encountered
    pub errors: Vec<String>,
}
/// A code block with its language and content
#[derive(Debug, Clone)]
pub struct CodeBlock {
    /// The language identifier (e.g., "ts", "bash", "python")
    pub language: String,
    /// The code content
    pub content: String,
}
/// Parse codefence blocks from markdown scriptlet content
///
/// Looks for:
/// - `\`\`\`metadata\n{...}\n\`\`\`` - JSON metadata block
/// - `\`\`\`schema\n{...}\n\`\`\`` - JSON schema block  
/// - `\`\`\`<lang>\n...\n\`\`\`` - Main code block
///
/// # Arguments
/// * `content` - The markdown content to parse
///
/// # Returns
/// `CodefenceParseResult` with parsed metadata, schema, code, and any errors
pub fn parse_codefence_metadata(content: &str) -> CodefenceParseResult {
    let mut result = CodefenceParseResult::default();

    let blocks = extract_all_codefence_blocks(content);

    for (language, block_content) in blocks {
        match language.as_str() {
            "metadata" => {
                // Try JSON first, then fall back to simple key: value format
                if let Ok(metadata) = serde_json::from_str::<TypedMetadata>(&block_content) {
                    debug!(
                        name = ?metadata.name,
                        description = ?metadata.description,
                        "Parsed codefence metadata (JSON)"
                    );
                    result.metadata = Some(metadata);
                } else if let Some(metadata) = parse_simple_metadata(&block_content) {
                    debug!(
                        keyword = ?metadata.keyword,
                        "Parsed codefence metadata (simple format)"
                    );
                    result.metadata = Some(metadata);
                } else {
                    result.errors.push(
                        "Failed to parse metadata: not valid JSON or simple key: value format"
                            .to_string(),
                    );
                }
            }
            "schema" => match serde_json::from_str::<Schema>(&block_content) {
                Ok(schema) => {
                    debug!(
                        input_fields = schema.input.len(),
                        output_fields = schema.output.len(),
                        "Parsed codefence schema"
                    );
                    result.schema = Some(schema);
                }
                Err(e) => {
                    result
                        .errors
                        .push(format!("Failed to parse schema JSON: {}", e));
                }
            },
            // Skip empty language specifier
            "" => {}
            // Any other language is treated as code
            lang => {
                // Only capture the first non-metadata/schema code block
                if result.code.is_none() {
                    result.code = Some(CodeBlock {
                        language: lang.to_string(),
                        content: block_content,
                    });
                }
            }
        }
    }

    result
}
/// Extract all codefence blocks from content
/// Returns Vec of (language, content) tuples
fn extract_all_codefence_blocks(content: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut i = 0;
    let mut in_html_comment = false;
    while i < lines.len() {
        let line = lines[i];
        let visible_line = strip_html_comment_segments(line, &mut in_html_comment);
        let trimmed = visible_line.trim_start();

        // Check for opening fence (``` or ~~~)
        if let Some((fence_char, fence_count, language)) = detect_fence_opening(trimmed) {
            let mut block_lines = Vec::new();
            i += 1;

            // Collect content until closing fence
            while i < lines.len() {
                let current = lines[i].trim_start();
                if is_closing_fence(current, fence_char, fence_count) {
                    break;
                }
                block_lines.push(lines[i]);
                i += 1;
            }

            let block_content = block_lines.join("\n");
            blocks.push((language, block_content.trim().to_string()));
        }

        i += 1;
    }

    blocks
}
/// Return only the text outside HTML comments on this line.
///
/// This is intentionally used only while looking for top-level fence openers.
/// Once a real fence is open, its content is preserved verbatim until the
/// closing fence.
fn strip_html_comment_segments(line: &str, in_html_comment: &mut bool) -> String {
    let mut visible = String::new();
    let mut rest = line;

    loop {
        if *in_html_comment {
            if let Some(end) = rest.find("-->") {
                rest = &rest[end + 3..];
                *in_html_comment = false;
            } else {
                break;
            }
        } else if let Some(start) = rest.find("<!--") {
            visible.push_str(&rest[..start]);
            rest = &rest[start + 4..];
            *in_html_comment = true;
        } else {
            visible.push_str(rest);
            break;
        }
    }

    visible
}
/// Detect opening fence, returns (fence_char, count, language)
fn detect_fence_opening(line: &str) -> Option<(char, usize, String)> {
    // Try backticks
    let backtick_count = line.chars().take_while(|&c| c == '`').count();
    if backtick_count >= 3 {
        let rest = &line[backtick_count..];
        let language = rest.split_whitespace().next().unwrap_or("").to_string();
        return Some(('`', backtick_count, language));
    }

    // Try tildes
    let tilde_count = line.chars().take_while(|&c| c == '~').count();
    if tilde_count >= 3 {
        let rest = &line[tilde_count..];
        let language = rest.split_whitespace().next().unwrap_or("").to_string();
        return Some(('~', tilde_count, language));
    }

    None
}
/// Check if line is a closing fence
fn is_closing_fence(line: &str, fence_char: char, min_count: usize) -> bool {
    let count = line.chars().take_while(|&c| c == fence_char).count();
    if count < min_count {
        return false;
    }

    // Rest of line should be empty or whitespace
    let rest = &line[count..];
    rest.chars().all(|c| c.is_whitespace())
}
/// Parse simple key: value metadata format
///
/// Supports lines like:
/// ```text
/// keyword: !testing
/// name: My Script
/// description: Does something useful
/// ```
///
/// The value is everything after `: ` (colon-space).
/// Lines starting with `//` are treated as comments and ignored.
/// Empty lines are ignored.
///
/// Special handling:
/// - `keyword`, `expand`, `snippet` all map to the `keyword` field
fn parse_simple_metadata(content: &str) -> Option<TypedMetadata> {
    use std::collections::HashMap;

    let mut fields: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // Look for `key: value` pattern (colon followed by space)
        if let Some(colon_pos) = line.find(": ") {
            let key = line[..colon_pos].trim().to_lowercase();
            let value = line[colon_pos + 2..].trim().to_string();

            if !value.is_empty() {
                fields.insert(key, value);
            }
        }
    }

    // If no fields were parsed, return None
    if fields.is_empty() {
        return None;
    }

    // Build TypedMetadata from parsed fields
    let mut metadata = TypedMetadata::default();

    for (key, value) in fields {
        match key.as_str() {
            "name" => metadata.name = Some(value),
            "description" => metadata.description = Some(value),
            "author" => metadata.author = Some(value),
            "enter" => metadata.enter = Some(value),
            "alias" => metadata.alias = Some(value),
            "keyword" | "expand" | "snippet" => metadata.keyword = Some(value),
            "icon" => metadata.icon = Some(value),
            "shortcut" => metadata.shortcut = Some(value),
            "placeholder" => metadata.placeholder = Some(value),
            "cron" => metadata.cron = Some(value),
            "schedule" => metadata.schedule = Some(value),
            "hidden" => metadata.hidden = value.to_lowercase() == "true" || value == "1",
            "background" => metadata.background = value.to_lowercase() == "true" || value == "1",
            "system" => metadata.system = value.to_lowercase() == "true" || value == "1",
            "fallback" => metadata.fallback = value.to_lowercase() == "true" || value == "1",
            "fallback_label" => metadata.fallback_label = Some(value),
            "sdkcapabilities" => {
                let parsed = serde_json::from_str(&value)
                    .unwrap_or_else(|_| serde_json::Value::String(value));
                metadata.extra.insert("sdkCapabilities".to_string(), parsed);
            }
            "executiontopology" => {
                let parsed = serde_json::from_str(&value)
                    .unwrap_or_else(|_| serde_json::Value::String(value));
                metadata
                    .extra
                    .insert("executionTopology".to_string(), parsed);
            }
            // Unknown fields go to extra
            _ => {
                metadata.extra.insert(key, serde_json::Value::String(value));
            }
        }
    }

    Some(metadata)
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    use crate::schema_parser::FieldType;
    // ========================================
    // Core Test Cases (from requirements)
    // ========================================

    #[test]
    fn test_parse_metadata_codefence() {
        let content = r#"
# Quick Todo

```metadata
{ "name": "Quick Todo", "description": "Add a todo item" }
```

```ts
const item = await arg("Todo item");
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert!(result.metadata.is_some());

        let metadata = result.metadata.unwrap();
        assert_eq!(metadata.name, Some("Quick Todo".to_string()));
        assert_eq!(metadata.description, Some("Add a todo item".to_string()));
    }
    #[test]
    fn test_parse_schema_codefence() {
        let content = r#"
```schema
{
    "input": {
        "item": { "type": "string", "required": true }
    }
}
```

```ts
const { item } = await input();
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert!(result.schema.is_some());

        let schema = result.schema.unwrap();
        assert_eq!(schema.input.len(), 1);
        assert!(schema.input.contains_key("item"));

        let item_field = schema.input.get("item").unwrap();
        assert_eq!(item_field.field_type, FieldType::String);
        assert!(item_field.required);
    }
    #[test]
    fn test_parse_both_metadata_and_schema() {
        let content = r#"
```metadata
{ "name": "Quick Todo", "description": "Add a todo item", "icon": "CheckSquare" }
```

```schema
{
    "input": {
        "item": { "type": "string", "required": true, "description": "The todo item text" }
    },
    "output": {
        "id": { "type": "string" }
    }
}
```

```ts
const { item } = await input();
const id = await addTodo(item);
output({ id });
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);

        // Check metadata
        assert!(result.metadata.is_some());
        let metadata = result.metadata.unwrap();
        assert_eq!(metadata.name, Some("Quick Todo".to_string()));
        assert_eq!(metadata.description, Some("Add a todo item".to_string()));
        assert_eq!(metadata.icon, Some("CheckSquare".to_string()));

        // Check schema
        assert!(result.schema.is_some());
        let schema = result.schema.unwrap();
        assert_eq!(schema.input.len(), 1);
        assert_eq!(schema.output.len(), 1);

        let item_field = schema.input.get("item").unwrap();
        assert_eq!(
            item_field.description,
            Some("The todo item text".to_string())
        );

        assert!(schema.output.contains_key("id"));
    }
    #[test]
    fn test_no_codefence_returns_none() {
        let content = r#"
# Just a Regular Markdown

Some text here with no code fences at all.

- List item 1
- List item 2
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_none());
        assert!(result.schema.is_none());
        assert!(result.code.is_none());
        assert!(result.errors.is_empty());
    }
    #[test]
    fn test_malformed_content_returns_error() {
        // Content that's neither valid JSON nor valid simple key: value format
        let content = r#"
```metadata
this is just random text with no valid format
no key value pairs here either
```

```ts
console.log("test");
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_none());
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("not valid JSON or simple key: value format"));
    }
    #[test]
    fn test_simple_format_keyword() {
        // Simple key: value format (not JSON)
        let content = r#"
```metadata
keyword: !testing
```

```paste
success!
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert!(result.errors.is_empty());
        let metadata = result.metadata.unwrap();
        assert_eq!(metadata.keyword, Some("!testing".to_string()));
    }
    #[test]
    fn test_simple_format_multiple_fields() {
        let content = r#"
```metadata
name: My Script
keyword: :sig
description: A helpful signature expander
shortcut: cmd shift s
```

```paste
Best regards,
John
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert!(result.errors.is_empty());
        let metadata = result.metadata.unwrap();
        assert_eq!(metadata.name, Some("My Script".to_string()));
        assert_eq!(metadata.keyword, Some(":sig".to_string()));
        assert_eq!(
            metadata.description,
            Some("A helpful signature expander".to_string())
        );
        assert_eq!(metadata.shortcut, Some("cmd shift s".to_string()));
    }
    #[test]
    fn test_simple_format_expand_alias() {
        // "expand" and "snippet" should also work as aliases for "keyword"
        let content1 = "```metadata\nexpand: !test\n```";
        let result1 = parse_codefence_metadata(content1);
        assert_eq!(result1.metadata.unwrap().keyword, Some("!test".to_string()));

        let content2 = "```metadata\nsnippet: :sig\n```";
        let result2 = parse_codefence_metadata(content2);
        assert_eq!(result2.metadata.unwrap().keyword, Some(":sig".to_string()));
    }
    #[test]
    fn test_simple_format_with_comments() {
        let content = r#"
```metadata
// This is a comment
keyword: !testing
// Another comment
name: Test Script
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        let metadata = result.metadata.unwrap();
        assert_eq!(metadata.keyword, Some("!testing".to_string()));
        assert_eq!(metadata.name, Some("Test Script".to_string()));
    }
    #[test]
    fn test_codefences_inside_html_comments_are_ignored() {
        let content = r#"
```metadata
keyword: active,,
```

```paste
active paste
```

<!--
```metadata
keyword: commented,,
```

```paste
commented paste
```
-->
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert_eq!(
            result.metadata.unwrap().keyword,
            Some("active,,".to_string())
        );
        let code = result.code.unwrap();
        assert_eq!(code.language, "paste");
        assert_eq!(code.content, "active paste");
    }
    #[test]
    fn test_comment_markers_inside_active_codefence_do_not_hide_later_metadata() {
        let content = r#"
```paste
literal <!-- comment marker --> in paste body
```

```metadata
keyword: active,,
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert_eq!(
            result.metadata.unwrap().keyword,
            Some("active,,".to_string())
        );
        let code = result.code.unwrap();
        assert_eq!(code.language, "paste");
        assert_eq!(
            code.content,
            "literal <!-- comment marker --> in paste body"
        );
    }
    #[test]
    fn test_code_block_extracted_correctly() {
        let content = r#"
```metadata
{ "name": "Test Script" }
```

```ts
const result = await arg("Pick one");
console.log(result);
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.code.is_some());
        let code = result.code.unwrap();
        assert_eq!(code.language, "ts");
        assert!(code.content.contains("const result = await arg"));
        assert!(code.content.contains("console.log(result)"));
    }
    // ========================================
    // Additional Test Cases
    // ========================================

    #[test]
    fn test_multiple_code_blocks_first_wins() {
        let content = r#"
```ts
first code block
```

```ts
second code block
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.code.is_some());
        let code = result.code.unwrap();
        assert_eq!(code.content, "first code block");
    }
    #[test]
    fn test_tilde_fences_supported() {
        let content = r#"
~~~metadata
{ "name": "Tilde Test" }
~~~

~~~ts
console.log("tilde fences");
~~~
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert_eq!(
            result.metadata.unwrap().name,
            Some("Tilde Test".to_string())
        );

        assert!(result.code.is_some());
        assert_eq!(result.code.unwrap().language, "ts");
    }
    #[test]
    fn test_mixed_fence_types() {
        let content = r#"
```metadata
{ "name": "Mixed Fences" }
```

~~~ts
const x = 1;
~~~
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert!(result.code.is_some());
        assert_eq!(result.code.unwrap().language, "ts");
    }
    #[test]
    fn test_metadata_with_all_fields() {
        let content = r#"
```metadata
{
    "name": "Full Script",
    "description": "A complete script",
    "author": "Test Author",
    "enter": "Execute",
    "alias": "fs",
    "icon": "Star",
    "shortcut": "cmd shift f",
    "tags": ["productivity", "utility"],
    "hidden": true,
    "placeholder": "Type something...",
    "cron": "0 9 * * *",
    "watch": ["*.ts", "*.js"],
    "background": true,
    "system": false
}
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert!(result.metadata.is_some());

        let meta = result.metadata.unwrap();
        assert_eq!(meta.name, Some("Full Script".to_string()));
        assert_eq!(meta.description, Some("A complete script".to_string()));
        assert_eq!(meta.author, Some("Test Author".to_string()));
        assert_eq!(meta.enter, Some("Execute".to_string()));
        assert_eq!(meta.alias, Some("fs".to_string()));
        assert_eq!(meta.icon, Some("Star".to_string()));
        assert_eq!(meta.shortcut, Some("cmd shift f".to_string()));
        assert_eq!(meta.tags, vec!["productivity", "utility"]);
        assert!(meta.hidden);
        assert_eq!(meta.placeholder, Some("Type something...".to_string()));
        assert_eq!(meta.cron, Some("0 9 * * *".to_string()));
        assert_eq!(meta.watch, vec!["*.ts", "*.js"]);
        assert!(meta.background);
        assert!(!meta.system);
    }
    #[test]
    fn test_schema_with_all_field_types() {
        let content = r#"
```schema
{
    "input": {
        "name": { "type": "string" },
        "count": { "type": "number" },
        "enabled": { "type": "boolean" },
        "items": { "type": "array", "items": "string" },
        "config": { "type": "object" },
        "anything": { "type": "any" }
    }
}
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.errors.is_empty(), "Errors: {:?}", result.errors);
        assert!(result.schema.is_some());

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
    fn test_malformed_schema_json_returns_error() {
        let content = r#"
```schema
{ "input": { not valid json } }
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.schema.is_none());
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("Failed to parse schema JSON"));
    }
    #[test]
    fn test_different_code_languages() {
        let languages = vec!["bash", "python", "ruby", "js", "kit", "template"];

        for lang in languages {
            let content = format!(
                r#"
```{}
code content
```
"#,
                lang
            );
            let result = parse_codefence_metadata(&content);

            assert!(result.code.is_some(), "Failed for language: {}", lang);
            assert_eq!(result.code.unwrap().language, lang);
        }
    }

    // --- merged from part_001.rs ---
    #[test]
    fn test_empty_codefence_blocks() {
        let content = r#"
```metadata
```

```ts
```
"#;
        let result = parse_codefence_metadata(content);

        // Empty metadata should fail to parse as JSON
        assert!(!result.errors.is_empty() || result.metadata.is_none());

        // Empty code block should still be captured
        assert!(result.code.is_some());
        assert_eq!(result.code.unwrap().content, "");
    }
    #[test]
    fn test_whitespace_handling() {
        let content = r#"
```metadata
  { "name": "Whitespace Test" }  
```

```ts
  const x = 1;  
  const y = 2;  
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert_eq!(
            result.metadata.unwrap().name,
            Some("Whitespace Test".to_string())
        );

        assert!(result.code.is_some());
        let code = result.code.unwrap();
        // Content should preserve internal whitespace but trim outer
        assert!(code.content.contains("const x = 1"));
        assert!(code.content.contains("const y = 2"));
    }
    #[test]
    fn test_nested_code_in_markdown() {
        // Simulate a scriptlet that contains a markdown code example
        let content = r#"
```metadata
{ "name": "Code Example Generator" }
```

```ts
const example = `
\`\`\`js
console.log("hello");
\`\`\`
`;
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert!(result.code.is_some());
        // The code block should contain the nested fence example
        assert!(result.code.unwrap().content.contains("console.log"));
    }
    #[test]
    fn test_order_independence() {
        // Schema before metadata should still work
        let content = r#"
```schema
{ "input": { "x": { "type": "string" } } }
```

```metadata
{ "name": "Order Test" }
```

```ts
code here
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_some());
        assert!(result.schema.is_some());
        assert!(result.code.is_some());

        assert_eq!(
            result.metadata.unwrap().name,
            Some("Order Test".to_string())
        );
        assert!(result.schema.unwrap().input.contains_key("x"));
    }
    #[test]
    fn test_code_block_without_metadata_or_schema() {
        let content = r#"
## Simple Script

```bash
echo "Hello World"
```
"#;
        let result = parse_codefence_metadata(content);

        assert!(result.metadata.is_none());
        assert!(result.schema.is_none());
        assert!(result.code.is_some());

        let code = result.code.unwrap();
        assert_eq!(code.language, "bash");
        assert_eq!(code.content, "echo \"Hello World\"");
    }
    #[test]
    fn test_multiple_errors_collected() {
        let content = r#"
```metadata
{ invalid json 1 }
```

```schema
{ invalid json 2 }
```
"#;
        let result = parse_codefence_metadata(content);

        assert_eq!(result.errors.len(), 2);
        assert!(result.errors[0].contains("metadata"));
        assert!(result.errors[1].contains("schema"));
    }

    #[test]
    fn json_metadata_preserves_typed_capability_and_interactive_topology_fields() {
        let content = r#"
```metadata
{
  "sdkCapabilities": ["arg", "readFile"],
  "executionTopology": "typescript-scriptlet-interactive"
}
```

```ts
await arg("Choose a file");
```
"#;

        let metadata = parse_codefence_metadata(content)
            .metadata
            .expect("typed scriptlet metadata");
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!(["arg", "readFile"]))
        );
        assert_eq!(
            metadata.extra.get("executionTopology"),
            Some(&serde_json::json!("typescript-scriptlet-interactive"))
        );
    }

    #[test]
    fn simple_metadata_keeps_canonical_names_and_json_array_shape() {
        let content = r#"
```metadata
sdkCapabilities: ["arg", "readFile"]
executionTopology: typescript-scriptlet-interactive
```

```ts
await arg("Prompt");
```
"#;

        let metadata = parse_codefence_metadata(content)
            .metadata
            .expect("simple scriptlet metadata");
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!(["arg", "readFile"]))
        );
        assert_eq!(
            metadata.extra.get("executionTopology"),
            Some(&serde_json::json!("typescript-scriptlet-interactive"))
        );
        assert!(!metadata.extra.contains_key("sdkcapabilities"));
        assert!(!metadata.extra.contains_key("executiontopology"));
    }

    #[test]
    fn malformed_simple_capability_declaration_is_preserved_for_validation() {
        let content = r#"
```metadata
sdkCapabilities: arg
executionTopology: invalid-topology
```

```ts
console.log("safe fixture");
```
"#;

        let metadata = parse_codefence_metadata(content)
            .metadata
            .expect("malformed values remain observable");
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!("arg"))
        );
        assert_eq!(
            metadata.extra.get("executionTopology"),
            Some(&serde_json::json!("invalid-topology"))
        );
    }
}

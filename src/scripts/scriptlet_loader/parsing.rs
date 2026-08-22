use std::path::Path;

use itertools::Itertools;

use crate::scriptlet_metadata::parse_codefence_metadata;

use super::super::types::Scriptlet;

pub(crate) fn extract_html_comment_metadata(
    text: &str,
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut metadata = HashMap::new();

    // Find HTML comment blocks
    if let Some(start) = text.find("<!--") {
        if let Some(end) = text.find("-->") {
            if start < end {
                let comment_content = &text[start + 4..end];
                // Parse key: value pairs, but ignore fenced markdown that was
                // commented out as an entire snippet block.
                let mut comment_fence: Option<(char, usize)> = None;
                for line in comment_content.lines() {
                    let trimmed = line.trim();
                    if let Some((fence_char, fence_count)) = comment_fence {
                        if is_comment_fence_close(trimmed, fence_char, fence_count) {
                            comment_fence = None;
                        }
                        continue;
                    }
                    if let Some(fence) = detect_comment_fence_open(trimmed) {
                        comment_fence = Some(fence);
                        continue;
                    }
                    if !trimmed.is_empty() {
                        // Handle format: "key: value"
                        if let Some(colon_pos) = trimmed.find(':') {
                            let key = trimmed[..colon_pos].trim().to_string();
                            let value = trimmed[colon_pos + 1..].trim().to_string();
                            metadata.insert(key, value);
                        }
                    }
                }
            }
        }
    }

    metadata
}

fn detect_comment_fence_open(line: &str) -> Option<(char, usize)> {
    let backtick_count = line.chars().take_while(|&c| c == '`').count();
    if backtick_count >= 3 {
        return Some(('`', backtick_count));
    }

    let tilde_count = line.chars().take_while(|&c| c == '~').count();
    if tilde_count >= 3 {
        return Some(('~', tilde_count));
    }

    None
}

fn is_comment_fence_close(line: &str, fence_char: char, min_count: usize) -> bool {
    let count = line.chars().take_while(|&c| c == fence_char).count();
    if count < min_count {
        return false;
    }

    line[count..].chars().all(|c| c.is_whitespace())
}

/// Extract code block from markdown text
/// Looks for ```language ... ``` pattern and returns (language, code)
/// Skips `metadata` and `schema` blocks which are used for configuration
pub(crate) fn extract_code_block(text: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut in_html_comment = false;

    let mut i = 0;
    while i < lines.len() {
        let visible_line = strip_html_comment_segments(lines[i], &mut in_html_comment);
        let trimmed = visible_line.trim_start();

        if let Some((fence_count, language)) = detect_backtick_fence_open(trimmed) {
            let mut code_lines = Vec::new();
            i += 1;

            while i < lines.len() {
                if is_comment_fence_close(lines[i].trim_start(), '`', fence_count) {
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }

            if i >= lines.len() {
                return None;
            }

            if language != "metadata" && language != "schema" {
                return Some((language, code_lines.join("\n").trim().to_string()));
            }

            in_html_comment = false;
        }

        i += 1;
    }

    None
}

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

fn detect_backtick_fence_open(line: &str) -> Option<(usize, String)> {
    let backtick_count = line.chars().take_while(|&c| c == '`').count();
    if backtick_count < 3 {
        return None;
    }

    let rest = &line[backtick_count..];
    Some((
        backtick_count,
        rest.split_whitespace().next().unwrap_or("").to_string(),
    ))
}

/// Convert a name to a command slug (lowercase, spaces/special chars to hyphens)
pub(crate) fn slugify_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .join("-")
}

/// Parse a single scriptlet section from markdown
/// Input should be text from ## Name to the next ## or end of file
/// `source_path` is the path to the .md file containing the scriptlet
pub(crate) fn parse_scriptlet_section(
    section: &str,
    source_path: Option<&Path>,
) -> Option<Scriptlet> {
    let lines: Vec<&str> = section.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // First line should be ## Name
    let first_line = lines[0];
    if !first_line.starts_with("##") {
        return None;
    }

    let name = first_line
        .strip_prefix("##")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if name.is_empty() {
        return None;
    }

    // Extract metadata from HTML comments
    let html_metadata = extract_html_comment_metadata(section);

    // Also try codefence metadata (```metadata ... ```)
    let codefence_result = parse_codefence_metadata(section);
    let typed_metadata = codefence_result.metadata;

    // Log what we found for debugging
    tracing::debug!(
        category = "KEYWORD",
        name = %name,
        html_keyword = ?html_metadata.get("keyword"),
        codefence_keyword = ?typed_metadata.as_ref().and_then(|t| t.keyword.as_ref()),
        "Parsing scriptlet section metadata"
    );

    // Extract code block and normalize fence language
    let (raw_tool, code) = extract_code_block(section)?;
    let tool = crate::scriptlets::normalize_scriptlet_tool(&raw_tool);

    // Generate command slug from name
    let command = slugify_name(&name);

    // Build file_path with anchor if source_path is provided
    let file_path = source_path.map(|p| format!("{}#{}", p.display(), command));

    // Prefer codefence metadata over HTML comment metadata for keyword
    let keyword = typed_metadata
        .as_ref()
        .and_then(|t| t.keyword.clone())
        .or_else(|| html_metadata.get("keyword").cloned());

    if keyword.is_some() {
        tracing::debug!(
            category = "KEYWORD",
            name = %name,
            keyword = ?keyword,
            "Found keyword trigger in scriptlet"
        );
    }

    let capability_metadata = super::super::validation::merge_scriptlet_capability_metadata(
        typed_metadata.as_ref(),
        &html_metadata,
    );
    let scriptlet = Scriptlet {
        name,
        description: typed_metadata
            .as_ref()
            .and_then(|t| t.description.clone())
            .or_else(|| html_metadata.get("description").cloned()),
        code,
        tool,
        shortcut: typed_metadata
            .as_ref()
            .and_then(|t| t.shortcut.clone())
            .or_else(|| html_metadata.get("shortcut").cloned()),
        keyword,
        group: None,
        plugin_id: String::new(),
        plugin_title: None,
        file_path,
        command: Some(command),
        alias: typed_metadata
            .as_ref()
            .and_then(|t| t.alias.clone())
            .or_else(|| html_metadata.get("alias").cloned()),
        icon: html_metadata.get("icon").cloned(),
    };
    super::super::validation::register_scriptlet_capabilities(
        &scriptlet,
        capability_metadata.as_ref(),
    );
    Some(scriptlet)
}

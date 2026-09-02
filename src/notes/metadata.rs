//! Derived metadata for Notes.
//!
//! The note body remains the user-editable source of truth. These helpers parse
//! tags, aliases, and note links so storage can maintain queryable indexes.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedNoteMetadata {
    pub(crate) aliases: Vec<ParsedAlias>,
    pub(crate) tags: Vec<ParsedTag>,
    pub(crate) links: Vec<ParsedNoteLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedAlias {
    pub(crate) alias: String,
    pub(crate) slug: String,
    pub(crate) source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedTag {
    pub(crate) display: String,
    pub(crate) normalized: String,
    pub(crate) source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedNoteLink {
    pub(crate) target_ref: String,
    pub(crate) target_slug: String,
    pub(crate) label: Option<String>,
    pub(crate) kind: &'static str,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MetadataFrontmatterPatch {
    pub(crate) tags: Vec<String>,
    pub(crate) aliases: Vec<String>,
    /// Provenance link (e.g. `scriptkit://agent-chat/{thread_id}`) recorded
    /// when a note is created by an agent or another surface.
    pub(crate) source: Option<String>,
}

pub(crate) fn parse_note_metadata(title: &str, content: &str) -> ParsedNoteMetadata {
    let frontmatter = parse_frontmatter(content);
    let body = strip_frontmatter(content);
    let searchable = strip_code_spans_and_fences(body);

    let mut aliases = Vec::new();
    if !title.trim().is_empty() {
        aliases.push(ParsedAlias {
            alias: title.trim().to_string(),
            slug: slugify_note_ref(title),
            source: "title",
        });
    }
    if let Some(frontmatter) = &frontmatter {
        aliases.extend(
            parse_frontmatter_values(frontmatter, "aliases")
                .into_iter()
                .map(|alias| ParsedAlias {
                    slug: slugify_note_ref(&alias),
                    alias,
                    source: "frontmatter",
                }),
        );
    }

    let mut tags = frontmatter
        .as_deref()
        .map(|frontmatter| parse_frontmatter_values(frontmatter, "tags"))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|tag| parsed_tag(tag, "frontmatter"));
    let mut tags: Vec<_> = tags.by_ref().collect();
    tags.extend(scan_hash_tags(&searchable));

    let links = scan_wiki_links(&searchable);

    ParsedNoteMetadata {
        aliases: dedupe_by(aliases, |alias| alias.slug.clone()),
        tags: dedupe_by(tags, |tag| tag.normalized.clone()),
        links: dedupe_by(links, |link| {
            format!("{}:{}:{}", link.target_slug, link.byte_start, link.byte_end)
        }),
    }
}

pub(crate) fn merge_frontmatter(content: &str, patch: MetadataFrontmatterPatch) -> String {
    if patch.tags.is_empty() && patch.aliases.is_empty() && patch.source.is_none() {
        return content.to_string();
    }

    let frontmatter = parse_frontmatter(content).unwrap_or_default();
    let tags = merge_values(parse_frontmatter_values(&frontmatter, "tags"), patch.tags);
    let aliases = merge_values(
        parse_frontmatter_values(&frontmatter, "aliases"),
        patch.aliases,
    );
    let source = patch
        .source
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if frontmatter.is_empty() && tags.is_empty() && aliases.is_empty() && source.is_none() {
        return strip_frontmatter(content).to_string();
    }

    let excluded: &[&str] = if source.is_some() {
        &["tags", "aliases", "source"]
    } else {
        &["tags", "aliases"]
    };

    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.extend(preserved_frontmatter_lines(&frontmatter, excluded));
    if !tags.is_empty() {
        lines.push(format!("tags: [{}]", format_yaml_list(&tags)));
    }
    if !aliases.is_empty() {
        lines.push(format!("aliases: [{}]", format_yaml_list(&aliases)));
    }
    if let Some(source) = source {
        lines.push(format!("source: {}", source));
    }
    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(strip_frontmatter(content).to_string());
    lines.join("\n")
}

pub(crate) fn parse_frontmatter_source(content: &str) -> Option<String> {
    let frontmatter = parse_frontmatter(content)?;
    if let Ok(serde_yaml::Value::Mapping(map)) =
        serde_yaml::from_str::<serde_yaml::Value>(&frontmatter)
    {
        if let Some(value) = map.get(serde_yaml::Value::String("source".to_string())) {
            if let Some(source) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(source.to_string());
            }
        }
    }

    frontmatter.lines().find_map(|line| {
        let (line_key, value) = line.split_once(':')?;
        (line_key.trim() == "source")
            .then(|| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn normalize_tag(tag: &str) -> Option<String> {
    let trimmed = tag.trim().trim_start_matches('#').trim();
    if trimmed.is_empty() || trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let mut normalized = String::new();
    let mut last_dash = false;
    for ch in trimmed.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() || ch == '/' || ch == '_' {
            normalized.push(ch);
            last_dash = false;
        } else if !last_dash {
            normalized.push('-');
            last_dash = true;
        }
    }

    let normalized = normalized.trim_matches('-').to_string();
    (!normalized.is_empty()).then_some(normalized)
}

pub(crate) fn slugify_note_ref(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn parsed_tag(tag: String, source: &'static str) -> Option<ParsedTag> {
    normalize_tag(&tag).map(|normalized| ParsedTag {
        display: tag.trim().trim_start_matches('#').trim().to_string(),
        normalized,
        source,
    })
}

fn parse_frontmatter(content: &str) -> Option<String> {
    frontmatter_bounds(content).map(|(start, end, _)| content[start..end].to_string())
}

pub(crate) fn strip_frontmatter(content: &str) -> &str {
    let Some((_, _, body_start)) = frontmatter_bounds(content) else {
        return content;
    };
    content[body_start..].trim_start_matches(['\n', '\r'])
}

fn parse_frontmatter_values(frontmatter: &str, key: &str) -> Vec<String> {
    if let Ok(serde_yaml::Value::Mapping(map)) =
        serde_yaml::from_str::<serde_yaml::Value>(frontmatter)
    {
        if let Some(value) = map.get(serde_yaml::Value::String(key.to_string())) {
            return yaml_value_strings(value);
        }
    }

    let Some(raw) = frontmatter.lines().find_map(|line| {
        let (line_key, value) = line.split_once(':')?;
        (line_key.trim() == key).then(|| value.trim())
    }) else {
        return Vec::new();
    };

    parse_value_list(raw)
}

fn frontmatter_bounds(content: &str) -> Option<(usize, usize, usize)> {
    let rest = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))?;
    let content_start = content.len() - rest.len();
    let mut offset = content_start;
    for line in rest.split_inclusive('\n') {
        let line_end = offset + line.len();
        if line.trim_end() == "---" {
            return Some((content_start, offset, line_end));
        }
        offset = line_end;
    }
    None
}

fn yaml_value_strings(value: &serde_yaml::Value) -> Vec<String> {
    match value {
        serde_yaml::Value::String(value) => vec![value.clone()],
        serde_yaml::Value::Sequence(values) => values
            .iter()
            .filter_map(|value| match value {
                serde_yaml::Value::String(value) => Some(value.clone()),
                serde_yaml::Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|value| !value.trim().is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn preserved_frontmatter_lines(frontmatter: &str, replaced_keys: &[&str]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut skipping_block_key = false;
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        let key = line
            .split_once(':')
            .map(|(key, _)| key.trim())
            .filter(|key| !key.is_empty());
        if key.is_some_and(|key| replaced_keys.contains(&key)) {
            skipping_block_key = true;
            continue;
        }
        if skipping_block_key {
            let is_block_item =
                line.starts_with(' ') || line.starts_with('\t') || trimmed.starts_with('-');
            if is_block_item || trimmed.is_empty() {
                continue;
            }
            skipping_block_key = false;
        }
        lines.push(line.to_string());
    }
    lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

fn merge_values(existing: Vec<String>, incoming: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    existing
        .into_iter()
        .chain(incoming)
        .filter_map(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty() && seen.insert(trimmed.to_lowercase())).then_some(trimmed)
        })
        .collect()
}

fn parse_value_list(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .map(|value| {
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn strip_code_spans_and_fences(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_fence = false;
    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            result.push('\n');
            continue;
        }
        if in_fence {
            result.push('\n');
            continue;
        }

        let mut in_inline = false;
        for ch in line.chars() {
            if ch == '`' {
                in_inline = !in_inline;
                result.push(' ');
            } else if in_inline {
                result.push(' ');
            } else {
                result.push(ch);
            }
        }
        result.push('\n');
    }
    result
}

fn scan_hash_tags(content: &str) -> Vec<ParsedTag> {
    let bytes = content.as_bytes();
    let mut tags = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'#' || (index > 0 && is_tag_char(bytes[index - 1] as char)) {
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && is_tag_char(bytes[end] as char) {
            end += 1;
        }

        if end > start {
            if let Some(tag) = parsed_tag(content[start..end].to_string(), "markdown") {
                tags.push(tag);
            }
        }
        index = end.max(index + 1);
    }
    tags
}

fn is_tag_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '/'
}

fn scan_wiki_links(content: &str) -> Vec<ParsedNoteLink> {
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = content[cursor..].find("[[") {
        let start = cursor + relative_start;
        let content_start = start + 2;
        let Some(relative_end) = content[content_start..].find("]]") else {
            break;
        };
        let end = content_start + relative_end + 2;
        let inner = content[content_start..content_start + relative_end].trim();
        if !inner.is_empty() {
            let (target, label) = inner
                .split_once('|')
                .map(|(target, label)| (target.trim(), Some(label.trim().to_string())))
                .unwrap_or((inner, None));
            let target_slug = slugify_note_ref(target);
            if !target_slug.is_empty() {
                links.push(ParsedNoteLink {
                    target_ref: target.to_string(),
                    target_slug,
                    label: label.filter(|value| !value.is_empty()),
                    kind: "wiki",
                    byte_start: start,
                    byte_end: end,
                });
            }
        }
        cursor = end;
    }
    links
}

fn dedupe_by<T, F>(items: Vec<T>, key_fn: F) -> Vec<T>
where
    F: Fn(&T) -> String,
{
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key_fn(item)))
        .collect()
}

fn format_yaml_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{}\"", value.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_tags_aliases_hash_tags_and_wiki_links() {
        let parsed = parse_note_metadata(
            "Project Plan",
            "---\ntags: [rust, notes/metadata]\naliases: [Plan, Project Index]\n---\n# Body\n#inbox [[Target Note|readable]]",
        );

        assert!(parsed.tags.iter().any(|tag| tag.normalized == "rust"));
        assert!(parsed
            .tags
            .iter()
            .any(|tag| tag.normalized == "notes/metadata"));
        assert!(parsed.tags.iter().any(|tag| tag.normalized == "inbox"));
        assert!(parsed
            .aliases
            .iter()
            .any(|alias| alias.slug == "project-plan"));
        assert!(parsed.aliases.iter().any(|alias| alias.slug == "plan"));
        assert_eq!(parsed.links[0].target_slug, "target-note");
        assert_eq!(parsed.links[0].label.as_deref(), Some("readable"));
    }

    #[test]
    fn ignores_tags_and_links_inside_code() {
        let parsed = parse_note_metadata(
            "Code",
            "```md\n#ignored [[Ignored]]\n```\n`#also-ignored [[Nope]]`\n#kept [[Kept]]",
        );

        assert_eq!(parsed.tags.len(), 1);
        assert_eq!(parsed.tags[0].normalized, "kept");
        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].target_slug, "kept");
    }

    #[test]
    fn merge_frontmatter_prepends_visible_metadata() {
        let merged = merge_frontmatter(
            "# Note\nBody",
            MetadataFrontmatterPatch {
                tags: vec!["rust".to_string()],
                aliases: vec!["Rust Note".to_string()],
                source: None,
            },
        );

        assert!(merged.starts_with("---\ntags: [\"rust\"]\naliases: [\"Rust Note\"]\n---"));
        assert!(merged.contains("# Note\nBody"));
    }

    #[test]
    fn merge_frontmatter_preserves_existing_keys_and_unions_tags_aliases() {
        let merged = merge_frontmatter(
            "---\ntitle: Kept\ntags:\n  - rust\naliases: [Plan]\nowner: John\n---\n# Note\nBody",
            MetadataFrontmatterPatch {
                tags: vec!["rust".to_string(), "notes/metadata".to_string()],
                aliases: vec!["Plan".to_string(), "Project Plan".to_string()],
                source: None,
            },
        );

        assert!(merged.contains("title: Kept"));
        assert!(merged.contains("owner: John"));
        assert!(merged.contains("tags: [\"rust\", \"notes/metadata\"]"));
        assert!(merged.contains("aliases: [\"Plan\", \"Project Plan\"]"));
        assert!(merged.ends_with("# Note\nBody"));
    }

    #[test]
    fn merge_frontmatter_records_source_provenance() {
        let merged = merge_frontmatter(
            "# Note\nBody",
            MetadataFrontmatterPatch {
                tags: vec![],
                aliases: vec![],
                source: Some("scriptkit://agent-chat/thread-123".to_string()),
            },
        );

        assert!(merged.starts_with("---\nsource: scriptkit://agent-chat/thread-123\n---"));
        assert!(merged.contains("# Note\nBody"));
    }

    #[test]
    fn merge_frontmatter_replaces_existing_source() {
        let merged = merge_frontmatter(
            "---\nsource: old-value\nowner: John\n---\n# Note\nBody",
            MetadataFrontmatterPatch {
                tags: vec![],
                aliases: vec![],
                source: Some("scriptkit://agent-chat/thread-456".to_string()),
            },
        );

        assert!(merged.contains("source: scriptkit://agent-chat/thread-456"));
        assert!(!merged.contains("old-value"));
        assert!(merged.contains("owner: John"));
    }

    #[test]
    fn parse_frontmatter_source_reads_source_without_brain_fields() {
        assert_eq!(
            parse_frontmatter_source("---\nsource: scriptkit://agent-chat/thread-789\n---\n# Note"),
            Some("scriptkit://agent-chat/thread-789".to_string())
        );
    }

    #[test]
    fn frontmatter_parser_handles_block_lists_crlf_and_quoted_commas() {
        let parsed = parse_note_metadata(
            "Fallback",
            "---\r\ntags:\r\n  - \"alpha, beta\"\r\n  - notes\r\naliases:\r\n  - \"Plan, Draft\"\r\n---\r\n# Body",
        );

        assert!(parsed.tags.iter().any(|tag| tag.display == "alpha, beta"));
        assert!(parsed
            .aliases
            .iter()
            .any(|alias| alias.alias == "Plan, Draft"));
    }

    #[test]
    fn markdown_tags_and_links_ignore_yaml_frontmatter() {
        let parsed = parse_note_metadata(
            "Frontmatter",
            "---\nsummary: \"#ignored [[Nope]]\"\ntags: [front]\n---\n# Body #kept [[Kept]]",
        );

        assert!(parsed.tags.iter().any(|tag| tag.normalized == "front"));
        assert!(parsed.tags.iter().any(|tag| tag.normalized == "kept"));
        assert!(!parsed.tags.iter().any(|tag| tag.normalized == "ignored"));
        assert_eq!(parsed.links.len(), 1);
        assert_eq!(parsed.links[0].target_slug, "kept");
    }
}

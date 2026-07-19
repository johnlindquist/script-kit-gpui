use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::menu_syntax::{LinkLookup, LinkScriptletDraft, LinkScriptletOperation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkMarkdownSection {
    pub title: String,
    pub id: String,
    pub url: Option<String>,
    pub description: Option<String>,
    pub metadata: Map<String, Value>,
    pub body: String,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStoreOperation {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStoreOutcome {
    pub operation: LinkStoreOperation,
    pub path: PathBuf,
}

pub fn links_markdown_path(sk_path: &Path) -> PathBuf {
    sk_path.join("plugins/main/scriptlets/links.md")
}

pub fn load_link_sections(path: &Path) -> Result<Vec<LinkMarkdownSection>, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("Read links.md failed: {error}")),
    };
    Ok(parse_sections(&content))
}

pub fn upsert_link_section(
    sk_path: &Path,
    draft: &LinkScriptletDraft,
) -> Result<LinkStoreOutcome, String> {
    if draft.url.as_deref().unwrap_or("").trim().is_empty() {
        return Err("Add a valid http:// or https:// URL.".to_string());
    }
    let path = links_markdown_path(sk_path);
    let original = read_or_default(&path)?;
    let sections = parse_sections(&original);
    let match_index = matching_section_index(&sections, draft)?;
    if !matches!(draft.operation, LinkScriptletOperation::Create) && match_index.is_none() {
        return Err("Link not found.".to_string());
    }
    let existing = match_index.and_then(|index| sections.get(index));
    let mut metadata = existing
        .map(|section| section.metadata.clone())
        .unwrap_or_default();
    for (key, value) in &draft.metadata {
        metadata.insert(key.clone(), value.clone());
    }
    let url = draft
        .url
        .clone()
        .or_else(|| existing.and_then(|section| section.url.clone()))
        .ok_or_else(|| "Add a valid http:// or https:// URL.".to_string())?;
    metadata.insert("url".to_string(), Value::String(url.clone()));
    metadata.insert("tool".to_string(), Value::String("open".to_string()));
    let title = draft
        .title
        .clone()
        .or_else(|| existing.map(|section| section.title.clone()))
        .unwrap_or_else(|| derive_title_from_url(&url));
    metadata.insert("title".to_string(), Value::String(title.clone()));
    let next_section = render_section(&title, &metadata, &url)?;
    let next = replace_or_append_section(&original, existing, &next_section);
    atomic_write(&path, &next)?;
    Ok(LinkStoreOutcome {
        operation: if existing.is_some() {
            LinkStoreOperation::Updated
        } else {
            LinkStoreOperation::Created
        },
        path,
    })
}

pub fn delete_link_section(
    sk_path: &Path,
    draft: &LinkScriptletDraft,
) -> Result<LinkStoreOutcome, String> {
    let path = links_markdown_path(sk_path);
    let original = read_or_default(&path)?;
    let sections = parse_sections(&original);
    let Some(index) = matching_section_index(&sections, draft)? else {
        return Err("Link not found.".to_string());
    };
    let section = &sections[index];
    let mut next = String::new();
    next.push_str(&original[..section.start]);
    next.push_str(original[section.end..].trim_start_matches('\n'));
    atomic_write(&path, &next)?;
    Ok(LinkStoreOutcome {
        operation: LinkStoreOperation::Deleted,
        path,
    })
}

pub fn link_object_candidates_from_markdown(
    sk_path: &Path,
) -> Result<Vec<crate::menu_syntax::ObjectSelectorCandidate>, String> {
    let path = links_markdown_path(sk_path);
    let mut candidates = load_link_sections(&path)?
        .into_iter()
        .filter_map(|section| {
            let url = section.url?;
            Some(crate::menu_syntax::ObjectSelectorCandidate {
                kind: crate::menu_syntax::CaptureObjectKind::Link,
                id: url,
                label: section.title,
                subtitle: section
                    .description
                    .unwrap_or_else(|| "Link scriptlet".to_string()),
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        a.label
            .to_ascii_lowercase()
            .cmp(&b.label.to_ascii_lowercase())
    });
    candidates.dedup_by(|a, b| a.id == b.id);
    Ok(candidates)
}

fn read_or_default(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("# Links\n\n".to_string()),
        Err(error) => Err(format!("Read links.md failed: {error}")),
    }
}

fn parse_sections(content: &str) -> Vec<LinkMarkdownSection> {
    let mut heads = content
        .match_indices("\n## ")
        .map(|(idx, _)| idx + 1)
        .collect::<Vec<_>>();
    if content.starts_with("## ") {
        heads.insert(0, 0);
    }
    heads
        .iter()
        .enumerate()
        .filter_map(|(idx, start)| {
            let end = heads.get(idx + 1).copied().unwrap_or(content.len());
            parse_section(content, *start, end)
        })
        .collect()
}

fn parse_section(content: &str, start: usize, end: usize) -> Option<LinkMarkdownSection> {
    let section = &content[start..end];
    let first_line_end = section.find('\n').unwrap_or(section.len());
    let title = section[..first_line_end]
        .trim_start_matches("## ")
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let metadata = parse_metadata_fence(section).unwrap_or_default();
    let body = fence_content(section, "open")
        .unwrap_or_default()
        .trim_end_matches('\n')
        .to_string();
    let url = metadata
        .get("url")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| (!body.trim().is_empty()).then(|| body.trim().to_string()));
    let description = metadata
        .get("description")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let id = url.clone().unwrap_or_else(|| slugify(&title));
    Some(LinkMarkdownSection {
        title,
        id,
        url,
        description,
        metadata,
        body,
        start,
        end,
    })
}

fn parse_metadata_fence(section: &str) -> Option<Map<String, Value>> {
    let content = fence_content(section, "metadata")?;
    serde_json::from_str::<Map<String, Value>>(content.trim()).ok()
}

fn fence_content<'a>(section: &'a str, lang: &str) -> Option<&'a str> {
    let marker = format!("```{lang}");
    let start = section.find(&marker)?;
    let after_marker = start + marker.len();
    let content_start = section[after_marker..].find('\n')? + after_marker + 1;
    let content_end = section[content_start..].find("\n```")? + content_start;
    Some(&section[content_start..content_end])
}

fn matching_section_index(
    sections: &[LinkMarkdownSection],
    draft: &LinkScriptletDraft,
) -> Result<Option<usize>, String> {
    if matches!(draft.operation, LinkScriptletOperation::Create) {
        return Ok(draft
            .url
            .as_deref()
            .and_then(|url| find_by_url(sections, url)));
    }
    let by_lookup = match &draft.lookup {
        Some(LinkLookup::SelectedRef(id)) => find_by_url(sections, id),
        Some(LinkLookup::Url(url)) => find_by_url(sections, url),
        Some(LinkLookup::Title(title)) => find_unique_by_title(sections, title)?,
        None => None,
    };
    let by_title = draft
        .title
        .as_deref()
        .map(|title| find_unique_by_title(sections, title))
        .transpose()?
        .flatten();
    let by_url = draft
        .url
        .as_deref()
        .and_then(|url| find_by_url(sections, url));
    if let (Some(title_idx), Some(url_idx)) = (by_title, by_url) {
        if title_idx != url_idx {
            return Err("Link title and URL match different links. Select one with @.".to_string());
        }
    }
    Ok(by_lookup.or(by_url).or(by_title))
}

fn find_by_url(sections: &[LinkMarkdownSection], url: &str) -> Option<usize> {
    sections
        .iter()
        .position(|section| section.url.as_deref() == Some(url) || section.id == url)
}

fn find_unique_by_title(
    sections: &[LinkMarkdownSection],
    title: &str,
) -> Result<Option<usize>, String> {
    let matches = sections
        .iter()
        .enumerate()
        .filter(|(_, section)| {
            section.title.eq_ignore_ascii_case(title) || slugify(&section.title) == slugify(title)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [index] => Ok(Some(*index)),
        _ => Err("Multiple links match that title. Select one with @.".to_string()),
    }
}

fn replace_or_append_section(
    original: &str,
    existing: Option<&LinkMarkdownSection>,
    next_section: &str,
) -> String {
    if let Some(existing) = existing {
        let mut next = String::new();
        next.push_str(&original[..existing.start]);
        next.push_str(next_section);
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(original[existing.end..].trim_start_matches('\n'));
        return next;
    }
    let mut next = if original.trim().is_empty() {
        "# Links\n\n".to_string()
    } else {
        original.to_string()
    };
    if !next.ends_with("\n\n") {
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
    }
    next.push_str(next_section);
    next
}

fn render_section(title: &str, metadata: &Map<String, Value>, url: &str) -> Result<String, String> {
    let metadata_json = serde_json::to_string_pretty(metadata)
        .map_err(|error| format!("Serialize link metadata failed: {error}"))?;
    let fence = fence_for(&[&metadata_json, url]);
    Ok(format!(
        "## {title}\n\n{fence}metadata\n{metadata_json}\n{fence}\n\n{fence}open\n{url}\n{fence}\n"
    ))
}

fn fence_for(parts: &[&str]) -> String {
    let longest = parts
        .iter()
        .flat_map(|part| part.split(|ch| ch != '`'))
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(longest.max(3) + 1)
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    // Unique-temp atomic write: a fixed temp path let concurrent savers corrupt
    // the store (torn interleaved writes). See src/atomic_file.rs.
    crate::atomic_file::write_atomic(path, contents.as_bytes())
        .map_err(|error| format!("Write links.md failed: {error}"))
}

fn derive_title_from_url(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_syntax::capture::{parse_capture, CaptureParse};
    use tempfile::TempDir;

    fn draft(input: &str) -> LinkScriptletDraft {
        let invocation = match parse_capture(input) {
            CaptureParse::Ok(invocation) => invocation,
            CaptureParse::Incomplete(incomplete) => panic!("{incomplete:?}"),
        };
        crate::menu_syntax::parse_link_scriptlet_capture(&invocation).expect("draft")
    }

    #[test]
    fn create_initializes_plugins_main_scriptlets_links_md() {
        let tmp = TempDir::new().unwrap();
        let draft = draft(";link https://example.com Example description:Docs #docs");

        let outcome = upsert_link_section(tmp.path(), &draft).unwrap();

        assert_eq!(outcome.operation, LinkStoreOperation::Created);
        let content = std::fs::read_to_string(links_markdown_path(tmp.path())).unwrap();
        assert!(content.contains("# Links"));
        assert!(content.contains("## Example"));
        assert!(content.contains(r#""url": "https://example.com""#));
        assert!(content.contains(r#""tool": "open""#));
        assert!(content.contains("````open\nhttps://example.com\n````"));
        assert!(!tmp.path().join("menu-syntax/bookmarks.jsonl").exists());
    }

    #[test]
    fn create_is_idempotent_by_url() {
        let tmp = TempDir::new().unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://example.com Example")).unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://example.com Better")).unwrap();
        let sections = load_link_sections(&links_markdown_path(tmp.path())).unwrap();

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Better");
    }

    #[test]
    fn create_with_existing_title_but_different_url_appends_section() {
        let tmp = TempDir::new().unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://example.com Example")).unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://second.example Example")).unwrap();
        let sections = load_link_sections(&links_markdown_path(tmp.path())).unwrap();

        assert_eq!(sections.len(), 2);
        assert!(sections
            .iter()
            .any(|section| section.url.as_deref() == Some("https://example.com")));
        assert!(sections
            .iter()
            .any(|section| section.url.as_deref() == Some("https://second.example")));
    }

    #[test]
    fn update_by_selected_ref_preserves_url_when_url_missing() {
        let tmp = TempDir::new().unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://example.com Example")).unwrap();

        upsert_link_section(
            tmp.path(),
            &draft(";link update @link:https://example.com title:New"),
        )
        .unwrap();
        let sections = load_link_sections(&links_markdown_path(tmp.path())).unwrap();

        assert_eq!(sections[0].url.as_deref(), Some("https://example.com"));
        assert_eq!(sections[0].title, "New");
    }

    #[test]
    fn delete_removes_only_selected_link_section() {
        let tmp = TempDir::new().unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://example.com Example")).unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://second.example Second")).unwrap();

        delete_link_section(tmp.path(), &draft(";link delete @link:https://example.com")).unwrap();
        let sections = load_link_sections(&links_markdown_path(tmp.path())).unwrap();

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "Second");
    }

    #[test]
    fn selected_missing_ref_does_not_create_links_md() {
        let tmp = TempDir::new().unwrap();
        let err = delete_link_section(
            tmp.path(),
            &draft(";link delete @link:https://missing.example"),
        )
        .expect_err("missing link");

        assert_eq!(err, "Link not found.");
        assert!(!links_markdown_path(tmp.path()).exists());
    }

    #[test]
    fn object_candidates_from_links_markdown_use_url_id_and_title_label() {
        let tmp = TempDir::new().unwrap();
        upsert_link_section(tmp.path(), &draft(";link https://example.com Example")).unwrap();

        let candidates = link_object_candidates_from_markdown(tmp.path()).unwrap();

        assert_eq!(candidates[0].id, "https://example.com");
        assert_eq!(candidates[0].label, "Example");
        assert_eq!(candidates[0].token(), "@link:https://example.com");
    }
}

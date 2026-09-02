use std::fs;

use anyhow::{Context, Result};
use tracing::info;

use super::types::{PluginIndex, PluginSkill};

/// Parse YAML frontmatter from a SKILL.md file.
///
/// Returns `(title, description)` parsed from `title:` and `description:` keys.
/// Both values are `None` when the key is missing or the file has no frontmatter.
fn parse_skill_frontmatter(content: &str) -> (Option<String>, Option<String>) {
    if !content.starts_with("---") {
        return (None, None);
    }
    let Some(end) = content[3..].find("---") else {
        return (None, None);
    };
    let frontmatter = &content[3..3 + end];

    let mut title = None;
    let mut description = None;

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("title:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                title = Some(val.to_string());
            }
        } else if let Some(rest) = trimmed.strip_prefix("description:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                description = Some(val.to_string());
            }
        }
    }

    (title, description)
}

/// Extract the first `# Heading` line from markdown content as a fallback title.
fn parse_first_h1(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}

/// Discover skills across all plugins in the index.
///
/// Each skill lives at `<plugin_root>/skills/<skill_id>/SKILL.md`. Results
/// are sorted by `(plugin_id, skill_id)` for deterministic ordering.
///
/// Title resolution order: frontmatter `title:` → first `# H1` → `skill_id`.
/// Description comes from frontmatter `description:` only; empty string if absent.
pub fn discover_plugin_skills(index: &PluginIndex) -> Result<Vec<PluginSkill>> {
    let mut skills = Vec::new();

    for plugin in &index.plugins {
        let skills_dir = plugin.root.join("skills");
        if !skills_dir.try_exists().with_context(|| {
            format!(
                "Failed to inspect skills directory: {}",
                skills_dir.display()
            )
        })? {
            continue;
        }

        let entries = fs::read_dir(&skills_dir).with_context(|| {
            format!(
                "Failed to read skills dir for plugin {}: {}",
                plugin.id,
                skills_dir.display()
            )
        })?;

        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "Failed to enumerate skills directory: {}",
                    skills_dir.display()
                )
            })?;

            let skill_root = entry.path();
            if !fs::metadata(&skill_root)
                .with_context(|| format!("Failed to inspect skill root: {}", skill_root.display()))?
                .is_dir()
            {
                continue;
            }
            let skill_doc = skill_root.join("SKILL.md");
            if !skill_doc.try_exists().with_context(|| {
                format!("Failed to inspect skill document: {}", skill_doc.display())
            })? {
                continue;
            }

            let skill_id = skill_root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // Parse title and description from SKILL.md content
            let content = fs::read_to_string(&skill_doc).with_context(|| {
                format!("Failed to read skill document: {}", skill_doc.display())
            })?;
            let (fm_title, fm_description) = parse_skill_frontmatter(&content);

            let title = fm_title
                .or_else(|| parse_first_h1(&content))
                .unwrap_or_else(|| skill_id.clone());

            let description = fm_description.unwrap_or_default();

            let plugin_title = if plugin.manifest.title.is_empty() {
                plugin.id.clone()
            } else {
                plugin.manifest.title.clone()
            };

            info!(
                plugin_id = %plugin.id,
                plugin_title = %plugin_title,
                skill_id = %skill_id,
                title = %title,
                "plugin_skill_cataloged"
            );

            skills.push(PluginSkill {
                plugin_id: plugin.id.clone(),
                plugin_title,
                skill_id: skill_id.clone(),
                path: skill_doc,
                title,
                description,
            });
        }
    }

    skills.sort_by(|a, b| {
        a.plugin_id
            .cmp(&b.plugin_id)
            .then_with(|| a.skill_id.cmp(&b.skill_id))
    });

    Ok(skills)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_skill_absence_is_empty_but_failed_document_read_rejects_snapshot() {
        let root = tempfile::tempdir().expect("create plugin container");
        let plugin = root.path().join("main");
        fs::create_dir(&plugin).expect("create plugin");
        let index = crate::plugins::discover_plugins_in(root.path()).expect("discover plugin");
        assert!(discover_plugin_skills(&index)
            .expect("optional absence")
            .is_empty());
        let skill = plugin.join("skills/retained");
        fs::create_dir_all(&skill).expect("create skill");
        fs::write(skill.join("SKILL.md"), "# Retained Skill").expect("write skill");
        assert_eq!(
            discover_plugin_skills(&index).expect("read good source")[0].title,
            "Retained Skill"
        );
        fs::write(skill.join("SKILL.md"), [0xff, 0xfe]).expect("write invalid source");
        assert!(
            discover_plugin_skills(&index).is_err(),
            "failed content read is not an empty successful skill"
        );
    }

    #[test]
    fn parse_frontmatter_title_and_description() {
        let content = "---\ntitle: My Skill\ndescription: Does things\n---\n# Body";
        let (title, desc) = parse_skill_frontmatter(content);
        assert_eq!(title.as_deref(), Some("My Skill"));
        assert_eq!(desc.as_deref(), Some("Does things"));
    }

    #[test]
    fn parse_frontmatter_quoted_values() {
        let content = "---\ntitle: \"Quoted Title\"\ndescription: 'Single quoted'\n---\n";
        let (title, desc) = parse_skill_frontmatter(content);
        assert_eq!(title.as_deref(), Some("Quoted Title"));
        assert_eq!(desc.as_deref(), Some("Single quoted"));
    }

    #[test]
    fn parse_frontmatter_missing_fields() {
        let content = "---\nother: value\n---\n# H1";
        let (title, desc) = parse_skill_frontmatter(content);
        assert!(title.is_none());
        assert!(desc.is_none());
    }

    #[test]
    fn parse_frontmatter_no_frontmatter() {
        let content = "# Just a heading\nSome body text";
        let (title, desc) = parse_skill_frontmatter(content);
        assert!(title.is_none());
        assert!(desc.is_none());
    }

    #[test]
    fn first_h1_found() {
        let content = "---\n---\n\n# My Heading\nSome text";
        assert_eq!(parse_first_h1(content).as_deref(), Some("My Heading"));
    }

    #[test]
    fn first_h1_skips_h2() {
        let content = "## Not H1\n# Actual H1";
        assert_eq!(parse_first_h1(content).as_deref(), Some("Actual H1"));
    }

    #[test]
    fn first_h1_none_when_absent() {
        let content = "No headings here\nJust text";
        assert!(parse_first_h1(content).is_none());
    }
}

//! Frontmatter schema for brain markdown files.

use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::notes::metadata::{merge_frontmatter, strip_frontmatter, MetadataFrontmatterPatch};
use crate::notes::NoteId;

/// Canonical frontmatter for brain notes and fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainFrontmatter {
    pub id: NoteId,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// User-provided "why" retained from older annotated clipboard captures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// User metadata not owned by the application, including nested YAML values.
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

impl BrainFrontmatter {
    pub fn new(id: NoteId, created: DateTime<Utc>, updated: DateTime<Utc>) -> Self {
        Self {
            id,
            created,
            updated,
            tags: Vec::new(),
            aliases: Vec::new(),
            pinned: false,
            source: None,
            why: None,
            extra: serde_yaml::Mapping::new(),
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_why(mut self, why: impl Into<String>) -> Self {
        self.why = Some(why.into());
        self
    }

    /// Serialize frontmatter plus body into a markdown document.
    pub fn render(&self, body: &str) -> Result<String> {
        let mut lines = Vec::new();
        lines.push("---".to_string());
        lines.push(format!("id: {}", self.id));
        lines.push(format!("created: {}", self.created.to_rfc3339()));
        lines.push(format!("updated: {}", self.updated.to_rfc3339()));
        if self.pinned {
            lines.push("pinned: true".to_string());
        }
        if !self.tags.is_empty() {
            lines.push(format!("tags: [{}]", format_yaml_quoted_list(&self.tags)));
        }
        if !self.aliases.is_empty() {
            lines.push(format!(
                "aliases: [{}]",
                format_yaml_quoted_list(&self.aliases)
            ));
        }
        if let Some(source) = &self.source {
            lines.push(format!("source: {source}"));
        }
        if let Some(why) = &self.why {
            lines.push(format!("why: {}", quote_yaml_scalar(why)));
        }
        if !self.extra.is_empty() {
            let mut extra = serde_yaml::to_string(&self.extra)?;
            // Joining the lines restores one separator; retain scalar trailing newlines.
            if extra.ends_with('\n') {
                extra.truncate(extra.len() - 1);
            }
            lines.push(extra);
        }
        lines.push("---".to_string());
        lines.push(String::new());

        let body = body.trim_start_matches(['\n', '\r']);
        Ok(if body.is_empty() {
            lines.join("\n")
        } else {
            lines.push(body.to_string());
            lines.join("\n")
        })
    }

    /// Parse frontmatter and body from a markdown document.
    pub fn parse(content: &str) -> Result<(Self, String)> {
        let frontmatter_raw = parse_frontmatter_block(content)
            .with_context(|| "brain document missing frontmatter")?;
        let body = strip_frontmatter(content).to_string();

        let parsed: BrainFrontmatterYaml = serde_yaml::from_str(&frontmatter_raw)
            .with_context(|| "parsing brain frontmatter yaml")?;

        let id = NoteId::parse(&parsed.id)
            .with_context(|| format!("invalid note id in frontmatter: {}", parsed.id))?;
        let created = DateTime::parse_from_rfc3339(&parsed.created)
            .with_context(|| format!("invalid created timestamp: {}", parsed.created))?
            .with_timezone(&Utc);
        let updated = DateTime::parse_from_rfc3339(&parsed.updated)
            .with_context(|| format!("invalid updated timestamp: {}", parsed.updated))?
            .with_timezone(&Utc);

        Ok((
            Self {
                id,
                created,
                updated,
                tags: parsed.tags.unwrap_or_default(),
                aliases: parsed.aliases.unwrap_or_default(),
                pinned: parsed.pinned.unwrap_or(false),
                source: parsed.source,
                why: parsed.why,
                extra: parsed.extra,
            },
            body,
        ))
    }

    /// Merge visible managed metadata and custom YAML into the document body.
    pub fn merge_into_body(&self, body: &str) -> Result<String> {
        let merged = merge_frontmatter(
            body,
            MetadataFrontmatterPatch {
                tags: self.tags.clone(),
                aliases: self.aliases.clone(),
                source: self.source.clone(),
            },
        );
        if self.extra.is_empty() {
            return Ok(merged);
        }
        let mut fields = parse_frontmatter_block(&merged)
            .map(|raw| serde_yaml::from_str::<serde_yaml::Mapping>(&raw))
            .transpose()?
            .unwrap_or_default();
        fields.extend(
            self.extra
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        Ok(format!(
            "---\n{}---\n\n{}",
            serde_yaml::to_string(&fields)?,
            strip_frontmatter(&merged)
        ))
    }

    /// Preserve editor-supplied metadata without accepting replacement identities.
    pub fn merge_extra_from_content(&mut self, content: &str) -> Result<()> {
        let Some(raw) = parse_frontmatter_block(content) else {
            return Ok(());
        };
        let fields: serde_yaml::Mapping =
            serde_yaml::from_str(&raw).context("parsing user note frontmatter")?;
        self.extra.extend(fields.into_iter().filter(|(key, _)| {
            !matches!(
                key.as_str(),
                Some(
                    "id" | "created" | "updated" | "tags" | "aliases" | "pinned" | "source" | "why"
                )
            )
        }));
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct BrainFrontmatterYaml {
    id: String,
    created: String,
    updated: String,
    tags: Option<Vec<String>>,
    aliases: Option<Vec<String>>,
    pinned: Option<bool>,
    source: Option<String>,
    why: Option<String>,
    #[serde(flatten)]
    extra: serde_yaml::Mapping,
}

fn quote_yaml_scalar(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn parse_frontmatter_block(content: &str) -> Option<String> {
    let rest = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))?;
    let body_start = content.len() - rest.len();
    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let body_end = body_start + offset;
            return Some(content[body_start..body_end].to_string());
        }
        offset += line.len();
    }
    None
}

fn format_yaml_quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{}\"", value.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ")
}

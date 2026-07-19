//! Slash-command and skill context models shared by the Agent Chat view.

use crate::ai::context_selector::types::SlashCommandPayload;

/// Parse the `description` field from YAML frontmatter in a SKILL.md file.
pub(super) fn parse_skill_description(content: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }
    let end = content[3..].find("---")?;
    let frontmatter = &content[3..3 + end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("description:") {
            let desc = rest.trim().trim_matches('"').trim_matches('\'');
            // Truncate long descriptions for the menu, counting *characters* so a
            // multibyte char straddling byte 77 can't panic `&desc[..77]`. Mirrors
            // the char-based truncation in `SlashCommandEntry::plugin_skill`.
            let desc_chars: Vec<char> = desc.chars().collect();
            if desc_chars.len() > 80 {
                let truncated: String = desc_chars.into_iter().take(77).collect();
                return Some(format!("{truncated}\u{2026}"));
            }
            return Some(desc.to_string());
        }
    }
    None
}

// ── Source-aware slash command model ──────────────────────────────────

/// The origin of a slash command entry discovered during skill enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SlashCommandSource {
    /// A built-in Claude Code command (e.g. `/compact`, `/clear`).
    Default,
    /// A skill owned by a discovered plugin.
    PluginSkill(crate::plugins::PluginSkill),
    /// A user-level Claude Code skill from `~/.scriptkit/.claude/skills/`.
    ClaudeCodeSkill {
        skill_id: String,
        skill_path: std::path::PathBuf,
    },
}

impl SlashCommandSource {
    pub(super) fn owner_label(&self) -> String {
        match self {
            Self::Default => "Built-in".to_string(),
            Self::PluginSkill(skill) => {
                if skill.plugin_title.is_empty() {
                    skill.plugin_id.clone()
                } else {
                    skill.plugin_title.clone()
                }
            }
            Self::ClaudeCodeSkill { .. } => "Claude Code".to_string(),
        }
    }
}

/// A discovered slash command entry with source identity and description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlashCommandEntry {
    /// The bare slash name (e.g. `"compact"`, `"review"`).
    pub name: String,
    /// Human-readable description for the picker.
    pub description: String,
    /// Where this entry came from.
    pub source: SlashCommandSource,
}

impl SlashCommandEntry {
    pub(crate) fn default_command(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            source: SlashCommandSource::Default,
        }
    }

    pub(super) fn plugin_skill(skill: &crate::plugins::PluginSkill) -> Self {
        let plugin_title = if skill.plugin_title.is_empty() {
            skill.plugin_id.clone()
        } else {
            skill.plugin_title.clone()
        };

        let raw_desc = if skill.description.is_empty() {
            format!("Plugin: {}", plugin_title)
        } else {
            format!("{} \u{2014} {}", plugin_title, skill.description)
        };

        let desc_chars: Vec<char> = raw_desc.chars().collect();
        let description = if desc_chars.len() > 80 {
            let truncated: String = desc_chars.into_iter().take(77).collect();
            format!("{truncated}\u{2026}")
        } else {
            raw_desc
        };

        Self {
            name: skill.skill_id.clone(),
            description,
            source: SlashCommandSource::PluginSkill(skill.clone()),
        }
    }

    pub(super) fn claude_code_skill(
        name: String,
        description: String,
        skill_path: std::path::PathBuf,
    ) -> Self {
        Self {
            name: name.clone(),
            description,
            source: SlashCommandSource::ClaudeCodeSkill {
                skill_id: name,
                skill_path,
            },
        }
    }

    /// A key that uniquely identifies this entry across sources.
    pub(crate) fn qualified_key(&self) -> String {
        match &self.source {
            SlashCommandSource::Default => format!("default:{}", self.name),
            SlashCommandSource::PluginSkill(skill) => {
                format!("{}:{}", skill.plugin_id, skill.skill_id)
            }
            SlashCommandSource::ClaudeCodeSkill { skill_id, .. } => {
                format!("claude:{skill_id}")
            }
        }
    }

    /// Convert to a `SlashCommandPayload` for the context selector item kind.
    pub(crate) fn to_payload(&self) -> SlashCommandPayload {
        match &self.source {
            SlashCommandSource::Default => SlashCommandPayload::Default {
                name: self.name.clone(),
            },
            SlashCommandSource::PluginSkill(skill) => {
                SlashCommandPayload::PluginSkill(skill.clone())
            }
            SlashCommandSource::ClaudeCodeSkill {
                skill_id,
                skill_path,
            } => SlashCommandPayload::ClaudeCodeSkill {
                skill_id: skill_id.clone(),
                skill_path: skill_path.clone(),
            },
        }
    }
}

/// Build the staged prompt text for a local skill being accepted from
/// the Agent Chat slash picker or main-menu skill launch.  Both entry paths
/// must produce the same deterministic payload so that the Agent Chat agent
/// receives identical context regardless of how the user invoked the skill.
pub(crate) fn build_staged_skill_prompt(
    skill_title: &str,
    owner_label: &str,
    skill_path: &std::path::Path,
) -> String {
    let skill_content = std::fs::read_to_string(skill_path).unwrap_or_default();
    if owner_label == FLOW_OWNER_LABEL {
        // `-` flow-search staging: same shape as skills, flow-native wording.
        return if skill_content.is_empty() {
            format!("Follow the flow \"{skill_title}\" from the mdflow roster for this session.")
        } else {
            format!(
                "Follow the attached flow \"{skill_title}\" from the mdflow roster for this session.\n\n<flow path=\"{}\">\n{}\n</flow>",
                skill_path.display(),
                skill_content
            )
        };
    }
    let owner_phrase = if owner_label == "Claude Code" {
        format!("from {owner_label}")
    } else {
        format!("from plugin \"{owner_label}\"")
    };
    if skill_content.is_empty() {
        format!("Use the skill \"{skill_title}\" {owner_phrase} for this session.")
    } else {
        format!(
            "Use the attached skill \"{skill_title}\" {owner_phrase} for this session.\n\n<skill path=\"{}\">\n{}\n</skill>",
            skill_path.display(),
            skill_content
        )
    }
}

/// Build the deterministic slash-prefill text for a selected skill.
pub(crate) fn build_skill_slash_command_text(slash_name: &str) -> String {
    format!("/{slash_name} ")
}

/// Build the attached skill context part shared by Agent Chat skill entry paths.
pub(crate) fn build_skill_context_part(
    skill_title: &str,
    owner_label: &str,
    slash_name: &str,
    skill_path: &std::path::Path,
) -> crate::ai::message_parts::AiContextPart {
    crate::ai::message_parts::AiContextPart::SkillFile {
        path: skill_path.to_string_lossy().to_string(),
        label: format!("/{slash_name}"),
        skill_name: skill_title.to_string(),
        owner_label: owner_label.to_string(),
        slash_name: slash_name.to_string(),
    }
}

/// Owner label marking a staged flow (the `-` flow search); switches the
/// staged-prompt wording in `build_staged_skill_prompt` from skill to flow.
pub(crate) const FLOW_OWNER_LABEL: &str = "Flow";

#[cfg(test)]
mod skill_description_tests {
    use super::parse_skill_description;

    #[test]
    fn parse_skill_description_truncates_multibyte_without_panicking() {
        // Regression: `&desc[..77]` byte-sliced a description guarded only by a
        // byte-length check (`desc.len() > 80`). A multibyte char straddling byte
        // 77 panicked ("byte index is not a char boundary").
        let long = "🌊".repeat(100); // 100 chars, 400 bytes
        let content = format!("---\ndescription: {long}\n---\n");
        let out = parse_skill_description(&content).expect("description present");
        assert!(out.ends_with('\u{2026}'));
        assert_eq!(out.chars().count(), 78); // 77 chars + ellipsis
        assert!(!out.contains('\u{fffd}')); // no mangling into replacement chars

        // Long in bytes but short in characters → shown in full, no panic.
        let medium = "é".repeat(41); // 41 chars, 82 bytes
        let content = format!("---\ndescription: {medium}\n---\n");
        let out = parse_skill_description(&content).expect("description present");
        assert_eq!(out, medium);
    }
}

/// Build the attached flow context part for the `-` flow search — skill
/// parity: the composer keeps a compact `-name` token while the submitted
/// prompt carries the full flow markdown.
pub(crate) fn build_flow_context_part(
    flow_title: &str,
    flow_token: &str,
    flow_path: &std::path::Path,
) -> crate::ai::message_parts::AiContextPart {
    let token = flow_token.trim();
    crate::ai::message_parts::AiContextPart::SkillFile {
        path: flow_path.to_string_lossy().to_string(),
        label: token.to_string(),
        skill_name: flow_title.to_string(),
        owner_label: FLOW_OWNER_LABEL.to_string(),
        slash_name: token.trim_start_matches('-').to_string(),
    }
}

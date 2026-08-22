use gpui::SharedString;

pub(crate) const PROFILE_TRIGGER_CHAR: char = '|';
pub(crate) const PROFILE_TRIGGER_STR: &str = "|";

/// Whether selector rows were requested by `@` (mention), `/` (slash command), or `|` (profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextSelectorTrigger {
    Mention,
    Slash,
    Profile,
}

/// Which full built-in view a portal item opens for rich browsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextPortalKind {
    /// Open the Spotlight-powered file search view.
    FileSearch,
    /// Open the recent browser history browser.
    BrowserHistory,
    /// Open the current browser tabs browser.
    BrowserTabs,
    /// Open the visual clipboard history browser.
    ClipboardHistory,
    /// Open the saved dictation history browser.
    DictationHistory,
    /// Open the main launcher filtered to scripts only.
    ScriptSearch,
    /// Open the main launcher filtered to scriptlets only.
    ScriptletSearch,
    /// Open the main launcher filtered to skills only.
    SkillSearch,
    /// Open the notes browser portal.
    NotesBrowse,
    /// Open the Agent Chat conversation history browser (prefiltered by typed query).
    AgentChatHistory,
    /// Open Quick Terminal and attach its transcript back to Agent Chat.
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPortalPrefixPayload {
    pub portal_kind: ContextPortalKind,
    pub prefix: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlinePortalAttachment {
    ResourceUri {
        uri: String,
        label: String,
    },
    FilePath {
        path: String,
        label: String,
    },
    SkillFile {
        path: String,
        label: String,
        skill_name: String,
        owner_label: String,
        slash_name: String,
    },
    TextBlock {
        label: String,
        source: String,
        text: String,
        mime_type: Option<String>,
    },
    FocusedTarget {
        source: String,
        kind: String,
        semantic_id: String,
        label: String,
        metadata: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlinePortalResultPayload {
    pub portal_kind: ContextPortalKind,
    pub attachment: InlinePortalAttachment,
}

/// Source-aware identity for a slash command in the Agent Chat picker.
///
/// Carries enough information to distinguish duplicate slash slugs from
/// different plugins and to stage local skill content on acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommandPayload {
    /// A built-in Claude Code slash command (e.g. `/compact`, `/clear`).
    Default { name: String },
    /// A skill from a discovered plugin.
    PluginSkill(crate::plugins::PluginSkill),
    /// A user-level Claude Code skill from `~/.scriptkit/.claude/skills/`.
    ClaudeCodeSkill {
        skill_id: String,
        skill_path: std::path::PathBuf,
    },
}

impl SlashCommandPayload {
    /// The bare slash name shown in the picker (e.g. `"compact"`, `"review"`).
    pub fn slash_name(&self) -> &str {
        match self {
            Self::Default { name } => name.as_str(),
            Self::PluginSkill(skill) => skill.skill_id.as_str(),
            Self::ClaudeCodeSkill { skill_id, .. } => skill_id.as_str(),
        }
    }

    /// A stable identifier that distinguishes duplicate slash slugs from
    /// different sources (e.g. `"default:compact"`, `"plugin:alpha:review"`).
    pub fn stable_id(&self) -> String {
        match self {
            Self::Default { name } => format!("default:{name}"),
            Self::PluginSkill(skill) => {
                format!("plugin:{}:{}", skill.plugin_id, skill.skill_id)
            }
            Self::ClaudeCodeSkill { skill_id, .. } => format!("claude:{skill_id}"),
        }
    }

    /// Formatted meta string for the selector row, showing the slash name
    /// and owner context. Default commands show just the slash name;
    /// plugin and Claude skills include the owner label.
    pub fn picker_owner_meta(&self) -> String {
        match self {
            Self::Default { name } => format!("/{name}"),
            Self::PluginSkill(skill) => {
                let owner = if skill.plugin_title.is_empty() {
                    skill.plugin_id.as_str()
                } else {
                    skill.plugin_title.as_str()
                };
                format!("/{} \u{b7} {} skill", skill.skill_id, owner)
            }
            Self::ClaudeCodeSkill { skill_id, .. } => {
                format!("/{skill_id} \u{b7} Claude Code skill")
            }
        }
    }

    /// Human-readable owner label for display in the selector meta column.
    pub fn owner_label(&self) -> String {
        match self {
            Self::Default { .. } => "Built-in".to_string(),
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

/// The kind of item in the context selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextSelectorRowKind {
    /// A built-in context attachment (seeded from `context_attachment_specs()`).
    BuiltIn(crate::ai::context_contract::ContextAttachmentKind),
    /// A local file attachment.
    File(std::path::PathBuf),
    /// A local folder attachment.
    Folder(std::path::PathBuf),
    /// A slash command with source-aware identity. Default commands insert
    /// literal `/command` text; plugin and Claude skills stage local content.
    SlashCommand(SlashCommandPayload),
    /// Agent Chat profile row shown from the `|` trigger.
    AgentChatProfile {
        profile_id: String,
        icon_name: Option<String>,
    },
    /// Opens a full built-in view as a portal for rich browsing.
    /// Selection in the portal attaches the result back to the Agent Chat chat.
    Portal(ContextPortalKind),
    /// Inserts a portal prefix such as `@browser-history:` and keeps the
    /// owning selector open for provider-backed results.
    PortalPrefix(ContextPortalPrefixPayload),
    /// A concrete provider-backed result from an inline portal query.
    PortalResult(InlinePortalResultPayload),
    /// A non-actionable placeholder row (loading spinner, empty state).
    /// Acceptance is a no-op; the row exists only for visual feedback.
    Inert,
}

/// A single row in the context selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSelectorRow {
    /// Unique identifier for this row (e.g. `"builtin:selection"`, `"file:/path"`).
    pub id: SharedString,
    /// Display label (e.g. `"Selection"`, `"chat.rs"`).
    pub label: SharedString,
    /// Compact synopsis shown for the focused item.
    pub description: SharedString,
    /// Right-side metadata (slash command, mention, or path).
    pub meta: SharedString,
    /// The kind of item — determines how acceptance creates a context part.
    pub kind: ContextSelectorRowKind,
    /// Relevance score used for deterministic ranking (higher = better match).
    /// Equal section/score ties are broken by canonical stable row identity.
    pub score: u32,
    /// Indices into `label` that matched the query (for gold highlighting).
    pub label_highlight_indices: Vec<usize>,
    /// Indices into `meta` that matched the query (for gold highlighting).
    pub meta_highlight_indices: Vec<usize>,
}

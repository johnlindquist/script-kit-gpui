//! Script and scriptlet type definitions
//!
//! This module contains the core data types for scripts, scriptlets,
//! and search results used throughout the script system.

use std::path::PathBuf;
use std::sync::Arc;

use crate::agents::Agent;
use crate::fallbacks::collector::FallbackItem;
use crate::metadata_parser::TypedMetadata;
use crate::schema_parser::Schema;

/// Represents a script file with its metadata
#[derive(Clone, Debug, Default)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub extension: String,
    pub description: Option<String>,
    /// Icon name from // Icon: metadata (e.g., "File", "Terminal", "Star")
    /// Defaults to "Code" if not specified
    pub icon: Option<String>,
    /// Alias for quick triggering (e.g., "gc" for "git-commit")
    pub alias: Option<String>,
    /// Keyboard shortcut for direct invocation (e.g., "opt i", "cmd shift k")
    pub shortcut: Option<String>,
    /// Typed metadata from `metadata = { ... }` declaration in script
    pub typed_metadata: Option<TypedMetadata>,
    /// Schema definition from `schema = { ... }` declaration in script
    pub schema: Option<Schema>,
    /// Plugin that owns this script (e.g., "main", "cleanshot", "tools")
    pub plugin_id: String,
    /// Human-readable plugin title for display (e.g., "Main", "CleanShot X")
    pub plugin_title: Option<String>,
    /// Kit name extracted from path (e.g., "main", "cleanshot")
    /// Used for grouping scripts by their source kit in the main menu
    pub kit_name: Option<String>,
    /// Full file body text, read once at load time for content search
    pub body: Option<String>,
}

/// Represents a scriptlet parsed from a markdown file
/// Scriptlets are code snippets extracted from .md files with metadata
#[derive(Clone, Debug)]
pub struct Scriptlet {
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub tool: String, // "ts", "bash", "paste", etc.
    pub shortcut: Option<String>,
    pub keyword: Option<String>,
    /// Group name from H1 header (e.g., "Productivity", "Development")
    pub group: Option<String>,
    /// Plugin that owns this scriptlet (e.g., "main", "cleanshot", "tools")
    pub plugin_id: String,
    /// Human-readable plugin title for display (e.g., "Main", "CleanShot X")
    pub plugin_title: Option<String>,
    /// Source file path with anchor for execution (e.g., "/path/to/file.md#slug")
    pub file_path: Option<String>,
    /// Command slug for execution
    pub command: Option<String>,
    /// Alias for quick triggering
    pub alias: Option<String>,
    /// Lucide icon name (kebab-case) from scriptlet `icon:` metadata or bundle frontmatter
    pub icon: Option<String>,
}

impl Scriptlet {
    /// Get a human-readable display name for the tool type
    pub fn tool_display_name(&self) -> &str {
        match self.tool.as_str() {
            "ts" => "TypeScript",
            "js" => "JavaScript",
            "bash" | "sh" => "Shell",
            "zsh" => "Zsh",
            "python" => "Python",
            "ruby" => "Ruby",
            "node" => "Node.js",
            "bun" => "Bun",
            "open" => "Open URL",
            "paste" => "Paste",
            "applescript" => "AppleScript",
            other => other,
        }
    }
}

/// Represents match indices for highlighting matched characters
#[derive(Clone, Debug, Default)]
pub struct MatchIndices {
    /// Indices of matched characters in the name
    pub name_indices: Vec<usize>,
    /// Indices of matched characters in the filename/path
    pub filename_indices: Vec<usize>,
    /// Indices of matched characters in the description
    pub description_indices: Vec<usize>,
}

/// Field that produced the winning match evidence for an active launcher row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchEvidenceField {
    Name,
    Description,
    Filename,
    Content,
    Alias,
    Shortcut,
    Keyword,
    Source,
    Tool,
    WindowApp,
    SkillId,
    PluginTitle,
}

/// Winning search evidence captured during scoring.
///
/// Stored evidence prevents the renderer from recomputing highlights against a
/// different field than the one that admitted and ranked the row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchEvidence {
    pub field: MatchEvidenceField,
    pub text: String,
    pub indices: Vec<usize>,
    pub tier: i32,
    pub score: i32,
}

/// Describes which field produced the winning match for a script
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ScriptMatchKind {
    /// Matched on name (default)
    #[default]
    Name,
    /// Matched on description
    Description,
    /// Matched on filename
    Filename,
    /// Matched on file body content
    Content,
}

/// Content-hit metadata for a script that matched on body text
#[derive(Clone, Debug)]
pub struct ScriptContentMatch {
    /// 1-based line number of the matching line
    pub line_number: usize,
    /// The full text of the matching line (trimmed)
    pub line_text: String,
    /// Character indices within `line_text` that matched the query
    pub line_match_indices: Vec<usize>,
    /// Byte range of the match within the original body text
    pub byte_range: std::ops::Range<usize>,
}

/// Compute a cache-key signature from a content match: (line_number, byte_start, byte_end).
/// Returns None when there is no content match, matching the "no match" cache state.
pub fn preview_match_signature(
    content_match: Option<&ScriptContentMatch>,
) -> Option<(usize, usize, usize)> {
    content_match.map(|cm| (cm.line_number, cm.byte_range.start, cm.byte_range.end))
}

/// Returns true when the preview cache already holds valid highlighted lines for the
/// requested script path and content-match signature. A miss forces a re-read + re-highlight.
pub fn preview_cache_is_valid(
    cached_path: Option<&str>,
    cached_match_signature: Option<(usize, usize, usize)>,
    cached_lines_empty: bool,
    requested_path: &str,
    content_match: Option<&ScriptContentMatch>,
) -> bool {
    cached_path == Some(requested_path)
        && cached_match_signature == preview_match_signature(content_match)
        && !cached_lines_empty
}

/// Represents a scored match result for fuzzy search
/// Uses Arc<Script> for cheap cloning during filter operations (H1 optimization)
#[derive(Clone, Debug)]
pub struct ScriptMatch {
    pub script: Arc<Script>,
    pub score: i32,
    /// The filename used for matching (e.g., "my-script.ts")
    pub filename: String,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
    /// Which field produced the dominant match
    pub match_kind: ScriptMatchKind,
    /// Content-hit metadata when match_kind == Content
    pub content_match: Option<ScriptContentMatch>,
    /// Winning field evidence captured during scoring.
    pub match_evidence: Option<MatchEvidence>,
}

/// Represents a scored match result for fuzzy search on scriptlets
/// Uses Arc<Scriptlet> for cheap cloning during filter operations (H1 optimization)
#[derive(Clone, Debug)]
pub struct ScriptletMatch {
    pub scriptlet: Arc<Scriptlet>,
    pub score: i32,
    /// The display file path with anchor for matching (e.g., "url.md#open-github")
    pub display_file_path: Option<String>,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
    /// Winning field evidence captured during scoring.
    pub match_evidence: Option<MatchEvidence>,
}

/// Represents a scored match result for fuzzy search on built-in entries
#[derive(Clone, Debug)]
pub struct BuiltInMatch {
    pub entry: crate::builtins::BuiltInEntry,
    pub score: i32,
    /// Winning field evidence captured during scoring.
    pub match_evidence: Option<MatchEvidence>,
}

/// Represents a scored match result for fuzzy search on applications
#[derive(Clone, Debug)]
pub struct AppMatch {
    pub app: crate::app_launcher::AppInfo,
    pub score: i32,
    /// Winning field evidence captured during scoring.
    pub match_evidence: Option<MatchEvidence>,
}

/// Represents a scored match result for fuzzy search on windows
#[derive(Clone, Debug)]
pub struct WindowMatch {
    pub window: crate::window_control::WindowInfo,
    pub app_icon: Option<crate::app_launcher::DecodedIcon>,
    pub subtitle: String,
    pub score: i32,
    /// Winning field evidence captured during scoring.
    pub match_evidence: Option<MatchEvidence>,
}

/// Root/unified Windows row enriched by the app layer for rendering.
#[derive(Clone, Debug)]
pub struct RootWindowEntry {
    pub window: crate::window_control::WindowInfo,
    pub app_icon: Option<crate::app_launcher::DecodedIcon>,
    pub subtitle: String,
    pub duplicate_rank: Option<usize>,
    pub duplicate_count: usize,
    pub local_recency_seq: Option<u64>,
}

/// Represents a scored match result for a plugin-owned skill.
/// Skills always open Agent Chat when selected from the main menu.
#[derive(Clone, Debug)]
pub struct SkillMatch {
    pub skill: Arc<crate::plugins::PluginSkill>,
    pub score: i32,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
    /// Winning field evidence captured during scoring.
    pub match_evidence: Option<MatchEvidence>,
}

/// Represents a scored match result for an mdflow flow.
/// Flows are the primary launcher experience: selecting one starts a
/// Threadline conversation with that flow (never a raw run).
#[derive(Clone, Debug)]
pub struct FlowMatch {
    /// Flow identity for flow rows (identity rows and flow conversations).
    /// `None` for detached conversations (Agent Chat, Quick AI), which have
    /// no flow descriptor. Invalid pairings are prevented by construction:
    /// only `prepend_root_conversations_section` builds detached rows.
    pub flow: Option<crate::flows::model::FlowDescriptor>,
    /// What Enter targets — typed, never a fake optional id.
    pub target: ConversationRowTarget,
    /// Precomputed `friendly_name()` so `name()` can return a borrow.
    pub display_name: String,
    /// Precomputed "purpose · engine · origin" row subtitle.
    pub subtitle: String,
    pub score: i32,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
}

impl FlowMatch {
    /// The backgrounded conversation this row resumes, if it is a
    /// conversation row (rather than a flow identity row).
    pub fn conversation_id(&self) -> Option<&crate::ai::conversations::ConversationSessionId> {
        match &self.target {
            ConversationRowTarget::Conversation(id) => Some(id),
            ConversationRowTarget::FlowIdentity { .. } => None,
        }
    }

    /// Flow session id when this row resumes a flow conversation.
    pub fn flow_session_id(&self) -> Option<u64> {
        match &self.target {
            ConversationRowTarget::Conversation(
                crate::ai::conversations::ConversationSessionId::Flow(
                    crate::ai::conversations::FlowSessionId(id),
                ),
            ) => Some(*id),
            _ => None,
        }
    }
}

/// Enter/click target for a `SearchResult::Flow` row.
///
/// Exhaustive by construction: a row is either a flow IDENTITY (Enter
/// starts/resumes that flow's conversation) or a backgrounded CONVERSATION
/// (Enter resumes the exact retained entity in the
/// `BackgroundedSessionStore`, dispatched by its tagged session id). There
/// is no unrepresentable "identity with a session id" state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversationRowTarget {
    FlowIdentity { flow_id: String },
    Conversation(crate::ai::conversations::ConversationSessionId),
}

/// Represents a scored match result for fuzzy search on agents
/// Uses Arc<Agent> for cheap cloning during filter operations
#[derive(Clone, Debug)]
pub struct AgentMatch {
    pub agent: Arc<Agent>,
    pub score: i32,
    /// The display name for matching
    pub display_name: String,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
}

/// Synthetic launcher row summarizing script validation failures.
///
/// Pinned to the top of the launcher when one or more scripts are excluded
/// from the catalog due to binding collisions or other fatal validation
/// issues. Opening it routes to a read-only diagnostic view.
#[derive(Clone, Debug)]
pub struct ScriptIssueMatch {
    pub title: String,
    pub description: Option<String>,
    pub failed_count: usize,
    pub fatal_count: usize,
    pub warning_count: usize,
    pub score: i32,
}

/// Represents a fallback command match for the "Use with..." section
///
/// Fallbacks are always shown at the bottom of search results when there's a filter query.
/// They provide Raycast-style actions like "Search Google", "Copy to Clipboard", etc.
#[derive(Clone, Debug)]
pub struct FallbackMatch {
    /// The fallback item (either built-in or script fallback)
    pub fallback: FallbackItem,
    /// Score is always 0 for fallbacks (they sort by priority, not score)
    pub score: i32,
    /// Optional row title override for synthetic fallback placements.
    pub title_override: Option<String>,
    /// Optional row description override for synthetic fallback placements.
    pub description_override: Option<String>,
    /// Optional stable selection identity for synthetic fallback placements.
    pub stable_selection_key_override: Option<String>,
}

impl FallbackMatch {
    pub fn new(fallback: FallbackItem, score: i32) -> Self {
        Self {
            fallback,
            score,
            title_override: None,
            description_override: None,
            stable_selection_key_override: None,
        }
    }

    pub fn with_display_overrides(
        mut self,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.title_override = Some(title.into());
        self.description_override = Some(description.into());
        self
    }

    pub fn with_stable_selection_key(mut self, key: impl Into<String>) -> Self {
        self.stable_selection_key_override = Some(key.into());
        self
    }

    pub fn display_label(&self) -> String {
        self.title_override
            .clone()
            .unwrap_or_else(|| self.fallback.display_label())
    }

    pub fn display_name(&self) -> String {
        self.title_override
            .clone()
            .unwrap_or_else(|| self.fallback.display_name())
    }

    pub fn display_description(&self) -> String {
        self.description_override
            .clone()
            .unwrap_or_else(|| self.fallback.display_description())
    }
}

/// Represents a scored local file result surfaced in root launcher search.
#[derive(Clone, Debug)]
pub struct FileMatch {
    pub file: crate::file_search::FileResult,
    pub score: i32,
}

/// Represents a passive root-search match for a local Note.
#[derive(Clone, Debug)]
pub struct NoteMatch {
    pub(crate) hit: crate::notes::RootNoteSearchHit,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Represents a passive root-search match for a local brain memory.
#[derive(Clone, Debug)]
pub struct BrainMatch {
    pub(crate) hit: crate::brain::RootBrainSearchHit,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Represents an open "Brain Inbox" item pinned at the top of the empty
/// root query (curator-filed commitments, questions, drift, stale pins).
#[derive(Clone, Debug)]
pub struct BrainInboxMatch {
    pub(crate) item: crate::brain::InboxItem,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Represents a passive root-search match for a captured todo.
#[derive(Clone, Debug)]
pub struct TodoMatch {
    pub(crate) hit: crate::menu_syntax::RootTodoSearchHit,
    pub(crate) score: i32,
}

/// Represents a passive root-search match for a saved Agent Chat conversation.
#[derive(Clone, Debug)]
pub struct AgentChatHistoryMatch {
    pub(crate) entry: crate::ai::agent_chat::ui::history::AgentChatHistoryEntry,
    pub(crate) score: i32,
    pub(crate) matched_field: crate::ai::agent_chat::ui::history::AgentChatHistorySearchField,
    pub(crate) subtitle: String,
    /// Qualification-time match evidence; highlights come from here, never
    /// from render-time fuzzy re-derivation.
    pub(crate) evidence: Option<crate::scripts::search::sentence::LongTextMatchEvidence>,
}

/// Represents a passive root-search match for cmux AI Vault metadata.
#[derive(Clone, Debug)]
pub struct AiVaultMatch {
    pub(crate) hit: crate::ai_vault::AiVaultHit,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Represents a passive root-search match for recent clipboard metadata.
#[derive(Clone, Debug)]
pub struct ClipboardHistoryMatch {
    pub(crate) entry: crate::clipboard_history::ClipboardEntryMeta,
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Represents a passive root-search match for a saved dictation transcript.
#[derive(Clone, Debug)]
pub struct DictationHistoryMatch {
    pub(crate) id: String,
    pub(crate) preview: String,
    pub(crate) target: String,
    pub(crate) timestamp: String,
    pub(crate) audio_duration_ms: u64,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
    pub(crate) matched_field: crate::dictation::DictationHistorySearchField,
    /// Qualification-time match evidence; highlights come from here, never
    /// from render-time fuzzy re-derivation.
    pub(crate) evidence: Option<crate::scripts::search::sentence::LongTextMatchEvidence>,
}

/// Represents a passive root-search match for local browser history metadata.
#[derive(Clone, Debug)]
pub struct BrowserHistoryMatch {
    pub(crate) hit: crate::browser_history::RootBrowserHistorySearchHit,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Represents a passive root-search match for an already open browser tab.
#[derive(Clone, Debug)]
pub struct BrowserTabMatch {
    pub(crate) hit: crate::browser_tabs::RootBrowserTabSearchHit,
    pub(crate) subtitle: String,
    pub(crate) score: i32,
}

/// Unified search result that can be a Script, Scriptlet, Skill, BuiltIn, App, Window, File, Agent, or Fallback
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SearchResult {
    Script(ScriptMatch),
    Scriptlet(ScriptletMatch),
    /// mdflow flow — the primary launcher row kind; Enter starts a
    /// Threadline conversation with the flow.
    Flow(FlowMatch),
    /// Plugin-owned skill that always opens Agent Chat when selected
    Skill(SkillMatch),
    BuiltIn(BuiltInMatch),
    App(AppMatch),
    Window(WindowMatch),
    /// Local file result appended from the root launcher file-search source.
    File(FileMatch),
    /// Local Note surfaced as a passive root-search source.
    Note(NoteMatch),
    /// Local brain memory surfaced as a passive root-search source.
    BrainHit(BrainMatch),
    /// Open brain-inbox item pinned at the top of the empty root query.
    BrainInboxItem(BrainInboxMatch),
    /// Captured todo surfaced as a root-search source.
    Todo(TodoMatch),
    /// Saved Agent Chat conversation surfaced as a passive root-search source.
    AgentChatHistory(AgentChatHistoryMatch),
    /// cmux AI Vault session metadata surfaced as a passive root-search source.
    AiVault(AiVaultMatch),
    /// Recent clipboard metadata surfaced as a passive root-search source.
    ClipboardHistory(ClipboardHistoryMatch),
    /// Saved dictation transcripts surfaced as a passive root-search source.
    DictationHistory(DictationHistoryMatch),
    /// Open browser tab metadata surfaced as a passive root-search source.
    BrowserTab(BrowserTabMatch),
    /// Local browser history metadata surfaced as a passive root-search source.
    BrowserHistory(BrowserHistoryMatch),
    /// Legacy agent artifact — suppressed from the launcher pipeline.
    /// Agent results are actively filtered out of search/grouping/selection.
    /// Agent Chat agent catalog and provider selection remain intact in `src/ai/agent_chat/ui/`.
    /// Kept as a variant for compilation compatibility; never yielded to the UI.
    Agent(AgentMatch),
    /// Fallback command from "Use with..." section (shown at bottom of search results)
    Fallback(FallbackMatch),
    /// Synthetic row summarizing script validation failures, pinned at the top
    /// of the launcher so authors see "my script vanished" repairs inline.
    ScriptIssue(ScriptIssueMatch),
    /// In-place Spine prompt-builder row projected into the main list.
    SpineProjection(crate::spine::SpineListRow),
}

impl SearchResult {
    /// Returns true if this result is a legacy Agent variant that should be
    /// suppressed from the launcher pipeline.
    pub fn is_suppressed_agent(&self) -> bool {
        matches!(self, SearchResult::Agent(_))
    }

    /// Return the declarative root-search source represented by this row.
    ///
    /// `type:` continues to filter by row kind; this source mapping powers
    /// valueless source heads such as `apps:`, `scripts:`, and `files:`.
    pub fn root_unified_source(&self) -> Option<crate::menu_syntax::RootUnifiedSourceFilter> {
        use crate::menu_syntax::RootUnifiedSourceFilter;

        match self {
            SearchResult::Script(_) | SearchResult::Scriptlet(_) => {
                Some(RootUnifiedSourceFilter::Scripts)
            }
            SearchResult::BuiltIn(_)
            | SearchResult::Flow(_)
            | SearchResult::Skill(_)
            | SearchResult::ScriptIssue(_) => Some(RootUnifiedSourceFilter::Commands),
            SearchResult::App(_) => Some(RootUnifiedSourceFilter::Apps),
            SearchResult::Window(_) => Some(RootUnifiedSourceFilter::Windows),
            SearchResult::File(_) => Some(RootUnifiedSourceFilter::Files),
            SearchResult::Note(_) => Some(RootUnifiedSourceFilter::Notes),
            SearchResult::BrainHit(_) | SearchResult::BrainInboxItem(_) => {
                Some(RootUnifiedSourceFilter::Brain)
            }
            SearchResult::Todo(_) => Some(RootUnifiedSourceFilter::Todo),
            SearchResult::AgentChatHistory(_) => Some(RootUnifiedSourceFilter::Conversations),
            SearchResult::AiVault(_) => Some(RootUnifiedSourceFilter::AiVault),
            SearchResult::ClipboardHistory(_) => Some(RootUnifiedSourceFilter::ClipboardHistory),
            SearchResult::DictationHistory(_) => Some(RootUnifiedSourceFilter::Dictation),
            SearchResult::BrowserTab(_) => Some(RootUnifiedSourceFilter::BrowserTabs),
            SearchResult::BrowserHistory(_) => Some(RootUnifiedSourceFilter::BrowserHistory),
            SearchResult::Agent(_) | SearchResult::Fallback(_) => None,
            SearchResult::SpineProjection(_) => None,
        }
    }
}

impl SearchResult {
    /// Get the display name for this result
    pub fn name(&self) -> &str {
        match self {
            SearchResult::Script(sm) => &sm.script.name,
            SearchResult::Scriptlet(sm) => &sm.scriptlet.name,
            SearchResult::Flow(fm) => &fm.display_name,
            SearchResult::Skill(sm) => &sm.skill.title,
            SearchResult::BuiltIn(bm) => &bm.entry.name,
            SearchResult::App(am) => &am.app.name,
            SearchResult::Window(wm) => &wm.window.title,
            SearchResult::File(fm) => &fm.file.name,
            SearchResult::Note(nm) => &nm.title,
            SearchResult::BrainHit(bm) => &bm.hit.title,
            SearchResult::BrainInboxItem(bm) => &bm.item.title,
            SearchResult::Todo(tm) => &tm.hit.title,
            SearchResult::AgentChatHistory(am) => am.entry.title_display(),
            SearchResult::AiVault(am) => &am.hit.safe_title,
            SearchResult::ClipboardHistory(cm) => &cm.title,
            SearchResult::DictationHistory(dm) => &dm.preview,
            SearchResult::BrowserTab(bm) => &bm.hit.title,
            SearchResult::BrowserHistory(bm) => &bm.hit.title,
            SearchResult::Agent(am) => &am.agent.name,
            SearchResult::Fallback(fm) => fm
                .title_override
                .as_deref()
                .unwrap_or_else(|| fm.fallback.name()),
            SearchResult::ScriptIssue(issue) => &issue.title,
            SearchResult::SpineProjection(row) => row.title.as_ref(),
        }
    }

    /// Get the description for this result
    pub fn description(&self) -> Option<&str> {
        match self {
            SearchResult::Script(sm) => sm.script.description.as_deref(),
            SearchResult::Scriptlet(sm) => sm.scriptlet.description.as_deref(),
            SearchResult::Flow(fm) => Some(fm.subtitle.as_str()),
            SearchResult::Skill(sm) => {
                if sm.skill.description.is_empty() {
                    None
                } else {
                    Some(&sm.skill.description)
                }
            }
            SearchResult::BuiltIn(bm) => Some(&bm.entry.description),
            SearchResult::App(am) => am.app.path.to_str(),
            SearchResult::Window(wm) => Some(wm.subtitle.as_str()),
            SearchResult::File(fm) => Some(fm.file.path.as_str()),
            SearchResult::Note(nm) => Some(nm.subtitle.as_str()),
            SearchResult::BrainHit(bm) => Some(bm.subtitle.as_str()),
            SearchResult::BrainInboxItem(bm) => Some(bm.subtitle.as_str()),
            SearchResult::Todo(tm) => Some(tm.hit.subtitle.as_str()),
            SearchResult::AgentChatHistory(am) => Some(am.subtitle.as_str()),
            SearchResult::AiVault(am) => Some(am.subtitle.as_str()),
            SearchResult::ClipboardHistory(cm) => Some(cm.subtitle.as_str()),
            SearchResult::DictationHistory(dm) => Some(dm.subtitle.as_str()),
            SearchResult::BrowserTab(bm) => Some(bm.subtitle.as_str()),
            SearchResult::BrowserHistory(bm) => Some(bm.subtitle.as_str()),
            SearchResult::Agent(am) => am.agent.description.as_deref(),
            SearchResult::Fallback(fm) => fm
                .description_override
                .as_deref()
                .or_else(|| Some(fm.fallback.description())),
            SearchResult::ScriptIssue(issue) => issue.description.as_deref(),
            SearchResult::SpineProjection(row) => row.subtitle.as_ref().map(|s| s.as_ref()),
        }
    }

    /// Get the score for this result
    pub fn score(&self) -> i32 {
        match self {
            SearchResult::Script(sm) => sm.score,
            SearchResult::Scriptlet(sm) => sm.score,
            SearchResult::Flow(fm) => fm.score,
            SearchResult::Skill(sm) => sm.score,
            SearchResult::BuiltIn(bm) => bm.score,
            SearchResult::App(am) => am.score,
            SearchResult::Window(wm) => wm.score,
            SearchResult::File(fm) => fm.score,
            SearchResult::Note(nm) => nm.score,
            SearchResult::BrainHit(bm) => bm.score,
            SearchResult::BrainInboxItem(bm) => bm.score,
            SearchResult::Todo(tm) => tm.score,
            SearchResult::AgentChatHistory(am) => am.score,
            SearchResult::AiVault(am) => am.score,
            SearchResult::ClipboardHistory(cm) => cm.score,
            SearchResult::DictationHistory(dm) => dm.score,
            SearchResult::BrowserTab(bm) => bm.score,
            SearchResult::BrowserHistory(bm) => bm.score,
            SearchResult::Agent(am) => am.score,
            SearchResult::Fallback(fm) => fm.score,
            SearchResult::ScriptIssue(issue) => issue.score,
            SearchResult::SpineProjection(row) => row.score,
        }
    }

    /// Relevance tier encoded into active launcher search scores.
    ///
    /// Raw scores can still carry within-tier quality, but sorting should compare
    /// this first so usage memory does not lift weak hidden-field matches above
    /// visible primary-name matches.
    pub fn match_tier(&self) -> i32 {
        crate::scripts::search::match_tier_from_score(self.score())
    }

    /// Get the type label for UI display
    pub fn type_label(&self) -> &'static str {
        match self {
            SearchResult::Script(_) => "Script",
            SearchResult::Scriptlet(_) => "Snippet",
            SearchResult::Flow(_) => "Flow",
            SearchResult::Skill(_) => "Skill",
            SearchResult::BuiltIn(_) => "Built-in",
            SearchResult::App(_) => "App",
            SearchResult::Window(_) => "Window",
            SearchResult::File(_) => "File",
            SearchResult::Note(_) => "Note",
            SearchResult::BrainHit(_) => "Brain Memory",
            SearchResult::BrainInboxItem(_) => "Brain Inbox",
            SearchResult::Todo(_) => "Todo",
            SearchResult::AgentChatHistory(_) => "Agent Chat Conversation",
            SearchResult::AiVault(_) => "Vault Conversation",
            SearchResult::ClipboardHistory(_) => "Clipboard",
            SearchResult::DictationHistory(_) => "Dictation",
            SearchResult::BrowserTab(_) => "Browser Tab",
            SearchResult::BrowserHistory(_) => "Browser History",
            SearchResult::Agent(_) => "Agent",
            SearchResult::Fallback(_) => "Fallback",
            SearchResult::ScriptIssue(_) => "Issues",
            SearchResult::SpineProjection(row) => row.kind.type_label(),
        }
    }

    /// Returns a plugin-qualified launcher command ID for alias/shortcut persistence.
    ///
    /// Format: `{category}/{plugin_id}:{name}` for scripts/scriptlets so that
    /// two plugins exposing the same script name produce distinct IDs.
    /// Built-ins, apps, and fallbacks keep their existing single-segment identifier.
    /// Skills and windows return `None` (non-bindable).
    pub fn launcher_command_id(&self) -> Option<String> {
        match self {
            SearchResult::Script(sm) => Some(sm.script.launcher_command_id()),
            SearchResult::Scriptlet(sm) => Some(sm.scriptlet.launcher_command_id()),
            SearchResult::BuiltIn(bm) => Some(bm.entry.id.clone()),
            SearchResult::App(am) => Some(
                am.app
                    .bundle_id
                    .as_ref()
                    .map(|bundle_id| format!("app/{bundle_id}"))
                    .unwrap_or_else(|| {
                        format!("app/{}", am.app.name.to_lowercase().replace(' ', "-"))
                    }),
            ),
            SearchResult::File(fm) => Some(format!("file/{}", fm.file.path)),
            SearchResult::Note(_) => None,
            SearchResult::BrainHit(_) => None,
            SearchResult::BrainInboxItem(_) => None,
            SearchResult::Todo(_) => None,
            SearchResult::AgentChatHistory(_) => None,
            SearchResult::AiVault(_) => None,
            SearchResult::ClipboardHistory(_) => None,
            SearchResult::DictationHistory(_) => None,
            SearchResult::BrowserTab(_) => None,
            SearchResult::BrowserHistory(_) => None,
            SearchResult::Window(_)
            | SearchResult::Flow(_)
            | SearchResult::Skill(_)
            | SearchResult::Agent(_) => None,
            SearchResult::Fallback(fm) => Some(format!("fallback/{}", fm.fallback.name())),
            SearchResult::ScriptIssue(_) => None,
            SearchResult::SpineProjection(_) => None,
        }
    }

    /// Returns the stable key used by exact-query launcher memory.
    ///
    /// This is broader than `launcher_command_id()` because query memory also
    /// needs to remember non-bindable items like skills and windows.
    pub fn history_result_key(&self) -> Option<String> {
        match self {
            // Conversation rows key by the TAGGED session id so selection
            // follows the session across recency reorders; the historical
            // `flow-session:{id}` prefix is kept for flow conversations so
            // existing exact-query memory entries stay valid.
            SearchResult::Flow(fm) => Some(match &fm.target {
                ConversationRowTarget::Conversation(id) => match id {
                    crate::ai::conversations::ConversationSessionId::Flow(
                        crate::ai::conversations::FlowSessionId(session_id),
                    ) => format!("flow-session:{session_id}"),
                    other => format!("conversation:{}", other.automation_id()),
                },
                ConversationRowTarget::FlowIdentity { flow_id } => format!("flow:{flow_id}"),
            }),
            SearchResult::Skill(sm) => Some(format!(
                "skill:{}:{}",
                sm.skill.plugin_id, sm.skill.skill_id
            )),
            SearchResult::Window(wm) => Some(wm.window.selection_key()),
            SearchResult::AgentChatHistory(am) => {
                Some(format!("agent_chat-history/{}", am.entry.session_id))
            }
            SearchResult::AiVault(am) => Some(am.hit.stable_key.clone()),
            SearchResult::Note(nm) => Some(format!("note/{}", nm.hit.id.as_str())),
            SearchResult::BrainHit(bm) => Some(format!(
                "brain/{}/{}",
                bm.hit.source.as_str(),
                bm.hit.source_id
            )),
            SearchResult::BrainInboxItem(bm) => Some(format!("brain-inbox/{}", bm.item.id)),
            SearchResult::Todo(tm) => Some(tm.hit.stable_key.clone()),
            SearchResult::ClipboardHistory(cm) => {
                Some(format!("clipboard-history/{}", cm.entry.id))
            }
            SearchResult::DictationHistory(dm) => Some(format!("dictation-history/{}", dm.id)),
            SearchResult::BrowserTab(_) => None,
            SearchResult::BrowserHistory(bm) => Some(bm.hit.stable_key.clone()),
            SearchResult::Fallback(_) | SearchResult::Agent(_) => None,
            SearchResult::ScriptIssue(_) => None,
            _ => self.launcher_command_id(),
        }
    }

    /// Returns the stable identity used to preserve and execute a visible
    /// launcher selection across passive root-search updates.
    ///
    /// This is intentionally separate from `history_result_key()`: selection
    /// stability must cover rows such as fallbacks and agents without teaching
    /// input history to promote those rows on future exact-query recall.
    pub fn stable_selection_key(&self) -> Option<String> {
        match self {
            SearchResult::Fallback(fm) => fm
                .stable_selection_key_override
                .clone()
                .or_else(|| Some(format!("fallback/{}", fm.fallback.name()))),
            SearchResult::Agent(am) => Some(format!("agent/{}", am.agent.path.display())),
            SearchResult::BrowserTab(bm) => Some(bm.hit.stable_key.clone()),
            SearchResult::ScriptIssue(issue) => Some(format!(
                "script-issue/{}:{}:{}:{}",
                issue.title, issue.failed_count, issue.fatal_count, issue.warning_count
            )),
            SearchResult::SpineProjection(row) => Some(row.id.to_string()),
            _ => self
                .history_result_key()
                .or_else(|| self.launcher_command_id()),
        }
    }

    /// Returns the display name for this result, used alongside `launcher_command_id`
    /// for shortcut recorder / alias input labels.
    pub fn launcher_command_name(&self) -> String {
        self.name().to_string()
    }

    /// Semantic type accessory for search-mode list rows.
    /// Returns (tooltip/accessibility label, Lucide icon hint name).
    pub fn type_accessory_info(&self) -> (&'static str, &'static str) {
        match self {
            SearchResult::Script(_) => ("Script", "file-code"),
            SearchResult::Scriptlet(_) => ("Snippet", "scroll-text"),
            SearchResult::Flow(fm) => match fm.target {
                ConversationRowTarget::Conversation(_) => ("Conversation", "message-circle"),
                ConversationRowTarget::FlowIdentity { .. } => ("Flow", "bot"),
            },
            SearchResult::Skill(_) => ("Skill", "workflow"),
            SearchResult::BuiltIn(_) => ("Command", "command"),
            SearchResult::App(_) => ("App", "package"),
            SearchResult::Window(_) => ("Window", "panel-top"),
            SearchResult::File(_) => ("File", "file"),
            SearchResult::Note(_) => ("Note", "notebook-text"),
            SearchResult::BrainHit(_) => ("Brain", "brain"),
            SearchResult::BrainInboxItem(_) => ("Brain Inbox", "inbox"),
            SearchResult::Todo(_) => ("Todo", "list-todo"),
            SearchResult::AgentChatHistory(_) => ("Agent Chat", "message-circle"),
            SearchResult::AiVault(_) => ("Vault", "vault"),
            SearchResult::ClipboardHistory(_) => ("Clipboard", "clipboard"),
            SearchResult::DictationHistory(_) => ("Dictation", "mic"),
            SearchResult::BrowserTab(_) => ("Tab", "panel-top"),
            SearchResult::BrowserHistory(_) => ("Web", "globe"),
            SearchResult::Agent(_) => ("Agent", "bot"),
            SearchResult::Fallback(_) => ("Fallback", "zap"),
            SearchResult::ScriptIssue(_) => ("Issues", "triangle-alert"),
            SearchResult::SpineProjection(row) => row.kind.type_accessory_info(),
        }
    }

    /// Get the plugin/source name for this result (used during search to show origin).
    ///
    /// Resolves to the owning plugin title (or id) for scripts, scriptlets, and skills.
    /// Returns None for items without a meaningful source (built-ins, apps, etc.)
    pub fn source_name(&self) -> Option<&str> {
        match self {
            SearchResult::Script(sm) => {
                sm.script
                    .plugin_title
                    .as_deref()
                    .or(if sm.script.plugin_id.is_empty() {
                        sm.script.kit_name.as_deref()
                    } else {
                        Some(sm.script.plugin_id.as_str())
                    })
            }
            SearchResult::Scriptlet(sm) => {
                sm.scriptlet
                    .plugin_title
                    .as_deref()
                    .or(if sm.scriptlet.plugin_id.is_empty() {
                        sm.scriptlet.group.as_deref()
                    } else {
                        Some(sm.scriptlet.plugin_id.as_str())
                    })
            }
            SearchResult::Flow(fm) => match fm.target {
                ConversationRowTarget::Conversation(_) => Some("Conversations"),
                ConversationRowTarget::FlowIdentity { .. } => Some("Flows"),
            },
            SearchResult::Skill(sm) => {
                if sm.skill.plugin_title.is_empty() {
                    Some(&sm.skill.plugin_id)
                } else {
                    Some(&sm.skill.plugin_title)
                }
            }
            SearchResult::Agent(am) => am.agent.kit.as_deref(),
            SearchResult::File(_) => Some("Files"),
            SearchResult::Note(_) => Some("Notes"),
            SearchResult::BrainHit(_) => Some("From Your Brain"),
            SearchResult::BrainInboxItem(_) => Some("Brain Inbox"),
            SearchResult::Todo(_) => Some("Todos"),
            SearchResult::AgentChatHistory(_) => Some("Agent Chat Conversations"),
            SearchResult::AiVault(_) => Some("AI Vault"),
            SearchResult::ClipboardHistory(_) => Some("Clipboard History"),
            SearchResult::DictationHistory(_) => Some("Dictation History"),
            SearchResult::BrowserTab(_) => Some("Browser Tabs"),
            SearchResult::BrowserHistory(_) => Some("Browser History"),
            SearchResult::Window(_) => Some("Windows"),
            SearchResult::ScriptIssue(_) => None,
            SearchResult::SpineProjection(_) => Some("Spine"),
            _ => None,
        }
    }

    /// Get the default action text for the primary button.
    ///
    /// Priority:
    /// 1. If the item has a custom `enter` text in typed metadata, use that
    /// 2. Otherwise, return type-based fallback text:
    ///    - Scripts → "Run Script"
    ///    - Built-ins → action-specific labels like "Open Settings"
    ///    - Scriptlets/Snippets → "Paste Snippet"
    ///    - Skills → "Open Skill"
    ///    - Apps → "Launch App"
    ///    - Windows → "Switch to Window"
    ///    - Agents → "Run Agent"
    ///    - Fallbacks → "Run"
    ///
    /// This method is used by both the footer button text and execute_selected().
    pub fn get_default_action_text(&self) -> &str {
        match self {
            SearchResult::Script(sm) => {
                // Check for custom enter text in typed metadata
                if let Some(ref typed_meta) = sm.script.typed_metadata {
                    if let Some(ref enter) = typed_meta.enter {
                        return enter.as_str();
                    }
                }
                "Run Script"
            }
            SearchResult::Scriptlet(sm) => {
                // Scriptlets can also have typed metadata with custom enter text
                // For now, use tool-based fallback
                match sm.scriptlet.tool.as_str() {
                    "paste" | "snippet" => "Paste Snippet",
                    "bash" | "sh" | "zsh" => "Run Command",
                    _ => "Run Snippet",
                }
            }
            SearchResult::Flow(fm) => match fm.target {
                ConversationRowTarget::Conversation(_) => "Resume",
                ConversationRowTarget::FlowIdentity { .. } => "Converse",
            },
            SearchResult::Skill(_) => "Open Skill",
            SearchResult::BuiltIn(bm) => bm.entry.default_action_text(),
            SearchResult::App(_) => "Launch App",
            SearchResult::Window(_) => "Switch to Window",
            SearchResult::File(fm) => {
                if fm.file.file_type == crate::file_search::FileType::Directory {
                    "Open Folder"
                } else {
                    "Open File"
                }
            }
            SearchResult::Note(_) => "Open Note",
            SearchResult::BrainHit(bm) => match bm.hit.source {
                crate::brain::DocSource::Note => "Open Note",
                crate::brain::DocSource::ChatTurn => "Resume Conversation",
                crate::brain::DocSource::Clipboard
                | crate::brain::DocSource::Activity
                | crate::brain::DocSource::Capture
                | crate::brain::DocSource::DayPage
                | crate::brain::DocSource::Fragment => "Ask Your Brain",
            },
            // Inbox sources use the DocSource string vocabulary; mirror the
            // BrainHit routing (note → editor, chat turn → resume, else ask).
            SearchResult::BrainInboxItem(bm) => match bm.item.source.as_str() {
                "note" => "Open Note",
                "chat_turn" => "Resume Conversation",
                _ => "Ask Your Brain",
            },
            SearchResult::Todo(_) => "Copy Todo",
            SearchResult::AgentChatHistory(_) => "Resume Conversation",
            SearchResult::AiVault(_) => "Paste Resume Command",
            SearchResult::ClipboardHistory(_) => "Paste Clipboard",
            SearchResult::DictationHistory(_) => "Paste Dictation",
            SearchResult::BrowserTab(_) => "Switch to Tab",
            SearchResult::BrowserHistory(_) => "Open Page",
            SearchResult::Agent(_) => {
                // Suppressed: agents are not launchable from the main menu
                "Agent (suppressed)"
            }
            SearchResult::Fallback(fm) => {
                // Fallbacks use their action type
                if fm.fallback.is_builtin() {
                    "Run"
                } else {
                    "Run Script"
                }
            }
            SearchResult::ScriptIssue(_) => "Inspect Issues",
            SearchResult::SpineProjection(row) => row.default_action_text(),
        }
    }
}

impl Script {
    /// Returns the plugin-qualified command ID used by launcher config entries.
    pub fn launcher_command_id(&self) -> String {
        let owner = if self.plugin_id.is_empty() {
            self.kit_name.as_deref().unwrap_or("main")
        } else {
            self.plugin_id.as_str()
        };
        format!("script/{}:{}", owner, self.name)
    }
}

impl Scriptlet {
    /// Returns the plugin-qualified command ID used by launcher config entries.
    pub fn launcher_command_id(&self) -> String {
        let owner = if self.plugin_id.is_empty() {
            self.group.as_deref().unwrap_or("main")
        } else {
            self.plugin_id.as_str()
        };
        format!("scriptlet/{}:{}", owner, self.name)
    }
}

/// Metadata extracted from script file comments
#[derive(Debug, Default, Clone)]
pub struct ScriptMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Icon name (e.g., "File", "Terminal", "Star", "Folder")
    pub icon: Option<String>,
    /// Alias for quick invocation (e.g., "gpt" triggers on "gpt ")
    pub alias: Option<String>,
    /// Keyboard shortcut for direct invocation (e.g., "opt i", "cmd shift k")
    pub shortcut: Option<String>,
}

/// Schedule metadata extracted from script file comments
/// Used for cron-based script scheduling
#[derive(Debug, Default, Clone)]
pub struct ScheduleMetadata {
    /// Raw cron expression from `// Cron: */5 * * * *`
    pub cron: Option<String>,
    /// Natural language schedule from `// Schedule: every tuesday at 2pm`
    pub schedule: Option<String>,
}

/// Runtime configuration for fallback commands
/// Fallback commands are shown when no search results match,
/// allowing the typed text to be used as input.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// The script that handles this fallback
    pub script: std::sync::Arc<Script>,
    /// Display label with {input} placeholder replaced with actual input
    /// (e.g., "Search docs for {input}" -> "Search docs for my query")
    pub label: String,
    /// The original label template with {input} placeholder
    pub label_template: String,
}

impl FallbackConfig {
    /// Create a new FallbackConfig from a script with fallback metadata
    ///
    /// Returns None if the script doesn't have fallback enabled
    pub fn from_script(script: std::sync::Arc<Script>) -> Option<Self> {
        let typed_meta = script.typed_metadata.as_ref()?;

        if !typed_meta.fallback {
            return None;
        }

        // Use fallback_label if provided, otherwise use script name with {input}
        let label_template = typed_meta
            .fallback_label
            .clone()
            .unwrap_or_else(|| format!("{} {{input}}", script.name));

        Some(Self {
            script,
            label: label_template.clone(), // Will be replaced with actual input at runtime
            label_template,
        })
    }

    /// Update the label by replacing {input} placeholder with actual user input
    pub fn with_input(&self, input: &str) -> Self {
        Self {
            script: self.script.clone(),
            label: self.label_template.replace("{input}", input),
            label_template: self.label_template.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AgentChatHistoryMatch, ClipboardHistoryMatch, FallbackMatch, FileMatch, SearchResult,
    };
    use crate::ai::agent_chat::ui::history::{AgentChatHistoryEntry, AgentChatHistorySearchField};
    use crate::clipboard_history::{ClipboardEntryMeta, ContentType};
    use crate::fallbacks::builtins::{BuiltinFallback, FallbackAction, FallbackCondition};
    use crate::fallbacks::collector::FallbackItem;
    use crate::file_search::{FileResult, FileType};

    fn file_result(file_type: FileType) -> FileResult {
        FileResult {
            path: "/Users/example/Desktop/fix spelling.png".to_string(),
            name: "fix spelling.png".to_string(),
            size: 0,
            modified: 0,
            file_type,
        }
    }

    #[test]
    fn file_search_result_exposes_launcher_metadata() {
        let result = SearchResult::File(FileMatch {
            file: file_result(FileType::Image),
            score: 42,
        });

        assert_eq!(result.name(), "fix spelling.png");
        assert_eq!(
            result.description(),
            Some("/Users/example/Desktop/fix spelling.png")
        );
        assert_eq!(result.score(), 42);
        assert_eq!(result.type_label(), "File");
        assert_eq!(
            result.launcher_command_id(),
            Some("file//Users/example/Desktop/fix spelling.png".to_string())
        );
        assert_eq!(result.history_result_key(), result.launcher_command_id());
        assert_eq!(result.type_accessory_info(), ("File", "file"));
        assert_eq!(result.source_name(), Some("Files"));
        assert_eq!(result.get_default_action_text(), "Open File");
    }

    #[test]
    fn file_search_result_labels_directories_as_folders() {
        let result = SearchResult::File(FileMatch {
            file: file_result(FileType::Directory),
            score: 1,
        });

        assert_eq!(result.get_default_action_text(), "Open Folder");
    }

    #[test]
    fn agent_chat_history_result_exposes_launcher_metadata() {
        let result = SearchResult::AgentChatHistory(AgentChatHistoryMatch {
            entry: AgentChatHistoryEntry {
                timestamp: "2026-05-10T17:13:06Z".to_string(),
                first_message: "How do I search files?".to_string(),
                message_count: 4,
                session_id: "session-123".to_string(),
                title: "How do I search files?".to_string(),
                custom_title: None,
                preview: "Use the root launcher".to_string(),
                search_text: "how do i search files use the root launcher".to_string(),
            },
            score: 80,
            matched_field: AgentChatHistorySearchField::Title,
            subtitle: "Use the root launcher · 4 messages".to_string(),
            evidence: None,
        });

        assert_eq!(result.name(), "How do I search files?");
        assert_eq!(
            result.description(),
            Some("Use the root launcher · 4 messages")
        );
        assert_eq!(result.score(), 80);
        assert_eq!(result.type_label(), "Agent Chat Conversation");
        assert_eq!(result.launcher_command_id(), None);
        assert_eq!(
            result.history_result_key(),
            Some("agent_chat-history/session-123".to_string())
        );
        assert_eq!(
            result.type_accessory_info(),
            ("Agent Chat", "message-circle")
        );
        assert_eq!(result.source_name(), Some("Agent Chat Conversations"));
        assert_eq!(result.get_default_action_text(), "Resume Conversation");
    }

    #[test]
    fn clipboard_history_result_exposes_launcher_metadata() {
        let result = SearchResult::ClipboardHistory(ClipboardHistoryMatch {
            entry: ClipboardEntryMeta {
                id: "clip-123".to_string(),
                content_type: ContentType::Text,
                timestamp: 1_778_000_000_000,
                pinned: false,
                text_preview: "fix spelling without changing case".to_string(),
                image_width: None,
                image_height: None,
                byte_size: 36,
                ocr_text: None,
            },
            title: "fix spelling without changing case".to_string(),
            subtitle: "Text · just now".to_string(),
            score: 70,
        });

        assert_eq!(result.name(), "fix spelling without changing case");
        assert_eq!(result.description(), Some("Text · just now"));
        assert_eq!(result.score(), 70);
        assert_eq!(result.type_label(), "Clipboard");
        assert_eq!(result.launcher_command_id(), None);
        assert_eq!(
            result.history_result_key(),
            Some("clipboard-history/clip-123".to_string())
        );
        assert_eq!(result.type_accessory_info(), ("Clipboard", "clipboard"));
        assert_eq!(result.source_name(), Some("Clipboard History"));
        assert_eq!(result.get_default_action_text(), "Paste Clipboard");
    }

    #[test]
    fn fallback_result_has_stable_selection_key_but_no_history_key() {
        let result = SearchResult::Fallback(FallbackMatch::new(
            FallbackItem::Builtin(BuiltinFallback::new(
                "search-test",
                "Search Test",
                "Search a test engine",
                "Search",
                FallbackAction::SearchUrl {
                    template: "https://example.com/?q={query}".to_string(),
                },
                FallbackCondition::Always,
                20,
            )),
            0,
        ));

        assert_eq!(result.history_result_key(), None);
        assert_eq!(
            result.stable_selection_key(),
            Some("fallback/Search Test".to_string())
        );
    }

    #[test]
    fn fallback_result_can_override_stable_selection_key() {
        let result = SearchResult::Fallback(
            FallbackMatch::new(
                FallbackItem::Builtin(BuiltinFallback::new(
                    "search-test",
                    "Search Test",
                    "Search a test engine",
                    "Search",
                    FallbackAction::SearchUrl {
                        template: "https://example.com/?q={query}".to_string(),
                    },
                    FallbackCondition::Always,
                    20,
                )),
                0,
            )
            .with_display_overrides("Search Test for \"docs\"", "Open search")
            .with_stable_selection_key("fallback/root-file-search-handoff/global"),
        );

        assert_eq!(result.history_result_key(), None);
        assert_eq!(
            result.stable_selection_key(),
            Some("fallback/root-file-search-handoff/global".to_string())
        );
    }

    #[test]
    fn file_result_stable_selection_key_matches_history_key() {
        let result = SearchResult::File(FileMatch {
            file: file_result(FileType::Image),
            score: 42,
        });

        assert_eq!(result.stable_selection_key(), result.history_result_key());
    }
}

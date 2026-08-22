//! Agent Chat conversation history persistence.
//!
//! - `agent_chat-history.jsonl` — One-line summaries for Cmd+P browsing
//! - `agent_chat-conversations/{session_id}.json` — Full message history for resume
//! - `agent_chat-prompt-history.jsonl` — Flat list of submitted composer
//!   prompts for shell-style Up/Down recall across sessions

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

type HistoryFileSignature = Option<(std::path::PathBuf, std::time::SystemTime, u64)>;
const HISTORY_SEARCH_TEXT_MAX_CHARS: usize = 4096;
const ROOT_AGENT_CHAT_HISTORY_REFRESH_LABEL: &str = "root-agent_chat-history-cache";

#[derive(Clone)]
struct AgentChatHistoryIndexCache {
    signature: HistoryFileSignature,
    entries: Vec<AgentChatHistoryEntry>,
}

type AgentChatHistoryRefreshLifecycle =
    crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle;
pub(crate) type RootAgentChatHistoryRefresh =
    crate::scripts::root_search_contract::RootOwnedProviderRefresh;

pub(crate) struct RootAgentChatHistorySnapshot {
    cache: AgentChatHistoryIndexCache,
}

static AGENT_CHAT_HISTORY_INDEX_CACHE: OnceLock<Mutex<Option<AgentChatHistoryIndexCache>>> =
    OnceLock::new();
static AGENT_CHAT_HISTORY_REFRESH_LIFECYCLE: OnceLock<Mutex<AgentChatHistoryRefreshLifecycle>> =
    OnceLock::new();

fn agent_chat_history_index_cache() -> &'static Mutex<Option<AgentChatHistoryIndexCache>> {
    AGENT_CHAT_HISTORY_INDEX_CACHE.get_or_init(|| Mutex::new(None))
}

fn agent_chat_history_refresh_lifecycle() -> &'static Mutex<AgentChatHistoryRefreshLifecycle> {
    AGENT_CHAT_HISTORY_REFRESH_LIFECYCLE
        .get_or_init(|| Mutex::new(AgentChatHistoryRefreshLifecycle::default()))
}

pub(crate) fn invalidate_history_cache() {
    if let Some(cache) = AGENT_CHAT_HISTORY_INDEX_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            *guard = None;
        }
    }
}

/// A single conversation history entry (summary for the index).
///
/// New fields (`title`, `preview`, `search_text`) are populated on save and
/// back-filled on read for older JSONL lines that lack them.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct AgentChatHistoryEntry {
    pub timestamp: String,
    pub first_message: String,
    pub message_count: usize,
    pub session_id: String,
    /// Short title derived from the first user message (max 100 chars).
    pub title: String,
    /// User- or LLM-provided conversation title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    /// Preview derived from the last assistant message (max 160 chars).
    pub preview: String,
    /// Lowercased searchable text from the first few transcript turns.
    pub search_text: String,
}

/// Which field produced the strongest match in a history search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatHistorySearchField {
    Title,
    Preview,
    SearchText,
    Timestamp,
}

/// A single ranked search hit from [`search_history`].
#[derive(Debug, Clone)]
pub(crate) struct AgentChatHistorySearchHit {
    pub entry: AgentChatHistoryEntry,
    pub score: u32,
    pub matched_field: AgentChatHistorySearchField,
    /// Word-level match evidence produced at qualification time; renderers
    /// highlight exactly these ranges. `None` for empty-query recency rows.
    pub evidence: Option<crate::scripts::search::sentence::LongTextMatchEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RootAgentChatHistorySectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
}

impl Default for RootAgentChatHistorySectionOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 3,
            min_query_chars: 3,
        }
    }
}

pub(crate) fn root_agent_chat_history_query_is_eligible(
    query: &str,
    options: RootAgentChatHistorySectionOptions,
) -> bool {
    options.enabled
        && crate::scripts::search::query_meets_min_query_chars(
            query.trim(),
            options.min_query_chars,
        )
}

impl AgentChatHistoryEntry {
    /// Returns `title` if populated, otherwise falls back to `first_message`.
    pub(crate) fn title_display(&self) -> &str {
        if let Some(custom_title) = self.custom_title.as_deref().map(str::trim) {
            if !custom_title.is_empty() {
                return custom_title;
            }
        }
        if self.title.is_empty() {
            &self.first_message
        } else {
            &self.title
        }
    }

    /// Returns `preview` if populated, otherwise falls back to `first_message`.
    pub(crate) fn preview_display(&self) -> &str {
        if self.preview.is_empty() {
            &self.first_message
        } else {
            &self.preview
        }
    }
}

/// A saved message for full conversation persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedMessage {
    pub role: String,
    pub body: String,
}

/// Full conversation snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedConversation {
    pub session_id: String,
    pub timestamp: String,
    pub messages: Vec<SavedMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
}

// ── Text helpers ─────────────────────────────────────────────────────

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push('\u{2026}'); // …
    }
    out
}

fn normalize_search_text(value: &str) -> String {
    collapse_whitespace(value).to_lowercase()
}

fn bounded_search_text(value: &str) -> String {
    truncate_chars(&normalize_search_text(value), HISTORY_SEARCH_TEXT_MAX_CHARS)
}

pub(crate) fn sanitize_conversation_title(value: &str) -> String {
    let collapsed = collapse_whitespace(value);
    let trimmed = collapsed
        .trim()
        .trim_matches(|ch| matches!(ch, '\'' | '"' | '“' | '”' | '‘' | '’'))
        .trim()
        .trim_end_matches(['.', ',', ';', ':', '!', '?'])
        .trim();
    truncate_chars(trimmed, 60)
}

// ── Index builder ────────────────────────────────────────────────────

/// Build a rich history entry from a full saved conversation.
///
/// Returns `None` if the conversation has no user message.
pub(crate) fn build_history_entry(
    conversation: &SavedConversation,
) -> Option<AgentChatHistoryEntry> {
    let first_user = conversation
        .messages
        .iter()
        .find(|m| m.role.eq_ignore_ascii_case("user"))?;

    let last_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role.eq_ignore_ascii_case("assistant"));

    let title = truncate_chars(&collapse_whitespace(&first_user.body), 100);

    let preview_source = last_assistant
        .map(|m| m.body.as_str())
        .unwrap_or(first_user.body.as_str());
    let preview = truncate_chars(&collapse_whitespace(preview_source), 160);

    // Build a small transcript sample for full-text search.
    let mut transcript_sample = String::new();
    for msg in conversation.messages.iter().take(8) {
        transcript_sample.push_str(msg.role.as_str());
        transcript_sample.push_str(": ");
        transcript_sample.push_str(&collapse_whitespace(&msg.body));
        transcript_sample.push('\n');
    }

    Some(AgentChatHistoryEntry {
        timestamp: conversation.timestamp.clone(),
        first_message: truncate_chars(&collapse_whitespace(&first_user.body), 100),
        message_count: conversation.messages.len(),
        session_id: conversation.session_id.clone(),
        title: title.clone(),
        custom_title: conversation.custom_title.clone(),
        preview: preview.clone(),
        search_text: bounded_search_text(&format!(
            "{}\n{}\n{}\n{}\n{}",
            title,
            conversation.custom_title.as_deref().unwrap_or_default(),
            preview,
            transcript_sample,
            conversation.timestamp
        )),
    })
}

// ── Search / ranking ─────────────────────────────────────────────────

/// Rank history entries against a query using the shared long-text
/// sentence contract (word-boundary matching, tiered relevance, evidence).
///
/// Empty query returns up to `limit` entries in recency order (no filtering).
fn rank_history_entries(
    entries: Vec<AgentChatHistoryEntry>,
    query: &str,
    limit: usize,
) -> Vec<AgentChatHistorySearchHit> {
    use crate::scripts::search::sentence::{
        compile_long_text_query, match_long_text_query, FieldClass, FieldVisibility, LongTextField,
        LongTextFieldId, RenderSlot,
    };

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return entries
            .into_iter()
            .take(limit)
            .map(|entry| AgentChatHistorySearchHit {
                entry,
                score: 0,
                matched_field: AgentChatHistorySearchField::Title,
                evidence: None,
            })
            .collect();
    }

    let Some(compiled) = compile_long_text_query(trimmed) else {
        return Vec::new();
    };

    let mut hits = Vec::new();

    for entry in entries {
        // Visible fields first so redundant hidden blobs (search_text
        // repeats the title/preview) do not claim primary attribution.
        let fields = [
            LongTextField {
                id: LongTextFieldId::Title,
                text: entry.title_display(),
                class: FieldClass::NaturalText,
                visibility: FieldVisibility::Visible(RenderSlot::Title),
                weight: 6,
            },
            LongTextField {
                id: LongTextFieldId::Preview,
                text: entry.preview_display(),
                class: FieldClass::NaturalText,
                visibility: FieldVisibility::Visible(RenderSlot::Subtitle),
                weight: 4,
            },
            LongTextField {
                id: LongTextFieldId::Transcript,
                text: entry.search_text.as_str(),
                class: FieldClass::NaturalText,
                visibility: FieldVisibility::Hidden,
                weight: 1,
            },
            LongTextField {
                id: LongTextFieldId::Timestamp,
                text: entry.timestamp.as_str(),
                class: FieldClass::Metadata,
                visibility: FieldVisibility::Hidden,
                weight: 1,
            },
        ];

        let Some(matched) = match_long_text_query(&compiled, &fields) else {
            continue;
        };

        let matched_field = match matched.evidence.primary_field {
            LongTextFieldId::Title => AgentChatHistorySearchField::Title,
            LongTextFieldId::Preview => AgentChatHistorySearchField::Preview,
            LongTextFieldId::Timestamp => AgentChatHistorySearchField::Timestamp,
            _ => AgentChatHistorySearchField::SearchText,
        };

        hits.push(AgentChatHistorySearchHit {
            entry,
            score: matched.rank_score(),
            matched_field,
            evidence: Some(matched.evidence),
        });
    }

    // Deterministic: relevance tier + in-tier score first, recency breaks
    // ties. A recent unrelated row must not beat an older phrase match.
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.entry.timestamp.cmp(&a.entry.timestamp))
    });

    hits.truncate(limit);
    hits
}

/// Search loaded history entries, returning ranked hits.
///
/// Emits `agent_chat_history_search_executed` structured log on every call.
pub(crate) fn search_history(query: &str, limit: usize) -> Vec<AgentChatHistorySearchHit> {
    let hits = rank_history_entries(load_history(), query, limit);
    let safe_query = crate::logging::log_private_user_value(query);
    tracing::info!(
        target: "script_kit::tab_ai",
        event = "agent_chat_history_search_executed",
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        limit,
        hit_count = hits.len(),
    );
    hits
}

pub(crate) fn search_history_direct(query: &str, limit: usize) -> Vec<AgentChatHistorySearchHit> {
    search_history(query, limit)
}

fn cached_history_entries_if_fresh() -> Option<Vec<AgentChatHistoryEntry>> {
    let path = history_path();
    let signature = history_file_signature(&path);
    let guard = agent_chat_history_index_cache().lock().ok()?;
    let cache = guard.as_ref()?;
    (cache.signature == signature).then(|| cache.entries.clone())
}

pub(crate) fn root_agent_chat_history_cache_is_fresh() -> bool {
    cached_history_entries_if_fresh().is_some()
}

pub(crate) fn try_begin_root_agent_chat_history_refresh() -> Option<RootAgentChatHistoryRefresh> {
    let cache_is_fresh = root_agent_chat_history_cache_is_fresh();
    agent_chat_history_refresh_lifecycle().lock().ok()?.begin(
        sk_protocol::command_contract::CommandSource::Conversation,
        cache_is_fresh,
    )
}

fn read_root_agent_chat_history_snapshot_at(
    path: &std::path::Path,
) -> RootAgentChatHistorySnapshot {
    let signature = history_file_signature(path);
    let entries = read_private_history_file(path)
        .map(|content| parse_history_entries(&content))
        .unwrap_or_default();
    RootAgentChatHistorySnapshot {
        cache: AgentChatHistoryIndexCache { signature, entries },
    }
}

pub(crate) fn read_root_agent_chat_history_snapshot() -> RootAgentChatHistorySnapshot {
    tracing::debug!(
        target: "script_kit::search",
        worker = ROOT_AGENT_CHAT_HISTORY_REFRESH_LABEL,
        "Reading owned private conversation history snapshot"
    );
    read_root_agent_chat_history_snapshot_at(&history_path())
}

fn root_agent_chat_history_snapshot_is_current_at(
    snapshot: &RootAgentChatHistorySnapshot,
    path: &std::path::Path,
) -> bool {
    snapshot.cache.signature == history_file_signature(path)
}

pub(crate) fn finish_root_agent_chat_history_refresh(
    refresh: RootAgentChatHistoryRefresh,
    snapshot: RootAgentChatHistorySnapshot,
) -> bool {
    let Ok(mut lifecycle) = agent_chat_history_refresh_lifecycle().lock() else {
        return false;
    };
    if !lifecycle.finish(refresh) {
        return false;
    }
    drop(lifecycle);

    if !root_agent_chat_history_snapshot_is_current_at(&snapshot, &history_path()) {
        return false;
    }
    let Ok(mut cache) = agent_chat_history_index_cache().lock() else {
        return false;
    };
    *cache = Some(snapshot.cache);
    true
}

pub(crate) fn discard_root_agent_chat_history_refresh(
    refresh: RootAgentChatHistoryRefresh,
) -> bool {
    agent_chat_history_refresh_lifecycle()
        .lock()
        .is_ok_and(|mut lifecycle| lifecycle.finish(refresh))
}

/// Cache-only Agent Chat history search for root launcher passive rows.
///
/// A cold or stale JSONL index returns no hits without starting work or
/// publishing a snapshot. The real input owner starts a generation-fenced
/// refresh and explicitly reconciles the selected launcher row on completion.
pub(crate) fn search_history_cached(query: &str, limit: usize) -> Vec<AgentChatHistorySearchHit> {
    let Some(entries) = cached_history_entries_if_fresh() else {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "root_agent_chat_history_search_cache_miss",
            query_len = query.trim().chars().count(),
            limit,
        );
        return Vec::new();
    };

    let hits = rank_history_entries(entries, query, limit);
    if crate::logging::filter_perf_trace_enabled() {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "root_agent_chat_history_search_cache_hit",
            query_len = query.trim().chars().count(),
            limit,
            hit_count = hits.len(),
        );
    }
    hits
}

// ── Persistence paths ────────────────────────────────────────────────

fn history_path() -> std::path::PathBuf {
    crate::setup::get_kit_path().join("agent_chat-history.jsonl")
}

/// One submitted composer prompt, for shell-style Up/Down recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PromptHistoryLine {
    timestamp: String,
    prompt: String,
}

const PROMPT_HISTORY_MAX_LINES: usize = 200;

/// Append a submitted composer prompt to the flat recall store. Consecutive
/// duplicates are skipped; the file compacts to the newest
/// `PROMPT_HISTORY_MAX_LINES` entries once it doubles that size.
pub(crate) fn append_prompt_history(prompt: &str) {
    if let Err(error) = append_prompt_history_at(&crate::setup::get_kit_path(), prompt) {
        tracing::debug!(reason = %error, "agent_chat_prompt_history_write_failed");
    }
}

fn append_prompt_history_at(
    kit_root: &std::path::Path,
    prompt: &str,
) -> Result<(), AgentChatConversationPersistenceError> {
    use std::io::Write as _;

    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Ok(());
    }

    let path = kit_root.join("agent_chat-prompt-history.jsonl");
    let _ = inspect_regular_history_file(&path)?;
    if load_prompt_history_at(kit_root, 1)?
        .last()
        .map(String::as_str)
        == Some(prompt)
    {
        return Ok(());
    }

    let line = PromptHistoryLine {
        timestamp: chrono::Utc::now().to_rfc3339(),
        prompt: prompt.to_string(),
    };
    let json = serde_json::to_string(&line)
        .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;

    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    ensure_private_regular_history_file(&file)?;
    writeln!(file, "{json}")
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    drop(file);

    let content = read_private_history_file(&path)?;
    if content.lines().count() > PROMPT_HISTORY_MAX_LINES * 2 {
        let keep: Vec<&str> = content
            .lines()
            .rev()
            .take(PROMPT_HISTORY_MAX_LINES)
            .collect();
        let mut rewritten = String::new();
        for entry in keep.iter().rev() {
            rewritten.push_str(entry);
            rewritten.push('\n');
        }
        write_private_history_file_atomically(&path, &rewritten)?;
    }

    Ok(())
}

/// Load the newest `limit` submitted prompts, oldest → newest.
pub(crate) fn load_prompt_history(limit: usize) -> Vec<String> {
    match load_prompt_history_at(&crate::setup::get_kit_path(), limit) {
        Ok(prompts) => prompts,
        Err(error) => {
            tracing::debug!(reason = %error, "agent_chat_prompt_history_read_failed");
            Vec::new()
        }
    }
}

fn load_prompt_history_at(
    kit_root: &std::path::Path,
    limit: usize,
) -> Result<Vec<String>, AgentChatConversationPersistenceError> {
    let path = kit_root.join("agent_chat-prompt-history.jsonl");
    if !inspect_regular_history_file(&path)? {
        return Ok(Vec::new());
    }
    let content = read_private_history_file(&path)?;
    let mut prompts: Vec<String> = content
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<PromptHistoryLine>(line).ok())
        .map(|line| line.prompt)
        .take(limit)
        .collect();
    prompts.reverse();
    Ok(prompts)
}

fn conversations_dir() -> std::path::PathBuf {
    crate::setup::get_kit_path().join("agent_chat-conversations")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChatConversationPersistenceError {
    InvalidSessionId,
    UnsafeConversationDirectory,
    UnsafeFileTarget,
    SessionIdMismatch,
    SerializationFailed,
    InvalidConversationPayload,
    Io(std::io::ErrorKind),
}

impl std::fmt::Display for AgentChatConversationPersistenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSessionId => formatter.write_str("invalid Agent Chat session identifier"),
            Self::UnsafeConversationDirectory => {
                formatter.write_str("unsafe Agent Chat conversation directory")
            }
            Self::UnsafeFileTarget => formatter.write_str("unsafe Agent Chat history file"),
            Self::SessionIdMismatch => {
                formatter.write_str("saved Agent Chat conversation identity does not match")
            }
            Self::SerializationFailed => {
                formatter.write_str("failed to encode Agent Chat conversation")
            }
            Self::InvalidConversationPayload => {
                formatter.write_str("invalid saved Agent Chat conversation")
            }
            Self::Io(kind) => write!(formatter, "Agent Chat history I/O failed ({kind:?})"),
        }
    }
}

impl std::error::Error for AgentChatConversationPersistenceError {}

pub(super) fn validate_agent_chat_session_id(
    session_id: &str,
) -> Result<(), AgentChatConversationPersistenceError> {
    use std::path::Component;

    let mut chars = session_id.chars();
    let has_windows_drive_prefix = matches!(
        (chars.next(), chars.next()),
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
    );
    if session_id.is_empty()
        || session_id.trim().is_empty()
        || has_windows_drive_prefix
        || session_id
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err(AgentChatConversationPersistenceError::InvalidSessionId);
    }

    let mut components = std::path::Path::new(session_id).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(component)), None)
            if component == std::ffi::OsStr::new(session_id) =>
        {
            Ok(())
        }
        _ => Err(AgentChatConversationPersistenceError::InvalidSessionId),
    }
}

fn conversation_path_at(
    kit_root: &std::path::Path,
    session_id: &str,
) -> Result<std::path::PathBuf, AgentChatConversationPersistenceError> {
    validate_agent_chat_session_id(session_id)?;
    Ok(kit_root
        .join("agent_chat-conversations")
        .join(format!("{session_id}.json")))
}

pub(super) fn inspect_regular_history_file(
    path: &std::path::Path,
) -> Result<bool, AgentChatConversationPersistenceError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => Ok(true),
        Ok(_) => Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AgentChatConversationPersistenceError::Io(error.kind())),
    }
}

pub(super) fn inspect_conversation_directory(
    path: &std::path::Path,
) -> Result<bool, AgentChatConversationPersistenceError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => Ok(true),
        Ok(_) => Err(AgentChatConversationPersistenceError::UnsafeConversationDirectory),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AgentChatConversationPersistenceError::Io(error.kind())),
    }
}

/// Repair legacy world/group-readable history through its already-open,
/// no-follow file descriptor before exposing or appending private content.
fn ensure_private_regular_history_file(
    file: &std::fs::File,
) -> Result<(), AgentChatConversationPersistenceError> {
    let metadata = file
        .metadata()
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    if !metadata.is_file() {
        return Err(AgentChatConversationPersistenceError::UnsafeFileTarget);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
        }
    }
    Ok(())
}

fn read_private_history_file(
    path: &std::path::Path,
) -> Result<String, AgentChatConversationPersistenceError> {
    use std::io::Read as _;

    if !inspect_regular_history_file(path)? {
        return Err(AgentChatConversationPersistenceError::Io(
            std::io::ErrorKind::NotFound,
        ));
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    ensure_private_regular_history_file(&file)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    Ok(contents)
}

pub(super) fn write_private_history_file_atomically(
    destination: &std::path::Path,
    contents: &str,
) -> Result<(), AgentChatConversationPersistenceError> {
    use std::io::Write as _;

    let _ = inspect_regular_history_file(destination)?;
    let parent = destination
        .parent()
        .ok_or(AgentChatConversationPersistenceError::UnsafeFileTarget)?;
    let temporary = parent.join(format!(".agent-chat-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
        file.flush()
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
        drop(file);

        // Revalidate immediately before rename. Renaming replaces a raced
        // directory entry itself; it never writes through a destination link.
        let _ = inspect_regular_history_file(destination)?;
        std::fs::rename(&temporary, destination)
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Append a history entry to the JSONL index file.
/// Compacts the file when it exceeds 200 lines.
pub(crate) fn save_history_entry(entry: &AgentChatHistoryEntry) {
    if let Err(error) = save_history_entry_at(&crate::setup::get_kit_path(), entry) {
        tracing::debug!(reason = %error, "agent_chat_history_write_failed");
    }
}

fn save_history_entry_at(
    kit_root: &std::path::Path,
    entry: &AgentChatHistoryEntry,
) -> Result<(), AgentChatConversationPersistenceError> {
    use std::io::Write as _;

    validate_agent_chat_session_id(&entry.session_id)?;
    let path = kit_root.join("agent_chat-history.jsonl");
    let json = serde_json::to_string(entry)
        .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;
    let _ = inspect_regular_history_file(&path)?;

    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    ensure_private_regular_history_file(&file)?;
    writeln!(file, "{json}")
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    drop(file);
    invalidate_history_cache();

    // Compact when file grows too large (>200 lines)
    let content = read_private_history_file(&path)?;
    if content.lines().count() > 200 {
        let compacted = parse_history_entries(&content);
        let mut rewritten = String::new();
        for compacted_entry in compacted.iter().rev() {
            if let Ok(json) = serde_json::to_string(compacted_entry) {
                rewritten.push_str(&json);
                rewritten.push('\n');
            }
        }
        write_private_history_file_atomically(&path, &rewritten)?;
        invalidate_history_cache();
    }

    Ok(())
}

/// Save full conversation messages to a session-specific JSON file.
pub(crate) fn save_conversation(conversation: &SavedConversation) {
    match save_conversation_at(&crate::setup::get_kit_path(), conversation) {
        Ok(()) => cleanup_old_conversations(50),
        Err(error) => {
            tracing::debug!(reason = %error, "agent_chat_conversation_write_failed");
        }
    }
}

fn save_conversation_at(
    kit_root: &std::path::Path,
    conversation: &SavedConversation,
) -> Result<(), AgentChatConversationPersistenceError> {
    // Validate the untrusted session before creating even the containing
    // directory: hostile IDs must not produce any filesystem side effects.
    let path = conversation_path_at(kit_root, &conversation.session_id)?;
    let json = serde_json::to_string_pretty(conversation)
        .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;
    let directory = kit_root.join("agent_chat-conversations");
    if !inspect_conversation_directory(&directory)? {
        std::fs::create_dir_all(&directory)
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
        if !inspect_conversation_directory(&directory)? {
            return Err(AgentChatConversationPersistenceError::UnsafeConversationDirectory);
        }
    }

    write_private_history_file_atomically(&path, &json)
}

/// Load history entries from the JSONL file (most recent first).
///
/// Older entries written before the `title`/`preview`/`search_text` fields
/// existed are back-filled on read from `first_message` so that callers
/// always see populated display fields.
pub(crate) fn load_history() -> Vec<AgentChatHistoryEntry> {
    let path = history_path();
    let signature = history_file_signature(&path);
    if let Ok(guard) = agent_chat_history_index_cache().lock() {
        if let Some(cache) = guard.as_ref() {
            if cache.signature == signature {
                return cache.entries.clone();
            }
        }
    }

    let entries = read_private_history_file(&path)
        .map(|content| parse_history_entries(&content))
        .unwrap_or_default();

    if let Ok(mut guard) = agent_chat_history_index_cache().lock() {
        *guard = Some(AgentChatHistoryIndexCache {
            signature,
            entries: entries.clone(),
        });
    }

    entries
}

fn history_file_signature(path: &std::path::Path) -> HistoryFileSignature {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    Some((
        path.to_path_buf(),
        metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        metadata.len(),
    ))
}

fn parse_history_entries(content: &str) -> Vec<AgentChatHistoryEntry> {
    let mut entries: Vec<AgentChatHistoryEntry> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .map(|mut entry: AgentChatHistoryEntry| {
            // Back-fill missing fields from legacy entries.
            if entry
                .custom_title
                .as_deref()
                .is_some_and(|title| title.trim().is_empty())
            {
                entry.custom_title = None;
            }
            if entry.title.is_empty() {
                entry.title = entry.first_message.clone();
            }
            if entry.preview.is_empty() {
                entry.preview = entry.first_message.clone();
            }
            if entry.search_text.is_empty() {
                entry.search_text = bounded_search_text(&format!(
                    "{}\n{}\n{}\n{}",
                    entry.title,
                    entry.custom_title.as_deref().unwrap_or_default(),
                    entry.preview,
                    entry.timestamp
                ));
            } else {
                entry.search_text = bounded_search_text(&entry.search_text);
            }
            entry
        })
        .collect();

    // Most recent first, then deduplicate (keeps latest per session_id)
    entries.reverse();
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.session_id.clone()));
    entries.truncate(100);
    entries
}

/// Whether a saved conversation exists for `session_id` (cheap stat, no
/// parse). Lets callers pick a different route up front when resume would
/// fall back — e.g. a brain chat_turn memory whose conversation file is gone
/// stages the memory as a context chip instead of opening an empty chat.
pub(crate) fn conversation_exists(session_id: &str) -> bool {
    conversation_exists_at(&crate::setup::get_kit_path(), session_id).unwrap_or(false)
}

fn conversation_exists_at(
    kit_root: &std::path::Path,
    session_id: &str,
) -> Result<bool, AgentChatConversationPersistenceError> {
    let path = conversation_path_at(kit_root, session_id)?;
    let directory = kit_root.join("agent_chat-conversations");
    if !inspect_conversation_directory(&directory)? {
        return Ok(false);
    }
    inspect_regular_history_file(&path)
}

/// Load a full conversation by session ID.
pub(crate) fn load_conversation(session_id: &str) -> Option<SavedConversation> {
    load_conversation_at(&crate::setup::get_kit_path(), session_id)
        .ok()
        .flatten()
}

fn load_conversation_at(
    kit_root: &std::path::Path,
    session_id: &str,
) -> Result<Option<SavedConversation>, AgentChatConversationPersistenceError> {
    let path = conversation_path_at(kit_root, session_id)?;
    if !conversation_exists_at(kit_root, session_id)? {
        return Ok(None);
    }

    let content = read_private_history_file(&path)?;
    let conversation: SavedConversation = serde_json::from_str(&content)
        .map_err(|_| AgentChatConversationPersistenceError::InvalidConversationPayload)?;
    if conversation.session_id != session_id {
        return Err(AgentChatConversationPersistenceError::SessionIdMismatch);
    }
    Ok(Some(conversation))
}

pub(crate) fn rename_conversation(session_id: &str, new_title: &str) -> anyhow::Result<()> {
    rename_conversation_at(&crate::setup::get_kit_path(), session_id, new_title)
}

fn rename_conversation_at(
    kit_root: &std::path::Path,
    session_id: &str,
    new_title: &str,
) -> anyhow::Result<()> {
    use anyhow::Context;

    validate_agent_chat_session_id(session_id)?;
    let mut conversation = load_conversation_at(kit_root, session_id)?
        .context("load saved Agent Chat conversation")?;
    let sanitized = sanitize_conversation_title(new_title);
    conversation.custom_title = (!sanitized.is_empty()).then_some(sanitized);
    let entry = build_history_entry(&conversation).context("rebuild Agent Chat history entry")?;
    let _ = inspect_regular_history_file(&kit_root.join("agent_chat-history.jsonl"))?;
    save_conversation_at(kit_root, &conversation)?;
    save_history_entry_at(kit_root, &entry)?;
    Ok(())
}

/// Delete a single conversation by session ID.
///
/// Removes the saved conversation file and rewrites `agent_chat-history.jsonl`
/// without the deleted `session_id`. Returns `Ok(())` even if the
/// session was not found (idempotent).
pub(crate) fn delete_conversation(session_id: &str) -> anyhow::Result<()> {
    delete_conversation_at(&crate::setup::get_kit_path(), session_id)?;
    tracing::info!(event = "agent_chat_history_item_deleted", session_id = %session_id);
    Ok(())
}

fn delete_conversation_at(kit_root: &std::path::Path, session_id: &str) -> anyhow::Result<()> {
    let conversation_path = conversation_path_at(kit_root, session_id)?;
    let directory = kit_root.join("agent_chat-conversations");
    let conversation_exists = if inspect_conversation_directory(&directory)? {
        inspect_regular_history_file(&conversation_path)?
    } else {
        false
    };
    if conversation_exists {
        let _ = load_conversation_at(kit_root, session_id)?
            .ok_or(AgentChatConversationPersistenceError::InvalidConversationPayload)?;
    }
    let attachment_paths =
        super::history_attachment::existing_history_attachment_paths_at(kit_root, session_id)?;

    // Preflight and fully prepare the index before mutating either target. An
    // unsafe index must never delete a conversation as a partial side effect.
    let index_path = kit_root.join("agent_chat-history.jsonl");
    let rewritten_index = if inspect_regular_history_file(&index_path)? {
        let content = read_private_history_file(&index_path)?;
        let entries = parse_history_entries(&content);
        let mut rewritten = String::new();
        for entry in entries
            .into_iter()
            .filter(|entry| entry.session_id != session_id)
        {
            let json = serde_json::to_string(&entry)
                .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;
            rewritten.push_str(&json);
            rewritten.push('\n');
        }
        Some(rewritten)
    } else {
        None
    };

    if conversation_exists {
        std::fs::remove_file(&conversation_path)
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    }

    if let Some(rewritten) = rewritten_index {
        write_private_history_file_atomically(&index_path, &rewritten)?;
        invalidate_history_cache();
    }
    for attachment in attachment_paths {
        std::fs::remove_file(&attachment)
            .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    }

    Ok(())
}

/// Remove oldest conversation files beyond the keep limit.
fn cleanup_old_conversations(keep: usize) {
    let dir = conversations_dir();
    if !matches!(inspect_conversation_directory(&dir), Ok(true)) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let session_id = path.file_name()?.to_str()?.strip_suffix(".json")?;
            validate_agent_chat_session_id(session_id).ok()?;
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    if files.len() <= keep {
        return;
    }

    // Sort oldest first
    files.sort_by_key(|(_, t)| *t);

    // Remove oldest
    for (path, _) in files.iter().take(files.len() - keep) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // ── Helpers ──────────────────────────────────────────────────────

    fn make_conversation(
        session_id: &str,
        timestamp: &str,
        messages: Vec<(&str, &str)>,
    ) -> SavedConversation {
        SavedConversation {
            session_id: session_id.to_string(),
            timestamp: timestamp.to_string(),
            custom_title: None,
            messages: messages
                .into_iter()
                .map(|(role, body)| SavedMessage {
                    role: role.to_string(),
                    body: body.to_string(),
                })
                .collect(),
        }
    }

    // SK_PATH is process-global, so these tests must share the repo-wide
    // lock; a module-local mutex races against every other test suite that
    // repoints SK_PATH (dictation history, config, scriptlets, ...).
    fn history_env_lock() -> &'static Mutex<()> {
        crate::test_utils::SK_PATH_TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn root_history_refresh_never_starts_for_fresh_cache_or_duplicates_active_worker() {
        let source = sk_protocol::command_contract::CommandSource::Conversation;
        let mut lifecycle = AgentChatHistoryRefreshLifecycle::default();
        assert!(lifecycle.begin(source, true).is_none());

        let first = lifecycle
            .begin(source, false)
            .expect("cold cache starts one worker");
        assert_eq!(first.generation, 1);
        assert!(lifecycle.begin(source, false).is_none());
        assert!(lifecycle.finish(first));
        assert!(lifecycle.begin(source, false).is_some());
    }

    #[test]
    fn root_history_refresh_stale_completion_cannot_release_a_newer_owned_worker() {
        let source = sk_protocol::command_contract::CommandSource::Conversation;
        let mut lifecycle = AgentChatHistoryRefreshLifecycle::default();
        let stale = lifecycle.begin(source, false).expect("first owned worker");
        assert!(lifecycle.finish(stale));

        let current = lifecycle
            .begin(source, false)
            .expect("replacement owned worker");
        assert!(current.generation > stale.generation);
        assert!(!lifecycle.finish(stale));
        assert!(lifecycle.begin(source, false).is_none());
        assert!(lifecycle.finish(current));
    }

    #[test]
    fn root_history_refresh_generation_wrap_never_issues_the_unowned_zero_token() {
        let mut lifecycle = AgentChatHistoryRefreshLifecycle {
            next_generation: u64::MAX,
            in_flight: None,
        };
        let refresh = lifecycle
            .begin(
                sk_protocol::command_contract::CommandSource::Conversation,
                false,
            )
            .expect("wrapped generation");
        assert_eq!(refresh.generation, 1);
        assert!(lifecycle.finish(refresh));
    }

    #[test]
    fn root_history_refresh_reads_private_snapshot_without_publishing_changed_file() {
        let temp = tempfile::tempdir().expect("isolated history refresh fixture");
        let path = temp.path().join("history.jsonl");
        let entry = AgentChatHistoryEntry {
            session_id: "owned-history-session".to_owned(),
            first_message: "private conversation".to_owned(),
            timestamp: "2026-08-22T10:00:00Z".to_owned(),
            ..Default::default()
        };
        std::fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();

        let snapshot = read_root_agent_chat_history_snapshot_at(&path);
        assert_eq!(snapshot.cache.entries.len(), 1);
        assert_eq!(snapshot.cache.entries[0].session_id, entry.session_id);
        assert!(root_agent_chat_history_snapshot_is_current_at(
            &snapshot, &path
        ));

        std::fs::write(&path, "a newer and deliberately different private snapshot")
            .expect("replace history after worker read");
        assert!(!root_agent_chat_history_snapshot_is_current_at(
            &snapshot, &path
        ));
    }

    // ── Serde roundtrip ─────────────────────────────────────────────

    #[test]
    fn history_entry_serializes_with_new_fields() {
        let entry = AgentChatHistoryEntry {
            timestamp: "2026-04-01T18:00:00Z".to_string(),
            first_message: "hello world".to_string(),
            message_count: 5,
            session_id: "test-123".to_string(),
            title: "hello world".to_string(),
            custom_title: Some("Real title".to_string()),
            preview: "The answer is 42".to_string(),
            search_text: "hello world the answer is 42".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: AgentChatHistoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.title, "hello world");
        assert_eq!(parsed.custom_title.as_deref(), Some("Real title"));
        assert_eq!(parsed.preview, "The answer is 42");
        assert!(!parsed.search_text.is_empty());
    }

    #[test]
    fn legacy_entry_without_new_fields_deserializes() {
        // Simulates an old JSONL line that has no title/preview/search_text.
        let legacy_json = r#"{"timestamp":"2026-03-01T12:00:00Z","first_message":"fix the login","message_count":3,"session_id":"legacy-1"}"#;
        let entry: AgentChatHistoryEntry =
            serde_json::from_str(legacy_json).expect("legacy entry should deserialize");
        assert_eq!(entry.first_message, "fix the login");
        // New fields default to empty strings.
        assert!(entry.title.is_empty());
        assert!(entry.custom_title.is_none());
        assert!(entry.preview.is_empty());
        assert!(entry.search_text.is_empty());
    }

    #[test]
    fn legacy_saved_conversation_without_custom_title_deserializes() {
        let legacy_json = r#"{"session_id":"legacy-conv","timestamp":"2026-03-01T12:00:00Z","messages":[{"role":"user","body":"hello"}]}"#;
        let conversation: SavedConversation =
            serde_json::from_str(legacy_json).expect("legacy conversation should deserialize");
        assert_eq!(conversation.session_id, "legacy-conv");
        assert!(conversation.custom_title.is_none());
    }

    #[test]
    fn saved_conversation_serializes() {
        let conv = make_conversation(
            "test-456",
            "2026-04-01T18:00:00Z",
            vec![("user", "hello"), ("assistant", "hi there!")],
        );
        let json = serde_json::to_string_pretty(&conv).expect("serialize");
        assert!(json.contains("hello"));
        assert!(json.contains("hi there!"));
    }

    #[test]
    fn prompt_history_preserves_trimmed_private_values_order_and_consecutive_deduplication() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        assert_eq!(load_prompt_history_at(temp.path(), 10), Ok(Vec::new()));
        assert_eq!(append_prompt_history_at(temp.path(), "   \n"), Ok(()));
        assert!(!temp.path().join("agent_chat-prompt-history.jsonl").exists());

        append_prompt_history_at(temp.path(), "  first private prompt  ")
            .expect("first private prompt saves");
        append_prompt_history_at(temp.path(), "first private prompt")
            .expect("consecutive duplicate is suppressed");
        append_prompt_history_at(temp.path(), "second private prompt")
            .expect("second private prompt saves");
        append_prompt_history_at(temp.path(), "first private prompt")
            .expect("non-consecutive repeated prompt remains legitimate");

        assert_eq!(
            load_prompt_history_at(temp.path(), 10),
            Ok(vec![
                "first private prompt".to_string(),
                "second private prompt".to_string(),
                "first private prompt".to_string(),
            ]),
        );
        assert_eq!(
            load_prompt_history_at(temp.path(), 2),
            Ok(vec![
                "second private prompt".to_string(),
                "first private prompt".to_string(),
            ]),
        );
        assert_eq!(load_prompt_history_at(temp.path(), 0), Ok(Vec::new()));
    }

    #[test]
    fn prompt_history_compaction_preserves_only_newest_entries_in_chronological_order() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        let path = temp.path().join("agent_chat-prompt-history.jsonl");
        let mut seeded = String::new();
        for index in 0..PROMPT_HISTORY_MAX_LINES * 2 {
            let line = PromptHistoryLine {
                timestamp: "2026-04-01T18:00:00Z".to_string(),
                prompt: format!("private-prompt-{index}"),
            };
            seeded.push_str(&serde_json::to_string(&line).expect("synthetic prompt line"));
            seeded.push('\n');
        }
        write_private_history_file_atomically(&path, &seeded)
            .expect("private bounded prompt fixture");

        append_prompt_history_at(temp.path(), "  newest private prompt  ")
            .expect("one extra prompt triggers private atomic compaction");
        let loaded = load_prompt_history_at(temp.path(), usize::MAX)
            .expect("compacted private prompts remain readable");
        assert_eq!(loaded.len(), PROMPT_HISTORY_MAX_LINES);
        assert_eq!(
            loaded.first().map(String::as_str),
            Some("private-prompt-201")
        );
        assert_eq!(
            loaded.last().map(String::as_str),
            Some("newest private prompt")
        );
        assert_eq!(
            std::fs::read_to_string(path)
                .expect("compacted prompt file remains regular")
                .lines()
                .count(),
            PROMPT_HISTORY_MAX_LINES,
        );
    }

    #[cfg(unix)]
    #[test]
    fn prompt_history_never_follows_symlinked_private_prompt_store() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        let external = temp.path().join("unrelated-sensitive-file.txt");
        std::fs::write(&external, "external private content")
            .expect("external private file fixture");
        std::os::unix::fs::symlink(&external, root.join("agent_chat-prompt-history.jsonl"))
            .expect("malicious private-prompt symlink fixture");

        assert_eq!(
            append_prompt_history_at(&root, "never append this secret"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            load_prompt_history_at(&root, 10),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            std::fs::read_to_string(&external).expect("external private file stays untouched"),
            "external private content",
        );
        assert!(
            std::fs::symlink_metadata(root.join("agent_chat-prompt-history.jsonl"))
                .expect("malicious symlink remains untouched")
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn prompt_history_rejects_non_file_targets_before_writing() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        std::fs::create_dir(temp.path().join("agent_chat-prompt-history.jsonl"))
            .expect("wrong-type prompt fixture");

        assert_eq!(
            append_prompt_history_at(temp.path(), "never write this private prompt"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            load_prompt_history_at(temp.path(), 5),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
    }

    #[cfg(unix)]
    #[test]
    fn prompt_history_creates_and_compacts_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        let path = temp.path().join("agent_chat-prompt-history.jsonl");
        append_prompt_history_at(temp.path(), "private prompt")
            .expect("private prompt store initializes safely");
        let created_mode = std::fs::metadata(&path)
            .expect("private prompt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(created_mode, 0o600);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("legacy over-permissive prompt fixture");
        append_prompt_history_at(temp.path(), "repair legacy private permissions")
            .expect("legacy prompt store permissions become private before append");
        let repaired_mode = std::fs::metadata(&path)
            .expect("repaired private prompt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(repaired_mode, 0o600);

        let mut oversized = String::new();
        for index in 0..PROMPT_HISTORY_MAX_LINES * 2 {
            let line = PromptHistoryLine {
                timestamp: "2026-04-01T18:00:00Z".to_string(),
                prompt: format!("private-{index}"),
            };
            oversized.push_str(&serde_json::to_string(&line).expect("synthetic prompt line"));
            oversized.push('\n');
        }
        write_private_history_file_atomically(&path, &oversized)
            .expect("private oversized fixture");
        append_prompt_history_at(temp.path(), "final private prompt")
            .expect("atomic prompt compaction");
        let compacted_mode = std::fs::metadata(&path)
            .expect("compacted private prompt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(compacted_mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn history_index_repairs_legacy_permissions_before_appending_private_transcript() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("isolated conversation index fixture");
        let path = temp.path().join("agent_chat-history.jsonl");
        std::fs::write(&path, "").expect("legacy conversation index fixture");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("over-permissive legacy conversation index fixture");
        let conversation = make_conversation(
            "private-history-session",
            "2026-08-22T10:00:00Z",
            vec![("user", "private medical transcript")],
        );
        let entry = build_history_entry(&conversation).expect("real conversation index entry");

        save_history_entry_at(temp.path(), &entry)
            .expect("private transcript index repairs legacy permissions before append");

        let mode = std::fs::metadata(&path)
            .expect("repaired conversation index metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let persisted = std::fs::read_to_string(path).expect("private index remains readable");
        assert!(persisted.contains("private medical transcript"));
    }

    #[cfg(unix)]
    #[test]
    fn private_history_reads_repair_legacy_permissions_before_exposing_contents() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("isolated private-history fixture");
        for filename in [
            "agent_chat-history.jsonl",
            "agent_chat-prompt-history.jsonl",
            "saved-conversation.json",
        ] {
            let path = temp.path().join(filename);
            std::fs::write(&path, "legacy private user content")
                .expect("legacy private-history fixture");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
                .expect("over-permissive legacy private-history fixture");

            assert_eq!(
                read_private_history_file(&path)
                    .expect("private-history migration succeeds before content is returned"),
                "legacy private user content",
            );
            let mode = std::fs::metadata(&path)
                .expect("migrated private-history metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{filename} stayed world-readable");
        }
    }

    #[test]
    fn conversation_session_ids_preserve_real_formats_and_reject_traversal() {
        for valid in [
            "warm:8ecf16f4-c02a-4a2b-a4d2-a64c76d69303",
            "standard-agent-chat-mock-fixture",
            "legacy.session_42",
            "東京-session",
        ] {
            assert_eq!(validate_agent_chat_session_id(valid), Ok(()));
        }

        for invalid in [
            "",
            " ",
            ".",
            "..",
            "../escaped",
            "../../outside",
            "/absolute/session",
            "nested/session",
            "nested\\session",
            "C:outside",
            "C:\\outside",
            "line\nbreak",
            "nul\0value",
        ] {
            assert_eq!(
                validate_agent_chat_session_id(invalid),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
                "session ID must fail closed: {invalid:?}",
            );
        }
    }

    #[test]
    fn conversation_persistence_rejects_traversal_before_any_filesystem_mutation() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        let root_sibling = root.join("escaped.json");
        let external_sibling = temp.path().join("outside.json");
        std::fs::write(&root_sibling, "preserve kit sibling").expect("kit sibling fixture");
        std::fs::write(&external_sibling, "preserve external sibling")
            .expect("external sibling fixture");

        for session_id in [
            "../escaped",
            "../../outside",
            "/absolute/session",
            "nested\\escape",
            ".",
            "..",
            "nul\0escape",
        ] {
            let conversation = make_conversation(
                session_id,
                "2026-04-01T18:00:00Z",
                vec![("user", "safe fixture")],
            );
            assert_eq!(
                save_conversation_at(&root, &conversation),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
            );
            assert_eq!(
                conversation_exists_at(&root, session_id),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
            );
            assert!(matches!(
                load_conversation_at(&root, session_id),
                Err(AgentChatConversationPersistenceError::InvalidSessionId)
            ));
            assert!(rename_conversation_at(&root, session_id, "ignored").is_err());
            assert!(delete_conversation_at(&root, session_id).is_err());

            let entry = build_history_entry(&conversation).expect("synthetic history entry");
            assert_eq!(
                save_history_entry_at(&root, &entry),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
            );
        }

        assert!(!root.join("agent_chat-conversations").exists());
        assert!(!root.join("agent_chat-history.jsonl").exists());
        assert_eq!(
            std::fs::read_to_string(root_sibling).expect("kit sibling stays intact"),
            "preserve kit sibling",
        );
        assert_eq!(
            std::fs::read_to_string(external_sibling).expect("external sibling stays intact"),
            "preserve external sibling",
        );
    }

    #[test]
    fn conversation_persistence_round_trips_warm_ids_and_preserves_private_permissions() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let session_id = "warm:8ecf16f4-c02a-4a2b-a4d2-a64c76d69303";
        let conversation = make_conversation(
            session_id,
            "2026-04-01T18:00:00Z",
            vec![
                ("user", "private question"),
                ("assistant", "private answer"),
            ],
        );

        save_conversation_at(temp.path(), &conversation).expect("safe session saves");
        assert_eq!(conversation_exists_at(temp.path(), session_id), Ok(true));
        let loaded = load_conversation_at(temp.path(), session_id)
            .expect("safe session loads")
            .expect("saved session exists");
        assert_eq!(loaded.session_id, session_id);
        assert_eq!(loaded.messages.len(), 2);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let path = conversation_path_at(temp.path(), session_id).expect("safe path");
            let mode = std::fs::metadata(path)
                .expect("private conversation metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        rename_conversation_at(temp.path(), session_id, "Private Title")
            .expect("safe session renames");
        let renamed = load_conversation_at(temp.path(), session_id)
            .expect("renamed session loads")
            .expect("renamed session exists");
        assert_eq!(renamed.custom_title.as_deref(), Some("Private Title"));
        delete_conversation_at(temp.path(), session_id).expect("safe session deletes");
        assert_eq!(conversation_exists_at(temp.path(), session_id), Ok(false));
        assert_eq!(
            std::fs::read_to_string(temp.path().join("agent_chat-history.jsonl"))
                .expect("safe index remains readable"),
            "",
        );
    }

    #[test]
    fn conversation_deletion_removes_only_the_selected_sessions_private_attachments() {
        let root = tempfile::tempdir().expect("isolated conversation fixture");
        let session_id = "warm:owned-session";
        let conversation = make_conversation(
            session_id,
            "2026-04-01T18:00:00Z",
            vec![
                ("user", "private question"),
                ("assistant", "private answer"),
            ],
        );
        save_conversation_at(root.path(), &conversation).expect("save owned conversation");
        let directory = root.path().join("agent_chat-history-attachments");
        std::fs::create_dir(&directory).expect("attachment directory");
        let owned_summary = directory.join(format!("{session_id}-summary.md"));
        let owned_transcript = directory.join(format!("{session_id}-transcript.md"));
        let unrelated = directory.join("another-session-transcript.md");
        std::fs::write(&owned_summary, "private summary").unwrap();
        std::fs::write(&owned_transcript, "private complete transcript").unwrap();
        std::fs::write(&unrelated, "another owner's private transcript").unwrap();

        delete_conversation_at(root.path(), session_id)
            .expect("delete only the selected conversation and its attachments");

        assert!(!owned_summary.exists());
        assert!(!owned_transcript.exists());
        assert_eq!(
            std::fs::read_to_string(unrelated).unwrap(),
            "another owner's private transcript"
        );
    }

    #[cfg(unix)]
    #[test]
    fn conversation_deletion_rejects_attachment_symlink_before_any_private_store_changes() {
        let root = tempfile::tempdir().expect("isolated conversation fixture");
        let session_id = "owned-session";
        let conversation = make_conversation(
            session_id,
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );
        save_conversation_at(root.path(), &conversation).expect("save owned conversation");
        save_history_entry_at(
            root.path(),
            &build_history_entry(&conversation).expect("private index entry"),
        )
        .expect("save owned index entry");
        let directory = root.path().join("agent_chat-history-attachments");
        std::fs::create_dir(&directory).expect("attachment directory");
        let external = root.path().join("external-private.md");
        std::fs::write(&external, "never follow or delete me").unwrap();
        std::os::unix::fs::symlink(&external, directory.join("owned-session-transcript.md"))
            .expect("hostile attachment symlink");

        assert!(delete_conversation_at(root.path(), session_id).is_err());
        assert_eq!(conversation_exists_at(root.path(), session_id), Ok(true));
        assert!(
            std::fs::read_to_string(root.path().join("agent_chat-history.jsonl"))
                .unwrap()
                .contains(session_id)
        );
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            "never follow or delete me"
        );
    }

    #[test]
    fn conversation_load_rename_and_delete_reject_spoofed_payload_identity() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let directory = temp.path().join("agent_chat-conversations");
        std::fs::create_dir(&directory).expect("conversation directory fixture");
        let spoofed = make_conversation(
            "other-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "another user's conversation")],
        );
        let requested_path = directory.join("requested-session.json");
        let payload = serde_json::to_string(&spoofed).expect("spoofed payload fixture");
        std::fs::write(&requested_path, &payload).expect("spoofed session fixture");

        assert!(matches!(
            load_conversation_at(temp.path(), "requested-session"),
            Err(AgentChatConversationPersistenceError::SessionIdMismatch)
        ));
        assert!(rename_conversation_at(temp.path(), "requested-session", "Wrong Title").is_err());
        assert!(delete_conversation_at(temp.path(), "requested-session").is_err());
        assert_eq!(
            std::fs::read_to_string(requested_path).expect("spoofed payload remains untouched"),
            payload,
        );
    }

    #[cfg(unix)]
    #[test]
    fn conversation_persistence_never_follows_symlinked_session_targets() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        let directory = root.join("agent_chat-conversations");
        std::fs::create_dir_all(&directory).expect("conversation directory fixture");
        let external = temp.path().join("private-sibling.json");
        std::fs::write(&external, "untouched sibling secrets").expect("sibling fixture");
        let session_path = directory.join("safe-session.json");
        std::os::unix::fs::symlink(&external, &session_path)
            .expect("malicious session symlink fixture");

        let conversation = make_conversation(
            "safe-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );
        assert_eq!(
            save_conversation_at(&root, &conversation),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            conversation_exists_at(&root, "safe-session"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert!(matches!(
            load_conversation_at(&root, "safe-session"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget)
        ));
        assert!(rename_conversation_at(&root, "safe-session", "Wrong Title").is_err());
        assert!(delete_conversation_at(&root, "safe-session").is_err());
        assert_eq!(
            std::fs::read_to_string(external).expect("symlink destination remains untouched"),
            "untouched sibling secrets",
        );
        assert!(std::fs::symlink_metadata(session_path)
            .expect("session link remains untouched")
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn conversation_persistence_rejects_symlinked_conversation_directory() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        let external = temp.path().join("private-external-directory");
        std::fs::create_dir(&root).expect("isolated kit root");
        std::fs::create_dir(&external).expect("external directory fixture");
        std::os::unix::fs::symlink(&external, root.join("agent_chat-conversations"))
            .expect("malicious directory symlink fixture");
        let conversation = make_conversation(
            "safe-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );

        assert_eq!(
            save_conversation_at(&root, &conversation),
            Err(AgentChatConversationPersistenceError::UnsafeConversationDirectory),
        );
        assert_eq!(
            conversation_exists_at(&root, "safe-session"),
            Err(AgentChatConversationPersistenceError::UnsafeConversationDirectory),
        );
        assert!(load_conversation_at(&root, "safe-session").is_err());
        assert!(delete_conversation_at(&root, "safe-session").is_err());
        assert!(!external.join("safe-session.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn conversation_delete_preflights_symlinked_index_before_touching_saved_session() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        let conversation = make_conversation(
            "safe-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );
        save_conversation_at(&root, &conversation).expect("safe saved conversation fixture");
        let external = temp.path().join("unrelated-private-index.jsonl");
        std::fs::write(&external, "external history secrets").expect("external index fixture");
        std::os::unix::fs::symlink(&external, root.join("agent_chat-history.jsonl"))
            .expect("malicious index symlink fixture");

        assert!(delete_conversation_at(&root, "safe-session").is_err());
        assert!(rename_conversation_at(&root, "safe-session", "Wrong Title").is_err());
        let entry = build_history_entry(&conversation).expect("synthetic index entry");
        assert_eq!(
            save_history_entry_at(&root, &entry),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        let original = load_conversation_at(&root, "safe-session")
            .expect("original session remains readable")
            .expect("original session remains on disk");
        assert!(original.custom_title.is_none());
        assert_eq!(
            std::fs::read_to_string(external).expect("external index remains untouched"),
            "external history secrets",
        );
    }

    #[test]
    fn rename_conversation_updates_saved_conversation_and_index() {
        let _guard = history_env_lock().lock().expect("history env lock");
        let previous_sk_path = std::env::var(crate::setup::SK_PATH_ENV).ok();
        let temp = tempfile::tempdir().expect("temp dir");
        std::env::set_var(crate::setup::SK_PATH_ENV, temp.path());

        let conv = make_conversation(
            "rename-1",
            "2026-04-01T18:00:00Z",
            vec![("user", "please debug auth"), ("assistant", "I found it")],
        );
        save_conversation(&conv);
        save_history_entry(&build_history_entry(&conv).expect("entry"));

        rename_conversation("rename-1", r#"" Auth Debugging Plan! ""#).expect("rename");
        let saved = load_conversation("rename-1").expect("saved conversation");
        assert_eq!(saved.custom_title.as_deref(), Some("Auth Debugging Plan"));

        let entries = load_history();
        let entry = entries
            .iter()
            .find(|entry| entry.session_id == "rename-1")
            .expect("history entry");
        assert_eq!(entry.title_display(), "Auth Debugging Plan");

        match previous_sk_path {
            Some(path) => std::env::set_var(crate::setup::SK_PATH_ENV, path),
            None => std::env::remove_var(crate::setup::SK_PATH_ENV),
        }
        invalidate_history_cache();
    }

    // ── build_history_entry ─────────────────────────────────────────

    #[test]
    fn build_entry_populates_title_preview_search_text() {
        let conv = make_conversation(
            "build-1",
            "2026-04-01T10:00:00Z",
            vec![
                ("user", "help me fix login"),
                (
                    "assistant",
                    "The root cause is an expired OAuth redirect URI",
                ),
            ],
        );
        let entry = build_history_entry(&conv).expect("should build");
        assert_eq!(entry.title, "help me fix login");
        assert!(entry.custom_title.is_none());
        assert!(entry.preview.contains("expired OAuth redirect URI"));
        assert!(entry.search_text.contains("oauth"));
        assert!(entry.search_text.contains("redirect"));
        assert_eq!(entry.message_count, 2);
    }

    #[test]
    fn build_entry_returns_none_without_user_message() {
        let conv = make_conversation(
            "no-user",
            "2026-04-01T10:00:00Z",
            vec![("assistant", "hello")],
        );
        assert!(build_history_entry(&conv).is_none());
    }

    #[test]
    fn build_entry_uses_first_user_for_preview_when_no_assistant() {
        let conv = make_conversation(
            "user-only",
            "2026-04-01T10:00:00Z",
            vec![("user", "just a question")],
        );
        let entry = build_history_entry(&conv).expect("should build");
        assert_eq!(entry.preview, "just a question");
    }

    #[test]
    fn build_entry_truncates_title_at_100_chars() {
        let long_msg = "a".repeat(200);
        let conv = make_conversation(
            "long-title",
            "2026-04-01T10:00:00Z",
            vec![("user", &long_msg)],
        );
        let entry = build_history_entry(&conv).expect("should build");
        // 100 chars + ellipsis
        assert!(entry.title.chars().count() <= 101);
        assert!(entry.title.ends_with('\u{2026}'));
    }

    #[test]
    fn build_entry_truncates_preview_at_160_chars() {
        let long_reply = "b".repeat(300);
        let conv = make_conversation(
            "long-preview",
            "2026-04-01T10:00:00Z",
            vec![("user", "question"), ("assistant", &long_reply)],
        );
        let entry = build_history_entry(&conv).expect("should build");
        assert!(entry.preview.chars().count() <= 161);
    }

    // ── title_display / preview_display ─────────────────────────────

    #[test]
    fn title_display_falls_back_to_first_message() {
        let entry = AgentChatHistoryEntry {
            first_message: "fallback title".to_string(),
            ..Default::default()
        };
        assert_eq!(entry.title_display(), "fallback title");

        let custom = AgentChatHistoryEntry {
            first_message: "ignored".to_string(),
            title: "heuristic title".to_string(),
            custom_title: Some("Custom Title".to_string()),
            ..Default::default()
        };
        assert_eq!(custom.title_display(), "Custom Title");

        let entry2 = AgentChatHistoryEntry {
            first_message: "ignored".to_string(),
            title: "real title".to_string(),
            ..Default::default()
        };
        assert_eq!(entry2.title_display(), "real title");
    }

    #[test]
    fn preview_display_falls_back_to_first_message() {
        let entry = AgentChatHistoryEntry {
            first_message: "fallback preview".to_string(),
            ..Default::default()
        };
        assert_eq!(entry.preview_display(), "fallback preview");
    }

    // ── Text helpers ────────────────────────────────────────────────

    #[test]
    fn collapse_whitespace_normalizes() {
        assert_eq!(collapse_whitespace("  a  b  c  "), "a b c");
        assert_eq!(collapse_whitespace("hello\n\nworld"), "hello world");
    }

    #[test]
    fn truncate_chars_adds_ellipsis() {
        assert_eq!(truncate_chars("abcde", 3), "abc\u{2026}");
        assert_eq!(truncate_chars("ab", 5), "ab");
    }

    // ── rank_history_entries / search ────────────────────────────────

    fn sample_entries() -> Vec<AgentChatHistoryEntry> {
        vec![
            AgentChatHistoryEntry {
                timestamp: "2026-04-01T10:00:00Z".to_string(),
                first_message: "help me fix login".to_string(),
                message_count: 4,
                session_id: "s1".to_string(),
                title: "help me fix login".to_string(),
                custom_title: None,
                preview: "The root cause is an expired OAuth redirect URI".to_string(),
                search_text: normalize_search_text(
                    "help me fix login\nThe root cause is an expired OAuth redirect URI\nuser: help me fix login\nassistant: The root cause is an expired OAuth redirect URI",
                ),
            },
            AgentChatHistoryEntry {
                timestamp: "2026-04-02T10:00:00Z".to_string(),
                first_message: "add dark mode".to_string(),
                message_count: 3,
                session_id: "s2".to_string(),
                title: "add dark mode".to_string(),
                custom_title: None,
                preview: "I added CSS variables for theming".to_string(),
                search_text: normalize_search_text(
                    "add dark mode\nI added CSS variables for theming\nuser: add dark mode\nassistant: I added CSS variables for theming",
                ),
            },
            AgentChatHistoryEntry {
                timestamp: "2026-04-03T10:00:00Z".to_string(),
                first_message: "review PR 42".to_string(),
                message_count: 6,
                session_id: "s3".to_string(),
                title: "review PR 42".to_string(),
                custom_title: None,
                preview: "The PR looks good but the OAuth scope is too broad".to_string(),
                search_text: normalize_search_text(
                    "review PR 42\nThe PR looks good but the OAuth scope is too broad\nuser: review PR 42\nassistant: The PR looks good but the OAuth scope is too broad",
                ),
            },
        ]
    }

    #[test]
    fn empty_query_returns_all_up_to_limit() {
        let hits = rank_history_entries(sample_entries(), "", 100);
        assert_eq!(hits.len(), 3);
        // All scores should be 0 for empty query.
        assert!(hits.iter().all(|h| h.score == 0));
    }

    #[test]
    fn search_matches_later_transcript_content() {
        let hits = rank_history_entries(sample_entries(), "oauth redirect", 10);
        // "oauth redirect" appears in s1's preview and s3's preview.
        assert!(!hits.is_empty());
        // s1 has "redirect" in preview AND search_text → higher score.
        assert_eq!(hits[0].entry.session_id, "s1");
    }

    #[test]
    fn search_excludes_non_matching_entries() {
        let hits = rank_history_entries(sample_entries(), "nonexistent xyz", 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn search_is_case_insensitive() {
        let hits = rank_history_entries(sample_entries(), "OAUTH", 10);
        assert!(!hits.is_empty());
    }

    #[test]
    fn search_multi_token_requires_all_tokens() {
        // "dark" matches s2, "oauth" matches s1/s3 → no entry has both.
        let hits = rank_history_entries(sample_entries(), "dark oauth", 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn search_title_prefix_scores_highest() {
        let hits = rank_history_entries(sample_entries(), "help", 10);
        assert_eq!(hits[0].entry.session_id, "s1");
        assert_eq!(hits[0].matched_field, AgentChatHistorySearchField::Title);
    }

    #[test]
    fn search_respects_limit() {
        let hits = rank_history_entries(sample_entries(), "oauth", 1);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn search_recency_breaks_ties() {
        // Both s1 and s3 match "oauth", but with different scores.
        // If scores tied, s3 (later timestamp) would come first.
        let mut entries = sample_entries();
        // Make s1 and s3 have identical search_text so score is equal.
        let shared_text = normalize_search_text("oauth common content");
        entries[0].search_text = shared_text.clone();
        entries[0].title = "oauth common content".to_string();
        entries[0].preview = "oauth common content".to_string();
        entries[2].search_text = shared_text;
        entries[2].title = "oauth common content".to_string();
        entries[2].preview = "oauth common content".to_string();

        let hits = rank_history_entries(entries, "oauth", 10);
        assert!(hits.len() >= 2);
        // s3 has later timestamp → should come first when scores tie.
        assert_eq!(hits[0].entry.session_id, "s3");
        assert_eq!(hits[1].entry.session_id, "s1");
    }

    #[test]
    fn search_whitespace_only_query_returns_all() {
        let hits = rank_history_entries(sample_entries(), "   ", 100);
        assert_eq!(hits.len(), 3);
    }

    /// Screenshot regression (2026-07-11): "what are the" must not surface
    /// conversations whose only hits are stopwords scattered mid-word or
    /// across distant transcript turns.
    #[test]
    fn sentence_query_rejects_scattered_stopword_noise() {
        let noise = AgentChatHistoryEntry {
            timestamp: "2026-07-11T10:00:00Z".to_string(),
            first_message: "Explain keyboard-first macOS launchers".to_string(),
            message_count: 5,
            session_id: "noise".to_string(),
            title: "Explain keyboard-first macOS launchers".to_string(),
            custom_title: None,
            preview: "Somewhat shared themes and other reports are generated".to_string(),
            search_text: normalize_search_text(
                "Explain keyboard-first macOS launchers\nuser: what happened\nassistant: many unrelated words separate everything here from anything useful and more filler keeps going until eventually are appears and then much later after so much more filler text the final token shows up: the",
            ),
        };
        let phrase = AgentChatHistoryEntry {
            timestamp: "2026-07-01T10:00:00Z".to_string(),
            first_message: "What are the release criteria?".to_string(),
            message_count: 2,
            session_id: "phrase".to_string(),
            title: "What are the release criteria?".to_string(),
            custom_title: None,
            preview: "Ship gates are green".to_string(),
            search_text: normalize_search_text("What are the release criteria?"),
        };

        let hits = rank_history_entries(vec![noise, phrase], "what are the", 10);
        assert_eq!(hits.len(), 1, "only the visible phrase row qualifies");
        assert_eq!(hits[0].entry.session_id, "phrase");
        let evidence = hits[0].evidence.as_ref().expect("evidence present");
        assert!(
            !evidence.title_indices.is_empty(),
            "phrase match highlights the title words"
        );
    }

    /// Hidden transcript matches still qualify, rank below visible phrase
    /// rows, and carry an excerpt explaining why they matched.
    #[test]
    fn hidden_transcript_match_ranks_below_visible_and_carries_excerpt() {
        let hidden = AgentChatHistoryEntry {
            timestamp: "2026-07-11T10:00:00Z".to_string(),
            first_message: "Planning session".to_string(),
            message_count: 8,
            session_id: "hidden".to_string(),
            title: "Planning session".to_string(),
            custom_title: None,
            preview: "Sounds good, next steps agreed".to_string(),
            search_text: normalize_search_text(
                "Planning session\nuser: so what are the migration constraints for launch\nassistant: mostly disk budget",
            ),
        };
        let visible = AgentChatHistoryEntry {
            timestamp: "2026-06-01T10:00:00Z".to_string(),
            first_message: "What are the migration constraints?".to_string(),
            message_count: 2,
            session_id: "visible".to_string(),
            title: "What are the migration constraints?".to_string(),
            custom_title: None,
            preview: "Disk budget mostly".to_string(),
            search_text: normalize_search_text("What are the migration constraints?"),
        };

        let hits = rank_history_entries(vec![hidden, visible], "what are the migration", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].entry.session_id, "visible",
            "visible phrase must outrank hidden transcript despite being older"
        );
        let hidden_evidence = hits[1].evidence.as_ref().expect("evidence present");
        assert!(hidden_evidence.title_indices.is_empty());
        let excerpt = hidden_evidence
            .hidden_excerpt
            .as_ref()
            .expect("hidden match explains itself");
        assert!(excerpt.text.contains("migration"));
    }
}

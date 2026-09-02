//! Agent Chat conversation history persistence.
//!
//! - `agent_chat-history.jsonl` — One-line summaries for Cmd+P browsing
//! - `agent_chat-conversations/{session_id}.json` — Full message history for resume
//! - `agent_chat-prompt-history.jsonl` — Flat list of submitted composer
//!   prompts for shell-style Up/Down recall across sessions

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

type HistoryFileSignature = Option<(std::path::PathBuf, std::time::SystemTime, u64)>;
const HISTORY_SEARCH_TEXT_MAX_CHARS: usize = 4096;
const ROOT_AGENT_CHAT_HISTORY_REFRESH_LABEL: &str = "root-agent_chat-history-cache";

#[derive(Clone)]
struct AgentChatHistoryIndexCache {
    signature: HistoryFileSignature,
    owned: bool,
    owned_fresh: bool,
    entries: Vec<AgentChatHistoryEntry>,
}

type AgentChatHistoryRefreshLifecycle =
    crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle;
pub(crate) type RootAgentChatHistoryRefresh =
    crate::scripts::root_search_contract::RootOwnedProviderRefresh;

pub(crate) struct RootAgentChatHistorySnapshot {
    cache: anyhow::Result<AgentChatHistoryIndexCache>,
}

impl RootAgentChatHistorySnapshot {
    pub(crate) fn read_outcome(&self) -> Result<usize, &anyhow::Error> {
        self.cache.as_ref().map(|cache| cache.entries.len())
    }
}

static AGENT_CHAT_HISTORY_INDEX_CACHE: OnceLock<Mutex<Option<AgentChatHistoryIndexCache>>> =
    OnceLock::new();
// Publication revision, advanced under AGENT_CHAT_HISTORY_INDEX_CACHE's lock.
static AGENT_CHAT_HISTORY_CACHE_REVISION: AtomicU64 = AtomicU64::new(0);
static AGENT_CHAT_HISTORY_WRITE_LOCK: Mutex<()> = Mutex::new(());
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

fn agent_chat_history_cache_is_fresh(cache: &AgentChatHistoryIndexCache) -> bool {
    if crate::runtime_policy::is_owned_evaluation() {
        return cache.owned && cache.owned_fresh;
    }
    history_file_signature(&history_path()).is_ok_and(|signature| cache.signature == signature)
}

/// Accepted snapshot publication revision and row count, never a worker identity.
pub(crate) fn root_agent_chat_history_fresh_cache_status() -> Option<(u64, usize)> {
    let lifecycle = agent_chat_history_refresh_lifecycle().try_lock().ok()?;
    if lifecycle.in_flight.is_some() {
        return None;
    }
    let guard = agent_chat_history_index_cache().try_lock().ok()?;
    let cache = guard.as_ref()?;
    let revision = AGENT_CHAT_HISTORY_CACHE_REVISION.load(Ordering::Relaxed);
    (revision != 0 && agent_chat_history_cache_is_fresh(cache))
        .then_some((revision, cache.entries.len()))
}

fn cached_history_entries_if_fresh() -> Option<Vec<AgentChatHistoryEntry>> {
    let guard = agent_chat_history_index_cache().lock().ok()?;
    let cache = guard.as_ref()?;
    agent_chat_history_cache_is_fresh(cache).then(|| cache.entries.clone())
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
    let parsed = match read_private_history_file(path) {
        Ok(content) => parse_history_entries(&content),
        Err(AgentChatConversationPersistenceError::Io(std::io::ErrorKind::NotFound)) => {
            Ok(Vec::new())
        }
        Err(error) => Err(error),
    };
    RootAgentChatHistorySnapshot {
        cache: parsed.map_err(anyhow::Error::from).and_then(|entries| {
            Ok(AgentChatHistoryIndexCache {
                signature: history_file_signature(path)?,
                owned: false,
                owned_fresh: true,
                entries,
            })
        }),
    }
}

pub(crate) fn read_root_agent_chat_history_snapshot() -> RootAgentChatHistorySnapshot {
    assert!(
        !crate::runtime_policy::is_owned_evaluation(),
        "owned_source_snapshot_required"
    );
    tracing::debug!(
        target: "script_kit::search",
        worker = ROOT_AGENT_CHAT_HISTORY_REFRESH_LABEL,
        "Reading owned private conversation history snapshot"
    );
    read_root_agent_chat_history_snapshot_at(&history_path())
}

pub(crate) fn owned_root_agent_chat_history_snapshot(
    result: anyhow::Result<Vec<AgentChatHistoryEntry>>,
) -> anyhow::Result<RootAgentChatHistorySnapshot> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    Ok(RootAgentChatHistorySnapshot {
        cache: result.map(|entries| AgentChatHistoryIndexCache {
            signature: None,
            owned: true,
            owned_fresh: true,
            entries,
        }),
    })
}

pub(crate) fn invalidate_owned_root_agent_chat_history_freshness() -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut lifecycle = agent_chat_history_refresh_lifecycle()
        .lock()
        .map_err(|_| anyhow::anyhow!("conversation_lifecycle_poisoned"))?;
    let mut cache = agent_chat_history_index_cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("conversation_cache_poisoned"))?;
    lifecycle.next_generation = lifecycle
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("conversation_generation_exhausted"))?;
    lifecycle.in_flight = None;
    if let Some(cache) = cache.as_mut() {
        cache.owned_fresh = false;
    }
    Ok(())
}

pub(crate) fn reset_owned_root_agent_chat_history() -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut lifecycle = agent_chat_history_refresh_lifecycle()
        .lock()
        .map_err(|_| anyhow::anyhow!("conversation_lifecycle_poisoned"))?;
    let mut cache = agent_chat_history_index_cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("conversation_cache_poisoned"))?;
    lifecycle.next_generation = lifecycle
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("conversation_generation_exhausted"))?;
    lifecycle.in_flight = None;
    *cache = None;
    Ok(())
}

fn root_agent_chat_history_snapshot_is_current_at(
    snapshot: &RootAgentChatHistorySnapshot,
    path: &std::path::Path,
) -> bool {
    snapshot.cache.as_ref().is_ok_and(|cache| {
        history_file_signature(path).is_ok_and(|signature| cache.signature == signature)
    })
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

    let owned = crate::runtime_policy::is_owned_evaluation();
    if !owned && !root_agent_chat_history_snapshot_is_current_at(&snapshot, &history_path()) {
        return false;
    }
    let Ok(snapshot) = snapshot.cache else {
        return false;
    };
    if snapshot.owned != owned {
        return false;
    }
    let Ok(mut cache) = agent_chat_history_index_cache().lock() else {
        return false;
    };
    *cache = Some(snapshot);
    AGENT_CHAT_HISTORY_CACHE_REVISION.fetch_add(1, Ordering::Relaxed);
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
/// Read the last accepted JSONL index without IO, including while refresh is
/// pending or failed. The input owner checks freshness and publishes only a
/// successfully completed generation-fenced replacement.
pub(crate) fn search_history_cached(query: &str, limit: usize) -> Vec<AgentChatHistorySearchHit> {
    let entries = agent_chat_history_index_cache()
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .as_ref()
                .filter(|cache| cache.owned == crate::runtime_policy::is_owned_evaluation())
                .map(|cache| cache.entries.clone())
        });
    let Some(entries) = entries else {
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
pub(super) fn append_prompt_history(
    prompt: &str,
) -> Result<(), AgentChatConversationPersistenceError> {
    append_prompt_history_at(&crate::setup::get_kit_path(), prompt)
}

fn append_prompt_history_at(
    kit_root: &std::path::Path,
    prompt: &str,
) -> Result<(), AgentChatConversationPersistenceError> {
    with_agent_chat_history_write_lock(|| append_prompt_history_at_locked(kit_root, prompt))
}

fn append_prompt_history_at_locked(
    kit_root: &std::path::Path,
    prompt: &str,
) -> Result<(), AgentChatConversationPersistenceError> {
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

    crate::atomic_file::append_private_jsonl_record(&path, json.as_bytes())
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;

    let content = read_private_history_file(&path)?;
    let entries = parse_prompt_history_lines(&content)?;
    if entries.len() > PROMPT_HISTORY_MAX_LINES * 2 {
        let keep: Vec<&PromptHistoryLine> = entries
            .iter()
            .rev()
            .take(PROMPT_HISTORY_MAX_LINES)
            .collect();
        let mut rewritten = String::new();
        for entry in keep.iter().rev() {
            let json = serde_json::to_string(entry)
                .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;
            rewritten.push_str(&json);
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
    let mut prompts: Vec<String> = parse_prompt_history_lines(&content)?
        .into_iter()
        .rev()
        .map(|line| line.prompt)
        .take(limit)
        .collect();
    prompts.reverse();
    Ok(prompts)
}

fn parse_prompt_history_lines(
    content: &str,
) -> Result<Vec<PromptHistoryLine>, AgentChatConversationPersistenceError> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<PromptHistoryLine>(line)
                .map_err(|_| AgentChatConversationPersistenceError::InvalidPromptHistoryPayload)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChatConversationPersistenceError {
    InvalidSessionId,
    UnsafeConversationDirectory,
    UnsafeFileTarget,
    SessionIdMismatch,
    SerializationFailed,
    InvalidConversationPayload,
    InvalidHistoryIndexPayload,
    InvalidPromptHistoryPayload,
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
            Self::InvalidHistoryIndexPayload => {
                formatter.write_str("invalid saved Agent Chat conversation index")
            }
            Self::InvalidPromptHistoryPayload => {
                formatter.write_str("invalid saved Agent Chat prompt history")
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

fn with_agent_chat_history_write_lock<T, E>(
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let _guard = AGENT_CHAT_HISTORY_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    operation()
}

fn save_history_entry_at(
    kit_root: &std::path::Path,
    entry: &AgentChatHistoryEntry,
) -> Result<(), AgentChatConversationPersistenceError> {
    validate_agent_chat_session_id(&entry.session_id)?;
    let path = kit_root.join("agent_chat-history.jsonl");
    let json = serde_json::to_string(entry)
        .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;
    if inspect_regular_history_file(&path)? {
        let existing = read_private_history_file(&path)?;
        parse_history_entries(&existing)?;
    }
    crate::atomic_file::append_private_jsonl_record(&path, json.as_bytes())
        .map_err(|error| AgentChatConversationPersistenceError::Io(error.kind()))?;
    invalidate_history_cache();

    // Compact when file grows too large (>200 lines)
    let content = read_private_history_file(&path)?;
    let compacted = parse_history_entries(&content)?;
    if content.lines().count() > 200 {
        let mut rewritten = String::new();
        for compacted_entry in compacted.iter().rev() {
            let json = serde_json::to_string(compacted_entry)
                .map_err(|_| AgentChatConversationPersistenceError::SerializationFailed)?;
            rewritten.push_str(&json);
            rewritten.push('\n');
        }
        write_private_history_file_atomically(&path, &rewritten)?;
        invalidate_history_cache();
    }

    Ok(())
}

/// Seed bounded owned data through the production completed-turn transaction.
pub(crate) fn seed_owned_history(conversations: &[SavedConversation]) -> anyhow::Result<()> {
    let scope = crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("owned_history_required"))?;
    scope.require_owned_path(&crate::setup::get_kit_path())?;
    anyhow::ensure!(conversations.len() <= 32, "history_fixture_limit");
    for conversation in conversations {
        let entry = build_history_entry(conversation)
            .ok_or_else(|| anyhow::anyhow!("fixture_history_has_no_user_turn"))?;
        if !conversation_exists(&conversation.session_id) {
            save_completed_conversation(conversation, &entry)?;
        }
    }
    Ok(())
}

/// Save a complete turn and its searchable index as one serialized transaction.
pub(super) fn save_completed_conversation(
    conversation: &SavedConversation,
    entry: &AgentChatHistoryEntry,
) -> Result<(), AgentChatConversationPersistenceError> {
    save_completed_conversation_at(&crate::setup::get_kit_path(), conversation, entry)
}

fn save_completed_conversation_at(
    kit_root: &std::path::Path,
    conversation: &SavedConversation,
    entry: &AgentChatHistoryEntry,
) -> Result<(), AgentChatConversationPersistenceError> {
    with_agent_chat_history_write_lock(|| {
        if conversation.session_id != entry.session_id {
            return Err(AgentChatConversationPersistenceError::SessionIdMismatch);
        }
        let index = kit_root.join("agent_chat-history.jsonl");
        if inspect_regular_history_file(&index)? {
            let existing = read_private_history_file(&index)?;
            parse_history_entries(&existing)?;
        }
        save_conversation_at(kit_root, conversation)?;
        save_history_entry_at(kit_root, entry)?;
        cleanup_old_conversations_at(kit_root, 50);
        Ok(())
    })
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
            if signature
                .as_ref()
                .is_ok_and(|signature| &cache.signature == signature)
            {
                return cache.entries.clone();
            }
        }
    }

    let snapshot = read_root_agent_chat_history_snapshot_at(&path);
    let snapshot = match snapshot.cache {
        Ok(cache) => cache,
        Err(error) => {
            tracing::warn!(
                error_bytes = error.to_string().len(),
                "agent_chat_history_read_failed"
            );
            return agent_chat_history_index_cache()
                .lock()
                .ok()
                .and_then(|cache| cache.as_ref().map(|cache| cache.entries.clone()))
                .unwrap_or_default();
        }
    };

    let entries = snapshot.entries.clone();
    if let Ok(mut guard) = agent_chat_history_index_cache().lock() {
        *guard = Some(snapshot);
        AGENT_CHAT_HISTORY_CACHE_REVISION.fetch_add(1, Ordering::Relaxed);
    }

    entries
}

fn history_file_signature(path: &std::path::Path) -> std::io::Result<HistoryFileSignature> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    Ok(Some((
        path.to_path_buf(),
        metadata.modified()?,
        metadata.len(),
    )))
}

fn parse_history_entries(
    content: &str,
) -> Result<Vec<AgentChatHistoryEntry>, AgentChatConversationPersistenceError> {
    let mut entries = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut entry: AgentChatHistoryEntry = serde_json::from_str(line)
            .map_err(|_| AgentChatConversationPersistenceError::InvalidHistoryIndexPayload)?;
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
        entries.push(entry);
    }

    // Most recent first, then deduplicate (keeps latest per session_id)
    entries.reverse();
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.session_id.clone()));
    entries.truncate(100);
    Ok(entries)
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
    with_agent_chat_history_write_lock(|| {
        rename_conversation_at_locked(kit_root, session_id, new_title)
    })
}

fn rename_conversation_at_locked(
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
    let index = kit_root.join("agent_chat-history.jsonl");
    if inspect_regular_history_file(&index)? {
        let existing = read_private_history_file(&index)?;
        parse_history_entries(&existing)?;
    }
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
    with_agent_chat_history_write_lock(|| delete_conversation_at_locked(kit_root, session_id))
}

fn delete_conversation_at_locked(
    kit_root: &std::path::Path,
    session_id: &str,
) -> anyhow::Result<()> {
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
        let entries = parse_history_entries(&content)?;
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
fn cleanup_old_conversations_at(kit_root: &std::path::Path, keep: usize) {
    let dir = kit_root.join("agent_chat-conversations");
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
#[test]
fn owned_conversation_snapshots_and_resets_require_runtime_authority() {
    assert!(owned_root_agent_chat_history_snapshot(Ok(Vec::new())).is_err());
    assert!(reset_owned_root_agent_chat_history().is_err());
    assert!(invalidate_owned_root_agent_chat_history_freshness().is_err());
}

#[cfg(test)]
include!("history_tests.rs");

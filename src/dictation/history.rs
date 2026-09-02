//! Persistent dictation history and Agent Chat-facing provider payloads.

use crate::dictation::DictationTarget;
use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const HISTORY_COMPACT_LIMIT: usize = 200;
const RESOURCE_ITEMS_LIMIT: usize = 10;
const ROOT_DICTATION_HISTORY_REFRESH_LABEL: &str = "root-dictation-history-cache";
pub const DICTATION_HISTORY_ENTRY_VERSION: u32 = 2;
pub const DICTATION_HISTORY_PAGE_SIZE: usize = 100;
pub const DICTATION_HISTORY_LEGACY_UNKNOWN_TARGET_ID: &str = "legacy-unknown";

fn dictation_history_entry_version() -> u32 {
    DICTATION_HISTORY_ENTRY_VERSION
}

type HistoryFileSignature = Option<(std::path::PathBuf, std::time::SystemTime, u64)>;

#[derive(Clone)]
struct DictationHistoryIndexCache {
    signature: HistoryFileSignature,
    owned: bool,
    owned_fresh: bool,
    entries: Vec<DictationHistoryEntry>,
}

pub(crate) type RootDictationHistoryRefresh =
    crate::scripts::root_search_contract::RootOwnedProviderRefresh;

pub(crate) struct RootDictationHistorySnapshot {
    cache: anyhow::Result<DictationHistoryIndexCache>,
}

impl RootDictationHistorySnapshot {
    #[allow(dead_code)] // Root search completion receipts use this through the binary app layer.
    pub(crate) fn read_outcome(&self) -> Result<usize, &anyhow::Error> {
        self.cache.as_ref().map(|cache| cache.entries.len())
    }
}

static DICTATION_HISTORY_INDEX_CACHE: OnceLock<Mutex<Option<DictationHistoryIndexCache>>> =
    OnceLock::new();
// Publication revision, advanced under DICTATION_HISTORY_INDEX_CACHE's lock.
static DICTATION_HISTORY_CACHE_REVISION: AtomicU64 = AtomicU64::new(0);
static DICTATION_HISTORY_WRITE_LOCK: Mutex<()> = Mutex::new(());
static DICTATION_HISTORY_REFRESH_LIFECYCLE: OnceLock<
    Mutex<crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle>,
> = OnceLock::new();

fn dictation_history_index_cache() -> &'static Mutex<Option<DictationHistoryIndexCache>> {
    DICTATION_HISTORY_INDEX_CACHE.get_or_init(|| Mutex::new(None))
}

fn dictation_history_refresh_lifecycle(
) -> &'static Mutex<crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle> {
    DICTATION_HISTORY_REFRESH_LIFECYCLE.get_or_init(|| {
        Mutex::new(
            crate::scripts::root_search_contract::RootOwnedProviderRefreshLifecycle::default(),
        )
    })
}

fn invalidate_history_cache() {
    if let Some(cache) = DICTATION_HISTORY_INDEX_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            *guard = None;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictationHistoryEntry {
    #[serde(default = "dictation_history_entry_version")]
    pub version: u32,
    pub id: String,
    pub timestamp: String,
    pub transcript: String,
    pub preview: String,
    #[serde(default)]
    pub target_id: String,
    #[serde(default, alias = "target")]
    pub target_label_snapshot: String,
    pub audio_duration_ms: u64,
}

pub fn delete_history_confirmation_body(pending_in_agent_chat: bool) -> &'static str {
    if pending_in_agent_chat {
        "This transcript is staged in Agent Chat. Deleting it will make that pending attachment unavailable. Sent-turn receipts are preserved."
    } else {
        "Delete this saved transcript from Dictation History? Sent-turn receipts are preserved."
    }
}

impl DictationHistoryEntry {
    /// One stable row identity shared by painting, semantic projection, and
    /// scroll receipts. Transcript/preview text and transient list indexes
    /// must never decide whether two receipts describe the same saved entry.
    pub fn semantic_id(&self) -> String {
        format!("dictation-history:{}", self.id)
    }

    pub fn canonical_target(&self) -> Option<DictationTarget> {
        crate::dictation::parse_dictation_target_label(&self.target_id)
    }

    pub fn display_target_label(&self) -> String {
        if let Some(target) = self.canonical_target() {
            if target == DictationTarget::ExternalApp
                && !self.target_label_snapshot.trim().is_empty()
            {
                return self.target_label_snapshot.clone();
            }
            return target.descriptor().selector_label.to_string();
        }
        if self.target_label_snapshot.trim().is_empty() {
            "Unknown destination".to_string()
        } else {
            self.target_label_snapshot.clone()
        }
    }

    pub fn resource_uri(&self) -> String {
        format!("kit://dictation-history?id={}", self.id)
    }
}

/// Reconcile target-scoped semantic projection with the exact entry owner
/// used by the actual painted row and the active scroll receipt.
pub fn apply_dictation_history_row_identities(
    elements: &mut [crate::protocol::ElementInfo],
    entries: &[DictationHistoryEntry],
) {
    let rows = elements
        .iter_mut()
        .filter(|element| element.element_type == crate::protocol::ElementType::Choice);
    for (element, entry) in rows.zip(entries) {
        element.semantic_id = entry.semantic_id();
        element.source = Some("dictationHistory".to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationHistoryPage {
    pub total_matches: usize,
    pub visible_count: usize,
    pub offset: usize,
    pub rows: Vec<DictationHistoryEntry>,
    pub has_more: bool,
}

impl DictationHistoryPage {
    pub fn count_label(&self) -> String {
        format!("Showing {} of {}", self.visible_count, self.total_matches)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationHistoryViewState {
    Loading {
        previous: Option<DictationHistoryPage>,
    },
    Failed {
        message: String,
        previous: Option<DictationHistoryPage>,
    },
    NoSavedDictation,
    NoFilteredMatches,
    Ready(DictationHistoryPage),
}

impl DictationHistoryViewState {
    pub fn page(&self) -> Option<&DictationHistoryPage> {
        match self {
            Self::Loading { previous } | Self::Failed { previous, .. } => previous.as_ref(),
            Self::Ready(page) => Some(page),
            Self::NoSavedDictation | Self::NoFilteredMatches => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationHistorySearchField {
    Transcript,
    Target,
    Timestamp,
}

#[derive(Debug, Clone)]
pub struct DictationHistorySearchHit {
    pub entry: DictationHistoryEntry,
    pub score: u32,
    pub matched_field: DictationHistorySearchField,
    /// Word-level match evidence produced at qualification time; renderers
    /// highlight exactly these ranges. `None` for empty-query recency rows.
    pub evidence: Option<crate::scripts::search::sentence::LongTextMatchEvidence>,
}

#[derive(Debug, Clone)]
pub struct RootDictationHistorySearchHit {
    pub id: String,
    pub preview: String,
    pub target: String,
    pub timestamp: String,
    pub audio_duration_ms: u64,
    pub score: u32,
    pub matched_field: DictationHistorySearchField,
    pub evidence: Option<crate::scripts::search::sentence::LongTextMatchEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootDictationHistorySectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
    pub scan_limit: usize,
}

impl Default for RootDictationHistorySectionOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_results: 0,
            min_query_chars: usize::MAX,
            scan_limit: 0,
        }
    }
}

fn history_path() -> std::path::PathBuf {
    crate::setup::get_kit_path().join("dictation-history.jsonl")
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push('\u{2026}');
    }
    out
}

fn target_label(target: DictationTarget) -> String {
    let target = match target {
        DictationTarget::AiChatComposer => DictationTarget::TabAiHarness,
        target => target,
    };
    if target == DictationTarget::ExternalApp {
        return crate::frontmost_app_tracker::get_last_real_app()
            .map(|app| app.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| target.descriptor().selector_label.to_string());
    }

    target.descriptor().selector_label.to_string()
}

pub fn format_history_timestamp(timestamp: &str) -> String {
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return timestamp.to_string();
    };

    let localized = parsed.with_timezone(&Local);
    let now = Local::now();
    let format = if localized.year() == now.year() {
        "%b %-d at %-I:%M %P"
    } else {
        "%b %-d, %Y at %-I:%M %P"
    };

    localized.format(format).to_string()
}

pub fn format_history_duration_ms(audio_duration_ms: u64) -> String {
    match audio_duration_ms {
        0..=999 => "under 1 sec".to_string(),
        1_000..=9_999 => format!("{:.1} sec", audio_duration_ms as f64 / 1_000.0),
        10_000..=59_999 => format!("{} sec", (audio_duration_ms + 500) / 1_000),
        _ => {
            let total_seconds = (audio_duration_ms + 500) / 1_000;
            let hours = total_seconds / 3_600;
            let minutes = (total_seconds % 3_600) / 60;
            let seconds = total_seconds % 60;

            if hours > 0 {
                if seconds == 0 {
                    format!("{hours} hr {minutes} min")
                } else {
                    format!("{hours} hr {minutes} min {seconds} sec")
                }
            } else if seconds == 0 {
                format!("{minutes} min")
            } else {
                format!("{minutes} min {seconds} sec")
            }
        }
    }
}

pub fn build_history_entry(
    transcript: &str,
    audio_duration: Duration,
    target: DictationTarget,
) -> DictationHistoryEntry {
    let now = chrono::Utc::now();
    let timestamp = now.to_rfc3339();
    let normalized = collapse_whitespace(transcript);
    let id = format!(
        "dictation-{}-{}",
        now.format("%Y%m%dT%H%M%S%.3fZ"),
        uuid::Uuid::new_v4().simple()
    );

    DictationHistoryEntry {
        version: DICTATION_HISTORY_ENTRY_VERSION,
        id,
        timestamp,
        preview: truncate_chars(&normalized, 120),
        transcript: transcript.trim().to_string(),
        target_id: target.sticky_label().to_string(),
        target_label_snapshot: target_label(target),
        audio_duration_ms: audio_duration.as_millis() as u64,
    }
}

fn write_history_at(
    path: &std::path::Path,
    entries: &[DictationHistoryEntry],
) -> std::io::Result<()> {
    let mut content = String::new();
    for entry in entries {
        let json = serde_json::to_string(entry).map_err(std::io::Error::other)?;
        content.push_str(&json);
        content.push('\n');
    }
    crate::atomic_file::write_private_atomic(path, content.as_bytes())?;
    invalidate_history_cache();
    Ok(())
}

fn write_history(entries: &[DictationHistoryEntry]) -> std::io::Result<()> {
    write_history_at(&history_path(), entries)
}

fn save_history_entry_at(
    path: &std::path::Path,
    entry: &DictationHistoryEntry,
) -> std::io::Result<()> {
    let json = serde_json::to_string(entry).map_err(std::io::Error::other)?;
    crate::atomic_file::append_private_jsonl_record(path, json.as_bytes())?;
    invalidate_history_cache();
    Ok(())
}

fn with_history_write_lock<T>(
    operation: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<T> {
    let _guard = DICTATION_HISTORY_WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    operation()
}

pub fn seed_owned_dictation_history(entries: &[DictationHistoryEntry]) -> anyhow::Result<()> {
    let scope = crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("owned_history_required"))?;
    scope.require_owned_path(&history_path())?;
    anyhow::ensure!(entries.len() <= 32, "history_fixture_limit");
    let current = load_history_result()?;
    for entry in entries {
        if !current.iter().any(|old| old.id == entry.id) {
            save_history_entry(entry)?;
        }
    }
    Ok(())
}

fn save_history_entry(entry: &DictationHistoryEntry) -> std::io::Result<()> {
    let path = history_path();
    with_history_write_lock(|| {
        save_history_entry_at(&path, entry)?;
        let mut entries = load_history_result_at(&path)?;
        if entries.len() > HISTORY_COMPACT_LIMIT {
            entries.truncate(HISTORY_COMPACT_LIMIT);
            let rewritten: Vec<DictationHistoryEntry> = entries.iter().cloned().rev().collect();
            write_history_at(&path, &rewritten)?;
        }
        entries.truncate(RESOURCE_ITEMS_LIMIT);
        refresh_published_resource_from_entries(&entries);
        Ok(())
    })
    .inspect_err(|error| {
        let safe_path = crate::logging::log_private_user_value(&path.display().to_string());
        tracing::warn!(
            category = "DICTATION",
            path_bytes = safe_path.raw_bytes,
            path_sha256 = %safe_path.sha256,
            reason = ?error.kind(),
            "dictation_history_write_failed"
        );
    })
}

pub fn load_history_result() -> std::io::Result<Vec<DictationHistoryEntry>> {
    if std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() == Some("1")
        && std::env::var("SCRIPT_KIT_TEST_DICTATION_HISTORY_LOAD_FAILURE")
            .ok()
            .as_deref()
            == Some("1")
    {
        return Err(std::io::Error::other(
            "deterministic Dictation History load failure",
        ));
    }

    load_history_result_at(&history_path())
}

fn load_history_result_at(path: &std::path::Path) -> std::io::Result<Vec<DictationHistoryEntry>> {
    let exists = crate::atomic_file::inspect_private_file(path)?;
    let signature = history_file_signature(path)?;
    if let Ok(guard) = dictation_history_index_cache().lock() {
        if let Some(cache) = guard.as_ref() {
            if cache.signature == signature {
                return Ok(cache.entries.clone());
            }
        }
    }

    let entries = if exists {
        parse_history_entries(&crate::atomic_file::read_private_file(path)?)?
    } else {
        Vec::new()
    };

    if let Ok(mut guard) = dictation_history_index_cache().lock() {
        *guard = Some(DictationHistoryIndexCache {
            signature,
            owned: false,
            owned_fresh: true,
            entries: entries.clone(),
        });
        DICTATION_HISTORY_CACHE_REVISION.fetch_add(1, Ordering::Relaxed);
    }

    Ok(entries)
}

pub fn load_history() -> Vec<DictationHistoryEntry> {
    load_history_result().unwrap_or_default()
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

fn migrate_history_entry(mut entry: DictationHistoryEntry) -> DictationHistoryEntry {
    let parsed = (!entry.target_id.trim().is_empty())
        .then(|| crate::dictation::parse_dictation_target_label(&entry.target_id))
        .flatten()
        .or_else(|| crate::dictation::parse_dictation_target_label(&entry.target_label_snapshot));

    if let Some(target) = parsed {
        entry.target_id = target.sticky_label().to_string();
        if target != DictationTarget::ExternalApp || entry.target_label_snapshot.trim().is_empty() {
            entry.target_label_snapshot = target.descriptor().selector_label.to_string();
        }
    } else {
        entry.target_id = DICTATION_HISTORY_LEGACY_UNKNOWN_TARGET_ID.to_string();
    }
    entry.version = DICTATION_HISTORY_ENTRY_VERSION;
    entry
}

fn parse_history_entries(content: &str) -> std::io::Result<Vec<DictationHistoryEntry>> {
    let mut entries = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry = serde_json::from_str::<DictationHistoryEntry>(line).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Dictation History contains an invalid record at line {}",
                    index + 1
                ),
            )
        })?;
        entries.push(migrate_history_entry(entry));
    }
    entries.reverse();
    Ok(entries)
}

pub fn get_history_entry(id: &str) -> Option<DictationHistoryEntry> {
    load_history().into_iter().find(|entry| entry.id == id)
}

fn rank_history_entries(
    entries: Vec<DictationHistoryEntry>,
    query: &str,
    limit: usize,
) -> Vec<DictationHistorySearchHit> {
    use crate::scripts::search::sentence::{
        compile_long_text_query, match_long_text_query, FieldClass, FieldVisibility, LongTextField,
        LongTextFieldId, RenderSlot,
    };

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return entries
            .into_iter()
            .take(limit)
            .map(|entry| DictationHistorySearchHit {
                entry,
                score: 0,
                matched_field: DictationHistorySearchField::Transcript,
                evidence: None,
            })
            .collect();
    }

    let Some(compiled) = compile_long_text_query(trimmed) else {
        return Vec::new();
    };

    let mut hits = Vec::new();

    for entry in entries {
        // Raw and formatted forms of the same metadata belong to one field.
        let timestamp_text = format!(
            "{} {}",
            entry.timestamp,
            format_history_timestamp(&entry.timestamp)
        );
        let duration_text = format_history_duration_ms(entry.audio_duration_ms);

        // The preview is the rendered row title; the full transcript stays a
        // hidden recall field. Timestamp/duration render inside the composed
        // subtitle, so they count as visible metadata but emit no highlight
        // offsets.
        let fields = [
            LongTextField {
                id: LongTextFieldId::Preview,
                text: entry.preview.as_str(),
                class: FieldClass::NaturalText,
                visibility: FieldVisibility::Visible(RenderSlot::Title),
                weight: 5,
            },
            LongTextField {
                id: LongTextFieldId::Target,
                text: entry.target_label_snapshot.as_str(),
                class: FieldClass::NaturalText,
                visibility: FieldVisibility::Visible(RenderSlot::Subtitle),
                weight: 3,
            },
            LongTextField {
                id: LongTextFieldId::Transcript,
                text: entry.transcript.as_str(),
                class: FieldClass::NaturalText,
                visibility: FieldVisibility::Hidden,
                weight: 2,
            },
            LongTextField {
                id: LongTextFieldId::Timestamp,
                text: timestamp_text.as_str(),
                class: FieldClass::Metadata,
                visibility: FieldVisibility::Visible(RenderSlot::Subtitle),
                weight: 1,
            },
            LongTextField {
                id: LongTextFieldId::Duration,
                text: duration_text.as_str(),
                class: FieldClass::Metadata,
                visibility: FieldVisibility::Visible(RenderSlot::Subtitle),
                weight: 1,
            },
        ];

        let Some(matched) = match_long_text_query(&compiled, &fields) else {
            continue;
        };

        let matched_field = match matched.evidence.primary_field {
            LongTextFieldId::Target => DictationHistorySearchField::Target,
            LongTextFieldId::Timestamp | LongTextFieldId::Duration => {
                DictationHistorySearchField::Timestamp
            }
            _ => DictationHistorySearchField::Transcript,
        };

        hits.push(DictationHistorySearchHit {
            entry,
            score: matched.rank_score(),
            matched_field,
            evidence: Some(matched.evidence),
        });
    }

    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.entry.timestamp.cmp(&a.entry.timestamp))
    });
    hits.truncate(limit);
    hits
}

pub fn search_history(query: &str, limit: usize) -> Vec<DictationHistorySearchHit> {
    let hits = rank_history_entries(load_history(), query, limit);
    tracing::info!(
        category = "DICTATION",
        event = "dictation_history_search_executed",
        query_len = query.trim().chars().count(),
        limit,
        hit_count = hits.len(),
    );
    hits
}

pub fn search_history_page(
    query: &str,
    offset: usize,
    visible_limit: usize,
) -> std::io::Result<DictationHistoryPage> {
    let entries = load_history_result()?;
    let hits = rank_history_entries(entries, query, usize::MAX);
    let total_matches = hits.len();
    let end = offset.saturating_add(visible_limit).min(total_matches);
    let rows = if offset >= total_matches {
        Vec::new()
    } else {
        hits[offset..end]
            .iter()
            .map(|hit| hit.entry.clone())
            .collect()
    };
    let visible_count = offset.saturating_add(rows.len()).min(total_matches);
    Ok(DictationHistoryPage {
        total_matches,
        visible_count,
        offset,
        rows,
        has_more: end < total_matches,
    })
}

pub fn dictation_history_view_state(
    query: &str,
    visible_limit: usize,
    previous: Option<DictationHistoryPage>,
) -> DictationHistoryViewState {
    match search_history_page(query, 0, visible_limit) {
        Ok(page) if page.total_matches > 0 => DictationHistoryViewState::Ready(page),
        Ok(_) if query.trim().is_empty() => DictationHistoryViewState::NoSavedDictation,
        Ok(_) => DictationHistoryViewState::NoFilteredMatches,
        Err(error) => DictationHistoryViewState::Failed {
            message: format!("Dictation History could not be loaded: {error}"),
            previous,
        },
    }
}

fn dictation_history_cache_is_fresh(cache: &DictationHistoryIndexCache) -> bool {
    if crate::runtime_policy::is_owned_evaluation() {
        return cache.owned && cache.owned_fresh;
    }
    history_file_signature(&history_path()).is_ok_and(|signature| cache.signature == signature)
}

#[allow(dead_code)] // Root search cache receipts use this through the binary app layer.
/// Accepted snapshot publication revision and row count, never a worker identity.
pub(crate) fn root_dictation_history_fresh_cache_status() -> Option<(u64, usize)> {
    let lifecycle = dictation_history_refresh_lifecycle().try_lock().ok()?;
    if lifecycle.in_flight.is_some() {
        return None;
    }
    let guard = dictation_history_index_cache().try_lock().ok()?;
    let cache = guard.as_ref()?;
    let revision = DICTATION_HISTORY_CACHE_REVISION.load(Ordering::Relaxed);
    (revision != 0 && dictation_history_cache_is_fresh(cache))
        .then_some((revision, cache.entries.len()))
}

fn cached_history_entries_if_fresh() -> Option<Vec<DictationHistoryEntry>> {
    let guard = dictation_history_index_cache().lock().ok()?;
    let cache = guard.as_ref()?;
    dictation_history_cache_is_fresh(cache).then(|| cache.entries.clone())
}

pub(crate) fn root_dictation_history_cache_is_fresh() -> bool {
    cached_history_entries_if_fresh().is_some()
}

pub(crate) fn try_begin_root_dictation_history_refresh() -> Option<RootDictationHistoryRefresh> {
    let cache_is_fresh = root_dictation_history_cache_is_fresh();
    dictation_history_refresh_lifecycle().lock().ok()?.begin(
        sk_protocol::command_contract::CommandSource::Dictation,
        cache_is_fresh,
    )
}

fn read_root_dictation_history_snapshot_at(path: &std::path::Path) -> RootDictationHistorySnapshot {
    let parsed = match crate::atomic_file::read_private_file(path) {
        Ok(content) => parse_history_entries(&content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    };
    RootDictationHistorySnapshot {
        cache: parsed.map_err(anyhow::Error::from).and_then(|entries| {
            Ok(DictationHistoryIndexCache {
                signature: history_file_signature(path)?,
                owned: false,
                owned_fresh: true,
                entries,
            })
        }),
    }
}

pub(crate) fn read_root_dictation_history_snapshot() -> RootDictationHistorySnapshot {
    assert!(
        !crate::runtime_policy::is_owned_evaluation(),
        "owned_source_snapshot_required"
    );
    tracing::debug!(
        target: "script_kit::search",
        worker = ROOT_DICTATION_HISTORY_REFRESH_LABEL,
        "Reading owned private dictation history snapshot"
    );
    read_root_dictation_history_snapshot_at(&history_path())
}

#[allow(dead_code)] // Owned search fixtures call this through the binary app layer.
pub(crate) fn owned_root_dictation_history_snapshot(
    result: anyhow::Result<Vec<DictationHistoryEntry>>,
) -> anyhow::Result<RootDictationHistorySnapshot> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    Ok(RootDictationHistorySnapshot {
        cache: result.map(|entries| DictationHistoryIndexCache {
            signature: None,
            owned: true,
            owned_fresh: true,
            entries,
        }),
    })
}

#[allow(dead_code)] // Owned search source changes call this through the binary app layer.
pub(crate) fn invalidate_owned_root_dictation_history_freshness() -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut lifecycle = dictation_history_refresh_lifecycle()
        .lock()
        .map_err(|_| anyhow::anyhow!("dictation_lifecycle_poisoned"))?;
    let mut cache = dictation_history_index_cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("dictation_cache_poisoned"))?;
    lifecycle.next_generation = lifecycle
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("dictation_generation_exhausted"))?;
    lifecycle.in_flight = None;
    if let Some(cache) = cache.as_mut() {
        cache.owned_fresh = false;
    }
    Ok(())
}

#[allow(dead_code)] // Owned search fixtures call this through the binary app layer.
pub(crate) fn reset_owned_root_dictation_history() -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut lifecycle = dictation_history_refresh_lifecycle()
        .lock()
        .map_err(|_| anyhow::anyhow!("dictation_lifecycle_poisoned"))?;
    let mut cache = dictation_history_index_cache()
        .lock()
        .map_err(|_| anyhow::anyhow!("dictation_cache_poisoned"))?;
    lifecycle.next_generation = lifecycle
        .next_generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("dictation_generation_exhausted"))?;
    lifecycle.in_flight = None;
    *cache = None;
    Ok(())
}

fn root_dictation_history_snapshot_is_current_at(
    snapshot: &RootDictationHistorySnapshot,
    path: &std::path::Path,
) -> bool {
    snapshot.cache.as_ref().is_ok_and(|cache| {
        history_file_signature(path).is_ok_and(|signature| cache.signature == signature)
    })
}

pub(crate) fn finish_root_dictation_history_refresh(
    refresh: RootDictationHistoryRefresh,
    snapshot: RootDictationHistorySnapshot,
) -> bool {
    let Ok(mut lifecycle) = dictation_history_refresh_lifecycle().lock() else {
        return false;
    };
    if !lifecycle.finish(refresh) {
        return false;
    }

    let owned = crate::runtime_policy::is_owned_evaluation();
    if !owned && !root_dictation_history_snapshot_is_current_at(&snapshot, &history_path()) {
        return false;
    }
    let Ok(snapshot) = snapshot.cache else {
        return false;
    };
    if snapshot.owned != owned {
        return false;
    }
    let Ok(mut cache) = dictation_history_index_cache().lock() else {
        return false;
    };
    *cache = Some(snapshot);
    DICTATION_HISTORY_CACHE_REVISION.fetch_add(1, Ordering::Relaxed);
    true
}

pub(crate) fn discard_root_dictation_history_refresh(refresh: RootDictationHistoryRefresh) -> bool {
    dictation_history_refresh_lifecycle()
        .lock()
        .is_ok_and(|mut lifecycle| lifecycle.finish(refresh))
}

pub fn root_dictation_history_query_is_eligible(
    query: &str,
    options: RootDictationHistorySectionOptions,
) -> bool {
    let trimmed = query.trim();
    options.enabled
        && crate::scripts::search::query_meets_min_query_chars(trimmed, options.min_query_chars)
        && !trimmed.contains('\n')
        && !trimmed.contains('\r')
}

pub fn search_root_dictation_history(
    query: &str,
    options: RootDictationHistorySectionOptions,
) -> Vec<RootDictationHistorySearchHit> {
    let entries = load_history()
        .into_iter()
        .take(options.scan_limit)
        .collect::<Vec<_>>();
    let hits = rank_history_entries(entries, query, options.max_results)
        .into_iter()
        .map(|hit| {
            let target = hit.entry.display_target_label();
            RootDictationHistorySearchHit {
                id: hit.entry.id,
                preview: hit.entry.preview,
                target,
                timestamp: hit.entry.timestamp,
                audio_duration_ms: hit.entry.audio_duration_ms,
                score: hit.score,
                matched_field: hit.matched_field,
                evidence: hit.evidence,
            }
        })
        .collect::<Vec<_>>();
    tracing::info!(
        category = "DICTATION",
        event = "root_dictation_history_search_executed",
        query_len = query.trim().chars().count(),
        scan_limit = options.scan_limit,
        max_results = options.max_results,
        hit_count = hits.len(),
    );
    hits
}

pub fn search_root_dictation_history_direct(
    query: &str,
    options: RootDictationHistorySectionOptions,
) -> Vec<RootDictationHistorySearchHit> {
    search_root_dictation_history(query, options)
}

/// Cache-only dictation history search for root launcher passive rows.
///
/// Cold JSONL indexes return no hits without starting a worker or publishing
/// state; the real launcher input owner coordinates generation-fenced refresh.
pub fn search_root_dictation_history_cached(
    query: &str,
    options: RootDictationHistorySectionOptions,
) -> Vec<RootDictationHistorySearchHit> {
    if !root_dictation_history_query_is_eligible(query, options) {
        return Vec::new();
    }

    let entries = dictation_history_index_cache()
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
            category = "DICTATION",
            event = "root_dictation_history_search_cache_miss",
            query_len = query.trim().chars().count(),
            scan_limit = options.scan_limit,
            max_results = options.max_results,
        );
        return Vec::new();
    };

    let hits = rank_history_entries(
        entries.into_iter().take(options.scan_limit).collect(),
        query,
        options.max_results,
    )
    .into_iter()
    .map(|hit| {
        let target = hit.entry.display_target_label();
        RootDictationHistorySearchHit {
            id: hit.entry.id,
            preview: hit.entry.preview,
            target,
            timestamp: hit.entry.timestamp,
            audio_duration_ms: hit.entry.audio_duration_ms,
            score: hit.score,
            matched_field: hit.matched_field,
            evidence: hit.evidence,
        }
    })
    .collect::<Vec<_>>();
    if crate::logging::filter_perf_trace_enabled() {
        tracing::info!(
            category = "DICTATION",
            event = "root_dictation_history_search_cache_hit",
            query_len = query.trim().chars().count(),
            scan_limit = options.scan_limit,
            max_results = options.max_results,
            hit_count = hits.len(),
        );
    }
    hits
}

fn resource_payload(entries: &[DictationHistoryEntry]) -> String {
    if entries.is_empty() {
        return serde_json::json!({
            "schemaVersion": 1,
            "type": "dictation",
            "ok": true,
            "available": false,
            "source": "history",
            "items": [],
            "note": "No saved dictation history yet.",
            "nextStep": "Start dictation to capture text."
        })
        .to_string();
    }

    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "timestamp": entry.timestamp,
                "displayTimestamp": format_history_timestamp(&entry.timestamp),
                "text": entry.transcript,
                "preview": entry.preview,
                "target": entry.display_target_label(),
                "targetId": entry.target_id,
                "audioDurationMs": entry.audio_duration_ms,
                "displayDuration": format_history_duration_ms(entry.audio_duration_ms),
            })
        })
        .collect();

    serde_json::json!({
        "schemaVersion": 1,
        "type": "dictation",
        "ok": true,
        "available": true,
        "source": "history",
        "count": entries.len(),
        "current": items.first().cloned(),
        "items": items,
    })
    .to_string()
}

fn refresh_published_resource_from_entries(entries: &[DictationHistoryEntry]) {
    crate::mcp_resources::publish_dictation_json(resource_payload(entries));
}

pub fn hydrate_dictation_resource_from_history() {
    let entries = match load_history_result() {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(
                category = "DICTATION",
                reason = ?error.kind(),
                "dictation_history_hydration_failed_preserving_existing_resource"
            );
            return;
        }
    };
    let latest: Vec<DictationHistoryEntry> =
        entries.into_iter().take(RESOURCE_ITEMS_LIMIT).collect();
    refresh_published_resource_from_entries(&latest);
}

pub fn record_dictation_history(
    transcript: &str,
    audio_duration: Duration,
    target: DictationTarget,
) -> std::io::Result<DictationHistoryEntry> {
    let entry = build_history_entry(transcript, audio_duration, target);
    save_history_entry(&entry)?;
    tracing::info!(
        category = "DICTATION",
        event = "dictation_history_entry_saved",
        entry_id = %entry.id,
        target_id = %entry.target_id,
        transcript_len = entry.transcript.len(),
        audio_duration_ms = entry.audio_duration_ms,
    );
    Ok(entry)
}

pub fn delete_history_entry(entry_id: &str) -> anyhow::Result<()> {
    use anyhow::Context;

    with_history_write_lock(|| {
        let entries: Vec<DictationHistoryEntry> = load_history_result()?
            .into_iter()
            .filter(|entry| entry.id != entry_id)
            .collect();
        let rewritten: Vec<DictationHistoryEntry> = entries.iter().cloned().rev().collect();
        write_history(&rewritten)?;
        let recent: Vec<DictationHistoryEntry> =
            entries.into_iter().take(RESOURCE_ITEMS_LIMIT).collect();
        refresh_published_resource_from_entries(&recent);
        Ok(())
    })
    .context("rewrite private Dictation History")?;
    tracing::info!(
        category = "DICTATION",
        event = "dictation_history_entry_deleted",
        entry_id = %entry_id,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_dictation_snapshots_and_resets_require_runtime_authority() {
        assert!(owned_root_dictation_history_snapshot(Ok(Vec::new())).is_err());
        assert!(reset_owned_root_dictation_history().is_err());
        assert!(invalidate_owned_root_dictation_history_freshness().is_err());
    }

    struct TestEnv {
        _sk_path_lock: std::sync::MutexGuard<'static, ()>,
        _provider_json_lock: std::sync::MutexGuard<'static, ()>,
        prev_sk_path: Option<String>,
        tempdir: tempfile::TempDir,
    }

    impl TestEnv {
        fn new() -> Self {
            let lock = crate::test_utils::SK_PATH_TEST_LOCK
                .get_or_init(|| std::sync::Mutex::new(()))
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let provider_json_lock = crate::test_utils::lock_provider_json_test();
            let tempdir = tempfile::tempdir().expect("tempdir");
            let prev_sk_path = std::env::var(crate::setup::SK_PATH_ENV).ok();
            std::env::set_var(crate::setup::SK_PATH_ENV, tempdir.path());
            crate::mcp_resources::clear_provider_json_slots();
            Self {
                _sk_path_lock: lock,
                _provider_json_lock: provider_json_lock,
                prev_sk_path,
                tempdir,
            }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            match &self.prev_sk_path {
                Some(value) => std::env::set_var(crate::setup::SK_PATH_ENV, value),
                None => std::env::remove_var(crate::setup::SK_PATH_ENV),
            }
            crate::mcp_resources::clear_provider_json_slots();
            let _ = &self.tempdir;
        }
    }

    #[test]
    fn fresh_dictation_cache_proof_tracks_publication_freshness_and_worker_ownership() {
        let _env = TestEnv::new();
        invalidate_history_cache();
        assert!(root_dictation_history_fresh_cache_status().is_none());
        let refresh = try_begin_root_dictation_history_refresh().unwrap();
        assert!(root_dictation_history_fresh_cache_status().is_none());
        assert!(finish_root_dictation_history_refresh(
            refresh,
            read_root_dictation_history_snapshot()
        ));
        let (revision, count) = root_dictation_history_fresh_cache_status().unwrap();
        assert!(revision > 0);
        assert_eq!(count, 0);
        {
            let _cache = dictation_history_index_cache().lock().unwrap();
            assert!(root_dictation_history_fresh_cache_status().is_none());
        }
        let worker = dictation_history_refresh_lifecycle()
            .lock()
            .unwrap()
            .begin(
                sk_protocol::command_contract::CommandSource::Dictation,
                false,
            )
            .unwrap();
        assert!(root_dictation_history_fresh_cache_status().is_none());
        assert!(discard_root_dictation_history_refresh(worker));
        assert_eq!(
            root_dictation_history_fresh_cache_status(),
            Some((revision, 0))
        );
        crate::atomic_file::write_private_atomic(&history_path(), b"").unwrap();
        assert!(root_dictation_history_fresh_cache_status().is_none());
        let refresh = try_begin_root_dictation_history_refresh().unwrap();
        assert!(finish_root_dictation_history_refresh(
            refresh,
            read_root_dictation_history_snapshot()
        ));
        assert!(root_dictation_history_fresh_cache_status().unwrap().0 > revision);
        invalidate_history_cache();
        assert!(root_dictation_history_fresh_cache_status().is_none());
    }

    #[test]
    fn dictation_history_semantic_projection_uses_stable_renderer_and_scroll_identity() {
        fn entry(id: &str, transcript: &str) -> DictationHistoryEntry {
            DictationHistoryEntry {
                version: DICTATION_HISTORY_ENTRY_VERSION,
                id: id.to_string(),
                timestamp: "2026-08-22T12:00:00Z".to_string(),
                transcript: transcript.to_string(),
                preview: transcript.to_string(),
                target_id: "notes".to_string(),
                target_label_snapshot: "Notes".to_string(),
                audio_duration_ms: 1000,
            }
        }

        let first = entry("saved-entry-1", "first private spoken transcript");
        let second = entry("saved-entry-2", "second private spoken transcript");
        let mut elements = vec![
            crate::protocol::ElementInfo::input(
                "dictation-history-filter",
                Some("private query"),
                true,
            ),
            crate::protocol::ElementInfo::list("dictation-history", 2),
            crate::protocol::ElementInfo::redacted_choice(
                0,
                &first.preview,
                &first.preview,
                false,
                crate::protocol::ElementContentKind::UserContent,
            ),
            crate::protocol::ElementInfo::redacted_choice(
                1,
                &second.preview,
                &second.preview,
                true,
                crate::protocol::ElementContentKind::UserContent,
            ),
        ];

        apply_dictation_history_row_identities(&mut elements, &[first.clone(), second.clone()]);

        assert_eq!(elements[2].semantic_id, first.semantic_id());
        assert_eq!(elements[3].semantic_id, second.semantic_id());
        assert_eq!(elements[3].selected, Some(true));
        assert_eq!(elements[3].source.as_deref(), Some("dictationHistory"));
        let serialized = serde_json::to_string(&elements).unwrap();
        assert!(!serialized.contains("first private spoken transcript"));
        assert!(!serialized.contains("second private spoken transcript"));
        assert!(!serialized.contains("private query"));

        let mut reordered = vec![crate::protocol::ElementInfo::redacted_choice(
            0,
            &second.preview,
            &second.preview,
            true,
            crate::protocol::ElementContentKind::UserContent,
        )];
        apply_dictation_history_row_identities(&mut reordered, &[second.clone()]);
        assert_eq!(reordered[0].semantic_id, second.semantic_id());
    }

    #[test]
    fn dictation_history_integrity_repairs_legacy_jsonl_record_boundaries() {
        let directory = tempfile::tempdir().expect("isolated legacy JSONL fixture");
        let path = directory.path().join("dictation-history.jsonl");
        let first = build_history_entry(
            "first private transcript without a terminal newline",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        );
        let second = build_history_entry(
            "second private transcript must remain independently readable",
            Duration::from_secs(2),
            DictationTarget::AiChatComposer,
        );
        let legacy = serde_json::to_string(&first).expect("serialize legacy private transcript");
        crate::atomic_file::write_private_atomic(&path, legacy.as_bytes())
            .expect("seed legacy transcript without newline");

        save_history_entry_at(&path, &second).expect("repair private JSONL boundary");

        let entries = load_history_result_at(&path).expect("both private records remain valid");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, second.id);
        assert_eq!(entries[1].id, first.id);
        let stored = crate::atomic_file::read_private_file(&path).expect("read private JSONL");
        assert_eq!(stored.lines().count(), 2);
        assert!(stored.ends_with('\n'));
    }

    #[test]
    fn dictation_history_integrity_refuses_to_delete_or_leak_malformed_private_history() {
        let _env = TestEnv::new();
        let retained = record_dictation_history(
            "retained private spoken transcript",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        )
        .expect("persist valid private transcript");
        let path = history_path();
        let canary = "private-malformed-spoken-transcript-never-expose";
        let valid = crate::atomic_file::read_private_file(&path).expect("read valid history");
        let corrupted = format!("{valid}{{\"transcript\":\"{canary}\"");
        crate::atomic_file::write_private_atomic(&path, corrupted.as_bytes())
            .expect("seed malformed private history");
        invalidate_history_cache();

        let read_error = load_history_result().expect_err("malformed history fails closed");
        assert_eq!(read_error.kind(), std::io::ErrorKind::InvalidData);
        assert!(!read_error.to_string().contains(canary));
        let deletion_error = delete_history_entry(&retained.id)
            .expect_err("deleting one row must never erase an unreadable history file");
        assert!(!deletion_error.to_string().contains(canary));
        assert_eq!(
            crate::atomic_file::read_private_file(&path).expect("preserve corrupted private bytes"),
            corrupted
        );
        assert!(crate::mcp_resources::has_provider_json_resource(
            crate::mcp_resources::ProviderJsonResourceKind::Dictation
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dictation_history_integrity_never_reports_or_publishes_an_unsaved_transcript() {
        use std::os::unix::fs::symlink;

        let env = TestEnv::new();
        let foreign = env.tempdir.path().join("protected-foreign-transcript.txt");
        let original = "another owner's spoken transcript remains untouched";
        std::fs::write(&foreign, original).expect("seed foreign private transcript");
        symlink(&foreign, history_path()).expect("plant hostile history symlink");

        let error = record_dictation_history(
            "new spoken words that cannot safely persist",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        )
        .expect_err("a failed private write must not fabricate a saved history entry");

        assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(std::fs::read_to_string(&foreign).unwrap(), original);
        assert!(!crate::mcp_resources::has_provider_json_resource(
            crate::mcp_resources::ProviderJsonResourceKind::Dictation
        ));
    }

    #[test]
    fn dictation_history_integrity_rejects_malformed_root_search_snapshots() {
        let directory = tempfile::tempdir().expect("isolated root Dictation History snapshot");
        let path = directory.path().join("dictation-history.jsonl");
        crate::atomic_file::write_private_atomic(&path, b"private malformed transcript\n")
            .expect("seed malformed private root snapshot");

        let snapshot = read_root_dictation_history_snapshot_at(&path);

        assert!(snapshot.read_outcome().is_err());
        assert!(!root_dictation_history_snapshot_is_current_at(
            &snapshot, &path
        ));
    }

    #[test]
    fn dictation_read_outcome_distinguishes_failure_from_successful_empty() {
        let failed = RootDictationHistorySnapshot {
            cache: Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()),
        };
        assert_eq!(
            failed
                .read_outcome()
                .unwrap_err()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        let empty = RootDictationHistorySnapshot {
            cache: Ok(DictationHistoryIndexCache {
                signature: None,
                owned: true,
                owned_fresh: true,
                entries: Vec::new(),
            }),
        };
        assert_eq!(empty.read_outcome().unwrap(), 0);
    }

    #[test]
    fn dictation_history_integrity_hydration_preserves_the_last_valid_provider_payload() {
        let _env = TestEnv::new();
        record_dictation_history(
            "last valid provider-backed private transcript",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        )
        .expect("publish valid private history payload");
        let kind = crate::mcp_resources::ProviderJsonResourceKind::Dictation;
        let before = crate::mcp_resources::read_provider_json_items(kind);
        assert_eq!(before.len(), 1);
        crate::atomic_file::write_private_atomic(&history_path(), b"malformed private history\n")
            .expect("seed unreadable private history");
        invalidate_history_cache();

        hydrate_dictation_resource_from_history();

        let after = crate::mcp_resources::read_provider_json_items(kind);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].title, before[0].title);
    }

    #[test]
    fn dictation_history_integrity_serializes_concurrent_saves_and_deletion() {
        use std::sync::{Arc, Barrier};

        let _env = TestEnv::new();
        let removed = record_dictation_history(
            "only this private transcript should be removed",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        )
        .expect("persist initial transcript");
        let start = Arc::new(Barrier::new(7));
        let workers = (0..6)
            .map(|index| {
                let start = Arc::clone(&start);
                std::thread::spawn(move || {
                    start.wait();
                    record_dictation_history(
                        &format!("concurrent private transcript {index}"),
                        Duration::from_secs(1),
                        DictationTarget::AiChatComposer,
                    )
                    .expect("persist concurrent transcript")
                    .id
                })
            })
            .collect::<Vec<_>>();

        start.wait();
        delete_history_entry(&removed.id).expect("delete only the selected private transcript");
        let recorded = workers
            .into_iter()
            .map(|worker| worker.join().expect("join isolated history worker"))
            .collect::<Vec<_>>();
        let entries = load_history_result().expect("read complete concurrent private history");

        assert_eq!(entries.len(), recorded.len());
        assert!(!entries.iter().any(|entry| entry.id == removed.id));
        assert!(recorded
            .iter()
            .all(|id| entries.iter().any(|entry| &entry.id == id)));
        assert_eq!(
            crate::mcp_resources::read_provider_json_items(
                crate::mcp_resources::ProviderJsonResourceKind::Dictation
            )
            .len(),
            recorded.len()
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_dictation_history_append_and_atomic_rewrite_are_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated dictation privacy fixture");
        let path = directory.path().join("dictation-history.jsonl");
        let first = build_history_entry(
            "first private spoken transcript",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        );
        let second = build_history_entry(
            "second private spoken transcript",
            Duration::from_secs(2),
            DictationTarget::AiChatComposer,
        );

        save_history_entry_at(&path, &first).expect("private first transcript");
        save_history_entry_at(&path, &second).expect("private second transcript");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(load_history_result_at(&path).unwrap().len(), 2);

        write_history_at(&path, std::slice::from_ref(&second))
            .expect("atomic private history rewrite");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let loaded = load_history_result_at(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].transcript, second.transcript);
    }

    #[cfg(unix)]
    #[test]
    fn private_dictation_history_repairs_legacy_permissions_before_loading() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated legacy dictation fixture");
        let path = directory.path().join("dictation-history.jsonl");
        let entry = build_history_entry(
            "previously exposed spoken transcript",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        );
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&entry).unwrap()),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let loaded = load_history_result_at(&path).expect("legacy transcript migrates safely");
        assert_eq!(loaded[0].transcript, entry.transcript);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_dictation_history_refuses_symlinks_before_read_append_or_rewrite() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated dictation symlink fixture");
        let external = directory.path().join("another-users-transcript.txt");
        let planted = directory.path().join("dictation-history.jsonl");
        std::fs::write(&external, "never expose or overwrite this transcript").unwrap();
        symlink(&external, &planted).unwrap();
        let entry = build_history_entry(
            "new private spoken transcript",
            Duration::from_secs(1),
            DictationTarget::AiChatComposer,
        );

        assert!(load_history_result_at(&planted).is_err());
        assert!(save_history_entry_at(&planted, &entry).is_err());
        assert!(write_history_at(&planted, &[entry]).is_err());
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            "never expose or overwrite this transcript"
        );
        assert!(std::fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn private_dictation_history_snapshot_refuses_changed_transcript_before_publication() {
        let directory = tempfile::tempdir().expect("isolated dictation snapshot fixture");
        let path = directory.path().join("dictation-history.jsonl");
        let entry = build_history_entry(
            "private spoken snapshot",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        );
        save_history_entry_at(&path, &entry).expect("private snapshot seed");

        let snapshot = read_root_dictation_history_snapshot_at(&path);
        assert_eq!(snapshot.read_outcome().unwrap(), 1);
        assert!(root_dictation_history_snapshot_is_current_at(
            &snapshot, &path
        ));

        std::fs::write(&path, "a later and differently sized spoken transcript")
            .expect("mutate isolated snapshot after read");
        assert!(!root_dictation_history_snapshot_is_current_at(
            &snapshot, &path
        ));
    }

    #[test]
    fn build_history_entry_captures_preview_and_target() {
        let entry = build_history_entry(
            "hello from dictation",
            Duration::from_secs(2),
            DictationTarget::AiChatComposer,
        );
        assert_eq!(entry.preview, "hello from dictation");
        assert_eq!(entry.display_target_label(), "Agent Chat");
        assert_eq!(entry.audio_duration_ms, 2_000);
    }

    #[test]
    fn deletion_confirmation_distinguishes_pending_context_without_claiming_sent_turn_loss() {
        let ordinary = delete_history_confirmation_body(false);
        assert!(ordinary.contains("Sent-turn receipts are preserved"));
        assert!(!ordinary.contains("staged in Agent Chat"));

        let pending = delete_history_confirmation_body(true);
        assert!(pending.contains("staged in Agent Chat"));
        assert!(pending.contains("pending attachment unavailable"));
        assert!(pending.contains("Sent-turn receipts are preserved"));
    }

    #[test]
    fn format_history_duration_humanizes_common_values() {
        assert_eq!(format_history_duration_ms(450), "under 1 sec");
        assert_eq!(format_history_duration_ms(8_507), "8.5 sec");
        assert_eq!(format_history_duration_ms(12_200), "12 sec");
        assert_eq!(format_history_duration_ms(61_400), "1 min 1 sec");
    }

    #[test]
    fn record_and_load_history_round_trip() {
        let _env = TestEnv::new();
        let first = record_dictation_history(
            "first transcript",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        )
        .expect("persist first private transcript");
        let second = record_dictation_history(
            "second transcript",
            Duration::from_secs(2),
            DictationTarget::AiChatComposer,
        )
        .expect("persist second private transcript");

        let loaded = load_history();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, second.id);
        assert_eq!(loaded[1].id, first.id);
    }

    #[test]
    fn search_history_matches_transcript_and_target() {
        let _env = TestEnv::new();
        record_dictation_history(
            "draft reply to the oauth ticket",
            Duration::from_secs(2),
            DictationTarget::AiChatComposer,
        )
        .expect("persist Agent Chat transcript");
        record_dictation_history(
            "quick note for the meeting",
            Duration::from_secs(1),
            DictationTarget::NotesEditor,
        )
        .expect("persist Notes transcript");

        let ai_hits = search_history("oauth agent", 10);
        assert_eq!(ai_hits.len(), 1);
        assert_eq!(
            ai_hits[0].matched_field,
            DictationHistorySearchField::Transcript
        );

        let notes_hits = search_history("notes", 10);
        assert_eq!(notes_hits.len(), 1);
        assert_eq!(notes_hits[0].entry.display_target_label(), "Notes");

        let duration_hits = search_history("agent 2 sec", 10);
        assert_eq!(duration_hits.len(), 1);
        assert_eq!(duration_hits[0].entry.display_target_label(), "Agent Chat");
    }

    /// Screenshot regression (2026-07-11): sentence queries must not match
    /// dictation rows whose only hits are stopword fragments inside words.
    #[test]
    fn sentence_query_rejects_mid_word_dictation_noise() {
        let _env = TestEnv::new();
        record_dictation_history(
            "Somewhat shared themes and other generated reports",
            Duration::from_secs(3),
            DictationTarget::NotesEditor,
        )
        .expect("persist unrelated transcript");
        record_dictation_history(
            "So what are the next steps for the launcher",
            Duration::from_secs(2),
            DictationTarget::AiChatComposer,
        )
        .expect("persist matching transcript");

        let hits = search_history("what are the", 10);
        assert_eq!(hits.len(), 1, "mid-word fragments must not qualify");
        assert!(hits[0].entry.transcript.starts_with("So what are the"));
        let evidence = hits[0].evidence.as_ref().expect("evidence present");
        assert!(
            !evidence.title_indices.is_empty(),
            "the matched words highlight in the visible preview"
        );
    }

    /// Matches beyond the 120-char preview still qualify via the hidden
    /// transcript and explain themselves with an excerpt.
    #[test]
    fn transcript_match_beyond_preview_carries_excerpt() {
        let _env = TestEnv::new();
        let filler = "unrelated filler words repeated over and over ".repeat(5);
        let transcript = format!("{filler} the oauth redirect ticket needs attention");
        record_dictation_history(
            &transcript,
            Duration::from_secs(4),
            DictationTarget::NotesEditor,
        )
        .expect("persist beyond-preview transcript");

        let hits = search_history("oauth redirect ticket", 10);
        assert_eq!(hits.len(), 1);
        let evidence = hits[0].evidence.as_ref().expect("evidence present");
        let excerpt = evidence
            .hidden_excerpt
            .as_ref()
            .expect("beyond-preview match explains itself");
        assert!(excerpt.text.contains("oauth redirect ticket"));
    }

    /// Ordinary language must never be satisfied by timestamp/duration
    /// metadata ("are" must not match formatted dates).
    #[test]
    fn alphabetic_terms_cannot_match_metadata() {
        let _env = TestEnv::new();
        record_dictation_history(
            "completely unrelated content",
            Duration::from_secs(2),
            DictationTarget::NotesEditor,
        )
        .expect("persist unrelated transcript");

        // "at" appears in every formatted timestamp ("Jul 11 at 4:50 pm");
        // it must not qualify the row.
        let hits = search_history("unrelated at", 10);
        assert!(
            hits.is_empty(),
            "formatted metadata must not satisfy ordinary words"
        );
    }

    #[test]
    fn delete_history_entry_rewrites_file_and_resource() {
        let _env = TestEnv::new();
        let keep = record_dictation_history(
            "keep me",
            Duration::from_secs(1),
            DictationTarget::MainWindowPrompt,
        )
        .expect("persist retained transcript");
        let drop = record_dictation_history(
            "drop me",
            Duration::from_secs(1),
            DictationTarget::ExternalApp,
        )
        .expect("persist deleted transcript");

        delete_history_entry(&drop.id).expect("delete");
        let loaded = load_history();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, keep.id);
    }

    #[test]
    fn history_pages_report_visible_and_total_counts_without_reordering() {
        let _env = TestEnv::new();
        let entries = (0..125)
            .map(|index| {
                build_history_entry(
                    &format!("history page transcript {index:03}"),
                    Duration::from_secs(1),
                    DictationTarget::NotesEditor,
                )
            })
            .collect::<Vec<_>>();
        write_history(&entries).expect("seed 125 rows");

        let first = search_history_page("", 0, DICTATION_HISTORY_PAGE_SIZE).expect("first page");
        assert_eq!(first.total_matches, 125);
        assert_eq!(first.visible_count, 100);
        assert_eq!(first.rows.len(), 100);
        assert!(first.has_more);
        assert_eq!(first.count_label(), "Showing 100 of 125");
        let selected_id = first.rows[63].id.clone();

        let expanded = search_history_page("", 0, 200).expect("expanded page");
        assert_eq!(expanded.total_matches, 125);
        assert_eq!(expanded.visible_count, 125);
        assert_eq!(expanded.rows.len(), 125);
        assert!(!expanded.has_more);
        assert_eq!(expanded.count_label(), "Showing 125 of 125");
        assert_eq!(expanded.rows[63].id, selected_id);

        let tail = search_history_page("", 100, 100).expect("tail page");
        assert_eq!(tail.offset, 100);
        assert_eq!(tail.rows.len(), 25);
        assert_eq!(tail.visible_count, 125);
    }

    #[test]
    fn legacy_targets_migrate_to_canonical_ids_without_guessing_unknown_labels() {
        let legacy = r#"{"id":"legacy-ai","timestamp":"2026-07-01T00:00:00Z","transcript":"one","preview":"one","target":"AI Chat","audio_duration_ms":1000}
{"id":"legacy-unknown","timestamp":"2026-07-02T00:00:00Z","transcript":"two","preview":"two","target":"Studio Console","audio_duration_ms":1000}"#;
        let entries = parse_history_entries(legacy).expect("valid legacy history");
        let unknown = entries
            .iter()
            .find(|entry| entry.id == "legacy-unknown")
            .expect("unknown entry");
        assert_eq!(unknown.version, DICTATION_HISTORY_ENTRY_VERSION);
        assert_eq!(
            unknown.target_id,
            DICTATION_HISTORY_LEGACY_UNKNOWN_TARGET_ID
        );
        assert_eq!(unknown.display_target_label(), "Studio Console");

        let ai = entries
            .iter()
            .find(|entry| entry.id == "legacy-ai")
            .expect("legacy AI entry");
        assert_eq!(ai.target_id, DictationTarget::TabAiHarness.sticky_label());
        assert_eq!(ai.display_target_label(), "Agent Chat");
    }

    #[test]
    fn view_state_distinguishes_empty_no_match_and_failure_with_prior_rows() {
        let _env = TestEnv::new();
        assert!(matches!(
            dictation_history_view_state("", 100, None),
            DictationHistoryViewState::NoSavedDictation
        ));
        assert!(matches!(
            dictation_history_view_state("missing", 100, None),
            DictationHistoryViewState::NoFilteredMatches
        ));

        let prior = DictationHistoryPage {
            total_matches: 1,
            visible_count: 1,
            offset: 0,
            rows: vec![build_history_entry(
                "retained row",
                Duration::from_secs(1),
                DictationTarget::MainWindowPrompt,
            )],
            has_more: false,
        };
        std::fs::create_dir_all(history_path()).expect("make history path unreadable as a file");
        invalidate_history_cache();
        let failed = dictation_history_view_state("", 100, Some(prior.clone()));
        assert!(matches!(failed, DictationHistoryViewState::Failed { .. }));
        assert_eq!(failed.page(), Some(&prior));
    }

    #[test]
    fn hydrate_publishes_empty_payload_when_no_history_exists() {
        let _env = TestEnv::new();
        crate::mcp_resources::clear_provider_json_slots();
        hydrate_dictation_resource_from_history();
        assert!(
            !crate::mcp_resources::has_provider_json_resource(
                crate::mcp_resources::ProviderJsonResourceKind::Dictation
            ),
            "empty history should not advertise dictation provider data"
        );
    }

    #[test]
    fn record_history_publishes_recent_items_to_provider_slot() {
        let _env = TestEnv::new();
        crate::mcp_resources::clear_provider_json_slots();

        record_dictation_history(
            "provider-backed dictation",
            Duration::from_secs(3),
            DictationTarget::AiChatComposer,
        )
        .expect("persist provider-backed transcript");

        assert!(
            crate::mcp_resources::has_provider_json_resource(
                crate::mcp_resources::ProviderJsonResourceKind::Dictation
            ),
            "saved history should hydrate the dictation provider slot"
        );
    }
}

//! Brain indexer: the background metabolism.
//!
//! A single low-priority thread that, on a timer and on demand:
//! 1. syncs notes into `brain_docs` (the librarian's raw material),
//! 2. embeds docs whose vectors are missing or stale,
//! 3. keeps everything incremental — content hashes mean unchanged docs are
//!    never re-embedded.
//!
//! The indexer NEVER blocks UI: all work happens on its own thread, the
//! embedder is a subprocess, and each cycle processes a bounded batch.

use super::embedder::{resolve_embed_model, BrainEmbedder};
use super::store::{self, DocSource};
use super::substrate::{BrainFrontmatter, BrainSubstrate};
use anyhow::{Context as _, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CYCLE_INTERVAL: Duration = Duration::from_secs(120);
const EMBED_BATCH: usize = 16;
const MAX_EMBED_PER_CYCLE: usize = 256;
/// Hard latency budget for query embedding on the submit path. When the
/// model isn't warm yet the caller falls back to lexical recall.
const QUERY_EMBED_BUDGET: Duration = Duration::from_millis(200);

enum IndexerRequest {
    Wake,
    EmbedQuery {
        text: String,
        reply: Sender<Option<(String, Vec<f32>)>>,
    },
}

static WAKE: OnceLock<Sender<IndexerRequest>> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BrainFileSourceSyncReceipt {
    pub notes: usize,
    pub day_pages: usize,
    pub fragments: usize,
    pub failed_sources: Vec<&'static str>,
}

/// Ask the indexer to run a cycle soon (e.g. after a chat turn ingests new
/// docs). Cheap; coalesces with pending wakes.
pub fn wake_indexer() {
    if let Some(tx) = WAKE.get() {
        let _ = tx.send(IndexerRequest::Wake);
    }
}

/// Embed a query using the indexer's warm model, within a hard latency
/// budget. Returns `(model_id, vector)`, or `None` when no model is on disk,
/// the model isn't warm yet, or the indexer is mid-cycle — callers fall back
/// to lexical recall. Never blocks longer than [`QUERY_EMBED_BUDGET`].
pub fn embed_query_within_budget(text: &str) -> Option<(String, Vec<f32>)> {
    let tx = WAKE.get()?;
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(IndexerRequest::EmbedQuery {
        text: text.to_string(),
        reply: reply_tx,
    })
    .ok()?;
    reply_rx.recv_timeout(QUERY_EMBED_BUDGET).ok().flatten()
}

/// Start the background indexer thread. Idempotent.
pub fn start_brain_indexer() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let (tx, rx) = mpsc::channel::<IndexerRequest>();
    let _ = WAKE.set(tx);
    let _ = std::thread::Builder::new()
        .name("script-kit-brain-indexer".to_string())
        .spawn(move || {
            // Let app startup settle before the first cycle.
            std::thread::sleep(Duration::from_secs(20));
            let mut embedder: Option<BrainEmbedder> = None;
            let mut backoff = EmbedBackoff::default();
            loop {
                let started_unix = unix_now();
                // Supervise the cycle: an unexpected panic here would otherwise
                // unwind the whole indexer thread and silently stop ALL
                // indexing/embedding/curation for the rest of the process. Catch
                // it, surface it via health, reset the embedder in case it was
                // left mid-operation, and keep the metabolism alive.
                let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_cycle(&mut embedder, &mut backoff)
                })) {
                    Ok(result) => result,
                    Err(panic) => {
                        let msg = panic
                            .downcast_ref::<&str>()
                            .map(|s| (*s).to_string())
                            .or_else(|| panic.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "unknown panic".to_string());
                        tracing::error!(target: "script_kit::brain", panic = %msg, "brain index cycle PANICKED; recovering");
                        embedder = None;
                        Err(anyhow::anyhow!("brain index cycle panicked: {msg}"))
                    }
                };
                if let Err(err) = &result {
                    tracing::warn!(target: "script_kit::brain", error = %err, "brain index cycle failed");
                }
                record_cycle_health(started_unix, result.as_ref().err(), &embedder, &backoff);
                // Serve wake/embed requests until the next cycle is due.
                let deadline = std::time::Instant::now() + CYCLE_INTERVAL;
                loop {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match rx.recv_timeout(remaining) {
                        Ok(IndexerRequest::Wake) => break,
                        Ok(IndexerRequest::EmbedQuery { text, reply }) => {
                            let result = embedder.as_ref().and_then(|embedder| {
                                embedder
                                    .embed(vec![text])
                                    .ok()
                                    .and_then(|mut vectors| vectors.pop())
                                    .filter(|vector| !vector.is_empty())
                                    .map(|vector| (embedder.model_id().to_string(), vector))
                            });
                            let _ = reply.send(result);
                        }
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            }
        });
}

/// One full cycle: sync sources, then embed what's missing.
pub(crate) fn run_cycle(
    embedder: &mut Option<BrainEmbedder>,
    backoff: &mut EmbedBackoff,
) -> Result<()> {
    store::init_brain_db()?;
    let synced = sync_notes().unwrap_or_else(|err| {
        tracing::debug!(target: "script_kit::brain", error = %err, "notes sync skipped");
        0
    });
    let day_pages = sync_day_pages().unwrap_or_else(|err| {
        tracing::debug!(target: "script_kit::brain", error = %err, "day page sync skipped");
        0
    });
    let fragments = sync_fragments().unwrap_or_else(|err| {
        tracing::debug!(target: "script_kit::brain", error = %err, "fragment sync skipped");
        0
    });
    let promoted = sync_pinned_clipboard().unwrap_or_else(|err| {
        tracing::debug!(target: "script_kit::brain", error = %err, "clipboard sync skipped");
        0
    });
    let captured = sync_capture_stores().unwrap_or_else(|err| {
        tracing::debug!(target: "script_kit::brain", error = %err, "capture sync skipped");
        0
    });
    sync_browser_attention();
    // Embedding trouble (missing helper binary, model load failure) must
    // never take down the rest of the metabolism — lexical search, journal,
    // and the curator all work without vectors.
    let embedded = embed_pending(embedder, backoff).unwrap_or_else(|err| {
        tracing::warn!(target: "script_kit::brain", error = %err, "embedding pass skipped");
        0
    });
    if synced > 0 || day_pages > 0 || fragments > 0 || promoted > 0 || captured > 0 || embedded > 0
    {
        tracing::info!(
            target: "script_kit::brain",
            synced,
            day_pages,
            fragments,
            promoted,
            captured,
            embedded,
            "brain index cycle"
        );
    }
    // Daily distillation pass (no-op until due; silently skips without pi).
    super::curator::run_if_due();
    prune_ambient_if_due();
    // Heartbeat for the kit://brain health surface.
    let _ = store::meta_set(
        "last_index_cycle",
        &chrono::Utc::now().timestamp().to_string(),
    );
    Ok(())
}

/// Wall-clock seconds since the Unix epoch, or `None` if the clock is before
/// the epoch (never in practice).
fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Record a [`BrainHealth`](super::health::BrainHealth) snapshot after a cycle,
/// success or failure, so brain failures surface on the `kit://brain` status
/// resource instead of being swallowed into a `tracing::warn!`.
fn record_cycle_health(
    started_unix: Option<u64>,
    err: Option<&anyhow::Error>,
    embedder: &Option<BrainEmbedder>,
    backoff: &EmbedBackoff,
) {
    let (docs_total, docs_pending_embedding) = match store::doc_stats() {
        // doc_stats() => (docs, embedded, signals); pending is what has no vector yet.
        Ok((docs, embedded, _signals)) => (docs, (docs - embedded).max(0)),
        Err(_) => (0, 0),
    };
    let embedder_alive = embedder.as_ref().is_some_and(BrainEmbedder::is_alive);
    let recall_mode = if resolve_embed_model().is_some() && embedder_alive {
        "semantic"
    } else {
        "lexical-only"
    };
    // A cycle that skipped the embed phase to back off a broken helper still
    // returns Ok, so surface the backoff on the health card via last_error
    // rather than letting it read as a clean cycle.
    let last_error = err.map(|err| format!("{err:#}")).or_else(|| {
        backoff.is_backing_off().then(|| {
            format!(
                "embedder backing off ({} consecutive failures)",
                backoff.consecutive_failures
            )
        })
    });
    let health = super::health::BrainHealth {
        last_cycle_started_unix: started_unix,
        last_cycle_finished_unix: unix_now(),
        last_cycle_ok: Some(err.is_none()),
        last_error,
        docs_total,
        docs_pending_embedding,
        recall_mode: recall_mode.to_string(),
        embedder_alive,
    };
    let _ = super::health::record_health(&health);
}

/// Once a day, age out the brain's own ambient records (old activity
/// journals, stale attention signals). User-created content is never pruned.
fn prune_ambient_if_due() {
    const PRUNE_INTERVAL_SECS: i64 = 24 * 60 * 60;
    let now = chrono::Utc::now().timestamp();
    let last = store::meta_get("ambient_prune_last")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    if now - last < PRUNE_INTERVAL_SECS {
        return;
    }
    let _ = store::meta_set("ambient_prune_last", &now.to_string());
    match store::prune_ambient_data() {
        Ok((journals, signals, inbox)) if journals > 0 || signals > 0 || inbox > 0 => {
            tracing::info!(
                target: "script_kit::brain",
                journals, signals, inbox, "brain pruned aged ambient data"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::debug!(target: "script_kit::brain", error = %error, "ambient prune skipped");
        }
    }
}

/// Promote PINNED clipboard entries into the brain. Pinning is an explicit
/// "this matters" act — the cleanest ambient-learning signal the clipboard
/// emits, with zero surveillance creep (raw history stays in its own store
/// with its own retention). Image entries contribute their OCR text.
fn sync_pinned_clipboard() -> Result<usize> {
    let entries = crate::clipboard_history::get_clipboard_history(500);
    let mut promoted = 0usize;
    let mut pinned_ids = Vec::new();
    for entry in entries.iter().filter(|entry| entry.pinned) {
        let (title, body) = pinned_clipboard_doc(entry);
        store::upsert_doc(
            DocSource::Clipboard,
            &entry.id,
            &title,
            &body,
            entry.timestamp,
        )?;
        pinned_ids.push(entry.id.clone());
        promoted += 1;
    }
    // Unpinning is the user revoking the "this matters" signal — forget it.
    let _ = store::retain_docs(DocSource::Clipboard, &pinned_ids)?;
    Ok(promoted)
}

fn pinned_clipboard_doc(entry: &crate::clipboard_history::ClipboardEntry) -> (String, String) {
    (
        "Pinned clipboard entry".to_string(),
        crate::clipboard_history::entry_resource_uri(&entry.id),
    )
}

/// Browser attention: hosts the user visited repeatedly in the last day
/// become attention SIGNALS — ranking hints only, never documents. Page
/// titles and URLs are not stored in the brain; the raw history stays in
/// the browser. This is the sensory-buffer pattern: observe, distill,
/// discard.
fn sync_browser_attention() {
    const SIGNAL_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
    const MIN_VISITS: i64 = 3;
    let last_marker = store::meta_get("browser_attention_last")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let now = chrono::Utc::now().timestamp();
    // At most once per cycle interval x4 — browser reads hit other apps'
    // sqlite files; be a polite neighbor.
    if now - last_marker < 8 * 60 {
        return;
    }
    let _ = store::meta_set("browser_attention_last", &now.to_string());
    let Ok(entries) = crate::browser_history::list_recent_history(250) else {
        return;
    };
    let cutoff_ms = chrono::Utc::now().timestamp_millis() - SIGNAL_WINDOW_MS;
    let mut by_host: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for entry in entries {
        if entry.last_visited_at_ms >= cutoff_ms {
            *by_host.entry(entry.host.to_string()).or_default() += 1;
        }
    }
    for (host, visits) in by_host {
        if visits >= MIN_VISITS && !host.is_empty() {
            let topic = host
                .trim_start_matches("www.")
                .split('.')
                .next()
                .unwrap_or(&host)
                .to_string();
            let _ = store::record_signal(&topic, 1, "browser");
        }
    }
}

#[cfg(not(test))]
fn brain_substrate() -> BrainSubstrate {
    BrainSubstrate::default_kit()
}

#[cfg(test)]
fn brain_substrate() -> BrainSubstrate {
    let base =
        std::env::temp_dir().join(format!("script-kit-gpui-test-brain-{}", std::process::id()));
    BrainSubstrate::with_timezone(base, chrono_tz::UTC)
}

pub(crate) fn sync_file_sources_for_recall() -> BrainFileSourceSyncReceipt {
    sync_file_sources_for_recall_with_substrate(&brain_substrate())
}

pub(crate) fn sync_file_sources_for_recall_with_substrate(
    substrate: &BrainSubstrate,
) -> BrainFileSourceSyncReceipt {
    let mut receipt = BrainFileSourceSyncReceipt::default();
    if let Err(error) = store::init_brain_db() {
        receipt.failed_sources.push("db");
        tracing::debug!(
            target: "script_kit::brain",
            error = %error,
            "brain recall file sync init skipped"
        );
        return receipt;
    }

    match sync_notes_with_substrate(substrate) {
        Ok(count) => receipt.notes = count,
        Err(error) => {
            receipt.failed_sources.push("notes");
            tracing::debug!(
                target: "script_kit::brain",
                error = %error,
                "brain recall notes sync skipped"
            );
        }
    }
    match sync_day_pages_with_substrate(substrate) {
        Ok(count) => receipt.day_pages = count,
        Err(error) => {
            receipt.failed_sources.push("day_pages");
            tracing::debug!(
                target: "script_kit::brain",
                error = %error,
                "brain recall day pages sync skipped"
            );
        }
    }
    match sync_fragments_with_substrate(substrate) {
        Ok(count) => receipt.fragments = count,
        Err(error) => {
            receipt.failed_sources.push("fragments");
            tracing::debug!(
                target: "script_kit::brain",
                error = %error,
                "brain recall fragments sync skipped"
            );
        }
    }

    receipt
}

fn list_markdown_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if let Some(brain_root) = dir.parent() {
        crate::atomic_file::ensure_private_directory(brain_root)
            .with_context(|| format!("preparing private brain root {}", brain_root.display()))?;
    }
    crate::atomic_file::ensure_private_directory(dir)
        .with_context(|| format!("preparing private brain dir {}", dir.display()))?;
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.context("reading dir entry")?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if is_conflict_copy_path(&path) {
            continue;
        }
        if !crate::atomic_file::inspect_private_file(&path)
            .with_context(|| format!("inspecting private brain document {}", path.display()))?
        {
            continue;
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

fn is_conflict_copy_path(path: &Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.contains(".conflict-"))
}

fn title_from_note_body(body: &str) -> String {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().trim_start_matches('#').trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Untitled Note".to_string())
}

fn fragment_title(fragment_id: &str, source: Option<&str>) -> String {
    if let Some(uri) = source {
        format!("Fragment: {uri}")
    } else {
        format!("Fragment: {fragment_id}")
    }
}

/// Meta key holding the source_ids produced by the LAST file sync for one
/// (source, substrate base path) pair.
///
/// Why scoped instead of `retain_docs`: a blanket "keep only what I just saw"
/// retain deletes every doc of that source produced by ANY other root —
/// brain tests share one process-global DB across parallel threads (each
/// test syncing its own temp substrate), and the same hazard exists for any
/// future multi-root setup. Scoping the previous-set by substrate base path
/// means each root only ever forgets docs it produced itself; production
/// behavior is unchanged (one stable `~/.scriptkit/brain` base → trashed or
/// deleted files are forgotten on the next cycle).
fn file_sync_meta_key(source: DocSource, base: &Path) -> String {
    use sha2::{Digest, Sha256};
    let canonical = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    // First 8 bytes = 16 hex chars: plenty to keep distinct roots apart.
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("file_sync_ids:{}:{hex}", source.as_str())
}

/// Forget docs this substrate produced in a previous sync that no longer
/// exist on disk: previous_set − current_set via targeted deletes, then the
/// current set is persisted for the next cycle. A fresh DB has an empty
/// previous set, which is exactly right — nothing stale can exist there, so
/// the full-rebuild contract (delete brain.sqlite, re-run, parity) holds.
fn forget_missing_file_docs(source: DocSource, base: &Path, live_ids: &[String]) -> Result<usize> {
    let key = file_sync_meta_key(source, base);
    let previous: std::collections::HashSet<String> = store::meta_get(&key)?
        .map(|value| value.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let live: std::collections::HashSet<&str> = live_ids.iter().map(String::as_str).collect();
    let stale: Vec<String> = previous
        .into_iter()
        .filter(|id| !live.contains(id.as_str()))
        .collect();
    let removed = if stale.is_empty() {
        0
    } else {
        store::delete_docs_by_source_ids(source, &stale)?
    };
    store::meta_set(&key, &live_ids.join("\n"))?;
    Ok(removed)
}

/// Mirror active notes into brain_docs from canonical `brain/notes/*.md`
/// files. Hash-guarded upserts make this cheap, and deletion sync forgets
/// notes the user trashed — a brain that remembers what its owner erased is
/// a bug, not a feature.
fn sync_notes() -> Result<usize> {
    sync_notes_with_substrate(&brain_substrate())
}

fn canonical_brain_path(kind: &str, filename: &str) -> String {
    format!("brain/{kind}/{filename}")
}

/// The identity + content a brain file source contributes to `brain_docs`.
///
/// Derived by ONE helper per source kind (`derive_day_page_doc`,
/// `derive_fragment_doc`, `derive_note_doc`) so the periodic file-source sync
/// and the synchronous capture-time index ([`index_capture_now`]) cannot
/// drift. Drift is the sharp edge here: if the two paths disagreed on
/// `source`/`source_id`/`canonical_path`, every capture would create a
/// DUPLICATE row that the next sync cycle re-adds or garbage-collects.
pub(crate) struct DerivedDoc {
    pub source: DocSource,
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub updated_at: i64,
    pub canonical_path: String,
}

impl DerivedDoc {
    fn upsert(&self) -> Result<i64> {
        store::upsert_doc_with_canonical_path(
            self.source,
            &self.source_id,
            &self.title,
            &self.content,
            self.updated_at,
            Some(&self.canonical_path),
        )
    }
}

/// Day page identity: `source_id` is the `YYYY-MM-DD` file stem; the content is
/// the raw file (day pages carry no frontmatter). `None` for a nameless or
/// empty-stem path.
fn derive_day_page_doc(path: &Path, content: &str) -> Option<DerivedDoc> {
    let stem = path.file_stem().and_then(|stem| stem.to_str())?;
    if stem.is_empty() {
        return None;
    }
    let filename = path.file_name().and_then(|name| name.to_str())?;
    Some(DerivedDoc {
        source: DocSource::DayPage,
        source_id: stem.to_string(),
        title: format!("Day Page {stem}"),
        content: content.to_string(),
        updated_at: file_mtime_timestamp(path),
        canonical_path: canonical_brain_path("days", filename),
    })
}

/// Fragment identity: `source_id` is the file stem; title, content (body plus
/// appended provenance), and `updated_at` come from the parsed frontmatter.
fn derive_fragment_doc(
    path: &Path,
    frontmatter: &BrainFrontmatter,
    body: &str,
) -> Option<DerivedDoc> {
    let fragment_id = path.file_stem().and_then(|stem| stem.to_str())?;
    if fragment_id.is_empty() {
        return None;
    }
    let filename = path.file_name().and_then(|name| name.to_str())?;
    let mut content = body.to_string();
    if let Some(source) = &frontmatter.source {
        content.push_str(&format!("\n\nProvenance: {source}"));
    }
    Some(DerivedDoc {
        source: DocSource::Fragment,
        source_id: fragment_id.to_string(),
        title: fragment_title(fragment_id, frontmatter.source.as_deref()),
        content,
        updated_at: frontmatter.updated.timestamp(),
        canonical_path: canonical_brain_path("fragments", filename),
    })
}

/// Note identity: `source_id` is the frontmatter id; the title is derived from
/// the body's first non-empty line.
fn derive_note_doc(path: &Path, frontmatter: &BrainFrontmatter, body: &str) -> Option<DerivedDoc> {
    let filename = path.file_name().and_then(|name| name.to_str())?;
    Some(DerivedDoc {
        source: DocSource::Note,
        source_id: frontmatter.id.to_string(),
        title: title_from_note_body(body),
        content: body.to_string(),
        updated_at: frontmatter.updated.timestamp(),
        canonical_path: canonical_brain_path("notes", filename),
    })
}

pub(crate) fn sync_notes_with_substrate(substrate: &BrainSubstrate) -> Result<usize> {
    let notes_dir = substrate.paths().notes_dir();
    let mut synced = 0usize;
    let mut live_ids = Vec::new();
    for path in list_markdown_files(&notes_dir)? {
        let raw = match crate::brain::substrate::io::read_private_document(&path) {
            Ok(raw) => raw,
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::brain",
                    path = %path.display(),
                    error = %error,
                    "note file read skipped"
                );
                continue;
            }
        };
        let (frontmatter, body) = match substrate.parse_document(&raw) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::brain",
                    path = %path.display(),
                    error = %error,
                    "note file parse skipped"
                );
                continue;
            }
        };
        let Some(doc) = derive_note_doc(&path, &frontmatter, &body) else {
            continue;
        };
        doc.upsert()?;
        live_ids.push(doc.source_id);
        synced += 1;
    }
    let removed = forget_missing_file_docs(DocSource::Note, substrate.paths().base(), &live_ids)?;
    if removed > 0 {
        tracing::info!(target: "script_kit::brain", removed, "brain forgot deleted notes");
    }
    Ok(synced)
}

/// Mirror day pages (`brain/days/YYYY-MM-DD.md`) into brain_docs — one doc
/// per day, re-upserted only when content hash changes.
fn sync_day_pages() -> Result<usize> {
    sync_day_pages_with_substrate(&brain_substrate())
}

pub(crate) fn sync_day_pages_with_substrate(substrate: &BrainSubstrate) -> Result<usize> {
    let days_dir = substrate.paths().days_dir();
    let mut synced = 0usize;
    let mut live_ids = Vec::new();
    for path in list_markdown_files(&days_dir)? {
        let content = match crate::brain::substrate::io::read_private_document(&path) {
            Ok(content) => content,
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::brain",
                    path = %path.display(),
                    error = %error,
                    "day page read skipped"
                );
                continue;
            }
        };
        let Some(doc) = derive_day_page_doc(&path, &content) else {
            continue;
        };
        doc.upsert()?;
        live_ids.push(doc.source_id);
        synced += 1;
    }
    let removed =
        forget_missing_file_docs(DocSource::DayPage, substrate.paths().base(), &live_ids)?;
    if removed > 0 {
        tracing::info!(target: "script_kit::brain", removed, "brain forgot deleted day pages");
    }
    Ok(synced)
}

/// Mirror fragment files (`brain/fragments/*.md`) into brain_docs — one doc
/// per fragment with provenance from frontmatter.
fn sync_fragments() -> Result<usize> {
    sync_fragments_with_substrate(&brain_substrate())
}

pub(crate) fn sync_fragments_with_substrate(substrate: &BrainSubstrate) -> Result<usize> {
    let fragments_dir = substrate.paths().fragments_dir();
    let mut synced = 0usize;
    let mut live_ids = Vec::new();
    for path in list_markdown_files(&fragments_dir)? {
        let raw = match crate::brain::substrate::io::read_private_document(&path) {
            Ok(raw) => raw,
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::brain",
                    path = %path.display(),
                    error = %error,
                    "fragment read skipped"
                );
                continue;
            }
        };
        let (frontmatter, body) = match substrate.parse_document(&raw) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::brain",
                    path = %path.display(),
                    error = %error,
                    "fragment parse skipped"
                );
                continue;
            }
        };
        let Some(doc) = derive_fragment_doc(&path, &frontmatter, &body) else {
            continue;
        };
        doc.upsert()?;
        live_ids.push(doc.source_id);
        synced += 1;
    }
    let removed =
        forget_missing_file_docs(DocSource::Fragment, substrate.paths().base(), &live_ids)?;
    if removed > 0 {
        tracing::info!(target: "script_kit::brain", removed, "brain forgot deleted fragments");
    }
    Ok(synced)
}

/// Index a just-captured brain file into the derived store immediately so
/// lexical (FTS) recall sees it without waiting for the next indexer cycle.
/// Embeddings remain async: the periodic cycle picks up docs whose vectors are
/// missing, and this wakes it. Failures are logged, never surfaced — capture
/// must never fail because indexing hiccuped.
pub fn index_capture_now(path: &Path) {
    if let Err(error) = try_index_capture(path) {
        tracing::warn!(
            target: "script_kit::brain",
            path = %path.display(),
            error = %error,
            "synchronous capture index skipped"
        );
    }
}

fn try_index_capture(path: &Path) -> Result<()> {
    let Some(doc) = derive_capture_doc(path)? else {
        return Ok(());
    };
    store::init_brain_db()?;
    doc.upsert()?;
    // Embeddings stay async; nudge the indexer to vectorize the new row. This
    // only sends a channel message — no model work runs on the caller thread.
    wake_indexer();
    Ok(())
}

/// Classify a brain file by its parent directory and derive its doc identity
/// using the SAME per-source-kind helpers the periodic sync uses, so a capture
/// and the next cycle agree on one row. Non-brain paths, non-markdown files,
/// and conflict copies are ignored (`Ok(None)`).
fn derive_capture_doc(path: &Path) -> Result<Option<DerivedDoc>> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Ok(None);
    }
    if is_conflict_copy_path(path) {
        return Ok(None);
    }
    let kind = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str());
    match kind {
        Some("days") => {
            let content = crate::brain::substrate::io::read_private_document(path)
                .with_context(|| format!("reading captured day page {}", path.display()))?;
            Ok(derive_day_page_doc(path, &content))
        }
        Some("fragments") => {
            let raw = crate::brain::substrate::io::read_private_document(path)
                .with_context(|| format!("reading captured fragment {}", path.display()))?;
            let (frontmatter, body) = BrainFrontmatter::parse(&raw)
                .with_context(|| format!("parsing captured fragment {}", path.display()))?;
            Ok(derive_fragment_doc(path, &frontmatter, &body))
        }
        Some("notes") => {
            let raw = crate::brain::substrate::io::read_private_document(path)
                .with_context(|| format!("reading captured note {}", path.display()))?;
            let (frontmatter, body) = BrainFrontmatter::parse(&raw)
                .with_context(|| format!("parsing captured note {}", path.display()))?;
            Ok(derive_note_doc(path, &frontmatter, &body))
        }
        _ => Ok(None),
    }
}

/// Call-site wrapper for capture write paths (substrate `append_to_day` /
/// `write_fragment` wrappers, Day Page editor saves). Real in production; a
/// no-op under `cfg(test)` so unrelated capture-path tests (day_trace,
/// sediment, day_page, substrate) that share the process-global test brain DB
/// cannot contaminate brain source counts. The behavior itself is unit-tested
/// directly via [`index_capture_now`] and proven end-to-end by the runtime
/// probe (`scripts/agentic/brain-instant-recall-probe.ts`).
#[cfg(not(test))]
pub fn index_capture_after_write(path: &Path) {
    index_capture_now(path);
}

#[cfg(test)]
pub fn index_capture_after_write(_path: &Path) {}

/// Mirror `;` capture stores into brain_docs: links (`links.md`) and snippets
/// (`snippets.md`). Notes captured via `;note` already arrive through
/// [`sync_notes`]. Todos now live on day pages (indexed in T7). Same contract
/// as the other mirrors: hash-guarded upserts, and deletion sync so a
/// link/snippet the user removed is forgotten by the brain.
fn sync_capture_stores() -> Result<usize> {
    sync_capture_stores_in_sk_path(&capture_sk_path())
}

fn capture_sk_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var(crate::setup::SK_PATH_ENV) {
        if !path.trim().is_empty() {
            return std::path::PathBuf::from(path);
        }
    }
    dirs::home_dir()
        .map(|home| home.join(".scriptkit"))
        .unwrap_or_else(|| std::path::PathBuf::from(".scriptkit"))
}

fn file_mtime_timestamp(path: &std::path::Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| {
            mtime
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
        })
        .unwrap_or(0)
}

pub(crate) fn sync_capture_stores_in_sk_path(sk_path: &std::path::Path) -> Result<usize> {
    let mut synced = 0usize;
    let mut live_ids: Vec<String> = Vec::new();

    // Links: plugins/main/scriptlets/links.md sections.
    let links_path = crate::scriptlets::link_markdown_store::links_markdown_path(sk_path);
    let links_ts = file_mtime_timestamp(&links_path);
    if let Ok(sections) = crate::scriptlets::link_markdown_store::load_link_sections(&links_path) {
        for section in sections {
            if section.id.trim().is_empty() {
                continue;
            }
            let mut content = section.url.clone().unwrap_or_default();
            if let Some(description) = &section.description {
                content.push_str(&format!("\n{description}"));
            }
            let title = format!("Link: {}", section.title);
            let source_id = format!("link:{}", section.id);
            store::upsert_doc(DocSource::Capture, &source_id, &title, &content, links_ts)?;
            live_ids.push(source_id);
            synced += 1;
        }
    }

    // Snippets: plugins/main/scriptlets/snippets.md sections.
    let snippets_path = crate::scriptlets::snippet_markdown_store::snippets_markdown_path(sk_path);
    let snippets_ts = file_mtime_timestamp(&snippets_path);
    if let Ok(sections) =
        crate::scriptlets::snippet_markdown_store::load_snippet_sections(&snippets_path)
    {
        for section in sections {
            if section.id.trim().is_empty() {
                continue;
            }
            let mut content = section.body.clone();
            if let Some(keyword) = &section.keyword {
                content.push_str(&format!("\nkeyword: {keyword}"));
            }
            if let Some(description) = &section.description {
                content.push_str(&format!("\n{description}"));
            }
            let title = format!("Snippet: {}", section.name);
            let source_id = format!("snippet:{}", section.id);
            store::upsert_doc(
                DocSource::Capture,
                &source_id,
                &title,
                &content,
                snippets_ts,
            )?;
            live_ids.push(source_id);
            synced += 1;
        }
    }

    let removed = store::retain_docs(DocSource::Capture, &live_ids)?;
    if removed > 0 {
        tracing::info!(target: "script_kit::brain", removed, "brain forgot deleted captures");
    }
    Ok(synced)
}

/// Number of consecutive embed failures before the backoff kicks in.
const EMBED_BACKOFF_THRESHOLD: u32 = 3;

/// Cross-cycle accounting for embed-helper failures, owned by the indexer loop.
/// Once the helper fails [`EMBED_BACKOFF_THRESHOLD`] times in a row we stop
/// hammering it for a few cycles instead of respawning-and-failing every 2min.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EmbedBackoff {
    consecutive_failures: u32,
    cycles_since_failure: u32,
}

impl EmbedBackoff {
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.cycles_since_failure = 0;
    }

    fn record_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.cycles_since_failure = 0;
    }

    fn note_skipped_cycle(&mut self) {
        self.cycles_since_failure = self.cycles_since_failure.saturating_add(1);
    }

    /// Whether the embedder is currently in its cool-down window.
    fn is_backing_off(&self) -> bool {
        should_skip_embed(self.consecutive_failures, self.cycles_since_failure)
    }
}

/// Pure backoff decision, split out so the cross-cycle accounting is unit
/// testable without a subprocess: after [`EMBED_BACKOFF_THRESHOLD`] consecutive
/// failures, skip the embed phase for the next `failures - (THRESHOLD - 1)`
/// cycles (simple linear backoff — 3 failures skips 1 cycle, 4 skips 2, ...).
pub(crate) fn should_skip_embed(consecutive_failures: u32, cycles_since_failure: u32) -> bool {
    consecutive_failures >= EMBED_BACKOFF_THRESHOLD
        && cycles_since_failure < consecutive_failures - (EMBED_BACKOFF_THRESHOLD - 1)
}

/// Embed docs with missing/stale vectors. Returns the number embedded. Owns the
/// respawn/backoff policy: a dead or model-mismatched helper is dropped and
/// respawned, an embed error drops the slot so the next cycle gets a fresh
/// pipe, and repeated failures trip [`EmbedBackoff`] to skip the phase.
fn embed_pending(
    embedder: &mut Option<BrainEmbedder>,
    backoff: &mut EmbedBackoff,
) -> Result<usize> {
    if backoff.is_backing_off() {
        backoff.note_skipped_cycle();
        tracing::warn!(
            target: "script_kit::brain",
            consecutive_failures = backoff.consecutive_failures,
            "brain embedder backing off; skipping embed phase this cycle"
        );
        return Ok(0);
    }
    let result = embed_pending_once(embedder);
    match &result {
        Ok(_) => backoff.record_success(),
        Err(_) => {
            // Broken pipe, timeout, or a crashed helper: drop the slot so the
            // next cycle respawns instead of reusing a dead process, and step
            // the backoff so we stop respawning-and-failing on every cycle.
            *embedder = None;
            backoff.record_failure();
        }
    }
    result
}

/// One embed attempt against a live helper: (re)spawn as needed, then run the
/// chunk/batch/split bookkeeping. Split from [`embed_pending`] so the borrow of
/// the embedder inside the embed closure ends before the caller can null the
/// slot on error.
fn embed_pending_once(embedder: &mut Option<BrainEmbedder>) -> Result<usize> {
    if resolve_embed_model().is_none() {
        // Zero-setup semantic search: fetch the model once the brain has
        // content worth embedding (politeness rules in brain::download).
        let (docs, _, _) = store::doc_stats().unwrap_or((0, 0, 0));
        if !super::download::ensure_embed_model(docs > 0) {
            return Ok(0); // FTS-only mode: no model on disk (yet).
        }
    }
    let Some(model) = resolve_embed_model() else {
        return Ok(0);
    };
    // Drop a cached helper whose model changed OR whose child has died; a
    // crashed helper was previously reused forever until app restart.
    if embedder
        .as_ref()
        .is_some_and(|e| e.model_id() != model.model_id || !e.is_alive())
    {
        *embedder = None;
    }
    if embedder.is_none() {
        *embedder = Some(BrainEmbedder::spawn(model)?);
    }
    let Some(e) = embedder.as_ref() else {
        // Unreachable: set to `Some` directly above. Fail soft instead of
        // panicking so a spawn-vs-drop race can never abort the indexer.
        return Err(anyhow::anyhow!(
            "brain embedder missing immediately after spawn"
        ));
    };
    let model_id = e.model_id().to_string();
    embed_pending_with(&model_id, |texts| e.embed(texts))
}

/// The embed cycle with the embedder injected — the chunk → one-batch-call →
/// split-back-per-doc bookkeeping is testable without the helper subprocess.
pub(crate) fn embed_pending_with(
    model_id: &str,
    mut embed: impl FnMut(Vec<String>) -> Result<Vec<Vec<f32>>>,
) -> Result<usize> {
    let mut total = 0usize;
    while total < MAX_EMBED_PER_CYCLE {
        let pending = store::docs_needing_embedding(model_id, EMBED_BATCH)?;
        if pending.is_empty() {
            break;
        }
        // qmd-style chunking: long docs embed as ~900-token pieces with
        // overlap so nothing past a truncation cap goes semantically dark.
        // All chunks of the batch ride one embed call, then split back out.
        let doc_chunks: Vec<Vec<super::chunker::Chunk>> = pending
            .iter()
            .map(|doc| super::chunker::chunk_markdown(&format!("{}\n{}", doc.title, doc.content)))
            .collect();
        let texts: Vec<String> = doc_chunks
            .iter()
            .flat_map(|chunks| chunks.iter().map(|chunk| chunk.text.clone()))
            .collect();
        if texts.is_empty() {
            // Whitespace-only docs produce zero chunks; store an empty set so
            // they stop reporting as pending.
            for doc in &pending {
                store::store_chunk_embeddings(doc.id, model_id, &doc.title, &doc.content, &[])?;
                total += 1;
            }
            continue;
        }
        let vectors = embed(texts)?;
        let mut cursor = 0usize;
        let mut stored_this_round = 0usize;
        for (doc, chunks) in pending.iter().zip(doc_chunks.iter()) {
            let end = cursor + chunks.len();
            if end > vectors.len() {
                break; // embedder returned a short batch; retry next cycle
            }
            let chunk_vecs: Vec<(usize, Vec<f32>)> = chunks
                .iter()
                .zip(vectors[cursor..end].iter())
                .map(|(chunk, vec)| (chunk.start, vec.clone()))
                .collect();
            cursor = end;
            if chunk_vecs.iter().all(|(_, vec)| vec.is_empty()) && !chunks.is_empty() {
                continue; // embedder returned nothing usable; retry next cycle
            }
            store::store_chunk_embeddings(doc.id, model_id, &doc.title, &doc.content, &chunk_vecs)?;
            total += 1;
            stored_this_round += 1;
        }
        if stored_this_round == 0 {
            // Nothing stored means the same docs would come back pending —
            // bail rather than spin inside one cycle.
            break;
        }
    }
    Ok(total)
}

/// Ingest one chat turn into the brain (called post-turn from Agent Chat).
/// `thread_id` + `turn_index` form the stable identity; re-ingesting the same
/// turn is a no-op thanks to hash guards.
pub(crate) fn chat_turn_source_id(thread_id: &str, turn_index: usize) -> String {
    format!("{thread_id}#{turn_index}")
}

pub fn ingest_chat_turn(
    thread_id: &str,
    turn_index: usize,
    user_text: &str,
    assistant_text: &str,
) -> Result<()> {
    store::init_brain_db()?;
    let user_text = user_text.trim();
    let assistant_text = assistant_text.trim();
    if user_text.is_empty() && assistant_text.is_empty() {
        return Ok(());
    }
    let title: String = user_text.chars().take(80).collect();
    let content = format!("User: {user_text}\n\nAssistant: {assistant_text}");
    let source_id = chat_turn_source_id(thread_id, turn_index);
    let now = chrono::Utc::now().timestamp();
    store::upsert_doc(DocSource::ChatTurn, &source_id, &title, &content, now)?;
    // The user's own words are the strongest attention signal we have.
    for topic in extract_topics(user_text) {
        let _ = store::record_signal(&topic, 2, "chat");
    }
    wake_indexer();
    Ok(())
}

/// Words too generic to ever stand as an attention topic on their own —
/// conversational filler ("again", "else", "really") accumulates ask weight
/// fast and then surfaces verbatim as Brain Inbox drift titles. Shared with
/// the curator's drift gate via [`is_substantive_topic`] so both layers
/// agree on what counts as substance. Words of 3 chars or fewer are already
/// dropped by the length filter and don't need listing.
const GENERIC_WORDS: &[&str] = &[
    "about",
    "after",
    "again",
    "also",
    "another",
    "anything",
    "around",
    "back",
    "because",
    "been",
    "before",
    "being",
    "could",
    "does",
    "doing",
    "done",
    "down",
    "each",
    "else",
    "even",
    "ever",
    "every",
    "everything",
    "first",
    "from",
    "getting",
    "give",
    "goes",
    "going",
    "gonna",
    "good",
    "have",
    "help",
    "here",
    "into",
    "just",
    "know",
    "later",
    "lets",
    "like",
    "made",
    "make",
    "many",
    "maybe",
    "more",
    "most",
    "much",
    "need",
    "never",
    "nothing",
    "okay",
    "only",
    "other",
    "over",
    "please",
    "really",
    "right",
    "same",
    "second",
    "should",
    "show",
    "some",
    "somehow",
    "something",
    "soon",
    "still",
    "stuff",
    "sure",
    "take",
    "tell",
    "than",
    "thanks",
    "that",
    "their",
    "them",
    "then",
    "there",
    "these",
    "they",
    "thing",
    "things",
    "think",
    "this",
    "those",
    "time",
    "today",
    "tomorrow",
    "trying",
    "very",
    "want",
    "well",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "will",
    "with",
    "without",
    "would",
    "yeah",
    "yesterday",
    "your",
];

/// Whether a candidate word carries enough meaning to be (part of) a topic.
fn is_substantive_word(word: &str) -> bool {
    word.len() > 3 && !GENERIC_WORDS.contains(&word)
}

/// Whether a topic/title string contains at least one substantive word.
/// The curator uses this to refuse drift inbox titles like "again" or
/// "else" while still accepting "second brain" or "build script".
pub(crate) fn is_substantive_topic(topic: &str) -> bool {
    topic
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .any(is_substantive_word)
}

/// Cheap, deterministic topic extraction: significant lowercase words and
/// adjacent pairs. No model in the hot path, by design.
pub(crate) fn extract_topics(text: &str) -> Vec<String> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| is_substantive_word(w))
        .map(|w| w.to_string())
        .collect();
    let mut topics: Vec<String> = Vec::new();
    for window in words.windows(2) {
        topics.push(format!("{} {}", window[0], window[1]));
    }
    for word in &words {
        topics.push(word.clone());
    }
    topics.truncate(8);
    topics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn brain_private_indexer_repairs_legacy_markdown_before_deriving_content() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempfile::tempdir().expect("isolated brain index fixture");
        let brain = fixture.path().join("brain");
        let days = brain.join("days");
        fs::create_dir_all(&days).unwrap();
        fs::set_permissions(&brain, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&days, fs::Permissions::from_mode(0o755)).unwrap();
        let day = days.join("2026-08-22.md");
        fs::write(&day, "09:42 private indexed capture").unwrap();
        fs::set_permissions(&day, fs::Permissions::from_mode(0o644)).unwrap();

        let doc = derive_capture_doc(&day)
            .expect("read canonical private day")
            .expect("derive canonical day document");
        assert_eq!(doc.source, DocSource::DayPage);
        assert_eq!(
            fs::metadata(brain).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(days).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(day).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn brain_private_indexer_refuses_symlinked_document_owners_without_foreign_reads() {
        use std::os::unix::fs::{symlink, PermissionsExt as _};

        let fixture = tempfile::tempdir().expect("isolated brain index symlink fixture");
        let brain = fixture.path().join("brain");
        fs::create_dir(&brain).unwrap();
        let foreign_dir = fixture.path().join("foreign-days");
        fs::create_dir(&foreign_dir).unwrap();
        fs::set_permissions(&foreign_dir, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(foreign_dir.join("2026-08-22.md"), "foreign private day").unwrap();
        let days = brain.join("days");
        symlink(&foreign_dir, &days).expect("plant hostile Brain directory");

        assert!(list_markdown_files(&days).is_err());
        assert_eq!(
            fs::metadata(&foreign_dir).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            fs::read_to_string(foreign_dir.join("2026-08-22.md")).unwrap(),
            "foreign private day"
        );
    }

    #[cfg(unix)]
    #[test]
    fn brain_private_indexer_rejects_symlinked_markdown_before_sync_or_capture() {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir().expect("isolated brain file symlink fixture");
        let days = fixture.path().join("brain").join("days");
        fs::create_dir_all(&days).unwrap();
        let foreign = fixture.path().join("foreign.md");
        fs::write(&foreign, "foreign private text").unwrap();
        let planted = days.join("2026-08-22.md");
        symlink(&foreign, &planted).expect("plant hostile indexed file");

        assert!(list_markdown_files(&days).is_err());
        assert!(derive_capture_doc(&planted).is_err());
        assert_eq!(fs::read_to_string(foreign).unwrap(), "foreign private text");
        assert!(fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn pinned_clipboard_doc_is_deeplink_only() {
        let entry = crate::clipboard_history::ClipboardEntry {
            id: "clip-raw-free".to_string(),
            content: "raw secret copied text".to_string(),
            content_type: crate::clipboard_history::ContentType::Text,
            timestamp: 123,
            pinned: true,
            ocr_text: Some("raw image ocr".to_string()),
            source_app_name: None,
            source_app_bundle_id: None,
        };

        let (title, body) = pinned_clipboard_doc(&entry);

        assert_eq!(title, "Pinned clipboard entry");
        assert_eq!(body, "kit://clipboard-history?id=clip-raw-free".to_string());
        assert!(!title.contains("raw secret"));
        assert!(!body.contains("raw secret"));
        assert!(!title.contains("raw image"));
        assert!(!body.contains("raw image"));
    }
}

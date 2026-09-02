//! Clipboard monitoring
//!
//! Background threads for clipboard polling and entry maintenance.

use anyhow::{Context, Result};
use arboard::Clipboard;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::cache::{
    cache_image, get_cached_entries, get_cached_image, init_cache_timestamp, refresh_entry_cache,
};
use super::change_detection::ClipboardChangeDetector;
use super::config::{get_max_text_content_len, get_retention_days, is_text_over_limit};
use super::database::{
    add_entry, get_connection, get_entry_content, prune_old_entries, run_incremental_vacuum,
    run_wal_checkpoint, trim_oversize_text_entries,
};
use super::image::{compute_image_hash, decode_to_render_image, encode_image_as_blob};
use super::ocr;
use super::rejection::{
    contains_concealed_pasteboard_type, evaluate_text_capture_rejection, record_rejection,
    reject_before_reading_payload, ClipboardPreCaptureProbe, RejectionReason,
};
use super::sediment::process_text_sediment;
use super::types::ContentType;

/// Interval between background pruning checks (1 hour)
const PRUNE_INTERVAL_SECS: u64 = 3600;

/// Polling interval for clipboard changes
const POLL_INTERVAL_MS: u64 = 500;

/// Flag to stop the monitoring thread (AtomicBool for lock-free polling)
static STOP_MONITORING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Guard to ensure init_clipboard_history() is only called once
static INIT_GUARD: OnceLock<()> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
struct LastImageState {
    hash: u64,
    blob_key: Option<String>,
}

impl LastImageState {
    fn with_blob_key(hash: u64, blob_key: String) -> Self {
        Self {
            hash,
            blob_key: Some(blob_key),
        }
    }

    fn without_blob_key(hash: u64) -> Self {
        Self {
            hash,
            blob_key: None,
        }
    }
}

/// Text payload accepted by the DevTools clipboard-capture fixture.
///
/// Large payloads use `textFile` because stdin JSONL is intentionally capped at
/// 16 KiB. The file must resolve under the active sandbox's `SK_PATH`.
#[derive(Clone, Debug, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
pub enum ClipboardCaptureFixturePayload {
    Text { text: String },
    TextFile { path: PathBuf },
}

/// Redacted result from a synthetic capture. Payload content is never carried
/// in protocol responses or logs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardCaptureFixtureOutcome {
    Stored { entry_id: String },
    Rejected { reason: RejectionReason },
    Oversized { text_len: usize, max_len: usize },
    Empty,
    UnchangedGeneration,
    Failed { error: String },
}

#[derive(Debug, Default)]
struct ClipboardCaptureFixtureState {
    last_change_generation: Option<u64>,
}

static CLIPBOARD_CAPTURE_FIXTURE_STATE: OnceLock<Mutex<ClipboardCaptureFixtureState>> =
    OnceLock::new();

fn clipboard_capture_fixture_state() -> &'static Mutex<ClipboardCaptureFixtureState> {
    CLIPBOARD_CAPTURE_FIXTURE_STATE
        .get_or_init(|| Mutex::new(ClipboardCaptureFixtureState::default()))
}

fn clipboard_fixture_root() -> Result<PathBuf> {
    if let Some(policy) = crate::runtime_policy::owned_evaluation() {
        return Ok(policy.root().to_path_buf());
    }
    let root = std::env::var_os("SK_PATH")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".scriptkit")))
        .context("clipboard fixture requires SK_PATH or HOME")?;
    root.canonicalize().with_context(|| {
        format!(
            "failed to resolve clipboard fixture root {}",
            root.display()
        )
    })
}

fn read_clipboard_fixture_text(payload: ClipboardCaptureFixturePayload) -> Result<String> {
    match payload {
        ClipboardCaptureFixturePayload::Text { text } => Ok(text),
        ClipboardCaptureFixturePayload::TextFile { path } => {
            if let Some(policy) = crate::runtime_policy::owned_evaluation() {
                policy.require_owned_path(&path)?;
            }
            let root = clipboard_fixture_root()?;
            let resolved = path.canonicalize().with_context(|| {
                format!("failed to resolve clipboard fixture {}", path.display())
            })?;
            if !resolved.starts_with(&root) {
                anyhow::bail!(
                    "clipboard fixture path must stay under SK_PATH (root={}, path={})",
                    root.display(),
                    resolved.display()
                );
            }
            std::fs::read_to_string(&resolved)
                .with_context(|| format!("failed to read clipboard fixture {}", resolved.display()))
        }
    }
}

fn capture_clipboard_fixture_with<Load, Add, Sediment>(
    state: &mut ClipboardCaptureFixtureState,
    change_generation: u64,
    source_bundle_id: Option<&str>,
    concealed_types: &[String],
    load_text: Load,
    add_entry: Add,
    process_sediment: Sediment,
) -> ClipboardCaptureFixtureOutcome
where
    Load: FnOnce() -> Result<String>,
    Add: FnOnce(&str) -> Result<String>,
    Sediment: FnOnce(&str, &str),
{
    if state.last_change_generation == Some(change_generation) {
        return ClipboardCaptureFixtureOutcome::UnchangedGeneration;
    }
    // Mirrors ClipboardChangeDetector: observing a new generation advances the
    // detector before any payload read or persistence attempt.
    state.last_change_generation = Some(change_generation);

    let pre_capture = ClipboardPreCaptureProbe {
        pasteboard_has_concealed_types: contains_concealed_pasteboard_type(
            concealed_types.iter().map(String::as_str),
        ),
    };
    if let Some(reason) = reject_before_reading_payload(source_bundle_id, pre_capture) {
        record_rejection(reason);
        return ClipboardCaptureFixtureOutcome::Rejected { reason };
    }

    let text = match load_text() {
        Ok(text) => text,
        Err(error) => {
            return ClipboardCaptureFixtureOutcome::Failed {
                error: error.to_string(),
            };
        }
    };
    if text.is_empty() {
        return ClipboardCaptureFixtureOutcome::Empty;
    }
    if is_text_over_limit(&text) {
        let max_len = get_max_text_content_len();
        warn!(
            text_len = text.len(),
            max_len, change_generation, "Skipping oversized clipboard text fixture entry"
        );
        return ClipboardCaptureFixtureOutcome::Oversized {
            text_len: text.len(),
            max_len,
        };
    }

    match capture_text_entry(&text, source_bundle_id, pre_capture, add_entry) {
        TextCaptureOutcome::Stored(entry_id) => {
            process_sediment(&entry_id, &text);
            ClipboardCaptureFixtureOutcome::Stored { entry_id }
        }
        TextCaptureOutcome::Rejected(reason) => ClipboardCaptureFixtureOutcome::Rejected { reason },
        TextCaptureOutcome::Failed(error) => ClipboardCaptureFixtureOutcome::Failed { error },
    }
}

/// Inject one synthetic text capture through the production rejection, size,
/// dedupe, and sediment pipeline without touching NSPasteboard.
pub fn inject_clipboard_capture_fixture(
    payload: ClipboardCaptureFixturePayload,
    source_bundle_id: Option<&str>,
    concealed_types: &[String],
    change_generation: u64,
) -> ClipboardCaptureFixtureOutcome {
    let mut state = match clipboard_capture_fixture_state().lock() {
        Ok(state) => state,
        Err(error) => {
            return ClipboardCaptureFixtureOutcome::Failed {
                error: format!("clipboard fixture state lock poisoned: {error}"),
            };
        }
    };
    capture_clipboard_fixture_with(
        &mut state,
        change_generation,
        source_bundle_id,
        concealed_types,
        || read_clipboard_fixture_text(payload),
        |text| add_entry(text, ContentType::Text),
        |entry_id, text| process_text_sediment(entry_id, text, chrono::Utc::now()),
    )
}

/// Initialize clipboard history: create DB and start monitoring
///
/// This should be called once at application startup. It will:
/// 1. Create the SQLite database if it doesn't exist (with WAL mode)
/// 2. Run initial pruning of old entries
/// 3. Pre-warm the entry cache
/// 4. Pre-decode images in background
/// 5. Start a background thread that polls the clipboard every 500ms
/// 6. Start a background pruning job (runs hourly)
///
/// # Errors
/// Returns error if database creation fails.
pub fn init_clipboard_history() -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::GlobalMonitor)?;
    if std::env::var("SCRIPT_KIT_DISABLE_CLIPBOARD_MONITOR").is_ok_and(|value| value == "1") {
        info!("Clipboard monitor disabled for sandboxed DevTools fixture injection");
        return Ok(());
    }

    // Ensure init is only called once (idempotency guard)
    if INIT_GUARD.set(()).is_err() {
        debug!("Clipboard history already initialized, skipping");
        return Ok(());
    }

    info!(
        retention_days = get_retention_days(),
        "Initializing clipboard history"
    );

    // Initialize the database connection (enables WAL, runs migrations)
    let _conn = get_connection().context("Failed to initialize database")?;

    // Initialize the cache timestamp
    init_cache_timestamp();

    // Run initial pruning of old entries
    if let Err(e) = prune_old_entries() {
        warn!(error = %e, "Initial pruning failed");
    }

    // Remove oversized text entries before caching
    if let Err(e) = trim_oversize_text_entries() {
        let correlation_id = Uuid::new_v4().to_string();
        warn!(
            correlation_id = %correlation_id,
            error = %e,
            "Initial oversize trim failed"
        );
    }

    // Pre-warm the entry cache from database
    refresh_entry_cache();

    // Pre-decode images in a background thread
    thread::spawn(|| {
        prewarm_image_cache();
    });

    // Initialize the stop flag (AtomicBool for lock-free polling)
    let stop_flag = Arc::new(AtomicBool::new(false));
    let _ = STOP_MONITORING.set(stop_flag.clone());

    // Start the monitoring thread
    let stop_flag_clone = stop_flag.clone();
    thread::spawn(move || {
        if let Err(e) = clipboard_monitor_loop(stop_flag_clone) {
            error!(error = %e, "Clipboard monitor thread failed");
        }
    });

    // Start background pruning thread (runs hourly)
    let stop_flag_prune = stop_flag.clone();
    thread::spawn(move || {
        background_prune_loop(stop_flag_prune);
    });

    let _ = ocr::start_ocr_worker();

    info!("Clipboard history initialized");
    Ok(())
}

/// Stop the clipboard monitoring thread
#[allow(dead_code)]
pub fn stop_clipboard_monitoring() {
    if let Some(stop_flag) = STOP_MONITORING.get() {
        stop_flag.store(true, Ordering::Relaxed);
        info!("Clipboard monitoring stopped");
    }

    let _ = ocr::stop_ocr_worker();
}

/// Background loop that monitors clipboard changes
///
/// Uses macOS NSPasteboard changeCount for efficient polling when available.
/// This is dramatically cheaper than reading clipboard payloads every 500ms,
/// especially for large content (images can be 100MB+).
///
/// The change count approach also fixes a correctness bug: with content-based
/// detection, copying the same text twice in a row doesn't update the timestamp.
/// With change count detection, we detect all clipboard changes regardless of content.
fn clipboard_monitor_loop(stop_flag: Arc<AtomicBool>) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    let mut clipboard = Clipboard::new().context("Failed to create clipboard instance")?;
    let mut change_detector = ClipboardChangeDetector::new();

    // Content-based tracking for deduplication after OS-level change detection
    // These are updated AFTER we read payload to avoid re-processing same content
    let mut last_text_hash: Option<u64> = None;
    let mut last_image_state: Option<LastImageState> = None;

    let poll_interval = Duration::from_millis(POLL_INTERVAL_MS);

    // Reduced poll interval when using change count (cheap operation)
    // 200ms is responsive enough for clipboard while reducing CPU from 20 to 5 wakeups/sec
    let fast_poll_interval = Duration::from_millis(200);

    // Check if we have efficient change detection available
    let has_change_detection = change_detector.has_changed().is_some();

    info!(
        poll_interval_ms = if has_change_detection {
            200
        } else {
            POLL_INTERVAL_MS
        },
        has_change_detection = has_change_detection,
        "Clipboard monitor started"
    );

    loop {
        // Check if we should stop (lock-free with AtomicBool)
        if stop_flag.load(Ordering::Relaxed) {
            info!("Clipboard monitor stopping");
            break;
        }

        let start = Instant::now();

        // Check if clipboard changed using efficient OS-level detection when available
        let should_check_payload = change_detector.has_changed().unwrap_or(true);

        if should_check_payload {
            // Clipboard changed (or fallback mode), now read the actual payload
            capture_clipboard_content(&mut clipboard, &mut last_text_hash, &mut last_image_state);
        }

        // Sleep for remaining time in poll interval
        // Use faster polling when we have cheap change detection
        let target_interval = if has_change_detection {
            fast_poll_interval
        } else {
            poll_interval
        };

        let elapsed = start.elapsed();
        if elapsed < target_interval {
            thread::sleep(target_interval - elapsed);
        }
    }

    Ok(())
}

/// Capture current clipboard content and add to history if new.
///
/// Uses content hashing for deduplication after OS-level change detection.
/// This is called only when the OS reports a clipboard change, making it
/// much more efficient than reading payloads on every poll.
fn capture_clipboard_content(
    clipboard: &mut Clipboard,
    last_text_hash: &mut Option<u64>,
    last_image_state: &mut Option<LastImageState>,
) {
    // Skip capture when a programmatic write (e.g. paste-sequential) is in progress.
    if super::clipboard::SUPPRESS_CLIPBOARD_CAPTURE.load(std::sync::atomic::Ordering::SeqCst) {
        debug!("clipboard_capture_suppressed_by_flag");
        return;
    }

    let source_bundle_id = crate::frontmost_app_tracker::get_last_real_app_bundle_id();
    let pre_capture = ClipboardPreCaptureProbe::live();
    if let Some(reason) = reject_before_reading_payload(source_bundle_id.as_deref(), pre_capture) {
        record_rejection(reason);
        debug!(
            reason = ?reason,
            source_bundle_id = %source_bundle_id.as_deref().unwrap_or("unknown"),
            "clipboard_capture_rejected_before_read"
        );
        return;
    }

    // Check for text changes
    if let Ok(text) = clipboard.get_text() {
        if !text.is_empty() {
            let text_hash = compute_text_hash(&text);

            // Check if content actually changed (handles same content copied twice)
            let is_new_content = last_text_hash.is_none_or(|last| last != text_hash);

            if is_new_content {
                debug!(text_len = text.len(), "New text detected in clipboard");

                if is_text_over_limit(&text) {
                    let correlation_id = Uuid::new_v4().to_string();
                    warn!(
                        correlation_id = %correlation_id,
                        text_len = text.len(),
                        max_len = get_max_text_content_len(),
                        "Skipping oversized clipboard text entry"
                    );
                    // Update hash even for oversized entries (intentionally skipped)
                    *last_text_hash = Some(text_hash);
                } else {
                    match capture_text_entry(
                        &text,
                        source_bundle_id.as_deref(),
                        pre_capture,
                        |content| add_entry(content, ContentType::Text),
                    ) {
                        TextCaptureOutcome::Stored(entry_id) => {
                            debug!(entry_id = %entry_id, "Added text entry to history");
                            process_text_sediment(&entry_id, &text, chrono::Utc::now());
                            *last_text_hash = Some(text_hash);
                        }
                        TextCaptureOutcome::Rejected(reason) => {
                            debug!(reason = ?reason, "Rejected text clipboard capture");
                            *last_text_hash = Some(text_hash);
                        }
                        TextCaptureOutcome::Failed(error) => {
                            // DON'T update hash on failure - we'll retry on next change
                            warn!(
                                error = %error,
                                "Failed to add text entry to history (will retry)"
                            );
                        }
                    }
                }
            } else {
                // Same content, but OS detected a change - this means user copied same text again
                // Update the timestamp to bubble this entry to the top
                debug!(
                    text_len = text.len(),
                    "Same text copied again, updating timestamp"
                );
                match capture_text_entry(
                    &text,
                    source_bundle_id.as_deref(),
                    pre_capture,
                    |content| add_entry(content, ContentType::Text),
                ) {
                    TextCaptureOutcome::Stored(entry_id) => {
                        debug!(
                            entry_id = %entry_id,
                            "Updated timestamp for existing text entry"
                        );
                        process_text_sediment(&entry_id, &text, chrono::Utc::now());
                    }
                    TextCaptureOutcome::Rejected(reason) => {
                        debug!(reason = ?reason, "Rejected repeated text clipboard capture");
                    }
                    TextCaptureOutcome::Failed(error) => {
                        warn!(error = %error, "Failed to update text entry timestamp");
                    }
                }
            }
            return; // Text takes priority, don't check image
        }
    }

    // Check for image changes (only if no text was found)
    if let Ok(image_data) = clipboard.get_image() {
        let hash = compute_image_hash(&image_data);

        let is_new_content = !is_same_image_hash(last_image_state, hash);

        if is_new_content {
            debug!(
                width = image_data.width,
                height = image_data.height,
                "New image detected in clipboard"
            );

            // Encode image as blob (PNG file on disk)
            match encode_image_as_blob(&image_data) {
                Ok(blob_key) => {
                    match add_entry(&blob_key, ContentType::Image) {
                        Ok(entry_id) => {
                            let _ = ocr::enqueue_ocr(entry_id.clone(), blob_key.clone());

                            // Pre-decode the image immediately so it's ready for display
                            if let Some(render_image) = decode_to_render_image(&blob_key) {
                                cache_image(&entry_id, render_image);
                                debug!(entry_id = %entry_id, "Pre-cached new image during monitoring");
                            }

                            *last_image_state = Some(LastImageState::with_blob_key(hash, blob_key));
                        }
                        Err(e) => {
                            // DON'T update state on DB failure - we'll retry on next change
                            warn!(error = %e, "Failed to add image entry to history (will retry)");
                        }
                    }
                }
                Err(e) => {
                    // Encoding failed (likely corrupt image data). Cache hash without blob key
                    // so we avoid repeated expensive attempts for the same image bytes.
                    warn!(
                        error = %e,
                        image_hash = hash,
                        "Failed to encode image as blob; caching hash without blob key"
                    );
                    *last_image_state = Some(LastImageState::without_blob_key(hash));
                }
            }
        } else {
            // Same image, but OS detected a change - update timestamp
            debug!(
                width = image_data.width,
                height = image_data.height,
                "Same image copied again, updating timestamp"
            );
            if let Some(blob_key) = cached_blob_key_for_hash(last_image_state, hash) {
                match add_entry(blob_key, ContentType::Image) {
                    Ok(entry_id) => {
                        debug!(
                            entry_id = %entry_id,
                            image_hash = hash,
                            "Updated timestamp for existing image entry using cached blob key"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to update image entry timestamp");
                    }
                }
            } else {
                debug!(
                    image_hash = hash,
                    "Same image copied again but no cached blob key is available; skipping re-encode"
                );
            }
        }
    }
}

fn is_same_image_hash(last_image_state: &Option<LastImageState>, hash: u64) -> bool {
    last_image_state
        .as_ref()
        .is_some_and(|state| state.hash == hash)
}

fn cached_blob_key_for_hash(last_image_state: &Option<LastImageState>, hash: u64) -> Option<&str> {
    last_image_state
        .as_ref()
        .filter(|state| state.hash == hash)
        .and_then(|state| state.blob_key.as_deref())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TextCaptureOutcome {
    Stored(String),
    Rejected(RejectionReason),
    Failed(String),
}

fn capture_text_entry(
    text: &str,
    source_bundle_id: Option<&str>,
    probe: ClipboardPreCaptureProbe,
    add_entry: impl FnOnce(&str) -> Result<String>,
) -> TextCaptureOutcome {
    if let Some(reason) = evaluate_text_capture_rejection(source_bundle_id, probe, text) {
        record_rejection(reason);
        return TextCaptureOutcome::Rejected(reason);
    }

    match add_entry(text) {
        Ok(entry_id) => TextCaptureOutcome::Stored(entry_id),
        Err(error) => TextCaptureOutcome::Failed(error.to_string()),
    }
}

/// Compute a simple hash of text content for change detection.
fn compute_text_hash(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Sleep with interruptible checks against a stop flag.
/// Returns true if sleep completed, false if stop was signaled.
fn interruptible_sleep(duration: Duration, stop_flag: &AtomicBool) -> bool {
    let check_interval = Duration::from_secs(1);
    let mut remaining = duration;

    while remaining > Duration::ZERO {
        if stop_flag.load(Ordering::Relaxed) {
            return false;
        }
        let sleep_time = remaining.min(check_interval);
        thread::sleep(sleep_time);
        remaining = remaining.saturating_sub(sleep_time);
    }
    true
}

/// Background loop that periodically prunes old entries
fn background_prune_loop(stop_flag: Arc<AtomicBool>) {
    let prune_interval = Duration::from_secs(PRUNE_INTERVAL_SECS);
    let mut prune_count: u32 = 0;

    loop {
        // Sleep first (initial prune already happened during init)
        // Use interruptible sleep so stop_clipboard_monitoring() takes effect within ~1s
        if !interruptible_sleep(prune_interval, &stop_flag) {
            info!("Background prune thread stopping (interrupted during sleep)");
            break;
        }

        // Check if we should stop (lock-free with AtomicBool)
        if stop_flag.load(Ordering::Relaxed) {
            info!("Background prune thread stopping");
            break;
        }

        // Prune old entries
        match prune_old_entries() {
            Ok(count) => {
                if count > 0 {
                    info!(pruned = count, "Background pruning completed");
                    refresh_entry_cache();
                }

                // Reclaim disk space incrementally after successful prune
                if let Err(e) = run_incremental_vacuum() {
                    warn!(error = %e, "Incremental vacuum failed");
                }
            }
            Err(e) => {
                warn!(error = %e, "Background pruning failed");
            }
        }

        prune_count += 1;

        // Checkpoint WAL every 10 prune cycles to bound WAL file growth
        if prune_count.is_multiple_of(10) {
            if let Err(e) = run_wal_checkpoint() {
                warn!(error = %e, "WAL checkpoint failed");
            } else {
                debug!(cycle = prune_count, "WAL checkpoint completed");
            }
        }
    }
}

/// Pre-warm the image cache by decoding cached image entries
///
/// This fetches content on-demand for each image entry to avoid
/// keeping all image payloads in memory during normal list views.
fn prewarm_image_cache() {
    let entries = get_cached_entries(100);
    let mut decoded_count = 0;

    for entry in entries {
        if entry.content_type == ContentType::Image {
            // Skip if already cached
            if get_cached_image(&entry.id).is_some() {
                continue;
            }

            // Fetch content on-demand and decode
            if let Some(content) = get_entry_content(&entry.id) {
                if let Some(render_image) = decode_to_render_image(&content) {
                    cache_image(&entry.id, render_image);
                    decoded_count += 1;
                }
            }
        }
    }

    info!(decoded_count, "Pre-warmed image cache");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_guard_exists() {
        let _guard: &OnceLock<()> = &INIT_GUARD;
    }

    #[test]
    fn test_stop_monitoring_is_atomic() {
        fn assert_atomic_bool(_: &OnceLock<Arc<AtomicBool>>) {}
        assert_atomic_bool(&STOP_MONITORING);
    }

    #[test]
    fn test_atomic_bool_operations() {
        let flag = Arc::new(AtomicBool::new(false));

        assert!(!flag.load(Ordering::Relaxed));

        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));

        flag.store(false, Ordering::Relaxed);
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_retry_on_db_failure_behavior() {
        // Verify the retry logic by checking Option behavior
        // When add_entry fails, last_text should remain None (or previous value)
        // allowing retry on next poll

        // Simulate first successful add
        let mut last_text: Option<String> = Some("success".to_string());
        assert_eq!(last_text.as_deref(), Some("success"));

        // Simulate failed add - the key insight is that on failure,
        // we DON'T update last_text. So if we check whether new_text != last_text,
        // a failed entry will be retried on the next poll.
        let new_text = "new_entry".to_string();

        // Check if it's "new" (different from last)
        let is_new = last_text.as_ref() != Some(&new_text);
        assert!(is_new, "new_text should be detected as new");

        // On failure, we DON'T update last_text (simulating the implemented behavior)
        // This means is_new will STILL be true on the next poll iteration

        // On success, we DO update:
        last_text = Some(new_text.clone());
        let is_new_after_success = last_text.as_ref() != Some(&new_text);
        assert!(
            !is_new_after_success,
            "After success, same text should not be 'new'"
        );
    }

    #[test]
    fn test_capture_text_entry_rejects_concealed_pasteboard_flag_without_add_entry() {
        let add_called = std::cell::Cell::new(false);
        let outcome = capture_text_entry(
            "hello",
            None,
            ClipboardPreCaptureProbe {
                pasteboard_has_concealed_types: true,
            },
            |_| {
                add_called.set(true);
                Ok("entry-id".to_string())
            },
        );

        assert_eq!(
            outcome,
            TextCaptureOutcome::Rejected(RejectionReason::ConcealedPasteboardType)
        );
        assert!(
            !add_called.get(),
            "add_entry must not run when concealed pasteboard types are present"
        );
    }

    #[test]
    fn test_capture_text_entry_rejects_blocked_source_without_add_entry() {
        let add_called = std::cell::Cell::new(false);
        let outcome = capture_text_entry(
            "hello",
            Some("com.1password.1password"),
            ClipboardPreCaptureProbe {
                pasteboard_has_concealed_types: false,
            },
            |_| {
                add_called.set(true);
                Ok("entry-id".to_string())
            },
        );

        assert_eq!(
            outcome,
            TextCaptureOutcome::Rejected(RejectionReason::BlockedSourceApp)
        );
        assert!(!add_called.get());
    }

    #[test]
    fn test_capture_text_entry_rejects_secret_pattern_without_add_entry() {
        let add_called = std::cell::Cell::new(false);
        let outcome = capture_text_entry(
            "token ghp_1234567890abcdefghijklmnopqrstuvwxyz",
            None,
            ClipboardPreCaptureProbe {
                pasteboard_has_concealed_types: false,
            },
            |_| {
                add_called.set(true);
                Ok("entry-id".to_string())
            },
        );

        assert_eq!(
            outcome,
            TextCaptureOutcome::Rejected(RejectionReason::SecretContentPattern)
        );
        assert!(!add_called.get());
    }

    #[test]
    fn clipboard_capture_fixture_concealed_rejection_happens_before_payload_read() {
        let mut state = ClipboardCaptureFixtureState::default();
        let payload_read = std::cell::Cell::new(false);
        let add_called = std::cell::Cell::new(false);
        let concealed = vec!["org.nspasteboard.ConcealedType".to_string()];
        let outcome = capture_clipboard_fixture_with(
            &mut state,
            1,
            None,
            &concealed,
            || {
                payload_read.set(true);
                Ok("must-not-be-read".to_string())
            },
            |_| {
                add_called.set(true);
                Ok("entry-id".to_string())
            },
            |_, _| {},
        );

        assert_eq!(
            outcome,
            ClipboardCaptureFixtureOutcome::Rejected {
                reason: RejectionReason::ConcealedPasteboardType,
            }
        );
        assert!(
            !payload_read.get(),
            "concealed capture must reject before payload read"
        );
        assert!(
            !add_called.get(),
            "concealed capture must not call add_entry"
        );
    }

    #[test]
    fn clipboard_capture_fixture_generation_allows_same_value_recopy_once_per_change() {
        let mut state = ClipboardCaptureFixtureState::default();
        let add_calls = std::cell::Cell::new(0usize);
        let sediment_calls = std::cell::Cell::new(0usize);
        let concealed = Vec::new();

        for generation in [40, 41] {
            let outcome = capture_clipboard_fixture_with(
                &mut state,
                generation,
                Some("com.apple.TextEdit"),
                &concealed,
                || Ok("same non-url value".to_string()),
                |_| {
                    add_calls.set(add_calls.get() + 1);
                    Ok("same-entry-id".to_string())
                },
                |entry_id, text| {
                    assert_eq!(entry_id, "same-entry-id");
                    assert_eq!(text, "same non-url value");
                    sediment_calls.set(sediment_calls.get() + 1);
                },
            );
            assert_eq!(
                outcome,
                ClipboardCaptureFixtureOutcome::Stored {
                    entry_id: "same-entry-id".to_string(),
                }
            );
        }

        let payload_read = std::cell::Cell::new(false);
        let unchanged = capture_clipboard_fixture_with(
            &mut state,
            41,
            None,
            &concealed,
            || {
                payload_read.set(true);
                Ok("same non-url value".to_string())
            },
            |_| panic!("unchanged generation must not add"),
            |_, _| panic!("unchanged generation must not process sediment"),
        );
        assert_eq!(
            unchanged,
            ClipboardCaptureFixtureOutcome::UnchangedGeneration
        );
        assert!(
            !payload_read.get(),
            "unchanged generation must not read payload"
        );
        assert_eq!(add_calls.get(), 2);
        assert_eq!(sediment_calls.get(), 2);
    }

    #[test]
    fn clipboard_capture_fixture_oversize_rejects_before_add_entry() {
        let mut state = ClipboardCaptureFixtureState::default();
        let add_called = std::cell::Cell::new(false);
        let max_len = get_max_text_content_len();
        let outcome = capture_clipboard_fixture_with(
            &mut state,
            7,
            None,
            &[],
            || Ok("x".repeat(max_len + 1)),
            |_| {
                add_called.set(true);
                Ok("entry-id".to_string())
            },
            |_, _| {},
        );
        assert_eq!(
            outcome,
            ClipboardCaptureFixtureOutcome::Oversized {
                text_len: max_len + 1,
                max_len,
            }
        );
        assert!(!add_called.get());
    }

    #[test]
    fn test_is_same_image_hash_returns_true_only_for_matching_hash() {
        let state = Some(LastImageState::with_blob_key(42, "blob:test".to_string()));
        assert!(is_same_image_hash(&state, 42));
        assert!(!is_same_image_hash(&state, 43));
        assert!(!is_same_image_hash(&None, 42));
    }

    #[test]
    fn test_cached_blob_key_for_hash_returns_key_only_when_available_for_same_hash() {
        let with_blob = Some(LastImageState::with_blob_key(42, "blob:test".to_string()));
        assert_eq!(cached_blob_key_for_hash(&with_blob, 42), Some("blob:test"));
        assert_eq!(cached_blob_key_for_hash(&with_blob, 43), None);

        let without_blob = Some(LastImageState::without_blob_key(42));
        assert_eq!(cached_blob_key_for_hash(&without_blob, 42), None);
    }

    #[test]
    fn test_capture_clipboard_content_returns_early_when_suppressed() {
        use crate::clipboard_history::clipboard::SUPPRESS_CLIPBOARD_CAPTURE;
        use std::sync::atomic::Ordering;

        // Set the suppression flag before calling capture
        SUPPRESS_CLIPBOARD_CAPTURE.store(true, Ordering::SeqCst);

        // Create a clipboard and tracking state
        let clipboard_result = Clipboard::new();
        if clipboard_result.is_err() {
            // CI environments may not have clipboard access; skip gracefully
            SUPPRESS_CLIPBOARD_CAPTURE.store(false, Ordering::SeqCst);
            return;
        }
        let mut clipboard = clipboard_result.expect("clipboard created above");
        let mut last_text_hash: Option<u64> = None;
        let mut last_image_state: Option<LastImageState> = None;

        // Call capture — it should return immediately without modifying state
        capture_clipboard_content(&mut clipboard, &mut last_text_hash, &mut last_image_state);

        // Verify tracking state was NOT modified (early return before any reads)
        assert_eq!(
            last_text_hash, None,
            "last_text_hash must remain None when capture is suppressed"
        );
        assert_eq!(
            last_image_state, None,
            "last_image_state must remain None when capture is suppressed"
        );

        // Clean up
        SUPPRESS_CLIPBOARD_CAPTURE.store(false, Ordering::SeqCst);
    }
}

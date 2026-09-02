//! Sequential paste state machine
//!
//! Implements Raycast-style "Paste Sequentially" — a hotkey-driven state machine
//! that snapshots clipboard history on first trigger, then advances through entries
//! one at a time on each subsequent trigger. Resets after a configurable idle timeout.
//!
//! ## Serialized Paste Worker
//!
//! The [`enqueue_sequential_paste`] function sends entry IDs to a single background
//! worker thread via a bounded channel. The worker processes one paste at a time:
//! suppress → clipboard write → delay → simulate Cmd+V → unsuppress.
//! This prevents races from rapid sequential triggers.

use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing::info;

use super::cache::get_cached_entries;
use super::clipboard::{
    write_entry_to_system_clipboard, ClipboardWriteError, SuppressGuard, SUPPRESS_CLIPBOARD_CAPTURE,
};
#[cfg(test)]
use super::types::ClipboardEntryMeta;

/// Outcome of advancing the paste-sequential state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteSequentialOutcome {
    /// Successfully advanced; contains the entry ID to paste.
    Pasted(String),
    /// The sequence ran through all snapshotted entries.
    Exhausted,
    /// Clipboard history was empty when a snapshot was attempted.
    Empty,
}

/// How long the sequence stays active after the last trigger before resetting.
const SEQUENCE_TIMEOUT: Duration = Duration::from_secs(3);

/// Maximum entries to snapshot from clipboard history.
const SNAPSHOT_LIMIT: usize = 500;

/// State for a running paste-sequential session.
#[derive(Debug)]
#[allow(dead_code)]
pub struct PasteSequentialState {
    /// Entry IDs captured at sequence start (newest-first order).
    pub snapshot_entry_ids: Vec<String>,
    /// Index of the *next* entry to paste.
    pub next_index: usize,
    /// Wall-clock time of the most recent trigger (for timeout detection).
    pub last_trigger_at: Instant,
}

impl PasteSequentialState {
    /// Create a new sequence by snapshotting current clipboard history.
    fn new_from_cache() -> Option<Self> {
        let entries = get_cached_entries(SNAPSHOT_LIMIT);
        if entries.is_empty() {
            info!(
                action = "paste_sequential",
                event = "snapshot_empty",
                "No clipboard entries to sequence"
            );
            return None;
        }
        let ids: Vec<String> = entries.into_iter().map(|e| e.id).collect();
        info!(
            action = "paste_sequential",
            event = "snapshot_created",
            count = ids.len(),
            "Snapshotted clipboard history for sequential paste"
        );
        Some(Self {
            snapshot_entry_ids: ids,
            next_index: 0,
            last_trigger_at: Instant::now(),
        })
    }

    /// Create a new sequence from a pre-built list of entries (for testing).
    #[cfg(test)]
    fn new_from_entries(entries: Vec<ClipboardEntryMeta>) -> Option<Self> {
        Self::new_from_entries_at(entries, Instant::now())
    }

    /// Create a new sequence from entries with a specific timestamp (for testing).
    #[cfg(test)]
    fn new_from_entries_at(entries: Vec<ClipboardEntryMeta>, now: Instant) -> Option<Self> {
        if entries.is_empty() {
            return None;
        }
        let ids: Vec<String> = entries.into_iter().map(|e| e.id).collect();
        Some(Self {
            snapshot_entry_ids: ids,
            next_index: 0,
            last_trigger_at: now,
        })
    }

    /// Peek at the next entry ID without advancing the index.
    ///
    /// Returns `None` when all entries are exhausted.
    fn peek(&self) -> Option<String> {
        if self.next_index >= self.snapshot_entry_ids.len() {
            info!(
                action = "paste_sequential",
                event = "exhausted",
                total = self.snapshot_entry_ids.len(),
                "Sequential paste exhausted all entries"
            );
            return None;
        }
        let id = self.snapshot_entry_ids[self.next_index].clone();
        info!(
            action = "paste_sequential",
            event = "peek",
            index = self.next_index,
            total = self.snapshot_entry_ids.len(),
            entry_id = %id,
            "Peeked at next sequential paste entry"
        );
        Some(id)
    }

    /// Commit the current entry: advance index and update trigger time.
    ///
    /// Call this only after the clipboard write succeeds.
    fn commit(&mut self) {
        if self.next_index < self.snapshot_entry_ids.len() {
            info!(
                action = "paste_sequential",
                event = "commit",
                index = self.next_index,
                total = self.snapshot_entry_ids.len(),
                "Committed sequential paste advance"
            );
            self.next_index += 1;
            self.last_trigger_at = Instant::now();
        }
    }

    /// Check if the sequence has timed out relative to `now`.
    fn is_timed_out(&self, now: Instant) -> bool {
        now.duration_since(self.last_trigger_at) > SEQUENCE_TIMEOUT
    }
}

/// Prepare the next paste candidate WITHOUT advancing the index.
///
/// - If `state` is `None` or timed out: snapshot clipboard history, peek at first entry.
/// - If `state` is active: peek at next entry.
/// - Returns [`PasteSequentialOutcome::Pasted`] with entry ID, [`Exhausted`], or [`Empty`].
///
/// Call [`commit_paste_sequence`] after a successful clipboard write to advance the index.
/// If the write fails, the index is NOT burned — retrying produces the same entry.
pub fn advance_paste_sequence(state: &mut Option<PasteSequentialState>) -> PasteSequentialOutcome {
    let now = Instant::now();

    // Check for timeout → reset
    if let Some(ref s) = state {
        if s.is_timed_out(now) {
            info!(
                action = "paste_sequential",
                event = "timeout_reset",
                elapsed_ms = now.duration_since(s.last_trigger_at).as_millis() as u64,
                "Sequence timed out, re-snapshotting"
            );
            *state = None;
        }
    }

    // Track whether we needed to create a fresh snapshot
    let was_fresh = state.is_none();

    // If no active state, create a fresh snapshot
    if was_fresh {
        *state = PasteSequentialState::new_from_cache();
    }

    // If still None after snapshot attempt, history is empty
    let Some(s) = state.as_mut() else {
        return PasteSequentialOutcome::Empty;
    };

    // Peek without advancing
    match s.peek() {
        Some(id) => PasteSequentialOutcome::Pasted(id),
        None => {
            // Exhausted — clear state so next trigger re-snapshots
            *state = None;
            PasteSequentialOutcome::Exhausted
        }
    }
}

/// Commit the current paste: advance the index after a successful clipboard write.
///
/// This is the second half of the prepare/commit protocol. Only call this when the
/// clipboard write succeeded, so that failed writes don't burn an index.
pub fn commit_paste_sequence(state: &mut Option<PasteSequentialState>) {
    if let Some(s) = state.as_mut() {
        s.commit();
    }
}

// ---------------------------------------------------------------------------
// Serialized paste worker
// ---------------------------------------------------------------------------

/// Channel sender for the single paste-worker thread.
static PASTE_WORKER_TX: OnceLock<std::sync::mpsc::SyncSender<String>> = OnceLock::new();

/// Pre-paste delay (let the window hide before simulating Cmd+V).
const PRE_PASTE_DELAY: Duration = Duration::from_millis(100);

/// Initialize the paste worker (idempotent — only the first call spawns).
fn paste_worker_sender() -> &'static std::sync::mpsc::SyncSender<String> {
    PASTE_WORKER_TX.get_or_init(|| {
        // Bounded(4): small buffer so rapid triggers queue but don't accumulate unboundedly.
        let (tx, rx) = std::sync::mpsc::sync_channel::<String>(4);

        if let Err(e) = std::thread::Builder::new()
            .name("paste-sequential-worker".into())
            .spawn(move || {
                info!(
                    action = "paste_sequential",
                    event = "worker_started",
                    "Serialized paste worker thread running"
                );
                while let Ok(entry_id) = rx.recv() {
                    info!(
                        action = "paste_sequential",
                        event = "worker_dequeue",
                        entry_id = %entry_id,
                        "Worker processing paste job"
                    );

                    // Hold suppression for the entire copy → paste span.
                    SUPPRESS_CLIPBOARD_CAPTURE.store(true, Ordering::SeqCst);
                    let _guard = SuppressGuard;

                    // Step 1: write entry to system clipboard.
                    if let Err(e) = write_entry_to_system_clipboard(&entry_id) {
                        let error_code = match &e {
                            ClipboardWriteError::EntryNotFound { .. } => "entry_not_found",
                            ClipboardWriteError::LockError { .. } => "lock_error",
                            ClipboardWriteError::ClipboardWriteFailed { .. } => {
                                "clipboard_write_failed"
                            }
                        };
                        tracing::error!(
                            action = "paste_sequential",
                            event = "worker_clipboard_error",
                            entry_id = %entry_id,
                            error_code = error_code,
                            error = %e,
                            "Failed to write entry to clipboard in paste worker"
                        );
                        // _guard drops here, clearing suppression.
                        continue;
                    }

                    // Step 2: pre-paste delay (let the window hide).
                    std::thread::sleep(PRE_PASTE_DELAY);

                    // Step 3: simulate Cmd+V.
                    if let Err(e) = crate::selected_text::simulate_paste_with_cg() {
                        tracing::error!(
                            action = "paste_sequential",
                            event = "worker_paste_error",
                            entry_id = %entry_id,
                            error = %e,
                            "Failed to simulate paste in paste worker"
                        );
                    } else {
                        info!(
                            action = "paste_sequential",
                            event = "worker_paste_success",
                            entry_id = %entry_id,
                            "Paste simulation completed"
                        );
                    }

                    // _guard drops here, clearing SUPPRESS_CLIPBOARD_CAPTURE.
                }
                info!(
                    action = "paste_sequential",
                    event = "worker_stopped",
                    "Serialized paste worker thread exiting"
                );
            })
        {
            tracing::error!(
                action = "paste_sequential",
                event = "worker_spawn_failed",
                error = %e,
                "Failed to spawn paste-sequential-worker thread"
            );
        }

        tx
    })
}

/// Error returned when enqueuing a paste job fails.
#[derive(Debug, thiserror::Error)]
pub enum EnqueuePasteError {
    #[error("{0}")]
    PolicyRefused(crate::runtime_policy::EffectRefusal),
    /// The paste worker thread has terminated unexpectedly.
    #[error("Paste worker disconnected")]
    WorkerDisconnected,
}

/// Enqueue an entry for serialized paste delivery.
///
/// The worker thread will: suppress clipboard monitor → write entry to
/// clipboard → sleep 100 ms → simulate Cmd+V → unsuppress.
/// Rapid triggers queue in order and are processed one at a time.
///
/// Returns `Ok(())` on success (including when the queue is full and we
/// block briefly). Returns [`EnqueuePasteError::WorkerDisconnected`] if the
/// worker thread is dead.
pub fn enqueue_sequential_paste(entry_id: String) -> Result<(), EnqueuePasteError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)
        .map_err(EnqueuePasteError::PolicyRefused)?;
    let tx = paste_worker_sender();
    match tx.try_send(entry_id.clone()) {
        Ok(()) => {
            info!(
                action = "paste_sequential",
                event = "enqueued",
                entry_id = %entry_id,
                "Entry enqueued for serialized paste"
            );
            Ok(())
        }
        Err(std::sync::mpsc::TrySendError::Full(id)) => {
            tracing::warn!(
                action = "paste_sequential",
                event = "queue_full",
                entry_id = %id,
                "Paste worker queue full, blocking to enqueue"
            );
            // Queue is full — the user is triggering faster than we can paste.
            // Block briefly to avoid losing the entry entirely.
            tx.send(id).map_err(|e| {
                tracing::error!(
                    action = "paste_sequential",
                    event = "enqueue_failed",
                    error_code = "worker_disconnected",
                    error = %e,
                    "Failed to enqueue entry (worker dead)"
                );
                EnqueuePasteError::WorkerDisconnected
            })
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(id)) => {
            tracing::error!(
                action = "paste_sequential",
                event = "worker_disconnected",
                error_code = "worker_disconnected",
                entry_id = %id,
                "Paste worker channel disconnected"
            );
            Err(EnqueuePasteError::WorkerDisconnected)
        }
    }
}

/// Prepare paste sequence using a pre-supplied state (test-friendly variant).
///
/// Same logic as [`advance_paste_sequence`] but accepts a custom snapshot builder
/// instead of reading from the global cache. Returns a candidate without advancing.
/// Call [`commit_paste_sequence`] after a successful write.
#[cfg(test)]
fn advance_paste_sequence_with_entries(
    state: &mut Option<PasteSequentialState>,
    entries: Vec<ClipboardEntryMeta>,
    now: Instant,
) -> PasteSequentialOutcome {
    // Check for timeout → reset
    if let Some(ref s) = state {
        if s.is_timed_out(now) {
            *state = None;
        }
    }

    // Track whether we needed a fresh snapshot
    let was_fresh = state.is_none();

    // If no active state, create from provided entries
    if was_fresh {
        *state = PasteSequentialState::new_from_entries_at(entries, now);
    }

    // If still None after snapshot attempt, history is empty
    let Some(s) = state.as_mut() else {
        return PasteSequentialOutcome::Empty;
    };

    s.last_trigger_at = now;

    // Peek without advancing (caller must commit_paste_sequence after success)
    match s.peek() {
        Some(id) => PasteSequentialOutcome::Pasted(id),
        None => {
            *state = None;
            PasteSequentialOutcome::Exhausted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard_history::types::ContentType;

    fn make_entries(ids: &[&str]) -> Vec<ClipboardEntryMeta> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| ClipboardEntryMeta {
                id: id.to_string(),
                content_type: ContentType::Text,
                timestamp: 1000 - i as i64,
                pinned: false,
                text_preview: format!("entry {id}"),
                image_width: None,
                image_height: None,
                byte_size: 5,
                ocr_text: None,
            })
            .collect()
    }

    #[test]
    fn test_fresh_snapshot_returns_first_entry() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["a", "b", "c"]);
        let now = Instant::now();

        let result = advance_paste_sequence_with_entries(&mut state, entries, now);
        assert_eq!(result, PasteSequentialOutcome::Pasted("a".to_string()));
        assert!(state.is_some());
        // Peek does not advance — index stays at 0 until commit
        assert_eq!(state.as_ref().map(|s| s.next_index), Some(0));
        commit_paste_sequence(&mut state);
        assert_eq!(state.as_ref().map(|s| s.next_index), Some(1));
    }

    #[test]
    fn test_sequential_advancement() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["a", "b", "c"]);
        let now = Instant::now();

        let r1 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r1, PasteSequentialOutcome::Pasted("a".to_string()));
        commit_paste_sequence(&mut state);

        let r2 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r2, PasteSequentialOutcome::Pasted("b".to_string()));
        commit_paste_sequence(&mut state);

        let r3 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r3, PasteSequentialOutcome::Pasted("c".to_string()));
        commit_paste_sequence(&mut state);
    }

    #[test]
    fn test_exhaustion_returns_exhausted_then_resnaps() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["x"]);
        let now = Instant::now();

        let r1 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r1, PasteSequentialOutcome::Pasted("x".to_string()));
        commit_paste_sequence(&mut state);

        // State still exists (next_index=1), peek finds exhaustion
        let r2 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r2, PasteSequentialOutcome::Exhausted);
        assert!(state.is_none());

        // Next call re-snapshots from scratch
        let r3 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r3, PasteSequentialOutcome::Pasted("x".to_string()));
    }

    #[test]
    fn test_exhaustion_clears_state() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["only"]);
        let now = Instant::now();

        // First call: snapshot + return "only"
        advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        commit_paste_sequence(&mut state);
        // State exists with next_index=1

        // Manually check exhaustion by providing empty entries for re-snapshot
        let empty: Vec<ClipboardEntryMeta> = vec![];
        let r = advance_paste_sequence_with_entries(&mut state, empty, now);
        // "only" was still in the snapshot, but next_index=1 >= len=1 → exhausted
        assert_eq!(r, PasteSequentialOutcome::Exhausted);
        assert!(state.is_none());
    }

    #[test]
    fn test_timeout_resets_sequence() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["a", "b", "c"]);
        let now = Instant::now();

        // Start sequence
        let r1 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r1, PasteSequentialOutcome::Pasted("a".to_string()));
        commit_paste_sequence(&mut state);

        // Simulate 4 seconds passing (exceeds 3s timeout)
        let later = now + Duration::from_secs(4);
        let r2 = advance_paste_sequence_with_entries(&mut state, entries.clone(), later);
        // Should have reset and re-snapshotted → returns first entry again
        assert_eq!(r2, PasteSequentialOutcome::Pasted("a".to_string()));
    }

    #[test]
    fn test_no_timeout_within_window() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["a", "b", "c"]);
        let now = Instant::now();

        let r1 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r1, PasteSequentialOutcome::Pasted("a".to_string()));
        commit_paste_sequence(&mut state);

        // 2 seconds later — still within the 3s window
        let later = now + Duration::from_secs(2);
        let r2 = advance_paste_sequence_with_entries(&mut state, entries.clone(), later);
        assert_eq!(r2, PasteSequentialOutcome::Pasted("b".to_string()));
    }

    #[test]
    fn test_empty_history_returns_empty() {
        let mut state: Option<PasteSequentialState> = None;
        let entries: Vec<ClipboardEntryMeta> = vec![];
        let now = Instant::now();

        let result = advance_paste_sequence_with_entries(&mut state, entries, now);
        assert_eq!(result, PasteSequentialOutcome::Empty);
        assert!(state.is_none());
    }

    #[test]
    fn test_state_fields_correct_after_creation() {
        let entries = make_entries(&["x", "y", "z"]);
        let state = PasteSequentialState::new_from_entries(entries);
        let s = state.expect("should create state");
        assert_eq!(s.snapshot_entry_ids, vec!["x", "y", "z"]);
        assert_eq!(s.next_index, 0);
    }

    #[test]
    fn test_failed_write_does_not_advance() {
        let mut state: Option<PasteSequentialState> = None;
        let entries = make_entries(&["a", "b", "c"]);
        let now = Instant::now();

        // Prepare returns "a"
        let r1 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r1, PasteSequentialOutcome::Pasted("a".to_string()));
        assert_eq!(state.as_ref().map(|s| s.next_index), Some(0));

        // Simulate failed write — do NOT call commit_paste_sequence

        // Retry: should still return "a" since we never committed
        let r2 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r2, PasteSequentialOutcome::Pasted("a".to_string()));
        assert_eq!(state.as_ref().map(|s| s.next_index), Some(0));

        // Now commit and verify advancement
        commit_paste_sequence(&mut state);
        assert_eq!(state.as_ref().map(|s| s.next_index), Some(1));

        // Next prepare returns "b"
        let r3 = advance_paste_sequence_with_entries(&mut state, entries.clone(), now);
        assert_eq!(r3, PasteSequentialOutcome::Pasted("b".to_string()));
    }
}

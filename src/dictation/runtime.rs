use crate::dictation::capture::{start_capture, DictationCaptureHandle};
use crate::dictation::catalog::{dictation_model_entry, DictationEngineKind, DictationModelId};
use crate::dictation::device::{list_input_devices, resolve_selected_input_device};
use crate::dictation::transcription::{
    captured_duration, merge_captured_chunks, DictationTranscriber, DictationTranscriptionConfig,
    ParakeetDictationEngine, WhisperDictationEngine,
};
use crate::dictation::types::{
    CapturedAudioChunk, CompletedDictationCapture, DictationCaptureConfig, DictationCaptureEvent,
    DictationDeviceInfo, DictationModelStatus, DictationSessionPhase, DictationTarget,
    DictationToggleOutcome,
};
use crate::dictation::visualizer::silent_bars;
use crate::dictation::window::DictationOverlayState;
use crate::dictation::DictationWrongTargetRefusalDraft;
use anyhow::{Context, Result};
use gpui::SharedString;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Session state
// ---------------------------------------------------------------------------

/// Live state of a dictation recording session.
struct DictationSession {
    capture_handle: Option<DictationCaptureHandle>,
    event_rx: async_channel::Receiver<DictationCaptureEvent>,
    chunks: Vec<CapturedAudioChunk>,
    last_bars: [f32; 9],
    started_at: Instant,
    /// The authoritative overlay phase — written by both the runtime (on start)
    /// and the overlay key handler (on Escape transitions).  The pump reads this
    /// on every tick so the overlay never drifts from shared state.
    overlay_phase: DictationSessionPhase,
    /// Target class selected for the session. Preferences persist only this
    /// class; the concrete destination identity is recaptured per session.
    target: DictationTarget,
    /// Exact destination identity frozen before the overlay can conceal or
    /// redirect focus. Retargeting replaces this atomically with the target.
    selection: Option<crate::dictation::DictationTargetSelection>,
    /// Ordered list of destinations the overlay badge cycles through.
    target_cycle: Vec<DictationTarget>,
    /// Microphone resolved when capture started. Changing preferences while
    /// recording applies to the next session, not this live AVCaptureSession.
    active_device: Option<DictationDeviceInfo>,
    /// Monotonic id used to match async partial-transcription results (and
    /// their published text) to the session they were computed for.
    session_generation: u64,
    /// Whether live partial transcripts are enabled for this session
    /// (snapshotted from config at start so the pump never reads config).
    live_preview: bool,
    /// Auto-stop ceiling snapshotted from config at start. `None` = no limit.
    max_duration: Option<Duration>,
    /// When the last partial transcription attempt started (rate limiter).
    last_partial_attempt_at: Option<Instant>,
}

/// Global singleton guarded by a parking_lot Mutex.
///
/// `None` means idle (no recording in progress).
static SESSION: Mutex<Option<DictationSession>> = Mutex::new(None);
static LAST_FROZEN_SELECTION: Mutex<Option<crate::dictation::DictationTargetSelection>> =
    Mutex::new(None);
static DICTATION_RETURN_ORIGIN: Mutex<Option<crate::dictation::DictationReturnOrigin>> =
    Mutex::new(None);

#[derive(Debug, Clone)]
#[allow(
    dead_code,
    reason = "the launcher binary preserves this work when external dictation delivery fails"
)]
pub(crate) struct DictationRecoveryWork {
    pub request: crate::dictation::DictationDeliveryRequest,
    pub audio_duration: Duration,
    pub target: crate::dictation::DictationTarget,
    pub reason: crate::dictation::DictationDeliveryFailureReason,
}

static DICTATION_RECOVERY_WORK: Mutex<Option<DictationRecoveryWork>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationStopReason {
    Hotkey,
    OverlaySubmit,
    OverlayAbort,
    GlobalEscape,
    NativeFooter,
    Synthetic,
    /// The configured `dictation.maxDurationSecs` ceiling was reached.
    MaxDurationReached,
}

#[derive(Debug, Clone)]
struct StopState {
    request_id: u64,
    reason: DictationStopReason,
    target: DictationTarget,
    selection: Option<crate::dictation::DictationTargetSelection>,
    requested_at: Instant,
}

pub enum BeginStopCapture {
    Started {
        request_id: u64,
        target: DictationTarget,
        selection: Option<crate::dictation::DictationTargetSelection>,
        session_generation: u64,
        job: Box<DictationStopJob>,
    },
    AlreadyStopping {
        request_id: u64,
    },
    NotRecording,
}

pub struct DictationStopJob {
    session: DictationSession,
}

static STOP_IN_FLIGHT: Mutex<Option<StopState>> = Mutex::new(None);
static STOP_GENERATION: AtomicU64 = AtomicU64::new(0);
static LAST_STOP_RECEIPT: Mutex<Option<serde_json::Value>> = Mutex::new(None);

/// Lazily-initialized transcriber, kept alive across sessions so the model
/// does not need to be reloaded for every dictation.  Unloaded after an idle
/// timeout via `maybe_unload_transcriber()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedTranscriberKey {
    pub model_id: DictationModelId,
    pub language: Option<String>,
}

struct CachedTranscriber {
    key: CachedTranscriberKey,
    transcriber: DictationTranscriber,
}

static TRANSCRIBER: Mutex<Option<CachedTranscriber>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Live partial transcription (recording-time preview)
// ---------------------------------------------------------------------------

/// Monotonic counter distinguishing capture sessions for partial results.
static SESSION_GENERATION: AtomicU64 = AtomicU64::new(0);

/// `true` while a partial transcription inference is running on its worker
/// thread — prevents overlapping inferences.
static PARTIAL_INFLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Latest partial transcript, tagged with the session generation it belongs
/// to so a stale result from a previous session is never displayed.
static PARTIAL_TRANSCRIPT: Mutex<Option<(u64, String)>> = Mutex::new(None);

/// Minimum spacing between partial transcription attempts.
const PARTIAL_TRANSCRIPTION_INTERVAL: Duration = Duration::from_millis(1_500);

/// Minimum audio (16 kHz mono samples) before the first partial attempt.
const PARTIAL_MIN_SAMPLES: usize = 16_000; // 1 s

/// Partial preview transcribes at most this much trailing audio so preview
/// latency stays flat on long recordings (the final pass uses everything).
const PARTIAL_WINDOW: Duration = Duration::from_secs(20);

/// Finalize-transcription progress in permille (0–1000), or [`u32::MAX`]
/// when no finalize pass is running. Written by the chunked transcription
/// worker, read by the overlay renderer's processing band.
static FINALIZE_PROGRESS_PERMILLE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(u32::MAX);

/// Fractional progress (0.0–1.0) of the in-flight finalize transcription,
/// or `None` when no finalize pass is running.
pub fn finalize_progress() -> Option<f32> {
    let permille = FINALIZE_PROGRESS_PERMILLE.load(Ordering::Acquire);
    (permille != u32::MAX).then(|| (permille.min(1000) as f32) / 1000.0)
}

fn set_finalize_progress(fraction: Option<f32>) {
    let permille = fraction
        .map(|f| (f.clamp(0.0, 1.0) * 1000.0) as u32)
        .unwrap_or(u32::MAX);
    FINALIZE_PROGRESS_PERMILLE.store(permille, Ordering::Release);
}

/// Monotonic counter for redacted delivery receipts exposed through automation.
static DELIVERY_RECEIPT_GENERATION: AtomicU64 = AtomicU64::new(0);
static DELIVERY_ID_GENERATION: AtomicU64 = AtomicU64::new(0);
static CLAIMED_DELIVERY_IDS: std::sync::LazyLock<Mutex<std::collections::HashSet<u64>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

pub fn next_dictation_delivery_id() -> u64 {
    DELIVERY_ID_GENERATION.fetch_add(1, Ordering::Relaxed) + 1
}

/// Atomically claim a delivery before validation or mutation.
///
/// A delivery id gets one coordinator pass. Keeping failed and uncertain ids
/// claimed is intentional: retrying the same request could duplicate a mutation
/// whose result was not observable.
pub fn claim_dictation_delivery(delivery_id: u64) -> bool {
    CLAIMED_DELIVERY_IDS.lock().insert(delivery_id)
}

pub fn dictation_delivery_was_claimed(delivery_id: u64) -> bool {
    CLAIMED_DELIVERY_IDS.lock().contains(&delivery_id)
}

/// Monotonic generation for passive dictation runtime state exposed to DevTools.
static DICTATION_STATE_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Monotonic generation incremented only by explicit destination selection.
static DICTATION_TARGET_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Last dictation delivery receipt. This stores only routing metadata, length,
/// and a one-way fingerprint; it never stores transcript text.
static LAST_DELIVERY_RECEIPT: Mutex<Option<serde_json::Value>> = Mutex::new(None);

/// Monotonic counter for wrong-target refusal receipts.
static WRONG_TARGET_REFUSAL_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Last fail-closed dictation delivery refusal. Stores only redacted routing
/// metadata and never stores transcript text or raw requested labels.
static LAST_WRONG_TARGET_REFUSAL: Mutex<Option<serde_json::Value>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationTranscriptResolution {
    pub transcript: Option<String>,
    pub used_partial_fallback: bool,
    pub final_len: usize,
    pub partial_len: Option<usize>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` when a dictation capture session is currently active.
pub fn is_dictation_recording() -> bool {
    SESSION.lock().is_some()
}

pub fn is_dictation_stopping() -> bool {
    STOP_IN_FLIGHT.lock().is_some()
}

/// Delivery target owned by the stop coordinator while the live session has
/// already been moved out of [`SESSION`]. Rapid toggle handling uses this to
/// restart against the same surface instead of re-resolving against a window
/// topology that may be temporarily concealed during the stop transition.
pub fn dictation_stop_target() -> Option<DictationTarget> {
    STOP_IN_FLIGHT.lock().as_ref().map(|state| state.target)
}

/// Toggle parity for shortcut requests received while capture teardown owns
/// the session. `false -> true` queues a restart after stop; the next request
/// flips it back to `false`. Keeping this pure makes the last-intent rule
/// independently testable without a microphone.
#[allow(
    dead_code,
    reason = "the launcher binary owns post-stop dictation shortcut replay"
)]
pub(crate) fn toggled_post_stop_restart(pending: bool) -> bool {
    !pending
}

pub fn is_dictation_busy() -> bool {
    is_dictation_recording() || is_dictation_stopping()
}

/// Returns the elapsed duration of the active dictation session, or `None`
/// when no session is active.  This reads `started_at` directly from the
/// live session so the caller gets an authoritative wall-clock elapsed time
/// rather than a pump-tick-stale snapshot.
pub fn dictation_elapsed() -> Option<std::time::Duration> {
    SESSION.lock().as_ref().map(|s| s.started_at.elapsed())
}

/// Update the overlay phase in the live session.
///
/// Called by the overlay key handler so the pump tick reads the correct phase
/// instead of overwriting it.  Returns `false` when no session is active.
pub fn set_overlay_phase(phase: DictationSessionPhase) -> bool {
    let mut guard = SESSION.lock();
    let Some(session) = guard.as_mut() else {
        return false;
    };
    session.overlay_phase = phase;
    bump_dictation_state_generation();
    true
}

/// Return the active dictation session's overlay phase, or `None` when no
/// session is recording. Used by automation state receipts to surface the
/// dictation lifecycle (`recording` → `confirming` → `transcribing` …) through
/// `getAgentChatState.dictationPhase`.
pub fn current_dictation_phase() -> Option<DictationSessionPhase> {
    SESSION.lock().as_ref().map(|s| s.overlay_phase.clone())
}

#[allow(
    dead_code,
    reason = "the launcher binary snapshots focus before opening the dictation overlay"
)]
pub(crate) fn replace_dictation_return_origin(origin: crate::dictation::DictationReturnOrigin) {
    *DICTATION_RETURN_ORIGIN.lock() = Some(origin);
    bump_dictation_state_generation();
}

#[allow(
    dead_code,
    reason = "the launcher binary restores the frozen dictation destination"
)]
pub(crate) fn dictation_return_origin() -> Option<crate::dictation::DictationReturnOrigin> {
    DICTATION_RETURN_ORIGIN.lock().clone()
}

#[allow(
    dead_code,
    reason = "the launcher binary clears dictation handoff state after delivery"
)]
pub(crate) fn clear_dictation_return_origin() {
    DICTATION_RETURN_ORIGIN.lock().take();
    bump_dictation_state_generation();
}

#[allow(
    dead_code,
    reason = "the launcher binary converts failed dictation delivery into recoverable UI state"
)]
pub(crate) fn dictation_pipeline_failure_state(
    operation_id: u64,
    target: crate::dictation::DictationTarget,
    detail: &str,
) -> crate::dictation::DictationFailureState {
    let failure = crate::ai::reliability::context_unavailable_failure(detail);
    crate::dictation::DictationFailureState {
        operation_id,
        destination_id: crate::dictation::redacted_transcript_fingerprint(target.sticky_label()),
        destination_label: target.overlay_label().to_string(),
        identity_generation: 0,
        transcript_id: String::new(),
        history_entry_id: String::new(),
        failure,
        retry_safety: sk_protocol::ai_reliability::RetrySafety::Never,
        preservation_receipt: crate::dictation::DictationTranscriptPreservationReceipt {
            transcript_id: String::new(),
            transcript_len: 0,
            transcript_fingerprint: crate::dictation::redacted_transcript_fingerprint(""),
            history_entry_id: String::new(),
            history_saved: false,
        },
        capabilities: crate::dictation::DictationFailureRecoveryCapabilities::default(),
    }
}

#[allow(
    dead_code,
    reason = "the launcher binary preserves dictation work instead of dropping failed transcripts"
)]
pub(crate) fn preserve_dictation_recovery_work(
    work: DictationRecoveryWork,
    failure: crate::ai::reliability::AppFailureRecord,
    retry_safety: sk_protocol::ai_reliability::RetrySafety,
    recovery_callback_installed: bool,
) -> crate::dictation::DictationFailureState {
    let reason = work.reason;
    let request = &work.request;
    let history_saved = !request.history_entry_id.is_empty();
    let capabilities = crate::dictation::DictationFailureRecoveryCapabilities {
        retry_same_destination: recovery_callback_installed
            && retry_safety != sk_protocol::ai_reliability::RetrySafety::Never
            && !matches!(
                reason,
                crate::dictation::DictationDeliveryFailureReason::DestinationStale
                    | crate::dictation::DictationDeliveryFailureReason::DestinationUnavailable
                    | crate::dictation::DictationDeliveryFailureReason::MutationOutcomeUnknown
            ),
        choose_destination: recovery_callback_installed,
        copy_transcript: recovery_callback_installed && arboard::Clipboard::new().is_ok(),
        open_dictation_history: recovery_callback_installed && history_saved,
    };
    let state = crate::dictation::DictationFailureState {
        operation_id: request.delivery_id,
        destination_id: request.selection.destination.identity_fingerprint(),
        destination_label: request.selection.display_label.clone(),
        identity_generation: request.selection.selection_generation,
        transcript_id: request.transcript.id().to_string(),
        history_entry_id: request.history_entry_id.clone(),
        failure,
        retry_safety,
        preservation_receipt: crate::dictation::DictationTranscriptPreservationReceipt {
            transcript_id: request.transcript.id().to_string(),
            transcript_len: request.transcript.len(),
            transcript_fingerprint: request.transcript.fingerprint().to_string(),
            history_entry_id: request.history_entry_id.clone(),
            history_saved,
        },
        capabilities,
    };
    *DICTATION_RECOVERY_WORK.lock() = Some(work);
    bump_dictation_state_generation();
    state
}

#[allow(
    dead_code,
    reason = "the launcher binary reads preserved transcript work when retrying delivery"
)]
pub(crate) fn dictation_recovery_work() -> Option<DictationRecoveryWork> {
    DICTATION_RECOVERY_WORK.lock().clone()
}

#[allow(
    dead_code,
    reason = "the launcher binary updates preserved transcript work during retry"
)]
pub(crate) fn replace_dictation_recovery_work(work: DictationRecoveryWork) {
    *DICTATION_RECOVERY_WORK.lock() = Some(work);
    bump_dictation_state_generation();
}

#[allow(
    dead_code,
    reason = "the launcher binary clears preserved transcript work after a terminal outcome"
)]
pub(crate) fn clear_dictation_recovery_work() {
    DICTATION_RECOVERY_WORK.lock().take();
    bump_dictation_state_generation();
}

/// Toggle dictation recording on/off.
///
/// - When idle: starts a new capture session targeting `target` and
///   returns `Started`.
/// - When recording: drops the capture handle (flushing the tail chunk),
///   drains events until `EndOfStream`, and returns `Stopped(Some(capture))`
///   or `Stopped(None)` for an empty recording.
///
/// This is the single entrypoint shared by both `BuiltInFeature::Dictation`
/// and `HotkeyAction::Dictation`.  The caller owns transcription, overlay
/// transitions, and delivery — the runtime only captures audio.
pub fn toggle_dictation(target: DictationTarget) -> Result<DictationToggleOutcome> {
    tracing::info!(category = "DICTATION", ?target, "toggle_dictation called");

    if SESSION.lock().is_some() {
        return Ok(DictationToggleOutcome::Stopped(stop_recording()?));
    }

    start_recording(target)?;
    Ok(DictationToggleOutcome::Started)
}

pub fn begin_stop_capture(reason: DictationStopReason) -> Result<BeginStopCapture> {
    if let Some(existing) = STOP_IN_FLIGHT.lock().as_ref().cloned() {
        tracing::info!(
            category = "DICTATION",
            stop_request_id = existing.request_id,
            ?reason,
            existing_reason = ?existing.reason,
            "Dictation stop already in flight"
        );
        return Ok(BeginStopCapture::AlreadyStopping {
            request_id: existing.request_id,
        });
    }

    let session = SESSION.lock().take();
    let Some(session) = session else {
        return Ok(BeginStopCapture::NotRecording);
    };

    let request_id = STOP_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let target = session.target;
    let selection = session.selection.clone();
    let session_generation = session.session_generation;
    let target_label = selection
        .as_ref()
        .map(|selection| selection.display_label.as_str())
        .unwrap_or_else(|| target.overlay_label());
    let requested_at = Instant::now();
    *STOP_IN_FLIGHT.lock() = Some(StopState {
        request_id,
        reason,
        target,
        selection: selection.clone(),
        requested_at,
    });
    *LAST_STOP_RECEIPT.lock() = Some(serde_json::json!({
        "inFlight": true,
        "requestId": request_id,
        "reason": format!("{:?}", reason),
        "target": format!("{:?}", target),
        "targetLabel": target_label,
        "collectDurationMs": null,
        "timedOut": false,
        "chunkCount": null,
        "audioDurationMs": null,
        "source": "runtime.dictation.stopCoordinator",
        "redacted": true,
    }));
    bump_dictation_state_generation();

    tracing::info!(
        category = "DICTATION",
        event = "dictation_stop_requested",
        stop_request_id = request_id,
        ?reason,
        ?target,
        target_label,
        "Dictation stop handed to coordinator"
    );

    Ok(BeginStopCapture::Started {
        request_id,
        target,
        selection,
        session_generation,
        job: Box::new(DictationStopJob { session }),
    })
}

pub fn finish_stop_capture(
    request_id: u64,
    result: &Result<Option<CompletedDictationCapture>>,
) -> bool {
    let stop_state = {
        let mut guard = STOP_IN_FLIGHT.lock();
        match guard.as_ref() {
            Some(state) if state.request_id == request_id => guard.take(),
            Some(state) => {
                tracing::warn!(
                    category = "DICTATION",
                    stop_request_id = request_id,
                    active_stop_request_id = state.request_id,
                    "Ignoring stale dictation stop completion"
                );
                None
            }
            None => None,
        }
    };

    let Some(stop_state) = stop_state else {
        return false;
    };

    let collect_duration = stop_state.requested_at.elapsed();
    let (chunk_count, audio_duration_ms, truncated, error) = match result {
        Ok(Some(capture)) => (
            Some(capture.chunks.len()),
            Some(capture.audio_duration.as_millis() as u64),
            capture.truncated,
            None,
        ),
        Ok(None) => (Some(0), Some(0), false, None),
        Err(error) => (None, None, false, Some(error.to_string())),
    };
    *LAST_STOP_RECEIPT.lock() = Some(serde_json::json!({
        "inFlight": false,
        "requestId": request_id,
        "reason": format!("{:?}", stop_state.reason),
        "target": format!("{:?}", stop_state.target),
        "targetLabel": stop_state.target.overlay_label(),
        "collectDurationMs": collect_duration.as_millis() as u64,
        "timedOut": error.as_ref().is_some_and(|message| message.contains("timed out")),
        "truncated": truncated,
        "chunkCount": chunk_count,
        "audioDurationMs": audio_duration_ms,
        "error": error,
        "source": "runtime.dictation.stopCoordinator",
        "redacted": true,
    }));
    bump_dictation_state_generation();

    tracing::info!(
        category = "DICTATION",
        event = "dictation_stop_completed",
        stop_request_id = request_id,
        reason = ?stop_state.reason,
        target = ?stop_state.target,
        collect_ms = collect_duration.as_millis() as u64,
        chunk_count = ?chunk_count,
        audio_duration_ms = ?audio_duration_ms,
        "Dictation stop coordinator completed"
    );

    true
}

pub fn last_stop_receipt() -> Option<serde_json::Value> {
    LAST_STOP_RECEIPT.lock().clone()
}

/// Return the delivery target that was captured when the current dictation
/// session started.  Returns `None` when no session is active.
pub fn get_dictation_target() -> Option<DictationTarget> {
    SESSION.lock().as_ref().map(|s| s.target)
}

pub fn get_dictation_target_selection() -> Option<crate::dictation::DictationTargetSelection> {
    SESSION
        .lock()
        .as_ref()
        .and_then(|session| session.selection.clone())
        .or_else(|| {
            STOP_IN_FLIGHT
                .lock()
                .as_ref()
                .and_then(|state| state.selection.clone())
        })
        .or_else(|| LAST_FROZEN_SELECTION.lock().clone())
}

pub(crate) fn retain_frozen_selection_for_delivery(
    selection: crate::dictation::DictationTargetSelection,
) {
    *LAST_FROZEN_SELECTION.lock() = Some(selection);
    bump_dictation_state_generation();
}

pub fn set_dictation_session_selection(
    selection: crate::dictation::DictationTargetSelection,
) -> Option<crate::dictation::DictationTargetSelection> {
    let mut guard = SESSION.lock();
    let session = guard.as_mut()?;
    if !session.target_cycle.contains(&selection.target) {
        session.target_cycle.push(selection.target);
    }
    session.target = selection.target;
    session.selection = Some(selection.clone());
    *LAST_FROZEN_SELECTION.lock() = Some(selection.clone());
    DICTATION_TARGET_GENERATION.fetch_add(1, Ordering::Relaxed);
    bump_dictation_state_generation();
    Some(selection)
}

/// Return the microphone that the active capture session opened with.
pub fn get_active_dictation_device() -> Option<DictationDeviceInfo> {
    SESSION
        .lock()
        .as_ref()
        .and_then(|session| session.active_device.clone())
}

/// Display label of a microphone selected while a recording session is live.
///
/// The live AVCaptureSession keeps the mic it opened with, so a preference
/// change only applies to the next session — the overlay footer shows this
/// label as "<name> (next)" so the switch is not silently deferred.
static PENDING_DEVICE_LABEL: Mutex<Option<String>> = Mutex::new(None);

pub fn set_pending_dictation_device_label(label: Option<String>) {
    *PENDING_DEVICE_LABEL.lock() = label;
    bump_dictation_state_generation();
}

pub fn pending_dictation_device_label() -> Option<String> {
    PENDING_DEVICE_LABEL.lock().clone()
}

/// Record a content-safe receipt for the most recent dictation delivery.
///
/// Agent-facing DevTools use this to prove target routing and delivery
/// generation without reading transcript contents from logs or UI text.
#[expect(
    clippy::too_many_arguments,
    reason = "the redacted delivery receipt must preserve all independently verified routing facts"
)]
pub fn record_delivery_receipt(
    delivery_id: u64,
    session_generation: u64,
    selection: &crate::dictation::DictationTargetSelection,
    transcript: &str,
    audio_duration: std::time::Duration,
    target: DictationTarget,
    destination: crate::dictation::DictationDestination,
    delivered_internally: bool,
    history_entry_id: &str,
    insertion_range: Option<serde_json::Value>,
) -> serde_json::Value {
    let generation = DELIVERY_RECEIPT_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let receipt = serde_json::json!({
        "generation": generation,
        "deliveryId": delivery_id,
        "sessionGeneration": session_generation,
        "selectionGeneration": selection.selection_generation,
        "frozenIdentityKind": selection.destination.kind(),
        "frozenIdentityFingerprint": selection.destination.identity_fingerprint(),
        "destinationAttemptCount": 1,
        "mutationCount": 1,
        "target": format!("{:?}", target),
        "targetLabel": selection.display_label,
        "destination": format!("{:?}", destination),
        "deliveredInternally": delivered_internally,
        "historyEntryId": history_entry_id,
        "transcriptLen": transcript.len(),
        "transcriptFingerprint": redacted_transcript_fingerprint(transcript),
        "audioDurationMs": audio_duration.as_millis() as u64,
        "insertionRange": insertion_range.unwrap_or_else(|| serde_json::json!({
            "available": false,
            "reason": "destination did not expose an insertion range receipt",
        })),
        "source": "deliveryPipeline",
        "redacted": true,
    });
    *LAST_DELIVERY_RECEIPT.lock() = Some(receipt.clone());
    bump_dictation_state_generation();
    receipt
}

pub fn delivery_receipt_generation() -> u64 {
    DELIVERY_RECEIPT_GENERATION.load(Ordering::Relaxed)
}

pub fn dictation_target_generation() -> u64 {
    DICTATION_TARGET_GENERATION.load(Ordering::Relaxed)
}

pub(crate) fn record_fixture_dictation_target_selection() -> u64 {
    debug_assert!(crate::dictation::dictation_overlay_fixture_mode());
    let generation = DICTATION_TARGET_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    bump_dictation_state_generation();
    generation
}

pub fn record_wrong_target_refusal(
    draft: DictationWrongTargetRefusalDraft,
    transcript_len: Option<usize>,
) -> serde_json::Value {
    let generation = WRONG_TARGET_REFUSAL_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let requested_target_label_len = draft
        .requested_target_label
        .as_ref()
        .map(|label| label.chars().count());
    let requested_target_label_fingerprint = draft
        .requested_target_label
        .as_ref()
        .map(|label| redacted_transcript_fingerprint(label));
    let delivery_generation_after = delivery_receipt_generation();

    let receipt = serde_json::json!({
        "generation": generation,
        "reasonCode": draft.reason.as_code(),
        "requestedTarget": draft.requested_target.map(|target| format!("{:?}", target)),
        "requestedTargetLabelLen": requested_target_label_len,
        "requestedTargetLabelFingerprint": requested_target_label_fingerprint,
        "deliveryGenerationBefore": draft.delivery_generation_before,
        "deliveryGenerationAfter": delivery_generation_after,
        "noDeliveryAttempted": true,
        "transcriptLen": transcript_len,
        "source": "deliveryActor",
        "redacted": true,
    });
    *LAST_WRONG_TARGET_REFUSAL.lock() = Some(receipt.clone());
    bump_dictation_state_generation();
    receipt
}

/// Resolve final transcription text with a partial fallback for stop/finalize
/// races where the provider returns an empty final result.
pub fn resolve_final_or_partial_transcript(
    final_transcript: &str,
    partial_transcript: Option<&str>,
) -> DictationTranscriptResolution {
    if !final_transcript.trim().is_empty() {
        return DictationTranscriptResolution {
            transcript: Some(final_transcript.to_string()),
            used_partial_fallback: false,
            final_len: final_transcript.len(),
            partial_len: partial_transcript.map(str::len),
        };
    }

    if let Some(partial) = partial_transcript.filter(|value| !value.trim().is_empty()) {
        return DictationTranscriptResolution {
            transcript: Some(partial.to_string()),
            used_partial_fallback: true,
            final_len: final_transcript.len(),
            partial_len: Some(partial.len()),
        };
    }

    DictationTranscriptResolution {
        transcript: None,
        used_partial_fallback: false,
        final_len: final_transcript.len(),
        partial_len: partial_transcript.map(str::len),
    }
}

/// Return the latest content-safe dictation delivery receipt.
pub fn last_delivery_receipt() -> Option<serde_json::Value> {
    LAST_DELIVERY_RECEIPT.lock().clone()
}

pub fn last_wrong_target_refusal() -> Option<serde_json::Value> {
    LAST_WRONG_TARGET_REFUSAL.lock().clone()
}

/// Process-keyed HMAC fingerprint for correlating private transcript receipts.
/// Public FNV/SHA digests let observers guess spoken words one prefix at a
/// time; the ephemeral shared privacy key preserves same-process equality
/// without exposing an offline guessing oracle.
pub fn redacted_transcript_fingerprint(transcript: &str) -> String {
    format!(
        "sha256:{}",
        crate::logging::log_private_user_value(transcript).sha256
    )
}

/// Replace the active session's destination cycle. The current target is
/// preserved and inserted if omitted so overlay state and delivery stay aligned.
pub fn set_dictation_target_cycle(targets: Vec<DictationTarget>) -> bool {
    let mut guard = SESSION.lock();
    let Some(session) = guard.as_mut() else {
        return false;
    };

    let mut cycle = vec![session.target];
    for target in targets {
        if !cycle.contains(&target) {
            cycle.push(target);
        }
    }

    session.target_cycle = cycle;
    bump_dictation_state_generation();
    true
}

/// Returns true when the active session has more than one destination in its
/// cycle, meaning the overlay badge should look interactive.
pub fn can_cycle_dictation_target() -> bool {
    SESSION
        .lock()
        .as_ref()
        .map(|session| session.target_cycle.len() > 1)
        .unwrap_or(false)
}

/// Explicitly set the active session's delivery target (overlay verb chips).
///
/// Unlike [`cycle_dictation_target`], this is a direct selection: the target
/// is inserted into the session cycle if absent so subsequent cycle calls and
/// overlay state stay coherent. Returns the applied target, or `None` when no
/// session is active.
pub fn set_dictation_session_target(target: DictationTarget) -> Option<DictationTarget> {
    let mut guard = SESSION.lock();
    let session = guard.as_mut()?;

    if !session.target_cycle.contains(&target) {
        session.target_cycle.push(target);
    }
    session.target = target;
    session.selection = None;
    *LAST_FROZEN_SELECTION.lock() = None;
    DICTATION_TARGET_GENERATION.fetch_add(1, Ordering::Relaxed);
    bump_dictation_state_generation();

    tracing::info!(
        category = "DICTATION",
        ?target,
        target_label = target.overlay_label(),
        "Dictation session target set explicitly"
    );

    Some(target)
}

/// Advance the active session to the next configured destination.
pub fn cycle_dictation_target() -> Option<DictationTarget> {
    let mut guard = SESSION.lock();
    let session = guard.as_mut()?;

    if session.target_cycle.len() < 2 {
        return Some(session.target);
    }

    let current_ix = session
        .target_cycle
        .iter()
        .position(|target| *target == session.target)
        .unwrap_or(0);
    let next_ix = (current_ix + 1) % session.target_cycle.len();
    let next_target = session.target_cycle[next_ix];
    session.target = next_target;
    session.selection = None;
    *LAST_FROZEN_SELECTION.lock() = None;
    bump_dictation_state_generation();

    tracing::info!(
        category = "DICTATION",
        ?next_target,
        target_label = next_target.overlay_label(),
        "Dictation target cycled"
    );

    Some(next_target)
}

/// Abort the active dictation session without transcribing or delivering text.
///
/// This is used by the overlay's Escape confirmation flow, where the user has
/// explicitly chosen to discard the current recording.
pub fn abort_dictation() -> Result<()> {
    if SESSION.lock().is_none() {
        return Ok(());
    }

    let _ = stop_recording()?;
    tracing::info!(category = "DICTATION", "Recording aborted");
    Ok(())
}

/// Snapshot the current overlay visual state from the live session.
///
/// Drains pending events (non-blocking) so the level/chunk data stays
/// current.  Returns `None` when no recording is active.
pub fn snapshot_overlay_state() -> Option<DictationOverlayState> {
    let mut guard = SESSION.lock();
    let session = guard.as_mut()?;

    // Non-blocking drain of queued events.
    while let Ok(event) = session.event_rx.try_recv() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => session.chunks.push(chunk),
            DictationCaptureEvent::Bars(bars) => session.last_bars = bars,
            DictationCaptureEvent::EndOfStream => {}
        }
    }

    maybe_begin_partial_transcription(session);

    let transcript = PARTIAL_TRANSCRIPT
        .lock()
        .as_ref()
        .filter(|(generation, _)| *generation == session.session_generation)
        .map(|(_, text)| SharedString::from(text.clone()))
        .unwrap_or_default();

    Some(DictationOverlayState {
        phase: session.overlay_phase.clone(),
        elapsed: session.started_at.elapsed(),
        bars: session.last_bars,
        transcript,
        target: session.target,
    })
}

/// Latest live partial transcript, if any.
///
/// Survives session teardown (it is only replaced by the next session's
/// partials) so the stop/transcribe path can keep the recognized-so-far text
/// on screen while the final transcription runs, instead of blanking the
/// overlay.
pub fn last_partial_transcript() -> Option<String> {
    PARTIAL_TRANSCRIPT
        .lock()
        .as_ref()
        .map(|(_, text)| text.clone())
        .filter(|text| !text.trim().is_empty())
}

/// Returns `true` when the active recording has outlived its configured
/// max-duration ceiling and should be auto-stopped by the caller.
pub fn dictation_auto_stop_due() -> bool {
    let guard = SESSION.lock();
    let Some(session) = guard.as_ref() else {
        return false;
    };
    matches!(
        session.overlay_phase,
        DictationSessionPhase::Recording | DictationSessionPhase::Confirming
    ) && session
        .max_duration
        .is_some_and(|limit| session.started_at.elapsed() >= limit)
}

/// Kick off a background partial transcription of the trailing audio window
/// when live preview is enabled and no attempt is already running.
///
/// Runs on the overlay pump tick while holding the session lock, so it only
/// copies samples out and spawns a worker thread — the inference itself never
/// blocks the lock.
fn maybe_begin_partial_transcription(session: &mut DictationSession) {
    if !session.live_preview
        || session.overlay_phase != DictationSessionPhase::Recording
        || PARTIAL_INFLIGHT.load(Ordering::Acquire)
    {
        return;
    }
    if session
        .last_partial_attempt_at
        .is_some_and(|at| at.elapsed() < PARTIAL_TRANSCRIPTION_INTERVAL)
    {
        return;
    }

    // Trailing window: walk chunks back-to-front until the window is full.
    let mut window_duration = Duration::ZERO;
    let mut start_ix = session.chunks.len();
    while start_ix > 0 && window_duration < PARTIAL_WINDOW {
        start_ix -= 1;
        window_duration += session.chunks[start_ix].duration;
    }
    let samples = merge_captured_chunks(&session.chunks[start_ix..]);
    if samples.len() < PARTIAL_MIN_SAMPLES {
        return;
    }

    let prefs = crate::config::load_user_preferences();
    let model_id = DictationModelId::from_preference(prefs.dictation.model.as_deref());
    if !dictation_model_entry(model_id).is_available() {
        // No model on disk — a partial attempt would only produce log noise.
        session.last_partial_attempt_at = Some(Instant::now());
        return;
    }

    session.last_partial_attempt_at = Some(Instant::now());
    let generation = session.session_generation;
    let truncated_front = start_ix > 0;
    PARTIAL_INFLIGHT.store(true, Ordering::Release);

    std::thread::Builder::new()
        .name("dictation-partial".into())
        .spawn(move || {
            let result = transcribe_samples_with_cached_engine(&samples);
            match result {
                Ok(Some(text)) => {
                    let display = if truncated_front {
                        format!("\u{2026}{text}")
                    } else {
                        text
                    };
                    let mut guard = PARTIAL_TRANSCRIPT.lock();
                    // Only publish for the session the audio came from.
                    if SESSION_GENERATION.load(Ordering::Acquire) == generation {
                        *guard = Some((generation, display));
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::debug!(
                        category = "DICTATION",
                        error = %error,
                        "Partial transcription attempt failed"
                    );
                }
            }
            PARTIAL_INFLIGHT.store(false, Ordering::Release);
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            PARTIAL_INFLIGHT.store(false, Ordering::Release);
            tracing::warn!(
                category = "DICTATION",
                error = %error,
                "Failed to spawn partial transcription thread"
            );
        });
}

/// Passive, redacted automation snapshot for Script Kit DevTools.
///
/// This is safe for `getState`: it does not start capture, open System
/// Settings, request TCC permission, transcribe audio, or expose transcript
/// contents or raw microphone device ids.
pub fn automation_state() -> serde_json::Value {
    let (is_recording, phase, target, target_label, elapsed_ms, audio_levels) = {
        let guard = SESSION.lock();
        if let Some(session) = guard.as_ref() {
            (
                true,
                session.overlay_phase.as_automation_str().to_string(),
                Some(format!("{:?}", session.target)),
                Some(
                    session
                        .selection
                        .as_ref()
                        .map(|selection| selection.display_label.clone())
                        .unwrap_or_else(|| session.target.overlay_label().to_string()),
                ),
                Some(session.started_at.elapsed().as_millis() as u64),
                serde_json::json!({
                    "available": true,
                    "bars": session.last_bars,
                    "barCount": session.last_bars.len(),
                    "source": "runtime.session.lastBars",
                    "stopReason": null,
                }),
            )
        } else if let Some(stop_state) = STOP_IN_FLIGHT.lock().as_ref().cloned() {
            (
                false,
                "stopping".to_string(),
                Some(format!("{:?}", stop_state.target)),
                Some(
                    stop_state
                        .selection
                        .as_ref()
                        .map(|selection| selection.display_label.clone())
                        .unwrap_or_else(|| stop_state.target.overlay_label().to_string()),
                ),
                Some(stop_state.requested_at.elapsed().as_millis() as u64),
                serde_json::json!({
                    "available": false,
                    "bars": [],
                    "barCount": 0,
                    "source": "runtime.session.lastBars",
                    "stopReason": "capture stop in flight",
                }),
            )
        } else if crate::dictation::dictation_overlay_fixture_mode() {
            let state = crate::dictation::last_dictation_overlay_state();
            (
                false,
                state.phase.as_automation_str().to_string(),
                Some(format!("{:?}", state.target)),
                Some(state.target.overlay_label().to_string()),
                Some(state.elapsed.as_millis() as u64),
                serde_json::json!({
                    "available": true,
                    "bars": state.bars,
                    "barCount": state.bars.len(),
                    "source": "fixture.lastOverlayState",
                    "stopReason": null,
                }),
            )
        } else {
            (
                false,
                "idle".to_string(),
                None,
                None,
                None,
                serde_json::json!({
                    "available": false,
                    "bars": [],
                    "barCount": 0,
                    "source": "runtime.session.lastBars",
                    "stopReason": "not recording",
                }),
            )
        }
    };

    let config = crate::config::load_config();
    let prefs = crate::config::load_user_preferences();
    let selected_device_id = prefs.dictation.selected_device_id.as_deref();
    let selected_model_id = DictationModelId::from_preference(prefs.dictation.model.as_deref());
    let selected_model = dictation_model_entry(selected_model_id);
    let permission = crate::dictation::microphone_permission_status();
    let devices_result = if matches!(
        permission,
        crate::dictation::DictationMicrophonePermissionStatus::Granted
            | crate::dictation::DictationMicrophonePermissionStatus::Unknown
    ) {
        crate::dictation::list_input_devices().map_err(|error| error.to_string())
    } else {
        Ok(Vec::new())
    };
    let device_snapshot = microphone_device_snapshot(&devices_result, selected_device_id);
    let setup_state = crate::dictation::build_dictation_setup_state(
        dictation_model_status(),
        permission,
        devices_result,
        selected_device_id,
        config.get_dictation_hotkey().as_ref(),
        config.is_dictation_hotkey_enabled(),
    );
    let hotkey = config.get_dictation_hotkey();
    let generation = DICTATION_STATE_GENERATION.load(Ordering::Relaxed);
    let target_actions = crate::dictation::DictationTarget::action_descriptors()
        .map(|descriptor| {
            serde_json::json!({
                "stableId": descriptor.stable_id,
                "selectorLabel": descriptor.selector_label,
                "badgeLabel": descriptor.badge_label,
                "icon": descriptor.icon,
                "deliveryVerb": descriptor.delivery_verb,
                "description": descriptor.description,
                "requiresFrozenIdentity": descriptor.requires_frozen_identity,
                "autoSubmitPermission": format!("{:?}", descriptor.auto_submit_permission),
                "selectable": descriptor.selectable,
            })
        })
        .collect::<Vec<_>>();
    let quick_target_ids = crate::dictation::DictationTarget::quick_chip_descriptors()
        .map(|descriptor| descriptor.stable_id)
        .collect::<Vec<_>>();
    let frozen_selection = crate::dictation::get_dictation_target_selection().map(|selection| {
        serde_json::json!({
            "target": format!("{:?}", selection.target),
            "kind": selection.destination.kind(),
            "identityFingerprint": selection.destination.identity_fingerprint(),
            "selectionGeneration": selection.selection_generation,
            "displayLabel": selection.display_label,
            "iconIdentityFingerprint": selection.icon_identity.as_deref().map(automation_fingerprint),
            "source": "runtime.session.frozenSelection",
            "redacted": true,
        })
    });
    let recovery = match crate::dictation::last_dictation_overlay_state().phase {
        DictationSessionPhase::Failed(state) => Some(serde_json::json!({
            "operationId": state.operation_id,
            "destinationId": state.destination_id,
            "destinationLabel": state.destination_label,
            "identityGeneration": state.identity_generation,
            "transcriptId": state.transcript_id,
            "historyEntryId": state.history_entry_id,
            "failureCode": format!("{:?}", state.failure.failure.code),
            "failureCategory": format!("{:?}", state.failure.failure.kind),
            "safeSummary": state.failure.primary_message(),
            "diagnosticFingerprint": state.failure.failure.diagnostic.as_ref().map(|value| value.fingerprint.0.clone()),
            "retrySafety": format!("{:?}", state.retry_safety),
            "preservation": {
                "transcriptId": state.preservation_receipt.transcript_id,
                "transcriptLength": state.preservation_receipt.transcript_len,
                "transcriptFingerprint": state.preservation_receipt.transcript_fingerprint,
                "historyEntryId": state.preservation_receipt.history_entry_id,
                "historySaved": state.preservation_receipt.history_saved,
            },
            "actions": state.capabilities.actions().map(|action| format!("{:?}", action)).collect::<Vec<_>>(),
            "messageOnlyCard": true,
            "source": "runtime.dictation.recoveryState",
            "redacted": true,
        })),
        _ => None,
    };

    serde_json::json!({
        "schemaVersion": 1,
        "source": "runtime.dictation.automationState",
        "passive": true,
        "redacted": true,
        "generation": generation,
        "recordingStateGeneration": generation,
        "isRecording": is_recording,
        "phase": phase,
        "target": target,
        "targetLabel": target_label,
        "targetGeneration": dictation_target_generation(),
        "frozenSelection": frozen_selection,
        "targetActions": target_actions,
        "quickTargetIds": quick_target_ids,
        "elapsedMs": elapsed_ms,
        "audioLevels": audio_levels,
        "setup": {
            "ready": setup_state.ready,
            "model": {
                "status": dictation_model_status_label(&setup_state.model_status),
                "id": selected_model.stable_id,
                "displayName": selected_model.display_name,
                "description": selected_model.description,
                "recommended": selected_model.recommended,
                "downloadedSize": selected_model.downloaded_size_label(),
                "generation": generation,
                "source": "dictationModelCatalog",
            },
            "microphone": {
                "permissionStatus": format!("{:?}", permission),
                "status": microphone_status_label(&setup_state.microphone_status),
                "deviceSnapshot": device_snapshot,
                "generation": generation,
                "source": "passiveAVFoundationEnumeration",
                "noPermissionPrompt": true,
            },
            "hotkey": {
                "enabled": config.is_dictation_hotkey_enabled(),
                "configured": hotkey.is_some(),
                "display": hotkey.as_ref().map(|value| value.to_display_string()),
                "status": format!("{:?}", setup_state.hotkey_status),
                "generation": generation,
                "source": "config.dictationHotkey",
            },
            "configFingerprint": crate::config::current_config_fingerprint_receipt().map(|receipt| serde_json::json!({
                "pathFingerprint": automation_fingerprint(&receipt.path),
                "len": receipt.len,
                "modifiedMs": receipt.modified_ms,
            })),
        },
        "lastDelivery": crate::dictation::last_delivery_receipt(),
        "deliveryReceiptAvailable": crate::dictation::last_delivery_receipt().is_some(),
        "wrongTargetRefusal": crate::dictation::last_wrong_target_refusal(),
        "wrongTargetRefusalAvailable": crate::dictation::last_wrong_target_refusal().is_some(),
        "recovery": recovery,
        "stop": crate::dictation::last_stop_receipt(),
        "cleanup": {
            "captureActive": is_recording,
            "captureStopInProgress": crate::dictation::is_dictation_stopping(),
            "transcriberCached": TRANSCRIBER.lock().is_some(),
            "generation": generation,
            "source": "runtime.dictation.cleanupState",
        },
        "windowLifecycle": crate::dictation::dictation_window_lifecycle_receipt(),
        "safety": {
            "noMicrophoneCaptureStarted": true,
            "noSystemSettingsOpened": true,
            "noTccMutation": true,
            "noTranscriptContent": true,
            "rawDeviceIdsRedacted": true,
            "deviceLabelsReturned": false,
        },
    })
}

fn bump_dictation_state_generation() -> u64 {
    DICTATION_STATE_GENERATION.fetch_add(1, Ordering::Relaxed) + 1
}

pub(crate) fn notify_dictation_device_preference_changed() -> u64 {
    bump_dictation_state_generation()
}

pub(crate) fn notify_dictation_model_preference_changed() -> u64 {
    *TRANSCRIBER.lock() = None;
    bump_dictation_state_generation()
}

fn dictation_model_status() -> DictationModelStatus {
    let prefs = crate::config::load_user_preferences();
    let model_id = DictationModelId::from_preference(prefs.dictation.model.as_deref());
    if dictation_model_entry(model_id).is_available() {
        DictationModelStatus::Available
    } else {
        DictationModelStatus::NotDownloaded
    }
}

pub(crate) fn should_rebuild_cached_transcriber(
    cached_key: Option<&CachedTranscriberKey>,
    requested_key: &CachedTranscriberKey,
    cached_is_idle: bool,
) -> bool {
    cached_key != Some(requested_key) || cached_is_idle
}

fn dictation_model_status_label(status: &DictationModelStatus) -> &'static str {
    match status {
        DictationModelStatus::Available => "available",
        DictationModelStatus::NotDownloaded => "notDownloaded",
        DictationModelStatus::Downloading { .. } => "downloading",
        DictationModelStatus::Extracting => "extracting",
        DictationModelStatus::DownloadFailed(_) => "downloadFailed",
    }
}

fn microphone_status_label(status: &crate::dictation::DictationMicrophoneStatus) -> &'static str {
    match status {
        crate::dictation::DictationMicrophoneStatus::Ready { .. } => "ready",
        crate::dictation::DictationMicrophoneStatus::SavedDeviceMissing { .. } => {
            "savedDeviceMissing"
        }
        crate::dictation::DictationMicrophoneStatus::PermissionNeeded(_) => "permissionNeeded",
        crate::dictation::DictationMicrophoneStatus::NoDevices => "noDevices",
        crate::dictation::DictationMicrophoneStatus::EnumerationFailed(_) => "enumerationFailed",
    }
}

fn automation_fingerprint(value: &str) -> String {
    redacted_transcript_fingerprint(value)
}

fn microphone_device_snapshot(
    devices_result: &Result<Vec<crate::dictation::DictationDeviceInfo>, String>,
    selected_device_id: Option<&str>,
) -> serde_json::Value {
    match devices_result {
        Ok(devices) => {
            let selected =
                crate::dictation::resolve_selected_input_device(devices, selected_device_id);
            serde_json::json!({
                "available": true,
                "deviceCount": devices.len(),
                "defaultDevice": devices.iter().find(|device| device.is_default).map(|device| {
                    serde_json::json!({
                        "labelFingerprint": automation_fingerprint(&device.name),
                        "idFingerprint": automation_fingerprint(&device.id.0),
                        "transport": format!("{:?}", device.transport),
                    })
                }),
                "selected": selected.as_ref().map(|selection| {
                    serde_json::json!({
                        "labelFingerprint": automation_fingerprint(&selection.device.name),
                        "idFingerprint": automation_fingerprint(&selection.device.id.0),
                        "transport": format!("{:?}", selection.device.transport),
                        "fellBack": selection.fell_back,
                    })
                }),
                "savedPreference": {
                    "configured": selected_device_id.is_some(),
                    "idFingerprint": selected_device_id.map(automation_fingerprint),
                    "resolved": selected.as_ref().is_some_and(|selection| !selection.fell_back),
                },
                "rawDeviceIdsRedacted": true,
                "deviceLabelsReturned": false,
            })
        }
        Err(error) => serde_json::json!({
            "available": false,
            "deviceCount": 0,
            "error": error,
            "rawDeviceIdsRedacted": true,
            "deviceLabelsReturned": false,
        }),
    }
}

/// Transcribe previously captured audio chunks.
///
/// Lazily initialises the Whisper engine on first use and caches it for
/// subsequent calls.  Returns `Ok(None)` when the audio is too short or
/// silent.
pub fn transcribe_captured_audio(chunks: &[CapturedAudioChunk]) -> Result<Option<String>> {
    let prefs = crate::config::load_user_preferences();
    let model_id = DictationModelId::from_preference(prefs.dictation.model.as_deref());
    let catalog_entry = dictation_model_entry(model_id);
    let config = DictationTranscriptionConfig {
        model_path: catalog_entry.path(),
        language: prefs.dictation.language.clone(),
        silence_rms: prefs.dictation.silence_rms_threshold(),
        ..Default::default()
    };
    let audio_duration = captured_duration(chunks);
    let samples = merge_captured_chunks(chunks);
    let rms = crate::dictation::transcription::rms(&samples);

    tracing::info!(
        category = "DICTATION",
        chunk_count = chunks.len(),
        audio_duration_ms = audio_duration.as_millis() as u64,
        sample_count = samples.len(),
        rms,
        minimum_samples = config.minimum_samples,
        silence_rms = config.silence_rms,
        model_id = catalog_entry.stable_id,
        model_display_name = catalog_entry.display_name,
        model_path = %config.model_path.display(),
        model_exists = config.model_path.exists(),
        model_is_file = config.model_path.is_file(),
        "Starting dictation transcription"
    );

    if samples.len() < config.minimum_samples {
        tracing::info!(
            category = "DICTATION",
            sample_count = samples.len(),
            minimum_samples = config.minimum_samples,
            "Skipping dictation transcription: audio too short"
        );
        return Ok(None);
    }

    if rms < config.silence_rms {
        tracing::info!(
            category = "DICTATION",
            rms,
            threshold = config.silence_rms,
            "Skipping dictation transcription: audio too silent"
        );
        return Ok(None);
    }

    set_finalize_progress(Some(0.0));
    let result = transcribe_samples_with_cached_engine_progress(&samples, &mut |fraction| {
        set_finalize_progress(Some(fraction));
    });
    set_finalize_progress(None);

    match &result {
        Ok(Some(transcript)) => tracing::info!(
            category = "DICTATION",
            transcript_len = transcript.len(),
            "Dictation transcription succeeded"
        ),
        Ok(None) => tracing::info!(
            category = "DICTATION",
            "Dictation transcription completed without text"
        ),
        Err(error) => tracing::error!(
            category = "DICTATION",
            error = %error,
            "Dictation transcription failed"
        ),
    }

    result
}

/// Transcribe raw 16 kHz mono samples through the shared cached engine,
/// (re)building the engine when the model/language preference changed or the
/// cache idled out.
///
/// Shared by the final delivery pass and the live partial preview so both use
/// identical engine state — a partial attempt also pre-warms the model that
/// the final pass will use.
pub(crate) fn transcribe_samples_with_cached_engine(samples: &[f32]) -> Result<Option<String>> {
    transcribe_samples_with_cached_engine_progress(samples, &mut |_| {})
}

/// Progress-reporting variant of [`transcribe_samples_with_cached_engine`].
/// Long audio routes through the chunked path, which fires `progress` with
/// the fraction of audio whose chunks have completed.
pub(crate) fn transcribe_samples_with_cached_engine_progress(
    samples: &[f32],
    progress: &mut dyn FnMut(f32),
) -> Result<Option<String>> {
    let prefs = crate::config::load_user_preferences();
    let model_id = DictationModelId::from_preference(prefs.dictation.model.as_deref());
    let catalog_entry = dictation_model_entry(model_id);
    let config = DictationTranscriptionConfig {
        model_path: catalog_entry.path(),
        language: prefs.dictation.language.clone(),
        silence_rms: prefs.dictation.silence_rms_threshold(),
        ..Default::default()
    };

    let mut guard = TRANSCRIBER.lock();
    let requested_key = CachedTranscriberKey {
        model_id,
        language: config.language.clone(),
    };

    let cached_is_idle = guard
        .as_ref()
        .map(|cached| cached.transcriber.is_idle())
        .unwrap_or(false);
    let should_rebuild = should_rebuild_cached_transcriber(
        guard.as_ref().map(|cached| &cached.key),
        &requested_key,
        cached_is_idle,
    );

    tracing::debug!(
        category = "DICTATION",
        has_cached_transcriber = guard.is_some(),
        should_rebuild,
        "Resolving dictation transcriber"
    );

    if should_rebuild {
        if !catalog_entry.is_available() {
            if matches!(catalog_entry.engine_kind, DictationEngineKind::Parakeet) {
                anyhow::bail!(
                    "Parakeet model not downloaded ({}). Use the dictation settings to download it.",
                    catalog_entry.display_name
                );
            } else {
                anyhow::bail!(
                    "{} model not downloaded. Use the dictation settings to download it.",
                    catalog_entry.display_name
                );
            }
        }
        tracing::info!(
            category = "DICTATION",
            model_id = catalog_entry.stable_id,
            model_display_name = catalog_entry.display_name,
            model_path = %config.model_path.display(),
            "Initializing dictation engine"
        );
        let engine: Box<dyn crate::dictation::transcription::DictationEngine> =
            match catalog_entry.engine_kind {
                DictationEngineKind::Parakeet => Box::new(
                    ParakeetDictationEngine::new(&config.model_path).with_context(|| {
                        format!(
                            "failed to initialize Parakeet engine from {}",
                            config.model_path.display()
                        )
                    })?,
                ),
                DictationEngineKind::Whisper => {
                    Box::new(WhisperDictationEngine::new(&config).with_context(|| {
                        format!(
                            "failed to initialize Whisper engine from {}",
                            config.model_path.display()
                        )
                    })?)
                }
            };
        *guard = Some(CachedTranscriber {
            key: requested_key,
            transcriber: DictationTranscriber::new(config.clone(), engine),
        });
    }

    guard
        .as_ref()
        .context("dictation transcriber unavailable")?
        .transcriber
        .transcribe_samples_with_progress(samples, progress)
}

/// Unload the cached transcriber if it has been idle for longer than its
/// configured timeout.  Intended to be called on a timer from the app layer.
pub fn maybe_unload_transcriber() {
    let mut guard = TRANSCRIBER.lock();
    if guard
        .as_ref()
        .map(|cached| cached.transcriber.is_idle())
        .unwrap_or(false)
    {
        *guard = None;
    }
}

#[cfg(test)]
pub(crate) fn reset_cached_transcriber_for_tests() {
    *TRANSCRIBER.lock() = None;
}

// ---------------------------------------------------------------------------
// Start recording
// ---------------------------------------------------------------------------

fn start_recording(target: DictationTarget) -> Result<()> {
    let active_device = resolve_preferred_device_info()?;
    let device_id = active_device.as_ref().map(|device| device.id.clone());
    let capture_config = DictationCaptureConfig::default();

    let (event_rx, capture_handle) = start_capture(capture_config, device_id.as_ref())
        .context("failed to start audio capture")?;

    let prefs = crate::config::load_user_preferences();
    let session_generation = SESSION_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    *PARTIAL_TRANSCRIPT.lock() = None;
    // A new session opens with the (possibly just-changed) saved preference,
    // so any pending-device note from the previous session is obsolete.
    *PENDING_DEVICE_LABEL.lock() = None;

    let session = DictationSession {
        capture_handle: Some(capture_handle),
        event_rx,
        chunks: Vec::new(),
        last_bars: silent_bars(),
        started_at: Instant::now(),
        overlay_phase: DictationSessionPhase::Recording,
        target,
        selection: None,
        target_cycle: vec![target],
        active_device: active_device.clone(),
        session_generation,
        live_preview: prefs.dictation.live_preview_enabled(),
        max_duration: prefs.dictation.max_duration(),
        last_partial_attempt_at: None,
    };

    *SESSION.lock() = Some(session);
    bump_dictation_state_generation();

    tracing::info!(
        category = "DICTATION",
        device = ?device_id,
        device_name = ?active_device.as_ref().map(|device| device.name.as_str()),
        ?target,
        target_label = target.overlay_label(),
        "Recording started"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Stop recording
// ---------------------------------------------------------------------------

/// Stop the current recording and return the completed capture.
///
/// **Important**: the capture handle is dropped *before* draining the event
/// channel.  This ensures the AVFoundation session stops, the processor
/// thread flushes the tail chunk, and `EndOfStream` is sent before we begin
/// collecting.  Without this ordering, `try_recv()` would miss the tail.
fn stop_recording() -> Result<Option<CompletedDictationCapture>> {
    let session = SESSION.lock().take();
    let Some(mut session) = session else {
        tracing::warn!(
            category = "DICTATION",
            "stop_recording called but no session active"
        );
        return Ok(None);
    };
    bump_dictation_state_generation();

    stop_capture_and_collect(&mut session)?;

    let audio_duration = captured_duration(&session.chunks);
    tracing::info!(
        category = "DICTATION",
        chunks = session.chunks.len(),
        audio_duration_ms = audio_duration.as_millis() as u64,
        "Recording stopped"
    );

    if session.chunks.is_empty() {
        return Ok(None);
    }

    Ok(Some(CompletedDictationCapture {
        chunks: session.chunks,
        audio_duration,
        truncated: false,
    }))
}

impl DictationStopJob {
    pub fn collect_with_deadline(
        mut self,
        timeout: Duration,
    ) -> Result<Option<CompletedDictationCapture>> {
        let started = Instant::now();
        tracing::info!(
            category = "DICTATION",
            timeout_ms = timeout.as_millis() as u64,
            "capture_stop_job_started"
        );

        let truncated = stop_capture_and_collect_with_deadline(&mut self.session, timeout)?;

        let audio_duration = captured_duration(&self.session.chunks);
        tracing::info!(
            category = "DICTATION",
            chunks = self.session.chunks.len(),
            audio_duration_ms = audio_duration.as_millis() as u64,
            elapsed_ms = started.elapsed().as_millis() as u64,
            truncated,
            "capture_stop_job_finished"
        );

        if self.session.chunks.is_empty() {
            return Ok(None);
        }

        Ok(Some(CompletedDictationCapture {
            chunks: self.session.chunks,
            audio_duration,
            truncated,
        }))
    }
}

/// Drop the capture handle first so the processor thread can flush its tail
/// chunk and send `EndOfStream`, then blocking-drain the channel.
fn stop_capture_and_collect(session: &mut DictationSession) -> Result<()> {
    // Drop capture handle — triggers AVCaptureSession stop, processor thread
    // flush, and EndOfStream emission.
    let _ = session.capture_handle.take();

    // Blocking drain until EndOfStream.
    while let Ok(event) = session.event_rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => session.chunks.push(chunk),
            DictationCaptureEvent::Bars(bars) => session.last_bars = bars,
            DictationCaptureEvent::EndOfStream => return Ok(()),
        }
    }

    anyhow::bail!("dictation capture stream closed before EndOfStream")
}

/// Returns `Ok(true)` when collection timed out before `EndOfStream` (the
/// capture tail may be missing), `Ok(false)` on a clean end of stream.
fn stop_capture_and_collect_with_deadline(
    session: &mut DictationSession,
    timeout: Duration,
) -> Result<bool> {
    let started = Instant::now();
    let _ = session.capture_handle.take();

    loop {
        if started.elapsed() > timeout {
            tracing::warn!(
                category = "DICTATION",
                event = "capture_stop_job_timed_out",
                elapsed_ms = started.elapsed().as_millis() as u64,
                chunks = session.chunks.len(),
                "Timed out waiting for dictation capture EndOfStream; using partial capture"
            );
            return Ok(true);
        }

        match session.event_rx.try_recv() {
            Ok(DictationCaptureEvent::Chunk(chunk)) => session.chunks.push(chunk),
            Ok(DictationCaptureEvent::Bars(bars)) => session.last_bars = bars,
            Ok(DictationCaptureEvent::EndOfStream) => return Ok(false),
            Err(async_channel::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(5)),
            Err(async_channel::TryRecvError::Closed) => return Ok(false),
        }
    }
}

// ---------------------------------------------------------------------------
// Mic preference resolution
// ---------------------------------------------------------------------------

/// Resolve the user's preferred microphone device.
///
/// Reads the config-backed `dictation.selected_device_id` preference and, if the
/// device is still present, returns it.  Falls back using ranked heuristics
/// (system default → built-in → USB → first non-virtual → any) when the
/// preference is unset or the device has disappeared.
///
/// Delegates the pure selection logic to [`resolve_selected_input_device`] so
/// the behavior is deterministic and testable without I/O.
pub(crate) fn resolve_preferred_device_info() -> Result<Option<DictationDeviceInfo>> {
    let prefs = crate::config::load_user_preferences();
    let preferred_id = prefs.dictation.selected_device_id.clone();

    let devices = list_input_devices()?;
    let resolution = resolve_selected_input_device(&devices, preferred_id.as_deref());

    match resolution {
        Some(res) if !res.fell_back => {
            tracing::info!(
                category = "DICTATION",
                device_id = %res.device.id.0,
                device_name = %res.device.name,
                transport = ?res.device.transport,
                saved = preferred_id.is_some(),
                "Using microphone"
            );
            Ok(Some(res.device))
        }
        Some(res) => {
            // Saved device disappeared — clear the stale preference.
            tracing::warn!(
                category = "DICTATION",
                missing_device_id = ?preferred_id,
                fallback_device_id = %res.device.id.0,
                fallback_device_name = %res.device.name,
                fallback_transport = ?res.device.transport,
                "Saved microphone not found, falling back"
            );
            if let Err(error) = crate::dictation::save_dictation_device_id(None) {
                tracing::warn!(
                    category = "DICTATION",
                    error = %error,
                    "Failed to clear stale microphone preference"
                );
            }
            Ok(Some(res.device))
        }
        None => {
            tracing::warn!(category = "DICTATION", "No input devices available");
            Ok(None)
        }
    }
}

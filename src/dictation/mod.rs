//! Voice dictation: audio capture, transcription, and transcript delivery.
//!
//! This module compiles into both the library and the binary crate.  The
//! re-export lists below form the library's public API, so the binary build
//! (which only consumes a subset) would otherwise flag them as unused —
//! hence the scoped `unused_imports` allow.  `dead_code` is deliberately NOT
//! allowed module-wide: genuinely dead items must be deleted or carry a
//! documented item-level allow.
#![allow(unused_imports)]

pub mod capture;
mod catalog;
mod delivery;
mod device;
pub mod download;
mod history;
mod live_caption;
mod microphone_popup_window;
mod runtime;
mod setup;
mod transcription;
mod types;
mod visualizer;
mod window;

pub use capture::{start_capture, DictationCaptureHandle};
pub use catalog::{
    dictation_model_catalog, dictation_model_entry, format_dictation_model_size,
    DictationEngineKind, DictationModelCatalogEntry, DictationModelId,
};
pub use delivery::{
    capture_frozen_external_destination, parse_dictation_target_label,
    resolve_delivery_target_request, resolve_dictation_target_label,
    validate_frozen_external_destination, DictationDeliveryTargetResolution,
    DictationDeliveryTargetSource, DictationTargetLabelResolution, DictationWrongTargetReason,
    DictationWrongTargetRefusalDraft,
};
pub use device::{
    apply_device_selection, build_device_menu_items, default_input_device,
    device_selection_action_from_value, device_selection_value, list_input_device_menu_items,
    list_input_devices, microphone_display_label, microphone_permission_status,
    request_microphone_permission, request_microphone_permission_nonblocking,
    resolve_selected_input_device, save_dictation_device_id, save_dictation_language,
    save_dictation_last_target, save_dictation_model, DeviceResolution, DictationDeviceMenuItem,
    DictationDeviceSelectionAction, DICTATION_SYSTEM_DEFAULT_DEVICE_VALUE,
};
pub use history::{
    build_history_entry, delete_history_entry, format_history_duration_ms,
    format_history_timestamp, get_history_entry, hydrate_dictation_resource_from_history,
    load_history, record_dictation_history, root_dictation_history_query_is_eligible,
    search_history, search_root_dictation_history, search_root_dictation_history_cached,
    search_root_dictation_history_direct, DictationHistoryEntry, DictationHistorySearchField,
    DictationHistorySearchHit, RootDictationHistorySearchHit, RootDictationHistorySectionOptions,
};
// The batch_select_* automation hooks are consumed by the binary crate
// (prompt_handler), which compiles this module separately — the library
// build alone cannot see those uses.
#[allow(unused_imports)]
pub(crate) use microphone_popup_window::{
    batch_select_dictation_microphone_popup_row_by_semantic_id,
    batch_select_dictation_microphone_popup_row_by_value,
    build_dictation_microphone_popup_snapshot, close_dictation_microphone_popup_window,
    close_dictation_microphone_popup_window_for_owner_loss,
    dismiss_dictation_microphone_popup_from_parent, is_dictation_microphone_popup_window_open,
    sync_dictation_microphone_popup_window, DictationMicrophonePopupRequest,
    DictationMicrophonePopupSelectionMode, DictationMicrophonePopupSnapshot,
    DictationPopupDismissOutcome, DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
};
pub use runtime::{
    abort_dictation, automation_state, begin_stop_capture, can_cycle_dictation_target,
    claim_dictation_delivery, current_dictation_phase, cycle_dictation_target,
    delivery_receipt_generation, dictation_auto_stop_due, dictation_delivery_was_claimed,
    dictation_elapsed, dictation_stop_target, dictation_target_generation, finalize_progress,
    finish_stop_capture, get_active_dictation_device, get_dictation_target,
    get_dictation_target_selection, is_dictation_busy, is_dictation_recording,
    is_dictation_stopping, last_delivery_receipt, last_partial_transcript, last_stop_receipt,
    last_wrong_target_refusal, maybe_unload_transcriber, next_dictation_delivery_id,
    pending_dictation_device_label, record_delivery_receipt, record_wrong_target_refusal,
    redacted_transcript_fingerprint, resolve_final_or_partial_transcript,
    set_dictation_session_selection, set_dictation_session_target, set_dictation_target_cycle,
    set_overlay_phase, set_pending_dictation_device_label, snapshot_overlay_state,
    toggle_dictation, transcribe_captured_audio, BeginStopCapture, DictationStopJob,
    DictationStopReason, DictationTranscriptResolution,
};
pub(crate) use runtime::{
    clear_dictation_recovery_work, clear_dictation_return_origin, dictation_pipeline_failure_state,
    dictation_recovery_work, dictation_return_origin, preserve_dictation_recovery_work,
    record_fixture_dictation_target_selection, replace_dictation_recovery_work,
    replace_dictation_return_origin, retain_frozen_selection_for_delivery,
    toggled_post_stop_restart, DictationRecoveryWork,
};
pub use setup::{
    build_dictation_setup_state, DictationHotkeyStatus, DictationMicrophonePermissionStatus,
    DictationMicrophoneStatus, DictationSetupState,
};
pub use transcription::{
    build_session_result, captured_duration, is_parakeet_model_available, merge_captured_chunks,
    resolve_default_model_path, resolve_whisper_model_path, DictationEngine, DictationTranscriber,
    DictationTranscriptionConfig, ParakeetDictationEngine, WhisperDictationEngine,
    PARAKEET_MODEL_ARCHIVE_SIZE, PARAKEET_MODEL_URL, WHISPER_MODEL_SIZE, WHISPER_MODEL_URL,
};
pub use types::{
    CapturedAudioChunk, CompletedDictationCapture, DictationAutoSubmitPermission,
    DictationCaptureConfig, DictationCaptureEvent, DictationDeliveryFailureReason,
    DictationDeliveryOutcome, DictationDeliveryRequest, DictationDestination, DictationDeviceId,
    DictationDeviceInfo, DictationDeviceTransport, DictationFailureRecoveryCapabilities,
    DictationFailureState, DictationModelStatus, DictationMutationReceipt, DictationRecoveryAction,
    DictationRecoveryCapabilities, DictationReturnOrigin, DictationSessionPhase,
    DictationSessionResult, DictationTarget, DictationTargetDescriptor,
    DictationTargetPersistenceClass, DictationTargetSelection, DictationToggleOutcome,
    DictationTranscriptPreservationReceipt, FrozenAgentChatPolicy, FrozenDictationDestination,
    ImmutableDictationTranscript, RawAudioChunk, ALL_DICTATION_TARGETS,
};
pub use visualizer::{animate_bars, silent_bars};
pub use window::{
    automation_layout_info, begin_overlay_session, close_dictation_overlay,
    is_dictation_overlay_open, open_dictation_overlay, overlay_generation,
    reopen_last_dictation_overlay, set_overlay_abort_callback, set_overlay_recovery_callback,
    set_overlay_retarget_callback, set_overlay_submit_callback, update_dictation_overlay,
    DictationOverlay, DictationOverlayState,
};
pub(crate) use window::{
    destination_selector_spec, dictation_overlay_fixture_mode, dictation_window_lifecycle_receipt,
    last_dictation_overlay_state, open_dictation_microphone_popup_fixture,
    overlay_recovery_callback_installed, set_dictation_overlay_fixture_mode,
};

#[cfg(test)]
mod tests;

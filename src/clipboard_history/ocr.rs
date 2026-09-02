//! Clipboard OCR worker backed by macOS Vision.
//!
//! Jobs are queued from clipboard history entries that store image content as
//! `blob:<hash>`. The worker resolves blobs to `~/.scriptkit/clipboard/blobs/<hash>.png`,
//! runs OCR, and persists OCR text into SQLite.

use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use tracing::{error, info, warn};

use super::{blob_store, database};

#[cfg(all(target_os = "macos", feature = "ocr"))]
use cocoa::base::{id, nil};
#[cfg(all(target_os = "macos", feature = "ocr"))]
use objc::{class, msg_send, sel, sel_impl};

const OCR_QUEUE_CAPACITY: usize = 8;

/// OCR worker message.
pub enum OcrMsg {
    Job { id: String, content: String },
    Shutdown,
}

static OCR_SENDER: OnceLock<SyncSender<OcrMsg>> = OnceLock::new();
static OCR_WORKER_HANDLE: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();

/// Start the clipboard OCR worker thread.
pub fn start_ocr_worker() -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    if OCR_SENDER.get().is_some() {
        info!("clipboard_ocr_worker_start_skipped_already_started");
        return Ok(());
    }

    let (tx, rx): (SyncSender<OcrMsg>, Receiver<OcrMsg>) = mpsc::sync_channel(OCR_QUEUE_CAPACITY);
    let handle = match thread::Builder::new()
        .name("clipboard-ocr-worker".to_string())
        .spawn(move || ocr_worker_loop(rx))
    {
        Ok(handle) => handle,
        Err(error) => {
            error!(
                error = %error,
                "clipboard_ocr_worker_start_failed_spawn"
            );
            return Err(anyhow!("clipboard_ocr_worker_start_failed_spawn: {error}"));
        }
    };

    if let Err(tx_on_race) = OCR_SENDER.set(tx) {
        warn!("clipboard_ocr_worker_start_race_detected_sender_already_set");
        if let Err(send_error) = tx_on_race.send(OcrMsg::Shutdown) {
            warn!(
                error = %send_error,
                "clipboard_ocr_worker_start_race_shutdown_send_failed"
            );
        }
        if handle.join().is_err() {
            error!("clipboard_ocr_worker_start_race_shutdown_join_failed");
        }
        return Ok(());
    }

    let handle_slot = OCR_WORKER_HANDLE.get_or_init(|| Mutex::new(None));
    match handle_slot.lock() {
        Ok(mut guard) => {
            *guard = Some(handle);
            info!(
                queue_capacity = OCR_QUEUE_CAPACITY,
                "clipboard_ocr_worker_started"
            );
        }
        Err(lock_error) => {
            error!(
                error = %lock_error,
                "clipboard_ocr_worker_start_failed_lock_poisoned"
            );
            if let Some(sender) = OCR_SENDER.get() {
                if let Err(send_error) = sender.send(OcrMsg::Shutdown) {
                    warn!(
                        error = %send_error,
                        "clipboard_ocr_worker_start_failed_lock_shutdown_send_failed"
                    );
                }
            }
            if handle.join().is_err() {
                error!("clipboard_ocr_worker_start_failed_lock_shutdown_join_failed");
            }
            return Err(anyhow!(
                "clipboard_ocr_worker_start_failed_lock_poisoned: {lock_error}"
            ));
        }
    }

    Ok(())
}

/// Stop the clipboard OCR worker thread.
pub fn stop_ocr_worker() -> Result<()> {
    if let Some(sender) = OCR_SENDER.get() {
        if let Err(send_error) = sender.send(OcrMsg::Shutdown) {
            warn!(
                error = %send_error,
                "clipboard_ocr_worker_stop_shutdown_send_failed"
            );
            return Err(anyhow!(
                "clipboard_ocr_worker_stop_shutdown_send_failed: {send_error}"
            ));
        }
    } else {
        info!("clipboard_ocr_worker_stop_skipped_not_started");
    }

    let Some(handle_slot) = OCR_WORKER_HANDLE.get() else {
        return Ok(());
    };

    match handle_slot.lock() {
        Ok(mut guard) => {
            let Some(handle) = guard.take() else {
                info!("clipboard_ocr_worker_stop_skipped_already_joined");
                return Ok(());
            };

            if handle.join().is_err() {
                error!("clipboard_ocr_worker_stop_join_failed_panic");
                return Err(anyhow!("clipboard_ocr_worker_stop_join_failed_panic"));
            } else {
                info!("clipboard_ocr_worker_stopped");
            }
        }
        Err(lock_error) => {
            error!(
                error = %lock_error,
                "clipboard_ocr_worker_stop_failed_lock_poisoned"
            );
            return Err(anyhow!(
                "clipboard_ocr_worker_stop_failed_lock_poisoned: {lock_error}"
            ));
        }
    }

    Ok(())
}

/// Queue an OCR job without blocking the caller.
pub fn enqueue_ocr(id: String, content: String) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    let Some(sender) = OCR_SENDER.get() else {
        warn!(
            entry_id = %id,
            "clipboard_ocr_enqueue_failed_worker_not_started"
        );
        return Err(anyhow!("clipboard_ocr_enqueue_failed_worker_not_started"));
    };

    match sender.try_send(OcrMsg::Job { id, content }) {
        Ok(()) => {
            info!("clipboard_ocr_enqueue_success");
            Ok(())
        }
        Err(TrySendError::Full(OcrMsg::Job { id, .. })) => {
            warn!(
                entry_id = %id,
                queue_capacity = OCR_QUEUE_CAPACITY,
                "clipboard_ocr_enqueue_dropped_queue_full"
            );
            Err(anyhow!(
                "clipboard_ocr_enqueue_dropped_queue_full: entry_id={id}"
            ))
        }
        Err(TrySendError::Disconnected(OcrMsg::Job { id, .. })) => {
            error!(
                entry_id = %id,
                "clipboard_ocr_enqueue_failed_worker_disconnected"
            );
            Err(anyhow!(
                "clipboard_ocr_enqueue_failed_worker_disconnected: entry_id={id}"
            ))
        }
        Err(TrySendError::Full(OcrMsg::Shutdown))
        | Err(TrySendError::Disconnected(OcrMsg::Shutdown)) => {
            warn!("clipboard_ocr_enqueue_unexpected_shutdown_variant");
            Err(anyhow!("clipboard_ocr_enqueue_unexpected_shutdown_variant"))
        }
    }
}

fn ocr_worker_loop(rx: Receiver<OcrMsg>) {
    info!("clipboard_ocr_worker_loop_started");

    for msg in rx {
        match msg {
            OcrMsg::Job { id, content } => process_ocr_job(id, content),
            OcrMsg::Shutdown => {
                info!("clipboard_ocr_worker_loop_shutdown_received");
                break;
            }
        }
    }

    info!("clipboard_ocr_worker_loop_exited");
}

fn process_ocr_job(id: String, content: String) {
    let blob_path = match resolve_blob_path(&content) {
        Ok(path) => path,
        Err(error) => {
            warn!(
                entry_id = %id,
                error = %error,
                "clipboard_ocr_job_failed_resolve_blob_path"
            );
            return;
        }
    };

    let blob_path_str = match blob_path.to_str() {
        Some(path) => path,
        None => {
            warn!(
                entry_id = %id,
                blob_path = %blob_path.display(),
                "clipboard_ocr_job_failed_non_utf8_blob_path"
            );
            return;
        }
    };

    match run_vision_ocr(blob_path_str) {
        Ok(text) => {
            if let Err(error) = database::update_ocr_text(&id, &text) {
                error!(
                    entry_id = %id,
                    text_len = text.len(),
                    error = %error,
                    "clipboard_ocr_job_failed_update_database"
                );
            } else {
                info!(
                    entry_id = %id,
                    text_len = text.len(),
                    "clipboard_ocr_job_persisted"
                );
            }
        }
        Err(error) => {
            warn!(
                entry_id = %id,
                blob_path = %blob_path.display(),
                error = %error,
                "clipboard_ocr_job_failed_vision"
            );
        }
    }
}

fn resolve_blob_path(content: &str) -> Result<PathBuf> {
    let hash = extract_blob_hash(content)?;
    let blob_dir = blob_store::get_blob_dir().context(
        "clipboard_ocr_resolve_blob_path_failed: unable to resolve ~/.scriptkit/clipboard/blobs",
    )?;
    Ok(blob_dir.join(format!("{hash}.png")))
}

fn extract_blob_hash(content: &str) -> Result<&str> {
    if !blob_store::is_blob_content(content) {
        bail!("clipboard_ocr_extract_blob_hash_failed: expected blob: prefix, got '{content}'");
    }

    let hash = content
        .strip_prefix("blob:")
        .context("clipboard_ocr_extract_blob_hash_failed: blob prefix missing")?;

    if hash.is_empty() {
        bail!("clipboard_ocr_extract_blob_hash_failed: blob hash is empty");
    }

    Ok(hash)
}

#[cfg(all(target_os = "macos", feature = "ocr"))]
#[link(name = "Vision", kind = "framework")]
extern "C" {}

/// Run OCR on a blob image path using macOS Vision.
#[cfg(all(target_os = "macos", feature = "ocr"))]
pub fn run_vision_ocr(blob_path: &str) -> Result<String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ExternalStorage)?;
    // SAFETY: run_vision_ocr_inner uses objc messaging to Vision framework classes.
    // All ObjC objects are nil-checked. The autoreleasepool ensures proper cleanup.
    objc::rc::autoreleasepool(|| unsafe { run_vision_ocr_inner(blob_path) })
}

// SAFETY: Caller must wrap in an autoreleasepool. Uses objc messaging to create
// VNImageRequestHandler and VNRecognizeTextRequest. All objects are nil-checked
// and released on error paths.
#[cfg(all(target_os = "macos", feature = "ocr"))]
unsafe fn run_vision_ocr_inner(blob_path: &str) -> Result<String> {
    if !Path::new(blob_path).exists() {
        bail!("clipboard_ocr_run_vision_failed_missing_blob_file: path={blob_path}");
    }

    let ns_path = nsstring_from_str(blob_path)
        .with_context(|| format!("clipboard_ocr_nsstring_failed: path={blob_path}"))?;

    let file_url: id = msg_send![class!(NSURL), fileURLWithPath: ns_path];
    if file_url == nil {
        bail!("clipboard_ocr_run_vision_failed_file_url_creation: path={blob_path}");
    }

    let image_data: id = msg_send![class!(NSData), dataWithContentsOfURL: file_url];
    if image_data == nil {
        bail!("clipboard_ocr_run_vision_failed_data_load: path={blob_path}");
    }

    let options: id = msg_send![class!(NSDictionary), dictionary];
    if options == nil {
        bail!("clipboard_ocr_run_vision_failed_options_dictionary");
    }

    let request_handler_alloc: id = msg_send![class!(VNImageRequestHandler), alloc];
    if request_handler_alloc == nil {
        bail!("clipboard_ocr_run_vision_failed_handler_alloc");
    }

    let request_handler: id =
        msg_send![request_handler_alloc, initWithData: image_data options: options];
    if request_handler == nil {
        bail!("clipboard_ocr_run_vision_failed_handler_init");
    }

    let text_request_alloc: id = msg_send![class!(VNRecognizeTextRequest), alloc];
    if text_request_alloc == nil {
        let _: () = msg_send![request_handler, release];
        bail!("clipboard_ocr_run_vision_failed_request_alloc");
    }

    let text_request: id = msg_send![text_request_alloc, init];
    if text_request == nil {
        let _: () = msg_send![request_handler, release];
        bail!("clipboard_ocr_run_vision_failed_request_init");
    }

    // 1 = VNRequestTextRecognitionLevelFast
    let _: () = msg_send![text_request, setRecognitionLevel: 1i64];
    let _: () = msg_send![text_request, setUsesLanguageCorrection: false];

    let requests: id = msg_send![class!(NSArray), arrayWithObject: text_request];
    if requests == nil {
        let _: () = msg_send![text_request, release];
        let _: () = msg_send![request_handler, release];
        bail!("clipboard_ocr_run_vision_failed_request_array");
    }

    let mut perform_error: id = nil;
    let success: bool = msg_send![
        request_handler,
        performRequests: requests
        error: &mut perform_error
    ];

    if !success || perform_error != nil {
        let error_message = if perform_error != nil {
            let localized: id = msg_send![perform_error, localizedDescription];
            nsstring_to_string(localized)
        } else {
            "Unknown Vision OCR error".to_string()
        };

        let _: () = msg_send![text_request, release];
        let _: () = msg_send![request_handler, release];

        return Err(anyhow!(
            "clipboard_ocr_run_vision_failed_perform_requests: {}",
            error_message
        ));
    }

    let results: id = msg_send![text_request, results];
    if results == nil {
        let _: () = msg_send![text_request, release];
        let _: () = msg_send![request_handler, release];

        info!("clipboard_ocr_run_vision_completed_no_results");
        return Ok(String::new());
    }

    let mut lines: Vec<String> = Vec::new();
    let count: usize = msg_send![results, count];

    for index in 0..count {
        let observation: id = msg_send![results, objectAtIndex: index];
        if observation == nil {
            continue;
        }

        let candidates: id = msg_send![observation, topCandidates: 1usize];
        if candidates == nil {
            continue;
        }

        let candidate_count: usize = msg_send![candidates, count];
        if candidate_count == 0 {
            continue;
        }

        let top_candidate: id = msg_send![candidates, objectAtIndex: 0usize];
        if top_candidate == nil {
            continue;
        }

        let text_ns: id = msg_send![top_candidate, string];
        let text = nsstring_to_string(text_ns);
        if !text.trim().is_empty() {
            lines.push(text);
        }
    }

    let _: () = msg_send![text_request, release];
    let _: () = msg_send![request_handler, release];

    let text = lines.join("\n");
    info!(
        path = blob_path,
        lines = lines.len(),
        text_len = text.len(),
        "clipboard_ocr_run_vision_completed"
    );

    Ok(text)
}

#[cfg(not(all(target_os = "macos", feature = "ocr")))]
pub fn run_vision_ocr(blob_path: &str) -> Result<String> {
    let _ = blob_path;
    bail!("clipboard_ocr_run_vision_unsupported_platform_or_feature")
}

// SAFETY: Caller must pass a valid NSString pointer or nil. The pointer is
// nil-checked, and UTF8String returns a pointer to the string's internal buffer.
#[cfg(all(target_os = "macos", feature = "ocr"))]
unsafe fn nsstring_to_string(ns_string: id) -> String {
    if ns_string == nil {
        return String::new();
    }

    let utf8_ptr: *const i8 = msg_send![ns_string, UTF8String];
    if utf8_ptr.is_null() {
        return String::new();
    }

    std::ffi::CStr::from_ptr(utf8_ptr)
        .to_string_lossy()
        .into_owned()
}

// SAFETY: Caller must ensure the returned NSString is used within an autoreleasepool.
// The CString is valid for the duration of the msg_send! call. Result is nil-checked.
#[cfg(all(target_os = "macos", feature = "ocr"))]
unsafe fn nsstring_from_str(value: &str) -> Result<id> {
    let c_string = std::ffi::CString::new(value)
        .context("clipboard_ocr_nsstring_from_str_failed: input contains interior NUL byte")?;

    let ns_string: id = msg_send![class!(NSString), stringWithUTF8String: c_string.as_ptr()];
    if ns_string == nil {
        bail!("clipboard_ocr_nsstring_from_str_failed: NSString allocation returned nil");
    }

    Ok(ns_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_blob_hash_returns_hash_when_content_has_blob_prefix() {
        let hash = extract_blob_hash("blob:abc123").expect("expected blob hash extraction to work");
        assert_eq!(hash, "abc123");
    }

    #[test]
    fn test_extract_blob_hash_errors_when_content_missing_blob_prefix() {
        let error = extract_blob_hash("png:abc123")
            .expect_err("expected missing blob prefix to fail")
            .to_string();

        assert!(
            error.contains("expected blob: prefix"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn test_resolve_blob_path_appends_png_when_content_is_blob_reference() {
        let path = resolve_blob_path("blob:testhash").expect("expected blob path resolution");
        assert!(
            path.to_string_lossy()
                .ends_with("clipboard/blobs/testhash.png"),
            "unexpected path: {}",
            path.display()
        );
    }
}

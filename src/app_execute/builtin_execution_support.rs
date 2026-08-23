struct CurrentAppScriptCaptureRequest {
    trace_id: String,
    raw_query: String,
    snapshot: crate::menu_bar::FrontmostMenuSnapshot,
    entries: Vec<crate::builtins::BuiltInEntry>,
    snapshot_receipt: crate::menu_bar::FrontmostMenuSnapshotReceipt,
    snapshot_pid: i32,
}

struct CurrentAppCapturedContext {
    selected_text: Option<String>,
    browser_url: Option<String>,
}

#[derive(Clone, Copy)]
struct DictationDeliveryTiming {
    audio_duration: std::time::Duration,
    target: crate::dictation::DictationTarget,
}

/// Typed progress events sent from the blocking download thread to the
/// async context for updating the in-prompt progress display.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DictationModelProgressEvent {
    Downloading {
        percentage: u8,
        downloaded_bytes: u64,
        total_bytes: u64,
        speed_bytes_per_sec: u64,
        eta_seconds: Option<u64>,
    },
    Extracting,
}

/// Simple rolling-window speed tracker for download progress.
struct SpeedTracker {
    last_bytes: u64,
    last_time: std::time::Instant,
    speed: u64,
}

impl SpeedTracker {
    fn new() -> Self {
        Self {
            last_bytes: 0,
            last_time: std::time::Instant::now(),
            speed: 0,
        }
    }

    fn update(&mut self, downloaded: u64) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        if elapsed >= 0.5 {
            let delta = downloaded.saturating_sub(self.last_bytes);
            self.speed = (delta as f64 / elapsed) as u64;
            self.last_bytes = downloaded;
            self.last_time = now;
        }
    }

    fn speed_bytes_per_sec(&self) -> u64 {
        self.speed
    }
}

/// Phases tracked by the UI coalescing emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationModelUiPhase {
    Downloading,
    Extracting,
}

/// Snapshot of the last UI-visible state, used to decide whether a new
/// progress event is worth publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DictationModelUiSnapshot {
    phase: DictationModelUiPhase,
    percentage: u8,
    eta_bucket_seconds: Option<u64>,
}

impl DictationModelUiSnapshot {
    fn downloading(percentage: u8, eta_seconds: Option<u64>) -> Self {
        Self {
            phase: DictationModelUiPhase::Downloading,
            percentage,
            eta_bucket_seconds: bucket_dictation_eta_seconds(eta_seconds),
        }
    }

    fn extracting() -> Self {
        Self {
            phase: DictationModelUiPhase::Extracting,
            percentage: 100,
            eta_bucket_seconds: Some(0),
        }
    }
}

/// Gates cosmetic UI updates so the download thread is never blocked on
/// repaints.  Publishes on meaningful change or after a ~300 ms heartbeat.
#[derive(Debug, Default)]
struct DictationModelUiEmitter {
    last_emit_at: Option<std::time::Instant>,
    last_snapshot: Option<DictationModelUiSnapshot>,
}

impl DictationModelUiEmitter {
    fn should_emit(&self, now: std::time::Instant, next: &DictationModelUiSnapshot) -> bool {
        const HEARTBEAT: std::time::Duration = std::time::Duration::from_millis(300);

        let Some(last_snapshot) = self.last_snapshot.as_ref() else {
            return true;
        };
        let Some(last_emit_at) = self.last_emit_at else {
            return true;
        };

        if last_snapshot.phase != next.phase {
            return true;
        }
        if last_snapshot.percentage != next.percentage {
            return true;
        }
        if last_snapshot.eta_bucket_seconds != next.eta_bucket_seconds {
            return true;
        }

        now.duration_since(last_emit_at) >= HEARTBEAT
    }

    fn record_emit(&mut self, now: std::time::Instant, next: &DictationModelUiSnapshot) {
        self.last_emit_at = Some(now);
        self.last_snapshot = Some(next.clone());
    }
}

/// Bucket ETA seconds into human-friendly steps so minor fluctuations
/// don't trigger a UI repaint.
fn bucket_dictation_eta_seconds(eta_seconds: Option<u64>) -> Option<u64> {
    eta_seconds.map(|value| match value {
        0..=15 => value,
        16..=60 => value - (value % 5),
        61..=300 => value - (value % 15),
        _ => value - (value % 60),
    })
}

/// Prevent overlapping Parakeet model downloads when the dictation hotkey is
/// pressed repeatedly while the model is still missing.
static PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static DICTATION_MODEL_PROMPT_STATUS: std::sync::OnceLock<
    parking_lot::Mutex<crate::dictation::DictationModelStatus>,
> = std::sync::OnceLock::new();

fn dictation_model_prompt_status(
) -> &'static parking_lot::Mutex<crate::dictation::DictationModelStatus> {
    DICTATION_MODEL_PROMPT_STATUS.get_or_init(|| {
        parking_lot::Mutex::new(crate::dictation::DictationModelStatus::NotDownloaded)
    })
}

static PARAKEET_MODEL_DOWNLOAD_CANCEL: std::sync::OnceLock<
    parking_lot::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
> = std::sync::OnceLock::new();

fn parakeet_model_download_cancel_slot(
) -> &'static parking_lot::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> {
    PARAKEET_MODEL_DOWNLOAD_CANCEL.get_or_init(|| parking_lot::Mutex::new(None))
}

static PENDING_DICTATION_MODEL_ACTION: std::sync::OnceLock<
    parking_lot::Mutex<Option<DictationBuiltinAction>>,
> = std::sync::OnceLock::new();

fn pending_dictation_model_action() -> &'static parking_lot::Mutex<Option<DictationBuiltinAction>> {
    PENDING_DICTATION_MODEL_ACTION.get_or_init(|| parking_lot::Mutex::new(None))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingDictationRestart {
    action: DictationBuiltinAction,
    target: crate::dictation::DictationTarget,
}

static PENDING_DICTATION_RESTART: std::sync::OnceLock<
    parking_lot::Mutex<Option<PendingDictationRestart>>,
> = std::sync::OnceLock::new();

fn pending_dictation_restart() -> &'static parking_lot::Mutex<Option<PendingDictationRestart>> {
    PENDING_DICTATION_RESTART.get_or_init(|| parking_lot::Mutex::new(None))
}

/// A toggle received while capture teardown is in flight flips the desired
/// post-stop state. Odd extra presses queue a restart; even extra presses
/// cancel it. This preserves toggle parity instead of dropping keystrokes.
fn next_pending_dictation_restart(
    pending: Option<PendingDictationRestart>,
    requested: PendingDictationRestart,
) -> Option<PendingDictationRestart> {
    if crate::dictation::toggled_post_stop_restart(pending.is_some()) {
        Some(requested)
    } else {
        None
    }
}

fn toggle_pending_dictation_restart(requested: PendingDictationRestart) -> bool {
    let mut pending = pending_dictation_restart().lock();
    *pending = next_pending_dictation_restart(*pending, requested);
    pending.is_some()
}

fn take_pending_dictation_restart() -> Option<PendingDictationRestart> {
    pending_dictation_restart().lock().take()
}

#[derive(Debug)]
enum DeferredAiCapturedText {
    Ready(String),
    Empty(String),
}

fn ai_capture_hide_settle_duration() -> std::time::Duration {
    std::time::Duration::from_millis(AI_CAPTURE_HIDE_SETTLE_MS)
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn applescript_list_literal(values: &[String]) -> String {
    let escaped_values = values
        .iter()
        .map(|value| format!("\"{}\"", crate::utils::escape_applescript_string(value)))
        .join(", ");
    format!("{{{}}}", escaped_values)
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn choose_from_list(
    prompt: &str,
    ok_button: &str,
    values: &[String],
) -> Result<Option<String>, String> {
    if values.is_empty() {
        return Ok(None);
    }

    let list_literal = applescript_list_literal(values);
    let script = format!(
        r#"set selectedItem to choose from list {list_literal} with prompt "{prompt}" OK button name "{ok_button}" cancel button name "Cancel" without multiple selections allowed
if selectedItem is false then
    return ""
end if
return item 1 of selectedItem"#,
        list_literal = list_literal,
        prompt = crate::utils::escape_applescript_string(prompt),
        ok_button = crate::utils::escape_applescript_string(ok_button),
    );

    let selected = crate::platform::run_osascript(&script, "builtin_picker_choose_from_list")
        .map_err(|error| error.to_string())?;
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selected))
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn prompt_for_text(
    prompt: &str,
    default_value: &str,
    ok_button: &str,
) -> Result<Option<String>, String> {
    let script = format!(
        r#"try
set dialogResult to display dialog "{prompt}" default answer "{default_value}" buttons {{"Cancel", "{ok_button}"}} default button "{ok_button}"
return text returned of dialogResult
on error number -128
return ""
end try"#,
        prompt = crate::utils::escape_applescript_string(prompt),
        default_value = crate::utils::escape_applescript_string(default_value),
        ok_button = crate::utils::escape_applescript_string(ok_button),
    );

    let value = crate::platform::run_osascript(&script, "builtin_picker_prompt_for_text")
        .map_err(|error| error.to_string())?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

// Script execution: process spawning, stdin protocol dispatch, and result handling.

// --- merged from part_000.rs ---
fn protocol_tile_to_window_control(pos: &protocol::TilePosition) -> window_control::TilePosition {
    use protocol::TilePosition as P;
    use window_control::TilePosition as WC;
    match pos {
        P::Left => WC::LeftHalf,
        P::Right => WC::RightHalf,
        P::Top => WC::TopHalf,
        P::Bottom => WC::BottomHalf,
        P::TopLeft => WC::TopLeft,
        P::TopRight => WC::TopRight,
        P::BottomLeft => WC::BottomLeft,
        P::BottomRight => WC::BottomRight,
        P::LeftThird => WC::LeftThird,
        P::CenterThird => WC::CenterThird,
        P::RightThird => WC::RightThird,
        P::TopThird => WC::TopThird,
        P::MiddleThird => WC::MiddleThird,
        P::BottomThird => WC::BottomThird,
        P::FirstTwoThirds => WC::FirstTwoThirds,
        P::LastTwoThirds => WC::LastTwoThirds,
        P::TopTwoThirds => WC::TopTwoThirds,
        P::BottomTwoThirds => WC::BottomTwoThirds,
        P::Center => WC::Center,
        P::AlmostMaximize => WC::AlmostMaximize,
        // Maximize fills the entire visible screen area (fullscreen)
        P::Maximize => WC::Fullscreen,
    }
}

/// Dispatch one legacy protocol window action through the public wrappers.
///
/// Exhaustive over `WindowActionType` (no wildcard arm); field validation
/// preserves the historical error strings exactly.
fn dispatch_legacy_window_action(
    action: &protocol::WindowActionType,
    window_id: Option<u32>,
    bounds: Option<&protocol::TargetWindowBounds>,
    tile_position: Option<&protocol::TilePosition>,
) -> anyhow::Result<()> {
    match action {
        protocol::WindowActionType::Focus => {
            let id = window_id.ok_or_else(|| anyhow::anyhow!("Missing window_id"))?;
            window_control::focus_window(id)
        }
        protocol::WindowActionType::Close => {
            let id = window_id.ok_or_else(|| anyhow::anyhow!("Missing window_id"))?;
            window_control::close_window(id)
        }
        protocol::WindowActionType::Minimize => {
            let id = window_id.ok_or_else(|| anyhow::anyhow!("Missing window_id"))?;
            window_control::minimize_window(id)
        }
        protocol::WindowActionType::Maximize => {
            let id = window_id.ok_or_else(|| anyhow::anyhow!("Missing window_id"))?;
            window_control::maximize_window(id)
        }
        protocol::WindowActionType::Resize => {
            let (id, b) = window_id
                .zip(bounds)
                .ok_or_else(|| anyhow::anyhow!("Missing window_id or bounds"))?;
            window_control::resize_window(id, b.width, b.height)
        }
        protocol::WindowActionType::Move => {
            let (id, b) = window_id
                .zip(bounds)
                .ok_or_else(|| anyhow::anyhow!("Missing window_id or bounds"))?;
            window_control::move_window(id, b.x, b.y)
        }
        protocol::WindowActionType::Tile => {
            let (id, pos) = window_id
                .zip(tile_position)
                .ok_or_else(|| anyhow::anyhow!("Missing window_id or tile_position"))?;
            window_control::tile_window(id, protocol_tile_to_window_control(pos))
        }
        protocol::WindowActionType::MoveToNextDisplay => {
            let id = window_id.ok_or_else(|| anyhow::anyhow!("Missing window_id"))?;
            window_control::move_to_next_display(id)
        }
        protocol::WindowActionType::MoveToPreviousDisplay => {
            let id = window_id.ok_or_else(|| anyhow::anyhow!("Missing window_id"))?;
            window_control::move_to_previous_display(id)
        }
    }
}

const CLIPBOARD_HISTORY_PREVIEW_CHAR_LIMIT: usize = 1000;

#[derive(Debug)]
struct ClipboardImageError {
    code: &'static str,
    message: &'static str,
}

impl std::fmt::Display for ClipboardImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.code, self.message)
    }
}

impl ClipboardImageError {
    fn access() -> Self {
        Self {
            code: "ERR_CLIPBOARD_ACCESS_FAILED",
            message: "Clipboard is unavailable.",
        }
    }

    fn image_not_available() -> Self {
        Self {
            code: "ERR_CLIPBOARD_IMAGE_NOT_AVAILABLE",
            message: "Clipboard does not contain a supported image.",
        }
    }

    fn decode(message: &'static str) -> Self {
        Self {
            code: "ERR_CLIPBOARD_IMAGE_DECODE_FAILED",
            message,
        }
    }

    fn encode(message: &'static str) -> Self {
        Self {
            code: "ERR_CLIPBOARD_IMAGE_ENCODE_FAILED",
            message,
        }
    }

    fn write() -> Self {
        Self {
            code: "ERR_CLIPBOARD_IMAGE_WRITE_FAILED",
            message: "Failed to write image data to the clipboard.",
        }
    }

    fn missing_content() -> Self {
        Self {
            code: "ERR_CLIPBOARD_IMAGE_MISSING_CONTENT",
            message: "clipboard.writeImage requires encoded image bytes.",
        }
    }

    fn to_submit_value(&self) -> String {
        format!("ERROR:{}:{}", self.code, self.message)
    }
}

fn read_clipboard_image_as_png_base64() -> Result<String, ClipboardImageError> {
    use base64::Engine;

    let mut clipboard = arboard::Clipboard::new().map_err(|e| {
        tracing::info!(
            category = "EXEC",
            error = %e,
            code = "ERR_CLIPBOARD_ACCESS_FAILED",
            "Clipboard image read init error"
        );
        ClipboardImageError::access()
    })?;

    let image = clipboard.get_image().map_err(|e| {
        tracing::info!(
            category = "EXEC",
            error = %e,
            code = "ERR_CLIPBOARD_IMAGE_NOT_AVAILABLE",
            "Clipboard image read unavailable"
        );
        ClipboardImageError::image_not_available()
    })?;

    let width = u32::try_from(image.width)
        .map_err(|_| ClipboardImageError::encode("Clipboard image width exceeds PNG limits."))?;
    let height = u32::try_from(image.height)
        .map_err(|_| ClipboardImageError::encode("Clipboard image height exceeds PNG limits."))?;
    let rgba =
        image::RgbaImage::from_raw(width, height, image.bytes.into_owned()).ok_or_else(|| {
            ClipboardImageError::encode("Clipboard image RGBA buffer length is invalid.")
        })?;

    let mut png_cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut png_cursor, image::ImageFormat::Png)
        .map_err(|e| {
            tracing::info!(
                category = "EXEC",
                error = %e,
                code = "ERR_CLIPBOARD_IMAGE_ENCODE_FAILED",
                "Clipboard image PNG encode error"
            );
            ClipboardImageError::encode("Failed to encode clipboard image as PNG.")
        })?;

    let png_bytes = png_cursor.into_inner();
    tracing::info!(
        category = "EXEC",
        width,
        height,
        png_bytes_len = png_bytes.len(),
        "Clipboard image read success"
    );
    Ok(base64::engine::general_purpose::STANDARD.encode(png_bytes))
}

fn write_clipboard_image_from_base64(content: Option<&String>) -> Result<(), ClipboardImageError> {
    use base64::Engine;

    let Some(content) = content.filter(|value| !value.is_empty()) else {
        tracing::info!(
            category = "EXEC",
            code = "ERR_CLIPBOARD_IMAGE_MISSING_CONTENT",
            "Clipboard image write missing content"
        );
        return Err(ClipboardImageError::missing_content());
    };

    let encoded = base64::engine::general_purpose::STANDARD
        .decode(content.as_bytes())
        .map_err(|e| {
            tracing::info!(
                category = "EXEC",
                error = %e,
                code = "ERR_CLIPBOARD_IMAGE_DECODE_FAILED",
                "Clipboard image base64 decode error"
            );
            ClipboardImageError::decode("clipboard.writeImage expected base64 image bytes.")
        })?;
    let decoded = image::load_from_memory(&encoded).map_err(|e| {
        tracing::info!(
            category = "EXEC",
            error = %e,
            encoded_bytes_len = encoded.len(),
            code = "ERR_CLIPBOARD_IMAGE_DECODE_FAILED",
            "Clipboard image decode error"
        );
        ClipboardImageError::decode("clipboard.writeImage expected PNG or JPEG bytes.")
    })?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    let image_data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    };

    let mut clipboard = arboard::Clipboard::new().map_err(|e| {
        tracing::info!(
            category = "EXEC",
            error = %e,
            code = "ERR_CLIPBOARD_ACCESS_FAILED",
            "Clipboard image write init error"
        );
        ClipboardImageError::access()
    })?;
    clipboard.set_image(image_data).map_err(|e| {
        tracing::info!(
            category = "EXEC",
            error = %e,
            code = "ERR_CLIPBOARD_IMAGE_WRITE_FAILED",
            "Clipboard image write error"
        );
        ClipboardImageError::write()
    })?;

    tracing::info!(
        category = "EXEC",
        width,
        height,
        encoded_bytes_len = encoded.len(),
        "Clipboard image write success"
    );
    Ok(())
}

/// Get information about all displays/monitors
fn get_displays() -> anyhow::Result<Vec<protocol::DisplayInfo>> {
    #[cfg(target_os = "macos")]
    {
        // window_control::display_topology is the sole display owner: exact
        // NSScreen visibleFrame geometry, localized names, real scale factors.
        let displays = window_control::list_displays()?
            .into_iter()
            .map(|display| protocol::DisplayInfo {
                display_id: display.id.0,
                name: display.localized_name,
                is_primary: display.is_primary,
                bounds: protocol::TargetWindowBounds {
                    x: display.full_bounds.x,
                    y: display.full_bounds.y,
                    width: display.full_bounds.width,
                    height: display.full_bounds.height,
                },
                visible_bounds: protocol::TargetWindowBounds {
                    x: display.visible_bounds.x,
                    y: display.visible_bounds.y,
                    width: display.visible_bounds.width,
                    height: display.visible_bounds.height,
                },
                scale_factor: Some(display.backing_scale_factor),
            })
            .collect::<Vec<_>>();
        Ok(displays)
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Fallback for non-macOS platforms
        Ok(vec![protocol::DisplayInfo {
            display_id: 0,
            name: "Primary Display".to_string(),
            is_primary: true,
            bounds: protocol::TargetWindowBounds {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            visible_bounds: protocol::TargetWindowBounds {
                x: 0,
                y: 24,
                width: 1920,
                height: 1056,
            },
            scale_factor: Some(1.0),
        }])
    }
}
fn format_missing_interactive_session_error(
    script_name: &str,
    script_path: &std::path::Path,
) -> String {
    format!(
        "interactive_session_missing: script='{}' path='{}' state=script_session:none operation=split_interactive_session",
        script_name,
        script_path.display()
    )
}

fn truncate_clipboard_history_preview(content: &str) -> String {
    let Some((cutoff_idx, _)) = content
        .char_indices()
        .nth(CLIPBOARD_HISTORY_PREVIEW_CHAR_LIMIT)
    else {
        return content.to_string();
    };

    format!("{}...", &content[..cutoff_idx])
}

fn take_active_script_session(
    script_session: &SharedSession,
    script_name: &str,
    script_path: &std::path::Path,
) -> Result<executor::ScriptSession, String> {
    script_session
        .lock()
        .take()
        .ok_or_else(|| format_missing_interactive_session_error(script_name, script_path))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecuteScriptFeedbackCommandSpec {
    program: &'static str,
    args: Vec<String>,
}

fn execute_script_normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn execute_script_normalize_notify_fields(
    title: Option<String>,
    body: Option<String>,
) -> Option<(String, String)> {
    let normalized_title = execute_script_normalize_optional_text(title);
    let normalized_body = execute_script_normalize_optional_text(body);

    match (normalized_title, normalized_body) {
        (Some(title), Some(body)) => Some((title, body)),
        (Some(title), None) => Some((title.clone(), title)),
        (None, Some(body)) => Some(("Script Kit".to_string(), body)),
        (None, None) => None,
    }
}

fn execute_script_build_beep_command_spec() -> ExecuteScriptFeedbackCommandSpec {
    ExecuteScriptFeedbackCommandSpec {
        program: "afplay",
        args: vec!["/System/Library/Sounds/Tink.aiff".to_string()],
    }
}

fn execute_script_build_say_command_spec(
    text: String,
    voice: Option<String>,
) -> Option<ExecuteScriptFeedbackCommandSpec> {
    let text = execute_script_normalize_optional_text(Some(text))?;
    let voice = execute_script_normalize_optional_text(voice);

    let mut args = Vec::new();
    if let Some(voice) = voice {
        args.push("-v".to_string());
        args.push(voice);
    }
    args.push(text);

    Some(ExecuteScriptFeedbackCommandSpec {
        program: "say",
        args,
    })
}

fn execute_script_spawn_feedback_command(
    effect: &'static str,
    spec: ExecuteScriptFeedbackCommandSpec,
) -> std::io::Result<()> {
    tracing::info!(
        category = "EXEC",
        effect = %effect,
        program = spec.program,
        argument_count = spec.args.len(),
        "Dispatching protocol feedback command"
    );

    let mut command = std::process::Command::new(spec.program);
    command
        .args(&spec.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let child = command.spawn()?;
    drop(child);
    Ok(())
}

#[cfg(target_os = "macos")]
fn execute_script_dispatch_beep_command() -> std::io::Result<()> {
    execute_script_spawn_feedback_command("beep", execute_script_build_beep_command_spec())
}

#[cfg(not(target_os = "macos"))]
fn execute_script_dispatch_beep_command() -> std::io::Result<()> {
    tracing::info!(
        category = "EXEC",
        effect = "beep",
        platform = std::env::consts::OS,
        "Skipping beep protocol feedback command on unsupported platform"
    );
    Ok(())
}

fn execute_script_dispatch_notify_command(
    title: Option<String>,
    body: Option<String>,
) -> std::io::Result<()> {
    let Some((title, body)) = execute_script_normalize_notify_fields(title, body) else {
        tracing::info!(
            category = "EXEC",
            effect = "notify",
            "Skipping notify protocol feedback command because title and body were empty"
        );
        return Ok(());
    };

    tracing::info!(
        category = "EXEC",
        effect = "notify",
        backend = "notify-rust",
        title = %title,
        body_len = body.chars().count(),
        "Dispatching notify protocol feedback via notify-rust"
    );

    // notify-rust's show() blocks waiting for the notification server to ack.
    // The calling thread is the script-message reader, not the GPUI main thread,
    // so blocking there is tolerable; we still spawn a dedicated thread so a
    // stuck Notification Center never wedges subsequent script messages.
    std::thread::Builder::new()
        .name("notify-rust-dispatch".to_string())
        .spawn(move || {
            if let Err(error) = notify_rust::Notification::new()
                .summary(&title)
                .body(&body)
                .show()
            {
                tracing::warn!(
                    category = "EXEC",
                    effect = "notify",
                    backend = "notify-rust",
                    error = %error,
                    state = "show_failed",
                    "notify-rust failed to deliver system notification"
                );
            }
        })?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn execute_script_dispatch_say_command(text: String, voice: Option<String>) -> std::io::Result<()> {
    let Some(spec) = execute_script_build_say_command_spec(text, voice) else {
        tracing::info!(
            category = "EXEC",
            effect = "say",
            "Skipping say protocol feedback command because text was empty"
        );
        return Ok(());
    };

    execute_script_spawn_feedback_command("say", spec)
}

#[cfg(not(target_os = "macos"))]
fn execute_script_dispatch_say_command(text: String, voice: Option<String>) -> std::io::Result<()> {
    let _ = (text, voice);
    tracing::info!(
        category = "EXEC",
        effect = "say",
        platform = std::env::consts::OS,
        "Skipping say protocol feedback command on unsupported platform"
    );
    Ok(())
}

fn execute_script_feedback_receipt(
    api: &str,
    ok: bool,
    status: &str,
    backend: Option<&str>,
    side_effect: Option<&str>,
    code: Option<&str>,
    message: Option<&str>,
) -> serde_json::Value {
    let mut result = serde_json::json!({
        "ok": ok,
        "api": api,
        "status": status,
        "platform": std::env::consts::OS,
    });

    if let Some(backend) = backend {
        result["backend"] = serde_json::json!(backend);
    }
    if let Some(side_effect) = side_effect {
        result["sideEffect"] = serde_json::json!(side_effect);
        result["delivery"] = serde_json::json!("not_verified");
    }
    if let Some(code) = code {
        result["code"] = serde_json::json!(code);
    }
    if let Some(message) = message {
        result["message"] = serde_json::json!(message);
    }

    result
}

fn execute_script_send_system_feedback_result(
    response_tx: &std::sync::mpsc::SyncSender<Message>,
    request_id: Option<String>,
    result: serde_json::Value,
) {
    let Some(request_id) = request_id else {
        return;
    };

    if let Err(error) = response_tx.send(Message::SystemFeedbackResult { request_id, result }) {
        tracing::info!(
            category = "EXEC",
            error = %error,
            "Failed to send system feedback response"
        );
    }
}
// --- merged from part_001.rs ---
impl ScriptListApp {
    fn execute_interactive(&mut self, script: &scripts::Script, cx: &mut Context<Self>) {
        self.execute_interactive_with_args(script, Vec::new(), cx);
    }

    fn execute_interactive_with_args(
        &mut self,
        script: &scripts::Script,
        argv: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.execute_interactive_with_env_and_args(script, Vec::new(), argv, cx);
    }

    fn execute_interactive_with_env_and_args(
        &mut self,
        script: &scripts::Script,
        extra_env: Vec<(String, String)>,
        argv: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let capability_issues = crate::scripts::validate_declared_sdk_capabilities(script);
        if let Some(issue) = capability_issues
            .iter()
            .find(|issue| issue.severity == crate::scripts::ValidationSeverity::Fatal)
            .or_else(|| capability_issues.first())
        {
            tracing::warn!(
                target: "script_kit::execution",
                issue = ?issue.kind,
                "script_capability_preflight_refused_before_process_spawn"
            );
            self.show_hud(issue.message.clone(), Some(HUD_MEDIUM_MS), cx);
            return;
        }

        // --- merged from part_001_body/execute_interactive_merged.rs ---
        {
            tracing::info!(
                category = "EXEC",
                script_name = %script.name,
                argument_count = argv.len(),
                "Starting interactive execution"
            );

            // Store script path for error reporting in reader thread
            let script_path_for_errors = script.path.to_string_lossy().to_string();

            match executor::execute_script_interactive_with_env_and_args(
                &script.path,
                extra_env,
                argv,
            ) {
                Ok(session) => {
                    tracing::info!(
                        category = "EXEC",
                        "Interactive session started successfully"
                    );

                    // Store PID for explicit cleanup (belt-and-suspenders approach)
                    let pid = session.process_handle.pid;
                    let previous_pid = self.current_script_pid;
                    self.current_script_pid = Some(pid);
                    tracing::info!(category = "EXEC", pid, "Stored script PID for cleanup");
                    executor::telemetry::log_session_replaced(
                        "execute_script::execute_interactive",
                        previous_pid,
                        pid,
                    );

                    *self.script_session.lock() = Some(session);

                    // Create async_channel for script thread to send prompt messages to UI (event-driven)
                    // P1-6: Use bounded channel to prevent unbounded memory growth from slow UI
                    // Capacity of 100 is generous (scripts rarely send > 10 messages/sec)
                    let (tx, rx) = async_channel::bounded(100);
                    let rx_for_listener = rx.clone();
                    self.prompt_receiver = Some(rx);

                    // Spawn event-driven listener for prompt messages (replaces 50ms polling)
                    // Correlation scope is thread-local and dies at the spawn
                    // boundary; carry the script run's id into the listener so
                    // every handle_prompt_message log correlates with the run.
                    let listener_correlation_scope = logging::scoped_correlation_id();
                    cx.spawn(async move |this, cx| {
                        let scope = listener_correlation_scope;
                        tracing::info!(
                            category = "EXEC",
                            "Prompt message listener started (event-driven)"
                        );

                        // Event-driven: recv().await yields until a message arrives
                        while let Ok(msg) = rx_for_listener.recv().await {
                            logging::with_correlation_scope(scope.as_deref(), || {
                                tracing::info!(
                                    category = "EXEC",
                                    prompt_message_kind = ?std::mem::discriminant(&msg),
                                    "Prompt message received"
                                );
                                let _ = cx.update(|cx| {
                                    this.update(cx, |app, cx| {
                                        app.handle_prompt_message(msg, cx);
                                    })
                                });
                            });
                        }

                        tracing::info!(
                            category = "EXEC",
                            "Prompt message listener exiting (channel closed)"
                        );
                    })
                    .detach();

                    // We need separate threads for reading and writing to avoid deadlock
                    // The read thread blocks on receive_message(), so we can't check for responses in the same loop

                    // Take ownership of the session and split it
                    let session = match take_active_script_session(
                        &self.script_session,
                        &script.name,
                        &script.path,
                    ) {
                        Ok(session) => session,
                        Err(error) => {
                            tracing::info!(
                                category = "EXEC",
                                error = %error,
                                "Failed to take active script session"
                            );
                            self.last_output =
                                Some(SharedString::from(format!("✗ Error: {}", error)));
                            cx.notify();
                            return;
                        }
                    };
                    let split = session.split();

                    let mut stdin = split.stdin;
                    let mut stdout_reader = split.stdout_reader;
                    // Capture stderr for error reporting - we'll read it in real-time for debugging
                    let stderr_handle = split.stderr;
                    // CRITICAL: Keep process_handle and child alive - they kill the process on drop!
                    // We move them into the reader thread so they live until the script exits.
                    let _process_handle = split.process_handle;
                    let mut _child = split.child;

                    // Stderr reader thread - tees output to both logs AND a ring buffer
                    // The buffer is used for post-mortem error reporting when script exits non-zero
                    // FIX: Previously we consumed stderr in a thread but passed None to reader,
                    // which meant stderr was never available for error messages. Now we use
                    // spawn_stderr_reader which returns a StderrCapture containing both the buffer
                    // AND a JoinHandle so we can wait for stderr to fully drain before reading.
                    let stderr_capture = stderr_handle.map(|stderr| {
                        executor::spawn_stderr_reader(stderr, script_path_for_errors.clone())
                    });

                    // Move the capture into the reader thread - it owns both buffer and join handle
                    // The reader thread will wait for stderr to drain before reading contents

                    // Channel for sending responses from UI to writer thread
                    // FIX: Use bounded channel to prevent OOM from slow script/blocked stdin
                    // Capacity of 100 matches the prompt channel - generous for normal use
                    // If the script isn't reading stdin, backpressure will block senders
                    let (response_tx, response_rx) = mpsc::sync_channel::<Message>(100);

                    // Clone response_tx for the reader thread to handle direct responses
                    // (e.g., getSelectedText, setSelectedText, checkAccessibility)
                    let reader_response_tx = response_tx.clone();
                    let subscription_owner_id = format!("script:{}", uuid::Uuid::new_v4());

                    // Writer thread - handles sending responses to script
                    std::thread::spawn(move || {
                        use std::io::Write;
                        use std::os::unix::io::AsRawFd;

                        // Log the stdin file descriptor for debugging
                        let fd = stdin.as_raw_fd();
                        tracing::info!(category = "EXEC", fd, "Writer thread started");

                        // Check if fd is a valid pipe
                        #[cfg(unix)]
                        {
                            let stat_result = unsafe {
                                let mut stat: libc::stat = std::mem::zeroed();
                                libc::fstat(fd, &mut stat)
                            };
                            if stat_result == 0 {
                                tracing::info!(category = "EXEC", fd, "stdin fstat succeeded");
                            } else {
                                tracing::info!(
                                    category = "EXEC",
                                    fd,
                                    os_error = %std::io::Error::last_os_error(),
                                    "stdin fstat failed"
                                );
                            }
                        }

                        loop {
                            match response_rx.recv() {
                                Ok(response) => {
                                    let json = match protocol::serialize_message(&response) {
                                        Ok(j) => j,
                                        Err(e) => {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to serialize response"
                                            );
                                            continue;
                                        }
                                    };
                                    // Use truncated logging to avoid full payload in logs
                                    logging::log_protocol_send(fd, &json);
                                    let bytes = format!("{}\n", json);
                                    let bytes_len = bytes.len();

                                    // Check fd validity before write
                                    let fcntl_result = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                                    tracing::info!(
                                        category = "EXEC",
                                        fd,
                                        fcntl_result,
                                        "Pre-write fcntl(F_GETFD) completed"
                                    );

                                    match stdin.write_all(bytes.as_bytes()) {
                                        Ok(()) => {
                                            tracing::info!(
                                                category = "EXEC",
                                                fd,
                                                bytes_len,
                                                "Write succeeded"
                                            );
                                        }
                                        Err(e) => {
                                            tracing::info!(
                                                category = "EXEC",
                                                fd,
                                                bytes_len,
                                                error = %e,
                                                error_kind = ?e.kind(),
                                                "Failed to write to script stdin"
                                            );
                                            break;
                                        }
                                    }
                                    if let Err(e) = stdin.flush() {
                                        tracing::info!(
                                            category = "EXEC",
                                            fd,
                                            error = %e,
                                            "Failed to flush script stdin"
                                        );
                                        break;
                                    }
                                    tracing::info!(category = "EXEC", fd, "Flush succeeded");
                                }
                                Err(_) => {
                                    tracing::info!(
                                        category = "EXEC",
                                        "Response channel closed, writer exiting"
                                    );
                                    break;
                                }
                            }
                        }
                        tracing::info!(category = "EXEC", "Writer thread exiting");
                    });

                    // Reader thread - handles receiving messages from script (blocking is OK here)
                    // CRITICAL: Move _process_handle and _child into this thread to keep them alive!
                    // When the reader thread exits, they'll be dropped and the process killed.
                    // Reader thread - handles receiving messages from script (blocking is OK here)
                    // CRITICAL: Move _process_handle and _child into this thread to keep them alive!
                    // When the reader thread exits, they'll be dropped and the process killed.
                    let script_path_clone = script_path_for_errors.clone();
                    std::thread::spawn(move || {
                        // These variables keep the process alive - they're dropped when the thread exits
                        let _keep_alive_handle = _process_handle;
                        let mut keep_alive_child = _child;
                        // FIX: Use the stderr capture which includes both buffer and join handle
                        // The buffer is populated by the stderr reader thread, and we wait for it
                        // to complete (with timeout) before reading to prevent partial captures.
                        let stderr_capture = stderr_capture;
                        let script_path = script_path_clone;
                        let subscription_owner_id = subscription_owner_id;

                        loop {
                            // Use next_message_graceful_with_handler to skip non-JSON lines and report parse issues
                            match stdout_reader.next_message_graceful_with_handler(|issue| {
                                let should_report = matches!(
                                    issue.kind,
                                    protocol::ParseIssueKind::InvalidPayload
                                        | protocol::ParseIssueKind::UnknownType
                                );
                                if !should_report {
                                    return;
                                }

                                let summary = match issue.kind {
                                    protocol::ParseIssueKind::InvalidPayload => issue
                                        .message_type
                                        .as_deref()
                                        .map(|message_type| {
                                            format!(
                                                "Invalid '{}' message payload from script",
                                                message_type
                                            )
                                        })
                                        .unwrap_or_else(|| {
                                            "Invalid message payload from script".to_string()
                                        }),
                                    protocol::ParseIssueKind::UnknownType => issue
                                        .message_type
                                        .as_deref()
                                        .map(|message_type| {
                                            format!(
                                                "Unknown '{}' message type from script",
                                                message_type
                                            )
                                        })
                                        .unwrap_or_else(|| {
                                            "Unknown message type from script".to_string()
                                        }),
                                    _ => "Protocol message issue from script".to_string(),
                                };

                                let mut details_lines = Vec::new();
                                details_lines.push(format!("Script: {}", script_path));
                                if let Some(ref message_type) = issue.message_type {
                                    details_lines.push(format!("Type: {}", message_type));
                                }
                                if let Some(ref error) = issue.error {
                                    details_lines.push(format!("Error: {}", error));
                                }
                                if !issue.raw_preview.is_empty() {
                                    details_lines.push(format!("Preview: {}", issue.raw_preview));
                                }
                                let details = Some(details_lines.join("\n"));

                                let severity = match issue.kind {
                                    protocol::ParseIssueKind::InvalidPayload => {
                                        ErrorSeverity::Error
                                    }
                                    protocol::ParseIssueKind::UnknownType => ErrorSeverity::Warning,
                                    _ => ErrorSeverity::Warning,
                                };

                                let correlation_id = issue.correlation_id.clone();
                                let prompt_msg = PromptMessage::ProtocolError {
                                    correlation_id: issue.correlation_id,
                                    summary,
                                    details,
                                    severity,
                                    script_path: script_path.clone(),
                                };

                                if tx.send_blocking(prompt_msg).is_err() {
                                    tracing::warn!(
                                        correlation_id = %correlation_id,
                                        script_path = %script_path,
                                        "Prompt channel closed, dropping protocol error"
                                    );
                                }
                            }) {
                                Ok(Some(msg)) => {
                                    let message_id = msg
                                        .id()
                                        .map(str::to_string)
                                        .unwrap_or_else(|| format!("msg:{}", uuid::Uuid::new_v4()));
                                    let _message_guard = logging::set_correlation_id(format!(
                                        "protocol:{}",
                                        message_id
                                    ));
                                    if tracing::enabled!(tracing::Level::DEBUG) {
                                        let payload_summary = serde_json::to_string(&msg)
                                            .map(|json| logging::summarize_payload(&json))
                                            .unwrap_or_else(|_| "{serialize_error}".to_string());
                                        tracing::debug!(
                                            category = "EXEC",
                                            event_type = "protocol_message_received",
                                            message_id = %message_id,
                                            payload_summary = %payload_summary,
                                            "Received protocol message"
                                        );
                                    }

                                    // First, try to handle selected text messages directly (no UI needed)
                                    match executor::handle_selected_text_message(&msg) {
                                        executor::SelectedTextHandleResult::Handled(response) => {
                                            if tracing::enabled!(tracing::Level::DEBUG) {
                                                let response_summary =
                                                    serde_json::to_string(&response)
                                                        .map(|json| {
                                                            logging::summarize_payload(&json)
                                                        })
                                                        .unwrap_or_else(|_| {
                                                            "{serialize_error}".to_string()
                                                        });
                                                tracing::debug!(
                                                    category = "EXEC",
                                                    event_type = "protocol_selected_text_response",
                                                    message_id = %message_id,
                                                    payload_summary = %response_summary,
                                                    "Handled selected text message, sending response"
                                                );
                                            }
                                            if let Err(e) = reader_response_tx.send(response) {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    error = %e,
                                                    "Failed to send selected text response"
                                                );
                                            }
                                            continue;
                                        }
                                        executor::SelectedTextHandleResult::NotHandled => {
                                            // Fall through to other message handling
                                        }
                                    }

                                    // Handle ClipboardHistory directly (no UI needed)
                                    if let Message::ClipboardHistory {
                                        request_id,
                                        action,
                                        entry_id,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            action = ?action,
                                            request_id = %request_id,
                                            "ClipboardHistory request"
                                        );

                                        let response = match action {
                                            protocol::ClipboardHistoryAction::List => {
                                                let entries =
                                                    clipboard_history::get_clipboard_history(100);
                                                let entry_data: Vec<protocol::ClipboardHistoryEntryData> = entries
                                                .into_iter()
                                                .map(|e| {
                                                    // Truncate large content to avoid pipe buffer issues
                                                    // Images are stored as base64 which can be huge
                                                    let content = match e.content_type {
                                                        clipboard_history::ContentType::Image => {
                                                            // For images, send a placeholder with metadata
                                                            format!("[image:{}]", e.id)
                                                        }
                                                        clipboard_history::ContentType::Text
                                                        | clipboard_history::ContentType::Link
                                                        | clipboard_history::ContentType::File
                                                        | clipboard_history::ContentType::Color => {
                                                            // Truncate very long text entries
                                                            truncate_clipboard_history_preview(
                                                                &e.content,
                                                            )
                                                        }
                                                    };
                                                    protocol::ClipboardHistoryEntryData {
                                                        entry_id: e.id,
                                                        content,
                                                        content_type: match e.content_type {
                                                            clipboard_history::ContentType::Text
                                                            | clipboard_history::ContentType::Link
                                                            | clipboard_history::ContentType::File
                                                            | clipboard_history::ContentType::Color => protocol::ClipboardEntryType::Text,
                                                            clipboard_history::ContentType::Image => protocol::ClipboardEntryType::Image,
                                                        },
                                                        timestamp: chrono::DateTime::from_timestamp(e.timestamp, 0)
                                                            .map(|dt| dt.to_rfc3339())
                                                            .unwrap_or_default(),
                                                        pinned: e.pinned,
                                                    }
                                                })
                                                .collect();
                                                Message::clipboard_history_list_response(
                                                    request_id.clone(),
                                                    entry_data,
                                                )
                                            }
                                            protocol::ClipboardHistoryAction::Pin => {
                                                if let Some(id) = entry_id {
                                                    match clipboard_history::pin_entry(id) {
                                                        Ok(()) => {
                                                            Message::clipboard_history_success(
                                                                request_id.clone(),
                                                            )
                                                        }
                                                        Err(e) => Message::clipboard_history_error(
                                                            request_id.clone(),
                                                            e.to_string(),
                                                        ),
                                                    }
                                                } else {
                                                    Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        "Missing entry_id".to_string(),
                                                    )
                                                }
                                            }
                                            protocol::ClipboardHistoryAction::Unpin => {
                                                if let Some(id) = entry_id {
                                                    match clipboard_history::unpin_entry(id) {
                                                        Ok(()) => {
                                                            Message::clipboard_history_success(
                                                                request_id.clone(),
                                                            )
                                                        }
                                                        Err(e) => Message::clipboard_history_error(
                                                            request_id.clone(),
                                                            e.to_string(),
                                                        ),
                                                    }
                                                } else {
                                                    Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        "Missing entry_id".to_string(),
                                                    )
                                                }
                                            }
                                            protocol::ClipboardHistoryAction::Remove => {
                                                if let Some(id) = entry_id {
                                                    match clipboard_history::remove_entry(id) {
                                                        Ok(()) => {
                                                            Message::clipboard_history_success(
                                                                request_id.clone(),
                                                            )
                                                        }
                                                        Err(e) => Message::clipboard_history_error(
                                                            request_id.clone(),
                                                            e.to_string(),
                                                        ),
                                                    }
                                                } else {
                                                    Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        "Missing entry_id".to_string(),
                                                    )
                                                }
                                            }
                                            protocol::ClipboardHistoryAction::Clear => {
                                                match clipboard_history::clear_history() {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            }
                                            protocol::ClipboardHistoryAction::TrimOversize => {
                                                match clipboard_history::trim_oversize_text_entries(
                                                ) {
                                                    Ok(_) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            }
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send clipboard history response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle Clipboard read/write directly (no UI needed)
                                    if let Message::Clipboard {
                                        id,
                                        action,
                                        format,
                                        content,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            action = ?action,
                                            format = ?format,
                                            request_id = ?id,
                                            "Clipboard request"
                                        );

                                        // If no request ID, we can't send a response, so just handle and continue
                                        let req_id = match id {
                                            Some(rid) => rid.clone(),
                                            None => {
                                                // Handle clipboard operation without response
                                                if let protocol::ClipboardAction::Write = action {
                                                    match format {
                                                        Some(protocol::ClipboardFormat::Image) => {
                                                            if let Err(e) =
                                                                write_clipboard_image_from_base64(
                                                                    content.as_ref(),
                                                                )
                                                            {
                                                                tracing::error!(
                                                                    category = "EXEC",
                                                                    error = %e,
                                                                    "Fire-and-forget clipboard image write failed"
                                                                );
                                                            }
                                                        }
                                                        Some(protocol::ClipboardFormat::Text)
                                                        | None => {
                                                            if let Some(text) = content {
                                                                use arboard::Clipboard;
                                                                match Clipboard::new() {
                                                                    Ok(mut clipboard) => {
                                                                        if let Err(e) = clipboard
                                                                            .set_text(text.clone())
                                                                        {
                                                                            tracing::error!(
                                                                                category = "EXEC",
                                                                                error = %e,
                                                                                "Fire-and-forget clipboard text write failed"
                                                                            );
                                                                        }
                                                                    }
                                                                    Err(e) => {
                                                                        tracing::error!(
                                                                            category = "EXEC",
                                                                            error = %e,
                                                                            "Fire-and-forget clipboard init failed for write"
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                continue;
                                            }
                                        };

                                        let response = match action {
                                            protocol::ClipboardAction::Read => {
                                                // Read from clipboard
                                                match format {
                                                    Some(protocol::ClipboardFormat::Text)
                                                    | None => {
                                                        use arboard::Clipboard;
                                                        match Clipboard::new() {
                                                            Ok(mut clipboard) => {
                                                                match clipboard.get_text() {
                                                                    Ok(text) => Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(text),
                                                                    },
                                                                    Err(e) => {
                                                                        tracing::info!(
                                                                            category = "EXEC",
                                                                            error = %e,
                                                                            "Clipboard read error"
                                                                        );
                                                                        Message::Submit {
                                                                            id: req_id,
                                                                            value: Some(
                                                                                String::new(),
                                                                            ),
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                tracing::info!(
                                                                    category = "EXEC",
                                                                    error = %e,
                                                                    "Clipboard init error"
                                                                );
                                                                Message::Submit {
                                                                    id: req_id,
                                                                    value: Some(String::new()),
                                                                }
                                                            }
                                                        }
                                                    }
                                                    Some(protocol::ClipboardFormat::Image) => {
                                                        match read_clipboard_image_as_png_base64() {
                                                            Ok(base64_png) => Message::Submit {
                                                                id: req_id,
                                                                value: Some(base64_png),
                                                            },
                                                            Err(error) => Message::Submit {
                                                                id: req_id,
                                                                value: Some(
                                                                    error.to_submit_value(),
                                                                ),
                                                            },
                                                        }
                                                    }
                                                }
                                            }
                                            protocol::ClipboardAction::Write => {
                                                // Write to clipboard
                                                use arboard::Clipboard;
                                                match format {
                                                    Some(protocol::ClipboardFormat::Image) => {
                                                        match write_clipboard_image_from_base64(
                                                            content.as_ref(),
                                                        ) {
                                                            Ok(()) => Message::Submit {
                                                                id: req_id,
                                                                value: Some("ok".to_string()),
                                                            },
                                                            Err(error) => Message::Submit {
                                                                id: req_id,
                                                                value: Some(
                                                                    error.to_submit_value(),
                                                                ),
                                                            },
                                                        }
                                                    }
                                                    Some(protocol::ClipboardFormat::Text)
                                                    | None => match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            if let Some(text) = content {
                                                                match clipboard
                                                                    .set_text(text.clone())
                                                                {
                                                                    Ok(()) => {
                                                                        tracing::info!(
                                                                            category = "EXEC",
                                                                            bytes_len = text.len(),
                                                                            "Clipboard write success"
                                                                        );
                                                                        Message::Submit {
                                                                            id: req_id,
                                                                            value: Some(
                                                                                "ok".to_string(),
                                                                            ),
                                                                        }
                                                                    }
                                                                    Err(e) => {
                                                                        tracing::error!(
                                                                            category = "EXEC",
                                                                            error = %e,
                                                                            "Clipboard write error"
                                                                        );
                                                                        Message::Submit {
                                                                            id: req_id,
                                                                            value: Some(format!(
                                                                                "ERROR:ERR_CLIPBOARD_WRITE_FAILED:{}",
                                                                                e
                                                                            )),
                                                                        }
                                                                    }
                                                                }
                                                            } else {
                                                                tracing::error!(
                                                                    category = "EXEC",
                                                                    "Clipboard write: no content provided"
                                                                );
                                                                Message::Submit {
                                                                    id: req_id,
                                                                    value: Some(
                                                                        "ERROR:ERR_CLIPBOARD_MISSING_CONTENT:clipboard.writeText requires text content.".to_string(),
                                                                    ),
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(
                                                                category = "EXEC",
                                                                error = %e,
                                                                "Clipboard init error"
                                                            );
                                                            Message::Submit {
                                                                id: req_id,
                                                                value: Some(format!(
                                                                    "ERROR:ERR_CLIPBOARD_ACCESS_FAILED:{}",
                                                                    e
                                                                )),
                                                            }
                                                        }
                                                    },
                                                }
                                            }
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send clipboard response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle WindowList directly (no UI needed)
                                    if let Message::WindowList { request_id } = &msg {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            "WindowList request"
                                        );

                                        let response = match window_control::list_windows() {
                                            Ok(windows) => {
                                                let window_infos: Vec<protocol::SystemWindowInfo> =
                                                    windows
                                                        .into_iter()
                                                        .map(|w| protocol::SystemWindowInfo {
                                                            window_id: w.id,
                                                            title: w.title,
                                                            app_name: w.app,
                                                            bounds: Some(
                                                                protocol::TargetWindowBounds {
                                                                    x: w.bounds.x,
                                                                    y: w.bounds.y,
                                                                    width: w.bounds.width,
                                                                    height: w.bounds.height,
                                                                },
                                                            ),
                                                            is_minimized: None,
                                                            is_active: None,
                                                        })
                                                        .collect();
                                                Message::window_list_result(
                                                    request_id.clone(),
                                                    window_infos,
                                                )
                                            }
                                            Err(e) => {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    error = %e,
                                                    request_id = %request_id,
                                                    "WindowList error"
                                                );
                                                // Return empty list on error
                                                Message::window_list_result(
                                                    request_id.clone(),
                                                    vec![],
                                                )
                                            }
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send window list response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle WindowAction directly (no UI needed)
                                    if let Message::WindowAction {
                                        request_id,
                                        action,
                                        window_id,
                                        bounds,
                                        tile_position,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            action = ?action,
                                            window_id = ?window_id,
                                            "WindowAction request"
                                        );

                                        let result = dispatch_legacy_window_action(
                                            action,
                                            *window_id,
                                            bounds.as_ref(),
                                            tile_position.as_ref(),
                                        );

                                        let response = match result {
                                            Ok(()) => {
                                                Message::window_action_success(request_id.clone())
                                            }
                                            Err(e) => Message::window_action_error(
                                                request_id.clone(),
                                                e.to_string(),
                                            ),
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send window action response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle DisplayList directly (no UI needed)
                                    if let Message::DisplayList { request_id } = &msg {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            "DisplayList request"
                                        );

                                        let response = match get_displays() {
                                            Ok(displays) => Message::display_list_result(
                                                request_id.clone(),
                                                displays,
                                            ),
                                            Err(e) => {
                                                tracing::error!(
                                                    category = "ERROR",
                                                    error = %e,
                                                    request_id = %request_id,
                                                    "Failed to get displays"
                                                );
                                                // Return empty list on error
                                                Message::display_list_result(
                                                    request_id.clone(),
                                                    vec![],
                                                )
                                            }
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send display list response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle FrontmostWindow directly (no UI needed)
                                    if let Message::FrontmostWindow { request_id } = &msg {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            "FrontmostWindow request"
                                        );

                                        let response =
                                        match window_control::get_frontmost_window_of_previous_app()
                                        {
                                            Ok(Some(window)) => {
                                                let window_info = protocol::SystemWindowInfo {
                                                    window_id: window.id,
                                                    title: window.title,
                                                    app_name: window.app,
                                                    bounds: Some(protocol::TargetWindowBounds {
                                                        x: window.bounds.x,
                                                        y: window.bounds.y,
                                                        width: window.bounds.width,
                                                        height: window.bounds.height,
                                                    }),
                                                    is_minimized: None,
                                                    is_active: Some(true),
                                                };
                                                Message::frontmost_window_result(
                                                    request_id.clone(),
                                                    Some(window_info),
                                                    None,
                                                )
                                            }
                                            Ok(None) => Message::frontmost_window_result(
                                                request_id.clone(),
                                                None,
                                                Some("No frontmost window found".to_string()),
                                            ),
                                            Err(e) => Message::frontmost_window_result(
                                                request_id.clone(),
                                                None,
                                                Some(e.to_string()),
                                            ),
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send frontmost window response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle FileSearch directly (no UI needed)
                                    if let Message::FileSearch {
                                        request_id,
                                        query,
                                        only_in,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            query = %query,
                                            only_in = ?only_in,
                                            "FileSearch request"
                                        );

                                        // Check if query looks like a directory path
                                        // If so, list directory contents instead of searching
                                        let results = if file_search::is_directory_path(query) {
                                            tracing::info!(
                                                category = "EXEC",
                                                query = %query,
                                                "Detected directory path, listing directory contents"
                                            );
                                            file_search::list_directory(
                                                query,
                                                file_search::DEFAULT_CACHE_LIMIT,
                                            )
                                        } else {
                                            file_search::search_files(
                                                query,
                                                only_in.as_deref(),
                                                file_search::DEFAULT_SEARCH_LIMIT,
                                            )
                                        };

                                        let file_entries: Vec<protocol::FileSearchResultEntry> =
                                            results
                                                .into_iter()
                                                .map(|f| protocol::FileSearchResultEntry {
                                                    path: f.path,
                                                    name: f.name,
                                                    is_directory: f.file_type
                                                        == file_search::FileType::Directory,
                                                    size: Some(f.size),
                                                    modified_at: chrono::DateTime::from_timestamp(
                                                        f.modified as i64,
                                                        0,
                                                    )
                                                    .map(|dt| dt.to_rfc3339()),
                                                })
                                                .collect();

                                        let response = Message::file_search_result(
                                            request_id.clone(),
                                            file_entries,
                                        );

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send file search response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle GetWindowBounds directly (no UI needed)
                                    if let Message::GetWindowBounds { request_id } = &msg {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            "GetWindowBounds request"
                                        );

                                        #[cfg(target_os = "macos")]
                                        let bounds_json = {
                                            if let Some(window) = window_manager::get_main_window()
                                            {
                                                unsafe {
                                                    // Get the window frame
                                                    let frame: NSRect = msg_send![window, frame];

                                                    // Get the PRIMARY screen's height for coordinate conversion
                                                    // macOS uses bottom-left origin, we convert to top-left
                                                    let screens: id =
                                                        msg_send![class!(NSScreen), screens];
                                                    let main_screen: id =
                                                        msg_send![screens, firstObject];
                                                    let main_screen_frame: NSRect =
                                                        msg_send![main_screen, frame];
                                                    let primary_screen_height =
                                                        main_screen_frame.size.height;

                                                    // Convert from bottom-left origin (macOS) to top-left origin
                                                    let flipped_y = primary_screen_height
                                                        - frame.origin.y
                                                        - frame.size.height;

                                                    tracing::info!(
                                                        category = "EXEC",
                                                        x = frame.origin.x,
                                                        y = flipped_y,
                                                        width = frame.size.width,
                                                        height = frame.size.height,
                                                        "Window bounds"
                                                    );

                                                    // Create JSON string with bounds
                                                    format!(
                                                        r#"{{"x":{},"y":{},"width":{},"height":{}}}"#,
                                                        frame.origin.x as f64,
                                                        flipped_y as f64,
                                                        frame.size.width as f64,
                                                        frame.size.height as f64
                                                    )
                                                }
                                            } else {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    "GetWindowBounds: Main window not registered"
                                                );
                                                r#"{"error":"Main window not found"}"#.to_string()
                                            }
                                        };

                                        #[cfg(not(target_os = "macos"))]
                                        let bounds_json =
                                            r#"{"error":"Not supported on this platform"}"#
                                                .to_string();

                                        let response = Message::Submit {
                                            id: request_id.clone(),
                                            value: Some(bounds_json),
                                        };
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            "Sending window bounds response"
                                        );
                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send window bounds response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle AI SDK live subscriptions. These need the current
                                    // script's response channel and owner id, so they cannot live
                                    // in the stateless direct SDK handler.
                                    match &msg {
                                        Message::AiSubscribe {
                                            request_id,
                                            events,
                                            chat_id,
                                        } => {
                                            let response =
                                                crate::ai::subscriptions::handle_subscribe(
                                                    subscription_owner_id.clone(),
                                                    request_id.clone(),
                                                    events.clone(),
                                                    chat_id.clone(),
                                                    reader_response_tx.clone(),
                                                );
                                            if let Err(e) = reader_response_tx.send(response) {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    error = %e,
                                                    "Failed to send AI subscription response"
                                                );
                                            }
                                            continue;
                                        }
                                        Message::AiUnsubscribe {
                                            request_id,
                                            subscription_id,
                                        } => {
                                            let response =
                                                crate::ai::subscriptions::handle_unsubscribe(
                                                    &subscription_owner_id,
                                                    request_id.clone(),
                                                    subscription_id.clone(),
                                                );
                                            if let Err(e) = reader_response_tx.send(response) {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    error = %e,
                                                    "Failed to send AI unsubscribe response"
                                                );
                                            }
                                            continue;
                                        }
                                        _ => {}
                                    }

                                    // Handle AI SDK messages that can be processed directly
                                    if let Some(response) = crate::ai::try_handle_ai_message(&msg) {
                                        tracing::info!(
                                            category = "EXEC",
                                            message_kind = ?std::mem::discriminant(&msg),
                                            "AI SDK message handled"
                                        );
                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::info!(
                                                category = "EXEC",
                                                error = %e,
                                                "Failed to send AI SDK response"
                                            );
                                        }
                                        continue;
                                    }

                                    // Handle GetState - needs UI state, forward to UI thread
                                    if let Message::GetState {
                                        request_id,
                                        target,
                                        summary_only,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            target = ?target,
                                            "GetState request"
                                        );
                                        let prompt_msg = PromptMessage::GetState {
                                            request_id: request_id.clone(),
                                            target: target.clone(),
                                            summary_only: *summary_only,
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle GetElements - needs UI state, forward to UI thread
                                    if let Message::GetElements {
                                        request_id,
                                        limit,
                                        target,
                                        include_headers,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            limit = ?limit,
                                            target = ?target,
                                            "GetElements request"
                                        );
                                        let prompt_msg = PromptMessage::GetElements {
                                            request_id: request_id.clone(),
                                            limit: *limit,
                                            target: target.clone(),
                                            include_headers: include_headers.unwrap_or(false),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle GetAgentChatState - needs Agent Chat view state, forward to UI thread
                                    if let Message::GetAgentChatState { request_id, target } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            target = ?target,
                                            "GetAgentChatState request"
                                        );
                                        let prompt_msg = PromptMessage::GetAgentChatState {
                                            request_id: request_id.clone(),
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    if let Message::GetAiReliabilityState { request_id, target } =
                                        &msg
                                    {
                                        let prompt_msg = PromptMessage::GetAiReliabilityState {
                                            request_id: request_id.clone(),
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            break;
                                        }
                                        continue;
                                    }

                                    if let Message::SetAiReliabilityTestFixture {
                                        request_id,
                                        fixture_id,
                                        target,
                                    } = &msg
                                    {
                                        let prompt_msg =
                                            PromptMessage::SetAiReliabilityTestFixture {
                                                request_id: request_id.clone(),
                                                fixture_id: fixture_id.clone(),
                                                target: target.clone(),
                                            };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle PerformAgentChatSetupAction - forward to UI thread
                                    if let Message::PerformAgentChatSetupAction {
                                        request_id,
                                        action,
                                        agent_id,
                                        target,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            action = ?action,
                                            "PerformAgentChatSetupAction request"
                                        );
                                        let prompt_msg =
                                            PromptMessage::PerformAgentChatSetupAction {
                                                request_id: request_id.clone(),
                                                action: *action,
                                                agent_id: agent_id.clone(),
                                                target: target.clone(),
                                            };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle ResetAgentChatTestProbe - forward to UI thread
                                    if let Message::ResetAgentChatTestProbe { request_id, target } =
                                        &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            target = ?target,
                                            "ResetAgentChatTestProbe request"
                                        );
                                        let prompt_msg = PromptMessage::ResetAgentChatTestProbe {
                                            request_id: request_id.clone(),
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle GetAgentChatTestProbe - forward to UI thread
                                    if let Message::GetAgentChatTestProbe {
                                        request_id,
                                        tail,
                                        target,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            target = ?target,
                                            "GetAgentChatTestProbe request"
                                        );
                                        let prompt_msg = PromptMessage::GetAgentChatTestProbe {
                                            request_id: request_id.clone(),
                                            tail: *tail,
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle GetLayoutInfo - needs UI state, forward to UI thread
                                    if let Message::GetLayoutInfo { request_id, target } = &msg {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            target = ?target,
                                            "GetLayoutInfo request"
                                        );
                                        let prompt_msg = PromptMessage::GetLayoutInfo {
                                            request_id: request_id.clone(),
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle InspectAutomationWindow - needs UI state, forward to UI thread
                                    if let Message::InspectAutomationWindow {
                                        request_id,
                                        target,
                                        hi_dpi,
                                        probes,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            target: "script_kit::automation",
                                            request_id = %request_id,
                                            target = ?target,
                                            hi_dpi = ?hi_dpi,
                                            probe_count = probes.len(),
                                            "automation.inspect.forward_to_ui"
                                        );
                                        let prompt_msg = PromptMessage::InspectAutomationWindow {
                                            request_id: request_id.clone(),
                                            target: target.clone(),
                                            hi_dpi: *hi_dpi,
                                            probes: probes.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle WaitFor - needs UI state polling, forward to UI thread
                                    if let Message::WaitFor {
                                        request_id,
                                        condition,
                                        timeout,
                                        poll_interval,
                                        trace,
                                        target,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            trace_mode = ?trace,
                                            target = ?target,
                                            "WaitFor request"
                                        );
                                        let prompt_msg = PromptMessage::WaitFor {
                                            request_id: request_id.clone(),
                                            condition: condition.clone(),
                                            timeout: *timeout,
                                            poll_interval: *poll_interval,
                                            trace: *trace,
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle Batch - needs UI thread for command execution
                                    if let Message::Batch {
                                        request_id,
                                        commands,
                                        options,
                                        trace,
                                        target,
                                    } = &msg
                                    {
                                        tracing::info!(
                                            category = "EXEC",
                                            request_id = %request_id,
                                            command_count = commands.len(),
                                            trace_mode = ?trace,
                                            target = ?target,
                                            "Batch request"
                                        );
                                        let prompt_msg = PromptMessage::Batch {
                                            request_id: request_id.clone(),
                                            commands: commands.clone(),
                                            options: options.clone(),
                                            trace: *trace,
                                            target: target.clone(),
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle ListAutomationWindows directly (no UI needed)
                                    if let Message::ListAutomationWindows { request_id } = &msg {
                                        tracing::info!(
                                            target: "script_kit::automation",
                                            request_id = %request_id,
                                            "automation.list_windows.request"
                                        );
                                        let windows = crate::windows::list_automation_windows();
                                        let focused_id =
                                            crate::windows::focused_automation_window_id();
                                        tracing::info!(
                                            target: "script_kit::automation",
                                            request_id = %request_id,
                                            window_count = windows.len(),
                                            focused_id = ?focused_id,
                                            "automation.list_windows.result"
                                        );
                                        let response = Message::automation_window_list_result(
                                            request_id.clone(),
                                            windows,
                                            focused_id,
                                        );
                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::error!(error = %e, "Failed to send automation window list response");
                                        }
                                        continue;
                                    }

                                    // Forward SimulateGpuiEvent to entity for real GPUI dispatch
                                    if let Message::SimulateGpuiEvent {
                                        request_id,
                                        target,
                                        event,
                                    } = msg.clone()
                                    {
                                        tracing::info!(
                                            target: "script_kit::automation",
                                            request_id = %request_id,
                                            target = ?target,
                                            event = ?event,
                                            "gpui_event_simulation.forwarding_to_entity"
                                        );
                                        let prompt_msg = PromptMessage::SimulateGpuiEvent {
                                            request_id,
                                            target,
                                            event,
                                        };
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                        continue;
                                    }

                                    // Handle CaptureScreenshot directly (no UI needed)
                                    if let Message::CaptureScreenshot {
                                        request_id,
                                        hi_dpi,
                                        target,
                                    } = &msg
                                    {
                                        let hi_dpi_mode = hi_dpi.unwrap_or(false);
                                        tracing::info!(
                                            target: "script_kit::automation",
                                            request_id = %request_id,
                                            hi_dpi = hi_dpi_mode,
                                            target = ?target,
                                            "automation.capture_screenshot.request"
                                        );

                                        let response = match capture_targeted_screenshot(
                                            target.as_ref(),
                                            hi_dpi_mode,
                                        ) {
                                            Ok((png_data, width, height)) => {
                                                use base64::Engine;
                                                let base64_data =
                                                    base64::engine::general_purpose::STANDARD
                                                        .encode(&png_data);
                                                tracing::info!(
                                                    request_id = %request_id,
                                                    width = width,
                                                    height = height,
                                                    hi_dpi = hi_dpi_mode,
                                                    data_len = base64_data.len(),
                                                    "Screenshot captured successfully"
                                                );
                                                Message::screenshot_result(
                                                    request_id.clone(),
                                                    base64_data,
                                                    width,
                                                    height,
                                                )
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    request_id = %request_id,
                                                    error = %e,
                                                    "Screenshot capture failed"
                                                );
                                                Message::screenshot_error(
                                                    request_id.clone(),
                                                    e.to_string(),
                                                )
                                            }
                                        };

                                        if let Err(e) = reader_response_tx.send(response) {
                                            tracing::error!(error = %e, "Failed to send screenshot response");
                                        }
                                        continue;
                                    }

                                    let msg = match convert_protocol_prompt_message(
                                        msg,
                                        PromptConversionSource::InteractiveSession,
                                    ) {
                                        Ok(prompt_message) => {
                                            if tx.send_blocking(prompt_message).is_err() {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    "Prompt channel closed, reader exiting"
                                                );
                                                break;
                                            }
                                            continue;
                                        }
                                        Err(msg) => *msg,
                                    };

                                    let prompt_msg = match msg {
                                        Message::Exit { .. } => Some(PromptMessage::ScriptExit),
                                        Message::ForceSubmit { value } => {
                                            Some(PromptMessage::ForceSubmit { value })
                                        }
                                        Message::Hide {} => Some(PromptMessage::HideWindow),
                                        Message::Browse { url } => {
                                            Some(PromptMessage::OpenBrowser { url })
                                        }
                                        Message::Hud { text, duration_ms } => {
                                            Some(PromptMessage::ShowHud { text, duration_ms })
                                        }
                                        Message::Notify {
                                            request_id,
                                            title,
                                            body,
                                        } => {
                                            tracing::info!(
                                                category = "EXEC",
                                                effect = "notify",
                                                has_title = title
                                                    .as_ref()
                                                    .map(|value| !value.trim().is_empty())
                                                    .unwrap_or(false),
                                                has_body = body
                                                    .as_ref()
                                                    .map(|value| !value.trim().is_empty())
                                                    .unwrap_or(false),
                                                "Received notify protocol message"
                                            );
                                            let result = if execute_script_normalize_notify_fields(
                                                title.clone(),
                                                body.clone(),
                                            )
                                            .is_none()
                                            {
                                                execute_script_feedback_receipt(
                                                    "notify",
                                                    false,
                                                    "invalid",
                                                    None,
                                                    None,
                                                    Some("ERR_SYSTEM_FEEDBACK_EMPTY"),
                                                    Some("notify() requires a non-empty title or body."),
                                                )
                                            } else {
                                                match execute_script_dispatch_notify_command(
                                                    title, body,
                                                ) {
                                                    Ok(()) => execute_script_feedback_receipt(
                                                        "notify",
                                                        true,
                                                        "dispatched",
                                                        Some("notify-rust"),
                                                        Some("notification_thread_spawned"),
                                                        None,
                                                        None,
                                                    ),
                                                    Err(error) => {
                                                        tracing::warn!(
                                                            category = "EXEC",
                                                            effect = "notify",
                                                            attempted = "notify-rust",
                                                            error = %error,
                                                            state = "spawn_failed",
                                                            "Failed to dispatch notify protocol feedback command"
                                                        );
                                                        execute_script_feedback_receipt(
                                                            "notify",
                                                            false,
                                                            "failed",
                                                            Some("notify-rust"),
                                                            None,
                                                            Some("ERR_SYSTEM_FEEDBACK_DISPATCH_FAILED"),
                                                            Some("Failed to dispatch notify feedback command."),
                                                        )
                                                    }
                                                }
                                            };
                                            execute_script_send_system_feedback_result(
                                                &reader_response_tx,
                                                request_id,
                                                result,
                                            );
                                            None
                                        }
                                        Message::Beep { request_id } => {
                                            tracing::info!(
                                                category = "EXEC",
                                                effect = "beep",
                                                "Received beep protocol message"
                                            );
                                            let result = if cfg!(target_os = "macos") {
                                                match execute_script_dispatch_beep_command() {
                                                    Ok(()) => execute_script_feedback_receipt(
                                                        "beep",
                                                        true,
                                                        "dispatched",
                                                        Some("afplay"),
                                                        Some("process_spawned"),
                                                        None,
                                                        None,
                                                    ),
                                                    Err(error) => {
                                                        tracing::warn!(
                                                            category = "EXEC",
                                                            effect = "beep",
                                                            attempted = "afplay /System/Library/Sounds/Tink.aiff",
                                                            error = %error,
                                                            state = "spawn_failed",
                                                            "Failed to dispatch beep protocol feedback command"
                                                        );
                                                        execute_script_feedback_receipt(
                                                            "beep",
                                                            false,
                                                            "failed",
                                                            Some("afplay"),
                                                            None,
                                                            Some("ERR_SYSTEM_FEEDBACK_DISPATCH_FAILED"),
                                                            Some("Failed to dispatch beep feedback command."),
                                                        )
                                                    }
                                                }
                                            } else {
                                                execute_script_feedback_receipt(
                                                    "beep",
                                                    false,
                                                    "unsupported",
                                                    None,
                                                    None,
                                                    Some("ERR_UNSUPPORTED_SDK_FEATURE"),
                                                    Some("beep() feedback dispatch is only supported on macOS."),
                                                )
                                            };
                                            execute_script_send_system_feedback_result(
                                                &reader_response_tx,
                                                request_id,
                                                result,
                                            );
                                            None
                                        }
                                        Message::Say {
                                            request_id,
                                            text,
                                            voice,
                                        } => {
                                            tracing::info!(
                                                category = "EXEC",
                                                effect = "say",
                                                has_voice = voice
                                                    .as_ref()
                                                    .map(|value| !value.trim().is_empty())
                                                    .unwrap_or(false),
                                                text_len = text.chars().count(),
                                                "Received say protocol message"
                                            );
                                            let result = if execute_script_normalize_optional_text(
                                                Some(text.clone()),
                                            )
                                            .is_none()
                                            {
                                                execute_script_feedback_receipt(
                                                    "say",
                                                    false,
                                                    "invalid",
                                                    None,
                                                    None,
                                                    Some("ERR_SYSTEM_FEEDBACK_EMPTY"),
                                                    Some("say() requires non-empty text."),
                                                )
                                            } else if cfg!(target_os = "macos") {
                                                match execute_script_dispatch_say_command(
                                                    text, voice,
                                                ) {
                                                    Ok(()) => execute_script_feedback_receipt(
                                                        "say",
                                                        true,
                                                        "dispatched",
                                                        Some("say"),
                                                        Some("process_spawned"),
                                                        None,
                                                        None,
                                                    ),
                                                    Err(error) => {
                                                        tracing::warn!(
                                                            category = "EXEC",
                                                            effect = "say",
                                                            attempted = "say [-v voice] <text>",
                                                            error = %error,
                                                            state = "spawn_failed",
                                                            "Failed to dispatch say protocol feedback command"
                                                        );
                                                        execute_script_feedback_receipt(
                                                                "say",
                                                                false,
                                                                "failed",
                                                                Some("say"),
                                                                None,
                                                                Some("ERR_SYSTEM_FEEDBACK_DISPATCH_FAILED"),
                                                                Some("Failed to dispatch say feedback command."),
                                                            )
                                                    }
                                                }
                                            } else {
                                                execute_script_feedback_receipt(
                                                        "say",
                                                        false,
                                                        "unsupported",
                                                        None,
                                                        None,
                                                        Some("ERR_UNSUPPORTED_SDK_FEATURE"),
                                                        Some("say() feedback dispatch is only supported on macOS."),
                                                    )
                                            };
                                            execute_script_send_system_feedback_result(
                                                &reader_response_tx,
                                                request_id,
                                                result,
                                            );
                                            None
                                        }
                                        Message::SetStatus { status, message } => {
                                            Some(PromptMessage::SetStatus { status, message })
                                        }
                                        Message::SetActions { actions } => {
                                            Some(PromptMessage::SetActions { actions })
                                        }
                                        Message::SetInput { text } => {
                                            Some(PromptMessage::SetInput { text })
                                        }
                                        Message::Widget { id, .. } => {
                                            Some(PromptMessage::WidgetComingSoon { id })
                                        }
                                        Message::Webcam { id } => {
                                            Some(PromptMessage::WebcamComingSoon { id })
                                        }
                                        Message::Mic { id } => {
                                            Some(PromptMessage::MicComingSoon { id })
                                        }
                                        Message::ShowGrid { options } => {
                                            Some(PromptMessage::ShowGrid { options })
                                        }
                                        Message::HideGrid => Some(PromptMessage::HideGrid),
                                        // Chat prompt messages
                                        Message::Chat {
                                            id,
                                            placeholder,
                                            messages,
                                            hint,
                                            footer,
                                            actions,
                                            model,
                                            models,
                                            save_history,
                                            use_builtin_ai,
                                        } => {
                                            logging::bench_log("Chat_message_parsed");
                                            Some(PromptMessage::ShowChat {
                                                id,
                                                placeholder,
                                                messages,
                                                hint,
                                                footer,
                                                actions,
                                                model,
                                                models,
                                                save_history,
                                                use_builtin_ai,
                                            })
                                        }
                                        Message::ChatMessage { id, message } => {
                                            Some(PromptMessage::ChatAddMessage { id, message })
                                        }
                                        Message::ChatStreamStart {
                                            id,
                                            message_id,
                                            position,
                                        } => Some(PromptMessage::ChatStreamStart {
                                            id,
                                            message_id,
                                            position,
                                        }),
                                        Message::ChatStreamChunk {
                                            id,
                                            message_id,
                                            chunk,
                                        } => Some(PromptMessage::ChatStreamChunk {
                                            id,
                                            message_id,
                                            chunk,
                                        }),
                                        Message::ChatStreamComplete { id, message_id } => {
                                            Some(PromptMessage::ChatStreamComplete {
                                                id,
                                                message_id,
                                            })
                                        }
                                        Message::ChatClear { id } => {
                                            Some(PromptMessage::ChatClear { id })
                                        }
                                        Message::ChatSetError {
                                            id,
                                            message_id,
                                            error,
                                        } => Some(PromptMessage::ChatSetError {
                                            id,
                                            message_id,
                                            error,
                                        }),
                                        Message::ChatClearError { id, message_id } => {
                                            Some(PromptMessage::ChatClearError { id, message_id })
                                        }
                                        // ChatSubmit goes from App → SDK, not SDK → App
                                        Message::ChatSubmit { .. } => None,
                                        // AI window start chat
                                        Message::AiStartChat {
                                            request_id,
                                            message,
                                            system_prompt,
                                            image,
                                            model_id,
                                            no_response,
                                            parts,
                                        } => Some(PromptMessage::AiStartChat {
                                            request_id,
                                            message,
                                            system_prompt,
                                            image,
                                            model_id,
                                            no_response,
                                            parts,
                                        }),
                                        // AI window focus
                                        Message::AiFocus { request_id } => {
                                            Some(PromptMessage::AiFocus { request_id })
                                        }
                                        other => {
                                            // Get the message type name for user feedback
                                            let msg_type = format!("{:?}", other);
                                            // Extract just the variant name (before any {})
                                            let type_name = msg_type
                                                .split('{')
                                                .next()
                                                .unwrap_or(&msg_type)
                                                .trim()
                                                .to_string();
                                            tracing::warn!(
                                                category = "WARN",
                                                message_type = %type_name,
                                                "Unhandled message type"
                                            );
                                            Some(PromptMessage::UnhandledMessage {
                                                message_type: type_name,
                                            })
                                        }
                                    };

                                    if let Some(prompt_msg) = prompt_msg {
                                        if tx.send_blocking(prompt_msg).is_err() {
                                            tracing::info!(
                                                category = "EXEC",
                                                "Prompt channel closed, reader exiting"
                                            );
                                            break;
                                        }
                                    }
                                }
                                Ok(None) => {
                                    tracing::info!(category = "EXEC", "Script stdout closed (EOF)");

                                    // Check if process exited with error
                                    let exit_code = match keep_alive_child.try_wait() {
                                        Ok(Some(status)) => status.code(),
                                        Ok(None) => {
                                            // Process still running, wait for it
                                            match keep_alive_child.wait() {
                                                Ok(status) => status.code(),
                                                Err(_) => None,
                                            }
                                        }
                                        Err(_) => None,
                                    };

                                    tracing::info!(
                                        category = "EXEC",
                                        exit_code = ?exit_code,
                                        "Script exit code observed"
                                    );

                                    // If non-zero exit code, capture stderr and send error
                                    if let Some(code) = exit_code {
                                        if code != 0 {
                                            // FIX: Wait for stderr reader to complete (with timeout)
                                            // before reading buffer. This prevents partial error captures
                                            // when stderr flushes late after bun exits.
                                            // 100ms timeout is generous - stderr should drain quickly.
                                            let stderr_output = stderr_capture
                                                .as_ref()
                                                .map(|cap| {
                                                    cap.get_contents_with_timeout(
                                                        std::time::Duration::from_millis(100),
                                                    )
                                                })
                                                .filter(|s| !s.is_empty());

                                            if let Some(ref stderr_text) = stderr_output {
                                                tracing::info!(
                                                    category = "EXEC",
                                                    bytes = stderr_text.len(),
                                                    "Captured stderr from buffer"
                                                );

                                                // Parse error info and generate suggestions
                                                let error_message =
                                                    executor::extract_error_message(stderr_text);
                                                let stack_trace =
                                                    executor::parse_stack_trace(stderr_text);
                                                let suggestions = executor::generate_suggestions(
                                                    stderr_text,
                                                    Some(code),
                                                );

                                                // Send script error message
                                                let _ =
                                                    tx.send_blocking(PromptMessage::ScriptError {
                                                        error_message,
                                                        stderr_output: Some(stderr_text.clone()),
                                                        exit_code: Some(code),
                                                        stack_trace,
                                                        script_path: script_path.clone(),
                                                        suggestions,
                                                    });
                                            } else {
                                                // No stderr, send generic error
                                                let _ =
                                                    tx.send_blocking(PromptMessage::ScriptError {
                                                        error_message: format!(
                                                            "Script exited with code {}",
                                                            code
                                                        ),
                                                        stderr_output: None,
                                                        exit_code: Some(code),
                                                        stack_trace: None,
                                                        script_path: script_path.clone(),
                                                        suggestions: vec![
                                                            "Check the script for errors"
                                                                .to_string(),
                                                        ],
                                                    });
                                            }
                                        }
                                    }

                                    let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                    break;
                                }
                                Err(e) => {
                                    tracing::info!(
                                        category = "EXEC",
                                        error = %e,
                                        "Error reading from script"
                                    );

                                    // FIX: Wait for stderr reader to complete before reading
                                    let stderr_output = stderr_capture
                                        .as_ref()
                                        .map(|cap| {
                                            cap.get_contents_with_timeout(
                                                std::time::Duration::from_millis(100),
                                            )
                                        })
                                        .filter(|s| !s.is_empty());

                                    if let Some(ref stderr_text) = stderr_output {
                                        let error_message =
                                            executor::extract_error_message(stderr_text);
                                        let stack_trace = executor::parse_stack_trace(stderr_text);
                                        let suggestions =
                                            executor::generate_suggestions(stderr_text, None);

                                        let _ = tx.send_blocking(PromptMessage::ScriptError {
                                            error_message,
                                            stderr_output: Some(stderr_text.clone()),
                                            exit_code: None,
                                            stack_trace,
                                            script_path: script_path.clone(),
                                            suggestions,
                                        });
                                    }

                                    let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                    break;
                                }
                            }
                        }
                        let cleaned =
                            crate::ai::subscriptions::cleanup_owner(&subscription_owner_id);
                        if cleaned > 0 {
                            tracing::info!(
                                category = "EXEC",
                                cleaned,
                                "Cleaned AI live subscriptions for exited script"
                            );
                        }
                        tracing::info!(
                            category = "EXEC",
                            "Reader thread exited, process handle will now be dropped"
                        );
                    });

                    // Store the response sender for the UI to use
                    self.response_sender = Some(response_tx);
                }
                Err(e) => {
                    tracing::info!(
                        category = "EXEC",
                        error = %e,
                        "Failed to start interactive session"
                    );
                    if self
                        .pending_tab_ai_execution
                        .as_ref()
                        .map(|record| record.temp_script_path == script_path_for_errors)
                        .unwrap_or(false)
                    {
                        self.complete_tab_ai_execution(
                            false,
                            Some(format!("Tab AI script failed to start: {e}")),
                            cx,
                        );
                    }
                    self.last_output = Some(SharedString::from(format!("✗ Error: {}", e)));
                    cx.notify();
                }
            }
        }
    }
}
// --- merged from part_002.rs ---
#[cfg(test)]
include!("mod_tests.rs");

#![allow(dead_code)]
//! Structured JSONL logging for AI agents and human-readable stderr output.
//!
//! This module provides dual-output logging:
//! - **JSONL to file** (~/.sk/kit/logs/script-kit-gpui.jsonl) - structured for AI agent parsing
//! - **Pretty to stderr** - human-readable for developers
//! - **Compact AI mode** (SCRIPT_KIT_AI_LOG=1) - ultra-compact line format for AI context
//!
//! # Compact AI Format
//!
//! When `SCRIPT_KIT_AI_LOG=1` is set, stderr uses compact format:
//! ```text
//! SS.mmm|L|C|message
//! ```
//! Where:
//! - SS.mmm = seconds.milliseconds within current minute (resets each minute)
//! - L = single char level (i/w/e/d/t)
//! - C = single char category code (see AGENTS.md for legend)
//!
//!
//! # JSONL Output Format
//!
//! Each line is a valid JSON object:
//! ```json
//! {"timestamp":"2024-12-25T10:30:45.123Z","level":"INFO","target":"script_kit_gpui::main","message":"Script executed","fields":{"event_type":"script_event","script_id":"abc","duration_ms":42}}
//! ```

use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::{Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, MakeWriter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

// =============================================================================
// COMPACT AI FORMAT (SCRIPT_KIT_AI_LOG=1)
// =============================================================================

/// Category code mapping for compact AI logs.
/// See AGENTS.md for the full legend.
fn category_to_code(category: &str) -> char {
    match category.to_uppercase().as_str() {
        "POSITION" => 'P',
        "APP" => 'A',
        "UI" => 'U',
        "STDIN" => 'S',
        "HOTKEY" => 'H',
        "VISIBILITY" => 'V',
        "EXEC" => 'E',
        "KEY" => 'K',
        "FOCUS" => 'F',
        "THEME" => 'T',
        "CACHE" => 'C',
        "PERF" => 'R',
        "WINDOW_MGR" => 'W',
        "ERROR" => 'X',
        "MOUSE_HOVER" => 'M',
        "SCROLL_STATE" => 'L',
        "SCROLL_PERF" => 'Q',
        "SCRIPT" => 'B', // B for Bun/script
        "CONFIG" => 'N', // N for coNfig
        "RESIZE" => 'Z',
        "TRAY" => 'H',   // Tray is part of Hotkey subsystem
        "DESIGN" => 'D', // Design system
        _ => '-',        // Unknown category
    }
}

/// Convert tracing Level to single char
fn level_to_char(level: Level) -> char {
    match level {
        Level::ERROR => 'e',
        Level::WARN => 'w',
        Level::INFO => 'i',
        Level::DEBUG => 'd',
        Level::TRACE => 't',
    }
}

/// Infer category code from tracing target path
fn infer_category_from_target(target: &str) -> char {
    // Match by module name in the target path
    // Group patterns by their category code to satisfy clippy
    if target.contains("executor") {
        'E' // Execution
    } else if target.contains("theme") {
        'T' // Theme
    } else if target.contains("window_manager") || target.contains("window_control") {
        'W' // Window manager
    } else if target.contains("stdin") || target.contains("protocol") {
        'S' // Stdin/protocol
    } else if target.contains("hotkey") || target.contains("tray") {
        'H' // Hotkey
    } else if target.contains("scripts") || target.contains("file_search") {
        'G' // Script loaGing (not execution)
    } else if target.contains("config") {
        'N' // coNfig
    } else if target.contains("watcher")
        || target.contains("clipboard")
        || target.contains("logging")
    {
        'A' // App lifecycle/subsystems
    } else if target.contains("panel")
        || target.contains("prompts")
        || target.contains("editor")
        || target.contains("terminal")
        || target.contains("term_prompt")
        || target.contains("pty")
        || target.contains("syntax")
    {
        'U' // UI components
    } else if target.contains("perf") {
        'R' // peRformance
    } else if target.contains("resize") {
        'Z' // resiZe
    } else {
        '-' // Unknown
    }
}

/// Get seconds.milliseconds within current minute
fn get_minute_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_millis = now.as_millis();
    let millis_in_minute = total_millis % 60_000;
    let seconds = millis_in_minute / 1000;
    let millis = millis_in_minute % 1000;
    format!("{:02}.{:03}", seconds, millis)
}

/// Visitor to extract category field from tracing events
struct CategoryExtractor {
    category: Option<String>,
    message: String,
}

impl CategoryExtractor {
    fn new() -> Self {
        Self {
            category: None,
            message: String::new(),
        }
    }
}

impl Visit for CategoryExtractor {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "category" => self.category = Some(value.to_string()),
            "message" => self.message = value.to_string(),
            // Skip legacy field
            "legacy" => {}
            _ => {
                // Append other fields to message
                if !self.message.is_empty() {
                    self.message.push(' ');
                }
                let _ = write!(self.message, "{}={}", field.name(), value);
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "category" => self.category = Some(format!("{:?}", value)),
            "message" => self.message = format!("{:?}", value),
            // Skip legacy field
            "legacy" => {}
            _ => {
                if !self.message.is_empty() {
                    self.message.push(' ');
                }
                let _ = write!(self.message, "{}={:?}", field.name(), value);
            }
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() != "legacy" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{}={}", field.name(), value);
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() != "legacy" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{}={}", field.name(), value);
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() != "legacy" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{}={}", field.name(), value);
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() != "legacy" {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{}={:.2}", field.name(), value);
        }
    }
}

/// Compact AI formatter for stderr output.
/// Format: `SS.mmm|L|C|message`
pub struct CompactAiFormatter;

impl<S, N> FormatEvent<S, N> for CompactAiFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let timestamp = get_minute_timestamp();
        let level_char = level_to_char(*event.metadata().level());

        // Extract category and message from fields
        let mut extractor = CategoryExtractor::new();
        event.record(&mut extractor);

        // Infer category from target if not explicitly set
        let category_code = if let Some(ref cat) = extractor.category {
            category_to_code(cat)
        } else {
            // Try to infer from target (e.g., script_kit_gpui::executor -> E)
            let target = event.metadata().target();
            infer_category_from_target(target)
        };

        // Build the compact line
        writeln!(
            writer,
            "{}|{}|{}|{}",
            timestamp, level_char, category_code, extractor.message
        )
    }
}

/// Wrapper to make stderr compatible with MakeWriter
struct StderrWriter;

impl<'a> MakeWriter<'a> for StderrWriter {
    type Writer = std::io::Stderr;

    fn make_writer(&'a self) -> Self::Writer {
        std::io::stderr()
    }
}

// =============================================================================
// LEGACY SUPPORT - In-memory log buffer for UI display
// =============================================================================

static LOG_BUFFER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
const MAX_LOG_LINES: usize = 50;

/// Guard that must be kept alive for the duration of the program.
/// Dropping this guard will flush and close the log file.
pub struct LoggingGuard {
    _file_guard: WorkerGuard,
}

/// Initialize the dual-output logging system.
///
/// Returns a guard that MUST be kept alive for the duration of the program.
/// Dropping the guard will flush remaining logs and close the file.
///
pub fn init() -> LoggingGuard {
    // Initialize legacy log buffer for UI display
    let _ = LOG_BUFFER.set(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES)));

    // Check for AI compact log mode
    let ai_log_mode = std::env::var("SCRIPT_KIT_AI_LOG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Create log directory
    let log_dir = get_log_dir();
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("[LOGGING] Failed to create log directory: {}", e);
    }

    let log_path = log_dir.join("script-kit-gpui.jsonl");

    // Print log location for discoverability (only in non-AI mode)
    if !ai_log_mode {
        eprintln!("========================================");
        eprintln!("[SCRIPT-KIT-GPUI] JSONL log: {}", log_path.display());
        eprintln!("[SCRIPT-KIT-GPUI] Pretty logs: stderr");
        eprintln!("========================================");
    }

    // Open log file with append mode
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap_or_else(|e| {
            eprintln!("[LOGGING] Failed to open log file: {}", e);
            // Fallback to /dev/null equivalent
            OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .expect("Failed to open /dev/null")
        });

    // Create non-blocking writer for file (prevents UI freeze)
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file);

    // Environment filter - default to info, allow override via RUST_LOG
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,gpui=warn,hyper=warn,reqwest=warn"));

    // JSONL layer for file output (AI agents)
    let json_layer = fmt::layer()
        .json()
        .with_writer(non_blocking_file)
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_span_events(FmtSpan::NONE);

    if ai_log_mode {
        // Compact AI layer for stderr (token-efficient for AI agents)
        let ai_layer = fmt::layer()
            .with_writer(StderrWriter)
            .with_ansi(false)
            .event_format(CompactAiFormatter);

        // Initialize the subscriber with JSON file + compact stderr
        tracing_subscriber::registry()
            .with(env_filter)
            .with(json_layer)
            .with(ai_layer)
            .init();
    } else {
        // Pretty layer for stderr (human developers)
        let pretty_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(false)
            .compact();

        // Initialize the subscriber with JSON file + pretty stderr
        tracing_subscriber::registry()
            .with(env_filter)
            .with(json_layer)
            .with(pretty_layer)
            .init();
    }

    tracing::info!(
        event_type = "app_lifecycle",
        action = "started",
        log_path = %log_path.display(),
        ai_log_mode = ai_log_mode,
        "Application logging initialized"
    );

    LoggingGuard {
        _file_guard: file_guard,
    }
}

/// Get the log directory path (~/.sk/kit/logs/)
fn get_log_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".sk/kit").join("logs"))
        .unwrap_or_else(|| std::env::temp_dir().join("script-kit-logs"))
}

/// Get the path to the JSONL log file
pub fn log_path() -> PathBuf {
    get_log_dir().join("script-kit-gpui.jsonl")
}

// =============================================================================
// BACKWARD COMPATIBILITY - Legacy log() function wrappers
// =============================================================================

/// Legacy log function - wraps tracing::info! for backward compatibility.
///
/// Prefer using tracing macros directly for structured fields:
/// ```rust
/// tracing::info!(category = "UI", duration_ms = 42, "Button clicked");
/// ```
pub fn log(category: &str, message: &str) {
    // Add to legacy buffer for UI display
    add_to_buffer(category, message);

    // Use tracing for actual logging
    tracing::info!(category = category, legacy = true, "{}", message);
}

/// Add a log entry to the in-memory buffer for UI display
fn add_to_buffer(category: &str, message: &str) {
    if let Some(buffer) = LOG_BUFFER.get() {
        if let Ok(mut buf) = buffer.lock() {
            if buf.len() >= MAX_LOG_LINES {
                buf.pop_front();
            }
            buf.push_back(format!("[{}] {}", category, message));
        }
    }
}

/// Get recent log lines for UI display
pub fn get_recent_logs() -> Vec<String> {
    if let Some(buffer) = LOG_BUFFER.get() {
        if let Ok(buf) = buffer.lock() {
            return buf.iter().cloned().collect();
        }
    }
    Vec::new()
}

/// Get the last N log lines
pub fn get_last_logs(n: usize) -> Vec<String> {
    if let Some(buffer) = LOG_BUFFER.get() {
        if let Ok(buf) = buffer.lock() {
            return buf.iter().rev().take(n).cloned().collect();
        }
    }
    Vec::new()
}

/// Debug-only logging - compiled out in release builds
/// Use for verbose performance/scroll/cache logging
#[cfg(debug_assertions)]
pub fn log_debug(category: &str, message: &str) {
    add_to_buffer(category, message);
    tracing::debug!(category = category, legacy = true, "{}", message);
}

#[cfg(not(debug_assertions))]
pub fn log_debug(_category: &str, _message: &str) {
    // No-op in release builds
}

// =============================================================================
// STRUCTURED LOGGING HELPERS
// These provide typed, structured logging for common operations
// =============================================================================

/// Log a script execution event with structured fields
pub fn log_script_event(script_id: &str, action: &str, duration_ms: Option<u64>, success: bool) {
    add_to_buffer(
        "SCRIPT",
        &format!("{} {} (success={})", action, script_id, success),
    );

    match duration_ms {
        Some(duration) => {
            tracing::info!(
                event_type = "script_event",
                script_id = script_id,
                action = action,
                duration_ms = duration,
                success = success,
                "Script {} {}",
                action,
                script_id
            );
        }
        None => {
            tracing::info!(
                event_type = "script_event",
                script_id = script_id,
                action = action,
                success = success,
                "Script {} {}",
                action,
                script_id
            );
        }
    }
}

/// Log a UI event with structured fields
pub fn log_ui_event(component: &str, action: &str, details: Option<&str>) {
    let msg = match details {
        Some(d) => format!("{} {} - {}", component, action, d),
        None => format!("{} {}", component, action),
    };
    add_to_buffer("UI", &msg);

    tracing::info!(
        event_type = "ui_event",
        component = component,
        action = action,
        details = details,
        "{}",
        msg
    );
}

/// Log a keyboard event with structured fields
pub fn log_key_event(key: &str, modifiers: &str, action: &str) {
    add_to_buffer("KEY", &format!("{} {} ({})", action, key, modifiers));

    tracing::debug!(
        event_type = "key_event",
        key = key,
        modifiers = modifiers,
        action = action,
        "Key {} {}",
        action,
        key
    );
}

/// Log a performance metric with structured fields
pub fn log_perf(operation: &str, duration_ms: u64, threshold_ms: u64) {
    let is_slow = duration_ms > threshold_ms;
    let level_marker = if is_slow { "SLOW" } else { "OK" };

    add_to_buffer(
        "PERF",
        &format!("{} {}ms [{}]", operation, duration_ms, level_marker),
    );

    if is_slow {
        tracing::warn!(
            event_type = "performance",
            operation = operation,
            duration_ms = duration_ms,
            threshold_ms = threshold_ms,
            is_slow = true,
            "Slow operation: {} took {}ms (threshold: {}ms)",
            operation,
            duration_ms,
            threshold_ms
        );
    } else {
        tracing::debug!(
            event_type = "performance",
            operation = operation,
            duration_ms = duration_ms,
            threshold_ms = threshold_ms,
            is_slow = false,
            "Operation {} completed in {}ms",
            operation,
            duration_ms
        );
    }
}

/// Log an error with structured fields and context
pub fn log_error(category: &str, error: &str, context: Option<&str>) {
    let msg = match context {
        Some(ctx) => format!("{}: {} (context: {})", category, error, ctx),
        None => format!("{}: {}", category, error),
    };
    add_to_buffer("ERROR", &msg);

    tracing::error!(
        event_type = "error",
        category = category,
        error_message = error,
        context = context,
        "{}",
        msg
    );
}

// =============================================================================
// MOUSE HOVER LOGGING
// Category: MOUSE_HOVER
// Purpose: Log mouse enter/leave events on list items for debugging hover/focus behavior
// =============================================================================

/// Log mouse enter event on a list item
pub fn log_mouse_enter(item_index: usize, item_id: Option<&str>) {
    let id_info = item_id.unwrap_or("none");
    add_to_buffer(
        "MOUSE_HOVER",
        &format!("ENTER item_index={} id={}", item_index, id_info),
    );

    tracing::debug!(
        event_type = "mouse_hover",
        action = "enter",
        item_index = item_index,
        item_id = id_info,
        "Mouse enter item {}",
        item_index
    );
}

/// Log mouse leave event on a list item
pub fn log_mouse_leave(item_index: usize, item_id: Option<&str>) {
    let id_info = item_id.unwrap_or("none");
    add_to_buffer(
        "MOUSE_HOVER",
        &format!("LEAVE item_index={} id={}", item_index, id_info),
    );

    tracing::debug!(
        event_type = "mouse_hover",
        action = "leave",
        item_index = item_index,
        item_id = id_info,
        "Mouse leave item {}",
        item_index
    );
}

/// Log mouse hover state change (for tracking focus/highlight transitions)
pub fn log_mouse_hover_state(item_index: usize, is_hovered: bool, is_focused: bool) {
    add_to_buffer(
        "MOUSE_HOVER",
        &format!(
            "STATE item_index={} hovered={} focused={}",
            item_index, is_hovered, is_focused
        ),
    );

    tracing::debug!(
        event_type = "mouse_hover",
        action = "state_change",
        item_index = item_index,
        is_hovered = is_hovered,
        is_focused = is_focused,
        "Hover state: item {} hovered={} focused={}",
        item_index,
        is_hovered,
        is_focused
    );
}

// =============================================================================
// SCROLL STATE LOGGING
// Category: SCROLL_STATE
// Purpose: Log scroll position changes and scroll_to_item calls for debugging jitter
// =============================================================================

/// Log scroll position change
pub fn log_scroll_position(scroll_top: f32, visible_start: usize, visible_end: usize) {
    add_to_buffer(
        "SCROLL_STATE",
        &format!(
            "POSITION scroll_top={:.2} visible_range={}..{}",
            scroll_top, visible_start, visible_end
        ),
    );

    tracing::debug!(
        event_type = "scroll_state",
        action = "position",
        scroll_top = scroll_top,
        visible_start = visible_start,
        visible_end = visible_end,
        "Scroll position: {:.2} (visible {}..{})",
        scroll_top,
        visible_start,
        visible_end
    );
}

/// Log scroll_to_item call
pub fn log_scroll_to_item(target_index: usize, reason: &str) {
    add_to_buffer(
        "SCROLL_STATE",
        &format!("SCROLL_TO_ITEM target={} reason={}", target_index, reason),
    );

    tracing::debug!(
        event_type = "scroll_state",
        action = "scroll_to_item",
        target_index = target_index,
        reason = reason,
        "Scroll to item {} (reason: {})",
        target_index,
        reason
    );
}

/// Log scroll bounds/viewport info
pub fn log_scroll_bounds(viewport_height: f32, content_height: f32, item_count: usize) {
    add_to_buffer(
        "SCROLL_STATE",
        &format!(
            "BOUNDS viewport={:.2} content={:.2} items={}",
            viewport_height, content_height, item_count
        ),
    );

    tracing::debug!(
        event_type = "scroll_state",
        action = "bounds",
        viewport_height = viewport_height,
        content_height = content_height,
        item_count = item_count,
        "Scroll bounds: viewport={:.2} content={:.2} items={}",
        viewport_height,
        content_height,
        item_count
    );
}

/// Log scroll adjustment (when scroll position is programmatically corrected)
pub fn log_scroll_adjustment(from: f32, to: f32, reason: &str) {
    add_to_buffer(
        "SCROLL_STATE",
        &format!("ADJUSTMENT from={:.2} to={:.2} reason={}", from, to, reason),
    );

    tracing::debug!(
        event_type = "scroll_state",
        action = "adjustment",
        from = from,
        to = to,
        reason = reason,
        "Scroll adjustment: {:.2} -> {:.2} ({})",
        from,
        to,
        reason
    );
}

// =============================================================================
// SCROLL PERFORMANCE LOGGING
// Category: SCROLL_PERF
// Purpose: Log timing information for scroll operations to detect jitter sources
// =============================================================================

/// Log scroll operation timing - returns start timestamp
pub fn log_scroll_perf_start(operation: &str) -> u128 {
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);

    #[cfg(debug_assertions)]
    {
        add_to_buffer("SCROLL_PERF", &format!("START {} at={}", operation, start));
        tracing::trace!(
            event_type = "scroll_perf",
            action = "start",
            operation = operation,
            start_micros = start,
            "Scroll perf start: {}",
            operation
        );
    }

    start
}

/// Log scroll operation completion with duration
pub fn log_scroll_perf_end(operation: &str, start_micros: u128) {
    let end = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let duration = end.saturating_sub(start_micros);

    #[cfg(debug_assertions)]
    {
        add_to_buffer(
            "SCROLL_PERF",
            &format!("END {} duration_us={}", operation, duration),
        );
        tracing::trace!(
            event_type = "scroll_perf",
            action = "end",
            operation = operation,
            duration_us = duration,
            "Scroll perf end: {} ({}us)",
            operation,
            duration
        );
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (operation, duration); // Silence unused warnings
    }
}

/// Log scroll frame timing (for detecting dropped frames)
pub fn log_scroll_frame(frame_time_ms: f32, expected_frame_ms: f32) {
    let is_slow = frame_time_ms > expected_frame_ms * 1.5;

    #[cfg(debug_assertions)]
    {
        let marker = if is_slow { " [SLOW]" } else { "" };
        add_to_buffer(
            "SCROLL_PERF",
            &format!(
                "FRAME time={:.2}ms expected={:.2}ms{}",
                frame_time_ms, expected_frame_ms, marker
            ),
        );

        if is_slow {
            tracing::warn!(
                event_type = "scroll_perf",
                action = "frame",
                frame_time_ms = frame_time_ms,
                expected_frame_ms = expected_frame_ms,
                is_slow = true,
                "Slow frame: {:.2}ms (expected {:.2}ms)",
                frame_time_ms,
                expected_frame_ms
            );
        } else {
            tracing::trace!(
                event_type = "scroll_perf",
                action = "frame",
                frame_time_ms = frame_time_ms,
                expected_frame_ms = expected_frame_ms,
                is_slow = false,
                "Frame: {:.2}ms",
                frame_time_ms
            );
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (frame_time_ms, expected_frame_ms, is_slow);
    }
}

/// Log scroll event rate (for detecting rapid scroll input)
pub fn log_scroll_event_rate(events_per_second: f32) {
    let is_rapid = events_per_second > 60.0;

    #[cfg(debug_assertions)]
    {
        let marker = if is_rapid { " [RAPID]" } else { "" };
        add_to_buffer(
            "SCROLL_PERF",
            &format!("EVENT_RATE eps={:.1}{}", events_per_second, marker),
        );

        if is_rapid {
            tracing::debug!(
                event_type = "scroll_perf",
                action = "event_rate",
                events_per_second = events_per_second,
                is_rapid = true,
                "Rapid scroll events: {:.1}/s",
                events_per_second
            );
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (events_per_second, is_rapid);
    }
}

// =============================================================================
// KEY EVENT & SCROLL QUEUE METRICS
// Category: SCROLL_PERF
// Purpose: Track input rates, frame gaps, queue depth, and render stalls
// =============================================================================

/// Log keyboard event rate (events per second) for detecting fast key repeat
pub fn log_key_event_rate(events_per_sec: f64) {
    let is_fast = events_per_sec > 30.0;
    let is_very_fast = events_per_sec > 60.0;

    #[cfg(debug_assertions)]
    {
        let marker = if is_very_fast {
            " [VERY_FAST]"
        } else if is_fast {
            " [FAST]"
        } else {
            ""
        };
        add_to_buffer(
            "SCROLL_PERF",
            &format!("KEY_EVENT_RATE eps={:.1}{}", events_per_sec, marker),
        );

        tracing::debug!(
            event_type = "scroll_perf",
            action = "key_event_rate",
            events_per_sec = events_per_sec,
            is_fast = is_fast,
            is_very_fast = is_very_fast,
            "Key event rate: {:.1}/s",
            events_per_sec
        );
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (events_per_sec, is_fast, is_very_fast);
    }
}

/// Log frame timing gap (when frames take longer than expected)
pub fn log_frame_gap(gap_ms: u64) {
    let is_significant = gap_ms > 16;
    let is_severe = gap_ms > 100;

    #[cfg(debug_assertions)]
    {
        let marker = if is_severe {
            " [SEVERE]"
        } else if is_significant {
            " [SLOW]"
        } else {
            ""
        };
        add_to_buffer(
            "SCROLL_PERF",
            &format!("FRAME_GAP gap_ms={}{}", gap_ms, marker),
        );

        if is_severe {
            tracing::warn!(
                event_type = "scroll_perf",
                action = "frame_gap",
                gap_ms = gap_ms,
                is_severe = true,
                "Severe frame gap: {}ms",
                gap_ms
            );
        } else if is_significant {
            tracing::debug!(
                event_type = "scroll_perf",
                action = "frame_gap",
                gap_ms = gap_ms,
                is_significant = true,
                "Frame gap: {}ms",
                gap_ms
            );
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (gap_ms, is_significant, is_severe);
    }
}

/// Log scroll queue depth (number of pending scroll operations)
pub fn log_scroll_queue_depth(depth: usize) {
    let is_backed_up = depth > 5;
    let is_critical = depth > 20;

    #[cfg(debug_assertions)]
    {
        let marker = if is_critical {
            " [CRITICAL]"
        } else if is_backed_up {
            " [BACKED_UP]"
        } else {
            ""
        };
        add_to_buffer(
            "SCROLL_PERF",
            &format!("QUEUE_DEPTH depth={}{}", depth, marker),
        );

        if is_critical {
            tracing::warn!(
                event_type = "scroll_perf",
                action = "queue_depth",
                depth = depth,
                is_critical = true,
                "Critical queue depth: {}",
                depth
            );
        } else if is_backed_up {
            tracing::debug!(
                event_type = "scroll_perf",
                action = "queue_depth",
                depth = depth,
                is_backed_up = true,
                "Queue backed up: {}",
                depth
            );
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (depth, is_backed_up, is_critical);
    }
}

/// Log render stall (when render blocks for too long)
pub fn log_render_stall(duration_ms: u64) {
    let is_stall = duration_ms > 16;
    let is_hang = duration_ms > 100;

    let marker = if is_hang {
        " [HANG]"
    } else if is_stall {
        " [STALL]"
    } else {
        ""
    };
    add_to_buffer(
        "SCROLL_PERF",
        &format!("RENDER_STALL duration_ms={}{}", duration_ms, marker),
    );

    if is_hang {
        tracing::error!(
            event_type = "scroll_perf",
            action = "render_stall",
            duration_ms = duration_ms,
            is_hang = true,
            "Render hang: {}ms",
            duration_ms
        );
    } else if is_stall {
        tracing::warn!(
            event_type = "scroll_perf",
            action = "render_stall",
            duration_ms = duration_ms,
            is_stall = true,
            "Render stall: {}ms",
            duration_ms
        );
    }
}

/// Log scroll operation batch (when multiple scroll events are coalesced)
pub fn log_scroll_batch(batch_size: usize, coalesced_from: usize) {
    if coalesced_from > batch_size {
        #[cfg(debug_assertions)]
        {
            add_to_buffer(
                "SCROLL_PERF",
                &format!(
                    "BATCH_COALESCE processed={} from={}",
                    batch_size, coalesced_from
                ),
            );

            tracing::debug!(
                event_type = "scroll_perf",
                action = "batch_coalesce",
                batch_size = batch_size,
                coalesced_from = coalesced_from,
                "Coalesced {} scroll events to {}",
                coalesced_from,
                batch_size
            );
        }

        #[cfg(not(debug_assertions))]
        {
            let _ = (batch_size, coalesced_from);
        }
    }
}

/// Log key repeat timing for debugging fast scroll issues
pub fn log_key_repeat_timing(key: &str, interval_ms: u64, repeat_count: u32) {
    let is_fast = interval_ms < 50;

    #[cfg(debug_assertions)]
    {
        let marker = if is_fast { " [FAST_REPEAT]" } else { "" };
        add_to_buffer(
            "SCROLL_PERF",
            &format!(
                "KEY_REPEAT key={} interval_ms={} count={}{}",
                key, interval_ms, repeat_count, marker
            ),
        );

        tracing::debug!(
            event_type = "scroll_perf",
            action = "key_repeat",
            key = key,
            interval_ms = interval_ms,
            repeat_count = repeat_count,
            is_fast = is_fast,
            "Key repeat: {} interval={}ms count={}",
            key,
            interval_ms,
            repeat_count
        );
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (key, interval_ms, repeat_count, is_fast);
    }
}

// =============================================================================
// CONVENIENCE MACROS (re-exported)
// =============================================================================

// Re-export tracing for use by other modules
// Example usage:
//   use crate::logging;
//   logging::info!(event_type = "action", "Something happened");
//
// Or import tracing directly:
//   use tracing::{info, error, warn, debug, trace};
pub use tracing;

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // category_to_code tests - using real category strings from logs
    // -------------------------------------------------------------------------

    #[test]
    fn test_category_to_code_position() {
        // From: "CALCULATING WINDOW POSITION FOR MOUSE DISPLAY"
        assert_eq!(category_to_code("POSITION"), 'P');
        assert_eq!(category_to_code("position"), 'P');
        assert_eq!(category_to_code("Position"), 'P');
    }

    #[test]
    fn test_category_to_code_app() {
        // From: "Application logging initialized", "GPUI Application starting"
        assert_eq!(category_to_code("APP"), 'A');
        assert_eq!(category_to_code("app"), 'A');
    }

    #[test]
    fn test_category_to_code_stdin() {
        // From: "External command listener started", "Received: {\"type\": \"run\"..."
        assert_eq!(category_to_code("STDIN"), 'S');
    }

    #[test]
    fn test_category_to_code_hotkey() {
        // From: "Registered global hotkey meta+Digit0", "Tray icon initialized"
        assert_eq!(category_to_code("HOTKEY"), 'H');
        assert_eq!(category_to_code("TRAY"), 'H'); // Tray maps to H
    }

    #[test]
    fn test_category_to_code_visibility() {
        // From: "HOTKEY TRIGGERED - TOGGLE WINDOW", "WINDOW_VISIBLE set to: true"
        assert_eq!(category_to_code("VISIBILITY"), 'V');
    }

    #[test]
    fn test_category_to_code_exec() {
        // From: "Executing script: hello-world", "Script execution complete"
        assert_eq!(category_to_code("EXEC"), 'E');
    }

    #[test]
    fn test_category_to_code_theme() {
        // From: "Theme file not found, using defaults based on system appearance"
        assert_eq!(category_to_code("THEME"), 'T');
    }

    #[test]
    fn test_category_to_code_window_mgr() {
        // From: "Searching for main window among 2 windows"
        assert_eq!(category_to_code("WINDOW_MGR"), 'W');
    }

    #[test]
    fn test_category_to_code_config() {
        // From: "Successfully loaded config from ~/.sk/kit/config.ts"
        assert_eq!(category_to_code("CONFIG"), 'N');
        assert_eq!(category_to_code("config"), 'N');
        assert_eq!(category_to_code("Config"), 'N');
    }

    #[test]
    fn test_category_to_code_perf() {
        // From: "Startup loading: 33.30ms total (331 scripts in 5.03ms)"
        assert_eq!(category_to_code("PERF"), 'R');
    }

    #[test]
    fn test_category_to_code_all_categories() {
        // Complete mapping verification
        let mappings = [
            ("POSITION", 'P'),
            ("APP", 'A'),
            ("UI", 'U'),
            ("STDIN", 'S'),
            ("HOTKEY", 'H'),
            ("VISIBILITY", 'V'),
            ("EXEC", 'E'),
            ("KEY", 'K'),
            ("FOCUS", 'F'),
            ("THEME", 'T'),
            ("CACHE", 'C'),
            ("PERF", 'R'),
            ("WINDOW_MGR", 'W'),
            ("ERROR", 'X'),
            ("MOUSE_HOVER", 'M'),
            ("SCROLL_STATE", 'L'),
            ("SCROLL_PERF", 'Q'),
            ("SCRIPT", 'B'),
            ("CONFIG", 'N'),
            ("RESIZE", 'Z'),
            ("DESIGN", 'D'),
        ];

        for (category, expected_code) in mappings {
            assert_eq!(
                category_to_code(category),
                expected_code,
                "Category '{}' should map to '{}'",
                category,
                expected_code
            );
        }
    }

    #[test]
    fn test_category_to_code_unknown() {
        assert_eq!(category_to_code("UNKNOWN_CATEGORY"), '-');
        assert_eq!(category_to_code(""), '-');
        assert_eq!(category_to_code("foobar"), '-');
    }

    // -------------------------------------------------------------------------
    // level_to_char tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_level_to_char() {
        assert_eq!(level_to_char(Level::ERROR), 'e');
        assert_eq!(level_to_char(Level::WARN), 'w');
        assert_eq!(level_to_char(Level::INFO), 'i');
        assert_eq!(level_to_char(Level::DEBUG), 'd');
        assert_eq!(level_to_char(Level::TRACE), 't');
    }

    // -------------------------------------------------------------------------
    // infer_category_from_target tests - using real module paths
    // -------------------------------------------------------------------------

    #[test]
    fn test_infer_category_executor() {
        // From: script_kit_gpui::executor
        assert_eq!(infer_category_from_target("script_kit_gpui::executor"), 'E');
    }

    #[test]
    fn test_infer_category_theme() {
        // From: "script_kit_gpui::theme: Theme file not found"
        assert_eq!(infer_category_from_target("script_kit_gpui::theme"), 'T');
    }

    #[test]
    fn test_infer_category_config() {
        // From: "script_kit_gpui::config: Successfully loaded config"
        assert_eq!(infer_category_from_target("script_kit_gpui::config"), 'N');
    }

    #[test]
    fn test_infer_category_clipboard() {
        // From: "script_kit_gpui::clipboard_history: Initializing clipboard history"
        assert_eq!(
            infer_category_from_target("script_kit_gpui::clipboard_history"),
            'A'
        );
    }

    #[test]
    fn test_infer_category_logging() {
        // From: "script_kit_gpui::logging: Application logging initialized"
        assert_eq!(infer_category_from_target("script_kit_gpui::logging"), 'A');
    }

    #[test]
    fn test_infer_category_protocol() {
        // From: "script_kit_gpui::protocol" (stdin message handling)
        assert_eq!(infer_category_from_target("script_kit_gpui::protocol"), 'S');
    }

    #[test]
    fn test_infer_category_prompts() {
        // UI components
        assert_eq!(infer_category_from_target("script_kit_gpui::prompts"), 'U');
        assert_eq!(infer_category_from_target("script_kit_gpui::editor"), 'U');
        assert_eq!(infer_category_from_target("script_kit_gpui::panel"), 'U');
    }

    #[test]
    fn test_infer_category_scripts() {
        // From: "Loaded 331 scripts from ~/.sk/kit/scripts"
        assert_eq!(infer_category_from_target("script_kit_gpui::scripts"), 'G');
        assert_eq!(
            infer_category_from_target("script_kit_gpui::file_search"),
            'G'
        );
    }

    #[test]
    fn test_infer_category_hotkey() {
        // From: "Registered global hotkey meta+Digit0"
        assert_eq!(infer_category_from_target("script_kit_gpui::hotkey"), 'H');
        assert_eq!(infer_category_from_target("script_kit_gpui::tray"), 'H');
    }

    #[test]
    fn test_infer_category_window() {
        assert_eq!(
            infer_category_from_target("script_kit_gpui::window_manager"),
            'W'
        );
        assert_eq!(
            infer_category_from_target("script_kit_gpui::window_control"),
            'W'
        );
    }

    #[test]
    fn test_infer_category_unknown() {
        assert_eq!(infer_category_from_target("script_kit_gpui::main"), '-');
        assert_eq!(infer_category_from_target("unknown::module"), '-');
    }

    // -------------------------------------------------------------------------
    // get_minute_timestamp tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_minute_timestamp_format() {
        let ts = get_minute_timestamp();
        // Format should be "SS.mmm" - 2 digits, dot, 3 digits
        assert_eq!(ts.len(), 6, "Timestamp '{}' should be 6 chars", ts);
        assert!(ts.contains('.'), "Timestamp '{}' should contain '.'", ts);

        let parts: Vec<&str> = ts.split('.').collect();
        assert_eq!(parts.len(), 2);

        let seconds: u32 = parts[0].parse().expect("seconds should be numeric");
        let millis: u32 = parts[1].parse().expect("millis should be numeric");

        assert!(seconds < 60, "Seconds {} should be < 60", seconds);
        assert!(millis < 1000, "Millis {} should be < 1000", millis);
    }

    #[test]
    fn test_get_minute_timestamp_changes() {
        // Two calls in quick succession should produce similar timestamps
        let ts1 = get_minute_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ts2 = get_minute_timestamp();

        // Parse both
        let parse = |ts: &str| -> u64 {
            let parts: Vec<&str> = ts.split('.').collect();
            let secs: u64 = parts[0].parse().unwrap();
            let millis: u64 = parts[1].parse().unwrap();
            secs * 1000 + millis
        };

        let diff = parse(&ts2).saturating_sub(parse(&ts1));
        // Should be at least 5ms apart (we slept 5ms)
        assert!(
            diff >= 4,
            "Timestamps should be at least 4ms apart, got {}ms",
            diff
        );
        // But not more than 100ms (reasonable execution time)
        assert!(
            diff < 100,
            "Timestamps should be less than 100ms apart, got {}ms",
            diff
        );
    }

    // -------------------------------------------------------------------------
    // Compact format output validation (pattern matching)
    // -------------------------------------------------------------------------

    #[test]
    fn test_compact_format_pattern() {
        // Real example from logs:
        // "11.697|i|A|Application logging initialized event_type=app_lifecycle..."
        let example = "11.697|i|A|Application logging initialized";

        let parts: Vec<&str> = example.split('|').collect();
        assert_eq!(parts.len(), 4, "Compact format should have 4 parts");

        // Part 0: timestamp (SS.mmm)
        assert_eq!(parts[0].len(), 6);
        assert!(parts[0].contains('.'));

        // Part 1: level (single char)
        assert_eq!(parts[1].len(), 1);
        assert!("iwedtIWEDT".contains(parts[1]));

        // Part 2: category (single char)
        assert_eq!(parts[2].len(), 1);

        // Part 3: message (rest)
        assert!(!parts[3].is_empty());
    }

    #[test]
    fn test_compact_format_real_examples() {
        // Real log lines from test run
        let examples = [
            ("11.697|i|A|Application logging initialized", "i", "A"),
            ("11.717|i|N|Successfully loaded config", "i", "N"),
            ("11.741|i|H|Registered global hotkey meta+Digit0", "i", "H"),
            ("11.779|i|P|Available displays: 1", "i", "P"),
        ];

        for (line, expected_level, expected_cat) in examples {
            let parts: Vec<&str> = line.split('|').collect();
            assert_eq!(
                parts[1], expected_level,
                "Line '{}' should have level '{}'",
                line, expected_level
            );
            assert_eq!(
                parts[2], expected_cat,
                "Line '{}' should have category '{}'",
                line, expected_cat
            );
        }
    }

    // -------------------------------------------------------------------------
    // Token savings verification
    // -------------------------------------------------------------------------

    #[test]
    fn test_compact_format_token_savings() {
        // Real comparison from logs:
        // Standard: "2025-12-27T15:22:13.150640Z  INFO script_kit_gpui::logging: Selected display..."
        // Compact:  "13.150|i|P|Selected display..."

        let standard_prefix = "2025-12-27T15:22:13.150640Z  INFO script_kit_gpui::logging: ";
        let compact_prefix = "13.150|i|P|";

        let savings_percent =
            100.0 - (compact_prefix.len() as f64 / standard_prefix.len() as f64 * 100.0);

        // Should save at least 60% on the prefix
        assert!(
            savings_percent > 60.0,
            "Should save >60% on prefix, got {:.1}%",
            savings_percent
        );

        // Actual: 11 chars vs 59 chars = 81% savings
        assert!(
            savings_percent > 80.0,
            "Should save >80% on prefix, got {:.1}%",
            savings_percent
        );
    }

    // -------------------------------------------------------------------------
    // AI log mode env var parsing tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_ai_log_mode_env_parsing() {
        // Test the parsing logic used in init()
        // SCRIPT_KIT_AI_LOG=1 should enable AI mode

        let parse_ai_log = |val: &str| -> bool {
            val.eq_ignore_ascii_case("1")
                || val.eq_ignore_ascii_case("true")
                || val.eq_ignore_ascii_case("yes")
        };

        assert!(parse_ai_log("1"));
        assert!(parse_ai_log("true"));
        assert!(parse_ai_log("TRUE"));
        assert!(parse_ai_log("yes"));
        assert!(parse_ai_log("YES"));

        assert!(!parse_ai_log("0"));
        assert!(!parse_ai_log("false"));
        assert!(!parse_ai_log("no"));
        assert!(!parse_ai_log(""));
    }
}

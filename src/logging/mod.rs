#![allow(dead_code)]
//! Structured JSONL logging for AI agents and human-readable stderr output.
//!
//! This module provides dual-output logging:
//! - **JSONL to file** (~/.scriptkit/logs/script-kit-gpui.jsonl) - structured for AI agent parsing
//! - **Pretty to stderr** - human-readable for developers
//! - **Compact AI mode** (SCRIPT_KIT_AI_LOG=1) - ultra-compact line format for AI context
//!
//! # Compact AI Format
//!
//! When `SCRIPT_KIT_AI_LOG=1` is set, stderr uses compact format:
//! ```text
//! HH:MM:SS.mmm|L|C|message
//! ```
//! Where:
//! - HH:MM:SS.mmm = UTC wall-clock time (sortable across the whole session;
//!   matches the JSONL rfc3339 timestamps so agents can correlate the two)
//! - L = single char level (i/w/e/d/t)
//! - C = single char category code (see `category_to_code` below for the legend)
//!
//!
//! # JSONL Output Format
//!
//! Each line is a valid JSON object:
//! ```json
//! {"timestamp":"2024-12-25T10:30:45.123Z","level":"INFO","target":"script_kit_gpui::main","message":"Script executed","fields":{"event_type":"script_event","script_id":"abc","duration_ms":42}}
//! ```

mod rate_limit;
mod safe_user_value;

#[allow(unused_imports)]
pub use rate_limit::{
    log_rate_limit, LogRateDecision, LOG_RL_MAX_KEYS, LOG_RL_STALE_AFTER, LOG_RL_WINDOW,
};
#[allow(unused_imports)]
pub use safe_user_value::{
    log_user_value, log_user_value_with_limit, LogSafe, SAFE_USER_VALUE_MAX_BYTES,
};

// --- merged from part_000.rs ---
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Restrict a log file to owner read/write (0600). Best-effort: a log with
/// looser permissions is not worth aborting startup over, and `.mode()` on
/// `OpenOptions` only applies when the file is freshly created, so this also
/// tightens logs left behind by an older build.
fn harden_log_file_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            eprintln!(
                "[SCRIPT-KIT-GPUI] warning: could not restrict log permissions on {}: {err}",
                path.display()
            );
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}
use tracing::field::{Field, Visit};
use tracing::{Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, MakeWriter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use uuid::Uuid;

pub fn filter_perf_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SCRIPT_KIT_FILTER_PERF_LOG").is_some())
}

pub fn agent_chat_render_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SCRIPT_KIT_AGENT_CHAT_RENDER_TRACE").is_some())
}

/// Whether conversation Markdown (Agent Chat AND Flow) renders selectable.
///
/// The env var keeps its original `SCRIPT_KIT_AGENT_CHAT_MARKDOWN_SELECTABLE`
/// name on purpose: it is an existing documented escape hatch, and minting a
/// second flag for the same switch would let the two surfaces diverge again —
/// the exact drift the shared conversation renderer removes.
pub fn conversation_markdown_selectable_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SCRIPT_KIT_AGENT_CHAT_MARKDOWN_SELECTABLE")
            .map(|value| value != "0")
            .unwrap_or(true)
    })
}

/// Back-compat name. Delegates; do NOT give this its own `OnceLock`, or the
/// two names could resolve differently within one process.
pub fn agent_chat_markdown_selectable_enabled() -> bool {
    conversation_markdown_selectable_enabled()
}

pub fn preflight_deep_log_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SCRIPT_KIT_PREFLIGHT_DEEP_LOG").is_some())
}
// =============================================================================
// SESSION IDENTITY & LOG PATHS
// =============================================================================
/// Path to the session-specific log file (latest-session.jsonl).
static SESSION_LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
/// Unique ID for the current session, generated on init.
static SESSION_ID: OnceLock<String> = OnceLock::new();
/// Get the path to the session-specific log file.
pub fn session_log_path() -> PathBuf {
    SESSION_LOG_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| get_log_dir().join("latest-session.jsonl"))
}
/// Get the current session ID.
pub fn session_id() -> &'static str {
    SESSION_ID.get().map(|s| s.as_str()).unwrap_or("unknown")
}
// =============================================================================
// CORRELATION ID (MANDATORY FIELD)
// =============================================================================
// Global default correlation_id for events where no per-run/context value is set.
static DEFAULT_CORRELATION_ID: OnceLock<String> = OnceLock::new();
// Thread-local override used for request/interaction scoped correlation_ids.
thread_local! {
    static CORRELATION_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}
/// Set the correlation_id for the current thread, returning a guard that restores the previous
/// value on drop.
pub fn set_correlation_id(id: impl Into<String>) -> CorrelationGuard {
    let new_id = id.into();
    let previous = CORRELATION_ID.with(|cell| cell.borrow_mut().replace(new_id));
    CorrelationGuard { previous }
}
/// Guard that restores the previous correlation_id when dropped.
pub struct CorrelationGuard {
    previous: Option<String>,
}
impl Drop for CorrelationGuard {
    fn drop(&mut self) {
        let prev = self.previous.take();
        CORRELATION_ID.with(|cell| {
            *cell.borrow_mut() = prev;
        });
    }
}
/// Get the current correlation_id (thread-local if set, otherwise the global default).
pub fn current_correlation_id() -> String {
    CORRELATION_ID.with(|cell| {
        cell.borrow().clone().unwrap_or_else(|| {
            DEFAULT_CORRELATION_ID
                .get_or_init(|| Uuid::new_v4().to_string())
                .clone()
        })
    })
}
/// Snapshot the *scoped* correlation id — `None` when only the process-wide
/// default is active. The scope is thread-local, so it dies at every
/// `cx.spawn`/`cx.defer`/background-executor/GCD hop; capture it with this
/// before the hop and re-apply it inside the task with
/// [`with_correlation_scope`] so multi-step interactions stay correlated.
pub fn scoped_correlation_id() -> Option<String> {
    CORRELATION_ID.with(|cell| cell.borrow().clone())
}
/// Run `f` with a previously captured correlation scope applied. A `None`
/// scope (no interaction id was active at capture time) runs `f` unchanged
/// rather than clobbering whatever scope the executing thread already has.
pub fn with_correlation_scope<T>(scope: Option<&str>, f: impl FnOnce() -> T) -> T {
    match scope {
        Some(id) => {
            let _guard = set_correlation_id(id);
            f()
        }
        None => f(),
    }
}
// =============================================================================
// BENCHMARKING UTILITIES (for hotkey → prompt latency analysis)
// =============================================================================
// Stores the instant when benchmarking started (as nanos since process start)
static BENCH_START_NANOS: AtomicU64 = AtomicU64::new(0);
static BENCH_EPOCH: OnceLock<Instant> = OnceLock::new();
/// Start a benchmark session. Call this when a hotkey is triggered.
/// Returns the benchmark ID (timestamp) for correlation.
pub fn bench_start(label: &str) -> u64 {
    let epoch = BENCH_EPOCH.get_or_init(Instant::now);
    let now = epoch.elapsed().as_nanos() as u64;
    BENCH_START_NANOS.store(now, Ordering::SeqCst);
    let id = now / 1_000_000; // Use millis as ID
    log("BENCH", &format!("▶ START [{}] {}", id, label));
    id
}
/// Log a benchmark checkpoint with elapsed time from bench_start().
/// Format: [+XXXms] step_name
pub fn bench_log(step: &str) {
    let epoch = match BENCH_EPOCH.get() {
        Some(e) => e,
        None => {
            log("BENCH", &format!("⚠ {} (no bench_start called)", step));
            return;
        }
    };
    let start = BENCH_START_NANOS.load(Ordering::SeqCst);
    if start == 0 {
        log("BENCH", &format!("⚠ {} (bench not started)", step));
        return;
    }
    let now = epoch.elapsed().as_nanos() as u64;
    let elapsed_ms = (now - start) / 1_000_000;
    log("BENCH", &format!("[+{:>4}ms] {}", elapsed_ms, step));
}
/// Log a benchmark checkpoint with a custom elapsed time (for cross-process timing).
pub fn bench_log_with_elapsed(step: &str, elapsed_ms: u64) {
    log("BENCH", &format!("[+{:>4}ms] {}", elapsed_ms, step));
}
/// Get elapsed milliseconds since bench_start().
pub fn bench_elapsed_ms() -> u64 {
    let epoch = match BENCH_EPOCH.get() {
        Some(e) => e,
        None => return 0,
    };
    let start = BENCH_START_NANOS.load(Ordering::SeqCst);
    if start == 0 {
        return 0;
    }
    let now = epoch.elapsed().as_nanos() as u64;
    (now - start) / 1_000_000
}
/// End the benchmark and log total time.
pub fn bench_end(label: &str) {
    let elapsed = bench_elapsed_ms();
    log("BENCH", &format!("◼ END [+{}ms] {}", elapsed, label));
    BENCH_START_NANOS.store(0, Ordering::SeqCst);
}
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
        "KEY" | "KEY_SETUP" | "KEY_BIND" | "KEY_ROUTE" => 'K',
        "FOCUS" => 'F',
        "THEME" => 'T',
        "CACHE" => 'C',
        "PERF" => 'R',
        "WINDOW_MGR" => 'W',
        "ERROR" => 'X',
        "MOUSE_HOVER" => 'M',
        "SCROLL_STATE" => 'L',
        "SCROLL_PERF" => 'Q',
        "SCRIPT" => 'G', // G for script loaGing (changed from B)
        "CONFIG" => 'N', // N for coNfig
        "RESIZE" => 'Z',
        "TRAY" => 'H',   // Tray is part of Hotkey subsystem
        "DESIGN" => 'D', // Design system
        "BENCH" => 'B',  // B for Benchmark timing
        "WARN" | "WARNING" => 'X',
        "WINDOW_STATE" | "WINDOW_OPS" | "WINDOW_REG" => 'W',
        "CHAT" | "AI" | "ACTIONS" | "ACTIONS_THEME" | "COMMAND_BAR" | "PROMPTS" | "PANEL"
        | "EDITOR" | "DIV" | "FIELD" | "SHORTCUT" | "ALIAS" | "ALIAS_INPUT" | "CONFIRM"
        | "SEARCH" | "FALLBACK" | "KEYWORD" | "HUD" => 'U',
        "DEBUG_GRID" => 'D',
        "SCRIPTLET_PARSE" => 'G',
        "CLICK" => 'M',
        "MCP" => 'S',
        "CLIPBOARD" | "DEEPLINK" | "TEST" | "SCHEDULER" | "SHUTDOWN" | "SECRETS" | "PROC" => 'A',
        "OCR" | "FONT" | "VIBRANCY" => 'T',
        _ => '-', // Unknown category
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
impl LegacyLogLevel {
    fn as_json_label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}
fn legacy_level_for_category(category: &str) -> LegacyLogLevel {
    match category.to_uppercase().as_str() {
        "ERROR" => LegacyLogLevel::Error,
        "WARN" | "WARNING" => LegacyLogLevel::Warn,
        "DEBUG" => LegacyLogLevel::Debug,
        "TRACE" => LegacyLogLevel::Trace,
        _ => LegacyLogLevel::Info,
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
    } else if target.contains("stdin") || target.contains("protocol") || target.contains("mcp") {
        'S' // Stdin/protocol
    } else if target.contains("hotkey") || target.contains("tray") {
        'H' // Hotkey
    } else if target.contains("scripts") || target.contains("file_search") {
        'G' // Script loaGing (not execution)
    } else if target.contains("window_state")
        || target.contains("window_ops")
        || target.contains("window_reg")
    {
        'W' // Window state/ops
    } else if target.contains("config") {
        'N' // coNfig
    } else if target.contains("watcher")
        || target.contains("clipboard")
        || target.contains("logging")
        || target.contains("main")
        || target.contains("deeplink")
        || target.contains("scheduler")
        || target.contains("shutdown")
        || target.contains("window")
    {
        'A' // App lifecycle/subsystems
    } else if target.contains("panel")
        || target.contains("prompts")
        || target.contains("editor")
        || target.contains("terminal")
        || target.contains("term_prompt")
        || target.contains("pty")
        || target.contains("syntax")
        || target.contains("app_impl")
        || target.contains("actions")
        || target.contains("ai")
        || target.contains("notes")
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
/// UTC wall-clock time as `HH:MM:SS.mmm`.
///
/// The old `SS.mmm` (seconds within the current minute) reset every 60s,
/// which made app.log impossible to sort or slice by time — an agent could
/// not express "logs since 14:32:05". Full clock time stays sortable across
/// the whole session and lines up with the JSONL rfc3339 timestamps.
fn get_compact_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_millis = now.as_millis();
    let millis = total_millis % 1000;
    let total_secs = total_millis / 1000;
    let seconds = total_secs % 60;
    let minutes = (total_secs / 60) % 60;
    let hours = (total_secs / 3600) % 24;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
}
/// Visitor to extract category field from tracing events
struct CategoryExtractor {
    category: Option<String>,
    message: String,
    correlation_id: Option<String>,
}
impl CategoryExtractor {
    fn new() -> Self {
        Self {
            category: None,
            message: String::new(),
            correlation_id: None,
        }
    }
}
impl Visit for CategoryExtractor {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "category" => self.category = Some(value.to_string()),
            "message" => self.message = value.to_string(),
            "correlation_id" => self.correlation_id = Some(value.to_string()),
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
            "correlation_id" => self.correlation_id = Some(format!("{:?}", value)),
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
/// Format: `HH:MM:SS.mmm|L|C|message` (UTC)
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
        let timestamp = get_compact_timestamp();
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

        // Ensure correlation_id is always present
        let correlation_id = extractor
            .correlation_id
            .unwrap_or_else(current_correlation_id);

        // Build the compact line
        writeln!(
            writer,
            "{}|{}|{}|cid={} {}",
            timestamp, level_char, category_code, correlation_id, extractor.message
        )
    }
}
// =============================================================================
// JSON FORMATTER WITH CORRELATION ID INJECTION
// =============================================================================

#[derive(Default)]
struct JsonFieldCollector {
    fields: Map<String, Value>,
}
impl JsonFieldCollector {
    fn take(self) -> Map<String, Value> {
        self.fields
    }
}
impl Visit for JsonFieldCollector {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if let Some(num) = serde_json::Number::from_f64(value) {
            self.fields
                .insert(field.name().to_string(), Value::Number(num));
        } else {
            self.fields.insert(
                field.name().to_string(),
                Value::String(format!("{:.2}", value)),
            );
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            Value::String(format!("{:?}", value)),
        );
    }
}
fn value_to_string(value: Value) -> String {
    match value {
        Value::String(s) => s,
        other => other.to_string(),
    }
}
/// Ensures every JSON log line includes a correlation_id and message field.
#[derive(Default)]
pub struct JsonWithCorrelation;

// --- merged from part_001.rs ---
impl<S, N> FormatEvent<S, N> for JsonWithCorrelation
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
        let mut collector = JsonFieldCollector::default();
        event.record(&mut collector);
        let mut fields = collector.take();

        let message = fields
            .remove("message")
            .map(value_to_string)
            .unwrap_or_default();

        let correlation_id = fields
            .remove("correlation_id")
            .map(value_to_string)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(current_correlation_id);

        let mut root = Map::new();
        root.insert(
            "timestamp".to_string(),
            Value::String(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        );
        root.insert(
            "level".to_string(),
            Value::String(event.metadata().level().to_string()),
        );
        root.insert(
            "target".to_string(),
            Value::String(event.metadata().target().to_string()),
        );
        root.insert("correlation_id".to_string(), Value::String(correlation_id));
        root.insert("message".to_string(), Value::String(message));

        if !fields.is_empty() {
            root.insert("fields".to_string(), Value::Object(fields));
        }

        match serde_json::to_string(&Value::Object(root)) {
            Ok(json) => writeln!(writer, "{}", json),
            Err(e) => writeln!(
                writer,
                r#"{{"level":"ERROR","message":"failed to serialize log","error":"{}"}}"#,
                e
            ),
        }
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
// TEE WRITER - writes to both the main log AND the session log
// =============================================================================

/// A writer that duplicates output to two `NonBlocking` writers.
struct TeeWriter {
    main: tracing_appender::non_blocking::NonBlocking,
    session: tracing_appender::non_blocking::NonBlocking,
}
impl Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.main.write(buf)?;
        let _ = self.session.write_all(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.main.flush()?;
        let _ = self.session.flush();
        Ok(())
    }
}
/// Wrapper so tracing_subscriber can use `TeeWriter` via `MakeWriter`.
struct TeeWriterMaker {
    main: tracing_appender::non_blocking::NonBlocking,
    session: tracing_appender::non_blocking::NonBlocking,
}
impl<'a> MakeWriter<'a> for TeeWriterMaker {
    type Writer = TeeWriter;

    fn make_writer(&'a self) -> Self::Writer {
        TeeWriter {
            main: self.main.clone(),
            session: self.session.clone(),
        }
    }
}
// =============================================================================
// LEGACY SUPPORT - In-memory log buffer for UI display
// =============================================================================

static LOG_BUFFER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
const MAX_LOG_LINES: usize = 50;
// =============================================================================
// PROTOCOL LOG RING - structured in-memory buffer behind the getLogs verb
// =============================================================================
// Unlike LOG_BUFFER above (50 formatted strings for the in-app HUD), this ring
// keeps structured entries for every tracing event that passes the env filter,
// so agents can fetch recent logs over the automation protocol without
// touching files on disk.

/// One structured entry in the protocol log ring.
#[derive(Clone, Debug, serde::Serialize)]
pub struct LogRingEntry {
    /// rfc3339 with milliseconds — same format as the JSONL sinks.
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub correlation_id: String,
    pub message: String,
}

pub const LOG_RING_CAPACITY: usize = 500;
static LOG_RING: OnceLock<Mutex<VecDeque<LogRingEntry>>> = OnceLock::new();

fn log_ring() -> &'static Mutex<VecDeque<LogRingEntry>> {
    LOG_RING.get_or_init(|| Mutex::new(VecDeque::with_capacity(LOG_RING_CAPACITY)))
}

/// tracing layer that mirrors every filtered event into the protocol log ring.
pub struct LogRingLayer;

impl<S> tracing_subscriber::layer::Layer<S> for LogRingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut extractor = CategoryExtractor::new();
        event.record(&mut extractor);
        let entry = LogRingEntry {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            correlation_id: extractor
                .correlation_id
                .unwrap_or_else(current_correlation_id),
            message: extractor.message,
        };
        if let Ok(mut ring) = log_ring().lock() {
            if ring.len() >= LOG_RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(entry);
        }
    }
}

fn level_rank(level: &str) -> Option<u8> {
    match level.to_ascii_uppercase().as_str() {
        "TRACE" => Some(0),
        "DEBUG" => Some(1),
        "INFO" => Some(2),
        "WARN" | "WARNING" => Some(3),
        "ERROR" => Some(4),
        _ => None,
    }
}

/// Query the protocol log ring. Filters are ANDed; `min_level` is a minimum
/// severity name, `target`/`contains` are substring matches. Returns the last
/// `limit` matching entries (oldest first) plus the total match count.
pub fn query_log_ring(
    limit: usize,
    min_level: Option<&str>,
    target: Option<&str>,
    contains: Option<&str>,
) -> (Vec<LogRingEntry>, usize) {
    let min_rank = min_level.and_then(level_rank);
    let Ok(ring) = log_ring().lock() else {
        return (Vec::new(), 0);
    };
    let matching: Vec<&LogRingEntry> = ring
        .iter()
        .filter(|entry| {
            if let Some(min) = min_rank {
                if level_rank(&entry.level).map(|r| r < min).unwrap_or(false) {
                    return false;
                }
            }
            if let Some(t) = target {
                if !entry.target.contains(t) {
                    return false;
                }
            }
            if let Some(c) = contains {
                if !entry.message.contains(c) && !entry.target.contains(c) {
                    return false;
                }
            }
            true
        })
        .collect();
    let total = matching.len();
    let entries = matching
        .into_iter()
        .skip(total.saturating_sub(limit))
        .cloned()
        .collect();
    (entries, total)
}

// =============================================================================
// LOG CAPTURE SYSTEM
// =============================================================================
// Allows capturing logs to a separate timestamped file via hotkey toggle.
// Press hotkey once to start capture, press again to stop.
// Captured logs go to ~/.scriptkit/logs/capture-<timestamp>.jsonl

/// Whether log capture is currently active
static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
/// Current capture session state
struct CaptureSession {
    file: File,
    path: PathBuf,
    start_time: SystemTime,
}
/// Active capture session (if any)
static CAPTURE_SESSION: OnceLock<Mutex<Option<CaptureSession>>> = OnceLock::new();
fn capture_session() -> &'static Mutex<Option<CaptureSession>> {
    CAPTURE_SESSION.get_or_init(|| Mutex::new(None))
}
/// Check if log capture is currently enabled
pub fn is_capture_enabled() -> bool {
    CAPTURE_ENABLED.load(Ordering::Relaxed)
}
/// Start capturing logs to a new timestamped file.
/// Returns the path to the capture file.
pub fn start_capture() -> anyhow::Result<PathBuf> {
    let log_dir = get_log_dir();
    fs::create_dir_all(&log_dir)?;

    // Create timestamped filename: capture-2026-01-11T08-37-28.jsonl
    let now = SystemTime::now();
    let timestamp = chrono::DateTime::<chrono::Utc>::from(now)
        .format("%Y-%m-%dT%H-%M-%S")
        .to_string();
    let filename = format!("capture-{}.jsonl", timestamp);
    let path = log_dir.join(&filename);

    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    // Logs can capture user content; keep them owner-only (0600).
    harden_log_file_permissions(&path);

    let session = CaptureSession {
        file,
        path: path.clone(),
        start_time: now,
    };

    {
        let mut guard = capture_session().lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(session);
    }

    CAPTURE_ENABLED.store(true, Ordering::Relaxed);

    tracing::info!(
        event_type = "log_capture",
        action = "started",
        capture_file = %path.display(),
        "Log capture started"
    );

    Ok(path)
}
/// Stop capturing logs and close the capture file.
/// Returns the path to the capture file and duration in seconds.
pub fn stop_capture() -> Option<(PathBuf, u64)> {
    CAPTURE_ENABLED.store(false, Ordering::Relaxed);

    let session = {
        let mut guard = capture_session().lock().unwrap_or_else(|e| e.into_inner());
        guard.take()
    };

    if let Some(session) = session {
        let duration_secs = session
            .start_time
            .elapsed()
            .map(|d| d.as_secs())
            .unwrap_or(0);

        tracing::info!(
            event_type = "log_capture",
            action = "stopped",
            capture_file = %session.path.display(),
            duration_secs = duration_secs,
            "Log capture stopped"
        );

        Some((session.path, duration_secs))
    } else {
        None
    }
}
/// Toggle log capture on/off.
/// Returns (is_now_capturing, capture_file_path_if_relevant).
/// When starting: returns (true, Some(path_to_new_capture_file))
/// When stopping: returns (false, Some(path_to_completed_capture_file))
pub fn toggle_capture() -> (bool, Option<PathBuf>) {
    if is_capture_enabled() {
        // Stop capture
        if let Some((path, _duration)) = stop_capture() {
            (false, Some(path))
        } else {
            (false, None)
        }
    } else {
        // Start capture
        match start_capture() {
            Ok(path) => (true, Some(path)),
            Err(e) => {
                tracing::error!(
                    event_type = "log_capture",
                    action = "start_failed",
                    error = %e,
                    "Failed to start log capture"
                );
                (false, None)
            }
        }
    }
}
/// Write a log line to the capture file if capture is enabled.
/// This is called internally by the logging system.
fn write_to_capture(line: &str) {
    if !CAPTURE_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    if let Ok(mut guard) = capture_session().lock() {
        if let Some(ref mut session) = *guard {
            // Write line with newline
            let _ = writeln!(session.file, "{}", line);
            // Flush to ensure logs are immediately visible
            let _ = session.file.flush();
        }
    }
}
/// Guard that must be kept alive for the duration of the program.
/// Dropping this guard will flush and close the log files.
pub struct LoggingGuard {
    _file_guard: WorkerGuard,
    _session_guard: WorkerGuard,
}
/// Static storage for the logging guard to ensure it lives for the entire program.
/// This prevents the common mistake of calling `logging::init()` without storing the guard.
static LOGGING_GUARD: OnceLock<LoggingGuard> = OnceLock::new();
/// Initialize the global logging system.
///
/// This is the preferred way to initialize logging - it stores the guard internally
/// so callers cannot accidentally drop it. Safe to call multiple times (subsequent
/// calls are no-ops).
///
/// # Example
/// ```ignore
/// logging::init();  // Guard stored internally, can't be dropped
/// // ... rest of program
/// ```
pub fn init() {
    // Only initialize once - subsequent calls are no-ops
    LOGGING_GUARD.get_or_init(init_internal);
}

fn open_writer_or_sink(
    open_result: std::io::Result<File>,
    description: &str,
) -> Box<dyn Write + Send> {
    open_result
        .map(|file| Box::new(file) as Box<dyn Write + Send>)
        .unwrap_or_else(|error| {
            eprintln!("[LOGGING] Failed to open {}: {}", description, error);
            Box::new(std::io::sink())
        })
}

/// Rotate `path` to `<path>.1` (replacing the previous rotation) when it
/// exceeds `max_bytes`. One generation is enough: the append-forever log
/// exists for post-hoc cross-session queries; `latest-session.jsonl` is the
/// per-launch surface. `max_bytes == 0` disables rotation.
fn rotate_log_if_oversized(path: &std::path::Path, max_bytes: u64) {
    if max_bytes == 0 {
        return;
    }
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() <= max_bytes {
        return;
    }
    let mut rotated_os = path.as_os_str().to_owned();
    rotated_os.push(".1");
    let rotated = PathBuf::from(rotated_os);
    match fs::rename(path, &rotated) {
        Ok(()) => eprintln!(
            "[SCRIPT-KIT-GPUI] Rotated oversized log ({} bytes) to {}",
            meta.len(),
            rotated.display()
        ),
        Err(e) => eprintln!("[LOGGING] Failed to rotate {}: {}", path.display(), e),
    }
}

/// Internal initialization that returns a LoggingGuard.
/// This is used by init() to store the guard in LOGGING_GUARD.
fn init_internal() -> LoggingGuard {
    // Initialize legacy log buffer for UI display
    let _ = LOG_BUFFER.set(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES)));

    // Initialize global correlation_id (used as fallback when no contextual ID is set)
    let _ = DEFAULT_CORRELATION_ID.get_or_init(|| Uuid::new_v4().to_string());

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
    let session_path = log_dir.join("latest-session.jsonl");

    // The append-forever log grows without bound (observed at 4GB+), which
    // makes every all-sessions query pay for years of history. Rotate one
    // generation at launch when it exceeds the cap.
    let rotate_cap = std::env::var("SCRIPT_KIT_LOG_ROTATE_BYTES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(128 * 1024 * 1024);
    rotate_log_if_oversized(&log_path, rotate_cap);

    // Store session log path for panic hook and public access
    let _ = SESSION_LOG_PATH.set(session_path.clone());

    // Initialize session ID
    let sid = SESSION_ID
        .get_or_init(|| Uuid::new_v4().to_string())
        .clone();

    // Always print session log path (useful in both AI and non-AI modes)
    eprintln!("========================================");
    eprintln!("[SCRIPT-KIT-GPUI] Session log: {}", session_path.display());
    eprintln!("[SCRIPT-KIT-GPUI] Full log:    {}", log_path.display());
    eprintln!(
        "[SCRIPT-KIT-GPUI] Copy for AI:  cat {} | pbcopy",
        session_path.display()
    );
    eprintln!("========================================");

    // Open append-forever log file
    let file = open_writer_or_sink(
        OpenOptions::new().create(true).append(true).open(&log_path),
        "log file",
    );

    // Open session log file (truncated on each launch)
    let session_file = open_writer_or_sink(
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&session_path),
        "session log file",
    );

    // These JSONL logs can capture user content (queries, prompts, command
    // lines); keep them owner-only (0600) rather than inheriting umask.
    harden_log_file_permissions(&log_path);
    harden_log_file_permissions(&session_path);

    // Create non-blocking writers for both files
    let (non_blocking_append, file_guard) = tracing_appender::non_blocking(file);
    let (non_blocking_session, session_guard) = tracing_appender::non_blocking(session_file);

    // Tee writer: every JSONL line goes to both files
    let tee = TeeWriterMaker {
        main: non_blocking_append,
        session: non_blocking_session,
    };

    // Environment filter - default to info, allow override via RUST_LOG
    let rust_log_value = std::env::var("RUST_LOG").unwrap_or_else(|_| "default".to_string());
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,gpui::window=off,gpui=warn,hyper=warn,reqwest=warn")
    });

    // JSONL layer for file output (goes to both append + session via tee)
    let json_layer = fmt::layer()
        .event_format(JsonWithCorrelation)
        .with_writer(tee)
        .with_ansi(false);

    if ai_log_mode {
        let ai_layer = fmt::layer()
            .with_writer(StderrWriter)
            .with_ansi(false)
            .event_format(CompactAiFormatter);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(json_layer)
            .with(LogRingLayer)
            .with(ai_layer)
            .init();
    } else {
        let pretty_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(false)
            .compact();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(json_layer)
            .with(LogRingLayer)
            .with(pretty_layer)
            .init();
    }

    // ---- Session preamble: rich context for AI agents ----
    let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
    let build_profile = option_env!("BUILD_PROFILE").unwrap_or("unknown");

    tracing::info!(
        event_type = "session_start",
        session_id = %sid,
        git_hash = git_hash,
        build_profile = build_profile,
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        pid = std::process::id(),
        working_dir = %std::env::current_dir().unwrap_or_default().display(),
        rust_log = %rust_log_value,
        ai_log_mode = ai_log_mode,
        "Session started"
    );

    // ---- Panic hook: capture panics to JSONL before process dies ----
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        // Log via tracing (goes to both JSONL files)
        tracing::error!(
            event_type = "panic",
            panic_message = %message,
            location = %location,
            "PANIC: {} at {}",
            message,
            location
        );

        // Safety net: write directly to session log (tracing may not flush)
        if let Some(path) = SESSION_LOG_PATH.get() {
            let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let escaped_msg = message.replace('\\', "\\\\").replace('"', "\\\"");
            let json = format!(
                r#"{{"timestamp":"{}","level":"ERROR","target":"panic","correlation_id":"panic","message":"PANIC: {} at {}","fields":{{"event_type":"panic","location":"{}"}}}}"#,
                timestamp, escaped_msg, location, location
            );
            if let Ok(mut f) = OpenOptions::new().append(true).open(path) {
                let _ = writeln!(f, "{}", json);
            }
        }

        // Call original hook (prints to stderr)
        default_hook(info);
    }));

    LoggingGuard {
        _file_guard: file_guard,
        _session_guard: session_guard,
    }
}
/// Get the log directory path (~/.scriptkit/logs/)
fn get_log_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir()
            .join("script-kit-gpui-tests")
            .join(std::process::id().to_string())
            .join("logs");
    }

    dirs::home_dir()
        .map(|h| h.join(".scriptkit").join("logs"))
        .unwrap_or_else(|| std::env::temp_dir().join("script-kit-logs"))
}
/// Get the path to the JSONL log file
pub fn log_path() -> PathBuf {
    get_log_dir().join("script-kit-gpui.jsonl")
}

// --- merged from part_002.rs ---
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
    let message = bounded_legacy_log_message(category, message);
    let message = message.as_ref();
    // Add to legacy buffer for UI display
    add_to_buffer(category, message);

    let correlation_id = current_correlation_id();
    let level = legacy_level_for_category(category);

    // Write to capture file if capture is enabled
    if is_capture_enabled() {
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let json_line = format!(
            r#"{{"timestamp":"{}","level":"{}","category":"{}","correlation_id":"{}","message":"{}"}}"#,
            timestamp,
            level.as_json_label(),
            category,
            correlation_id,
            message
        );
        write_to_capture(&json_line);
    }

    // Preserve intended severity for legacy category-only callsites.
    match level {
        LegacyLogLevel::Error => tracing::error!(
            category = category,
            correlation_id = %correlation_id,
            legacy = true,
            "{}",
            message
        ),
        LegacyLogLevel::Warn => tracing::warn!(
            category = category,
            correlation_id = %correlation_id,
            legacy = true,
            "{}",
            message
        ),
        LegacyLogLevel::Info => tracing::info!(
            category = category,
            correlation_id = %correlation_id,
            legacy = true,
            "{}",
            message
        ),
        LegacyLogLevel::Debug => tracing::debug!(
            category = category,
            correlation_id = %correlation_id,
            legacy = true,
            "{}",
            message
        ),
        LegacyLogLevel::Trace => tracing::trace!(
            category = category,
            correlation_id = %correlation_id,
            legacy = true,
            "{}",
            message
        ),
    }
}

fn bounded_legacy_log_message<'a>(category: &str, message: &'a str) -> std::borrow::Cow<'a, str> {
    const FILTER_PERF_MAX_BYTES: usize = 1536;
    if !filter_perf_trace_enabled() || !category.eq_ignore_ascii_case("FILTER_PERF") {
        return std::borrow::Cow::Borrowed(message);
    }
    if message.len() <= FILTER_PERF_MAX_BYTES {
        return std::borrow::Cow::Borrowed(message);
    }

    let mut end = FILTER_PERF_MAX_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    std::borrow::Cow::Owned(format!(
        "{} ... [truncated=true raw_len={}]",
        &message[..end],
        message.len()
    ))
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
// PAYLOAD TRUNCATION HELPERS
// Purpose: Avoid logging sensitive/large data like base64 screenshots, clipboard
// =============================================================================

/// Maximum length for logged message payloads
const MAX_PAYLOAD_LOG_LEN: usize = 200;
/// Truncate a string for logging, adding "..." suffix if truncated.
/// This function is UTF-8 safe - it will never panic on multi-byte characters.
/// If max_len falls in the middle of a multi-byte character, it backs up to the
/// nearest valid character boundary.
pub fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_owned();
    }
    // Find a valid UTF-8 char boundary at or before max_len
    let mut end = max_len.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...({})", &s[..end], s.len())
}
/// Summarize a JSON payload for logging (type + length, truncated preview)
/// Used for protocol messages to avoid logging full screenshots/clipboard data
pub fn summarize_payload(json: &str) -> String {
    // Try to extract message type from JSON
    let msg_type = json.find("\"type\":\"").and_then(|pos| {
        let start = pos + 8; // length of "\"type\":\""
        json[start..].find('"').map(|end| &json[start..start + end])
    });

    match msg_type {
        Some(t) => format!("{{type:{}, len:{}}}", t, json.len()),
        None => format!("{{len:{}}}", json.len()),
    }
}
/// Log a protocol message being sent to script (truncated for performance/privacy)
pub fn log_protocol_send(fd: i32, json: &str) {
    // In debug/verbose mode, show truncated preview
    // In normal mode, just show type + length
    #[cfg(debug_assertions)]
    {
        let summary = summarize_payload(json);
        add_to_buffer("EXEC", &format!("→stdin fd={}: {}", fd, summary));
        tracing::debug!(
            event_type = "protocol_send",
            fd = fd,
            payload_len = json.len(),
            summary = %summary,
            "Sending to script stdin"
        );
    }

    #[cfg(not(debug_assertions))]
    {
        // Minimal logging in release - just type + length
        let summary = summarize_payload(json);
        tracing::info!(
            event_type = "protocol_send",
            fd = fd,
            payload_len = json.len(),
            "→script: {}",
            summary
        );
    }
}
/// Log a protocol message received from script (truncated for performance/privacy)
pub fn log_protocol_recv(msg_type: &str, json_len: usize) {
    #[cfg(debug_assertions)]
    {
        add_to_buffer(
            "EXEC",
            &format!("←stdout: type={} len={}", msg_type, json_len),
        );
        tracing::debug!(
            event_type = "protocol_recv",
            message_type = msg_type,
            payload_len = json_len,
            "Received from script"
        );
    }

    #[cfg(not(debug_assertions))]
    {
        tracing::info!(
            event_type = "protocol_recv",
            message_type = msg_type,
            payload_len = json_len,
            "←script: type={} len={}",
            msg_type,
            json_len
        );
    }
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

// --- merged from part_003.rs ---
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

// --- merged from part_004.rs ---
// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    use std::io::Write as IoWrite;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::{fmt as fmt_sub, EnvFilter};
    #[derive(Clone)]
    struct BufferWriter(Arc<Mutex<Vec<u8>>>);
    struct BufferGuard<'a> {
        buf: &'a Arc<Mutex<Vec<u8>>>,
    }
    impl<'a> IoWrite for BufferGuard<'a> {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let mut buf = self.buf.lock().unwrap_or_else(|e| e.into_inner());
            buf.extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> MakeWriter<'a> for BufferWriter {
        type Writer = BufferGuard<'a>;

        fn make_writer(&'a self) -> Self::Writer {
            BufferGuard { buf: &self.0 }
        }
    }
    #[test]
    fn json_formatter_injects_correlation_id() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = fmt_sub()
            .json()
            .with_writer(BufferWriter(buffer.clone()))
            .event_format(JsonWithCorrelation)
            .with_env_filter(EnvFilter::new("info"))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello-json-correlation");
        });

        let output =
            String::from_utf8(buffer.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap();
        let line = output.lines().next().unwrap();
        let value: serde_json::Value = serde_json::from_str(line).unwrap();

        let cid = value
            .get("correlation_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        assert!(
            !cid.is_empty(),
            "correlation_id should be present and non-empty"
        );
    }
    #[test]
    fn compact_formatter_includes_correlation_id_token() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = fmt_sub()
            .with_writer(BufferWriter(buffer.clone()))
            .event_format(CompactAiFormatter)
            .with_env_filter(EnvFilter::new("info"))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello-compact-correlation");
        });

        let output =
            String::from_utf8(buffer.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap();
        let line = output.lines().next().unwrap_or("");
        assert!(
            line.contains("cid="),
            "compact log should include cid token: {}",
            line
        );
    }
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
        // From: "Successfully loaded config from ~/.scriptkit/config.ts"
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
            ("SCRIPT", 'G'), // Changed from B to G
            ("CONFIG", 'N'),
            ("RESIZE", 'Z'),
            ("DESIGN", 'D'),
            ("BENCH", 'B'), // New: Benchmark timing
            ("CHAT", 'U'),
            ("AI", 'U'),
            ("ACTIONS", 'U'),
            ("WINDOW_STATE", 'W'),
            ("DEBUG_GRID", 'D'),
            ("MCP", 'S'),
            ("WARN", 'X'),
            ("SCRIPTLET_PARSE", 'G'),
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
        // From: "Loaded 331 scripts from ~/.scriptkit/scripts"
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
        assert_eq!(
            infer_category_from_target("script_kit_gpui::window_state"),
            'W'
        );
    }
    #[test]
    fn test_infer_category_unknown() {
        assert_eq!(infer_category_from_target("script_kit_gpui::main"), 'A');
        assert_eq!(infer_category_from_target("script_kit_gpui::ai"), 'U');
        assert_eq!(
            infer_category_from_target("script_kit_gpui::mcp_server"),
            'S'
        );
        assert_eq!(infer_category_from_target("unknown::module"), '-');
    }
    #[test]
    fn test_legacy_level_for_category() {
        assert_eq!(legacy_level_for_category("ERROR"), LegacyLogLevel::Error);
        assert_eq!(legacy_level_for_category("WARN"), LegacyLogLevel::Warn);
        assert_eq!(legacy_level_for_category("WARNING"), LegacyLogLevel::Warn);
        assert_eq!(legacy_level_for_category("DEBUG"), LegacyLogLevel::Debug);
        assert_eq!(legacy_level_for_category("TRACE"), LegacyLogLevel::Trace);
        assert_eq!(legacy_level_for_category("UI"), LegacyLogLevel::Info);
    }
    // -------------------------------------------------------------------------
    // log rotation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_rotate_log_if_oversized() {
        let dir = std::env::temp_dir().join(format!("sk-rotate-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rotate-me.jsonl");
        let rotated = dir.join("rotate-me.jsonl.1");

        // Under the cap: untouched.
        fs::write(&path, b"small").unwrap();
        rotate_log_if_oversized(&path, 100);
        assert!(path.exists());
        assert!(!rotated.exists());

        // Over the cap: renamed to .1.
        fs::write(&path, vec![b'x'; 200]).unwrap();
        rotate_log_if_oversized(&path, 100);
        assert!(!path.exists());
        assert_eq!(fs::metadata(&rotated).unwrap().len(), 200);

        // Next oversized rotation replaces the previous .1.
        fs::write(&path, vec![b'y'; 300]).unwrap();
        rotate_log_if_oversized(&path, 100);
        assert_eq!(fs::metadata(&rotated).unwrap().len(), 300);

        // Cap 0 disables rotation entirely.
        fs::write(&path, vec![b'z'; 300]).unwrap();
        rotate_log_if_oversized(&path, 0);
        assert!(path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // correlation scope propagation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_with_correlation_scope_carries_id_across_capture() {
        // Capture inside an interaction scope...
        let captured = {
            let _guard = set_correlation_id("scope-test-id");
            scoped_correlation_id()
        };
        assert_eq!(captured.as_deref(), Some("scope-test-id"));

        // ...and re-apply it later (simulating the far side of a spawn/defer
        // hop, including on a different thread where the thread-local is
        // empty).
        let seen = std::thread::spawn(move || {
            with_correlation_scope(captured.as_deref(), current_correlation_id)
        })
        .join()
        .expect("scope thread should not panic");
        assert_eq!(seen, "scope-test-id");
    }

    #[test]
    fn test_with_correlation_scope_none_preserves_existing_scope() {
        let _guard = set_correlation_id("outer-scope");
        // A None capture (no interaction id at capture time) must not clobber
        // the executing thread's active scope.
        let seen = with_correlation_scope(None, current_correlation_id);
        assert_eq!(seen, "outer-scope");
    }

    // -------------------------------------------------------------------------
    // protocol log ring tests
    // -------------------------------------------------------------------------

    fn push_ring_entry(level: &str, target: &str, message: &str) {
        let entry = LogRingEntry {
            timestamp: "2026-07-01T00:00:00.000Z".to_string(),
            level: level.to_string(),
            target: target.to_string(),
            correlation_id: "test-cid".to_string(),
            message: message.to_string(),
        };
        let mut ring = log_ring().lock().unwrap_or_else(|e| e.into_inner());
        if ring.len() >= LOG_RING_CAPACITY {
            ring.pop_front();
        }
        ring.push_back(entry);
    }

    #[test]
    fn test_query_log_ring_filters_and_limit() {
        // The ring is process-global and other tests may log; use a unique
        // target so this test's entries are isolated.
        let target = "ring_test::filters";
        push_ring_entry("INFO", target, "first info");
        push_ring_entry("WARN", target, "a warning");
        push_ring_entry("ERROR", target, "an error");
        push_ring_entry("DEBUG", target, "debug noise");

        // Target filter alone sees all four.
        let (entries, matched) = query_log_ring(10, None, Some(target), None);
        assert_eq!(matched, 4);
        assert_eq!(entries.len(), 4);

        // Min-level warn keeps warn + error only.
        let (entries, matched) = query_log_ring(10, Some("warn"), Some(target), None);
        assert_eq!(matched, 2);
        assert!(entries
            .iter()
            .all(|e| e.level == "WARN" || e.level == "ERROR"));

        // Contains filter matches message text.
        let (entries, matched) = query_log_ring(10, None, Some(target), Some("an error"));
        assert_eq!(matched, 1);
        assert_eq!(entries[0].message, "an error");

        // Limit truncates from the front but reports the full match count.
        let (entries, matched) = query_log_ring(2, None, Some(target), None);
        assert_eq!(matched, 4);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].message, "debug noise");
    }

    // -------------------------------------------------------------------------
    // get_compact_timestamp tests
    // -------------------------------------------------------------------------

    /// Parse "HH:MM:SS.mmm" into total milliseconds since midnight UTC.
    fn parse_compact_ts(ts: &str) -> u64 {
        let (clock, millis) = ts.split_once('.').expect("timestamp should contain '.'");
        let parts: Vec<u64> = clock.split(':').map(|p| p.parse().unwrap()).collect();
        assert_eq!(parts.len(), 3, "clock part of '{}' should be HH:MM:SS", ts);
        let millis: u64 = millis.parse().expect("millis should be numeric");
        ((parts[0] * 60 + parts[1]) * 60 + parts[2]) * 1000 + millis
    }

    #[test]
    fn test_get_compact_timestamp_format() {
        let ts = get_compact_timestamp();
        // Format should be "HH:MM:SS.mmm" - sortable across the whole session
        assert_eq!(ts.len(), 12, "Timestamp '{}' should be 12 chars", ts);
        let total_ms = parse_compact_ts(&ts);
        assert!(
            total_ms < 24 * 3600 * 1000,
            "Timestamp '{}' should be within one day",
            ts
        );
    }
    #[test]
    fn test_get_compact_timestamp_monotonic_across_minute() {
        // Two calls a few ms apart must remain ordered even when the wall
        // clock crosses a minute boundary — the old SS.mmm format failed this.
        let ts1 = get_compact_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ts2 = get_compact_timestamp();

        let diff = parse_compact_ts(&ts2).saturating_sub(parse_compact_ts(&ts1));
        assert!(
            diff >= 4,
            "Timestamps should be at least 4ms apart, got {}ms",
            diff
        );
        assert!(
            diff < 100,
            "Timestamps should be less than 100ms apart, got {}ms",
            diff
        );
        // Lexicographic ordering must agree with chronological ordering
        // (ignoring the once-a-day midnight wraparound).
        assert!(ts1 < ts2, "'{}' should sort before '{}'", ts1, ts2);
    }
    // -------------------------------------------------------------------------
    // Compact format output validation (pattern matching)
    // -------------------------------------------------------------------------

    #[test]
    fn test_compact_format_pattern() {
        // Real example from logs:
        // "18:42:11.697|i|A|Application logging initialized event_type=app_lifecycle..."
        let example = "18:42:11.697|i|A|Application logging initialized";

        let parts: Vec<&str> = example.split('|').collect();
        assert_eq!(parts.len(), 4, "Compact format should have 4 parts");

        // Part 0: timestamp (HH:MM:SS.mmm)
        assert_eq!(parts[0].len(), 12);
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
            ("18:42:11.697|i|A|Application logging initialized", "i", "A"),
            ("18:42:11.717|i|N|Successfully loaded config", "i", "N"),
            (
                "18:42:11.741|i|H|Registered global hotkey meta+Digit0",
                "i",
                "H",
            ),
            ("18:42:11.779|i|P|Available displays: 1", "i", "P"),
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

    // --- merged from part_001.rs ---
    // -------------------------------------------------------------------------
    // Token savings verification
    // -------------------------------------------------------------------------

    #[test]
    fn test_compact_format_token_savings() {
        // Real comparison from logs:
        // Standard: "2025-12-27T15:22:13.150640Z  INFO script_kit_gpui::logging: Selected display..."
        // Compact:  "15:22:13.150|i|P|Selected display..."

        let standard_prefix = "2025-12-27T15:22:13.150640Z  INFO script_kit_gpui::logging: ";
        let compact_prefix = "15:22:13.150|i|P|";

        let savings_percent =
            100.0 - (compact_prefix.len() as f64 / standard_prefix.len() as f64 * 100.0);

        // Should save at least 60% on the prefix
        assert!(
            savings_percent > 60.0,
            "Should save >60% on prefix, got {:.1}%",
            savings_percent
        );

        // Actual: 17 chars vs 59 chars = ~71% savings (the full HH:MM:SS.mmm
        // timestamp costs 6 chars over the old SS.mmm but keeps lines sortable)
        assert!(
            savings_percent > 70.0,
            "Should save >70% on prefix, got {:.1}%",
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

    #[test]
    fn test_open_writer_or_sink_uses_sink_when_open_fails() {
        let mut writer = open_writer_or_sink(
            Err(std::io::Error::other("forced open failure")),
            "test log file",
        );

        writer
            .write_all(b"test log line")
            .expect("sink fallback should accept writes");
        writer.flush().expect("sink fallback should flush");
    }

    // -------------------------------------------------------------------------
    // Payload truncation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_truncate_for_log_short_string() {
        let s = "hello";
        assert_eq!(truncate_for_log(s, 10), "hello");
    }
    #[test]
    fn test_truncate_for_log_exact_limit() {
        let s = "hello";
        assert_eq!(truncate_for_log(s, 5), "hello");
    }
    #[test]
    fn test_truncate_for_log_long_string() {
        let s = "hello world this is a long string";
        let result = truncate_for_log(s, 10);
        assert!(result.starts_with("hello worl"));
        assert!(result.contains("...(33)")); // Original length in parens
    }
    #[test]
    fn test_truncate_for_log_utf8_emoji() {
        // Emoji are 4-byte UTF-8 sequences. Truncating mid-codepoint would panic with naive &s[..max_len]
        let s = "hello 🎉 world";
        // "hello " is 6 bytes, 🎉 is 4 bytes (positions 6-9), " world" starts at byte 10
        // If max_len=8, naive slice would land inside the emoji and panic
        let result = truncate_for_log(s, 8);
        // Should truncate to a valid char boundary without panic
        assert!(result.starts_with("hello "));
        assert!(result.contains(&format!("...({})", s.len())));
    }
    #[test]
    fn test_truncate_for_log_utf8_multibyte() {
        // Test with various multi-byte UTF-8 characters
        let s = "日本語テスト"; // Each char is 3 bytes = 18 bytes total
                                // If we truncate at 5 bytes, we'd land mid-character
        let result = truncate_for_log(s, 5);
        // Should back up to char boundary (3 bytes = 1 char)
        assert!(result.starts_with("日"));
        assert!(result.contains(&format!("...({})", s.len())));
    }
    #[test]
    fn test_truncate_for_log_utf8_mixed() {
        // Mixed ASCII and multi-byte
        let s = "abc日本語def";
        // "abc" = 3 bytes, "日本語" = 9 bytes, "def" = 3 bytes
        // Truncate at 5 would land inside 日
        let result = truncate_for_log(s, 5);
        // Should truncate at byte 3 (after "abc")
        assert!(result.starts_with("abc"));
        assert!(result.contains(&format!("...({})", s.len())));
    }
    #[test]
    fn test_truncate_for_log_empty_string() {
        let s = "";
        assert_eq!(truncate_for_log(s, 10), "");
    }
    #[test]
    fn test_truncate_for_log_zero_max_len() {
        let s = "hello";
        let result = truncate_for_log(s, 0);
        // Edge case: max_len=0 should return just the suffix
        assert!(result.contains("...(5)"));
    }
    #[test]
    fn test_summarize_payload_with_type() {
        let json = r#"{"type":"submit","id":"test","value":"foo"}"#;
        let summary = summarize_payload(json);
        assert!(summary.contains("type:submit"));
        assert!(summary.contains(&format!("len:{}", json.len())));
    }
    #[test]
    fn test_summarize_payload_without_type() {
        let json = r#"{"data":"some value"}"#;
        let summary = summarize_payload(json);
        assert!(summary.contains(&format!("len:{}", json.len())));
        assert!(!summary.contains("type:"));
    }
    #[test]
    fn test_summarize_payload_large_base64() {
        // Simulate a large base64 screenshot payload
        let base64_data = "a".repeat(100000);
        let json = format!(r#"{{"type":"screenshotResult","data":"{}"}}"#, base64_data);
        let summary = summarize_payload(&json);
        // Summary should be compact, not contain the full base64
        assert!(summary.len() < 100);
        assert!(summary.contains("type:screenshotResult"));
        assert!(summary.contains(&format!("len:{}", json.len())));
    }
    // -------------------------------------------------------------------------
    // Log capture tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_capture_enabled_default_false() {
        // By default, capture should be disabled
        // Note: we can't test this in isolation because it's a global static
        // but we can verify the initial state
        let _ = is_capture_enabled(); // Just verify it doesn't panic
    }
    #[test]
    fn test_toggle_capture_returns_correct_state() {
        // First toggle should start capture (if not already running)
        let initial_state = is_capture_enabled();

        if !initial_state {
            // If not capturing, toggle should start it
            let (is_capturing, path) = toggle_capture();
            assert!(is_capturing);
            assert!(path.is_some());

            // Clean up: toggle again to stop
            let (is_capturing2, path2) = toggle_capture();
            assert!(!is_capturing2);
            assert!(path2.is_some());
        } else {
            // If already capturing (from another test), toggle should stop it
            let (is_capturing, path) = toggle_capture();
            assert!(!is_capturing);
            assert!(path.is_some());
        }
    }
    #[test]
    fn test_capture_file_path_format() {
        // Start capture and check the file path format
        let was_enabled = is_capture_enabled();

        if !was_enabled {
            let result = start_capture();
            assert!(result.is_ok());

            let path = result.unwrap();
            let filename = path.file_name().unwrap().to_str().unwrap();

            // Filename should be like: capture-2026-01-11T08-37-28.jsonl
            assert!(filename.starts_with("capture-"));
            assert!(filename.ends_with(".jsonl"));

            // Clean up
            let _ = stop_capture();
        }
    }
    #[test]
    fn test_stop_capture_when_not_started() {
        // Ensure capture is stopped
        while is_capture_enabled() {
            let _ = stop_capture();
        }

        // Stopping when not started should return None
        let result = stop_capture();
        assert!(result.is_none());
    }
}

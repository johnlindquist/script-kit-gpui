//! MCP Server-Sent Events (SSE) Streaming and Audit Logging
//!
//! Provides:
//! - SSE streaming for real-time event delivery to clients
//! - Audit logging for tool calls to ~/.sk/kit/logs/mcp-audit.jsonl
//!
//! Event format: `event: {type}\ndata: {json}\n\n`

// Allow dead code - SSE streaming and audit logging infrastructure for future features
#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// SSE event types supported by the MCP server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SseEventType {
    Progress,
    Output,
    Error,
    Complete,
}

impl SseEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SseEventType::Progress => "progress",
            SseEventType::Output => "output",
            SseEventType::Error => "error",
            SseEventType::Complete => "complete",
        }
    }
}

impl std::fmt::Display for SseEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// SSE Stream manager for broadcasting events to connected clients
#[derive(Debug)]
pub struct SseStream {
    /// Buffer of formatted SSE messages ready to send
    buffer: Vec<String>,
}

impl SseStream {
    /// Create a new SSE stream
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Format and queue an SSE event for broadcast
    ///
    /// Event format: `event: {type}\ndata: {json}\n\n`
    pub fn broadcast_event(&mut self, event_type: SseEventType, data: &serde_json::Value) {
        let formatted = format_sse_event(event_type, data);
        self.buffer.push(formatted);
    }

    /// Get all pending events and clear the buffer
    pub fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.buffer)
    }

    /// Get the number of pending events
    pub fn pending_count(&self) -> usize {
        self.buffer.len()
    }
}

impl Default for SseStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a single SSE event
///
/// Format: `event: {type}\ndata: {json}\n\n`
pub fn format_sse_event(event_type: SseEventType, data: &serde_json::Value) -> String {
    let json_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    format!("event: {}\ndata: {}\n\n", event_type.as_str(), json_str)
}

/// Format an SSE heartbeat comment
///
/// Format: `: heartbeat\n\n`
pub fn format_sse_heartbeat() -> String {
    ": heartbeat\n\n".to_string()
}

/// Audit log entry for tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Method/tool name that was called
    pub method: String,
    /// Parameters passed to the method (as JSON)
    pub params: serde_json::Value,
    /// Duration of the call in milliseconds
    pub duration_ms: u64,
    /// Whether the call succeeded
    pub success: bool,
    /// Error message if the call failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AuditLogEntry {
    /// Create a new successful audit log entry
    pub fn success(method: &str, params: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            timestamp: iso8601_now(),
            method: method.to_string(),
            params,
            duration_ms,
            success: true,
            error: None,
        }
    }

    /// Create a new failed audit log entry
    pub fn failure(method: &str, params: serde_json::Value, duration_ms: u64, error: &str) -> Self {
        Self {
            timestamp: iso8601_now(),
            method: method.to_string(),
            params,
            duration_ms,
            success: false,
            error: Some(error.to_string()),
        }
    }
}

/// Audit logger that writes to ~/.sk/kit/logs/mcp-audit.jsonl
pub struct AuditLogger {
    log_path: PathBuf,
}

impl AuditLogger {
    /// Create a new audit logger
    ///
    /// # Arguments
    /// * `kit_path` - Path to ~/.sk/kit directory
    pub fn new(kit_path: PathBuf) -> Self {
        let log_path = kit_path.join("logs").join("mcp-audit.jsonl");
        Self { log_path }
    }

    /// Create audit logger with default ~/.sk/kit path
    pub fn with_defaults() -> Result<Self> {
        let kit_path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".sk/kit");
        Ok(Self::new(kit_path))
    }

    /// Get the log file path
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Write an audit log entry
    pub fn log(&self, entry: &AuditLogEntry) -> Result<()> {
        // Ensure logs directory exists
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent).context("Failed to create logs directory")?;
        }

        // Serialize entry to JSON
        let json = serde_json::to_string(entry).context("Failed to serialize audit log entry")?;

        // Append to log file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open audit log file")?;

        writeln!(file, "{}", json).context("Failed to write audit log entry")?;

        Ok(())
    }

    /// Log a successful tool call
    pub fn log_success(
        &self,
        method: &str,
        params: serde_json::Value,
        duration_ms: u64,
    ) -> Result<()> {
        let entry = AuditLogEntry::success(method, params, duration_ms);
        self.log(&entry)
    }

    /// Log a failed tool call
    pub fn log_failure(
        &self,
        method: &str,
        params: serde_json::Value,
        duration_ms: u64,
        error: &str,
    ) -> Result<()> {
        let entry = AuditLogEntry::failure(method, params, duration_ms, error);
        self.log(&entry)
    }
}

/// Get current timestamp in ISO 8601 format
fn iso8601_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();

    // Convert to datetime components (simplified - just for formatting)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    // Simplified calculation - good enough for logging purposes
    let mut year = 1970i32;
    let mut remaining_days = days_since_epoch as i32;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let (month, day) = day_of_year_to_month_day(remaining_days as u32 + 1, is_leap_year(year));

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, minutes, seconds, millis
    )
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn day_of_year_to_month_day(day_of_year: u32, leap: bool) -> (u32, u32) {
    let days_in_months: [u32; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut remaining = day_of_year;
    for (i, &days) in days_in_months.iter().enumerate() {
        if remaining <= days {
            return ((i + 1) as u32, remaining);
        }
        remaining -= days;
    }
    (12, 31) // Fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ==========================================
    // TDD TESTS - Written FIRST before implementation
    // ==========================================

    #[test]
    fn test_sse_event_format() {
        // Test that SSE events are formatted correctly per the SSE spec:
        // event: {type}\ndata: {json}\n\n

        let data = serde_json::json!({"message": "hello", "progress": 50});
        let formatted = format_sse_event(SseEventType::Progress, &data);

        // Must start with "event: progress\n"
        assert!(
            formatted.starts_with("event: progress\n"),
            "Event line must come first"
        );

        // Must contain "data: " line with JSON
        assert!(formatted.contains("data: "), "Must have data line");
        assert!(
            formatted.contains(r#""message":"hello""#),
            "Data must contain JSON"
        );
        assert!(
            formatted.contains(r#""progress":50"#),
            "Data must contain progress"
        );

        // Must end with double newline
        assert!(
            formatted.ends_with("\n\n"),
            "Must end with double newline for SSE"
        );

        // Test all event types format correctly
        for event_type in [
            SseEventType::Progress,
            SseEventType::Output,
            SseEventType::Error,
            SseEventType::Complete,
        ] {
            let formatted = format_sse_event(event_type, &serde_json::json!({}));
            assert!(
                formatted.starts_with(&format!("event: {}\n", event_type.as_str())),
                "Event type {} should format correctly",
                event_type
            );
        }
    }

    #[test]
    fn test_sse_stream_broadcast() {
        let mut stream = SseStream::new();

        // Initially empty
        assert_eq!(stream.pending_count(), 0);

        // Broadcast some events
        stream.broadcast_event(SseEventType::Progress, &serde_json::json!({"step": 1}));
        stream.broadcast_event(SseEventType::Output, &serde_json::json!({"line": "test"}));

        assert_eq!(stream.pending_count(), 2);

        // Drain events
        let events = stream.drain_events();
        assert_eq!(events.len(), 2);
        assert!(events[0].contains("event: progress"));
        assert!(events[1].contains("event: output"));

        // Buffer should be empty after drain
        assert_eq!(stream.pending_count(), 0);
    }

    #[test]
    fn test_sse_heartbeat_format() {
        let heartbeat = format_sse_heartbeat();

        // Heartbeat is a comment (starts with :)
        assert!(
            heartbeat.starts_with(":"),
            "Heartbeat must be SSE comment (start with :)"
        );
        assert!(
            heartbeat.ends_with("\n\n"),
            "Heartbeat must end with double newline"
        );
    }

    #[test]
    fn test_audit_log_written() {
        // Test that audit logs are actually written to the file
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf());

        // Log should not exist yet
        assert!(
            !logger.log_path().exists(),
            "Log file should not exist initially"
        );

        // Log a successful call
        logger
            .log_success(
                "tools/run_script",
                serde_json::json!({"name": "test.ts"}),
                100,
            )
            .expect("Should write log successfully");

        // Log file should now exist
        assert!(logger.log_path().exists(), "Log file should be created");

        // Read and verify content
        let content = fs::read_to_string(logger.log_path()).unwrap();
        assert!(!content.is_empty(), "Log file should have content");

        // Log another entry
        logger
            .log_failure(
                "tools/bad_call",
                serde_json::json!({}),
                50,
                "Invalid params",
            )
            .expect("Should write failure log");

        // Should have two lines
        let content = fs::read_to_string(logger.log_path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "Should have 2 log entries");
    }

    #[test]
    fn test_audit_log_format() {
        // Test that audit log entries have the correct JSONL format
        let temp_dir = TempDir::new().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf());

        let params = serde_json::json!({
            "script": "hello.ts",
            "args": ["--verbose"]
        });

        logger
            .log_success("tools/run_script", params.clone(), 250)
            .expect("Should log successfully");

        // Read and parse the log entry
        let content = fs::read_to_string(logger.log_path()).unwrap();
        let entry: AuditLogEntry =
            serde_json::from_str(content.trim()).expect("Log entry should be valid JSON");

        // Verify all required fields
        assert!(!entry.timestamp.is_empty(), "timestamp must be present");
        assert!(
            entry.timestamp.contains("T"),
            "timestamp must be ISO 8601 format"
        );
        assert_eq!(entry.method, "tools/run_script", "method must match");
        assert_eq!(entry.params, params, "params must match");
        assert_eq!(entry.duration_ms, 250, "duration_ms must match");
        assert!(entry.success, "success must be true");
        assert!(entry.error.is_none(), "error must be None for success");

        // Test failure entry format
        logger
            .log_failure(
                "tools/fail",
                serde_json::json!({}),
                10,
                "Something went wrong",
            )
            .unwrap();

        let content = fs::read_to_string(logger.log_path()).unwrap();
        let last_line = content.lines().last().unwrap();
        let fail_entry: AuditLogEntry = serde_json::from_str(last_line).unwrap();

        assert!(!fail_entry.success, "success must be false for failure");
        assert_eq!(
            fail_entry.error,
            Some("Something went wrong".to_string()),
            "error message must match"
        );
    }

    #[test]
    fn test_audit_entry_constructors() {
        let params = serde_json::json!({"test": true});

        // Test success constructor
        let success = AuditLogEntry::success("my_method", params.clone(), 100);
        assert_eq!(success.method, "my_method");
        assert_eq!(success.params, params);
        assert_eq!(success.duration_ms, 100);
        assert!(success.success);
        assert!(success.error.is_none());

        // Test failure constructor
        let failure = AuditLogEntry::failure("my_method", params.clone(), 50, "oops");
        assert_eq!(failure.method, "my_method");
        assert_eq!(failure.params, params);
        assert_eq!(failure.duration_ms, 50);
        assert!(!failure.success);
        assert_eq!(failure.error, Some("oops".to_string()));
    }

    #[test]
    fn test_sse_event_type_display() {
        assert_eq!(SseEventType::Progress.as_str(), "progress");
        assert_eq!(SseEventType::Output.as_str(), "output");
        assert_eq!(SseEventType::Error.as_str(), "error");
        assert_eq!(SseEventType::Complete.as_str(), "complete");

        assert_eq!(format!("{}", SseEventType::Progress), "progress");
    }

    #[test]
    fn test_iso8601_timestamp_format() {
        let ts = iso8601_now();

        // Should be in format: YYYY-MM-DDTHH:MM:SS.mmmZ
        assert!(ts.len() >= 24, "Timestamp should be at least 24 chars");
        assert!(ts.contains("T"), "Should have T separator");
        assert!(ts.ends_with("Z"), "Should end with Z for UTC");

        // Should be parseable (basic validation)
        let parts: Vec<&str> = ts.split('T').collect();
        assert_eq!(parts.len(), 2, "Should have date and time parts");

        let date_parts: Vec<&str> = parts[0].split('-').collect();
        assert_eq!(date_parts.len(), 3, "Date should have 3 parts");

        // Year should be reasonable
        let year: i32 = date_parts[0].parse().unwrap();
        assert!(year >= 2024, "Year should be current or later");
    }
}

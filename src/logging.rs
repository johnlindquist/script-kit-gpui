use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::OnceLock;
use std::collections::VecDeque;

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static LOG_BUFFER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

const MAX_LOG_LINES: usize = 50;

pub fn init() {
    // Initialize log buffer
    let _ = LOG_BUFFER.set(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES)));
    
    let path = std::env::temp_dir().join("script-kit-gpui.log");
    println!("========================================");
    println!("[SCRIPT-KIT-GPUI] Log file: {}", path.display());
    println!("========================================");
    
    if let Ok(file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        let _ = LOG_FILE.set(Mutex::new(file));
        log("APP", "Application started");
    }
}

pub fn log(category: &str, message: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    
    let line = format!("[{}] [{}] {}", timestamp, category, message);
    
    // Always print to stdout
    println!("{}", line);
    
    // Add to in-memory buffer for UI display
    if let Some(buffer) = LOG_BUFFER.get() {
        if let Ok(mut buf) = buffer.lock() {
            if buf.len() >= MAX_LOG_LINES {
                buf.pop_front();
            }
            buf.push_back(format!("[{}] {}", category, message));
        }
    }
    
    // Write to file
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut file) = mutex.lock() {
            let _ = writeln!(file, "{}", line);
            let _ = file.flush();
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

pub fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("script-kit-gpui.log")
}

/// Debug-only logging - compiled out in release builds
/// Use for verbose performance/scroll/cache logging
#[cfg(debug_assertions)]
pub fn log_debug(category: &str, message: &str) {
    log(category, message);
}

#[cfg(not(debug_assertions))]
pub fn log_debug(_category: &str, _message: &str) {
    // No-op in release builds
}

// =============================================================================
// MOUSE HOVER LOGGING
// Category: MOUSE_HOVER
// Purpose: Log mouse enter/leave events on list items for debugging hover/focus behavior
// =============================================================================

/// Log mouse enter event on a list item
pub fn log_mouse_enter(item_index: usize, item_id: Option<&str>) {
    let id_info = item_id.map(|id| format!(" id={}", id)).unwrap_or_default();
    log("MOUSE_HOVER", &format!("ENTER item_index={}{}", item_index, id_info));
}

/// Log mouse leave event on a list item
pub fn log_mouse_leave(item_index: usize, item_id: Option<&str>) {
    let id_info = item_id.map(|id| format!(" id={}", id)).unwrap_or_default();
    log("MOUSE_HOVER", &format!("LEAVE item_index={}{}", item_index, id_info));
}

/// Log mouse hover state change (for tracking focus/highlight transitions)
pub fn log_mouse_hover_state(item_index: usize, is_hovered: bool, is_focused: bool) {
    log("MOUSE_HOVER", &format!(
        "STATE item_index={} hovered={} focused={}",
        item_index, is_hovered, is_focused
    ));
}

// =============================================================================
// SCROLL STATE LOGGING
// Category: SCROLL_STATE
// Purpose: Log scroll position changes and scroll_to_item calls for debugging jitter
// =============================================================================

/// Log scroll position change
pub fn log_scroll_position(scroll_top: f32, visible_start: usize, visible_end: usize) {
    log("SCROLL_STATE", &format!(
        "POSITION scroll_top={:.2} visible_range={}..{}",
        scroll_top, visible_start, visible_end
    ));
}

/// Log scroll_to_item call
pub fn log_scroll_to_item(target_index: usize, reason: &str) {
    log("SCROLL_STATE", &format!(
        "SCROLL_TO_ITEM target={} reason={}",
        target_index, reason
    ));
}

/// Log scroll bounds/viewport info
pub fn log_scroll_bounds(viewport_height: f32, content_height: f32, item_count: usize) {
    log("SCROLL_STATE", &format!(
        "BOUNDS viewport={:.2} content={:.2} items={}",
        viewport_height, content_height, item_count
    ));
}

/// Log scroll adjustment (when scroll position is programmatically corrected)
pub fn log_scroll_adjustment(from: f32, to: f32, reason: &str) {
    log("SCROLL_STATE", &format!(
        "ADJUSTMENT from={:.2} to={:.2} reason={}",
        from, to, reason
    ));
}

// =============================================================================
// SCROLL PERFORMANCE LOGGING
// Category: SCROLL_PERF
// Purpose: Log timing information for scroll operations to detect jitter sources
// =============================================================================

/// Log scroll operation timing
pub fn log_scroll_perf_start(operation: &str) -> u128 {
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    log_debug("SCROLL_PERF", &format!("START {} at={}", operation, start));
    start
}

/// Log scroll operation completion with duration
pub fn log_scroll_perf_end(operation: &str, start_micros: u128) {
    let end = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let duration = end.saturating_sub(start_micros);
    log_debug("SCROLL_PERF", &format!(
        "END {} duration_us={}",
        operation, duration
    ));
}

/// Log scroll frame timing (for detecting dropped frames)
pub fn log_scroll_frame(frame_time_ms: f32, expected_frame_ms: f32) {
    let is_slow = frame_time_ms > expected_frame_ms * 1.5;
    let marker = if is_slow { " [SLOW]" } else { "" };
    log_debug("SCROLL_PERF", &format!(
        "FRAME time={:.2}ms expected={:.2}ms{}",
        frame_time_ms, expected_frame_ms, marker
    ));
}

/// Log scroll event rate (for detecting rapid scroll input)
pub fn log_scroll_event_rate(events_per_second: f32) {
    let is_rapid = events_per_second > 60.0;
    let marker = if is_rapid { " [RAPID]" } else { "" };
    log_debug("SCROLL_PERF", &format!(
        "EVENT_RATE eps={:.1}{}",
        events_per_second, marker
    ));
}

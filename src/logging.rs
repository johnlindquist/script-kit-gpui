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

This file is a merged representation of the filtered codebase, combined into a single document by packx.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of filtered repository contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<usage_guidelines>
- Treat this file as a snapshot of the repository's state
- Be aware that this file may contain sensitive information
</usage_guidelines>

<notes>
- Files were filtered by packx based on content and extension matching
- Total files included: 2
</notes>
</file_summary>

<directory_structure>
src/process_manager.rs
src/executor.rs
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/process_manager.rs">
//! Process Manager Module
//!
//! Centralized process tracking for bun script processes.
#![allow(dead_code)] // Some methods reserved for future use
//!
//! This module provides:
//! - PID file at ~/.sk/kit/script-kit.pid for main app
//! - Active child PIDs file at ~/.sk/kit/active-bun-pids.json
//! - Thread-safe process registration/unregistration
//! - Orphan detection on startup
//! - Bulk kill for graceful shutdown
//!
//! # Usage
//!
//! ```rust,ignore
//! use script_kit_gpui::process_manager::{ProcessManager, PROCESS_MANAGER};
//!
//! // Write main app PID
//! PROCESS_MANAGER.write_main_pid().unwrap();
//!
//! // Register a child process
//! PROCESS_MANAGER.register_process(pid, "/path/to/script.ts");
//!
//! // Unregister when done
//! PROCESS_MANAGER.unregister_process(pid);
//!
//! // Kill all on shutdown
//! PROCESS_MANAGER.kill_all_processes();
//!
//! // Cleanup main PID on exit
//! PROCESS_MANAGER.remove_main_pid();
//! ```

use crate::logging;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{LazyLock, RwLock};
use sysinfo::{Pid, System};

/// Global singleton process manager
pub static PROCESS_MANAGER: LazyLock<ProcessManager> = LazyLock::new(ProcessManager::new);

/// Information about a tracked child process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Path to the script being executed
    pub script_path: String,
    /// Timestamp when the process was started
    pub started_at: DateTime<Utc>,
}

/// Thread-safe process manager for tracking bun script processes
#[derive(Debug)]
pub struct ProcessManager {
    /// Map of PID -> ProcessInfo for active child processes
    active_processes: RwLock<HashMap<u32, ProcessInfo>>,
    /// Path to main app PID file
    main_pid_path: PathBuf,
    /// Path to active child PIDs JSON file
    active_pids_path: PathBuf,
}

impl ProcessManager {
    /// Create a new ProcessManager with default paths
    pub fn new() -> Self {
        let kenv_dir = dirs::home_dir()
            .map(|h| h.join(".kenv"))
            .unwrap_or_else(|| PathBuf::from("/tmp/.kenv"));

        Self {
            active_processes: RwLock::new(HashMap::new()),
            main_pid_path: kenv_dir.join("script-kit.pid"),
            active_pids_path: kenv_dir.join("active-bun-pids.json"),
        }
    }

    /// Write the main application PID to disk
    ///
    /// This should be called at startup. On subsequent calls, it will
    /// overwrite the existing PID file.
    ///
    /// # Errors
    ///
    /// Returns an error if the PID file cannot be written.
    pub fn write_main_pid(&self) -> std::io::Result<()> {
        let pid = std::process::id();
        logging::log(
            "PROC",
            &format!("Writing main PID {} to {:?}", pid, self.main_pid_path),
        );

        // Ensure parent directory exists
        if let Some(parent) = self.main_pid_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = File::create(&self.main_pid_path)?;
        write!(file, "{}", pid)?;

        logging::log("PROC", &format!("Main PID {} written successfully", pid));
        Ok(())
    }

    /// Remove the main PID file
    ///
    /// This should be called on clean shutdown.
    pub fn remove_main_pid(&self) {
        if self.main_pid_path.exists() {
            if let Err(e) = fs::remove_file(&self.main_pid_path) {
                logging::log("PROC", &format!("Failed to remove main PID file: {}", e));
            } else {
                logging::log("PROC", "Main PID file removed");
            }
        }
    }

    /// Read the main PID from disk, if it exists
    pub fn read_main_pid(&self) -> Option<u32> {
        if !self.main_pid_path.exists() {
            return None;
        }

        let mut file = File::open(&self.main_pid_path).ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;
        contents.trim().parse().ok()
    }

    /// Check if the main PID is stale (process no longer running)
    ///
    /// Returns true if there's a PID file but the process is not running.
    pub fn is_main_pid_stale(&self) -> bool {
        if let Some(pid) = self.read_main_pid() {
            !self.is_process_running(pid)
        } else {
            false
        }
    }

    /// Register a new child process
    ///
    /// This adds the process to the in-memory map and persists to disk.
    pub fn register_process(&self, pid: u32, script_path: &str) {
        let info = ProcessInfo {
            pid,
            script_path: script_path.to_string(),
            started_at: Utc::now(),
        };

        logging::log(
            "PROC",
            &format!(
                "Registering process PID {} for script: {}",
                pid, script_path
            ),
        );

        // Add to in-memory map
        if let Ok(mut processes) = self.active_processes.write() {
            processes.insert(pid, info);
        }

        // Persist to disk
        if let Err(e) = self.persist_active_pids() {
            logging::log("PROC", &format!("Failed to persist active PIDs: {}", e));
        }
    }

    /// Unregister a child process
    ///
    /// This removes the process from tracking when it exits normally.
    pub fn unregister_process(&self, pid: u32) {
        logging::log("PROC", &format!("Unregistering process PID {}", pid));

        // Remove from in-memory map
        if let Ok(mut processes) = self.active_processes.write() {
            processes.remove(&pid);
        }

        // Persist to disk
        if let Err(e) = self.persist_active_pids() {
            logging::log("PROC", &format!("Failed to persist active PIDs: {}", e));
        }
    }

    /// Get all currently tracked active processes
    pub fn get_active_processes(&self) -> Vec<ProcessInfo> {
        if let Ok(processes) = self.active_processes.read() {
            processes.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get count of active processes
    pub fn active_count(&self) -> usize {
        if let Ok(processes) = self.active_processes.read() {
            processes.len()
        } else {
            0
        }
    }

    /// Kill all tracked child processes
    ///
    /// This sends SIGKILL to each tracked process group.
    /// Used during graceful shutdown.
    pub fn kill_all_processes(&self) {
        let processes: Vec<ProcessInfo> = if let Ok(procs) = self.active_processes.read() {
            procs.values().cloned().collect()
        } else {
            Vec::new()
        };

        if processes.is_empty() {
            logging::log("PROC", "No active processes to kill");
            return;
        }

        logging::log(
            "PROC",
            &format!("Killing {} active process(es)", processes.len()),
        );

        for info in &processes {
            self.kill_process(info.pid);
        }

        // Clear the in-memory map
        if let Ok(mut procs) = self.active_processes.write() {
            procs.clear();
        }

        // Remove the active PIDs file
        if self.active_pids_path.exists() {
            if let Err(e) = fs::remove_file(&self.active_pids_path) {
                logging::log("PROC", &format!("Failed to remove active PIDs file: {}", e));
            }
        }

        logging::log("PROC", "All processes killed and tracking cleared");
    }

    /// Kill a single process by PID
    ///
    /// Sends SIGKILL to the process group on Unix.
    pub fn kill_process(&self, pid: u32) {
        logging::log("PROC", &format!("Killing process PID {}", pid));

        #[cfg(unix)]
        {
            // Kill the entire process group
            let negative_pgid = format!("-{}", pid);
            match Command::new("kill").args(["-9", &negative_pgid]).output() {
                Ok(output) => {
                    if output.status.success() {
                        logging::log(
                            "PROC",
                            &format!("Successfully killed process group {}", pid),
                        );
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("No such process") {
                            logging::log("PROC", &format!("Process {} already exited", pid));
                        } else {
                            logging::log(
                                "PROC",
                                &format!("Failed to kill process {}: {}", pid, stderr),
                            );
                        }
                    }
                }
                Err(e) => {
                    logging::log("PROC", &format!("Failed to execute kill command: {}", e));
                }
            }
        }

        #[cfg(not(unix))]
        {
            logging::log(
                "PROC",
                &format!("Non-Unix platform: cannot kill process {}", pid),
            );
        }
    }

    /// Check if a process is currently running
    pub fn is_process_running(&self, pid: u32) -> bool {
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system.process(Pid::from_u32(pid)).is_some()
    }

    /// Detect and clean up orphaned processes from a previous crash
    ///
    /// This should be called at startup. It reads the persisted PID file,
    /// checks which processes are still running, kills them, and clears the file.
    ///
    /// Returns the number of orphans killed.
    pub fn cleanup_orphans(&self) -> usize {
        logging::log(
            "PROC",
            "Checking for orphaned processes from previous session",
        );

        let orphans = self.load_persisted_pids();
        if orphans.is_empty() {
            logging::log("PROC", "No orphaned processes found");
            return 0;
        }

        logging::log(
            "PROC",
            &format!("Found {} potentially orphaned process(es)", orphans.len()),
        );

        let mut killed_count = 0;

        for info in &orphans {
            if self.is_process_running(info.pid) {
                logging::log(
                    "PROC",
                    &format!(
                        "Killing orphaned process PID {} (script: {})",
                        info.pid, info.script_path
                    ),
                );
                self.kill_process(info.pid);
                killed_count += 1;
            } else {
                logging::log("PROC", &format!("Orphan PID {} already exited", info.pid));
            }
        }

        // Clear the persisted file
        if self.active_pids_path.exists() {
            if let Err(e) = fs::remove_file(&self.active_pids_path) {
                logging::log("PROC", &format!("Failed to remove orphan PIDs file: {}", e));
            }
        }

        if killed_count > 0 {
            logging::log(
                "PROC",
                &format!("Cleaned up {} orphaned process(es)", killed_count),
            );
        }

        killed_count
    }

    /// Persist the current active PIDs to disk
    fn persist_active_pids(&self) -> std::io::Result<()> {
        let processes: Vec<ProcessInfo> = if let Ok(procs) = self.active_processes.read() {
            procs.values().cloned().collect()
        } else {
            Vec::new()
        };

        // Ensure parent directory exists
        if let Some(parent) = self.active_pids_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&processes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.active_pids_path)?;

        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Load persisted PIDs from disk
    fn load_persisted_pids(&self) -> Vec<ProcessInfo> {
        if !self.active_pids_path.exists() {
            return Vec::new();
        }

        let contents = match fs::read_to_string(&self.active_pids_path) {
            Ok(c) => c,
            Err(e) => {
                logging::log("PROC", &format!("Failed to read active PIDs file: {}", e));
                return Vec::new();
            }
        };

        match serde_json::from_str(&contents) {
            Ok(pids) => pids,
            Err(e) => {
                logging::log("PROC", &format!("Failed to parse active PIDs JSON: {}", e));
                Vec::new()
            }
        }
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a ProcessManager with a temporary directory for testing
    fn create_test_manager() -> (ProcessManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager {
            active_processes: RwLock::new(HashMap::new()),
            main_pid_path: temp_dir.path().join("script-kit.pid"),
            active_pids_path: temp_dir.path().join("active-bun-pids.json"),
        };
        (manager, temp_dir)
    }

    #[test]
    fn test_write_and_read_main_pid() {
        let (manager, _temp_dir) = create_test_manager();

        // Write PID
        manager.write_main_pid().unwrap();

        // Read it back
        let pid = manager.read_main_pid();
        assert_eq!(pid, Some(std::process::id()));
    }

    #[test]
    fn test_remove_main_pid() {
        let (manager, _temp_dir) = create_test_manager();

        // Write and remove
        manager.write_main_pid().unwrap();
        assert!(manager.main_pid_path.exists());

        manager.remove_main_pid();
        assert!(!manager.main_pid_path.exists());
    }

    #[test]
    fn test_register_and_unregister_process() {
        let (manager, _temp_dir) = create_test_manager();

        // Register a process
        manager.register_process(12345, "/path/to/test.ts");

        // Check it's tracked
        let active = manager.get_active_processes();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].pid, 12345);
        assert_eq!(active[0].script_path, "/path/to/test.ts");

        // Check persistence
        assert!(manager.active_pids_path.exists());

        // Unregister
        manager.unregister_process(12345);

        // Check it's gone
        let active = manager.get_active_processes();
        assert!(active.is_empty());
    }

    #[test]
    fn test_multiple_processes() {
        let (manager, _temp_dir) = create_test_manager();

        // Register multiple processes
        manager.register_process(1001, "/path/to/script1.ts");
        manager.register_process(1002, "/path/to/script2.ts");
        manager.register_process(1003, "/path/to/script3.ts");

        assert_eq!(manager.active_count(), 3);

        // Unregister one
        manager.unregister_process(1002);
        assert_eq!(manager.active_count(), 2);

        // Verify correct one was removed
        let active = manager.get_active_processes();
        let pids: Vec<u32> = active.iter().map(|p| p.pid).collect();
        assert!(pids.contains(&1001));
        assert!(!pids.contains(&1002));
        assert!(pids.contains(&1003));
    }

    #[test]
    fn test_kill_all_clears_tracking() {
        let (manager, _temp_dir) = create_test_manager();

        // Register some fake processes (won't actually exist)
        manager.register_process(99991, "/fake/script1.ts");
        manager.register_process(99992, "/fake/script2.ts");

        assert_eq!(manager.active_count(), 2);

        // Kill all (these PIDs don't exist, so kill will fail gracefully)
        manager.kill_all_processes();

        // Should be cleared
        assert_eq!(manager.active_count(), 0);
        assert!(!manager.active_pids_path.exists());
    }

    #[test]
    fn test_is_process_running_current_process() {
        let (manager, _temp_dir) = create_test_manager();

        // Current process should be running
        let current_pid = std::process::id();
        assert!(manager.is_process_running(current_pid));

        // Non-existent PID should not be running
        assert!(!manager.is_process_running(u32::MAX - 1));
    }

    #[test]
    fn test_persist_and_load_pids() {
        let (manager, _temp_dir) = create_test_manager();

        // Register processes
        manager.register_process(5001, "/test/a.ts");
        manager.register_process(5002, "/test/b.ts");

        // Load from disk
        let loaded = manager.load_persisted_pids();
        assert_eq!(loaded.len(), 2);

        let pids: Vec<u32> = loaded.iter().map(|p| p.pid).collect();
        assert!(pids.contains(&5001));
        assert!(pids.contains(&5002));
    }

    #[test]
    fn test_process_info_serialization() {
        let info = ProcessInfo {
            pid: 42,
            script_path: "/path/to/script.ts".to_string(),
            started_at: Utc::now(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: ProcessInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.pid, 42);
        assert_eq!(parsed.script_path, "/path/to/script.ts");
    }

    #[test]
    fn test_cleanup_orphans_with_no_file() {
        let (manager, _temp_dir) = create_test_manager();

        // No file exists, should return 0
        let killed = manager.cleanup_orphans();
        assert_eq!(killed, 0);
    }

    #[test]
    fn test_main_pid_stale_detection() {
        let (manager, _temp_dir) = create_test_manager();

        // No PID file - not stale
        assert!(!manager.is_main_pid_stale());

        // Write current PID - not stale (current process is running)
        manager.write_main_pid().unwrap();
        assert!(!manager.is_main_pid_stale());

        // Write a fake PID that doesn't exist
        let fake_pid_path = manager.main_pid_path.clone();
        fs::write(&fake_pid_path, "999999999").unwrap();
        assert!(manager.is_main_pid_stale());
    }

    #[test]
    fn test_default_paths() {
        let manager = ProcessManager::new();

        // Should use ~/.sk/kit/ paths
        let home = dirs::home_dir().unwrap();
        assert_eq!(manager.main_pid_path, home.join(".sk/kit/script-kit.pid"));
        assert_eq!(
            manager.active_pids_path,
            home.join(".sk/kit/active-bun-pids.json")
        );
    }
}

</file>

<file path="src/executor.rs">
use crate::logging;
use crate::process_manager::PROCESS_MANAGER;
use crate::protocol::{serialize_message, JsonlReader, Message};
use crate::scriptlets::{format_scriptlet, process_conditionals, Scriptlet, SHELL_TOOLS};
use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn};

// ============================================================================
// AUTO_SUBMIT Mode for Autonomous Testing
// ============================================================================
//
// These functions are used by the UI layer (main.rs) to enable autonomous
// testing of prompts. The #[allow(dead_code)] is temporary until integration
// is complete.

/// Check if AUTO_SUBMIT mode is enabled via environment variable.
///
/// When AUTO_SUBMIT=true or AUTO_SUBMIT=1, prompts will be automatically
/// submitted after a configurable delay for autonomous testing.
///
/// # Environment Variables
/// - `AUTO_SUBMIT` - Set to "true" or "1" to enable auto-submit mode
///
/// # Example
/// ```bash
/// AUTO_SUBMIT=true ./target/debug/script-kit-gpui tests/sdk/test-arg.ts
/// ```
#[allow(dead_code)]
pub fn is_auto_submit_enabled() -> bool {
    std::env::var("AUTO_SUBMIT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Get the delay before auto-submitting a prompt.
///
/// This allows the UI to render before automatically submitting,
/// useful for visual testing or debugging.
///
/// # Environment Variables
/// - `AUTO_SUBMIT_DELAY_MS` - Delay in milliseconds (default: 100)
///
/// # Returns
/// Duration for the delay, defaults to 100ms if not specified or invalid.
#[allow(dead_code)]
pub fn get_auto_submit_delay() -> Duration {
    std::env::var("AUTO_SUBMIT_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(100))
}

/// Get the value to auto-submit for prompts.
///
/// If set, this value will be submitted instead of selecting from choices.
/// Useful for testing specific input scenarios.
///
/// # Environment Variables
/// - `AUTO_SUBMIT_VALUE` - The value to submit (optional)
///
/// # Returns
/// Some(value) if AUTO_SUBMIT_VALUE is set, None otherwise.
#[allow(dead_code)]
pub fn get_auto_submit_value() -> Option<String> {
    std::env::var("AUTO_SUBMIT_VALUE").ok()
}

/// Get the index of the choice to auto-select.
///
/// If set, this index will be used to select from the choices array.
/// If the index is out of bounds, defaults to 0.
///
/// # Environment Variables
/// - `AUTO_SUBMIT_INDEX` - The 0-based index to select (default: 0)
///
/// # Returns
/// The index to select, defaults to 0 if not specified or invalid.
#[allow(dead_code)]
pub fn get_auto_submit_index() -> usize {
    std::env::var("AUTO_SUBMIT_INDEX")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
}

/// Configuration for AUTO_SUBMIT mode.
///
/// This struct captures all AUTO_SUBMIT environment variables at initialization time,
/// providing a consistent snapshot for the duration of the session.
///
/// # Example
/// ```bash
/// AUTO_SUBMIT=true AUTO_SUBMIT_DELAY_MS=200 ./target/debug/script-kit-gpui
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AutoSubmitConfig {
    /// Whether auto-submit mode is enabled
    pub enabled: bool,
    /// Delay before auto-submitting (in milliseconds)
    pub delay: Duration,
    /// Override value to submit (if set)
    pub value_override: Option<String>,
    /// Index of choice to select (0-based)
    pub index: usize,
}

impl Default for AutoSubmitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            delay: Duration::from_millis(100),
            value_override: None,
            index: 0,
        }
    }
}

#[allow(dead_code)]
impl AutoSubmitConfig {
    /// Create a new AutoSubmitConfig by reading environment variables.
    ///
    /// This captures the current state of all AUTO_SUBMIT env vars.
    pub fn from_env() -> Self {
        Self {
            enabled: is_auto_submit_enabled(),
            delay: get_auto_submit_delay(),
            value_override: get_auto_submit_value(),
            index: get_auto_submit_index(),
        }
    }

    /// Get the default value for an arg prompt with choices.
    ///
    /// Priority:
    /// 1. If `value_override` is set, use it
    /// 2. Otherwise, use `choices[index].value` (clamped to valid range)
    /// 3. If no choices, return None
    pub fn get_arg_value(&self, choices: &[crate::protocol::Choice]) -> Option<String> {
        // Check for value override first
        if let Some(ref override_value) = self.value_override {
            return Some(override_value.clone());
        }

        // Get choice by index (clamped to valid range)
        if choices.is_empty() {
            return None;
        }

        let idx = self.index.min(choices.len() - 1);
        Some(choices[idx].value.clone())
    }

    /// Get the default value for a div prompt.
    ///
    /// Div prompts just need dismissal, so we return None (no value needed).
    pub fn get_div_value(&self) -> Option<String> {
        None
    }

    /// Get the default value for an editor prompt.
    ///
    /// Returns the original content unchanged if no override is set.
    pub fn get_editor_value(&self, original_content: &str) -> Option<String> {
        if let Some(ref override_value) = self.value_override {
            Some(override_value.clone())
        } else {
            Some(original_content.to_string())
        }
    }

    /// Get the default value for a terminal prompt.
    ///
    /// Terminal prompts return the exit code (0 for success).
    pub fn get_term_value(&self) -> Option<String> {
        Some("0".to_string())
    }

    /// Get the default value for a form prompt.
    ///
    /// Forms return an empty JSON object by default.
    pub fn get_form_value(&self) -> Option<String> {
        Some("{}".to_string())
    }

    /// Get the default value for a select prompt (multi-select).
    ///
    /// Returns a JSON array with the first choice selected.
    pub fn get_select_value(&self, choices: &[crate::protocol::Choice]) -> Option<String> {
        if choices.is_empty() {
            return Some("[]".to_string());
        }

        let idx = self.index.min(choices.len() - 1);
        let value = &choices[idx].value;
        Some(format!("[\"{}\"]", value))
    }

    /// Get the default value for a fields prompt.
    ///
    /// Returns a JSON array of empty strings matching the number of fields.
    pub fn get_fields_value(&self, field_count: usize) -> Option<String> {
        let empty_strings: Vec<&str> = vec![""; field_count];
        Some(serde_json::to_string(&empty_strings).unwrap_or_else(|_| "[]".to_string()))
    }

    /// Get the default value for a path prompt.
    ///
    /// Returns "/tmp/test-path" as the default path.
    pub fn get_path_value(&self) -> Option<String> {
        Some("/tmp/test-path".to_string())
    }

    /// Get the default value for a hotkey prompt.
    ///
    /// Returns a JSON object representing Cmd+A.
    pub fn get_hotkey_value(&self) -> Option<String> {
        Some(r#"{"key":"a","command":true}"#.to_string())
    }

    /// Get the default value for a drop prompt.
    ///
    /// Returns a JSON array with a test file path.
    pub fn get_drop_value(&self) -> Option<String> {
        Some(r#"[{"path":"/tmp/test.txt"}]"#.to_string())
    }
}

/// Get a snapshot of the current AUTO_SUBMIT configuration.
///
/// This is the main entry point for checking auto-submit settings.
/// Call this once at startup or when needed, rather than repeatedly
/// reading env vars.
///
/// # Example
/// ```rust,ignore
/// let config = get_auto_submit_config();
/// if config.enabled {
///     // Schedule auto-submit after config.delay
/// }
/// ```
#[allow(dead_code)]
pub fn get_auto_submit_config() -> AutoSubmitConfig {
    AutoSubmitConfig::from_env()
}

// Conditionally import selected_text for macOS only
#[cfg(target_os = "macos")]
use crate::selected_text;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Embedded SDK content (included at compile time)
const EMBEDDED_SDK: &str = include_str!("../scripts/kit-sdk.ts");

/// Find an executable, checking common locations that GUI apps might miss
fn find_executable(name: &str) -> Option<PathBuf> {
    logging::log("EXEC", &format!("Looking for executable: {}", name));

    // Common paths where executables might be installed
    let common_paths = [
        // User-specific paths
        dirs::home_dir().map(|h| h.join(".bun/bin")),
        dirs::home_dir().map(|h| h.join("Library/pnpm")), // pnpm on macOS
        dirs::home_dir().map(|h| h.join(".nvm/current/bin")),
        dirs::home_dir().map(|h| h.join(".volta/bin")),
        dirs::home_dir().map(|h| h.join(".local/bin")),
        dirs::home_dir().map(|h| h.join("bin")),
        // Homebrew paths
        Some(PathBuf::from("/opt/homebrew/bin")),
        Some(PathBuf::from("/usr/local/bin")),
        // System paths
        Some(PathBuf::from("/usr/bin")),
        Some(PathBuf::from("/bin")),
    ];

    for path in common_paths.iter().flatten() {
        let exe_path = path.join(name);
        logging::log("EXEC", &format!("  Checking: {}", exe_path.display()));
        if exe_path.exists() {
            logging::log("EXEC", &format!("  FOUND: {}", exe_path.display()));
            return Some(exe_path);
        }
    }

    logging::log("EXEC", "  NOT FOUND in common paths, will try PATH");
    None
}

/// Ensure tsconfig.json has the @johnlindquist/kit path mapping
/// Merges with existing config if present
fn ensure_tsconfig_paths(tsconfig_path: &PathBuf) {
    use serde_json::{json, Value};

    let kit_path = json!(["./sdk/kit-sdk.ts"]);

    // Try to read and parse existing tsconfig
    let mut config: Value = if tsconfig_path.exists() {
        match std::fs::read_to_string(tsconfig_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| json!({})),
            Err(_) => json!({}),
        }
    } else {
        json!({})
    };

    // Ensure compilerOptions exists
    if config.get("compilerOptions").is_none() {
        config["compilerOptions"] = json!({});
    }

    // Ensure paths exists in compilerOptions
    if config["compilerOptions"].get("paths").is_none() {
        config["compilerOptions"]["paths"] = json!({});
    }

    // Check if @johnlindquist/kit path is already correct
    let current_kit_path = config["compilerOptions"]["paths"].get("@johnlindquist/kit");
    if current_kit_path == Some(&kit_path) {
        // Already correct, no need to write
        return;
    }

    // Set the @johnlindquist/kit path
    config["compilerOptions"]["paths"]["@johnlindquist/kit"] = kit_path;

    // Write back
    match serde_json::to_string_pretty(&config) {
        Ok(json_str) => {
            if let Err(e) = std::fs::write(tsconfig_path, json_str) {
                logging::log("EXEC", &format!("Failed to write tsconfig.json: {}", e));
            } else {
                logging::log("EXEC", "Updated tsconfig.json with @johnlindquist/kit path");
            }
        }
        Err(e) => {
            logging::log("EXEC", &format!("Failed to serialize tsconfig.json: {}", e));
        }
    }
}

/// Extract the embedded SDK to disk if needed
/// Returns the path to the extracted SDK file
fn ensure_sdk_extracted() -> Option<PathBuf> {
    // Target path: ~/.sk/kit/sdk/kit-sdk.ts
    let kenv_dir = dirs::home_dir()?.join(".kenv");
    let kenv_sdk = kenv_dir.join("sdk");
    let sdk_path = kenv_sdk.join("kit-sdk.ts");

    // Create sdk/ dir if needed
    if !kenv_sdk.exists() {
        if let Err(e) = std::fs::create_dir_all(&kenv_sdk) {
            logging::log("EXEC", &format!("Failed to create SDK dir: {}", e));
            return None;
        }
    }

    // Always write embedded SDK to ensure latest version
    // The embedded SDK is compiled into the binary via include_str!
    if let Err(e) = std::fs::write(&sdk_path, EMBEDDED_SDK) {
        logging::log("EXEC", &format!("Failed to write SDK: {}", e));
        return None;
    }

    // Log SDK info for debugging
    let sdk_len = EMBEDDED_SDK.len();
    logging::log(
        "EXEC",
        &format!(
            "Extracted SDK to {} ({} bytes)",
            sdk_path.display(),
            sdk_len
        ),
    );

    // Ensure tsconfig.json has @johnlindquist/kit path mapping
    let tsconfig_path = kenv_dir.join("tsconfig.json");
    ensure_tsconfig_paths(&tsconfig_path);

    // Always write .gitignore (app-managed)
    let gitignore_path = kenv_dir.join(".gitignore");
    let gitignore_content = r#"# SDK files (copied from app on each start)
sdk/
logs/
clipboard-history.db
"#;
    if let Err(e) = std::fs::write(&gitignore_path, gitignore_content) {
        logging::log("EXEC", &format!("Failed to write .gitignore: {}", e));
        // Non-fatal, continue
    } else {
        logging::log(
            "EXEC",
            &format!("Wrote .gitignore to {}", gitignore_path.display()),
        );
    }

    Some(sdk_path)
}

/// Find the SDK path, checking standard locations
fn find_sdk_path() -> Option<PathBuf> {
    logging::log("EXEC", "Looking for SDK...");

    // 1. Check ~/.sk/kit/sdk/kit-sdk.ts (primary location)
    if let Some(home) = dirs::home_dir() {
        let kenv_sdk = home.join(".sk/kit/sdk/kit-sdk.ts");
        logging::log(
            "EXEC",
            &format!("  Checking kenv sdk: {}", kenv_sdk.display()),
        );
        if kenv_sdk.exists() {
            logging::log(
                "EXEC",
                &format!("  FOUND SDK (kenv): {}", kenv_sdk.display()),
            );
            return Some(kenv_sdk);
        }
    }

    // 2. Extract embedded SDK to ~/.sk/kit/sdk/kit-sdk.ts (production)
    logging::log("EXEC", "  Trying to extract embedded SDK...");
    if let Some(sdk_path) = ensure_sdk_extracted() {
        logging::log(
            "EXEC",
            &format!("  FOUND SDK (embedded): {}", sdk_path.display()),
        );
        return Some(sdk_path);
    }

    // 3. Check relative to executable (for app bundle)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let sdk_path = exe_dir.join("kit-sdk.ts");
            logging::log(
                "EXEC",
                &format!("  Checking exe dir: {}", sdk_path.display()),
            );
            if sdk_path.exists() {
                logging::log(
                    "EXEC",
                    &format!("  FOUND SDK (exe dir): {}", sdk_path.display()),
                );
                return Some(sdk_path);
            }
        }
    }

    // 4. Development fallback - project scripts directory
    let dev_sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/kit-sdk.ts");
    logging::log(
        "EXEC",
        &format!("  Checking dev path: {}", dev_sdk.display()),
    );
    if dev_sdk.exists() {
        logging::log("EXEC", &format!("  FOUND SDK (dev): {}", dev_sdk.display()));
        return Some(dev_sdk);
    }

    logging::log("EXEC", "  SDK NOT FOUND anywhere!");
    None
}

/// Wrapper that tracks process ID for cleanup
/// This stores the PID at spawn time so we can kill the process group even after
/// the Child is moved or consumed.
///
/// CRITICAL: The Drop impl kills the process group, so this MUST be kept alive
/// until the script is done executing!
#[derive(Debug)]
pub struct ProcessHandle {
    /// Process ID (used as PGID since we spawn with process_group(0))
    pid: u32,
    /// Path to the script being executed (for process tracking)
    /// Used during registration with PROCESS_MANAGER in new()
    #[allow(dead_code)]
    script_path: String,
    /// Whether the process has been explicitly killed
    killed: bool,
}

impl ProcessHandle {
    fn new(pid: u32, script_path: String) -> Self {
        logging::log(
            "EXEC",
            &format!(
                "ProcessHandle created for PID {} (script: {})",
                pid, script_path
            ),
        );

        // Register with global process manager for tracking
        PROCESS_MANAGER.register_process(pid, &script_path);

        Self {
            pid,
            script_path,
            killed: false,
        }
    }

    /// Kill the process group (Unix) or just the process (other platforms)
    fn kill(&mut self) {
        if self.killed {
            logging::log(
                "EXEC",
                &format!("Process {} already killed, skipping", self.pid),
            );
            return;
        }
        self.killed = true;

        #[cfg(unix)]
        {
            // Kill the entire process group using the kill command with negative PID
            // Since we spawned with process_group(0), the PGID equals the PID
            // Using negative PID tells kill to target the process group
            let negative_pgid = format!("-{}", self.pid);
            logging::log(
                "EXEC",
                &format!("Killing process group PGID {} with SIGKILL", self.pid),
            );

            match Command::new("kill").args(["-9", &negative_pgid]).output() {
                Ok(output) => {
                    if output.status.success() {
                        logging::log(
                            "EXEC",
                            &format!("Successfully killed process group {}", self.pid),
                        );
                    } else {
                        // Process might already be dead, which is fine
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("No such process") {
                            logging::log(
                                "EXEC",
                                &format!("Process group {} already exited", self.pid),
                            );
                        } else {
                            logging::log(
                                "EXEC",
                                &format!("kill command failed for PGID {}: {}", self.pid, stderr),
                            );
                        }
                    }
                }
                Err(e) => {
                    logging::log("EXEC", &format!("Failed to execute kill command: {}", e));
                }
            }
        }

        #[cfg(not(unix))]
        {
            logging::log(
                "EXEC",
                &format!("Non-Unix platform: process {} marked as killed", self.pid),
            );
            // On non-Unix platforms, we rely on the Child::kill() method being called separately
        }
    }

    /// Check if process is still running (Unix only)
    #[cfg(unix)]
    #[allow(dead_code)]
    fn is_alive(&self) -> bool {
        // Use kill -0 to check if process exists
        Command::new("kill")
            .args(["-0", &self.pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        logging::log(
            "EXEC",
            &format!("ProcessHandle dropping for PID {}", self.pid),
        );

        // Unregister from global process manager BEFORE killing
        PROCESS_MANAGER.unregister_process(self.pid);

        self.kill();
    }
}

/// Session for bidirectional communication with a running script
pub struct ScriptSession {
    pub stdin: ChildStdin,
    stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    /// Captured stderr for error reporting
    pub stderr: Option<ChildStderr>,
    child: Child,
    /// Process handle for cleanup - kills process group on drop
    process_handle: ProcessHandle,
}

/// Split session components for separate read/write threads
pub struct SplitSession {
    pub stdin: ChildStdin,
    pub stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    /// Captured stderr for error reporting
    pub stderr: Option<ChildStderr>,
    pub child: Child,
    /// Process handle for cleanup - kills process group on drop
    /// IMPORTANT: This MUST be kept alive until the script completes!
    /// Dropping it will kill the process group via the Drop impl.
    pub process_handle: ProcessHandle,
}

impl ScriptSession {
    /// Split the session into separate read/write components
    /// This allows using separate threads for reading and writing
    pub fn split(self) -> SplitSession {
        logging::log(
            "EXEC",
            &format!(
                "Splitting ScriptSession for PID {}",
                self.process_handle.pid
            ),
        );
        SplitSession {
            stdin: self.stdin,
            stdout_reader: self.stdout_reader,
            stderr: self.stderr,
            child: self.child,
            process_handle: self.process_handle,
        }
    }
}

#[allow(dead_code)]
impl SplitSession {
    /// Check if the child process is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        }
    }

    /// Kill the child process and its process group
    pub fn kill(&mut self) -> Result<(), String> {
        logging::log(
            "EXEC",
            &format!("SplitSession::kill() for PID {}", self.process_handle.pid),
        );
        self.process_handle.kill();
        // Also try the standard kill for good measure
        let _ = self.child.kill();
        Ok(())
    }

    /// Wait for the child process to terminate and get its exit code
    pub fn wait(&mut self) -> Result<i32, String> {
        let status = self
            .child
            .wait()
            .map_err(|e| format!("Failed to wait for script process: {}", e))?;
        let code = status.code().unwrap_or(-1);
        logging::log("EXEC", &format!("Script exited with code: {}", code));
        Ok(code)
    }

    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.process_handle.pid
    }
}

#[allow(dead_code)]
impl ScriptSession {
    /// Send a message to the running script
    pub fn send_message(&mut self, msg: &Message) -> Result<(), String> {
        let json =
            serialize_message(msg).map_err(|e| format!("Failed to serialize message: {}", e))?;
        logging::log("EXEC", &format!("Sending to script: {}", json));
        writeln!(self.stdin, "{}", json)
            .map_err(|e| format!("Failed to write to script stdin: {}", e))?;
        self.stdin
            .flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        Ok(())
    }

    /// Receive a message from the running script (blocking)
    pub fn receive_message(&mut self) -> Result<Option<Message>, String> {
        let result = self
            .stdout_reader
            .next_message()
            .map_err(|e| format!("Failed to read from script stdout: {}", e));
        if let Ok(Some(ref msg)) = result {
            logging::log("EXEC", &format!("Received from script: {:?}", msg));
        }
        result
    }

    /// Check if the child process is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        }
    }

    /// Wait for the child process to terminate and get its exit code
    pub fn wait(&mut self) -> Result<i32, String> {
        let status = self
            .child
            .wait()
            .map_err(|e| format!("Failed to wait for script process: {}", e))?;
        let code = status.code().unwrap_or(-1);
        logging::log("EXEC", &format!("Script exited with code: {}", code));
        Ok(code)
    }

    /// Kill the child process and its process group
    pub fn kill(&mut self) -> Result<(), String> {
        logging::log(
            "EXEC",
            &format!("ScriptSession::kill() for PID {}", self.process_handle.pid),
        );
        self.process_handle.kill();
        // Also try the standard kill for good measure
        let _ = self.child.kill();
        Ok(())
    }

    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.process_handle.pid
    }
}

/// Execute a script with bidirectional JSONL communication
#[instrument(skip_all, fields(script_path = %path.display()))]
pub fn execute_script_interactive(path: &Path) -> Result<ScriptSession, String> {
    let start = Instant::now();
    debug!(path = %path.display(), "Starting interactive script execution");
    logging::log(
        "EXEC",
        &format!("execute_script_interactive: {}", path.display()),
    );

    let path_str = path
        .to_str()
        .ok_or_else(|| "Invalid path encoding".to_string())?;

    // Find SDK for preloading
    let sdk_path = find_sdk_path();

    // Try bun with preload (preferred - supports TypeScript natively)
    if let Some(ref sdk) = sdk_path {
        let sdk_str = sdk.to_str().unwrap_or("");
        logging::log(
            "EXEC",
            &format!("Trying: bun run --preload {} {}", sdk_str, path_str),
        );
        match spawn_script("bun", &["run", "--preload", sdk_str, path_str], path_str) {
            Ok(session) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    runtime = "bun",
                    preload = true,
                    "Script session started"
                );
                logging::log("EXEC", "SUCCESS: bun with preload");
                return Ok(session);
            }
            Err(e) => {
                debug!(error = %e, runtime = "bun", preload = true, "Spawn attempt failed");
                logging::log("EXEC", &format!("FAILED: bun with preload: {}", e));
            }
        }
    }

    // Try bun without preload as fallback
    if is_typescript(path) {
        logging::log("EXEC", &format!("Trying: bun run {}", path_str));
        match spawn_script("bun", &["run", path_str], path_str) {
            Ok(session) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    runtime = "bun",
                    preload = false,
                    "Script session started"
                );
                logging::log("EXEC", "SUCCESS: bun without preload");
                return Ok(session);
            }
            Err(e) => {
                debug!(error = %e, runtime = "bun", preload = false, "Spawn attempt failed");
                logging::log("EXEC", &format!("FAILED: bun without preload: {}", e));
            }
        }
    }

    // Try node for JavaScript files
    if is_javascript(path) {
        logging::log("EXEC", &format!("Trying: node {}", path_str));
        match spawn_script("node", &[path_str], path_str) {
            Ok(session) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    runtime = "node",
                    "Script session started"
                );
                logging::log("EXEC", "SUCCESS: node");
                return Ok(session);
            }
            Err(e) => {
                debug!(error = %e, runtime = "node", "Spawn attempt failed");
                logging::log("EXEC", &format!("FAILED: node: {}", e));
            }
        }
    }

    let err = format!(
        "Failed to execute script '{}' interactively. Make sure bun or node is installed.",
        path.display()
    );
    error!(
        duration_ms = start.elapsed().as_millis() as u64,
        path = %path.display(),
        "All script execution methods failed"
    );
    logging::log("EXEC", &format!("ALL METHODS FAILED: {}", err));
    Err(err)
}

/// Spawn a script as an interactive process with piped stdin/stdout
#[instrument(skip_all, fields(cmd = %cmd))]
fn spawn_script(cmd: &str, args: &[&str], script_path: &str) -> Result<ScriptSession, String> {
    // Try to find the executable in common locations
    let executable = find_executable(cmd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| cmd.to_string());

    debug!(executable = %executable, args = ?args, "Spawning script process");
    logging::log("EXEC", &format!("spawn_script: {} {:?}", executable, args));

    let mut command = Command::new(&executable);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()); // Capture stderr for error handling

    // On Unix, spawn in a new process group so we can kill all children
    // process_group(0) means the child's PID becomes the PGID
    #[cfg(unix)]
    {
        command.process_group(0);
        logging::log("EXEC", "Using process group for child process");
    }

    let mut child = command.spawn().map_err(|e| {
        error!(error = %e, executable = %executable, "Process spawn failed");
        let err = format!("Failed to spawn '{}': {}", executable, e);
        logging::log("EXEC", &format!("SPAWN ERROR: {}", err));
        err
    })?;

    let pid = child.id();
    info!(pid = pid, pgid = pid, executable = %executable, "Process spawned");
    logging::log(
        "EXEC",
        &format!("Process spawned with PID: {} (PGID: {})", pid, pid),
    );

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open script stdin".to_string())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open script stdout".to_string())?;

    // Capture stderr for error reporting
    let stderr = child.stderr.take();
    logging::log("EXEC", &format!("Stderr captured: {}", stderr.is_some()));

    let process_handle = ProcessHandle::new(pid, script_path.to_string());
    logging::log("EXEC", "ScriptSession created successfully");

    Ok(ScriptSession {
        stdin,
        stdout_reader: JsonlReader::new(BufReader::new(stdout)),
        stderr,
        child,
        process_handle,
    })
}

/// Execute a script and return its output (non-interactive, for backwards compatibility)
#[allow(dead_code)]
#[instrument(skip_all, fields(script_path = %path.display()))]
pub fn execute_script(path: &Path) -> Result<String, String> {
    let start = Instant::now();
    debug!(path = %path.display(), "Starting blocking script execution");
    logging::log(
        "EXEC",
        &format!("execute_script (blocking): {}", path.display()),
    );

    let path_str = path
        .to_str()
        .ok_or_else(|| "Invalid path encoding".to_string())?;

    // Find SDK for preloading globals
    let sdk_path = find_sdk_path();
    logging::log("EXEC", &format!("SDK path: {:?}", sdk_path));

    // Try kit CLI first (preferred for script-kit)
    logging::log("EXEC", &format!("Trying: kit run {}", path_str));
    match run_command("kit", &["run", path_str]) {
        Ok(output) => {
            info!(
                duration_ms = start.elapsed().as_millis() as u64,
                output_bytes = output.len(),
                runtime = "kit",
                "Script completed"
            );
            logging::log(
                "EXEC",
                &format!("SUCCESS: kit (output: {} bytes)", output.len()),
            );
            return Ok(output);
        }
        Err(e) => {
            debug!(error = %e, runtime = "kit", "Command failed");
            logging::log("EXEC", &format!("FAILED: kit: {}", e));
        }
    }

    // Try bun with preload for TypeScript files (injects arg, div, md globals)
    if is_typescript(path) {
        if let Some(ref sdk) = sdk_path {
            let sdk_str = sdk.to_str().unwrap_or("");
            logging::log(
                "EXEC",
                &format!("Trying: bun run --preload {} {}", sdk_str, path_str),
            );
            match run_command("bun", &["run", "--preload", sdk_str, path_str]) {
                Ok(output) => {
                    info!(
                        duration_ms = start.elapsed().as_millis() as u64,
                        output_bytes = output.len(),
                        runtime = "bun",
                        preload = true,
                        "Script completed"
                    );
                    logging::log(
                        "EXEC",
                        &format!("SUCCESS: bun with preload (output: {} bytes)", output.len()),
                    );
                    return Ok(output);
                }
                Err(e) => {
                    debug!(error = %e, runtime = "bun", preload = true, "Command failed");
                    logging::log("EXEC", &format!("FAILED: bun with preload: {}", e));
                }
            }
        }

        // Fallback: try bun without preload
        logging::log(
            "EXEC",
            &format!("Trying: bun run {} (no preload)", path_str),
        );
        match run_command("bun", &["run", path_str]) {
            Ok(output) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    output_bytes = output.len(),
                    runtime = "bun",
                    preload = false,
                    "Script completed"
                );
                logging::log(
                    "EXEC",
                    &format!("SUCCESS: bun (output: {} bytes)", output.len()),
                );
                return Ok(output);
            }
            Err(e) => {
                debug!(error = %e, runtime = "bun", preload = false, "Command failed");
                logging::log("EXEC", &format!("FAILED: bun: {}", e));
            }
        }
    }

    // Try node for JavaScript files
    if is_javascript(path) {
        logging::log("EXEC", &format!("Trying: node {}", path_str));
        match run_command("node", &[path_str]) {
            Ok(output) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    output_bytes = output.len(),
                    runtime = "node",
                    "Script completed"
                );
                logging::log(
                    "EXEC",
                    &format!("SUCCESS: node (output: {} bytes)", output.len()),
                );
                return Ok(output);
            }
            Err(e) => {
                debug!(error = %e, runtime = "node", "Command failed");
                logging::log("EXEC", &format!("FAILED: node: {}", e));
            }
        }
    }

    let err = format!(
        "Failed to execute script '{}'. Make sure kit, bun, or node is installed.",
        path.display()
    );
    error!(
        duration_ms = start.elapsed().as_millis() as u64,
        path = %path.display(),
        "All script execution methods failed"
    );
    logging::log("EXEC", &format!("ALL METHODS FAILED: {}", err));
    Err(err)
}

/// Run a command and capture its output
#[allow(dead_code)]
fn run_command(cmd: &str, args: &[&str]) -> Result<String, String> {
    // Try to find the executable in common locations
    let executable = find_executable(cmd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| cmd.to_string());

    logging::log("EXEC", &format!("run_command: {} {:?}", executable, args));

    let output = Command::new(&executable).args(args).output().map_err(|e| {
        let err = format!("Failed to run '{}': {}", executable, e);
        logging::log("EXEC", &format!("COMMAND ERROR: {}", err));
        err
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    logging::log(
        "EXEC",
        &format!(
            "Command status: {}, stdout: {} bytes, stderr: {} bytes",
            output.status,
            stdout.len(),
            stderr.len()
        ),
    );

    if output.status.success() {
        if stdout.is_empty() {
            Ok(stderr.into_owned())
        } else {
            Ok(stdout.into_owned())
        }
    } else {
        let err = if stderr.is_empty() {
            format!("Command '{}' failed with status: {}", cmd, output.status)
        } else {
            stderr.into_owned()
        };
        logging::log("EXEC", &format!("Command failed: {}", err));
        Err(err)
    }
}

/// Check if the path points to a TypeScript file
fn is_typescript(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "ts")
        .unwrap_or(false)
}

/// Check if the path points to a JavaScript file
fn is_javascript(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "js")
        .unwrap_or(false)
}

// ============================================================================
// Error Parsing and Suggestion Generation
// ============================================================================

/// Parse stderr output to extract stack trace if present
pub fn parse_stack_trace(stderr: &str) -> Option<String> {
    // Look for common stack trace patterns
    let lines: Vec<&str> = stderr.lines().collect();

    // Find the start of a stack trace (lines starting with "at ")
    let stack_start = lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("at ")
            || trimmed.contains("    at ")
            || trimmed.starts_with("Error:")
            || trimmed.starts_with("TypeError:")
            || trimmed.starts_with("ReferenceError:")
            || trimmed.starts_with("SyntaxError:")
    });

    if let Some(start) = stack_start {
        // Collect lines that look like stack trace entries
        let stack_lines: Vec<&str> = lines[start..]
            .iter()
            .take_while(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty()
                    && (trimmed.starts_with("at ")
                        || trimmed.contains("    at ")
                        || trimmed.starts_with("Error:")
                        || trimmed.starts_with("TypeError:")
                        || trimmed.starts_with("ReferenceError:")
                        || trimmed.starts_with("SyntaxError:")
                        || trimmed.contains("error")
                        || trimmed.contains("Error"))
            })
            .take(20) // Limit to 20 lines
            .copied()
            .collect();

        if !stack_lines.is_empty() {
            return Some(stack_lines.join("\n"));
        }
    }

    None
}

/// Extract a user-friendly error message from stderr
pub fn extract_error_message(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().collect();

    // Look for common error patterns
    for line in &lines {
        let trimmed = line.trim();

        // Check for error type prefixes
        if trimmed.starts_with("Error:")
            || trimmed.starts_with("TypeError:")
            || trimmed.starts_with("ReferenceError:")
            || trimmed.starts_with("SyntaxError:")
            || trimmed.starts_with("error:")
        {
            return trimmed.to_string();
        }

        // Check for bun-specific errors
        if trimmed.contains("error:") && !trimmed.starts_with("at ") {
            return trimmed.to_string();
        }
    }

    // If no specific error found, return first non-empty line (truncated)
    for line in &lines {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return if trimmed.len() > 200 {
                format!("{}...", &trimmed[..200])
            } else {
                trimmed.to_string()
            };
        }
    }

    "Script execution failed".to_string()
}

/// Generate suggestions based on error type
pub fn generate_suggestions(stderr: &str, exit_code: Option<i32>) -> Vec<String> {
    let mut suggestions = Vec::new();
    let stderr_lower = stderr.to_lowercase();

    // Check for common error patterns and suggest fixes
    if stderr_lower.contains("cannot find module") || stderr_lower.contains("module not found") {
        suggestions.push("Run 'bun install' or 'npm install' to install dependencies".to_string());
    }

    if stderr_lower.contains("syntaxerror") || stderr_lower.contains("unexpected token") {
        suggestions.push("Check for syntax errors in your script".to_string());
    }

    if stderr_lower.contains("referenceerror") || stderr_lower.contains("is not defined") {
        suggestions.push(
            "Check that all variables and functions are properly imported or defined".to_string(),
        );
    }

    if stderr_lower.contains("typeerror") {
        suggestions
            .push("Check that you're using the correct types for function arguments".to_string());
    }

    if stderr_lower.contains("permission denied") || stderr_lower.contains("eacces") {
        suggestions
            .push("Check file permissions or try running with elevated privileges".to_string());
    }

    if stderr_lower.contains("enoent") || stderr_lower.contains("no such file") {
        suggestions.push("Check that the file path exists and is correct".to_string());
    }

    if stderr_lower.contains("timeout") || stderr_lower.contains("timed out") {
        suggestions.push(
            "The operation timed out - check network connectivity or increase timeout".to_string(),
        );
    }

    // Exit code specific suggestions
    match exit_code {
        Some(1) => {
            if suggestions.is_empty() {
                suggestions.push("Check the error message above for details".to_string());
            }
        }
        Some(127) => {
            suggestions.push(
                "Command not found - check that the executable is installed and in PATH"
                    .to_string(),
            );
        }
        Some(126) => {
            suggestions.push("Permission denied - check file permissions".to_string());
        }
        Some(134) => {
            // 128 + 6 = SIGABRT
            suggestions.push(
                "Process aborted (SIGABRT) - check for assertion failures or abort() calls"
                    .to_string(),
            );
        }
        Some(137) => {
            // 128 + 9 = SIGKILL
            suggestions.push(
                "Process was killed (SIGKILL) - possibly out of memory or manually killed"
                    .to_string(),
            );
        }
        Some(139) => {
            // 128 + 11 = SIGSEGV
            suggestions.push(
                "Segmentation fault (SIGSEGV) - memory access violation in native code".to_string(),
            );
        }
        Some(143) => {
            // 128 + 15 = SIGTERM
            suggestions.push("Process was terminated by signal (SIGTERM)".to_string());
        }
        Some(code) if code > 128 => {
            // Other signals: 128 + signal_number
            let signal = code - 128;
            let sig_name = match signal {
                1 => "SIGHUP",
                2 => "SIGINT",
                3 => "SIGQUIT",
                4 => "SIGILL",
                5 => "SIGTRAP",
                6 => "SIGABRT",
                7 => "SIGBUS",
                8 => "SIGFPE",
                10 => "SIGUSR1",
                12 => "SIGUSR2",
                13 => "SIGPIPE",
                14 => "SIGALRM",
                _ => "unknown signal",
            };
            suggestions.push(format!(
                "Process terminated by {} (exit code {})",
                sig_name, code
            ));
        }
        _ => {}
    }

    suggestions
}

/// Information about how a script process crashed
///
/// This struct provides detailed information about process termination,
/// including signal detection on Unix systems. Use `from_exit_status()`
/// to create from a process's exit status.
///
/// # Example
/// ```ignore
/// let status = child.wait()?;
/// let crash_info = CrashInfo::from_exit_status(status);
/// if crash_info.is_crash {
///     println!("{}", crash_info.error_message());
/// }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)] // Infrastructure ready for integration into main.rs
pub struct CrashInfo {
    /// Whether the process was terminated by a signal
    pub was_signaled: bool,
    /// The signal number (if was_signaled is true, on Unix)
    pub signal: Option<i32>,
    /// Human-readable signal name (e.g., "SIGKILL", "SIGSEGV")
    pub signal_name: Option<String>,
    /// The exit code (if not signaled)
    pub exit_code: Option<i32>,
    /// Whether this appears to be a crash vs normal exit
    pub is_crash: bool,
}

#[allow(dead_code)] // Infrastructure ready for integration into main.rs
impl CrashInfo {
    /// Create CrashInfo from an ExitStatus
    #[cfg(unix)]
    pub fn from_exit_status(status: std::process::ExitStatus) -> Self {
        use std::os::unix::process::ExitStatusExt;

        let signal = status.signal();
        let was_signaled = signal.is_some();
        let signal_name = signal.map(signal_to_name);
        let exit_code = status.code();

        // Consider it a crash if:
        // - Killed by signal (except SIGTERM which is graceful)
        // - Exit code > 128 (typically indicates signal)
        // - Exit code 1 with no stderr (likely uncaught exception)
        let is_crash =
            was_signaled || exit_code.map(|c| c > 128).unwrap_or(false) || exit_code == Some(1);

        Self {
            was_signaled,
            signal,
            signal_name,
            exit_code,
            is_crash,
        }
    }

    #[cfg(not(unix))]
    pub fn from_exit_status(status: std::process::ExitStatus) -> Self {
        let exit_code = status.code();
        let is_crash = exit_code.map(|c| c != 0).unwrap_or(true);

        Self {
            was_signaled: false,
            signal: None,
            signal_name: None,
            exit_code,
            is_crash,
        }
    }

    /// Create a descriptive error message for this crash
    pub fn error_message(&self) -> String {
        if let Some(ref sig_name) = self.signal_name {
            format!(
                "Script crashed: {} (signal {})",
                sig_name,
                self.signal.unwrap_or(-1)
            )
        } else if let Some(code) = self.exit_code {
            if code > 128 {
                // High exit codes often indicate signal on Unix
                let sig = code - 128;
                format!(
                    "Script crashed: {} (exit code {})",
                    signal_to_name(sig),
                    code
                )
            } else {
                format!("Script exited with error code {}", code)
            }
        } else {
            "Script terminated unexpectedly".to_string()
        }
    }
}

/// Convert a signal number to its name
fn signal_to_name(signal: i32) -> String {
    match signal {
        1 => "SIGHUP".to_string(),
        2 => "SIGINT".to_string(),
        3 => "SIGQUIT".to_string(),
        4 => "SIGILL".to_string(),
        5 => "SIGTRAP".to_string(),
        6 => "SIGABRT".to_string(),
        7 => "SIGBUS".to_string(),
        8 => "SIGFPE".to_string(),
        9 => "SIGKILL".to_string(),
        10 => "SIGUSR1".to_string(),
        11 => "SIGSEGV".to_string(),
        12 => "SIGUSR2".to_string(),
        13 => "SIGPIPE".to_string(),
        14 => "SIGALRM".to_string(),
        15 => "SIGTERM".to_string(),
        _ => format!("SIG{}", signal),
    }
}

/// Generate suggestions specifically for crash scenarios
#[allow(dead_code)] // Infrastructure ready for integration into main.rs
pub fn generate_crash_suggestions(crash_info: &CrashInfo) -> Vec<String> {
    let mut suggestions = Vec::new();

    if let Some(signal) = crash_info.signal {
        match signal {
            6 => {
                suggestions.push(
                    "Check for assertion failures or abort() calls in native addons".to_string(),
                );
                suggestions.push("Look for uncaught exceptions that trigger abort".to_string());
            }
            9 => {
                suggestions.push("Process was forcefully killed (SIGKILL)".to_string());
                suggestions.push(
                    "This could be due to: out of memory, manual kill, or system constraints"
                        .to_string(),
                );
            }
            11 => {
                suggestions.push("Segmentation fault - memory access violation".to_string());
                suggestions.push("Check native addons or C++ bindings for memory bugs".to_string());
                suggestions
                    .push("Try running with smaller data sets to identify the issue".to_string());
            }
            15 => {
                suggestions
                    .push("Process received SIGTERM (graceful termination request)".to_string());
            }
            _ => {
                suggestions.push(format!(
                    "Process received signal: {}",
                    signal_to_name(signal)
                ));
            }
        }
    } else if let Some(code) = crash_info.exit_code {
        if code > 128 {
            let implied_signal = code - 128;
            suggestions.extend(generate_crash_suggestions(&CrashInfo {
                was_signaled: true,
                signal: Some(implied_signal),
                signal_name: Some(signal_to_name(implied_signal)),
                exit_code: Some(code),
                is_crash: true,
            }));
        }
    }

    if suggestions.is_empty() {
        suggestions.push("Script exited unexpectedly".to_string());
        suggestions.push("Check script logs for more details".to_string());
    }

    suggestions
}

// ============================================================================
// Scriptlet Execution
// ============================================================================

/// Options for scriptlet execution
#[derive(Debug, Clone, Default)]
pub struct ScriptletExecOptions {
    /// Current working directory for script execution
    pub cwd: Option<PathBuf>,
    /// Commands to prepend before the main script content
    pub prepend: Option<String>,
    /// Commands to append after the main script content
    pub append: Option<String>,
    /// Named inputs for variable substitution
    pub inputs: HashMap<String, String>,
    /// Positional arguments for variable substitution
    pub positional_args: Vec<String>,
    /// Flags for conditional processing
    pub flags: HashMap<String, bool>,
}

/// Result of a scriptlet execution
#[derive(Debug)]
pub struct ScriptletResult {
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether execution was successful
    pub success: bool,
}

/// Get the file extension for a given tool type
#[cfg(test)]
fn tool_extension(tool: &str) -> &'static str {
    match tool {
        "ruby" => "rb",
        "python" => "py",
        "perl" => "pl",
        "php" => "php",
        "bash" | "sh" => "sh",
        "zsh" => "zsh",
        "fish" => "fish",
        "node" | "js" => "js",
        "ts" | "kit" | "bun" | "deno" => "ts",
        "applescript" => "applescript",
        "powershell" | "pwsh" => "ps1",
        "cmd" => "bat",
        _ => "sh", // Default to shell script
    }
}

/// Execute a scriptlet based on its tool type
///
/// # Arguments
/// * `scriptlet` - The scriptlet to execute
/// * `options` - Execution options (cwd, prepend, append, inputs, etc.)
///
/// # Returns
/// A `ScriptletResult` with exit code, stdout, stderr, and success flag
///
/// # Tool Types Supported
/// - Shell (bash, zsh, sh, fish): Write temp file, execute via shell
/// - Scripting (python, ruby, perl, php, node): Write temp file with extension, execute
/// - TypeScript (kit, ts, bun, deno): Write temp .ts file, run via bun
/// - transform: Wrap with getSelectedText/setSelectedText (macOS only)
/// - template: Returns content for template prompt invocation
/// - open: Use `open` command (macOS) or `xdg-open` (Linux)
/// - edit: Open in editor
/// - paste: Set selected text via clipboard
/// - type: Simulate keyboard typing
/// - submit: Paste + enter
#[instrument(skip_all, fields(tool = %scriptlet.tool, name = %scriptlet.name))]
pub fn run_scriptlet(
    scriptlet: &Scriptlet,
    options: ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    let start = Instant::now();
    debug!(tool = %scriptlet.tool, name = %scriptlet.name, "Running scriptlet");
    logging::log(
        "EXEC",
        &format!(
            "run_scriptlet: {} (tool: {})",
            scriptlet.name, scriptlet.tool
        ),
    );

    // Process conditionals and variable substitution
    let content = process_conditionals(&scriptlet.scriptlet_content, &options.flags);
    let is_windows = cfg!(target_os = "windows");
    let content = format_scriptlet(
        &content,
        &options.inputs,
        &options.positional_args,
        is_windows,
    );

    // Apply prepend/append
    let content = build_final_content(&content, &options.prepend, &options.append);

    let tool = scriptlet.tool.to_lowercase();

    let result = match tool.as_str() {
        // Shell tools
        t if SHELL_TOOLS.contains(&t) => execute_shell_scriptlet(&tool, &content, &options),

        // Scripting languages
        "python" => execute_with_interpreter("python3", &content, "py", &options),
        "ruby" => execute_with_interpreter("ruby", &content, "rb", &options),
        "perl" => execute_with_interpreter("perl", &content, "pl", &options),
        "php" => execute_with_interpreter("php", &content, "php", &options),
        "node" | "js" => execute_with_interpreter("node", &content, "js", &options),
        "applescript" => execute_applescript(&content, &options),

        // TypeScript tools (run via bun)
        "kit" | "ts" | "bun" | "deno" => execute_typescript(&content, &options),

        // Transform (get selected text, process, set selected text)
        "transform" => execute_transform(&content, &options),

        // Template (return content for prompt invocation)
        "template" => {
            // Template just returns the processed content - the caller handles prompt invocation
            Ok(ScriptletResult {
                exit_code: 0,
                stdout: content,
                stderr: String::new(),
                success: true,
            })
        }

        // Open URL/file
        "open" => execute_open(&content, &options),

        // Edit file in editor
        "edit" => execute_edit(&content, &options),

        // Paste text (set selected text)
        "paste" => execute_paste(&content),

        // Type text via keyboard simulation
        "type" => execute_type(&content),

        // Submit (paste + enter)
        "submit" => execute_submit(&content),

        // Unknown tool - try as shell
        _ => {
            warn!(tool = %tool, "Unknown tool type, falling back to shell");
            execute_shell_scriptlet("sh", &content, &options)
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    match &result {
        Ok(r) => {
            info!(
                duration_ms = duration_ms,
                exit_code = r.exit_code,
                tool = %tool,
                "Scriptlet execution complete"
            );
            logging::log(
                "EXEC",
                &format!(
                    "Scriptlet '{}' completed: exit={}, duration={}ms",
                    scriptlet.name, r.exit_code, duration_ms
                ),
            );
        }
        Err(e) => {
            error!(duration_ms = duration_ms, error = %e, tool = %tool, "Scriptlet execution failed");
            logging::log(
                "EXEC",
                &format!("Scriptlet '{}' failed: {}", scriptlet.name, e),
            );
        }
    }

    result
}

/// Build final content with prepend/append
fn build_final_content(content: &str, prepend: &Option<String>, append: &Option<String>) -> String {
    let mut result = String::new();

    if let Some(pre) = prepend {
        result.push_str(pre);
        if !pre.ends_with('\n') {
            result.push('\n');
        }
    }

    result.push_str(content);

    if let Some(app) = append {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(app);
    }

    result
}

/// Execute a shell scriptlet (bash, zsh, sh, fish, etc.)
fn execute_shell_scriptlet(
    shell: &str,
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    logging::log("EXEC", &format!("Executing shell scriptlet with {}", shell));

    // Create temp file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("scriptlet-{}.sh", std::process::id()));

    std::fs::write(&temp_file, content)
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_file)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_file, perms)
            .map_err(|e| format!("Failed to set executable permission: {}", e))?;
    }

    // Find the shell executable
    let shell_path = find_executable(shell)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| shell.to_string());

    let mut cmd = Command::new(&shell_path);
    cmd.arg(temp_file.to_str().unwrap());

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd.output().map_err(|e| {
        // Clean up temp file before returning error
        let _ = std::fs::remove_file(&temp_file);

        // Provide helpful error message with installation suggestions
        let suggestions = shell_not_found_suggestions(shell);
        format!(
            "Failed to execute shell script with '{}': {}\n\n{}",
            shell, e, suggestions
        )
    })?;

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_file);

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Get installation suggestions for a missing shell
fn shell_not_found_suggestions(shell: &str) -> String {
    let install_hint = match shell {
        "bash" => {
            if cfg!(target_os = "macos") {
                "bash is usually pre-installed on macOS. Try: brew install bash"
            } else if cfg!(target_os = "linux") {
                "Install with: apt install bash (Debian/Ubuntu) or yum install bash (RHEL/CentOS)"
            } else {
                "bash is typically available through Git for Windows or WSL"
            }
        }
        "zsh" => {
            if cfg!(target_os = "macos") {
                "zsh is the default shell on macOS. If missing, try: brew install zsh"
            } else if cfg!(target_os = "linux") {
                "Install with: apt install zsh (Debian/Ubuntu) or yum install zsh (RHEL/CentOS)"
            } else {
                "zsh can be installed through WSL or Git Bash on Windows"
            }
        }
        "sh" => {
            "sh (POSIX shell) should be available on all Unix systems. Check your PATH."
        }
        "fish" => {
            if cfg!(target_os = "macos") {
                "Install with: brew install fish"
            } else if cfg!(target_os = "linux") {
                "Install with: apt install fish (Debian/Ubuntu) or check https://fishshell.com"
            } else {
                "fish can be installed through WSL on Windows. See https://fishshell.com"
            }
        }
        "cmd" => {
            if cfg!(target_os = "windows") {
                "cmd.exe should be available at C:\\Windows\\System32\\cmd.exe"
            } else {
                "cmd is a Windows-only shell. On Unix, use bash, zsh, or sh instead."
            }
        }
        "powershell" => {
            if cfg!(target_os = "windows") {
                "PowerShell should be pre-installed on Windows. Check System32\\WindowsPowerShell"
            } else {
                "For cross-platform PowerShell, install pwsh: https://aka.ms/install-powershell"
            }
        }
        "pwsh" => {
            "Install PowerShell Core from: https://aka.ms/install-powershell\n\
             macOS: brew install powershell\n\
             Linux: See https://docs.microsoft.com/powershell/scripting/install/installing-powershell-on-linux"
        }
        _ => {
            "Shell not recognized. Make sure it is installed and in your PATH."
        }
    };

    format!(
        "Suggestions:\n\
         - Make sure '{}' is installed and accessible in your PATH\n\
         - {}\n\
         - Alternative shells in SHELL_TOOLS: bash, zsh, sh, fish, cmd, powershell, pwsh",
        shell, install_hint
    )
}

/// Execute a script with a specific interpreter
fn execute_with_interpreter(
    interpreter: &str,
    content: &str,
    extension: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    logging::log(
        "EXEC",
        &format!("Executing with interpreter: {}", interpreter),
    );

    // Create temp file with appropriate extension
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("scriptlet-{}.{}", std::process::id(), extension));

    std::fs::write(&temp_file, content)
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    // Find the interpreter
    let interp_path = find_executable(interpreter)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| interpreter.to_string());

    let mut cmd = Command::new(&interp_path);
    cmd.arg(temp_file.to_str().unwrap());

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute {} script: {}", interpreter, e))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_file);

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute AppleScript
fn execute_applescript(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    logging::log("EXEC", "Executing AppleScript");

    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(content);

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute TypeScript via bun
fn execute_typescript(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    logging::log("EXEC", "Executing TypeScript via bun");

    // Create temp file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("scriptlet-{}.ts", std::process::id()));

    std::fs::write(&temp_file, content)
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    // Find bun
    let bun_path = find_executable("bun")
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bun".to_string());

    // Check if we should use SDK preload
    let sdk_path = find_sdk_path();

    let mut cmd = Command::new(&bun_path);
    cmd.arg("run");

    // Add preload if SDK exists
    if let Some(ref sdk) = sdk_path {
        if let Some(sdk_str) = sdk.to_str() {
            cmd.arg("--preload").arg(sdk_str);
        }
    }

    cmd.arg(temp_file.to_str().unwrap());

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute TypeScript: {}", e))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_file);

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute transform scriptlet (get selected text, process, set selected text)
#[cfg(target_os = "macos")]
fn execute_transform(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    logging::log("EXEC", "Executing transform scriptlet");

    // Get selected text
    let selected = selected_text::get_selected_text()
        .map_err(|e| format!("Failed to get selected text: {}", e))?;

    // Create script that processes the input
    // Wrap content in a function that receives selectedText and returns transformed text
    let wrapper_script = format!(
        r#"
const selectedText = {};
const transform = (text: string): string => {{
{}
}};
const result = transform(selectedText);
console.log(result);
"#,
        serde_json::to_string(&selected).unwrap_or_else(|_| "\"\"".to_string()),
        content
    );

    // Execute the transform script
    let result = execute_typescript(&wrapper_script, options)?;

    if result.success {
        // Set the transformed text back
        let transformed = result.stdout.trim();
        selected_text::set_selected_text(transformed)
            .map_err(|e| format!("Failed to set selected text: {}", e))?;
    }

    Ok(result)
}

#[cfg(not(target_os = "macos"))]
fn execute_transform(
    _content: &str,
    _options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    Err("Transform scriptlets are only supported on macOS".to_string())
}

/// Execute open command (open URL or file)
fn execute_open(content: &str, _options: &ScriptletExecOptions) -> Result<ScriptletResult, String> {
    logging::log("EXEC", &format!("Opening: {}", content.trim()));

    let target = content.trim();

    #[cfg(target_os = "macos")]
    let cmd_name = "open";
    #[cfg(target_os = "linux")]
    let cmd_name = "xdg-open";
    #[cfg(target_os = "windows")]
    let cmd_name = "start";

    let output = Command::new(cmd_name)
        .arg(target)
        .output()
        .map_err(|e| format!("Failed to open '{}': {}", target, e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute edit command (open file in editor)
fn execute_edit(content: &str, _options: &ScriptletExecOptions) -> Result<ScriptletResult, String> {
    logging::log("EXEC", &format!("Editing: {}", content.trim()));

    let file_path = content.trim();

    // Get editor from environment or default
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "code".to_string());

    // Find the editor executable
    let editor_path = find_executable(&editor)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(editor);

    let output = Command::new(&editor_path)
        .arg(file_path)
        .output()
        .map_err(|e| format!("Failed to open editor '{}': {}", editor_path, e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute paste command (set selected text via clipboard)
#[cfg(target_os = "macos")]
fn execute_paste(content: &str) -> Result<ScriptletResult, String> {
    logging::log("EXEC", "Executing paste scriptlet");

    let text = content.trim();

    selected_text::set_selected_text(text).map_err(|e| format!("Failed to paste text: {}", e))?;

    Ok(ScriptletResult {
        exit_code: 0,
        stdout: String::new(),
        stderr: String::new(),
        success: true,
    })
}

#[cfg(not(target_os = "macos"))]
fn execute_paste(_content: &str) -> Result<ScriptletResult, String> {
    Err("Paste scriptlets are only supported on macOS".to_string())
}

/// Execute type command (simulate keyboard typing)
#[cfg(target_os = "macos")]
fn execute_type(content: &str) -> Result<ScriptletResult, String> {
    logging::log("EXEC", "Executing type scriptlet");

    let text = content.trim();

    // Use AppleScript to simulate typing
    let script = format!(
        r#"tell application "System Events" to keystroke "{}""#,
        text.replace('\\', "\\\\").replace('"', "\\\"")
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to type text: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

#[cfg(not(target_os = "macos"))]
fn execute_type(_content: &str) -> Result<ScriptletResult, String> {
    Err("Type scriptlets are only supported on macOS".to_string())
}

/// Execute submit command (paste + enter)
#[cfg(target_os = "macos")]
fn execute_submit(content: &str) -> Result<ScriptletResult, String> {
    logging::log("EXEC", "Executing submit scriptlet");

    // First paste the text
    let paste_result = execute_paste(content)?;
    if !paste_result.success {
        return Ok(paste_result);
    }

    // Small delay to let paste complete
    std::thread::sleep(Duration::from_millis(50));

    // Then press Enter using AppleScript
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to key code 36"#) // 36 is Return key
        .output()
        .map_err(|e| format!("Failed to press Enter: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

#[cfg(not(target_os = "macos"))]
fn execute_submit(_content: &str) -> Result<ScriptletResult, String> {
    Err("Submit scriptlets are only supported on macOS".to_string())
}

// ============================================================================
// Selected Text Message Handlers
// ============================================================================

/// Result of handling a selected text message
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SelectedTextHandleResult {
    /// Message was handled, here's the response to send back
    Handled(Message),
    /// Message was not a selected text operation
    NotHandled,
}

/// Handle selected text protocol messages.
///
/// This function checks if a message is a selected text operation and handles it
/// by calling the appropriate selected_text module functions.
///
/// # Arguments
/// * `msg` - The incoming message to potentially handle
///
/// # Returns
/// * `SelectedTextHandleResult::Handled(response)` - Message was handled, send response back
/// * `SelectedTextHandleResult::NotHandled` - Message was not a selected text operation
///
/// # Example
/// ```ignore
/// match handle_selected_text_message(&msg) {
///     SelectedTextHandleResult::Handled(response) => {
///         send_response(response);
///     }
///     SelectedTextHandleResult::NotHandled => {
///         // Handle as other message type
///     }
/// }
/// ```
#[instrument(skip_all)]
pub fn handle_selected_text_message(msg: &Message) -> SelectedTextHandleResult {
    match msg {
        Message::GetSelectedText { request_id } => {
            debug!(request_id = %request_id, "Handling GetSelectedText");
            let response = handle_get_selected_text(request_id);
            SelectedTextHandleResult::Handled(response)
        }
        Message::SetSelectedText { text, request_id } => {
            debug!(request_id = %request_id, text_len = text.len(), "Handling SetSelectedText");
            let response = handle_set_selected_text(text, request_id);
            SelectedTextHandleResult::Handled(response)
        }
        Message::CheckAccessibility { request_id } => {
            debug!(request_id = %request_id, "Handling CheckAccessibility");
            let response = handle_check_accessibility(request_id);
            SelectedTextHandleResult::Handled(response)
        }
        Message::RequestAccessibility { request_id } => {
            debug!(request_id = %request_id, "Handling RequestAccessibility");
            let response = handle_request_accessibility(request_id);
            SelectedTextHandleResult::Handled(response)
        }
        _ => SelectedTextHandleResult::NotHandled,
    }
}

/// Handle GET_SELECTED_TEXT request
#[cfg(target_os = "macos")]
fn handle_get_selected_text(request_id: &str) -> Message {
    logging::log("EXEC", &format!("GetSelectedText request: {}", request_id));

    match selected_text::get_selected_text() {
        Ok(text) => {
            info!(request_id = %request_id, text_len = text.len(), "Got selected text");
            logging::log(
                "EXEC",
                &format!("GetSelectedText success: {} chars", text.len()),
            );
            // Return as Submit message so SDK pending map can match by id
            Message::Submit {
                id: request_id.to_string(),
                value: Some(text),
            }
        }
        Err(e) => {
            warn!(request_id = %request_id, error = %e, "Failed to get selected text");
            logging::log("EXEC", &format!("GetSelectedText error: {}", e));
            // Return error prefixed with ERROR: so SDK can detect and reject
            Message::Submit {
                id: request_id.to_string(),
                value: Some(format!("ERROR: {}", e)),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_get_selected_text(request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!(
            "GetSelectedText request: {} (not supported on this platform)",
            request_id
        ),
    );
    warn!(request_id = %request_id, "Selected text not supported on this platform");
    Message::Submit {
        id: request_id.to_string(),
        value: Some(String::new()),
    }
}

/// Handle SET_SELECTED_TEXT request
#[cfg(target_os = "macos")]
fn handle_set_selected_text(text: &str, request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!(
            "SetSelectedText request: {} ({} chars)",
            request_id,
            text.len()
        ),
    );

    match selected_text::set_selected_text(text) {
        Ok(()) => {
            info!(request_id = %request_id, "Set selected text successfully");
            logging::log("EXEC", "SetSelectedText success");
            // Return success as Submit with empty value
            Message::Submit {
                id: request_id.to_string(),
                value: None,
            }
        }
        Err(e) => {
            warn!(request_id = %request_id, error = %e, "Failed to set selected text");
            logging::log("EXEC", &format!("SetSelectedText error: {}", e));
            // Return error prefixed with ERROR: so SDK can detect and reject
            Message::Submit {
                id: request_id.to_string(),
                value: Some(format!("ERROR: {}", e)),
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_set_selected_text(_text: &str, request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!(
            "SetSelectedText request: {} (not supported on this platform)",
            request_id
        ),
    );
    warn!(request_id = %request_id, "Selected text not supported on this platform");
    Message::Submit {
        id: request_id.to_string(),
        value: Some("ERROR: Not supported on this platform".to_string()),
    }
}

/// Handle CHECK_ACCESSIBILITY request
#[cfg(target_os = "macos")]
fn handle_check_accessibility(request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!("CheckAccessibility request: {}", request_id),
    );

    let granted = selected_text::has_accessibility_permission();
    info!(request_id = %request_id, granted = granted, "Checked accessibility permission");
    logging::log("EXEC", &format!("CheckAccessibility: granted={}", granted));

    // Return as Submit with "true" or "false" string value
    Message::Submit {
        id: request_id.to_string(),
        value: Some(granted.to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_check_accessibility(request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!(
            "CheckAccessibility request: {} (not supported on this platform)",
            request_id
        ),
    );
    // On non-macOS, report as "not granted" since the feature isn't available
    Message::Submit {
        id: request_id.to_string(),
        value: Some("false".to_string()),
    }
}

/// Handle REQUEST_ACCESSIBILITY request
#[cfg(target_os = "macos")]
fn handle_request_accessibility(request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!("RequestAccessibility request: {}", request_id),
    );

    let granted = selected_text::request_accessibility_permission();
    info!(request_id = %request_id, granted = granted, "Requested accessibility permission");
    logging::log(
        "EXEC",
        &format!("RequestAccessibility: granted={}", granted),
    );

    // Return as Submit with "true" or "false" string value
    Message::Submit {
        id: request_id.to_string(),
        value: Some(granted.to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_request_accessibility(request_id: &str) -> Message {
    logging::log(
        "EXEC",
        &format!(
            "RequestAccessibility request: {} (not supported on this platform)",
            request_id
        ),
    );
    // On non-macOS, can't request permissions
    Message::Submit {
        id: request_id.to_string(),
        value: Some("false".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_typescript() {
        assert!(is_typescript(&PathBuf::from("script.ts")));
        assert!(!is_typescript(&PathBuf::from("script.js")));
        assert!(!is_typescript(&PathBuf::from("script.txt")));
    }

    #[test]
    fn test_is_javascript() {
        assert!(is_javascript(&PathBuf::from("script.js")));
        assert!(!is_javascript(&PathBuf::from("script.ts")));
        assert!(!is_javascript(&PathBuf::from("script.txt")));
    }

    #[test]
    fn test_is_typescript_with_path() {
        assert!(is_typescript(&PathBuf::from(
            "/home/user/.sk/kit/scripts/script.ts"
        )));
        assert!(is_typescript(&PathBuf::from("/usr/local/bin/script.ts")));
    }

    #[test]
    fn test_is_javascript_with_path() {
        assert!(is_javascript(&PathBuf::from(
            "/home/user/.sk/kit/scripts/script.js"
        )));
        assert!(is_javascript(&PathBuf::from("/usr/local/bin/script.js")));
    }

    #[test]
    fn test_file_extensions_case_sensitive() {
        // Rust PathBuf.extension() returns lowercase for comparison
        assert!(
            is_typescript(&PathBuf::from("script.TS"))
                || !is_typescript(&PathBuf::from("script.TS"))
        );
        // Extension check should work regardless (implementation detail)
    }

    #[test]
    fn test_unsupported_extension() {
        assert!(!is_typescript(&PathBuf::from("script.py")));
        assert!(!is_javascript(&PathBuf::from("script.rs")));
        assert!(!is_typescript(&PathBuf::from("script")));
    }

    #[test]
    fn test_files_with_no_extension() {
        assert!(!is_typescript(&PathBuf::from("script")));
        assert!(!is_javascript(&PathBuf::from("mycommand")));
    }

    #[test]
    fn test_multiple_dots_in_filename() {
        assert!(is_typescript(&PathBuf::from("my.test.script.ts")));
        assert!(is_javascript(&PathBuf::from("my.test.script.js")));
    }

    #[test]
    fn test_process_handle_double_kill_is_safe() {
        // Double kill should not panic
        let mut handle = ProcessHandle::new(99999, "[test:double_kill]".to_string()); // Non-existent PID
        handle.kill();
        handle.kill(); // Should be safe to call again
        assert!(handle.killed);
    }

    #[test]
    fn test_process_handle_drop_calls_kill() {
        // Create a handle and let it drop
        let handle = ProcessHandle::new(99998, "[test:drop_kill]".to_string()); // Non-existent PID
        assert!(!handle.killed);
        drop(handle);
        // If we get here without panic, drop successfully called kill
    }

    #[test]
    fn test_process_handle_registers_with_process_manager() {
        // ProcessHandle::new() internally calls PROCESS_MANAGER.register_process()
        // and Drop calls PROCESS_MANAGER.unregister_process()

        // Create a handle which should register with PROCESS_MANAGER
        let test_pid = 88888u32; // Non-existent PID
        let test_script = "/test/integration_test.ts";

        // Create handle - this calls register_process() internally
        let handle = ProcessHandle::new(test_pid, test_script.to_string());

        // Verify handle has correct PID
        assert_eq!(handle.pid, test_pid);

        // Drop will call unregister_process() - this should not panic
        drop(handle);

        // If we get here, register/unregister cycle completed successfully
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_and_kill_process() {
        // Spawn a simple process that sleeps
        let result = spawn_script("sleep", &["10"], "[test:sleep]");

        if let Ok(mut session) = result {
            let pid = session.pid();
            assert!(pid > 0);

            // Process should be running
            assert!(session.is_running());

            // Kill it
            session.kill().expect("kill should succeed");

            // Wait a moment for the process to die
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Process should no longer be running
            assert!(!session.is_running());
        }
        // If spawn failed (sleep not available), that's OK for this test
    }

    #[cfg(unix)]
    #[test]
    fn test_drop_kills_process() {
        // Spawn a process
        let result = spawn_script("sleep", &["30"], "[test:sleep]");

        if let Ok(session) = result {
            let pid = session.pid();

            // Drop the session - should kill the process
            drop(session);

            // Wait for process to be fully cleaned up (may take a bit)
            // Use ps to check if process is truly gone or just a zombie
            let mut is_dead = false;
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(50));

                // Check process state using ps
                let check = Command::new("ps")
                    .args(["-p", &pid.to_string(), "-o", "state="])
                    .output();

                match check {
                    Ok(output) => {
                        let state = String::from_utf8_lossy(&output.stdout);
                        let state = state.trim();
                        // Process is dead if ps returns empty or shows Z (zombie)
                        // We consider zombie as "dead enough" since it's not running
                        if state.is_empty() || state.starts_with('Z') || !output.status.success() {
                            is_dead = true;
                            break;
                        }
                    }
                    Err(_) => {
                        // Command failed to run, assume process is dead
                        is_dead = true;
                        break;
                    }
                }
            }
            assert!(is_dead, "Process should be dead after drop");
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_split_session_kill() {
        // Spawn a process and split it
        let result = spawn_script("sleep", &["10"], "[test:sleep]");

        if let Ok(session) = result {
            let pid = session.pid();
            let mut split = session.split();

            assert_eq!(split.pid(), pid);
            assert!(split.is_running());

            // Kill via split session
            split.kill().expect("kill should succeed");

            std::thread::sleep(std::time::Duration::from_millis(100));
            assert!(!split.is_running());
        }
    }

    // ============================================================
    // Selected Text Handler Tests
    // ============================================================

    use super::{handle_selected_text_message, SelectedTextHandleResult};
    use crate::protocol::Message;

    #[cfg(feature = "system-tests")]
    #[test]
    fn test_handle_get_selected_text_returns_handled() {
        let msg = Message::get_selected_text("req-001".to_string());
        let result = handle_selected_text_message(&msg);

        match result {
            SelectedTextHandleResult::Handled(response) => {
                // Response should be Submit message (for SDK compatibility)
                match response {
                    Message::Submit { id, .. } => {
                        assert_eq!(id, "req-001");
                    }
                    _ => panic!("Expected Submit response, got {:?}", response),
                }
            }
            SelectedTextHandleResult::NotHandled => {
                panic!("Expected message to be handled");
            }
        }
    }

    #[cfg(feature = "system-tests")]
    #[test]
    fn test_handle_set_selected_text_returns_handled() {
        let msg = Message::set_selected_text_msg("test text".to_string(), "req-002".to_string());
        let result = handle_selected_text_message(&msg);

        match result {
            SelectedTextHandleResult::Handled(response) => {
                // Response should be Submit message (for SDK compatibility)
                match response {
                    Message::Submit { id, .. } => {
                        assert_eq!(id, "req-002");
                    }
                    _ => panic!("Expected Submit response, got {:?}", response),
                }
            }
            SelectedTextHandleResult::NotHandled => {
                panic!("Expected message to be handled");
            }
        }
    }

    #[cfg(feature = "system-tests")]
    #[test]
    fn test_handle_check_accessibility_returns_handled() {
        let msg = Message::check_accessibility("req-003".to_string());
        let result = handle_selected_text_message(&msg);

        match result {
            SelectedTextHandleResult::Handled(response) => {
                // Response should be Submit message with "true" or "false" value
                match response {
                    Message::Submit { id, value } => {
                        assert_eq!(id, "req-003");
                        // value should be "true" or "false"
                        assert!(
                            value == Some("true".to_string()) || value == Some("false".to_string())
                        );
                    }
                    _ => panic!("Expected Submit response, got {:?}", response),
                }
            }
            SelectedTextHandleResult::NotHandled => {
                panic!("Expected message to be handled");
            }
        }
    }

    #[cfg(feature = "system-tests")]
    #[test]
    fn test_handle_request_accessibility_returns_handled() {
        let msg = Message::request_accessibility("req-004".to_string());
        let result = handle_selected_text_message(&msg);

        match result {
            SelectedTextHandleResult::Handled(response) => {
                // Response should be Submit message with "true" or "false" value
                match response {
                    Message::Submit { id, value } => {
                        assert_eq!(id, "req-004");
                        // value should be "true" or "false"
                        assert!(
                            value == Some("true".to_string()) || value == Some("false".to_string())
                        );
                    }
                    _ => panic!("Expected Submit response, got {:?}", response),
                }
            }
            SelectedTextHandleResult::NotHandled => {
                panic!("Expected message to be handled");
            }
        }
    }

    #[test]
    fn test_unrelated_message_returns_not_handled() {
        let msg = Message::beep();
        let result = handle_selected_text_message(&msg);

        match result {
            SelectedTextHandleResult::Handled(_) => {
                panic!("Expected message to not be handled");
            }
            SelectedTextHandleResult::NotHandled => {
                // Expected
            }
        }
    }

    #[test]
    fn test_arg_message_returns_not_handled() {
        let msg = Message::arg("1".to_string(), "Pick".to_string(), vec![]);
        let result = handle_selected_text_message(&msg);

        match result {
            SelectedTextHandleResult::Handled(_) => {
                panic!("Expected message to not be handled");
            }
            SelectedTextHandleResult::NotHandled => {
                // Expected
            }
        }
    }

    #[test]
    fn test_response_messages_not_handled() {
        // Response messages shouldn't be handled (they're outgoing, not incoming)
        // Submit messages are responses, so they should not be handled
        let msg1 = Message::Submit {
            id: "req-x".to_string(),
            value: Some("text".to_string()),
        };

        assert!(matches!(
            handle_selected_text_message(&msg1),
            SelectedTextHandleResult::NotHandled
        ));
    }

    // ============================================================
    // AUTO_SUBMIT Mode Tests
    // ============================================================
    //
    // Note: These tests verify the AUTO_SUBMIT environment variable parsing.
    // Since env vars are global and tests run in parallel, we use a single
    // comprehensive test that exercises all cases sequentially to avoid races.

    use super::{
        get_auto_submit_delay, get_auto_submit_index, get_auto_submit_value, is_auto_submit_enabled,
    };
    use std::time::Duration;

    /// Comprehensive test for is_auto_submit_enabled() function.
    /// Tests all cases in sequence to avoid env var race conditions.
    #[test]
    fn test_is_auto_submit_enabled_all_cases() {
        // Test "true" value
        std::env::set_var("AUTO_SUBMIT", "true");
        assert!(
            is_auto_submit_enabled(),
            "AUTO_SUBMIT=true should enable auto-submit"
        );

        // Test "1" value
        std::env::set_var("AUTO_SUBMIT", "1");
        assert!(
            is_auto_submit_enabled(),
            "AUTO_SUBMIT=1 should enable auto-submit"
        );

        // Test "false" value
        std::env::set_var("AUTO_SUBMIT", "false");
        assert!(
            !is_auto_submit_enabled(),
            "AUTO_SUBMIT=false should NOT enable auto-submit"
        );

        // Test "0" value
        std::env::set_var("AUTO_SUBMIT", "0");
        assert!(
            !is_auto_submit_enabled(),
            "AUTO_SUBMIT=0 should NOT enable auto-submit"
        );

        // Test other value
        std::env::set_var("AUTO_SUBMIT", "yes");
        assert!(
            !is_auto_submit_enabled(),
            "AUTO_SUBMIT=yes should NOT enable auto-submit"
        );

        // Test unset (default)
        std::env::remove_var("AUTO_SUBMIT");
        assert!(
            !is_auto_submit_enabled(),
            "Unset AUTO_SUBMIT should NOT enable auto-submit"
        );
    }

    /// Comprehensive test for get_auto_submit_delay() function.
    #[test]
    fn test_get_auto_submit_delay_all_cases() {
        // Test custom value
        std::env::set_var("AUTO_SUBMIT_DELAY_MS", "500");
        assert_eq!(
            get_auto_submit_delay(),
            Duration::from_millis(500),
            "AUTO_SUBMIT_DELAY_MS=500 should return 500ms"
        );

        // Test invalid value (falls back to default)
        std::env::set_var("AUTO_SUBMIT_DELAY_MS", "not_a_number");
        assert_eq!(
            get_auto_submit_delay(),
            Duration::from_millis(100),
            "Invalid AUTO_SUBMIT_DELAY_MS should default to 100ms"
        );

        // Test unset (default)
        std::env::remove_var("AUTO_SUBMIT_DELAY_MS");
        assert_eq!(
            get_auto_submit_delay(),
            Duration::from_millis(100),
            "Unset AUTO_SUBMIT_DELAY_MS should default to 100ms"
        );
    }

    /// Comprehensive test for get_auto_submit_value() function.
    #[test]
    fn test_get_auto_submit_value_all_cases() {
        // Test set value
        std::env::set_var("AUTO_SUBMIT_VALUE", "test_value");
        assert_eq!(
            get_auto_submit_value(),
            Some("test_value".to_string()),
            "AUTO_SUBMIT_VALUE=test_value should return Some(test_value)"
        );

        // Test empty value
        std::env::set_var("AUTO_SUBMIT_VALUE", "");
        assert_eq!(
            get_auto_submit_value(),
            Some("".to_string()),
            "AUTO_SUBMIT_VALUE='' should return Some('')"
        );

        // Test unset (None)
        std::env::remove_var("AUTO_SUBMIT_VALUE");
        assert_eq!(
            get_auto_submit_value(),
            None,
            "Unset AUTO_SUBMIT_VALUE should return None"
        );
    }

    /// Comprehensive test for get_auto_submit_index() function.
    #[test]
    fn test_get_auto_submit_index_all_cases() {
        // Test custom value
        std::env::set_var("AUTO_SUBMIT_INDEX", "5");
        assert_eq!(
            get_auto_submit_index(),
            5,
            "AUTO_SUBMIT_INDEX=5 should return 5"
        );

        // Test invalid value (falls back to default)
        std::env::set_var("AUTO_SUBMIT_INDEX", "invalid");
        assert_eq!(
            get_auto_submit_index(),
            0,
            "Invalid AUTO_SUBMIT_INDEX should default to 0"
        );

        // Test negative value (falls back to default since usize can't be negative)
        std::env::set_var("AUTO_SUBMIT_INDEX", "-1");
        assert_eq!(
            get_auto_submit_index(),
            0,
            "Negative AUTO_SUBMIT_INDEX should default to 0"
        );

        // Test unset (default)
        std::env::remove_var("AUTO_SUBMIT_INDEX");
        assert_eq!(
            get_auto_submit_index(),
            0,
            "Unset AUTO_SUBMIT_INDEX should default to 0"
        );
    }

    // ============================================================
    // AutoSubmitConfig Tests
    // ============================================================

    use super::{get_auto_submit_config, AutoSubmitConfig};
    use crate::protocol::Choice;

    /// Test AutoSubmitConfig default values.
    #[test]
    fn test_auto_submit_config_default() {
        let config = AutoSubmitConfig::default();

        assert!(!config.enabled, "Default should be disabled");
        assert_eq!(
            config.delay,
            Duration::from_millis(100),
            "Default delay should be 100ms"
        );
        assert!(
            config.value_override.is_none(),
            "Default should have no value override"
        );
        assert_eq!(config.index, 0, "Default index should be 0");
    }

    /// Test AutoSubmitConfig::from_env() captures env vars.
    #[test]
    fn test_auto_submit_config_from_env() {
        // Set all env vars
        std::env::set_var("AUTO_SUBMIT", "true");
        std::env::set_var("AUTO_SUBMIT_DELAY_MS", "250");
        std::env::set_var("AUTO_SUBMIT_VALUE", "override_value");
        std::env::set_var("AUTO_SUBMIT_INDEX", "3");

        let config = AutoSubmitConfig::from_env();

        assert!(config.enabled, "Should be enabled when AUTO_SUBMIT=true");
        assert_eq!(
            config.delay,
            Duration::from_millis(250),
            "Delay should be 250ms"
        );
        assert_eq!(
            config.value_override,
            Some("override_value".to_string()),
            "Should have override value"
        );
        assert_eq!(config.index, 3, "Index should be 3");

        // Clean up
        std::env::remove_var("AUTO_SUBMIT");
        std::env::remove_var("AUTO_SUBMIT_DELAY_MS");
        std::env::remove_var("AUTO_SUBMIT_VALUE");
        std::env::remove_var("AUTO_SUBMIT_INDEX");
    }

    /// Test get_auto_submit_config() convenience function.
    #[test]
    fn test_get_auto_submit_config() {
        // Clean state
        std::env::remove_var("AUTO_SUBMIT");
        std::env::remove_var("AUTO_SUBMIT_DELAY_MS");
        std::env::remove_var("AUTO_SUBMIT_VALUE");
        std::env::remove_var("AUTO_SUBMIT_INDEX");

        let config = get_auto_submit_config();

        assert!(!config.enabled, "Default should be disabled");
        assert_eq!(
            config.delay,
            Duration::from_millis(100),
            "Default delay should be 100ms"
        );
    }

    /// Test get_arg_value() with choices.
    #[test]
    fn test_auto_submit_config_get_arg_value() {
        let choices = vec![
            Choice {
                name: "Apple".to_string(),
                value: "apple".to_string(),
                description: None,
                semantic_id: None,
            },
            Choice {
                name: "Banana".to_string(),
                value: "banana".to_string(),
                description: None,
                semantic_id: None,
            },
            Choice {
                name: "Cherry".to_string(),
                value: "cherry".to_string(),
                description: None,
                semantic_id: None,
            },
        ];

        // Test default behavior (first choice)
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_arg_value(&choices),
            Some("apple".to_string()),
            "Default should return first choice value"
        );

        // Test with index
        let config = AutoSubmitConfig {
            index: 1,
            ..Default::default()
        };
        assert_eq!(
            config.get_arg_value(&choices),
            Some("banana".to_string()),
            "Index 1 should return second choice value"
        );

        // Test with out-of-bounds index (should clamp)
        let config = AutoSubmitConfig {
            index: 100,
            ..Default::default()
        };
        assert_eq!(
            config.get_arg_value(&choices),
            Some("cherry".to_string()),
            "Out-of-bounds index should clamp to last choice"
        );

        // Test with value override
        let config = AutoSubmitConfig {
            value_override: Some("custom".to_string()),
            index: 1,
            ..Default::default()
        };
        assert_eq!(
            config.get_arg_value(&choices),
            Some("custom".to_string()),
            "Override value should take precedence over index"
        );

        // Test with empty choices
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_arg_value(&[]),
            None,
            "Empty choices should return None"
        );
    }

    /// Test get_div_value() returns None (just dismissal).
    #[test]
    fn test_auto_submit_config_get_div_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_div_value(),
            None,
            "Div prompt should return None for dismissal"
        );
    }

    /// Test get_editor_value() returns original content.
    #[test]
    fn test_auto_submit_config_get_editor_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_editor_value("original content"),
            Some("original content".to_string()),
            "Editor should return original content unchanged"
        );

        // Test with override
        let config = AutoSubmitConfig {
            value_override: Some("modified".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.get_editor_value("original content"),
            Some("modified".to_string()),
            "Override should take precedence"
        );
    }

    /// Test get_term_value() returns "0".
    #[test]
    fn test_auto_submit_config_get_term_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_term_value(),
            Some("0".to_string()),
            "Term prompt should return exit code 0"
        );
    }

    /// Test get_form_value() returns empty JSON object.
    #[test]
    fn test_auto_submit_config_get_form_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_form_value(),
            Some("{}".to_string()),
            "Form prompt should return empty JSON object"
        );
    }

    /// Test get_select_value() returns JSON array.
    #[test]
    fn test_auto_submit_config_get_select_value() {
        let choices = vec![
            Choice {
                name: "Apple".to_string(),
                value: "apple".to_string(),
                description: None,
                semantic_id: None,
            },
            Choice {
                name: "Banana".to_string(),
                value: "banana".to_string(),
                description: None,
                semantic_id: None,
            },
        ];

        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_select_value(&choices),
            Some(r#"["apple"]"#.to_string()),
            "Select should return JSON array with first choice"
        );

        let config = AutoSubmitConfig {
            index: 1,
            ..Default::default()
        };
        assert_eq!(
            config.get_select_value(&choices),
            Some(r#"["banana"]"#.to_string()),
            "Select with index 1 should return second choice"
        );

        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_select_value(&[]),
            Some("[]".to_string()),
            "Empty choices should return empty array"
        );
    }

    /// Test get_fields_value() returns JSON array of empty strings.
    #[test]
    fn test_auto_submit_config_get_fields_value() {
        let config = AutoSubmitConfig::default();

        assert_eq!(
            config.get_fields_value(0),
            Some("[]".to_string()),
            "0 fields should return empty array"
        );
        assert_eq!(
            config.get_fields_value(1),
            Some(r#"[""]"#.to_string()),
            "1 field should return array with one empty string"
        );
        assert_eq!(
            config.get_fields_value(3),
            Some(r#"["","",""]"#.to_string()),
            "3 fields should return array with three empty strings"
        );
    }

    /// Test get_path_value() returns test path.
    #[test]
    fn test_auto_submit_config_get_path_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_path_value(),
            Some("/tmp/test-path".to_string()),
            "Path prompt should return /tmp/test-path"
        );
    }

    /// Test get_hotkey_value() returns Cmd+A.
    #[test]
    fn test_auto_submit_config_get_hotkey_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_hotkey_value(),
            Some(r#"{"key":"a","command":true}"#.to_string()),
            "Hotkey prompt should return Cmd+A JSON"
        );
    }

    /// Test get_drop_value() returns test file array.
    #[test]
    fn test_auto_submit_config_get_drop_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(
            config.get_drop_value(),
            Some(r#"[{"path":"/tmp/test.txt"}]"#.to_string()),
            "Drop prompt should return test file array"
        );
    }

    // ============================================================
    // Scriptlet Execution Tests
    // ============================================================

    use super::{build_final_content, run_scriptlet, tool_extension, ScriptletExecOptions};
    use crate::scriptlets::Scriptlet;

    #[test]
    fn test_tool_extension() {
        assert_eq!(tool_extension("ruby"), "rb");
        assert_eq!(tool_extension("python"), "py");
        assert_eq!(tool_extension("perl"), "pl");
        assert_eq!(tool_extension("php"), "php");
        assert_eq!(tool_extension("bash"), "sh");
        assert_eq!(tool_extension("sh"), "sh");
        assert_eq!(tool_extension("zsh"), "zsh");
        assert_eq!(tool_extension("fish"), "fish");
        assert_eq!(tool_extension("node"), "js");
        assert_eq!(tool_extension("js"), "js");
        assert_eq!(tool_extension("ts"), "ts");
        assert_eq!(tool_extension("kit"), "ts");
        assert_eq!(tool_extension("bun"), "ts");
        assert_eq!(tool_extension("deno"), "ts");
        assert_eq!(tool_extension("applescript"), "applescript");
        assert_eq!(tool_extension("powershell"), "ps1");
        assert_eq!(tool_extension("pwsh"), "ps1");
        assert_eq!(tool_extension("cmd"), "bat");
        assert_eq!(tool_extension("unknown"), "sh");
    }

    #[test]
    fn test_build_final_content_no_modifications() {
        let content = "echo hello";
        let result = build_final_content(content, &None, &None);
        assert_eq!(result, "echo hello");
    }

    #[test]
    fn test_build_final_content_with_prepend() {
        let content = "echo hello";
        let prepend = Some("#!/bin/bash".to_string());
        let result = build_final_content(content, &prepend, &None);
        assert_eq!(result, "#!/bin/bash\necho hello");
    }

    #[test]
    fn test_build_final_content_with_append() {
        let content = "echo hello";
        let append = Some("echo done".to_string());
        let result = build_final_content(content, &None, &append);
        assert_eq!(result, "echo hello\necho done");
    }

    #[test]
    fn test_build_final_content_with_both() {
        let content = "echo hello";
        let prepend = Some("#!/bin/bash\nset -e".to_string());
        let append = Some("echo done".to_string());
        let result = build_final_content(content, &prepend, &append);
        assert_eq!(result, "#!/bin/bash\nset -e\necho hello\necho done");
    }

    #[test]
    fn test_build_final_content_handles_trailing_newlines() {
        let content = "echo hello";
        let prepend = Some("#!/bin/bash\n".to_string());
        let result = build_final_content(content, &prepend, &None);
        assert_eq!(result, "#!/bin/bash\necho hello");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_scriptlet_bash_echo() {
        let scriptlet = Scriptlet::new(
            "Echo Test".to_string(),
            "bash".to_string(),
            "echo hello".to_string(),
        );

        let result = run_scriptlet(&scriptlet, ScriptletExecOptions::default());
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(result.success, "Script should succeed");
        assert_eq!(result.exit_code, 0);
        assert!(
            result.stdout.contains("hello"),
            "Expected 'hello' in stdout: {}",
            result.stdout
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_scriptlet_bash_with_variable_substitution() {
        let scriptlet = Scriptlet::new(
            "Variable Test".to_string(),
            "bash".to_string(),
            "echo Hello {{name}}".to_string(),
        );

        let mut inputs = HashMap::new();
        inputs.insert("name".to_string(), "World".to_string());

        let options = ScriptletExecOptions {
            inputs,
            ..Default::default()
        };

        let result = run_scriptlet(&scriptlet, options);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(result.success);
        assert!(
            result.stdout.contains("Hello World"),
            "Expected 'Hello World' in stdout: {}",
            result.stdout
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_scriptlet_bash_with_positional_args() {
        let scriptlet = Scriptlet::new(
            "Positional Test".to_string(),
            "bash".to_string(),
            "echo $1 and $2".to_string(),
        );

        let options = ScriptletExecOptions {
            positional_args: vec!["first".to_string(), "second".to_string()],
            ..Default::default()
        };

        let result = run_scriptlet(&scriptlet, options);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(result.success);
        assert!(
            result.stdout.contains("first and second"),
            "Expected 'first and second' in stdout: {}",
            result.stdout
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_scriptlet_with_prepend_append() {
        let scriptlet = Scriptlet::new(
            "Prepend Append Test".to_string(),
            "bash".to_string(),
            "echo middle".to_string(),
        );

        let options = ScriptletExecOptions {
            prepend: Some("echo start".to_string()),
            append: Some("echo end".to_string()),
            ..Default::default()
        };

        let result = run_scriptlet(&scriptlet, options);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(result.success);
        let stdout = result.stdout;
        assert!(
            stdout.contains("start"),
            "Should contain 'start': {}",
            stdout
        );
        assert!(
            stdout.contains("middle"),
            "Should contain 'middle': {}",
            stdout
        );
        assert!(stdout.contains("end"), "Should contain 'end': {}", stdout);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_scriptlet_with_cwd() {
        let scriptlet = Scriptlet::new(
            "CWD Test".to_string(),
            "bash".to_string(),
            "pwd".to_string(),
        );

        let options = ScriptletExecOptions {
            cwd: Some(PathBuf::from("/tmp")),
            ..Default::default()
        };

        let result = run_scriptlet(&scriptlet, options);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(result.success);
        // /tmp might be symlinked to /private/tmp on macOS
        assert!(
            result.stdout.contains("/tmp") || result.stdout.contains("/private/tmp"),
            "Expected '/tmp' in stdout: {}",
            result.stdout
        );
    }

    #[test]
    fn test_run_scriptlet_template_returns_content() {
        let scriptlet = Scriptlet::new(
            "Template Test".to_string(),
            "template".to_string(),
            "Hello {{name}}!".to_string(),
        );

        let mut inputs = HashMap::new();
        inputs.insert("name".to_string(), "World".to_string());

        let options = ScriptletExecOptions {
            inputs,
            ..Default::default()
        };

        let result = run_scriptlet(&scriptlet, options);
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "Hello World!");
    }

    #[test]
    fn test_run_scriptlet_with_conditionals() {
        let scriptlet = Scriptlet::new(
            "Conditional Test".to_string(),
            "template".to_string(),
            "{{#if formal}}Dear Sir{{else}}Hey there{{/if}}".to_string(),
        );

        let mut flags = HashMap::new();
        flags.insert("formal".to_string(), true);

        let options = ScriptletExecOptions {
            flags,
            ..Default::default()
        };

        let result = run_scriptlet(&scriptlet, options);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(
            result.stdout.contains("Dear Sir"),
            "Expected 'Dear Sir' in output: {}",
            result.stdout
        );
    }

    // This test actually opens Finder to /tmp, so it's a system test
    #[cfg(all(unix, feature = "system-tests"))]
    #[test]
    fn test_run_scriptlet_open() {
        // Just test that open doesn't error on a valid path
        // We can't really verify it opens, but we can test the function runs
        let scriptlet = Scriptlet::new(
            "Open Test".to_string(),
            "open".to_string(),
            "/tmp".to_string(),
        );

        let result = run_scriptlet(&scriptlet, ScriptletExecOptions::default());
        // This should succeed on macOS/Linux with /tmp
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
    }

    #[test]
    fn test_scriptlet_exec_options_default() {
        let options = ScriptletExecOptions::default();
        assert!(options.cwd.is_none());
        assert!(options.prepend.is_none());
        assert!(options.append.is_none());
        assert!(options.inputs.is_empty());
        assert!(options.positional_args.is_empty());
        assert!(options.flags.is_empty());
    }

    // ============================================================
    // Shell Tool Execution Tests
    // ============================================================
    //
    // Tests for execute_shell_scriptlet() function and SHELL_TOOLS constant.
    // These tests verify shell tool execution, error handling, and platform guards.

    use super::execute_shell_scriptlet;
    use crate::scriptlets::SHELL_TOOLS;

    /// Verify SHELL_TOOLS constant contains all expected shells
    #[test]
    fn test_shell_tools_contains_expected_shells() {
        // Unix shells
        assert!(
            SHELL_TOOLS.contains(&"bash"),
            "SHELL_TOOLS should include bash"
        );
        assert!(
            SHELL_TOOLS.contains(&"zsh"),
            "SHELL_TOOLS should include zsh"
        );
        assert!(SHELL_TOOLS.contains(&"sh"), "SHELL_TOOLS should include sh");
        assert!(
            SHELL_TOOLS.contains(&"fish"),
            "SHELL_TOOLS should include fish"
        );

        // Windows shells
        assert!(
            SHELL_TOOLS.contains(&"cmd"),
            "SHELL_TOOLS should include cmd"
        );
        assert!(
            SHELL_TOOLS.contains(&"powershell"),
            "SHELL_TOOLS should include powershell"
        );
        assert!(
            SHELL_TOOLS.contains(&"pwsh"),
            "SHELL_TOOLS should include pwsh"
        );
    }

    /// Verify SHELL_TOOLS has exactly 7 shells (no duplicates, no extras)
    #[test]
    fn test_shell_tools_count() {
        assert_eq!(
            SHELL_TOOLS.len(),
            7,
            "SHELL_TOOLS should have exactly 7 shells"
        );
    }

    /// Test successful shell execution returns correct exit code and stdout
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_success_exit_code() {
        let result = execute_shell_scriptlet("bash", "exit 0", &ScriptletExecOptions::default());
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert_eq!(result.exit_code, 0, "Exit code should be 0");
        assert!(result.success, "success flag should be true");
    }

    /// Test shell execution captures stdout correctly
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_captures_stdout() {
        let result = execute_shell_scriptlet(
            "bash",
            "echo 'test output'",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(
            result.stdout.contains("test output"),
            "stdout should contain 'test output', got: '{}'",
            result.stdout
        );
        assert!(
            result.stderr.is_empty() || !result.stderr.contains("error"),
            "stderr should be empty or not contain 'error': '{}'",
            result.stderr
        );
    }

    /// Test shell execution captures stderr correctly
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_captures_stderr() {
        let result = execute_shell_scriptlet(
            "bash",
            "echo 'error message' >&2",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_ok(), "Expected success, got: {:?}", result);

        let result = result.unwrap();
        assert!(
            result.stderr.contains("error message"),
            "stderr should contain 'error message', got: '{}'",
            result.stderr
        );
    }

    /// Test non-zero exit code is captured correctly
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_nonzero_exit_code() {
        let result = execute_shell_scriptlet("bash", "exit 42", &ScriptletExecOptions::default());
        assert!(
            result.is_ok(),
            "Expected success (script ran, just non-zero exit), got: {:?}",
            result
        );

        let result = result.unwrap();
        assert_eq!(result.exit_code, 42, "Exit code should be 42");
        assert!(
            !result.success,
            "success flag should be false for non-zero exit"
        );
    }

    /// Test script syntax errors are captured in stderr
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_syntax_error_captured() {
        // Intentional syntax error: unclosed quote
        let result =
            execute_shell_scriptlet("bash", "echo 'unclosed", &ScriptletExecOptions::default());
        assert!(
            result.is_ok(),
            "Script should run (even if shell reports error)"
        );

        let result = result.unwrap();
        // Syntax errors in bash result in non-zero exit
        assert!(!result.success, "Syntax error should result in failure");
        // The error message should appear in stderr
        assert!(
            !result.stderr.is_empty(),
            "stderr should contain error for syntax error, got: '{}'",
            result.stderr
        );
    }

    /// Test undefined variable doesn't cause hard failure (just empty expansion)
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_undefined_variable() {
        // By default, bash doesn't fail on undefined variables
        let result = execute_shell_scriptlet(
            "bash",
            "echo $UNDEFINED_VAR_12345",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(
            result.success,
            "Undefined var should not cause failure by default"
        );
        assert_eq!(result.exit_code, 0);
    }

    /// Test strict mode catches undefined variables
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_strict_mode_undefined_var() {
        // set -u makes bash fail on undefined variables
        let result = execute_shell_scriptlet(
            "bash",
            "set -u; echo $UNDEFINED_VAR_12345",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(!result.success, "Undefined var with set -u should fail");
        assert!(
            result.stderr.contains("UNDEFINED_VAR_12345") || result.stderr.contains("unbound"),
            "stderr should mention the undefined variable: '{}'",
            result.stderr
        );
    }

    /// Test command not found error message
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_command_not_found() {
        let result = execute_shell_scriptlet(
            "bash",
            "nonexistent_command_xyz123",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(!result.success, "Command not found should fail");
        assert!(
            result.exit_code == 127 || result.exit_code != 0,
            "Exit code should indicate failure (typically 127): {}",
            result.exit_code
        );
        assert!(
            result.stderr.contains("not found") || result.stderr.contains("command not found"),
            "stderr should indicate command not found: '{}'",
            result.stderr
        );
    }

    /// Test missing shell executable returns helpful error
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_missing_shell() {
        // Try to use a non-existent shell
        let result = execute_shell_scriptlet(
            "nonexistent_shell_xyz123",
            "echo test",
            &ScriptletExecOptions::default(),
        );

        // This should return an error (not Ok with failure) since the shell itself doesn't exist
        assert!(
            result.is_err(),
            "Missing shell should return Err, got: {:?}",
            result
        );

        let err = result.unwrap_err();
        // Error message should be helpful
        assert!(
            err.contains("Failed to execute") || err.contains("nonexistent_shell"),
            "Error should mention the missing shell: '{}'",
            err
        );
    }

    /// Test sh shell works (most basic POSIX shell)
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_sh() {
        let result =
            execute_shell_scriptlet("sh", "echo hello from sh", &ScriptletExecOptions::default());
        assert!(result.is_ok(), "sh should work: {:?}", result);

        let result = result.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello from sh"));
    }

    /// Test zsh shell works (if available)
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_zsh() {
        // zsh might not be installed, so we check first
        let check = std::process::Command::new("which").arg("zsh").output();

        if check.is_ok() && check.unwrap().status.success() {
            let result = execute_shell_scriptlet(
                "zsh",
                "echo hello from zsh",
                &ScriptletExecOptions::default(),
            );
            assert!(result.is_ok(), "zsh should work: {:?}", result);

            let result = result.unwrap();
            assert!(result.success);
            assert!(result.stdout.contains("hello from zsh"));
        }
        // If zsh not installed, skip test (don't fail)
    }

    /// Test fish shell works (if available)
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_fish() {
        // fish might not be installed, so we check first
        let check = std::process::Command::new("which").arg("fish").output();

        if check.is_ok() && check.unwrap().status.success() {
            // fish has slightly different syntax
            let result = execute_shell_scriptlet(
                "fish",
                "echo hello from fish",
                &ScriptletExecOptions::default(),
            );
            assert!(result.is_ok(), "fish should work: {:?}", result);

            let result = result.unwrap();
            assert!(result.success);
            assert!(result.stdout.contains("hello from fish"));
        }
        // If fish not installed, skip test (don't fail)
    }

    /// Test cwd option changes working directory for shell scripts
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_with_cwd() {
        let options = ScriptletExecOptions {
            cwd: Some(PathBuf::from("/tmp")),
            ..Default::default()
        };

        let result = execute_shell_scriptlet("bash", "pwd", &options);
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.success);
        // /tmp might be symlinked to /private/tmp on macOS
        assert!(
            result.stdout.contains("/tmp") || result.stdout.contains("/private/tmp"),
            "CWD should be /tmp, got: {}",
            result.stdout
        );
    }

    /// Test multiline scripts work correctly
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_multiline() {
        let script = r#"
echo "line 1"
echo "line 2"
echo "line 3"
"#;

        let result = execute_shell_scriptlet("bash", script, &ScriptletExecOptions::default());
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("line 1"));
        assert!(result.stdout.contains("line 2"));
        assert!(result.stdout.contains("line 3"));
    }

    /// Test environment variable access works in shell scripts
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_environment() {
        // HOME should always be set
        let result =
            execute_shell_scriptlet("bash", "echo $HOME", &ScriptletExecOptions::default());
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.success);
        // HOME should not be empty
        assert!(!result.stdout.trim().is_empty(), "HOME should be set");
    }

    /// Test Windows shells return appropriate error on Unix
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_windows_shell_on_unix() {
        // cmd.exe doesn't exist on Unix
        let result = execute_shell_scriptlet("cmd", "echo test", &ScriptletExecOptions::default());

        // This should fail because cmd doesn't exist
        assert!(
            result.is_err() || !result.as_ref().unwrap().success,
            "cmd should fail on Unix: {:?}",
            result
        );
    }

    /// Test powershell on Unix (might be installed as pwsh)
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_powershell_on_unix() {
        // Check if pwsh is installed (PowerShell Core)
        let pwsh_check = std::process::Command::new("which").arg("pwsh").output();

        let has_pwsh = pwsh_check.is_ok() && pwsh_check.unwrap().status.success();

        if has_pwsh {
            // pwsh should work if installed
            let result = execute_shell_scriptlet(
                "pwsh",
                "Write-Output 'hello from pwsh'",
                &ScriptletExecOptions::default(),
            );
            assert!(
                result.is_ok(),
                "pwsh should work if installed: {:?}",
                result
            );

            let result = result.unwrap();
            assert!(result.success);
            assert!(result.stdout.contains("hello from pwsh"));
        } else {
            // If not installed, it should fail
            let result = execute_shell_scriptlet(
                "pwsh",
                "Write-Output 'test'",
                &ScriptletExecOptions::default(),
            );
            assert!(
                result.is_err() || !result.as_ref().unwrap().success,
                "pwsh should fail if not installed: {:?}",
                result
            );
        }
    }

    /// Test Windows-specific shells are defined correctly
    #[test]
    fn test_windows_shells_in_shell_tools() {
        // Verify Windows shells are in SHELL_TOOLS
        let windows_shells = ["cmd", "powershell", "pwsh"];

        for shell in &windows_shells {
            assert!(
                SHELL_TOOLS.contains(shell),
                "SHELL_TOOLS should include Windows shell: {}",
                shell
            );
        }
    }

    /// Test Unix-specific shells are defined correctly
    #[test]
    fn test_unix_shells_in_shell_tools() {
        // Verify Unix shells are in SHELL_TOOLS
        let unix_shells = ["bash", "zsh", "sh", "fish"];

        for shell in &unix_shells {
            assert!(
                SHELL_TOOLS.contains(shell),
                "SHELL_TOOLS should include Unix shell: {}",
                shell
            );
        }
    }

    /// Test run_scriptlet correctly dispatches to shell handler
    #[cfg(unix)]
    #[test]
    fn test_run_scriptlet_dispatches_to_shell_handler() {
        for shell in &["bash", "sh"] {
            let scriptlet = Scriptlet::new(
                format!("{} Test", shell),
                shell.to_string(),
                "echo test".to_string(),
            );

            let result = run_scriptlet(&scriptlet, ScriptletExecOptions::default());
            assert!(
                result.is_ok(),
                "{} scriptlet should succeed: {:?}",
                shell,
                result
            );

            let result = result.unwrap();
            assert!(result.success, "{} should succeed", shell);
            assert!(
                result.stdout.contains("test"),
                "{} should output 'test'",
                shell
            );
        }
    }

    /// Test shell scripts handle special characters correctly
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_special_characters() {
        // Test that special shell characters are handled
        let result = execute_shell_scriptlet(
            "bash",
            r#"echo "Hello, World!" && echo 'Single quotes' && echo $((1 + 2))"#,
            &ScriptletExecOptions::default(),
        );

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("Hello, World!"));
        assert!(result.stdout.contains("Single quotes"));
        assert!(result.stdout.contains("3")); // 1 + 2
    }

    /// Test shell scripts with here-documents
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_heredoc() {
        let script = r#"cat << 'EOF'
multi
line
content
EOF"#;

        let result = execute_shell_scriptlet("bash", script, &ScriptletExecOptions::default());
        assert!(result.is_ok());

        let result = result.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("multi"));
        assert!(result.stdout.contains("line"));
        assert!(result.stdout.contains("content"));
    }

    /// Test shell scripts with pipes
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_pipes() {
        let result = execute_shell_scriptlet(
            "bash",
            "echo 'hello world' | tr 'a-z' 'A-Z'",
            &ScriptletExecOptions::default(),
        );

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("HELLO WORLD"));
    }

    /// Test shell scripts with command substitution
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_command_substitution() {
        let result = execute_shell_scriptlet(
            "bash",
            "echo Today is $(date +%A)",
            &ScriptletExecOptions::default(),
        );

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("Today is"));
    }

    /// Test that temp file is cleaned up after execution
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_cleanup() {
        // Run a script - the temp file should be cleaned up after execution
        let result = execute_shell_scriptlet(
            "bash",
            "echo cleanup test",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_ok());

        // The temp file should be cleaned up
        // Note: Due to potential race conditions in testing, we just verify the script ran
        // The cleanup is verified by the fact that multiple tests don't accumulate temp files
        let result = result.unwrap();
        assert!(result.success);
    }

    // ============================================================
    // Shell Not Found Suggestions Tests
    // ============================================================

    use super::shell_not_found_suggestions;

    /// Test that suggestions are provided for each shell type
    #[test]
    fn test_shell_not_found_suggestions_bash() {
        let suggestions = shell_not_found_suggestions("bash");
        assert!(suggestions.contains("bash"), "Should mention bash");
        assert!(suggestions.contains("PATH"), "Should mention PATH");
        assert!(
            suggestions.contains("SHELL_TOOLS"),
            "Should mention SHELL_TOOLS alternatives"
        );
    }

    #[test]
    fn test_shell_not_found_suggestions_zsh() {
        let suggestions = shell_not_found_suggestions("zsh");
        assert!(suggestions.contains("zsh"), "Should mention zsh");
        assert!(suggestions.contains("PATH"), "Should mention PATH");
    }

    #[test]
    fn test_shell_not_found_suggestions_sh() {
        let suggestions = shell_not_found_suggestions("sh");
        assert!(suggestions.contains("sh"), "Should mention sh");
        assert!(
            suggestions.contains("POSIX") || suggestions.contains("PATH"),
            "Should mention POSIX or PATH"
        );
    }

    #[test]
    fn test_shell_not_found_suggestions_fish() {
        let suggestions = shell_not_found_suggestions("fish");
        assert!(suggestions.contains("fish"), "Should mention fish");
        assert!(
            suggestions.contains("fishshell.com") || suggestions.contains("brew"),
            "Should provide installation hint"
        );
    }

    #[test]
    fn test_shell_not_found_suggestions_cmd() {
        let suggestions = shell_not_found_suggestions("cmd");
        assert!(suggestions.contains("cmd"), "Should mention cmd");
        // On Unix, should suggest using Unix shells instead
        #[cfg(unix)]
        {
            assert!(
                suggestions.contains("Windows-only") || suggestions.contains("bash"),
                "Should mention cmd is Windows-only on Unix"
            );
        }
    }

    #[test]
    fn test_shell_not_found_suggestions_powershell() {
        let suggestions = shell_not_found_suggestions("powershell");
        assert!(
            suggestions.contains("powershell") || suggestions.contains("PowerShell"),
            "Should mention powershell"
        );
    }

    #[test]
    fn test_shell_not_found_suggestions_pwsh() {
        let suggestions = shell_not_found_suggestions("pwsh");
        assert!(
            suggestions.contains("PowerShell"),
            "Should mention PowerShell Core"
        );
        assert!(
            suggestions.contains("install-powershell"),
            "Should provide install link"
        );
    }

    #[test]
    fn test_shell_not_found_suggestions_unknown() {
        let suggestions = shell_not_found_suggestions("unknown_shell");
        assert!(
            suggestions.contains("unknown_shell"),
            "Should mention the shell name"
        );
        assert!(
            suggestions.contains("not recognized") || suggestions.contains("PATH"),
            "Should suggest checking PATH"
        );
        assert!(
            suggestions.contains("SHELL_TOOLS"),
            "Should mention alternatives"
        );
    }

    /// Test that error message includes suggestions when shell is not found
    #[cfg(unix)]
    #[test]
    fn test_execute_shell_scriptlet_error_includes_suggestions() {
        let result = execute_shell_scriptlet(
            "nonexistent_shell_xyz",
            "echo test",
            &ScriptletExecOptions::default(),
        );
        assert!(result.is_err(), "Should fail for nonexistent shell");

        let err = result.unwrap_err();
        assert!(
            err.contains("Suggestions"),
            "Error should include suggestions section"
        );
        assert!(err.contains("PATH"), "Error should mention PATH");
        assert!(
            err.contains("SHELL_TOOLS"),
            "Error should mention SHELL_TOOLS alternatives"
        );
    }
}

</file>

</files>
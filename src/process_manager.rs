//! Process Manager Module
//!
//! Centralized process tracking for bun script processes.
#![allow(dead_code)] // Some methods reserved for future use
//!
//! This module provides:
//! - PID file at ~/.kenv/script-kit.pid for main app
//! - Active child PIDs file at ~/.kenv/active-bun-pids.json
//! - Thread-safe process registration/unregistration
//! - Orphan detection on startup
//! - Bulk kill for graceful shutdown
//!

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

        // Should use ~/.kenv/ paths
        let home = dirs::home_dir().unwrap();
        assert_eq!(manager.main_pid_path, home.join(".kenv/script-kit.pid"));
        assert_eq!(
            manager.active_pids_path,
            home.join(".kenv/active-bun-pids.json")
        );
    }
}

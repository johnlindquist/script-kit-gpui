//! Process Manager Module
//!
//! Centralized process tracking for bun script processes.
//!
//! This module provides:
//! - PID file at ~/.scriptkit/script-kit.pid for main app
//! - Active child PIDs file at ~/.scriptkit/active-bun-pids.json
//! - Thread-safe process registration/unregistration
//! - Orphan detection on startup
//! - Bulk kill for graceful shutdown
//!

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};
use sysinfo::{Pid, System};
use tracing::{debug, info, warn};
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
#[derive(Debug, Clone)]
pub struct ProcessManager {
    /// Map of PID -> ProcessInfo for active child processes
    active_processes: Arc<RwLock<HashMap<u32, ProcessInfo>>>,
    /// Path to main app PID file
    main_pid_path: PathBuf,
    /// Path to active child PIDs JSON file
    active_pids_path: PathBuf,
}

/// A failed liveness probe is never evidence that owned work stopped.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnedProcessGroupLiveness {
    Alive,
    Exited,
    Unverified,
}

#[cfg(unix)]
fn owned_process_signal_is_authorized(pid: u32, tracked: bool) -> bool {
    tracked && pid > 0 && pid <= i32::MAX as u32
}

#[cfg(unix)]
pub(crate) fn classify_owned_process_group_probe(
    result: i32,
    errno: Option<i32>,
) -> OwnedProcessGroupLiveness {
    if result == 0 || errno == Some(libc::EPERM) {
        OwnedProcessGroupLiveness::Alive
    } else if errno == Some(libc::ESRCH) {
        OwnedProcessGroupLiveness::Exited
    } else {
        OwnedProcessGroupLiveness::Unverified
    }
}

#[cfg(unix)]
pub(crate) fn observe_owned_process_group(pid: u32) -> OwnedProcessGroupLiveness {
    if pid == 0 || pid > i32::MAX as u32 {
        return OwnedProcessGroupLiveness::Unverified;
    }
    // SAFETY: signal zero only observes this exact positive process group;
    // it never sends a signal or touches the caller's own implicit group.
    let result = unsafe { libc::kill(-(pid as libc::pid_t), 0) };
    let errno = if result == 0 {
        None
    } else {
        std::io::Error::last_os_error().raw_os_error()
    };
    classify_owned_process_group_probe(result, errno)
}

#[cfg(unix)]
fn observe_tracked_process(pid: u32) -> OwnedProcessGroupLiveness {
    let group = observe_owned_process_group(pid);
    if group != OwnedProcessGroupLiveness::Exited {
        return group;
    }
    // Some legacy children never created a dedicated group. A missing group
    // is not proof that the tracked individual process has also exited.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    let errno = if result == 0 {
        None
    } else {
        std::io::Error::last_os_error().raw_os_error()
    };
    classify_owned_process_group_probe(result, errno)
}

impl ProcessManager {
    const DIR_PERMISSIONS: u32 = 0o700;
    const FILE_PERMISSIONS: u32 = 0o600;

    /// Create a new ProcessManager with default paths
    pub fn new() -> Self {
        let kit_dir = dirs::home_dir()
            .map(|h| h.join(".scriptkit"))
            .unwrap_or_else(|| {
                // Use system temp directory instead of hardcoded /tmp for better security
                std::env::temp_dir().join(".scriptkit")
            });

        Self {
            active_processes: Arc::new(RwLock::new(HashMap::new())),
            main_pid_path: kit_dir.join("script-kit.pid"),
            // Per-instance file: parallel dev instances must not clobber each
            // other's registrations (a dying instance's cleanup would delete a
            // shared file AFTER its successor wrote to it). The startup sweep
            // scans every active-pids-*.json whose owner pid is dead.
            active_pids_path: kit_dir.join(format!("active-pids-{}.json", std::process::id())),
        }
    }

    /// Parse the owning instance pid out of an `active-pids-<pid>.json`
    /// filename. The legacy shared `active-bun-pids.json` has no owner.
    fn registry_file_owner(file_name: &str) -> Option<u32> {
        file_name
            .strip_prefix("active-pids-")?
            .strip_suffix(".json")?
            .parse()
            .ok()
    }

    /// True when `file_name` is a child-pid registry file the startup sweep
    /// should consider (per-instance or legacy shared).
    fn is_registry_file(file_name: &str) -> bool {
        file_name == "active-bun-pids.json" || Self::registry_file_owner(file_name).is_some()
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
        info!(
            pid,
            path = ?self.main_pid_path,
            "process_manager.write_main_pid.start"
        );

        // Ensure parent directory exists
        if let Some(parent) = self.main_pid_path.parent() {
            fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(Self::DIR_PERMISSIONS);
                fs::set_permissions(parent, perms)?;
            }
        }

        let mut file = File::create(&self.main_pid_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(Self::FILE_PERMISSIONS);
            file.set_permissions(perms)?;
        }
        write!(file, "{}", pid)?;

        info!(pid, "process_manager.write_main_pid.success");
        Ok(())
    }

    /// Remove the main PID file
    ///
    /// This should be called on clean shutdown.
    pub fn remove_main_pid(&self) {
        if self.main_pid_path.exists() {
            if let Err(e) = fs::remove_file(&self.main_pid_path) {
                warn!(
                    error = %e,
                    path = ?self.main_pid_path,
                    "process_manager.remove_main_pid.failed"
                );
            } else {
                info!(path = ?self.main_pid_path, "process_manager.remove_main_pid.success");
            }
        }
    }

    /// Read the main PID from disk, if it exists
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

        info!(pid, script_path, "process_manager.register_process.start");

        // Add to in-memory map
        if let Ok(mut processes) = self.active_processes.write() {
            processes.insert(pid, info);
        }

        // Persist to disk
        if let Err(e) = self.persist_active_pids() {
            warn!(
                error = %e,
                path = ?self.active_pids_path,
                pid,
                "process_manager.register_process.persist_failed"
            );
        }
    }

    /// Unregister a child process
    ///
    /// This removes the process from tracking when it exits normally.
    pub fn unregister_process(&self, pid: u32) {
        info!(pid, "process_manager.unregister_process.start");

        // Remove from in-memory map
        if let Ok(mut processes) = self.active_processes.write() {
            processes.remove(&pid);
        }

        // Persist to disk
        if let Err(e) = self.persist_active_pids() {
            warn!(
                error = %e,
                path = ?self.active_pids_path,
                pid,
                "process_manager.unregister_process.persist_failed"
            );
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

    /// Get all active processes sorted by start time (newest first).
    pub fn get_active_processes_sorted(&self) -> Vec<ProcessInfo> {
        let mut processes = self.get_active_processes();
        processes.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        processes
    }

    /// Build a human-readable process report that can be shown in the UI/clipboard.
    #[allow(dead_code)]
    pub fn format_active_process_report(&self, max_entries: usize) -> String {
        let processes = self.get_active_processes_sorted();
        if processes.is_empty() {
            return "No active Script Kit processes.".to_string();
        }

        let total = processes.len();
        let limit = max_entries.max(1);
        let mut lines = Vec::with_capacity(limit + 2);
        lines.push(format!("Active Script Kit processes: {}", total));

        for process in processes.iter().take(limit) {
            lines.push(format!(
                "PID {} • {} • started {}",
                process.pid,
                process.script_path,
                crate::formatting::format_absolute_datetime(process.started_at)
            ));
        }

        if total > limit {
            lines.push(format!("... and {} more", total - limit));
        }

        lines.join("\n")
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
            debug!("process_manager.kill_all_processes.no_active_processes");
            return;
        }

        info!(
            process_count = processes.len(),
            "process_manager.kill_all_processes.start"
        );

        // Verify each entry still IS the process we registered before
        // signalling: a tracked pid can go stale between child exit and
        // unregistration, and the single-pid kill fallback must never hit a
        // recycled pid.
        let mut system = System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing().with_cmd(sysinfo::UpdateKind::Always),
        );
        for info in &processes {
            let Some(process) = system.process(Pid::from_u32(info.pid)) else {
                #[cfg(unix)]
                if self.reconcile_owned_process_cleanup(info.pid, observe_tracked_process(info.pid))
                {
                    debug!(pid = info.pid, "process_manager.kill_all.already_exited");
                } else {
                    warn!(
                        pid = info.pid,
                        "process_manager.kill_all.descendants_still_owned"
                    );
                }
                #[cfg(not(unix))]
                self.unregister_process(info.pid);
                continue;
            };
            if !cmdline_matches_recorded(process.cmd(), &info.script_path) {
                warn!(
                    pid = info.pid,
                    script_path = info.script_path.as_str(),
                    "process_manager.kill_all.pid_recycled_skip"
                );
                // This PID now belongs to somebody else, not our recorded
                // child. Forget the stale claim without signalling it.
                self.unregister_process(info.pid);
                continue;
            }
            self.kill_process(info.pid);
            #[cfg(unix)]
            if !self.reconcile_owned_process_cleanup(info.pid, observe_tracked_process(info.pid)) {
                warn!(
                    pid = info.pid,
                    "process_manager.kill_all.cleanup_unverified_retained"
                );
            }
            #[cfg(not(unix))]
            self.unregister_process(info.pid);
        }

        let remaining = self.active_count();
        if remaining > 0 {
            warn!(
                remaining,
                "process_manager.kill_all_processes.retaining_unverified_owned_processes"
            );
            return;
        }

        // Remove the active PIDs file
        if self.active_pids_path.exists() {
            if let Err(e) = fs::remove_file(&self.active_pids_path) {
                warn!(
                    error = %e,
                    path = ?self.active_pids_path,
                    "process_manager.kill_all_processes.remove_active_pids_failed"
                );
            }
        }

        info!("process_manager.kill_all_processes.success");
    }

    /// Kill a single process by PID
    ///
    /// Sends SIGKILL to the process group on Unix. Children spawned without
    /// `process_group(0)` share the app's group, so when no group with id ==
    /// pid exists, fall back to signalling the bare pid.
    pub fn kill_process(&self, pid: u32) {
        debug!(pid, "process_manager.kill_process.start");

        #[cfg(unix)]
        {
            if !owned_process_signal_is_authorized(pid, self.is_tracked_process(pid)) {
                warn!(
                    pid,
                    "process_manager.kill_process.refused_invalid_or_unowned_group"
                );
                return;
            }
            let pgid = -(pid as i32);
            // SAFETY: libc::kill with a negative PID targets the process group.
            // The PID is validated as a tracked process before calling.
            let ret = unsafe { libc::kill(pgid, libc::SIGKILL) };

            if ret == 0 {
                info!(pid, "killed process group");
            } else if ret == -1 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ESRCH) {
                    // SAFETY: same contract as above, targeting the single pid.
                    let single = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
                    if single == 0 {
                        info!(pid, "killed process (no dedicated group)");
                    } else {
                        debug!(pid, "process already exited");
                    }
                } else {
                    warn!(pid, %err, "failed to kill process group");
                }
            }
        }

        #[cfg(not(unix))]
        {
            warn!(pid, "process_manager.kill_process.unsupported_platform");
        }
    }

    /// Request graceful termination without dropping ownership prematurely.
    ///
    /// `Ok(())` acknowledges the request; the process stays tracked until a
    /// background watcher independently observes the exact group as exited.
    /// Failed signals or uncertain liveness retain ownership for retry.
    /// Timeout in seconds before escalating from SIGTERM to SIGKILL.
    const SIGKILL_TIMEOUT_SECS: u64 = 5;

    pub fn terminate_process(&self, pid: u32) -> Result<(), String> {
        info!(pid, "process_manager.terminate_process.start");

        if pid == 0 || pid > i32::MAX as u32 {
            return Err("Refusing to terminate an invalid process-group identity".to_string());
        }

        // Check if process is still tracked
        let is_tracked = if let Ok(procs) = self.active_processes.read() {
            procs.contains_key(&pid)
        } else {
            return Err("Failed to read process list".to_string());
        };

        if !is_tracked {
            return Err(format!("Process {} is not tracked", pid));
        }

        #[cfg(unix)]
        {
            let pgid = -(pid as i32);
            // SAFETY: libc::kill with a negative PID targets the process group.
            // The PID was validated as tracked above.
            let ret = unsafe { libc::kill(pgid, libc::SIGTERM) };

            if ret == 0 {
                info!(pid, "process_manager.terminate_process.sigterm_sent");
                if self.reconcile_owned_process_cleanup(pid, observe_owned_process_group(pid)) {
                    return Ok(());
                }

                // Cloning shares the same exact process registry and persistence
                // paths; unlike a detached PID-only watcher, it cannot lose the
                // ownership record while graceful cancellation is pending.
                let manager = self.clone();
                std::thread::Builder::new()
                    .name(format!("owned-process-cleanup-{pid}"))
                    .spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(
                            ProcessManager::SIGKILL_TIMEOUT_SECS,
                        ));

                        if !manager.is_tracked_process(pid) {
                            return;
                        }
                        match observe_owned_process_group(pid) {
                            OwnedProcessGroupLiveness::Exited => {
                                manager.reconcile_owned_process_cleanup(
                                    pid,
                                    OwnedProcessGroupLiveness::Exited,
                                );
                                return;
                            }
                            OwnedProcessGroupLiveness::Unverified => {
                                warn!(pid, "process_manager.terminate_process.cleanup_unverified");
                                return;
                            }
                            OwnedProcessGroupLiveness::Alive => {}
                        }

                        warn!(
                            pid,
                            timeout_secs = ProcessManager::SIGKILL_TIMEOUT_SECS,
                            "process_manager.terminate_process.sigkill_escalation"
                        );
                        // SAFETY: `pid` is still tracked, positive, and owns
                        // this exact dedicated process group.
                        let result = unsafe { libc::kill(pgid, libc::SIGKILL) };
                        if result != 0 {
                            warn!(
                                pid,
                                error = %std::io::Error::last_os_error(),
                                "process_manager.terminate_process.sigkill_failed_retaining_owner"
                            );
                        }
                        for _ in 0..10 {
                            let state = observe_owned_process_group(pid);
                            if manager.reconcile_owned_process_cleanup(pid, state) {
                                return;
                            }
                            if state == OwnedProcessGroupLiveness::Unverified {
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(25));
                        }
                        warn!(pid, "process_manager.terminate_process.group_still_owned");
                    })
                    .map(|_| ())
                    .map_err(|error| {
                        format!("Failed to monitor owned process-group cleanup: {error}")
                    })
            } else {
                let err = std::io::Error::last_os_error();
                if err.kind() == ErrorKind::NotFound {
                    if self.reconcile_owned_process_cleanup(pid, observe_owned_process_group(pid)) {
                        info!(pid, "process_manager.terminate_process.already_exited");
                        Ok(())
                    } else {
                        Err(format!(
                            "Process group {pid} could not be proven stopped; ownership retained"
                        ))
                    }
                } else {
                    warn!(pid, %err, "process_manager.terminate_process.failed");
                    Err(format!("Failed to terminate PID {}: {}", pid, err))
                }
            }
        }

        #[cfg(not(unix))]
        {
            let _ = pid;
            Err("Process termination not supported on this platform".to_string())
        }
    }

    #[cfg(unix)]
    fn is_tracked_process(&self, pid: u32) -> bool {
        self.active_processes
            .read()
            .map(|processes| processes.contains_key(&pid))
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn reconcile_owned_process_cleanup(
        &self,
        pid: u32,
        observation: OwnedProcessGroupLiveness,
    ) -> bool {
        if observation != OwnedProcessGroupLiveness::Exited {
            return false;
        }
        if self.is_tracked_process(pid) {
            self.unregister_process(pid);
        }
        true
    }

    /// Check if a process is currently running
    pub fn is_process_running(&self, pid: u32) -> bool {
        let mut system = System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system.process(Pid::from_u32(pid)).is_some()
    }

    /// Detect and clean up orphaned processes from a previous crash
    ///
    /// This should be called at startup. It scans every registry file in the
    /// tracking directory (per-instance `active-pids-<pid>.json` plus the
    /// legacy shared file), skips registries whose owning instance is still
    /// alive, kills the orphans recorded in dead-owner registries, and removes
    /// those files.
    ///
    /// A recorded pid may have been recycled by the OS for an unrelated
    /// process since the crash, so a candidate is only killed when its live
    /// command line still matches what was recorded at registration time AND
    /// its parent is gone (true orphans are reparented to launchd/init).
    ///
    /// Returns the number of orphans killed.
    pub fn cleanup_orphans(&self) -> usize {
        debug!("process_manager.cleanup_orphans.start");

        let Some(dir) = self.active_pids_path.parent() else {
            return 0;
        };
        let mut registry_files: Vec<(PathBuf, Option<u32>)> = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if Self::is_registry_file(&name) {
                    registry_files.push((entry.path(), Self::registry_file_owner(&name)));
                }
            }
        }
        if registry_files.is_empty() {
            debug!("process_manager.cleanup_orphans.none_found");
            return 0;
        }

        let mut killed_count = 0;
        let self_pid = std::process::id();
        let mut system = System::new();
        // The default process refresh does not load command lines at all,
        // which would make the identity check below reject every candidate;
        // request cmd explicitly.
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing().with_cmd(sysinfo::UpdateKind::Always),
        );

        for (path, owner) in registry_files {
            if let Some(owner) = owner {
                if owner == self_pid {
                    continue;
                }
                if system.process(Pid::from_u32(owner)).is_some() {
                    debug!(
                        owner,
                        registry = ?path,
                        "process_manager.cleanup_orphans.live_owner_skip"
                    );
                    continue;
                }
            }

            let orphans = load_pids_from(&path);
            if !orphans.is_empty() {
                info!(
                    orphan_count = orphans.len(),
                    registry = ?path,
                    "process_manager.cleanup_orphans.found_candidates"
                );
            }

            for info in &orphans {
                let Some(process) = system.process(Pid::from_u32(info.pid)) else {
                    debug!(
                        pid = info.pid,
                        "process_manager.cleanup_orphans.orphan_already_exited"
                    );
                    continue;
                };
                if !cmdline_matches_recorded(process.cmd(), &info.script_path) {
                    warn!(
                        pid = info.pid,
                        script_path = info.script_path.as_str(),
                        "process_manager.cleanup_orphans.pid_recycled_skip"
                    );
                    continue;
                }
                // A true orphan was reparented to launchd/init when its
                // spawning instance died. A candidate whose parent is still
                // alive belongs to a live sibling instance (legacy shared
                // registries carry no owner) and must not be killed.
                let parent_alive = process
                    .parent()
                    .is_some_and(|parent| parent.as_u32() > 1 && system.process(parent).is_some());
                if parent_alive {
                    info!(
                        pid = info.pid,
                        script_path = info.script_path.as_str(),
                        "process_manager.cleanup_orphans.parent_alive_skip"
                    );
                    continue;
                }
                info!(
                    pid = info.pid,
                    script_path = info.script_path.as_str(),
                    "process_manager.cleanup_orphans.kill_orphan"
                );
                // Adopt only after independently checking the persisted
                // command identity and dead parent. The signalling boundary
                // accepts exclusively groups owned by this live registry.
                self.register_process(info.pid, &info.script_path);
                self.kill_process(info.pid);
                #[cfg(unix)]
                if self.reconcile_owned_process_cleanup(info.pid, observe_tracked_process(info.pid))
                {
                    killed_count += 1;
                } else {
                    warn!(
                        pid = info.pid,
                        "process_manager.cleanup_orphans.cleanup_unverified_retained"
                    );
                }
                #[cfg(not(unix))]
                {
                    self.unregister_process(info.pid);
                }
            }

            if let Err(e) = fs::remove_file(&path) {
                warn!(
                    error = %e,
                    path = ?path,
                    "process_manager.cleanup_orphans.remove_file_failed"
                );
            }
        }

        if killed_count > 0 {
            info!(killed_count, "process_manager.cleanup_orphans.success");
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(Self::FILE_PERMISSIONS);
            file.set_permissions(perms)?;
        }

        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Load persisted PIDs from this manager's own registry file
    #[cfg(test)]
    fn load_persisted_pids(&self) -> Vec<ProcessInfo> {
        load_pids_from(&self.active_pids_path)
    }
}

/// Load persisted PIDs from an arbitrary registry file
fn load_pids_from(path: &std::path::Path) -> Vec<ProcessInfo> {
    if !path.exists() {
        return Vec::new();
    }

    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                error = %e,
                path = ?path,
                "process_manager.load_persisted_pids.read_failed"
            );
            return Vec::new();
        }
    };

    match serde_json::from_str(&contents) {
        Ok(pids) => pids,
        Err(e) => {
            warn!(
                error = %e,
                path = ?path,
                "process_manager.load_persisted_pids.parse_failed"
            );
            Vec::new()
        }
    }
}
/// RAII registration for a tracked child: unregisters on drop so callers with
/// multiple exit paths (or `?` propagation) cannot leak a stale pid entry.
/// The caller remains responsible for actually killing/reaping the child.
pub struct ChildRegistration {
    pid: u32,
}

impl ChildRegistration {
    /// Register `pid` with the global manager for the lifetime of the guard.
    ///
    /// No-op under `cfg(test)`: test binaries must not write pid entries into
    /// the user's real ~/.scriptkit tracking files (a live app instance shares
    /// them).
    pub fn register(pid: u32, command_path: &str) -> Self {
        #[cfg(not(test))]
        PROCESS_MANAGER.register_process(pid, command_path);
        #[cfg(test)]
        let _ = command_path;
        Self { pid }
    }
}

impl Drop for ChildRegistration {
    fn drop(&mut self) {
        #[cfg(not(test))]
        {
            #[cfg(unix)]
            if observe_tracked_process(self.pid) != OwnedProcessGroupLiveness::Exited {
                warn!(
                    pid = self.pid,
                    "process_manager.child_registration.live_owner_retained"
                );
                return;
            }
            PROCESS_MANAGER.unregister_process(self.pid);
        }
        #[cfg(test)]
        let _ = self.pid;
    }
}

/// True when a live process command line still matches the path recorded at
/// registration time — the guard that keeps orphan cleanup from killing an
/// unrelated process that recycled the pid.
///
/// Matches when any argv segment contains the recorded string (bun scripts
/// record the script path, sidecars record the binary path, so both appear
/// verbatim in their own argv).
fn cmdline_matches_recorded(cmd: &[std::ffi::OsString], recorded: &str) -> bool {
    if recorded.is_empty() {
        return false;
    }
    cmd.iter()
        .any(|segment| segment.to_string_lossy().contains(recorded))
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a ProcessManager with a temporary directory for testing
    fn create_test_manager() -> (ProcessManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = ProcessManager {
            active_processes: Arc::new(RwLock::new(HashMap::new())),
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

    #[cfg(unix)]
    #[test]
    fn owned_process_group_probe_never_treats_failure_as_verified_cleanup() {
        assert_eq!(
            classify_owned_process_group_probe(0, None),
            OwnedProcessGroupLiveness::Alive
        );
        assert_eq!(
            classify_owned_process_group_probe(-1, Some(libc::EPERM)),
            OwnedProcessGroupLiveness::Alive
        );
        assert_eq!(
            classify_owned_process_group_probe(-1, Some(libc::EINVAL)),
            OwnedProcessGroupLiveness::Unverified
        );
        assert_eq!(
            classify_owned_process_group_probe(-1, None),
            OwnedProcessGroupLiveness::Unverified
        );
        assert_eq!(
            classify_owned_process_group_probe(-1, Some(libc::ESRCH)),
            OwnedProcessGroupLiveness::Exited
        );
    }

    #[cfg(unix)]
    #[test]
    fn process_signals_require_a_valid_explicitly_owned_group() {
        assert!(!owned_process_signal_is_authorized(0, true));
        assert!(!owned_process_signal_is_authorized(
            i32::MAX as u32 + 1,
            true
        ));
        assert!(!owned_process_signal_is_authorized(4242, false));
        assert!(owned_process_signal_is_authorized(4242, true));
    }

    #[cfg(unix)]
    #[test]
    fn owned_process_tracking_survives_cancellation_until_exact_group_exit() {
        let (manager, _temp_dir) = create_test_manager();
        manager.register_process(4242, "/isolated/synthetic-owner.ts");

        assert!(!manager.reconcile_owned_process_cleanup(4242, OwnedProcessGroupLiveness::Alive));
        assert_eq!(manager.active_count(), 1);
        assert!(
            !manager.reconcile_owned_process_cleanup(4242, OwnedProcessGroupLiveness::Unverified)
        );
        assert_eq!(manager.active_count(), 1);
        assert!(manager.reconcile_owned_process_cleanup(4242, OwnedProcessGroupLiveness::Exited));
        assert_eq!(manager.active_count(), 0);
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
    fn test_get_active_processes_sorted_newest_first() {
        let (manager, _temp_dir) = create_test_manager();
        manager.register_process(3001, "/path/to/first.ts");
        std::thread::sleep(std::time::Duration::from_millis(5));
        manager.register_process(3002, "/path/to/second.ts");

        let sorted = manager.get_active_processes_sorted();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].pid, 3002);
        assert_eq!(sorted[1].pid, 3001);
    }

    #[test]
    fn test_format_active_process_report_includes_summary_and_limit() {
        let (manager, _temp_dir) = create_test_manager();
        manager.register_process(4001, "/path/to/alpha.ts");
        manager.register_process(4002, "/path/to/beta.ts");

        let report = manager.format_active_process_report(1);
        assert!(report.contains("Active Script Kit processes: 2"));
        assert!(report.contains("PID "));
        assert!(report.contains("... and 1 more"));
    }

    #[test]
    fn test_format_active_process_report_empty_state() {
        let (manager, _temp_dir) = create_test_manager();
        let report = manager.format_active_process_report(5);
        assert_eq!(report, "No active Script Kit processes.");
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
    fn cmdline_match_accepts_recorded_path_in_any_argv_segment() {
        let cmd: Vec<std::ffi::OsString> = vec![
            "/opt/homebrew/bin/bun".into(),
            "/Users/me/.scriptkit/scripts/hello.ts".into(),
        ];
        assert!(cmdline_matches_recorded(
            &cmd,
            "/Users/me/.scriptkit/scripts/hello.ts"
        ));
        assert!(cmdline_matches_recorded(&cmd, "/opt/homebrew/bin/bun"));
    }

    #[test]
    fn cmdline_match_rejects_recycled_pid_and_empty_recording() {
        let cmd: Vec<std::ffi::OsString> = vec!["/usr/bin/ssh".into(), "example.com".into()];
        assert!(!cmdline_matches_recorded(
            &cmd,
            "/Users/me/.scriptkit/scripts/hello.ts"
        ));
        assert!(!cmdline_matches_recorded(&cmd, ""));
        assert!(!cmdline_matches_recorded(&[], "/anything"));
    }

    #[test]
    fn cleanup_orphans_skips_live_process_whose_cmdline_no_longer_matches() {
        let (manager, _temp_dir) = create_test_manager();

        // Record the CURRENT test process pid under a bogus script path. The
        // pid is alive, but its cmdline cannot contain the bogus path, so
        // cleanup must treat it as recycled and refuse to kill it (killing it
        // would take down this test run).
        manager.register_process(std::process::id(), "/definitely/not/this/test/binary.ts");
        // Simulate a fresh start: tracking survives only on disk.
        manager.active_processes.write().unwrap().clear();

        let killed = manager.cleanup_orphans();
        assert_eq!(killed, 0);
        assert!(
            !manager.active_pids_path.exists(),
            "cleanup must still clear the persisted pid file"
        );
    }

    #[test]
    fn cleanup_orphans_skips_child_whose_parent_is_still_alive() {
        let (manager, _temp_dir) = create_test_manager();

        // Our own child: cmdline matches the recorded path, but the parent
        // (this test process) is alive, so it is a sibling's child, not an
        // orphan — cleanup must leave it running.
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        manager.register_process(child.id(), "/bin/sleep");
        manager.active_processes.write().unwrap().clear();

        let killed = manager.cleanup_orphans();

        let still_running = matches!(child.try_wait(), Ok(None));
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(killed, 0);
        assert!(still_running, "live-parent child must not be killed");
    }

    #[test]
    #[cfg(unix)]
    fn cleanup_orphans_kills_true_orphan_with_matching_cmdline() {
        let (manager, _temp_dir) = create_test_manager();

        // Double-fork: sh backgrounds a sleep and exits, orphaning the sleep
        // to launchd — the real post-crash shape. The sleep keeps the dead
        // shell's pgid, so this also exercises the single-pid kill fallback.
        // (No `set -m`: a job-control sh kills its job group on exit. Stdout
        // must be detached or .output() blocks on the inherited pipe.)
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("/bin/sleep 300 >/dev/null 2>&1 & echo $!")
            .output()
            .expect("spawn orphaned sleep");
        let orphan_pid: u32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .expect("orphan pid");
        manager.register_process(orphan_pid, "/bin/sleep");
        manager.active_processes.write().unwrap().clear();

        // Give the shell a moment to exit so the sleep is reparented.
        std::thread::sleep(std::time::Duration::from_millis(200));

        let killed = manager.cleanup_orphans();
        assert_eq!(killed, 1, "orphaned sleep must be reaped");

        // SIGKILL delivery is asynchronous; poll briefly.
        let mut gone = false;
        for _ in 0..20 {
            // SAFETY: signal 0 only checks liveness.
            if unsafe { libc::kill(orphan_pid as i32, 0) } != 0 {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(gone, "orphan {orphan_pid} still alive after cleanup");
    }

    #[test]
    fn cleanup_orphans_respects_registry_owner_liveness() {
        let (manager, temp_dir) = create_test_manager();
        let dir = temp_dir.path();

        // Registry owned by a LIVE instance (this test process) must be left
        // untouched; registry owned by a dead pid must be swept and removed.
        let live_registry = dir.join(format!("active-pids-{}.json", std::process::id()));
        let dead_owner = {
            let mut probe = std::process::Command::new("/usr/bin/true")
                .spawn()
                .expect("spawn true");
            let pid = probe.id();
            probe.wait().expect("wait true");
            pid
        };
        let dead_registry = dir.join(format!("active-pids-{dead_owner}.json"));
        fs::write(&live_registry, "[]").unwrap();
        fs::write(&dead_registry, "[]").unwrap();

        let killed = manager.cleanup_orphans();

        assert_eq!(killed, 0);
        assert!(
            live_registry.exists(),
            "live-owner registry must not be deleted"
        );
        assert!(
            !dead_registry.exists(),
            "dead-owner registry must be swept away"
        );
    }

    #[test]
    fn registry_file_owner_parses_only_per_instance_names() {
        assert_eq!(
            ProcessManager::registry_file_owner("active-pids-12345.json"),
            Some(12345)
        );
        assert_eq!(
            ProcessManager::registry_file_owner("active-pids-.json"),
            None
        );
        assert_eq!(
            ProcessManager::registry_file_owner("active-bun-pids.json"),
            None
        );
        assert!(ProcessManager::is_registry_file("active-bun-pids.json"));
        assert!(ProcessManager::is_registry_file("active-pids-1.json"));
        assert!(!ProcessManager::is_registry_file("script-kit.pid"));
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

        // Should use ~/.scriptkit/ paths
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            manager.main_pid_path,
            home.join(".scriptkit/script-kit.pid")
        );
        assert_eq!(
            manager.active_pids_path,
            home.join(format!(
                ".scriptkit/active-pids-{}.json",
                std::process::id()
            ))
        );
    }
}

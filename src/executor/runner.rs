//! Script execution and process spawning
//!
//! This module handles the core script execution logic, including:
//! - Finding executables (bun, node, etc.)
//! - Spawning interactive script processes
//! - SDK path management
//! - File type detection

use super::stderr_buffer::{spawn_stderr_reader, StderrCapture};
use crate::logging;
use crate::process_manager::PROCESS_MANAGER;
use crate::protocol::JsonlReader;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;
use tracing::{debug, error, info, instrument};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

// Unix-specific process control using libc for correctness and performance
#[cfg(unix)]
mod unix_process {
    use libc::{c_int, pid_t, ESRCH};

    /// Send a signal to a process group (negative PID targets the group)
    ///
    /// Returns Ok(()) if signal was sent successfully.
    /// Returns Err with errno description on failure.
    pub fn kill_process_group(pgid: u32, signal: c_int) -> Result<(), &'static str> {
        if pgid == 0 || pgid > i32::MAX as u32 {
            return Err("Invalid owned process group");
        }
        // SAFETY: kill() is a simple syscall with no memory safety concerns.
        // Negative PID targets the process group. pgid is a valid u32 from Child::id().
        let rc = unsafe { libc::kill(-(pgid as pid_t), signal) };
        if rc == 0 {
            Ok(())
        } else {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            match errno {
                libc::ESRCH => Err("No such process group"),
                libc::EPERM => Err("Permission denied"),
                libc::EINVAL => Err("Invalid signal"),
                _ => Err("Could not stop script process group; stop it manually"),
            }
        }
    }

    /// Check if a process group is still alive
    ///
    /// Uses signal 0 which doesn't actually send a signal but checks if the
    /// process group exists. Returns true if any process in the group is alive.
    ///
    /// Note: EPERM (permission denied) also means the process exists but we
    /// don't have permission to signal it - we still count this as "alive".
    pub fn process_group_alive(pgid: u32) -> bool {
        if pgid == 0 || pgid > i32::MAX as u32 {
            return true;
        }
        // SAFETY: kill() with signal 0 is safe — it only checks process group existence
        // without sending an actual signal. pgid is a valid u32 from Child::id().
        let rc = unsafe { libc::kill(-(pgid as pid_t), 0) };
        if rc == 0 {
            true
        } else {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            // EPERM means process exists but we can't signal it - still alive
            // ESRCH means no such process - dead
            errno != ESRCH
        }
    }

    /// SIGTERM signal number
    pub const SIGTERM: c_int = libc::SIGTERM;
    /// SIGKILL signal number
    pub const SIGKILL: c_int = libc::SIGKILL;
}

/// Embedded SDK content (included at compile time)
const EMBEDDED_SDK: &str = include_str!("../../scripts/kit-sdk.ts");

const SAFE_SCRIPT_ENV_VARS: &[&str] = &[
    "PATH",
    "HOME",
    "TMPDIR",
    "USER",
    "LANG",
    "TERM",
    "SHELL",
    "XDG_RUNTIME_DIR",
];

/// OnceLock for single-flight SDK extraction
/// Ensures SDK is extracted exactly once, preventing race conditions
/// when multiple scripts start simultaneously
static SDK_EXTRACTED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Find an executable, checking common locations that GUI apps might miss
pub fn find_executable(name: &str) -> Option<PathBuf> {
    info!(category = "EXEC", executable = %name, "Looking for executable");

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
        info!(
            category = "EXEC",
            executable_path = %exe_path.display(),
            "Checking executable path"
        );
        if exe_path.exists() {
            info!(
                category = "EXEC",
                executable_path = %exe_path.display(),
                "Found executable"
            );
            return Some(exe_path);
        }
    }

    info!(
        category = "EXEC",
        "Executable not found in common paths, will try PATH"
    );
    None
}

/// Get the SDK path - SDK extraction is now handled by setup::ensure_kit_setup() at startup
/// This function just returns the expected path since setup has already done the work
///
/// ## Race Condition Prevention
/// Uses OnceLock to ensure SDK is extracted exactly once, even when multiple scripts
/// start simultaneously. The fallback extraction uses atomic write (temp + rename)
/// to prevent partial reads.
fn ensure_sdk_extracted() -> Option<PathBuf> {
    SDK_EXTRACTED
        .get_or_init(|| {
            // Target path: ~/.scriptkit/sdk/kit-sdk.ts
            // This is extracted by setup::ensure_kit_setup() which runs at app startup
            let home = dirs::home_dir()?;
            let sdk_path = home.join(".scriptkit/sdk/kit-sdk.ts");

            if sdk_path.exists() {
                return Some(sdk_path);
            }

            // Fallback: write embedded SDK if somehow missing
            // This shouldn't happen in normal operation since setup runs first
            info!(
                category = "EXEC",
                "SDK not found at expected path, extracting embedded SDK"
            );

            let kit_sdk = home.join(".scriptkit/sdk");
            if !kit_sdk.exists() {
                std::fs::create_dir_all(&kit_sdk).ok()?;
            }

            // Atomic write: temp file then rename to prevent partial reads
            let temp_path = sdk_path.with_extension("tmp");
            std::fs::write(&temp_path, EMBEDDED_SDK).ok()?;
            std::fs::rename(&temp_path, &sdk_path).ok()?;

            info!(
                category = "EXEC",
                sdk_path = %sdk_path.display(),
                "Extracted fallback SDK"
            );
            Some(sdk_path)
        })
        .clone()
}

/// Find the SDK path, checking standard locations
pub fn find_sdk_path() -> Option<PathBuf> {
    info!(category = "EXEC", "Looking for SDK");

    // 1. Check ~/.scriptkit/sdk/kit-sdk.ts (primary location)
    if let Some(home) = dirs::home_dir() {
        let kit_sdk = home.join(".scriptkit/sdk/kit-sdk.ts");
        info!(
            category = "EXEC",
            sdk_path = %kit_sdk.display(),
            "Checking kit SDK path"
        );
        if kit_sdk.exists() {
            info!(
                category = "EXEC",
                sdk_path = %kit_sdk.display(),
                "Found SDK in kit directory"
            );
            return Some(kit_sdk);
        }
    }

    // 2. Extract embedded SDK to ~/.scriptkit/sdk/kit-sdk.ts (production)
    info!(category = "EXEC", "Trying to extract embedded SDK");
    if let Some(sdk_path) = ensure_sdk_extracted() {
        info!(
            category = "EXEC",
            sdk_path = %sdk_path.display(),
            "Found embedded SDK"
        );
        return Some(sdk_path);
    }

    // 3. Check relative to executable (for app bundle)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let sdk_path = exe_dir.join("kit-sdk.ts");
            info!(
                category = "EXEC",
                sdk_path = %sdk_path.display(),
                "Checking SDK in executable directory"
            );
            if sdk_path.exists() {
                info!(
                    category = "EXEC",
                    sdk_path = %sdk_path.display(),
                    "Found SDK in executable directory"
                );
                return Some(sdk_path);
            }
        }
    }

    // 4. Development fallback - project scripts directory
    let dev_sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/kit-sdk.ts");
    info!(
        category = "EXEC",
        sdk_path = %dev_sdk.display(),
        "Checking development SDK path"
    );
    if dev_sdk.exists() {
        info!(
            category = "EXEC",
            sdk_path = %dev_sdk.display(),
            "Found development SDK"
        );
        return Some(dev_sdk);
    }

    info!(category = "EXEC", "SDK not found anywhere");
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
    pub(crate) pid: u32,
    /// Path to the script being executed (for process tracking)
    /// Used during registration with PROCESS_MANAGER in new()
    #[allow(dead_code)]
    script_path: String,
    /// Whether the process has been explicitly killed
    pub(crate) killed: bool,
}

impl ProcessHandle {
    pub fn new(pid: u32, script_path: String) -> Self {
        info!(
            category = "EXEC",
            pid,
            script_path = %script_path,
            "ProcessHandle created"
        );

        // Register with global process manager for tracking
        PROCESS_MANAGER.register_process(pid, &script_path);

        Self {
            pid,
            script_path,
            killed: false,
        }
    }

    /// Kill the process group with graceful escalation (Unix) or just the process (other platforms)
    ///
    /// ## Escalation Protocol
    /// 1. Send SIGTERM to process group (graceful termination request)
    /// 2. Wait up to TERM_GRACE_MS for process group to exit
    /// 3. If still alive, send SIGKILL (forceful termination)
    ///
    /// ## Critical Fix
    /// Uses libc::kill() to check process GROUP liveness (not just leader PID).
    /// This prevents orphan child processes when the leader exits but children remain.
    ///
    /// This gives scripts a chance to clean up before being forcefully killed.
    pub fn kill(&mut self) {
        /// Grace period after SIGTERM before escalating to SIGKILL (milliseconds)
        const TERM_GRACE_MS: u64 = 250;
        /// How often to check if process has exited during grace period
        const POLL_INTERVAL_MS: u64 = 50;

        if self.killed {
            info!(
                category = "EXEC",
                pid = self.pid,
                "Process already killed, skipping"
            );
            return;
        }

        #[cfg(unix)]
        {
            use unix_process::{kill_process_group, process_group_alive, SIGKILL, SIGTERM};

            // Since we spawned with process_group(0), the PGID equals the PID
            let pgid = self.pid;
            if pgid == 0 || pgid > i32::MAX as u32 {
                error!(
                    category = "EXEC",
                    pgid, "Refusing invalid owned process group"
                );
                return;
            }

            // Step 1: Send SIGTERM for graceful shutdown
            info!(
                category = "EXEC",
                pgid,
                signal = "SIGTERM",
                "Sending graceful shutdown signal to process group"
            );

            match kill_process_group(pgid, SIGTERM) {
                Ok(()) => {
                    info!(
                        category = "EXEC",
                        pgid,
                        signal = "SIGTERM",
                        "Signal sent to process group"
                    );
                }
                Err("No such process group") => {
                    if self.confirm_group_cleanup("SIGTERM") {
                        info!(category = "EXEC", pgid, "Process group already exited");
                    }
                    return;
                }
                Err(e) => {
                    info!(
                        category = "EXEC",
                        pgid,
                        signal = "SIGTERM",
                        error = %e,
                        "Failed to send signal to process group"
                    );
                    // Continue to try SIGKILL anyway
                }
            }

            // Step 2: Wait for grace period, polling process GROUP (not just leader)
            let start = std::time::Instant::now();
            let grace_duration = std::time::Duration::from_millis(TERM_GRACE_MS);
            let poll_interval = std::time::Duration::from_millis(POLL_INTERVAL_MS);

            while start.elapsed() < grace_duration {
                // CRITICAL: Check if process GROUP is alive, not just the leader PID
                // This prevents orphan processes when the leader exits but children remain
                if !process_group_alive(pgid) {
                    if self.confirm_group_cleanup("SIGTERM") {
                        info!(
                            category = "EXEC",
                            pgid,
                            signal = "SIGTERM",
                            "Process group terminated gracefully"
                        );
                    }
                    return;
                }
                std::thread::sleep(poll_interval);
            }

            // Step 3: Process group didn't exit in time, escalate to SIGKILL
            info!(
                category = "EXEC",
                pgid,
                grace_ms = TERM_GRACE_MS,
                signal = "SIGKILL",
                "Escalating to force kill after grace period"
            );

            match kill_process_group(pgid, SIGKILL) {
                Ok(()) => {
                    info!(
                        category = "EXEC",
                        pgid,
                        signal = "SIGKILL",
                        "Force-stop signal delivered; awaiting owned group cleanup"
                    );
                }
                Err("No such process group") => {
                    info!(
                        category = "EXEC",
                        pgid,
                        signal = "SIGKILL",
                        "Process group exited before force kill"
                    );
                }
                Err(e) => {
                    info!(
                        category = "EXEC",
                        pgid,
                        signal = "SIGKILL",
                        error = %e,
                        "Force kill failed"
                    );
                }
            }

            if !self.confirm_group_cleanup("SIGKILL") {
                error!(
                    category = "EXEC",
                    pgid, "Owned process group remains tracked because cleanup was not verified"
                );
            }
        }

        #[cfg(not(unix))]
        {
            info!(
                category = "EXEC",
                pid = self.pid,
                platform = "non_unix",
                "Process marked as killed"
            );
            // On non-Unix platforms, we rely on the Child::kill() method being called separately
        }
    }

    #[cfg(unix)]
    fn confirm_group_cleanup(&mut self, signal: &'static str) -> bool {
        let state = crate::process_manager::observe_owned_process_group(self.pid);
        if !owned_group_cleanup_confirmed(state) {
            return false;
        }
        self.killed = true;
        super::telemetry::log_script_killed("executor::process_handle_kill", self.pid, signal);
        true
    }

    /// Check if process group is still running (Unix only)
    ///
    /// Checks the entire process group, not just the leader PID.
    /// This ensures we properly detect when all child processes have exited.
    #[cfg(unix)]
    #[allow(dead_code)]
    pub fn is_alive(&self) -> bool {
        unix_process::process_group_alive(self.pid)
    }
}

#[cfg(unix)]
fn owned_group_cleanup_confirmed(
    observation: crate::process_manager::OwnedProcessGroupLiveness,
) -> bool {
    observation == crate::process_manager::OwnedProcessGroupLiveness::Exited
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        info!(category = "EXEC", pid = self.pid, "Dropping process handle");

        self.kill();
        if self.killed {
            PROCESS_MANAGER.unregister_process(self.pid);
        } else {
            error!(
                category = "EXEC",
                pid = self.pid,
                "Retaining unverified owned process in the global cleanup registry"
            );
        }
    }
}

/// Session for bidirectional communication with a running script
pub struct ScriptSession {
    pub stdin: ChildStdin,
    pub(crate) stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    /// Captured stderr for error reporting
    pub stderr: Option<StderrCapture>,
    pub(crate) child: Child,
    /// Process handle for cleanup - kills process group on drop
    pub(crate) process_handle: ProcessHandle,
}

/// Split session components for separate read/write threads
pub struct SplitSession {
    pub stdin: ChildStdin,
    pub stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    /// Captured stderr for error reporting
    pub stderr: Option<StderrCapture>,
    pub child: Child,
    /// Process handle for cleanup - kills process group on drop
    /// IMPORTANT: This MUST be kept alive until the script completes!
    /// Dropping it will kill the process group via the Drop impl.
    pub process_handle: ProcessHandle,
}

#[allow(dead_code)]
impl ScriptSession {
    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.process_handle.pid
    }

    /// Split the session into separate read/write components
    /// This allows using separate threads for reading and writing
    pub fn split(self) -> SplitSession {
        info!(
            category = "EXEC",
            pid = self.process_handle.pid,
            "Splitting script session"
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
        info!(
            category = "EXEC",
            pid = self.process_handle.pid,
            "Killing split session process"
        );
        self.process_handle.kill();
        // Also try the standard kill for good measure
        let _ = self.child.kill();
        #[cfg(unix)]
        {
            if !self.process_handle.killed && !self.process_handle.confirm_group_cleanup("SIGKILL")
            {
                return Err(
                    "Owned script process group is still alive; cleanup remains pending"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    /// Wait for the child process to terminate and get its exit code
    pub fn wait(&mut self) -> Result<i32, String> {
        let status = self
            .child
            .wait()
            .map_err(|e| format!("Failed to wait for script process: {}", e))?;
        let code = status.code().unwrap_or(-1);
        info!(category = "EXEC", exit_code = code, "Script exited");
        super::telemetry::log_script_exited(
            "executor::split_session_wait",
            self.process_handle.pid,
            code,
            None,
        );
        Ok(code)
    }

    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.process_handle.pid
    }
}

#[derive(Debug)]
struct RuntimeAttempt {
    name: &'static str,
    label: String,
    cmd: String,
    args: Vec<String>,
}

fn run_fallback_chain<T>(
    attempts: &[RuntimeAttempt],
    start: &Instant,
    mut runner: impl FnMut(&str, &[&str], &str) -> Result<T, String>,
    script_path: &str,
) -> Option<T> {
    for attempt in attempts {
        if attempt.args.is_empty() {
            info!(
                category = "EXEC",
                command = %attempt.cmd,
                "Trying runtime fallback command"
            );
        } else {
            info!(
                category = "EXEC",
                command = %attempt.cmd,
                args = ?attempt.args,
                "Trying runtime fallback command"
            );
        }
        logging::bench_log(attempt.name);

        let args: Vec<&str> = attempt.args.iter().map(String::as_str).collect();
        match runner(&attempt.cmd, &args, script_path) {
            Ok(result) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                info!(
                    runtime = attempt.name,
                    duration_ms = duration_ms,
                    "Runtime fallback succeeded"
                );
                logging::bench_log(&format!("{} succeeded in {}ms", attempt.name, duration_ms));
                info!(
                    category = "EXEC",
                    runtime_label = %attempt.label,
                    "Runtime fallback succeeded"
                );
                return Some(result);
            }
            Err(e) => {
                debug!(error = %e, runtime = attempt.name, "Spawn attempt failed");
                info!(
                    category = "EXEC",
                    runtime_label = %attempt.label,
                    error = %e,
                    "Runtime fallback failed"
                );
            }
        }
    }
    None
}

#[instrument(skip_all, fields(script_path = %path.display(), extra_env_keys = extra_env.len(), argv_count = argv.len()))]
pub fn execute_script_interactive_with_env_and_args(
    path: &Path,
    extra_env: Vec<(String, String)>,
    argv: Vec<String>,
) -> Result<ScriptSession, String> {
    execute_script_interactive_impl(path, extra_env, argv)
}

fn execute_script_interactive_impl(
    path: &Path,
    extra_env: Vec<(String, String)>,
    argv: Vec<String>,
) -> Result<ScriptSession, String> {
    let start = Instant::now();
    logging::bench_log("execute_script_interactive_start");
    debug!(path = %path.display(), "Starting interactive script execution");
    info!(
        category = "EXEC",
        script_path = %path.display(),
        "Starting execute_script_interactive"
    );

    let path_str = path
        .to_str()
        .ok_or_else(|| "Invalid path encoding".to_string())?;

    // Brain activity journal: running a script is a decision worth
    // remembering ("what script did I run earlier?"). Off-thread.
    crate::brain::record_activity("ran script", path_str);

    // Find SDK for preloading
    let sdk_path = find_sdk_path();

    let bun_path = "bun".to_string();
    let node_path = "node".to_string();

    let mut attempts = Vec::new();
    if is_typescript(path) {
        if let Some(ref sdk) = sdk_path {
            let sdk_str = sdk.to_string_lossy().into_owned();
            attempts.push(RuntimeAttempt {
                name: "bun",
                label: "bun with preload".to_string(),
                cmd: bun_path.clone(),
                args: with_script_argv(
                    vec![
                        "run".to_string(),
                        "--preload".to_string(),
                        sdk_str,
                        path_str.to_string(),
                    ],
                    &argv,
                ),
            });
        }

        attempts.push(RuntimeAttempt {
            name: "bun",
            label: "bun without preload".to_string(),
            cmd: bun_path,
            args: with_script_argv(vec!["run".to_string(), path_str.to_string()], &argv),
        });
    }

    if is_javascript(path) {
        attempts.push(RuntimeAttempt {
            name: "node",
            label: "node".to_string(),
            cmd: node_path,
            args: with_script_argv(vec![path_str.to_string()], &argv),
        });
    }

    let extra_env_for_closure = extra_env.clone();
    let runner = move |cmd: &str, args: &[&str], script_path: &str| {
        spawn_script_with_extra_env(cmd, args, script_path, &extra_env_for_closure)
    };
    if let Some(session) = run_fallback_chain(&attempts, &start, runner, path_str) {
        return Ok(session);
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
    info!(
        category = "EXEC",
        error = %err,
        "All interactive script execution methods failed"
    );
    Err(err)
}

fn with_script_argv(mut args: Vec<String>, argv: &[String]) -> Vec<String> {
    args.extend(argv.iter().cloned());
    args
}

/// Spawn a script as an interactive process with piped stdin/stdout
#[instrument(skip_all, fields(cmd = %cmd))]
#[allow(dead_code)]
pub fn spawn_script(cmd: &str, args: &[&str], script_path: &str) -> Result<ScriptSession, String> {
    spawn_script_with_extra_env(cmd, args, script_path, &[])
}

pub fn spawn_script_with_extra_env(
    cmd: &str,
    args: &[&str],
    script_path: &str,
    extra_env: &[(String, String)],
) -> Result<ScriptSession, String> {
    // Try to find the executable in common locations
    let executable = find_executable(cmd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| cmd.to_string());

    debug!(executable = %executable, args = ?args, "Spawning script process");
    info!(
        category = "EXEC",
        executable = %executable,
        args = ?args,
        "Spawning script"
    );

    let mut command = Command::new(&executable);
    command.env_clear();
    for key in SAFE_SCRIPT_ENV_VARS {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    for (key, value) in std::env::vars() {
        if key.starts_with("SCRIPT_KIT") {
            command.env(key, value);
        }
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
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
        info!(category = "EXEC", "Using process group for child process");
    }

    let mut child = command.spawn().map_err(|e| {
        error!(error = %e, executable = %executable, "Process spawn failed");
        let err = format!("Failed to spawn '{}': {}", executable, e);
        info!(category = "EXEC", error = %err, "Script spawn failed");
        err
    })?;

    let pid = child.id();
    info!(pid = pid, pgid = pid, executable = %executable, "Process spawned");
    info!(category = "EXEC", pid, pgid = pid, "Process spawned");

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open script stdin".to_string())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open script stdout".to_string())?;

    // Start a background stderr drain so child writes cannot block on a full pipe.
    let stderr = child
        .stderr
        .take()
        .map(|stderr| spawn_stderr_reader(stderr, script_path.to_string()));
    info!(
        category = "EXEC",
        stderr_capture_started = stderr.is_some(),
        "Started stderr capture"
    );

    let process_handle = ProcessHandle::new(pid, script_path.to_string());
    info!(category = "EXEC", pid, "ScriptSession created");
    super::telemetry::log_script_spawned("executor::spawn_script_with_extra_env", script_path, pid);

    Ok(ScriptSession {
        stdin,
        stdout_reader: JsonlReader::new(BufReader::new(stdout)),
        stderr,
        child,
        process_handle,
    })
}

/// Check if the path points to a TypeScript file
pub fn is_typescript(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "ts")
        .unwrap_or(false)
}

/// Check if the path points to a JavaScript file
pub fn is_javascript(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext == "js")
        .unwrap_or(false)
}

#[cfg(all(test, unix))]
mod process_cleanup_contract_tests {
    use super::owned_group_cleanup_confirmed;
    use crate::process_manager::OwnedProcessGroupLiveness;

    #[test]
    fn owned_script_cleanup_requires_observed_process_group_exit() {
        assert!(!owned_group_cleanup_confirmed(
            OwnedProcessGroupLiveness::Alive
        ));
        assert!(!owned_group_cleanup_confirmed(
            OwnedProcessGroupLiveness::Unverified
        ));
        assert!(owned_group_cleanup_confirmed(
            OwnedProcessGroupLiveness::Exited
        ));
    }
}

#[cfg(test)]
mod runtime_fallback_tests {
    use super::{run_fallback_chain, RuntimeAttempt};
    use std::cell::RefCell;
    use std::time::Instant;

    #[test]
    fn test_run_fallback_chain_returns_first_success_when_previous_attempt_fails() {
        let start = Instant::now();
        let calls = RefCell::new(Vec::new());
        let attempts = vec![
            RuntimeAttempt {
                name: "kit",
                label: "kit".to_string(),
                cmd: "kit".to_string(),
                args: vec!["run".to_string(), "test.ts".to_string()],
            },
            RuntimeAttempt {
                name: "bun",
                label: "bun without preload".to_string(),
                cmd: "bun".to_string(),
                args: vec!["run".to_string(), "test.ts".to_string()],
            },
            RuntimeAttempt {
                name: "node",
                label: "node".to_string(),
                cmd: "node".to_string(),
                args: vec!["test.js".to_string()],
            },
        ];

        let result = run_fallback_chain(
            &attempts,
            &start,
            |cmd, _, _| {
                calls.borrow_mut().push(cmd.to_string());
                if cmd == "kit" {
                    Err("kit missing".to_string())
                } else {
                    Ok(cmd.to_string())
                }
            },
            "test.ts",
        );

        assert_eq!(result, Some("bun".to_string()));
        assert_eq!(
            calls.borrow().as_slice(),
            &["kit".to_string(), "bun".to_string()]
        );
    }

    #[test]
    fn test_run_fallback_chain_returns_none_when_all_attempts_fail() {
        let start = Instant::now();
        let attempts = vec![
            RuntimeAttempt {
                name: "kit",
                label: "kit".to_string(),
                cmd: "kit".to_string(),
                args: vec!["run".to_string(), "test.ts".to_string()],
            },
            RuntimeAttempt {
                name: "node",
                label: "node".to_string(),
                cmd: "node".to_string(),
                args: vec!["test.js".to_string()],
            },
        ];

        let result: Option<String> = run_fallback_chain(
            &attempts,
            &start,
            |_, _, _| Err("failed".to_string()),
            "test.ts",
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_run_fallback_chain_passes_script_path_and_args_to_runner() {
        let start = Instant::now();
        let attempts = vec![RuntimeAttempt {
            name: "bun",
            label: "bun with preload".to_string(),
            cmd: "bun".to_string(),
            args: vec![
                "run".to_string(),
                "--preload".to_string(),
                "sdk.ts".to_string(),
                "test.ts".to_string(),
            ],
        }];

        let captured = RefCell::new((String::new(), Vec::new(), String::new()));
        let result = run_fallback_chain(
            &attempts,
            &start,
            |cmd, args, script_path| {
                captured.borrow_mut().0 = cmd.to_string();
                captured.borrow_mut().1 = args.iter().map(|arg| arg.to_string()).collect();
                captured.borrow_mut().2 = script_path.to_string();
                Ok("ok".to_string())
            },
            "test.ts",
        );

        assert_eq!(result, Some("ok".to_string()));
        assert_eq!(captured.borrow().0, "bun".to_string());
        assert_eq!(
            captured.borrow().1,
            vec![
                "run".to_string(),
                "--preload".to_string(),
                "sdk.ts".to_string(),
                "test.ts".to_string()
            ]
        );
        assert_eq!(captured.borrow().2, "test.ts".to_string());
    }
}

#[cfg(test)]
#[cfg(unix)]
mod env_scrub_tests {
    use super::spawn_script;
    use std::env;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var_os(key);
            env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn test_spawn_script_forwards_script_kit_vars_when_env_is_scrubbed() {
        let _lock = env_lock().lock().expect("env lock should be available");
        let _script_kit = EnvVarGuard::set("SCRIPT_KIT_ENV_SCRUB_SPAWN", "forwarded");
        let _private = EnvVarGuard::set("RUNNER_ENV_SCRUB_SPAWN_PRIVATE", "blocked");

        let session = spawn_script(
            "sh",
            &[
                "-c",
                "printf '%s|%s' \"${SCRIPT_KIT_ENV_SCRUB_SPAWN:-missing}\" \"${RUNNER_ENV_SCRUB_SPAWN_PRIVATE:-missing}\" >&2",
            ],
            "[test:runner_env_scrub_spawn]",
        )
        .expect("spawn_script should succeed");

        let mut split = session.split();

        let exit_code = split.wait().expect("wait should succeed");
        assert_eq!(exit_code, 0);

        let stderr_capture = split.stderr.take().expect("stderr should be captured");
        let stderr = stderr_capture.get_contents_with_timeout(Duration::from_millis(500));

        assert_eq!(stderr, "forwarded|missing");
    }

    #[test]
    fn test_spawn_script_drains_stderr_when_output_exceeds_pipe_capacity() {
        let session = spawn_script(
            "sh",
            &["-c", "yes stderr_line | head -n 25000 >&2"],
            "[test:runner_stderr_drain]",
        )
        .expect("spawn_script should succeed");

        let mut split = session.split();
        let deadline = Instant::now() + Duration::from_secs(5);

        let status = loop {
            match split.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = split.kill();
                        panic!("child did not exit after writing large stderr output");
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => {
                    let _ = split.kill();
                    panic!("failed to poll child status: {}", error);
                }
            }
        };
        assert!(
            status.success(),
            "stderr stress script should exit successfully"
        );

        let capture = split.stderr.take().expect("stderr should be captured");
        let stderr = capture.get_contents_with_timeout(Duration::from_millis(500));
        assert!(
            !stderr.is_empty(),
            "stderr capture should include drained output"
        );
    }
}

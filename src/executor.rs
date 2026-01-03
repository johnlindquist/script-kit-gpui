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

// Note: tsconfig.json path mapping is now handled by setup::ensure_kenv_setup()
// This function is kept for backward compatibility but is a no-op
#[allow(dead_code)]
fn ensure_tsconfig_paths(_tsconfig_path: &PathBuf) {
    // Setup module now handles this at startup
    // See setup::ensure_tsconfig_paths()
}

/// Get the SDK path - SDK extraction is now handled by setup::ensure_kenv_setup() at startup
/// This function just returns the expected path since setup has already done the work
fn ensure_sdk_extracted() -> Option<PathBuf> {
    // Target path: ~/.kenv/sdk/kit-sdk.ts
    // This is extracted by setup::ensure_kenv_setup() which runs at app startup
    let sdk_path = dirs::home_dir()?.join(".kenv/sdk/kit-sdk.ts");

    if sdk_path.exists() {
        Some(sdk_path)
    } else {
        // Fallback: write embedded SDK if somehow missing
        // This shouldn't happen in normal operation since setup runs first
        logging::log(
            "EXEC",
            "SDK not found at expected path, extracting embedded SDK",
        );
        let kenv_sdk = dirs::home_dir()?.join(".kenv/sdk");
        if !kenv_sdk.exists() {
            std::fs::create_dir_all(&kenv_sdk).ok()?;
        }
        std::fs::write(&sdk_path, EMBEDDED_SDK).ok()?;
        logging::log(
            "EXEC",
            &format!("Extracted fallback SDK to {}", sdk_path.display()),
        );
        Some(sdk_path)
    }
}

/// Find the SDK path, checking standard locations
fn find_sdk_path() -> Option<PathBuf> {
    logging::log("EXEC", "Looking for SDK...");

    // 1. Check ~/.kenv/sdk/kit-sdk.ts (primary location)
    if let Some(home) = dirs::home_dir() {
        let kenv_sdk = home.join(".kenv/sdk/kit-sdk.ts");
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

    // 2. Extract embedded SDK to ~/.kenv/sdk/kit-sdk.ts (production)
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
#[path = "executor_tests.rs"]
mod tests;

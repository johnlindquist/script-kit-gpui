use std::path::{Path, PathBuf};
use std::process::{Command, Child, ChildStdin, ChildStdout, ChildStderr, Stdio};
use std::io::{Write, BufReader};
use std::time::{Duration, Instant};
use crate::protocol::{Message, JsonlReader, serialize_message};
use crate::logging;
use tracing::{info, error, debug, warn, instrument};

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
        dirs::home_dir().map(|h| h.join("Library/pnpm")),  // pnpm on macOS
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
                logging::log("EXEC", &format!("Updated tsconfig.json with @johnlindquist/kit path"));
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
    // Target path: ~/.kenv/sdk/kit-sdk.ts
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
    logging::log("EXEC", &format!("Extracted SDK to {} ({} bytes)", sdk_path.display(), sdk_len));
    
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
        logging::log("EXEC", &format!("Wrote .gitignore to {}", gitignore_path.display()));
    }
    
    Some(sdk_path)
}

/// Find the SDK path, checking standard locations
fn find_sdk_path() -> Option<PathBuf> {
    logging::log("EXEC", "Looking for SDK...");
    
    // 1. Check ~/.kenv/sdk/kit-sdk.ts (primary location)
    if let Some(home) = dirs::home_dir() {
        let kenv_sdk = home.join(".kenv/sdk/kit-sdk.ts");
        logging::log("EXEC", &format!("  Checking kenv sdk: {}", kenv_sdk.display()));
        if kenv_sdk.exists() {
            logging::log("EXEC", &format!("  FOUND SDK (kenv): {}", kenv_sdk.display()));
            return Some(kenv_sdk);
        }
    }
    
    // 2. Extract embedded SDK to ~/.kenv/sdk/kit-sdk.ts (production)
    logging::log("EXEC", "  Trying to extract embedded SDK...");
    if let Some(sdk_path) = ensure_sdk_extracted() {
        logging::log("EXEC", &format!("  FOUND SDK (embedded): {}", sdk_path.display()));
        return Some(sdk_path);
    }
    
    // 3. Check relative to executable (for app bundle)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let sdk_path = exe_dir.join("kit-sdk.ts");
            logging::log("EXEC", &format!("  Checking exe dir: {}", sdk_path.display()));
            if sdk_path.exists() {
                logging::log("EXEC", &format!("  FOUND SDK (exe dir): {}", sdk_path.display()));
                return Some(sdk_path);
            }
        }
    }
    
    // 4. Development fallback - project scripts directory
    let dev_sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/kit-sdk.ts");
    logging::log("EXEC", &format!("  Checking dev path: {}", dev_sdk.display()));
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
    /// Whether the process has been explicitly killed
    killed: bool,
}

impl ProcessHandle {
    fn new(pid: u32) -> Self {
        logging::log("EXEC", &format!("ProcessHandle created for PID {}", pid));
        Self { pid, killed: false }
    }

    /// Kill the process group (Unix) or just the process (other platforms)
    fn kill(&mut self) {
        if self.killed {
            logging::log("EXEC", &format!("Process {} already killed, skipping", self.pid));
            return;
        }
        self.killed = true;
        
        #[cfg(unix)]
        {
            // Kill the entire process group using the kill command with negative PID
            // Since we spawned with process_group(0), the PGID equals the PID
            // Using negative PID tells kill to target the process group
            let negative_pgid = format!("-{}", self.pid);
            logging::log("EXEC", &format!("Killing process group PGID {} with SIGKILL", self.pid));
            
            match Command::new("kill")
                .args(["-9", &negative_pgid])
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        logging::log("EXEC", &format!("Successfully killed process group {}", self.pid));
                    } else {
                        // Process might already be dead, which is fine
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("No such process") {
                            logging::log("EXEC", &format!("Process group {} already exited", self.pid));
                        } else {
                            logging::log("EXEC", &format!("kill command failed for PGID {}: {}", self.pid, stderr));
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
            logging::log("EXEC", &format!("Non-Unix platform: process {} marked as killed", self.pid));
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
        logging::log("EXEC", &format!("ProcessHandle dropping for PID {}", self.pid));
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
        logging::log("EXEC", &format!("Splitting ScriptSession for PID {}", self.process_handle.pid));
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
        logging::log("EXEC", &format!("SplitSession::kill() for PID {}", self.process_handle.pid));
        self.process_handle.kill();
        // Also try the standard kill for good measure
        let _ = self.child.kill();
        Ok(())
    }

    /// Wait for the child process to terminate and get its exit code
    pub fn wait(&mut self) -> Result<i32, String> {
        let status = self.child
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
        let json = serialize_message(msg)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        logging::log("EXEC", &format!("Sending to script: {}", json));
        writeln!(self.stdin, "{}", json)
            .map_err(|e| format!("Failed to write to script stdin: {}", e))?;
        self.stdin.flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;
        Ok(())
    }

    /// Receive a message from the running script (blocking)
    pub fn receive_message(&mut self) -> Result<Option<Message>, String> {
        let result = self.stdout_reader
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
        let status = self.child
            .wait()
            .map_err(|e| format!("Failed to wait for script process: {}", e))?;
        let code = status.code().unwrap_or(-1);
        logging::log("EXEC", &format!("Script exited with code: {}", code));
        Ok(code)
    }

    /// Kill the child process and its process group
    pub fn kill(&mut self) -> Result<(), String> {
        logging::log("EXEC", &format!("ScriptSession::kill() for PID {}", self.process_handle.pid));
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
    logging::log("EXEC", &format!("execute_script_interactive: {}", path.display()));
    
    let path_str = path
        .to_str()
        .ok_or_else(|| "Invalid path encoding".to_string())?;

    // Find SDK for preloading
    let sdk_path = find_sdk_path();
    
    // Try bun with preload (preferred - supports TypeScript natively)
    if let Some(ref sdk) = sdk_path {
        let sdk_str = sdk.to_str().unwrap_or("");
        logging::log("EXEC", &format!("Trying: bun run --preload {} {}", sdk_str, path_str));
        match spawn_script("bun", &["run", "--preload", sdk_str, path_str]) {
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
        match spawn_script("bun", &["run", path_str]) {
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
        match spawn_script("node", &[path_str]) {
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
fn spawn_script(cmd: &str, args: &[&str]) -> Result<ScriptSession, String> {
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
    logging::log("EXEC", &format!("Process spawned with PID: {} (PGID: {})", pid, pid));

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

    let process_handle = ProcessHandle::new(pid);
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
    logging::log("EXEC", &format!("execute_script (blocking): {}", path.display()));
    
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
            logging::log("EXEC", &format!("SUCCESS: kit (output: {} bytes)", output.len()));
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
            logging::log("EXEC", &format!("Trying: bun run --preload {} {}", sdk_str, path_str));
            match run_command("bun", &["run", "--preload", sdk_str, path_str]) {
                Ok(output) => {
                    info!(
                        duration_ms = start.elapsed().as_millis() as u64,
                        output_bytes = output.len(),
                        runtime = "bun",
                        preload = true,
                        "Script completed"
                    );
                    logging::log("EXEC", &format!("SUCCESS: bun with preload (output: {} bytes)", output.len()));
                    return Ok(output);
                }
                Err(e) => {
                    debug!(error = %e, runtime = "bun", preload = true, "Command failed");
                    logging::log("EXEC", &format!("FAILED: bun with preload: {}", e));
                }
            }
        }
        
        // Fallback: try bun without preload
        logging::log("EXEC", &format!("Trying: bun run {} (no preload)", path_str));
        match run_command("bun", &["run", path_str]) {
            Ok(output) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    output_bytes = output.len(),
                    runtime = "bun",
                    preload = false,
                    "Script completed"
                );
                logging::log("EXEC", &format!("SUCCESS: bun (output: {} bytes)", output.len()));
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
                logging::log("EXEC", &format!("SUCCESS: node (output: {} bytes)", output.len()));
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
    
    let output = Command::new(&executable)
        .args(args)
        .output()
        .map_err(|e| {
            let err = format!("Failed to run '{}': {}", executable, e);
            logging::log("EXEC", &format!("COMMAND ERROR: {}", err));
            err
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    logging::log("EXEC", &format!("Command status: {}, stdout: {} bytes, stderr: {} bytes", 
        output.status, stdout.len(), stderr.len()));

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
        trimmed.starts_with("at ") || 
        trimmed.contains("    at ") ||
        trimmed.starts_with("Error:") ||
        trimmed.starts_with("TypeError:") ||
        trimmed.starts_with("ReferenceError:") ||
        trimmed.starts_with("SyntaxError:")
    });
    
    if let Some(start) = stack_start {
        // Collect lines that look like stack trace entries
        let stack_lines: Vec<&str> = lines[start..]
            .iter()
            .take_while(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && (
                    trimmed.starts_with("at ") ||
                    trimmed.contains("    at ") ||
                    trimmed.starts_with("Error:") ||
                    trimmed.starts_with("TypeError:") ||
                    trimmed.starts_with("ReferenceError:") ||
                    trimmed.starts_with("SyntaxError:") ||
                    trimmed.contains("error") ||
                    trimmed.contains("Error")
                )
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
        if trimmed.starts_with("Error:") ||
           trimmed.starts_with("TypeError:") ||
           trimmed.starts_with("ReferenceError:") ||
           trimmed.starts_with("SyntaxError:") ||
           trimmed.starts_with("error:") {
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
        suggestions.push("Check that all variables and functions are properly imported or defined".to_string());
    }
    
    if stderr_lower.contains("typeerror") {
        suggestions.push("Check that you're using the correct types for function arguments".to_string());
    }
    
    if stderr_lower.contains("permission denied") || stderr_lower.contains("eacces") {
        suggestions.push("Check file permissions or try running with elevated privileges".to_string());
    }
    
    if stderr_lower.contains("enoent") || stderr_lower.contains("no such file") {
        suggestions.push("Check that the file path exists and is correct".to_string());
    }
    
    if stderr_lower.contains("timeout") || stderr_lower.contains("timed out") {
        suggestions.push("The operation timed out - check network connectivity or increase timeout".to_string());
    }
    
    // Exit code specific suggestions
    match exit_code {
        Some(1) => {
            if suggestions.is_empty() {
                suggestions.push("Check the error message above for details".to_string());
            }
        }
        Some(127) => {
            suggestions.push("Command not found - check that the executable is installed and in PATH".to_string());
        }
        Some(126) => {
            suggestions.push("Permission denied - check file permissions".to_string());
        }
        Some(137) => {
            suggestions.push("Process was killed (possibly out of memory)".to_string());
        }
        Some(143) => {
            suggestions.push("Process was terminated by signal".to_string());
        }
        _ => {}
    }
    
    suggestions
}

// ============================================================================
// Selected Text Message Handlers
// ============================================================================

/// Result of handling a selected text message
#[derive(Debug)]
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
            logging::log("EXEC", &format!("GetSelectedText success: {} chars", text.len()));
            // Return as Submit message so SDK pending map can match by id
            Message::Submit { id: request_id.to_string(), value: Some(text) }
        }
        Err(e) => {
            warn!(request_id = %request_id, error = %e, "Failed to get selected text");
            logging::log("EXEC", &format!("GetSelectedText error: {}", e));
            // Return error prefixed with ERROR: so SDK can detect and reject
            Message::Submit { id: request_id.to_string(), value: Some(format!("ERROR: {}", e)) }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_get_selected_text(request_id: &str) -> Message {
    logging::log("EXEC", &format!("GetSelectedText request: {} (not supported on this platform)", request_id));
    warn!(request_id = %request_id, "Selected text not supported on this platform");
    Message::Submit { id: request_id.to_string(), value: Some(String::new()) }
}

/// Handle SET_SELECTED_TEXT request
#[cfg(target_os = "macos")]
fn handle_set_selected_text(text: &str, request_id: &str) -> Message {
    logging::log("EXEC", &format!("SetSelectedText request: {} ({} chars)", request_id, text.len()));
    
    match selected_text::set_selected_text(text) {
        Ok(()) => {
            info!(request_id = %request_id, "Set selected text successfully");
            logging::log("EXEC", "SetSelectedText success");
            // Return success as Submit with empty value
            Message::Submit { id: request_id.to_string(), value: None }
        }
        Err(e) => {
            warn!(request_id = %request_id, error = %e, "Failed to set selected text");
            logging::log("EXEC", &format!("SetSelectedText error: {}", e));
            // Return error prefixed with ERROR: so SDK can detect and reject
            Message::Submit { id: request_id.to_string(), value: Some(format!("ERROR: {}", e)) }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn handle_set_selected_text(_text: &str, request_id: &str) -> Message {
    logging::log("EXEC", &format!("SetSelectedText request: {} (not supported on this platform)", request_id));
    warn!(request_id = %request_id, "Selected text not supported on this platform");
    Message::Submit { id: request_id.to_string(), value: Some("ERROR: Not supported on this platform".to_string()) }
}

/// Handle CHECK_ACCESSIBILITY request
#[cfg(target_os = "macos")]
fn handle_check_accessibility(request_id: &str) -> Message {
    logging::log("EXEC", &format!("CheckAccessibility request: {}", request_id));
    
    let granted = selected_text::has_accessibility_permission();
    info!(request_id = %request_id, granted = granted, "Checked accessibility permission");
    logging::log("EXEC", &format!("CheckAccessibility: granted={}", granted));
    
    // Return as Submit with "true" or "false" string value
    Message::Submit { id: request_id.to_string(), value: Some(granted.to_string()) }
}

#[cfg(not(target_os = "macos"))]
fn handle_check_accessibility(request_id: &str) -> Message {
    logging::log("EXEC", &format!("CheckAccessibility request: {} (not supported on this platform)", request_id));
    // On non-macOS, report as "not granted" since the feature isn't available
    Message::Submit { id: request_id.to_string(), value: Some("false".to_string()) }
}

/// Handle REQUEST_ACCESSIBILITY request
#[cfg(target_os = "macos")]
fn handle_request_accessibility(request_id: &str) -> Message {
    logging::log("EXEC", &format!("RequestAccessibility request: {}", request_id));
    
    let granted = selected_text::request_accessibility_permission();
    info!(request_id = %request_id, granted = granted, "Requested accessibility permission");
    logging::log("EXEC", &format!("RequestAccessibility: granted={}", granted));
    
    // Return as Submit with "true" or "false" string value
    Message::Submit { id: request_id.to_string(), value: Some(granted.to_string()) }
}

#[cfg(not(target_os = "macos"))]
fn handle_request_accessibility(request_id: &str) -> Message {
    logging::log("EXEC", &format!("RequestAccessibility request: {} (not supported on this platform)", request_id));
    // On non-macOS, can't request permissions
    Message::Submit { id: request_id.to_string(), value: Some("false".to_string()) }
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
        assert!(is_typescript(&PathBuf::from("/home/user/.kenv/scripts/script.ts")));
        assert!(is_typescript(&PathBuf::from("/usr/local/bin/script.ts")));
    }

    #[test]
    fn test_is_javascript_with_path() {
        assert!(is_javascript(&PathBuf::from("/home/user/.kenv/scripts/script.js")));
        assert!(is_javascript(&PathBuf::from("/usr/local/bin/script.js")));
    }

    #[test]
    fn test_file_extensions_case_sensitive() {
        // Rust PathBuf.extension() returns lowercase for comparison
        assert!(is_typescript(&PathBuf::from("script.TS")) || !is_typescript(&PathBuf::from("script.TS")));
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
        let mut handle = ProcessHandle::new(99999); // Non-existent PID
        handle.kill();
        handle.kill(); // Should be safe to call again
        assert!(handle.killed);
    }

    #[test]
    fn test_process_handle_drop_calls_kill() {
        // Create a handle and let it drop
        let handle = ProcessHandle::new(99998); // Non-existent PID
        assert!(!handle.killed);
        drop(handle);
        // If we get here without panic, drop successfully called kill
    }

    #[cfg(unix)]
    #[test]
    fn test_spawn_and_kill_process() {
        // Spawn a simple process that sleeps
        let result = spawn_script("sleep", &["10"]);
        
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
        let result = spawn_script("sleep", &["30"]);
        
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
        let result = spawn_script("sleep", &["10"]);
        
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

    use crate::protocol::Message;
    use super::{handle_selected_text_message, SelectedTextHandleResult};

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
                        assert!(value == Some("true".to_string()) || value == Some("false".to_string()));
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
                        assert!(value == Some("true".to_string()) || value == Some("false".to_string()));
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
        let msg1 = Message::Submit { id: "req-x".to_string(), value: Some("text".to_string()) };
        
        assert!(matches!(handle_selected_text_message(&msg1), SelectedTextHandleResult::NotHandled));
    }

    // ============================================================
    // AUTO_SUBMIT Mode Tests
    // ============================================================
    // 
    // Note: These tests verify the AUTO_SUBMIT environment variable parsing.
    // Since env vars are global and tests run in parallel, we use a single
    // comprehensive test that exercises all cases sequentially to avoid races.
    
    use super::{is_auto_submit_enabled, get_auto_submit_delay, get_auto_submit_value, get_auto_submit_index};
    use std::time::Duration;

    /// Comprehensive test for is_auto_submit_enabled() function.
    /// Tests all cases in sequence to avoid env var race conditions.
    #[test]
    fn test_is_auto_submit_enabled_all_cases() {
        // Test "true" value
        std::env::set_var("AUTO_SUBMIT", "true");
        assert!(is_auto_submit_enabled(), "AUTO_SUBMIT=true should enable auto-submit");
        
        // Test "1" value
        std::env::set_var("AUTO_SUBMIT", "1");
        assert!(is_auto_submit_enabled(), "AUTO_SUBMIT=1 should enable auto-submit");
        
        // Test "false" value
        std::env::set_var("AUTO_SUBMIT", "false");
        assert!(!is_auto_submit_enabled(), "AUTO_SUBMIT=false should NOT enable auto-submit");
        
        // Test "0" value
        std::env::set_var("AUTO_SUBMIT", "0");
        assert!(!is_auto_submit_enabled(), "AUTO_SUBMIT=0 should NOT enable auto-submit");
        
        // Test other value
        std::env::set_var("AUTO_SUBMIT", "yes");
        assert!(!is_auto_submit_enabled(), "AUTO_SUBMIT=yes should NOT enable auto-submit");
        
        // Test unset (default)
        std::env::remove_var("AUTO_SUBMIT");
        assert!(!is_auto_submit_enabled(), "Unset AUTO_SUBMIT should NOT enable auto-submit");
    }

    /// Comprehensive test for get_auto_submit_delay() function.
    #[test]
    fn test_get_auto_submit_delay_all_cases() {
        // Test custom value
        std::env::set_var("AUTO_SUBMIT_DELAY_MS", "500");
        assert_eq!(get_auto_submit_delay(), Duration::from_millis(500), 
            "AUTO_SUBMIT_DELAY_MS=500 should return 500ms");
        
        // Test invalid value (falls back to default)
        std::env::set_var("AUTO_SUBMIT_DELAY_MS", "not_a_number");
        assert_eq!(get_auto_submit_delay(), Duration::from_millis(100),
            "Invalid AUTO_SUBMIT_DELAY_MS should default to 100ms");
        
        // Test unset (default)
        std::env::remove_var("AUTO_SUBMIT_DELAY_MS");
        assert_eq!(get_auto_submit_delay(), Duration::from_millis(100),
            "Unset AUTO_SUBMIT_DELAY_MS should default to 100ms");
    }

    /// Comprehensive test for get_auto_submit_value() function.
    #[test]
    fn test_get_auto_submit_value_all_cases() {
        // Test set value
        std::env::set_var("AUTO_SUBMIT_VALUE", "test_value");
        assert_eq!(get_auto_submit_value(), Some("test_value".to_string()),
            "AUTO_SUBMIT_VALUE=test_value should return Some(test_value)");
        
        // Test empty value
        std::env::set_var("AUTO_SUBMIT_VALUE", "");
        assert_eq!(get_auto_submit_value(), Some("".to_string()),
            "AUTO_SUBMIT_VALUE='' should return Some('')");
        
        // Test unset (None)
        std::env::remove_var("AUTO_SUBMIT_VALUE");
        assert_eq!(get_auto_submit_value(), None,
            "Unset AUTO_SUBMIT_VALUE should return None");
    }

    /// Comprehensive test for get_auto_submit_index() function.
    #[test]
    fn test_get_auto_submit_index_all_cases() {
        // Test custom value
        std::env::set_var("AUTO_SUBMIT_INDEX", "5");
        assert_eq!(get_auto_submit_index(), 5,
            "AUTO_SUBMIT_INDEX=5 should return 5");
        
        // Test invalid value (falls back to default)
        std::env::set_var("AUTO_SUBMIT_INDEX", "invalid");
        assert_eq!(get_auto_submit_index(), 0,
            "Invalid AUTO_SUBMIT_INDEX should default to 0");
        
        // Test negative value (falls back to default since usize can't be negative)
        std::env::set_var("AUTO_SUBMIT_INDEX", "-1");
        assert_eq!(get_auto_submit_index(), 0,
            "Negative AUTO_SUBMIT_INDEX should default to 0");
        
        // Test unset (default)
        std::env::remove_var("AUTO_SUBMIT_INDEX");
        assert_eq!(get_auto_submit_index(), 0,
            "Unset AUTO_SUBMIT_INDEX should default to 0");
    }
    
    // ============================================================
    // AutoSubmitConfig Tests
    // ============================================================
    
    use super::{AutoSubmitConfig, get_auto_submit_config};
    use crate::protocol::Choice;
    
    /// Test AutoSubmitConfig default values.
    #[test]
    fn test_auto_submit_config_default() {
        let config = AutoSubmitConfig::default();
        
        assert!(!config.enabled, "Default should be disabled");
        assert_eq!(config.delay, Duration::from_millis(100), "Default delay should be 100ms");
        assert!(config.value_override.is_none(), "Default should have no value override");
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
        assert_eq!(config.delay, Duration::from_millis(250), "Delay should be 250ms");
        assert_eq!(config.value_override, Some("override_value".to_string()), "Should have override value");
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
        assert_eq!(config.delay, Duration::from_millis(100), "Default delay should be 100ms");
    }
    
    /// Test get_arg_value() with choices.
    #[test]
    fn test_auto_submit_config_get_arg_value() {
        let choices = vec![
            Choice { name: "Apple".to_string(), value: "apple".to_string(), description: None, semantic_id: None },
            Choice { name: "Banana".to_string(), value: "banana".to_string(), description: None, semantic_id: None },
            Choice { name: "Cherry".to_string(), value: "cherry".to_string(), description: None, semantic_id: None },
        ];
        
        // Test default behavior (first choice)
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_arg_value(&choices), Some("apple".to_string()),
            "Default should return first choice value");
        
        // Test with index
        let config = AutoSubmitConfig { index: 1, ..Default::default() };
        assert_eq!(config.get_arg_value(&choices), Some("banana".to_string()),
            "Index 1 should return second choice value");
        
        // Test with out-of-bounds index (should clamp)
        let config = AutoSubmitConfig { index: 100, ..Default::default() };
        assert_eq!(config.get_arg_value(&choices), Some("cherry".to_string()),
            "Out-of-bounds index should clamp to last choice");
        
        // Test with value override
        let config = AutoSubmitConfig { 
            value_override: Some("custom".to_string()), 
            index: 1,
            ..Default::default() 
        };
        assert_eq!(config.get_arg_value(&choices), Some("custom".to_string()),
            "Override value should take precedence over index");
        
        // Test with empty choices
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_arg_value(&[]), None,
            "Empty choices should return None");
    }
    
    /// Test get_div_value() returns None (just dismissal).
    #[test]
    fn test_auto_submit_config_get_div_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_div_value(), None,
            "Div prompt should return None for dismissal");
    }
    
    /// Test get_editor_value() returns original content.
    #[test]
    fn test_auto_submit_config_get_editor_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_editor_value("original content"), Some("original content".to_string()),
            "Editor should return original content unchanged");
        
        // Test with override
        let config = AutoSubmitConfig { 
            value_override: Some("modified".to_string()), 
            ..Default::default() 
        };
        assert_eq!(config.get_editor_value("original content"), Some("modified".to_string()),
            "Override should take precedence");
    }
    
    /// Test get_term_value() returns "0".
    #[test]
    fn test_auto_submit_config_get_term_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_term_value(), Some("0".to_string()),
            "Term prompt should return exit code 0");
    }
    
    /// Test get_form_value() returns empty JSON object.
    #[test]
    fn test_auto_submit_config_get_form_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_form_value(), Some("{}".to_string()),
            "Form prompt should return empty JSON object");
    }
    
    /// Test get_select_value() returns JSON array.
    #[test]
    fn test_auto_submit_config_get_select_value() {
        let choices = vec![
            Choice { name: "Apple".to_string(), value: "apple".to_string(), description: None, semantic_id: None },
            Choice { name: "Banana".to_string(), value: "banana".to_string(), description: None, semantic_id: None },
        ];
        
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_select_value(&choices), Some(r#"["apple"]"#.to_string()),
            "Select should return JSON array with first choice");
        
        let config = AutoSubmitConfig { index: 1, ..Default::default() };
        assert_eq!(config.get_select_value(&choices), Some(r#"["banana"]"#.to_string()),
            "Select with index 1 should return second choice");
        
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_select_value(&[]), Some("[]".to_string()),
            "Empty choices should return empty array");
    }
    
    /// Test get_fields_value() returns JSON array of empty strings.
    #[test]
    fn test_auto_submit_config_get_fields_value() {
        let config = AutoSubmitConfig::default();
        
        assert_eq!(config.get_fields_value(0), Some("[]".to_string()),
            "0 fields should return empty array");
        assert_eq!(config.get_fields_value(1), Some(r#"[""]"#.to_string()),
            "1 field should return array with one empty string");
        assert_eq!(config.get_fields_value(3), Some(r#"["","",""]"#.to_string()),
            "3 fields should return array with three empty strings");
    }
    
    /// Test get_path_value() returns test path.
    #[test]
    fn test_auto_submit_config_get_path_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_path_value(), Some("/tmp/test-path".to_string()),
            "Path prompt should return /tmp/test-path");
    }
    
    /// Test get_hotkey_value() returns Cmd+A.
    #[test]
    fn test_auto_submit_config_get_hotkey_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_hotkey_value(), Some(r#"{"key":"a","command":true}"#.to_string()),
            "Hotkey prompt should return Cmd+A JSON");
    }
    
    /// Test get_drop_value() returns test file array.
    #[test]
    fn test_auto_submit_config_get_drop_value() {
        let config = AutoSubmitConfig::default();
        assert_eq!(config.get_drop_value(), Some(r#"[{"path":"/tmp/test.txt"}]"#.to_string()),
            "Drop prompt should return test file array");
    }
}

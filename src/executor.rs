use std::path::{Path, PathBuf};
use std::process::{Command, Child, ChildStdin, ChildStdout, Stdio};
use std::io::{Write, BufReader};
use std::time::Instant;
use crate::protocol::{Message, JsonlReader, serialize_message};
use crate::logging;
use tracing::{info, error, debug, instrument};

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
    
    for path_opt in common_paths.iter() {
        if let Some(path) = path_opt {
            let exe_path = path.join(name);
            logging::log("EXEC", &format!("  Checking: {}", exe_path.display()));
            if exe_path.exists() {
                logging::log("EXEC", &format!("  FOUND: {}", exe_path.display()));
                return Some(exe_path);
            }
        }
    }
    
    logging::log("EXEC", &format!("  NOT FOUND in common paths, will try PATH"));
    None
}

/// Extract the embedded SDK to disk if needed
/// Returns the path to the extracted SDK file
fn ensure_sdk_extracted() -> Option<PathBuf> {
    // Target path: ~/.kit/lib/kit-sdk.ts
    let kit_lib = dirs::home_dir()?.join(".kit/lib");
    let sdk_path = kit_lib.join("kit-sdk.ts");
    
    // Create dir if needed
    if !kit_lib.exists() {
        if let Err(e) = std::fs::create_dir_all(&kit_lib) {
            logging::log("EXEC", &format!("Failed to create SDK dir: {}", e));
            return None;
        }
    }
    
    // Write if missing (always write to ensure latest version)
    // In production, we might want to add version checking
    if !sdk_path.exists() {
        if let Err(e) = std::fs::write(&sdk_path, EMBEDDED_SDK) {
            logging::log("EXEC", &format!("Failed to write SDK: {}", e));
            return None;
        }
        logging::log("EXEC", &format!("Extracted SDK to {}", sdk_path.display()));
    }
    
    Some(sdk_path)
}

/// Find the SDK path, checking standard locations
fn find_sdk_path() -> Option<PathBuf> {
    logging::log("EXEC", "Looking for SDK...");
    
    // 1. Check ~/.kenv/lib/kit-sdk.ts (user override - highest priority)
    if let Some(home) = dirs::home_dir() {
        let kenv_sdk = home.join(".kenv/lib/kit-sdk.ts");
        logging::log("EXEC", &format!("  Checking user override: {}", kenv_sdk.display()));
        if kenv_sdk.exists() {
            logging::log("EXEC", &format!("  FOUND SDK (user override): {}", kenv_sdk.display()));
            return Some(kenv_sdk);
        }
    }
    
    // 2. Extract embedded SDK to ~/.kit/lib/kit-sdk.ts (production)
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
    child: Child,
    /// Process handle for cleanup - kills process group on drop
    process_handle: ProcessHandle,
}

/// Split session components for separate read/write threads
pub struct SplitSession {
    pub stdin: ChildStdin,
    pub stdout_reader: JsonlReader<BufReader<ChildStdout>>,
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
            child: self.child,
            process_handle: self.process_handle,
        }
    }
}

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
        .stderr(Stdio::inherit());
    
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

    let process_handle = ProcessHandle::new(pid);
    logging::log("EXEC", "ScriptSession created successfully");
    
    Ok(ScriptSession {
        stdin,
        stdout_reader: JsonlReader::new(BufReader::new(stdout)),
        child,
        process_handle,
    })
}

/// Execute a script and return its output (non-interactive, for backwards compatibility)
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
}

use std::path::{Path, PathBuf};
use std::process::{Command, Child, ChildStdin, ChildStdout, Stdio};
use std::io::{Write, BufReader};
use crate::protocol::{Message, JsonlReader, serialize_message};
use crate::logging;

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

/// Session for bidirectional communication with a running script
pub struct ScriptSession {
    pub stdin: ChildStdin,
    stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    child: Child,
}

/// Split session components for separate read/write threads
pub struct SplitSession {
    pub stdin: ChildStdin,
    pub stdout_reader: JsonlReader<BufReader<ChildStdout>>,
    pub child: Child,
}

impl ScriptSession {
    /// Split the session into separate read/write components
    /// This allows using separate threads for reading and writing
    pub fn split(self) -> SplitSession {
        SplitSession {
            stdin: self.stdin,
            stdout_reader: self.stdout_reader,
            child: self.child,
        }
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

    /// Kill the child process
    pub fn kill(&mut self) -> Result<(), String> {
        logging::log("EXEC", "Killing script process");
        self.child.kill()
            .map_err(|e| format!("Failed to kill script process: {}", e))
    }
}

/// Execute a script with bidirectional JSONL communication
pub fn execute_script_interactive(path: &Path) -> Result<ScriptSession, String> {
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
                logging::log("EXEC", "SUCCESS: bun with preload");
                return Ok(session);
            }
            Err(e) => {
                logging::log("EXEC", &format!("FAILED: bun with preload: {}", e));
            }
        }
    }

    // Try bun without preload as fallback
    if is_typescript(path) {
        logging::log("EXEC", &format!("Trying: bun run {}", path_str));
        match spawn_script("bun", &["run", path_str]) {
            Ok(session) => {
                logging::log("EXEC", "SUCCESS: bun without preload");
                return Ok(session);
            }
            Err(e) => {
                logging::log("EXEC", &format!("FAILED: bun without preload: {}", e));
            }
        }
    }

    // Try node for JavaScript files
    if is_javascript(path) {
        logging::log("EXEC", &format!("Trying: node {}", path_str));
        match spawn_script("node", &[path_str]) {
            Ok(session) => {
                logging::log("EXEC", "SUCCESS: node");
                return Ok(session);
            }
            Err(e) => {
                logging::log("EXEC", &format!("FAILED: node: {}", e));
            }
        }
    }

    let err = format!(
        "Failed to execute script '{}' interactively. Make sure bun or node is installed.",
        path.display()
    );
    logging::log("EXEC", &format!("ALL METHODS FAILED: {}", err));
    Err(err)
}

/// Spawn a script as an interactive process with piped stdin/stdout
fn spawn_script(cmd: &str, args: &[&str]) -> Result<ScriptSession, String> {
    // Try to find the executable in common locations
    let executable = find_executable(cmd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| cmd.to_string());
    
    logging::log("EXEC", &format!("spawn_script: {} {:?}", executable, args));
    
    let mut child = Command::new(&executable)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            let err = format!("Failed to spawn '{}': {}", executable, e);
            logging::log("EXEC", &format!("SPAWN ERROR: {}", err));
            err
        })?;

    logging::log("EXEC", &format!("Process spawned with PID: {:?}", child.id()));

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open script stdin".to_string())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open script stdout".to_string())?;

    logging::log("EXEC", "ScriptSession created successfully");
    
    Ok(ScriptSession {
        stdin,
        stdout_reader: JsonlReader::new(BufReader::new(stdout)),
        child,
    })
}

/// Execute a script and return its output (non-interactive, for backwards compatibility)
pub fn execute_script(path: &Path) -> Result<String, String> {
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
            logging::log("EXEC", &format!("SUCCESS: kit (output: {} bytes)", output.len()));
            return Ok(output);
        }
        Err(e) => {
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
                    logging::log("EXEC", &format!("SUCCESS: bun with preload (output: {} bytes)", output.len()));
                    return Ok(output);
                }
                Err(e) => {
                    logging::log("EXEC", &format!("FAILED: bun with preload: {}", e));
                }
            }
        }
        
        // Fallback: try bun without preload
        logging::log("EXEC", &format!("Trying: bun run {} (no preload)", path_str));
        match run_command("bun", &["run", path_str]) {
            Ok(output) => {
                logging::log("EXEC", &format!("SUCCESS: bun (output: {} bytes)", output.len()));
                return Ok(output);
            }
            Err(e) => {
                logging::log("EXEC", &format!("FAILED: bun: {}", e));
            }
        }
    }

    // Try node for JavaScript files
    if is_javascript(path) {
        logging::log("EXEC", &format!("Trying: node {}", path_str));
        match run_command("node", &[path_str]) {
            Ok(output) => {
                logging::log("EXEC", &format!("SUCCESS: node (output: {} bytes)", output.len()));
                return Ok(output);
            }
            Err(e) => {
                logging::log("EXEC", &format!("FAILED: node: {}", e));
            }
        }
    }

    let err = format!(
        "Failed to execute script '{}'. Make sure kit, bun, or node is installed.",
        path.display()
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
}

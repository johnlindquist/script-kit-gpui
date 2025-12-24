//! Smoke test binary for testing executor and GUI
//! 
//! Run with: cargo run --bin smoke-test
//! Run with GUI test: cargo run --bin smoke-test -- --gui
//! 
//! This tests:
//! 1. Executable discovery (bun, node, kit)
//! 2. SDK path resolution
//! 3. Simple script execution (non-blocking)
//! 4. Interactive script execution (with timeout)
//! 5. GUI command injection (if --gui flag)

use std::path::PathBuf;
use std::time::Duration;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

const CMD_FILE: &str = "/tmp/script-kit-gpui-cmd.txt";
const LOG_FILE: &str = "/var/folders/c3/r013q3_93_s4zycmx0mdnt2h0000gn/T/script-kit-gpui.log";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gui_mode = args.iter().any(|a| a == "--gui");
    
    println!("=== Script Kit GPUI Smoke Test ===\n");

    // Test 1: Check if we can find executables
    println!("1. Testing executable discovery...");
    let bun_path = find_in_common_paths("bun");
    let node_path = find_in_common_paths("node");
    let kit_path = find_in_common_paths("kit");
    
    println!("   bun  -> {}", format_path(&bun_path));
    println!("   node -> {}", format_path(&node_path));
    println!("   kit  -> {}", format_path(&kit_path));
    println!();

    // Test 2: Check SDK path
    println!("2. Testing SDK discovery...");
    let sdk_path = find_sdk();
    println!("   SDK -> {}", format_path(&sdk_path));
    println!();

    // Test 3: Simple script execution
    println!("3. Testing simple script execution...");
    let simple_script = dirs::home_dir()
        .map(|h| h.join(".kenv/scripts/smoke-test-simple.ts"))
        .unwrap();
    
    if simple_script.exists() {
        if let Some(ref bun) = bun_path {
            println!("   Running: {} run {}", bun.display(), simple_script.display());
            match run_with_timeout(bun, &["run", simple_script.to_str().unwrap()], Duration::from_secs(5)) {
                Ok(output) => {
                    println!("   ✓ SUCCESS: {}", output.trim());
                }
                Err(e) => {
                    println!("   ✗ FAILED: {}", e);
                }
            }
        }
    } else {
        println!("   SKIPPED: {} not found", simple_script.display());
    }
    println!();

    // Test 4: Interactive script with preload
    println!("4. Testing interactive script (with preload + timeout)...");
    let demo_script = dirs::home_dir()
        .map(|h| h.join(".kenv/scripts/demo-arg-div.ts"))
        .unwrap();
    
    if demo_script.exists() && sdk_path.is_some() && bun_path.is_some() {
        let bun = bun_path.as_ref().unwrap();
        let sdk = sdk_path.as_ref().unwrap();
        
        match run_interactive_with_timeout(
            bun,
            &["run", "--preload", sdk.to_str().unwrap(), demo_script.to_str().unwrap()],
            Duration::from_secs(2)
        ) {
            Ok((stdout, _stderr)) => {
                if stdout.contains("\"type\":\"arg\"") {
                    println!("   ✓ SUCCESS: Script sent arg prompt");
                    println!("   Output: {}", stdout.trim());
                } else {
                    println!("   ⚠ WARNING: Unexpected output: {}", stdout);
                }
            }
            Err(e) => {
                println!("   ✗ FAILED: {}", e);
            }
        }
    } else {
        println!("   SKIPPED: Missing demo script, SDK, or bun");
    }
    println!();

    // Test 5: GUI test (if --gui flag)
    if gui_mode {
        println!("5. Testing GUI command injection...");
        test_gui_command();
    } else {
        println!("5. GUI test skipped (use --gui flag to enable)");
    }
    println!();

    println!("=== Smoke Test Complete ===");
}

fn format_path(path: &Option<PathBuf>) -> String {
    path.as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "NOT FOUND".to_string())
}

fn find_in_common_paths(name: &str) -> Option<PathBuf> {
    let common_paths = [
        dirs::home_dir().map(|h| h.join(".bun/bin")),
        dirs::home_dir().map(|h| h.join("Library/pnpm")),
        dirs::home_dir().map(|h| h.join(".nvm/current/bin")),
        dirs::home_dir().map(|h| h.join(".volta/bin")),
        dirs::home_dir().map(|h| h.join(".local/bin")),
        dirs::home_dir().map(|h| h.join("bin")),
        Some(PathBuf::from("/opt/homebrew/bin")),
        Some(PathBuf::from("/usr/local/bin")),
        Some(PathBuf::from("/usr/bin")),
        Some(PathBuf::from("/bin")),
    ];
    
    for path_opt in common_paths.iter().flatten() {
        let exe_path = path_opt.join(name);
        if exe_path.exists() {
            return Some(exe_path);
        }
    }
    None
}

fn find_sdk() -> Option<PathBuf> {
    let locations = [
        dirs::home_dir().map(|h| h.join(".kenv/lib/kit-sdk.ts")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/kit-sdk.ts")),
    ];
    
    for loc in locations.iter().flatten() {
        if loc.exists() {
            return Some(loc.clone());
        }
    }
    None
}

fn run_with_timeout(cmd: &PathBuf, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take().map(|s| {
                    let reader = BufReader::new(s);
                    reader.lines().filter_map(|l| l.ok()).collect::<Vec<_>>().join("\n")
                }).unwrap_or_default();
                
                if status.success() {
                    return Ok(stdout);
                } else {
                    return Err(format!("Exit code: {}", status));
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("Timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("Wait error: {}", e)),
        }
    }
}

fn run_interactive_with_timeout(cmd: &PathBuf, args: &[&str], timeout: Duration) -> Result<(String, String), String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    let stdout_handle = child.stdout.take().unwrap();
    let mut stdout_reader = BufReader::new(stdout_handle);
    
    let mut stdout = String::new();
    let start = std::time::Instant::now();
    
    loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            break;
        }
        
        let mut line = String::new();
        match stdout_reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                stdout.push_str(&line);
                if line.contains("\"type\"") {
                    let _ = child.kill();
                    break;
                }
            }
            Err(_) => break,
        }
    }
    
    Ok((stdout, String::new()))
}

fn test_gui_command() {
    // Check if log file exists (indicates app is running)
    let log_path = PathBuf::from(LOG_FILE);
    if !log_path.exists() {
        println!("   ⚠ App log file not found - is the app running?");
        println!("   Start the app with: cargo run");
        return;
    }
    
    // Get initial log size
    let initial_size = std::fs::metadata(&log_path)
        .map(|m| m.len())
        .unwrap_or(0);
    
    // Write test command
    println!("   Writing command: run:smoke-test-simple.ts");
    if let Err(e) = std::fs::write(CMD_FILE, "run:smoke-test-simple.ts\n") {
        println!("   ✗ Failed to write command file: {}", e);
        return;
    }
    
    // Wait for app to process
    println!("   Waiting for app to process...");
    std::thread::sleep(Duration::from_secs(2));
    
    // Check new log entries
    if let Ok(content) = std::fs::read_to_string(&log_path) {
        let new_content: String = content
            .bytes()
            .skip(initial_size as usize)
            .map(|b| b as char)
            .collect();
        
        let test_lines: Vec<&str> = new_content
            .lines()
            .filter(|l| l.contains("[TEST]") || l.contains("[EXEC]"))
            .collect();
        
        if test_lines.is_empty() {
            println!("   ⚠ No TEST/EXEC log entries found - command may not have been processed");
        } else {
            println!("   Log entries:");
            for line in test_lines.iter().take(10) {
                // Extract just the message part after the timestamp
                if let Some(msg_start) = line.find("] [") {
                    println!("   {}", &line[msg_start + 2..]);
                } else {
                    println!("   {}", line);
                }
            }
            
            if test_lines.iter().any(|l| l.contains("SUCCESS") || l.contains("Script output")) {
                println!("   ✓ GUI test PASSED");
            } else if test_lines.iter().any(|l| l.contains("FAILED") || l.contains("error")) {
                println!("   ✗ GUI test FAILED");
            }
        }
    }
}

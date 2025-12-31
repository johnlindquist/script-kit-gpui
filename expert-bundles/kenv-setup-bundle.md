# ~/.kenv Setup/Initialization Expert Bundle

## Executive Summary

Script Kit GPUI needs a unified setup/initialization process that ensures the `~/.kenv` environment exists and is properly configured when the application starts. Currently, initialization logic is scattered across multiple files (`executor.rs`, `logging.rs`, `build.rs`, `scripts.rs`) with no single entry point. The setup must be idempotent - creating missing directories/files without overwriting existing user settings.

### Key Problems:
1. **No unified setup function**: Initialization is scattered across `executor.rs::ensure_sdk_extracted()`, `logging.rs::init()`, `scripts.rs::read_scripts()`, etc.
2. **Missing directory creation**: The app expects `~/.kenv/scripts/` and `~/.kenv/scriptlets/` to exist but doesn't create them on first run.
3. **No template config creation**: Users must manually create `config.ts` - there's no starter template copied on first install.

### Required Fixes:
1. Create a new `src/setup.rs` module with `ensure_kenv_setup()` function called at app startup
2. Create all required directories: `scripts/`, `scriptlets/`, `sdk/`, `logs/`, `cache/app-icons/`
3. Copy template files if they don't exist: `config.ts` (from embedded template), optionally `theme.json`
4. Verify runtime dependencies (bun) and log warnings
5. Add sample scripts and scriptlets for new users (optional, can be minimal)

### Files Included:
- `src/executor.rs`: Current SDK extraction and tsconfig.json creation logic
- `src/config.rs`: Config loading and structure definitions
- `src/logging.rs`: Log directory creation
- `src/scripts.rs`: Script/scriptlet loading (shows expected directory structure)
- `src/main.rs`: Application entry point (where setup should be called)
- `scripts/config-template.ts`: Template config file for new users
- `build.rs`: Build-time SDK copying
- `theme.example.json`: Example theme file structure

---

## Source Files

### src/executor.rs (lines 259-404 - SDK extraction)

```rust
/// Embedded SDK content (included at compile time)
const EMBEDDED_SDK: &str = include_str!("../scripts/kit-sdk.ts");

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
```

### src/config.rs (key structures)

```rust
/// Configuration for built-in features (clipboard history, app launcher, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltInConfig {
    #[serde(default = "default_clipboard_history")]
    pub clipboard_history: bool,
    #[serde(default = "default_app_launcher")]
    pub app_launcher: bool,
    #[serde(default = "default_window_switcher")]
    pub window_switcher: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bun_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<ContentPadding>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "editorFontSize")]
    pub editor_font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "terminalFontSize")]
    pub terminal_font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "uiScale")]
    pub ui_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "builtIns")]
    pub built_ins: Option<BuiltInConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "processLimits")]
    pub process_limits: Option<ProcessLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "clipboardHistoryMaxTextLength")]
    pub clipboard_history_max_text_length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub modifiers: Vec<String>,
    pub key: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: HotkeyConfig {
                modifiers: vec!["meta".to_string()],
                key: "Semicolon".to_string(), // Cmd+; matches main.rs default
            },
            bun_path: None,
            editor: None,
            padding: None,
            editor_font_size: None,
            terminal_font_size: None,
            ui_scale: None,
            built_ins: None,
            process_limits: None,
            clipboard_history_max_text_length: None,
        }
    }
}

#[instrument(name = "load_config")]
pub fn load_config() -> Config {
    let config_path = PathBuf::from(shellexpand::tilde("~/.kenv/config.ts").as_ref());

    // Check if config file exists
    if !config_path.exists() {
        info!(path = %config_path.display(), "Config file not found, using defaults");
        return Config::default();
    }

    // Step 1: Transpile TypeScript to JavaScript using bun build
    let tmp_js_path = "/tmp/kit-config.js";
    let build_output = Command::new("bun")
        .arg("build")
        .arg("--target=bun")
        .arg(config_path.to_string_lossy().to_string())
        .arg(format!("--outfile={}", tmp_js_path))
        .output();

    // ... rest of config loading ...
}
```

### src/logging.rs (log directory creation)

```rust
/// Get the log directory path (~/.kenv/logs/)
fn get_log_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".kenv").join("logs"))
        .unwrap_or_else(|| std::env::temp_dir().join("script-kit-logs"))
}

pub fn init() -> LoggingGuard {
    // Initialize legacy log buffer for UI display
    let _ = LOG_BUFFER.set(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES)));

    // Check for AI compact log mode
    let ai_log_mode = std::env::var("SCRIPT_KIT_AI_LOG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Create log directory
    let log_dir = get_log_dir();
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("[LOGGING] Failed to create log directory: {}", e);
    }

    let log_path = log_dir.join("script-kit-gpui.jsonl");
    // ... rest of logging init ...
}
```

### src/scripts.rs (expected directory structure)

```rust
/// Reads scripts from ~/.kenv/scripts directory
pub fn read_scripts() -> Vec<Script> {
    let home = match env::var("HOME") {
        Ok(home_path) => PathBuf::from(home_path),
        Err(e) => {
            warn!(error = %e, "HOME environment variable not set, cannot read scripts");
            return vec![];
        }
    };

    let scripts_dir = home.join(".kenv/scripts");

    // Check if directory exists
    if !scripts_dir.exists() {
        debug!(path = %scripts_dir.display(), "Scripts directory does not exist");
        return vec![];
    }
    // ... rest of script loading
}

/// Load scriptlets from markdown files
pub fn load_scriptlets() -> Vec<Scriptlet> {
    let home = match env::var("HOME") {
        Ok(home_path) => PathBuf::from(home_path),
        Err(e) => {
            warn!(error = %e, "HOME environment variable not set, cannot load scriptlets");
            return vec![];
        }
    };

    // Glob patterns to search
    let patterns = [
        home.join(".kenv/scriptlets/*.md"),
        home.join(".kenv/kenvs/*/scriptlets/*.md"),
    ];
    // ... rest of scriptlet loading
}
```

### scripts/config-template.ts (template for new users)

```typescript
import type { Config } from "@johnlindquist/kit";

/**
 * Script Kit Configuration
 * ========================
 *
 * This file controls Script Kit's behavior, appearance, and built-in features.
 * It's loaded on startup from ~/.kenv/config.ts.
 */
export default {
  // ===========================================================================
  // REQUIRED: Global Hotkey
  // ===========================================================================
  hotkey: {
    // Modifier keys: "meta" (Cmd/Win), "ctrl", "alt" (Option), "shift"
    modifiers: ["meta"],
    // Main key (W3C key codes)
    key: "Semicolon", // Cmd+; on Mac, Win+; on Windows
  },

  // ===========================================================================
  // UI Settings (all optional)
  // ===========================================================================
  // editorFontSize: 14,
  // terminalFontSize: 14,
  // uiScale: 1.0,
  // padding: { top: 8, left: 12, right: 12 },

  // ===========================================================================
  // Editor Settings (optional)
  // ===========================================================================
  // editor: "code",

  // ===========================================================================
  // Built-in Features (optional)
  // ===========================================================================
  // builtIns: {
  //   clipboardHistory: true,
  //   appLauncher: true,
  //   windowSwitcher: true,
  // },

} satisfies Config;
```

### build.rs (build-time SDK copying)

```rust
use std::fs;
use std::path::PathBuf;

fn main() {
    // Ensure rebuild when SDK changes
    println!("cargo:rerun-if-changed=scripts/kit-sdk.ts");

    let sdk_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/kit-sdk.ts");

    if let Some(home) = dirs::home_dir() {
        // Copy SDK to ~/.kenv/sdk/ (the only location now)
        let dest_dir = home.join(".kenv/sdk");
        let sdk_dest = dest_dir.join("kit-sdk.ts");

        // Create directory if needed
        if !dest_dir.exists() {
            if let Err(e) = fs::create_dir_all(&dest_dir) {
                println!("cargo:warning=Failed to create {}: {}", dest_dir.display(), e);
                return;
            }
        }

        // Copy SDK
        match fs::copy(&sdk_src, &sdk_dest) {
            Ok(bytes) => {
                println!("cargo:warning=Copied SDK to {} ({} bytes)", sdk_dest.display(), bytes);
            }
            Err(e) => {
                println!("cargo:warning=Failed to copy SDK to {}: {}", sdk_dest.display(), e);
            }
        }
    }
}
```

### theme.example.json (theme structure)

```json
{
  "colors": {
    "background": {
      "main": 1980410,
      "title_bar": 2961712,
      "search_box": 3947580,
      "log_panel": 851213
    },
    "text": {
      "primary": 16777215,
      "secondary": 14737920,
      "tertiary": 10066329,
      "muted": 8421504,
      "dimmed": 6710886
    },
    "accent": {
      "selected": 31948
    },
    "ui": {
      "border": 4609607,
      "success": 65280
    }
  },
  "opacity": {
    "main": 0.85,
    "title_bar": 0.9,
    "search_box": 0.92,
    "log_panel": 0.8
  },
  "drop_shadow": {
    "enabled": true,
    "blur_radius": 20.0,
    "spread_radius": 0.0,
    "offset_x": 0.0,
    "offset_y": 8.0,
    "color": 0,
    "opacity": 0.25
  }
}
```

---

## Implementation Guide

### Step 1: Create new src/setup.rs module

```rust
// File: src/setup.rs
//! Kenv environment setup and initialization.
//!
//! This module provides a unified setup process for the ~/.kenv directory structure.
//! Called once at application startup to ensure all required directories and files exist.

use crate::logging;
use std::path::PathBuf;
use tracing::{info, warn, instrument};

/// Embedded config template (included at compile time)
const EMBEDDED_CONFIG_TEMPLATE: &str = include_str!("../scripts/config-template.ts");

/// Embedded SDK content (included at compile time)
const EMBEDDED_SDK: &str = include_str!("../scripts/kit-sdk.ts");

/// Result of setup process
#[derive(Debug)]
pub struct SetupResult {
    /// Whether this is a fresh install (kenv didn't exist before)
    pub is_fresh_install: bool,
    /// Path to the kenv directory
    pub kenv_path: PathBuf,
    /// Whether bun was found in PATH
    pub bun_available: bool,
    /// Any warnings encountered during setup
    pub warnings: Vec<String>,
}

/// Ensure the ~/.kenv environment is properly set up.
///
/// This function is idempotent - it will create missing directories and files
/// without overwriting existing user configurations.
///
/// # Directory Structure Created
/// ```text
/// ~/.kenv/
/// ├── scripts/           # User scripts (.ts, .js files)
/// ├── scriptlets/        # Markdown scriptlet files
/// ├── sdk/               # Runtime SDK (kit-sdk.ts)
/// ├── logs/              # Application logs
/// ├── cache/
/// │   └── app-icons/     # Cached application icons
/// ├── config.ts          # User configuration (created from template if missing)
/// ├── tsconfig.json      # TypeScript path mappings
/// └── .gitignore         # Ignore transient files
/// ```
///
/// # Returns
/// `SetupResult` with information about the setup process.
#[instrument(level = "info", name = "ensure_kenv_setup")]
pub fn ensure_kenv_setup() -> SetupResult {
    let mut warnings = Vec::new();
    
    // Get home directory
    let home_dir = match dirs::home_dir() {
        Some(h) => h,
        None => {
            warn!("Could not determine home directory");
            return SetupResult {
                is_fresh_install: false,
                kenv_path: PathBuf::new(),
                bun_available: false,
                warnings: vec!["Could not determine home directory".to_string()],
            };
        }
    };
    
    let kenv_dir = home_dir.join(".kenv");
    let is_fresh_install = !kenv_dir.exists();
    
    info!(
        kenv_path = %kenv_dir.display(),
        is_fresh_install = is_fresh_install,
        "Setting up kenv environment"
    );
    
    // Create directory structure
    let directories = [
        kenv_dir.join("scripts"),
        kenv_dir.join("scriptlets"),
        kenv_dir.join("sdk"),
        kenv_dir.join("logs"),
        kenv_dir.join("cache").join("app-icons"),
    ];
    
    for dir in &directories {
        if !dir.exists() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                let msg = format!("Failed to create directory {}: {}", dir.display(), e);
                warn!("{}", msg);
                warnings.push(msg);
            } else {
                info!(path = %dir.display(), "Created directory");
            }
        }
    }
    
    // Extract SDK (always - ensures latest version)
    let sdk_path = kenv_dir.join("sdk").join("kit-sdk.ts");
    if let Err(e) = std::fs::write(&sdk_path, EMBEDDED_SDK) {
        let msg = format!("Failed to write SDK: {}", e);
        warn!("{}", msg);
        warnings.push(msg);
    } else {
        info!(
            path = %sdk_path.display(),
            bytes = EMBEDDED_SDK.len(),
            "Extracted SDK"
        );
    }
    
    // Create config.ts from template if it doesn't exist
    let config_path = kenv_dir.join("config.ts");
    if !config_path.exists() {
        if let Err(e) = std::fs::write(&config_path, EMBEDDED_CONFIG_TEMPLATE) {
            let msg = format!("Failed to write config template: {}", e);
            warn!("{}", msg);
            warnings.push(msg);
        } else {
            info!(path = %config_path.display(), "Created config.ts from template");
        }
    }
    
    // Ensure tsconfig.json has path mappings
    ensure_tsconfig_paths(&kenv_dir.join("tsconfig.json"));
    
    // Write .gitignore (always - app-managed)
    let gitignore_path = kenv_dir.join(".gitignore");
    let gitignore_content = r#"# Script Kit managed files (regenerated on each start)
sdk/
logs/
cache/
clipboard-history.db
frecency.json
"#;
    if let Err(e) = std::fs::write(&gitignore_path, gitignore_content) {
        let msg = format!("Failed to write .gitignore: {}", e);
        warn!("{}", msg);
        warnings.push(msg);
    }
    
    // Check for bun availability
    let bun_available = check_bun_available();
    if !bun_available {
        let msg = "bun not found in PATH - scripts may not execute correctly. Install from https://bun.sh".to_string();
        warn!("{}", msg);
        warnings.push(msg);
    }
    
    // Create sample scripts for fresh installs
    if is_fresh_install {
        create_sample_scripts(&kenv_dir, &mut warnings);
    }
    
    info!(
        is_fresh_install = is_fresh_install,
        bun_available = bun_available,
        warning_count = warnings.len(),
        "Kenv setup complete"
    );
    
    SetupResult {
        is_fresh_install,
        kenv_path: kenv_dir,
        bun_available,
        warnings,
    }
}

/// Ensure tsconfig.json has the @johnlindquist/kit path mapping
fn ensure_tsconfig_paths(tsconfig_path: &PathBuf) {
    use serde_json::{json, Value};

    let kit_path = json!(["./sdk/kit-sdk.ts"]);

    let mut config: Value = if tsconfig_path.exists() {
        match std::fs::read_to_string(tsconfig_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| json!({})),
            Err(_) => json!({}),
        }
    } else {
        json!({})
    };

    if config.get("compilerOptions").is_none() {
        config["compilerOptions"] = json!({});
    }

    if config["compilerOptions"].get("paths").is_none() {
        config["compilerOptions"]["paths"] = json!({});
    }

    let current_kit_path = config["compilerOptions"]["paths"].get("@johnlindquist/kit");
    if current_kit_path == Some(&kit_path) {
        return;
    }

    config["compilerOptions"]["paths"]["@johnlindquist/kit"] = kit_path;

    if let Ok(json_str) = serde_json::to_string_pretty(&config) {
        if let Err(e) = std::fs::write(tsconfig_path, json_str) {
            warn!(error = %e, "Failed to write tsconfig.json");
        } else {
            info!("Updated tsconfig.json with @johnlindquist/kit path");
        }
    }
}

/// Check if bun is available in PATH
fn check_bun_available() -> bool {
    use std::process::Command;
    
    // Try common locations first
    let common_paths = [
        dirs::home_dir().map(|h| h.join(".bun/bin/bun")),
        Some(PathBuf::from("/opt/homebrew/bin/bun")),
        Some(PathBuf::from("/usr/local/bin/bun")),
    ];
    
    for path in common_paths.iter().flatten() {
        if path.exists() {
            return true;
        }
    }
    
    // Fall back to PATH lookup
    Command::new("bun")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create sample scripts for new installations
fn create_sample_scripts(kenv_dir: &PathBuf, warnings: &mut Vec<String>) {
    let scripts_dir = kenv_dir.join("scripts");
    
    // Sample hello-world script
    let hello_script = r#"// Name: Hello World
// Description: A simple greeting script

const name = await arg("What's your name?");
await div(`<h1 class="text-2xl p-4">Hello, ${name}! Welcome to Script Kit.</h1>`);
"#;
    
    let hello_path = scripts_dir.join("hello-world.ts");
    if !hello_path.exists() {
        if let Err(e) = std::fs::write(&hello_path, hello_script) {
            warnings.push(format!("Failed to create sample script: {}", e));
        } else {
            info!(path = %hello_path.display(), "Created sample script");
        }
    }
    
    // Sample scriptlet
    let scriptlets_dir = kenv_dir.join("scriptlets");
    let sample_scriptlet = r#"# My Scriptlets

## Current Date
<!-- shortcut: cmd d -->

```bash
date +"%Y-%m-%d"
```

## Open Downloads
<!-- shortcut: cmd shift d -->

```bash
open ~/Downloads
```
"#;
    
    let scriptlet_path = scriptlets_dir.join("getting-started.md");
    if !scriptlet_path.exists() {
        if let Err(e) = std::fs::write(&scriptlet_path, sample_scriptlet) {
            warnings.push(format!("Failed to create sample scriptlet: {}", e));
        } else {
            info!(path = %scriptlet_path.display(), "Created sample scriptlet");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_bun_available() {
        // This test just verifies the function doesn't panic
        let _ = check_bun_available();
    }
}
```

### Step 2: Add setup module to lib.rs

```rust
// File: src/lib.rs
// Add this line with the other module declarations:
pub mod setup;
```

### Step 3: Call setup at application startup in main.rs

```rust
// File: src/main.rs
// At the top with other imports:
use crate::setup::ensure_kenv_setup;

// In the main() function, BEFORE logging::init():
fn main() {
    // Ensure kenv environment is set up first
    // This must happen before logging::init() because logging creates ~/.kenv/logs/
    let setup_result = ensure_kenv_setup();
    
    // Initialize logging (creates ~/.kenv/logs/ if needed)
    let _guard = logging::init();
    
    // Log setup results
    if setup_result.is_fresh_install {
        tracing::info!("Fresh Script Kit installation detected - welcome!");
    }
    
    if !setup_result.bun_available {
        tracing::warn!("bun not found - install from https://bun.sh for script execution");
    }
    
    for warning in &setup_result.warnings {
        tracing::warn!(warning = %warning, "Setup warning");
    }
    
    // ... rest of main()
}
```

### Step 4: Remove duplicate SDK extraction from executor.rs

```rust
// File: src/executor.rs
// Remove or simplify ensure_sdk_extracted() since setup.rs now handles it

/// Find the SDK path, checking standard locations
fn find_sdk_path() -> Option<PathBuf> {
    // Primary location - always check here first (setup.rs ensures it exists)
    if let Some(home) = dirs::home_dir() {
        let kenv_sdk = home.join(".kenv/sdk/kit-sdk.ts");
        if kenv_sdk.exists() {
            return Some(kenv_sdk);
        }
    }

    // Development fallback
    let dev_sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/kit-sdk.ts");
    if dev_sdk.exists() {
        return Some(dev_sdk);
    }

    None
}
```

### Testing

To verify the setup works correctly:

```bash
# 1. Remove existing kenv (backup first!)
mv ~/.kenv ~/.kenv.backup

# 2. Build and run the app
cargo build && echo '{"type":"show"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# 3. Check that directories were created
ls -la ~/.kenv/

# Expected output:
# drwxr-xr-x  scripts/
# drwxr-xr-x  scriptlets/
# drwxr-xr-x  sdk/
# drwxr-xr-x  logs/
# drwxr-xr-x  cache/
# -rw-r--r--  config.ts
# -rw-r--r--  tsconfig.json
# -rw-r--r--  .gitignore

# 4. Verify sample scripts exist
ls ~/.kenv/scripts/
# hello-world.ts

# 5. Verify config.ts has content
head ~/.kenv/config.ts

# 6. Restore backup if needed
rm -rf ~/.kenv && mv ~/.kenv.backup ~/.kenv
```

---

## Instructions For The Next AI Agent

You are reading the "~/.kenv Setup/Initialization Expert Bundle". This file is self-contained and includes all the context you should assume you have.

Your job:

* Design and describe the minimal, safe changes needed to fully resolve the issues described in the Executive Summary and Key Problems.
* Operate **only** on the files and code snippets included in this bundle. If you need additional files or context, clearly say so.

When you propose changes, follow these rules strictly:

1. Always provide **precise code snippets** that can be copy-pasted directly into the repo.
2. Always include **exact file paths** (e.g. `src/setup.rs`) and, when possible, line numbers or a clear description of the location.
3. Never describe code changes only in prose. Show the full function or block as it should look **after** the change, or show both "before" and "after" versions.
4. Keep instructions **unmistakable and unambiguous**. A human or tool following your instructions should not need to guess what to do.
5. Assume you cannot see any files outside this bundle. If you must rely on unknown code, explicitly note assumptions and risks.

Key implementation notes:
- The setup must be **idempotent** - running it multiple times should not corrupt existing data
- User files (config.ts, scripts/, scriptlets/) should NEVER be overwritten if they exist
- App-managed files (sdk/, .gitignore, tsconfig.json paths) CAN be overwritten to ensure latest version
- The setup should complete quickly (<100ms) since it runs on every app start
- Use `tracing` for all logging, not the legacy `logging::log()` function

When you answer, you do not need to restate this bundle. Work directly with the code and instructions it contains and return a clear, step-by-step plan plus exact code edits.

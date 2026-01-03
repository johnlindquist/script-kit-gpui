//! Script Kit environment setup and initialization.
//!
//! Ensures ~/.sk/kit exists with required directories and starter files.
//! The path can be overridden via the SK_PATH environment variable.
//! Idempotent: user-owned files are never overwritten; app-owned files may be refreshed.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info, instrument, warn};

/// Embedded config template (included at compile time)
const EMBEDDED_CONFIG_TEMPLATE: &str = include_str!("../scripts/config-template.ts");

/// Embedded SDK content (included at compile time)
const EMBEDDED_SDK: &str = include_str!("../scripts/kit-sdk.ts");

/// Optional theme example (included at compile time)
const EMBEDDED_THEME_EXAMPLE: &str = include_str!("../theme.example.json");

/// Embedded package.json template for user's kit directory
/// The "type": "module" enables top-level await in all .ts scripts
const EMBEDDED_PACKAGE_JSON: &str = r#"{
  "name": "scriptkit-user",
  "type": "module",
  "private": true
}
"#;

/// Embedded GUIDE.md comprehensive user guide
const EMBEDDED_GUIDE_MD: &str = include_str!("../scripts/GUIDE.md");

/// Embedded AGENTS.md guide for AI agents writing user scripts
const EMBEDDED_AGENTS_MD: &str = r###"# Script Kit User Scripts Guide

This guide is for AI agents and developers writing scripts for Script Kit.
Script Kit is a productivity tool that runs TypeScript scripts with a rich UI.

---

## Quick Start

```typescript
import "@scriptkit/kit";

export const metadata = {
  name: "My Script",
  description: "Does something useful",
};

const choice = await arg("Pick an option", ["Option 1", "Option 2"]);
await div(`<h1>You chose: ${choice}</h1>`);
```

---

## Table of Contents

1. [Script Metadata](#script-metadata)
2. [SDK Import](#sdk-import)
3. [Core SDK Functions](#core-sdk-functions)
4. [Scriptlet Format](#scriptlet-format)
5. [Configuration (config.ts)](#configuration-configts)
6. [Testing Scripts](#testing-scripts)
7. [Examples](#examples)

---

## Script Metadata

Scripts use the `metadata` export for configuration. This is the **preferred format** over comment-based metadata.

### Basic Metadata

```typescript
import "@scriptkit/kit";

export const metadata = {
  name: "My Script",           // Display name in menu
  description: "What it does", // Shown below the name
  shortcut: "cmd shift m",     // Global hotkey (optional)
  alias: "ms",                 // Quick search alias (optional)
};
```

### All Metadata Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name in the Script Kit menu |
| `description` | string | Description shown below the name |
| `shortcut` | string | Global keyboard shortcut (e.g., "cmd shift k") |
| `alias` | string | Short alias for quick triggering |
| `author` | string | Script author |
| `enter` | string | Custom text for Enter/Submit button |
| `icon` | string | Icon name (e.g., "file", "terminal", "star") |
| `tags` | string[] | Tags for categorization and search |
| `hidden` | boolean | Hide from the main script list |
| `background` | boolean | Run without UI (background process) |
| `schedule` | string | Cron expression for scheduled execution |
| `watch` | string | File path pattern that triggers the script |

### Legacy Comment-Based Metadata (Deprecated)

Comment-based metadata is supported but **deprecated** for new scripts:

```typescript
// Name: My Script
// Description: What it does
// Shortcut: cmd shift m
```

**Why prefer `export const metadata`?**
- Full TypeScript type safety
- IDE autocomplete and error checking
- Access to more metadata fields
- Easier to read and maintain

---

## SDK Import

Import the Script Kit SDK to get global functions:

```typescript
import "@scriptkit/kit";
```

This import:
- Makes all SDK functions available globally (arg, div, editor, etc.)
- Provides TypeScript types for IDE support
- Is resolved via tsconfig.json path mapping

---

## Core SDK Functions

### Prompts

#### `arg()` - Text Input with Choices

```typescript
// Simple text input
const name = await arg("What's your name?");

// With string choices
const color = await arg("Pick a color", ["Red", "Green", "Blue"]);

// With rich choices
const file = await arg("Select a file", [
  { name: "Document.pdf", value: "/path/to/doc.pdf", description: "PDF file" },
  { name: "Image.png", value: "/path/to/img.png", description: "Image file" },
]);

// With dynamic choices (async function)
const repo = await arg("Select repo", async () => {
  const response = await fetch("https://api.github.com/user/repos");
  const repos = await response.json();
  return repos.map((r: any) => ({ name: r.name, value: r.html_url }));
});
```

#### `div()` - Display HTML Content

```typescript
// Simple HTML
await div("<h1>Hello World!</h1>");

// With Tailwind CSS classes
await div(`
  <div class="flex flex-col items-center p-8">
    <h1 class="text-4xl font-bold text-yellow-400">Welcome!</h1>
    <p class="text-gray-400 mt-4">Press Escape to close</p>
  </div>
`);
```

#### `editor()` - Code Editor

```typescript
// Open editor with content
const code = await editor("// Write your code here", "typescript");

// Edit existing file content
const edited = await editor(existingContent, "json");
```

#### `fields()` - Multi-Field Form

```typescript
const [name, email, age] = await fields([
  { name: "name", label: "Name", type: "text", placeholder: "John Doe" },
  { name: "email", label: "Email", type: "email" },
  { name: "age", label: "Age", type: "number" },
]);
```

### File System

#### `path()` - File/Folder Picker

```typescript
// Pick a file
const filePath = await path("Select a file");

// Pick with starting directory
const docPath = await path({ startPath: "~/Documents", hint: "Choose a document" });
```

#### `drop()` - Drag and Drop

```typescript
// Accept dropped files
const files = await drop("Drop files here");
for (const file of files) {
  console.log(file.path, file.name, file.size);
}
```

### Input Capture

#### `hotkey()` - Capture Keyboard Shortcut

```typescript
const shortcut = await hotkey("Press a keyboard shortcut");
console.log(shortcut.key, shortcut.command, shortcut.shift);
```

### Display

#### `md()` - Render Markdown

```typescript
const html = md(`
# Hello World
This is **bold** and this is *italic*.
`);
await div(html);
```

### Advanced

#### `term()` - Terminal Emulator

```typescript
await term("htop");  // Run interactive command
await term({ command: "npm install", cwd: "/path/to/project" });
```

#### `chat()` - Chat Interface

```typescript
await chat({
  onSubmit: async (input) => {
    // Handle user message
    return { text: `You said: ${input}`, position: "left" };
  }
});
```

#### `widget()` - Floating Widget Window

```typescript
const w = await widget(`<h1>Floating Widget</h1>`, {
  width: 300,
  height: 200,
  draggable: true,
  alwaysOnTop: true,
});
```

---

## Scriptlet Format

Scriptlets are markdown files with embedded code blocks. They live in `~/.sk/kit/main/scriptlets/`.

### Basic Scriptlet

```markdown
---
name: My Scriptlet
description: A quick tool
author: Your Name
---

# My Scriptlet

## Greeting Tool
\`\`\`tool:greet
import "@scriptkit/kit";
const name = await arg("Enter name");
await div(`<h1>Hello, ${name}!</h1>`);
\`\`\`
```

### Frontmatter Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name for the bundle |
| `description` | string | Brief description |
| `author` | string | Author name |
| `icon` | string | Icon identifier |

### Code Block Types

- `tool:name` - Executable tool
- `template:name` - Text expansion template
- `snippet:name` - Code snippet
- `prompt:name` - AI prompt

### Variable Substitution

Templates support `{{variable}}` substitution:

```markdown
\`\`\`template:email-reply
Hi {{name}},

Thank you for your email about {{topic}}.

Best regards
\`\`\`
```

---

## Configuration (config.ts)

The `~/.sk/kit/config.ts` file configures Script Kit:

```typescript
import type { Config } from "@scriptkit/kit";

export default {
  // Global hotkey to show Script Kit
  hotkey: {
    key: "Space",
    modifiers: ["command"],
  },

  // UI settings
  editorFontSize: 14,
  terminalFontSize: 14,

  // Built-in features
  builtIns: {
    clipboardHistory: true,
    appLauncher: true,
  },
} satisfies Config;
```

### Config Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `hotkey` | object | cmd+; | Global activation hotkey |
| `editorFontSize` | number | 14 | Editor font size |
| `terminalFontSize` | number | 14 | Terminal font size |
| `builtIns.clipboardHistory` | boolean | true | Enable clipboard history |
| `builtIns.appLauncher` | boolean | true | Enable app launcher |

---

## Testing Scripts

### Run from Script Kit

1. Open Script Kit (default: Cmd+;)
2. Type your script name
3. Press Enter to run

### Run from Terminal

```bash
# Using bun directly
bun run ~/.sk/kit/main/scripts/my-script.ts

# With the kit CLI (if installed)
kit run my-script
```

### Debugging

Add console.error() for debug output:

```typescript
import "@scriptkit/kit";

console.error("[DEBUG] Script starting...");
const result = await arg("Choose", ["A", "B"]);
console.error("[DEBUG] User chose:", result);
```

---

## Examples

### Example 1: Quick Note

```typescript
import "@scriptkit/kit";

export const metadata = {
  name: "Quick Note",
  description: "Save a quick note to a file",
  shortcut: "cmd shift n",
};

const note = await arg("Enter your note");
const timestamp = new Date().toISOString();
const entry = `\n## ${timestamp}\n${note}\n`;

await Bun.write(
  Bun.file(`${home()}/notes.md`),
  (await Bun.file(`${home()}/notes.md`).text().catch(() => "# Notes\n")) + entry
);

await div(`<p class="text-green-400">Note saved!</p>`);
```

### Example 2: GitHub Repo Opener

```typescript
import "@scriptkit/kit";

export const metadata = {
  name: "Open GitHub Repo",
  description: "Search and open a GitHub repository",
  shortcut: "cmd shift g",
};

const repo = await arg("Search repos", async (input) => {
  if (!input) return [];
  const res = await fetch(`https://api.github.com/search/repositories?q=${input}`);
  const data = await res.json();
  return data.items?.map((r: any) => ({
    name: r.full_name,
    value: r.html_url,
    description: r.description || "No description",
  })) || [];
});

await open(repo);
```

### Example 3: JSON Formatter

```typescript
import "@scriptkit/kit";

export const metadata = {
  name: "Format JSON",
  description: "Pretty-print JSON from clipboard",
};

const clipboard = await paste();
try {
  const formatted = JSON.stringify(JSON.parse(clipboard), null, 2);
  await copy(formatted);
  await div(`<pre class="text-green-400">${formatted}</pre>`);
} catch {
  await div(`<p class="text-red-400">Invalid JSON in clipboard</p>`);
}
```

### Example 4: System Info Widget

```typescript
import "@scriptkit/kit";
import os from "os";

export const metadata = {
  name: "System Info",
  description: "Show system information",
};

const info = `
  <div class="p-4 space-y-2">
    <p><strong>Platform:</strong> ${os.platform()}</p>
    <p><strong>Arch:</strong> ${os.arch()}</p>
    <p><strong>CPUs:</strong> ${os.cpus().length}</p>
    <p><strong>Memory:</strong> ${Math.round(os.totalmem() / 1024 / 1024 / 1024)}GB</p>
    <p><strong>Uptime:</strong> ${Math.round(os.uptime() / 3600)} hours</p>
  </div>
`;

await div(info);
```

### Example 5: File Search

```typescript
import "@scriptkit/kit";

export const metadata = {
  name: "Find Files",
  description: "Search for files by name",
  shortcut: "cmd shift f",
};

const query = await arg("Search for files");
const { stdout } = await $`find ~ -name "*${query}*" -type f 2>/dev/null | head -20`;

const files = stdout.trim().split("\n").filter(Boolean);

if (files.length === 0) {
  await div(`<p class="text-yellow-400">No files found</p>`);
} else {
  const selected = await arg("Select file", files.map(f => ({
    name: f.split("/").pop() || f,
    value: f,
    description: f,
  })));
  
  await open(selected);
}
```

---

## Best Practices

1. **Always use `export const metadata`** - Get type safety and IDE support
2. **Import the SDK first** - `import "@scriptkit/kit"` at the top
3. **Use Tailwind classes** - Built-in support for styling in div()
4. **Handle errors gracefully** - Wrap async operations in try/catch
5. **Keep scripts focused** - One script, one task
6. **Use meaningful names** - Clear metadata.name and description
7. **Add shortcuts sparingly** - Only for frequently used scripts

---

## File Locations

| Path | Purpose |
|------|---------|
| `~/.sk/kit/main/scripts/` | Your scripts |
| `~/.sk/kit/main/scriptlets/` | Your scriptlets |
| `~/.sk/kit/config.ts` | Configuration |
| `~/.sk/kit/theme.json` | Theme customization |
| `~/.sk/kit/sdk/` | SDK (managed by app) |
"###;

/// Environment variable to override the default ~/.sk/kit path
pub const SK_PATH_ENV: &str = "SK_PATH";

/// Result of setup process
#[derive(Debug)]
pub struct SetupResult {
    /// Whether ~/.sk/kit didn't exist before this run
    pub is_fresh_install: bool,
    /// Path to ~/.sk/kit (or SK_PATH override, or fallback if home dir couldn't be resolved)
    pub kit_path: PathBuf,
    /// Whether bun looks discoverable on this machine
    pub bun_available: bool,
    /// Any warnings encountered during setup
    pub warnings: Vec<String>,
}

/// Get the kit path, respecting SK_PATH environment variable
///
/// Priority:
/// 1. SK_PATH environment variable (if set)
/// 2. ~/.sk/kit (default)
/// 3. Temp directory fallback (if home dir unavailable)
pub fn get_kit_path() -> PathBuf {
    // Check for SK_PATH override first
    if let Ok(sk_path) = std::env::var(SK_PATH_ENV) {
        return PathBuf::from(shellexpand::tilde(&sk_path).as_ref());
    }

    // Default: ~/.sk/kit
    match dirs::home_dir() {
        Some(home) => home.join(".sk").join("kit"),
        None => std::env::temp_dir().join("script-kit"),
    }
}

/// Migrate from legacy ~/.kenv to new ~/.sk/kit structure
///
/// This function handles one-time migration from the old directory structure:
/// - Moves ~/.kenv contents to ~/.sk/kit
/// - Moves ~/.kenv/scripts to ~/.sk/kit/main/scripts  
/// - Moves ~/.kenv/scriptlets to ~/.sk/kit/main/scriptlets
/// - Creates a symlink ~/.kenv -> ~/.sk/kit for backwards compatibility
///
/// Returns true if migration was performed, false if not needed
#[instrument(level = "info", name = "migrate_from_kenv")]
pub fn migrate_from_kenv() -> bool {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };

    let old_kenv = home.join(".kenv");
    let new_sk_kit = home.join(".sk").join("kit");

    // Only migrate if old path exists and new path doesn't
    if !old_kenv.exists() || new_sk_kit.exists() {
        return false;
    }

    info!(
        old_path = %old_kenv.display(),
        new_path = %new_sk_kit.display(),
        "Migrating from ~/.kenv to ~/.sk/kit"
    );

    // Ensure parent directory exists
    if let Err(e) = fs::create_dir_all(home.join(".sk")) {
        warn!(error = %e, "Failed to create ~/.sk directory");
        return false;
    }

    // Create the new structure
    let main_scripts = new_sk_kit.join("main").join("scripts");
    let main_scriptlets = new_sk_kit.join("main").join("scriptlets");

    if let Err(e) = fs::create_dir_all(&main_scripts) {
        warn!(error = %e, "Failed to create main/scripts directory");
        return false;
    }

    if let Err(e) = fs::create_dir_all(&main_scriptlets) {
        warn!(error = %e, "Failed to create main/scriptlets directory");
        return false;
    }

    // Move scripts from ~/.kenv/scripts to ~/.sk/kit/main/scripts
    let old_scripts = old_kenv.join("scripts");
    if old_scripts.exists() && old_scripts.is_dir() {
        if let Ok(entries) = fs::read_dir(&old_scripts) {
            for entry in entries.flatten() {
                let old_path = entry.path();
                let file_name = old_path.file_name().unwrap_or_default();
                let new_path = main_scripts.join(file_name);

                if let Err(e) = fs::rename(&old_path, &new_path) {
                    warn!(
                        error = %e,
                        old = %old_path.display(),
                        new = %new_path.display(),
                        "Failed to move script"
                    );
                }
            }
        }
    }

    // Move scriptlets from ~/.kenv/scriptlets to ~/.sk/kit/main/scriptlets
    let old_scriptlets = old_kenv.join("scriptlets");
    if old_scriptlets.exists() && old_scriptlets.is_dir() {
        if let Ok(entries) = fs::read_dir(&old_scriptlets) {
            for entry in entries.flatten() {
                let old_path = entry.path();
                let file_name = old_path.file_name().unwrap_or_default();
                let new_path = main_scriptlets.join(file_name);

                if let Err(e) = fs::rename(&old_path, &new_path) {
                    warn!(
                        error = %e,
                        old = %old_path.display(),
                        new = %new_path.display(),
                        "Failed to move scriptlet"
                    );
                }
            }
        }
    }

    // Move config files to new root
    let config_files = ["config.ts", "theme.json", "tsconfig.json", ".gitignore"];
    for file in config_files {
        let old_path = old_kenv.join(file);
        let new_path = new_sk_kit.join(file);
        if old_path.exists() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                warn!(error = %e, file = file, "Failed to move config file");
            }
        }
    }

    // Move data directories to new root
    let data_dirs = ["logs", "cache", "db", "sdk"];
    for dir in data_dirs {
        let old_path = old_kenv.join(dir);
        let new_path = new_sk_kit.join(dir);
        if old_path.exists() && old_path.is_dir() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                warn!(error = %e, dir = dir, "Failed to move data directory");
            }
        }
    }

    // Move data files to new root
    let data_files = [
        "frecency.json",
        "store.json",
        "server.json",
        "agent-token",
        "notes.db",
        "ai-chats.db",
        "clipboard-history.db",
    ];
    for file in data_files {
        let old_path = old_kenv.join(file);
        let new_path = new_sk_kit.join(file);
        if old_path.exists() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                warn!(error = %e, file = file, "Failed to move data file");
            }
        }
    }

    // Remove the old ~/.kenv directory (should be mostly empty now)
    if let Err(e) = fs::remove_dir_all(&old_kenv) {
        warn!(error = %e, "Failed to remove old ~/.kenv directory, may have remaining files");
    }

    // Create symlink for backwards compatibility (Unix only)
    #[cfg(unix)]
    {
        if let Err(e) = std::os::unix::fs::symlink(&new_sk_kit, &old_kenv) {
            warn!(error = %e, "Failed to create ~/.kenv symlink for backwards compatibility");
        } else {
            info!("Created ~/.kenv -> ~/.sk/kit symlink for backwards compatibility");
        }
    }

    info!("Migration from ~/.kenv to ~/.sk/kit complete");
    true
}

/// Ensure the ~/.sk/kit environment is properly set up.
///
/// This function is idempotent - it will create missing directories and files
/// without overwriting existing user configurations.
///
/// # Directory Structure Created
/// ```text
/// ~/.sk/kit/                  # Root (can be overridden via SK_PATH)
/// ├── main/                   # Default user kit
/// │   ├── scripts/            # User scripts (.ts, .js files)
/// │   └── scriptlets/         # Markdown scriptlet files
/// ├── examples/               # Example kit (created on fresh install)
/// │   ├── scripts/
/// │   └── scriptlets/
/// ├── sdk/                    # Runtime SDK (kit-sdk.ts)
/// ├── db/                     # Databases
/// ├── logs/                   # Application logs
/// ├── cache/
/// │   └── app-icons/          # Cached application icons
/// ├── config.ts               # User configuration (created from template if missing)
/// ├── theme.json              # Theme configuration (created from example if missing)
/// ├── tsconfig.json           # TypeScript path mappings
/// └── .gitignore              # Ignore transient files
/// ```
///
/// # Environment Variables
/// - `SK_PATH`: Override the default ~/.sk/kit path
///
/// # Returns
/// `SetupResult` with information about the setup process.
#[instrument(level = "info", name = "ensure_kit_setup")]
pub fn ensure_kit_setup() -> SetupResult {
    let mut warnings = Vec::new();

    let kit_dir = get_kit_path();

    // Check if this is a fresh install before we create anything
    let is_fresh_install = !kit_dir.exists();

    // Log if using SK_PATH override
    if std::env::var(SK_PATH_ENV).is_ok() {
        info!(
            kit_path = %kit_dir.display(),
            "Using SK_PATH override"
        );
    }

    // Ensure root kit directory exists first
    if let Err(e) = fs::create_dir_all(&kit_dir) {
        warnings.push(format!(
            "Failed to create kit root {}: {}",
            kit_dir.display(),
            e
        ));
        // If we can't create the root, there's not much else we can safely do.
        return SetupResult {
            is_fresh_install,
            kit_path: kit_dir,
            bun_available: false,
            warnings,
        };
    }

    // Required directory structure
    // Note: main/scripts and main/scriptlets are the default user workspace
    let required_dirs = [
        kit_dir.join("main").join("scripts"),
        kit_dir.join("main").join("scriptlets"),
        kit_dir.join("sdk"),
        kit_dir.join("db"),
        kit_dir.join("logs"),
        kit_dir.join("cache").join("app-icons"),
    ];

    for dir in required_dirs {
        ensure_dir(&dir, &mut warnings);
    }

    // App-managed: SDK (refresh if changed)
    let sdk_path = kit_dir.join("sdk").join("kit-sdk.ts");
    write_string_if_changed(&sdk_path, EMBEDDED_SDK, &mut warnings, "sdk/kit-sdk.ts");

    // User-owned: config.ts (only create if missing)
    let config_path = kit_dir.join("config.ts");
    write_string_if_missing(
        &config_path,
        EMBEDDED_CONFIG_TEMPLATE,
        &mut warnings,
        "config.ts",
    );

    // User-owned (optional): theme.json (only create if missing)
    let theme_path = kit_dir.join("theme.json");
    write_string_if_missing(
        &theme_path,
        EMBEDDED_THEME_EXAMPLE,
        &mut warnings,
        "theme.json",
    );

    // App-managed: tsconfig.json path mappings (merge-safe)
    ensure_tsconfig_paths(&kit_dir.join("tsconfig.json"), &mut warnings);

    // App-managed: package.json for top-level await support
    let package_json_path = kit_dir.join("package.json");
    write_string_if_missing(
        &package_json_path,
        EMBEDDED_PACKAGE_JSON,
        &mut warnings,
        "package.json",
    );

    // User guide: AGENTS.md for AI agents writing scripts
    let agents_md_path = kit_dir.join("AGENTS.md");
    write_string_if_missing(
        &agents_md_path,
        EMBEDDED_AGENTS_MD,
        &mut warnings,
        "AGENTS.md",
    );

    // User-owned: GUIDE.md (only create if missing)
    // Comprehensive user guide for learning Script Kit
    let guide_md_path = kit_dir.join("GUIDE.md");
    write_string_if_missing(&guide_md_path, EMBEDDED_GUIDE_MD, &mut warnings, "GUIDE.md");

    // App-managed: .gitignore (refresh if changed)
    let gitignore_path = kit_dir.join(".gitignore");
    let gitignore_content = r#"# Script Kit managed .gitignore
# This file is regenerated on app start - edit with caution

# =============================================================================
# Node.js / Bun dependencies
# =============================================================================
# Root node_modules (for package.json at ~/.sk/kit/)
node_modules/

# Kit-specific node_modules (e.g., main/node_modules, examples/node_modules)
*/node_modules/

# Package manager files
package-lock.json
yarn.lock
pnpm-lock.yaml
bun.lockb
.pnpm-store/

# =============================================================================
# Databases
# =============================================================================
# SQLite databases
*.db
*.db-journal
*.db-shm
*.db-wal

# Specific databases (redundant with *.db but explicit for clarity)
db/
clipboard-history.db
notes.db
ai-chats.db

# =============================================================================
# Runtime & Cache
# =============================================================================
# SDK is managed by the app, always regenerated
sdk/

# Application logs
logs/

# Cache files (app icons, etc.)
cache/

# Frecency tracking (regenerated from usage)
frecency.json

# Server state
server.json

# Authentication tokens
agent-token

# =============================================================================
# Build & Tooling
# =============================================================================
# TypeScript build output
*.tsbuildinfo
dist/
build/
.turbo/

# IDE
.idea/
.vscode/
*.swp
*.swo
*~

# macOS
.DS_Store
._*

# =============================================================================
# Secrets & Environment
# =============================================================================
.env
.env.local
.env.*.local
*.pem
*.key

# =============================================================================
# Temporary files
# =============================================================================
*.tmp
*.temp
*.log
tmp/
temp/
"#;
    write_string_if_changed(
        &gitignore_path,
        gitignore_content,
        &mut warnings,
        ".gitignore",
    );

    // Dependency check: bun (no process spawn; just path checks)
    let bun_available = bun_is_discoverable();
    if !bun_available {
        warnings.push(
            "bun not found (PATH/common install locations). Config/scripts may not run until bun is installed.".to_string(),
        );
    }

    // Optional "getting started" content only on truly fresh installs
    if is_fresh_install {
        create_sample_files(&kit_dir, &mut warnings);
    }

    info!(
        kit_path = %kit_dir.display(),
        is_fresh_install,
        bun_available,
        warning_count = warnings.len(),
        "Kit setup complete"
    );

    SetupResult {
        is_fresh_install,
        kit_path: kit_dir,
        bun_available,
        warnings,
    }
}

fn ensure_dir(path: &Path, warnings: &mut Vec<String>) {
    if path.exists() {
        return;
    }
    if let Err(e) = fs::create_dir_all(path) {
        warnings.push(format!(
            "Failed to create directory {}: {}",
            path.display(),
            e
        ));
    } else {
        debug!(path = %path.display(), "Created directory");
    }
}

fn write_string_if_missing(path: &Path, contents: &str, warnings: &mut Vec<String>, label: &str) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            warnings.push(format!(
                "Failed to create parent dir for {} ({}): {}",
                label,
                parent.display(),
                e
            ));
            return;
        }
    }
    if let Err(e) = fs::write(path, contents) {
        warnings.push(format!(
            "Failed to write {} ({}): {}",
            label,
            path.display(),
            e
        ));
    } else {
        info!(path = %path.display(), "Created {}", label);
    }
}

fn write_string_if_changed(path: &Path, contents: &str, warnings: &mut Vec<String>, label: &str) {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return;
        }
    }

    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            warnings.push(format!(
                "Failed to create parent dir for {} ({}): {}",
                label,
                parent.display(),
                e
            ));
            return;
        }
    }

    if let Err(e) = fs::write(path, contents) {
        warnings.push(format!(
            "Failed to write {} ({}): {}",
            label,
            path.display(),
            e
        ));
    } else {
        debug!(path = %path.display(), "Updated {}", label);
    }
}

/// Ensure tsconfig.json has the @scriptkit/kit path mapping (merge-safe)
fn ensure_tsconfig_paths(tsconfig_path: &Path, warnings: &mut Vec<String>) {
    use serde_json::{json, Value};

    let kit_path = json!(["./sdk/kit-sdk.ts"]);

    let mut config: Value = if tsconfig_path.exists() {
        match fs::read_to_string(tsconfig_path) {
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

    // Check if already has the correct @scriptkit/kit path
    let current_kit_path = config["compilerOptions"]["paths"].get("@scriptkit/kit");
    if current_kit_path == Some(&kit_path) {
        return;
    }

    // Set the new @scriptkit/kit path
    config["compilerOptions"]["paths"]["@scriptkit/kit"] = kit_path;

    // Remove old @johnlindquist/kit path if it exists (migration)
    if let Some(paths) = config["compilerOptions"]["paths"].as_object_mut() {
        paths.remove("@johnlindquist/kit");
    }

    match serde_json::to_string_pretty(&config) {
        Ok(json_str) => {
            if let Err(e) = fs::write(tsconfig_path, json_str) {
                warnings.push(format!(
                    "Failed to write tsconfig.json ({}): {}",
                    tsconfig_path.display(),
                    e
                ));
                warn!(error = %e, "Failed to write tsconfig.json");
            } else {
                info!("Updated tsconfig.json with @scriptkit/kit path mapping");
            }
        }
        Err(e) => {
            warnings.push(format!("Failed to serialize tsconfig.json: {}", e));
            warn!(error = %e, "Failed to serialize tsconfig.json");
        }
    }
}

/// Fast check: looks for bun in common locations and PATH without spawning a process.
fn bun_is_discoverable() -> bool {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Common install locations
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".bun").join("bin").join(bun_exe_name()));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin").join(bun_exe_name()));
    candidates.push(PathBuf::from("/usr/local/bin").join(bun_exe_name()));
    candidates.push(PathBuf::from("/usr/bin").join(bun_exe_name()));

    // PATH scan
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join(bun_exe_name()));
        }
    }

    candidates.into_iter().any(|p| p.exists())
}

fn bun_exe_name() -> &'static str {
    #[cfg(windows)]
    {
        "bun.exe"
    }
    #[cfg(not(windows))]
    {
        "bun"
    }
}

fn create_sample_files(kit_dir: &Path, warnings: &mut Vec<String>) {
    // Create sample files in the main kit
    let main_scripts_dir = kit_dir.join("main").join("scripts");
    let main_scriptlets_dir = kit_dir.join("main").join("scriptlets");

    // Create hello-world.ts script
    let hello_script_path = main_scripts_dir.join("hello-world.ts");
    if !hello_script_path.exists() {
        let hello_script = r#"/*
# Hello World

A simple greeting script demonstrating Script Kit basics.

## Features shown:
- `arg()` - Prompt for user input with choices
- `div()` - Display HTML content with Tailwind CSS
- `md()` - Render markdown to HTML
*/

export const metadata = {
  name: "Hello World",
  description: "A simple greeting script",
  // shortcut: "cmd shift h",  // Uncomment to add a global hotkey
};

// Prompt the user to select or type their name
const name = await arg("What's your name?", [
  "World",
  "Script Kit",
  "Friend",
]);

// Display a greeting using HTML with Tailwind CSS classes
await div(`
  <div class="flex flex-col items-center justify-center h-full p-8">
    <h1 class="text-4xl font-bold text-yellow-400 mb-4">
      Hello, ${name}! 👋
    </h1>
    <p class="text-gray-400 text-lg">
      Welcome to Script Kit
    </p>
    <div class="mt-6 text-sm text-gray-500">
      Press <kbd class="px-2 py-1 bg-gray-700 rounded">Escape</kbd> to close
    </div>
  </div>
`);
"#;
        if let Err(e) = fs::write(&hello_script_path, hello_script) {
            warnings.push(format!(
                "Failed to create sample script {}: {}",
                hello_script_path.display(),
                e
            ));
        } else {
            info!(path = %hello_script_path.display(), "Created sample script");
        }
    }

    // Create hello-world.md scriptlet
    let hello_scriptlet_path = main_scriptlets_dir.join("hello-world.md");
    if !hello_scriptlet_path.exists() {
        let hello_scriptlet = r#"# Hello World Scriptlets

Quick shell commands you can run from Script Kit.
Each code block is a separate scriptlet that appears in the menu.

---

## Say Hello
<!-- 
name: Say Hello
description: Display a greeting notification
shortcut: ctrl h
-->

```bash
echo "Hello from Script Kit! 🎉"
```

---

## Current Date
<!-- 
name: Current Date
description: Copy today's date to clipboard
shortcut: ctrl d
-->

```bash
date +"%Y-%m-%d" | pbcopy
echo "Date copied: $(date +"%Y-%m-%d")"
```

---

## Open Downloads
<!-- 
name: Open Downloads
description: Open the Downloads folder in Finder
-->

```bash
open ~/Downloads
```

---

## Quick Note
<!-- 
name: Quick Note
description: Append a timestamped note to notes.txt
-->

```bash
echo "[$(date +"%Y-%m-%d %H:%M")] $1" >> ~/notes.txt
echo "Note saved!"
```

---

## System Info
<!-- 
name: System Info
description: Show basic system information
-->

```bash
echo "User: $(whoami)"
echo "Host: $(hostname)"
echo "OS: $(sw_vers -productName) $(sw_vers -productVersion)"
echo "Shell: $SHELL"
```
"#;
        if let Err(e) = fs::write(&hello_scriptlet_path, hello_scriptlet) {
            warnings.push(format!(
                "Failed to create sample scriptlet {}: {}",
                hello_scriptlet_path.display(),
                e
            ));
        } else {
            info!(path = %hello_scriptlet_path.display(), "Created sample scriptlet");
        }
    }

    // Create README.md at kit root
    let readme_path = kit_dir.join("README.md");
    if !readme_path.exists() {
        let readme = r##"# Script Kit

Welcome to Script Kit! This directory contains your scripts, configuration, and data.

## Directory Structure

```
~/.sk/kit/
├── main/                   # Your default kit (scripts & scriptlets)
│   ├── scripts/            # TypeScript/JavaScript scripts (.ts, .js)
│   └── scriptlets/         # Markdown scriptlet files (.md)
├── sdk/                    # Runtime SDK (managed by app)
├── db/                     # Databases (clipboard history, etc.)
├── logs/                   # Application logs
├── cache/                  # Cached data (app icons, etc.)
├── config.ts               # Your configuration
├── theme.json              # Theme customization
├── tsconfig.json           # TypeScript path mappings
└── README.md               # This file
```

## File Watching

Script Kit watches these files and reloads automatically:

| File/Directory | What happens on change |
|----------------|------------------------|
| `config.ts` | Reloads configuration (hotkeys, settings) |
| `theme.json` | Applies new theme colors immediately |
| `main/scripts/*.ts` | Updates script list and metadata |
| `main/scriptlets/*.md` | Updates scriptlet list |

## Scripts

Scripts are TypeScript files in `main/scripts/`. They have full access to the Script Kit SDK.

### Example Script

```typescript
// main/scripts/my-script.ts

export const metadata = {
  name: "My Script",
  description: "Does something useful",
  shortcut: "cmd shift m",  // Optional global hotkey
};

// Prompt for input
const choice = await arg("Pick an option", ["Option 1", "Option 2"]);

// Show result
await div(`<div class="p-4">You chose: ${choice}</div>`);
```

### Script Metadata

Use the `metadata` export for type-safe configuration:

```typescript
export const metadata = {
  name: "Script Name",           // Display name in menu
  description: "What it does",   // Shown below the name
  shortcut: "cmd shift x",       // Global hotkey (optional)
  alias: "sn",                   // Quick search alias (optional)
};
```

## Scriptlets

Scriptlets are Markdown files containing quick shell commands. Each code block becomes a menu item.

### Example Scriptlet

```markdown
# My Scriptlets

## Open Project
<!-- shortcut: cmd shift p -->

\`\`\`bash
cd ~/projects/myapp && code .
\`\`\`

## Git Status
<!-- name: Check Git Status -->

\`\`\`bash
git status
\`\`\`
```

### Scriptlet Metadata

Add HTML comments before code blocks:

```markdown
<!-- 
name: Display Name
description: What this does
shortcut: cmd shift x
-->
```

## Configuration (config.ts)

Your `config.ts` controls Script Kit behavior:

```typescript
export default {
  // Global hotkey to open Script Kit
  hotkey: {
    key: "Semicolon",
    modifiers: ["meta"],  // cmd+;
  },
  
  // UI Settings
  editorFontSize: 16,
  terminalFontSize: 14,
  
  // Built-in features
  builtIns: {
    clipboardHistory: true,
    appLauncher: true,
  },
} satisfies Config;
```

## Theme (theme.json)

Customize colors in `theme.json`:

```json
{
  "colors": {
    "background": {
      "main": "#1E1E1E"
    },
    "text": {
      "primary": "#FFFFFF",
      "secondary": "#CCCCCC"
    },
    "accent": {
      "selected": "#FBBF24"
    }
  }
}
```

Colors can be specified as:
- Hex strings: `"#FBBF24"` or `"FBBF24"`
- RGB: `"rgb(251, 191, 36)"`
- RGBA: `"rgba(251, 191, 36, 1.0)"`

## Environment Variable

Set `SK_PATH` to use a different directory:

```bash
export SK_PATH=~/my-scripts
```

## Quick Tips

1. **Create a new script**: Add a `.ts` file to `main/scripts/`
2. **Add a hotkey**: Set `shortcut` in the metadata
3. **Test changes**: Scripts reload automatically on save
4. **View logs**: Check `logs/script-kit-gpui.jsonl` for debugging
5. **Complete guide**: See `GUIDE.md` for comprehensive tutorials and documentation

## Links

- Documentation: https://scriptkit.com/docs
- GitHub: https://github.com/johnlindquist/kit

---

Happy scripting! 🚀
"##;
        if let Err(e) = fs::write(&readme_path, readme) {
            warnings.push(format!(
                "Failed to create README {}: {}",
                readme_path.display(),
                e
            ));
        } else {
            info!(path = %readme_path.display(), "Created README.md");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bun_is_discoverable() {
        // This test just verifies the function doesn't panic
        let _ = bun_is_discoverable();
    }

    #[test]
    fn test_bun_exe_name() {
        let name = bun_exe_name();
        #[cfg(windows)]
        assert_eq!(name, "bun.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "bun");
    }

    #[test]
    fn test_get_kit_path_default() {
        // Without SK_PATH set, should return ~/.sk/kit
        std::env::remove_var(SK_PATH_ENV);
        let path = get_kit_path();
        assert!(path.to_string_lossy().contains(".sk"));
        assert!(path.to_string_lossy().ends_with("kit"));
    }

    #[test]
    fn test_get_kit_path_with_override() {
        // With SK_PATH set, should return the override
        std::env::set_var(SK_PATH_ENV, "/custom/path");
        let path = get_kit_path();
        assert_eq!(path, PathBuf::from("/custom/path"));
        std::env::remove_var(SK_PATH_ENV);
    }

    #[test]
    fn test_get_kit_path_with_tilde() {
        // SK_PATH with tilde should expand
        std::env::set_var(SK_PATH_ENV, "~/.config/kit");
        let path = get_kit_path();
        assert!(!path.to_string_lossy().contains("~"));
        assert!(path.to_string_lossy().contains(".config/kit"));
        std::env::remove_var(SK_PATH_ENV);
    }
}

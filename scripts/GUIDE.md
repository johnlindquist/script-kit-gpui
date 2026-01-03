# Script Kit User Guide

Welcome to Script Kit! This guide will help you get started and master the powerful automation capabilities of Script Kit.

---

## Table of Contents

1. [Welcome & Quick Start](#welcome--quick-start)
2. [Directory Structure](#directory-structure)
3. [Writing Scripts](#writing-scripts)
4. [SDK Functions (Core)](#sdk-functions-core)
5. [Scriptlets](#scriptlets)
6. [Configuration](#configuration-configts)
7. [Themes](#themes-themejson)
8. [Built-in Features](#built-in-features)
9. [AI Window (BYOK)](#ai-window-byok)
10. [Notes Window](#notes-window)
11. [File Watching](#file-watching)
12. [Multiple Environments](#multiple-environments)
13. [SDK Quick Reference](#sdk-quick-reference)

---

## Welcome & Quick Start

### What is Script Kit?

Script Kit is a powerful automation tool that lets you create scripts to automate your daily workflows. Built with the GPUI framework from Zed, it offers:

- **Blazing fast performance** - Native Rust with GPU-accelerated rendering
- **Beautiful UI prompts** - Text inputs, editors, forms, file pickers, and more
- **Global hotkey access** - Trigger scripts from anywhere
- **Bun runtime** - Fast JavaScript/TypeScript execution
- **Tailwind CSS** - Style your prompts with familiar utility classes

### Your First Script in 60 Seconds

1. **Create the scripts directory** (if it doesn't exist):
   ```bash
   mkdir -p ~/.sk/kit/main/scripts
   ```

2. **Create your first script** at `~/.sk/kit/main/scripts/hello.ts`:
   ```typescript
   export const metadata = {
     name: "Hello World",
     description: "My first Script Kit script"
   };

   const name = await arg("What's your name?");
   await div(`<h1 class="text-4xl p-8 text-center">Hello, ${name}! 👋</h1>`);
   ```

3. **Open Script Kit** by pressing the global hotkey (default: `Cmd+;`)

4. **Type "hello"** and press Enter

5. **Enter your name** and see the greeting!

### The Main Hotkey

The global hotkey opens the Script Kit launcher from anywhere:

| Platform | Default Hotkey |
|----------|----------------|
| macOS    | `Cmd+;`        |
| Windows  | `Ctrl+;`       |
| Linux    | `Ctrl+;`       |

You can customize this in `~/.sk/kit/config.ts` (see [Configuration](#configuration-configts)).

---

## Directory Structure

Script Kit stores all its data in `~/.sk/kit/`. Here's the layout:

```
~/.sk/kit/
├── main/                    # Default kit (your scripts)
│   ├── scripts/             # Your script files (.ts, .js)
│   └── scriptlets/          # Markdown-based mini-scripts
├── sdk/                     # SDK runtime (auto-managed)
│   └── kit-sdk.ts           # The Script Kit SDK
├── db/                      # Databases
│   ├── notes.sqlite         # Notes window data
│   └── ai-chats.sqlite      # AI chat history
├── logs/                    # Application logs
│   └── script-kit-gpui.jsonl
├── cache/                   # Cached data (frecency, etc.)
├── config.ts                # Your configuration
└── theme.json               # Your theme customization
```

### Key Directories

| Directory | Purpose |
|-----------|---------|
| `main/scripts/` | Your primary scripts - create `.ts` files here |
| `main/scriptlets/` | Markdown shell command templates |
| `sdk/` | Runtime SDK (auto-extracted, don't edit) |
| `db/` | SQLite databases for Notes and AI |
| `logs/` | Debug logs in JSONL format |

---

## Writing Scripts

### Creating a Script File

Create a `.ts` file in `~/.sk/kit/main/scripts/`:

```typescript
// ~/.sk/kit/main/scripts/my-script.ts

export const metadata = {
  name: "My Script",
  description: "What this script does",
  shortcut: "cmd+shift+m",  // Optional: global shortcut
};

// Your script code here
const result = await arg("Pick an option", ["Option A", "Option B", "Option C"]);
console.log("You chose:", result);
```

### Script Metadata

Use the global `metadata` variable to define script properties:

```typescript
export const metadata = {
  // Required
  name: "My Script",           // Display name in launcher

  // Optional
  description: "Description",  // Shown below the name
  author: "Your Name",         // Script author
  shortcut: "cmd+shift+m",     // Global keyboard shortcut
  alias: "ms",                 // Short alias for quick triggering
  icon: "File",                // Icon name (e.g., "Terminal", "Star")
  tags: ["utility", "dev"],    // Categories for organization
  hidden: false,               // Hide from main list
  
  // Scheduling
  schedule: "every day at 2pm", // Natural language schedule
  cron: "0 14 * * *",          // Or use cron syntax
  
  // Advanced
  background: false,           // Run without UI
  watch: ["~/Documents/*.md"], // Trigger on file changes
};
```

### Legacy Comment-Based Metadata

For backwards compatibility, comment-based metadata still works:

```typescript
// Name: My Script
// Description: What this script does
// Shortcut: cmd+shift+m
// Author: Your Name

const result = await arg("Pick something");
```

> **Recommendation:** Use the global `metadata` format for better TypeScript support and IDE autocomplete.

### Importing the SDK

The SDK is automatically preloaded, so all functions are available globally. However, for TypeScript type hints, you can import it:

```typescript
import "@scriptkit/kit";

// Now you get full autocomplete for arg(), div(), editor(), etc.
```

---

## SDK Functions (Core)

### arg() - Text Input with Choices

The most versatile prompt - get text input with optional choice list:

```typescript
// Simple text input
const name = await arg("What's your name?");

// Text input with choices
const fruit = await arg("Pick a fruit", ["Apple", "Banana", "Cherry"]);

// Rich choices with metadata
const app = await arg("Launch app", [
  { name: "VS Code", value: "code", description: "Code editor" },
  { name: "Terminal", value: "term", description: "Command line" },
  { name: "Browser", value: "chrome", description: "Web browser" },
]);

// Dynamic choices (async function)
const repo = await arg("Select repo", async () => {
  const response = await fetch("https://api.github.com/users/me/repos");
  const repos = await response.json();
  return repos.map(r => ({ name: r.name, value: r.clone_url }));
});

// Filter function (called on each keystroke)
const file = await arg("Search files", (input) => {
  return files.filter(f => f.includes(input));
});
```

**Configuration Object:**

```typescript
const result = await arg({
  placeholder: "Type something...",
  hint: "Press Enter to submit",
  choices: ["Option 1", "Option 2"],
  onInit: () => console.log("Prompt opened"),
  onSubmit: (value) => console.log("Submitted:", value),
  actions: [
    {
      name: "Copy",
      shortcut: "cmd+c",
      onAction: async (input) => {
        await copy(input);
        hud("Copied!");
      }
    }
  ]
});
```

### div() - HTML Display with Tailwind

Display rich HTML content styled with Tailwind CSS:

```typescript
// Simple HTML
await div("<h1 class='text-4xl p-8'>Hello World!</h1>");

// Complex layout
await div(`
  <div class="flex flex-col gap-4 p-8">
    <h1 class="text-3xl font-bold text-blue-500">Dashboard</h1>
    <div class="grid grid-cols-2 gap-4">
      <div class="bg-gray-800 p-4 rounded-lg">
        <p class="text-gray-400">Total Scripts</p>
        <p class="text-2xl font-bold">42</p>
      </div>
      <div class="bg-gray-800 p-4 rounded-lg">
        <p class="text-gray-400">Runs Today</p>
        <p class="text-2xl font-bold">128</p>
      </div>
    </div>
  </div>
`);

// With configuration
await div({
  html: "<p>Custom styled content</p>",
  placeholder: "My Title",
  containerClasses: "bg-gradient-to-r from-purple-500 to-pink-500",
  containerBg: "transparent",
  containerPadding: "none",
  opacity: 95,
});
```

### md() - Markdown to HTML

Convert Markdown to HTML for use with `div()`:

```typescript
const html = md(`
# Hello World

This is **bold** and this is *italic*.

- List item 1
- List item 2
- List item 3

\`\`\`javascript
const greeting = "Hello!";
console.log(greeting);
\`\`\`
`);

await div(html);
```

Supported Markdown features:
- Headings (h1-h6)
- Bold, italic, strikethrough
- Ordered and unordered lists
- Code blocks and inline code
- Links and images
- Blockquotes
- Horizontal rules

### editor() - Code Editor

Open a Monaco-style code editor:

```typescript
// Basic editor
const code = await editor();

// With initial content and language
const typescript = await editor(`
function greet(name: string) {
  return \`Hello, \${name}!\`;
}
`, "typescript");

// Supported languages
// "typescript", "javascript", "json", "html", "css", "markdown", "python", "rust", etc.
```

### fields() - Multi-Field Forms

Create forms with multiple input fields:

```typescript
// Simple fields (strings become both name and label)
const [firstName, lastName] = await fields(["First Name", "Last Name"]);

// Rich field definitions
const [name, email, age] = await fields([
  { name: "name", label: "Full Name", placeholder: "John Doe" },
  { name: "email", label: "Email", type: "email", placeholder: "john@example.com" },
  { name: "age", label: "Age", type: "number" },
]);

// All supported field types
const values = await fields([
  { name: "text", label: "Text", type: "text" },
  { name: "password", label: "Password", type: "password" },
  { name: "email", label: "Email", type: "email" },
  { name: "number", label: "Number", type: "number" },
  { name: "date", label: "Date", type: "date" },
  { name: "time", label: "Time", type: "time" },
  { name: "url", label: "URL", type: "url" },
  { name: "tel", label: "Phone", type: "tel" },
  { name: "color", label: "Color", type: "color" },
]);
```

### path() - File/Folder Picker

Browse and select files or folders:

```typescript
// Basic file picker
const filePath = await path();

// Start in a specific directory
const document = await path({
  startPath: "~/Documents",
  hint: "Select a document to open",
});

// Common patterns
const image = await path({ startPath: "~/Pictures" });
const project = await path({ startPath: "~/Projects" });
```

### hotkey() - Capture Keyboard Shortcuts

Capture a keyboard shortcut from the user:

```typescript
const shortcut = await hotkey("Press a keyboard shortcut");

console.log(shortcut);
// {
//   key: "k",
//   command: true,
//   shift: true,
//   option: false,
//   control: false,
//   shortcut: "cmd+shift+k",
//   keyCode: "KeyK"
// }
```

### drop() - Drag and Drop

Create a drop zone for files:

```typescript
const files = await drop();

for (const file of files) {
  console.log(`File: ${file.name}`);
  console.log(`Path: ${file.path}`);
  console.log(`Size: ${file.size} bytes`);
}
```

### term() - Terminal Emulator

Open an interactive terminal:

```typescript
// Open empty terminal
await term();

// Run a command
await term("htop");

// Run with environment setup
await term("cd ~/Projects && npm start");
```

### Additional Prompts

```typescript
// Compact prompt variants
const result1 = await mini("Pick one", ["A", "B", "C"]);
const result2 = await micro("Pick one", ["A", "B", "C"]);

// Multi-select
const selected = await select("Pick multiple", ["A", "B", "C", "D"]);
// Returns: ["A", "C"] (array of selected values)

// Custom HTML form
const formData = await form(`
  <form>
    <input type="text" name="username" placeholder="Username">
    <input type="password" name="password" placeholder="Password">
    <button type="submit">Login</button>
  </form>
`);
// Returns: { username: "john", password: "secret" }

// Template with tabstops (VSCode snippet syntax)
const filled = await template(`
Hello \${1:name},

Thank you for \${2:reason}.

Best regards,
\${3:Your Name}
`);
```

---

## Scriptlets

Scriptlets are markdown-based mini-scripts that execute shell commands. They're perfect for quick automations that don't need a full TypeScript file.

### Creating a Scriptlet

Create a `.md` file in `~/.sk/kit/main/scriptlets/`:

```markdown
<!-- 
name: Open Project
description: Open a project in VS Code
author: Your Name
-->

# Open Project

Opens the selected project in VS Code.

```bash
code ~/Projects/{{project}}
```
```

### Variable Substitution

Use `{{variableName}}` syntax for user input:

```markdown
<!-- name: Git Clone -->

# Clone Repository

```bash
cd ~/Projects
git clone {{url}}
cd $(basename {{url}} .git)
code .
```
```

When this scriptlet runs, Script Kit will prompt for `url` before executing.

### Multiple Variables

```markdown
<!-- name: Create Note -->

# Create Note

```bash
echo "# {{title}}" > ~/Notes/{{filename}}.md
echo "" >> ~/Notes/{{filename}}.md
echo "Created: $(date)" >> ~/Notes/{{filename}}.md
echo "" >> ~/Notes/{{filename}}.md
echo "{{content}}" >> ~/Notes/{{filename}}.md
code ~/Notes/{{filename}}.md
```
```

### Scriptlet Metadata

Use HTML comments for metadata:

```markdown
<!--
name: My Scriptlet
description: What it does
author: Your Name
shortcut: cmd+shift+s
icon: Terminal
tags: utility, shell
-->
```

---

## Configuration (config.ts)

Create `~/.sk/kit/config.ts` to customize Script Kit:

```typescript
import type { Config } from "@scriptkit/kit";

export default {
  // Required: Global hotkey to show/hide Script Kit
  hotkey: {
    modifiers: ["meta"],    // "meta" (Cmd/Win), "ctrl", "alt", "shift"
    key: "Semicolon"        // Key codes: "KeyK", "Digit0", "Space", etc.
  },

  // UI Settings
  padding: {
    top: 8,                 // Top padding in pixels (default: 8)
    left: 12,               // Left padding in pixels (default: 12)
    right: 12               // Right padding in pixels (default: 12)
  },
  editorFontSize: 16,       // Editor prompt font size (default: 14)
  terminalFontSize: 14,     // Terminal prompt font size (default: 14)
  uiScale: 1.0,             // UI scale factor (default: 1.0)

  // Built-in Features
  builtIns: {
    clipboardHistory: true, // Enable clipboard history (default: true)
    appLauncher: true,      // Enable app launcher (default: true)
    windowSwitcher: true    // Enable window switcher (default: true)
  },

  // Clipboard History Settings
  clipboardHistoryMaxTextLength: 100000,  // Max text length in bytes (default: 100000)

  // Process Limits
  processLimits: {
    maxMemoryMb: 512,              // Max memory per script (optional)
    maxRuntimeSeconds: 300,        // Max runtime in seconds (optional)
    healthCheckIntervalMs: 5000    // Health check interval (default: 5000)
  },

  // Frecency Settings (for script ranking)
  frecency: {
    enabled: true,           // Enable frecency tracking (default: true)
    halfLifeDays: 7.0,       // Decay half-life in days (default: 7.0)
    maxRecentItems: 10       // Max items in RECENT section (default: 10)
  },

  // Secondary Window Hotkeys
  notesHotkey: {
    modifiers: ["meta", "shift"],
    key: "KeyN"              // Default: Cmd+Shift+N
  },
  aiHotkey: {
    modifiers: ["meta", "shift"],
    key: "Space"             // Default: Cmd+Shift+Space
  },

  // Custom Paths
  bun_path: "/opt/homebrew/bin/bun",  // Custom bun path (optional)
  editor: "code",                      // Editor command (default: $EDITOR or "code")

  // Per-Command Configuration
  commands: {
    "builtin/clipboard-history": {
      shortcut: {
        modifiers: ["meta", "shift"],
        key: "KeyV"
      }
    },
    "app/com.apple.Safari": {
      hidden: true  // Hide Safari from app launcher
    }
  }
} satisfies Config;
```

### Hotkey Configuration

Valid modifier keys:
- `"meta"` - Cmd on macOS, Win on Windows
- `"ctrl"` - Control key
- `"alt"` - Option on macOS, Alt on Windows
- `"shift"` - Shift key

Common key codes:
- Letters: `"KeyA"` through `"KeyZ"`
- Numbers: `"Digit0"` through `"Digit9"`
- Special: `"Space"`, `"Enter"`, `"Semicolon"`
- Function keys: `"F1"` through `"F12"`

---

## Themes (theme.json)

Customize the look and feel with `~/.sk/kit/theme.json`:

```json
{
  "colors": {
    "background": {
      "main": 1973790,
      "titleBar": 1973790,
      "searchBox": 2500134,
      "logPanel": 1579032
    },
    "text": {
      "primary": 15066597,
      "secondary": 10066329,
      "tertiary": 7829367,
      "muted": 6710886,
      "dimmed": 5592405
    },
    "accent": {
      "selected": 3447003,
      "selectedSubtle": 2236962,
      "buttonText": 16777215
    },
    "ui": {
      "border": 3355443,
      "success": 5025616
    }
  },
  "opacity": {
    "background": 0.95
  },
  "vibrancy": {
    "enabled": true,
    "style": "popover"
  },
  "dropShadow": {
    "enabled": true,
    "color": 0,
    "opacity": 0.5,
    "blur": 20,
    "spread": 0
  }
}
```

### Color Formats

Colors can be specified as:
- **Decimal integers**: `16777215` (white)
- **Hex strings**: `"#FFFFFF"` or `"FFFFFF"`
- **RGB objects**: `{"r": 255, "g": 255, "b": 255}`
- **RGBA objects**: `{"r": 255, "g": 255, "b": 255, "a": 1.0}`

### Vibrancy Styles (macOS)

Available vibrancy styles:
- `"popover"` - Popover-style blur (default)
- `"menu"` - Menu-style blur
- `"sidebar"` - Sidebar-style blur
- `"header"` - Header-style blur
- `"sheet"` - Sheet-style blur
- `"window"` - Window-style blur
- `"hud"` - HUD-style blur

### Focus-Aware Colors

For windows that dim when unfocused, use `focusAware`:

```json
{
  "focusAware": {
    "focused": {
      "background": { "main": 1973790 },
      "text": { "primary": 15066597 }
    },
    "unfocused": {
      "background": { "main": 1579032 },
      "text": { "primary": 10066329 }
    }
  }
}
```

---

## Built-in Features

### Clipboard History

Access your clipboard history from any script:

```typescript
// Get clipboard history entries
const entries = await clipboardHistory();

for (const entry of entries) {
  console.log(entry.entryId);
  console.log(entry.content);
  console.log(entry.contentType);  // "text" or "image"
  console.log(entry.timestamp);
  console.log(entry.pinned);
}

// Pin an entry (prevents auto-removal)
await clipboardHistoryPin(entryId);

// Unpin an entry
await clipboardHistoryUnpin(entryId);

// Remove a specific entry
await clipboardHistoryRemove(entryId);

// Clear all entries (except pinned)
await clipboardHistoryClear();

// Remove oversized text entries
await clipboardHistoryTrimOversize();
```

Enable in config:
```typescript
builtIns: {
  clipboardHistory: true
}
```

### App Launcher

Launch applications from Script Kit:

Enable in config:
```typescript
builtIns: {
  appLauncher: true
}
```

### Window Switcher

Manage system windows programmatically:

```typescript
// Get all windows
const windows = await getWindows();

for (const win of windows) {
  console.log(win.windowId);
  console.log(win.title);
  console.log(win.appName);
  console.log(win.bounds);  // { x, y, width, height }
  console.log(win.isMinimized);
  console.log(win.isActive);
}

// Window actions
await focusWindow(windowId);
await closeWindow(windowId);
await minimizeWindow(windowId);
await maximizeWindow(windowId);
await moveWindow(windowId, x, y);
await resizeWindow(windowId, width, height);

// Tile positions: "left", "right", "top", "bottom",
// "top-left", "top-right", "bottom-left", "bottom-right",
// "center", "maximize"
await tileWindow(windowId, "left");
```

---

## AI Window (BYOK)

Script Kit includes a built-in AI chat window that uses your own API keys (BYOK = Bring Your Own Key).

### Opening the AI Window

- **Hotkey**: `Cmd+Shift+Space` (default, configurable)
- **From script**: See [SDK Reference](#sdk-quick-reference)

### API Key Setup

Set one of these environment variables:

| Provider | Environment Variable |
|----------|---------------------|
| Anthropic (Claude) | `SCRIPT_KIT_ANTHROPIC_API_KEY` |
| OpenAI (GPT) | `SCRIPT_KIT_OPENAI_API_KEY` |

**Where to set keys:**

1. **Shell profile** (recommended):
   ```bash
   # ~/.zshrc or ~/.bashrc
   export SCRIPT_KIT_ANTHROPIC_API_KEY="sk-ant-..."
   ```

2. **Environment file**:
   ```bash
   # ~/.sk/kit/.env
   SCRIPT_KIT_ANTHROPIC_API_KEY=sk-ant-...
   ```

3. **macOS Keychain** (for extra security):
   ```bash
   security add-generic-password -a "$USER" -s "SCRIPT_KIT_ANTHROPIC_API_KEY" -w "sk-ant-..."
   ```

### Features

- **Streaming responses** with real-time token display
- **Markdown rendering** for formatted AI responses
- **Model picker** to select AI models
- **Chat history** with sidebar navigation
- **Multi-provider support** (Anthropic Claude, OpenAI GPT)

### Configuring the Hotkey

```typescript
// ~/.sk/kit/config.ts
aiHotkey: {
  modifiers: ["meta", "shift"],
  key: "Space"
}
```

---

## Notes Window

A floating notes window with Markdown support for quick note-taking.

### Opening the Notes Window

- **Hotkey**: `Cmd+Shift+N` (default, configurable)
- **From script**: See [SDK Reference](#sdk-quick-reference)

### Features

- **Markdown editing** with formatting toolbar
- **Formatting shortcuts**:
  - `Cmd+B` - Bold
  - `Cmd+I` - Italic
  - `Cmd+K` - Link
  - `Cmd+Shift+C` - Code block
- **Multiple notes** with sidebar navigation
- **Full-text search** across all notes
- **Soft delete** with trash and restore
- **Export** to plain text, Markdown, or HTML (copies to clipboard)
- **Character count** in footer

### Configuring the Hotkey

```typescript
// ~/.sk/kit/config.ts
notesHotkey: {
  modifiers: ["meta", "shift"],
  key: "KeyN"
}
```

### Storage

Notes are stored in SQLite at `~/.sk/kit/db/notes.sqlite`.

---

## File Watching

Script Kit automatically watches for changes and reloads:

### Watched Files and Directories

| Path | What Happens |
|------|--------------|
| `main/scripts/*.ts` | Scripts reload in launcher |
| `main/scripts/*.js` | Scripts reload in launcher |
| `main/scriptlets/*.md` | Scriptlets reload in launcher |
| `config.ts` | Configuration reloads (requires restart) |
| `theme.json` | Theme reloads live (no restart) |

### Auto-Reload Behavior

- **Scripts**: Changes appear immediately in the launcher
- **Theme**: Colors update live without restart
- **Config**: Most settings require an app restart

### File Watch Triggers

Use `watch` in script metadata to trigger on file changes:

```typescript
export const metadata = {
  name: "Watch Downloads",
  watch: ["~/Downloads/*"],
};

// This script runs when files change in ~/Downloads
const changedFile = process.argv[2];  // Path of changed file
await notify(`New download: ${changedFile}`);
```

---

## Multiple Environments

### Adding Additional Kits

Beyond the default `main/` kit, you can add additional kits:

```
~/.sk/kit/
├── main/              # Default kit
│   └── scripts/
├── work/              # Work scripts
│   └── scripts/
├── personal/          # Personal scripts
│   └── scripts/
└── experiments/       # Experimental scripts
    └── scripts/
```

### SK_PATH Environment Variable

Override the kit path for different environments:

```bash
# Use a custom kit location
export SK_PATH="~/my-custom-kit"

# Or in a script
SK_PATH=~/work-kit ./script-kit-gpui
```

### Per-Project Kits

Create a kit in your project directory:

```
~/Projects/my-app/
├── .kit/
│   └── scripts/
│       └── dev-server.ts
├── src/
└── package.json
```

Then use `SK_PATH` to switch to it:

```bash
export SK_PATH=~/Projects/my-app/.kit
```

---

## SDK Quick Reference

### Prompt Functions

| Function | Description | Returns |
|----------|-------------|---------|
| `arg(placeholder?, choices?)` | Text input with optional choices | `Promise<string>` |
| `div(html?, config?)` | Display HTML/Tailwind content | `Promise<void>` |
| `md(markdown)` | Convert Markdown to HTML | `string` |
| `editor(content?, language?)` | Code editor | `Promise<string>` |
| `fields(definitions)` | Multi-field form | `Promise<string[]>` |
| `form(html)` | Custom HTML form | `Promise<Record<string, string>>` |
| `path(options?)` | File/folder picker | `Promise<string>` |
| `hotkey(placeholder?)` | Capture keyboard shortcut | `Promise<HotkeyInfo>` |
| `drop()` | Drag and drop zone | `Promise<FileInfo[]>` |
| `template(template, options?)` | VSCode snippet-style editor | `Promise<string>` |
| `env(key, promptFn?)` | Get/set environment variable | `Promise<string>` |
| `mini(placeholder, choices)` | Compact prompt | `Promise<string>` |
| `micro(placeholder, choices)` | Tiny prompt | `Promise<string>` |
| `select(placeholder, choices)` | Multi-select | `Promise<string[]>` |
| `term(command?)` | Terminal emulator | `Promise<string>` |
| `chat(options?)` | Chat interface | `Promise<string>` |
| `widget(html, options?)` | Floating widget window | `Promise<WidgetController>` |
| `webcam()` | Camera capture | `Promise<Buffer>` |
| `mic()` | Audio recording | `Promise<Buffer>` |
| `eyeDropper()` | Color picker | `Promise<ColorInfo>` |
| `find(placeholder, options?)` | File search (Spotlight) | `Promise<string>` |

### System Functions

| Function | Description |
|----------|-------------|
| `beep()` | Play system beep |
| `say(text, voice?)` | Text-to-speech |
| `notify(options)` | System notification |
| `hud(message, options?)` | Brief HUD notification |
| `setStatus(options)` | Set app status |
| `menu(icon, scripts?)` | Set system menu |
| `copy(text)` | Copy to clipboard |
| `paste()` | Paste from clipboard |
| `setSelectedText(text)` | Replace selected text |
| `getSelectedText()` | Get selected text |
| `hasAccessibilityPermission()` | Check accessibility permission |
| `requestAccessibilityPermission()` | Request accessibility permission |

### Clipboard Object

| Method | Description |
|--------|-------------|
| `clipboard.readText()` | Read text from clipboard |
| `clipboard.writeText(text)` | Write text to clipboard |
| `clipboard.readImage()` | Read image from clipboard |
| `clipboard.writeImage(buffer)` | Write image to clipboard |

### Keyboard Object

| Method | Description |
|--------|-------------|
| `keyboard.type(text)` | Type text |
| `keyboard.tap(...keys)` | Press key combination |

### Mouse Object

| Method | Description |
|--------|-------------|
| `mouse.move(positions)` | Move mouse along path |
| `mouse.leftClick()` | Left click |
| `mouse.rightClick()` | Right click |
| `mouse.setPosition(pos)` | Set mouse position |

### Window Control

| Function | Description |
|----------|-------------|
| `show()` | Show main window |
| `hide()` | Hide main window |
| `blur()` | Return focus to previous app |
| `getWindowBounds()` | Get window bounds |
| `submit(value)` | Force submit |
| `exit(code?)` | Exit script |
| `wait(ms)` | Delay |
| `setPanel(html)` | Set panel content |
| `setPreview(html)` | Set preview content |
| `setPrompt(html)` | Set prompt content |
| `setActions(actions)` | Set prompt actions |
| `setInput(text)` | Set input text |

### Path Utilities

| Function | Description |
|----------|-------------|
| `home(...segments)` | Path relative to ~ |
| `skPath(...segments)` | Path relative to ~/.sk/kit |
| `kitPath(...segments)` | Alias for skPath |
| `tmpPath(...segments)` | Path in temp directory |

### File Utilities

| Function | Description |
|----------|-------------|
| `isFile(path)` | Check if path is a file |
| `isDir(path)` | Check if path is a directory |
| `isBin(path)` | Check if file is executable |
| `fileSearch(query, options?)` | Search for files |

### Clipboard History

| Function | Description |
|----------|-------------|
| `clipboardHistory()` | Get clipboard history |
| `clipboardHistoryPin(entryId)` | Pin an entry |
| `clipboardHistoryUnpin(entryId)` | Unpin an entry |
| `clipboardHistoryRemove(entryId)` | Remove an entry |
| `clipboardHistoryClear()` | Clear all entries |
| `clipboardHistoryTrimOversize()` | Remove oversized entries |

### Window Management

| Function | Description |
|----------|-------------|
| `getWindows()` | Get all system windows |
| `focusWindow(windowId)` | Focus a window |
| `closeWindow(windowId)` | Close a window |
| `minimizeWindow(windowId)` | Minimize a window |
| `maximizeWindow(windowId)` | Maximize a window |
| `moveWindow(windowId, x, y)` | Move a window |
| `resizeWindow(windowId, width, height)` | Resize a window |
| `tileWindow(windowId, position)` | Tile a window |

### Miscellaneous

| Function | Description |
|----------|-------------|
| `uuid()` | Generate a UUID |
| `compile(template)` | Compile a template string |
| `browse(url)` | Open URL in browser |
| `editFile(path)` | Open file in editor |
| `run(scriptName, ...args)` | Run another script |
| `inspect(data)` | Pretty-print data |

---

## Getting Help

- **Documentation**: You're reading it!
- **Source Code**: https://github.com/johnlindquist/script-kit-gpui
- **Community**: https://scriptkit.com
- **Issues**: https://github.com/johnlindquist/script-kit-gpui/issues

---

*This guide covers Script Kit GPUI. For the original Script Kit (Electron-based), visit https://scriptkit.com.*

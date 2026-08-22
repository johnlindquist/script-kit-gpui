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
9. [Agent Chat](#agent-chat)
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
   mkdir -p ~/.scriptkit/plugins/main/scripts
   ```

2. **Create your first script** at `~/.scriptkit/plugins/main/scripts/hello.ts`:
   ```typescript
   import "@scriptkit/sdk";

   export const metadata = {
     name: "Hello World",
     description: "My first Script Kit script",
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

You can customize this in `~/.scriptkit/config.ts` (see [Configuration](#configuration-configts)).

---

## Directory Structure

Script Kit stores all its data in `~/.scriptkit/`. Here's the layout:

```
~/.scriptkit/
├── CLAUDE.md                    # Canonical harness instructions
├── AGENTS.md                    # SDK reference for agents
├── GUIDE.md                     # User guide
├── plugins/
│   ├── scriptkit/
│   │   ├── skills/
│   │   │   ├── new-script/SKILL.md
│   │   │   ├── new-scriptlet/SKILL.md
│   │   │   ├── new-agent/SKILL.md
│   │   │   ├── update-config/SKILL.md
│   │   │   └── troubleshoot/SKILL.md
│   │   ├── plugin.json
│   ├── examples/
│   │   ├── scripts/
│   │   ├── scriptlets/
│   │   └── agents/
│   ├── main/
│   │   ├── scripts/
│   │   ├── scriptlets/
│   │   ├── skills/
│   │   └── agents/
├── profiles/
├── config.ts
├── theme.json
├── package.json
├── tsconfig.json
├── sdk/
│   └── kit-sdk.ts
├── db/
├── logs/
└── cache/
```

### Key Directories

| Directory | Purpose |
|-----------|---------|
| `plugins/main/scripts/` | Your primary scripts - create `.ts` files here |
| `plugins/main/scriptlets/` | Markdown scriptlet files with shell commands |
| `plugins/main/skills/` | Reusable Agent Chat workflows (preferred reusable AI unit) |
| `profiles/` | Agent Chat profiles — one markdown file per profile (`profiles/<id>.md`, mdflow format) |
| `plugins/main/agents/` | AI agent definitions |
| `sdk/` | Runtime SDK (auto-extracted, don't edit) |
| `db/` | SQLite databases for Notes and AI |
| `logs/` | Debug logs in JSONL format |

---

## Writing Scripts

### Creating a Script File

Create a `.ts` file in `~/.scriptkit/plugins/main/scripts/`:

```typescript
// ~/.scriptkit/plugins/main/scripts/my-script.ts
import "@scriptkit/sdk";

export const metadata = {
  name: "My Script",
  description: "What this script does",
  shortcut: "cmd shift m",
};

const result = await arg("Pick an option", ["Option A", "Option B", "Option C"]);
await div(`<div class="p-8 text-xl">${result}</div>`);
```

### Script Metadata

Use `export const metadata = { ... }` for new scripts:

```typescript
export const metadata = {
  name: "My Script",
  description: "Shown in the launcher",
  shortcut: "cmd shift m",
  alias: "ms",
  sdkCapabilities: ["arg", "div"],
  executionTopology: "typescript-script",
};
```

`sdkCapabilities` is optional and explicitly lists the SDK globals or namespace
methods the script requires. The host rejects unsupported, unknown, or duplicate
ordinary-script declarations before indexing. Invalid scriptlets remain visible
but disabled, with typed repair diagnostics and execution blocked before side
effects; `kit://failed-scripts` exposes them as `retainedIssues`. The host also
verifies that `executionTopology` can provide the necessary transport. Valid
topologies are
`typescript-script`, `typescript-scriptlet`,
`typescript-scriptlet-interactive`, `shell-scriptlet`, and `python-scriptlet`.
Launcher-opened TypeScript scriptlets run interactively and can use prompts;
the legacy `typescript-scriptlet` mode has no interactive stdin response pipe.
Shell and Python scriptlets do not receive SDK globals.

For a side-effect-free authoring diagnosis, read the host MCP resource
`kit://command-doctor`. Its typed `HostCommandDoctorReport` lists already-loaded
commands, genuine launcher action availability, capability compatibility,
repair alternatives, and `permissionPending` when no existing permission
inventory is available. `kit://sdk-reference` advertises this resource in
`authoringResources`. Resource URIs are not SDK functions: never invent
`commandDoctor()` or prompt for a permission merely to inspect readiness.

### Harness-Safe Creation Rules

- Start new scripts with `import "@scriptkit/sdk";`.
- Use `export const metadata = { name, description }`.
- Write new scripts as `.ts` files in `plugins/main/scripts/`.
- Treat legacy comment headers as compatibility-only for older files; do not generate them for new harness-authored scripts.

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
  { name: "appointment", label: "Appointment", type: "datetime-local" },
  { name: "month", label: "Month", type: "month" },
  { name: "week", label: "Week", type: "week" },
  { name: "query", label: "Search", type: "search" },
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

Scriptlets are markdown files containing one or more mini-scripts. They're perfect for quick automations that don't need a full TypeScript file, and support multiple command types including bash, TypeScript, AppleScript, and more.

### Creating a Scriptlet File

Create a `.md` file in `~/.scriptkit/plugins/main/scriptlets/`:

~~~md
---
name: My Tools
description: A collection of useful tools
icon: sparkles
---

## Say Hello

```metadata
keyword: !hello
description: Greet the user
```

```paste
Hello, World!
```

## Quick Note

```metadata
description: Save a quick note
```

```tool:quick-note
import "@scriptkit/sdk";

const note = `Quick note created ${new Date().toISOString()}`;
await writeFile(home("quick-note.txt"), note);
hud("Saved");
```
~~~

### Scriptlet Metadata

Prefer `metadata` code fences for new bundles:

~~~md
## Open Dashboard

```metadata
description: Open the admin dashboard
shortcut: cmd shift d
```

```open
https://admin.example.com/dashboard
```
~~~

Legacy HTML comment metadata still works for backwards compatibility, but do not use it for new harness-authored bundles.

Available scriptlet metadata:
- `description` - Shown below the scriptlet name
- `shortcut` - Global keyboard shortcut (e.g., `cmd+shift+k`)

### Code Block Types

Script Kit supports several code block types:

#### `bash` - Shell Commands
```markdown
\`\`\`bash
echo "Hello from bash"
ls -la ~/Projects
\`\`\`
```

#### `open` - Open URLs/Files/Apps
```markdown
\`\`\`open
https://github.com
\`\`\`

\`\`\`open
file:///Applications/Safari.app
\`\`\`
```

#### `ts` or `typescript` - TypeScript Code
```markdown
\`\`\`ts
const message = \`Scriptlet ran at \${new Date().toISOString()}\`;
await writeFile(home("scriptlet-ran.txt"), message);
hud("Saved scriptlet output");
\`\`\`
```

TypeScript scriptlets receive noninteractive SDK helpers but not an interactive
stdin response pipe. Put `arg()`, `div()`, `fields()`, `mini()`, `micro()`,
`hotkey()`, and other prompts in a regular TypeScript script instead. Shell and
Python scriptlets do not receive SDK globals.

#### `applescript` - AppleScript Commands
```markdown
\`\`\`applescript
tell application "Finder"
    activate
end tell
\`\`\`
```

#### `template` - Text Templates
```markdown
\`\`\`template
Hello {{name}},

Thank you for your inquiry about {{topic}}.

Best regards,
{{sender}}
\`\`\`
```

### Variable Substitution

Use `{{variableName}}` syntax for user input. Script Kit will prompt for each variable before execution:

```markdown
## Git Clone
<!-- description: Clone a repository -->

\`\`\`bash
cd ~/Projects
git clone {{url}}
cd $(basename {{url}} .git)
code .
\`\`\`
```

When this scriptlet runs, you'll be prompted for `url` before the commands execute.

### Multiple Scriptlets Per File

A single markdown file can contain many scriptlets, organized under one bundle:

```markdown
---
name: Quick Links
description: Common websites and tools
---

# Quick Links

## GitHub
<!-- description: Open GitHub -->
\`\`\`open
https://github.com
\`\`\`

## Gmail
<!-- description: Open Gmail -->
\`\`\`open
https://mail.google.com
\`\`\`

## Calendar
<!-- description: Open Google Calendar -->
\`\`\`open
https://calendar.google.com
\`\`\`
```

All three scriptlets appear in Script Kit as separate commands, grouped under "Quick Links".

---

## Shared Actions

Shared actions let you define reusable actions that automatically apply to ALL scriptlets in a scriptlet file. This is perfect for common operations like "Copy URL", "Open in Browser", etc.

### Creating Shared Actions

Create a companion `.actions.md` file with the same base name as your scriptlet bundle:

```
~/.scriptkit/plugins/main/scriptlets/
├── quicklinks.md           # Main scriptlet file
└── quicklinks.actions.md   # Shared actions for all quicklinks
```

### Shared Actions File Format

Shared actions use H3 headers (###) instead of H2:

```markdown
# URL Actions

### Copy URL
<!-- shortcut: cmd+c -->
<!-- description: Copy the URL to clipboard -->
\`\`\`bash
echo -n "{{content}}" | pbcopy
\`\`\`

### Open in Safari
<!-- shortcut: cmd+shift+s -->
<!-- description: Open URL in Safari -->
\`\`\`bash
open -a Safari "{{content}}"
\`\`\`

### Open in Chrome
<!-- description: Open URL in Google Chrome -->
\`\`\`bash
open -a "Google Chrome" "{{content}}"
\`\`\`
```

### The `{{content}}` Variable

In shared actions, `{{content}}` is automatically replaced with the parent scriptlet's code content. For example, if your scriptlet is:

```markdown
## GitHub
\`\`\`open
https://github.com
\`\`\`
```

And your shared action is:
```markdown
### Copy URL
\`\`\`bash
echo -n "{{content}}" | pbcopy
\`\`\`
```

Then `{{content}}` becomes `https://github.com` when the action runs.

### How Shared Actions Work

1. When you open the Actions menu on a scriptlet
2. Script Kit loads both the scriptlet's inline actions (H3 headers within the scriptlet) AND the shared actions from the companion `.actions.md` file
3. Inline actions take precedence over shared actions with the same name
4. All actions appear together in the Actions menu

### Action Precedence

If a scriptlet has an inline action with the same command name as a shared action, the inline action wins:

```markdown
## Special Link
<!-- This inline action overrides the shared "Copy URL" action -->

\`\`\`open
https://example.com/special
\`\`\`

### Copy URL
<!-- This takes precedence over shared Copy URL -->
\`\`\`bash
echo "Custom copy for this link: {{content}}" | pbcopy
\`\`\`
```

### Built-in Shared Actions

Script Kit ships with shared actions for several built-in scriptlets:

| Scriptlet | Shared Actions |
|-----------|---------------|
| Quick Links | Copy URL, Open in Safari/Chrome/Firefox |
| CleanShot | Copy URL scheme, Open Settings |
| Conductor | Copy URL, Open in Browser |

### Creating Your Own Shared Actions

1. Create your scriptlet file: `my-tools.md`
2. Create the companion: `my-tools.actions.md`
3. Add H3 actions with optional shortcuts and descriptions
4. Use `{{content}}` to access the parent scriptlet's code

Example for a code snippets scriptlet bundle:

```markdown
# Snippet Actions

### Copy to Clipboard
<!-- shortcut: cmd+c -->
\`\`\`bash
echo -n "{{content}}" | pbcopy
\`\`\`

### Insert at Cursor
<!-- shortcut: cmd+shift+v -->
\`\`\`ts
await setSelectedText("{{content}}");
\`\`\`

### Open in Editor
\`\`\`ts
const tmp = \`/tmp/snippet-\${Date.now()}.txt\`;
await Bun.write(tmp, "{{content}}");
await \$\`code \${tmp}\`;
\`\`\`
```

---

## Agents (mdflow)

Agent files are reusable backend-specific markdown prompts. They are not TypeScript scripts and they do not use `export const metadata`.

Write them in `~/.scriptkit/plugins/main/agents/` with a backend suffix in the filename:

- `review.claude.md`
- `plan.agy.md`
- `code.codex.md`
- `assist.copilot.md`
- `chat.i.agy.md` for interactive AGY agents

Minimal example:

```markdown
---
_sk_name: "Review PR"
_sk_description: "Review staged changes and call out risks"
_sk_icon: "git-pull-request"
model: sonnet
---

Review the current git diff.

Return:
1. findings ordered by severity
2. concrete fixes
3. tests to add
```

For more patterns, read `~/.scriptkit/plugins/examples/agents/`.

---

## Configuration (config.ts)

`~/.scriptkit/config.ts` is the single home for every Script Kit setting —
hotkeys, theme, search sources, dictation, AI profiles, and more.

> **Tip: let AI edit it for you.** Open Agent Chat (`Cmd+Shift+Space` or the
> "Agent Chat" entry in the launcher) and just ask — for example *"switch my
> theme preset to a light one"*, *"rebind the rewrite hotkey"*, or *"turn off
> browser history in search"*. The agent updates `config.ts` through Script
> Kit's validated config tools (`kit/config_set`), so mistakes are rejected
> instead of breaking your setup. You can also press `Cmd+K` in the launcher
> and run **Change Settings with AI**.

Create `~/.scriptkit/config.ts` to customize Script Kit:

```typescript
import type { Config } from "@scriptkit/sdk";

export default {
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon",
  },

  padding: {
    top: 8,
    left: 12,
    right: 12,
  },
  editorFontSize: 16,
  terminalFontSize: 14,
  uiScale: 1.0,

  builtIns: {
    clipboardHistory: true,
    appLauncher: true,
    windowSwitcher: true,
  },

  clipboardHistoryMaxTextLength: 100000,

  processLimits: {
    maxMemoryMb: 512,
    maxRuntimeSeconds: 300,
    healthCheckIntervalMs: 5000,
  },

  suggested: {
    enabled: true,
    maxItems: 10,
    minScore: 0.1,
    halfLifeDays: 7,
    trackUsage: true,
    excludedCommands: ["builtin-quit-script-kit"],
  },

  // No default notes shortcut — set one only if you want it.
  // notesHotkey: { modifiers: ["meta", "shift"], key: "KeyN" },

  // Defaults to Cmd+Shift+Space when enabled and unset.
  // aiHotkey: { modifiers: ["meta", "shift"], key: "Space" },
  aiHotkeyEnabled: true,

  // Defaults to Cmd+Shift+L when enabled and unset.
  // logsHotkey: { modifiers: ["meta", "shift"], key: "KeyL" },
  logsHotkeyEnabled: true,

  // Defaults to Cmd+Shift+; when enabled and unset.
  dictationHotkey: { modifiers: ["meta", "shift"], key: "Semicolon" },
  dictationHotkeyEnabled: true,

  watcher: {
    debounceMs: 500,
    stormThreshold: 200,
    initialBackoffMs: 100,
    maxBackoffMs: 30000,
    maxNotifyErrors: 10,
  },

  layout: {
    standardHeight: 500,
    maxHeight: 700,
  },

  theme: {
    presetId: "nord",
  },

  dictation: {
    selectedDeviceId: "usb-mic",
  },

  ai: {
    selectedAgentChatAgentId: "codex-agent_chat",
    selectedModelId: "gpt-5.4",
  },

  windowManagement: {
    snapMode: "expanded",
  },

  bun_path: "/opt/homebrew/bin/bun",
  editor: "code",

  commands: {
    "builtin/clipboard-history": {
      shortcut: {
        modifiers: ["meta", "shift"],
        key: "KeyV",
      },
    },
    "builtin/empty-trash": {
      confirmationRequired: true,
    },
  },
} satisfies Config;
```

Behavior:
- `dictationHotkeyEnabled: true` does not create a shortcut by itself; set `dictationHotkey` to register one
- No `selectedDeviceId` → use the macOS default microphone
- Use the built-in **Select Microphone** action to persist a device
- Saved microphone missing → fall back to the best available device and clear the stale preference

### Hotkey Configuration

Valid modifier keys:
- `"meta"` - Cmd on macOS, Win on Windows
- `"ctrl"` - Control key
- `"alt"` - Option on macOS, Alt on Windows
- `"shift"` - Shift key

Common key codes:
- Letters: `"KeyA"` through `"KeyZ"`
- Numbers: `"Digit0"` through `"Digit9"`
- Special: `"Space"`, `"Enter"`, `"Semicolon"`, `"Comma"`, `"Period"`, `"Slash"`
- Function keys: `"F1"` through `"F12"`

### Common Mistakes

- Putting skills at the workspace root instead of in `~/.scriptkit/plugins/scriptkit/skills/`
- Using `command` / `control` instead of `meta` / `ctrl`
- Editing `theme.json` when you meant to change `theme.presetId` in `config.ts`

---

## Themes (theme.json)

Customize the look and feel with `~/.scriptkit/theme.json`:

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

## Agent Chat

Script Kit includes Agent Chat - a built-in chat surface for command-line agents. Agent Chat is the primary AI interface, accessible from the launcher and from scripts via the SDK.

### Opening Agent Chat

- **From script**: Use `aiStartChat()` to create an Agent Chat conversation; it
  may invoke a configured provider unless `noResponse: true` is explicit
- **Focus existing Agent Chat**: Use `aiFocus()`
- **From skills**: Selecting a skill opens Agent Chat
- **Hotkey**: `Cmd+Shift+Space` (default, configurable)
- **Command+Enter behavior**: `Command+Enter` from the launcher opens Agent Chat with current context

### Agent Setup

Agent defaults, model preferences, profiles, and related runtime settings live in `~/.scriptkit/config.ts`. Agent catalog recovery may also open `~/.scriptkit/agent_chat/agents.json` when a local agent entry needs repair.

### Features

- **Streaming responses** with real-time token display
- **Markdown rendering** for formatted AI responses
- **Model picker** to select agent-advertised models
- **Chat history** with sidebar navigation
- **Multiple agent support** (Claude Code, OpenCode, AGY, Codex, and compatible adapters)
- **Explicit context** — attach only the user-selected launcher row, note, file,
  or other reviewed context; Tab never authorizes implicit desktop capture

### Agent Chat SDK

Scripts can programmatically create and manage Agent Chat conversations. Both
`aiStartChat()` and `aiSendMessage()` can submit a request to a configured AI
provider, which may use paid inference. Use `noResponse: true` to intentionally
stage a new conversation without requesting a provider response; send a follow-up
only after an explicit user action:

```typescript
// Stage a new chat with explicit context parts without provider inference
const result = await aiStartChat("Summarize this context", {
  systemPrompt: "Be concise",
  modelId: "claude-3-5-sonnet-20241022",
  noResponse: true,
  parts: [{ kind: "resourceUri", uri: "kit://context?profile=minimal", label: "Current Context" }],
});
console.log(result.chatId); // use for follow-up messages

// Intentional user-approved provider request with an explicit file attachment
await aiSendMessage(result.chatId, "Now review this file", undefined, [
  { kind: "filePath", path: "/tmp/example.rs", label: "example.rs" },
]);

// Append a message without triggering AI response
await aiAppendMessage(result.chatId, "System note", "system");

// Subscribe to streaming events
const unsub = await aiOn("streamChunk", (event) => {
  console.log(event.chunk);
}, result.chatId);

// Query chat state
const isOpen = await aiIsOpen();
const chats = await aiListChats();
const messages = await aiGetConversation(result.chatId);

// Focus or delete
await aiFocus();
await aiDeleteChat(result.chatId);
```

See the full function list in [Agent Chat SDK](#agent-chat-sdk-1) under SDK Quick Reference.

### Configuring the Hotkey

```typescript
// ~/.scriptkit/config.ts
aiHotkey: {
  modifiers: ["meta", "shift"],
  key: "Space"
}
```

---

## Notes Window

A floating notes window with Markdown support for quick note-taking.

### Opening the Notes Window

- **Hotkey**: No default — set `notesHotkey` in `config.ts` to enable
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
// ~/.scriptkit/config.ts
notesHotkey: {
  modifiers: ["meta", "shift"],
  key: "KeyN"
}
```

### Storage

Notes are stored in SQLite at `~/.scriptkit/db/notes.sqlite`.

### Notes Automation

Notes are currently exposed to scripts through the automation protocol rather than dedicated JavaScript globals.

```json
{"type":"getElements","requestId":"elm-notes","target":{"type":"kind","kind":"notes"},"limit":10}
```

```json
{"type":"waitFor","requestId":"w-notes","target":{"type":"kind","kind":"notes"},"condition":{"type":"elementExists","semanticId":"input:notes-editor"},"timeout":3000,"pollInterval":25}
```

```json
{"type":"batch","requestId":"b-notes","target":{"type":"kind","kind":"notes"},"commands":[{"type":"setInput","text":"Hello from automation"}]}
```

Use the semantic IDs `panel:notes-window` and `input:notes-editor`. Do not read or write `~/.scriptkit/db/notes.sqlite` directly from scripts.

## Agent Chat Handoffs

Use the Notes UI actions for cross-surface handoffs:

- **Send to Agent Chat** — opens or focuses Agent Chat with the active note content
- **Save as Note** — creates or updates a note from Agent Chat content

These are UI actions, not JavaScript globals. The current public Notes script surface is the automation target (`kind: notes`) unless real Notes functions are added to `scripts/kit-sdk.ts`.

---

## File Watching

Script Kit automatically watches for changes and reloads:

### Watched Files and Directories

| Path | What Happens |
|------|--------------|
| `plugins/main/scripts/*.ts` | Scripts reload in launcher |
| `plugins/main/scripts/*.js` | Scripts reload in launcher |
| `plugins/main/scriptlets/*.md` | Scriptlets reload in launcher |
| `config.ts` | Most settings reload live (hotkey needs restart) |
| `theme.json` | Theme reloads live (no restart) |

### Auto-Reload Behavior

- **Scripts**: Changes appear immediately in the launcher
- **Theme**: Colors update live without restart
- **Config**: Most settings reload live (font sizes, padding, built-ins)
- **Main hotkey**: Requires app restart to take effect

### File Watch Triggers

Use `watch` in script metadata to trigger on file changes:

```typescript
export const metadata = {
  name: "Watch Downloads",
  watch: ["~/Downloads/*"],
};

// This script runs when files change in ~/Downloads
const changedFile = process.argv[2];  // Path of changed file
hud(`New download: ${changedFile}`);
```

---

## Multiple Environments

### Adding Additional Kits

Beyond the default `main/` kit, you can add additional kits under `~/.scriptkit/plugins/`:

```
~/.scriptkit/plugins/
├── main/              # Default kit
│   ├── scripts/
│   ├── scriptlets/
│   └── agents/
├── work/              # Work scripts
│   ├── scripts/
│   └── scriptlets/
├── personal/          # Personal scripts
│   ├── scripts/
│   └── scriptlets/
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
| `chat(options?)` | Chat interface (prompt-level) | `Promise<ChatResult>` |
| `widget(html, options?)` | Unsupported; use `div(html)` | Rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `webcam()` | Unsupported; use an explicitly configured external camera tool | Rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `mic()` | Unsupported; use an explicitly configured external audio tool | Rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `eyeDropper()` | Unsupported; request a color through `arg(...)` | Rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `find(placeholder, options?)` | Unsupported; use `fileSearch(query, options)` or `path(options)` | Rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |

### System Functions

| Function | Description |
|----------|-------------|
| `beep()` | Request platform system beep; resolves to a dispatch receipt |
| `say(text, voice?)` | Request platform text-to-speech; resolves to a dispatch receipt |
| `notify(options)` | OS-level system notification (Notification Center); resolves to a dispatch receipt. Distinct from `hud(message)` — use `hud` for in-launcher feedback, `notify` when the message should persist outside the launcher. |
| `hud(message, options?)` | Brief HUD notification |
| `setStatus(options)` | Unsupported in GPUI; returns `ERR_UNSUPPORTED_SDK_FEATURE` |
| `menu(icon, scripts?)` | Unsupported in GPUI; returns `ERR_UNSUPPORTED_SDK_FEATURE` |
| `exec(command, args?)` | Execute an explicit subprocess without invoking a shell; returns stdout, stderr, and exit code |
| `copy(text)` | Copy to clipboard |
| `paste()` | Read clipboard text; does not inject input or send a paste shortcut |
| `setSelectedText(text)` | Replace explicitly selected text; requires Accessibility permission |
| `getSelectedText()` | Read explicitly selected text; requires Accessibility permission |
| `hasAccessibilityPermission()` | Check accessibility permission |
| `requestAccessibilityPermission()` | Explicitly request Accessibility permission; may open macOS System Settings |

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
| `keyboard.type(text)` | Unsupported in GPUI; rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `keyboard.tap(...keys)` | Unsupported in GPUI; rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |

### Mouse Object

| Method | Description |
|--------|-------------|
| `mouse.move(positions)` | Unsupported in GPUI; rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `mouse.leftClick()` | Unsupported in GPUI; rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `mouse.rightClick()` | Unsupported in GPUI; rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |
| `mouse.setPosition(pos)` | Unsupported in GPUI; rejects with `ERR_UNSUPPORTED_SDK_FEATURE` |

### Window Control

| Function | Description |
|----------|-------------|
| `show()` | Show main window |
| `hide()` | Hide main window |
| `blur()` | Return focus to previous app |
| `getWindowBounds()` | Get window bounds |
| `submit(value)` | Force submit |
| `exit(code?)` | Exit script |
| `setPanel(html)` | Unsupported; throws `ERR_UNSUPPORTED_SDK_FEATURE`. Use `div(html)` |
| `setPreview(html)` | Unsupported; throws `ERR_UNSUPPORTED_SDK_FEATURE`. Use `arg(...)` choice previews |
| `setPrompt(html)` | Unsupported; throws `ERR_UNSUPPORTED_SDK_FEATURE`. Open a supported prompt |
| `setActions(actions)` | Set prompt actions |
| `setInput(text)` | Set input text |

### Path Utilities

| Function | Description |
|----------|-------------|
| `home(...segments)` | Path relative to ~ |
| `skPath(...segments)` | Path relative to ~/.scriptkit |
| `kitPath(...segments)` | Alias for skPath |
| `tmpPath(...segments)` | Path in temp directory |

### File Utilities

| Function | Description |
|----------|-------------|
| `isFile(path)` | Check if path is a file |
| `isDir(path)` | Check if path is a directory |
| `isBin(path)` | Check if file is executable |
| `readFile(path, encoding?)` | Read text from an explicit path; UTF-8 by default |
| `writeFile(path, content, encoding?)` | Write text to an explicit path; UTF-8 by default |
| `fileSearch(query, options?)` | Search for files |

### Clipboard History

| Function | Description |
|----------|-------------|
| `clipboardHistory()` | Get clipboard history |
| `clipboardHistoryPin(entryId)` | Pin an entry |
| `clipboardHistoryUnpin(entryId)` | Unpin an entry |
| `clipboardHistoryRemove(entryId)` | Remove an entry |
| `clipboardHistoryClear()` | Clear non-pinned entries while preserving pinned entries |
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

### Agent Chat SDK

| Function | Description |
|----------|-------------|
| `aiIsOpen()` | Check if Agent Chat is open |
| `aiGetActiveChat()` | Get the active chat info |
| `aiListChats(limit?, includeDeleted?)` | List all chats |
| `aiGetConversation(chatId?, limit?)` | Get messages from a chat |
| `aiStartChat(message, options?)` | Start Agent Chat; may trigger paid provider inference unless `options.noResponse` is `true` |
| `aiAppendMessage(chatId, content, role)` | Append a message without triggering a response |
| `aiSendMessage(chatId, content, imagePath?, parts?)` | Send a user-approved follow-up; triggers potentially paid provider inference |
| `aiOn(eventType, handler, chatId?)` | Subscribe to Agent Chat events |
| `aiSetSystemPrompt(chatId, prompt)` | Set the system prompt for a chat |
| `aiFocus()` | Focus Agent Chat |
| `aiGetStreamingStatus(chatId?)` | Check if a chat is currently streaming |
| `aiDeleteChat(chatId, permanent?)` | Delete a chat |

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

### Main menu tips

Script Kit shows a rotating discoverability tip in the empty root main-menu footer. Disable only that footer line in `~/.scriptkit/config.ts` with:

```typescript
tips: { enabled: false }
```

The **Tips** built-in remains available from the launcher.

The catalog lives at `~/.scriptkit/tips.json` (user-owned). Tip text may use
the `{mainHotkey}` placeholder, which resolves at load time to your registered
main hotkey (e.g. `⌘;`).

- **Documentation**: You're reading it!
- **Source Code**: https://github.com/johnlindquist/script-kit-gpui
- **Community**: https://scriptkit.com
- **Issues**: https://github.com/johnlindquist/script-kit-gpui/issues

---

*This guide covers Script Kit GPUI. For the original Script Kit (Electron-based), visit https://scriptkit.com.*

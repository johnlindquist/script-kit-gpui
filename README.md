# Script Kit GPUI

A complete rewrite of [Script Kit](https://scriptkit.com) using the [GPUI](https://gpui.rs) framework from Zed. This version combines the SDK and app into a single repository for a streamlined development experience.

## Project Goals

### Complete Rewrite with GPUI

Script Kit GPUI is built from the ground up using Zed's GPUI framework, delivering:

- **Blazing Fast Performance** - Native Rust performance with GPU-accelerated rendering
- **Sub-Second Compilation** - Hot reload development with cargo-watch rebuilds in 2-5 seconds
- **Single Repository** - SDK and app live together, making contributions and customizations straightforward
- **Bun Runtime** - Scripts execute via Bun for fast startup and modern JavaScript/TypeScript support

### Simplified SDK Philosophy

This rewrite takes a **focused approach** to the SDK:

- **Prompts Are the Core** - The SDK focuses on the prompt APIs (`arg`, `div`, `editor`, `term`, `fields`, `form`, `drop`, `hotkey`, etc.)
- **Bring Your Own Libraries** - Utilities and helpers are no longer bundled; install what you need via `bun add`
- **Full Control** - You manage your own dependencies, versions, and tooling
- **Lighter Weight** - The SDK stays small and focused on UI primitives

### Not Backwards Compatible

> **Important**: This is NOT a drop-in replacement for previous Script Kit versions.

What's preserved:
- Core prompt APIs (`arg`, `div`, `editor`, `fields`, `form`, `drop`, `hotkey`, `mini`, `micro`, `path`, `term`, `chat`)
- Choice/option structure and props
- Basic script metadata format

What's changed:
- No bundled utilities (file helpers, clipboard wrappers, etc.)
- No `kit` global with hundreds of helpers
- Scripts must explicitly import dependencies via Bun
- Configuration is TypeScript-based (`~/.scriptkit/config.ts`)

## Quick Start

New to Script Kit GPUI? Start with the user guides:

- [Getting Started](docs/guides/getting-started.md)
- [Feature Tour](docs/guides/feature-tour.md)
- [Main Menu Input](docs/guides/main-menu-input.md)
- [SDK Scripting](docs/guides/sdk-scripting.md)
- [MCP and Agent Context](docs/guides/mcp-and-agent-context.md)
- [Dictation](docs/guides/dictation.md)
- [Computer Use](docs/guides/computer-use.md)

### Prerequisites

- **macOS** (Linux/Windows support planned)
- **Rust** (1.70+) - Install from https://rustup.rs
- **Bun** - Install from https://bun.sh
- **cargo-watch** (optional, for hot reload):
  ```bash
  cargo install cargo-watch
  ```

### Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/johnlindquist/script-kit-gpui.git
   cd script-kit-gpui
   ```

2. **Create the kit directory**
   ```bash
   mkdir -p ~/.scriptkit/plugins/main
   ```

3. **Build and run**
   ```bash
   cargo build --release
   ./target/release/script-kit-gpui
   ```

   Or for development with hot reload:
   ```bash
   ./dev.sh
   ```

4. **Configure your hotkey** (optional)
   
   Create `~/.scriptkit/config.ts`:
   ```typescript
   export default {
     hotkey: {
       modifiers: ["meta"],  // Cmd on macOS
       key: "Semicolon"      // Press Cmd+; to toggle
     }
   };
   ```

### Your First Script

Create `~/.scriptkit/plugins/main/scripts/hello.ts`:

```typescript
import "@scriptkit/sdk";

export const metadata = {
  name: "Hello World",
  description: "My first script"
};

const name = await arg("What's your name?");
await div(`<h1 class="text-4xl p-8">Hello, ${name}!</h1>`);
```

Press your hotkey, type "hello", and press Enter.

## Writing Scripts

### Prompts (The Core API)

```typescript
// Text input with choices
const fruit = await arg("Pick a fruit", ["Apple", "Banana", "Cherry"]);

// Rich choices with metadata
const app = await arg("Launch app", [
  { name: "VS Code", value: "code", description: "Editor" },
  { name: "Terminal", value: "term", description: "Shell" },
]);

// HTML display with Tailwind CSS
await div(`
  <div class="p-8 bg-gradient-to-r from-blue-500 to-purple-600">
    <h1 class="text-white text-3xl font-bold">Beautiful UI</h1>
  </div>
`);

// Multi-line editor
const code = await editor("// Write your code here", "typescript");

// Form with multiple fields
const [name, email] = await fields([
  { name: "name", label: "Name", placeholder: "John Doe" },
  { name: "email", label: "Email", type: "email" },
]);

// File/folder picker
const file = await path({ startPath: "~/Documents" });

// Capture a hotkey
const shortcut = await hotkey("Press a shortcut");

// Terminal emulator
await term("htop");

// Drop zone for files
const files = await drop();
```

### Using Bun Packages

Since utilities aren't bundled, install what you need:

```bash
cd ~/.scriptkit
bun add zod lodash-es date-fns
```

Then use them in your scripts:

```typescript
import { z } from "zod";
import { groupBy } from "lodash-es";

export const metadata = {
  name: "Process Data",
  description: "Using external packages"
}

const data = await arg("Enter JSON data");
const parsed = z.object({ items: z.array(z.string()) }).parse(JSON.parse(data));

await div(`<pre>${JSON.stringify(groupBy(parsed.items, x => x[0]), null, 2)}</pre>`);
```

### Script Metadata

Use `export const metadata = { ... }` to define script properties:

```typescript
export const metadata = {
  name: "My Script",
  description: "What it does",
  author: "Your Name",
  shortcut: "cmd+shift+m",
  schedule: "0 9 * * *",
  // Additional options:
  // hidden: true,        // Hide from script list
  // tags: ["utility"],   // Categorize scripts
};

// Your code here...
```

> **Note:** The `export const metadata` object is typed as `ScriptMetadata`, providing TypeScript type checking and IDE support. Comment-based metadata (`// Name:`, `// Description:`) is a compatibility-only pattern.

## Configuration

### `~/.scriptkit/config.ts`

```typescript
export default {
  // Global hotkey to show/hide Script Kit
  hotkey: {
    modifiers: ["meta"],      // "meta", "ctrl", "alt", "shift"
    key: "Semicolon"          // Key codes: "KeyK", "Digit0", "Semicolon", etc.
  },
  
  // UI customization
  padding: { top: 8, left: 12, right: 12 },
  editorFontSize: 16,
  terminalFontSize: 14,
  uiScale: 1.0,
  
  // Built-in features
  builtIns: {
    clipboardHistory: true,
    appLauncher: true,
    windowSwitcher: true
  },
  
  // Custom paths
  bun_path: "/opt/homebrew/bin/bun",
  editor: "code",

  // Runtime preferences also live here
  theme: { presetId: "nord" },
  dictation: { selectedDeviceId: "usb-mic" },
  ai: {
    selectedProfileId: "script-kit",
    selectedBackend: "pi",
    selectedModelId: "openai-codex/gpt-5.4"
  },
  windowManagement: { snapMode: "expanded" }
};
```

### Agent Chat Configuration

Agent Chat with Pi Backend uses profiles in `~/.scriptkit/config.ts`. Codex is
the default provider; alternative providers belong in advanced profile
configuration rather than the primary setup flow.

```typescript
ai: {
  selectedBackend: "pi",
  selectedProfileId: "script-kit",
  selectedModelId: "openai-codex/gpt-5.4",
  profiles: [
    {
      id: "script-kit",
      name: "Script Kit",
      backend: "pi",
      provider: "openai-codex",
      model: "gpt-5.4",
      cwd: "~/.scriptkit",
      tools: ["read", "write", "edit", "bash", "grep", "find", "ls", "hashline_edit"],
      disableExtensions: true,
      disableSkills: true,
      disablePromptTemplates: true
    }
  ]
}
```

Pi-backed Agent Chat uses the bundled Pi sidecar in release builds. Developers
and support builds can override it with `SCRIPT_KIT_PI_BINARY`, `ai.piBinary`,
or a per-profile `piBinary`.

Use the Agent Chat actions menu to change profile or model. Compatibility
settings for other command-line agents may still be honored when explicitly
configured, but they are not the primary Agent Chat setup flow.

### Advanced Provider Configuration

These keys are for legacy direct-provider AI features and advanced provider
setups, not for the default Codex-backed Agent Chat flow. Agent Chat setup
should go through profiles in `config.ts`.

```bash
# Direct providers
export SCRIPT_KIT_OPENAI_API_KEY="sk-..."
export SCRIPT_KIT_ANTHROPIC_API_KEY="sk-ant-..."

# Additional providers
export SCRIPT_KIT_GOOGLE_API_KEY="..."
export SCRIPT_KIT_GROQ_API_KEY="..."
export SCRIPT_KIT_OPENROUTER_API_KEY="..."
```

After adding, restart your terminal or run `source ~/.zshrc`.

### `~/.scriptkit/theme.json`

Customize the look and feel:

```json
{
  "colors": {
    "background": { "main": "#1E1E1E" },
    "text": { "primary": "#FFFFFF" },
    "accent": { "selected": "#FBBF24" }
  },
  "opacity": { "main": 0.3, "selected": 0.12 },
  "vibrancy": { "enabled": true, "material": "popover" }
}
```

See `kit-init/theme.example.json` for all available options.

## Development

### Hot Reload

```bash
./dev.sh  # Starts cargo-watch, rebuilds on file changes
```

Changes to Rust code trigger a rebuild (~2-5 seconds). Theme and script changes reload instantly without restart.

### Project Structure

```
script-kit-gpui/
├── src/                    # Rust application source
│   ├── main.rs            # Entry point, window setup
│   ├── protocol/          # JSON message protocol
│   ├── prompts/           # Prompt implementations
│   ├── terminal/          # Terminal emulator
│   ├── notes/             # Notes window feature
│   └── ai/                # Agent Chat runtime + context assembly
├── scripts/
│   └── kit-sdk.ts         # The SDK (preloaded into scripts)
├── tests/
│   ├── smoke/             # End-to-end tests
│   └── sdk/               # SDK method tests
└── ~/.scriptkit/               # User workspace (plugin-based)
    └── kit/
        ├── main/          # Primary plugin
        │   ├── scripts/    # Executable TypeScript scripts
        │   ├── scriptlets/ # Scriptlet bundles
        │   ├── skills/     # Agent-first reusable AI units
        │   └── agents/     # Legacy agent definitions (compatibility)
        ├── config.ts       # Configuration
        ├── theme.json      # Theme customization
        ├── package.json
        └── tsconfig.json
```

### Running Tests

```bash
# Validation gate used for release tags
make verify

# Full local ship check (includes macOS bundle build + sanity check)
make ship-check

# SDK tests via stdin protocol
echo '{"type":"run","path":"'$(pwd)'/tests/smoke/hello-world.ts"}' | ./target/debug/script-kit-gpui
```

### Building for Release

```bash
# Optimized binary
cargo build --release --bin script-kit-gpui

# macOS app bundle
cargo install cargo-bundle
bash scripts/prepare-pi-sidecar.sh
cargo bundle --release --bin script-kit-gpui
bash scripts/install-pi-sidecar-into-bundle.sh

# Verify bundle contents
bash scripts/verify-macos-bundle.sh
```

## Features

### Built-in Capabilities

- **Clipboard History** - Access your clipboard history (enable in config)
- **App Launcher** - Quick launch applications
- **Window Switcher** - Switch between open windows (enable in config)
- **Notes Window** - Floating notes with Markdown support (`Cmd+Shift+N`)
- **Agent Chat** - Press Command+Enter to open Agent Chat with the current context staged for the active profile
- **System Tray** - Menu bar icon with quick actions
- **Global Hotkeys** - Trigger scripts from anywhere

### Prompt Types

| Prompt | Description |
|--------|-------------|
| `arg(placeholder, choices?)` | Text input with optional choices |
| `div(html)` | Display HTML/Tailwind content |
| `editor(content?, language?)` | Multi-line code editor |
| `fields(definitions)` | Form with multiple inputs |
| `form(html)` | Custom HTML form |
| `path(options?)` | File/folder picker |
| `drop()` | Drag and drop zone |
| `hotkey(placeholder?)` | Capture keyboard shortcut |
| `mini(placeholder, choices)` | Native compact-choice prompt |
| `micro(placeholder, choices)` | Native minimal-choice prompt |
| `term(command?)` | Interactive terminal |
| `chat(options?)` | Chat interface |

`mic()`, `webcam()`, `eyeDropper()`, floating `widget()` windows, and legacy
`setPanel()` / `setPreview()` / `setPrompt()` mutations are not supported by the
GPUI host. They fail explicitly with `ERR_UNSUPPORTED_SDK_FEATURE` and an
actionable alternative instead of pretending to succeed. Search **SDK
Reference** in the launcher for the host-owned, versioned capability catalog.

## AI & Context Features

Script Kit exposes desktop context and UI state to scripts and AI agents through protocol commands and MCP resources.

### Agent Chat

Agent Chat is the primary and only AI chat surface. Command+Enter routes into
Agent Chat; some internal helpers and compatibility types still use `tab_ai_*`
naming, but those are implementation details rather than separate chat products.

**Entry path:**
- Command+Enter opens Agent Chat and stages current context for the active profile.
- Command+Enter with typed launcher text can submit that text as user intent when the active surface supports it.
- Detached Agent Chat windows use the same conversation model and automation targeting contract as the in-panel Agent Chat view.

**Close semantics:**
- `Cmd+W` closes the detached Agent Chat window.
- Plain `Escape` returns the in-panel Agent Chat view to the previous launcher surface when applicable.
- The footer keeps Agent Chat aligned with the launcher chrome and action model.

**Runtime contract:**
- Agent Chat entry is driven from `src/app_impl/agent_handoff.rs` and rendered through the compatibility `AppView::AgentChatView` surface.
- Detached Agent Chat windows are managed by the compatibility implementation in `src/ai/agent_chat/ui/chat_window.rs`.
- Agent selection, model preferences, and profiles live under the `ai` block in `~/.scriptkit/config.ts` and are backed by the Agent Catalog.
- Context bundle: `~/.scriptkit/context/latest.md` (deterministic path)
- Context assembly still uses compatibility-named helpers such as `snapshot_tab_ai_ui()`, `capture_context_snapshot(CaptureContextOptions::tab_ai_submit())`, and `build_tab_ai_context_from()`.
- Compatibility-named types such as `TabAiContextBlob` remain the schema contract backing Agent Chat context capture.

### Element Introspection (`getElements`)

Scripts can query the visible UI surface to discover what elements are currently displayed — inputs, choices, buttons, panels, and lists. This enables AI-driven automation that targets elements by stable semantic IDs.

**Request:**
```json
{"type": "getElements", "requestId": "elm-1", "limit": 50}
```

- `requestId` (string, required) — correlation ID for the response
- `limit` (number, optional) — max elements to return (default 50, clamped 1–1000)

**Response:**
```json
{
  "type": "elementsResult",
  "requestId": "elm-1",
  "elements": [
    {"semanticId": "input:filter", "type": "input", "value": "app", "focused": true},
    {"semanticId": "list:choices", "type": "list", "text": "2 items"},
    {"semanticId": "choice:0:apple", "type": "choice", "text": "Apple", "value": "apple", "selected": true, "index": 0}
  ],
  "totalCount": 3,
  "truncated": false,
  "focusedSemanticId": "input:filter",
  "selectedSemanticId": "choice:0:apple",
  "warnings": []
}
```

**Semantic ID format:** `input:filter`, `list:choices`, `choice:<index>:<value>`, `button:<index>:<label>`, `panel:<type>`

**Observation receipts** are included in every response:
- `focusedSemanticId` — which element has keyboard focus
- `selectedSemanticId` — which choice/item is currently selected
- `truncated` — `true` if elements were capped by limit
- `warnings` — machine-readable codes like `panel_only_div_prompt` when a view has limited introspection

### MCP Context Resources

Script Kit exposes desktop context as an MCP resource that AI agents can read to understand what the user is currently doing.

**`kit://context`** — Full desktop snapshot:
```json
{
  "schemaVersion": 1,
  "selectedText": "function hello() { ... }",
  "frontmostApp": {"pid": 1234, "bundleId": "com.apple.Safari", "name": "Safari"},
  "menuBarItems": [{"title": "File", "enabled": true, "children": [...]}],
  "browser": {"url": "https://docs.rs/gpui"},
  "focusedWindow": {"title": "PROTOCOL.md", "width": 1440, "height": 900},
  "warnings": []
}
```

**Profiles** control which fields are captured:

| URI | Fields | Use Case |
|-----|--------|----------|
| `kit://context` | All fields | Comprehensive context |
| `kit://context?profile=minimal` | App, browser, window (no text/menu) | Low-token overhead |
| `kit://context?selectedText=1&menuBar=0` | Custom field selection | Fine-grained control |
| `kit://context/schema` | Schema JSON | Discover profiles, parameters, diagnostics |
| `kit://context?diagnostics=1` | Snapshot + field status | Debug capture failures |

Per-field flags: `selectedText`, `frontmostApp`, `menuBar`, `browserUrl`, `focusedWindow` — each accepts `0`/`1`/`true`/`false`.

### Context Parts

Context parts attach structured desktop state to AI interactions. Agent Chat stages context automatically and also supports attaching context via slash commands:

| Command | Context Attached |
|---------|-----------------|
| `/context` | Desktop snapshot (minimal profile) |
| `/context-full` | Desktop snapshot (full profile) |
| `/selection` | Selected text only |
| `/browser` | Browser URL only |
| `/window` | Focused window info only |

Context parts can also be file attachments. All parts are resolved at submit time with partial-failure tolerance — if one part fails (e.g., browser not available), successful parts are still included and failures are tracked in a resolution receipt.

**Resolution receipt structure:**
```json
{
  "attempted": 2,
  "resolved": 1,
  "failures": [{"label": "Browser URL", "source": "kit://context?browserUrl=1", "error": "No browser detected"}],
  "promptPrefix": "<context source=\"kit://context?profile=minimal\">...</context>"
}
```

### Deterministic Transactions (`waitFor` + `batch`)

AI agents can execute verifiable UI transactions without sleeps or polling loops. The `waitFor` command polls a condition until satisfied, and `batch` chains multiple atomic commands into a single request.

**Agent workflow:** set input → wait for choices to render → select by value → submit. No timing guesses required.

```json
{
  "type": "batch",
  "requestId": "txn-1",
  "commands": [
    {"type": "setInput", "text": "apple"},
    {"type": "waitFor", "condition": "choicesRendered", "timeout": 1000},
    {"type": "selectByValue", "value": "apple", "submit": true}
  ]
}
```

The app replies with per-command results including elapsed time and selected values:

```json
{
  "type": "batchResult",
  "requestId": "txn-1",
  "success": true,
  "results": [
    {"index": 0, "success": true, "command": "setInput"},
    {"index": 1, "success": true, "command": "waitFor", "elapsed": 17},
    {"index": 2, "success": true, "command": "selectByValue", "value": "apple"}
  ],
  "totalElapsed": 24
}
```

Available wait conditions: `choicesRendered`, `inputEmpty`, `windowVisible`, `windowFocused`, plus detailed conditions like `elementExists`, `elementFocused`, and `stateMatch`. See [`docs/PROTOCOL.md`](docs/PROTOCOL.md) for the full reference.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `make verify`
5. Submit a pull request

See `AGENTS.md` for detailed development guidelines.

## License

MIT License - see LICENSE file for details.

## Links

- [Script Kit Website](https://scriptkit.com)
- [GPUI Documentation](https://gpui.rs)
- [Bun Runtime](https://bun.sh)
- [Zed Editor](https://zed.dev) (GPUI origin)

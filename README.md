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
- Core prompt APIs (`arg`, `div`, `editor`, `fields`, `form`, `drop`, `hotkey`, `path`, `term`, `chat`, `mic`, `webcam`, `screenshot`)
- Choice/option structure and props
- Basic script metadata format

What's changed:
- No bundled utilities (file helpers, clipboard wrappers, etc.)
- No `kit` global with hundreds of helpers
- Scripts must explicitly import dependencies via Bun
- Configuration is TypeScript-based (`~/.sk/kit/config.ts`)

## Quick Start

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
   mkdir -p ~/.sk/kit/scripts
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
   
   Create `~/.sk/kit/config.ts`:
   ```typescript
   export default {
     hotkey: {
       modifiers: ["meta"],  // Cmd on macOS
       key: "Semicolon"      // Press Cmd+; to toggle
     }
   };
   ```

### Your First Script

Create `~/.sk/kit/scripts/hello.ts`:

```typescript
metadata = {
  name: "Hello World",
  description: "My first script"
}

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
const files = await drop("Drop files here");
```

### Using Bun Packages

Since utilities aren't bundled, install what you need:

```bash
cd ~/.sk/kit
bun add zod lodash-es date-fns
```

Then use them in your scripts:

```typescript
metadata = {
  name: "Process Data",
  description: "Using external packages"
}

import { z } from "zod";
import { groupBy } from "lodash-es";
import { format } from "date-fns";

const data = await arg("Enter JSON data");
const parsed = z.object({ items: z.array(z.string()) }).parse(JSON.parse(data));

await div(`<pre>${JSON.stringify(groupBy(parsed.items, x => x[0]), null, 2)}</pre>`);
```

### Script Metadata

Use the global `metadata` variable to define script properties:

```typescript
metadata = {
  name: "My Script",
  description: "What it does",
  author: "Your Name",
  shortcut: "cmd+shift+m",
  schedule: "0 9 * * *",
  // Additional options:
  // hidden: true,        // Hide from script list
  // tags: ["utility"],   // Categorize scripts
}

// Your code here...
```

> **Note:** The global `metadata` format is the recommended approach. It provides TypeScript type checking, better IDE support, and access to more fields. Comment-based metadata (`// Name:`, `// Description:`) still works for backwards compatibility.

## Configuration

### `~/.sk/kit/config.ts`

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
    appLauncher: true
  },
  
  // Custom paths
  bun_path: "/opt/homebrew/bin/bun",
  editor: "code"
};
```

### `~/.sk/kit/theme.json`

Customize the look and feel:

```json
{
  "colors": {
    "background": { "main": 1973790 },
    "text": { "primary": 15066597 },
    "accent": { "selected": 3447003 }
  },
  "opacity": { "background": 0.95 },
  "vibrancy": { "enabled": true, "style": "popover" }
}
```

See `theme.example.json` for all available options.

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
│   └── ai/                # AI chat window (BYOK)
├── scripts/
│   └── kit-sdk.ts         # The SDK (preloaded into scripts)
├── tests/
│   ├── smoke/             # End-to-end tests
│   └── sdk/               # SDK method tests
└── ~/.sk/kit/               # User's scripts and config
    ├── scripts/           # Your scripts live here
    ├── config.ts          # Configuration
    └── theme.json         # Theme customization
```

### Running Tests

```bash
# Rust unit tests
cargo test

# Full verification (run before commits)
cargo check && cargo clippy --all-targets -- -D warnings && cargo test

# SDK tests via stdin protocol
echo '{"type":"run","path":"'$(pwd)'/tests/smoke/hello-world.ts"}' | ./target/debug/script-kit-gpui
```

### Building for Release

```bash
# Optimized binary
cargo build --release

# macOS app bundle
cargo install cargo-bundle
cargo bundle --release
```

## Features

### Built-in Capabilities

- **Clipboard History** - Access your clipboard history (enable in config)
- **App Launcher** - Quick launch applications
- **Notes Window** - Floating notes with Markdown support (`Cmd+Shift+N`)
- **AI Chat** - BYOK chat interface (`Cmd+Shift+Space`, requires API key)
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
| `drop(placeholder?)` | Drag and drop zone |
| `hotkey(placeholder?)` | Capture keyboard shortcut |
| `term(command?)` | Interactive terminal |
| `chat(options?)` | Chat interface |
| `mic()` | Audio recording |
| `webcam()` | Camera capture |

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `cargo check && cargo clippy && cargo test`
5. Submit a pull request

See `AGENTS.md` for detailed development guidelines.

## License

MIT License - see LICENSE file for details.

## Links

- [Script Kit Website](https://scriptkit.com)
- [GPUI Documentation](https://gpui.rs)
- [Bun Runtime](https://bun.sh)
- [Zed Editor](https://zed.dev) (GPUI origin)

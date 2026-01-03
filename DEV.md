# Development Guide for script-kit-gpui

This guide explains how to set up and use the development environment with hot-reload capabilities.

## Prerequisites

### Required

- **Rust** (1.70+) – Install from https://rustup.rs/
- **cargo-watch** – Auto-recompiler tool for Rust projects

  ```bash
  cargo install cargo-watch
  ```

### Optional but Recommended

- A terminal with good color support for clear output
- Text editor with Rust support (VS Code, Neovim, etc.)

## Running the Dev Server

Start the development runner with automatic rebuilds:

```bash
./dev.sh
```

Or if you prefer bash explicitly:

```bash
bash dev.sh
```

The script will:
1. Check if `cargo-watch` is installed (offering installation instructions if not)
2. Start the Rust compiler with `cargo watch -c -x run`
3. Clear the screen between rebuilds for clean output
4. Automatically rebuild and restart the app whenever you save a `.rs` file

Press **Ctrl+C** to stop the development runner.

## Hot Reload Workflow

This project supports multiple hot-reload mechanisms for a smooth development experience:

### 1. **Rust Code Changes** (via cargo-watch)
- Editing any `.rs` file triggers `cargo watch` to rebuild and restart the application
- The app instantly reflects your code changes
- No manual restart needed

### 2. **Theme Changes** (via ~/.kit/theme.json)
The app automatically watches `~/.kit/theme.json` for changes:
- Modify colors, fonts, or any theme settings in this file
- The UI refreshes in real-time without restarting the app
- See the "Theme Configuration" section below for details

### 3. **Script List Changes** (via ~/.sk/kit/scripts)
The app automatically detects new or modified scripts:
- Add a new script to `~/.sk/kit/scripts/`
- Remove or rename an existing script
- The app refreshes the script list without restarting
- Changes appear in the UI immediately

## Theme Configuration

To set up hot-reload for UI themes:

### First Time Setup

1. Create the Kit home directory:
   ```bash
   mkdir -p ~/.kit
   ```

2. Create or edit `~/.kit/theme.json`:
   ```json
   {
     "background": "#1e1e1e",
     "foreground": "#e0e0e0",
     "accent": "#007acc",
     "border": "#464647"
   }
   ```

3. Run the dev server - it will automatically watch this file for changes

### Editing Your Theme

Edit `~/.kit/theme.json` in your text editor while the dev server runs. Changes appear instantly in the UI without restarting.

## Global Hotkey Configuration

Script Kit allows you to customize the hotkey used to open/focus the application.

### Setup

The hotkey is configured in `~/.kit/config.ts` (TypeScript):

```typescript
// ~/.kit/config.ts
export default {
  hotkey: {
    modifiers: ['meta'],      // 'meta' = Cmd on macOS, Win on Windows
    key: 'Digit0',            // The key to press (default: 0)
  },
};
```

### How It Works

1. **App Startup**: The Rust app loads `~/.kit/config.ts`
2. **Transpilation**: Uses `bun build` to transpile TypeScript to JavaScript
3. **Extraction**: Runs the JavaScript with `bun` to extract the default export as JSON
4. **Registration**: Converts the JSON config to native hotkey codes and registers them
5. **Listening**: A background thread listens for the hotkey press
6. **Action**: When pressed, sets `HOTKEY_TRIGGERED` flag, which causes the app window to show/hide

### Supported Keys

**Number keys:** `Digit0`, `Digit1`, `Digit2`, ..., `Digit9`

**Letter keys:** `KeyA`, `KeyB`, ..., `KeyZ`

**Special keys:** `Space`, `Enter`, `Semicolon`

### Supported Modifiers

- `meta` - Command (⌘) on macOS, Windows key on Windows
- `ctrl` - Control key
- `alt` - Option/Alt key  
- `shift` - Shift key

### Examples

```typescript
// Cmd+K (like VSCode command palette)
hotkey: { modifiers: ['meta'], key: 'KeyK' }

// Cmd+Shift+P (like VSCode)
hotkey: { modifiers: ['meta', 'shift'], key: 'KeyP' }

// Ctrl+Alt+Space (like Raycast on Linux)
hotkey: { modifiers: ['ctrl', 'alt'], key: 'Space' }
```

### Debugging Hotkey Issues

If your configured hotkey isn't working:

1. **Check startup logs**: Run the app and look for:
   ```
   [APP] Loaded config: hotkey=["meta"]+Digit0
   [HOTKEY] Registered global hotkey meta+Digit0 (id: 536870917)
   ```

2. **Verify the config is being read**: Check that the hotkey line shows your intended hotkey

3. **Test hotkey press**: Press the hotkey and look for:
   ```
   [HOTKEY] meta+Digit0 pressed (trigger #1)
   ```

4. **Unknown key warning**: If you see `Unknown key code: XYZ`, that key is not supported

5. **Restart after changes**: Configuration file changes are watched, but the hotkey listener needs a restart

## Best Practices for Development

### Terminal Setup

- Use a terminal with **256-color support** for the best visual experience
- **Full-screen terminal** recommended for viewing logs and output
- **Clear scrollback** between dev sessions for cleaner logs

### Workflow Tips

1. **Keep the log panel open** – Use `Cmd+L` in the app to toggle the logs panel
   - Shows real-time events: hotkey presses, script executions, filter changes
   - Helpful for debugging configuration issues

2. **Test scripts incrementally**
   - Create test scripts in `~/.sk/kit/scripts/`
   - Run them through the UI to verify behavior
   - Check logs for execution details

3. **Hotkey testing**
    - Configure your hotkey in `~/.kit/config.ts` (TypeScript configuration)
    - The app loads this file, transpiles it with bun, and extracts the hotkey config
    - Press the configured hotkey (default: `Cmd+0`) to toggle the app visibility
    - Watch logs (Cmd+L) for hotkey registration: `[HOTKEY] Registered global hotkey meta+Digit0 (id: ...)`
    - When you press the hotkey, you'll see: `[HOTKEY] meta+Digit0 pressed (trigger #N)`
    - If your configured hotkey isn't working:
      1. Check the startup logs for the registered hotkey (should match your config)
      2. Verify the key code is valid (Digit0-9, KeyA-Z, Space, Enter, Semicolon, etc.)
      3. Verify the modifier is valid (meta=Cmd, ctrl=Control, alt=Option, shift=Shift)
      4. Restart the app after config changes (hot-reload watches for file changes)

4. **Use filtering** – Type to search scripts
   - Helps verify the filtering logic is working correctly
   - Type to add characters, Backspace to remove, Escape to clear

### Common Development Tasks

#### Test a Single File Change
```bash
# Dev server is already running with cargo-watch
# Just save your file and wait ~2-5 seconds for recompile
```

#### Check the Build Log
```bash
# Look at the cargo-watch output in your terminal
# It shows compilation errors, warnings, and execution output
```

#### Revert a Change
```bash
# Stop dev server: Ctrl+C
# Run: git checkout path/to/file.rs
# Start dev server again: ./dev.sh
```

#### Clean Build
```bash
# Stop dev server: Ctrl+C
# Run: cargo clean
# Start dev server again: ./dev.sh
# (This will recompile everything from scratch)
```

## Troubleshooting

### Script crashes immediately after startup
- Check the terminal output for panic messages
- Look at the logs panel (Cmd+L) for detailed events
- Verify Rust dependencies are correct: `cargo build`

### cargo-watch not detecting changes
- Ensure files are being saved to disk (check modification timestamps)
- Stop and restart the dev server: Ctrl+C, then `./dev.sh`
- Try `cargo clean && ./dev.sh` for a full rebuild

### Hotkey not registering
- Check the logs panel (Cmd+L) for hotkey registration messages
- Verify your hotkey config in `~/.kit/config.json` is valid
- Some system shortcuts may conflict - try a different key combination

### Theme changes not appearing
- Verify `~/.kit/theme.json` exists and is valid JSON
- Check the logs for file watcher errors
- Restart the dev server if hot-reload doesn't trigger

## Architecture Overview

The dev experience is built on several components:

- **cargo-watch** – Detects Rust source changes → triggers rebuild/restart
- **notify crate** – File system watcher for config and script changes
- **GPUI** – The UI framework with reactive rendering
- **Global hotkey listener** – Background thread detecting system hotkey press

These work together to provide instant feedback on:
1. Code changes (cargo-watch)
2. Configuration/theme changes (notify)
3. New/modified scripts (notify + file watcher)
4. Hotkey presses (global-hotkey thread)

## Interactive Prompt System (NEW!)

The app now supports Script Kit's v1 API prompts via bidirectional JSONL:

### Testing Interactive Scripts

1. Create a script using `arg()` or `div()`:
   ```typescript
   // ~/.sk/kit/scripts/my-test.ts
   const choice = await arg('Pick one', [
     { name: 'Option A', value: 'a' },
     { name: 'Option B', value: 'b' },
   ]);
   await div(`<h1>You picked: ${choice}</h1>`);
   ```

2. Run via the app UI (type to filter, Enter to execute)

3. Or trigger via test command:
   ```bash
   echo "run:my-test.ts" > /tmp/script-kit-gpui-cmd.txt
   ```

### Architecture

The interactive system uses:
- **Split threads**: Reader (blocks on script stdout) + Writer (sends to stdin)
- **Channels**: `mpsc` for thread-safe UI updates
- **AppView state**: ScriptList → ArgPrompt → DivPrompt → ScriptList

### Key Log Events

Watch for these in the logs (`Cmd+L`):
```
[EXEC] Received message: Arg { ... }     # Script sent prompt
[UI] Showing arg prompt: 1 with 2 choices # UI displaying
[KEY] ArgPrompt key: 'enter'              # User selected
[UI] Submitting response for 1: Some(...) # Sending back
[EXEC] Sending to script: {...}           # Writer thread
[EXEC] Received message: Div { ... }      # Next prompt
```

### Smoke Test

Run the binary smoke test:
```bash
cargo run --bin smoke-test
cargo run --bin smoke-test -- --gui  # With GUI test
```

## Window Focus/Unfocus Theming (NEW!)

The app now supports context-aware theming based on window focus state. When the window loses focus (user clicks another app), the UI automatically transitions to a dimmed theme for visual feedback that it's inactive.

### How It Works

**Automatic Behavior (Default)**
- When window is **focused**: Uses standard, vibrant theme colors
- When window is **unfocused**: Colors are automatically dimmed by ~30% toward gray, reducing brightness and saturation
- This happens seamlessly without any configuration needed

**Custom Focus-Aware Colors**
You can customize the focused/unfocused appearance in `~/.kit/theme.json`:

```json
{
  "colors": {
    "background": { "main": 1980410, ... },
    "text": { "primary": 16777215, ... },
    ...
  },
  "focus_aware": {
    "focused": {
      "background": { "main": 1980410, ... },
      "text": { "primary": 16777215, ... },
      "ui": { "border": 4609607, "success": 65280 },
      "cursor": {
        "color": 65535,
        "blink_interval_ms": 500
      }
    },
    "unfocused": {
      "background": { "main": 1447037, ... },
      "text": { "primary": 11842475, ... },
      "ui": { "border": 3158809, "success": 43008 },
      "cursor": {
        "color": 43605,
        "blink_interval_ms": 1000
      }
    }
  }
}
```

### Fields Reference

- **`focus_aware.focused`** – Colors when window has keyboard focus (optional)
- **`focus_aware.unfocused`** – Colors when window is in background (optional)
- **`cursor.color`** – Cursor color in hex (e.g., 0x00ffff = cyan)
- **`cursor.blink_interval_ms`** – Blink speed in milliseconds

If focus-aware colors aren't specified in your theme.json, the app automatically creates a dimmed version of your standard colors when the window loses focus.

### Implementation Details

**Code Structure:**
- `theme.rs::Theme::get_colors(is_focused)` – Returns appropriate ColorScheme based on window state
- `theme.rs::Theme::get_cursor_style(is_focused)` – Returns cursor styling (only when focused)
- `main.rs::render()` – Tracks window focus via `focus_handle.is_focused(window)`
- All render functions use `colors` from focus-aware selection instead of direct `theme.colors`

**Focus Tracking:**
```rust
if self.is_window_focused != is_focused {
    self.is_window_focused = is_focused;
    logging::log("FOCUS", &format!("Window focus state changed: {}", is_focused));
    cx.notify();  // Trigger re-render with new colors
}
```

**Dimming Algorithm:**
The automatic unfocused dimming blends each color channel 30% toward gray (0x808080):
```rust
new_value = (original * 70 + gray * 30) / 100
```
This reduces both brightness and saturation for a muted appearance.

### Testing Focus Behavior

1. Run the app: `./dev.sh`
2. Press your configured hotkey to show the window
3. Click on another application – window loses focus
4. Observe the UI colors dim automatically
5. Click back on the Script Kit window – colors return to normal
6. Watch the logs (`Cmd+L`) for focus change events:
   ```
   [FOCUS] Window focus state changed: true
   [THEME] Using focused colors (is_focused=true)
   ```

## Scroll Performance Optimization

The application includes a keyboard scroll performance fix that prevents UI hangs during fast keyboard repeat. When users hold down arrow keys to scroll through long lists, events can arrive faster than the UI can render individual updates (up to 100+ events per second).

### The Problem

Without optimization, each key event would trigger:
1. Selection index update
2. Scroll position recalculation
3. Full list re-render
4. Visibility recalculation

At 100 events/second, this creates an unbounded event queue that freezes the UI.

### The Solution: Event Coalescing

The fix implements a **20ms coalescing window** that batches rapid key events:

1. **Direction Tracking**: Tracks whether user is scrolling up or down
2. **Event Counting**: Accumulates events within the coalescing window
3. **Batched Updates**: Single UI update for multiple key events
4. **Delta Movement**: `move_selection_by(delta)` jumps N items at once

Key implementation in `src/main.rs`:
- `ScrollDirection` enum tracks up/down direction
- `process_arrow_key_with_coalescing()` handles event batching
- `flush_pending_scroll()` applies accumulated delta
- `move_selection_by(delta)` moves selection by N items

### Performance Instrumentation

The `src/perf.rs` module provides timing utilities:

- **KeyEventTracker**: Measures key event rates and processing latency
- **ScrollTimer**: Tracks scroll operation timing
- **FrameTimer**: Monitors frame rates and dropped frames
- **TimingGuard**: RAII-style timing with threshold alerts

Usage in code:
```rust
use crate::perf::{start_key_event, end_key_event, log_perf_summary};

let start = start_key_event();
// ... handle key event ...
end_key_event(start);
```

### Performance Logging

The `src/logging.rs` module includes scroll-specific log functions:

- `log_key_event_rate()` - Key events per second
- `log_scroll_queue_depth()` - Pending scroll events
- `log_render_stall()` - Detects UI freezes
- `log_scroll_batch()` - Batched scroll operations
- `log_key_repeat_timing()` - Time between key repeats

View performance logs with `Cmd+L` and filter for:
```
[KEY_PERF], [SCROLL_TIMING], [FRAME_PERF], [PERF_SLOW]
```

### Running Performance Tests

**SDK Test Harness** (`tests/sdk/test-scroll-perf.ts`):
```bash
# Run via bun (requires kit-sdk setup)
bun run tests/sdk/test-scroll-perf.ts
```

Test cases:
1. **scroll-normal**: 200ms interval (baseline)
2. **scroll-fast**: 50ms interval
3. **scroll-rapid**: 10ms interval (simulates key hold)
4. **scroll-burst**: Rapid bursts with pauses

**Benchmark Script** (`scripts/scroll-bench.ts`):
```bash
# Runs multiple iterations and outputs statistics
npx tsx scripts/scroll-bench.ts
```

Benchmark output includes:
- Min/Max/Avg latency per iteration
- P50, P95, P99 percentiles
- Pass/Fail assessment (P95 < 50ms threshold)

### Performance Thresholds

| Metric | Threshold | Description |
|--------|-----------|-------------|
| P95 Latency | < 50ms | 95th percentile key response time |
| Slow Key Event | > 16.67ms | Exceeds 60fps frame budget |
| Slow Scroll | > 8ms | Single scroll operation time |
| Dropped Frame | > 32ms | Below 30fps threshold |

### Interpreting Results

**Good performance:**
```
[KEY_PERF] rate=45.0/s avg=2.50ms slow=0.0% total=100
[SCROLL_TIMING] avg=1.50ms max=5.00ms slow=0 total=50
```

**Performance regression:**
```
[KEY_PERF] rate=45.0/s avg=25.00ms slow=15.0% total=100
[PERF_SLOW] key_event took 35.00ms (threshold: 16.67ms)
```

## Next Steps

1. ✅ Install `cargo-watch`: `cargo install cargo-watch`
2. ✅ Start dev server: `./dev.sh`
3. ✅ Create a test script in `~/.sk/kit/scripts/`
4. ✅ Configure hotkey in `~/.kit/config.json`
5. ✅ Use `Cmd+L` to view logs while developing
6. ✅ (NEW!) Customize focus-aware theme in `~/.kit/theme.json`
7. ✅ (NEW!) Run scroll performance benchmarks to validate UI responsiveness

Happy hacking! 🚀

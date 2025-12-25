# Smoke Tests for GPUI Script Kit

This directory contains TypeScript scriptlets for smoke testing the GPUI-based Script Kit executor.

## Overview

These are **not Rust tests** - they are TypeScript fixture scripts that verify the complete integration between:
- The Rust executor (`src/executor.rs`)
- The JSONL protocol (`src/protocol.rs`)
- The GPUI panel UI (`src/panel.rs`)
- The TypeScript SDK (`scripts/kit-sdk.ts`)

## Test Files

| File | Purpose | Tests |
|------|---------|-------|
| `hello-world.ts` | Basic sanity check | SDK preload, div(), md(), clean exit |
| `hello-world-args.ts` | Interactive prompts | arg() with simple/structured choices, multi-step flow |
| `test-window-reset.ts` | Window state reset | NEEDS_RESET flag, fresh UI after script completion |

### Multi-Monitor Testing

| File | Purpose | Tests |
|------|---------|-------|
| `scripts/test-monitor-positioning.ts` | Multi-monitor window positioning | Window appears on mouse cursor's monitor |

See [Multi-Monitor Test](#multi-monitor-positioning-test) section below for detailed usage.

### Window Reset Testing

| File | Purpose | Tests |
|------|---------|-------|
| `test-window-reset.ts` | Window state reset after script exit | NEEDS_RESET flag clears stale UI |

See [Window Reset Test](#window-reset-test) section below for detailed usage.

## Quick Start

### Option 1: Test from Project Directory (Development)

The scripts use relative imports, so they work directly from the project:

```bash
# Build the GPUI app
cargo build

# Run a smoke test directly
./target/debug/script-kit-gpui tests/smoke/hello-world.ts
./target/debug/script-kit-gpui tests/smoke/hello-world-args.ts
```

### Option 2: Copy to ~/.kenv/scripts/ (Production-like)

For testing in a production-like environment:

```bash
# Create scripts directory if needed
mkdir -p ~/.kenv/scripts

# Copy SDK to lib location
mkdir -p ~/.kenv/lib
cp scripts/kit-sdk.ts ~/.kenv/lib/kit-sdk.ts

# Copy smoke tests (update import paths first!)
# Note: You'll need to change the import from:
#   import '../../scripts/kit-sdk';
# To:
#   import '../lib/kit-sdk';
```

### Option 3: Use Bun Directly (SDK Testing Only)

To test just the SDK without the GPUI app:

```bash
# This won't show UI, but tests the protocol messages
bun run --preload scripts/kit-sdk.ts tests/smoke/hello-world.ts
```

## Expected Log Output

### Successful Execution

When running `hello-world.ts`, you should see:

```
[EXEC] execute_script_interactive: tests/smoke/hello-world.ts
[EXEC] Looking for SDK...
[EXEC]   Checking: /Users/<you>/.kenv/lib/kit-sdk.ts
[EXEC]   Checking dev path: /path/to/script-kit-gpui/scripts/kit-sdk.ts
[EXEC]   FOUND SDK (dev): /path/to/script-kit-gpui/scripts/kit-sdk.ts
[EXEC] Trying: bun run --preload /path/to/sdk tests/smoke/hello-world.ts
[EXEC] SUCCESS: bun with preload
[EXEC] Process spawned with PID: 12345
[EXEC] ScriptSession created successfully
[EXEC] Received from script: {"type":"div","id":"1","html":"<h1>Hello..."}
```

### Script's stderr Output

The scripts also log to stderr for debugging:

```
[SMOKE] hello-world.ts starting...
[SMOKE] SDK globals available: function function function
[SMOKE] hello-world.ts completed successfully!
```

## Debugging Guide

### Issue: "SDK NOT FOUND anywhere!"

**Symptom:** Executor logs show SDK search failing
**Solution:** 
1. Ensure `scripts/kit-sdk.ts` exists in project root
2. Or copy it to `~/.kenv/lib/kit-sdk.ts`

### Issue: "Failed to spawn 'bun'"

**Symptom:** Executor can't find bun executable
**Solution:**
1. Install bun: `curl -fsSL https://bun.sh/install | bash`
2. Or ensure it's in PATH for GUI apps (see `find_executable` in executor.rs)

### Issue: Script hangs / no output

**Symptom:** No messages received from script
**Causes:**
1. SDK not preloaded - globals don't exist
2. Script threw an error before sending first message
3. JSONL parse error

**Debug:**
```bash
# Run script standalone to see errors
bun run tests/smoke/hello-world.ts

# Check if SDK loads
bun run --preload scripts/kit-sdk.ts -e "console.log(typeof arg, typeof div)"
```

### Issue: "Received from script" shows but UI doesn't update

**Symptom:** Protocol works but UI blank
**Solution:** Check `src/panel.rs` for message handling

## Observability Checklist

When adding new smoke tests, verify these checkpoints:

### 1. Executor Logs (Rust side)
- [ ] `execute_script_interactive` called with correct path
- [ ] SDK found and preload path correct
- [ ] Process spawned with valid PID
- [ ] "Received from script" shows valid JSON
- [ ] "Sending to script" shows submit messages
- [ ] "Script exited with code: 0"

### 2. Script Logs (TypeScript side - stderr)
- [ ] Script starting message appears
- [ ] SDK globals are available (typeof check)
- [ ] Each prompt completion logged
- [ ] Script completion message appears

### 3. Protocol Messages (JSONL)
- [ ] `arg` messages have: type, id, placeholder, choices[]
- [ ] `div` messages have: type, id, html
- [ ] `submit` messages have: type, id, value

### 4. UI Behavior
- [ ] Panel appears when script sends first message
- [ ] arg() shows filterable choice list
- [ ] div() renders markdown/HTML correctly
- [ ] Escape or click dismisses and sends submit
- [ ] Panel closes when script exits

## Writing New Smoke Tests

Follow this pattern:

```typescript
// Name: Test Name (shown in script list)
// Description: What this tests

import '../../scripts/kit-sdk';

// Always log start for observability
console.error('[SMOKE] test-name.ts starting...');

// Test functionality
const result = await arg('Prompt text', ['Choice 1', 'Choice 2']);
console.error(`[SMOKE] User selected: ${result}`);

// Show result
await div(md(`# Result: ${result}`));

// Always log completion
console.error('[SMOKE] test-name.ts completed!');
```

## Window Reset Test

The `tests/smoke/test-window-reset.ts` script tests that window state properly resets after a script completes and the window hides.

### Why This Test Exists

When a script completes, the window hides. If the user presses the hotkey to show the window again, it should display a fresh script list (ScriptList mode), not the stale content from the previous script.

This is controlled by the `NEEDS_RESET` atomic flag in `src/main.rs`:
1. When a script completes/exits, `NEEDS_RESET` is set to `true`
2. When the hotkey shows the window, it checks `NEEDS_RESET`
3. If `true`, it resets the view to ScriptList mode before showing

### How to Run the Test

1. **Build the app**: `cargo build`
2. **Run the test script**:
   ```bash
   ./target/debug/script-kit-gpui tests/smoke/test-window-reset.ts
   ```
3. **Wait 2 seconds** - the script will auto-exit
4. **Press your hotkey** (Ctrl+Cmd+O) to show the window
5. **Verify**: Window should show the script list, NOT the div content

### Expected Behavior

| Step | Expected Result |
|------|-----------------|
| Script runs | Shows div with "Window Reset Test" heading |
| After 2 seconds | Window hides automatically |
| Press hotkey | Window shows fresh ScriptList (search bar, script list) |

### Debugging Failed Tests

If the window shows stale content after hotkey:

1. **Check NEEDS_RESET flag**: Verify `script_exited` sets `NEEDS_RESET.store(true, ...)`
2. **Check hotkey handler**: Verify it checks `NEEDS_RESET.load(...)` and resets view
3. **Check view reset**: Verify `set_content(ViewContent::ScriptList)` is called

### Expected Logs

```
[SMOKE] test-window-reset.ts starting...
[SMOKE] Waiting 2 seconds before exit...
[SMOKE] test-window-reset.ts completed - window should reset on next hotkey!
```

## Multi-Monitor Positioning Test

The `scripts/test-monitor-positioning.ts` script tests that windows appear on the correct monitor (the one where the mouse cursor is located).

### Why This Test Exists

macOS has a complex coordinate system:
- Primary display has origin at (0, 0) at bottom-left
- Secondary displays have their own origin offsets
- Y coordinates increase upward (opposite of most UI frameworks)

The Script Kit app needs to:
1. Detect the global mouse cursor position
2. Find which display contains that position
3. Create the window centered on that display

### How to Run the Test

1. **Setup**: Ensure you have multiple monitors connected
2. **Build the app**: `cargo build`
3. **Move your mouse** to a specific monitor
4. **Activate Script Kit** with the global hotkey (Ctrl+Cmd+O)
5. **Run the test script** from the script list (search for "Multi-Monitor")
6. **Answer the prompt** about which monitor the window appeared on

### Expected Logs

When the test runs, check the terminal for `[POSITION]` logs:

```
[POSITION] ╔════════════════════════════════════════════════════════════╗
[POSITION] ║  CALCULATING WINDOW POSITION FOR MOUSE DISPLAY             ║
[POSITION] ╚════════════════════════════════════════════════════════════╝
[POSITION] Available displays: 2
[POSITION]   Display 0: origin=(0, 0) size=1920x1080 [bounds: x=0..1920, y=0..1080]
[POSITION]   Display 1: origin=(1920, 0) size=2560x1440 [bounds: x=1920..4480, y=0..1440]
[POSITION] Mouse cursor at (2500, 720)
[POSITION]   -> Mouse is on display 1
[POSITION] Selected display: origin=(1920, 0) size=2560x1440
[POSITION] Final window bounds: origin=(2685, 360) size=750x500
```

### Debugging Failed Tests

If the window appears on the wrong monitor:

1. **Check coordinate systems**: The logs show both mouse position and display bounds
2. **Verify display detection**: "Mouse is on display N" should match your expectation
3. **Check coordinate conversion**: macOS Y=0 is at bottom, we flip to top-left origin

Common issues:
- **Display origin mismatch**: Secondary displays may report incorrect origins from GPUI (we use NSScreen directly to work around this)
- **Retina scaling**: High-DPI displays may have coordinate scaling issues
- **Vertical monitor arrangement**: Y-coordinate ranges may overlap unexpectedly

### Test Script Output

The test script logs to stderr with `[TEST]` prefix:

```
[TEST] ════════════════════════════════════════════════════════
[TEST] MULTI-MONITOR POSITIONING TEST
[TEST] ════════════════════════════════════════════════════════
[TEST] Step 1: Displaying initial prompt...
[TEST] User selected: monitor-2
[TEST] Result: PASS
[TEST] ════════════════════════════════════════════════════════
```

## CI Integration (Future)

These tests can be automated with:

```bash
# Headless mode (when implemented)
SCRIPT_KIT_HEADLESS=1 ./target/debug/script-kit-gpui tests/smoke/hello-world.ts

# With timeout
timeout 10 ./target/debug/script-kit-gpui tests/smoke/hello-world.ts || echo "Test timed out"
```

## Related Files

- `src/executor.rs` - Script execution and process management
- `src/protocol.rs` - JSONL message types and parsing
- `src/panel.rs` - GPUI UI rendering
- `scripts/kit-sdk.ts` - TypeScript SDK with global functions
- `src/bin/smoke-test.rs` - Rust-based smoke test binary

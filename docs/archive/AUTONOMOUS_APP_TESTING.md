# Autonomous App Testing Plan

Comprehensive documentation for autonomous testing of Script Kit GPUI APIs.

---

## 1. Overview

### Goal

Build an autonomous testing framework that validates all Script Kit GPUI APIs without requiring manual user interaction. Tests should:

1. **Auto-submit prompts** - Simulate user selections programmatically
2. **Verify protocol messages** - Ensure correct JSONL format
3. **Detect failures** - Catch crashes, timeouts, and protocol errors
4. **Report results** - Output structured JSONL for CI integration

### Current State

| Component | Status | Notes |
|-----------|--------|-------|
| **SDK (`kit-sdk.ts`)** | ✅ Complete | 47+ global functions implemented |
| **Protocol (`protocol.rs`)** | ✅ Complete | All message types defined |
| **Manual Tests** | ⚠️ Partial | `tests/sdk/` has 4 tests, `tests/smoke/` has 3 |
| **gpui-*.ts Demos** | ✅ 47 scripts | In `~/.sk/kit/scripts/` - API coverage reference |
| **Autonomous Runner** | ❌ Missing | Need test harness with auto-submit |

### Target State

```
┌─────────────────────────────────────────────────────────────┐
│                  AUTONOMOUS TEST FLOW                       │
├─────────────────────────────────────────────────────────────┤
│  1. Test Runner spawns script-kit-gpui with test script    │
│  2. Test script sends prompt message to stdout (JSONL)     │
│  3. App receives message, renders UI (optional headless)   │
│  4. App AUTO-SUBMITS first choice (or configured value)    │
│  5. Script receives submit, validates, continues           │
│  6. Test runner monitors stdout/stderr for results         │
│  7. Test runner parses JSONL output, reports pass/fail     │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Architecture

### Test Harness Design

```
┌──────────────────────────────────────────────────────────────┐
│                    Test Runner (Bun/Node)                    │
├──────────────────────────────────────────────────────────────┤
│  • Discovers test files in tests/autonomous/                 │
│  • Spawns script-kit-gpui process per test                   │
│  • Passes AUTO_SUBMIT=true environment variable              │
│  • Monitors stdout for JSONL test results                    │
│  • Monitors stderr for debug logs                            │
│  • Enforces timeout (default 30s per test)                   │
│  • Aggregates results, outputs summary                       │
└──────────────────────────────────────────────────────────────┘
                              │
                              │ spawn
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                script-kit-gpui (Rust App)                    │
├──────────────────────────────────────────────────────────────┤
│  • Loads test script via Bun with SDK preload                │
│  • Receives JSONL messages from script stdout                │
│  • When AUTO_SUBMIT=true:                                    │
│    - Prompts auto-submit after configurable delay            │
│    - Uses first choice, or value from AUTO_SUBMIT_VALUE      │
│  • Sends submit message to script stdin                      │
│  • Logs all protocol traffic for debugging                   │
└──────────────────────────────────────────────────────────────┘
                              │
                              │ stdin/stdout (JSONL)
                              ▼
┌──────────────────────────────────────────────────────────────┐
│                  Test Script (TypeScript)                    │
├──────────────────────────────────────────────────────────────┤
│  • Imports kit-sdk for global functions                      │
│  • Calls API under test (e.g., arg(), div(), fields())       │
│  • Validates returned value                                  │
│  • Outputs JSONL test result to stdout                       │
│  • Logs debug info to stderr                                 │
└──────────────────────────────────────────────────────────────┘
```

### Message Flow

```
Test Script                    GPUI App                     Test Runner
    │                             │                              │
    │─────── arg message ────────>│                              │
    │         (stdout)            │                              │
    │                             │                              │
    │                             │<──── AUTO_SUBMIT=true ───────│
    │                             │      (env var at spawn)      │
    │                             │                              │
    │                    [renders prompt UI]                     │
    │                    [waits 100ms]                           │
    │                    [auto-selects first choice]             │
    │                             │                              │
    │<────── submit message ──────│                              │
    │         (stdin)             │                              │
    │                             │                              │
    │ [validates result]          │                              │
    │ [outputs test result]       │                              │
    │─────────────────────────────┼──── JSONL result ───────────>│
    │                             │      (stdout)                │
    │                             │                              │
    │ [exits cleanly]             │                              │
    │                             │                              │
```

### Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `AUTO_SUBMIT` | bool | `false` | Enable autonomous testing mode |
| `AUTO_SUBMIT_DELAY_MS` | int | `100` | Delay before auto-submit (for UI render) |
| `AUTO_SUBMIT_VALUE` | string | (first choice) | Override auto-submit value |
| `AUTO_SUBMIT_INDEX` | int | `0` | Select N-th choice instead of first |
| `TEST_TIMEOUT_MS` | int | `30000` | Max time per test |
| `HEADLESS` | bool | `false` | Skip UI rendering entirely |

---

## 3. Protocol

### Script → App Messages (stdout)

All prompts follow this pattern:

```json
{
  "type": "<prompt_type>",
  "id": "<unique_id>",
  ...prompt-specific fields...
}
```

#### Core Prompt Types

| Type | Required Fields | Auto-Submit Behavior |
|------|-----------------|----------------------|
| `arg` | `id`, `placeholder`, `choices[]` | Submit `choices[0].value` |
| `div` | `id`, `html` | Submit immediately (dismissal) |
| `editor` | `id`, `content?`, `language?` | Submit `content` (unchanged) |
| `mini` | `id`, `placeholder`, `choices[]` | Submit `choices[0].value` |
| `micro` | `id`, `placeholder`, `choices[]` | Submit `choices[0].value` |
| `select` | `id`, `placeholder`, `choices[]`, `multiple?` | Submit `[choices[0].value]` |

#### Form Types

| Type | Required Fields | Auto-Submit Behavior |
|------|-----------------|----------------------|
| `fields` | `id`, `fields[]` | Submit `["", "", ...]` (empty strings) |
| `form` | `id`, `html` | Submit `{}` (empty form data) |
| `template` | `id`, `template` | Submit original `template` |
| `env` | `id`, `key`, `secret?` | Submit `"test-value"` |

#### Input Capture Types

| Type | Required Fields | Auto-Submit Behavior |
|------|-----------------|----------------------|
| `path` | `id`, `startPath?`, `hint?` | Submit `/tmp/test-path` |
| `hotkey` | `id`, `placeholder?` | Submit `{"key":"a","command":true,...}` |
| `drop` | `id` | Submit `[{path:"/tmp/test.txt",...}]` |

#### Media Types

| Type | Required Fields | Auto-Submit Behavior |
|------|-----------------|----------------------|
| `chat` | `id` | Submit `"test message"` |
| `term` | `id`, `command?` | Submit `"command output"` |
| `widget` | `id`, `html`, `options?` | Send `widgetEvent` close |
| `webcam` | `id` | Submit base64 test image |
| `mic` | `id` | Submit base64 test audio |
| `eyeDropper` | `id` | Submit `{"sRGBHex":"#FF0000",...}` |
| `find` | `id`, `placeholder`, `onlyin?` | Submit `"/tmp/found-file.txt"` |

### App → Script Messages (stdin)

```json
{
  "type": "submit",
  "id": "<matching_prompt_id>",
  "value": "<selected_value_or_null>"
}
```

### Fire-and-Forget Messages (no response)

These messages don't expect a submit response:

| Type | Purpose | Verification |
|------|---------|--------------|
| `beep` | Play system sound | Check no error |
| `say` | Text-to-speech | Check no error |
| `notify` | System notification | Check no error |
| `setStatus` | Update status bar | Check no error |
| `menu` | Set menu bar | Check no error |
| `show` | Show window | Check no error |
| `hide` | Hide window | Check no error |
| `browse` | Open URL | Check no error |
| `setPanel` | Update panel HTML | Check no error |
| `setPreview` | Update preview HTML | Check no error |
| `setPrompt` | Update prompt HTML | Check no error |

---

## 4. API Coverage Matrix

### TIER 1: Core Prompts (6 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `arg()` | `gpui-arg.ts` | ⚠️ Manual | ✅ Supported | Choice selection |
| `div()` | `gpui-div.ts` | ⚠️ Manual | ✅ Supported | HTML display |
| `editor()` | `gpui-editor.ts` | ❌ None | ✅ Supported | Code editor |
| `mini()` | `gpui-mini.ts` | ❌ None | ✅ Supported | Compact prompt |
| `micro()` | `gpui-micro.ts` | ❌ None | ✅ Supported | Tiny prompt |
| `select()` | `gpui-select.ts` | ❌ None | ✅ Supported | Multi-select |

### TIER 2: Form Prompts (4 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `fields()` | `gpui-fields.ts` | ❌ None | ✅ Supported | Multi-field form |
| `form()` | `gpui-form.ts` | ❌ None | ✅ Supported | Custom HTML form |
| `template()` | `gpui-template.ts` | ❌ None | ✅ Supported | VSCode-style snippets |
| `env()` | `gpui-env.ts` | ❌ None | ✅ Supported | Env var prompt |

### TIER 3: System APIs (11 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `beep()` | `gpui-beep.ts` | ❌ None | N/A | Fire-and-forget |
| `say()` | `gpui-say.ts` | ❌ None | N/A | Fire-and-forget |
| `notify()` | `gpui-notify.ts` | ❌ None | N/A | Fire-and-forget |
| `setStatus()` | `gpui-set-status.ts` | ❌ None | N/A | Fire-and-forget |
| `menu()` | `gpui-menu.ts` | ❌ None | N/A | Fire-and-forget |
| `copy()` | `gpui-clipboard.ts` | ❌ None | N/A | Clipboard write |
| `paste()` | `gpui-clipboard.ts` | ❌ None | ✅ Supported | Clipboard read |
| `clipboard.*` | `gpui-clipboard.ts` | ❌ None | Mixed | Read/write API |
| `keyboard.*` | `gpui-keyboard.ts` | ❌ None | N/A | Fire-and-forget |
| `mouse.*` | `gpui-mouse.ts` | ❌ None | N/A | Fire-and-forget |
| `getSelectedText()` | `gpui-selected-text.ts` | ❌ None | ✅ Supported | Accessibility API |
| `setSelectedText()` | `gpui-selected-text.ts` | ❌ None | N/A | Fire-and-forget |

### TIER 4A: Input Capture (3 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `hotkey()` | `gpui-hotkey.ts` | ❌ None | ✅ Supported | Key capture |
| `drop()` | `gpui-drop.ts` | ❌ None | ✅ Supported | File drop zone |
| `path()` | `gpui-path.ts` | ❌ None | ✅ Supported | File browser |

### TIER 4B: Media Prompts (6 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `chat()` | `gpui-chat.ts` | ❌ None | ✅ Supported | Chat interface |
| `term()` | `gpui-term.ts` | ⚠️ Manual | ✅ Supported | Terminal emulator |
| `widget()` | `gpui-widget.ts` | ❌ None | ✅ Supported | Custom widget |
| `webcam()` | `gpui-webcam.ts` | ❌ None | ⚠️ Hardware | Camera capture |
| `mic()` | `gpui-mic.ts` | ❌ None | ⚠️ Hardware | Audio recording |
| `eyeDropper()` | `gpui-eye-dropper.ts` | ❌ None | ✅ Supported | Color picker |
| `find()` | `gpui-find.ts` | ❌ None | ✅ Supported | Spotlight search |

### TIER 5A: Utility Functions (14 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `exec()` | `gpui-exec.ts` | ❌ None | ✅ Supported | Shell command |
| `get()` | `gpui-http.ts` | ❌ None | ✅ Supported | HTTP GET |
| `post()` | `gpui-http.ts` | ❌ None | ✅ Supported | HTTP POST |
| `put()` | `gpui-http.ts` | ❌ None | ✅ Supported | HTTP PUT |
| `patch()` | `gpui-http.ts` | ❌ None | ✅ Supported | HTTP PATCH |
| `del()` | `gpui-http.ts` | ❌ None | ✅ Supported | HTTP DELETE |
| `download()` | `gpui-download.ts` | ❌ None | ✅ Supported | File download |
| `trash()` | `gpui-trash.ts` | ❌ None | ✅ Supported | Move to trash |
| `show()` | `gpui-window-control.ts` | ❌ None | N/A | Fire-and-forget |
| `hide()` | `gpui-window-control.ts` | ❌ None | N/A | Fire-and-forget |
| `blur()` | `gpui-window-control.ts` | ❌ None | N/A | Fire-and-forget |
| `submit()` | `gpui-submit-exit.ts` | ❌ None | N/A | Force submit |
| `exit()` | `gpui-submit-exit.ts` | ❌ None | N/A | Script exit |
| `wait()` | `gpui-wait.ts` | ❌ None | N/A | Delay |

### TIER 5B: Storage & Path APIs (13 APIs)

| API | Demo Script | Test Status | Auto-Submit | Notes |
|-----|-------------|-------------|-------------|-------|
| `uuid()` | `gpui-uuid.ts` | ❌ None | N/A | Pure function |
| `compile()` | `gpui-compile.ts` | ❌ None | N/A | Pure function |
| `home()` | `gpui-paths.ts` | ❌ None | N/A | Pure function |
| `skPath()` | `gpui-paths.ts` | ❌ None | N/A | Pure function |
| `kitPath()` | `gpui-paths.ts` | ❌ None | N/A | Pure function |
| `tmpPath()` | `gpui-paths.ts` | ❌ None | N/A | Pure function |
| `isFile()` | `gpui-file-checks.ts` | ❌ None | N/A | Pure function |
| `isDir()` | `gpui-file-checks.ts` | ❌ None | N/A | Pure function |
| `isBin()` | `gpui-file-checks.ts` | ❌ None | N/A | Pure function |
| `db()` | `gpui-db.ts` | ❌ None | ✅ Supported | JSON database |
| `store.*` | `gpui-store.ts` | ❌ None | ✅ Supported | Key-value store |
| `memoryMap.*` | `gpui-memory-map.ts` | ❌ None | N/A | In-memory only |
| `browse()` | `gpui-browse.ts` | ❌ None | N/A | Fire-and-forget |
| `editFile()` | `gpui-edit.ts` | ❌ None | N/A | Fire-and-forget |
| `run()` | `gpui-run.ts` | ❌ None | ✅ Supported | Run another script |
| `inspect()` | `gpui-inspect.ts` | ❌ None | N/A | Fire-and-forget |

### Summary

| Tier | Total APIs | With Demo | With Test | Auto-Submittable |
|------|------------|-----------|-----------|------------------|
| TIER 1: Core | 6 | 6 | 2 | 6 |
| TIER 2: Forms | 4 | 4 | 0 | 4 |
| TIER 3: System | 11 | 5 | 0 | 3 |
| TIER 4A: Input | 3 | 3 | 0 | 3 |
| TIER 4B: Media | 7 | 7 | 1 | 5 |
| TIER 5A: Utility | 14 | 7 | 0 | 7 |
| TIER 5B: Storage | 15 | 9 | 0 | 3 |
| **TOTAL** | **60** | **41** | **3** | **31** |

---

## 5. Test Categories

### 5.1 Core Prompts

Tests for the fundamental prompt types that form the basis of user interaction.

```typescript
// tests/autonomous/test-core-prompts.ts
import '../../scripts/kit-sdk';

// Test: arg with string choices
const fruit = await arg('Select fruit', ['Apple', 'Banana', 'Cherry']);
assert(fruit === 'Apple', 'Should auto-select first choice');

// Test: arg with structured choices
const action = await arg('Select action', [
  { name: 'Run', value: 'run' },
  { name: 'Edit', value: 'edit' }
]);
assert(action === 'run', 'Should return value, not name');

// Test: div display
await div('<h1>Test Content</h1>');
// Success = no error thrown

// Test: editor with content
const edited = await editor('initial content', 'text');
assert(typeof edited === 'string', 'Should return string');

// Test: mini and micro (same behavior as arg)
const miniResult = await mini('Quick pick', ['A', 'B']);
const microResult = await micro('Tiny pick', ['X', 'Y']);

// Test: select (multi-select)
const selections = await select('Pick multiple', ['One', 'Two', 'Three']);
assert(Array.isArray(selections), 'Should return array');
```

### 5.2 Form Prompts

Tests for multi-field forms and data entry.

```typescript
// tests/autonomous/test-form-prompts.ts
import '../../scripts/kit-sdk';

// Test: fields with array of strings
const [name, email] = await fields(['Name', 'Email']);
assert(Array.isArray([name, email]), 'Should return array of values');

// Test: fields with structured definitions
const [user, pass] = await fields([
  { name: 'username', label: 'Username', type: 'text' },
  { name: 'password', label: 'Password', type: 'password' }
]);

// Test: custom HTML form
const formData = await form(`
  <input name="field1" value="test" />
  <input name="field2" value="data" />
`);
assert(typeof formData === 'object', 'Should return object');

// Test: template with placeholders
const filled = await template('Hello $1, welcome to $2!');
assert(typeof filled === 'string', 'Should return filled template');

// Test: env variable
const apiKey = await env('TEST_API_KEY');
assert(typeof apiKey === 'string', 'Should return string value');
```

### 5.3 System APIs

Tests for system interactions (beeps, notifications, clipboard, etc.).

```typescript
// tests/autonomous/test-system-apis.ts
import '../../scripts/kit-sdk';

// Test: beep (fire-and-forget)
await beep();
// Success = no error

// Test: say (fire-and-forget)
await say('Hello world');
// Success = no error

// Test: notify
await notify({ title: 'Test', body: 'Notification body' });
await notify('Simple notification');
// Success = no error

// Test: setStatus
await setStatus({ status: 'busy', message: 'Testing...' });
// Success = no error

// Test: clipboard
await copy('test clipboard content');
const pasted = await paste();
assert(pasted === 'test clipboard content', 'Clipboard round-trip');

// Test: keyboard (fire-and-forget)
await keyboard.type('hello');
await keyboard.tap('cmd', 'a');

// Test: mouse (fire-and-forget)
await mouse.setPosition({ x: 100, y: 100 });
await mouse.leftClick();
```

### 5.4 File Operations

Tests for file browsing, paths, and storage.

```typescript
// tests/autonomous/test-file-apis.ts
import '../../scripts/kit-sdk';

// Test: path browser
const selectedPath = await path({ startPath: '/tmp' });
assert(typeof selectedPath === 'string', 'Should return path string');

// Test: drop zone
const files = await drop();
assert(Array.isArray(files), 'Should return array of FileInfo');

// Test: path utilities (pure functions)
const homePath = home('Documents');
assert(homePath.includes('Documents'), 'home() should join paths');

const kenv = skPath('scripts');
assert(kenv.includes('.kenv'), 'skPath() should use kenv');

const kit = kitPath('config');
assert(kit.includes('.kit'), 'kitPath() should use kit');

const tmp = tmpPath('test');
assert(tmp.includes('tmp') || tmp.includes('temp'), 'tmpPath() should use temp');

// Test: file checks (async)
const isF = await isFile('/etc/hosts');
const isD = await isDir('/tmp');
assert(typeof isF === 'boolean' && typeof isD === 'boolean');

// Test: storage APIs
const database = await db({ items: [] });
assert(database.data !== undefined, 'db() should return database');

await store.set('test-key', 'test-value');
const stored = await store.get('test-key');
assert(stored === 'test-value', 'store round-trip');
```

### 5.5 Media Prompts

Tests for terminal, chat, widgets, and media capture.

```typescript
// tests/autonomous/test-media-prompts.ts
import '../../scripts/kit-sdk';

// Test: term with command
const output = await term('echo "test"');
assert(typeof output === 'string', 'term() should return output');

// Test: term interactive (no command)
const interactiveOutput = await term();
assert(typeof interactiveOutput === 'string');

// Test: chat
const chatResult = await chat();
assert(typeof chatResult === 'string', 'chat() should return string');

// Test: widget
const w = await widget('<div>Widget Content</div>', {
  width: 300,
  height: 200
});
assert(typeof w.setState === 'function', 'Should return controller');
w.close();

// Test: eyeDropper
const color = await eyeDropper();
assert(color.sRGBHex !== undefined, 'Should return color info');

// Test: find (Spotlight search)
const found = await find('Search files');
assert(typeof found === 'string', 'find() should return path');

// Hardware-dependent (skip in CI):
// const photo = await webcam();
// const audio = await mic();
```

---

## 6. Monitoring Strategy

### Log Parsing

The test runner monitors stdout for JSONL test results:

```json
{"test": "arg-string-choices", "status": "running", "timestamp": "2024-..."}
{"test": "arg-string-choices", "status": "pass", "result": "Apple", "duration_ms": 45}
```

#### Status Values

| Status | Meaning | Contains |
|--------|---------|----------|
| `running` | Test started | `test`, `timestamp` |
| `pass` | Test succeeded | `result?`, `duration_ms` |
| `fail` | Test failed | `error`, `duration_ms` |
| `skip` | Test skipped | `reason` |

### Crash Detection

The test runner monitors for:

1. **Process exit code ≠ 0** - App crashed
2. **Timeout exceeded** - Script hung (default 30s)
3. **Stderr patterns** - `panic`, `SIGSEGV`, `assertion failed`
4. **No stdout for 10s** - Script frozen

### Timeout Handling

```bash
# Environment configuration
TEST_TIMEOUT_MS=30000  # Per-test timeout
PROMPT_TIMEOUT_MS=5000 # Per-prompt timeout (auto-submit)
```

If a prompt doesn't auto-submit within `PROMPT_TIMEOUT_MS`, the test fails with:

```json
{"test": "test-name", "status": "fail", "error": "Prompt timeout: arg prompt exceeded 5000ms"}
```

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All tests passed |
| `1` | Some tests failed |
| `2` | Test runner error (not test failure) |
| `3` | Timeout exceeded |
| `4` | Crash detected |

---

## 7. Iteration Plan

### Phase 1: Infrastructure (Week 1)

1. **Add `AUTO_SUBMIT` support to `executor.rs`**
   - Check `AUTO_SUBMIT` env var on startup
   - Implement auto-submit delay timer
   - Send submit message to script stdin

2. **Create test runner script**
   - `scripts/autonomous-test-runner.ts`
   - Discover tests in `tests/autonomous/`
   - Spawn app process with env vars
   - Parse JSONL output
   - Report results

3. **Add test infrastructure**
   - `tests/autonomous/helpers.ts` - Assertion helpers
   - `tests/autonomous/setup.ts` - Common setup

### Phase 2: Core Tests (Week 2)

4. **Implement TIER 1 tests**
   - `tests/autonomous/test-arg.ts`
   - `tests/autonomous/test-div.ts`
   - `tests/autonomous/test-editor.ts`
   - `tests/autonomous/test-mini-micro.ts`
   - `tests/autonomous/test-select.ts`

5. **Implement TIER 2 tests**
   - `tests/autonomous/test-fields.ts`
   - `tests/autonomous/test-form.ts`
   - `tests/autonomous/test-template.ts`
   - `tests/autonomous/test-env.ts`

### Phase 3: System Tests (Week 3)

6. **Implement TIER 3 tests**
   - `tests/autonomous/test-beep-say.ts`
   - `tests/autonomous/test-notify.ts`
   - `tests/autonomous/test-clipboard.ts`
   - `tests/autonomous/test-keyboard-mouse.ts`

7. **Implement TIER 4A tests**
   - `tests/autonomous/test-hotkey.ts`
   - `tests/autonomous/test-drop.ts`
   - `tests/autonomous/test-path.ts`

### Phase 4: Media & Storage Tests (Week 4)

8. **Implement TIER 4B tests**
   - `tests/autonomous/test-term.ts`
   - `tests/autonomous/test-chat.ts`
   - `tests/autonomous/test-widget.ts`
   - `tests/autonomous/test-find.ts`
   - (Skip hardware: webcam, mic)

9. **Implement TIER 5 tests**
   - `tests/autonomous/test-exec.ts`
   - `tests/autonomous/test-http.ts`
   - `tests/autonomous/test-storage.ts`
   - `tests/autonomous/test-paths.ts`

### Phase 5: CI Integration (Week 5)

10. **Add GitHub Actions workflow**
    - `.github/workflows/autonomous-tests.yml`
    - Run on PR and push to main
    - Report results in PR comments

11. **Add headless mode**
    - `HEADLESS=true` skips UI rendering
    - Faster CI execution

---

## 8. Running Tests

### Prerequisites

```bash
# 1. Build the GPUI app
cargo build

# 2. Ensure Bun is installed
which bun || curl -fsSL https://bun.sh/install | bash

# 3. SDK exists
ls scripts/kit-sdk.ts
```

### Run All Autonomous Tests

```bash
# Standard run
bun run scripts/autonomous-test-runner.ts

# With verbose output
SDK_TEST_VERBOSE=true bun run scripts/autonomous-test-runner.ts

# With custom timeout
TEST_TIMEOUT_MS=60000 bun run scripts/autonomous-test-runner.ts
```

### Run Single Test

```bash
# Via test runner
bun run scripts/autonomous-test-runner.ts tests/autonomous/test-arg.ts

# Direct with app (manual auto-submit)
AUTO_SUBMIT=true ./target/debug/script-kit-gpui tests/autonomous/test-arg.ts
```

### Run by Category

```bash
# Core prompts only
bun run scripts/autonomous-test-runner.ts --filter "test-arg|test-div|test-editor"

# System APIs only
bun run scripts/autonomous-test-runner.ts --filter "test-beep|test-clipboard|test-notify"
```

### CI Command

```bash
# For GitHub Actions
cargo build --release && \
  AUTO_SUBMIT=true \
  HEADLESS=true \
  TEST_TIMEOUT_MS=30000 \
  bun run scripts/autonomous-test-runner.ts
```

### Expected Output

```
╔═══════════════════════════════════════════════════════════════╗
║              SCRIPT KIT AUTONOMOUS TEST RUNNER                ║
╚═══════════════════════════════════════════════════════════════╝

Environment:
  AUTO_SUBMIT: true
  TIMEOUT: 30000ms
  HEADLESS: false

────────────────────────────────────────────────────────────────
Running: tests/autonomous/test-arg.ts
────────────────────────────────────────────────────────────────
  ✅ arg-string-choices                          (45ms)
  ✅ arg-structured-choices                      (38ms)
  ✅ arg-empty-choices                           (22ms)

────────────────────────────────────────────────────────────────
Running: tests/autonomous/test-div.ts
────────────────────────────────────────────────────────────────
  ✅ div-html-content                            (31ms)
  ✅ div-with-tailwind                           (28ms)

────────────────────────────────────────────────────────────────
Running: tests/autonomous/test-fields.ts
────────────────────────────────────────────────────────────────
  ✅ fields-string-array                         (52ms)
  ✅ fields-structured                           (48ms)
  ⚠️  fields-validation (skipped: not implemented)

════════════════════════════════════════════════════════════════
RESULTS: 7 passed, 0 failed, 1 skipped
TOTAL TIME: 264ms
════════════════════════════════════════════════════════════════
```

---

## 9. Reference Files

### Key Implementation Files

| File | Purpose |
|------|---------|
| `scripts/kit-sdk.ts` | SDK with all global functions |
| `src/protocol.rs` | JSONL message types |
| `src/executor.rs` | Script execution, auto-submit logic |
| `src/main.rs` | App entry, window management |
| `src/prompts.rs` | Prompt rendering |

### Test Infrastructure Files

| File | Purpose |
|------|---------|
| `scripts/autonomous-test-runner.ts` | Test orchestrator (to be created) |
| `tests/autonomous/helpers.ts` | Assertion utilities (to be created) |
| `tests/autonomous/*.ts` | Individual test files (to be created) |
| `tests/sdk/README.md` | Existing manual test documentation |
| `tests/smoke/README.md` | Existing smoke test documentation |

### Demo Scripts (API Reference)

All 47 demo scripts in `~/.sk/kit/scripts/gpui-*.ts` serve as API usage reference:

```
gpui-beep.ts          gpui-browse.ts        gpui-chat.ts
gpui-clipboard.ts     gpui-compile.ts       gpui-db.ts
gpui-download.ts      gpui-drop.ts          gpui-edit.ts
gpui-editor.ts        gpui-env.ts           gpui-exec.ts
gpui-eye-dropper.ts   gpui-fields.ts        gpui-file-checks.ts
gpui-find.ts          gpui-form.ts          gpui-get-selected-text.ts
gpui-hotkey.ts        gpui-http.ts          gpui-inspect.ts
gpui-keyboard.ts      gpui-memory-map.ts    gpui-menu.ts
gpui-mic.ts           gpui-micro.ts         gpui-mini.ts
gpui-mouse.ts         gpui-notify.ts        gpui-path.ts
gpui-paths.ts         gpui-run.ts           gpui-say.ts
gpui-select.ts        gpui-selected-text.ts gpui-set-panel.ts
gpui-set-status.ts    gpui-store.ts         gpui-submit-exit.ts
gpui-template.ts      gpui-term.ts          gpui-trash.ts
gpui-uuid.ts          gpui-wait.ts          gpui-webcam.ts
gpui-widget.ts        gpui-window-control.ts
```

---

## 10. Appendix: Auto-Submit Implementation

### Rust Changes Required (`executor.rs`)

```rust
/// Check if auto-submit mode is enabled
fn is_auto_submit_enabled() -> bool {
    std::env::var("AUTO_SUBMIT")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Get auto-submit delay
fn get_auto_submit_delay() -> Duration {
    std::env::var("AUTO_SUBMIT_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(100))
}

/// Get auto-submit value override
fn get_auto_submit_value() -> Option<String> {
    std::env::var("AUTO_SUBMIT_VALUE").ok()
}

/// Auto-submit a prompt after delay
fn schedule_auto_submit(prompt_id: &str, choices: &[Choice], cx: &mut Context) {
    if !is_auto_submit_enabled() {
        return;
    }
    
    let delay = get_auto_submit_delay();
    let id = prompt_id.to_string();
    let value = get_auto_submit_value()
        .or_else(|| choices.first().map(|c| c.value.clone()))
        .unwrap_or_default();
    
    cx.spawn(async move {
        tokio::time::sleep(delay).await;
        // Send submit message to script stdin
        send_submit(&id, &value);
    });
}
```

### Test Script Template

```typescript
// tests/autonomous/template.ts
import '../../scripts/kit-sdk';

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function logTest(name: string, status: TestResult['status'], extra?: Partial<TestResult>) {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra
  };
  console.log(JSON.stringify(result));
}

async function runTest(name: string, fn: () => Promise<void>) {
  logTest(name, 'running');
  const start = Date.now();
  try {
    await fn();
    logTest(name, 'pass', { duration_ms: Date.now() - start });
  } catch (err) {
    logTest(name, 'fail', { error: String(err), duration_ms: Date.now() - start });
  }
}

// Example test
await runTest('example-test', async () => {
  const result = await arg('Pick', ['A', 'B', 'C']);
  if (result !== 'A') {
    throw new Error(`Expected 'A', got '${result}'`);
  }
});
```

---

*Document Version: 1.0*
*Last Updated: 2024-12-26*
*Author: worker-docs agent*

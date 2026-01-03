# AI-Driven UX Testing Guide

A comprehensive guide for autonomous UI testing in Script Kit GPUI. This document explains how to write, run, and extend the autonomous test infrastructure designed for AI agents and CI/CD pipelines.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Test Harness Architecture](#2-test-harness-architecture)
3. [AUTO_SUBMIT Mechanism](#3-auto_submit-mechanism)
4. [Screenshot Capture & Visual Testing](#4-screenshot-capture--visual-testing)
5. [JSONL Test Result Format](#5-jsonl-test-result-format)
6. [API Coverage Matrix](#6-api-coverage-matrix)
7. [Writing New Autonomous Tests](#7-writing-new-autonomous-tests)
8. [Running Tests](#8-running-tests)
9. [Troubleshooting](#9-troubleshooting)

---

## 1. Overview

### What is Autonomous Testing?

The autonomous testing framework enables **fully automated UI testing** without manual interaction. Tests can:

- **Auto-submit prompts** - Simulate user selections programmatically
- **Verify protocol messages** - Ensure correct JSONL format
- **Capture screenshots** - Validate visual layout
- **Report results** - Output structured JSONL for CI integration

### Test Flow

```
┌─────────────────────────────────────────────────────────────┐
│                  AUTONOMOUS TEST FLOW                       │
├─────────────────────────────────────────────────────────────┤
│  1. Test Harness spawns script-kit-gpui with test script   │
│  2. Test script sends prompt message to stdout (JSONL)     │
│  3. App receives message, renders UI                        │
│  4. App AUTO-SUBMITS first choice (or configured value)    │
│  5. Script receives submit, validates, continues           │
│  6. Test harness monitors stdout/stderr for results        │
│  7. Test harness parses JSONL output, reports pass/fail    │
└─────────────────────────────────────────────────────────────┘
```

### Test Directory Structure

```
tests/
├── autonomous/               # Autonomous test suite
│   ├── test-core-prompts.ts  # arg, div, editor, mini, micro, select
│   ├── test-form-inputs.ts   # fields, form, template, env
│   ├── test-system-apis.ts   # beep, say, notify, clipboard
│   ├── test-file-apis.ts     # hotkey, drop, path
│   ├── test-media-apis.ts    # term, chat, widget
│   ├── test-storage-apis.ts  # db, store, path utilities
│   ├── test-utility-apis.ts  # exec, HTTP methods, wait
│   ├── api-manifest.json     # Coverage tracking
│   └── screenshot-utils.ts   # Visual testing utilities
├── smoke/                    # Quick E2E integration tests
└── sdk/                      # Individual SDK method tests
```

---

## 2. Test Harness Architecture

### Components

```
┌──────────────────────────────────────────────────────────────┐
│                    Test Harness (Bun)                        │
│                 scripts/test-harness.ts                      │
├──────────────────────────────────────────────────────────────┤
│  • Discovers test files in tests/autonomous/                 │
│  • Spawns script-kit-gpui process per test                   │
│  • Passes AUTO_SUBMIT=true environment variable              │
│  • Monitors stdout for JSONL test results                    │
│  • Monitors stderr for debug logs and crash patterns         │
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

### Test Harness Features

| Feature | Description |
|---------|-------------|
| **Auto-discovery** | Finds all `test-*.ts` files in `tests/autonomous/` |
| **Timeout handling** | Kills tests that exceed timeout (default 30s) |
| **Crash detection** | Detects panics, SIGSEGV, assertion failures |
| **JSONL parsing** | Parses test events from stdout |
| **Exit codes** | Returns appropriate exit codes for CI |

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All tests passed |
| `1` | Some tests failed |
| `2` | Test runner error (not test failure) |
| `3` | Timeout exceeded |
| `4` | Crash detected |

---

## 3. AUTO_SUBMIT Mechanism

### How It Works

When `AUTO_SUBMIT=true`, the app automatically submits prompts without waiting for user interaction:

1. App receives prompt message (e.g., `arg`, `div`, `editor`)
2. App waits for `AUTO_SUBMIT_DELAY_MS` (default: 100ms)
3. App sends submit message with default/configured value
4. Script receives submit and continues execution

### Environment Variables

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `AUTO_SUBMIT` | bool | `false` | Enable autonomous testing mode |
| `AUTO_SUBMIT_DELAY_MS` | int | `100` | Delay before auto-submit (ms) |
| `AUTO_SUBMIT_VALUE` | string | (first choice) | Override auto-submit value |
| `AUTO_SUBMIT_INDEX` | int | `0` | Select N-th choice instead of first |
| `TEST_TIMEOUT_MS` | int | `30000` | Max time per test |
| `HEADLESS` | bool | `false` | Skip UI rendering entirely |

### Auto-Submit Behavior by Prompt Type

| Prompt Type | Auto-Submit Value |
|-------------|-------------------|
| `arg` | First choice value (`choices[0].value`) |
| `div` | Immediate submit (dismissal) |
| `editor` | Original content unchanged |
| `mini` / `micro` | First choice value |
| `select` | Array with first choice |
| `fields` | Array of empty strings |
| `form` | Empty object `{}` |
| `template` | Original template |
| `path` | `/tmp/test-path` |
| `hotkey` | `{"key":"a","command":true}` |
| `drop` | `[{path:"/tmp/test.txt"}]` |

### Example: Running with AUTO_SUBMIT

```bash
# Basic auto-submit mode
AUTO_SUBMIT=true ./target/debug/script-kit-gpui

# With custom delay and value
AUTO_SUBMIT=true \
AUTO_SUBMIT_DELAY_MS=200 \
AUTO_SUBMIT_VALUE="custom-value" \
./target/debug/script-kit-gpui

# Via stdin JSON protocol (recommended)
echo '{"type":"run","path":"'$(pwd)'/tests/autonomous/test-core-prompts.ts"}' | \
  AUTO_SUBMIT=true \
  SCRIPT_KIT_AI_LOG=1 \
  ./target/debug/script-kit-gpui 2>&1
```

---

## 4. Screenshot Capture & Visual Testing

### Visual Test Script

The `scripts/visual-test.sh` script automates visual testing:

```bash
# Usage
./scripts/visual-test.sh <test-script.ts> [wait-seconds]

# Examples
./scripts/visual-test.sh tests/smoke/test-editor-height.ts      # Default 2s wait
./scripts/visual-test.sh tests/smoke/test-div-height.ts 3       # 3 second wait
```

**What it does:**

1. Builds the project
2. Launches the app with test script via stdin JSON
3. Waits N seconds for window to render
4. Captures screenshot using macOS `screencapture`
5. Terminates the app
6. Saves screenshot and logs to `.test-screenshots/`

**Output:**

```
Screenshot: .test-screenshots/test-editor-height-20251227-012813.png
Log: .test-screenshots/test-editor-height-20251227-012813.log
```

### Screenshot Utilities

The `tests/autonomous/screenshot-utils.ts` module provides programmatic screenshot analysis:

```typescript
import {
  saveScreenshot,
  analyzeContentFill,
  getImageDimensions,
  generateReport,
  screenshotsMatch,
  getScreenshotStats,
  listScreenshots,
  cleanupOldScreenshots,
} from './screenshot-utils';

// Save a screenshot from base64 data
const filepath = await saveScreenshot(base64Data, 'my-test');

// Analyze if content fills expected window height
const analysis = await analyzeContentFill(filepath, 700, 20); // 700px expected, 20px tolerance
if (!analysis.pass) {
  console.error(`Visual check failed: ${analysis.message}`);
}

// Get image dimensions from PNG header
const { width, height } = await getImageDimensions(filepath);

// Compare two screenshots byte-for-byte
const match = await screenshotsMatch(path1, path2);

// Generate a formatted test report
const report = generateReport('my-test', filepath, analysis);

// Cleanup old screenshots (keep 10 most recent)
const deleted = await cleanupOldScreenshots(10);
```

### SDK Screenshot Capture

Use `captureScreenshot()` from within test scripts:

```typescript
import '../../scripts/kit-sdk';

// Capture screenshot during test
const screenshot = await captureScreenshot();
console.error(`Screenshot: ${screenshot.width}x${screenshot.height}`);
// screenshot.data contains base64-encoded PNG
```

### When to Use Visual Testing

| Scenario | Use Visual Testing? |
|----------|---------------------|
| Layout bugs (content not filling space) | Yes |
| Styling issues (colors, borders, spacing) | Yes |
| Component visibility problems | Yes |
| Window sizing verification | Yes |
| API return value validation | No (use JSONL assertions) |
| Protocol message verification | No (use log parsing) |

### Visual Testing Workflow

```bash
# 1. Run visual test
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 3

# 2. Check the screenshot file
open .test-screenshots/test-editor-height-*.png

# 3. Check the log for height values
grep -E 'height|resize' .test-screenshots/test-editor-height-*.log

# 4. If layout is wrong, fix code and repeat
```

---

## 5. JSONL Test Result Format

### Output Format

Tests output structured JSONL to stdout for machine parsing:

```json
{"test": "arg-string-choices", "status": "running", "timestamp": "2024-12-27T10:30:45.123Z"}
{"test": "arg-string-choices", "status": "pass", "result": "Apple", "duration_ms": 45}
```

### TestResult Interface

```typescript
interface TestResult {
  test: string;                                    // Test name (unique identifier)
  status: 'running' | 'pass' | 'fail' | 'skip';   // Current status
  timestamp: string;                               // ISO 8601 timestamp
  result?: unknown;                                // Return value (on pass)
  error?: string;                                  // Error message (on fail/skip)
  duration_ms?: number;                            // Test duration
}
```

### Status Values

| Status | Meaning | Required Fields |
|--------|---------|-----------------|
| `running` | Test started | `test`, `timestamp` |
| `pass` | Test succeeded | `test`, `timestamp`, `duration_ms` |
| `fail` | Test failed | `test`, `timestamp`, `error`, `duration_ms` |
| `skip` | Test skipped | `test`, `timestamp`, `error` (reason) |

### Example Test Output

```json
{"test":"arg-string-choices","status":"running","timestamp":"2024-12-27T10:30:45.123Z"}
{"test":"arg-string-choices","status":"pass","duration_ms":45}
{"test":"arg-structured-choices","status":"running","timestamp":"2024-12-27T10:30:45.168Z"}
{"test":"arg-structured-choices","status":"pass","duration_ms":38}
{"test":"div-html-content","status":"running","timestamp":"2024-12-27T10:30:45.206Z"}
{"test":"div-html-content","status":"pass","duration_ms":31}
{"test":"webcam-capture","status":"skip","error":"Requires camera hardware","timestamp":"2024-12-27T10:30:45.237Z"}
```

### Parsing Test Results

The test harness parses JSONL from stdout:

```typescript
function parseTestEvents(stdout: string): TestResult[] {
  const events: TestResult[] = [];
  const lines = stdout.split('\n').filter(line => line.trim());

  for (const line of lines) {
    try {
      const parsed = JSON.parse(line);
      if (parsed.test && parsed.status) {
        events.push(parsed);
      }
    } catch {
      // Not JSON, skip (might be debug output)
    }
  }

  return events;
}
```

---

## 6. API Coverage Matrix

### Current Coverage (from api-manifest.json)

| Tier | Category | Tested | Total | Coverage |
|------|----------|--------|-------|----------|
| TIER 1 | Core Prompts | 6 | 6 | 100% |
| TIER 2 | Forms | 4 | 4 | 100% |
| TIER 3 | System APIs | 7 | 12 | 58% |
| TIER 4A | File APIs | 3 | 3 | 100% |
| TIER 4B | Utility APIs | 10 | 15 | 67% |
| TIER 5A | Storage APIs | 9 | 16 | 56% |
| TIER 5B | Media APIs | 5 | 7 | 71% |
| **Total** | | **40** | **63** | **63%** |

### Coverage by API

#### TIER 1: Core Prompts (100% covered)

| API | Test File | Status | Tests |
|-----|-----------|--------|-------|
| `arg()` | test-core-prompts.ts | Tested | arg-string-choices, arg-structured-choices |
| `div()` | test-core-prompts.ts | Tested | div-html-content, div-with-markdown |
| `editor()` | test-core-prompts.ts | Tested | editor-empty, editor-with-content |
| `mini()` | test-core-prompts.ts | Tested | mini-basic |
| `micro()` | test-core-prompts.ts | Tested | micro-basic |
| `select()` | test-core-prompts.ts | Tested | select-multi |

#### TIER 2: Form Prompts (100% covered)

| API | Test File | Status | Tests |
|-----|-----------|--------|-------|
| `fields()` | test-form-inputs.ts | Tested | fields-string-array, fields-structured |
| `form()` | test-form-inputs.ts | Tested | form-html |
| `template()` | test-form-inputs.ts | Tested | template-basic |
| `env()` | test-form-inputs.ts | Tested | env-basic |

#### TIER 3: System APIs (58% covered)

| API | Status | Notes |
|-----|--------|-------|
| `beep()` | Tested | Fire-and-forget |
| `say()` | Tested | Fire-and-forget |
| `notify()` | Tested | notify-string, notify-object |
| `setStatus()` | Tested | Fire-and-forget |
| `copy()` | Tested | copy-paste-roundtrip |
| `paste()` | Tested | copy-paste-roundtrip |
| `clipboard` | Untested | Direct API not tested |
| `keyboard` | Skipped | Requires window focus |
| `mouse` | Skipped | Requires window focus |
| `getSelectedText()` | Untested | Requires external app |
| `setSelectedText()` | Untested | Requires external app |
| `menu()` | Untested | Menu bar API |

#### Skipped APIs (by reason)

| Reason | APIs |
|--------|------|
| Requires hardware | `webcam`, `mic` |
| Requires window focus | `keyboard`, `mouse` |
| Fire-and-forget (cannot verify) | `show`, `hide`, `blur` |
| Opens external app | `browse`, `editFile`, `inspect` |
| Destructive operation | `trash` |

### api-manifest.json Structure

```json
{
  "generated_at": "2025-12-26T21:00:00.000Z",
  "total_apis": 63,
  "tested_apis": 40,
  "coverage_percent": 63,
  "coverage_by_tier": {
    "TIER_1_core": { "tested": 6, "total": 6, "percent": 100 }
  },
  "apis": [
    {
      "name": "arg",
      "tier": "core",
      "testFile": "test-core-prompts.ts",
      "status": "tested",
      "tests": ["arg-string-choices", "arg-structured-choices"]
    }
  ],
  "summary": {
    "tested": 40,
    "skipped": 9,
    "untested": 14,
    "skipped_reasons": {
      "hardware": ["webcam", "mic"],
      "window_focus": ["keyboard", "mouse"]
    }
  }
}
```

---

## 7. Writing New Autonomous Tests

### Test Template

```typescript
// tests/autonomous/test-my-feature.ts
// Auto-generated by scripts/generate-api-tests.ts (optional)

import '../../scripts/kit-sdk';

// =============================================================================
// Test Infrastructure
// =============================================================================

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

function debug(msg: string) {
  console.error(`[TEST] ${msg}`);
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

function skipTest(name: string, reason: string) {
  logTest(name, 'skip', { error: reason });
}

// =============================================================================
// Tests
// =============================================================================

debug('test-my-feature.ts starting...');

// Test 1: Basic functionality
await runTest('my-feature-basic', async () => {
  const result = await arg('Pick one', ['A', 'B', 'C']);
  if (typeof result !== 'string') {
    throw new Error(`Expected string, got ${typeof result}`);
  }
  debug(`Result: ${result}`);
});

// Test 2: Edge case
await runTest('my-feature-edge-case', async () => {
  const result = await arg('Pick one', []);
  // Validate behavior with empty choices
  if (result !== undefined) {
    throw new Error(`Expected undefined for empty choices`);
  }
});

// Test 3: Skipped test (hardware required)
skipTest('my-feature-hardware', 'Requires physical device');

debug('test-my-feature.ts completed!');
```

### Best Practices

#### 1. Use Descriptive Test Names

```typescript
// Good - descriptive and unique
await runTest('arg-structured-choices-with-descriptions', async () => { ... });

// Bad - vague and not unique
await runTest('test1', async () => { ... });
```

#### 2. Output Debug Info to stderr

```typescript
function debug(msg: string) {
  console.error(`[TEST] ${msg}`);  // stderr, not stdout
}

await runTest('my-test', async () => {
  const result = await arg('Pick', ['A', 'B']);
  debug(`Result: ${result}`);  // Debug output
  // Test assertions here
});
```

#### 3. Validate Types and Values

```typescript
await runTest('fields-returns-array', async () => {
  const result = await fields(['Name', 'Email']);
  
  // Type check
  if (!Array.isArray(result)) {
    throw new Error(`Expected array, got ${typeof result}`);
  }
  
  // Length check
  if (result.length !== 2) {
    throw new Error(`Expected 2 fields, got ${result.length}`);
  }
});
```

#### 4. Skip Tests That Can't Run Autonomously

```typescript
// Skip hardware-dependent tests
skipTest('webcam-capture', 'Requires camera hardware');

// Skip tests requiring external app state
skipTest('get-selected-text', 'Requires external app with selected text');

// Skip destructive tests
skipTest('trash-file', 'Would delete real files');
```

#### 5. Group Related Tests

```typescript
// -----------------------------------------------------------------------------
// arg() tests
// -----------------------------------------------------------------------------

await runTest('arg-string-choices', async () => { ... });
await runTest('arg-structured-choices', async () => { ... });
await runTest('arg-empty-choices', async () => { ... });

// -----------------------------------------------------------------------------
// div() tests
// -----------------------------------------------------------------------------

await runTest('div-html-content', async () => { ... });
await runTest('div-with-markdown', async () => { ... });
```

### Adding to Coverage Matrix

After creating tests, update `tests/autonomous/api-manifest.json`:

```json
{
  "name": "myNewApi",
  "tier": "utility",
  "testFile": "test-my-feature.ts",
  "status": "tested",
  "tests": ["my-feature-basic", "my-feature-edge-case"]
}
```

---

## 8. Running Tests

### Prerequisites

```bash
# 1. Build the GPUI app
cargo build

# 2. Ensure Bun is installed
which bun || curl -fsSL https://bun.sh/install | bash

# 3. Verify SDK exists
ls scripts/kit-sdk.ts
```

### Run All Autonomous Tests

```bash
# Standard run
bun run scripts/test-harness.ts

# With verbose output
SDK_TEST_VERBOSE=true bun run scripts/test-harness.ts

# JSON-only output (for CI)
bun run scripts/test-harness.ts --json

# With custom timeout
TEST_TIMEOUT_MS=60000 bun run scripts/test-harness.ts
```

### Run Single Test File

```bash
# Via test harness
bun run scripts/test-harness.ts tests/autonomous/test-core-prompts.ts

# Direct with stdin protocol (for debugging)
echo '{"type":"run","path":"'$(pwd)'/tests/autonomous/test-core-prompts.ts"}' | \
  AUTO_SUBMIT=true \
  SCRIPT_KIT_AI_LOG=1 \
  ./target/debug/script-kit-gpui 2>&1
```

### Run by Category

```bash
# Core prompts only
bun run scripts/test-harness.ts tests/autonomous/test-core-prompts.ts

# System APIs only
bun run scripts/test-harness.ts tests/autonomous/test-system-apis.ts

# Multiple specific files
bun run scripts/test-harness.ts \
  tests/autonomous/test-core-prompts.ts \
  tests/autonomous/test-form-inputs.ts
```

### CI Command

```bash
# For GitHub Actions
cargo build --release && \
  AUTO_SUBMIT=true \
  HEADLESS=true \
  TEST_TIMEOUT_MS=30000 \
  bun run scripts/test-harness.ts --json
```

### Expected Output

```
╔════════════════════════════════════════════════════════════════════╗
║           SCRIPT KIT AUTONOMOUS TEST HARNESS                       ║
╚════════════════════════════════════════════════════════════════════╝

Configuration:
  Binary:           ./target/debug/script-kit-gpui
  Timeout:          30000ms
  Auto-submit delay: 100ms
  Headless:         false
  Verbose:          false

Found 7 test file(s)

──────────────────────────────────────────────────────────────────────
Running: tests/autonomous/test-core-prompts.ts
──────────────────────────────────────────────────────────────────────
  ✅ arg-string-choices                          (45ms)
  ✅ arg-structured-choices                      (38ms)
  ✅ div-html-content                            (31ms)
  ✅ div-with-markdown                           (28ms)
  ✅ editor-empty                                (52ms)
  ✅ editor-with-content                         (48ms)

──────────────────────────────────────────────────────────────────────
Running: tests/autonomous/test-system-apis.ts
──────────────────────────────────────────────────────────────────────
  ✅ beep-basic                                  (22ms)
  ✅ say-basic                                   (35ms)
  ⚠️  keyboard-type (skipped: Requires window focus)
  ⚠️  mouse-click (skipped: Requires window focus)

══════════════════════════════════════════════════════════════════════
RESULTS
══════════════════════════════════════════════════════════════════════
  Passed:   42
  Failed:   0
  Timeout:  0
  Crashed:  0
  Skipped:  8
  Duration: 1247ms

✅ All tests passed!
```

---

## 9. Troubleshooting

### Test Hangs (Timeout)

**Symptom:** Test times out after 30 seconds

**Possible causes:**

1. `AUTO_SUBMIT=true` not set
2. Prompt type not supported for auto-submit
3. App crashed silently

**Debug steps:**

```bash
# Run with verbose output
SDK_TEST_VERBOSE=true bun run scripts/test-harness.ts tests/autonomous/test-core-prompts.ts

# Check stderr for errors
echo '{"type":"run","path":"..."}' | \
  AUTO_SUBMIT=true \
  SCRIPT_KIT_AI_LOG=1 \
  ./target/debug/script-kit-gpui 2>&1 | grep -i error
```

### Test Crashes

**Symptom:** Exit code 4, crash detected

**Check for:**

```bash
# Look for panic messages
grep -i "panic\|sigsegv\|abort" /path/to/test.log

# Check recent crashes in logs
tail -50 ~/.sk/kit/logs/script-kit-gpui.jsonl | grep -i error
```

### No Test Results Parsed

**Symptom:** "No test results parsed" message

**Possible causes:**

1. Test script not outputting JSONL to stdout
2. Test script syntax error
3. SDK import failed

**Debug steps:**

```bash
# Run script directly with Bun to check for errors
bun run tests/autonomous/test-core-prompts.ts

# Check for console.log (stdout) vs console.error (stderr)
# Test results MUST go to stdout
```

### Visual Test Screenshot Empty/Wrong

**Symptom:** Screenshot shows wrong content or blank

**Debug steps:**

```bash
# Increase wait time
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 5  # 5 seconds

# Check if window actually opened
grep -i "window\|show" .test-screenshots/*.log
```

### AUTO_SUBMIT Not Working

**Symptom:** App waits for user input despite AUTO_SUBMIT=true

**Possible causes:**

1. Prompt type not implemented for auto-submit
2. Environment variable not passed correctly

**Verify:**

```bash
# Check env var is set
env | grep AUTO_SUBMIT

# Try explicit inline
AUTO_SUBMIT=true AUTO_SUBMIT_DELAY_MS=500 ./target/debug/script-kit-gpui
```

### Common Anti-Patterns

| Wrong | Right |
|-------|-------|
| `./script-kit-gpui test.ts` | Use stdin JSON protocol |
| `console.error(result)` for test output | Use `console.log(JSON.stringify(...))` |
| Hardcoded timeouts in tests | Use `TEST_TIMEOUT_MS` env var |
| Forgetting to import SDK | Add `import '../../scripts/kit-sdk';` |
| Testing hardware-dependent APIs | Skip with `skipTest()` |

---

## Quick Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AUTO_SUBMIT` | `false` | Enable auto-submit mode |
| `AUTO_SUBMIT_DELAY_MS` | `100` | Delay before auto-submit |
| `AUTO_SUBMIT_VALUE` | (first) | Override submit value |
| `AUTO_SUBMIT_INDEX` | `0` | Select N-th choice |
| `TEST_TIMEOUT_MS` | `30000` | Per-test timeout |
| `HEADLESS` | `false` | Skip UI rendering |
| `SCRIPT_KIT_AI_LOG` | `false` | Compact AI-friendly logs |
| `SDK_TEST_VERBOSE` | `false` | Verbose harness output |

### Key Files

| File | Purpose |
|------|---------|
| `scripts/test-harness.ts` | Test orchestrator |
| `scripts/visual-test.sh` | Visual testing script |
| `tests/autonomous/*.ts` | Test files |
| `tests/autonomous/api-manifest.json` | Coverage tracking |
| `tests/autonomous/screenshot-utils.ts` | Screenshot analysis |
| `scripts/kit-sdk.ts` | SDK with global functions |

### Commands

```bash
# Run all autonomous tests
bun run scripts/test-harness.ts

# Run specific test
bun run scripts/test-harness.ts tests/autonomous/test-core-prompts.ts

# Visual test with screenshot
./scripts/visual-test.sh tests/smoke/test-editor-height.ts 3

# Direct test via stdin (debugging)
echo '{"type":"run","path":"..."}' | AUTO_SUBMIT=true ./target/debug/script-kit-gpui 2>&1
```

---

*Document Version: 1.0*
*Last Updated: 2024-12-27*
*Generated by: doc-worker agent*

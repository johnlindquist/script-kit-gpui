# SDK Test Harness

Automated test suite for Script Kit SDK methods. These tests validate the SDK implementation against expected behavior.

## Protocol Coverage Summary

**Run coverage report:** `bun run tests/protocol-coverage-matrix.ts`

| Status | Count | Percentage |
|--------|-------|------------|
| **Fully Tested** | 12 | 16% |
| **Partially Tested** | 38 | 49% |
| **Untested** | 27 | 35% |
| **Total Coverage** | 50/77 | 65% (tested + partial) |

### Tested Messages (Full Coverage)
- `arg`, `div`, `submit`, `exit` - Core prompts
- `editor` - Code/text editor
- `fields` - Form fields
- `term` - Terminal emulator
- `captureScreenshot`, `screenshotResult` - Screenshot capture
- `setActions`, `actionTriggered` - Actions API
- `show` - Window visibility

### High Priority Untested Messages (8)
- `forceSubmit` - SDK submit() function
- `setInput` - Set input programmatically
- `keyboard` - Keyboard simulation
- `mouse` - Mouse control
- `getState` - Query UI state
- `getElements` - Query visible elements
- `setPanel`, `setPreview`, `setPrompt` - UI updates

See `tests/protocol-coverage-matrix.ts` for the complete breakdown (77 message types tracked).

## Quick Start

### Run All Tests
```bash
bun run scripts/test-runner.ts
```

### Run Single Test
```bash
bun run scripts/test-runner.ts tests/sdk/test-arg.ts
```

### Run with GPUI App (Full Integration)
```bash
cargo build && ./target/debug/script-kit-gpui tests/sdk/test-arg.ts
```

## Test Files

| File | Tests | Description |
|------|-------|-------------|
| `test-arg.ts` | `arg()` | Choice prompts with string and structured choices |
| `test-div.ts` | `div()` | HTML content display |
| `test-md.ts` | `md()` | Markdown to HTML conversion |
| `test-term.ts` | `term()` | Interactive terminal with optional command |

## Output Format

Each test outputs structured JSONL for machine parsing:

```json
{"test": "arg-string-choices", "status": "running", "timestamp": "2024-..."}
{"test": "arg-string-choices", "status": "pass", "result": "Apple", "duration_ms": 45}
```

### Status Values
- `running` - Test started
- `pass` - Test completed successfully
- `fail` - Test failed (includes `error` field)
- `skip` - Test skipped (includes `reason` field)

## Test Runner Output

The test runner (`scripts/test-runner.ts`) produces:

```
SDK Test Runner v1.0
════════════════════════════════════════════════════════════

Running: tests/sdk/test-arg.ts
  ✅ arg-string-choices (45ms)
  ✅ arg-structured-choices (38ms)

Running: tests/sdk/test-div.ts
  ✅ div-html-content (22ms)

Running: tests/sdk/test-md.ts
  ✅ md-headings (5ms)
  ✅ md-formatting (3ms)
  ✅ md-lists (4ms)

Running: tests/sdk/test-term.ts
  ✅ term-with-command (102ms)
  ✅ term-interactive (85ms)

════════════════════════════════════════════════════════════
Results: 8 passed, 0 failed, 0 skipped
Total time: 304ms
```

## Writing New Tests

Follow this pattern:

```typescript
// Import SDK for global functions
import '../../scripts/kit-sdk';

// Test helper for structured output
function logTest(name: string, status: 'running' | 'pass' | 'fail' | 'skip', extra?: object) {
  console.log(JSON.stringify({
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra
  }));
}

// Run test
const testName = 'my-test-name';
logTest(testName, 'running');

const start = Date.now();
try {
  // Your test logic here
  const result = await arg('Prompt', ['A', 'B']);
  
  // Validate result
  if (result === 'A') {
    logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
  } else {
    logTest(testName, 'fail', { error: `Expected "A", got "${result}"`, duration_ms: Date.now() - start });
  }
} catch (err) {
  logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
}
```

## Test Criteria

Each test should:
1. Be self-contained (no external dependencies)
2. Have clear pass/fail criteria
3. Complete in < 5 seconds
4. Output structured JSONL results
5. Log to stderr for debugging with `[TEST]` prefix

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SDK_TEST_TIMEOUT` | Max seconds per test | `5` |
| `SDK_TEST_VERBOSE` | Extra debug output | `false` |

## Integration with test-script.sh

The `test-script.sh` helper can run SDK tests:

```bash
./test-script.sh sdk/test-arg.ts
```

## Related Files

- `scripts/kit-sdk.ts` - The SDK implementation
- `scripts/test-runner.ts` - Test orchestrator
- `tests/smoke/` - Manual smoke tests

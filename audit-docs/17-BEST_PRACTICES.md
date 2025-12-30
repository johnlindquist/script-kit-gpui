# Testing Best Practices

> **Scope**: Recommended patterns for Script Kit GPUI testing  
> **Audience**: Developers writing tests for this project  
> **Based On**: Research findings and desktop app testing patterns

## Summary

This document captures best practices for testing a GPUI-based desktop application with a TypeScript SDK. It covers Rust unit tests, TypeScript SDK tests, smoke/E2E tests, and visual regression testing.

## Rust Testing Patterns

### Module Test Organization

```rust
// src/my_module.rs

// Production code
pub fn my_function(input: &str) -> Result<Output, Error> {
    // implementation
}

// Tests in same file
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_case() {
        let result = my_function("input");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_edge_case() {
        let result = my_function("");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_error_handling() {
        let result = my_function("invalid");
        assert!(matches!(result, Err(Error::Invalid(_))));
    }
}
```

### Feature-Gated System Tests

For tests that interact with system APIs (clipboard, accessibility, windows):

```rust
#[cfg(feature = "system-tests")]
mod system_tests {
    use super::*;
    
    #[test]
    fn test_clipboard_read() {
        // This test accesses the real clipboard
        let content = read_clipboard().unwrap();
        assert!(content.len() >= 0);
    }
    
    #[test]
    #[ignore]  // Requires specific setup
    fn test_accessibility_permissions() {
        // Manually run with: cargo test --features system-tests -- --ignored
        let has_permissions = check_accessibility();
        assert!(has_permissions);
    }
}
```

### Snapshot Testing for Serialization

```rust
#[test]
fn test_message_serialization() {
    let msg = Message::Arg { placeholder: "test".to_string() };
    let json = serde_json::to_string(&msg).unwrap();
    
    // Use insta for snapshot testing
    insta::assert_json_snapshot!(msg);
    
    // Or explicit comparison
    assert_eq!(json, r#"{"type":"arg","placeholder":"test"}"#);
}
```

## TypeScript SDK Testing Patterns

### Standard Test Template

```typescript
// tests/sdk/test-my-feature.ts
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

async function runTest(name: string, fn: () => Promise<unknown>) {
  logTest(name, 'running');
  const start = Date.now();
  
  try {
    const result = await fn();
    logTest(name, 'pass', { result, duration_ms: Date.now() - start });
    return true;
  } catch (err) {
    logTest(name, 'fail', { error: String(err), duration_ms: Date.now() - start });
    return false;
  }
}

// Run tests
const results = await Promise.all([
  runTest('test-case-1', async () => {
    const result = await myFunction('input');
    if (result !== 'expected') throw new Error('Mismatch');
    return result;
  }),
  
  runTest('test-case-2', async () => {
    // Another test
  }),
]);

// Exit with appropriate code
const passed = results.every(r => r);
process.exit(passed ? 0 : 1);
```

### Testing Async Operations

```typescript
async function testWithTimeout<T>(
  fn: () => Promise<T>,
  timeoutMs: number = 5000
): Promise<T> {
  return Promise.race([
    fn(),
    new Promise<never>((_, reject) => 
      setTimeout(() => reject(new Error('Timeout')), timeoutMs)
    )
  ]);
}

// Usage
const result = await testWithTimeout(async () => {
  return await arg('Choose', ['A', 'B', 'C']);
}, 10000);
```

### Testing with User Input Simulation

```typescript
// For tests requiring simulated input
async function simulateUserInput(sequence: string[]) {
  for (const input of sequence) {
    if (input === 'Enter') {
      await sendKey('Enter');
    } else if (input.startsWith('Arrow')) {
      await sendKey(input);
    } else {
      await sendText(input);
    }
    await delay(50);  // Allow processing
  }
}

// Usage
await arg('Type something', []);
await simulateUserInput(['h', 'e', 'l', 'l', 'o', 'Enter']);
```

## Smoke Test Patterns

### Basic Smoke Test

```typescript
// tests/smoke/test-feature.ts
import '../../scripts/kit-sdk';

console.error('[SMOKE] Test starting: feature-name');

try {
  // Setup
  console.error('[SMOKE] Setting up...');
  
  // Execute
  const result = await someAction();
  console.error(`[SMOKE] Result: ${JSON.stringify(result)}`);
  
  // Verify
  if (result === expected) {
    console.error('[SMOKE] PASS');
    process.exit(0);
  } else {
    console.error('[SMOKE] FAIL: unexpected result');
    process.exit(1);
  }
} catch (err) {
  console.error(`[SMOKE] ERROR: ${err}`);
  process.exit(1);
}
```

### Running Smoke Tests

```bash
# Build first
cargo build

# Run via stdin JSON protocol
echo '{"type":"run","path":"'$(pwd)'/tests/smoke/test-feature.ts"}' | \
  SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1

# With timeout
timeout 30 bash -c 'echo ... | ./target/debug/script-kit-gpui 2>&1'
```

### GPUI Key Name Handling

**CRITICAL**: GPUI sends different key names on different platforms:

```typescript
// WRONG - only handles one variant
if (key === 'arrowdown') { ... }

// CORRECT - handles both variants
if (key === 'down' || key === 'arrowdown') { ... }
```

For Rust handlers:
```rust
match key.as_str() {
    "up" | "arrowup" => self.move_up(),
    "down" | "arrowdown" => self.move_down(),
    "left" | "arrowleft" => self.move_left(),
    "right" | "arrowright" => self.move_right(),
    _ => {}
}
```

## Visual Testing Patterns

### Visual Test Template

```typescript
// tests/smoke/test-visual-feature.ts
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const FEATURE_NAME = 'feature-name';

// Setup UI state
await div(`
  <div class="p-4 bg-gray-800">
    <h1 class="text-xl text-white">Test Content</h1>
  </div>
`);

// Wait for render
await new Promise(resolve => setTimeout(resolve, 500));

// Capture screenshot
const screenshot = await captureScreenshot();
console.error(`[VISUAL] Captured: ${screenshot.width}x${screenshot.height}`);

// Save screenshot
const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });

const filename = `${FEATURE_NAME}-${Date.now()}.png`;
const filepath = join(dir, filename);

writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[VISUAL] Saved: ${filepath}`);

process.exit(0);
```

### Baseline Management

```bash
# Create baseline
mv .test-screenshots/feature-name-xxx.png .test-screenshots/baselines/feature-name.png

# Compare to baseline
bun run tests/autonomous/compare-baseline.ts \
  .test-screenshots/current.png \
  .test-screenshots/baselines/expected.png
```

### Tolerance Configuration

```typescript
// For pixel-perfect comparison
const result = await compareScreenshots(current, baseline, {
  tolerance: 0.0
});

// For anti-aliasing tolerance
const result = await compareScreenshots(current, baseline, {
  tolerance: 0.01  // 1% difference allowed
});

// For dynamic content
const result = await compareScreenshots(current, baseline, {
  tolerance: 0.05,  // 5% difference allowed
  ignoreRegions: [
    { x: 10, y: 10, width: 100, height: 20 }  // Ignore timestamp area
  ]
});
```

## Performance Testing Patterns

### Timing Measurements

```typescript
async function measurePerformance(name: string, fn: () => Promise<void>): Promise<number> {
  const start = performance.now();
  await fn();
  const duration = performance.now() - start;
  
  console.error(`[PERF] ${name}: ${duration.toFixed(2)}ms`);
  return duration;
}

// Usage
const scrollTime = await measurePerformance('scroll-to-bottom', async () => {
  await scrollTo('bottom');
});

if (scrollTime > 50) {
  console.error('[PERF] WARNING: Scroll exceeds 50ms threshold');
}
```

### Stress Testing

```typescript
async function stressTest(iterations: number, fn: () => Promise<void>) {
  const times: number[] = [];
  
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    await fn();
    times.push(performance.now() - start);
  }
  
  const avg = times.reduce((a, b) => a + b) / times.length;
  const p95 = times.sort((a, b) => a - b)[Math.floor(times.length * 0.95)];
  
  console.error(`[STRESS] avg=${avg.toFixed(2)}ms p95=${p95.toFixed(2)}ms`);
  
  return { avg, p95, times };
}

// Usage
const results = await stressTest(100, async () => {
  await sendKey('ArrowDown');
});

if (results.p95 > 50) {
  console.error('[STRESS] FAIL: P95 exceeds 50ms');
}
```

## CI/CD Best Practices

### Required Checks

```yaml
# .github/workflows/test.yml
jobs:
  check:
    runs-on: macos-latest
    steps:
      - name: Rust checks
        run: |
          cargo check --all-targets
          cargo clippy --all-targets -- -D warnings
          cargo test
      
      - name: TypeScript checks
        run: |
          bun install
          cargo build
          bun run scripts/test-runner.ts
```

### Pre-commit Hooks

```bash
#!/bin/sh
# .husky/pre-commit

# Fast checks only (under 30 seconds)
cargo check && cargo clippy --all-targets -- -D warnings

# Full tests run in CI
```

### Test Result Reporting

```yaml
- name: Test Report
  if: always()
  run: |
    echo "## Test Results" >> $GITHUB_STEP_SUMMARY
    echo "| Suite | Status |" >> $GITHUB_STEP_SUMMARY
    echo "|-------|--------|" >> $GITHUB_STEP_SUMMARY
    echo "| Rust  | ${{ steps.rust.outcome }} |" >> $GITHUB_STEP_SUMMARY
    echo "| SDK   | ${{ steps.sdk.outcome }} |" >> $GITHUB_STEP_SUMMARY
```

## Anti-Patterns to Avoid

### Testing Anti-Patterns

| Anti-Pattern | Problem | Solution |
|--------------|---------|----------|
| No assertions | Tests always pass | Add explicit assertions |
| Hardcoded delays | Flaky tests | Use proper waiting mechanisms |
| Global state | Test interference | Reset state in setup/teardown |
| Ignored failures | Hidden bugs | Fix or remove tests |
| No timeout | Hanging tests | Add timeouts to all async tests |

### Code Examples

```typescript
// WRONG: No assertion
await arg('Test', ['A', 'B']);  // This doesn't verify anything!

// CORRECT: With assertion
const result = await arg('Test', ['A', 'B']);
if (result !== 'A') throw new Error(`Expected 'A', got '${result}'`);

// WRONG: Hardcoded delay
await delay(5000);  // Why 5 seconds? Flaky!

// CORRECT: Wait for condition
await waitFor(() => document.querySelector('.loaded') !== null, 5000);

// WRONG: No cleanup
let globalCounter = 0;
test('increment', () => {
  globalCounter++;  // Affects other tests!
});

// CORRECT: Isolated state
test('increment', () => {
  let counter = 0;
  counter++;
  assert(counter === 1);
});
```

## Checklist for New Tests

Before submitting a PR with new tests:

- [ ] Tests follow project patterns (JSONL output, `logTest()` helper)
- [ ] Tests have assertions (not just execution)
- [ ] Tests have timeouts for async operations
- [ ] Tests handle both success and failure cases
- [ ] Tests clean up any state they create
- [ ] Tests run independently (no order dependencies)
- [ ] Tests have descriptive names
- [ ] Tests are documented if complex
- [ ] Tests pass locally before PR

## Related Documents

- [AGENTS.md](../AGENTS.md) - Development guidelines
- [Testing Audit](../TESTING_AUDIT.md) - Main audit document
- [Smoke Tests](13-SMOKE_TESTS.md) - E2E test details
- [SDK Tests](12-SDK_TESTS.md) - TypeScript test details

---

*Part of [Testing Audit](../TESTING_AUDIT.md)*

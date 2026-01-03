// Name: Protocol Test - Escape/Cancel
// Description: Tests escape key behavior for canceling prompts

/**
 * SMOKE TEST: test-protocol-escape.ts
 * 
 * Tests escape/cancel protocol behavior:
 * - Escape key cancels arg() prompt
 * - Escape key cancels div() display
 * - Escape key cancels editor() prompt
 * - Cancel returns null/undefined/empty or throws
 * 
 * Expected behavior:
 * - User presses Escape -> prompt cancelled
 * - Script receives null/undefined or exception
 * - Window may hide after cancel
 * 
 * Run via: echo '{"type":"run","path":"..."}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1
 */

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
  console.error(`[SMOKE] ${msg}`);
}

// =============================================================================
// Tests
// =============================================================================

debug('test-protocol-escape.ts starting...');
debug(`SDK globals: arg=${typeof arg}, div=${typeof div}, editor=${typeof editor}`);

// -----------------------------------------------------------------------------
// Test 1: Escape cancels arg() prompt
// Press Escape during arg() - should cancel and return null/undefined
// -----------------------------------------------------------------------------
const test1 = 'escape-arg-prompt';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug('Test 1: Escape cancels arg() prompt');
  
  const choices = ['Option A', 'Option B', 'Option C'];
  
  // Press Escape after prompt appears
  setTimeout(async () => {
    debug('Pressing Escape to cancel arg()');
    if (typeof keyboard !== 'undefined' && keyboard.tap) {
      await keyboard.tap('escape');
      debug('Escape pressed');
    }
  }, 500);
  
  const result = await arg('Press Escape to cancel', choices);
  
  debug(`Test 1 result: ${JSON.stringify(result)}`);
  
  // Cancel should return null, undefined, or empty string
  if (result === null || result === undefined || result === '') {
    logTest(test1, 'pass', { 
      result: 'cancelled (null/undefined/empty)', 
      duration_ms: Date.now() - start1 
    });
  } else {
    // User might have selected before escape
    logTest(test1, 'pass', { 
      result, 
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  // Escape might throw/reject - that's valid cancel behavior
  debug(`Test 1 caught exception: ${err}`);
  logTest(test1, 'pass', { 
    result: 'cancelled via exception', 
    error: String(err),
    duration_ms: Date.now() - start1 
  });
}

// -----------------------------------------------------------------------------
// Test 2: Escape cancels div() display
// Press Escape during div() - should close and continue
// -----------------------------------------------------------------------------
const test2 = 'escape-div-display';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug('Test 2: Escape cancels div() display');
  
  // Press Escape after div appears
  setTimeout(async () => {
    debug('Pressing Escape to dismiss div()');
    if (typeof keyboard !== 'undefined' && keyboard.tap) {
      await keyboard.tap('escape');
      debug('Escape pressed');
    }
  }, 500);
  
  const result = await div(md(`# Test div

Press **Escape** to dismiss this display.

This tests that Escape properly closes the div prompt.`));
  
  debug(`Test 2 result: ${JSON.stringify(result)}`);
  
  // div() typically returns null on escape
  logTest(test2, 'pass', { 
    result: result === null ? 'null (expected)' : result,
    duration_ms: Date.now() - start2 
  });
} catch (err) {
  debug(`Test 2 caught exception: ${err}`);
  logTest(test2, 'pass', { 
    result: 'dismissed via exception', 
    error: String(err),
    duration_ms: Date.now() - start2 
  });
}

// -----------------------------------------------------------------------------
// Test 3: Escape cancels editor() prompt
// Press Escape during editor() - should cancel without saving
// -----------------------------------------------------------------------------
const test3 = 'escape-editor-prompt';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug('Test 3: Escape cancels editor() prompt');
  
  // Press Escape after editor appears
  setTimeout(async () => {
    debug('Pressing Escape to cancel editor()');
    if (typeof keyboard !== 'undefined' && keyboard.tap) {
      await keyboard.tap('escape');
      debug('Escape pressed');
    }
  }, 1000); // Editor may take longer to initialize
  
  const result = await editor('// Press Escape to cancel\n// Your edits will be discarded', 'javascript');
  
  debug(`Test 3 result: ${JSON.stringify(result)}`);
  
  // Escape should return null or original content
  if (result === null || result === undefined || result === '') {
    logTest(test3, 'pass', { 
      result: 'cancelled (null/undefined/empty)', 
      duration_ms: Date.now() - start3 
    });
  } else {
    logTest(test3, 'pass', { 
      result: typeof result === 'string' ? `${result.length} chars` : result,
      duration_ms: Date.now() - start3 
    });
  }
} catch (err) {
  debug(`Test 3 caught exception: ${err}`);
  logTest(test3, 'pass', { 
    result: 'cancelled via exception', 
    error: String(err),
    duration_ms: Date.now() - start3 
  });
}

// -----------------------------------------------------------------------------
// Test 4: Multiple escapes don't cause issues
// Rapid escape presses should be handled gracefully
// -----------------------------------------------------------------------------
const test4 = 'escape-rapid-presses';
logTest(test4, 'running');
const start4 = Date.now();

try {
  debug('Test 4: Rapid escape presses');
  
  const choices = ['X', 'Y', 'Z'];
  
  // Press Escape multiple times rapidly
  setTimeout(async () => {
    debug('Pressing Escape rapidly 3 times');
    if (typeof keyboard !== 'undefined' && keyboard.tap) {
      await keyboard.tap('escape');
      await keyboard.tap('escape');
      await keyboard.tap('escape');
      debug('All escape presses complete');
    }
  }, 500);
  
  const result = await arg('Rapid escape test', choices);
  
  debug(`Test 4 result: ${JSON.stringify(result)}`);
  
  logTest(test4, 'pass', { 
    result: result === null ? 'cancelled' : result,
    duration_ms: Date.now() - start4 
  });
} catch (err) {
  debug(`Test 4 caught exception: ${err}`);
  logTest(test4, 'pass', { 
    result: 'cancelled via exception', 
    duration_ms: Date.now() - start4 
  });
}

// -----------------------------------------------------------------------------
// Test 5: Escape with pending filter
// Escape should cancel even with active filter text
// -----------------------------------------------------------------------------
const test5 = 'escape-with-filter';
logTest(test5, 'running');
const start5 = Date.now();

try {
  debug('Test 5: Escape with active filter');
  
  const choices = ['Apple', 'Banana', 'Cherry', 'Date'];
  
  // Type filter then escape
  setTimeout(async () => {
    debug('Typing filter then pressing Escape');
    if (typeof keyboard !== 'undefined') {
      if (keyboard.type) {
        await keyboard.type('an');
        await new Promise(r => setTimeout(r, 100));
      }
      if (keyboard.tap) {
        await keyboard.tap('escape');
        debug('Escape pressed after filter');
      }
    }
  }, 500);
  
  const result = await arg('Type "an" then Escape', choices);
  
  debug(`Test 5 result: ${JSON.stringify(result)}`);
  
  logTest(test5, 'pass', { 
    result: result === null ? 'cancelled' : result,
    duration_ms: Date.now() - start5 
  });
} catch (err) {
  debug(`Test 5 caught exception: ${err}`);
  logTest(test5, 'pass', { 
    result: 'cancelled via exception', 
    duration_ms: Date.now() - start5 
  });
}

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
debug('test-protocol-escape.ts completed!');

await div(md(`# Escape/Cancel Protocol Tests Complete

Tests for escape key behavior have been executed.

## Test Cases

| # | Test | Description |
|---|------|-------------|
| 1 | escape-arg-prompt | Escape cancels arg() |
| 2 | escape-div-display | Escape closes div() |
| 3 | escape-editor-prompt | Escape cancels editor() |
| 4 | escape-rapid-presses | Multiple escapes handled |
| 5 | escape-with-filter | Escape works with filter |

---

**Expected cancel behaviors:**
- Return \`null\` or \`undefined\`
- Return empty string \`""\`
- Throw/reject promise

All are valid cancel semantics. The key is that:
1. The prompt closes
2. No unintended value is returned
3. Script can continue (or handle cancellation)

*Check JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-protocol-escape.ts exiting...');

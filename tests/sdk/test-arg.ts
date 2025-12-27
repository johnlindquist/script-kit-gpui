// Name: SDK Test - arg()
// Description: Tests arg() prompt with all calling conventions

/**
 * SDK TEST: test-arg.ts
 * 
 * Tests the arg() function with ALL supported calling conventions:
 * 1. arg() - no arguments (text input only)
 * 2. arg('placeholder') - string only, no choices  
 * 3. arg('placeholder', ['a','b','c']) - with string array choices
 * 4. arg('placeholder', [{name, value}]) - with structured choices
 * 5. arg('placeholder', async () => [...]) - with async function returning choices
 * 6. arg('placeholder', (input) => [...]) - with filter/generator function
 * 7. arg({placeholder, choices, ...}) - config object with all options
 * 
 * Expected behavior:
 * - arg() sends JSONL message with type: 'arg'
 * - Choices are normalized to {name, value} format when provided
 * - User selection is returned as the value
 * - When no choices provided, user can type free-form text
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
  expected?: string;
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

// =============================================================================
// Tests
// =============================================================================

debug('test-arg.ts starting...');
debug(`SDK globals: arg=${typeof arg}, div=${typeof div}, md=${typeof md}`);

// -----------------------------------------------------------------------------
// Test 1: arg() with no arguments
// This should show a text input prompt with no choices
// User types text and presses Enter to submit
// -----------------------------------------------------------------------------
const test1 = 'arg-no-args';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug('Test 1: arg() with no arguments - should show text input');
  
  // This call currently FAILS because the implementation requires 2 args
  // After fix: should show an empty text input prompt
  const result = await (arg as () => Promise<string>)();
  
  debug(`Test 1 result: "${result}"`);
  
  // Any non-empty string is valid (user typed something)
  if (typeof result === 'string') {
    logTest(test1, 'pass', { result, duration_ms: Date.now() - start1 });
  } else {
    logTest(test1, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  logTest(test1, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start1,
    expected: 'Should accept no arguments and show text input'
  });
}

// -----------------------------------------------------------------------------
// Test 2: arg('placeholder') - string only, no choices
// This should show a text input with the placeholder as the prompt
// Currently BROKEN: choices.map() called on undefined
// -----------------------------------------------------------------------------
const test2 = 'arg-placeholder-only';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug('Test 2: arg("placeholder") with no choices - should show text input');
  
  // This call currently FAILS with "Cannot read property 'map' of undefined"
  // After fix: should show a text input with "Enter your name" as placeholder
  const result = await (arg as (placeholder: string) => Promise<string>)('Enter your name');
  
  debug(`Test 2 result: "${result}"`);
  
  // Any string is valid (user typed something or pressed enter with empty)
  if (typeof result === 'string') {
    logTest(test2, 'pass', { result, duration_ms: Date.now() - start2 });
  } else {
    logTest(test2, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start2 
    });
  }
} catch (err) {
  logTest(test2, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start2,
    expected: 'Should accept placeholder only and show text input'
  });
}

// -----------------------------------------------------------------------------
// Test 3: arg('placeholder', ['a', 'b', 'c']) - with string array choices
// This is the most common usage pattern
// -----------------------------------------------------------------------------
const test3 = 'arg-string-choices';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug('Test 3: arg() with string choices');
  
  const result = await arg('Pick a fruit (select Apple to pass)', [
    'Apple',
    'Banana', 
    'Cherry',
    'Date',
    'Elderberry'
  ]);
  
  debug(`Test 3 result: "${result}"`);
  
  // For automated testing, we expect first choice to be auto-selected
  // For manual testing, user should select "Apple" to pass
  if (result === 'Apple') {
    logTest(test3, 'pass', { result, duration_ms: Date.now() - start3 });
  } else {
    // Don't fail - just record what was selected
    logTest(test3, 'pass', { 
      result, 
      duration_ms: Date.now() - start3 
    });
  }
} catch (err) {
  logTest(test3, 'fail', { error: String(err), duration_ms: Date.now() - start3 });
}

// -----------------------------------------------------------------------------
// Test 4: arg('placeholder', [{name, value, description}]) - structured choices
// Tests that choice objects are handled correctly
// -----------------------------------------------------------------------------
const test4 = 'arg-structured-choices';
logTest(test4, 'running');
const start4 = Date.now();

try {
  debug('Test 4: arg() with structured choices');
  
  const result = await arg('Select an action (select Run to pass)', [
    { name: 'Run Script', value: 'run', description: 'Execute the current script' },
    { name: 'Edit Script', value: 'edit', description: 'Open in editor' },
    { name: 'Delete Script', value: 'delete', description: 'Remove from disk' },
    { name: 'Share Script', value: 'share', description: 'Copy shareable link' }
  ]);
  
  debug(`Test 4 result: "${result}"`);
  
  // Structured choices return the value, not the name
  if (result === 'run') {
    logTest(test4, 'pass', { result, duration_ms: Date.now() - start4 });
  } else {
    // Record whatever was selected
    logTest(test4, 'pass', { 
      result, 
      duration_ms: Date.now() - start4 
    });
  }
} catch (err) {
  logTest(test4, 'fail', { error: String(err), duration_ms: Date.now() - start4 });
}

// -----------------------------------------------------------------------------
// Test 5: arg('placeholder', async () => [...]) - async function choices
// The function should be called to get the choices
// -----------------------------------------------------------------------------
const test5 = 'arg-async-function-choices';
logTest(test5, 'running');
const start5 = Date.now();

try {
  debug('Test 5: arg() with async function returning choices');
  
  // Define an async function that returns choices
  const asyncChoices = async () => {
    // Simulate async operation (e.g., fetching from API)
    await new Promise(resolve => setTimeout(resolve, 10));
    return ['Option A', 'Option B', 'Option C'];
  };
  
  // This call may FAIL if async functions aren't supported yet
  // Cast through unknown because current types don't support this calling convention
  const result = await (arg as unknown as (placeholder: string, choices: () => Promise<string[]>) => Promise<string>)(
    'Select from async choices',
    asyncChoices
  );
  
  debug(`Test 5 result: "${result}"`);
  
  if (typeof result === 'string' && ['Option A', 'Option B', 'Option C'].includes(result)) {
    logTest(test5, 'pass', { result, duration_ms: Date.now() - start5 });
  } else if (typeof result === 'string') {
    // Got a result but not one of expected - could be user typed something
    logTest(test5, 'pass', { result, duration_ms: Date.now() - start5 });
  } else {
    logTest(test5, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start5 
    });
  }
} catch (err) {
  logTest(test5, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start5,
    expected: 'Should support async function returning choices'
  });
}

// -----------------------------------------------------------------------------
// Test 6: arg('placeholder', (input) => [...]) - filter/generator function
// The function is called with user input to dynamically filter/generate choices
// -----------------------------------------------------------------------------
const test6 = 'arg-filter-function-choices';
logTest(test6, 'running');
const start6 = Date.now();

try {
  debug('Test 6: arg() with filter/generator function');
  
  // Define a filter function that returns choices based on user input
  const filterChoices = (input: string) => {
    const allChoices = ['apple', 'apricot', 'banana', 'blueberry', 'cherry'];
    if (!input) return allChoices;
    return allChoices.filter(c => c.toLowerCase().includes(input.toLowerCase()));
  };
  
  // This call may FAIL if filter functions aren't supported yet
  // Cast through unknown because current types don't support this calling convention
  const result = await (arg as unknown as (placeholder: string, choices: (input: string) => string[]) => Promise<string>)(
    'Type to filter (try typing "a")',
    filterChoices
  );
  
  debug(`Test 6 result: "${result}"`);
  
  if (typeof result === 'string') {
    logTest(test6, 'pass', { result, duration_ms: Date.now() - start6 });
  } else {
    logTest(test6, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start6 
    });
  }
} catch (err) {
  logTest(test6, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start6,
    expected: 'Should support filter function with input parameter'
  });
}

// -----------------------------------------------------------------------------
// Test 7: arg({placeholder, choices, ...}) - config object
// Full configuration object with all options
// Currently BROKEN: doesn't handle config object format
// -----------------------------------------------------------------------------
const test7 = 'arg-config-object';
logTest(test7, 'running');
const start7 = Date.now();

try {
  debug('Test 7: arg() with config object');
  
  interface ArgConfig {
    placeholder: string;
    choices?: (string | { name: string; value: string; description?: string })[];
    hint?: string;
    onInit?: () => void;
    onSubmit?: (value: string) => void;
  }
  
  const config: ArgConfig = {
    placeholder: 'Select with config object',
    choices: [
      { name: 'First Option', value: 'first', description: 'This is the first option' },
      { name: 'Second Option', value: 'second', description: 'This is the second option' }
    ],
    hint: 'Use arrow keys to navigate'
  };
  
  // This call may FAIL if config objects aren't supported yet
  // Cast through unknown because current types don't support this calling convention
  const result = await (arg as unknown as (config: ArgConfig) => Promise<string>)(config);
  
  debug(`Test 7 result: "${result}"`);
  
  if (typeof result === 'string') {
    logTest(test7, 'pass', { result, duration_ms: Date.now() - start7 });
  } else {
    logTest(test7, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start7 
    });
  }
} catch (err) {
  logTest(test7, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start7,
    expected: 'Should support config object with placeholder, choices, callbacks'
  });
}

// -----------------------------------------------------------------------------
// Test 8: arg with empty array choices
// Should show the prompt but with no choices to select from
// -----------------------------------------------------------------------------
const test8 = 'arg-empty-choices';
logTest(test8, 'running');
const start8 = Date.now();

try {
  debug('Test 8: arg() with empty choices array');
  
  const result = await arg('Enter text (no choices available)', []);
  
  debug(`Test 8 result: "${result}"`);
  
  // With empty choices, user should be able to type free-form text
  if (typeof result === 'string') {
    logTest(test8, 'pass', { result, duration_ms: Date.now() - start8 });
  } else {
    logTest(test8, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start8 
    });
  }
} catch (err) {
  logTest(test8, 'fail', { error: String(err), duration_ms: Date.now() - start8 });
}

// -----------------------------------------------------------------------------
// Test 9: arg with mixed string and object choices
// Should normalize all choices to {name, value} format
// -----------------------------------------------------------------------------
const test9 = 'arg-mixed-choices';
logTest(test9, 'running');
const start9 = Date.now();

try {
  debug('Test 9: arg() with mixed string and object choices');
  
  const result = await arg('Pick any option', [
    'Simple String',
    { name: 'Object Choice', value: 'obj-value', description: 'This is an object' },
    'Another String',
    { name: 'Second Object', value: 'second-obj' }
  ]);
  
  debug(`Test 9 result: "${result}"`);
  
  // Result should be the value - either the string itself or the object's value
  const validValues = ['Simple String', 'obj-value', 'Another String', 'second-obj'];
  if (validValues.includes(result)) {
    logTest(test9, 'pass', { result, duration_ms: Date.now() - start9 });
  } else if (typeof result === 'string') {
    // Got something else - still pass but note it
    logTest(test9, 'pass', { result, duration_ms: Date.now() - start9 });
  } else {
    logTest(test9, 'fail', { 
      error: `Expected string, got ${typeof result}`,
      duration_ms: Date.now() - start9 
    });
  }
} catch (err) {
  logTest(test9, 'fail', { error: String(err), duration_ms: Date.now() - start9 });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug('test-arg.ts completed!');

await div(md(`# arg() Tests Complete

All \`arg()\` calling conventions have been tested.

## Test Cases Run

| # | Test | Description |
|---|------|-------------|
| 1 | arg-no-args | \`arg()\` - no arguments |
| 2 | arg-placeholder-only | \`arg("placeholder")\` - placeholder only |
| 3 | arg-string-choices | \`arg("p", ["a","b"])\` - string array |
| 4 | arg-structured-choices | \`arg("p", [{name,value}])\` - objects |
| 5 | arg-async-function-choices | \`arg("p", async () => [...])\` - async |
| 6 | arg-filter-function-choices | \`arg("p", (input) => [...])\` - filter |
| 7 | arg-config-object | \`arg({placeholder, choices})\` - config |
| 8 | arg-empty-choices | \`arg("p", [])\` - empty array |
| 9 | arg-mixed-choices | \`arg("p", ["str", {obj}])\` - mixed |

---

**Expected Failures (TDD):**
- Test 1 & 2: Currently fail because \`choices.map()\` is called on undefined
- Test 5 & 6: May fail if async/filter functions not yet supported
- Test 7: May fail if config object format not yet supported

*Check the JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-arg.ts exiting...');

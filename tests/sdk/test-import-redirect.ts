// Name: SDK Test - @johnlindquist/kit import redirect
// Description: Tests that import '@johnlindquist/kit' works correctly

/**
 * SDK TEST: test-import-redirect.ts
 * 
 * Tests that the @johnlindquist/kit import redirect works correctly.
 * This verifies that the package.json "imports" field properly redirects
 * `import '@johnlindquist/kit'` to our local kit-sdk.ts implementation.
 * 
 * Expected behavior:
 * - import '@johnlindquist/kit' should work without errors
 * - After import, global functions (arg, div, md, etc.) should be available
 * - The SDK_VERSION export should be accessible
 * - All major API categories should be defined
 */

// THE KEY TEST: Using the @johnlindquist/kit import path
// This tests the package.json "imports" field redirect
import '@johnlindquist/kit';

// Also test that we can import specific exports
import { SDK_VERSION } from '@johnlindquist/kit';

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

debug('test-import-redirect.ts starting...');
debug(`Testing @johnlindquist/kit import redirect...`);

// -----------------------------------------------------------------------------
// Test 1: SDK_VERSION is accessible from named import
// -----------------------------------------------------------------------------
const test1 = 'import-sdk-version';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug(`SDK_VERSION = ${SDK_VERSION}`);
  
  if (typeof SDK_VERSION === 'string' && SDK_VERSION.length > 0) {
    logTest(test1, 'pass', { 
      result: SDK_VERSION, 
      duration_ms: Date.now() - start1 
    });
  } else {
    logTest(test1, 'fail', { 
      error: `Expected non-empty string, got ${typeof SDK_VERSION}: ${SDK_VERSION}`,
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  logTest(test1, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start1,
    expected: 'SDK_VERSION should be a string like "0.2.0"'
  });
}

// -----------------------------------------------------------------------------
// Test 2: arg() is available as a global function
// -----------------------------------------------------------------------------
const test2 = 'global-arg-available';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug(`typeof arg = ${typeof arg}`);
  
  if (typeof arg === 'function') {
    logTest(test2, 'pass', { 
      result: 'arg is a function', 
      duration_ms: Date.now() - start2 
    });
  } else {
    logTest(test2, 'fail', { 
      error: `Expected function, got ${typeof arg}`,
      duration_ms: Date.now() - start2 
    });
  }
} catch (err) {
  logTest(test2, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start2,
    expected: 'arg should be a global function'
  });
}

// -----------------------------------------------------------------------------
// Test 3: div() is available as a global function
// -----------------------------------------------------------------------------
const test3 = 'global-div-available';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug(`typeof div = ${typeof div}`);
  
  if (typeof div === 'function') {
    logTest(test3, 'pass', { 
      result: 'div is a function', 
      duration_ms: Date.now() - start3 
    });
  } else {
    logTest(test3, 'fail', { 
      error: `Expected function, got ${typeof div}`,
      duration_ms: Date.now() - start3 
    });
  }
} catch (err) {
  logTest(test3, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start3,
    expected: 'div should be a global function'
  });
}

// -----------------------------------------------------------------------------
// Test 4: md() is available as a global function
// -----------------------------------------------------------------------------
const test4 = 'global-md-available';
logTest(test4, 'running');
const start4 = Date.now();

try {
  debug(`typeof md = ${typeof md}`);
  
  if (typeof md === 'function') {
    logTest(test4, 'pass', { 
      result: 'md is a function', 
      duration_ms: Date.now() - start4 
    });
  } else {
    logTest(test4, 'fail', { 
      error: `Expected function, got ${typeof md}`,
      duration_ms: Date.now() - start4 
    });
  }
} catch (err) {
  logTest(test4, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start4,
    expected: 'md should be a global function'
  });
}

// -----------------------------------------------------------------------------
// Test 5: md() works correctly (synchronous, returns HTML)
// -----------------------------------------------------------------------------
const test5 = 'md-function-works';
logTest(test5, 'running');
const start5 = Date.now();

try {
  const result = md('# Hello World');
  debug(`md('# Hello World') = ${result}`);
  
  if (typeof result === 'string' && result.includes('<h1>')) {
    logTest(test5, 'pass', { 
      result, 
      duration_ms: Date.now() - start5 
    });
  } else {
    logTest(test5, 'fail', { 
      error: `Expected HTML with <h1>, got: ${result}`,
      duration_ms: Date.now() - start5 
    });
  }
} catch (err) {
  logTest(test5, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start5,
    expected: 'md() should convert markdown to HTML'
  });
}

// -----------------------------------------------------------------------------
// Test 6: editor() is available as a global function
// -----------------------------------------------------------------------------
const test6 = 'global-editor-available';
logTest(test6, 'running');
const start6 = Date.now();

try {
  debug(`typeof editor = ${typeof editor}`);
  
  if (typeof editor === 'function') {
    logTest(test6, 'pass', { 
      result: 'editor is a function', 
      duration_ms: Date.now() - start6 
    });
  } else {
    logTest(test6, 'fail', { 
      error: `Expected function, got ${typeof editor}`,
      duration_ms: Date.now() - start6 
    });
  }
} catch (err) {
  logTest(test6, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start6,
    expected: 'editor should be a global function'
  });
}

// -----------------------------------------------------------------------------
// Test 7: select() is available as a global function
// -----------------------------------------------------------------------------
const test7 = 'global-select-available';
logTest(test7, 'running');
const start7 = Date.now();

try {
  debug(`typeof select = ${typeof select}`);
  
  if (typeof select === 'function') {
    logTest(test7, 'pass', { 
      result: 'select is a function', 
      duration_ms: Date.now() - start7 
    });
  } else {
    logTest(test7, 'fail', { 
      error: `Expected function, got ${typeof select}`,
      duration_ms: Date.now() - start7 
    });
  }
} catch (err) {
  logTest(test7, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start7,
    expected: 'select should be a global function'
  });
}

// -----------------------------------------------------------------------------
// Test 8: fields() is available as a global function
// -----------------------------------------------------------------------------
const test8 = 'global-fields-available';
logTest(test8, 'running');
const start8 = Date.now();

try {
  debug(`typeof fields = ${typeof fields}`);
  
  if (typeof fields === 'function') {
    logTest(test8, 'pass', { 
      result: 'fields is a function', 
      duration_ms: Date.now() - start8 
    });
  } else {
    logTest(test8, 'fail', { 
      error: `Expected function, got ${typeof fields}`,
      duration_ms: Date.now() - start8 
    });
  }
} catch (err) {
  logTest(test8, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start8,
    expected: 'fields should be a global function'
  });
}

// -----------------------------------------------------------------------------
// Test 9: clipboard object is available
// -----------------------------------------------------------------------------
const test9 = 'global-clipboard-available';
logTest(test9, 'running');
const start9 = Date.now();

try {
  debug(`typeof clipboard = ${typeof clipboard}`);
  
  if (typeof clipboard === 'object' && 
      typeof clipboard.readText === 'function' &&
      typeof clipboard.writeText === 'function') {
    logTest(test9, 'pass', { 
      result: 'clipboard object with readText/writeText', 
      duration_ms: Date.now() - start9 
    });
  } else {
    logTest(test9, 'fail', { 
      error: `Expected clipboard object with methods, got ${typeof clipboard}`,
      duration_ms: Date.now() - start9 
    });
  }
} catch (err) {
  logTest(test9, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start9,
    expected: 'clipboard should be an object with readText/writeText methods'
  });
}

// -----------------------------------------------------------------------------
// Test 10: path utilities are available
// -----------------------------------------------------------------------------
const test10 = 'global-path-utils-available';
logTest(test10, 'running');
const start10 = Date.now();

try {
  const homeAvailable = typeof home === 'function';
  const skPathAvailable = typeof skPath === 'function';
  const kitPathAvailable = typeof kitPath === 'function';
  
  debug(`home: ${homeAvailable}, skPath: ${skPathAvailable}, kitPath: ${kitPathAvailable}`);
  
  if (homeAvailable && skPathAvailable && kitPathAvailable) {
    logTest(test10, 'pass', { 
      result: 'home, skPath, kitPath all available', 
      duration_ms: Date.now() - start10 
    });
  } else {
    logTest(test10, 'fail', { 
      error: `Missing path utilities: home=${homeAvailable}, skPath=${skPathAvailable}, kitPath=${kitPathAvailable}`,
      duration_ms: Date.now() - start10 
    });
  }
} catch (err) {
  logTest(test10, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start10,
    expected: 'home, skPath, kitPath should all be functions'
  });
}

// -----------------------------------------------------------------------------
// Test 11: home() returns a valid path
// -----------------------------------------------------------------------------
const test11 = 'home-returns-path';
logTest(test11, 'running');
const start11 = Date.now();

try {
  const homePath = home();
  debug(`home() = ${homePath}`);
  
  if (typeof homePath === 'string' && homePath.startsWith('/')) {
    logTest(test11, 'pass', { 
      result: homePath, 
      duration_ms: Date.now() - start11 
    });
  } else {
    logTest(test11, 'fail', { 
      error: `Expected absolute path, got: ${homePath}`,
      duration_ms: Date.now() - start11 
    });
  }
} catch (err) {
  logTest(test11, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start11,
    expected: 'home() should return absolute path to home directory'
  });
}

// -----------------------------------------------------------------------------
// Test 12: System APIs are available (beep, say, notify)
// -----------------------------------------------------------------------------
const test12 = 'global-system-apis-available';
logTest(test12, 'running');
const start12 = Date.now();

try {
  const beepAvailable = typeof beep === 'function';
  const sayAvailable = typeof say === 'function';
  const notifyAvailable = typeof notify === 'function';
  
  debug(`beep: ${beepAvailable}, say: ${sayAvailable}, notify: ${notifyAvailable}`);
  
  if (beepAvailable && sayAvailable && notifyAvailable) {
    logTest(test12, 'pass', { 
      result: 'beep, say, notify all available', 
      duration_ms: Date.now() - start12 
    });
  } else {
    logTest(test12, 'fail', { 
      error: `Missing system APIs: beep=${beepAvailable}, say=${sayAvailable}, notify=${notifyAvailable}`,
      duration_ms: Date.now() - start12 
    });
  }
} catch (err) {
  logTest(test12, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start12,
    expected: 'beep, say, notify should all be functions'
  });
}

// -----------------------------------------------------------------------------
// Test 13: TIER 4 APIs available (chat, widget, term)
// -----------------------------------------------------------------------------
const test13 = 'global-tier4-apis-available';
logTest(test13, 'running');
const start13 = Date.now();

try {
  const chatAvailable = typeof chat === 'function';
  const widgetAvailable = typeof widget === 'function';
  const termAvailable = typeof term === 'function';
  
  debug(`chat: ${chatAvailable}, widget: ${widgetAvailable}, term: ${termAvailable}`);
  
  if (chatAvailable && widgetAvailable && termAvailable) {
    logTest(test13, 'pass', { 
      result: 'chat, widget, term all available', 
      duration_ms: Date.now() - start13 
    });
  } else {
    logTest(test13, 'fail', { 
      error: `Missing TIER 4 APIs: chat=${chatAvailable}, widget=${widgetAvailable}, term=${termAvailable}`,
      duration_ms: Date.now() - start13 
    });
  }
} catch (err) {
  logTest(test13, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start13,
    expected: 'chat, widget, term should all be functions'
  });
}

// -----------------------------------------------------------------------------
// Test 14: TIER 5 utility APIs available (wait, uuid)
// Note: exec was removed from SDK - use Bun.$ or Bun.spawn() instead
// -----------------------------------------------------------------------------
const test14 = 'global-tier5-utils-available';
logTest(test14, 'running');
const start14 = Date.now();

try {
  const waitAvailable = typeof wait === 'function';
  const uuidAvailable = typeof uuid === 'function';
  
  debug(`wait: ${waitAvailable}, uuid: ${uuidAvailable}`);
  
  if (waitAvailable && uuidAvailable) {
    logTest(test14, 'pass', { 
      result: 'wait, uuid all available', 
      duration_ms: Date.now() - start14 
    });
  } else {
    logTest(test14, 'fail', { 
      error: `Missing TIER 5 APIs: wait=${waitAvailable}, uuid=${uuidAvailable}`,
      duration_ms: Date.now() - start14 
    });
  }
} catch (err) {
  logTest(test14, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start14,
    expected: 'wait, uuid should all be functions'
  });
}

// -----------------------------------------------------------------------------
// Test 15: uuid() generates valid UUIDs
// -----------------------------------------------------------------------------
const test15 = 'uuid-generates-valid';
logTest(test15, 'running');
const start15 = Date.now();

try {
  const generatedUuid = uuid();
  debug(`uuid() = ${generatedUuid}`);
  
  // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  
  if (uuidRegex.test(generatedUuid)) {
    logTest(test15, 'pass', { 
      result: generatedUuid, 
      duration_ms: Date.now() - start15 
    });
  } else {
    logTest(test15, 'fail', { 
      error: `UUID doesn't match v4 format: ${generatedUuid}`,
      duration_ms: Date.now() - start15 
    });
  }
} catch (err) {
  logTest(test15, 'fail', { 
    error: String(err), 
    duration_ms: Date.now() - start15,
    expected: 'uuid() should generate valid v4 UUIDs'
  });
}

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
debug('test-import-redirect.ts completed!');
debug('All tests verify that @johnlindquist/kit import redirect works correctly.');

// Display summary using div/md
await div(md(`# @johnlindquist/kit Import Redirect Tests Complete

The import redirect from \`@johnlindquist/kit\` has been tested.

## What Was Verified

| Category | Functions Tested |
|----------|------------------|
| **Named Exports** | SDK_VERSION |
| **Core Prompts** | arg, div, md, editor, select, fields |
| **System APIs** | clipboard, beep, say, notify |
| **Path Utils** | home, skPath, kitPath |
| **TIER 4 APIs** | chat, widget, term |
| **TIER 5 Utils** | wait, uuid |

## Import Syntax Tested

\`\`\`typescript
// Side-effect import (registers globals)
import '@johnlindquist/kit';

// Named export import
import { SDK_VERSION } from '@johnlindquist/kit';
\`\`\`

---

**SDK Version:** ${SDK_VERSION}

*Check the JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-import-redirect.ts exiting...');

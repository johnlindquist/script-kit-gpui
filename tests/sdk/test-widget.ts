// Name: SDK Test - Widget and Media APIs
// Description: Tests widget, term, webcam, mic, eyeDropper, and find functions

/**
 * SDK TEST: test-widget.ts
 * 
 * Tests the widget(), term(), and media prompt functions.
 * 
 * Test cases:
 * 1. widget-basic: Unsupported widget rejects before dispatch
 * 2. widget-options: Widget options do not bypass the support boundary
 * 3. widget-events: Unsupported widgets never return fake event controllers
 * 4. term-basic: term() function exists
 * 5. media-apis: Media API functions exist
 * 6. find-api: find() rejects with typed unsupported error before sending
 * 
 * Expected behavior:
 * - widget() rejects explicitly because GPUI has no widget surface
 * - term() opens terminal
 * - Media APIs (webcam, mic, eyeDropper) are available but unsupported
 * - find() is an explicit unsupported GPUI boundary; use fileSearch() for non-interactive file results
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
  actual?: string;
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

async function expectUnsupportedWidget(call: () => Promise<unknown>) {
  let failure: any;
  const dispatched: string[] = [];
  const originalWrite = process.stdout.write;
  process.stdout.write = ((chunk: unknown, ..._args: unknown[]) => {
    dispatched.push(String(chunk));
    return true;
  }) as typeof process.stdout.write;

  try {
    await call();
  } catch (error) {
    failure = error;
  } finally {
    process.stdout.write = originalWrite;
  }

  if (
    failure?.name !== 'UnsupportedSdkFeatureError' ||
    failure?.code !== 'ERR_UNSUPPORTED_SDK_FEATURE' ||
    failure?.supported !== false ||
    failure?.feature !== 'widget' ||
    !Array.isArray(failure?.alternatives) ||
    !failure.alternatives.some((alternative: string) => alternative.includes('div('))
  ) {
    throw new Error(`Expected actionable widget compatibility failure: ${String(failure)}`);
  }

  if (dispatched.length > 0) {
    throw new Error(`Unsupported widget emitted protocol traffic: ${dispatched.join('')}`);
  }

  return failure;
}

// =============================================================================
// Tests
// =============================================================================

debug('test-widget.ts starting...');
debug(`SDK globals: widget=${typeof widget}, term=${typeof term}`);

// -----------------------------------------------------------------------------
// Test 1: Unsupported widgets reject before dispatch.
// -----------------------------------------------------------------------------
const test1 = 'widget-basic';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug('Test 1: Widget rejects before a fake surface or controller is created');
  const failure = await expectUnsupportedWidget(() =>
    widget('<div><h2>Hello Widget!</h2></div>'),
  );
  logTest(test1, 'pass', {
    result: { code: failure.code, alternatives: failure.alternatives },
    duration_ms: Date.now() - start1,
  });
} catch (err) {
  logTest(test1, 'fail', { error: String(err), duration_ms: Date.now() - start1 });
}

// -----------------------------------------------------------------------------
// Test 2: Widget options do not bypass the unsupported boundary.
// -----------------------------------------------------------------------------
const test2 = 'widget-options';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug('Test 2: Widget options cannot manufacture an unsupported native surface');
  const failure = await expectUnsupportedWidget(() =>
    widget('<div>Positioned Widget</div>', {
      x: 100,
      y: 100,
      width: 300,
      height: 200,
      alwaysOnTop: true,
    }),
  );
  logTest(test2, 'pass', {
    result: { code: failure.code, feature: failure.feature },
    duration_ms: Date.now() - start2,
  });
} catch (err) {
  logTest(test2, 'fail', { error: String(err), duration_ms: Date.now() - start2 });
}

// -----------------------------------------------------------------------------
// Test 3: Unsupported widgets never return a pretend event controller.
// -----------------------------------------------------------------------------
const test3 = 'widget-events';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug('Test 3: Unsupported widget does not return a fake controller');
  await expectUnsupportedWidget(() => widget('<div>Event Test</div>'));
  logTest(test3, 'pass', {
    result: { controllerReturned: false },
    duration_ms: Date.now() - start3,
  });
} catch (err) {
  logTest(test3, 'fail', { error: String(err), duration_ms: Date.now() - start3 });
}

// -----------------------------------------------------------------------------
// Test 4: term() function exists
// -----------------------------------------------------------------------------
const test4 = 'term-exists';
logTest(test4, 'running');
const start4 = Date.now();

try {
  debug('Test 4: term() function exists');
  
  const hasTerm = typeof term === 'function';
  
  debug(`term function exists: ${hasTerm}`);
  
  if (hasTerm) {
    logTest(test4, 'pass', { result: 'term() function available', duration_ms: Date.now() - start4 });
  } else {
    logTest(test4, 'fail', { 
      error: 'term() function not found',
      duration_ms: Date.now() - start4 
    });
  }
} catch (err) {
  logTest(test4, 'fail', { error: String(err), duration_ms: Date.now() - start4 });
}

// -----------------------------------------------------------------------------
// Test 5: Media API functions exist
// -----------------------------------------------------------------------------
const test5 = 'media-apis';
logTest(test5, 'running');
const start5 = Date.now();

try {
  debug('Test 5: Media API functions exist');
  
  const hasWebcam = typeof webcam === 'function';
  const hasMic = typeof mic === 'function';
  const hasEyeDropper = typeof eyeDropper === 'function';
  
  debug(`webcam function exists: ${hasWebcam}`);
  debug(`mic function exists: ${hasMic}`);
  debug(`eyeDropper function exists: ${hasEyeDropper}`);
  
  const checks = [hasWebcam, hasMic, hasEyeDropper];
  
  if (checks.every(Boolean)) {
    logTest(test5, 'pass', { result: 'All media APIs available', duration_ms: Date.now() - start5 });
  } else {
    logTest(test5, 'fail', { 
      error: 'Some media APIs missing',
      duration_ms: Date.now() - start5 
    });
  }
} catch (err) {
  logTest(test5, 'fail', { error: String(err), duration_ms: Date.now() - start5 });
}

// -----------------------------------------------------------------------------
// Test 6: find() rejects with typed unsupported error
// -----------------------------------------------------------------------------
const test6 = 'find-api';
logTest(test6, 'running');
const start6 = Date.now();

try {
  debug('Test 6: find() typed unsupported boundary');
  
  const hasFind = typeof find === 'function';
  
  if (!hasFind) {
    logTest(test6, 'fail', { 
      error: 'find() function not found',
      duration_ms: Date.now() - start6 
    });
  } else {
    let error: any = null;
    try {
      await find('Find a file', { onlyin: '/tmp' });
    } catch (err) {
      error = err;
    }

    const checks = [
      error?.name === 'UnsupportedSdkFeatureError',
      error?.code === 'ERR_UNSUPPORTED_SDK_FEATURE',
      error?.supported === false,
      error?.feature === 'find',
      Array.isArray(error?.alternatives),
      error?.alternatives?.some((alt: string) => alt.includes('fileSearch')),
    ];

    if (checks.every(Boolean)) {
      logTest(test6, 'pass', {
        result: {
          code: error.code,
          supported: error.supported,
          feature: error.feature,
          alternatives: error.alternatives,
        },
        duration_ms: Date.now() - start6,
      });
    } else {
      logTest(test6, 'fail', {
        error: `Unexpected find() error shape: ${JSON.stringify(error)}`,
        duration_ms: Date.now() - start6,
      });
    }
  }
} catch (err) {
  logTest(test6, 'fail', { error: String(err), duration_ms: Date.now() - start6 });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug('test-widget.ts completed!');

await div(md(`# Widget and Media Tests Complete

All widget and media API tests have been executed.

## Test Cases Run
1. **widget-basic**: Unsupported widget fails with an actionable error
2. **widget-options**: Options do not bypass the unsupported boundary
3. **widget-events**: Unsupported widgets never return fake controllers
4. **term-exists**: term() function availability
5. **media-apis**: webcam, mic, eyeDropper availability
6. **find-api**: find() function availability

---

*Check the JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-widget.ts exiting...');

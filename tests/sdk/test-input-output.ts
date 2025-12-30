// Name: SDK Test - input() and output()
// Description: TDD tests for typed input/output functions for MCP integration

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

// Helper to reset state between tests
function resetState() {
  // Use the internal reset function that has access to module-scoped variables
  (globalThis as any)._resetScriptIO();
}

async function runTests() {
  console.error('[TEST] Starting input()/output() SDK tests');

  // ============================================
  // Test 1: input() returns empty object by default
  // ============================================
  resetState();
  {
    const testName = 'input-returns-empty-by-default';
    logTest(testName, 'running');
    const start = Date.now();
    
    try {
      const result = await input();
      
      if (typeof result === 'object' && Object.keys(result).length === 0) {
        logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected empty object, got: ${JSON.stringify(result)}`,
          result,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    }
  }

  // ============================================
  // Test 2: input() is typed generic function
  // ============================================
  resetState();
  {
    const testName = 'input-is-typed-generic';
    logTest(testName, 'running');
    const start = Date.now();
    
    try {
      // TypeScript should allow this - testing runtime behavior
      const result = await input<{ name: string; age: number }>();
      
      // Should return empty object that can be typed
      if (typeof result === 'object') {
        logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected object, got: ${typeof result}`,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    }
  }

  // ============================================
  // Test 3: _setScriptInput sets data for input()
  // ============================================
  resetState();
  {
    const testName = 'setScriptInput-provides-data';
    logTest(testName, 'running');
    const start = Date.now();
    
    try {
      // Set input data via internal function
      const testData = { greeting: 'Hello', recipient: 'World' };
      (globalThis as any)._setScriptInput(testData);
      
      const result = await input<typeof testData>();
      
      if (result.greeting === 'Hello' && result.recipient === 'World') {
        logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected testData, got: ${JSON.stringify(result)}`,
          result,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    }
  }

  // ============================================
  // Test 4: output() is a function
  // ============================================
  resetState();
  {
    const testName = 'output-is-function';
    logTest(testName, 'running');
    const start = Date.now();
    
    try {
      if (typeof output === 'function') {
        logTest(testName, 'pass', { duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected function, got: ${typeof output}`,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    }
  }

  // ============================================
  // Test 5: output() accumulates data
  // ============================================
  resetState();
  {
    const testName = 'output-accumulates-data';
    logTest(testName, 'running');
    const start = Date.now();
    
    // Suppress send() output for this test
    const originalSend = (globalThis as any).send;
    (globalThis as any).send = () => {};
    
    try {
      // First output call
      output({ message: 'Hello' });
      
      // Second output call - should merge
      output({ timestamp: '2024-01-01' });
      
      // Get accumulated output
      const result = (globalThis as any)._getScriptOutput();
      
      if (result.message === 'Hello' && result.timestamp === '2024-01-01') {
        logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected merged object, got: ${JSON.stringify(result)}`,
          result,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    } finally {
      (globalThis as any).send = originalSend;
    }
  }

  // ============================================
  // Test 6: output() sends scriptOutput message to stdout
  // ============================================
  resetState();
  {
    const testName = 'output-sends-scriptOutput-message';
    logTest(testName, 'running');
    const start = Date.now();
    
    try {
      // Capture stdout by temporarily replacing process.stdout.write
      const writtenData: string[] = [];
      const originalWrite = process.stdout.write.bind(process.stdout);
      (process.stdout as any).write = (data: string) => {
        writtenData.push(data);
        return true;
      };
      
      // Call output
      output({ testField: 'testValue' });
      
      // Restore stdout.write
      (process.stdout as any).write = originalWrite;
      
      // Parse the captured JSONL output
      const outputLine = writtenData.find(line => line.includes('scriptOutput'));
      let parsed: any = null;
      if (outputLine) {
        try {
          parsed = JSON.parse(outputLine.trim());
        } catch {
          // ignore parse errors
        }
      }
      
      if (parsed && parsed.type === 'scriptOutput' && parsed.data?.testField === 'testValue') {
        logTest(testName, 'pass', { result: parsed, duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected scriptOutput message with testField, got: ${JSON.stringify(writtenData)}`,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    }
  }

  // ============================================
  // Test 7: _getScriptOutput returns accumulated data
  // ============================================
  resetState();
  {
    const testName = 'getScriptOutput-returns-accumulated';
    logTest(testName, 'running');
    const start = Date.now();
    
    // Suppress send() output for this test
    const originalSend = (globalThis as any).send;
    (globalThis as any).send = () => {};
    
    try {
      // Accumulate data via output() calls
      output({ a: 1 });
      output({ b: 2 });
      
      const result = (globalThis as any)._getScriptOutput();
      
      if (result.a === 1 && result.b === 2) {
        logTest(testName, 'pass', { result, duration_ms: Date.now() - start });
      } else {
        logTest(testName, 'fail', { 
          error: `Expected {a:1,b:2}, got: ${JSON.stringify(result)}`,
          duration_ms: Date.now() - start 
        });
      }
    } catch (err) {
      logTest(testName, 'fail', { error: String(err), duration_ms: Date.now() - start });
    } finally {
      (globalThis as any).send = originalSend;
    }
  }

  console.error('[TEST] All input()/output() tests completed');
}

runTests().then(() => {
  process.exit(0);
}).catch((err) => {
  console.error('[TEST] Fatal error:', err);
  process.exit(1);
});

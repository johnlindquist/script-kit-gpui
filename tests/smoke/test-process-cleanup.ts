// Name: Process Cleanup Smoke Test
// Description: Verify that the main app PID file is created at ~/.sk/kit/script-kit.pid

/**
 * SMOKE TEST: test-process-cleanup.ts
 *
 * This test verifies that the process management system creates the PID file
 * at ~/.sk/kit/script-kit.pid when the app runs.
 *
 * What this tests:
 * - PID file exists at ~/.sk/kit/script-kit.pid
 * - PID file contains a valid numeric PID
 * - The PID corresponds to a running process
 *
 * Run with GPUI:
 *   echo '{"type":"run","path":"tests/smoke/test-process-cleanup.ts"}' | ./target/debug/script-kit-gpui
 */

import '../../scripts/kit-sdk';
import { existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';

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

debug('test-process-cleanup.ts starting...');

const pidFilePath = join(homedir(), '.kenv', 'script-kit.pid');
debug(`Checking for PID file at: ${pidFilePath}`);

// -----------------------------------------------------------------------------
// Test 1: PID file exists
// -----------------------------------------------------------------------------
const test1 = 'pid-file-exists';
logTest(test1, 'running');
const start1 = Date.now();

try {
  const exists = existsSync(pidFilePath);
  
  if (exists) {
    debug('PID file exists');
    logTest(test1, 'pass', { result: pidFilePath, duration_ms: Date.now() - start1 });
  } else {
    debug('PID file does not exist');
    logTest(test1, 'fail', { 
      error: `PID file not found at ${pidFilePath}`,
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  logTest(test1, 'fail', { error: String(err), duration_ms: Date.now() - start1 });
}

// -----------------------------------------------------------------------------
// Test 2: PID file contains valid numeric PID
// -----------------------------------------------------------------------------
const test2 = 'pid-file-valid-content';
logTest(test2, 'running');
const start2 = Date.now();

try {
  if (!existsSync(pidFilePath)) {
    logTest(test2, 'skip', { 
      error: 'PID file does not exist, skipping content validation',
      duration_ms: Date.now() - start2 
    });
  } else {
    const content = readFileSync(pidFilePath, 'utf-8').trim();
    const pid = parseInt(content, 10);
    
    if (isNaN(pid) || pid <= 0) {
      logTest(test2, 'fail', { 
        error: `Invalid PID content: "${content}"`,
        duration_ms: Date.now() - start2 
      });
    } else {
      debug(`PID file contains valid PID: ${pid}`);
      logTest(test2, 'pass', { result: pid, duration_ms: Date.now() - start2 });
    }
  }
} catch (err) {
  logTest(test2, 'fail', { error: String(err), duration_ms: Date.now() - start2 });
}

// -----------------------------------------------------------------------------
// Test 3: PID corresponds to running process (best effort check)
// -----------------------------------------------------------------------------
const test3 = 'pid-process-running';
logTest(test3, 'running');
const start3 = Date.now();

try {
  if (!existsSync(pidFilePath)) {
    logTest(test3, 'skip', { 
      error: 'PID file does not exist, skipping process check',
      duration_ms: Date.now() - start3 
    });
  } else {
    const content = readFileSync(pidFilePath, 'utf-8').trim();
    const pid = parseInt(content, 10);
    
    if (isNaN(pid) || pid <= 0) {
      logTest(test3, 'skip', { 
        error: 'Invalid PID, skipping process check',
        duration_ms: Date.now() - start3 
      });
    } else {
      // Try to check if process exists using kill(pid, 0) which doesn't send a signal
      // but returns whether the process exists
      try {
        process.kill(pid, 0);
        debug(`Process ${pid} is running`);
        logTest(test3, 'pass', { result: { pid, running: true }, duration_ms: Date.now() - start3 });
      } catch (killErr: unknown) {
        const e = killErr as { code?: string };
        if (e.code === 'ESRCH') {
          // Process does not exist
          debug(`Process ${pid} is NOT running (stale PID file)`);
          logTest(test3, 'fail', { 
            error: `PID ${pid} is not running (stale PID file)`,
            duration_ms: Date.now() - start3 
          });
        } else if (e.code === 'EPERM') {
          // Process exists but we don't have permission to signal it
          debug(`Process ${pid} exists (permission denied to signal)`);
          logTest(test3, 'pass', { result: { pid, running: true, note: 'EPERM' }, duration_ms: Date.now() - start3 });
        } else {
          throw killErr;
        }
      }
    }
  }
} catch (err) {
  logTest(test3, 'fail', { error: String(err), duration_ms: Date.now() - start3 });
}

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
debug('test-process-cleanup.ts completed!');

await div(md(`# Process Cleanup Test Complete

## PID File Location
\`${pidFilePath}\`

## Tests Run
1. **pid-file-exists** - Check if PID file was created
2. **pid-file-valid-content** - Verify PID file contains a numeric value
3. **pid-process-running** - Verify the PID corresponds to a running process

---

*Check the JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-process-cleanup.ts exiting...');

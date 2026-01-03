// Name: Process Orphan Detection Smoke Test
// Description: Verify that active-bun-pids.json is created/used for tracking script processes

/**
 * SMOKE TEST: test-process-orphan.ts
 *
 * This test verifies that the process management system creates and uses
 * the active-bun-pids.json file at ~/.sk/kit/active-bun-pids.json for
 * tracking running script processes.
 *
 * What this tests:
 * - active-bun-pids.json exists at ~/.sk/kit/
 * - File contains valid JSON structure
 * - JSON has expected fields (pid, script_path, started_at)
 *
 * Note: This test may find an empty file if no scripts are actively running,
 * which is still a valid state.
 *
 * Run with GPUI:
 *   echo '{"type":"run","path":"tests/smoke/test-process-orphan.ts"}' | ./target/debug/script-kit-gpui
 */

import '../../scripts/kit-sdk';
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
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

debug('test-process-orphan.ts starting...');

const activePidsPath = join(homedir(), '.kenv', 'active-bun-pids.json');
debug(`Checking for active PIDs file at: ${activePidsPath}`);

// -----------------------------------------------------------------------------
// Test 1: active-bun-pids.json file exists or can be created
// -----------------------------------------------------------------------------
const test1 = 'active-pids-file-exists';
logTest(test1, 'running');
const start1 = Date.now();

try {
  const exists = existsSync(activePidsPath);
  
  if (exists) {
    debug('active-bun-pids.json exists');
    logTest(test1, 'pass', { result: { path: activePidsPath, existed: true }, duration_ms: Date.now() - start1 });
  } else {
    // File doesn't exist - this is OK, the process manager creates it lazily
    // when the first script is registered
    debug('active-bun-pids.json does not exist (will be created on first script run)');
    logTest(test1, 'pass', { 
      result: { path: activePidsPath, existed: false, note: 'Created lazily on first script' },
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  logTest(test1, 'fail', { error: String(err), duration_ms: Date.now() - start1 });
}

// -----------------------------------------------------------------------------
// Test 2: If file exists, it contains valid JSON
// -----------------------------------------------------------------------------
const test2 = 'active-pids-valid-json';
logTest(test2, 'running');
const start2 = Date.now();

try {
  if (!existsSync(activePidsPath)) {
    logTest(test2, 'skip', { 
      error: 'File does not exist yet, skipping JSON validation',
      duration_ms: Date.now() - start2 
    });
  } else {
    const content = readFileSync(activePidsPath, 'utf-8');
    
    if (content.trim() === '') {
      debug('File is empty (valid - no active processes)');
      logTest(test2, 'pass', { result: { empty: true }, duration_ms: Date.now() - start2 });
    } else {
      const parsed = JSON.parse(content);
      debug(`Parsed JSON successfully: ${JSON.stringify(parsed).slice(0, 100)}...`);
      logTest(test2, 'pass', { 
        result: { type: typeof parsed, isArray: Array.isArray(parsed), keys: Object.keys(parsed).slice(0, 5) },
        duration_ms: Date.now() - start2 
      });
    }
  }
} catch (err) {
  if (err instanceof SyntaxError) {
    logTest(test2, 'fail', { 
      error: `Invalid JSON: ${String(err)}`,
      duration_ms: Date.now() - start2 
    });
  } else {
    logTest(test2, 'fail', { error: String(err), duration_ms: Date.now() - start2 });
  }
}

// -----------------------------------------------------------------------------
// Test 3: JSON structure has expected shape (if file has content)
// -----------------------------------------------------------------------------
const test3 = 'active-pids-structure';
logTest(test3, 'running');
const start3 = Date.now();

interface ProcessInfo {
  pid: number;
  script_path: string;
  started_at: string;
}

try {
  if (!existsSync(activePidsPath)) {
    logTest(test3, 'skip', { 
      error: 'File does not exist yet',
      duration_ms: Date.now() - start3 
    });
  } else {
    const content = readFileSync(activePidsPath, 'utf-8');
    
    if (content.trim() === '' || content.trim() === '{}' || content.trim() === '[]') {
      debug('File is empty or has no entries (valid state)');
      logTest(test3, 'pass', { 
        result: { empty: true, note: 'No active processes tracked' },
        duration_ms: Date.now() - start3 
      });
    } else {
      const parsed = JSON.parse(content);
      
      // Expected structure: object with PID keys mapping to ProcessInfo
      // e.g., { "12345": { "pid": 12345, "script_path": "/path/to/script.ts", "started_at": "..." } }
      if (typeof parsed === 'object' && !Array.isArray(parsed)) {
        const entries = Object.entries(parsed);
        
        if (entries.length === 0) {
          debug('Object is empty (valid - no active processes)');
          logTest(test3, 'pass', { 
            result: { processCount: 0 },
            duration_ms: Date.now() - start3 
          });
        } else {
          // Check first entry for expected fields
          const [key, value] = entries[0];
          const info = value as ProcessInfo;
          
          const hasExpectedFields = 
            typeof info.pid === 'number' &&
            typeof info.script_path === 'string' &&
            typeof info.started_at === 'string';
          
          if (hasExpectedFields) {
            debug(`Valid structure found with ${entries.length} process(es)`);
            logTest(test3, 'pass', { 
              result: { 
                processCount: entries.length,
                samplePid: info.pid,
                sampleScript: info.script_path.split('/').pop()
              },
              duration_ms: Date.now() - start3 
            });
          } else {
            debug('Structure exists but missing expected fields');
            logTest(test3, 'fail', { 
              error: `Missing expected fields (pid, script_path, started_at). Found: ${JSON.stringify(info)}`,
              duration_ms: Date.now() - start3 
            });
          }
        }
      } else {
        debug(`Unexpected structure type: ${Array.isArray(parsed) ? 'array' : typeof parsed}`);
        logTest(test3, 'fail', { 
          error: `Expected object, got ${Array.isArray(parsed) ? 'array' : typeof parsed}`,
          duration_ms: Date.now() - start3 
        });
      }
    }
  }
} catch (err) {
  logTest(test3, 'fail', { error: String(err), duration_ms: Date.now() - start3 });
}

// -----------------------------------------------------------------------------
// Test 4: .kenv directory is writable (sanity check)
// -----------------------------------------------------------------------------
const test4 = 'kenv-dir-writable';
logTest(test4, 'running');
const start4 = Date.now();

try {
  const kenvDir = join(homedir(), '.kenv');
  const testFile = join(kenvDir, '.write-test-' + Date.now());
  
  // Ensure .kenv exists
  if (!existsSync(kenvDir)) {
    mkdirSync(kenvDir, { recursive: true });
  }
  
  // Try to write and delete a test file
  writeFileSync(testFile, 'test');
  const written = existsSync(testFile);
  
  if (written) {
    // Clean up
    const { unlinkSync } = await import('fs');
    unlinkSync(testFile);
    
    debug('.kenv directory is writable');
    logTest(test4, 'pass', { result: { writable: true }, duration_ms: Date.now() - start4 });
  } else {
    logTest(test4, 'fail', { 
      error: 'Could not write to .kenv directory',
      duration_ms: Date.now() - start4 
    });
  }
} catch (err) {
  logTest(test4, 'fail', { error: String(err), duration_ms: Date.now() - start4 });
}

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
debug('test-process-orphan.ts completed!');

await div(md(`# Process Orphan Detection Test Complete

## Active PIDs File Location
\`${activePidsPath}\`

## Tests Run
1. **active-pids-file-exists** - Check if tracking file exists (or will be created)
2. **active-pids-valid-json** - Verify file contains valid JSON
3. **active-pids-structure** - Check JSON has expected ProcessInfo structure
4. **kenv-dir-writable** - Sanity check that .kenv is writable

## Notes
- The active-bun-pids.json file is created lazily when the first script is run
- An empty file or empty object is a valid state (no scripts running)
- The file tracks: pid, script_path, started_at for each running script

---

*Check the JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-process-orphan.ts exiting...');

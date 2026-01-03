#!/usr/bin/env bun
/**
 * SDK Test Runner
 * 
 * Runs all tests in tests/sdk/ and reports results.
 * 
 * Usage:
 *   bun run scripts/test-runner.ts                    # Run all tests sequentially
 *   bun run scripts/test-runner.ts test-arg.ts        # Run single test
 *   bun run scripts/test-runner.ts --json             # Output JSON only
 *   bun run scripts/test-runner.ts --parallel         # Run tests concurrently
 *   bun run scripts/test-runner.ts --filter "arg|div" # Run tests matching pattern
 *   bun run scripts/test-runner.ts --parallel --filter "editor"
 * 
 * Environment:
 *   SDK_TEST_TIMEOUT=10    # Max seconds per test (default: 30)
 *   SDK_TEST_VERBOSE=true  # Extra debug output
 *   SDK_TEST_CONCURRENCY=4 # Max concurrent tests in parallel mode (default: 4)
 * 
 * =============================================================================
 * MANUAL VERIFICATION TESTS - UI Bug Fixes
 * =============================================================================
 * 
 * After running automated tests, manually verify these UI behaviors:
 * 
 * ## Bug Fix 1: Mouse Hover Highlighting
 * 
 * **Expected behavior:**
 * - Start the app: `cargo run --release`
 * - Wait for list items to load (e.g., scripts or choices)
 * - Move mouse over list items
 * - VERIFY: The highlighted item (with visual background) follows mouse cursor
 * - VERIFY: Moving mouse up/down instantly updates which item is highlighted
 * - VERIFY: Clicking a hovered item selects it
 * 
 * **Technical implementation:**
 * - list_item.rs has .index() method to track each item's position
 * - list_item.rs has .on_hover() handler to respond to mouse enter events
 * - main.rs uses cx.listener() to update selected_index when mouse enters
 * 
 * ## Bug Fix 2: Scroll Jitter Prevention
 * 
 * **Expected behavior:**
 * - Start the app: `cargo run --release`  
 * - Use keyboard (Up/Down arrows) to navigate list items
 * - VERIFY: List scrolls smoothly to keep selected item visible
 * - VERIFY: No visual jitter or jumping when navigating
 * - Now move mouse to hover over a different item (without clicking)
 * - VERIFY: Hovering does NOT cause the list to scroll
 * - VERIFY: Only keyboard navigation triggers scroll adjustments
 * 
 * **Technical implementation:**
 * - last_scrolled_index tracks the last scroll position
 * - scroll_to_selected_if_needed() helper skips redundant scroll_to_item calls
 * - Keyboard navigation uses the helper (triggers scroll when needed)
 * - Mouse hover updates selection WITHOUT triggering scroll
 * 
 * ## Combined Test Scenario
 * 
 * 1. Launch app with many items (enough to require scrolling)
 * 2. Use keyboard to navigate to bottom of list (should scroll smoothly)
 * 3. Hover mouse over items near top (should highlight them WITHOUT scrolling)
 * 4. Press Down arrow (should scroll back to maintain keyboard position)
 * 5. VERIFY: No jitter throughout this sequence
 * 
 * =============================================================================
 */

import { readdir } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';

import { spawn } from 'bun';

// =============================================================================
// Types
// =============================================================================

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

interface TestFileResult {
  file: string;
  tests: TestResult[];
  duration_ms: number;
  passed: number;
  failed: number;
  skipped: number;
}

interface RunnerSummary {
  files: TestFileResult[];
  total_passed: number;
  total_failed: number;
  total_skipped: number;
  total_duration_ms: number;
  pass_rate: number;
  slowest_tests: Array<{ file: string; duration_ms: number }>;
  mode: 'sequential' | 'parallel';
}

// =============================================================================
// Configuration
// =============================================================================

const PROJECT_ROOT = resolve(import.meta.dir, '..');
const SDK_PATH = join(PROJECT_ROOT, 'scripts', 'kit-sdk.ts');
const TESTS_DIR = join(PROJECT_ROOT, 'tests', 'sdk');
const TIMEOUT_MS = parseInt(process.env.SDK_TEST_TIMEOUT || '30', 10) * 1000;
const VERBOSE = process.env.SDK_TEST_VERBOSE === 'true';
const JSON_ONLY = process.argv.includes('--json');
const PARALLEL = process.argv.includes('--parallel');
const CONCURRENCY = parseInt(process.env.SDK_TEST_CONCURRENCY || '4', 10);

// Parse --filter pattern
function getFilterPattern(): RegExp | null {
  const filterIdx = process.argv.indexOf('--filter');
  if (filterIdx === -1 || filterIdx + 1 >= process.argv.length) {
    return null;
  }
  const pattern = process.argv[filterIdx + 1];
  try {
    return new RegExp(pattern, 'i');
  } catch {
    console.error(`Invalid filter pattern: ${pattern}`);
    process.exit(1);
  }
}

const FILTER_PATTERN = getFilterPattern();

// =============================================================================
// Utilities
// =============================================================================

function log(msg: string) {
  if (!JSON_ONLY) {
    console.log(msg);
  }
}

function logVerbose(msg: string) {
  if (VERBOSE && !JSON_ONLY) {
    console.log(`  [VERBOSE] ${msg}`);
  }
}

function jsonlLog(data: object) {
  console.log(JSON.stringify(data));
}

// Real-time progress tracking for parallel execution
let completedCount = 0;
let totalCount = 0;

function updateProgress(fileName: string, status: 'start' | 'done', result?: TestFileResult) {
  if (JSON_ONLY) return;
  
  if (status === 'start') {
    log(`  [${completedCount}/${totalCount}] Starting: ${fileName}`);
  } else {
    completedCount++;
    const icon = result && result.failed === 0 ? '✅' : '❌';
    const stats = result ? `${result.passed}p/${result.failed}f` : '';
    log(`  [${completedCount}/${totalCount}] ${icon} ${fileName} (${result?.duration_ms}ms) ${stats}`);
  }
}

// Run tests with concurrency limit
async function runTestsWithConcurrency(
  testFiles: string[],
  concurrency: number
): Promise<TestFileResult[]> {
  const results: TestFileResult[] = [];
  const queue = [...testFiles];
  const running = new Set<Promise<void>>();
  
  while (queue.length > 0 || running.size > 0) {
    // Start new tasks up to concurrency limit
    while (running.size < concurrency && queue.length > 0) {
      const file = queue.shift()!;
      const fileName = basename(file);
      updateProgress(fileName, 'start');
      
      const task = (async () => {
        const result = await runTestFile(file);
        results.push(result);
        updateProgress(fileName, 'done', result);
      })();
      
      running.add(task);
      task.finally(() => running.delete(task));
    }
    
    // Wait for at least one task to complete
    if (running.size > 0) {
      await Promise.race(running);
    }
  }
  
  return results;
}

// =============================================================================
// Test Execution
// =============================================================================

async function runTestFile(filePath: string): Promise<TestFileResult> {
  const fileName = basename(filePath);
  const startTime = Date.now();
  const tests: TestResult[] = [];
  
  // Only log individual file start in sequential mode (parallel mode uses updateProgress)
  if (!PARALLEL) {
    log(`\nRunning: ${fileName}`);
  }
  logVerbose(`Full path: ${filePath}`);
  logVerbose(`SDK path: ${SDK_PATH}`);
  
  try {
    // Run the test file with SDK preload
    // Note: For automated testing, we simulate user input by providing stdin
    const proc = spawn({
      cmd: ['bun', 'run', '--preload', SDK_PATH, filePath],
      cwd: PROJECT_ROOT,
      stdout: 'pipe',
      stderr: 'pipe',
      stdin: 'pipe',
    });
    
    // Collect stdout (JSONL test results)
    let stdout = '';
    let stderr = '';
    
    // Create a timeout promise
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error(`Test timed out after ${TIMEOUT_MS}ms`)), TIMEOUT_MS);
    });
    
    // Read stdout in chunks
    const stdoutReader = (async () => {
      const reader = proc.stdout.getReader();
      const decoder = new TextDecoder();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        stdout += decoder.decode(value);
      }
    })();
    
    // Read stderr in chunks
    const stderrReader = (async () => {
      const reader = proc.stderr.getReader();
      const decoder = new TextDecoder();
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        const chunk = decoder.decode(value);
        stderr += chunk;
        if (VERBOSE) {
          // Print stderr in real-time for debugging
          process.stderr.write(chunk);
        }
      }
    })();
    
    // For SDK-only testing (no GPUI app), we need to simulate responses
    // The test will hang waiting for submit messages, so we just let it timeout
    // In a real integration test with the GPUI app, the app would respond
    
    // For now, just wait for the process or timeout
    try {
      await Promise.race([
        Promise.all([stdoutReader, stderrReader, proc.exited]),
        timeoutPromise,
      ]);
    } catch {
      // Kill the process on timeout
      proc.kill();
      logVerbose(`Process killed due to timeout`);
    }
    
    const exitCode = await proc.exited;
    logVerbose(`Exit code: ${exitCode}`);
    logVerbose(`Stdout length: ${stdout.length}`);
    logVerbose(`Stderr length: ${stderr.length}`);
    
    // Parse JSONL results from stdout
    const lines = stdout.split('\n').filter(line => line.trim());
    for (const line of lines) {
      try {
        const result = JSON.parse(line) as TestResult;
        if (result.test && result.status) {
          tests.push(result);
          
          // Print result in human-readable format
          const icon = result.status === 'pass' ? '✅' : 
                       result.status === 'fail' ? '❌' : 
                       result.status === 'skip' ? '⏭️' : '🔄';
          const duration = result.duration_ms ? ` (${result.duration_ms}ms)` : '';
          const error = result.error ? ` - ${result.error}` : '';
          
          if (result.status !== 'running') {
            log(`  ${icon} ${result.test}${duration}${error}`);
          }
        }
      } catch {
        // Not JSON, might be other output
        logVerbose(`Non-JSON line: ${line.substring(0, 80)}...`);
      }
    }
    
    // If no tests were parsed, mark as failed
    if (tests.length === 0) {
      tests.push({
        test: fileName,
        status: 'fail',
        timestamp: new Date().toISOString(),
        error: 'No test results parsed from output',
        duration_ms: Date.now() - startTime,
      });
      log(`  ❌ No test results (check stderr output)`);
    }
    
  } catch (err) {
    tests.push({
      test: fileName,
      status: 'fail',
      timestamp: new Date().toISOString(),
      error: String(err),
      duration_ms: Date.now() - startTime,
    });
    log(`  ❌ Error: ${err}`);
  }
  
  const duration_ms = Date.now() - startTime;
  
  // Count results (only count final status, not 'running')
  const finalTests = tests.filter(t => t.status !== 'running');
  const uniqueTests = new Map<string, TestResult>();
  for (const t of finalTests) {
    uniqueTests.set(t.test, t);
  }
  
  const passed = Array.from(uniqueTests.values()).filter(t => t.status === 'pass').length;
  const failed = Array.from(uniqueTests.values()).filter(t => t.status === 'fail').length;
  const skipped = Array.from(uniqueTests.values()).filter(t => t.status === 'skip').length;
  
  return {
    file: fileName,
    tests,
    duration_ms,
    passed,
    failed,
    skipped,
  };
}

async function findTestFiles(specificTest?: string): Promise<string[]> {
  if (specificTest) {
    // Handle relative or absolute path
    if (specificTest.startsWith('/')) {
      return [specificTest];
    }
    // Check if it's just a filename
    const testPath = specificTest.includes('/') 
      ? join(PROJECT_ROOT, specificTest)
      : join(TESTS_DIR, specificTest);
    return [testPath];
  }
  
  // Find all test-*.ts files in tests/sdk/
  try {
    const files = await readdir(TESTS_DIR);
    let testFiles = files
      .filter(f => f.startsWith('test-') && f.endsWith('.ts'))
      .sort();
    
    // Apply filter pattern if specified
    if (FILTER_PATTERN) {
      testFiles = testFiles.filter(f => FILTER_PATTERN!.test(f));
      logVerbose(`Filter pattern matched ${testFiles.length} files`);
    }
    
    return testFiles.map(f => join(TESTS_DIR, f));
  } catch {
    log(`Warning: Could not read ${TESTS_DIR}`);
    return [];
  }
}

// =============================================================================
// Main
// =============================================================================

async function main() {
  const startTime = Date.now();
  
  // Parse arguments - filter out flags and their values
  const args: string[] = [];
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--filter') {
      i++; // Skip the next arg (the filter value)
    } else if (!arg.startsWith('--')) {
      args.push(arg);
    }
  }
  const specificTest = args[0];
  
  const mode = PARALLEL ? 'parallel' : 'sequential';
  
  if (!JSON_ONLY) {
    log('SDK Test Runner v2.0');
    log('═'.repeat(60));
    log(`Mode: ${mode}${PARALLEL ? ` (concurrency: ${CONCURRENCY})` : ''}`);
    if (FILTER_PATTERN) {
      log(`Filter: ${FILTER_PATTERN.source}`);
    }
    log('');
  }
  
  // Find test files
  const testFiles = await findTestFiles(specificTest);
  
  if (testFiles.length === 0) {
    log('No test files found');
    if (FILTER_PATTERN) {
      log(`Hint: No files matched filter pattern "${FILTER_PATTERN.source}"`);
    }
    process.exit(1);
  }
  
  log(`Found ${testFiles.length} test file(s)`);
  logVerbose(`Files: ${testFiles.map(f => basename(f)).join(', ')}`);
  
  // Initialize progress tracking
  totalCount = testFiles.length;
  completedCount = 0;
  
  // Run tests (parallel or sequential)
  let results: TestFileResult[];
  
  if (PARALLEL) {
    log('');
    log('Running tests in parallel...');
    results = await runTestsWithConcurrency(testFiles, CONCURRENCY);
  } else {
    // Sequential execution (original behavior)
    results = [];
    for (const file of testFiles) {
      const result = await runTestFile(file);
      results.push(result);
      
      // Output JSONL for machine parsing
      if (JSON_ONLY) {
        jsonlLog({
          type: 'file_result',
          ...result,
        });
      }
    }
  }
  
  // Calculate statistics
  const totalTests = results.reduce((sum, r) => sum + r.passed + r.failed + r.skipped, 0);
  const totalPassed = results.reduce((sum, r) => sum + r.passed, 0);
  const totalFailed = results.reduce((sum, r) => sum + r.failed, 0);
  const totalSkipped = results.reduce((sum, r) => sum + r.skipped, 0);
  const totalDuration = Date.now() - startTime;
  const passRate = totalTests > 0 ? (totalPassed / totalTests) * 100 : 0;
  
  // Find slowest tests (top 5)
  const slowestTests = [...results]
    .sort((a, b) => b.duration_ms - a.duration_ms)
    .slice(0, 5)
    .map(r => ({ file: r.file, duration_ms: r.duration_ms }));
  
  // Build summary
  const summary: RunnerSummary = {
    files: results,
    total_passed: totalPassed,
    total_failed: totalFailed,
    total_skipped: totalSkipped,
    total_duration_ms: totalDuration,
    pass_rate: Math.round(passRate * 100) / 100,
    slowest_tests: slowestTests,
    mode,
  };
  
  // Print summary
  if (!JSON_ONLY) {
    log('');
    log('═'.repeat(60));
    log('SUMMARY');
    log('═'.repeat(60));
    log(`Mode:       ${mode}${PARALLEL ? ` (${CONCURRENCY} workers)` : ''}`);
    log(`Tests:      ${totalTests} total`);
    log(`Results:    ${totalPassed} passed, ${totalFailed} failed, ${totalSkipped} skipped`);
    log(`Pass rate:  ${passRate.toFixed(1)}%`);
    log(`Duration:   ${totalDuration}ms`);
    
    if (slowestTests.length > 0) {
      log('');
      log('Slowest tests:');
      for (const t of slowestTests) {
        log(`  ${t.duration_ms.toString().padStart(6)}ms  ${t.file}`);
      }
    }
    
    // Show failed test files if any
    const failedFiles = results.filter(r => r.failed > 0);
    if (failedFiles.length > 0) {
      log('');
      log('Failed test files:');
      for (const f of failedFiles) {
        log(`  ❌ ${f.file} (${f.failed} failed)`);
      }
    }
    
    log('');
    log(totalFailed === 0 ? '✅ All tests passed!' : `❌ ${totalFailed} test(s) failed`);
  }
  
  // Output final summary as JSONL
  if (JSON_ONLY) {
    jsonlLog({
      type: 'summary',
      ...summary,
    });
  }
  
  // Exit with appropriate code
  process.exit(summary.total_failed > 0 ? 1 : 0);
}

main().catch(err => {
  console.error('Test runner error:', err);
  process.exit(1);
});

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
 *   bun run scripts/test-runner.ts --include-system  # Include tests that send real keystrokes
 * 
 * Environment:
 *   SDK_TEST_TIMEOUT=10    # Max seconds per test (default: 5)
 *   SDK_TEST_VERBOSE=true  # Extra debug output
 *   SDK_TEST_CONCURRENCY=4 # Max workers (default: 2 noninteractive, 4 otherwise)
 *   SDK_TEST_MAX_OUTPUT_BYTES=1048576 # Per-stream child-output safety budget
 *   SCRIPT_KIT_NONINTERACTIVE=1 # Refuse system-input tests before starting children
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

import { readdir, realpath } from 'node:fs/promises';
import { basename, isAbsolute, join, relative, resolve, sep } from 'node:path';

import { spawn } from 'bun';
import { SDK_SYSTEM_INPUT_TESTS } from '../tests/sdk/system-input-tests.ts';

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
const RUNNER_ARGUMENTS = process.argv.slice(2);

function refuseRunner(message: string): never {
  console.error(`[sdk-tests] REFUSED ${message}`);
  process.exit(78);
}

function validateRunnerArguments(): void {
  let specificTests = 0;
  let filterCount = 0;
  for (let index = 0; index < RUNNER_ARGUMENTS.length; index += 1) {
    const argument = RUNNER_ARGUMENTS[index]!;
    if (argument === '--filter') {
      filterCount += 1;
      const pattern = RUNNER_ARGUMENTS[index + 1];
      if (!pattern || pattern.startsWith('-')) {
        refuseRunner('--filter requires one non-option pattern');
      }
      if (filterCount > 1) {
        refuseRunner('--filter may be provided only once');
      }
      index += 1;
      continue;
    }
    if (argument === '--json' || argument === '--parallel' || argument === '--include-system') {
      continue;
    }
    if (argument.startsWith('-')) {
      refuseRunner(`unknown SDK test-runner option: ${argument}`);
    }
    specificTests += 1;
    if (specificTests > 1) {
      refuseRunner('only one reviewed SDK test path may be provided');
    }
  }
}

validateRunnerArguments();

function positiveSafeInteger(name: string, fallback: number): number {
  const configured = process.env[name];
  if (configured === undefined || configured === '') return fallback;
  const parsed = Number(configured);
  if (!/^[1-9][0-9]*$/.test(configured) || !Number.isSafeInteger(parsed)) {
    refuseRunner(`${name} must be a positive safe integer`);
  }
  return parsed;
}

const NONINTERACTIVE_AUTHORITY = process.env.SCRIPT_KIT_NONINTERACTIVE ?? '0';
if (NONINTERACTIVE_AUTHORITY !== '0' && NONINTERACTIVE_AUTHORITY !== '1') {
  refuseRunner('SCRIPT_KIT_NONINTERACTIVE must be 0 or 1');
}
const NONINTERACTIVE = NONINTERACTIVE_AUTHORITY === '1';
const TIMEOUT_SECONDS = positiveSafeInteger('SDK_TEST_TIMEOUT', 5);
if (TIMEOUT_SECONDS > Math.floor(0x7fffffff / 1000)) {
  refuseRunner('SDK_TEST_TIMEOUT exceeds the supported timer range');
}
const TIMEOUT_MS = TIMEOUT_SECONDS * 1000;
const MAX_OUTPUT_BYTES = positiveSafeInteger('SDK_TEST_MAX_OUTPUT_BYTES', 1024 * 1024);
if (MAX_OUTPUT_BYTES > 8 * 1024 * 1024) {
  refuseRunner('SDK_TEST_MAX_OUTPUT_BYTES exceeds the eight-megabyte safety ceiling');
}
const VERBOSE = process.env.SDK_TEST_VERBOSE === 'true';
const JSON_ONLY = process.argv.includes('--json');
const PARALLEL = process.argv.includes('--parallel');
const INCLUDE_SYSTEM = process.argv.includes('--include-system');
const CONCURRENCY = positiveSafeInteger(
  'SDK_TEST_CONCURRENCY',
  NONINTERACTIVE ? 2 : 4,
);
if (CONCURRENCY > (NONINTERACTIVE ? 2 : 8)) {
  refuseRunner(`SDK_TEST_CONCURRENCY exceeds the ${NONINTERACTIVE ? 'two' : 'eight'}-worker safety ceiling`);
}

if (NONINTERACTIVE && INCLUDE_SYSTEM) {
  console.error(
    'Refusing --include-system: SCRIPT_KIT_NONINTERACTIVE=1 prohibits real system input.',
  );
  process.exit(1);
}

// Tests that send real keystrokes/clipboard writes to the OS or exercise
// GPUI-unsupported SDK helpers.
// Excluded by default to avoid interfering with the user's desktop and to keep
// CI aligned with the supported GPUI runtime surface.
// Pass --include-system to run them.
const SYSTEM_INPUT_TESTS = new Set<string>(SDK_SYSTEM_INPUT_TESTS);

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
    // SDK_TEST_AUTOSUBMIT=1 enables auto-resolution of prompts for CI testing
    const proc = spawn({
      cmd: ['bun', 'run', '--preload', SDK_PATH, filePath],
      detached: true,
      cwd: PROJECT_ROOT,
      stdout: 'pipe',
      stderr: 'pipe',
      stdin: 'pipe',
      env: {
        ...process.env,
        SDK_TEST_AUTOSUBMIT: '1',
        SDK_TEST_WINDOW_FIXTURES: fileName === 'test-window-management.ts' ? '1' : '0',
        SDK_TEST_CLIPBOARD_FIXTURES: fileName === 'test-clipboard-history.ts' ? '1' : '0',
        SDK_TEST_FILE_FIXTURES: fileName === 'test-file-search.ts' ? '1' : '0',
        SDK_TEST_MENU_FIXTURES: fileName === 'test-menu-bar-api.ts' ? '1' : '0',
        SDK_TEST_CONCURRENCY: String(CONCURRENCY),
        SDK_TEST_MAX_OUTPUT_BYTES: String(MAX_OUTPUT_BYTES),
        SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: '0',
        SCRIPT_KIT_ALLOW_VISIBLE_PROBES: '0',
        SCRIPT_KIT_ALLOW_NATIVE_INPUT: '0',
        SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: '0',
        SCRIPT_KIT_ALLOW_LIVE_AI: '0',
        SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: '0',
        SCRIPT_KIT_TEST_STATUS: '0',
        ...(NONINTERACTIVE ? { INCLUDE_SYSTEM_INPUT: '0' } : {}),
      },
    });
    
    // Collect stdout (JSONL test results)
    let stdout = '';
    let stderr = '';
    
    // Create a timeout promise
    let timeoutHandle: ReturnType<typeof setTimeout> | undefined;
    let processFailure: Error | undefined;
    let processFailureLabel = 'process failure';
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutHandle = setTimeout(() => {
        processFailure = new Error(`Test timed out after ${TIMEOUT_MS}ms`);
        processFailureLabel = 'process timeout';
        reject(processFailure);
      }, TIMEOUT_MS);
    });
    
    // Read stdout in chunks
    const stdoutReader = (async () => {
      const reader = proc.stdout.getReader();
      const decoder = new TextDecoder();
      let outputBytes = 0;
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        const remaining = MAX_OUTPUT_BYTES - outputBytes;
        if (value.byteLength > remaining) {
          if (remaining > 0) {
            stdout += decoder.decode(value.subarray(0, remaining), { stream: true });
          }
          throw new Error(`SDK stdout exceeds the ${MAX_OUTPUT_BYTES}-byte safety budget`);
        }
        outputBytes += value.byteLength;
        stdout += decoder.decode(value, { stream: true });
      }
      stdout += decoder.decode();
    })();
    
    // Read stderr in chunks
    const stderrReader = (async () => {
      const reader = proc.stderr.getReader();
      const decoder = new TextDecoder();
      let outputBytes = 0;
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        const remaining = MAX_OUTPUT_BYTES - outputBytes;
        if (value.byteLength > remaining) {
          if (remaining > 0) {
            stderr += decoder.decode(value.subarray(0, remaining), { stream: true });
          }
          throw new Error(`SDK stderr exceeds the ${MAX_OUTPUT_BYTES}-byte safety budget`);
        }
        outputBytes += value.byteLength;
        const chunk = decoder.decode(value, { stream: true });
        stderr += chunk;
        if (VERBOSE) {
          // Print stderr in real-time for debugging
          process.stderr.write(chunk);
        }
      }
      stderr += decoder.decode();
    })();
    
    try {
      await Promise.race([
        Promise.all([stdoutReader, stderrReader, proc.exited]),
        timeoutPromise,
      ]);
    } catch (error) {
      try {
        process.kill(-proc.pid, 'SIGTERM');
      } catch {
        proc.kill();
      }
      const closed = Promise.allSettled([stdoutReader, stderrReader, proc.exited]);
      let graceTimer: ReturnType<typeof setTimeout> | undefined;
      const drained = await Promise.race([
        closed.then(() => true),
        new Promise<boolean>((resolveGrace) => {
          graceTimer = setTimeout(() => resolveGrace(false), 150);
        }),
      ]);
      if (graceTimer !== undefined) clearTimeout(graceTimer);
      if (!drained) {
        try {
          process.kill(-proc.pid, 'SIGKILL');
        } catch {
          proc.kill();
        }
      }
      await closed;
      processFailure ??= error instanceof Error ? error : new Error(String(error));
      if (processFailure.message.includes('-byte safety budget')) {
        processFailureLabel = 'output limit';
      }
      logVerbose(`Process killed: ${processFailure.message}`);
    } finally {
      if (timeoutHandle !== undefined) {
        clearTimeout(timeoutHandle);
      }
    }
    
    const exitCode = await proc.exited;
    logVerbose(`Exit code: ${exitCode}`);
    logVerbose(`Stdout length: ${stdout.length}`);
    logVerbose(`Stderr length: ${stderr.length}`);
    
    // Parse JSONL results from stdout
    const lines = stdout.split('\n').filter(line => line.trim());
    const terminalResults = new Map<string, TestResult['status']>();
    for (const [lineIndex, line] of lines.entries()) {
      let parsed: unknown;
      try {
        parsed = JSON.parse(line);
      } catch {
        if (/^\s*\{/.test(line) && /"(?:test|status)"\s*:/.test(line)) {
          tests.push({
            test: `${fileName} [malformed result ${lineIndex + 1}]`,
            status: 'fail',
            timestamp: new Date().toISOString(),
            error: 'malformed SDK result JSON cannot be ignored',
          });
          continue;
        }
        logVerbose(`Non-JSON line: ${line.substring(0, 80)}...`);
        continue;
      }

      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
        continue;
      }
      const rawResult = parsed as Record<string, unknown>;
      if (!Object.hasOwn(rawResult, 'test') && !Object.hasOwn(rawResult, 'status')) {
        continue;
      }

      const malformedResult = (message: string) => {
        const resultName = typeof rawResult.test === 'string' && rawResult.test.trim()
          ? rawResult.test
          : fileName;
        tests.push({
          test: `${resultName} [invalid result ${lineIndex + 1}]`,
          status: 'fail',
          timestamp: new Date().toISOString(),
          error: message,
        });
      };

      if (typeof rawResult.test !== 'string' || rawResult.test.trim() === '') {
        malformedResult('SDK result requires a nonempty test name');
        continue;
      }
      if (typeof rawResult.status !== 'string') {
        malformedResult('SDK result requires a recognized status');
        continue;
      }
      if (!['running', 'pass', 'fail', 'skip'].includes(rawResult.status)) {
        tests.push({
          test: `${rawResult.test} [invalid status]`,
          status: 'fail',
          timestamp: new Date().toISOString(),
          error: `Unrecognized test status: ${String(rawResult.status)}`,
        });
        log(`  ❌ ${rawResult.test} - Unrecognized test status: ${String(rawResult.status)}`);
        continue;
      }
      if (typeof rawResult.timestamp !== 'string' || Number.isNaN(Date.parse(rawResult.timestamp))) {
        malformedResult('SDK result requires a valid timestamp');
        continue;
      }
      if (
        rawResult.duration_ms !== undefined &&
        (!Number.isSafeInteger(rawResult.duration_ms) || Number(rawResult.duration_ms) < 0)
      ) {
        malformedResult('SDK result requires a nonnegative safe duration');
        continue;
      }

      const result = rawResult as unknown as TestResult;
      const terminalStatus = terminalResults.get(result.test);
      if (terminalStatus) {
        const failureLabel = result.status === 'running'
          ? 'post-terminal transition'
          : 'duplicate terminal result';
        tests.push({
          test: `${result.test} [${failureLabel}]`,
          status: 'fail',
          timestamp: new Date().toISOString(),
          error:
            `SDK result ${result.test} already completed as ${terminalStatus}; ` +
            `a later ${result.status} result cannot replace it.`,
          duration_ms: result.duration_ms,
        });
        continue;
      }
      if ((result.status === 'pass' || result.status === 'skip') && result.error != null) {
        tests.push({
          ...result,
          status: 'fail',
          error: `A ${result.status === 'pass' ? 'passing' : 'skipped'} SDK result cannot carry an error: ${String(result.error)}`,
        });
        terminalResults.set(result.test, 'fail');
        continue;
      }
      if (result.status !== 'running') {
        terminalResults.set(result.test, result.status);
      }

      tests.push(result);

      const icon = result.status === 'pass' ? '✅' :
                   result.status === 'fail' ? '❌' :
                   result.status === 'skip' ? '⏭️' : '🔄';
      const duration = result.duration_ms ? ` (${result.duration_ms}ms)` : '';
      const error = result.error ? ` - ${result.error}` : '';

      if (result.status !== 'running') {
        log(`  ${icon} ${result.test}${duration}${error}`);
      }
    }

    const terminalNames = new Set(
      tests.filter((result) => result.status !== 'running').map((result) => result.test),
    );
    for (const result of tests.filter((candidate) => candidate.status === 'running')) {
      if (!terminalNames.has(result.test)) {
        tests.push({
          test: `${result.test} [missing terminal result]`,
          status: 'fail',
          timestamp: new Date().toISOString(),
          error: 'Test started but never emitted pass, fail, or an explicit skip.',
          duration_ms: Date.now() - startTime,
        });
      }
    }

    if (processFailure) {
      tests.push({
        test: `${fileName} [${processFailureLabel}]`,
        status: 'fail',
        timestamp: new Date().toISOString(),
        error: processFailure.message,
        duration_ms: Date.now() - startTime,
      });
      log(`  ❌ ${fileName} - ${processFailure.message}`);
    } else if (exitCode !== 0) {
      tests.push({
        test: `${fileName} [process exit]`,
        status: 'fail',
        timestamp: new Date().toISOString(),
        error: `Test process exited with code ${exitCode}.`,
        duration_ms: Date.now() - startTime,
      });
      log(`  ❌ ${fileName} - Process exited with code ${exitCode}`);
    }
    
    // If no tests were parsed, mark as failed
    if (!tests.some((result) => result.status !== 'running')) {
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

async function protectedSystemInputOwner(testPath: string): Promise<string | null> {
  const requestedName = basename(testPath);
  if (SYSTEM_INPUT_TESTS.has(requestedName)) return requestedName;

  const canonicalName = basename(await realpath(testPath));
  return SYSTEM_INPUT_TESTS.has(canonicalName) ? canonicalName : null;
}

async function canonicalReviewedSdkTest(testPath: string, canonicalRoot: string): Promise<string> {
  const canonicalTest = await realpath(testPath);
  const owner = relative(canonicalRoot, canonicalTest);
  if (
    owner === '' ||
    owner === '..' ||
    owner.startsWith(`..${sep}`) ||
    isAbsolute(owner)
  ) {
    throw new Error('Noninteractive SDK tests require a reviewed tests/sdk owner.');
  }
  return canonicalTest;
}

async function findTestFiles(specificTest?: string): Promise<string[]> {
  if (specificTest) {
    const testPath = specificTest.startsWith('/')
      ? specificTest
      : specificTest.includes('/')
        ? join(PROJECT_ROOT, specificTest)
        : join(TESTS_DIR, specificTest);

    const protectedOwner = INCLUDE_SYSTEM ? null : await protectedSystemInputOwner(testPath);
    if (protectedOwner) {
      throw new Error(
        `Refusing system-input test ${protectedOwner} without --include-system.`,
      );
    }

    if (NONINTERACTIVE) {
      const canonicalRoot = await realpath(TESTS_DIR);
      return [await canonicalReviewedSdkTest(testPath, canonicalRoot)];
    }

    return [testPath];
  }
  
  // Find all test-*.ts files in tests/sdk/
  try {
    const files = await readdir(TESTS_DIR);
    let testFiles = files
      .filter(f => f.startsWith('test-') && f.endsWith('.ts'))
      .sort();
    let canonicalPaths = new Map<string, string>();

    // Exclude tests that send real system input (keystrokes, clipboard)
    // unless --include-system is passed
    if (!INCLUDE_SYSTEM) {
      const before = testFiles.length;
      const reviewedFiles = await Promise.all(
        testFiles.map(async (file) => ({
          file,
          protectedOwner: await protectedSystemInputOwner(join(TESTS_DIR, file)),
        })),
      );
      testFiles = reviewedFiles
        .filter(({ protectedOwner }) => protectedOwner === null)
        .map(({ file }) => file);
      if (testFiles.length < before) {
        log(`Skipping ${before - testFiles.length} system-input test(s) (use --include-system to include)`);
      }
    }

    if (NONINTERACTIVE) {
      const canonicalRoot = await realpath(TESTS_DIR);
      canonicalPaths = new Map(await Promise.all(
        testFiles.map(async (file) => [
          file,
          await canonicalReviewedSdkTest(join(TESTS_DIR, file), canonicalRoot),
        ] as const),
      ));
    }

    // Apply filter pattern if specified
    if (FILTER_PATTERN) {
      testFiles = testFiles.filter(f => FILTER_PATTERN!.test(f));
      logVerbose(`Filter pattern matched ${testFiles.length} files`);
    }
    
    return testFiles.map(f => canonicalPaths.get(f) ?? join(TESTS_DIR, f));
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`SDK test discovery failed: ${detail}`);
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

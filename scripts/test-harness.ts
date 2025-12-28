#!/usr/bin/env bun
/**
 * Autonomous Test Harness
 *
 * Comprehensive test runner that enables fully autonomous testing without manual interaction.
 * Spawns the script-kit-gpui binary with AUTO_SUBMIT mode and monitors for results.
 *
 * IMPORTANT: Uses stdin JSON protocol to run scripts. Command line arguments do NOT work!
 * Protocol: echo '{"type":"run","path":"/abs/path/test.ts"}' | ./script-kit-gpui
 *
 * Usage:
 *   bun run scripts/test-harness.ts                          # Run all autonomous tests
 *   bun run scripts/test-harness.ts tests/autonomous/*.ts    # Run specific tests
 *   bun run scripts/test-harness.ts --json                   # Output JSONL only
 *   bun run scripts/test-harness.ts --verbose                # Extra debug output
 *
 * Environment:
 *   TEST_TIMEOUT_MS=30000       # Max milliseconds per test (default: 30000)
 *   AUTO_SUBMIT_DELAY_MS=100    # Delay before auto-submit (default: 100)
 *   BINARY_PATH=./target/debug/script-kit-gpui  # Path to binary
 *   HEADLESS=true               # Skip UI rendering (if supported)
 *   SCRIPT_KIT_AI_LOG=1         # Enable compact AI logs (auto-set by harness)
 *
 * Exit Codes:
 *   0 = All tests passed
 *   1 = Some tests failed
 *   2 = Test runner error
 *   3 = Timeout exceeded
 *   4 = Crash detected
 *
 * Test Result JSONL Format (stdout from test scripts):
 *   {"test": "name", "status": "pass"|"fail"|"skip", "timestamp": "...", "duration_ms": 123}
 */

import { spawn, type Subprocess } from 'bun';
import { readdir, stat } from 'node:fs/promises';
import { basename, join, resolve, dirname } from 'node:path';
import { existsSync } from 'node:fs';

// =============================================================================
// Timeout Handling Utility
// =============================================================================

const TEST_TIMEOUT_MS = parseInt(process.env.TEST_TIMEOUT_MS || '30000');

/**
 * Run an async function with a timeout.
 * Rejects with a timeout error if the function doesn't complete in time.
 */
async function runWithTimeout<T>(
  fn: () => Promise<T>,
  timeoutMs: number,
  name: string
): Promise<T> {
  return Promise.race([
    fn(),
    new Promise<never>((_, reject) =>
      setTimeout(
        () => reject(new Error(`${name} timed out after ${timeoutMs}ms`)),
        timeoutMs
      )
    ),
  ]);
}

// =============================================================================
// Types
// =============================================================================

export interface TestResult {
  test: string;
  status: 'pass' | 'fail' | 'timeout' | 'crash';
  duration_ms: number;
  error?: string;
  stdout?: string;
  stderr?: string;
}

interface InternalTestEvent {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
  reason?: string;
}

interface TestFileResult {
  file: string;
  tests: TestResult[];
  duration_ms: number;
  passed: number;
  failed: number;
  timeout: number;
  crashed: number;
  skipped: number;
}

interface HarnessSummary {
  files: TestFileResult[];
  total_passed: number;
  total_failed: number;
  total_timeout: number;
  total_crashed: number;
  total_skipped: number;
  total_duration_ms: number;
  exit_code: number;
}

// =============================================================================
// Configuration
// =============================================================================

const PROJECT_ROOT = resolve(import.meta.dir, '..');
const DEFAULT_BINARY = join(PROJECT_ROOT, 'target', 'debug', 'script-kit-gpui');
const AUTONOMOUS_TESTS_DIR = join(PROJECT_ROOT, 'tests', 'autonomous');
const DEFAULT_TIMEOUT_MS = 30000;
const DEFAULT_AUTO_SUBMIT_DELAY_MS = 100;

// Parse CLI flags
const args = process.argv.slice(2);
const JSON_ONLY = args.includes('--json');
const VERBOSE = args.includes('--verbose') || process.env.SDK_TEST_VERBOSE === 'true';
const testArgs = args.filter(a => !a.startsWith('--'));

// Environment configuration
const TIMEOUT_MS = parseInt(process.env.TEST_TIMEOUT_MS || String(DEFAULT_TIMEOUT_MS), 10);
const AUTO_SUBMIT_DELAY_MS = parseInt(process.env.AUTO_SUBMIT_DELAY_MS || String(DEFAULT_AUTO_SUBMIT_DELAY_MS), 10);
const BINARY_PATH = process.env.BINARY_PATH || DEFAULT_BINARY;
const HEADLESS = process.env.HEADLESS === 'true';

// Crash detection patterns in stderr
const CRASH_PATTERNS = [
  /panic/i,
  /SIGSEGV/i,
  /SIGBUS/i,
  /SIGABRT/i,
  /assertion failed/i,
  /segmentation fault/i,
  /bus error/i,
  /abort trap/i,
  /fatal error/i,
  /thread.*panicked/i,
];

// =============================================================================
// Logging Utilities
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

function logStderr(msg: string) {
  if (!JSON_ONLY) {
    console.error(msg);
  }
}

function jsonlLog(data: object) {
  console.log(JSON.stringify(data));
}

// =============================================================================
// Binary Verification
// =============================================================================

async function verifyBinary(): Promise<boolean> {
  if (!existsSync(BINARY_PATH)) {
    logStderr(`Error: Binary not found at ${BINARY_PATH}`);
    logStderr('Run `cargo build` first to build the script-kit-gpui binary.');
    return false;
  }

  try {
    const info = await stat(BINARY_PATH);
    if (!info.isFile()) {
      logStderr(`Error: ${BINARY_PATH} is not a file`);
      return false;
    }
    logVerbose(`Binary found: ${BINARY_PATH} (${info.size} bytes)`);
    return true;
  } catch (err) {
    logStderr(`Error checking binary: ${err}`);
    return false;
  }
}

// =============================================================================
// Test Discovery
// =============================================================================

async function findTestFiles(patterns: string[]): Promise<string[]> {
  if (patterns.length > 0) {
    // Resolve provided paths
    const files: string[] = [];
    for (const pattern of patterns) {
      // Handle glob patterns or direct paths
      if (pattern.includes('*')) {
        // Simple glob handling - expand wildcards
        const dir = dirname(pattern);
        const basePattern = basename(pattern);
        const regex = new RegExp('^' + basePattern.replace(/\*/g, '.*') + '$');
        
        try {
          const resolvedDir = resolve(PROJECT_ROOT, dir);
          const entries = await readdir(resolvedDir);
          for (const entry of entries) {
            if (regex.test(entry) && entry.endsWith('.ts')) {
              files.push(join(resolvedDir, entry));
            }
          }
        } catch {
          logVerbose(`Could not read directory: ${dir}`);
        }
      } else {
        // Direct path
        const resolved = pattern.startsWith('/') ? pattern : resolve(PROJECT_ROOT, pattern);
        if (existsSync(resolved)) {
          files.push(resolved);
        } else {
          logStderr(`Warning: Test file not found: ${resolved}`);
        }
      }
    }
    return files.sort();
  }

  // Default: find all tests in tests/autonomous/
  if (!existsSync(AUTONOMOUS_TESTS_DIR)) {
    log(`Creating tests/autonomous/ directory...`);
    await Bun.write(join(AUTONOMOUS_TESTS_DIR, '.gitkeep'), '');
    return [];
  }

  try {
    const files = await readdir(AUTONOMOUS_TESTS_DIR);
    return files
      .filter(f => f.startsWith('test-') && f.endsWith('.ts'))
      .map(f => join(AUTONOMOUS_TESTS_DIR, f))
      .sort();
  } catch (err) {
    logStderr(`Error reading tests directory: ${err}`);
    return [];
  }
}

// =============================================================================
// Crash Detection
// =============================================================================

function detectCrash(stderr: string, exitCode: number | null): string | null {
  // Check exit code first
  if (exitCode !== null && exitCode !== 0) {
    // Signal-based exit codes (128 + signal number)
    if (exitCode === 139) return 'SIGSEGV (segmentation fault)';
    if (exitCode === 134) return 'SIGABRT (abort)';
    if (exitCode === 138) return 'SIGBUS (bus error)';
    if (exitCode === 137) return 'SIGKILL (killed)';
  }

  // Check stderr for crash patterns
  for (const pattern of CRASH_PATTERNS) {
    const match = stderr.match(pattern);
    if (match) {
      return `Crash detected: ${match[0]}`;
    }
  }

  // Non-zero exit without crash pattern
  if (exitCode !== null && exitCode !== 0) {
    return `Process exited with code ${exitCode}`;
  }

  return null;
}

// =============================================================================
// JSONL Parsing
// =============================================================================

function parseTestEvents(stdout: string): InternalTestEvent[] {
  const events: InternalTestEvent[] = [];
  const lines = stdout.split('\n').filter(line => line.trim());

  for (const line of lines) {
    try {
      const parsed = JSON.parse(line);
      // Validate it looks like a test event
      if (parsed.test && parsed.status) {
        events.push(parsed as InternalTestEvent);
      }
    } catch {
      // Not JSON, skip
      logVerbose(`Non-JSON stdout line: ${line.substring(0, 100)}...`);
    }
  }

  return events;
}

function aggregateTestResults(events: InternalTestEvent[]): Map<string, InternalTestEvent> {
  // Group events by test name, keep the final state
  const results = new Map<string, InternalTestEvent>();

  for (const event of events) {
    const existing = results.get(event.test);
    // Only update if this is a final state (not 'running')
    if (!existing || event.status !== 'running') {
      results.set(event.test, event);
    }
  }

  return results;
}

// =============================================================================
// Test Execution
// =============================================================================

async function runTestFile(testPath: string): Promise<TestFileResult> {
  const fileName = basename(testPath);
  const startTime = Date.now();
  const tests: TestResult[] = [];

  log(`\n${'─'.repeat(70)}`);
  log(`Running: ${testPath}`);
  log(`${'─'.repeat(70)}`);

  logVerbose(`Binary: ${BINARY_PATH}`);
  logVerbose(`Timeout: ${TIMEOUT_MS}ms`);
  logVerbose(`Auto-submit delay: ${AUTO_SUBMIT_DELAY_MS}ms`);

  let stdout = '';
  let stderr = '';
  let exitCode: number | null = null;
  let timedOut = false;
  let proc: Subprocess | null = null;

  try {
    // Spawn the script-kit-gpui binary using stdin JSON protocol
    // CRITICAL: Command line args don't work! Must use stdin JSON messages.
    proc = spawn({
      cmd: [BINARY_PATH],
      cwd: PROJECT_ROOT,
      env: {
        ...process.env,
        AUTO_SUBMIT: 'true',
        AUTO_SUBMIT_DELAY_MS: String(AUTO_SUBMIT_DELAY_MS),
        TEST_TIMEOUT_MS: String(TIMEOUT_MS),
        HEADLESS: HEADLESS ? 'true' : 'false',
        SCRIPT_KIT_AI_LOG: '1',  // Enable compact AI logs
        // Ensure Rust logging goes to stderr
        RUST_LOG: process.env.RUST_LOG || 'info',
      },
      stdin: 'pipe',
      stdout: 'pipe',
      stderr: 'pipe',
    });

    // Send the run command via stdin JSON protocol
    const runCommand = JSON.stringify({ type: 'run', path: testPath }) + '\n';
    logVerbose(`Sending stdin command: ${runCommand.trim()}`);
    proc.stdin.write(runCommand);
    proc.stdin.end();

    // Create timeout promise
    const timeoutId = setTimeout(() => {
      timedOut = true;
      if (proc) {
        logVerbose('Timeout reached, killing process...');
        proc.kill();
      }
    }, TIMEOUT_MS);

    // Read stdout and stderr concurrently
    const stdoutPromise = (async () => {
      const reader = proc!.stdout.getReader();
      const decoder = new TextDecoder();
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const chunk = decoder.decode(value);
          stdout += chunk;
          logVerbose(`stdout: ${chunk.trim()}`);
        }
      } catch (err) {
        logVerbose(`stdout read error: ${err}`);
      }
    })();

    const stderrPromise = (async () => {
      const reader = proc!.stderr.getReader();
      const decoder = new TextDecoder();
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const chunk = decoder.decode(value);
          stderr += chunk;
          if (VERBOSE) {
            // Stream stderr in real-time for debugging
            process.stderr.write(`  [stderr] ${chunk}`);
          }
        }
      } catch (err) {
        logVerbose(`stderr read error: ${err}`);
      }
    })();

    // Wait for process to complete
    await Promise.all([stdoutPromise, stderrPromise]);
    exitCode = await proc.exited;
    clearTimeout(timeoutId);

    logVerbose(`Exit code: ${exitCode}`);
    logVerbose(`Stdout length: ${stdout.length} bytes`);
    logVerbose(`Stderr length: ${stderr.length} bytes`);

  } catch (err) {
    logStderr(`Process error: ${err}`);
    stderr += `\nProcess error: ${err}`;
  }

  // Parse test events from stdout
  const events = parseTestEvents(stdout);
  const aggregated = aggregateTestResults(events);
  const duration_ms = Date.now() - startTime;

  logVerbose(`Parsed ${events.length} events, ${aggregated.size} unique tests`);

  // Check for crashes
  const crashReason = detectCrash(stderr, exitCode);

  // Convert events to test results
  if (aggregated.size > 0) {
    for (const [testName, event] of aggregated) {
      let status: TestResult['status'];
      let error: string | undefined;

      if (timedOut) {
        status = 'timeout';
        error = `Test timed out after ${TIMEOUT_MS}ms`;
      } else if (crashReason) {
        status = 'crash';
        error = crashReason;
      } else if (event.status === 'pass') {
        status = 'pass';
      } else if (event.status === 'skip') {
        // Convert skip to pass with note (we track skips separately)
        status = 'pass';
        error = event.reason || 'Skipped';
      } else {
        status = 'fail';
        error = event.error;
      }

      const result: TestResult = {
        test: testName,
        status,
        duration_ms: event.duration_ms || duration_ms,
        error,
        stdout: VERBOSE ? stdout : undefined,
        stderr: VERBOSE ? stderr : undefined,
      };

      tests.push(result);

      // Log result
      const icon = status === 'pass' ? '  \u2705' :
                   status === 'fail' ? '  \u274C' :
                   status === 'timeout' ? '  \u23F1\uFE0F ' :
                   '  \u{1F4A5}';
      const durationStr = result.duration_ms ? ` (${result.duration_ms}ms)` : '';
      const errorStr = error ? ` - ${error}` : '';

      log(`${icon} ${testName}${durationStr}${errorStr}`);
    }
  } else {
    // No test events parsed - treat as single test for the file
    let status: TestResult['status'];
    let error: string;

    if (timedOut) {
      status = 'timeout';
      error = `Test file timed out after ${TIMEOUT_MS}ms`;
    } else if (crashReason) {
      status = 'crash';
      error = crashReason;
    } else if (exitCode === 0) {
      status = 'pass';
      error = 'No JSONL output but exit code 0';
    } else {
      status = 'fail';
      error = `No test results parsed. Exit code: ${exitCode}`;
    }

    const result: TestResult = {
      test: fileName,
      status,
      duration_ms,
      error,
      stdout: VERBOSE ? stdout : undefined,
      stderr: VERBOSE || status !== 'pass' ? stderr : undefined,
    };

    tests.push(result);

    const icon = status === 'pass' ? '  \u2705' :
                 status === 'fail' ? '  \u274C' :
                 status === 'timeout' ? '  \u23F1\uFE0F ' :
                 '  \u{1F4A5}';

    log(`${icon} ${fileName} (${duration_ms}ms) - ${error}`);
  }

  // Calculate counts
  const passed = tests.filter(t => t.status === 'pass').length;
  const failed = tests.filter(t => t.status === 'fail').length;
  const timeout = tests.filter(t => t.status === 'timeout').length;
  const crashed = tests.filter(t => t.status === 'crash').length;
  // Note: skipped tests are converted to pass with error message
  const skipped = events.filter(e => e.status === 'skip').length;

  return {
    file: fileName,
    tests,
    duration_ms,
    passed,
    failed,
    timeout,
    crashed,
    skipped,
  };
}

// =============================================================================
// Main Entry Point
// =============================================================================

async function main() {
  const startTime = Date.now();

  // Header
  if (!JSON_ONLY) {
    log('\u2554' + '\u2550'.repeat(68) + '\u2557');
    log('\u2551' + '           SCRIPT KIT AUTONOMOUS TEST HARNESS'.padEnd(68) + '\u2551');
    log('\u255A' + '\u2550'.repeat(68) + '\u255D');
    log('');
    log('Configuration:');
    log(`  Binary:           ${BINARY_PATH}`);
    log(`  Timeout:          ${TIMEOUT_MS}ms`);
    log(`  Auto-submit delay: ${AUTO_SUBMIT_DELAY_MS}ms`);
    log(`  Headless:         ${HEADLESS}`);
    log(`  Verbose:          ${VERBOSE}`);
  }

  // Verify binary exists
  if (!await verifyBinary()) {
    process.exit(2);
  }

  // Find test files
  const testFiles = await findTestFiles(testArgs);

  if (testFiles.length === 0) {
    log('\nNo test files found.');
    log('Create tests in tests/autonomous/ directory, e.g.:');
    log('  tests/autonomous/test-core-prompts.ts');
    process.exit(0);
  }

  log(`\nFound ${testFiles.length} test file(s)`);

  // Run tests
  const results: TestFileResult[] = [];

  for (const testFile of testFiles) {
    const result = await runTestFile(testFile);
    results.push(result);

    // Output JSONL for each file result
    if (JSON_ONLY) {
      jsonlLog({
        type: 'file_result',
        ...result,
      });
    }
  }

  // Calculate summary
  const totalPassed = results.reduce((sum, r) => sum + r.passed, 0);
  const totalFailed = results.reduce((sum, r) => sum + r.failed, 0);
  const totalTimeout = results.reduce((sum, r) => sum + r.timeout, 0);
  const totalCrashed = results.reduce((sum, r) => sum + r.crashed, 0);
  const totalSkipped = results.reduce((sum, r) => sum + r.skipped, 0);
  const totalDuration = Date.now() - startTime;

  // Determine exit code
  let exitCode = 0;
  if (totalCrashed > 0) exitCode = 4;
  else if (totalTimeout > 0) exitCode = 3;
  else if (totalFailed > 0) exitCode = 1;

  const summary: HarnessSummary = {
    files: results,
    total_passed: totalPassed,
    total_failed: totalFailed,
    total_timeout: totalTimeout,
    total_crashed: totalCrashed,
    total_skipped: totalSkipped,
    total_duration_ms: totalDuration,
    exit_code: exitCode,
  };

  // Print summary
  if (!JSON_ONLY) {
    log('');
    log('\u2550'.repeat(70));
    log('RESULTS');
    log('\u2550'.repeat(70));
    log(`  Passed:   ${totalPassed}`);
    log(`  Failed:   ${totalFailed}`);
    log(`  Timeout:  ${totalTimeout}`);
    log(`  Crashed:  ${totalCrashed}`);
    log(`  Skipped:  ${totalSkipped}`);
    log(`  Duration: ${totalDuration}ms`);
    log('');

    if (exitCode === 0) {
      log('\u2705 All tests passed!');
    } else {
      log(`\u274C Tests failed (exit code ${exitCode})`);
    }
  }

  // Output final JSONL summary
  if (JSON_ONLY) {
    jsonlLog({
      type: 'summary',
      ...summary,
    });
  }

  process.exit(exitCode);
}

// Run
main().catch(err => {
  console.error('Test harness error:', err);
  process.exit(2);
});

/**
 * Click Simulation Test Utilities
 *
 * This module provides helper functions for simulating mouse clicks
 * via the stdin JSON protocol. It enables automated visual testing
 * of click behaviors without requiring actual user interaction.
 *
 * Usage:
 *   import { simulateClick, waitForRender, captureAndSave } from './test-click-utils';
 *
 *   // Simulate a click at coordinates
 *   await simulateClick(100, 200);
 *
 *   // Wait for UI to update
 *   await waitForRender(500);
 *
 *   // Capture and save a screenshot
 *   const filepath = await captureAndSave('test-result');
 */

import '../../scripts/kit-sdk';

// Node built-ins are available in Bun runtime
// @ts-ignore - Bun provides Node compatibility
import { writeFileSync, mkdirSync, existsSync, mkdtempSync, rmSync } from 'fs';
// @ts-ignore - Bun provides Node compatibility
import { join } from 'path';
// @ts-ignore - Bun provides Node compatibility
import { tmpdir } from 'os';

// =============================================================================
// Types
// =============================================================================

export interface SimulateClickOptions {
  /** X coordinate relative to window (required) */
  x: number;
  /** Y coordinate relative to window (required) */
  y: number;
  /** Mouse button: "left" (default), "right", or "middle" */
  button?: 'left' | 'right' | 'middle';
  /** Timeout in ms to wait for response (default: 5000) */
  timeout?: number;
}

export interface ClickResult {
  success: boolean;
  error?: string;
  requestId: string;
}

export interface ScreenshotResult {
  /** Base64-encoded PNG data */
  data: string;
  /** Width in pixels */
  width: number;
  /** Height in pixels */
  height: number;
}

// =============================================================================
// Private Helpers
// =============================================================================

let requestCounter = 0;

function generateRequestId(): string {
  return `click-${Date.now()}-${++requestCounter}`;
}

/**
 * Send a JSON message to the app's stdin and wait for a response.
 *
 * NOTE: This function relies on the SDK's internal message handling.
 * The app must be configured to handle simulateClick messages.
 */
async function sendClickMessage(options: SimulateClickOptions): Promise<ClickResult> {
  const requestId = generateRequestId();
  const message = {
    type: 'simulateClick',
    requestId,
    x: options.x,
    y: options.y,
    ...(options.button && { button: options.button }),
  };

  // Log for debugging
  console.error(`[CLICK] Sending simulateClick: x=${options.x}, y=${options.y}, button=${options.button || 'left'}`);

  // Send the message via the SDK's internal mechanism
  // The SDK's sendMessage function writes to stdout which the app reads
  // @ts-ignore - globalThis may have sendMessage in SDK context
  if (typeof globalThis.sendMessage === 'function') {
    // @ts-ignore
    globalThis.sendMessage(message);
  } else {
    // Fallback: write directly to stdout as JSONL
    // @ts-ignore - process is available in Bun
    process.stdout.write(JSON.stringify(message) + '\n');
  }

  // For now, we return a pending result
  // TODO: Implement proper response handling when the app supports it
  return {
    success: true,
    requestId,
  };
}

// =============================================================================
// Public API
// =============================================================================

/**
 * Simulate a mouse click at the specified window-relative coordinates.
 *
 * @param x - X coordinate relative to the window's content area
 * @param y - Y coordinate relative to the window's content area
 * @param button - Mouse button: "left" (default), "right", or "middle"
 * @returns Promise that resolves when the click is processed
 *
 * @example
 * ```typescript
 * // Click at coordinates (100, 200)
 * await simulateClick(100, 200);
 *
 * // Right-click at coordinates (150, 300)
 * await simulateClick(150, 300, 'right');
 * ```
 */
export async function simulateClick(
  x: number,
  y: number,
  button: 'left' | 'right' | 'middle' = 'left'
): Promise<ClickResult> {
  return sendClickMessage({ x, y, button });
}

/**
 * Wait for the UI to render after state changes.
 *
 * This is useful after triggering clicks or other interactions
 * to ensure the UI has updated before taking screenshots.
 *
 * @param ms - Milliseconds to wait (default: 100)
 * @returns Promise that resolves after the delay
 *
 * @example
 * ```typescript
 * await simulateClick(100, 200);
 * await waitForRender(300); // Wait 300ms for animation
 * const screenshot = await captureAndSave('after-click');
 * ```
 */
export async function waitForRender(ms: number = 100): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Capture a screenshot and save it to the test-screenshots directory.
 *
 * The screenshot is saved as a PNG file with a timestamp suffix.
 * The directory ./test-screenshots/ is created if it doesn't exist.
 *
 * @param name - Base name for the screenshot file (without extension)
 * @param outputDirectory - Optional explicit destination for isolated fixtures
 * @returns Promise that resolves to the full path of the saved screenshot
 *
 * @example
 * ```typescript
 * // Capture and save a screenshot
 * const filepath = await captureAndSave('login-button-clicked');
 * console.error(`Screenshot saved to: ${filepath}`);
 * ```
 */
export async function captureAndSave(name: string, outputDirectory?: string): Promise<string> {
  // Ensure the screenshots directory exists
  // @ts-ignore - process.cwd() available in Bun
  const screenshotDir = outputDirectory ?? join(process.cwd(), 'test-screenshots');
  if (!existsSync(screenshotDir)) {
    mkdirSync(screenshotDir, { recursive: true });
  }

  // Capture the screenshot using the SDK's captureScreenshot function
  const screenshot = await captureScreenshot();

  // Generate filename with timestamp
  const timestamp = Date.now();
  const filename = `${name}-${timestamp}.png`;
  const filepath = join(screenshotDir, filename);

  // Save the screenshot
  // @ts-ignore - Buffer available in Bun
  const buffer = Buffer.from(screenshot.data, 'base64');
  writeFileSync(filepath, buffer);

  console.error(`[SCREENSHOT] Saved: ${filepath} (${screenshot.width}x${screenshot.height})`);

  return filepath;
}

/**
 * Capture a screenshot and return the raw data.
 *
 * Use this when you need the screenshot data directly without saving.
 *
 * @returns Promise that resolves to the screenshot data
 *
 * @example
 * ```typescript
 * const screenshot = await capture();
 * console.error(`Screenshot: ${screenshot.width}x${screenshot.height}`);
 * ```
 */
export async function capture(): Promise<ScreenshotResult> {
  const screenshot = await captureScreenshot();
  return {
    data: screenshot.data,
    width: screenshot.width,
    height: screenshot.height,
  };
}

/**
 * Click at coordinates and immediately capture a screenshot.
 *
 * This is a convenience function that combines simulateClick,
 * waitForRender, and captureAndSave.
 *
 * @param x - X coordinate
 * @param y - Y coordinate
 * @param screenshotName - Name for the screenshot
 * @param waitMs - Milliseconds to wait after click (default: 300)
 * @returns Promise that resolves to the screenshot filepath
 *
 * @example
 * ```typescript
 * // Click and capture in one call
 * const filepath = await clickAndCapture(100, 200, 'button-clicked');
 * ```
 */
export async function clickAndCapture(
  x: number,
  y: number,
  screenshotName: string,
  waitMs: number = 300
): Promise<string> {
  await simulateClick(x, y);
  await waitForRender(waitMs);
  return captureAndSave(screenshotName);
}

/**
 * Get the window bounds to help calculate click coordinates.
 *
 * @returns Promise that resolves to the window bounds
 *
 * @example
 * ```typescript
 * const bounds = await getWindowBounds();
 * // Click in the center of the window
 * await simulateClick(bounds.width / 2, bounds.height / 2);
 * ```
 */
export async function getWindowBoundsForClick(): Promise<{
  x: number;
  y: number;
  width: number;
  height: number;
}> {
  // Use the SDK's getWindowBounds function
  const bounds = await getWindowBounds();
  return {
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
  };
}

// =============================================================================
// Test Runner Helpers
// =============================================================================

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
  screenshot?: string;
}

/**
 * Log a test result in JSONL format for parsing by test runners.
 */
export function logTestResult(
  name: string,
  status: TestResult['status'],
  extra?: Partial<TestResult>
): void {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra,
  };
  console.log(JSON.stringify(result));
}

/**
 * Run a visual test that captures a screenshot and compares behavior.
 *
 * @param testName - Name of the test
 * @param testFn - Async function that performs the test actions
 * @returns Promise that resolves when the test completes
 *
 * @example
 * ```typescript
 * await runVisualTest('click-list-item', async () => {
 *   // Setup
 *   await arg('Pick', ['Apple', 'Banana', 'Cherry']);
 *   await waitForRender(500);
 *
 *   // Action
 *   await simulateClick(100, 150);
 *   await waitForRender(300);
 *
 *   // Capture
 *   return await captureAndSave('clicked-list-item');
 * });
 * ```
 */
export async function runVisualTest(
  testName: string,
  testFn: () => Promise<string | void>
): Promise<void> {
  logTestResult(testName, 'running');
  const start = Date.now();

  try {
    const result = await testFn();
    logTestResult(testName, 'pass', {
      duration_ms: Date.now() - start,
      screenshot: typeof result === 'string' ? result : undefined,
    });
  } catch (err) {
    logTestResult(testName, 'fail', {
      duration_ms: Date.now() - start,
      error: String(err),
    });
  }
}

// =============================================================================
// Self-Test (when run directly)
// =============================================================================

// @ts-ignore - require.main available in Bun
if (require.main === module) {
  (async () => {
    console.error('[TEST] test-click-utils.ts self-test starting...');

    // Test 1: Check exports exist
    logTestResult('click-utils-exports', 'running');
    const start1 = Date.now();
    try {
      const exportsExist =
        typeof simulateClick === 'function' &&
        typeof waitForRender === 'function' &&
        typeof captureAndSave === 'function' &&
        typeof capture === 'function' &&
        typeof clickAndCapture === 'function' &&
        typeof getWindowBoundsForClick === 'function';

      if (exportsExist) {
        logTestResult('click-utils-exports', 'pass', {
          duration_ms: Date.now() - start1,
          result: 'All exports available',
        });
      } else {
        logTestResult('click-utils-exports', 'fail', {
          duration_ms: Date.now() - start1,
          error: 'Some exports are missing',
        });
      }
    } catch (err) {
      logTestResult('click-utils-exports', 'fail', {
        duration_ms: Date.now() - start1,
        error: String(err),
      });
    }

    // Test 2: waitForRender timing
    logTestResult('click-utils-wait', 'running');
    const start2 = Date.now();
    try {
      const waitStart = Date.now();
      await waitForRender(100);
      const elapsed = Date.now() - waitStart;

      if (elapsed >= 90 && elapsed <= 200) {
        logTestResult('click-utils-wait', 'pass', {
          duration_ms: Date.now() - start2,
          result: `Waited ${elapsed}ms`,
        });
      } else {
        logTestResult('click-utils-wait', 'fail', {
          duration_ms: Date.now() - start2,
          error: `Expected ~100ms, got ${elapsed}ms`,
        });
      }
    } catch (err) {
      logTestResult('click-utils-wait', 'fail', {
        duration_ms: Date.now() - start2,
        error: String(err),
      });
    }

    // Test 3: Screenshot capture (requires running in the app context)
    logTestResult('click-utils-capture', 'running');
    const start3 = Date.now();
    try {
      if (typeof captureScreenshot === 'function') {
        if (process.env.SDK_TEST_AUTOSUBMIT !== '1') {
          throw new Error(
            'Click utility self-tests require isolated SDK_TEST_AUTOSUBMIT=1; native screen capture is forbidden.',
          );
        }
        const fixtureDirectory = mkdtempSync(join(tmpdir(), 'script-kit-sdk-click-utils-'));
        try {
          const filepath = await captureAndSave('self-test', fixtureDirectory);
          logTestResult('click-utils-capture', 'pass', {
            duration_ms: Date.now() - start3,
            result: filepath,
            screenshot: filepath,
          });
        } finally {
          rmSync(fixtureDirectory, { recursive: true, force: true });
        }
      } else {
        logTestResult('click-utils-capture', 'skip', {
          duration_ms: Date.now() - start3,
          error: 'captureScreenshot not available (not running in app context)',
        });
      }
    } catch (err) {
      logTestResult('click-utils-capture', 'fail', {
        duration_ms: Date.now() - start3,
        error: String(err),
      });
    }

    console.error('[TEST] test-click-utils.ts self-test complete');

    // Show completion message
    await div(
      md(`# Click Utils Self-Test Complete

All click utility tests have been executed.

## Exported Functions
- \`simulateClick(x, y, button?)\` - Simulate mouse click
- \`waitForRender(ms?)\` - Wait for UI update
- \`captureAndSave(name)\` - Capture and save screenshot
- \`capture()\` - Get raw screenshot data
- \`clickAndCapture(x, y, name)\` - Click and capture in one call
- \`getWindowBoundsForClick()\` - Get window dimensions

---

*Check the JSONL output for detailed results*

Press Escape or click to exit.`)
    );
  })();
}

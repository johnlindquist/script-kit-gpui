// Test: Visual Bug Report Guideline Compliance Test
//
// Purpose: This test demonstrates the CORRECT visual testing protocol that agents
// MUST follow when a user reports a visual bug with a screenshot.
//
// The Visual Testing Protocol (6 steps):
// 1. CREATE test script that reproduces UI state
// 2. CAPTURE screenshot using captureScreenshot() SDK function
// 3. SAVE the PNG to ./.test-screenshots/
// 4. READ the screenshot file using Read tool - MUST ACTUALLY READ IT
// 5. COMPARE to expected state
// 6. ITERATE until fixed
//
// This test creates a UI with an INTENTIONAL "bug" (missing border on one side)
// and validates that the screenshot capture + save workflow works correctly.
// An agent testing this code MUST READ the saved screenshot to verify the bug.

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync, existsSync, statSync } from 'fs';
import { join } from 'path';

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
  screenshot_path?: string;
  screenshot_size_bytes?: number;
  requires_visual_inspection?: boolean;
  visual_bug_description?: string;
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

console.error('[SMOKE] test-guideline-visual-bug.ts starting...');
console.error('[SMOKE] This test validates the visual bug reporting workflow');

const testName = 'visual-bug-guideline-compliance';
logTest(testName, 'running');
const startTime = Date.now();

try {
  // ============================================================================
  // STEP 1: CREATE UI with an intentional visual bug
  // ============================================================================
  //
  // The "bug" here is: The container has a border on only 3 sides (missing left border)
  // This simulates a real visual bug that can ONLY be detected by looking at a screenshot
  //
  console.error('[SMOKE] Step 1: Creating div with intentional visual bug...');
  console.error('[SMOKE] Bug: Container is missing left border (has top, right, bottom only)');

  const buggyHtml = `
    <div class="p-8 bg-gray-900 min-h-full">
      <h1 class="text-white text-xl mb-4">Visual Bug Report Test</h1>

      <!-- This box has an INTENTIONAL BUG: missing left border -->
      <div
        id="buggy-container"
        style="
          border-top: 3px solid #ef4444;
          border-right: 3px solid #ef4444;
          border-bottom: 3px solid #ef4444;
          border-left: none;
          padding: 16px;
          margin: 20px 0;
          background: #1f2937;
          border-radius: 0;
        "
        class="text-white"
      >
        <p class="font-bold mb-2">Bug: Missing Left Border</p>
        <p class="text-gray-400 text-sm">
          This container should have a red border on ALL 4 sides.
          The left border is intentionally missing to simulate a visual bug.
          An AI agent MUST read the screenshot file to verify this bug exists.
        </p>
      </div>

      <!-- Reference: correct box with all borders -->
      <div
        id="correct-container"
        class="border-2 border-green-500 p-4 mt-4 text-white bg-gray-800"
      >
        <p class="font-bold mb-2">Reference: Correct Styling</p>
        <p class="text-gray-400 text-sm">
          This box has the CORRECT styling with borders on all 4 sides.
        </p>
      </div>

      <div class="mt-6 p-4 bg-yellow-900/30 border border-yellow-500 rounded text-yellow-200 text-sm">
        <strong>Agent Instructions:</strong>
        <ol class="list-decimal list-inside mt-2 space-y-1">
          <li>The screenshot has been saved to ./.test-screenshots/</li>
          <li>You MUST use the Read tool to read the PNG file</li>
          <li>Verify the missing left border on the red box</li>
          <li>Compare to the green reference box</li>
          <li>Claiming verification without reading = ANTI-PATTERN</li>
        </ol>
      </div>
    </div>
  `;

  // Display the buggy UI
  // NOTE: We do NOT await div() here because it blocks until user dismisses
  // Instead, we show the div and immediately proceed to capture screenshot
  div(buggyHtml);

  // Wait for the UI to fully render (div is non-blocking when not awaited)
  console.error('[SMOKE] Waiting for UI to render...');
  await new Promise(resolve => setTimeout(resolve, 1000));

  // ============================================================================
  // STEP 2: CAPTURE screenshot using captureScreenshot() SDK function
  // ============================================================================
  console.error('[SMOKE] Step 2: Capturing screenshot...');

  const screenshot = await captureScreenshot();
  console.error(`[SMOKE] Screenshot captured: ${screenshot.width}x${screenshot.height}`);

  // Validate screenshot was captured
  if (!screenshot.data || screenshot.data.length === 0) {
    throw new Error('Screenshot capture returned empty data');
  }

  if (screenshot.width === 0 || screenshot.height === 0) {
    throw new Error('Screenshot has zero dimensions');
  }

  // ============================================================================
  // STEP 3: SAVE the PNG to ./.test-screenshots/
  // ============================================================================
  console.error('[SMOKE] Step 3: Saving screenshot to ./.test-screenshots/...');

  const screenshotDir = join(process.cwd(), '.test-screenshots');
  mkdirSync(screenshotDir, { recursive: true });

  const timestamp = Date.now();
  const filename = `visual-bug-guideline-${timestamp}.png`;
  const filepath = join(screenshotDir, filename);

  // Decode base64 and write
  const buffer = Buffer.from(screenshot.data, 'base64');
  writeFileSync(filepath, buffer);

  console.error(`[SMOKE] Screenshot saved to: ${filepath}`);

  // Verify the file was saved correctly
  if (!existsSync(filepath)) {
    throw new Error(`Screenshot file was not created at ${filepath}`);
  }

  const fileStats = statSync(filepath);
  console.error(`[SMOKE] Screenshot file size: ${fileStats.size} bytes`);

  if (fileStats.size === 0) {
    throw new Error('Screenshot file is empty (0 bytes)');
  }

  // ============================================================================
  // STEP 4-6: These steps MUST be done by the AGENT reading this test output
  // ============================================================================
  //
  // The agent must:
  // 4. READ the screenshot file at ${filepath} using the Read tool
  // 5. COMPARE the buggy container (red borders, missing left) to the reference (green, all sides)
  // 6. If fixing the bug, ITERATE: modify code, re-run test, capture new screenshot, verify fix
  //
  console.error('[SMOKE] ==================================================');
  console.error('[SMOKE] CRITICAL: Visual verification required!');
  console.error(`[SMOKE] Screenshot path: ${filepath}`);
  console.error('[SMOKE] Agent MUST read this file to verify the visual bug.');
  console.error('[SMOKE] ==================================================');

  // ============================================================================
  // Test assertions
  // ============================================================================

  // Test passes if:
  // 1. Screenshot was captured
  // 2. Screenshot was saved to disk
  // 3. File exists and has content
  //
  // NOTE: The visual verification of the bug itself must be done by reading the PNG

  const duration_ms = Date.now() - startTime;

  logTest(testName, 'pass', {
    result: {
      screenshot_captured: true,
      screenshot_dimensions: `${screenshot.width}x${screenshot.height}`,
      screenshot_saved: true,
      file_size_bytes: fileStats.size,
    },
    screenshot_path: filepath,
    screenshot_size_bytes: fileStats.size,
    duration_ms,
    requires_visual_inspection: true,
    visual_bug_description: 'Container has red border on 3 sides (top, right, bottom) but MISSING left border. Compare to green reference box which has all 4 borders.'
  });

  console.error('[SMOKE] Test completed successfully');
  console.error(`[SMOKE] Duration: ${duration_ms}ms`);
  console.error('');
  console.error('[SMOKE] ╔════════════════════════════════════════════════════════════╗');
  console.error('[SMOKE] ║  VISUAL VERIFICATION REQUIRED                              ║');
  console.error('[SMOKE] ╠════════════════════════════════════════════════════════════╣');
  console.error('[SMOKE] ║  An AI agent testing this MUST:                            ║');
  console.error('[SMOKE] ║  1. Use Read tool to read the PNG file                     ║');
  console.error('[SMOKE] ║  2. Verify the missing left border on red container        ║');
  console.error('[SMOKE] ║  3. NOT claim "verified" without actually reading the PNG  ║');
  console.error('[SMOKE] ╚════════════════════════════════════════════════════════════╝');
  console.error('');
  console.error(`[SMOKE] Screenshot: ${filepath}`);

} catch (error) {
  const duration_ms = Date.now() - startTime;
  const errorMessage = error instanceof Error ? error.message : String(error);

  logTest(testName, 'fail', {
    error: errorMessage,
    duration_ms
  });

  console.error(`[SMOKE] Test FAILED: ${errorMessage}`);
}

// Exit cleanly
process.exit(0);

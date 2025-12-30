// Visual test for header button design improvements
// Tests the Run/Actions buttons and separators in the top-right header area
// @ts-nocheck

import '../../scripts/kit-sdk';

console.error('[SMOKE] Header buttons visual test starting...');

// Use div() which allows us to capture without waiting for input
await div(`
  <div class="p-4 text-center">
    <h2 class="text-lg font-bold mb-2">Header Button Visual Test</h2>
    <p class="text-sm text-gray-400">Verifying Run/Actions button styling</p>
    <p class="text-xs text-gray-500 mt-4">Check the top-right area of the main window for button styling</p>
  </div>
`);

// Wait for UI to render fully
await new Promise(resolve => setTimeout(resolve, 500));

// Capture screenshot
console.error('[SMOKE] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[SMOKE] Screenshot: ${screenshot.width}x${screenshot.height}`);

// Save to test-screenshots directory using Bun's built-in fs
const fs = require('fs');
const path = require('path');
const screenshotDir = path.join(process.cwd(), '.test-screenshots');
fs.mkdirSync(screenshotDir, { recursive: true });

const filename = `header-buttons-visual-${Date.now()}.png`;
const filepath = path.join(screenshotDir, filename);
fs.writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] Saved to: ${filepath}`);
console.error('[SMOKE] Header buttons visual test complete');

process.exit(0);

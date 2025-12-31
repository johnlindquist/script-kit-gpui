// Name: Clipboard Newline Test
// Description: Verifies that clipboard history items with newlines display on single line

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[SMOKE] Clipboard newline handling test starting...');
console.error('[SMOKE] This test verifies that text with newlines displays on a single line');
console.error('[SMOKE] The fix normalizes newlines to spaces before truncating for display');

// Wait for clipboard history view to render
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture screenshot
console.error('[SMOKE] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[SMOKE] Screenshot: ${screenshot.width}x${screenshot.height}`);

// Save to ./.test-screenshots/
const screenshotDir = join(process.cwd(), '.test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

const filename = `clipboard-newlines-${Date.now()}.png`;
const filepath = join(screenshotDir, filename);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] Saved to: ${filepath}`);
console.error('[SMOKE] Test complete - verify list items are single-line without overlap');

// Exit cleanly
process.exit(0);

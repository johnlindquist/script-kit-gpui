// Name: Clipboard Layout Test
// Description: Captures screenshot of clipboard history to verify layout is not smooshed

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[SMOKE] Clipboard layout test starting...');
console.error('[SMOKE] This test verifies that clipboard history list items have proper height/spacing');

// Wait for clipboard history view to render
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture screenshot
console.error('[SMOKE] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[SMOKE] Screenshot: ${screenshot.width}x${screenshot.height}`);

// Save to ./.test-screenshots/
const screenshotDir = join(process.cwd(), '.test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

const filename = `clipboard-layout-${Date.now()}.png`;
const filepath = join(screenshotDir, filename);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] Saved to: ${filepath}`);
console.error('[SMOKE] Test complete - check screenshot for proper list item spacing');

// Exit cleanly
process.exit(0);

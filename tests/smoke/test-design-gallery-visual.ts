// Name: Design Gallery Visual Test
// Description: Captures screenshot of design gallery for visual verification

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[SMOKE] Design gallery visual test starting...');

// The design gallery should already be open when this runs via triggerBuiltin
// Wait a moment for rendering
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture screenshot
console.error('[SMOKE] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[SMOKE] Screenshot: ${screenshot.width}x${screenshot.height}`);

// Save to ./.test-screenshots/
const screenshotDir = join(process.cwd(), '.test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

const filename = `design-gallery-${Date.now()}.png`;
const filepath = join(screenshotDir, filename);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] Saved to: ${filepath}`);

// Exit cleanly
process.exit(0);

// Name: Design Gallery Screenshot Capture
// Description: Captures a screenshot of the design gallery

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

// Wait for rendering
await new Promise(resolve => setTimeout(resolve, 800));

// Capture screenshot
const screenshot = await captureScreenshot();

// Save to ./.test-screenshots/
const screenshotDir = join(process.cwd(), '.test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

const filepath = join(screenshotDir, 'design-gallery-manual.png');
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] ${filepath}`);
process.exit(0);

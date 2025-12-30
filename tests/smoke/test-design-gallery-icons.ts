// Name: Design Gallery Icons Screenshot
// Description: Scrolls to icons section and captures screenshot

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

// Wait for gallery to load
await new Promise(resolve => setTimeout(resolve, 500));

// Type "icon" to filter to just icons
await setFilterText("icon");

// Wait for filter to apply
await new Promise(resolve => setTimeout(resolve, 500));

// Capture screenshot
const screenshot = await captureScreenshot();

// Save to ./.test-screenshots/
const screenshotDir = join(process.cwd(), '.test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

const filepath = join(screenshotDir, 'design-gallery-icons.png');
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] ${filepath}`);
process.exit(0);

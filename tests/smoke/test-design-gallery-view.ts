// Test Design Gallery view rendering
// This test triggers the Design Gallery built-in and captures a screenshot

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Design Gallery View Test Starting');

// Wait for app initialization
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture initial state screenshot
console.error('[TEST] Capturing screenshot of Design Gallery view');
try {
  const screenshot = await captureScreenshot();
  console.error(`[TEST] Screenshot captured: ${screenshot.width}x${screenshot.height}`);

  const screenshotDir = join(process.cwd(), '.test-screenshots');
  mkdirSync(screenshotDir, { recursive: true });

  const filename = `design-gallery-view-${Date.now()}.png`;
  const filepath = join(screenshotDir, filename);
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[TEST] Screenshot saved to: ${filepath}`);
} catch (err) {
  console.error(`[TEST] Screenshot error: ${err}`);
}

console.error('[TEST] Test complete');
process.exit(0);

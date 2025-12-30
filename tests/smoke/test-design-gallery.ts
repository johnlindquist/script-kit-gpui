// Test Design Gallery rendering
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Starting Design Gallery test');

// First wait for app to initialize
await new Promise(resolve => setTimeout(resolve, 500));

// Type "design" to filter to the Design Gallery option
console.error('[TEST] Typing "design" to filter');

// We need to use the arg() with choices to simulate the main menu
// but actually we need to just navigate to Design Gallery
// The built-in is at index based on where it appears

// Actually let's just test that arg works first
console.error('[TEST] Calling arg with choices including Design Gallery entry');

const choices = [
  { name: "Design Gallery", value: "design-gallery", description: "Browse separator and icon variations" },
  { name: "Other Option", value: "other", description: "Another choice" }
];

// This will show an arg prompt - we need to capture screenshot before interaction
setTimeout(async () => {
  console.error('[TEST] Capturing screenshot after 1s');
  try {
    const screenshot = await captureScreenshot();
    console.error(`[TEST] Screenshot captured: ${screenshot.width}x${screenshot.height}`);

    const screenshotDir = join(process.cwd(), '.test-screenshots');
    mkdirSync(screenshotDir, { recursive: true });

    const filename = `design-gallery-${Date.now()}.png`;
    const filepath = join(screenshotDir, filename);
    writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
    console.error(`[TEST] Screenshot saved to: ${filepath}`);
  } catch (err) {
    console.error(`[TEST] Screenshot error: ${err}`);
  }

  // Exit after screenshot
  process.exit(0);
}, 1500);

const result = await arg("Select option", choices);
console.error(`[TEST] Selected: ${result}`);

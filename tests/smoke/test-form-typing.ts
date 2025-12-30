// Name: Form Typing Test
// Description: Tests that typing in form fields works correctly

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[SMOKE] test-form-typing.ts starting...');

// Simple form with one text field
const formHtml = `
<div class="p-4 space-y-4">
  <h2 class="text-lg font-bold mb-4">Test Form</h2>

  <div class="space-y-2">
    <label for="name" class="block text-sm font-medium">Name</label>
    <input type="text" name="name" id="name" placeholder="Type your name here" class="w-full px-4 py-2 border rounded" />
  </div>
</div>
`;

console.error('[SMOKE] Starting form with simple text field...');

// Start the form prompt
const formPromise = form(formHtml);

// Wait for the UI to render
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture screenshot before any typing
console.error('[SMOKE] Capturing initial screenshot...');
try {
  const screenshot1 = await captureScreenshot();
  console.error(`[SMOKE] Initial screenshot: ${screenshot1.width}x${screenshot1.height}`);

  const screenshotDir = join(process.cwd(), '.test-screenshots');
  mkdirSync(screenshotDir, { recursive: true });

  const filepath1 = join(screenshotDir, `form-typing-initial-${Date.now()}.png`);
  writeFileSync(filepath1, Buffer.from(screenshot1.data, 'base64'));
  console.error(`[SCREENSHOT] Initial: ${filepath1}`);
} catch (err) {
  console.error('[SMOKE] Initial screenshot failed:', err);
}

console.error('[SMOKE] Form displayed. Please type some characters to test input.');
console.error('[SMOKE] The test will capture a screenshot in 5 seconds...');

// Wait for user to type (or for automated test to simulate typing)
await new Promise(resolve => setTimeout(resolve, 5000));

// Capture screenshot after typing
console.error('[SMOKE] Capturing screenshot after typing window...');
try {
  const screenshot2 = await captureScreenshot();
  console.error(`[SMOKE] Post-typing screenshot: ${screenshot2.width}x${screenshot2.height}`);

  const screenshotDir = join(process.cwd(), '.test-screenshots');
  const filepath2 = join(screenshotDir, `form-typing-after-${Date.now()}.png`);
  writeFileSync(filepath2, Buffer.from(screenshot2.data, 'base64'));
  console.error(`[SCREENSHOT] After typing: ${filepath2}`);
} catch (err) {
  console.error('[SMOKE] Post-typing screenshot failed:', err);
}

console.error('[SMOKE] test-form-typing.ts completed - now awaiting form result...');

// Actually await the form to keep the process alive and allow typing
try {
  const result = await formPromise;
  console.error('[SMOKE] Form result:', JSON.stringify(result));
} catch (err) {
  console.error('[SMOKE] Form error:', err);
}

process.exit(0);

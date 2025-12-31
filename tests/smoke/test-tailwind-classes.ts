// Name: SDK Test - Tailwind CSS Classes
// Description: Tests Tailwind CSS class parsing and styling

import '../../scripts/kit-sdk';

console.error('[TAILWIND-TEST] Starting Tailwind CSS class test...');

// Test: Basic Tailwind classes with colors and spacing
const testHtml = `
<div class="flex flex-col gap-4 p-4">
  <div class="p-4 bg-blue-500 text-white rounded-lg">
    <h1 class="text-2xl font-bold">Tailwind Test</h1>
    <p class="mt-2 text-sm">This should have blue background and white text</p>
  </div>
  
  <div class="p-4 bg-green-500 text-white rounded-lg">
    <p class="font-bold">Green Box</p>
  </div>
  
  <div class="flex flex-row gap-2">
    <div class="p-2 bg-red-500 text-white rounded">Red</div>
    <div class="p-2 bg-yellow-500 text-black rounded">Yellow</div>
    <div class="p-2 bg-purple-500 text-white rounded">Purple</div>
  </div>
  
  <div class="p-4 bg-gray-800 text-gray-100 rounded-xl border">
    <p class="text-lg font-medium">Gray Box with Border</p>
    <span class="text-gray-400">Muted text</span>
  </div>
</div>
`;

// Display the test content
console.error('[TAILWIND-TEST] Displaying div...');
await div(testHtml);
console.error('[TAILWIND-TEST] Div displayed');

// Wait for render
console.error('[TAILWIND-TEST] Waiting for render...');
await new Promise(resolve => setTimeout(resolve, 2000));

// Capture screenshot
console.error('[TAILWIND-TEST] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[TAILWIND-TEST] Screenshot captured: ${screenshot.width}x${screenshot.height}`);

// Save screenshot
const fs = await import('fs');
const path = await import('path');
const screenshotDir = path.join(process.cwd(), 'test-screenshots');
fs.mkdirSync(screenshotDir, { recursive: true });

const filename = `tailwind-test-${Date.now()}.png`;
const filepath = path.join(screenshotDir, filename);
fs.writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[TAILWIND-TEST] Screenshot saved to: ${filepath}`);
console.error('[TAILWIND-TEST] Test complete');

process.exit(0);

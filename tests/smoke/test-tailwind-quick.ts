// Quick Tailwind test - minimal version
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TAILWIND] Starting quick Tailwind test...');

// Display Tailwind-styled content
const testHtml = `
<div class="flex flex-col gap-4 p-4">
  <div class="p-4 bg-blue-500 text-white rounded-lg">
    <p class="font-bold text-xl">Blue Box</p>
  </div>
  <div class="p-4 bg-green-500 text-white rounded-lg">
    <p class="font-bold">Green Box</p>
  </div>
  <div class="flex flex-row gap-2">
    <div class="p-2 bg-red-500 text-white rounded">Red</div>
    <div class="p-2 bg-yellow-500 text-black rounded">Yellow</div>
  </div>
</div>
`;

// Show div
const divPromise = div(testHtml);

// Wait for render
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture screenshot
const screenshot = await captureScreenshot();
console.error(`[TAILWIND] Captured: ${screenshot.width}x${screenshot.height}`);

// Save screenshot
const dir = join(process.cwd(), 'test-screenshots');
mkdirSync(dir, { recursive: true });
const filepath = join(dir, `tailwind-quick-${Date.now()}.png`);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

process.exit(0);

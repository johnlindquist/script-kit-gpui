// Name: Test Div Actions Visual
// Description: Visual test for div() with actions panel

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Starting div actions visual test');

const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

// Define actions for the div prompt
const actions = [
  { name: 'Refresh', shortcut: 'cmd+r' },
  { name: 'Copy HTML', shortcut: 'cmd+c' },
  { name: 'Close', shortcut: 'escape' },
];

// Show div with actions
console.error('[TEST] Showing div with actions...');

// Use setTimeout to capture screenshot after render
setTimeout(async () => {
  try {
    console.error('[TEST] Capturing screenshot...');
    const screenshot = await captureScreenshot();
    console.error(`[TEST] Screenshot: ${screenshot.width}x${screenshot.height}`);
    
    const filepath = join(screenshotDir, `div-actions-${Date.now()}.png`);
    writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
    console.error(`[SCREENSHOT] ${filepath}`);
    
    // Exit after screenshot
    process.exit(0);
  } catch (err) {
    console.error('[TEST] Screenshot error:', err);
    process.exit(1);
  }
}, 1500);

await div(
  `<div class="p-4">
    <h1 class="text-2xl font-bold mb-4">Div with Actions</h1>
    <p class="text-gray-400">Press Cmd+K to see available actions</p>
    <p class="text-gray-500 mt-2">Actions button should appear in header</p>
  </div>`,
  undefined,
  actions
);

// Name: Test Div Actions Panel
// Description: Tests div() with actions and Cmd+K toggle

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Starting div actions test');

// Create screenshot directory
const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

// Define actions for the div prompt
const actions = [
  { name: 'Refresh', shortcut: 'cmd+r', onAction: () => console.error('[ACTION] Refresh triggered') },
  { name: 'Copy HTML', shortcut: 'cmd+c', onAction: () => console.error('[ACTION] Copy HTML triggered') },
  { name: 'Close', shortcut: 'escape', onAction: () => console.error('[ACTION] Close triggered') },
];

console.error('[TEST] Showing div prompt with actions...');

// Set up a delayed screenshot capture
setTimeout(async () => {
  console.error('[TEST] Capturing delayed screenshot (2s after div shown)...');
  try {
    const screenshot = await captureScreenshot();
    console.error(`[TEST] Screenshot captured: ${screenshot.width}x${screenshot.height}`);
    const filepath = join(screenshotDir, `div-actions-${Date.now()}.png`);
    writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
    console.error(`[SCREENSHOT] Saved to: ${filepath}`);
  } catch (err) {
    console.error('[TEST] Screenshot error:', err);
  }
}, 2000);

// Show div with actions - using the new API with actions as third parameter
await div(
  `<div class="p-4">
    <h1 class="text-2xl font-bold mb-4">Div with Actions</h1>
    <p class="text-gray-400">Press Cmd+K to see available actions</p>
    <ul class="mt-4 list-disc list-inside">
      <li>Refresh (Cmd+R)</li>
      <li>Copy HTML (Cmd+C)</li>
      <li>Close (Escape)</li>
    </ul>
  </div>`,
  undefined, // tailwind classes
  actions    // actions array
);

console.error('[TEST] Div completed');
console.error('[TEST] Test complete');

process.exit(0);

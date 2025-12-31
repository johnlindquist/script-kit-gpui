// Name: Test Editor Actions Visual
// Description: Visual test for editor() with actions panel

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Starting editor actions visual test');

const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

// Define actions for the editor prompt
const actions = [
  { name: 'Format Code', shortcut: 'cmd+shift+f' },
  { name: 'Save Draft', shortcut: 'cmd+s' },
  { name: 'Clear', shortcut: 'cmd+shift+c' },
];

// Use setTimeout to capture screenshot after render
setTimeout(async () => {
  try {
    console.error('[TEST] Capturing screenshot...');
    const screenshot = await captureScreenshot();
    console.error(`[TEST] Screenshot: ${screenshot.width}x${screenshot.height}`);
    
    const filepath = join(screenshotDir, `editor-actions-${Date.now()}.png`);
    writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
    console.error(`[SCREENSHOT] ${filepath}`);
    
    process.exit(0);
  } catch (err) {
    console.error('[TEST] Screenshot error:', err);
    process.exit(1);
  }
}, 1500);

console.error('[TEST] Showing editor with actions...');

// Show editor with actions
await editor(
  `function hello() {
  console.log("Hello, World!");
}`,
  "typescript",
  actions
);

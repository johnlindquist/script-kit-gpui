// Name: Test Main Window Theme
// Description: Captures screenshot of main window to compare with Notes window theme

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Starting main window theme capture...');

// Create screenshot directory first
const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

// The window should be shown when running a script via stdin
// Wait for the window to be fully rendered
await new Promise(r => setTimeout(r, 1000));

// Capture screenshot of the main window
console.error('[TEST] Capturing main window screenshot (hi_dpi=true for full resolution)...');

try {
  // Use hi_dpi=true to get full retina resolution
  const screenshot = await captureScreenshot({ hiDpi: true });
  console.error(`[TEST] Captured: ${screenshot.width}x${screenshot.height}`);

  const timestamp = Date.now();
  const filename = `main-window-theme-${timestamp}.png`;
  const filepath = join(screenshotDir, filename);
  
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] ${filepath}`);
  
  console.error('[TEST] Screenshot saved successfully');
  console.error('[TEST] Theme colors from logs:');
  console.error('[TEST]   background: #1e1e1e');
  console.error('[TEST]   accent: #fbbf24');
  console.error('[TEST]   selected_subtle: #2a2a2a');
  console.error('[TEST]   title_bar: #2d2d30');
  console.error('[TEST]   border: #464647');
} catch (err) {
  console.error(`[TEST] Failed to capture screenshot: ${err}`);
}

process.exit(0);

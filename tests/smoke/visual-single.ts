// Name: Visual Single Test
// Description: Captures a single screenshot to verify the screenshot flow works

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Visual Single Test",
  description: "Single screenshot capture test",
};

const screenshotDir = join(process.cwd(), '.test-screenshots', 'visual-single');
mkdirSync(screenshotDir, { recursive: true });

console.error('[TEST] Starting single screenshot test...');

// Show a div without awaiting it
div(`
  <div style="padding: 24px; font-family: system-ui;">
    <h1 style="color: #3b82f6; margin: 0 0 12px 0; font-size: 24px;">Screenshot Test</h1>
    <p style="color: #9ca3af;">Testing screenshot capture functionality</p>
  </div>
`);

console.error('[TEST] div() called, waiting for render...');

// Wait for render
await new Promise(r => setTimeout(r, 800));

console.error('[TEST] Render wait complete, calling captureScreenshot...');

try {
  const ss = await captureScreenshot();
  console.error(`[TEST] Got screenshot: ${ss.width}x${ss.height}, data length: ${ss.data.length}`);
  
  if (ss.data.length > 0) {
    const filepath = join(screenshotDir, 'test-screenshot.png');
    writeFileSync(filepath, Buffer.from(ss.data, 'base64'));
    console.error(`[SUCCESS] Screenshot saved to: ${filepath}`);
    console.log(JSON.stringify({ status: 'success', path: filepath, width: ss.width, height: ss.height }));
  } else {
    console.error('[ERROR] Screenshot data is empty');
    console.log(JSON.stringify({ status: 'error', error: 'Empty screenshot data' }));
  }
} catch (err) {
  console.error(`[ERROR] Failed to capture screenshot: ${err}`);
  console.log(JSON.stringify({ status: 'error', error: String(err) }));
}

// Exit
console.error('[TEST] Exiting...');
process.exit(0);

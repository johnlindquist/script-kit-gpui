// Name: Test Transitions Showcase
// Description: Visual test showcasing hover effects, toasts, and list item interactions

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

async function captureAndSave(name: string, description: string): Promise<string> {
  console.error(`[TEST] Capturing: ${name} - ${description}`);
  await new Promise(r => setTimeout(r, 300)); // Wait for render
  
  const screenshot = await captureScreenshot({ hiDpi: true });
  console.error(`[TEST] Captured: ${screenshot.width}x${screenshot.height}`);
  
  const timestamp = Date.now();
  const filename = `${name}-${timestamp}.png`;
  const filepath = join(screenshotDir, filename);
  
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] ${filepath}`);
  return filepath;
}

console.error('[TEST] === TRANSITIONS SHOWCASE TEST ===');

// Test 1: List items with multiple scripts showing selection styling
console.error('\n[TEST] === TEST 1: List Item Selection Styles ===');

// Show arg prompt with multiple choices to demonstrate selection styling
const fruits = ['Apple', 'Banana', 'Cherry', 'Date', 'Elderberry', 'Fig', 'Grape'];
console.error('[TEST] Displaying arg prompt with choices...');

// We'll use a timeout to capture before user interacts
setTimeout(async () => {
  await captureAndSave('list-selection', 'List items showing selection styling');
  process.exit(0);
}, 1500);

// Show the arg prompt - this will display list items
await arg('Select a fruit (observe hover/selection styling):', fruits.map(f => ({
  name: f,
  value: f.toLowerCase(),
  description: `A delicious ${f.toLowerCase()}`
})));

process.exit(0);

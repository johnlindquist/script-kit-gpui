// Name: Test List Hover Visual
// Description: Captures list items to verify hover styling is applied

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

async function captureAndSave(name: string): Promise<string> {
  await new Promise(r => setTimeout(r, 500));
  const screenshot = await captureScreenshot({ hiDpi: true });
  const filepath = join(screenshotDir, `${name}-${Date.now()}.png`);
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] ${filepath}`);
  return filepath;
}

console.error('[TEST] === LIST HOVER VISUAL TEST ===');
console.error('[TEST] This test displays list items to verify:');
console.error('[TEST]   1. Selected item has accent background (selected_subtle color)');
console.error('[TEST]   2. Non-selected items have transparent background');
console.error('[TEST]   3. Hover effect shows subtle highlight (via GPUI .hover() modifier)');

// Create a list with various items to show styling
const items = [
  { name: 'First Item', value: '1', description: 'This item should be selected (highlighted)' },
  { name: 'Second Item', value: '2', description: 'Hover over this to see hover effect' },
  { name: 'Third Item', value: '3', description: 'Another item to hover' },
  { name: 'Fourth Item', value: '4', description: 'Keyboard navigation test' },
  { name: 'Fifth Item', value: '5', description: 'Last item in list' },
];

// Capture after a delay
setTimeout(async () => {
  const path = await captureAndSave('list-hover-visual');
  console.error(`[TEST] List hover visual captured: ${path}`);
  console.error('[TEST] Verify in screenshot:');
  console.error('[TEST]   - First item has subtle accent background');
  console.error('[TEST]   - Other items have no background');
  process.exit(0);
}, 1000);

await arg({
  placeholder: 'Use arrow keys to change selection, observe styling',
  hint: 'Press Escape to exit',
}, items);

process.exit(0);

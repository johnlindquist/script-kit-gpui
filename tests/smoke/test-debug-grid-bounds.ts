import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Debug Grid - Bounds Test",
  description: "Tests component bounding boxes",
};

console.error('[TEST] Starting debug grid bounds test...');

await div(`<div class="p-8">
  <h1 class="text-xl mb-4">Bounds Test</h1>
  <p>Component bounds should be visible.</p>
</div>`);

await showGrid({ showBounds: true, gridSize: 8 });
await new Promise(r => setTimeout(r, 500));

const screenshot = await captureScreenshot();
const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });
const filepath = join(dir, `debug-grid-bounds-${Date.now()}.png`);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

await hideGrid();
process.exit(0);

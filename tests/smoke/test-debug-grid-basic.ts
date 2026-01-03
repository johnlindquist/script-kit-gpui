import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Debug Grid - Basic Test",
  description: "Tests basic grid overlay with grid lines",
};

console.error('[TEST] Starting debug grid basic test...');

await div(`<div class="p-8 text-white">
  <h1 class="text-2xl font-bold mb-4">Grid Overlay Test</h1>
  <p>The grid should appear over this content.</p>
</div>`);

await showGrid({ gridSize: 16 });
await new Promise(r => setTimeout(r, 500));

const screenshot = await captureScreenshot();
console.error(`[TEST] Captured: ${screenshot.width}x${screenshot.height}`);

const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });
const filepath = join(dir, `debug-grid-basic-${Date.now()}.png`);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

await hideGrid();
process.exit(0);

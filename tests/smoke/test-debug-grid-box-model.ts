import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Debug Grid - Box Model Test",
  description: "Tests padding/margin visualization",
};

console.error('[TEST] Starting debug grid box model test...');

await div(`<div class="p-8 m-4 bg-blue-500 text-white rounded-lg">
  <div class="p-4 bg-blue-700 rounded">
    <h1 class="text-xl font-bold">Box Model Test</h1>
    <p class="mt-2">Padding in green, margin in orange.</p>
  </div>
</div>`);

await showGrid({ showBounds: true, showBoxModel: true, gridSize: 8 });
await new Promise(r => setTimeout(r, 500));

const screenshot = await captureScreenshot();
const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });
const filepath = join(dir, `debug-grid-box-model-${Date.now()}.png`);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

await hideGrid();
process.exit(0);

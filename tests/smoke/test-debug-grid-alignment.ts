import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Debug Grid - Alignment Test",
  description: "Tests alignment guides",
};

console.error('[TEST] Starting debug grid alignment test...');

await div(`<div class="flex flex-col gap-4 p-8">
  <div class="flex gap-4">
    <div class="w-32 h-16 bg-red-500 rounded"></div>
    <div class="w-32 h-16 bg-green-500 rounded"></div>
    <div class="w-32 h-16 bg-blue-500 rounded"></div>
  </div>
  <div class="flex gap-4">
    <div class="w-32 h-16 bg-yellow-500 rounded"></div>
    <div class="w-32 h-16 bg-purple-500 rounded"></div>
    <div class="w-32 h-16 bg-pink-500 rounded"></div>
  </div>
</div>`);

await showGrid({ showBounds: true, showAlignmentGuides: true, gridSize: 8 });
await new Promise(r => setTimeout(r, 500));

const screenshot = await captureScreenshot();
const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });
const filepath = join(dir, `debug-grid-alignment-${Date.now()}.png`);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

await hideGrid();
process.exit(0);

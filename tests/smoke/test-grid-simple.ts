import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[GRID] Starting simple grid test...');

// Use setPanel which doesn't wait for response
setPanel(`<div class="p-8 text-white"><h1>Grid Test</h1><div class="flex gap-4"><div class="w-20 h-12 bg-blue-500">A</div><div class="w-20 h-12 bg-green-500">B</div></div></div>`);
console.error('[GRID] setPanel called');

// Wait a moment for render
await new Promise(r => setTimeout(r, 300));
console.error('[GRID] Delay done');

// Enable grid
console.error('[GRID] Calling showGrid...');
await showGrid({ gridSize: 16, showBounds: true });
console.error('[GRID] showGrid called');

// Wait for render  
await new Promise(r => setTimeout(r, 500));
console.error('[GRID] Grid should be visible');

// Capture screenshot
console.error('[GRID] Capturing...');
const shot = await captureScreenshot();
console.error(`[GRID] Got ${shot.width}x${shot.height}`);

const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });
const file = join(dir, `grid-simple-${Date.now()}.png`);
writeFileSync(file, Buffer.from(shot.data, 'base64'));
console.error(`[GRID] Saved: ${file}`);

console.error('[GRID] Done!');
process.exit(0);

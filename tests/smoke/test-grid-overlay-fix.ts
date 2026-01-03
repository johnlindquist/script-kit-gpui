import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Test Grid Overlay Fix",
  description: "Tests that grid overlay shows correct bounds for DivPrompt",
};

const dir = join(process.cwd(), '.test-screenshots', 'grid-audit');
mkdirSync(dir, { recursive: true });

console.error('[TEST] Starting grid overlay fix test...');
console.error('[TEST] Displaying div prompt...');

// Display div content - don't await since we want to capture while it's showing
setTimeout(async () => {
  console.error('[TEST] Waiting 2s for render...');
  await new Promise(r => setTimeout(r, 2000));
  
  console.error('[TEST] Capturing screenshot...');
  try {
    const ss = await captureScreenshot();
    console.error(`[TEST] Screenshot size: ${ss.width}x${ss.height}`);
    
    const filepath = join(dir, 'test-div-grid-fix.png');
    writeFileSync(filepath, Buffer.from(ss.data, 'base64'));
    console.error(`[TEST] Screenshot saved: ${filepath}`);
  } catch (err) {
    console.error('[TEST] Screenshot failed:', err);
  }
  
  process.exit(0);
}, 500);

// Display the div
await div(`
  <div class="p-8 flex flex-col gap-4">
    <h1 class="text-3xl font-bold text-white">Div Prompt Test</h1>
    <p class="text-lg text-gray-300">This is testing the grid overlay for DivPrompt</p>
    <p class="text-sm text-gray-500">The grid should show "DivContent" not "ScriptList"</p>
    <div class="mt-4 p-4 bg-blue-600 rounded-lg">
      <p class="text-white">Blue box content</p>
    </div>
  </div>
`);

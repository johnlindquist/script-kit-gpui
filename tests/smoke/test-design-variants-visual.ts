// Name: Test Design Variants Visual
// Description: Captures screenshots of different design system variants

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

console.error('[TEST] === DESIGN VARIANTS VISUAL TEST ===');

// Show a rich UI with various elements that use hover styling
await div(`
  <div class="flex flex-col gap-4 p-6 min-h-[400px]">
    <h2 class="text-xl font-bold text-white">Design System Elements</h2>
    
    <!-- Buttons with hover -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400 uppercase tracking-wide">Buttons (hover for effect)</h3>
      <div class="flex gap-2">
        <button class="px-4 py-2 bg-yellow-500 text-black rounded font-medium hover:bg-yellow-400 transition-colors">
          Primary
        </button>
        <button class="px-4 py-2 bg-gray-700 text-white rounded font-medium hover:bg-gray-600 transition-colors">
          Secondary
        </button>
        <button class="px-4 py-2 border border-gray-600 text-gray-300 rounded font-medium hover:bg-gray-800 transition-colors">
          Outline
        </button>
      </div>
    </div>
    
    <!-- List Items -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400 uppercase tracking-wide">List Items (GPUI hover effect)</h3>
      <div class="flex flex-col rounded-lg overflow-hidden border border-gray-700">
        <div class="p-3 bg-yellow-500/20 border-l-3 border-l-yellow-500 flex items-center gap-3">
          <span class="text-lg">📜</span>
          <div>
            <div class="text-white font-medium">Selected Item</div>
            <div class="text-gray-400 text-sm">This item is currently selected</div>
          </div>
        </div>
        <div class="p-3 hover:bg-white/5 flex items-center gap-3 cursor-pointer">
          <span class="text-lg">📁</span>
          <div>
            <div class="text-white font-medium">Hover Me</div>
            <div class="text-gray-400 text-sm">This item has hover effect</div>
          </div>
        </div>
        <div class="p-3 hover:bg-white/5 flex items-center gap-3 cursor-pointer">
          <span class="text-lg">⚡</span>
          <div>
            <div class="text-white font-medium">Another Item</div>
            <div class="text-gray-400 text-sm">Standard list item styling</div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Transition demo -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400 uppercase tracking-wide">Transition Module Capabilities</h3>
      <div class="bg-gray-800 rounded-lg p-4 text-sm font-mono text-gray-300">
        <div>• TransitionColor - RGBA interpolation</div>
        <div>• Opacity - 0.0 to 1.0 fade</div>
        <div>• SlideOffset - X/Y movement</div>
        <div>• AppearTransition - Combined fade + slide</div>
        <div>• Easing: linear, ease_out_quad, ease_in_quad</div>
      </div>
    </div>
  </div>
`);

await captureAndSave('design-variants');

process.exit(0);

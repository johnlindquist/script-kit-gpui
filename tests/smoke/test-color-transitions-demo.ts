// Name: Test Color Transitions Demo
// Description: Visual demonstration of color transition capabilities

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

console.error('[TEST] === COLOR TRANSITIONS DEMO ===');
console.error('[TEST] The transitions module provides:');
console.error('[TEST]   - TransitionColor: Interpolates between colors');
console.error('[TEST]   - Lerp trait: Linear interpolation for any value');
console.error('[TEST]   - Easing functions: Smooth animation curves');

// Create a visual demo showing color gradients that represent transitions
await div(`
  <div class="flex flex-col gap-6 p-6 min-h-[500px]">
    <h2 class="text-xl font-bold text-white">Color Transition Demonstration</h2>
    
    <!-- Gradient showing color interpolation -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400">TransitionColor.lerp() - Color Interpolation</h3>
      <div class="flex h-12 rounded-lg overflow-hidden">
        <div class="flex-1 bg-transparent"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.1)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.2)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.3)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.4)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.5)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.6)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.7)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.8)"></div>
        <div class="flex-1" style="background: rgba(251, 191, 36, 0.9)"></div>
        <div class="flex-1 bg-yellow-500"></div>
      </div>
      <div class="flex justify-between text-xs text-gray-500">
        <span>t=0.0</span>
        <span>t=0.5</span>
        <span>t=1.0</span>
      </div>
    </div>
    
    <!-- Easing curve visualizations -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400">Easing Functions</h3>
      <div class="grid grid-cols-3 gap-4">
        <!-- Linear -->
        <div class="bg-gray-800 rounded-lg p-3">
          <div class="text-xs text-gray-400 mb-2">linear(t)</div>
          <div class="h-20 relative border-l border-b border-gray-600">
            <div class="absolute bottom-0 left-0 w-full h-full" style="background: linear-gradient(45deg, transparent 49%, #fbbf24 49%, #fbbf24 51%, transparent 51%)"></div>
          </div>
        </div>
        
        <!-- Ease Out Quad -->
        <div class="bg-gray-800 rounded-lg p-3">
          <div class="text-xs text-gray-400 mb-2">ease_out_quad(t)</div>
          <div class="h-20 relative border-l border-b border-gray-600 flex items-end">
            <svg viewBox="0 0 100 100" class="w-full h-full" preserveAspectRatio="none">
              <path d="M0,100 Q50,0 100,0" stroke="#fbbf24" fill="none" stroke-width="2"/>
            </svg>
          </div>
        </div>
        
        <!-- Ease In Quad -->
        <div class="bg-gray-800 rounded-lg p-3">
          <div class="text-xs text-gray-400 mb-2">ease_in_quad(t)</div>
          <div class="h-20 relative border-l border-b border-gray-600 flex items-end">
            <svg viewBox="0 0 100 100" class="w-full h-full" preserveAspectRatio="none">
              <path d="M0,100 Q50,100 100,0" stroke="#fbbf24" fill="none" stroke-width="2"/>
            </svg>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Opacity transitions -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400">Opacity Transition States</h3>
      <div class="flex gap-3">
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-16 bg-yellow-500 rounded-lg opacity-0"></div>
          <span class="text-xs text-gray-500">0%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-16 bg-yellow-500 rounded-lg opacity-25"></div>
          <span class="text-xs text-gray-500">25%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-16 bg-yellow-500 rounded-lg opacity-50"></div>
          <span class="text-xs text-gray-500">50%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-16 bg-yellow-500 rounded-lg opacity-75"></div>
          <span class="text-xs text-gray-500">75%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-16 bg-yellow-500 rounded-lg opacity-100"></div>
          <span class="text-xs text-gray-500">100%</span>
        </div>
      </div>
    </div>
    
    <!-- AppearTransition (fade + slide) -->
    <div class="flex flex-col gap-2">
      <h3 class="text-sm text-gray-400">AppearTransition (Opacity + Slide)</h3>
      <div class="flex gap-3 items-end h-24">
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-12 bg-blue-500 rounded-lg opacity-0 translate-y-4"></div>
          <span class="text-xs text-gray-500">hidden</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-12 bg-blue-500 rounded-lg opacity-25 translate-y-3"></div>
          <span class="text-xs text-gray-500">25%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-12 bg-blue-500 rounded-lg opacity-50 translate-y-2"></div>
          <span class="text-xs text-gray-500">50%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-12 bg-blue-500 rounded-lg opacity-75 translate-y-1"></div>
          <span class="text-xs text-gray-500">75%</span>
        </div>
        <div class="flex flex-col items-center gap-1">
          <div class="w-16 h-12 bg-blue-500 rounded-lg opacity-100 translate-y-0"></div>
          <span class="text-xs text-gray-500">visible</span>
        </div>
      </div>
    </div>
  </div>
`);

await captureAndSave('color-transitions-demo');

process.exit(0);

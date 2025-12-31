// Comprehensive Tailwind CSS Test - Tests ALL supported Tailwind classes
// Captures screenshots for visual verification of styling
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const screenshotDir = join(process.cwd(), 'test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

async function showAndCapture(html: string, name: string): Promise<void> {
  // Show div (don't await - it waits for user submit)
  div(html);
  
  // Wait for render
  await new Promise(resolve => setTimeout(resolve, 800));
  
  // Capture screenshot
  const screenshot = await captureScreenshot();
  const filename = `tailwind-comprehensive-${name}-${Date.now()}.png`;
  const filepath = join(screenshotDir, filename);
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] ${name}: ${filepath}`);
}

console.error('[TAILWIND-TEST] Starting comprehensive Tailwind tests...');

// ============================================================================
// TEST 1: Background Colors (All Major Colors)
// ============================================================================
console.error('[TAILWIND-TEST] Testing background colors...');

await showAndCapture(`
<div class="flex flex-col gap-2 p-4">
  <h2 class="text-white font-bold text-xl">Background Colors</h2>
  <div class="flex flex-row gap-2">
    <div class="p-2 bg-red-500 text-white">red</div>
    <div class="p-2 bg-orange-500 text-white">orange</div>
    <div class="p-2 bg-yellow-500 text-black">yellow</div>
    <div class="p-2 bg-green-500 text-white">green</div>
  </div>
  <div class="flex flex-row gap-2">
    <div class="p-2 bg-blue-500 text-white">blue</div>
    <div class="p-2 bg-indigo-500 text-white">indigo</div>
    <div class="p-2 bg-purple-500 text-white">purple</div>
    <div class="p-2 bg-pink-500 text-white">pink</div>
  </div>
  <div class="flex flex-row gap-2">
    <div class="p-2 bg-slate-500 text-white">slate</div>
    <div class="p-2 bg-gray-500 text-white">gray</div>
    <div class="p-2 bg-black text-white">black</div>
    <div class="p-2 bg-white text-black border">white</div>
  </div>
</div>
`, 'bg-colors');

// ============================================================================
// TEST 2: Color Shades (50-950)
// ============================================================================
console.error('[TAILWIND-TEST] Testing color shades...');

await showAndCapture(`
<div class="flex flex-col gap-2 p-4">
  <h2 class="text-white font-bold text-xl">Blue Color Shades</h2>
  <div class="flex flex-row gap-1">
    <div class="p-2 bg-blue-50 text-black">50</div>
    <div class="p-2 bg-blue-200 text-black">200</div>
    <div class="p-2 bg-blue-400 text-white">400</div>
    <div class="p-2 bg-blue-600 text-white">600</div>
    <div class="p-2 bg-blue-800 text-white">800</div>
    <div class="p-2 bg-blue-950 text-white">950</div>
  </div>
</div>
`, 'color-shades');

// ============================================================================
// TEST 3: Text Colors
// ============================================================================
console.error('[TAILWIND-TEST] Testing text colors...');

await showAndCapture(`
<div class="flex flex-col gap-1 p-4 bg-gray-900">
  <h2 class="text-white font-bold text-xl">Text Colors</h2>
  <p class="text-red-500">text-red-500</p>
  <p class="text-green-500">text-green-500</p>
  <p class="text-blue-500">text-blue-500</p>
  <p class="text-yellow-500">text-yellow-500</p>
  <p class="text-purple-500">text-purple-500</p>
  <p class="text-white">text-white</p>
</div>
`, 'text-colors');

// ============================================================================
// TEST 4: Spacing - Padding & Margin
// ============================================================================
console.error('[TAILWIND-TEST] Testing padding and margin...');

await showAndCapture(`
<div class="flex flex-col gap-2 p-4">
  <h2 class="text-white font-bold text-xl">Padding</h2>
  <div class="bg-blue-500 text-white p-1">p-1</div>
  <div class="bg-blue-500 text-white p-4">p-4</div>
  <div class="bg-blue-500 text-white px-8 py-2">px-8 py-2</div>
  <h2 class="text-white font-bold text-xl mt-4">Margin</h2>
  <div class="bg-gray-700 p-2">
    <div class="bg-green-500 text-white p-2 ml-8">ml-8</div>
  </div>
</div>
`, 'padding-margin');

// ============================================================================
// TEST 5: Flexbox Layout
// ============================================================================
console.error('[TAILWIND-TEST] Testing flexbox...');

await showAndCapture(`
<div class="flex flex-col gap-4 p-4">
  <h2 class="text-white font-bold text-xl">Flexbox</h2>
  <h3 class="text-white">flex-row:</h3>
  <div class="flex flex-row gap-2 bg-gray-700 p-2">
    <div class="bg-blue-500 text-white p-2">A</div>
    <div class="bg-blue-500 text-white p-2">B</div>
    <div class="bg-blue-500 text-white p-2">C</div>
  </div>
  <h3 class="text-white">flex-col:</h3>
  <div class="flex flex-col gap-2 bg-gray-700 p-2">
    <div class="bg-green-500 text-white p-2">A</div>
    <div class="bg-green-500 text-white p-2">B</div>
  </div>
  <h3 class="text-white">flex-1 (equal width):</h3>
  <div class="flex flex-row gap-2 bg-gray-700 p-2">
    <div class="flex-1 bg-purple-500 text-white p-2">flex-1</div>
    <div class="flex-1 bg-purple-500 text-white p-2">flex-1</div>
  </div>
</div>
`, 'flexbox');

// ============================================================================
// TEST 6: Alignment
// ============================================================================
console.error('[TAILWIND-TEST] Testing alignment...');

await showAndCapture(`
<div class="flex flex-col gap-4 p-4">
  <h2 class="text-white font-bold text-xl">Alignment</h2>
  <div class="flex flex-row gap-2">
    <div class="flex items-start h-16 w-16 bg-gray-700">
      <div class="bg-red-500 p-1 text-white text-xs">start</div>
    </div>
    <div class="flex items-center h-16 w-16 bg-gray-700">
      <div class="bg-green-500 p-1 text-white text-xs">center</div>
    </div>
    <div class="flex items-end h-16 w-16 bg-gray-700">
      <div class="bg-blue-500 p-1 text-white text-xs">end</div>
    </div>
  </div>
  <div class="flex justify-between bg-gray-700 p-2">
    <div class="bg-orange-500 p-2 text-white">A</div>
    <div class="bg-orange-500 p-2 text-white">B</div>
  </div>
</div>
`, 'alignment');

// ============================================================================
// TEST 7: Typography
// ============================================================================
console.error('[TAILWIND-TEST] Testing typography...');

await showAndCapture(`
<div class="flex flex-col gap-1 p-4">
  <h2 class="text-white font-bold text-xl">Typography</h2>
  <p class="text-xs text-white">text-xs</p>
  <p class="text-sm text-white">text-sm</p>
  <p class="text-base text-white">text-base</p>
  <p class="text-lg text-white">text-lg</p>
  <p class="text-xl text-white">text-xl</p>
  <p class="text-2xl text-white">text-2xl</p>
  <p class="font-thin text-white">font-thin</p>
  <p class="font-bold text-white">font-bold</p>
</div>
`, 'typography');

// ============================================================================
// TEST 8: Border Radius & Borders
// ============================================================================
console.error('[TAILWIND-TEST] Testing borders and radius...');

await showAndCapture(`
<div class="flex flex-col gap-4 p-4">
  <h2 class="text-white font-bold text-xl">Border Radius</h2>
  <div class="flex flex-row gap-2">
    <div class="bg-blue-500 text-white p-4">none</div>
    <div class="bg-blue-500 text-white p-4 rounded">rounded</div>
    <div class="bg-blue-500 text-white p-4 rounded-lg">rounded-lg</div>
    <div class="bg-blue-500 text-white p-4 rounded-full">full</div>
  </div>
  <h2 class="text-white font-bold text-xl">Borders</h2>
  <div class="flex flex-row gap-2">
    <div class="bg-gray-800 text-white p-4 border border-white">border</div>
    <div class="bg-gray-800 text-white p-4 border-2 border-red-500">border-2</div>
    <div class="bg-gray-800 text-white p-4 border-4 border-blue-500">border-4</div>
  </div>
</div>
`, 'borders');

// ============================================================================
// TEST 9: Gap Spacing
// ============================================================================
console.error('[TAILWIND-TEST] Testing gap spacing...');

await showAndCapture(`
<div class="flex flex-col gap-4 p-4">
  <h2 class="text-white font-bold text-xl">Gap Spacing</h2>
  <p class="text-gray-400">gap-0:</p>
  <div class="flex flex-row gap-0">
    <div class="bg-blue-500 text-white p-2">A</div>
    <div class="bg-blue-600 text-white p-2">B</div>
    <div class="bg-blue-700 text-white p-2">C</div>
  </div>
  <p class="text-gray-400">gap-4:</p>
  <div class="flex flex-row gap-4">
    <div class="bg-green-500 text-white p-2">A</div>
    <div class="bg-green-600 text-white p-2">B</div>
    <div class="bg-green-700 text-white p-2">C</div>
  </div>
</div>
`, 'gap-spacing');

// ============================================================================
// TEST 10: Complex Layout
// ============================================================================
console.error('[TAILWIND-TEST] Testing complex layout...');

await showAndCapture(`
<div class="flex flex-col gap-4 p-4">
  <h1 class="text-white font-bold text-2xl">Complex Layout</h1>
  <div class="flex flex-row gap-4">
    <div class="flex-1 flex flex-col gap-2 bg-gray-800 p-4 rounded-lg">
      <h2 class="text-blue-400 font-bold">Card 1</h2>
      <p class="text-gray-300 text-sm">Card with Tailwind styling.</p>
      <div class="flex flex-row gap-2 mt-2">
        <span class="bg-blue-500 text-white px-2 py-1 rounded text-xs">Tag 1</span>
        <span class="bg-green-500 text-white px-2 py-1 rounded text-xs">Tag 2</span>
      </div>
    </div>
    <div class="flex-1 flex flex-col gap-2 bg-gray-800 p-4 rounded-lg">
      <h2 class="text-purple-400 font-bold">Card 2</h2>
      <p class="text-gray-300 text-sm">Another styled card.</p>
    </div>
  </div>
</div>
`, 'complex-layout');

console.error('[TAILWIND-TEST] All Tailwind tests complete!');
console.error('[TAILWIND-TEST] Screenshots saved to: ' + screenshotDir);

process.exit(0);

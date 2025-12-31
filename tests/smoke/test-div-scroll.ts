// Name: Test Div Scroll
// Description: RED PHASE - Tests that div() content scrolls when it exceeds window height

import '../../scripts/kit-sdk';

console.error('[SMOKE] test-div-scroll.ts starting...');
console.error('[SMOKE] This test should FAIL until div() scroll is implemented');

// Generate 60 items - this should definitely overflow the 500px window
const items: string[] = [];
for (let i = 1; i <= 60; i++) {
  items.push(`<div class="py-2 px-4 border-b border-gray-700">
    <span class="text-blue-400 font-mono">Item ${i.toString().padStart(2, '0')}</span>
    <span class="text-gray-400 ml-2">- This is test content that should be scrollable</span>
  </div>`);
}

const html = `
<div class="flex flex-col">
  <h1 class="text-xl font-bold p-4 bg-gray-800 sticky top-0">Scroll Test (60 Items)</h1>
  <p class="px-4 py-2 text-gray-400">All 60 items below should be accessible via scrolling.</p>
  <p class="px-4 py-2 text-yellow-500">Currently: Content is CLIPPED (no scroll)</p>
  <hr class="border-gray-700 my-2">
  ${items.join('\n')}
  <div class="p-4 bg-green-900 text-green-100">
    <strong>END MARKER:</strong> If you can see this, scroll is working!
  </div>
</div>
`;

console.error('[SMOKE] Content generated with 60 items');
console.error('[SMOKE] Expected behavior: User can scroll to see all items and END MARKER');
console.error('[SMOKE] Current behavior: Content is clipped, cannot scroll');

await div(html);

console.error('[SMOKE] test-div-scroll.ts completed');
process.exit(0);

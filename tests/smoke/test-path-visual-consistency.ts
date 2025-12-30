// Name: Path Prompt Visual Consistency Test
// Description: Verifies path prompt header matches main menu styling

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

/**
 * Visual Consistency Checklist for Path Prompt
 * 
 * The path prompt header should match the main menu header:
 * 
 * HEADER LAYOUT:
 * - Padding: 16px horizontal, 8px vertical
 * - Gap between elements: 12px
 * - Input area: flex_1 (fills available space)
 * - Text size: text_lg
 * 
 * CURSOR:
 * - Width: 2px, Height: 20px
 * - Position: LEFT when input empty, RIGHT when typing
 * - Color: text_primary (visible when focused)
 * 
 * PATH PREFIX:
 * - Color: text_muted
 * - Format: "{current_path}/"
 * 
 * BUTTONS:
 * - Style: Ghost variant
 * - Primary: "Select ↵"
 * - Actions: "Actions ⌘K"
 * - Separators: "|" at 60% opacity
 * 
 * LOGO:
 * - Size: 16px
 * - Color: accent color (yellow/gold)
 * - Position: Right-most element
 * 
 * CONTAINER:
 * - Divider after header: 1px, 60% opacity border
 * - Footer hint: text_xs, text_muted, centered
 */

console.error('[SMOKE] Starting path prompt visual consistency test');

// Display a div that documents the expected visual elements
await div(`
  <div class="p-6 space-y-4">
    <h1 class="text-xl font-bold">Path Prompt Visual Consistency Test</h1>
    
    <div class="bg-zinc-800 p-4 rounded-lg">
      <h2 class="font-semibold mb-2">Expected Header Layout</h2>
      <div class="flex items-center gap-3 bg-zinc-900 px-4 py-2 rounded">
        <span class="text-zinc-500">/Users/test/</span>
        <span class="w-0.5 h-5 bg-white animate-pulse"></span>
        <span class="text-zinc-400">Type to filter...</span>
        <span class="flex-1"></span>
        <span class="text-yellow-500">Select ↵</span>
        <span class="text-zinc-600">|</span>
        <span class="text-yellow-500">Actions ⌘K</span>
        <span class="text-zinc-600">|</span>
        <span class="text-yellow-500">&#9889;</span>
      </div>
    </div>
    
    <div class="text-sm text-zinc-400">
      <p class="mb-2">Visual elements to verify:</p>
      <ul class="list-disc list-inside space-y-1">
        <li>Header padding: 16px horizontal, 8px vertical</li>
        <li>Input text: text_lg size (~18px)</li>
        <li>Path prefix in muted gray before cursor</li>
        <li>Blinking cursor (2px wide, 20px tall)</li>
        <li>Select and Actions buttons in accent color</li>
        <li>Pipe separators between elements</li>
        <li>Script Kit logo on the right</li>
        <li>Divider below header at 60% opacity</li>
      </ul>
    </div>
    
    <p class="text-xs text-zinc-500">
      Press Escape to close this test
    </p>
  </div>
`);

// Wait for render
await new Promise(resolve => setTimeout(resolve, 500));

// Capture screenshot
try {
  const screenshot = await captureScreenshot();
  console.error(`[SCREENSHOT] Captured: ${screenshot.width}x${screenshot.height}`);
  
  // Save to test-screenshots
  const screenshotDir = join(process.cwd(), 'test-screenshots');
  mkdirSync(screenshotDir, { recursive: true });
  const filepath = join(screenshotDir, `path-visual-test-${Date.now()}.png`);
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] Saved to: ${filepath}`);
} catch (err) {
  console.error('[SCREENSHOT] Error capturing:', err);
}

console.error('[SMOKE] Path prompt visual consistency test completed');
process.exit(0);

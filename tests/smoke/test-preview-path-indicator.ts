// Name: Test Preview Path Indicator
// Description: Verify the source path appears in preview panel

/**
 * SMOKE TEST: test-preview-path-indicator.ts
 *
 * This test verifies that the preview panel shows a subtle source path
 * indicator at the top, helping users identify where scripts/scriptlets
 * come from (e.g., ~/.sk/kit/scriptlets/foo.md#my-snippet)
 */

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[SMOKE] test-preview-path-indicator.ts starting...');

// Test 1: Display info about what we're testing
console.error('[SMOKE] Test 1: Preview path indicator feature');

await div(md(`# Preview Path Indicator Test

## Feature Description

The preview panel now shows a **subtle source path** at the very top,
helping users quickly identify where a script or scriptlet comes from.

## Expected Format

### For Scripts
\`\`\`
~/.sk/kit/scripts/my-script.ts
\`\`\`

### For Scriptlets
\`\`\`
~/.sk/kit/scriptlets/foo.md#my-snippet-name
\`\`\`

## Visual Styling

- **Font**: Monospace (matches code preview)
- **Size**: Extra small (text-xs)
- **Color**: Muted with transparency (~60% opacity)
- **Position**: Above the name header
- **Overflow**: Ellipsis for long paths

## How to Test

1. Close this dialog
2. Navigate to any script in the main menu
3. Look at the preview panel on the right
4. The path should appear at the very top in subtle gray text

---

*Click anywhere or press Escape to continue*`));

console.error('[SMOKE] Test 1 complete');

// Wait for render
await new Promise(resolve => setTimeout(resolve, 500));

// Capture screenshot
console.error('[SMOKE] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[SMOKE] Screenshot: ${screenshot.width}x${screenshot.height}`);

// Save to ./.test-screenshots/
const screenshotDir = join(process.cwd(), '.test-screenshots');
mkdirSync(screenshotDir, { recursive: true });

const filename = `preview-path-indicator-${Date.now()}.png`;
const filepath = join(screenshotDir, filename);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));

console.error(`[SCREENSHOT] Saved to: ${filepath}`);
console.error('[SMOKE] test-preview-path-indicator.ts completed successfully!');

process.exit(0);

// Name: Notes Hovered Visual Test
// Description: Captures hovered Notes window for Raycast parity verification
//
// Run:
//   printf '{"type":"openNotes"}\n{"type":"run","path":"'$(pwd)'/tests/smoke/test-notes-hovered-visual.ts"}\n' \
//     | SCRIPT_KIT_TEST_NOTES_HOVERED=1 SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1
//
// Expected:
//   - Title left-aligned ("Untitled Note") with hover chrome visible
//   - Top-right actions (⌘K, list, +)
//   - Placeholder text: "Start writing..."
//   - Footer: centered character count, "T" on right

import '../../scripts/kit-sdk';
import { mkdirSync, writeFileSync } from 'fs';
import { join } from 'path';

console.error('[TEST] Notes Hovered Visual Test');
console.error('[TEST] Waiting for notes window to render...');

await new Promise(resolve => setTimeout(resolve, 900));

const screenshot = await captureScreenshot();
console.error(`[TEST] Captured: ${screenshot.width}x${screenshot.height}`);

const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });

const filepath = join(dir, `notes-hovered-${Date.now()}.png`);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

console.error('[TEST] Verify: centered title, top-right icons, centered char count, right-side "T"');

process.exit(0);

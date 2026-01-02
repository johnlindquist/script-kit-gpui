// Name: Visual Audit
// Description: Non-interactive visual tests - captures screenshots of UI states

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Visual Audit",
  description: "Automated visual regression testing - captures before user interaction",
};

const screenshotDir = join(process.cwd(), '.test-screenshots', 'visual-audit');
mkdirSync(screenshotDir, { recursive: true });

interface TestResult {
  name: string;
  path: string;
  width: number;
  height: number;
}

const results: TestResult[] = [];

async function captureTest(name: string): Promise<void> {
  console.error(`[CAPTURE] ${name} - waiting for render...`);
  // Wait for render
  await new Promise(r => setTimeout(r, 700));
  console.error(`[CAPTURE] ${name} - calling captureScreenshot...`);
  
  try {
    const ss = await captureScreenshot();
    console.error(`[CAPTURE] ${name} - got response: ${ss.width}x${ss.height}, ${ss.data.length} chars`);
    
    if (ss.data.length > 0) {
      const filename = `${name}.png`;
      const filepath = join(screenshotDir, filename);
      writeFileSync(filepath, Buffer.from(ss.data, 'base64'));
      results.push({ name, path: filepath, width: ss.width, height: ss.height });
      console.error(`[CAPTURED] ${name}: ${ss.width}x${ss.height}`);
    } else {
      console.error(`[ERROR] ${name}: Empty screenshot data`);
    }
  } catch (err) {
    console.error(`[ERROR] ${name}: ${err}`);
  }
}

// ============================================================
// Run tests
// ============================================================

console.error(`[SUITE] Starting visual audit`);
console.error(`[SUITE] Output dir: ${screenshotDir}`);

async function runTests() {
  // Test 1: Basic HTML div
  console.error(`[TEST] 01-div-basic-html`);
  div(`
    <div style="padding: 24px; font-family: system-ui;">
      <h1 style="color: #3b82f6; margin: 0 0 12px 0; font-size: 24px;">Basic HTML Test</h1>
      <p style="color: #9ca3af;">Testing inline styles and basic HTML elements</p>
      <ul style="margin: 0; padding-left: 20px; color: #d1d5db;">
        <li>First list item</li>
        <li>Second list item</li>
        <li>Third list item</li>
      </ul>
    </div>
  `);
  await captureTest('01-div-basic-html');

  // Test 2: Tailwind CSS
  console.error(`[TEST] 02-div-tailwind`);
  div({
    html: `
      <div class="text-center">
        <h1 class="text-2xl font-bold text-blue-400 mb-2">Tailwind CSS Test</h1>
        <p class="text-gray-400 mb-4">Using Tailwind utility classes</p>
        <div class="flex gap-2 justify-center flex-wrap">
          <span class="px-3 py-1 bg-green-500/20 text-green-400 rounded-full text-sm">Success</span>
          <span class="px-3 py-1 bg-blue-500/20 text-blue-400 rounded-full text-sm">Info</span>
          <span class="px-3 py-1 bg-yellow-500/20 text-yellow-400 rounded-full text-sm">Warning</span>
          <span class="px-3 py-1 bg-red-500/20 text-red-400 rounded-full text-sm">Error</span>
        </div>
      </div>
    `,
    containerClasses: 'p-6'
  });
  await captureTest('02-div-tailwind');

  // Test 3: arg with string choices
  console.error(`[TEST] 03-arg-strings`);
  arg('Select a fruit', ['🍎 Apple', '🍌 Banana', '🍒 Cherry', '🍇 Grape', '🫐 Blueberry']);
  await captureTest('03-arg-strings');

  // Summary
  console.error(`\n[SUITE] Complete: ${results.length} screenshots captured`);
  console.error(`[OUTPUT] ${screenshotDir}`);
  
  // Output JSON for parsing
  console.log(JSON.stringify({
    suite: 'visual-audit',
    status: 'complete',
    captured: results.length,
    results,
    outputDir: screenshotDir
  }, null, 2));
  
  // Force exit
  process.exit(0);
}

runTests().catch(err => {
  console.error(`[FATAL] ${err}`);
  process.exit(1);
});

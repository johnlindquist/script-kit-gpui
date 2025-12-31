// Comprehensive HTML Rendering Test - Tests ALL supported HTML elements
// Captures screenshots for visual verification of each element type
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
  const filename = `html-comprehensive-${name}-${Date.now()}.png`;
  const filepath = join(screenshotDir, filename);
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] ${name}: ${filepath}`);
}

console.error('[HTML-TEST] Starting comprehensive HTML element tests...');

// ============================================================================
// TEST 1: Headers (h1-h6)
// ============================================================================
console.error('[HTML-TEST] Testing headers h1-h6...');

await showAndCapture(`
<div class="p-4">
  <h1>Header 1 - Largest</h1>
  <h2>Header 2 - Very Large</h2>
  <h3>Header 3 - Large</h3>
  <h4>Header 4 - Medium</h4>
  <h5>Header 5 - Small</h5>
  <h6>Header 6 - Smallest</h6>
</div>
`, 'headers');

// ============================================================================
// TEST 2: Paragraphs and Text Styling
// ============================================================================
console.error('[HTML-TEST] Testing paragraphs and text styling...');

await showAndCapture(`
<div class="p-4">
  <h2>Text Styling Tests</h2>
  <p>This is a plain paragraph with normal text.</p>
  <p>This paragraph has <strong>bold text</strong> in the middle.</p>
  <p>This paragraph has <em>italic text</em> for emphasis.</p>
  <p>This paragraph has <strong><em>bold and italic</em></strong> combined.</p>
  <p>Regular text, then <strong>bold</strong>, then <em>italic</em>, back to regular.</p>
</div>
`, 'text-styling');

// ============================================================================
// TEST 3: Inline Code and Code Blocks
// ============================================================================
console.error('[HTML-TEST] Testing inline code and code blocks...');

await showAndCapture(`
<div class="p-4">
  <h2>Code Tests</h2>
  <p>Here is <code>inline code</code> within a paragraph.</p>
  <p>Multiple inline codes: <code>const x = 1</code> and <code>console.log(x)</code></p>
  <h3>TypeScript Code Block:</h3>
  <pre><code class="language-typescript">interface User {
  name: string;
  age: number;
}

const user: User = {
  name: "John",
  age: 30
};</code></pre>
</div>
`, 'code-blocks');

// ============================================================================
// TEST 4: Lists (Ordered and Unordered)
// ============================================================================
console.error('[HTML-TEST] Testing lists...');

await showAndCapture(`
<div class="p-4">
  <h2>List Tests</h2>
  <h3>Unordered List:</h3>
  <ul>
    <li>First item</li>
    <li>Second item</li>
    <li>Third item with more text</li>
  </ul>
  <h3>Ordered List:</h3>
  <ol>
    <li>Step one</li>
    <li>Step two</li>
    <li>Step three</li>
  </ol>
</div>
`, 'lists');

// ============================================================================
// TEST 5: Blockquotes, HR, Links, LineBreaks
// ============================================================================
console.error('[HTML-TEST] Testing blockquotes, hr, links, line breaks...');

await showAndCapture(`
<div class="p-4">
  <h2>Other Elements</h2>
  <blockquote>This is a blockquote with important content.</blockquote>
  <hr>
  <p>Here is a <a href="https://example.com">link</a> in text.</p>
  <p>Line one<br>Line two<br>Line three</p>
</div>
`, 'other-elements');

// ============================================================================
// TEST 6: Nested Containers (div/span)
// ============================================================================
console.error('[HTML-TEST] Testing nested containers...');

await showAndCapture(`
<div class="p-4">
  <h2>Nested Container Tests</h2>
  <div class="p-2 bg-gray-800">
    <p>Outer div with background</p>
    <div class="p-2 bg-gray-700 rounded">
      <p>Inner nested div</p>
      <span class="text-blue-400">A colored span</span>
    </div>
  </div>
</div>
`, 'nested-containers');

// ============================================================================
// TEST 7: All Elements Combined
// ============================================================================
console.error('[HTML-TEST] Testing all elements combined...');

await showAndCapture(`
<div class="p-4">
  <h1>Comprehensive HTML Test</h1>
  <h2>Introduction</h2>
  <p>This test combines <strong>all HTML elements</strong> to verify they render correctly.</p>
  <pre><code class="language-typescript">const test = "all elements";</code></pre>
  <ul>
    <li>Headers (h1-h6)</li>
    <li>Paragraphs with <strong>bold</strong> and <em>italic</em></li>
    <li><code>Inline code</code> and code blocks</li>
  </ul>
  <blockquote>All elements should render properly.</blockquote>
  <hr>
  <p>Visit <a href="https://scriptkit.com">Script Kit</a> for more info.</p>
</div>
`, 'all-combined');

console.error('[HTML-TEST] All HTML element tests complete!');
console.error('[HTML-TEST] Screenshots saved to: ' + screenshotDir);

process.exit(0);

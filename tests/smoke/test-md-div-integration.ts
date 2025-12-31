// md() -> div() Integration Test
// Tests that md() converts markdown properly and renders via div()
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
  const filename = `md-div-integration-${name}-${Date.now()}.png`;
  const filepath = join(screenshotDir, filename);
  writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
  console.error(`[SCREENSHOT] ${name}: ${filepath}`);
}

console.error('[MD-DIV-TEST] Starting md() -> div() integration tests...');

// ============================================================================
// TEST 1: Headers in Markdown
// ============================================================================
console.error('[MD-DIV-TEST] Testing markdown headers...');

const headersHtml = md(`# Header 1
## Header 2
### Header 3
#### Header 4
##### Header 5
###### Header 6`);

console.error('[MD-DIV-TEST] Headers HTML:', headersHtml);
await showAndCapture(headersHtml, 'md-headers');

// ============================================================================
// TEST 2: Text Styling in Markdown
// ============================================================================
console.error('[MD-DIV-TEST] Testing markdown text styling...');

const textHtml = md(`# Text Styling

This is **bold text** in a paragraph.

This is *italic text* for emphasis.

This is ***bold and italic*** combined.`);

console.error('[MD-DIV-TEST] Text HTML:', textHtml);
await showAndCapture(textHtml, 'md-text-styling');

// ============================================================================
// TEST 3: Code in Markdown
// ============================================================================
console.error('[MD-DIV-TEST] Testing markdown code...');

const codeHtml = md(`# Code Examples

Here is \`inline code\` in a sentence.

\`\`\`typescript
interface User {
  name: string;
  email: string;
}

const user: User = {
  name: "John",
  email: "john@example.com"
};
\`\`\``);

console.error('[MD-DIV-TEST] Code HTML:', codeHtml);
await showAndCapture(codeHtml, 'md-code');

// ============================================================================
// TEST 4: Lists in Markdown
// ============================================================================
console.error('[MD-DIV-TEST] Testing markdown lists...');

const listsHtml = md(`# Lists

## Unordered List:
- First item
- Second item
- Third item

## Ordered List:
1. Step one
2. Step two
3. Step three`);

console.error('[MD-DIV-TEST] Lists HTML:', listsHtml);
await showAndCapture(listsHtml, 'md-lists');

// ============================================================================
// TEST 5: Blockquotes, HR, Links
// ============================================================================
console.error('[MD-DIV-TEST] Testing markdown blockquotes, hr, links...');

const otherHtml = md(`# Other Elements

> This is a blockquote with important content.

---

Here is a [link](https://example.com) in text.

Line one  
Line two  
Line three`);

console.error('[MD-DIV-TEST] Other HTML:', otherHtml);
await showAndCapture(otherHtml, 'md-other-elements');

// ============================================================================
// TEST 6: Full Document with md() -> div()
// ============================================================================
console.error('[MD-DIV-TEST] Testing full markdown document...');

const fullHtml = md(`# Complete Markdown Test

## Introduction

This document tests **all markdown features** rendered via \`md()\` and displayed with \`div()\`.

## Code Example

\`\`\`typescript
const greeting = "Hello, World!";
console.log(greeting);
\`\`\`

## Features Tested

1. Headers (h1-h6)
2. **Bold** and *italic* text
3. \`Inline code\` and code blocks
4. Lists (ordered and unordered)

### Blockquote

> This is a quote about markdown.

---

Visit [Script Kit](https://scriptkit.com) for more info.`);

console.error('[MD-DIV-TEST] Full document HTML length:', fullHtml.length);
await showAndCapture(fullHtml, 'md-full-document');

// ============================================================================
// TEST 7: Verify HTML Structure
// ============================================================================
console.error('[MD-DIV-TEST] Verifying HTML structure from md()...');

const testMd = `# Test
**bold** and *italic*
\`code\``;

const html = md(testMd);

// Check for expected HTML elements
const checks = [
  { name: 'h1 tag', pattern: /<h1>Test<\/h1>/, found: false },
  { name: 'strong tag', pattern: /<strong>bold<\/strong>/, found: false },
  { name: 'em tag', pattern: /<em>italic<\/em>/, found: false },
  { name: 'code tag', pattern: /<code>code<\/code>/, found: false },
];

for (const check of checks) {
  check.found = check.pattern.test(html);
  console.error(`[MD-DIV-TEST] ${check.name}: ${check.found ? 'PASS' : 'FAIL'}`);
}

const passedCount = checks.filter(c => c.found).length;
console.error(`[MD-DIV-TEST] Structure verification: ${passedCount}/${checks.length} passed`);

// ============================================================================
// TEST 8: md() with Tailwind wrapper
// ============================================================================
console.error('[MD-DIV-TEST] Testing md() with Tailwind wrapper...');

const mdContent = md(`# Markdown + Tailwind

This is **bold** and *italic* text.

\`\`\`typescript
const x = 42;
\`\`\`

- List item 1
- List item 2
`);

const wrappedHtml = `<div class="p-4 bg-gray-800 rounded-lg">${mdContent}</div>`;
await showAndCapture(wrappedHtml, 'md-with-tailwind-wrapper');

console.error('[MD-DIV-TEST] All md() -> div() integration tests complete!');
console.error('[MD-DIV-TEST] Screenshots saved to: ' + screenshotDir);

process.exit(0);

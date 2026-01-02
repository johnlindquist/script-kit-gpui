// Test editor font rendering with JetBrains Mono
import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

export const metadata = {
  name: "Test Editor Font",
  description: "Verify JetBrains Mono font rendering at 16px",
};

console.error('[TEST] Starting editor font test...');

// Sample code with various characters that show font features
const testCode = `// JetBrains Mono Font Test
function greet(name: string): string {
  const message = \`Hello, \${name}!\`;
  return message;
}

// Ligatures test: => -> !== === <= >=
const arrow = (x) => x * 2;
const notEqual = 1 !== 2;

// Numbers and symbols: 0O Il1 {} [] ()
const data = { items: [1, 2, 3] };

console.log(greet("World"));`;

// First show the window
await show();

// Wait for window to be ready
await new Promise(resolve => setTimeout(resolve, 500));

// Open editor with TypeScript syntax (don't await - we'll capture while it's showing)
editor(testCode, "typescript");

// Wait for editor to render
await new Promise(resolve => setTimeout(resolve, 1000));

// Capture screenshot
console.error('[TEST] Capturing screenshot...');
const screenshot = await captureScreenshot();
console.error(`[TEST] Captured: ${screenshot.width}x${screenshot.height}`);

// Save screenshot
const dir = join(process.cwd(), '.test-screenshots');
mkdirSync(dir, { recursive: true });
const filename = `editor-font-${Date.now()}.png`;
const filepath = join(dir, filename);
writeFileSync(filepath, Buffer.from(screenshot.data, 'base64'));
console.error(`[SCREENSHOT] ${filepath}`);

// Exit
process.exit(0);

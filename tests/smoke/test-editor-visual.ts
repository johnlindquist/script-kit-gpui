// Name: Editor Visual Test
// Description: Quick visual verification of editor prompt

import '../../scripts/kit-sdk';

// Show a simple editor with TypeScript code
const code = `// Hello from Script Kit GPUI Editor!
function greet(name: string): string {
  return \`Hello, \${name}!\`;
}

const message = greet("World");
console.log(message);

// Press Cmd+Enter to submit or Escape to cancel
`;

console.error("[TEST] Showing editor with TypeScript code...");

const result = await editor(code, "typescript");

console.error(`[TEST] Editor returned: ${result ? result.substring(0, 50) + "..." : "null"}`);

await div(md(`# Editor Test Complete

You ${result ? "submitted" : "cancelled"} the editor.

${result ? `**Result length:** ${result.length} characters` : "*No content returned*"}

Press Escape to exit.`));

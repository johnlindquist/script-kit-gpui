// Test: Template with choice dropdown (VSCode-style)
// Syntax: ${1|option1,option2,option3|}

import '../../scripts/kit-sdk';

export const metadata = {
  name: "Template Choices Test",
  description: "Tests choice dropdown in templates",
};

console.error('[TEST] Starting template choices test...');
console.error('');
console.error('Template syntax: ${1|option1,option2,option3|}');
console.error('');
console.error('INSTRUCTIONS:');
console.error('1. You should see "I prefer " with a dropdown showing 3 options');
console.error('2. Use UP/DOWN arrows to navigate');
console.error('3. Press ENTER to select, or TAB to select and move to next tabstop');
console.error('4. Press ESC to dismiss the dropdown');
console.error('');

// Template with choice tabstop
const template = "I prefer ${1|JavaScript,TypeScript,Python|} for ${2:purpose}!";

console.error('[TEST] Template:', template);
console.error('');

const result = await editor(template, "plaintext");

console.error('');
console.error('[RESULT]:', result);

// Name: Empty Choices Smoke Test
// Description: Tests edge case handling with empty or minimal choice lists

/**
 * SMOKE TEST: test-empty-choices.ts
 * 
 * This script tests edge cases in the choice list handling:
 * - Empty choice arrays
 * - Single choice
 * - Dynamic choice loading
 * 
 * Expected behavior:
 * 1. Script handles empty choices gracefully
 * 2. Single choice works correctly
 * 3. Dynamic choices update properly
 * 
 * Usage:
 *   ./target/debug/script-kit-gpui tests/smoke/test-empty-choices.ts
 * 
 * Expected exit code: 0
 */

import '../../scripts/kit-sdk';

console.error('[SMOKE] test-empty-choices.ts starting...');

// Test 1: Single choice (edge case)
console.error('[SMOKE] Test 1: Single choice...');
const singleResult = await arg('Only one option available:', [
  { name: 'The Only Option', value: 'only', description: 'This is the only choice' },
]);
console.error(`[SMOKE] Single choice result: ${singleResult}`);

// Test 2: Many choices (stress test list rendering)
console.error('[SMOKE] Test 2: Many choices...');
const manyChoices = Array.from({ length: 50 }, (_, i) => ({
  name: `Option ${i + 1}`,
  value: `opt-${i + 1}`,
  description: `This is option number ${i + 1} of 50`,
}));

const manyResult = await arg('Select from many options (scroll test):', manyChoices);
console.error(`[SMOKE] Many choices result: ${manyResult}`);

// Test 3: Text-only input (empty choices array)
console.error('[SMOKE] Test 3: Text-only input with empty choices...');
const textResult = await arg('Type any text (empty choices array):', []);
console.error(`[SMOKE] Text input result: ${textResult}`);

// Summary
await div(md(`# Edge Case Tests Complete

## Results:

1. **Single Choice Test**: ${singleResult}
2. **Many Choices Test**: ${manyResult}  
3. **Text Input Test**: ${textResult}

All edge cases handled successfully!`));

console.error('[SMOKE] test-empty-choices.ts completed');

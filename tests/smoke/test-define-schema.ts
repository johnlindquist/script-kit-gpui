import '../../scripts/kit-sdk';

console.error('[TEST] Starting defineSchema test');

// Test defineSchema
const { input, output } = defineSchema({
  input: {
    name: { type: "string", required: true },
    count: { type: "number", default: 1 },
  },
  output: {
    greeting: { type: "string" },
    processed: { type: "boolean" },
  },
} as const);

console.error('[TEST] defineSchema created input/output functions');
console.error('[TEST] typeof input:', typeof input);
console.error('[TEST] typeof output:', typeof output);

// Test _setScriptInput to simulate MCP providing input
console.error('[TEST] Testing _setScriptInput...');
_setScriptInput({ name: "Claude", count: 3 });

// Now input() should return the data we set
const inputData = await input();
console.error('[TEST] input() returned:', JSON.stringify(inputData));

// Verify input values
if (inputData.name !== "Claude") {
  console.error('[TEST] FAIL: name should be "Claude", got:', inputData.name);
  process.exit(1);
}
if (inputData.count !== 3) {
  console.error('[TEST] FAIL: count should be 3, got:', inputData.count);
  process.exit(1);
}
console.error('[TEST] PASS: input() returned correct values');

// Test output - call it multiple times to test accumulation
output({ greeting: `Hello ${inputData.name}!` });
console.error('[TEST] First output() called');

output({ processed: true });
console.error('[TEST] Second output() called');

// Check accumulated output using internal function
const accumulatedOutput = _getScriptOutput();
console.error('[TEST] _getScriptOutput():', JSON.stringify(accumulatedOutput));

// Verify accumulated output
if (accumulatedOutput.greeting !== "Hello Claude!") {
  console.error('[TEST] FAIL: greeting should be "Hello Claude!", got:', accumulatedOutput.greeting);
  process.exit(1);
}
if (accumulatedOutput.processed !== true) {
  console.error('[TEST] FAIL: processed should be true, got:', accumulatedOutput.processed);
  process.exit(1);
}
console.error('[TEST] PASS: output() accumulated correctly');

console.error('[TEST] All tests passed!');
process.exit(0);

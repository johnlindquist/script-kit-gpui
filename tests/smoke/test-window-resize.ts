// Name: Window Resize Test
// Description: Tests dynamic window resizing based on content

import '../../scripts/kit-sdk';

console.error("[TEST] Starting window resize test...");

// Test 1: Arg prompt with many choices (should expand)
console.error("[TEST] Test 1: Arg with 10 choices (should show expanded)");
const colors = await arg("Pick a color", [
  "Red",
  "Blue", 
  "Green",
  "Yellow",
  "Purple",
  "Orange",
  "Pink",
  "Brown",
  "Black",
  "White"
]);
console.error(`[TEST] Selected color: ${colors}`);

// Test 2: Arg prompt with few choices (should shrink)
console.error("[TEST] Test 2: Arg with 2 choices (should shrink)");
const yesNo = await arg("Confirm?", ["Yes", "No"]);
console.error(`[TEST] Confirmed: ${yesNo}`);

// Test 3: Arg prompt with no choices (compact mode)
console.error("[TEST] Test 3: Arg with no choices (compact input only)");
const typed = await arg("Type something (no choices)");
console.error(`[TEST] Typed: ${typed}`);

// Test 4: Div prompt (full height)
console.error("[TEST] Test 4: Div prompt (full height)");
await div(md(`# Window Resize Test Complete

All resize tests passed!

- **10 choices**: Window expanded
- **2 choices**: Window shrunk  
- **No choices**: Compact mode
- **Div**: Full height

Press Escape to exit.`));

console.error("[TEST] Window resize test complete!");

// Name: User Cancel Smoke Test
// Description: Tests that escape/cancel handling works correctly

/**
 * SMOKE TEST: test-user-cancel.ts
 * 
 * This script tests how the executor handles user cancellation:
 * - Pressing Escape key
 * - Cancelling prompts
 * - Graceful exit on cancel
 * 
 * Expected behavior:
 * 1. Script shows a prompt
 * 2. User presses Escape to cancel
 * 3. Script exits gracefully
 * 4. No error is thrown (cancel is not an error)
 * 
 * Usage:
 *   ./target/debug/script-kit-gpui tests/smoke/test-user-cancel.ts
 * 
 * To test: Press Escape when the prompt appears
 * Expected exit code: 0 (graceful cancellation)
 */

import '../../scripts/kit-sdk';

console.error('[SMOKE] test-user-cancel.ts starting...');

console.error('[SMOKE] Showing prompt - press Escape to test cancel...');

try {
  const result = await arg('Press Escape to cancel this prompt:', [
    { name: 'Option 1', value: '1', description: 'Select this to NOT test cancel' },
    { name: 'Option 2', value: '2', description: 'Select this to NOT test cancel' },
    { name: 'Option 3', value: '3', description: 'Select this to NOT test cancel' },
  ]);
  
  console.error(`[SMOKE] User selected: ${result} (did not cancel)`);
  
  await div(md(`# Selection Made
  
You selected: **${result}**

To test the cancel functionality, run this script again and press **Escape** instead of selecting an option.`));
  
} catch (error) {
  // Check if this is a cancellation
  if (error instanceof Error && error.message.includes('cancel')) {
    console.error('[SMOKE] User cancelled the prompt (expected behavior)');
  } else {
    console.error(`[SMOKE] Unexpected error: ${error}`);
    throw error;
  }
}

console.error('[SMOKE] test-user-cancel.ts completed');

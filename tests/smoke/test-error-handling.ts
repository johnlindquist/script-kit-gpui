// Name: Error Handling Smoke Test
// Description: Tests that script errors are properly handled and reported

/**
 * SMOKE TEST: test-error-handling.ts
 * 
 * This script tests how the executor handles various error scenarios:
 * - Thrown errors in script
 * - Graceful error display to user
 * - Proper exit code handling
 * 
 * Expected behavior:
 * 1. Script throws an error intentionally
 * 2. Executor captures the error
 * 3. Error is logged appropriately
 * 4. Script exits with non-zero exit code
 * 
 * Usage:
 *   ./target/debug/script-kit-gpui tests/smoke/test-error-handling.ts
 * 
 * Expected exit code: non-zero (1)
 */

import '../../scripts/kit-sdk';

console.error('[SMOKE] test-error-handling.ts starting...');

// Test 1: Show a prompt first to prove the script runs
console.error('[SMOKE] Showing initial prompt...');

const choice = await arg('Select an error type to test:', [
  { name: 'Throw Error', value: 'throw', description: 'Throws a standard Error' },
  { name: 'Type Error', value: 'type', description: 'Causes a TypeError' },
  { name: 'Reference Error', value: 'reference', description: 'Causes a ReferenceError' },
  { name: 'No Error (control)', value: 'none', description: 'Exits successfully' },
]);

console.error(`[SMOKE] User selected: ${choice}`);

switch (choice) {
  case 'throw': {
    console.error('[SMOKE] Throwing standard Error...');
    throw new Error('Intentional error for smoke testing');
  }
  
  case 'type': {
    console.error('[SMOKE] Causing TypeError...');
    // @ts-expect-error - intentional: testing null assignment to string
    const x: string = null;
    x.toUpperCase(); // Will throw TypeError
    break;
  }
  
  case 'reference': {
    console.error('[SMOKE] Causing ReferenceError...');
    // @ts-expect-error - intentional: testing undefined variable access
    undefinedVariable.toString();
    break;
  }
  
  case 'none': {
    console.error('[SMOKE] No error - showing success message');
    await div(md(`# Success!
    
You chose the control option. No error was thrown.

This proves the error handling test works correctly when:
- User selects "No Error (control)"
- Script completes normally
- Exit code should be 0`));
    break;
  }
}

console.error('[SMOKE] test-error-handling.ts completed');

// Name: Process Cleanup Test
// Description: Test script to verify process group cleanup works correctly

/**
 * SMOKE TEST: test-process-cleanup.ts
 *
 * This script is used to manually verify that killing the parent process
 * also kills child processes (via process group cleanup).
 *
 * Usage:
 *   bun run tests/smoke/test-process-cleanup.ts
 *
 * Then in another terminal:
 *   kill <PARENT_PID>
 *
 * Verify child is also killed:
 *   ps -p <CHILD_PID>  # Should show "no such process"
 */

// Make this a module
export {};

// Print parent PID
console.error(`PARENT_PID: ${process.pid}`);

// Spawn a long-running child process
const child = Bun.spawn(["sleep", "60"], {
  stdout: "ignore",
  stderr: "ignore",
});

// Print child PID
console.error(`CHILD_PID: ${child.pid}`);

console.error("[TEST] Waiting forever... Kill the parent process to test cleanup.");

// Wait forever
await new Promise(() => {});

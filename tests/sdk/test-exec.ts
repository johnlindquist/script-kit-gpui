// Name: SDK Test - exec()
// Description: Tests exec() function for shell command execution

/**
 * SDK TEST: test-exec.ts
 *
 * Tests the exec() function for executing shell commands.
 * 
 * NOTE: exec() is currently NOT implemented in the SDK.
 * This test serves as a TDD-style specification for expected behavior.
 * Tests will fail until exec() is implemented.
 *
 * Test cases:
 * 1. exec-function-exists: Verify exec function is defined
 * 2. exec-basic-command: Run simple shell command (echo)
 * 3. exec-with-args: Run command with arguments
 * 4. exec-returns-output: Command output is captured
 * 5. exec-exit-code: Non-zero exit codes are handled
 * 6. exec-error-handling: Invalid commands are handled gracefully
 *
 * Expected behavior (when implemented):
 * - exec(command) runs a shell command
 * - Returns stdout as string or { stdout, stderr, exitCode }
 * - Handles errors gracefully
 */

import "../../scripts/kit-sdk";

// =============================================================================
// Test Infrastructure
// =============================================================================

interface TestResult {
  test: string;
  status: "running" | "pass" | "fail" | "skip";
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
  expected?: string;
  actual?: string;
}

function logTest(
  name: string,
  status: TestResult["status"],
  extra?: Partial<TestResult>
) {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra,
  };
  console.log(JSON.stringify(result));
}

function debug(msg: string) {
  console.error(`[TEST] ${msg}`);
}

// =============================================================================
// Tests
// =============================================================================

debug("test-exec.ts starting...");

// Declare exec type for the tests (since it's not in SDK yet)
declare global {
  // exec() should be added to the SDK
  var exec: ((command: string) => Promise<string>) | undefined;
}

debug(`SDK globals: exec=${typeof globalThis.exec}`);

// -----------------------------------------------------------------------------
// Test 1: Verify exec function exists
// NOTE: This test is expected to FAIL until exec() is implemented
// -----------------------------------------------------------------------------
const test1 = "exec-function-exists";
logTest(test1, "running");
const start1 = Date.now();

try {
  debug("Test 1: Verify exec function exists");

  if (typeof globalThis.exec !== "function") {
    // This is the expected result until exec() is implemented
    logTest(test1, "fail", {
      error: `exec() is not yet implemented in the SDK. Expected a function, got ${typeof globalThis.exec}`,
      expected: "function",
      actual: typeof globalThis.exec,
      duration_ms: Date.now() - start1,
    });
  } else {
    logTest(test1, "pass", {
      result: { type: typeof globalThis.exec },
      duration_ms: Date.now() - start1,
    });
  }
} catch (err) {
  logTest(test1, "fail", {
    error: String(err),
    duration_ms: Date.now() - start1,
  });
}

// -----------------------------------------------------------------------------
// Test 2: Run simple shell command (echo)
// SKIP: exec() not implemented yet
// -----------------------------------------------------------------------------
const test2 = "exec-basic-command";
logTest(test2, "running");
const start2 = Date.now();

try {
  debug("Test 2: Run simple shell command (echo)");

  if (typeof globalThis.exec !== "function") {
    logTest(test2, "skip", {
      error: "exec() is not yet implemented - skipping test",
      duration_ms: Date.now() - start2,
    });
  } else {
    const result = await globalThis.exec('echo "Hello World"');

    if (result.trim() === "Hello World") {
      logTest(test2, "pass", {
        result: result.trim(),
        duration_ms: Date.now() - start2,
      });
    } else {
      logTest(test2, "fail", {
        error: "Unexpected output from echo command",
        expected: "Hello World",
        actual: result.trim(),
        duration_ms: Date.now() - start2,
      });
    }
  }
} catch (err) {
  logTest(test2, "fail", {
    error: String(err),
    duration_ms: Date.now() - start2,
  });
}

// -----------------------------------------------------------------------------
// Test 3: Run command with arguments
// SKIP: exec() not implemented yet
// -----------------------------------------------------------------------------
const test3 = "exec-with-args";
logTest(test3, "running");
const start3 = Date.now();

try {
  debug("Test 3: Run command with arguments");

  if (typeof globalThis.exec !== "function") {
    logTest(test3, "skip", {
      error: "exec() is not yet implemented - skipping test",
      duration_ms: Date.now() - start3,
    });
  } else {
    // Test: date command with format argument
    const result = await globalThis.exec("date +%Y");

    // Should return the current year (4 digits)
    const year = parseInt(result.trim(), 10);
    const currentYear = new Date().getFullYear();

    if (year === currentYear) {
      logTest(test3, "pass", {
        result: year,
        duration_ms: Date.now() - start3,
      });
    } else {
      logTest(test3, "fail", {
        error: "Unexpected output from date command",
        expected: String(currentYear),
        actual: result.trim(),
        duration_ms: Date.now() - start3,
      });
    }
  }
} catch (err) {
  logTest(test3, "fail", {
    error: String(err),
    duration_ms: Date.now() - start3,
  });
}

// -----------------------------------------------------------------------------
// Test 4: Command output is captured
// SKIP: exec() not implemented yet
// -----------------------------------------------------------------------------
const test4 = "exec-returns-output";
logTest(test4, "running");
const start4 = Date.now();

try {
  debug("Test 4: Command output is captured");

  if (typeof globalThis.exec !== "function") {
    logTest(test4, "skip", {
      error: "exec() is not yet implemented - skipping test",
      duration_ms: Date.now() - start4,
    });
  } else {
    // Test: multi-line output
    const result = await globalThis.exec('printf "line1\\nline2\\nline3"');
    const lines = result.split("\n");

    if (lines.length >= 3 && lines[0] === "line1" && lines[1] === "line2") {
      logTest(test4, "pass", {
        result: { lines: lines.length },
        duration_ms: Date.now() - start4,
      });
    } else {
      logTest(test4, "fail", {
        error: "Multi-line output not captured correctly",
        expected: "3 lines",
        actual: `${lines.length} lines`,
        duration_ms: Date.now() - start4,
      });
    }
  }
} catch (err) {
  logTest(test4, "fail", {
    error: String(err),
    duration_ms: Date.now() - start4,
  });
}

// -----------------------------------------------------------------------------
// Test 5: Non-zero exit codes are handled
// SKIP: exec() not implemented yet
// -----------------------------------------------------------------------------
const test5 = "exec-exit-code";
logTest(test5, "running");
const start5 = Date.now();

try {
  debug("Test 5: Non-zero exit codes are handled");

  if (typeof globalThis.exec !== "function") {
    logTest(test5, "skip", {
      error: "exec() is not yet implemented - skipping test",
      duration_ms: Date.now() - start5,
    });
  } else {
    // Test: command that exits with non-zero
    try {
      await globalThis.exec("exit 1");
      // If we get here, exec didn't throw on non-zero exit
      logTest(test5, "fail", {
        error: "exec() should throw or indicate failure on non-zero exit code",
        duration_ms: Date.now() - start5,
      });
    } catch (execError) {
      // Expected: exec should throw on non-zero exit
      logTest(test5, "pass", {
        result: "Correctly throws on non-zero exit code",
        duration_ms: Date.now() - start5,
      });
    }
  }
} catch (err) {
  logTest(test5, "fail", {
    error: String(err),
    duration_ms: Date.now() - start5,
  });
}

// -----------------------------------------------------------------------------
// Test 6: Invalid commands are handled gracefully
// SKIP: exec() not implemented yet
// -----------------------------------------------------------------------------
const test6 = "exec-error-handling";
logTest(test6, "running");
const start6 = Date.now();

try {
  debug("Test 6: Invalid commands are handled gracefully");

  if (typeof globalThis.exec !== "function") {
    logTest(test6, "skip", {
      error: "exec() is not yet implemented - skipping test",
      duration_ms: Date.now() - start6,
    });
  } else {
    // Test: non-existent command
    try {
      await globalThis.exec("this_command_does_not_exist_12345");
      // If we get here, exec didn't throw
      logTest(test6, "fail", {
        error: "exec() should throw on invalid command",
        duration_ms: Date.now() - start6,
      });
    } catch (execError) {
      // Expected: exec should throw for invalid commands
      logTest(test6, "pass", {
        result: "Correctly throws on invalid command",
        duration_ms: Date.now() - start6,
      });
    }
  }
} catch (err) {
  logTest(test6, "fail", {
    error: String(err),
    duration_ms: Date.now() - start6,
  });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug("test-exec.ts completed!");

await div(
  md(`# exec() Tests Complete

All exec() tests have been executed.

## Status: NOT IMPLEMENTED

The \`exec()\` function is **not yet implemented** in the SDK.
These tests serve as a TDD-style specification for expected behavior.

## Test Cases (Expected to Skip/Fail)

| # | Test | Description |
|---|------|-------------|
| 1 | exec-function-exists | Verify function is defined |
| 2 | exec-basic-command | Run echo command |
| 3 | exec-with-args | Run command with arguments |
| 4 | exec-returns-output | Multi-line output captured |
| 5 | exec-exit-code | Non-zero exit handling |
| 6 | exec-error-handling | Invalid command handling |

---

**Implementation Notes:**

When implementing \`exec()\`, it should:
- Accept a shell command string
- Return stdout as a string (or object with stdout/stderr/exitCode)
- Throw or reject on non-zero exit codes
- Handle timeouts (optional)

*Check the JSONL output for detailed results*

Press Escape or click to exit.`)
);

debug("test-exec.ts exiting...");

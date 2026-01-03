// Name: SDK Test - db()
// Description: Tests db() function for database read/write/delete operations

/**
 * SDK TEST: test-db.ts
 *
 * Tests the db() function for persistent JSON database operations.
 *
 * NOTE: db() is currently NOT implemented in the SDK.
 * This test serves as a TDD-style specification for expected behavior.
 * Tests will fail until db() is implemented.
 *
 * Test cases:
 * 1. db-function-exists: Verify db function is defined
 * 2. db-write-read: Write data and read it back
 * 3. db-update: Update existing data
 * 4. db-delete: Delete data
 * 5. db-non-existent: Reading non-existent database returns empty
 * 6. db-complex-data: Store complex nested objects
 * 7. db-cleanup: Clean up test data
 *
 * Expected behavior (when implemented):
 * - db(name) returns a lowdb-like database object
 * - Data persists to ~/.kenv/db/{name}.json
 * - Supports .get(), .set(), .write(), .data properties
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

debug("test-db.ts starting...");

// Declare db type for the tests (since it's not in SDK yet)
interface DbInstance {
  data: Record<string, unknown>;
  get(path: string): unknown;
  set(path: string, value: unknown): DbInstance;
  write(): Promise<void>;
}

declare global {
  // db() should be added to the SDK
  var db: ((name: string) => Promise<DbInstance>) | undefined;
}

debug(`SDK globals: db=${typeof globalThis.db}`);

const TEST_DB_NAME = "_test_sdk_db_" + Date.now();

// -----------------------------------------------------------------------------
// Test 1: Verify db function exists
// NOTE: This test is expected to FAIL until db() is implemented
// -----------------------------------------------------------------------------
const test1 = "db-function-exists";
logTest(test1, "running");
const start1 = Date.now();

try {
  debug("Test 1: Verify db function exists");

  if (typeof globalThis.db !== "function") {
    // This is the expected result until db() is implemented
    logTest(test1, "fail", {
      error: `db() is not yet implemented in the SDK. Expected a function, got ${typeof globalThis.db}`,
      expected: "function",
      actual: typeof globalThis.db,
      duration_ms: Date.now() - start1,
    });
  } else {
    logTest(test1, "pass", {
      result: { type: typeof globalThis.db },
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
// Test 2: Write data and read it back
// SKIP: db() not implemented yet
// -----------------------------------------------------------------------------
const test2 = "db-write-read";
logTest(test2, "running");
const start2 = Date.now();

try {
  debug("Test 2: Write data and read it back");

  if (typeof globalThis.db !== "function") {
    logTest(test2, "skip", {
      error: "db() is not yet implemented - skipping test",
      duration_ms: Date.now() - start2,
    });
  } else {
    const database = await globalThis.db(TEST_DB_NAME);

    // Write data
    database.set("user.name", "John");
    database.set("user.age", 30);
    await database.write();

    // Read data back
    const name = database.get("user.name");
    const age = database.get("user.age");

    if (name === "John" && age === 30) {
      logTest(test2, "pass", {
        result: { name, age },
        duration_ms: Date.now() - start2,
      });
    } else {
      logTest(test2, "fail", {
        error: "Data read does not match data written",
        expected: "name=John, age=30",
        actual: `name=${name}, age=${age}`,
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
// Test 3: Update existing data
// SKIP: db() not implemented yet
// -----------------------------------------------------------------------------
const test3 = "db-update";
logTest(test3, "running");
const start3 = Date.now();

try {
  debug("Test 3: Update existing data");

  if (typeof globalThis.db !== "function") {
    logTest(test3, "skip", {
      error: "db() is not yet implemented - skipping test",
      duration_ms: Date.now() - start3,
    });
  } else {
    const database = await globalThis.db(TEST_DB_NAME);

    // Update existing value
    database.set("user.age", 31);
    await database.write();

    // Verify update
    const age = database.get("user.age");

    if (age === 31) {
      logTest(test3, "pass", {
        result: { updatedAge: age },
        duration_ms: Date.now() - start3,
      });
    } else {
      logTest(test3, "fail", {
        error: "Update did not take effect",
        expected: "31",
        actual: String(age),
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
// Test 4: Delete data
// SKIP: db() not implemented yet
// -----------------------------------------------------------------------------
const test4 = "db-delete";
logTest(test4, "running");
const start4 = Date.now();

try {
  debug("Test 4: Delete data");

  if (typeof globalThis.db !== "function") {
    logTest(test4, "skip", {
      error: "db() is not yet implemented - skipping test",
      duration_ms: Date.now() - start4,
    });
  } else {
    const database = await globalThis.db(TEST_DB_NAME);

    // Delete by setting to undefined or using a delete method
    database.set("user.age", undefined);
    await database.write();

    // Verify deletion
    const age = database.get("user.age");

    if (age === undefined) {
      logTest(test4, "pass", {
        result: "Data deleted successfully",
        duration_ms: Date.now() - start4,
      });
    } else {
      logTest(test4, "fail", {
        error: "Delete did not remove data",
        expected: "undefined",
        actual: String(age),
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
// Test 5: Reading non-existent path returns undefined
// SKIP: db() not implemented yet
// -----------------------------------------------------------------------------
const test5 = "db-non-existent";
logTest(test5, "running");
const start5 = Date.now();

try {
  debug("Test 5: Reading non-existent path returns undefined");

  if (typeof globalThis.db !== "function") {
    logTest(test5, "skip", {
      error: "db() is not yet implemented - skipping test",
      duration_ms: Date.now() - start5,
    });
  } else {
    const database = await globalThis.db(TEST_DB_NAME);

    // Read non-existent path
    const nonExistent = database.get("this.path.does.not.exist");

    if (nonExistent === undefined) {
      logTest(test5, "pass", {
        result: "Non-existent path returns undefined",
        duration_ms: Date.now() - start5,
      });
    } else {
      logTest(test5, "fail", {
        error: "Non-existent path should return undefined",
        expected: "undefined",
        actual: String(nonExistent),
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
// Test 6: Store complex nested objects
// SKIP: db() not implemented yet
// -----------------------------------------------------------------------------
const test6 = "db-complex-data";
logTest(test6, "running");
const start6 = Date.now();

try {
  debug("Test 6: Store complex nested objects");

  if (typeof globalThis.db !== "function") {
    logTest(test6, "skip", {
      error: "db() is not yet implemented - skipping test",
      duration_ms: Date.now() - start6,
    });
  } else {
    const database = await globalThis.db(TEST_DB_NAME);

    const complexData = {
      users: [
        { id: 1, name: "Alice", tags: ["admin", "user"] },
        { id: 2, name: "Bob", tags: ["user"] },
      ],
      settings: {
        theme: "dark",
        notifications: {
          email: true,
          push: false,
        },
      },
    };

    database.set("complex", complexData);
    await database.write();

    // Read back and verify
    const retrieved = database.get("complex") as typeof complexData;

    if (
      retrieved &&
      retrieved.users?.length === 2 &&
      retrieved.users[0]?.name === "Alice" &&
      retrieved.settings?.notifications?.email === true
    ) {
      logTest(test6, "pass", {
        result: {
          userCount: retrieved.users.length,
          theme: retrieved.settings.theme,
        },
        duration_ms: Date.now() - start6,
      });
    } else {
      logTest(test6, "fail", {
        error: "Complex data not stored/retrieved correctly",
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
// Test 7: Cleanup test data
// SKIP: db() not implemented yet
// -----------------------------------------------------------------------------
const test7 = "db-cleanup";
logTest(test7, "running");
const start7 = Date.now();

try {
  debug("Test 7: Cleanup test data");

  if (typeof globalThis.db !== "function") {
    logTest(test7, "skip", {
      error: "db() is not yet implemented - skipping test",
      duration_ms: Date.now() - start7,
    });
  } else {
    // Clean up: delete the test database file
    // This might require a separate API or file system access
    const dbPath = kenvPath("db", TEST_DB_NAME + ".json");
    debug(`Would delete: ${dbPath}`);

    // For now, just clear the data
    const database = await globalThis.db(TEST_DB_NAME);
    database.data = {};
    await database.write();

    logTest(test7, "pass", {
      result: "Test data cleared",
      duration_ms: Date.now() - start7,
    });
  }
} catch (err) {
  logTest(test7, "fail", {
    error: String(err),
    duration_ms: Date.now() - start7,
  });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug("test-db.ts completed!");

await div(
  md(`# db() Tests Complete

All db() tests have been executed.

## Status: NOT IMPLEMENTED

The \`db()\` function is **not yet implemented** in the SDK.
These tests serve as a TDD-style specification for expected behavior.

## Test Cases (Expected to Skip/Fail)

| # | Test | Description |
|---|------|-------------|
| 1 | db-function-exists | Verify function is defined |
| 2 | db-write-read | Write and read data |
| 3 | db-update | Update existing data |
| 4 | db-delete | Delete data |
| 5 | db-non-existent | Non-existent path handling |
| 6 | db-complex-data | Complex nested objects |
| 7 | db-cleanup | Clean up test data |

---

**Implementation Notes:**

When implementing \`db()\`, it should:
- Accept a database name (string)
- Return a lowdb-like object with:
  - \`data\`: Direct access to the database object
  - \`get(path)\`: Get value at dot-notation path
  - \`set(path, value)\`: Set value at path
  - \`write()\`: Persist changes to disk
- Store data in \`~/.kenv/db/{name}.json\`

*Check the JSONL output for detailed results*

Press Escape or click to exit.`)
);

debug("test-db.ts exiting...");

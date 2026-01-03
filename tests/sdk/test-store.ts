// Name: SDK Test - store()
// Description: Tests store() function for persistent key-value storage

/**
 * SDK TEST: test-store.ts
 *
 * Tests the store() function for simple persistent key-value storage.
 *
 * NOTE: store() is currently NOT implemented in the SDK.
 * This test serves as a TDD-style specification for expected behavior.
 * Tests will fail until store() is implemented.
 *
 * Test cases:
 * 1. store-function-exists: Verify store function is defined
 * 2. store-set-get: Set and get a value
 * 3. store-update: Update an existing value
 * 4. store-delete: Delete a value
 * 5. store-non-existent: Getting non-existent key returns undefined
 * 6. store-multiple-keys: Store multiple independent keys
 * 7. store-complex-values: Store objects and arrays
 * 8. store-cleanup: Clean up test data
 *
 * Expected behavior (when implemented):
 * - store.get(key) returns stored value
 * - store.set(key, value) stores a value
 * - store.delete(key) removes a value
 * - Data persists across script runs
 * - Stored in ~/.kenv/store.json or similar
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

debug("test-store.ts starting...");

// Declare store type for the tests (since it's not in SDK yet)
interface StoreAPI {
  get<T = unknown>(key: string): Promise<T | undefined>;
  set<T = unknown>(key: string, value: T): Promise<void>;
  delete(key: string): Promise<boolean>;
  clear(): Promise<void>;
}

declare global {
  // store should be added to the SDK
  var store: StoreAPI | undefined;
}

debug(`SDK globals: store=${typeof globalThis.store}`);

const TEST_KEY_PREFIX = "_test_sdk_store_" + Date.now() + "_";

// -----------------------------------------------------------------------------
// Test 1: Verify store object exists
// NOTE: This test is expected to FAIL until store is implemented
// -----------------------------------------------------------------------------
const test1 = "store-function-exists";
logTest(test1, "running");
const start1 = Date.now();

try {
  debug("Test 1: Verify store object exists");

  if (typeof globalThis.store !== "object" || globalThis.store === null) {
    // This is the expected result until store is implemented
    logTest(test1, "fail", {
      error: `store is not yet implemented in the SDK. Expected an object, got ${typeof globalThis.store}`,
      expected: "object",
      actual: typeof globalThis.store,
      duration_ms: Date.now() - start1,
    });
  } else {
    const hasGet = typeof globalThis.store.get === "function";
    const hasSet = typeof globalThis.store.set === "function";
    const hasDelete = typeof globalThis.store.delete === "function";

    if (hasGet && hasSet && hasDelete) {
      logTest(test1, "pass", {
        result: { methods: ["get", "set", "delete"] },
        duration_ms: Date.now() - start1,
      });
    } else {
      logTest(test1, "fail", {
        error: "store is missing required methods",
        expected: "get, set, delete",
        actual: `get=${hasGet}, set=${hasSet}, delete=${hasDelete}`,
        duration_ms: Date.now() - start1,
      });
    }
  }
} catch (err) {
  logTest(test1, "fail", {
    error: String(err),
    duration_ms: Date.now() - start1,
  });
}

// -----------------------------------------------------------------------------
// Test 2: Set and get a value
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test2 = "store-set-get";
logTest(test2, "running");
const start2 = Date.now();

try {
  debug("Test 2: Set and get a value");

  if (!globalThis.store || typeof globalThis.store.get !== "function") {
    logTest(test2, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start2,
    });
  } else {
    const key = TEST_KEY_PREFIX + "greeting";
    const value = "Hello, World!";

    await globalThis.store.set(key, value);
    const retrieved = await globalThis.store.get<string>(key);

    if (retrieved === value) {
      logTest(test2, "pass", {
        result: { key, value: retrieved },
        duration_ms: Date.now() - start2,
      });
    } else {
      logTest(test2, "fail", {
        error: "Retrieved value does not match stored value",
        expected: value,
        actual: String(retrieved),
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
// Test 3: Update an existing value
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test3 = "store-update";
logTest(test3, "running");
const start3 = Date.now();

try {
  debug("Test 3: Update an existing value");

  if (!globalThis.store || typeof globalThis.store.get !== "function") {
    logTest(test3, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start3,
    });
  } else {
    const key = TEST_KEY_PREFIX + "counter";

    await globalThis.store.set(key, 1);
    await globalThis.store.set(key, 2);
    await globalThis.store.set(key, 3);

    const retrieved = await globalThis.store.get<number>(key);

    if (retrieved === 3) {
      logTest(test3, "pass", {
        result: { finalValue: retrieved },
        duration_ms: Date.now() - start3,
      });
    } else {
      logTest(test3, "fail", {
        error: "Value was not updated correctly",
        expected: "3",
        actual: String(retrieved),
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
// Test 4: Delete a value
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test4 = "store-delete";
logTest(test4, "running");
const start4 = Date.now();

try {
  debug("Test 4: Delete a value");

  if (!globalThis.store || typeof globalThis.store.delete !== "function") {
    logTest(test4, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start4,
    });
  } else {
    const key = TEST_KEY_PREFIX + "toDelete";

    await globalThis.store.set(key, "temporary");
    const beforeDelete = await globalThis.store.get(key);

    const deleted = await globalThis.store.delete(key);
    const afterDelete = await globalThis.store.get(key);

    if (
      beforeDelete === "temporary" &&
      deleted === true &&
      afterDelete === undefined
    ) {
      logTest(test4, "pass", {
        result: "Value deleted successfully",
        duration_ms: Date.now() - start4,
      });
    } else {
      logTest(test4, "fail", {
        error: "Delete did not work correctly",
        actual: `before=${beforeDelete}, deleted=${deleted}, after=${afterDelete}`,
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
// Test 5: Getting non-existent key returns undefined
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test5 = "store-non-existent";
logTest(test5, "running");
const start5 = Date.now();

try {
  debug("Test 5: Getting non-existent key returns undefined");

  if (!globalThis.store || typeof globalThis.store.get !== "function") {
    logTest(test5, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start5,
    });
  } else {
    const nonExistentKey = TEST_KEY_PREFIX + "this_key_does_not_exist_12345";
    const value = await globalThis.store.get(nonExistentKey);

    if (value === undefined) {
      logTest(test5, "pass", {
        result: "Non-existent key returns undefined",
        duration_ms: Date.now() - start5,
      });
    } else {
      logTest(test5, "fail", {
        error: "Non-existent key should return undefined",
        expected: "undefined",
        actual: String(value),
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
// Test 6: Store multiple independent keys
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test6 = "store-multiple-keys";
logTest(test6, "running");
const start6 = Date.now();

try {
  debug("Test 6: Store multiple independent keys");

  if (!globalThis.store || typeof globalThis.store.get !== "function") {
    logTest(test6, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start6,
    });
  } else {
    const key1 = TEST_KEY_PREFIX + "key1";
    const key2 = TEST_KEY_PREFIX + "key2";
    const key3 = TEST_KEY_PREFIX + "key3";

    await globalThis.store.set(key1, "value1");
    await globalThis.store.set(key2, "value2");
    await globalThis.store.set(key3, "value3");

    const val1 = await globalThis.store.get<string>(key1);
    const val2 = await globalThis.store.get<string>(key2);
    const val3 = await globalThis.store.get<string>(key3);

    if (val1 === "value1" && val2 === "value2" && val3 === "value3") {
      logTest(test6, "pass", {
        result: { key1: val1, key2: val2, key3: val3 },
        duration_ms: Date.now() - start6,
      });
    } else {
      logTest(test6, "fail", {
        error: "Multiple keys not stored correctly",
        actual: `val1=${val1}, val2=${val2}, val3=${val3}`,
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
// Test 7: Store objects and arrays
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test7 = "store-complex-values";
logTest(test7, "running");
const start7 = Date.now();

try {
  debug("Test 7: Store objects and arrays");

  if (!globalThis.store || typeof globalThis.store.get !== "function") {
    logTest(test7, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start7,
    });
  } else {
    const objKey = TEST_KEY_PREFIX + "object";
    const arrKey = TEST_KEY_PREFIX + "array";

    const obj = { name: "Test", count: 42, nested: { a: 1, b: 2 } };
    const arr = [1, "two", { three: 3 }, [4, 5]];

    await globalThis.store.set(objKey, obj);
    await globalThis.store.set(arrKey, arr);

    const retrievedObj = await globalThis.store.get<typeof obj>(objKey);
    const retrievedArr = await globalThis.store.get<typeof arr>(arrKey);

    const objMatch =
      retrievedObj &&
      retrievedObj.name === "Test" &&
      retrievedObj.count === 42 &&
      retrievedObj.nested?.a === 1;

    const arrMatch =
      retrievedArr &&
      Array.isArray(retrievedArr) &&
      retrievedArr.length === 4 &&
      retrievedArr[0] === 1 &&
      retrievedArr[1] === "two";

    if (objMatch && arrMatch) {
      logTest(test7, "pass", {
        result: {
          object: { name: retrievedObj?.name, count: retrievedObj?.count },
          arrayLength: retrievedArr?.length,
        },
        duration_ms: Date.now() - start7,
      });
    } else {
      logTest(test7, "fail", {
        error: "Complex values not stored/retrieved correctly",
        duration_ms: Date.now() - start7,
      });
    }
  }
} catch (err) {
  logTest(test7, "fail", {
    error: String(err),
    duration_ms: Date.now() - start7,
  });
}

// -----------------------------------------------------------------------------
// Test 8: Cleanup test data
// SKIP: store not implemented yet
// -----------------------------------------------------------------------------
const test8 = "store-cleanup";
logTest(test8, "running");
const start8 = Date.now();

try {
  debug("Test 8: Cleanup test data");

  if (!globalThis.store || typeof globalThis.store.delete !== "function") {
    logTest(test8, "skip", {
      error: "store is not yet implemented - skipping test",
      duration_ms: Date.now() - start8,
    });
  } else {
    // Delete all test keys
    const keysToDelete = [
      "greeting",
      "counter",
      "toDelete",
      "key1",
      "key2",
      "key3",
      "object",
      "array",
    ];

    for (const suffix of keysToDelete) {
      await globalThis.store.delete(TEST_KEY_PREFIX + suffix);
    }

    logTest(test8, "pass", {
      result: { deletedKeys: keysToDelete.length },
      duration_ms: Date.now() - start8,
    });
  }
} catch (err) {
  logTest(test8, "fail", {
    error: String(err),
    duration_ms: Date.now() - start8,
  });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug("test-store.ts completed!");

await div(
  md(`# store Tests Complete

All store tests have been executed.

## Status: NOT IMPLEMENTED

The \`store\` API is **not yet implemented** in the SDK.
These tests serve as a TDD-style specification for expected behavior.

## Test Cases (Expected to Skip/Fail)

| # | Test | Description |
|---|------|-------------|
| 1 | store-function-exists | Verify store object is defined |
| 2 | store-set-get | Set and get a value |
| 3 | store-update | Update existing value |
| 4 | store-delete | Delete a value |
| 5 | store-non-existent | Non-existent key handling |
| 6 | store-multiple-keys | Multiple independent keys |
| 7 | store-complex-values | Objects and arrays |
| 8 | store-cleanup | Clean up test data |

---

**Implementation Notes:**

When implementing \`store\`, it should:
- Be a global object with get/set/delete methods
- Persist data to disk (e.g., \`~/.kenv/store.json\`)
- Support any JSON-serializable values
- Methods:
  - \`store.get<T>(key): Promise<T | undefined>\`
  - \`store.set(key, value): Promise<void>\`
  - \`store.delete(key): Promise<boolean>\`
  - \`store.clear(): Promise<void>\`

*Check the JSONL output for detailed results*

Press Escape or click to exit.`)
);

debug("test-store.ts exiting...");

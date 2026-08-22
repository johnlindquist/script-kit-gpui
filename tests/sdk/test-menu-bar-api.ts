// Name: SDK Test - Menu Bar API
// Description: Unit tests for Menu Bar SDK functions (getMenuBar, executeMenuAction)

/**
 * SDK TEST: test-menu-bar-api.ts
 *
 * Tests the Menu Bar API functions:
 * - getMenuBar(bundleId?: string): Promise<MenuBarItem[]>
 * - executeMenuAction(bundleId: string, menuPath: string[]): Promise<void>
 *
 * Test categories:
 * 1. Function existence - Verify SDK functions are available
 * 2. Type verification - Verify return types match SDK types
 * 3. Error handling - Test error cases (wrong bundle_id, etc.)
 * 4. Accessibility checks - Verify behavior when permissions are missing
 *
 * Note: These tests focus on the SDK interface. Tests that require
 * accessibility permissions will be marked [SKIP] if permission is not granted.
 */

import '../../scripts/kit-sdk';

// Import types from kit-sdk
import type { MenuBarItem } from '../../scripts/kit-sdk';

// =============================================================================
// Test Infrastructure
// =============================================================================

interface TestResult {
  test: string;
  status: 'running' | 'pass' | 'fail' | 'skip';
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
  expected?: string;
  reason?: string;
}

function logTest(
  name: string,
  status: TestResult['status'],
  extra?: Partial<TestResult>,
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

debug('test-menu-bar-api.ts starting...');
debug(`SDK globals: getMenuBar=${typeof getMenuBar}, executeMenuAction=${typeof executeMenuAction}`);

// -----------------------------------------------------------------------------
// Test 1: Function existence - getMenuBar
// -----------------------------------------------------------------------------
const test1 = 'getMenuBar-exists';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug('Test 1: Verify getMenuBar function exists');

  if (typeof getMenuBar !== 'function') {
    throw new Error(`Expected getMenuBar to be a function, got ${typeof getMenuBar}`);
  }

  debug('Test 1 passed - getMenuBar is a function');
  logTest(test1, 'pass', {
    result: { type: typeof getMenuBar },
    duration_ms: Date.now() - start1,
  });
} catch (err) {
  logTest(test1, 'fail', {
    error: String(err),
    duration_ms: Date.now() - start1,
  });
}

// -----------------------------------------------------------------------------
// Test 2: Function existence - executeMenuAction
// -----------------------------------------------------------------------------
const test2 = 'executeMenuAction-exists';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug('Test 2: Verify executeMenuAction function exists');

  if (typeof executeMenuAction !== 'function') {
    throw new Error(`Expected executeMenuAction to be a function, got ${typeof executeMenuAction}`);
  }

  debug('Test 2 passed - executeMenuAction is a function');
  logTest(test2, 'pass', {
    result: { type: typeof executeMenuAction },
    duration_ms: Date.now() - start2,
  });
} catch (err) {
  logTest(test2, 'fail', {
    error: String(err),
    duration_ms: Date.now() - start2,
  });
}

// -----------------------------------------------------------------------------
// Test 3: getMenuBar returns Promise
// -----------------------------------------------------------------------------
const test3 = 'getMenuBar-returns-promise';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug('Test 3: Verify getMenuBar returns a Promise');

  const result = getMenuBar();

  // Check if result is a Promise-like object
  if (!result || typeof result.then !== 'function') {
    throw new Error(`Expected getMenuBar() to return a Promise, got ${typeof result}`);
  }

  // Wait for the promise to resolve to avoid unhandled rejection
  await result.catch(() => {
    // Ignore errors (e.g., accessibility permission) - we're just testing the return type
  });

  debug('Test 3 passed - getMenuBar returns a Promise');
  logTest(test3, 'pass', {
    result: { isPromise: true },
    duration_ms: Date.now() - start3,
  });
} catch (err) {
  logTest(test3, 'fail', {
    error: String(err),
    duration_ms: Date.now() - start3,
  });
}

// -----------------------------------------------------------------------------
// Test 4: getMenuBar with no args - frontmost app
// -----------------------------------------------------------------------------
const test4 = 'getMenuBar-no-args';
logTest(test4, 'running');
const start4 = Date.now();

let menuItems: MenuBarItem[] = [];

try {
  debug('Test 4: getMenuBar() with no arguments');

  menuItems = await getMenuBar();

  // Should return an array
  if (!Array.isArray(menuItems)) {
    throw new Error(`Expected array, got ${typeof menuItems}`);
  }

  debug(`Test 4 passed - got ${menuItems.length} menu items`);
  logTest(test4, 'pass', {
    result: { count: menuItems.length },
    duration_ms: Date.now() - start4,
  });
} catch (err) {
  const errorMessage = String(err);
  if (errorMessage.toLowerCase().includes('accessibility') ||
      errorMessage.toLowerCase().includes('permission')) {
    debug('[SKIP] Test 4 - accessibility permission required');
    logTest(test4, 'skip', {
      reason: 'Accessibility permission required',
      duration_ms: Date.now() - start4,
    });
  } else {
    logTest(test4, 'fail', {
      error: errorMessage,
      duration_ms: Date.now() - start4,
    });
  }
}

// -----------------------------------------------------------------------------
// Test 5: getMenuBar with bundleId - specific app
// -----------------------------------------------------------------------------
const test5 = 'getMenuBar-with-bundleId';
logTest(test5, 'running');
const start5 = Date.now();

try {
  debug('Test 5: getMenuBar() with bundleId');

  // Use Finder as it should always be running on macOS
  const finderMenus = await getMenuBar('com.apple.finder');

  if (!Array.isArray(finderMenus)) {
    throw new Error(`Expected array, got ${typeof finderMenus}`);
  }

  debug(`Test 5 passed - got ${finderMenus.length} Finder menu items`);
  logTest(test5, 'pass', {
    result: { 
      bundleId: 'com.apple.finder',
      count: finderMenus.length 
    },
    duration_ms: Date.now() - start5,
  });
} catch (err) {
  const errorMessage = String(err);
  if (errorMessage.toLowerCase().includes('accessibility') ||
      errorMessage.toLowerCase().includes('permission') ||
      errorMessage.toLowerCase().includes('not running') ||
      errorMessage.toLowerCase().includes('not found')) {
    debug('[SKIP] Test 5 - accessibility permission or app not accessible');
    logTest(test5, 'skip', {
      reason: 'Accessibility permission required or app not accessible',
      duration_ms: Date.now() - start5,
    });
  } else {
    logTest(test5, 'fail', {
      error: errorMessage,
      duration_ms: Date.now() - start5,
    });
  }
}

// -----------------------------------------------------------------------------
// Test 6: getMenuBar with invalid bundleId - error handling
// -----------------------------------------------------------------------------
const test6 = 'getMenuBar-invalid-bundleId';
logTest(test6, 'running');
const start6 = Date.now();

try {
  debug('Test 6: getMenuBar() with invalid bundleId');

  // Use a bundle ID that definitely doesn't exist
  const result = await getMenuBar('com.fake.nonexistent.app.12345');

  // If we get here, it returned something - should be empty array or error
  if (Array.isArray(result)) {
    debug(`Test 6 - got empty array for invalid bundleId (expected behavior)`);
    logTest(test6, 'pass', {
      result: { 
        bundleId: 'com.fake.nonexistent.app.12345',
        count: result.length,
        note: 'Returns empty array for invalid bundleId'
      },
      duration_ms: Date.now() - start6,
    });
  } else {
    throw new Error(`Unexpected result type: ${typeof result}`);
  }
} catch (err) {
  const errorMessage = String(err);
  // An error is also acceptable behavior for invalid bundleId
  if (errorMessage.toLowerCase().includes('not found') ||
      errorMessage.toLowerCase().includes('not running') ||
      errorMessage.toLowerCase().includes('invalid')) {
    debug('Test 6 passed - error thrown for invalid bundleId');
    logTest(test6, 'pass', {
      result: { 
        bundleId: 'com.fake.nonexistent.app.12345',
        error: errorMessage,
        note: 'Throws error for invalid bundleId'
      },
      duration_ms: Date.now() - start6,
    });
  } else if (errorMessage.toLowerCase().includes('accessibility') ||
             errorMessage.toLowerCase().includes('permission')) {
    debug('[SKIP] Test 6 - accessibility permission required');
    logTest(test6, 'skip', {
      reason: 'Accessibility permission required',
      duration_ms: Date.now() - start6,
    });
  } else {
    logTest(test6, 'fail', {
      error: errorMessage,
      duration_ms: Date.now() - start6,
    });
  }
}

// -----------------------------------------------------------------------------
// Test 7: executeMenuAction returns Promise
// -----------------------------------------------------------------------------
const test7 = 'executeMenuAction-returns-promise';
logTest(test7, 'running');
const start7 = Date.now();

try {
  debug('Test 7: Verify executeMenuAction returns a Promise');

  // Call with placeholder args - will fail but should return a Promise
  const result = executeMenuAction('com.fake.app', ['File', 'Fake']);

  if (!result || typeof result.then !== 'function') {
    throw new Error(`Expected executeMenuAction() to return a Promise, got ${typeof result}`);
  }

  // Wait for the promise to reject (expected since the app doesn't exist)
  await result.catch(() => {
    // Ignore errors - we're just testing the return type
  });

  debug('Test 7 passed - executeMenuAction returns a Promise');
  logTest(test7, 'pass', {
    result: { isPromise: true },
    duration_ms: Date.now() - start7,
  });
} catch (err) {
  logTest(test7, 'fail', {
    error: String(err),
    duration_ms: Date.now() - start7,
  });
}

// -----------------------------------------------------------------------------
// Test 8: executeMenuAction with invalid bundleId - error handling
// -----------------------------------------------------------------------------
const test8 = 'executeMenuAction-invalid-bundleId';
logTest(test8, 'running');
const start8 = Date.now();

try {
  debug('Test 8: executeMenuAction() with invalid bundleId should reject');

  await executeMenuAction('com.fake.nonexistent.app.12345', ['File', 'New']);

  logTest(test8, 'fail', {
    error: 'executeMenuAction must reject an invalid bundle identifier.',
    duration_ms: Date.now() - start8,
  });
} catch (err) {
  const errorMessage = String(err);
  // An error is expected behavior for invalid bundleId
  if (errorMessage.toLowerCase().includes('not found') ||
      errorMessage.toLowerCase().includes('not running') ||
      errorMessage.toLowerCase().includes('invalid') ||
      errorMessage.toLowerCase().includes('failed')) {
    debug('Test 8 passed - error thrown for invalid bundleId');
    logTest(test8, 'pass', {
      result: { 
        error: errorMessage,
        note: 'Correctly throws error for invalid bundleId'
      },
      duration_ms: Date.now() - start8,
    });
  } else if (errorMessage.toLowerCase().includes('accessibility') ||
             errorMessage.toLowerCase().includes('permission')) {
    debug('[SKIP] Test 8 - accessibility permission required');
    logTest(test8, 'skip', {
      reason: 'Accessibility permission required',
      duration_ms: Date.now() - start8,
    });
  } else {
    // Any other error is also acceptable - it means the function rejects invalid input
    debug(`Test 8 passed - error: ${errorMessage}`);
    logTest(test8, 'pass', {
      result: { error: errorMessage },
      duration_ms: Date.now() - start8,
    });
  }
}

// -----------------------------------------------------------------------------
// Test 9: executeMenuAction with invalid menuPath - error handling
// -----------------------------------------------------------------------------
const test9 = 'executeMenuAction-invalid-menuPath';
logTest(test9, 'running');
const start9 = Date.now();

try {
  debug('Test 9: executeMenuAction() with invalid menuPath');

  // Use Finder with an invalid menu path
  await executeMenuAction('com.apple.finder', ['NonExistent', 'Menu', 'Path']);

  logTest(test9, 'fail', {
    error: 'executeMenuAction must reject an invalid menu path.',
    duration_ms: Date.now() - start9,
  });
} catch (err) {
  const errorMessage = String(err);
  // An error is expected behavior for invalid menu path
  if (errorMessage.toLowerCase().includes('not found') ||
      errorMessage.toLowerCase().includes('menu') ||
      errorMessage.toLowerCase().includes('failed')) {
    debug('Test 9 passed - error thrown for invalid menu path');
    logTest(test9, 'pass', {
      result: { 
        error: errorMessage,
        note: 'Correctly throws error for invalid menu path'
      },
      duration_ms: Date.now() - start9,
    });
  } else if (errorMessage.toLowerCase().includes('accessibility') ||
             errorMessage.toLowerCase().includes('permission')) {
    debug('[SKIP] Test 9 - accessibility permission required');
    logTest(test9, 'skip', {
      reason: 'Accessibility permission required',
      duration_ms: Date.now() - start9,
    });
  } else {
    // Any other error is also acceptable
    debug(`Test 9 passed - error: ${errorMessage}`);
    logTest(test9, 'pass', {
      result: { error: errorMessage },
      duration_ms: Date.now() - start9,
    });
  }
}

// -----------------------------------------------------------------------------
// Test 10: MenuBarItem type verification
// -----------------------------------------------------------------------------
const test10 = 'menuBarItem-type-verification';
logTest(test10, 'running');
const start10 = Date.now();

try {
  debug('Test 10: Verify MenuBarItem structure');

  if (menuItems.length === 0) {
    logTest(test10, 'fail', {
      error: 'The deterministic native-menu fixture must include a typed menu tree.',
      duration_ms: Date.now() - start10,
    });
  } else {
    // Recursively verify all items match MenuBarItem interface
    function verifyMenuItemType(item: MenuBarItem, path: string[]): void {
      const itemPath = [...path, item.title];
      
      // Required: title
      if (typeof item.title !== 'string') {
        throw new Error(`${itemPath.join(' > ')}: title should be string, got ${typeof item.title}`);
      }
      
      // Required: enabled
      if (typeof item.enabled !== 'boolean') {
        throw new Error(`${itemPath.join(' > ')}: enabled should be boolean, got ${typeof item.enabled}`);
      }
      
      // Required: children (array)
      if (!Array.isArray(item.children)) {
        throw new Error(`${itemPath.join(' > ')}: children should be array, got ${typeof item.children}`);
      }
      
      // Required: menuPath (array)
      if (!Array.isArray(item.menuPath)) {
        throw new Error(`${itemPath.join(' > ')}: menuPath should be array, got ${typeof item.menuPath}`);
      }
      
      // Optional: shortcut
      if (item.shortcut !== undefined && typeof item.shortcut !== 'string') {
        throw new Error(`${itemPath.join(' > ')}: shortcut should be string or undefined, got ${typeof item.shortcut}`);
      }
      
      // Recursively verify children
      for (const child of item.children) {
        verifyMenuItemType(child, itemPath);
      }
    }

    let totalItems = 0;
    for (const item of menuItems) {
      verifyMenuItemType(item, []);
      totalItems++;
      // Count nested items
      function countItems(items: MenuBarItem[]): number {
        let count = 0;
        for (const i of items) {
          count++;
          count += countItems(i.children);
        }
        return count;
      }
      totalItems += countItems(item.children) - 1; // -1 because we already counted the root
    }

    debug(`Test 10 passed - verified ${totalItems} menu items match MenuBarItem interface`);
    logTest(test10, 'pass', {
      result: { 
        itemsVerified: totalItems,
        topLevelMenus: menuItems.length
      },
      duration_ms: Date.now() - start10,
    });
  }
} catch (err) {
  logTest(test10, 'fail', {
    error: String(err),
    duration_ms: Date.now() - start10,
  });
}

// -----------------------------------------------------------------------------
// Test 11: Empty menuPath handling
// -----------------------------------------------------------------------------
const test11 = 'executeMenuAction-empty-menuPath';
logTest(test11, 'running');
const start11 = Date.now();

try {
  debug('Test 11: executeMenuAction() with empty menuPath');

  await executeMenuAction('com.apple.finder', []);

  logTest(test11, 'fail', {
    error: 'executeMenuAction must reject an empty menu path.',
    duration_ms: Date.now() - start11,
  });
} catch (err) {
  const errorMessage = String(err);
  // An error is expected for empty menu path
  debug(`Test 11 passed - error for empty menuPath: ${errorMessage}`);
  logTest(test11, 'pass', {
    result: { 
      error: errorMessage,
      note: 'Correctly handles empty menuPath'
    },
    duration_ms: Date.now() - start11,
  });
}

// -----------------------------------------------------------------------------
// Test 12: hasAccessibilityPermission check (if available)
// -----------------------------------------------------------------------------
const test12 = 'accessibility-permission-check';
logTest(test12, 'running');
const start12 = Date.now();

try {
  debug('Test 12: Check hasAccessibilityPermission');

  if (typeof hasAccessibilityPermission !== 'function') {
    debug('[SKIP] Test 12 - hasAccessibilityPermission function not available');
    logTest(test12, 'skip', {
      reason: 'hasAccessibilityPermission function not available',
      duration_ms: Date.now() - start12,
    });
  } else {
    const hasPermission = await hasAccessibilityPermission();

    debug(`Test 12 completed - hasAccessibilityPermission: ${hasPermission}`);
    logTest(test12, 'pass', {
      result: { 
        hasPermission,
        note: hasPermission 
          ? 'Accessibility permission granted - all tests should run'
          : 'Accessibility permission not granted - some tests will skip'
      },
      duration_ms: Date.now() - start12,
    });
  }
} catch (err) {
  logTest(test12, 'fail', {
    error: String(err),
    duration_ms: Date.now() - start12,
  });
}

// -----------------------------------------------------------------------------
// Summary and Exit
// -----------------------------------------------------------------------------
debug('test-menu-bar-api.ts completed!');
debug('All 12 tests executed. Check JSONL output for detailed results.');

// Exit cleanly for autonomous testing
exit(0);

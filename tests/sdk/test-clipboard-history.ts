// Name: SDK Test - Clipboard History
// Description: Tests clipboard history management functions

/**
 * SDK TEST: test-clipboard-history.ts
 * 
 * Tests clipboard history functions.
 * 
 * Test cases:
 * 1. clipboard-history-list: Get clipboard history (should return array)
 * 2. clipboard-history-copy-and-list: Copy to clipboard and verify in history
 * 3. clipboard-history-pin: Pin a clipboard entry
 * 4. clipboard-history-unpin: Unpin a clipboard entry
 * 5. clipboard-history-remove: Remove a specific entry
 * 6. clipboard-history-clear: Clear all non-pinned entries
 * 
 * Expected behavior:
 * - clipboardHistory() returns array of ClipboardHistoryEntry
 * - copy() adds entries to clipboard history
 * - Pin/unpin operations update entry state
 * - Remove/clear operations delete entries
 */

// SDK is loaded via --preload, no import needed

// =============================================================================
// Helpers
// =============================================================================

// Isolated auto-submit fixtures update immediately; retain propagation waits
// only for deliberately requested host-backed runs.
const wait = async (ms: number): Promise<void> => {
  if (process.env.SDK_TEST_AUTOSUBMIT !== '1') {
    await new Promise<void>((resolve) => setTimeout(resolve, ms));
  }
};

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
  actual?: string;
}

function logTest(name: string, status: TestResult['status'], extra?: Partial<TestResult>) {
  const result: TestResult = {
    test: name,
    status,
    timestamp: new Date().toISOString(),
    ...extra
  };
  console.log(JSON.stringify(result));
}

function debug(msg: string) {
  console.error(`[TEST] ${msg}`);
}

// =============================================================================
// Tests
// =============================================================================

debug('test-clipboard-history.ts starting...');
debug(`SDK globals: clipboardHistory=${typeof clipboardHistory}, copy=${typeof copy}`);

// -----------------------------------------------------------------------------
// Test 1: Get clipboard history (should return array)
// -----------------------------------------------------------------------------
const test1 = 'clipboard-history-list';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug('Test 1: Get clipboard history');
  
  const history = await clipboardHistory();
  
  debug(`clipboardHistory() returned: ${JSON.stringify(history).slice(0, 200)}...`);
  
  const isArray = Array.isArray(history);
  
  if (isArray) {
    logTest(test1, 'pass', { 
      result: `Got ${history.length} entries`, 
      duration_ms: Date.now() - start1 
    });
  } else {
    logTest(test1, 'fail', { 
      error: 'clipboardHistory() did not return an array',
      actual: typeof history,
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  logTest(test1, 'fail', { error: String(err), duration_ms: Date.now() - start1 });
}

// -----------------------------------------------------------------------------
// Test 2: Copy to clipboard and verify in history
// -----------------------------------------------------------------------------
const test2 = 'clipboard-history-copy-and-list';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug('Test 2: Copy to clipboard and verify in history');
  
  const testText = `test-entry-${Date.now()}`;
  
  // Copy to clipboard
  await copy(testText);
  debug(`Copied text: "${testText}"`);
  
  // Small delay to allow clipboard history to update
  await wait(100);
  
  // Get history and check if the entry is there
  const history = await clipboardHistory();
  
  debug(`History after copy: ${history.length} entries`);
  
  const foundEntry = history.find(entry => entry.content === testText);
  
  if (foundEntry) {
    debug(`Found entry: ${JSON.stringify(foundEntry)}`);
    
    const checks = [
      typeof foundEntry.entryId === 'string' && foundEntry.entryId.length > 0,
      foundEntry.content === testText,
      foundEntry.contentType === 'text',
      typeof foundEntry.timestamp === 'string',
      typeof foundEntry.pinned === 'boolean',
    ];
    
    if (checks.every(Boolean)) {
      logTest(test2, 'pass', { 
        result: `Entry found with correct structure`, 
        duration_ms: Date.now() - start2 
      });
    } else {
      logTest(test2, 'fail', { 
        error: 'Entry structure incorrect',
        actual: JSON.stringify(foundEntry),
        duration_ms: Date.now() - start2 
      });
    }
  } else {
    logTest(test2, 'fail', {
      error: 'A copied entry must be present in deterministic clipboard history.',
      duration_ms: Date.now() - start2 
    });
  }
} catch (err) {
  logTest(test2, 'fail', { error: String(err), duration_ms: Date.now() - start2 });
}

// -----------------------------------------------------------------------------
// Test 3: Pin a clipboard entry
// -----------------------------------------------------------------------------
const test3 = 'clipboard-history-pin';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug('Test 3: Pin a clipboard entry');
  
  // First get the history to find an entry to pin
  const history = await clipboardHistory();
  
  if (history.length === 0) {
    logTest(test3, 'fail', {
      error: 'The previous copy test must leave an entry available for pinning.',
      duration_ms: Date.now() - start3 
    });
  } else {
    const entryToPin = history[0];
    debug(`Pinning entry: ${entryToPin.entryId}`);
    
    // Pin the entry
    await clipboardHistoryPin(entryToPin.entryId);
    debug('Pin command sent');
    
    // Verify the entry is now pinned
    await wait(100);
    const historyAfter = await clipboardHistory();
    const pinnedEntry = historyAfter.find(e => e.entryId === entryToPin.entryId);
    
    if (pinnedEntry && pinnedEntry.pinned === true) {
      logTest(test3, 'pass', { 
        result: `Entry ${entryToPin.entryId} successfully pinned`, 
        duration_ms: Date.now() - start3 
      });
    } else if (pinnedEntry) {
      logTest(test3, 'fail', {
        error: 'Pin completed without updating clipboard entry state.',
        duration_ms: Date.now() - start3 
      });
    } else {
      logTest(test3, 'fail', { 
        error: 'Entry not found after pinning',
        duration_ms: Date.now() - start3 
      });
    }
  }
} catch (err) {
  logTest(test3, 'fail', { error: String(err), duration_ms: Date.now() - start3 });
}

// -----------------------------------------------------------------------------
// Test 4: Unpin a clipboard entry
// -----------------------------------------------------------------------------
const test4 = 'clipboard-history-unpin';
logTest(test4, 'running');
const start4 = Date.now();

try {
  debug('Test 4: Unpin a clipboard entry');
  
  // Get the history to find a pinned entry
  const history = await clipboardHistory();
  const pinnedEntry = history.find(e => e.pinned === true);
  
  if (!pinnedEntry) {
    logTest(test4, 'fail', {
      error: 'The pin test must leave a pinned clipboard entry.',
      duration_ms: Date.now() - start4 
    });
  } else {
    debug(`Unpinning entry: ${pinnedEntry.entryId}`);
    
    // Unpin the entry
    await clipboardHistoryUnpin(pinnedEntry.entryId);
    debug('Unpin command sent');
    
    // Verify the entry is now unpinned
    await wait(100);
    const historyAfter = await clipboardHistory();
    const unpinnedEntry = historyAfter.find(e => e.entryId === pinnedEntry.entryId);
    
    if (unpinnedEntry && unpinnedEntry.pinned === false) {
      logTest(test4, 'pass', { 
        result: `Entry ${pinnedEntry.entryId} successfully unpinned`, 
        duration_ms: Date.now() - start4 
      });
    } else if (unpinnedEntry) {
      logTest(test4, 'fail', {
        error: 'Unpin completed without updating clipboard entry state.',
        duration_ms: Date.now() - start4 
      });
    } else {
      logTest(test4, 'fail', { 
        error: 'Entry not found after unpinning',
        duration_ms: Date.now() - start4 
      });
    }
  }
} catch (err) {
  logTest(test4, 'fail', { error: String(err), duration_ms: Date.now() - start4 });
}

// -----------------------------------------------------------------------------
// Test 5: Remove a specific entry
// -----------------------------------------------------------------------------
const test5 = 'clipboard-history-remove';
logTest(test5, 'running');
const start5 = Date.now();

try {
  debug('Test 5: Remove a specific entry');
  
  // First add a known entry
  const testText = `remove-test-${Date.now()}`;
  await copy(testText);
  await wait(100);
  
  // Get history and find our entry
  const historyBefore = await clipboardHistory();
  const entryToRemove = historyBefore.find(e => e.content === testText);
  
  if (!entryToRemove) {
    logTest(test5, 'fail', {
      error: 'A copied removal fixture must exist before remove is called.',
      duration_ms: Date.now() - start5 
    });
  } else {
    debug(`Removing entry: ${entryToRemove.entryId} (history has ${historyBefore.length} entries)`);
    
    // Remove the entry
    await clipboardHistoryRemove(entryToRemove.entryId);
    debug('Remove command sent');
    
    // Verify the entry is gone
    await wait(100);
    const historyAfter = await clipboardHistory();
    const removedEntry = historyAfter.find(e => e.entryId === entryToRemove.entryId);
    
    if (!removedEntry) {
      logTest(test5, 'pass', { 
        result: `Entry ${entryToRemove.entryId} successfully removed`, 
        duration_ms: Date.now() - start5 
      });
    } else {
      logTest(test5, 'fail', {
        error: 'Remove completed but the entry remained in clipboard history.',
        duration_ms: Date.now() - start5 
      });
    }
  }
} catch (err) {
  logTest(test5, 'fail', { error: String(err), duration_ms: Date.now() - start5 });
}

// -----------------------------------------------------------------------------
// Test 6: Clear all non-pinned entries
// -----------------------------------------------------------------------------
const test6 = 'clipboard-history-clear';
logTest(test6, 'running');
const start6 = Date.now();

try {
  debug('Test 6: Clear all non-pinned entries');
  
  // First add some entries
  await copy(`clear-test-1-${Date.now()}`);
  await copy(`clear-test-2-${Date.now()}`);
  await wait(100);
  
  // Get count before clear
  const historyBefore = await clipboardHistory();
  const countBefore = historyBefore.length;
  debug(`Entries before clear: ${countBefore}`);
  
  // Clear history
  await clipboardHistoryClear();
  debug('Clear command sent');
  
  // Verify entries are cleared
  await wait(100);
  const historyAfter = await clipboardHistory();
  const countAfter = historyAfter.length;
  
  // Check that pinned entries are preserved
  const pinnedPreserved = historyAfter.every(e => e.pinned === true);
  
  debug(`Entries after clear: ${countAfter}`);
  
  if (countAfter === 0 || (countAfter < countBefore && pinnedPreserved)) {
    logTest(test6, 'pass', { 
      result: `Cleared ${countBefore - countAfter} entries, ${countAfter} pinned entries preserved`, 
      duration_ms: Date.now() - start6 
    });
  } else if (countAfter === countBefore) {
    logTest(test6, 'fail', {
      error: 'Clear completed without removing non-pinned clipboard entries.',
      duration_ms: Date.now() - start6 
    });
  } else {
    logTest(test6, 'fail', { 
      error: `Unexpected state after clear: ${countAfter} entries`,
      duration_ms: Date.now() - start6 
    });
  }
} catch (err) {
  logTest(test6, 'fail', { error: String(err), duration_ms: Date.now() - start6 });
}

// -----------------------------------------------------------------------------
// Test 7: Unknown entry IDs reject instead of claiming a successful mutation.
// -----------------------------------------------------------------------------
const test7 = 'clipboard-history-unknown-id-rejects';
logTest(test7, 'running');
const start7 = Date.now();

try {
  for (const [name, action] of [
    ['pin', clipboardHistoryPin],
    ['unpin', clipboardHistoryUnpin],
    ['remove', clipboardHistoryRemove],
  ] as const) {
    let rejected = false;
    try {
      await action('synthetic-clipboard-does-not-exist');
    } catch (error) {
      rejected = String(error).includes('Unknown synthetic clipboard entry id');
    }
    if (!rejected) {
      throw new Error(`${name} must reject an unknown clipboard history entry id.`);
    }
  }

  logTest(test7, 'pass', {
    result: 'pin, unpin, and remove reject unknown synthetic entry IDs',
    duration_ms: Date.now() - start7,
  });
} catch (error) {
  logTest(test7, 'fail', { error: String(error), duration_ms: Date.now() - start7 });
}

// -----------------------------------------------------------------------------
// Test 8: Clear preserves explicitly pinned entries and removes other entries.
// -----------------------------------------------------------------------------
const test8 = 'clipboard-history-clear-preserves-pinned';
logTest(test8, 'running');
const start8 = Date.now();

try {
  await copy('synthetic-pinned-entry');
  const pinned = (await clipboardHistory()).find((entry) => entry.content === 'synthetic-pinned-entry');
  if (!pinned) {
    throw new Error('Could not prepare synthetic pinned clipboard entry.');
  }
  await clipboardHistoryPin(pinned.entryId);
  await copy('synthetic-disposable-entry');
  await clipboardHistoryClear();

  const remaining = await clipboardHistory();
  if (
    remaining.length !== 1
    || remaining[0].entryId !== pinned.entryId
    || !remaining[0].pinned
  ) {
    throw new Error(`Clear must preserve only the pinned entry: ${JSON.stringify(remaining)}`);
  }

  logTest(test8, 'pass', {
    result: { entryId: pinned.entryId, pinned: true },
    duration_ms: Date.now() - start8,
  });
} catch (error) {
  logTest(test8, 'fail', { error: String(error), duration_ms: Date.now() - start8 });
}

// -----------------------------------------------------------------------------
// Summary and Exit
// -----------------------------------------------------------------------------
debug('test-clipboard-history.ts completed!');
debug('All 8 isolated clipboard protocol tests executed.');

// Exit cleanly for autonomous testing
exit(0);

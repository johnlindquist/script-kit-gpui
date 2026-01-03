// Name: SDK Test - Storage and Path Utilities
// Description: Tests path utilities, file utilities, and memory map

/**
 * SDK TEST: test-storage.ts
 * 
 * Tests storage and path utility functions.
 * 
 * Test cases:
 * 1. path-home: home() path function
 * 2. path-kenv: skPath() function
 * 3. path-kit: kitPath() function
 * 4. path-tmp: tmpPath() function
 * 5. file-isFile: isFile() check
 * 6. file-isDir: isDir() check
 * 7. memoryMap-operations: memoryMap get/set/delete/clear
 * 
 * Expected behavior:
 * - Path functions return valid file system paths
 * - File checks return boolean values
 * - memoryMap provides in-memory key-value storage
 */

import '../../scripts/kit-sdk';

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

debug('test-storage.ts starting...');
debug(`SDK globals: home=${typeof home}, skPath=${typeof skPath}, kitPath=${typeof kitPath}`);

// -----------------------------------------------------------------------------
// Test 1: home() path function
// -----------------------------------------------------------------------------
const test1 = 'path-home';
logTest(test1, 'running');
const start1 = Date.now();

try {
  debug('Test 1: home() path function');
  
  const homePath = home();
  const downloadsPath = home('Downloads', 'file.txt');
  
  debug(`home(): ${homePath}`);
  debug(`home('Downloads', 'file.txt'): ${downloadsPath}`);
  
  const checks = [
    homePath.startsWith('/'),
    downloadsPath.includes('Downloads'),
    downloadsPath.endsWith('file.txt'),
  ];
  
  if (checks.every(Boolean)) {
    logTest(test1, 'pass', { result: homePath, duration_ms: Date.now() - start1 });
  } else {
    logTest(test1, 'fail', { 
      error: 'home() did not return valid paths',
      actual: homePath,
      duration_ms: Date.now() - start1 
    });
  }
} catch (err) {
  logTest(test1, 'fail', { error: String(err), duration_ms: Date.now() - start1 });
}

// -----------------------------------------------------------------------------
// Test 2: skPath() function
// -----------------------------------------------------------------------------
const test2 = 'path-kenv';
logTest(test2, 'running');
const start2 = Date.now();

try {
  debug('Test 2: skPath() function');
  
  const kenvRoot = skPath();
  const scriptsPath = skPath('scripts', 'hello.ts');
  
  debug(`skPath(): ${kenvRoot}`);
  debug(`skPath('scripts', 'hello.ts'): ${scriptsPath}`);
  
  const checks = [
    kenvRoot.startsWith('/'),
    scriptsPath.includes('scripts'),
    scriptsPath.endsWith('hello.ts'),
  ];
  
  if (checks.every(Boolean)) {
    logTest(test2, 'pass', { result: kenvRoot, duration_ms: Date.now() - start2 });
  } else {
    logTest(test2, 'fail', { 
      error: 'skPath() did not return valid paths',
      actual: kenvRoot,
      duration_ms: Date.now() - start2 
    });
  }
} catch (err) {
  logTest(test2, 'fail', { error: String(err), duration_ms: Date.now() - start2 });
}

// -----------------------------------------------------------------------------
// Test 3: kitPath() function
// -----------------------------------------------------------------------------
const test3 = 'path-kit';
logTest(test3, 'running');
const start3 = Date.now();

try {
  debug('Test 3: kitPath() function');
  
  const kitRoot = kitPath();
  const dbPath = kitPath('db', 'scripts.json');
  
  debug(`kitPath(): ${kitRoot}`);
  debug(`kitPath('db', 'scripts.json'): ${dbPath}`);
  
  const checks = [
    kitRoot.startsWith('/'),
    dbPath.includes('db'),
    dbPath.endsWith('scripts.json'),
  ];
  
  if (checks.every(Boolean)) {
    logTest(test3, 'pass', { result: kitRoot, duration_ms: Date.now() - start3 });
  } else {
    logTest(test3, 'fail', { 
      error: 'kitPath() did not return valid paths',
      actual: kitRoot,
      duration_ms: Date.now() - start3 
    });
  }
} catch (err) {
  logTest(test3, 'fail', { error: String(err), duration_ms: Date.now() - start3 });
}

// -----------------------------------------------------------------------------
// Test 4: tmpPath() function
// -----------------------------------------------------------------------------
const test4 = 'path-tmp';
logTest(test4, 'running');
const start4 = Date.now();

try {
  debug('Test 4: tmpPath() function');
  
  const tmpRoot = tmpPath();
  const tmpFile = tmpPath('output.txt');
  
  debug(`tmpPath(): ${tmpRoot}`);
  debug(`tmpPath('output.txt'): ${tmpFile}`);
  
  const checks = [
    tmpRoot.startsWith('/'),
    tmpFile.endsWith('output.txt'),
  ];
  
  if (checks.every(Boolean)) {
    logTest(test4, 'pass', { result: tmpRoot, duration_ms: Date.now() - start4 });
  } else {
    logTest(test4, 'fail', { 
      error: 'tmpPath() did not return valid paths',
      actual: tmpRoot,
      duration_ms: Date.now() - start4 
    });
  }
} catch (err) {
  logTest(test4, 'fail', { error: String(err), duration_ms: Date.now() - start4 });
}

// -----------------------------------------------------------------------------
// Test 5: isFile() check
// -----------------------------------------------------------------------------
const test5 = 'file-isFile';
logTest(test5, 'running');
const start5 = Date.now();

try {
  debug('Test 5: isFile() check');
  
  // @ts-expect-error Bun runtime provides these
  const thisFile = import.meta.path || Bun.argv[1];
  const isThisFile = await isFile(thisFile);
  const nonExistent = '/this/does/not/exist.txt';
  const isNonExistent = await isFile(nonExistent);
  
  debug(`isFile('${thisFile}'): ${isThisFile}`);
  debug(`isFile('${nonExistent}'): ${isNonExistent}`);
  
  const checks = [
    isThisFile === true,
    isNonExistent === false,
  ];
  
  if (checks.every(Boolean)) {
    logTest(test5, 'pass', { result: 'isFile works correctly', duration_ms: Date.now() - start5 });
  } else {
    logTest(test5, 'fail', { 
      error: 'isFile() did not return expected values',
      duration_ms: Date.now() - start5 
    });
  }
} catch (err) {
  logTest(test5, 'fail', { error: String(err), duration_ms: Date.now() - start5 });
}

// -----------------------------------------------------------------------------
// Test 6: isDir() check
// -----------------------------------------------------------------------------
const test6 = 'file-isDir';
logTest(test6, 'running');
const start6 = Date.now();

try {
  debug('Test 6: isDir() check');
  
  const homeDir = home();
  const isHomeDir = await isDir(homeDir);
  // @ts-expect-error Bun runtime provides these
  const thisFile = import.meta.path || Bun.argv[1];
  const isFileDirTest = await isDir(thisFile);
  
  debug(`isDir('${homeDir}'): ${isHomeDir}`);
  debug(`isDir('${thisFile}'): ${isFileDirTest}`);
  
  const checks = [
    isHomeDir === true,
    isFileDirTest === false,
  ];
  
  if (checks.every(Boolean)) {
    logTest(test6, 'pass', { result: 'isDir works correctly', duration_ms: Date.now() - start6 });
  } else {
    logTest(test6, 'fail', { 
      error: 'isDir() did not return expected values',
      duration_ms: Date.now() - start6 
    });
  }
} catch (err) {
  logTest(test6, 'fail', { error: String(err), duration_ms: Date.now() - start6 });
}

// -----------------------------------------------------------------------------
// Test 7: memoryMap operations
// -----------------------------------------------------------------------------
const test7 = 'memoryMap-operations';
logTest(test7, 'running');
const start7 = Date.now();

try {
  debug('Test 7: memoryMap operations');
  
  // Set some values
  memoryMap.set('counter', 42);
  memoryMap.set('user', { name: 'John', age: 30 });
  memoryMap.set('tags', ['javascript', 'typescript', 'bun']);
  
  // Get values
  const counter = memoryMap.get('counter');
  const user = memoryMap.get('user') as { name: string; age: number };
  const tags = memoryMap.get('tags') as string[];
  
  debug(`memoryMap.get('counter'): ${counter}`);
  debug(`memoryMap.get('user'): ${JSON.stringify(user)}`);
  debug(`memoryMap.get('tags'): ${JSON.stringify(tags)}`);
  
  // Get non-existent key
  const nonExistent = memoryMap.get('nonexistent');
  debug(`memoryMap.get('nonexistent'): ${nonExistent}`);
  
  // Delete a key
  const deleted = memoryMap.delete('counter');
  const counterAfterDelete = memoryMap.get('counter');
  debug(`memoryMap.delete('counter'): ${deleted}`);
  debug(`memoryMap.get('counter') after delete: ${counterAfterDelete}`);
  
  // Clear all
  memoryMap.clear();
  const userAfterClear = memoryMap.get('user');
  debug(`After clear, memoryMap.get('user'): ${userAfterClear}`);
  
  const checks = [
    counter === 42,
    user.name === 'John',
    tags.length === 3,
    nonExistent === undefined,
    deleted === true,
    counterAfterDelete === undefined,
    userAfterClear === undefined,
  ];
  
  if (checks.every(Boolean)) {
    logTest(test7, 'pass', { result: 'memoryMap works correctly', duration_ms: Date.now() - start7 });
  } else {
    logTest(test7, 'fail', { 
      error: 'memoryMap did not behave as expected',
      duration_ms: Date.now() - start7 
    });
  }
} catch (err) {
  logTest(test7, 'fail', { error: String(err), duration_ms: Date.now() - start7 });
}

// -----------------------------------------------------------------------------
// Show Summary
// -----------------------------------------------------------------------------
debug('test-storage.ts completed!');

await div(md(`# Storage and Path Tests Complete

All storage and path utility tests have been executed.

## Test Cases Run
1. **path-home**: home() path function
2. **path-kenv**: skPath() function
3. **path-kit**: kitPath() function
4. **path-tmp**: tmpPath() function
5. **file-isFile**: isFile() check
6. **file-isDir**: isDir() check
7. **memoryMap-operations**: memoryMap get/set/delete/clear

---

*Check the JSONL output for detailed results*

Press Escape or click to exit.`));

debug('test-storage.ts exiting...');

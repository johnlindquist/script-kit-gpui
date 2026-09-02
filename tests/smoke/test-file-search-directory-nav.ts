import '../../scripts/kit-sdk';
import { mkdirSync, rmdirSync, writeFileSync, existsSync } from 'fs';
import { join } from 'path';
import { homedir, tmpdir } from 'os';

/**
 * Test file search directory navigation
 * 
 * This tests the directory listing behavior when the filter text is a path:
 * - ~/path/ should list home directory contents
 * - /absolute/path/ should list absolute path contents
 * - ./relative/ should list relative path contents
 * - Non-existent paths should show empty or error gracefully
 */

function log(test: string, status: string, extra: any = {}) {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...extra }));
}

async function runTests() {
  const results: { name: string; pass: boolean; message?: string }[] = [];

  // Setup: Create a test directory structure
  const testDir = join(tmpdir(), 'sk-file-nav-test-' + Date.now());
  const subDir = join(testDir, 'subdir');
  
  try {
    // Create test structure
    mkdirSync(testDir, { recursive: true });
    mkdirSync(subDir, { recursive: true });
    writeFileSync(join(testDir, 'file1.txt'), 'test content 1');
    writeFileSync(join(testDir, 'file2.md'), 'test content 2');
    writeFileSync(join(subDir, 'nested.json'), '{}');

    console.error('[TEST] Created test directory structure at:', testDir);

    // Test 1: List absolute path directory
    log('list-absolute-path', 'running');
    const absResults = await fileSearch(testDir + '/');
    const absPass = absResults.length >= 2 && absResults.some(r => r.name === 'subdir' || r.name.includes('subdir'));
    results.push({
      name: 'list-absolute-path',
      pass: absPass,
      message: absPass ? `Found ${absResults.length} items` : `Expected 3 items (subdir, file1.txt, file2.md), got ${absResults.length}: ${JSON.stringify(absResults.map(r => r.name))}`
    });
    log('list-absolute-path', absPass ? 'pass' : 'fail', { count: absResults.length });

    // Test 2: List home directory (~)
    log('list-home-dir', 'running');
    const homeResults = await fileSearch('~/');
    const homePass = homeResults.length > 0;
    results.push({
      name: 'list-home-dir',
      pass: homePass,
      message: homePass ? `Found ${homeResults.length} items in home` : 'Home directory returned no items'
    });
    log('list-home-dir', homePass ? 'pass' : 'fail', { count: homeResults.length });

    // Test 3: An unavailable directory must reject, not become an empty success.
    log('nonexistent-path', 'running');
    const nonExistPath = testDir + '/missing-source-directory/';
    let sourceError: unknown;
    try { await fileSearch(nonExistPath); } catch (error) { sourceError = error; }
    const nonExistPass = sourceError instanceof Error && sourceError.message.length > 0;
    results.push({ name: 'nonexistent-path', pass: nonExistPass,
      message: nonExistPass ? `Rejected failed source: ${sourceError.message}` : 'Source failure was silently converted to success' });
    log('nonexistent-path', nonExistPass ? 'pass' : 'fail', { error: String(sourceError) });

    // Test 4: Directories should appear first in results
    log('dirs-first', 'running');
    const testDirResults = await fileSearch(testDir + '/');
    let dirsFirst = true;
    let seenFile = false;
    for (const r of testDirResults) {
      if (r.isDirectory) {
        if (seenFile) {
          dirsFirst = false;
          break;
        }
      } else {
        seenFile = true;
      }
    }
    results.push({
      name: 'dirs-first',
      pass: dirsFirst,
      message: dirsFirst ? 'Directories appear before files' : 'Files appeared before directories'
    });
    log('dirs-first', dirsFirst ? 'pass' : 'fail');

    // Test 5: Path without trailing slash should also work
    log('path-no-slash', 'running');
    const noSlashResults = await fileSearch(testDir);
    const noSlashPass = noSlashResults.length >= 2;
    results.push({
      name: 'path-no-slash',
      pass: noSlashPass,
      message: noSlashPass ? `Found ${noSlashResults.length} items without trailing slash` : 'Path without trailing slash failed'
    });
    log('path-no-slash', noSlashPass ? 'pass' : 'fail', { count: noSlashResults.length });

    // Test 6: Relative path (current directory)
    log('relative-path', 'running');
    const relResults = await fileSearch('./');
    const relPass = Array.isArray(relResults); // Should work, might be empty depending on cwd
    results.push({
      name: 'relative-path',
      pass: relPass,
      message: relPass ? `Relative path works, found ${relResults.length} items` : 'Relative path failed'
    });
    log('relative-path', relPass ? 'pass' : 'fail', { count: relResults.length });

  } finally {
    // Cleanup
    try {
      if (existsSync(join(subDir, 'nested.json'))) {
        const { unlinkSync } = await import('fs');
        unlinkSync(join(subDir, 'nested.json'));
      }
      if (existsSync(join(testDir, 'file1.txt'))) {
        const { unlinkSync } = await import('fs');
        unlinkSync(join(testDir, 'file1.txt'));
      }
      if (existsSync(join(testDir, 'file2.md'))) {
        const { unlinkSync } = await import('fs');
        unlinkSync(join(testDir, 'file2.md'));
      }
      if (existsSync(subDir)) rmdirSync(subDir);
      if (existsSync(testDir)) rmdirSync(testDir);
      console.error('[TEST] Cleaned up test directory');
    } catch (e) {
      console.error('[TEST] Cleanup failed:', e);
    }
  }

  // Summary
  const passed = results.filter(r => r.pass).length;
  const failed = results.filter(r => !r.pass).length;
  
  console.error(`\n[TEST SUMMARY] ${passed}/${results.length} tests passed`);
  for (const r of results) {
    console.error(`  ${r.pass ? '✓' : '✗'} ${r.name}: ${r.message || ''}`);
  }

  log('test-summary', failed === 0 ? 'pass' : 'fail', { passed, failed, total: results.length });
  
  exit(failed === 0 ? 0 : 1);
}

runTests().catch(e => {
  console.error('[TEST] Fatal error:', e);
  exit(1);
});

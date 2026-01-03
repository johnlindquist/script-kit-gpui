// Name: Alias Conflict Detection Test
// Description: Tests that duplicate aliases are detected and reported

/**
 * SMOKE TEST: test-alias-conflict.ts
 *
 * This test verifies the alias conflict detection functionality:
 * 1. Creates two temp scripts in ~/.sk/kit/scripts/ with the same alias
 * 2. Waits for file watcher to trigger reload_scripts
 * 3. Verifies conflict message appears in logs/HUD
 * 4. Cleans up temp scripts
 *
 * Expected log output:
 * Conflict: alias 'conflicttest' in _test-alias-conflict-2.ts blocked (already used by ...)
 *
 * Expected HUD message:
 * "Alias conflict: 'conflicttest' already used by _test-alias-conflict-1.ts"
 * 
 * Run with:
 * echo '{"type":"run","path":"tests/smoke/test-alias-conflict.ts"}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1
 */

import '../../scripts/kit-sdk';
import { writeFileSync, unlinkSync, existsSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';

const scriptsDir = join(homedir(), '.kenv', 'scripts');
const script1Path = join(scriptsDir, '_test-alias-conflict-1.ts');
const script2Path = join(scriptsDir, '_test-alias-conflict-2.ts');

const ALIAS = 'conflicttest';

const script1Content = `// Name: Test Script 1
// Alias: ${ALIAS}

console.log('Script 1 with alias ${ALIAS}');
`;

const script2Content = `// Name: Test Script 2
// Alias: ${ALIAS}

console.log('Script 2 with alias ${ALIAS}');
`;

// Cleanup function to ensure temp files are removed
function cleanup() {
    console.error('[SMOKE] Cleaning up temp scripts...');
    
    try {
        if (existsSync(script1Path)) {
            unlinkSync(script1Path);
            console.error(`[SMOKE] Deleted: ${script1Path}`);
        }
    } catch (e) {
        console.error(`[SMOKE] Warning: Could not delete script 1: ${e}`);
    }
    
    try {
        if (existsSync(script2Path)) {
            unlinkSync(script2Path);
            console.error(`[SMOKE] Deleted: ${script2Path}`);
        }
    } catch (e) {
        console.error(`[SMOKE] Warning: Could not delete script 2: ${e}`);
    }
    
    console.error('[SMOKE] Cleanup complete');
}

// Register cleanup for various exit scenarios
process.on('exit', cleanup);
process.on('SIGINT', () => { cleanup(); process.exit(1); });
process.on('SIGTERM', () => { cleanup(); process.exit(0); });

console.error('[SMOKE] test-alias-conflict.ts starting...');

async function runTest() {
    // Step 1: Write first temp script
    console.error(`[SMOKE] Writing script 1 to: ${script1Path}`);
    writeFileSync(script1Path, script1Content, 'utf-8');
    console.error('[SMOKE] Script 1 written successfully');

    // Step 2: Write second temp script with same alias
    console.error(`[SMOKE] Writing script 2 to: ${script2Path}`);
    writeFileSync(script2Path, script2Content, 'utf-8');
    console.error('[SMOKE] Script 2 written successfully');

    // Step 3: Wait for file watcher to pick up changes and trigger reload
    // The file watcher in GPUI app will detect the new scripts and call refresh_scripts()
    // which internally calls rebuild_registries() that detects conflicts
    console.error('[SMOKE] Waiting for file watcher to detect changes...');
    await new Promise(resolve => setTimeout(resolve, 1500));

    // Step 4: The conflict detection happens in Rust via file watcher
    // Expected log messages:
    // - "Conflict: alias 'conflicttest' in ... blocked (already used by ...)"
    // - "Showing HUD: 'Alias conflict: 'conflicttest' already used by ...'"
    
    console.error('[SMOKE] Conflict detection should have occurred by now');
    console.error('[SMOKE] Expected in logs: Conflict: alias \'' + ALIAS + '\' in _test-alias-conflict-2.ts blocked');
    console.error('[SMOKE] Expected HUD: Alias conflict: \'' + ALIAS + '\' already used by _test-alias-conflict-1.ts');
    
    // Step 5: Show result via div (optional - for interactive verification)
    await div(md(`# Alias Conflict Test Complete

The test has created two scripts with the same alias \`${ALIAS}\`:
- \`_test-alias-conflict-1.ts\`
- \`_test-alias-conflict-2.ts\`

## Expected Behavior

After file watcher triggers \`refresh_scripts\`:
1. The first script's alias is registered
2. The second script's alias is **blocked**
3. A conflict message appears in:
   - Logs: \`Conflict: alias '${ALIAS}' in ... blocked\`
   - HUD: \`Alias conflict: '${ALIAS}' already used by ...\`

## Verification

Check the logs for:
\`\`\`
Conflict: alias '${ALIAS}' in _test-alias-conflict-2.ts blocked
Showing HUD: 'Alias conflict: '${ALIAS}' already used by _test-alias-conflict-1.ts'
\`\`\`

*Press Enter or click to finish test*`));

    console.error('[SMOKE] Test interaction complete');
}

runTest()
    .then(() => {
        console.error('[SMOKE] test-alias-conflict.ts completed successfully!');
        cleanup();
        process.exit(0);
    })
    .catch((err) => {
        console.error('[SMOKE] Test failed:', err);
        cleanup();
        process.exit(1);
    });

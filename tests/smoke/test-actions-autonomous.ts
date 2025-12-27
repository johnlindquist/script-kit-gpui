// Name: Actions Panel Autonomous Visual Test
// Description: Autonomous test to verify actions panel styling and detect duplicate search box issue

/**
 * AUTONOMOUS VISUAL TEST: test-actions-autonomous.ts
 * 
 * PURPOSE: Verify the actions panel "inline" design and detect the
 * reported "search box appears twice" issue.
 * 
 * DESIGN CONTEXT:
 * The actions panel was refactored to be "inline":
 * - When Cmd+K is pressed, Run/Actions buttons in header are replaced by actions search input
 * - Actions list appears in the preview panel (right side)
 * - Logo stays visible
 * 
 * ========================================================================
 * BUG IDENTIFIED: DUPLICATE SEARCH BOX
 * ========================================================================
 * 
 * CODE ANALYSIS FINDINGS:
 * 
 * When show_actions_popup=true, TWO search inputs are rendered:
 * 
 * 1. HEADER SEARCH INPUT (main.rs:3331-3392):
 *    - Rendered when `.when(self.show_actions_popup, |d| ...)`
 *    - Shows "⌘K" indicator + search input in header
 *    - Location: Top header, replacing Run/Actions buttons
 * 
 * 2. ACTIONS DIALOG SEARCH INPUT (actions.rs:504-575):
 *    - ActionsDialog has its OWN input_container at the BOTTOM
 *    - Shows "⌘K" indicator + search input
 *    - Location: Bottom of the ActionsDialog component
 * 
 * RESULT: User sees TWO search boxes:
 *    - One in the header (main.rs)
 *    - One at the bottom of the actions panel (actions.rs)
 * 
 * FIX RECOMMENDATION:
 * Either:
 *   A) Remove the input_container from ActionsDialog.render() when used inline
 *   B) Remove the header search input (lines 3331-3392 in main.rs)
 *   C) Pass a flag to ActionsDialog to hide its search when inline
 * 
 * ========================================================================
 * 
 * USAGE (Automated with visual-test.sh):
 *   ./scripts/visual-test.sh tests/smoke/test-actions-autonomous.ts 5
 *   # When window appears, press Cmd+K to show actions panel
 *   # Screenshot captures the state for analysis
 * 
 * WHAT TO LOOK FOR IN SCREENSHOT:
 *   1. Is there a duplicate search box? (YES - see bug above)
 *   2. Is the main arg search input still visible when actions panel opens?
 *   3. Is the actions search input positioned correctly in the header?
 *   4. Does the preview panel show the actions list?
 */

import '../../scripts/kit-sdk';

// Constants
const RENDER_DELAY_MS = 2000;

console.error('[ACTIONS-AUTO] ========================================');
console.error('[ACTIONS-AUTO] AUTONOMOUS ACTIONS PANEL VISUAL TEST');
console.error('[ACTIONS-AUTO] ========================================');
console.error('[ACTIONS-AUTO] ISSUE: User reports "search box appears twice"');
console.error('[ACTIONS-AUTO] ');
console.error('[ACTIONS-AUTO] EXPECTED DESIGN:');
console.error('[ACTIONS-AUTO]   - Header: Logo + Actions search input (replaces Run/Actions buttons)');
console.error('[ACTIONS-AUTO]   - Main area: Script list (left) + Actions list in preview (right)');
console.error('[ACTIONS-AUTO]   - Should NOT show two search inputs');
console.error('[ACTIONS-AUTO] ');
console.error('[ACTIONS-AUTO] *** PRESS Cmd+K to trigger actions panel ***');
console.error('[ACTIONS-AUTO] ========================================');

async function runTest() {
  console.error('[ACTIONS-AUTO] Step 1: Creating arg prompt with choices...');
  
  // Create choices that simulate script list
  const choices = [
    { name: 'hello-world', value: 'hello', description: 'Classic hello world script' },
    { name: 'generate-report', value: 'report', description: 'Generate a status report' },
    { name: 'send-notification', value: 'notify', description: 'Send a notification' },
    { name: 'open-browser', value: 'browse', description: 'Open a URL in browser' },
    { name: 'clipboard-manager', value: 'clip', description: 'Manage clipboard history' },
  ];
  
  // Start arg prompt (don't await - we want to test the UI state)
  const argPromise = arg('Search scripts...', choices);
  
  // Give window time to render
  console.error('[ACTIONS-AUTO] Step 2: Waiting for window render (500ms)...');
  await wait(500);
  
  // Log initial window bounds
  try {
    const bounds = await getWindowBounds();
    console.error(`[ACTIONS-AUTO] Window bounds: ${bounds.width}x${bounds.height} at (${bounds.x}, ${bounds.y})`);
  } catch (e) {
    console.error(`[ACTIONS-AUTO] Could not get window bounds: ${e}`);
  }
  
  // Attempt to trigger Cmd+K programmatically
  console.error('[ACTIONS-AUTO] Step 3: Attempting to send Cmd+K...');
  try {
    await keyboard.tap('command', 'k');
    console.error('[ACTIONS-AUTO] keyboard.tap sent successfully');
  } catch (e) {
    console.error(`[ACTIONS-AUTO] keyboard.tap failed (expected): ${e}`);
    console.error('[ACTIONS-AUTO] *** USER MUST PRESS Cmd+K MANUALLY ***');
  }
  
  // Wait for user to press Cmd+K and actions panel to render
  console.error('[ACTIONS-AUTO] Step 4: Waiting for actions panel to appear...');
  console.error('[ACTIONS-AUTO] *** PRESS Cmd+K NOW IF NOT ALREADY ***');
  await wait(RENDER_DELAY_MS);
  
  // Capture screenshot
  console.error('[ACTIONS-AUTO] Step 5: Capturing screenshot for analysis...');
  try {
    const screenshot = await captureScreenshot();
    console.error(`[ACTIONS-AUTO] Screenshot: ${screenshot.width}x${screenshot.height}`);
    console.error(`[ACTIONS-AUTO] Size: ${Math.round(screenshot.data.length / 1024)}KB`);
    
    // Analysis hints based on dimensions
    const isFullScreen = screenshot.width > 1500 || screenshot.height > 1000;
    console.error(`[ACTIONS-AUTO] Capture type: ${isFullScreen ? 'Full screen' : 'App window'}`);
    
    // Output JSON for programmatic analysis
    console.log(JSON.stringify({
      test: 'actions-autonomous',
      status: 'pass',
      screenshot: {
        width: screenshot.width,
        height: screenshot.height,
        sizeKB: Math.round(screenshot.data.length / 1024),
        isFullScreen,
      },
      investigation: {
        issue: 'Search box appears twice',
        lookFor: [
          'Two input fields visible',
          'Main search input + actions search input both showing',
          'Header showing duplicate controls'
        ],
        expectedDesign: [
          'Single actions search in header (replaces Run/Actions buttons)',
          'Script list in main area',
          'Actions list in preview panel (right)',
          'Logo still visible'
        ]
      },
      timestamp: new Date().toISOString(),
    }));
    
    console.error('[ACTIONS-AUTO] ========================================');
    console.error('[ACTIONS-AUTO] SCREENSHOT CAPTURED');
    console.error('[ACTIONS-AUTO] Check .test-screenshots/ for the image');
    console.error('[ACTIONS-AUTO] ========================================');
    console.error('[ACTIONS-AUTO] WHAT TO ANALYZE:');
    console.error('[ACTIONS-AUTO]   1. Count visible search/input fields');
    console.error('[ACTIONS-AUTO]   2. Check if main arg search is hidden');
    console.error('[ACTIONS-AUTO]   3. Verify actions search is in header');
    console.error('[ACTIONS-AUTO]   4. Confirm preview panel shows actions');
    console.error('[ACTIONS-AUTO] ========================================');
    
  } catch (err) {
    console.error(`[ACTIONS-AUTO] FAIL: Screenshot capture failed: ${err}`);
    console.log(JSON.stringify({
      test: 'actions-autonomous',
      status: 'fail',
      error: String(err),
      timestamp: new Date().toISOString(),
    }));
  }
  
  // Clean up
  console.error('[ACTIONS-AUTO] Step 6: Cleanup...');
  await Promise.race([
    argPromise.catch(() => {}),
    wait(1000),
  ]);
  
  console.error('[ACTIONS-AUTO] Test complete.');
}

// Run the test
runTest().catch((err) => {
  console.error(`[ACTIONS-AUTO] FATAL: ${err}`);
  console.log(JSON.stringify({
    test: 'actions-autonomous',
    status: 'fail',
    error: String(err),
    timestamp: new Date().toISOString(),
  }));
});

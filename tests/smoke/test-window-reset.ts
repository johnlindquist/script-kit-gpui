// Name: Window Reset Test
// Description: Verify window visibility state and reset after script completion

import '../../scripts/kit-sdk';

// Test script to verify:
// 1. WINDOW_VISIBLE state is correctly updated when hide() is called
// 2. Single hotkey press reopens the window (no double-press needed)
// 3. Window shows fresh ScriptList after reopening

console.error('[SMOKE] ══════════════════════════════════════════════════════════');
console.error('[SMOKE] WINDOW RESET TEST - Starting');
console.error('[SMOKE] ══════════════════════════════════════════════════════════');

await div(`
  <div style="font-family: system-ui; padding: 20px;">
    <h1 style="color: #4a90e2;">Window Reset Test</h1>
    <p>This tests the visibility state machine fix.</p>
    
    <h2>What will happen:</h2>
    <ol>
      <li>In <strong>2 seconds</strong>, this script will call <code>hide()</code></li>
      <li>The window will hide and <code>WINDOW_VISIBLE</code> will be set to <code>false</code></li>
      <li>Then the script will exit</li>
    </ol>
    
    <h2 style="color: #7ed321;">Expected behavior:</h2>
    <p>Press your hotkey <strong>ONCE</strong> - the window should immediately reopen to the script list.</p>
    
    <h2 style="color: #f5a623;">Bug symptom (if not fixed):</h2>
    <p>You would need to press the hotkey <strong>TWICE</strong> - first press does nothing, second press opens.</p>
    
    <p style="color: #888; margin-top: 30px; font-size: 12px;">
      Check logs for VISIBILITY entries to see state transitions.
    </p>
  </div>
`);

console.error('[SMOKE] Waiting 2 seconds before calling hide()...');
await new Promise(resolve => setTimeout(resolve, 2000));

console.error('[SMOKE] Calling hide() - this should set WINDOW_VISIBLE=false');
hide();

console.error('[SMOKE] Script complete - press hotkey ONCE to verify window reopens');
console.error('[SMOKE] ══════════════════════════════════════════════════════════');

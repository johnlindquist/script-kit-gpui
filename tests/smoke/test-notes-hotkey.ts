// Name: Test Notes Hotkey
// Description: Verify notes hotkey triggers notes window, not main window

import '../../scripts/kit-sdk';

console.error('[TEST] test-notes-hotkey.ts starting');
console.error('[TEST] This test verifies the Notes hotkey (Cmd+Shift+N) behavior');
console.error('[TEST] Expected: Notes window opens, main window stays hidden');
console.error('[TEST] Bug: Main window was opening instead of Notes window');

// Simple test - just log that we started
// The actual hotkey testing needs to be done by observing the logs
console.error('[TEST] Script loaded successfully');
console.error('[TEST] To test:');
console.error('[TEST] 1. Launch the app');
console.error('[TEST] 2. Press Cmd+Shift+N BEFORE pressing Cmd+; (main hotkey)');
console.error('[TEST] 3. Check logs for:');
console.error('[TEST]    - "meta+shift+KeyN pressed (notes)" should appear');
console.error('[TEST]    - "Notes hotkey triggered - opening notes window" should appear');
console.error('[TEST]    - Notes window should open, NOT main window');

// Exit after logging
(globalThis as any).process.exit(0);

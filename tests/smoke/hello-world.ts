// Name: Hello World Smoke Test
// Description: Basic scriptlet for smoke testing the GPUI executor
// Author: Script Kit Team

/**
 * SMOKE TEST: hello-world.ts
 * 
 * This is the simplest possible scriptlet for testing:
 * - SDK preload works correctly
 * - JSONL protocol communication is functional
 * - div() message is sent and displayed
 * - Script exits cleanly after user interaction
 * 
 * Expected log output from executor.rs:
 * [EXEC] execute_script_interactive: ~/.sk/kit/scripts/hello-world.ts
 * [EXEC] Looking for SDK...
 * [EXEC] FOUND SDK: ~/.sk/kit/sdk/kit-sdk.ts (or dev path)
 * [EXEC] Trying: bun run --preload <sdk> <script>
 * [EXEC] SUCCESS: bun with preload
 * [EXEC] Process spawned with PID: <pid>
 * [EXEC] Received from script: {"type":"div","id":"1","html":"..."}
 */

// Import SDK - registers global functions (arg, div, md)
import '../../scripts/kit-sdk';

// Log to stderr for debugging (stderr is passed through to terminal)
console.error('[SMOKE] hello-world.ts starting...');
console.error('[SMOKE] SDK globals available:', typeof arg, typeof div, typeof md);

// Simple div display - this tests:
// 1. md() markdown parsing works
// 2. div() sends JSONL message to Rust
// 3. UI renders the HTML content
// 4. User can dismiss (sends submit back)
await div(md(`# Hello from Smoke Test! 🎉

Welcome to the **GPUI Script Kit** smoke test.

## What this tests:
- SDK preload mechanism
- JSONL stdout communication
- Markdown rendering via \`md()\`
- div() prompt display

---

*Click anywhere or press Escape to continue*`));

console.error('[SMOKE] hello-world.ts completed successfully!');

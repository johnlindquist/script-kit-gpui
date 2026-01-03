// Name: Hello World Args Smoke Test
// Description: Scriptlet demonstrating arg() prompt with choices
// Author: Script Kit Team

/**
 * SMOKE TEST: hello-world-args.ts
 * 
 * Tests the interactive arg() prompt functionality:
 * - arg() sends JSONL message with choices to Rust
 * - UI renders choice list with filtering
 * - User selection is sent back via JSONL submit
 * - Script receives and processes the selection
 * - div() displays the result
 * 
 * Expected log output from executor.rs:
 * [EXEC] execute_script_interactive: ~/.sk/kit/scripts/hello-world-args.ts
 * [EXEC] Looking for SDK...
 * [EXEC] FOUND SDK: ~/.sk/kit/sdk/kit-sdk.ts
 * [EXEC] Trying: bun run --preload <sdk> <script>
 * [EXEC] SUCCESS: bun with preload
 * [EXEC] Process spawned with PID: <pid>
 * [EXEC] Received from script: {"type":"arg","id":"1","placeholder":"...","choices":[...]}
 * [EXEC] Sending to script: {"type":"submit","id":"1","value":"<selected>"}
 * [EXEC] Received from script: {"type":"div","id":"2","html":"..."}
 * [EXEC] Sending to script: {"type":"submit","id":"2","value":null}
 * [EXEC] Script exited with code: 0
 */

// Import SDK - registers global functions (arg, div, md)
import '../../scripts/kit-sdk';

// Log to stderr for observability
console.error('[SMOKE] hello-world-args.ts starting...');
console.error('[SMOKE] Testing arg() prompt with choices...');

// First prompt: arg() with string choices (simple mode)
const greeting = await arg('Select a greeting style:', [
  'Hello',
  'Hi there',
  'Hey',
  'Greetings',
  'Welcome',
]);

console.error(`[SMOKE] User selected greeting: "${greeting}"`);

// Second prompt: arg() with structured choices (rich mode)
const recipient = await arg('Who do you want to greet?', [
  { name: 'World', value: 'world', description: 'The whole world' },
  { name: 'Developer', value: 'developer', description: 'A fellow coder' },
  { name: 'Tester', value: 'tester', description: 'Our QA hero' },
  { name: 'Script Kit', value: 'script-kit', description: 'The tool itself!' },
]);

console.error(`[SMOKE] User selected recipient: "${recipient}"`);

// Display result using div() with markdown
const message = `${greeting}, ${recipient}!`;
console.error(`[SMOKE] Final message: "${message}"`);

await div(md(`# ${message}

## Test Results

| Component | Status |
|-----------|--------|
| SDK Preload | ✅ Working |
| arg() simple | ✅ Working |
| arg() structured | ✅ Working |
| div() display | ✅ Working |
| JSONL Protocol | ✅ Working |

---

### Debug Info
- Greeting selected: **${greeting}**
- Recipient selected: **${recipient}**
- Timestamp: \`${new Date().toISOString()}\`

*Click anywhere or press Escape to exit*`));

console.error('[SMOKE] hello-world-args.ts completed successfully!');
console.error('[SMOKE] All prompts handled correctly.');

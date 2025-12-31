// Name: Test Div Mixed Links
// Description: RED PHASE - Tests both external and submit links in the same div

import '../../scripts/kit-sdk';

console.error('[SMOKE] test-div-mixed-links.ts starting...');
console.error('[SMOKE] This test should FAIL until link handling is implemented');

const html = md(`# Mixed Links Test

This div contains BOTH external links and submit links.

## Learn More (External - open in browser)
- [Read the Docs](https://scriptkit.com/docs)
- [View on GitHub](https://github.com/johnlindquist/kit)
- [Watch Tutorial](https://youtube.com/@scriptkit)

## Make a Choice (Submit - returns value)
- [Option A](submit:option-a)
- [Option B](submit:option-b)
- [Option C](submit:option-c)

## Combined Section
Here's a paragraph with both types: 
Check out [the website](https://scriptkit.com) for more info, 
or just [pick this option](submit:quick-pick) to continue.

---

**Link behavior summary:**
| Link Type | Example | Action |
|-----------|---------|--------|
| External | \`https://...\` | Opens in browser, div stays open |
| Submit | \`submit:value\` | Closes div, returns value |

---

**Current status:**
- External links: NOT clickable
- Submit links: NOT clickable
- div() return: Always void (should return submit value)

Press Escape to close.
`);

console.error('[SMOKE] HTML generated with mixed link types');
console.error('[SMOKE] Expected external behavior: Click opens browser');
console.error('[SMOKE] Expected submit behavior: Click returns value and closes div');
console.error('[SMOKE] Current: Neither type works');

const result = await div(html);

console.error(`[SMOKE] div() returned: ${JSON.stringify(result)}`);
console.error('[SMOKE] Expected: Value from submit link if clicked, undefined if escaped');

console.error('[SMOKE] test-div-mixed-links.ts completed');
process.exit(0);

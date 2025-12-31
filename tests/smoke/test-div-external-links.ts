// Name: Test Div External Links
// Description: RED PHASE - Tests that external URLs open in the default browser

import '../../scripts/kit-sdk';

console.error('[SMOKE] test-div-external-links.ts starting...');
console.error('[SMOKE] This test should FAIL until external link handling is implemented');

const html = md(`# External Link Test

Click any link below to open in your browser:

## Documentation Links
- [Script Kit Website](https://scriptkit.com)
- [GitHub Repository](https://github.com/johnlindquist/kit)
- [GPUI Documentation](https://docs.rs/gpui/latest/gpui/)

## General Links
- [Google](https://google.com)
- [Example Domain](https://example.com)

---

**Expected behavior:** Clicking an external link opens it in the default browser.

**Current behavior:** Links are styled but NOT clickable.

---

External links (https://, http://) should:
1. Be visually styled as clickable links
2. Open in the system's default browser when clicked
3. NOT close the div window (just open externally)

Press Escape or Enter to close this div.
`);

console.error('[SMOKE] HTML generated with external links');
console.error('[SMOKE] Expected: Clicking [Google](https://google.com) opens browser');
console.error('[SMOKE] Current: Links are not interactive');

await div(html);

console.error('[SMOKE] test-div-external-links.ts completed');
process.exit(0);

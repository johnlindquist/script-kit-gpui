// Name: Test Actions Panel Height
// Description: Test that 4-item actions panel doesn't scroll

import '../../scripts/kit-sdk';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

console.error('[SMOKE] Testing actions panel height with 4 items...');

// Exactly 4 actions
const actions = [
  { name: "Copy to Clipboard", shortcut: "cmd+c" },
  { name: "Preview", shortcut: "cmd+p" },
  { name: "Edit", shortcut: "cmd+e" },
  { name: "Delete", shortcut: "cmd+backspace" },
];

const result = await arg("Select an item", ["Item 1", "Item 2", "Item 3"], actions);

console.error('[SMOKE] Result:', result);

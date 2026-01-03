#!/usr/bin/env bun
/**
 * Protocol Coverage Matrix
 *
 * Tracks test coverage for all 59+ protocol message types defined in
 * src/protocol/message.rs and docs/PROTOCOL.md
 *
 * Run: bun run tests/protocol-coverage-matrix.ts
 */

// =============================================================================
// Types
// =============================================================================

type CoverageStatus = 'tested' | 'partial' | 'untested';

interface ProtocolMessage {
  /** Message type name (matches serde rename in Rust) */
  name: string;
  /** Category for grouping */
  category: string;
  /** Coverage status */
  status: CoverageStatus;
  /** Test file(s) that cover this message */
  testFiles: string[];
  /** Notes about coverage */
  notes?: string;
}

// =============================================================================
// Protocol Message Definitions
// =============================================================================

const PROTOCOL_MESSAGES: ProtocolMessage[] = [
  // ============================================================
  // CORE PROMPTS (5)
  // ============================================================
  {
    name: 'arg',
    category: 'Core Prompts',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-arg.ts',
      'tests/smoke/test-arg-visual.ts',
      'tests/smoke/test-arg-two-choices.ts',
      'tests/smoke/test-arg-with-actions.ts',
      'tests/smoke/audit-arg-choices.ts',
      'tests/smoke/audit-01-arg.ts',
    ],
    notes: 'Good coverage of choices, filtering, and actions',
  },
  {
    name: 'div',
    category: 'Core Prompts',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-div.ts',
      'tests/smoke/test-div-height.ts',
      'tests/smoke/test-div-scroll.ts',
      'tests/smoke/test-div-submit-links.ts',
      'tests/smoke/test-div-external-links.ts',
      'tests/smoke/test-div-options-visual.ts',
      'tests/smoke/test-div-container-options.ts',
      'tests/smoke/audit-div-basic.ts',
    ],
    notes: 'Good coverage including container options and links',
  },
  {
    name: 'submit',
    category: 'Core Prompts',
    status: 'tested',
    testFiles: ['tests/sdk/test-arg.ts', 'tests/sdk/test-div.ts'],
    notes: 'Tested via prompt responses',
  },
  {
    name: 'update',
    category: 'Core Prompts',
    status: 'partial',
    testFiles: [],
    notes: 'Used internally for live updates, needs explicit test',
  },
  {
    name: 'exit',
    category: 'Core Prompts',
    status: 'tested',
    testFiles: ['tests/smoke/test-user-cancel.ts', 'tests/smoke/test-error-handling.ts'],
    notes: 'Tested via script termination',
  },

  // ============================================================
  // INPUT CONTROL (2)
  // ============================================================
  {
    name: 'forceSubmit',
    category: 'Input Control',
    status: 'untested',
    testFiles: [],
    notes: 'SDK submit() function - needs test',
  },
  {
    name: 'setInput',
    category: 'Input Control',
    status: 'untested',
    testFiles: [],
    notes: 'Set input text programmatically - needs test',
  },

  // ============================================================
  // TEXT INPUT PROMPTS (3)
  // ============================================================
  {
    name: 'editor',
    category: 'Text Input',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-editor.ts',
      'tests/smoke/test-editor-height.ts',
      'tests/smoke/test-editor-visual-fill.ts',
      'tests/smoke/test-editor-v2-visual.ts',
      'tests/smoke/test-editor-find-replace-visual.ts',
      'tests/smoke/test-editor-actions-keys.ts',
      'tests/smoke/audit-editor.ts',
    ],
    notes: 'Good coverage including height, actions, find/replace',
  },
  {
    name: 'mini',
    category: 'Text Input',
    status: 'untested',
    testFiles: [],
    notes: 'Compact prompt variant - needs test',
  },
  {
    name: 'micro',
    category: 'Text Input',
    status: 'untested',
    testFiles: [],
    notes: 'Tiny prompt variant - needs test',
  },

  // ============================================================
  // SELECTION PROMPTS (1)
  // ============================================================
  {
    name: 'select',
    category: 'Selection',
    status: 'partial',
    testFiles: ['tests/sdk/test-select.ts'],
    notes: 'Basic test exists, needs multi-select coverage',
  },

  // ============================================================
  // FORM PROMPTS (2)
  // ============================================================
  {
    name: 'fields',
    category: 'Form',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-fields.ts',
      'tests/sdk/test-fields-basic.ts',
      'tests/sdk/test-fields-datetime.ts',
      'tests/smoke/test-form-typing.ts',
    ],
    notes: 'Good coverage including datetime fields',
  },
  {
    name: 'form',
    category: 'Form',
    status: 'partial',
    testFiles: [
      'tests/sdk/test-form-all-types.ts',
      'tests/sdk/test-form-specialized.ts',
      'tests/smoke/test-form-prompt.ts',
    ],
    notes: 'Custom HTML form - partial coverage',
  },

  // ============================================================
  // FILE/PATH PROMPTS (2)
  // ============================================================
  {
    name: 'path',
    category: 'File/Path',
    status: 'partial',
    testFiles: [
      'tests/sdk/test-path.ts',
      'tests/smoke/test-path-key-events.ts',
      'tests/smoke/test-path-visual-consistency.ts',
      'tests/smoke/test-path-actions-visual.ts',
    ],
    notes: 'Path picker with visual tests',
  },
  {
    name: 'drop',
    category: 'File/Path',
    status: 'partial',
    testFiles: ['tests/sdk/test-drop.ts'],
    notes: 'File drop zone - basic test',
  },

  // ============================================================
  // INPUT CAPTURE PROMPTS (1)
  // ============================================================
  {
    name: 'hotkey',
    category: 'Input Capture',
    status: 'partial',
    testFiles: ['tests/sdk/test-hotkey.ts'],
    notes: 'Keyboard shortcut capture - basic test',
  },

  // ============================================================
  // TEMPLATE/TEXT PROMPTS (2)
  // ============================================================
  {
    name: 'template',
    category: 'Template/Text',
    status: 'partial',
    testFiles: [
      'tests/sdk/test-template.ts',
      'tests/smoke/test-template.ts',
      'tests/smoke/test-template-choices.ts',
      'tests/smoke/test-template-manual.ts',
      'tests/smoke/test-template-tab-nav.ts',
    ],
    notes: 'Template with placeholders - good coverage',
  },
  {
    name: 'env',
    category: 'Template/Text',
    status: 'partial',
    testFiles: ['tests/sdk/test-env.ts', 'tests/smoke/test-env-visual.ts'],
    notes: 'Environment variable prompt',
  },

  // ============================================================
  // MEDIA PROMPTS (5)
  // ============================================================
  {
    name: 'chat',
    category: 'Media',
    status: 'partial',
    testFiles: ['tests/sdk/test-chat.ts'],
    notes: 'Chat interface - basic test',
  },
  {
    name: 'term',
    category: 'Media',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-term.ts',
      'tests/smoke/test-term.ts',
      'tests/smoke/test-term-visual.ts',
      'tests/smoke/audit-term.ts',
    ],
    notes: 'Terminal emulator - good coverage',
  },
  {
    name: 'widget',
    category: 'Media',
    status: 'partial',
    testFiles: ['tests/sdk/test-widget.ts'],
    notes: 'Custom widget window - basic test',
  },
  {
    name: 'webcam',
    category: 'Media',
    status: 'untested',
    testFiles: [],
    notes: 'Webcam capture - requires hardware',
  },
  {
    name: 'mic',
    category: 'Media',
    status: 'untested',
    testFiles: [],
    notes: 'Microphone recording - requires hardware',
  },

  // ============================================================
  // NOTIFICATION/FEEDBACK (5)
  // ============================================================
  {
    name: 'notify',
    category: 'Notification',
    status: 'untested',
    testFiles: [],
    notes: 'System notification - OS dependent',
  },
  {
    name: 'beep',
    category: 'Notification',
    status: 'untested',
    testFiles: [],
    notes: 'System beep sound - audio test',
  },
  {
    name: 'say',
    category: 'Notification',
    status: 'untested',
    testFiles: [],
    notes: 'Text-to-speech - audio test',
  },
  {
    name: 'setStatus',
    category: 'Notification',
    status: 'untested',
    testFiles: [],
    notes: 'Status bar update - needs test',
  },
  {
    name: 'hud',
    category: 'Notification',
    status: 'partial',
    testFiles: ['tests/smoke/test-hud.ts'],
    notes: 'HUD overlay message',
  },

  // ============================================================
  // SYSTEM CONTROL (8)
  // ============================================================
  {
    name: 'menu',
    category: 'System Control',
    status: 'untested',
    testFiles: [],
    notes: 'Menu bar icon/scripts - needs test',
  },
  {
    name: 'clipboard',
    category: 'System Control',
    status: 'partial',
    testFiles: ['tests/smoke/test-clipboard-newlines.ts'],
    notes: 'Clipboard read/write operations',
  },
  {
    name: 'keyboard',
    category: 'System Control',
    status: 'untested',
    testFiles: [],
    notes: 'Keyboard simulation (type/tap) - needs test',
  },
  {
    name: 'mouse',
    category: 'System Control',
    status: 'untested',
    testFiles: [],
    notes: 'Mouse control (move/click) - needs test',
  },
  {
    name: 'show',
    category: 'System Control',
    status: 'tested',
    testFiles: ['tests/smoke/test-window-vibrancy.ts'],
    notes: 'Show window - implicit in most tests',
  },
  {
    name: 'hide',
    category: 'System Control',
    status: 'partial',
    testFiles: [],
    notes: 'Hide window - used at end of scripts',
  },
  {
    name: 'browse',
    category: 'System Control',
    status: 'untested',
    testFiles: [],
    notes: 'Open URL in browser - needs test',
  },
  {
    name: 'exec',
    category: 'System Control',
    status: 'untested',
    testFiles: [],
    notes: 'Execute shell command - needs test',
  },

  // ============================================================
  // UI UPDATE (3)
  // ============================================================
  {
    name: 'setPanel',
    category: 'UI Update',
    status: 'untested',
    testFiles: [],
    notes: 'Set panel HTML content - needs test',
  },
  {
    name: 'setPreview',
    category: 'UI Update',
    status: 'untested',
    testFiles: [],
    notes: 'Set preview pane HTML - needs test',
  },
  {
    name: 'setPrompt',
    category: 'UI Update',
    status: 'untested',
    testFiles: [],
    notes: 'Set prompt area HTML - needs test',
  },

  // ============================================================
  // SELECTED TEXT (7)
  // ============================================================
  {
    name: 'getSelectedText',
    category: 'Selected Text',
    status: 'partial',
    testFiles: ['tests/sdk/test-selected-text.ts'],
    notes: 'Get selection from focused app',
  },
  {
    name: 'setSelectedText',
    category: 'Selected Text',
    status: 'partial',
    testFiles: ['tests/sdk/test-selected-text.ts'],
    notes: 'Replace selection in focused app',
  },
  {
    name: 'checkAccessibility',
    category: 'Selected Text',
    status: 'untested',
    testFiles: [],
    notes: 'Check accessibility permissions',
  },
  {
    name: 'requestAccessibility',
    category: 'Selected Text',
    status: 'untested',
    testFiles: [],
    notes: 'Request accessibility permissions',
  },
  {
    name: 'selectedText',
    category: 'Selected Text',
    status: 'partial',
    testFiles: ['tests/sdk/test-selected-text.ts'],
    notes: 'Response with selected text',
  },
  {
    name: 'textSet',
    category: 'Selected Text',
    status: 'partial',
    testFiles: ['tests/sdk/test-selected-text.ts'],
    notes: 'Response after setting text',
  },
  {
    name: 'accessibilityStatus',
    category: 'Selected Text',
    status: 'untested',
    testFiles: [],
    notes: 'Response with permission status',
  },

  // ============================================================
  // WINDOW INFO (2)
  // ============================================================
  {
    name: 'getWindowBounds',
    category: 'Window Info',
    status: 'partial',
    testFiles: ['tests/sdk/test-window-management.ts'],
    notes: 'Get app window position/size',
  },
  {
    name: 'windowBounds',
    category: 'Window Info',
    status: 'partial',
    testFiles: ['tests/sdk/test-window-management.ts'],
    notes: 'Response with bounds',
  },

  // ============================================================
  // CLIPBOARD HISTORY (4)
  // ============================================================
  {
    name: 'clipboardHistory',
    category: 'Clipboard History',
    status: 'partial',
    testFiles: ['tests/sdk/test-clipboard-history.ts'],
    notes: 'Clipboard history operations',
  },
  {
    name: 'clipboardHistoryEntry',
    category: 'Clipboard History',
    status: 'partial',
    testFiles: ['tests/sdk/test-clipboard-history.ts'],
    notes: 'Single history entry response',
  },
  {
    name: 'clipboardHistoryList',
    category: 'Clipboard History',
    status: 'partial',
    testFiles: ['tests/sdk/test-clipboard-history.ts'],
    notes: 'List of history entries response',
  },
  {
    name: 'clipboardHistoryResult',
    category: 'Clipboard History',
    status: 'partial',
    testFiles: ['tests/sdk/test-clipboard-history.ts'],
    notes: 'Action result response',
  },

  // ============================================================
  // WINDOW MANAGEMENT (4)
  // ============================================================
  {
    name: 'windowList',
    category: 'Window Management',
    status: 'partial',
    testFiles: ['tests/sdk/test-window-management.ts'],
    notes: 'List system windows',
  },
  {
    name: 'windowAction',
    category: 'Window Management',
    status: 'partial',
    testFiles: ['tests/sdk/test-window-management.ts'],
    notes: 'Window actions (focus/close/minimize)',
  },
  {
    name: 'windowListResult',
    category: 'Window Management',
    status: 'partial',
    testFiles: ['tests/sdk/test-window-management.ts'],
    notes: 'Window list response',
  },
  {
    name: 'windowActionResult',
    category: 'Window Management',
    status: 'partial',
    testFiles: ['tests/sdk/test-window-management.ts'],
    notes: 'Action result response',
  },

  // ============================================================
  // FILE SEARCH (2)
  // ============================================================
  {
    name: 'fileSearch',
    category: 'File Search',
    status: 'partial',
    testFiles: [
      'tests/sdk/test-file-search.ts',
      'tests/smoke/test-file-search-simple.ts',
      'tests/smoke/test-file-search-timeout.ts',
    ],
    notes: 'File search request',
  },
  {
    name: 'fileSearchResult',
    category: 'File Search',
    status: 'partial',
    testFiles: ['tests/sdk/test-file-search.ts'],
    notes: 'Search result response',
  },

  // ============================================================
  // SCREENSHOT (2)
  // ============================================================
  {
    name: 'captureScreenshot',
    category: 'Screenshot',
    status: 'tested',
    testFiles: [
      'tests/smoke/audit-capture-all.ts',
      'tests/smoke/audit-visual-test.ts',
    ],
    notes: 'Capture app window screenshot - used in visual tests',
  },
  {
    name: 'screenshotResult',
    category: 'Screenshot',
    status: 'tested',
    testFiles: ['tests/smoke/audit-capture-all.ts'],
    notes: 'Screenshot response with base64 PNG',
  },

  // ============================================================
  // STATE QUERY (2)
  // ============================================================
  {
    name: 'getState',
    category: 'State Query',
    status: 'untested',
    testFiles: [],
    notes: 'Query current UI state - needs test',
  },
  {
    name: 'stateResult',
    category: 'State Query',
    status: 'untested',
    testFiles: [],
    notes: 'State response - needs test',
  },

  // ============================================================
  // ELEMENT QUERY (2)
  // ============================================================
  {
    name: 'getElements',
    category: 'Element Query',
    status: 'untested',
    testFiles: [],
    notes: 'Query visible UI elements - needs test',
  },
  {
    name: 'elementsResult',
    category: 'Element Query',
    status: 'untested',
    testFiles: [],
    notes: 'Elements response - needs test',
  },

  // ============================================================
  // LAYOUT INFO (2)
  // ============================================================
  {
    name: 'getLayoutInfo',
    category: 'Layout Info',
    status: 'partial',
    testFiles: ['tests/smoke/test-layout-info-simple.ts'],
    notes: 'Layout information request',
  },
  {
    name: 'layoutInfoResult',
    category: 'Layout Info',
    status: 'partial',
    testFiles: ['tests/smoke/test-layout-info-simple.ts'],
    notes: 'Layout information response',
  },

  // ============================================================
  // ERROR REPORTING (1)
  // ============================================================
  {
    name: 'setError',
    category: 'Error Reporting',
    status: 'partial',
    testFiles: [
      'tests/smoke/test-error-handling.ts',
      'tests/smoke/test-script-crash.ts',
    ],
    notes: 'Script error message',
  },

  // ============================================================
  // SCRIPTLET OPERATIONS (4)
  // ============================================================
  {
    name: 'runScriptlet',
    category: 'Scriptlet',
    status: 'partial',
    testFiles: [
      'tests/smoke/test-scriptlet-macos-tools.ts',
      'tests/smoke/test-scriptlet-utility-tools.ts',
    ],
    notes: 'Run scriptlet with inputs',
  },
  {
    name: 'getScriptlets',
    category: 'Scriptlet',
    status: 'untested',
    testFiles: [],
    notes: 'List available scriptlets - needs test',
  },
  {
    name: 'scriptletList',
    category: 'Scriptlet',
    status: 'untested',
    testFiles: [],
    notes: 'Scriptlet list response - needs test',
  },
  {
    name: 'scriptletResult',
    category: 'Scriptlet',
    status: 'partial',
    testFiles: ['tests/smoke/test-scriptlet-macos-tools.ts'],
    notes: 'Scriptlet execution result',
  },

  // ============================================================
  // TEST INFRASTRUCTURE (2)
  // ============================================================
  {
    name: 'simulateClick',
    category: 'Test Infrastructure',
    status: 'partial',
    testFiles: ['tests/sdk/test-click-utils.ts'],
    notes: 'Simulate mouse click for testing',
  },
  {
    name: 'simulateClickResult',
    category: 'Test Infrastructure',
    status: 'partial',
    testFiles: ['tests/sdk/test-click-utils.ts'],
    notes: 'Click simulation result',
  },

  // ============================================================
  // DEBUG/VISUAL TESTING (2)
  // ============================================================
  {
    name: 'showGrid',
    category: 'Debug',
    status: 'partial',
    testFiles: [
      'tests/smoke/test-grid-div.ts',
      'tests/smoke/test-grid-dimensions.ts',
      'tests/smoke/test-grid-final.ts',
      'tests/smoke/test-grid-simple.ts',
      'tests/smoke/test-grid-now.ts',
      'tests/smoke/test-grid-quick.ts',
      'tests/smoke/test-debug-grid-bounds.ts',
    ],
    notes: 'Show debug grid overlay',
  },
  {
    name: 'hideGrid',
    category: 'Debug',
    status: 'partial',
    testFiles: ['tests/smoke/test-grid-div.ts'],
    notes: 'Hide debug grid overlay',
  },

  // ============================================================
  // ACTIONS API (2)
  // ============================================================
  {
    name: 'setActions',
    category: 'Actions API',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-actions.ts',
      'tests/smoke/test-actions-panel-quick.ts',
      'tests/smoke/test-actions-visual.ts',
      'tests/smoke/test-actions-autonomous.ts',
      'tests/smoke/test-actions-input-resize.ts',
      'tests/smoke/test-actions-click-outside.ts',
    ],
    notes: 'Set actions for actions panel - good coverage',
  },
  {
    name: 'actionTriggered',
    category: 'Actions API',
    status: 'tested',
    testFiles: [
      'tests/sdk/test-actions.ts',
      'tests/smoke/test-sdk-actions.ts',
    ],
    notes: 'Action triggered notification',
  },
];

// =============================================================================
// Coverage Analysis Functions
// =============================================================================

interface CoverageStats {
  total: number;
  tested: number;
  partial: number;
  untested: number;
  percentTested: number;
  percentCovered: number; // tested + partial
}

function calculateStats(): CoverageStats {
  const total = PROTOCOL_MESSAGES.length;
  const tested = PROTOCOL_MESSAGES.filter((m) => m.status === 'tested').length;
  const partial = PROTOCOL_MESSAGES.filter((m) => m.status === 'partial').length;
  const untested = PROTOCOL_MESSAGES.filter((m) => m.status === 'untested').length;

  return {
    total,
    tested,
    partial,
    untested,
    percentTested: Math.round((tested / total) * 100),
    percentCovered: Math.round(((tested + partial) / total) * 100),
  };
}

interface CategoryStats {
  category: string;
  total: number;
  tested: number;
  partial: number;
  untested: number;
}

function getCategoryStats(): CategoryStats[] {
  const categories = new Map<string, CategoryStats>();

  for (const msg of PROTOCOL_MESSAGES) {
    let stats = categories.get(msg.category);
    if (!stats) {
      stats = { category: msg.category, total: 0, tested: 0, partial: 0, untested: 0 };
      categories.set(msg.category, stats);
    }

    stats.total++;
    if (msg.status === 'tested') stats.tested++;
    else if (msg.status === 'partial') stats.partial++;
    else stats.untested++;
  }

  return Array.from(categories.values());
}

function getUntestedMessages(): ProtocolMessage[] {
  return PROTOCOL_MESSAGES.filter((m) => m.status === 'untested');
}

function getPartialMessages(): ProtocolMessage[] {
  return PROTOCOL_MESSAGES.filter((m) => m.status === 'partial');
}

// =============================================================================
// Report Generation
// =============================================================================

function generateReport(): void {
  const stats = calculateStats();
  const categoryStats = getCategoryStats();
  const untested = getUntestedMessages();
  const partial = getPartialMessages();

  console.log('');
  console.log('='.repeat(70));
  console.log('  PROTOCOL COVERAGE REPORT');
  console.log('='.repeat(70));
  console.log('');

  // Overall stats
  console.log('OVERALL COVERAGE');
  console.log('-'.repeat(40));
  console.log(`  Total Messages:   ${stats.total}`);
  console.log(`  Fully Tested:     ${stats.tested} (${stats.percentTested}%)`);
  console.log(`  Partially Tested: ${stats.partial}`);
  console.log(`  Untested:         ${stats.untested}`);
  console.log(`  Coverage:         ${stats.percentCovered}% (tested + partial)`);
  console.log('');

  // Visual bar
  const barWidth = 50;
  const testedBars = Math.round((stats.tested / stats.total) * barWidth);
  const partialBars = Math.round((stats.partial / stats.total) * barWidth);
  const untestedBars = barWidth - testedBars - partialBars;

  console.log('  [' + '#'.repeat(testedBars) + '~'.repeat(partialBars) + '.'.repeat(untestedBars) + ']');
  console.log('   # = tested  ~ = partial  . = untested');
  console.log('');

  // Category breakdown
  console.log('COVERAGE BY CATEGORY');
  console.log('-'.repeat(60));
  console.log('  Category                  Total  Tested  Partial  Untested');
  console.log('  ' + '-'.repeat(56));

  for (const cat of categoryStats) {
    const name = cat.category.padEnd(24);
    const total = cat.total.toString().padStart(5);
    const tested = cat.tested.toString().padStart(7);
    const partial = cat.partial.toString().padStart(8);
    const untested = cat.untested.toString().padStart(9);
    console.log(`  ${name}${total}${tested}${partial}${untested}`);
  }
  console.log('');

  // Untested messages
  if (untested.length > 0) {
    console.log('UNTESTED MESSAGES (' + untested.length + ')');
    console.log('-'.repeat(40));
    for (const msg of untested) {
      console.log(`  - ${msg.name} (${msg.category})`);
      if (msg.notes) console.log(`    ${msg.notes}`);
    }
    console.log('');
  }

  // Partial messages
  if (partial.length > 0) {
    console.log('PARTIALLY TESTED MESSAGES (' + partial.length + ')');
    console.log('-'.repeat(40));
    for (const msg of partial) {
      console.log(`  - ${msg.name} (${msg.category})`);
      if (msg.notes) console.log(`    ${msg.notes}`);
    }
    console.log('');
  }

  // High priority gaps
  console.log('HIGH PRIORITY GAPS');
  console.log('-'.repeat(40));
  const highPriority = [
    'forceSubmit',
    'setInput',
    'keyboard',
    'mouse',
    'getState',
    'getElements',
    'setPanel',
    'setPreview',
  ];

  for (const name of highPriority) {
    const msg = PROTOCOL_MESSAGES.find((m) => m.name === name);
    if (msg && msg.status === 'untested') {
      console.log(`  - ${msg.name}: ${msg.notes || 'needs test'}`);
    }
  }
  console.log('');

  console.log('='.repeat(70));
  console.log('  Run: bun run tests/protocol-coverage-matrix.ts');
  console.log('='.repeat(70));
  console.log('');
}

// =============================================================================
// Export for programmatic use
// =============================================================================

export { PROTOCOL_MESSAGES, calculateStats, getCategoryStats, getUntestedMessages, getPartialMessages };

export type { ProtocolMessage, CoverageStatus, CoverageStats, CategoryStats };

// =============================================================================
// Main execution
// =============================================================================

// Run when executed directly with bun
// @ts-ignore - import.meta.main is Bun-specific
if ((import.meta as { main?: boolean }).main) {
  generateReport();
}

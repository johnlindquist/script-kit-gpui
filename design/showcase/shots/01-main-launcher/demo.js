/* Demo script: 01-main-launcher — "type once, everything is searchable". */
SKDemo.define({
  id: "01-main-launcher",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  controls: {
    input: {
      target: '[data-demo-key="query"]',
      maxLength: 40,
      items: '[data-demo-role="result"]',
      matchAttribute: "data-demo-match",
    },
    list: {
      items: '[data-demo-role="result"]',
      state: { type: "attribute", name: "data-state", selected: "selected", hover: "hover" },
    },
  },
  states: {
    notesFiltered: [
      { op: "setState", target: '[data-demo-key="theme-designer"]', attribute: "data-state", value: null },
      { op: "setState", target: '[data-demo-key="open-notes"]', attribute: "data-state", value: "selected" },
    ],
  },
  steps: [
    { id: "intro", op: "caption", text: "Search scripts, files, clipboard, apps, and more.", holdMs: 1100 },
    { op: "pause", ms: 500 },
    {
      id: "type-notes",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "notes",
      clear: true,
      perCharacterMs: 90,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    { id: "notes-filtered", op: "applyState", name: "notesFiltered" },
    { op: "pause", ms: 700 },
    { op: "setText", target: '[data-demo-key="primary-label"]', text: "Open Notes" },
    { op: "pause", ms: 500 },
    {
      id: "actions",
      op: "keypress",
      keys: ["⌘", "K"],
      activate: '[data-demo-key="actions"]',
      holdMs: 700,
    },
    { op: "pause", ms: 300 },
    {
      id: "agent",
      op: "keypress",
      keys: ["⌘", "↵"],
      activate: '[data-demo-key="agent"]',
      holdMs: 700,
    },
    { op: "loop", delayMs: 1200 },
  ],
});

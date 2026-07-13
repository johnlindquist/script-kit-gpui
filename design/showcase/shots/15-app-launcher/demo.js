/* Demo script: 15-app-launcher — "type an app name, Return launches it". */
SKDemo.define({
  id: "15-app-launcher",
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
    unfiltered: [
      { op: "setText", target: '[data-demo-key="query"]', text: "" },
      { op: "show", target: "[data-demo-only]" },
      { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="finder"]' },
    ],
  },
  steps: [
    { id: "intro", op: "caption", text: "Type an app name and press Return.", holdMs: 1100 },
    { id: "unfiltered", op: "applyState", name: "unfiltered" },
    { op: "pause", ms: 900 },
    {
      id: "type-safari",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "safari",
      clear: true,
      perCharacterMs: 75,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    {
      id: "safari-filtered",
      op: "moveSelection",
      group: '[data-demo-role="result"]',
      to: '[data-demo-key="safari"]',
      holdMs: 300,
    },
    { op: "pause", ms: 500 },
    { id: "run-caption", op: "caption", text: "Return launches Safari.", holdMs: 900 },
    {
      id: "run",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="primary"]',
      holdMs: 700,
    },
    { op: "pause", ms: 400 },
    {
      id: "actions",
      op: "keypress",
      keys: ["⌘", "K"],
      activate: '[data-demo-key="actions"]',
      holdMs: 700,
    },
    { op: "loop", delayMs: 1200 },
  ],
});

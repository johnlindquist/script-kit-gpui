/* Demo script: 20-brain-inbox — "recent captures resurface before you search". */
SKDemo.define({
  id: "20-brain-inbox",
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
    dashFiltered: [
      { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="dash"]' },
      { op: "hide", target: '[data-demo-key="suggested-slot"]' },
    ],
  },
  steps: [
    { id: "intro", op: "caption", text: "Recent captures resurface before you search.", holdMs: 1100 },
    { op: "pause", ms: 500 },
    { id: "arrow-down-1", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="sideshow"]', holdMs: 350 },
    { id: "arrow-down-2", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="lively-rafter"]', holdMs: 550 },
    {
      id: "show-query",
      op: "patch",
      ops: [
        { op: "hide", target: '[data-demo-key="placeholder"]' },
        { op: "show", target: '[data-demo-key="query"]' },
      ],
    },
    {
      id: "type-dash",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "dash",
      clear: true,
      perCharacterMs: 75,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    { id: "dash-filtered", op: "applyState", name: "dashFiltered" },
    { op: "pause", ms: 650 },
    { id: "caption-search", op: "caption", text: "Search your memory like the launcher.", holdMs: 1100 },
    {
      id: "activate-ask",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="ask"]',
      holdMs: 700,
    },
    { op: "pause", ms: 300 },
    {
      id: "activate-agent",
      op: "keypress",
      keys: ["⌘", "↵"],
      activate: '[data-demo-key="agent"]',
      holdMs: 700,
    },
    { op: "loop", delayMs: 1200 },
  ],
});

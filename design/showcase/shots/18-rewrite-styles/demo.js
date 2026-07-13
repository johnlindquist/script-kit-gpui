/* Demo script: 18-rewrite-styles — "arrow through the rewrite styles, then
 * narrow the list with the dot grammar before firing the rewrite". */
SKDemo.define({
  id: "18-rewrite-styles",
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
    },
  },
  steps: [
    {
      id: "intro",
      op: "caption",
      text: "Choose Professional, Concise, Friendly, or Direct.",
      holdMs: 1300,
    },
    { id: "arrow-down-concise", op: "keypress", keys: ["↓"], holdMs: 250 },
    {
      op: "moveSelection",
      group: '[data-demo-role="result"]',
      to: '[data-demo-key="concise"]',
      holdMs: 350,
    },
    { id: "arrow-down-friendly", op: "keypress", keys: ["↓"], holdMs: 250 },
    {
      id: "friendly-selected",
      op: "moveSelection",
      group: '[data-demo-role="result"]',
      to: '[data-demo-key="friendly"]',
      holdMs: 500,
    },
    { id: "hide-caret", op: "hide", target: '[data-demo-key="caret"]' },
    {
      id: "type-friendly",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "friendly",
      clear: false,
      perCharacterMs: 65,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    {
      id: "dot-grammar-caption",
      op: "caption",
      text: "The dot grammar filters rewrite styles.",
      holdMs: 1100,
    },
    {
      id: "enter-key",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="rewrite-affordance"]',
      holdMs: 500,
    },
    {
      id: "sparkle-pulse",
      op: "effect",
      name: "pulse",
      target: '[data-demo-key="friendly-sparkle"]',
      durationMs: 600,
      holdMs: 650,
    },
    { op: "pause", ms: 400 },
    {
      id: "actions-key",
      op: "keypress",
      keys: ["⌘", "K"],
      activate: '[data-demo-key="actions-affordance"]',
      holdMs: 700,
    },
    { op: "loop", delayMs: 1200 },
  ],
});

/* Demo script: 05-emoji-picker — "stay on the keyboard from search to paste". */
SKDemo.define({
  id: "05-emoji-picker",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  controls: {
    list: {
      items: ".cell",
      // Grid's existing selection vocabulary is data-selected="true" (see
      // .cell[data-selected="true"] in index.html). Exploration hover uses a
      // distinct token ("hover") so pointermove can never clear the real
      // selected cell — the runner only strips elements whose value equals
      // vocab.hover, which "true" (selected) never does.
      state: { type: "attribute", name: "data-selected", selected: "true", hover: "hover" },
    },
  },
  steps: [
    { id: "intro", op: "caption", text: "Stay on the keyboard from search to paste.", holdMs: 1100 },
    { op: "pause", ms: 500 },

    { id: "arrow-right-a1", op: "keypress", keys: ["→"], holdMs: 220 },
    { id: "arrow-right-a2", op: "keypress", keys: ["→"], holdMs: 220 },
    {
      op: "moveSelection",
      group: ".cell",
      to: '[data-demo-key="emoji-0-2"]',
      state: { type: "attribute", name: "data-selected", selected: "true" },
      holdMs: 500,
    },

    { id: "emoji-row-2", op: "keypress", keys: ["↓"], holdMs: 220 },
    {
      op: "moveSelection",
      group: ".cell",
      to: '[data-demo-key="emoji-1-2"]',
      state: { type: "attribute", name: "data-selected", selected: "true" },
      holdMs: 500,
    },

    { id: "arrow-right-b1", op: "keypress", keys: ["→"], holdMs: 220 },
    { id: "arrow-right-b2", op: "keypress", keys: ["→"], holdMs: 220 },
    {
      op: "moveSelection",
      group: ".cell",
      to: '[data-demo-key="emoji-1-4"]',
      state: { type: "attribute", name: "data-selected", selected: "true" },
      holdMs: 500,
    },

    { id: "outro", op: "caption", text: "Arrow keys move through the grid.", holdMs: 1100 },

    {
      id: "paste",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="primary"]',
      holdMs: 700,
    },
    { op: "pause", ms: 300 },
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

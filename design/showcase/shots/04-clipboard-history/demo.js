/* Demo script: 04-clipboard-history — "every copied item keeps a preview and metadata". */
SKDemo.define({
  id: "04-clipboard-history",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  controls: {
    list: {
      items: '[data-demo-role="result"]',
      state: { type: "attribute", name: "data-state", selected: "selected", hover: "hover" },
    },
  },
  steps: [
    { id: "intro", op: "caption", text: "Every copied item keeps a preview and metadata.", holdMs: 1200 },
    { op: "pause", ms: 500 },

    { id: "arrow-down-1", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="row-git-rebase"]', holdMs: 200 },
    { op: "setText", target: '[data-demo-key="preview-text"]', text: "git rebase -i HEAD~3" },
    { op: "setText", target: '[data-demo-key="info-size"]', text: "20 bytes" },
    { op: "setText", target: '[data-demo-key="info-chars"]', text: "20" },
    { op: "setText", target: '[data-demo-key="info-lines"]', text: "1" },
    { op: "pause", ms: 700 },

    { id: "arrow-down-2", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="row-hex"]', holdMs: 200 },
    { op: "setText", target: '[data-demo-key="preview-text"]', text: "#7C5CFF" },
    { op: "setText", target: '[data-demo-key="info-size"]', text: "7 bytes" },
    { op: "setText", target: '[data-demo-key="info-chars"]', text: "7" },
    { id: "hex-preview", op: "setText", target: '[data-demo-key="info-lines"]', text: "1" },
    { op: "pause", ms: 550 },

    { id: "preview-follows", op: "caption", text: "The preview follows the highlighted item.", holdMs: 1200 },

    {
      id: "paste",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="primary"]',
      holdMs: 500,
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

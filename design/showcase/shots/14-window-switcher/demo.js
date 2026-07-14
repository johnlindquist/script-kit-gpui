/* Demo script: 14-window-switcher — "search and switch across every open window". */
SKDemo.define({
  id: "14-window-switcher",
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
      state: { type: "class", selected: "selected", hover: "hover-demo" },
    },
  },
  states: {
    cmuxDetail: [
      { op: "setText", target: '[data-demo-key="detail-title"]', text: "· Claude Code" },
      { op: "setText", target: '[data-demo-key="detail-sub"]', text: "cmux" },
      { op: "hide", target: '[data-demo-key="detail-bounds"]' },
    ],
    uadDetail: [
      { op: "setText", target: '[data-demo-key="detail-title"]', text: "UAD Meter" },
      { op: "setText", target: '[data-demo-key="detail-sub"]', text: "UAD Meter & Control Panel" },
      { op: "show", target: '[data-demo-key="detail-bounds"]' },
    ],
  },
  steps: [
    { id: "intro", op: "caption", text: "Search and switch across every open window.", holdMs: 1200 },
    { op: "pause", ms: 500 },

    { id: "select-cmux", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="cmux"]', holdMs: 200 },
    { op: "applyState", name: "cmuxDetail" },
    { op: "pause", ms: 750 },

    { id: "select-uad", op: "keypress", keys: ["↑"], holdMs: 250 },
    { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="uad"]', holdMs: 200 },
    { op: "applyState", name: "uadDetail" },
    { op: "pause", ms: 600 },

    { op: "hide", target: '[data-demo-key="placeholder"]' },
    { op: "show", target: '[data-demo-key="query"]' },
    {
      id: "type-uad",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "uad",
      clear: true,
      perCharacterMs: 90,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    { id: "uad-filtered", op: "setText", target: '[data-demo-key="count"]', text: "1 window" },
    { op: "pause", ms: 700 },

    { id: "switch-hint", op: "effect", name: "pulse", target: '[data-demo-key="switch-btn"]', durationMs: 650, holdMs: 550 },
    { id: "run-window", op: "keypress", keys: ["↵"], activate: '[data-demo-key="run"]', holdMs: 550 },
    { op: "pause", ms: 400 },

    { id: "return-caption", op: "caption", text: "Return switches to the highlighted window.", holdMs: 1200 },

    { id: "esc-back", op: "keypress", keys: ["Esc"], activate: '[data-demo-key="back-btn"]', holdMs: 500 },
    { op: "loop", delayMs: 1200 },
  ],
});

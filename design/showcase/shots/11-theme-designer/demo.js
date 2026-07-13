/* Demo script: 11-theme-designer — "preview themes before you apply them". */
SKDemo.define({
  id: "11-theme-designer",
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
      state: { type: "class", selected: "sel", hover: "hov" },
    },
  },
  states: {
    dracula: [
      { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="theme-dracula"]', holdMs: 200 },
      {
        op: "patch",
        ops: [
          { op: "setText", target: '[data-demo-key="base"]', text: "Base: Dracula" },
          { op: "setState", target: '[data-demo-key="accent-chip"]', attribute: "data-demo-theme", value: "dracula" },
          { op: "setState", target: '[data-demo-key="bg-chip"]', attribute: "data-demo-theme", value: "dracula" },
          { op: "setText", target: '[data-demo-key="accent-text"]', text: "#BD93F9" },
          { op: "setText", target: '[data-demo-key="bg-text"]', text: "#282A36" },
        ],
      },
    ],
    nord: [
      { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="theme-nord"]', holdMs: 200 },
      {
        op: "patch",
        ops: [
          { op: "setText", target: '[data-demo-key="base"]', text: "Base: Nord" },
          { op: "setState", target: '[data-demo-key="accent-chip"]', attribute: "data-demo-theme", value: "nord" },
          { op: "setState", target: '[data-demo-key="bg-chip"]', attribute: "data-demo-theme", value: "nord" },
          { op: "setText", target: '[data-demo-key="accent-text"]', text: "#88C0D0" },
          { op: "setText", target: '[data-demo-key="bg-text"]', text: "#2E3440" },
        ],
      },
    ],
    tokyoNight: [
      { op: "moveSelection", group: '[data-demo-role="result"]', to: '[data-demo-key="theme-tokyo-night"]', holdMs: 200 },
      {
        op: "patch",
        ops: [
          { op: "setText", target: '[data-demo-key="base"]', text: "Base: Tokyo Night" },
          { op: "setState", target: '[data-demo-key="accent-chip"]', attribute: "data-demo-theme", value: "tokyo-night" },
          { op: "setState", target: '[data-demo-key="bg-chip"]', attribute: "data-demo-theme", value: "tokyo-night" },
          { op: "setText", target: '[data-demo-key="accent-text"]', text: "#7AA2F7" },
          { op: "setText", target: '[data-demo-key="bg-text"]', text: "#1A1B26" },
        ],
      },
    ],
  },
  steps: [
    { id: "intro", op: "caption", text: "Preview themes before you apply them.", holdMs: 1100 },
    { op: "pause", ms: 500 },
    { id: "arrow-down-1", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "applyState", name: "dracula" },
    { op: "pause", ms: 700 },
    { id: "arrow-down-2", op: "keypress", keys: ["↓"], holdMs: 250 },
    { op: "applyState", name: "nord" },
    { op: "pause", ms: 700 },
    { id: "facts-live", op: "caption", text: "The facts panel updates live.", holdMs: 1100 },
    {
      id: "tokyo-night",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "tokyo",
      clear: true,
      perCharacterMs: 80,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    { op: "applyState", name: "tokyoNight" },
    { op: "pause", ms: 700 },
    { id: "apply", op: "keypress", keys: ["↵"], activate: '[data-demo-key="apply"]', holdMs: 500 },
    { id: "actions", op: "keypress", keys: ["⌘", "K"], activate: '[data-demo-key="actions"]', holdMs: 700 },
    { op: "loop", delayMs: 1200 },
  ],
});

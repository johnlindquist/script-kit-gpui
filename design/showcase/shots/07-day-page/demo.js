/* Demo script: 07-day-page — "the clipboard → day page loop is invisible". */
SKDemo.define({
  id: "07-day-page",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  steps: [
    { id: "intro", op: "caption", text: "Focus at the top; captured history underneath.", holdMs: 1100 },
    { id: "arrow-down", op: "keypress", keys: ["↓"], holdMs: 250 },
    { id: "caret-to-focus-task", op: "moveNode", target: '[data-demo-key="caret"]', into: '[data-demo-key="ln-focus-task"]' },
    { id: "focus-task-checked", op: "setText", target: '[data-demo-key="focus-task-checkbox"]', text: "[x]" },
    { op: "pause", ms: 700 },
    {
      id: "reference-focused",
      op: "moveNode",
      target: '[data-demo-key="caret"]',
      into: '[data-demo-key="ln-ref-entry"]',
    },
    { op: "pause", ms: 500 },
    { id: "caption-provenance", op: "caption", text: "Copied links return with provenance.", holdMs: 1100 },
    { id: "return-key", op: "keypress", keys: ["↵"], holdMs: 250 },
    {
      id: "reference-pulse",
      op: "patch",
      ops: [
        { op: "effect", name: "pulse", target: '[data-demo-key="ref-lbl"]', durationMs: 700, holdMs: 0 },
        { op: "effect", name: "pulse", target: '[data-demo-key="ref-dst"]', durationMs: 700, holdMs: 0 },
      ],
    },
    { op: "pause", ms: 700 },
    { id: "caption-return-reopens", op: "caption", text: "Return reopens the original clipboard entry.", holdMs: 1200 },
    { op: "loop", delayMs: 1200 },
  ],
});

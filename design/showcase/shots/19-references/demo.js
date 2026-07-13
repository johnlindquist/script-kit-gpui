/* Demo script: 19-references — "kit:// links keep the original resource attached." */
SKDemo.define({
  id: "19-references",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  hudPlacement: "top-right",
  steps: [
    { id: "intro", op: "caption", text: "kit:// links keep the original resource attached.", holdMs: 1200 },
    { id: "tab", op: "keypress", keys: ["⇥"], holdMs: 300 },
    {
      id: "pulse-label",
      op: "effect",
      name: "pulse",
      target: '[data-demo-key="clipboard-reference"]',
      durationMs: 650,
    },
    {
      id: "pulse-dest-1",
      op: "effect",
      name: "pulse",
      target: '[data-demo-key="clipboard-reference-dest-1"]',
      durationMs: 650,
    },
    {
      id: "pulse-dest-2",
      op: "effect",
      name: "pulse",
      target: '[data-demo-key="clipboard-reference-dest-2"]',
      durationMs: 650,
    },
    { id: "reference-activated", op: "keypress", keys: ["↵"], holdMs: 700 },
    { id: "outro", op: "caption", text: "Open the original Clipboard entry.", holdMs: 1200 },
    { op: "loop", delayMs: 1200 },
  ],
});

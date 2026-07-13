/* Demo script: 06-notes — "a markdown scratchpad with live checklists". */
SKDemo.define({
  id: "06-notes",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  steps: [
    { id: "intro", op: "caption", text: "A markdown scratchpad with live checklists.", holdMs: 1300 },
    { op: "pause", ms: 500 },
    {
      id: "check-hero-demo",
      op: "patch",
      ops: [
        { op: "hide", target: '[data-demo-key="task-hero-demo-off"]' },
        { op: "show", target: '[data-demo-key="task-hero-demo-on"]' },
      ],
    },
    { op: "pause", ms: 500 },
    {
      id: "two-tasks-checked",
      op: "patch",
      ops: [
        { op: "hide", target: '[data-demo-key="task-launch-post-off"]' },
        { op: "show", target: '[data-demo-key="task-launch-post-on"]' },
      ],
    },
    { op: "pause", ms: 650 },
    {
      id: "saved-pulse",
      op: "effect",
      name: "pulse",
      target: '[data-demo-key="updated-status"]',
      durationMs: 700,
      holdMs: 700,
    },
    { op: "pause", ms: 300 },
    { id: "outro", op: "caption", text: "Changes are already saved.", holdMs: 1300 },
    { op: "loop", delayMs: 1200 },
  ],
});

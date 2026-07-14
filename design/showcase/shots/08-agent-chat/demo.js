/* Demo script: 08-agent-chat — "Agent Chat streams real markdown." */
SKDemo.define({
  id: "08-agent-chat",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  steps: [
    { id: "intro", op: "caption", text: "Agent Chat streams real markdown.", holdMs: 1200 },

    { id: "hide-stream", op: "hide", target: '[data-demo-role="stream-item"]' },
    {
      id: "thinking-pulse",
      op: "patch",
      ops: [
        { op: "effect", name: "thinking", target: '[data-demo-key="thinking-tri"]', durationMs: 1500 },
        { op: "effect", name: "thinking", target: '[data-demo-key="thinking-lbl"]', durationMs: 1500 },
      ],
    },
    { op: "pause", ms: 1500 },

    { id: "progress-caption", op: "caption", text: "Progress stays visible while the answer is prepared.", holdMs: 1100 },

    { id: "show-bullet-1", op: "show", target: '[data-demo-key="bullet-1"]' },
    { op: "pause", ms: 280 },
    { id: "show-bullet-2", op: "show", target: '[data-demo-key="bullet-2"]' },
    { op: "pause", ms: 480 },

    {
      id: "answer-streamed",
      op: "show",
      target:
        '[data-demo-key="bullet-3"], [data-demo-key="bullet-4"], [data-demo-key="bullet-5"], [data-demo-key="tip"]',
    },
    { op: "pause", ms: 700 },

    { id: "markdown-caption", op: "caption", text: "The response arrives as structured markdown.", holdMs: 1100 },

    {
      id: "paste-response",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="paste-response"]',
      holdMs: 650,
    },
    { op: "pause", ms: 350 },
    {
      id: "actions",
      op: "keypress",
      keys: ["⌘", "K"],
      activate: '[data-demo-key="actions"]',
      holdMs: 650,
    },

    { op: "loop", delayMs: 1200 },
  ],
});

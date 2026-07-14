/* Demo script: 21-dictation — "dictation is live: waveform, timer, and the
 * overlay's mic/stop/cancel controls all stay reachable without a popup."
 * Pure visual simulation: no microphone access, no backend, no controls. */
SKDemo.define({
  id: "21-dictation",
  hudPlacement: "top-right",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  steps: [
    {
      id: "intro",
      op: "caption",
      text: "The waveform and timer show active dictation.",
      holdMs: 1200,
    },

    {
      id: "waveform-active",
      op: "effect",
      name: "waveform",
      target: '[data-demo-key="waveform"]',
      durationMs: 2000,
      holdMs: 0,
    },
    { id: "timer-08", op: "setText", target: '[data-demo-key="timer"]', text: "0:08" },
    { op: "pause", ms: 700 },
    { id: "timer-09", op: "setText", target: '[data-demo-key="timer"]', text: "0:09" },
    { op: "pause", ms: 700 },

    {
      id: "mic-caption",
      op: "caption",
      text: "Choose the active microphone without leaving the overlay.",
      holdMs: 1100,
    },
    {
      id: "mic-pulse",
      op: "effect",
      name: "pulse",
      target: '[data-demo-key="mic"]',
      durationMs: 600,
    },
    { op: "pause", ms: 400 },

    {
      id: "dictation-stopped",
      op: "keypress",
      keys: ["⇧", "⌘", ";"],
      holdMs: 600,
    },
    {
      id: "stop-pulse",
      op: "patch",
      ops: [
        { op: "effect", name: "pulse", target: '[data-demo-key="stop-label"]', durationMs: 600 },
        { op: "effect", name: "pulse", target: '[data-demo-key="stop-keys"]', durationMs: 600 },
      ],
    },
    { op: "pause", ms: 700 },

    {
      id: "cancel-pulse",
      op: "patch",
      ops: [
        { op: "effect", name: "pulse", target: '[data-demo-key="cancel-label"]', durationMs: 600 },
        { op: "effect", name: "pulse", target: '[data-demo-key="cancel-undo"]', durationMs: 600 },
      ],
    },
    { op: "pause", ms: 700 },
    { id: "timer-reset", op: "setText", target: '[data-demo-key="timer"]', text: "0:07" },
    { op: "pause", ms: 500 },

    { op: "loop", delayMs: 1200 },
  ],
});

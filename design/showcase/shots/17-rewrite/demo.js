/* Demo script: 17-rewrite — "the rewrite streams in, then pastes back into
 * the original TextEdit selection". Demo-driven scene: no exploration
 * controls, the whole cycle is a single narrated pass. */
SKDemo.define({
  id: "17-rewrite",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  steps: [
    {
      id: "intro",
      op: "caption",
      text: "The original TextEdit selection stays attached.",
      holdMs: 1300,
    },
    {
      id: "clear-response",
      op: "setText",
      target: '[data-demo-key="response"]',
      text: "",
    },
    {
      id: "streaming-caption",
      op: "caption",
      text: "The rewrite streams in.",
      holdMs: 500,
    },
    {
      id: "type-response",
      op: "typeInto",
      target: '[data-demo-key="response"]',
      text: "Hi, I think we should consider rescheduling the meeting, as something has come up and things are a bit hectic right now. Please let me know if that works for you.",
      clear: true,
      perCharacterMs: 18,
    },
    { op: "pause", ms: 500 },
    {
      id: "paste-key",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="paste-response"]',
      holdMs: 500,
    },
    {
      id: "rewrite-pasted",
      op: "setText",
      target: '[data-demo-key="te-seltext"]',
      text: "Please let me know if that works for you.",
    },
    {
      id: "panel-fade",
      op: "effect",
      name: "fadeOut",
      target: '[data-demo-key="panel"]',
      durationMs: 220,
    },
    { op: "pause", ms: 900 },
    { op: "loop", delayMs: 1200 },
  ],
});

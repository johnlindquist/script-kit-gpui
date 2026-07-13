/* Demo script: 13-agent-chat-composer — "attach context explicitly before
 * you send" walkthrough of the composer's guidance-row shortcuts. */
SKDemo.define({
  id: "13-agent-chat-composer",
  initialHoldMs: 900,
  idleResetMs: 8000,
  loopDelayMs: 1200,
  controls: {
    list: {
      items: '[data-demo-role="guidance"]',
      state: { type: "attribute", name: "data-state", selected: "selected", hover: "selected" },
    },
  },
  steps: [
    { id: "intro", op: "caption", text: "Attach context explicitly before you send.", holdMs: 1100 },

    { id: "key-at", op: "keypress", keys: ["@"], holdMs: 500 },
    { op: "moveSelection", group: '[data-demo-role="guidance"]', to: '[data-demo-key="attach-context"]', holdMs: 250 },
    { op: "pause", ms: 650 },

    { id: "key-slash", op: "keypress", keys: ["/"], holdMs: 500 },
    { op: "moveSelection", group: '[data-demo-role="guidance"]', to: '[data-demo-key="use-skill"]', holdMs: 250 },
    { op: "pause", ms: 650 },

    { id: "key-newline", op: "keypress", keys: ["⇧", "↵"], holdMs: 500 },
    { op: "moveSelection", group: '[data-demo-role="guidance"]', to: '[data-demo-key="add-newline"]', holdMs: 250 },
    { op: "pause", ms: 650 },

    { id: "key-previous-chats", op: "keypress", keys: ["⌘", "P"], holdMs: 500 },
    { op: "moveSelection", group: '[data-demo-role="guidance"]', to: '[data-demo-key="previous-chats"]', holdMs: 250 },
    { op: "pause", ms: 650 },

    {
      id: "key-chat-actions",
      op: "keypress",
      keys: ["⌘", "K"],
      activate: '[data-demo-key="actions"]',
      persist: true,
      holdMs: 500,
    },
    { id: "context-shortcuts", op: "moveSelection", group: '[data-demo-role="guidance"]', to: '[data-demo-key="chat-actions"]', holdMs: 700 },
    { op: "pause", ms: 650 },

    { op: "setState", target: '[data-demo-key="chat-actions"]', attribute: "data-state", value: null },
    { op: "setState", target: '[data-demo-key="actions"]', attribute: "data-selected", value: null },
    { id: "key-send", op: "keypress", keys: ["↵"], activate: '[data-demo-key="send"]', holdMs: 700 },
    { op: "pause", ms: 400 },

    { op: "loop", delayMs: 1200 },
  ],
});

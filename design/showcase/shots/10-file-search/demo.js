/* Demo script: 10-file-search — "refine a file search, inspect details, then run". */
SKDemo.define({
  id: "10-file-search",
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
      state: { type: "attribute", name: "data-state", selected: "selected", hover: "hover" },
    },
  },
  steps: [
    { id: "intro", op: "caption", text: "Refine file search, inspect details, then run.", holdMs: 1100 },
    { op: "pause", ms: 500 },
    {
      id: "type-plist",
      op: "typeInto",
      target: '[data-demo-key="query"]',
      text: "plist",
      clear: true,
      perCharacterMs: 75,
      filter: { items: '[data-demo-role="result"]', matchAttribute: "data-demo-match" },
    },
    { op: "pause", ms: 400 },
    {
      id: "select-plist",
      op: "moveSelection",
      group: '[data-demo-role="result"]',
      to: '[data-demo-key="launch-agent-plist"]',
      holdMs: 200,
    },
    {
      id: "plist-preview",
      op: "patch",
      ops: [
        { op: "setText", target: '[data-demo-key="preview-title"]', text: "LaunchAgent.plist" },
        { op: "setText", target: '[data-demo-key="preview-path"]', text: "/Users/johnlindquist/dev/beads/npm-package/LaunchAgent.plist" },
        { op: "setText", target: '[data-demo-key="preview-size"]', text: "196 B" },
        { op: "setText", target: '[data-demo-key="preview-modified"]', text: "May 21" },
        { op: "setText", target: '[data-demo-key="preview-type"]', text: "File" },
      ],
    },
    { op: "pause", ms: 900 },
    {
      id: "run",
      op: "keypress",
      keys: ["↵"],
      activate: '[data-demo-key="run"]',
      holdMs: 700,
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

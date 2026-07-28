/* One Conversation, Three Modes — schema v3 story.
 *
 * PROVES: the settled product decision that Ask / Work / Flow are MODES of a
 * single Conversation shell, not three surfaces. The assertion
 * `conversation.same-shell-rects` fails if any of the five structural nodes
 * moves between modes.
 *
 * Deliberately uses ONE surface with no showSurface/hideSurface anywhere: a
 * mode change that swapped surfaces would prove the opposite of the claim.
 *
 * Every action is an idempotent set*, so seeking to any chapter yields the
 * same state as playing into it (see tests/story-seek-determinism.test.mjs).
 */
(function () {
  "use strict";
  var story = {
    storyVersion: 3,
    id: "10-conversation-three-modes",
    title: "One Conversation, Three Modes",
    blurb:
      "Ask, Work, and Flow are the same shell. Identity, composer, context lane, transcript, and footer keep identical geometry; only text and chips change.",
    covers: ["A01", "A03", "A04", "A07", "A10", "B01", "C01", "C02", "H04"],
    effort: "new-design",
    presentation: "stack",
    durationMs: 12000,
    loop: true,
    surfaces: [{ id: "conversation", fixture: "conversation", role: "window", initial: true }],
    chapters: [
      { id: "ask", label: "Ask — zero context", at: 0 },
      { id: "ask-answer", label: "Ask answers", at: 2200 },
      { id: "work", label: "Work — explicit grants", at: 5200 },
      { id: "flow", label: "Flow — governed by a definition", at: 8600 },
    ],
    snapshots: [
      { chapter: "ask", name: "conversation-ask-rest" },
      { chapter: "work", name: "conversation-work-rest" },
      { chapter: "flow", name: "conversation-flow-rest" },
    ],
    assertions: [
      {
        id: "conversation.same-shell-rects",
        kind: "rectEquals",
        surface: "conversation",
        baselineChapter: "ask",
        atChapters: ["work", "flow"],
        selectors: [
          "[data-conversation-identity]",
          "[data-conversation-composer]",
          "[data-conversation-context-lane]",
          "[data-conversation-transcript]",
          "[data-native-footer]",
        ],
      },
      {
        id: "conversation.mode-changes-without-surface-swap",
        kind: "actionKindsAbsent",
        surface: "conversation",
        kinds: ["showSurface", "hideSurface"],
      },
      {
        id: "conversation.ask-receipt-attempted-zero",
        kind: "receiptAtChapter",
        surface: "conversation",
        chapter: "ask-answer",
        expect: { attempted: 0, resolved: 0, failed: 0, outcome: "none" },
      },
    ],
    actions: [
      // ── Ask: zero local context, locked policy, no add-context control ──
      {
        at: 0,
        kind: "setConversationMode",
        surface: "conversation",
        mode: "ask",
        identity: { label: "Ask", subject: "", authority: "No local context" },
        contextPolicy: "none",
      },
      { at: 0, kind: "setContextGrants", surface: "conversation", grants: [] },
      {
        at: 0,
        kind: "setListRows",
        surface: "conversation",
        sections: [],
      },
      { at: 0, kind: "setText", surface: "conversation", as: "composer", text: "" },
      {
        at: 200,
        duration: 1400,
        kind: "type",
        surface: "conversation",
        as: "composer",
        text: "difference between a trait object and impl Trait?",
      },
      { at: 1800, kind: "pressKey", surface: "conversation", key: "Enter", outcome: "send" },
      {
        at: 1900,
        kind: "appendMessage",
        surface: "conversation",
        role: "user",
        msgId: "ask-u1",
        text: "difference between a trait object and impl Trait?",
      },
      { at: 1900, kind: "setText", surface: "conversation", as: "composer", text: "" },
      {
        at: 2200,
        duration: 2200,
        kind: "streamText",
        surface: "conversation",
        msgId: "ask-a1",
        text: "dyn Trait erases the type and dispatches through a vtable; impl Trait keeps one concrete type resolved at compile time.",
      },
      {
        at: 4400,
        kind: "setContextReceipt",
        surface: "conversation",
        receipt: { id: "r-ask", turnId: "ask-a1", outcome: "none", attempted: [], resolved: [], failed: [] },
      },

      // ── Work: same shell, explicit grants, add-context now permitted ──
      {
        at: 5200,
        kind: "setConversationMode",
        surface: "conversation",
        mode: "work",
        identity: { label: "Work", subject: "· Build", authority: "Build profile · 2 grants" },
        contextPolicy: "explicit",
      },
      {
        at: 5200,
        kind: "setContextGrants",
        surface: "conversation",
        grants: [
          {
            id: "g-note",
            kind: "note",
            label: "Design Contract Notes",
            detail: "Selected block · 284 chars",
            state: "staged",
            bytes: 284,
            tokens: 71,
            lifetime: "turn",
            removable: true,
          },
          {
            id: "g-dir",
            kind: "file",
            label: "src/design_contract",
            detail: "Directory grant · session",
            state: "resolved",
            lifetime: "session",
            removable: true,
          },
        ],
      },
      {
        at: 5400,
        kind: "setContextReceipt",
        surface: "conversation",
        receipt: {
          id: "r-work",
          turnId: "work-1",
          outcome: "resolved",
          attempted: [{ id: "g-note" }, { id: "g-dir" }],
          resolved: [{ id: "g-note" }, { id: "g-dir" }],
          failed: [],
        },
      },
      // Exactly one queued turn is legal; schema validation rejects a second.
      {
        at: 6800,
        kind: "setQueuedTurn",
        surface: "conversation",
        turn: { id: "q1", text: "Also turn this into a flow.", state: "queued", position: 1 },
      },
      { at: 8200, kind: "setQueuedTurn", surface: "conversation", turn: null },

      // ── Flow: same shell again, governed by an mdflow definition ──
      {
        at: 8600,
        kind: "setConversationMode",
        surface: "conversation",
        mode: "flow",
        identity: {
          label: "Flow",
          subject: "· Deep Improvements",
          authority: "mdflow · Global · Claude",
          flow: {
            id: "deep-improvements",
            engine: "Claude",
            origin: "Package",
            source: "@johnlindquist/flows",
            scope: "Global",
            definitionState: "ready",
          },
        },
        contextPolicy: "flow-governed",
      },
      {
        at: 8600,
        kind: "setContextGrants",
        surface: "conversation",
        grants: [
          {
            id: "g-repo",
            kind: "file",
            label: "script-kit-gpui",
            detail: "Flow cwd",
            state: "resolved",
            lifetime: "session",
            removable: false,
          },
        ],
      },
      {
        at: 10800,
        kind: "setArtifactState",
        surface: "conversation",
        artifact: {
          kind: "note",
          label: "Improvement notes",
          path: "~/.scriptkit/brain/notes/improvements.md",
          state: "saved",
          sourceTurnId: "flow-1",
        },
      },
    ],
  };
  window.StoryPlayer.mount({
    root: document.querySelector("[data-story-root]") || document.body,
    story: story,
    autoplay: true,
  });
})();

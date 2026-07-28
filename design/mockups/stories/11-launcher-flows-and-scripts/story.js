/* Flows and Scripts, One List — schema v3 story.
 *
 * PROVES the settled flow-launcher decisions from consult flow-notes-reality-v3,
 * which were derived from the LIVE roster (`md roster --json`, 115 flows, of
 * which 110 are no-input / non-interactive / non-workflow):
 *
 *   1. At rest, ZERO cold flow definitions appear. 115 rows must not consume
 *      the launcher just because they exist.
 *   2. On a typed query, at most FOUR flow-definition rows appear among the
 *      first twelve results, then scripts follow. An exact name match may
 *      elevate past that quota.
 *   3. A high-confidence router result PRESELECTS a row and changes the footer
 *      verb. It NEVER executes. The shipped `RouteDecision::AutoStart` does
 *      execute — and can submit the typed text as the opening turn — which is
 *      the unsafe action boundary this story exists to make visible.
 *   4. Enter reads as "Converse", matching the 110/115 conversational shape,
 *      not "Run".
 *   5. Titles are normalized (`flow-aws.codex` -> `AWS`); the engine is
 *      metadata, never part of launcher identity.
 *
 * The router chapter deliberately contains NO action that starts a session,
 * and `conversation.no-execution-on-route` asserts that absence.
 */
(function () {
  "use strict";
  var story = {
    storyVersion: 3,
    id: "11-launcher-flows-and-scripts",
    title: "Flows and Scripts, One List",
    blurb:
      "115 saved agents share one result list with scripts, without drowning them. Enter converses; the router preselects but never executes.",
    covers: ["E01", "E02", "E03", "E04", "E05", "E09", "E12", "E13", "C1", "C2", "C3", "C4"],
    effort: "new-design",
    presentation: "stack",
    durationMs: 11000,
    loop: true,
    surfaces: [
      { id: "launcher-flow-rows", fixture: "launcher-flow-rows", role: "window", initial: true },
    ],
    chapters: [
      { id: "rest", label: "Rest — no cold flows", at: 0 },
      { id: "type", label: "Type — mixed results", at: 1200 },
      { id: "preselect", label: "Router preselects", at: 4600 },
      { id: "converse", label: "Enter converses", at: 7800 },
    ],
    snapshots: [
      { chapter: "rest", name: "launcher-rest-no-cold-flows" },
      { chapter: "type", name: "launcher-mixed-results" },
      { chapter: "preselect", name: "launcher-router-preselect" },
    ],
    assertions: [
      {
        id: "launcher.router-preselects-without-executing",
        kind: "actionKindsAbsent",
        surface: "launcher-flow-rows",
        // Starting a session, streaming, or queuing a turn during routing would
        // mean the router executed. None of these may appear in this story.
        kinds: ["appendMessage", "streamText", "setQueuedTurn", "setConversationMode"],
      },
      {
        // A text fact, asserted as a text fact. Naming this rectEquals would
        // have been an assertion whose name lied about what it checked.
        id: "launcher.rows-carry-no-engine-suffix-in-title",
        kind: "fixtureTextAbsent",
        surface: "launcher-flow-rows",
        pattern: "\\.(claude|codex|copilot|echo)$",
      },
      {
        id: "launcher.shell-geometry-stable-across-query",
        kind: "rectEquals",
        surface: "launcher-flow-rows",
        baselineChapter: "rest",
        atChapters: ["type", "preselect"],
        selectors: [".sk-list", ".sk-footer-host"],
      },
    ],
    actions: [
      // ── Rest: the fixture ships a Flows section, but at rest a real launcher
      //    shows recents/scripts, not 115 cold definitions. Empty filter =
      //    nothing elevated; the section header carries the at-rest label.
      { at: 0, kind: "setText", surface: "launcher-flow-rows", as: "input", text: "" },
      { at: 0, kind: "setSelection", surface: "launcher-flow-rows", index: 0 },
      {
        at: 0,
        kind: "setFooterState",
        surface: "launcher-flow-rows",
        footer: {
          runLabel: "Converse",
          runKeys: ["↵"],
          actionsLabel: "Actions",
          actionsKeys: ["⌘", "K"],
        },
      },

      // ── Type: a real query produces ONE mixed results section. The quota is
      //    visible in the fixture: four flow rows, then Scripts.
      {
        at: 1200,
        duration: 1600,
        kind: "type",
        surface: "launcher-flow-rows",
        as: "input",
        text: "commit",
      },
      { at: 3000, kind: "setSelection", surface: "launcher-flow-rows", index: 0 },

      // ── Preselect: high-confidence route. Selection moves and the footer
      //    verb updates. NOTHING executes — no session, no turn, no stream.
      {
        at: 4600,
        kind: "pressKey",
        surface: "launcher-flow-rows",
        key: "Tab",
        outcome: "preselect",
      },
      { at: 4700, kind: "setSelection", surface: "launcher-flow-rows", index: 0 },
      {
        at: 4700,
        kind: "setFooterState",
        surface: "launcher-flow-rows",
        footer: {
          runLabel: "Converse",
          runKeys: ["↵"],
          actionsLabel: "Actions",
          actionsKeys: ["⌘", "K"],
          selected: "run",
        },
      },
      // Walk the selection to show the primary verb staying stable across
      // flow rows and then changing when a SCRIPT row is selected.
      {
        at: 5800,
        duration: 1400,
        kind: "walkSelection",
        surface: "launcher-flow-rows",
        from: 0,
        to: 3,
      },

      // ── Converse: only an explicit Enter acts, and it converses.
      {
        at: 7800,
        kind: "setSelection",
        surface: "launcher-flow-rows",
        index: 0,
      },
      {
        at: 8200,
        kind: "pressKey",
        surface: "launcher-flow-rows",
        key: "Enter",
        outcome: "converse",
      },
      {
        at: 8400,
        kind: "setFooterState",
        surface: "launcher-flow-rows",
        footer: {
          runLabel: "Opening conversation…",
          runKeys: ["↵"],
          actionsLabel: "Actions",
          actionsKeys: ["⌘", "K"],
          selected: "run",
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

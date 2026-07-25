import { describe, expect, test } from "bun:test";

import {
  AGENT_CHAT_ANSWER_SCOPE_PREFIX,
  AGENT_CHAT_ANSWER_SCOPE_SUFFIX,
  FLOW_ANSWER_SCOPE_PREFIX,
  FLOW_ANSWER_SCOPE_SUFFIX,
  clipboardCopyPasses,
  evaluateClipboardCopy,
  evaluateSelectable,
  jumpPillBehaviour,
  selectionParity,
} from "./ai-chat-parity-evidence";

const flowNode = (selectable: unknown) => ({
  id: "chat-prompt/flow-7/turn/u:msg-1/assistant/document",
  metadata: { selectable },
});

const agentChatNode = (selectable: unknown) => ({
  id: "agent-chat-transcript-row-assistant-2/text/document",
  metadata: { selectable },
});

describe("selectable evidence", () => {
  test("a selectable answer region passes", () => {
    expect(
      evaluateSelectable([flowNode(true)], FLOW_ANSWER_SCOPE_PREFIX, FLOW_ANSWER_SCOPE_SUFFIX),
    ).toEqual({ kind: "selectable", scopeId: "chat-prompt/flow-7/turn/u:msg-1/assistant/document" });
  });

  test("selectable:false is a product regression, not missing evidence", () => {
    expect(
      evaluateSelectable([flowNode(false)], FLOW_ANSWER_SCOPE_PREFIX, FLOW_ANSWER_SCOPE_SUFFIX).kind,
    ).toBe("notSelectable");
  });

  // The distinction that matters: a broken proof channel must not be
  // reportable as a product bug, or someone "fixes" the product and the
  // channel stays broken.
  test("an un-annotated scope reports missing metadata, not not-selectable", () => {
    expect(
      evaluateSelectable(
        [{ id: "chat-prompt/flow-7/turn/u:msg-1/assistant/document" }],
        FLOW_ANSWER_SCOPE_PREFIX,
        FLOW_ANSWER_SCOPE_SUFFIX,
      ).kind,
    ).toBe("missingMetadata");
  });

  // The failure mode this whole file exists to prevent: a probe that drove
  // nothing reporting no failures.
  test("no answer region at all is absent, never a pass", () => {
    const verdict = evaluateSelectable([], FLOW_ANSWER_SCOPE_PREFIX, FLOW_ANSWER_SCOPE_SUFFIX);
    expect(verdict.kind).toBe("absent");
    expect(
      evaluateSelectable(null, FLOW_ANSWER_SCOPE_PREFIX, FLOW_ANSWER_SCOPE_SUFFIX).kind,
    ).toBe("absent");
    expect(
      evaluateSelectable(undefined, FLOW_ANSWER_SCOPE_PREFIX, FLOW_ANSWER_SCOPE_SUFFIX).kind,
    ).toBe("absent");
  });

  test("one unselectable region among several fails the surface", () => {
    const verdict = evaluateSelectable(
      [
        flowNode(true),
        { id: "chat-prompt/flow-7/turn/u:msg-2/assistant/document", metadata: { selectable: false } },
      ],
      FLOW_ANSWER_SCOPE_PREFIX,
      FLOW_ANSWER_SCOPE_SUFFIX,
    );
    expect(verdict.kind).toBe("notSelectable");
  });

  test("scope matching does not confuse the two surfaces", () => {
    const mixed = [flowNode(true), agentChatNode(false)];
    expect(
      evaluateSelectable(mixed, FLOW_ANSWER_SCOPE_PREFIX, FLOW_ANSWER_SCOPE_SUFFIX).kind,
    ).toBe("selectable");
    expect(
      evaluateSelectable(
        mixed,
        AGENT_CHAT_ANSWER_SCOPE_PREFIX,
        AGENT_CHAT_ANSWER_SCOPE_SUFFIX,
      ).kind,
    ).toBe("notSelectable");
  });
});

describe("selection parity", () => {
  test("both selectable passes", () => {
    const parity = selectionParity(
      { kind: "selectable", scopeId: "flow" },
      { kind: "selectable", scopeId: "agent" },
    );
    expect(parity.pass).toBe(true);
  });

  // Two absent surfaces "agree" in the useless sense. That must never read as
  // parity — it is the shape of a probe that failed to drive anything.
  test("two missing surfaces do not agree into a pass", () => {
    const parity = selectionParity(
      { kind: "absent", scopePattern: "a" },
      { kind: "absent", scopePattern: "b" },
    );
    expect(parity.pass).toBe(false);
    expect(parity.reason).toContain("no evidence");
  });

  test("one selectable surface is not parity", () => {
    expect(
      selectionParity(
        { kind: "selectable", scopeId: "flow" },
        { kind: "notSelectable", scopeId: "agent" },
      ).pass,
    ).toBe(false);
  });
});

describe("clipboard copy evidence", () => {
  const SENTINEL = "sentinel-before-the-chord";

  test("a real answer on the clipboard passes", () => {
    const verdict = evaluateClipboardCopy(SENTINEL, "Here is the answer.", []);
    expect(verdict).toEqual({ kind: "copied", text: "Here is the answer." });
    expect(clipboardCopyPasses(verdict)).toBe(true);
  });

  // The exact shape of an unbound chord: nothing happened, and without the
  // sentinel "the clipboard has text in it" would have passed anyway.
  test("an untouched clipboard is unchanged, not a pass", () => {
    const verdict = evaluateClipboardCopy(SENTINEL, SENTINEL, []);
    expect(verdict.kind).toBe("unchanged");
    expect(clipboardCopyPasses(verdict)).toBe(false);
  });

  // Writing "" to the clipboard SUCCEEDS, which is why the empty in-flight
  // turn was dangerous in the first place.
  test("a successful copy of nothing is not a pass", () => {
    expect(clipboardCopyPasses(evaluateClipboardCopy(SENTINEL, "", []))).toBe(false);
    expect(clipboardCopyPasses(evaluateClipboardCopy(SENTINEL, "   \n ", []))).toBe(false);
  });

  test("copying the user's own message is a distinct failure from copying nothing", () => {
    const verdict = evaluateClipboardCopy(SENTINEL, "Say something quotable.", [
      "Say something quotable.",
    ]);
    expect(verdict.kind).toBe("wrongText");
    expect(clipboardCopyPasses(verdict)).toBe(false);
  });

  test("forbidden matching ignores surrounding whitespace", () => {
    expect(
      evaluateClipboardCopy(SENTINEL, "  the draft  ", ["the draft"]).kind,
    ).toBe("wrongText");
  });
});

describe("jump-to-latest behaviour", () => {
  const pill = "agent-chat-jump-to-latest";

  test("hidden at the tail and shown after scrolling up", () => {
    expect(jumpPillBehaviour([], [{ name: pill }], pill).pass).toBe(true);
  });

  // The likelier bug than "never appears" is "always on screen".
  test("an always-visible pill fails", () => {
    expect(jumpPillBehaviour([{ name: pill }], [{ name: pill }], pill).pass).toBe(false);
  });

  test("a pill that never appears fails", () => {
    expect(jumpPillBehaviour([], [], pill).pass).toBe(false);
  });
});

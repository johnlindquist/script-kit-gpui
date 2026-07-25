/**
 * Pure evaluation of AI chat parity evidence.
 *
 * The probe that drives the app lives in `ai-chat-parity-probe.ts`. Every
 * judgement it makes lives HERE, as functions over plain snapshots, for one
 * reason: a probe whose assertions are tangled up with its driving can only be
 * checked by running the whole app, and an app run that cannot start reports
 * the same "no failures" as an app run where everything passed.
 *
 * These functions are total over their inputs and never throw. Absent evidence
 * is a distinct, named outcome — never silently `true`.
 */

export type FidelityNode = {
  id?: string;
  metadata?: Record<string, unknown> | null;
};

export type LayoutComponent = {
  name?: string;
};

/** A selection claim about one rendered answer region. */
export type SelectableVerdict =
  | { kind: "selectable"; scopeId: string }
  | { kind: "notSelectable"; scopeId: string }
  | { kind: "missingMetadata"; scopeId: string }
  /** No answer region was painted at all — the probe drove nothing. */
  | { kind: "absent"; scopePattern: string };

/**
 * Scope id of a Flow (ChatPrompt) assistant answer region.
 *
 * Must stay in sync with `ChatPrompt::render_answer_region`, which builds
 * `chat-prompt/{prompt_id}/turn/{render_key}/assistant`.
 */
export const FLOW_ANSWER_SCOPE_PREFIX = "chat-prompt/";
/**
 * The vendored `TextView` appends `/document` to whatever `fidelity_scope` it
 * was given, so the painted id is
 * `chat-prompt/{prompt}/turn/{key}/assistant/document` — NOT the
 * `.../assistant` string the renderer passes in. Matching the renderer's
 * string instead of the painted one yields `absent` for a perfectly healthy
 * surface, which reads exactly like the regression this probe hunts.
 */
export const FLOW_ANSWER_SCOPE_SUFFIX = "/assistant/document";

/** Scope id of an Agent Chat assistant answer region. Same `/document` rule. */
export const AGENT_CHAT_ANSWER_SCOPE_PREFIX = "agent-chat-transcript-row-assistant";
export const AGENT_CHAT_ANSWER_SCOPE_SUFFIX = "/text/document";

export function findScopes(
  nodes: FidelityNode[] | null | undefined,
  prefix: string,
  suffix: string,
): FidelityNode[] {
  if (!Array.isArray(nodes)) return [];
  return nodes.filter((node) => {
    const id = node?.id;
    return typeof id === "string" && id.startsWith(prefix) && id.endsWith(suffix);
  });
}

/**
 * Decide whether an answer region reported itself selectable.
 *
 * `missingMetadata` is deliberately distinct from `notSelectable`: the first
 * means the vendored `TextView` never annotated its scope (a broken proof
 * channel), the second means it annotated `selectable: false` (a real product
 * regression). Collapsing them would let a silent instrumentation break read
 * as a product bug, or worse, get "fixed" by weakening the assertion.
 */
export function evaluateSelectable(
  nodes: FidelityNode[] | null | undefined,
  prefix: string,
  suffix: string,
): SelectableVerdict {
  const matches = findScopes(nodes, prefix, suffix);
  if (matches.length === 0) {
    return { kind: "absent", scopePattern: `${prefix}*${suffix}` };
  }
  // Any non-selectable region fails the surface: one unselectable answer is
  // exactly the defect, even if its neighbours are fine.
  for (const node of matches) {
    const scopeId = String(node.id);
    const metadata = node.metadata;
    if (!metadata || !("selectable" in metadata)) {
      return { kind: "missingMetadata", scopeId };
    }
    if (metadata.selectable !== true) {
      return { kind: "notSelectable", scopeId };
    }
  }
  return { kind: "selectable", scopeId: String(matches[0].id) };
}

export function verdictPasses(verdict: SelectableVerdict): boolean {
  return verdict.kind === "selectable";
}

/**
 * Both chat surfaces must agree. A parity claim is only meaningful when BOTH
 * sides produced evidence, so a missing surface fails rather than matching
 * another missing surface.
 */
export function selectionParity(
  flow: SelectableVerdict,
  agentChat: SelectableVerdict,
): { pass: boolean; reason: string } {
  if (flow.kind === "absent" || agentChat.kind === "absent") {
    return {
      pass: false,
      reason: `no evidence: flow=${flow.kind} agentChat=${agentChat.kind}`,
    };
  }
  if (verdictPasses(flow) && verdictPasses(agentChat)) {
    return { pass: true, reason: "both surfaces report selectable answer text" };
  }
  return {
    pass: false,
    reason: `flow=${flow.kind} agentChat=${agentChat.kind}`,
  };
}

/** What a `⇧⌘C` press did to the real system clipboard. */
export type ClipboardCopyVerdict =
  /** Something new and plausible landed. */
  | { kind: "copied"; text: string }
  /** The clipboard still holds the sentinel — the chord did nothing at all. */
  | { kind: "unchanged"; sentinel: string }
  /** The app wrote, but wrote blank. A successful copy of nothing. */
  | { kind: "empty" }
  /** The app copied the wrong side of the turn (or the composer draft). */
  | { kind: "wrongText"; text: string; matched: string };

/**
 * Judge a clipboard copy against a sentinel planted before the keypress.
 *
 * The sentinel is what makes this falsifiable. Without it, "the clipboard
 * contains text" passes for a clipboard nobody touched — which is exactly the
 * state an unbound chord leaves behind, and exactly the bug this probe exists
 * to catch.
 *
 * `forbidden` carries strings that would mean the RIGHT chord copied the WRONG
 * thing — the user's own message, or an unsent draft. Those must not read as a
 * pass just because the clipboard changed.
 */
export function evaluateClipboardCopy(
  sentinel: string,
  after: string,
  forbidden: string[] = [],
): ClipboardCopyVerdict {
  if (after === sentinel) return { kind: "unchanged", sentinel };
  if (after.trim() === "") return { kind: "empty" };
  const matched = forbidden.find((candidate) => candidate.trim() === after.trim());
  if (matched !== undefined) return { kind: "wrongText", text: after, matched };
  return { kind: "copied", text: after };
}

export function clipboardCopyPasses(verdict: ClipboardCopyVerdict): boolean {
  return verdict.kind === "copied";
}

export function componentPresent(
  components: LayoutComponent[] | null | undefined,
  name: string,
): boolean {
  if (!Array.isArray(components)) return false;
  return components.some((component) => component?.name === name);
}

/**
 * The jump-to-latest pill must be absent while following the tail and present
 * after the user scrolls up. Asserting only the "present" half would pass for
 * a pill that is ALWAYS on screen, which is the more likely bug.
 */
export function jumpPillBehaviour(
  followingTail: LayoutComponent[] | null | undefined,
  afterManualScroll: LayoutComponent[] | null | undefined,
  pillName: string,
): { pass: boolean; whileFollowing: boolean; afterScroll: boolean } {
  const whileFollowing = componentPresent(followingTail, pillName);
  const afterScroll = componentPresent(afterManualScroll, pillName);
  return { pass: !whileFollowing && afterScroll, whileFollowing, afterScroll };
}

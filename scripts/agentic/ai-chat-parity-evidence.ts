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
export const FLOW_ANSWER_SCOPE_SUFFIX = "/assistant";
export const FLOW_ANSWER_SCOPE_PREFIX = "chat-prompt/";

/** Scope id of an Agent Chat assistant answer region. */
export const AGENT_CHAT_ANSWER_SCOPE_SUFFIX = "/text";
export const AGENT_CHAT_ANSWER_SCOPE_PREFIX = "agent-chat-transcript-row-assistant";

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

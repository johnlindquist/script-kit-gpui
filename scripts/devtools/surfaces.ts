#!/usr/bin/env bun
/**
 * scripts/devtools/surfaces.ts — source-backed surface inventory and the
 * PF-009 coverage binding generator.
 *
 * Source authority is two independent layers that must agree exactly:
 *
 * 1. Rust census authority — a delimiter-aware extraction of the
 *    `SurfaceKind` enum plus the exhaustive `AppView::app_view_variant()` and
 *    `AppView::surface_kind()` matches in
 *    `src/main_sections/app_view_state.rs`. Wildcard arms, unknown tokens,
 *    and unmapped variants are hard errors, never approximations.
 * 2. Generated contract detail authority — `docs/ai/contracts/
 *    surface-contracts.json` supplies family/focus/keyboard/actions/proof/
 *    visual/dismiss detail and must have exact kind+mapping parity with the
 *    Rust extraction.
 *
 * The expected census is 37 kinds / 54 mappings (FileSearchView contributes
 * two mappings). Any other census, a parity failure, or a missing real
 * feature map produces BLOCKED_SCOPE_DRIFT — never a trimmed or padded
 * binding list.
 *
 * CLI:
 *   bun scripts/devtools/surfaces.ts                     # inventory report
 *   bun scripts/devtools/surfaces.ts --markdown
 *   bun scripts/devtools/surfaces.ts bindings [--out p]  # PF-009 receipt
 *   bun scripts/devtools/surfaces.ts validate-bindings --input p
 *   bun scripts/devtools/surfaces.ts verify-negative-controls [--out-dir d]
 */

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import {
  coverageProfiles,
  validateCoverageProfiles,
  type CoverageProfile,
  type CoverageStatus,
} from "./coverage.ts";
import {
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseTaskCatalog,
} from "./consistency.ts";
import {
  emitValidatedReceipt,
  receiptSchemaRegistry,
  validateReceiptFile,
  RECEIPT_SCHEMA_VERSION,
} from "./lib/receipt-schema.ts";
import {
  buildRuntimeCoverageScorecard,
  discoverRuntimeCoverageReceipts,
} from "./lib/runtime-coverage.ts";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SurfaceContract = {
  surfaceKind: string;
  appViewVariants: string[];
  appViewFooters: Array<{ variant: string; nativeFooterSurface: string | null }>;
  vocabulary?: {
    family?: string;
    inputOwnership?: string;
    previewRole?: string;
  };
  focusPolicy?: string;
  keyboardPolicy?: string;
  actionsPolicy?: string;
  proofPolicy?: string;
  visualPolicy?: string;
  dismissPolicy?: {
    policy: string;
    windowBlur: string;
    backdropClick: string;
    escape: string;
    cmdW: string;
  };
  automationSemanticSurface?: string;
};

export type SurfaceContractRegistry = {
  schemaVersion: number;
  generatedFrom: string;
  registry: string;
  entries: SurfaceContract[];
  /** sha256 of the generated JSON file. */
  contractSha256: string;
  /** sha256 of the resolved Rust source file named by generatedFrom. */
  rustSha256: string | null;
  rustSourceExists: boolean;
};

export type RustSurfaceMapping = {
  contractKind: string;
  appViewVariant: string;
  discriminator: string | null;
};

export type RustSurfaceCensus = {
  kinds: string[];
  mappings: RustSurfaceMapping[];
  variantNames: string[];
  errors: string[];
};

export type ProjectionParity = {
  pass: boolean;
  missingMappingKeys: string[];
  unexpectedMappingKeys: string[];
  missingKinds: string[];
  unexpectedKinds: string[];
};

export type CanonicalSurfaceMapping = {
  contractKind: string;
  appViewVariant: string;
  discriminator: string | null;
  contract: SurfaceContract;
};

export type EvidenceGrade =
  | "Direct"
  | "Derived"
  | "Alias"
  | "Partial"
  | "Unsupported";

export type CoverageBinding = {
  contractKind: string;
  appViewVariant: string;
  hostKind: string;
  profileId: string;
  evidenceGrade: EvidenceGrade;
  fixtureFamily: string;
  expectedTargetIdentity: {
    windowKind: string;
    surfaceKind: string;
    hostKind: string | null;
    parentRequired: boolean;
  };
  requiredPrimitiveIds: string[];
  missingPrimitiveIds: string[];
};

export type CoverageBindingRecord = CoverageBinding & {
  bindingId: string;
  profileStatus: CoverageStatus;
  relation: "Direct" | "Derived";
  supported: boolean;
  sourceContractFingerprint: string;
};

export type CoverageAliasBinding = Omit<
  CoverageBindingRecord,
  "evidenceGrade" | "relation" | "supported"
> & {
  alias: string;
  evidenceGrade: "Alias";
  countsAsCoverage: false;
  supported: false;
  canonicalBindingId: string;
};

export type CoverageBindingSet = {
  bindings: CoverageBindingRecord[];
  aliases: CoverageAliasBinding[];
  /** Every canonical mapping key the source census demands. */
  sourceMappingKeys: string[];
  fingerprint: string;
};

export type CoverageBindingValidation = {
  pass: boolean;
  disposition: "EVALUABLE_PASS" | "INVALID_SCHEMA";
  errors: string[];
  duplicateTupleCount: number;
  missingBindingCount: number;
  invalidProfileReferenceCount: number;
  invalidPrimitiveReferenceCount: number;
  supportInvariantViolationCount: number;
};

type FeatureMapEntry = {
  id: string;
  feature: string;
  cluster: string;
  primaryOwners: string[];
  rawOracle: string;
  chapter: string;
};

type OracleBatch = {
  id: string;
  name: string;
  priority: number;
  outsideInPhase: "window-container" | "surface-shell" | "content-controls" | "supporting-systems";
  priorityRationale: string;
  owners: string[];
  surfaceKinds: string[];
  featureIds: string[];
  requiredDevToolsPrimitives: string[];
  questionsForOracle: string[];
};

// ---------------------------------------------------------------------------
// Constants and paths
// ---------------------------------------------------------------------------

const root = new URL("../..", import.meta.url);

export const paths = {
  contracts: "docs/ai/contracts/surface-contracts.json",
  rustSource: "src/main_sections/app_view_state.rs",
  featureMap: "FEATURE_MAP.md",
  maintainedFeatureAtlas: "feature-map/index.md",
  coverage: "scripts/devtools/coverage.ts",
  surfacesModule: "scripts/devtools/surfaces.ts",
};

export const EXPECTED_CENSUS = {
  contractKindCount: 37,
  contractMappingCount: 54,
} as const;

/**
 * Explicit non-counting orientation aliases. An alias exists ONLY here —
 * name normalization, semantic-surface equivalence, or profile-id matching
 * never creates one. `confirm-prompt-popup` records the live popup-confirm
 * orientation (crate::confirm) while the canonical ConfirmPrompt mapping
 * stays the in-window Main AppView.
 */
export const coverageSurfaceAliases = [
  { alias: "main", resolvesTo: { surfaceKind: "ScriptList" }, countsAsCoverage: false },
  { alias: "actions-dialog", resolvesTo: { surfaceKind: "ActionsDialog" }, countsAsCoverage: false },
  { alias: "dictation-history", resolvesTo: { surfaceKind: "AttachmentPortalBrowser", appViewVariant: "DictationHistoryView" }, countsAsCoverage: false },
  { alias: "notes-agent_chat", resolvesTo: { surfaceKind: "AgentChat", hostKind: "NotesWindow" }, countsAsCoverage: false },
  { alias: "confirm-prompt-popup", resolvesTo: { surfaceKind: "ConfirmPrompt", hostKind: "PromptPopup" }, countsAsCoverage: false },
] as const;

export const liquidGlassAuditExclusions: readonly { surfaceKind: string; reason: string }[] = [];

const sourceFamilyToFixtureFamily = {
  MainMenu: "main-menu",
  FilterableLauncherList: "filterable-launcher-list",
  ScriptPrompt: "script-prompt",
  UtilityWorkspace: "utility-workspace",
  AttachmentPortal: "attachment-portal",
  AssistantWorkspace: "assistant-workspace",
  FeedbackSurface: "feedback-surface",
} as const;

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function readText(path: string) {
  return Bun.file(new URL(path, root)).text();
}

async function readTextIfExists(path: string): Promise<string | null> {
  const file = Bun.file(new URL(path, root));
  return (await file.exists()) ? file.text() : null;
}

export function mappingKey(contractKind: string, appViewVariant: string): string {
  return `${contractKind}::${appViewVariant}`;
}

export function bindingTuple(binding: {
  contractKind: string;
  appViewVariant: string;
  hostKind: string;
}): string {
  return JSON.stringify([
    binding.contractKind,
    binding.appViewVariant,
    binding.hostKind,
  ]);
}

export function bindingIdFor(binding: {
  contractKind: string;
  appViewVariant: string;
  hostKind: string;
}): string {
  return `${binding.contractKind}::${binding.appViewVariant}@${binding.hostKind}`;
}

// ---------------------------------------------------------------------------
// Rust source census extraction (delimiter-aware; no occurrence counting)
// ---------------------------------------------------------------------------

/**
 * Remove line and block comments while preserving string/char literal
 * contents. This keeps `=> "Name"` intact but drops doc comments that could
 * otherwise contain code-shaped text such as `_ => ...`.
 */
export function stripRustComments(source: string): string {
  let out = "";
  let index = 0;
  const length = source.length;
  while (index < length) {
    const char = source[index];
    const next = source[index + 1];
    if (char === "/" && next === "/") {
      while (index < length && source[index] !== "\n") index += 1;
      continue;
    }
    if (char === "/" && next === "*") {
      index += 2;
      let depth = 1;
      while (index < length && depth > 0) {
        if (source[index] === "/" && source[index + 1] === "*") {
          depth += 1;
          index += 2;
        } else if (source[index] === "*" && source[index + 1] === "/") {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      continue;
    }
    if (char === '"') {
      out += char;
      index += 1;
      while (index < length) {
        const inner = source[index];
        out += inner;
        if (inner === "\\") {
          out += source[index + 1] ?? "";
          index += 2;
          continue;
        }
        index += 1;
        if (inner === '"') break;
      }
      continue;
    }
    out += char;
    index += 1;
  }
  return out;
}

/** Find the `{...}` body that starts at/after `anchor`; returns inner text. */
function braceBody(source: string, anchor: string): string | null {
  const anchorIndex = source.indexOf(anchor);
  if (anchorIndex < 0) return null;
  const open = source.indexOf("{", anchorIndex);
  if (open < 0) return null;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    const char = source[index];
    if (char === "{") depth += 1;
    else if (char === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open + 1, index);
    }
  }
  return null;
}

type MatchArm = { pattern: string; body: string };

/** Split a match body into arms structurally (handles block-bodied arms). */
function splitMatchArms(matchBody: string): { arms: MatchArm[]; errors: string[] } {
  const arms: MatchArm[] = [];
  const errors: string[] = [];
  let index = 0;
  const length = matchBody.length;

  const skipWhitespace = () => {
    while (index < length && /\s/.test(matchBody[index])) index += 1;
  };

  while (index < length) {
    skipWhitespace();
    if (index >= length) break;
    // Pattern: read until a top-level "=>".
    let depth = 0;
    const patternStart = index;
    let arrowIndex = -1;
    while (index < length) {
      const char = matchBody[index];
      if (char === "{" || char === "(" || char === "[") depth += 1;
      else if (char === "}" || char === ")" || char === "]") depth -= 1;
      else if (depth === 0 && char === "=" && matchBody[index + 1] === ">") {
        arrowIndex = index;
        break;
      }
      index += 1;
    }
    if (arrowIndex < 0) {
      const trailing = matchBody.slice(patternStart).trim();
      if (trailing.length > 0) errors.push(`unparseable match arm tail: ${trailing.slice(0, 60)}`);
      break;
    }
    const pattern = matchBody.slice(patternStart, arrowIndex).trim();
    index = arrowIndex + 2;
    skipWhitespace();
    // Body: either a block or an expression ending at a top-level comma.
    let body = "";
    if (matchBody[index] === "{") {
      let bodyDepth = 0;
      const bodyStart = index;
      while (index < length) {
        const char = matchBody[index];
        if (char === "{") bodyDepth += 1;
        else if (char === "}") {
          bodyDepth -= 1;
          if (bodyDepth === 0) {
            index += 1;
            break;
          }
        }
        index += 1;
      }
      body = matchBody.slice(bodyStart, index);
      skipWhitespace();
      if (matchBody[index] === ",") index += 1;
    } else {
      let bodyDepth = 0;
      const bodyStart = index;
      while (index < length) {
        const char = matchBody[index];
        if (char === "{" || char === "(" || char === "[") bodyDepth += 1;
        else if (char === "}" || char === ")" || char === "]") bodyDepth -= 1;
        else if (bodyDepth === 0 && char === ",") break;
        index += 1;
      }
      body = matchBody.slice(bodyStart, index);
      if (matchBody[index] === ",") index += 1;
    }
    arms.push({ pattern, body: body.trim() });
  }
  return { arms, errors };
}

export function extractRustSurfaceCensus(source: string): RustSurfaceCensus {
  const errors: string[] = [];
  const stripped = stripRustComments(source);

  // 1. SurfaceKind enum variants.
  const enumBody = braceBody(stripped, "enum SurfaceKind");
  const kinds: string[] = [];
  if (enumBody === null) {
    errors.push("enum SurfaceKind not found");
  } else {
    for (const rawLine of enumBody.split(",")) {
      const token = rawLine.trim();
      if (token.length === 0) continue;
      // Skip attributes attached to the next variant.
      const identifier = token.replace(/#\[[^\]]*\]/g, "").trim();
      if (identifier.length === 0) continue;
      if (!/^[A-Z][A-Za-z0-9_]*$/.test(identifier)) {
        errors.push(`unrecognized SurfaceKind enum entry: ${identifier.slice(0, 60)}`);
        continue;
      }
      kinds.push(identifier);
    }
  }
  const kindSet = new Set(kinds);
  if (kindSet.size !== kinds.length) errors.push("duplicate SurfaceKind enum variants");

  // 2. app_view_variant(): AppView pattern -> stable string name.
  const variantFnBody = braceBody(stripped, "fn app_view_variant");
  const variantNames: string[] = [];
  const appViewVariants = new Set<string>();
  if (variantFnBody === null) {
    errors.push("fn app_view_variant not found");
  } else {
    const matchBody = braceBody(variantFnBody, "match self");
    if (matchBody === null) {
      errors.push("app_view_variant match body not found");
    } else {
      const { arms, errors: armErrors } = splitMatchArms(matchBody);
      errors.push(...armErrors);
      for (const arm of arms) {
        if (/^_$/.test(arm.pattern) || /(^|\|)\s*_\s*($|\|)/.test(arm.pattern)) {
          errors.push("app_view_variant contains a wildcard arm");
          continue;
        }
        const patternVariants = [...arm.pattern.matchAll(/AppView::([A-Za-z0-9_]+)/g)].map((m) => m[1]);
        const name = arm.body.match(/^"([^"]+)"$/);
        if (patternVariants.length !== 1 || !name) {
          errors.push(`unrecognized app_view_variant arm: ${arm.pattern.slice(0, 60)}`);
          continue;
        }
        if (patternVariants[0] !== name[1]) {
          errors.push(`app_view_variant name mismatch: ${patternVariants[0]} -> ${name[1]}`);
        }
        appViewVariants.add(patternVariants[0]);
        variantNames.push(name[1]);
      }
    }
  }
  if (new Set(variantNames).size !== variantNames.length) {
    errors.push("duplicate app_view_variant names");
  }

  // 3. surface_kind(): AppView patterns (possibly or-patterns and
  //    discriminated FileSearch presentations) -> SurfaceKind target.
  const kindFnBody = braceBody(stripped, "fn surface_kind");
  const mappings: RustSurfaceMapping[] = [];
  if (kindFnBody === null) {
    errors.push("fn surface_kind not found");
  } else {
    const matchBody = braceBody(kindFnBody, "match self");
    if (matchBody === null) {
      errors.push("surface_kind match body not found");
    } else {
      const { arms, errors: armErrors } = splitMatchArms(matchBody);
      errors.push(...armErrors);
      for (const arm of arms) {
        if (/^_$/.test(arm.pattern) || /(^|\|)\s*_\s*($|\|)/.test(arm.pattern)) {
          errors.push("surface_kind contains a wildcard arm");
          continue;
        }
        const targets = [...arm.body.matchAll(/SurfaceKind::([A-Za-z0-9_]+)/g)].map((m) => m[1]);
        if (targets.length !== 1) {
          errors.push(`surface_kind arm lacks exactly one SurfaceKind target: ${arm.pattern.slice(0, 60)}`);
          continue;
        }
        const target = targets[0];
        if (!kindSet.has(target)) {
          errors.push(`surface_kind targets unknown SurfaceKind::${target}`);
        }
        const patternVariants = [...arm.pattern.matchAll(/AppView::([A-Za-z0-9_]+)/g)].map((m) => m[1]);
        if (patternVariants.length === 0) {
          errors.push(`surface_kind arm has no AppView pattern: ${arm.pattern.slice(0, 60)}`);
          continue;
        }
        const discriminators = [...arm.pattern.matchAll(/FileSearchPresentation::([A-Za-z0-9_]+)/g)].map((m) => m[1]);
        const discriminator = discriminators.length === 1 ? discriminators[0] : null;
        if (discriminators.length > 1) {
          errors.push(`surface_kind arm has multiple discriminators: ${arm.pattern.slice(0, 60)}`);
        }
        for (const variant of patternVariants) {
          if (!appViewVariants.has(variant)) {
            errors.push(`surface_kind maps unknown AppView::${variant}`);
          }
          mappings.push({ contractKind: target, appViewVariant: variant, discriminator });
        }
      }
    }
  }

  // Cross-checks: every declared variant is mapped; plain variants once,
  // discriminated variants once per distinct discriminator.
  const byVariant = new Map<string, RustSurfaceMapping[]>();
  for (const mapping of mappings) {
    const list = byVariant.get(mapping.appViewVariant) ?? [];
    list.push(mapping);
    byVariant.set(mapping.appViewVariant, list);
  }
  for (const variant of appViewVariants) {
    const list = byVariant.get(variant) ?? [];
    if (list.length === 0) {
      errors.push(`AppView::${variant} has no surface_kind mapping`);
    } else if (list.length > 1) {
      const discriminators = new Set(list.map((entry) => entry.discriminator ?? ""));
      if (discriminators.size !== list.length || discriminators.has("")) {
        errors.push(`AppView::${variant} is mapped ${list.length} times without distinct discriminators`);
      }
    }
  }
  const usedKinds = new Set(mappings.map((mapping) => mapping.contractKind));
  for (const kind of kinds) {
    if (!usedKinds.has(kind)) errors.push(`SurfaceKind::${kind} is never produced by surface_kind`);
  }

  return {
    kinds,
    mappings,
    variantNames: [...appViewVariants].sort(),
    errors,
  };
}

// ---------------------------------------------------------------------------
// Generated contract registry + parity
// ---------------------------------------------------------------------------

export async function loadSurfaceContractRegistry(): Promise<SurfaceContractRegistry> {
  const contractText = await readText(paths.contracts);
  const parsed = JSON.parse(contractText) as {
    schemaVersion: number;
    generatedFrom: string;
    registry: string;
    entries: SurfaceContract[];
  };
  const rustText = parsed.generatedFrom ? await readTextIfExists(parsed.generatedFrom) : null;
  return {
    ...parsed,
    contractSha256: sha256(contractText),
    rustSha256: rustText === null ? null : sha256(rustText),
    rustSourceExists: rustText !== null,
  };
}

export function compareContractProjection(
  rust: RustSurfaceCensus,
  generated: Pick<SurfaceContractRegistry, "entries">,
): ProjectionParity {
  const rustKeys = new Set(rust.mappings.map((m) => mappingKey(m.contractKind, m.appViewVariant)));
  const generatedKeys = new Set(
    generated.entries.flatMap((entry) =>
      entry.appViewVariants.map((variant) => mappingKey(entry.surfaceKind, variant))
    ),
  );
  const rustKinds = new Set(rust.kinds);
  const generatedKinds = new Set(generated.entries.map((entry) => entry.surfaceKind));
  const missingMappingKeys = [...rustKeys].filter((key) => !generatedKeys.has(key)).sort();
  const unexpectedMappingKeys = [...generatedKeys].filter((key) => !rustKeys.has(key)).sort();
  const missingKinds = [...rustKinds].filter((kind) => !generatedKinds.has(kind)).sort();
  const unexpectedKinds = [...generatedKinds].filter((kind) => !rustKinds.has(kind)).sort();
  return {
    pass:
      rust.errors.length === 0 &&
      missingMappingKeys.length === 0 &&
      unexpectedMappingKeys.length === 0 &&
      missingKinds.length === 0 &&
      unexpectedKinds.length === 0,
    missingMappingKeys,
    unexpectedMappingKeys,
    missingKinds,
    unexpectedKinds,
  };
}

export function buildCanonicalMappings(
  registry: Pick<SurfaceContractRegistry, "entries">,
  rust?: RustSurfaceCensus,
): CanonicalSurfaceMapping[] {
  const discriminators = new Map<string, string | null>();
  if (rust) {
    for (const mapping of rust.mappings) {
      discriminators.set(mappingKey(mapping.contractKind, mapping.appViewVariant), mapping.discriminator);
    }
  }
  return registry.entries.flatMap((entry) =>
    entry.appViewVariants.map((variant) => ({
      contractKind: entry.surfaceKind,
      appViewVariant: variant,
      discriminator: discriminators.get(mappingKey(entry.surfaceKind, variant)) ?? null,
      contract: entry,
    })),
  );
}

// ---------------------------------------------------------------------------
// Feature map gate
// ---------------------------------------------------------------------------

function parseOwners(raw: string) {
  return [...raw.matchAll(/`([^`]+)`/g)].map((match) => match[1]);
}

function parseLink(raw: string) {
  const match = raw.match(/\]\(([^)]+)\)/);
  return match?.[1] ?? "";
}

export function parseFeatureMap(markdown: string): FeatureMapEntry[] {
  return markdown
    .split("\n")
    .filter((line) => /^\|\s*\d{3}\s*\|/.test(line))
    .map((line) => {
      const cells = line
        .slice(1, -1)
        .split("|")
        .map((cell) => cell.trim());
      return {
        id: cells[0],
        feature: cells[1],
        cluster: cells[2],
        primaryOwners: parseOwners(cells[4] ?? ""),
        rawOracle: parseLink(cells[5] ?? ""),
        chapter: parseLink(cells[6] ?? ""),
      };
    });
}

export type FeatureMapGate = {
  pass: boolean;
  sourcePath: string;
  sourceSha256: string | null;
  parsedEntryCount: number;
  attemptedPaths: string[];
  status: string;
  entries: FeatureMapEntry[];
};

/**
 * Resolve the actual real feature map. `FEATURE_MAP.md` is only acceptable
 * when it parses to a nonzero entry table itself; a compatibility stub
 * pointing at a missing maintained atlas is a gate failure
 * (BLOCKED_SCOPE_DRIFT), never an empty successful report.
 */
export async function resolveFeatureMapGate(): Promise<FeatureMapGate> {
  const attemptedPaths: string[] = [];

  const atlasText = await readTextIfExists(paths.maintainedFeatureAtlas);
  attemptedPaths.push(paths.maintainedFeatureAtlas);
  if (atlasText !== null) {
    const entries = parseFeatureMap(atlasText);
    if (entries.length > 0) {
      return {
        pass: true,
        sourcePath: paths.maintainedFeatureAtlas,
        sourceSha256: sha256(atlasText),
        parsedEntryCount: entries.length,
        attemptedPaths,
        status: "maintained-atlas",
        entries,
      };
    }
  }

  const compatText = await readTextIfExists(paths.featureMap);
  attemptedPaths.push(paths.featureMap);
  if (compatText !== null) {
    const entries = parseFeatureMap(compatText);
    if (entries.length > 0) {
      return {
        pass: true,
        sourcePath: paths.featureMap,
        sourceSha256: sha256(compatText),
        parsedEntryCount: entries.length,
        attemptedPaths,
        status: "compatibility-index-with-entries",
        entries,
      };
    }
    return {
      pass: false,
      sourcePath: paths.featureMap,
      sourceSha256: sha256(compatText),
      parsedEntryCount: 0,
      attemptedPaths,
      status: atlasText === null
        ? "compatibility-index-points-to-missing-atlas"
        : "maintained-atlas-unparsed",
      entries: [],
    };
  }

  return {
    pass: false,
    sourcePath: paths.featureMap,
    sourceSha256: null,
    parsedEntryCount: 0,
    attemptedPaths,
    status: "feature-map-missing",
    entries: [],
  };
}

// ---------------------------------------------------------------------------
// PF-009 binding derivation
// ---------------------------------------------------------------------------

export function resolveContractHost(contractKind: string): string {
  // Runtime registration truth: the live Cmd+K Actions route is an attached
  // popup window; every other canonical AppView mapping renders in the Main
  // launcher window. Native secondary windows never inflate the census —
  // they are represented by explicit non-counting orientation aliases.
  return contractKind === "ActionsDialog" ? "ActionsDialog" : "MainWindow";
}

const hostToTargetIdentity: Record<string, { windowKind: string; hostKind: string; parentRequired: boolean }> = {
  MainWindow: { windowKind: "Main", hostKind: "mainWindow", parentRequired: false },
  ActionsDialog: { windowKind: "ActionsDialog", hostKind: "attachedPopup", parentRequired: true },
  PromptPopup: { windowKind: "PromptPopup", hostKind: "attachedPopup", parentRequired: true },
  NotesWindow: { windowKind: "Notes", hostKind: "detachedWindow", parentRequired: false },
};

export function expectedTargetIdentity(
  contractKind: string,
  hostKind: string,
): CoverageBinding["expectedTargetIdentity"] {
  const identity = hostToTargetIdentity[hostKind];
  if (!identity) throw new Error(`unknown binding hostKind: ${hostKind}`);
  return {
    windowKind: identity.windowKind,
    surfaceKind: contractKind,
    hostKind: identity.hostKind,
    parentRequired: identity.parentRequired,
  };
}

export function requiredPrimitiveIdsForContract(
  contract: SurfaceContract,
): string[] {
  const ids = new Set<string>([
    "devtools.targets.inspect",
    "devtools.surface.inspect",
  ]);
  switch (contract.proofPolicy) {
    case "StateReceiptProof":
      break;
    case "StateAndElementsProof":
      ids.add("devtools.elements.snapshot");
      break;
    case "ChildViewStateProof":
      ids.add("devtools.elements.snapshot");
      ids.add("devtools.focus.inspect");
      break;
    case "PopupStateProof":
      ids.add("devtools.elements.snapshot");
      ids.add("devtools.focus.inspect");
      ids.add("devtools.layout.measure");
      break;
    default:
      throw new Error(`unknown proof policy: ${contract.proofPolicy}`);
  }
  if (contract.focusPolicy !== "NoEditableFocus") {
    ids.add("devtools.focus.inspect");
  }
  if (contract.keyboardPolicy !== "NoEditableKeyboard") {
    ids.add("devtools.keyboard.inspect");
  }
  if (contract.actionsPolicy === "ActionsDialogActions") {
    ids.add("devtools.actions.inspect");
    ids.add("devtools.act");
  } else if (contract.actionsPolicy !== "NoSurfaceActions") {
    ids.add("devtools.act");
  }
  if (
    contract.visualPolicy === "SplitPreviewVisual" ||
    contract.visualPolicy === "ContentPaneVisual" ||
    contract.visualPolicy === "PopupVisual"
  ) {
    ids.add("devtools.layout.measure");
  }
  if (contract.vocabulary?.inputOwnership === "LauncherFilter") {
    ids.add("devtools.scroll.inspect");
  }
  return [...ids].sort();
}

export type ProfileResolution = {
  profile: CoverageProfile | null;
  relation: "Direct" | "Derived" | null;
  priority: number | null;
  errors: string[];
};

function selectorMatches(
  selector: CoverageProfile["bindingSelectors"][number],
  mapping: { contractKind: string; appViewVariant: string; family: string; hostKind: string },
): boolean {
  if (selector.contractKinds && !selector.contractKinds.includes(mapping.contractKind)) return false;
  if (selector.appViewVariants && !selector.appViewVariants.includes(mapping.appViewVariant)) return false;
  if (selector.families && !selector.families.includes(mapping.family)) return false;
  if (selector.hostKinds && !selector.hostKinds.includes(mapping.hostKind)) return false;
  return true;
}

export function resolveCoverageProfile(
  mapping: { contractKind: string; appViewVariant: string; family: string; hostKind: string },
  profiles: readonly CoverageProfile[] = coverageProfiles,
): ProfileResolution {
  type Candidate = { profile: CoverageProfile; relation: "Direct" | "Derived"; priority: number };
  const candidates: Candidate[] = [];
  for (const profile of profiles) {
    for (const selector of profile.bindingSelectors) {
      if (selectorMatches(selector, mapping)) {
        candidates.push({ profile, relation: selector.relation, priority: selector.priority });
      }
    }
  }
  if (candidates.length === 0) {
    return {
      profile: null,
      relation: null,
      priority: null,
      errors: [`no coverage profile selector matches ${mapping.contractKind}::${mapping.appViewVariant}@${mapping.hostKind}`],
    };
  }
  const best = Math.max(...candidates.map((candidate) => candidate.priority));
  const winners = candidates.filter((candidate) => candidate.priority === best);
  const distinctWinners = new Set(winners.map((winner) => winner.profile.id));
  if (distinctWinners.size > 1) {
    return {
      profile: null,
      relation: null,
      priority: best,
      errors: [
        `ambiguous profile selection for ${mapping.contractKind}::${mapping.appViewVariant}@${mapping.hostKind}: ${[...distinctWinners].sort().join(", ")} tie at priority ${best}`,
      ],
    };
  }
  const winner = winners[0];
  // Derived-only fallback rule: a host-wide fallback must never claim Direct.
  return { profile: winner.profile, relation: winner.relation, priority: winner.priority, errors: [] };
}

export function deriveEvidenceGrade(
  profile: CoverageProfile,
  relation: "Direct" | "Derived",
  missingPrimitiveIds: readonly string[],
): Exclude<EvidenceGrade, "Alias"> {
  if (profile.status === "planned" || profile.status === "missing") {
    return "Unsupported";
  }
  if (missingPrimitiveIds.length > 0) {
    return "Partial";
  }
  return relation;
}

export function bindingSupported(binding: Pick<CoverageBinding, "evidenceGrade" | "missingPrimitiveIds">): boolean {
  return (
    (binding.evidenceGrade === "Direct" || binding.evidenceGrade === "Derived") &&
    binding.missingPrimitiveIds.length === 0
  );
}

export function fixtureFamilyForMapping(family: string, hostKind: string): string {
  if (hostKind === "ActionsDialog" || hostKind === "PromptPopup") return "attached-popup-dialog";
  if (hostKind === "NotesWindow" || hostKind === "DictationWindow" || hostKind === "AgentChatDetached") {
    return "native-secondary-window";
  }
  const fixtureFamily = sourceFamilyToFixtureFamily[family as keyof typeof sourceFamilyToFixtureFamily];
  if (!fixtureFamily) throw new Error(`unknown source family: ${family}`);
  return fixtureFamily;
}

export type BindingBuildResult = {
  set: CoverageBindingSet;
  errors: string[];
};

export function buildCoverageBindingSet(
  canonicalMappings: CanonicalSurfaceMapping[],
  profiles: readonly CoverageProfile[] = coverageProfiles,
): BindingBuildResult {
  const errors: string[] = [];
  const bindings: CoverageBindingRecord[] = [];

  for (const mapping of canonicalMappings) {
    const family = mapping.contract.vocabulary?.family ?? "Unknown";
    const hostKind = resolveContractHost(mapping.contractKind);
    const resolution = resolveCoverageProfile(
      { contractKind: mapping.contractKind, appViewVariant: mapping.appViewVariant, family, hostKind },
      profiles,
    );
    errors.push(...resolution.errors);
    if (!resolution.profile || !resolution.relation) continue;
    const requiredPrimitiveIds = requiredPrimitiveIdsForContract(mapping.contract);
    const available = new Set(resolution.profile.availablePrimitiveIds);
    const missingPrimitiveIds = requiredPrimitiveIds.filter((id) => !available.has(id));
    const evidenceGrade = deriveEvidenceGrade(resolution.profile, resolution.relation, missingPrimitiveIds);
    const binding: CoverageBindingRecord = {
      contractKind: mapping.contractKind,
      appViewVariant: mapping.appViewVariant,
      hostKind,
      profileId: resolution.profile.id,
      evidenceGrade,
      fixtureFamily: fixtureFamilyForMapping(family, hostKind),
      expectedTargetIdentity: expectedTargetIdentity(mapping.contractKind, hostKind),
      requiredPrimitiveIds,
      missingPrimitiveIds,
      bindingId: bindingIdFor({ contractKind: mapping.contractKind, appViewVariant: mapping.appViewVariant, hostKind }),
      profileStatus: resolution.profile.status,
      relation: resolution.relation,
      supported: false,
      sourceContractFingerprint: sha256(JSON.stringify(mapping.contract)),
    };
    binding.supported = bindingSupported(binding);
    bindings.push(binding);
  }
  bindings.sort((a, b) => a.bindingId.localeCompare(b.bindingId));

  const bindingById = new Map(bindings.map((binding) => [binding.bindingId, binding]));
  const aliases: CoverageAliasBinding[] = [];
  for (const alias of coverageSurfaceAliases) {
    if (alias.countsAsCoverage !== false) {
      errors.push(`alias ${alias.alias} must declare countsAsCoverage: false`);
      continue;
    }
    const targetVariant = "appViewVariant" in alias.resolvesTo ? alias.resolvesTo.appViewVariant : undefined;
    const canonicalCandidates = bindings.filter((binding) =>
      binding.contractKind === alias.resolvesTo.surfaceKind &&
      (targetVariant === undefined || binding.appViewVariant === targetVariant)
    );
    if (canonicalCandidates.length !== 1) {
      errors.push(`alias ${alias.alias} does not resolve to exactly one canonical binding (${canonicalCandidates.length})`);
      continue;
    }
    const canonical = canonicalCandidates[0];
    const hostOverride = "hostKind" in alias.resolvesTo ? alias.resolvesTo.hostKind : undefined;
    const aliasHost = hostOverride ?? canonical.hostKind;
    const mappingFamily = canonicalMappings.find((mapping) =>
      mapping.contractKind === canonical.contractKind && mapping.appViewVariant === canonical.appViewVariant
    )?.contract.vocabulary?.family ?? "Unknown";
    // Alias profile: resolve at the overridden host when a selector exists
    // there; otherwise inherit the canonical binding's profile.
    const aliasResolution = hostOverride
      ? resolveCoverageProfile(
          { contractKind: canonical.contractKind, appViewVariant: canonical.appViewVariant, family: mappingFamily, hostKind: aliasHost },
          profiles,
        )
      : null;
    const aliasProfile = aliasResolution?.profile ?? null;
    aliases.push({
      contractKind: canonical.contractKind,
      appViewVariant: canonical.appViewVariant,
      hostKind: aliasHost,
      profileId: aliasProfile?.id ?? canonical.profileId,
      fixtureFamily: fixtureFamilyForMapping(mappingFamily, aliasHost),
      expectedTargetIdentity: expectedTargetIdentity(canonical.contractKind, aliasHost),
      requiredPrimitiveIds: canonical.requiredPrimitiveIds,
      missingPrimitiveIds: aliasProfile
        ? canonical.requiredPrimitiveIds.filter((id) => !aliasProfile.availablePrimitiveIds.includes(id))
        : canonical.missingPrimitiveIds,
      bindingId: `alias:${alias.alias}`,
      profileStatus: (aliasProfile ?? { status: canonical.profileStatus }).status as CoverageStatus,
      sourceContractFingerprint: canonical.sourceContractFingerprint,
      alias: alias.alias,
      evidenceGrade: "Alias",
      countsAsCoverage: false,
      supported: false,
      canonicalBindingId: canonical.bindingId,
    });
  }
  aliases.sort((a, b) => a.alias.localeCompare(b.alias));

  const sourceMappingKeys = canonicalMappings
    .map((mapping) => bindingIdFor({
      contractKind: mapping.contractKind,
      appViewVariant: mapping.appViewVariant,
      hostKind: resolveContractHost(mapping.contractKind),
    }))
    .sort();

  const set: CoverageBindingSet = {
    bindings,
    aliases,
    sourceMappingKeys,
    fingerprint: sha256(JSON.stringify({ bindings, aliases, sourceMappingKeys })),
  };
  return { set, errors };
}

export function validateCoverageBindingSet(
  set: CoverageBindingSet,
  profiles: readonly CoverageProfile[] = coverageProfiles,
): CoverageBindingValidation {
  const errors: string[] = [];
  const knownProfiles = new Map(profiles.map((profile) => [profile.id, profile]));
  const knownPrimitiveIds = new Set(receiptSchemaRegistry.map((entry) => entry.primitiveId));

  const tuples = set.bindings.map((binding) => bindingTuple(binding));
  const duplicateTuples = tuples.filter((tuple, index) => tuples.indexOf(tuple) !== index);
  for (const duplicate of [...new Set(duplicateTuples)]) {
    errors.push(`duplicate canonical binding tuple: ${duplicate}`);
  }

  const bindingIds = new Set(set.bindings.map((binding) => binding.bindingId));
  const missingBindingIds = set.sourceMappingKeys.filter((key) => !bindingIds.has(key));
  for (const missing of missingBindingIds) {
    errors.push(`missing canonical binding: ${missing}`);
  }
  const unexpectedBindingIds = [...bindingIds].filter((id) => !set.sourceMappingKeys.includes(id));
  for (const unexpected of unexpectedBindingIds) {
    errors.push(`binding not present in source census: ${unexpected}`);
  }

  let invalidProfileReferenceCount = 0;
  let invalidPrimitiveReferenceCount = 0;
  let supportInvariantViolationCount = 0;

  for (const binding of set.bindings) {
    const profile = knownProfiles.get(binding.profileId);
    if (!profile) {
      invalidProfileReferenceCount += 1;
      errors.push(`binding ${binding.bindingId} references unknown profile: ${binding.profileId}`);
    }
    for (const primitiveId of [...binding.requiredPrimitiveIds, ...binding.missingPrimitiveIds]) {
      if (!knownPrimitiveIds.has(primitiveId)) {
        invalidPrimitiveReferenceCount += 1;
        errors.push(`binding ${binding.bindingId} references unknown primitive: ${primitiveId}`);
      }
    }
    if (binding.evidenceGrade === "Alias") {
      supportInvariantViolationCount += 1;
      errors.push(`alias grade found in canonical bindings: ${binding.bindingId}`);
    }
    const expectedSupported = bindingSupported(binding);
    if (binding.supported !== expectedSupported) {
      supportInvariantViolationCount += 1;
      errors.push(`binding ${binding.bindingId} violates the supported invariant (${binding.evidenceGrade}, ${binding.missingPrimitiveIds.length} missing, supported: ${binding.supported})`);
    }
    if (profile && (profile.status === "planned" || profile.status === "missing")) {
      if (binding.evidenceGrade !== "Unsupported" || binding.supported) {
        supportInvariantViolationCount += 1;
        errors.push(`binding ${binding.bindingId} against ${profile.status} profile must be Unsupported`);
      }
    }
    if (binding.evidenceGrade === "Direct" && binding.relation !== "Direct") {
      errors.push(`binding ${binding.bindingId} grade Direct disagrees with relation ${binding.relation}`);
    }
  }

  for (const alias of set.aliases) {
    if (alias.evidenceGrade !== "Alias") {
      supportInvariantViolationCount += 1;
      errors.push(`alias ${alias.alias} must be graded Alias, got ${String(alias.evidenceGrade)}`);
    }
    if (alias.countsAsCoverage !== false) {
      errors.push(`alias ${alias.alias} must not count as coverage`);
    }
    if (alias.supported !== false) {
      supportInvariantViolationCount += 1;
      errors.push(`alias ${alias.alias} must not be supported`);
    }
    if (!bindingIds.has(alias.canonicalBindingId)) {
      errors.push(`alias ${alias.alias} resolves to unknown canonical binding: ${alias.canonicalBindingId}`);
    }
    if (!knownProfiles.has(alias.profileId)) {
      invalidProfileReferenceCount += 1;
      errors.push(`alias ${alias.alias} references unknown profile: ${alias.profileId}`);
    }
  }

  return {
    pass: errors.length === 0,
    disposition: errors.length === 0 ? "EVALUABLE_PASS" : "INVALID_SCHEMA",
    errors,
    duplicateTupleCount: new Set(duplicateTuples).size,
    missingBindingCount: missingBindingIds.length,
    invalidProfileReferenceCount,
    invalidPrimitiveReferenceCount,
    supportInvariantViolationCount,
  };
}

// ---------------------------------------------------------------------------
// PF-009 negative controls
// ---------------------------------------------------------------------------

type NegativeControlMutation = {
  id: string;
  file: string;
  description: string;
  mutate: (set: CoverageBindingSet) => CoverageBindingSet;
};

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

export const coverageNegativeControlMutations: NegativeControlMutation[] = [
  {
    id: "duplicate-binding",
    file: "duplicate-binding.json",
    description: "Duplicate the first canonical binding tuple.",
    mutate(set) {
      const mutated = clone(set);
      mutated.bindings.push(clone(mutated.bindings[0]));
      return mutated;
    },
  },
  {
    id: "missing-binding",
    file: "missing-binding.json",
    description: "Remove the first canonical binding.",
    mutate(set) {
      const mutated = clone(set);
      mutated.bindings.splice(0, 1);
      return mutated;
    },
  },
  {
    id: "alias-graded-direct",
    file: "alias-graded-direct.json",
    description: "Grade the first orientation alias Direct.",
    mutate(set) {
      const mutated = clone(set);
      (mutated.aliases[0] as { evidenceGrade: string }).evidenceGrade = "Direct";
      return mutated;
    },
  },
  {
    id: "nonexistent-profile",
    file: "nonexistent-profile.json",
    description: "Point the first binding at a profile id absent from the registry.",
    mutate(set) {
      const mutated = clone(set);
      mutated.bindings[0].profileId = "profile-that-does-not-exist";
      return mutated;
    },
  },
  {
    id: "partial-called-supported",
    file: "partial-called-supported.json",
    description: "Mark a binding with missing primitives as supported.",
    mutate(set) {
      const mutated = clone(set);
      const target = mutated.bindings.find((binding) => binding.missingPrimitiveIds.length > 0)
        ?? mutated.bindings[0];
      if (target.missingPrimitiveIds.length === 0) {
        target.missingPrimitiveIds = ["devtools.elements.snapshot"];
        target.evidenceGrade = "Partial";
      }
      target.supported = true;
      return mutated;
    },
  },
];

export type NegativeControlResult = {
  id: string;
  description: string;
  expectedDisposition: "INVALID_SCHEMA";
  actualDisposition: string;
  pass: boolean;
  errorSample: string[];
  receiptPath: string | null;
};

export function runCoverageNegativeControls(
  validSet: CoverageBindingSet,
  outDir: string | null,
): NegativeControlResult[] {
  return coverageNegativeControlMutations.map((mutation) => {
    const mutated = mutation.mutate(validSet);
    const validation = validateCoverageBindingSet(mutated);
    const pass = validation.disposition === "INVALID_SCHEMA" && !validation.pass;
    let receiptPath: string | null = null;
    if (outDir) {
      receiptPath = `${outDir}/${mutation.file}`;
      mkdirSync(dirname(receiptPath), { recursive: true });
      writeFileSync(
        receiptPath,
        `${JSON.stringify(
          {
            schemaVersion: 1,
            tool: "script-kit-devtools.surfaces",
            control: mutation.id,
            description: mutation.description,
            expectedDisposition: "INVALID_SCHEMA",
            actualDisposition: validation.disposition,
            pass,
            validation: {
              duplicateTupleCount: validation.duplicateTupleCount,
              missingBindingCount: validation.missingBindingCount,
              invalidProfileReferenceCount: validation.invalidProfileReferenceCount,
              invalidPrimitiveReferenceCount: validation.invalidPrimitiveReferenceCount,
              supportInvariantViolationCount: validation.supportInvariantViolationCount,
              errorSample: validation.errors.slice(0, 5),
            },
            generatedAt: new Date().toISOString(),
          },
          null,
          2,
        )}\n`,
      );
    }
    return {
      id: mutation.id,
      description: mutation.description,
      expectedDisposition: "INVALID_SCHEMA" as const,
      actualDisposition: validation.disposition,
      pass,
      errorSample: validation.errors.slice(0, 3),
      receiptPath,
    };
  });
}

// ---------------------------------------------------------------------------
// Bindings pipeline (shared by report + bindings CLI)
// ---------------------------------------------------------------------------

export type BindingsPipeline = {
  registry: SurfaceContractRegistry;
  rust: RustSurfaceCensus;
  parity: ProjectionParity;
  featureMapGate: FeatureMapGate;
  canonicalMappings: CanonicalSurfaceMapping[];
  build: BindingBuildResult;
  validation: CoverageBindingValidation;
  censusPass: boolean;
  usable: boolean;
  blockReasons: string[];
  fingerprints: Record<string, string | null>;
};

export async function runBindingsPipeline(): Promise<BindingsPipeline> {
  const registry = await loadSurfaceContractRegistry();
  const rustText = await readText(paths.rustSource);
  const rust = extractRustSurfaceCensus(rustText);
  const parity = compareContractProjection(rust, registry);
  const featureMapGate = await resolveFeatureMapGate();
  const canonicalMappings = buildCanonicalMappings(registry, rust);
  const build = buildCoverageBindingSet(canonicalMappings);
  const validation = validateCoverageBindingSet(build.set);
  const coverageRegistryErrors = validateCoverageProfiles();

  const censusPass =
    rust.kinds.length === EXPECTED_CENSUS.contractKindCount &&
    rust.mappings.length === EXPECTED_CENSUS.contractMappingCount &&
    rust.errors.length === 0;

  const blockReasons: string[] = [];
  if (!censusPass) {
    blockReasons.push(
      `source census drift: ${rust.kinds.length} kinds / ${rust.mappings.length} mappings (expected ${EXPECTED_CENSUS.contractKindCount}/${EXPECTED_CENSUS.contractMappingCount})`,
    );
    blockReasons.push(...rust.errors.map((error) => `rust extraction: ${error}`));
  }
  if (!parity.pass) {
    blockReasons.push(
      `generated contract does not match Rust source (missing: ${parity.missingMappingKeys.length}, unexpected: ${parity.unexpectedMappingKeys.length})`,
    );
  }
  if (!registry.generatedFrom || !registry.registry) {
    blockReasons.push("generated contract lacks provenance (generatedFrom/registry)");
  }
  if (!registry.rustSourceExists) {
    blockReasons.push(`generated contract source path does not resolve: ${registry.generatedFrom}`);
  }
  if (!featureMapGate.pass) {
    blockReasons.push(
      `feature map is not a real nonempty source (status: ${featureMapGate.status}; attempted: ${featureMapGate.attemptedPaths.join(", ")})`,
    );
  }
  if (coverageRegistryErrors.length > 0) {
    blockReasons.push(
      ...coverageRegistryErrors.map((error) => `coverage registry: ${error}`),
    );
  }

  const surfacesText = await readText(paths.surfacesModule);
  const coverageText = await readText(paths.coverage);
  const featureMapText = await readTextIfExists(paths.featureMap);

  return {
    registry,
    rust,
    parity,
    featureMapGate,
    canonicalMappings,
    build,
    validation,
    censusPass,
    usable: censusPass && parity.pass && featureMapGate.pass && registry.rustSourceExists
      && build.errors.length === 0 && validation.pass
      && coverageRegistryErrors.length === 0,
    blockReasons,
    fingerprints: {
      rustSourcePath: paths.rustSource,
      rustSourceSha256: registry.rustSha256,
      contractPath: paths.contracts,
      contractSha256: registry.contractSha256,
      coverageModulePath: paths.coverage,
      coverageModuleSha256: sha256(coverageText),
      surfacesModulePath: paths.surfacesModule,
      surfacesModuleSha256: sha256(surfacesText),
      featureMapPath: featureMapGate.sourcePath,
      featureMapSha256: featureMapText === null ? null : sha256(featureMapText),
    },
  };
}

export function buildBindingsReceipt(
  pipeline: BindingsPipeline,
  negativeControls: NegativeControlResult[],
) {
  const { rust, parity, featureMapGate, build, validation } = pipeline;
  const catalog = parseTaskCatalog(
    readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
    DEFAULT_CONSISTENCY_CATALOG_PATH,
  );
  const task = catalog.byId.get("PF-009");
  const statusCounts = coverageProfiles.reduce<Record<CoverageStatus, number>>(
    (counts, profile) => {
      counts[profile.status] += 1;
      return counts;
    },
    { supported: 0, partial: 0, missing: 0, planned: 0 },
  );
  const gradeCounts = build.set.bindings.reduce<Record<string, number>>((counts, binding) => {
    counts[binding.evidenceGrade] = (counts[binding.evidenceGrade] ?? 0) + 1;
    return counts;
  }, {});
  const blocked =
    pipeline.blockReasons.length > 0 ||
    catalog.errors.length > 0 ||
    !task;
  const negativesPass = negativeControls.every((control) => control.pass);
  const classification = blocked
    ? "blocked-by-scope-drift"
    : validation.pass && negativesPass
      ? "ok"
      : "reproduced";

  return {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    tool: "script-kit-devtools.surfaces",
    command: "surfaces.coverage-bindings",
    evidenceClass: "STATIC_INVENTORY",
    taskIds: ["PF-009"],
    catalogBinding: task
      ? {
          catalogPath: DEFAULT_CONSISTENCY_CATALOG_PATH,
          taskId: task.id,
          title: task.title,
          sectionSha256: task.sectionSha256,
        }
      : null,
    classification,
    census: {
      expected: { ...EXPECTED_CENSUS },
      actual: {
        contractKindCount: rust.kinds.length,
        contractMappingCount: rust.mappings.length,
        uniqueAppViewVariantCount: rust.variantNames.length,
        runtimeCoverageProfileCount: coverageProfiles.length,
        orientationAliasCount: coverageSurfaceAliases.length,
      },
    },
    sourceParity: {
      pass: parity.pass,
      missingMappingKeys: parity.missingMappingKeys,
      unexpectedMappingKeys: parity.unexpectedMappingKeys,
      missingKinds: parity.missingKinds,
      unexpectedKinds: parity.unexpectedKinds,
    },
    featureMapGate: {
      pass: featureMapGate.pass,
      sourcePath: featureMapGate.sourcePath,
      sourceSha256: featureMapGate.sourceSha256,
      parsedEntryCount: featureMapGate.parsedEntryCount,
      attemptedPaths: featureMapGate.attemptedPaths,
      status: featureMapGate.status,
    },
    sourceFingerprints: pipeline.fingerprints,
    profileRegistry: {
      ids: coverageProfiles.map((profile) => profile.id),
      statusCounts,
      validationErrorCount: validateCoverageProfiles().length,
    },
    bindingSetUsable: pipeline.usable,
    bindings: pipeline.usable ? build.set.bindings : [],
    aliases: pipeline.usable ? build.set.aliases : [],
    structuralBindingCount: build.set.bindings.length,
    structuralAliasCount: build.set.aliases.length,
    bindingSetFingerprint: build.set.fingerprint,
    summary: {
      duplicateTupleCount: validation.duplicateTupleCount,
      missingBindingCount: validation.missingBindingCount,
      invalidProfileReferenceCount: validation.invalidProfileReferenceCount,
      invalidPrimitiveReferenceCount: validation.invalidPrimitiveReferenceCount,
      supportInvariantViolationCount: validation.supportInvariantViolationCount,
      evidenceGradeCounts: gradeCounts,
      staticDirectBindingCount: gradeCounts.Direct ?? 0,
      freshDirectRuntimeProofCount: 0,
      runtimeProofDisposition: "NOT_EVALUATED",
      plannedBindingCount: build.set.bindings.filter((binding) => binding.profileStatus === "planned").length,
      supportedBindingCount: build.set.bindings.filter((binding) => binding.supported).length,
    },
    blockReasons: pipeline.blockReasons,
    generatorNotes: build.errors,
    assertions: blocked ? [] : [
      { id: "census-37-54", pass: pipeline.censusPass },
      { id: "source-parity", pass: parity.pass },
      { id: "binding-set-valid", pass: validation.pass },
      { id: "negative-controls-detected", pass: negativesPass },
    ],
    negativeControls: negativeControls.map((control) => ({
      id: control.id,
      description: control.description,
      expectedDisposition: control.expectedDisposition,
      actualDisposition: control.actualDisposition,
      pass: control.pass,
      receiptPath: control.receiptPath,
    })),
    requiredPrimitives: [],
    missingPrimitives: [],
    errors: [],
    warnings: [],
    interference: { monitored: false, disposition: null, details: null },
    cleanup: { closed: true, ownedPids: [], ownedSessions: [], survivors: [] },
    evidence: {
      intended: { source: "rust-census", kinds: rust.kinds.length, mappings: rust.mappings.length },
      model: { source: "generated-contract", parityPass: parity.pass },
    },
  };
}

// ---------------------------------------------------------------------------
// Inventory report (RPT-001-compatible, extended with coverageBindings)
// ---------------------------------------------------------------------------

function aliasesForSurfaceKind(surfaceKind: string) {
  return coverageSurfaceAliases.filter((alias) => alias.resolvesTo.surfaceKind === surfaceKind).map((alias) => alias.alias);
}

function featureIds(entries: FeatureMapEntry[], owners: string[], terms: string[]) {
  return entries
    .filter((entry) => {
      const haystack = `${entry.feature} ${entry.cluster}`.toLowerCase();
      return (
        entry.primaryOwners.some((owner) => owners.includes(owner)) ||
        terms.some((term) => haystack.includes(term.toLowerCase()))
      );
    })
    .map((entry) => entry.id);
}

export function buildOracleBatches(contracts: SurfaceContract[], features: FeatureMapEntry[]): OracleBatch[] {
  const surfaceKinds = new Set(contracts.map((entry) => entry.surfaceKind));
  const keepKinds = (kinds: string[]) => kinds.filter((kind) => surfaceKinds.has(kind));

  return [
    {
      id: "platform-windowing-permissions",
      name: "Platform windows, containers, materials, resizing, screenshots, lifecycle",
      priority: 1,
      outsideInPhase: "window-container",
      priorityRationale: "Highest impact: outer windows, materials, safe areas, lifecycle, and resize behavior constrain every inner layout.",
      owners: ["platform-windowing-macos", "window-resizing", "launcher-surface-contracts"],
      surfaceKinds: keepKinds(["About", "Feedback"]),
      featureIds: featureIds(features, ["platform-windowing-macos", "window-resizing", "launcher-surface-contracts"], [
        "window",
        "permission",
        "tray",
        "sizing",
      ]),
      requiredDevToolsPrimitives: ["devtools.windows.inspect", "devtools.permissions.inspect", "devtools.visual.compare", "devtools.lifecycle.trace"],
      questionsForOracle: [
        "How should Script Kit prove outer window/container material, resize, safe-area, and backdrop behavior before auditing inner controls?",
        "Which permission and screenshot receipts stay passive and avoid changing macOS settings?",
      ],
    },
    {
      id: "launcher-main-actions",
      name: "Launcher, main menu, source filters, actions, shortcuts, aliases",
      priority: 2,
      outsideInPhase: "surface-shell",
      priorityRationale: "Main window shell, launcher container, action popup container, and footer chrome define the default app layout.",
      owners: ["main-menu-search-selection", "actions-popups", "keyboard-focus-routing"],
      surfaceKinds: keepKinds(["ScriptList", "ActionsDialog", "ConfirmPrompt"]),
      featureIds: featureIds(features, ["main-menu-search-selection", "actions-popups", "keyboard-focus-routing"], [
        "main menu",
        "source filter",
        "shortcut",
        "alias",
      ]),
      requiredDevToolsPrimitives: ["devtools.targets.watch", "devtools.act", "devtools.measure.layout", "devtools.measure.text"],
      questionsForOracle: [
        "Which target-scoped action and shortcut receipts make main-menu bugs reproducible without recipes?",
        "How should actions popups expose route stack, anchor rects, disabled reasons, and clipping metrics?",
      ],
    },
    {
      id: "prompt-runtime-family",
      name: "Prompt runtime family, child content, terminal, editor, forms, path, drop, env, confirm",
      priority: 3,
      outsideInPhase: "surface-shell",
      priorityRationale: "Prompt windows and child-content containers are the broadest SDK-facing layout shells after the launcher.",
      owners: ["prompt-runtime", "sdk-script-execution", "quick-terminal-pty", "file-search-portals"],
      surfaceKinds: keepKinds(["PromptEntity", "PromptChildContent", "ExplicitPromptEntity", "UtilityChildContent", "Webcam", "ConfirmPrompt"]),
      featureIds: featureIds(features, ["prompt-runtime", "sdk-script-execution", "quick-terminal-pty", "file-search-portals"], [
        "prompt",
        "term",
        "editor",
        "path",
        "drop",
        "env",
      ]),
      requiredDevToolsPrimitives: ["devtools.prompt.inspect", "devtools.measure.scroll", "devtools.measure.selection", "devtools.act.safeSubmit"],
      questionsForOracle: [
        "What per-prompt contract fields should exist before an agent can call a prompt UX bug reproduced?",
        "How should oversized div, md, editor, terminal, and form containers report scrollability and resize pressure?",
      ],
    },
    {
      id: "builtins-filterable",
      name: "Built-in filterable views and split-preview rows",
      priority: 4,
      outsideInPhase: "surface-shell",
      priorityRationale: "Filterable built-ins reuse shared list and preview containers; prove the shell before row-level controls.",
      owners: ["builtin-filterable-surfaces", "theme-config-preferences", "storage-cache-security"],
      surfaceKinds: keepKinds([
        "ClipboardHistory",
        "AppLauncher",
        "WindowSwitcher",
        "BrowserTabs",
        "GenericFilterableList",
        "Settings",
        "KitStoreBrowse",
        "KitStoreInstalled",
        "ProcessManager",
        "CurrentAppCommands",
        "ThemeChooser",
        "EmojiPicker",
        "AgentChatHistory",
      ]),
      featureIds: featureIds(features, ["builtin-filterable-surfaces", "theme-config-preferences", "storage-cache-security"], [
        "built-in",
        "clipboard",
        "settings",
        "theme",
      ]),
      requiredDevToolsPrimitives: ["devtools.resources.inspect", "devtools.measure.preview", "devtools.list.diff", "devtools.storage.fingerprint"],
      questionsForOracle: [
        "Which shared list, preview, cache, and privacy receipts cover all filterable built-ins?",
        "How should split-preview surfaces expose preview overflow, stale selection, and row action availability?",
      ],
    },
    {
      id: "portals-resources-context",
      name: "File portals, attachment portals, MCP resources, context catalogs",
      priority: 5,
      outsideInPhase: "surface-shell",
      priorityRationale: "Portal windows and return containers affect attachment, resource, and context layouts before individual rows.",
      owners: ["file-search-portals", "mcp-context-resources", "agent_chat-context-composer"],
      surfaceKinds: keepKinds(["FileSearchMini", "FileSearchFull", "AttachmentPortalBrowser", "ScriptTemplateCatalog", "SdkReference"]),
      featureIds: featureIds(features, ["file-search-portals", "mcp-context-resources", "agent_chat-context-composer"], [
        "portal",
        "resource",
        "context",
        "file",
      ]),
      requiredDevToolsPrimitives: ["devtools.portal.inspect", "devtools.resources.inspect", "devtools.act.portalReturn", "devtools.privacy.redaction"],
      questionsForOracle: [
        "How should origin, return target, staged parts, and privacy-safe resource rows be proven across portals?",
        "Which receipts prevent agents from confusing portal fixture data with real user files or context?",
      ],
    },
    {
      id: "agent_chat-chat-ai",
      name: "Agent Chat chat, composer, history, SDK AI APIs, model setup",
      priority: 6,
      outsideInPhase: "content-controls",
      priorityRationale: "Agent Chat has important window shells, but after detached/window proof the remaining work is composer and transcript internals.",
      owners: ["agent_chat-chat-core", "agent_chat-context-composer", "sdk-script-execution"],
      surfaceKinds: keepKinds(["AgentChat", "AgentChatHistory", "AttachmentPortalBrowser", "GenericFilterableList"]),
      featureIds: featureIds(features, ["agent_chat-chat-core", "agent_chat-context-composer", "sdk-script-execution"], ["agent_chat", "agent chat", "ai"]),
      requiredDevToolsPrimitives: ["devtools.agent_chat.inspect", "devtools.agent_chat.timeline", "devtools.composer.inspect", "devtools.turn.diff"],
      questionsForOracle: [
        "What generation, host, composer, model, and context-part receipts are required for Agent Chat UI bugs?",
        "How should agents prove wrong-host, stale-turn, and delayed-action failures without starting external AI calls?",
      ],
    },
    {
      id: "notes-dictation-media",
      name: "Notes, notes-hosted Agent Chat, dictation, media, history, target delivery",
      priority: 7,
      outsideInPhase: "content-controls",
      priorityRationale: "Notes and Dictation have practical window proof; remaining work is embedded Agent Chat, media state, history, and delivery details.",
      owners: ["notes-window", "dictation-media", "agent_chat-chat-core"],
      surfaceKinds: keepKinds(["AgentChat", "AgentChatHistory", "ClipboardHistory"]),
      featureIds: featureIds(features, ["notes-window", "dictation-media", "agent_chat-chat-core"], ["notes", "dictation", "media"]),
      requiredDevToolsPrimitives: ["devtools.notes.inspect", "devtools.media.inspect", "devtools.measure.selection", "devtools.delivery.trace"],
      questionsForOracle: [
        "Which passive media, target-delivery, editor-selection, and notes-resize receipts unlock reliable Dictation and Notes bug proof?",
        "How should the tools separate visible Notes UI state, embedded Agent Chat state, and background storage state?",
      ],
    },
    {
      id: "observability-security-storage",
      name: "Observability, storage, sharing, security, diagnostics, replay",
      priority: 8,
      outsideInPhase: "supporting-systems",
      priorityRationale: "Supporting receipts and diagnostics are essential, but they should follow visible window/container proof.",
      owners: ["dev-loop-observability", "storage-cache-security", "testing-quality-gates"],
      surfaceKinds: keepKinds(["Feedback", "SdkReference", "ScriptTemplateCatalog"]),
      featureIds: featureIds(features, ["dev-loop-observability", "storage-cache-security", "testing-quality-gates"], [
        "logging",
        "diagnostics",
        "sharing",
        "storage",
      ]),
      requiredDevToolsPrimitives: ["devtools.events.tail", "devtools.storage.fingerprint", "devtools.security.inspect", "devtools.investigate"],
      questionsForOracle: [
        "What event, storage, privacy, and replay receipts should every investigation artifact include?",
        "How should missing primitive reports become a prioritized build backlog instead of failed bug investigations?",
      ],
    },
  ];
}

export type SurfaceReportInputs = {
  pipeline: BindingsPipeline;
  maintainedFeatureAtlasExists: boolean;
  featureMapText: string;
};

export function buildSurfaceReport(inputs: SurfaceReportInputs) {
  const { pipeline } = inputs;
  const contracts = pipeline.registry;
  const compatibilityFeatureMap = parseFeatureMap(inputs.featureMapText);
  const featureMap = pipeline.featureMapGate.pass
    ? pipeline.featureMapGate.entries
    : compatibilityFeatureMap;
  const coverageSurfaceIds = coverageProfiles.map((profile) => profile.id);
  const coveredNames = new Set(coverageSurfaceIds);
  const contractSurfaceKinds = contracts.entries.map((entry) => entry.surfaceKind);
  const contractMappings = contracts.entries.flatMap((entry) =>
    entry.appViewVariants.map((variant) => ({ surfaceKind: entry.surfaceKind, appViewVariant: variant })),
  );
  const uniqueAppViewVariants = [...new Set(contractMappings.map((entry) => entry.appViewVariant))].sort();
  const excludedAuditKinds = new Set(liquidGlassAuditExclusions.map((entry) => entry.surfaceKind));
  const auditContracts = contracts.entries.filter((entry) => !excludedAuditKinds.has(entry.surfaceKind));
  const contractFamilies = [...new Set(contracts.entries.map((entry) => entry.vocabulary?.family ?? "Unknown"))].sort();
  const ownerSkills = [...new Set(featureMap.flatMap((entry) => entry.primaryOwners))].sort();
  const batches = buildOracleBatches(auditContracts, featureMap);
  const serializeContract = (entry: SurfaceContract) => ({
    surfaceKind: entry.surfaceKind,
    appViewVariants: entry.appViewVariants,
    nativeFooterSurfaces: entry.appViewFooters
      .map((footer) => footer.nativeFooterSurface)
      .filter((footer): footer is string => Boolean(footer)),
    vocabulary: entry.vocabulary,
    focusPolicy: entry.focusPolicy,
    keyboardPolicy: entry.keyboardPolicy,
    actionsPolicy: entry.actionsPolicy,
    proofPolicy: entry.proofPolicy,
    visualPolicy: entry.visualPolicy,
    dismissPolicy: entry.dismissPolicy ?? null,
    automationSemanticSurface: entry.automationSemanticSurface,
    coverageAliases: aliasesForSurfaceKind(entry.surfaceKind),
  });

  return {
    schemaVersion: 1,
    tool: "script-kit-devtools.surfaces",
    generatedAt: new Date().toISOString(),
    philosophy:
      "Inventory app surfaces first, then build protocol/MCP/CLI DevTools primitives; scripted recipes remain regression packs after direct proof exists.",
    sourceArtifacts: [
      {
        path: paths.contracts,
        role: "Generated AppView to SurfaceKind contracts and proof policies.",
        generatedFrom: contracts.generatedFrom,
        registry: contracts.registry,
      },
      {
        path: paths.featureMap,
        role: "Feature ownership map and Oracle-backed chapters.",
      },
      {
        path: paths.coverage,
        role: "Currently checked-in DevTools domain and surface coverage.",
      },
    ],
    evidenceStatus: "SOURCE-CONFIRMED" as const,
    evidenceClass: "STATIC_INVENTORY" as const,
    inventoryNamespaces: {
      contractKindCount: contracts.entries.length,
      contractMappingCount: contractMappings.length,
      uniqueAppViewVariantCount: uniqueAppViewVariants.length,
      runtimeCoverageProfileCount: coverageSurfaceIds.length,
      orientationAliasCount: coverageSurfaceAliases.length,
    },
    totals: {
      surfaceContractCount: contracts.entries.length,
      liquidGlassAuditSurfaceCount: auditContracts.length,
      contractMappingCount: contractMappings.length,
      appViewVariantCount: uniqueAppViewVariants.length,
      featureMapCount: featureMap.length,
      ownerSkillCount: ownerSkills.length,
      currentlyCoveredSurfacesCount: coverageSurfaceIds.length,
      orientationAliasCount: coverageSurfaceAliases.length,
      oracleBatchCount: batches.length,
    },
    featureMapSource: {
      path: paths.featureMap,
      compatibilityIndexExists: inputs.featureMapText.trim().length > 0,
      parsedEntryCount: featureMap.length,
      maintainedAtlasPath: paths.maintainedFeatureAtlas,
      maintainedAtlasExists: inputs.maintainedFeatureAtlasExists,
      status: pipeline.featureMapGate.status,
    },
    sourceCensus: {
      pass: pipeline.censusPass,
      kinds: pipeline.rust.kinds.length,
      mappings: pipeline.rust.mappings.length,
      uniqueVariants: pipeline.rust.variantNames.length,
      extractionErrors: pipeline.rust.errors,
      parity: pipeline.parity,
    },
    coverageBindings: {
      usable: pipeline.usable,
      bindings: pipeline.build.set.bindings,
      aliases: pipeline.build.set.aliases,
      fingerprint: pipeline.build.set.fingerprint,
      blockReasons: pipeline.blockReasons,
    },
    runtimeCoverage: buildRuntimeCoverageScorecard(
      pipeline.build.set.bindings,
      [],
      { ownerValidationErrors: validateCoverageProfiles() },
    ),
    surfaceContracts: contracts.entries.map(serializeContract),
    auditSurfaceContracts: auditContracts.map(serializeContract),
    liquidGlassAuditExclusions,
    featureMap,
    existingDevToolsCoverage: {
      surfaceIds: coverageSurfaceIds,
      source: paths.coverage,
      note: "These are the only surfaces with explicit coverage.ts entries today; every other contract and feature family should be treated as backlog until a direct primitive exists.",
    },
    coverageSurfaceAliases,
    uncoveredContractSurfaceKinds: contractSurfaceKinds.filter((kind) => {
      const kebabKind = kind.replace(/[A-Z]/g, (char, index) => `${index ? "-" : ""}${char.toLowerCase()}`);
      return !coveredNames.has(kind) && !coveredNames.has(kebabKind);
    }),
    contractFamilies,
    ownerSkills,
    recommendedOracleBatches: batches,
    recommendedNext: [
      "Work outside-in: prove window/container material, resizing, and lifecycle before inner controls and content.",
      "Ask Oracle to turn each batch into inspect, measure, act, compare, media, resources, events, and investigate primitives.",
      "Add fail-closed CLI contracts before implementing runtime behavior so agents cannot confuse screenshots or recipes for proof.",
      "Promote recurring direct-primitive flows into agentic-testing recipes only after red/green receipts stabilize.",
    ],
  };
}

function markdown(report: Awaited<ReturnType<typeof buildSurfaceReport>>) {
  const lines = [
    "# Script Kit DevTools Surface Inventory",
    "",
    report.philosophy,
    "",
    "## Totals",
    "",
    `- Contract kinds: ${report.inventoryNamespaces.contractKindCount}`,
    `- Contract mappings: ${report.inventoryNamespaces.contractMappingCount}`,
    `- Unique AppView variants: ${report.inventoryNamespaces.uniqueAppViewVariantCount}`,
    `- Runtime coverage profiles: ${report.inventoryNamespaces.runtimeCoverageProfileCount}`,
    `- Non-counting orientation aliases: ${report.inventoryNamespaces.orientationAliasCount}`,
    `- Liquid Glass audit surfaces: ${report.totals.liquidGlassAuditSurfaceCount}`,
    `- Feature-map entries: ${report.totals.featureMapCount}`,
    `- Feature-map source status: ${report.featureMapSource.status}`,
    `- Owner skills: ${report.totals.ownerSkillCount}`,
    `- Oracle batches: ${report.totals.oracleBatchCount}`,
    "",
    "## Surface Contracts",
    "",
    "| SurfaceKind | AppView variants | Family | Focus | Keyboard | Proof | Visual |",
    "| --- | --- | --- | --- | --- | --- | --- |",
    ...report.surfaceContracts.map((entry) =>
      `| ${entry.surfaceKind} | ${entry.appViewVariants.join(", ")} | ${entry.vocabulary?.family ?? ""} | ${entry.focusPolicy ?? ""} | ${entry.keyboardPolicy ?? ""} | ${entry.proofPolicy ?? ""} | ${entry.visualPolicy ?? ""} |`
    ),
    "",
    "## Liquid Glass Audit Exclusions",
    "",
    "| SurfaceKind | Reason |",
    "| --- | --- |",
    ...report.liquidGlassAuditExclusions.map((entry) => `| ${entry.surfaceKind} | ${entry.reason} |`),
    "",
    "## Current Explicit Coverage",
    "",
    report.existingDevToolsCoverage.surfaceIds.join(", "),
    "",
    "## Uncovered Contract SurfaceKinds",
    "",
    report.uncoveredContractSurfaceKinds.join(", "),
    "",
    "## Feature Map",
    "",
    "| ID | Feature | Cluster | Owners |",
    "| --- | --- | --- | --- |",
    ...report.featureMap.map((entry) => `| ${entry.id} | ${entry.feature} | ${entry.cluster} | ${entry.primaryOwners.join(", ")} |`),
    "",
    "## Oracle Batches",
    "",
    "| Priority | Phase | Batch | SurfaceKinds | Feature IDs | Required primitives |",
    "| --- | --- | --- | --- | --- | --- |",
    ...report.recommendedOracleBatches.map((batch) =>
      `| ${batch.priority} | ${batch.outsideInPhase} | ${batch.name} | ${batch.surfaceKinds.join(", ")} | ${batch.featureIds.join(", ")} | ${batch.requiredDevToolsPrimitives.join(", ")} |`
    ),
  ];
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function argValue(argv: string[], flag: string): string | null {
  const index = argv.indexOf(flag);
  return index >= 0 ? argv[index + 1] ?? null : null;
}

export async function main(argv: string[] = Bun.argv.slice(2)) {
  const subcommand = argv[0] && !argv[0].startsWith("--") ? argv[0] : "report";

  if (subcommand === "report") {
    const pipeline = await runBindingsPipeline();
    const maintainedFeatureAtlasExists = await Bun.file(new URL(paths.maintainedFeatureAtlas, root)).exists();
    const featureMapText = (await readTextIfExists(paths.featureMap)) ?? "";
    const report = buildSurfaceReport({ pipeline, maintainedFeatureAtlasExists, featureMapText });
    if (argv.includes("--markdown")) {
      console.log(markdown(report));
    } else {
      console.log(JSON.stringify(report, null, 2));
    }
    return;
  }

  if (subcommand === "bindings") {
    const out = argValue(argv, "--out") ?? ".artifacts/consistency/PF-009/coverage-bindings.json";
    const negativeDir = argValue(argv, "--negative-dir") ?? ".artifacts/consistency/PF-009/negative";
    const pipeline = await runBindingsPipeline();
    const negatives = runCoverageNegativeControls(pipeline.build.set, negativeDir);
    const receipt = buildBindingsReceipt(pipeline, negatives);
    emitValidatedReceipt("devtools.coverage.bindings", receipt, out);
    return;
  }

  if (subcommand === "scorecard") {
    const receiptsRoot = argValue(argv, "--receipts");
    if (!receiptsRoot) {
      console.error("Usage: surfaces.ts scorecard --receipts <directory> [--source-sha <sha>] [--binary-sha <sha>]");
      process.exitCode = 64;
      return;
    }
    const pipeline = await runBindingsPipeline();
    const scorecard = buildRuntimeCoverageScorecard(
      pipeline.build.set.bindings,
      discoverRuntimeCoverageReceipts(receiptsRoot),
      {
        sourceCommit: argValue(argv, "--source-sha"),
        binarySha256: argValue(argv, "--binary-sha"),
        ownerValidationErrors: validateCoverageProfiles(),
      },
    );
    console.log(JSON.stringify(scorecard, null, 2));
    if (scorecard.disposition !== "EVALUABLE_PASS") {
      process.exitCode = 3;
    }
    return;
  }

  if (subcommand === "validate-bindings") {
    const input = argValue(argv, "--input");
    if (!input) {
      console.error("Usage: surfaces.ts validate-bindings --input <path>");
      process.exitCode = 2;
      return;
    }
    const prepared = validateReceiptFile("devtools.coverage.bindings", input);
    console.log(JSON.stringify(
      {
        schemaVersion: 1,
        tool: "script-kit-devtools.surfaces",
        command: "surfaces.validate-bindings",
        input,
        disposition: prepared.receipt.disposition,
        pass: prepared.receipt.pass,
        producerValidation: prepared.receipt.producerValidation,
      },
      null,
      2,
    ));
    process.exitCode = prepared.exitCode;
    return;
  }

  if (subcommand === "verify-negative-controls") {
    const outDir = argValue(argv, "--out-dir") ?? ".artifacts/consistency/PF-009/negative";
    const pipeline = await runBindingsPipeline();
    const negatives = runCoverageNegativeControls(pipeline.build.set, outDir);
    const pass = negatives.every((control) => control.pass);
    console.log(JSON.stringify(
      {
        schemaVersion: 1,
        tool: "script-kit-devtools.surfaces",
        command: "surfaces.verify-negative-controls",
        pass,
        negativeControls: negatives,
      },
      null,
      2,
    ));
    if (!pass) process.exitCode = 2;
    return;
  }

  console.error(`Unknown subcommand: ${subcommand}`);
  process.exitCode = 2;
}

if (import.meta.main) {
  await main();
}

#!/usr/bin/env bun
/**
 * Verify both actually completed Agent Chat facade migrations.
 *
 * The conversation-style source graph and bounded popup-window owner inventory
 * must both pass. This static evidence observes Rust module/import ownership,
 * persisted generated token paths, and source identity; it never claims
 * application-runtime or generated-exporter-byte proof.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import {
  attachFacadeMigrationScope,
  POPUP_WINDOW_FACADE,
  REQUIRED_FACADE_SOURCE_PATHS,
} from "./facade-migrations.ts";

export const CANONICAL_CONVERSATION_OWNER =
  "src/components/conversation_style.rs";
export const REMOVED_CONVERSATION_FACADE =
  "src/ai/agent_chat/ui/style_contract.rs";
export const PERSISTED_CONVERSATION_CONTRACT =
  "design/mockups/generated/tokens.json";

const requiredProductionConsumers = [
  "src/ai/agent_chat/ui/components/transcript.rs",
  "src/prompts/chat/render_turns.rs",
  "src/design_contract/mod.rs",
  "src/components/conversation_text.rs",
] as const;

/**
 * Named production owners, consumers, retained popup policy, and exporter.
 * This inventory is intentionally bounded and never scans the repository.
 */
export const CANONICAL_MIGRATION_SOURCE_PATHS: readonly string[] = [
  ...new Set([
    ...REQUIRED_FACADE_SOURCE_PATHS,
    ...requiredProductionConsumers,
    "src/bin/export_design_tokens.rs",
  ]),
].sort((left, right) => left.localeCompare(right));

export type RustMigrationSource = { path: string; source: string };

export interface ConversationMigrationInventoryOptions {
  refresh?: boolean;
  sourceExists?: (path: string) => boolean;
  readSource?: (path: string) => string;
  /**
   * A deliberately poisoned injectable legacy runner. Named-source discovery
   * never needs it; negative controls prove it cannot spawn rg or another CLI.
   */
  externalRunner?: (argv: readonly string[]) => unknown;
}

let cachedConversationMigrationSources: RustMigrationSource[] | undefined;

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

/** Preserve line boundaries while excluding comments and ordinary string literals. */
function rustCode(source: string): string {
  const chars = [...source];
  const mask = (start: number, end: number) => {
    for (let index = start; index < end; index += 1) {
      if (chars[index] !== "\n") chars[index] = " ";
    }
  };
  let index = 0;
  while (index < chars.length) {
    if (chars[index] === "/" && chars[index + 1] === "/") {
      const start = index;
      while (index < chars.length && chars[index] !== "\n") index += 1;
      mask(start, index);
      continue;
    }
    if (chars[index] === "/" && chars[index + 1] === "*") {
      const start = index;
      let depth = 1;
      index += 2;
      while (index < chars.length && depth > 0) {
        if (chars[index] === "/" && chars[index + 1] === "*") {
          depth += 1;
          index += 2;
        } else if (chars[index] === "*" && chars[index + 1] === "/") {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      mask(start, index);
      continue;
    }

    if (chars[index] === "r") {
      let opening = index + 1;
      while (chars[opening] === "#") opening += 1;
      if (chars[opening] === '"') {
        const start = index;
        const hashes = opening - index - 1;
        index = opening + 1;
        while (index < chars.length) {
          if (
            chars[index] === '"' &&
            Array.from({ length: hashes }, (_, offset) => chars[index + 1 + offset])
              .every((char) => char === "#")
          ) {
            index += hashes + 1;
            break;
          }
          index += 1;
        }
        mask(start, index);
        continue;
      }
    }

    const isQuotedChar = chars[index] === "'" &&
      (chars[index + 2] === "'" || chars[index + 1] === "\\");
    if (chars[index] === '"' || isQuotedChar) {
      const quote = chars[index];
      const start = index;
      index += 1;
      while (index < chars.length) {
        if (chars[index] === "\\") index += 2;
        else if (chars[index] === quote) {
          index += 1;
          break;
        } else index += 1;
      }
      mask(start, index);
      continue;
    }
    index += 1;
  }
  return chars.join("");
}

function inlineTestRanges(code: string): Array<{ start: number; end: number }> {
  const declaration =
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z_0-9]*\s*\{/g;
  const ranges: Array<{ start: number; end: number }> = [];
  for (const match of code.matchAll(declaration)) {
    const start = (match.index ?? 0) + match[0].length;
    let depth = 1;
    let index = start;
    while (index < code.length && depth > 0) {
      if (code[index] === "{") depth += 1;
      else if (code[index] === "}") depth -= 1;
      index += 1;
    }
    ranges.push({ start, end: index });
  }
  return ranges;
}

export function collectConversationMigrationSources(
  options: ConversationMigrationInventoryOptions = {},
): RustMigrationSource[] {
  const customReader =
    options.sourceExists !== undefined || options.readSource !== undefined;
  if (
    options.refresh !== true &&
    !customReader &&
    cachedConversationMigrationSources !== undefined
  ) {
    return cachedConversationMigrationSources.map((source) => ({ ...source }));
  }

  const sourceExists = options.sourceExists ?? existsSync;
  const readSource = options.readSource ??
    ((path: string) => readFileSync(path, "utf8"));
  const retiredFacades = new Set([
    REMOVED_CONVERSATION_FACADE,
    POPUP_WINDOW_FACADE,
  ]);
  const sources: RustMigrationSource[] = [];
  for (const path of CANONICAL_MIGRATION_SOURCE_PATHS) {
    if (!sourceExists(path)) {
      if (retiredFacades.has(path)) continue;
      throw new Error("missing named facade migration source: " + path);
    }
    sources.push({ path, source: readSource(path) });
  }
  if (!customReader) {
    cachedConversationMigrationSources = sources;
  }
  return sources.map((source) => ({ ...source }));
}

export function inspectConversationFacadeMigration(
  sources: readonly RustMigrationSource[],
  persistedBundle: Record<string, unknown>,
  options: { facadeExists?: boolean; canonicalOwnerExists?: boolean } = {},
) {
  const facadeExists = options.facadeExists ?? existsSync(REMOVED_CONVERSATION_FACADE);
  const ownerExists = options.canonicalOwnerExists ?? existsSync(CANONICAL_CONVERSATION_OWNER);
  const staleProductionReferences: string[] = [];
  const staleTestReferences: string[] = [];
  const unboundLocalAliases: string[] = [];
  const reintroducedModuleDeclarations: string[] = [];
  const canonicalProductionConsumers = new Set<string>();
  const canonicalTestConsumers = new Set<string>();
  let canonicalModuleRegistered = false;

  for (const { path, source } of sources) {
    const code = rustCode(source);
    const isTestFile = path.startsWith("tests/") || path.endsWith("/tests.rs");
    const testRanges = inlineTestRanges(code);
    const inTestRegion = (offset: number) =>
      isTestFile || testRanges.some(({ start, end }) => offset >= start && offset < end);
    if (path === "src/components/mod.rs") {
      canonicalModuleRegistered = /\bpub\s+mod\s+conversation_style\s*;/.test(code);
    }
    if (/\b(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+style_contract\s*;/.test(code)) {
      reintroducedModuleDeclarations.push(path);
    }

    const legacyQualified =
      /\b(?:crate|self|super)(?:\s*::\s*[A-Za-z_][A-Za-z_0-9]*)*\s*::\s*style_contract\b/g;
    for (const match of code.matchAll(legacyQualified)) {
      const inTest = inTestRegion(match.index ?? 0);
      (inTest ? staleTestReferences : staleProductionReferences).push(path);
    }

    const canonical = /\bcrate\s*::\s*components\s*::\s*conversation_style\b/g;
    for (const match of code.matchAll(canonical)) {
      const inTest = inTestRegion(match.index ?? 0);
      (inTest ? canonicalTestConsumers : canonicalProductionConsumers).add(path);
    }

    const usesStyleAlias = /\bstyle_contract\s*::/.test(code);
    const bindsCanonicalAlias =
      /\buse\s+crate\s*::\s*components\s*::\s*conversation_style\s*(?:::)?\s*\{[^;]*\bself\s+as\s+style_contract\b[^;]*\}\s*;/.test(code) ||
      /\buse\s+crate\s*::\s*components\s*::\s*conversation_style\s+as\s+style_contract\s*;/.test(code);
    if (usesStyleAlias && !bindsCanonicalAlias) {
      unboundLocalAliases.push(path);
    }
  }

  const tokenRecords = persistedBundle.tokens &&
      typeof persistedBundle.tokens === "object" &&
      !Array.isArray(persistedBundle.tokens)
    ? Object.entries(persistedBundle.tokens as Record<string, unknown>)
    : [];
  const canonicalPersistedTokens: string[] = [];
  const legacyPersistedTokens: string[] = [];
  for (const [id, raw] of tokenRecords) {
    const record = raw && typeof raw === "object" && !Array.isArray(raw)
      ? raw as Record<string, unknown>
      : {};
    const path = typeof record.rustPath === "string" ? record.rustPath : "";
    if (/\bstyle_contract\s*::/.test(path)) legacyPersistedTokens.push(id);
    if (/\bconversation_style\s*::/.test(path)) canonicalPersistedTokens.push(id);
  }

  const missingProductionConsumers = requiredProductionConsumers
    .filter((path) => !canonicalProductionConsumers.has(path));
  const assertions = {
    allFacadesValueFree: !facadeExists && reintroducedModuleDeclarations.length === 0,
    allProductionCallersMigrated:
      ownerExists && canonicalModuleRegistered &&
      missingProductionConsumers.length === 0 &&
      staleProductionReferences.length === 0 &&
      unboundLocalAliases.length === 0,
    allTestCallersMigrated:
      canonicalTestConsumers.size > 0 && staleTestReferences.length === 0,
    zeroCallerFacadesRemoved:
      !facadeExists && staleProductionReferences.length === 0 &&
      staleTestReferences.length === 0 && unboundLocalAliases.length === 0,
    persistedNamesLiveAtCanonicalOwnersOnly:
      canonicalPersistedTokens.length > 0 && legacyPersistedTokens.length === 0,
  };
  const pass = Object.values(assertions).every(Boolean);
  return {
    schemaVersion: 1,
    generatedBy: "scripts/devtools/facade-ledger.ts",
    taskId: "GOV-002",
    evidenceClass: "STATIC_INVENTORY",
    provesRuntimeBehavior: false,
    canonicalOwner: CANONICAL_CONVERSATION_OWNER,
    removedFacade: REMOVED_CONVERSATION_FACADE,
    sourceCorpus: {
      coverageMode: "BOUNDED_CANONICAL_OWNERS_AND_CONSUMERS",
      sourceGraphExhaustive: false,
      sourceDiscovery: "DECLARED_PATHS_ONLY",
      externalProcessesStarted: 0,
      inspectedRustFileCount: sources.length,
      sourceFingerprints: Object.fromEntries(
        sources.map(({ path, source }) => [path, sha256(source)]),
      ),
      canonicalProductionConsumers: [...canonicalProductionConsumers].sort(),
      canonicalTestConsumers: [...canonicalTestConsumers].sort(),
      missingProductionConsumers,
      staleProductionReferences,
      staleTestReferences,
      unboundLocalAliases,
      reintroducedModuleDeclarations,
      canonicalModuleRegistered,
      facadeFileExists: facadeExists,
    },
    persistedNames: {
      generatedArtifactPath: PERSISTED_CONVERSATION_CONTRACT,
      canonicalTokenCount: canonicalPersistedTokens.length,
      canonicalTokenIds: canonicalPersistedTokens,
      legacyTokenIds: legacyPersistedTokens,
    },
    assertions,
    disposition: pass ? "EVALUABLE_PASS" : "EVALUABLE_FAIL",
    pass,
  };
}

export function inspectCurrentConversationFacadeMigration() {
  return inspectConversationFacadeMigration(
    collectConversationMigrationSources(),
    JSON.parse(readFileSync(PERSISTED_CONVERSATION_CONTRACT, "utf8")),
  );
}

/**
 * Inventory only the exact owners/consumers necessary for the second removed
 * facade.  This stays capture-free and does not spawn rg, cargo, or the app.
 */
async function auditBothRequiredFacadeMigrations() {
  const { existsSync, readFileSync } = await import("node:fs");
  const { resolve } = await import("node:path");
  const {
    auditFacadeMigrationScope,
    validateCompleteFacadeMigrationScope,
    validateFacadeMigrationSourceIdentity,
  } = await import("./facade-migrations");

  const paths = CANONICAL_MIGRATION_SOURCE_PATHS;
  const sources = paths.map((path) => {
    const absolutePath = resolve(process.cwd(), path);
    return {
      path,
      content: existsSync(absolutePath)
        ? readFileSync(absolutePath, "utf8")
        : undefined,
    };
  });
  const tokenFile = resolve(
    process.cwd(),
    "design/mockups/generated/tokens.json",
  );
  const tokens: unknown = JSON.parse(readFileSync(tokenFile, "utf8"));
  const persistedTokenPaths: string[] = [];
  const visit = (value: unknown): void => {
    if (typeof value === "string") {
      if (value.includes("::")) persistedTokenPaths.push(value);
      return;
    }
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
      return;
    }
    if (value !== null && typeof value === "object") {
      for (const child of Object.values(value)) visit(child);
    }
  };
  visit(tokens);

  const scope = auditFacadeMigrationScope(sources, persistedTokenPaths);
  const structuralFailures = validateCompleteFacadeMigrationScope(scope);
  if (structuralFailures.length > 0) {
    throw new Error(
      "GOV-002 complete facade ledger is invalid: " +
        structuralFailures.join(", "),
    );
  }
  const identityFailures = validateFacadeMigrationSourceIdentity(scope, sources);
  if (identityFailures.length > 0) {
    throw new Error(
      "GOV-002 facade source identity drifted: " +
        identityFailures.join(", "),
    );
  }
  if (scope.disposition !== "REMOVED") {
    throw new Error(`GOV-002 facade migration is incomplete: ${scope.failures.join(", ")}`);
  }
  return scope;
}

export async function inspectCurrentFacadeMigrations() {
  const scope = await auditBothRequiredFacadeMigrations();
  const conversationLedger = inspectCurrentConversationFacadeMigration();
  if (!conversationLedger.pass) {
    return {
      ...conversationLedger,
      provesExporterByteEquality: false,
      facadeMigrations: scope,
      facades: scope.facades,
    };
  }
  return attachFacadeMigrationScope(
    {
      ...conversationLedger,
      provesExporterByteEquality: false,
    },
    scope,
  );
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.log(
      "Usage: bun scripts/devtools/facade-ledger.ts [--out .artifacts/consistency/GOV-002/facade-ledger.json]",
    );
    process.exit(0);
  }
  const output = args[0] === "--out" ? args[1] : null;
  if (args.length !== (output === null ? 0 : 2)) {
    console.error("only optional --out .artifacts/consistency/GOV-002/facade-ledger.json is supported");
    process.exit(64);
  }
  // Parsing help and usage stays passive. A one-facade ledger is never
  // generated: exact-file popup scope and full conversation scope both pass.
  const ledger = await inspectCurrentFacadeMigrations();
  if (!ledger.pass) {
    console.error(JSON.stringify(ledger, null, 2));
    process.exit(2);
  }
  if (output !== null) {
    const expected = resolve(".artifacts/consistency/GOV-002/facade-ledger.json");
    if (resolve(output) !== expected) {
      console.error("facade ledger may write only .artifacts/consistency/GOV-002/facade-ledger.json");
      process.exit(64);
    }
    mkdirSync(dirname(expected), { recursive: true });
    writeFileSync(expected, `${JSON.stringify(ledger, null, 2)}\n`);
  }
  console.log(JSON.stringify(ledger, null, 2));
}

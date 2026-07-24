#!/usr/bin/env bun
/**
 * Generate the agent-readable launcher surface contract matrix.
 *
 * The source of truth is the typed Rust registry in
 * src/main_sections/app_view_state.rs. This script intentionally parses the
 * `AppView::surface_kind()` and `SurfaceKind::surface_contract()` matches
 * instead of maintaining a parallel hand-written matrix.
 *
 * Usage:
 *   bun scripts/generate-surface-contracts.ts --write
 *   bun scripts/generate-surface-contracts.ts --check
 *   bun scripts/generate-surface-contracts.ts --stdout
 */

import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

const PROJECT_ROOT = resolve(import.meta.dir, "..");
const SOURCE_PATH = "src/main_sections/app_view_state.rs";
const OUTPUT_PATH = "docs/ai/contracts/surface-contracts.json";
const SCHEMA_VERSION = 1;

type DismissPolicyName = "standard" | "explicit";

interface SurfaceContractEntry {
  surfaceKind: string;
  appViewVariants: string[];
  appViewFooters: Array<{
    variant: string;
    nativeFooterSurface: string | null;
  }>;
  vocabulary: {
    family: string;
    inputOwnership: string;
    previewRole: string;
  };
  focusPolicy: string;
  keyboardPolicy: string;
  actionsPolicy: string;
  proofPolicy: string;
  visualPolicy: string;
  dismissPolicy: {
    policy: DismissPolicyName;
    windowBlur: string;
    backdropClick: string;
    escape: string;
    cmdW: string;
  };
  automationSemanticSurface: string;
}

interface SurfaceContractMatrix {
  schemaVersion: number;
  generatedFrom: string;
  registry: string;
  entries: SurfaceContractEntry[];
}

function sourceBetween(source: string, start: string, end: string): string {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    throw new Error(`Missing start marker: ${start}`);
  }
  const afterStart = source.slice(startIndex);
  const endIndex = afterStart.indexOf(end);
  if (endIndex < 0) {
    throw new Error(`Missing end marker after ${start}: ${end}`);
  }
  return afterStart.slice(0, endIndex);
}

function parseSurfaceKinds(source: string): string[] {
  const enumBody = sourceBetween(source, "pub(crate) enum SurfaceKind {", "}\n\n/// First-pass vocabulary");
  return enumBody
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.endsWith(","))
    .filter((line) => !line.startsWith("#["))
    .map((line) => line.replace(/,$/, ""))
    .filter((line) => /^[A-Za-z][A-Za-z0-9_]*$/.test(line));
}

function parseAppViewVariantsByKind(source: string): Map<string, string[]> {
  const body = sourceBetween(
    source,
    "pub(crate) fn surface_kind(&self) -> SurfaceKind",
    "/// Exhaustive behavior contract for every top-level launcher view.",
  );
  const result = new Map<string, string[]>();
  const armRegex = /([\s\S]*?)=>\s*\{?\s*SurfaceKind::([A-Za-z0-9_]+)/g;
  let match: RegExpExecArray | null;
  while ((match = armRegex.exec(body)) !== null) {
    const armSource = match[1] ?? "";
    const kind = match[2] ?? "";
    const variants = [...armSource.matchAll(/AppView::([A-Za-z0-9_]+)/g)].map(
      (variantMatch) => variantMatch[1],
    );
    if (variants.length === 0) {
      continue;
    }
    const existing = result.get(kind) ?? [];
    result.set(kind, [...new Set([...existing, ...variants])]);
  }
  return result;
}

function parseNativeFooterSurfaceByVariant(source: string): Map<string, string | null> {
  const body = sourceBetween(
    source,
    "pub(crate) fn native_footer_surface(&self) -> Option<&'static str>",
    "}\n}\n\nimpl SurfaceKind",
  );
  const result = new Map<string, string | null>();
  const armRegex = /([\s\S]*?)=>\s*(Some\("([^"]+)"\)|None)/g;
  let match: RegExpExecArray | null;
  while ((match = armRegex.exec(body)) !== null) {
    const armSource = match[1] ?? "";
    const footer = match[3] ?? null;
    for (const variantMatch of armSource.matchAll(/AppView::([A-Za-z0-9_]+)/g)) {
      const variant = variantMatch[1];
      if (variant) {
        result.set(variant, footer);
      }
    }
  }
  return result;
}

function surfaceKindArms(source: string): Array<{ kind: string; body: string }> {
  const body = sourceBetween(
    source,
    "pub(crate) fn surface_contract(self) -> LauncherSurfaceContract",
    "/// Map an [`AppView`] variant to the automation",
  );
  const markers = [...body.matchAll(/SurfaceKind::([A-Za-z0-9_]+)\s*=>/g)].map((match) => ({
    kind: match[1] ?? "",
    index: match.index ?? 0,
  }));
  return markers.map((marker, index) => {
    const next = markers[index + 1]?.index ?? body.length;
    return {
      kind: marker.kind,
      body: body.slice(marker.index, next),
    };
  });
}

function dismissPolicy(token: string): SurfaceContractEntry["dismissPolicy"] {
  if (token === "standard") {
    return {
      policy: "standard",
      windowBlur: "CloseMainWindow",
      backdropClick: "CloseMainWindow",
      escape: "CloseMainWindow",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "explicit") {
    return {
      policy: "explicit",
      windowBlur: "Ignore",
      backdropClick: "Ignore",
      escape: "LetViewHandle",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "DismissPolicy::cancel_to_script_prompt()") {
    return {
      policy: "cancelToScriptPrompt",
      windowBlur: "CloseMainWindow",
      backdropClick: "CloseMainWindow",
      escape: "LetViewHandle",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "DismissPolicy::sticky_escape_closes()") {
    return {
      policy: "stickyEscapeCloses",
      windowBlur: "Ignore",
      backdropClick: "Ignore",
      escape: "CloseMainWindow",
      cmdW: "CloseMainWindow",
    };
  }
  if (token === "DismissPolicy::blur_closes_escape_view_owned()") {
    return {
      policy: "blurClosesEscapeViewOwned",
      windowBlur: "CloseMainWindow",
      backdropClick: "CloseMainWindow",
      escape: "LetViewHandle",
      cmdW: "CloseMainWindow",
    };
  }
  throw new Error(`Unknown dismiss policy token: ${token}`);
}

function parseContractMatrix(): SurfaceContractMatrix {
  const source = readFileSync(resolve(PROJECT_ROOT, SOURCE_PATH), "utf8");
  const surfaceKinds = parseSurfaceKinds(source);
  const appViewVariantsByKind = parseAppViewVariantsByKind(source);
  const nativeFooterSurfaceByVariant = parseNativeFooterSurfaceByVariant(source);
  const arms = surfaceKindArms(source);

  const entries = arms.map(({ kind, body }) => {
    const vocabulary = body.match(
      /LauncherSurfaceContractVocabulary::new\(\s*([A-Za-z0-9_]+),\s*([A-Za-z0-9_]+),\s*([A-Za-z0-9_]+),?\s*\)/,
    );
    if (!vocabulary) {
      throw new Error(`Missing vocabulary tuple for SurfaceKind::${kind}`);
    }
    const policyAndSurface = body.match(
      /\)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*([A-Za-z0-9_]+)\s*,\s*(standard|explicit|DismissPolicy::[A-Za-z0-9_]+\(\))\s*,\s*"([^"]+)"/,
    );
    if (!policyAndSurface) {
      throw new Error(
        `Missing focus, keyboard, actions, proof, visual, dismiss policy, or semantic surface for SurfaceKind::${kind}`,
      );
    }
    return {
      surfaceKind: kind,
      appViewVariants: appViewVariantsByKind.get(kind) ?? [],
      appViewFooters: (appViewVariantsByKind.get(kind) ?? []).map((variant) => ({
        variant,
        nativeFooterSurface: nativeFooterSurfaceByVariant.get(variant) ?? null,
      })),
      vocabulary: {
        family: vocabulary[1] ?? "",
        inputOwnership: vocabulary[2] ?? "",
        previewRole: vocabulary[3] ?? "",
      },
      focusPolicy: policyAndSurface[1] ?? "",
      keyboardPolicy: policyAndSurface[2] ?? "",
      actionsPolicy: policyAndSurface[3] ?? "",
      proofPolicy: policyAndSurface[4] ?? "",
      visualPolicy: policyAndSurface[5] ?? "",
      dismissPolicy: dismissPolicy(policyAndSurface[6] ?? ""),
      automationSemanticSurface: policyAndSurface[7] ?? "",
    };
  });

  const missingContract = surfaceKinds.filter(
    (kind) => !entries.some((entry) => entry.surfaceKind === kind),
  );
  if (missingContract.length > 0) {
    throw new Error(`SurfaceKind contract entries missing: ${missingContract.join(", ")}`);
  }

  const missingIdentity = entries.filter((entry) => entry.appViewVariants.length === 0);
  if (missingIdentity.length > 0) {
    throw new Error(
      `SurfaceKind identity entries missing AppView variants: ${missingIdentity
        .map((entry) => entry.surfaceKind)
        .join(", ")}`,
    );
  }

  const missingFooter = entries
    .flatMap((entry) => entry.appViewVariants)
    .filter((variant) => !nativeFooterSurfaceByVariant.has(variant));
  if (missingFooter.length > 0) {
    throw new Error(
      `AppView native footer entries missing: ${[...new Set(missingFooter)].join(", ")}`,
    );
  }

  return {
    schemaVersion: SCHEMA_VERSION,
    generatedFrom: SOURCE_PATH,
    registry: "AppView::surface_kind -> SurfaceKind::surface_contract",
    entries,
  };
}

function renderJson(matrix: SurfaceContractMatrix): string {
  return `${JSON.stringify(matrix, null, 2)}\n`;
}

function hasFlag(flag: string): boolean {
  return process.argv.includes(flag);
}

const output = renderJson(parseContractMatrix());
const outputPath = resolve(PROJECT_ROOT, OUTPUT_PATH);

if (hasFlag("--stdout")) {
  process.stdout.write(output);
} else if (hasFlag("--check")) {
  const current = readFileSync(outputPath, "utf8");
  if (current !== output) {
    throw new Error(`${OUTPUT_PATH} is stale. Run: bun scripts/generate-surface-contracts.ts --write`);
  }
} else if (hasFlag("--write")) {
  writeFileSync(outputPath, output);
} else {
  process.stderr.write("Usage: bun scripts/generate-surface-contracts.ts --write|--check|--stdout\n");
  process.exit(2);
}

#!/usr/bin/env bun
/**
 * Pure inventory of protected consistency-program sources.
 *
 * A matching byte hash proves only that these exact files have not drifted
 * since the inventory was generated. It never proves an application run,
 * visible motion, exporter regeneration, or completed user interaction.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";

export const LOCKED_GLASS_SOURCE_PATHS = [
  "src/theme/opacity.rs",
  "src/platform/secondary_window_config.rs",
  "src/platform/secondary_window_config_behavior_tests.rs",
  "src/platform/secondary_window_glass_animation.rs",
  "src/platform/secondary_window_glass_backdrop.rs",
  "src/platform/secondary_window_glass_lifecycle.rs",
  "src/platform/secondary_window_glass_style.rs",
  "src/platform/secondary_window_resize_policy.rs",
  "src/platform/secondary_window_vibrancy_impl.rs",
  "src/footer_popup.rs",
  "src/footer_popup_fidelity.rs",
  "src/footer_popup_glass_geometry.rs",
  "src/footer_popup_native_dispatch.rs",
  "src/footer_popup_native_layout.rs",
  "src/ui/chrome/tokens.rs",
  "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
] as const;

export const REQUIRED_PROTECTED_SOURCE_PATHS = [
  "AGENTS.md",
  "scripts/devtools/consistency-catalog.md",
  ...LOCKED_GLASS_SOURCE_PATHS,
] as const;

type ProtectedPath = { path: string; sha256: string };

function sourceHash(path: string): string | null {
  try {
    return createHash("sha256").update(readFileSync(path)).digest("hex");
  } catch {
    return null;
  }
}

function reviewedRelativePath(path: string): boolean {
  if (path.length === 0 || path.includes("\\")) return false;
  const relationship = relative(process.cwd(), resolve(path));
  return relationship === path && relationship !== "" &&
    !relationship.startsWith("..") && existsSync(path);
}

export function createProtectedSourceManifest(
  paths: readonly string[] = REQUIRED_PROTECTED_SOURCE_PATHS,
) {
  const protectedPaths = paths.map((path) => ({
    path,
    sha256: sourceHash(path),
  }));
  return {
    schemaVersion: 1,
    generatedBy: "scripts/devtools/protected-sources.ts",
    evidenceClass: "STATIC_INVENTORY",
    provesRuntimeBehavior: false,
    provesExporterByteEquality: false,
    protectedPaths,
  };
}

export function validateProtectedSourceManifest(
  candidate: Record<string, unknown>,
  options: {
    requiredPaths?: readonly string[];
    currentHash?: (path: string) => string | null;
  } = {},
) {
  const requiredPaths = options.requiredPaths ?? REQUIRED_PROTECTED_SOURCE_PATHS;
  const hash = options.currentHash ?? sourceHash;
  const entries = Array.isArray(candidate.protectedPaths)
    ? candidate.protectedPaths
    : [];
  const errors: string[] = [];
  if (
    candidate.schemaVersion !== 1 ||
    candidate.evidenceClass !== "STATIC_INVENTORY" ||
    candidate.provesRuntimeBehavior !== false ||
    candidate.provesExporterByteEquality !== false
  ) {
    errors.push("protected source inventory must remain static and claim no runtime or exporter proof");
  }
  const identities = new Set<string>();
  for (const value of entries) {
    const entry = value && typeof value === "object" && !Array.isArray(value)
      ? value as Partial<ProtectedPath>
      : {};
    const path = typeof entry.path === "string" ? entry.path : "";
    if (!reviewedRelativePath(path)) {
      errors.push(`protected source path is missing, absolute, or escaping: ${path}`);
      continue;
    }
    if (identities.has(path)) {
      errors.push(`duplicate protected source identity: ${path}`);
    }
    identities.add(path);
    if (!/^[a-f0-9]{64}$/.test(String(entry.sha256 ?? ""))) {
      errors.push(`protected source hash is absent or malformed: ${path}`);
    } else if (hash(path) !== entry.sha256) {
      errors.push(`protected source drifted since observation: ${path}`);
    }
  }
  for (const required of requiredPaths) {
    if (!identities.has(required)) {
      errors.push(`required protected source omitted: ${required}`);
    }
  }
  return { pass: errors.length === 0, protectedPathCount: entries.length, errors };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.log(
      "Usage: bun scripts/devtools/protected-sources.ts [--out .artifacts/consistency/run.json]",
    );
    process.exit(0);
  }
  const output = args[0] === "--out" ? args[1] : null;
  if (args.length !== (output === null ? 0 : 2)) {
    console.error("only optional --out .artifacts/consistency/run.json is supported");
    process.exit(64);
  }
  const manifest = createProtectedSourceManifest();
  const validation = validateProtectedSourceManifest(manifest);
  if (!validation.pass) {
    console.error(JSON.stringify(validation, null, 2));
    process.exit(2);
  }
  if (output !== null) {
    const expected = resolve(".artifacts/consistency/run.json");
    if (resolve(output) !== expected) {
      console.error("protected source inventory may write only .artifacts/consistency/run.json");
      process.exit(64);
    }
    mkdirSync(dirname(expected), { recursive: true });
    writeFileSync(expected, `${JSON.stringify(manifest, null, 2)}\n`);
  }
  console.log(JSON.stringify({ ...manifest, validation }, null, 2));
}

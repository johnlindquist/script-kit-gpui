import { describe, expect, test } from "bun:test";
import {
  createProtectedSourceManifest,
  REQUIRED_PROTECTED_SOURCE_PATHS,
  validateProtectedSourceManifest,
} from "./protected-sources.ts";

describe("static protected consistency-source inventory", () => {
  test("observes every real locked glass owner and its current SHA-256", () => {
    const manifest = createProtectedSourceManifest();
    const validation = validateProtectedSourceManifest(manifest);
    expect(validation.pass).toBe(true);
    expect(validation.protectedPathCount).toBe(REQUIRED_PROTECTED_SOURCE_PATHS.length);
    expect(manifest.evidenceClass).toBe("STATIC_INVENTORY");
    expect(manifest.provesRuntimeBehavior).toBe(false);
    expect(manifest.provesExporterByteEquality).toBe(false);
    expect(manifest.protectedPaths.map((entry) => entry.path)).toContain(
      "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
    );
  });

  test("missing, duplicate, absolute, and escaping protected owners fail closed", () => {
    const baseline = createProtectedSourceManifest();
    const missing = {
      ...baseline,
      protectedPaths: baseline.protectedPaths.slice(1),
    };
    expect(validateProtectedSourceManifest(missing).errors[0]).toContain(
      "required protected source omitted",
    );
    const duplicate = {
      ...baseline,
      protectedPaths: [...baseline.protectedPaths, baseline.protectedPaths[0]],
    };
    expect(validateProtectedSourceManifest(duplicate).errors[0]).toContain(
      "duplicate protected source identity",
    );
    for (const path of ["/tmp/external-source.rs", "../outside.rs"]) {
      const escaped = {
        ...baseline,
        protectedPaths: [...baseline.protectedPaths, {
          path,
          sha256: "a".repeat(64),
        }],
      };
      expect(validateProtectedSourceManifest(escaped).errors[0]).toContain(
        "protected source path is missing, absolute, or escaping",
      );
    }
  });

  test("drifted hashes and fake runtime or exporter assertions are rejected", () => {
    const baseline = createProtectedSourceManifest();
    const drift = validateProtectedSourceManifest(baseline, {
      currentHash: (path) =>
        path === REQUIRED_PROTECTED_SOURCE_PATHS[0]
          ? "a".repeat(64)
          : baseline.protectedPaths.find((entry) => entry.path === path)?.sha256 ?? null,
    });
    expect(drift.pass).toBe(false);
    expect(drift.errors[0]).toContain("drifted since observation");
    for (const override of [
      { evidenceClass: "RUNTIME_HIDDEN" },
      { provesRuntimeBehavior: true },
      { provesExporterByteEquality: true },
    ]) {
      expect(validateProtectedSourceManifest({ ...baseline, ...override }).pass).toBe(false);
    }
  });
});

import { describe, expect, test } from "bun:test";
import {
  buildCoverageReport,
  coverageProfileById,
  coverageProfiles,
  validateCoverageProfiles,
  type CoverageProfile,
} from "./coverage.ts";

function mainProfile(changes: Partial<CoverageProfile> = {}): CoverageProfile {
  const profile = coverageProfileById("main");
  if (!profile) throw new Error("main coverage profile is required");
  return { ...profile, ...changes };
}

describe("fail-closed coverage ownership registry", () => {
  test("every tracked coverage owner resolves inside the repository", () => {
    expect(validateCoverageProfiles(coverageProfiles)).toEqual([]);
    expect(mainProfile().sourceFiles).toEqual([
      "src/main_sections/render_impl.rs",
      "src/main_sections/app_state.rs",
      "src/render_script_list/mod.rs",
    ]);
    expect(coverageProfileById("dictation-history")?.sourceFiles).toContain(
      "src/mcp_resources/mod.rs",
    );
  });

  test("a stale source owner invalidates its profile", () => {
    const errors = validateCoverageProfiles([
      mainProfile({ sourceFiles: ["src/does-not-exist.rs"] }),
    ]);
    expect(errors).toContain(
      "profile main references missing source owner: src/does-not-exist.rs",
    );
  });

  test("empty, duplicate, absolute, and escaping owners fail closed", () => {
    expect(validateCoverageProfiles([mainProfile({ sourceFiles: [] })])).toContain(
      "profile main has no source owners",
    );
    expect(validateCoverageProfiles([mainProfile({ sourceFiles: [""] })])).toContain(
      "profile main has an empty source owner",
    );
    expect(
      validateCoverageProfiles([
        mainProfile({
          sourceFiles: [
            "src/main_sections/render_impl.rs",
            "src/main_sections/../main_sections/render_impl.rs",
          ],
        }),
      ]),
    ).toContain(
      "profile main lists source owner twice: src/main_sections/../main_sections/render_impl.rs",
    );
    expect(
      validateCoverageProfiles([mainProfile({ sourceFiles: ["../outside.rs"] })]),
    ).toContain("profile main source owner escapes repository: ../outside.rs");
    expect(
      validateCoverageProfiles([mainProfile({ sourceFiles: ["/tmp/outside.rs"] })]),
    ).toContain("profile main source owner escapes repository: /tmp/outside.rs");
  });

  test("source owner validation can be deterministically fault-injected", () => {
    const errors = validateCoverageProfiles([mainProfile()], {
      ownerExists: (path) => !path.endsWith("render_script_list/mod.rs"),
    });
    expect(errors).toContain(
      "profile main references missing source owner: src/render_script_list/mod.rs",
    );
  });

  test("static Direct bindings never masquerade as runtime proof", () => {
    const report = buildCoverageReport({ surface: "agent-chat" });
    expect(report.surfaces[0]?.bindingSelectors[0]?.relation).toBe("Direct");
    expect(report.evidenceClass).toBe("STATIC_INVENTORY");
    expect(report.runtimeProof.disposition).toBe("NOT_EVALUATED");
    expect(report.runtimeProof.provenSurfaceCount).toBe(0);
    expect(report.registryValidation.errors).toEqual([]);
  });
});

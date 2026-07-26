import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  FixtureDocument,
  fixtureCoverageGaps,
  parseArgs,
  resolveSuites,
  SUITES,
  validateFixture,
} from "./window-engine-foundation";

const FIXTURE_PATH = resolve(
  import.meta.dir,
  "fixtures/window-engine-provider.v1.json",
);

function loadFixture(): FixtureDocument {
  return JSON.parse(readFileSync(FIXTURE_PATH, "utf8")) as FixtureDocument;
}

describe("provider fixture", () => {
  test("v1 fixture parses and validates with zero issues", () => {
    const fixture = loadFixture();
    expect(validateFixture(fixture)).toEqual([]);
  });

  test("v1 fixture covers every required WP-00 scenario", () => {
    const fixture = loadFixture();
    expect(fixtureCoverageGaps(fixture)).toEqual([]);
  });

  test("legacy bare-array fixtures validate", () => {
    const legacy = [{ app: "A", title: "T" }];
    expect(validateFixture(legacy)).toEqual([]);
  });

  test("duplicate window ids are reported", () => {
    const issues = validateFixture({
      windows: [
        { id: 1, app: "A", title: "One" },
        { id: 1, app: "A", title: "Two" },
      ],
    });
    expect(issues.some((issue) => issue.message.includes("duplicate window id 1"))).toBe(true);
  });

  test("dangling frontmostWindowId is reported", () => {
    const issues = validateFixture({
      windows: [{ id: 1, app: "A", title: "One" }],
      frontmostWindowId: 99,
    });
    expect(issues.some((issue) => issue.path === "$.frontmostWindowId")).toBe(true);
  });

  test("missing display fields are reported", () => {
    const issues = validateFixture({
      windows: [{ id: 1, app: "A", title: "One" }],
      displays: [{ id: 1 } as never],
    });
    expect(issues.length).toBeGreaterThan(0);
  });
});

describe("probe CLI", () => {
  test("parses suites, provider, and cycles", () => {
    const { suites, context } = parseArgs([
      "--suite",
      "identity,topology",
      "--provider",
      "fixture.json",
      "--cycles",
      "25",
    ]);
    expect(suites).toEqual(["identity", "topology"]);
    expect(context.providerPath).toBe("fixture.json");
    expect(context.cycles).toBe(25);
  });

  test("rejects unknown suites with the known list", () => {
    expect(() => resolveSuites(["nonsense"])).toThrow(/unknown suites: nonsense/);
  });

  test("all expands to the full suite registry", () => {
    expect(resolveSuites(["all"])).toEqual(Object.keys(SUITES));
  });

  test("fixture suite passes against the shipped v1 fixture", () => {
    const raw = readFileSync(FIXTURE_PATH, "utf8");
    const result = SUITES.fixture({ providerRaw: raw, cycles: 100 });
    expect(result.status).toBe("pass");
  });
});

#!/usr/bin/env bun
/**
 * Window Engine Foundation probe.
 *
 * Orchestrates deterministic-provider, native-fixture, and live-app proof for
 * the window engine foundation wave (S2..S16). Provider suites run the Rust
 * engine's provider-driven scenario tests with the JSON fixture exported via
 * SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER; native/live suites drive real
 * AppKit windows.
 *
 * Usage:
 *   bun scripts/devtools/window-engine-foundation.ts \
 *     --suite identity,topology \
 *     --provider scripts/devtools/fixtures/window-engine-provider.v1.json
 */

import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

export interface FixtureBounds {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
}

export interface FixtureWindow {
  id?: number;
  app: string;
  title: string;
  pid?: number;
  bundleId?: string;
  nativeWindowId?: number;
  role?: string;
  subrole?: string;
  bounds?: FixtureBounds;
  displayId?: number;
  minimized?: boolean;
  currentSpace?: boolean;
  focused?: boolean;
  main?: boolean;
  frontmostApp?: boolean;
  positionSettable?: boolean;
  sizeSettable?: boolean;
  minimizedSettable?: boolean;
  fullscreenSettable?: boolean;
  raiseSupported?: boolean;
  closeSupported?: boolean;
  nativeTabGroup?: string;
  mutation?: Record<string, unknown>;
}

export interface FixtureDisplay {
  id: number;
  uuid: string;
  name: string;
  fullBounds: FixtureBounds;
  visibleBounds: FixtureBounds;
  scaleFactor?: number;
  isPrimary?: boolean;
  legacyOrder?: number;
}

export interface FixtureDocument {
  windows: FixtureWindow[];
  displays?: FixtureDisplay[];
  frontmostWindowId?: number;
}

export interface FixtureIssue {
  path: string;
  message: string;
}

/** Validate a provider fixture document. Returns human-actionable issues. */
export function validateFixture(document: unknown): FixtureIssue[] {
  const issues: FixtureIssue[] = [];
  if (typeof document !== "object" || document === null) {
    return [{ path: "$", message: "fixture must be an object or array" }];
  }
  const windows: FixtureWindow[] = Array.isArray(document)
    ? (document as FixtureWindow[])
    : ((document as FixtureDocument).windows ?? []);
  if (windows.length === 0) {
    issues.push({ path: "$.windows", message: "fixture declares no windows" });
  }
  const seenIds = new Map<number, string>();
  windows.forEach((window, index) => {
    const path = `$.windows[${index}]`;
    if (typeof window.app !== "string") {
      issues.push({ path, message: "missing required string field: app" });
    }
    if (typeof window.title !== "string") {
      issues.push({ path, message: "missing required string field: title" });
    }
    const id = window.id ?? index + 1;
    const existing = seenIds.get(id);
    if (existing !== undefined) {
      issues.push({
        path,
        message: `duplicate window id ${id} (also used by "${existing}")`,
      });
    }
    seenIds.set(id, window.title ?? "");
  });
  if (!Array.isArray(document)) {
    const displays = (document as FixtureDocument).displays ?? [];
    const displayIds = new Set<number>();
    displays.forEach((display, index) => {
      const path = `$.displays[${index}]`;
      for (const field of ["id", "uuid", "name", "fullBounds", "visibleBounds"] as const) {
        if (display[field] === undefined) {
          issues.push({ path, message: `missing required field: ${field}` });
        }
      }
      if (displayIds.has(display.id)) {
        issues.push({ path, message: `duplicate display id ${display.id}` });
      }
      displayIds.add(display.id);
    });
    const frontmost = (document as FixtureDocument).frontmostWindowId;
    if (frontmost !== undefined && !seenIds.has(frontmost)) {
      issues.push({
        path: "$.frontmostWindowId",
        message: `frontmostWindowId ${frontmost} does not match any window`,
      });
    }
  }
  return issues;
}

/** Coverage checks the canonical v1 fixture must satisfy (plan WP-00). */
export function fixtureCoverageGaps(document: FixtureDocument): string[] {
  const gaps: string[] = [];
  const windows = document.windows ?? [];
  const has = (predicate: (w: FixtureWindow) => boolean, label: string) => {
    if (!windows.some(predicate)) gaps.push(label);
  };
  has((w) => (w.role ?? "AXWindow") === "AXWindow" && (w.title ?? "").length > 0, "ordinary window");
  has((w) => w.mutation !== undefined && "minWidth" in (w.mutation ?? {}), "minimum-size-constrained window");
  has((w) => w.role === "AXDialog" && (w.title ?? "") === "", "untitled dialog");
  has((w) => w.role === "AXSheet", "sheet");
  const titleCounts = new Map<string, number>();
  for (const w of windows) {
    titleCounts.set(w.title, (titleCounts.get(w.title) ?? 0) + 1);
  }
  if (![...titleCounts.entries()].some(([title, count]) => title.length > 0 && count >= 2)) {
    gaps.push("duplicate-title windows");
  }
  const tabGroups = new Map<string, number>();
  for (const w of windows) {
    if (w.nativeTabGroup) tabGroups.set(w.nativeTabGroup, (tabGroups.get(w.nativeTabGroup) ?? 0) + 1);
  }
  if (![...tabGroups.values()].some((count) => count >= 2)) {
    gaps.push("native tab group");
  }
  has((w) => (w.mutation as { delayMs?: number } | undefined)?.delayMs !== undefined, "slow responder");
  has(
    (w) => {
      const m = w.mutation as { positionDeltaX?: number; positionDeltaY?: number } | undefined;
      return (m?.positionDeltaX ?? 0) !== 0 || (m?.positionDeltaY ?? 0) !== 0;
    },
    "readback-offset window",
  );
  has(
    (w) => (w.mutation as { destroyOnAttempt?: number } | undefined)?.destroyOnAttempt !== undefined,
    "destroy-on-attempt window",
  );
  if (new Set(windows.map((w) => w.pid ?? 0)).size < 2) {
    gaps.push("two independent PIDs");
  }
  const displays = document.displays ?? [];
  if (displays.length < 2) gaps.push("two displays");
  if (!displays.some((d) => (d.fullBounds.x ?? 0) < 0)) gaps.push("negative-origin display");
  return gaps;
}

export interface SuiteResult {
  suite: string;
  status: "pass" | "fail" | "pending" | "environment-blocked";
  detail: string;
}

type SuiteRunner = (context: ProbeContext) => SuiteResult;

export interface ProbeContext {
  providerPath?: string;
  providerRaw?: string;
  nativeFixture?: string;
  apps?: string[];
  cycles: number;
  bundleId?: string;
  sequence?: string;
}

function cargoProviderTests(context: ProbeContext, module: string, label: string): SuiteResult {
  if (!context.providerRaw) {
    return { suite: label, status: "fail", detail: "--provider fixture required" };
  }
  const result = spawnSync(
    "./scripts/agentic/agent-cargo.sh",
    ["test", "--lib", module],
    {
      cwd: resolve(import.meta.dir, "../.."),
      env: {
        ...process.env,
        SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER: context.providerRaw,
      },
      encoding: "utf8",
      timeout: 600_000,
    },
  );
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  const okLine = output.match(/test result: ok\. (\d+) passed/);
  if (result.status === 0 && okLine) {
    return { suite: label, status: "pass", detail: `${okLine[1]} tests passed (${module})` };
  }
  return {
    suite: label,
    status: "fail",
    detail: `cargo test ${module} exited ${result.status}: ${output.slice(-600)}`,
  };
}

function pendingSuite(label: string, lands: string): SuiteRunner {
  return () => ({
    suite: label,
    status: "pending",
    detail: `suite lands with plan step ${lands}`,
  });
}

export const SUITES: Record<string, SuiteRunner> = {
  fixture: (context) => {
    if (!context.providerRaw) {
      return { suite: "fixture", status: "fail", detail: "--provider fixture required" };
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(context.providerRaw);
    } catch (error) {
      return { suite: "fixture", status: "fail", detail: `fixture is not JSON: ${error}` };
    }
    const issues = validateFixture(parsed);
    if (issues.length > 0) {
      return {
        suite: "fixture",
        status: "fail",
        detail: issues.map((issue) => `${issue.path}: ${issue.message}`).join("; "),
      };
    }
    const gaps = Array.isArray(parsed) ? [] : fixtureCoverageGaps(parsed as FixtureDocument);
    if (gaps.length > 0) {
      return { suite: "fixture", status: "fail", detail: `coverage gaps: ${gaps.join(", ")}` };
    }
    return { suite: "fixture", status: "pass", detail: "fixture valid with full scenario coverage" };
  },
  identity: (context) => cargoProviderTests(context, "window_control::observation", "identity"),
  topology: (context) =>
    cargoProviderTests(context, "window_control::display_topology", "topology"),
  transaction: (context) => {
    const executor = cargoProviderTests(context, "window_control::transaction", "transaction");
    if (executor.status !== "pass") return executor;
    const undo = cargoProviderTests(context, "window_control::undo", "transaction");
    if (undo.status !== "pass") return { ...undo, suite: "transaction" };
    return {
      suite: "transaction",
      status: "pass",
      detail: `${executor.detail}; ${undo.detail}`,
    };
  },
  "legacy-actions": (context) =>
    cargoProviderTests(context, "window_control::actions", "legacy-actions"),
  snap: (context) => cargoProviderTests(context, "window_control::snap", "snap"),
  protocol: pendingSuite("protocol", "S14"),
  "window-switcher": pendingSuite("window-switcher", "S14"),
  "sdk-parity": pendingSuite("sdk-parity", "S15"),
  native: pendingSuite("native", "S16"),
  "app-profiles": pendingSuite("app-profiles", "S16"),
  "rapid-cycle": pendingSuite("rapid-cycle", "S16"),
  profile: pendingSuite("profile", "S16"),
  permission: pendingSuite("permission", "S16"),
};

// Internal helper reused by later steps when suites become cargo-backed.
export const _cargoProviderTests = cargoProviderTests;

export function parseArgs(argv: string[]): { suites: string[]; context: ProbeContext } {
  const context: ProbeContext = { cycles: 100 };
  const suites: string[] = [];
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      index += 1;
      const value = argv[index];
      if (value === undefined) throw new Error(`missing value for ${arg}`);
      return value;
    };
    switch (arg) {
      case "--suite":
        suites.push(...next().split(",").map((value) => value.trim()).filter(Boolean));
        break;
      case "--provider":
        context.providerPath = next();
        break;
      case "--native-fixture":
        context.nativeFixture = next();
        break;
      case "--apps":
        context.apps = next().split(",").map((value) => value.trim()).filter(Boolean);
        break;
      case "--cycles":
        context.cycles = Number.parseInt(next(), 10);
        break;
      case "--bundle-id":
        context.bundleId = next();
        break;
      case "--sequence":
        context.sequence = next();
        break;
      default:
        throw new Error(`unknown argument: ${arg}`);
    }
  }
  return { suites, context };
}

export function resolveSuites(requested: string[]): string[] {
  if (requested.length === 1 && requested[0] === "all") {
    return Object.keys(SUITES);
  }
  const unknown = requested.filter((suite) => !(suite in SUITES));
  if (unknown.length > 0) {
    throw new Error(`unknown suites: ${unknown.join(", ")} (known: ${Object.keys(SUITES).join(", ")})`);
  }
  return requested;
}

if (import.meta.main) {
  const { suites: requested, context } = parseArgs(process.argv.slice(2));
  if (requested.length === 0) {
    console.error("usage: window-engine-foundation.ts --suite <name[,name]|all> [--provider fixture.json]");
    process.exit(2);
  }
  const suites = resolveSuites(requested);
  if (context.providerPath) {
    context.providerRaw = readFileSync(context.providerPath, "utf8");
  }
  const results = suites.map((suite) => SUITES[suite](context));
  for (const result of results) {
    console.log(JSON.stringify(result));
  }
  const failed = results.filter((result) => result.status === "fail");
  const pending = results.filter((result) => result.status === "pending");
  if (failed.length > 0) {
    console.error(`FAIL: ${failed.map((result) => result.suite).join(", ")}`);
    process.exit(1);
  }
  if (pending.length > 0) {
    console.error(`PENDING (not yet implemented): ${pending.map((result) => result.suite).join(", ")}`);
    process.exit(3);
  }
  console.log("OK");
}

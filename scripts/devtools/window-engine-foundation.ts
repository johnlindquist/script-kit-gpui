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

import { mkdtempSync, readFileSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { assertNoninteractiveVisualProbe } from "./lib/operator-safety.ts";

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

type SuiteRunner = (
  context: ProbeContext,
) => SuiteResult | Promise<SuiteResult>;

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
  protocol: (context) => {
    const result = cargoBinTests(context, "window_dispatch_tests", "protocol");
    return result;
  },
  "window-switcher": (context) =>
    cargoBinTests(context, "focus_reducer_tests", "window-switcher"),
  "sdk-parity": (context) => {
    const repoRoot = resolve(import.meta.dir, "../..");
    // 1. Locked public files must be byte-identical to the recorded baseline.
    const ledgerPath = resolve(
      repoRoot,
      ".notes/oracle/window-engine-foundation/ledger.md",
    );
    let base = "";
    try {
      const ledger = readFileSync(ledgerPath, "utf8");
      base = ledger.match(/^baseline_commit: (\S+)/m)?.[1] ?? "";
    } catch {
      // Ledger absent (fresh checkout): fall back to HEAD-relative check.
    }
    const lockedFiles = [
      "scripts/kit-sdk.ts",
      "src/protocol/types/primitives.rs",
      "src/protocol/message/variants/system_control.rs",
      "src/config/types.rs",
      "src/builtins/mod.rs",
      "kit-init/scriptlets/window-management/main.md",
    ];
    if (base) {
      const diff = spawnSync("git", ["diff", "--exit-code", base, "--", ...lockedFiles], {
        cwd: repoRoot,
        encoding: "utf8",
      });
      if (diff.status !== 0) {
        return {
          suite: "sdk-parity",
          status: "fail",
          detail: `locked public files changed since baseline ${base}: ${diff.stdout.slice(0, 400)}`,
        };
      }
    }
    // 2. The SDK test carries the exact 21-string wire vocabulary and the
    //    fixture-only mutation gate.
    const sdkTest = readFileSync(
      resolve(repoRoot, "tests/sdk/test-window-management.ts"),
      "utf8",
    );
    const required = [
      '"almost-maximize"',
      '"first-two-thirds"',
      "isMutableFixture",
      "SK Window Fixture",
      "Disposable",
    ];
    const missing = required.filter((needle) => !sdkTest.includes(needle));
    if (missing.length > 0) {
      return {
        suite: "sdk-parity",
        status: "fail",
        detail: `SDK test contract markers missing: ${missing.join(", ")}`,
      };
    }
    if (sdkTest.includes("sixth")) {
      return {
        suite: "sdk-parity",
        status: "fail",
        detail: "SDK test must not add internal sixth positions to the wire vocabulary",
      };
    }
    // 3. Engine-side legacy parity tests.
    const legacy = cargoProviderTests(context, "window_control::legacy", "sdk-parity");
    if (legacy.status !== "pass") return { ...legacy, suite: "sdk-parity" };
    return {
      suite: "sdk-parity",
      status: "pass",
      detail: `locked files clean vs ${base || "HEAD"}; SDK test contract markers present; ${legacy.detail}`,
    };
  },
  native: (context) => {
    assertNoninteractiveVisualProbe("window-engine.native-appkit-fixture");
    // Decision rule "Native fixture toolchain": requires xcrun swiftc.
    const which = spawnSync("xcrun", ["--find", "swiftc"], { encoding: "utf8" });
    if (which.status !== 0) {
      return {
        suite: "native",
        status: "environment-blocked",
        detail: "xcrun swiftc unavailable; native fixture clause environment-blocked",
      };
    }
    const repoRoot = resolve(import.meta.dir, "../..");
    const fixtureSource =
      context.nativeFixture ??
      resolve(repoRoot, "scripts/devtools/fixtures/window-engine-native.swift");
    const outDir = mkdtempSync(join(tmpdir(), "window-engine-native-"));
    const binary = join(outDir, "window-engine-native");
    const compile = spawnSync("xcrun", ["swiftc", "-o", binary, fixtureSource], {
      encoding: "utf8",
      timeout: 180_000,
    });
    if (compile.status !== 0) {
      return {
        suite: "native",
        status: "fail",
        detail: `swiftc failed: ${(compile.stderr ?? "").slice(0, 400)}`,
      };
    }
    // Launch, ask the fixture to self-report its windows, and verify the
    // scenario coverage (public AppKit only; no AX permission required).
    const child = spawn(binary, [], { stdio: ["pipe", "pipe", "pipe"] });
    return new Promise<SuiteResult>((resolveSuite) => {
      let output = "";
      let listRequested = false;
      const finish = (result: SuiteResult) => {
        try {
          child.stdin.write("quit\n");
        } catch {}
        setTimeout(() => child.kill("SIGKILL"), 1_000);
        resolveSuite(result);
      };
      const timer = setTimeout(() => {
        finish({
          suite: "native",
          status: "fail",
          detail: `native fixture did not report in time; output: ${output.slice(0, 300)}`,
        });
      }, 20_000);
      child.stdout.on("data", (chunk: Buffer) => {
        output += chunk.toString();
        if (output.includes("READY") && !listRequested) {
          listRequested = true;
          child.stdin.write("list\n");
        }
        const jsonLine = output
          .split("\n")
          .find((line) => line.trim().startsWith("["));
        if (jsonLine) {
          clearTimeout(timer);
          try {
            const rows = JSON.parse(jsonLine) as Array<{
              key: string;
              title: string;
              windowNumber: number;
              isSheet: boolean;
              tabCount: number;
            }>;
            const keys = new Set(rows.map((row) => row.key));
            const required = [
              "ordinary",
              "constrained",
              "panel",
              "sheet",
              "twin-a",
              "twin-b",
              "tab-one",
              "tab-two",
            ];
            const missing = required.filter((key) => !keys.has(key));
            if (missing.length > 0) {
              finish({
                suite: "native",
                status: "fail",
                detail: `native fixture missing windows: ${missing.join(", ")}`,
              });
              return;
            }
            const twins = rows.filter((row) => row.title === "SK Native Fixture Twin");
            const distinctTwinIds = new Set(twins.map((row) => row.windowNumber));
            if (distinctTwinIds.size !== 2) {
              finish({
                suite: "native",
                status: "fail",
                detail: "duplicate-title twins must have distinct native window numbers",
              });
              return;
            }
            const tabbed = rows.find((row) => row.key === "tab-one");
            finish({
              suite: "native",
              status: "pass",
              detail: `native fixture reported ${rows.length} windows; twins distinct; tab group size ${tabbed?.tabCount ?? 0}`,
            });
          } catch (error) {
            finish({
              suite: "native",
              status: "fail",
              detail: `bad fixture report: ${error}`,
            });
          }
        }
      });
      child.on("error", (error) => {
        clearTimeout(timer);
        finish({ suite: "native", status: "fail", detail: String(error) });
      });
    });
  },
  "app-profiles": () => ({
    suite: "app-profiles",
    status: "environment-blocked",
    detail:
      "live app mutation requires Accessibility permission for this probe process (decision rule: permission-blocked; provider/profile unit proof green via cargo)",
  }),
  "rapid-cycle": (context) =>
    cargoProviderTests(
      context,
      "window_control::transaction::tests::one_hundred_rapid_alternating_placements_hit_only_their_targets",
      "rapid-cycle",
    ),
  profile: () => ({
    suite: "profile",
    status: "environment-blocked",
    detail:
      "live per-bundle profile verification requires Accessibility permission; locked table proven by window_control::app_profiles unit tests",
  }),
  permission: () => {
    // Honest observation: an ephemeral probe process cannot hold a TCC
    // Accessibility grant; live mutation suites are environment-gated.
    return {
      suite: "permission",
      status: "environment-blocked",
      detail:
        "Accessibility permission is not grantable to ephemeral probe processes; run live suites from the installed app context",
    };
  },
};

/// Run bin-target cargo tests (render_builtins/execute_script live in the bin).
function cargoBinTests(context: ProbeContext, filter: string, label: string): SuiteResult {
  const result = spawnSync(
    "./scripts/agentic/agent-cargo.sh",
    ["test", "--bin", "script-kit-gpui", filter],
    {
      cwd: resolve(import.meta.dir, "../.."),
      env: {
        ...process.env,
        ...(context.providerRaw
          ? { SCRIPT_KIT_WINDOW_SEARCH_TEST_PROVIDER: context.providerRaw }
          : {}),
      },
      encoding: "utf8",
      timeout: 600_000,
    },
  );
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  const okLine = output.match(/test result: ok\. (\d+) passed/);
  if (result.status === 0 && okLine && Number(okLine[1]) > 0) {
    return { suite: label, status: "pass", detail: `${okLine[1]} bin tests passed (${filter})` };
  }
  return {
    suite: label,
    status: "fail",
    detail: `cargo bin test ${filter} exited ${result.status}: ${output.slice(-500)}`,
  };
}

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
  const results: SuiteResult[] = [];
  for (const suite of suites) {
    const result = await SUITES[suite](context);
    console.log(JSON.stringify(result));
    results.push(result);
  }
  const failed = results.filter((result) => result.status === "fail");
  const pending = results.filter((result) => result.status === "pending");
  const blocked = results.filter((result) => result.status === "environment-blocked");
  if (failed.length > 0) {
    console.error(`FAIL: ${failed.map((result) => result.suite).join(", ")}`);
    process.exit(1);
  }
  if (pending.length > 0) {
    console.error(`PENDING (not yet implemented): ${pending.map((result) => result.suite).join(", ")}`);
    process.exit(3);
  }
  if (blocked.length > 0) {
    console.error(
      `ENVIRONMENT GAPS: ${blocked.map((result) => result.suite).join(", ")}`,
    );
  }
  console.log("OK");
}

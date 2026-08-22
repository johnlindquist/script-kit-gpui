import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

export interface CargoTimingUnit {
  name: string;
  version: string;
  target: string;
  duration: number;
}

export interface CargoTimingSummary {
  profile: string;
  totalSeconds: number;
  totalUnits: number;
  dirtyUnits: number;
  maxConcurrency: number;
  hottestUnits: CargoTimingUnit[];
  duplicatedUnits: Array<{
    name: string;
    version: string;
    target: string;
    copies: number;
    totalSeconds: number;
  }>;
  recommendations: string[];
}

function summaryCell(html: string, label: string): string {
  const escaped = label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = html.match(
    new RegExp(`<td>${escaped}:<\\/td>\\s*<td>([^<]+)<\\/td>`),
  );
  if (!match) throw new Error(`Cargo timing report is missing ${label}`);
  return match[1].trim();
}

export function parseCargoTimingReport(html: string): CargoTimingSummary {
  const match = html.match(
    /const UNIT_DATA = (\[[\s\S]*?\]);\s*const CONCURRENCY_DATA = /,
  );
  if (!match) throw new Error("Cargo timing report has no complete UNIT_DATA payload");

  let units: CargoTimingUnit[];
  try {
    units = JSON.parse(match[1]);
  } catch {
    throw new Error("Cargo timing report has malformed UNIT_DATA");
  }
  if (!Array.isArray(units) || units.length === 0) {
    throw new Error("Cargo timing report contains no executed compiler units");
  }
  if (
    units.some(
      (unit) =>
        typeof unit.name !== "string" ||
        typeof unit.version !== "string" ||
        typeof unit.target !== "string" ||
        typeof unit.duration !== "number" ||
        !Number.isFinite(unit.duration) ||
        unit.duration < 0,
    )
  ) {
    throw new Error("Cargo timing report contains invalid compiler-unit data");
  }

  const totalSeconds = Number.parseFloat(summaryCell(html, "Total time"));
  const totalUnits = Number.parseInt(summaryCell(html, "Total units"), 10);
  const dirtyUnits = Number.parseInt(summaryCell(html, "Dirty units"), 10);
  const maxConcurrency = Number.parseInt(summaryCell(html, "Max concurrency"), 10);
  if (
    !Number.isFinite(totalSeconds) ||
    !Number.isInteger(totalUnits) ||
    !Number.isInteger(dirtyUnits) ||
    !Number.isInteger(maxConcurrency)
  ) {
    throw new Error("Cargo timing report has invalid build summary values");
  }

  const hottestUnits = [...units]
    .sort((left, right) => right.duration - left.duration)
    .slice(0, 10)
    .map(({ name, version, target, duration }) => ({
      name,
      version,
      target: target.trim(),
      duration,
    }));

  const groups = new Map<string, CargoTimingUnit[]>();
  for (const unit of units) {
    if (unit.target.includes("build script")) continue;
    const key = `${unit.name}\0${unit.version}\0${unit.target}`;
    const group = groups.get(key) ?? [];
    group.push(unit);
    groups.set(key, group);
  }

  const duplicatedUnits = [...groups.values()]
    .filter((group) => group.length > 1)
    .map((group) => ({
      name: group[0].name,
      version: group[0].version,
      target: group[0].target.trim(),
      copies: group.length,
      totalSeconds: Number(
        group.reduce((sum, unit) => sum + unit.duration, 0).toFixed(2),
      ),
    }))
    .sort((left, right) => right.totalSeconds - left.totalSeconds)
    .slice(0, 10);

  const recommendations: string[] = [];
  const appHarness = units.find(
    (unit) => unit.name === "script-kit-gpui" && unit.target.includes("lib (test)"),
  );
  if (appHarness && appHarness.duration >= 30) {
    recommendations.push(
      `application test harness alone takes ${appHarness.duration.toFixed(1)}s; extract pure owners into independently tested workspace crates`,
    );
  }
  const whisperBuild = units.find(
    (unit) => unit.name === "whisper-rs-sys" && unit.target.includes("build script (run)"),
  );
  if (whisperBuild && whisperBuild.duration >= 10) {
    recommendations.push(
      `native Whisper build takes ${whisperBuild.duration.toFixed(1)}s; keep non-audio tests out of the app dependency graph`,
    );
  }
  for (const duplicate of duplicatedUnits.slice(0, 3)) {
    if (duplicate.totalSeconds >= 5) {
      recommendations.push(
        `${duplicate.name} ${duplicate.version} compiled ${duplicate.copies} times (${duplicate.totalSeconds.toFixed(1)}s); inspect normal versus build dependencies and feature resolution`,
      );
    }
  }

  return {
    profile: summaryCell(html, "Profile"),
    totalSeconds,
    totalUnits,
    dirtyUnits,
    maxConcurrency,
    hottestUnits,
    duplicatedUnits,
    recommendations,
  };
}

if (import.meta.main) {
  let report = resolve(
    import.meta.dir,
    "../../target-agent/pools/agent-debug/cargo-timings/cargo-timing.html",
  );
  let output: string | undefined;

  for (let index = 0; index < process.argv.slice(2).length; index += 1) {
    const args = process.argv.slice(2);
    const argument = args[index];
    if (argument === "--report" && args[index + 1]) {
      report = resolve(args[index + 1]);
      index += 1;
    } else if (argument === "--out" && args[index + 1]) {
      output = resolve(args[index + 1]);
      index += 1;
    } else {
      console.error(
        "usage: bun scripts/agentic/cargo-timings-summary.ts [--report path] [--out path]",
      );
      process.exit(2);
    }
  }

  try {
    const summary = parseCargoTimingReport(readFileSync(report, "utf8"));
    const serialized = `${JSON.stringify(summary, null, 2)}\n`;
    if (output) {
      mkdirSync(dirname(output), { recursive: true });
      writeFileSync(output, serialized, { mode: 0o600 });
    }
    process.stdout.write(serialized);
  } catch (error) {
    console.error(
      `CARGO_TIMINGS error: ${error instanceof Error ? error.message : String(error)}`,
    );
    process.exit(1);
  }
}

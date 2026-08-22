import { describe, expect, test } from "bun:test";
import { parseCargoTimingReport } from "./cargo-timings-summary.ts";

const units = [
  { name: "gpui", version: "0.2.2", target: "", duration: 7.2 },
  { name: "gpui", version: "0.2.2", target: "", duration: 6.3 },
  {
    name: "whisper-rs-sys",
    version: "0.15.0",
    target: " build script (run)",
    duration: 17.2,
  },
  {
    name: "script-kit-gpui",
    version: "0.1.17",
    target: " lib (test)",
    duration: 67,
  },
];

function fixture(compilerUnits: unknown = units) {
  return `
<td>Profile:</td><td>test</td>
<td>Dirty units:</td><td>1228</td>
<td>Total units:</td><td>1233</td>
<td>Max concurrency:</td><td>2 (jobs=2 ncpu=16)</td>
<td>Total time:</td><td>257.2s (4m 17.2s)</td>
const UNIT_DATA = ${JSON.stringify(compilerUnits, null, 2)};
const CONCURRENCY_DATA = [];
`;
}

describe("truthful Cargo critical-path diagnostics", () => {
  test("child-process output remains observable to evidence-producing tests", () => {
    const result = Bun.spawnSync(
      ["bun", "-e", "console.log('stdout-visible'); console.error('stderr-visible')"],
      { stdout: "pipe", stderr: "pipe" },
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("stdout-visible");
    expect(result.stderr.toString()).toContain("stderr-visible");
  });

  test("reports the actual app, native audio, duplicate GPUI, and bounded-worker bottlenecks", () => {
    const summary = parseCargoTimingReport(fixture());
    expect(summary).toMatchObject({
      profile: "test",
      totalSeconds: 257.2,
      totalUnits: 1233,
      dirtyUnits: 1228,
      maxConcurrency: 2,
    });
    expect(summary.hottestUnits[0]).toMatchObject({
      name: "script-kit-gpui",
      duration: 67,
    });
    expect(summary.duplicatedUnits).toContainEqual({
      name: "gpui",
      version: "0.2.2",
      target: "",
      copies: 2,
      totalSeconds: 13.5,
    });
    expect(summary.recommendations).toHaveLength(3);
    expect(summary.recommendations.join(" ")).toContain("Whisper");
  });

  test("fails closed on missing timing data instead of inventing a green baseline", () => {
    expect(() => parseCargoTimingReport("<html>no timing receipt</html>")).toThrow(
      "no complete UNIT_DATA payload",
    );
    expect(() => parseCargoTimingReport(fixture([]))).toThrow(
      "no executed compiler units",
    );
  });

  test("rejects malformed or negative compiler observations", () => {
    expect(() =>
      parseCargoTimingReport(
        fixture([{ name: "gpui", version: "0.2.2", target: "", duration: -1 }]),
      ),
    ).toThrow("invalid compiler-unit data");
    expect(() => parseCargoTimingReport(fixture().replace("257.2s", "unknown"))).toThrow(
      "invalid build summary values",
    );
  });
});

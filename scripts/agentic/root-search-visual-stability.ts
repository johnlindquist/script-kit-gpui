#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { assertNoninteractiveVisualProbe } from "../devtools/lib/operator-safety.ts";

assertNoninteractiveVisualProbe("root-search.visual-stability");

type Json = Record<string, any>;

const repoRoot = resolve(import.meta.dir, "../..");
const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ?? join(repoRoot, "target/debug/script-kit-gpui");
const windowScript = join(repoRoot, "scripts/agentic/window.ts");
const macosInputScript = join(repoRoot, "scripts/agentic/macos-input.ts");

function argValue(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

function hasFlag(name: string): boolean {
  return process.argv.includes(name);
}

function timestampSlug(): string {
  return new Date().toISOString().replace(/[:.]/g, "-");
}

const query = argValue("--query", "fix");
const durationMs = Number(argValue("--duration", "1400"));
const captureEveryMs = Number(argValue("--capture-every", "90"));
const stateEveryMs = Number(argValue("--state-every", "60"));
const providerDelayMs = Number(argValue("--provider-delay", "250"));
const maxGroupMs = Number(argValue("--max-group-ms", "80"));
const maxHandlerMs = Number(argValue("--max-handler-ms", "80"));
const outDir = resolve(
  repoRoot,
  argValue("--out", `.test-output/root-search-visual-stability-${timestampSlug()}`),
);
const noFixture = hasFlag("--no-fixture");
const warmProvider = hasFlag("--warm-provider");
const expectVisibleFileResults = hasFlag("--expect-visible-file-results");
const protocolInput = hasFlag("--protocol-input");

const screenshotDir = join(outDir, "screens");
mkdirSync(screenshotDir, { recursive: true });

type Pending = {
  expect?: string;
  resolve: (value: Json) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

const logLines: string[] = [];
const jsonLines: Json[] = [];
const pending = new Map<string, Pending>();
let responseCounter = 0;

function nowMs(startedAt: number): number {
  return Math.round(performance.now() - startedAt);
}

function appendLog(source: string, line: string, startedAt: number): void {
  const tagged = `${nowMs(startedAt).toString().padStart(5, " ")}ms ${source} ${line}`;
  logLines.push(tagged);

  try {
    const parsed = JSON.parse(line);
    if (parsed && typeof parsed === "object") {
      jsonLines.push(parsed);
      const requestId = parsed.requestId;
      if (typeof requestId === "string") {
        const waiter = pending.get(requestId);
        if (waiter) {
          if (waiter.expect && parsed.type !== waiter.expect) {
            waiter.reject(
              new Error(
                `request ${requestId} expected ${waiter.expect}, got ${parsed.type}: ${line}`,
              ),
            );
          } else {
            waiter.resolve(parsed);
          }
          clearTimeout(waiter.timer);
          pending.delete(requestId);
        }
      }
    }
  } catch {
    // Normal app log line.
  }
}

async function readStream(
  stream: ReadableStream<Uint8Array>,
  source: string,
  startedAt: number,
): Promise<void> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const chunk = await reader.read();
    if (chunk.done) break;
    buffer += decoder.decode(chunk.value, { stream: true });
    let newline = buffer.indexOf("\n");
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trimEnd();
      buffer = buffer.slice(newline + 1);
      if (line.length > 0) appendLog(source, line, startedAt);
      newline = buffer.indexOf("\n");
    }
  }

  const tail = buffer.trim();
  if (tail.length > 0) appendLog(source, tail, startedAt);
}

function send(proc: Bun.Subprocess<"ignore", "pipe", "pipe", "pipe">, command: Json): string {
  const requestId = command.requestId ?? `visual-${++responseCounter}-${Date.now()}`;
  command.requestId = requestId;
  proc.stdin.write(`${JSON.stringify(command)}\n`);
  return requestId;
}

function rpc(
  proc: Bun.Subprocess<"ignore", "pipe", "pipe", "pipe">,
  command: Json,
  expect: string,
  timeoutMs = 5000,
): Promise<Json> {
  const requestId = command.requestId ?? `visual-${++responseCounter}-${Date.now()}`;
  command.requestId = requestId;
  return new Promise((resolvePromise, rejectPromise) => {
    const timer = setTimeout(() => {
      pending.delete(requestId);
      rejectPromise(new Error(`timed out waiting for ${expect} (${requestId})`));
    }, timeoutMs);
    pending.set(requestId, {
      expect,
      resolve: resolvePromise,
      reject: rejectPromise,
      timer,
    });
    proc.stdin.write(`${JSON.stringify(command)}\n`);
  });
}

async function runCommand(args: string[], label: string): Promise<{ code: number; stdout: string; stderr: string }> {
  const proc = Bun.spawn(args, {
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  return { code, stdout, stderr };
}

async function windowStatus(): Promise<Json> {
  const result = await runCommand(["bun", windowScript, "status", "--json"], "window-status");
  if (result.code !== 0) {
    throw new Error(`window status failed: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

async function waitForWindow(timeoutMs = 5000): Promise<Json> {
  const deadline = performance.now() + timeoutMs;
  let last: Json | null = null;
  while (performance.now() < deadline) {
    last = await windowStatus();
    const windows = last?.data?.windows;
    if (Array.isArray(windows) && windows.length > 0) {
      return windows[0];
    }
    await Bun.sleep(100);
  }
  throw new Error(`no visible Script Kit window; last=${JSON.stringify(last)}`);
}

function visibleFrame(state: Json): Json {
  const preflight = state.mainWindowPreflight ?? {};
  return {
    inputValue: state.inputValue ?? "",
    selectedIndex: preflight.selectedIndex ?? null,
    selectedResultKey: preflight.selectedResultKey ?? null,
    selectedResultRole: preflight.selectedResultRole ?? null,
    visibleResultKeyFingerprint: preflight.visibleResultKeyFingerprint ?? "",
    visibleRowFingerprint: preflight.visibleRowFingerprint ?? "",
    visibleResults: preflight.visibleResults ?? [],
    visibleResultCount: preflight.visibleResultCount ?? 0,
    enterAction: preflight.enterAction ?? null,
    rootFileSearch: state.rootFileSearch ?? null,
    rootPassiveFrame: preflight.rootPassiveFrame ?? null,
  };
}

function visibleRootFileCount(frame: Json): number {
  const visibleResults = frame.visibleResults;
  if (!Array.isArray(visibleResults)) return 0;
  return visibleResults.filter((result) => result?.role === "rootFile").length;
}

async function sampleStates(
  proc: Bun.Subprocess<"ignore", "pipe", "pipe", "pipe">,
  startedAt: number,
  stopAt: number,
): Promise<Json[]> {
  const samples: Json[] = [];
  let index = 0;
  while (performance.now() < stopAt) {
    try {
      const state = await rpc(
        proc,
        { type: "getState", requestId: `visual-state-${index}-${Date.now()}` },
        "stateResult",
        2500,
      );
      samples.push({
        tMs: nowMs(startedAt),
        frame: visibleFrame(state),
      });
    } catch (error) {
      samples.push({
        tMs: nowMs(startedAt),
        error: error instanceof Error ? error.message : String(error),
      });
    }
    index++;
    await Bun.sleep(stateEveryMs);
  }
  return samples;
}

async function captureFrames(windowId: number, startedAt: number, stopAt: number): Promise<Json[]> {
  const frames: Json[] = [];
  let index = 0;
  while (performance.now() < stopAt) {
    const tMs = nowMs(startedAt);
    const path = join(screenshotDir, `frame-${String(index).padStart(3, "0")}-${tMs}ms.png`);
    const result = await runCommand(["screencapture", "-x", "-l", String(windowId), path], "capture");
    frames.push({
      index,
      tMs,
      path,
      ok: result.code === 0 && existsSync(path),
      stderr: result.stderr.trim(),
    });
    index++;
    await Bun.sleep(captureEveryMs);
  }
  return frames;
}

async function typeQueryNative(): Promise<Json> {
  const steps = query.split("").flatMap((character, index) => {
    const step: Json[] = [{ action: "type", text: character }];
    if (index < query.length - 1) step.push({ action: "sleep", ms: 35 });
    return step;
  });
  const result = await runCommand(
    [
      "bun",
      macosInputScript,
      "sequence",
      JSON.stringify(steps),
      "--ensure-focus",
      "--focus-title",
      "Script Kit",
      "--json",
    ],
    "native-type",
  );
  if (result.code !== 0) {
    throw new Error(`native typing failed: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout);
}

async function waitForRootFileProviderSettlement(
  proc: Bun.Subprocess<"ignore", "pipe", "pipe", "pipe">,
  startedAt: number,
  timeoutMs = 10000,
): Promise<Json> {
  const deadline = performance.now() + timeoutMs;
  let lastFrame: Json | null = null;

  while (performance.now() < deadline) {
    const state = await rpc(
      proc,
      { type: "getState", requestId: `visual-warm-state-${Date.now()}` },
      "stateResult",
      2500,
    );
    const frame = visibleFrame(state);
    lastFrame = frame;
    const rootFile = frame.rootFileSearch;
    if (
      frame.inputValue === query &&
      rootFile?.query === query &&
      rootFile?.loading === false &&
      (rootFile?.cacheResultCount ?? 0) > 0
    ) {
      return {
        tMs: nowMs(startedAt),
        frame,
      };
    }
    await Bun.sleep(80);
  }

  throw new Error(
    `root file provider did not settle with cached results for ${query}; last=${JSON.stringify(lastFrame)}`,
  );
}

async function warmRootFileProvider(
  proc: Bun.Subprocess<"ignore", "pipe", "pipe", "pipe">,
  startedAt: number,
): Promise<Json> {
  send(proc, { type: "setFilter", text: query, requestId: "visual-warm-query" });
  const settled = await waitForRootFileProviderSettlement(proc, startedAt);
  send(proc, { type: "setFilter", text: "", requestId: "visual-warm-clear" });
  await Bun.sleep(150);
  return settled;
}

function frameChanged(frame: Json, baseline: Json): boolean {
  return (
    frame.selectedResultKey !== baseline.selectedResultKey ||
    frame.selectedResultRole !== baseline.selectedResultRole ||
    frame.visibleResultKeyFingerprint !== baseline.visibleResultKeyFingerprint ||
    frame.visibleRowFingerprint !== baseline.visibleRowFingerprint
  );
}

function assertInputFramesStable(samples: Json[]): Json[] {
  const byInput = new Map<string, Json[]>();
  for (const sample of samples) {
    const input = sample.frame?.inputValue;
    if (typeof input !== "string" || input.length === 0) continue;
    const list = byInput.get(input) ?? [];
    list.push(sample);
    byInput.set(input, list);
  }

  const timeline: Json[] = [];
  const unstable: Json[] = [];

  for (const [input, inputSamples] of byInput.entries()) {
    if (inputSamples.length < 2) continue;
    const baseline = inputSamples[0].frame;
    const changes = inputSamples
      .filter((sample) => frameChanged(sample.frame, baseline))
      .map((sample) => ({
        tMs: sample.tMs,
        selectedResultKey: sample.frame.selectedResultKey,
        selectedResultRole: sample.frame.selectedResultRole,
        visibleResultKeyFingerprint: sample.frame.visibleResultKeyFingerprint,
        visibleRowFingerprint: sample.frame.visibleRowFingerprint,
      }));

    const rootFileStates = Array.from(
      new Set(
        inputSamples.map((sample) => {
          const rootFile = sample.frame.rootFileSearch;
          if (!rootFile || rootFile.query !== input) return "none";
          return `loading=${rootFile.loading};cache=${rootFile.cacheResultCount};visible=${rootFile.visibleResultCount}`;
        }),
      ),
    );

    timeline.push({
      input,
      sampleCount: inputSamples.length,
      firstAtMs: inputSamples[0].tMs,
      lastAtMs: inputSamples[inputSamples.length - 1].tMs,
      selectedResultKey: baseline.selectedResultKey,
      selectedResultRole: baseline.selectedResultRole,
      visibleResultCount: baseline.visibleResultCount,
      visibleRootFileCount: visibleRootFileCount(baseline),
      rootFileStates,
    });

    if (changes.length > 0) {
      unstable.push({
        input,
        baseline: {
          selectedResultKey: baseline.selectedResultKey,
          selectedResultRole: baseline.selectedResultRole,
          visibleResultKeyFingerprint: baseline.visibleResultKeyFingerprint,
          visibleRowFingerprint: baseline.visibleRowFingerprint,
        },
        changes,
      });
    }
  }

  if (unstable.length > 0) {
    throw new Error(`visible frame changed for the same input value: ${JSON.stringify(unstable)}`);
  }

  return timeline;
}

function parseLatencyMetrics(lines: string[]): Json {
  const grouped: Json[] = [];
  const handlers: Json[] = [];
  const groupRegex = /\[4b\/5\] GROUP_DONE '([^']*)' in ([0-9.]+)ms ->/;
  const handlerRegex = /\[HANDLER_SLOW\] handle_filter_input_change took ([0-9.]+)ms for '([^']*)'/;

  for (const line of lines) {
    const groupMatch = line.match(groupRegex);
    if (groupMatch) {
      grouped.push({
        input: groupMatch[1],
        ms: Number(groupMatch[2]),
        line,
      });
      continue;
    }

    const handlerMatch = line.match(handlerRegex);
    if (handlerMatch) {
      handlers.push({
        input: handlerMatch[2],
        ms: Number(handlerMatch[1]),
        line,
      });
    }
  }

  const nonEmptyGrouped = grouped.filter((entry) => entry.input.length > 0);
  const nonEmptyHandlers = handlers.filter((entry) => entry.input.length > 0);
  return {
    maxGroupMs,
    maxHandlerMs,
    grouped,
    handlers,
    slowGrouped: nonEmptyGrouped.filter((entry) => entry.ms > maxGroupMs),
    slowHandlers: nonEmptyHandlers.filter((entry) => entry.ms > maxHandlerMs),
    maxObservedGroupMs: nonEmptyGrouped.reduce((max, entry) => Math.max(max, entry.ms), 0),
    maxObservedHandlerMs: nonEmptyHandlers.reduce((max, entry) => Math.max(max, entry.ms), 0),
  };
}

function assertLatency(lines: string[]): Json {
  const metrics = parseLatencyMetrics(lines);
  if (metrics.slowGrouped.length > 0 || metrics.slowHandlers.length > 0) {
    throw new Error(`search latency exceeded visual stability budget: ${JSON.stringify(metrics)}`);
  }
  return metrics;
}

function assertStable(samples: Json[]): Json {
  const fullQuerySamples = samples.filter((sample) => sample.frame?.inputValue === query);
  if (fullQuerySamples.length < 2) {
    throw new Error(`expected at least two state samples for ${query}; samples=${JSON.stringify(samples)}`);
  }

  const prefixTimeline = assertInputFramesStable(samples);
  const baseline = fullQuerySamples[0].frame;
  const changes = fullQuerySamples.filter((sample) => frameChanged(sample.frame, baseline));

  const observedProviderLoading = fullQuerySamples.some(
    (sample) => sample.frame.rootFileSearch?.query === query && sample.frame.rootFileSearch?.loading === true,
  );
  const observedProviderSettled = fullQuerySamples.some(
    (sample) =>
      sample.frame.rootFileSearch?.query === query &&
      sample.frame.rootFileSearch?.loading === false &&
      (sample.frame.rootFileSearch?.cacheResultCount ?? 0) > 0,
  );

  if (!observedProviderLoading) {
    throw new Error(`never observed delayed provider loading for ${query}`);
  }
  if (!observedProviderSettled) {
    throw new Error(`never observed delayed provider settlement for ${query}`);
  }
  if (changes.length > 0) {
    throw new Error(
      `visible frame changed after full query was displayed: baseline=${JSON.stringify(
        baseline,
      )} changes=${JSON.stringify(changes)}`,
    );
  }

  if (expectVisibleFileResults) {
    const missingVisibleFiles = fullQuerySamples
      .filter((sample) => visibleRootFileCount(sample.frame) === 0)
      .map((sample) => ({
        tMs: sample.tMs,
        rootFileSearch: sample.frame.rootFileSearch,
        visibleResultKeyFingerprint: sample.frame.visibleResultKeyFingerprint,
        visibleRowFingerprint: sample.frame.visibleRowFingerprint,
      }));
    if (missingVisibleFiles.length > 0) {
      throw new Error(
        `expected visible root-file rows for every full-query frame: ${JSON.stringify(missingVisibleFiles)}`,
      );
    }
  }

  return {
    fullQuerySampleCount: fullQuerySamples.length,
    baseline,
    firstFullQueryAtMs: fullQuerySamples[0].tMs,
    lastFullQueryAtMs: fullQuerySamples[fullQuerySamples.length - 1].tMs,
    observedProviderLoading,
    observedProviderSettled,
    expectedVisibleFileResults: expectVisibleFileResults,
    visibleRootFileCount: visibleRootFileCount(baseline),
    prefixTimeline,
  };
}

function createContactSheet(framePaths: string[]): string | null {
  if (framePaths.length === 0) return null;
  const contactPath = join(outDir, "contact-sheet.png");
  const result = spawnSync(
    "magick",
    [
      "montage",
      ...framePaths,
      "-tile",
      "4x",
      "-geometry",
      "360x+8+8",
      "-background",
      "#101010",
      contactPath,
    ],
    { cwd: repoRoot, encoding: "utf8" },
  );
  if (result.status !== 0 || !existsSync(contactPath)) {
    return null;
  }
  return contactPath;
}

async function main() {
  if (!existsSync(binary)) {
    throw new Error(`missing ${binary}; run cargo build --bin script-kit-gpui first`);
  }

  const startedAt = performance.now();
  const env = {
    ...process.env,
    SCRIPT_KIT_AI_LOG: "1",
    RUST_LOG: process.env.RUST_LOG || "info",
  };

  if (!noFixture) {
    env.SCRIPT_KIT_ROOT_FILE_SEARCH_TEST_PROVIDER = JSON.stringify({
      query,
      delayMs: providerDelayMs,
      results: [
        {
          path: `/tmp/${query}-late-provider-result.txt`,
          name: `${query}-late-provider-result.txt`,
          fileType: "document",
          size: 42,
          modified: 1,
        },
      ],
    });
  }

  const proc = Bun.spawn([binary], {
    cwd: repoRoot,
    env,
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });

  const stdoutReader = readStream(proc.stdout, "stdout", startedAt);
  const stderrReader = readStream(proc.stderr, "stderr", startedAt);

  try {
    await rpc(proc, { type: "getState", requestId: "visual-ready-state" }, "stateResult", 8000);
    await rpc(proc, { type: "show", requestId: "visual-show" }, "windowVisibilityAck", 5000);
    await rpc(proc, { type: "setFilter", text: "", requestId: "visual-clear" }, "stateResult", 250).catch(() => null);
    const warmProviderResult = warmProvider ? await warmRootFileProvider(proc, startedAt) : null;

    const window = await waitForWindow(5000);
    const windowId = Number(window.windowId);
    if (!Number.isFinite(windowId) || windowId <= 0) {
      throw new Error(`invalid window id: ${JSON.stringify(window)}`);
    }

    if (!protocolInput) {
      await runCommand(["bun", windowScript, "focus", "--json"], "window-focus");
    }
    const stopAt = performance.now() + durationMs;
    const statePromise = sampleStates(proc, startedAt, stopAt);
    const capturePromise = captureFrames(windowId, startedAt, stopAt);

    await Bun.sleep(120);
    const nativeInput = protocolInput
      ? { mode: "protocol", requestId: send(proc, { type: "setFilter", text: query }) }
      : await typeQueryNative();

    const [stateSamples, frames] = await Promise.all([statePromise, capturePromise]);
    const stability = assertStable(stateSamples);
    const latency = assertLatency(logLines);

    const framePaths = frames.filter((frame) => frame.ok).map((frame) => frame.path);
    const contactSheet = createContactSheet(framePaths);
    const receipt = {
      schemaVersion: 1,
      status: "pass",
      query,
      outDir,
      providerFixture: noFixture
        ? null
        : {
            delayMs: providerDelayMs,
            resultName: `${query}-late-provider-result.txt`,
          },
      warmProvider,
      warmProviderResult,
      expectVisibleFileResults,
      nativeInput,
      window: {
        windowId,
        title: window.title,
        bounds: window.bounds,
      },
      stability,
      latency,
      captures: {
        count: framePaths.length,
        contactSheet,
        frames,
      },
      stateSamples,
      logPath: join(outDir, "app.log"),
      responsePath: join(outDir, "responses.jsonl"),
    };

    writeFileSync(join(outDir, "app.log"), `${logLines.join("\n")}\n`);
    writeFileSync(join(outDir, "responses.jsonl"), `${jsonLines.map((line) => JSON.stringify(line)).join("\n")}\n`);
    writeFileSync(join(outDir, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);

    process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  } finally {
    for (const waiter of pending.values()) {
      clearTimeout(waiter.timer);
      waiter.reject(new Error("process shutting down"));
    }
    pending.clear();

    proc.kill();
    await proc.exited.catch(() => null);
    await Promise.allSettled([stdoutReader, stderrReader]);
  }
}

main().catch((error) => {
  writeFileSync(join(outDir, "app.log"), `${logLines.join("\n")}\n`);
  writeFileSync(join(outDir, "responses.jsonl"), `${jsonLines.map((line) => JSON.stringify(line)).join("\n")}\n`);
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
  process.exit(1);
});

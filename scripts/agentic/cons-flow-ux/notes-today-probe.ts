#!/usr/bin/env bun
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.notes-today");

type Obj = Record<string, any>;

const PROJECT_ROOT = resolve(import.meta.dir, "../../..");
const BINARY =
  process.env.PROBE_BINARY ??
  join(PROJECT_ROOT, "target-agent/artifacts/cons-flow-c09/script-kit-gpui");
const OUT_PATH = join(PROJECT_ROOT, ".test-output", "cons-flow-c09", "notes-today-receipt.json");
const scenarios = [
  {
    id: "notes-mention-parity",
    script: join(PROJECT_ROOT, "scripts/agentic/notes-spine-host-wiring-probe.ts"),
  },
  {
    id: "notes-agent-chat-return",
    script: join(PROJECT_ROOT, "scripts/agentic/cons-flow-ux/notes-agent-chat-return-probe.ts"),
  },
  {
    id: "today-scope-matrix",
    script: join(PROJECT_ROOT, "scripts/agentic/day-page-agent-chat-handoff-scope-probe.ts"),
  },
  {
    id: "today-agent-chat-return",
    script: join(PROJECT_ROOT, "scripts/agentic/day-agent-chat-return-probe.ts"),
  },
];

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Obj) : {};
}

async function runScenario(spec: { id: string; script: string }): Promise<Obj> {
  const child = Bun.spawn(["bun", spec.script], {
    cwd: PROJECT_ROOT,
    env: { ...process.env, PROBE_BINARY: BINARY },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  let parsed: Obj = {};
  try {
    parsed = asObj(JSON.parse(stdout.trim()));
  } catch (error) {
    parsed = {
      pass: false,
      parseError: error instanceof Error ? error.message : String(error),
      stdoutTail: stdout.slice(-2000),
    };
  }
  return {
    id: spec.id,
    script: spec.script,
    exitCode,
    pass: exitCode === 0 && parsed.pass !== false && parsed.status !== "fail",
    cleanup: parsed.cleanup ?? null,
    failures: parsed.failures ?? [],
    stderrTail: stderr.slice(-2000),
    receipt: parsed,
  };
}

async function exactArtifactProcesses(): Promise<Obj[]> {
  const child = Bun.spawn(["/bin/ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    child.exited,
  ]);
  if (exitCode !== 0) return [{ inspectionFailed: true, exitCode }];
  const exact = resolve(BINARY);
  return stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const command = match[2].trim().split(/\s+/, 1)[0];
      return resolve(command) === exact
        ? [{ pid: Number(match[1]), executable: command }]
        : [];
    });
}

const results: Obj[] = [];
for (const scenario of scenarios) {
  results.push(await runScenario(scenario));
}
const ownedProcesses = await exactArtifactProcesses();
const cleanupReceiptsExact = results.every((result) => {
  const cleanup = asObj(result.cleanup);
  return (
    cleanup.processExited === true &&
    cleanup.streamsDrained === true &&
    cleanup.logWriterClosed === true
  );
});
const receipt: Obj = {
  schemaVersion: 1,
  tool: "notes-today-probe",
  binary: BINARY,
  pass:
    results.every((result) => result.pass === true) &&
    cleanupReceiptsExact &&
    ownedProcesses.length === 0,
  scenarios: results,
  cleanupReceiptsExact,
  exactArtifactOwnedProcessCount: ownedProcesses.length,
  exactArtifactOwnedProcesses: ownedProcesses,
};

mkdirSync(join(PROJECT_ROOT, ".test-output", "cons-flow-c09"), { recursive: true });
await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exitCode = 1;

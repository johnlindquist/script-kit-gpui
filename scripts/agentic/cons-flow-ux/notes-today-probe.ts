#!/usr/bin/env bun
import { runtimeArtifactFromEnvironment } from "../../devtools/lib/runtime-task-proof.ts";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observedWorkflowStage,
  prepareBlockedWorkflowTaskProof,
  prepareWorkflowTaskProof,
  writeWorkflowTaskProof,
  type WorkflowObservedSegment,
} from "../../devtools/lib/workflow-task-proof.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.notes-today");

type Obj = Record<string, any>;

const PROJECT_ROOT = resolve(import.meta.dir, "../../..");
const BINARY = runtimeArtifactFromEnvironment().executablePath
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
    id: "today-mention-parity",
    script: join(PROJECT_ROOT, "scripts/agentic/day-page-context-roundtrip-probe.ts"),
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
for (const taskId of ["WF-013", "WF-014", "WF-015"] as const) {
  try {
    if (receipt.pass !== true) throw new Error("Notes/Today host journey did not pass");
    const stageIds = taskId === "WF-013"
      ? ["notes-mention-parity", "today-mention-parity"]
      : taskId === "WF-014"
        ? ["notes-agent-chat-return", "today-scope-matrix"]
        : ["notes-agent-chat-return", "today-agent-chat-return"];
    const selected = stageIds.map((id) => {
      const result = results.find((candidate) => candidate.id === id);
      const segment = asObj(result?.receipt?.workflowObservedSegment);
      if (result?.pass !== true || segment.id !== id) {
        throw new Error(`missing actual Notes/Today child target observation: ${id}`);
      }
      return { result, segment: segment as unknown as WorkflowObservedSegment };
    });
    const notesParity = results.find((result) => result.id === "notes-mention-parity");
    const notesChecks = Array.isArray(notesParity?.receipt?.checks)
      ? notesParity.receipt.checks.map(asObj)
      : [];
    const notesReturn = asObj(results.find((result) => result.id === "notes-agent-chat-return")?.receipt);
    const dayScope = asObj(results.find((result) => result.id === "today-scope-matrix")?.receipt);
    const dayReturn = asObj(results.find((result) => result.id === "today-agent-chat-return")?.receipt);
    const controls = taskId === "WF-013"
      ? {
          "partial-reference-never-survives-deletion":
            notesChecks.some((check) => check.name === "context_reference_deletes_atomically" && check.ok === true),
          "file-discovery-never-silently-disappears":
            notesChecks.some((check) => check.name === "at_context_subsearch_loading_visible" && check.ok === true),
        }
      : taskId === "WF-014"
        ? {
            "outside-selected-range-never-staged":
              asObj(dayScope.selection_receipt_has_no_outside_range_canary).ok === true,
            "context-handoff-never-auto-submits":
              asObj(notesReturn.main_agent_chat_opened_composer_only).ok === true &&
              asObj(dayScope.current_line_is_composer_only_without_authored_prompt).ok === true,
          }
        : {
            "return-never-targets-a-different-host":
              asObj(notesReturn.notes_host_state_preserved_on_return).ok === true &&
              asObj(dayReturn.restored_day_page).ok === true,
            "unsaved-editor-state-never-discarded":
              asObj(notesReturn.notes_host_state_preserved_on_return).ok === true,
          };
    const prepared = prepareWorkflowTaskProof(taskId, {
      producerOwner: "scripts/agentic/cons-flow-ux/notes-today-probe.ts",
      segments: selected.map((item) => item.segment),
      stages: selected.map(({ result, segment }) => observedWorkflowStage({
        id: result.id,
        primitiveId: "devtools.act",
        segment,
        command: "notesToday.executeHostJourney",
        requestId: `${taskId}:${result.id}`,
        result: {
          pass: result.pass,
          exitCode: result.exitCode,
          failureCount: Array.isArray(result.failures) ? result.failures.length : 0,
        },
        pass: result.pass === true,
      })),
      negativeControls: controls,
      safety: {
        microphoneCaptureStarted: false,
        nativeInputInjected: false,
        liveAiStarted: false,
        screenTakeoverStarted: false,
        clipboardTouched: false,
      },
    });
    writeWorkflowTaskProof(taskId, prepared.receipt);
  } catch (error) {
    writeWorkflowTaskProof(taskId, prepareBlockedWorkflowTaskProof(
      taskId,
      error instanceof Error ? error.message : String(error),
    ).receipt);
  }
}
console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exitCode = 1;

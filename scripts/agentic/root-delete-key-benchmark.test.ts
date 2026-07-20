import { afterEach, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { isAbsolute, join, relative, resolve } from "node:path";
import { tmpdir } from "node:os";

const repoRoot = resolve(import.meta.dir, "../..");
const binaryCandidates = [
  process.env.SCRIPT_KIT_GPUI_BINARY,
  join(repoRoot, "target-agent", "pools", "agent-debug", "debug", "script-kit-gpui"),
  join(repoRoot, "target", "debug", "script-kit-gpui"),
].filter((candidate): candidate is string => Boolean(candidate));
const binary = binaryCandidates.find(existsSync) ?? null;
const ownedRuns: Array<{
  outputDir: string;
  session: string;
  pid: number | null;
  generation: string | null;
}> = [];
const sentinels: number[] = [];

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function launchSentinel(): number {
  const child = spawn("sleep", ["60"], { detached: true, stdio: "ignore" });
  child.unref();
  if (!child.pid) throw new Error("sentinel launch did not return a PID");
  sentinels.push(child.pid);
  return child.pid;
}

afterEach(() => {
  for (const pid of sentinels.splice(0)) {
    try {
      process.kill(-pid, "SIGKILL");
    } catch {
      try {
        process.kill(pid, "SIGKILL");
      } catch {}
    }
  }
  for (const { outputDir, session, pid, generation } of ownedRuns.splice(0)) {
    const sessionEnv = {
      ...process.env,
      SCRIPT_KIT_SESSION_DIR: join(outputDir, "sessions"),
    };
    const status = spawnSync(
      "bash",
      ["scripts/agentic/session.sh", "status", session],
      {
        cwd: repoRoot,
        env: sessionEnv,
        encoding: "utf8",
        timeout: 10_000,
      },
    );
    let safeToRemove = false;
    try {
      safeToRemove = JSON.parse(status.stdout).status === "not_found";
    } catch {}
    if (!safeToRemove && pid && generation) {
      const stop = spawnSync(
        "bash",
        [
          "scripts/agentic/session.sh",
          "stop",
          session,
          "--expected-pid",
          String(pid),
          "--expected-generation",
          generation,
        ],
        {
          cwd: repoRoot,
          env: sessionEnv,
          encoding: "utf8",
          timeout: 10_000,
        },
      );
      try {
        const envelope = JSON.parse(stop.stdout);
        safeToRemove =
          stop.status === 0
          && envelope.ownershipVerified === true
          && envelope.expectedPid === pid
          && envelope.actualPid === pid
          && envelope.expectedGeneration === generation
          && envelope.actualGeneration === generation;
      } catch {}
    }
    if (safeToRemove) rmSync(outputDir, { recursive: true, force: true });
  }
});

test.skipIf(binary === null)(
  "an injected delete run failure still hides and stops only its owned session",
  () => {
    const outputDir = mkdtempSync(join(tmpdir(), "root-delete-cleanup-test-"));
    const session = `root-delete-cleanup-test-${process.pid}-${Date.now()}`;
    const ownedRun: (typeof ownedRuns)[number] = {
      outputDir,
      session,
      pid: null,
      generation: null,
    };
    ownedRuns.push(ownedRun);
    const sentinelPid = launchSentinel();
    const env = {
      ...process.env,
      SCRIPT_KIT_GPUI_BINARY: binary!,
    };
    const result = spawnSync(
      "bun",
      [
        "scripts/agentic/root-delete-key-benchmark.ts",
        "--session",
        session,
        "--output-dir",
        outputDir,
        "--samples",
        "1",
        "--burst-samples",
        "1",
        "--delete-count",
        "1",
        "--inject-run-failure-before-measurement",
      ],
      { cwd: repoRoot, env, encoding: "utf8", timeout: 70_000 },
    );

    expect(result.status).not.toBe(0);
    const receiptPath = join(outputDir, "receipt.json");
    expect(existsSync(receiptPath)).toBe(true);
    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    expect(receipt.status).toBe("error");
    expect(receipt.failure).toContain("injected run failure before delete measurement");
    expect(receipt.session.name).toBe(session);
    ownedRun.pid = receipt.session.pid;
    ownedRun.generation = receipt.session.generation;
    expect(receipt.provenance.binarySha256).toMatch(/^[a-f0-9]{64}$/);
    expect(receipt.provenance.gitSha).toMatch(/^[a-f0-9]{40}$/);
    expect(receipt.cleanup.hidden).toBe(true);
    expect(receipt.cleanup.hiddenState.windowVisible).toBe(false);
    expect(receipt.cleanup.stopped).toBe(true);
    expect(receipt.cleanup.error).toBeNull();
    expect(receipt.cleanup.stopResult).toMatchObject({
      status: "ok",
      session,
      ownershipVerified: true,
      expectedPid: receipt.session.pid,
      actualPid: receipt.session.pid,
      expectedGeneration: receipt.session.generation,
      actualGeneration: receipt.session.generation,
    });
    expect(alive(receipt.session.pid)).toBe(false);
    expect(alive(sentinelPid)).toBe(true);
    expect(receipt.artifactLifecycle).toMatchObject({
      schemaVersion: 1,
      phase: "committed",
      finalization: {
        kind: "strict-session-stop",
        writersFinalized: true,
      },
      allRequiredValid: true,
      allRecordedPathsReadable: true,
      missingRequired: [],
      invalidRequired: [],
    });
    const sessionDir = join(outputDir, "sessions", session);
    for (const artifact of receipt.artifactLifecycle.artifacts) {
      if (!artifact.readable) {
        expect(artifact.required).toBe(false);
        continue;
      }
      expect(existsSync(artifact.path)).toBe(true);
      expect(statSync(artifact.path).isFile()).toBe(true);
      expect(statSync(artifact.path).size).toBe(artifact.bytes);
      expect(createHash("sha256").update(readFileSync(artifact.path)).digest("hex")).toBe(
        artifact.sha256,
      );
      if (artifact.required) {
        expect(artifact.bytes).toBeGreaterThan(0);
        expect(artifact.validation.parsed).toBe(true);
        expect(artifact.validation.semanticallyNonEmpty).toBe(true);
        expect(artifact.validation.failures).toEqual([]);
      }
      const rel = relative(sessionDir, resolve(artifact.path));
      expect(rel === "" || (!rel.startsWith("..") && !isAbsolute(rel))).toBe(false);
    }
    const protocol = receipt.artifactLifecycle.artifacts.find(
      (artifact: any) => artifact.id === "protocol-responses",
    );
    expect(protocol.validation.correlation).toMatchObject({
      expected: receipt.artifactLifecycle.artifacts.find(
        (artifact: any) => artifact.id === "protocol-responses",
      ).validation.correlation.expected,
      missing: [],
      duplicates: [],
      unexpectedType: [],
    });
    expect(protocol.validation.correlation.matchedExactlyOnce).toBe(
      protocol.validation.correlation.expected,
    );
    for (const line of readFileSync(protocol.path, "utf8").trimEnd().split("\n")) {
      expect(() => JSON.parse(line)).not.toThrow();
    }
    const lifecycleArtifact = receipt.artifactLifecycle.artifacts.find(
      (artifact: any) => artifact.id === "lifecycle",
    );
    const lifecycle = JSON.parse(readFileSync(lifecycleArtifact.path, "utf8"));
    expect(lifecycle).toMatchObject({
      finalizationKind: "strict-session-stop",
      hidden: true,
      app: { pid: receipt.session.pid, dead: true },
      supervisor: { dead: true },
      forwarder: { dead: true },
      ownership: { exact: true },
      stop: { wasRunning: true, finalStatus: "not_found" },
    });
    expect(receipt.failurePreservation.reason).toContain(
      "injected run failure before delete measurement",
    );

    const status = spawnSync(
      "bash",
      ["scripts/agentic/session.sh", "status", session],
      {
        cwd: repoRoot,
        env: { ...env, SCRIPT_KIT_SESSION_DIR: join(outputDir, "sessions") },
        encoding: "utf8",
      },
    );
    expect(status.status).toBe(0);
    const statusReceipt = JSON.parse(status.stdout);
    expect(statusReceipt.status).toBe("not_found");
    expect(statusReceipt.alive).toBe(false);
  },
  80_000,
);

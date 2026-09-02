import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  chmodSync,
  closeSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import {
  approveStagingAnchor,
  atomicManagedJson,
  beginManagedTask,
  buildArtifactLifecycle,
  canonicalJson,
  claimOutput,
  commitFinalReceipt,
  createOwnedStagingDirectory,
  emptyOwnedCleanup,
  finalizeManagedTask,
  isRetiredManagedArtifact,
  listManagedTasks,
  managedKeepSet,
  managedRetentionPlan,
  materializeAtomic,
  OUTPUT_OWNER_FILE,
  pruneManagedRecords,
  readOwnedJson,
  registerManagedArtifactReference,
  registerManagedPublicationIntent,
  removeOwnedTree,
  retainLiveSessionArtifacts,
  sha256File,
  updateManagedTask,
  updateManagedKeepSet,
  updateManagedPublicationIntent,
  validateArtifact,
  validateOutputTarget,
  waitForProcessesDead,
  withManagedMetadata,
  writeJsonArtifactAtomic,
  type ArtifactReceipt,
  type ArtifactSpec,
  type ManagedTask,
  type RetentionCandidate,
} from "./artifact-lifecycle";
import { createArtifactFixture } from "./build-artifact-fixture";
import type { ArtifactReference } from "./build-artifact";

const repoRoot = resolve(import.meta.dir, "../..");
const roots: string[] = [];

function tempRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "artifact-lifecycle-test-"));
  roots.push(root);
  return root;
}

async function withProcessKill<T>(
  replacement: typeof process.kill,
  run: () => Promise<T>,
): Promise<T> {
  const original = process.kill;
  process.kill = replacement;
  try {
    return await run();
  } finally {
    process.kill = original;
  }
}

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("pure pre-mutation output validation", () => {
  test("rejects broad roots without creating ownership state", () => {
    for (const candidate of ["/", repoRoot, homedir(), tmpdir()]) {
      expect(() =>
        validateOutputTarget({
          repoRoot,
          candidate,
          kind: "directory",
          probeId: "safety-test",
        })
      ).toThrow();
    }
  });

  test("preserves a non-empty unowned directory and existing receipt", () => {
    const root = tempRoot();
    const output = join(root, "output");
    mkdirSync(output);
    const sentinel = join(output, "sentinel.txt");
    writeFileSync(sentinel, "preserve-me\n");
    expect(() =>
      validateOutputTarget({ repoRoot, candidate: output, kind: "directory", probeId: "test" })
    ).toThrow("absent or empty");
    expect(readFileSync(sentinel, "utf8")).toBe("preserve-me\n");
    expect(existsSync(join(output, ".artifact-lifecycle-owner.json"))).toBe(false);

    const receipt = join(root, "receipt.json");
    writeFileSync(receipt, "foreign receipt\n");
    expect(() =>
      validateOutputTarget({ repoRoot, candidate: receipt, kind: "receipt", probeId: "test" })
    ).toThrow("will not be overwritten");
    expect(readFileSync(receipt, "utf8")).toBe("foreign receipt\n");
  });

  test("rejects a symlinked output component that escapes the anchor", () => {
    const root = tempRoot();
    const outside = tempRoot();
    const link = join(root, "escape");
    symlinkSync(outside, link);
    expect(() =>
      validateOutputTarget({
        repoRoot,
        candidate: join(link, "receipt.json"),
        kind: "receipt",
        probeId: "test",
      })
    ).toThrow();
    expect(existsSync(join(outside, "receipt.json"))).toBe(false);
  });

  test("claims a fresh root with a run-specific ownership token", () => {
    const root = join(tempRoot(), "fresh");
    const plan = validateOutputTarget({
      repoRoot,
      candidate: root,
      kind: "directory",
      probeId: "claim-test",
    });
    expect(existsSync(root)).toBe(false);
    const claim = claimOutput(plan, "run-one");
    const owner = JSON.parse(readFileSync(claim.markerPath, "utf8"));
    expect(owner).toMatchObject({
      schemaVersion: 1,
      owner: "script-kit-gpui-probe",
      probeId: "claim-test",
      runId: "run-one",
      canonicalRoot: claim.owner.canonicalRoot,
    });
    expect(owner.token).toMatch(/^[0-9a-f-]{36}$/);
  });

  test("revalidates a validated target before claim without mutating through a swapped symlink", () => {
    const root = tempRoot();
    const outside = tempRoot();
    const parent = join(root, "parent");
    mkdirSync(parent);
    const target = join(parent, "claimed");
    const plan = validateOutputTarget({
      repoRoot,
      candidate: target,
      kind: "directory",
      probeId: "claim-swap-test",
    });
    renameSync(parent, join(root, "original-parent"));
    symlinkSync(outside, parent);

    expect(() => claimOutput(plan, "claim-swap-run")).toThrow();
    expect(existsSync(join(outside, "claimed"))).toBe(false);
  });

  test("rejects a symlinked receipt artifact root before creating the run directory", () => {
    const root = tempRoot();
    const outside = tempRoot();
    const receipt = join(root, "result.json");
    const plan = validateOutputTarget({
      repoRoot,
      candidate: receipt,
      kind: "receipt",
      probeId: "receipt-root-symlink-test",
    });
    symlinkSync(outside, join(root, "result-artifacts"));

    expect(() => claimOutput(plan, "receipt-symlink-run")).toThrow();
    expect(existsSync(join(outside, "receipt-symlink-run"))).toBe(false);
  });

  test("rejects an unapproved staging parent before mkdir", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "output"),
        kind: "directory",
        probeId: "staging-parent-test",
      }),
      "staging-parent-run",
    );
    const unapproved = join(root, "unapproved");
    mkdirSync(unapproved);

    expect(() => createOwnedStagingDirectory(claim, {
      name: "must-not-exist",
      anchor: {
        canonicalParent: unapproved,
        parentDevice: -1,
        parentInode: -1,
        claimRunId: claim.owner.runId,
        claimToken: claim.owner.token,
      },
    })).toThrow("approved");
    expect(existsSync(join(unapproved, "must-not-exist"))).toBe(false);
  });

  test("rejects a same-path replacement of an approved auxiliary staging parent", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "output"),
        kind: "directory",
        probeId: "staging-parent-replacement-test",
      }),
      "staging-parent-replacement-run",
    );
    const parent = createOwnedStagingDirectory(claim, { name: "approved-parent" });
    const anchor = approveStagingAnchor(claim, parent);
    renameSync(parent, join(root, "original-approved-parent"));
    mkdirSync(parent);
    const sentinel = join(parent, "foreign-sentinel.txt");
    writeFileSync(sentinel, "preserve foreign replacement\n");

    expect(() => createOwnedStagingDirectory(claim, {
      name: "must-not-exist",
      anchor,
    })).toThrow();
    expect(readFileSync(sentinel, "utf8")).toBe("preserve foreign replacement\n");
    expect(existsSync(join(parent, "must-not-exist"))).toBe(false);
  });

  test("recursive removal requires the fresh in-memory ownership token", () => {
    const root = join(tempRoot(), "owned");
    const claim = claimOutput(
      validateOutputTarget({ repoRoot, candidate: root, kind: "directory", probeId: "remove-test" }),
      "remove-run",
    );
    const sentinel = join(root, "sentinel.txt");
    writeFileSync(sentinel, "preserve-unless-owned\n");
    const wrongClaim = {
      ...claim,
      owner: { ...claim.owner, token: "wrong-token" },
    };
    expect(() => removeOwnedTree(wrongClaim)).toThrow("output claim identity changed");
    expect(readFileSync(sentinel, "utf8")).toBe("preserve-unless-owned\n");
    removeOwnedTree(claim);
    expect(existsSync(root)).toBe(false);
  });

  test("all five migrated CLIs reject unsafe output before launch or session start", () => {
    const helper = join(repoRoot, "scripts/agentic/macos-input.ts");
    const binarySha = createHash("sha256").update(readFileSync(process.execPath)).digest("hex");
    const helperSha = createHash("sha256").update(readFileSync(helper)).digest("hex");
    const referencePath = join(tempRoot(), "artifact-reference.json");
    const artifact = createArtifactFixture(repoRoot, { existingRepository: true });
    try {
      writeFileSync(referencePath, JSON.stringify(artifact.reference));
      const cases = [
        ["scripts/agentic/main-menu-focus-flicker.ts", "--out", "/"],
        [
          "scripts/agentic/root-search-frame-stability.ts",
          "--artifact",
          referencePath,
          "--receipt",
          "/",
        ],
        ["scripts/agentic/root-delete-key-benchmark.ts", "--output-dir", "/"],
        ["scripts/agentic/root-typing-lag-benchmark.ts", "--output-dir", "/"],
        [
          "scripts/agentic/native-escape-delivery-probe.ts",
          "--binary",
          process.execPath,
          "--expected-sha256",
          binarySha,
          "--helper",
          helper,
          "--expected-helper-sha256",
          helperSha,
          "--out",
          "/",
        ],
      ];
      for (const args of cases) {
        const result = Bun.spawnSync([process.execPath, ...args], {
          cwd: repoRoot,
          env: { ...process.env, SCRIPT_KIT_NONINTERACTIVE: "1" },
          stdout: "pipe",
          stderr: "pipe",
        });
        expect(result.exitCode, args[0]).not.toBe(0);
        expect(result.stderr.toString(), args[0]).toContain("unsafe");
      }
    } finally {
      artifact.dispose();
    }
  });
});

describe("writer process finalization", () => {
  test("reports a writer dead when the zero-signal probe returns ESRCH", async () => {
    await withProcessKill(
      ((pid: number, signal: NodeJS.Signals | number) => {
        expect(pid).toBe(4242);
        expect(signal).toBe(0);
        throw Object.assign(new Error("no such process"), { code: "ESRCH" });
      }) as typeof process.kill,
      async () => {
        expect(await waitForProcessesDead({ writer: 4242 }, { timeoutMs: 0 })).toEqual({
          writer: true,
        });
      },
    );
  });

  test("treats EPERM as alive or unverifiable and cannot report writers dead", async () => {
    await withProcessKill(
      ((pid: number, signal: NodeJS.Signals | number) => {
        expect(pid).toBe(4343);
        expect(signal).toBe(0);
        throw Object.assign(new Error("operation not permitted"), { code: "EPERM" });
      }) as typeof process.kill,
      async () => {
        expect(
          waitForProcessesDead({ writer: 4343 }, { timeoutMs: 0, pollMs: 1 }),
        ).rejects.toThrow("writers still alive after 0ms: writer");
      },
    );
  });

  test("missing and invalid PIDs are never probed or reported dead", async () => {
    let probes = 0;
    await withProcessKill(
      ((() => {
        probes += 1;
        return true;
      }) as unknown) as typeof process.kill,
      async () => {
        const result = await waitForProcessesDead({
          missing: undefined,
          absent: null,
          zero: 0,
          negative: -1,
          fractional: 1.5,
          notANumber: Number.NaN,
        });
        expect(result).toEqual({});
        expect(Object.values(result).some((dead) => dead === true)).toBe(false);
        expect(probes).toBe(0);
      },
    );
  });
});

describe("post-finalization materialization", () => {
  test("rejects source and destination artifact leaf traversal", () => {
    const root = tempRoot();
    const sessionDir = join(root, "session");
    mkdirSync(sessionDir);
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "durable"),
        kind: "directory",
        probeId: "traversal",
      }),
      "traversal-run",
    );
    const stagingDir = createOwnedStagingDirectory(claim);
    const outsideSource = join(root, "outside.log");
    writeFileSync(outsideSource, "outside\n");

    const sourceTraversal: ArtifactSpec = {
      id: "source-traversal",
      sourceName: "../outside.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    expect(() =>
      retainLiveSessionArtifacts(claim, sessionDir, stagingDir, [sourceTraversal])
    ).toThrow("leaf");
    expect(existsSync(join(stagingDir, "outside.log"))).toBe(false);

    writeFileSync(join(sessionDir, "inside.log"), "inside\n");
    const destinationTraversal: ArtifactSpec = {
      id: "destination-traversal",
      sourceName: "inside.log",
      destinationName: "../escaped.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    expect(() =>
      retainLiveSessionArtifacts(claim, sessionDir, stagingDir, [destinationTraversal])
    ).toThrow("leaf");
    expect(existsSync(join(root, "escaped.log"))).toBe(false);
  });

  test("fails no-replace when a destination appears at materialize commit", () => {
    const root = tempRoot();
    const source = join(root, "source.log");
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "durable"),
        kind: "directory",
        probeId: "materialize-race",
      }),
      "materialize-race",
    );
    const destination = join(claim.artifactsRoot, "app.log");
    writeFileSync(source, "owned artifact\n");

    expect(() =>
      materializeAtomic(claim, {
        sourceRoot: root,
        sourceName: "source.log",
        destinationName: "app.log",
      }, {
        beforeCommit() {
          writeFileSync(destination, "foreign artifact\n", { flag: "wx" });
        },
      })
    ).toThrow();
    expect(readFileSync(destination, "utf8")).toBe("foreign artifact\n");
  });

  test("does not unlink a foreign replacement of the materialize temporary", () => {
    const root = tempRoot();
    const source = join(root, "source.log");
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "durable"),
        kind: "directory",
        probeId: "materialize-temp-swap",
      }),
      "materialize-temp-swap-run",
    );
    const destination = join(claim.artifactsRoot, "app.log");
    let replacement = "";
    writeFileSync(source, "owned artifact\n");

    expect(() =>
      materializeAtomic(claim, {
        sourceRoot: root,
        sourceName: "source.log",
        destinationName: "app.log",
      }, {
        beforeCommit() {
          const temporaryName = readdirSync(claim.artifactsRoot).find((name) =>
            name.startsWith(".app.log.tmp-")
          );
          if (!temporaryName) throw new Error("materialize temporary not found");
          replacement = join(claim.artifactsRoot, temporaryName);
          renameSync(replacement, join(root, "displaced-materialize-temp"));
          writeFileSync(replacement, "foreign materialize sentinel\n", { flag: "wx" });
          throw new Error("injected materialize failure after temp swap");
        },
      })
    ).toThrow("injected materialize failure after temp swap");
    expect(existsSync(destination)).toBe(false);
    expect(readFileSync(replacement, "utf8")).toBe("foreign materialize sentinel\n");
  });

  test("a retained inode includes bytes appended after retention", async () => {
    const root = tempRoot();
    const sessionDir = join(root, "session");
    const durableDir = join(root, "durable");
    mkdirSync(sessionDir);
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: durableDir,
        kind: "directory",
        probeId: "retained-inode",
      }),
      "run-retained",
    );
    const stagingDir = createOwnedStagingDirectory(claim);
    const source = join(sessionDir, "protocol-responses.ndjson");
    const ready = join(root, "ready");
    const proceed = join(root, "proceed");
    const writer = Bun.spawn(
      [
        process.execPath,
        "-e",
        `import { appendFileSync, existsSync, writeFileSync } from "node:fs";
         appendFileSync(${JSON.stringify(source)}, JSON.stringify({requestId:"a",type:"stateResult",protocolVersion:2}) + "\\n");
         writeFileSync(${JSON.stringify(ready)}, "ready");
         while (!existsSync(${JSON.stringify(proceed)})) await Bun.sleep(5);
         appendFileSync(${JSON.stringify(source)}, JSON.stringify({requestId:"b",type:"waitForResult",protocolVersion:2}) + "\\n");`,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    const deadline = Date.now() + 2_000;
    while (!existsSync(ready) && Date.now() < deadline) await Bun.sleep(5);
    expect(existsSync(ready)).toBe(true);

    const spec: ArtifactSpec = {
      id: "protocol",
      sourceName: "protocol-responses.ndjson",
      required: true,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      correlations: [
        { requestId: "a", expectedType: "stateResult" },
        { requestId: "b", expectedType: "waitForResult" },
      ],
    };
    const [retained] = retainLiveSessionArtifacts(claim, sessionDir, stagingDir, [spec]);
    writeFileSync(proceed, "go");
    expect(await writer.exited).toBe(0);
    unlinkSync(source);

    materializeAtomic(claim, {
      sourceRoot: stagingDir,
      sourceName: "protocol-responses.ndjson",
      destinationName: "protocol-responses.ndjson",
    });
    const durable = join(claim.artifactsRoot, "protocol-responses.ndjson");
    const artifact = validateArtifact(durable, spec, claim.artifactsRoot);
    expect(artifact.bytes).toBe(readFileSync(durable).byteLength);
    expect(artifact.sha256).toBe(sha256File(durable));
    expect(artifact.validation.recordCount).toBe(2);
    expect(artifact.validation.correlation?.matchedExactlyOnce).toBe(2);
    expect(artifact.validation.failures).toEqual([]);
  });
});

describe("required and optional artifact validation", () => {
  const requiredLog: ArtifactSpec = {
    id: "app-log",
    sourceName: "app.log",
    required: true,
    mediaType: "text/plain",
    kind: "text",
    acceptedTextMarkers: ["STARTUP_READY ", "APP_READY|"],
  };
  const requiredProtocol: ArtifactSpec = {
    id: "protocol",
    sourceName: "protocol-responses.ndjson",
    required: true,
    mediaType: "application/x-ndjson",
    kind: "ndjson",
    correlations: [{ requestId: "expected", expectedType: "stateResult" }],
  };

  test("outside and symlinked durable artifact paths fail closed", () => {
    const root = tempRoot();
    const durableRoot = join(root, "durable");
    mkdirSync(durableRoot);
    const outsidePath = join(root, "outside.log");
    writeFileSync(outsidePath, "outside evidence\n");

    const outside = validateArtifact(outsidePath, requiredLog, durableRoot);
    expect(outside.readable).toBe(false);
    expect(outside.bytes).toBe(0);
    expect(outside.sha256).toBe("");
    expect(outside.validation.failures).toContain("artifact is outside owned durable root");

    const symlinkedParent = join(durableRoot, "linked-outside");
    symlinkSync(root, symlinkedParent);
    const symlinkPath = join(symlinkedParent, "outside.log");
    const symlinked = validateArtifact(symlinkPath, requiredLog, durableRoot);
    expect(symlinked.readable).toBe(false);
    expect(symlinked.bytes).toBe(0);
    expect(symlinked.sha256).toBe("");
    expect(symlinked.validation.failures).toContain("artifact path contains symlink component");
  });

  test("binds validation to the declared durable destination leaf", () => {
    const root = tempRoot();
    const expectedPath = join(root, "durable.log");
    const wrongPath = join(root, "wrong.log");
    const spec: ArtifactSpec = {
      id: "durable-name",
      sourceName: "source.log",
      destinationName: "durable.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    writeFileSync(expectedPath, "expected evidence\n");
    writeFileSync(wrongPath, "expected evidence\n");

    const wrong = validateArtifact(wrongPath, spec, root);
    expect(wrong.readable).toBe(false);
    expect(wrong.path).toBe(expectedPath);
    expect(wrong.relativePath).toBe("durable.log");
    expect(wrong.validation.failures).toContain(
      "artifact path does not match expected durable destination",
    );

    const expected = validateArtifact(expectedPath, spec, root);
    expect(expected.readable).toBe(true);
    expect(expected.path).toBe(expectedPath);
    expect(expected.relativePath).toBe("durable.log");
  });

  test("missing/empty/malformed required evidence fails closed", () => {
    const root = tempRoot();
    const missing = validateArtifact(join(root, requiredLog.sourceName), requiredLog, root);
    expect(missing.readable).toBe(false);
    expect(missing.validation.failures.length).toBeGreaterThan(0);

    const empty = join(root, requiredProtocol.sourceName);
    writeFileSync(empty, "");
    const emptyReceipt = validateArtifact(empty, requiredProtocol, root);
    expect(emptyReceipt.validation.failures).toContain("artifact is semantically empty");

    writeFileSync(empty, '{"requestId":"expected"');
    expect(validateArtifact(empty, requiredProtocol, root).validation.failures).toEqual(
      expect.arrayContaining([
        "truncated NDJSON final line",
        "artifact is semantically empty",
      ]),
    );
  });

  test("non-object NDJSON records fail closed without throwing", () => {
    const cases: Array<[string, unknown]> = [
      ["null", null],
      ["array", []],
      ["string", "response"],
      ["number", 42],
    ];

    for (const [kind, value] of cases) {
      const root = tempRoot();
      const claim = claimOutput(
        validateOutputTarget({
          repoRoot,
          candidate: join(root, "output"),
          kind: "directory",
          probeId: `non-object-${kind}`,
        }),
        `non-object-${kind}-run`,
      );
      mkdirSync(claim.artifactsRoot, { recursive: true });
      const path = join(claim.artifactsRoot, requiredProtocol.sourceName);
      writeFileSync(path, `${JSON.stringify(value)}\n`);

      expect(() => validateArtifact(path, requiredProtocol, claim.artifactsRoot)).not.toThrow();
      const artifact = validateArtifact(path, requiredProtocol, claim.artifactsRoot);
      expect(artifact.validation.parsed, kind).toBe(false);
      expect(artifact.validation.recordCount, kind).toBe(1);
      expect(artifact.validation.failures, kind).toContain(
        `non-object NDJSON record at line 1: ${kind}`,
      );
      expect(artifact.validation.correlation, kind).toMatchObject({
        matchedExactlyOnce: 0,
        missing: ["expected"],
        duplicates: [],
        unexpectedType: [],
      });

      const lifecycle = buildArtifactLifecycle({
        claim,
        finalizationKind: "driver-close",
        writersFinalized: true,
        specs: [requiredProtocol],
        artifacts: [artifact],
      });
      expect(lifecycle.invalidRequired, kind).toEqual(["protocol"]);
      expect(lifecycle.allRequiredValid, kind).toBe(false);
    }
  });

  test("mixed NDJSON preserves object correlations but remains invalid", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "output"),
        kind: "directory",
        probeId: "mixed-non-object",
      }),
      "mixed-non-object-run",
    );
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, requiredProtocol.sourceName);
    const records = [
      { requestId: "expected", type: "stateResult", protocolVersion: 2 },
      null,
      ["expected", "stateResult"],
      "expected",
      42,
    ];
    writeFileSync(path, `${records.map((record) => JSON.stringify(record)).join("\n")}\n`);

    const artifact = validateArtifact(path, requiredProtocol, claim.artifactsRoot);
    expect(artifact.validation.parsed).toBe(false);
    expect(artifact.validation.recordCount).toBe(5);
    expect(artifact.validation.correlation).toEqual({
      expected: 1,
      matchedExactlyOnce: 1,
      missing: [],
      duplicates: [],
      unexpectedType: [],
    });
    expect(artifact.validation.failures).toEqual([
      "non-object NDJSON record at line 2: null",
      "non-object NDJSON record at line 3: array",
      "non-object NDJSON record at line 4: string",
      "non-object NDJSON record at line 5: number",
    ]);

    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [requiredProtocol],
      artifacts: [artifact],
    });
    expect(lifecycle.invalidRequired).toEqual(["protocol"]);
    expect(lifecycle.allRequiredValid).toBe(false);
  });

  test("missing, duplicate, and wrong-type correlations are distinct failures", () => {
    const root = tempRoot();
    const path = join(root, requiredProtocol.sourceName);
    const response = { requestId: "expected", type: "stateResult", protocolVersion: 2 };
    writeFileSync(path, `${JSON.stringify(response)}\n`);
    const valid = validateArtifact(path, requiredProtocol, root);
    expect(valid.validation.failures).toEqual([]);
    expect(valid.validation.correlation).toEqual({
      expected: 1, matchedExactlyOnce: 1, missing: [], duplicates: [], unexpectedType: [],
    });

    const cases = [
      {
        records: [{ ...response, requestId: "other" }],
        missing: ["expected"], duplicates: [], unexpectedType: [],
        failure: "missing correlations: expected",
      },
      {
        records: [response, response],
        missing: [], duplicates: ["expected"], unexpectedType: [],
        failure: "duplicate correlations: expected",
      },
      {
        records: [{ ...response, type: "waitForResult" }],
        missing: [], duplicates: [], unexpectedType: ["expected:terminal-protocol-correlation-mismatch"],
        failure: "wrong response types: expected:terminal-protocol-correlation-mismatch",
      },
    ];
    for (const { records, missing, duplicates, unexpectedType, failure } of cases) {
      writeFileSync(path, `${records.map((record) => JSON.stringify(record)).join("\n")}\n`);
      const artifact = validateArtifact(path, requiredProtocol, root);
      expect(artifact.validation.correlation).toEqual({
        expected: 1, matchedExactlyOnce: 0, missing, duplicates, unexpectedType,
      });
      expect(artifact.validation.failures).toEqual([failure]);
    }
  });

  test("artifact IDs and requiredness form a strict one-to-one contract", () => {
    const root = tempRoot();
    const output = join(root, "identity-output");
    const claim = claimOutput(
      validateOutputTarget({ repoRoot, candidate: output, kind: "directory", probeId: "identity" }),
      "identity-run",
    );
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, "shared.txt");
    writeFileSync(path, "valid optional evidence\n");
    const requiredSpec: ArtifactSpec = {
      id: "shared",
      sourceName: "required.txt",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    const optionalSpec: ArtifactSpec = {
      id: "shared",
      sourceName: "shared.txt",
      required: false,
      mediaType: "text/plain",
      kind: "text",
    };
    const optionalReceipt = validateArtifact(path, optionalSpec, claim.artifactsRoot);

    expect(() => buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [requiredSpec, optionalSpec],
      artifacts: [optionalReceipt],
    })).toThrow("duplicate artifact spec id: shared");

    expect(() => buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [optionalSpec],
      artifacts: [optionalReceipt, optionalReceipt],
    })).toThrow("duplicate artifact receipt id: shared");

    expect(() => buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [requiredSpec],
      artifacts: [optionalReceipt],
    })).toThrow("artifact requiredness mismatch for shared");
  });

  test("artifact receipt identity must exactly match its declaring spec", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "identity-fields-output"),
        kind: "directory",
        probeId: "identity-fields",
      }),
      "identity-fields-run",
    );
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const spec: ArtifactSpec = {
      id: "evidence",
      sourceName: "source.log",
      destinationName: "durable.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    const path = join(claim.artifactsRoot, "durable.log");
    writeFileSync(path, "valid evidence\n");
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);

    for (const [field, value] of [
      ["sourceName", "other-source.log"],
      ["destinationName", "other-durable.log"],
      ["kind", "json"],
      ["mediaType", "application/json"],
    ] as const) {
      const mismatched = {
        ...artifact,
        identity: { ...artifact.identity, [field]: value },
      };
      expect(() => buildArtifactLifecycle({
        claim,
        finalizationKind: "driver-close",
        writersFinalized: true,
        specs: [spec],
        artifacts: [mismatched],
      }), field).toThrow(`artifact identity mismatch for evidence: ${field}`);
    }
  });

  test("fresh lifecycle validation rejects a caller receipt redirected outside durable storage", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "redirected-receipt-output"),
        kind: "directory",
        probeId: "redirected-receipt",
      }),
      "redirected-receipt-run",
    );
    const spec: ArtifactSpec = {
      id: "evidence",
      sourceName: "source.log",
      destinationName: "durable.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const durablePath = join(claim.artifactsRoot, "durable.log");
    const outsidePath = join(root, "outside.log");
    writeFileSync(durablePath, "same evidence\n");
    writeFileSync(outsidePath, "same evidence\n");
    const artifact = validateArtifact(durablePath, spec, claim.artifactsRoot);
    const redirected = { ...artifact, path: outsidePath };

    expect(() => buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [redirected],
    })).toThrow("artifact receipt does not match fresh validation for evidence");
  });

  test("lifecycle artifacts are deep snapshots independent of caller receipts", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "snapshot-output"),
        kind: "directory",
        probeId: "snapshot",
      }),
      "snapshot-run",
    );
    const spec: ArtifactSpec = {
      id: "evidence",
      sourceName: "evidence.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, "evidence.log");
    writeFileSync(path, "snapshot evidence\n");
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });
    const snapshot = JSON.stringify(lifecycle.artifacts[0]);

    artifact.sha256 = "0".repeat(64);
    artifact.identity.destinationName = "mutated.log";
    artifact.validation.failures.push("caller mutation");

    expect(JSON.stringify(lifecycle.artifacts[0])).toBe(snapshot);
  });

  test("lifecycle artifacts are canonicalized into declaring-spec order", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "canonical-order-output"),
        kind: "directory",
        probeId: "canonical-order",
      }),
      "canonical-order-run",
    );
    const firstSpec: ArtifactSpec = {
      id: "first",
      sourceName: "first.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    const secondSpec: ArtifactSpec = {
      id: "second",
      sourceName: "second.log",
      required: false,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const firstPath = join(claim.artifactsRoot, "first.log");
    const secondPath = join(claim.artifactsRoot, "second.log");
    writeFileSync(firstPath, "first evidence\n");
    writeFileSync(secondPath, "second evidence\n");
    const first = validateArtifact(firstPath, firstSpec, claim.artifactsRoot);
    const second = validateArtifact(secondPath, secondSpec, claim.artifactsRoot);

    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [firstSpec, secondSpec],
      artifacts: [second, first],
    });

    expect(lifecycle.artifacts.map((artifact) => artifact.id)).toEqual(["first", "second"]);
    expect(lifecycle.recordedPaths).toEqual([firstPath, secondPath]);
  });

  test("lifecycle comparison covers the complete receipt and nested validation evidence", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "complete-comparison-output"),
        kind: "directory",
        probeId: "complete-comparison",
      }),
      "complete-comparison-run",
    );
    const spec: ArtifactSpec = {
      id: "protocol",
      sourceName: "protocol.ndjson",
      required: true,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      correlations: [{ requestId: "expected", expectedType: "stateResult" }],
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, "protocol.ndjson");
    writeFileSync(path, `${JSON.stringify({ requestId: "expected", type: "stateResult", protocolVersion: 2 })}\n`);
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);
    expect(artifact.validation.failures).toEqual([]);
    const mutations: Array<[string, string, (receipt: ArtifactReceipt) => void]> = [
      ["path", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.path = join(root, "outside.ndjson");
      }],
      ["relativePath", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.relativePath = "other.ndjson";
      }],
      ["mediaType", "artifact identity mismatch for protocol: mediaType", (receipt) => {
        receipt.mediaType = "text/plain";
      }],
      ["bytes", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.bytes += 1;
      }],
      ["sha256", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.sha256 = "0".repeat(64);
      }],
      ["finalizedAfterWriters", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        (receipt as { finalizedAfterWriters: boolean }).finalizedAfterWriters = false;
      }],
      ["readable", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.readable = false;
      }],
      ["validation.parsed", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.validation.parsed = false;
      }],
      ["validation.failures", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.validation.failures.push("mutated");
      }],
      ["validation.correlation", "artifact receipt does not match fresh validation for protocol", (receipt) => {
        receipt.validation.correlation!.missing.push("mutated");
      }],
    ];

    for (const [field, expectedError, mutate] of mutations) {
      const mismatched = structuredClone(artifact);
      mutate(mismatched);
      expect(() => buildArtifactLifecycle({
        claim,
        finalizationKind: "driver-close",
        writersFinalized: true,
        specs: [spec],
        artifacts: [mismatched],
      }), field).toThrow(expectedError);
    }
  });

  test("final receipt commit rejects a same-ID artifact with changed identity", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "commit-identity-output"),
        kind: "directory",
        probeId: "commit-identity",
      }),
      "commit-identity-run",
    );
    const spec: ArtifactSpec = {
      id: "evidence",
      sourceName: "source.log",
      destinationName: "durable.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, "durable.log");
    writeFileSync(path, "valid evidence\n");
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });
    const mismatched = {
      ...artifact,
      identity: { ...artifact.identity, mediaType: "application/json" },
    };

    expect(() => commitFinalReceipt(
      claim,
      { status: "pass", artifactLifecycle: lifecycle },
      [spec],
      [mismatched],
    )).toThrow("final receipt artifact identity mismatch for evidence: mediaType");
    expect(existsSync(claim.receiptPath)).toBe(false);
  });

  test("final receipt rejects a symlink replacement made before commit", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "precommit-symlink-output"),
        kind: "directory",
        probeId: "precommit-symlink",
      }),
      "precommit-symlink-run",
    );
    const spec: ArtifactSpec = {
      id: "evidence",
      sourceName: "evidence.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, "evidence.log");
    const outside = join(root, "outside.log");
    writeFileSync(path, "same evidence\n");
    writeFileSync(outside, "same evidence\n");
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });
    unlinkSync(path);
    symlinkSync(outside, path);

    expect(() => commitFinalReceipt(
      claim,
      { status: "pass", artifactLifecycle: lifecycle },
      [spec],
      [artifact],
    )).toThrow("fresh validation");
    expect(existsSync(claim.receiptPath)).toBe(false);
  });

  test("final receipt revalidates after beforeCommit and rejects a symlink replacement", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "hook-symlink-output"),
        kind: "directory",
        probeId: "hook-symlink",
      }),
      "hook-symlink-run",
    );
    const spec: ArtifactSpec = {
      id: "evidence",
      sourceName: "evidence.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const path = join(claim.artifactsRoot, "evidence.log");
    const outside = join(root, "outside.log");
    writeFileSync(path, "same evidence\n");
    writeFileSync(outside, "same evidence\n");
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });

    expect(() => commitFinalReceipt(
      claim,
      { status: "pass", artifactLifecycle: lifecycle },
      [spec],
      [artifact],
      {
        beforeCommit() {
          unlinkSync(path);
          symlinkSync(outside, path);
        },
      },
    )).toThrow("fresh validation");
    expect(existsSync(claim.receiptPath)).toBe(false);
  });

  test("zero-byte optional evidence is allowed while lifecycle JSON remains required", () => {
    const root = tempRoot();
    const output = join(root, "output");
    const claim = claimOutput(
      validateOutputTarget({ repoRoot, candidate: output, kind: "directory", probeId: "aggregate" }),
      "aggregate-run",
    );
    const optionalSpec: ArtifactSpec = {
      id: "raw-lifecycle",
      sourceName: "lifecycle.ndjson",
      required: false,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      requireNonEmpty: false,
    };
    const lifecycleSpec: ArtifactSpec = {
      id: "lifecycle",
      sourceName: "lifecycle.json",
      required: true,
      mediaType: "application/json",
      kind: "json",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const optionalPath = join(claim.artifactsRoot, "lifecycle.ndjson");
    const lifecyclePath = join(claim.artifactsRoot, "lifecycle.json");
    writeFileSync(optionalPath, "");
    writeJsonArtifactAtomic(claim, "lifecycle.json", { writersFinalized: true });
    const artifacts = [
      validateArtifact(optionalPath, optionalSpec, claim.artifactsRoot),
      validateArtifact(lifecyclePath, lifecycleSpec, claim.artifactsRoot),
    ];
    expect(artifacts[0].validation.failures).toEqual([]);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [optionalSpec, lifecycleSpec],
      artifacts,
    });
    expect(lifecycle.allRequiredValid).toBe(true);
    expect(lifecycle.allRecordedPathsReadable).toBe(true);

    const receipt = { status: "pass", artifactLifecycle: lifecycle };
    commitFinalReceipt(claim, receipt, [optionalSpec, lifecycleSpec], artifacts);
    expect(JSON.parse(readFileSync(claim.receiptPath, "utf8")).status).toBe("pass");
    for (const artifact of artifacts) {
      expect(createHash("sha256").update(readFileSync(artifact.path)).digest("hex")).toBe(
        artifact.sha256,
      );
    }
  });

  test("publishes with one missing optional receipt while required evidence remains valid", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "missing-optional-output"),
        kind: "directory",
        probeId: "missing-optional",
      }),
      "missing-optional-run",
    );
    const requiredSpec: ArtifactSpec = {
      id: "required",
      sourceName: "required.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    const optionalSpec: ArtifactSpec = {
      id: "optional",
      sourceName: "optional.log",
      required: false,
      mediaType: "text/plain",
      kind: "text",
      requireNonEmpty: false,
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const requiredPath = join(claim.artifactsRoot, "required.log");
    writeFileSync(requiredPath, "required evidence\n");
    const artifacts = [
      validateArtifact(requiredPath, requiredSpec, claim.artifactsRoot),
      validateArtifact(join(claim.artifactsRoot, "optional.log"), optionalSpec, claim.artifactsRoot),
    ];
    expect(artifacts[1].readable).toBe(false);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [requiredSpec, optionalSpec],
      artifacts,
    });
    expect(lifecycle.allRequiredValid).toBe(true);

    commitFinalReceipt(
      claim,
      { status: "pass", artifactLifecycle: lifecycle },
      [requiredSpec, optionalSpec],
      artifacts,
    );
    expect(JSON.parse(readFileSync(claim.receiptPath, "utf8")).status).toBe("pass");
  });

  test("rejects forged lifecycle success aggregates for unreadable required evidence", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "forged-success-output"),
        kind: "directory",
        probeId: "forged-success",
      }),
      "forged-success-run",
    );
    const spec: ArtifactSpec = {
      id: "required",
      sourceName: "missing-required.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const artifact = validateArtifact(
      join(claim.artifactsRoot, spec.sourceName),
      spec,
      claim.artifactsRoot,
    );
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });
    lifecycle.missingRequired = [];
    lifecycle.invalidRequired = [];
    lifecycle.allRequiredValid = true;
    lifecycle.recordedPaths = [artifact.path];
    lifecycle.allRecordedPathsReadable = true;

    expect(() => commitFinalReceipt(
      claim,
      { status: "pass", artifactLifecycle: lifecycle },
      [spec],
      [artifact],
    )).toThrow("lifecycle");
    expect(existsSync(claim.receiptPath)).toBe(false);
  });

  test("publishes a truthful failure receipt for unreadable required evidence", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "truthful-failure-output"),
        kind: "directory",
        probeId: "truthful-failure",
      }),
      "truthful-failure-run",
    );
    const spec: ArtifactSpec = {
      id: "required",
      sourceName: "missing-required.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const artifact = validateArtifact(
      join(claim.artifactsRoot, spec.sourceName),
      spec,
      claim.artifactsRoot,
    );
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });

    commitFinalReceipt(
      claim,
      { status: "fail", failure: "required evidence unavailable", artifactLifecycle: lifecycle },
      [spec],
      [artifact],
    );

    const published = JSON.parse(readFileSync(claim.receiptPath, "utf8"));
    expect(published.status).toBe("fail");
    expect(published.artifactLifecycle).toEqual(lifecycle);
    expect(published.artifactLifecycle.allRequiredValid).toBe(false);
  });

  test("rejects in-place mutation of final receipt temporary bytes", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "receipt-temp-mutation-output"),
        kind: "directory",
        probeId: "receipt-temp-mutation",
      }),
      "receipt-temp-mutation-run",
    );
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [],
      artifacts: [],
    });

    expect(() => commitFinalReceipt(
      claim,
      { status: "pass", artifactLifecycle: lifecycle },
      [],
      [],
      {
        beforeCommit() {
          const temporaryName = readdirSync(claim.root).find((name) =>
            name.startsWith(".receipt.json.tmp-")
          );
          if (!temporaryName) throw new Error("receipt temporary not found");
          writeFileSync(join(claim.root, temporaryName), "forged receipt bytes\n");
        },
      },
    )).toThrow("bytes changed");
    expect(existsSync(claim.receiptPath)).toBe(false);
  });

  test("final receipt byte comparison does not dispatch through mutable Buffer.prototype.equals", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "receipt-buffer-equals-output"),
        kind: "directory",
        probeId: "receipt-buffer-equals",
      }),
      "receipt-buffer-equals-run",
    );
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [],
      artifacts: [],
    });
    const originalEquals = Buffer.prototype.equals;
    const forgedBytes = "forged receipt bytes\n";
    let temporaryPath = "";
    let mutableEqualsInvoked = false;
    let commitError: unknown;

    try {
      commitFinalReceipt(
        claim,
        { status: "pass", artifactLifecycle: lifecycle },
        [],
        [],
        {
          beforeCommit() {
            const temporaryName = readdirSync(claim.root).find((name) =>
              name.startsWith(".receipt.json.tmp-")
            );
            if (!temporaryName) throw new Error("receipt temporary not found");
            temporaryPath = join(claim.root, temporaryName);
            Buffer.prototype.equals = function equals(other: Uint8Array): boolean {
              mutableEqualsInvoked = true;
              const matched = originalEquals.call(this, other);
              if (matched) writeFileSync(temporaryPath, forgedBytes);
              return matched;
            };
          },
        },
      );
    } catch (error) {
      commitError = error;
    } finally {
      Buffer.prototype.equals = originalEquals;
    }

    expect(
      existsSync(claim.receiptPath) ? readFileSync(claim.receiptPath, "utf8") : undefined,
    ).not.toBe(forgedBytes);
    expect(commitError).toBeUndefined();
    expect(mutableEqualsInvoked).toBe(false);
    expect(JSON.parse(readFileSync(claim.receiptPath, "utf8")).status).toBe("pass");
    expect(existsSync(temporaryPath)).toBe(false);
  });

  test("rejects final receipt temporary mutation during post-hook artifact validation", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "receipt-validation-mutation-output"),
        kind: "directory",
        probeId: "receipt-validation-mutation",
      }),
      "receipt-validation-mutation-run",
    );
    const spec: ArtifactSpec = {
      id: "ready-log",
      sourceName: "ready.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
      acceptedTextMarkers: ["READY"],
    };
    mkdirSync(claim.artifactsRoot, { recursive: true });
    const artifactPath = join(claim.artifactsRoot, spec.sourceName);
    writeFileSync(artifactPath, "READY\n");
    const artifact = validateArtifact(artifactPath, spec, claim.artifactsRoot);
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [spec],
      artifacts: [artifact],
    });
    const originalIncludes = String.prototype.includes;
    let temporaryPath = "";

    try {
      expect(() => commitFinalReceipt(
        claim,
        { status: "pass", artifactLifecycle: lifecycle },
        [spec],
        [artifact],
        {
          beforeCommit() {
            const temporaryName = readdirSync(claim.root).find((name) =>
              name.startsWith(".receipt.json.tmp-")
            );
            if (!temporaryName) throw new Error("receipt temporary not found");
            temporaryPath = join(claim.root, temporaryName);
            String.prototype.includes = function includes(
              searchString: string,
              position?: number,
            ): boolean {
              if (String(this) === "READY\n" && searchString === "READY") {
                writeFileSync(temporaryPath, "forged receipt bytes\n");
              }
              return originalIncludes.call(String(this), searchString, position);
            };
          },
        },
      )).toThrow("bytes changed");
    } finally {
      String.prototype.includes = originalIncludes;
    }
    expect(existsSync(claim.receiptPath)).toBe(false);
  });

  test("fails no-replace when a receipt appears at final commit", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "output"),
        kind: "directory",
        probeId: "receipt-race",
      }),
      "receipt-race-run",
    );
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [],
      artifacts: [],
    });
    expect(() =>
      commitFinalReceipt(
        claim,
        { status: "pass", artifactLifecycle: lifecycle },
        [],
        [],
        {
          beforeCommit() {
            writeFileSync(claim.receiptPath, "foreign receipt\n", { flag: "wx" });
          },
        },
      )
    ).toThrow();
    expect(readFileSync(claim.receiptPath, "utf8")).toBe("foreign receipt\n");
  });

  test("does not unlink a foreign replacement of the final receipt temporary", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "output"),
        kind: "directory",
        probeId: "receipt-temp-swap",
      }),
      "receipt-temp-swap-run",
    );
    const lifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized: true,
      specs: [],
      artifacts: [],
    });
    let replacement = "";

    expect(() =>
      commitFinalReceipt(
        claim,
        { status: "pass", artifactLifecycle: lifecycle },
        [],
        [],
        {
          beforeCommit() {
            const temporaryName = readdirSync(claim.root).find((name) =>
              name.startsWith(".receipt.json.tmp-")
            );
            if (!temporaryName) throw new Error("receipt temporary not found");
            replacement = join(claim.root, temporaryName);
            renameSync(replacement, join(root, "displaced-receipt-temp"));
            writeFileSync(replacement, "foreign receipt sentinel\n", { flag: "wx" });
            throw new Error("injected receipt failure after temp swap");
          },
        },
      )
    ).toThrow("injected receipt failure after temp swap");
    expect(existsSync(claim.receiptPath)).toBe(false);
    expect(readFileSync(replacement, "utf8")).toBe("foreign receipt sentinel\n");
  });

  test("final receipt commit rejects an omitted artifact set", () => {
    const root = tempRoot();
    const claim = claimOutput(
      validateOutputTarget({
        repoRoot,
        candidate: join(root, "output"),
        kind: "directory",
        probeId: "receipt-artifacts",
      }),
      "receipt-artifacts-run",
    );
    const receipt = {
      status: "fail",
      artifactLifecycle: buildArtifactLifecycle({
        claim,
        finalizationKind: "driver-close",
        writersFinalized: false,
        specs: [],
        artifacts: [],
      }),
    };
    expect(() =>
      (commitFinalReceipt as unknown as (
        currentClaim: typeof claim,
        currentReceipt: Record<string, unknown>,
      ) => void)(claim, receipt)
    ).toThrow("artifact");
    expect(existsSync(claim.receiptPath)).toBe(false);
  });
});

describe("managed subtree retention", () => {
  function createRetentionTask(root: string, id: string, directory = id, references: readonly ArtifactReference[] = []): ManagedTask {
    const claim = claimOutput(validateOutputTarget({
      repoRoot: root, candidate: join(root, ".test-output", directory), kind: "directory", probeId: "retention-fixture",
    }), id);
    return beginManagedTask(claim, "runtime-run", references);
  }

  function createRetentionAuxiliaries(root: string, directory = "aux-owner") {
    const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output", directory),
      kind: "directory", probeId: "retention-fixture" }), "aux-owner");
    const task = beginManagedTask(claim, "runtime-run", []);
    const staging = createOwnedStagingDirectory(claim, { name: "staging" });
    const nested = createOwnedStagingDirectory(claim, { name: "nested", anchor: approveStagingAnchor(claim, staging) });
    mkdirSync(join(nested, "logs"));
    writeFileSync(join(nested, "logs/output.log"), "closed writer\n");
    finalizeManagedTask(task, emptyOwnedCleanup());
    return { claim, task, staging, nested };
  }

  test("closed tasks retire their exact nested auxiliary chain, not the unmanaged campaign", () => {
    const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root, "campaign/owner");
    const plan = managedRetentionPlan(root);
    expect(plan.candidates).toHaveLength(1);
    expect(plan.candidates[0].auxiliaries.map((entry: { path: string }) => entry.path)).toEqual([fixture.staging, fixture.nested]);
    expect(plan.candidates[0].auxiliaries.every((entry: { ownerTaskPath: string }) => entry.ownerTaskPath === fixture.claim.root)).toBe(true);
    const result = pruneManagedRecords(root, plan.revision, plan.candidates);
    expect(result.removed).toEqual([{ kind: "runtime-run", id: fixture.task.identity.id, generation: fixture.task.identity.generation }]);
    expect(existsSync(fixture.claim.root)).toBe(false);
    expect(existsSync(join(root, ".test-output/campaign"))).toBe(true);
    expect(readOwnedJson(join(root, ".test-output/managed-task-index.json"))).toEqual({});
    expect(readOwnedJson(result.receiptPath as string).candidates[0].auxiliaries).toHaveLength(2);
  });

  test.each(["valid", "unknown-child"] as const)("scans registered descendants below a blocked ancestor: %s", integrity => {
    const root = realpathSync(tempRoot());
    const ancestor = createRetentionTask(root, "ancestor");
    finalizeManagedTask(ancestor, emptyOwnedCleanup());
    const unregistered = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/ancestor/unregistered"),
      kind: "directory", probeId: "retention-fixture" }), "unregistered");
    const fixture = createRetentionAuxiliaries(root, "ancestor/unregistered/child");
    if (integrity === "unknown-child") claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(fixture.staging, "unknown"),
      kind: "directory", probeId: "retention-fixture" }), "unknown-child");
    const plan = managedRetentionPlan(root);
    if (integrity === "valid") {
      expect(plan.candidates.map((candidate: RetentionCandidate) => candidate.id)).toEqual([fixture.task.identity.id]);
      expect(plan.candidates[0].auxiliaries.map((entry: { path: string }) => entry.path)).toEqual([fixture.staging, fixture.nested]);
      expect(pruneManagedRecords(root, plan.revision, plan.candidates).removed).toHaveLength(1);
      expect(existsSync(fixture.claim.root)).toBe(false);
    } else {
      expect(plan.candidates).toEqual([]);
      expect(pruneManagedRecords(root, plan.revision, []).removed).toEqual([]);
      expect(existsSync(fixture.nested)).toBe(true);
    }
    expect(existsSync(ancestor.recordPath)).toBe(true);
    expect(existsSync(join(unregistered.root, OUTPUT_OWNER_FILE))).toBe(true);
  });

  test.each(["schemaVersion", "owner", "probeId", "runId", "token", "createdAt", "canonicalRoot", "canonicalParent", "markerKind"])(
    "protects an auxiliary with forged %s despite its surrounding ownership chain", field => {
      const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
      const markerPath = join(fixture.nested, OUTPUT_OWNER_FILE), marker = readOwnedJson(markerPath);
      marker[field] = "forged";
      atomicManagedJson(markerPath, marker);
      const plan = managedRetentionPlan(root);
      expect(plan.candidates).toEqual([]);
      expect(plan.protectedRecords.length).toBeGreaterThan(0);
      expect(pruneManagedRecords(root, plan.revision, []).removed).toEqual([]);
      expect(existsSync(fixture.nested)).toBe(true);
    },
  );

  test("an auxiliary cannot cross an unowned intermediate parent or another task", () => {
    const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
    const other = createRetentionTask(root, "other", "aux-owner/staging/other");
    finalizeManagedTask(other, emptyOwnedCleanup());
    const moved = join(dirname(other.recordPath), "stolen");
    renameSync(fixture.nested, moved);
    const markerPath = join(moved, OUTPUT_OWNER_FILE), marker = readOwnedJson(markerPath);
    atomicManagedJson(markerPath, { ...marker, canonicalRoot: moved, canonicalParent: dirname(moved) });
    expect(managedRetentionPlan(root).candidates).toEqual([]);
    renameSync(moved, fixture.nested);
    const plain = join(fixture.staging, "plain");
    mkdirSync(plain);
    renameSync(fixture.nested, join(plain, "nested"));
    atomicManagedJson(join(plain, "nested", OUTPUT_OWNER_FILE), { ...marker, canonicalRoot: join(plain, "nested"), canonicalParent: plain });
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.map((entry: RetentionCandidate) => entry.id)).toEqual(["other"]);
    expect(existsSync(join(plain, "nested"))).toBe(true);
  });

  test("selecting auxiliary ownership never selects a separate closed descendant task", () => {
    const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
    const child = createRetentionTask(root, "child", "aux-owner/staging/child");
    finalizeManagedTask(child, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root), parent = plan.candidates.find((entry: RetentionCandidate) => entry.id === fixture.task.identity.id);
    expect(() => pruneManagedRecords(root, plan.revision, [parent])).toThrow("retention_unselected_descendant");
    expect(existsSync(join(root, ".test-output/managed-retention.json"))).toBe(false);
    expect(pruneManagedRecords(root, plan.revision, plan.candidates).removed).toHaveLength(2);
  });

  test.each(["marker-bytes", "marker-inode", "directory-inode", "unknown-marker", "symlink"] as const)(
    "refuses after-plan auxiliary %s mutation before writing a journal", mutation => {
      const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
      const plan = managedRetentionPlan(root), markerPath = join(fixture.nested, OUTPUT_OWNER_FILE);
      const indexPath = join(root, ".test-output/managed-task-index.json"), indexBefore = readFileSync(indexPath, "utf8");
      if (mutation === "marker-bytes") writeFileSync(markerPath, `${readFileSync(markerPath, "utf8")} `);
      else if (mutation === "marker-inode") {
        renameSync(markerPath, join(root, "old-marker"));
        writeFileSync(markerPath, readFileSync(join(root, "old-marker")));
      } else if (mutation === "directory-inode") {
        renameSync(fixture.nested, join(root, "old-nested"));
        mkdirSync(fixture.nested);
        renameSync(join(root, "old-nested", OUTPUT_OWNER_FILE), markerPath);
      } else if (mutation === "unknown-marker") {
        mkdirSync(join(fixture.nested, "unknown"));
        atomicManagedJson(join(fixture.nested, "unknown", OUTPUT_OWNER_FILE), { ...fixture.claim.owner, canonicalRoot: join(fixture.nested, "unknown") });
      } else symlinkSync(root, join(fixture.nested, "link"));
      expect(() => pruneManagedRecords(root, plan.revision, plan.candidates)).toThrow("retention_plan_changed");
      expect(readFileSync(indexPath, "utf8")).toBe(indexBefore);
      expect(existsSync(join(root, ".test-output/managed-retention.json"))).toBe(false);
      expect(existsSync(fixture.claim.root)).toBe(true);
    },
  );

  test.each(["afterQuarantine", "beforeRemove"] as const)("nested auxiliaries remain recoverable after %s", checkpoint => {
    const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
    const plan = managedRetentionPlan(root), markerBefore = readFileSync(join(fixture.nested, OUTPUT_OWNER_FILE), "utf8");
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { [checkpoint](path: string) {
      quarantine = path;
      expect(readFileSync(join(path, "staging/nested", OUTPUT_OWNER_FILE), "utf8")).toBe(markerBefore);
      if (checkpoint === "beforeRemove") unlinkSync(join(path, "staging", OUTPUT_OWNER_FILE));
      throw new Error("auxiliary interruption");
    } })).toThrow("auxiliary interruption");
    const recovery = managedRetentionPlan(root);
    expect(readOwnedJson(recovery.recovery.path).plan.candidates[0].auxiliaries).toHaveLength(2);
    expect(pruneManagedRecords(root, recovery.revision, recovery.candidates).removed).toHaveLength(1);
    expect(existsSync(quarantine)).toBe(false);
  });

  test.each(["marker", "directory"] as const)("revalidates auxiliary %s inode after quarantine relocation", replaced => {
    const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
    const plan = managedRetentionPlan(root), indexPath = join(root, ".test-output/managed-task-index.json");
    const indexBefore = readFileSync(indexPath, "utf8");
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterQuarantine(path) {
      quarantine = path;
      const nested = join(path, "staging/nested"), markerPath = join(nested, OUTPUT_OWNER_FILE);
      if (replaced === "marker") {
        renameSync(markerPath, join(root, "old-marker"));
        writeFileSync(markerPath, readFileSync(join(root, "old-marker")));
      } else {
        renameSync(nested, join(root, "old-nested"));
        mkdirSync(nested);
        renameSync(join(root, "old-nested", OUTPUT_OWNER_FILE), markerPath);
      }
    } })).toThrow(replaced === "marker" ? "identity changed" : "retention_directory_changed");
    const recovery = managedRetentionPlan(root), journalBefore = readFileSync(recovery.recovery.path, "utf8");
    expect(() => pruneManagedRecords(root, recovery.revision, recovery.candidates)).toThrow("retention_recovery_protected");
    expect(readFileSync(recovery.recovery.path, "utf8")).toBe(journalBefore);
    expect(readFileSync(indexPath, "utf8")).toBe(indexBefore);
    expect(existsSync(quarantine)).toBe(true);
  });

  test("an open auxiliary descendant is never quarantined", () => {
    const root = realpathSync(tempRoot()), fixture = createRetentionAuxiliaries(root);
    const plan = managedRetentionPlan(root), fd = openSync(join(fixture.nested, "logs/output.log"), "r");
    try {
      expect(() => pruneManagedRecords(root, plan.revision, plan.candidates)).toThrow("retention_open_handles_present_or_unknown");
      expect(existsSync(fixture.nested)).toBe(true);
      expect(managedRetentionPlan(root).recovery.steps[0].phase).toBe("pending");
    } finally { closeSync(fd); }
  });

  test("managed JSON writers enforce the UTF8 byte limit including the newline before replacement", () => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), "retention-bytes-"))), path = join(root, "record.json");
    try {
      const limit = 8 * 1024 * 1024;
      const overhead = Buffer.byteLength(`${canonicalJson({ payload: "" })}\n`);
      const value = { payload: "é".repeat(Math.floor((limit - overhead) / 2)) + "x".repeat((limit - overhead) % 2) };
      atomicManagedJson(path, value);
      expect(lstatSync(path).size).toBe(limit);
      expect(readOwnedJson(path).payload).toBe(value.payload);
      const before = sha256File(path), inode = lstatSync(path).ino;
      value.payload += "x";
      expect(() => atomicManagedJson(path, value)).toThrow("managed_record_too_large");
      expect(sha256File(path)).toBe(before);
      expect(lstatSync(path).ino).toBe(inode);
      expect(readdirSync(root)).toEqual(["record.json"]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test.each(["journal", "history"] as const)("retirement preflights the exact %s limit before mutation and stays readable at the boundary", boundary => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), "retention-boundary-")));
    try {
      const limit = 8 * 1024 * 1024;
      // A long task id makes historical outcomes larger than the journal without large files or many tasks.
      const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/task"),
        kind: "directory", probeId: "retention-fixture" }), boundary === "history" ? "i".repeat(2048) : "task");
      const task = beginManagedTask(claim, "runtime-run", []);
      let auxiliary = createOwnedStagingDirectory(claim, { name: "a" });
      updateManagedTask(task, { result: { payload: "" } });
      finalizeManagedTask(task, emptyOwnedCleanup());
      const moveAuxiliary = (destination: string) => {
        renameSync(auxiliary, destination);
        auxiliary = destination;
        const markerPath = join(auxiliary, OUTPUT_OWNER_FILE), marker = readOwnedJson(markerPath);
        writeFileSync(markerPath, `${canonicalJson({ ...marker, canonicalRoot: auxiliary })}\n`);
      };
      const serializedSizes = (plan: Record<string, any>) => {
        const generation = "00000000-0000-0000-0000-000000000000";
        const selection = plan.candidates.map(({ kind, id, generation, revision, recordSha256, directoryDevice, directoryInode }: RetentionCandidate) =>
          ({ kind, id, generation, revision, recordSha256, directoryDevice, directoryInode }));
        const journal = { schemaVersion: 2, generation, plan, selection,
          steps: plan.candidates.map((candidate: { path: string }) => ({ quarantine: `${candidate.path}.quarantine-${generation}`, phase: "quarantined" })) };
        const receipt = { schemaVersion: 1, generation, expectedRevision: plan.revision, selection, candidates: plan.candidates,
          removed: selection.map(({ kind, id, generation }: RetentionCandidate) => ({ kind, id, generation })), withdrawn: [],
          replanRequired: false, physicalBytesReclaimed: null };
        return { journal: Buffer.byteLength(`${canonicalJson(journal)}\n`), history: Buffer.byteLength(`${canonicalJson(receipt)}\n`), journalValue: journal };
      };
      let plan = managedRetentionPlan(root), sizes = serializedSizes(plan);
      // The task snapshot occurs twice; the single auxiliary path adjusts odd-byte boundaries.
      if ((limit - sizes[boundary]) % 2) {
        moveAuxiliary(`${auxiliary}a`);
        plan = managedRetentionPlan(root);
        sizes = serializedSizes(plan);
      }
      const record = readOwnedJson(task.recordPath);
      record.result.payload = "x".repeat((limit - sizes[boundary]) / 2);
      atomicManagedJson(task.recordPath, record);
      plan = managedRetentionPlan(root);
      expect(serializedSizes(plan)[boundary]).toBe(limit);
      expect(serializedSizes(plan)[boundary === "journal" ? "history" : "journal"]).toBeLessThan(limit);
      const boundaryPath = auxiliary;
      moveAuxiliary(`${auxiliary}x`);
      plan = managedRetentionPlan(root);
      expect(serializedSizes(plan)[boundary]).toBe(limit + 1);
      const indexPath = join(root, ".test-output/managed-task-index.json"), indexBefore = readFileSync(indexPath, "utf8");
      const entriesBefore = readdirSync(join(root, ".test-output")).sort();
      expect(() => pruneManagedRecords(root, plan.revision, plan.candidates)).toThrow(`retention_${boundary}_too_large`);
      expect(readFileSync(indexPath, "utf8")).toBe(indexBefore);
      expect(readdirSync(join(root, ".test-output")).sort()).toEqual(entriesBefore);
      expect(existsSync(claim.root)).toBe(true);
      expect(existsSync(join(root, ".test-output/managed-retention.json"))).toBe(false);
      expect(existsSync(join(root, ".test-output/managed-retention-receipts"))).toBe(false);
      // A readable pending journal can still overflow on its next phase or terminal receipt.
      moveAuxiliary(`${auxiliary}x`);
      plan = managedRetentionPlan(root);
      const seed = serializedSizes(plan).journalValue;
      seed.steps[0].phase = "pending";
      const pendingPath = join(root, ".test-output/managed-retention.json");
      atomicManagedJson(pendingPath, seed);
      const pendingHash = sha256File(pendingPath), recovery = managedRetentionPlan(root);
      expect(readOwnedJson(pendingPath).steps[0].phase).toBe("pending");
      expect(() => pruneManagedRecords(root, recovery.revision, recovery.candidates)).toThrow(`retention_${boundary}_too_large`);
      expect(sha256File(pendingPath)).toBe(pendingHash);
      expect(readFileSync(indexPath, "utf8")).toBe(indexBefore);
      expect(existsSync(claim.root)).toBe(true);
      expect(existsSync(seed.steps[0].quarantine)).toBe(false);
      expect(existsSync(join(root, ".test-output/managed-retention-receipts"))).toBe(false);
      unlinkSync(pendingPath);
      moveAuxiliary(boundaryPath);
      plan = managedRetentionPlan(root);
      if (boundary === "journal") {
        expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterIndexCommit() { throw new Error("bounded interruption"); } })).toThrow("bounded interruption");
        const journalPath = join(root, ".test-output/managed-retention.json");
        expect(lstatSync(journalPath).size).toBe(limit);
        expect(readOwnedJson(journalPath).steps[0].phase).toBe("quarantined");
        const recovery = managedRetentionPlan(root);
        const result = pruneManagedRecords(root, recovery.revision, recovery.candidates);
        expect(readOwnedJson(result.receiptPath as string).removed).toHaveLength(1);
        expect(existsSync(journalPath)).toBe(false);
      } else {
        const result = pruneManagedRecords(root, plan.revision, plan.candidates);
        expect(lstatSync(result.receiptPath as string).size).toBe(limit);
        expect(readOwnedJson(result.receiptPath as string).removed).toHaveLength(1);
      }
      expect(existsSync(claim.root)).toBe(false);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test("legacy journals without immutable owner bindings refuse without changing quarantined evidence", () => {
    const root = realpathSync(tempRoot()), task = createRetentionTask(root, "legacy");
    finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterQuarantine(path) {
      quarantine = path;
      throw new Error("legacy interruption");
    } })).toThrow("legacy interruption");
    const path = join(root, ".test-output/managed-retention.json"), journal = readOwnedJson(path);
    journal.schemaVersion = 1;
    for (const candidate of journal.plan.candidates) {
      delete candidate.auxiliaries;
      delete candidate.ownerIdentity;
      for (const record of candidate.coveredRecords) delete record.ownerIdentity;
    }
    const { revision: _revision, ...body } = journal.plan;
    journal.plan.revision = createHash("sha256").update(canonicalJson(body)).digest("hex");
    atomicManagedJson(path, journal);
    const journalBefore = sha256File(path), indexBefore = sha256File(join(root, ".test-output/managed-task-index.json"));
    expect(() => managedRetentionPlan(root)).toThrow("retention_legacy_journal_requires_compatible_reader_or_reviewed_recovery");
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates)).toThrow("retention_legacy_journal_requires_compatible_reader_or_reviewed_recovery");
    expect(sha256File(path)).toBe(journalBefore);
    expect(sha256File(join(root, ".test-output/managed-task-index.json"))).toBe(indexBefore);
    expect(readOwnedJson(join(quarantine, "task.json")).identity.id).toBe("legacy");
  });

  test("nested metadata operations require the exact acquired live lease", () => {
    const root = realpathSync(tempRoot()), leasePath = join(root, "target-agent/.locks/metadata.lock/lease.json");
    withManagedMetadata(root, () => {
      expect(withManagedMetadata(root, () => "nested-operation")).toBe("nested-operation");
      const acquired = readOwnedJson(leasePath);
      atomicManagedJson(leasePath, { ...acquired, generation: "replaced-generation" });
      let called = false;
      try {
        expect(() => withManagedMetadata(root, () => { called = true; })).toThrow("metadata_lease_changed");
        expect(called).toBe(false);
      } finally { atomicManagedJson(leasePath, acquired); }
      expect(withManagedMetadata(root, () => readOwnedJson(leasePath).generation)).toBe(acquired.generation);
    });
    expect(existsSync(dirname(leasePath))).toBe(false);
  });

  function createRetentionArtifact(root: string, id: string, input?: ArtifactReference) {
    const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output", `producer-${id}`), kind: "directory", probeId: "retention-fixture" }), `producer-${id}`);
    const task = beginManagedTask(claim, "build-job", []);
    updateManagedTask(task, { state: "running" });
    const directory = join(root, "target-agent/artifacts", id);
    mkdirSync(directory, { recursive: true });
    const manifestPath = `target-agent/artifacts/${id}/manifest.json`;
    atomicManagedJson(join(root, manifestPath), { schemaVersion: 3, artifactId: id,
      publication: { owner: "scripts/agentic/agent-cargo.sh", pool: "agent-debug", leaseGeneration: `lease-${id}`,
        buildTask: task.identity, immutable: true, exportedWhileLeaseHeld: true },
      ...(input ? { derivation: { input, transformation: "signed-and-stapled-bundle" } } : {}) });
    const reference = { manifestPath, manifestSha256: sha256File(join(root, manifestPath)) };
    updateManagedTask(task, { result: { status: "succeeded", artifacts: [reference] } });
    finalizeManagedTask(task, emptyOwnedCleanup());
    return { task, directory, reference };
  }

  function createPublicationIntent(root: string, id: string) {
    const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output", `producer-${id}`), kind: "directory", probeId: "retention-fixture" }), `producer-${id}`);
    const task = beginManagedTask(claim, "build-job", []);
    const pendingPath = `target-agent/artifacts/.pending-${id}`, destinationPath = `target-agent/artifacts/${id}`;
    mkdirSync(join(root, pendingPath), { recursive: true });
    const stat = lstatSync(join(root, pendingPath));
    const intent = { id, generation: `generation-${id}`, pendingPath, destinationPath, directoryDevice: stat.dev, directoryInode: stat.ino };
    registerManagedPublicationIntent(task, intent);
    writeFileSync(join(root, pendingPath, "partial-binary"), "owned partial bytes");
    return { task, intent };
  }

  test("an explicit parent selection cannot absorb an unselected descendant or audit sibling", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "parent"), child = createRetentionTask(root, "child", "parent/child");
    const audit = createRetentionTask(root, "audit");
    for (const task of [parent, child, audit]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    const parentCandidate = plan.candidates.find((candidate: RetentionCandidate) => candidate.id === "parent");
    expect(() => pruneManagedRecords(root, plan.revision, [parentCandidate])).toThrow("retention_unselected_descendant");
    expect(existsSync(join(root, ".test-output/managed-retention.json"))).toBe(false);
    const selection = plan.candidates.filter((candidate: RetentionCandidate) => candidate.id !== "audit");
    const result = pruneManagedRecords(root, plan.revision, selection);
    expect(result.removed).toHaveLength(2);
    expect(existsSync(audit.recordPath)).toBe(true);
    expect(existsSync(parent.recordPath)).toBe(false);
    expect(readOwnedJson(result.receiptPath as string).selection).toHaveLength(2);
  });

  test("missing, forged, duplicate, and malformed selections cannot authorize mutation", () => {
    const root = realpathSync(tempRoot());
    const task = createRetentionTask(root, "selected");
    finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root), candidate = plan.candidates[0];
    expect(() => pruneManagedRecords(root, plan.revision, undefined as never)).toThrow("retention_selection_required");
    expect(() => pruneManagedRecords(root, plan.revision, [candidate, candidate])).toThrow("retention_selection_duplicate");
    expect(() => pruneManagedRecords(root, plan.revision, [{ ...candidate, directoryInode: -1 }])).toThrow("retention_selection_invalid");
    expect(() => pruneManagedRecords(root, plan.revision, [{ ...candidate, recordSha256: "0".repeat(64) }])).toThrow("retention_candidate_not_authorized");
    expect(pruneManagedRecords(root, plan.revision, []).removed).toEqual([]);
    expect(existsSync(task.recordPath)).toBe(true);
    expect(existsSync(join(root, ".test-output/managed-retention.json"))).toBe(false);
  });

  test("revision-bound keep references protect derivation inputs and publishers without selecting audit siblings", () => {
    const root = realpathSync(tempRoot());
    const input = createRetentionArtifact(root, "input"), derived = createRetentionArtifact(root, "derived", input.reference);
    const obsolete = createRetentionArtifact(root, "obsolete"), audit = createRetentionTask(root, "audit");
    finalizeManagedTask(audit, emptyOwnedCleanup());
    const empty = managedKeepSet(root);
    const kept = updateManagedKeepSet(root, empty.revision, [derived.reference]);
    expect(managedKeepSet(root)).toEqual(kept);
    expect(() => updateManagedKeepSet(root, empty.revision, [])).toThrow("managed_keep_set_changed");
    expect(() => updateManagedKeepSet(root, kept.revision, [{ ...derived.reference, manifestSha256: "0".repeat(64) }])).toThrow("artifact_reference_changed");
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.some((candidate: RetentionCandidate) => ["input", "derived", input.task.identity.id, derived.task.identity.id].includes(candidate.id))).toBe(false);
    const selected = plan.candidates.filter((candidate: RetentionCandidate) => ["obsolete", obsolete.task.identity.id].includes(candidate.id));
    pruneManagedRecords(root, plan.revision, selected);
    for (const path of [input.directory, derived.directory, input.task.recordPath, derived.task.recordPath, audit.recordPath]) expect(existsSync(path)).toBe(true);
    expect(existsSync(obsolete.directory)).toBe(false);
  });

  test("unselected exports keep their selected producer from being retired", () => {
    const root = realpathSync(tempRoot()), artifact = createRetentionArtifact(root, "retained");
    const plan = managedRetentionPlan(root);
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates.filter((candidate: RetentionCandidate) => candidate.kind !== "artifact"))).toThrow("retention_unselected_publication");
    expect(existsSync(artifact.task.recordPath)).toBe(true);
  });

  test("reused exact references register under the task revision and protect the producer", () => {
    const root = realpathSync(tempRoot()), artifact = createRetentionArtifact(root, "reused");
    const task = createRetentionTask(root, "consumer"), revision = task.identity.revision;
    expect(registerManagedArtifactReference(task, artifact.reference).artifactReferences).toEqual([artifact.reference]);
    expect(task.identity.revision).toBe(revision + 1);
    expect(registerManagedArtifactReference(task, artifact.reference).artifactReferences).toEqual([artifact.reference]);
    expect(() => registerManagedArtifactReference(task, { ...artifact.reference, manifestSha256: "f".repeat(64) })).toThrow("artifact_reference_changed");
    expect(managedRetentionPlan(root).candidates).toEqual([]);
    finalizeManagedTask(task, emptyOwnedCleanup());
    expect(() => registerManagedArtifactReference(task, artifact.reference)).toThrow("managed_task_revision_changed_or_terminal");
    expect(managedRetentionPlan(root).candidates).toHaveLength(3);
  });

  test("recovery finishes started work, withdraws pending A, and never appends newly derived B", () => {
    const root = realpathSync(tempRoot()), started = createRetentionTask(root, "a");
    finalizeManagedTask(started, emptyOwnedCleanup());
    const input = createRetentionArtifact(root, "A"), plan = managedRetentionPlan(root);
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterQuarantine(path) {
      quarantine = path;
      throw new Error("checkpoint interruption");
    } })).toThrow("checkpoint interruption");
    expect(quarantine.startsWith(dirname(started.recordPath))).toBe(true);
    const beforePublication = managedRetentionPlan(root);
    const derived = createRetentionArtifact(root, "B", input.reference);
    const recovery = managedRetentionPlan(root);
    expect(recovery.revision).not.toBe(beforePublication.revision);
    expect(() => pruneManagedRecords(root, beforePublication.revision, beforePublication.candidates)).toThrow("retention_plan_changed");
    expect(() => pruneManagedRecords(root, recovery.revision, [...recovery.candidates, { ...recovery.candidates[0], id: "B" }])).toThrow("retention_selection_changed");
    const result = pruneManagedRecords(root, recovery.revision, recovery.candidates);
    expect(result.removed).toEqual([{ kind: started.identity.kind, id: started.identity.id, generation: started.identity.generation }]);
    expect(result.withdrawn).toHaveLength(2);
    expect(result.replanRequired).toBe(true);
    const receipt = readOwnedJson(result.receiptPath as string);
    expect(receipt.withdrawn).toEqual(result.withdrawn);
    expect(receipt.candidates.some((candidate: RetentionCandidate) => candidate.id === "B")).toBe(false);
    expect(existsSync(recovery.recovery.path)).toBe(false);
    expect(existsSync(quarantine)).toBe(false);
    expect(existsSync(input.directory)).toBe(true);
    expect(existsSync(input.task.recordPath)).toBe(true);
    const fresh = managedRetentionPlan(root);
    expect(fresh.candidates.some((candidate: RetentionCandidate) => candidate.id === "A")).toBe(false);
    pruneManagedRecords(root, fresh.revision, fresh.candidates.filter((candidate: RetentionCandidate) => ["B", derived.task.identity.id].includes(candidate.id)));
    const released = managedRetentionPlan(root);
    expect(released.candidates.some((candidate: RetentionCandidate) => candidate.id === "A")).toBe(true);
    pruneManagedRecords(root, released.revision, released.candidates);
    expect(existsSync(input.directory)).toBe(false);
  });

  test("new references to a started artifact refuse recovery without deleting it", () => {
    const root = realpathSync(tempRoot()), artifact = createRetentionArtifact(root, "started-artifact");
    const consumer = createRetentionTask(root, "consumer");
    const plan = managedRetentionPlan(root), selected = plan.candidates.filter((candidate: RetentionCandidate) => candidate.kind === "artifact");
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, selected, { afterQuarantine(path) { quarantine = path; throw new Error("interrupt"); } })).toThrow("interrupt");
    const record = readOwnedJson(consumer.recordPath);
    record.artifactReferences = [artifact.reference];
    atomicManagedJson(consumer.recordPath, record);
    const recovery = managedRetentionPlan(root);
    expect(() => pruneManagedRecords(root, recovery.revision, recovery.candidates)).toThrow("retention_new_reference");
    expect(existsSync(join(quarantine, "manifest.json"))).toBe(true);
    expect(existsSync(recovery.recovery.path)).toBe(true);
    expect(existsSync(artifact.task.recordPath)).toBe(true);
  });

  test("sealed trees retire without changing the modes of hardlinked files", () => {
    const root = realpathSync(tempRoot()), task = createRetentionTask(root, "sealed");
    const nested = join(dirname(task.recordPath), "nested"), outside = join(root, "hardlink-sentinel");
    mkdirSync(nested);
    writeFileSync(outside, "shared file bytes", { mode: 0o444 });
    linkSync(outside, join(nested, "shared"));
    finalizeManagedTask(task, emptyOwnedCleanup());
    chmodSync(nested, 0o500);
    chmodSync(dirname(task.recordPath), 0o500);
    const before = lstatSync(outside).mode, plan = managedRetentionPlan(root);
    pruneManagedRecords(root, plan.revision, plan.candidates);
    expect(existsSync(task.recordPath)).toBe(false);
    expect(lstatSync(outside).mode).toBe(before);
    expect(readFileSync(outside, "utf8")).toBe("shared file bytes");
  });

  test("open handles refuse quarantine until the owner closes them and a fresh plan is approved", () => {
    const root = realpathSync(tempRoot()), task = createRetentionTask(root, "open-handle");
    finalizeManagedTask(task, emptyOwnedCleanup());
    const fd = openSync(task.recordPath, "r"), plan = managedRetentionPlan(root);
    try { expect(() => pruneManagedRecords(root, plan.revision, plan.candidates)).toThrow("retention_open_handles_present_or_unknown"); }
    finally { closeSync(fd); }
    const recovery = managedRetentionPlan(root);
    expect(pruneManagedRecords(root, recovery.revision, recovery.candidates).removed).toEqual([]);
    expect(existsSync(task.recordPath)).toBe(true);
    const fresh = managedRetentionPlan(root);
    expect(pruneManagedRecords(root, fresh.revision, fresh.candidates).removed).toHaveLength(1);
  });

  test.each(["pending", "published"] as const)("failed %s publication intents retire only with exact ownership and full closure", location => {
    const root = realpathSync(tempRoot()), { task, intent } = createPublicationIntent(root, `failed-${location}`);
    if (location === "published") {
      renameSync(join(root, intent.pendingPath), join(root, intent.destinationPath));
      updateManagedPublicationIntent(task, intent.id, "published");
    }
    updateManagedPublicationIntent(task, intent.id, "failed");
    expect(managedRetentionPlan(root).candidates).toEqual([]);
    updateManagedTask(task, { result: { status: "failed" } });
    finalizeManagedTask(task, emptyOwnedCleanup());
    const historical = join(root, "target-agent/artifacts/.pending-historical");
    mkdirSync(historical);
    writeFileSync(join(historical, "unknown"), "historical bytes");
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.map((candidate: RetentionCandidate) => candidate.id)).toEqual([intent.id, task.identity.id]);
    expect(pruneManagedRecords(root, plan.revision, plan.candidates).removed).toHaveLength(2);
    expect(existsSync(join(root, location === "pending" ? intent.pendingPath : intent.destinationPath))).toBe(false);
    expect(readFileSync(join(historical, "unknown"), "utf8")).toBe("historical bytes");
  });

  test.each(["incomplete-cleanup", "reference", "replaced-directory"] as const)("failed intent remains protected with %s", blocker => {
    const root = realpathSync(tempRoot()), { task, intent } = createPublicationIntent(root, "protected-intent");
    updateManagedPublicationIntent(task, intent.id, "failed");
    finalizeManagedTask(task, blocker === "incomplete-cleanup" ? { ...emptyOwnedCleanup(), streamsDrained: false } : emptyOwnedCleanup());
    if (blocker === "reference") {
      const consumer = createRetentionTask(root, "consumer"), record = readOwnedJson(consumer.recordPath);
      record.artifactReferences = [{ manifestPath: `${intent.destinationPath}/manifest.json`, manifestSha256: "a".repeat(64) }];
      atomicManagedJson(consumer.recordPath, record);
    } else if (blocker === "replaced-directory") {
      renameSync(join(root, intent.pendingPath), join(root, "original-intent"));
      mkdirSync(join(root, intent.pendingPath));
    }
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.some((candidate: RetentionCandidate) => [intent.id, task.identity.id].includes(candidate.id))).toBe(false);
    expect(existsSync(join(root, intent.pendingPath))).toBe(true);
    expect(existsSync(task.recordPath)).toBe(true);
  });

  test.each(["prune-cargo-targets.sh", "disk-space-cargo-emergency-clean.sh"])("%s refuses noninteractive destruction before touching the repository", script => {
    const root = tempRoot(), sentinel = join(root, "sentinel");
    writeFileSync(sentinel, "untouched");
    const result = spawnSync("bash", [join(repoRoot, "scripts/agentic", script), "--apply"], {
      encoding: "utf8", env: { ...process.env, SCRIPT_KIT_REPO_ROOT: join(root, "not-a-repository"), SCRIPT_KIT_NONINTERACTIVE: "1" },
    });
    expect(result.status).toBe(78);
    expect(result.stderr).toContain("refused in noninteractive mode");
    expect(readFileSync(sentinel, "utf8")).toBe("untouched");
    expect(readdirSync(root)).toEqual(["sentinel"]);
  });

  test("only exact completed removals establish retired immutable references", () => {
    const root = realpathSync(tempRoot()), artifact = createRetentionArtifact(root, "retired");
    expect(isRetiredManagedArtifact(root, artifact.reference)).toBe(false);
    const plan = managedRetentionPlan(root);
    const result = pruneManagedRecords(root, plan.revision, plan.candidates.filter((candidate: RetentionCandidate) => candidate.kind === "artifact"));
    expect(isRetiredManagedArtifact(root, artifact.reference)).toBe(true);
    expect(isRetiredManagedArtifact(root, { ...artifact.reference, manifestSha256: "f".repeat(64) })).toBe(false);
    const receipt = readOwnedJson(result.receiptPath as string);
    receipt.candidates[0].coveredRecords[0].manifestContents += " ";
    atomicManagedJson(result.receiptPath as string, receipt);
    expect(() => isRetiredManagedArtifact(root, artifact.reference)).toThrow("retention_history_manifest_changed");
  });

  test("withdrawn originals and unreadable history never establish retirement", () => {
    const root = realpathSync(tempRoot()), task = createRetentionTask(root, "a");
    finalizeManagedTask(task, emptyOwnedCleanup());
    const artifact = createRetentionArtifact(root, "withdrawn"), plan = managedRetentionPlan(root);
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterQuarantine() { throw new Error("interrupt"); } })).toThrow("interrupt");
    const recovery = managedRetentionPlan(root), result = pruneManagedRecords(root, recovery.revision, recovery.candidates);
    expect(isRetiredManagedArtifact(root, artifact.reference)).toBe(false);
    writeFileSync(result.receiptPath as string, "not-json");
    expect(() => isRetiredManagedArtifact(root, artifact.reference)).toThrow();
  });

  test("a new unregistered managed descendant in an indexed quarantine stays protected", () => {
    const root = realpathSync(tempRoot()), task = createRetentionTask(root, "parent");
    finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { beforeRemove(path) { quarantine = path; throw new Error("interrupt"); } })).toThrow("interrupt");
    const child = join(quarantine, "new-child");
    mkdirSync(child);
    atomicManagedJson(join(child, OUTPUT_OWNER_FILE), { owner: "new-independent-owner" });
    const recovery = managedRetentionPlan(root);
    expect(() => pruneManagedRecords(root, recovery.revision, recovery.candidates)).toThrow("retention_unselected_descendant");
    expect(existsSync(join(child, OUTPUT_OWNER_FILE))).toBe(true);
    expect(existsSync(recovery.recovery.path)).toBe(true);
  });

  test.each(["record", "owned-descendant", "index"] as const)("a later scratch root stays protected after a mid-batch %s change", mutation => {
    const root = realpathSync(tempRoot());
    const first = createRetentionTask(root, "batch-a"), later = createRetentionTask(root, "batch-b");
    for (const task of [first, later]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    let changed = false;
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { beforeRemove() {
      if (changed) return;
      changed = true;
      if (mutation === "record") {
        const record = readOwnedJson(later.recordPath);
        record.result = { changed: true };
        atomicManagedJson(later.recordPath, record);
      } else if (mutation === "owned-descendant") {
        const child = join(dirname(later.recordPath), "new-owner");
        mkdirSync(child);
        atomicManagedJson(join(child, OUTPUT_OWNER_FILE), { owner: "independent-owner" });
      } else {
        const indexPath = join(root, ".test-output/managed-task-index.json");
        const index = readOwnedJson(indexPath);
        index["new-independent-task"] = join(dirname(later.recordPath), "new-task/task.json");
        atomicManagedJson(indexPath, index);
      }
    } })).toThrow(/retention_/);
    expect(changed).toBe(true);
    expect(existsSync(dirname(first.recordPath))).toBe(false);
    const journal = readOwnedJson(join(root, ".test-output/managed-retention.json"));
    const position = journal.plan.candidates.findIndex((candidate: RetentionCandidate) => candidate.id === later.identity.id);
    expect(journal.steps[position].phase).not.toBe("removed");
    const retainedRoot = existsSync(dirname(later.recordPath)) ? dirname(later.recordPath) : journal.steps[position].quarantine;
    expect(existsSync(join(retainedRoot, "task.json"))).toBe(true);
    expect(existsSync(join(retainedRoot, OUTPUT_OWNER_FILE))).toBe(true);
  });

  test("prunes closed parent and nested records once and removes every covered index entry", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    const grandchild = createRetentionTask(root, "c-grandchild", "a-parent/b-child/c-grandchild");
    const sibling = createRetentionTask(root, "a-parent-extra");
    for (const task of [parent, child, grandchild, sibling]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.map((candidate: { id: string }) => candidate.id).sort()).toEqual(["a-parent", "a-parent-extra", "b-child", "c-grandchild"]);
    expect(plan.candidates.find((candidate: { id: string }) => candidate.id === "a-parent").coveredRecords.map((record: { id: string }) => record.id)).toEqual(["a-parent", "b-child", "c-grandchild"]);
    expect(pruneManagedRecords(root, plan.revision, plan.candidates).removed).toEqual(expect.arrayContaining(
      [parent, child, grandchild, sibling].map(task => ({ kind: task.identity.kind, id: task.identity.id, generation: task.identity.generation })),
    ));
    for (const task of [parent, child, grandchild, sibling]) expect(existsSync(dirname(task.recordPath))).toBe(false);
    expect(readOwnedJson(join(root, ".test-output/managed-task-index.json"))).toEqual({});
    expect(listManagedTasks(root)).toEqual([]);
    expect(managedRetentionPlan(root).protectedRecords).toEqual([]);
  });

  test.each(["running", "protected", "malformed", "missing", "unregistered"] as const)("protects all ancestors of a %s child", state => {
    const root = realpathSync(tempRoot());
    const ancestor = createRetentionTask(root, "ancestor");
    const parent = createRetentionTask(root, "a-parent", "ancestor/a-parent");
    const child = createRetentionTask(root, "b-child", "ancestor/a-parent/b-child");
    const sibling = createRetentionTask(root, "a-parent-extra", "ancestor/a-parent-extra");
    for (const task of [ancestor, parent, sibling]) finalizeManagedTask(task, emptyOwnedCleanup());
    if (state === "running") updateManagedTask(child, { state: "running" });
    else if (state === "protected") finalizeManagedTask(child, { ...emptyOwnedCleanup(), closed: false, processExited: false });
    else if (state === "malformed") writeFileSync(child.recordPath, "not-json\n");
    else if (state === "missing") unlinkSync(child.recordPath);
    else {
      const indexPath = join(root, ".test-output/managed-task-index.json");
      const index = readOwnedJson(indexPath);
      delete index[child.identity.id];
      atomicManagedJson(indexPath, index);
    }
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.map((candidate: { id: string }) => candidate.id)).toEqual(["a-parent-extra"]);
    pruneManagedRecords(root, plan.revision, plan.candidates);
    for (const task of [ancestor, parent, child]) expect(existsSync(dirname(task.recordPath))).toBe(true);
    expect(existsSync(dirname(sibling.recordPath))).toBe(false);
    expect(readOwnedJson(join(root, ".test-output/managed-task-index.json"))[parent.identity.id]).toBe(parent.recordPath);
  });

  test("a closed child can be removed without removing its active parent", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    updateManagedTask(parent, { state: "running" });
    finalizeManagedTask(child, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    expect(plan.candidates.map((candidate: { id: string }) => candidate.id)).toEqual(["b-child"]);
    pruneManagedRecords(root, plan.revision, plan.candidates);
    expect(existsSync(parent.recordPath)).toBe(true);
    expect(existsSync(dirname(child.recordPath))).toBe(false);
    expect(listManagedTasks(root).map(entry => entry.record?.identity.id)).toEqual(["a-parent"]);
  });

  test("interruption immediately after rename keeps exact ownership and index for revision-bound resume", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    for (const task of [parent, child]) finalizeManagedTask(task, emptyOwnedCleanup());
    const indexPath = join(root, ".test-output/managed-task-index.json");
    const indexBefore = readFileSync(indexPath, "utf8");
    const parentBefore = readFileSync(parent.recordPath, "utf8");
    const childBefore = readFileSync(child.recordPath, "utf8");
    const ownerBefore = readFileSync(join(dirname(child.recordPath), OUTPUT_OWNER_FILE), "utf8");
    const plan = managedRetentionPlan(root);
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterQuarantine(path) {
      quarantine = path;
      throw new Error("interrupted after quarantine");
    } })).toThrow("interrupted after quarantine");
    expect(readFileSync(indexPath, "utf8")).toBe(indexBefore);
    expect(readFileSync(join(quarantine, "task.json"), "utf8")).toBe(parentBefore);
    expect(readFileSync(join(quarantine, "b-child/task.json"), "utf8")).toBe(childBefore);
    expect(readFileSync(join(quarantine, "b-child", OUTPUT_OWNER_FILE), "utf8")).toBe(ownerBefore);
    const recovery = managedRetentionPlan(root);
    expect(recovery.recovery.expectedRevision).toBe(recovery.revision);
    expect(recovery.revision).not.toBe(plan.revision);
    expect(recovery.recovery.steps[0].phase).toBe("pending");
    expect(recovery.protectedRecords).toEqual([]);
    expect(() => pruneManagedRecords(root, "wrong-revision", plan.candidates)).toThrow("retention_plan_changed");
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates)).toThrow("retention_plan_changed");
    expect(existsSync(quarantine)).toBe(true);
    expect(pruneManagedRecords(root, recovery.revision, recovery.candidates).removed).toHaveLength(2);
    expect(existsSync(quarantine)).toBe(false);
    expect(existsSync(recovery.recovery.path)).toBe(false);
    expect(listManagedTasks(root)).toEqual([]);
  });

  test("recovers an atomic index commit interrupted before its journal phase advances", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    for (const task of [parent, child]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterIndexCommit(path) {
      quarantine = path;
      throw new Error("interrupted after index commit");
    } })).toThrow("interrupted after index commit");
    expect(readOwnedJson(join(root, ".test-output/managed-task-index.json"))).toEqual({});
    expect(managedRetentionPlan(root).recovery.steps[0].phase).toBe("quarantined");
    expect(existsSync(join(quarantine, "b-child/task.json"))).toBe(true);
    const recovery = managedRetentionPlan(root);
    expect(pruneManagedRecords(root, recovery.revision, recovery.candidates).removed).toHaveLength(2);
    expect(existsSync(quarantine)).toBe(false);
    expect(listManagedTasks(root)).toEqual([]);
  });

  test("resumes partial deletion from indexed ownership without touching a replacement original path", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    for (const task of [parent, child]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { beforeRemove(path) {
      quarantine = path;
      rmSync(join(path, "b-child"), { recursive: true });
      unlinkSync(join(path, OUTPUT_OWNER_FILE));
      throw new Error("interrupted partial deletion");
    } })).toThrow("interrupted partial deletion");
    expect(listManagedTasks(root)).toEqual([]);
    const recovery = managedRetentionPlan(root);
    expect(recovery.recovery.steps[0].phase).toBe("indexed");
    const journal = readOwnedJson(recovery.recovery.path);
    expect(journal.plan.candidates[0].coveredRecords.map((record: { owner: { runId: string } }) => record.owner.runId)).toEqual(["a-parent", "b-child"]);
    mkdirSync(dirname(parent.recordPath));
    writeFileSync(join(dirname(parent.recordPath), "sentinel"), "replacement survives\n");
    const current = managedRetentionPlan(root);
    expect(pruneManagedRecords(root, current.revision, current.candidates).removed).toHaveLength(2);
    expect(existsSync(quarantine)).toBe(false);
    expect(readFileSync(join(dirname(parent.recordPath), "sentinel"), "utf8")).toBe("replacement survives\n");
    expect(readOwnedJson(join(root, ".test-output/managed-task-index.json"))).toEqual({});
  });

  test("recovers completed deletion before its journal phase advances", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    for (const task of [parent, child]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { beforeRemove(path) {
      rmSync(path, { recursive: true });
      throw new Error("interrupted after deletion");
    } })).toThrow("interrupted after deletion");
    const recovery = managedRetentionPlan(root);
    expect(recovery.recovery.steps[0].phase).toBe("indexed");
    expect(existsSync(recovery.recovery.steps[0].quarantine)).toBe(false);
    expect(pruneManagedRecords(root, recovery.revision, recovery.candidates).removed).toHaveLength(2);
    expect(listManagedTasks(root)).toEqual([]);
    expect(managedRetentionPlan(root).protectedRecords).toEqual([]);
  });

  test("a changed quarantined descendant refuses recovery without losing its journal or index", () => {
    const root = realpathSync(tempRoot());
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    for (const task of [parent, child]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    const childBefore = readFileSync(child.recordPath, "utf8");
    let quarantine = "";
    expect(() => pruneManagedRecords(root, plan.revision, plan.candidates, { afterQuarantine(path) {
      quarantine = path;
      throw new Error("interrupted");
    } })).toThrow("interrupted");
    writeFileSync(join(quarantine, "b-child/task.json"), "{}\n");
    const recovery = managedRetentionPlan(root);
    expect(() => pruneManagedRecords(root, recovery.revision, recovery.candidates)).toThrow("retention_record_changed");
    expect(Object.keys(readOwnedJson(join(root, ".test-output/managed-task-index.json"))).sort()).toEqual(["a-parent", "b-child"]);
    expect(existsSync(managedRetentionPlan(root).recovery.path)).toBe(true);
    writeFileSync(join(quarantine, "b-child/task.json"), childBefore);
    const current = managedRetentionPlan(root);
    expect(pruneManagedRecords(root, current.revision, current.candidates).removed).toHaveLength(2);
  });

  test("removed nested tasks never become unknown global pins and artifact dependencies prune coherently", () => {
    const root = realpathSync(tempRoot());
    const artifact = createArtifactFixture(root);
    const consumer = createRetentionTask(root, "artifact-consumer", "artifact-consumer", [artifact.reference]);
    const parent = createRetentionTask(root, "a-parent");
    const child = createRetentionTask(root, "b-child", "a-parent/b-child");
    for (const task of [parent, child]) finalizeManagedTask(task, emptyOwnedCleanup());
    const plan = managedRetentionPlan(root);
    expect(pruneManagedRecords(root, plan.revision, plan.candidates).removed).toHaveLength(2);
    expect(listManagedTasks(root).every(entry => entry.record !== undefined)).toBe(true);
    finalizeManagedTask(consumer, emptyOwnedCleanup());
    const released = managedRetentionPlan(root);
    const artifactIndex = released.candidates.findIndex((candidate: { kind: string }) => candidate.kind === "artifact");
    const publisherIndex = released.candidates.findIndex((candidate: { id: string }) => candidate.id === artifact.task.identity.id);
    expect(artifactIndex).toBeGreaterThanOrEqual(0);
    expect(publisherIndex).toBeGreaterThan(artifactIndex);
    expect(pruneManagedRecords(root, released.revision, released.candidates).removed).toHaveLength(3);
    expect(existsSync(artifact.publicationDirectory)).toBe(false);
    expect(listManagedTasks(root)).toEqual([]);
    expect(managedRetentionPlan(root).protectedRecords).toEqual([]);
  });
});

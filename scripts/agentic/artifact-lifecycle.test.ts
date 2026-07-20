import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  approveStagingAnchor,
  buildArtifactLifecycle,
  claimOutput,
  commitFinalReceipt,
  createOwnedStagingDirectory,
  materializeAtomic,
  removeOwnedTree,
  retainLiveSessionArtifacts,
  sha256File,
  validateArtifact,
  validateOutputTarget,
  waitForProcessesDead,
  writeJsonArtifactAtomic,
  type ArtifactReceipt,
  type ArtifactSpec,
} from "./artifact-lifecycle";

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
    expect(() => removeOwnedTree(wrongClaim)).toThrow("marker/token mismatch");
    expect(readFileSync(sentinel, "utf8")).toBe("preserve-unless-owned\n");
    removeOwnedTree(claim);
    expect(existsSync(root)).toBe(false);
  });

  test("all five migrated CLIs reject unsafe output before launch or session start", () => {
    const helper = join(repoRoot, "scripts/agentic/macos-input.ts");
    const binarySha = createHash("sha256").update(readFileSync(process.execPath)).digest("hex");
    const helperSha = createHash("sha256").update(readFileSync(helper)).digest("hex");
    const cases = [
      ["scripts/agentic/main-menu-focus-flicker.ts", "--out", "/"],
      [
        "scripts/agentic/root-search-frame-stability.ts",
        "--binary",
        process.execPath,
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
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode, args[0]).not.toBe(0);
      expect(result.stderr.toString(), args[0]).toContain("unsafe");
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
         appendFileSync(${JSON.stringify(source)}, JSON.stringify({requestId:"a",type:"stateResult"}) + "\\n");
         writeFileSync(${JSON.stringify(ready)}, "ready");
         while (!existsSync(${JSON.stringify(proceed)})) await Bun.sleep(5);
         appendFileSync(${JSON.stringify(source)}, JSON.stringify({requestId:"b",type:"waitForResult"}) + "\\n");`,
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
      { requestId: "expected", type: "stateResult" },
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
    writeFileSync(path, `${JSON.stringify({ requestId: "other", type: "stateResult" })}\n`);
    expect(validateArtifact(path, requiredProtocol, root).validation.correlation?.missing).toEqual([
      "expected",
    ]);

    writeFileSync(
      path,
      `${JSON.stringify({ requestId: "expected", type: "stateResult" })}\n${JSON.stringify({ requestId: "expected", type: "stateResult" })}\n`,
    );
    expect(validateArtifact(path, requiredProtocol, root).validation.correlation?.duplicates).toEqual([
      "expected",
    ]);

    writeFileSync(path, `${JSON.stringify({ requestId: "expected", type: "wrong" })}\n`);
    expect(validateArtifact(path, requiredProtocol, root).validation.correlation?.unexpectedType).toEqual([
      "expected:wrong!=stateResult",
    ]);
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
    writeFileSync(path, `${JSON.stringify({ requestId: "expected", type: "stateResult" })}\n`);
    const artifact = validateArtifact(path, spec, claim.artifactsRoot);
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
        (receipt as ArtifactReceipt & { finalizedAfterWriters: boolean }).finalizedAfterWriters = false;
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

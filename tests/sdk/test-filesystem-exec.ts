// Name: SDK Test - Filesystem and Explicit Subprocess Helpers
// Description: Verifies supported readFile/writeFile/exec contracts without UI or shell injection.

import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

type Outcome = "running" | "pass" | "fail";

function report(test: string, status: Outcome, details: Record<string, unknown> = {}): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...details }));
}

async function check(name: string, operation: () => Promise<unknown>): Promise<void> {
  report(name, "running");
  const started = Date.now();
  try {
    const result = await operation();
    report(name, "pass", { result, duration_ms: Date.now() - started });
  } catch (error) {
    report(name, "fail", { error: String(error), duration_ms: Date.now() - started });
  }
}

const fixtureDirectory = await mkdtemp(join(tmpdir(), "script-kit-sdk-fs-"));

try {
  await check("filesystem-roundtrips-explicit-utf8-path", async () => {
    const path = join(fixtureDirectory, "roundtrip.txt");
    const content = "Hello, 世界 — 🩵";
    await writeFile(path, content, "utf8");
    const restored = await readFile(path, "utf8");
    if (restored !== content) {
      throw new Error(`UTF-8 content changed: ${JSON.stringify(restored)}`);
    }
    return { bytes: Buffer.byteLength(restored, "utf8") };
  });

  await check("filesystem-missing-path-rejects", async () => {
    let failure: any;
    try {
      await readFile(join(fixtureDirectory, "does-not-exist.txt"));
    } catch (error) {
      failure = error;
    }
    if (failure?.code !== "ENOENT") {
      throw new Error(`Expected ENOENT, got ${String(failure)}`);
    }
    return { code: failure.code };
  });

  await check("exec-explicit-argv-captures-stdout-and-stderr", async () => {
    const result = await exec(process.execPath, [
      "-e",
      "process.stdout.write('safe stdout');process.stderr.write('safe stderr')",
    ]);
    if (result.stdout !== "safe stdout" || result.stderr !== "safe stderr" || result.exitCode !== 0) {
      throw new Error(`Unexpected subprocess receipt: ${JSON.stringify(result)}`);
    }
    return result;
  });

  await check("exec-quoted-command-preserves-argument-boundaries", async () => {
    const result = await exec(
      `${JSON.stringify(process.execPath)} -e "process.stdout.write('two words')"`,
    );
    if (result.stdout !== "two words") {
      throw new Error(`Quoted arguments were not preserved: ${JSON.stringify(result)}`);
    }
    return { stdout: result.stdout };
  });

  await check("exec-nonzero-exit-preserves-diagnostics", async () => {
    let failure: any;
    try {
      await exec(process.execPath, ["-e", "process.stderr.write('expected failure');process.exit(7)"]);
    } catch (error) {
      failure = error;
    }
    if (
      failure?.name !== "SdkExecError" ||
      failure?.code !== "ERR_SDK_EXEC_NONZERO_EXIT" ||
      failure?.result?.exitCode !== 7 ||
      failure?.result?.stderr !== "expected failure"
    ) {
      throw new Error(`Expected typed nonzero-exit diagnostics, got ${JSON.stringify(failure)}`);
    }
    return { code: failure.code, exitCode: failure.result.exitCode };
  });

  await check("exec-missing-binary-preserves-typed-spawn-failure", async () => {
    let failure: any;
    try {
      await exec(join(fixtureDirectory, "binary-does-not-exist"), []);
    } catch (error) {
      failure = error;
    }
    if (failure?.name !== "SdkExecError" || failure?.code !== "ERR_SDK_EXEC_SPAWN_FAILED") {
      throw new Error(`Expected a typed subprocess spawn failure, got ${String(failure)}`);
    }
    return { code: failure.code };
  });

  await check("exec-invalid-arguments-reject-before-spawn", async () => {
    for (const [command, args] of [
      ["", []],
      [process.execPath, "not-an-array"],
      [process.execPath, ["bad\0argument"]],
    ] as const) {
      let failure: any;
      try {
        await exec(command, args as string[]);
      } catch (error) {
        failure = error;
      }
      if (failure?.code !== "ERR_SDK_EXEC_INVALID_COMMAND") {
        throw new Error(`Malformed explicit subprocess was not rejected: ${JSON.stringify(command)}`);
      }
    }
    return { malformedRequestsRejected: 3 };
  });

  await check("exec-shell-injection-is-rejected-before-spawn", async () => {
    const canary = join(fixtureDirectory, "must-not-exist");
    for (const command of [
      `echo safe; touch ${canary}`,
      `echo $(touch ${canary})`,
      `echo safe | touch ${canary}`,
      `echo safe > ${canary}`,
    ]) {
      let failure: any;
      try {
        await exec(command);
      } catch (error) {
        failure = error;
      }
      if (failure?.code !== "ERR_SDK_EXEC_INVALID_COMMAND") {
        throw new Error(`Unsafe command was not rejected: ${JSON.stringify(command)}`);
      }
    }
    try {
      await stat(canary);
      throw new Error("A rejected command unexpectedly created its canary file.");
    } catch (error: any) {
      if (error?.code !== "ENOENT") {
        throw error;
      }
    }
    return { blockedCommands: 4, spawnedUnsafeCommand: false };
  });
} finally {
  await rm(fixtureDirectory, { recursive: true, force: true });
}

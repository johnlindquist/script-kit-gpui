import { expect, test } from "bun:test";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { deflateSync } from "node:zlib";

test(
  "await-response emits a byte-complete large JSON envelope through command substitution",
  async () => {
    const root = mkdtempSync(join(tmpdir(), "sk-await-response-flush-"));

    try {
      const session = "large-output";
      const requestId = "stdout-flush-large";
      const responseType = "stateResult";
      const sessionDir = join(root, session);
      const responsesPath = join(sessionDir, "protocol-responses.ndjson");
      const helper = resolve(
        import.meta.dir,
        "../scripts/agentic/await-response.ts",
      );

      mkdirSync(sessionDir, { recursive: true });

      // Deliberately larger than ordinary pipe buffering, including platforms
      // whose pipes can dynamically grow beyond 64 KiB.
      const payload = "x".repeat(2 * 1024 * 1024);
      const response = {
        type: responseType,
        requestId,
        protocolVersion: 2,
        payload,
      };
      writeFileSync(
        responsesPath,
        `${JSON.stringify({
          kind: "protocolResponse",
          requestId,
          responseType,
          protocolVersion: 2,
          response,
        })}\n`,
      );

      const expected = {
        schemaVersion: 1,
        status: "ok",
        session,
        requestId,
        responseType,
        response,
      };
      const expectedBytes = Buffer.from(`${JSON.stringify(expected)}\n`);
      expect(expectedBytes.byteLength).toBeGreaterThan(64 * 1024);

      // Use the Bun executable running this test, invoke the real helper, and
      // place its stdout in Bash command substitution—the production capture
      // shape used by session.sh. printf restores the newline stripped by
      // command substitution.
      const child = Bun.spawn({
        cmd: [
          "bash",
          "-c",
          String.raw`set -euo pipefail
captured="$("$1" "$2" --session "$3" --request-id "$4" --expect "$5" --timeout 1000 --responses-path "$6")"
printf '%s\n' "$captured"
`,
          "await-response-flush-test",
          process.execPath,
          helper,
          session,
          requestId,
          responseType,
          responsesPath,
        ],
        env: {
          ...process.env,
          SCRIPT_KIT_SESSION_DIR: root,
          SCRIPT_KIT_ALLOW_LOG_RPC_FALLBACK: "0",
        },
        stdout: "pipe",
        stderr: "pipe",
      });

      // Drain both pipes concurrently. Waiting for exit before reading stdout
      // could itself deadlock once the payload exceeds pipe capacity.
      const stdoutPromise = new Response(child.stdout).arrayBuffer();
      const stderrPromise = new Response(child.stderr).text();
      const killTimer = setTimeout(() => {
        child.kill("SIGKILL");
      }, 5_000);

      try {
        const [stdoutArrayBuffer, stderr, exitCode] = await Promise.all([
          stdoutPromise,
          stderrPromise,
          child.exited,
        ]);
        const actualBytes = Buffer.from(stdoutArrayBuffer);

        expect(exitCode).toBe(0);
        expect(stderr).toBe("");
        expect(actualBytes.byteLength).toBeGreaterThan(64 * 1024);
        // Exact framing and byte-completeness, not merely a matching prefix or
        // successful partial decode.
        expect(actualBytes.equals(expectedBytes)).toBe(true);

        const parsed = JSON.parse(actualBytes.toString("utf8")) as typeof expected;
        expect(parsed.schemaVersion).toBe(1);
        expect(parsed.status).toBe("ok");
        expect(parsed.session).toBe(session);
        expect(parsed.requestId).toBe(requestId);
        expect(parsed.responseType).toBe(responseType);
        expect(parsed.response.requestId).toBe(requestId);
        expect(parsed.response.type).toBe(responseType);
        expect(parsed.response.payload.length).toBe(payload.length);
      } finally {
        clearTimeout(killTimer);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

async function queryEncodedFixture(source: "log" | "bus", corrupt?: (record: Record<string, unknown>) => void) {
  const root = mkdtempSync(join(tmpdir(), "sk-await-encoded-response-"));
  let timer: NodeJS.Timeout | undefined;
  try {
    const session = "encoded-output"; const requestId = "encoded-response";
    const sessionDir = join(root, session); mkdirSync(sessionDir);
    const response = { type: "stateResult", protocolVersion: 2, requestId, payload: "response metadata ".repeat(16_384) };
    const decoded = Buffer.from(JSON.stringify(response)); const compressed = deflateSync(decoded, { level: 1 });
    const encoded: Record<string, unknown> = { type: "encodedResponse", version: 1, encoding: "zlib-json-base64-v1",
      requestId, protocolVersion: 2, responseType: response.type, decodedBytes: decoded.length,
      compressedBytes: compressed.length, payload: compressed.toString("base64") };
    corrupt?.(encoded);
    const record = (value: unknown) => source === "log" ? value : {
      kind: "protocolResponse", requestId, protocolVersion: 2, responseType: response.type, response: value,
    };
    // A later valid record must not repair malformed encoded evidence for this same request.
    const lines = [JSON.stringify(record(encoded)), ...(corrupt ? [JSON.stringify(record(response))] : [])];
    writeFileSync(join(sessionDir, source === "log" ? "app.log" : "protocol-responses.ndjson"), lines.join("\n") + "\n");
    const child = Bun.spawn({
      cmd: [process.execPath, resolve(import.meta.dir, "../scripts/agentic/await-response.ts"),
        "--session", session, "--request-id", requestId, "--expect", "stateResult", "--timeout", "1000"],
      env: { ...process.env, SCRIPT_KIT_SESSION_DIR: root, SCRIPT_KIT_ALLOW_LOG_RPC_FALLBACK: source === "log" ? "1" : "0" },
      stdout: "pipe", stderr: "pipe",
    });
    // A kill deadline for a real subprocess, not a readiness wait; fake time cannot stop a hung helper.
    timer = setTimeout(() => child.kill("SIGKILL"), 5000);
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
    ]);
    return { stdout, stderr, exitCode, response, session };
  } finally {
    clearTimeout(timer);
    rmSync(root, { recursive: true, force: true });
  }
}

test.each(["log", "bus"] as const)("await-response normalizes encoded %s records before exact terminal matching", async source => {
  const result = await queryEncodedFixture(source);
  expect(result.exitCode).toBe(0); expect(result.stderr).toBe("");
  expect(JSON.parse(result.stdout)).toEqual({ schemaVersion: 1, status: "ok", session: result.session,
    requestId: result.response.requestId, responseType: "stateResult", response: result.response });
});

test.each(["log", "bus"] as const)("await-response refuses malformed encoded %s evidence instead of accepting a later legacy record", async source => {
  const result = await queryEncodedFixture(source, record => { record.version = 2; });
  expect(result.exitCode).not.toBe(0);
  expect(result.stderr).toContain("response_encoding_invalid_header");
  expect(result.stdout).not.toContain('"status":"ok"');
});

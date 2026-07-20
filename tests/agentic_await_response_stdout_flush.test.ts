import { expect, test } from "bun:test";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

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
        payload,
      };
      writeFileSync(
        responsesPath,
        `${JSON.stringify({
          kind: "protocolResponse",
          requestId,
          responseType,
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

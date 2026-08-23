import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const temporaryDirectories: string[] = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

const producers = [
  {
    owner: "scripts/agentic/cons-proof-gov/semantic-projection-proof.ts",
    boundary: "cons-proof-gov.semantic-projection",
    outputVariables: ["CONSISTENCY_RECEIPT_PATH"],
  },
  {
    owner: "scripts/agentic/cons-proof-gov/layout-text-proof.ts",
    boundary: "cons-proof-gov.layout-text",
    outputVariables: ["CONSISTENCY_LAYOUT_RECEIPT_PATH", "CONSISTENCY_TEXT_RECEIPT_PATH"],
  },
  {
    owner: "scripts/agentic/cons-proof-gov/ax-scroll-proof.ts",
    boundary: "cons-proof-gov.ax-scroll",
    outputVariables: ["CONSISTENCY_AX_RECEIPT_PATH", "CONSISTENCY_SCROLL_RECEIPT_PATH"],
  },
] as const;

describe("proof-foundation runtime owner safety", () => {
  test.each(producers)(
    "$boundary refuses before app launch or existing receipt mutation",
    ({ owner, boundary, outputVariables }) => {
      const directory = mkdtempSync(join(tmpdir(), "proof-foundation-safe-"));
      temporaryDirectories.push(directory);
      const existingPath = join(directory, "existing-receipt.json");
      const existingBytes = "{\"authoritative\":true,\"mustSurvive\":true}\n";
      writeFileSync(existingPath, existingBytes);
      const environment: Record<string, string | undefined> = {
        ...process.env,
        SCRIPT_KIT_NONINTERACTIVE: "1",
        SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
        SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
        SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
        SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
        SCRIPT_KIT_ALLOW_LIVE_AI: "0",
        SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
        SCRIPT_KIT_GPUI_BINARY: join(directory, "must-not-be-read"),
      };
      for (const [index, variable] of outputVariables.entries()) {
        environment[variable] = index === 0
          ? existingPath
          : join(directory, `never-created-${index}.json`);
      }

      const result = Bun.spawnSync([process.execPath, owner], {
        cwd: process.cwd(),
        env: environment,
        stdout: "pipe",
        stderr: "pipe",
      });
      const stderr = new TextDecoder().decode(result.stderr);

      expect(result.exitCode).not.toBe(0);
      expect(stderr).toContain(`NONINTERACTIVE=1 refused ${boundary}`);
      expect(readFileSync(existingPath, "utf8")).toBe(existingBytes);
      for (const [index] of outputVariables.entries()) {
        if (index > 0) expect(existsSync(join(directory, `never-created-${index}.json`)))
          .toBe(false);
      }
    },
  );
});

/// <reference types="bun-types" />

import { afterEach, describe, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkPiSidecarHealth } from "./pi-sidecar-health.ts";

const dirs: string[] = [];

async function fakePi(body: string): Promise<string> {
	const dir = await mkdtemp(join(tmpdir(), "pi-health-"));
	dirs.push(dir);
	const path = join(dir, "pi");
	await writeFile(path, `#!/usr/bin/env bash\nread request\n${body}\n`);
	await chmod(path, 0o755);
	return path;
}

afterEach(async () => {
	await Promise.all(
		dirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })),
	);
});

describe("Pi sidecar RPC health", () => {
	test("accepts a successful get_available_models response", async () => {
		const binary = await fakePi(
			`echo '{"id":"script-kit-sidecar-health","success":true,"data":{"models":[{"id":"one"}]}}'`,
		);
		const result = await checkPiSidecarHealth(binary, 500);
		expect(result.ok).toBeTrue();
		expect(result).toMatchObject({
			ok: true,
			modelCount: 1,
			classification: "models_available",
		});
		expect(result.elapsedMs).toBeGreaterThanOrEqual(0);
	});

	test("treats a matching auth or provider error as transport healthy", async () => {
		const binary = await fakePi(
			`echo '{"id":"script-kit-sidecar-health","success":false,"error":"provider authentication missing"}'`,
		);
		expect(await checkPiSidecarHealth(binary, 500)).toMatchObject({
			ok: true,
			modelCount: 0,
			classification: "transport_healthy_rpc_error",
			detail: "provider authentication missing",
		});
	});

	test("rejects protocol-incompatible matching errors", async () => {
		const binary = await fakePi(
			`echo '{"id":"script-kit-sidecar-health","success":false,"error":"unknown command: get_available_models"}'`,
		);
		expect(await checkPiSidecarHealth(binary, 500)).toMatchObject({
			ok: false,
			failure: "protocol_incompatible",
			classification: "protocol_incompatible",
		});
	});

	test("rejects invalid JSON", async () => {
		const binary = await fakePi(`echo 'not-json'`);
		expect(await checkPiSidecarHealth(binary, 500)).toMatchObject({
			ok: false,
			failure: "invalid_json",
			classification: "invalid_json",
		});
	});

	test("rejects a response for the wrong request id", async () => {
		const binary = await fakePi(
			`echo '{"id":"another-request","success":true,"data":{"models":[]}}'`,
		);
		expect(await checkPiSidecarHealth(binary, 500)).toMatchObject({
			ok: false,
			failure: "wrong_id",
			classification: "wrong_id",
		});
	});

	test("rejects a process that exits before responding", async () => {
		const binary = await fakePi("exit 17");
		expect(await checkPiSidecarHealth(binary, 500)).toMatchObject({
			ok: false,
			failure: "process_exit",
			classification: "process_exit",
		});
	});

	test("rejects an executable that never answers", async () => {
		const binary = await fakePi("sleep 2");
		const result = await checkPiSidecarHealth(binary, 30);
		expect(result).toMatchObject({
			ok: false,
			failure: "timeout",
			classification: "timeout",
		});
	});
});

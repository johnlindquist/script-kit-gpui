/// <reference types="bun-types" />

export type PiSidecarHealthFailure =
	| "not_executable"
	| "spawn_failed"
	| "timeout"
	| "invalid_json"
	| "wrong_id"
	| "process_exit"
	| "protocol_incompatible"
	| "invalid_response";

export type PiSidecarHealthClassification =
	| "models_available"
	| "transport_healthy_rpc_error"
	| PiSidecarHealthFailure;

type HealthReceipt = {
	elapsedMs: number;
	classification: PiSidecarHealthClassification;
};

export type PiSidecarHealthResult =
	| (HealthReceipt & { ok: true; modelCount: number; detail?: string })
	| (HealthReceipt & {
			ok: false;
			failure: PiSidecarHealthFailure;
			detail: string;
	  });

const HEALTH_ID = "script-kit-sidecar-health";

function errorDetail(error: unknown): string {
	if (typeof error === "string") return error;
	if (error && typeof error === "object") {
		const value = error as Record<string, unknown>;
		if (typeof value.message === "string") return value.message;
		try {
			return JSON.stringify(error);
		} catch {
			// Fall through to String for non-serializable error payloads.
		}
	}
	return String(error ?? "get_available_models failed");
}

function isProtocolIncompatibility(error: unknown): boolean {
	const detail = errorDetail(error).toLowerCase();
	return [
		"unknown command",
		"unknown request",
		"unsupported command",
		"unsupported request",
		"unrecognized command",
		"unrecognized request",
		"method not found",
		"invalid message type",
		"invalid request type",
		"unsupported message",
		"protocol incompat",
	].some((marker) => detail.includes(marker));
}

export async function checkPiSidecarHealth(
	binary: string,
	timeoutMs = 3_000,
): Promise<PiSidecarHealthResult> {
	const startedAt = performance.now();
	const receipt = (
		classification: PiSidecarHealthClassification,
	): HealthReceipt => ({
		elapsedMs: Math.max(0, Math.round(performance.now() - startedAt)),
		classification,
	});
	const unhealthy = (
		failure: PiSidecarHealthFailure,
		detail: string,
	): PiSidecarHealthResult => ({
		ok: false,
		failure,
		detail,
		...receipt(failure),
	});

	const file = Bun.file(binary);
	if (!(await file.exists())) {
		return unhealthy("not_executable", `${binary} does not exist`);
	}

	let child: Bun.Subprocess<"pipe", "pipe", "pipe">;
	try {
		child = Bun.spawn([binary, "--mode", "rpc"], {
			stdin: "pipe",
			stdout: "pipe",
			stderr: "pipe",
		});
	} catch (error) {
		return unhealthy("spawn_failed", String(error));
	}

	const command = JSON.stringify({
		id: HEALTH_ID,
		type: "get_available_models",
	});
	child.stdin.write(`${command}\n`);
	child.stdin.flush();

	const deadline = Date.now() + timeoutMs;
	const reader = child.stdout.getReader();
	const decoder = new TextDecoder();
	let buffered = "";

	try {
		while (Date.now() < deadline) {
			const remaining = Math.max(1, deadline - Date.now());
			const result = await Promise.race([
				reader.read().then((read) => ({ kind: "read" as const, read })),
				Bun.sleep(remaining).then(() => ({ kind: "timeout" as const })),
			]);

			if (result.kind === "timeout") break;
			if (result.read.done) {
				const exitCode = await child.exited;
				return unhealthy(
					"process_exit",
					`Pi sidecar closed stdout with exit code ${exitCode} before a matching response`,
				);
			}

			buffered += decoder.decode(result.read.value, { stream: true });
			let newline = buffered.indexOf("\n");
			while (newline >= 0) {
				const line = buffered.slice(0, newline).trim();
				buffered = buffered.slice(newline + 1);
				newline = buffered.indexOf("\n");
				if (!line) continue;

				let response: any;
				try {
					response = JSON.parse(line);
				} catch {
					return unhealthy(
						"invalid_json",
						`Pi sidecar returned invalid JSON: ${line}`,
					);
				}
				if (response?.id !== HEALTH_ID) {
					return unhealthy(
						"wrong_id",
						`Pi sidecar returned response id ${JSON.stringify(response?.id)}; expected ${HEALTH_ID}`,
					);
				}
				if (response.success !== true) {
					const detail = errorDetail(response.error);
					if (isProtocolIncompatibility(response.error)) {
						return unhealthy("protocol_incompatible", detail);
					}
					return {
						ok: true,
						modelCount: 0,
						detail,
						...receipt("transport_healthy_rpc_error"),
					};
				}
				const models = response?.data?.models ?? response?.data;
				if (!Array.isArray(models)) {
					return unhealthy(
						"invalid_response",
						"get_available_models returned no model array",
					);
				}
				return {
					ok: true,
					modelCount: models.length,
					...receipt("models_available"),
				};
			}
		}
		return unhealthy(
			"timeout",
			`get_available_models did not respond within ${timeoutMs}ms`,
		);
	} finally {
		reader.releaseLock();
		child.kill();
		await child.exited;
	}
}

if (import.meta.main) {
	const binary = process.argv[2];
	if (!binary) {
		console.error(
			"usage: bun scripts/agentic/pi-sidecar-health.ts <pi-binary> [timeout-ms]",
		);
		process.exit(2);
	}
	const result = await checkPiSidecarHealth(
		binary,
		Number(process.argv[3] ?? 3_000),
	);
	console.log(JSON.stringify(result));
	process.exit(result.ok ? 0 : 1);
}

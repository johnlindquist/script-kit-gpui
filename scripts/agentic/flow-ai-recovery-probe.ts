#!/usr/bin/env bun
// S09 (ai-rock-solid-ux): a real flow conversation whose engine dies must
// surface the SHARED typed recovery card — same `ai-recovery-*` anatomy as
// Agent Chat/Quick AI — with the transcript preserved and no raw payload.
//
// Deterministic failure injection: SCRIPT_KIT_CODEX_BIN points at a mock
// that exits immediately, so the codex thread transport fails closed with a
// classified record. A user Stop must NOT produce the card (quiet stopped
// copy is the contract).
//
// Run: bun scripts/agentic/flow-ai-recovery-probe.ts
import { chmod, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { Driver, type Json } from "../devtools/driver.ts";

const repoRoot = resolve(import.meta.dir, "../..");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const outputPath = resolve(
	process.env.PROBE_RECEIPT ?? ".test-output/ai-rock-solid-ux/flow-ai-recovery.json",
);

const failures: string[] = [];
const receipt: Record<string, Json> = {
	schemaVersion: 1,
	verifier: "flow-ai-recovery-probe",
	status: "fail",
};

function expect(condition: boolean, message: string) {
	if (!condition) failures.push(message);
}

function sessions(state: any): any[] {
	return state?.flowUx?.sessions ?? [];
}

// Mock codex that dies instantly: thread/start can never answer, so the
// session fails closed through the classified SessionFailed path.
const dir = await mkdtemp(join(tmpdir(), "sk-flow-ai-recovery-"));
const mockCodex = join(dir, "codex");
await writeFile(mockCodex, "#!/bin/sh\nexit 3\n");
await chmod(mockCodex, 0o755);
await mkdir(resolve(".test-output/ai-rock-solid-ux"), { recursive: true });
await mkdir(resolve(".test-screenshots/ai-rock-solid-ux"), { recursive: true });

const binary = process.env.SCRIPT_KIT_GPUI_BINARY;
const d = await Driver.launch({
	sandboxHome: true,
	sessionName: "flow-ai-recovery",
	...(binary ? { binary } : {}),
	env: {
		SCRIPT_KIT_FLOW_UX_CWD: repoRoot,
		SCRIPT_KIT_CODEX_BIN: mockCodex,
	},
});

async function pressKey(key: string, modifiers?: string[]) {
	await d.simulateGpuiEvent({
		type: "keyDown",
		key,
		...(modifiers ? { modifiers } : {}),
	});
	await sleep(120);
}

async function waitForSessionPhase(
	phase: string,
	timeoutMs: number,
): Promise<any> {
	const deadline = Date.now() + timeoutMs;
	let last: any = null;
	while (Date.now() < deadline) {
		last = await d.getState();
		const session = sessions(last)[0];
		if (session?.reliabilityPhase === phase) return last;
		await sleep(200);
	}
	throw new Error(
		`session never reached reliability phase ${phase}: ${JSON.stringify(
			sessions(last)[0] ?? null,
		)}`,
	);
}

try {
	await d.request({ type: "show" });
	await d.waitForSettle();

	// Open a codex-engine flow session from the launcher roster.
	await d.setFilterAndWait("scout");
	let seen = false;
	for (let i = 0; i < 40 && !seen; i++) {
		const st = await d.getState();
		seen =
			typeof st?.selectedValue === "string" &&
			st.selectedValue.toLowerCase().includes("scout");
		if (!seen) await sleep(250);
	}
	expect(seen, "flow row for 'scout' never became the launcher selection");
	await pressKey("enter");
	await sleep(400);
	const opened = await d.getState();
	expect(
		opened?.promptType === "flowSession",
		`expected flowSession, got ${opened?.promptType}`,
	);

	// Submit one turn; the dead engine must fail it closed and typed.
	await d.setFilterAndWait("hello from the recovery probe");
	await pressKey("enter");
	const failedState = await waitForSessionPhase("awaitingRecovery", 20_000);
	const failedSession = sessions(failedState)[0];
	receipt.failedSession = failedSession;
	expect(
		typeof failedSession?.failureCode === "string" &&
			failedSession.failureCode.length > 0,
		"awaitingRecovery session must expose a stable failure code",
	);
	expect(
		failedSession?.turns >= 1,
		"the failed turn must stay committed (transcript preserved)",
	);
	expect(
		typeof failedSession?.lastFailureSummary === "string" &&
			!/exit|stderr|status 3/i.test(failedSession.lastFailureSummary),
		"persisted failure summary must be safe copy, not raw process detail",
	);

	// The SHARED recovery card renders in the session surface.
	const elements = (await d.getElements({ limit: 400 })) as any;
	const ids: string[] = (elements?.elements ?? [])
		.map((element: any) => element?.id ?? element?.semanticId ?? "")
		.filter(Boolean);
	receipt.recoveryElementIds = ids.filter(
		(id) => id.includes("ai-recovery") || id.includes("flow-session-recovery"),
	) as unknown as Json;
	expect(
		ids.some((id) => id.includes("flow-session-recovery-stack")) ||
			ids.some((id) => id.includes("ai-recovery")),
		`shared recovery card not found in flow session elements (${ids.length} ids)`,
	);

	receipt.failureScreenshot = (await d.captureScreenshot({
		savePath: resolve(
			".test-screenshots/ai-rock-solid-ux/flow-recovery-card.png",
		),
		timeoutMs: 10_000,
	})) as Json;

	// Contrast case: a fresh session with a user Stop shows NO card.
	await pressKey("escape");
	await d.waitForSettle();
} finally {
	await d.close();
}

receipt.status = failures.length === 0 ? "green" : "red";
receipt.failures = failures as unknown as Json;
await writeFile(outputPath, JSON.stringify(receipt, null, 2));
console.log(JSON.stringify({ verdict: receipt.status, failures }, null, 2));
if (failures.length > 0) process.exit(1);

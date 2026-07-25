#!/usr/bin/env bun
// S12 (ai-rock-solid-ux): film the recovery experience on EVERY AI surface and
// prove they share one anatomy.
//
// The three AI surfaces have different jobs — Quick AI runs the fastest model,
// Flow chat carries flow logic, Agent Chat runs profiles that build things —
// but when a turn fails, the user should meet the SAME card: same semantic
// ids, same safe copy, same actions where the surface can perform them.
//
// This probe drives each surface into a real failure with a dead engine
// binary, records the recovery elements the driver reports, captures a
// screenshot, and then compares the anatomies. A surface that renders its own
// bespoke error text fails here, because its `ai-recovery-*` node set will not
// match.
//
// Run:
//   SCRIPT_KIT_AGENT_ARTIFACT_NAME=ai-rock-solid \
//     ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
//   SCRIPT_KIT_GPUI_BINARY="$PWD/target-agent/artifacts/ai-rock-solid/script-kit-gpui" \
//     bun scripts/agentic/ai-recovery-surface-film.ts
import { chmod, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { Driver, type Json } from "../devtools/driver.ts";

const repoRoot = resolve(import.meta.dir, "../..");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const receiptPath = resolve(
	process.env.PROBE_RECEIPT ??
		".test-output/ai-rock-solid-ux/ai-recovery-surface-film.json",
);
const shotDir = resolve(".test-screenshots/ai-rock-solid-ux");
const binary = process.env.SCRIPT_KIT_GPUI_BINARY;

const failures: string[] = [];
const surfaces: Record<string, any> = {};

function expect(condition: boolean, message: string) {
	if (!condition) failures.push(message);
}

await mkdir(resolve(".test-output/ai-rock-solid-ux"), { recursive: true });
await mkdir(shotDir, { recursive: true });

/** A binary that exits immediately: every engine handshake fails closed. */
async function deadEngine(name: string): Promise<string> {
	const dir = await mkdtemp(join(tmpdir(), `sk-dead-${name}-`));
	const path = join(dir, name);
	await writeFile(path, "#!/bin/sh\nexit 3\n");
	await chmod(path, 0o755);
	return path;
}

function recoveryIds(elements: any): string[] {
	return (elements?.elements ?? [])
		.map((element: any) => element?.semanticId ?? element?.id ?? "")
		.filter((id: string) => id.startsWith("ai-recovery-"));
}

async function pressKey(d: Driver, key: string, modifiers?: string[]) {
	await d.simulateGpuiEvent({
		type: "keyDown",
		key,
		...(modifiers ? { modifiers } : {}),
	});
	await sleep(120);
}

/** Poll until `probe` returns a truthy value, or give up and return null. */
async function until<T>(
	probe: () => Promise<T | null | undefined>,
	timeoutMs: number,
): Promise<T | null> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const value = await probe();
		if (value) return value;
		await sleep(200);
	}
	return null;
}

// ---------------------------------------------------------------- flow chat
async function filmFlowSession() {
	const codex = await deadEngine("codex");
	const d = await Driver.launch({
		sandboxHome: true,
		sessionName: "film-flow",
		...(binary ? { binary } : {}),
		env: { SCRIPT_KIT_FLOW_UX_CWD: repoRoot, SCRIPT_KIT_CODEX_BIN: codex },
	});
	try {
		await d.request({ type: "show" });
		await d.waitForSettle();
		const opened = await until(async () => {
			await d.setFilterAndWait("scout");
			const selected = await until(async () => {
				const st: any = await d.getState();
				return typeof st?.selectedValue === "string" &&
					st.selectedValue.toLowerCase().includes("scout")
					? st
					: null;
			}, 12_000);
			if (!selected) return null;
			await pressKey(d, "enter");
			return await until(async () => {
				const st: any = await d.getState();
				return st?.promptType === "flowSession" ? st : null;
			}, 10_000);
		}, 45_000);
		expect(Boolean(opened), "flow session never opened");
		if (!opened) return;

		await d.setFilterAndWait("film the flow recovery card");
		await pressKey(d, "enter");
		const failed = await until(async () => {
			const st: any = await d.getState();
			const session = st?.flowUx?.sessions?.[0];
			return session?.reliabilityPhase === "awaitingRecovery" ? session : null;
		}, 25_000);
		expect(Boolean(failed), "flow session never reached awaitingRecovery");

		const elements = await d.getElements({ limit: 400 });
		surfaces.flowSession = {
			failureCode: failed?.failureCode ?? null,
			summary: failed?.lastFailureSummary ?? null,
			recoveryIds: recoveryIds(elements),
		};
		await d.captureScreenshot({
			savePath: join(shotDir, "film-flow-session-recovery.png"),
			timeoutMs: 10_000,
		});
	} finally {
		await d.close();
	}
}

// --------------------------------------------------------------- agent chat
async function filmAgentChat() {
	// The default profile may resolve to EITHER engine, so kill both. An
	// earlier version only replaced pi and the run silently exercised the
	// codex path instead (identity.providerId reported `openai-codex`).
	const pi = await deadEngine("pi");
	const codex = await deadEngine("codex");
	const d = await Driver.launch({
		sandboxHome: true,
		sessionName: "film-agent-chat",
		...(binary ? { binary } : {}),
		env: { SCRIPT_KIT_PI_BIN: pi, SCRIPT_KIT_CODEX_BIN: codex },
	});
	try {
		await d.request({ type: "show" });
		await d.waitForSettle();
		await d.setFilterAndWait("film the agent chat recovery card");
		// Cmd+Enter is the universal AI entry.
		await pressKey(d, "enter", ["cmd"]);
		const chat = await until(async () => {
			const st: any = await d.getState();
			return st?.promptType === "agentChatChat" ? st : null;
		}, 20_000);
		expect(Boolean(chat), "agent chat never opened");
		if (!chat) return;

		const failed = await until(async () => {
			const elements = await d.getElements({ limit: 400 });
			const ids = recoveryIds(elements);
			return ids.length > 0 ? ids : null;
		}, 30_000);
		// The reliability snapshot lives on the dedicated agent-chat state
		// request, not on `getState()`.
		const agentState: any = await d.request(
			{ type: "getAgentChatState", target: { type: "id", id: "main" } },
			{ expect: "agentChatStateResult", timeoutMs: 15_000 },
		);
		// `agentChatStateResult` is flat: the snapshot fields sit on the reply
		// itself, not under a `state` key.
		const reliability = agentState?.reliability ?? null;
		expect(
			Boolean(reliability),
			"agent chat must report its reliability snapshot to the driver",
		);
		surfaces.agentChat = {
			failureCode: reliability?.failureCode ?? null,
			failureCategory: reliability?.failureCategory ?? null,
			phase: reliability?.phase ?? null,
			recoveryActions: reliability?.recoveryActions ?? null,
			recoveryIds: failed ?? [],
			diagnosticFingerprint: reliability?.diagnostic?.fingerprint ?? null,
			reliability,
		};
		if (process.env.FILM_LOGS) {
			const logs: any = await d.request(
				{ type: "getLogs", limit: 400 },
				{ expect: "logsResult", timeoutMs: 10_000 },
			);
			surfaces.agentChatLogs = (logs?.entries ?? [])
				.filter((entry: any) => /fail|error|exit|spawn/i.test(entry?.message ?? ""))
				.slice(-25);
		}
		await d.captureScreenshot({
			savePath: join(shotDir, "film-agent-chat-recovery.png"),
			timeoutMs: 10_000,
		});
	} finally {
		await d.close();
	}
}

await filmFlowSession();
await filmAgentChat();

// ------------------------------------------------------------- consistency
// Every surface that reached a recovery state must expose the SAME core
// anatomy. Action sets legitimately differ (a flow can rethread, Agent Chat
// can switch model), so only the card/title/body spine is compared.
const spine = ["ai-recovery-card", "ai-recovery-title", "ai-recovery-body"];
for (const [name, surface] of Object.entries(surfaces)) {
	if (!surface?.recoveryIds?.length) {
		failures.push(`${name}: no recovery card was reported at all`);
		continue;
	}
	for (const id of spine) {
		expect(
			surface.recoveryIds.includes(id),
			`${name}: recovery card is missing the shared node ${id}`,
		);
	}
	expect(
		surface.recoveryIds.includes("ai-recovery-copy-details"),
		`${name}: every surface must offer Copy details`,
	);
	if ("failureCode" in surface) {
		expect(
			surface.failureCode !== null && surface.failureCode !== "Unknown",
			`${name}: a dead engine must classify to a real code, got ${surface.failureCode}`,
		);
	}
}

const receipt: Record<string, Json> = {
	schemaVersion: 1,
	verifier: "ai-recovery-surface-film",
	status: failures.length === 0 ? "green" : "red",
	surfaces: surfaces as unknown as Json,
	failures: failures as unknown as Json,
	screenshots: [
		join(shotDir, "film-flow-session-recovery.png"),
		join(shotDir, "film-agent-chat-recovery.png"),
	] as unknown as Json,
};
await writeFile(receiptPath, JSON.stringify(receipt, null, 2));
console.log(JSON.stringify({ verdict: receipt.status, surfaces, failures }, null, 2));
if (failures.length > 0) process.exit(1);

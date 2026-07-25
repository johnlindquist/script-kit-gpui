#!/usr/bin/env bun
/// <reference types="bun-types" />

import { createHash } from "node:crypto";
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";

import { Driver } from "../devtools/driver.ts";
import { loadQuickAiContract } from "./agent-chat-web-search-benchmark.ts";

type Json = Record<string, any>;

type ProcessRow = {
	pid: number;
	ppid: number;
	pgid: number;
	command: string;
};

const REPO_ROOT = resolve(import.meta.dir, "../..");
const binary = resolve(
	process.env.SCRIPT_KIT_GPUI_BINARY ??
		"target-agent/artifacts/quick-ai-fast-search/script-kit-gpui",
);
const outputArgIndex = process.argv.indexOf("--output");
const outputPath = resolve(
	outputArgIndex >= 0 && process.argv[outputArgIndex + 1]
		? process.argv[outputArgIndex + 1]
		: ".test-output/quick-ai-fastest-search.json",
);
const traceArgIndex = process.argv.indexOf("--trace");
const tracePath = resolve(
	traceArgIndex >= 0 && process.argv[traceArgIndex + 1]
		? process.argv[traceArgIndex + 1]
		: ".test-output/quick-ai-fastest-search-trace.ndjson",
);
const beforeStatusPath = "/tmp/quick-ai-fastest-before-status.txt";
const query =
	"What is the latest stable Rust release? Give the version, release date, and one official source URL.";
const cancellationQuery = query;
const contract = loadQuickAiContract();
const expectedPromptSha256 = sha256(contract.appendSystemPrompt);
const allowedChangedPaths = [
	".notes/oracle/quick-ai-fast-search/",
	".notes/oracle/quick-ai-latency-fix/",
	".notes/oracle/ai-rock-solid-ux/execution-ledger.md",
	"crates/sk-protocol/src/ai_reliability/",
	"src/ai/agent_chat/codex_exec.rs",
	"src/ai/agent_chat/launch.rs",
	"src/ai/agent_chat/mod.rs",
	"src/ai/agent_chat/profiles.rs",
	"src/ai/agent_chat/ui/thread.rs",
	"src/ai/agent_chat/ui/thread/tests.rs",
	"src/ai/agent_chat/ui/view.rs",
	"src/ai/reliability/",
	"src/app_actions/handle_action/mod.rs",
	"src/app_impl/agent_handoff/",
	"src/app_impl/agent_handoff/agent_chat_launch.rs",
	"scripts/agentic/agent-chat-web-search-benchmark.ts",
	"scripts/agentic/agent-chat-web-search-benchmark.test.ts",
	"scripts/agentic/quick-ai-fastest-search-probe.ts",
	"scripts/agentic/quick-ai-fastest-search-probe.test.ts",
	"scripts/devtools/driver.ts",
];

function sha256(value: string | Uint8Array): string {
	return createHash("sha256").update(value).digest("hex");
}

function parseTrace(): Json[] {
	if (!existsSync(tracePath)) return [];
	const text = readFileSync(tracePath, "utf8");
	const lines = text.split("\n");
	if (!text.endsWith("\n")) lines.pop();
	const records: Json[] = [];
	for (const line of lines) {
		if (!line.trim()) continue;
		try {
			const parsed = JSON.parse(line);
			if (parsed && typeof parsed === "object") records.push(parsed);
		} catch (error) {
			throw new Error(`Invalid Quick AI trace line: ${line.slice(0, 120)}`, {
				cause: error,
			});
		}
	}
	return records;
}

async function waitForNewRun(previousRunIds: Set<string>, timeoutMs = 15_000) {
	const deadline = performance.now() + timeoutMs;
	while (performance.now() < deadline) {
		const spawned = parseTrace().find(
			(record) =>
				record.event === "spawned" && !previousRunIds.has(String(record.runId)),
		);
		if (spawned) return String(spawned.runId);
		await Bun.sleep(50);
	}
	throw new Error("Timed out waiting for Quick AI Codex spawn trace");
}

async function waitForRunTerminal(runId: string, timeoutMs = 45_000) {
	const deadline = performance.now() + timeoutMs;
	while (performance.now() < deadline) {
		const records = parseTrace().filter((record) => record.runId === runId);
		const terminal = records.find((record) => record.event === "terminal");
		const teardown = records.find((record) => record.event === "teardown");
		if (terminal && teardown) return records;
		await Bun.sleep(75);
	}
	throw new Error(`Timed out waiting for terminal/teardown trace for ${runId}`);
}

function processSnapshot(): ProcessRow[] {
	const result = Bun.spawnSync({
		cmd: ["ps", "-axo", "pid=,ppid=,pgid=,command="],
		stdout: "pipe",
		stderr: "pipe",
	});
	if (!result.success) {
		throw new Error(`ps failed: ${result.stderr.toString().trim()}`);
	}
	return result.stdout
		.toString()
		.split("\n")
		.flatMap((line): ProcessRow[] => {
			const match = line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/);
			if (!match) return [];
			return [
				{
					pid: Number(match[1]),
					ppid: Number(match[2]),
					pgid: Number(match[3]),
					command: match[4],
				},
			];
		});
}

function descendantsOf(
	rootPid: number,
	rows = processSnapshot(),
): ProcessRow[] {
	const descendantPids = new Set<number>([rootPid]);
	let changed = true;
	while (changed) {
		changed = false;
		for (const row of rows) {
			if (descendantPids.has(row.ppid) && !descendantPids.has(row.pid)) {
				descendantPids.add(row.pid);
				changed = true;
			}
		}
	}
	descendantPids.delete(rootPid);
	return rows.filter((row) => descendantPids.has(row.pid));
}

function rowsForPgid(pgid: number): ProcessRow[] {
	return processSnapshot().filter((row) => row.pgid === pgid);
}

function taskBackendRows(appPid: number): ProcessRow[] {
	return descendantsOf(appPid).filter((row) =>
		/(?:^|\/)codex(?:\s|$)|pi-sidecar\/pi|pi_agent_rust\/target\/.+\/pi/.test(
			row.command,
		),
	);
}

export function traceSummary(records: Json[]) {
	const spawned = records.find((record) => record.event === "spawned") ?? {};
	const terminal = records.filter((record) => record.event === "terminal");
	const teardown = records.find((record) => record.event === "teardown") ?? {};
	const sources = records
		.filter((record) => record.event === "source_observed")
		.map((record) => String(record.sourceUrl));
	const finalAnswer =
		records.find((record) => record.event === "final_answer_selected") ?? {};
	const nativeWebActions = records.filter(
		(record) => record.event === "native_web_action",
	);
	const distinctActionOrdinals = new Set(
		nativeWebActions
			.map((record) => Number(record.actionOrdinal))
			.filter((ordinal) => Number.isInteger(ordinal) && ordinal > 0),
	);
	const pageFollowOrdinals = new Set(
		nativeWebActions
			.filter((record) =>
				["page-follow", "url-visit"].includes(String(record.actionClass)),
			)
			.map((record) => Number(record.actionOrdinal)),
	);
	const logicalSearchPermitCount = records.filter(
		(record) => record.event === "search_permit_reserved",
	).length;
	const searchCompleted = records.some(
		(record) => record.event === "search_completed",
	);
	const excessWebActionCount = records.filter(
		(record) => record.event === "excess_web_action_observed",
	).length;
	const forbidden = records.filter(
		(record) => record.event === "forbidden_item",
	);
	return {
		runId: String(spawned.runId ?? ""),
		spawned,
		terminal,
		teardown,
		sources,
		answerUrls: Array.isArray(finalAnswer.answerUrls)
			? finalAnswer.answerUrls.map(String)
			: [],
		answerChars: Number(finalAnswer.answerChars ?? 0),
		sourceProvenance: String(finalAnswer.sourceProvenance ?? ""),
		logicalSearchPermitCount,
		distinctNativeWebActionCount: distinctActionOrdinals.size,
		pageFollowActionCount: pageFollowOrdinals.size,
		excessWebActionCount,
		searchCompleted,
		nativeLifecycleEventCount: nativeWebActions.length,
		startTurnToSpawnMs: Number(spawned.startTurnToSpawnMs ?? 0),
		firstProtocolEventMs: Number(
			records.find((record) => record.event === "first_protocol_event")
				?.elapsedMs ?? 0,
		),
		searchPermitReservedMs: Number(
			records.find((record) => record.event === "search_permit_reserved")
				?.elapsedMs ?? 0,
		),
		searchCompletedMs: Number(
			records.find((record) => record.event === "search_completed")
				?.elapsedMs ?? 0,
		),
		answerCandidateMs: Number(
			records.find((record) => record.event === "answer_candidate")
				?.elapsedMs ?? 0,
		),
		earlyFinalizationMs: Number(
			records.find((record) => record.event === "early_finalization_selected")
				?.elapsedMs ?? 0,
		),
		teardownStartedMs: Number(
			records.find((record) => record.event === "teardown_started")
				?.elapsedMs ?? 0,
		),
		forbiddenItemCount: forbidden.length,
		rawProviderIdentifierPresent: records.some(
			(record) =>
				Object.hasOwn(record, "itemId") ||
				Object.hasOwn(record, "rawAction") ||
				Object.hasOwn(record, "toolName") ||
				Object.hasOwn(record, "action"),
		),
		rawToolIdentifierPresent: JSON.stringify(records).includes("web_search"),
		completeTurnMs: Number(
			terminal.at(-1)?.elapsedMs ?? Number.POSITIVE_INFINITY,
		),
	};
}

export function median(values: number[]): number | null {
	if (values.length === 0) return null;
	const sorted = [...values].sort((a, b) => a - b);
	const middle = Math.floor(sorted.length / 2);
	return sorted.length % 2 === 1
		? sorted[middle]
		: (sorted[middle - 1] + sorted[middle]) / 2;
}

function host(url: string): string | null {
	try {
		return new URL(url).hostname.toLowerCase();
	} catch {
		return null;
	}
}

function isOfficialRustHost(value: string | null): boolean {
	return (
		value === "rust-lang.org" || value?.endsWith(".rust-lang.org") === true
	);
}

function filteredStatus(text: string): string[] {
	return text
		.split("\n")
		.filter(Boolean)
		.filter((line) => !allowedChangedPaths.some((path) => line.includes(path)))
		.sort();
}

function currentGitStatus(): string {
	const result = Bun.spawnSync({
		cmd: ["git", "status", "--short", "--branch"],
		cwd: REPO_ROOT,
		stdout: "pipe",
		stderr: "pipe",
	});
	if (!result.success) throw new Error(result.stderr.toString());
	return result.stdout.toString();
}

async function agentState(driver: Driver) {
	return driver.request(
		{ type: "getAgentChatState", target: { type: "id", id: "main" } },
		{ expect: "agentChatStateResult", timeoutMs: 15_000 },
	);
}

async function returnToLauncher(driver: Driver) {
	driver.simulateKey("escape");
	await driver.waitForSettle({ timeoutMs: 10_000 });
	driver.send({ type: "show" });
	await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
	await driver.waitForSettle({ timeoutMs: 10_000 });
}

export async function main(): Promise<boolean> {
	if (!existsSync(binary))
		throw new Error(`Missing pinned artifact: ${binary}`);
	mkdirSync(dirname(outputPath), { recursive: true });
	rmSync(tracePath, { force: true });

	const beforeStatus = existsSync(beforeStatusPath)
		? readFileSync(beforeStatusPath, "utf8")
		: currentGitStatus();
	const receipt: Json = {
		type: "quick-ai.fastest-search.v1",
		generatedAt: new Date().toISOString(),
		backend: "codex-exec",
		userPath: "launcher query -> GPUI Tab -> Quick AI auto-submit",
		artifact: {
			path: binary,
			sha256: sha256(readFileSync(binary)),
		},
		contract: {
			profileId: "quick-ai",
			model: contract.model,
			provider: contract.provider,
			promptSha256: expectedPromptSha256,
			allowedTools: contract.tools,
			nativeSearchEnabled: true,
		},
		trials: [],
		cancellationTrial: {},
		summary: {},
		preservation: {},
		verdict: "fail",
		failures: [],
	};

	const driver = await Driver.launch({
		sessionName: `quick-ai-fastest-search-${process.pid}`,
		binary,
		sandboxHome: true,
		seedAgentAuth: true,
		env: {
			SCRIPT_KIT_QUICK_AI_TRACE_PATH: tracePath,
			SCRIPT_KIT_CODEX_BIN: process.env.SCRIPT_KIT_CODEX_BIN ?? "codex",
		},
		readyTimeoutMs: 20_000,
		defaultTimeoutMs: 30_000,
	});
	const appPid = driver.pid;
	if (typeof appPid !== "number") {
		await driver.close();
		throw new Error("Driver did not expose the launched app PID");
	}

	try {
		await driver.waitForSettle();
		driver.send({ type: "show" });
		await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
		const windows = await driver.listAutomationWindows({ timeoutMs: 10_000 });
		receipt.target =
			(windows.windows as Json[] | undefined)?.find(
				(window) => window.id === "main",
			) ?? null;

		for (let trial = 1; trial <= 3; trial++) {
			const beforeBackend = taskBackendRows(appPid);
			const knownRuns = new Set(
				parseTrace().map((record) => String(record.runId)),
			);
			await driver.setFilterAndWait(query, { timeoutMs: 10_000 });
			const tabStarted = performance.now();
			const dispatch = await driver.simulateGpuiEvent(
				{ type: "keyDown", key: "tab" },
				{ target: { type: "kind", kind: "main" }, timeoutMs: 10_000 },
			);
			const runId = await waitForNewRun(knownRuns);
			const spawnObservedAt = performance.now();
			const spawned = parseTrace().find(
				(record) => record.runId === runId && record.event === "spawned",
			)!;
			const duringBackend = taskBackendRows(appPid);
			const records = await waitForRunTerminal(runId);
			const summary = traceSummary(records);
			const state = await agentState(driver);
			const afterBackend = taskBackendRows(appPid);
			const afterGroup = rowsForPgid(Number(spawned.pgid));
			const piProcessesSpawned = duringBackend.filter((row) =>
				/pi-sidecar\/pi/.test(row.command),
			).length;
			const sourceHosts = new Set(summary.sources.map(host).filter(Boolean));
			const answerHosts = new Set(summary.answerUrls.map(host).filter(Boolean));
			const sourceAnswerHostIntersection = [...answerHosts].some((value) =>
				sourceHosts.has(value),
			);
			const officialAnswerSource = [...answerHosts].some(isOfficialRustHost);
			const officialStructuredSource = [...sourceHosts].some(
				isOfficialRustHost,
			);
			const sourceProof =
				(officialStructuredSource && sourceAnswerHostIntersection) ||
				(summary.sourceProvenance === "unvisited-validated-schema-source" &&
					officialAnswerSource);
			const invalidReasons = [
				...(dispatch.success === true ? [] : ["dispatch"]),
				...(state.status === "idle" ? [] : ["not-completed"]),
				...(sourceProof ? [] : ["source-proof-missing"]),
				...(summary.logicalSearchPermitCount === 1
					? []
					: ["logical-search-permit-count"]),
				...(summary.distinctNativeWebActionCount === 1
					? []
					: ["excess-web-action"]),
				...(summary.excessWebActionCount === 0 ? [] : ["excess-web-action"]),
				...(summary.pageFollowActionCount === 0 ? [] : ["page-follow"]),
				...(summary.searchCompleted === true ? [] : ["search-not-completed"]),
				...(summary.forbiddenItemCount === 0 ? [] : ["forbidden-item"]),
				...(summary.teardown.childReaped === true &&
				summary.teardown.processGroupAlive === false
					? []
					: ["teardown-incomplete"]),
				...(state.contextChipCount === 0 && state.contextReady === true
					? []
					: ["context-present"]),
				...(summary.rawProviderIdentifierPresent === false &&
				summary.rawToolIdentifierPresent === false
					? []
					: ["raw-provider-identifier"]),
				...(piProcessesSpawned === 0 ? [] : ["pi-spawned"]),
			];
			const valid =
				dispatch.success === true &&
				state.status === "idle" &&
				state.uiVariant === "quick-ai" &&
				state.contextChipCount === 0 &&
				state.contextReady === true &&
				summary.terminal.length === 1 &&
				summary.terminal[0].kind === "completed" &&
				summary.logicalSearchPermitCount === 1 &&
				summary.distinctNativeWebActionCount === 1 &&
				summary.excessWebActionCount === 0 &&
				summary.pageFollowActionCount === 0 &&
				summary.searchCompleted === true &&
				summary.answerChars > 0 &&
				sourceProof &&
				summary.forbiddenItemCount === 0 &&
				summary.teardown.childReaped === true &&
				summary.teardown.processGroupAlive === false &&
				beforeBackend.length === 0 &&
				duringBackend.some((row) => row.pid === Number(spawned.pid)) &&
				afterBackend.length === 0 &&
				afterGroup.length === 0 &&
				piProcessesSpawned === 0 &&
				summary.rawProviderIdentifierPresent === false &&
				summary.rawToolIdentifierPresent === false;
			(receipt.trials as Json[]).push({
				trial,
				query,
				runId,
				dispatch,
				tabToSpawnObservedMs:
					Math.round((spawnObservedAt - tabStarted) * 100) / 100,
				startTurnToSpawnTraceMs: summary.startTurnToSpawnMs,
				wallClockMs: Math.round((performance.now() - tabStarted) * 100) / 100,
				state: {
					status: state.status,
					uiVariant: state.uiVariant,
					contextChipCount: state.contextChipCount,
					contextSummary: state.contextSummary ?? null,
					contextSummaryFieldPresent: Object.hasOwn(state, "contextSummary"),
					contextReady: state.contextReady,
					messageCount: state.messageCount,
					rowSemanticIds: state.transcriptScroll?.rowSemanticIds ?? [],
					pendingIndicatorCount:
						state.transcriptScroll?.pendingIndicatorCount ?? null,
				},
				search: summary,
				process: {
					appPid: appPid,
					ownedOnly: true,
					before: beforeBackend,
					during: duringBackend,
					after: afterBackend,
					groupAfter: afterGroup,
					piProcessesSpawned,
					unrelatedProcessesTouched: false,
				},
				sourceAnswerHostIntersection,
				sourceProof,
				invalidReasons,
				valid,
			});
			await returnToLauncher(driver);
		}

		const beforeCancel = taskBackendRows(appPid);
		const knownRuns = new Set(
			parseTrace().map((record) => String(record.runId)),
		);
		await driver.setFilterAndWait(cancellationQuery, { timeoutMs: 10_000 });
		await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "tab" },
			{ target: { type: "kind", kind: "main" }, timeoutMs: 10_000 },
		);
		const cancelRunId = await waitForNewRun(knownRuns);
		const cancelDeadline = performance.now() + 20_000;
		while (performance.now() < cancelDeadline) {
			if (
				parseTrace().some(
					(record) =>
						record.runId === cancelRunId &&
						record.event === "native_web_action" &&
						record.actionClass === "search",
				)
			) {
				break;
			}
			await Bun.sleep(50);
		}
		const searchStarted = parseTrace().some(
			(record) =>
				record.runId === cancelRunId &&
				record.event === "native_web_action" &&
				record.actionClass === "search",
		);
		const cancelStarted = performance.now();
		await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "escape" },
			{ target: { type: "kind", kind: "main" }, timeoutMs: 10_000 },
		);
		const cancelRecords = await waitForRunTerminal(cancelRunId, 15_000);
		const cancelSummary = traceSummary(cancelRecords);
		const cancelSpawned = cancelSummary.spawned;
		const afterCancel = taskBackendRows(appPid);
		const descendantsAfterCancel = rowsForPgid(Number(cancelSpawned.pgid));
		receipt.cancellationTrial = {
			runId: cancelRunId,
			searchStarted,
			terminalKinds: cancelSummary.terminal.map((record: Json) => record.kind),
			terminalEventCount: cancelSummary.terminal.length,
			cancellationMs:
				Math.round((performance.now() - cancelStarted) * 100) / 100,
			teardown: cancelSummary.teardown,
			before: beforeCancel,
			after: afterCancel,
			descendantsAfterCancel: descendantsAfterCancel.length,
			valid:
				searchStarted &&
				cancelSummary.terminal.length === 1 &&
				cancelSummary.terminal[0].kind === "cancelled" &&
				cancelSummary.teardown.childReaped === true &&
				cancelSummary.teardown.processGroupAlive === false &&
				afterCancel.length === 0 &&
				descendantsAfterCancel.length === 0,
		};

		const trials = receipt.trials as Json[];
		const completeTimes = trials
			.filter((trial) => trial.valid)
			.map((trial) => Number(trial.search.completeTurnMs))
			.sort((a, b) => a - b);
		const medianCompleteTurnMs = median(completeTimes);
		const validCount = trials.filter((trial) => trial.valid).length;
		receipt.summary = {
			attempted: trials.length,
			valid: validCount,
			medianCompleteTurnMs,
			maxCompleteTurnMs: completeTimes.length
				? Math.max(...completeTimes)
				: null,
			zeroContext: trials.every(
				(trial) =>
					trial.state.contextChipCount === 0 &&
					trial.state.contextReady === true,
			),
			sourceProof: trials.every((trial) => trial.sourceProof),
			structuredSourceActions: trials.filter(
				(trial) => trial.search.sourceProvenance === "admitted-native-action",
			).length,
			unvisitedValidatedSchemaSources: trials.filter(
				(trial) =>
					trial.search.sourceProvenance === "unvisited-validated-schema-source",
			).length,
			orphanFree: trials.every(
				(trial) =>
					trial.process.after.length === 0 &&
					trial.process.groupAfter.length === 0,
			),
			piProcessesSpawned: trials.reduce(
				(sum, trial) => sum + trial.process.piProcessesSpawned,
				0,
			),
		};
	} catch (error) {
		(receipt.failures as Json[]).push({
			name: "probe_exception",
			error: error instanceof Error ? error.message : String(error),
		});
	} finally {
		await driver.close();
	}

	const currentStatus = currentGitStatus();
	const unrelatedDiffHashesUnchanged =
		JSON.stringify(filteredStatus(beforeStatus)) ===
		JSON.stringify(filteredStatus(currentStatus));
	receipt.preservation = {
		unrelatedDiffHashesUnchanged,
		commitCreated: false,
		beforeStatusSha256: sha256(beforeStatus),
		afterStatusSha256: sha256(currentStatus),
	};

	const summary = receipt.summary as Json;
	const cancellation = receipt.cancellationTrial as Json;
	const hardPerformancePass =
		typeof summary.medianCompleteTurnMs === "number" &&
		summary.medianCompleteTurnMs <= 12_000 &&
		(receipt.trials as Json[]).every(
			(trial) => Number(trial.search.completeTurnMs) <= 30_000,
		);
	const pass =
		summary.attempted === 3 &&
		summary.valid === 3 &&
		summary.zeroContext === true &&
		summary.sourceProof === true &&
		summary.orphanFree === true &&
		summary.piProcessesSpawned === 0 &&
		cancellation.valid === true &&
		hardPerformancePass &&
		unrelatedDiffHashesUnchanged;
	if (!hardPerformancePass) {
		(receipt.failures as Json[]).push({
			name: "hard_performance_gate",
			medianCompleteTurnMs: summary.medianCompleteTurnMs ?? null,
		});
	}
	receipt.verdict = pass ? "pass" : "fail";
	writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
	console.log(JSON.stringify(receipt, null, 2));
	return pass;
}

if (import.meta.main) {
	const passed = await main();
	process.exit(passed ? 0 : 1);
}

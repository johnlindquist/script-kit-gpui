#!/usr/bin/env bun
/**
 * NN=29 AI feature shell chaos probe.
 *
 * Safety contract:
 * - Every row launches a fresh scratch HOME (`sandboxHome: true`).
 * - Profile-shell rows never seed auth, submit an AI prompt, or execute a script.
 * - Fixtures are written only below the driver's scratch HOME.
 *
 * Run one row at a time:
 *   bun scripts/agentic/chaos-ai-feature-shell-probe.ts a1-valid-current
 */
import {
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	realpathSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Driver } from "../devtools/driver.ts";

const LANE = process.env.NN29_LANE ?? "finder-ai-2";
const BINARY =
	process.env.SCRIPT_KIT_GPUI_BINARY ??
	`target-agent/artifacts/${LANE}/script-kit-gpui`;
const ROW = process.argv[2] ?? "a1-valid-current";
const PROBE_STARTED_AT = performance.now();
const PROBE_STARTED_WALL_MS = Date.now();
const PROBE_DIR = dirname(fileURLToPath(import.meta.url));
const FLOW_FIXTURE = resolve(PROBE_DIR, "fixtures/flow-ux-project");
const FLOW_PACKAGE_FIXTURE = resolve(PROBE_DIR, "fixtures/flow-desk-package");
const RECEIPT_DIR = resolve(
	process.env.NN29_RECEIPT_DIR ?? ".test-output/chaos-29-finder-ai-2/local",
);
const IMPLEMENTED_ROWS = new Set([
	"a1-valid-current",
	"a2-malformed",
	"a3-outdated-remnants",
	"a4-hostile-valid",
	"a5-create",
	"a6-deleted-selected-heals",
	"c3-app-positional-turn",
	"r89-profile-switch-mid-chat",
	"r89-generate-script-handoff",
]);

const rowIsImplemented = IMPLEMENTED_ROWS.has(ROW);

const receipt: Record<string, unknown> = {
	probe: "chaos-ai-feature-shell",
	battery: "NN=29",
	row: ROW,
	binary: BINARY,
	safety: {
		sandboxHome: true,
		seedAgentAuth: false,
		aiPromptSubmitted: false,
		scriptExecuted: false,
		showSent: ROW.startsWith("r89-"),
	},
};

let driver: Driver | undefined;
let exitCode = 1;
const stateSamples: Record<string, unknown>[] = [];
const timingMilestones: Record<string, unknown>[] = [];
console.error(`[driver] binary: ${BINARY} (pinned NN=29 ${LANE} artifact)`);

function markTiming(label: string, detail?: unknown): void {
	timingMilestones.push({
		label,
		probeStartedWallTime: new Date(PROBE_STARTED_WALL_MS).toISOString(),
		wallTime: new Date().toISOString(),
		elapsedMs: Math.round(performance.now() - PROBE_STARTED_AT),
		detail,
	});
}

async function recordStateSample(
	activeDriver: Driver,
	label: string,
	detail?: unknown,
): Promise<void> {
	let appState: unknown;
	let agentChatState: unknown;
	try {
		appState = await activeDriver.getState({ timeoutMs: 5_000 });
	} catch (error) {
		appState = {
			error: error instanceof Error ? error.message : String(error),
		};
	}
	try {
		agentChatState = await getAgentChatState(activeDriver);
	} catch (error) {
		agentChatState = {
			error: error instanceof Error ? error.message : String(error),
		};
	}
	stateSamples.push({
		label,
		wallTime: new Date().toISOString(),
		elapsedMs: Math.round(performance.now() - PROBE_STARTED_AT),
		appState,
		agentChatState,
		detail,
	});
	markTiming(label);
}

function findSemanticElement(
	value: unknown,
	semanticId: string,
): Record<string, unknown> | null {
	if (Array.isArray(value)) {
		for (const item of value) {
			const found = findSemanticElement(item, semanticId);
			if (found) return found;
		}
		return null;
	}
	if (value && typeof value === "object") {
		const object = value as Record<string, unknown>;
		if (object.semanticId === semanticId || object.semantic_id === semanticId)
			return object;
		for (const child of Object.values(object)) {
			const found = findSemanticElement(child, semanticId);
			if (found) return found;
		}
	}
	return null;
}

async function waitForCurrentProfile(
	activeDriver: Driver,
	profileId: string,
	timeoutMs: number,
): Promise<{
	elements: unknown;
	current: Record<string, unknown> | null;
	waitedMs: number;
}> {
	const startedAt = performance.now();
	let elements: unknown = {};
	let current: Record<string, unknown> | null = null;
	while (performance.now() - startedAt < timeoutMs) {
		elements = await activeDriver.getElements(
			{ limit: 300 },
			{ timeoutMs: 5_000 },
		);
		current = findSemanticElement(elements, "status:profile-search-current");
		if (current?.value === profileId) {
			return {
				elements,
				current,
				waitedMs: Math.round(performance.now() - startedAt),
			};
		}
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return {
		elements,
		current,
		waitedMs: Math.round(performance.now() - startedAt),
	};
}

function countNeedle(value: unknown, needle: string): number {
	return JSON.stringify(value).split(needle).length - 1;
}

async function waitForLogNeedleCount(
	activeDriver: Driver,
	needle: string,
	minimumCount: number,
	timeoutMs: number,
): Promise<{ reached: boolean; count: number }> {
	const startedAt = performance.now();
	let count = 0;
	while (performance.now() - startedAt < timeoutMs) {
		const logs = await activeDriver.getLogs({ limit: 2_000 });
		count = countNeedle(logs, needle);
		if (count >= minimumCount) return { reached: true, count };
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return { reached: false, count };
}

async function waitForLogNeedle(
	activeDriver: Driver,
	needle: string,
	timeoutMs: number,
): Promise<boolean> {
	const startedAt = performance.now();
	while (performance.now() - startedAt < timeoutMs) {
		const logs = await activeDriver.getLogs({ limit: 500 });
		if (JSON.stringify(logs).includes(needle)) return true;
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return false;
}

async function getAgentChatState(
	activeDriver: Driver,
): Promise<Record<string, unknown>> {
	const result = (await activeDriver.request(
		{ type: "getAgentChatState", target: { type: "main" } },
		{ timeoutMs: 10_000 },
	)) as Record<string, unknown>;
	return (result.state ?? result) as Record<string, unknown>;
}

async function waitForAgentChatState(
	activeDriver: Driver,
	predicate: (state: Record<string, unknown>) => boolean,
	timeoutMs: number,
): Promise<Record<string, unknown>> {
	const startedAt = performance.now();
	let state: Record<string, unknown> = {};
	while (performance.now() - startedAt < timeoutMs) {
		try {
			state = await getAgentChatState(activeDriver);
		} catch (error) {
			state = {
				error: error instanceof Error ? error.message : String(error),
			};
		}
		if (predicate(state)) return state;
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return state;
}

async function waitForAppState(
	activeDriver: Driver,
	predicate: (state: Record<string, unknown>) => boolean,
	timeoutMs: number,
): Promise<Record<string, unknown>> {
	const startedAt = performance.now();
	let state: Record<string, unknown> = {};
	while (performance.now() - startedAt < timeoutMs) {
		state = (await activeDriver.getState({ timeoutMs: 5_000 })) as Record<
			string,
			unknown
		>;
		if (predicate(state)) return state;
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return state;
}

async function diagnosticCall<T>(
	label: string,
	call: () => Promise<T>,
): Promise<T | { error: string; label: string }> {
	try {
		return await call();
	} catch (error) {
		return {
			label,
			error: error instanceof Error ? error.message : String(error),
		};
	}
}

async function waitForMainWindowFocused(
	activeDriver: Driver,
	timeoutMs: number,
): Promise<{ focused: boolean; windows: unknown }> {
	const startedAt = performance.now();
	let windows: unknown = {};
	while (performance.now() - startedAt < timeoutMs) {
		windows = await activeDriver.listAutomationWindows({ timeoutMs: 5_000 });
		const list = (windows as { windows?: unknown }).windows;
		const main = Array.isArray(list)
			? (list.find(
					(window) =>
						(window as { id?: unknown }).id === "main" &&
						(window as { visible?: unknown }).visible === true,
				) as { focused?: unknown } | undefined)
			: undefined;
		if (main?.focused === true) return { focused: true, windows };
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return { focused: false, windows };
}

async function waitForElementNeedle(
	activeDriver: Driver,
	needle: string,
	timeoutMs: number,
): Promise<{ elements: unknown; waitedMs: number }> {
	const startedAt = performance.now();
	let elements: unknown = {};
	while (performance.now() - startedAt < timeoutMs) {
		elements = await activeDriver.getElements(
			{ limit: 300 },
			{ timeoutMs: 5_000 },
		);
		if (JSON.stringify(elements).includes(needle)) {
			return { elements, waitedMs: Math.round(performance.now() - startedAt) };
		}
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return { elements, waitedMs: Math.round(performance.now() - startedAt) };
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
	return value && typeof value === "object"
		? (value as Record<string, unknown>)
		: undefined;
}

function normalizedFailedChecks(value: unknown): string[] {
	return Array.isArray(value)
		? value.filter((item): item is string => typeof item === "string")
		: [];
}

function appendFailedCheck(name: string): void {
	const failedChecks = normalizedFailedChecks(receipt.failedChecks);
	if (!failedChecks.includes(name)) failedChecks.push(name);
	receipt.failedChecks = failedChecks;
}

function finalVerdict(): string {
	if (receipt.verdict === "ABORTED" || typeof receipt.error === "string") {
		return "ABORTED";
	}
	if (receipt.pass === true) return "PASS";
	return typeof receipt.classification === "string"
		? receipt.classification
		: "FAILED";
}

function recordedRowVerdicts(): Record<string, unknown>[] {
	const observed = asRecord(receipt.observed);
	const cells = Array.isArray(observed?.cells) ? observed.cells : [];
	if (cells.length > 0) {
		return cells.map((value, index) => {
			const cell = asRecord(value) ?? {};
			const checks = asRecord(cell.checks) ?? {};
			const failedChecks = Object.entries(checks).flatMap(([name, passed]) =>
				passed === false ? [name] : [],
			);
			let verdict = "RECORDED";
			if (typeof cell.decision === "string") {
				verdict = cell.decision;
			} else if (Object.keys(checks).length > 0 && failedChecks.length === 0) {
				verdict = "PASS";
			}
			return {
				id: typeof cell.id === "string" ? cell.id : `cell-${index}`,
				verdict,
				checks,
				failedChecks,
			};
		});
	}

	return [
		{
			id: ROW,
			verdict: finalVerdict(),
			checks: asRecord(observed?.checks) ?? {},
			failedChecks: normalizedFailedChecks(receipt.failedChecks),
		},
	];
}

function persistOutcomeArtifacts(): void {
	mkdirSync(RECEIPT_DIR, { recursive: true });
	const writtenAt = new Date().toISOString();
	const failedChecks = normalizedFailedChecks(receipt.failedChecks);
	const rowVerdicts = recordedRowVerdicts();
	receipt.failedChecks = failedChecks;
	receipt.rowVerdicts = rowVerdicts;
	receipt.writtenAt = writtenAt;
	const adjudication = {
		schemaVersion: 1,
		probe: receipt.probe,
		battery: receipt.battery,
		row: receipt.row,
		verdict: finalVerdict(),
		classification: receipt.classification ?? null,
		pass: receipt.pass === true,
		failedChecks,
		thrownError: typeof receipt.error === "string" ? receipt.error : null,
		rowVerdicts,
		safety: receipt.safety,
		cleanup: receipt.cleanup ?? null,
		finalization: receipt.finalization ?? null,
		writtenAt,
	};
	writeFileSync(
		resolve(RECEIPT_DIR, "receipt.json"),
		`${JSON.stringify(receipt, null, 2)}\n`,
	);
	writeFileSync(
		resolve(RECEIPT_DIR, "adjudication.json"),
		`${JSON.stringify(adjudication, null, 2)}\n`,
	);
}

try {
	if (!rowIsImplemented) {
		exitCode = 2;
		throw new Error(`Unknown or not-yet-implemented row: ${ROW}`);
	}
	markTiming("launch-start");
	driver = await Driver.launch({
		sessionName: `${LANE}-${ROW}`,
		binary: BINARY,
		sandboxHome: true,
		seedAgentAuth: false,
		env:
			ROW === "c3-app-positional-turn"
				? {
						SCRIPT_KIT_FLOW_UX_CWD: FLOW_FIXTURE,
						SCRIPT_KIT_FLOWS_PACKAGE_DIR: FLOW_PACKAGE_FIXTURE,
						SCRIPT_KIT_FLOWS_BIN_DIR: resolve(FLOW_PACKAGE_FIXTURE, "bin"),
						SCRIPT_KIT_CODEX_BIN: resolve(
							FLOW_PACKAGE_FIXTURE,
							"bin/fake-codex",
						),
						PATH: `${resolve(FLOW_FIXTURE, "bin")}:${resolve(
							FLOW_PACKAGE_FIXTURE,
							"bin",
						)}:${process.env.PATH ?? ""}`,
					}
				: ROW.startsWith("r89-")
					? {
							EDITOR: "/bin/echo",
							SCRIPT_KIT_PI_BINARY: resolve(PROBE_DIR, "mock-pi-rpc.js"),
							SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
						}
					: { EDITOR: "/bin/echo" },
	});
	markTiming("launch-ready", { pid: driver.pid ?? null });
	await driver.waitForSettle();
	await recordStateSample(driver, "initial-settled");

	if (ROW === "c3-app-positional-turn") {
		let sawVisible = false;
		let lastStateSample = "";
		const stateSamples: Record<string, unknown>[] = [];
		const flowUx = (state: Record<string, unknown>) =>
			(state.flowUx as Record<string, unknown> | undefined) ?? {};
		const lastSession = (state: Record<string, unknown>) => {
			const sessions = flowUx(state).sessions;
			return Array.isArray(sessions)
				? (sessions.at(-1) as Record<string, unknown> | undefined)
				: undefined;
		};
		const pollFlowState = async (
			predicate: (state: Record<string, unknown>) => boolean,
			timeoutMs: number,
		) => {
			const startedAt = performance.now();
			let state: Record<string, unknown> = {};
			while (performance.now() - startedAt < timeoutMs) {
				state = (await driver?.getState({ timeoutMs: 5_000 })) as Record<
					string,
					unknown
				>;
				sawVisible ||= state.windowVisible === true;
				const session = lastSession(state);
				const sample = {
					promptType: state.promptType,
					windowVisible: state.windowVisible,
					selectedFlowId: flowUx(state).selectedFlowId,
					session: session
						? {
								flowId: session.flowId,
								state: session.state,
								turns: session.turns,
								turnInFlight: session.turnInFlight,
								transport: session.transport,
							}
						: undefined,
				};
				const sampleSignature = JSON.stringify(sample);
				if (sampleSignature !== lastStateSample) {
					stateSamples.push(sample);
					lastStateSample = sampleSignature;
				}
				if (predicate(state)) return state;
				await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
			}
			return state;
		};

		driver.send({ type: "triggerBuiltin", builtinId: "builtin/flows" });
		const deskState = await pollFlowState(
			(state) =>
				(flowUx(state).roster as { status?: unknown } | undefined)?.status ===
				"ready",
			10_000,
		);
		await driver.setFilterAndWait("nn29-positional-turn", {
			timeoutMs: 10_000,
		});
		await pollFlowState(
			(state) =>
				flowUx(state).selectedFlowId === "project:nn29-positional-turn.fasteng",
			10_000,
		);
		driver.simulateKey("enter");
		const openedState = await pollFlowState(
			(state) =>
				state.promptType === "flowSession" &&
				lastSession(state)?.flowId === "project:nn29-positional-turn.fasteng",
			10_000,
		);
		await driver.request(
			{
				type: "batch",
				commands: [{ type: "setInput", text: "NN29 app runtime turn" }],
				options: {
					stopOnError: true,
					rollbackOnError: false,
					timeout: 5_000,
				},
			},
			{ expect: "batchResult", timeoutMs: 8_000 },
		);
		await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "enter", modifiers: [] },
			{ target: { type: "main" }, timeoutMs: 5_000 },
		);
		const completedState = await pollFlowState((state) => {
			const session = lastSession(state);
			return session?.turns === 1 || session?.state === "error";
		}, 20_000);
		const session = lastSession(completedState);
		const elements = await driver.getElements(
			{ limit: 400 },
			{ timeoutMs: 8_000 },
		);
		const logs = await driver.getLogs({ limit: 500 });
		const elementsBlob = JSON.stringify(elements);
		const logsBlob = JSON.stringify(logs);
		const checks: Record<string, boolean> = {
			deskRosterReady:
				(flowUx(deskState).roster as { status?: unknown } | undefined)
					?.status === "ready",
			positionalFlowSelected:
				lastSession(openedState)?.flowId ===
				"project:nn29-positional-turn.fasteng",
			flowSessionOpened: openedState.promptType === "flowSession",
			mdflowTransport: session?.transport === "mdflowTurns",
			turnCompleted: session?.turns === 1 && session?.state === "needs you",
			engineReplyVisible: elementsBlob.includes("FASTENG_OK"),
			taskReachedEngine: elementsBlob.includes("NN29 app runtime turn"),
			noLegacyTaskShapeFailure:
				!logsBlob.includes("UNUSED_VARIABLE_FLAG") &&
				!logsBlob.includes("Missing template variables: _1"),
			hiddenThroughout: !sawVisible,
			appAlive: driver.alive,
		};
		receipt.sessionDir = driver.sessionDir;
		receipt.scratchHome = resolve(driver.sessionDir, "home");
		receipt.observed = {
			checks,
			session,
			stateSamples,
			legacyFailureInLogs:
				logsBlob.includes("UNUSED_VARIABLE_FLAG") ||
				logsBlob.includes("Missing template variables: _1"),
		};
		receipt.pass = Object.values(checks).every(Boolean);
		receipt.failedChecks = Object.entries(checks)
			.filter(([, passed]) => !passed)
			.map(([name]) => name);
		exitCode = receipt.pass ? 0 : 1;
	} else if (ROW === "r89-profile-switch-mid-chat") {
		const scratchHome = resolve(driver.sessionDir, "home");
		const profilesDir = resolve(scratchHome, ".scriptkit", "profiles");
		mkdirSync(profilesDir, { recursive: true });
		const profilePath = resolve(profilesDir, "nn29-switch-target.md");
		writeFileSync(
			profilePath,
			[
				"---",
				"name: NN29 Switch Target",
				"model: openai-codex/gpt-5.4",
				"no-session: true",
				"---",
				"",
				"Provider-free profile-switch fixture. Never submit a turn.",
				"",
			].join("\n"),
		);
		const profileRealPath = realpathSync(profilePath);
		if (!profileRealPath.startsWith(`${realpathSync(scratchHome)}/`)) {
			throw new Error(
				`profile fixture escaped scratch HOME: ${profileRealPath}`,
			);
		}

		driver.send({ type: "openAiWithMockData" });
		await waitForAgentChatState(
			driver,
			(state) => typeof state.status === "string" && state.status !== "setup",
			15_000,
		);
		await driver.request(
			{
				type: "setAgentChatTestFixture",
				phase: "idle",
				userText: "NN29 deterministic context survives profile switching.",
				assistantText: "NN29 fixture reply; no provider was called.",
			},
			{ timeoutMs: 15_000 },
		);
		const draft = "NN29 exact draft survives profile switch";
		await driver.request(
			{ type: "setAgentChatInput", text: draft },
			{ timeoutMs: 10_000 },
		);
		const before = await waitForAgentChatState(
			driver,
			(state) =>
				state.status === "idle" &&
				state.messageCount === 2 &&
				state.inputText === draft,
			15_000,
		);

		const showReceipt = await driver.request(
			{ type: "show" },
			{ timeoutMs: 8_000 },
		);
		const mainFocus = await waitForMainWindowFocused(driver, 8_000);
		const shown = mainFocus.focused;
		const shiftTabDispatch = await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "tab", modifiers: ["shift"] },
			{ target: { type: "focused" }, timeoutMs: 5_000 },
		);
		const pickerOpened = await waitForAgentChatState(
			driver,
			(state) => {
				const spine = state.spine as
					| { ownsList?: unknown; activeSegmentKind?: unknown }
					| undefined;
				return (
					spine?.ownsList === true && spine.activeSegmentKind === "profile"
				);
			},
			10_000,
		);
		for (const character of "nn29-switch-target") {
			driver.simulateKey(character);
		}
		const pickerFiltered = await waitForAgentChatState(
			driver,
			(state) => {
				const spine = state.spine as
					| {
							ownsList?: unknown;
							activeSegmentKind?: unknown;
							rowCount?: unknown;
							selectableRowCount?: unknown;
					  }
					| undefined;
				return (
					spine?.ownsList === true &&
					spine.activeSegmentKind === "profile" &&
					spine.rowCount === 1 &&
					spine.selectableRowCount === 1
				);
			},
			10_000,
		);
		mkdirSync(RECEIPT_DIR, { recursive: true });
		const pickerScreenshotPath = resolve(
			RECEIPT_DIR,
			"profile-switch-picker.png",
		);
		const pickerScreenshot = await driver.captureScreenshot({
			target: { type: "main" },
			savePath: pickerScreenshotPath,
			timeoutMs: 15_000,
		});
		const profileRowClick = await driver.simulateGpuiClick(100, 110, {
			target: { type: "main" },
			timeoutMs: 5_000,
		});
		const persisted = await waitForLogNeedle(
			driver,
			"agent_chat_profile_persisted",
			10_000,
		);
		const selected = await waitForLogNeedle(
			driver,
			"agent_chat_profile_selector_selected",
			10_000,
		);
		const after = await waitForAgentChatState(
			driver,
			(state) =>
				(state.spine as { ownsList?: unknown } | undefined)?.ownsList !==
					true &&
				state.messageCount === 2 &&
				(state.lastAcceptedItem as { id?: unknown } | undefined)?.id ===
					"agent-chat-profile:nn29-switch-target",
			15_000,
		);
		const screenshotPath = resolve(RECEIPT_DIR, "profile-switch-after.png");
		const screenshot = await driver.captureScreenshot({
			target: { type: "main" },
			savePath: screenshotPath,
			timeoutMs: 15_000,
		});
		const elements = await driver.getElements(
			{ target: { type: "main" }, limit: 400 },
			{ timeoutMs: 10_000 },
		);
		const logs = await driver.getLogs({ limit: 500 });
		const logsBlob = JSON.stringify(logs);
		const instrumentChecks: Record<string, boolean> = {
			fixtureContextReady:
				before.status === "idle" && before.messageCount === 2,
			draftReady: before.inputText === draft,
			windowRevealed: shown,
			pickerOpened:
				(pickerOpened.spine as { ownsList?: unknown } | undefined)?.ownsList ===
				true,
			targetProfileFiltered:
				(
					pickerFiltered.spine as
						| {
								rowCount?: unknown;
								selectableRowCount?: unknown;
						  }
						| undefined
				)?.rowCount === 1 &&
				(pickerFiltered.spine as { selectableRowCount?: unknown } | undefined)
					?.selectableRowCount === 1 &&
				pickerScreenshot.error == null,
			profileAcceptedStructured:
				(after.lastAcceptedItem as { id?: unknown } | undefined)?.id ===
				"agent-chat-profile:nn29-switch-target",
			profileSelectionLogged: selected,
			profilePersistenceLogged:
				persisted && logsBlob.includes("nn29-switch-target"),
			pickerClosed:
				(after.spine as { ownsList?: unknown } | undefined)?.ownsList !== true,
			messageContextSurvived: after.messageCount === before.messageCount,
			contextChipsSurvived: after.contextChipCount === before.contextChipCount,
			uiAlive: driver.alive && screenshot.error == null,
			noTurnSubmitted: after.messageCount === 2 && after.status === "idle",
		};
		const productChecks: Record<string, boolean> = {
			draftSurvived: after.inputText === draft,
		};
		const checks = { ...instrumentChecks, ...productChecks };
		receipt.sessionDir = driver.sessionDir;
		receipt.scratchHome = scratchHome;
		receipt.seededProfiles = [profileRealPath];
		receipt.observed = {
			checks,
			before,
			showReceipt,
			mainFocus,
			shiftTabDispatch,
			pickerOpened: pickerOpened.spine,
			pickerFiltered: pickerFiltered.spine,
			pickerScreenshot: {
				path: pickerScreenshotPath,
				error: pickerScreenshot.error ?? null,
			},
			profileRowClick,
			after,
			screenshot: { path: screenshotPath, error: screenshot.error ?? null },
			elementHasTargetProfile:
				JSON.stringify(elements).includes("NN29 Switch Target"),
		};
		const instrumentValid = Object.values(instrumentChecks).every(Boolean);
		receipt.pass =
			instrumentValid && Object.values(productChecks).every(Boolean);
		receipt.classification = instrumentValid
			? receipt.pass
				? "verified"
				: "failed-product"
			: "invalid-harness";
		receipt.failedChecks = Object.entries(checks)
			.filter(([, passed]) => !passed)
			.map(([name]) => name);
		exitCode = receipt.pass ? 0 : instrumentValid ? 1 : 2;
	} else if (ROW === "r89-generate-script-handoff") {
		const scratchHome = resolve(driver.sessionDir, "home");
		const scriptDir = resolve(
			scratchHome,
			".scriptkit",
			"plugins",
			"main",
			"scripts",
		);
		const executionCanary = resolve(scratchHome, "NN29_GENERATED_EXECUTED");
		const cells: Record<string, unknown>[] = [];
		(receipt.safety as Record<string, unknown>).fakePiRpc = true;

		const showReceipt = await driver.request(
			{ type: "show" },
			{ timeoutMs: 8_000 },
		);
		const launcherFocus = await waitForMainWindowFocused(driver, 8_000);
		await recordStateSample(driver, "row2-launcher-ready");
		await driver.setFilterAndWait("Generate Script with Agent Chat", {
			timeoutMs: 10_000,
		});
		const emptyLauncherElements = await driver.getElements(
			{ target: { type: "main" }, limit: 400 },
			{ timeoutMs: 10_000 },
		);
		const emptyVisible = JSON.stringify(emptyLauncherElements).includes(
			"Generate Script with Agent Chat",
		);
		const routeLogsBeforeEmpty = await driver.getLogs({ limit: 2_000 });
		const routeCountBeforeEmpty = countNeedle(
			routeLogsBeforeEmpty,
			"ai_generate_script_routed_to_harness",
		);
		driver.simulateKey("enter");
		const emptyRoute = await waitForLogNeedleCount(
			driver,
			"ai_generate_script_routed_to_harness",
			routeCountBeforeEmpty + 1,
			10_000,
		);
		const emptyEntry = await waitForAgentChatState(
			driver,
			(state) => typeof state.status === "string" && state.status !== "setup",
			20_000,
		);
		const emptyFocus = await waitForMainWindowFocused(driver, 8_000);
		await recordStateSample(driver, "row2-empty-entry", { emptyRoute });
		const emptyChecks = {
			realBuiltinVisible: emptyVisible,
			routed: emptyRoute.reached,
			agentChatMain:
				(emptyEntry.resolvedTarget as { windowId?: unknown } | undefined)
					?.windowId === "main",
			revealed: emptyFocus.focused,
			entryContextStaged:
				emptyEntry.inputText === '@cmd:"Generate Script with Agent Chat" ' &&
				emptyEntry.messageCount === 0 &&
				emptyEntry.contextChipCount === 2 &&
				String(emptyEntry.contextSummary ?? "").includes("Note Instructions") &&
				String(emptyEntry.contextSummary ?? "").includes(
					"Command: Generate Script with Agent Chat",
				),
		};
		cells.push({
			id: "empty-query",
			checks: emptyChecks,
			entryState: emptyEntry,
		});

		const canaryLiteral = JSON.stringify(executionCanary);
		await driver.request(
			{
				type: "setAgentChatTestFixture",
				phase: "idle",
				userText: "Generate an inert NN29 Script Kit fixture.",
				assistantText: [
					"```typescript",
					`await Bun.write(${canaryLiteral}, "MUST_NOT_RUN");`,
					'await div("NN29 inert generated output");',
					"```",
				].join("\n"),
			},
			{ timeoutMs: 15_000 },
		);
		const fixtureState = await waitForAgentChatState(
			driver,
			(state) => state.status === "idle" && state.messageCount === 2,
			15_000,
		);
		await recordStateSample(driver, "row2-empty-inert-fixture");
		mkdirSync(RECEIPT_DIR, { recursive: true });
		const emptyShotPath = resolve(RECEIPT_DIR, "empty-query-handoff.png");
		const emptyShot = await driver.captureScreenshot({
			target: { type: "main" },
			savePath: emptyShotPath,
			timeoutMs: 15_000,
		});

		driver.simulateKey("escape");
		const returnedState = await waitForAppState(
			driver,
			(state) =>
				(state.surfaceContract as { surfaceKind?: unknown } | undefined)
					?.surfaceKind === "ScriptList",
			12_000,
		);
		await driver.request({ type: "show" }, { timeoutMs: 8_000 });
		const returnedFocus = await waitForMainWindowFocused(driver, 8_000);
		await recordStateSample(driver, "row2-returned-to-launcher");

		const populatedQuery =
			process.env.NN29_POPULATED_QUERY ??
			"Generate Script with Agent Chat build an inert NN29 clock";
		await driver.setFilterAndWait(populatedQuery, { timeoutMs: 10_000 });
		const populatedLauncherElements = await driver.getElements(
			{ target: { type: "main" }, limit: 400 },
			{ timeoutMs: 10_000 },
		);
		const populatedVisible = JSON.stringify(populatedLauncherElements).includes(
			"Generate Script with Agent Chat",
		);
		const populatedPreEnterState = (await driver.getState({
			timeoutMs: 8_000,
		})) as Record<string, unknown>;
		await recordStateSample(driver, "row2-populated-before-enter");
		const routeLogsBeforePopulated = await driver.getLogs({ limit: 2_000 });
		const routeCountBeforePopulated = countNeedle(
			routeLogsBeforePopulated,
			"ai_generate_script_routed_to_harness",
		);
		const firstEnterDispatch = await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "enter", modifiers: [] },
			{ target: { type: "focused" }, timeoutMs: 5_000 },
		);
		let populatedRoute = await waitForLogNeedleCount(
			driver,
			"ai_generate_script_routed_to_harness",
			routeCountBeforePopulated + 1,
			3_000,
		);
		let secondEnterSent = false;
		if (!populatedRoute.reached) {
			secondEnterSent = true;
			driver.simulateKey("enter");
			populatedRoute = await waitForLogNeedleCount(
				driver,
				"ai_generate_script_routed_to_harness",
				routeCountBeforePopulated + 1,
				8_000,
			);
		}
		(receipt.safety as Record<string, unknown>).aiPromptSubmitted =
			populatedRoute.reached;
		const populatedAppState = populatedRoute.reached
			? await waitForAppState(
					driver,
					(state) => {
						const kind = (
							state.surfaceContract as { surfaceKind?: unknown } | undefined
						)?.surfaceKind;
						return kind === "AgentChat" || kind === "QuickTerminal";
					},
					12_000,
				)
			: ((await driver.getState({ timeoutMs: 8_000 })) as Record<
					string,
					unknown
				>);
		const populatedKind = (
			populatedAppState.surfaceContract as { surfaceKind?: unknown } | undefined
		)?.surfaceKind;
		const populatedEntry =
			populatedKind === "AgentChat"
				? await waitForAgentChatState(
						driver,
						(state) =>
							typeof state.status === "string" && state.status !== "setup",
						8_000,
					)
				: {
						status: "notAgentChat",
						inputText: "",
						messageCount: 0,
						contextChipCount: 0,
					};
		const populatedFocus = await waitForMainWindowFocused(driver, 8_000);
		await recordStateSample(driver, "row2-populated-entry", { populatedRoute });
		const populatedSelectedKey = (
			populatedPreEnterState.mainWindowPreflight as
				| { selectedResultKey?: unknown }
				| undefined
		)?.selectedResultKey;
		const routeDelta = populatedRoute.count - routeCountBeforePopulated;
		const populatedChecks = {
			realBuiltinVisible: populatedVisible,
			selectedBuiltinBeforeEnter:
				populatedSelectedKey === "builtin/generate-script-with-ai",
			routedExactlyOnce: populatedRoute.reached && routeDelta === 1,
			returnedFromEmpty:
				(returnedState.surfaceContract as { surfaceKind?: unknown } | undefined)
					?.surfaceKind === "ScriptList" && returnedFocus.focused,
			agentChatEntry: populatedKind === "AgentChat",
			entryContextCaptured:
				populatedEntry.inputText !== undefined &&
				populatedEntry.messageCount !== undefined &&
				populatedEntry.contextChipCount !== undefined,
			revealed: populatedFocus.focused,
		};
		cells.push({
			id: "populated-query",
			query: populatedQuery,
			checks: populatedChecks,
			preEnterState: populatedPreEnterState,
			firstEnterDispatch,
			secondEnterSent,
			routeDelta,
			appState: populatedAppState,
			entryState: populatedEntry,
		});
		await driver.request({ type: "show" }, { timeoutMs: 8_000 });
		await waitForMainWindowFocused(driver, 8_000);
		const populatedShotPath = resolve(
			RECEIPT_DIR,
			"populated-query-handoff.png",
		);
		const populatedShot = await driver.captureScreenshot({
			target: { type: "main" },
			savePath: populatedShotPath,
			timeoutMs: 15_000,
		});

		// Round 90 rule-12d: adjudicate W-PROFILE-ACCEPT in this paid launch.
		// Earlier native Enter/Tab/click attempts never emitted an acceptance.
		// This cell uses the app's acknowledged focused simulateGpuiEvent path
		// exactly once for Enter; accepted => prior instrument limitation,
		// delivered-but-not-accepted => product finding.
		const profilesDir = resolve(scratchHome, ".scriptkit", "profiles");
		mkdirSync(profilesDir, { recursive: true });
		const profilePath = resolve(profilesDir, "nn29-switch-target.md");
		writeFileSync(
			profilePath,
			[
				"---",
				"name: NN29 Switch Target",
				"model: openai-codex/gpt-5.4",
				"no-session: true",
				"---",
				"",
				"Provider-free profile acceptance fixture. Never submit a turn.",
				"",
			].join("\n"),
		);
		const profileRealPath = realpathSync(profilePath);
		if (!profileRealPath.startsWith(`${realpathSync(scratchHome)}/`)) {
			throw new Error(
				`profile fixture escaped scratch HOME: ${profileRealPath}`,
			);
		}

		driver.send({ type: "openAiWithMockData" });
		await waitForAgentChatState(
			driver,
			(state) => typeof state.status === "string" && state.status !== "setup",
			15_000,
		);
		await driver.request(
			{
				type: "setAgentChatTestFixture",
				phase: "idle",
				userText: "NN29 profile acceptance decision fixture.",
				assistantText: "NN29 fixture reply; no provider was called.",
			},
			{ timeoutMs: 15_000 },
		);
		await driver.request(
			{ type: "setAgentChatInput", text: "NN29 profile accept draft" },
			{ timeoutMs: 10_000 },
		);
		await driver.request({ type: "show" }, { timeoutMs: 8_000 });
		const profileFocus = await waitForMainWindowFocused(driver, 8_000);
		const profileOpenDispatch = await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "tab", modifiers: ["shift"] },
			{ target: { type: "focused" }, timeoutMs: 5_000 },
		);
		const profilePickerOpened = await waitForAgentChatState(
			driver,
			(state) => {
				const spine = state.spine as
					| { ownsList?: unknown; activeSegmentKind?: unknown }
					| undefined;
				return (
					spine?.ownsList === true && spine.activeSegmentKind === "profile"
				);
			},
			10_000,
		);
		for (const character of "nn29-switch-target") {
			driver.simulateKey(character);
		}
		const profilePickerFiltered = await waitForAgentChatState(
			driver,
			(state) => {
				const spine = state.spine as
					| {
							ownsList?: unknown;
							activeSegmentKind?: unknown;
							rowCount?: unknown;
							selectableRowCount?: unknown;
					  }
					| undefined;
				return (
					spine?.ownsList === true &&
					spine.activeSegmentKind === "profile" &&
					spine.rowCount === 1 &&
					spine.selectableRowCount === 1
				);
			},
			10_000,
		);
		await recordStateSample(driver, "w-profile-before-accept", {
			profileFocus,
			profileOpenDispatch,
		});
		const profilePickerShotPath = resolve(
			RECEIPT_DIR,
			"profile-accept-before-enter.png",
		);
		const profilePickerShot = await driver.captureScreenshot({
			target: { type: "main" },
			savePath: profilePickerShotPath,
			timeoutMs: 15_000,
		});
		const selectedBeforeAccept = countNeedle(
			await driver.getLogs({ limit: 1_000 }),
			"agent_chat_profile_selector_selected",
		);
		const profileAcceptDispatch = await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "enter", modifiers: [] },
			{ target: { type: "focused" }, timeoutMs: 5_000 },
		);
		const profileSelected = await waitForLogNeedleCount(
			driver,
			"agent_chat_profile_selector_selected",
			selectedBeforeAccept + 1,
			10_000,
		);
		const profilePersisted = await waitForLogNeedle(
			driver,
			"agent_chat_profile_persisted",
			10_000,
		);
		const profileAfterAccept = await waitForAgentChatState(
			driver,
			(state) =>
				(state.spine as { ownsList?: unknown } | undefined)?.ownsList !==
					true &&
				(state.lastAcceptedItem as { id?: unknown } | undefined)?.id ===
					"agent-chat-profile:nn29-switch-target",
			15_000,
		);
		await recordStateSample(driver, "w-profile-after-accept", {
			profileAcceptDispatch,
			profileSelected,
			profilePersisted,
		});
		const profileAfterShotPath = resolve(
			RECEIPT_DIR,
			"profile-accept-after-enter.png",
		);
		const profileAfterShot = await driver.captureScreenshot({
			target: { type: "main" },
			savePath: profileAfterShotPath,
			timeoutMs: 15_000,
		});
		const profileSpine = profilePickerFiltered.spine as
			| {
					ownsList?: unknown;
					activeSegmentKind?: unknown;
					rowCount?: unknown;
					selectableRowCount?: unknown;
			  }
			| undefined;
		const profileAcceptanceInstrumentValid =
			profileFocus.focused &&
			(profilePickerOpened.spine as { ownsList?: unknown } | undefined)
				?.ownsList === true &&
			profileSpine?.activeSegmentKind === "profile" &&
			profileSpine.rowCount === 1 &&
			profileSpine.selectableRowCount === 1 &&
			(profilePickerShot.error === undefined ||
				profilePickerShot.error === null);
		const profileAccepted =
			profileSelected &&
			profilePersisted &&
			(profileAfterAccept.lastAcceptedItem as { id?: unknown } | undefined)
				?.id === "agent-chat-profile:nn29-switch-target" &&
			(profileAfterAccept.spine as { ownsList?: unknown } | undefined)
				?.ownsList !== true &&
			(profileAfterShot.error === undefined || profileAfterShot.error === null);
		let profileAcceptanceDecision = "failed-product";
		if (!profileAcceptanceInstrumentValid) {
			profileAcceptanceDecision = "invalid-harness";
		} else if (profileAccepted) {
			profileAcceptanceDecision = "instrument-limitation";
		}
		const profileAcceptanceCell = {
			id: "w-profile-accept",
			decision: profileAcceptanceDecision,
			instrumentValid: profileAcceptanceInstrumentValid,
			accepted: profileAccepted,
			profileRealPath,
			profileOpenDispatch,
			pickerOpened: profilePickerOpened.spine,
			pickerFiltered: profilePickerFiltered.spine,
			profileAcceptDispatch,
			profileSelected,
			profilePersisted,
			after: profileAfterAccept,
			screenshots: [profilePickerShotPath, profileAfterShotPath],
		};
		cells.push(profileAcceptanceCell);

		const generatedFiles = existsSync(scriptDir) ? readdirSync(scriptDir) : [];
		const checks: Record<string, boolean> = {
			emptyCell: Object.values(emptyChecks).every(Boolean),
			populatedCell: Object.values(populatedChecks).every(Boolean),
			fixtureOutputVisible:
				fixtureState.status === "idle" && fixtureState.messageCount === 2,
			noGeneratedArtifact: generatedFiles.length === 0,
			executionCanaryAbsent: !existsSync(executionCanary),
			profileAcceptanceInstrumented: profileAcceptanceInstrumentValid,
			profileAcceptanceAccepted: profileAccepted,
			uiAlive:
				driver.alive && emptyShot.error == null && populatedShot.error == null,
		};
		receipt.sessionDir = driver.sessionDir;
		receipt.scratchHome = scratchHome;
		receipt.observed = {
			checks,
			cells,
			showReceipt,
			launcherFocus,
			fixtureState,
			generatedFiles,
			executionCanary,
			screenshots: [
				{ path: emptyShotPath, error: emptyShot.error ?? null },
				{ path: populatedShotPath, error: populatedShot.error ?? null },
			],
		};
		receipt.pass = Object.values(checks).every(Boolean);
		if (!profileAcceptanceInstrumentValid) {
			receipt.classification = "invalid-harness";
			exitCode = 2;
		} else if (receipt.pass) {
			receipt.classification = "verified";
			exitCode = 0;
		} else {
			receipt.classification = "failed-product";
			exitCode = 1;
		}
		receipt.failedChecks = Object.entries(checks)
			.filter(([, passed]) => !passed)
			.map(([name]) => name);
	} else {
		const scratchHome = resolve(driver.sessionDir, "home");
		const profilesDir = resolve(scratchHome, ".scriptkit", "profiles");
		mkdirSync(profilesDir, { recursive: true });
		const seededPaths: string[] = [];
		const seedProfile = (filename: string, lines: string[]): void => {
			const path = resolve(profilesDir, filename);
			mkdirSync(dirname(path), { recursive: true });
			writeFileSync(path, lines.join("\n"));
			const realPath = realpathSync(path);
			if (!realPath.startsWith(`${realpathSync(scratchHome)}/`)) {
				throw new Error(`fixture escaped scratch HOME: ${realPath}`);
			}
			seededPaths.push(realPath);
		};

		seedProfile("nn29-valid.md", [
			"---",
			"name: NN29 Valid Current",
			"provider: openai-codex",
			"model: gpt-5.4",
			"tools:",
			"  - web_search",
			"no-session: true",
			"---",
			"",
			"This is a deterministic profile-shell fixture. Do not run anything.",
			"",
		]);

		if (ROW === "a2-malformed") {
			seedProfile("nn29-bad-frontmatter.md", [
				"---",
				"name: [unterminated",
				"model: openai-codex/gpt-5.4",
				"---",
				"This malformed profile must not poison its valid sibling.",
			]);
			seedProfile("nn29-wrong-types.md", [
				"---",
				"name:",
				"  - not",
				"  - a scalar",
				"model:",
				"  nested: wrong",
				"tools:",
				"  - nested: wrong",
				"no-session: definitely-not-a-boolean",
				"---",
				"This wrong-typed profile must not silently become a different profile.",
			]);
		} else if (ROW === "a3-outdated-remnants") {
			seedProfile("profile.json", [
				'{"id":"plugin:legacy/direct","name":"NN29 Legacy Direct JSON","model":"stale-model"}',
			]);
			const legacyPath = resolve(
				scratchHome,
				".scriptkit",
				"plugins",
				"nn29-legacy-plugin",
				"profiles",
				"stale",
				"profile.json",
			);
			mkdirSync(dirname(legacyPath), { recursive: true });
			writeFileSync(
				legacyPath,
				'{"id":"plugin:nn29-legacy/stale","name":"NN29 Legacy Nested JSON","model":"stale-model"}',
			);
			const legacyRealPath = realpathSync(legacyPath);
			if (!legacyRealPath.startsWith(`${realpathSync(scratchHome)}/`)) {
				throw new Error(`fixture escaped scratch HOME: ${legacyRealPath}`);
			}
			seededPaths.push(legacyRealPath);
		} else if (ROW === "a4-hostile-valid") {
			seedProfile("nn29-huge.md", [
				"---",
				"name: NN29 Huge 1MiB",
				"model: openai-codex/gpt-5.4",
				"no-session: true",
				"---",
				"",
				"H".repeat(1024 * 1024),
			]);
			seedProfile("nn29-zalgo.md", [
				"---",
				"name: NN29 Zalgo Z̴̙̓͗a̷̻͒l̵͎͋g̶̯͗o̴̰̕ 👩🏽‍💻 مرحبا",
				"model: openai-codex/gpt-5.4",
				"no-session: true",
				"---",
				"",
				"Combining marks, emoji ZWJ, and RTL must remain inert profile text.",
			]);
		}

		driver.simulateKey("tab", ["shift"]);
		await driver.waitForSettle({ timeoutMs: 8_000 });

		const profileRowNeedle = "profile-search-row:nn29-valid";
		const [state, elementWait] = await Promise.all([
			driver.getState({ timeoutMs: 5_000 }),
			waitForElementNeedle(driver, profileRowNeedle, 5_000),
		]);
		const hostileEvidence: Record<string, unknown> = {};
		const createEvidence: Record<string, unknown> = {};
		const deletionEvidence: Record<string, unknown> = {};
		if (ROW === "a4-hostile-valid") {
			await driver.setFilterAndWait("nn29 huge", { timeoutMs: 8_000 });
			const hugeWait = await waitForElementNeedle(
				driver,
				"profile-search-row:nn29-huge",
				8_000,
			);
			hostileEvidence.huge = {
				visible: JSON.stringify(hugeWait.elements).includes(
					"profile-search-row:nn29-huge",
				),
				waitedMs: hugeWait.waitedMs,
			};
			await driver.setFilterAndWait("nn29 zalgo", { timeoutMs: 8_000 });
			const zalgoWait = await waitForElementNeedle(
				driver,
				"profile-search-row:nn29-zalgo",
				8_000,
			);
			hostileEvidence.zalgo = {
				visible: JSON.stringify(zalgoWait.elements).includes(
					"profile-search-row:nn29-zalgo",
				),
				waitedMs: zalgoWait.waitedMs,
			};
		} else if (ROW === "a5-create") {
			await driver.setFilterAndWait("create", { timeoutMs: 8_000 });
			const createWait = await waitForElementNeedle(
				driver,
				"profile-search-row:create-new-profile",
				8_000,
			);
			driver.simulateKey("enter");
			const createLogged = await waitForLogNeedle(
				driver,
				"profile_search_create_profile",
				8_000,
			);
			await driver.waitForSettle({ timeoutMs: 8_000 });
			const createdName = readdirSync(profilesDir).find(
				(name) => name.endsWith(".md") && name !== "nn29-valid.md",
			);
			const createdPath = createdName
				? resolve(profilesDir, createdName)
				: null;
			const createdBody = createdPath ? readFileSync(createdPath, "utf8") : "";
			createEvidence.rowVisible = JSON.stringify(createWait.elements).includes(
				"profile-search-row:create-new-profile",
			);
			createEvidence.createLogged = createLogged;
			createEvidence.createdPath = createdPath;
			createEvidence.templateValid =
				createdBody.includes("name: My Profile") &&
				createdBody.includes("no-session: true") &&
				createdBody.includes("You are a focused Agent Chat profile");
			createEvidence.returnedToScriptList = JSON.stringify(
				await driver.getState({ timeoutMs: 5_000 }),
			).includes('"promptType":"none"');
		} else if (ROW === "a6-deleted-selected-heals") {
			await driver.setFilterAndWait("NN29 Valid Current", { timeoutMs: 8_000 });
			const selectedWait = await waitForElementNeedle(
				driver,
				profileRowNeedle,
				8_000,
			);
			driver.simulateKey("enter");
			const persisted = await waitForLogNeedle(
				driver,
				"profile_search_profile_persisted",
				8_000,
			);
			await driver.waitForSettle({ timeoutMs: 8_000 });
			unlinkSync(resolve(profilesDir, "nn29-valid.md"));
			driver.simulateKey("tab", ["shift"]);
			await driver.waitForSettle({ timeoutMs: 8_000 });
			const healed = await waitForCurrentProfile(driver, "brain", 8_000);
			deletionEvidence.selectedRowWasVisible = JSON.stringify(
				selectedWait.elements,
			).includes(profileRowNeedle);
			deletionEvidence.persisted = persisted;
			deletionEvidence.current = healed.current;
			deletionEvidence.healWaitMs = healed.waitedMs;
			deletionEvidence.deletedRowAbsent = !JSON.stringify(
				healed.elements,
			).includes(profileRowNeedle);
		}
		const logs = await driver.getLogs({ limit: 500 });
		const elementsBlob = JSON.stringify(elementWait.elements);
		const logsBlob = JSON.stringify(logs);
		const checks: Record<string, boolean> = {
			profileSearchOpen: JSON.stringify(state).includes("ProfileSearch"),
			validRowVisible: elementsBlob.includes(profileRowNeedle),
			createRowVisible: elementsBlob.includes(
				"profile-search-row:create-new-profile",
			),
			appAlive: driver.alive,
		};
		if (ROW === "a1-valid-current") {
			checks.noParseFailure = !logsBlob.includes("mdflow_profile_parse_failed");
		} else if (ROW === "a2-malformed") {
			checks.badFrontmatterOmitted = !elementsBlob.includes(
				"profile-search-row:nn29-bad-frontmatter",
			);
			checks.badFrontmatterWarned =
				logsBlob.includes("mdflow_profile_parse_failed") &&
				logsBlob.includes("nn29-bad-frontmatter.md");
			checks.wrongTypesOmitted = !elementsBlob.includes(
				"profile-search-row:nn29-wrong-types",
			);
			checks.wrongTypesWarned = logsBlob.includes("nn29-wrong-types.md");
		} else if (ROW === "a3-outdated-remnants") {
			checks.directJsonIgnored =
				!elementsBlob.includes("plugin:legacy/direct") &&
				!elementsBlob.includes("NN29 Legacy Direct JSON");
			checks.nestedPluginJsonIgnored =
				!elementsBlob.includes("plugin:nn29-legacy/stale") &&
				!elementsBlob.includes("NN29 Legacy Nested JSON");
			checks.noOutdatedParseNoise =
				!logsBlob.includes("NN29 Legacy Direct JSON") &&
				!logsBlob.includes("NN29 Legacy Nested JSON");
		} else if (ROW === "a4-hostile-valid") {
			checks.hugeProfileVisibleAndFilterable =
				(hostileEvidence.huge as { visible?: boolean }).visible === true;
			checks.zalgoProfileVisibleAndFilterable =
				(hostileEvidence.zalgo as { visible?: boolean }).visible === true;
			checks.noHostileParseFailure = !logsBlob.includes(
				"mdflow_profile_parse_failed",
			);
		} else if (ROW === "a5-create") {
			checks.createRowSelectable = createEvidence.rowVisible === true;
			checks.createEventLogged = createEvidence.createLogged === true;
			checks.templateCreatedInScratch = createEvidence.templateValid === true;
			checks.returnedToScriptList =
				createEvidence.returnedToScriptList === true;
			checks.inertEditorOnly = logsBlob.includes("Opening file in editor");
		} else {
			checks.selectedProfilePersisted = deletionEvidence.persisted === true;
			checks.deletedProfileRowHealed =
				deletionEvidence.deletedRowAbsent === true;
			checks.currentProfileFellBackToBrain =
				(deletionEvidence.current as { value?: unknown } | undefined)?.value ===
				"brain";
		}

		receipt.sessionDir = driver.sessionDir;
		receipt.scratchHome = scratchHome;
		receipt.seededProfiles = seededPaths;
		receipt.observed = {
			state,
			profileRowNeedle,
			cacheRefreshWaitMs: elementWait.waitedMs,
			hostileEvidence,
			createEvidence,
			deletionEvidence,
			checks,
		};
		receipt.pass = Object.values(checks).every(Boolean);
		receipt.failedChecks = Object.entries(checks)
			.filter(([, passed]) => !passed)
			.map(([name]) => name);
		exitCode = receipt.pass ? 0 : 1;
	}
} catch (error) {
	const message = error instanceof Error ? error.message : String(error);
	const environmentBlocked =
		message.includes("did not become ready") ||
		message.includes("Pi Agent Chat is unavailable") ||
		message.includes("Binary not found");
	receipt.pass = false;
	receipt.verdict = "ABORTED";
	if (!rowIsImplemented) {
		receipt.classification = "ABORTED";
	} else if (environmentBlocked) {
		receipt.classification = "ENV";
	} else {
		receipt.classification = "HARNESS_OR_PRODUCT_RED";
	}
	receipt.error = message;
	receipt.thrownError = message;
	appendFailedCheck(rowIsImplemented ? "row_exception" : "row_not_implemented");
} finally {
	try {
		if (driver) {
			mkdirSync(RECEIPT_DIR, { recursive: true });
			await recordStateSample(driver, "final-before-cleanup");
			markTiming("diagnostic-bundle-start");
			const [elements, layout, windowsBeforeCleanup] = await Promise.all([
				diagnosticCall(
					"getElements",
					() =>
						driver?.getElements(
							{ target: { type: "main" }, limit: 1_000 },
							{ timeoutMs: 10_000 },
						) as Promise<unknown>,
				),
				diagnosticCall(
					"getLayoutInfo",
					() =>
						driver?.getLayoutInfo(
							{ target: { type: "main" } },
							{ timeoutMs: 10_000 },
						) as Promise<unknown>,
				),
				diagnosticCall(
					"listAutomationWindows",
					() =>
						driver?.listAutomationWindows({
							timeoutMs: 10_000,
						}) as Promise<unknown>,
				),
			]);
			driver.send({ type: "hide" });
			const hidden = await driver
				.waitForState({ windowVisible: false }, { timeoutMs: 4_000 })
				.then(() => true)
				.catch(() => false);
			await recordStateSample(driver, "after-hide", { hidden });
			const [logs, windowsAfterHide] = await Promise.all([
				diagnosticCall(
					"getLogs",
					() => driver?.getLogs({ limit: 2_000 }) as Promise<unknown>,
				),
				diagnosticCall(
					"listAutomationWindows-after-hide",
					() =>
						driver?.listAutomationWindows({
							timeoutMs: 10_000,
						}) as Promise<unknown>,
				),
			]);
			markTiming("diagnostic-bundle-collected");
			writeFileSync(
				resolve(RECEIPT_DIR, "elements.json"),
				`${JSON.stringify(elements, null, 2)}\n`,
			);
			writeFileSync(
				resolve(RECEIPT_DIR, "layout.json"),
				`${JSON.stringify(layout, null, 2)}\n`,
			);
			writeFileSync(
				resolve(RECEIPT_DIR, "app-logs.json"),
				`${JSON.stringify(logs, null, 2)}\n`,
			);
			writeFileSync(
				resolve(RECEIPT_DIR, "state-samples.json"),
				`${JSON.stringify(stateSamples, null, 2)}\n`,
			);
			writeFileSync(
				resolve(RECEIPT_DIR, "windows.json"),
				`${JSON.stringify({ beforeCleanup: windowsBeforeCleanup, afterHide: windowsAfterHide }, null, 2)}\n`,
			);
			receipt.diagnosticBundle = {
				directory: RECEIPT_DIR,
				elements: "elements.json",
				layout: "layout.json",
				logs: "app-logs.json",
				stateSamples: "state-samples.json",
				windows: "windows.json",
				timings: "timings.json",
			};
			receipt.cleanup = { hidden, pid: driver.pid ?? null };
			markTiming("driver-close-start");
			await driver.close();
			receipt.finalization = driver.finalization;
			markTiming("driver-close-complete", driver.finalization);
			writeFileSync(
				resolve(RECEIPT_DIR, "timings.json"),
				`${JSON.stringify(
					{
						probeStartedWallTime: new Date(PROBE_STARTED_WALL_MS).toISOString(),
						totalElapsedMs: Math.round(performance.now() - PROBE_STARTED_AT),
						milestones: timingMilestones,
					},
					null,
					2,
				)}\n`,
			);
		}
	} catch (finalizationError) {
		const message =
			finalizationError instanceof Error
				? finalizationError.message
				: String(finalizationError);
		receipt.pass = false;
		receipt.verdict = "ABORTED";
		receipt.classification = "HARNESS_OR_PRODUCT_RED";
		receipt.finalizationError = message;
		receipt.thrownError = message;
		receipt.error =
			typeof receipt.error === "string"
				? `${receipt.error}; finalization: ${message}`
				: `finalization: ${message}`;
		appendFailedCheck("finalization_exception");
		exitCode = 2;
	} finally {
		persistOutcomeArtifacts();
	}
}

console.log(JSON.stringify(receipt, null, 2));
process.exit(exitCode);
